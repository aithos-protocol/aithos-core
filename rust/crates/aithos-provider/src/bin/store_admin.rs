//! aithos-store-admin: the P7 control-plane admin (INFRA-PROVIDER §7,
//! lot P7) — the ONLY writer of the control table. Runs under the
//! OPERATOR's credentials (policy `…-control-admin`, module
//! `control-plane-min`); the store task role reads and never writes.
//!
//! Commands (every write is public enrollment material — no secret, no
//! private key, doctrine §1):
//!
//! ```text
//! aithos-store-admin create         <tenant>
//! aithos-store-admin bind-did       <tenant> <did>
//! aithos-store-admin bind-gateway   <tenant> <gateway_pub> <hostname>
//! aithos-store-admin unbind-gateway <tenant> <gateway_pub>
//! aithos-store-admin suspend        <tenant>
//! aithos-store-admin reactivate     <tenant>
//! aithos-store-admin purge          <tenant> --yes
//! ```
//!
//! P7b (bascule relay) : `bind-gateway` écrit DEUX items — le miroir
//! `tenant#<t>/gateway#<gw>` D'ABORD (une ligne miroir sans binding est un
//! poids mort inerte que `purge` balaie ; un binding sans miroir serait
//! INVISIBLE à `purge` — l'ordre est le fail-safe), puis le binding
//! `gateway#<gw>/meta {t, h, s:false}` qui ne s'écrit jamais par-dessus un
//! existant (`unbind-gateway` d'abord — un déplacement de hostname est un
//! geste délibéré). La suspension reste UNE écriture tenant-niveau : le
//! wire et le relay joignent l'état du tenant à la résolution (autorité
//! B.5, arbitrage ① P7b).
//!
//! Environment (twelve-factor, fail-closed like the service):
//!
//! | Variable | Rôle |
//! |---|---|
//! | `AITHOS_ADMIN_CONTROL_TABLE` | REQUIRED — the control-plane table |
//! | `AITHOS_ADMIN_OBJECTS_BUCKET`| REQUIRED for `purge` — the data bucket whose `t/<tenant>/` versions the purge sweeps |
//! | `AITHOS_ADMIN_HEADS_TABLE`   | REQUIRED for `purge` — the A.5 heads table whose tenant items the purge removes |
//!
//! `purge` IS the §8 GC runbook mechanized (the gate-6 manual sweep):
//! every S3 version under the tenant prefix, every heads item of the
//! tenant, then every control item — in that order, so a half-purge
//! leaves the tenant refusing (`unknown_tenant` needs the control rows
//! gone LAST or the data would be orphaned invisible). It requires the
//! explicit `--yes` — an irreversible sweep never rides a typo.
//!
//! Freshness note: the service caches control reads for
//! `AITHOS_STORE_CONTROL_TTL_SECS` (30 s default) — every write here is
//! live on the wire within that bound (< 60 s promise, P7 gate).

use aws_sdk_dynamodb::types::AttributeValue;

fn required(name: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => {
            eprintln!("fatal: {name} is required (fail-closed)");
            std::process::exit(2);
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: aithos-store-admin <create|bind-did|suspend|reactivate> <tenant> [<did>]\n\
         \x20      aithos-store-admin bind-gateway <tenant> <gateway_pub> <hostname>\n\
         \x20      aithos-store-admin unbind-gateway <tenant> <gateway_pub>\n\
         \x20      aithos-store-admin purge <tenant> --yes"
    );
    std::process::exit(2);
}

/// The A.1 tenant grammar (lowercase dns-label shape) — the admin refuses
/// junk BEFORE it lands in the table; the service would refuse it anyway
/// (`path_invalid`), an unroutable tenant row is dead weight.
fn check_tenant(tenant: &str) {
    let ok = !tenant.is_empty()
        && tenant.len() <= 63
        && tenant
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !tenant.starts_with('-')
        && !tenant.ends_with('-');
    if !ok {
        eprintln!("fatal: tenant `{tenant}` is outside the A.1 grammar");
        std::process::exit(2);
    }
}

fn check_did(did: &str) {
    if !did.starts_with("did:aithos:z") {
        eprintln!("fatal: did `{did}` is not a did:aithos identifier");
        std::process::exit(2);
    }
}

/// P7b: the gateway key must be REAL public material — it decodes as a
/// multibase Ed25519 key or it never lands in the table (the relay would
/// only ever answer `signature_invalid` on it: dead weight).
fn check_gateway_pub(gateway_pub: &str) {
    if aithos_core::wire::multibase_to_ed25519_pub(gateway_pub).is_err() {
        eprintln!("fatal: gateway_pub `{gateway_pub}` is not a multibase Ed25519 public key");
        std::process::exit(2);
    }
}

/// P7b: the tunnel hostname grammar — lowercase DNS labels (each the A.1
/// shape) joined by dots, ≤ 253 chars. The relay matches SNI exactly and
/// case-insensitively lowered; junk here would be an unroutable row.
fn hostname_ok(hostname: &str) -> bool {
    hostname.len() <= 253
        && !hostname.is_empty()
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

fn check_hostname(hostname: &str) {
    if !hostname_ok(hostname) {
        eprintln!("fatal: hostname `{hostname}` is outside the tunnel hostname grammar");
        std::process::exit(2);
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, tenant) = match (args.first(), args.get(1)) {
        (Some(c), Some(t)) => (c.as_str(), t.as_str()),
        _ => usage(),
    };
    check_tenant(tenant);

    let table = required("AITHOS_ADMIN_CONTROL_TABLE");
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let ddb = aws_sdk_dynamodb::Client::new(&config);
    let meta_pk = format!("tenant#{tenant}");

    match command {
        "create" => {
            // Creation is distinct: an existing tenant (even suspended)
            // refuses — a suspended state must never be clobbered back to
            // active by a re-run.
            let put = ddb
                .put_item()
                .table_name(&table)
                .item("pk", AttributeValue::S(meta_pk))
                .item("sk", AttributeValue::S("meta".into()))
                .item("s", AttributeValue::Bool(false))
                .condition_expression("attribute_not_exists(pk)")
                .send()
                .await;
            match put {
                Ok(_) => println!("created tenant {tenant} (active)"),
                Err(e) if is_conditional_failure(&e) => {
                    eprintln!("fatal: tenant {tenant} already exists (create never overwrites)");
                    std::process::exit(1);
                }
                Err(e) => fail(&e),
            }
        }
        "bind-did" => {
            let Some(did) = args.get(2) else { usage() };
            check_did(did);
            // The binding demands its tenant row first — no orphan bindings.
            if !meta_exists(&ddb, &table, tenant).await {
                eprintln!("fatal: tenant {tenant} does not exist (create it first)");
                std::process::exit(1);
            }
            match ddb
                .put_item()
                .table_name(&table)
                .item("pk", AttributeValue::S(format!("tenant#{tenant}")))
                .item("sk", AttributeValue::S(format!("did#{did}")))
                .send()
                .await
            {
                Ok(_) => println!("bound {did} to tenant {tenant}"),
                Err(e) => fail(&e),
            }
        }
        "bind-gateway" => {
            let (Some(gateway_pub), Some(hostname)) = (args.get(2), args.get(3)) else {
                usage()
            };
            check_gateway_pub(gateway_pub);
            check_hostname(hostname);
            // The binding demands its tenant row first — no orphan
            // bindings (the B.5/B.2 join would refuse them anyway).
            if !meta_exists(&ddb, &table, tenant).await {
                eprintln!("fatal: tenant {tenant} does not exist (create it first)");
                std::process::exit(1);
            }
            // Refuse a key already enrolled — for THIS tenant or another
            // (one gateway key is one identity; a move is `unbind-gateway`
            // first, a deliberate two-step).
            if gateway_row(&ddb, &table, gateway_pub).await.is_some() {
                eprintln!(
                    "fatal: gateway is already enrolled (unbind-gateway first — \
                     bind-gateway never overwrites)"
                );
                std::process::exit(1);
            }
            // Mirror FIRST (fail-safe order, doc header): a mirror without
            // a binding is inert dead weight purge sweeps; a binding
            // without a mirror would be invisible to purge.
            if let Err(e) = ddb
                .put_item()
                .table_name(&table)
                .item("pk", AttributeValue::S(format!("tenant#{tenant}")))
                .item("sk", AttributeValue::S(format!("gateway#{gateway_pub}")))
                .send()
                .await
            {
                fail(&e);
            }
            let put = ddb
                .put_item()
                .table_name(&table)
                .item("pk", AttributeValue::S(format!("gateway#{gateway_pub}")))
                .item("sk", AttributeValue::S("meta".into()))
                .item("t", AttributeValue::S(tenant.to_owned()))
                .item("h", AttributeValue::S(hostname.clone()))
                .item("s", AttributeValue::Bool(false))
                .condition_expression("attribute_not_exists(pk)")
                .send()
                .await;
            match put {
                Ok(_) => println!(
                    "bound gateway to tenant {tenant}, hostname {hostname} \
                     (live on the wire within the control TTL)"
                ),
                Err(e) if is_conditional_failure(&e) => {
                    // Raced by another enrollment between the precheck and
                    // the conditional put (single-writer doctrine makes
                    // this operator error, not a code path): the mirror we
                    // just wrote is inert; report and stop.
                    eprintln!("fatal: gateway was enrolled concurrently (unbind-gateway first)");
                    std::process::exit(1);
                }
                Err(e) => fail(&e),
            }
        }
        "unbind-gateway" => {
            let Some(gateway_pub) = args.get(2) else {
                usage()
            };
            check_gateway_pub(gateway_pub);
            // The binding must exist AND belong to this tenant — an unbind
            // never reaches across tenants.
            match gateway_row(&ddb, &table, gateway_pub).await {
                None => {
                    eprintln!("fatal: gateway is not enrolled");
                    std::process::exit(1);
                }
                Some(item) => {
                    let owner = item.get("t").and_then(|v| v.as_s().ok().cloned());
                    if owner.as_deref() != Some(tenant) {
                        eprintln!("fatal: gateway is not enrolled for tenant {tenant}");
                        std::process::exit(1);
                    }
                }
            }
            // Binding FIRST (resolution dies immediately), mirror second —
            // a leftover mirror is inert and purge-swept.
            if let Err(e) = ddb
                .delete_item()
                .table_name(&table)
                .key("pk", AttributeValue::S(format!("gateway#{gateway_pub}")))
                .key("sk", AttributeValue::S("meta".into()))
                .condition_expression("t = :t")
                .expression_attribute_values(":t", AttributeValue::S(tenant.to_owned()))
                .send()
                .await
            {
                if is_conditional_failure(&e) {
                    eprintln!("fatal: gateway enrollment changed concurrently");
                    std::process::exit(1);
                }
                fail(&e);
            }
            if let Err(e) = ddb
                .delete_item()
                .table_name(&table)
                .key("pk", AttributeValue::S(format!("tenant#{tenant}")))
                .key("sk", AttributeValue::S(format!("gateway#{gateway_pub}")))
                .send()
                .await
            {
                fail(&e);
            }
            println!(
                "unbound gateway from tenant {tenant} (registrations refuse within the \
                 control TTL; the active tunnel closes at the next reconcile sweep)"
            );
        }
        "suspend" | "reactivate" => {
            let suspended = command == "suspend";
            let update = ddb
                .update_item()
                .table_name(&table)
                .key("pk", AttributeValue::S(meta_pk))
                .key("sk", AttributeValue::S("meta".into()))
                .update_expression("SET s = :s")
                .expression_attribute_values(":s", AttributeValue::Bool(suspended))
                .condition_expression("attribute_exists(pk)")
                .send()
                .await;
            match update {
                Ok(_) => println!(
                    "{} tenant {tenant} (live on the wire within the control TTL)",
                    if suspended {
                        "suspended"
                    } else {
                        "reactivated"
                    }
                ),
                Err(e) if is_conditional_failure(&e) => {
                    eprintln!("fatal: tenant {tenant} does not exist");
                    std::process::exit(1);
                }
                Err(e) => fail(&e),
            }
        }
        "purge" => {
            if args.get(2).map(String::as_str) != Some("--yes") {
                eprintln!("fatal: purge is irreversible — repeat with --yes");
                std::process::exit(2);
            }
            let bucket = required("AITHOS_ADMIN_OBJECTS_BUCKET");
            let heads_table = required("AITHOS_ADMIN_HEADS_TABLE");
            let s3 = aws_sdk_s3::Client::new(&config);

            // §8 GC runbook order: data plane first (S3 versions, heads),
            // control rows LAST — the tenant keeps refusing on the wire
            // while its data drains, and a re-run resumes cleanly.
            let swept = purge_s3_versions(&s3, &bucket, tenant).await;
            println!("swept {swept} S3 version(s) under t/{tenant}/");
            let heads = purge_heads(&ddb, &heads_table, tenant).await;
            println!("removed {heads} heads item(s)");
            // P7b: the gateway bindings (found through the tenant's mirror
            // rows) go BEFORE the tenant partition — the relay's reconcile
            // sweep closes the tunnels, and the rows purge LAST keeps the
            // tenant refusing while its data drains.
            let gateways = purge_gateway_rows(&ddb, &table, tenant).await;
            println!("removed {gateways} gateway binding(s)");
            let rows = purge_control_rows(&ddb, &table, tenant).await;
            println!("removed {rows} control row(s) — tenant {tenant} purged");
        }
        _ => usage(),
    }
}

fn is_conditional_failure<E, R>(err: &aws_sdk_dynamodb::error::SdkError<E, R>) -> bool
where
    E: aws_sdk_dynamodb::error::ProvideErrorMetadata,
{
    matches!(
        aws_sdk_dynamodb::error::ProvideErrorMetadata::code(err),
        Some("ConditionalCheckFailedException")
    )
}

fn fail<E: std::fmt::Debug>(err: &E) -> ! {
    // Operator tool: the AWS error surface is the operator's to read —
    // no request-path log discipline applies here (nothing of A.8 rides).
    eprintln!("fatal: aws error: {err:?}");
    std::process::exit(1);
}

/// The gateway binding row, if enrolled (P7b bind/unbind precondition).
async fn gateway_row(
    client: &aws_sdk_dynamodb::Client,
    table: &str,
    gateway_pub: &str,
) -> Option<std::collections::HashMap<String, AttributeValue>> {
    match client
        .get_item()
        .table_name(table)
        .key("pk", AttributeValue::S(format!("gateway#{gateway_pub}")))
        .key("sk", AttributeValue::S("meta".into()))
        .send()
        .await
    {
        Ok(got) => got.item,
        Err(e) => fail(&e),
    }
}

async fn meta_exists(client: &aws_sdk_dynamodb::Client, table: &str, tenant: &str) -> bool {
    match client
        .get_item()
        .table_name(table)
        .key("pk", AttributeValue::S(format!("tenant#{tenant}")))
        .key("sk", AttributeValue::S("meta".into()))
        .send()
        .await
    {
        Ok(got) => got.item().is_some(),
        Err(e) => fail(&e),
    }
}

/// Every version and delete-marker under `t/<tenant>/` — the bucket is
/// versioned (A5), a plain delete would only stack markers.
async fn purge_s3_versions(client: &aws_sdk_s3::Client, bucket: &str, tenant: &str) -> usize {
    let prefix = format!("t/{tenant}/");
    let mut swept = 0usize;
    let mut key_marker: Option<String> = None;
    let mut version_marker: Option<String> = None;
    loop {
        let page = match client
            .list_object_versions()
            .bucket(bucket)
            .prefix(&prefix)
            .set_key_marker(key_marker.clone())
            .set_version_id_marker(version_marker.clone())
            .send()
            .await
        {
            Ok(p) => p,
            Err(e) => fail(&e),
        };
        let mut targets: Vec<(String, String)> = Vec::new();
        for v in page.versions() {
            if let (Some(k), Some(id)) = (v.key(), v.version_id()) {
                targets.push((k.to_owned(), id.to_owned()));
            }
        }
        for m in page.delete_markers() {
            if let (Some(k), Some(id)) = (m.key(), m.version_id()) {
                targets.push((k.to_owned(), id.to_owned()));
            }
        }
        for (key, version_id) in targets {
            if let Err(e) = client
                .delete_object()
                .bucket(bucket)
                .key(&key)
                .version_id(&version_id)
                .send()
                .await
            {
                fail(&e);
            }
            swept += 1;
        }
        if page.is_truncated() == Some(true) {
            key_marker = page.next_key_marker().map(str::to_owned);
            version_marker = page.next_version_id_marker().map(str::to_owned);
        } else {
            return swept;
        }
    }
}

/// Every heads item of the tenant (hash key `t`, range `d` — A.5 table).
async fn purge_heads(client: &aws_sdk_dynamodb::Client, table: &str, tenant: &str) -> usize {
    let mut removed = 0usize;
    loop {
        let page = match client
            .query()
            .table_name(table)
            .key_condition_expression("#t = :t")
            .expression_attribute_names("#t", "t")
            .expression_attribute_values(":t", AttributeValue::S(tenant.to_owned()))
            .send()
            .await
        {
            Ok(p) => p,
            Err(e) => fail(&e),
        };
        let items = page.items();
        if items.is_empty() {
            return removed;
        }
        for item in items {
            let Some(did) = item.get("d").and_then(|v| v.as_s().ok()) else {
                continue;
            };
            if let Err(e) = client
                .delete_item()
                .table_name(table)
                .key("t", AttributeValue::S(tenant.to_owned()))
                .key("d", AttributeValue::S(did.clone()))
                .send()
                .await
            {
                fail(&e);
            }
            removed += 1;
        }
    }
}

/// Every gateway binding of the tenant, found through its mirror rows
/// (`tenant#<t>/gateway#<gw>` — written by `bind-gateway`, the only way a
/// binding lands). P7b: swept BEFORE the tenant partition so no
/// `gateway#…/meta` row is ever orphaned invisible.
async fn purge_gateway_rows(client: &aws_sdk_dynamodb::Client, table: &str, tenant: &str) -> usize {
    let pk = format!("tenant#{tenant}");
    let mut removed = 0usize;
    loop {
        let page = match client
            .query()
            .table_name(table)
            .key_condition_expression("pk = :pk AND begins_with(sk, :g)")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .expression_attribute_values(":g", AttributeValue::S("gateway#".into()))
            .send()
            .await
        {
            Ok(p) => p,
            Err(e) => fail(&e),
        };
        let items = page.items();
        if items.is_empty() {
            return removed;
        }
        for item in items {
            let Some(sk) = item.get("sk").and_then(|v| v.as_s().ok()) else {
                continue;
            };
            let Some(gateway_pub) = sk.strip_prefix("gateway#") else {
                continue;
            };
            // The binding row dies FIRST, then its mirror — and the delete
            // is CONDITIONAL on `t = <tenant>` (verdict du témoin de gate
            // P7b, D1) : un miroir périmé (bind perdu sur course, crash
            // entre miroir et binding, unbind interrompu) ne doit JAMAIS
            // détruire le binding vif d'un AUTRE tenant. Condition fausse
            // (binding absent ou possédé ailleurs) → on ne balaie que le
            // miroir.
            match client
                .delete_item()
                .table_name(table)
                .key("pk", AttributeValue::S(format!("gateway#{gateway_pub}")))
                .key("sk", AttributeValue::S("meta".into()))
                .condition_expression("t = :t")
                .expression_attribute_values(":t", AttributeValue::S(tenant.to_owned()))
                .send()
                .await
            {
                Ok(_) => {}
                Err(e) if is_conditional_failure(&e) => {
                    println!(
                        "note: stale mirror for gateway#{gateway_pub} (binding absent or \
                         owned by another tenant) — mirror swept, binding untouched"
                    );
                }
                Err(e) => fail(&e),
            }
            if let Err(e) = client
                .delete_item()
                .table_name(table)
                .key("pk", AttributeValue::S(pk.clone()))
                .key("sk", AttributeValue::S(sk.clone()))
                .send()
                .await
            {
                fail(&e);
            }
            removed += 1;
        }
    }
}

/// Every control row of the tenant partition (meta + did bindings + any
/// leftover gateway mirrors), LAST.
async fn purge_control_rows(client: &aws_sdk_dynamodb::Client, table: &str, tenant: &str) -> usize {
    let pk = format!("tenant#{tenant}");
    let mut removed = 0usize;
    loop {
        let page = match client
            .query()
            .table_name(table)
            .key_condition_expression("pk = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()))
            .send()
            .await
        {
            Ok(p) => p,
            Err(e) => fail(&e),
        };
        let items = page.items();
        if items.is_empty() {
            return removed;
        }
        for item in items {
            let Some(sk) = item.get("sk").and_then(|v| v.as_s().ok()) else {
                continue;
            };
            if let Err(e) = client
                .delete_item()
                .table_name(table)
                .key("pk", AttributeValue::S(pk.clone()))
                .key("sk", AttributeValue::S(sk.clone()))
                .send()
                .await
            {
                fail(&e);
            }
            removed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hostname_ok;

    /// The P7b tunnel-hostname grammar refuses junk BEFORE it lands in the
    /// table (an unroutable row is dead weight; the relay lowercases and
    /// matches exactly).
    #[test]
    fn the_hostname_grammar_is_lowercase_dns() {
        for ok in ["demo.mcp.aithos.fr", "a.b", "x1-y2.mcp.aithos.fr"] {
            assert!(hostname_ok(ok), "{ok} should pass");
        }
        for bad in [
            "",
            "Demo.mcp.aithos.fr", // uppercase: the relay lowers SNI, rows are stored lowered
            "-x.mcp.aithos.fr",
            "x-.mcp.aithos.fr",
            "a..b",
            ".a",
            "a.",
            "un_der.score",
        ] {
            assert!(!hostname_ok(bad), "{bad} should refuse");
        }
        // 63-char label passes, 64 refuses; 253 total passes, 254 refuses.
        let l63 = "a".repeat(63);
        let l64 = "a".repeat(64);
        assert!(hostname_ok(&format!("{l63}.fr")));
        assert!(!hostname_ok(&format!("{l64}.fr")));
        let mut long = vec!["a"; 100].join(".");
        long.push_str(&"a".repeat(253 - long.len()));
        assert!(hostname_ok(&long[..253].trim_end_matches('.').to_string()));
        assert!(!hostname_ok(&format!("{}.{}", "a".repeat(63), "b".repeat(190))));
    }
}

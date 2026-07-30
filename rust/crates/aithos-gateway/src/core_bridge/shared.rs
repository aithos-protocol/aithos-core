//! Helpers du runtime de la gateway et helpers partagés
//! runtime/cérémonies propriétaire — isolés de `core_bridge` au lot
//! SPL-3 du chantier split, sans changement de comportement. Le bloc
//! owner (les `pub fn owner_*`, leurs lecteurs et leurs helpers à
//! runtime = 0) reste dans `core_bridge.rs` jusqu'au lot SPL-4.

#[allow(unused_imports)]
use super::*;

/// Verify SC1 and the two independent proofs over the same operation_ref.
/// No session-specific verifier is implemented in the gateway.
pub fn verify_delegated_session(
    evidence: DelegatedSessionEvidence<'_>,
) -> Result<serde_json::Value> {
    verify_session(SessionEvidence {
        mandate: evidence.mandate,
        certificate: evidence.certificate,
        projection: evidence.projection,
        operation_ref: evidence.operation_ref,
        native_leaf_proof: evidence.native_leaf_proof,
        native_leaf_domain: MCP_SESSION_NATIVE_PROOF_DOMAIN,
        session_proof: evidence.session_proof,
    })
    .map(|verified| verified.operation_ref().clone())
    .map_err(|error| GatewayError::MandateDenied {
        op: "delegated_session".into(),
        reason: error.to_string(),
    })
}

pub fn verify_delegated_chain_session(
    evidence: DelegatedChainSessionEvidence<'_>,
) -> Result<serde_json::Value> {
    verify_delegated_session_core(CoreDelegatedSessionEvidence {
        chain: evidence.chain,
        did: evidence.did,
        at: evidence.at,
        revocations: evidence.revocations,
        session: SessionEvidence {
            mandate: evidence.mandate,
            certificate: evidence.certificate,
            projection: evidence.projection,
            operation_ref: evidence.operation_ref,
            native_leaf_proof: Some(evidence.native_leaf_proof),
            native_leaf_domain: MCP_SESSION_NATIVE_PROOF_DOMAIN,
            session_proof: Some(evidence.session_proof),
        },
    })
    .map(|verified| verified.operation_ref().clone())
    .map_err(|error| GatewayError::MandateDenied {
        op: "delegated_session".into(),
        reason: error.to_string(),
    })
}

/// Apply Core's active-session tally to an injected, already-verified set.
pub fn enforce_max_sessions(max_sessions: u64, active_session_keys: &[&str]) -> Result<usize> {
    verify_max_sessions(max_sessions, active_session_keys)
        .map(|verified| verified.active())
        .map_err(|error| GatewayError::MandateDenied {
            op: "delegated_session".into(),
            reason: error.to_string(),
        })
}

/// Where mandate certificates live in the store.
pub(crate) fn cert_path(id: &str) -> String {
    format!("certs/{id}.json")
}

/// Wire encoding of the agent public key (`z…`, base58btc multicodec) —
/// the ONLY thing that leaves the runner at birth.
pub fn agent_pub_multibase(kh: &Keyholder) -> String {
    let vk = SigningKey::from_bytes(kh.agent_seed()).verifying_key();
    aithos_core::wire::ed25519_pub_to_multibase(&vk.to_bytes())
}

pub fn agent_kex_pub_multibase(kh: &Keyholder) -> String {
    let signing = SigningKey::from_bytes(kh.agent_seed());
    let public = ed2x(&signing.verifying_key());
    aithos_core::wire::x25519_pub_to_multibase(public.as_bytes())
}

/// Wire encoding of the gateway's own public key.
pub fn gateway_pub_multibase(kh: &Keyholder) -> String {
    let vk = SigningKey::from_bytes(kh.gateway_seed()).verifying_key();
    aithos_core::wire::ed25519_pub_to_multibase(&vk.to_bytes())
}

pub fn gateway_kex_pub_multibase(kh: &Keyholder) -> String {
    let signing = SigningKey::from_bytes(kh.gateway_seed());
    let public = ed2x(&signing.verifying_key());
    aithos_core::wire::x25519_pub_to_multibase(public.as_bytes())
}

/// Build the one bounded C2/B.2 registration line under the existing
/// gateway identity. The private seed remains inside the bridge; callers
/// cannot ask the keyholder to sign arbitrary bytes.
pub fn gateway_tunnel_registration_line(
    kh: &Keyholder,
    tenant: &str,
    hostname: &str,
    at: &str,
    nonce: &str,
) -> Result<Vec<u8>> {
    let signing = SigningKey::from_bytes(kh.gateway_seed());
    crate::relay::registration_line_with_key(
        tenant,
        hostname,
        &gateway_pub_multibase(kh),
        at,
        nonce,
        &signing,
    )
}

/// Build the bounded B.5 `X-Aithos-Auth` value for the delegated
/// DNS-01 endpoint. This is deliberately not a general-purpose signing
/// primitive: the path, version, mandate set and signature algorithm are
/// fixed here, and only PUT/DELETE challenge effects are admitted.
pub fn gateway_acme_authorization_header(
    kh: &Keyholder,
    host: &str,
    method: &str,
    body: &[u8],
    at: &str,
    nonce: &str,
) -> Result<String> {
    use ed25519_dalek::Signer as _;

    if !matches!(method, "PUT" | "DELETE")
        || host.is_empty()
        || host
            .bytes()
            .any(|byte| byte.is_ascii_uppercase() || byte <= b' ')
        || at.is_empty()
        || nonce.len() < 16
        || nonce.len() > 64
        || nonce.contains('\n')
    {
        return Err(GatewayError::RelayUnavailable(
            "acme_authorization_input_invalid".into(),
        ));
    }
    let signing = SigningKey::from_bytes(kh.gateway_seed());
    let mut envelope = serde_json::json!({
        "v": 1,
        "host": host,
        "method": method,
        "path": "/acme/txt",
        "body_b3": blake3::hash(body).to_hex().to_string(),
        "at": at,
        "nonce": nonce,
        "mandate": [],
        "key": gateway_pub_multibase(kh),
        "signature": { "alg": "ed25519", "value": "" },
    });
    let unsigned = aithos_core::jcs::canonicalize(&envelope)
        .map_err(|_| GatewayError::RelayUnavailable("acme_authorization_encode_failed".into()))?;
    envelope["signature"]["value"] =
        serde_json::Value::String(hex::encode(signing.sign(unsigned.as_bytes()).to_bytes()));
    let signed = aithos_core::jcs::canonicalize(&envelope)
        .map_err(|_| GatewayError::RelayUnavailable("acme_authorization_encode_failed".into()))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signed.as_bytes()))
}

pub(crate) fn enrollment_chain_is_direct_owner(chain: &[Mandate]) -> bool {
    chain.len() == 1 && chain[0].parent.is_none()
}

pub(crate) fn validate_runtime_tool(
    context: &str,
    exposed: &str,
    reference: &crate::config::HubToolRef,
    approved: &ApprovedTool,
) -> Result<()> {
    if approved.exposed_name != exposed {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` names `{exposed}`, approved manifest names `{}`",
            approved.exposed_name
        )));
    }
    if approved.risk_class != reference.access {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` class for `{exposed}` differs from the approved manifest"
        )));
    }
    if reference.is_granted() != approved.is_granted() {
        return Err(GatewayError::ConfigRejected(format!(
            "context `{context}` grant decision for `{exposed}` differs from the approved manifest"
        )));
    }
    Ok(())
}

pub(crate) fn merge_server_pins(
    pins: &mut BTreeMap<String, BTreeMap<String, String>>,
    manifest: &ApprovedManifest,
) -> Result<()> {
    let candidate: BTreeMap<String, String> = manifest
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.pin_sha256.clone()))
        .collect();
    if let Some(existing) = pins.get(&manifest.server) {
        if existing != &candidate {
            return Err(GatewayError::ConfigRejected(format!(
                "contexts pin conflicting manifests for shared server `{}`",
                manifest.server
            )));
        }
    } else {
        pins.insert(manifest.server.clone(), candidate);
    }
    Ok(())
}

pub(crate) fn hub_manifest_paths(server: &str) -> (String, String) {
    (
        format!("e/x/{server}/header.json"),
        format!("e/x/{server}/{HUB_MANIFEST_FILE}"),
    )
}

/// Pin of exactly the upstream-controlled fields approved by the owner.
pub fn manifest_tool_pin(
    name: &str,
    description: Option<&str>,
    input_schema: &serde_json::Value,
) -> Result<String> {
    hash_of(&serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    }))
}

/// Stable digest of the exact upstream-controlled catalogue sealed in an
/// owner-approved H3 manifest. Risk/grant/bounds remain owner policy and do
/// not alter the live-discovery comparison digest.
pub fn approved_manifest_catalog_digest(manifest: &ApprovedManifest) -> Result<String> {
    validate_approved(manifest)?;
    manifest_catalog_digest(
        &manifest.server,
        manifest
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str())),
    )
}

pub fn proposed_manifest_catalog_digest(manifest: &ProposedManifest) -> Result<String> {
    manifest_catalog_digest(
        &manifest.server,
        manifest
            .tools
            .iter()
            .map(|tool| (tool.name.as_str(), tool.pin_sha256.as_str())),
    )
}

/// Mint one root mandate: `ops` are perimeter entry strings. Every
/// caller passes its constraints explicitly (empty object = none).
#[allow(clippy::too_many_arguments)]
pub(crate) fn mint(
    owner: &OwnerKeys,
    bundle: &Bundle<GatewayStore>,
    ent: &mut dyn EntropySource,
    label: &str,
    grantee_pub: &ed25519_dalek::VerifyingKey,
    ops: &[String],
    constraints: serde_json::Value,
    window: &MandateWindow,
    now: &str,
) -> Result<Mandate> {
    let perimeter = ops
        .iter()
        .map(|op| PerimeterEntry::parse(op).map_err(bridge_err))
        .collect::<Result<Vec<_>>>()?;
    mint_entries(
        owner,
        bundle,
        ent,
        label,
        grantee_pub,
        perimeter,
        constraints,
        window,
        now,
    )
}

/// No constraints — the shape most mints use.
pub(crate) fn no_constraints() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

pub(crate) fn view(e: &aithos_core::gamma::Entry) -> EntryView {
    EntryView {
        id: e.id.clone(),
        at: e.at.clone(),
        kind: e.kind.clone(),
        target: e.target.clone(),
        authorized_via: e.authorized_via.clone(),
        payload: e.payload.clone(),
    }
}

pub(crate) fn hash_of(value: &serde_json::Value) -> Result<String> {
    let canon = aithos_core::jcs::canonical_bytes(value).map_err(bridge_err)?;
    Ok(format!("sha256:{}", aithos_core::gamma::sha256_hex(&canon)))
}

pub(crate) fn commitment_of(domain: &str, value: &serde_json::Value) -> Result<String> {
    let canon = aithos_core::jcs::canonical_bytes(value).map_err(bridge_err)?;
    let mut preimage = domain.as_bytes().to_vec();
    preimage.push(0);
    preimage.extend_from_slice(&canon);
    Ok(format!(
        "sha256:{}",
        aithos_core::gamma::sha256_hex(&preimage)
    ))
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(
    bundle: &Bundle<GatewayStore>,
    path: &str,
) -> Result<T> {
    let bytes = bundle
        .store
        .get(path)
        .map_err(bridge_err)?
        .ok_or_else(|| GatewayError::BridgeFailed(format!("missing {path}")))?;
    serde_json::from_slice(&bytes).map_err(bridge_err)
}

/// Read the bridge state, migrating the pre-SPL-2 key on first touch:
/// when [`STATE_PATH`] is absent and [`LEGACY_STATE_PATH`] present, the
/// bytes are copied verbatim under the new key — the legacy object is
/// never deleted — then read back from the new key.
pub(crate) fn read_state_migrating(bundle: &mut Bundle<GatewayStore>) -> Result<BridgeState> {
    if bundle.store.get(STATE_PATH).map_err(bridge_err)?.is_none() {
        // The legacy key left the canonical grammar with SPL-2 — the read
        // is a raw pod-territory access, never a Store::get.
        if let Some(legacy) = bundle.store.legacy_state_bytes().map_err(bridge_err)? {
            bundle.store.put(STATE_PATH, &legacy).map_err(bridge_err)?;
        }
    }
    read_json(bundle, STATE_PATH)
}

pub(crate) fn bridge_err(e: impl std::fmt::Display) -> GatewayError {
    GatewayError::BridgeFailed(e.to_string())
}

/// The memory shelf's clear index rows, oldest first — see [`zone_rows`].
pub(crate) fn memory_rows(
    bundle: &Bundle<GatewayStore>,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    zone_rows(bundle, Zone::Circle, MEMORY_FOLDER, query, tag)
}

/// One zone folder's clear index rows, oldest first, optionally filtered
/// by a case-insensitive `query` over name/title/tags and an exact
/// `tag`. This reads the SKELETON the readability frontier already
/// grants whoever holds the files — no body is touched here. A folder
/// that does not exist yields no rows (nothing was ever shelved there).
pub(crate) fn zone_rows(
    bundle: &Bundle<GatewayStore>,
    zone: Zone,
    folder: &str,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<MemoryRow>> {
    let Ok(folders) = bundle.resolve_folder(zone, folder) else {
        return Ok(Vec::new());
    };
    let folder_sid = folders.last().map(ToString::to_string);
    let index: serde_json::Value = read_json(bundle, &format!("e/{}/index.json", zone.as_str()))?;
    let needle = query.map(str::to_lowercase);
    let mut rows = Vec::new();
    for row in index["sections"].as_array().into_iter().flatten() {
        if row["folder_sid"].as_str().map(str::to_owned) != folder_sid {
            continue;
        }
        let name = row["name"].as_str().unwrap_or_default().to_owned();
        let title = row["title"].as_str().unwrap_or_default().to_owned();
        let tags: Vec<String> = row["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(q) = &needle {
            let hay = format!(
                "{}\u{0}{}\u{0}{}",
                name.to_lowercase(),
                title.to_lowercase(),
                tags.join("\u{0}").to_lowercase()
            );
            if !hay.contains(q.as_str()) {
                continue;
            }
        }
        if let Some(t) = tag {
            if !tags.iter().any(|x| x == t) {
                continue;
            }
        }
        rows.push(MemoryRow { name, title, tags });
    }
    Ok(rows)
}

/// Verify one concrete row with its SID. This preserves the historical
/// folder/tag semantics and additionally supports exact `#id=` grants.
/// K1-C's public carrier deliberately exposes no folder SIDs or tags, so
/// those rows carry empty dimensions: zone-wide and exact-id grants work,
/// while unverifiable folder/tag-restricted grants remain fail-closed.
pub(crate) fn ethos_row_is_covered(
    chain: &[Mandate],
    doc: &DidDocument,
    now: &str,
    zone: Zone,
    row: &EthosRow,
) -> bool {
    if verify_chain(chain, doc, now).is_err() {
        return false;
    }
    let Some(leaf) = chain.last() else {
        return false;
    };
    let Ok(perimeter) = leaf.parsed_perimeter() else {
        return false;
    };
    covers_section_op(
        &perimeter,
        &SectionOp {
            verb: Verb::Read,
            zone,
            sid: row.sid,
            folders: &row.folders,
            tags: &row.tags,
        },
    )
}

/// Read the active public carrier. A present K1-C index is authoritative:
/// malformed modern state never falls back to a possibly stale legacy copy.
pub(crate) fn public_read_current(
    bundle: &Bundle<GatewayStore>,
    path: &str,
) -> std::result::Result<String, aithos_core::error::Error> {
    match bundle.store.get("indices/public.json") {
        Ok(Some(_)) => Bundle::public_read_k1c(&bundle.store, path),
        Ok(None) => Bundle::public_read(&bundle.store, path),
        Err(error) => Err(aithos_core::error::Error::SealRejected(format!(
            "public index read: {error}"
        ))),
    }
}

/// The whole CLEAR index of one zone, display paths resolved — the
/// readability frontier (§02.1: the gateway holds the files); AUTHORITY
/// is checked per row by the callers. A zone with no index yields no
/// rows. `self` never goes through here in v1: its structure is sealed
/// and the delegated resolution is its own core lot. The `briefing/`
/// shelves are EXCLUDED: the owner's directives keep their own
/// dedicated surface (`briefing.read`, lot K) — the data tools serve
/// the rest of the Ethos, and the demo hot path stays byte-identical.
pub(crate) fn zone_all_rows(bundle: &Bundle<GatewayStore>, zone: Zone) -> Vec<EthosRow> {
    if zone == Zone::Public {
        match bundle.store.get("indices/public.json") {
            Ok(Some(bytes)) => {
                let Ok(index) =
                    serde_json::from_slice::<aithos_bundle::bundle::K1cPublicIndex>(&bytes)
                else {
                    return Vec::new();
                };
                if index.root_digest().is_err() {
                    return Vec::new();
                }
                return index
                    .sections
                    .into_iter()
                    .filter_map(|row| {
                        if row.path.split('/').next() == Some(BRIEFING_FOLDER) {
                            return None;
                        }
                        let sid = Sid::parse(&row.sid).ok()?;
                        let title = row
                            .path
                            .rsplit('/')
                            .next()
                            .unwrap_or(row.path.as_str())
                            .to_owned();
                        Some(EthosRow {
                            sid,
                            path: row.path,
                            title,
                            tags: Vec::new(),
                            folders: Vec::new(),
                        })
                    })
                    .collect();
            }
            Ok(None) => {}
            Err(_) => return Vec::new(),
        }
    }
    let Ok(index) =
        read_json::<serde_json::Value>(bundle, &format!("e/{}/index.json", zone.as_str()))
    else {
        return Vec::new();
    };
    let mut parents: BTreeMap<String, (String, Option<String>)> = BTreeMap::new();
    for folder in index["folders"].as_array().into_iter().flatten() {
        let sid = folder["sid"].as_str().unwrap_or_default().to_owned();
        let name = folder["name"].as_str().unwrap_or_default().to_owned();
        let parent = folder["parent_sid"].as_str().map(str::to_owned);
        parents.insert(sid, (name, parent));
    }
    let resolve = |folder_sid: Option<&str>| -> Option<(Vec<String>, Vec<Sid>)> {
        let mut names = Vec::new();
        let mut sids = Vec::new();
        let mut cursor = folder_sid.map(str::to_owned);
        while let Some(sid) = cursor {
            let (name, parent) = parents.get(&sid)?.clone();
            sids.insert(0, Sid::parse(&sid).ok()?);
            names.insert(0, name);
            cursor = parent;
        }
        Some((names, sids))
    };
    let mut rows = Vec::new();
    for row in index["sections"].as_array().into_iter().flatten() {
        let Some(sid) = row["sid"].as_str().and_then(|sid| Sid::parse(sid).ok()) else {
            continue;
        };
        let name = row["name"].as_str().unwrap_or_default().to_owned();
        let Some((mut names, sids)) = resolve(row["folder_sid"].as_str()) else {
            continue;
        };
        names.push(name);
        if names.first().map(String::as_str) == Some(BRIEFING_FOLDER) {
            continue;
        }
        let tags: Vec<String> = row["tags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|t| t.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        rows.push(EthosRow {
            sid,
            path: names.join("/"),
            title: row["title"].as_str().unwrap_or_default().to_owned(),
            tags,
            folders: sids,
        });
    }
    rows
}

/// Map a refused delegated read to the caller-facing verdict: a mandate
/// verdict (perimeter, window, revocation) is a denial; anything else
/// is a bridge failure (never silently empty).
pub(crate) fn read_denied_op(
    op: &'static str,
) -> impl Fn(aithos_core::error::Error) -> GatewayError {
    move |e| match e {
        aithos_core::error::Error::InvalidMandate(reason) => GatewayError::MandateDenied {
            op: op.to_owned(),
            reason,
        },
        other => GatewayError::BridgeFailed(other.to_string()),
    }
}

/// Like [`write_denied`], but naming the exact refused tool (lot 4):
/// `ethos.create` / `ethos.edit` / `ethos.delete` refusals must cite
/// themselves precisely.
pub(crate) fn write_denied_op(
    op: &'static str,
) -> impl Fn(aithos_core::error::Error) -> GatewayError {
    move |e| match e {
        aithos_core::error::Error::InvalidMandate(reason)
        | aithos_core::error::Error::InvalidPath(reason) => GatewayError::MandateDenied {
            op: op.to_owned(),
            reason,
        },
        other => GatewayError::LogAppendRefused(other.to_string()),
    }
}

/// Map a refused delegated write to the caller-facing verdict: a mandate
/// verdict (perimeter, window, revocation) is a denial; anything else is
/// an append refusal.
pub(crate) fn write_denied(e: aithos_core::error::Error) -> GatewayError {
    match e {
        aithos_core::error::Error::InvalidMandate(reason) => GatewayError::MandateDenied {
            op: "ethos.write".to_owned(),
            reason,
        },
        other => GatewayError::LogAppendRefused(other.to_string()),
    }
}

pub(crate) fn constraints_bind_resource(constraints: &serde_json::Value, resource: &str) -> bool {
    constraints
        .get("purpose")
        .and_then(serde_json::Value::as_str)
        == Some(resource)
}

//! `aithos action` — log a connector action under a mandate chain (leaf
//! last). The gamma entry IS the authorization evidence (I5).

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, now_string};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    /// Certificate file(s), root first, leaf last.
    #[arg(long = "cert")]
    pub certs: Vec<String>,
    #[arg(long)]
    pub agent_seed_hex: String,
    pub connector: String,
    pub action: String,
    /// Free-form action arguments; only their hash enters the log.
    #[arg(long, default_value = "")]
    pub args: String,
    /// Budget profile citation (spec 04.11).
    #[arg(long)]
    pub budget_ref: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub tokens: Option<u64>,
    /// Provider attestation receipt as raw JSON (spec 04.11.1).
    #[arg(long)]
    pub receipt_json: Option<String>,
    /// Full argument object (JSON), sealed under the connector's audit
    /// key for a-posteriori audit (spec 07.9.3). Overrides --args.
    #[arg(long)]
    pub args_json: Option<String>,
    /// Obligation receipt(s) as raw JSON (spec 04.12), as printed by
    /// `approve`. Repeatable; rides in the entry's checks[].
    #[arg(long = "check-json")]
    pub checks_json: Vec<String>,
}

pub fn run(cmd_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        certs,
        agent_seed_hex,
        connector,
        action,
        args,
        budget_ref,
        model,
        tokens,
        receipt_json,
        args_json,
        checks_json,
    } = cmd_args;
    let mut bundle = bundle_at(&dir)?;
    let chain: Vec<aithos_core::mandate::Mandate> = certs
        .iter()
        .map(|c| -> Result<_, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&std::fs::read(c)?)?)
        })
        .collect::<Result<_, _>>()?;
    let agent = ed25519_dalek::SigningKey::from_bytes(
        &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
            .map_err(|_| "agent-seed-hex: 32 bytes")?,
    );
    let args_hash = format!(
        "sha256:{}",
        aithos_bundle::manifest::sha256_hex(args.as_bytes())
    );
    let mut budget = serde_json::Map::new();
    if let Some(b) = &budget_ref {
        budget.insert("budget_ref".into(), serde_json::json!(b));
    }
    if let Some(m) = &model {
        budget.insert("model".into(), serde_json::json!(m));
    }
    if let Some(t) = tokens {
        budget.insert("tokens".into(), serde_json::json!(t));
    }
    if let Some(r) = &receipt_json {
        budget.insert("receipt".into(), serde_json::from_str(r)?);
    }
    let checks: Vec<serde_json::Value> = checks_json
        .iter()
        .map(|c| serde_json::from_str(c))
        .collect::<Result<_, _>>()?;
    let entry = bundle.log_action_with_checks(
        &chain,
        &agent,
        &aithos_bundle::log::ActionSpec {
            connector: &connector,
            action: &action,
            args_hash: &args_hash,
            now: &now_string(),
            budget: (!budget.is_empty()).then_some(serde_json::Value::Object(budget)),
            sealed_args: args_json.as_deref().map(serde_json::from_str).transpose()?,
        },
        (!checks.is_empty()).then_some(serde_json::Value::Array(checks)),
        &mut OsEntropy,
    )?;
    println!("action logged: {}", entry.id);
    Ok(())
}

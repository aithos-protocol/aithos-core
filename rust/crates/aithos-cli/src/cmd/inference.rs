//! `aithos inference` — log one metered LLM call (spec 07.9.1): counters
//! only, never text.

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, now_string};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long = "cert")]
    pub certs: Vec<String>,
    #[arg(long)]
    pub agent_seed_hex: String,
    pub provider: String,
    pub model: String,
    #[arg(long)]
    pub tokens_in: u64,
    #[arg(long)]
    pub tokens_out: u64,
    #[arg(long)]
    pub budget_ref: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        certs,
        agent_seed_hex,
        provider,
        model,
        tokens_in,
        tokens_out,
        budget_ref,
    } = args;
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
    let entry = bundle.log_inference(
        &chain,
        &agent,
        &aithos_bundle::log::InferenceSpec {
            provider: &provider,
            model: &model,
            tokens_in,
            tokens_out,
            budget_ref: budget_ref.as_deref(),
            now: &now_string(),
        },
        &mut OsEntropy,
    )?;
    println!("inference logged: {}", entry.id);
    Ok(())
}

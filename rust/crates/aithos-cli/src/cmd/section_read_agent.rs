//! `aithos section-read-agent` — read a circle section AS an agent, gated
//! by its mandate (spec 04.5).

use aithos_core::path::Zone;

use super::common::bundle_at;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub cert: String,
    #[arg(long)]
    pub agent_seed_hex: String,
    #[arg(long)]
    pub at: String,
    pub path: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        cert,
        agent_seed_hex,
        at,
        path,
    } = args;
    let bundle = bundle_at(&dir)?;
    let m: aithos_core::mandate::Mandate = serde_json::from_slice(&std::fs::read(&cert)?)?;
    let agent = ed25519_dalek::SigningKey::from_bytes(
        &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
            .map_err(|_| "agent-seed-hex: 32 bytes")?,
    );
    let body = bundle.read_section_as_agent(&[m], &agent, Zone::Circle, &path, &at)?;
    println!("{body}");
    Ok(())
}

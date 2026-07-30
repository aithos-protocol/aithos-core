//! `aithos heartbeat` — publish an owner liveness beacon (spec 07.5).

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    #[arg(long, default_value_t = 1)]
    pub seq: u64,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir, seed_hex, seq } = args;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    bundle.log_heartbeat(&owner, seq, &now_string(), &mut OsEntropy)?;
    println!("beacon {seq} published");
    Ok(())
}

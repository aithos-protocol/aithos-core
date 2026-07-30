//! `aithos edition-publish` — publish a new edition (height+1), signed by root.

use super::common::{bundle_at, now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir, seed_hex } = args;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    bundle.publish(&owner, &now_string())?;
    println!("edition published");
    Ok(())
}

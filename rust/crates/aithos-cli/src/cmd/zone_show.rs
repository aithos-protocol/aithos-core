//! `aithos zone-show` — show a zone's display tree (owner-side for circle/self).

use aithos_core::path::Zone;

use super::common::{bundle_at, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    pub zone: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        zone,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let bundle = bundle_at(&dir)?;
    for path in bundle.zone_tree(Zone::parse(&zone)?, &owner)? {
        println!("{path}");
    }
    Ok(())
}

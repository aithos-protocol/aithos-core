//! `aithos folder-add` — create a folder (mkdir -p) in a zone of the bundle.

use aithos_bundle::entropy::OsEntropy;
use aithos_core::path::Zone;

use super::common::{bundle_at, owner_from, resolved_dir};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    pub zone: String,
    pub path: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        zone,
        path,
    } = args;
    let dir = resolved_dir(&dir)?;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    bundle.ensure_folder(Zone::parse(&zone)?, &path, &owner, &mut OsEntropy)?;
    println!("folder ready: {zone}/{path}");
    Ok(())
}

//! `aithos section-read` — read one section. Public needs NO key.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::FsStore;
use aithos_core::path::Zone;

use super::common::{bundle_at, owner_from};

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
    let zone = Zone::parse(&zone)?;
    let body = match (zone, seed_hex) {
        (Zone::Public, _) => Bundle::<FsStore>::public_read(&bundle_at(&dir)?.store, &path)?,
        (_, Some(seed)) => {
            let owner = owner_from(&seed)?;
            bundle_at(&dir)?.read_section(zone, &path, &owner)?
        }
        _ => {
            let owner = owner_from(&None)?;
            bundle_at(&dir)?.read_section(zone, &path, &owner)?
        }
    };
    println!("{body}");
    Ok(())
}

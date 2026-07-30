//! `aithos edition-merge` — merge another copy's competing edition (spec 02.6).

use aithos_bundle::Store;

use super::common::{bundle_at, now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    /// The other copy's bundle directory.
    #[arg(long)]
    pub other: String,
    #[arg(long)]
    pub seed_hex: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        other,
        seed_hex,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let mut mine = bundle_at(&dir)?;
    let theirs = bundle_at(&other)?;
    mine.edition_merge(&theirs, &owner, &now_string())?;
    let manifest: aithos_bundle::manifest::Manifest = serde_json::from_slice(
        &mine
            .store
            .get("manifest.json")?
            .ok_or("missing manifest after merge")?,
    )?;
    println!(
        "merge edition published (height {}, parents {} + {})",
        manifest.edition.height, manifest.merges[0], manifest.merges[1]
    );
    Ok(())
}

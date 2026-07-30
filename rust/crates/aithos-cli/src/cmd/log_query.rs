//! `aithos log-query` — owner search over the log (spec 07.8): every
//! present filter narrows.

use aithos_core::path::Zone;

use super::common::{bundle_at, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    #[arg(long)]
    pub kind: Option<String>,
    #[arg(long)]
    pub action: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub until: Option<String>,
    /// Display folder path in circle, e.g. projets/perso
    #[arg(long)]
    pub folder: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub mandate: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        kind,
        action,
        since,
        until,
        folder,
        tag,
        mandate,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let bundle = bundle_at(&dir)?;
    let filter = aithos_bundle::log::LogFilter {
        kind,
        action,
        since,
        until,
        zone_dir: folder.map(|f| (Zone::Circle, f)),
        tag,
        mandate,
    };
    for hit in bundle.log_query_owner(&owner, &filter)? {
        let e = &hit.entry;
        let target = hit
            .body
            .as_ref()
            .map(|b| b.target.clone())
            .or_else(|| e.target.clone())
            .unwrap_or_default();
        println!("{}  {}  {:<14}  {}", e.id, e.at, e.kind, target);
    }
    Ok(())
}

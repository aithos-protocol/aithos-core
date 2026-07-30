//! `aithos move` — move a circle folder under a new parent (spec 02.9):
//! move IS a rotation. (Module named `move_folder` — `move` is a keyword.)

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// The circle folder to move (display path).
    pub folder: String,
    /// The destination parent folder ("" = the zone root).
    #[arg(long)]
    pub under: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        folder,
        under,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    bundle.move_folder(&owner, &folder, &under, &mut OsEntropy)?;
    let dest = if under.is_empty() {
        "the zone root"
    } else {
        &under
    };
    println!("moved {folder} under {dest} — rotated, wrap posted, subtree re-encrypted");
    bundle.publish(&owner, &now_string())?;
    Ok(())
}

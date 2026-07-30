//! `aithos edition-diff` — root-descent diff between two editions (spec 02.10).

use super::common::resolved_dir;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub from: Option<u64>,
    #[arg(long)]
    pub to: Option<u64>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir, from, to } = args;
    let dir = resolved_dir(&dir)?;
    let manifest: aithos_bundle::manifest::Manifest =
        serde_json::from_slice(&std::fs::read(format!("{dir}/manifest.json"))?)?;
    let to = to.unwrap_or(manifest.edition.height);
    let from = from.unwrap_or_else(|| to.saturating_sub(1));
    let tree = |h: u64| -> Result<aithos_bundle::state::StateTree, Box<dyn std::error::Error>> {
        Ok(serde_json::from_slice(&std::fs::read(format!(
            "{dir}/manifests/tree-{h}.json"
        ))?)?)
    };
    let diff = aithos_bundle::state::tree_diff(&tree(from)?, &tree(to)?);
    if diff.is_empty() {
        println!("editions {from} → {to}: identical");
    } else {
        for (label, what) in diff {
            println!("{what:>7}  {label}");
        }
    }
    Ok(())
}

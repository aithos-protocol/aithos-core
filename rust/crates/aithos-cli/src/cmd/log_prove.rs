//! `aithos log-prove` — count proof or absence proof for a mandate
//! against the committed gamma counts root (spec 07.10).

use super::common::{bundle_at, resolved_dir};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    /// The mandate id whose counters are proven.
    #[arg(long, conflicts_with = "absent")]
    pub mandate: Option<String>,
    /// Prove this id was NEVER counted (sorted-adjacency absence).
    #[arg(long)]
    pub absent: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        mandate,
        absent,
    } = args;
    let dir = resolved_dir(&dir)?;
    let bundle = bundle_at(&dir)?;
    let manifest: aithos_bundle::manifest::Manifest =
        serde_json::from_slice(&std::fs::read(format!("{dir}/manifest.json"))?)?;
    if manifest.gamma_counts_root.is_empty() {
        return Err("no gamma counts root — publish an edition first".into());
    }
    let root: [u8; 32] = hex::decode(&manifest.gamma_counts_root)?
        .try_into()
        .map_err(|_| "malformed counts root")?;
    let tallies = aithos_core::gamma::counts_tally(&bundle.gamma_entries()?);
    match (mandate, absent) {
        (Some(id), None) => {
            let proof = aithos_core::gamma::prove_count(&tallies, &id)?;
            let (_, counters) = aithos_core::gamma::verify_count_proof(&proof, &root)?;
            println!("{}", serde_json::to_string_pretty(&proof)?);
            eprintln!(
                "count proof verifies against the committed counts root: {}",
                serde_json::to_string(&counters)?
            );
        }
        (None, Some(id)) => {
            let proof = aithos_core::gamma::prove_absence(&tallies, &id)?;
            aithos_core::gamma::verify_absence(&id, &proof, &root)?;
            println!("{}", serde_json::to_string_pretty(&proof)?);
            eprintln!("absence proof verifies: {id} was never counted");
        }
        _ => return Err("pass exactly one of --mandate or --absent".into()),
    }
    Ok(())
}

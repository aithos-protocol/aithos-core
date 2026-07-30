//! `aithos prove` — O(log n) inclusion proof of a section against the
//! signed Merkle root (spec 02.10), verified offline before printing.

use super::common::{bundle_at, resolved_dir};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    /// public | circle | self
    pub zone: String,
    /// Display path (public/circle) or the blob sid (self).
    pub path: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir, zone, path } = args;
    let dir = resolved_dir(&dir)?;
    let bundle = bundle_at(&dir)?;
    let manifest: aithos_bundle::manifest::Manifest =
        serde_json::from_slice(&std::fs::read(format!("{dir}/manifest.json"))?)?;
    let proof = match zone.as_str() {
        "self" => bundle.prove_self(&path)?,
        z => bundle.prove_section(aithos_core::path::Zone::parse(z)?, &path)?,
    };
    let pinned = manifest
        .roots
        .get(if zone == "self" {
            "self"
        } else {
            zone.as_str()
        })
        .ok_or("no root pinned — publish an edition first")?;
    let root: [u8; 32] = hex::decode(pinned)?
        .try_into()
        .map_err(|_| "malformed pinned root")?;
    aithos_core::merkle::verify_proof(&proof, &root)?;
    println!("{}", serde_json::to_string_pretty(&proof)?);
    eprintln!(
        "proof verifies against the signed {zone} root ({} steps)",
        proof.steps.len()
    );
    Ok(())
}

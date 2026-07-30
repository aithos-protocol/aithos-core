//! `aithos node-key` — DEV ONLY: derive a node key along a canonical
//! sid-path (spec 02.5).

#[derive(clap::Args)]
pub struct Args {
    /// Canonical path, e.g. /e/circle/d/<sid>/s/<sid>
    pub path: String,
    /// The zone-root DK as hex (32 bytes).
    #[arg(long)]
    pub zone_dk_hex: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { path, zone_dk_hex } = args;
    eprintln!("WARNING: node-key is a DEV/debug verb; never expose real zone keys.");
    let zone: [u8; 32] = hex::decode(zone_dk_hex)?
        .try_into()
        .map_err(|_| "zone-dk-hex: expected 32 bytes")?;
    let parsed = aithos_core::path::NodePath::parse(&path)?;
    println!(
        "{}",
        hex::encode(aithos_core::derive::node_key(&zone, &parsed))
    );
    Ok(())
}

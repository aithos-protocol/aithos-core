//! `aithos mandate-verify` — verify a mandate chain (one cert file) at
//! time T. No keys needed.

use aithos_core::did::DidDocument;

use super::common::{bundle_at, resolved_dir};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub cert: String,
    #[arg(long)]
    pub at: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir, cert, at } = args;
    let dir = resolved_dir(&dir)?;
    let bundle = bundle_at(&dir)?;
    let doc: DidDocument = serde_json::from_slice(&std::fs::read(format!("{dir}/did.json"))?)?;
    let m: aithos_core::mandate::Mandate = serde_json::from_slice(&std::fs::read(&cert)?)?;
    let _ = &bundle;
    aithos_core::mandate::verify_chain(&[m], &doc, &at)?;
    println!("mandate: OK at {at}");
    Ok(())
}

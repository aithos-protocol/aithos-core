//! `aithos log-verify` — verify the whole gamma chain and every entry
//! signature. No keys needed.

use super::common::bundle_at;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir } = args;
    bundle_at(&dir)?.gamma_verify()?;
    println!("gamma chain: OK");
    Ok(())
}

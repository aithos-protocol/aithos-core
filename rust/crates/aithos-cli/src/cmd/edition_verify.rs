//! `aithos edition-verify` — verify the whole edition chain and pinned files.

use super::common::bundle_at;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir } = args;
    bundle_at(&dir)?.verify()?;
    println!("edition chain: OK");
    Ok(())
}

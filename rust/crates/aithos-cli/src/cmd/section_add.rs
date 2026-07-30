//! `aithos section-add` — add a section to a zone of the bundle.

use aithos_bundle::bundle::SectionSpec;
use aithos_bundle::entropy::OsEntropy;
use aithos_core::path::Zone;

use super::common::{bundle_at, now_string, owner_from, split_path};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    pub zone: String,
    pub path: String,
    #[arg(long, default_value = "")]
    pub title: String,
    /// Comma-separated tags.
    #[arg(long, default_value = "")]
    pub tags: String,
    #[arg(long)]
    pub body: String,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        zone,
        path,
        title,
        tags,
        body,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    let (folder, name) = split_path(&path);
    let tags: Vec<String> = tags
        .split(',')
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect();
    bundle.section_add(
        &SectionSpec {
            zone: Zone::parse(&zone)?,
            folder_path: &folder,
            name: &name,
            title: &title,
            tags: &tags,
            body: &body,
            now: &now_string(),
        },
        &owner,
        &mut OsEntropy,
    )?;
    println!("section written: {zone}/{path}");
    Ok(())
}

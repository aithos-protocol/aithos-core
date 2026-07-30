//! `aithos log-show` — print the log's counting skeleton (what any
//! file-holder sees).

use super::common::bundle_at;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args { dir } = args;
    let bundle = bundle_at(&dir)?;
    for e in bundle.gamma_entries()? {
        let author = e.authorized_by.as_deref().unwrap_or("owner");
        let target = match (&e.target, &e.body_enc) {
            (Some(t), _) => t.as_str(),
            (None, Some(_)) => "(sealed)",
            (None, None) => "-",
        };
        println!(
            "{}  {}  {:<14}  {:<10}  {}",
            e.id, e.at, e.kind, author, target
        );
    }
    println!("head: {}", bundle.gamma_head()?);
    Ok(())
}

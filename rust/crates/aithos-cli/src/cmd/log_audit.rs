//! `aithos log-audit` — owner audit of sealed action args (spec 07.9.3).

use super::common::{bundle_at, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// Re-evaluate this certificate's action_params on the args.
    #[arg(long)]
    pub cert: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        cert,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let bundle = bundle_at(&dir)?;
    let mandate: Option<aithos_core::mandate::Mandate> = cert
        .map(|c| -> Result<_, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&std::fs::read(c)?)?)
        })
        .transpose()?;
    let mut audited = 0;
    for e in bundle.gamma_entries()? {
        if e.kind != "action" || e.body_enc.is_none() {
            continue;
        }
        let args = bundle.audit_action_args(&owner, &e)?;
        let action = e
            .payload
            .as_ref()
            .and_then(|p| p.get("action"))
            .and_then(|a| a.as_str())
            .unwrap_or_default()
            .to_owned();
        if let Some(m) = &mandate {
            aithos_core::constraints::check_action_params(&m.constraints, &action, &args)?;
        }
        println!("{}  {action}  args = {args}", e.id);
        audited += 1;
    }
    println!("audited: {audited} sealed action(s), all consistent");
    Ok(())
}

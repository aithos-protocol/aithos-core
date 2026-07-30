//! `aithos grant` — grant an agent a circle perimeter: mints the cert AND
//! delivers keys.

use aithos_bundle::entropy::OsEntropy;
use aithos_core::path::Zone;

use super::common::{bundle_at, now_secs, now_string, owner_from, ts};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// DEV: the agent's Ed25519 seed (its single keypair).
    #[arg(long)]
    pub agent_seed_hex: String,
    #[arg(long, default_value = "agent")]
    pub label: String,
    /// Display folder path in circle, e.g. projets/perso
    pub folder: String,
    #[arg(long)]
    pub tag: Option<String>,
    /// Perimeter verb (spec 04.2): read | edit | append | delete | write.
    #[arg(long, default_value = "read")]
    pub verb: String,
    #[arg(long, default_value_t = 7)]
    pub ttl_days: u32,
    #[arg(long, default_value_t = 0)]
    pub issue_depth: u32,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        agent_seed_hex,
        label,
        folder,
        tag,
        verb,
        ttl_days,
        issue_depth,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let agent = ed25519_dalek::SigningKey::from_bytes(
        &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
            .map_err(|_| "agent-seed-hex: 32 bytes")?,
    );
    let start = now_secs();
    let (nb, na) = (ts(start), ts(start + u64::from(ttl_days) * 86_400));
    eprintln!("window: not_before={nb} not_after={na}");
    let mut bundle = bundle_at(&dir)?;
    let spec = aithos_bundle::grants::GrantSpec {
        zone: Zone::Circle,
        verb: aithos_core::mandate::Verb::parse(&verb)?,
        dir: folder,
        tag,
    };
    let m = bundle.grant(
        &owner,
        &label,
        &agent.verifying_key(),
        &[spec],
        &nb,
        &na,
        issue_depth,
        &mut OsEntropy,
    )?;
    // Issuance is never silent (spec 07.4).
    bundle.log_owner_grant(&owner, &m.id, &now_string(), &mut OsEntropy)?;
    println!("granted; cert = certs/{}.json", m.id);
    Ok(())
}

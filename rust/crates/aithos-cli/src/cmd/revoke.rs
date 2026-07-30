//! `aithos revoke` — revoke a mandate (spec 06): one signed, anchored
//! gamma entry, optionally rotating the folder out of the revoked grantee.

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// The mandate id to revoke.
    pub mandate_id: String,
    #[arg(long, default_value = "revoked")]
    pub reason: String,
    /// Also rotate this circle folder out of the revoked grantee.
    #[arg(long)]
    pub rotate: Option<String>,
    /// The revoked grantee's Ed25519 seed (to compute its header kid).
    #[arg(long)]
    pub revoked_seed_hex: Option<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        mandate_id,
        reason,
        rotate,
        revoked_seed_hex,
    } = args;
    let owner = owner_from(&seed_hex)?;
    let mut bundle = bundle_at(&dir)?;
    let entry =
        bundle.log_revoke_owner(&owner, &mandate_id, &reason, &now_string(), &mut OsEntropy)?;
    println!("revoked; entry = {}", entry.id);
    if let Some(folder) = rotate {
        let seed = revoked_seed_hex
            .ok_or("--rotate needs --revoked-seed-hex to compute the revoked kid")?;
        let revoked = ed25519_dalek::SigningKey::from_bytes(
            &<[u8; 32]>::try_from(hex::decode(seed)?).map_err(|_| "revoked-seed-hex: 32 bytes")?,
        );
        let kid = aithos_core::wire::ed25519_pub_to_multibase(&revoked.verifying_key().to_bytes());
        bundle.rotate_folder(&owner, &folder, &kid, &mut OsEntropy)?;
        println!("rotated {folder} out of the revoked grantee");
    }
    bundle.publish(&owner, &now_string())?;
    Ok(())
}

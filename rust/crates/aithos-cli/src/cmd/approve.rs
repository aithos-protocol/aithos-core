//! `aithos approve` — sign an obligation receipt (spec 04.12): the
//! attestor's verdict, bound to one mandate+action+args.

use super::common::{now_string, owner_from};

#[derive(clap::Args)]
pub struct Args {
    /// The approver's device-held Ed25519 seed (hex, 32 bytes).
    #[arg(long, conflicts_with = "owner_seed_hex")]
    pub approver_seed_hex: Option<String>,
    /// Owner mode: derive the content key and sign a co_sign receipt.
    #[arg(long)]
    pub owner_seed_hex: Option<String>,
    /// Obligation id to discharge (defaults to co_sign in owner mode).
    #[arg(long)]
    pub obligation: Option<String>,
    /// The LEAF mandate id the entry will cite (its authorized_by).
    #[arg(long)]
    pub mandate: String,
    pub action: String,
    /// The exact action arguments the agent will log (same --args).
    #[arg(long, default_value = "")]
    pub args: String,
    #[arg(long, default_value = "approve")]
    pub verdict: String,
    /// What was shown on the device; hashed into presented_digest
    /// inside the signed payload (WYSIWYS).
    #[arg(long)]
    pub presented: Option<String>,
    /// Receipt instant (RFC 3339 Z); defaults to now.
    #[arg(long)]
    pub at: Option<String>,
    /// Print the attestor public key (multibase) and exit — pin it in
    /// --obligations-json at grant time.
    #[arg(long, default_value_t = false)]
    pub key_only: bool,
}

pub fn run(cmd_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    use ed25519_dalek::Signer;
    let Args {
        approver_seed_hex,
        owner_seed_hex,
        obligation,
        mandate,
        action,
        args,
        verdict,
        presented,
        at,
        key_only,
    } = cmd_args;
    let (sk, default_ob) = match (&approver_seed_hex, &owner_seed_hex) {
        (Some(seed), None) => (
            ed25519_dalek::SigningKey::from_bytes(
                &<[u8; 32]>::try_from(hex::decode(seed)?)
                    .map_err(|_| "approver-seed-hex: 32 bytes")?,
            ),
            None,
        ),
        (None, Some(seed)) => (
            owner_from(seed)?.content_sign.clone(),
            Some("co_sign".to_owned()),
        ),
        _ => return Err("exactly one of --approver-seed-hex / --owner-seed-hex".into()),
    };
    if key_only {
        println!(
            "{}",
            aithos_core::wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes())
        );
        return Ok(());
    }
    let obligation = obligation
        .or(default_ob)
        .ok_or("--obligation is required (unless owner co_sign mode)")?;
    let args_hash = format!(
        "sha256:{}",
        aithos_bundle::manifest::sha256_hex(args.as_bytes())
    );
    let at = at.unwrap_or_else(now_string);
    let mut payload = serde_json::json!({
        "obligation": obligation, "mandate_id": mandate, "action": action,
        "args_hash": args_hash, "verdict": verdict, "at": at,
    });
    if let Some(p) = &presented {
        payload["presented_digest"] = serde_json::json!(format!(
            "sha256:{}",
            aithos_bundle::manifest::sha256_hex(p.as_bytes())
        ));
    }
    let sig = hex::encode(
        sk.sign(&aithos_core::jcs::canonical_bytes(&payload)?)
            .to_bytes(),
    );
    let mut receipt = serde_json::json!({
        "obligation": obligation, "args_hash": args_hash,
        "verdict": verdict, "at": at, "sig": sig,
    });
    if let Some(d) = payload.get("presented_digest") {
        receipt["presented_digest"] = d.clone();
    }
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}

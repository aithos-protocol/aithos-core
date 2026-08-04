//! `aithos header-seal` — seal a node key into a header (spec 03). DEV
//! surface over test keys.

use super::common::getrandom;

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub node: String,
    #[arg(long)]
    pub subject_did: String,
    #[arg(long)]
    pub dk_hex: String,
    /// The subject's `owner_kex` (x25519 pubkey, hex) as published in its DID
    /// document. I3 (spec 03.1) DEFINES the owner line as the line sealed to
    /// this key, so it is REQUIRED: without it this surface could silently
    /// emit a header that every edition verifier rejects (spec 09.4). The
    /// owner line is built from it here — do not pass it as a `--recipient`.
    #[arg(long)]
    pub owner_kex_hex: String,
    /// Repeatable: label:kid:x25519_pub_hex — the GRANTEE lines only.
    #[arg(long = "recipient")]
    pub recipients: Vec<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::{Header, Recipient};
    let Args {
        node,
        subject_did,
        dk_hex,
        owner_kex_hex,
        recipients,
    } = args;
    let dk: [u8; 32] = hex::decode(dk_hex)?
        .try_into()
        .map_err(|_| "dk-hex: expected 32 bytes")?;
    let owner_bytes: [u8; 32] = hex::decode(owner_kex_hex)?
        .try_into()
        .map_err(|_| "owner-kex-hex: expected 32 bytes")?;
    // The owner line first, sealed to owner_kex and naming it (I3, spec 03.1).
    let owner_line = Recipient::owner(owner_bytes.into());
    let owner_kex = owner_line.pubkey;
    let mut recs = vec![owner_line];
    for spec in &recipients {
        let parts: Vec<&str> = spec.splitn(3, ':').collect();
        let [label, kid, pub_hex] = parts[..] else {
            return Err("recipient format: label:kid:x25519_pub_hex".into());
        };
        let pubkey: [u8; 32] = hex::decode(pub_hex)?
            .try_into()
            .map_err(|_| "recipient pubkey: expected 32 bytes")?;
        recs.push(Recipient {
            to: label.to_owned(),
            kid: kid.to_owned(),
            pubkey: pubkey.into(),
        });
    }
    // Randomness injected at the surface: one ephemeral + nonce per line.
    let mut ephemerals = Vec::new();
    let mut nonces = Vec::new();
    for _ in &recs {
        let mut e = [0u8; 32];
        getrandom(&mut e)?;
        let mut n = [0u8; 24];
        getrandom(&mut n)?;
        ephemerals.push(e);
        nonces.push(n);
    }
    let header = Header::build(
        &subject_did,
        &node,
        &dk,
        &owner_kex,
        &recs,
        &ephemerals,
        &nonces,
    )?;
    println!("{}", serde_json::to_string_pretty(&header)?);
    Ok(())
}

//! `aithos-core` — reference CLI (spec §09.1). Everything is local; no
//! command needs a network to be correct.

use aithos_core::did::DidDocument;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aithos-core", version, about = "Aithos Core reference CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate S, DID doc, empty bundle. (Scaffold: derives and prints the
    /// owner public keys; bundle writing lands with the bundle crate.)
    Init {
        /// DEV ONLY: fixed 32-byte seed as hex (deterministic, for vectors).
        /// Omit to generate a fresh random seed.
        #[arg(long)]
        seed_hex: Option<String>,
        /// DEV ONLY: fixed succession entropy as hex (deterministic).
        #[arg(long)]
        succession_seed_hex: Option<String>,
    },
    /// DEV ONLY: derive a node key along a canonical sid-path (spec 02.5).
    /// Proves determinism by hand: same path, same key — every time.
    NodeKey {
        /// Canonical path, e.g. /e/circle/d/<sid>/s/<sid>
        path: String,
        /// The zone-root DK as hex (32 bytes).
        #[arg(long)]
        zone_dk_hex: String,
    },
    /// Seal a node key into a header (spec 03). DEV surface over test keys.
    HeaderSeal {
        #[arg(long)]
        node: String,
        #[arg(long)]
        subject_did: String,
        #[arg(long)]
        dk_hex: String,
        /// Repeatable: label:kid:x25519_pub_hex — one MUST be labelled "owner".
        #[arg(long = "recipient")]
        recipients: Vec<String>,
    },
    /// Open one's line in a header JSON (from --file) and print the node key.
    HeaderOpen {
        #[arg(long)]
        file: String,
        #[arg(long)]
        subject_did: String,
        #[arg(long)]
        kid: String,
        #[arg(long)]
        sk_hex: String,
        #[arg(long, default_value_t = 1)]
        version: u64,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Init {
            seed_hex,
            succession_seed_hex,
        } => init(seed_hex, succession_seed_hex),
        Command::NodeKey { path, zone_dk_hex } => node_key_cmd(&path, &zone_dk_hex),
        Command::HeaderSeal {
            node,
            subject_did,
            dk_hex,
            recipients,
        } => header_seal_cmd(&node, &subject_did, &dk_hex, &recipients),
        Command::HeaderOpen {
            file,
            subject_did,
            kid,
            sk_hex,
            version,
        } => header_open_cmd(&file, &subject_did, &kid, &sk_hex, version),
    }
}

fn header_seal_cmd(
    node: &str,
    subject_did: &str,
    dk_hex: &str,
    recipients: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::{Header, Recipient};
    let dk: [u8; 32] = hex::decode(dk_hex)?
        .try_into()
        .map_err(|_| "dk-hex: expected 32 bytes")?;
    let mut recs = Vec::new();
    for spec in recipients {
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
    let header = Header::build(subject_did, node, &dk, &recs, &ephemerals, &nonces)?;
    println!("{}", serde_json::to_string_pretty(&header)?);
    Ok(())
}

fn header_open_cmd(
    file: &str,
    subject_did: &str,
    kid: &str,
    sk_hex: &str,
    version: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::Header;
    let header: Header = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    header.validate()?;
    let sk: [u8; 32] = hex::decode(sk_hex)?
        .try_into()
        .map_err(|_| "sk-hex: expected 32 bytes")?;
    let dk = header.open(subject_did, version, kid, &sk.into())?;
    println!("{}", hex::encode(dk));
    Ok(())
}

fn node_key_cmd(path: &str, zone_dk_hex: &str) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("WARNING: node-key is a DEV/debug verb; never expose real zone keys.");
    let zone: [u8; 32] = hex::decode(zone_dk_hex)?
        .try_into()
        .map_err(|_| "zone-dk-hex: expected 32 bytes")?;
    let parsed = aithos_core::path::NodePath::parse(path)?;
    println!(
        "{}",
        hex::encode(aithos_core::derive::node_key(&zone, &parsed))
    );
    Ok(())
}

fn seed32(
    hex_or_random: Option<String>,
    what: &str,
) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    match hex_or_random {
        Some(h) => {
            eprintln!("WARNING: --{what} is for tests/vectors only.");
            Ok(hex::decode(h)?
                .try_into()
                .map_err(|_| format!("{what}: expected 32 bytes"))?)
        }
        None => {
            // OS randomness is injected here, at the surface — never inside core.
            let mut bytes = [0u8; 32];
            getrandom(&mut bytes)?;
            Ok(bytes)
        }
    }
}

fn init(
    seed_hex: Option<String>,
    succession_seed_hex: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let seed = MasterSeed::from_bytes(seed32(seed_hex, "seed-hex")?);
    let succession_entropy = seed32(succession_seed_hex, "succession-seed-hex")?;
    let keys = OwnerKeys::genesis(&seed);
    let succession = succession_from_entropy(succession_entropy);
    let doc = DidDocument::build(
        &keys,
        &succession.verifying_key(),
        vec!["file://local".to_owned()],
        "gamma/gamma.jsonl".to_owned(),
    )?;
    doc.verify()?;
    let root_pub = keys.root_sign.verifying_key().to_bytes();
    let out = serde_json::json!({
        "did": doc.id,
        "root_sign_pub": hex::encode(root_pub),
        "content_sign_pub": hex::encode(keys.content_sign.verifying_key().to_bytes()),
        "owner_kex_pub": hex::encode(keys.owner_kex_pub().to_bytes()),
        "succession_pub": hex::encode(succession.verifying_key().to_bytes()),
        "succession_secret_hex": hex::encode(succession_entropy),
        "did_document": doc,
    });
    eprintln!("STORE succession_secret_hex COLD (paper/HSM) — it is shown ONCE and never derivable again.");
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Read;
    File::open("/dev/urandom")?.read_exact(buf)
}

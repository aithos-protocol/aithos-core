//! `aithos-core` — reference CLI (spec §09.1). Everything is local; no
//! command needs a network to be correct.

use aithos_core::keys::{MasterSeed, OwnerKeys};
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
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Init { seed_hex } => init(seed_hex),
    }
}

fn init(seed_hex: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let seed = match seed_hex {
        Some(h) => {
            eprintln!("WARNING: --seed-hex is for tests/vectors only.");
            MasterSeed::from_slice(&hex::decode(h)?)?
        }
        None => {
            // OS randomness is injected here, at the surface — never inside core.
            let mut bytes = [0u8; 32];
            getrandom(&mut bytes)?;
            MasterSeed::from_bytes(bytes)
        }
    };
    let keys = OwnerKeys::genesis(&seed);
    let root_pub = keys.root_sign.verifying_key().to_bytes();
    let out = serde_json::json!({
        "did": aithos_core::wire::did_aithos(&root_pub),
        "root_sign_pub": hex::encode(root_pub),
        "content_sign_pub": hex::encode(keys.content_sign.verifying_key().to_bytes()),
        "owner_kex_pub": hex::encode(keys.owner_kex_pub().to_bytes()),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Read;
    File::open("/dev/urandom")?.read_exact(buf)
}

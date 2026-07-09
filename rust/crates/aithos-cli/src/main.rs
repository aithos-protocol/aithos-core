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
    let out = serde_json::json!({
        "root_sign_pub": hex::encode(keys.root_sign.verifying_key().to_bytes()),
        "sphere_public_pub": hex::encode(keys.sphere_public.verifying_key().to_bytes()),
        "sphere_circle_pub": hex::encode(keys.sphere_circle.verifying_key().to_bytes()),
        "sphere_self_pub": hex::encode(keys.sphere_self.verifying_key().to_bytes()),
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

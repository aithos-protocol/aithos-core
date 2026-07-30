//! `aithos init` — create an Ethos, its DID and real random keys under
//! managed custody.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::entropy::OsEntropy;
use aithos_bundle::FsStore;
use aithos_core::did::DidDocument;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};

use super::common::{now_string, seed32};
use crate::custody;

#[derive(clap::Args)]
pub struct Args {
    /// DEV ONLY: fixed 32-byte seed as hex (deterministic, for vectors).
    /// Omit to generate a fresh random seed.
    #[arg(long)]
    pub seed_hex: Option<String>,
    /// DEV ONLY: fixed succession entropy as hex (deterministic).
    #[arg(long)]
    pub succession_seed_hex: Option<String>,
    /// Also create a bundle (spec 02.3) in this directory.
    #[arg(long)]
    pub dir: Option<String>,
    /// keychain (macOS default), file, or vault.
    #[arg(long, value_parser = ["keychain", "file", "vault"])]
    pub key_store: Option<String>,
    /// HashiCorp Vault base URL (or VAULT_ADDR).
    #[arg(long)]
    pub vault_address: Option<String>,
    /// Vault KV v2 mount.
    #[arg(long, default_value = "secret")]
    pub vault_mount: String,
    /// Vault KV v2 secret path (default: aithos/ethos/<profile>).
    #[arg(long)]
    pub vault_path: Option<String>,
    /// Environment variable carrying the Vault token.
    #[arg(long, default_value = "VAULT_TOKEN")]
    pub vault_token_env: String,
}

pub fn run(
    args: Args,
    home: &std::path::Path,
    profile_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        seed_hex,
        succession_seed_hex,
        dir,
        key_store,
        vault_address,
        vault_mount,
        vault_path,
        vault_token_env,
    } = args;
    let managed = seed_hex.is_none();
    if managed && custody::profile_path(home, profile_name).exists() {
        return Err(format!("profile '{profile_name}' already exists").into());
    }
    let seed_bytes = seed32(seed_hex, "seed-hex")?;
    let seed = MasterSeed::from_bytes(seed_bytes);
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
    let managed_dir = home.join("ethos").join(profile_name).join("bundle");
    let dir = dir
        .map(std::path::PathBuf::from)
        .or_else(|| managed.then_some(managed_dir));
    if let Some(dir) = &dir {
        if managed && dir.exists() {
            return Err(format!("bundle destination already exists: {}", dir.display()).into());
        }
        Bundle::init(
            FsStore::new(dir),
            &keys,
            &succession.verifying_key(),
            &mut OsEntropy,
            &now_string(),
        )?;
        eprintln!("bundle initialised in {}", dir.display());
    }
    let root_pub = keys.root_sign.verifying_key().to_bytes();
    if managed {
        let bundle_dir = dir.ok_or("managed init requires a bundle directory")?;
        let store = match key_store.as_deref() {
            None => custody::default_key_store(home, profile_name, &doc.id),
            Some("file") => custody::KeyStoreConfig::File {
                path: home.join("keys").join(format!("{profile_name}.json")),
            },
            Some("keychain") => custody::KeyStoreConfig::Keychain {
                service: "fr.aithos.cli".to_owned(),
                account: format!("{profile_name}:{}", doc.id),
            },
            Some("vault") => {
                let address = vault_address
                    .or_else(|| std::env::var("VAULT_ADDR").ok())
                    .ok_or("Vault custody needs --vault-address or VAULT_ADDR")?;
                custody::KeyStoreConfig::VaultKv2 {
                    address,
                    mount: vault_mount,
                    path: vault_path.unwrap_or_else(|| format!("aithos/ethos/{profile_name}")),
                    token_env: vault_token_env,
                }
            }
            Some(_) => return Err("unsupported key store".into()),
        };
        let material = custody::KeyMaterial {
            master_seed_hex: hex::encode(seed_bytes),
            succession_seed_hex: hex::encode(succession_entropy),
        };
        let profile = custody::new_profile(
            profile_name.to_owned(),
            doc.id.clone(),
            bundle_dir.clone(),
            store,
        );
        custody::save_profile(home, &profile)?;
        if let Err(error) = custody::save_keys(&profile.key_store, &material) {
            let _ = std::fs::remove_file(custody::profile_path(home, profile_name));
            let _ = std::fs::remove_dir_all(&bundle_dir);
            return Err(error);
        }
        println!("did: {}", doc.id);
        println!("profile: {profile_name}");
        println!("bundle: {}", bundle_dir.display());
        println!("key_store: {}", profile.key_store.label());
        eprintln!("owner and succession seeds were stored by the selected custody backend; no secret was printed");
        return Ok(());
    }
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

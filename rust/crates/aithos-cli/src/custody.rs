use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use zeroize::Zeroize;

const PROFILE_VERSION: u32 = 1;
const KEYCHAIN_SERVICE: &str = "fr.aithos.cli";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum KeyStoreConfig {
    File {
        path: PathBuf,
    },
    Keychain {
        service: String,
        account: String,
    },
    VaultKv2 {
        address: String,
        mount: String,
        path: String,
        token_env: String,
    },
}

impl KeyStoreConfig {
    pub fn label(&self) -> &'static str {
        match self {
            Self::File { .. } => "file",
            Self::Keychain { .. } => "keychain",
            Self::VaultKv2 { .. } => "vault-kv2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub name: String,
    pub did: String,
    pub bundle_dir: PathBuf,
    pub key_store: KeyStoreConfig,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyMaterial {
    pub master_seed_hex: String,
    pub succession_seed_hex: String,
}

impl Drop for KeyMaterial {
    fn drop(&mut self) {
        self.master_seed_hex.zeroize();
        self.succession_seed_hex.zeroize();
    }
}

pub fn validate_profile_name(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("profile name must be 1-64 ASCII letters, digits, '-' or '_'".into());
    }
    Ok(())
}

pub fn default_home() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = std::env::var_os("AITHOS_HOME") {
        return Ok(PathBuf::from(path));
    }
    let user_home = std::env::var_os("HOME").ok_or("HOME is unset; pass --home")?;
    #[cfg(target_os = "macos")]
    return Ok(PathBuf::from(user_home)
        .join("Library")
        .join("Application Support")
        .join("Aithos"));
    #[cfg(not(target_os = "macos"))]
    return Ok(PathBuf::from(user_home)
        .join(".local")
        .join("share")
        .join("aithos"));
}

pub fn profile_path(home: &Path, name: &str) -> PathBuf {
    home.join("profiles").join(format!("{name}.json"))
}

pub fn load_profile(home: &Path, name: &str) -> Result<Profile, Box<dyn std::error::Error>> {
    validate_profile_name(name)?;
    let path = profile_path(home, name);
    let profile: Profile = serde_json::from_slice(&std::fs::read(&path).map_err(|e| {
        format!(
            "profile '{}' is unavailable at {}: {e}; run `aithos init` first",
            name,
            path.display()
        )
    })?)?;
    if profile.version != PROFILE_VERSION || profile.name != name {
        return Err("profile version or name mismatch".into());
    }
    Ok(profile)
}

pub fn save_profile(home: &Path, profile: &Profile) -> Result<(), Box<dyn std::error::Error>> {
    validate_profile_name(&profile.name)?;
    let path = profile_path(home, &profile.name);
    if path.exists() {
        return Err(format!("profile '{}' already exists", profile.name).into());
    }
    write_private(&path, &serde_json::to_vec_pretty(profile)?)
}

pub fn new_profile(
    name: String,
    did: String,
    bundle_dir: PathBuf,
    key_store: KeyStoreConfig,
) -> Profile {
    Profile {
        version: PROFILE_VERSION,
        name,
        did,
        bundle_dir,
        key_store,
    }
}

pub fn default_key_store(home: &Path, profile: &str, did: &str) -> KeyStoreConfig {
    #[cfg(target_os = "macos")]
    {
        let _ = home;
        KeyStoreConfig::Keychain {
            service: KEYCHAIN_SERVICE.to_owned(),
            account: format!("{profile}:{did}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = did;
        KeyStoreConfig::File {
            path: home.join("keys").join(format!("{profile}.json")),
        }
    }
}

pub fn save_keys(
    config: &KeyStoreConfig,
    material: &KeyMaterial,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(material)?;
    match config {
        KeyStoreConfig::File { path } => {
            if path.exists() {
                return Err(format!("key file already exists: {}", path.display()).into());
            }
            write_private(path, &bytes)
        }
        KeyStoreConfig::Keychain { service, account } => {
            #[cfg(target_os = "macos")]
            {
                security_framework::passwords::set_generic_password(service, account, &bytes)?;
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (service, account, bytes);
                Err("the keychain backend is only available on macOS".into())
            }
        }
        KeyStoreConfig::VaultKv2 {
            address,
            mount,
            path,
            token_env,
        } => vault_write(address, mount, path, token_env, &bytes),
    }
}

pub fn load_keys(config: &KeyStoreConfig) -> Result<KeyMaterial, Box<dyn std::error::Error>> {
    let bytes = match config {
        KeyStoreConfig::File { path } => std::fs::read(path)
            .map_err(|e| format!("key file is unavailable at {}: {e}", path.display()))?,
        KeyStoreConfig::Keychain { service, account } => {
            #[cfg(target_os = "macos")]
            {
                security_framework::passwords::get_generic_password(service, account)?
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (service, account);
                return Err("the keychain backend is only available on macOS".into());
            }
        }
        KeyStoreConfig::VaultKv2 {
            address,
            mount,
            path,
            token_env,
        } => vault_read(address, mount, path, token_env)?,
    };
    let material: KeyMaterial = serde_json::from_slice(&bytes)
        .map_err(|_| "key store returned malformed Aithos key material")?;
    validate_seed(&material.master_seed_hex, "master seed")?;
    validate_seed(&material.succession_seed_hex, "succession seed")?;
    Ok(material)
}

fn validate_seed(value: &str, label: &str) -> Result<(), Box<dyn std::error::Error>> {
    if hex::decode(value)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .is_none()
    {
        return Err(format!("{label} in key store is not 32 bytes").into());
    }
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn vault_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into()
}

fn vault_url(address: &str, mount: &str, path: &str) -> Result<String, Box<dyn std::error::Error>> {
    if address.trim().is_empty()
        || mount.is_empty()
        || path.is_empty()
        || mount.contains('/')
        || path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
    {
        return Err("invalid Vault address, mount or path".into());
    }
    Ok(format!(
        "{}/v1/{}/data/{}",
        address.trim_end_matches('/'),
        mount,
        path
    ))
}

fn vault_token(token_env: &str) -> Result<zeroize::Zeroizing<String>, Box<dyn std::error::Error>> {
    let token = std::env::var(token_env)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            format!("Vault token environment variable `{token_env}` is unset or empty")
        })?;
    Ok(zeroize::Zeroizing::new(token))
}

fn vault_write(
    address: &str,
    mount: &str,
    path: &str,
    token_env: &str,
    material: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let url = vault_url(address, mount, path)?;
    let token = vault_token(token_env)?;
    let material: serde_json::Value = serde_json::from_slice(material)?;
    // KV v2 CAS=0 means create-only: `init` never overwrites an existing
    // enterprise secret at the selected path.
    let body = serde_json::to_vec(&serde_json::json!({
        "options": { "cas": 0 },
        "data": material
    }))?;
    let response = vault_agent()
        .post(&url)
        .header("X-Vault-Token", token.as_str())
        .header("Content-Type", "application/json")
        .send(&body)
        .map_err(|_| "Vault write transport failed")?;
    if !response.status().is_success() {
        return Err(format!("Vault write refused with status {}", response.status()).into());
    }
    Ok(())
}

fn vault_read(
    address: &str,
    mount: &str,
    path: &str,
    token_env: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let url = vault_url(address, mount, path)?;
    let token = vault_token(token_env)?;
    let mut response = vault_agent()
        .get(&url)
        .header("X-Vault-Token", token.as_str())
        .call()
        .map_err(|_| "Vault read transport failed")?;
    if !response.status().is_success() {
        return Err(format!("Vault read refused with status {}", response.status()).into());
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(64 * 1024)
        .read_to_vec()
        .map_err(|_| "Vault response body is unreadable")?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Vault response is not JSON")?;
    let data = value
        .pointer("/data/data")
        .cloned()
        .ok_or("Vault response is not a KV v2 secret")?;
    serde_json::to_vec(&data).map_err(Into::into)
}

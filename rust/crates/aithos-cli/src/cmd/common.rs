//! Shared surface helpers: profile resolution, custody-backed owner keys,
//! bundle opening, UTC timestamps and OS entropy. Moved verbatim from
//! `main.rs` at lot SPL-5 — no behaviour change.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::FsStore;
use aithos_core::keys::{MasterSeed, OwnerKeys};
use std::sync::OnceLock;

use crate::custody;

pub static RUNTIME_PROFILE: OnceLock<(std::path::PathBuf, String)> = OnceLock::new();

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`): lexicographic order ==
/// chronological order (the verifier compares time strings, §04.5) and the
/// gamma layer parses it strictly (§07.1). civil_from_days per Hinnant.
pub fn ts(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

pub fn now_string() -> String {
    ts(now_secs())
}

pub fn owner_from_hex(seed_hex: &str) -> Result<OwnerKeys, Box<dyn std::error::Error>> {
    let seed = MasterSeed::from_slice(&hex::decode(seed_hex)?)?;
    Ok(OwnerKeys::genesis(&seed))
}

pub trait SeedInput {
    fn seed_value(&self) -> Option<&str>;
}

impl SeedInput for String {
    fn seed_value(&self) -> Option<&str> {
        Some(self)
    }
}

impl SeedInput for Option<String> {
    fn seed_value(&self) -> Option<&str> {
        self.as_deref()
    }
}

pub fn owner_from(input: &impl SeedInput) -> Result<OwnerKeys, Box<dyn std::error::Error>> {
    if let Some(seed) = input.seed_value().filter(|seed| !seed.is_empty()) {
        eprintln!("WARNING: --seed-hex on the command line is DEV ONLY.");
        return owner_from_hex(seed);
    }
    let (home, profile_name) = RUNTIME_PROFILE
        .get()
        .ok_or("CLI profile is not initialised")?;
    let profile = custody::load_profile(home, profile_name)?;
    let material = custody::load_keys(&profile.key_store)?;
    owner_from_hex(&material.master_seed_hex)
}

pub trait DirInput {
    fn dir_value(&self) -> Option<&str>;
}

impl DirInput for String {
    fn dir_value(&self) -> Option<&str> {
        Some(self)
    }
}

impl DirInput for Option<String> {
    fn dir_value(&self) -> Option<&str> {
        self.as_deref()
    }
}

pub fn resolved_dir(input: &impl DirInput) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(dir) = input.dir_value().filter(|dir| !dir.is_empty()) {
        return Ok(dir.to_owned());
    }
    let (home, profile_name) = RUNTIME_PROFILE
        .get()
        .ok_or("CLI profile is not initialised")?;
    Ok(custody::load_profile(home, profile_name)?
        .bundle_dir
        .to_string_lossy()
        .into_owned())
}

pub fn bundle_at(dir: &impl DirInput) -> Result<Bundle<FsStore>, Box<dyn std::error::Error>> {
    Ok(Bundle::open(FsStore::new(resolved_dir(dir)?))?)
}

pub fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder.to_owned(), name.to_owned()),
        None => (String::new(), path.to_owned()),
    }
}

pub fn seed32(
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

pub fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Read;
    File::open("/dev/urandom")?.read_exact(buf)
}

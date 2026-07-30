//! `aithos status` — show the active profile, custody backend and
//! verification status.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::FsStore;

use super::common::owner_from_hex;
use crate::custody;

pub fn run(home: &std::path::Path, profile_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let profile = custody::load_profile(home, profile_name)?;
    let material = custody::load_keys(&profile.key_store)?;
    let owner = owner_from_hex(&material.master_seed_hex)?;
    let did = aithos_core::wire::did_aithos(&owner.root_sign.verifying_key().to_bytes());
    if did != profile.did {
        return Err("custody key does not match the profile DID".into());
    }
    let bundle = Bundle::open(FsStore::new(&profile.bundle_dir))?;
    bundle.verify()?;
    bundle.gamma_verify()?;
    println!("profile: {}", profile.name);
    println!("did: {}", profile.did);
    println!("bundle: {}", profile.bundle_dir.display());
    println!("key_store: {}", profile.key_store.label());
    println!("custody: OK");
    println!("edition_chain: OK");
    println!("gamma_chain: OK");
    Ok(())
}

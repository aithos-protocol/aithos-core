//! Lot SPL-6 — l'artefact WASM de cérémonie servi par la gateway est un
//! build VÉRIFIÉ, plus un binaire commité sur parole : les octets compilés
//! dans le binaire (`include_bytes!`, `proxy_mcp.rs`) doivent correspondre
//! aux digests pinnés dans `assets/ceremony/wasm-bundle-digest.json`.
//!
//! L'autre moitié du contrat — le pin correspond bien à un build du crate
//! `aithos-wasm` courant — est tenue par `scripts/wasm-bundle.sh check`
//! (job CI `wasm-bundle`), qui reconstruit depuis la source sous la recette
//! gravée dans le pin. Ensemble : source ↔ pin ↔ artefact servi.

use sha2::{Digest as _, Sha256};

fn pinned(manifest: &serde_json::Value, name: &str) -> String {
    manifest["artifacts"][name]
        .as_str()
        .unwrap_or_else(|| panic!("wasm-bundle-digest.json: digest manquant pour {name}"))
        .to_owned()
}

#[test]
fn committed_ceremony_bundle_matches_the_pinned_digests() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../assets/ceremony/wasm-bundle-digest.json"))
            .expect("wasm-bundle-digest.json est un JSON valide");

    for (name, bytes) in [
        (
            "aithos_wasm.js",
            include_bytes!("../assets/ceremony/aithos_wasm.js").as_slice(),
        ),
        (
            "aithos_wasm_bg.wasm",
            include_bytes!("../assets/ceremony/aithos_wasm_bg.wasm").as_slice(),
        ),
    ] {
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        assert_eq!(
            pinned(&manifest, name),
            digest,
            "{name} diverge du pin — `scripts/wasm-bundle.sh regen` puis committer assets + pin ensemble"
        );
    }
}

#[test]
fn the_pin_records_a_complete_reproducible_recipe() {
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../assets/ceremony/wasm-bundle-digest.json")).unwrap();
    for key in ["rustc", "wasm_bindgen_cli", "target", "profile", "build"] {
        assert!(
            manifest["recipe"][key]
                .as_str()
                .is_some_and(|v| !v.is_empty()),
            "recette du pin incomplète : `{key}` manquant — le check CI serait invérifiable"
        );
    }
    assert_eq!(manifest["recipe"]["target"], "wasm32-unknown-unknown");
}

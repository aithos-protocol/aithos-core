//! AID-001 / AID-002 surface replay — audit `docs/audits/features/a-identity.md`.
//!
//! The Core verdict is proved in `aithos-core/tests/a2_did.rs`. What this file
//! proves is that the SURFACES which consume a `did.json` reach the same
//! verdict instead of running a permissive parser of their own:
//!
//! * `Bundle::open` and `Bundle::verify` — the offline reading path;
//! * `aithos_core::mandate::verify_chain` — the exact call the public WASM
//!   export `verify_chain` delegates to (`aithos-wasm/src/lib.rs`), so the
//!   browser surface inherits the same rejection without a wasm toolchain.
//!
//! Every defective document below is CORRECTLY re-signed under its own root
//! key: only the semantic control under test can explain a rejection.

use aithos_bundle::bundle::Bundle;
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::{MemStore, Store};
use aithos_core::did::DidDocument;
use aithos_core::error::Error;
use aithos_core::jcs;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{Mandate, MandateSpec, PerimeterEntry, Verb, MANDATE_VERSION_DRAFT1};
use aithos_core::path::Zone;
use ed25519_dalek::{Signer, SigningKey};

fn owner() -> OwnerKeys {
    OwnerKeys::genesis(&MasterSeed::from_bytes([0x21; 32]))
}

/// One named way to make a DID document semantically invalid.
type DidDefect = (&'static str, Box<dyn Fn(&mut DidDocument)>);

fn resign(doc: &mut DidDocument, root: &SigningKey) {
    doc.signature.value = String::new();
    let bytes = jcs::canonical_bytes(&*doc).unwrap();
    doc.signature.value = hex::encode(root.sign(&bytes).to_bytes());
}

/// A published bundle plus the owner that built it.
fn published_bundle() -> (MemStore, OwnerKeys) {
    let owner = owner();
    let succession = succession_from_entropy([0x22; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-29T09:00:00Z",
    )
    .expect("bundle initializes");
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-29T09:01:00Z"))
        .expect("bundle publishes");
    (bundle.store, owner)
}

/// Every defect a signed-but-malformed `did.json` can carry, as raw wire
/// bytes. `None` for the re-signing key means the wire text is injected
/// directly (unknown members cannot survive a typed round-trip by design).
fn defective_did_json(doc: &DidDocument, root: &SigningKey) -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();

    let mutations: Vec<DidDefect> = vec![
        (
            "malformed content key",
            Box::new(|d: &mut DidDocument| d.keys.content = "not-a-key".to_owned()),
        ),
        (
            "kex key in the Ed25519 codec",
            Box::new(|d: &mut DidDocument| {
                let bytes = aithos_core::wire::multibase_to_x25519_pub(&d.keys.kex).unwrap();
                d.keys.kex = aithos_core::wire::ed25519_pub_to_multibase(&bytes);
            }),
        ),
        (
            "malformed succession key",
            Box::new(|d: &mut DidDocument| d.keys.succession = "z6Mk".to_owned()),
        ),
        (
            "unsupported version",
            Box::new(|d: &mut DidDocument| d.version = "9.9.9".to_owned()),
        ),
        (
            "unsupported signature algorithm",
            Box::new(|d: &mut DidDocument| d.signature.alg = "secp256k1".to_owned()),
        ),
        (
            "signature fragment other than #root",
            Box::new(|d: &mut DidDocument| d.signature.key = "#content".to_owned()),
        ),
    ];
    for (label, mutate) in mutations {
        let mut defective = doc.clone();
        mutate(&mut defective);
        resign(&mut defective, root);
        out.push((label, jcs::canonical_bytes(&defective).unwrap()));
    }

    let text = jcs::canonicalize(doc).unwrap();
    for (label, needle, replacement) in [
        (
            "unknown top-level member",
            r#"{"aithos-did-core""#,
            r#"{"aithos-extra":"x","aithos-did-core""#,
        ),
        (
            "unknown keys member",
            r#""keys":{"#,
            r#""keys":{"extra":"x","#,
        ),
        (
            "unknown signature member",
            r#""signature":{"#,
            r#""signature":{"extra":"x","#,
        ),
    ] {
        let wire = text.replacen(needle, replacement, 1);
        assert_ne!(wire, text, "{label} injection must apply");
        out.push((label, wire.into_bytes()));
    }
    out
}

#[test]
fn aid001_bundle_open_refuses_every_signed_but_malformed_did_json() {
    let (store, owner) = published_bundle();
    let pristine = store.get("did.json").unwrap().expect("did.json exists");
    let doc: DidDocument = serde_json::from_slice(&pristine).unwrap();

    // Control: the untouched bundle opens and verifies.
    let bundle = Bundle::open(store.clone()).expect("pristine bundle opens");
    bundle.verify().expect("pristine bundle verifies");

    for (label, bytes) in defective_did_json(&doc, &owner.root_sign) {
        let mut poisoned = store.clone();
        poisoned.put("did.json", &bytes).unwrap();
        assert!(
            Bundle::open(poisoned.clone()).is_err(),
            "Bundle::open must refuse a did.json with a {label}"
        );
        // Offline verification reads the same object through its own path and
        // must reach the same verdict — an already-open bundle is no bypass.
        let mut opened = Bundle::open(store.clone()).expect("pristine bundle opens");
        opened.store = poisoned;
        assert!(
            opened.verify().is_err(),
            "Bundle::verify must refuse a did.json with a {label}"
        );
    }
}

/// A well-formed root mandate for `owner`, so `verify_chain` gets past its
/// form checks and actually reaches the DID document verdict.
fn root_mandate(owner: &OwnerKeys, subject: &str) -> Mandate {
    let agent = SigningKey::from_bytes(&[0x33; 32]);
    Mandate::build_root_with_version(
        &owner.root_sign,
        MANDATE_VERSION_DRAFT1,
        &MandateSpec {
            id: "mandate_0000000000000000000000001A".to_owned(),
            subject: subject.to_owned(),
            constraints: MandateSpec::no_constraints(),
            grantee_id: "urn:aithos:agent:agent".to_owned(),
            grantee_label: "agent".to_owned(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::Ethos {
                verb: Verb::Read,
                zone: Zone::Circle,
                dir: vec![],
                tag: None,
            }],
            not_before: "2026-07-29T00:00:00Z".to_owned(),
            not_after: "2027-07-29T00:00:00Z".to_owned(),
            issued_at: "2026-07-29T00:00:00Z".to_owned(),
            nonce: "000102030405060708090a0b0c0d0e0f".to_owned(),
        },
    )
    .expect("root mandate builds")
}

#[test]
fn aid001_wasm_chain_surface_inherits_the_strict_verdict() {
    // `aithos_core::mandate::verify_chain` is exactly what the public WASM
    // export delegates to (`aithos-wasm/src/lib.rs`). A signed-but-malformed
    // DID document must sink the chain there too, with the SAME verdict.
    let (store, owner) = published_bundle();
    let pristine = store.get("did.json").unwrap().expect("did.json exists");
    let doc: DidDocument = serde_json::from_slice(&pristine).unwrap();
    let chain = [root_mandate(&owner, &doc.id)];
    let at = "2026-07-29T09:02:00Z";

    // Control: with the pristine document the chain verifies end to end.
    aithos_core::mandate::verify_chain(&chain, &doc, at).expect("pristine chain verifies");

    for (label, bytes) in defective_did_json(&doc, &owner.root_sign) {
        match serde_json::from_slice::<DidDocument>(&bytes) {
            // Unknown members: the closed schema refuses them at the door,
            // exactly like the WASM export's own `from_str` does.
            Err(_) => continue,
            Ok(defective) => assert!(
                matches!(
                    aithos_core::mandate::verify_chain(&chain, &defective, at),
                    Err(Error::InvalidDidDocument(_))
                ),
                "verify_chain must refuse a DID document with a {label}"
            ),
        }
    }
}

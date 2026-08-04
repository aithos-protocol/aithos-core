//! Conformance vector C3 (I3 owner line, spec §03.1) — edition tier.
//!
//! I3 has two halves (spec §00.2, §03.1, §09.4): a header without the owner
//! line is invalid, **and so is the edition carrying it**. An edition verifier
//! MUST parse every header the edition pins and MUST reject the edition if any
//! key version of any of them has no owner line — the owner line being the one
//! whose recipient key is the subject's `owner_kex`, never the one the routing
//! label points at.
//!
//! The header tier of the same vector lives in
//! `aithos-core/tests/c3_owner_line.rs`. Case `no_owner_line_at_all` is
//! deliberately consumed at BOTH tiers: the header half already holds, the
//! edition half is what no verifier enforced.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::{GenericGrantRequest, GrantSelector};
use aithos_bundle::{FsStore, Store};
use aithos_core::did::DidDocument;
use aithos_core::header::Header;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::Verb;
use aithos_core::path::Zone;
use aithos_core::wire;
use ed25519_dalek::SigningKey;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    name: String,
    must_fail: Option<String>,
}

#[derive(Deserialize)]
struct C3 {
    stranger_multibase: String,
    cases: Vec<Case>,
}

fn vector() -> C3 {
    let raw = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../vectors/c3-owner-line.json"
    ));
    serde_json::from_str(raw).expect("vector c3-owner-line.json parses")
}

fn must_fail(v: &C3, name: &str) -> String {
    v.cases
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("vector C3 has no case {name}"))
        .must_fail
        .clone()
        .unwrap_or_else(|| panic!("vector C3 case {name} states no must_fail"))
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&base).expect("create C3 test base");
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-c3-owner-line-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create C3 root {path:?}: {error}"),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn agent_key() -> SigningKey {
    SigningKey::from_bytes(&[0xc3; 32])
}

fn agent_kid() -> String {
    wire::ed25519_pub_to_multibase(&agent_key().verifying_key().to_bytes())
}

/// A bundle with one granted circle node, hence one real `header.json`
/// pinned by a signed edition, published and verifying.
fn fixture(label: &str) -> (TempRoot, Bundle<FsStore>, OwnerKeys) {
    let root = TempRoot::new(label);
    let seed = MasterSeed::from_slice(&[0xc3; 32]).expect("valid C3 owner seed");
    let owner = OwnerKeys::genesis(&seed);
    let succession = succession_from_entropy([0xc4; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-08-04T10:00:00Z",
    )
    .expect("initialize C3 bundle");
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "C3 target",
                    tags: &["toto".to_owned()],
                    body: "body",
                    now: "2026-08-04T10:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-08-04T10:02:00Z")
        })
        .expect("publish C3 section");
    bundle
        .grant_generic(
            &owner,
            "c3-agent",
            &agent_key().verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Zone,
            )],
            "2026-08-04T10:03:00Z",
            "2026-08-11T10:03:00Z",
            0,
            "2026-08-04T10:03:00Z",
            &mut entropy,
        )
        .expect("issue the C3 circle grant");
    (root, bundle, owner)
}

/// An `e/<zone>/hdr/*.json` the fixture's grant created — the first in path
/// order. One mutilated header is enough to invalidate the edition.
fn header_path(bundle: &Bundle<FsStore>) -> String {
    let mut found: Vec<String> = bundle
        .store
        .list("")
        .expect("list C3 store")
        .into_iter()
        .filter(|p| p.contains("/hdr/") && p.ends_with(".json"))
        .collect();
    found.sort();
    assert!(
        !found.is_empty(),
        "the C3 fixture grant must pin at least one header"
    );
    found.swap_remove(0)
}

fn read_header(bundle: &Bundle<FsStore>, path: &str) -> Header {
    let bytes = bundle
        .store
        .get(path)
        .expect("read C3 header")
        .expect("C3 header exists");
    serde_json::from_slice(&bytes).expect("C3 header parses as a Header")
}

fn write_header(bundle: &mut Bundle<FsStore>, path: &str, header: &Header) {
    let bytes = serde_json::to_vec(header).expect("serialize C3 header");
    bundle.store.put(path, &bytes).expect("write C3 header");
}

/// CHDR-012 at the edition tier: the owner line the production path writes
/// must name `owner_kex` — the very key the DID document publishes — so that
/// a keyless verifier can recognize it without holding anything.
#[test]
fn c3_edition_owner_line_names_the_did_documents_owner_kex() {
    let (_root, bundle, _owner) = fixture("names-kex");
    let doc: DidDocument = serde_json::from_slice(
        &bundle
            .store
            .get("did.json")
            .expect("read did.json")
            .expect("did.json exists"),
    )
    .expect("did.json parses");
    let header = read_header(&bundle, &header_path(&bundle));
    let agent = agent_kid();
    let owner_kids: Vec<String> = header
        .key_versions
        .values()
        .flat_map(|kv| kv.lines.iter())
        .filter(|l| l.kid != agent)
        .map(|l| l.kid.clone())
        .collect();
    assert_eq!(
        owner_kids,
        vec![doc.keys.kex.clone()],
        "the owner line must declare owner_kex as its kid (spec §03.1)"
    );
}

/// CHDR-007, the half of I3 no verifier enforced — vector case
/// `no_owner_line_at_all`, consumed here at the edition tier. The edition is
/// internally consistent: pins, Merkle roots and signature are all recomputed
/// over the mutilated header. Only I3 separates it from a valid edition.
#[test]
fn c3_edition_with_no_owner_line_at_all_is_rejected() {
    let v = vector();
    let (_root, mut bundle, owner) = fixture("no-owner-line");
    bundle
        .verify()
        .expect("C3 fixture edition verifies as built");

    let path = header_path(&bundle);
    let mut header = read_header(&bundle, &path);
    let agent = agent_kid();
    for kv in header.key_versions.values_mut() {
        kv.lines.retain(|l| l.kid == agent);
        assert!(!kv.lines.is_empty(), "the grantee line must survive");
    }
    write_header(&mut bundle, &path, &header);
    bundle
        .publish(&owner, "2026-08-04T10:04:00Z")
        .expect("republish over the mutilated header");

    let error = bundle
        .verify()
        .expect_err("an edition pinning a header with no owner line is invalid (I3, §09.4)");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(&must_fail(&v, "no_owner_line_at_all")),
        "expected {} from Bundle::verify, got {rendered}",
        must_fail(&v, "no_owner_line_at_all")
    );
}

/// CHDR-007 × CHDR-012 — vector case `owner_label_foreign_key`. The header
/// still carries a line labelled `"owner"`; its declared recipient key is a
/// stranger's. The label grants nothing, so the edition is invalid.
#[test]
fn c3_edition_with_owner_labelled_foreign_key_is_rejected() {
    let v = vector();
    let (_root, mut bundle, owner) = fixture("foreign-key");
    bundle
        .verify()
        .expect("C3 fixture edition verifies as built");

    let path = header_path(&bundle);
    let mut header = read_header(&bundle, &path);
    let agent = agent_kid();
    let mut retargeted = 0usize;
    for kv in header.key_versions.values_mut() {
        for line in kv.lines.iter_mut().filter(|l| l.kid != agent) {
            line.kid = v.stranger_multibase.clone();
            retargeted += 1;
        }
    }
    assert!(
        retargeted > 0,
        "the fixture must carry an owner line to retarget"
    );
    write_header(&mut bundle, &path, &header);
    bundle
        .publish(&owner, "2026-08-04T10:04:00Z")
        .expect("republish over the retargeted header");

    let error = bundle
        .verify()
        .expect_err("a line labelled \"owner\" sealed to a stranger is not the owner line");
    let rendered = format!("{error:?}");
    assert!(
        rendered.contains(&must_fail(&v, "owner_label_foreign_key")),
        "expected {} from Bundle::verify, got {rendered}",
        must_fail(&v, "owner_label_foreign_key")
    );
}

//! SPL-7 (chantier split repo) — asservissement de la partition de
//! propriété de `vectors/`.
//!
//! Le manifeste `vectors/ownership.json` matérialise la règle : chaque
//! entrée du répertoire a exactement un propriétaire (`core` reste dans
//! ce dépôt, `service` part avec `aithos-service` au lot SPL-8), et les
//! vecteurs core consommés par les crates service (`shared: true`) ont un
//! propriétaire unique — jamais dupliqués. Le manifeste épingle aussi le
//! SHA-256 de chaque vecteur : la règle 3 du README (« frozen once
//! green ») devient mécanique.
//!
//! Ce harnais asserte l'arbre de CE dépôt : les chemins sont résolus
//! relativement au manifeste du crate (pas de `AITHOS_VECTORS_DIR` ici —
//! contrairement aux fixtures SPL-1, l'objet du test est le répertoire
//! canonique lui-même, pas un vecteur individuel).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Racine du dépôt (le crate vit dans `rust/crates/aithos-bundle`).
fn repo_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.."))
}

/// Crates qui restent côté core, plus le corpus BDD racine.
const CORE_ROOTS: &[&str] = &[
    "rust/crates/aithos-core",
    "rust/crates/aithos-bundle",
    "rust/crates/aithos-cli",
    "rust/crates/aithos-wasm",
    "rust/crates/aithos-owner",
    "features",
];

/// Crates qui partent avec le dépôt service au lot SPL-8.
const SERVICE_ROOTS: &[&str] = &["rust/crates/aithos-gateway", "rust/crates/aithos-provider"];

/// Racines du scan de non-duplication d'octets (tout sauf `vectors/`,
/// `docs/` et les répertoires générés).
const DUP_SCAN_ROOTS: &[&str] = &[
    "rust/crates",
    "features",
    "docker",
    "scripts",
    "spec",
    "demo",
];

struct Entry {
    name: String,
    kind: String,
    owner: String,
    shared: bool,
    sha256: Option<String>,
}

fn manifest() -> Vec<Entry> {
    let path = repo_root().join("vectors/ownership.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("manifest `{}` unreadable: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&raw).expect("ownership.json parses");
    doc["entries"]
        .as_array()
        .expect("ownership.json has an `entries` array")
        .iter()
        .map(|e| Entry {
            name: e["name"].as_str().expect("entry name").to_string(),
            kind: e["kind"].as_str().expect("entry kind").to_string(),
            owner: e["owner"].as_str().expect("entry owner").to_string(),
            shared: e["shared"].as_bool().unwrap_or(false),
            sha256: e["sha256"].as_str().map(str::to_string),
        })
        .collect()
}

/// Fichiers réguliers sous `dir`, récursif, en sautant tout répertoire
/// `target` (artefacts de build éventuels).
fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

/// Contenus des fichiers `.rs` / `.feature` sous les racines données.
fn sources_under(roots: &[&str]) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    for root in roots {
        walk_files(&repo_root().join(root), &mut files);
    }
    files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "rs" || e == "feature"))
        .map(|p| {
            let bytes = fs::read(&p).unwrap_or_default();
            (p, String::from_utf8_lossy(&bytes).into_owned())
        })
        .collect()
}

/// Le manifeste et le répertoire décrivent exactement le même ensemble :
/// aucune entrée non classée, aucune entrée fantôme, aucune double
/// classification, et les invariants de forme du manifeste tiennent.
#[test]
fn manifest_is_a_partition_of_the_vectors_directory() {
    let entries = manifest();
    let mut names = BTreeSet::new();
    for e in &entries {
        assert!(
            matches!(e.kind.as_str(), "vector" | "tooling" | "doc"),
            "entry `{}` has unknown kind `{}`",
            e.name,
            e.kind
        );
        assert!(
            matches!(e.owner.as_str(), "core" | "service"),
            "entry `{}` has unknown owner `{}`",
            e.name,
            e.owner
        );
        if e.shared {
            assert_eq!(
                e.owner, "core",
                "shared entry `{}` must be core-owned",
                e.name
            );
            assert_eq!(
                e.kind, "vector",
                "shared entry `{}` must be a vector",
                e.name
            );
        }
        if e.kind == "vector" {
            assert!(
                e.sha256.is_some(),
                "vector `{}` has no pinned sha256",
                e.name
            );
        }
        assert!(
            names.insert(e.name.clone()),
            "entry `{}` classified twice",
            e.name
        );
    }
    let mut on_disk = BTreeSet::new();
    for de in fs::read_dir(repo_root().join("vectors")).expect("vectors/ readable") {
        let name = de
            .expect("dir entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        // Le manifeste ne se classe pas lui-même.
        if name != "ownership.json" {
            on_disk.insert(name);
        }
    }
    let unclassified: Vec<_> = on_disk.difference(&names).collect();
    let phantom: Vec<_> = names.difference(&on_disk).collect();
    assert!(
        unclassified.is_empty() && phantom.is_empty(),
        "ownership partition broken — unclassified on disk: {unclassified:?}; \
         in manifest but not on disk: {phantom:?}"
    );
}

/// Chaque vecteur égale octet pour octet son digest épinglé (règle 3 du
/// README rendue mécanique — un vecteur gelé ne change jamais).
#[test]
fn vectors_match_their_pinned_digests() {
    for e in manifest().iter().filter(|e| e.kind == "vector") {
        let path = repo_root().join("vectors").join(&e.name);
        let bytes =
            fs::read(&path).unwrap_or_else(|err| panic!("`{}` unreadable: {err}", path.display()));
        let got = aithos_bundle::manifest::sha256_hex(&bytes);
        assert_eq!(
            Some(&got),
            e.sha256.as_ref(),
            "vector `{}` drifted from its pinned digest",
            e.name
        );
    }
}

/// Les crates qui restent (core, bundle, cli, wasm, owner) et le corpus
/// BDD racine ne référencent aucune entrée service : après SPL-8, rien
/// de ce qui reste ne dépend de ce qui part.
#[test]
fn core_side_never_references_service_entries() {
    let service_names: Vec<String> = manifest()
        .into_iter()
        .filter(|e| e.owner == "service")
        .map(|e| e.name)
        .collect();
    let mut hits = Vec::new();
    for (path, content) in sources_under(CORE_ROOTS) {
        for name in &service_names {
            if content.contains(name.as_str()) {
                hits.push(format!("{} -> {}", path.display(), name));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "core-side sources reference service-owned vector entries: {hits:?}"
    );
}

/// Les crates qui partent (gateway, provider) ne consomment que leurs
/// vecteurs et les vecteurs core explicitement `shared` — et chaque
/// `shared` déclaré est réellement consommé (la liste est exacte dans
/// les deux sens).
#[test]
fn service_side_consumes_only_its_own_and_declared_shared_vectors() {
    let entries = manifest();
    let core_vectors: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.owner == "core" && e.kind == "vector")
        .collect();
    let sources = sources_under(SERVICE_ROOTS);
    let mut undeclared = Vec::new();
    let mut shared_hits: BTreeMap<&str, usize> = core_vectors
        .iter()
        .filter(|e| e.shared)
        .map(|e| (e.name.as_str(), 0))
        .collect();
    for (path, content) in &sources {
        for e in &core_vectors {
            if content.contains(e.name.as_str()) {
                if e.shared {
                    *shared_hits.get_mut(e.name.as_str()).expect("shared entry") += 1;
                } else {
                    undeclared.push(format!("{} -> {}", path.display(), e.name));
                }
            }
        }
    }
    assert!(
        undeclared.is_empty(),
        "service-side sources consume core vectors not declared `shared`: {undeclared:?}"
    );
    let stale: Vec<_> = shared_hits
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| *k)
        .collect();
    assert!(
        stale.is_empty(),
        "declared `shared` vectors with no service-side consumer (stale flag): {stale:?}"
    );
}

/// Aucune copie octet-à-octet d'un vecteur hors de `vectors/` : la
/// non-duplication (critère SPL-7) est prouvée par digest sur tout
/// l'arbre livrable du dépôt.
#[test]
fn vector_bytes_are_never_duplicated_outside_vectors_dir() {
    let digests: BTreeMap<String, String> = manifest()
        .into_iter()
        .filter(|e| e.kind == "vector")
        .map(|e| (e.sha256.expect("pinned digest"), e.name))
        .collect();
    let mut files = Vec::new();
    for root in DUP_SCAN_ROOTS {
        walk_files(&repo_root().join(root), &mut files);
    }
    let mut dup = Vec::new();
    for path in files {
        let bytes = fs::read(&path).unwrap_or_default();
        if let Some(name) = digests.get(&aithos_bundle::manifest::sha256_hex(&bytes)) {
            dup.push(format!("{} duplicates {}", path.display(), name));
        }
    }
    assert!(
        dup.is_empty(),
        "vector bytes duplicated outside vectors/: {dup:?}"
    );
}

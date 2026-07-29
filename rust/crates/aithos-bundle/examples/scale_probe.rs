//! Sonde de coût : que coûte UNE édition de section selon la taille du bundle ?
use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::{MemStore, Store};
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use std::time::Instant;

fn len_of(s: &MemStore, p: &str) -> usize {
    s.get(p).ok().flatten().map(|b| b.len()).unwrap_or(0)
}

fn main() {
    let sizes: Vec<usize> = std::env::args()
        .skip(1)
        .map(|a| a.parse().unwrap())
        .collect();
    let sizes = if sizes.is_empty() {
        vec![100, 500, 1000, 2000]
    } else {
        sizes
    };
    println!(
        "{:>8} | {:>11} | {:>12} | {:>10} | {:>10} | {:>8}",
        "sections", "edit+publish", "manifest Ko", "index Ko", "tree Ko", "objets"
    );
    println!("{}", "-".repeat(80));
    for n in sizes {
        let seed = MasterSeed::from_bytes([7u8; 32]);
        let owner = OwnerKeys::genesis(&seed);
        let succ = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]).verifying_key();
        let mut ent = SeqEntropy::default();
        let store = MemStore::default();
        let mut b = Bundle::init(store, &owner, &succ, &mut ent, "2026-01-01T00:00:00Z").unwrap();
        b.ensure_folder(Zone::Circle, "notes", &owner, &mut ent)
            .unwrap();
        for i in 0..n {
            let name = format!("s{i}");
            let spec = SectionSpec {
                zone: Zone::Circle,
                folder_path: "notes",
                name: &name,
                title: "t",
                tags: &[],
                body: "corps de section de test",
                now: "2026-01-01T00:00:00Z",
            };
            b.section_add(&spec, &owner, &mut ent).unwrap();
        }
        b.publish(&owner, "2026-01-01T00:00:00Z").unwrap();

        let t0 = Instant::now();
        b.section_rewrite(
            Zone::Circle,
            "notes/s0",
            "nouveau corps",
            &owner,
            "2026-01-01T00:01:00Z",
            &mut ent,
        )
        .unwrap();
        b.publish(&owner, "2026-01-01T00:01:00Z").unwrap();
        let dt = t0.elapsed();

        let s = &b.store;
        let man = len_of(s, "manifest.json");
        let idx = len_of(s, "e/circle/index.json");
        let tree = len_of(s, "manifests/tree-2.json");
        let objs = s.list("").map(|v| v.len()).unwrap_or(0);
        println!(
            "{:>8} | {:>9.1} ms | {:>12.1} | {:>10.1} | {:>10.1} | {:>8}",
            n,
            dt.as_secs_f64() * 1000.0,
            man as f64 / 1024.0,
            idx as f64 / 1024.0,
            tree as f64 / 1024.0,
            objs
        );
    }
}

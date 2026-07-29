//! Coût d'éditions SUCCESSIVES sur un bundle de taille fixe.
use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::{MemStore, Store};
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use std::time::Instant;

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .map(|a| a.parse().unwrap())
        .unwrap_or(1000);
    let editions: usize = std::env::args()
        .nth(2)
        .map(|a| a.parse().unwrap())
        .unwrap_or(10);
    let seed = MasterSeed::from_bytes([7u8; 32]);
    let owner = OwnerKeys::genesis(&seed);
    let succ = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]).verifying_key();
    let mut ent = SeqEntropy::default();
    let mut b = Bundle::init(
        MemStore::default(),
        &owner,
        &succ,
        &mut ent,
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
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
            body: "corps",
            now: "2026-01-01T00:00:00Z",
        };
        b.section_add(&spec, &owner, &mut ent).unwrap();
    }
    b.publish(&owner, "2026-01-01T00:00:00Z").unwrap();
    println!("bundle de {n} sections — coût de la Nième édition d'UNE section\n");
    println!(
        "{:>8} | {:>11} | {:>12} | {:>10} | {:>12}",
        "édition", "durée", "manifest Ko", "objets", "octets store"
    );
    println!("{}", "-".repeat(64));
    for e in 2..(2 + editions) {
        let now = format!("2026-01-01T00:{:02}:00Z", e);
        let t0 = Instant::now();
        b.section_rewrite(
            Zone::Circle,
            "notes/s0",
            &format!("corps v{e}"),
            &owner,
            &now,
            &mut ent,
        )
        .unwrap();
        b.publish(&owner, &now).unwrap();
        let dt = t0.elapsed();
        let s = &b.store;
        let man = s
            .get("manifest.json")
            .ok()
            .flatten()
            .map(|v| v.len())
            .unwrap_or(0);
        let paths = s.list("").unwrap_or_default();
        let total: usize = paths
            .iter()
            .filter_map(|p| s.get(p).ok().flatten().map(|v| v.len()))
            .sum();
        println!(
            "{:>8} | {:>8.1} ms | {:>12.1} | {:>10} | {:>10.1} Mo",
            e,
            dt.as_secs_f64() * 1000.0,
            man as f64 / 1024.0,
            paths.len(),
            total as f64 / 1_048_576.0
        );
    }
}

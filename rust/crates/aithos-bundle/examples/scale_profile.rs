//! Où part le temps d'une publication ? Décomposition par phase.
use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::{MemStore, Store};
use aithos_core::keys::{MasterSeed, OwnerKeys};
use aithos_core::path::Zone;
use std::time::Instant;

fn main() {
    for n in [1000usize, 5000] {
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
            b.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "notes",
                    name: &name,
                    title: "t",
                    tags: &[],
                    body: "corps",
                    now: "2026-01-01T00:00:00Z",
                },
                &owner,
                &mut ent,
            )
            .unwrap();
        }
        b.publish(&owner, "2026-01-01T00:00:00Z").unwrap();

        let t = Instant::now();
        b.section_rewrite(
            Zone::Circle,
            "notes/s0",
            "v2",
            &owner,
            "2026-01-01T00:01:00Z",
            &mut ent,
        )
        .unwrap();
        let t_mut = t.elapsed();

        let t = Instant::now();
        let tree = b.state_tree().unwrap();
        let t_tree = t.elapsed();
        let nodes = tree.nodes.len();

        let t = Instant::now();
        let paths = b.store.list("").unwrap();
        let mut total = 0usize;
        for p in &paths {
            total += b.store.get(p).unwrap().map(|v| v.len()).unwrap_or(0);
        }
        let t_scan = t.elapsed();

        let t = Instant::now();
        b.publish(&owner, "2026-01-01T00:02:00Z").unwrap();
        let t_pub = t.elapsed();

        println!("--- {n} sections ---");
        println!(
            "  mutation de section  : {:>8.1} ms",
            t_mut.as_secs_f64() * 1000.0
        );
        println!(
            "  state_tree()         : {:>8.1} ms   ({nodes} nœuds)",
            t_tree.as_secs_f64() * 1000.0
        );
        println!(
            "  scan+lecture complet : {:>8.1} ms   ({} chemins, {:.1} Mo)",
            t_scan.as_secs_f64() * 1000.0,
            paths.len(),
            total as f64 / 1048576.0
        );
        println!(
            "  publish() complet    : {:>8.1} ms",
            t_pub.as_secs_f64() * 1000.0
        );
    }
}

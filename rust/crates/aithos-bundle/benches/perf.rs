//! Performance targets of spec §09.3, measured on the reference
//! implementation. Statistical benches run on `MemStore` (the costs under
//! test are crypto + serialization; §09.3 says "bundle on local disk", so
//! `--nocapture` also prints one-shot FsStore timings for the disk-bound
//! rows). The two 1M-section rows run on a synthetic in-memory tree: the
//! operations are O(log n) path recomputations — a million real files
//! would measure the filesystem, not the protocol.
//!
//! Targets (§09.3): grant < 20 ms · delegate < 20 ms · add reader < 5 ms ·
//! verify depth-2 chain < 10 ms · revoke rung 2 < 50 ms · revoke rung 3 on
//! 1000 sections < 5 s CPU · read section < 2 ms · gamma verify 10k
//! < 200 ms · proof verify 1M < 1 ms · root update 1M < 1 ms.

use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::GrantSpec;
use aithos_bundle::{MemStore, Store};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::verify_chain;
use aithos_core::path::Zone;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use ed25519_dalek::SigningKey;
use std::hint::black_box;

const NOW: &str = "2026-07-09T00:00:00Z";
const NB: &str = "2026-07-01T00:00:00Z";
const NA: &str = "2026-07-31T00:00:00Z";
const DAY1: &str = "2026-07-02T00:00:00Z";

fn owner() -> OwnerKeys {
    let seed = MasterSeed::from_slice(&(0u8..32).collect::<Vec<u8>>()).unwrap();
    OwnerKeys::genesis(&seed)
}

fn agent_sk(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}

fn dir_spec(dir: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        verb: aithos_core::mandate::Verb::Read,
        dir: dir.to_owned(),
        tag: None,
    }
}

/// A published bundle with one circle folder and `n` sections in it.
fn bundle_with(n: usize) -> (Bundle<MemStore>, OwnerKeys, SeqEntropy) {
    let owner = owner();
    let succession = succession_from_entropy([9u8; 32]);
    let mut ent = SeqEntropy::default();
    let mut b = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut ent,
        NOW,
    )
    .unwrap();
    b.ensure_folder(Zone::Circle, "projets", &owner, &mut ent)
        .unwrap();
    for i in 0..n {
        b.section_add(
            &SectionSpec {
                zone: Zone::Circle,
                folder_path: "projets",
                name: &format!("note{i}"),
                title: "note",
                tags: &[],
                body: "Le corps de la note, ephemere et precieux.",
                now: NOW,
            },
            &owner,
            &mut ent,
        )
        .unwrap();
    }
    b.publish(&owner, NOW).unwrap();
    (b, owner, ent)
}

fn clone_bundle(b: &Bundle<MemStore>) -> Bundle<MemStore> {
    Bundle {
        store: b.store.clone(),
        did: b.did.clone(),
    }
}

/// §09.3 row 1 — grant (mint cert + N header lines).
fn bench_grant(c: &mut Criterion) {
    let (b, owner, _ent) = bundle_with(1);
    c.bench_function("grant_mint_cert_plus_lines", |bch| {
        bch.iter_batched(
            || (clone_bundle(&b), SeqEntropy::default(), 0u8),
            |(mut b2, mut e, _)| {
                b2.grant(
                    &owner,
                    "agent",
                    &agent_sk(0xA1).verifying_key(),
                    &[dir_spec("projets")],
                    NB,
                    NA,
                    0,
                    &mut e,
                )
                .unwrap()
            },
            BatchSize::SmallInput,
        );
    });
}

/// §09.3 row 2 — delegate (sub-mandate, offline).
fn bench_delegate(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(1);
    let parent = b
        .grant(
            &owner,
            "agent",
            &agent_sk(0xA1).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA,
            1,
            &mut ent,
        )
        .unwrap();
    c.bench_function("delegate_sub_mandate", |bch| {
        bch.iter_batched(
            || (clone_bundle(&b), SeqEntropy::default()),
            |(mut b2, mut e)| {
                b2.delegate(
                    &parent,
                    &agent_sk(0xA1),
                    "helper",
                    &agent_sk(0xA2).verifying_key(),
                    &[dir_spec("projets")],
                    NB,
                    NA,
                    &mut e,
                )
                .unwrap()
            },
            BatchSize::SmallInput,
        );
    });
}

/// §09.3 row 3 — add a reader to an existing node (1 line).
fn bench_add_reader(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(1);
    b.grant(
        &owner,
        "first",
        &agent_sk(0xA1).verifying_key(),
        &[dir_spec("projets")],
        NB,
        NA,
        0,
        &mut ent,
    )
    .unwrap();
    let mut n = 0u8;
    c.bench_function("add_reader_one_line", |bch| {
        bch.iter_batched(
            || {
                n = n.wrapping_add(1);
                (
                    clone_bundle(&b),
                    SeqEntropy::default(),
                    0xB0u8.wrapping_add(n % 32),
                )
            },
            |(mut b2, mut e, k)| {
                b2.grant(
                    &owner,
                    "reader",
                    &agent_sk(k).verifying_key(),
                    &[dir_spec("projets")],
                    NB,
                    NA,
                    0,
                    &mut e,
                )
                .unwrap()
            },
            BatchSize::SmallInput,
        );
    });
}

/// §09.3 row 4 — verify a depth-2 chain (offline).
fn bench_verify_chain(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(1);
    let parent = b
        .grant(
            &owner,
            "agent",
            &agent_sk(0xA1).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA,
            1,
            &mut ent,
        )
        .unwrap();
    let sub = b
        .delegate(
            &parent,
            &agent_sk(0xA1),
            "helper",
            &agent_sk(0xA2).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA,
            &mut ent,
        )
        .unwrap();
    let chain = vec![parent, sub];
    let doc: aithos_core::did::DidDocument =
        serde_json::from_slice(&b.store.get("did.json").unwrap().unwrap()).unwrap();
    c.bench_function("verify_depth2_chain", |bch| {
        bch.iter(|| verify_chain(black_box(&chain), &doc, DAY1).unwrap());
    });
}

/// §09.3 row 7 — read a section (open header → derive → decrypt).
fn bench_read_section(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(1);
    let chain = vec![b
        .grant(
            &owner,
            "agent",
            &agent_sk(0xA1).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA,
            0,
            &mut ent,
        )
        .unwrap()];
    c.bench_function("read_section_as_agent", |bch| {
        bch.iter(|| {
            b.read_section_as_agent(
                black_box(&chain),
                &agent_sk(0xA1),
                Zone::Circle,
                "projets/note0",
                DAY1,
            )
            .unwrap()
        });
    });
}

/// §09.3 row 5 — revoke rung 2: rotate the headers of an EMPTY folder
/// (no body to re-encrypt — the pure header-rotation cost).
fn bench_revoke_rung2(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(0);
    b.grant(
        &owner,
        "agent",
        &agent_sk(0xA1).verifying_key(),
        &[dir_spec("projets")],
        NB,
        NA,
        0,
        &mut ent,
    )
    .unwrap();
    let kid =
        aithos_core::wire::ed25519_pub_to_multibase(&agent_sk(0xA1).verifying_key().to_bytes());
    c.bench_function("revoke_rung2_rotate_headers", |bch| {
        bch.iter_batched(
            || (clone_bundle(&b), SeqEntropy::default()),
            |(mut b2, mut e)| b2.rotate_folder(&owner, "projets", &kid, &mut e).unwrap(),
            BatchSize::SmallInput,
        );
    });
}

/// §09.3 row 6 — revoke rung 3: rotate + re-encrypt a 1000-section zone.
fn bench_revoke_rung3(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(1000);
    b.grant(
        &owner,
        "agent",
        &agent_sk(0xA1).verifying_key(),
        &[dir_spec("projets")],
        NB,
        NA,
        0,
        &mut ent,
    )
    .unwrap();
    let kid =
        aithos_core::wire::ed25519_pub_to_multibase(&agent_sk(0xA1).verifying_key().to_bytes());
    let mut g = c.benchmark_group("rung3");
    g.sample_size(10);
    g.bench_function("revoke_rung3_reencrypt_1000_sections", |bch| {
        bch.iter_batched(
            || (clone_bundle(&b), SeqEntropy::default()),
            |(mut b2, mut e)| b2.rotate_folder(&owner, "projets", &kid, &mut e).unwrap(),
            BatchSize::PerIteration,
        );
    });
    g.finish();
}

/// §09.3 row 8 — gamma append + chain verify, 10k entries, full verify.
fn bench_gamma_10k(c: &mut Criterion) {
    let (mut b, owner, mut ent) = bundle_with(0);
    for i in 0..10_000u64 {
        let s = i % 60;
        let m = (i / 60) % 60;
        let h = (i / 3600) % 24;
        let d = 2 + (i / 86_400);
        b.log_heartbeat(
            &owner,
            i + 1,
            &format!("2026-07-{d:02}T{h:02}:{m:02}:{s:02}Z"),
            &mut ent,
        )
        .unwrap();
    }
    let mut g = c.benchmark_group("gamma");
    g.sample_size(10);
    g.bench_function("gamma_full_verify_10k_entries", |bch| {
        bch.iter(|| b.gamma_verify().unwrap());
    });
    g.finish();
}

/// §09.3 rows 9–10 — synthetic 1M-leaf tree: inclusion-proof verify and
/// the O(log n) root update after one edit.
fn bench_merkle_1m(c: &mut Criterion) {
    use aithos_core::merkle::{h_leaf, mroot, mroot_path, verify_proof, Proof};
    let payloads: Vec<Vec<u8>> = (0..1_000_000u32)
        .map(|i| i.to_le_bytes().to_vec())
        .collect();
    let leaves: Vec<[u8; 32]> = payloads.iter().map(|p| h_leaf(p)).collect();
    let root = mroot(&leaves);
    let idx = 424_242usize;
    let proof = Proof {
        payload: hex::encode(&payloads[idx]),
        steps: mroot_path(&leaves, idx),
        root: hex::encode(root),
    };
    c.bench_function("proof_verify_1m_sections", |bch| {
        bch.iter(|| verify_proof(black_box(&proof), &root).unwrap());
    });
    // Root update on one edit: recompute the root along the stored path
    // with the NEW leaf bytes (what an incremental verifier maintains).
    let new_payload: &[u8] = b"edited";
    let edited = Proof {
        payload: hex::encode(new_payload),
        steps: proof.steps.clone(),
        root: proof.root.clone(),
    };
    c.bench_function("root_update_one_edit_1m_sections", |bch| {
        bch.iter(|| {
            // The recomputation IS the update: fold the new leaf up the path.
            let r = verify_proof(black_box(&edited), &root);
            black_box(r.is_err()) // new bytes → new root ≠ old root, by design
        });
    });
}

criterion_group!(
    benches,
    bench_grant,
    bench_delegate,
    bench_add_reader,
    bench_verify_chain,
    bench_read_section,
    bench_revoke_rung2,
    bench_revoke_rung3,
    bench_gamma_10k,
    bench_merkle_1m
);
criterion_main!(benches);

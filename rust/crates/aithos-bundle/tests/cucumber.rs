//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use aithos_core::keys::{MasterSeed, OwnerKeys};
use cucumber::{given, then, when, World};

/// One derived identity, as its public keys (hex), in a fixed order:
/// root_sign, content_sign, owner_kex (§01.1).
type PublicIdentity = Vec<String>;

fn public_identity(keys: &OwnerKeys) -> PublicIdentity {
    vec![
        hex::encode(keys.root_sign.verifying_key().to_bytes()),
        hex::encode(keys.content_sign.verifying_key().to_bytes()),
        hex::encode(keys.owner_kex_pub().to_bytes()),
    ]
}

#[derive(Debug, Default, World)]
pub struct ProtocolWorld {
    seeds: Vec<Vec<u8>>,
    identities: Vec<PublicIdentity>,
    rejection: Option<String>,
}

impl ProtocolWorld {
    fn derive_from(&mut self, seed_index: usize) {
        let seed = MasterSeed::from_slice(&self.seeds[seed_index]).expect("valid seed");
        self.identities
            .push(public_identity(&OwnerKeys::genesis(&seed)));
    }
}

// ---------------------------------------------------------------- givens

#[given("a master seed")]
fn a_master_seed(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
}

#[given("two different master seeds")]
fn two_master_seeds(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.seeds.push((100u8..132).collect());
}

#[given("a 31-byte seed candidate")]
fn a_short_seed(w: &mut ProtocolWorld) {
    w.seeds.push(vec![7u8; 31]);
}

// ----------------------------------------------------------------- whens

#[when("I derive the owner keys")]
fn derive_once(w: &mut ProtocolWorld) {
    w.derive_from(0);
}

#[when("I derive the owner keys twice")]
fn derive_twice(w: &mut ProtocolWorld) {
    w.derive_from(0);
    w.derive_from(0);
}

#[when("I derive the owner keys from each seed")]
fn derive_from_each(w: &mut ProtocolWorld) {
    for i in 0..w.seeds.len() {
        w.derive_from(i);
    }
}

#[when("I try to derive the owner keys")]
fn try_derive(w: &mut ProtocolWorld) {
    match MasterSeed::from_slice(&w.seeds[0]) {
        Ok(seed) => {
            w.identities
                .push(public_identity(&OwnerKeys::genesis(&seed)));
        }
        Err(e) => w.rejection = Some(e.to_string()),
    }
}

// ----------------------------------------------------------------- thens

#[then("both derivations yield the same public identity")]
fn same_identity(w: &mut ProtocolWorld) {
    assert_eq!(w.identities.len(), 2);
    assert_eq!(w.identities[0], w.identities[1]);
}

#[then("the two identities share no public key")]
fn unrelated_identities(w: &mut ProtocolWorld) {
    assert_eq!(w.identities.len(), 2);
    let shared = w.identities[0]
        .iter()
        .filter(|k| w.identities[1].contains(k))
        .count();
    assert_eq!(shared, 0, "identities must share no public key");
}

#[then("the three public keys are pairwise distinct")]
fn domain_separated(w: &mut ProtocolWorld) {
    let id = &w.identities[0];
    let unique: std::collections::BTreeSet<_> = id.iter().collect();
    assert_eq!(unique.len(), id.len(), "keys must be pairwise distinct");
}

#[then(expr = "genesis is rejected with {string}")]
fn rejected_with(w: &mut ProtocolWorld, expected: String) {
    let rejection = w
        .rejection
        .as_deref()
        .expect("genesis should have been rejected");
    assert!(
        rejection.contains(&expected),
        "rejection '{rejection}' should mention '{expected}'"
    );
    assert!(
        w.identities.is_empty(),
        "no identity may exist after rejection"
    );
}

// ------------------------------------------------------------------ main

fn main() {
    let features = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../features");
    // Ritual (docs/EXECUTION-PLAN.md): each phase's .feature is co-written and
    // committed BEFORE implementation, its scenarios tagged @wip. The filter
    // keeps the suite green until a scenario is implemented and untagged.
    futures::executor::block_on(
        ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| {
            !scenario.tags.iter().any(|t| t == "wip")
        }),
    );
}

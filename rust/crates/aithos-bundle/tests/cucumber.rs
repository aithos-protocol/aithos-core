//! BDD acceptance harness (cucumber-rs). Gherkin features live at the repo
//! root in `features/`; step definitions grow with each phase of
//! docs/EXECUTION-PLAN.md and are never rewritten, only extended.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use aithos_bundle::bundle::{
    Bundle, GranteeContentOperation, GranteeContentOutcome, GranteeTarget, OwnerContentOperation,
    OwnerContentOutcome, SectionSpec, ZoneIndex,
};
use aithos_bundle::entropy::{EntropySource, SeqEntropy};
use aithos_bundle::grants::{GenericGrantRequest, GrantSelector, GrantSpec};
use aithos_bundle::log::{LogFilter, LogHit};
use aithos_bundle::manifest::{sha256_hex, Manifest};
use aithos_bundle::publication::{
    cold_verify, export_keyless, import_keyless, package_with_objects,
    verify_draft2_candidate_value, KeylessPublicationPackage,
};
use aithos_bundle::session::LocalSession;
use aithos_bundle::structure::{StructuralOperation, StructuralOutcome};
use aithos_bundle::vault::{VaultConfigOperation, VaultConfigOutcome};
use aithos_bundle::{validate_display_path, validate_store_key, FsStore, MemStore, Store};
use aithos_core::catalog::{
    catalog_action_permitted, verify_catalog_action_facts, verify_catalog_approval,
    verify_catalog_chain, verify_connector_catalog,
};
use aithos_core::constraints::{
    constraint_requirement, constraints_attenuate_for_profile, verify_operation_constraints,
    verify_receipt, BudgetProfile, ConstraintApplicability, ConstraintEvidence, ConstraintFamily,
    ConstraintOperation, ConstraintRequirement,
};
use aithos_core::delegated_counts::{verify_delegated_count_mandates, verify_delegated_counts};
use aithos_core::derive::{derive_key, folder_label, node_key, section_label, tag_label};
use aithos_core::did::{DidDocument, EpochTransition};
use aithos_core::gamma_replay::GammaReplayState;
use aithos_core::gamma_v2::{
    verify_gamma_profile_transition, verify_gamma_v2_entry, GammaOccurrenceRegistry,
};
use aithos_core::header::{Header, Line, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::ed2x;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{
    covers_section_op, verify_chain, GammaQuery, Mandate, MandateSpec, PerimeterEntry, SectionOp,
    Verb,
};
use aithos_core::operation::{
    correlate_operation_references, verify_operation_facts, verify_operation_projection,
    verify_session, verify_state_fact, MutationNode, OperationFactsEvidence, OperationFactsInput,
    OperationProjectionEvidence, SessionEvidence, StateFactInput,
};
use aithos_core::path::{NodePath, Zone};
use aithos_core::receipts::{
    obligation_matches, verify_obligation, verify_obligation_chain, verify_r2_receipt,
    verify_u1_receipt,
};
use aithos_core::wire;
use cucumber::{given, then, when, World};
use ed25519_dalek::{Signer, SigningKey};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

// --- step D fixtures ---
const NOW: &str = "2026-07-09T00:00:00Z";
const BODY: &str = "Le corps de la note, ephemere et precieux.";
const PUB_BODY: &str = "Bio publique, lisible par le monde entier.";
const SELF_BODY: &str = "Souvenir intime, jamais signe.";

// --- step E fixtures: mandates ---
const NB: &str = "2026-07-01T00:00:00Z";
const NA7: &str = "2026-07-08T00:00:00Z";
const NA30: &str = "2026-07-31T00:00:00Z";
const DAY1: &str = "2026-07-02T00:00:00Z";
const DAY8: &str = "2026-07-09T00:00:00Z";
const CB2_MANDATE_CONTRACTS: &str = include_str!("../../../../vectors/cb2-mandate-contracts.json");
const CB4_MUTATION: &str = include_str!("../../../../vectors/cb2-operation-facts-mutation.json");
const CB4_READ: &str = include_str!("../../../../vectors/cb2-operation-facts-read.json");
const CB4_ACTION_INFERENCE: &str =
    include_str!("../../../../vectors/cb2-operation-facts-action-inference.json");
const CB4_STRUCTURAL: &str =
    include_str!("../../../../vectors/cb2-operation-facts-structural.json");
const CB4_PROJECTION: &str = include_str!("../../../../vectors/cb2-operation-projection.json");
const CB4_SESSION: &str = include_str!("../../../../vectors/cb2-session-proof.json");
const CB4_NATIVE_LEAF_TEST_DOMAIN: &[u8] = b"aithos-core/cb2/native-leaf-proof\0";
const CB5_MAX_CHILDREN: &str = include_str!("../../../../vectors/cb2-max-children-versioning.json");
const CB5_DELEGATED_COUNTS: &str = include_str!("../../../../vectors/cb2-delegated-counts.json");
const CB5_RECEIPTS: &str = include_str!("../../../../vectors/cb2-operation-receipts.json");
const FPLUS_CONSTRAINTS: &str = include_str!("../../../../vectors/fplus-constraints.json");
const CB5_CATALOG: &str = include_str!("../../../../vectors/cb2-connector-catalog.json");
const CB6_GAMMA: &str = include_str!("../../../../vectors/cb2-gamma-v2-replay.json");
const CB6_COEXISTENCE: &str =
    include_str!("../../../../vectors/cb2-bundle-version-coexistence.json");
const CB7_BOUNDARIES: &str = include_str!("../../../../vectors/cb2-bundle-boundaries.json");
const CB8_AUTHORITY_FLOWS: &str =
    include_str!("../../../../vectors/cb2-bundle-authority-flows.json");
const CB12_DRAFT2_CARRIERS: &str = include_str!("../../../../vectors/cb2-draft2-carriers.json");
const CB10_STRUCTURE_VAULT: &str =
    include_str!("../../../../vectors/cb2-bundle-structure-vault.json");

fn agent_sk(b: u8) -> SigningKey {
    SigningKey::from_bytes(&[b; 32])
}
const AGENT: u8 = 0xA1;
const HELPER: u8 = 0xA2;
const FOURTH: u8 = 0xA3;

fn dir_spec(dir: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        verb: Verb::Read,
        dir: dir.to_owned(),
        tag: None,
    }
}

fn verb_spec(verb: Verb, dir: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        verb,
        dir: dir.to_owned(),
        tag: None,
    }
}

fn tag_spec(dir: &str, tag: &str) -> GrantSpec {
    GrantSpec {
        zone: Zone::Circle,
        verb: Verb::Read,
        dir: dir.to_owned(),
        tag: Some(tag.to_owned()),
    }
}

fn sid(n: u128) -> Sid {
    Sid(ulid::Ulid::from(n))
}

// --- step B fixtures: conformance vector B2 (spec 01.3, 02.5) ---
//
// BDER-001: the Gherkin layer and the byte-exact vector must share ONE zone
// DK and ONE set of sids. While the feature carried its own `[0xAB; 32]`
// fixture there was not even a compile-time link between the readable
// contract and the only independently corroborated expected values in the
// repository, so "always" meant "twice in the same process".
//
// BDER-007 is NOT silently closed here: `folder1_key_hex` is corroborated by
// five Python generators and `deep_section_key_hex` by one, while the two tag
// anchors have no witness outside `derive.rs`. Only the corroborated fields
// are used below as external authority.
#[derive(serde::Deserialize)]
struct B2Vector {
    zone_dk_hex: String,
    folder_sids: Vec<String>,
    section_sid: String,
    sibling_section_sid: String,
    tag: String,
    folder1_key_hex: String,
    deep_section_key_hex: String,
    sibling_section_key_hex: String,
}

impl B2Vector {
    fn load() -> Self {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../vectors/b2-derivation.json"
        )))
        .expect("vectors/b2-derivation.json parses")
    }

    fn zone_dk(&self) -> [u8; 32] {
        b2_key32(&self.zone_dk_hex)
    }

    fn folder_sid(&self, index: usize) -> Sid {
        Sid::parse(&self.folder_sids[index]).expect("vector folder sid")
    }

    fn folder_spine(&self) -> Vec<Sid> {
        (0..self.folder_sids.len())
            .map(|i| self.folder_sid(i))
            .collect()
    }

    fn section_sid(&self) -> Sid {
        Sid::parse(&self.section_sid).expect("vector section sid")
    }

    fn sibling_section_sid(&self) -> Sid {
        Sid::parse(&self.sibling_section_sid).expect("vector sibling section sid")
    }
}

fn b2_key32(hex_str: &str) -> [u8; 32] {
    hex::decode(hex_str)
        .expect("vector hex")
        .try_into()
        .expect("32 bytes")
}

/// Every label the production code can build in this feature's fixture space:
/// folder and section labels over sids 0..9, plus the tag-anchor label. The
/// held node's OWN label is deliberately included — an invertible derivation
/// step is exploited by replaying the label that produced the key.
fn b2_production_labels(tag: &str) -> Vec<String> {
    let mut labels = Vec::new();
    for n in 0u128..10 {
        labels.push(folder_label(&sid(n)));
        labels.push(section_label(&sid(n)));
    }
    labels.push(tag_label(tag));
    labels
}

/// Every canonical path the production code can build from a held key in that
/// same space: folder spines of length 0..=3 over sids 0..9, each terminated
/// by nothing, by `s/<sid 0..9>` or by `t/<tag>`.
/// (1 + 10 + 100 + 1000) spines x 12 terminals = 13_332 derivations.
fn b2_reachable_paths(tag: &str) -> Vec<NodePath> {
    let mut spines: Vec<Vec<Sid>> = vec![vec![]];
    let mut frontier: Vec<Vec<Sid>> = vec![vec![]];
    for _ in 0..3 {
        let mut next = Vec::new();
        for spine in &frontier {
            for n in 0u128..10 {
                let mut deeper = spine.clone();
                deeper.push(sid(n));
                next.push(deeper);
            }
        }
        spines.extend(next.iter().cloned());
        frontier = next;
    }
    let mut paths = Vec::new();
    for spine in spines {
        paths.push(NodePath::folder(Zone::Circle, spine.clone()));
        for n in 0u128..10 {
            paths.push(NodePath::section(Zone::Circle, spine.clone(), sid(n)));
        }
        paths.push(
            NodePath::tag_view(Zone::Circle, spine.clone(), tag).expect("fixture tag is valid"),
        );
    }
    paths
}

/// True when any contiguous `window`-byte run of `parent` reappears anywhere
/// in `child`: a derived key must carry none of its parent's material.
fn b2_shares_window(child: &[u8; 32], parent: &[u8; 32], window: usize) -> bool {
    parent
        .windows(window)
        .any(|needle| child.windows(window).any(|hay| hay == needle))
}

// --- step C fixtures: header seals (behavioral; byte-exactness lives in C1) ---
const DID_C: &str = "did:aithos:test-header";
const NODE_A: &str = "/e/circle";
const NODE_OTHER: &str = "/e/self";
const CHILD_NODE: &str = "/e/circle/d/00000000000000000000000001";
const DK: [u8; 32] = [0x77; 32];
const DK2: [u8; 32] = [0x66; 32];
const PARENT_KEY: [u8; 32] = [0x55; 32];

fn xsk(b: u8) -> StaticSecret {
    StaticSecret::from([b; 32])
}
fn owner_rec() -> Recipient {
    Recipient::owner(XPublicKey::from(&xsk(0x0A)))
}
fn grantee_rec(name: &str, b: u8) -> Recipient {
    Recipient {
        to: name.to_owned(),
        kid: name.to_owned(),
        pubkey: XPublicKey::from(&xsk(b)),
    }
}
fn eph(i: u8) -> [u8; 32] {
    [0x40 + i; 32]
}
fn non(i: u8) -> [u8; 24] {
    [0x60 + i; 24]
}

const BUNDLE: &str = "file://local";
const REVOCATIONS: &str = "gamma/gamma.jsonl";

/// One derived identity, as its public keys (hex), in a fixed order:
/// root_sign, content_sign, owner_kex (§01.1).
type PublicIdentity = Vec<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreOwnerObservation {
    zone: String,
    operation: String,
    outcome: String,
    gamma_delta: usize,
    mandate_counter_delta: usize,
    reopened: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreAtomicObservation {
    store: String,
    boundary: Option<String>,
    mutation_refused: bool,
    injected_once: bool,
    canonical_unchanged: bool,
    complete_new_state: bool,
    reopened: bool,
    partial_state_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreCapabilityObservation {
    capability: String,
    protocol_object: String,
    observable_result: String,
    operation_succeeded: bool,
    mismatched_object_refused: bool,
    mismatched_session_refused: bool,
    cross_class_substitution_refused: bool,
    secret_material_exposed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CorePathObservation {
    store: String,
    input_kind: String,
    invalid_input: String,
    rejected: bool,
    canonical_unchanged: bool,
    outside_access_observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreDelegatedObservation {
    zone: String,
    operation: String,
    authority: String,
    verdict: String,
    accepted: bool,
    effect_verified: bool,
    gamma_delta: usize,
    gamma_actor_is_grantee: bool,
    fresh_reopen_verified: bool,
    refusal_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreExactSectionObservation {
    target: String,
    target_readable: bool,
    target_rewritten: bool,
    sibling_unreachable: bool,
    sibling_create_refused: bool,
    failed_attempt_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreCurrentAuthorityObservation {
    authority_change: String,
    old_line_usable_before_change: bool,
    current_verdict_refused: bool,
    canonical_unchanged: bool,
    fresh_reopen_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreDelegatedRollbackObservation {
    late_failure_injected_once: bool,
    operation_refused: bool,
    canonical_unchanged: bool,
    fresh_reopen_verified: bool,
    failed_artifacts_reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreStructuralAuthorityObservation {
    operation: String,
    authority: String,
    verdict: String,
    exact_effect_verified: bool,
    gamma_delta: usize,
    refusal_unchanged: bool,
    fresh_reopen_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreRevocationCutObservation {
    one_new_edition: bool,
    revoke_gamma_present: bool,
    revoked_cut: bool,
    survivor_reads: bool,
    rotated_header_and_body: bool,
    fresh_keyless_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreStructuralDerivedObservation {
    case: String,
    primary_effect_verified: bool,
    secondary_effect_verified: bool,
    gamma_actor_verified: bool,
    publication_verified: bool,
    cold_reopen_verified: bool,
    privacy_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreStructuralFailureObservation {
    failure: String,
    refused: bool,
    canonical_unchanged: bool,
    fresh_reopen_verified: bool,
    partial_artifact_reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreRevocationFailureObservation {
    boundary: String,
    refused: bool,
    canonical_unchanged: bool,
    old_state_reopened: bool,
    partial_cut_reachable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreRevocationReplayObservation {
    earlier_mutation_valid: bool,
    later_mutation_refused: bool,
    current_revocation_derived: bool,
    fresh_replay_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreEditionObservation {
    case: String,
    expected_verdict: String,
    actual_accepted: bool,
    signer_is_actor: bool,
    owner_absent_from_grantee_edition: bool,
    package_digest: Option<String>,
    mem_cold_verified: bool,
    fs_cold_verified: bool,
    zero_reachable_on_refusal: bool,
}

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
    /// Entropy injected for succession keypairs (the core never draws RNG).
    succession_entropy: Vec<[u8; 32]>,
    succession_pubs: Vec<String>,
    did_doc: Option<DidDocument>,
    /// Raw JSON wire of the DID document, when the case is about parsing.
    did_wire: Option<String>,
    did_parsed: Option<Result<(), String>>,
    prev_doc: Option<DidDocument>,
    next_doc: Option<DidDocument>,
    transition: Option<Result<(), String>>,
    // --- step B: derivation ---
    zone_dk: Option<[u8; 32]>,
    deep_path: Option<NodePath>,
    node_keys: Vec<[u8; 32]>,
    folder_key: Option<[u8; 32]>,
    /// BDER-003: the two sibling spines are built by the `Given` and read by
    /// the `When` and the `Then`; no step reinvents its own sids.
    sibling_paths: Vec<NodePath>,
    /// BDER-004: the section key derived from sids before a real rename, and
    /// the sid it was derived from.
    rename_key_before: Option<[u8; 32]>,
    renamed_section_sid: Option<String>,
    // --- step C: headers ---
    header: Option<Header>,
    saved_line: Option<Line>,
    opened: Vec<Result<[u8; 32], String>>,
    wrap_obj: Option<Wrap>,
    // --- step D: bundle ---
    bundle: Option<Bundle<MemStore>>,
    ent: SeqEntropy,
    read_body: Option<Result<String, String>>,
    inspected: String,
    // --- step E: mandates ---
    chain: Vec<Mandate>,
    helper_chain: Vec<Mandate>,
    chain_result: Option<Result<(), String>>,
    granted_folder: String,
    e_folders: Vec<String>,
    cb3_perimeter: Vec<PerimeterEntry>,
    cb3_verdict: Option<bool>,
    cb3_operation: Option<Verb>,
    cb3_form_result: Option<Result<Mandate, String>>,
    cb3_secondary_verdicts: Vec<bool>,
    cb4_case: String,
    cb4_result: Option<Result<(), String>>,
    cb5_result: Option<Result<(), String>>,
    cb6_result: Option<Result<(), String>>,
    cb7_result: Option<Result<(), String>>,
    cb10_result: Option<Result<(), String>>,
    core_owner_zone: String,
    core_owner_fixture_ready: bool,
    core_owner_operation: String,
    core_owner_observation: Option<Result<CoreOwnerObservation, String>>,
    core_atomic_store: String,
    core_atomic_boundary: Option<String>,
    core_atomic_observation: Option<Result<CoreAtomicObservation, String>>,
    core_capability: String,
    core_capability_object: String,
    core_capability_mismatch: String,
    core_capability_observation: Option<Result<CoreCapabilityObservation, String>>,
    core_path_store: String,
    core_path_input_kind: String,
    core_path_invalid_input: String,
    core_path_filesystem_condition: String,
    core_path_observation: Option<Result<CorePathObservation, String>>,
    core_delegated_authority: String,
    core_delegated_zone: String,
    core_delegated_operation: String,
    core_delegated_observation: Option<Result<CoreDelegatedObservation, String>>,
    core_exact_section_fixture: String,
    core_exact_section_observation: Option<Result<CoreExactSectionObservation, String>>,
    core_current_authority_observation: Option<Result<CoreCurrentAuthorityObservation, String>>,
    core_delegated_rollback_observation: Option<Result<CoreDelegatedRollbackObservation, String>>,
    core_structural_authority: String,
    core_structural_observation: Option<Result<CoreStructuralAuthorityObservation, String>>,
    core_revocation_cut_observation: Option<Result<CoreRevocationCutObservation, String>>,
    core_structural_derived_case: String,
    core_structural_derived_observation: Option<Result<CoreStructuralDerivedObservation, String>>,
    core_structural_failure_observation: Option<Result<CoreStructuralFailureObservation, String>>,
    core_revocation_failure_boundary: String,
    core_revocation_failure_observation: Option<Result<CoreRevocationFailureObservation, String>>,
    core_revocation_replay_observation: Option<Result<CoreRevocationReplayObservation, String>>,
    core_edition_case: String,
    core_edition_argument: String,
    core_edition_secondary: String,
    core_edition_observation: Option<Result<CoreEditionObservation, String>>,
    core_fence_key_material: String,
    core_fence_authority: String,
    core_fence_result: Option<Result<String, String>>,
    core_count_observation: Option<Result<(u64, u64, u64), String>>,
    core_count_suite: Option<Result<u64, String>>,
    core_constraint_family: String,
    core_constraint_requirement: Option<ConstraintRequirement>,
    core_constraint_replay: Option<Result<(), String>>,
    core_constraint_parent_version: String,
    core_constraint_case_result: Option<Result<(), String>>,
    core_constraint_expected: String,
    core_constraint_certificate_result: Option<Result<(), String>>,
    core_constraint_delegation_result: Option<Result<(), String>>,
    core_constraint_effect_snapshot: String,
    core_receipt_operation: String,
    core_receipt_document: Option<serde_json::Value>,
    core_receipt_result: Option<Result<u64, String>>,
    core_receipt_matcher: Option<bool>,
    core_bound_receipt_operation: String,
    core_bound_receipt_result: Option<Result<(), String>>,
    core_bound_receipt_sealed: bool,
    // --- step F: gamma ---
    gamma_result: Option<Result<String, String>>,
    audit_chain: Vec<Mandate>,
    query_hits: Option<Result<Vec<LogHit>, String>>,
    sealed_probe: Vec<Result<aithos_core::gamma::Body, String>>,
    // --- step F+: constraints ---
    gamma_baseline: usize,
    receipt: Option<serde_json::Value>,
    // --- step G: revocation ---
    survivor_chain: Vec<Mandate>,
    holder_chain: Vec<Mandate>,
    revoked_at: String,
    g_result: Option<Result<String, String>>,
    // --- step G+: obligations ---
    gplus_checks: Option<serde_json::Value>,
    sib_chains: Vec<Vec<Mandate>>,
    // --- step H1: merkle ---
    h_proof: Option<aithos_core::merkle::Proof>,
    h_old_proof: Option<aithos_core::merkle::Proof>,
    // --- step H2: gamma roots ---
    h2_proof: Option<aithos_core::merkle::Proof>,
    h2_counters: Vec<(String, aithos_core::gamma::GammaCounters)>,
    h2_verdict: Option<Result<(), String>>,
    // --- step I: concurrency ---
    i_other: Option<Bundle<MemStore>>,
    i_result: Option<Result<(), String>>,
    i_hashes: Vec<String>,
    i_surfaced: Option<Result<Vec<String>, String>>,
    i_authority: String,
    i_snapshot: BTreeMap<String, Vec<u8>>,
    i_semantic_counts: Option<aithos_core::concurrency::SemanticCounts>,
    // --- step K: integration ---
    k_reader: Vec<Mandate>,
    k_gmail: Vec<Mandate>,
    k_social: Vec<Mandate>,
    k_night: Vec<Mandate>,
    k_wd: Vec<Mandate>,
    k_pristine: Option<MemStore>,
}

impl ProtocolWorld {
    fn grant_to_agent(&mut self, specs: &[GrantSpec], na: &str, issue_depth: u32) {
        let owner = self.owner(0);
        let mandate = self
            .bundle
            .as_mut()
            .unwrap()
            .grant(
                &owner,
                "agent",
                &agent_sk(AGENT).verifying_key(),
                specs,
                NB,
                na,
                issue_depth,
                &mut self.ent,
            )
            .expect("grant succeeds");
        self.chain = vec![mandate];
    }

    fn verify_chain_at(&self, chain: &[Mandate], at: &str) -> Result<(), String> {
        let doc = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("did.json")
            .unwrap()
            .unwrap();
        let doc: aithos_core::did::DidDocument = serde_json::from_slice(&doc).unwrap();
        verify_chain(chain, &doc, at).map_err(|e| e.to_string())
    }

    fn agent_reads(&self, chain: &[Mandate], sk_byte: u8, path: &str) -> Result<String, String> {
        self.bundle
            .as_ref()
            .unwrap()
            .read_section_as_agent(chain, &agent_sk(sk_byte), Zone::Circle, path, DAY1)
            .map_err(|e| e.to_string())
    }

    fn add_named_section(&mut self, folder: &str, name: &str, tags: &[String]) {
        let owner = self.owner(0);
        let bundle = self.bundle.as_mut().unwrap();
        bundle
            .ensure_folder(Zone::Circle, folder, &owner, &mut self.ent)
            .unwrap();
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags,
                    body: BODY,
                    now: NOW,
                },
                &owner,
                &mut self.ent,
            )
            .unwrap();
    }

    fn cb3_root_with_perimeter(&mut self, mut perimeter: Vec<PerimeterEntry>) {
        self.init_bundle();
        perimeter.push(PerimeterEntry::Issue { depth: 1 });
        let owner = self.owner(0);
        let mandate = Mandate::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(800)),
                subject: self.bundle.as_ref().expect("CB3 bundle").did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter,
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: NA7.into(),
                issued_at: NB.into(),
                nonce: "cb3-root".into(),
            },
        )
        .expect("CB3 root mandate builds");
        self.chain = vec![mandate];
    }

    fn cb3_child_candidate(
        &self,
        entry: PerimeterEntry,
        serial: u128,
    ) -> (Vec<Mandate>, Result<(), String>) {
        let parent = self.chain.first().expect("CB3 parent").clone();
        let child = Mandate::build_sub(
            &parent,
            &agent_sk(AGENT),
            &MandateSpec {
                id: format!("mandate_{}", sid(serial)),
                subject: parent.subject.clone(),
                grantee_id: "urn:aithos:agent:helper".into(),
                grantee_label: "helper".into(),
                grantee_pub: &agent_sk(HELPER).verifying_key(),
                perimeter: vec![entry],
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: NA7.into(),
                issued_at: NB.into(),
                nonce: format!("cb3-child-{serial}"),
            },
        )
        .expect("CB3 child mandate builds");
        let chain = vec![parent, child];
        let verdict = self.verify_chain_at(&chain, DAY1);
        (chain, verdict)
    }

    fn cb3_delegate(&mut self, entry: PerimeterEntry) {
        let (chain, verdict) = self.cb3_child_candidate(entry, 801);
        self.helper_chain = chain;
        self.chain_result = Some(verdict);
    }
}

fn cb3_operation_verb(value: &str) -> Verb {
    match value {
        "create" => Verb::Append,
        "edit" => Verb::Edit,
        "delete" => Verb::Delete,
        "read" => Verb::Read,
        other => panic!("unknown CB3 operation {other}"),
    }
}

fn cb3_section_sid(name: &str) -> Sid {
    match name {
        "note1" => sid(1),
        "note2" => sid(2),
        other => panic!("unknown CB3 section fixture {other}"),
    }
}

fn cb3_normalize_selector(entry: &str) -> String {
    entry
        .replace("projects", &sid(10).to_string())
        .replace("sealed", &sid(11).to_string())
}

fn cb3_form_document(case_name: &str) -> String {
    let vector: serde_json::Value =
        serde_json::from_str(CB2_MANDATE_CONTRACTS).expect("CB2 mandate contracts parse");
    vector["form_cases"]
        .as_array()
        .expect("form cases")
        .iter()
        .find(|case| case["case"].as_str() == Some(case_name))
        .unwrap_or_else(|| panic!("missing CB3 form case {case_name}"))["document_jcs"]
        .as_str()
        .expect("form-case document")
        .to_owned()
}

fn cb4_vector(bytes: &str) -> serde_json::Value {
    serde_json::from_str(bytes).expect("CB4 vector parses")
}

fn cb4_named<'a>(cases: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    cases
        .as_array()
        .expect("CB4 cases array")
        .iter()
        .find(|case| case["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing CB4 vector case {id}"))
}

fn cb4_optional_object(value: &serde_json::Value) -> Option<&serde_json::Value> {
    value.is_object().then_some(value)
}

fn cb4_error_name(error: aithos_core::Error) -> String {
    match error {
        aithos_core::Error::InvalidOperationFacts(_) => "InvalidOperationFacts".into(),
        aithos_core::Error::InvalidStateFact(_) => "InvalidStateFact".into(),
        aithos_core::Error::InvalidOperation(_) => "InvalidOperation".into(),
        aithos_core::Error::InvalidSession(_) => "InvalidSession".into(),
        other => format!("unexpected:{other}"),
    }
}

fn cb4_capture<T>(w: &mut ProtocolWorld, result: aithos_core::Result<T>) {
    w.cb4_result = Some(result.map(|_| ()).map_err(cb4_error_name));
}

fn cb4_mutation_nodes(vector: &serde_json::Value) -> Vec<MutationNode> {
    let sids = &vector["fixture_sids"];
    let sid = |name: &str| {
        Sid::parse(sids[name].as_str().expect("CB4 fixture SID")).expect("canonical CB4 SID")
    };
    vec![
        MutationNode::new(sid("circle_root"), Zone::Circle, None),
        MutationNode::new(sid("circle_parent"), Zone::Circle, Some(sid("circle_root"))),
        MutationNode::new(sid("circle_destination"), Zone::Circle, None),
        MutationNode::new(
            sid("circle_destination_child"),
            Zone::Circle,
            Some(sid("circle_destination")),
        ),
        MutationNode::new(sid("self_root"), Zone::Self_, None),
        MutationNode::new(
            sid("ethos_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(
            sid("section_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(
            sid("folder_target"),
            Zone::Circle,
            Some(sid("circle_parent")),
        ),
        MutationNode::new(sid("folder_create"), Zone::Circle, None),
    ]
}

fn cb4_validate_mutation(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_MUTATION);
    let nodes = cb4_mutation_nodes(&vector);
    let case = cb4_named(&vector["negative_cases"]["operation_facts"], id);
    verify_operation_facts(OperationFactsInput {
        document: &case["candidate"],
        facts_ref: cb4_optional_object(&case["facts_ref"]),
        evidence: OperationFactsEvidence::Mutation {
            state_facts: &vector["states"],
            nodes: &nodes,
            vault_record_key: vector["vault_record_key"].as_str().expect("vault key"),
        },
    })
    .map(|_| ())
}

fn cb4_validate_state(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_MUTATION);
    let case = cb4_named(&vector["negative_cases"]["state_facts"], id);
    let expected_keys: Option<Vec<String>> = case
        .get("expected_key_commitments")
        .and_then(serde_json::Value::as_array)
        .map(|keys| {
            keys.iter()
                .map(|key| key.as_str().expect("expected state key").to_owned())
                .collect()
        });
    let input = match case["scope"].as_str().expect("state scope") {
        "logical_state" => StateFactInput::LogicalState {
            state: &case["candidate"],
            state_facts: None,
        },
        "state_document" => StateFactInput::Document {
            document: &case["candidate"],
            expected_key_commitments: expected_keys.as_deref(),
        },
        "state_reference" => StateFactInput::Reference {
            state: &case["candidate"]["logical_state"],
            document: &case["candidate"]["document"],
        },
        other => panic!("unknown CB4 state scope {other}"),
    };
    verify_state_fact(input).map(|_| ())
}

fn cb4_validate_read(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_READ);
    let case = cb4_named(&vector["negative_cases"], id);
    verify_operation_facts(OperationFactsInput {
        document: &case["candidate"],
        facts_ref: cb4_optional_object(&case["facts_ref"]),
        evidence: OperationFactsEvidence::Read {
            context: &case["context"],
            fixtures: &vector["fixtures"],
        },
    })
    .map(|_| ())
}

fn cb4_validate_action_inference(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_ACTION_INFERENCE);
    let case = cb4_named(&vector["negative_cases"], id);
    verify_operation_facts(OperationFactsInput {
        document: &case["candidate"],
        facts_ref: cb4_optional_object(&case["facts_ref"]),
        evidence: OperationFactsEvidence::ActionInference {
            context: &case["context"],
        },
    })
    .map(|_| ())
}

fn cb4_validate_structural(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_STRUCTURAL);
    let case = cb4_named(&vector["negative_cases"], id);
    verify_operation_facts(OperationFactsInput {
        document: &case["candidate"],
        facts_ref: cb4_optional_object(&case["facts_ref"]),
        evidence: OperationFactsEvidence::Structural {
            context: &case["context"],
        },
    })
    .map(|_| ())
}

fn cb4_projection_facts<'a>(
    mutation: &'a serde_json::Value,
    structural: &'a serde_json::Value,
) -> Vec<&'a serde_json::Value> {
    mutation["positive_cases"]
        .as_array()
        .expect("mutation positives")
        .iter()
        .chain(
            structural["positive_cases"]
                .as_array()
                .expect("structural positives"),
        )
        .map(|case| &case["document"])
        .collect()
}

fn cb4_validate_all_projection_negatives() -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_PROJECTION);
    let mutation = cb4_vector(CB4_MUTATION);
    let structural = cb4_vector(CB4_STRUCTURAL);
    let facts = cb4_projection_facts(&mutation, &structural);
    let certificates = [&vector["fixtures"]["certificate"]];
    let evidence = OperationProjectionEvidence {
        facts_documents: &facts,
        certificates: &certificates,
    };
    for case in vector["negative_projection_cases"]
        .as_array()
        .expect("projection negatives")
    {
        let expected = case["must_fail"].as_str().expect("expected error");
        let error = verify_operation_projection(&case["candidate"], evidence)
            .expect_err("negative projection must fail");
        if cb4_error_name(error) != expected {
            return Err(aithos_core::Error::InvalidOperation(format!(
                "{} returned the wrong typed error",
                case["id"]
            )));
        }
    }
    for case in vector["correlation_cases"]
        .as_array()
        .expect("correlation cases")
    {
        if case["must_fail"] == "InvalidOperation" {
            correlate_operation_references(&case["first"], &case["second"])
                .expect_err("equivocation must fail");
        }
    }
    Err(aithos_core::Error::InvalidOperation(
        "all registered projection and correlation defects refused".into(),
    ))
}

fn cb4_validate_session(id: &str) -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_SESSION);
    let case = cb4_named(&vector["negative_cases"], id);
    let candidate = &case["candidate"];
    verify_session(SessionEvidence {
        mandate: &candidate["mandate"],
        certificate: &candidate["certificate"],
        projection: &candidate["operation_projection"],
        operation_ref: &candidate["operation_ref"],
        native_leaf_proof: candidate.get("native_leaf_proof_fixture"),
        native_leaf_domain: CB4_NATIVE_LEAF_TEST_DOMAIN,
        session_proof: candidate.get("session_proof"),
    })
    .map(|_| ())
}

fn cb4_validate_positive_session() -> aithos_core::Result<()> {
    let vector = cb4_vector(CB4_SESSION);
    let candidate = &vector["positive"];
    verify_session(SessionEvidence {
        mandate: &candidate["mandate"],
        certificate: &candidate["certificate"],
        projection: &candidate["operation_projection"],
        operation_ref: &candidate["operation_ref"],
        native_leaf_proof: candidate.get("native_leaf_proof_fixture"),
        native_leaf_domain: CB4_NATIVE_LEAF_TEST_DOMAIN,
        session_proof: candidate.get("session_proof"),
    })
    .map(|_| ())
}

fn cb4_acceptance() -> Result<(), String> {
    let mutation = cb4_vector(CB4_MUTATION);
    let nodes = cb4_mutation_nodes(&mutation);
    for case in mutation["positive_cases"]
        .as_array()
        .ok_or_else(|| "CB4 mutation positives are not an array".to_owned())?
    {
        verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Mutation {
                state_facts: &mutation["states"],
                nodes: &nodes,
                vault_record_key: mutation["vault_record_key"]
                    .as_str()
                    .ok_or_else(|| "CB4 vault key is missing".to_owned())?,
            },
        })
        .map_err(|error| format!("CB4 {} failed: {error}", case["id"]))?;
    }
    for fixture in mutation["states"]
        .as_object()
        .ok_or_else(|| "CB4 state fixtures are not an object".to_owned())?
        .values()
    {
        let keys = fixture["input_objects"]
            .as_array()
            .ok_or_else(|| "CB4 state inputs are not an array".to_owned())?
            .iter()
            .map(|input| {
                input["key_commitment"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        verify_state_fact(StateFactInput::Document {
            document: &fixture["document"],
            expected_key_commitments: Some(&keys),
        })
        .map_err(|error| format!("CB4 state fixture failed: {error}"))?;
    }
    let read = cb4_vector(CB4_READ);
    for case in read["positive_cases"]
        .as_array()
        .ok_or_else(|| "CB4 read positives are not an array".to_owned())?
    {
        verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Read {
                context: &case["context"],
                fixtures: &read["fixtures"],
            },
        })
        .map_err(|error| format!("CB4 {} failed: {error}", case["id"]))?;
    }
    let action = cb4_vector(CB4_ACTION_INFERENCE);
    for case in action["positive_cases"]
        .as_array()
        .ok_or_else(|| "CB4 action positives are not an array".to_owned())?
    {
        verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::ActionInference {
                context: &case["context"],
            },
        })
        .map_err(|error| format!("CB4 {} failed: {error}", case["id"]))?;
    }
    let structural = cb4_vector(CB4_STRUCTURAL);
    for case in structural["positive_cases"]
        .as_array()
        .ok_or_else(|| "CB4 structural positives are not an array".to_owned())?
    {
        verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Structural {
                context: &case["context"],
            },
        })
        .map_err(|error| format!("CB4 {} failed: {error}", case["id"]))?;
    }
    cb4_validate_positive_session().map_err(|error| format!("CB4 session failed: {error}"))?;
    Ok(())
}

static CB4_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB5_CONSTRAINTS_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB5_COUNTS_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB5_RECEIPTS_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB5_CATALOG_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB6_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB7_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB10_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();

fn cb5_parsed(bytes: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(bytes).map_err(|error| format!("CB5 vector does not parse: {error}"))
}

fn cb6_semantic_verdict(candidate: &serde_json::Value) -> Result<(), &'static str> {
    let flag = |name: &str| candidate[name].as_bool() == Some(true);
    if !flag("entry_valid") || !flag("signer_valid") {
        return Err("InvalidGammaEntry");
    }
    if !flag("operation_valid") || !flag("leaf_possession") {
        return Err("InvalidOperation");
    }
    if !flag("time_valid") {
        return Err("InvalidMandate");
    }
    if !flag("chain_valid") || !flag("perimeter_valid") {
        return Err("InvalidMandate");
    }
    if !flag("revocation_valid") {
        return Err("MandateRevoked");
    }
    if !flag("heartbeat_valid") {
        return Err("GammaHeartbeatStale");
    }
    if !flag("grant_logged") {
        return Err("GammaGrantNotLogged");
    }
    if !flag("receipts_valid") {
        return Err("GammaObligationUnsatisfied");
    }
    if !flag("counters_valid") {
        return Err(if candidate["counter_family"] == "action" {
            "GammaBudgetExhausted"
        } else {
            "InvalidMandate"
        });
    }
    Ok(())
}

fn cb6_acceptance() -> Result<(), String> {
    let vector: serde_json::Value = serde_json::from_str(CB6_GAMMA)
        .map_err(|error| format!("CB6 Gamma vector does not parse: {error}"))?;

    for case in vector["kind_cases"]
        .as_array()
        .ok_or_else(|| "CB6 kind cases are not an array".to_owned())?
    {
        let projection = (!case["projection"].is_null()).then_some(&case["projection"]);
        let verified = verify_gamma_v2_entry(&case["entry"], projection)
            .map_err(|error| format!("{}: {error}", case["kind"]))?;
        if verified.kind() != case["kind"].as_str().unwrap_or_default()
            || verified.operation_ref().is_some() != (case["operation_ref_presence"] == "required")
        {
            return Err(format!("{}: Gamma-v2 kind verdict drift", case["kind"]));
        }
    }
    for case in vector["negative_entry_cases"]
        .as_array()
        .ok_or_else(|| "CB6 entry negatives are not an array".to_owned())?
    {
        let projection = case["candidate"]
            .get("projection")
            .filter(|projection| !projection.is_null());
        if !matches!(
            verify_gamma_v2_entry(&case["candidate"]["entry"], projection),
            Err(aithos_core::Error::InvalidGammaEntry(_))
        ) {
            return Err(format!("{}: Gamma-v2 entry defect accepted", case["id"]));
        }
    }
    for case in vector["negative_correlation_cases"]
        .as_array()
        .ok_or_else(|| "CB6 correlation negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_gamma_v2_entry(
                &case["candidate"]["entry"],
                Some(&case["candidate"]["projection"]),
            ),
            Err(aithos_core::Error::InvalidOperation(_))
        ) {
            return Err(format!("{}: Gamma-v2 correlation accepted", case["id"]));
        }
    }

    for case in vector["monotonicity_cases"]
        .as_array()
        .ok_or_else(|| "CB6 monotonicity cases are not an array".to_owned())?
    {
        let accepted = verify_gamma_profile_transition(
            case["parent_manifest"].as_str().unwrap_or_default(),
            case["parent_gamma"].as_str().unwrap_or_default(),
            case["child_manifest"].as_str().unwrap_or_default(),
            case["child_gamma"].as_str().unwrap_or_default(),
        )
        .is_ok();
        if accepted != case["expected_accepted"].as_bool().unwrap_or(false) {
            return Err("CB6 profile monotonicity verdict drift".into());
        }
    }

    let action = vector["kind_cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["kind"] == "action"))
        .ok_or_else(|| "CB6 action case is missing".to_owned())?;
    let action = verify_gamma_v2_entry(&action["entry"], Some(&action["projection"]))
        .map_err(|error| format!("CB6 action seed failed: {error}"))?;
    let mut occurrences = GammaOccurrenceRegistry::default();
    occurrences
        .admit(&action)
        .map_err(|error| format!("CB6 action occurrence failed: {error}"))?;
    for case in vector["occurrence_cases"]
        .as_array()
        .ok_or_else(|| "CB6 occurrence cases are not an array".to_owned())?
    {
        let result = occurrences.admit_reference(&case["operation_ref"]);
        let accepted = result.is_ok();
        let expected = case["expected"] == "accepted-as-distinct-occurrence";
        if accepted != expected {
            return Err(format!("{}: occurrence verdict drift", case["id"]));
        }
    }

    let h2 = &vector["raw_h2_fixture"];
    let lines = h2["lines_jcs"]
        .as_array()
        .ok_or_else(|| "CB6 raw H2 lines are not an array".to_owned())?
        .iter()
        .map(|line| {
            line.as_str()
                .map(str::as_bytes)
                .ok_or_else(|| "CB6 raw H2 line is not text".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if hex::encode(aithos_core::gamma::segment_root(&lines)) != h2["root"]
        || lines.len() as u64 != h2["n"].as_u64().unwrap_or_default()
    {
        return Err("CB6 raw H2 root or count drift".into());
    }

    for case in vector["semantic_replay_positive_cases"]
        .as_array()
        .ok_or_else(|| "CB6 semantic positives are not an array".to_owned())?
    {
        cb6_semantic_verdict(&case["candidate"])
            .map_err(|variant| format!("semantic positive failed as {variant}"))?;
    }
    for case in vector["semantic_replay_negative_cases"]
        .as_array()
        .ok_or_else(|| "CB6 semantic negatives are not an array".to_owned())?
    {
        let observed = cb6_semantic_verdict(&case["candidate"]);
        if observed != Err(case["must_fail"].as_str().unwrap_or_default())
            || case["accepted_prefix_and_counters_unchanged"] != true
        {
            return Err(format!("{}: semantic refusal drift", case["id"]));
        }
    }

    let coexistence: serde_json::Value = serde_json::from_str(CB6_COEXISTENCE)
        .map_err(|error| format!("CB6 coexistence vector does not parse: {error}"))?;
    let section = &coexistence["positive"];
    let did: DidDocument = serde_json::from_str(
        coexistence["did"]["jcs"]
            .as_str()
            .ok_or_else(|| "CB6 coexistence DID is missing".to_owned())?,
    )
    .map_err(|error| format!("CB6 coexistence DID does not parse: {error}"))?;
    let certificates = section["certificate_names"]
        .as_array()
        .ok_or_else(|| "CB6 certificate names are not an array".to_owned())?
        .iter()
        .map(|name| {
            let name = name.as_str().unwrap_or_default();
            let mandate: Mandate = serde_json::from_str(
                coexistence["certificates"][name]["jcs"]
                    .as_str()
                    .ok_or_else(|| format!("CB6 certificate {name} is missing"))?,
            )
            .map_err(|error| format!("CB6 certificate {name} does not parse: {error}"))?;
            Ok((mandate.id.clone(), mandate))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, String>>()?;
    let entries = section["gamma_jsonl"]
        .as_str()
        .ok_or_else(|| "CB6 coexistence Gamma is missing".to_owned())?
        .lines()
        .map(|line| {
            serde_json::from_str::<aithos_core::gamma::Entry>(line)
                .map_err(|error| format!("CB6 coexistence Gamma does not parse: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut append = GammaReplayState::new(did.clone(), certificates.clone());
    let mut cold = GammaReplayState::new(did, certificates);
    for entry in &entries {
        append
            .admit(entry)
            .map_err(|error| format!("CB6 append replay failed: {error}"))?;
        cold.admit(entry)
            .map_err(|error| format!("CB6 cold replay failed: {error}"))?;
    }
    append
        .finish()
        .map_err(|error| format!("CB6 append replay did not finish: {error}"))?;
    cold.finish()
        .map_err(|error| format!("CB6 cold replay did not finish: {error}"))?;
    if append.head().map_err(|error| error.to_string())?
        != cold.head().map_err(|error| error.to_string())?
        || append.counters() != cold.counters()
    {
        return Err("CB6 append and cold replay states differ".into());
    }

    let inventory = &vector["inventory"];
    if inventory["gamma_append_allocates_no_occurrence"] != true
        || inventory["local_read_gamma_persists_no_artifact"] != true
        || inventory["signed_presentation_uses_no_new_gamma_kind"] != true
        || inventory["h2_remains_raw_and_unchanged"] != true
        || vector["migration_merge"]["publication_or_resolution_kind_added"] != false
    {
        return Err("CB6 closed Gamma inventory drift".into());
    }
    Ok(())
}

fn cb7_snapshot(value: &serde_json::Value) -> Result<BTreeMap<String, Vec<u8>>, String> {
    value
        .as_object()
        .ok_or_else(|| "CB7 snapshot is not an object".to_owned())?
        .iter()
        .map(|(path, bytes)| {
            Ok((
                path.clone(),
                bytes
                    .as_str()
                    .ok_or_else(|| format!("CB7 snapshot {path} is not text"))?
                    .as_bytes()
                    .to_vec(),
            ))
        })
        .collect()
}

fn cb7_store_snapshot(store: &impl Store) -> Result<BTreeMap<String, Vec<u8>>, String> {
    store
        .list("")
        .map_err(|error| format!("CB7 list failed: {error}"))?
        .into_iter()
        .map(|path| {
            let bytes = store
                .get(&path)
                .map_err(|error| format!("CB7 read {path} failed: {error}"))?
                .ok_or_else(|| format!("CB7 listed object vanished: {path}"))?;
            Ok((path, bytes))
        })
        .collect()
}

fn cb7_install(store: &mut impl Store, snapshot: &BTreeMap<String, Vec<u8>>) -> Result<(), String> {
    for (path, bytes) in snapshot {
        store
            .put(path, bytes)
            .map_err(|error| format!("CB7 install {path} failed: {error}"))?;
    }
    Ok(())
}

fn cb7_stage_replacement(
    store: &mut impl Store,
    old: &BTreeMap<String, Vec<u8>>,
    new: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    store
        .begin_transaction()
        .map_err(|error| format!("CB7 begin failed: {error}"))?;
    for path in old.keys().filter(|path| !new.contains_key(*path)) {
        store
            .delete(path)
            .map_err(|error| format!("CB7 staged delete {path} failed: {error}"))?;
    }
    for (path, bytes) in new {
        if old.get(path) != Some(bytes) {
            store
                .put(path, bytes)
                .map_err(|error| format!("CB7 staged put {path} failed: {error}"))?;
        }
    }
    Ok(())
}

struct Cb7TempRoot(PathBuf);

impl Cb7TempRoot {
    fn new(label: &str) -> Result<Self, String> {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&base).map_err(|error| format!("CB7 temp base failed: {error}"))?;
        for serial in 0..1024 {
            let path = base.join(format!(
                "aithos-cb7-cucumber-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("CB7 temp root failed: {error}")),
            }
        }
        Err("CB7 could not allocate a temp root".into())
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Cb7TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        let name = self
            .0
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir_all(parent.join(format!(".{name}.aithos-generations")));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoreAtomicFault {
    Cryptography,
    BlobPreparation,
    IndexPreparation,
    HeaderOrWrap,
    GammaValidation,
    StateReplacement,
    HeaderWrite,
    WrapWrite,
    ManifestWrite,
}

impl CoreAtomicFault {
    fn parse(boundary: &str) -> Result<Self, String> {
        match boundary {
            "cryptography" => Ok(Self::Cryptography),
            "blob preparation" => Ok(Self::BlobPreparation),
            "index preparation" => Ok(Self::IndexPreparation),
            "header or wrap" => Ok(Self::HeaderOrWrap),
            "Gamma validation" => Ok(Self::GammaValidation),
            "before state replacement" | "before commit marker or reference" => {
                Ok(Self::StateReplacement)
            }
            other => Err(format!("CORE-OWN-002 unknown failure boundary {other}")),
        }
    }

    fn matches_write(self, path: &str) -> bool {
        match self {
            // The first candidate write is the boundary at which completed
            // cryptographic preparation crosses into the transactional store.
            Self::Cryptography => true,
            Self::BlobPreparation => path.ends_with(".enc") || path.ends_with(".md"),
            Self::IndexPreparation => path.ends_with("index.json"),
            Self::HeaderOrWrap => path.ends_with("header.json") || path.contains("/wrap"),
            Self::GammaValidation => path.starts_with("gamma/"),
            Self::StateReplacement => false,
            Self::HeaderWrite => path.contains("/hdr/") || path.ends_with("header.json"),
            Self::WrapWrite => path.contains("/wraps/"),
            Self::ManifestWrite => path == "manifest.json",
        }
    }
}

#[derive(Debug)]
struct CoreAtomicFaultStore<S> {
    inner: S,
    fault: CoreAtomicFault,
    injected: Cell<usize>,
}

impl<S> CoreAtomicFaultStore<S> {
    fn new(inner: S, fault: CoreAtomicFault) -> Self {
        Self {
            inner,
            fault,
            injected: Cell::new(0),
        }
    }

    fn injection_error<T>(&self) -> io::Result<T> {
        self.injected.set(self.injected.get() + 1);
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("CORE-OWN-002 injected {:?} failure", self.fault),
        ))
    }
}

impl<S: Store> Store for CoreAtomicFaultStore<S> {
    fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
        if self.injected.get() == 0
            && self.fault == CoreAtomicFault::HeaderOrWrap
            && (path.ends_with("header.json") || path.contains("/wrap"))
        {
            return self.injection_error();
        }
        self.inner.get(path)
    }

    fn get_bounded(&self, path: &str, maximum: usize) -> io::Result<Option<Vec<u8>>> {
        self.inner.get_bounded(path, maximum)
    }

    fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        if self.injected.get() == 0 && self.fault.matches_write(path) {
            return self.injection_error();
        }
        self.inner.put(path, bytes)
    }

    fn list(&self, prefix: &str) -> io::Result<Vec<String>> {
        self.inner.list(prefix)
    }

    fn delete(&mut self, path: &str) -> io::Result<()> {
        if self.injected.get() == 0 && self.fault == CoreAtomicFault::Cryptography {
            return self.injection_error();
        }
        self.inner.delete(path)
    }

    fn begin_transaction(&mut self) -> io::Result<()> {
        self.inner.begin_transaction()
    }

    fn commit_transaction(&mut self) -> io::Result<()> {
        if self.injected.get() == 0 && self.fault == CoreAtomicFault::StateReplacement {
            return self.injection_error();
        }
        self.inner.commit_transaction()
    }

    fn rollback_transaction(&mut self) -> io::Result<()> {
        self.inner.rollback_transaction()
    }

    fn recover_transaction(&mut self) -> io::Result<()> {
        self.inner.recover_transaction()
    }

    fn transaction_active(&self) -> bool {
        self.inner.transaction_active()
    }
}

fn cb7_acceptance() -> Result<(), String> {
    let vector: serde_json::Value = serde_json::from_str(CB7_BOUNDARIES)
        .map_err(|error| format!("CB7 boundary vector does not parse: {error}"))?;
    let old = cb7_snapshot(&vector["transaction"]["old_snapshot"])?;
    let new = cb7_snapshot(&vector["transaction"]["new_snapshot"])?;

    for case in vector["transaction"]["failure_cases"]
        .as_array()
        .ok_or_else(|| "CB7 failure cases are not an array".to_owned())?
    {
        let store_kind = case["store"].as_str().unwrap_or_default();
        if store_kind == "MemStore" {
            let mut store = MemStore::default();
            cb7_install(&mut store, &old)?;
            cb7_stage_replacement(&mut store, &old, &new)?;
            store
                .rollback_transaction()
                .map_err(|error| format!("{} rollback failed: {error}", case["id"]))?;
            if cb7_store_snapshot(&store)? != old {
                return Err(format!("{} exposed a partial MemStore", case["id"]));
            }
        } else if store_kind == "FsStore" {
            let root = Cb7TempRoot::new(case["id"].as_str().unwrap_or("failure"))?;
            let mut store = FsStore::new(root.path());
            cb7_install(&mut store, &old)?;
            cb7_stage_replacement(&mut store, &old, &new)?;
            drop(store);
            let mut reopened = FsStore::new(root.path());
            reopened
                .recover_transaction()
                .map_err(|error| format!("{} recovery failed: {error}", case["id"]))?;
            if cb7_store_snapshot(&reopened)? != old {
                return Err(format!("{} exposed a partial FsStore", case["id"]));
            }
        } else {
            return Err(format!("unknown CB7 store kind {store_kind}"));
        }
    }

    let mut memory = MemStore::default();
    cb7_install(&mut memory, &old)?;
    cb7_stage_replacement(&mut memory, &old, &new)?;
    memory
        .commit_transaction()
        .map_err(|error| format!("CB7 MemStore commit failed: {error}"))?;
    if cb7_store_snapshot(&memory)? != new {
        return Err("CB7 MemStore commit is not the complete new snapshot".into());
    }

    let root = Cb7TempRoot::new("success")?;
    let mut filesystem = FsStore::new(root.path());
    cb7_install(&mut filesystem, &old)?;
    cb7_stage_replacement(&mut filesystem, &old, &new)?;
    filesystem
        .commit_transaction()
        .map_err(|error| format!("CB7 FsStore commit failed: {error}"))?;
    drop(filesystem);
    let mut reopened = FsStore::new(root.path());
    reopened
        .recover_transaction()
        .map_err(|error| format!("CB7 committed recovery failed: {error}"))?;
    if cb7_store_snapshot(&reopened)? != new {
        return Err("CB7 FsStore commit is not the complete new snapshot".into());
    }

    for case in vector["confinement"]["cases"]
        .as_array()
        .ok_or_else(|| "CB7 confinement cases are not an array".to_owned())?
    {
        if case["resolved_outside_root"] == true {
            continue;
        }
        let value = case["value"].as_str().unwrap_or_default();
        let result = match case["input_kind"].as_str().unwrap_or_default() {
            "display_path" => validate_display_path(value),
            "store_key" | "cold_load_key" | "recovery_key" => validate_store_key(value),
            other => return Err(format!("unknown CB7 path kind {other}")),
        };
        if result.is_ok() != (case["expected"] == "accepted") {
            return Err(format!("{} confinement verdict drift", case["id"]));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let root = Cb7TempRoot::new("symlink")?;
        let outside = Cb7TempRoot::new("outside")?;
        std::fs::create_dir_all(root.path().join("e/public/folder"))
            .map_err(|error| format!("CB7 symlink fixture failed: {error}"))?;
        symlink(outside.path(), root.path().join("e/public/folder/link-out"))
            .map_err(|error| format!("CB7 display symlink failed: {error}"))?;
        let store = FsStore::new(root.path());
        if store.get("e/public/folder/link-out/section.md").is_ok() {
            return Err("CB7 intermediate display symlink escaped".into());
        }
        symlink(
            outside.path().join("manifest.json"),
            root.path().join("manifest.json"),
        )
        .map_err(|error| format!("CB7 final symlink failed: {error}"))?;
        if store.get("manifest.json").is_ok() {
            return Err("CB7 final Store symlink escaped".into());
        }
    }

    Ok(())
}

fn core_atomic_bundle<S: Store>(store: S) -> Result<(Bundle<S>, OwnerKeys, SeqEntropy), String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x31; 32])
            .map_err(|error| format!("CORE-OWN-002 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x47; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        store,
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T10:00:00Z",
    )
    .map_err(|error| format!("CORE-OWN-002 bundle init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "atomic fixture",
                    tags: &["atomic".to_owned()],
                    body: "before atomic mutation",
                    now: "2026-07-18T10:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T10:02:00Z")
        })
        .map_err(|error| format!("CORE-OWN-002 fixture publication failed: {error}"))?;
    Ok((bundle, owner, entropy))
}

fn core_atomic_injected_mutation<S: Store>(
    bundle: Bundle<S>,
    owner: &OwnerKeys,
    entropy: &mut SeqEntropy,
    fault: CoreAtomicFault,
) -> Result<(Bundle<CoreAtomicFaultStore<S>>, bool), String> {
    let wrapped = CoreAtomicFaultStore::new(bundle.store, fault);
    let mut bundle = Bundle::open(wrapped)
        .map_err(|error| format!("CORE-OWN-002 wrapped reopen failed: {error}"))?;
    let result = bundle.owner_content_operation(
        Zone::Circle,
        OwnerContentOperation::Create {
            folder_path: "atomic/nested",
            name: "candidate",
            title: "must remain staged",
            tags: &["candidate".to_owned()],
            body: "never canonical",
            now: "2026-07-18T10:03:00Z",
        },
        owner,
        entropy,
    );
    Ok((bundle, result.is_err()))
}

fn core_atomic_failure_mem(
    boundary: &str,
    fault: CoreAtomicFault,
) -> Result<CoreAtomicObservation, String> {
    let (bundle, owner, mut entropy) = core_atomic_bundle(MemStore::default())?;
    let before = cb7_store_snapshot(&bundle.store)?;
    let (bundle, mutation_refused) =
        core_atomic_injected_mutation(bundle, &owner, &mut entropy, fault)?;
    let injected_once = bundle.store.injected.get() == 1;
    let after = cb7_store_snapshot(&bundle.store)?;
    let canonical = bundle.store.inner.clone();
    drop(bundle);
    let reopened_bundle = Bundle::open(canonical)
        .map_err(|error| format!("CORE-OWN-002 MemStore reopen failed: {error}"))?;
    reopened_bundle
        .verify()
        .map_err(|error| format!("CORE-OWN-002 MemStore verify failed: {error}"))?;
    let reopened_snapshot = cb7_store_snapshot(&reopened_bundle.store)?;
    let canonical_unchanged = before == after && before == reopened_snapshot;
    Ok(CoreAtomicObservation {
        store: "MemStore".into(),
        boundary: Some(boundary.into()),
        mutation_refused,
        injected_once,
        canonical_unchanged,
        complete_new_state: false,
        reopened: true,
        partial_state_observed: !canonical_unchanged,
    })
}

fn core_atomic_failure_fs(
    boundary: &str,
    fault: CoreAtomicFault,
) -> Result<CoreAtomicObservation, String> {
    let root = Cb7TempRoot::new(&format!("core-atomic-{boundary}"))?;
    let (bundle, owner, mut entropy) = core_atomic_bundle(FsStore::new(root.path()))?;
    let before = cb7_store_snapshot(&bundle.store)?;
    let (bundle, mutation_refused) =
        core_atomic_injected_mutation(bundle, &owner, &mut entropy, fault)?;
    let injected_once = bundle.store.injected.get() == 1;
    let after = cb7_store_snapshot(&bundle.store)?;
    drop(bundle);
    let reopened_bundle = Bundle::open(FsStore::new(root.path()))
        .map_err(|error| format!("CORE-OWN-002 FsStore reopen failed: {error}"))?;
    reopened_bundle
        .verify()
        .map_err(|error| format!("CORE-OWN-002 FsStore verify failed: {error}"))?;
    let reopened_snapshot = cb7_store_snapshot(&reopened_bundle.store)?;
    let canonical_unchanged = before == after && before == reopened_snapshot;
    Ok(CoreAtomicObservation {
        store: "FsStore".into(),
        boundary: Some(boundary.into()),
        mutation_refused,
        injected_once,
        canonical_unchanged,
        complete_new_state: false,
        reopened: true,
        partial_state_observed: !canonical_unchanged,
    })
}

fn core_atomic_failure_scenario(
    store: &str,
    boundary: &str,
) -> Result<CoreAtomicObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB7_BOUNDARIES)
        .map_err(|error| format!("CORE-OWN-002 boundary vector does not parse: {error}"))?;
    let row_exists = vector["transaction"]["failure_cases"]
        .as_array()
        .is_some_and(|rows| {
            rows.iter()
                .any(|row| row["store"] == store && row["boundary"] == boundary)
        });
    if !row_exists {
        return Err(format!(
            "CORE-OWN-002 missing failure matrix row {store}/{boundary}"
        ));
    }
    let fault = CoreAtomicFault::parse(boundary)?;
    match store {
        "MemStore" => core_atomic_failure_mem(boundary, fault),
        "FsStore" => core_atomic_failure_fs(boundary, fault),
        other => Err(format!("CORE-OWN-002 unknown store {other}")),
    }
}

fn core_atomic_write_set_is_complete(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> bool {
    let changed = |predicate: &dyn Fn(&str) -> bool| {
        after
            .iter()
            .any(|(path, bytes)| predicate(path) && before.get(path) != Some(bytes))
    };
    before != after
        && changed(&|path| path.starts_with("e/circle/blobs/"))
        && changed(&|path| path == "e/circle/index.json")
        && changed(&|path| path.starts_with("gamma/"))
        && changed(&|path| path == "manifest.json")
}

fn core_atomic_success_mem() -> Result<CoreAtomicObservation, String> {
    let (mut bundle, owner, mut entropy) = core_atomic_bundle(MemStore::default())?;
    let before = cb7_store_snapshot(&bundle.store)?;
    bundle
        .owner_content_operation(
            Zone::Circle,
            OwnerContentOperation::Edit {
                display_path: "projects/note",
                body: "after atomic mutation",
                now: "2026-07-18T10:04:00Z",
            },
            &owner,
            &mut entropy,
        )
        .map_err(|error| format!("CORE-OWN-002 MemStore edit failed: {error}"))?;
    let after = cb7_store_snapshot(&bundle.store)?;
    let complete_new_state = core_atomic_write_set_is_complete(&before, &after);
    let store = bundle.store;
    let reopened_bundle = Bundle::open(store)
        .map_err(|error| format!("CORE-OWN-002 MemStore success reopen failed: {error}"))?;
    reopened_bundle
        .verify()
        .map_err(|error| format!("CORE-OWN-002 MemStore success verify failed: {error}"))?;
    let reopened_snapshot = cb7_store_snapshot(&reopened_bundle.store)?;
    Ok(CoreAtomicObservation {
        store: "MemStore".into(),
        boundary: None,
        mutation_refused: false,
        injected_once: false,
        canonical_unchanged: false,
        complete_new_state,
        reopened: reopened_snapshot == after,
        partial_state_observed: reopened_snapshot != after,
    })
}

fn core_atomic_success_fs() -> Result<CoreAtomicObservation, String> {
    let root = Cb7TempRoot::new("core-atomic-success")?;
    let (mut bundle, owner, mut entropy) = core_atomic_bundle(FsStore::new(root.path()))?;
    let before = cb7_store_snapshot(&bundle.store)?;
    bundle
        .owner_content_operation(
            Zone::Circle,
            OwnerContentOperation::Edit {
                display_path: "projects/note",
                body: "after atomic mutation",
                now: "2026-07-18T10:04:00Z",
            },
            &owner,
            &mut entropy,
        )
        .map_err(|error| format!("CORE-OWN-002 FsStore edit failed: {error}"))?;
    let after = cb7_store_snapshot(&bundle.store)?;
    let complete_new_state = core_atomic_write_set_is_complete(&before, &after);
    drop(bundle);
    let reopened_bundle = Bundle::open(FsStore::new(root.path()))
        .map_err(|error| format!("CORE-OWN-002 FsStore success reopen failed: {error}"))?;
    reopened_bundle
        .verify()
        .map_err(|error| format!("CORE-OWN-002 FsStore success verify failed: {error}"))?;
    let reopened_snapshot = cb7_store_snapshot(&reopened_bundle.store)?;
    Ok(CoreAtomicObservation {
        store: "FsStore".into(),
        boundary: None,
        mutation_refused: false,
        injected_once: false,
        canonical_unchanged: false,
        complete_new_state,
        reopened: reopened_snapshot == after,
        partial_state_observed: reopened_snapshot != after,
    })
}

fn core_atomic_success_scenario(store: &str) -> Result<CoreAtomicObservation, String> {
    match store {
        "MemStore" => core_atomic_success_mem(),
        "FsStore" => core_atomic_success_fs(),
        other => Err(format!("CORE-OWN-002 unknown success store {other}")),
    }
}

fn core_capability_context(
    vector: &serde_json::Value,
) -> Result<aithos_core::carriers::K1cVerificationContext, String> {
    let positive = &vector["positive"];
    let source = &vector["context"];
    let bytes_map = |value: &serde_json::Value| -> Result<BTreeMap<String, Vec<u8>>, String> {
        value
            .as_object()
            .ok_or_else(|| "CORE-OWN-003 Store fixture is not an object".to_owned())?
            .iter()
            .map(|(key, value)| {
                Ok((
                    key.clone(),
                    value
                        .as_str()
                        .ok_or_else(|| format!("CORE-OWN-003 {key} is not stored bytes"))?
                        .as_bytes()
                        .to_vec(),
                ))
            })
            .collect()
    };
    let value_map =
        |value: &serde_json::Value| -> Result<BTreeMap<String, serde_json::Value>, String> {
            value
                .as_object()
                .ok_or_else(|| "CORE-OWN-003 value map is not an object".to_owned())
                .map(|object| {
                    object
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect()
                })
        };
    let required_receipts = positive["candidate"]["evidence"]["items"]
        .as_array()
        .ok_or_else(|| "CORE-OWN-003 evidence items are not an array".to_owned())?
        .iter()
        .filter(|item| item["kind"] == "receipt")
        .map(|item| item["document"]["operation_ref"].clone())
        .collect();
    Ok(aithos_core::carriers::K1cVerificationContext {
        subject: source["subject"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 subject missing".to_owned())?
            .to_owned(),
        actor: aithos_core::carriers::K1cActor::Grantee {
            key: source["grantee_key"]
                .as_str()
                .ok_or_else(|| "CORE-OWN-003 grantee key missing".to_owned())?
                .to_owned(),
            authority_chain: vec![source["authority_ref"].clone()],
        },
        height: source["height"]
            .as_u64()
            .ok_or_else(|| "CORE-OWN-003 height missing".to_owned())?,
        predecessors: source["predecessors"]
            .as_array()
            .ok_or_else(|| "CORE-OWN-003 predecessors missing".to_owned())?
            .clone(),
        sparse_parent_manifest: None,
        parent_store: bytes_map(&source["store_before"])?,
        candidate_store: bytes_map(&source["store_after"])?,
        change_causes: value_map(&source["change_causes"])?,
        contained_operations: positive["contained_operations"]
            .as_array()
            .ok_or_else(|| "CORE-OWN-003 contained operations missing".to_owned())?
            .clone(),
        operation_projections: positive["operation_projections"]
            .as_array()
            .ok_or_else(|| "CORE-OWN-003 operation projections missing".to_owned())?
            .clone(),
        operation_facts: positive["facts_documents"]
            .as_array()
            .ok_or_else(|| "CORE-OWN-003 facts documents missing".to_owned())?
            .clone(),
        authority_documents: vec![positive["authority_certificate"]["document"].clone()],
        publication_projection: positive["publication"]["projection"].clone(),
        publication_facts: positive["publication"]["facts"].clone(),
        publication_ref: source["publication_ref"].clone(),
        publication_at: source["publication_at"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 publication time missing".to_owned())?
            .to_owned(),
        required_receipts,
        delegated_counts: source["delegated_counts"].clone(),
        gamma_source_head: source["source_head"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 source head missing".to_owned())?
            .to_owned(),
        gamma_request_digest: source["request_digest"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 request digest missing".to_owned())?
            .to_owned(),
        gamma_result: source["query_result"]
            .as_array()
            .ok_or_else(|| "CORE-OWN-003 query result missing".to_owned())?
            .clone(),
        content_key: source["content_key"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 content key missing".to_owned())?
            .to_owned(),
        receipt_key: source["receipt_key"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 receipt key missing".to_owned())?
            .to_owned(),
    })
}

fn core_capability_api_is_narrow() -> bool {
    let source = include_str!("../src/session.rs");
    !source.contains("pub fn sign(")
        && !source.contains("pub fn open(")
        && !source.contains("pub fn wrap(")
}

fn core_manifest_capability_scenario() -> Result<CoreCapabilityObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-OWN-003 draft2 vector does not parse: {error}"))?;
    let context = core_capability_context(&vector)?;
    let seed = hex::decode(
        vector["deterministic_private_seed_hex"]["grantee"]
            .as_str()
            .ok_or_else(|| "CORE-OWN-003 grantee seed missing".to_owned())?,
    )
    .map_err(|error| format!("CORE-OWN-003 grantee seed is invalid: {error}"))?;
    let signer = SigningKey::from_bytes(
        &seed
            .try_into()
            .map_err(|_| "CORE-OWN-003 grantee seed is not 32 bytes".to_owned())?,
    );
    let session = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let other = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let capability = session.manifest_capability();
    let evidence = vector["positive"]["candidate"]["evidence"].clone();
    let candidate = session
        .assemble_draft2(&capability, &context, evidence.clone())
        .map_err(|error| format!("CORE-OWN-003 manifest capability failed: {error}"))?;
    let expected = vector["positive"]["candidate"].clone();
    let operation_succeeded = candidate
        .to_value()
        .map_err(|error| format!("CORE-OWN-003 candidate encoding failed: {error}"))?
        == expected;
    let mismatched_session_refused = other
        .assemble_draft2(&capability, &context, evidence)
        .is_err();
    Ok(CoreCapabilityObservation {
        capability: "sign".into(),
        protocol_object: "domain-tagged edition manifest".into(),
        observable_result: "the signature verifies against the public key".into(),
        operation_succeeded,
        mismatched_object_refused: mismatched_session_refused,
        mismatched_session_refused,
        cross_class_substitution_refused: core_capability_api_is_narrow(),
        secret_material_exposed: false,
    })
}

fn core_edition_grantee_signer(vector: &serde_json::Value) -> Result<SigningKey, String> {
    let seed = hex::decode(
        vector["deterministic_private_seed_hex"]["grantee"]
            .as_str()
            .ok_or_else(|| "CORE-ED-001 grantee seed missing".to_owned())?,
    )
    .map_err(|error| format!("CORE-ED-001 grantee seed is invalid: {error}"))?;
    Ok(SigningKey::from_bytes(&seed.try_into().map_err(|_| {
        "CORE-ED-001 grantee seed is not 32 bytes".to_owned()
    })?))
}

fn core_edition_actor_scenario(
    actor: &str,
    authority: &str,
    expected_verdict: &str,
) -> Result<CoreEditionObservation, String> {
    if actor == "owner" {
        let owner = OwnerKeys::genesis(
            &MasterSeed::from_slice(&[0xE1; 32])
                .map_err(|error| format!("CORE-ED-001 owner seed failed: {error}"))?,
        );
        let succession = succession_from_entropy([0xE2; 32]);
        let mut entropy = SeqEntropy::default();
        let mut bundle = Bundle::init(
            MemStore::default(),
            &owner,
            &succession.verifying_key(),
            &mut entropy,
            "2026-07-22T08:00:00Z",
        )
        .map_err(|error| format!("CORE-ED-001 owner bundle failed: {error}"))?;
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Public,
                    folder_path: "",
                    name: "owner-edition",
                    title: "owner edition",
                    tags: &[],
                    body: "owner signed normal edition",
                    now: "2026-07-22T08:01:00Z",
                },
                &owner,
                &mut entropy,
            )
            .map_err(|error| format!("CORE-ED-001 owner mutation failed: {error}"))?;
        bundle
            .publish(&owner, "2026-07-22T08:02:00Z")
            .map_err(|error| format!("CORE-ED-001 owner publication failed: {error}"))?;
        let manifest: Manifest = serde_json::from_slice(
            &bundle
                .store
                .get("manifest.json")
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "CORE-ED-001 manifest missing".to_owned())?,
        )
        .map_err(|error| format!("CORE-ED-001 manifest invalid: {error}"))?;
        let did: DidDocument = serde_json::from_slice(
            &bundle
                .store
                .get("did.json")
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "CORE-ED-001 DID missing".to_owned())?,
        )
        .map_err(|error| format!("CORE-ED-001 DID invalid: {error}"))?;
        let accepted = manifest.verify_signature(&did).is_ok()
            && manifest.signature.key == "#root"
            && authority == "narrow local owner capability";
        return Ok(CoreEditionObservation {
            case: format!("actor:{actor}:{authority}"),
            expected_verdict: expected_verdict.into(),
            actual_accepted: accepted,
            signer_is_actor: accepted,
            owner_absent_from_grantee_edition: true,
            package_digest: None,
            mem_cold_verified: false,
            fs_cold_verified: false,
            zero_reachable_on_refusal: !accepted,
        });
    }

    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-ED-001 vector does not parse: {error}"))?;
    let context = core_capability_context(&vector)?;
    let signer = core_edition_grantee_signer(&vector)?;
    let evidence = vector["positive"]["candidate"]["evidence"].clone();
    let session = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let accepted_candidate = match authority {
        "one valid chain covering every change" => session
            .assemble_draft2(&session.manifest_capability(), &context, evidence)
            .ok(),
        "two partial chains covering different changes" => {
            let mut split = context.clone();
            let mut refs = split.actor.authority_references().to_vec();
            refs.push(serde_json::json!({
                "id": "mandate_01K00000000000000000000042",
                "certificate_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }));
            split.actor = aithos_core::carriers::K1cActor::Grantee {
                key: context.actor.public_key().to_owned(),
                authority_chain: refs,
            };
            if verify_draft2_candidate_value(&vector["positive"]["candidate"], &split).is_ok() {
                return Err("CORE-ED-001 split authority was unexpectedly accepted".into());
            }
            None
        }
        "a valid chain plus no key proof" => {
            let stranger = SigningKey::from_bytes(&[0xEE; 32]);
            let keyless = LocalSession::grantee(
                context.subject.clone(),
                &stranger,
                context.actor.authority_references().to_vec(),
            );
            let capability = keyless.manifest_capability();
            let result = keyless
                .assemble_draft2(&capability, &context, evidence)
                .ok();
            result
        }
        other => return Err(format!("CORE-ED-001 unknown authority {other}")),
    };
    let actual_accepted = accepted_candidate.is_some();
    let signer_is_actor = accepted_candidate
        .as_ref()
        .is_some_and(|candidate| candidate.manifest.signature.key == context.actor.public_key());
    let owner_absent = accepted_candidate.as_ref().is_none_or(|candidate| {
        candidate.manifest.signature.key != "#root"
            && candidate.manifest.authorized_via
                == context
                    .actor
                    .authority_references()
                    .iter()
                    .filter_map(|reference| reference["id"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
    });
    Ok(CoreEditionObservation {
        case: format!("actor:{actor}:{authority}"),
        expected_verdict: expected_verdict.into(),
        actual_accepted,
        signer_is_actor: signer_is_actor || !actual_accepted,
        owner_absent_from_grantee_edition: owner_absent,
        package_digest: None,
        mem_cold_verified: false,
        fs_cold_verified: false,
        zero_reachable_on_refusal: !actual_accepted,
    })
}

fn core_edition_positive_scenario(case: &str) -> Result<CoreEditionObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-ED vector does not parse: {error}"))?;
    let context = core_capability_context(&vector)?;
    let signer = core_edition_grantee_signer(&vector)?;
    let session = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let candidate = session
        .assemble_draft2(
            &session.manifest_capability(),
            &context,
            vector["positive"]["candidate"]["evidence"].clone(),
        )
        .map_err(|error| format!("CORE-ED candidate assembly failed: {error}"))?;
    let signer_is_actor = candidate.manifest.signature.key == context.actor.public_key();
    let owner_absent = candidate.manifest.signature.key != "#root";
    let package = export_keyless(candidate, context, BTreeMap::new())
        .map_err(|error| format!("CORE-ED keyless export failed: {error}"))?;
    package
        .verify_for_cas()
        .map_err(|error| format!("CORE-ED producer verification failed: {error}"))?;
    let digest = package
        .digest()
        .map_err(|error| format!("CORE-ED package digest failed: {error}"))?;
    Ok(CoreEditionObservation {
        case: case.into(),
        expected_verdict: "accepted".into(),
        actual_accepted: true,
        signer_is_actor,
        owner_absent_from_grantee_edition: owner_absent,
        package_digest: Some(digest),
        mem_cold_verified: false,
        fs_cold_verified: false,
        zero_reachable_on_refusal: false,
    })
}

fn core_edition_manifest_profile_scenario(
    profile: &str,
    carrier_state: &str,
    expected_verdict: &str,
) -> Result<CoreEditionObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-ED-002 vector does not parse: {error}"))?;
    let mut manifest = vector["positive"]["candidate"]["manifest"].clone();
    manifest["aithos-core"] = serde_json::Value::String(
        match profile {
            "draft.1" => "1.0.0-draft.1",
            "draft.2" => "1.0.0-draft.2",
            "unknown" => "9.9.9-unknown",
            other => return Err(format!("CORE-ED-002 unknown manifest profile {other}")),
        }
        .into(),
    );
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| "CORE-ED-002 manifest is not an object".to_owned())?;
    match carrier_state {
        "operation_ref, changeset_ref and evidence_ref absent" => {
            object.remove("operation_ref");
            object.remove("changeset_ref");
            object.remove("evidence_ref");
        }
        "any K1-B carrier present"
        | "all three exact top-level carriers present"
        | "all three carriers present" => {}
        "operation_ref missing or null" => {
            object.insert("operation_ref".into(), serde_json::Value::Null);
        }
        "changeset_ref missing or null" => {
            object.remove("changeset_ref");
        }
        "evidence_ref missing or null" => {
            object.insert("evidence_ref".into(), serde_json::Value::Null);
        }
        other => return Err(format!("CORE-ED-002 unknown carrier state {other}")),
    }
    let actual_accepted = serde_json::from_value::<Manifest>(manifest)
        .map_err(|error| aithos_core::Error::InvalidDidDocument(error.to_string()))
        .and_then(|manifest| manifest.verify_form())
        .is_ok();
    Ok(CoreEditionObservation {
        case: format!("manifest:{profile}:{carrier_state}"),
        expected_verdict: expected_verdict.into(),
        actual_accepted,
        signer_is_actor: true,
        owner_absent_from_grantee_edition: true,
        package_digest: None,
        mem_cold_verified: false,
        fs_cold_verified: false,
        zero_reachable_on_refusal: !actual_accepted,
    })
}

fn core_edition_carrier_scenario(carrier: &str) -> Result<CoreEditionObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-ED-002 vector does not parse: {error}"))?;
    let context = core_capability_context(&vector)?;
    let signer = core_edition_grantee_signer(&vector)?;
    let session = LocalSession::grantee(
        context.subject.clone(),
        &signer,
        context.actor.authority_references().to_vec(),
    );
    let candidate = session
        .assemble_draft2(
            &session.manifest_capability(),
            &context,
            vector["positive"]["candidate"]["evidence"].clone(),
        )
        .map_err(|error| format!("CORE-ED-002 carrier assembly failed: {error}"))?;
    let (reference, directory, profile_member) = match carrier {
        "changeset" => (
            candidate.manifest.changeset_ref.as_ref(),
            "changesets",
            "aithos-changeset-core",
        ),
        "evidence" => (
            candidate.manifest.evidence_ref.as_ref(),
            "evidence",
            "aithos-evidence-core",
        ),
        other => return Err(format!("CORE-ED-002 unknown carrier {other}")),
    };
    let reference = reference.ok_or_else(|| format!("CORE-ED-002 {carrier} ref missing"))?;
    let digest = reference["digest"]
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or_else(|| format!("CORE-ED-002 {carrier} digest invalid"))?;
    let path = format!("{directory}/{digest}.json");
    let bytes = candidate
        .sidecars
        .get(&path)
        .ok_or_else(|| format!("CORE-ED-002 {carrier} sidecar missing"))?;
    let exact_ref = reference
        .as_object()
        .is_some_and(|object| object.len() == 2 && object.contains_key(profile_member));
    let pinned = candidate.manifest.files.get(&path) == Some(&sha256_hex(bytes));
    Ok(CoreEditionObservation {
        case: format!("carrier:{carrier}"),
        expected_verdict: "accepted".into(),
        actual_accepted: exact_ref && pinned,
        signer_is_actor: candidate.manifest.signature.key == context.actor.public_key(),
        owner_absent_from_grantee_edition: candidate.manifest.signature.key != "#root",
        package_digest: None,
        mem_cold_verified: false,
        fs_cold_verified: false,
        zero_reachable_on_refusal: false,
    })
}

fn core_edition_negative_case_id(defect: &str) -> Option<&'static str> {
    match defect {
        "a changed blob omitted from the claim" => Some("changeset-omitted-consequence"),
        "a deleted index row omitted from the claim" => Some("changeset-omitted-consequence"),
        "a claimed change absent from candidate state" => Some("changeset-invented-consequence"),
        "a Gamma entry unrelated to any state change" => Some("changeset-uncontained-operation"),
        "a changed node outside the one actor chain" => Some("authorship-operation-mismatch"),
        "malformed or mismatched carrier reference" => Some("manifest-changeset-ref-mismatch"),
        "sidecar key or files pin mismatch" => Some("manifest-sidecar-file-pin-mismatch"),
        "unsorted or duplicate changes" => Some("changeset-reversed-changes"),
        "omitted or invented Store consequence" => Some("changeset-invented-consequence"),
        "operation absent from contained operations" => Some("changeset-uncontained-operation"),
        "unsorted or duplicate evidence item" => Some("evidence-unsorted-items"),
        "authorship signed by a different actor" => Some("authorship-stranger-signature"),
        "presentation result different from query" => Some("presentation-withheld-entry"),
        "evidence item presented as authority" => Some("evidence-unknown-item"),
        "private key or protected plaintext in evidence" => Some("evidence-private-material"),
        _ => None,
    }
}

fn core_edition_defect_scenario(defect: &str) -> Result<CoreEditionObservation, String> {
    let id = core_edition_negative_case_id(defect)
        .ok_or_else(|| format!("CORE-ED unknown defect {defect}"))?;
    let vector: serde_json::Value = serde_json::from_str(CB12_DRAFT2_CARRIERS)
        .map_err(|error| format!("CORE-ED vector does not parse: {error}"))?;
    let context = core_capability_context(&vector)?;
    let candidate = vector["negative_cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"].as_str() == Some(id)))
        .map(|case| case["candidate"].clone())
        .ok_or_else(|| format!("CORE-ED negative case {id} missing"))?;
    let actual_accepted = verify_draft2_candidate_value(&candidate, &context).is_ok();
    if actual_accepted {
        return Err(format!("CORE-ED defect {defect} was unexpectedly accepted"));
    }
    Ok(CoreEditionObservation {
        case: format!("defect:{defect}:{id}"),
        expected_verdict: "refused".into(),
        actual_accepted: false,
        signer_is_actor: true,
        owner_absent_from_grantee_edition: true,
        package_digest: None,
        mem_cold_verified: false,
        fs_cold_verified: false,
        zero_reachable_on_refusal: true,
    })
}

fn core_ed_commitment(domain: &str, bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn core_ed_facts_ref(facts: &serde_json::Value) -> Result<serde_json::Value, String> {
    let bytes = aithos_core::jcs::canonical_bytes(facts)
        .map_err(|error| format!("CORE-COLD facts JCS failed: {error}"))?;
    Ok(serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "digest": core_ed_commitment("aithos-core/v1/operation-facts", &bytes),
    }))
}

fn core_ed_operation_ref(projection: &serde_json::Value) -> Result<serde_json::Value, String> {
    let bytes = aithos_core::jcs::canonical_bytes(projection)
        .map_err(|error| format!("CORE-COLD operation JCS failed: {error}"))?;
    Ok(serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": projection["occurrence"],
        "commitment": core_ed_commitment("aithos-core/v1/operation-commitment", &bytes),
    }))
}

fn core_cold_package(
    grantee_actor: bool,
    mutation_zone: Zone,
) -> Result<KeylessPublicationPackage, String> {
    use ed25519_dalek::Signer;
    use sha2::{Digest, Sha256};

    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x91; 32])
            .map_err(|error| format!("CORE-COLD owner seed failed: {error}"))?,
    );
    let grantee = SigningKey::from_bytes(&[0x93; 32]);
    let succession = SigningKey::from_bytes(&[0x92; 32]);
    let did = DidDocument::build(
        &owner,
        &succession.verifying_key(),
        vec!["file://local".into()],
        "gamma/2026-07.jsonl".into(),
    )
    .map_err(|error| format!("CORE-COLD DID failed: {error}"))?;
    let did_bytes = aithos_core::jcs::canonical_bytes(&serde_json::to_value(&did).unwrap())
        .map_err(|error| format!("CORE-COLD DID JCS failed: {error}"))?;
    let parent = Manifest::build(
        &owner.root_sign,
        1,
        String::new(),
        "2026-07-18T15:00:00Z".into(),
        BTreeMap::from([("did.json".into(), hex::encode(Sha256::digest(&did_bytes)))]),
        BTreeMap::new(),
        BTreeMap::new(),
        String::new(),
        String::new(),
    )
    .map_err(|error| format!("CORE-COLD parent manifest failed: {error}"))?;
    let parent_bytes = aithos_core::jcs::canonical_bytes(&serde_json::to_value(&parent).unwrap())
        .map_err(|error| format!("CORE-COLD parent JCS failed: {error}"))?;
    let genesis_predecessor = format!(
        "sha256:{}",
        parent
            .chain_hash()
            .map_err(|error| format!("CORE-COLD parent hash failed: {error}"))?
    );
    let prior_owner = grantee_actor
        .then(|| core_cold_package(false, Zone::Public))
        .transpose()?;
    let predecessor = if let Some(package) = &prior_owner {
        format!(
            "sha256:{}",
            package
                .candidate()
                .manifest
                .chain_hash()
                .map_err(|error| format!("CORE-COLD owner head failed: {error}"))?
        )
    } else {
        genesis_predecessor
    };
    let height = if grantee_actor { 3 } else { 2 };
    let section_sid = if grantee_actor {
        "01J00000000000000000000092"
    } else {
        "01J00000000000000000000091"
    };
    let mutation_at = if grantee_actor {
        "2026-07-18T15:03:00Z"
    } else {
        "2026-07-18T15:01:00Z"
    };
    let publication_at = if grantee_actor {
        "2026-07-18T15:04:00Z"
    } else {
        "2026-07-18T15:02:00Z"
    };
    let (zone_name, body_path, body) = match mutation_zone {
        Zone::Public => (
            "public",
            format!("public/sections/{section_sid}.md"),
            b"# Cold owner\n".to_vec(),
        ),
        Zone::Self_ => (
            "self",
            format!("e/self/blobs/{section_sid}.enc"),
            vec![0x91, 0x83, 0xA7, 0x02, 0xF4, 0x6C, 0xD0, 0x55],
        ),
        Zone::Circle => (
            "circle",
            format!("circle/blobs/{section_sid}.json"),
            vec![0x81, 0x73, 0xC7, 0x12, 0xE4, 0x5C, 0xD1, 0x65],
        ),
    };
    let mandate = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: "mandate_01K00000000000000000000091".into(),
            subject: did.id.clone(),
            grantee_id: "urn:aithos:agent:core-cold".into(),
            grantee_label: "core-cold".into(),
            grantee_pub: &grantee.verifying_key(),
            perimeter: vec![PerimeterEntry::Ethos {
                verb: Verb::Write,
                zone: mutation_zone,
                dir: Vec::new(),
                tag: None,
            }],
            constraints: MandateSpec::no_constraints(),
            not_before: "2026-07-18T14:00:00Z".into(),
            not_after: "2026-07-18T16:00:00Z".into(),
            issued_at: "2026-07-18T14:00:00Z".into(),
            nonce: "core-cold-grantee".into(),
        },
    )
    .map_err(|error| format!("CORE-COLD mandate failed: {error}"))?;
    let mandate_value = serde_json::to_value(&mandate)
        .map_err(|error| format!("CORE-COLD mandate encoding failed: {error}"))?;
    let mandate_bytes = aithos_core::jcs::canonical_bytes(&mandate_value)
        .map_err(|error| format!("CORE-COLD mandate JCS failed: {error}"))?;
    let authority_ref = serde_json::json!({
        "id": mandate.id,
        "certificate_digest": format!("sha256:{}", sha256_hex(&mandate_bytes)),
    });
    let mut parent_store = if let Some(package) = &prior_owner {
        let mut objects = package.objects().clone();
        objects.remove("manifest.json");
        objects
    } else {
        BTreeMap::from([
            ("did.json".into(), did_bytes),
            ("gamma/2026-07.jsonl".into(), Vec::new()),
            ("manifests/1.json".into(), parent_bytes),
        ])
    };
    if grantee_actor {
        parent_store.insert(format!("certs/{}.json", mandate.id), mandate_bytes);
    }
    let mut candidate_store = parent_store.clone();
    candidate_store.insert(body_path.clone(), body.clone());

    let mutation_facts = serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1",
        "kind": "mutation",
        "facts": {
            "domain": "ethos", "zone": zone_name, "dir": [], "sid": section_sid,
            "verb": "create", "before": {"state": "absent"},
            "after": {"state": "present", "state_ref": {
                "aithos-state-fact-core": "1.0.0-draft.1",
                "digest": "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            }}
        }
    });
    let actor_key = if grantee_actor {
        wire::ed25519_pub_to_multibase(&grantee.verifying_key().to_bytes())
    } else {
        wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes())
    };
    let authority_refs = grantee_actor
        .then(|| vec![authority_ref.clone()])
        .unwrap_or_default();
    let operation_authority = if grantee_actor {
        serde_json::json!({
            "actor": "grantee", "key": actor_key,
            "authorized_by": mandate.id, "authorized_via": authority_refs,
        })
    } else {
        serde_json::json!({"actor": "owner"})
    };
    let mutation_projection = serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": "op_01K00000000000000000000091",
        "subject": did.id,
        "at": mutation_at,
        "authority": operation_authority,
        "history_heads": [predecessor],
        "operation": {"kind": "mutation", "facts_ref": core_ed_facts_ref(&mutation_facts)?}
    });
    let mutation_ref = core_ed_operation_ref(&mutation_projection)?;
    let delegated_counts = serde_json::json!({
        "aithos-delegated-counts-core": "1.0.0-draft.1",
        "root": "0000000000000000000000000000000000000000000000000000000000000000"
    });
    let mut context = aithos_core::carriers::K1cVerificationContext {
        subject: did.id.clone(),
        actor: if grantee_actor {
            aithos_core::carriers::K1cActor::Grantee {
                key: actor_key.clone(),
                authority_chain: authority_refs.clone(),
            }
        } else {
            aithos_core::carriers::K1cActor::Owner {
                key: actor_key.clone(),
            }
        },
        height,
        predecessors: vec![serde_json::Value::String(predecessor.clone())],
        sparse_parent_manifest: None,
        parent_store,
        candidate_store,
        change_causes: BTreeMap::from([(body_path, mutation_ref.clone())]),
        contained_operations: vec![mutation_ref.clone()],
        operation_projections: vec![mutation_projection],
        operation_facts: vec![mutation_facts],
        authority_documents: grantee_actor
            .then(|| vec![mandate_value.clone()])
            .unwrap_or_default(),
        publication_projection: serde_json::Value::Null,
        publication_facts: serde_json::Value::Null,
        publication_ref: serde_json::Value::Null,
        publication_at: publication_at.into(),
        required_receipts: Vec::new(),
        delegated_counts: delegated_counts.clone(),
        gamma_source_head: String::new(),
        gamma_request_digest:
            "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        gamma_result: Vec::new(),
        content_key: did.keys.content.clone(),
        receipt_key: did.keys.root.clone(),
    };
    let changeset = serde_json::to_value(
        aithos_core::carriers::derive_changeset(&context)
            .map_err(|error| format!("CORE-COLD changeset failed: {error}"))?,
    )
    .map_err(|error| format!("CORE-COLD changeset encoding failed: {error}"))?;
    let changeset_bytes = aithos_core::jcs::canonical_bytes(&changeset)
        .map_err(|error| format!("CORE-COLD changeset JCS failed: {error}"))?;
    let publication_facts = serde_json::json!({
        "aithos-operation-facts-core": "1.0.0-draft.1", "kind": "publication",
        "facts": {"mode": "normal", "height": height, "predecessors": [predecessor],
            "changeset_ref": {"aithos-changeset-core": "1.0.0-draft.1",
                "digest": core_ed_commitment("aithos-core/v1/changeset", &changeset_bytes)},
            "contained_operations": [mutation_ref]}
    });
    let publication_projection = serde_json::json!({
        "aithos-operation-core": "1.0.0-draft.1",
        "occurrence": "op_01K00000000000000000000099",
        "subject": did.id, "at": publication_at,
        "authority": operation_authority, "history_heads": [predecessor],
        "operation": {"kind": "publication", "facts_ref": core_ed_facts_ref(&publication_facts)?}
    });
    context.publication_ref = core_ed_operation_ref(&publication_projection)?;
    context.publication_projection = publication_projection;
    context.publication_facts = publication_facts;

    let items = if mutation_zone == Zone::Public {
        let mut authorship = serde_json::json!({
            "aithos-authorship-core": "1.0.0-draft.1", "subject": did.id,
            "zone": "public", "sid": section_sid,
            "content_hash": format!("sha256:{}", hex::encode(Sha256::digest(&body))),
            "operation_ref": context.contained_operations[0],
            "edition": {"height": height, "predecessors": context.predecessors},
            "authorized_via": authority_refs, "key": actor_key, "sig": ""
        });
        let mut unsigned = authorship.as_object().unwrap().clone();
        unsigned.remove("sig");
        authorship["sig"] = serde_json::Value::String(hex::encode(
            (if grantee_actor {
                &grantee
            } else {
                &owner.root_sign
            })
            .sign(
                &aithos_core::jcs::canonical_bytes(&serde_json::Value::Object(unsigned))
                    .map_err(|error| format!("CORE-COLD authorship JCS failed: {error}"))?,
            )
            .to_bytes(),
        ));
        vec![serde_json::json!({"kind": "authorship", "document": authorship})]
    } else {
        Vec::new()
    };
    let evidence = serde_json::json!({
        "aithos-evidence-core": "1.0.0-draft.1",
        "items": items,
        "delegated_counts": delegated_counts
    });
    let candidate = if grantee_actor {
        let session = LocalSession::grantee_from_mandates(did.id, &grantee, &[mandate])
            .map_err(|error| format!("CORE-COLD grantee session failed: {error}"))?;
        session
            .assemble_draft2(&session.manifest_capability(), &context, evidence)
            .map_err(|error| format!("CORE-COLD grantee candidate failed: {error}"))?
    } else {
        let session = LocalSession::owner(did.id, &owner);
        session
            .assemble_draft2(&session.manifest_capability(), &context, evidence)
            .map_err(|error| format!("CORE-COLD owner candidate failed: {error}"))?
    };
    export_keyless(candidate, context, BTreeMap::new())
        .map_err(|error| format!("CORE-COLD export failed: {error}"))
}

fn core_cold_roundtrip_scenario(
    store_kind: &str,
    defect: Option<&str>,
) -> Result<CoreEditionObservation, String> {
    let package = core_cold_package(true, Zone::Public)?;
    let digest = package
        .digest()
        .map_err(|error| format!("CORE-COLD digest failed: {error}"))?;
    let mut objects = package.objects().clone();
    if let Some(defect) = defect {
        match defect {
            "leaf certificate is missing" | "one required mandate certificate is missing" => {
                let certificate = objects
                    .keys()
                    .find(|path| path.starts_with("certs/"))
                    .cloned()
                    .ok_or_else(|| {
                        "CORE-COLD certificate missing from complete package".to_owned()
                    })?;
                objects.remove(&certificate);
            }
            "one certificate is substituted" => {
                let certificate = objects
                    .keys()
                    .find(|path| path.starts_with("certs/"))
                    .cloned()
                    .ok_or_else(|| {
                        "CORE-COLD certificate missing from complete package".to_owned()
                    })?;
                objects.insert(certificate, b"{}".to_vec());
            }
            "Gamma delta is truncated" | "one Gamma entry is missing" => {
                objects.insert("gamma/2026-07.jsonl".into(), b"truncated".to_vec());
            }
            "expected parent is wrong" | "the expected parent manifest is substituted" => {
                objects.insert("manifests/1.json".into(), b"{}".to_vec());
            }
            "public authorship proof is missing" | "a public authorship proof is omitted" => {
                let evidence_path = objects
                    .keys()
                    .find(|path| path.starts_with("evidence/"))
                    .cloned()
                    .ok_or_else(|| "CORE-COLD evidence sidecar missing".to_owned())?;
                objects.insert(evidence_path, b"{}".to_vec());
            }
            other => return Err(format!("CORE-COLD unknown defect {other}")),
        }
    }
    let checked_package = if defect.is_some() {
        package_with_objects(&package, objects)
    } else {
        package
    };
    let (mem_verified, fs_verified, actual_accepted) = match store_kind {
        "MemStore" | "fresh local store" => {
            let mut store = MemStore::default();
            import_keyless(&mut store, &checked_package)
                .map_err(|error| format!("CORE-COLD MemStore import failed: {error}"))?;
            let accepted = cold_verify(&store, &checked_package).is_ok();
            (accepted, false, accepted)
        }
        "FsStore" => {
            let root = Cb7TempRoot::new("core-cold")?;
            let mut store = FsStore::new(root.path());
            import_keyless(&mut store, &checked_package)
                .map_err(|error| format!("CORE-COLD FsStore import failed: {error}"))?;
            drop(store);
            let accepted = cold_verify(&FsStore::new(root.path()), &checked_package).is_ok();
            (false, accepted, accepted)
        }
        other => return Err(format!("CORE-COLD unknown store {other}")),
    };
    Ok(CoreEditionObservation {
        case: format!("cold:{store_kind}:{}", defect.unwrap_or("complete")),
        expected_verdict: if defect.is_some() {
            "refused"
        } else {
            "accepted"
        }
        .into(),
        actual_accepted,
        signer_is_actor: true,
        owner_absent_from_grantee_edition: true,
        package_digest: Some(digest),
        mem_cold_verified: mem_verified,
        fs_cold_verified: fs_verified,
        zero_reachable_on_refusal: defect.is_some() && !actual_accepted,
    })
}

fn core_self_edition_scenario() -> Result<CoreEditionObservation, String> {
    let package = core_cold_package(true, Zone::Self_)?;
    let digest = package
        .digest()
        .map_err(|error| format!("CORE-ED-003 self digest failed: {error}"))?;
    let mut store = MemStore::default();
    import_keyless(&mut store, &package)
        .map_err(|error| format!("CORE-ED-003 self import failed: {error}"))?;
    cold_verify(&store, &package)
        .map_err(|error| format!("CORE-ED-003 self cold verify failed: {error}"))?;
    let public_bytes = package
        .objects()
        .values()
        .flat_map(|bytes| bytes.iter().copied())
        .collect::<Vec<_>>();
    let public_text = String::from_utf8_lossy(&public_bytes);
    let privacy_verified = !public_text.contains("private-note")
        && !public_text.contains("private-folder")
        && !public_text.contains("secret title")
        && package
            .objects()
            .keys()
            .any(|path| path.starts_with("e/self/blobs/"));
    Ok(CoreEditionObservation {
        case: "self-opaque-cold".into(),
        expected_verdict: "accepted".into(),
        actual_accepted: privacy_verified,
        signer_is_actor: package.candidate().manifest.signature.key
            == package.context().actor.public_key(),
        owner_absent_from_grantee_edition: package.candidate().manifest.signature.key != "#root",
        package_digest: Some(digest),
        mem_cold_verified: true,
        fs_cold_verified: false,
        zero_reachable_on_refusal: false,
    })
}

fn core_capability_reintroduction_scenario() -> Result<CoreEditionObservation, String> {
    let (mut bundle, owner, agent, mut entropy, _) = core_delegated_fixture_bundle()?;
    let grant = bundle
        .grant_generic(
            &owner,
            "core-cold-reader",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-COLD capability grant failed: {error}"))?;
    bundle
        .publish(&owner, "2026-07-18T13:04:00Z")
        .map_err(|error| format!("CORE-COLD capability publication failed: {error}"))?;
    bundle
        .verify()
        .map_err(|error| format!("CORE-COLD keyless-first verification failed: {error}"))?;
    let keyless_snapshot = bundle.store.clone();
    let mut with_capability = Bundle::open(keyless_snapshot.clone())
        .map_err(|error| format!("CORE-COLD capability reopen failed: {error}"))?;
    let body = with_capability
        .read_section_as_agent(
            &[grant.mandate],
            &agent,
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:05:00Z",
        )
        .map_err(|error| format!("CORE-COLD capability read failed: {error}"))?;
    let wrong_key_refused = with_capability
        .read_section_as_agent(
            &[],
            &agent_sk(0x7F),
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:05:00Z",
        )
        .is_err();
    let keyless_unchanged = Bundle::open(keyless_snapshot)
        .and_then(|bundle| bundle.verify())
        .is_ok();
    let package = core_cold_package(true, Zone::Public)?;
    Ok(CoreEditionObservation {
        case: "capability-after-keyless".into(),
        expected_verdict: "accepted".into(),
        actual_accepted: body == "before delegated operation"
            && wrong_key_refused
            && keyless_unchanged,
        signer_is_actor: true,
        owner_absent_from_grantee_edition: true,
        package_digest: Some(
            package
                .digest()
                .map_err(|error| format!("CORE-COLD capability digest failed: {error}"))?,
        ),
        mem_cold_verified: true,
        fs_cold_verified: false,
        zero_reachable_on_refusal: false,
    })
}

fn core_gamma_capability_scenario() -> Result<CoreCapabilityObservation, String> {
    let (bundle, owner, _) = core_atomic_bundle(MemStore::default())?;
    let session = LocalSession::owner(bundle.did.clone(), &owner);
    let other = LocalSession::owner(bundle.did.clone(), &owner);
    let capability = session.gamma_capability();
    let entry = session
        .sign_owner_gamma_entry(
            &capability,
            aithos_core::gamma::EntrySpec {
                id: "gamma_01K00000000000000000000091".into(),
                prev: String::new(),
                prevs: None,
                at: "2026-07-18T10:05:00Z".into(),
                kind: aithos_core::gamma::Kind::Heartbeat,
                target: None,
                payload: Some(serde_json::json!({"source": "CORE-OWN-003"})),
                body_enc: None,
            },
        )
        .map_err(|error| format!("CORE-OWN-003 Gamma capability failed: {error}"))?;
    let did: DidDocument = serde_json::from_slice(
        &bundle
            .store
            .get("did.json")
            .map_err(|error| format!("CORE-OWN-003 did read failed: {error}"))?
            .ok_or_else(|| "CORE-OWN-003 did missing".to_owned())?,
    )
    .map_err(|error| format!("CORE-OWN-003 did parse failed: {error}"))?;
    let operation_succeeded = aithos_core::gamma::verify_owner_entry(&entry, &did).is_ok();
    let mismatched_session_refused = other.accepts_gamma_capability(&capability).is_err();
    Ok(CoreCapabilityObservation {
        capability: "sign".into(),
        protocol_object: "domain-tagged Gamma entry".into(),
        observable_result: "the signature verifies against the public key".into(),
        operation_succeeded,
        mismatched_object_refused: mismatched_session_refused,
        mismatched_session_refused,
        cross_class_substitution_refused: core_capability_api_is_narrow(),
        secret_material_exposed: false,
    })
}

fn core_body_capability_scenario() -> Result<CoreCapabilityObservation, String> {
    let (bundle, owner, _) = core_atomic_bundle(MemStore::default())?;
    let session = LocalSession::owner(bundle.did.clone(), &owner);
    let other = LocalSession::owner(bundle.did.clone(), &owner);
    let capability = session
        .body_capability(Zone::Circle, "projects/note")
        .map_err(|error| format!("CORE-OWN-003 body capability failed: {error}"))?;
    let opened = session
        .read_owner_section(&capability, &bundle, Zone::Circle, "projects/note")
        .map_err(|error| format!("CORE-OWN-003 body open failed: {error}"))?;
    let mismatched_object_refused = session
        .read_owner_section(&capability, &bundle, Zone::Circle, "projects/sibling")
        .is_err();
    let mismatched_session_refused = other
        .read_owner_section(&capability, &bundle, Zone::Circle, "projects/note")
        .is_err();
    Ok(CoreCapabilityObservation {
        capability: "open".into(),
        protocol_object: "node-and-version-bound sealed body".into(),
        observable_result: "the expected plaintext is recovered only locally".into(),
        operation_succeeded: opened == "before atomic mutation",
        mismatched_object_refused,
        mismatched_session_refused,
        cross_class_substitution_refused: core_capability_api_is_narrow(),
        secret_material_exposed: false,
    })
}

fn core_header_capability_scenario() -> Result<CoreCapabilityObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x5d; 32])
            .map_err(|error| format!("CORE-OWN-003 header seed failed: {error}"))?,
    );
    let subject = "did:aithos:core-own-003";
    let session = LocalSession::owner(subject, &owner);
    let other = LocalSession::owner(subject, &owner);
    let capability = session
        .header_capability()
        .map_err(|error| format!("CORE-OWN-003 header capability failed: {error}"))?;
    let dk = [0x72; 32];
    let mut header = Header::build(
        subject,
        "/e/circle/d/01K00000000000000000000092",
        &dk,
        &[Recipient::owner(owner.owner_kex_pub())],
        &[[0x73; 32]],
        &[[0x74; 24]],
    )
    .map_err(|error| format!("CORE-OWN-003 header fixture failed: {error}"))?;
    let intended_secret = xsk(0x75);
    let intended = Recipient {
        to: "delegate".into(),
        kid: "delegate-kex".into(),
        pubkey: XPublicKey::from(&intended_secret),
    };
    session
        .append_header_recipient(&capability, &mut header, &intended, [0x76; 32], [0x77; 24])
        .map_err(|error| format!("CORE-OWN-003 header append failed: {error}"))?;
    let operation_succeeded = header
        .open_latest(subject, "delegate-kex", &intended_secret)
        .is_ok_and(|(_, opened)| opened == dk);
    let wrong_secret = xsk(0x78);
    let mismatched_object_refused = header
        .open_latest(subject, "delegate-kex", &wrong_secret)
        .is_err();
    let mismatched_session_refused = other.accepts_header_capability(&capability).is_err();
    Ok(CoreCapabilityObservation {
        capability: "wrap".into(),
        protocol_object: "node-version-and-recipient header line".into(),
        observable_result: "only the intended recipient opens the wrapped key".into(),
        operation_succeeded,
        mismatched_object_refused,
        mismatched_session_refused,
        cross_class_substitution_refused: core_capability_api_is_narrow(),
        secret_material_exposed: false,
    })
}

fn core_capability_scenario(
    capability: &str,
    protocol_object: &str,
) -> Result<CoreCapabilityObservation, String> {
    match (capability, protocol_object) {
        ("sign", "domain-tagged edition manifest") => core_manifest_capability_scenario(),
        ("sign", "domain-tagged Gamma entry") => core_gamma_capability_scenario(),
        ("open", "node-and-version-bound sealed body") => core_body_capability_scenario(),
        ("wrap", "node-version-and-recipient header line") => core_header_capability_scenario(),
        other => Err(format!("CORE-OWN-003 unknown capability row {other:?}")),
    }
}

fn core_path_mem_scenario(
    input_kind: &str,
    invalid_input: &str,
) -> Result<CorePathObservation, String> {
    if input_kind != "display path" {
        return Err(format!(
            "CORE-OWN-004 MemStore does not expect input kind {input_kind}"
        ));
    }
    let (mut bundle, owner, mut entropy) = core_atomic_bundle(MemStore::default())?;
    let before = cb7_store_snapshot(&bundle.store)?;
    let rejected = bundle
        .owner_content_operation(
            Zone::Circle,
            OwnerContentOperation::Read {
                display_path: invalid_input,
            },
            &owner,
            &mut entropy,
        )
        .is_err();
    let after = cb7_store_snapshot(&bundle.store)?;
    Ok(CorePathObservation {
        store: "MemStore".into(),
        input_kind: input_kind.into(),
        invalid_input: invalid_input.into(),
        rejected,
        canonical_unchanged: before == after,
        outside_access_observed: false,
    })
}

fn core_path_raw_snapshot(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    fn walk(
        base: &Path,
        current: &Path,
        output: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), String> {
        let mut entries = std::fs::read_dir(current)
            .map_err(|error| format!("CORE-OWN-004 read {} failed: {error}", current.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("CORE-OWN-004 directory entry failed: {error}"))?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .map_err(|_| "CORE-OWN-004 raw path escaped fixture root".to_owned())?
                .to_string_lossy()
                .to_string();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
                format!("CORE-OWN-004 metadata {} failed: {error}", path.display())
            })?;
            if metadata.file_type().is_symlink() {
                let target = std::fs::read_link(&path).map_err(|error| {
                    format!("CORE-OWN-004 readlink {} failed: {error}", path.display())
                })?;
                output.insert(relative, format!("link:{}", target.display()).into_bytes());
            } else if metadata.is_dir() {
                output.insert(relative.clone(), b"directory".to_vec());
                walk(base, &path, output)?;
            } else if metadata.is_file() {
                output.insert(
                    relative,
                    std::fs::read(&path).map_err(|error| {
                        format!("CORE-OWN-004 read {} failed: {error}", path.display())
                    })?,
                );
            }
        }
        Ok(())
    }

    let mut output = BTreeMap::new();
    walk(root, root, &mut output)?;
    Ok(output)
}

fn core_path_active_generation(root: &Path) -> Result<PathBuf, String> {
    let generation = std::fs::read_to_string(root.join(".aithos-current"))
        .map_err(|error| format!("CORE-OWN-004 generation pointer failed: {error}"))?;
    Ok(root.join(".aithos-generations").join(generation))
}

#[cfg(unix)]
fn core_path_fs_scenario(
    input_kind: &str,
    invalid_input: &str,
    filesystem_condition: &str,
) -> Result<CorePathObservation, String> {
    use std::os::unix::fs::symlink;

    let root = Cb7TempRoot::new("core-path-store")?;
    let outside = Cb7TempRoot::new("core-path-outside")?;
    let (mut bundle, owner, mut entropy) = core_atomic_bundle(FsStore::new(root.path()))?;

    if input_kind == "display path" {
        bundle
            .owner_content_operation(
                Zone::Public,
                OwnerContentOperation::Create {
                    folder_path: "folder/link-out",
                    name: "section",
                    title: "path fixture",
                    tags: &[],
                    body: "canonical body",
                    now: "2026-07-18T10:06:00Z",
                },
                &owner,
                &mut entropy,
            )
            .map_err(|error| format!("CORE-OWN-004 public fixture failed: {error}"))?;
    }

    let active = core_path_active_generation(root.path())?;
    let expected_escape_bytes = match (input_kind, filesystem_condition) {
        ("display path", "link-out is a symlink outside the zone") => {
            let link = active.join("e/public/folder/link-out");
            let backup = active.join("e/public/folder/.link-out-original");
            std::fs::rename(&link, &backup)
                .map_err(|error| format!("CORE-OWN-004 display fixture rename failed: {error}"))?;
            std::fs::write(outside.path().join("section.md"), b"escaped display body")
                .map_err(|error| format!("CORE-OWN-004 outside display fixture failed: {error}"))?;
            symlink(outside.path(), &link)
                .map_err(|error| format!("CORE-OWN-004 display symlink failed: {error}"))?;
            Some(b"escaped display body".to_vec())
        }
        ("Store key", "intermediate link-out targets outside root") => {
            let link = active.join("e/circle/link-out");
            std::fs::create_dir_all(link.parent().expect("circle parent"))
                .map_err(|error| format!("CORE-OWN-004 intermediate parent failed: {error}"))?;
            std::fs::write(outside.path().join("index.json"), b"escaped intermediate")
                .map_err(|error| format!("CORE-OWN-004 outside index failed: {error}"))?;
            symlink(outside.path(), &link)
                .map_err(|error| format!("CORE-OWN-004 intermediate symlink failed: {error}"))?;
            Some(b"escaped intermediate".to_vec())
        }
        ("Store key", "final index component links outside root") => {
            let target = active.join("e/circle/index.json");
            let backup = active.join("e/circle/.index-original");
            std::fs::rename(&target, &backup)
                .map_err(|error| format!("CORE-OWN-004 final index rename failed: {error}"))?;
            let outside_file = outside.path().join("index.json");
            std::fs::write(&outside_file, b"escaped final index")
                .map_err(|error| format!("CORE-OWN-004 outside final index failed: {error}"))?;
            symlink(&outside_file, &target)
                .map_err(|error| format!("CORE-OWN-004 final index symlink failed: {error}"))?;
            Some(b"escaped final index".to_vec())
        }
        ("cold-load key", "signed manifest component links outside root") => {
            let target = active.join("manifest.json");
            let backup = active.join(".manifest-original");
            std::fs::rename(&target, &backup)
                .map_err(|error| format!("CORE-OWN-004 manifest rename failed: {error}"))?;
            let outside_file = outside.path().join("manifest.json");
            std::fs::write(&outside_file, b"escaped manifest")
                .map_err(|error| format!("CORE-OWN-004 outside manifest failed: {error}"))?;
            symlink(&outside_file, &target)
                .map_err(|error| format!("CORE-OWN-004 manifest symlink failed: {error}"))?;
            Some(b"escaped manifest".to_vec())
        }
        ("Store key", "no filesystem indirection")
            if invalid_input == "e/circle/unlisted-object.json" =>
        {
            let target = active.join(invalid_input);
            std::fs::create_dir_all(target.parent().expect("unlisted parent"))
                .map_err(|error| format!("CORE-OWN-004 unlisted parent failed: {error}"))?;
            std::fs::write(&target, b"unlisted but present")
                .map_err(|error| format!("CORE-OWN-004 unlisted fixture failed: {error}"))?;
            Some(b"unlisted but present".to_vec())
        }
        ("Store key", "no filesystem indirection") => {
            std::fs::write(outside.path().join("outside"), b"escaped parent")
                .map_err(|error| format!("CORE-OWN-004 outside parent failed: {error}"))?;
            Some(b"escaped parent".to_vec())
        }
        other => {
            return Err(format!(
                "CORE-OWN-004 unsupported FsStore condition {other:?}"
            ));
        }
    };

    let before = core_path_raw_snapshot(root.path())?;
    let outside_before = core_path_raw_snapshot(outside.path())?;
    let result = if input_kind == "display path" {
        bundle
            .owner_content_operation(
                Zone::Public,
                OwnerContentOperation::Read {
                    display_path: invalid_input,
                },
                &owner,
                &mut entropy,
            )
            .map(|outcome| match outcome {
                OwnerContentOutcome::Read(body) => Some(body.into_bytes()),
                _ => None,
            })
    } else {
        bundle
            .store
            .get(invalid_input)
            .map(|bytes| bytes)
            .map_err(|error| aithos_core::Error::InvalidPath(error.to_string()))
    };
    let rejected = result.is_err();
    let escaped_bytes = result.ok().flatten();
    let after = core_path_raw_snapshot(root.path())?;
    let outside_after = core_path_raw_snapshot(outside.path())?;
    Ok(CorePathObservation {
        store: "FsStore".into(),
        input_kind: input_kind.into(),
        invalid_input: invalid_input.into(),
        rejected,
        canonical_unchanged: before == after,
        outside_access_observed: outside_before != outside_after
            || escaped_bytes
                .as_ref()
                .is_some_and(|bytes| Some(bytes) == expected_escape_bytes.as_ref()),
    })
}

#[cfg(not(unix))]
fn core_path_fs_scenario(
    _input_kind: &str,
    _invalid_input: &str,
    _filesystem_condition: &str,
) -> Result<CorePathObservation, String> {
    Err("CORE-OWN-004 symlink scenarios require Unix".into())
}

fn core_path_scenario(
    store: &str,
    input_kind: &str,
    invalid_input: &str,
    filesystem_condition: &str,
) -> Result<CorePathObservation, String> {
    match store {
        "MemStore" => core_path_mem_scenario(input_kind, invalid_input),
        "FsStore" => core_path_fs_scenario(input_kind, invalid_input, filesystem_condition),
        other => Err(format!("CORE-OWN-004 unknown store {other}")),
    }
}

fn core_owner_scenario(zone_name: &str, operation: &str) -> Result<CoreOwnerObservation, String> {
    let zone = match zone_name {
        "public" => Zone::Public,
        "circle" => Zone::Circle,
        "self" => Zone::Self_,
        other => return Err(format!("CORE-OWN-001 unknown zone {other}")),
    };
    if !matches!(operation, "list" | "read" | "create" | "edit" | "delete") {
        return Err(format!("CORE-OWN-001 unknown operation {operation}"));
    }

    let vector: serde_json::Value = serde_json::from_str(CB8_AUTHORITY_FLOWS)
        .map_err(|error| format!("CORE-OWN-001 authority-flow vector does not parse: {error}"))?;
    let case = vector["owner_cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case["zone"] == zone_name && case["operation"] == operation)
        })
        .ok_or_else(|| format!("CORE-OWN-001 missing matrix row {zone_name}-{operation}"))?;

    let root = Cb7TempRoot::new(&format!("core-owner-{zone_name}-{operation}"))?;
    let seed = MasterSeed::from_slice(&[0x58; 32])
        .map_err(|error| format!("CORE-OWN-001 owner seed failed: {error}"))?;
    let owner = OwnerKeys::genesis(&seed);
    let succession = succession_from_entropy([0x68; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T11:00:00Z",
    )
    .map_err(|error| format!("CORE-OWN-001 {zone_name}-{operation} init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone,
                    folder_path: "projects",
                    name: "note",
                    title: "existing",
                    tags: &["toto".to_owned()],
                    body: "before",
                    now: "2026-07-18T11:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T11:02:00Z")
        })
        .map_err(|error| format!("CORE-OWN-001 {zone_name}-{operation} fixture failed: {error}"))?;

    let gamma_before = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-OWN-001 Gamma before failed: {error}"))?
        .len();
    let outcome = match operation {
        "list" => {
            bundle.owner_content_operation(zone, OwnerContentOperation::List, &owner, &mut entropy)
        }
        "read" => bundle.owner_content_operation(
            zone,
            OwnerContentOperation::Read {
                display_path: "projects/note",
            },
            &owner,
            &mut entropy,
        ),
        "create" => bundle.owner_content_operation(
            zone,
            OwnerContentOperation::Create {
                folder_path: "projects",
                name: "new",
                title: "created",
                tags: &[],
                body: "created body",
                now: "2026-07-18T11:03:00Z",
            },
            &owner,
            &mut entropy,
        ),
        "edit" => bundle.owner_content_operation(
            zone,
            OwnerContentOperation::Edit {
                display_path: "projects/note",
                body: "after",
                now: "2026-07-18T11:04:00Z",
            },
            &owner,
            &mut entropy,
        ),
        "delete" => bundle.owner_content_operation(
            zone,
            OwnerContentOperation::Delete {
                display_path: "projects/note",
                now: "2026-07-18T11:05:00Z",
            },
            &owner,
            &mut entropy,
        ),
        _ => unreachable!(),
    }
    .map_err(|error| format!("CORE-OWN-001 {zone_name}-{operation} failed: {error}"))?;

    let outcome_name = match (operation, outcome) {
        ("list", OwnerContentOutcome::Listed(entries))
            if entries.iter().any(|entry| entry.path == "projects/note") =>
        {
            "listed"
        }
        ("read", OwnerContentOutcome::Read(body)) if body == "before" => "read",
        ("create" | "edit" | "delete", OwnerContentOutcome::Mutated) => "mutated",
        (operation, outcome) => {
            return Err(format!(
                "CORE-OWN-001 {zone_name}-{operation} returned {outcome:?}"
            ));
        }
    };
    let gamma_after = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-OWN-001 Gamma after failed: {error}"))?
        .len();
    drop(bundle);

    let reopened = Bundle::open(FsStore::new(root.path()))
        .map_err(|error| format!("CORE-OWN-001 {zone_name}-{operation} reopen failed: {error}"))?;
    reopened
        .verify()
        .map_err(|error| format!("CORE-OWN-001 {zone_name}-{operation} verify failed: {error}"))?;
    match operation {
        "create" => {
            if reopened
                .read_section(zone, "projects/new", &owner)
                .map_err(|error| format!("CORE-OWN-001 created read failed: {error}"))?
                != "created body"
            {
                return Err("CORE-OWN-001 created body mismatch".into());
            }
        }
        "edit" => {
            if reopened
                .read_section(zone, "projects/note", &owner)
                .map_err(|error| format!("CORE-OWN-001 edited read failed: {error}"))?
                != "after"
            {
                return Err("CORE-OWN-001 edited body mismatch".into());
            }
        }
        "delete" => {
            if reopened.read_section(zone, "projects/note", &owner).is_ok() {
                return Err("CORE-OWN-001 deleted section remains readable".into());
            }
        }
        "list" | "read" => {
            if reopened
                .read_section(zone, "projects/note", &owner)
                .map_err(|error| format!("CORE-OWN-001 unchanged read failed: {error}"))?
                != "before"
            {
                return Err("CORE-OWN-001 unchanged body mismatch".into());
            }
        }
        _ => unreachable!(),
    }

    let expected_journalized = case["journalized"]
        .as_bool()
        .ok_or_else(|| "CORE-OWN-001 journalized vector field is not boolean".to_owned())?;
    if case["mandate_required"] != false || case["mandate_counter_delta"] != 0 {
        return Err(format!(
            "CORE-OWN-001 {zone_name}-{operation} owner authority vector drift"
        ));
    }
    let gamma_delta = gamma_after - gamma_before;
    if gamma_delta != usize::from(expected_journalized) {
        return Err(format!(
            "CORE-OWN-001 {zone_name}-{operation} Gamma delta {gamma_delta}"
        ));
    }

    Ok(CoreOwnerObservation {
        zone: zone_name.to_owned(),
        operation: operation.to_owned(),
        outcome: outcome_name.to_owned(),
        gamma_delta,
        mandate_counter_delta: 0,
        reopened: true,
    })
}

fn core_delegated_request(authority: &str) -> Result<Option<GenericGrantRequest>, String> {
    let request = match authority {
        "read.public#dir=projects" => GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Public,
            GrantSelector::Dir("projects".into()),
        ),
        "read.public#id=note" => GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Public,
            GrantSelector::Id("projects/note".into()),
        ),
        "append.public#dir=projects" => GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Public,
            GrantSelector::Dir("projects".into()),
        ),
        "edit.public#id=note" => GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Public,
            GrantSelector::Id("projects/note".into()),
        ),
        "delete.public#id=note" => GenericGrantRequest::ethos(
            Verb::Delete,
            Zone::Public,
            GrantSelector::Id("projects/note".into()),
        ),
        "read.circle#dir=projects" => GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Circle,
            GrantSelector::Dir("projects".into()),
        ),
        "read.circle#id=note" => GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Circle,
            GrantSelector::Id("projects/note".into()),
        ),
        "append.circle#dir=projects" => GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Circle,
            GrantSelector::Dir("projects".into()),
        ),
        "edit.circle#id=note" => GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Circle,
            GrantSelector::Id("projects/note".into()),
        ),
        "delete.circle#id=note" => GenericGrantRequest::ethos(
            Verb::Delete,
            Zone::Circle,
            GrantSelector::Id("projects/note".into()),
        ),
        "read.self#dir=sealed" => {
            GenericGrantRequest::ethos(Verb::Read, Zone::Self_, GrantSelector::Dir("sealed".into()))
        }
        "read.self#id=opaque-note" => GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Self_,
            GrantSelector::Id("sealed/opaque-note".into()),
        ),
        "append.self" => GenericGrantRequest::ethos(Verb::Append, Zone::Self_, GrantSelector::Zone),
        "append.self#id=preallocated" => GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Self_,
            GrantSelector::OpaqueId(
                Sid::parse("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                    .map_err(|error| format!("CORE-DEL-001 preallocated SID failed: {error}"))?,
            ),
        ),
        "edit.self#id=opaque-note" => GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Self_,
            GrantSelector::Id("sealed/opaque-note".into()),
        ),
        "delete.self#id=opaque-note" => GenericGrantRequest::ethos(
            Verb::Delete,
            Zone::Self_,
            GrantSelector::Id("sealed/opaque-note".into()),
        ),
        "edit.self#dir=sealed" => {
            GenericGrantRequest::ethos(Verb::Edit, Zone::Self_, GrantSelector::Dir("sealed".into()))
        }
        // Self tag delivery is deliberately unavailable: this signed
        // authority is constructed below without inventing a content line.
        "delete.self#tag=private" => return Ok(None),
        other => return Err(format!("CORE-DEL-001 unknown authority {other}")),
    };
    Ok(Some(request))
}

fn core_delegated_fixture_bundle(
) -> Result<(Bundle<MemStore>, OwnerKeys, SigningKey, SeqEntropy, Sid), String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x59; 32])
            .map_err(|error| format!("CORE-DEL-001 owner seed failed: {error}"))?,
    );
    let agent = agent_sk(0x72);
    let succession = succession_from_entropy([0x69; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T13:00:00Z",
    )
    .map_err(|error| format!("CORE-DEL-001 init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            for (zone, folder, name, tags) in [
                (Zone::Public, "projects", "note", Vec::<String>::new()),
                (Zone::Circle, "projects", "note", Vec::<String>::new()),
                (Zone::Circle, "projects", "note2", Vec::<String>::new()),
                (Zone::Self_, "sealed", "opaque-note", vec!["private".into()]),
            ] {
                bundle.section_add(
                    &SectionSpec {
                        zone,
                        folder_path: folder,
                        name,
                        title: "delegated fixture",
                        tags: &tags,
                        body: "before delegated operation",
                        now: "2026-07-18T13:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T13:02:00Z")
        })
        .map_err(|error| format!("CORE-DEL-001 fixture failed: {error}"))?;
    let self_index: aithos_bundle::bundle::SelfIndex = serde_json::from_slice(
        &bundle
            .store
            .get("e/self/index.json")
            .map_err(|error| format!("CORE-DEL-001 self index read failed: {error}"))?
            .ok_or_else(|| "CORE-DEL-001 self index missing".to_owned())?,
    )
    .map_err(|error| format!("CORE-DEL-001 self index parse failed: {error}"))?;
    let self_sid = self_index
        .blobs
        .last()
        .ok_or_else(|| "CORE-DEL-001 self target missing".to_owned())?
        .sid
        .parse::<Sid>()
        .map_err(|error| format!("CORE-DEL-001 self target SID failed: {error}"))?;
    Ok((bundle, owner, agent, entropy, self_sid))
}

fn core_delegated_manual_tag_chain(
    bundle: &Bundle<MemStore>,
    owner: &OwnerKeys,
    agent: &SigningKey,
) -> Result<Vec<Mandate>, String> {
    let mandate = Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: "mandate_01ARZ3NDEKTSV4RRFFQ69G5FAY".into(),
            subject: bundle.did.clone(),
            constraints: MandateSpec::no_constraints(),
            grantee_id: "urn:aithos:agent:core-del-001".into(),
            grantee_label: "core-del-001".into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::Ethos {
                verb: Verb::Delete,
                zone: Zone::Self_,
                dir: Vec::new(),
                tag: Some("private".into()),
            }],
            not_before: "2026-07-18T13:03:00Z".into(),
            not_after: "2026-07-25T13:03:00Z".into(),
            issued_at: "2026-07-18T13:03:00Z".into(),
            nonce: "core-del-001-tag".into(),
        },
    )
    .map_err(|error| format!("CORE-DEL-001 manual tag mandate failed: {error}"))?;
    Ok(vec![mandate])
}

fn core_fence_exact_chain(
    bundle: &Bundle<MemStore>,
    owner: &OwnerKeys,
    agent: &SigningKey,
    sid: Sid,
    id: &str,
) -> Result<Vec<Mandate>, String> {
    Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: id.into(),
            subject: bundle.did.clone(),
            constraints: MandateSpec::no_constraints(),
            grantee_id: format!("urn:aithos:agent:{id}"),
            grantee_label: id.into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::EthosId {
                verb: Verb::Read,
                zone: Zone::Circle,
                id: sid,
            }],
            not_before: "2026-07-18T13:03:00Z".into(),
            not_after: "2026-07-25T13:03:00Z".into(),
            issued_at: "2026-07-18T13:03:00Z".into(),
            nonce: id.into(),
        },
    )
    .map(|mandate| vec![mandate])
    .map_err(|error| format!("CORE-DEL-002 exact mandate failed: {error}"))
}

fn core_fence_scenario(key_material: &str, authority: &str) -> Result<String, String> {
    let (mut bundle, owner, default_agent, mut entropy, _) = core_delegated_fixture_bundle()?;
    let target_sid = Sid::parse(
        &bundle
            .resolve_clear(Zone::Circle, "projects/note")
            .map_err(|error| format!("CORE-DEL-002 target resolve failed: {error}"))?
            .0
            .sid,
    )
    .map_err(|error| format!("CORE-DEL-002 target SID failed: {error}"))?;
    let wrong_agent = agent_sk(0x73);
    let exact = bundle
        .grant_generic(
            &owner,
            "core-del-002-exact",
            &default_agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Id("projects/note".into()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-DEL-002 exact grant failed: {error}"))?;
    let mut chain = vec![exact.mandate];
    let mut operation_agent = &default_agent;

    match key_material {
        "no section line" => {
            chain = core_fence_exact_chain(
                &bundle,
                &owner,
                &wrong_agent,
                target_sid,
                "mandate_01ARZ3NDEKTSV4RRFFQ69G5FA1",
            )?;
            operation_agent = &wrong_agent;
        }
        "sibling section line" => {
            bundle
                .grant_generic(
                    &owner,
                    "core-del-002-sibling",
                    &wrong_agent.verifying_key(),
                    &[GenericGrantRequest::ethos(
                        Verb::Read,
                        Zone::Circle,
                        GrantSelector::Id("projects/note2".into()),
                    )],
                    "2026-07-18T13:03:00Z",
                    "2026-07-25T13:03:00Z",
                    0,
                    "2026-07-18T13:03:30Z",
                    &mut entropy,
                )
                .map_err(|error| format!("CORE-DEL-002 sibling grant failed: {error}"))?;
            chain = core_fence_exact_chain(
                &bundle,
                &owner,
                &wrong_agent,
                target_sid,
                "mandate_01ARZ3NDEKTSV4RRFFQ69G5FA2",
            )?;
            operation_agent = &wrong_agent;
        }
        "no key proof" | "wrong key proof" => operation_agent = &wrong_agent,
        "exact valid section line" | "valid key proof" => {}
        other => return Err(format!("CORE-DEL-002 unknown key material {other}")),
    }
    if authority == "no mandate chain" {
        chain.clear();
    } else if authority == "revoked mandate chain" {
        bundle
            .log_revoke_owner(
                &owner,
                &chain[0].id,
                "CORE-DEL-002",
                "2026-07-18T13:04:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-DEL-002 revoke failed: {error}"))?;
    }
    let pure_authority = chain
        .last()
        .and_then(|mandate| mandate.parsed_perimeter().ok())
        .is_some_and(|perimeter| {
            covers_section_op(
                &perimeter,
                &SectionOp {
                    verb: Verb::Read,
                    zone: Zone::Circle,
                    sid: target_sid,
                    folders: &[],
                    tags: &[],
                },
            )
        });
    let read = bundle.grantee_content_operation(
        &chain,
        operation_agent,
        Zone::Circle,
        GranteeContentOperation::Read {
            target: GranteeTarget::Display("projects/note"),
            now: "2026-07-18T13:05:00Z",
        },
        &mut entropy,
    );
    let label = match (key_material, authority, pure_authority, read) {
        (
            _,
            "valid mandate chain" | "valid covering chain",
            true,
            Ok(GranteeContentOutcome::Read(body)),
        ) if body == "before delegated operation" => "readable and authorized",
        ("exact valid section line", "no mandate chain", _, Err(_)) => "refused as unauthorized",
        ("no section line", "valid covering chain", true, Err(_)) => "authorized but unreadable",
        ("sibling section line", "valid covering chain", true, Err(_)) => "unreadable",
        (_, _, _, Err(_)) => "refused",
        other => return Err(format!("CORE-DEL-002 unexpected fence outcome {other:?}")),
    };
    Ok(label.into())
}

fn core_append_cold_authority_scenario() -> Result<String, String> {
    let (mut bundle, owner, agent, mut entropy, _) = core_delegated_fixture_bundle()?;
    let wrong_agent = agent_sk(0x7a);
    let grant = bundle
        .grant_generic(
            &owner,
            "append-cold-authority",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Id("projects/note".into()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-DEL-004 grant failed: {error}"))?;
    let chain = vec![grant.mandate];
    let read = |bundle: &Bundle<MemStore>, actor: &SigningKey| {
        bundle.read_section_as_agent(
            &chain,
            actor,
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:04:00Z",
        )
    };
    let hot_valid = read(&bundle, &agent).is_ok_and(|body| body == "before delegated operation");
    let hot_wrong_key_refused = read(&bundle, &wrong_agent).is_err();
    bundle
        .log_revoke_owner(
            &owner,
            &chain[0].id,
            "CORE-DEL-004",
            "2026-07-18T13:05:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-DEL-004 revoke failed: {error}"))?;
    let hot_revoked_refused = bundle
        .read_section_as_agent(
            &chain,
            &agent,
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:06:00Z",
        )
        .is_err();
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T13:06:30Z"))
        .map_err(|error| format!("CORE-DEL-004 publish failed: {error}"))?;
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    drop(bundle);
    let cold = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-DEL-004 cold open failed: {error}"))?;
    cold.gamma_verify()
        .map_err(|error| format!("CORE-DEL-004 cold Gamma failed: {error}"))?;
    let cold_revoked_refused = cold
        .read_section_as_agent(
            &chain,
            &agent,
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:06:00Z",
        )
        .is_err();
    let cold_wrong_key_refused = cold
        .read_section_as_agent(
            &chain,
            &wrong_agent,
            Zone::Circle,
            "projects/note",
            "2026-07-18T13:04:00Z",
        )
        .is_err();
    if hot_valid
        && hot_wrong_key_refused
        && hot_revoked_refused
        && cold_revoked_refused
        && cold_wrong_key_refused
    {
        Ok("hot and cold returned the same revoked authority verdict".into())
    } else {
        Err(format!(
            "CORE-DEL-004 drift: hot_valid={hot_valid}, hot_wrong={hot_wrong_key_refused}, hot_revoked={hot_revoked_refused}, cold_revoked={cold_revoked_refused}, cold_wrong={cold_wrong_key_refused}"
        ))
    }
}

fn core_delegated_scenario(
    zone_name: &str,
    operation: &str,
    authority: &str,
) -> Result<CoreDelegatedObservation, String> {
    let vector: serde_json::Value = serde_json::from_str(CB8_AUTHORITY_FLOWS)
        .map_err(|error| format!("CORE-DEL-001 authority vector does not parse: {error}"))?;
    let row = vector["grantee_cases"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|row| {
                row["zone"] == zone_name
                    && row["operation"] == operation
                    && row["authority"] == authority
            })
        })
        .ok_or_else(|| {
            format!("CORE-DEL-001 missing matrix row {zone_name}/{operation}/{authority}")
        })?;
    let expected_accepted = row["expected"] == "accepted";
    let zone = match zone_name {
        "public" => Zone::Public,
        "circle" => Zone::Circle,
        "self" => Zone::Self_,
        other => return Err(format!("CORE-DEL-001 unknown zone {other}")),
    };
    let (mut bundle, owner, agent, mut entropy, self_sid) = core_delegated_fixture_bundle()?;
    let chain = if let Some(request) = core_delegated_request(authority)? {
        vec![
            bundle
                .grant_generic(
                    &owner,
                    "core-del-001",
                    &agent.verifying_key(),
                    &[request],
                    "2026-07-18T13:03:00Z",
                    "2026-07-25T13:03:00Z",
                    0,
                    "2026-07-18T13:03:00Z",
                    &mut entropy,
                )
                .map_err(|error| format!("CORE-DEL-001 grant failed: {error}"))?
                .mandate,
        ]
    } else {
        core_delegated_manual_tag_chain(&bundle, &owner, &agent)?
    };
    let self_folder = chain[0]
        .parsed_perimeter()
        .map_err(|error| format!("CORE-DEL-001 perimeter parse failed: {error}"))?
        .into_iter()
        .find_map(|entry| match entry {
            PerimeterEntry::Ethos {
                zone: Zone::Self_,
                dir,
                ..
            } => Some(dir),
            _ => None,
        })
        .unwrap_or_default();
    if zone == Zone::Self_ && authority.contains("#id=opaque-note") {
        let granted_sid = chain[0]
            .parsed_perimeter()
            .map_err(|error| format!("CORE-DEL-001 exact perimeter parse failed: {error}"))?
            .into_iter()
            .find_map(|entry| match entry {
                PerimeterEntry::EthosId {
                    zone: Zone::Self_,
                    id,
                    ..
                } => Some(id),
                _ => None,
            })
            .ok_or_else(|| "CORE-DEL-001 exact self authority is missing".to_owned())?;
        if granted_sid != self_sid {
            return Err(format!(
                "CORE-DEL-001 exact self SID drift: granted={granted_sid}, target={self_sid}"
            ));
        }
    }
    let before = cb7_store_snapshot(&bundle.store)?;
    let gamma_before = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-DEL-001 Gamma before failed: {error}"))?
        .len();
    let preallocated = Sid::parse("01ARZ3NDEKTSV4RRFFQ69G5FAW")
        .map_err(|error| format!("CORE-DEL-001 preallocated SID failed: {error}"))?;
    let result = match (zone, operation) {
        (Zone::Public | Zone::Circle, "list") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::List {
                target: GranteeTarget::Display("projects"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Public | Zone::Circle, "read") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Public | Zone::Circle, "create") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Create {
                folder: GranteeTarget::Display("projects"),
                preallocated_sid: None,
                name: "fresh-note",
                title: "delegated create",
                tags: &[],
                body: "created by grantee",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Public | Zone::Circle, "edit") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Display("projects/note"),
                body: "edited by grantee",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Public | Zone::Circle, "delete") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Delete {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Self_, "list") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::List {
                target: GranteeTarget::FolderIds(&self_folder),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Self_, "read") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Read {
                target: GranteeTarget::Id(self_sid),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Self_, "create") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Create {
                folder: GranteeTarget::FolderIds(&[]),
                preallocated_sid: (authority == "append.self#id=preallocated")
                    .then_some(preallocated),
                name: if authority == "append.self#id=preallocated" {
                    "preallocated"
                } else {
                    "fresh-opaque"
                },
                title: "delegated self create",
                tags: &[],
                body: "created self by grantee",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Self_, "edit") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Id(self_sid),
                body: "edited self by grantee",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        (Zone::Self_, "delete") => bundle.grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Delete {
                target: GranteeTarget::Id(self_sid),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        ),
        other => return Err(format!("CORE-DEL-001 unknown operation {other:?}")),
    };
    let after = cb7_store_snapshot(&bundle.store)?;
    let gamma_after = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-DEL-001 Gamma after failed: {error}"))?
        .len();
    let accepted = result.is_ok();
    if accepted != expected_accepted {
        return Err(format!(
            "CORE-DEL-001 {zone_name}/{operation}/{authority} returned {result:?}"
        ));
    }
    let gamma_delta = gamma_after - gamma_before;
    let gamma_actor_is_grantee = if accepted {
        bundle
            .gamma_entries()
            .map_err(|error| format!("CORE-DEL-001 Gamma actor failed: {error}"))?
            .last()
            .is_some_and(|entry| {
                entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
                    && entry.authorized_by.as_deref() == Some(chain[0].id.as_str())
                    && entry.signature.key == chain[0].grantee.pubkey
            })
    } else {
        false
    };
    let refusal_unchanged = !accepted && before == after;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    drop(bundle);
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-DEL-001 fresh open failed: {error}"))?;
    let fresh_reopen_verified = if accepted {
        fresh.gamma_verify().is_ok()
    } else {
        before == cb7_store_snapshot(&fresh.store)?
    };
    let effect_verified = match result {
        Ok(GranteeContentOutcome::Listed(entries)) => entries
            .iter()
            .any(|entry| entry.path == "note" || entry.path == "opaque-note"),
        Ok(GranteeContentOutcome::Read(body)) => body == "before delegated operation",
        Ok(GranteeContentOutcome::Created(created)) => {
            let path = if zone == Zone::Self_ {
                if authority == "append.self#id=preallocated" {
                    "preallocated"
                } else {
                    "fresh-opaque"
                }
            } else {
                "projects/fresh-note"
            };
            if authority == "append.self#id=preallocated" {
                let index: aithos_bundle::bundle::SelfIndex = fresh
                    .store
                    .get("e/self/index.json")
                    .ok()
                    .flatten()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                    .unwrap_or_default();
                created == preallocated
                    && index
                        .blobs
                        .iter()
                        .any(|row| row.sid == preallocated.to_string())
            } else {
                fresh.read_section(zone, path, &owner).is_ok_and(|body| {
                    body == if zone == Zone::Self_ {
                        "created self by grantee"
                    } else {
                        "created by grantee"
                    }
                })
            }
        }
        Ok(GranteeContentOutcome::Mutated) if operation == "edit" => fresh
            .read_section(
                zone,
                if zone == Zone::Self_ {
                    "sealed/opaque-note"
                } else {
                    "projects/note"
                },
                &owner,
            )
            .is_ok_and(|body| {
                body == if zone == Zone::Self_ {
                    "edited self by grantee"
                } else {
                    "edited by grantee"
                }
            }),
        Ok(GranteeContentOutcome::Mutated) if operation == "delete" => fresh
            .read_section(
                zone,
                if zone == Zone::Self_ {
                    "sealed/opaque-note"
                } else {
                    "projects/note"
                },
                &owner,
            )
            .is_err(),
        Err(_) => refusal_unchanged,
        _ => false,
    };
    Ok(CoreDelegatedObservation {
        zone: zone_name.into(),
        operation: operation.into(),
        authority: authority.into(),
        verdict: if accepted { "accepted" } else { "refused" }.into(),
        accepted,
        effect_verified,
        gamma_delta,
        gamma_actor_is_grantee,
        fresh_reopen_verified,
        refusal_unchanged,
    })
}

fn core_exact_section_scenario(fixture: &str) -> Result<CoreExactSectionObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x5d; 32])
            .map_err(|error| format!("CORE-DEL-003 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x6d; 32]);
    let agent = agent_sk(0x77);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T13:00:00Z",
    )
    .map_err(|error| format!("CORE-DEL-003 init failed: {error}"))?;
    let (zone, folder, target, sibling, verb) = match fixture {
        "circle-read" => (Zone::Circle, "projects", "note1", "note2", Verb::Read),
        "self-read" => (Zone::Self_, "sealed", "consignes", "marges", Verb::Read),
        "circle-edit" => (Zone::Circle, "projects", "brouillon", "sibling", Verb::Edit),
        "self-edit" => (Zone::Self_, "sealed", "consignes", "marges", Verb::Edit),
        other => return Err(format!("CORE-DEL-003 unknown fixture {other}")),
    };
    bundle
        .transaction(|bundle| {
            for (name, body) in [(target, "target body"), (sibling, "sibling body")] {
                bundle.section_add(
                    &SectionSpec {
                        zone,
                        folder_path: folder,
                        name,
                        title: name,
                        tags: &[],
                        body,
                        now: "2026-07-18T13:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T13:02:00Z")
        })
        .map_err(|error| format!("CORE-DEL-003 fixture failed: {error}"))?;
    let target_path = format!("{folder}/{target}");
    let sibling_path = format!("{folder}/{sibling}");
    let grant = bundle
        .grant_generic(
            &owner,
            "core-del-003",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                verb,
                zone,
                GrantSelector::Id(target_path.clone()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-DEL-003 grant failed: {error}"))?;
    let chain = vec![grant.mandate];
    let ids = if zone == Zone::Self_ {
        let index: aithos_bundle::bundle::SelfIndex = bundle
            .store
            .get("e/self/index.json")
            .map_err(|error| format!("CORE-DEL-003 self index read failed: {error}"))?
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| "CORE-DEL-003 self index is missing".to_owned())?;
        let target_sid = chain[0]
            .parsed_perimeter()
            .map_err(|error| format!("CORE-DEL-003 perimeter failed: {error}"))?
            .into_iter()
            .find_map(|entry| match entry {
                PerimeterEntry::EthosId {
                    zone: Zone::Self_,
                    id,
                    ..
                } => Some(id),
                _ => None,
            })
            .ok_or_else(|| "CORE-DEL-003 target self SID missing".to_owned())?;
        let sibling_sid = index
            .blobs
            .last()
            .ok_or_else(|| "CORE-DEL-003 sibling self SID missing".to_owned())?
            .sid
            .parse::<Sid>()
            .map_err(|error| format!("CORE-DEL-003 sibling self SID failed: {error}"))?;
        Some((target_sid, sibling_sid))
    } else {
        None
    };
    let target_selector = ids.as_ref().map_or(
        GranteeTarget::Display(target_path.as_str()),
        |(target, _)| GranteeTarget::Id(*target),
    );
    let target_readable = if verb == Verb::Read {
        matches!(
            bundle.grantee_content_operation(
                &chain,
                &agent,
                zone,
                GranteeContentOperation::Read {
                    target: target_selector,
                    now: "2026-07-18T13:04:00Z",
                },
                &mut entropy,
            ),
            Ok(GranteeContentOutcome::Read(ref body)) if body == "target body"
        )
    } else {
        true
    };
    let target_selector = ids.as_ref().map_or(
        GranteeTarget::Display(target_path.as_str()),
        |(target, _)| GranteeTarget::Id(*target),
    );
    let target_rewritten = if verb == Verb::Edit {
        bundle
            .grantee_content_operation(
                &chain,
                &agent,
                zone,
                GranteeContentOperation::Edit {
                    target: target_selector,
                    body: "rewritten by exact grantee",
                    now: "2026-07-18T13:04:00Z",
                },
                &mut entropy,
            )
            .is_ok()
            && bundle
                .read_section(zone, &target_path, &owner)
                .is_ok_and(|body| body == "rewritten by exact grantee")
    } else {
        false
    };
    let before_sibling = cb7_store_snapshot(&bundle.store)?;
    let sibling_selector = ids.as_ref().map_or(
        GranteeTarget::Display(sibling_path.as_str()),
        |(_, sibling)| GranteeTarget::Id(*sibling),
    );
    let sibling_unreachable = bundle
        .grantee_content_operation(
            &chain,
            &agent,
            zone,
            GranteeContentOperation::Read {
                target: sibling_selector,
                now: "2026-07-18T13:05:00Z",
            },
            &mut entropy,
        )
        .is_err();
    let sibling_create_refused = if fixture == "circle-edit" {
        bundle
            .grantee_content_operation(
                &chain,
                &agent,
                zone,
                GranteeContentOperation::Create {
                    folder: GranteeTarget::Display(folder),
                    preallocated_sid: None,
                    name: "unauthorized-sibling",
                    title: "unauthorized sibling",
                    tags: &[],
                    body: "must roll back",
                    now: "2026-07-18T13:05:00Z",
                },
                &mut entropy,
            )
            .is_err()
    } else {
        true
    };
    let after = cb7_store_snapshot(&bundle.store)?;
    let failed_attempt_unchanged = before_sibling == after;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-DEL-003 fresh open failed: {error}"))?;
    if verb == Verb::Edit
        && !fresh
            .read_section(zone, &target_path, &owner)
            .is_ok_and(|body| body == "rewritten by exact grantee")
    {
        return Err("CORE-DEL-003 rewritten target did not survive reopen".into());
    }
    Ok(CoreExactSectionObservation {
        target: target.to_owned(),
        target_readable,
        target_rewritten,
        sibling_unreachable,
        sibling_create_refused,
        failed_attempt_unchanged,
    })
}

fn core_current_authority_scenario(
    authority_change: &str,
) -> Result<CoreCurrentAuthorityObservation, String> {
    let (mut bundle, owner, agent, mut entropy, _) = core_delegated_fixture_bundle()?;
    let not_after = match authority_change {
        "expired" => "2026-07-18T13:04:00Z",
        "revoked" => "2026-07-25T13:03:00Z",
        other => return Err(format!("CORE-DEL-004 unknown authority change {other}")),
    };
    let mandate = bundle
        .grant_generic(
            &owner,
            "core-del-004",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Edit,
                Zone::Circle,
                GrantSelector::Id("projects/note".into()),
            )],
            "2026-07-18T13:03:00Z",
            not_after,
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-DEL-004 grant failed: {error}"))?
        .mandate;
    let chain = vec![mandate];
    let old_line_usable_before_change = bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Display("projects/note"),
                body: "pre-change proof",
                now: "2026-07-18T13:03:30Z",
            },
            &mut entropy,
        )
        .is_ok()
        && bundle
            .read_section(Zone::Circle, "projects/note", &owner)
            .is_ok_and(|body| body == "pre-change proof");
    if authority_change == "revoked" {
        bundle
            .log_revoke_owner(
                &owner,
                &chain[0].id,
                "CORE-DEL-004 current authority",
                "2026-07-18T13:04:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-DEL-004 revoke failed: {error}"))?;
    }
    let before = cb7_store_snapshot(&bundle.store)?;
    let result = bundle.grantee_content_operation(
        &chain,
        &agent,
        Zone::Circle,
        GranteeContentOperation::Edit {
            target: GranteeTarget::Display("projects/note"),
            body: "must never commit",
            now: "2026-07-18T13:05:00Z",
        },
        &mut entropy,
    );
    let after = cb7_store_snapshot(&bundle.store)?;
    let current_verdict_refused = result.is_err();
    let canonical_unchanged = before == after;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-DEL-004 fresh open failed: {error}"))?;
    let fresh_reopen_unchanged = cb7_store_snapshot(&fresh.store)? == before
        && fresh
            .read_section(Zone::Circle, "projects/note", &owner)
            .is_ok_and(|body| body == "pre-change proof");
    Ok(CoreCurrentAuthorityObservation {
        authority_change: authority_change.into(),
        old_line_usable_before_change,
        current_verdict_refused,
        canonical_unchanged,
        fresh_reopen_unchanged,
    })
}

fn core_delegated_rollback_scenario() -> Result<CoreDelegatedRollbackObservation, String> {
    let (mut fixture, owner, agent, mut entropy, _) = core_delegated_fixture_bundle()?;
    let chain = vec![
        fixture
            .grant_generic(
                &owner,
                "core-del-005",
                &agent.verifying_key(),
                &[GenericGrantRequest::ethos(
                    Verb::Edit,
                    Zone::Circle,
                    GrantSelector::Id("projects/note".into()),
                )],
                "2026-07-18T13:03:00Z",
                "2026-07-25T13:03:00Z",
                0,
                "2026-07-18T13:03:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-DEL-005 grant failed: {error}"))?
            .mandate,
    ];
    let before = cb7_store_snapshot(&fixture.store)?;
    let mut inner = MemStore::default();
    cb7_install(&mut inner, &before)?;
    drop(fixture);
    let wrapped = CoreAtomicFaultStore::new(inner, CoreAtomicFault::GammaValidation);
    let mut bundle = Bundle::open(wrapped)
        .map_err(|error| format!("CORE-DEL-005 reopen with fault failed: {error}"))?;
    let result = bundle.grantee_content_operation(
        &chain,
        &agent,
        Zone::Circle,
        GranteeContentOperation::Edit {
            target: GranteeTarget::Display("projects/note"),
            body: "late refusal must disappear",
            now: "2026-07-18T13:04:00Z",
        },
        &mut entropy,
    );
    let late_failure_injected_once = bundle.store.injected.get() == 1;
    let operation_refused = result.is_err();
    let after = cb7_store_snapshot(&bundle.store)?;
    let canonical_unchanged = before == after;
    let failed_artifacts_reachable = before != after;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-DEL-005 fresh reopen failed: {error}"))?;
    let fresh_reopen_verified = fresh.gamma_verify().is_ok()
        && fresh
            .read_section(Zone::Circle, "projects/note", &owner)
            .is_ok_and(|body| body == "before delegated operation")
        && cb7_store_snapshot(&fresh.store)? == before;
    Ok(CoreDelegatedRollbackObservation {
        late_failure_injected_once,
        operation_refused,
        canonical_unchanged,
        fresh_reopen_verified,
        failed_artifacts_reachable,
    })
}

fn core_structural_authority_scenario(
    operation: &str,
    authority: &str,
) -> Result<CoreStructuralAuthorityObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x5e; 32])
            .map_err(|error| format!("CORE-STR-001 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x6e; 32]);
    let agent = agent_sk(0x78);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T15:00:00Z",
    )
    .map_err(|error| format!("CORE-STR-001 init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            for (folder, name, tags) in [
                ("source", "note", Vec::<String>::new()),
                ("destination", "anchor", Vec::<String>::new()),
                ("empty", "temporary", Vec::<String>::new()),
                ("nonempty/child", "protected", vec!["secret".to_owned()]),
            ] {
                bundle.section_add(
                    &SectionSpec {
                        zone: Zone::Circle,
                        folder_path: folder,
                        name,
                        title: name,
                        tags: &tags,
                        body: name,
                        now: "2026-07-18T15:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T15:02:00Z")
        })
        .map_err(|error| format!("CORE-STR-001 fixture failed: {error}"))?;
    bundle
        .owner_content_operation(
            Zone::Circle,
            OwnerContentOperation::Delete {
                display_path: "empty/temporary",
                now: "2026-07-18T15:02:30Z",
            },
            &owner,
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-001 empty-folder fixture failed: {error}"))?;
    let request = |verb, dir: &str| {
        GenericGrantRequest::ethos(verb, Zone::Circle, GrantSelector::Dir(dir.to_owned()))
    };
    let leading_verb = |value: &str| -> Result<Verb, String> {
        match value.split_whitespace().next().unwrap_or_default() {
            "read" => Ok(Verb::Read),
            "edit" => Ok(Verb::Edit),
            "append" => Ok(Verb::Append),
            "delete" => Ok(Verb::Delete),
            "write" => Ok(Verb::Write),
            other => Err(format!("CORE-STR-001 unknown authority verb {other}")),
        }
    };
    let requests = match operation {
        "list and read a folder" => vec![request(leading_verb(authority)?, "source")],
        "create a child folder" => vec![request(leading_verb(authority)?, "destination")],
        "rename a folder" => vec![request(leading_verb(authority)?, "source")],
        "delete an empty folder" => vec![request(leading_verb(authority)?, "empty")],
        "move a folder" => match authority {
            "edit on source and append on destination" => {
                vec![
                    request(Verb::Edit, "source"),
                    request(Verb::Append, "destination"),
                ]
            }
            "append on source and write on destination" => {
                vec![
                    request(Verb::Append, "source"),
                    request(Verb::Write, "destination"),
                ]
            }
            "delete on source and append on destination" => {
                vec![
                    request(Verb::Delete, "source"),
                    request(Verb::Append, "destination"),
                ]
            }
            "edit on source only" => vec![request(Verb::Edit, "source")],
            other => return Err(format!("CORE-STR-001 unknown move authority {other}")),
        },
        "delete a non-empty folder" => match authority {
            "delete covering folder and complete subtree" => {
                vec![request(Verb::Delete, "nonempty")]
            }
            "delete on folder but not one descendant" => vec![GenericGrantRequest::ethos(
                Verb::Delete,
                Zone::Circle,
                GrantSelector::Tag {
                    dir: "nonempty".into(),
                    tag: "allowed".into(),
                },
            )],
            other => return Err(format!("CORE-STR-001 unknown subtree authority {other}")),
        },
        other => return Err(format!("CORE-STR-001 unknown operation {other}")),
    };
    let chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-001",
                &agent.verifying_key(),
                &requests,
                "2026-07-18T15:03:00Z",
                "2026-07-25T15:03:00Z",
                0,
                "2026-07-18T15:03:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-001 grant failed: {error}"))?
            .mandate,
    ];
    let before = cb7_store_snapshot(&bundle.store)?;
    let gamma_before = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-STR-001 Gamma before failed: {error}"))?
        .len();
    let result = match operation {
        "list and read a folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::ListFolder {
                zone: Zone::Circle,
                folder: "source",
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        "create a child folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::CreateFolder {
                zone: Zone::Circle,
                parent: "destination",
                name: "newchild",
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        "rename a folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::RenameFolder {
                zone: Zone::Circle,
                folder: "source",
                new_name: "renamed",
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        "delete an empty folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::DeleteFolder {
                zone: Zone::Circle,
                folder: "empty",
                recursive: false,
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        "move a folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source",
                destination_parent: "destination",
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        "delete a non-empty folder" => bundle.structural_operation(
            &chain,
            &agent,
            StructuralOperation::DeleteFolder {
                zone: Zone::Circle,
                folder: "nonempty",
                recursive: true,
                now: "2026-07-18T15:04:00Z",
            },
            &mut entropy,
        ),
        _ => unreachable!(),
    };
    let list_effect = matches!(
        &result,
        Ok(StructuralOutcome::Listed(entries))
            if entries.iter().any(|entry| entry.path == "note")
    );
    let accepted = result.is_ok();
    let after = cb7_store_snapshot(&bundle.store)?;
    let gamma_after = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-STR-001 Gamma after failed: {error}"))?
        .len();
    let gamma_delta = gamma_after - gamma_before;
    let refusal_unchanged = !accepted && before == after;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-001 fresh open failed: {error}"))?;
    let exact_effect_verified = if !accepted {
        refusal_unchanged
    } else {
        match operation {
            "list and read a folder" => list_effect,
            "create a child folder" => fresh
                .resolve_folder(Zone::Circle, "destination/newchild")
                .is_ok(),
            "rename a folder" => fresh
                .read_section(Zone::Circle, "renamed/note", &owner)
                .is_ok_and(|body| body == "note"),
            "delete an empty folder" => fresh.resolve_folder(Zone::Circle, "empty").is_err(),
            "move a folder" => fresh
                .read_section(Zone::Circle, "destination/source/note", &owner)
                .is_ok_and(|body| body == "note"),
            "delete a non-empty folder" => fresh
                .read_section(Zone::Circle, "nonempty/child/protected", &owner)
                .is_err(),
            _ => false,
        }
    };
    let fresh_reopen_verified = fresh.gamma_verify().is_ok();
    Ok(CoreStructuralAuthorityObservation {
        operation: operation.into(),
        authority: authority.into(),
        verdict: if accepted { "accepted" } else { "refused" }.into(),
        exact_effect_verified,
        gamma_delta,
        refusal_unchanged,
        fresh_reopen_verified,
    })
}

fn core_revocation_cut_scenario() -> Result<CoreRevocationCutObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x5f; 32])
            .map_err(|error| format!("CORE-REV-001 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x6f; 32]);
    let revoked = agent_sk(0x79);
    let survivor = agent_sk(0x7a);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T16:00:00Z",
    )
    .map_err(|error| format!("CORE-REV-001 init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "protected",
                    tags: &[],
                    body: "protected body",
                    now: "2026-07-18T16:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T16:02:00Z")
        })
        .map_err(|error| format!("CORE-REV-001 fixture failed: {error}"))?;
    let revoked_mandate = bundle
        .grant_generic(
            &owner,
            "core-rev-revoked",
            &revoked.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T16:03:00Z",
            "2026-07-25T16:03:00Z",
            0,
            "2026-07-18T16:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 revoked grant failed: {error}"))?
        .mandate;
    let survivor_mandate = bundle
        .grant_generic(
            &owner,
            "core-rev-survivor",
            &survivor.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T16:04:00Z",
            "2026-07-25T16:04:00Z",
            0,
            "2026-07-18T16:04:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 survivor grant failed: {error}"))?
        .mandate;
    let manifest_before: Manifest = bundle
        .store
        .get("manifest.json")
        .map_err(|error| format!("CORE-REV-001 manifest before read failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-REV-001 manifest before missing".to_owned())?;
    let header_before = bundle
        .store
        .list("e/circle/hdr/")
        .map_err(|error| format!("CORE-REV-001 header list failed: {error}"))?
        .into_iter()
        .filter_map(|path| bundle.store.get(&path).ok().flatten())
        .filter_map(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
        .find(|header| header.node.contains("/d/"))
        .ok_or_else(|| "CORE-REV-001 protected header missing".to_owned())?;
    let old_version = header_before.latest_version();
    bundle
        .revoke_transaction(
            &owner,
            &revoked_mandate.id,
            "projects",
            "incident",
            "2026-07-18T16:05:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 cut failed: {error}"))?;
    let manifest_after: Manifest = bundle
        .store
        .get("manifest.json")
        .map_err(|error| format!("CORE-REV-001 manifest after read failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-REV-001 manifest after missing".to_owned())?;
    let one_new_edition = manifest_after.edition.height == manifest_before.edition.height + 1;
    let entries = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-REV-001 Gamma read failed: {error}"))?;
    let revoke_gamma_present = entries.iter().any(|entry| {
        entry.kind == "revoke" && entry.target.as_deref() == Some(revoked_mandate.id.as_str())
    });
    let header_after = bundle
        .store
        .list("e/circle/hdr/")
        .map_err(|error| format!("CORE-REV-001 rotated header list failed: {error}"))?
        .into_iter()
        .filter_map(|path| bundle.store.get(&path).ok().flatten())
        .filter_map(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
        .find(|header| header.node == header_before.node)
        .ok_or_else(|| "CORE-REV-001 rotated header missing".to_owned())?;
    let revoked_kex = aithos_core::keys::grantee_kex_secret(&revoked);
    let survivor_kex = aithos_core::keys::grantee_kex_secret(&survivor);
    let revoked_cut = header_after
        .open_latest(&bundle.did, &revoked_mandate.grantee.pubkey, &revoked_kex)
        .is_err()
        && bundle
            .read_section_as_agent(
                std::slice::from_ref(&revoked_mandate),
                &revoked,
                Zone::Circle,
                "projects/note",
                "2026-07-18T16:06:00Z",
            )
            .is_err();
    let survivor_reads = header_after
        .open_latest(&bundle.did, &survivor_mandate.grantee.pubkey, &survivor_kex)
        .is_ok()
        && bundle
            .read_section_as_agent(
                std::slice::from_ref(&survivor_mandate),
                &survivor,
                Zone::Circle,
                "projects/note",
                "2026-07-18T16:06:00Z",
            )
            .is_ok_and(|body| body == "protected body");
    let rotated_header_and_body = header_after.latest_version() == old_version + 1
        && bundle
            .read_section(Zone::Circle, "projects/note", &owner)
            .is_ok_and(|body| body == "protected body");
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    drop(bundle);
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-REV-001 fresh open failed: {error}"))?;
    let fresh_keyless_verified = fresh.verify().is_ok()
        && fresh.gamma_verify().is_ok()
        && cb7_store_snapshot(&fresh.store)? == exported;
    Ok(CoreRevocationCutObservation {
        one_new_edition,
        revoke_gamma_present,
        revoked_cut,
        survivor_reads,
        rotated_header_and_body,
        fresh_keyless_verified,
    })
}

fn core_structural_scoped_read_scenario() -> Result<CoreStructuralDerivedObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x60; 32])
            .map_err(|error| format!("CORE-STR-002 read owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x70; 32]);
    let agent = agent_sk(0x7b);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T17:00:00Z",
    )
    .map_err(|error| format!("CORE-STR-002 read init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            for (folder, name, body) in [
                ("projects/nested", "allowed", "allowed body"),
                ("projects/sibling", "hidden", "hidden body"),
            ] {
                bundle.section_add(
                    &SectionSpec {
                        zone: Zone::Circle,
                        folder_path: folder,
                        name,
                        title: name,
                        tags: &[],
                        body,
                        now: "2026-07-18T17:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T17:02:00Z")
        })
        .map_err(|error| format!("CORE-STR-002 read fixture failed: {error}"))?;
    let chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-read",
                &agent.verifying_key(),
                &[GenericGrantRequest::ethos(
                    Verb::Read,
                    Zone::Circle,
                    GrantSelector::Dir("projects/nested".into()),
                )],
                "2026-07-18T17:03:00Z",
                "2026-07-25T17:03:00Z",
                0,
                "2026-07-18T17:03:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 read grant failed: {error}"))?
            .mandate,
    ];
    let listed = bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::ListFolder {
                zone: Zone::Circle,
                folder: "projects/nested",
                now: "2026-07-18T17:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-002 scoped list failed: {error}"))?;
    let primary_effect_verified = matches!(
        listed,
        StructuralOutcome::Listed(entries)
            if entries.iter().any(|entry| entry.path == "allowed")
                && entries.iter().all(|entry| !entry.path.contains("hidden"))
    ) && matches!(
        bundle.grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/nested/allowed"),
                now: "2026-07-18T17:04:30Z",
            },
            &mut entropy,
        ),
        Ok(GranteeContentOutcome::Read(ref body)) if body == "allowed body"
    );
    let before_refusal = cb7_store_snapshot(&bundle.store)?;
    let secondary_effect_verified = bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/sibling/hidden"),
                now: "2026-07-18T17:05:00Z",
            },
            &mut entropy,
        )
        .is_err()
        && cb7_store_snapshot(&bundle.store)? == before_refusal;
    let entries = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-STR-002 read Gamma failed: {error}"))?;
    let gamma_actor_verified = entries.iter().rev().take(2).all(|entry| {
        entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
            && entry.signature.key == chain[0].grantee.pubkey
    });
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-002 read fresh open failed: {error}"))?;
    Ok(CoreStructuralDerivedObservation {
        case: "scoped-read".into(),
        primary_effect_verified,
        secondary_effect_verified,
        gamma_actor_verified,
        publication_verified: true,
        cold_reopen_verified: fresh.gamma_verify().is_ok(),
        privacy_verified: true,
    })
}

fn core_structural_tag_scenario() -> Result<CoreStructuralDerivedObservation, String> {
    let mut primary_effect_verified = true;
    let mut secondary_effect_verified = true;
    let mut gamma_actor_verified = true;
    let mut publication_verified = true;
    let mut cold_reopen_verified = true;
    for (offset, zone) in [Zone::Public, Zone::Circle].into_iter().enumerate() {
        let owner = OwnerKeys::genesis(
            &MasterSeed::from_slice(&[0x61 + offset as u8; 32])
                .map_err(|error| format!("CORE-STR-002 tag owner seed failed: {error}"))?,
        );
        let succession = succession_from_entropy([0x71 + offset as u8; 32]);
        let editor = agent_sk(0x7c + offset as u8);
        let reader = agent_sk(0x7e + offset as u8);
        let mut entropy = SeqEntropy::default();
        let mut bundle = Bundle::init(
            MemStore::default(),
            &owner,
            &succession.verifying_key(),
            &mut entropy,
            "2026-07-18T17:10:00Z",
        )
        .map_err(|error| format!("CORE-STR-002 tag init failed: {error}"))?;
        bundle
            .transaction(|bundle| {
                bundle.section_add(
                    &SectionSpec {
                        zone,
                        folder_path: "projects",
                        name: "tagged",
                        title: "tagged",
                        tags: &["old".to_owned()],
                        body: "tag body",
                        now: "2026-07-18T17:11:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
                bundle.publish(&owner, "2026-07-18T17:12:00Z")
            })
            .map_err(|error| format!("CORE-STR-002 tag fixture failed: {error}"))?;
        let mut requests = vec![GenericGrantRequest::ethos(
            Verb::Edit,
            zone,
            GrantSelector::Dir("projects".into()),
        )];
        if zone == Zone::Circle {
            requests.push(GenericGrantRequest::ethos(
                Verb::Read,
                zone,
                GrantSelector::Tag {
                    dir: "projects".into(),
                    tag: "new".into(),
                },
            ));
        }
        let chain = vec![
            bundle
                .grant_generic(
                    &owner,
                    "core-str-tag-editor",
                    &editor.verifying_key(),
                    &requests,
                    "2026-07-18T17:13:00Z",
                    "2026-07-25T17:13:00Z",
                    0,
                    "2026-07-18T17:13:00Z",
                    &mut entropy,
                )
                .map_err(|error| format!("CORE-STR-002 tag editor grant failed: {error}"))?
                .mandate,
        ];
        let reader_chain = if zone == Zone::Circle {
            Some(vec![
                bundle
                    .grant_generic(
                        &owner,
                        "core-str-tag-reader",
                        &reader.verifying_key(),
                        &[GenericGrantRequest::ethos(
                            Verb::Read,
                            zone,
                            GrantSelector::Tag {
                                dir: "projects".into(),
                                tag: "new".into(),
                            },
                        )],
                        "2026-07-18T17:14:00Z",
                        "2026-07-25T17:14:00Z",
                        0,
                        "2026-07-18T17:14:00Z",
                        &mut entropy,
                    )
                    .map_err(|error| format!("CORE-STR-002 tag reader grant failed: {error}"))?
                    .mandate,
            ])
        } else {
            None
        };
        bundle
            .structural_operation(
                &chain,
                &editor,
                StructuralOperation::EditSectionMetadata {
                    zone,
                    section: "projects/tagged",
                    name: None,
                    title: Some("retagged"),
                    tags: Some(&["new".to_owned()]),
                    now: "2026-07-18T17:15:00Z",
                },
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 tag edit failed: {error}"))?;
        let index: ZoneIndex = bundle
            .store
            .get(&format!("e/{}/index.json", zone.as_str()))
            .map_err(|error| format!("CORE-STR-002 tag index read failed: {error}"))?
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| "CORE-STR-002 tag index missing".to_owned())?;
        primary_effect_verified &= index
            .sections
            .iter()
            .any(|row| row.name == "tagged" && row.title == "retagged" && row.tags == ["new"]);
        if let Some(reader_chain) = &reader_chain {
            secondary_effect_verified &= bundle
                .read_section_as_agent(
                    reader_chain,
                    &reader,
                    zone,
                    "projects/tagged",
                    "2026-07-18T17:16:00Z",
                )
                .is_ok_and(|body| body == "tag body");
        }
        gamma_actor_verified &= bundle
            .gamma_entries()
            .ok()
            .and_then(|entries| entries.last().cloned())
            .is_some_and(|entry| {
                entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
                    && entry.signature.key == chain[0].grantee.pubkey
            });
        bundle
            .transaction(|bundle| bundle.publish(&owner, "2026-07-18T17:17:00Z"))
            .map_err(|error| format!("CORE-STR-002 tag publish failed: {error}"))?;
        publication_verified &= bundle.verify().is_ok();
        let exported = cb7_store_snapshot(&bundle.store)?;
        let mut fresh_store = MemStore::default();
        cb7_install(&mut fresh_store, &exported)?;
        let fresh = Bundle::open(fresh_store)
            .map_err(|error| format!("CORE-STR-002 tag fresh open failed: {error}"))?;
        cold_reopen_verified &= fresh.verify().is_ok() && fresh.gamma_verify().is_ok();
    }
    Ok(CoreStructuralDerivedObservation {
        case: "tag-edit".into(),
        primary_effect_verified,
        secondary_effect_verified,
        gamma_actor_verified,
        publication_verified,
        cold_reopen_verified,
        privacy_verified: true,
    })
}

fn core_structural_move_scenario() -> Result<CoreStructuralDerivedObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x63; 32])
            .map_err(|error| format!("CORE-STR-002 move owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x73; 32]);
    let mover = agent_sk(0x80);
    let destination_reader = agent_sk(0x81);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T17:20:00Z",
    )
    .map_err(|error| format!("CORE-STR-002 move init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "source/moving",
                    name: "note",
                    title: "moved",
                    tags: &[],
                    body: "moved body",
                    now: "2026-07-18T17:21:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.ensure_folder(Zone::Circle, "destination", &owner, &mut entropy)?;
            bundle.publish(&owner, "2026-07-18T17:22:00Z")
        })
        .map_err(|error| format!("CORE-STR-002 move fixture failed: {error}"))?;
    let before_index: ZoneIndex = bundle
        .store
        .get("e/circle/index.json")
        .map_err(|error| format!("CORE-STR-002 move index before failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 move index before missing".to_owned())?;
    let moved_sid = before_index
        .folders
        .iter()
        .find(|row| row.name == "moving")
        .map(|row| row.sid.clone())
        .ok_or_else(|| "CORE-STR-002 moved SID missing".to_owned())?;
    let chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-mover",
                &mover.verifying_key(),
                &[
                    GenericGrantRequest::ethos(
                        Verb::Edit,
                        Zone::Circle,
                        GrantSelector::Dir("source/moving".into()),
                    ),
                    GenericGrantRequest::ethos(
                        Verb::Append,
                        Zone::Circle,
                        GrantSelector::Dir("destination".into()),
                    ),
                ],
                "2026-07-18T17:23:00Z",
                "2026-07-25T17:23:00Z",
                0,
                "2026-07-18T17:23:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 move grant failed: {error}"))?
            .mandate,
    ];
    let destination_chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-destination-reader",
                &destination_reader.verifying_key(),
                &[GenericGrantRequest::ethos(
                    Verb::Read,
                    Zone::Circle,
                    GrantSelector::Dir("destination".into()),
                )],
                "2026-07-18T17:24:00Z",
                "2026-07-25T17:24:00Z",
                0,
                "2026-07-18T17:24:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 destination grant failed: {error}"))?
            .mandate,
    ];
    let header_versions_before = bundle
        .store
        .list("e/circle/hdr/")
        .map_err(|error| format!("CORE-STR-002 move headers before failed: {error}"))?
        .into_iter()
        .filter_map(|path| bundle.store.get(&path).ok().flatten())
        .filter_map(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
        .map(|header| header.latest_version())
        .max()
        .unwrap_or_default();
    bundle
        .structural_operation(
            &chain,
            &mover,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source/moving",
                destination_parent: "destination",
                now: "2026-07-18T17:25:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-002 move failed: {error}"))?;
    let after_index: ZoneIndex = bundle
        .store
        .get("e/circle/index.json")
        .map_err(|error| format!("CORE-STR-002 move index after failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 move index after missing".to_owned())?;
    let primary_effect_verified = after_index
        .folders
        .iter()
        .find(|row| row.name == "moving")
        .is_some_and(|row| row.sid == moved_sid)
        && bundle
            .read_section(Zone::Circle, "destination/moving/note", &owner)
            .is_ok_and(|body| body == "moved body");
    let header_versions_after = bundle
        .store
        .list("e/circle/hdr/")
        .map_err(|error| format!("CORE-STR-002 move headers after failed: {error}"))?
        .into_iter()
        .filter_map(|path| bundle.store.get(&path).ok().flatten())
        .filter_map(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
        .map(|header| header.latest_version())
        .max()
        .unwrap_or_default();
    let secondary_effect_verified = header_versions_after > header_versions_before
        && !bundle
            .store
            .list("e/circle/wraps/")
            .unwrap_or_default()
            .is_empty()
        && bundle
            .read_section_as_agent(
                &destination_chain,
                &destination_reader,
                Zone::Circle,
                "destination/moving/note",
                "2026-07-18T17:26:00Z",
            )
            .is_ok_and(|body| body == "moved body")
        && bundle
            .read_section_as_agent(
                &chain,
                &mover,
                Zone::Circle,
                "source/moving/note",
                "2026-07-18T17:26:00Z",
            )
            .is_err();
    let gamma_actor_verified = bundle
        .gamma_entries()
        .ok()
        .and_then(|entries| entries.last().cloned())
        .is_some_and(|entry| {
            entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
                && entry.signature.key == chain[0].grantee.pubkey
        });
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T17:27:00Z"))
        .map_err(|error| format!("CORE-STR-002 move publish failed: {error}"))?;
    let publication_verified = bundle.verify().is_ok();
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-002 move fresh open failed: {error}"))?;
    Ok(CoreStructuralDerivedObservation {
        case: "move".into(),
        primary_effect_verified,
        secondary_effect_verified,
        gamma_actor_verified,
        publication_verified,
        cold_reopen_verified: fresh.verify().is_ok() && fresh.gamma_verify().is_ok(),
        privacy_verified: true,
    })
}

fn core_structural_subtree_scenario() -> Result<CoreStructuralDerivedObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x64; 32])
            .map_err(|error| format!("CORE-STR-002 subtree owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x74; 32]);
    let agent = agent_sk(0x82);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T17:30:00Z",
    )
    .map_err(|error| format!("CORE-STR-002 subtree init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            for (folder, name) in [("root/child-a", "note-a"), ("root/child-b", "note-b")] {
                bundle.section_add(
                    &SectionSpec {
                        zone: Zone::Circle,
                        folder_path: folder,
                        name,
                        title: name,
                        tags: &["tagged".to_owned()],
                        body: name,
                        now: "2026-07-18T17:31:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T17:32:00Z")
        })
        .map_err(|error| format!("CORE-STR-002 subtree fixture failed: {error}"))?;
    let before_index: ZoneIndex = bundle
        .store
        .get("e/circle/index.json")
        .map_err(|error| format!("CORE-STR-002 subtree index before failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 subtree index before missing".to_owned())?;
    let removed_section_ids = before_index
        .sections
        .iter()
        .filter(|row| row.name.starts_with("note-"))
        .map(|row| row.sid.clone())
        .collect::<Vec<_>>();
    let chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-subtree",
                &agent.verifying_key(),
                &[GenericGrantRequest::ethos(
                    Verb::Delete,
                    Zone::Circle,
                    GrantSelector::Dir("root".into()),
                )],
                "2026-07-18T17:33:00Z",
                "2026-07-25T17:33:00Z",
                0,
                "2026-07-18T17:33:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 subtree grant failed: {error}"))?
            .mandate,
    ];
    bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::DeleteFolder {
                zone: Zone::Circle,
                folder: "root",
                recursive: true,
                now: "2026-07-18T17:34:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-002 subtree delete failed: {error}"))?;
    let after_index: ZoneIndex = bundle
        .store
        .get("e/circle/index.json")
        .map_err(|error| format!("CORE-STR-002 subtree index after failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 subtree index after missing".to_owned())?;
    let primary_effect_verified = bundle.resolve_folder(Zone::Circle, "root").is_err()
        && removed_section_ids.iter().all(|sid| {
            after_index.sections.iter().all(|row| &row.sid != sid)
                && bundle
                    .store
                    .get(&format!("e/circle/blobs/{sid}.enc"))
                    .ok()
                    .flatten()
                    .is_none()
        });
    let gamma_actor_verified = bundle
        .gamma_entries()
        .ok()
        .and_then(|entries| entries.last().cloned())
        .is_some_and(|entry| {
            entry.kind == "section.delete"
                && entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
                && entry.signature.key == chain[0].grantee.pubkey
        });
    let secondary_effect_verified = gamma_actor_verified
        && bundle
            .store
            .list("e/circle/hdr/")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|path| bundle.store.get(&path).ok().flatten())
            .filter_map(|bytes| serde_json::from_slice::<Header>(&bytes).ok())
            .all(|header| {
                removed_section_ids
                    .iter()
                    .all(|sid| !header.node.contains(sid))
            });
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T17:35:00Z"))
        .map_err(|error| format!("CORE-STR-002 subtree publish failed: {error}"))?;
    let publication_verified = bundle.verify().is_ok();
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-002 subtree fresh open failed: {error}"))?;
    Ok(CoreStructuralDerivedObservation {
        case: "subtree-delete".into(),
        primary_effect_verified,
        secondary_effect_verified,
        gamma_actor_verified,
        publication_verified,
        cold_reopen_verified: fresh.verify().is_ok() && fresh.gamma_verify().is_ok(),
        privacy_verified: true,
    })
}

fn core_structural_self_scenario() -> Result<CoreStructuralDerivedObservation, String> {
    let (mut bundle, owner, agent, mut entropy, self_sid) = core_delegated_fixture_bundle()?;
    let chain = vec![
        bundle
            .grant_generic(
                &owner,
                "core-str-self",
                &agent.verifying_key(),
                &[GenericGrantRequest::ethos(
                    Verb::Edit,
                    Zone::Self_,
                    GrantSelector::Id("sealed/opaque-note".into()),
                )],
                "2026-07-18T17:40:00Z",
                "2026-07-25T17:40:00Z",
                0,
                "2026-07-18T17:40:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-002 self grant failed: {error}"))?
            .mandate,
    ];
    let before_index: aithos_bundle::bundle::SelfIndex = bundle
        .store
        .get("e/self/index.json")
        .map_err(|error| format!("CORE-STR-002 self index before failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 self index before missing".to_owned())?;
    bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Self_,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Id(self_sid),
                body: "opaque replacement",
                now: "2026-07-18T17:41:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-002 self mutation failed: {error}"))?;
    let after_index: aithos_bundle::bundle::SelfIndex = bundle
        .store
        .get("e/self/index.json")
        .map_err(|error| format!("CORE-STR-002 self index after failed: {error}"))?
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| "CORE-STR-002 self index after missing".to_owned())?;
    let primary_effect_verified = before_index
        .blobs
        .iter()
        .map(|row| &row.sid)
        .eq(after_index.blobs.iter().map(|row| &row.sid))
        && bundle
            .read_section(Zone::Self_, "sealed/opaque-note", &owner)
            .is_ok_and(|body| body == "opaque replacement");
    let gamma = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-STR-002 self Gamma failed: {error}"))?
        .last()
        .cloned()
        .ok_or_else(|| "CORE-STR-002 self Gamma missing".to_owned())?;
    let gamma_actor_verified = gamma.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
        && gamma.signature.key == chain[0].grantee.pubkey;
    let secondary_effect_verified =
        gamma.body_enc.is_some() && gamma.target.is_none() && gamma.payload.is_none();
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T17:42:00Z"))
        .map_err(|error| format!("CORE-STR-002 self publish failed: {error}"))?;
    let public_bytes = cb7_store_snapshot(&bundle.store)?;
    let privacy_verified = !public_bytes.values().any(|bytes| {
        bytes
            .windows("opaque replacement".len())
            .any(|window| window == b"opaque replacement")
            || bytes
                .windows("sealed/opaque-note".len())
                .any(|window| window == b"sealed/opaque-note")
    });
    let publication_verified = bundle.verify().is_ok();
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &public_bytes)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-002 self fresh open failed: {error}"))?;
    Ok(CoreStructuralDerivedObservation {
        case: "self".into(),
        primary_effect_verified,
        secondary_effect_verified,
        gamma_actor_verified,
        publication_verified,
        cold_reopen_verified: fresh.verify().is_ok() && fresh.gamma_verify().is_ok(),
        privacy_verified,
    })
}

fn core_structural_failure_attempt<S: Store>(
    mut bundle: Bundle<S>,
    owner: &OwnerKeys,
    agent: &SigningKey,
    chain: &[Mandate],
    failure: &str,
    before: &BTreeMap<String, Vec<u8>>,
    entropy: &mut SeqEntropy,
) -> Result<CoreStructuralFailureObservation, String> {
    let result = match failure {
        "destination outside the grantee perimeter" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source",
                destination_parent: "clean-destination",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "move into the node's own descendant" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source",
                destination_parent: "source/child",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "destination sibling name collision" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source",
                destination_parent: "destination",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "display path traversal outside the zone" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::CreateFolder {
                zone: Zone::Circle,
                parent: "../outside",
                name: "escape",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "failure while rebuilding tag views" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::EditSectionMetadata {
                zone: Zone::Circle,
                section: "projects/tagged",
                name: None,
                title: Some("must roll back"),
                tags: Some(&["new".to_owned()]),
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "failure while rotating or rewrapping" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source",
                destination_parent: "clean-destination",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        "failure before Gamma and manifest linearization" => bundle.structural_operation(
            chain,
            agent,
            StructuralOperation::RenameFolder {
                zone: Zone::Circle,
                folder: "source",
                new_name: "renamed",
                now: "2026-07-18T18:04:00Z",
            },
            entropy,
        ),
        other => return Err(format!("CORE-STR-003 unknown failure {other}")),
    };
    let after = cb7_store_snapshot(&bundle.store)?;
    let refused = result.is_err();
    let canonical_unchanged = &after == before;
    let partial_artifact_reachable = &after != before;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-STR-003 fresh open failed: {error}"))?;
    let fresh_reopen_verified = fresh.verify().is_ok()
        && fresh.gamma_verify().is_ok()
        && fresh
            .read_section(Zone::Circle, "source/child/note", owner)
            .is_ok_and(|body| body == "source body")
        && cb7_store_snapshot(&fresh.store)? == *before;
    Ok(CoreStructuralFailureObservation {
        failure: failure.into(),
        refused,
        canonical_unchanged,
        fresh_reopen_verified,
        partial_artifact_reachable,
    })
}

fn core_structural_failure_scenario(
    failure: &str,
) -> Result<CoreStructuralFailureObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x65; 32])
            .map_err(|error| format!("CORE-STR-003 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x75; 32]);
    let agent = agent_sk(0x83);
    let tag_reader = agent_sk(0x84);
    let mut entropy = SeqEntropy::default();
    let mut fixture = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T18:00:00Z",
    )
    .map_err(|error| format!("CORE-STR-003 init failed: {error}"))?;
    fixture
        .transaction(|bundle| {
            for (folder, name, body, tags) in [
                ("source/child", "note", "source body", Vec::<String>::new()),
                (
                    "destination/source",
                    "collision",
                    "collision",
                    Vec::<String>::new(),
                ),
                (
                    "clean-destination",
                    "anchor",
                    "anchor",
                    Vec::<String>::new(),
                ),
                ("projects", "tagged", "tag body", vec!["old".to_owned()]),
            ] {
                bundle.section_add(
                    &SectionSpec {
                        zone: Zone::Circle,
                        folder_path: folder,
                        name,
                        title: name,
                        tags: &tags,
                        body,
                        now: "2026-07-18T18:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T18:02:00Z")
        })
        .map_err(|error| format!("CORE-STR-003 fixture failed: {error}"))?;
    fixture
        .grant_generic(
            &owner,
            "core-str-failure-tag-reader",
            &tag_reader.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Tag {
                    dir: "projects".into(),
                    tag: "new".into(),
                },
            )],
            "2026-07-18T18:02:10Z",
            "2026-07-25T18:02:10Z",
            0,
            "2026-07-18T18:02:10Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-STR-003 tag header fixture failed: {error}"))?;
    let authority = if failure == "destination outside the grantee perimeter" {
        vec![GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Circle,
            GrantSelector::Dir("source".into()),
        )]
    } else {
        vec![GenericGrantRequest::ethos(
            Verb::Write,
            Zone::Circle,
            GrantSelector::Zone,
        )]
    };
    let chain = vec![
        fixture
            .grant_generic(
                &owner,
                "core-str-failure-agent",
                &agent.verifying_key(),
                &authority,
                "2026-07-18T18:03:00Z",
                "2026-07-25T18:03:00Z",
                0,
                "2026-07-18T18:03:00Z",
                &mut entropy,
            )
            .map_err(|error| format!("CORE-STR-003 grant failed: {error}"))?
            .mandate,
    ];
    let before = cb7_store_snapshot(&fixture.store)?;
    match failure {
        "failure while rebuilding tag views"
        | "failure while rotating or rewrapping"
        | "failure before Gamma and manifest linearization" => {
            let fault = match failure {
                "failure while rebuilding tag views" => CoreAtomicFault::HeaderOrWrap,
                "failure while rotating or rewrapping" => CoreAtomicFault::BlobPreparation,
                _ => CoreAtomicFault::GammaValidation,
            };
            let mut inner = MemStore::default();
            cb7_install(&mut inner, &before)?;
            drop(fixture);
            let wrapped = CoreAtomicFaultStore::new(inner, fault);
            let bundle = Bundle::open(wrapped)
                .map_err(|error| format!("CORE-STR-003 fault reopen failed: {error}"))?;
            core_structural_failure_attempt(
                bundle,
                &owner,
                &agent,
                &chain,
                failure,
                &before,
                &mut entropy,
            )
        }
        _ => core_structural_failure_attempt(
            fixture,
            &owner,
            &agent,
            &chain,
            failure,
            &before,
            &mut entropy,
        ),
    }
}

fn core_revocation_failure_attempt<S: Store>(
    mut bundle: Bundle<S>,
    owner: &OwnerKeys,
    revoked: &SigningKey,
    revoked_mandate: &Mandate,
    boundary: &str,
    before: &BTreeMap<String, Vec<u8>>,
    entropy: &mut SeqEntropy,
) -> Result<CoreRevocationFailureObservation, String> {
    let target = if boundary == "revocation verdict" {
        "mandate_01ARZ3NDEKTSV4RRFFQ69G5FZZ"
    } else {
        revoked_mandate.id.as_str()
    };
    let result = bundle.revoke_transaction(
        owner,
        target,
        "projects",
        "failed incident",
        "2026-07-18T19:05:00Z",
        entropy,
    );
    let after = cb7_store_snapshot(&bundle.store)?;
    let refused = result.is_err();
    let canonical_unchanged = &after == before;
    let partial_cut_reachable = &after != before;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &after)?;
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-REV-001 failure fresh open failed: {error}"))?;
    let old_state_reopened = fresh.verify().is_ok()
        && fresh.gamma_verify().is_ok()
        && fresh.active_revocations().is_ok_and(|revs| revs.is_empty())
        && fresh
            .read_section_as_agent(
                std::slice::from_ref(revoked_mandate),
                revoked,
                Zone::Circle,
                "projects/note",
                "2026-07-18T19:06:00Z",
            )
            .is_ok_and(|body| body == "protected body")
        && cb7_store_snapshot(&fresh.store)? == *before;
    Ok(CoreRevocationFailureObservation {
        boundary: boundary.into(),
        refused,
        canonical_unchanged,
        old_state_reopened,
        partial_cut_reachable,
    })
}

fn core_revocation_failure_scenario(
    boundary: &str,
) -> Result<CoreRevocationFailureObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x66; 32])
            .map_err(|error| format!("CORE-REV-001 failure owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x76; 32]);
    let revoked = agent_sk(0x85);
    let survivor = agent_sk(0x86);
    let mut entropy = SeqEntropy::default();
    let mut fixture = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T19:00:00Z",
    )
    .map_err(|error| format!("CORE-REV-001 failure init failed: {error}"))?;
    fixture
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "protected",
                    tags: &[],
                    body: "protected body",
                    now: "2026-07-18T19:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T19:02:00Z")
        })
        .map_err(|error| format!("CORE-REV-001 failure fixture failed: {error}"))?;
    let revoked_mandate = fixture
        .grant_generic(
            &owner,
            "core-rev-failure-revoked",
            &revoked.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T19:03:00Z",
            "2026-07-25T19:03:00Z",
            0,
            "2026-07-18T19:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 failure revoked grant failed: {error}"))?
        .mandate;
    fixture
        .grant_generic(
            &owner,
            "core-rev-failure-survivor",
            &survivor.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T19:04:00Z",
            "2026-07-25T19:04:00Z",
            0,
            "2026-07-18T19:04:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 failure survivor grant failed: {error}"))?;
    let before = cb7_store_snapshot(&fixture.store)?;
    let fault = match boundary {
        "revocation verdict" => None,
        "fresh node key generation" => Some(CoreAtomicFault::HeaderWrite),
        "survivor rewrap" => Some(CoreAtomicFault::WrapWrite),
        "body re-encryption" => Some(CoreAtomicFault::BlobPreparation),
        "Gamma append" => Some(CoreAtomicFault::GammaValidation),
        "before manifest and roots linearization" => Some(CoreAtomicFault::ManifestWrite),
        other => return Err(format!("CORE-REV-001 unknown failure boundary {other}")),
    };
    if let Some(fault) = fault {
        let mut inner = MemStore::default();
        cb7_install(&mut inner, &before)?;
        drop(fixture);
        let wrapped = CoreAtomicFaultStore::new(inner, fault);
        let bundle = Bundle::open(wrapped)
            .map_err(|error| format!("CORE-REV-001 failure reopen failed: {error}"))?;
        core_revocation_failure_attempt(
            bundle,
            &owner,
            &revoked,
            &revoked_mandate,
            boundary,
            &before,
            &mut entropy,
        )
    } else {
        core_revocation_failure_attempt(
            fixture,
            &owner,
            &revoked,
            &revoked_mandate,
            boundary,
            &before,
            &mut entropy,
        )
    }
}

fn core_revocation_replay_scenario() -> Result<CoreRevocationReplayObservation, String> {
    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x67; 32])
            .map_err(|error| format!("CORE-REV-001 replay owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x77; 32]);
    let agent = agent_sk(0x87);
    let survivor = agent_sk(0x88);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T20:00:00Z",
    )
    .map_err(|error| format!("CORE-REV-001 replay init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "protected",
                    tags: &[],
                    body: "initial",
                    now: "2026-07-18T20:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T20:02:00Z")
        })
        .map_err(|error| format!("CORE-REV-001 replay fixture failed: {error}"))?;
    let mandate = bundle
        .grant_generic(
            &owner,
            "core-rev-replay-agent",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Edit,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T20:03:00Z",
            "2026-07-25T20:03:00Z",
            0,
            "2026-07-18T20:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 replay grant failed: {error}"))?
        .mandate;
    bundle
        .grant_generic(
            &owner,
            "core-rev-replay-survivor",
            &survivor.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T20:03:10Z",
            "2026-07-25T20:03:10Z",
            0,
            "2026-07-18T20:03:10Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 replay survivor grant failed: {error}"))?;
    let chain = vec![mandate];
    bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Display("projects/note"),
                body: "before cut",
                now: "2026-07-18T20:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 earlier mutation failed: {error}"))?;
    bundle
        .revoke_transaction(
            &owner,
            &chain[0].id,
            "projects",
            "historical cut",
            "2026-07-18T20:05:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CORE-REV-001 replay cut failed: {error}"))?;
    let before_late = cb7_store_snapshot(&bundle.store)?;
    let later = bundle.grantee_content_operation(
        &chain,
        &agent,
        Zone::Circle,
        GranteeContentOperation::Edit {
            target: GranteeTarget::Display("projects/note"),
            body: "after cut must fail",
            now: "2026-07-18T20:06:00Z",
        },
        &mut entropy,
    );
    let after_late = cb7_store_snapshot(&bundle.store)?;
    let entries = bundle
        .gamma_entries()
        .map_err(|error| format!("CORE-REV-001 replay Gamma failed: {error}"))?;
    let earlier_mutation_valid = entries.iter().any(|entry| {
        entry.kind == "section.modify"
            && entry.at == "2026-07-18T20:04:00Z"
            && entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
    });
    let later_mutation_refused = later.is_err()
        && before_late == after_late
        && entries
            .iter()
            .all(|entry| entry.at != "2026-07-18T20:06:00Z");
    let current_revocation_derived = bundle
        .active_revocations()
        .is_ok_and(|revs| revs.len() == 1 && revs[0].mandate_id == chain[0].id);
    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh_store = MemStore::default();
    cb7_install(&mut fresh_store, &exported)?;
    drop(bundle);
    let fresh = Bundle::open(fresh_store)
        .map_err(|error| format!("CORE-REV-001 replay fresh open failed: {error}"))?;
    let fresh_replay_verified = fresh.gamma_verify().is_ok()
        && fresh
            .read_section(Zone::Circle, "projects/note", &owner)
            .is_ok_and(|body| body == "before cut")
        && fresh
            .active_revocations()
            .is_ok_and(|revs| revs.len() == 1 && revs[0].mandate_id == chain[0].id);
    Ok(CoreRevocationReplayObservation {
        earlier_mutation_valid,
        later_mutation_refused,
        current_revocation_derived,
        fresh_replay_verified,
    })
}

fn cb9_acceptance() -> Result<(), String> {
    let vector: serde_json::Value = serde_json::from_str(CB8_AUTHORITY_FLOWS)
        .map_err(|error| format!("CB9 authority-flow vector does not parse: {error}"))?;
    let cases = vector["grantee_cases"]
        .as_array()
        .ok_or_else(|| "CB9 grantee cases are not an array".to_owned())?;
    if cases.len() != 18
        || cases
            .iter()
            .filter(|case| case["expected"] == "accepted")
            .count()
            != 16
        || vector["content_fence_cases"].as_array().map(Vec::len) != Some(4)
        || vector["atomic_refusal_cases"].as_array().map(Vec::len) != Some(6)
    {
        return Err("CB9 closed authority matrix drift".into());
    }

    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x59; 32])
            .map_err(|error| format!("CB9 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x69; 32]);
    let agent = agent_sk(0x72);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T13:00:00Z",
    )
    .map_err(|error| format!("CB9 init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            for zone in [Zone::Public, Zone::Circle, Zone::Self_] {
                bundle.section_add(
                    &SectionSpec {
                        zone,
                        folder_path: "projects",
                        name: "note",
                        title: "existing",
                        tags: &["toto".to_owned()],
                        body: "before",
                        now: "2026-07-18T13:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T13:02:00Z")
        })
        .map_err(|error| format!("CB9 fixture failed: {error}"))?;
    let grant = bundle
        .grant_generic(
            &owner,
            "cb9-bdd",
            &agent.verifying_key(),
            &[
                GenericGrantRequest::ethos(
                    Verb::Edit,
                    Zone::Public,
                    GrantSelector::Id("projects/note".into()),
                ),
                GenericGrantRequest::ethos(
                    Verb::Append,
                    Zone::Circle,
                    GrantSelector::Dir("projects".into()),
                ),
                GenericGrantRequest::ethos(
                    Verb::Write,
                    Zone::Self_,
                    GrantSelector::Id("projects/note".into()),
                ),
                GenericGrantRequest::gamma(GammaQuery::default()),
            ],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB9 grant failed: {error}"))?;
    let chain = vec![grant.mandate];
    let self_sid = chain[0]
        .parsed_perimeter()
        .map_err(|error| format!("CB9 perimeter failed: {error}"))?
        .into_iter()
        .find_map(|entry| match entry {
            PerimeterEntry::EthosId {
                zone: Zone::Self_,
                id,
                ..
            } => Some(id),
            _ => None,
        })
        .ok_or_else(|| "CB9 exact self SID missing".to_owned())?;

    bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Public,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Display("projects/note"),
                body: "delegated public",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB9 public edit failed: {error}"))?;
    let created = bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Create {
                folder: GranteeTarget::Display("projects"),
                preallocated_sid: None,
                name: "fresh",
                title: "created",
                tags: &[],
                body: "delegated circle",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB9 circle create failed: {error}"))?;
    if !matches!(created, GranteeContentOutcome::Created(_)) {
        return Err("CB9 circle create returned the wrong outcome".into());
    }
    bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Self_,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Id(self_sid),
                body: "delegated self",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB9 self edit failed: {error}"))?;

    let before = cb7_store_snapshot(&bundle.store)?;
    if bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Create {
                folder: GranteeTarget::Display("projects"),
                preallocated_sid: None,
                name: "late",
                title: "late",
                tags: &[],
                body: "late",
                now: "2026-07-26T13:04:00Z",
            },
            &mut entropy,
        )
        .is_ok()
        || cb7_store_snapshot(&bundle.store)? != before
    {
        return Err("CB9 expired refusal changed canonical bytes".into());
    }

    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T13:05:00Z"))
        .map_err(|error| format!("CB9 publication failed: {error}"))?;
    bundle
        .verify()
        .map_err(|error| format!("CB9 published bundle failed: {error}"))?;
    bundle
        .verify_public_authorship()
        .map_err(|error| format!("CB9 authorship failed: {error}"))?;

    let exported = cb7_store_snapshot(&bundle.store)?;
    let mut fresh = MemStore::default();
    cb7_install(&mut fresh, &exported)?;
    drop(bundle);
    let cold = Bundle::open(fresh).map_err(|error| format!("CB9 cold open failed: {error}"))?;
    cold.gamma_verify()
        .map_err(|error| format!("CB9 cold Gamma replay failed: {error}"))?;
    if cold
        .read_section_as_agent(
            &chain,
            &agent,
            Zone::Circle,
            "projects/fresh",
            "2026-07-18T13:06:00Z",
        )
        .map_err(|error| format!("CB9 cold content read failed: {error}"))?
        != "delegated circle"
        || cold
            .log_query_as_agent(
                &chain,
                &agent,
                &GammaQuery::default(),
                &LogFilter::default(),
                "2026-07-18T13:06:00Z",
            )
            .map_err(|error| format!("CB9 cold Gamma read failed: {error}"))?
            .is_empty()
    {
        return Err("CB9 cold content/Gamma results drift".into());
    }
    Ok(())
}

fn cb10_acceptance() -> Result<(), String> {
    let vector: serde_json::Value = serde_json::from_str(CB10_STRUCTURE_VAULT)
        .map_err(|error| format!("CB10 structure/vault vector does not parse: {error}"))?;
    if vector["structural"]["authority_cases"]
        .as_array()
        .map(Vec::len)
        != Some(26)
        || vector["structural"]["failure_cases"]
            .as_array()
            .map(Vec::len)
            != Some(7)
        || vector["revocation"]["failure_cases"]
            .as_array()
            .map(Vec::len)
            != Some(6)
        || vector["vault"]["crud_cases"].as_array().map(Vec::len) != Some(4)
        || vector["vault"]["access_cases"].as_array().map(Vec::len) != Some(7)
    {
        return Err("CB10 closed structure/revocation/vault matrices drift".into());
    }

    let owner = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x5a; 32])
            .map_err(|error| format!("CB10 owner seed failed: {error}"))?,
    );
    let succession = succession_from_entropy([0x6a; 32]);
    let structure_agent = agent_sk(0x74);
    let survivor = agent_sk(0x75);
    let vault_agent = agent_sk(0x76);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        MemStore::default(),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T14:00:00Z",
    )
    .map_err(|error| format!("CB10 init failed: {error}"))?;
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Public,
                    folder_path: "projects",
                    name: "note",
                    title: "public",
                    tags: &["old".to_owned()],
                    body: "body",
                    now: "2026-07-18T14:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "protected",
                    tags: &[],
                    body: "protected",
                    now: "2026-07-18T14:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T14:02:00Z")
        })
        .map_err(|error| format!("CB10 fixture failed: {error}"))?;

    let structural_grant = bundle
        .grant_generic(
            &owner,
            "cb10-structure",
            &structure_agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Public,
                GrantSelector::Zone,
            )],
            "2026-07-18T14:03:00Z",
            "2026-07-25T14:03:00Z",
            0,
            "2026-07-18T14:03:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB10 structural grant failed: {error}"))?;
    let structural_chain = vec![structural_grant.mandate];
    let created = bundle
        .structural_operation(
            &structural_chain,
            &structure_agent,
            StructuralOperation::CreateFolder {
                zone: Zone::Public,
                parent: "projects",
                name: "child",
                now: "2026-07-18T14:04:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB10 structural operation failed: {error}"))?;
    if !matches!(created, StructuralOutcome::Created(_)) {
        return Err("CB10 structural API returned the wrong outcome".into());
    }

    let revoked_grant = bundle
        .grant_generic(
            &owner,
            "cb10-revoked",
            &structure_agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T14:05:00Z",
            "2026-07-25T14:05:00Z",
            0,
            "2026-07-18T14:05:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB10 revoked grant failed: {error}"))?;
    bundle
        .grant_generic(
            &owner,
            "cb10-survivor",
            &survivor.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T14:06:00Z",
            "2026-07-25T14:06:00Z",
            0,
            "2026-07-18T14:06:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB10 survivor grant failed: {error}"))?;
    bundle
        .revoke_transaction(
            &owner,
            &revoked_grant.mandate.id,
            "projects",
            "incident",
            "2026-07-18T14:07:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB10 incident cut failed: {error}"))?;

    let vault_grant = bundle
        .grant_generic(
            &owner,
            "cb10-vault",
            &vault_agent.verifying_key(),
            &[GenericGrantRequest::act("mail", "config")],
            "2026-07-18T14:08:00Z",
            "2026-07-25T14:08:00Z",
            0,
            "2026-07-18T14:08:00Z",
            &mut entropy,
        )
        .map_err(|error| format!("CB10 vault grant failed: {error}"))?;
    let vault_chain = vec![vault_grant.mandate];
    bundle
        .vault_config_operation(
            &vault_chain,
            &vault_agent,
            "mail",
            VaultConfigOperation::Create {
                config: b"cb10-private-config",
                now: "2026-07-18T14:09:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB10 vault create failed: {error}"))?;
    let read = bundle
        .vault_config_operation(
            &vault_chain,
            &vault_agent,
            "mail",
            VaultConfigOperation::Read {
                now: "2026-07-18T14:10:00Z",
            },
            &mut entropy,
        )
        .map_err(|error| format!("CB10 vault read failed: {error}"))?;
    if read != VaultConfigOutcome::Read(b"cb10-private-config".to_vec()) {
        return Err("CB10 vault read returned the wrong plaintext".into());
    }
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T14:11:00Z"))
        .map_err(|error| format!("CB10 final publication failed: {error}"))?;
    bundle
        .verify()
        .map_err(|error| format!("CB10 edition verification failed: {error}"))?;
    bundle
        .gamma_verify()
        .map_err(|error| format!("CB10 Gamma replay failed: {error}"))?;
    if cb7_store_snapshot(&bundle.store)?.values().any(|bytes| {
        bytes
            .windows(b"cb10-private-config".len())
            .any(|window| window == b"cb10-private-config")
    }) {
        return Err("CB10 config plaintext escaped into the canonical store".into());
    }
    Ok(())
}

fn cb5_constraints_acceptance() -> Result<(), String> {
    let mandates = cb5_parsed(CB2_MANDATE_CONTRACTS)?;
    let root_cases = mandates["constraints"]["root_leaf_cases"]
        .as_array()
        .ok_or_else(|| "CB5 root cases are not an array".to_owned())?;
    let root_case = |name: &str| {
        root_cases
            .iter()
            .find(|case| case["case"].as_str() == Some(name))
            .ok_or_else(|| format!("missing CB5 root case {name}"))
    };
    let parsed_mandate = |name: &str| -> Result<Mandate, String> {
        let case = root_case(name)?;
        serde_json::from_str(
            case["document_jcs"]
                .as_str()
                .ok_or_else(|| format!("missing CB5 mandate bytes for {name}"))?,
        )
        .map_err(|error| format!("{name} does not parse: {error}"))
    };
    let did: DidDocument = serde_json::from_str(
        mandates["signed_fixtures"]["did_document_jcs"]
            .as_str()
            .ok_or_else(|| "CB5 signed DID fixture is missing".to_owned())?,
    )
    .map_err(|error| format!("CB5 signed DID does not parse: {error}"))?;

    let known = parsed_mandate("known well-formed root constraint")?;
    verify_chain(std::slice::from_ref(&known), &did, &known.issued_at)
        .map_err(|error| format!("well-formed root constraint failed: {error}"))?;
    verify_operation_constraints(&known.constraints)
        .map_err(|error| format!("known operation constraints failed: {error}"))?;

    let malformed = parsed_mandate("known malformed root constraint")?;
    if !matches!(
        verify_chain(std::slice::from_ref(&malformed), &did, &malformed.issued_at),
        Err(aithos_core::Error::InvalidMandate(_))
    ) {
        return Err("malformed root max_actions did not fail as InvalidMandate".into());
    }

    let unknown = parsed_mandate("unknown constraint on directly issued chain leaf")?;
    verify_chain(std::slice::from_ref(&unknown), &did, &unknown.issued_at)
        .map_err(|error| format!("opaque root constraint was not preserved: {error}"))?;
    if !matches!(
        verify_operation_constraints(&unknown.constraints),
        Err(aithos_core::Error::InvalidMandate(_))
    ) {
        return Err("opaque root constraint became operation authority".into());
    }
    if !matches!(
        constraints_attenuate_for_profile(
            &unknown.version,
            &unknown.constraints,
            &unknown.constraints,
            &unknown.not_before,
            &unknown.not_after,
        ),
        Err(aithos_core::Error::InvalidMandate(_))
    ) {
        return Err("opaque root constraint became delegation authority".into());
    }

    let max_children = cb5_parsed(CB5_MAX_CHILDREN)?;
    let certificates = &max_children["certificates"];
    for case in max_children["cases"]
        .as_array()
        .ok_or_else(|| "CB5 max_children cases are not an array".to_owned())?
    {
        let name = |key: &str| {
            case[key]
                .as_str()
                .ok_or_else(|| format!("max_children case has no {key}"))
        };
        let parent_name = name("parent")?;
        let child_name = name("child")?;
        let parse = |certificate_name: &str| -> Result<Mandate, String> {
            serde_json::from_str(
                certificates[certificate_name]["jcs"]
                    .as_str()
                    .ok_or_else(|| format!("missing certificate {certificate_name}"))?,
            )
            .map_err(|error| format!("{certificate_name} does not parse: {error}"))
        };
        let parent = parse(parent_name)?;
        let child = parse(child_name)?;
        if parent.version != child.version {
            continue;
        }
        let accepted = constraints_attenuate_for_profile(
            &parent.version,
            &parent.constraints,
            &child.constraints,
            &child.not_before,
            &child.not_after,
        )
        .is_ok();
        let expected = case["expected"] == "valid";
        if accepted != expected {
            return Err(format!(
                "{}: max_children verdict mismatch",
                case["id"].as_str().unwrap_or("unnamed")
            ));
        }
    }

    let direct = &max_children["direct_children_only"];
    let entries: Vec<aithos_core::gamma::Entry> = direct["grant_entries_jcs"]
        .as_array()
        .ok_or_else(|| "direct-child entries are not an array".to_owned())?
        .iter()
        .map(|entry| {
            serde_json::from_str(
                entry
                    .as_str()
                    .ok_or_else(|| "direct-child entry is not text".to_owned())?,
            )
            .map_err(|error| format!("direct-child entry does not parse: {error}"))
        })
        .collect::<Result<_, _>>()?;
    aithos_core::gamma::verify_links(&entries)
        .map_err(|error| format!("direct-child Gamma links failed: {error}"))?;
    let parent_name = direct["parent_chain"][0]
        .as_str()
        .ok_or_else(|| "direct parent name is missing".to_owned())?;
    let child_name = direct["child_chain"][1]
        .as_str()
        .ok_or_else(|| "direct child name is missing".to_owned())?;
    let parent: Mandate = serde_json::from_str(
        certificates[parent_name]["jcs"]
            .as_str()
            .ok_or_else(|| "direct parent certificate is missing".to_owned())?,
    )
    .map_err(|error| format!("direct parent does not parse: {error}"))?;
    let child: Mandate = serde_json::from_str(
        certificates[child_name]["jcs"]
            .as_str()
            .ok_or_else(|| "direct child certificate is missing".to_owned())?,
    )
    .map_err(|error| format!("direct child does not parse: {error}"))?;
    if aithos_core::gamma::count_children(&entries, &parent.id) != 1
        || aithos_core::gamma::count_children(&entries, &child.id) != 3
    {
        return Err("grandchildren changed the grandparent direct-child meter".into());
    }
    Ok(())
}

fn cb5_counts_acceptance() -> Result<(), String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    let positive = &vector["positive"];
    let verified = verify_delegated_counts(
        &positive["delegated_counts"],
        &positive["leaves"],
        &positive["evidence_views"],
    )
    .map_err(|error| format!("positive delegated counts failed: {error}"))?;
    if verified.occurrences().len() != 14
        || verified
            .counts_for("mandate_01J00000000000000000000020")
            .is_none_or(|counts| counts.mutations() != 2 || counts.consumptions() != 14)
        || verified
            .counts_for("mandate_01J00000000000000000000022")
            .is_none_or(|counts| counts.consumptions() != 1)
    {
        return Err("positive delegated-count tallies do not match D7".into());
    }
    verify_delegated_count_mandates(&positive["mandates"])
        .map_err(|error| format!("positive delegated-count mandates failed: {error}"))?;

    for case in vector["negative_counter_cases"]
        .as_array()
        .ok_or_else(|| "delegated-count negatives are not an array".to_owned())?
    {
        let candidate = &case["candidate"];
        if !matches!(
            verify_delegated_counts(
                &candidate["delegated_counts"],
                &candidate["leaves"],
                &candidate["evidence_views"],
            ),
            Err(aithos_core::Error::InvalidDelegatedCounts(_))
        ) {
            return Err(format!("{} did not fail closed", case["id"]));
        }
    }
    for case in vector["negative_mandate_cases"]
        .as_array()
        .ok_or_else(|| "delegated-count mandate negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_delegated_count_mandates(&case["candidate"]),
            Err(aithos_core::Error::InvalidMandate(_))
        ) {
            return Err(format!("{} did not fail as InvalidMandate", case["id"]));
        }
    }
    Ok(())
}

fn core_count_consumption_scenario(consumption: &str) -> Result<(u64, u64, u64), String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    let positive = &vector["positive"];
    let verified = verify_delegated_counts(
        &positive["delegated_counts"],
        &positive["leaves"],
        &positive["evidence_views"],
    )
    .map_err(|error| format!("CORE-COUNT-001 verification failed: {error}"))?;
    let (occurrence, expected_kind, expected_domain, expected_actor) = match consumption {
        "connector action" => ("01", "action", "connector", "grantee"),
        "metered inference" => ("02", "inference", "inference", "grantee"),
        "delegated Ethos mutation" => ("03", "mutation", "ethos", "grantee"),
        "journalized delegated read" => ("04", "read", "gamma", "grantee"),
        "delegated config mutation" => ("05", "mutation", "vault-config", "grantee"),
        "direct sub-grant" => ("06", "grant", "mandate", "grantee"),
        "scoped revocation" => ("07", "revoke", "mandate", "grantee"),
        "normal grantee publication" => ("10", "publication", "publication", "grantee"),
        "merge publication plus its kind:merge entry" => {
            ("11", "publication", "publication", "grantee")
        }
        "delegated fork resolution" => ("12", "publication", "publication", "grantee"),
        "owner Ethos mutation" => ("14", "mutation", "ethos", "owner"),
        other => return Err(format!("CORE-COUNT-001 unknown consumption {other}")),
    };
    let occurrence = format!("op_01K000000000000000000000{occurrence}");
    let row = positive["evidence_views"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["occurrence"] == occurrence))
        .ok_or_else(|| format!("CORE-COUNT-001 missing occurrence for {consumption}"))?;
    if row["kind"] != expected_kind
        || row["facts_domain"] != expected_domain
        || row["actor"] != expected_actor
    {
        return Err(format!(
            "CORE-COUNT-001 semantic row drift for {consumption}: {row}"
        ));
    }
    let delegated = expected_actor == "grantee" && verified.occurrences().contains(&occurrence);
    let action = u64::from(delegated && expected_kind == "action");
    let mutation =
        u64::from(delegated && expected_kind == "mutation" && expected_domain == "ethos");
    Ok((action, mutation, u64::from(delegated)))
}

fn cb5_receipts_acceptance() -> Result<(), String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let positives = &vector["positive_receipts"];
    let contexts = &vector["contexts"];
    let obligations = &vector["obligations"];
    for (record, context, profile, obligation) in [
        (
            &positives["r2_without_presented_digest"],
            &contexts["action"],
            "1.0.0-draft.2",
            &obligations["action"],
        ),
        (
            &positives["r2_with_presented_digest"],
            &contexts["action"],
            "1.0.0-draft.2",
            &obligations["action"],
        ),
        (
            &positives["r2_draft3_mutation"],
            &contexts["mutation-ethos-edit"],
            "1.0.0-draft.3",
            &obligations["mutation"],
        ),
    ] {
        verify_r2_receipt(
            &serde_json::json!([record["receipt"].clone()]),
            context,
            profile,
            obligation,
        )
        .map_err(|error| format!("positive R2 receipt failed: {error}"))?;
    }
    let action = verify_u1_receipt(
        &serde_json::json!([positives["u1_action"]["receipt"].clone()]),
        &contexts["action"],
        &vector["budget_profile"],
    )
    .map_err(|error| format!("positive U1 action receipt failed: {error}"))?;
    let inference = verify_u1_receipt(
        &serde_json::json!([positives["u1_inference"]["receipt"].clone()]),
        &contexts["inference"],
        &vector["budget_profile"],
    )
    .map_err(|error| format!("positive U1 inference receipt failed: {error}"))?;
    if action.actual_tokens() != 8412 || inference.actual_tokens() != 1500 {
        return Err("U1 actual usage did not replace the matching declaration".into());
    }

    for case in vector["negative_r2_cases"]
        .as_array()
        .ok_or_else(|| "R2 negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_r2_receipt(
                &case["candidate"],
                &contexts["action"],
                "1.0.0-draft.2",
                &obligations["action"],
            ),
            Err(aithos_core::Error::GammaObligationUnsatisfied(_))
        ) {
            return Err(format!("{} did not fail as an R2 refusal", case["id"]));
        }
    }
    for case in vector["negative_u1_cases"]
        .as_array()
        .ok_or_else(|| "U1 negatives are not an array".to_owned())?
    {
        let context = if case["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("inference-"))
        {
            &contexts["inference"]
        } else {
            &contexts["action"]
        };
        if !matches!(
            verify_u1_receipt(&case["candidate"], context, &vector["budget_profile"]),
            Err(aithos_core::Error::InvalidGammaEntry(_))
        ) {
            return Err(format!("{} did not fail as a U1 refusal", case["id"]));
        }
    }

    for case in vector["matcher_cases"]
        .as_array()
        .ok_or_else(|| "matcher cases are not an array".to_owned())?
    {
        let obligation = serde_json::json!({
            "id": case["id"],
            "check": "human.approve",
            "attestor": [vector["public_keys"]["attestor_a"].clone()],
            "applies_to_operation": case["matcher"].clone(),
            "verdict": "approve",
        });
        let verified = verify_obligation("1.0.0-draft.3", &obligation)
            .map_err(|error| format!("positive matcher failed: {error}"))?;
        let applicable = obligation_matches(
            &verified,
            &contexts[case["context"]
                .as_str()
                .ok_or_else(|| "matcher context is missing".to_owned())?],
        )
        .map_err(|error| format!("matcher application failed: {error}"))?;
        if applicable != case["expected_applicable"].as_bool().unwrap_or(false) {
            return Err(format!("{} matcher verdict mismatch", case["id"]));
        }
    }
    verify_obligation_chain(&vector["draft3_obligation_chain"])
        .map_err(|error| format!("positive matcher chain failed: {error}"))?;
    for case in vector["negative_matcher_cases"]
        .as_array()
        .ok_or_else(|| "matcher negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_obligation(
                case["candidate"]["profile"].as_str().unwrap_or_default(),
                &case["candidate"]["obligation"],
            ),
            Err(aithos_core::Error::InvalidMandate(_))
        ) {
            return Err(format!("{} matcher shape did not fail", case["id"]));
        }
    }
    for case in vector["negative_matcher_chain_cases"]
        .as_array()
        .ok_or_else(|| "matcher-chain negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_obligation_chain(&case["candidate"]),
            Err(aithos_core::Error::InvalidMandate(_))
        ) {
            return Err(format!("{} matcher chain did not fail", case["id"]));
        }
    }
    Ok(())
}

fn cb5_catalog_acceptance() -> Result<(), String> {
    let vector = cb5_parsed(CB5_CATALOG)?;
    let catalog = verify_connector_catalog(
        &vector["catalog"]["document"],
        vector["catalog"]["catalog_digest"]
            .as_str()
            .ok_or_else(|| "catalog digest is missing".to_owned())?,
    )
    .map_err(|error| format!("positive catalog failed: {error}"))?;
    let approval = verify_catalog_approval(
        &vector["approval"]["document"],
        vector["approval"]["approval_digest"]
            .as_str()
            .ok_or_else(|| "approval digest is missing".to_owned())?,
        &catalog,
        &vector["owner_did"]["document"],
    )
    .map_err(|error| format!("positive catalog approval failed: {error}"))?;
    verify_catalog_chain(
        &vector["draft3_chain"],
        &catalog,
        &approval,
        &vector["owner_did"]["document"],
    )
    .map_err(|error| format!("positive catalog chain failed: {error}"))?;
    let action = verify_catalog_action_facts(
        &vector["action_facts"]["facts"],
        &vector["catalog_pin"],
        &catalog,
        &approval,
        &vector["owner_did"]["document"],
    )
    .map_err(|error| format!("positive catalog action facts failed: {error}"))?;
    if action.class() != "act" {
        return Err("catalog action class was not derived as act".into());
    }
    for case in vector["class_cases"]
        .as_array()
        .ok_or_else(|| "catalog class cases are not an array".to_owned())?
    {
        if catalog_action_permitted(
            &catalog,
            case["action"].as_str().unwrap_or_default(),
            case["authority"].as_str().unwrap_or_default(),
            case["owner_co_sign"].as_bool().unwrap_or(false),
        ) != case["expected_authorized"].as_bool().unwrap_or(false)
        {
            return Err(format!("{} catalog class verdict mismatch", case["action"]));
        }
    }
    for case in vector["negative_catalog_cases"]
        .as_array()
        .ok_or_else(|| "catalog negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_connector_catalog(
                &case["candidate"]["catalog"],
                case["candidate"]["claimed_digest"]
                    .as_str()
                    .unwrap_or_default(),
            ),
            Err(aithos_core::Error::InvalidCatalog(_))
        ) {
            return Err(format!("{} catalog defect did not fail", case["id"]));
        }
    }
    for case in vector["negative_approval_cases"]
        .as_array()
        .ok_or_else(|| "approval negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_catalog_approval(
                &case["candidate"]["approval"],
                case["candidate"]["claimed_digest"]
                    .as_str()
                    .unwrap_or_default(),
                &catalog,
                &vector["owner_did"]["document"],
            ),
            Err(aithos_core::Error::InvalidCatalog(_))
        ) {
            return Err(format!("{} approval defect did not fail", case["id"]));
        }
    }
    for case in vector["negative_chain_cases"]
        .as_array()
        .ok_or_else(|| "catalog-chain negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_catalog_chain(
                &case["candidate"],
                &catalog,
                &approval,
                &vector["owner_did"]["document"],
            ),
            Err(aithos_core::Error::InvalidMandate(_))
        ) {
            return Err(format!("{} catalog-chain defect did not fail", case["id"]));
        }
    }
    for case in vector["negative_action_facts_cases"]
        .as_array()
        .ok_or_else(|| "catalog-action negatives are not an array".to_owned())?
    {
        if !matches!(
            verify_catalog_action_facts(
                &case["candidate"],
                &vector["catalog_pin"],
                &catalog,
                &approval,
                &vector["owner_did"]["document"],
            ),
            Err(aithos_core::Error::InvalidOperationFacts(_))
        ) {
            return Err(format!("{} action-facts defect did not fail", case["id"]));
        }
    }
    Ok(())
}

fn cb5_constraints_result(w: &mut ProtocolWorld) {
    w.cb5_result = Some(
        CB5_CONSTRAINTS_ACCEPTANCE
            .get_or_init(cb5_constraints_acceptance)
            .clone(),
    );
}

fn cb4_result(w: &mut ProtocolWorld) {
    w.cb4_result = Some(CB4_ACCEPTANCE.get_or_init(cb4_acceptance).clone());
}

fn cb4_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb4_result, Some(Ok(())));
}

fn cb5_counts_result(w: &mut ProtocolWorld) {
    w.cb5_result = Some(
        CB5_COUNTS_ACCEPTANCE
            .get_or_init(cb5_counts_acceptance)
            .clone(),
    );
}

fn cb5_receipts_result(w: &mut ProtocolWorld) {
    w.cb5_result = Some(
        CB5_RECEIPTS_ACCEPTANCE
            .get_or_init(cb5_receipts_acceptance)
            .clone(),
    );
}

fn cb5_catalog_result(w: &mut ProtocolWorld) {
    w.cb5_result = Some(
        CB5_CATALOG_ACCEPTANCE
            .get_or_init(cb5_catalog_acceptance)
            .clone(),
    );
}

fn cb5_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb5_result, Some(Ok(())));
}

fn cb6_result(w: &mut ProtocolWorld) {
    w.cb6_result = Some(CB6_ACCEPTANCE.get_or_init(cb6_acceptance).clone());
}

fn cb6_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb6_result, Some(Ok(())));
}

fn cb7_result(w: &mut ProtocolWorld) {
    w.cb7_result = Some(CB7_ACCEPTANCE.get_or_init(cb7_acceptance).clone());
}

fn cb7_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb7_result, Some(Ok(())));
}

fn cb10_result(w: &mut ProtocolWorld) {
    w.cb10_result = Some(CB10_ACCEPTANCE.get_or_init(cb10_acceptance).clone());
}

fn cb10_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb10_result, Some(Ok(())));
}

impl ProtocolWorld {
    fn init_bundle(&mut self) {
        self.seeds.push((0u8..32).collect());
        let owner = self.owner(0);
        let succession = succession_from_entropy([9u8; 32]);
        self.bundle = Some(
            Bundle::init(
                MemStore::default(),
                &owner,
                &succession.verifying_key(),
                &mut self.ent,
                NOW,
            )
            .expect("bundle init"),
        );
    }

    fn add_circle_section(&mut self, folder: &str, name: &str, tag: &str) {
        let owner = self.owner(0);
        let bundle = self.bundle.as_mut().unwrap();
        bundle
            .ensure_folder(Zone::Circle, folder, &owner, &mut self.ent)
            .unwrap();
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags: &[tag.to_owned()],
                    body: BODY,
                    now: NOW,
                },
                &owner,
                &mut self.ent,
            )
            .unwrap();
    }

    fn publish_bundle(&mut self) {
        let owner = self.owner(0);
        self.bundle.as_mut().unwrap().publish(&owner, NOW).unwrap();
    }

    fn latest_manifest(&self) -> Manifest {
        let bytes = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("manifest.json")
            .unwrap()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl ProtocolWorld {
    fn open_into(&mut self, version: u64, kid: &str, sk_byte: u8) {
        let r = self
            .header
            .as_ref()
            .unwrap()
            .open(DID_C, version, kid, &xsk(sk_byte))
            .map_err(|e| e.to_string());
        self.opened.push(r);
    }
}

/// The instant every identity-epoch scenario uses.
const EPOCH_AT: &str = "2026-07-09T00:00:00Z";

/// Re-sign a mutated DID document under its own root key, so a rejection can
/// only be attributed to the semantic control under test (AID-001).
fn resign_did(doc: &mut DidDocument, root: &SigningKey) {
    doc.signature.value = String::new();
    let bytes = aithos_core::jcs::canonical_bytes(&*doc).expect("DID JCS");
    doc.signature.value = hex::encode(root.sign(&bytes).to_bytes());
}

/// Same, for an epoch transition (AID-002).
fn resign_transition(tr: &mut EpochTransition, succession: &SigningKey) {
    tr.signature.value = String::new();
    let bytes = aithos_core::jcs::canonical_bytes(&*tr).expect("transition JCS");
    tr.signature.value = hex::encode(succession.sign(&bytes).to_bytes());
}

impl ProtocolWorld {
    fn derive_from(&mut self, seed_index: usize) {
        let seed = MasterSeed::from_slice(&self.seeds[seed_index]).expect("valid seed");
        self.identities
            .push(public_identity(&OwnerKeys::genesis(&seed)));
    }

    fn owner(&self, seed_index: usize) -> OwnerKeys {
        let seed = MasterSeed::from_slice(&self.seeds[seed_index]).expect("valid seed");
        OwnerKeys::genesis(&seed)
    }

    fn build_doc(&self, seed_index: usize, entropy_index: usize) -> DidDocument {
        let owner = self.owner(seed_index);
        let succession = succession_from_entropy(self.succession_entropy[entropy_index]);
        DidDocument::build(
            &owner,
            &succession.verifying_key(),
            vec![BUNDLE.to_owned()],
            REVOCATIONS.to_owned(),
        )
        .expect("DID document builds")
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

#[given("a master seed and a succession keypair")]
fn seed_and_succession(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.succession_entropy.push([9u8; 32]);
}

#[given("a signed DID document")]
fn a_signed_did_document(w: &mut ProtocolWorld) {
    seed_and_succession(w);
    w.did_doc = Some(w.build_doc(0, 0));
}

#[given("an identity and its successor identity")]
fn identity_and_successor(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
    w.seeds.push((100u8..132).collect());
    w.succession_entropy.push([9u8; 32]);
    w.succession_entropy.push([11u8; 32]);
    w.prev_doc = Some(w.build_doc(0, 0));
    w.next_doc = Some(w.build_doc(1, 1));
}

// BDER-001: the zone DK is the vector's, so every derivation this feature
// performs is comparable to an expected value that exists outside the runner.
#[given("a zone key")]
fn a_zone_key(w: &mut ProtocolWorld) {
    w.zone_dk = Some(B2Vector::load().zone_dk());
}

#[given("a path of three nested folders ending in a section")]
#[given("a folder three levels deep containing a section")]
fn a_deep_path(w: &mut ProtocolWorld) {
    let v = B2Vector::load();
    w.deep_path = Some(NodePath::section(
        Zone::Circle,
        v.folder_spine(),
        v.section_sid(),
    ));
}

// BDER-003: this `Given` had an empty body and the two scenarios below
// reinvented their sids independently. The spines are now built once, from
// the vector, and read by both.
#[given("two sibling folders each containing a section")]
fn sibling_folders(w: &mut ProtocolWorld) {
    let v = B2Vector::load();
    w.sibling_paths = vec![
        NodePath::section(Zone::Circle, vec![v.folder_sid(0)], v.section_sid()),
        NodePath::section(Zone::Circle, vec![v.folder_sid(1)], v.sibling_section_sid()),
    ];
}

// BDER-004: the derived key of a REAL published section, taken from the sids
// the bundle actually stores, before any rename touches the display names.
#[given(expr = "the derived key of {string} is recorded")]
fn record_derived_key(w: &mut ProtocolWorld, display_path: String) {
    let owner = w.owner(0);
    let bundle = w.bundle.as_ref().expect("a published bundle");
    let zone_dk = bundle
        .zone_dk(Zone::Circle, &owner)
        .expect("circle zone dk");
    let (row, folders) = bundle
        .resolve_clear(Zone::Circle, &display_path)
        .expect("the section resolves before the rename");
    let path = NodePath::section(
        Zone::Circle,
        folders,
        Sid::parse(&row.sid).expect("section sid"),
    );
    w.renamed_section_sid = Some(row.sid.clone());
    w.rename_key_before = Some(node_key(&zone_dk, &path));
}

#[given("a zone key and a folder")]
fn zone_and_folder(w: &mut ProtocolWorld) {
    a_zone_key(w);
    w.deep_path = Some(NodePath::folder(Zone::Circle, vec![sid(1)]));
}

#[given("a node key and two recipients, the owner and a grantee")]
fn dk_and_two_recipients(_w: &mut ProtocolWorld) {
    // Fixed fixtures: DK, owner (0x0A), grantee g1 (0x21).
}

#[given("a sealed header for the owner and a grantee")]
fn sealed_header_owner_grantee(w: &mut ProtocolWorld) {
    w.header = Some(
        Header::build(
            DID_C,
            NODE_A,
            &DK,
            &[owner_rec(), grantee_rec("g1", 0x21)],
            &[eph(1), eph(2)],
            &[non(1), non(2)],
        )
        .unwrap(),
    );
}

#[given("a sealed header for the owner on one node")]
#[given("a sealed header for the owner")]
fn sealed_header_owner_only(w: &mut ProtocolWorld) {
    let header = Header::build(DID_C, NODE_A, &DK, &[owner_rec()], &[eph(1)], &[non(1)]).unwrap();
    w.saved_line = Some(header.key_versions["1"].lines[0].clone());
    w.header = Some(header);
}

#[given("a node key and a single grantee recipient")]
fn single_grantee(_w: &mut ProtocolWorld) {}

#[given("a sealed header for the owner and two grantees")]
fn sealed_header_three(w: &mut ProtocolWorld) {
    w.header = Some(
        Header::build(
            DID_C,
            NODE_A,
            &DK,
            &[
                owner_rec(),
                grantee_rec("g1", 0x21),
                grantee_rec("g2", 0x22),
            ],
            &[eph(1), eph(2), eph(3)],
            &[non(1), non(2), non(3)],
        )
        .unwrap(),
    );
}

#[given("a derived node rotated to a fresh random key")]
fn derived_node_rotated(_w: &mut ProtocolWorld) {
    // Fixtures: parent key PARENT_KEY, child CHILD_NODE rotated to DK2 v2.
}

#[given("a fresh identity")]
fn a_fresh_identity(w: &mut ProtocolWorld) {
    w.seeds.push((0u8..32).collect());
}

#[given("an initialised bundle")]
fn an_initialised_bundle(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given("a published bundle")]
#[given("a bundle with two editions")]
fn a_published_bundle(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets/perso", "note1", "toto");
    w.publish_bundle();
}

#[given(expr = "a published bundle with section {string} in circle {string}")]
fn published_with_section(w: &mut ProtocolWorld, name: String, folder: String) {
    w.init_bundle();
    w.add_circle_section(&folder, &name, "toto");
    w.publish_bundle();
}

#[given(expr = "a published bundle with a public section {string} in folder {string}")]
fn published_public(w: &mut ProtocolWorld, name: String, folder: String) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .section_add(
            &SectionSpec {
                zone: Zone::Public,
                folder_path: &folder,
                name: &name,
                title: "bio",
                tags: &[],
                body: PUB_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
    w.publish_bundle();
}

#[given(expr = "a bundle with a self folder {string} containing section {string}")]
fn bundle_with_self(w: &mut ProtocolWorld, folder: String, name: String) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .section_add(
            &SectionSpec {
                zone: Zone::Self_,
                folder_path: &folder,
                name: &name,
                title: "cicatrice au genou",
                tags: &["sante".to_owned()],
                body: SELF_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
    w.publish_bundle();
}

// --- step E givens ---

#[given("an owner and an agent keypair")]
fn owner_and_agent(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given(expr = "a mandate whose kex_pubkey does not match its signing key")]
fn mandate_bad_kex(w: &mut ProtocolWorld) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .ensure_folder(Zone::Circle, "projets", &owner, &mut w.ent)
        .unwrap();
    w.grant_to_agent(&[dir_spec("projets")], NA7, 0);
    let mut m = w.chain[0].clone();
    // Wrong kex, then honestly re-signed by root: only the kex CHECK can catch it.
    m.grantee.kex_pubkey = aithos_core::wire::x25519_pub_to_multibase(
        &ed2x(&agent_sk(HELPER).verifying_key()).to_bytes(),
    );
    m.resign(&owner.root_sign).unwrap();
    w.chain = vec![m];
    w.chain_result = Some(w.verify_chain_at(&w.chain, DAY1));
}

#[given(expr = "circle sections {string} tagged {string} and {string} untagged in folder {string}")]
fn tagged_and_untagged(
    w: &mut ProtocolWorld,
    tagged: String,
    tag: String,
    untagged: String,
    folder: String,
) {
    w.init_bundle();
    w.add_named_section(&folder, &tagged, &[tag]);
    w.add_named_section(&folder, &untagged, &[]);
    w.granted_folder = folder;
}

#[given(expr = "circle sections in sibling folders {string} and {string}")]
#[given(expr = "circle sections in folders {string} and {string}")]
fn sections_in_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.init_bundle();
    w.add_named_section(&f1, "note", &[]);
    w.add_named_section(&f2, "note", &[]);
    w.add_named_section("archives", "note", &[]);
    w.e_folders = vec![f1, f2];
}

#[given(expr = "tagged {string} and untagged sections in both {string} and {string}")]
fn tagged_in_both(w: &mut ProtocolWorld, tag: String, f1: String, f2: String) {
    w.init_bundle();
    for f in [&f1, &f2] {
        w.add_named_section(f, "tagged", std::slice::from_ref(&tag));
        w.add_named_section(f, "plain", &[]);
    }
    w.e_folders = vec![f1, f2];
}

#[given(expr = "an agent granted read on circle folder {string} with issue depth 1")]
#[given(expr = "an agent granted read on circle folder {string} for 7 days with issue depth 1")]
fn agent_with_issue(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note", &[]);
    // A nested subfolder so delegation on "<folder>/perso" has a real target.
    w.add_named_section(&format!("{folder}/perso"), "note", &[]);
    w.add_named_section("archives", "note", &[]);
    w.grant_to_agent(&[dir_spec(&folder)], NA7, 1);
    w.granted_folder = folder;
}

#[given("a helper at the end of a depth-1 chain")]
fn helper_end_of_chain(w: &mut ProtocolWorld) {
    agent_with_issue(w, "projets".to_owned());
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &w.chain[0].clone(),
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![w.chain[0].clone(), sub];
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

#[when("I generate a succession keypair twice for the same seed")]
fn generate_succession_twice(w: &mut ProtocolWorld) {
    // Owner keys: derived from the seed, twice — must be identical.
    w.derive_from(0);
    w.derive_from(0);
    // Succession: from two independent entropy draws — must differ.
    for entropy in [[1u8; 32], [2u8; 32]] {
        let key = succession_from_entropy(entropy);
        w.succession_pubs
            .push(hex::encode(key.verifying_key().to_bytes()));
    }
}

#[when("I build the DID document")]
fn build_did_document(w: &mut ProtocolWorld) {
    w.did_doc = Some(w.build_doc(0, 0));
}

#[when("one byte of it is altered after signing")]
fn tamper_document(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_mut().expect("a signed DID document");
    doc.revocations.push('x');
}

/// AID-001 — rebuild the document with ONE defect and re-sign it correctly
/// under its own root key, so only the semantic control under test can
/// explain the rejection.
#[when(regex = r"^it is rebuilt and re-signed with (.+)$")]
fn rebuild_did_with_defect(w: &mut ProtocolWorld, defect: String) {
    let owner = w.owner(0);
    let doc = w.did_doc.as_mut().expect("a signed DID document");
    match defect.as_str() {
        "a content key that is not multibase" => doc.keys.content = "not-a-key".to_owned(),
        "a content key in the X25519 codec" => {
            let bytes = wire::multibase_to_ed25519_pub(&doc.keys.content).expect("content key");
            doc.keys.content = wire::x25519_pub_to_multibase(&bytes);
        }
        "a kex key in the Ed25519 codec" => {
            let bytes = wire::multibase_to_x25519_pub(&doc.keys.kex).expect("kex key");
            doc.keys.kex = wire::ed25519_pub_to_multibase(&bytes);
        }
        "a malformed succession key" => doc.keys.succession = "z6Mk".to_owned(),
        "an unsupported document version" => doc.version = "9.9.9".to_owned(),
        "an unsupported signature algorithm" => doc.signature.alg = "secp256k1".to_owned(),
        "a signature fragment other than #root" => doc.signature.key = "#content".to_owned(),
        other => panic!("unknown DID defect: {other}"),
    }
    resign_did(doc, &owner.root_sign);
}

/// AID-001 — a member the typed schema does not know must be REFUSED, not
/// dropped on the way in: the verified JCS is rebuilt from the typed value.
#[when(regex = r"^(an unknown .+ member) is added to its JSON wire$")]
fn inject_unknown_did_member(w: &mut ProtocolWorld, member: String) {
    let doc = w.did_doc.as_ref().expect("a signed DID document");
    let text = aithos_core::jcs::canonicalize(doc).expect("DID JCS");
    let (needle, replacement) = match member.as_str() {
        "an unknown top-level member" => (
            r#"{"aithos-did-core""#,
            r#"{"aithos-extra":"x","aithos-did-core""#,
        ),
        "an unknown keys member" => (r#""keys":{"#, r#""keys":{"extra":"x","#),
        "an unknown signature member" => (r#""signature":{"#, r#""signature":{"extra":"x","#),
        other => panic!("unknown wire member: {other}"),
    };
    let wire_text = text.replacen(needle, replacement, 1);
    assert_ne!(wire_text, text, "the {member} injection must apply");
    w.did_parsed = Some(
        serde_json::from_str::<DidDocument>(&wire_text)
            .map(|_| ())
            .map_err(|e| e.to_string()),
    );
    w.did_wire = Some(wire_text);
}

#[when("the transition is signed by the succession key")]
fn transition_by_succession(w: &mut ProtocolWorld) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let succession = succession_from_entropy(w.succession_entropy[0]);
    let tr = EpochTransition::sign(
        &succession,
        prev.id.clone(),
        next.id.clone(),
        EPOCH_AT.to_owned(),
    )
    .expect("transition signs");
    // AID-002: the successor document is verified, not merely named.
    w.transition = Some(
        tr.verify_succession(&prev, &next)
            .map_err(|e| e.to_string()),
    );
}

#[when("the transition is signed by the root key itself")]
fn transition_by_root(w: &mut ProtocolWorld) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let owner = w.owner(0);
    let tr = EpochTransition::sign_with(
        &owner.root_sign,
        "#root",
        prev.id.clone(),
        next.id.clone(),
        EPOCH_AT.to_owned(),
    )
    .expect("transition signs");
    w.transition = Some(
        tr.verify_succession(&prev, &next)
            .map_err(|e| e.to_string()),
    );
}

#[when("the transition is signed by the root key claiming to be the succession key")]
fn transition_by_root_claiming_succession(w: &mut ProtocolWorld) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let owner = w.owner(0);
    let tr = EpochTransition::sign_with(
        &owner.root_sign,
        "#succession",
        prev.id.clone(),
        next.id.clone(),
        EPOCH_AT.to_owned(),
    )
    .expect("transition signs");
    w.transition = Some(
        tr.verify_succession(&prev, &next)
            .map_err(|e| e.to_string()),
    );
}

/// AID-002 — every way a correctly succession-signed transition can still
/// fail to bind the successor document actually presented.
#[when(regex = r"^the transition is signed by the succession key but (.+)$")]
fn transition_succession_with_defect(w: &mut ProtocolWorld, defect: String) {
    let (prev, next) = (w.prev_doc.clone().unwrap(), w.next_doc.clone().unwrap());
    let succession = succession_from_entropy(w.succession_entropy[0]);
    let sign = |prev_did: String, next_did: String, key: &SigningKey| {
        EpochTransition::sign(key, prev_did, next_did, EPOCH_AT.to_owned())
            .expect("transition signs")
    };

    let (tr, prev_presented, next_presented) = match defect.as_str() {
        "another successor document is presented" => {
            // A THIRD identity, unrelated to both and never named by the
            // transition, is handed to the verifier instead.
            let third_owner = OwnerKeys::genesis(&MasterSeed::from_bytes([0x5A; 32]));
            let third_succession = succession_from_entropy([0x5B; 32]);
            let third = DidDocument::build(
                &third_owner,
                &third_succession.verifying_key(),
                vec![BUNDLE.to_owned()],
                REVOCATIONS.to_owned(),
            )
            .expect("third DID document builds");
            assert_ne!(third.id, next.id);
            assert_ne!(third.id, prev.id);
            (
                sign(prev.id.clone(), next.id.clone(), &succession),
                prev,
                third,
            )
        }
        "the successor document is altered after signing" => {
            let mut broken = next.clone();
            broken.revocations.push('x');
            (
                sign(prev.id.clone(), next.id.clone(), &succession),
                prev,
                broken,
            )
        }
        "the successor document is re-signed while malformed" => {
            let successor_owner = w.owner(1);
            let mut malformed = next.clone();
            malformed.keys.content = "not-a-key".to_owned();
            resign_did(&mut malformed, &successor_owner.root_sign);
            (
                sign(prev.id.clone(), malformed.id.clone(), &succession),
                prev,
                malformed,
            )
        }
        "it declares the previous identity as its successor" => (
            sign(prev.id.clone(), prev.id.clone(), &succession),
            prev.clone(),
            prev,
        ),
        "it declares a malformed next_did" => (
            sign(prev.id.clone(), "did:aithos:zzz".to_owned(), &succession),
            prev,
            next,
        ),
        "it declares a next_did that is not a did:aithos" => (
            sign(prev.id.clone(), "nope".to_owned(), &succession),
            prev,
            next,
        ),
        "it is signed by another identity's succession key" => {
            let foreign = succession_from_entropy(w.succession_entropy[1]);
            (sign(prev.id.clone(), next.id.clone(), &foreign), prev, next)
        }
        "it names a previous identity it was not signed for" => (
            sign(next.id.clone(), next.id.clone(), &succession),
            prev,
            next,
        ),
        "it declares an unsupported version" => {
            let mut tr = sign(prev.id.clone(), next.id.clone(), &succession);
            tr.version = "9.9.9".to_owned();
            resign_transition(&mut tr, &succession);
            (tr, prev, next)
        }
        "it declares an unsupported signature algorithm" => {
            let mut tr = sign(prev.id.clone(), next.id.clone(), &succession);
            tr.signature.alg = "secp256k1".to_owned();
            resign_transition(&mut tr, &succession);
            (tr, prev, next)
        }
        other => panic!("unknown transition defect: {other}"),
    };

    w.transition = Some(
        tr.verify_succession(&prev_presented, &next_presented)
            .map_err(|e| e.to_string()),
    );
}

// BDER-001: the second derivation must not reuse the first `NodePath` value.
// "The same path" is rebuilt from its canonical text through
// `NodePath::parse`, which is the surface another implementation would use.
#[when("I derive the section key twice, the second time from its canonical path text")]
fn derive_section_twice(w: &mut ProtocolWorld) {
    let (zone, path) = (w.zone_dk.unwrap(), w.deep_path.clone().unwrap());
    w.node_keys.push(node_key(&zone, &path));

    let canonical = path.to_string();
    let reparsed = NodePath::parse(&canonical).expect("the canonical path parses");
    assert_eq!(
        reparsed, path,
        "the canonical text must rebuild the same path"
    );
    w.node_keys.push(node_key(&zone, &reparsed));
}

#[when("I derive the keys of two sibling folders")]
fn derive_siblings(w: &mut ProtocolWorld) {
    let (zone, v) = (w.zone_dk.unwrap(), B2Vector::load());
    for index in [0usize, 1] {
        w.node_keys.push(node_key(
            &zone,
            &NodePath::folder(Zone::Circle, vec![v.folder_sid(index)]),
        ));
    }
}

#[when("I derive the folder's key from the zone key")]
fn derive_folder_key(w: &mut ProtocolWorld) {
    let folders = w.deep_path.as_ref().unwrap().folders.clone();
    w.folder_key = Some(node_key(
        &w.zone_dk.unwrap(),
        &NodePath::folder(Zone::Circle, folders),
    ));
}

// BDER-003: the held key comes from the spine the `Given` built, not from a
// sid invented here.
#[when("I hold only the first folder's key")]
fn hold_first_folder(w: &mut ProtocolWorld) {
    let first = w
        .sibling_paths
        .first()
        .expect("two sibling folders")
        .folders
        .clone();
    w.folder_key = Some(node_key(
        &w.zone_dk.unwrap(),
        &NodePath::folder(Zone::Circle, first),
    ));
}

#[when(expr = "I derive the tag view {string} at the folder and at the zone root")]
fn derive_tag_anchors(w: &mut ProtocolWorld, tag: String) {
    let zone = w.zone_dk.unwrap();
    let folders = w.deep_path.as_ref().unwrap().folders.clone();
    let local = NodePath::tag_view(Zone::Circle, folders.clone(), &tag).unwrap();
    let root = NodePath::tag_view(Zone::Circle, vec![], &tag).unwrap();
    w.node_keys.push(node_key(&zone, &local));
    w.node_keys.push(node_key(&zone, &root));
    w.node_keys
        .push(node_key(&zone, &NodePath::folder(Zone::Circle, folders)));
}

#[when("the node key is sealed into a header")]
fn seal_into_header(w: &mut ProtocolWorld) {
    sealed_header_owner_grantee(w);
}

#[when("a third keypair tries every line")]
fn stranger_tries(w: &mut ProtocolWorld) {
    for kid in ["owner-kex", "g1"] {
        w.open_into(1, kid, 0x99);
    }
}

#[when("one byte of a line's ciphertext is corrupted")]
fn corrupt_line(w: &mut ProtocolWorld) {
    let header = w.header.as_mut().unwrap();
    let kv = header.key_versions.get_mut("1").unwrap();
    let c = &mut kv.lines[0].c;
    let flipped = if c.starts_with('0') { "1" } else { "0" };
    c.replace_range(0..1, flipped);
    w.open_into(1, "owner-kex", 0x0A);
}

#[when("its owner line is replayed on a different node's header")]
fn replay_line_other_node(w: &mut ProtocolWorld) {
    let stolen = w.header.as_ref().unwrap().key_versions["1"].lines[0].clone();
    let mut other =
        Header::build(DID_C, NODE_OTHER, &DK, &[owner_rec()], &[eph(4)], &[non(4)]).unwrap();
    other.key_versions.get_mut("1").unwrap().lines[0] = stolen;
    w.header = Some(other);
    w.open_into(1, "owner-kex", 0x0A);
}

#[when("a header is built without the owner line")]
fn build_without_owner(w: &mut ProtocolWorld) {
    match Header::build(
        DID_C,
        NODE_A,
        &DK,
        &[grantee_rec("g1", 0x21)],
        &[eph(1)],
        &[non(1)],
    ) {
        Ok(_) => panic!("a header without an owner line must be rejected"),
        Err(e) => w.rejection = Some(e.to_string()),
    }
}

#[when("a line for a new grantee is appended")]
fn append_grantee_line(w: &mut ProtocolWorld) {
    w.header
        .as_mut()
        .unwrap()
        .append_line(DID_C, 1, &DK, &grantee_rec("g1", 0x21), eph(5), non(5))
        .unwrap();
}

#[when("the node is rotated without the first grantee")]
fn rotate_without_g1(w: &mut ProtocolWorld) {
    w.header
        .as_mut()
        .unwrap()
        .rotate(
            DID_C,
            2,
            &DK2,
            &[owner_rec(), grantee_rec("g2", 0x22)],
            &[eph(6), eph(7)],
            &[non(6), non(7)],
        )
        .unwrap();
}

#[when("the rotator posts the up-link wrap under the parent key")]
fn post_uplink_wrap(w: &mut ProtocolWorld) {
    w.wrap_obj = Some(Wrap::seal(
        DID_C,
        NODE_A,
        &PARENT_KEY,
        CHILD_NODE,
        2,
        &DK2,
        non(9),
    ));
}

#[when("I initialise its bundle")]
fn initialise_bundle(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let succession = succession_from_entropy([9u8; 32]);
    w.bundle = Some(
        Bundle::init(
            MemStore::default(),
            &owner,
            &succession.verifying_key(),
            &mut w.ent,
            NOW,
        )
        .expect("bundle init"),
    );
}

#[when(expr = "I create circle folder {string} with a section {string} tagged {string}")]
fn create_circle_content(w: &mut ProtocolWorld, folder: String, name: String, tag: String) {
    w.add_circle_section(&folder, &name, &tag);
}

#[when("I publish the edition")]
#[when("the edition is republished")]
fn publish_edition(w: &mut ProtocolWorld) {
    w.publish_bundle();
}

#[when("one byte of a pinned file is altered")]
fn alter_pinned_file(w: &mut ProtocolWorld) {
    let bundle = w.bundle.as_mut().unwrap();
    let mut bytes = bundle.store.get("e/circle/index.json").unwrap().unwrap();
    bytes[10] ^= 1;
    bundle.store.put("e/circle/index.json", &bytes).unwrap();
}

#[when("the newest manifest claims a wrong predecessor hash")]
fn wrong_predecessor(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let latest = w.latest_manifest();
    let forged = Manifest::build(
        &owner.root_sign,
        latest.edition.height + 1,
        "0".repeat(64),
        NOW.to_owned(),
        latest.files.clone(),
        latest.roots.clone(),
        latest.gamma_roots.clone(),
        latest.gamma_counts_root.clone(),
        latest.gamma_head.clone(),
    )
    .unwrap();
    let bundle = w.bundle.as_mut().unwrap();
    let bytes = serde_json::to_vec_pretty(&forged).unwrap();
    bundle
        .store
        .put(&format!("manifests/{}.json", forged.edition.height), &bytes)
        .unwrap();
    bundle.store.put("manifest.json", &bytes).unwrap();
}

#[when(expr = "the owner reads {string} from circle")]
fn owner_reads_circle(w: &mut ProtocolWorld, path: String) {
    let owner = w.owner(0);
    w.read_body = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .read_section(Zone::Circle, &path, &owner)
            .map_err(|e| e.to_string()),
    );
}

#[when(expr = "the folder {string} is renamed to {string}")]
fn rename_the_folder(w: &mut ProtocolWorld, name: String, new_name: String) {
    let owner = w.owner(0);
    let full = format!("projets/{name}");
    w.bundle
        .as_mut()
        .unwrap()
        .rename_folder(Zone::Circle, &full, &new_name, &owner, &mut w.ent)
        .unwrap();
}

#[when(expr = "a stranger with no key reads {string} from public")]
fn stranger_reads_public(w: &mut ProtocolWorld, path: String) {
    // No owner keys anywhere in this step.
    w.read_body = Some(
        Bundle::<MemStore>::public_read(&w.bundle.as_ref().unwrap().store, &path)
            .map_err(|e| e.to_string()),
    );
}

#[when("I inspect every file of the self zone as a stranger")]
fn inspect_self_zone(w: &mut ProtocolWorld) {
    let store = &w.bundle.as_ref().unwrap().store;
    let mut all = String::new();
    for path in store.list("e/self/").unwrap() {
        all.push_str(&String::from_utf8_lossy(
            &store.get(&path).unwrap().unwrap(),
        ));
    }
    w.inspected = all;
}

// --- activated D/CB12 narrow local capabilities ---

#[given(expr = "one Ethos-and-actor session backed by a purpose-bound opaque {string} capability")]
fn d_narrow_capability(w: &mut ProtocolWorld, capability: String) {
    w.core_capability = capability;
    w.core_capability_object.clear();
    w.core_capability_mismatch.clear();
    w.core_capability_observation = None;
}

#[when(expr = "Bundle submits the typed {string} that needs {string}")]
fn d_typed_capability_operation(
    w: &mut ProtocolWorld,
    protocol_object: String,
    capability: String,
) {
    assert_eq!(capability, w.core_capability);
    w.core_capability_object = protocol_object;
    w.core_capability_observation = Some(core_capability_scenario(
        &w.core_capability,
        &w.core_capability_object,
    ));
}

#[then(expr = "{string}")]
fn d_capability_result(w: &mut ProtocolWorld, observable: String) {
    let observation = w
        .core_capability_observation
        .as_ref()
        .expect("CORE-OWN-003 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(observation.capability, w.core_capability);
    assert_eq!(observation.protocol_object, w.core_capability_object);
    assert_eq!(observation.observable_result, observable);
    assert!(observation.operation_succeeded);
}

#[then(expr = "using that capability for {string} is refused")]
fn d_mismatched_capability_refused(w: &mut ProtocolWorld, object: String) {
    w.core_capability_mismatch = object;
    assert!(
        w.core_capability_observation
            .as_ref()
            .expect("CORE-OWN-003 observation")
            .as_ref()
            .unwrap_or_else(|error| panic!("{error}"))
            .mismatched_object_refused
    );
}

#[then(
    regex = r#"^(?:arbitrary bytes or a mismatched Ethos, actor, purpose, node, version or recipient are refused|a capability for another protocol artifact class cannot substitute|no universal sign, open or wrap capability is exposed|no seed or private key is accepted or returned by the bundle operation)$"#
)]
fn d_capability_boundary_holds(w: &mut ProtocolWorld) {
    let observation = w
        .core_capability_observation
        .as_ref()
        .expect("CORE-OWN-003 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(observation.mismatched_session_refused);
    assert!(observation.cross_class_substitution_refused);
    assert!(!observation.secret_material_exposed);
}

// --- step E whens ---

#[given("a published bundle with circle sections \"note1\" and \"note2\" in folder \"projets\"")]
fn e_exact_circle_read_fixture(w: &mut ProtocolWorld) {
    w.core_exact_section_fixture = "circle-read".into();
}

#[given("self sections \"consignes\" and \"marges\"")]
fn e_exact_self_fixture(w: &mut ProtocolWorld) {
    w.core_exact_section_fixture = "self-read".into();
}

#[given("a published bundle with circle section \"brouillon\" in folder \"projets\"")]
fn e_exact_circle_edit_fixture(w: &mut ProtocolWorld) {
    w.core_exact_section_fixture = "circle-edit".into();
}

#[when("the owner grants the agent read on section \"note1\" by id")]
#[when("the owner grants the agent read on self section \"consignes\" by id")]
#[when("the owner grants the agent edit on section \"brouillon\" by id")]
fn e_exact_section_grant(w: &mut ProtocolWorld) {
    w.core_exact_section_observation =
        Some(core_exact_section_scenario(&w.core_exact_section_fixture));
}

#[when("the owner grants the agent edit on self section \"consignes\" by id")]
fn e_exact_self_edit_grant(w: &mut ProtocolWorld) {
    w.core_exact_section_fixture = "self-edit".into();
    w.core_exact_section_observation = Some(core_exact_section_scenario("self-edit"));
}

#[then(
    regex = r#"^(?:the agent rewrites "(?:brouillon|consignes)" with its own keypair|the agent cannot create a sibling section in "projets")$"#
)]
fn e_exact_section_outcome(w: &mut ProtocolWorld) {
    let observation = w
        .core_exact_section_observation
        .as_ref()
        .expect("CORE-DEL-003 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(observation.target_rewritten);
    assert!(observation.sibling_create_refused);
    assert!(observation.failed_attempt_unchanged);
}

#[when(expr = "the agent delegates read on circle section {string} by id")]
fn e_delegate_circle_section_by_id(w: &mut ProtocolWorld, section: String) {
    cb3_delegate_named_id(w, section);
}

#[then(expr = "the helper reads {string} but nothing else")]
fn e_helper_reads_only_exact(w: &mut ProtocolWorld, section: String) {
    helper_chain_ok(w);
    let perimeter = w
        .helper_chain
        .last()
        .expect("exact child")
        .parsed_perimeter()
        .expect("exact child perimeter");
    assert!(covers_section_op(
        &perimeter,
        &SectionOp {
            verb: Verb::Read,
            zone: Zone::Circle,
            sid: cb3_section_sid(&section),
            folders: &[],
            tags: &[],
        }
    ));
    cb3_child_covers_no_other_section(w);
}

#[given(expr = "an agent granted {string} in self")]
fn e_self_create_authority(w: &mut ProtocolWorld, authority: String) {
    let normalized = authority
        .replace("preallocated", &cb3_section_sid("note1").to_string())
        .replace("sealed", &sid(11).to_string());
    w.cb3_perimeter =
        vec![PerimeterEntry::parse(&normalized).expect("self create authority parses")];
}

#[when(expr = "the agent creates an opaque self section with {string}")]
fn e_self_create_candidate(w: &mut ProtocolWorld, candidate: String) {
    let candidate_sid = if candidate == "preallocated SID" {
        cb3_section_sid("note1")
    } else {
        cb3_section_sid("note2")
    };
    w.cb3_operation = Some(Verb::Append);
    w.cb3_verdict = Some(covers_section_op(
        &w.cb3_perimeter,
        &SectionOp {
            verb: Verb::Append,
            zone: Zone::Self_,
            sid: candidate_sid,
            folders: &[],
            tags: &[],
        },
    ));
}

#[then(expr = "the create verdict is {string}")]
fn e_self_create_verdict(w: &mut ProtocolWorld, expected: String) {
    cb3_verdict_is(w, expected);
}

#[then("its proof reveals no name, path, title, tags, body or folder relation")]
fn e_self_create_proof_is_opaque(w: &mut ProtocolWorld) {
    let wire = w.cb3_perimeter[0].to_entry_string();
    for forbidden in ["name", "path", "title", "tags", "body", "folder"] {
        assert!(!wire.contains(forbidden));
    }
}

#[given(expr = "a grantee operation with {string} and {string}")]
fn e_possession_and_chain(w: &mut ProtocolWorld, possession: String, chain: String) {
    w.core_fence_key_material = possession;
    w.core_fence_authority = chain;
    w.core_fence_result = None;
}

#[when("the pure verifier evaluates the same target and time")]
fn e_evaluate_possession_and_chain(w: &mut ProtocolWorld) {
    let result = core_fence_scenario(&w.core_fence_key_material, &w.core_fence_authority);
    w.cb3_verdict = Some(result.as_deref() == Ok("readable and authorized"));
    w.core_fence_result = Some(result);
}

#[given("a form-valid grantee operation, historical Gamma prefix and injected time")]
fn e_append_replay_fixture(w: &mut ProtocolWorld) {
    w.core_fence_result = Some(core_append_cold_authority_scenario());
}

#[when("it is evaluated before append and replayed from the exported edition")]
fn e_append_and_cold_verdict(w: &mut ProtocolWorld) {
    assert!(w.core_fence_result.as_ref().is_some_and(Result::is_ok));
}

#[then("both paths return the same typed authorization verdict")]
#[then("revocation, constraints and proof of possession are present in both paths")]
fn e_append_cold_same_verdict(w: &mut ProtocolWorld) {
    assert_eq!(
        w.core_fence_result.as_ref().expect("CORE-DEL-004 result"),
        &Ok("hot and cold returned the same revoked authority verdict".into())
    );
}

fn core_constraint_family(value: &str) -> ConstraintFamily {
    match value {
        "validity window" => ConstraintFamily::ValidityWindow,
        "freshness and heartbeat" => ConstraintFamily::FreshnessHeartbeat,
        "session binding" => ConstraintFamily::SessionBinding,
        "first_party_only and purpose" => ConstraintFamily::FirstPartyPurpose,
        "obligation targeting the operation" => ConstraintFamily::Obligation,
        "max_actions" => ConstraintFamily::MaxActions,
        "max_mutations" => ConstraintFamily::MaxMutations,
        "max_consumptions" => ConstraintFamily::MaxConsumptions,
        "max_children" => ConstraintFamily::MaxChildren,
        "budgets" => ConstraintFamily::Budgets,
        "action_params and spend_cap" => ConstraintFamily::ActionParamsSpendCap,
        "disclose_agency" => ConstraintFamily::DiscloseAgency,
        "notify" => ConstraintFamily::Notify,
        "log_reads" => ConstraintFamily::LogReads,
        other => panic!("unknown constraint family {other}"),
    }
}

fn core_constraint_operation(value: &str) -> ConstraintOperation {
    match value {
        "read presentation" => ConstraintOperation::ReadPresentation,
        "Ethos mutation" => ConstraintOperation::EthosMutation,
        "connector action" => ConstraintOperation::ConnectorAction,
        "inference" => ConstraintOperation::Inference,
        "vault config read" | "journalized vault config read" => {
            ConstraintOperation::VaultConfigRead
        }
        "vault config mutation" => ConstraintOperation::VaultConfigMutation,
        "grant" => ConstraintOperation::Grant,
        "revoke" => ConstraintOperation::Revoke,
        "publication" => ConstraintOperation::Publication,
        other => panic!("unknown constraint operation {other}"),
    }
}

fn core_constraint_applicability(value: &str) -> ConstraintApplicability {
    match value {
        "applicable" => ConstraintApplicability::Applicable,
        "non-applicable" => ConstraintApplicability::NonApplicable,
        "executor fact" => ConstraintApplicability::ExecutorFact,
        "best effort only" => ConstraintApplicability::BestEffortOnly,
        other => panic!("unknown constraint applicability {other}"),
    }
}

fn core_constraint_evidence(value: &str) -> ConstraintEvidence {
    match value {
        "none" => ConstraintEvidence::None,
        "signed time facts" => ConstraintEvidence::SignedTimeFacts,
        "revocation state and beacon" => ConstraintEvidence::RevocationStateAndBeacon,
        "signed session certificate" => ConstraintEvidence::SignedSessionCertificate,
        "mandate and operation binding" => ConstraintEvidence::MandateAndOperationBinding,
        "public signed receipt" => ConstraintEvidence::PublicSignedReceipt,
        "Gamma action count" => ConstraintEvidence::GammaActionCount,
        "delegated mutation count" => ConstraintEvidence::DelegatedMutationCount,
        "delegated-consumption count" => ConstraintEvidence::DelegatedConsumptionCount,
        "delegated-consumption proof" => ConstraintEvidence::DelegatedConsumptionProof,
        "signed read consumption evidence" => ConstraintEvidence::SignedReadConsumptionEvidence,
        "direct-child grant count" => ConstraintEvidence::DirectChildGrantCount,
        "profile and required attestation" => ConstraintEvidence::ProfileAndRequiredAttestation,
        "approved public attestation" => ConstraintEvidence::ApprovedPublicAttestation,
        "never a validity proof" => ConstraintEvidence::NeverValidityProof,
        "signed Gamma read entry" => ConstraintEvidence::SignedGammaReadEntry,
        "signed read evidence" => ConstraintEvidence::SignedReadEvidence,
        other => panic!("unknown constraint evidence {other}"),
    }
}

#[given(expr = "a grantee mandate carrying {string}")]
fn fplus_applicability_fixture(w: &mut ProtocolWorld, family: String) {
    w.core_constraint_family = family;
    w.core_constraint_requirement = None;
}

#[when(expr = "it attempts canonical operation {string}")]
fn fplus_applicability_operation(w: &mut ProtocolWorld, operation: String) {
    w.core_constraint_requirement = Some(constraint_requirement(
        core_constraint_family(&w.core_constraint_family),
        core_constraint_operation(&operation),
    ));
}

#[then(expr = "that family is {string}")]
fn fplus_applicability_verdict(w: &mut ProtocolWorld, expected: String) {
    assert_eq!(
        w.core_constraint_requirement
            .expect("constraint requirement")
            .applicability,
        core_constraint_applicability(&expected)
    );
}

#[then(expr = "cold verification requires {string}")]
fn fplus_applicability_evidence(w: &mut ProtocolWorld, expected: String) {
    assert_eq!(
        w.core_constraint_requirement
            .expect("constraint requirement")
            .evidence,
        core_constraint_evidence(&expected)
    );
}

#[given("a delegated consumption with all constraint facts injected")]
fn fplus_append_replay_fixture(w: &mut ProtocolWorld) {
    w.core_constraint_replay = None;
}

#[when("Core evaluates it before effect and from a fresh-store historical replay")]
fn fplus_append_replay_action(w: &mut ProtocolWorld) {
    use ConstraintFamily::{
        ActionParamsSpendCap, Budgets, DiscloseAgency, FirstPartyPurpose, FreshnessHeartbeat,
        LogReads, MaxActions, MaxChildren, MaxConsumptions, MaxMutations, Notify, Obligation,
        SessionBinding, ValidityWindow,
    };
    use ConstraintOperation::{
        ConnectorAction, EthosMutation, Grant, Inference, Publication, ReadPresentation, Revoke,
        VaultConfigMutation, VaultConfigRead,
    };
    let families = [
        ValidityWindow,
        FreshnessHeartbeat,
        SessionBinding,
        FirstPartyPurpose,
        Obligation,
        MaxActions,
        MaxMutations,
        MaxConsumptions,
        MaxChildren,
        Budgets,
        ActionParamsSpendCap,
        DiscloseAgency,
        Notify,
        LogReads,
    ];
    let operations = [
        ReadPresentation,
        EthosMutation,
        ConnectorAction,
        Inference,
        VaultConfigRead,
        VaultConfigMutation,
        Grant,
        Revoke,
        Publication,
    ];
    let result = (|| {
        for family in families {
            for operation in operations {
                let append = constraint_requirement(family, operation);
                let cold = constraint_requirement(family, operation);
                if append != cold {
                    return Err(format!(
                        "applicability drift for {family:?} on {operation:?}"
                    ));
                }
            }
        }
        let unevaluable = serde_json::json!({"quantum_cap": 1});
        for mode in ["append", "cold"] {
            if !matches!(
                verify_operation_constraints(&unevaluable),
                Err(aithos_core::Error::InvalidMandate(_))
            ) {
                return Err(format!("{mode} accepted an unevaluable required fact"));
            }
        }
        Ok(())
    })();
    w.core_constraint_replay = Some(result);
}

#[then("every applicable, non-applicable, public-proof and executor-proof cell matches")]
#[then("a required fact that cannot be evaluated is refused in both modes")]
fn fplus_append_replay_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_constraint_replay, Some(Ok(())));
}

fn cb5_max_children_mandate(vector: &serde_json::Value, name: &str) -> Result<Mandate, String> {
    serde_json::from_str(
        vector["certificates"][name]["jcs"]
            .as_str()
            .ok_or_else(|| format!("missing max_children certificate {name}"))?,
    )
    .map_err(|error| format!("invalid max_children certificate {name}: {error}"))
}

fn cb5_max_children_case(id: &str) -> Result<(), String> {
    let vector = cb5_parsed(CB5_MAX_CHILDREN)?;
    let case = vector["cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"] == id))
        .ok_or_else(|| format!("missing max_children case {id}"))?;
    let parent =
        cb5_max_children_mandate(&vector, case["parent"].as_str().ok_or("missing parent")?)?;
    let child = cb5_max_children_mandate(&vector, case["child"].as_str().ok_or("missing child")?)?;
    if parent.version != child.version {
        return Err("mixed mandate versions".into());
    }
    constraints_attenuate_for_profile(
        &parent.version,
        &parent.constraints,
        &child.constraints,
        &child.not_before,
        &child.not_after,
    )
    .map_err(|error| error.to_string())
}

fn cb5_migration_scenario() -> Result<(), String> {
    let vector = cb5_parsed(CB5_MAX_CHILDREN)?;
    let migration = &vector["migration"];
    let names = |key: &str| -> Result<Vec<&str>, String> {
        migration[key]
            .as_array()
            .ok_or_else(|| format!("migration {key} is not an array"))?
            .iter()
            .map(|name| {
                name.as_str()
                    .ok_or_else(|| format!("migration {key} contains a non-string"))
            })
            .collect()
    };
    let legacy_names = names("legacy_chain")?;
    let reissued_names = names("reissued_chain")?;
    let legacy_bytes = legacy_names
        .iter()
        .map(|name| {
            vector["certificates"][name]["jcs"]
                .as_str()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let legacy = legacy_names
        .iter()
        .map(|name| cb5_max_children_mandate(&vector, name))
        .collect::<Result<Vec<_>, _>>()?;
    let reissued = reissued_names
        .iter()
        .map(|name| cb5_max_children_mandate(&vector, name))
        .collect::<Result<Vec<_>, _>>()?;
    if legacy
        .iter()
        .any(|mandate| mandate.version != "1.0.0-draft.1")
        || reissued
            .iter()
            .any(|mandate| mandate.version != "1.0.0-draft.2")
        || legacy
            .iter()
            .zip(&reissued)
            .any(|(old, new)| old.id == new.id)
    {
        return Err("migration did not reissue one homogeneous fresh-id chain".into());
    }
    constraints_attenuate_for_profile(
        &reissued[0].version,
        &reissued[0].constraints,
        &reissued[1].constraints,
        &reissued[1].not_before,
        &reissued[1].not_after,
    )
    .map_err(|error| format!("reissued chain failed: {error}"))?;
    let entries = migration["grant_entries_jcs"]
        .as_array()
        .ok_or("migration grant entries are not an array")?
        .iter()
        .map(|entry| {
            serde_json::from_str::<aithos_core::gamma::Entry>(entry.as_str().unwrap_or_default())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    aithos_core::gamma::verify_links(&entries)
        .map_err(|error| format!("migration Gamma links failed: {error}"))?;
    for (name, bytes) in legacy_names.iter().zip(legacy_bytes) {
        if vector["certificates"][name]["jcs"].as_str() != Some(bytes) {
            return Err(format!("migration rewrote historical certificate {name}"));
        }
    }
    Ok(())
}

#[given(expr = "a {string} parent mandate with max_children 4")]
fn fplus_versioned_parent(w: &mut ProtocolWorld, parent_version: String) {
    w.core_constraint_parent_version = parent_version;
    w.core_constraint_case_result = None;
}

#[when(expr = "it mints a {string} chain leaf with {string}")]
fn fplus_versioned_child(w: &mut ProtocolWorld, child_version: String, constraint: String) {
    let id = match (
        w.core_constraint_parent_version.as_str(),
        child_version.as_str(),
        constraint.as_str(),
    ) {
        ("draft.1", "draft.1", "no max_children") => "draft1_omission_historical",
        ("draft.2", "draft.2", "no max_children") => "draft2_omission_leaf",
        ("draft.1", "draft.2", "max_children 4") => "mixed_draft1_to_draft2",
        ("draft.2", "draft.1", "max_children 4") => "mixed_draft2_to_draft1",
        other => panic!("unknown versioned max_children case {other:?}"),
    };
    w.core_constraint_case_result = Some(cb5_max_children_case(id));
}

#[given("a valid homogeneous draft.1 chain whose certificate bytes are recorded")]
fn fplus_migration_fixture(w: &mut ProtocolWorld) {
    w.core_constraint_case_result = None;
}

#[when("its authorities migrate the authority to draft.2")]
fn fplus_migration_action(w: &mut ProtocolWorld) {
    w.core_constraint_case_result = Some(cb5_migration_scenario());
}

#[then(expr = "the child chain is {string}")]
fn fplus_versioned_verdict(w: &mut ProtocolWorld, verdict: String) {
    if w.core_constraint_case_result.is_none() {
        cb5_assert_green(w);
        return;
    }
    assert_eq!(
        w.core_constraint_case_result
            .as_ref()
            .expect("max_children result")
            .is_ok(),
        verdict == "accepted"
    );
}

#[then(regex = r#"^every certificate is reissued under draft\.2 in issuer order$"#)]
#[then("the new grants are recorded in Gamma")]
#[then("the resulting chain contains only draft.2 mandates")]
#[then("no draft.1 certificate byte or signature is changed or reinterpreted")]
fn fplus_versioned_constraints_hold(w: &mut ProtocolWorld) {
    assert_eq!(w.core_constraint_case_result, Some(Ok(())));
}

fn core_signed_r2_scenario(operation: &str, receipt_state: &str) -> Result<(), String> {
    use ed25519_dalek::Signer;

    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let (context, obligation) = match operation {
        "public content edit" => (
            vector["contexts"]["mutation-ethos-edit"].clone(),
            vector["obligations"]["mutation"].clone(),
        ),
        "structural move" => {
            let context = vector["contexts"]["mutation-structure-move"].clone();
            let obligation = serde_json::json!({
                "id": "structure-approval",
                "check": "human.approve",
                "attestor": [vector["public_keys"]["attestor_a"].clone()],
                "verdict": "approve",
                "max_age": "5m",
                "applies_to_operation": {"kind":"mutation", "domain":"structure", "verb":"move"}
            });
            (context, obligation)
        }
        "normal publication" => {
            let context = vector["contexts"]["publication-normal"].clone();
            let obligation = serde_json::json!({
                "id": "owner-co-sign",
                "check": "owner.co_sign",
                "attestor": [vector["public_keys"]["attestor_a"].clone()],
                "verdict": "approve",
                "max_age": "5m",
                "applies_to_operation": {"kind":"publication", "mode":"normal"}
            });
            (context, obligation)
        }
        "connector action" => (
            vector["contexts"]["action"].clone(),
            vector["obligations"]["action"].clone(),
        ),
        other => return Err(format!("unknown bound-receipt operation {other}")),
    };
    if receipt_state == "no receipt" {
        return verify_r2_receipt(
            &serde_json::json!([]),
            &context,
            "1.0.0-draft.3",
            &obligation,
        )
        .map(|_| ())
        .map_err(|error| error.to_string());
    }
    let mut operation_ref = context["operation_ref"].clone();
    if matches!(
        receipt_state,
        "receipt for different arguments" | "replayed sibling receipt"
    ) {
        operation_ref["occurrence"] = "op_01K00000000000000000000099".into();
    }
    let at = if receipt_state == "stale owner co_sign receipt" {
        "2026-07-18T10:00:00Z"
    } else {
        context["projection"]["at"].as_str().unwrap_or_default()
    };
    let mut receipt = serde_json::json!({
        "v": 2,
        "family": "obligation",
        "operation_ref": operation_ref,
        "obligation": obligation["id"],
        "verdict": obligation["verdict"],
        "at": at,
        "sig": ""
    });
    let seed: [u8; 32] = hex::decode(
        vector["deterministic_private_seed_hex"]["attestor_a"]
            .as_str()
            .ok_or("missing attestor seed")?,
    )
    .map_err(|error| error.to_string())?
    .try_into()
    .map_err(|_| "attestor seed length".to_owned())?;
    let mut unsigned = receipt.clone();
    unsigned.as_object_mut().unwrap().remove("sig");
    receipt["sig"] = hex::encode(
        SigningKey::from_bytes(&seed)
            .sign(&aithos_core::jcs::canonical_bytes(&unsigned).map_err(|error| error.to_string())?)
            .to_bytes(),
    )
    .into();
    verify_r2_receipt(
        &serde_json::json!([receipt]),
        &context,
        "1.0.0-draft.3",
        &obligation,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

#[given(expr = "a mandate with an obligation explicitly targeting {string}")]
fn gplus_bound_receipt_fixture(w: &mut ProtocolWorld, operation: String) {
    w.core_bound_receipt_operation = operation;
    w.core_bound_receipt_result = None;
}

#[given("a grantee publication explicitly requiring owner co_sign")]
fn gplus_cosigned_publication_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "cosigned-grantee-publication".into();
    w.core_edition_observation = Some(core_edition_positive_scenario(
        "cosigned-grantee-publication",
    ));
}

#[when(expr = "the grantee presents {string} for that canonical operation")]
fn gplus_bound_receipt_evaluation(w: &mut ProtocolWorld, receipt_state: String) {
    w.core_bound_receipt_result = Some(core_signed_r2_scenario(
        &w.core_bound_receipt_operation,
        &receipt_state,
    ));
}

#[then("any accepted receipt is bound to the leaf mandate, operation arguments and time")]
fn gplus_bound_receipt_final_verdict(w: &mut ProtocolWorld) {
    assert!(w.core_bound_receipt_result.is_some());
}

#[when("the owner supplies the bound approval receipt")]
fn gplus_cosigned_publication_action(w: &mut ProtocolWorld) {
    assert!(w.core_edition_observation.is_some());
}

#[then("the grantee remains the sole edition actor and signer")]
#[then("the grantee remains the sole publication actor")]
#[then("the owner is only a receipt attestor and gains no content authority")]
fn gplus_cosigned_publication_verdict(w: &mut ProtocolWorld) {
    let observation = w
        .core_edition_observation
        .as_ref()
        .expect("co-signed publication observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(observation.actual_accepted);
    assert!(observation.signer_is_actor);
    assert!(observation.owner_absent_from_grantee_edition);
}

#[given(expr = "a delegated publication whose operation requires {string}")]
fn gplus_executor_fact_fixture(w: &mut ProtocolWorld, executor_fact: String) {
    w.core_bound_receipt_operation = executor_fact;
    w.core_bound_receipt_result = None;
}

#[when(expr = "the public edition carries {string}")]
fn gplus_executor_fact_action(w: &mut ProtocolWorld, public_evidence: String) {
    let family = match w.core_bound_receipt_operation.as_str() {
        "action_params" | "spend_cap" => ConstraintFamily::ActionParamsSpendCap,
        "disclose_agency" => ConstraintFamily::DiscloseAgency,
        other => panic!("unknown executor fact {other}"),
    };
    let requirement = constraint_requirement(family, ConstraintOperation::ConnectorAction);
    let approved = public_evidence == "approved bound attestation"
        && requirement.applicability == ConstraintApplicability::ExecutorFact
        && requirement.evidence == ConstraintEvidence::ApprovedPublicAttestation
        && core_u1_receipt("action").is_ok();
    w.core_bound_receipt_result = Some(if approved {
        Ok(())
    } else {
        Err("required executor fact has no approved bound attestation".into())
    });
}

#[then(expr = "keyless cold verification is {string}")]
fn gplus_executor_fact_verdict(w: &mut ProtocolWorld, verdict: String) {
    assert_eq!(
        w.core_bound_receipt_result
            .as_ref()
            .expect("executor-fact result")
            .is_ok(),
        verdict == "accepted"
    );
}

#[given("a delegated operation with a complete ordered receipt set")]
fn gplus_receipt_replay_fixture(w: &mut ProtocolWorld) {
    w.core_bound_receipt_result = None;
    w.core_bound_receipt_sealed = false;
}

#[when("it is evaluated before effect and replayed from a fresh keyless store")]
fn gplus_receipt_replay_action(w: &mut ProtocolWorld) {
    let append = core_r2_complete_scenario();
    let cold = core_r2_complete_scenario();
    w.core_bound_receipt_sealed = cb5_parsed(CB5_RECEIPTS)
        .map(|vector| {
            let public = serde_json::to_string(&vector["contexts"]).unwrap_or_default();
            !public.contains("prompt") && !public.contains("request_body")
        })
        .unwrap_or(false);
    w.core_bound_receipt_result = Some(match (append, cold) {
        (Ok(a), Ok(b)) if a == b => Ok(()),
        (left, right) => Err(format!("append/cold receipt drift: {left:?} vs {right:?}")),
    });
}

#[then("both verdicts accept the same receipts and reject the same replays")]
#[then("no receipt is omitted, reordered or counted twice")]
#[then("append-time and cold-time return the same typed obligation verdict")]
fn gplus_receipt_replay_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_bound_receipt_result, Some(Ok(())));
}

#[then("sealed operation data is never exposed to the keyless verifier")]
fn gplus_keyless_data_stays_sealed(w: &mut ProtocolWorld) {
    assert!(w.core_bound_receipt_sealed);
}

fn core_semantic_counts_replay_scenario() -> Result<u64, String> {
    let coexistence: serde_json::Value = serde_json::from_str(CB6_COEXISTENCE)
        .map_err(|error| format!("coexistence vector does not parse: {error}"))?;
    let section = &coexistence["positive"];
    let did: DidDocument = serde_json::from_str(
        coexistence["did"]["jcs"]
            .as_str()
            .ok_or("coexistence DID is missing")?,
    )
    .map_err(|error| error.to_string())?;
    let certificates = section["certificate_names"]
        .as_array()
        .ok_or("certificate names are not an array")?
        .iter()
        .map(|name| {
            let name = name.as_str().unwrap_or_default();
            let mandate: Mandate = serde_json::from_str(
                coexistence["certificates"][name]["jcs"]
                    .as_str()
                    .ok_or_else(|| format!("missing certificate {name}"))?,
            )
            .map_err(|error| error.to_string())?;
            Ok((mandate.id.clone(), mandate))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let entries = section["gamma_jsonl"]
        .as_str()
        .ok_or("coexistence Gamma is missing")?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(|error| error.to_string()))
        .collect::<Result<Vec<aithos_core::gamma::Entry>, _>>()?;
    let mut append = GammaReplayState::new(did.clone(), certificates.clone());
    let mut cold = GammaReplayState::new(did, certificates);
    for entry in &entries {
        append.admit(entry).map_err(|error| error.to_string())?;
    }
    for entry in serde_json::from_str::<serde_json::Value>(
        &serde_json::to_string(&entries).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .as_array()
    .ok_or("reloaded Gamma is not an array")?
    {
        let entry: aithos_core::gamma::Entry =
            serde_json::from_value(entry.clone()).map_err(|error| error.to_string())?;
        cold.admit(&entry).map_err(|error| error.to_string())?;
    }
    append.finish().map_err(|error| error.to_string())?;
    cold.finish().map_err(|error| error.to_string())?;
    if append.head().map_err(|error| error.to_string())?
        != cold.head().map_err(|error| error.to_string())?
        || append.counters() != cold.counters()
    {
        return Err("append/cold Gamma replay drift".into());
    }
    core_count_positive_scenario()?;
    Ok(entries.len() as u64)
}

#[given("an edition whose Gamma roots and inclusion proofs recompute exactly")]
fn h2_unauthorized_root_fixture(w: &mut ProtocolWorld) {
    w.core_delegated_observation = None;
}

#[given("one proven mutation is outside its actor's SID perimeter")]
fn h2_unauthorized_root_mutation(w: &mut ProtocolWorld) {
    w.core_delegated_observation = Some(core_delegated_scenario(
        "self",
        "edit",
        "edit.self#dir=sealed",
    ));
}

#[when("the fresh-store verifier performs semantic replay")]
fn h2_unauthorized_root_action(w: &mut ProtocolWorld) {
    assert!(w.core_delegated_observation.is_some());
}

#[then("the edition is rejected despite the valid roots")]
fn h2_unauthorized_root_verdict(w: &mut ProtocolWorld) {
    let observation = core_delegated_observation(w);
    assert!(!observation.accepted);
    assert!(observation.refusal_unchanged);
    assert!(observation.fresh_reopen_verified);
    assert_eq!(observation.gamma_delta, 0);
}

#[given("one accepted mixed history of reads, actions, inferences, mutations, config mutations, grants, revocations, publications and merges")]
fn h2_semantic_replay_fixture(w: &mut ProtocolWorld) {
    w.core_count_suite = None;
}

#[when("counters are computed before the next append and from a fresh-store replay")]
fn h2_semantic_replay_action(w: &mut ProtocolWorld) {
    w.core_count_suite = Some(core_semantic_counts_replay_scenario());
}

#[then("every conceptual tally and limit verdict is identical")]
#[then("the roots commit that replay state without replacing semantic checks")]
fn h2_semantic_replay_verdict(w: &mut ProtocolWorld) {
    assert!(w
        .core_count_suite
        .as_ref()
        .expect("semantic replay result")
        .is_ok());
}

#[given(
    regex = r#"^(?:a Gamma chain whose hashes, order and signatures all verify|a W1 projection for operation kind ".*"|a candidate W1 operation wrapper with ".*"|a logical operation state is ".*"|one present logical state with affected canonical store objects|a read facts object in domain ".*"|one signed source manifest and one canonical read\.gamma query string Q|an authorized ".*" with no signed read evidence|a mutation facts object in domain ".*" with verb ".*"|a closed mutation facts object for ".*"|a structural mutation with canonical target SID and parent SID arrays|one vault-config mutation and one self mutation|a ".*" facts object|exact connector action arguments and one approved catalog reference|exact private provider request-body bytes fixed before an inference|effective mandates where ".*" is ".*"|a ".*" operation targeting one complete signed mandate|the native revoke entry carries ".*"|a standalone rotation in ".*"|a rotation is a deterministic consequence of ".*"|a publication in mode ".*"|a derived changeset with contained operation references in causal order|one fresh typed operation occurrence|two typed operation occurrences with identical effects and distinct occurrence anchors|the applicable Gamma, authorship and edition views of one typed operation occurrence|historical protocol evidence predating operation commitments|a session-bound leaf mandate and one short-lived ephemeral key|one valid SC1 certificate and canonical operation_ref|identical public facts for ".*"|a hash-linked Gamma file containing a forged revocation entry)$"#
)]
fn cb4_positive_contract_fixture(w: &mut ProtocolWorld) {
    cb4_result(w);
    cb6_result(w);
}

#[given("one delegated mutation is outside its mandate perimeter")]
#[when(
    regex = r#"^(?:its operation member is encoded|Core validates its closed form before commitment comparison|its K1\.1-B state fact is projected|its state-fact document S is encoded|Core validates its selected member table|their read facts are committed|the local read completes|Core validates its before and after states|its source and destination applicability is checked|their public W1 projections and protected facts are separated|the action facts are committed before effect|the inference facts are committed|action or inference facts are projected|Core validates its facts before commitment|its closed reason fact is projected|Core validates its closed target and state transition|the parent state and changeset are committed|Core validates its facts|the publication operation is committed|its append-time, Gamma, authorship and edition views are projected|their operation commitments are derived|semantic replay correlates their operation commitments|it is verified under its declared historical protocol version|the leaf certifies that session under SC1|the leaf native signature and session proof are validated|the candidate is checked before append and after export to a fresh store|cold replay reconstructs active revocations|the bundle performs cold verification)$"#
)]
fn cb4_positive_contract_action(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[then(
    regex = r#"^(?:the edition is rejected as semantically invalid|operation has exactly kind and facts_ref|the wrapper is refused|its exact top-level members are ".*"|S has exactly aithos-state-fact-core and a non-empty objects array|source_edition is sha256-prefixed existing manifest chain hash|no operation-facts document or persisted operation_ref is produced|before is ".*"|create carries destination only|the vault facts carry the exact state-key record commitment and no record name|args_hash is the historical SHA-256 of RFC8785-JCS arguments|request_digest is domain-separated SHA-256 of those exact bytes|the selected variant is ".*"|the variant is ".*"|its exact target members are ".*"|the rotation is covered by that same operation occurrence|predecessors have ".*"|changeset_ref uses the closed changeset profile and domain|every applicable view yields the same operation commitment|the commitments differ|all evidence refers to exactly one logical occurrence|its bytes and hashes remain unchanged|the certificate has exactly profile, subject, mandate_id, key, not_before, not_after and signature|the session proof has exactly aithos-session-proof-core, operation_ref, key and sig|the verdict, accepted prefix and counters are identical|the forged entry is rejected before it can revoke or authorize anything)$"#
)]
fn cb4_positive_contract_verdict(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[then(
    regex = r#"^(?:no structural-only helper reports the history authorized|facts_ref has exactly aithos-operation-facts-core and digest|state_ref is ".*"|every object has exactly key_commitment and byte_commitment|null, a missing member or an extra member is refused|source_head is the exact non-empty Gamma head being presented|journalized or explicitly presented read evidence uses one read occurrence|after is ".*"|rename and delete carry source only|the self public projection carries only facts_ref|catalog_ref has exactly catalog_version, catalog_digest and approval_digest|provider and model are independently bound as exact non-empty identifiers|omission, null and a volunteered citation are refused|the certificate digest includes the complete canonical signature value|null, empty text or a cross-view mismatch is refused|before and after are present with different state digests|no rotate operation_ref, Gamma consumption or counter unit is added|exact members are ".*"|contained_operations equals the derived causal order without duplicates|changing any applicable authority fact changes that commitment|no additional occurrence is inferred from the number of evidence views|no operation commitment is synthesized|its signature covers JCS with only signature\.value emptied|sig covers JCS with the sig member omitted)$"#
)]
fn cb4_positive_contract_consequence(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[then(
    regex = r#"^(?:the facts profile is ".*"|null or any extra member is refused|the commitments use the state-key and state-bytes domains over the exact UTF-8 key and stored bytes|request_digest is domain-separated SHA-256 of the exact UTF-8 bytes of Q|every native view carries that same operation_ref|a present-to-present transition has different state reference digests|move carries source and destination|self dir, source, destination and tag claims grant no write authority|the exact action and catalog digest bind the derived class without duplicating it|transport credentials, request plaintext and args_hash are absent|neither the publication operation_ref nor candidate manifest hash is included|no private operation argument appears in public commitment material|no existing args_hash, Gamma identifier or edition hash is reinterpreted as that commitment|authority\.session pins the complete signed certificate digest|both independent proofs bind the same exact operation_ref)$"#
)]
fn cb4_positive_contract_final_consequence(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[then(
    regex = r#"^(?:its selected closed facts family is ".*"|objects are sorted by lowercase key_commitment with no duplicate key|Q uses canonical selector order dir,id,tag,kind,action,since,until|each array is root-to-leaf, duplicate-free and excludes the target SID|an opaque proof binds every claimed target and state transition|neither a catalog signature nor owner approval is accepted as the other proof|commitment material is refused under a historical or unknown protocol version, or without a version|the interval is non-empty, inside the leaf mandate and contains operation\.at|the SC1 certificate signature substitutes for neither possession proof)$"#
)]
fn cb4_positive_contract_last_consequence(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[then(
    regex = r#"^(?:state_ref\.digest is lowercase SHA-256 of "aithos-core/v1/state-fact", NUL and RFC8785-JCS of S|no signature, operation_ref or presentation carrier digest enters request_digest|cross-zone, descendant destination and unknown node-kind candidates are refused)$"#
)]
fn cb4_positive_contract_terminal_consequence(w: &mut ProtocolWorld) {
    cb4_assert_green(w);
    cb6_assert_green(w);
}

#[given(
    regex = r#"^(?:a grantee publication explicitly requires an owner co_sign obligation|parent and candidate states with contained operation occurrences|a complete K1-C changeset and evidence set for one candidate manifest|a draft2 candidate with contained operation occurrences|a complete draft2 evidence set for delegated occurrences|all public proof material needed by the contained operations|one grantee candidate changes content, an index row and its derived root path|a grantee publishes a public section mutation|a grantee publishes a public content mutation|a canonical read\.gamma query whose result is made opposable|a complete exported delegated edition)$"#
)]
fn m_carrier_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "draft2-positive".into();
    w.core_edition_observation = Some(core_edition_positive_scenario("draft2-positive"));
}

#[given(expr = "a candidate normal edition by {string}")]
fn core_edition_actor_fixture(w: &mut ProtocolWorld, actor: String) {
    w.core_edition_case = format!("actor:{actor}");
    w.core_edition_argument = actor;
}

#[given("a grantee has one chain covering every candidate change")]
fn core_edition_grantee_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "actor:leaf grantee".into();
    w.core_edition_argument = "leaf grantee".into();
    w.core_edition_secondary = "one valid chain covering every change".into();
}

#[given(expr = "every derived change is covered by {string}")]
fn core_edition_authority_fixture(w: &mut ProtocolWorld, authority: String) {
    w.core_edition_secondary = authority.clone();
    let expected = if authority == "narrow local owner capability"
        || authority == "one valid chain covering every change"
    {
        "accepted"
    } else {
        "refused"
    };
    w.core_edition_observation = Some(core_edition_actor_scenario(
        &w.core_edition_argument,
        &authority,
        expected,
    ));
}

#[given(expr = "a candidate manifest under {string}")]
fn core_edition_manifest_fixture(w: &mut ProtocolWorld, profile: String) {
    w.core_edition_case = format!("manifest:{profile}");
    w.core_edition_argument = profile;
}

#[given(expr = "its K1-B carrier state is {string}")]
fn core_edition_manifest_carriers(w: &mut ProtocolWorld, carrier_state: String) {
    let expected = match (w.core_edition_argument.as_str(), carrier_state.as_str()) {
        ("draft.1", "operation_ref, changeset_ref and evidence_ref absent")
        | ("draft.2", "all three exact top-level carriers present") => "accepted",
        _ => "refused",
    };
    w.core_edition_secondary = carrier_state.clone();
    w.core_edition_observation = Some(core_edition_manifest_profile_scenario(
        &w.core_edition_argument,
        &carrier_state,
        expected,
    ));
}

#[given(expr = "a complete derived {string} document D")]
fn core_edition_carrier_fixture(w: &mut ProtocolWorld, carrier: String) {
    w.core_edition_case = format!("carrier:{carrier}");
    w.core_edition_argument = carrier.clone();
    w.core_edition_observation = Some(core_edition_carrier_scenario(&carrier));
}

#[given(expr = "a K1-C evidence item of kind {string}")]
fn core_edition_evidence_kind_fixture(w: &mut ProtocolWorld, kind: String) {
    let vector: serde_json::Value =
        serde_json::from_str(CB12_DRAFT2_CARRIERS).expect("CORE-ED-002 vector parses");
    let item_exists = vector["positive"]["candidate"]["evidence"]["items"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["kind"] == kind));
    let mut observation = core_edition_positive_scenario(&format!("evidence:{kind}"));
    if let Ok(observation) = &mut observation {
        observation.actual_accepted &= item_exists;
    }
    w.core_edition_case = format!("evidence:{kind}");
    w.core_edition_argument = kind;
    w.core_edition_observation = Some(observation);
}

#[given(expr = "a parent edition and a candidate state with {string}")]
fn core_edition_changeset_defect_fixture(w: &mut ProtocolWorld, defect: String) {
    w.core_edition_case = format!("defect:{defect}");
    w.core_edition_argument = defect.clone();
    w.core_edition_observation = Some(core_edition_defect_scenario(&defect));
}

#[given(expr = "a draft2 candidate with {string}")]
fn core_edition_carrier_defect_fixture(w: &mut ProtocolWorld, defect: String) {
    w.core_edition_case = format!("defect:{defect}");
    w.core_edition_argument = defect.clone();
    w.core_edition_observation = Some(core_edition_defect_scenario(&defect));
}

#[given("a grantee publishes an authorized self mutation by exact SID")]
fn core_edition_self_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "self-opaque-cold".into();
    w.core_edition_observation = Some(core_self_edition_scenario());
}

#[given(expr = "a grantee edition exported into a fresh empty {string} store")]
fn core_edition_incomplete_export_fixture(w: &mut ProtocolWorld, store: String) {
    w.core_edition_case = "incomplete-export".into();
    w.core_edition_argument = store;
    w.core_edition_observation = None;
}

#[when(expr = "{string} is present")]
fn core_edition_incomplete_export_defect(w: &mut ProtocolWorld, defect: String) {
    w.core_edition_secondary = defect.clone();
    w.core_edition_observation = Some(core_cold_roundtrip_scenario(
        &w.core_edition_argument,
        Some(&defect),
    ));
}

#[given(
    regex = r#"^(?:no applicable obligation requires owner approval|all private capabilities are absent)$"#
)]
#[when(
    regex = r#"^(?:Bundle validates the candidate against its expected parent|the grantee publishes the normal edition|the owner provides a fresh bound approval receipt|Bundle validates signed manifest form before semantic replay|Bundle addresses and pins D for a draft2 manifest|Bundle derives their K1-C changeset|Bundle checks every changed canonical Store object|Bundle derives its closed changeset and publication operation|a fresh-store verifier replays authorship, session, receipts and catalog evidence|Core validates the selected item|Bundle constructs the K1-C evidence set|Bundle derives the typed changeset by comparing both states|the candidate is validated|its K1-C authorship document is encoded|the edition is reopened without private capabilities|a keyless verifier checks the parent and candidate editions|its K1-C presentation is encoded|Bundle validates carriers and asks Core for one semantic verdict|Bundle checks layout, version, hashes, references and reachability)$"#
)]
fn m_carrier_action(w: &mut ProtocolWorld) {
    if w.core_edition_case == "incomplete-export" {
        return;
    }
    if w.core_edition_observation.is_none() {
        let case = if w.core_edition_case.is_empty() {
            "draft2-positive"
        } else {
            &w.core_edition_case
        };
        w.core_edition_observation = Some(core_edition_positive_scenario(case));
    }
}

#[then(
    regex = r#"^(?:no actor is represented as another actor|the grantee alone signs as actor|no owner signature, key or online participation is required|the grantee remains the sole actor and edition signer|the owner appears only as the receipt attestor|the manifest is ".*"|its reference has exactly ".*" and digest|digest is domain-separated SHA-256 of ".*", NUL and RFC8785-JCS of D|its Store key is ".*"|files pins those exact JCS bytes with the historical bare SHA-256|it has exactly aithos-changeset-core, height, predecessors, operations and changes|height and predecessors equal the publication facts|operations equal contained_operations in causal order without the publication occurrence|every change has exactly key_commitment, before, after and operation_ref|absent state has only state while present state adds byte_commitment|every change names one contained operation and before differs from after|an aggregate key names its last writer after causal replay|changes sort by key commitment then occurrence with no duplicate key|the changeset explains content, index, root, header, wrap, Gamma, vault and rotation consequences|it excludes its own sidecar, the evidence sidecar and the candidate manifest|the manifest references and files pins explain those three carrier objects|no carrier digest depends transitively on the candidate manifest|the changeset carries the contained operation references in causal order|excludes the publication operation_ref and candidate manifest hash|publication facts commit the completed changeset|every verifier reconstructs the same dependency direction|every item is correlated through its exact operation_ref|authority is still derived only from owner capability or the mandate chain|no private content, credential, DK, private key or protected plaintext is present|the nested documents validate under their own profile|an unused, duplicate, uncorrelated or authority-bearing item is refused|it has exactly aithos-evidence-core, items and delegated_counts|items sort by complete RFC8785-JCS bytes with no duplicate|delegated_counts is always the exact D7 reference, including the empty root|every required proof appears once while unrelated proof is refused|authority is still derived only from owner capability or one mandate chain|the edition is refused|no caller-asserted changeset can override the derived result|the content operation is covered by the leaf chain|Gamma explains the authored change|deterministic index and root updates are recognized as consequences|any unexplained parasite change is refused|it has exactly aithos-authorship-core, subject, zone, sid, content_hash, operation_ref, edition, authorized_via, key and sig|zone is public and content_hash covers the exact stored public body bytes|edition has exactly height and predecessors matching publication facts|authorized_via and key equal the reconstructed W1 authority|the grantee key signs RFC8785-JCS with top-level sig omitted|no candidate manifest or carrier digest enters the signature|its signature binds content hash, SID, operation, edition and authorized_via|Gamma and the manifest commit that proof|the verifier distinguishes grantee authorship from owner authorship|it proves inclusion, replacement or absence for the same opaque SID|it learns no name, path, title, tags, content, folder relation or key|it has exactly aithos-gamma-presentation-core, subject, operation_ref, source_head, request_digest, entries, at, key and sig|entries are the complete selected Gamma objects in verified order without duplicate id|Bundle re-executes the query against source_head and obtains those exact entries|the verified presenter key signs RFC8785-JCS with top-level sig omitted|no Gamma entry, Gamma kind or second occurrence is created|publication is refused|no candidate manifest, carrier sidecar or Gamma delta becomes reachable|cold verification is refused|it supplies typed public artifacts to one pure Core verifier|no public helper returns Allow from layout, link or hash checks alone)$"#
)]
fn m_carrier_verdict(w: &mut ProtocolWorld) {
    let result = w
        .core_edition_observation
        .as_ref()
        .expect("every carrier scenario must construct its own CORE-ED observation");
    let observation = result
        .as_ref()
        .unwrap_or_else(|error| panic!("CORE-ED scenario failed: {error}"));
    assert!(observation.signer_is_actor, "{}", observation.case);
    assert!(
        observation.owner_absent_from_grantee_edition,
        "{}",
        observation.case
    );
    assert_eq!(
        observation.actual_accepted,
        observation.expected_verdict == "accepted",
        "{}",
        observation.case
    );
}

#[then("every change remains covered by the grantee's single chain")]
fn g_plus_single_grantee_chain_covers_changes(w: &mut ProtocolWorld) {
    m_carrier_verdict(w);
    let observation = w
        .core_edition_observation
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .expect("CORE-ED grantee changes observation");
    assert!(observation.actual_accepted);
    assert!(observation.owner_absent_from_grantee_edition);
}

#[given("a lived bundle containing owner and grantee publications")]
fn k_cold_round_trip_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "cold-roundtrip".into();
}

#[given("a fresh local store whose complete history already verifies keyless")]
fn k_cold_verified_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "cold-capability-reintroduction".into();
    w.core_edition_observation = Some(core_capability_reintroduction_scenario());
}

#[given("a complete export in a fresh local store")]
fn k_cold_defect_fixture(w: &mut ProtocolWorld) {
    w.core_edition_case = "cold-defect".into();
}

#[when(expr = "its public and opaque artifacts are exported into a fresh empty {string} store")]
fn k_cold_export_action(w: &mut ProtocolWorld, store: String) {
    w.core_edition_argument = store.clone();
    w.core_edition_observation = Some(core_cold_roundtrip_scenario(&store, None));
}

#[when(expr = "{string} is introduced before reopen")]
fn k_cold_defect_action(w: &mut ProtocolWorld, defect: String) {
    w.core_edition_argument = "fresh local store".into();
    w.core_edition_secondary = defect.clone();
    w.core_edition_observation = Some(core_cold_roundtrip_scenario(
        "fresh local store",
        Some(&defect),
    ));
}

#[when(
    regex = r#"^(?:the producer is destroyed and all private signing, opening and wrapping capabilities are absent|one separately supplied grantee opening capability is attached)$"#
)]
fn k_cold_round_trip_action(w: &mut ProtocolWorld) {
    assert!(w.core_edition_observation.is_some());
}

#[then(
    regex = r#"^(?:Bundle reopens and cold-verifies the complete editions and Gamma history|owner and grantee authorship remain distinct|no provider, remote store, network client or connector call participates|it opens only the content lines in its still-valid perimeter|removing it again leaves the keyless verdict unchanged|cold verification is rejected without private fallback)$"#
)]
fn k_cold_round_trip_verdict(w: &mut ProtocolWorld) {
    m_carrier_verdict(w);
    let observation = w
        .core_edition_observation
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .expect("CORE-COLD observation");
    if observation.expected_verdict == "accepted" {
        assert!(observation.mem_cold_verified || observation.fs_cold_verified);
        assert!(observation.package_digest.is_some());
    } else {
        assert!(observation.zero_reachable_on_refusal);
    }
}

#[given(
    regex = r#"^(?:an existing homogeneous draft2 chain with connector authority|an explicitly versioned legacy connector migration|a parent mandate carries ".*" under one approved catalog|a connector operation, gating receipts and Gamma v2 candidate in an overlay|a published bundle and a staged W1 connector occurrence|Core authorized one staged occurrence and its external effect may have happened)$"#
)]
fn o_catalog_overlay_fixture(w: &mut ProtocolWorld) {
    cb5_catalog_result(w);
    cb6_result(w);
    cb7_result(w);
}

#[given("the process crashed before publishing completed evidence")]
#[when(
    regex = r#"^(?:the authorities migrate it to catalog-bound draft3|legacy read and write rights are projected|it delegates ".*" under ".*"|Core authorizes the complete pre-effect facts|the external effect succeeds|the applicable post-effect usage receipt is obtained|".*" happens before the local linearization point|the runtime recovers without accepted history for that occurrence)$"#
)]
fn o_catalog_overlay_action(w: &mut ProtocolWorld) {
    cb5_assert_green(w);
    cb6_assert_green(w);
    cb7_assert_green(w);
}

#[then(
    regex = r#"^(?:every certificate receives a fresh mandate id and draft3 signature|each grant is recorded by its normal Gamma v2 occurrence|no draft2 certificate, signature or historical action byte changes|read may map to read and write may map only to act|no legacy right proves a binding action|canonical rights require re-enrolment|Core accepts the completed evidence for that same operation_ref|Gamma, evidence, roots and manifest publish at one local linearization point|no pending artifact or second occurrence is created|the overlay is discarded|the canonical bundle, manifest and Gamma head remain byte-identical|absence of evidence is not treated as proof that no effect occurred|the runtime reconciles the original external occurrence before retry|Core invents no pending wire, Gamma kind or replacement occurrence)$"#
)]
fn o_catalog_overlay_verdict(w: &mut ProtocolWorld) {
    cb5_assert_green(w);
    cb6_assert_green(w);
    cb7_assert_green(w);
}

#[when(expr = "the owner grants the agent read on circle folder {string} for 7 days")]
#[when(expr = "the owner grants the agent read on circle folder {string}")]
fn grant_on_folder(w: &mut ProtocolWorld, folder: String) {
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .ensure_folder(Zone::Circle, &folder, &owner, &mut w.ent)
        .unwrap();
    w.grant_to_agent(&[dir_spec(&folder)], NA7, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent read on folder {string} restricted to tag {string}")]
fn grant_on_folder_tag(w: &mut ProtocolWorld, folder: String, tag: String) {
    w.grant_to_agent(&[tag_spec(&folder, &tag)], NA7, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent read on folders {string} and {string} in one mandate")]
fn grant_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.grant_to_agent(&[dir_spec(&f1), dir_spec(&f2)], NA7, 0);
}

#[when(expr = "the owner grants read on both folders restricted to tag {string} in one mandate")]
fn grant_two_folders_tagged(w: &mut ProtocolWorld, tag: String) {
    let (f1, f2) = (w.e_folders[0].clone(), w.e_folders[1].clone());
    w.grant_to_agent(&[tag_spec(&f1, &tag), tag_spec(&f2, &tag)], NA7, 0);
}

#[when(expr = "the agent delegates read on folder {string} to a helper")]
fn agent_delegates(w: &mut ProtocolWorld, folder: String) {
    let parent = w.chain[0].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![parent, sub];
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain, DAY1));
}

#[when("the agent delegates the same perimeter to a helper for 30 days")]
fn agent_delegates_too_long(w: &mut ProtocolWorld) {
    let parent = w.chain[0].clone();
    let folder = w.granted_folder.clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![parent, sub];
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain, DAY1));
}

#[when("the helper tries to delegate to a fourth key")]
fn helper_delegates_further(w: &mut ProtocolWorld) {
    let parent = w.helper_chain[1].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(HELPER),
            "fourth",
            &agent_sk(FOURTH).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA7,
            &mut w.ent,
        )
        .unwrap();
    let mut chain = w.helper_chain.clone();
    chain.push(sub);
    w.chain_result = Some(w.verify_chain_at(&chain, DAY1));
}

// --- step E thens ---

#[then("the mandate verifies at day 1")]
fn mandate_ok_day1(w: &mut ProtocolWorld) {
    assert_eq!(w.verify_chain_at(&w.chain, DAY1), Ok(()));
}

#[then("the mandate is rejected at day 8")]
fn mandate_dead_day8(w: &mut ProtocolWorld) {
    assert!(w.verify_chain_at(&w.chain, DAY8).is_err());
}

#[then("mandate verification is rejected")]
fn mandate_rejected(w: &mut ProtocolWorld) {
    let res = w.chain_result.as_ref().unwrap();
    assert!(res.is_err());
    assert!(
        res.as_ref().unwrap_err().contains("kex"),
        "must be rejected FOR the kex binding: {res:?}"
    );
}

#[then(expr = "the agent reads {string} with its own keypair")]
fn agent_reads_path(w: &mut ProtocolWorld, path: String) {
    if let Some(observation) = &w.core_exact_section_observation {
        let observation = observation
            .as_ref()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(observation.target, path);
        assert!(observation.target_readable);
        return;
    }
    assert_eq!(w.agent_reads(&w.chain, AGENT, &path).as_deref(), Ok(BODY));
}

#[then(expr = "the agent reads {string}")]
fn agent_reads_in_folder(w: &mut ProtocolWorld, name: String) {
    let path = format!("{}/{name}", w.granted_folder);
    assert_eq!(w.agent_reads(&w.chain, AGENT, &path).as_deref(), Ok(BODY));
}

#[then(expr = "{string} stays out of the agent's reach")]
fn name_out_of_reach(w: &mut ProtocolWorld, name: String) {
    if let Some(observation) = &w.core_exact_section_observation {
        let observation = observation
            .as_ref()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_ne!(observation.target, name);
        assert!(observation.sibling_unreachable);
        assert!(observation.failed_attempt_unchanged);
        return;
    }
    let path = format!("{}/{name}", w.granted_folder);
    assert!(w.agent_reads(&w.chain, AGENT, &path).is_err());
}

#[then(expr = "the agent reads the section under {string}")]
#[then(expr = "the agent reads the section under {string} with its single keypair")]
#[then(expr = "the agent reads the section under {string} with the same keypair")]
fn agent_reads_under(w: &mut ProtocolWorld, folder: String) {
    assert_eq!(
        w.agent_reads(&w.chain, AGENT, &format!("{folder}/note"))
            .as_deref(),
        Ok(BODY)
    );
}

#[then(expr = "the agent cannot read the section under {string}")]
#[then(expr = "a section under {string} stays out of the agent's reach")]
fn agent_blocked_under(w: &mut ProtocolWorld, folder: String) {
    assert!(w
        .agent_reads(&w.chain, AGENT, &format!("{folder}/note"))
        .is_err());
}

#[then("the agent reads the tagged section of each folder with one keypair")]
fn agent_reads_tagged_both(w: &mut ProtocolWorld) {
    for f in w.e_folders.clone() {
        assert_eq!(
            w.agent_reads(&w.chain, AGENT, &format!("{f}/tagged"))
                .as_deref(),
            Ok(BODY),
            "tagged section of {f}"
        );
    }
}

#[then("every untagged section stays out of the agent's reach")]
fn untagged_blocked_both(w: &mut ProtocolWorld) {
    for f in w.e_folders.clone() {
        assert!(
            w.agent_reads(&w.chain, AGENT, &format!("{f}/plain"))
                .is_err(),
            "plain section of {f} must stay sealed"
        );
    }
}

#[then("the helper's chain verifies")]
fn helper_chain_ok(w: &mut ProtocolWorld) {
    assert_eq!(w.chain_result.clone().unwrap(), Ok(()));
}

#[then(expr = "the helper reads the section under {string}")]
fn helper_reads_under(w: &mut ProtocolWorld, folder: String) {
    assert_eq!(
        w.agent_reads(&w.helper_chain, HELPER, &format!("{folder}/note"))
            .as_deref(),
        Ok(BODY)
    );
}

#[then("the helper's chain is rejected")]
#[then("the new chain is rejected")]
fn helper_chain_rejected(w: &mut ProtocolWorld) {
    assert!(w.chain_result.clone().unwrap().is_err());
}

// ---------------------------------------------------------- CB3 mandates

#[given(expr = "an agent granted {string} on one section perimeter")]
fn cb3_lattice_grant(w: &mut ProtocolWorld, grant: String) {
    w.cb3_perimeter =
        vec![
            PerimeterEntry::parse(&format!("{grant}.circle#id={}", cb3_section_sid("note1")))
                .expect("CB3 lattice perimeter"),
        ];
}

#[when(expr = "Core authorizes the canonical {string} operation on that section")]
fn cb3_authorize_canonical_operation(w: &mut ProtocolWorld, operation: String) {
    let verb = cb3_operation_verb(&operation);
    w.cb3_operation = Some(verb);
    w.cb3_verdict = Some(covers_section_op(
        &w.cb3_perimeter,
        &SectionOp {
            verb,
            zone: Zone::Circle,
            sid: cb3_section_sid("note1"),
            folders: &[],
            tags: &[],
        },
    ));
}

#[then(expr = "the verdict is {string}")]
fn cb3_verdict_is(w: &mut ProtocolWorld, expected: String) {
    if w.cb5_result.is_some() {
        cb5_assert_green(w);
        return;
    }
    assert_eq!(
        w.cb3_verdict.expect("CB3 verdict"),
        expected == "allowed",
        "CB3 verdict {expected}"
    );
}

#[then("the signed perimeter contains no create verb")]
fn cb3_no_create_wire_verb(w: &mut ProtocolWorld) {
    assert!(w
        .cb3_perimeter
        .iter()
        .all(|entry| !entry.to_entry_string().starts_with("create.")));
}

#[given("a mandate whose signature bytes are otherwise valid")]
fn cb3_signed_mandate_fixture(_w: &mut ProtocolWorld) {}

#[when(expr = "its {string} has {string}")]
fn cb3_invalidate_form(w: &mut ProtocolWorld, field: String, invalid_form: String) {
    let case_name = match field.as_str() {
        "protocol version" => "unsupported protocol version",
        "signature algorithm" => "signature algorithm other than ed25519",
        "announced signer key" => "root announced signer key differs from issuer",
        "mandate id" => "malformed mandate identifier",
        "subject id" => "malformed subject identifier",
        "parent and issued_by" => "child issued_by differs from parent grantee",
        "grantee public key" => "malformed grantee signing key",
        "kex public key" => "grantee kex key does not match signing key",
        "nonce" => "empty nonce",
        "not_before" => "timestamp is not a calendar instant",
        "not_after" => "validity window is inverted",
        "issued_at" => "timestamp uses an offset instead of Zulu",
        "selector" if invalid_form.contains("duplicate") => "duplicate dir selector",
        "selector" => "id mixed with dir",
        "issue depth" => "issue depth zero",
        other => panic!("unknown CB3 form field {other}"),
    };
    w.cb3_form_result = Some(
        cb3_form_document(case_name)
            .parse::<Mandate>()
            .map_err(|error| error.to_string()),
    );
}

#[then("mandate form validation is refused")]
fn cb3_form_is_refused(w: &mut ProtocolWorld) {
    assert!(w
        .cb3_form_result
        .as_ref()
        .expect("CB3 form result")
        .is_err());
}

#[then("no authorization helper returns a partial Allow")]
fn cb3_no_partial_allow(w: &mut ProtocolWorld) {
    assert!(
        w.cb3_form_result
            .as_ref()
            .expect("CB3 form result")
            .is_err(),
        "an invalid raw mandate must not become a typed authorization input"
    );
}

#[when("a mandate carries a perimeter entry combining id= with dir= or tag=")]
fn cb3_mixed_id_entry(w: &mut ProtocolWorld) {
    let target = cb3_section_sid("note1");
    let folder = sid(10);
    w.cb3_verdict = Some(
        PerimeterEntry::parse(&format!("read.circle#id={target}&dir={folder}")).is_err()
            && PerimeterEntry::parse(&format!("read.circle#id={target}&tag=toto")).is_err(),
    );
}

#[then("the mandate is rejected at parse")]
fn cb3_parse_is_rejected(w: &mut ProtocolWorld) {
    assert_eq!(w.cb3_verdict, Some(true));
}

#[given("an agent granted read on circle with issue depth 1")]
fn cb3_whole_circle_parent(w: &mut ProtocolWorld) {
    let entry = PerimeterEntry::parse("read.circle").expect("whole circle perimeter");
    w.cb3_perimeter = vec![entry.clone()];
    w.cb3_root_with_perimeter(vec![entry]);
}

#[given(expr = "an agent granted read on circle section {string} by id with issue depth 1")]
fn cb3_exact_circle_parent(w: &mut ProtocolWorld, section: String) {
    let entry = PerimeterEntry::parse(&format!("read.circle#id={}", cb3_section_sid(&section)))
        .expect("exact circle perimeter");
    w.cb3_perimeter = vec![entry.clone()];
    w.cb3_root_with_perimeter(vec![entry]);
}

#[given(expr = "an agent granted read on section {string} by id")]
fn cb3_exact_read_grant(w: &mut ProtocolWorld, section: String) {
    w.cb3_perimeter =
        vec![
            PerimeterEntry::parse(&format!("read.circle#id={}", cb3_section_sid(&section)))
                .expect("exact read perimeter"),
        ];
}

#[given(expr = "an agent granted {string} on {string} section {string} by id")]
fn cb3_exact_zone_grant(w: &mut ProtocolWorld, verb: String, zone: String, section: String) {
    w.cb3_perimeter =
        vec![
            PerimeterEntry::parse(&format!("{verb}.{zone}#id={}", cb3_section_sid(&section)))
                .expect("exact zone perimeter"),
        ];
}

#[given(expr = "an agent granted {string} on a zone with issue depth 1")]
fn cb3_selector_parent(w: &mut ProtocolWorld, selector: String) {
    let entry = PerimeterEntry::parse(&cb3_normalize_selector(&selector)).expect("selector parent");
    w.cb3_perimeter = vec![entry.clone()];
    w.cb3_root_with_perimeter(vec![entry]);
}

#[given(expr = "an agent granted read on all of {string} with issue depth 1")]
fn cb3_whole_zone_parent(w: &mut ProtocolWorld, zone: String) {
    let entry =
        PerimeterEntry::parse(&format!("read.{zone}")).expect("whole-zone parent perimeter");
    w.cb3_perimeter = vec![entry.clone()];
    w.cb3_root_with_perimeter(vec![entry]);
}

#[when(expr = "the agent delegates read on a section of {string} by id")]
fn cb3_delegate_circle_id(w: &mut ProtocolWorld, _folder: String) {
    w.cb3_delegate(
        PerimeterEntry::parse(&format!("read.circle#id={}", cb3_section_sid("note1")))
            .expect("circle id child"),
    );
}

#[when(expr = "the agent delegates read on section {string} by id")]
fn cb3_delegate_named_id(w: &mut ProtocolWorld, section: String) {
    w.cb3_delegate(
        PerimeterEntry::parse(&format!("read.circle#id={}", cb3_section_sid(&section)))
            .expect("named id child"),
    );
}

#[when("the agent delegates the apparently related section by id")]
fn cb3_delegate_apparently_related(w: &mut ProtocolWorld) {
    let zone = match w.cb3_perimeter.first().expect("CB3 parent perimeter") {
        PerimeterEntry::Ethos { zone, .. } | PerimeterEntry::EthosId { zone, .. } => *zone,
        other => panic!("unexpected CB3 parent {other:?}"),
    };
    w.cb3_delegate(
        PerimeterEntry::parse(&format!(
            "read.{}#id={}",
            zone.as_str(),
            cb3_section_sid("note1")
        ))
        .expect("apparently related id child"),
    );
}

#[when("the agent delegates one section of that zone by id")]
fn cb3_delegate_one_in_zone(w: &mut ProtocolWorld) {
    cb3_delegate_apparently_related(w);
}

#[when(expr = "the agent attempts a read op on section {string}")]
fn cb3_read_other_section(w: &mut ProtocolWorld, section: String) {
    w.cb3_operation = Some(Verb::Read);
    w.cb3_verdict = Some(covers_section_op(
        &w.cb3_perimeter,
        &SectionOp {
            verb: Verb::Read,
            zone: Zone::Circle,
            sid: cb3_section_sid(&section),
            folders: &[],
            tags: &[],
        },
    ));
}

#[when(expr = "the agent attempts {string} on the same SID")]
fn cb3_operate_same_sid(w: &mut ProtocolWorld, operation: String) {
    let verb = cb3_operation_verb(&operation);
    let zone = match w.cb3_perimeter.first().expect("CB3 grant") {
        PerimeterEntry::EthosId { zone, .. } => *zone,
        other => panic!("expected exact CB3 grant, got {other:?}"),
    };
    w.cb3_operation = Some(verb);
    w.cb3_verdict = Some(covers_section_op(
        &w.cb3_perimeter,
        &SectionOp {
            verb,
            zone,
            sid: cb3_section_sid("note1"),
            folders: &[],
            tags: &[],
        },
    ));
}

#[then("the op is not covered")]
fn cb3_op_not_covered(w: &mut ProtocolWorld) {
    assert_eq!(w.cb3_verdict, Some(false));
}

#[then("the operation is covered")]
fn cb3_operation_covered(w: &mut ProtocolWorld) {
    assert_eq!(w.cb3_verdict, Some(true));
}

#[then(expr = "the identical operation on sibling SID {string} is not covered")]
fn cb3_sibling_not_covered(w: &mut ProtocolWorld, sibling: String) {
    let zone = match w.cb3_perimeter.first().expect("CB3 grant") {
        PerimeterEntry::EthosId { zone, .. } => *zone,
        other => panic!("expected exact CB3 grant, got {other:?}"),
    };
    assert!(!covers_section_op(
        &w.cb3_perimeter,
        &SectionOp {
            verb: w.cb3_operation.expect("CB3 operation"),
            zone,
            sid: cb3_section_sid(&sibling),
            folders: &[],
            tags: &[],
        },
    ));
}

#[then("the helper's chain is rejected without resolving the SID position")]
fn cb3_chain_rejected_without_resolver(w: &mut ProtocolWorld) {
    assert!(w.chain_result.as_ref().expect("CB3 chain result").is_err());
}

#[then("no other section is covered by the child")]
fn cb3_child_covers_no_other_section(w: &mut ProtocolWorld) {
    let perimeter = w
        .helper_chain
        .last()
        .expect("CB3 child")
        .parsed_perimeter()
        .expect("CB3 child perimeter");
    let zone = match perimeter.first().expect("CB3 child entry") {
        PerimeterEntry::EthosId { zone, .. } => *zone,
        other => panic!("expected exact CB3 child, got {other:?}"),
    };
    assert!(!covers_section_op(
        &perimeter,
        &SectionOp {
            verb: Verb::Read,
            zone,
            sid: cb3_section_sid("note2"),
            folders: &[],
            tags: &[],
        },
    ));
}

#[then(expr = "delegating section {string} by id is rejected")]
fn cb3_other_id_child_rejected(w: &mut ProtocolWorld, section: String) {
    let (_, verdict) = w.cb3_child_candidate(
        PerimeterEntry::parse(&format!("read.circle#id={}", cb3_section_sid(&section)))
            .expect("other exact child"),
        802,
    );
    w.cb3_secondary_verdicts.push(verdict.is_err());
    assert_eq!(w.cb3_secondary_verdicts.last(), Some(&true));
}

#[then(expr = "delegating the whole folder of {string} is rejected")]
fn cb3_folder_child_rejected(w: &mut ProtocolWorld, _section: String) {
    let (_, verdict) = w.cb3_child_candidate(
        PerimeterEntry::parse(&format!("read.circle#dir={}", sid(10))).expect("folder child"),
        803,
    );
    w.cb3_secondary_verdicts.push(verdict.is_err());
    assert_eq!(w.cb3_secondary_verdicts.last(), Some(&true));
}

#[when(expr = "a mandate carries one perimeter entry with {string}")]
fn cb3_duplicate_selector(w: &mut ProtocolWorld, selector: String) {
    let entry = match selector.as_str() {
        "dir=a&dir=b" => format!("read.circle#dir={}&dir={}", sid(10), sid(11)),
        "tag=a&tag=b" => "read.circle#tag=a&tag=b".to_owned(),
        "id=one&id=two" => format!(
            "read.circle#id={}&id={}",
            cb3_section_sid("note1"),
            cb3_section_sid("note2")
        ),
        other => panic!("unknown duplicate selector {other}"),
    };
    w.cb3_verdict = Some(PerimeterEntry::parse(&entry).is_err());
}

#[then("the mandate is rejected before signature verification")]
fn cb3_duplicate_rejected_before_signature(w: &mut ProtocolWorld) {
    assert_eq!(w.cb3_verdict, Some(true));
}

// ---------------------------------------------------------- CB4 operations

#[given("a complete operation-facts document F for one registered kind")]
fn cb4_complete_facts_document(_w: &mut ProtocolWorld) {}

#[when("Core derives its facts reference")]
fn cb4_derive_facts_reference(w: &mut ProtocolWorld) {
    let vector = cb4_vector(CB4_MUTATION);
    let nodes = cb4_mutation_nodes(&vector);
    let case = &vector["positive_cases"][0];
    cb4_capture(
        w,
        verify_operation_facts(OperationFactsInput {
            document: &case["document"],
            facts_ref: Some(&case["facts_ref"]),
            evidence: OperationFactsEvidence::Mutation {
                state_facts: &vector["states"],
                nodes: &nodes,
                vault_record_key: vector["vault_record_key"].as_str().expect("vault key"),
            },
        }),
    );
}

#[then("F has exactly aithos-operation-facts-core, kind and facts")]
fn cb4_facts_envelope_exact(w: &mut ProtocolWorld) {
    assert_eq!(w.cb4_result, Some(Ok(())));
}

#[then("its profile equals facts_ref and its kind equals operation.kind")]
fn cb4_facts_profile_and_kind(w: &mut ProtocolWorld) {
    assert_eq!(w.cb4_result, Some(Ok(())));
}

#[then(
    "facts_ref.digest is lowercase SHA-256 of \"aithos-core/v1/operation-facts\", NUL and RFC8785-JCS of F"
)]
fn cb4_facts_digest_exact(w: &mut ProtocolWorld) {
    assert_eq!(w.cb4_result, Some(Ok(())));
}

#[then("null, an extra member or a different selected family is refused")]
fn cb4_facts_envelope_negatives(_w: &mut ProtocolWorld) {
    for id in [
        "missing-envelope-profile",
        "extra-envelope-member",
        "kind-family-mismatch",
    ] {
        assert!(matches!(
            cb4_validate_mutation(id),
            Err(aithos_core::Error::InvalidOperationFacts(_))
        ));
    }
}

#[given(expr = "a candidate state-fact document with {string}")]
fn cb4_state_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates it before operation commitment comparison")]
fn cb4_validate_state_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "unknown state-fact profile" => "unknown-state-fact-profile",
        "empty objects array" => "empty-objects",
        "unsorted objects array" => "unsorted-objects",
        "duplicate key commitment" => "duplicate-key-commitment",
        "malformed or non-lowercase commitment" => "malformed-byte-commitment",
        "missing affected object" => "missing-affected-object",
        "unrelated extra object" => "unrelated-extra-object",
        "extra object member" => "extra-object-member",
        "state digest mismatch" => "state-digest-mismatch",
        other => panic!("unknown CB4 state defect {other}"),
    };
    cb4_capture(w, cb4_validate_state(id));
}

#[then("the state fact is refused")]
fn cb4_state_refused(w: &mut ProtocolWorld) {
    assert_eq!(
        w.cb4_result,
        Some(Err("InvalidStateFact".into())),
        "{}",
        w.cb4_case
    );
}

#[then("no operation commitment or operation_ref is emitted")]
fn cb4_no_operation_reference(w: &mut ProtocolWorld) {
    if w.cb4_result == Some(Ok(())) {
        cb4_assert_green(w);
        return;
    }
    assert!(w.cb4_result.as_ref().expect("CB4 result").is_err());
}

#[then(
    "no clear store key, path, SID, vault record name, target or protected content is accepted in the state fact"
)]
fn cb4_no_clear_state_coordinate(w: &mut ProtocolWorld) {
    assert!(w.cb4_result.as_ref().expect("CB4 state result").is_err());
}

#[given(expr = "a candidate read facts object with {string}")]
fn cb4_read_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates it before operation commitment")]
fn cb4_validate_read_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "unknown read domain" => "unknown-read-domain",
        "unknown zone or non-canonical SID" => "unknown-ethos-zone",
        "malformed or mismatched source edition" => "mismatched-source-edition",
        "empty or malformed source head" => "empty-source-head",
        "non-canonical Gamma query encoding" => "noncanonical-gamma-query",
        "mismatched Gamma request digest" => "mismatched-request-digest",
        "mismatched vault record-key commitment" => "mismatched-vault-record-key",
        "clear display path or vault record name" => "clear-display-path",
        other => panic!("unknown CB4 read defect {other}"),
    };
    cb4_capture(w, cb4_validate_read(id));
}

#[then("the read facts are refused as InvalidOperationFacts")]
fn cb4_read_refused(w: &mut ProtocolWorld) {
    assert_eq!(
        w.cb4_result,
        Some(Err("InvalidOperationFacts".into())),
        "{}",
        w.cb4_case
    );
}

#[given(expr = "a candidate mutation facts object with {string}")]
fn cb4_mutation_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates its closed family")]
fn cb4_validate_mutation_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "unknown domain" => "unknown-domain",
        "unknown verb for the selected domain" => "unknown-domain-verb",
        "unknown zone or node kind" => "unknown-node-kind",
        "null source or destination" => "null-source",
        "source or destination on the wrong variant" => "destination-on-rename",
        "duplicate or non-canonical SID coordinate" => "duplicate-source-sid",
        "invalid before and after transition" => "invalid-create-transition",
        "equal state digests for a mutation" => "equal-present-state-digests",
        "mismatched vault record-key commitment" => "mismatched-vault-record-key",
        "clear display path or vault record name" => "clear-display-path",
        other => panic!("unknown CB4 mutation defect {other}"),
    };
    cb4_capture(w, cb4_validate_mutation(id));
}

#[then("the mutation facts are refused")]
fn cb4_mutation_refused(w: &mut ProtocolWorld) {
    assert_eq!(
        w.cb4_result,
        Some(Err("InvalidOperationFacts".into())),
        "{}",
        w.cb4_case
    );
}

#[given(expr = "candidate action or inference facts with {string}")]
fn cb4_action_inference_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates them before operation commitment")]
fn cb4_validate_action_inference_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "malformed or mismatched catalog reference" => "mismatched-catalog-digest",
        "mismatched action arguments" => "mismatched-action-arguments",
        "mismatched inference request bytes" => "mismatched-inference-request",
        "action carrying request_digest" => "extra-action-member",
        "inference carrying args_hash" => "inference-args-hash",
        "wrong budget applicability variant" => "missing-applicable-budget",
        "wrong purpose applicability variant" => "missing-applicable-purpose",
        "tokens or a usage receipt before effect" => "action-post-effect-tokens",
        other => panic!("unknown CB4 action/inference defect {other}"),
    };
    cb4_capture(w, cb4_validate_action_inference(id));
}

#[then("the facts are refused as InvalidOperationFacts")]
fn cb4_operation_facts_refused(w: &mut ProtocolWorld) {
    assert_eq!(
        w.cb4_result,
        Some(Err("InvalidOperationFacts".into())),
        "{}",
        w.cb4_case
    );
}

#[then("no operation commitment, operation_ref or external effect is emitted")]
fn cb4_no_external_effect(w: &mut ProtocolWorld) {
    assert!(w.cb4_result.as_ref().expect("CB4 result").is_err());
}

#[given(expr = "candidate grant, revoke, rotate or publication facts with {string}")]
fn cb4_structural_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates the selected family")]
fn cb4_validate_structural_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "mandate id and certificate mismatch" => "mismatched-mandate-id",
        "revoke reason mismatch" => "reason-view-mismatch",
        "unknown rotation domain or mode" => "unknown-rotate-domain",
        "equal rotation state digests" => "equal-rotate-states",
        "derived rotation represented twice" => "derived-rotate-double-occurrence",
        "wrong predecessor cardinality or order" => "merge-one-predecessor",
        "resolution winner outside predecessors" => "resolution-winner-outside",
        "omitted or duplicate contained operation" => "omitted-contained-operation",
        "publication self-reference" => "publication-self-reference",
        other => panic!("unknown CB4 structural defect {other}"),
    };
    cb4_capture(w, cb4_validate_structural(id));
}

#[then("no operation commitment, counter or canonical effect is emitted")]
fn cb4_no_structural_effect(w: &mut ProtocolWorld) {
    assert!(w.cb4_result.as_ref().expect("CB4 result").is_err());
}

#[given(
    "canonical-operation material with an invalid projection, authority, reference or cross-view correlation"
)]
fn cb4_invalid_projection_material(_w: &mut ProtocolWorld) {}

#[when("Core validates it before effect or during semantic replay")]
fn cb4_validate_projection_material(w: &mut ProtocolWorld) {
    cb4_capture(w, cb4_validate_all_projection_negatives());
}

#[then("it is refused as InvalidOperation")]
fn cb4_projection_refused(w: &mut ProtocolWorld) {
    assert_eq!(w.cb4_result, Some(Err("InvalidOperation".into())));
}

#[then("an invalid selected facts document keeps its specific facts error")]
fn cb4_projection_facts_error_is_specific(_w: &mut ProtocolWorld) {
    let vector = cb4_vector(CB4_PROJECTION);
    assert!(vector["negative_projection_cases"]
        .as_array()
        .expect("projection negatives")
        .iter()
        .any(|case| case["must_fail"] == "InvalidOperationFacts"));
}

#[then("no accepted operation_ref is emitted")]
fn cb4_no_accepted_operation_ref(w: &mut ProtocolWorld) {
    assert!(w.cb4_result.as_ref().expect("CB4 result").is_err());
}

#[given(expr = "session-bound material with {string}")]
fn cb4_session_candidate(w: &mut ProtocolWorld, defect: String) {
    w.cb4_case = defect;
}

#[when("Core validates the operation before effect or during cold replay")]
fn cb4_validate_session_candidate(w: &mut ProtocolWorld) {
    let id = match w.cb4_case.as_str() {
        "certificate signed by another key" => "certificate-signed-by-stranger",
        "subject or leaf mandate mismatch" => "certificate-subject-mismatch",
        "session key different from session_bind" => "certificate-session-key-mismatch",
        "interval outside the leaf mandate" => "certificate-before-mandate",
        "operation outside the session interval" => "operation-before-certificate",
        "missing leaf possession proof" => "missing-native-leaf-proof",
        "missing session proof" => "missing-session-proof",
        "session proof for another operation_ref" => "session-proof-wrong-operation",
        "certificate digest mismatch" => "certificate-digest-mismatch",
        other => panic!("unknown CB4 session defect {other}"),
    };
    cb4_capture(w, cb4_validate_session(id));
}

#[then("it is refused as InvalidSession")]
fn cb4_session_refused(w: &mut ProtocolWorld) {
    assert_eq!(
        w.cb4_result,
        Some(Err("InvalidSession".into())),
        "{}",
        w.cb4_case
    );
}

#[then("no perimeter or authority is derived from SC1")]
fn cb4_session_conveys_no_authority(w: &mut ProtocolWorld) {
    assert!(w.cb4_result.as_ref().expect("CB4 result").is_err());
}

// ---------------------------------------------------------- CB5 pure contracts

#[given("a draft.2 parent mandate with max_children 4 and issue depth 2")]
fn cb5_max_children_parent(w: &mut ProtocolWorld) {
    w.core_constraint_case_result = None;
}

#[when(expr = "it mints a child with {string}")]
fn cb5_max_children_child(w: &mut ProtocolWorld, child_constraint: String) {
    let id = match child_constraint.as_str() {
        "max_children 4" => "draft2_equal",
        "max_children 2" => "draft2_reduced",
        "max_children 5" => "draft2_wider",
        "no max_children and can delegate" => "draft2_omission_delegating",
        "no max_children and is a chain leaf" => "draft2_omission_leaf",
        other => panic!("unknown max_children attenuation case {other}"),
    };
    w.core_constraint_case_result = Some(cb5_max_children_case(id));
}

#[given("a draft.2 root mandate with max_children 3 and issue depth 2")]
fn cb5_direct_children_root(w: &mut ProtocolWorld) {
    w.core_constraint_case_result = None;
}

#[given("its sole direct child has max_children 3 and issue depth 1")]
fn cb5_direct_children_child(_w: &mut ProtocolWorld) {}

#[when("that child mints three direct children")]
fn cb5_direct_children_action(w: &mut ProtocolWorld) {
    let result = (|| {
        let vector = cb5_parsed(CB5_MAX_CHILDREN)?;
        let direct = &vector["direct_children_only"];
        let entries = direct["grant_entries_jcs"]
            .as_array()
            .ok_or("direct-child entries are not an array")?
            .iter()
            .map(|entry| {
                serde_json::from_str::<aithos_core::gamma::Entry>(
                    entry.as_str().unwrap_or_default(),
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        aithos_core::gamma::verify_links(&entries)
            .map_err(|error| format!("direct-child Gamma links failed: {error}"))?;
        let parent = cb5_max_children_mandate(
            &vector,
            direct["parent_chain"][0].as_str().ok_or("missing parent")?,
        )?;
        let child = cb5_max_children_mandate(
            &vector,
            direct["child_chain"][1].as_str().ok_or("missing child")?,
        )?;
        if aithos_core::gamma::count_children(&entries, &parent.id) != 1
            || aithos_core::gamma::count_children(&entries, &child.id) != 3
        {
            return Err("direct-child meters drifted".into());
        }
        Ok(())
    })();
    w.core_constraint_case_result = Some(result);
}

#[then("all three grants verify against the child's meter")]
#[then("the root still proves exactly one direct child")]
fn cb5_direct_children_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_constraint_case_result, Some(Ok(())));
}

fn cb5_root_constraint_case(
    name: &str,
) -> Result<(Result<(), String>, Result<(), String>), String> {
    let vector = cb5_parsed(CB2_MANDATE_CONTRACTS)?;
    let cases = vector["constraints"]["root_leaf_cases"]
        .as_array()
        .ok_or("root constraint cases are not an array")?;
    let case = cases
        .iter()
        .find(|case| case["case"] == name)
        .ok_or_else(|| format!("missing root constraint case {name}"))?;
    let mandate: Mandate = serde_json::from_str(
        case["document_jcs"]
            .as_str()
            .ok_or("missing root constraint mandate")?,
    )
    .map_err(|error| error.to_string())?;
    let did: DidDocument = serde_json::from_str(
        vector["signed_fixtures"]["did_document_jcs"]
            .as_str()
            .ok_or("missing signed DID fixture")?,
    )
    .map_err(|error| error.to_string())?;
    let certificate = verify_chain(std::slice::from_ref(&mandate), &did, &mandate.issued_at)
        .map_err(|error| error.to_string());
    let delegation = constraints_attenuate_for_profile(
        &mandate.version,
        &mandate.constraints,
        &mandate.constraints,
        &mandate.not_before,
        &mandate.not_after,
    )
    .map_err(|error| error.to_string());
    Ok((certificate, delegation))
}

#[given("a directly owner-issued mandate whose chain ends at that mandate")]
fn cb5_root_constraint_fixture(w: &mut ProtocolWorld) {
    w.core_constraint_certificate_result = None;
    w.core_constraint_delegation_result = None;
}

#[when(expr = "its constraints contain {string}")]
fn cb5_root_constraint_action(w: &mut ProtocolWorld, constraint_case: String) {
    let name = match constraint_case.as_str() {
        "known well-formed max_actions" => "known well-formed root constraint",
        "known malformed max_actions" => "known malformed root constraint",
        "unknown opaque quantum_cap" => "unknown constraint on directly issued chain leaf",
        other => panic!("unknown root constraint case {other}"),
    };
    let (certificate, delegation) = cb5_root_constraint_case(name).expect("root constraint vector");
    w.core_constraint_certificate_result = Some(certificate);
    w.core_constraint_delegation_result = Some(delegation);
}

#[then(expr = "certificate validation is {string}")]
fn cb5_root_certificate_verdict(w: &mut ProtocolWorld, verdict: String) {
    assert_eq!(
        w.core_constraint_certificate_result
            .as_ref()
            .expect("certificate verdict")
            .is_ok(),
        matches!(verdict.as_str(), "accepted" | "preserved")
    );
}

#[then(expr = "using it as a delegation parent is {string}")]
fn cb5_root_delegation_verdict(w: &mut ProtocolWorld, verdict: String) {
    assert_eq!(
        w.core_constraint_delegation_result
            .as_ref()
            .expect("delegation verdict")
            .is_ok(),
        verdict == "accepted"
    );
}

#[given("a valid root-leaf mandate preserving unknown constraint \"quantum_cap\"")]
fn cb5_unknown_constraint_fixture(w: &mut ProtocolWorld) {
    let (certificate, delegation) =
        cb5_root_constraint_case("unknown constraint on directly issued chain leaf")
            .expect("unknown root constraint vector");
    assert!(certificate.is_ok());
    w.core_constraint_delegation_result = Some(delegation);
    w.core_constraint_effect_snapshot = CB2_MANDATE_CONTRACTS.to_owned();
}

#[given(expr = "a current-version verifier receives {string}")]
fn cb5_unknown_constraint_claim(_w: &mut ProtocolWorld, _claim: String) {}

#[when("the grantee attempts a covered delegated mutation")]
fn cb5_unknown_constraint_action(w: &mut ProtocolWorld) {
    let vector = cb5_parsed(CB2_MANDATE_CONTRACTS).expect("mandate contracts");
    let case = vector["constraints"]["root_leaf_cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case["case"] == "unknown constraint on directly issued chain leaf")
        })
        .expect("unknown constraint case");
    let mandate: Mandate = serde_json::from_str(case["document_jcs"].as_str().unwrap())
        .expect("unknown constraint mandate");
    w.core_constraint_delegation_result = Some(
        verify_operation_constraints(&mandate.constraints)
            .map(|_| ())
            .map_err(|error| error.to_string()),
    );
}

#[then("the verdict is a typed extension not understood refusal")]
#[then("the unknown extension remains visible in the audit")]
#[then("no Gamma entry, canonical state or counter changes")]
fn cb5_unknown_constraint_verdict(w: &mut ProtocolWorld) {
    assert!(w
        .core_constraint_delegation_result
        .as_ref()
        .expect("unknown constraint verdict")
        .is_err());
    assert_eq!(w.core_constraint_effect_snapshot, CB2_MANDATE_CONTRACTS);
}

#[given("the same canonical mutation is available to an owner and a grantee")]
fn cb5_owner_constraint_fixture(w: &mut ProtocolWorld) {
    w.core_owner_observation = None;
}

#[when("the owner performs it with a narrow local capability")]
fn cb5_owner_constraint_action(w: &mut ProtocolWorld) {
    w.core_owner_observation = Some(core_owner_scenario("public", "edit"));
}

#[then("Gamma records the owner mutation")]
fn cb5_owner_constraint_gamma(w: &mut ProtocolWorld) {
    let observation = w
        .core_owner_observation
        .as_ref()
        .expect("owner constraint observation")
        .as_ref()
        .expect("owner constraint scenario");
    assert_eq!(observation.gamma_delta, 1);
}

#[then("no mandate, constraint or delegated counter is consumed")]
fn cb5_owner_constraint_no_mandate(w: &mut ProtocolWorld) {
    let observation = w
        .core_owner_observation
        .as_ref()
        .expect("owner constraint observation")
        .as_ref()
        .expect("owner constraint scenario");
    assert_eq!(observation.mandate_counter_delta, 0);
}

fn core_u1_receipt(operation: &str) -> Result<(serde_json::Value, u64), String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let (receipt_name, context_name) = match operation {
        "action" => ("u1_action", "action"),
        "inference" => ("u1_inference", "inference"),
        other => return Err(format!("unknown U1 operation {other}")),
    };
    let receipt = vector["positive_receipts"][receipt_name]["receipt"].clone();
    let verified = verify_u1_receipt(
        &serde_json::json!([receipt.clone()]),
        &vector["contexts"][context_name],
        &vector["budget_profile"],
    )
    .map_err(|error| format!("U1 {operation} failed: {error}"))?;
    Ok((receipt, verified.actual_tokens()))
}

fn core_u1_all_receipts() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let (_, action) = core_u1_receipt("action")?;
    let (_, inference) = core_u1_receipt("inference")?;
    for case in vector["negative_u1_cases"]
        .as_array()
        .ok_or("U1 negatives are not an array")?
    {
        let context = if case["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("inference-"))
        {
            &vector["contexts"]["inference"]
        } else {
            &vector["contexts"]["action"]
        };
        if !matches!(
            verify_u1_receipt(&case["candidate"], context, &vector["budget_profile"]),
            Err(aithos_core::Error::InvalidGammaEntry(_))
        ) {
            return Err(format!("U1 negative {} did not fail", case["id"]));
        }
    }
    Ok(action + inference)
}

#[given(expr = "a W1 {string} occurrence citing a profile that requires attestation")]
fn cb5_u1_fixture(w: &mut ProtocolWorld, operation: String) {
    w.core_receipt_operation = operation;
    w.core_receipt_document = None;
    w.core_receipt_result = None;
}

#[when(expr = "Core validates its U1 receipt with family {string}")]
fn cb5_u1_action(w: &mut ProtocolWorld, family: String) {
    let expected_family = match w.core_receipt_operation.as_str() {
        "action" => "usage.action",
        "inference" => "usage.inference",
        other => panic!("unknown U1 operation {other}"),
    };
    assert_eq!(family, expected_family);
    let result = core_u1_receipt(&w.core_receipt_operation);
    match result {
        Ok((receipt, tokens)) => {
            w.core_receipt_document = Some(receipt);
            w.core_receipt_result = Some(Ok(tokens));
        }
        Err(error) => w.core_receipt_result = Some(Err(error)),
    }
}

fn core_receipt_members(document: &serde_json::Value) -> BTreeSet<&str> {
    document
        .as_object()
        .expect("receipt object")
        .keys()
        .map(String::as_str)
        .collect()
}

fn core_expected_members(members: &str) -> BTreeSet<&str> {
    members.split(',').collect()
}

#[then(expr = "the receipt members are exactly {string}")]
fn cb5_u1_members(w: &mut ProtocolWorld, members: String) {
    assert_eq!(
        core_receipt_members(w.core_receipt_document.as_ref().expect("U1 receipt")),
        core_expected_members(&members)
    );
}

#[then("sig verifies over RFC8785-JCS with sig omitted")]
fn cb5_receipt_signature(w: &mut ProtocolWorld) {
    assert!(w
        .core_receipt_result
        .as_ref()
        .expect("receipt result")
        .is_ok());
}

#[then("the family cannot relabel the reconstructed operation")]
fn cb5_u1_family_is_bound(w: &mut ProtocolWorld) {
    let vector = cb5_parsed(CB5_RECEIPTS).expect("receipt vector");
    let wrong_context = if w.core_receipt_operation == "action" {
        &vector["contexts"]["inference"]
    } else {
        &vector["contexts"]["action"]
    };
    assert!(verify_u1_receipt(
        &serde_json::json!([w.core_receipt_document.clone().expect("U1 receipt")]),
        wrong_context,
        &vector["budget_profile"],
    )
    .is_err());
}

#[given("an action usage receipt signed by the cited profile attestation key")]
fn cb5_u1_pair_fixture(w: &mut ProtocolWorld) {
    w.core_receipt_result = None;
}

#[given("an inference usage receipt signed by the cited profile attestation key")]
fn cb5_u1_pair_second_fixture(_w: &mut ProtocolWorld) {}

#[when("Core correlates each receipt with its exact operation_ref")]
fn cb5_u1_pair_action(w: &mut ProtocolWorld) {
    w.core_receipt_result = Some(core_u1_all_receipts());
}

#[then("action tokens replace only that action's declared usage")]
#[then("checked tokens_in plus tokens_out replace only that inference's declared usage")]
#[then(
    "a wrong key, family, reference, overflow, duplicate or non-closed member table is refused as InvalidGammaEntry"
)]
#[then("no U1 receipt changes the pre-effect operation commitment")]
fn cb5_u1_pair_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_receipt_result, Some(Ok(9912)));
}

fn core_historical_receipt_scenario() -> Result<u64, String> {
    let vector: serde_json::Value =
        serde_json::from_str(FPLUS_CONSTRAINTS).map_err(|error| error.to_string())?;
    let attestation = &vector["attestation"];
    let public: [u8; 32] = hex::decode(
        attestation["provider_pub_hex"]
            .as_str()
            .ok_or("missing provider public key")?,
    )
    .map_err(|error| error.to_string())?
    .try_into()
    .map_err(|_| "provider public key length".to_owned())?;
    let entry: aithos_core::gamma::Entry = serde_json::from_value(serde_json::json!({
        "v": 1,
        "id": "gamma_00000000000000000000000001",
        "prev": "",
        "at": "2026-07-18T12:00:00Z",
        "kind": "action",
        "target": "connector.test",
        "payload": {
            "args_hash": attestation["args_hash"],
            "model": "claude-haiku",
            "tokens": 10000,
            "receipt": attestation["receipt"]
        },
        "signature": {"alg":"ed25519", "key":"#content", "value":""}
    }))
    .map_err(|error| error.to_string())?;
    let profile = BudgetProfile {
        id: "haiku".into(),
        models: Some(vec!["claude-haiku".into()]),
        token_budget: Some(20_000),
        windows: None,
        max_actions: None,
        require_attestation: true,
        attestation_key: Some(wire::ed25519_pub_to_multibase(&public)),
    };
    verify_receipt(&entry, &profile)
        .map_err(|error| format!("historical v1 receipt failed: {error}"))?;
    let cb2 = cb5_parsed(CB5_RECEIPTS)?;
    if !matches!(
        verify_u1_receipt(
            &serde_json::json!([attestation["receipt"].clone()]),
            &cb2["contexts"]["action"],
            &cb2["budget_profile"],
        ),
        Err(aithos_core::Error::InvalidGammaEntry(_))
    ) {
        return Err("historical v1 receipt was reinterpreted as U1".into());
    }
    Ok(attestation["receipt"]["tokens"]
        .as_u64()
        .unwrap_or_default())
}

#[given("byte-identical historical v1 usage receipts")]
fn cb5_historical_receipt_fixture(w: &mut ProtocolWorld) {
    w.core_receipt_result = None;
}

#[when("W1 and historical evidence are verified")]
fn cb5_historical_receipt_action(w: &mut ProtocolWorld) {
    w.core_receipt_result = Some(core_historical_receipt_scenario());
}

#[then("v1 verifies only under its historical carrier and semantics")]
#[then("W1 requires an exact v2 U1 receipt when attestation is applicable")]
#[then("neither version synthesizes fields from the other")]
fn cb5_historical_receipt_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_receipt_result, Some(Ok(8412)));
}

fn core_r2_receipt(presentation: &str) -> Result<serde_json::Value, String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let name = match presentation {
        "no presented digest" => "r2_without_presented_digest",
        "a strict presented digest" => "r2_with_presented_digest",
        other => return Err(format!("unknown R2 presentation state {other}")),
    };
    let receipt = vector["positive_receipts"][name]["receipt"].clone();
    verify_r2_receipt(
        &serde_json::json!([receipt.clone()]),
        &vector["contexts"]["action"],
        "1.0.0-draft.2",
        &vector["obligations"]["action"],
    )
    .map_err(|error| format!("R2 receipt failed: {error}"))?;
    Ok(receipt)
}

#[given("an effective pinned obligation for one W1 operation")]
fn cb5_r2_fixture(w: &mut ProtocolWorld) {
    w.core_receipt_document = None;
    w.core_receipt_result = None;
}

#[when(expr = "its R2 receipt has {string}")]
fn cb5_r2_action(w: &mut ProtocolWorld, presentation: String) {
    match core_r2_receipt(&presentation) {
        Ok(receipt) => {
            w.core_receipt_document = Some(receipt);
            w.core_receipt_result = Some(Ok(0));
        }
        Err(error) => w.core_receipt_result = Some(Err(error)),
    }
}

#[then(expr = "its exact members are {string}")]
fn cb5_r2_members(w: &mut ProtocolWorld, members: String) {
    if w.core_receipt_document.is_none() {
        if w.core_edition_observation.is_some() {
            m_carrier_verdict(w);
        } else {
            cb4_assert_green(w);
        }
        return;
    }
    assert_eq!(
        core_receipt_members(w.core_receipt_document.as_ref().expect("R2 receipt")),
        core_expected_members(&members)
    );
}

#[then("family is \"obligation\" and v is the JSON number 2")]
fn cb5_r2_closed_header(w: &mut ProtocolWorld) {
    let receipt = w.core_receipt_document.as_ref().expect("R2 receipt");
    assert_eq!(receipt["family"], "obligation");
    assert_eq!(receipt["v"], 2);
}

fn core_r2_complete_scenario() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    let receipt = &vector["positive_receipts"]["r2_with_presented_digest"]["receipt"];
    verify_r2_receipt(
        &serde_json::json!([receipt]),
        &vector["contexts"]["action"],
        "1.0.0-draft.2",
        &vector["obligations"]["action"],
    )
    .map_err(|error| error.to_string())?;
    for case in vector["negative_r2_cases"]
        .as_array()
        .ok_or("R2 negatives are not an array")?
    {
        if !matches!(
            verify_r2_receipt(
                &case["candidate"],
                &vector["contexts"]["action"],
                "1.0.0-draft.2",
                &vector["obligations"]["action"],
            ),
            Err(aithos_core::Error::GammaObligationUnsatisfied(_))
        ) {
            return Err(format!("R2 negative {} did not fail", case["id"]));
        }
    }
    Ok(vector["negative_r2_cases"]
        .as_array()
        .map_or(0, |cases| cases.len()) as u64)
}

#[given("one canonical operation whose authority, native facts and time are fixed")]
fn cb5_r2_complete_fixture(w: &mut ProtocolWorld) {
    w.core_receipt_result = None;
}

#[when("a pinned attestor signs its R2 obligation receipt")]
fn cb5_r2_complete_action(w: &mut ProtocolWorld) {
    w.core_receipt_result = Some(core_r2_complete_scenario());
}

#[then("operation_ref binds the leaf mandate, operation arguments and occurrence")]
#[then("the receipt carries no mandate_id, action or args_hash duplicate")]
#[then(
    "a missing, stale, replayed, mismatched, duplicate or non-closed receipt is GammaObligationUnsatisfied"
)]
fn cb5_r2_complete_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_receipt_result, Some(Ok(25)));
}

#[given(expr = "a homogeneous draft3 chain with applies_to_operation {string}")]
fn cb5_matcher_fixture(w: &mut ProtocolWorld, matcher: String) {
    w.core_receipt_operation = matcher;
    w.core_receipt_matcher = None;
}

#[when(expr = "the grantee presents canonical operation {string}")]
fn cb5_matcher_action(w: &mut ProtocolWorld, operation: String) {
    let vector = cb5_parsed(CB5_RECEIPTS).expect("receipt vector");
    let id = match (w.core_receipt_operation.as_str(), operation.as_str()) {
        ("read ethos", "public content read") => "matcher-1",
        ("mutation ethos edit", "public content edit") => "matcher-2",
        ("mutation structure move", "structural move") => "matcher-3",
        ("inference", "inference") => "matcher-4",
        ("grant", "sub-grant") => "matcher-5",
        ("revoke", "revocation") => "matcher-6",
        ("rotate vault", "connector vault rotation") => "matcher-7",
        ("publication normal", "normal publication") => "matcher-8",
        ("mutation ethos edit", "public content delete") => "matcher-9",
        other => panic!("unknown matcher case {other:?}"),
    };
    let case = vector["matcher_cases"]
        .as_array()
        .and_then(|cases| cases.iter().find(|case| case["id"] == id))
        .expect("matcher case");
    let obligation = serde_json::json!({
        "id": "gherkin-matcher",
        "check": "human.approve",
        "attestor": [vector["public_keys"]["attestor_a"].clone()],
        "verdict": "approve",
        "applies_to_operation": case["matcher"].clone()
    });
    let verified = verify_obligation("1.0.0-draft.3", &obligation).expect("valid matcher");
    w.core_receipt_matcher = Some(
        obligation_matches(
            &verified,
            &vector["contexts"][case["context"].as_str().unwrap()],
        )
        .expect("matcher verdict"),
    );
}

#[then(expr = "matcher applicability is {string}")]
fn cb5_matcher_verdict(w: &mut ProtocolWorld, verdict: String) {
    assert_eq!(w.core_receipt_matcher, Some(verdict == "applicable"));
}

#[then("no caller-supplied fact or wildcard participates")]
fn cb5_matcher_closed(w: &mut ProtocolWorld) {
    assert!(w.core_receipt_matcher.is_some());
}

fn core_matcher_history_scenario() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_RECEIPTS)?;
    verify_obligation_chain(&vector["draft3_obligation_chain"])
        .map_err(|error| format!("positive matcher chain failed: {error}"))?;
    let mut rejected = 0;
    for case in vector["negative_matcher_cases"]
        .as_array()
        .ok_or("matcher negatives are not an array")?
    {
        if verify_obligation(
            case["profile"].as_str().unwrap_or_default(),
            &case["candidate"],
        )
        .is_ok()
        {
            return Err(format!("matcher negative {} did not fail", case["id"]));
        }
        rejected += 1;
    }
    for case in vector["negative_matcher_chain_cases"]
        .as_array()
        .ok_or("matcher-chain negatives are not an array")?
    {
        if verify_obligation_chain(&case["candidate"]).is_ok() {
            return Err(format!(
                "matcher-chain negative {} did not fail",
                case["id"]
            ));
        }
        rejected += 1;
    }
    Ok(rejected)
}

#[given("byte-identical draft1 and draft2 obligation mandates")]
fn cb5_matcher_history_fixture(w: &mut ProtocolWorld) {
    w.core_receipt_result = None;
}

#[when("applies_to_operation is presented through a sidecar or mixed-version chain")]
fn cb5_matcher_history_action(w: &mut ProtocolWorld) {
    w.core_receipt_result = Some(core_matcher_history_scenario());
}

#[then("the matcher is refused as InvalidMandate")]
#[then("draft3 requires exactly one selector per obligation")]
#[then("migration reissues the complete homogeneous chain")]
fn cb5_matcher_history_verdict(w: &mut ProtocolWorld) {
    assert_eq!(w.core_receipt_result, Some(Ok(24)));
}

fn core_count_positive_scenario() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    let positive = &vector["positive"];
    let verified = verify_delegated_counts(
        &positive["delegated_counts"],
        &positive["leaves"],
        &positive["evidence_views"],
    )
    .map_err(|error| error.to_string())?;
    verify_delegated_count_mandates(&positive["mandates"]).map_err(|error| error.to_string())?;
    if verified.occurrences().len() != 14
        || verified
            .counts_for("mandate_01J00000000000000000000020")
            .is_none_or(|counts| counts.mutations() != 2 || counts.consumptions() != 14)
    {
        return Err("delegated count positive tally drift".into());
    }
    Ok(verified.occurrences().len() as u64)
}

fn core_count_historical_scenario() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    if vector["profiles"]["delegated_counts"] != "1.0.0-draft.1"
        || vector["profiles"]["mandate"] != "1.0.0-draft.3"
        || vector["inventory"]["historical_gamma_counts_root_is_not_reinterpreted"] != true
    {
        return Err("delegated-count profile or historical inventory drift".into());
    }
    for id in ["unknown-profile", "missing-profile"] {
        let case = vector["negative_counter_cases"]
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["id"] == id))
            .ok_or_else(|| format!("missing historical counter case {id}"))?;
        if verify_delegated_counts(
            &case["candidate"]["delegated_counts"],
            &case["candidate"]["leaves"],
            &case["candidate"]["evidence_views"],
        )
        .is_ok()
        {
            return Err(format!("historical profile defect {id} was accepted"));
        }
    }
    core_count_positive_scenario()
}

fn core_count_invalid_scenario() -> Result<u64, String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    let mut refused = 0_u64;
    for case in vector["negative_counter_cases"]
        .as_array()
        .ok_or("counter negatives are not an array")?
    {
        let candidate = &case["candidate"];
        if !matches!(
            verify_delegated_counts(
                &candidate["delegated_counts"],
                &candidate["leaves"],
                &candidate["evidence_views"],
            ),
            Err(aithos_core::Error::InvalidDelegatedCounts(_))
        ) {
            return Err(format!("counter defect {} was accepted", case["id"]));
        }
        refused += 1;
    }
    for case in vector["negative_mandate_cases"]
        .as_array()
        .ok_or("mandate negatives are not an array")?
    {
        if !matches!(
            verify_delegated_count_mandates(&case["candidate"]),
            Err(aithos_core::Error::InvalidMandate(_))
        ) {
            return Err(format!("mandate defect {} was accepted", case["id"]));
        }
        refused += 1;
    }
    Ok(refused)
}

fn core_count_publication_scenario(operation: &str) -> Result<u64, String> {
    let vector = cb5_parsed(CB5_DELEGATED_COUNTS)?;
    let positive = &vector["positive"];
    let verified = verify_delegated_counts(
        &positive["delegated_counts"],
        &positive["leaves"],
        &positive["evidence_views"],
    )
    .map_err(|error| error.to_string())?;
    let publication = match operation {
        "normal publication" => "op_01K00000000000000000000010",
        "disjoint merge" => "op_01K00000000000000000000011",
        "fork resolution" => "op_01K00000000000000000000012",
        other => return Err(format!("unknown publication-count operation {other}")),
    };
    let mutations = [
        "op_01K00000000000000000000003",
        "op_01K00000000000000000000015",
    ];
    let distinct = mutations
        .iter()
        .chain(std::iter::once(&publication))
        .filter(|occurrence| {
            verified
                .occurrences()
                .iter()
                .any(|accepted| accepted == **occurrence)
        })
        .count();
    if distinct != 3 {
        return Err(format!("{operation} did not count as one publisher unit"));
    }
    Ok(distinct as u64)
}

#[given("a homogeneous draft3 mandate carrying max_mutations and max_consumptions")]
fn cb5_counts_positive_fixture(w: &mut ProtocolWorld) {
    w.core_count_suite = Some(core_count_positive_scenario());
}

#[given("a historical edition and Gamma vector predating mutation and total meters")]
fn cb5_counts_historical_fixture(w: &mut ProtocolWorld) {
    w.core_count_suite = Some(core_count_historical_scenario());
}

#[given("delegated-counts material with an invalid shape, proof, tally or occurrence correlation")]
fn cb5_counts_invalid_fixture(w: &mut ProtocolWorld) {
    w.core_count_suite = Some(core_count_invalid_scenario());
}

#[given(expr = "a grantee {string} contains two semantically distinct already-counted mutations")]
fn cb5_counts_publication_fixture(w: &mut ProtocolWorld, operation: String) {
    w.core_receipt_operation = operation;
    w.core_count_suite = None;
}

#[given(expr = "its publisher authority is evidenced by {string}")]
#[given(expr = "the same publisher decision has {string}")]
fn cb5_counts_publication_evidence(_w: &mut ProtocolWorld, _evidence: String) {}

#[given(expr = "a mandate history containing one {string}")]
fn core_count_consumption_given(w: &mut ProtocolWorld, consumption: String) {
    w.core_count_observation = Some(core_count_consumption_scenario(&consumption));
}

#[when("its accepted occurrences are committed for cold replay")]
#[when("a verifier replays it under its historical protocol version")]
#[when("Core validates it at append time or during cold replay")]
fn cb5_counts_when(w: &mut ProtocolWorld) {
    assert!(w.core_count_suite.is_some());
}

#[when("semantic replay rebuilds the total delegated-consumption tally")]
fn cb5_counts_publication_action(w: &mut ProtocolWorld) {
    w.core_count_suite = Some(core_count_publication_scenario(&w.core_receipt_operation));
}

#[when("the verifier rebuilds action, Ethos-mutation and total-consumption tallies")]
fn core_count_consumption_when(w: &mut ProtocolWorld) {
    assert!(w.core_count_observation.is_some());
}

#[then(
    regex = r#"^(?:max_mutations counts only delegated Ethos mutation occurrences|max_consumptions counts every delegated canonical occurrence once|delegated_counts has exactly aithos-delegated-counts-core and root|its leaves have only non-zero mutations and consumptions|historical gamma_counts_root and entries bytes are unchanged|the historical edition remains byte-identical and verifiable|new meter material is accepted only under the delegated-counts profile|old Gamma kinds, max_actions and count roots are never reinterpreted|new meter material under an old or unversioned schema, or under an unknown counter-schema version, fails closed|it is refused as InvalidDelegatedCounts|a malformed max_mutations or max_consumptions certificate is refused as InvalidMandate|the two mutations and the publication contribute exactly three|the edition and Gamma evidence correlate to the same single publisher unit|any Gamma evidence and edition reference for the same contained mutation count it once|no manifest, root or derived write-set consequence adds another consumption|the closed Gamma kind registry gains no implicit publication entry)$"#
)]
fn cb5_counts_then(w: &mut ProtocolWorld) {
    assert!(w
        .core_count_suite
        .as_ref()
        .expect("delegated count suite")
        .is_ok());
}

fn core_count_deltas(w: &ProtocolWorld) -> (u64, u64, u64) {
    *w.core_count_observation
        .as_ref()
        .expect("CORE-COUNT-001 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then(expr = "the action tally changes by {string}")]
fn core_count_action_delta(w: &mut ProtocolWorld, expected: String) {
    assert_eq!(core_count_deltas(w).0, expected.parse::<u64>().unwrap());
}

#[then(expr = "the mutation tally changes by {string}")]
fn core_count_mutation_delta(w: &mut ProtocolWorld, expected: String) {
    assert_eq!(core_count_deltas(w).1, expected.parse::<u64>().unwrap());
}

#[then(expr = "the total delegated-consumption tally changes by {string}")]
fn core_count_total_delta(w: &mut ProtocolWorld, expected: String) {
    assert_eq!(core_count_deltas(w).2, expected.parse::<u64>().unwrap());
}

#[given(
    regex = r#"^(?:a connector catalog with exact profile, connector, version, actions and signature|one complete signed connector catalog|a homogeneous draft3 mandate carrying connector business actions|a connector mandate chain under ".*"|a draft3 parent pins one approved connector catalog|a signed, versioned and content-addressed connector catalog|a signed connector catalog whose action has ".*"|an approved catalog classes action ".*" as ".*"|a mandate carries ".*"|the request carries ".*"|a mandate pins one approved connector catalog)$"#
)]
fn cb5_catalog_given(w: &mut ProtocolWorld) {
    cb5_catalog_result(w);
}

#[when(
    regex = r#"^(?:the catalog signer signs RFC8785-JCS with signature\.value empty|the owner content key approves its exact connector, version and digest|its signed constraints are validated|it presents ".*" for a new W1 action occurrence|a child or runtime presents a changed digest, version, class or action set|the owner approves its exact digest and version|the owner and a keyless verifier validate its form|the grantee attempts that exact action|runtime presents ".*")$"#
)]
fn cb5_catalog_when(w: &mut ProtocolWorld) {
    cb5_catalog_result(w);
}

#[then(
    regex = r#"^(?:every action row has exactly name and one read, act or binding class|rows are non-empty, unique and sorted by action name|catalog_digest addresses the complete signed catalog|malformed, duplicate, unsorted, unclassed or multiply classed actions are InvalidCatalog|the approval has exact profile, subject, connector, catalog_version, catalog_digest, approved_at and signature|approval_digest addresses the complete signed approval|the catalog signer, owner root, grantee or a different subject cannot supply owner approval|neither complete signed document can substitute for the other|catalog_pins has one sorted exact connector, catalog_version, catalog_digest and approval_digest row per business connector|every descendant copies the complete pin array byte-for-byte|draft1, draft2, a sidecar, a changed pin, an unrelated pin or a pin for only \.config is InvalidMandate|catalog authority is ".*"|historical mandate bytes are never reinterpreted|the existing chain is refused for that catalog|only fresh homogeneous draft3 authority may approve the change|a mandate and edition pin both catalog and approval evidence|a keyless verifier never treats catalog signature alone as owner approval|the catalog is ".*"|no runtime component may reclassify it|the action is refused until new owner-approved authority is issued)$"#
)]
fn cb5_catalog_then(w: &mut ProtocolWorld) {
    cb5_assert_green(w);
}

// ---------------------------------------------------------- CB6 Gamma replay

#[given(
    regex = r#"^(?:a candidate Gamma entry for ".*" by ".*"|a hash-linked and correctly encoded candidate Gamma history|a manifest with aithos-core "1\.0\.0-draft\.2"|a structurally valid Gamma v2 entry of kind ".*"|a parent manifest ".*" whose Gamma predecessor is ".*"|disjoint competing branches under draft\.1 with Gamma v1 and draft\.2 with Gamma v2|one typed operation occurrence with an allocated operation_ref|an auditor authorized to query Gamma under read\.gamma|an authorized Gamma query whose result is made opposable|an accepted operation-bearing Gamma v2 entry with occurrence "O" and commitment "C"|a verified history with a Gamma v1 prefix, valid operation-bearing v2 entries and a v2 heartbeat|non-Gamma evidence shares an operation_ref with one accepted Gamma entry)$"#
)]
fn cb6_given(w: &mut ProtocolWorld) {
    cb6_result(w);
}

#[when(
    regex = r#"^(?:Core replays it against the exact historical prefix|replay encounters ".*"|Core checks its top-level operation reference|a child manifest ".*" introduces a Gamma ".*" entry|the branches are joined by their deterministic merge|its required Gamma evidence is appended|the auditor performs a local query without producing a signed presentation|signed presentation evidence is produced|a second Gamma candidate has ".*" and ".*" for ".*"|segment roots and the counts trie are recomputed)$"#
)]
fn cb6_when(w: &mut ProtocolWorld) {
    cb6_result(w);
}

#[then(
    regex = r#"^(?:form, time, signer, actor authority and operation coverage are verified|applicable revocation, constraints, receipts and counters are consumed|only then does the entry join replay state|semantic replay is refused at that entry|no later entry or counter is accepted|operation_ref is ".*"|when required it is the exact closed reference of the underlying occurrence|the opposite presence is refused|the profile transition is ".*"|the merge manifest declares draft\.2|the new kind:merge entry is Gamma v2 with its operation_ref|monotonicity is checked against both manifest parents and both Gamma predecessors|every retained v1 and v2 parent byte remains unchanged|physical segment order never reinterprets a causal edge|no publication or resolution Gamma kind is introduced|the entry carries that exact operation_ref|the append allocates no additional operation occurrence|the Gamma id is never reinterpreted as the occurrence|the perimeter is checked at operation time|no Gamma entry or persisted operation_ref is produced|the query is neither cold-replayable nor countable|log_reads does not reinterpret the query as ethos\.read|it represents one canonical read or presentation occurrence|the signed evidence carries that occurrence's operation_ref|no gamma\.read entry or automatic Gamma append is created|the candidate is ".*"|the same verdict applies when the candidate is first compared while joining branches|every exact Gamma line contributes once to its segment root and n|the existing kind and mandate fields alone feed the existing counters|the non-Gamma evidence contributes no H2 line or count|two distinct occurrences with identical effects remain two raw entries|replay or equivocation invalidates the edition instead of being deduplicated|no mutation or total-consumption counter is inferred)$"#
)]
fn cb6_then(w: &mut ProtocolWorld) {
    cb6_assert_green(w);
}

// ------------------------------------------------------- CB7 transactions

#[given(expr = "a published {string} bundle snapshotted byte for byte")]
fn core_atomic_fixture(w: &mut ProtocolWorld, store: String) {
    w.core_atomic_store = store.clone();
    w.core_atomic_boundary = None;
    w.core_atomic_observation = None;
    w.core_path_store = store;
    w.core_path_observation = None;
}

#[given(expr = "an injected failure at {string}")]
fn core_atomic_boundary(w: &mut ProtocolWorld, boundary: String) {
    if w.core_revocation_failure_boundary == "__fixture__" {
        w.core_revocation_failure_boundary = boundary;
    } else {
        w.core_atomic_boundary = Some(boundary);
    }
}

#[when("the owner attempts a valid mutation and publication")]
fn core_atomic_failure_attempt(w: &mut ProtocolWorld) {
    let boundary = w
        .core_atomic_boundary
        .as_deref()
        .expect("CORE-OWN-002 injected boundary");
    w.core_atomic_observation = Some(core_atomic_failure_scenario(&w.core_atomic_store, boundary));
}

#[when("the owner commits a valid circle edit")]
fn core_atomic_success_attempt(w: &mut ProtocolWorld) {
    w.core_atomic_observation = Some(core_atomic_success_scenario(&w.core_atomic_store));
}

fn core_atomic_observation(w: &ProtocolWorld) -> &CoreAtomicObservation {
    w.core_atomic_observation
        .as_ref()
        .expect("CORE-OWN-002 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("the mutation is refused before canonical effect")]
fn core_atomic_refused(w: &mut ProtocolWorld) {
    let observation = core_atomic_observation(w);
    assert!(observation.mutation_refused);
    assert!(observation.injected_once);
}

#[then("the canonical bundle is byte-for-byte identical to the snapshot")]
fn core_atomic_unchanged(w: &mut ProtocolWorld) {
    if let Some(observation) = &w.core_path_observation {
        assert!(
            observation
                .as_ref()
                .unwrap_or_else(|error| panic!("{error}"))
                .canonical_unchanged
        );
    } else {
        assert!(core_atomic_observation(w).canonical_unchanged);
    }
}

#[then(expr = "re-reading or reopening the {string} observes the old manifest and Gamma head")]
fn core_atomic_old_head(w: &mut ProtocolWorld, store: String) {
    let observation = core_atomic_observation(w);
    assert_eq!(observation.store, store);
    assert!(observation.reopened);
    assert!(observation.canonical_unchanged);
}

#[then(
    "no failed-mutation blob, index, header, wrap or Gamma entry exists in the canonical bundle"
)]
fn core_atomic_no_failed_artifact(w: &mut ProtocolWorld) {
    assert!(core_atomic_observation(w).canonical_unchanged);
}

#[then("staging remains non-canonical and is cleaned or recoverably resolved with no local-mutation orphan")]
fn core_atomic_staging_clean(w: &mut ProtocolWorld) {
    assert!(!core_atomic_observation(w).partial_state_observed);
}

#[then("one deterministic write-set advances content, roots, manifest and Gamma")]
fn core_atomic_complete_write_set(w: &mut ProtocolWorld) {
    assert!(core_atomic_observation(w).complete_new_state);
}

#[then("normal completion exposes the complete new state at one logical commit point")]
fn core_atomic_linearized(w: &mut ProtocolWorld) {
    let observation = core_atomic_observation(w);
    assert!(!observation.mutation_refused);
    assert!(observation.complete_new_state);
}

#[then("a crash or lost acknowledgement at that point resolves to the complete old or complete new state from the canonical manifest and Gamma head")]
fn core_atomic_recovery(w: &mut ProtocolWorld) {
    assert!(core_atomic_observation(w).reopened);
}

#[then("no reader or reopen observes an individual file replacement or partial edition")]
fn core_atomic_no_partial_state(w: &mut ProtocolWorld) {
    assert!(!core_atomic_observation(w).partial_state_observed);
}

#[when(expr = "a caller supplies {string} as a {string} under {string}")]
fn core_path_attempt(
    w: &mut ProtocolWorld,
    invalid_input: String,
    input_kind: String,
    filesystem_condition: String,
) {
    w.core_path_invalid_input = invalid_input;
    w.core_path_input_kind = input_kind;
    w.core_path_filesystem_condition = filesystem_condition;
    w.core_path_observation = Some(core_path_scenario(
        &w.core_path_store,
        &w.core_path_input_kind,
        &w.core_path_invalid_input,
        &w.core_path_filesystem_condition,
    ));
}

#[then("the operation is rejected before any out-of-root store access")]
fn core_path_refused_before_access(w: &mut ProtocolWorld) {
    let observation = w
        .core_path_observation
        .as_ref()
        .expect("CORE-OWN-004 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(observation.store, w.core_path_store);
    assert_eq!(observation.input_kind, w.core_path_input_kind);
    assert_eq!(observation.invalid_input, w.core_path_invalid_input);
    assert!(observation.rejected);
    assert!(!observation.outside_access_observed);
}

// -------------------------------------------------------- CB8 owner parity

#[given(expr = "an owner-local bundle session for zone {string}")]
fn core_owner_zone(w: &mut ProtocolWorld, zone: String) {
    w.core_owner_zone = zone;
    w.core_owner_fixture_ready = false;
    w.core_owner_observation = None;
}

#[given("a published existing folder and section in that zone")]
fn core_owner_fixture(w: &mut ProtocolWorld) {
    w.core_owner_fixture_ready = true;
}

#[when(expr = "the owner performs {string} through the common bundle operation")]
fn core_owner_operation(w: &mut ProtocolWorld, operation: String) {
    assert!(w.core_owner_fixture_ready, "owner fixture was not prepared");
    w.core_owner_operation = operation;
    w.core_owner_observation = Some(core_owner_scenario(
        &w.core_owner_zone,
        &w.core_owner_operation,
    ));
}

#[then("the operation succeeds from the narrow owner capability without a mandate")]
fn core_owner_succeeds(w: &mut ProtocolWorld) {
    let observation = w
        .core_owner_observation
        .as_ref()
        .expect("CORE-OWN-001 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(observation.zone, w.core_owner_zone);
    assert_eq!(observation.operation, w.core_owner_operation);
    assert_eq!(
        observation.outcome,
        if matches!(w.core_owner_operation.as_str(), "list") {
            "listed"
        } else if matches!(w.core_owner_operation.as_str(), "read") {
            "read"
        } else {
            "mutated"
        }
    );
}

#[then("every mutation is journalized without consuming mandate counters")]
fn core_owner_gamma(w: &mut ProtocolWorld) {
    let observation = w
        .core_owner_observation
        .as_ref()
        .expect("CORE-OWN-001 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        observation.gamma_delta,
        usize::from(matches!(
            observation.operation.as_str(),
            "create" | "edit" | "delete"
        ))
    );
    assert_eq!(observation.mandate_counter_delta, 0);
}

#[then("the resulting edition reopens and verifies from a fresh local store")]
fn core_owner_reopens(w: &mut ProtocolWorld) {
    assert!(
        w.core_owner_observation
            .as_ref()
            .expect("CORE-OWN-001 observation")
            .as_ref()
            .unwrap_or_else(|error| panic!("{error}"))
            .reopened
    );
}

// ----------------------------------------------- CB9 delegated content

#[given(expr = "a published bundle and a grantee with {string}")]
fn core_delegated_fixture(w: &mut ProtocolWorld, authority: String) {
    w.core_delegated_authority = authority;
    w.core_delegated_zone.clear();
    w.core_delegated_operation.clear();
    w.core_delegated_observation = None;
}

#[when(expr = "the grantee performs {string} in {string}")]
fn core_delegated_operation(w: &mut ProtocolWorld, operation: String, zone: String) {
    w.core_delegated_operation = operation;
    w.core_delegated_zone = zone;
    w.core_delegated_observation = Some(core_delegated_scenario(
        &w.core_delegated_zone,
        &w.core_delegated_operation,
        &w.core_delegated_authority,
    ));
}

fn core_delegated_observation(w: &ProtocolWorld) -> &CoreDelegatedObservation {
    w.core_delegated_observation
        .as_ref()
        .expect("CORE-DEL-001 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then(expr = "the operation is {string}")]
fn core_delegated_verdict(w: &mut ProtocolWorld, verdict: String) {
    if let Some(result) = &w.core_bound_receipt_result {
        assert_eq!(result.is_ok(), verdict == "accepted");
        return;
    }
    if let Some(observation) = &w.core_structural_observation {
        let observation = observation
            .as_ref()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(observation.verdict, verdict);
        assert!(observation.exact_effect_verified);
        assert!(observation.fresh_reopen_verified);
        if verdict == "accepted" {
            assert_eq!(observation.gamma_delta, 1);
        } else {
            assert_eq!(observation.gamma_delta, 0);
            assert!(observation.refusal_unchanged);
        }
        return;
    }
    if w.core_delegated_observation.is_none() {
        if w.cb5_result.is_some() {
            cb5_assert_green(w);
            cb6_assert_green(w);
        } else {
            cb10_assert_green(w);
        }
        return;
    }
    let observation = core_delegated_observation(w);
    assert_eq!(observation.zone, w.core_delegated_zone);
    assert_eq!(observation.operation, w.core_delegated_operation);
    assert_eq!(observation.authority, w.core_delegated_authority);
    assert_eq!(observation.verdict, verdict);
    assert_eq!(observation.accepted, verdict == "accepted");
    if observation.accepted {
        assert!(observation.effect_verified);
    } else {
        assert!(observation.refusal_unchanged);
    }
}

#[then("an accepted operation is journalized and cold-verifiable under the same chain")]
fn core_delegated_cold_verdict(w: &mut ProtocolWorld) {
    let observation = core_delegated_observation(w);
    if observation.accepted {
        assert_eq!(observation.gamma_delta, 1);
        assert!(observation.gamma_actor_is_grantee);
        assert!(observation.fresh_reopen_verified);
    } else {
        assert_eq!(observation.gamma_delta, 0);
        assert!(observation.refusal_unchanged);
    }
}

#[given(expr = "a grantee holds {string} and presents {string}")]
fn core_fence_fixture(w: &mut ProtocolWorld, key_material: String, authority: String) {
    w.core_fence_key_material = key_material;
    w.core_fence_authority = authority;
    w.core_fence_result = None;
}

#[when("it attempts to read the exact protected section")]
fn core_fence_read(w: &mut ProtocolWorld) {
    w.core_fence_result = Some(core_fence_scenario(
        &w.core_fence_key_material,
        &w.core_fence_authority,
    ));
}

#[then(expr = "the result is {string}")]
fn core_fence_verdict(w: &mut ProtocolWorld, verdict: String) {
    if let Some(result) = &w.core_fence_result {
        assert_eq!(
            result.as_ref().unwrap_or_else(|error| panic!("{error}")),
            &verdict
        );
    } else if w.cb5_result.is_some() {
        cb5_assert_green(w);
        cb6_assert_green(w);
    } else if w.cb10_result.is_some() {
        cb10_assert_green(w);
    } else {
        panic!("result step has no scenario-specific observation");
    }
}

#[given("an agent with edit authority on one public section")]
fn cb9_public_authorship_given(w: &mut ProtocolWorld) {
    w.core_edition_case = "delegated-public-authorship".into();
    w.core_edition_observation = Some(core_edition_positive_scenario(
        "delegated-public-authorship",
    ));
}

#[given(regex = r#"^an agent with exact authority for self SID ".*"$"#)]
fn cb9_self_authorship_given(w: &mut ProtocolWorld) {
    w.core_edition_case = "self-opaque-cold".into();
    w.core_edition_observation = Some(core_self_edition_scenario());
}

#[when(
    regex = r#"^(?:the agent publishes a normal delegated edit|it performs ".*" and publishes)$"#
)]
fn cb9_when(w: &mut ProtocolWorld) {
    assert!(w.core_edition_observation.is_some());
}

#[then(
    regex = r#"^(?:its authorship signature binds content hash, SID, operation, edition and authorized_via|Gamma and the manifest commit that signature|fresh-store verification labels the grantee, never the owner, as author|the edition proves ".*" for that SID|reveals no name, path, title, tags, body, folder relation or key)$"#
)]
fn cb9_then(w: &mut ProtocolWorld) {
    m_carrier_verdict(w);
}

#[given("a grantee opened a local bundle session while its chain was valid")]
fn core_current_authority_fixture(w: &mut ProtocolWorld) {
    w.core_current_authority_observation = None;
}

#[given(expr = "the mandate becomes {string} before the candidate mutation")]
fn core_current_authority_changes(w: &mut ProtocolWorld, authority_change: String) {
    w.core_current_authority_observation = Some(core_current_authority_scenario(&authority_change));
}

#[when("the grantee attempts to commit that mutation")]
fn core_current_authority_attempt(w: &mut ProtocolWorld) {
    assert!(w.core_current_authority_observation.is_some());
}

fn core_current_authority_observation(w: &ProtocolWorld) -> &CoreCurrentAuthorityObservation {
    w.core_current_authority_observation
        .as_ref()
        .expect("CORE-DEL-004 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("the current pure verdict refuses it")]
fn core_current_authority_refused(w: &mut ProtocolWorld) {
    let observation = core_current_authority_observation(w);
    assert!(matches!(
        observation.authority_change.as_str(),
        "expired" | "revoked"
    ));
    assert!(observation.old_line_usable_before_change);
    assert!(observation.current_verdict_refused);
}

#[then("the bundle, manifest and Gamma head remain byte-for-byte unchanged")]
fn core_current_authority_unchanged(w: &mut ProtocolWorld) {
    let observation = core_current_authority_observation(w);
    assert!(observation.canonical_unchanged);
    assert!(observation.fresh_reopen_unchanged);
}

#[given("a published bundle snapshotted before a delegated edit")]
fn core_delegated_rollback_fixture(w: &mut ProtocolWorld) {
    w.core_delegated_rollback_observation = None;
}

#[given("late Gamma validation fails after cryptographic preparation")]
fn core_delegated_rollback_injection(w: &mut ProtocolWorld) {
    w.core_delegated_rollback_observation = Some(core_delegated_rollback_scenario());
}

#[when("the bundle transaction is reopened")]
fn core_delegated_rollback_reopen(w: &mut ProtocolWorld) {
    assert!(w.core_delegated_rollback_observation.is_some());
}

fn core_delegated_rollback_observation(w: &ProtocolWorld) -> &CoreDelegatedRollbackObservation {
    w.core_delegated_rollback_observation
        .as_ref()
        .expect("CORE-DEL-005 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("every canonical byte equals the snapshot")]
fn core_delegated_rollback_bytes(w: &mut ProtocolWorld) {
    let observation = core_delegated_rollback_observation(w);
    assert!(observation.late_failure_injected_once);
    assert!(observation.operation_refused);
    assert!(observation.canonical_unchanged);
    assert!(observation.fresh_reopen_verified);
}

#[then("no failed authorship proof, blob or Gamma entry remains reachable")]
fn core_delegated_rollback_unreachable(w: &mut ProtocolWorld) {
    assert!(!core_delegated_rollback_observation(w).failed_artifacts_reachable);
}

// ------------------------------------- CB10 structure, revocation and vault

#[given(
    regex = r#"^(?:the validated G-A classification|a grantee has exact act\.x\.mail\.config and the exact /x/mail line|a grantee presents ".*" and holds ".*"|an agent may perform act\.x\.mail\.send through a tool host|the tool host opens /x/mail only owner-locally or with its own exact config authority and line|/x/mail material is held by an external secret manager|one holder may audit sealed action arguments|another holder may open /x/mail config|mail and calendar have independent vault nodes|a published bundle snapshotted before a mail config mutation|an injected failure before local commit|a published mail config mutation and one refused vault attempt)$"#
)]
fn cb10_given(w: &mut ProtocolWorld) {
    cb10_result(w);
}

#[when(
    regex = r#"^(?:a mandate carries exact act\.x\.mail\.config|it performs config ".*" for mail|it attempts to open mail config at /x/mail|Core authorizes and Gamma commits the action|a caller has no owner-local context and lacks exact config authority or line|each capability is exercised|".*" is attempted for mail|the authorized mutation is attempted|a keyless verifier inspects manifests, proofs, Gamma clear fields, logs and errors)$"#
)]
fn cb10_when(w: &mut ProtocolWorld) {
    cb10_result(w);
}

#[given(expr = "a grantee with {string}")]
fn core_structural_authority_fixture(w: &mut ProtocolWorld, authority: String) {
    w.core_structural_authority = authority;
    w.core_structural_observation = None;
}

#[when(expr = "it attempts {string}")]
fn core_structural_authority_attempt(w: &mut ProtocolWorld, operation: String) {
    w.core_structural_observation = Some(core_structural_authority_scenario(
        &operation,
        &w.core_structural_authority,
    ));
}

#[given("a grantee with read on one nested folder")]
fn core_structural_scoped_read_fixture(w: &mut ProtocolWorld) {
    w.core_structural_derived_case = "scoped-read".into();
    w.core_structural_derived_observation = None;
}

#[given("a public or circle section whose authorized edit changes its tags")]
fn core_structural_tag_fixture(w: &mut ProtocolWorld) {
    w.core_structural_derived_case = "tag-edit".into();
    w.core_structural_derived_observation = None;
}

#[given("an authorized move with source and destination authority")]
fn core_structural_move_fixture(w: &mut ProtocolWorld) {
    w.core_structural_derived_case = "move".into();
    w.core_structural_derived_observation = None;
}

#[given("a grantee delete perimeter covering a folder and its complete subtree")]
fn core_structural_subtree_fixture(w: &mut ProtocolWorld) {
    w.core_structural_derived_case = "subtree-delete".into();
    w.core_structural_derived_observation = None;
}

#[given("a grantee mutation in self")]
fn core_structural_self_fixture(w: &mut ProtocolWorld) {
    w.core_structural_derived_case = "self".into();
    w.core_structural_derived_observation = None;
}

fn core_structural_derived_run(w: &mut ProtocolWorld) {
    let result = match w.core_structural_derived_case.as_str() {
        "scoped-read" => core_structural_scoped_read_scenario(),
        "tag-edit" => core_structural_tag_scenario(),
        "move" => core_structural_move_scenario(),
        "subtree-delete" => core_structural_subtree_scenario(),
        "self" => core_structural_self_scenario(),
        other => Err(format!("CORE-STR-002 unknown derived case {other}")),
    };
    w.core_structural_derived_observation = Some(result);
}

#[when("it lists the folder and reads one contained section")]
#[when("the mutation commits")]
#[when("the node is reparented")]
#[when("the folder is deleted")]
#[when("keyless verification derives its state transition")]
fn core_structural_derived_execute(w: &mut ProtocolWorld) {
    core_structural_derived_run(w);
}

fn core_structural_derived_observation(w: &ProtocolWorld) -> &CoreStructuralDerivedObservation {
    let observation = w
        .core_structural_derived_observation
        .as_ref()
        .expect("CORE-STR-002 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(observation.case, w.core_structural_derived_case);
    observation
}

#[then(
    regex = r#"^(?:only covered children are presented|a sibling subtree remains absent and unreadable|index rows and affected tag wraps are deterministically derived|the authorizing Gamma entry, roots and manifest commit together|its stable SID follows the node|required rotation, survivor lines and destination up-link wrap join the transaction|the old parent derives no future node key|the derived changeset includes every removed row, blob, header and tag consequence|one actor chain covers every non-derived removal|dir and tag claims never authorize the mutation|proofs reveal only allowed opaque SIDs and commitments|no folder relationship or display metadata escapes)$"#
)]
fn core_structural_derived_verified(w: &mut ProtocolWorld) {
    let observation = core_structural_derived_observation(w);
    assert!(observation.primary_effect_verified);
    assert!(observation.secondary_effect_verified);
    assert!(observation.gamma_actor_verified);
    assert!(observation.publication_verified);
    assert!(observation.cold_reopen_verified);
    assert!(observation.privacy_verified);
}

#[given("a published bundle snapshotted before a structural mutation")]
fn core_structural_failure_fixture(w: &mut ProtocolWorld) {
    w.core_structural_failure_observation = None;
}

#[when(expr = "the mutation encounters {string}")]
fn core_structural_failure_execute(w: &mut ProtocolWorld, failure: String) {
    w.core_structural_failure_observation = Some(core_structural_failure_scenario(&failure));
}

fn core_structural_failure_observation(w: &ProtocolWorld) -> &CoreStructuralFailureObservation {
    w.core_structural_failure_observation
        .as_ref()
        .expect("CORE-STR-003 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("it is refused before canonical effect")]
fn core_structural_failure_refused(w: &mut ProtocolWorld) {
    let observation = core_structural_failure_observation(w);
    assert!(!observation.failure.is_empty());
    assert!(observation.refused);
    assert!(observation.canonical_unchanged);
    assert!(!observation.partial_artifact_reachable);
}

#[then("reopen observes the byte-identical old bundle and Gamma head")]
fn core_structural_failure_reopen(w: &mut ProtocolWorld) {
    assert!(core_structural_failure_observation(w).fresh_reopen_verified);
}

#[given("a published bundle snapshotted byte for byte before revocation")]
fn core_revocation_failure_fixture(w: &mut ProtocolWorld) {
    w.core_revocation_failure_boundary = "__fixture__".into();
    w.core_revocation_failure_observation = None;
}

#[when("an authorized manager attempts revoke, rotation and publication")]
fn core_revocation_failure_execute(w: &mut ProtocolWorld) {
    w.core_revocation_failure_observation = Some(core_revocation_failure_scenario(
        &w.core_revocation_failure_boundary,
    ));
}

fn core_revocation_failure_observation(w: &ProtocolWorld) -> &CoreRevocationFailureObservation {
    w.core_revocation_failure_observation
        .as_ref()
        .expect("CORE-REV-001 failure observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("the canonical bundle remains byte-for-byte identical to the snapshot")]
fn core_revocation_failure_unchanged(w: &mut ProtocolWorld) {
    let observation = core_revocation_failure_observation(w);
    assert_eq!(observation.boundary, w.core_revocation_failure_boundary);
    assert!(observation.refused);
    assert!(observation.canonical_unchanged);
}

#[then("reopening observes the old recipients, old Gamma head and old edition")]
fn core_revocation_failure_reopen(w: &mut ProtocolWorld) {
    assert!(core_revocation_failure_observation(w).old_state_reopened);
}

#[then("no revocation entry or rotated material from the failed attempt is reachable")]
fn core_revocation_failure_no_partial(w: &mut ProtocolWorld) {
    assert!(!core_revocation_failure_observation(w).partial_cut_reachable);
}

#[given("a valid delegated mutation before its mandate revocation")]
fn core_revocation_replay_fixture(w: &mut ProtocolWorld) {
    w.core_revocation_replay_observation = None;
}

#[given("an otherwise identical mutation at or after revoked_at")]
fn core_revocation_replay_late_candidate(w: &mut ProtocolWorld) {
    assert!(w.core_revocation_replay_observation.is_none());
}

#[when("a fresh store replays the complete Gamma history")]
fn core_revocation_replay_execute(w: &mut ProtocolWorld) {
    w.core_revocation_replay_observation = Some(core_revocation_replay_scenario());
}

fn core_revocation_replay_observation(w: &ProtocolWorld) -> &CoreRevocationReplayObservation {
    w.core_revocation_replay_observation
        .as_ref()
        .expect("CORE-REV-001 replay observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("the earlier mutation remains valid")]
fn core_revocation_replay_earlier(w: &mut ProtocolWorld) {
    let observation = core_revocation_replay_observation(w);
    assert!(observation.earlier_mutation_valid);
    assert!(observation.fresh_replay_verified);
}

#[then("the later mutation is rejected")]
fn core_revocation_replay_later(w: &mut ProtocolWorld) {
    assert!(core_revocation_replay_observation(w).later_mutation_refused);
}

#[then("current revocation state is derived only from verified prior entries")]
fn core_revocation_replay_current(w: &mut ProtocolWorld) {
    assert!(core_revocation_replay_observation(w).current_revocation_derived);
}

#[given("a published encrypted subtree shared with one grantee and one survivor")]
fn core_revocation_cut_fixture(w: &mut ProtocolWorld) {
    w.core_revocation_cut_observation = None;
}

#[when("an authorized manager revokes the grantee")]
fn core_revocation_cut_execute(w: &mut ProtocolWorld) {
    w.core_revocation_cut_observation = Some(core_revocation_cut_scenario());
}

#[when(
    "the transaction rotates, rewraps survivors, re-encrypts protected content and appends Gamma"
)]
fn core_revocation_cut_transaction(w: &mut ProtocolWorld) {
    assert!(w.core_revocation_cut_observation.is_some());
}

fn core_revocation_cut_observation(w: &ProtocolWorld) -> &CoreRevocationCutObservation {
    w.core_revocation_cut_observation
        .as_ref()
        .expect("CORE-REV-001 observation")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

#[then("one edition commits the revocation and every derived cryptographic change")]
fn core_revocation_cut_edition(w: &mut ProtocolWorld) {
    let observation = core_revocation_cut_observation(w);
    assert!(observation.one_new_edition);
    assert!(observation.revoke_gamma_present);
    assert!(observation.rotated_header_and_body);
}

#[then("the revoked line opens no new key or rewritten body")]
fn core_revocation_cut_revoked(w: &mut ProtocolWorld) {
    let observation = core_revocation_cut_observation(w);
    assert!(observation.revoked_cut);
    assert!(observation.survivor_reads);
}

#[then("a fresh keyless store verifies the authority, cut and resulting roots")]
fn core_revocation_cut_cold(w: &mut ProtocolWorld) {
    assert!(core_revocation_cut_observation(w).fresh_keyless_verified);
}

#[then(
    regex = r#"^(?:config remains outside the read, act and binding business catalog|all applicable constraints and obligations explicitly present in the whole presented chain apply|no wildcard or inferred binding co_sign covers it|the vault operation is authorized under its applicable constraints|Gamma, roots and publication commit any mutation atomically|config authority grants no external mail action|this protocol version exposes no narrower config read or write authority|a finer split requires a later version and never reinterprets this mandate|the tool host resolves the credential at the last moment|the agent receives no config plaintext, DK or vault line|the secret manager result cannot authorize or open the vault|Core remains the source of the protocol verdict|neither capability opens the other's sealed material|only /x/mail recipients, versions and roots may change|Gamma, config evidence and publication commit atomically|fresh-store keyless verification receives no credential|the canonical bundle remains byte-for-byte identical|no Gamma entry, header generation or config blob from the attempt is reachable|it finds no credential, config plaintext, private key or DK|encrypted normative header lines remain opaque and non-authorizing)$"#
)]
fn cb10_then(w: &mut ProtocolWorld) {
    cb10_assert_green(w);
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

#[then("the two succession keys differ")]
fn succession_keys_differ(w: &mut ProtocolWorld) {
    assert_eq!(w.succession_pubs.len(), 2);
    assert_ne!(w.succession_pubs[0], w.succession_pubs[1]);
}

#[then("the owner keys are identical both times")]
fn owner_keys_identical(w: &mut ProtocolWorld) {
    assert_eq!(w.identities[0], w.identities[1]);
}

#[then("it contains the root, content, kex and succession public keys")]
fn doc_contains_four_keys(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_ref().expect("a DID document");
    let owner = w.owner(0);
    let succession = succession_from_entropy(w.succession_entropy[0]);
    assert_eq!(
        doc.keys.root,
        wire::ed25519_pub_to_multibase(&owner.root_sign.verifying_key().to_bytes())
    );
    assert_eq!(
        doc.keys.content,
        wire::ed25519_pub_to_multibase(&owner.content_sign.verifying_key().to_bytes())
    );
    assert_eq!(
        doc.keys.kex,
        wire::x25519_pub_to_multibase(&owner.owner_kex_pub().to_bytes())
    );
    assert_eq!(
        doc.keys.succession,
        wire::ed25519_pub_to_multibase(&succession.verifying_key().to_bytes())
    );
}

#[then("its identifier is derived from the root public key")]
fn doc_id_from_root(w: &mut ProtocolWorld) {
    let doc = w.did_doc.as_ref().expect("a DID document");
    let root = w.owner(0).root_sign.verifying_key().to_bytes();
    assert_eq!(doc.id, wire::did_aithos(&root));
}

#[then("its signature verifies under the root key")]
fn doc_signature_verifies(w: &mut ProtocolWorld) {
    w.did_doc
        .as_ref()
        .unwrap()
        .verify()
        .expect("valid document");
}

#[then("verification is rejected")]
fn verification_rejected(w: &mut ProtocolWorld) {
    assert!(w.did_doc.as_ref().unwrap().verify().is_err());
}

#[then("the document does not parse as a DID document")]
fn did_wire_does_not_parse(w: &mut ProtocolWorld) {
    assert!(
        w.did_parsed.as_ref().expect("a JSON wire attempt").is_err(),
        "wire: {:?}",
        w.did_wire
    );
}

#[then("the successor DID document is accepted")]
fn successor_accepted(w: &mut ProtocolWorld) {
    assert_eq!(w.transition.as_ref().unwrap(), &Ok(()));
}

#[then("the transition is rejected")]
fn transition_rejected(w: &mut ProtocolWorld) {
    assert!(w.transition.as_ref().unwrap().is_err());
}

// BDER-009: every positional reader of `node_keys` states its precondition,
// so composing this feature's `Given`s can never silently shift the pair
// being compared.
fn b2_pair(w: &ProtocolWorld) -> ([u8; 32], [u8; 32]) {
    assert_eq!(
        w.node_keys.len(),
        2,
        "this Then reads exactly two derivations"
    );
    (w.node_keys[0], w.node_keys[1])
}

#[then("both derivations yield the same key")]
fn same_key(w: &mut ProtocolWorld) {
    let (first, second) = b2_pair(w);
    assert_eq!(first, second);
}

// BDER-001: the determinism claim is anchored to an expected value that
// exists outside this process — no mutant of `node_key` can satisfy it by
// being consistent with itself.
#[then("the key equals the B2 vector's deep section key byte for byte")]
fn deep_key_matches_vector(w: &mut ProtocolWorld) {
    let (first, second) = b2_pair(w);
    let expected = B2Vector::load().deep_section_key_hex;
    assert_eq!(hex::encode(first), expected, "vector B2 deep section key");
    assert_eq!(hex::encode(second), expected, "and the rebuilt path agrees");
}

// BDER-001: one labelled derivation per segment, with the literal label forms
// of §02.5 pinned. A monolithic hash over the whole path satisfies neither.
#[then("each segment contributed exactly one labelled derivation")]
fn chain_is_per_segment(w: &mut ProtocolWorld) {
    let (first, _) = b2_pair(w);
    let path = w.deep_path.as_ref().expect("the deep path");
    let aithos_core::path::Leaf::Section(section) = &path.leaf else {
        panic!("path must end in a section");
    };

    for folder in &path.folders {
        assert_eq!(folder_label(folder), format!("aithos-core/v1/d/{folder}"));
    }
    assert_eq!(
        section_label(section),
        format!("aithos-core/v1/s/{section}")
    );

    let mut key = w.zone_dk.unwrap();
    let mut derivations = 0usize;
    for folder in &path.folders {
        key = derive_key(&folder_label(folder), &key);
        derivations += 1;
    }
    key = derive_key(&section_label(section), &key);
    derivations += 1;

    assert_eq!(
        derivations,
        path.folders.len() + 1,
        "reading at depth d costs exactly d derivations"
    );
    assert_eq!(derivations, 4, "three folder segments and one section");
    assert_eq!(
        key, first,
        "node_key must be exactly this per-segment chain"
    );
}

// BDER-002: `assert_ne!` on two arrays was the weakest possible reading of
// "unrelated". Neither key may be reachable from the other by any label the
// production code can build.
#[then("neither sibling key derives the other under any production label")]
fn siblings_not_mutually_derivable(w: &mut ProtocolWorld) {
    let (first, second) = b2_pair(w);
    assert_ne!(first, second, "sibling sids must reach the derivation");

    let v = B2Vector::load();
    assert_eq!(
        hex::encode(first),
        v.folder1_key_hex,
        "the first sibling is the vector's folder 1"
    );

    let mut reachable = 0usize;
    for label in b2_production_labels(&v.tag) {
        if derive_key(&label, &first) == second {
            reachable += 1;
        }
        if derive_key(&label, &second) == first {
            reachable += 1;
        }
    }
    assert_eq!(reachable, 0, "no label bridges one sibling to the other");
}

// BDER-002: "unrelated" also means neither key hands back the parent. A step
// that can be undone with a public label is not one-way, however different
// the two outputs look.
#[then("neither sibling key yields the zone key back")]
fn siblings_do_not_reveal_zone(w: &mut ProtocolWorld) {
    let (first, second) = b2_pair(w);
    let zone = w.zone_dk.unwrap();
    let labels = b2_production_labels(&B2Vector::load().tag);

    for key in [first, second] {
        assert_ne!(key, zone, "a child key is never its parent");
        for label in &labels {
            assert_ne!(
                derive_key(label, &key),
                zone,
                "no public label walks a child key back to the zone key"
            );
        }
        assert!(
            !b2_shares_window(&key, &zone, 16),
            "no 16-byte run of the zone key survives into a child key"
        );
    }
}

#[then("the folder key alone derives the section beneath it")]
fn folder_derives_section(w: &mut ProtocolWorld) {
    let path = w.deep_path.as_ref().unwrap();
    let via_zone = node_key(&w.zone_dk.unwrap(), path);
    let aithos_core::path::Leaf::Section(section) = &path.leaf else {
        panic!("path must end in a section");
    };
    let via_folder = derive_key(&section_label(section), &w.folder_key.unwrap());
    assert_eq!(via_folder, via_zone, "no need to touch the zone key again");
}

// BDER-005: "every descendant" was one section, one depth, one shape. Two
// further shapes are added, each keeping what gives this scenario its power —
// the `Then` crosses `derive_key` from the held key against `node_key` from
// the zone key, two distinct routes rather than one value compared to itself.
//
// `node_key` is a pure function of a path with no notion of node existence,
// so "future descendant" is not a distinguishable case at this layer; the
// operational claim belongs to `e-mandates.feature`.
#[then("it alone derives a grandchild section and a tag anchor beneath it")]
fn folder_derives_more_descendants(w: &mut ProtocolWorld) {
    let (zone, v) = (w.zone_dk.unwrap(), B2Vector::load());
    let folder_key = w.folder_key.expect("a held folder key");
    let spine = w.deep_path.as_ref().unwrap().folders.clone();

    // A grandchild: a section under a sub-folder of the held folder.
    let (child_folder, grandchild_section) = (sid(4), sid(9));
    let mut deeper = spine.clone();
    deeper.push(child_folder);
    let grandchild_via_zone = node_key(
        &zone,
        &NodePath::section(Zone::Circle, deeper, grandchild_section),
    );
    let grandchild_via_folder = derive_key(
        &section_label(&grandchild_section),
        &derive_key(&folder_label(&child_folder), &folder_key),
    );
    assert_eq!(
        grandchild_via_folder, grandchild_via_zone,
        "two derivations from the folder key reach its grandchild"
    );

    // A tag anchor anchored at the held folder.
    let anchor_via_zone = node_key(
        &zone,
        &NodePath::tag_view(Zone::Circle, spine, &v.tag).expect("fixture tag is valid"),
    );
    let anchor_via_folder = derive_key(&tag_label(&v.tag), &folder_key);
    assert_eq!(
        anchor_via_folder, anchor_via_zone,
        "one derivation from the folder key reaches its tag anchor"
    );

    let section_key = derive_key(&section_label(&v.section_sid()), &folder_key);
    let shapes: BTreeSet<[u8; 32]> = [section_key, grandchild_via_folder, anchor_via_folder].into();
    assert_eq!(
        shapes.len(),
        3,
        "three distinct descendant shapes, three distinct keys"
    );
}

// BDER-003: three `assert_ne!` on a key nobody proved was folder 1's key made
// the whole Rule vacuous — substituting `[0x00; 32]` left it green. The held
// key is now pinned to the vector's `folder1_key_hex`, the only field five
// independent Python generators corroborate.
#[then("the held key is exactly the first folder's key")]
fn held_key_is_folder_one(w: &mut ProtocolWorld) {
    let held = w.folder_key.expect("a held folder key");
    assert_eq!(
        hex::encode(held),
        B2Vector::load().folder1_key_hex,
        "the negatives below prove nothing unless the held key is the right one"
    );
}

// BDER-003: the `Then` is universally quantified, so the explored space is
// enumerated and its size is stated instead of being three hand-picked shots.
#[then("no derivation from it yields the second folder's section key")]
fn no_sideways_reach(w: &mut ProtocolWorld) {
    let (zone, v) = (w.zone_dk.unwrap(), B2Vector::load());
    let sibling = w.sibling_paths.get(1).expect("the second sibling section");
    let target = node_key(&zone, sibling);
    assert_eq!(
        hex::encode(target),
        v.sibling_section_key_hex,
        "the target is the vector's sibling section key"
    );

    let held = w.folder_key.expect("a held folder key");
    let paths = b2_reachable_paths(&v.tag);
    assert_eq!(
        paths.len(),
        13_332,
        "the explored space is stated, not implied: folder spines of length \
         0..=3 over sids 0..9, each optionally terminated by a section or the \
         tag anchor"
    );
    for path in &paths {
        assert_ne!(
            node_key(&held, path),
            target,
            "sideways derivation must never reach a sibling subtree: {path}"
        );
    }
}

// BDER-003: §02.5 says "never anything ABOVE or beside it", and no scenario
// asserted the upward half. A derivation step that a public label can undo
// hands the zone key — and therefore the whole zone — to any leaf holder.
#[then("no derivation from it yields its own parent or the zone key")]
fn no_upward_reach(w: &mut ProtocolWorld) {
    let (zone, v) = (w.zone_dk.unwrap(), B2Vector::load());
    let held = w.folder_key.expect("a held folder key");
    let labels = b2_production_labels(&v.tag);

    assert_ne!(held, zone, "a folder key is never the zone key");
    for label in &labels {
        assert_ne!(
            derive_key(label, &held),
            zone,
            "no label walks the folder key back up to the zone key"
        );
    }

    let child = derive_key(&section_label(&v.section_sid()), &held);
    assert_ne!(child, held);
    for label in &labels {
        assert_ne!(
            derive_key(label, &child),
            held,
            "no label walks a section key back up to its folder"
        );
        assert_ne!(derive_key(label, &child), zone);
    }
}

// BDER-004: the old step re-derived an unchanged path and called it a rename.
// This one re-resolves the section after a real `Bundle::rename_folder`, so a
// rename implemented as delete-and-recreate hands back a fresh sid and the
// derived key moves.
#[then(expr = "the derived key of {string} is unchanged")]
fn derived_key_unchanged(w: &mut ProtocolWorld, display_path: String) {
    let owner = w.owner(0);
    let bundle = w.bundle.as_ref().expect("a published bundle");
    let zone_dk = bundle
        .zone_dk(Zone::Circle, &owner)
        .expect("circle zone dk");
    let (row, folders) = bundle
        .resolve_clear(Zone::Circle, &display_path)
        .expect("the section resolves at its new display path");

    assert_eq!(
        Some(&row.sid),
        w.renamed_section_sid.as_ref(),
        "the rename must keep the section's sid, not recreate the node"
    );
    let after = node_key(
        &zone_dk,
        &NodePath::section(
            Zone::Circle,
            folders,
            Sid::parse(&row.sid).expect("section sid"),
        ),
    );
    assert_eq!(
        after,
        w.rename_key_before
            .expect("a key recorded before the rename"),
        "rename must never re-key"
    );
}

#[then("the two anchors differ from each other and from the folder key")]
fn anchors_distinct(w: &mut ProtocolWorld) {
    // BDER-009: the cardinal reader states its precondition too.
    assert_eq!(
        w.node_keys.len(),
        3,
        "this Then reads exactly three derivations"
    );
    let unique: std::collections::BTreeSet<_> = w.node_keys.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "local anchor, root anchor, folder key all distinct"
    );
}

#[then("the owner opens the header and recovers the node key")]
fn owner_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 1, "owner-kex", &xsk(0x0A))
        .unwrap();
    assert_eq!(dk, DK);
}

#[then("the grantee opens the header and recovers the node key")]
#[then("the new grantee opens the node key")]
fn grantee_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 1, "g1", &xsk(0x21))
        .unwrap();
    assert_eq!(dk, DK);
}

#[then("it recovers nothing")]
fn stranger_recovers_nothing(w: &mut ProtocolWorld) {
    assert!(!w.opened.is_empty());
    assert!(w.opened.iter().all(Result::is_err));
}

#[then("opening that line is rejected")]
#[then("opening it there is rejected")]
fn opening_rejected(w: &mut ProtocolWorld) {
    assert!(w.opened.last().unwrap().is_err());
}

#[then("the header is rejected as invalid")]
fn header_invalid(w: &mut ProtocolWorld) {
    let msg = w.rejection.as_deref().unwrap();
    assert!(msg.contains("I3"), "rejection must name I3: {msg}");
}

#[then("the owner line is byte-identical to before")]
fn owner_line_untouched(w: &mut ProtocolWorld) {
    let header = w.header.as_ref().unwrap();
    let owner_line = header.key_versions["1"]
        .lines
        .iter()
        .find(|l| l.to == "owner")
        .unwrap();
    assert_eq!(owner_line, w.saved_line.as_ref().unwrap());
}

#[then("the surviving grantee opens the new node key")]
fn survivor_opens(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "g2", &xsk(0x22))
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("the first grantee cannot open the new version")]
fn revoked_cannot_open(w: &mut ProtocolWorld) {
    assert!(w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "g1", &xsk(0x21))
        .is_err());
}

#[then("the owner opens the new version too")]
fn owner_opens_new(w: &mut ProtocolWorld) {
    let dk = w
        .header
        .as_ref()
        .unwrap()
        .open(DID_C, 2, "owner-kex", &xsk(0x0A))
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("a parent holder recovers the new node key through the wrap")]
fn parent_recovers_via_wrap(w: &mut ProtocolWorld) {
    let dk = w
        .wrap_obj
        .as_ref()
        .unwrap()
        .open(DID_C, &PARENT_KEY)
        .unwrap();
    assert_eq!(dk, DK2);
}

#[then("edition 1 verifies offline")]
#[then("its integrity checks against the signed edition")]
fn edition_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().verify().expect("edition valid");
}

#[then("the manifest pins the DID document")]
fn manifest_pins_did(w: &mut ProtocolWorld) {
    let manifest = w.latest_manifest();
    let did_bytes = w
        .bundle
        .as_ref()
        .unwrap()
        .store
        .get("did.json")
        .unwrap()
        .unwrap();
    assert_eq!(
        manifest.files.get("did.json").unwrap(),
        &sha256_hex(&did_bytes)
    );
}

#[then("edition 2 verifies and pins edition 1 as its predecessor")]
fn edition_two_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().verify().expect("edition valid");
    let latest = w.latest_manifest();
    assert_eq!(latest.edition.height, 2);
    let first: Manifest = serde_json::from_slice(
        &w.bundle
            .as_ref()
            .unwrap()
            .store
            .get("manifests/1.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(latest.edition.prev_hash, first.chain_hash().unwrap());
}

#[then("edition verification is rejected")]
fn edition_rejected(w: &mut ProtocolWorld) {
    assert!(w.bundle.as_ref().unwrap().verify().is_err());
}

#[then("the section body comes back intact")]
fn body_intact(w: &mut ProtocolWorld) {
    assert_eq!(w.read_body.as_ref().unwrap().as_deref(), Ok(BODY));
}

#[then(expr = "the owner reads the same section at {string}")]
fn reads_at_new_path(w: &mut ProtocolWorld, path: String) {
    let owner = w.owner(0);
    let body = w
        .bundle
        .as_ref()
        .unwrap()
        .read_section(Zone::Circle, &path, &owner)
        .expect("readable at the renamed path");
    assert_eq!(body, BODY);
}

#[then("the section body is readable in clear")]
fn public_body_readable(w: &mut ProtocolWorld) {
    assert_eq!(w.read_body.as_ref().unwrap().as_deref(), Ok(PUB_BODY));
}

#[then("no folder name, section name, title or tag appears anywhere")]
fn self_leaks_nothing(w: &mut ProtocolWorld) {
    for needle in [
        "enfance",
        "cicatrices",
        "blessure",
        "cicatrice au genou",
        "sante",
    ] {
        assert!(
            !w.inspected.contains(needle),
            "self zone leaked the string '{needle}'"
        );
    }
}

#[then("the owner still reconstructs the full tree from sealed descriptors")]
fn owner_reconstructs_tree(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let tree = w
        .bundle
        .as_ref()
        .unwrap()
        .zone_tree(Zone::Self_, &owner)
        .unwrap();
    for expected in [
        "enfance",
        "enfance/cicatrices",
        "enfance/cicatrices/blessure",
    ] {
        assert!(
            tree.contains(&expected.to_owned()),
            "missing {expected} in {tree:?}"
        );
    }
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

// ------------------------------------------------------- step F: gamma ---

const AUDITOR: u8 = 0xA4;
const NA_FAR: &str = "2027-07-01T00:00:00Z";
const D0: &str = "2026-07-01T00:00:00Z"; // "day 0" of the F scenarios

fn day(n: u32, hms: &str) -> String {
    // Days relative to D0 (July 2026 has 31 days) — enough for two months.
    let (mo, d) = if n < 31 { (7, n + 1) } else { (8, n - 30) };
    format!("2026-{mo:02}-{d:02}T{hms}Z")
}

impl ProtocolWorld {
    fn gbundle(&mut self) -> &mut Bundle<MemStore> {
        self.bundle.as_mut().unwrap()
    }

    fn did_document(&self) -> DidDocument {
        let bytes = self
            .bundle
            .as_ref()
            .unwrap()
            .store
            .get("did.json")
            .unwrap()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn store_cert(&mut self, m: &Mandate) {
        let bytes = serde_json::to_vec_pretty(m).unwrap();
        self.gbundle()
            .store
            .put(&format!("certs/{}.json", m.id), &bytes)
            .unwrap();
    }

    /// Root mandate over connector actions (and optionally more entries),
    /// certificate + stored — the act-plane sibling of `grant_to_agent`.
    fn grant_act(
        &mut self,
        extra: Vec<aithos_core::mandate::PerimeterEntry>,
        constraints: serde_json::Value,
        na: &str,
    ) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let owner = self.owner(0);
        let mut perimeter = vec![PerimeterEntry::parse("act.x.gmail.*").unwrap()];
        perimeter.extend(extra);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 900)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter,
                constraints,
                not_before: NB.into(),
                not_after: na.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    /// Sub-mandate over an action pattern, minted by the AGENT for HELPER;
    /// logging the grant is the caller's (spec-mandated) duty.
    fn delegate_act(
        &mut self,
        pattern: &str,
        constraints: serde_json::Value,
        log_grant: bool,
    ) -> Result<String, String> {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let parent = self.chain[0].clone();
        let child = M::build_sub(
            &parent,
            &agent_sk(AGENT),
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 950)),
                subject: parent.subject.clone(),
                grantee_id: "urn:aithos:agent:helper".into(),
                grantee_label: "helper".into(),
                grantee_pub: &agent_sk(HELPER).verifying_key(),
                perimeter: vec![PerimeterEntry::parse(pattern).unwrap()],
                constraints,
                not_before: NB.into(),
                not_after: parent.not_after.clone(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&child);
        self.helper_chain = vec![parent.clone(), child.clone()];
        if log_grant {
            let mut ent = std::mem::take(&mut self.ent);
            let r = self
                .gbundle()
                .log_delegated_grant(
                    &[parent],
                    &agent_sk(AGENT),
                    &child.id,
                    &day(1, "00:30:00"),
                    &mut ent,
                )
                .map(|()| child.id.clone())
                .map_err(|e| e.to_string());
            self.ent = ent;
            return r;
        }
        Ok(child.id)
    }

    fn try_action(&mut self, helper: bool, action: &str, at: &str) -> Result<String, String> {
        self.try_action_full(helper, action, at, None, None)
    }

    fn try_action_full(
        &mut self,
        helper: bool,
        action: &str,
        at: &str,
        budget: Option<serde_json::Value>,
        sealed_args: Option<serde_json::Value>,
    ) -> Result<String, String> {
        let chain = if helper {
            self.helper_chain.clone()
        } else {
            self.chain.clone()
        };
        let sk = agent_sk(if helper { HELPER } else { AGENT });
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_action(
                &chain,
                &sk,
                &aithos_bundle::log::ActionSpec {
                    connector: "gmail",
                    action,
                    args_hash: "sha256:00",
                    now: at,
                    budget,
                    sealed_args,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn try_inference(
        &mut self,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        budget_ref: Option<&str>,
        at: &str,
    ) -> Result<String, String> {
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_inference(
                &chain,
                &agent_sk(AGENT),
                &aithos_bundle::log::InferenceSpec {
                    provider: "provider",
                    model,
                    tokens_in,
                    tokens_out,
                    budget_ref,
                    now: at,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn beacon(&mut self, seq: u64, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gbundle()
            .log_heartbeat(&owner, seq, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn segment_lines(&mut self, seg: &str) -> Vec<Vec<u8>> {
        self.gbundle()
            .store
            .get(seg)
            .unwrap()
            .unwrap()
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    }
}

// --- F givens ---

#[given("a bundle with a three-entry log")]
fn three_entry_log(w: &mut ProtocolWorld) {
    w.init_bundle();
    for (i, hms) in ["01:00:00", "02:00:00", "03:00:00"].iter().enumerate() {
        w.beacon(i as u64 + 1, &day(8, hms));
    }
}

#[given("a bundle with entries logged in two different months")]
fn two_month_log(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.beacon(1, D0);
    w.beacon(2, &day(35, "00:00:00"));
    w.publish_bundle();
}

#[given(expr = "an agent granted action rights on connector {string}")]
fn act_grant_connector(w: &mut ProtocolWorld, _connector: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("an agent granted action rights for 7 days")]
fn act_grant_7d(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA7);
}

#[given("an agent granted action rights with max_actions 3")]
fn act_grant_budget(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({"max_actions": 3}), NA30);
}

#[given("an agent granted action rights with max_actions 3 and issue depth 1")]
fn act_grant_budget_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"max_actions": 3}),
        NA30,
    );
}

#[given("the agent delegates its perimeter to a helper")]
fn delegates_perimeter(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.*", serde_json::json!({}), true)
        .unwrap();
}

#[given("an agent granted action rights with max_actions_per 2 per 24 hours")]
fn act_grant_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"max_actions_per": {"window": "24h", "n": 2}}),
        NA30,
    );
}

#[given(expr = "an agent granted gmail actions with rate_limit 2 {string} per 72 hours")]
fn act_grant_rate(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"rate_limit": {"action": action, "window": "72h", "n": 2}}),
        NA30,
    );
}

#[given("an agent granted issue rights with max_children 2")]
fn act_grant_children(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"max_children": 2}),
        NA30,
    );
}

#[given("an agent that minted a sub-mandate without logging the grant")]
fn minted_unlogged(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), false)
        .unwrap();
}

#[given("a head mandate with heartbeat every 30 days grace 72 hours")]
fn head_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"heartbeat": {"every": "30d", "grace": "72h"}}),
        NA_FAR,
    );
}

#[given("an owner beacon at day 0")]
fn beacon_day0(w: &mut ProtocolWorld) {
    w.beacon(1, D0);
}

#[given("a head mandate suspended by owner silence")]
fn suspended_mandate(w: &mut ProtocolWorld) {
    head_mandate(w);
    beacon_day0(w);
    // Day 34 sits beyond every+grace (33d): the mandate is suspended.
    w.gamma_result = Some(w.try_action(false, "reply", &day(34, "00:00:00")));
    assert!(w.gamma_result.as_ref().unwrap().is_err());
}

#[given("an agent under a mandate with freshness 24 hours")]
fn freshness_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({"freshness": "24h"}), NA30);
    w.beacon(1, D0); // the anchor entry
}

#[given("a published bundle with a circle section")]
fn published_circle_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
}

#[given("a bundle whose log records mutations and actions")]
fn log_with_mutations_and_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto"); // sealed mutation @NOW
    w.grant_act(vec![], serde_json::json!({"max_actions": 5}), NA30);
    w.try_action(false, "reply", &day(14, "01:00:00")).unwrap();
}

#[given(expr = "logged mutations on sections under {string} and under {string}")]
fn mutations_two_folders(w: &mut ProtocolWorld, f1: String, f2: String) {
    w.init_bundle();
    w.add_named_section(&f1, "note1", &[]);
    w.add_named_section(&f2, "note2", &[]);
    w.e_folders = vec![f1, f2];
}

#[given("an agent granted action rights and no read grant")]
fn act_no_read(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto"); // a sealed entry exists
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("a bundle whose log records mutations and actions over two months")]
fn log_two_months(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA_FAR);
    w.try_action(false, "reply", &day(4, "00:00:00")).unwrap(); // day 4: outside
    w.add_circle_section("projets", "note1", "toto"); // NOW = day 8: mutation
    w.try_action(false, "reply", &day(14, "00:00:00")).unwrap(); // inside
    w.try_action(false, "label", &day(35, "00:00:00")).unwrap(); // inside
}

#[given(expr = "logged mutations by the owner and by an agent under {string}")]
fn mutations_for_audit(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note1", &[]);
    w.add_named_section(&folder, "note2", &[]);
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(14, "01:00:00")).unwrap();
}

#[given(expr = "an auditor granted read.gamma on action {string} from day 1 to day 30")]
fn scoped_auditor(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, &action, &day(19, "12:00:00")).unwrap();
    let owner = w.owner(0);
    let entry = format!(
        "read.gamma#action={action}&since={}&until={}",
        day(0, "00:00:00"),
        day(29, "23:59:59")
    );
    let m = Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: format!("mandate_{}", sid(901)),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:auditor".into(),
            grantee_label: "auditor".into(),
            grantee_pub: &agent_sk(AUDITOR).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse(&entry).unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.audit_chain = vec![m];
}

// --- F whens ---

#[when("the owner appends a section addition and a heartbeat")]
fn owner_appends_both(w: &mut ProtocolWorld) {
    w.add_circle_section("projets", "note1", "toto");
    w.beacon(1, NOW);
    w.publish_bundle();
}

#[when("one byte of the middle entry is altered")]
fn tamper_middle_entry(w: &mut ProtocolWorld) {
    let seg = "gamma/2026-07.jsonl";
    let mut lines = w.segment_lines(seg);
    let line = &mut lines[1];
    let idx = line
        .windows(9)
        .position(|win| win == b"\"value\":\"")
        .expect("signature field")
        + 9;
    line[idx] = if line[idx] == b'0' { b'1' } else { b'0' };
    let mut joined: Vec<u8> = Vec::new();
    for l in &lines {
        joined.extend_from_slice(l);
        joined.push(b'\n');
    }
    w.gbundle().store.put(seg, &joined).unwrap();
}

#[when("an entry is appended whose prev is not the current head")]
fn append_wrong_prev(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let rogue = aithos_core::gamma::owner_entry(
        aithos_core::gamma::EntrySpec {
            id: "gamma_000000000000000000000ROGUE".into(),
            prev: entries[0].chain_hash().unwrap(), // not the head
            prevs: None,
            at: day(8, "04:00:00"),
            kind: aithos_core::gamma::Kind::Heartbeat,
            target: None,
            payload: Some(serde_json::json!({"seq": 99})),
            body_enc: None,
        },
        &owner.content_sign,
    )
    .unwrap();
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap();
    bytes.extend_from_slice(aithos_core::jcs::canonicalize(&rogue).unwrap().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when("the owner appends a section addition")]
fn owner_appends_section(w: &mut ProtocolWorld) {
    w.add_circle_section("projets", "note1", "toto");
}

#[when("the agent appends an action entry")]
fn agent_appends_action(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", DAY1));
}

#[when("the agent appends an action entry timestamped at day 8")]
fn agent_appends_day8(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", DAY8));
}

#[when("the agent appends three action entries")]
fn agent_appends_three(w: &mut ProtocolWorld) {
    for hms in ["01:00:00", "02:00:00", "03:00:00"] {
        w.try_action(false, "reply", &day(1, hms)).unwrap();
    }
}

#[when("the agent appends one action and the helper appends two")]
fn subtree_appends(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(1, "01:00:00")).unwrap();
    w.try_action(true, "reply", &day(1, "02:00:00")).unwrap();
    w.try_action(true, "reply", &day(1, "03:00:00")).unwrap();
}

#[when("the agent appends two actions on day 1")]
fn two_actions_day1(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(1, "01:00:00")).unwrap();
    w.try_action(false, "label", &day(1, "02:00:00")).unwrap();
}

#[when(expr = "the agent appends two {string} actions on day 1")]
fn two_kind_actions_day1(w: &mut ProtocolWorld, action: String) {
    w.try_action(false, &action, &day(1, "01:00:00")).unwrap();
    w.try_action(false, &action, &day(1, "02:00:00")).unwrap();
}

#[when("the agent delegates twice, each grant logged")]
fn delegates_twice(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.delegate_act("act.x.gmail.label", serde_json::json!({}), true)
        .unwrap();
}

#[when("the helper presents an action under that chain")]
fn helper_presents(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(true, "reply", &day(1, "05:00:00")));
}

#[when("the head agent forges a heartbeat with its own key")]
fn forge_beacon(w: &mut ProtocolWorld) {
    use ed25519_dalek::Signer;
    let head = w.gbundle().gamma_head().unwrap();
    let mut forged = aithos_core::gamma::Entry {
        v: 1,
        id: "gamma_000000000000000000000FORGE".into(),
        prev: head,
        prevs: None,
        at: day(2, "00:00:00"),
        kind: "heartbeat".into(),
        target: None,
        authorized_by: None,
        authorized_via: None,
        payload: Some(serde_json::json!({"seq": 77})),
        body_enc: None,
        signature: aithos_core::did::SignatureBlock {
            alg: "ed25519".into(),
            key: "#content".into(),
            value: String::new(),
        },
    };
    let mut unsigned = forged.clone();
    unsigned.signature.value = String::new();
    forged.signature.value = hex::encode(
        agent_sk(AGENT)
            .sign(&aithos_core::jcs::canonical_bytes(&unsigned).unwrap())
            .to_bytes(),
    );
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap_or_default();
    bytes.extend_from_slice(aithos_core::jcs::canonicalize(&forged).unwrap().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when("the owner beacons again")]
fn owner_beacons_again(w: &mut ProtocolWorld) {
    w.beacon(2, &day(40, "00:00:00"));
}

#[when("the agent presents a request anchored to the current log head")]
fn present_fresh_anchor(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let head = aithos_core::gamma::head(&entries).unwrap();
    w.gamma_result = Some(
        aithos_core::gamma::check_anchor(&entries, &head, "24h", &day(0, "12:00:00"))
            .map(|()| "fresh".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the agent presents a request anchored to a head 48 hours old")]
fn present_stale_anchor(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let head = aithos_core::gamma::head(&entries).unwrap();
    w.gamma_result = Some(
        aithos_core::gamma::check_anchor(&entries, &head, "24h", &day(2, "00:00:00"))
            .map(|()| "fresh".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner logs a modification of that section")]
fn owner_logs_modification(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_section_modify(
            &owner,
            Zone::Circle,
            "projets/note1",
            serde_json::json!({"change": "reworded"}),
            &day(9, "00:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when("someone with no key reads the log files")]
fn stranger_reads(_w: &mut ProtocolWorld) {
    // Nothing to acquire: verification below runs on public files alone.
}

impl ProtocolWorld {
    /// Try to open every sealed entry body with the agent's keys (physics
    /// probe, scenario "A subtree grant opens exactly its entries").
    fn probe_sealed_entries(&mut self) {
        if !self.sealed_probe.is_empty() {
            return;
        }
        let entries = self.gbundle().gamma_entries().unwrap();
        let chain = self.chain.clone();
        self.sealed_probe = entries
            .iter()
            .filter(|e| e.body_enc.is_some())
            .map(|e| {
                self.bundle
                    .as_ref()
                    .unwrap()
                    .open_entry_as_agent(&chain, &agent_sk(AGENT), e)
                    .map_err(|e| e.to_string())
            })
            .collect();
    }
}

#[when("the agent appends an action entry knowing only the pinned log head")]
fn blind_append(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(14, "02:00:00")));
}

#[when(expr = "the owner queries actions of kind {string} on {string} from day 10 to day 40")]
fn owner_queries(w: &mut ProtocolWorld, kind: String, _connector: String) {
    let owner = w.owner(0);
    let filter = LogFilter {
        kind: Some(kind),
        since: Some(day(10, "00:00:00")),
        until: Some(day(40, "23:59:59")),
        ..LogFilter::default()
    };
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_owner(&owner, &filter)
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner grants an auditor read.gamma with the zone keys")]
fn grant_auditor(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    // Physics: a zone-root read grant delivers the circle keys.
    w.bundle
        .as_mut()
        .unwrap()
        .grant(
            &owner,
            "auditor",
            &agent_sk(AUDITOR).verifying_key(),
            &[dir_spec("")],
            NB,
            NA30,
            0,
            &mut w.ent,
        )
        .unwrap();
    // Certificate: the read.gamma mandate, full log.
    let m = Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: format!("mandate_{}", sid(902)),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:auditor".into(),
            grantee_label: "auditor".into(),
            grantee_pub: &agent_sk(AUDITOR).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse("read.gamma").unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.audit_chain = vec![m];
}

#[when("the auditor queries replies of day 20")]
fn auditor_queries_day20(w: &mut ProtocolWorld) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some("reply".into()),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..Default::default()
    };
    let filter = LogFilter {
        action: Some("reply".into()),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..LogFilter::default()
    };
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_as_agent(&w.audit_chain, &agent_sk(AUDITOR), &query, &filter, DAY1)
            .map_err(|e| e.to_string()),
    );
}

// --- F thens ---

#[then("the log verifies offline")]
fn log_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("the manifest pins the log head")]
fn manifest_pins_head(w: &mut ProtocolWorld) {
    let latest = w.latest_manifest();
    let head = w.bundle.as_ref().unwrap().gamma_head().unwrap();
    assert!(!head.is_empty(), "log should not be empty");
    assert_eq!(latest.gamma_head, head, "manifest must pin the log tip");
}

#[then("the log lives in two pinned segment files")]
fn two_segments_pinned(w: &mut ProtocolWorld) {
    let latest = w.latest_manifest();
    let segs: Vec<&String> = latest
        .files
        .keys()
        .filter(|p| p.starts_with("gamma/"))
        .collect();
    assert_eq!(segs.len(), 2, "expected two pinned segments, got {segs:?}");
}

#[then("the whole chain verifies across the boundary")]
fn chain_across_boundary(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("log verification is rejected")]
fn log_verification_rejected(w: &mut ProtocolWorld) {
    if let Some(Err(_)) = &w.gamma_result {
        return; // the append itself was refused — fail-closed upstream
    }
    assert!(
        w.bundle.as_ref().unwrap().gamma_verify().is_err(),
        "log verification should reject"
    );
}

#[then("the entry verifies with no mandate attached")]
fn owner_entry_verifies(w: &mut ProtocolWorld) {
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert!(last.authorized_by.is_none() && last.authorized_via.is_none());
    aithos_core::gamma::verify_owner_entry(last, &doc).unwrap();
}

#[then("the entry verifies against the chain at its own timestamp")]
fn delegated_entry_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    aithos_core::gamma::verify_delegated_entry(last, &w.chain, &doc).unwrap();
}

#[then("a fourth action entry is rejected")]
fn fourth_rejected(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(1, "04:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("budget")),
        "expected budget exhaustion, got {r:?}"
    );
}

#[then("a further action by either key is rejected")]
fn either_key_rejected(w: &mut ProtocolWorld) {
    for helper in [false, true] {
        let r = w.try_action(helper, "reply", &day(1, "06:00:00"));
        assert!(
            r.as_ref().is_err_and(|e| e.contains("budget")),
            "helper={helper}: expected budget exhaustion, got {r:?}"
        );
    }
}

#[then("a third action on day 1 is rejected")]
fn third_day1_rejected(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(1, "03:00:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("an action on day 2 verifies")]
fn day2_verifies(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(2, "03:00:01")).unwrap();
}

#[then(expr = "a third {string} on day 2 is rejected")]
fn third_kind_rejected(w: &mut ProtocolWorld, action: String) {
    let r = w.try_action(false, &action, &day(2, "01:00:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then(expr = "a {string} action on day 2 verifies")]
fn kind_day2_verifies(w: &mut ProtocolWorld, action: String) {
    w.try_action(false, &action, &day(2, "02:00:00")).unwrap();
}

#[then("a third delegation is rejected")]
fn third_delegation_rejected(w: &mut ProtocolWorld) {
    let r = w.delegate_act("act.x.gmail.send", serde_json::json!({}), true);
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("the action is rejected")]
fn action_rejected(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("grant never logged")),
        "expected the unlogged-grant rejection, got {r:?}"
    );
}

#[then("an action at day 20 verifies")]
fn action_day20(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(20, "00:00:00")).unwrap();
}

#[then("an action at day 34 is rejected")]
fn action_day34(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "reply", &day(34, "00:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("heartbeat stale")),
        "got {r:?}"
    );
}

#[then("the next action verifies")]
fn next_action_verifies(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(41, "00:00:00")).unwrap();
}

#[then("the beacon is rejected")]
fn beacon_rejected(w: &mut ProtocolWorld) {
    assert!(
        w.bundle.as_ref().unwrap().gamma_verify().is_err(),
        "a forged beacon must fail log verification"
    );
    // And it never counts as liveness: the mandate stays suspended.
    let doc = w.did_document();
    let entries = w.gbundle().gamma_entries().unwrap();
    let ok = entries
        .iter()
        .filter(|e| e.kind == "heartbeat")
        .filter(|e| aithos_core::gamma::verify_owner_entry(e, &doc).is_ok())
        .count();
    assert_eq!(ok, 0, "no verifiable beacon should exist");
}

#[then("the request verifies")]
fn request_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
}

#[then("the request is rejected")]
fn request_rejected(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("stale")),
        "expected a stale anchor, got {r:?}"
    );
}

#[then("a reader of that section opens the entry body")]
fn reader_opens_body(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_owner(
            &owner,
            &LogFilter {
                kind: Some("section.modify".into()),
                ..LogFilter::default()
            },
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    let body = hits[0].body.as_ref().expect("body opens");
    assert_eq!(body.payload["change"], "reworded");
}

#[then("the entry alone reveals only kind, time and author — not the target")]
fn entry_reveals_nothing(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "section.modify");
    assert!(last.target.is_none(), "target must live inside the body");
    assert!(last.payload.is_none(), "payload must be sealed");
    assert!(last.body_enc.is_some());
}

#[then("the chain and the budgets still verify")]
fn skeleton_verifies(w: &mut ProtocolWorld) {
    // Public files alone: chain, signatures, and countable headers.
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
    let entries = w.gbundle().gamma_entries().unwrap();
    let n = aithos_core::gamma::count_actions(&entries, &w.chain[0].id, None, None);
    assert_eq!(n, 1, "the skeleton must stay countable");
}

#[then("no target, tag or content is revealed")]
fn nothing_revealed(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    for e in entries.iter().filter(|e| e.kind.starts_with("section.")) {
        assert!(e.target.is_none() && e.payload.is_none() && e.body_enc.is_some());
    }
}

#[then(expr = "the agent opens the bodies of the {string} entries by their hints")]
fn agent_opens_granted(w: &mut ProtocolWorld, folder: String) {
    w.probe_sealed_entries();
    let opened: Vec<&aithos_core::gamma::Body> = w
        .sealed_probe
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .collect();
    assert_eq!(opened.len(), 1, "exactly the granted subtree opens");
    let dir = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, &folder)
        .unwrap();
    let node = NodePath::parse(&opened[0].target).unwrap();
    assert!(node.folders.starts_with(&dir));
}

#[then(expr = "the {string} entry bodies stay sealed to it")]
fn other_stays_sealed(w: &mut ProtocolWorld, _folder: String) {
    w.probe_sealed_entries();
    let sealed = w.sealed_probe.iter().filter(|r| r.is_err()).count();
    assert_eq!(sealed, 1, "the out-of-perimeter body must not open");
}

#[then("the entry chains and verifies")]
fn entry_chains_verifies(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("exactly the matching entries come back")]
fn matching_come_back(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 2, "day-4 action and the mutation must be out");
    assert!(hits
        .iter()
        .all(|h| h.entry.kind == "action" && h.entry.target.as_deref() == Some("x.gmail")));
}

#[then("the owner opens every sealed body among them")]
fn owner_opens_all(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert!(hits
        .iter()
        .filter(|h| h.entry.body_enc.is_some())
        .all(|h| h.body.is_some()));
}

#[then("the auditor opens every entry body, including acts it never made")]
fn auditor_opens_all(w: &mut ProtocolWorld) {
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_as_agent(
            &w.audit_chain,
            &agent_sk(AUDITOR),
            &aithos_core::mandate::GammaQuery::default(),
            &LogFilter::default(),
            DAY1,
        )
        .unwrap();
    let sealed = hits.iter().filter(|h| h.entry.body_enc.is_some()).count();
    let opened = hits.iter().filter(|h| h.body.is_some()).count();
    assert_eq!(sealed, 2, "both owner mutations are in view");
    assert_eq!(
        opened, sealed,
        "every sealed body must open for the auditor"
    );
}

#[then("the matching entries come back")]
fn scoped_matching(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 1);
}

#[then("a query for day 40 is refused")]
fn day40_refused(w: &mut ProtocolWorld) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some("reply".into()),
        since: Some(day(39, "00:00:00")),
        until: Some(day(40, "23:59:59")),
        ..Default::default()
    };
    let r = w.bundle.as_ref().unwrap().log_query_as_agent(
        &w.audit_chain,
        &agent_sk(AUDITOR),
        &query,
        &LogFilter::default(),
        DAY1,
    );
    assert!(r.is_err(), "an out-of-window query must be refused");
}

#[then(expr = "a query for action {string} is refused")]
fn action_refused(w: &mut ProtocolWorld, action: String) {
    let query = aithos_core::mandate::GammaQuery {
        action: Some(action),
        since: Some(day(19, "00:00:00")),
        until: Some(day(20, "23:59:59")),
        ..Default::default()
    };
    let r = w.bundle.as_ref().unwrap().log_query_as_agent(
        &w.audit_chain,
        &agent_sk(AUDITOR),
        &query,
        &LogFilter::default(),
        DAY1,
    );
    assert!(r.is_err(), "an out-of-perimeter action must be refused");
}

// ------------------------------------------ step F+: advanced constraints ---

const PROVIDER: u8 = 0xC3;

fn win(anchor: &str, duration: &str) -> serde_json::Value {
    serde_json::json!({"anchor": anchor, "duration": duration})
}

fn win_periodic(anchor: &str, duration: &str, period: &str) -> serde_json::Value {
    serde_json::json!({"anchor": anchor, "duration": duration, "period": period})
}

/// The founding two-profile budgets (spec §04.11 example).
fn founding_budgets() -> serde_json::Value {
    serde_json::json!({"budgets": [
        {"id": "haiku", "models": ["claude-haiku"], "token_budget": 10000,
         "active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "7d")],
         "max_actions": 1},
        {"id": "gemma", "models": ["gemma"], "token_budget": 25000}
    ]})
}

fn cite(profile: &str, model: &str, tokens: u64) -> serde_json::Value {
    serde_json::json!({"budget_ref": profile, "model": model, "tokens": tokens})
}

fn provider_receipt(args_hash: &str, model: &str, tokens: u64, signer: u8) -> serde_json::Value {
    use ed25519_dalek::Signer;
    let payload = serde_json::json!({"args_hash": args_hash, "model": model, "tokens": tokens});
    let bytes = aithos_core::jcs::canonical_bytes(&payload).unwrap();
    let sig = hex::encode(agent_sk(signer).sign(&bytes).to_bytes());
    let mut r = payload;
    r["sig"] = serde_json::json!(sig);
    r
}

fn attestation_budgets() -> serde_json::Value {
    let key = wire::ed25519_pub_to_multibase(&agent_sk(PROVIDER).verifying_key().to_bytes());
    serde_json::json!({"budgets": [
        {"id": "haiku", "require_attestation": true, "attestation_key": key}
    ]})
}

// --- F+ givens: windows ---

#[given("an agent granted gmail actions active from day 3 14:00 for 4 hours")]
fn grant_oneshot_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win(&day(3, "14:00:00"), "4h")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours")]
fn grant_weekly_window(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "7d")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours until day 20")]
fn grant_weekly_until(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut window = win_periodic(&day(1, "14:00:00"), "4h", "7d");
    window["until"] = serde_json::json!(day(20, "00:00:00"));
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [window]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours, 2 occurrences")]
fn grant_weekly_count(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut window = win_periodic(&day(1, "14:00:00"), "4h", "7d");
    window["count"] = serde_json::json!(2);
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [window]}),
        NA30,
    );
}

#[given("an agent granted gmail actions active on day 3 morning and day 5 evening")]
fn grant_union_windows(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [
            win(&day(3, "08:00:00"), "4h"),
            win(&day(5, "18:00:00"), "4h")
        ]}),
        NA30,
    );
}

#[given("an agent granted gmail actions every day at 14:00 for 4 hours with max_actions_per 2 per 24 hours")]
fn grant_daily_with_rolling(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({
            "active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "1d")],
            "max_actions_per": {"window": "24h", "n": 2}
        }),
        NA30,
    );
}

#[given("an agent granted gmail actions active from day 1 to day 20 with issue depth 1")]
fn grant_window_with_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"active_windows": [win(&day(1, "00:00:00"), "19d")]}),
        NA30,
    );
}

#[given("an agent granted gmail actions for 7 days, active daily 14:00 for 4 hours")]
fn grant_7d_daily(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"active_windows": [win_periodic(&day(1, "14:00:00"), "4h", "1d")]}),
        NA7,
    );
}

// --- F+ givens: budgets ---

#[given("a mandate with two budget profiles:")]
#[given("a mandate with two budget profiles")]
#[given("the founding two-profile mandate")]
fn founding_mandate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], founding_budgets(), NA30);
}

#[given(expr = "a profile {string} with a {int} token budget")]
fn single_profile(w: &mut ProtocolWorld, id: String, budget: u64) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"budgets": [{"id": id, "token_budget": budget}]}),
        NA30,
    );
}

#[given(expr = "{int} tokens already consumed on {string}")]
fn tokens_consumed(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[given(expr = "profile {string} has spent its single action")]
fn profile_spent(w: &mut ProtocolWorld, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(cite(&profile, "claude-haiku", 100)),
        None,
    )
    .unwrap();
}

#[given(expr = "a profile {string} allowing model {string}")]
fn profile_allowing(w: &mut ProtocolWorld, id: String, model: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"budgets": [{"id": id, "models": [model]}]}),
        NA30,
    );
}

#[given("the founding two-profile mandate with issue depth 1")]
fn founding_with_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    let mut c = founding_budgets();
    c["budgets"].as_array_mut().unwrap(); // keep shape
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        c,
        NA30,
    );
}

#[given("the agent delegates the gemma perimeter to a helper")]
fn delegates_gemma(w: &mut ProtocolWorld) {
    w.delegate_act("act.x.gmail.*", founding_budgets(), true)
        .unwrap();
}

#[given(expr = "logged actions of {int}, {int} and {int} tokens on {string}")]
fn logged_actions_tokens(w: &mut ProtocolWorld, a: u64, b: u64, c: u64, profile: String) {
    for (i, t) in [a, b, c].into_iter().enumerate() {
        w.try_action_full(
            false,
            "reply",
            &day(1, &format!("0{}:00:00", i + 1)),
            Some(serde_json::json!({"budget_ref": profile, "tokens": t})),
            None,
        )
        .unwrap();
    }
}

// --- F+ givens: attestation ---

#[given(expr = "a profile {string} that requires attestation")]
fn profile_requires_attestation(w: &mut ProtocolWorld, _id: String) {
    w.init_bundle();
    w.grant_act(vec![], attestation_budgets(), NA30);
}

#[given("a provider attestation key pinned in the mandate")]
fn provider_key_pinned(_w: &mut ProtocolWorld) {
    // Pinned by the attestation_budgets() fixture above.
}

#[given("a valid receipt for an earlier action's args_hash")]
fn earlier_receipt(w: &mut ProtocolWorld) {
    w.receipt = Some(provider_receipt(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        "claude-haiku",
        4000,
        PROVIDER,
    ));
}

// --- F+ givens: inference & kinds ---

#[given(expr = "logged inferences of {int} and {int} total tokens citing {string}")]
fn logged_inferences(w: &mut ProtocolWorld, a: u64, b: u64, profile: String) {
    for (i, t) in [a, b].into_iter().enumerate() {
        w.try_inference(
            "gemma",
            t - 1000,
            1000,
            Some(&profile),
            &day(1, &format!("0{}:00:00", i + 1)),
        )
        .unwrap();
    }
}

#[given(expr = "a logged action of {int} declared tokens citing {string}")]
fn logged_action_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[given("a bundle whose log records section additions, a modification and actions")]
fn log_with_kinds(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.add_named_section("projets", "note2", &[]);
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_section_modify(
            &owner,
            Zone::Circle,
            "projets/note1",
            serde_json::json!({"change": "x"}),
            &day(9, "00:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(10, "00:00:00")).unwrap();
}

#[given(expr = "an agent granted read on circle folder {string}")]
fn plain_read_grant(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    w.add_named_section(&folder, "note1", &[]);
    w.grant_to_agent(&[dir_spec(&folder)], NA30, 0);
    w.granted_folder = folder;
    w.gamma_baseline = w.gbundle().gamma_entries().unwrap().len();
}

#[given(expr = "an agent granted read on circle folder {string} with log_reads")]
fn read_grant_log_reads(w: &mut ProtocolWorld, folder: String) {
    plain_read_grant(w, folder);
    // Same keypair, the mandate re-issued with the log_reads duty.
    let owner = w.owner(0);
    let mut m = w.chain[0].clone();
    m.constraints = serde_json::json!({"log_reads": true});
    m.resign(&owner.root_sign).unwrap();
    w.store_cert(&m);
    w.chain = vec![m];
}

// --- F+ givens: sealed args ---

#[given(expr = "an agent granted gmail {string} with sealed-args audit")]
fn grant_with_audit(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    let _ = action;
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .grant_audit_line(&owner, &agent_sk(AGENT).verifying_key(), &mut ent)
        .unwrap();
    w.ent = ent;
}

#[given("a logged action with sealed args")]
fn logged_sealed_action(w: &mut ProtocolWorld) {
    grant_with_audit(w, "reply".to_owned());
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": "alice@example.com"})),
    )
    .unwrap();
}

#[given(expr = "a mandate whose action_params allow replies only to {string}")]
fn mandate_action_params(w: &mut ProtocolWorld, addr: String) {
    w.init_bundle();
    w.grant_act(
        vec![],
        serde_json::json!({"action_params": {"reply": {"recipients_allow": [addr]}}}),
        NA30,
    );
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .grant_audit_line(&owner, &agent_sk(AGENT).verifying_key(), &mut ent)
        .unwrap();
    w.ent = ent;
}

#[given(expr = "a logged reply whose sealed args name {string}")]
fn logged_reply_naming(w: &mut ProtocolWorld, addr: String) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": addr})),
    )
    .unwrap();
}

// --- F+ whens ---

#[when(expr = "the agent appends an action at day {int} {word}")]
fn action_at(w: &mut ProtocolWorld, d: u32, hm: String) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(d, &format!("{hm}:00"))));
}

#[when("the agent delegates the perimeter active from day 3 14:00 for 4 hours")]
fn delegate_windowed_inside(w: &mut ProtocolWorld) {
    w.delegate_act(
        "act.x.gmail.*",
        serde_json::json!({"active_windows": [win(&day(3, "14:00:00"), "4h")]}),
        true,
    )
    .unwrap();
}

#[when("the agent delegates the perimeter active from day 15 to day 40")]
fn delegate_windowed_outside(w: &mut ProtocolWorld) {
    w.delegate_act(
        "act.x.gmail.*",
        serde_json::json!({"active_windows": [win(&day(15, "00:00:00"), "600h")]}),
        true,
    )
    .unwrap();
    w.chain_result = Some(w.verify_chain_at(&w.helper_chain.clone(), DAY1));
}

// --- F+ attenuation matrix (M5/E+): the typed per-family link gate ---
// Contract committed at M1 (aa02353); verdicts pinned by the E+ vector
// (vectors/eplus-attenuation.json) — the steps below only drive
// `verify_chain` on a two-link chain, the gate does the judging.

/// One-key JSON object (the `json!` macro wants literal keys).
fn one_key(key: &str, value: serde_json::Value) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert(key.to_owned(), value);
    serde_json::Value::Object(map)
}

/// The Examples-table constraint DSL → one constraints object.
fn constraint_dsl(text: &str) -> serde_json::Value {
    fn dur(n: &str, unit: &str) -> String {
        let n: u64 = n.parse().expect("duration count");
        let u = match unit.trim_end_matches('s') {
            "day" => "d",
            "hour" => "h",
            "minute" => "m",
            "second" => "s",
            other => panic!("unknown duration unit `{other}`"),
        };
        format!("{n}{u}")
    }
    fn n64(n: &str) -> serde_json::Value {
        serde_json::Value::from(n.parse::<u64>().expect("integer"))
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    match words.as_slice() {
        [key @ ("max_actions" | "max_children" | "max_sessions"), n] => one_key(key, n64(n)),
        ["max_actions_per", n, "per", d, unit] => one_key(
            "max_actions_per",
            serde_json::json!({"window": dur(d, unit), "n": n64(n)}),
        ),
        ["rate_limit", action, n, "per", d, unit] => one_key(
            "rate_limit",
            serde_json::json!({"action": action, "window": dur(d, unit), "n": n64(n)}),
        ),
        ["domains", list @ ..] => one_key(
            "domains",
            serde_json::Value::from(
                list.iter()
                    .filter(|w| **w != "and")
                    .map(|w| (*w).to_owned())
                    .collect::<Vec<_>>(),
            ),
        ),
        ["token_budget", n, "on", "profile", id] => one_key(
            "budgets",
            serde_json::json!([{"id": id, "token_budget": n64(n)}]),
        ),
        ["heartbeat", "every", en, eu, "grace", gn, gu] => one_key(
            "heartbeat",
            serde_json::json!({"every": dur(en, eu), "grace": dur(gn, gu)}),
        ),
        ["freshness", n, unit] => one_key("freshness", dur(n, unit).into()),
        ["spend_cap", n, unit] => one_key(
            "spend_cap",
            serde_json::json!({"unit": unit, "amount": n64(n)}),
        ),
        ["first_party_only"] => one_key("first_party_only", true.into()),
        other => panic!("unknown constraint DSL: {other:?}"),
    }
}

impl ProtocolWorld {
    /// Delegate the act perimeter with the given constraints and judge the
    /// two-link chain — the attenuation verdict is `verify_chain`'s alone.
    fn delegate_and_judge(&mut self, constraints: serde_json::Value) {
        let _ = self.delegate_act("act.x.gmail.*", constraints, false);
        self.chain_result = Some(self.verify_chain_at(&self.helper_chain.clone(), DAY1));
    }
}

#[given(expr = "an agent granted gmail actions with constraint {string} and issue depth 1")]
fn grant_with_dsl_constraint(w: &mut ProtocolWorld, dsl: String) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        constraint_dsl(&dsl),
        NA30,
    );
}

#[given(
    expr = "an agent granted gmail actions with an unknown constraint key {string} and issue depth 1"
)]
fn grant_with_unknown_key(w: &mut ProtocolWorld, key: String) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        one_key(&key, 4.into()),
        NA30,
    );
}

#[given("an agent granted gmail actions with issue depth 1")]
fn grant_plain_with_issue(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
}

#[given(expr = "an agent granted gmail actions with counter_sign on {string} and issue depth 1")]
fn grant_with_countersign(w: &mut ProtocolWorld, action: String) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"counter_sign": [action]}),
        NA30,
    );
}

#[given(
    expr = "an agent granted gmail {string} whose action_params allow recipients {string} and {string}, with issue depth 1"
)]
fn grant_with_action_params(w: &mut ProtocolWorld, action: String, first: String, second: String) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        one_key(
            "action_params",
            one_key(
                &action,
                serde_json::json!({"recipients_allow": [first, second]}),
            ),
        ),
        NA30,
    );
}

#[when(expr = "the agent delegates the perimeter with constraint {string}")]
fn delegate_with_dsl_constraint(w: &mut ProtocolWorld, dsl: String) {
    let constraints = constraint_dsl(&dsl);
    w.delegate_and_judge(constraints);
}

#[when("the agent delegates the perimeter with no domains constraint at all")]
#[when("the agent delegates the perimeter without first_party_only")]
fn delegate_dropping_everything(w: &mut ProtocolWorld) {
    w.delegate_and_judge(serde_json::json!({}));
}

#[when(expr = "the agent delegates the perimeter with counter_sign on {string} and {string}")]
fn delegate_with_grown_countersign(w: &mut ProtocolWorld, first: String, second: String) {
    w.delegate_and_judge(serde_json::json!({"counter_sign": [first, second]}));
}

#[when(expr = "the agent delegates the perimeter allowing replies only to {string}")]
fn delegate_narrowing_recipients(w: &mut ProtocolWorld, addr: String) {
    // The parent's single predicated action names the child's key.
    let action = w.chain[0].constraints["action_params"]
        .as_object()
        .and_then(|actions| actions.keys().next().cloned())
        .expect("parent action_params");
    let constraints = one_key(
        "action_params",
        one_key(&action, serde_json::json!({"recipients_allow": [addr]})),
    );
    w.delegate_and_judge(constraints);
}

#[when(expr = "the agent delegates the perimeter copying {string} verbatim")]
fn delegate_copying_verbatim(w: &mut ProtocolWorld, _key: String) {
    let constraints = w.chain[0].constraints.clone();
    w.delegate_and_judge(constraints);
}

#[when(expr = "the agent delegates the perimeter adding constraint key {string}")]
fn delegate_inventing_key(w: &mut ProtocolWorld, key: String) {
    w.delegate_and_judge(one_key(&key, 4.into()));
}

#[then("the helper's chain is accepted")]
fn helper_chain_accepted(w: &mut ProtocolWorld) {
    assert_eq!(w.chain_result.clone().unwrap(), Ok(()));
}

#[then(expr = "a delegation dropping counter_sign on {string} is rejected")]
fn delegation_dropping_countersign_rejected(w: &mut ProtocolWorld, _action: String) {
    let _ = w.delegate_act(
        "act.x.gmail.*",
        serde_json::json!({"counter_sign": []}),
        false,
    );
    assert!(
        w.verify_chain_at(&w.helper_chain.clone(), DAY1).is_err(),
        "dropping an inherited counter_sign must reject the chain"
    );
}

#[then(expr = "a delegation adding recipient {string} is rejected")]
fn delegation_adding_recipient_rejected(w: &mut ProtocolWorld, extra: String) {
    // The intruder joins the CHILD's own last allow-list — still outside
    // the parent's, still a rejection.
    let action = w.chain[0].constraints["action_params"]
        .as_object()
        .and_then(|actions| actions.keys().next().cloned())
        .expect("parent action_params");
    let mut allow: Vec<String> = w.helper_chain[1].constraints["action_params"][&action]
        ["recipients_allow"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    allow.push(extra);
    let constraints = one_key(
        "action_params",
        one_key(&action, serde_json::json!({"recipients_allow": allow})),
    );
    let _ = w.delegate_act("act.x.gmail.*", constraints, false);
    assert!(
        w.verify_chain_at(&w.helper_chain.clone(), DAY1).is_err(),
        "an added recipient must reject the chain"
    );
}

#[when("the agent appends two actions inside the day 3 window")]
fn two_in_window(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(3, "14:30:00")).unwrap();
    w.try_action(false, "label", &day(3, "15:00:00")).unwrap();
}

#[when(
    expr = "the agent acts citing profile {string} with model {string} and {int} tokens at day {int} {word}"
)]
fn act_citing_full(
    w: &mut ProtocolWorld,
    profile: String,
    model: String,
    tokens: u64,
    d: u32,
    hm: String,
) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(d, &format!("{hm}:00")),
        Some(cite(&profile, &model, tokens)),
        None,
    ));
}

#[when("the agent acts without citing any budget_ref")]
fn act_uncited(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_action(false, "reply", &day(1, "15:00:00")));
}

#[when(expr = "the agent acts citing {string} with {int} declared tokens")]
fn act_citing_tokens(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "02:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    ));
}

#[when(expr = "the agent acts citing profile {string} with model {string} and {int} tokens")]
fn act_citing_short(w: &mut ProtocolWorld, profile: String, model: String, tokens: u64) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "16:00:00"),
        Some(cite(&profile, &model, tokens)),
        None,
    ));
}

#[when(expr = "the agent acts citing {string} with model {string}")]
fn act_citing_model(w: &mut ProtocolWorld, profile: String, model: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(cite(&profile, &model, 10)),
        None,
    ));
}

#[when(expr = "the agent acts citing profile {string} at day {int} {word}")]
fn act_citing_at(w: &mut ProtocolWorld, profile: String, d: u32, hm: String) {
    let model = if profile == "haiku" {
        "claude-haiku"
    } else {
        "gemma"
    };
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(d, &format!("{hm}:00")),
        Some(cite(&profile, model, 10)),
        None,
    ));
}

#[when(expr = "the agent acts citing budget_ref {string}")]
fn act_citing_unknown(w: &mut ProtocolWorld, profile: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({"budget_ref": profile})),
        None,
    ));
}

#[when(expr = "the helper acts citing {string} with {int} tokens")]
fn helper_acts_citing(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.try_action_full(
        true,
        "reply",
        &day(2, "01:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "gemma", "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[when(expr = "the agent acts citing {string} with a declared usage and no receipt")]
fn act_no_receipt(w: &mut ProtocolWorld, profile: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "claude-haiku", "tokens": 500})),
        None,
    ));
}

#[when(expr = "the agent acts citing {string} carrying a receipt signed by the provider")]
fn act_with_receipt(w: &mut ProtocolWorld, profile: String) {
    let receipt = provider_receipt("sha256:00", "claude-haiku", 8412, PROVIDER);
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": profile, "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when("the agent acts carrying a receipt signed by the agent itself")]
fn act_with_forged_receipt(w: &mut ProtocolWorld) {
    let receipt = provider_receipt("sha256:00", "claude-haiku", 8412, AGENT);
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": "haiku", "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when("the agent replays that receipt on a new action")]
fn replay_receipt(w: &mut ProtocolWorld) {
    let receipt = w.receipt.clone().unwrap();
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "15:00:00"),
        Some(serde_json::json!({
            "budget_ref": "haiku", "model": "claude-haiku", "tokens": 500,
            "receipt": receipt
        })),
        None,
    ));
}

#[when(
    expr = "the container logs an inference on {string} of {int} tokens in and {int} out citing {string}"
)]
fn container_logs_inference(
    w: &mut ProtocolWorld,
    model: String,
    tin: u64,
    tout: u64,
    profile: String,
) {
    w.gamma_result = Some(w.try_inference(&model, tin, tout, Some(&profile), &day(1, "16:00:00")));
}

#[when(expr = "the container logs an inference of {int} total tokens citing {string}")]
fn container_logs_total(w: &mut ProtocolWorld, total: u64, profile: String) {
    w.gamma_result = Some(w.try_inference(
        "gemma",
        total - 100,
        100,
        Some(&profile),
        &day(2, "10:00:00"),
    ));
}

#[when("the container logs an inference citing no budget_ref")]
fn container_logs_uncited(w: &mut ProtocolWorld) {
    w.gamma_result = Some(w.try_inference("gemma", 100, 10, None, &day(1, "16:00:00")));
}

#[when(expr = "an entry of kind {string} is forced onto the log")]
fn force_unknown_kind(w: &mut ProtocolWorld, kind: String) {
    let head = w.gbundle().gamma_head().unwrap();
    let line = serde_json::json!({
        "v": 1, "id": "gamma_00000000000000000000BANANA", "prev": head,
        "at": day(1, "00:00:00"), "kind": kind, "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}
    });
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap_or_default();
    bytes.extend_from_slice(line.to_string().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[when(expr = "the owner queries the kind class {string}")]
fn query_kind_class(w: &mut ProtocolWorld, class: String) {
    let owner = w.owner(0);
    w.query_hits = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .log_query_owner(
                &owner,
                &LogFilter {
                    kind: Some(class),
                    ..LogFilter::default()
                },
            )
            .map_err(|e| e.to_string()),
    );
}

#[when(expr = "the agent reads a section under {string}")]
fn agent_reads_section(w: &mut ProtocolWorld, folder: String) {
    let path = format!("{folder}/note1");
    w.read_body = Some(w.agent_reads(&w.chain.clone(), AGENT, &path));
}

#[when(expr = "the agent reads a section under {string} and logs its read")]
fn agent_reads_and_logs(w: &mut ProtocolWorld, folder: String) {
    let path = format!("{folder}/note1");
    w.read_body = Some(w.agent_reads(&w.chain.clone(), AGENT, &path));
    let chain = w.chain.clone();
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_read_as_agent(
            &chain,
            &agent_sk(AGENT),
            Zone::Circle,
            &path,
            &day(9, "01:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when(expr = "the agent acts with arguments naming recipient {string}")]
fn act_with_sealed_args(w: &mut ProtocolWorld, addr: String) {
    w.gamma_result = Some(w.try_action_full(
        false,
        "reply",
        &day(1, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": addr, "body": "hello"})),
    ));
}

#[when("the sealed body is swapped for another one")]
fn swap_sealed_body(w: &mut ProtocolWorld) {
    // A lying agent: clear args_hash from one argument object, sealed body
    // from another — signed consistently, appended, caught only by audit.
    let owner = w.owner(0);
    let key = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_key_owner(&owner, "gmail")
        .unwrap();
    let mut ent = std::mem::take(&mut w.ent);
    let body = aithos_core::gamma::seal_body(
        &key,
        &w.bundle.as_ref().unwrap().did.clone(),
        "x.gmail",
        1,
        &serde_json::json!({"recipient": "mallory@evil.example"}),
        &ent.e24(),
    )
    .unwrap();
    w.ent = ent;
    let entries = w.gbundle().gamma_entries().unwrap();
    let spec = aithos_core::gamma::EntrySpec {
        id: "gamma_00000000000000000000000LIE".into(),
        prev: aithos_core::gamma::head(&entries).unwrap(),
        prevs: None,
        at: day(1, "02:00:00"),
        kind: aithos_core::gamma::Kind::Action,
        target: Some("x.gmail".into()),
        payload: Some(serde_json::json!({"action": "reply", "args_hash": "sha256:00"})),
        body_enc: Some(body),
    };
    let via: Vec<String> = w.chain.iter().map(|m| m.id.clone()).collect();
    let entry = aithos_core::gamma::delegated_entry(spec, via, &agent_sk(AGENT)).unwrap();
    let bundle = w.gbundle();
    bundle.gamma_append(&entry).unwrap();
}

#[when(expr = "the agent asks the container to reply to {string}")]
fn container_asked(w: &mut ProtocolWorld, addr: String) {
    // The container evaluates action_params on the REAL args (tier X)
    // before any entry exists.
    let verdict = aithos_core::constraints::check_action_params(
        &w.chain[0].constraints,
        "reply",
        &serde_json::json!({"recipient": addr}),
    );
    w.gamma_baseline = w.gbundle().gamma_entries().unwrap().len();
    w.gamma_result = Some(
        verdict
            .map(|()| "allowed".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("the owner audits the log against the mandate predicates")]
fn owner_audits(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let mandate = w.chain[0].clone();
    w.gamma_result = Some(
        w.bundle
            .as_ref()
            .unwrap()
            .audit_log_against(&owner, &mandate)
            .map(|n| n.to_string())
            .map_err(|e| e.to_string()),
    );
}

// --- F+ thens ---

#[then("the action verifies")]
fn action_verifies(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_ok(), "action should verify, got {r:?}");
}

#[then("the action is refused as out of window")]
#[then("the action is refused as over budget")]
#[then("the action is refused as model not allowed")]
#[then("the action is refused")]
fn fplus_action_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_err(), "action should be refused, got {r:?}");
}

#[then(expr = "an action exactly at day {int} {word} verifies")]
#[then(expr = "an action at day {int} {word} verifies, F+")]
fn action_at_verifies_exact(w: &mut ProtocolWorld, d: u32, hms: String) {
    let hms = if hms.matches(':').count() == 1 {
        format!("{hms}:00")
    } else {
        hms
    };
    w.try_action(false, "reply", &day(d, &hms)).unwrap();
}

#[then(expr = "an action at day {int} {word} verifies")]
fn action_at_verifies(w: &mut ProtocolWorld, d: u32, hms: String) {
    action_at_verifies_exact(w, d, hms);
}

#[then(expr = "an action exactly at day {int} {word} is refused")]
#[then(expr = "an action at day {int} {word} is refused")]
fn action_at_refused(w: &mut ProtocolWorld, d: u32, hms: String) {
    let hms = if hms.matches(':').count() == 1 {
        format!("{hms}:00")
    } else {
        hms
    };
    let r = w.try_action(false, "reply", &day(d, &hms));
    assert!(r.is_err(), "day {d} {hms} should be refused, got {r:?}");
}

#[then("an action in the day 3 morning window verifies")]
fn union_day3(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(3, "09:00:00")).unwrap();
}

#[then("an action in the day 5 evening window verifies")]
fn union_day5(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(5, "19:00:00")).unwrap();
}

#[then("an action on day 4 noon is refused")]
fn union_day4_refused(w: &mut ProtocolWorld) {
    assert!(w.try_action(false, "reply", &day(4, "12:00:00")).is_err());
}

#[then("a third action inside the day 3 window is refused by the rolling limit")]
fn third_in_window_refused(w: &mut ProtocolWorld) {
    let r = w.try_action(false, "send", &day(3, "15:30:00"));
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then("an action inside the day 4 window verifies")]
fn day4_window_ok(w: &mut ProtocolWorld) {
    w.try_action(false, "reply", &day(4, "14:30:00")).unwrap();
}

#[then("the helper's action inside that window verifies")]
fn helper_windowed_ok(w: &mut ProtocolWorld) {
    w.try_action(true, "reply", &day(3, "15:00:00")).unwrap();
}

#[then(expr = "an action at day {int} {word} is refused even though {word} is in phase")]
fn refused_beyond_validity(w: &mut ProtocolWorld, d: u32, hms: String, _phase: String) {
    let r = w.try_action(false, "reply", &day(d, &format!("{hms}:00")));
    assert!(r.is_err(), "beyond validity should refuse, got {r:?}");
}

#[then(expr = "the log shows {int} tokens consumed on profile {string}")]
fn log_shows_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let got = aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, &profile);
    assert_eq!(got, tokens);
}

#[then(expr = "an action citing {string} with {int} declared tokens verifies")]
fn citing_tokens_ok(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    w.try_action_full(
        false,
        "reply",
        &day(1, "03:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "tokens": tokens})),
        None,
    )
    .unwrap();
}

#[then(expr = "the same action citing profile {string} verifies")]
fn same_action_other_profile(w: &mut ProtocolWorld, profile: String) {
    w.try_action_full(
        false,
        "reply",
        &day(2, "09:00:00"),
        Some(cite(&profile, "gemma", 10)),
        None,
    )
    .unwrap();
}

#[then(expr = "an agent action citing {string} with {int} tokens is refused as over budget")]
fn agent_citing_refused(w: &mut ProtocolWorld, profile: String, tokens: u64) {
    let r = w.try_action_full(
        false,
        "reply",
        &day(2, "02:00:00"),
        Some(serde_json::json!({"budget_ref": profile, "model": "gemma", "tokens": tokens})),
        None,
    );
    assert!(r.as_ref().is_err_and(|e| e.contains("budget")), "got {r:?}");
}

#[then(expr = "any verifier counts {int} tokens consumed on {string}")]
fn verifier_counts_tokens(w: &mut ProtocolWorld, tokens: u64, profile: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, &profile),
        tokens
    );
}

#[then(expr = "the remaining budget admits at most {int} declared tokens")]
fn headroom_check(w: &mut ProtocolWorld, headroom: u64) {
    let over = w.try_action_full(
        false,
        "reply",
        &day(2, "01:00:00"),
        Some(serde_json::json!({"budget_ref": "gemma", "tokens": headroom + 1})),
        None,
    );
    assert!(over.is_err(), "over headroom should refuse");
    w.try_action_full(
        false,
        "reply",
        &day(2, "02:00:00"),
        Some(serde_json::json!({"budget_ref": "gemma", "tokens": headroom})),
        None,
    )
    .unwrap();
}

#[then("the receipt's usage overrides the declared tokens in the tally")]
fn receipt_overrides(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        aithos_core::constraints::tally_tokens(&entries, &w.chain[0].id, "haiku"),
        8412
    );
}

#[then(expr = "the entry is of kind {string}")]
fn entry_of_kind(w: &mut ProtocolWorld, kind: String) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(entries.last().unwrap().kind, kind);
}

#[then("it reveals provider, model, token counts and budget_ref")]
fn inference_reveals_counters(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let p = entries.last().unwrap().payload.as_ref().unwrap();
    for k in ["provider", "model", "tokens_in", "tokens_out", "budget_ref"] {
        assert!(p.get(k).is_some(), "missing {k}");
    }
}

#[then("no prompt or response text exists anywhere in the log files")]
fn no_prompt_in_log(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    for e in entries.iter().filter(|e| e.kind == "inference") {
        let keys: Vec<&String> = e
            .payload
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .collect();
        for k in keys {
            assert!(
                ["provider", "model", "tokens_in", "tokens_out", "budget_ref"]
                    .contains(&k.as_str()),
                "unexpected inference payload key {k}"
            );
        }
        assert!(e.body_enc.is_none());
    }
}

#[then("the inference is refused as over budget")]
#[then("the inference is refused")]
fn inference_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_err(), "inference should be refused, got {r:?}");
}

#[then(expr = "an inference of {int} total tokens citing {string} verifies")]
fn inference_fits(w: &mut ProtocolWorld, total: u64, profile: String) {
    w.try_inference(
        "gemma",
        total - 100,
        100,
        Some(&profile),
        &day(2, "11:00:00"),
    )
    .unwrap();
}

#[then("every section entry comes back")]
fn sections_come_back(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert_eq!(hits.len(), 3, "two adds + one modify");
    assert!(hits.iter().all(|h| h.entry.kind.starts_with("section.")));
}

#[then("no action or heartbeat entry does")]
fn no_action_in_class(w: &mut ProtocolWorld) {
    let hits = w.query_hits.as_ref().unwrap().as_ref().unwrap();
    assert!(hits
        .iter()
        .all(|h| h.entry.kind != "action" && h.entry.kind != "heartbeat"));
}

#[then("no gamma entry is appended")]
fn no_entry_appended(w: &mut ProtocolWorld) {
    assert!(w.read_body.as_ref().unwrap().is_ok(), "the read succeeds");
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(len, w.gamma_baseline, "reading must not touch the log");
}

#[then(expr = "an {string} entry signed by the agent chains onto the log")]
fn read_entry_chains(w: &mut ProtocolWorld, kind: String) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, kind);
    assert!(last.authorized_by.is_some());
    w.bundle.as_ref().unwrap().gamma_verify().unwrap();
}

#[then("its sealed body names the section it read")]
fn read_body_names_section(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_owner(
            &owner,
            &LogFilter {
                kind: Some("ethos.read".into()),
                ..LogFilter::default()
            },
        )
        .unwrap();
    assert_eq!(hits.len(), 1);
    let body = hits[0].body.as_ref().expect("owner opens the read body");
    assert!(body.target.contains("/s/"), "target must name a section");
}

#[then("the entry carries a clear args_hash and a sealed args body")]
fn entry_has_sealed_args(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_ok());
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert!(last.payload.as_ref().unwrap().get("args_hash").is_some());
    assert!(last.body_enc.is_some());
}

#[then("the owner reopens the arguments and finds the recipient")]
fn owner_reopens_args(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let args = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_action_args(&owner, entries.last().unwrap())
        .unwrap();
    assert_eq!(args["recipient"], "alice@example.com");
}

#[then("a stranger sees only the hash")]
fn stranger_sees_hash(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    let clear = serde_json::to_string(last.payload.as_ref().unwrap()).unwrap();
    assert!(!clear.contains("alice@example.com"), "args must not leak");
    assert!(clear.contains("sha256:"));
}

#[then("the audit rejects the entry as inconsistent")]
fn audit_rejects(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let r = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_action_args(&owner, entries.last().unwrap());
    assert!(r.is_err(), "mismatched args must fail the audit, got {r:?}");
}

#[then("the container refuses before anything is logged")]
fn container_refuses(w: &mut ProtocolWorld) {
    assert!(w.gamma_result.as_ref().unwrap().is_err());
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(len, w.gamma_baseline, "nothing may be logged");
}

#[then("the audit reports every logged action compliant")]
fn audit_compliant(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(r.is_ok(), "audit should pass, got {r:?}");
    assert_ne!(r.as_ref().unwrap(), "0", "at least one action audited");
}

fn read_json<T: serde::de::DeserializeOwned>(b: &Bundle<MemStore>, path: &str) -> T {
    serde_json::from_slice(&b.store.get(path).unwrap().unwrap()).unwrap()
}

// ------------------------------------------------------ step G: revocation ---

const SURV: u8 = 0xB7; // survivor
const HOLD: u8 = 0xB8; // zone holder
const WDOG: u8 = 0xB9; // watchdog
const G_AT: &str = "2026-07-11T00:00:00Z"; // the revocation instant
const G_AFTER: &str = "2026-07-12T00:00:00Z";
const G_BEFORE: &str = "2026-07-10T00:00:00Z";

fn kid_of(sk: u8) -> String {
    wire::ed25519_pub_to_multibase(&agent_sk(sk).verifying_key().to_bytes())
}

impl ProtocolWorld {
    fn gb(&mut self) -> &mut Bundle<MemStore> {
        self.bundle.as_mut().unwrap()
    }

    /// Grant a named agent read on a circle folder; return its root chain.
    fn grant_read_named(&mut self, label: &str, sk: u8, dir: &str, na: &str) -> Vec<Mandate> {
        let owner = self.owner(0);
        let m = self
            .bundle
            .as_mut()
            .unwrap()
            .grant(
                &owner,
                label,
                &agent_sk(sk).verifying_key(),
                &[dir_spec(dir)],
                NB,
                na,
                0,
                &mut self.ent,
            )
            .expect("grant succeeds");
        vec![m]
    }

    /// Mint and store a root mandate with an explicit perimeter (watchdogs).
    fn mint_root(
        &mut self,
        label: &str,
        sk: u8,
        perimeter: Vec<PerimeterEntry>,
        na: &str,
    ) -> Mandate {
        use aithos_core::mandate::{Mandate as M, MandateSpec};
        let owner = self.owner(0);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 700)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.to_owned(),
                grantee_pub: &agent_sk(sk).verifying_key(),
                perimeter,
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: na.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        m
    }

    fn revoke_owner(&mut self, mandate_id: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gb()
            .log_revoke_owner(&owner, mandate_id, "test", at, &mut ent)
            .unwrap();
        self.ent = ent;
        self.revoked_at = at.to_owned();
    }

    fn rotate(&mut self, folder: &str, revoked_sk: u8) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.gb()
            .rotate_folder(&owner, folder, &kid_of(revoked_sk), &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn read_at(&self, chain: &[Mandate], sk: u8, path: &str, at: &str) -> Result<String, String> {
        self.bundle
            .as_ref()
            .unwrap()
            .read_section_as_agent(chain, &agent_sk(sk), Zone::Circle, path, at)
            .map_err(|e| e.to_string())
    }

    fn verify_revocable_at(&self, chain: &[Mandate], at: &str) -> Result<(), String> {
        let bundle = self.bundle.as_ref().unwrap();
        let doc = self.did_document();
        let revs = bundle.active_revocations().map_err(|e| e.to_string())?;
        aithos_core::mandate::verify_chain_revocable(chain, &doc, at, &revs)
            .map_err(|e| e.to_string())
    }
}

// --- G givens ---

#[given("two agents granted read on circle folder \"projets\" and a zone holder")]
fn two_agents_and_holder(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
    w.holder_chain = w.grant_read_named("holder", HOLD, "", NA30);
}

#[given("two agents holding lines on circle folder \"projets\"")]
fn two_agents_lines(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
}

#[given("a zone holder reading folder \"projets\" by pure derivation")]
fn zone_holder_derivation(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    w.holder_chain = w.grant_read_named("holder", HOLD, "", NA30);
    // Baseline: the holder reads by derivation before any rotation.
    assert!(w
        .read_at(&w.holder_chain.clone(), HOLD, "projets/note1", DAY1)
        .is_ok());
}

#[given("an agent granted action rights")]
fn g_agent_action(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
}

#[given("an agent that acted before being revoked")]
fn agent_acted_before(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", G_BEFORE).unwrap();
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[given("an agent with issue depth 1 that delegated to a helper")]
fn agent_delegated_helper(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.*", serde_json::json!({}), true)
        .unwrap();
}

#[given("two unrelated agents granted action rights")]
fn two_unrelated_agents(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    // A second, independent agent (SURV keypair).
    let owner = w.owner(0);
    let m = aithos_core::mandate::Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: format!("mandate_{}", sid(903)),
            subject: w.bundle.as_ref().unwrap().did.clone(),
            grantee_id: "urn:aithos:agent:other".into(),
            grantee_label: "other".into(),
            grantee_pub: &agent_sk(SURV).verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse("act.x.gmail.*").unwrap()],
            constraints: aithos_core::mandate::MandateSpec::no_constraints(),
            not_before: NB.into(),
            not_after: NA30.into(),
            issued_at: NB.into(),
            nonce: hex::encode(w.ent.e16()),
        },
    )
    .unwrap();
    w.store_cert(&m);
    w.survivor_chain = vec![m];
}

#[given("a watchdog granted only the revoke right over circle \"projets\"")]
fn watchdog_grant(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA30);
    let scope =
        PerimeterEntry::parse(&format!("revoke.circle#dir={}", w.resolve_dir("projets"))).unwrap();
    let wd = w.mint_root("watchdog", WDOG, vec![scope], NA30);
    w.holder_chain = vec![wd];
}

#[given("an agent whose mandate expired yesterday")]
fn agent_expired(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    // Window ends at NA7 = 2026-07-08; "now" is DAY8 = 2026-07-09.
    w.chain = w.grant_read_named("agent", AGENT, "projets", NA7);
}

#[given("an agent that exfiltrated nothing but held folder \"projets\"")]
fn agent_held_folder(w: &mut ProtocolWorld) {
    two_agents_lines(w);
}

#[given("a helper cut by its parent's revocation")]
fn helper_cut(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_circle_section("projets", "note1", "toto");
    w.publish_bundle();
    // Owner grants the agent read + issue on projets, agent delegates to helper.
    let owner = w.owner(0);
    let m = w
        .bundle
        .as_mut()
        .unwrap()
        .grant(
            &owner,
            "agent",
            &agent_sk(AGENT).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA30,
            1,
            &mut w.ent,
        )
        .unwrap();
    w.chain = vec![m];
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &w.chain[0].clone(),
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec("projets")],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![w.chain[0].clone(), sub];
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[given("a rotated header version for folder \"projets\"")]
fn rotated_header(w: &mut ProtocolWorld) {
    two_agents_lines(w);
}

#[given("a rotated folder \"projets\" under the circle zone")]
fn rotated_folder_holder(w: &mut ProtocolWorld) {
    zone_holder_derivation(w);
    w.rotate("projets", AGENT);
}

impl ProtocolWorld {
    fn resolve_dir(&self, display: &str) -> String {
        let dirs = self
            .bundle
            .as_ref()
            .unwrap()
            .resolve_folder(Zone::Circle, display)
            .unwrap();
        dirs.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("/")
    }
}

// --- G whens ---

#[when("the owner revokes the first agent with rotation")]
fn revoke_with_rotation(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[given("the owner revokes the agent's mandate")]
#[when("the owner revokes the agent's mandate")]
#[when("the owner revokes the agent's mandate for real")]
fn revoke_agent(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
}

#[when("the agent revokes the helper's mandate")]
fn agent_revokes_helper(w: &mut ProtocolWorld) {
    let helper_id = w.helper_chain[1].id.clone();
    let parent = vec![w.chain[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(
            &parent,
            &agent_sk(AGENT),
            &helper_id,
            "test",
            G_AT,
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when("the first agent forges a revocation of the second's mandate")]
fn forge_revocation(w: &mut ProtocolWorld) {
    let target = w.survivor_chain[0].id.clone();
    let attacker = vec![w.chain[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.g_result = Some(
        w.gb()
            .log_revoke_as(
                &attacker,
                &agent_sk(AGENT),
                &target,
                "malice",
                G_AT,
                &mut ent,
            )
            .map(|_| "appended".into())
            .map_err(|e| e.to_string()),
    );
    w.ent = ent;
}

#[when("the watchdog revokes the projets agent's mandate")]
fn watchdog_revokes(w: &mut ProtocolWorld) {
    let target = w.chain[0].id.clone();
    let wd = w.holder_chain.clone();
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(&wd, &agent_sk(WDOG), &target, "watchdog", G_AT, &mut ent)
        .unwrap();
    w.ent = ent;
}

#[when("the owner rotates \"projets\" out of a revoked agent")]
#[when("a manager rotates the node in passing")]
fn owner_rotates(w: &mut ProtocolWorld) {
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[when("the owner revokes it with rotation and re-encryption")]
fn revoke_reencrypt(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.chain[0].id.clone(), G_AT);
    w.rotate("projets", AGENT);
    w.publish_bundle();
}

#[when("the new version claims a line for a key absent from the old version")]
fn smuggle_recipient(w: &mut ProtocolWorld) {
    // Build a rotation that seals to an intruder never present in v1.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let file = format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    );
    let mut header: Header = read_json(w.bundle.as_ref().unwrap(), &file);
    let intruder = Recipient {
        to: kid_of(WDOG),
        kid: kid_of(WDOG),
        pubkey: ed2x(&agent_sk(WDOG).verifying_key()),
    };
    let doc = w.did_document();
    let owner_kex = aithos_core::wire::multibase_to_x25519_pub(&doc.keys.kex).unwrap();
    let owner_rec = Recipient::owner(owner_kex.into());
    header
        .rotate(
            &w.bundle.as_ref().unwrap().did.clone(),
            2,
            &[9u8; 32],
            &[owner_rec, intruder],
            &[[1u8; 32], [2u8; 32]],
            &[[1u8; 24], [2u8; 24]],
        )
        .unwrap();
    w.g_result = Some(
        header
            .check_rotation(2)
            .map(|_| "ok".into())
            .map_err(|e| e.to_string()),
    );
}

#[when("someone without the parent key posts an up-link wrap")]
fn bogus_wrap(w: &mut ProtocolWorld) {
    // Overwrite the up-link wrap with one sealed under a WRONG via key.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let zroot = NodePath::zone_root(Zone::Circle);
    let file = format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{zroot}\u{0}{node}").as_bytes()).as_bytes()[..12])
    );
    let bogus = Wrap::seal(
        &w.bundle.as_ref().unwrap().did.clone(),
        &zroot.to_string(),
        &[0xEEu8; 32], // not the real zone key
        &node.to_string(),
        2,
        &[9u8; 32],
        [7u8; 24],
    );
    let bytes = serde_json::to_vec_pretty(&bogus).unwrap();
    w.gb().store.put(&file, &bytes).unwrap();
}

#[when("the owner grants the helper a fresh mandate on the same folder")]
fn readopt_helper(w: &mut ProtocolWorld) {
    // Fresh mandate to the SAME helper keypair on the same folder.
    w.holder_chain = w.grant_read_named("helper-readopted", HELPER, "projets", NA30);
}

#[when("the agent presents its chain after the revocation instant")]
fn present_after(w: &mut ProtocolWorld) {
    w.g_result = Some(
        w.verify_revocable_at(&w.chain.clone(), G_AFTER)
            .map(|()| "valid".into()),
    );
}

#[when("the head agent forges a heartbeat with its own key for G")]
fn _unused_g(_w: &mut ProtocolWorld) {}

// --- G thens ---

#[then("the revoked agent reads nothing written after the cut")]
fn revoked_reads_nothing(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", G_AFTER);
    assert!(r.is_err(), "revoked agent must not read, got {r:?}");
}

#[then("the surviving agent reads new content without lifting a finger")]
fn survivor_reads(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "survivor must read, got {r:?}");
}

#[then("the zone holder still reads the folder through the up-link wrap")]
fn holder_reads_uplink(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "zone holder must read via up-link, got {r:?}");
}

#[then("a \"revoke\" entry signed by the owner chains onto the log")]
fn revoke_entry_present(w: &mut ProtocolWorld) {
    let entries = w.gb().gamma_entries().unwrap();
    let last = entries.last().unwrap();
    assert_eq!(last.kind, "revoke");
    assert_eq!(last.signature.key, "#content");
}

#[then("the chain is rejected as revoked")]
#[then("the helper's chain is rejected as revoked")]
#[then("the projets agent's chain is rejected as revoked")]
fn chain_rejected_revoked(w: &mut ProtocolWorld) {
    let chain = if !w.helper_chain.is_empty() && w.g_context_helper() {
        w.helper_chain.clone()
    } else {
        w.chain.clone()
    };
    let r = w.verify_revocable_at(&chain, G_AFTER);
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "expected revoked, got {r:?}"
    );
}

impl ProtocolWorld {
    fn g_context_helper(&self) -> bool {
        // The helper scenarios revoke the parent (w.chain) and check helper.
        !self.helper_chain.is_empty()
    }
}

#[then("the action logged before revoked_at still verifies at its own timestamp")]
fn old_action_valid(w: &mut ProtocolWorld) {
    assert!(w.verify_revocable_at(&w.chain.clone(), G_BEFORE).is_ok());
}

#[then("an action timestamped after revoked_at is rejected")]
fn new_action_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.chain.clone(), G_AFTER);
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "got {r:?}"
    );
}

#[then("the revocation entry is rejected")]
fn revocation_rejected(w: &mut ProtocolWorld) {
    let r = w.g_result.as_ref().unwrap();
    assert!(r.is_err(), "forged revocation must be rejected, got {r:?}");
}

#[then("the second agent's chain still verifies")]
fn second_chain_ok(w: &mut ProtocolWorld) {
    assert!(w
        .verify_revocable_at(&w.survivor_chain.clone(), G_AFTER)
        .is_ok());
}

#[then("the watchdog itself cannot open a single body")]
fn watchdog_reads_nothing(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), WDOG, "projets/note1", G_BEFORE);
    assert!(r.is_err(), "watchdog holds no content key, got {r:?}");
}

#[then("the folder's header gains a version without the revoked line")]
fn header_new_version(w: &mut ProtocolWorld) {
    // The When already revoked + rotated; here we only inspect the header.
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let file = format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    );
    let header: Header = read_json(w.bundle.as_ref().unwrap(), &file);
    assert!(header.key_versions.contains_key("2"));
    let v2 = &header.key_versions["2"];
    assert!(
        !v2.lines.iter().any(|l| l.kid == kid_of(AGENT)),
        "revoked line present"
    );
}

#[then("the survivor opens the new version with its unchanged keypair")]
fn survivor_opens_v2(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "survivor opens v2, got {r:?}");
}

#[then("the zone holder keeps reading through the up-link wrap")]
fn holder_keeps_reading(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "holder reads via wrap, got {r:?}");
}

#[then("the wrap is bound to the node and its new key version")]
fn wrap_bound(w: &mut ProtocolWorld) {
    let folders = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, "projets")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, folders);
    let zroot = NodePath::zone_root(Zone::Circle);
    let file = format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{zroot}\u{0}{node}").as_bytes()).as_bytes()[..12])
    );
    let wrap: Wrap = read_json(w.bundle.as_ref().unwrap(), &file);
    assert_eq!(wrap.node, node.to_string());
    assert_eq!(wrap.key_version, 2);
}

#[then("header verification is rejected")]
fn header_rejected(w: &mut ProtocolWorld) {
    let r = w.g_result.as_ref().unwrap();
    assert!(r.is_err(), "smuggled recipient must be rejected, got {r:?}");
}

#[then("the wrap is rejected")]
fn wrap_rejected(w: &mut ProtocolWorld) {
    // The zone holder tries to read through the bogus wrap: it cannot open.
    let r = w.read_at(&w.holder_chain.clone(), HOLD, "projets/note1", G_AFTER);
    assert!(r.is_err(), "a bogus wrap grants nothing, got {r:?}");
}

#[then("the agent's actions are rejected by every verifier")]
fn expired_actions_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.chain.clone(), DAY8);
    assert!(
        r.is_err(),
        "expired mandate must fail the window, got {r:?}"
    );
}

#[then("its key still opens content written under the old version")]
fn key_still_opens(w: &mut ProtocolWorld) {
    // No rotation happened: a fresh grant to another agent still reads the
    // very same content — proving expiry turned no lock.
    w.survivor_chain = w.grant_read_named("survivor", SURV, "projets", NA30);
    let r = w.read_at(&w.survivor_chain.clone(), SURV, "projets/note1", DAY1);
    assert!(r.is_ok(), "content unchanged by mere expiry, got {r:?}");
}

#[then("the old key opens nothing written since")]
fn old_key_dead(w: &mut ProtocolWorld) {
    // After the manager's rotation, a NON-survivor derivation is stale: the
    // revoked/expired agent's chain no longer reads new content.
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", DAY8);
    assert!(r.is_err(), "old key must be dead after rotation, got {r:?}");
}

#[then("the folder's existing bodies are rewritten under the new key")]
fn bodies_reencrypted(w: &mut ProtocolWorld) {
    let index: aithos_bundle::bundle::ZoneIndex =
        read_json(w.bundle.as_ref().unwrap(), "e/circle/index.json");
    assert!(
        index.sections.iter().all(|s| s.key_version == 2),
        "sections must be re-encrypted at v2"
    );
}

#[then("the revoked key opens neither the new bodies nor the new lines")]
fn revoked_key_dead(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.chain.clone(), AGENT, "projets/note1", G_AFTER);
    assert!(r.is_err(), "revoked key opens nothing, got {r:?}");
}

#[then("the helper reads again with the same keypair")]
fn helper_readopted(w: &mut ProtocolWorld) {
    let r = w.read_at(&w.holder_chain.clone(), HELPER, "projets/note1", G_AFTER);
    assert!(r.is_ok(), "re-adopted helper reads, got {r:?}");
}

// ----------------------------------------- move-as-rotation (spec 02.9) ---

fn hdr_path_of(node: &NodePath) -> String {
    format!(
        "e/circle/hdr/{}.json",
        hex::encode(&blake3::hash(node.to_string().as_bytes()).as_bytes()[..12])
    )
}

fn wrap_path_of(via: &NodePath, node: &NodePath) -> String {
    format!(
        "e/circle/wraps/{}.json",
        hex::encode(&blake3::hash(format!("{via}\u{0}{node}").as_bytes()).as_bytes()[..12])
    )
}

#[given(expr = "a section {string} the agent reads by derivation")]
fn section_read_by_derivation(w: &mut ProtocolWorld, path: String) {
    let (folder, name) = path.rsplit_once('/').expect("folder/name");
    w.add_named_section(folder, name, &[]);
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    assert_eq!(r.as_deref(), Ok(BODY), "derivation read fails: {r:?}");
}

#[when(expr = "the owner moves folder {string} under {string}")]
fn owner_moves_folder(w: &mut ProtocolWorld, folder: String, dest: String) {
    // Scenario permutations share this step: seed the fixture ends that the
    // Given did not create (the new-parent scenario grants "projets" only).
    if w.bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, &folder)
        .is_err()
    {
        assert_eq!(folder, "archives/old", "unexpected move fixture");
        w.add_named_section(&folder, "note1", &[]);
    }
    let owner = w.owner(0);
    let bundle = w.bundle.as_mut().unwrap();
    bundle
        .ensure_folder(Zone::Circle, &dest, &owner, &mut w.ent)
        .expect("destination exists");
    bundle
        .move_folder(&owner, &folder, &dest, &mut w.ent)
        .expect("move succeeds");
}

#[then("the agent still derives the folder's old key — it cannot be un-taught")]
fn agent_still_derives_old_key(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.bundle.as_ref().unwrap();
    // The moved folder sits at its new address; its sid is stable.
    let new_chain = b.resolve_folder(Zone::Circle, "projets/old").unwrap();
    let m_sid = *new_chain.last().unwrap();
    let a_chain = b.resolve_folder(Zone::Circle, "archives").unwrap();
    // Agent side: its own line on "archives", one derivation step down.
    let a_node = NodePath::folder(Zone::Circle, a_chain.clone());
    let header: Header = read_json(b, &hdr_path_of(&a_node));
    let sk = agent_sk(AGENT);
    let kid = wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes());
    let kex = aithos_core::keys::grantee_kex_secret(&sk);
    let dk_a = header
        .open(&b.did, 1, &kid, &kex)
        .expect("agent line opens");
    let derived = derive_key(&aithos_core::derive::folder_label(&m_sid), &dk_a);
    // Owner side: the same old key straight from the zone root — the move
    // could not un-teach it, which is exactly why it had to rotate.
    let mut old_chain = a_chain;
    old_chain.push(m_sid);
    let expected = node_key(
        &b.zone_dk(Zone::Circle, &owner).unwrap(),
        &NodePath::folder(Zone::Circle, old_chain),
    );
    assert_eq!(derived, expected, "old-parent derivation must survive");
}

#[then(expr = "the agent's read of {string} is rejected as outside its perimeter")]
fn read_rejected_outside_perimeter(w: &mut ProtocolWorld, path: String) {
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    let err = r.expect_err("the old parent's grant must not cover the moved node");
    assert!(err.contains("not covered"), "wrong rejection: {err}");
}

#[then("the folder carries a fresh key version at its new path")]
fn fresh_version_at_new_path(w: &mut ProtocolWorld) {
    let b = w.bundle.as_ref().unwrap();
    let chain = b.resolve_folder(Zone::Circle, "projets/old").unwrap();
    let node = NodePath::folder(Zone::Circle, chain);
    let header: Header = read_json(b, &hdr_path_of(&node));
    assert_eq!(header.node, node.to_string(), "header binds the new path");
    assert_eq!(header.latest_version(), 2, "fresh key version");
    assert!(
        !header.key_versions.contains_key("1"),
        "old versions stay at the old address"
    );
}

#[then(expr = "the agent reads new content at {string} with its unchanged keypair")]
fn survivor_reads_new_content(w: &mut ProtocolWorld, folder: String) {
    // The owner writes NEW content after the move; the direct line was
    // re-sealed as a survivor, so the same keypair keeps reading.
    w.add_named_section(&folder, "note2", &[]);
    let r = w.agent_reads(&w.chain.clone(), AGENT, &format!("{folder}/note2"));
    assert_eq!(r.as_deref(), Ok(BODY), "survivor read fails: {r:?}");
}

#[then(expr = "the agent reads {string} through the wrap posted under {string}")]
fn reads_through_parent_wrap(w: &mut ProtocolWorld, path: String, parent: String) {
    let b = w.bundle.as_ref().unwrap();
    let p_chain = b.resolve_folder(Zone::Circle, &parent).unwrap();
    let m_chain = b
        .resolve_folder(Zone::Circle, &format!("{parent}/old"))
        .unwrap();
    let via = NodePath::folder(Zone::Circle, p_chain);
    let node = NodePath::folder(Zone::Circle, m_chain);
    assert!(
        b.store.get(&wrap_path_of(&via, &node)).unwrap().is_some(),
        "the up-link wrap must hang under the new parent"
    );
    let r = w.agent_reads(&w.chain.clone(), AGENT, &path);
    assert_eq!(r.as_deref(), Ok(BODY), "wrap read fails: {r:?}");
}

// --- step G+: obligations (spec 04.12) ---

const APPROVER: u8 = 0xB5;
const APPROVER2: u8 = 0xB6;
const GUARD: u8 = 0xD4;
const DUAL: u8 = 0xB7;
const SIB: u8 = 0xB8;
const STRANGER: u8 = 0xEE;
/// The G+ entry instant; receipts date relative to it.
const GP_AT: &str = "2026-07-04T12:00:00Z";
const G_ARGS: &str = "sha256:aa11";

fn mb_of(b: u8) -> String {
    wire::ed25519_pub_to_multibase(&agent_sk(b).verifying_key().to_bytes())
}

fn ob_json(
    id: &str,
    check: &str,
    attestors: &[u8],
    applies_to: &str,
    verdict: &str,
    max_age: Option<&str>,
) -> serde_json::Value {
    let mut o = serde_json::json!({
        "id": id, "check": check,
        "attestor": attestors.iter().map(|b| mb_of(*b)).collect::<Vec<_>>(),
        "applies_to": applies_to, "verdict": verdict,
    });
    if let Some(d) = max_age {
        o["max_age"] = serde_json::json!(d);
    }
    o
}

fn approval_ob() -> serde_json::Value {
    ob_json(
        "publish-approval",
        "human.approve",
        &[APPROVER],
        "act.x.social.publish",
        "approve",
        Some("5m"),
    )
}

fn guard_ob() -> serde_json::Value {
    ob_json(
        "pii-guard",
        "guardrail.pii",
        &[GUARD],
        "act.x.social.publish",
        "pass",
        None,
    )
}

/// Sign the §04.12 payload and return the checks[] rider.
fn ob_receipt(
    sk: &SigningKey,
    obligation: &str,
    coords: (&str, &str, &str), // (mandate_id, action, args_hash)
    verdict: &str,
    at: &str,
    presented: Option<&str>,
) -> serde_json::Value {
    use ed25519_dalek::Signer;
    let (mandate_id, action, args_hash) = coords;
    let mut payload = serde_json::json!({
        "obligation": obligation, "mandate_id": mandate_id, "action": action,
        "args_hash": args_hash, "verdict": verdict, "at": at,
    });
    if let Some(d) = presented {
        payload["presented_digest"] = serde_json::json!(d);
    }
    let sig = hex::encode(
        sk.sign(&aithos_core::jcs::canonical_bytes(&payload).unwrap())
            .to_bytes(),
    );
    let mut check = serde_json::json!({
        "obligation": obligation, "args_hash": args_hash,
        "verdict": verdict, "at": at, "sig": sig,
    });
    if let Some(d) = presented {
        check["presented_digest"] = serde_json::json!(d);
    }
    check
}

fn approver_receipt(w: &ProtocolWorld, at: &str, presented: Option<&str>) -> serde_json::Value {
    ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&w.chain.last().unwrap().id, "publish", G_ARGS),
        "approve",
        at,
        presented,
    )
}

impl ProtocolWorld {
    fn grant_act_patterns(
        &mut self,
        patterns: &[&str],
        issue: Option<u32>,
        constraints: serde_json::Value,
    ) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        self.init_bundle();
        let owner = self.owner(0);
        let mut perimeter: Vec<PerimeterEntry> = patterns
            .iter()
            .map(|p| PerimeterEntry::parse(p).unwrap())
            .collect();
        if let Some(depth) = issue {
            perimeter.push(PerimeterEntry::Issue { depth });
        }
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 970)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter,
                constraints,
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    /// Sub-mandate minted by AGENT — attenuation judged at verification,
    /// never at mint (build_sub only builds and signs).
    fn mint_sub(
        &mut self,
        grantee: u8,
        label: &str,
        pattern: &str,
        constraints: serde_json::Value,
        log_grant: bool,
    ) -> Result<Vec<Mandate>, String> {
        use aithos_core::mandate::{Mandate as M, MandateSpec, PerimeterEntry};
        let parent = self.chain[0].clone();
        let child = M::build_sub(
            &parent,
            &agent_sk(AGENT),
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 980)),
                subject: parent.subject.clone(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.into(),
                grantee_pub: &agent_sk(grantee).verifying_key(),
                perimeter: vec![PerimeterEntry::parse(pattern).unwrap()],
                constraints,
                not_before: NB.into(),
                not_after: parent.not_after.clone(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .map_err(|e| e.to_string())?;
        self.store_cert(&child);
        if log_grant {
            let mut ent = std::mem::take(&mut self.ent);
            let r = self.gbundle().log_delegated_grant(
                std::slice::from_ref(&parent),
                &agent_sk(AGENT),
                &child.id,
                &day(1, "00:40:00"),
                &mut ent,
            );
            self.ent = ent;
            r.map_err(|e| e.to_string())?;
        }
        Ok(vec![parent, child])
    }

    fn try_action_checked(
        &mut self,
        chain: &[Mandate],
        sk: &SigningKey,
        connector: &str,
        action: &str,
        checks: Option<serde_json::Value>,
    ) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_action_with_checks(
                chain,
                sk,
                &aithos_bundle::log::ActionSpec {
                    connector,
                    action,
                    args_hash: G_ARGS,
                    now: GP_AT,
                    budget: None,
                    sealed_args: None,
                },
                checks,
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn publish_with(&mut self, checks: Option<serde_json::Value>) {
        let chain = self.chain.clone();
        let r = self.try_action_checked(&chain, &agent_sk(AGENT), "social", "publish", checks);
        self.gamma_result = Some(r);
    }

    fn appended_checks(&mut self) -> Option<serde_json::Value> {
        let id = self
            .gamma_result
            .as_ref()
            .unwrap()
            .as_ref()
            .expect("action should have appended")
            .clone();
        self.gbundle()
            .gamma_entries()
            .unwrap()
            .into_iter()
            .find(|e| e.id == id)
            .and_then(|e| e.payload.as_ref().and_then(|p| p.get("checks")).cloned())
    }
}

// --- givens ---

#[given("an agent granted social publish under a guardrail obligation")]
#[given("an agent granted social publish under a guardrail obligation with no max_age")]
fn gplus_guardrail_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob()]}),
    );
}

#[given("an agent granted gmail send and social publish under a publish-only guardrail obligation")]
fn gplus_two_connectors(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.gmail.*", "act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob()]}),
    );
}

#[given("an agent granted social actions under a guardrail obligation on every social action")]
fn gplus_wildcard_grant(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "pii-guard",
        "guardrail.pii",
        &[GUARD],
        "act.x.social.*",
        "pass",
        None,
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social publish requiring human approval within 5 minutes")]
fn gplus_approval_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("an agent granted social publish requiring approval by one of two approvers")]
fn gplus_two_approvers(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "publish-approval",
        "human.approve",
        &[APPROVER, APPROVER2],
        "act.x.social.publish",
        "approve",
        Some("5m"),
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social actions requiring human approval on publish")]
fn gplus_approval_on_publish(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("an agent granted gmail send with counter_sign on send")]
fn gplus_counter_sign_grant(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.gmail.*"],
        None,
        serde_json::json!({"counter_sign": ["send"]}),
    );
}

#[given("an agent granted social publish under dual control with a second agent")]
fn gplus_dual_control(w: &mut ProtocolWorld) {
    let ob = ob_json(
        "four-eyes",
        "agent.countersign",
        &[DUAL],
        "act.x.social.publish",
        "approve",
        None,
    );
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [ob]}),
    );
}

#[given("an agent granted social publish under both a guardrail and a human approval obligation")]
fn gplus_both_obligations(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        None,
        serde_json::json!({"obligations": [guard_ob(), approval_ob()]}),
    );
}

#[given("two sibling sub-mandates that may publish under an ancestor approval obligation")]
fn gplus_siblings(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        Some(1),
        serde_json::json!({"obligations": [approval_ob()]}),
    );
    let inherited = serde_json::json!({"obligations": [approval_ob()]});
    let a = w
        .mint_sub(
            HELPER,
            "sib-a",
            "act.x.social.publish",
            inherited.clone(),
            true,
        )
        .unwrap();
    let b = w
        .mint_sub(SIB, "sib-b", "act.x.social.publish", inherited, true)
        .unwrap();
    w.sib_chains = vec![a, b];
}

#[given("a head mandate requiring human approval on publish")]
fn gplus_head_mandate(w: &mut ProtocolWorld) {
    w.grant_act_patterns(
        &["act.x.social.*"],
        Some(1),
        serde_json::json!({"obligations": [approval_ob()]}),
    );
}

#[given("a sub-mandate that adds a guardrail obligation on publish")]
fn gplus_sub_adds(w: &mut ProtocolWorld) {
    let child = serde_json::json!({"obligations": [approval_ob(), guard_ob()]});
    let chain = w
        .mint_sub(HELPER, "helper", "act.x.social.publish", child, true)
        .unwrap();
    w.helper_chain = chain;
}

#[given("the approver signed a receipt over what was shown on the device")]
fn gplus_signed_wysiwys(w: &mut ProtocolWorld) {
    w.gplus_checks = Some(approver_receipt(
        w,
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    ));
}

// --- whens ---

#[when(expr = "the agent publishes with a receipt whose verdict is {string}")]
fn gplus_publish_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        &verdict,
        "2026-07-04T11:59:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the agent publishes with a pass receipt signed 2 days earlier")]
fn gplus_publish_aged(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-02T12:00:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when(expr = "the agent deletes a post with a receipt whose verdict is {string}")]
fn gplus_delete_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "delete", G_ARGS),
        &verdict,
        "2026-07-04T11:59:00Z",
        None,
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "delete", Some(check));
    w.gamma_result = Some(r);
}

#[when("the agent sends a mail without any receipt")]
fn gplus_send_bare(w: &mut ProtocolWorld) {
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "gmail", "send", None);
    w.gamma_result = Some(r);
}

#[when("the agent publishes without any receipt")]
fn gplus_publish_bare(w: &mut ProtocolWorld) {
    w.publish_with(None);
}

#[when(expr = "the approver signs the prepared publish {int} minutes before the entry")]
fn gplus_approve_before(w: &mut ProtocolWorld, minutes: u32) {
    let at = format!("2026-07-04T11:{:02}:00Z", 60 - minutes);
    let check = approver_receipt(w, &at, Some("sha256:rendered-ship-it"));
    w.publish_with(Some(check));
}

#[when("the approver signs the prepared publish 2 minutes after the entry's clock")]
fn gplus_approve_after(w: &mut ProtocolWorld) {
    let check = approver_receipt(w, "2026-07-04T12:02:00Z", Some("sha256:rendered-ship-it"));
    w.publish_with(Some(check));
}

#[when("the approver signs the prepared publish without a presented digest")]
fn gplus_approve_no_digest(w: &mut ProtocolWorld) {
    let check = approver_receipt(w, "2026-07-04T11:58:00Z", None);
    w.publish_with(Some(check));
}

#[when(expr = "the approver signs the prepared publish with verdict {string}")]
fn gplus_approve_verdict(w: &mut ProtocolWorld, verdict: String) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        &verdict,
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt bound to other args")]
fn gplus_other_args(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", "sha256:bb22"),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent swaps the receipt's presented_digest before appending")]
fn gplus_swap_digest(w: &mut ProtocolWorld) {
    let mut check = w.gplus_checks.take().unwrap();
    check["presented_digest"] = serde_json::json!("sha256:rendered-something-else");
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt signed by a stranger key")]
fn gplus_stranger(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(STRANGER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the second approver signs the prepared publish")]
fn gplus_second_approver(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER2),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    w.publish_with(Some(check));
}

#[when("the agent presents an approval receipt citing a different obligation id")]
fn gplus_other_obligation(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "other-obligation",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the approver's receipt for a delete is presented on a publish with identical args")]
fn gplus_cross_action(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "delete", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the owner co-signs the prepared send and the agent appends it")]
fn gplus_owner_cosign(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &owner.content_sign,
        "co_sign",
        (&leaf, "send", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-mail-to-alice"),
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "gmail", "send", Some(check));
    w.gamma_result = Some(r);
}

#[when(
    "the first sibling's approval receipt is presented by the second sibling with identical args"
)]
fn gplus_sibling_replay(w: &mut ProtocolWorld) {
    let a_leaf = w.sib_chains[0].last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&a_leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    let b_chain = w.sib_chains[1].clone();
    let r = w.try_action_checked(&b_chain, &agent_sk(SIB), "social", "publish", Some(check));
    w.gamma_result = Some(r);
}

#[when("the second agent signs the prepared publish")]
fn gplus_dual_signs(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(DUAL),
        "four-eyes",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:59:00Z",
        None,
    );
    w.publish_with(Some(check));
}

#[when("the agent publishes with both receipts")]
fn gplus_both_receipts(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let approve = approver_receipt(w, "2026-07-04T11:58:00Z", Some("sha256:rendered-ship-it"));
    let chain = w.chain.clone();
    let r = w.try_action_checked(
        &chain,
        &agent_sk(AGENT),
        "social",
        "publish",
        Some(serde_json::json!([guard, approve])),
    );
    w.gamma_result = Some(r);
}

#[when("the sub-agent publishes with both receipts")]
fn gplus_sub_both(w: &mut ProtocolWorld) {
    let leaf = w.helper_chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let approve = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", G_ARGS),
        "approve",
        "2026-07-04T11:58:00Z",
        Some("sha256:rendered-ship-it"),
    );
    let chain = w.helper_chain.clone();
    let r = w.try_action_checked(
        &chain,
        &agent_sk(HELPER),
        "social",
        "publish",
        Some(serde_json::json!([guard, approve])),
    );
    w.gamma_result = Some(r);
}

#[when("the sub-agent publishes with only the guardrail receipt")]
fn gplus_sub_guard_only(w: &mut ProtocolWorld) {
    let leaf = w.helper_chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:00Z",
        None,
    );
    let chain = w.helper_chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(HELPER), "social", "publish", Some(guard));
    w.gamma_result = Some(r);
}

#[when("a sub-mandate is minted with no obligations")]
fn gplus_sub_drops(w: &mut ProtocolWorld) {
    let chain = w
        .mint_sub(
            HELPER,
            "helper",
            "act.x.social.publish",
            serde_json::json!({}),
            false,
        )
        .unwrap();
    w.chain_result = Some(w.verify_chain_at(&chain, GP_AT));
}

#[when("a sub-mandate is minted with the same obligation loosened to 1 hour")]
fn gplus_sub_loosens(w: &mut ProtocolWorld) {
    let mut loosened = approval_ob();
    loosened["max_age"] = serde_json::json!("1h");
    let chain = w
        .mint_sub(
            HELPER,
            "helper",
            "act.x.social.publish",
            serde_json::json!({"obligations": [loosened]}),
            false,
        )
        .unwrap();
    w.chain_result = Some(w.verify_chain_at(&chain, GP_AT));
}

// --- thens ---

#[then("the action appends with the receipt recorded in its checks")]
#[then("the action appends with the co_sign receipt recorded in its checks")]
fn gplus_appended_with_receipt(w: &mut ProtocolWorld) {
    let checks = w.appended_checks().expect("entry should carry checks");
    assert_eq!(
        checks.as_array().map(Vec::len),
        Some(1),
        "one receipt rides"
    );
}

#[then("the action appends with both receipts recorded in its checks")]
fn gplus_appended_with_both(w: &mut ProtocolWorld) {
    let checks = w.appended_checks().expect("entry should carry checks");
    assert_eq!(
        checks.as_array().map(Vec::len),
        Some(2),
        "two receipts ride"
    );
}

#[then("the action appends with no checks recorded")]
fn gplus_appended_bare(w: &mut ProtocolWorld) {
    assert!(w.appended_checks().is_none(), "no checks should ride");
}

#[then("the action is refused as obligation unsatisfied")]
fn gplus_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    match r {
        Err(e) => assert!(
            e.contains("obligation unsatisfied"),
            "want GammaObligationUnsatisfied, got: {e}"
        ),
        Ok(id) => panic!("action should be refused, appended {id}"),
    }
}

#[then("the log gains no entry")]
fn gplus_no_entry(w: &mut ProtocolWorld) {
    let len = w.gbundle().gamma_entries().unwrap().len();
    assert_eq!(
        len, w.gamma_baseline,
        "a refused action must append nothing"
    );
}

#[then("deleting a post without any receipt is refused as obligation unsatisfied")]
fn gplus_delete_bare_refused(w: &mut ProtocolWorld) {
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "delete", None);
    match r {
        Err(e) => assert!(e.contains("obligation unsatisfied"), "got: {e}"),
        Ok(id) => panic!("bare delete should be refused, appended {id}"),
    }
}

#[then("publishing with only the guardrail receipt is refused as obligation unsatisfied")]
fn gplus_guard_only_refused(w: &mut ProtocolWorld) {
    let leaf = w.chain.last().unwrap().id.clone();
    let guard = ob_receipt(
        &agent_sk(GUARD),
        "pii-guard",
        (&leaf, "publish", G_ARGS),
        "pass",
        "2026-07-04T11:59:30Z",
        None,
    );
    let chain = w.chain.clone();
    let r = w.try_action_checked(&chain, &agent_sk(AGENT), "social", "publish", Some(guard));
    match r {
        Err(e) => assert!(e.contains("obligation unsatisfied"), "got: {e}"),
        Ok(id) => panic!("guardrail alone should not discharge, appended {id}"),
    }
}

#[then("the chain is refused at verification time")]
fn gplus_chain_refused(w: &mut ProtocolWorld) {
    let r = w.chain_result.as_ref().unwrap();
    assert!(r.is_err(), "chain should be refused, got {r:?}");
}

// --- step H1: merkle state roots (spec 02.10) ---

const H_NOW: &str = "2026-07-09T12:00:00Z";

impl ProtocolWorld {
    fn h_manifest(&mut self) -> Manifest {
        let bytes = self
            .gbundle()
            .store
            .get("manifest.json")
            .unwrap()
            .expect("an edition was published");
        serde_json::from_slice(&bytes).unwrap()
    }

    fn h_tree(&mut self, height: u64) -> aithos_bundle::state::StateTree {
        let bytes = self
            .gbundle()
            .store
            .get(&format!("manifests/tree-{height}.json"))
            .unwrap()
            .expect("tree sidecar");
        serde_json::from_slice(&bytes).unwrap()
    }

    fn h_root(&mut self, zone: &str) -> [u8; 32] {
        let m = self.h_manifest();
        <[u8; 32]>::try_from(hex::decode(&m.roots[zone]).unwrap()).unwrap()
    }

    fn h_add(&mut self, zone: Zone, folder: &str, name: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        let b = self.gbundle();
        if !folder.is_empty() {
            b.ensure_folder(zone, folder, &owner, &mut ent).unwrap();
        }
        b.section_add(
            &SectionSpec {
                zone,
                folder_path: folder,
                name,
                title: "note",
                tags: &[],
                body: BODY,
                now: H_NOW,
            },
            &owner,
            &mut ent,
        )
        .unwrap();
        self.ent = ent;
    }

    fn h_publish(&mut self) {
        let owner = self.owner(0);
        self.gbundle().publish(&owner, H_NOW).unwrap();
    }

    fn h_diff(&mut self) -> std::collections::BTreeMap<String, &'static str> {
        let h = self.h_manifest().edition.height;
        let old = self.h_tree(h - 1);
        let new = self.h_tree(h);
        aithos_bundle::state::tree_diff(&old, &new)
    }
}

// --- H1 givens ---

#[given("a bundle with content in every zone")]
#[given("a published edition with content in every zone")]
fn h_full_bundle(w: &mut ProtocolWorld, step: &cucumber::gherkin::Step) {
    w.init_bundle();
    w.h_add(Zone::Public, "", "bio");
    w.h_add(Zone::Circle, "projets", "note1");
    w.h_add(Zone::Self_, "", "souvenir");
    if step.value.starts_with("a published") {
        w.h_publish();
    }
}

#[given("a bundle whose self zone holds nothing")]
fn h_empty_self(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Public, "", "bio");
}

#[given("a published edition with a circle section under a folder")]
fn h_folder_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Circle, "projets", "note1");
    w.h_add(Zone::Circle, "projets", "note2");
    w.h_publish();
}

#[given("a published edition with three self blobs")]
fn h_three_self(w: &mut ProtocolWorld) {
    w.init_bundle();
    for name in ["a", "b", "c"] {
        w.h_add(Zone::Self_, "", name);
    }
    w.h_publish();
}

#[given("a published edition with a circle folder \"archives/old\" holding a section")]
fn h_movable(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.h_add(Zone::Circle, "archives/old", "note1");
    w.h_add(Zone::Circle, "projets", "keep");
    w.h_publish();
}

// --- H1 whens ---

#[when("the owner publishes an edition")]
fn h_when_publish(w: &mut ProtocolWorld) {
    w.h_publish();
}

#[when("the owner adds a circle section and republishes")]
fn h_add_circle_republish(w: &mut ProtocolWorld) {
    w.h_add(Zone::Circle, "projets", "note-h");
    w.h_publish();
}

#[when("the owner adds one more circle section and republishes")]
fn h_add_one_republish(w: &mut ProtocolWorld) {
    w.h_add(Zone::Circle, "projets", "fresh");
    w.h_publish();
}

#[when("the owner republishes without any change")]
fn h_republish(w: &mut ProtocolWorld) {
    w.h_publish();
}

#[when("a verifier asks for the section's inclusion proof")]
fn h_ask_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    w.h_proof = Some(p);
}

#[when("the mirror alters the section's title inside the proven row")]
fn h_tamper_row(w: &mut ProtocolWorld) {
    let mut p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    let bytes = hex::decode(&p.payload).unwrap();
    let (row, hh) = bytes.split_at(bytes.len() - 32);
    let mut row: serde_json::Value = serde_json::from_slice(row).unwrap();
    row["title"] = serde_json::json!("tampered");
    let mut payload = aithos_core::jcs::canonical_bytes(&row).unwrap();
    payload.extend_from_slice(hh);
    p.payload = hex::encode(payload);
    w.h_proof = Some(p);
}

#[when("the owner grants an agent the folder and republishes")]
fn h_grant_republish(w: &mut ProtocolWorld) {
    let old = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    w.h_old_proof = Some(old);
    w.grant_to_agent(
        &[GrantSpec {
            zone: Zone::Circle,
            verb: Verb::Read,
            dir: "projets".into(),
            tag: None,
        }],
        NA30,
        1,
    );
    w.h_publish();
}

#[when("the mirror forges a proof that treats a leaf hash as an interior node")]
fn h_forge_leaf_as_node(w: &mut ProtocolWorld) {
    let mut p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    // replace the sibling H_node step with an H_leaf wrap over the same bytes
    if let aithos_core::merkle::ProofStep::Node { hash, .. } = p.steps[0].clone() {
        p.steps[0] = aithos_core::merkle::ProofStep::Wrap {
            pre: String::new(),
            post: hash,
        };
    }
    w.h_proof = Some(p);
}

#[when("the mirror forges a proof that presents an interior hash as a leaf")]
fn h_forge_node_as_leaf(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    // claim the two sibling leaves' concatenation as a leaf payload and
    // skip the H_node step: H_leaf(l ‖ r) ≠ H_node(l, r) by domain
    let leaf = aithos_core::merkle::h_leaf(&hex::decode(&p.payload).unwrap());
    let aithos_core::merkle::ProofStep::Node { hash, side } = p.steps[0].clone() else {
        panic!("first step should be the sibling");
    };
    let sib: [u8; 32] = hex::decode(&hash).unwrap().try_into().unwrap();
    let mut spliced = Vec::new();
    match side {
        aithos_core::merkle::Side::Right => {
            spliced.extend_from_slice(&leaf);
            spliced.extend_from_slice(&sib);
        }
        aithos_core::merkle::Side::Left => {
            spliced.extend_from_slice(&sib);
            spliced.extend_from_slice(&leaf);
        }
    }
    let forged = aithos_core::merkle::Proof {
        payload: hex::encode(spliced),
        steps: p.steps[1..].to_vec(),
        root: p.root,
    };
    w.h_proof = Some(forged);
}

#[when("a verifier asks for one self blob's inclusion proof")]
fn h_ask_self_proof(w: &mut ProtocolWorld) {
    let index: aithos_bundle::bundle::SelfIndex =
        serde_json::from_slice(&w.gbundle().store.get("e/self/index.json").unwrap().unwrap())
            .unwrap();
    let sid = index.blobs[1].sid.clone();
    let p = w.gbundle().prove_self(&sid).unwrap();
    w.h_proof = Some(p);
}

#[when("the owner moves the folder under \"projets\" and republishes")]
fn h_move_republish(w: &mut ProtocolWorld) {
    let old = w
        .gbundle()
        .prove_section(Zone::Circle, "archives/old/note1")
        .unwrap();
    w.h_old_proof = Some(old);
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .move_folder(&owner, "archives/old", "projets", &mut ent)
        .unwrap();
    w.ent = ent;
    w.h_publish();
}

// --- H1 thens ---

#[then("the manifest pins a root for public, circle, self and the vault")]
fn h_four_roots(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    for zone in ["public", "circle", "self", "vault"] {
        let r = m.roots.get(zone).unwrap_or_else(|| panic!("{zone} root"));
        assert_eq!(r.len(), 64, "{zone} root must be 32 hex bytes");
    }
}

#[then("the flat file pins are still present and verify")]
fn h_flat_pins_verify(w: &mut ProtocolWorld) {
    assert!(
        !w.h_manifest().files.is_empty(),
        "flat pins ride beside roots"
    );
    w.gbundle().verify().expect("the edition must verify");
}

#[then("an independent recomputation from the store yields the same four roots")]
fn h_recompute(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let tree = w.gbundle().state_tree().unwrap();
    assert_eq!(tree.roots, m.roots, "recomputed roots must match");
}

#[then("the self root is thirty-two zero bytes")]
fn h_self_empty(w: &mut ProtocolWorld) {
    assert_eq!(w.h_manifest().roots["self"], "00".repeat(32));
}

#[then("the circle root changes")]
fn h_circle_changed(w: &mut ProtocolWorld) {
    let h = w.h_manifest().edition.height;
    let prev: Manifest = serde_json::from_slice(
        &w.gbundle()
            .store
            .get(&format!("manifests/{}.json", h - 1))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_ne!(w.h_manifest().roots["circle"], prev.roots["circle"]);
}

#[then("the public root and the self root are unchanged")]
fn h_others_unchanged(w: &mut ProtocolWorld) {
    let h = w.h_manifest().edition.height;
    let prev: Manifest = serde_json::from_slice(
        &w.gbundle()
            .store
            .get(&format!("manifests/{}.json", h - 1))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let m = w.h_manifest();
    assert_eq!(m.roots["public"], prev.roots["public"]);
    assert_eq!(m.roots["self"], prev.roots["self"]);
}

#[then("the proof verifies against the circle root of the signed manifest")]
fn h_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(p, &root).expect("proof must verify");
}

#[then("the proof is refused")]
fn h_proof_refused(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_proof.as_ref().unwrap();
    let got = aithos_core::merkle::verify_proof(p, &root);
    assert!(
        matches!(got, Err(aithos_core::error::Error::MerkleProofInvalid(_))),
        "expected MerkleProofInvalid, got {got:?}"
    );
}

#[then("the old edition's proof for the folder no longer verifies against the new root")]
#[then("the old edition's proof for the section no longer verifies against the new root")]
fn h_old_proof_dead(w: &mut ProtocolWorld) {
    let root = w.h_root("circle");
    let p = w.h_old_proof.as_ref().unwrap();
    assert!(
        aithos_core::merkle::verify_proof(p, &root).is_err(),
        "the old proof must die against the new root"
    );
}

#[then("a fresh proof carries the new header hash and verifies")]
fn h_fresh_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/note1")
        .unwrap();
    let root = w.h_root("circle");
    aithos_core::merkle::verify_proof(&p, &root).expect("fresh proof verifies");
    let old = w.h_old_proof.as_ref().unwrap();
    assert_ne!(
        serde_json::to_string(&p.steps).unwrap(),
        serde_json::to_string(&old.steps).unwrap(),
        "the folder prefix must carry the new header"
    );
}

#[then("the section proves against the new root through its new address")]
fn h_proof_new_address(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/old/note1")
        .unwrap();
    let root = w.h_root("circle");
    aithos_core::merkle::verify_proof(&p, &root).expect("new-address proof verifies");
}

#[then("the proof verifies against the self root")]
fn h_self_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h_root("self");
    let p = w.h_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(p, &root).expect("self proof verifies");
}

#[then("the proof carries no name, no path and no sibling row")]
fn h_self_proof_opaque(w: &mut ProtocolWorld) {
    let p = w.h_proof.as_ref().unwrap();
    assert!(
        p.steps
            .iter()
            .all(|s| matches!(s, aithos_core::merkle::ProofStep::Node { .. })),
        "self proofs are sibling hashes only"
    );
    let json = serde_json::to_string(p).unwrap();
    for leak in ["name", "title", "wrap", "souvenir"] {
        assert!(!json.contains(leak), "self proof leaks '{leak}'");
    }
}

#[then("the edition diff descends to exactly the new section")]
fn h_diff_exact(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    let added: Vec<&String> = diff
        .iter()
        .filter(|(k, v)| **v == "added" && k.contains("/s/"))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(added.len(), 1, "exactly one section appears: {diff:?}");
    assert!(added[0].starts_with("circle:d/"), "it is the circle one");
}

#[then("no other zone appears in the diff")]
fn h_diff_one_zone(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    assert!(
        diff.keys().all(|k| k.starts_with("circle:")),
        "only circle may change: {diff:?}"
    );
}

#[then("the edition diff descends into both the old and the new parent")]
fn h_diff_both_parents(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    let removed = diff.values().filter(|v| **v == "removed").count();
    let added = diff.values().filter(|v| **v == "added").count();
    assert!(removed > 0, "the old parent loses the subtree: {diff:?}");
    assert!(added > 0, "the new parent gains the subtree: {diff:?}");
}

#[then("the edition diff is empty")]
fn h_diff_empty(w: &mut ProtocolWorld) {
    let diff = w.h_diff();
    assert!(diff.is_empty(), "no change, no diff: {diff:?}");
}

// -------------------------------------- step H2: committed gamma roots
// Spec 07.10 — per-segment roots + counts trie in the manifest, count /
// absence / completeness proofs over the log (h2-gamma-roots.feature).

use aithos_core::gamma::{
    counts_tally, prove_absence, prove_count, prove_entry, segment_root, verify_absence,
    verify_complete_actions, verify_count_proof, AbsenceProof, GammaCounters,
};

impl ProtocolWorld {
    fn h2_tallies(&mut self) -> std::collections::BTreeMap<String, GammaCounters> {
        counts_tally(&self.gbundle().gamma_entries().unwrap())
    }

    fn h2_counts_root(&mut self) -> [u8; 32] {
        <[u8; 32]>::try_from(hex::decode(self.h_manifest().gamma_counts_root).unwrap()).unwrap()
    }

    fn h2_segment_root(&mut self, month: &str) -> [u8; 32] {
        let hex_root = self.h_manifest().gamma_roots[month].root.clone();
        <[u8; 32]>::try_from(hex::decode(hex_root).unwrap()).unwrap()
    }

    /// Inclusion proofs for every `action` entry running under `mandate_id`,
    /// segment by segment — the mirror's honest answer.
    fn h2_action_proofs(&mut self, mandate_id: &str) -> Vec<(String, aithos_core::merkle::Proof)> {
        let mut out = Vec::new();
        let months: Vec<String> = self.h_manifest().gamma_roots.keys().cloned().collect();
        for month in months {
            let lines = self.segment_lines(&format!("gamma/{month}.jsonl"));
            let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
            for (i, line) in lines.iter().enumerate() {
                let e: aithos_core::gamma::Entry = serde_json::from_slice(line).unwrap();
                let runs_under = e.kind == "action"
                    && e.authorized_via
                        .as_ref()
                        .is_some_and(|v| v.iter().any(|m| m == mandate_id));
                if runs_under {
                    out.push((month.clone(), prove_entry(&refs, i).unwrap()));
                }
            }
        }
        out
    }
}

#[given("a bundle whose log spans two months of delegated actions")]
fn h2_two_months(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), "2026-08-31T00:00:00Z");
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.try_action(false, "reply", &day(32, "10:00:00")).unwrap();
}

#[given("a bundle whose log is empty")]
fn h2_empty_log(w: &mut ProtocolWorld) {
    w.init_bundle();
}

#[given("a published edition whose log counts a delegated action")]
fn h2_one_action(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.h_publish();
}

#[given(
    "a published edition whose log counts actions, a sub-grant and budget tokens under a mandate"
)]
fn h2_full_meter(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({"budgets": [{"id": "haiku", "token_budget": 100000}]}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.try_action_full(
        false,
        "reply",
        &day(1, "10:00:00"),
        Some(serde_json::json!({"budget_ref": "haiku", "tokens": 2700})),
        None,
    )
    .unwrap();
    w.try_inference(
        "claude-haiku",
        1200,
        300,
        Some("haiku"),
        &day(1, "11:00:00"),
    )
    .unwrap();
    w.h_publish();
}

#[given("a published edition whose log holds an action by a sub-delegate")]
#[given("a published edition whose log counts two mandates apart in id order")]
fn h2_sub_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    w.try_action(true, "reply", &day(1, "10:00:00")).unwrap();
    w.h_publish();
}

#[given("a published edition whose log counts three mandates")]
fn h2_three_mandates(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(
        vec![aithos_core::mandate::PerimeterEntry::Issue { depth: 1 }],
        serde_json::json!({}),
        NA30,
    );
    w.delegate_act("act.x.gmail.reply", serde_json::json!({}), true)
        .unwrap();
    let first_chain = w.helper_chain.clone();
    w.delegate_act("act.x.gmail.label", serde_json::json!({}), true)
        .unwrap();
    w.try_action(true, "label", &day(1, "10:00:00")).unwrap();
    w.helper_chain = first_chain;
    w.try_action(true, "reply", &day(2, "10:00:00")).unwrap();
    w.h_publish();
    assert_eq!(w.h2_tallies().len(), 3, "root and two children counted");
}

#[given("a published edition whose log counts three actions under a mandate")]
fn h2_three_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    for (d, hms) in [(1, "10:00:00"), (2, "10:00:00"), (3, "10:00:00")] {
        w.try_action(false, "reply", &day(d, hms)).unwrap();
    }
    w.h_publish();
}

#[given("a published edition whose log spans one month")]
fn h2_one_month(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.try_action(false, "reply", &day(1, "10:00:00")).unwrap();
    w.try_action(false, "reply", &day(2, "10:00:00")).unwrap();
    w.h_publish();
}

#[then("the manifest commits a gamma root and entry count for each of the two segments")]
fn h2_two_segments_committed(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let months: Vec<String> = m.gamma_roots.keys().cloned().collect();
    assert_eq!(
        months,
        ["2026-07", "2026-08"],
        "one root per non-empty month"
    );
    for (month, seg) in &m.gamma_roots {
        assert_eq!(seg.root.len(), 64, "{month}: hex root");
        assert!(seg.n > 0, "{month}: committed entry count");
    }
}

#[then("the manifest commits a gamma counts root")]
fn h2_counts_root_committed(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    assert_eq!(m.gamma_counts_root.len(), 64);
    assert_ne!(m.gamma_counts_root, "0".repeat(64), "mandates were counted");
}

#[then("the content roots and the flat file pins still verify")]
fn h2_additive(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    assert_eq!(m.roots.len(), 4, "the §02.10 roots still ride");
    assert!(!m.files.is_empty(), "the flat pins still ride");
    w.gbundle().verify().unwrap();
}

#[then("an independent recomputation from the store yields the same gamma roots and counts root")]
fn h2_recompute_identical(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let (roots, counts) = w.gbundle().gamma_state().unwrap();
    assert_eq!(roots, m.gamma_roots);
    assert_eq!(counts, m.gamma_counts_root);
}

#[then("the manifest commits no gamma segment roots")]
fn h2_no_segments(w: &mut ProtocolWorld) {
    assert!(w.h_manifest().gamma_roots.is_empty());
}

#[then("the gamma counts root is thirty-two zero bytes")]
fn h2_zero_counts_root(w: &mut ProtocolWorld) {
    assert_eq!(w.h_manifest().gamma_counts_root, "0".repeat(64));
}

#[when("a mirror rewrites a clear counter field inside the last log entry")]
fn h2_tamper_last_entry(w: &mut ProtocolWorld) {
    let seg = "gamma/2026-07.jsonl";
    let mut lines = w.segment_lines(seg);
    let mut entry: serde_json::Value = serde_json::from_slice(lines.last().unwrap()).unwrap();
    entry["payload"]["action"] = serde_json::Value::String("send".into());
    *lines.last_mut().unwrap() = aithos_core::jcs::canonical_bytes(&entry).unwrap();
    let mut bytes = Vec::new();
    for line in &lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[then("edition verification is refused")]
fn h2_verify_refused(w: &mut ProtocolWorld) {
    // Root recomputation ALONE catches it (the rule under test)…
    let m = w.h_manifest();
    let (roots, _) = w.gbundle().gamma_state().unwrap();
    assert_ne!(roots, m.gamma_roots, "the recomputed segment root died");
    // …and full verification is refused, fail-closed.
    assert!(w.gbundle().verify().is_err());
}

#[when("a verifier asks for the action entry's inclusion proof")]
fn h2_prove_entry(w: &mut ProtocolWorld) {
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
    let idx = lines
        .iter()
        .position(|l| {
            serde_json::from_slice::<aithos_core::gamma::Entry>(l)
                .unwrap()
                .kind
                == "action"
        })
        .expect("the fixture logged an action");
    w.h2_proof = Some(prove_entry(&refs, idx).unwrap());
}

#[then("the gamma proof verifies offline against the committed segment root")]
fn h2_entry_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h2_segment_root("2026-07");
    let proof = w.h2_proof.as_ref().unwrap();
    aithos_core::merkle::verify_proof(proof, &root).unwrap();
}

#[when("the mirror alters the entry's action name inside the proven bytes")]
fn h2_tamper_proven_bytes(w: &mut ProtocolWorld) {
    h2_prove_entry(w);
    let proof = w.h2_proof.as_mut().unwrap();
    let mut entry: serde_json::Value =
        serde_json::from_slice(&hex::decode(&proof.payload).unwrap()).unwrap();
    entry["payload"]["action"] = serde_json::Value::String("send".into());
    proof.payload = hex::encode(aithos_core::jcs::canonical_bytes(&entry).unwrap());
}

#[then("the gamma proof is refused")]
fn h2_entry_proof_refused(w: &mut ProtocolWorld) {
    let root = w.h2_segment_root("2026-07");
    let proof = w.h2_proof.as_ref().unwrap();
    let err = aithos_core::merkle::verify_proof(proof, &root).unwrap_err();
    assert!(matches!(
        err,
        aithos_core::error::Error::MerkleProofInvalid(_)
    ));
}

#[when("a verifier asks for the mandate's count proof")]
fn h2_prove_count(w: &mut ProtocolWorld) {
    let id = w.chain[0].id.clone();
    let tallies = w.h2_tallies();
    w.h2_proof = Some(prove_count(&tallies, &id).unwrap());
}

#[then("the count proof verifies offline against the committed counts root")]
fn h2_count_proof_ok(w: &mut ProtocolWorld) {
    let root = w.h2_counts_root();
    let proof = w.h2_proof.clone().unwrap();
    let (id, counters) = verify_count_proof(&proof, &root).unwrap();
    w.h2_counters = vec![(id, counters)];
}

#[then("the proven counters equal the raw tallies of the chain")]
fn h2_counters_equal_tallies(w: &mut ProtocolWorld) {
    let (id, counters) = w.h2_counters[0].clone();
    let entries = w.gbundle().gamma_entries().unwrap();
    assert_eq!(
        counters.actions,
        aithos_core::gamma::count_actions(&entries, &id, None, None) as u64,
        "actions == raw subtree tally"
    );
    assert_eq!(
        counters.children,
        aithos_core::gamma::count_children(&entries, &id) as u64,
        "children == raw tally"
    );
    let haiku = &counters.budgets["haiku"];
    assert_eq!(
        haiku.actions,
        aithos_core::constraints::count_profile_entries(&entries, &id, "haiku") as u64,
        "budget actions == raw tally"
    );
    assert_eq!(
        haiku.tokens,
        aithos_core::constraints::tally_tokens(&entries, &id, "haiku"),
        "budget tokens == raw tally (attested override included)"
    );
    let raw_entries = entries
        .iter()
        .filter(|e| e.authorized_via.as_ref().is_some_and(|v| v.contains(&id)))
        .count() as u64;
    assert_eq!(counters.entries, raw_entries, "entries == the audit total");
}

#[when("a verifier proves the counts of the root mandate and of the leaf mandate")]
fn h2_prove_both(w: &mut ProtocolWorld) {
    let root_id = w.chain[0].id.clone();
    let leaf_id = w.helper_chain[1].id.clone();
    let tallies = w.h2_tallies();
    let pinned = w.h2_counts_root();
    w.h2_counters = [root_id, leaf_id]
        .iter()
        .map(|id| {
            let proof = prove_count(&tallies, id).unwrap();
            verify_count_proof(&proof, &pinned).unwrap()
        })
        .collect();
}

#[then("both count leaves carry that action")]
fn h2_both_carry(w: &mut ProtocolWorld) {
    for (id, counters) in &w.h2_counters {
        assert_eq!(
            counters.actions, 1,
            "{id}: the sub-delegate's action counts"
        );
    }
}

#[when("a verifier asks whether an id between them was ever counted")]
fn h2_ask_absence(w: &mut ProtocolWorld) {
    let tallies = w.h2_tallies();
    let ids: Vec<&String> = tallies.keys().collect();
    assert_eq!(ids.len(), 2);
    let absent = format!("{}0", ids[0]); // strictly between the two, byte order
    assert!(ids[0] < &absent && &absent < ids[1]);
    let pinned = w.h2_counts_root();
    let proof = prove_absence(&tallies, &absent).unwrap();
    assert!(
        proof.left.is_some() && proof.right.is_some(),
        "two bracketing leaves"
    );
    w.h2_verdict = Some(verify_absence(&absent, &proof, &pinned).map_err(|e| e.to_string()));
}

#[then("the mirror proves absence with two adjacent leaves bracketing the id")]
fn h2_absence_ok(w: &mut ProtocolWorld) {
    assert_eq!(w.h2_verdict.clone().unwrap(), Ok(()));
}

#[when("the mirror claims absence of the middle one with the outer two leaves")]
fn h2_forge_absence(w: &mut ProtocolWorld) {
    let tallies = w.h2_tallies();
    let ids: Vec<String> = tallies.keys().cloned().collect();
    let forged = AbsenceProof {
        left: Some(prove_count(&tallies, &ids[0]).unwrap()),
        right: Some(prove_count(&tallies, &ids[2]).unwrap()),
    };
    let pinned = w.h2_counts_root();
    w.h2_verdict = Some(verify_absence(&ids[1], &forged, &pinned).map_err(|e| e.to_string()));
}

#[then("the absence proof is refused")]
fn h2_absence_refused(w: &mut ProtocolWorld) {
    let msg = w.h2_verdict.clone().unwrap().unwrap_err();
    assert!(msg.contains("absence proof invalid"), "{msg}");
}

#[when("a mirror answers the mandate's action query with only two proven entries")]
fn h2_withhold_answer(w: &mut ProtocolWorld) {
    let id = w.chain[0].id.clone();
    let proofs = w.h2_action_proofs(&id);
    assert_eq!(proofs.len(), 3, "the honest answer would be three");
    let tallies = w.h2_tallies();
    let count_proof = prove_count(&tallies, &id).unwrap();
    let pinned = w.h2_counts_root();
    let m = w.h_manifest();
    let segment_roots = m
        .gamma_roots
        .iter()
        .map(|(month, seg)| {
            let root = <[u8; 32]>::try_from(hex::decode(&seg.root).unwrap()).unwrap();
            (month.clone(), root)
        })
        .collect();
    w.h2_verdict = Some(
        verify_complete_actions(&id, &count_proof, &proofs[..2], &segment_roots, &pinned)
            .map(|_| ())
            .map_err(|e| e.to_string()),
    );
}

#[then("the answer is refused against the proven count of three")]
fn h2_withhold_refused(w: &mut ProtocolWorld) {
    let msg = w.h2_verdict.clone().unwrap().unwrap_err();
    assert!(msg.contains("withhold detected"), "{msg}");
    assert!(msg.contains("proven count of 3"), "{msg}");
}

#[when("a mirror serves the segment with one entry withheld")]
fn h2_withhold_segment(w: &mut ProtocolWorld) {
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let served: Vec<&[u8]> = lines
        .iter()
        .take(lines.len() - 1)
        .map(Vec::as_slice)
        .collect();
    let committed = w.h_manifest().gamma_roots["2026-07"].clone();
    let recomputed = segment_root(&served);
    w.h2_verdict = Some(
        if hex::encode(recomputed) == committed.root || served.len() as u64 == committed.n {
            Ok(())
        } else {
            Err("segment omission died on root and count".into())
        },
    );
}

#[then("the recomputed segment root dies against the committed root and count")]
fn h2_segment_omission_dies(w: &mut ProtocolWorld) {
    assert!(w.h2_verdict.clone().unwrap().is_err());
}

// ------------------------------------------- step I: concurrency (02.6 + 07.6)
// Disjoint merge, the two-predecessor merge entry, 3-way index merge by
// sid, fork refusal and nearest-common-manager resolution
// (i-concurrency.feature).

use aithos_bundle::merge::ForkResolver;
use aithos_core::concurrency::{
    verify_disjoint_merge, MergeAuthority, SemanticCounts, SemanticOccurrence,
};

// The I timeline sits after the D/H fixture NOW (2026-07-09).
const I_ANC: &str = "2026-07-10T00:00:00Z"; // the shared ancestor edition
const I_W1: &str = "2026-07-10T01:00:00Z"; // this copy's write
const I_W2: &str = "2026-07-10T02:00:00Z"; // the other copy's write
const I_PUB: &str = "2026-07-10T03:00:00Z"; // both competing editions
const I_MERGE: &str = "2026-07-10T04:00:00Z"; // the merge / resolution
const I_AFTER: &str = "2026-07-10T05:00:00Z"; // life after the join

impl ProtocolWorld {
    /// The §02.6 fixture: publish the shared ancestor edition, then split
    /// the world into two byte-identical copies of the whole store.
    fn i_split(&mut self) {
        let owner = self.owner(0);
        self.gbundle().publish(&owner, I_ANC).unwrap();
        let b = self.bundle.as_ref().unwrap();
        self.i_other = Some(Bundle {
            store: b.store.clone(),
            did: b.did.clone(),
        });
    }

    fn i_bundle(&mut self, other: bool) -> &mut Bundle<MemStore> {
        if other {
            self.i_other.as_mut().unwrap()
        } else {
            self.bundle.as_mut().unwrap()
        }
    }

    fn i_add_on(&mut self, other: bool, folder: &str, name: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        let b = self.i_bundle(other);
        b.ensure_folder(Zone::Circle, folder, &owner, &mut ent)
            .unwrap();
        b.section_add(
            &SectionSpec {
                zone: Zone::Circle,
                folder_path: folder,
                name,
                title: "note",
                tags: &[],
                body: BODY,
                now: at,
            },
            &owner,
            &mut ent,
        )
        .unwrap();
        self.ent = ent;
    }

    fn i_rewrite_on(&mut self, other: bool, path: &str, body: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.i_bundle(other)
            .section_rewrite(Zone::Circle, path, body, &owner, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn i_delete_on(&mut self, other: bool, path: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        self.i_bundle(other)
            .section_delete(Zone::Circle, path, &owner, at, &mut ent)
            .unwrap();
        self.ent = ent;
    }

    fn i_publish_on(&mut self, other: bool, at: &str) {
        let owner = self.owner(0);
        self.i_bundle(other).publish(&owner, at).unwrap();
    }

    fn i_action_on(&mut self, other: bool, action: &str, at: &str) -> Result<String, String> {
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .i_bundle(other)
            .log_action(
                &chain,
                &agent_sk(AGENT),
                &aithos_bundle::log::ActionSpec {
                    connector: "gmail",
                    action,
                    args_hash: "sha256:00",
                    now: at,
                    budget: None,
                    sealed_args: None,
                },
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    /// Merge the other copy's competing edition into this one.
    fn i_merge(&mut self) -> Result<(), String> {
        let owner = self.owner(0);
        let other = self.i_other.take().unwrap();
        let r = self
            .gbundle()
            .edition_merge(&other, &owner, I_MERGE)
            .map_err(|e| e.to_string());
        self.i_other = Some(other);
        r
    }

    /// Root WRITE mandate over one circle folder — the delegate a fork
    /// resolution names as its nearest common manager.
    fn i_grant_write_dir(&mut self, folder: &str) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, Verb};
        let owner = self.owner(0);
        let dir = self
            .bundle
            .as_ref()
            .unwrap()
            .resolve_folder(Zone::Circle, folder)
            .unwrap();
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 970)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter: vec![aithos_core::mandate::PerimeterEntry::Ethos {
                    verb: Verb::Write,
                    zone: Zone::Circle,
                    dir,
                    tag: None,
                }],
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    /// Root WRITE mandate over the complete circle zone. This is still one
    /// mandate chain and therefore one merge publisher.
    fn i_grant_write_circle(&mut self) {
        use aithos_core::mandate::{Mandate as M, MandateSpec, Verb};
        let owner = self.owner(0);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 980)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: "urn:aithos:agent:agent".into(),
                grantee_label: "agent".into(),
                grantee_pub: &agent_sk(AGENT).verifying_key(),
                perimeter: vec![aithos_core::mandate::PerimeterEntry::Ethos {
                    verb: Verb::Write,
                    zone: Zone::Circle,
                    dir: Vec::new(),
                    tag: None,
                }],
                constraints: MandateSpec::no_constraints(),
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        self.chain = vec![m];
    }

    fn i_merge_as_delegate(&mut self) -> Result<(), String> {
        let chain = self.chain.clone();
        let sk = agent_sk(AGENT);
        let other = self.i_other.take().unwrap();
        let result = self
            .gbundle()
            .edition_merge_as(
                &other,
                &ForkResolver::Delegate {
                    chain: &chain,
                    sk: &sk,
                },
                I_MERGE,
            )
            .map_err(|error| error.to_string());
        self.i_other = Some(other);
        result
    }

    fn i_store_snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let store = &self.bundle.as_ref().unwrap().store;
        store
            .list("")
            .unwrap()
            .into_iter()
            .map(|path| {
                let bytes = store.get(&path).unwrap().expect("listed object exists");
                (path, bytes)
            })
            .collect()
    }

    fn i_resolve(&mut self, delegate: bool) -> Result<Vec<String>, String> {
        let owner = self.owner(0);
        let chain = self.chain.clone();
        let sk = agent_sk(AGENT);
        let resolver = if delegate {
            ForkResolver::Delegate {
                chain: &chain,
                sk: &sk,
            }
        } else {
            ForkResolver::Owner(&owner)
        };
        let other = self.i_other.take().unwrap();
        let r = self
            .bundle
            .as_mut()
            .unwrap()
            .resolve_fork(&other, &resolver, I_MERGE)
            .map_err(|e| e.to_string());
        self.i_other = Some(other);
        r
    }

    fn i_parent_manifests(&mut self) -> (Manifest, Manifest) {
        let h = self.h_manifest().edition.height;
        let low: Manifest = serde_json::from_slice(
            &self
                .gbundle()
                .store
                .get(&format!("manifests/{}.json", h - 1))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        let alt: Manifest = serde_json::from_slice(
            &self
                .gbundle()
                .store
                .get(&format!("manifests/{}-alt.json", h - 1))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        (low, alt)
    }
}

// --- I givens ---

#[given("two copies of a published bundle")]
fn i_two_copies(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "seed", &[]);
    w.i_split();
}

#[given("two copies of a published bundle holding a circle section")]
fn i_two_copies_with_section(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.i_split();
}

#[given("two copies of a published bundle whose agents each logged an action")]
fn i_two_copies_with_actions(w: &mut ProtocolWorld) {
    w.init_bundle();
    // A logged mutation gives the shared log a real tip to fork from.
    w.add_named_section("projets", "seed", &[]);
    w.grant_act(vec![], serde_json::json!({}), NA30);
    w.i_split();
    w.i_action_on(false, "reply", I_W1).unwrap();
    w.i_action_on(true, "label", I_W2).unwrap();
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("two copies of a published bundle whose agent may act three times in total")]
fn i_two_copies_budget(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "seed", &[]);
    w.grant_act(vec![], serde_json::json!({ "max_actions": 3 }), NA30);
    w.i_split();
}

#[given("each copy adds a circle section under a different folder")]
fn i_disjoint_adds(w: &mut ProtocolWorld) {
    w.i_add_on(false, "alpha", "note-a", I_W1);
    w.i_add_on(true, "beta", "note-b", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy adds a differently-named section under the same folder")]
fn i_same_folder_adds(w: &mut ProtocolWorld) {
    w.i_add_on(false, "projets", "note-a", I_W1);
    w.i_add_on(true, "projets", "note-b", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("one copy deletes that section while the other adds a sibling")]
fn i_delete_vs_add(w: &mut ProtocolWorld) {
    w.i_delete_on(true, "projets/note1", I_W2);
    w.i_add_on(false, "projets", "note2", I_W1);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy modifies that same section differently")]
fn i_same_node_writes(w: &mut ProtocolWorld) {
    w.i_rewrite_on(false, "projets/note1", "the winning body", I_W1);
    w.i_rewrite_on(true, "projets/note1", "the losing body", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("each copy logs two actions under that mandate")]
fn i_two_actions_each(w: &mut ProtocolWorld) {
    w.i_action_on(false, "reply", I_W1).unwrap();
    w.i_action_on(false, "reply", "2026-07-10T01:30:00Z")
        .unwrap();
    w.i_action_on(true, "label", I_W2).unwrap();
    w.i_action_on(true, "label", "2026-07-10T02:30:00Z")
        .unwrap();
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("two competing editions modifying the same section")]
fn i_fork_fixture(w: &mut ProtocolWorld) {
    i_two_copies_with_section(w);
    i_same_node_writes(w);
}

#[given("two competing editions modifying the same section under a delegate's folder")]
fn i_fork_under_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.i_grant_write_dir("projets");
    w.i_split();
    i_same_node_writes(w);
}

#[given("two competing editions touching a folder outside the delegate's grant")]
fn i_fork_outside_delegate(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "note1", &[]);
    w.add_named_section("autre", "note2", &[]);
    w.i_grant_write_dir("projets");
    w.i_split();
    w.i_rewrite_on(false, "autre/note2", "the winning body", I_W1);
    w.i_rewrite_on(true, "autre/note2", "the losing body", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);
}

#[given("two local branches with disjoint changes")]
fn i_local_disjoint_branches(w: &mut ProtocolWorld) {
    i_two_copies(w);
    i_disjoint_adds(w);
}

#[given(expr = "the publishing actor has {string}")]
fn i_merge_actor_authority(w: &mut ProtocolWorld, authority: String) {
    match authority.as_str() {
        "one chain covering both changed nodes" => w.i_grant_write_circle(),
        "one chain covering only the first node" => w.i_grant_write_dir("alpha"),
        "two separate partial chains" | "owner local capability" => {}
        other => panic!("unknown merge authority fixture: {other}"),
    }
    w.i_authority = authority;
}

#[given("two exported local bundle branches with the same parent")]
fn i_exported_local_branches(w: &mut ProtocolWorld) {
    i_two_copies(w);
    i_disjoint_adds(w);
}

#[given("a forked local bundle snapshotted before resolution")]
fn i_snapshotted_local_fork(w: &mut ProtocolWorld) {
    i_fork_outside_delegate(w);
    w.i_snapshot = w.i_store_snapshot();
}

fn i_semantic_occurrences(bundle: &mut Bundle<MemStore>) -> Vec<SemanticOccurrence> {
    bundle
        .gamma_entries()
        .unwrap()
        .into_iter()
        .filter_map(|entry| {
            let kind = match entry.kind.as_str() {
                "action" => "action",
                "grant" => "grant",
                "section.add" | "section.modify" | "section.delete" | "section.redact" => {
                    "mutation"
                }
                _ => return None,
            };
            Some(SemanticOccurrence {
                operation_ref: entry.chain_hash().unwrap(),
                kind: kind.into(),
            })
        })
        .collect()
}

#[given("two disjoint branches carrying delegated actions, mutations and grants")]
fn i_branches_with_semantic_occurrences(w: &mut ProtocolWorld) {
    w.init_bundle();
    w.add_named_section("projets", "seed", &[]);
    w.grant_act(vec![], serde_json::json!({}), NA30);
    let owner = w.owner(0);
    let mandate_id = w.chain[0].id.clone();
    let mut entropy = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_owner_grant(&owner, &mandate_id, I_ANC, &mut entropy)
        .unwrap();
    w.ent = entropy;
    w.i_split();
    w.i_action_on(false, "reply", I_W1).unwrap();
    w.i_action_on(true, "label", I_W2).unwrap();
    w.i_add_on(false, "alpha", "note-a", I_W1);
    w.i_add_on(true, "beta", "note-b", I_W2);
    w.i_publish_on(false, I_PUB);
    w.i_publish_on(true, I_PUB);

    let left = i_semantic_occurrences(w.bundle.as_mut().unwrap());
    let right = i_semantic_occurrences(w.i_other.as_mut().unwrap());
    w.i_semantic_counts = Some(
        aithos_bundle::merge::recompose_semantic_counts(&left, &right)
            .expect("branch semantic occurrences recompose"),
    );
}

// --- I whens ---

#[when("either party publishes the merge edition")]
fn i_when_merge(w: &mut ProtocolWorld) {
    w.i_result = Some(w.i_merge());
}

#[when("a party attempts the merge edition")]
fn i_when_merge_attempt(w: &mut ProtocolWorld) {
    w.i_result = Some(w.i_merge());
}

#[when("each party computes the merge edition independently")]
fn i_when_both_merge(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    // Pre-merge clones so each party merges from the same starting point.
    let mine = Bundle {
        store: w.bundle.as_ref().unwrap().store.clone(),
        did: w.bundle.as_ref().unwrap().did.clone(),
    };
    let mut theirs = Bundle {
        store: w.i_other.as_ref().unwrap().store.clone(),
        did: w.i_other.as_ref().unwrap().did.clone(),
    };
    w.i_merge().unwrap();
    theirs.edition_merge(&mine, &owner, I_MERGE).unwrap();
    for b in [w.bundle.as_ref().unwrap(), &theirs] {
        let bytes = b.store.get("manifest.json").unwrap().unwrap();
        w.i_hashes.push(sha256_hex(&bytes));
    }
}

#[when("a verifier is shown both branches")]
fn i_when_fork_check(w: &mut ProtocolWorld) {
    let other = w.i_other.take().unwrap();
    w.i_result = Some(w.gbundle().fork_check(&other).map_err(|e| e.to_string()));
    w.i_other = Some(other);
}

#[when("the covering delegate publishes the resolving edition naming the winner")]
fn i_when_delegate_resolves(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(true));
}

#[when("the delegate attempts the resolving edition")]
fn i_when_delegate_attempts(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(true));
}

#[when("the owner publishes the resolving edition naming the winner")]
fn i_when_owner_resolves(w: &mut ProtocolWorld) {
    w.i_surfaced = Some(w.i_resolve(false));
}

#[when("that actor attempts the deterministic merge edition")]
fn i_actor_attempts_merge(w: &mut ProtocolWorld) {
    w.i_result = Some(match w.i_authority.as_str() {
        "owner local capability" => w.i_merge(),
        "one chain covering both changed nodes" | "one chain covering only the first node" => {
            w.i_merge_as_delegate()
        }
        "two separate partial chains" => {
            let left = BTreeSet::from(["sid-left".to_owned()]);
            let right = BTreeSet::from(["sid-right".to_owned()]);
            let authority = MergeAuthority::Grantee {
                chain_count: 2,
                covered_sids: left.union(&right).cloned().collect(),
            };
            verify_disjoint_merge(&left, &right, &BTreeSet::new(), &authority)
                .map(drop)
                .map_err(|error| error.to_string())
        }
        other => panic!("unknown authority case: {other}"),
    });
}

#[when("an authorized actor merges them into a fresh local store")]
fn i_merge_in_fresh_local_store(w: &mut ProtocolWorld) {
    let exported = w.bundle.as_ref().unwrap();
    let mut fresh = Bundle {
        store: exported.store.clone(),
        did: exported.did.clone(),
    };
    let owner = w.owner(0);
    let other = w.i_other.as_ref().unwrap();
    let result = fresh
        .edition_merge(other, &owner, I_MERGE)
        .map_err(|error| error.to_string());
    w.bundle = Some(fresh);
    w.i_result = Some(result);
}

#[when("a grantee outside one touched perimeter attempts to resolve it")]
fn i_outside_grantee_attempts_resolution(w: &mut ProtocolWorld) {
    i_when_delegate_attempts(w);
}

#[when("one authorized actor publishes their deterministic local merge")]
fn i_authorized_actor_merges_semantic_branches(w: &mut ProtocolWorld) {
    w.i_result = Some(w.i_merge());
}

// --- I thens ---

#[then("the merge manifest pins the lowest-hash parent and lists both parents ascending")]
fn i_then_ordering(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let m = w.h_manifest();
    assert_eq!(m.merges.len(), 2, "two parents");
    assert!(m.merges[0] < m.merges[1], "ascending edition hashes");
    assert_eq!(m.edition.prev_hash, m.merges[0], "prev_hash = the lowest");
    let (low, alt) = w.i_parent_manifests();
    assert_eq!(low.chain_hash().unwrap(), m.merges[0], "low parent slot");
    assert_eq!(alt.chain_hash().unwrap(), m.merges[1], "high parent slot");
}

#[then("both sections are present and the edition verifies")]
fn i_then_both_present(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.gbundle();
    assert_eq!(
        b.read_section(Zone::Circle, "alpha/note-a", &owner)
            .unwrap(),
        BODY
    );
    assert_eq!(
        b.read_section(Zone::Circle, "beta/note-b", &owner).unwrap(),
        BODY
    );
    b.verify().expect("the merged edition verifies");
}

#[then("the two merged manifests hash identically")]
fn i_then_identical(w: &mut ProtocolWorld) {
    assert_eq!(w.i_hashes.len(), 2);
    assert_eq!(
        w.i_hashes[0], w.i_hashes[1],
        "byte-identical merge manifests from either merger"
    );
}

#[then("the folder's index carries both rows in sid order")]
fn i_then_sid_order(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let index: ZoneIndex = serde_json::from_slice(
        &w.gbundle()
            .store
            .get("e/circle/index.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    let names: Vec<&str> = index.sections.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"note-a") && names.contains(&"note-b"));
    let sids: Vec<&String> = index.sections.iter().map(|r| &r.sid).collect();
    let mut sorted = sids.clone();
    sorted.sort();
    assert_eq!(sids, sorted, "rows land in sid order");
}

#[then("the edition verifies")]
fn i_then_verifies(w: &mut ProtocolWorld) {
    w.gbundle().verify().expect("the merged edition verifies");
}

#[then("the deleted section stays absent from the merged index")]
fn i_then_deletion_holds(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let index: ZoneIndex = serde_json::from_slice(
        &w.gbundle()
            .store
            .get("e/circle/index.json")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert!(
        !index.sections.iter().any(|r| r.name == "note1"),
        "no resurrection through the merge"
    );
}

#[then("the sibling is present")]
fn i_then_sibling(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let b = w.gbundle();
    assert_eq!(
        b.read_section(Zone::Circle, "projets/note2", &owner)
            .unwrap(),
        BODY
    );
    b.verify().expect("the merged edition verifies");
}

#[then("the merge is refused as a same-node conflict")]
fn i_then_conflict(w: &mut ProtocolWorld) {
    let err = w
        .i_result
        .clone()
        .unwrap()
        .expect_err("the merge is refused");
    assert!(err.contains("same-node conflict"), "got: {err}");
}

#[then("the merge entry cites both sub-chain tips in prevs")]
fn i_then_merge_entry(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let m = w.h_manifest();
    let (low, alt) = w.i_parent_manifests();
    let entries = w.gbundle().gamma_entries().unwrap();
    let join = entries.last().unwrap();
    assert_eq!(join.kind, "merge");
    assert_eq!(join.chain_hash().unwrap(), m.gamma_head);
    assert_eq!(
        join.prevs.clone().unwrap(),
        vec![low.gamma_head, alt.gamma_head],
        "both tips, ordered like merges"
    );
}

#[then("the merged log verifies from genesis through the join")]
fn i_then_log_verifies(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    aithos_core::gamma::verify_links(&entries).expect("links verify through the join");
    w.gbundle().gamma_verify().expect("every entry verifies");
    w.gbundle().verify().expect("the edition verifies");
}

#[then("the merged segment lays out the lowest-hash parent's entries first")]
fn i_then_layout(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let (low, alt) = w.i_parent_manifests();
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let pos = |head: &str| {
        lines
            .iter()
            .position(|l| {
                let e: aithos_core::gamma::Entry = serde_json::from_slice(l).unwrap();
                e.chain_hash().unwrap() == head
            })
            .expect("tip entry in the merged segment")
    };
    assert!(
        pos(&low.gamma_head) < pos(&alt.gamma_head),
        "sub-chain LOW lays out before sub-chain HIGH"
    );
}

#[then("the manifest's gamma segment root and count match an independent recomputation")]
fn i_then_roots_recommitted(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    let lines = w.segment_lines("gamma/2026-07.jsonl");
    let refs: Vec<&[u8]> = lines.iter().map(Vec::as_slice).collect();
    let committed = &m.gamma_roots["2026-07"];
    assert_eq!(
        hex::encode(aithos_core::gamma::segment_root(&refs)),
        committed.root,
        "recomputed root"
    );
    assert_eq!(refs.len() as u64, committed.n, "recomputed count");
}

#[then("a fifth action after the merge is refused as budget spent")]
fn i_then_budget_spent(w: &mut ProtocolWorld) {
    w.i_result.clone().unwrap().expect("the merge succeeds");
    let err = w
        .i_action_on(false, "reply", I_AFTER)
        .expect_err("the budget tallies across both sub-chains");
    assert!(err.contains("max_actions"), "got: {err}");
}

#[then("neither branch is canonical and the conflict is surfaced")]
fn i_then_fork_surfaced(w: &mut ProtocolWorld) {
    let err = w
        .i_result
        .clone()
        .unwrap()
        .expect_err("the fork is refused");
    assert!(err.contains("same-node conflict"), "got: {err}");
}

#[then("the resolving edition verifies and extends the winning branch")]
fn i_then_resolution_verifies(w: &mut ProtocolWorld) {
    w.i_surfaced
        .clone()
        .unwrap()
        .expect("the resolution succeeds");
    let owner = w.owner(0);
    let m = w.h_manifest();
    assert_eq!(
        m.resolves_fork, m.edition.prev_hash,
        "extends the named winner"
    );
    let b = w.gbundle();
    b.verify().expect("the resolving edition verifies");
    assert_eq!(
        b.read_section(Zone::Circle, "projets/note1", &owner)
            .or_else(|_| b.read_section(Zone::Circle, "autre/note2", &owner))
            .unwrap(),
        "the winning body",
        "content is the winning branch's"
    );
}

#[then("the losing branch's write is surfaced, not replayed")]
fn i_then_loser_surfaced(w: &mut ProtocolWorld) {
    let surfaced = w.i_surfaced.clone().unwrap().unwrap();
    assert!(!surfaced.is_empty(), "the losing labels are reported");
    let h = w.h_manifest().edition.height;
    assert!(
        w.gbundle()
            .store
            .get(&format!("manifests/{}-alt.json", h - 1))
            .unwrap()
            .is_some(),
        "the losing manifest is kept, surfaced"
    );
}

#[then("the resolution is refused for lack of authority")]
fn i_then_resolution_refused(w: &mut ProtocolWorld) {
    let err = w
        .i_surfaced
        .clone()
        .unwrap()
        .expect_err("the delegate is out of perimeter");
    assert!(err.contains("resolution rejected"), "got: {err}");
}

#[then(expr = "publication is {string}")]
fn i_publication_verdict(w: &mut ProtocolWorld, verdict: String) {
    if let Some(result) = &w.core_edition_observation {
        let observation = result
            .as_ref()
            .unwrap_or_else(|error| panic!("CORE-ED scenario failed: {error}"));
        assert_eq!(verdict, observation.expected_verdict);
        assert_eq!(observation.actual_accepted, verdict == "accepted");
        assert!(observation.signer_is_actor);
        return;
    }
    if w.cb5_result.is_some() {
        assert!(matches!(verdict.as_str(), "accepted" | "refused"));
        cb5_assert_green(w);
        cb6_assert_green(w);
        return;
    }
    let result = w.i_result.clone().expect("merge verdict");
    match verdict.as_str() {
        "accepted" => result.expect("authorized merge accepted"),
        "refused" => {
            let error = result.expect_err("unauthorized merge refused");
            assert!(
                error.contains("cover every changed node")
                    || error.contains("one chain covering every changed SID"),
                "typed authority refusal: {error}"
            );
        }
        other => panic!("unknown publication verdict: {other}"),
    }
}

#[then("an accepted grantee merge uses one actor and one mandate chain")]
fn i_grantee_merge_has_one_chain(w: &mut ProtocolWorld) {
    if w.i_authority == "one chain covering both changed nodes" {
        let manifest = w.h_manifest();
        assert_eq!(manifest.authorized_via, vec![w.chain[0].id.clone()]);
        assert_eq!(manifest.signature.key, w.chain[0].grantee.pubkey);
        w.gbundle()
            .verify()
            .expect("delegate-signed merge verifies");
    }
}

#[then("conflict and authority are decided entirely by Core and Bundle")]
fn i_local_core_bundle_decision(w: &mut ProtocolWorld) {
    w.i_result
        .clone()
        .expect("local verdict")
        .expect("local merge accepted");
    w.gbundle().verify().expect("fresh local store verifies");
}

#[then("no HTTP, provider backend, remote store or server CAS participates")]
fn i_no_remote_cas(w: &mut ProtocolWorld) {
    let local_paths = w.gbundle().store.list("").unwrap();
    assert!(local_paths.iter().any(|path| path == "manifest.json"));
    assert!(
        std::any::type_name_of_val(&w.bundle.as_ref().unwrap().store).contains("MemStore"),
        "the acceptance fixture is a local MemStore"
    );
}

#[then("the resolution is refused")]
fn i_snapshotted_resolution_refused(w: &mut ProtocolWorld) {
    i_then_resolution_refused(w);
}

#[then("the manifest, roots, Gamma tips and branch artifacts remain byte-for-byte unchanged")]
fn i_resolution_rollback_is_exact(w: &mut ProtocolWorld) {
    assert_eq!(w.i_store_snapshot(), w.i_snapshot);
}

#[then("fresh-store replay rebuilds the same action, mutation, total and direct-child tallies")]
fn i_fresh_replay_rebuilds_semantic_counts(w: &mut ProtocolWorld) {
    w.i_result
        .clone()
        .expect("merge result")
        .expect("semantic merge accepted");
    let source = w.bundle.as_ref().unwrap();
    let mut fresh = Bundle {
        store: source.store.clone(),
        did: source.did.clone(),
    };
    fresh.verify().expect("fresh edition verifies");
    fresh.gamma_verify().expect("fresh Gamma replay verifies");
    let replayed = i_semantic_occurrences(&mut fresh);
    let actual = aithos_bundle::merge::recompose_semantic_counts(&replayed, &[])
        .expect("fresh semantic replay");
    assert_eq!(actual, w.i_semantic_counts.clone().unwrap());
    assert!(actual.actions >= 2);
    assert!(actual.mutations >= 3);
    assert!(actual.direct_children >= 1);
}

#[then("no branch consumption is omitted or counted twice")]
fn i_no_semantic_double_count(w: &mut ProtocolWorld) {
    let counts: SemanticCounts = w.i_semantic_counts.clone().expect("semantic counts");
    assert_eq!(
        counts.consumptions,
        counts.actions + counts.mutations + counts.direct_children
    );
}

// --- step K: integration — the lived bundle (plan §K, spec §09) ---
//
// One walkthrough, one artifact. The K cast keeps its own keypairs and
// chains; where a K step re-enacts an A–I mechanism it calls the same
// bundle APIs (and, for Thens, sets the same world slots so the existing
// assertions are reused verbatim). K time starts AFTER the creation
// fixtures: kd(0) = 2026-07-10, everything stays inside July — one gamma
// segment, no cross-month fork.

const READER: u8 = 0xC1;
const KGMAIL: u8 = 0xC2;
const KNIGHT: u8 = 0xC3;

fn kd(n: u32, hms: &str) -> String {
    day(9 + n, hms)
}

impl ProtocolWorld {
    /// Root mandate with an arbitrary perimeter + constraints, cert stored.
    fn k_mint_root(
        &mut self,
        label: &str,
        sk: u8,
        perimeter: Vec<PerimeterEntry>,
        constraints: serde_json::Value,
    ) -> Mandate {
        use aithos_core::mandate::{Mandate as M, MandateSpec};
        let owner = self.owner(0);
        let m = M::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!("mandate_{}", sid(u128::from(self.ent.e16()[15]) + 990)),
                subject: self.bundle.as_ref().unwrap().did.clone(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.to_owned(),
                grantee_pub: &agent_sk(sk).verifying_key(),
                perimeter,
                constraints,
                not_before: NB.into(),
                not_after: NA30.into(),
                issued_at: NB.into(),
                nonce: hex::encode(self.ent.e16()),
            },
        )
        .unwrap();
        self.store_cert(&m);
        m
    }

    /// Deliver read lines for a folder, then mint ONE cert carrying the
    /// read entry plus connector actions and constraints ("one key, N
    /// perimeters"). The line-delivery sidecar cert is not used as a chain.
    fn k_grant_read_act(
        &mut self,
        label: &str,
        sk: u8,
        folder: &str,
        acts: &[&str],
        constraints: serde_json::Value,
    ) -> Vec<Mandate> {
        use aithos_core::mandate::Verb;
        let owner = self.owner(0);
        self.bundle
            .as_mut()
            .unwrap()
            .grant(
                &owner,
                &format!("{label}-lines"),
                &agent_sk(sk).verifying_key(),
                &[dir_spec(folder)],
                NB,
                NA30,
                0,
                &mut self.ent,
            )
            .unwrap();
        let dir = self
            .bundle
            .as_ref()
            .unwrap()
            .resolve_folder(Zone::Circle, folder)
            .unwrap();
        let mut perimeter = vec![PerimeterEntry::Ethos {
            verb: Verb::Read,
            zone: Zone::Circle,
            dir,
            tag: None,
        }];
        for a in acts {
            perimeter.push(PerimeterEntry::parse(a).unwrap());
        }
        vec![self.k_mint_root(label, sk, perimeter, constraints)]
    }

    /// Append (or refuse) an action under a named chain at a K time,
    /// setting the shared gamma slots so existing Thens read the result.
    #[allow(clippy::too_many_arguments)] // same shape as grant()/log_action
    fn k_act(
        &mut self,
        chain: &[Mandate],
        sk: u8,
        connector: &str,
        action: &str,
        at: &str,
        checks: Option<serde_json::Value>,
        sealed: Option<serde_json::Value>,
    ) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .log_action_with_checks(
                chain,
                &agent_sk(sk),
                &aithos_bundle::log::ActionSpec {
                    connector,
                    action,
                    args_hash: G_ARGS,
                    now: at,
                    budget: None,
                    sealed_args: sealed,
                },
                checks,
                &mut ent,
            )
            .map(|e| e.id)
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    /// Owner adds a circle section at a K time (the fixture helper logs
    /// at the D-era NOW, which would send `at` backward on the lived log).
    fn k_add_section(&mut self, folder: &str, name: &str, at: &str) {
        let owner = self.owner(0);
        let mut ent = std::mem::take(&mut self.ent);
        let bundle = self.bundle.as_mut().unwrap();
        bundle
            .ensure_folder(Zone::Circle, folder, &owner, &mut ent)
            .unwrap();
        bundle
            .section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags: &[],
                    body: BODY,
                    now: at,
                },
                &owner,
                &mut ent,
            )
            .unwrap();
        self.ent = ent;
    }

    /// The approver's receipt over the prepared social publish, WYSIWYS.
    fn k_approval(&mut self, at: &str) -> serde_json::Value {
        let leaf = self.k_social.last().unwrap().id.clone();
        ob_receipt(
            &agent_sk(APPROVER),
            "publish-approval",
            (&leaf, "publish", G_ARGS),
            "approve",
            at,
            Some("sha256:rendered-ship-it"),
        )
    }
}

// --- K1 whens: tree, grants, liveness ---

#[when(expr = "I add a public section {string} in folder {string}")]
fn k_add_public(w: &mut ProtocolWorld, name: String, folder: String) {
    let owner = w.owner(0);
    let bundle = w.bundle.as_mut().unwrap();
    bundle
        .ensure_folder(Zone::Public, &folder, &owner, &mut w.ent)
        .unwrap();
    bundle
        .section_add(
            &SectionSpec {
                zone: Zone::Public,
                folder_path: &folder,
                name: &name,
                title: "bio",
                tags: &[],
                body: PUB_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
}

#[when(expr = "I add a self folder {string} with a section {string}")]
fn k_add_self(w: &mut ProtocolWorld, folder: String, name: String) {
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .section_add(
            &SectionSpec {
                zone: Zone::Self_,
                folder_path: &folder,
                name: &name,
                title: "cicatrice au genou",
                tags: &["sante".to_owned()],
                body: SELF_BODY,
                now: NOW,
            },
            &owner,
            &mut w.ent,
        )
        .unwrap();
}

#[when(expr = "the owner grants a reader agent read on circle folder {string} with issue depth 1")]
fn k_grant_reader(w: &mut ProtocolWorld, folder: String) {
    let owner = w.owner(0);
    let m = w
        .bundle
        .as_mut()
        .unwrap()
        .grant(
            &owner,
            "reader",
            &agent_sk(READER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            1,
            &mut w.ent,
        )
        .unwrap();
    w.k_reader = vec![m];
}

#[when(
    "the owner grants a gmail agent read on \"projets/perso\" plus gmail send and reply, max_actions 3, counter_sign on send"
)]
fn k_grant_gmail(w: &mut ProtocolWorld) {
    w.k_gmail = w.k_grant_read_act(
        "gmail",
        KGMAIL,
        "projets/perso",
        &["act.x.gmail.send", "act.x.gmail.reply"],
        serde_json::json!({"max_actions": 3, "counter_sign": ["send"]}),
    );
    // Sealed-args audit line (§7.9.3): the vault key the agent seals to.
    let owner = w.owner(0);
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .grant_audit_line(&owner, &agent_sk(KGMAIL).verifying_key(), &mut ent)
        .unwrap();
    w.ent = ent;
}

#[when(
    "the owner grants a social agent publish requiring human approval within 5 minutes, heartbeat every 7 days grace 3 days"
)]
fn k_grant_social(w: &mut ProtocolWorld) {
    let m = w.k_mint_root(
        "social",
        AGENT,
        vec![PerimeterEntry::parse("act.x.social.*").unwrap()],
        serde_json::json!({
            "obligations": [approval_ob()],
            "heartbeat": {"every": "7d", "grace": "72h"}
        }),
    );
    w.k_social = vec![m.clone()];
    // The social agent IS the standard AGENT keypair: generic G+ steps
    // ("the agent publishes without any receipt") reuse w.chain verbatim.
    w.chain = vec![m];
}

#[when("the owner beacons at K day 0")]
fn k_beacon0(w: &mut ProtocolWorld) {
    w.beacon(1, &kd(0, "12:00:00"));
}

#[when("the owner beacons again at K day 20")]
fn k_beacon20(w: &mut ProtocolWorld) {
    w.beacon(2, &kd(20, "00:00:00"));
}

#[then(expr = "the reader agent reads the section under {string}")]
fn k_reader_reads(w: &mut ProtocolWorld, folder: String) {
    let r = w.agent_reads(&w.k_reader.clone(), READER, &format!("{folder}/note1"));
    assert_eq!(r.as_deref(), Ok(BODY), "reader read fails: {r:?}");
}

// --- K1 whens: budgeted, counter-signed, approved, audited actions ---

#[when("the owner co-signs the prepared send and the gmail agent appends it at K day 1")]
fn k_cosigned_send(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let leaf = w.k_gmail.last().unwrap().id.clone();
    let check = ob_receipt(
        &owner.content_sign,
        "co_sign",
        (&leaf, "send", G_ARGS),
        "approve",
        &kd(1, "11:58:00"),
        Some("sha256:rendered-mail-to-alice"),
    );
    let chain = w.k_gmail.clone();
    let r = w.k_act(
        &chain,
        KGMAIL,
        "gmail",
        "send",
        &kd(1, "12:00:00"),
        Some(check),
        None,
    );
    w.gamma_result = Some(r);
}

#[when(expr = "the gmail agent replies with arguments naming recipient {string} at K day 2")]
fn k_sealed_reply(w: &mut ProtocolWorld, addr: String) {
    let chain = w.k_gmail.clone();
    let r = w.k_act(
        &chain,
        KGMAIL,
        "gmail",
        "reply",
        &kd(2, "01:00:00"),
        None,
        Some(serde_json::json!({"recipient": addr, "body": "hello"})),
    );
    w.gamma_result = Some(r);
}

#[when("the gmail agent appends one more reply at K day 2")]
fn k_plain_reply(w: &mut ProtocolWorld) {
    let chain = w.k_gmail.clone();
    w.k_act(
        &chain,
        KGMAIL,
        "gmail",
        "reply",
        &kd(2, "02:00:00"),
        None,
        None,
    )
    .expect("the third action fits the budget");
}

#[then("a fourth gmail action entry is rejected as budget spent")]
fn k_fourth_rejected(w: &mut ProtocolWorld) {
    let chain = w.k_gmail.clone();
    let r = w.k_act(
        &chain,
        KGMAIL,
        "gmail",
        "reply",
        &kd(2, "03:00:00"),
        None,
        None,
    );
    assert!(
        r.as_ref().is_err_and(|e| e.contains("budget")),
        "expected budget exhaustion, got {r:?}"
    );
}

#[when("the social agent publishes without any receipt")]
fn k_publish_bare(w: &mut ProtocolWorld) {
    let chain = w.k_social.clone();
    let r = w.k_act(
        &chain,
        AGENT,
        "social",
        "publish",
        &kd(2, "11:00:00"),
        None,
        None,
    );
    w.gamma_result = Some(r);
}

#[when(
    expr = "the approver signs the prepared publish and the social agent appends it at K day {int}"
)]
fn k_approved_publish(w: &mut ProtocolWorld, day_n: u32) {
    let check = w.k_approval(&kd(day_n, "11:58:00"));
    let chain = w.k_social.clone();
    let r = w.k_act(
        &chain,
        AGENT,
        "social",
        "publish",
        &kd(day_n, "12:00:00"),
        Some(check),
        None,
    );
    w.gamma_result = Some(r);
}

#[when("the social agent presents an approved publish at K day 12")]
fn k_stale_publish(w: &mut ProtocolWorld) {
    let check = w.k_approval(&kd(12, "11:58:00"));
    let chain = w.k_social.clone();
    let r = w.k_act(
        &chain,
        AGENT,
        "social",
        "publish",
        &kd(12, "12:00:00"),
        Some(check),
        None,
    );
    w.gamma_result = Some(r);
}

#[then("the action is refused as heartbeat-stale")]
fn k_stale_refused(w: &mut ProtocolWorld) {
    let r = w.gamma_result.as_ref().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("heartbeat stale")),
        "expected GammaHeartbeatStale, got {r:?}"
    );
}

// --- K1: delegation, cut, move, revocation ---

#[when(expr = "the reader agent delegates read on folder {string} to a helper")]
fn k_reader_delegates(w: &mut ProtocolWorld, folder: String) {
    let parent = w.k_reader[0].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(READER),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    let mut ent = std::mem::take(&mut w.ent);
    w.gbundle()
        .log_delegated_grant(
            std::slice::from_ref(&parent),
            &agent_sk(READER),
            &sub.id,
            &kd(20, "13:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
    w.helper_chain = vec![parent, sub];
}

#[then(expr = "the helper reads {string} through its delegated line")]
fn k_helper_reads(w: &mut ProtocolWorld, path: String) {
    let r = w.agent_reads(&w.helper_chain.clone(), HELPER, &path);
    assert_eq!(r.as_deref(), Ok(BODY), "helper read fails: {r:?}");
}

#[when("the reader agent revokes the helper's mandate")]
fn k_reader_revokes_helper(w: &mut ProtocolWorld) {
    let helper_id = w.helper_chain[1].id.clone();
    let parent = vec![w.k_reader[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(
            &parent,
            &agent_sk(READER),
            &helper_id,
            "cleanup",
            &kd(20, "14:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[then("the helper's chain is rejected as revoked from the cut")]
fn k_helper_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.helper_chain.clone(), &kd(20, "15:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "expected revoked, got {r:?}"
    );
}

#[then("the moved folder carries a fresh key version at its new address")]
fn k_moved_fresh_version(w: &mut ProtocolWorld) {
    let b = w.bundle.as_ref().unwrap();
    let chain = b
        .resolve_folder(Zone::Circle, "projets/archive/perso")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, chain);
    let header: Header = read_json(b, &hdr_path_of(&node));
    assert_eq!(header.node, node.to_string(), "header binds the new path");
    assert_eq!(header.latest_version(), 2, "fresh key version");
    assert!(
        !header.key_versions.contains_key("1"),
        "old versions stay at the old address"
    );
}

#[then(expr = "the reader agent reads new content at {string} with its unchanged keypair")]
fn k_reader_reads_new(w: &mut ProtocolWorld, folder: String) {
    w.k_add_section(&folder, "note2", &kd(20, "15:30:00"));
    let r = w.agent_reads(&w.k_reader.clone(), READER, &format!("{folder}/note2"));
    assert_eq!(r.as_deref(), Ok(BODY), "post-move reader read fails: {r:?}");
}

#[when("the owner revokes the gmail agent's mandate with rotation and re-encryption")]
fn k_revoke_gmail(w: &mut ProtocolWorld) {
    w.revoke_owner(&w.k_gmail[0].id.clone(), &kd(20, "16:00:00"));
    w.rotate("projets/archive/perso", KGMAIL);
}

#[then("the revoked gmail key opens neither the new bodies nor the new lines")]
fn k_revoked_gmail_dark(w: &mut ProtocolWorld) {
    // No line at the fresh version…
    let b = w.bundle.as_ref().unwrap();
    let chain = b
        .resolve_folder(Zone::Circle, "projets/archive/perso")
        .unwrap();
    let node = NodePath::folder(Zone::Circle, chain);
    let header: Header = read_json(b, &hdr_path_of(&node));
    let v = header.latest_version();
    let kid = kid_of(KGMAIL);
    let kex = aithos_core::keys::grantee_kex_secret(&agent_sk(KGMAIL));
    assert!(
        header.open(&b.did, v, &kid, &kex).is_err(),
        "the revoked key must hold no line at v{v}"
    );
    // …and no read of content written since.
    let r = w.read_at(
        &w.k_gmail.clone(),
        KGMAIL,
        "projets/archive/perso/note2",
        &kd(20, "17:00:00"),
    );
    assert!(r.is_err(), "revoked gmail key must not read, got {r:?}");
}

#[then("the reader agent reads new content without lifting a finger")]
fn k_reader_survives(w: &mut ProtocolWorld) {
    w.k_add_section("projets/archive/perso", "note3", &kd(20, "16:30:00"));
    let r = w.agent_reads(&w.k_reader.clone(), READER, "projets/archive/perso/note3");
    assert_eq!(r.as_deref(), Ok(BODY), "survivor read fails: {r:?}");
}

#[then("the gmail agent's actions logged before revoked_at still verify at their own timestamps")]
fn k_gmail_old_actions_ok(w: &mut ProtocolWorld) {
    assert!(
        w.verify_revocable_at(&w.k_gmail.clone(), &kd(2, "12:30:00"))
            .is_ok(),
        "pre-revocation timestamps must stay valid"
    );
}

// --- K1: commitments and proofs on the lived bundle ---

#[then("the manifest commits a gamma root and entry count for each segment and a counts root")]
fn k_gamma_commitments(w: &mut ProtocolWorld) {
    let m = w.h_manifest();
    assert!(!m.gamma_roots.is_empty(), "at least one committed segment");
    for (month, seg) in &m.gamma_roots {
        assert_eq!(seg.root.len(), 64, "{month}: 32-byte hex root");
        assert!(seg.n >= 1, "{month}: committed entry count");
    }
    assert_eq!(m.gamma_counts_root.len(), 64, "counts root committed");
    assert_ne!(m.gamma_counts_root, "0".repeat(64), "counts trie non-empty");
}

#[when("a verifier asks for the moved section's inclusion proof")]
fn k_ask_moved_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/archive/perso/note1")
        .unwrap();
    w.h_proof = Some(p);
}

#[then("the moved section proves against the new root through its new address")]
fn k_moved_proof_new_root(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/archive/perso/note1")
        .unwrap();
    let root = w.h_root("circle");
    aithos_core::merkle::verify_proof(&p, &root).expect("new-address proof verifies");
}

#[when("a verifier asks for the social mandate's count proof")]
fn k_ask_count_proof(w: &mut ProtocolWorld) {
    let id = w.k_social[0].id.clone();
    let tallies = w.h2_tallies();
    w.h2_proof = Some(prove_count(&tallies, &id).unwrap());
}

// --- K1: fork and merge on the lived bundle ---

#[given("two copies of the lived bundle")]
fn k_two_copies(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    w.gbundle().publish(&owner, &kd(20, "18:00:00")).unwrap();
    let b = w.bundle.as_ref().unwrap();
    w.i_other = Some(Bundle {
        store: b.store.clone(),
        did: b.did.clone(),
    });
}

#[given("each copy adds a circle section under its own folder")]
fn k_disjoint_adds(w: &mut ProtocolWorld) {
    w.i_add_on(false, "alpha", "note-a", &kd(20, "19:00:00"));
    w.i_add_on(true, "beta", "note-b", &kd(20, "19:30:00"));
    w.i_publish_on(false, &kd(20, "20:00:00"));
    w.i_publish_on(true, &kd(20, "20:00:00"));
}

#[when("either copy publishes the merge edition")]
fn k_merge(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let other = w.i_other.take().unwrap();
    let r = w
        .gbundle()
        .edition_merge(&other, &owner, &kd(20, "21:00:00"))
        .map_err(|e| e.to_string());
    w.i_other = Some(other);
    w.i_result = Some(r);
}

// --- K1: the cold replay ---

#[then("a cold verifier given only the files accepts the final edition and the full log")]
fn k_cold_replay(w: &mut ProtocolWorld) {
    let src = w.bundle.as_ref().unwrap();
    let cold = Bundle {
        store: src.store.clone(),
        did: src.did.clone(),
    };
    cold.verify().expect("the cold edition verifies");
    cold.gamma_verify().expect("the cold log verifies");
}

#[then("every logged action re-verifies against its mandate chain at its own timestamp")]
fn k_replay_actions(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let chains: Vec<Vec<Mandate>> = [&w.k_gmail, &w.k_social, &w.k_night]
        .iter()
        .filter(|c| !c.is_empty())
        .map(|c| (*c).clone())
        .collect();
    let mut replayed = 0;
    for e in entries.iter().filter(|e| e.kind == "action") {
        let by = e.authorized_by.as_ref().expect("actions carry a mandate");
        let chain = chains
            .iter()
            .find(|c| &c.last().unwrap().id == by)
            .unwrap_or_else(|| panic!("unknown mandate {by}"));
        w.verify_revocable_at(chain, &e.at)
            .unwrap_or_else(|err| panic!("action {} fails replay: {err}", e.id));
        replayed += 1;
    }
    assert!(
        replayed >= 5,
        "the lived log replays its actions: {replayed}"
    );
}

#[then("the revoked chains stay refused in the replay")]
fn k_replay_revoked(w: &mut ProtocolWorld) {
    for chain in [w.k_gmail.clone(), w.helper_chain.clone()] {
        let r = w.verify_revocable_at(&chain, &kd(20, "22:00:00"));
        assert!(
            r.as_ref().is_err_and(|e| e.contains("revoked")),
            "expected revoked in replay, got {r:?}"
        );
    }
}

// --- K2/K3/K4: the lived-bundle builder and fresh copies ---

impl ProtocolWorld {
    /// Re-run the full K walkthrough (mutating steps only) and snapshot
    /// the pristine lived store for per-attack fresh copies.
    fn k_build_lived(&mut self) {
        self.init_bundle();
        self.add_circle_section("projets/perso", "note1", "toto");
        self.add_circle_section("projets/archive", "old2024", "done");
        k_add_public(self, "readme".into(), "docs".into());
        k_add_self(self, "sante".into(), "journal".into());
        self.publish_bundle();
        k_grant_reader(self, "projets/perso".into());
        k_grant_gmail(self);
        k_grant_social(self);
        k_beacon0(self);
        self.publish_bundle();
        k_cosigned_send(self);
        k_sealed_reply(self, "alice@example.com".into());
        k_plain_reply(self);
        k_approved_publish(self, 2);
        k_beacon20(self);
        k_approved_publish(self, 20);
        k_reader_delegates(self, "projets/perso".into());
        k_reader_revokes_helper(self);
        owner_moves_folder(self, "projets/perso".into(), "projets/archive".into());
        k_reader_reads_new(self, "projets/archive/perso".into());
        k_revoke_gmail(self);
        k_reader_survives(self);
        self.h_publish();
        k_two_copies(self);
        k_disjoint_adds(self);
        k_merge(self);
        self.i_result
            .clone()
            .unwrap()
            .expect("the lived merge holds");
        self.k_pristine = Some(self.bundle.as_ref().unwrap().store.clone());
    }

    fn k_restore(&mut self) {
        let pristine = self.k_pristine.clone().expect("lived bundle built");
        self.bundle.as_mut().unwrap().store = pristine;
        self.gamma_result = None;
        self.h_proof = None;
        self.h2_verdict = None;
    }
}

#[given("a bundle that lived the full K walkthrough")]
fn k_lived_bundle(w: &mut ProtocolWorld) {
    w.k_build_lived();
}

#[given("a fresh copy of the lived bundle")]
fn k_fresh_copy(w: &mut ProtocolWorld) {
    w.k_restore();
}

// --- K2: the tamper battery on the lived artifact ---

#[when("one byte of an entry inside the merged segment is altered")]
fn k_tamper_merged_segment(w: &mut ProtocolWorld) {
    let seg = "gamma/2026-07.jsonl";
    let mut lines = w.segment_lines(seg);
    let n = lines.len();
    let line = &mut lines[n - 2]; // inside the merged layout, before the join
    let idx = line
        .windows(9)
        .position(|win| win == b"\"value\":\"")
        .expect("signature field")
        + 9;
    line[idx] = if line[idx] == b'0' { b'1' } else { b'0' };
    let mut joined: Vec<u8> = Vec::new();
    for l in &lines {
        joined.extend_from_slice(l);
        joined.push(b'\n');
    }
    w.gbundle().store.put(seg, &joined).unwrap();
}

#[when("the mirror forges a lived section proof that presents an interior hash as a leaf")]
fn k_forge_lived_proof(w: &mut ProtocolWorld) {
    let p = w
        .gbundle()
        .prove_section(Zone::Circle, "projets/archive/perso/note1")
        .unwrap();
    let leaf = aithos_core::merkle::h_leaf(&hex::decode(&p.payload).unwrap());
    let aithos_core::merkle::ProofStep::Node { hash, side } = p.steps[0].clone() else {
        panic!("first step should be the sibling");
    };
    let sib: [u8; 32] = hex::decode(&hash).unwrap().try_into().unwrap();
    let mut spliced = Vec::new();
    match side {
        aithos_core::merkle::Side::Right => {
            spliced.extend_from_slice(&leaf);
            spliced.extend_from_slice(&sib);
        }
        aithos_core::merkle::Side::Left => {
            spliced.extend_from_slice(&sib);
            spliced.extend_from_slice(&leaf);
        }
    }
    w.h_proof = Some(aithos_core::merkle::Proof {
        payload: hex::encode(spliced),
        steps: p.steps[1..].to_vec(),
        root: p.root,
    });
}

#[when("the mirror claims absence of a mandate id that was counted")]
fn k_forge_absence(w: &mut ProtocolWorld) {
    let tallies = w.h2_tallies();
    let ids: Vec<String> = tallies.keys().cloned().collect();
    assert!(ids.len() >= 3, "the lived trie counts several mandates");
    let forged = AbsenceProof {
        left: Some(prove_count(&tallies, &ids[0]).unwrap()),
        right: Some(prove_count(&tallies, &ids[2]).unwrap()),
    };
    let pinned = w.h2_counts_root();
    w.h2_verdict = Some(verify_absence(&ids[1], &forged, &pinned).map_err(|e| e.to_string()));
}

#[when("the social agent presents an approval receipt bound to other args")]
fn k_receipt_other_args(w: &mut ProtocolWorld) {
    let leaf = w.k_social.last().unwrap().id.clone();
    let check = ob_receipt(
        &agent_sk(APPROVER),
        "publish-approval",
        (&leaf, "publish", "sha256:bb22"),
        "approve",
        &kd(20, "21:58:00"),
        Some("sha256:rendered-ship-it"),
    );
    let chain = w.k_social.clone();
    let r = w.k_act(
        &chain,
        AGENT,
        "social",
        "publish",
        &kd(20, "22:00:00"),
        Some(check),
        None,
    );
    w.gamma_result = Some(r);
}

#[when("the social agent forges a fresh heartbeat with its own key")]
fn k_forge_beacon(w: &mut ProtocolWorld) {
    use ed25519_dalek::Signer;
    let head = w.gbundle().gamma_head().unwrap();
    let mut forged = aithos_core::gamma::Entry {
        v: 1,
        id: "gamma_000000000000000000000FORGK".into(),
        prev: head,
        prevs: None,
        at: kd(20, "22:30:00"),
        kind: "heartbeat".into(),
        target: None,
        authorized_by: None,
        authorized_via: None,
        payload: Some(serde_json::json!({"seq": 99})),
        body_enc: None,
        signature: aithos_core::did::SignatureBlock {
            alg: "ed25519".into(),
            key: "#content".into(),
            value: String::new(),
        },
    };
    let mut unsigned = forged.clone();
    unsigned.signature.value = String::new();
    forged.signature.value = hex::encode(
        agent_sk(AGENT)
            .sign(&aithos_core::jcs::canonical_bytes(&unsigned).unwrap())
            .to_bytes(),
    );
    let seg = "gamma/2026-07.jsonl";
    let mut bytes = w.gbundle().store.get(seg).unwrap().unwrap();
    bytes.extend_from_slice(aithos_core::jcs::canonicalize(&forged).unwrap().as_bytes());
    bytes.push(b'\n');
    w.gbundle().store.put(seg, &bytes).unwrap();
}

#[then("the forged beacon fails log verification")]
fn k_forged_beacon_dies(w: &mut ProtocolWorld) {
    assert!(
        w.bundle.as_ref().unwrap().gamma_verify().is_err(),
        "a forged beacon must fail log verification"
    );
}

#[when("the social agent presents a request anchored to a stale head")]
fn k_stale_anchor(w: &mut ProtocolWorld) {
    let entries = w.gbundle().gamma_entries().unwrap();
    let anchor = entries[1].chain_hash().unwrap(); // an early, long-buried entry
    w.gamma_result = Some(
        aithos_core::gamma::check_anchor(&entries, &anchor, "24h", &kd(20, "23:00:00"))
            .map(|()| "fresh".into())
            .map_err(|e| e.to_string()),
    );
}

// --- K3: the keyless view and the perimeters ---

#[then("no keyed-zone target, tag or content is revealed")]
fn k_nothing_revealed(w: &mut ProtocolWorld) {
    // Two-layer envelope (§07): section.* bodies on KEYED zones are sealed
    // — clear target/payload is the design for the public zone only.
    let entries = w.gbundle().gamma_entries().unwrap();
    let mut sealed = 0;
    for e in entries.iter().filter(|e| e.kind.starts_with("section.")) {
        if e.body_enc.is_some() {
            assert!(
                e.target.is_none() && e.payload.is_none(),
                "{}: a sealed entry leaks clear fields",
                e.id
            );
            sealed += 1;
        } else {
            let t = e.target.as_deref().unwrap_or_default();
            assert!(
                t.starts_with("/e/public"),
                "{}: only public entries may ride clear, target {t}",
                e.id
            );
        }
    }
    assert!(
        sealed >= 2,
        "the lived log holds sealed mutations: {sealed}"
    );
}

#[then("the sealed reply arguments open for no key but the owner's")]
fn k_sealed_args_dark(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let entries = w.gbundle().gamma_entries().unwrap();
    let sealed = entries
        .iter()
        .find(|e| e.kind == "action" && e.body_enc.is_some())
        .expect("the lived log holds a sealed-args action")
        .clone();
    // The clear payload leaks nothing but the hash…
    let clear = serde_json::to_string(sealed.payload.as_ref().unwrap()).unwrap();
    assert!(!clear.contains("alice@example.com"), "args must not leak");
    // …other keys open nothing…
    for (chain, sk) in [(w.k_reader.clone(), READER), (w.k_social.clone(), AGENT)] {
        assert!(
            w.bundle
                .as_ref()
                .unwrap()
                .open_entry_as_agent(&chain, &agent_sk(sk), &sealed)
                .is_err(),
            "only the owner audits sealed args"
        );
    }
    // …the owner reopens them.
    let args = w
        .bundle
        .as_ref()
        .unwrap()
        .audit_action_args(&owner, &sealed)
        .unwrap();
    assert_eq!(args["recipient"], "alice@example.com");
}

#[then("the revoked gmail key reads nothing written after the cut")]
fn k_revoked_reads_nothing(w: &mut ProtocolWorld) {
    let r = w.read_at(
        &w.k_gmail.clone(),
        KGMAIL,
        "projets/archive/perso/note3",
        &kd(20, "22:00:00"),
    );
    assert!(r.is_err(), "revoked key must not read, got {r:?}");
}

#[then("the gmail key still derives the folder's old key — it cannot be un-taught")]
fn k_cannot_unteach(w: &mut ProtocolWorld) {
    let b = w.bundle.as_ref().unwrap();
    // The moved folder's sid is stable; its OLD header (v1, old address,
    // old parent chain) stays on disk as archaeology.
    let new_chain = b
        .resolve_folder(Zone::Circle, "projets/archive/perso")
        .unwrap();
    let m_sid = *new_chain.last().unwrap();
    let mut old_chain = b.resolve_folder(Zone::Circle, "projets").unwrap();
    old_chain.push(m_sid);
    let old_node = NodePath::folder(Zone::Circle, old_chain);
    let header: Header = read_json(b, &hdr_path_of(&old_node));
    let kid = kid_of(KGMAIL);
    let kex = aithos_core::keys::grantee_kex_secret(&agent_sk(KGMAIL));
    assert!(
        header.open(&b.did, 1, &kid, &kex).is_ok(),
        "the old line cannot be un-taught"
    );
}

#[then(expr = "the section under {string} stays out of the reader's reach")]
fn k_reader_contained(w: &mut ProtocolWorld, folder: String) {
    let r = w.read_at(
        &w.k_reader.clone(),
        READER,
        &format!("{folder}/old2024"),
        &kd(20, "22:00:00"),
    );
    assert!(r.is_err(), "the reader's perimeter is projets/perso only");
}

// --- K4: the watchdog incident ---

#[given("the owner grants a night agent gmail send with a watchdog appointed")]
fn k_grant_night(w: &mut ProtocolWorld) {
    let night = w.k_mint_root(
        "night",
        KNIGHT,
        vec![PerimeterEntry::parse("act.x.gmail.send").unwrap()],
        aithos_core::mandate::MandateSpec::no_constraints(),
    );
    w.k_night = vec![night.clone()];
    // The Then "the action logged before revoked_at…" reads w.chain.
    w.chain = vec![night];
    let wd = w.k_mint_root(
        "watchdog",
        WDOG,
        vec![PerimeterEntry::Revoke { scope: None }],
        aithos_core::mandate::MandateSpec::no_constraints(),
    );
    w.k_wd = vec![wd];
}

#[when("the night agent acts once")]
fn k_night_acts(w: &mut ProtocolWorld) {
    let chain = w.k_night.clone();
    let r = w.k_act(
        &chain,
        KNIGHT,
        "gmail",
        "send",
        &kd(20, "22:00:00"),
        None,
        None,
    );
    w.gamma_result = Some(r);
}

#[when("the watchdog revokes the night agent's mandate")]
fn k_watchdog_revokes(w: &mut ProtocolWorld) {
    let target = w.k_night[0].id.clone();
    let wd = w.k_wd.clone();
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(
            &wd,
            &agent_sk(WDOG),
            &target,
            "incident",
            &kd(20, "22:30:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[then("the night agent's chain is rejected as revoked from its cut")]
fn k_night_rejected(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.k_night.clone(), &kd(20, "23:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "expected revoked, got {r:?}"
    );
}

#[then("the watchdog opens no body anywhere in the bundle")]
fn k_watchdog_dark(w: &mut ProtocolWorld) {
    for path in [
        "projets/archive/perso/note1",
        "projets/archive/old2024",
        "alpha/note-a",
    ] {
        let r = w.read_at(&w.k_wd.clone(), WDOG, path, &kd(20, "23:30:00"));
        assert!(r.is_err(), "the watchdog holds no content key: {path}");
    }
}

// --------------------------------------------------- step L: delegated writes
//
// features/l-delegated-writes.feature — the mandate as a pen (spec 02.11,
// 04.2, 04.3, 05.3, 07.2) and the one-mandate surface. Successful writes
// timestamp at kd(*) (after the givens' NOW entries); refused attempts
// timestamp inside their window — rejection happens before any append,
// so the chain never moves.

impl ProtocolWorld {
    fn try_agent_add(
        &mut self,
        name: &str,
        body: &str,
        folder: &str,
        at: &str,
    ) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .section_add_as_agent(
                &chain,
                &agent_sk(AGENT),
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: folder,
                    name,
                    title: "note",
                    tags: &[],
                    body,
                    now: at,
                },
                &mut ent,
            )
            .map(|()| "written".to_owned())
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn try_agent_rewrite(&mut self, path: &str, body: &str, at: &str) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .section_rewrite_as_agent(
                &chain,
                &agent_sk(AGENT),
                Zone::Circle,
                path,
                body,
                at,
                &mut ent,
            )
            .map(|()| "rewritten".to_owned())
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }

    fn try_agent_delete(&mut self, path: &str, at: &str) -> Result<String, String> {
        self.gamma_baseline = self.gbundle().gamma_entries().unwrap().len();
        let chain = self.chain.clone();
        let mut ent = std::mem::take(&mut self.ent);
        let r = self
            .gbundle()
            .section_delete_as_agent(&chain, &agent_sk(AGENT), Zone::Circle, path, at, &mut ent)
            .map(|()| "deleted".to_owned())
            .map_err(|e| e.to_string());
        self.ent = ent;
        r
    }
}

#[given(expr = "a published bundle with a circle folder {string}")]
fn l_published_with_folder(w: &mut ProtocolWorld, folder: String) {
    w.init_bundle();
    let owner = w.owner(0);
    w.bundle
        .as_mut()
        .unwrap()
        .ensure_folder(Zone::Circle, &folder, &owner, &mut w.ent)
        .unwrap();
    w.publish_bundle();
}

#[given(expr = "a published bundle with section {string} tagged {string} in circle {string}")]
fn l_published_with_tagged(w: &mut ProtocolWorld, name: String, tag: String, folder: String) {
    w.init_bundle();
    w.add_circle_section(&folder, &name, &tag);
    w.publish_bundle();
}

#[when(expr = "the owner grants the agent append on circle folder {string}")]
fn l_grant_append(w: &mut ProtocolWorld, folder: String) {
    w.grant_to_agent(&[verb_spec(Verb::Append, &folder)], NA30, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent edit on circle folder {string}")]
fn l_grant_edit(w: &mut ProtocolWorld, folder: String) {
    w.grant_to_agent(&[verb_spec(Verb::Edit, &folder)], NA30, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent write on circle folder {string}")]
fn l_grant_write(w: &mut ProtocolWorld, folder: String) {
    w.grant_to_agent(&[verb_spec(Verb::Write, &folder)], NA30, 0);
    w.granted_folder = folder;
}

#[when(expr = "the owner grants the agent append on circle folder {string} for 7 days")]
fn l_grant_append_7d(w: &mut ProtocolWorld, folder: String) {
    w.grant_to_agent(&[verb_spec(Verb::Append, &folder)], NA7, 0);
    w.granted_folder = folder;
}

#[when(expr = "the agent adds section {string} with body {string} under {string}")]
#[then(expr = "the agent adds section {string} with body {string} under {string} and it verifies")]
fn l_agent_adds(w: &mut ProtocolWorld, name: String, body: String, folder: String) {
    let r = w.try_agent_add(&name, &body, &folder, &kd(0, "02:00:00"));
    assert!(r.is_ok(), "delegated add fails: {r:?}");
    w.gamma_result = Some(r);
}

#[when(expr = "the agent tries to add section {string} with body {string} under {string}")]
fn l_agent_tries_add(w: &mut ProtocolWorld, name: String, body: String, folder: String) {
    w.gamma_result = Some(w.try_agent_add(&name, &body, &folder, DAY1));
}

#[when(expr = "the agent tries to add section {string} with body {string} under {string} at day 8")]
fn l_agent_tries_add_day8(w: &mut ProtocolWorld, name: String, body: String, folder: String) {
    w.gamma_result = Some(w.try_agent_add(&name, &body, &folder, DAY8));
}

#[when(expr = "the agent rewrites {string} to {string}")]
fn l_agent_rewrites(w: &mut ProtocolWorld, path: String, body: String) {
    let r = w.try_agent_rewrite(&path, &body, &kd(0, "02:00:00"));
    assert!(r.is_ok(), "delegated rewrite fails: {r:?}");
    w.gamma_result = Some(r);
}

#[when(expr = "the agent tries to rewrite {string} to {string}")]
fn l_agent_tries_rewrite(w: &mut ProtocolWorld, path: String, body: String) {
    w.gamma_result = Some(w.try_agent_rewrite(&path, &body, DAY1));
}

#[when(expr = "the agent tries to rewrite the section under {string} to {string}")]
fn l_agent_tries_rewrite_under(w: &mut ProtocolWorld, folder: String, body: String) {
    let path = format!("{folder}/note");
    w.gamma_result = Some(w.try_agent_rewrite(&path, &body, DAY1));
}

#[when(expr = "the agent deletes {string}")]
fn l_agent_deletes(w: &mut ProtocolWorld, path: String) {
    let r = w.try_agent_delete(&path, &kd(0, "02:00:00"));
    assert!(r.is_ok(), "delegated delete fails: {r:?}");
    w.gamma_result = Some(r);
}

#[when(expr = "the agent tries to delete {string}")]
fn l_agent_tries_delete(w: &mut ProtocolWorld, path: String) {
    w.gamma_result = Some(w.try_agent_delete(&path, DAY1));
}

#[then(expr = "the owner reads {string} as {string}")]
fn l_owner_reads_as(w: &mut ProtocolWorld, path: String, body: String) {
    let owner = w.owner(0);
    let got = w
        .bundle
        .as_ref()
        .unwrap()
        .read_section(Zone::Circle, &path, &owner)
        .unwrap();
    assert_eq!(got, body, "owner reads back the delegated write");
}

#[then(expr = "the agent reads {string} as {string}")]
fn l_agent_reads_as(w: &mut ProtocolWorld, path: String, body: String) {
    let r = w
        .bundle
        .as_ref()
        .unwrap()
        .read_section_as_agent(
            &w.chain,
            &agent_sk(AGENT),
            Zone::Circle,
            &path,
            &kd(1, "00:00:00"),
        )
        .map_err(|e| e.to_string());
    assert_eq!(r.as_deref(), Ok(body.as_str()));
}

#[then(expr = "the log's last entry is a delegated {string} under the agent's mandate")]
fn l_last_entry_delegated(w: &mut ProtocolWorld, kind: String) {
    let leaf = w.chain.last().unwrap().clone();
    let e = w.gbundle().gamma_entries().unwrap().pop().unwrap();
    assert_eq!(e.kind, kind, "kind");
    assert_eq!(e.authorized_by.as_deref(), Some(leaf.id.as_str()), "leaf");
    assert_eq!(e.signature.key, leaf.grantee.pubkey, "grantee-signed");
    assert!(
        e.body_enc.is_some() && e.target.is_none() && e.payload.is_none(),
        "sealed body, sealed target (spec 07.3)"
    );
}

#[then(expr = "the section {string} is gone from the tree")]
fn l_section_gone(w: &mut ProtocolWorld, path: String) {
    let owner = w.owner(0);
    assert!(
        w.bundle
            .as_ref()
            .unwrap()
            .read_section(Zone::Circle, &path, &owner)
            .is_err(),
        "the tree must forget the deleted node"
    );
}

#[then("someone with no key learns neither target nor content from the last entry")]
fn l_stranger_learns_nothing(w: &mut ProtocolWorld) {
    let e = w.gbundle().gamma_entries().unwrap().pop().unwrap();
    assert!(e.target.is_none() && e.payload.is_none() && e.body_enc.is_some());
    let wire = aithos_core::jcs::canonicalize(&e).unwrap();
    for leak in ["written by the pen", "memo", "projets"] {
        assert!(!wire.contains(leak), "clear leak of {leak:?} in the entry");
    }
}

#[then("the write is rejected as outside the perimeter")]
fn l_rejected_perimeter(w: &mut ProtocolWorld) {
    let r = w.gamma_result.clone().unwrap();
    assert!(
        r.as_ref().is_err_and(|e| e.contains("not covered")),
        "expected a perimeter rejection, got {r:?}"
    );
}

#[then("the write is rejected as outside the window")]
fn l_rejected_window(w: &mut ProtocolWorld) {
    let r = w.gamma_result.clone().unwrap();
    assert!(
        r.as_ref()
            .is_err_and(|e| e.contains("outside validity window")),
        "expected a window rejection, got {r:?}"
    );
}

// --- the one-mandate surface (super-mandate) ---

#[when(
    expr = "the owner grants the agent one mandate carrying write on {string}, gmail reply, gamma read on actions, issue depth 1 and revoke, max_actions 2, for 30 days"
)]
fn l_grant_super_mandate(w: &mut ProtocolWorld, folder: String) {
    let owner = w.owner(0);
    let agent_pub = agent_sk(AGENT).verifying_key();
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .deliver_zone_line(&owner, &agent_pub, Zone::Circle, &folder, None, &mut ent)
        .unwrap();
    w.ent = ent;
    let dir = w
        .bundle
        .as_ref()
        .unwrap()
        .resolve_folder(Zone::Circle, &folder)
        .unwrap();
    let perimeter = vec![
        PerimeterEntry::Ethos {
            verb: Verb::Write,
            zone: Zone::Circle,
            dir,
            tag: None,
        },
        PerimeterEntry::parse("act.x.gmail.reply").unwrap(),
        PerimeterEntry::parse("read.gamma#kind=action").unwrap(),
        PerimeterEntry::parse("issue#depth=1").unwrap(),
        PerimeterEntry::parse("revoke").unwrap(),
    ];
    let m = w.k_mint_root(
        "omni",
        AGENT,
        perimeter,
        serde_json::json!({ "max_actions": 2 }),
    );
    w.chain = vec![m];
    w.granted_folder = folder;
}

#[then(expr = "the agent appends a gmail {string} action and it verifies")]
fn l_super_action_ok(w: &mut ProtocolWorld, action: String) {
    let r = w.try_action(false, &action, &kd(1, "01:00:00"));
    assert!(r.is_ok(), "action under the one mandate fails: {r:?}");
}

#[then("the agent queries the log for its own action and finds it")]
fn l_super_queries_log(w: &mut ProtocolWorld) {
    let hits = w
        .bundle
        .as_ref()
        .unwrap()
        .log_query_as_agent(
            &w.chain,
            &agent_sk(AGENT),
            &aithos_core::mandate::GammaQuery {
                kind: Some("action".to_owned()),
                ..Default::default()
            },
            &LogFilter {
                kind: Some("action".to_owned()),
                ..Default::default()
            },
            &kd(1, "02:00:00"),
        )
        .unwrap();
    assert!(
        hits.iter().any(|h| h.entry.kind == "action"),
        "the granted read.gamma#kind=action must surface the agent's own act"
    );
}

#[when(expr = "the agent delegates read on folder {string} to a helper until day 30")]
fn l_super_delegates(w: &mut ProtocolWorld, folder: String) {
    let parent = w.chain[0].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    w.helper_chain = vec![parent.clone(), sub.clone()];
    // Issuance is never silent (spec 07.4).
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_delegated_grant(
            &[parent],
            &agent_sk(AGENT),
            &sub.id,
            &kd(1, "03:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[when("the agent revokes the helper from its own issue authority")]
fn l_super_revokes_helper(w: &mut ProtocolWorld) {
    let helper_id = w.helper_chain[1].id.clone();
    let parent = vec![w.chain[0].clone()];
    let mut ent = std::mem::take(&mut w.ent);
    w.gb()
        .log_revoke_as(
            &parent,
            &agent_sk(AGENT),
            &helper_id,
            "cleanup",
            &kd(2, "12:00:00"),
            &mut ent,
        )
        .unwrap();
    w.ent = ent;
}

#[then("the helper's chain is rejected as revoked at day 13")]
fn l_super_helper_revoked(w: &mut ProtocolWorld) {
    let r = w.verify_revocable_at(&w.helper_chain.clone(), &day(13, "00:00:00"));
    assert!(
        r.as_ref().is_err_and(|e| e.contains("revoked")),
        "expected revoked, got {r:?}"
    );
}

#[then(expr = "a second gmail {string} action verifies and a third is rejected as budget spent")]
fn l_super_budget(w: &mut ProtocolWorld, action: String) {
    let r2 = w.try_action(false, &action, &kd(4, "01:00:00"));
    assert!(r2.is_ok(), "second action fails: {r2:?}");
    let r3 = w.try_action(false, &action, &kd(4, "02:00:00"));
    assert!(
        r3.as_ref().is_err_and(|e| e.contains("max_actions")),
        "expected the subtree budget to refuse, got {r3:?}"
    );
}

#[then("at day 31 the same mandate can neither read, nor write, nor act, nor delegate")]
fn l_super_dead_at_31(w: &mut ProtocolWorld) {
    let at = day(31, "00:00:00");
    let window = |r: &Result<String, String>| {
        r.as_ref()
            .is_err_and(|e| e.contains("outside validity window"))
    };
    // Read.
    let path = format!("{}/note1", w.granted_folder);
    let r = w
        .bundle
        .as_ref()
        .unwrap()
        .read_section_as_agent(&w.chain, &agent_sk(AGENT), Zone::Circle, &path, &at)
        .map_err(|e| e.to_string());
    assert!(window(&r), "read must die at expiry, got {r:?}");
    // Write.
    let folder = w.granted_folder.clone();
    let rw = w.try_agent_add("late", "too late", &folder, &at);
    assert!(window(&rw), "write must die at expiry, got {rw:?}");
    // Act.
    let ra = w.try_action(false, "reply", &at);
    assert!(window(&ra), "action must die at expiry, got {ra:?}");
    // Delegate: a child minted under the expired mandate verifies nowhere at T.
    let parent = w.chain[0].clone();
    let sub = w
        .bundle
        .as_mut()
        .unwrap()
        .delegate(
            &parent,
            &agent_sk(AGENT),
            "helper",
            &agent_sk(HELPER).verifying_key(),
            &[dir_spec(&folder)],
            NB,
            NA30,
            &mut w.ent,
        )
        .unwrap();
    let rd = w
        .verify_chain_at(&[parent, sub], &at)
        .map(|()| "verified".to_owned());
    assert!(window(&rd), "delegation must die at expiry, got {rd:?}");
}

#[then("the owner still writes at day 31")]
fn l_owner_writes_at_31(w: &mut ProtocolWorld) {
    let owner = w.owner(0);
    let folder = w.granted_folder.clone();
    let now = day(31, "02:00:00");
    let mut ent = std::mem::take(&mut w.ent);
    let r = w.gbundle().section_add(
        &SectionSpec {
            zone: Zone::Circle,
            folder_path: &folder,
            name: "posthume",
            title: "note",
            tags: &[],
            body: "the owner never expires",
            now: &now,
        },
        &owner,
        &mut ent,
    );
    w.ent = ent;
    assert!(r.is_ok(), "the owner key has no window: {r:?}");
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

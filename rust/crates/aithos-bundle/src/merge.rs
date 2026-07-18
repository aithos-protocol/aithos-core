//! Concurrency (spec §02.6 + §07.6, pass I): deterministic disjoint merge,
//! fork detection, and fork resolution by the nearest common manager.
//!
//! The wire is graved: a merge edition's `prev_hash` pins the parent with
//! the LOWEST edition hash and `merges` lists both ascending; shared index
//! files merge **3-way by sid** (changed rows from their branch, additions
//! unioned, deletions hold, the same sid changed on both sides IS a fork);
//! the merged log lays out the shared prefix, sub-chain LOW, sub-chain
//! HIGH, then the signed two-predecessor `merge` entry — existing bytes
//! never rewritten, §07.10 roots recommitted. Every merger reproduces the
//! same bytes; every verifier recomputes the same verdicts from the files.

use std::collections::{BTreeMap, BTreeSet};

use aithos_core::concurrency::{
    recompose_counts, verify_disjoint_merge, verify_fork_resolution, MergeAuthority,
    SemanticCounts, SemanticOccurrence,
};
use aithos_core::did::DidDocument;
use aithos_core::error::{Error, Result};
use aithos_core::gamma::{self, delegated_entry, owner_entry, EntrySpec, Kind};
use aithos_core::ids::Sid;
use aithos_core::keys::OwnerKeys;
use aithos_core::mandate::{covers_op, verify_chain, Mandate, Op, PerimeterEntry, Verb};
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;

use crate::bundle::{Bundle, SelfIndex, ZoneIndex};
use crate::manifest::{sha256_hex, Manifest, ManifestSigner, ManifestSpec};
use crate::publication::{
    assemble_draft2_candidate, cold_verify, export_keyless, KeylessPublicationPackage,
};
use crate::state::{tree_diff, StateTree};
use crate::Store;
use aithos_core::carriers::{K1cActor, K1cVerificationContext, VerifiedK1cCarriers};
use serde_json::Value;

/// Derivation domain of the deterministic merge-entry id: both mergers must
/// produce byte-identical entries, so the id derives from what they share.
const MERGE_ID_DOMAIN: &[u8] = b"aithos-core/v1/merge-entry-id";

fn io_err(e: std::io::Error) -> Error {
    Error::SealRejected(format!("store i/o: {e}"))
}

// ------------------------------------------------------------ pure helpers

/// 3-way row merge by sid (§02.6, graved): base = the common ancestor's
/// rows; a row changed on one branch is taken from that branch; additions
/// union; deletions hold (no resurrection); the SAME sid changed on both
/// sides is a same-node conflict. The result is sorted by sid — with JCS
/// that makes every merger byte-identical.
fn merge_rows<T: Clone + PartialEq>(
    base: &[T],
    a: &[T],
    b: &[T],
    sid_of: impl Fn(&T) -> &str,
) -> Result<Vec<T>> {
    let index = |rows: &[T]| -> BTreeMap<String, T> {
        rows.iter()
            .map(|r| (sid_of(r).to_owned(), r.clone()))
            .collect()
    };
    let (base_rows, a_rows, b_rows) = (index(base), index(a), index(b));
    let mut sids: BTreeSet<String> = BTreeSet::new();
    sids.extend(base_rows.keys().cloned());
    sids.extend(a_rows.keys().cloned());
    sids.extend(b_rows.keys().cloned());
    let mut out = Vec::new();
    for sid in sids {
        let base_row = base_rows.get(&sid);
        let (ra, rb) = (a_rows.get(&sid), b_rows.get(&sid));
        // "changed" covers edits, additions AND deletions vs the base.
        let a_changed = ra != base_row;
        let b_changed = rb != base_row;
        let row = match (a_changed, b_changed) {
            (true, true) if ra != rb => {
                return Err(Error::EditionFork(format!(
                    "same-node conflict on sid {sid}"
                )));
            }
            (true, _) => ra, // None = deleted on A — the deletion holds
            (false, true) => rb,
            (false, false) => base_row,
        };
        if let Some(r) = row {
            out.push(r.clone());
        }
    }
    Ok(out)
}

/// 3-way merge of a hierarchical zone index (§02.6): folders and sections,
/// each by sid, result sid-sorted.
pub fn merge_zone_index(base: &ZoneIndex, a: &ZoneIndex, b: &ZoneIndex) -> Result<ZoneIndex> {
    Ok(ZoneIndex {
        folders: merge_rows(&base.folders, &a.folders, &b.folders, |r| &r.sid)?,
        sections: merge_rows(&base.sections, &a.sections, &b.sections, |r| &r.sid)?,
    })
}

/// 3-way merge of the flat `self` index (§02.6), by sid.
pub fn merge_self_index(base: &SelfIndex, a: &SelfIndex, b: &SelfIndex) -> Result<SelfIndex> {
    Ok(SelfIndex {
        blobs: merge_rows(&base.blobs, &a.blobs, &b.blobs, |r| &r.sid)?,
    })
}

/// Deterministic merged segment layout (§07.6, graved): the common byte
/// prefix (the shared history), then sub-chain LOW's remaining lines, then
/// sub-chain HIGH's — existing lines byte-identical, never rewritten. The
/// merge entry itself is appended by the caller (it may land in a later
/// month's segment).
#[must_use]
pub fn merge_segment_lines(lo: &[Vec<u8>], hi: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let prefix = lo.iter().zip(hi.iter()).take_while(|(a, b)| a == b).count();
    let mut out: Vec<Vec<u8>> = lo.to_vec();
    out.extend(hi[prefix..].iter().cloned());
    out
}

/// The frontier of a root-descent diff (§02.6): the touched node labels
/// with every purely-induced ancestor dropped — a folder (or zone root)
/// that only changed because a descendant did is not "touched". Two
/// changesets are disjoint iff their frontiers do not intersect.
#[must_use]
pub fn frontier(diff: &BTreeMap<String, &'static str>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for l in diff.keys() {
        let deeper = format!("{l}/");
        // Zone-root labels (`<zone>:z`) do not textually prefix their
        // children — any other change in the zone makes them induced.
        let zone_prefix = l.strip_suffix(":z").map(|z| format!("{z}:"));
        let induced = diff.keys().any(|m| {
            m != l
                && (m.starts_with(deeper.as_str())
                    || zone_prefix
                        .as_ref()
                        .is_some_and(|p| m.starts_with(p.as_str())))
        });
        if !induced {
            out.insert(l.clone());
        }
    }
    out
}

/// Map a state-tree label to the (zone, folder sid-chain) a WRITE authority
/// check runs against (§04.2 nodal coverage). Section and tag-view labels
/// resolve to their folder chain; `self`/`vault` labels return `None` —
/// only the owner resolves those this pass.
fn label_zone_chain(label: &str) -> Option<(Zone, Vec<Sid>)> {
    let (zone_str, rest) = label.split_once(':')?;
    let zone = Zone::parse(zone_str).ok()?;
    if zone == Zone::Self_ {
        return None;
    }
    if rest == "z" {
        return Some((zone, Vec::new()));
    }
    let mut chain = Vec::new();
    let mut parts = rest.split('/');
    while let Some(kind) = parts.next() {
        let value = parts.next()?;
        match kind {
            "d" => chain.push(Sid::parse(value).ok()?),
            "s" | "t" => return Some((zone, chain)), // leaf: authority = its folder chain
            _ => return None,
        }
    }
    Some((zone, chain))
}

/// Does the perimeter carry WRITE authority over every touched label?
fn write_covers_labels(perimeter: &[PerimeterEntry], labels: &BTreeSet<String>) -> bool {
    labels.iter().all(|label| {
        label_zone_chain(label).is_some_and(|(zone, chain)| {
            covers_op(
                perimeter,
                &Op {
                    verb: Verb::Write,
                    zone,
                    folders: &chain,
                    tags: &[],
                },
            )
        })
    })
}

/// Who signs a resolving edition (§02.6): the owner root, or the nearest
/// common manager under its mandate chain.
pub enum ForkResolver<'a> {
    Owner(&'a OwnerKeys),
    Delegate {
        chain: &'a [Mandate],
        sk: &'a SigningKey,
    },
}

/// Public, secret-free qualification inputs for one draft.2 disjoint merge.
pub struct Draft2MergePlan {
    pub parents: [String; 2],
    pub left_changed_sids: BTreeSet<String>,
    pub right_changed_sids: BTreeSet<String>,
    pub deleted_sids: BTreeSet<String>,
    pub authority: MergeAuthority,
    pub left_occurrences: Vec<SemanticOccurrence>,
    pub right_occurrences: Vec<SemanticOccurrence>,
}

/// Public, secret-free qualification inputs for one fork resolution.
pub struct Draft2ResolutionPlan {
    pub parents: [String; 2],
    pub winner: String,
    pub touched_sids: BTreeSet<String>,
    pub authority: MergeAuthority,
    pub left_occurrences: Vec<SemanticOccurrence>,
    pub right_occurrences: Vec<SemanticOccurrence>,
}

fn context_changed_sids(context: &K1cVerificationContext) -> BTreeSet<String> {
    context
        .change_causes
        .keys()
        .filter_map(|path| {
            path.strip_prefix("public/sections/")
                .and_then(|name| name.strip_suffix(".md"))
                .or_else(|| {
                    path.strip_prefix("circle/blobs/")
                        .and_then(|name| name.strip_suffix(".json"))
                })
                .or_else(|| {
                    path.split('/')
                        .find(|segment| segment.len() == 26 && segment.starts_with("01"))
                })
                .map(str::to_owned)
        })
        .collect()
}

fn verify_plan_actor(context: &K1cVerificationContext, authority: &MergeAuthority) -> Result<()> {
    match (&context.actor, authority) {
        (K1cActor::Owner { .. }, MergeAuthority::Owner) => Ok(()),
        (
            K1cActor::Grantee {
                authority_chain, ..
            },
            MergeAuthority::Grantee { chain_count: 1, .. },
        ) if !authority_chain.is_empty() => Ok(()),
        _ => Err(Error::InvalidOperation(
            "merge plan authority differs from the K1-C publication actor".into(),
        )),
    }
}

fn verify_plan_parents(context: &K1cVerificationContext, parents: &[String; 2]) -> Result<()> {
    if parents[0] >= parents[1]
        || context.predecessors
            != parents
                .iter()
                .cloned()
                .map(Value::String)
                .collect::<Vec<_>>()
    {
        return Err(Error::InvalidOperation(
            "merge plan parents differ from the sorted publication predecessors".into(),
        ));
    }
    Ok(())
}

/// Qualify a real two-parent K1-C merge, assemble its signed draft.2
/// candidate, and export the same keyless package used by normal publication.
#[allow(clippy::too_many_arguments)]
pub fn merge_draft2_package(
    plan: &Draft2MergePlan,
    context: K1cVerificationContext,
    evidence: Value,
    signer: ManifestSigner<'_>,
    extra_public_objects: BTreeMap<String, Vec<u8>>,
) -> Result<KeylessPublicationPackage> {
    verify_plan_parents(&context, &plan.parents)?;
    verify_plan_actor(&context, &plan.authority)?;
    let touched = plan
        .left_changed_sids
        .union(&plan.right_changed_sids)
        .cloned()
        .collect::<BTreeSet<_>>();
    verify_disjoint_merge(
        &plan.left_changed_sids,
        &plan.right_changed_sids,
        &plan.deleted_sids,
        &plan.authority,
    )?;
    if context_changed_sids(&context) != touched {
        return Err(Error::InvalidOperation(
            "derived K1-C changeset SIDs differ from the merge plan".into(),
        ));
    }
    let counts = recompose_counts(&plan.left_occurrences, &plan.right_occurrences)?;
    let expected_counts = serde_json::to_value(&counts)
        .map_err(|error| Error::InvalidOperation(format!("merge counts: {error}")))?;
    if context
        .publication_facts
        .pointer("/facts/mode")
        .and_then(Value::as_str)
        != Some("merge")
        || context
            .publication_facts
            .pointer("/facts/semantic_counts")
            .is_some_and(|actual| actual != &expected_counts)
    {
        return Err(Error::InvalidOperation(
            "merge publication facts mode or semantic counts differ".into(),
        ));
    }
    let candidate = assemble_draft2_candidate(&context, evidence, signer)?;
    export_keyless(candidate, context, extra_public_objects)
}

/// Qualify a two-parent resolution and export its signed draft.2 package.
#[allow(clippy::too_many_arguments)]
pub fn resolve_draft2_package(
    plan: &Draft2ResolutionPlan,
    context: K1cVerificationContext,
    evidence: Value,
    signer: ManifestSigner<'_>,
    extra_public_objects: BTreeMap<String, Vec<u8>>,
) -> Result<KeylessPublicationPackage> {
    verify_plan_parents(&context, &plan.parents)?;
    verify_plan_actor(&context, &plan.authority)?;
    verify_fork_resolution(&plan.touched_sids, &plan.authority)?;
    if context_changed_sids(&context) != plan.touched_sids
        || context
            .publication_facts
            .pointer("/facts/mode")
            .and_then(Value::as_str)
            != Some("resolution")
        || context
            .publication_facts
            .pointer("/facts/winner")
            .and_then(Value::as_str)
            != Some(&plan.winner)
        || !plan.parents.contains(&plan.winner)
    {
        return Err(Error::InvalidOperation(
            "resolution facts, winner or derived SIDs differ from the plan".into(),
        ));
    }
    recompose_counts(&plan.left_occurrences, &plan.right_occurrences)?;
    let candidate = assemble_draft2_candidate(&context, evidence, signer)?;
    export_keyless(candidate, context, extra_public_objects)
}

/// Rebuild semantic counts from both branches, deduplicated by occurrence.
pub fn recompose_semantic_counts(
    left: &[SemanticOccurrence],
    right: &[SemanticOccurrence],
) -> Result<SemanticCounts> {
    recompose_counts(left, right)
}

/// Cold-verify a merged/resolved keyless package from one fresh Store.
pub fn cold_merge_from_keyless_store<S: Store>(
    store: &S,
    package: &KeylessPublicationPackage,
) -> Result<VerifiedK1cCarriers> {
    cold_verify(store, package)
}

fn cold_object_digest(objects: &BTreeMap<String, Vec<u8>>) -> Result<String> {
    let object = objects
        .iter()
        .map(|(path, bytes)| {
            let value = match std::str::from_utf8(bytes) {
                Ok(text) => Value::String(text.to_owned()),
                Err(_) => serde_json::json!({ "hex": hex::encode(bytes) }),
            };
            (path.clone(), value)
        })
        .collect::<serde_json::Map<_, _>>();
    let bytes = aithos_core::jcs::canonical_bytes(&Value::Object(object))?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

/// Prove that each supplied order inserts exactly the same complete object
/// set and therefore produces one cold digest.
pub fn verify_insertion_order_independence(
    objects: &BTreeMap<String, Vec<u8>>,
    orders: &[Vec<String>],
) -> Result<String> {
    let expected_keys = objects.keys().cloned().collect::<BTreeSet<_>>();
    let expected_digest = cold_object_digest(objects)?;
    for order in orders {
        if order.iter().cloned().collect::<BTreeSet<_>>() != expected_keys
            || order.len() != expected_keys.len()
        {
            return Err(Error::InvalidOperation(
                "insertion order is not an exact object-set permutation".into(),
            ));
        }
        let rebuilt = order
            .iter()
            .map(|path| {
                objects
                    .get(path)
                    .map(|bytes| (path.clone(), bytes.clone()))
                    .ok_or_else(|| {
                        Error::InvalidOperation(format!(
                            "insertion order names an unknown object: {path}"
                        ))
                    })
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        if cold_object_digest(&rebuilt)? != expected_digest {
            return Err(Error::InvalidOperation(
                "cold digest depends on insertion order".into(),
            ));
        }
    }
    Ok(expected_digest)
}

// -------------------------------------------------------------- the bundle

impl<S: Store> Bundle<S> {
    fn manifest_at(&self, height: u64) -> Result<Manifest> {
        self.get_json(&format!("manifests/{height}.json"))
    }

    fn tree_sidecar(&self, height: u64) -> Result<StateTree> {
        self.get_json(&format!("manifests/tree-{height}.json"))
    }

    /// The competing-siblings shape every merge/resolution requires
    /// (§02.6): same subject, same height, both extending the same
    /// grandparent, tips actually different. Returns (mine, theirs, height).
    fn competing_tips<S2: Store>(&self, other: &Bundle<S2>) -> Result<(Manifest, Manifest, u64)> {
        let err = |m: String| Error::MergeRejected(m);
        if self.did != other.did {
            return Err(err("the two copies are not the same subject".into()));
        }
        let mine: Manifest = self.get_json("manifest.json")?;
        let theirs: Manifest = other.get_json("manifest.json")?;
        if mine.edition.height != theirs.edition.height {
            return Err(err(format!(
                "competing editions must share a height ({} vs {})",
                mine.edition.height, theirs.edition.height
            )));
        }
        if mine.edition.height < 2 {
            return Err(err("nothing published to merge".into()));
        }
        if mine.chain_hash()? == theirs.chain_hash()? {
            return Err(err("the copies are identical — nothing to merge".into()));
        }
        if mine.edition.prev_hash != theirs.edition.prev_hash {
            return Err(err(
                "competing editions must share their grandparent (one edition each side)".into(),
            ));
        }
        // The shared history must really be shared.
        let anc_h = mine.edition.height - 1;
        let anc_mine: Manifest = self.manifest_at(anc_h)?;
        let anc_theirs: Manifest = other.get_json(&format!("manifests/{anc_h}.json"))?;
        if anc_mine.chain_hash()? != anc_theirs.chain_hash()?
            || anc_mine.chain_hash()? != mine.edition.prev_hash
        {
            return Err(err("the common ancestor differs between the copies".into()));
        }
        let height = mine.edition.height;
        Ok((mine, theirs, height))
    }

    /// Both changeset frontiers vs the common ancestor: (mine, theirs).
    fn changeset_frontiers<S2: Store>(
        &self,
        other: &Bundle<S2>,
        height: u64,
    ) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
        let anc = self.tree_sidecar(height - 1)?;
        let mine = self.tree_sidecar(height)?;
        let theirs: StateTree = other.get_json(&format!("manifests/tree-{height}.json"))?;
        Ok((
            frontier(&tree_diff(&anc, &mine)),
            frontier(&tree_diff(&anc, &theirs)),
        ))
    }

    /// Is the pair mergeable? `Ok(())` = disjoint changesets; a same-node
    /// conflict is the fork verdict every verifier must surface (§02.6).
    pub fn fork_check<S2: Store>(&self, other: &Bundle<S2>) -> Result<()> {
        let (_, _, height) = self.competing_tips(other)?;
        let (mine, theirs) = self.changeset_frontiers(other, height)?;
        if let Some(label) = mine.intersection(&theirs).next() {
            return Err(Error::EditionFork(format!("same-node conflict on {label}")));
        }
        Ok(())
    }

    /// Publish the deterministic disjoint-merge edition (§02.6 + §07.6):
    /// both changesets applied over the common ancestor, parents ordered by
    /// ascending edition hash, the log re-joined at a signed merge entry.
    /// Either party runs this and produces byte-identical results.
    pub fn edition_merge<S2: Store>(
        &mut self,
        other: &Bundle<S2>,
        owner: &OwnerKeys,
        now: &str,
    ) -> Result<()> {
        self.edition_merge_as(other, &ForkResolver::Owner(owner), now)
    }

    /// Publish a deterministic merge through one owner or one fully covering
    /// grantee chain. The complete mutation is one Store transaction.
    pub fn edition_merge_as<S2: Store>(
        &mut self,
        other: &Bundle<S2>,
        publisher: &ForkResolver<'_>,
        now: &str,
    ) -> Result<()> {
        self.transaction(|bundle| bundle.edition_merge_inner(other, publisher, now))
    }

    fn edition_merge_inner<S2: Store>(
        &mut self,
        other: &Bundle<S2>,
        publisher: &ForkResolver<'_>,
        now: &str,
    ) -> Result<()> {
        let (mine, theirs, height) = self.competing_tips(other)?;
        let (f_mine, f_theirs) = self.changeset_frontiers(other, height)?;
        if let Some(label) = f_mine.intersection(&f_theirs).next() {
            return Err(Error::EditionFork(format!("same-node conflict on {label}")));
        }
        if let ForkResolver::Delegate { chain, sk } = publisher {
            let doc: DidDocument = self.get_json("did.json")?;
            verify_chain(chain, &doc, now)?;
            let leaf = chain
                .last()
                .ok_or_else(|| Error::MergeRejected("empty publisher chain".into()))?;
            if leaf.grantee.pubkey
                != aithos_core::wire::ed25519_pub_to_multibase(&sk.verifying_key().to_bytes())
            {
                return Err(Error::MergeRejected(
                    "merge publisher key differs from its chain leaf".into(),
                ));
            }
            let touched = f_mine.union(&f_theirs).cloned().collect::<BTreeSet<_>>();
            if !write_covers_labels(&leaf.parsed_perimeter()?, &touched) {
                return Err(Error::MergeRejected(
                    "merge publisher chain does not cover every changed node".into(),
                ));
            }
        }
        let (my_hash, their_hash) = (mine.chain_hash()?, theirs.chain_hash()?);
        let i_am_low = my_hash < their_hash;
        let (hash_lo, hash_hi) = if i_am_low {
            (my_hash.clone(), their_hash.clone())
        } else {
            (their_hash.clone(), my_hash.clone())
        };
        let (head_lo, head_hi) = if i_am_low {
            (mine.gamma_head.clone(), theirs.gamma_head.clone())
        } else {
            (theirs.gamma_head.clone(), mine.gamma_head.clone())
        };
        let anc_h = height - 1;

        // 1. Union the other branch's files — indexes, gamma and manifests
        //    are handled by their own rules below. A non-index file changed
        //    differently on both sides is a same-node conflict the frontier
        //    should have caught: refuse rather than pick a side.
        let index_paths = [
            "e/public/index.json".to_owned(),
            "e/circle/index.json".to_owned(),
            "e/self/index.json".to_owned(),
        ];
        for path in other.store.list("").map_err(io_err)? {
            if path == "manifest.json"
                || path.starts_with("manifests/")
                || path.starts_with("gamma/")
                || index_paths.contains(&path)
            {
                continue;
            }
            let their_bytes = other
                .store
                .get(&path)
                .map_err(io_err)?
                .ok_or_else(|| Error::SealRejected(format!("listed file vanished: {path}")))?;
            match self.store.get(&path).map_err(io_err)? {
                None => self.write_object(&path, &their_bytes)?,
                Some(my_bytes) if my_bytes == their_bytes => {}
                Some(_) => {
                    return Err(Error::MergeRejected(format!(
                        "non-index file changed on both branches: {path}"
                    )));
                }
            }
        }

        // 2. 3-way index merges against the ancestor's pinned snapshots.
        let base_pub: ZoneIndex = self.get_json(&format!("manifests/index-public-{anc_h}.json"))?;
        let base_cir: ZoneIndex = self.get_json(&format!("manifests/index-circle-{anc_h}.json"))?;
        let base_self: SelfIndex = self.get_json(&format!("manifests/index-self-{anc_h}.json"))?;
        let merged_pub = merge_zone_index(
            &base_pub,
            &self.get_json("e/public/index.json")?,
            &other.get_json("e/public/index.json")?,
        )?;
        let merged_cir = merge_zone_index(
            &base_cir,
            &self.get_json("e/circle/index.json")?,
            &other.get_json("e/circle/index.json")?,
        )?;
        let merged_self = merge_self_index(
            &base_self,
            &self.get_json("e/self/index.json")?,
            &other.get_json("e/self/index.json")?,
        )?;
        self.put_json("e/public/index.json", &merged_pub)?;
        self.put_json("e/circle/index.json", &merged_cir)?;
        self.put_json("e/self/index.json", &merged_self)?;

        // 3. Deterministic parent placement: the LOW parent keeps the
        //    canonical `manifests/<h>` slots (the linear walk follows it),
        //    the HIGH parent moves to the `-alt` slots. Exact bytes, never
        //    re-serialized — the pins stay valid.
        let parent_slots: Vec<String> = {
            let mut v = vec![
                format!("manifests/{height}.json"),
                format!("manifests/tree-{height}.json"),
            ];
            for zone in ["public", "circle", "self"] {
                v.push(format!("manifests/index-{zone}-{height}.json"));
            }
            v
        };
        for slot in &parent_slots {
            let alt = alt_slot(slot);
            if i_am_low {
                let hi_bytes =
                    other.store.get(slot).map_err(io_err)?.ok_or_else(|| {
                        Error::MergeRejected(format!("missing parent file: {slot}"))
                    })?;
                self.write_object(&alt, &hi_bytes)?;
            } else {
                let my_bytes = self.get(slot)?;
                self.write_object(&alt, &my_bytes)?;
                let lo_bytes =
                    other.store.get(slot).map_err(io_err)?.ok_or_else(|| {
                        Error::MergeRejected(format!("missing parent file: {slot}"))
                    })?;
                self.write_object(slot, &lo_bytes)?;
            }
        }

        // 4. Merged gamma segments (§07.6): per segment, shared prefix,
        //    sub-chain LOW, sub-chain HIGH; then the signed merge entry in
        //    its own month — the only entry with two predecessors.
        let mut segs: BTreeSet<String> = BTreeSet::new();
        for p in self.store.list("gamma/").map_err(io_err)? {
            if p.ends_with(".jsonl") {
                segs.insert(p);
            }
        }
        for p in other.store.list("gamma/").map_err(io_err)? {
            if p.ends_with(".jsonl") {
                segs.insert(p);
            }
        }
        let lines_of = |bytes: Option<Vec<u8>>| -> Vec<Vec<u8>> {
            bytes
                .unwrap_or_default()
                .split(|b| *b == b'\n')
                .filter(|l| !l.is_empty())
                .map(<[u8]>::to_vec)
                .collect()
        };
        for seg in &segs {
            let my_lines = lines_of(self.store.get(seg).map_err(io_err)?);
            let their_lines = lines_of(other.store.get(seg).map_err(io_err)?);
            let (lo_lines, hi_lines) = if i_am_low {
                (my_lines, their_lines)
            } else {
                (their_lines, my_lines)
            };
            let merged = merge_segment_lines(&lo_lines, &hi_lines);
            let mut bytes = Vec::new();
            for line in &merged {
                bytes.extend_from_slice(line);
                bytes.push(b'\n');
            }
            self.write_object(seg, &bytes)?;
        }
        if head_lo != head_hi {
            // The log really forked: re-join it. Deterministic id — both
            // mergers must emit byte-identical entries (owner Ed25519 is
            // deterministic, the id derives from the ordered parents).
            let mut hasher = blake3::Hasher::new();
            hasher.update(MERGE_ID_DOMAIN);
            hasher.update(&[0]);
            hasher.update(hash_lo.as_bytes());
            hasher.update(&[0]);
            hasher.update(hash_hi.as_bytes());
            let digest = hasher.finalize();
            let id_bytes: [u8; 16] = digest.as_bytes()[..16].try_into().expect("16 bytes");
            let entry_spec = EntrySpec {
                id: format!(
                    "gamma_{}",
                    Sid(ulid::Ulid::from(u128::from_be_bytes(id_bytes)))
                ),
                prev: head_lo.clone(),
                prevs: Some(vec![head_lo.clone(), head_hi.clone()]),
                at: now.to_owned(),
                kind: Kind::Merge,
                target: None,
                payload: Some(serde_json::json!({
                    "merges": [hash_lo.clone(), hash_hi.clone()]
                })),
                body_enc: None,
            };
            let entry = match publisher {
                ForkResolver::Owner(owner) => owner_entry(entry_spec, &owner.content_sign)?,
                ForkResolver::Delegate { chain, sk } => delegated_entry(
                    entry_spec,
                    chain.iter().map(|mandate| mandate.id.clone()).collect(),
                    sk,
                )?,
            };
            let seg = crate::log::segment_of(now)?;
            let mut bytes = self.store.get(&seg).map_err(io_err)?.unwrap_or_default();
            bytes.extend_from_slice(aithos_core::jcs::canonicalize(&entry)?.as_bytes());
            bytes.push(b'\n');
            self.write_object(&seg, &bytes)?;
        }
        // Fail-closed self-check: the merged log must verify through the
        // join before anything is signed.
        gamma::verify_links(&self.gamma_entries()?)?;

        // 5. The merge manifest: prev_hash pins the LOW parent, `merges`
        //    lists both ascending (§02.6).
        let a = self.publish_artifacts(height + 1)?;
        let (signer, authorized_via) = match publisher {
            ForkResolver::Owner(owner) => (ManifestSigner::Root(&owner.root_sign), Vec::new()),
            ForkResolver::Delegate { chain, sk } => {
                let leaf = chain.last().expect("publisher chain checked above");
                (
                    ManifestSigner::Delegate {
                        key_multibase: leaf.grantee.pubkey.clone(),
                        sk,
                    },
                    chain.iter().map(|mandate| mandate.id.clone()).collect(),
                )
            }
        };
        let manifest = Manifest::build_spec(
            ManifestSpec {
                height: height + 1,
                prev_hash: hash_lo.clone(),
                created_at: now.to_owned(),
                files: a.files,
                roots: a.roots,
                gamma_roots: a.gamma_roots,
                gamma_counts_root: a.gamma_counts_root,
                gamma_head: a.gamma_head,
                merges: vec![hash_lo, hash_hi],
                resolves_fork: String::new(),
                authorized_via,
            },
            signer,
        )?;
        self.put_json(&format!("manifests/{}.json", height + 1), &manifest)?;
        self.put_json("manifest.json", &manifest)
    }

    /// Resolve a fork (§02.6): the nearest common manager — an authority
    /// whose perimeter covers every node touched by BOTH branches; the
    /// owner root always qualifies — publishes the resolving edition. Its
    /// content extends the WINNING branch (this bundle); the losing
    /// branch's manifest and tree are kept in the `-alt` slots — surfaced,
    /// never silently replayed. Returns the losing frontier labels.
    pub fn resolve_fork<S2: Store>(
        &mut self,
        loser: &Bundle<S2>,
        resolver: &ForkResolver<'_>,
        now: &str,
    ) -> Result<Vec<String>> {
        self.transaction(|bundle| bundle.resolve_fork_inner(loser, resolver, now))
    }

    fn resolve_fork_inner<S2: Store>(
        &mut self,
        loser: &Bundle<S2>,
        resolver: &ForkResolver<'_>,
        now: &str,
    ) -> Result<Vec<String>> {
        let (mine, _theirs, height) = self.competing_tips(loser)?;
        let (f_win, f_lose) = self.changeset_frontiers(loser, height)?;
        if let ForkResolver::Delegate { chain, .. } = resolver {
            let doc: DidDocument = self.get_json("did.json")?;
            verify_chain(chain, &doc, now)?;
            let leaf = chain
                .last()
                .ok_or_else(|| Error::ForkResolutionRejected("empty resolver chain".into()))?;
            let touched: BTreeSet<String> = f_win.union(&f_lose).cloned().collect();
            if !write_covers_labels(&leaf.parsed_perimeter()?, &touched) {
                return Err(Error::ForkResolutionRejected(format!(
                    "the resolver's perimeter does not cover every touched node of both branches \
                     ({} labels)",
                    touched.len()
                )));
            }
        }
        // Surface the losing branch: its manifest and tree, exact bytes.
        let loser_manifest = loser
            .store
            .get(&format!("manifests/{height}.json"))
            .map_err(io_err)?
            .ok_or_else(|| Error::MergeRejected("the losing branch lacks its manifest".into()))?;
        self.write_object(&format!("manifests/{height}-alt.json"), &loser_manifest)?;
        let loser_tree = loser
            .store
            .get(&format!("manifests/tree-{height}.json"))
            .map_err(io_err)?
            .ok_or_else(|| Error::MergeRejected("the losing branch lacks its tree".into()))?;
        self.write_object(&format!("manifests/tree-{height}-alt.json"), &loser_tree)?;

        // The resolving edition extends the winner — content unchanged.
        let win_hash = mine.chain_hash()?;
        let a = self.publish_artifacts(height + 1)?;
        let (signer, via) = match resolver {
            ForkResolver::Owner(owner) => (ManifestSigner::Root(&owner.root_sign), Vec::new()),
            ForkResolver::Delegate { chain, sk } => {
                let leaf = chain.last().expect("checked above");
                (
                    ManifestSigner::Delegate {
                        key_multibase: leaf.grantee.pubkey.clone(),
                        sk,
                    },
                    chain.iter().map(|m| m.id.clone()).collect(),
                )
            }
        };
        let manifest = Manifest::build_spec(
            ManifestSpec {
                height: height + 1,
                prev_hash: win_hash.clone(),
                created_at: now.to_owned(),
                files: a.files,
                roots: a.roots,
                gamma_roots: a.gamma_roots,
                gamma_counts_root: a.gamma_counts_root,
                gamma_head: a.gamma_head,
                merges: Vec::new(),
                resolves_fork: win_hash,
                authorized_via: via,
            },
            signer,
        )?;
        self.put_json(&format!("manifests/{}.json", height + 1), &manifest)?;
        self.put_json("manifest.json", &manifest)?;
        Ok(f_lose.into_iter().collect())
    }

    /// Read a tree sidecar and authenticate its bytes against the sha a
    /// SIGNED manifest pins for it — sidecars are caches; the signature
    /// chain is what makes them verifier inputs.
    fn sidecar_checked(
        &self,
        actual_path: &str,
        pinned_by: &Manifest,
        pinned_path: &str,
    ) -> Result<StateTree> {
        let bytes = self.get(actual_path)?;
        let pinned = pinned_by.files.get(pinned_path).ok_or_else(|| {
            Error::MergeRejected(format!("no pin for {pinned_path} in the parent manifest"))
        })?;
        if &sha256_hex(&bytes) != pinned {
            return Err(Error::MergeRejected(format!(
                "sidecar bytes do not match their pinned sha: {actual_path}"
            )));
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::SealRejected(format!("{actual_path}: {e}")))
    }

    /// Verifier side of a merge edition (§02.6, graved): two same-height
    /// parents sharing a grandparent, disjoint changesets, ascending
    /// `merges` with `prev_hash` on the lowest, and the two-predecessor
    /// merge entry pinning both parents' log tips.
    pub(crate) fn verify_merge_edition(
        &self,
        m: &Manifest,
        low: &Manifest,
        height: u64,
    ) -> Result<()> {
        let err = |msg: String| Error::MergeRejected(format!("height {height}: {msg}"));
        if m.merges.len() != 2 || m.merges[0] >= m.merges[1] {
            return Err(err("merges must list two parents ascending".into()));
        }
        if m.edition.prev_hash != m.merges[0] {
            return Err(err("prev_hash must pin the lowest parent".into()));
        }
        if low.chain_hash()? != m.merges[0] {
            return Err(err("the walked parent is not the pinned low parent".into()));
        }
        if height < 3 {
            return Err(err("a merge needs a published common ancestor".into()));
        }
        let alt: Manifest = self.manifest_at_alt(height - 1)?;
        if !alt.authorized_via.is_empty() {
            return Err(err("merge parents must be owner-signed this pass".into()));
        }
        let doc: DidDocument = self.get_json("did.json")?;
        alt.verify_signature(&doc)?;
        if alt.chain_hash()? != m.merges[1] {
            return Err(err("the alt parent is not the pinned high parent".into()));
        }
        if alt.edition.height != height - 1 {
            return Err(err("parents must share the merge's parent height".into()));
        }
        if alt.edition.prev_hash != low.edition.prev_hash {
            return Err(err("parents must share their grandparent".into()));
        }
        // Disjointness, from manifest-pinned sidecars only.
        let gp: Manifest = self.manifest_at(height - 2)?;
        let anc_tree = self.sidecar_checked(
            &format!("manifests/tree-{}.json", height - 2),
            &gp,
            &format!("manifests/tree-{}.json", height - 2),
        )?;
        let lo_tree = self.sidecar_checked(
            &format!("manifests/tree-{}.json", height - 1),
            low,
            &format!("manifests/tree-{}.json", height - 1),
        )?;
        let hi_tree = self.sidecar_checked(
            &format!("manifests/tree-{}-alt.json", height - 1),
            &alt,
            &format!("manifests/tree-{}.json", height - 1),
        )?;
        let f_lo = frontier(&tree_diff(&anc_tree, &lo_tree));
        let f_hi = frontier(&tree_diff(&anc_tree, &hi_tree));
        if let Some(label) = f_lo.intersection(&f_hi).next() {
            return Err(Error::EditionFork(format!(
                "height {height}: same-node conflict on {label}"
            )));
        }
        if !m.authorized_via.is_empty() {
            let chain: Vec<Mandate> = m
                .authorized_via
                .iter()
                .map(|id| self.get_json(&format!("certs/{id}.json")))
                .collect::<Result<_>>()?;
            verify_chain(&chain, &doc, &m.edition.created_at)?;
            let leaf = chain
                .last()
                .ok_or_else(|| err("empty authorized_via".into()))?;
            m.verify_delegate_signature(leaf)?;
            let touched = f_lo.union(&f_hi).cloned().collect::<BTreeSet<_>>();
            if !write_covers_labels(&leaf.parsed_perimeter()?, &touched) {
                return Err(Error::MergeRejected(format!(
                    "height {height}: the merge publisher does not cover every changed node"
                )));
            }
        }
        // The log join: the pinned head is the merge entry citing both
        // parents' tips — or the shared tip when the log never forked.
        if low.gamma_head == alt.gamma_head {
            if m.gamma_head != low.gamma_head {
                return Err(err("un-forked log must keep the shared tip".into()));
            }
            return Ok(());
        }
        let entries = self.gamma_entries()?;
        let join = entries
            .iter()
            .find(|e| e.chain_hash().ok().as_deref() == Some(m.gamma_head.as_str()))
            .ok_or_else(|| err("the pinned gamma head is not in the log".into()))?;
        if join.kind != "merge" {
            return Err(err("the merge edition must pin a merge entry".into()));
        }
        if join.prevs.as_deref() != Some(&[low.gamma_head.clone(), alt.gamma_head.clone()][..]) {
            return Err(err(
                "the merge entry must cite both parents' log tips in order".into(),
            ));
        }
        let mirrors = join
            .payload
            .as_ref()
            .and_then(|p| p.get("merges"))
            .and_then(|v| serde_json::from_value::<Vec<String>>(v.clone()).ok());
        if mirrors.as_deref() != Some(&m.merges[..]) {
            return Err(err(
                "the merge entry payload must mirror the manifest's merges".into(),
            ));
        }
        Ok(())
    }

    fn manifest_at_alt(&self, height: u64) -> Result<Manifest> {
        self.get_json(&format!("manifests/{height}-alt.json"))
    }

    /// Verifier side of a fork resolution (§02.6): the resolving edition
    /// extends the winner it names, the loser is surfaced in the alt slot,
    /// and the signer's authority covers every node touched by BOTH
    /// branches — a delegate only inside its own perimeter, the owner
    /// always.
    pub(crate) fn verify_resolution_edition(
        &self,
        m: &Manifest,
        winner: &Manifest,
        height: u64,
        doc: &DidDocument,
    ) -> Result<()> {
        let err = |msg: String| Error::MergeRejected(format!("height {height}: {msg}"));
        if m.resolves_fork != m.edition.prev_hash {
            return Err(err(
                "a resolving edition must extend the winner it names".into()
            ));
        }
        if winner.chain_hash()? != m.resolves_fork {
            return Err(err("the walked parent is not the named winner".into()));
        }
        if height < 3 {
            return Err(err("a resolution needs a published common ancestor".into()));
        }
        let alt: Manifest = self.manifest_at_alt(height - 1)?;
        if !alt.authorized_via.is_empty() {
            return Err(err("fork parents must be owner-signed this pass".into()));
        }
        alt.verify_signature(doc)?;
        if alt.edition.height != height - 1 {
            return Err(err("the losing branch must share the fork height".into()));
        }
        if alt.edition.prev_hash != winner.edition.prev_hash {
            return Err(err("the fork branches must share their grandparent".into()));
        }
        if alt.chain_hash()? == m.resolves_fork {
            return Err(err("the losing branch cannot be the named winner".into()));
        }
        if m.authorized_via.is_empty() {
            return Ok(()); // owner-signed: authority is the root signature
        }
        // Delegate-signed: chain validity at the edition's own time, the
        // leaf key signs, and the perimeter covers both frontiers.
        let chain: Vec<Mandate> = m
            .authorized_via
            .iter()
            .map(|id| self.get_json(&format!("certs/{id}.json")))
            .collect::<Result<_>>()?;
        verify_chain(&chain, doc, &m.edition.created_at)?;
        let leaf = chain
            .last()
            .ok_or_else(|| err("empty authorized_via".into()))?;
        m.verify_delegate_signature(leaf)?;
        let gp: Manifest = self.manifest_at(height - 2)?;
        let anc_tree = self.sidecar_checked(
            &format!("manifests/tree-{}.json", height - 2),
            &gp,
            &format!("manifests/tree-{}.json", height - 2),
        )?;
        let win_tree = self.sidecar_checked(
            &format!("manifests/tree-{}.json", height - 1),
            winner,
            &format!("manifests/tree-{}.json", height - 1),
        )?;
        let lose_tree = self.sidecar_checked(
            &format!("manifests/tree-{}-alt.json", height - 1),
            &alt,
            &format!("manifests/tree-{}.json", height - 1),
        )?;
        let mut touched = frontier(&tree_diff(&anc_tree, &win_tree));
        touched.extend(frontier(&tree_diff(&anc_tree, &lose_tree)));
        if !write_covers_labels(&leaf.parsed_perimeter()?, &touched) {
            return Err(Error::ForkResolutionRejected(format!(
                "height {height}: the signer's perimeter does not cover every touched node of \
                 both branches"
            )));
        }
        Ok(())
    }
}

/// `manifests/<h>.json` → `manifests/<h>-alt.json` (same for `tree-` and
/// `index-` sidecars): the deterministic home of the HIGH/losing parent.
fn alt_slot(path: &str) -> String {
    path.strip_suffix(".json")
        .map_or_else(|| format!("{path}-alt"), |stem| format!("{stem}-alt.json"))
}

//! Pure D7 delegated-occurrence correlation and counter verification.
//!
//! Evidence views are grouped by their canonical operation occurrence. Native
//! duplicates therefore cannot inflate a counter, while conflicting views fail
//! closed. The verified result is opaque protocol evidence, not authority.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::jcs;
use crate::merkle::{h_leaf, mroot};
use crate::{Error, Result};

const PROFILE_KEY: &str = "aithos-delegated-counts-core";
const PROFILE: &str = "1.0.0-draft.1";
const MANDATE_PROFILE_KEY: &str = "aithos-mandate-core";
const MANDATE_PROFILE: &str = "1.0.0-draft.3";
const VIEW_KEYS: [&str; 9] = [
    "view",
    "occurrence",
    "commitment",
    "actor",
    "authorized_via",
    "kind",
    "facts_domain",
    "opposable",
    "derived",
];
const KINDS: [&str; 8] = [
    "read",
    "mutation",
    "action",
    "inference",
    "grant",
    "revoke",
    "rotate",
    "publication",
];
const FACT_DOMAINS: [&str; 9] = [
    "ethos",
    "structure",
    "vault-config",
    "connector",
    "inference",
    "mandate",
    "rotation",
    "publication",
    "gamma",
];

fn invalid(detail: impl Into<String>) -> Error {
    Error::InvalidDelegatedCounts(detail.into())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid(format!("{label} has a non-exact member set")));
    }
    if object.values().any(Value::is_null) {
        return Err(invalid(format!("{label} contains null")));
    }
    Ok(object)
}

fn is_lower_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_commitment(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| is_lower_hex(hex, 32))
}

fn is_ulid(value: &str) -> bool {
    value.len() == 26
        && value.bytes().all(|byte| {
            byte.is_ascii_digit()
                || matches!(
                    byte,
                    b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z'
                )
        })
}

fn is_mandate_id(value: &str) -> bool {
    value.strip_prefix("mandate_").is_some_and(is_ulid)
}

fn is_occurrence(value: &str) -> bool {
    value.strip_prefix("op_").is_some_and(is_ulid)
}

fn required_text<'a>(object: &'a Map<String, Value>, key: &str, label: &str) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| invalid(format!("{label} has invalid {key}")))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DelegatedCounters {
    mutations: u64,
    consumptions: u64,
}

impl DelegatedCounters {
    #[must_use]
    pub const fn mutations(self) -> u64 {
        self.mutations
    }

    #[must_use]
    pub const fn consumptions(self) -> u64 {
        self.consumptions
    }
}

#[derive(Debug)]
pub struct VerifiedDelegatedCounts {
    counts: BTreeMap<String, DelegatedCounters>,
    occurrences: Vec<String>,
}

impl VerifiedDelegatedCounts {
    #[must_use]
    pub fn counts_for(&self, mandate_id: &str) -> Option<DelegatedCounters> {
        self.counts.get(mandate_id).copied()
    }

    #[must_use]
    pub fn occurrences(&self) -> &[String] {
        &self.occurrences
    }
}

#[derive(Debug)]
struct EvidenceView {
    view: String,
    occurrence: String,
    commitment: String,
    actor: String,
    authorized_via: Vec<String>,
    kind: String,
    facts_domain: String,
    opposable: bool,
    derived: bool,
}

fn parse_view(value: &Value) -> Result<EvidenceView> {
    let object = exact_object(value, &VIEW_KEYS, "delegated evidence view")?;
    let view = required_text(object, "view", "delegated evidence view")?.to_owned();
    let occurrence = required_text(object, "occurrence", "delegated evidence view")?.to_owned();
    if !is_occurrence(&occurrence) {
        return Err(invalid("delegated evidence view has invalid occurrence"));
    }
    let commitment = required_text(object, "commitment", "delegated evidence view")?.to_owned();
    if !is_commitment(&commitment) {
        return Err(invalid("delegated evidence view has invalid commitment"));
    }
    let actor = required_text(object, "actor", "delegated evidence view")?.to_owned();
    if !matches!(actor.as_str(), "owner" | "grantee") {
        return Err(invalid("delegated evidence view has invalid actor"));
    }
    let authorized_via = object["authorized_via"]
        .as_array()
        .ok_or_else(|| invalid("authorized_via is not an array"))?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|id| is_mandate_id(id))
                .map(str::to_owned)
                .ok_or_else(|| invalid("authorized_via contains an invalid mandate id"))
        })
        .collect::<Result<Vec<_>>>()?;
    if authorized_via.iter().collect::<BTreeSet<_>>().len() != authorized_via.len() {
        return Err(invalid("authorized_via contains a duplicate mandate id"));
    }
    if (actor == "owner" && !authorized_via.is_empty())
        || (actor == "grantee" && authorized_via.is_empty())
    {
        return Err(invalid("actor and authorized_via disagree"));
    }
    let kind = required_text(object, "kind", "delegated evidence view")?.to_owned();
    if !KINDS.contains(&kind.as_str()) {
        return Err(invalid(
            "delegated evidence view has unknown operation kind",
        ));
    }
    let facts_domain = required_text(object, "facts_domain", "delegated evidence view")?.to_owned();
    if !FACT_DOMAINS.contains(&facts_domain.as_str()) {
        return Err(invalid("delegated evidence view has unknown facts domain"));
    }
    let opposable = object["opposable"]
        .as_bool()
        .ok_or_else(|| invalid("opposable is not boolean"))?;
    let derived = object["derived"]
        .as_bool()
        .ok_or_else(|| invalid("derived is not boolean"))?;
    if kind != "read" && !opposable {
        return Err(invalid("only a read may be non-opposable"));
    }
    Ok(EvidenceView {
        view,
        occurrence,
        commitment,
        actor,
        authorized_via,
        kind,
        facts_domain,
        opposable,
        derived,
    })
}

fn same_occurrence(left: &EvidenceView, right: &EvidenceView) -> bool {
    left.commitment == right.commitment
        && left.actor == right.actor
        && left.authorized_via == right.authorized_via
        && left.kind == right.kind
        && left.facts_domain == right.facts_domain
        && left.opposable == right.opposable
}

fn tally(evidence_views: &Value) -> Result<(BTreeMap<String, DelegatedCounters>, Vec<String>)> {
    let views = evidence_views
        .as_array()
        .ok_or_else(|| invalid("delegated evidence views are not an array"))?;
    let mut grouped: BTreeMap<String, Vec<EvidenceView>> = BTreeMap::new();
    for raw in views {
        let parsed = parse_view(raw)?;
        grouped
            .entry(parsed.occurrence.clone())
            .or_default()
            .push(parsed);
    }

    let mut counts: BTreeMap<String, DelegatedCounters> = BTreeMap::new();
    let mut occurrences = Vec::new();
    for (occurrence, group) in grouped {
        let first = group
            .first()
            .ok_or_else(|| invalid("empty occurrence group"))?;
        if group
            .iter()
            .skip(1)
            .any(|item| !same_occurrence(first, item))
        {
            return Err(invalid("one occurrence has conflicting evidence"));
        }
        if group.iter().all(|item| item.derived) {
            return Err(invalid("derived evidence has no parent occurrence"));
        }
        let views = group.iter().map(|item| &item.view).collect::<BTreeSet<_>>();
        if views.len() != group.len() {
            return Err(invalid("one occurrence duplicates a native evidence view"));
        }
        if first.actor == "owner" || (first.kind == "read" && !first.opposable) {
            continue;
        }
        occurrences.push(occurrence);
        for mandate_id in &first.authorized_via {
            let counters = counts.entry(mandate_id.clone()).or_default();
            counters.consumptions = counters
                .consumptions
                .checked_add(1)
                .ok_or_else(|| invalid("delegated consumption count overflows u64"))?;
            if first.kind == "mutation" && first.facts_domain == "ethos" {
                counters.mutations = counters
                    .mutations
                    .checked_add(1)
                    .ok_or_else(|| invalid("delegated mutation count overflows u64"))?;
            }
        }
    }
    Ok((counts, occurrences))
}

fn canonical_leaves(
    counts: &BTreeMap<String, DelegatedCounters>,
) -> Result<(Value, Vec<[u8; 32]>)> {
    let mut leaves = Vec::new();
    let mut hashes = Vec::new();
    for (mandate_id, counters) in counts {
        let counters_value = if counters.mutations == 0 {
            json!({"consumptions": counters.consumptions})
        } else {
            json!({
                "consumptions": counters.consumptions,
                "mutations": counters.mutations,
            })
        };
        let mut payload = mandate_id.as_bytes().to_vec();
        payload.push(0);
        payload.extend_from_slice(&jcs::canonical_bytes(&counters_value)?);
        let hash = h_leaf(&payload);
        leaves.push(json!({
            "mandate_id": mandate_id,
            "counters": counters_value,
            "payload_hex": hex::encode(&payload),
            "leaf_hex": hex::encode(hash),
        }));
        hashes.push(hash);
    }
    Ok((Value::Array(leaves), hashes))
}

/// Validate the closed delegated-count reference, canonical leaves, occurrence
/// correlation and separate Merkle root.
pub fn verify_delegated_counts(
    reference: &Value,
    leaves: &Value,
    evidence_views: &Value,
) -> Result<VerifiedDelegatedCounts> {
    let reference = exact_object(reference, &[PROFILE_KEY, "root"], "delegated_counts")?;
    if reference[PROFILE_KEY].as_str() != Some(PROFILE) {
        return Err(invalid("unknown delegated-counts profile"));
    }
    let root = reference["root"]
        .as_str()
        .filter(|value| is_lower_hex(value, 32))
        .ok_or_else(|| invalid("invalid delegated-counts root"))?;
    let (counts, occurrences) = tally(evidence_views)?;
    let (expected_leaves, hashes) = canonical_leaves(&counts)?;
    if *leaves != expected_leaves {
        return Err(invalid(
            "delegated-count leaves do not match canonical occurrences",
        ));
    }
    if root != hex::encode(mroot(&hashes)) {
        return Err(invalid("delegated-count root mismatch"));
    }
    Ok(VerifiedDelegatedCounts {
        counts,
        occurrences,
    })
}

#[derive(Debug)]
pub struct VerifiedDelegatedCountMandates {
    depth: usize,
}

impl VerifiedDelegatedCountMandates {
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

fn mandate_u64(constraints: &Map<String, Value>, name: &str) -> Result<u64> {
    constraints
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::InvalidMandate(format!("{name} is not an unsigned integer")))
}

/// Validate the frozen draft.3 D7 constraint projection and attenuation chain.
pub fn verify_delegated_count_mandates(chain: &Value) -> Result<VerifiedDelegatedCountMandates> {
    let chain = chain
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| Error::InvalidMandate("delegated-count mandate chain is empty".into()))?;
    let mut previous: Option<(&str, u64, u64)> = None;
    for item in chain {
        let object = item.as_object().ok_or_else(|| {
            Error::InvalidMandate("delegated-count mandate is not an object".into())
        })?;
        let keys = [MANDATE_PROFILE_KEY, "constraints", "id", "parent"];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(Error::InvalidMandate(
                "delegated-count mandate has a non-exact member set".into(),
            ));
        }
        if object[MANDATE_PROFILE_KEY].as_str() != Some(MANDATE_PROFILE) {
            return Err(Error::InvalidMandate(
                "delegated counts require a homogeneous draft.3 chain".into(),
            ));
        }
        let id = object["id"]
            .as_str()
            .filter(|id| is_mandate_id(id))
            .ok_or_else(|| Error::InvalidMandate("invalid delegated-count mandate id".into()))?;
        let constraints = object["constraints"].as_object().ok_or_else(|| {
            Error::InvalidMandate("delegated-count constraints are not an object".into())
        })?;
        let mutations = mandate_u64(constraints, "max_mutations")?;
        let consumptions = mandate_u64(constraints, "max_consumptions")?;
        match previous {
            None => {
                if !object["parent"].is_null() {
                    return Err(Error::InvalidMandate(
                        "delegated-count root mandate has a parent".into(),
                    ));
                }
            }
            Some((parent_id, parent_mutations, parent_consumptions)) => {
                if object["parent"].as_str() != Some(parent_id) {
                    return Err(Error::InvalidMandate(
                        "delegated-count mandate parent mismatch".into(),
                    ));
                }
                if mutations > parent_mutations || consumptions > parent_consumptions {
                    return Err(Error::InvalidMandate(
                        "delegated-count constraint widens".into(),
                    ));
                }
            }
        }
        previous = Some((id, mutations, consumptions));
    }
    Ok(VerifiedDelegatedCountMandates { depth: chain.len() })
}

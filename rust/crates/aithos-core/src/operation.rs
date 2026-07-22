//! Pure validation boundary for canonical W1 operations and SC1 sessions.
//!
//! The verified values in this module are intentionally opaque. They prove only
//! their named protocol layer and are not an authorization `Allow`.

use std::collections::BTreeSet;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::did::DidDocument;
use crate::ids::Sid;
use crate::jcs;
use crate::mandate::{self, CanonicalTimestamp, Mandate};
use crate::path::Zone;
use crate::revocation::Revocation;
use crate::wire;
use crate::{Error, Result};

const FACTS_PROFILE_KEY: &str = "aithos-operation-facts-core";
const FACTS_PROFILE: &str = "1.0.0-draft.1";
const STATE_PROFILE_KEY: &str = "aithos-state-fact-core";
const STATE_PROFILE: &str = "1.0.0-draft.1";
const OPERATION_PROFILE_KEY: &str = "aithos-operation-core";
const OPERATION_PROFILE: &str = "1.0.0-draft.1";
const SESSION_PROFILE_KEY: &str = "aithos-session-core";
const SESSION_PROFILE: &str = "1.0.0-draft.1";
const SESSION_PROOF_PROFILE_KEY: &str = "aithos-session-proof-core";
const SESSION_PROOF_PROFILE: &str = "1.0.0-draft.1";
const FACTS_DOMAIN: &str = "aithos-core/v1/operation-facts";
const STATE_FACT_DOMAIN: &str = "aithos-core/v1/state-fact";
const STATE_KEY_DOMAIN: &str = "aithos-core/v1/state-key";
const GAMMA_READ_DOMAIN: &str = "aithos-core/v1/gamma-read-request";
const INFERENCE_REQUEST_DOMAIN: &str = "aithos-core/v1/inference-request";
const PURPOSE_DOMAIN: &str = "aithos-core/v1/purpose";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";

#[derive(Clone, Copy)]
enum Rejection {
    Facts,
    State,
    Operation,
    Session,
}

fn rejected(kind: Rejection, detail: impl Into<String>) -> Error {
    match kind {
        Rejection::Facts => Error::InvalidOperationFacts(detail.into()),
        Rejection::State => Error::InvalidStateFact(detail.into()),
        Rejection::Operation => Error::InvalidOperation(detail.into()),
        Rejection::Session => Error::InvalidSession(detail.into()),
    }
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    kind: Rejection,
    label: &str,
) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| rejected(kind, format!("{label} is not an object")))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(rejected(
            kind,
            format!("{label} has a non-exact member set"),
        ));
    }
    if object.values().any(Value::is_null) {
        return Err(rejected(kind, format!("{label} contains null")));
    }
    Ok(object)
}

fn canonical(value: &Value, kind: Rejection) -> Result<String> {
    jcs::canonicalize(value).map_err(|error| rejected(kind, error.to_string()))
}

fn sha256_text(payload: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(payload)))
}

fn commitment(domain: &str, payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(payload);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn is_commitment(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn required_commitment<'a>(value: &'a Value, kind: Rejection, label: &str) -> Result<&'a str> {
    let text = value
        .as_str()
        .ok_or_else(|| rejected(kind, format!("{label} is not text")))?;
    if !is_commitment(text) {
        return Err(rejected(
            kind,
            format!("{label} is not strict lowercase sha256 text"),
        ));
    }
    Ok(text)
}

fn is_ulid(value: &str) -> bool {
    value.len() == 26 && value.bytes().all(|byte| {
        byte.is_ascii_digit()
            || matches!(byte, b'A'..=b'H' | b'J'..=b'K' | b'M'..=b'N' | b'P'..=b'T' | b'V'..=b'Z')
    })
}

fn is_mandate_id(value: &str) -> bool {
    value.strip_prefix("mandate_").is_some_and(is_ulid)
}

fn is_occurrence(value: &str) -> bool {
    value.strip_prefix("op_").is_some_and(is_ulid)
}

fn is_token(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn is_version(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_did(value: &str) -> bool {
    value
        .strip_prefix("did:aithos:")
        .is_some_and(|key| wire::multibase_to_ed25519_pub(key).is_ok())
}

fn is_key(value: &str) -> bool {
    wire::multibase_to_ed25519_pub(value).is_ok()
}

fn parse_timestamp(value: &Value, kind: Rejection, label: &str) -> Result<CanonicalTimestamp> {
    let text = value
        .as_str()
        .ok_or_else(|| rejected(kind, format!("{label} is not text")))?;
    CanonicalTimestamp::parse(text).map_err(|()| {
        rejected(
            kind,
            format!("{label} is not a canonical calendar Z instant"),
        )
    })
}

fn verify_ed25519(key: &str, message: &[u8], signature: &Value, kind: Rejection) -> Result<()> {
    let signature = signature
        .as_str()
        .ok_or_else(|| rejected(kind, "signature is not text"))?;
    if signature.len() != 128
        || !signature
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(rejected(kind, "malformed Ed25519 signature"));
    }
    let public = wire::multibase_to_ed25519_pub(key)
        .map_err(|_| rejected(kind, "malformed Ed25519 public key"))?;
    let public = VerifyingKey::from_bytes(&public)
        .map_err(|_| rejected(kind, "malformed Ed25519 public key"))?;
    let bytes: [u8; 64] = hex::decode(signature)
        .map_err(|_| rejected(kind, "malformed Ed25519 signature"))?
        .try_into()
        .map_err(|_| rejected(kind, "malformed Ed25519 signature"))?;
    public
        .verify(message, &Signature::from_bytes(&bytes))
        .map_err(|_| rejected(kind, "Ed25519 signature does not verify"))
}

#[derive(Debug, Clone, Copy)]
pub struct MutationNode {
    pub sid: Sid,
    pub zone: Zone,
    pub parent: Option<Sid>,
}

impl MutationNode {
    #[must_use]
    pub const fn new(sid: Sid, zone: Zone, parent: Option<Sid>) -> Self {
        Self { sid, zone, parent }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OperationFactsEvidence<'a> {
    Mutation {
        state_facts: &'a Value,
        nodes: &'a [MutationNode],
        vault_record_key: &'a str,
    },
    Read {
        context: &'a Value,
        fixtures: &'a Value,
    },
    ActionInference {
        context: &'a Value,
    },
    Structural {
        context: &'a Value,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct OperationFactsInput<'a> {
    pub document: &'a Value,
    pub facts_ref: Option<&'a Value>,
    pub evidence: OperationFactsEvidence<'a>,
}

#[derive(Debug)]
pub struct VerifiedOperationFacts {
    kind: String,
    digest: String,
}

impl VerifiedOperationFacts {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

pub enum StateFactInput<'a> {
    LogicalState {
        state: &'a Value,
        state_facts: Option<&'a Value>,
    },
    Document {
        document: &'a Value,
        expected_key_commitments: Option<&'a [String]>,
    },
    Reference {
        state: &'a Value,
        document: &'a Value,
    },
}

#[derive(Debug)]
pub struct VerifiedStateFact {
    digest: String,
}

impl VerifiedStateFact {
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OperationProjectionEvidence<'a> {
    pub facts_documents: &'a [&'a Value],
    pub certificates: &'a [&'a Value],
}

#[derive(Debug)]
pub struct VerifiedOperationProjection {
    operation_ref: Value,
}

impl VerifiedOperationProjection {
    #[must_use]
    pub fn operation_ref(&self) -> &Value {
        &self.operation_ref
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationCorrelation {
    Correlated,
    Distinct,
}

#[derive(Debug, Clone, Copy)]
pub struct SessionEvidence<'a> {
    pub mandate: &'a Value,
    pub certificate: &'a Value,
    pub projection: &'a Value,
    pub operation_ref: &'a Value,
    pub native_leaf_proof: Option<&'a Value>,
    pub native_leaf_domain: &'a [u8],
    pub session_proof: Option<&'a Value>,
}

/// A delegated SC1 session whose authority is a verified non-root mandate
/// chain. The certificate/projection/proof wire remains exactly the one
/// consumed by [`verify_session`]; this wrapper supplies the prerequisite
/// chain, DID and fresh revocation state that a non-root leaf needs.
#[derive(Debug, Clone, Copy)]
pub struct DelegatedSessionEvidence<'a> {
    pub chain: &'a [Mandate],
    pub did: &'a DidDocument,
    pub at: &'a str,
    pub revocations: &'a [Revocation],
    pub session: SessionEvidence<'a>,
}

#[derive(Debug)]
pub struct VerifiedSession {
    operation_ref: Value,
}

impl VerifiedSession {
    #[must_use]
    pub fn operation_ref(&self) -> &Value {
        &self.operation_ref
    }
}

fn state_document(document: &Value, expected_keys: Option<&[String]>) -> Result<String> {
    let value = exact_object(
        document,
        &[STATE_PROFILE_KEY, "objects"],
        Rejection::State,
        "state-fact document",
    )?;
    if value[STATE_PROFILE_KEY] != STATE_PROFILE {
        return Err(rejected(Rejection::State, "unknown state-fact profile"));
    }
    let objects = value["objects"]
        .as_array()
        .filter(|objects| !objects.is_empty())
        .ok_or_else(|| rejected(Rejection::State, "objects is not a non-empty array"))?;
    let mut keys = Vec::with_capacity(objects.len());
    for (index, candidate) in objects.iter().enumerate() {
        let object = exact_object(
            candidate,
            &["key_commitment", "byte_commitment"],
            Rejection::State,
            &format!("state object {index}"),
        )?;
        let key = required_commitment(
            &object["key_commitment"],
            Rejection::State,
            &format!("state object {index} key"),
        )?;
        required_commitment(
            &object["byte_commitment"],
            Rejection::State,
            &format!("state object {index} bytes"),
        )?;
        keys.push(key.to_owned());
    }
    if keys.iter().collect::<BTreeSet<_>>().len() != keys.len() {
        return Err(rejected(Rejection::State, "duplicate key commitment"));
    }
    let mut sorted = keys.clone();
    sorted.sort();
    if keys != sorted {
        return Err(rejected(Rejection::State, "unsorted objects"));
    }
    if let Some(expected) = expected_keys {
        if keys.iter().collect::<BTreeSet<_>>() != expected.iter().collect::<BTreeSet<_>>() {
            return Err(rejected(Rejection::State, "affected object set mismatch"));
        }
    }
    Ok(commitment(
        STATE_FACT_DOMAIN,
        canonical(document, Rejection::State)?.as_bytes(),
    ))
}

fn state_catalog(state_facts: &Value) -> Result<Vec<(String, &Value)>> {
    let fixtures = state_facts
        .as_object()
        .ok_or_else(|| rejected(Rejection::State, "state catalog is not an object"))?;
    fixtures
        .values()
        .map(|fixture| {
            let document = fixture.get("document").unwrap_or(fixture);
            state_document(document, None).map(|digest| (digest, document))
        })
        .collect()
}

fn logical_state<'a>(value: &'a Value, catalog: &[(String, &'a Value)]) -> Result<Option<&'a str>> {
    let object = value
        .as_object()
        .ok_or_else(|| rejected(Rejection::State, "logical state is not a tagged object"))?;
    match object.get("state").and_then(Value::as_str) {
        Some("absent") => {
            exact_object(value, &["state"], Rejection::State, "absent state")?;
            Ok(None)
        }
        Some("present") => {
            let present = exact_object(
                value,
                &["state", "state_ref"],
                Rejection::State,
                "present state",
            )?;
            let reference = exact_object(
                &present["state_ref"],
                &[STATE_PROFILE_KEY, "digest"],
                Rejection::State,
                "state_ref",
            )?;
            if reference[STATE_PROFILE_KEY] != STATE_PROFILE {
                return Err(rejected(Rejection::State, "unknown state_ref profile"));
            }
            let digest =
                required_commitment(&reference["digest"], Rejection::State, "state_ref digest")?;
            let document = catalog
                .iter()
                .find_map(|(candidate, document)| (candidate == digest).then_some(*document))
                .ok_or_else(|| rejected(Rejection::State, "state_ref digest mismatch"))?;
            if state_document(document, None)? != digest {
                return Err(rejected(Rejection::State, "state_ref digest mismatch"));
            }
            Ok(Some(digest))
        }
        _ => Err(rejected(Rejection::State, "unknown logical state")),
    }
}

fn operation_envelope(document: &Value) -> Result<&Map<String, Value>> {
    let value = exact_object(
        document,
        &[FACTS_PROFILE_KEY, "kind", "facts"],
        Rejection::Facts,
        "operation-facts document",
    )?;
    if value[FACTS_PROFILE_KEY] != FACTS_PROFILE {
        return Err(rejected(
            Rejection::Facts,
            "unknown operation-facts profile",
        ));
    }
    Ok(value)
}

fn facts_digest(document: &Value, facts_ref: Option<&Value>) -> Result<String> {
    let digest = commitment(
        FACTS_DOMAIN,
        canonical(document, Rejection::Facts)?.as_bytes(),
    );
    if let Some(reference) = facts_ref {
        let reference = exact_object(
            reference,
            &[FACTS_PROFILE_KEY, "digest"],
            Rejection::Facts,
            "facts_ref",
        )?;
        if reference[FACTS_PROFILE_KEY] != FACTS_PROFILE {
            return Err(rejected(Rejection::Facts, "facts_ref profile mismatch"));
        }
        let announced =
            required_commitment(&reference["digest"], Rejection::Facts, "facts_ref digest")?;
        if announced != digest {
            return Err(rejected(Rejection::Facts, "facts_ref digest mismatch"));
        }
    }
    Ok(digest)
}

fn mutation_node(nodes: &[MutationNode], sid: Sid) -> Option<&MutationNode> {
    nodes.iter().find(|node| node.sid == sid)
}

fn strict_sid(value: &Value, kind: Rejection, label: &str) -> Result<Sid> {
    let text = value
        .as_str()
        .ok_or_else(|| rejected(kind, format!("{label} is not text")))?;
    if !is_ulid(text) {
        return Err(rejected(kind, format!("{label} is not a canonical SID")));
    }
    Sid::parse(text).map_err(|_| rejected(kind, format!("{label} is not a canonical SID")))
}

fn validate_path(
    value: &Value,
    zone: Zone,
    target: Sid,
    nodes: &[MutationNode],
    label: &str,
) -> Result<Vec<Sid>> {
    let raw = value
        .as_array()
        .ok_or_else(|| rejected(Rejection::Facts, format!("{label} is not a SID array")))?;
    let path: Vec<Sid> = raw
        .iter()
        .map(|value| strict_sid(value, Rejection::Facts, label))
        .collect::<Result<_>>()?;
    if path.iter().collect::<BTreeSet<_>>().len() != path.len() {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} has duplicate SID"),
        ));
    }
    if path.contains(&target) {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} includes target SID"),
        ));
    }
    if path
        .iter()
        .any(|sid| mutation_node(nodes, *sid).is_none_or(|candidate| candidate.zone != zone))
    {
        return Err(rejected(Rejection::Facts, format!("{label} crosses zones")));
    }
    if let Some(first) = path.first() {
        if mutation_node(nodes, *first)
            .and_then(|node| node.parent)
            .is_some()
        {
            return Err(rejected(
                Rejection::Facts,
                format!("{label} does not start at a root"),
            ));
        }
        for pair in path.windows(2) {
            if mutation_node(nodes, pair[1]).and_then(|node| node.parent) != Some(pair[0]) {
                return Err(rejected(
                    Rejection::Facts,
                    format!("{label} is not root-to-leaf"),
                ));
            }
        }
    }
    Ok(path)
}

fn current_parent_path(target: Sid, nodes: &[MutationNode]) -> Result<Vec<Sid>> {
    let mut current = mutation_node(nodes, target)
        .ok_or_else(|| {
            rejected(
                Rejection::Facts,
                "target SID is absent from native topology",
            )
        })?
        .parent;
    let mut path = Vec::new();
    while let Some(sid) = current {
        if path.contains(&sid) {
            return Err(rejected(
                Rejection::Facts,
                "native topology contains a cycle",
            ));
        }
        path.push(sid);
        current = mutation_node(nodes, sid)
            .ok_or_else(|| rejected(Rejection::Facts, "native topology is incomplete"))?
            .parent;
    }
    path.reverse();
    Ok(path)
}

fn transition(verb: &str, before: &Value, after: &Value) -> Result<()> {
    let before_state = before["state"].as_str().unwrap_or("");
    let after_state = after["state"].as_str().unwrap_or("");
    let expected = match verb {
        "create" => ("absent", "present"),
        "delete" => ("present", "absent"),
        "edit" | "redact" | "rename" | "move" => ("present", "present"),
        _ => return Err(rejected(Rejection::Facts, "unknown mutation verb")),
    };
    if (before_state, after_state) != expected {
        return Err(rejected(
            Rejection::Facts,
            "invalid before/after transition",
        ));
    }
    if expected == ("present", "present")
        && before["state_ref"]["digest"] == after["state_ref"]["digest"]
    {
        return Err(rejected(Rejection::Facts, "equal state digests"));
    }
    Ok(())
}

fn validate_mutation(
    facts: &Value,
    state_facts: &Value,
    nodes: &[MutationNode],
    vault_record_key: &str,
) -> Result<()> {
    let object = facts
        .as_object()
        .ok_or_else(|| rejected(Rejection::Facts, "mutation facts is not an object"))?;
    if object.values().any(Value::is_null) {
        return Err(rejected(Rejection::Facts, "mutation facts contains null"));
    }
    let domain = object.get("domain").and_then(Value::as_str).unwrap_or("");
    let value = match domain {
        "ethos" => {
            let value = exact_object(
                facts,
                &["domain", "verb", "zone", "sid", "dir", "before", "after"],
                Rejection::Facts,
                "ethos mutation facts",
            )?;
            let verb = value["verb"].as_str().unwrap_or("");
            if !matches!(verb, "create" | "edit" | "delete" | "redact") {
                return Err(rejected(Rejection::Facts, "unknown ethos verb"));
            }
            let zone = match value["zone"].as_str() {
                Some("public") => Zone::Public,
                Some("circle") => Zone::Circle,
                Some("self") => Zone::Self_,
                _ => return Err(rejected(Rejection::Facts, "unknown ethos zone")),
            };
            let target = strict_sid(&value["sid"], Rejection::Facts, "target SID")?;
            if mutation_node(nodes, target).is_none_or(|node| node.zone != zone) {
                return Err(rejected(Rejection::Facts, "target SID crosses zones"));
            }
            let path = validate_path(&value["dir"], zone, target, nodes, "dir")?;
            if verb != "create" && path != current_parent_path(target, nodes)? {
                return Err(rejected(
                    Rejection::Facts,
                    "dir is not the current parent path",
                ));
            }
            value
        }
        "structure" => {
            let verb = object.get("verb").and_then(Value::as_str).unwrap_or("");
            let keys: &[&str] = match verb {
                "create" => &[
                    "domain",
                    "verb",
                    "zone",
                    "node_kind",
                    "sid",
                    "destination",
                    "before",
                    "after",
                ],
                "rename" | "delete" => &[
                    "domain",
                    "verb",
                    "zone",
                    "node_kind",
                    "sid",
                    "source",
                    "before",
                    "after",
                ],
                "move" => &[
                    "domain",
                    "verb",
                    "zone",
                    "node_kind",
                    "sid",
                    "source",
                    "destination",
                    "before",
                    "after",
                ],
                _ => return Err(rejected(Rejection::Facts, "unknown structure verb")),
            };
            let value = exact_object(facts, keys, Rejection::Facts, "structure mutation facts")?;
            let zone = match value["zone"].as_str() {
                Some("public") => Zone::Public,
                Some("circle") => Zone::Circle,
                Some("self") => Zone::Self_,
                _ => return Err(rejected(Rejection::Facts, "unknown structure zone")),
            };
            let node_kind = value["node_kind"].as_str().unwrap_or("");
            if !matches!(node_kind, "folder" | "section") {
                return Err(rejected(Rejection::Facts, "unknown node kind"));
            }
            if matches!(verb, "create" | "delete") && node_kind != "folder" {
                return Err(rejected(
                    Rejection::Facts,
                    format!("structure {verb} requires folder"),
                ));
            }
            let target = strict_sid(&value["sid"], Rejection::Facts, "target SID")?;
            if mutation_node(nodes, target).is_none_or(|node| node.zone != zone) {
                return Err(rejected(Rejection::Facts, "target SID crosses zones"));
            }
            if let Some(source) = value.get("source") {
                let path = validate_path(source, zone, target, nodes, "source")?;
                if path != current_parent_path(target, nodes)? {
                    return Err(rejected(
                        Rejection::Facts,
                        "source is not the current parent path",
                    ));
                }
            }
            if let Some(destination) = value.get("destination") {
                validate_path(destination, zone, target, nodes, "destination")?;
            }
            value
        }
        "vault-config" => {
            let value = exact_object(
                facts,
                &[
                    "domain",
                    "verb",
                    "connector",
                    "record_key",
                    "before",
                    "after",
                ],
                Rejection::Facts,
                "vault-config mutation facts",
            )?;
            if !matches!(value["verb"].as_str(), Some("create" | "edit" | "delete")) {
                return Err(rejected(Rejection::Facts, "unknown vault-config verb"));
            }
            if !value["connector"].as_str().is_some_and(is_token) {
                return Err(rejected(Rejection::Facts, "non-canonical connector"));
            }
            let record = required_commitment(&value["record_key"], Rejection::Facts, "record_key")?;
            if record != vault_record_key {
                return Err(rejected(Rejection::Facts, "mismatched vault record_key"));
            }
            value
        }
        _ => return Err(rejected(Rejection::Facts, "unknown mutation domain")),
    };

    let catalog = state_catalog(state_facts).map_err(|error| match error {
        Error::InvalidStateFact(detail) => Error::InvalidOperationFacts(detail),
        other => other,
    })?;
    let before = logical_state(&value["before"], &catalog).map_err(|error| match error {
        Error::InvalidStateFact(detail) => Error::InvalidOperationFacts(detail),
        other => other,
    })?;
    let after = logical_state(&value["after"], &catalog).map_err(|error| match error {
        Error::InvalidStateFact(detail) => Error::InvalidOperationFacts(detail),
        other => other,
    })?;
    transition(
        value["verb"].as_str().unwrap_or(""),
        &value["before"],
        &value["after"],
    )?;
    if domain == "vault-config" {
        for digest in [before, after].into_iter().flatten() {
            let document = catalog
                .iter()
                .find_map(|(candidate, document)| (candidate == digest).then_some(*document))
                .ok_or_else(|| {
                    rejected(
                        Rejection::Facts,
                        "present state is absent from the state catalog",
                    )
                })?;
            if !document["objects"].as_array().is_some_and(|objects| {
                objects
                    .iter()
                    .any(|object| object["key_commitment"] == value["record_key"])
            }) {
                return Err(rejected(
                    Rejection::Facts,
                    "vault record_key absent from present state",
                ));
            }
        }
    }
    Ok(())
}

fn source_edition(fixtures: &Value, name: &str) -> Result<String> {
    let fixture = fixtures
        .get(name)
        .ok_or_else(|| rejected(Rejection::Facts, "source manifest fixture is missing"))?;
    let mut document = fixture
        .get("document")
        .cloned()
        .ok_or_else(|| rejected(Rejection::Facts, "source manifest document is missing"))?;
    let signature = document
        .get_mut("signature")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| rejected(Rejection::Facts, "source manifest signature is missing"))?;
    if !signature.contains_key("value") {
        return Err(rejected(
            Rejection::Facts,
            "source manifest signature value is missing",
        ));
    }
    signature.insert("value".into(), Value::String(String::new()));
    Ok(sha256_text(
        canonical(&document, Rejection::Facts)?.as_bytes(),
    ))
}

fn canonical_gamma_query(value: &str) -> Result<&str> {
    if value == "read.gamma" {
        return Ok(value);
    }
    let selector_text = value
        .strip_prefix("read.gamma#")
        .filter(|text| !text.is_empty())
        .ok_or_else(|| rejected(Rejection::Facts, "Gamma query is not canonical"))?;
    const ORDER: [&str; 7] = ["dir", "id", "tag", "kind", "action", "since", "until"];
    let mut seen = BTreeSet::new();
    let mut last = None;
    for part in selector_text.split('&') {
        let (name, selector) = part
            .split_once('=')
            .filter(|(_, selector)| !selector.is_empty())
            .ok_or_else(|| rejected(Rejection::Facts, "Gamma selector is empty"))?;
        let order = ORDER
            .iter()
            .position(|candidate| *candidate == name)
            .ok_or_else(|| rejected(Rejection::Facts, "unknown Gamma selector"))?;
        if !seen.insert(name) || last.is_some_and(|previous| order <= previous) {
            return Err(rejected(
                Rejection::Facts,
                "Gamma selectors are duplicated or out of order",
            ));
        }
        last = Some(order);
        match name {
            "dir" => {
                let mut sids = BTreeSet::new();
                for segment in selector.split('/') {
                    if !is_ulid(segment) || !sids.insert(segment) {
                        return Err(rejected(Rejection::Facts, "Gamma dir is not canonical"));
                    }
                }
            }
            "id" if !is_ulid(selector) => {
                return Err(rejected(Rejection::Facts, "Gamma id is not canonical"));
            }
            "tag"
                if selector.len() > 64
                    || selector.is_empty()
                    || !selector.bytes().all(|byte| {
                        byte.is_ascii_lowercase()
                            || byte.is_ascii_digit()
                            || matches!(byte, b'_' | b'-')
                    }) =>
            {
                return Err(rejected(Rejection::Facts, "Gamma tag is not canonical"));
            }
            "kind" | "action" if !is_token(selector) => {
                return Err(rejected(Rejection::Facts, "Gamma token is not canonical"));
            }
            "since" | "until" => {
                CanonicalTimestamp::parse(selector)
                    .map_err(|()| rejected(Rejection::Facts, "Gamma timestamp is not canonical"))?;
            }
            _ => {}
        }
    }
    Ok(value)
}

fn validate_read(facts: &Value, context: &Value, fixtures: &Value) -> Result<()> {
    let context = context
        .as_object()
        .ok_or_else(|| rejected(Rejection::Facts, "read context is not an object"))?;
    let domain = facts.get("domain").and_then(Value::as_str).unwrap_or("");
    match domain {
        "ethos" => {
            let value = exact_object(
                facts,
                &["domain", "zone", "sid", "source_edition"],
                Rejection::Facts,
                "Ethos read facts",
            )?;
            if !matches!(value["zone"].as_str(), Some("public" | "circle" | "self")) {
                return Err(rejected(Rejection::Facts, "unknown Ethos read zone"));
            }
            strict_sid(&value["sid"], Rejection::Facts, "Ethos read SID")?;
            required_commitment(&value["source_edition"], Rejection::Facts, "source_edition")?;
            if value["zone"] != context["zone"] || value["sid"] != context["sid"] {
                return Err(rejected(Rejection::Facts, "Ethos native target mismatch"));
            }
            let fixture_name = context["source_manifest"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "source manifest name is missing"))?;
            if value["source_edition"] != source_edition(fixtures, fixture_name)? {
                return Err(rejected(Rejection::Facts, "Ethos source edition mismatch"));
            }
        }
        "gamma" => {
            let value = exact_object(
                facts,
                &["domain", "source_head", "request_digest"],
                Rejection::Facts,
                "Gamma read facts",
            )?;
            let source_head =
                required_commitment(&value["source_head"], Rejection::Facts, "source_head")?;
            let request =
                required_commitment(&value["request_digest"], Rejection::Facts, "request_digest")?;
            if context["source_head"].as_str() != Some(source_head) {
                return Err(rejected(Rejection::Facts, "Gamma source head mismatch"));
            }
            let query = context["query"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "Gamma query is missing"))?;
            let query = canonical_gamma_query(query)?;
            if request != commitment(GAMMA_READ_DOMAIN, query.as_bytes()) {
                return Err(rejected(Rejection::Facts, "Gamma request digest mismatch"));
            }
        }
        "vault-config" => {
            let value = exact_object(
                facts,
                &["domain", "connector", "record_key", "source_edition"],
                Rejection::Facts,
                "vault-config read facts",
            )?;
            if value["connector"] != context["connector"] {
                return Err(rejected(Rejection::Facts, "vault connector mismatch"));
            }
            let record = required_commitment(&value["record_key"], Rejection::Facts, "record_key")?;
            let store_key = context["store_key_utf8"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "vault store key is missing"))?;
            if record != commitment(STATE_KEY_DOMAIN, store_key.as_bytes()) {
                return Err(rejected(Rejection::Facts, "vault record_key mismatch"));
            }
            required_commitment(&value["source_edition"], Rejection::Facts, "source_edition")?;
            let fixture_name = context["source_manifest"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "source manifest name is missing"))?;
            if value["source_edition"] != source_edition(fixtures, fixture_name)? {
                return Err(rejected(Rejection::Facts, "vault source edition mismatch"));
            }
        }
        _ => return Err(rejected(Rejection::Facts, "unknown read domain")),
    }
    Ok(())
}

fn required_token<'a>(value: &'a Value, label: &str) -> Result<&'a str> {
    let value = value
        .as_str()
        .ok_or_else(|| rejected(Rejection::Facts, format!("{label} is not text")))?;
    if !is_token(value) {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} is not a canonical non-empty identifier"),
        ));
    }
    Ok(value)
}

fn validate_catalog_ref(value: &Value, context: &Map<String, Value>) -> Result<()> {
    let reference = exact_object(
        value,
        &["catalog_version", "catalog_digest", "approval_digest"],
        Rejection::Facts,
        "catalog_ref",
    )?;
    let version = reference["catalog_version"]
        .as_str()
        .filter(|value| is_version(value))
        .ok_or_else(|| rejected(Rejection::Facts, "catalog version is not canonical"))?;
    if version.is_empty() {
        return Err(rejected(Rejection::Facts, "catalog version is empty"));
    }
    required_commitment(
        &reference["catalog_digest"],
        Rejection::Facts,
        "catalog digest",
    )?;
    required_commitment(
        &reference["approval_digest"],
        Rejection::Facts,
        "approval digest",
    )?;
    if value != &context["catalog_ref"] {
        return Err(rejected(
            Rejection::Facts,
            "catalog reference does not match native evidence",
        ));
    }
    Ok(())
}

fn validate_applicability(
    value: &Value,
    label: &str,
    expected_applicable: bool,
    expected_value: &str,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| rejected(Rejection::Facts, format!("{label} is not an object")))?;
    match object.get("state").and_then(Value::as_str) {
        Some("not-applicable") => {
            exact_object(
                value,
                &["state"],
                Rejection::Facts,
                &format!("{label} applicability"),
            )?;
            if expected_applicable {
                return Err(rejected(
                    Rejection::Facts,
                    format!("applicable {label} was omitted"),
                ));
            }
        }
        Some("cited") => {
            let member = if label == "budget" {
                "budget_ref"
            } else {
                "purpose_ref"
            };
            let cited = exact_object(
                value,
                &["state", member],
                Rejection::Facts,
                &format!("{label} applicability"),
            )?;
            if !expected_applicable {
                return Err(rejected(
                    Rejection::Facts,
                    format!("non-applicable {label} was volunteered"),
                ));
            }
            let actual = if label == "budget" {
                required_token(&cited[member], member)?
            } else {
                required_commitment(&cited[member], Rejection::Facts, member)?
            };
            if actual != expected_value {
                return Err(rejected(
                    Rejection::Facts,
                    format!("{label} citation mismatch"),
                ));
            }
        }
        _ => {
            return Err(rejected(
                Rejection::Facts,
                format!("unknown {label} applicability state"),
            ));
        }
    }
    Ok(())
}

fn validate_action_inference(facts: &Value, kind: &str, context: &Value) -> Result<()> {
    let context = context
        .as_object()
        .ok_or_else(|| rejected(Rejection::Facts, "native context is not an object"))?;
    match kind {
        "action" => {
            let value = exact_object(
                facts,
                &[
                    "connector",
                    "action",
                    "catalog_ref",
                    "args_hash",
                    "budget",
                    "purpose",
                ],
                Rejection::Facts,
                "action facts",
            )?;
            let connector = required_token(&value["connector"], "connector")?;
            let action = required_token(&value["action"], "action")?;
            if context["connector"].as_str() != Some(connector)
                || context["action"].as_str() != Some(action)
            {
                return Err(rejected(
                    Rejection::Facts,
                    "native connector action mismatch",
                ));
            }
            validate_catalog_ref(&value["catalog_ref"], context)?;
            let args_hash =
                required_commitment(&value["args_hash"], Rejection::Facts, "args_hash")?;
            let expected_args =
                sha256_text(canonical(&context["args"], Rejection::Facts)?.as_bytes());
            if args_hash != expected_args {
                return Err(rejected(
                    Rejection::Facts,
                    "action arguments do not match args_hash",
                ));
            }
            let budget_applicable = context["budget_applicable"]
                .as_bool()
                .ok_or_else(|| rejected(Rejection::Facts, "budget applicability is missing"))?;
            let budget_ref = context["budget_ref"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "budget_ref is missing"))?;
            validate_applicability(&value["budget"], "budget", budget_applicable, budget_ref)?;
            let purpose_applicable = context["purpose_applicable"]
                .as_bool()
                .ok_or_else(|| rejected(Rejection::Facts, "purpose applicability is missing"))?;
            let purpose_text = context["purpose_text"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "purpose text is missing"))?;
            validate_applicability(
                &value["purpose"],
                "purpose",
                purpose_applicable,
                &commitment(PURPOSE_DOMAIN, purpose_text.as_bytes()),
            )?;
        }
        "inference" => {
            let value = exact_object(
                facts,
                &["provider", "model", "request_digest", "budget", "purpose"],
                Rejection::Facts,
                "inference facts",
            )?;
            let provider = required_token(&value["provider"], "provider")?;
            let model = required_token(&value["model"], "model")?;
            if context["provider"].as_str() != Some(provider)
                || context["model"].as_str() != Some(model)
            {
                return Err(rejected(
                    Rejection::Facts,
                    "native inference provider or model mismatch",
                ));
            }
            let request =
                required_commitment(&value["request_digest"], Rejection::Facts, "request_digest")?;
            let body = context["request_body_hex"]
                .as_str()
                .and_then(|body| hex::decode(body).ok())
                .ok_or_else(|| rejected(Rejection::Facts, "request body is not exact hex"))?;
            if request != commitment(INFERENCE_REQUEST_DOMAIN, &body) {
                return Err(rejected(
                    Rejection::Facts,
                    "private inference request mismatch",
                ));
            }
            let budget_applicable = context["budget_applicable"]
                .as_bool()
                .ok_or_else(|| rejected(Rejection::Facts, "budget applicability is missing"))?;
            let budget_ref = context["budget_ref"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "budget_ref is missing"))?;
            validate_applicability(&value["budget"], "budget", budget_applicable, budget_ref)?;
            let purpose_applicable = context["purpose_applicable"]
                .as_bool()
                .ok_or_else(|| rejected(Rejection::Facts, "purpose applicability is missing"))?;
            let purpose_text = context["purpose_text"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Facts, "purpose text is missing"))?;
            validate_applicability(
                &value["purpose"],
                "purpose",
                purpose_applicable,
                &commitment(PURPOSE_DOMAIN, purpose_text.as_bytes()),
            )?;
        }
        _ => {
            return Err(rejected(
                Rejection::Facts,
                "operation kind does not select action or inference facts",
            ));
        }
    }
    Ok(())
}

fn certificate_fields(facts: &Map<String, Value>, context: &Map<String, Value>) -> Result<()> {
    let mandate_id = facts["mandate_id"]
        .as_str()
        .filter(|value| is_mandate_id(value))
        .ok_or_else(|| rejected(Rejection::Facts, "mandate_id is not canonical"))?;
    let digest = required_commitment(
        &facts["certificate_digest"],
        Rejection::Facts,
        "certificate_digest",
    )?;
    let certificate = context
        .get("certificate")
        .ok_or_else(|| rejected(Rejection::Facts, "certificate evidence is missing"))?;
    if certificate["id"].as_str() != Some(mandate_id) {
        return Err(rejected(
            Rejection::Facts,
            "mandate id and certificate mismatch",
        ));
    }
    if digest != sha256_text(canonical(certificate, Rejection::Facts)?.as_bytes()) {
        return Err(rejected(Rejection::Facts, "certificate digest mismatch"));
    }
    Ok(())
}

fn present_state(value: &Value, expected: &Value, label: &str) -> Result<()> {
    let state = exact_object(value, &["state", "state_ref"], Rejection::Facts, label)?;
    if state["state"] != "present" {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} is not present"),
        ));
    }
    let reference = exact_object(
        &state["state_ref"],
        &[STATE_PROFILE_KEY, "digest"],
        Rejection::Facts,
        &format!("{label} state_ref"),
    )?;
    if reference[STATE_PROFILE_KEY] != STATE_PROFILE {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} has unknown state profile"),
        ));
    }
    required_commitment(
        &reference["digest"],
        Rejection::Facts,
        &format!("{label} digest"),
    )?;
    if value != expected {
        return Err(rejected(
            Rejection::Facts,
            format!("{label} does not match native state"),
        ));
    }
    Ok(())
}

fn operation_reference_shape<'a>(
    value: &'a Value,
    kind: Rejection,
    label: &str,
) -> Result<&'a Map<String, Value>> {
    let reference = exact_object(
        value,
        &[OPERATION_PROFILE_KEY, "occurrence", "commitment"],
        kind,
        label,
    )?;
    if reference[OPERATION_PROFILE_KEY] != OPERATION_PROFILE {
        return Err(rejected(kind, format!("{label} has unknown profile")));
    }
    if !reference["occurrence"].as_str().is_some_and(is_occurrence) {
        return Err(rejected(kind, format!("{label} occurrence is malformed")));
    }
    required_commitment(
        &reference["commitment"],
        kind,
        &format!("{label} commitment"),
    )?;
    Ok(reference)
}

fn validate_structural(facts: &Value, kind: &str, context: &Value) -> Result<()> {
    let context = context
        .as_object()
        .ok_or_else(|| rejected(Rejection::Facts, "structural context is not an object"))?;
    match kind {
        "grant" => {
            let value = exact_object(
                facts,
                &["mandate_id", "certificate_digest"],
                Rejection::Facts,
                "grant facts",
            )?;
            certificate_fields(value, context)?;
        }
        "revoke" => {
            let value = exact_object(
                facts,
                &["mandate_id", "certificate_digest", "reason"],
                Rejection::Facts,
                "revoke facts",
            )?;
            certificate_fields(value, context)?;
            let reason = value["reason"]
                .as_object()
                .ok_or_else(|| rejected(Rejection::Facts, "reason is not an object"))?;
            match reason.get("state").and_then(Value::as_str) {
                Some("absent") => {
                    exact_object(
                        &value["reason"],
                        &["state"],
                        Rejection::Facts,
                        "absent reason",
                    )?;
                    if !context["native_reason"].is_null() {
                        return Err(rejected(Rejection::Facts, "native reason was omitted"));
                    }
                }
                Some("present") => {
                    let present = exact_object(
                        &value["reason"],
                        &["state", "text"],
                        Rejection::Facts,
                        "present reason",
                    )?;
                    let text = present["text"]
                        .as_str()
                        .filter(|text| !text.is_empty())
                        .ok_or_else(|| rejected(Rejection::Facts, "reason text is empty"))?;
                    if context["native_reason"].as_str() != Some(text) {
                        return Err(rejected(Rejection::Facts, "native reason mismatch"));
                    }
                }
                _ => return Err(rejected(Rejection::Facts, "unknown reason state")),
            }
        }
        "rotate" => {
            let object = facts
                .as_object()
                .ok_or_else(|| rejected(Rejection::Facts, "rotate facts is not an object"))?;
            let domain = object.get("domain").and_then(Value::as_str).unwrap_or("");
            let keys: &[&str] = match domain {
                "ethos-zone" => &["domain", "zone", "mode", "before", "after"],
                "ethos-node" => &["domain", "zone", "sid", "mode", "before", "after"],
                "vault" => &["domain", "connector", "mode", "before", "after"],
                "identity" => &[
                    "domain",
                    "previous_did",
                    "next_did",
                    "transition_digest",
                    "before",
                    "after",
                ],
                _ => return Err(rejected(Rejection::Facts, "unknown rotate domain")),
            };
            let value = exact_object(facts, keys, Rejection::Facts, "rotate facts")?;
            if context["derived"].as_bool() == Some(true) {
                return Err(rejected(
                    Rejection::Facts,
                    "derived rotation has a second occurrence",
                ));
            }
            if domain != "identity" {
                let mode = value["mode"].as_str().unwrap_or("");
                if !matches!(mode, "rotate" | "reencrypt") {
                    return Err(rejected(Rejection::Facts, "unknown rotate mode"));
                }
                if context["mode"].as_str() != Some(mode) {
                    return Err(rejected(Rejection::Facts, "native rotate mode mismatch"));
                }
            }
            if matches!(domain, "ethos-zone" | "ethos-node") {
                let zone = value["zone"].as_str().unwrap_or("");
                if !matches!(zone, "public" | "circle" | "self") {
                    return Err(rejected(Rejection::Facts, "unknown rotate zone"));
                }
                if context["zone"].as_str() != Some(zone) {
                    return Err(rejected(Rejection::Facts, "native rotate zone mismatch"));
                }
            }
            if domain == "ethos-node" {
                let sid = value["sid"].as_str().unwrap_or("");
                if !is_ulid(sid) || context["sid"].as_str() != Some(sid) {
                    return Err(rejected(Rejection::Facts, "native rotate SID mismatch"));
                }
            }
            if domain == "vault" {
                let connector = value["connector"].as_str().unwrap_or("");
                if !is_token(connector) || context["connector"].as_str() != Some(connector) {
                    return Err(rejected(
                        Rejection::Facts,
                        "native vault connector mismatch",
                    ));
                }
            }
            if domain == "identity" {
                let previous = value["previous_did"].as_str().unwrap_or("");
                let next = value["next_did"].as_str().unwrap_or("");
                if !is_did(previous) || !is_did(next) || previous == next {
                    return Err(rejected(
                        Rejection::Facts,
                        "identity rotation DIDs are invalid",
                    ));
                }
                if context["previous_did"].as_str() != Some(previous)
                    || context["next_did"].as_str() != Some(next)
                {
                    return Err(rejected(Rejection::Facts, "identity rotation DID mismatch"));
                }
                let digest = required_commitment(
                    &value["transition_digest"],
                    Rejection::Facts,
                    "transition_digest",
                )?;
                let transition = &context["transition"];
                if transition["prev_did"].as_str() != Some(previous)
                    || transition["next_did"].as_str() != Some(next)
                    || digest != sha256_text(canonical(transition, Rejection::Facts)?.as_bytes())
                {
                    return Err(rejected(Rejection::Facts, "identity transition mismatch"));
                }
            }
            present_state(&value["before"], &context["before"], "rotate before")?;
            present_state(&value["after"], &context["after"], "rotate after")?;
            if value["before"]["state_ref"]["digest"] == value["after"]["state_ref"]["digest"] {
                return Err(rejected(Rejection::Facts, "rotate state digests are equal"));
            }
        }
        "publication" => {
            let object = facts
                .as_object()
                .ok_or_else(|| rejected(Rejection::Facts, "publication facts is not an object"))?;
            let mode = object.get("mode").and_then(Value::as_str).unwrap_or("");
            let keys: &[&str] = match mode {
                "normal" | "merge" => &[
                    "mode",
                    "height",
                    "predecessors",
                    "changeset_ref",
                    "contained_operations",
                ],
                "resolution" => &[
                    "mode",
                    "height",
                    "predecessors",
                    "winner",
                    "changeset_ref",
                    "contained_operations",
                ],
                _ => return Err(rejected(Rejection::Facts, "unknown publication mode")),
            };
            let value = exact_object(facts, keys, Rejection::Facts, "publication facts")?;
            let height = value["height"]
                .as_u64()
                .filter(|height| *height >= 1)
                .ok_or_else(|| rejected(Rejection::Facts, "publication height is invalid"))?;
            let predecessors = value["predecessors"]
                .as_array()
                .ok_or_else(|| rejected(Rejection::Facts, "predecessors is not an array"))?;
            let predecessor_text: Vec<&str> = predecessors
                .iter()
                .map(|value| {
                    required_commitment(value, Rejection::Facts, "publication predecessor")
                })
                .collect::<Result<_>>()?;
            if predecessor_text.iter().collect::<BTreeSet<_>>().len() != predecessor_text.len() {
                return Err(rejected(
                    Rejection::Facts,
                    "publication predecessors contain a duplicate",
                ));
            }
            if mode == "normal" {
                let expected = usize::from(height != 1);
                if predecessors.len() != expected {
                    return Err(rejected(
                        Rejection::Facts,
                        "normal predecessor cardinality mismatch",
                    ));
                }
            } else {
                if height < 3 || predecessors.len() != 2 {
                    return Err(rejected(
                        Rejection::Facts,
                        "fork predecessor cardinality mismatch",
                    ));
                }
                let mut sorted = predecessor_text.clone();
                sorted.sort();
                if predecessor_text != sorted {
                    return Err(rejected(
                        Rejection::Facts,
                        "fork predecessors are not sorted",
                    ));
                }
            }
            if mode == "resolution"
                && !predecessors
                    .iter()
                    .any(|predecessor| predecessor == &value["winner"])
            {
                return Err(rejected(
                    Rejection::Facts,
                    "resolution winner is outside predecessors",
                ));
            }
            let changeset = exact_object(
                &value["changeset_ref"],
                &["aithos-changeset-core", "digest"],
                Rejection::Facts,
                "changeset_ref",
            )?;
            if changeset["aithos-changeset-core"] != "1.0.0-draft.1" {
                return Err(rejected(Rejection::Facts, "unknown changeset profile"));
            }
            required_commitment(&changeset["digest"], Rejection::Facts, "changeset digest")?;
            let operations = value["contained_operations"].as_array().ok_or_else(|| {
                rejected(Rejection::Facts, "contained_operations is not an array")
            })?;
            let occurrences: Vec<&str> = operations
                .iter()
                .map(|operation| {
                    operation_reference_shape(
                        operation,
                        Rejection::Facts,
                        "contained operation_ref",
                    )
                    .and_then(|reference| {
                        reference["occurrence"].as_str().ok_or_else(|| {
                            rejected(Rejection::Facts, "contained occurrence is not text")
                        })
                    })
                })
                .collect::<Result<_>>()?;
            if occurrences.iter().collect::<BTreeSet<_>>().len() != occurrences.len() {
                return Err(rejected(
                    Rejection::Facts,
                    "contained operation occurrence is duplicated",
                ));
            }
            if context
                .get("publication_occurrence")
                .and_then(Value::as_str)
                .is_some_and(|publication| occurrences.contains(&publication))
            {
                return Err(rejected(
                    Rejection::Facts,
                    "publication occurrence is self-referenced",
                ));
            }
            for field in [
                "height",
                "predecessors",
                "changeset_ref",
                "contained_operations",
            ] {
                if value[field] != context[field] {
                    return Err(rejected(
                        Rejection::Facts,
                        format!("publication {field} mismatch"),
                    ));
                }
            }
            if mode == "resolution" && value["winner"] != context["winner"] {
                return Err(rejected(Rejection::Facts, "publication winner mismatch"));
            }
        }
        _ => {
            return Err(rejected(
                Rejection::Facts,
                "operation kind does not select a structural family",
            ));
        }
    }
    Ok(())
}
pub fn verify_operation_facts(input: OperationFactsInput<'_>) -> Result<VerifiedOperationFacts> {
    let envelope = operation_envelope(input.document)?;
    let kind = envelope["kind"]
        .as_str()
        .ok_or_else(|| rejected(Rejection::Facts, "operation kind is not text"))?;
    match input.evidence {
        OperationFactsEvidence::Mutation {
            state_facts,
            nodes,
            vault_record_key,
        } => {
            if kind != "mutation" {
                return Err(rejected(Rejection::Facts, "operation kind is not mutation"));
            }
            validate_mutation(&envelope["facts"], state_facts, nodes, vault_record_key)?;
        }
        OperationFactsEvidence::Read { context, fixtures } => {
            if kind != "read" {
                return Err(rejected(Rejection::Facts, "operation kind is not read"));
            }
            validate_read(&envelope["facts"], context, fixtures)?;
        }
        OperationFactsEvidence::ActionInference { context } => {
            validate_action_inference(&envelope["facts"], kind, context)?;
        }
        OperationFactsEvidence::Structural { context } => {
            validate_structural(&envelope["facts"], kind, context)?;
        }
    }
    Ok(VerifiedOperationFacts {
        kind: kind.to_owned(),
        digest: facts_digest(input.document, input.facts_ref)?,
    })
}

pub fn verify_state_fact(input: StateFactInput<'_>) -> Result<VerifiedStateFact> {
    match input {
        StateFactInput::LogicalState { state, state_facts } => {
            let catalog = match state_facts {
                Some(state_facts) => state_catalog(state_facts)?,
                None => Vec::new(),
            };
            let digest = logical_state(state, &catalog)?.unwrap_or("").to_owned();
            Ok(VerifiedStateFact { digest })
        }
        StateFactInput::Document {
            document,
            expected_key_commitments,
        } => Ok(VerifiedStateFact {
            digest: state_document(document, expected_key_commitments)?,
        }),
        StateFactInput::Reference { state, document } => {
            let digest = state_document(document, None)?;
            let catalog = [(digest.clone(), document)];
            let selected = logical_state(state, &catalog)?
                .ok_or_else(|| rejected(Rejection::State, "state reference is absent"))?;
            if selected != digest {
                return Err(rejected(Rejection::State, "state_ref digest mismatch"));
            }
            Ok(VerifiedStateFact { digest })
        }
    }
}

fn verified_mandate(value: &Value, issuer: Option<&Mandate>) -> Result<Mandate> {
    let encoded = serde_json::to_string(value)
        .map_err(|error| rejected(Rejection::Operation, error.to_string()))?;
    let mandate: Mandate = encoded
        .parse()
        .map_err(|error: Error| rejected(Rejection::Operation, error.to_string()))?;
    let key = match issuer {
        Some(parent) => parent
            .grantee_pub()
            .map_err(|error| rejected(Rejection::Operation, error.to_string()))?,
        None => {
            let root = mandate
                .subject
                .strip_prefix("did:aithos:")
                .ok_or_else(|| rejected(Rejection::Operation, "invalid mandate subject"))?;
            let bytes = wire::multibase_to_ed25519_pub(root)
                .map_err(|_| rejected(Rejection::Operation, "invalid mandate root key"))?;
            VerifyingKey::from_bytes(&bytes)
                .map_err(|_| rejected(Rejection::Operation, "invalid mandate root key"))?
        }
    };
    mandate::verify_sig(&mandate, &key)
        .map_err(|error| rejected(Rejection::Operation, error.to_string()))?;
    Ok(mandate)
}

fn facts_document_for<'a>(digest: &str, documents: &'a [&'a Value]) -> Result<&'a Value> {
    for document in documents {
        let candidate = commitment(
            FACTS_DOMAIN,
            canonical(document, Rejection::Facts)?.as_bytes(),
        );
        if candidate == digest {
            return Ok(document);
        }
    }
    Err(rejected(
        Rejection::Facts,
        "selected facts document is missing",
    ))
}

fn validate_selected_facts(facts_ref: &Value, kind: &str, documents: &[&Value]) -> Result<()> {
    let reference = exact_object(
        facts_ref,
        &[FACTS_PROFILE_KEY, "digest"],
        Rejection::Facts,
        "facts_ref",
    )?;
    if reference[FACTS_PROFILE_KEY] != FACTS_PROFILE {
        return Err(rejected(Rejection::Facts, "unknown facts profile"));
    }
    let digest = required_commitment(&reference["digest"], Rejection::Facts, "facts_ref.digest")?;
    let document = facts_document_for(digest, documents)?;
    let envelope = operation_envelope(document)?;
    if envelope["kind"].as_str() != Some(kind) {
        return Err(rejected(Rejection::Facts, "selected facts family mismatch"));
    }
    Ok(())
}

fn authority(
    value: &Value,
    projection: &Map<String, Value>,
    certificates: &[&Value],
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| rejected(Rejection::Operation, "authority is not an object"))?;
    match object.get("actor").and_then(Value::as_str) {
        Some("owner") => {
            exact_object(value, &["actor"], Rejection::Operation, "owner authority")?;
            Ok(())
        }
        Some("grantee") => {
            let authority = exact_object(
                value,
                &["actor", "key", "authorized_by", "authorized_via"],
                Rejection::Operation,
                "grantee authority",
            )?;
            let key = authority["key"]
                .as_str()
                .filter(|key| is_key(key))
                .ok_or_else(|| rejected(Rejection::Operation, "invalid grantee authority key"))?;
            let authorized_by = authority["authorized_by"]
                .as_str()
                .filter(|id| is_mandate_id(id))
                .ok_or_else(|| rejected(Rejection::Operation, "invalid authorized_by"))?;
            let via = authority["authorized_via"]
                .as_array()
                .filter(|via| !via.is_empty())
                .ok_or_else(|| {
                    rejected(Rejection::Operation, "authorized_via must be non-empty")
                })?;
            let mut ids = BTreeSet::new();
            let mut previous: Option<Mandate> = None;
            for item in via {
                let item = exact_object(
                    item,
                    &["id", "certificate_digest"],
                    Rejection::Operation,
                    "authorized_via item",
                )?;
                let id = item["id"]
                    .as_str()
                    .filter(|id| is_mandate_id(id))
                    .ok_or_else(|| {
                        rejected(Rejection::Operation, "invalid authority mandate id")
                    })?;
                if !ids.insert(id) {
                    return Err(rejected(
                        Rejection::Operation,
                        "duplicate authority mandate id",
                    ));
                }
                let certificate = certificates
                    .iter()
                    .copied()
                    .find(|certificate| certificate["id"].as_str() == Some(id))
                    .ok_or_else(|| {
                        rejected(Rejection::Operation, "authority certificate is missing")
                    })?;
                if certificate["subject"] != projection["subject"] {
                    return Err(rejected(
                        Rejection::Operation,
                        "certificate subject mismatch",
                    ));
                }
                let digest = required_commitment(
                    &item["certificate_digest"],
                    Rejection::Operation,
                    "certificate digest",
                )?;
                if digest != sha256_text(canonical(certificate, Rejection::Operation)?.as_bytes()) {
                    return Err(rejected(
                        Rejection::Operation,
                        "certificate digest mismatch",
                    ));
                }
                let parsed = verified_mandate(certificate, previous.as_ref())?;
                if let Some(parent) = &previous {
                    if parsed.parent.as_deref() != Some(parent.id.as_str())
                        || parsed.issued_by != parent.grantee.pubkey
                    {
                        return Err(rejected(
                            Rejection::Operation,
                            "certificate chain link mismatch",
                        ));
                    }
                }
                previous = Some(parsed);
            }
            let leaf = previous.ok_or_else(|| {
                rejected(
                    Rejection::Operation,
                    "authorized_via must select a leaf mandate",
                )
            })?;
            if leaf.id != authorized_by {
                return Err(rejected(
                    Rejection::Operation,
                    "authorized_by is not the leaf",
                ));
            }
            if leaf.grantee.pubkey != key {
                return Err(rejected(
                    Rejection::Operation,
                    "authority key is not the leaf grantee",
                ));
            }
            Ok(())
        }
        _ => Err(rejected(Rejection::Operation, "unknown authority actor")),
    }
}

pub fn verify_operation_projection(
    projection: &Value,
    evidence: OperationProjectionEvidence<'_>,
) -> Result<VerifiedOperationProjection> {
    let projection = exact_object(
        projection,
        &[
            OPERATION_PROFILE_KEY,
            "occurrence",
            "subject",
            "at",
            "history_heads",
            "authority",
            "operation",
        ],
        Rejection::Operation,
        "operation projection",
    )?;
    if projection[OPERATION_PROFILE_KEY] != OPERATION_PROFILE {
        return Err(rejected(Rejection::Operation, "unknown operation profile"));
    }
    let occurrence = projection["occurrence"]
        .as_str()
        .filter(|occurrence| is_occurrence(occurrence))
        .ok_or_else(|| rejected(Rejection::Operation, "invalid occurrence"))?;
    if !projection["subject"].as_str().is_some_and(is_did) {
        return Err(rejected(Rejection::Operation, "invalid subject"));
    }
    parse_timestamp(&projection["at"], Rejection::Operation, "at")?;
    let heads = projection["history_heads"]
        .as_array()
        .filter(|heads| heads.len() <= 2)
        .ok_or_else(|| rejected(Rejection::Operation, "history_heads cardinality is invalid"))?;
    let head_text: Vec<&str> = heads
        .iter()
        .map(|head| required_commitment(head, Rejection::Operation, "history head"))
        .collect::<Result<_>>()?;
    let mut sorted = head_text.clone();
    sorted.sort();
    sorted.dedup();
    if sorted != head_text {
        return Err(rejected(
            Rejection::Operation,
            "history heads are not distinct and sorted",
        ));
    }
    authority(&projection["authority"], projection, evidence.certificates)?;
    let operation = exact_object(
        &projection["operation"],
        &["kind", "facts_ref"],
        Rejection::Operation,
        "operation",
    )?;
    let kind = operation["kind"]
        .as_str()
        .filter(|kind| {
            matches!(
                *kind,
                "read"
                    | "mutation"
                    | "action"
                    | "inference"
                    | "grant"
                    | "revoke"
                    | "rotate"
                    | "publication"
            )
        })
        .ok_or_else(|| rejected(Rejection::Operation, "unknown operation kind"))?;
    validate_selected_facts(&operation["facts_ref"], kind, evidence.facts_documents)?;
    let projection_value = Value::Object(projection.clone());
    let derived = commitment(
        OPERATION_DOMAIN,
        canonical(&projection_value, Rejection::Operation)?.as_bytes(),
    );
    Ok(VerifiedOperationProjection {
        operation_ref: serde_json::json!({
            "aithos-operation-core": OPERATION_PROFILE,
            "occurrence": occurrence,
            "commitment": derived,
        }),
    })
}

pub fn verify_operation_reference(
    reference: &Value,
    projection: &VerifiedOperationProjection,
) -> Result<()> {
    operation_reference_shape(reference, Rejection::Operation, "operation_ref")?;
    if reference != projection.operation_ref() {
        return Err(rejected(
            Rejection::Operation,
            "operation_ref does not select the projection",
        ));
    }
    Ok(())
}

pub fn correlate_operation_references(
    first: &Value,
    second: &Value,
) -> Result<OperationCorrelation> {
    let first = operation_reference_shape(first, Rejection::Operation, "first operation_ref")?;
    let second = operation_reference_shape(second, Rejection::Operation, "second operation_ref")?;
    if first["occurrence"] == second["occurrence"] {
        if first["commitment"] != second["commitment"] {
            return Err(rejected(
                Rejection::Operation,
                "operation occurrence equivocation",
            ));
        }
        Ok(OperationCorrelation::Correlated)
    } else {
        Ok(OperationCorrelation::Distinct)
    }
}

fn session_mandate(value: &Value) -> Result<Mandate> {
    // SC1 proves only the session layer. Full T3 form, chain, perimeter and
    // constraints remain a separate prerequisite before any authorization.
    let mandate: Mandate = serde_json::from_value(value.clone())
        .map_err(|error| rejected(Rejection::Session, error.to_string()))?;
    if mandate.parent.is_some() {
        return Err(rejected(
            Rejection::Session,
            "SC1 leaf chain is required for a non-root mandate",
        ));
    }
    let root = mandate
        .subject
        .strip_prefix("did:aithos:")
        .ok_or_else(|| rejected(Rejection::Session, "invalid mandate subject"))?;
    let bytes = wire::multibase_to_ed25519_pub(root)
        .map_err(|_| rejected(Rejection::Session, "invalid mandate root key"))?;
    let key = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| rejected(Rejection::Session, "invalid mandate root key"))?;
    mandate::verify_sig(&mandate, &key)
        .map_err(|error| rejected(Rejection::Session, error.to_string()))?;
    Ok(mandate)
}

type SessionProjection<'a> = (&'a Map<String, Value>, &'a Map<String, Value>);

fn session_projection<'a>(
    projection: &'a Value,
    mandate: &Mandate,
    session_key: &str,
) -> Result<SessionProjection<'a>> {
    let projection = exact_object(
        projection,
        &[
            OPERATION_PROFILE_KEY,
            "occurrence",
            "subject",
            "at",
            "history_heads",
            "authority",
            "operation",
        ],
        Rejection::Session,
        "session-bound projection",
    )?;
    if projection[OPERATION_PROFILE_KEY] != OPERATION_PROFILE {
        return Err(rejected(Rejection::Session, "unknown operation profile"));
    }
    if !projection["occurrence"].as_str().is_some_and(is_occurrence) {
        return Err(rejected(Rejection::Session, "invalid operation occurrence"));
    }
    if projection["subject"].as_str() != Some(mandate.subject.as_str()) {
        return Err(rejected(
            Rejection::Session,
            "operation subject differs from mandate",
        ));
    }
    parse_timestamp(&projection["at"], Rejection::Session, "operation at")?;
    let heads = projection["history_heads"]
        .as_array()
        .filter(|heads| heads.len() <= 2)
        .ok_or_else(|| rejected(Rejection::Session, "invalid history_heads"))?;
    let head_text: Vec<&str> = heads
        .iter()
        .map(|head| required_commitment(head, Rejection::Session, "history head"))
        .collect::<Result<_>>()?;
    let mut sorted = head_text.clone();
    sorted.sort();
    sorted.dedup();
    if sorted != head_text {
        return Err(rejected(
            Rejection::Session,
            "history heads are not distinct and sorted",
        ));
    }
    let authority = exact_object(
        &projection["authority"],
        &["actor", "key", "authorized_by", "authorized_via", "session"],
        Rejection::Session,
        "session authority",
    )?;
    if authority["actor"] != "grantee"
        || authority["key"].as_str() != Some(mandate.grantee.pubkey.as_str())
        || authority["authorized_by"].as_str() != Some(mandate.id.as_str())
    {
        return Err(rejected(
            Rejection::Session,
            "session authority does not select the leaf mandate",
        ));
    }
    let via = authority["authorized_via"]
        .as_array()
        .filter(|via| via.len() == 1)
        .ok_or_else(|| {
            rejected(
                Rejection::Session,
                "session authority must select the exact leaf chain",
            )
        })?;
    let via = exact_object(
        &via[0],
        &["id", "certificate_digest"],
        Rejection::Session,
        "authorized_via item",
    )?;
    let mandate_value = serde_json::to_value(mandate)
        .map_err(|error| rejected(Rejection::Session, error.to_string()))?;
    if via["id"].as_str() != Some(mandate.id.as_str())
        || required_commitment(
            &via["certificate_digest"],
            Rejection::Session,
            "mandate certificate digest",
        )? != sha256_text(canonical(&mandate_value, Rejection::Session)?.as_bytes())
    {
        return Err(rejected(
            Rejection::Session,
            "session authority mandate certificate mismatch",
        ));
    }
    let session = exact_object(
        &authority["session"],
        &["key", "certificate_digest"],
        Rejection::Session,
        "session fact",
    )?;
    if session["key"].as_str() != Some(session_key) {
        return Err(rejected(
            Rejection::Session,
            "authority session key mismatch",
        ));
    }
    required_commitment(
        &session["certificate_digest"],
        Rejection::Session,
        "session certificate digest",
    )?;
    let operation = exact_object(
        &projection["operation"],
        &["kind", "facts_ref"],
        Rejection::Session,
        "operation",
    )?;
    if !matches!(
        operation["kind"].as_str(),
        Some(
            "read"
                | "mutation"
                | "action"
                | "inference"
                | "grant"
                | "revoke"
                | "rotate"
                | "publication"
        )
    ) {
        return Err(rejected(Rejection::Session, "unknown operation kind"));
    }
    let facts_ref = exact_object(
        &operation["facts_ref"],
        &[FACTS_PROFILE_KEY, "digest"],
        Rejection::Session,
        "facts_ref",
    )?;
    if facts_ref[FACTS_PROFILE_KEY] != FACTS_PROFILE {
        return Err(rejected(Rejection::Session, "unknown facts profile"));
    }
    required_commitment(&facts_ref["digest"], Rejection::Session, "facts digest")?;
    Ok((projection, session))
}

fn verify_session_with_mandate(
    evidence: SessionEvidence<'_>,
    mandate: Mandate,
) -> Result<VerifiedSession> {
    let session_key = mandate.constraints["session_bind"]
        .as_str()
        .filter(|key| is_key(key))
        .ok_or_else(|| rejected(Rejection::Session, "leaf session_bind is missing"))?;
    let (projection, session) = session_projection(evidence.projection, &mandate, session_key)?;

    let certificate = exact_object(
        evidence.certificate,
        &[
            SESSION_PROFILE_KEY,
            "subject",
            "mandate_id",
            "key",
            "not_before",
            "not_after",
            "signature",
        ],
        Rejection::Session,
        "SC1 certificate",
    )?;
    if certificate[SESSION_PROFILE_KEY] != SESSION_PROFILE {
        return Err(rejected(Rejection::Session, "unknown SC1 profile"));
    }
    if certificate["subject"].as_str() != Some(mandate.subject.as_str()) {
        return Err(rejected(Rejection::Session, "SC1 subject mismatch"));
    }
    if certificate["mandate_id"].as_str() != Some(mandate.id.as_str()) {
        return Err(rejected(Rejection::Session, "SC1 leaf mandate mismatch"));
    }
    if certificate["key"].as_str() != Some(session_key) {
        return Err(rejected(Rejection::Session, "SC1 session key mismatch"));
    }
    let signature = exact_object(
        &certificate["signature"],
        &["alg", "key", "value"],
        Rejection::Session,
        "SC1 signature",
    )?;
    if signature["alg"] != "ed25519"
        || signature["key"].as_str() != Some(mandate.grantee.pubkey.as_str())
    {
        return Err(rejected(Rejection::Session, "SC1 signer mismatch"));
    }
    let mut unsigned = evidence.certificate.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    verify_ed25519(
        &mandate.grantee.pubkey,
        canonical(&unsigned, Rejection::Session)?.as_bytes(),
        &signature["value"],
        Rejection::Session,
    )?;

    let cert_before = parse_timestamp(
        &certificate["not_before"],
        Rejection::Session,
        "SC1 not_before",
    )?;
    let cert_after = parse_timestamp(
        &certificate["not_after"],
        Rejection::Session,
        "SC1 not_after",
    )?;
    let mandate_before = CanonicalTimestamp::parse(&mandate.not_before)
        .map_err(|()| rejected(Rejection::Session, "mandate not_before is invalid"))?;
    let mandate_after = CanonicalTimestamp::parse(&mandate.not_after)
        .map_err(|()| rejected(Rejection::Session, "mandate not_after is invalid"))?;
    let operation_at = parse_timestamp(&projection["at"], Rejection::Session, "operation at")?;
    if !cert_before.compare(&cert_after).is_lt() {
        return Err(rejected(Rejection::Session, "SC1 interval is empty"));
    }
    if cert_before.compare(&mandate_before).is_lt() || cert_after.compare(&mandate_after).is_gt() {
        return Err(rejected(
            Rejection::Session,
            "SC1 interval escapes leaf mandate",
        ));
    }
    if operation_at.compare(&cert_before).is_lt() || operation_at.compare(&cert_after).is_gt() {
        return Err(rejected(
            Rejection::Session,
            "operation is outside SC1 interval",
        ));
    }
    let certificate_digest =
        sha256_text(canonical(evidence.certificate, Rejection::Session)?.as_bytes());
    if session["certificate_digest"].as_str() != Some(certificate_digest.as_str()) {
        return Err(rejected(
            Rejection::Session,
            "SC1 certificate digest mismatch",
        ));
    }

    operation_reference_shape(evidence.operation_ref, Rejection::Session, "operation_ref")?;
    let projection_value = Value::Object(projection.clone());
    let expected_reference = serde_json::json!({
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            OPERATION_DOMAIN,
            canonical(&projection_value, Rejection::Session)?.as_bytes()
        ),
    });
    if evidence.operation_ref != &expected_reference {
        return Err(rejected(
            Rejection::Session,
            "operation_ref does not select session-bound projection",
        ));
    }

    let native = evidence
        .native_leaf_proof
        .ok_or_else(|| rejected(Rejection::Session, "native leaf proof is missing"))?;
    let native = exact_object(
        native,
        &["key", "sig"],
        Rejection::Session,
        "native leaf proof",
    )?;
    if native["key"].as_str() != Some(mandate.grantee.pubkey.as_str()) {
        return Err(rejected(
            Rejection::Session,
            "native leaf proof key mismatch",
        ));
    }
    if evidence.native_leaf_domain.is_empty() {
        return Err(rejected(
            Rejection::Session,
            "native leaf proof domain is empty",
        ));
    }
    let mut native_message = evidence.native_leaf_domain.to_vec();
    native_message
        .extend_from_slice(canonical(evidence.operation_ref, Rejection::Session)?.as_bytes());
    verify_ed25519(
        &mandate.grantee.pubkey,
        &native_message,
        &native["sig"],
        Rejection::Session,
    )?;

    let proof = evidence
        .session_proof
        .ok_or_else(|| rejected(Rejection::Session, "session proof is missing"))?;
    let proof = exact_object(
        proof,
        &[SESSION_PROOF_PROFILE_KEY, "operation_ref", "key", "sig"],
        Rejection::Session,
        "session proof",
    )?;
    if proof[SESSION_PROOF_PROFILE_KEY] != SESSION_PROOF_PROFILE {
        return Err(rejected(
            Rejection::Session,
            "unknown session-proof profile",
        ));
    }
    operation_reference_shape(
        &proof["operation_ref"],
        Rejection::Session,
        "session operation_ref",
    )?;
    if proof["operation_ref"] != *evidence.operation_ref {
        return Err(rejected(
            Rejection::Session,
            "session proof operation_ref mismatch",
        ));
    }
    if proof["key"].as_str() != Some(session_key) {
        return Err(rejected(Rejection::Session, "session proof key mismatch"));
    }
    let mut proof_preimage = proof.clone();
    proof_preimage.remove("sig");
    verify_ed25519(
        session_key,
        canonical(&Value::Object(proof_preimage), Rejection::Session)?.as_bytes(),
        &proof["sig"],
        Rejection::Session,
    )?;
    Ok(VerifiedSession {
        operation_ref: expected_reference,
    })
}

pub fn verify_session(evidence: SessionEvidence<'_>) -> Result<VerifiedSession> {
    let mandate = session_mandate(evidence.mandate)?;
    verify_session_with_mandate(evidence, mandate)
}

/// Verify a non-root session leaf and then reuse the frozen SC1/W1.1 and
/// double-possession verifier. Every chain failure is surfaced as one closed
/// session refusal; callers never get a partial authority token.
pub fn verify_delegated_session(evidence: DelegatedSessionEvidence<'_>) -> Result<VerifiedSession> {
    if evidence.chain.len() < 2
        || evidence
            .chain
            .last()
            .is_none_or(|leaf| leaf.parent.is_none())
    {
        return Err(rejected(
            Rejection::Session,
            "delegated session requires a non-root mandate chain",
        ));
    }
    mandate::verify_chain_revocable(
        evidence.chain,
        evidence.did,
        evidence.at,
        evidence.revocations,
    )
    .map_err(|error| {
        rejected(
            Rejection::Session,
            format!("delegated session chain is invalid: {error}"),
        )
    })?;
    let leaf = evidence.chain.last().expect("length checked");
    let leaf_value = serde_json::to_value(leaf).map_err(|error| {
        rejected(
            Rejection::Session,
            format!("delegated session leaf is not serializable: {error}"),
        )
    })?;
    if &leaf_value != evidence.session.mandate {
        return Err(rejected(
            Rejection::Session,
            "SC1 mandate does not select the verified delegated leaf",
        ));
    }
    if evidence
        .session
        .projection
        .get("at")
        .and_then(Value::as_str)
        != Some(evidence.at)
    {
        return Err(rejected(
            Rejection::Session,
            "operation time differs from delegated chain verification time",
        ));
    }
    verify_session_with_mandate(evidence.session, leaf.clone())
}

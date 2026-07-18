//! K1-C publication carriers.
//!
//! Bundle owns Store traversal and atomic writes. This module owns the one
//! pure semantic verdict over the typed changeset/evidence carriers and the
//! public replay context that Bundle reconstructed from the parent edition.

use crate::error::{Error, Result};
use crate::{jcs, wire};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CHANGESET_PROFILE: &str = "1.0.0-draft.1";
pub const EVIDENCE_PROFILE: &str = "1.0.0-draft.1";
pub const AUTHORSHIP_PROFILE: &str = "1.0.0-draft.1";
pub const PRESENTATION_PROFILE: &str = "1.0.0-draft.1";
pub const OPERATION_PROFILE: &str = "1.0.0-draft.1";
pub const DELEGATED_COUNTS_PROFILE: &str = "1.0.0-draft.1";

const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";
const OPERATION_FACTS_DOMAIN: &str = "aithos-core/v1/operation-facts";
const CHANGESET_DOMAIN: &str = "aithos-core/v1/changeset";
const EVIDENCE_DOMAIN: &str = "aithos-core/v1/evidence";
const STATE_KEY_DOMAIN: &str = "aithos-core/v1/state-key";
const STATE_BYTES_DOMAIN: &str = "aithos-core/v1/state-bytes";

fn invalid(detail: impl Into<String>) -> Error {
    Error::InvalidOperation(detail.into())
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

fn canonical(value: &Value, label: &str) -> Result<Vec<u8>> {
    jcs::canonical_bytes(value).map_err(|error| invalid(format!("{label} JCS failed: {error}")))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

fn commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn is_prefixed_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn is_bare_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn verify_key_signature(key: &str, bytes: &[u8], signature: &Value, label: &str) -> Result<()> {
    let public = wire::multibase_to_ed25519_pub(key)
        .map_err(|error| invalid(format!("{label} key is invalid: {error}")))?;
    let verifying_key = VerifyingKey::from_bytes(&public)
        .map_err(|_| invalid(format!("{label} key is malformed")))?;
    let signature_bytes: [u8; 64] = signature
        .as_str()
        .and_then(|value| hex::decode(value).ok())
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| invalid(format!("{label} signature encoding is invalid")))?;
    verifying_key
        .verify(bytes, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| invalid(format!("{label} signature does not verify")))
}

fn verify_omitted_signature(document: &Value, key: &str, label: &str) -> Result<()> {
    let object = document
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    let signature = object
        .get("sig")
        .ok_or_else(|| invalid(format!("{label} signature is missing")))?;
    let mut unsigned = object.clone();
    unsigned.remove("sig");
    verify_key_signature(
        key,
        &canonical(&Value::Object(unsigned), label)?,
        signature,
        label,
    )
}

fn verify_blank_value_signature(document: &Value, key: &str, label: &str) -> Result<()> {
    let object = document
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    let signature = exact_object(
        object
            .get("signature")
            .ok_or_else(|| invalid(format!("{label} signature is missing")))?,
        &["alg", "key", "value"],
        &format!("{label} signature"),
    )?;
    if signature["alg"] != "ed25519" {
        return Err(invalid(format!("{label} signature algorithm is invalid")));
    }
    let mut unsigned = document.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    verify_key_signature(
        key,
        &canonical(&unsigned, label)?,
        &signature["value"],
        label,
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct OperationRef {
    #[serde(rename = "aithos-operation-core")]
    pub profile: String,
    pub occurrence: String,
    pub commitment: String,
}

impl OperationRef {
    fn validate(&self, label: &str) -> Result<()> {
        if self.profile != OPERATION_PROFILE {
            return Err(invalid(format!("{label} has an unknown profile")));
        }
        if !self.occurrence.starts_with("op_") || self.occurrence.len() != 29 {
            return Err(invalid(format!("{label} occurrence is invalid")));
        }
        if !is_prefixed_digest(&self.commitment) {
            return Err(invalid(format!("{label} commitment is invalid")));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierRef {
    #[serde(flatten)]
    profile: BTreeMap<String, String>,
    digest: String,
}

impl CarrierRef {
    fn parse(value: &Value, profile_key: &str, profile: &str, label: &str) -> Result<Self> {
        let object = exact_object(value, &[profile_key, "digest"], label)?;
        if object[profile_key].as_str() != Some(profile) {
            return Err(invalid(format!("{label} has an unknown profile")));
        }
        let digest = object["digest"]
            .as_str()
            .filter(|digest| is_prefixed_digest(digest))
            .ok_or_else(|| invalid(format!("{label} digest is invalid")))?;
        Ok(Self {
            profile: BTreeMap::from([(profile_key.to_owned(), profile.to_owned())]),
            digest: digest.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateValue {
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_commitment: Option<String>,
}

impl StateValue {
    fn absent() -> Self {
        Self {
            state: "absent".into(),
            byte_commitment: None,
        }
    }

    fn present(bytes: &[u8]) -> Self {
        Self {
            state: "present".into(),
            byte_commitment: Some(commitment(STATE_BYTES_DOMAIN, bytes)),
        }
    }

    fn validate(&self, label: &str) -> Result<()> {
        match (self.state.as_str(), self.byte_commitment.as_deref()) {
            ("absent", None) => Ok(()),
            ("present", Some(digest)) if is_prefixed_digest(digest) => Ok(()),
            _ => Err(invalid(format!("{label} state is invalid"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateChange {
    pub key_commitment: String,
    pub before: StateValue,
    pub after: StateValue,
    pub operation_ref: OperationRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Changeset {
    #[serde(rename = "aithos-changeset-core")]
    pub profile: String,
    pub height: u64,
    pub predecessors: Vec<String>,
    pub operations: Vec<OperationRef>,
    pub changes: Vec<StateChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum EvidenceItem {
    Authorship { document: Value },
    Session { certificate: Value, proof: Value },
    Receipt { document: Value },
    Catalog { catalog: Value, approval: Value },
    Presentation { document: Value },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSet {
    #[serde(rename = "aithos-evidence-core")]
    pub profile: String,
    pub items: Vec<EvidenceItem>,
    pub delegated_counts: Value,
}

/// One normal edition has exactly one actor. A grantee actor carries one
/// complete authority chain; partial or alternate chains are not representable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "actor", rename_all = "lowercase")]
pub enum K1cActor {
    Owner {
        key: String,
    },
    Grantee {
        key: String,
        authority_chain: Vec<Value>,
    },
}

impl K1cActor {
    fn key(&self) -> &str {
        match self {
            Self::Owner { key } | Self::Grantee { key, .. } => key,
        }
    }

    fn authority_chain(&self) -> &[Value] {
        match self {
            Self::Owner { .. } => &[],
            Self::Grantee {
                authority_chain, ..
            } => authority_chain,
        }
    }

    #[must_use]
    pub fn public_key(&self) -> &str {
        self.key()
    }

    #[must_use]
    pub fn authority_references(&self) -> &[Value] {
        self.authority_chain()
    }
}

/// Public replay material supplied by Bundle after Store/layout verification.
/// It deliberately contains bytes and public proofs, never a signing/opening
/// capability or private key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct K1cVerificationContext {
    pub subject: String,
    pub actor: K1cActor,
    pub height: u64,
    pub predecessors: Vec<Value>,
    pub parent_store: BTreeMap<String, Vec<u8>>,
    pub candidate_store: BTreeMap<String, Vec<u8>>,
    pub change_causes: BTreeMap<String, Value>,
    pub contained_operations: Vec<Value>,
    pub operation_projections: Vec<Value>,
    pub operation_facts: Vec<Value>,
    pub authority_documents: Vec<Value>,
    pub publication_projection: Value,
    pub publication_facts: Value,
    pub publication_ref: Value,
    pub publication_at: String,
    pub required_receipts: Vec<Value>,
    pub delegated_counts: Value,
    pub gamma_source_head: String,
    pub gamma_request_digest: String,
    pub gamma_result: Vec<Value>,
    pub content_key: String,
    pub receipt_key: String,
}

/// Carrier bytes and the already form-checked manifest links. Bundle is the
/// sole constructor at the Store boundary; Core rechecks every semantic link.
#[derive(Debug, Clone)]
pub struct K1cCarrierEnvelope {
    pub changeset: Value,
    pub evidence: Value,
    pub operation_ref: Value,
    pub changeset_ref: Value,
    pub evidence_ref: Value,
    pub files: BTreeMap<String, String>,
    pub sidecars: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedK1cCarriers {
    changeset: Changeset,
    evidence_count: usize,
}

impl VerifiedK1cCarriers {
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.changeset.height
    }

    #[must_use]
    pub fn change_count(&self) -> usize {
        self.changeset.changes.len()
    }

    #[must_use]
    pub const fn evidence_count(&self) -> usize {
        self.evidence_count
    }

    #[must_use]
    pub const fn changeset(&self) -> &Changeset {
        &self.changeset
    }
}

fn operation_ref(value: &Value, label: &str) -> Result<OperationRef> {
    let reference: OperationRef = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("{label} form is invalid: {error}")))?;
    reference.validate(label)?;
    Ok(reference)
}

fn state_for(bytes: Option<&Vec<u8>>) -> StateValue {
    bytes.map_or_else(StateValue::absent, |bytes| StateValue::present(bytes))
}

/// Derive the closed changeset from the two Store states. Caller-supplied
/// change rows are never an input.
pub fn derive_changeset(context: &K1cVerificationContext) -> Result<Changeset> {
    if context
        .predecessors
        .iter()
        .any(|value| !value.as_str().is_some_and(is_prefixed_digest))
    {
        return Err(invalid("publication predecessors are invalid"));
    }

    let operations = context
        .contained_operations
        .iter()
        .map(|value| operation_ref(value, "contained operation_ref"))
        .collect::<Result<Vec<_>>>()?;
    let mut unique_operations = BTreeSet::new();
    if operations
        .iter()
        .any(|reference| !unique_operations.insert(reference.clone()))
    {
        return Err(invalid("contained operations contain a duplicate"));
    }

    let all_keys = context
        .parent_store
        .keys()
        .chain(context.candidate_store.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for key in all_keys {
        let before = context.parent_store.get(&key);
        let after = context.candidate_store.get(&key);
        if before == after {
            continue;
        }
        let cause = context.change_causes.get(&key).ok_or_else(|| {
            invalid(format!(
                "changed Store object has no operation cause: {key}"
            ))
        })?;
        let cause = operation_ref(cause, "change operation_ref")?;
        if !unique_operations.contains(&cause) {
            return Err(invalid(format!(
                "changed Store object cites an uncontained operation: {key}"
            )));
        }
        changes.push(StateChange {
            key_commitment: commitment(STATE_KEY_DOMAIN, key.as_bytes()),
            before: state_for(before),
            after: state_for(after),
            operation_ref: cause,
        });
    }
    if context
        .change_causes
        .keys()
        .any(|key| context.parent_store.get(key) == context.candidate_store.get(key))
    {
        return Err(invalid(
            "change cause names an unchanged or absent Store object",
        ));
    }
    changes.sort_by(|left, right| {
        left.key_commitment
            .cmp(&right.key_commitment)
            .then_with(|| {
                left.operation_ref
                    .occurrence
                    .cmp(&right.operation_ref.occurrence)
            })
    });

    Ok(Changeset {
        profile: CHANGESET_PROFILE.into(),
        height: context.height,
        predecessors: context
            .predecessors
            .iter()
            .map(|value| value.as_str().expect("validated predecessor").to_owned())
            .collect(),
        operations,
        changes,
    })
}

fn validate_changeset(candidate: &Value, context: &K1cVerificationContext) -> Result<Changeset> {
    let changeset: Changeset = serde_json::from_value(candidate.clone())
        .map_err(|error| invalid(format!("changeset form is invalid: {error}")))?;
    if changeset.profile != CHANGESET_PROFILE {
        return Err(invalid("changeset has an unknown profile"));
    }
    for change in &changeset.changes {
        if !is_prefixed_digest(&change.key_commitment) {
            return Err(invalid("changeset key commitment is invalid"));
        }
        change.before.validate("changeset before")?;
        change.after.validate("changeset after")?;
        change.operation_ref.validate("changeset operation_ref")?;
        if change.before == change.after {
            return Err(invalid("changeset contains a no-effect row"));
        }
    }
    if changeset
        .changes
        .windows(2)
        .any(|pair| pair[0].key_commitment >= pair[1].key_commitment)
    {
        return Err(invalid(
            "changeset changes are not strictly sorted by key commitment",
        ));
    }
    let derived = derive_changeset(context)?;
    if changeset != derived {
        return Err(invalid(
            "changeset is not the complete derivation of parent and candidate state",
        ));
    }
    Ok(changeset)
}

struct OperationInventory<'a> {
    facts_by_ref: BTreeMap<OperationRef, &'a Value>,
    projection_by_ref: BTreeMap<OperationRef, &'a Value>,
    authorship: BTreeSet<OperationRef>,
    sessions: BTreeSet<OperationRef>,
    catalogs: BTreeSet<String>,
    presentations: BTreeSet<OperationRef>,
}

fn validate_authority(authority: &Value, actor: &K1cActor) -> Result<()> {
    match actor {
        K1cActor::Owner { .. } => {
            let authority = exact_object(authority, &["actor"], "owner authority")?;
            if authority["actor"] != "owner" {
                return Err(invalid("normal edition mixes actors"));
            }
        }
        K1cActor::Grantee {
            key,
            authority_chain,
        } => {
            if authority_chain.is_empty() {
                return Err(invalid("grantee edition has no authority chain"));
            }
            let authority = authority
                .as_object()
                .ok_or_else(|| invalid("grantee authority is not an object"))?;
            let allowed = ["actor", "key", "authorized_by", "authorized_via", "session"];
            if authority.len() < 4
                || authority.len() > 5
                || authority
                    .keys()
                    .any(|member| !allowed.contains(&member.as_str()))
                || ["actor", "key", "authorized_by", "authorized_via"]
                    .iter()
                    .any(|member| !authority.contains_key(*member))
            {
                return Err(invalid("grantee authority has a non-exact member set"));
            }
            let leaf = authority_chain
                .last()
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("authority chain leaf is invalid"))?;
            if authority["actor"] != "grantee"
                || authority["key"].as_str() != Some(key)
                || authority["authorized_by"] != leaf["id"]
                || authority["authorized_via"] != Value::Array(authority_chain.clone())
            {
                return Err(invalid("normal edition actor or authority chain differs"));
            }
        }
    }
    Ok(())
}

fn validate_authority_documents(context: &K1cVerificationContext) -> Result<()> {
    let references = context.actor.authority_chain();
    if references.is_empty() {
        if !context.authority_documents.is_empty() {
            return Err(invalid(
                "owner edition carries delegated authority documents",
            ));
        }
        return Ok(());
    }
    if references.len() != context.authority_documents.len() {
        return Err(invalid("authority certificate chain is incomplete"));
    }
    let mut parent_id: Option<String> = None;
    let mut parent_key: Option<String> = None;
    for (reference, document) in references.iter().zip(&context.authority_documents) {
        let reference = exact_object(
            reference,
            &["id", "certificate_digest"],
            "authority reference",
        )?;
        let document_object = document
            .as_object()
            .ok_or_else(|| invalid("authority certificate is not an object"))?;
        let id = document_object["id"]
            .as_str()
            .ok_or_else(|| invalid("authority certificate id is invalid"))?;
        if reference["id"].as_str() != Some(id)
            || reference["certificate_digest"].as_str()
                != Some(&sha256_prefixed(&canonical(
                    document,
                    "authority certificate",
                )?))
            || document_object["subject"].as_str() != Some(&context.subject)
        {
            return Err(invalid("authority certificate/reference mismatch"));
        }
        match (&parent_id, document_object.get("parent")) {
            (None, Some(Value::Null)) => {}
            (Some(expected), Some(Value::String(parent))) if parent == expected => {}
            _ => return Err(invalid("authority certificate chain is not contiguous")),
        }
        let signature = exact_object(
            document_object
                .get("signature")
                .ok_or_else(|| invalid("authority certificate signature is missing"))?,
            &["alg", "key", "value"],
            "authority certificate signature",
        )?;
        let signer = match &parent_key {
            Some(key) => key.as_str(),
            None => context
                .subject
                .strip_prefix("did:aithos:")
                .ok_or_else(|| invalid("authority subject is not an Aithos DID"))?,
        };
        let expected_label = parent_key.as_deref().unwrap_or("#root");
        if signature["alg"] != "ed25519" || signature["key"].as_str() != Some(expected_label) {
            return Err(invalid("authority certificate signer label mismatch"));
        }
        verify_blank_value_signature(document, signer, "authority certificate")?;
        parent_id = Some(id.to_owned());
        parent_key = Some(
            document_object["grantee"]["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("authority certificate grantee key is invalid"))?
                .to_owned(),
        );
    }
    if parent_key.as_deref() != Some(context.actor.key()) {
        return Err(invalid("authority chain leaf differs from edition actor"));
    }
    Ok(())
}

/// Enforce the normal-edition v1 invariant independently of any manifest:
/// every contained operation has the same actor and, for a grantee, the same
/// one complete mandate chain.
pub fn verify_normal_edition_actor(actor: &K1cActor, authorities: &[Value]) -> Result<()> {
    if authorities.is_empty() {
        return Err(invalid("normal edition has no contained actor"));
    }
    wire::multibase_to_ed25519_pub(actor.key())
        .map_err(|error| invalid(format!("edition actor key is invalid: {error}")))?;
    for authority in authorities {
        validate_authority(authority, actor)?;
    }
    Ok(())
}

fn validate_operation_inventory(
    context: &K1cVerificationContext,
) -> Result<OperationInventory<'_>> {
    validate_authority_documents(context)?;
    if context.contained_operations.len() != context.operation_projections.len()
        || context.contained_operations.len() != context.operation_facts.len()
    {
        return Err(invalid("operation replay inventory is incomplete"));
    }
    let mut inventory = OperationInventory {
        facts_by_ref: BTreeMap::new(),
        projection_by_ref: BTreeMap::new(),
        authorship: BTreeSet::new(),
        sessions: BTreeSet::new(),
        catalogs: BTreeSet::new(),
        presentations: BTreeSet::new(),
    };
    let authorities = context
        .operation_projections
        .iter()
        .map(|projection| {
            projection
                .as_object()
                .and_then(|projection| projection.get("authority"))
                .cloned()
                .ok_or_else(|| invalid("operation projection has no authority"))
        })
        .collect::<Result<Vec<_>>>()?;
    verify_normal_edition_actor(&context.actor, &authorities)?;
    for ((reference, projection), facts) in context
        .contained_operations
        .iter()
        .zip(&context.operation_projections)
        .zip(&context.operation_facts)
    {
        let reference = operation_ref(reference, "contained operation_ref")?;
        let projection_object = projection
            .as_object()
            .ok_or_else(|| invalid("operation projection is not an object"))?;
        if projection_object["aithos-operation-core"] != OPERATION_PROFILE
            || projection_object["occurrence"].as_str() != Some(&reference.occurrence)
            || projection_object["subject"].as_str() != Some(&context.subject)
        {
            return Err(invalid("operation projection identity mismatch"));
        }
        let expected_commitment = commitment(
            OPERATION_DOMAIN,
            &canonical(projection, "operation projection")?,
        );
        if reference.commitment != expected_commitment {
            return Err(invalid("operation_ref does not select its projection"));
        }
        let facts_object = facts
            .as_object()
            .ok_or_else(|| invalid("operation facts are not an object"))?;
        if facts_object["aithos-operation-facts-core"] != OPERATION_PROFILE {
            return Err(invalid("operation facts have an unknown profile"));
        }
        let facts_digest = commitment(
            OPERATION_FACTS_DOMAIN,
            &canonical(facts, "operation facts")?,
        );
        let facts_ref = projection_object
            .get("operation")
            .and_then(Value::as_object)
            .and_then(|operation| operation.get("facts_ref"))
            .ok_or_else(|| invalid("operation projection has no facts_ref"))?;
        let facts_ref = exact_object(
            facts_ref,
            &["aithos-operation-facts-core", "digest"],
            "operation facts_ref",
        )?;
        if facts_ref["aithos-operation-facts-core"] != OPERATION_PROFILE
            || facts_ref["digest"].as_str() != Some(&facts_digest)
        {
            return Err(invalid("operation projection selects different facts"));
        }
        let kind = facts_object["kind"]
            .as_str()
            .ok_or_else(|| invalid("operation facts kind is missing"))?;
        let body = facts_object["facts"]
            .as_object()
            .ok_or_else(|| invalid("operation facts body is invalid"))?;
        match kind {
            "mutation" if body["zone"] == "public" => {
                inventory.authorship.insert(reference.clone());
            }
            "action" => {
                let connector = body["connector"]
                    .as_str()
                    .ok_or_else(|| invalid("action facts connector is missing"))?;
                inventory.catalogs.insert(connector.to_owned());
            }
            "read" if body["domain"] == "gamma" => {
                inventory.presentations.insert(reference.clone());
            }
            _ => {}
        }
        if projection_object["authority"]
            .as_object()
            .is_some_and(|authority| authority.contains_key("session"))
        {
            inventory.sessions.insert(reference.clone());
        }
        if inventory
            .facts_by_ref
            .insert(reference.clone(), facts)
            .is_some()
            || inventory
                .projection_by_ref
                .insert(reference, projection)
                .is_some()
        {
            return Err(invalid("operation replay inventory contains a duplicate"));
        }
    }
    Ok(inventory)
}

fn validate_publication(
    context: &K1cVerificationContext,
    changeset_ref: &Value,
) -> Result<OperationRef> {
    let reference = operation_ref(&context.publication_ref, "publication operation_ref")?;
    let projection = context
        .publication_projection
        .as_object()
        .ok_or_else(|| invalid("publication projection is not an object"))?;
    if projection["aithos-operation-core"] != OPERATION_PROFILE
        || projection["occurrence"].as_str() != Some(&reference.occurrence)
        || projection["subject"].as_str() != Some(&context.subject)
        || projection["at"].as_str() != Some(&context.publication_at)
        || projection["history_heads"] != Value::Array(context.predecessors.clone())
    {
        return Err(invalid("publication projection identity mismatch"));
    }
    validate_authority(&projection["authority"], &context.actor)?;
    if commitment(
        OPERATION_DOMAIN,
        &canonical(&context.publication_projection, "publication projection")?,
    ) != reference.commitment
    {
        return Err(invalid(
            "publication operation_ref does not select its projection",
        ));
    }
    let facts = exact_object(
        &context.publication_facts,
        &["aithos-operation-facts-core", "kind", "facts"],
        "publication facts",
    )?;
    if facts["aithos-operation-facts-core"] != OPERATION_PROFILE || facts["kind"] != "publication" {
        return Err(invalid("publication facts profile or kind is invalid"));
    }
    let body_value = &facts["facts"];
    let body = body_value
        .as_object()
        .ok_or_else(|| invalid("publication facts body is invalid"))?;
    let common_members = [
        "mode",
        "height",
        "predecessors",
        "changeset_ref",
        "contained_operations",
    ];
    let mode = body["mode"]
        .as_str()
        .ok_or_else(|| invalid("publication mode is invalid"))?;
    match mode {
        "normal" => {
            exact_object(body_value, &common_members, "normal publication facts")?;
            match (context.height, context.predecessors.len()) {
                (1, 0) => {}
                (height, 1) if height > 1 => {}
                _ => {
                    return Err(invalid(
                        "normal publication predecessor cardinality is invalid",
                    ));
                }
            }
        }
        "merge" => {
            exact_object(body_value, &common_members, "merge publication facts")?;
            validate_two_predecessors(context)?;
        }
        "resolution" => {
            let mut resolution_members = common_members.to_vec();
            resolution_members.push("winner");
            exact_object(
                body_value,
                &resolution_members,
                "resolution publication facts",
            )?;
            let predecessors = validate_two_predecessors(context)?;
            if !body["winner"]
                .as_str()
                .is_some_and(|winner| predecessors.contains(&winner))
            {
                return Err(invalid(
                    "resolution winner is not one of its two predecessors",
                ));
            }
        }
        _ => return Err(invalid("publication facts mode is unknown")),
    }
    if body["height"].as_u64() != Some(context.height)
        || body["predecessors"] != Value::Array(context.predecessors.clone())
        || body["changeset_ref"] != *changeset_ref
        || body["contained_operations"] != Value::Array(context.contained_operations.clone())
    {
        return Err(invalid(
            "publication facts differ from the closed changeset or contained operations",
        ));
    }
    let expected_facts_ref = commitment(
        OPERATION_FACTS_DOMAIN,
        &canonical(&context.publication_facts, "publication facts")?,
    );
    let projected_facts_ref = projection["operation"]
        .as_object()
        .and_then(|operation| operation.get("facts_ref"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("publication projection facts_ref is invalid"))?;
    if projection["operation"]["kind"] != "publication"
        || projected_facts_ref["aithos-operation-facts-core"] != OPERATION_PROFILE
        || projected_facts_ref["digest"].as_str() != Some(&expected_facts_ref)
    {
        return Err(invalid("publication projection selects different facts"));
    }
    Ok(reference)
}

fn validate_two_predecessors(context: &K1cVerificationContext) -> Result<BTreeSet<&str>> {
    if context.height < 3 || context.predecessors.len() != 2 {
        return Err(invalid(
            "merge or resolution requires two predecessors at height three or later",
        ));
    }
    let predecessors = context
        .predecessors
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|digest| is_prefixed_digest(digest))
                .ok_or_else(|| invalid("publication predecessor is invalid"))
        })
        .collect::<Result<Vec<_>>>()?;
    if predecessors[0] >= predecessors[1] {
        return Err(invalid(
            "merge or resolution predecessors are not distinct and sorted",
        ));
    }
    Ok(predecessors.into_iter().collect())
}

fn item_jcs(item: &EvidenceItem) -> Result<Vec<u8>> {
    let value =
        serde_json::to_value(item).map_err(|error| invalid(format!("evidence item: {error}")))?;
    canonical(&value, "evidence item")
}

fn expected_authorized_via(actor: &K1cActor) -> Value {
    Value::Array(actor.authority_chain().to_vec())
}

fn verify_authorship(
    document: &Value,
    context: &K1cVerificationContext,
    inventory: &OperationInventory<'_>,
) -> Result<OperationRef> {
    let document = exact_object(
        document,
        &[
            "aithos-authorship-core",
            "subject",
            "zone",
            "sid",
            "content_hash",
            "operation_ref",
            "edition",
            "authorized_via",
            "key",
            "sig",
        ],
        "authorship document",
    )?;
    if document["aithos-authorship-core"] != AUTHORSHIP_PROFILE
        || document["subject"].as_str() != Some(&context.subject)
        || document["zone"] != "public"
        || document["key"].as_str() != Some(context.actor.key())
        || document["authorized_via"] != expected_authorized_via(&context.actor)
    {
        return Err(invalid("authorship identity or authority mismatch"));
    }
    let reference = operation_ref(&document["operation_ref"], "authorship operation_ref")?;
    if !inventory.authorship.contains(&reference) {
        return Err(invalid(
            "authorship does not select a public mutation operation",
        ));
    }
    let facts = inventory.facts_by_ref[&reference]
        .as_object()
        .and_then(|facts| facts["facts"].as_object())
        .ok_or_else(|| invalid("public mutation facts are invalid"))?;
    let sid = document["sid"]
        .as_str()
        .ok_or_else(|| invalid("authorship SID is invalid"))?;
    if facts["sid"].as_str() != Some(sid) {
        return Err(invalid("authorship SID differs from mutation facts"));
    }
    let path = format!("public/sections/{sid}.md");
    let body = context
        .candidate_store
        .get(&path)
        .ok_or_else(|| invalid("authorship body is absent from candidate state"))?;
    if document["content_hash"].as_str() != Some(&sha256_prefixed(body)) {
        return Err(invalid("authorship content hash differs from stored bytes"));
    }
    let edition = exact_object(
        &document["edition"],
        &["height", "predecessors"],
        "authorship edition",
    )?;
    if edition["height"].as_u64() != Some(context.height)
        || edition["predecessors"] != Value::Array(context.predecessors.clone())
    {
        return Err(invalid("authorship edition mismatch"));
    }
    verify_omitted_signature(
        &Value::Object(document.clone()),
        context.actor.key(),
        "authorship",
    )?;
    Ok(reference)
}

fn verify_session_item(
    certificate: &Value,
    proof: &Value,
    context: &K1cVerificationContext,
    inventory: &OperationInventory<'_>,
) -> Result<OperationRef> {
    let certificate_object = exact_object(
        certificate,
        &[
            "aithos-session-core",
            "subject",
            "mandate_id",
            "key",
            "not_before",
            "not_after",
            "signature",
        ],
        "session certificate",
    )?;
    if certificate_object["aithos-session-core"] != OPERATION_PROFILE
        || certificate_object["subject"].as_str() != Some(&context.subject)
    {
        return Err(invalid("session certificate profile or subject mismatch"));
    }
    crate::gamma::ts_epoch(
        certificate_object["not_before"]
            .as_str()
            .ok_or_else(|| invalid("session not_before is invalid"))?,
    )
    .map_err(|error| invalid(format!("session not_before is invalid: {error}")))?;
    crate::gamma::ts_epoch(
        certificate_object["not_after"]
            .as_str()
            .ok_or_else(|| invalid("session not_after is invalid"))?,
    )
    .map_err(|error| invalid(format!("session not_after is invalid: {error}")))?;
    let signature = exact_object(
        &certificate_object["signature"],
        &["alg", "key", "value"],
        "session certificate signature",
    )?;
    if signature["key"].as_str() != Some(context.actor.key()) {
        return Err(invalid("session certificate signer differs from actor"));
    }
    let leaf_id = context
        .actor
        .authority_chain()
        .last()
        .and_then(Value::as_object)
        .and_then(|leaf| leaf["id"].as_str())
        .ok_or_else(|| invalid("session evidence requires a grantee chain"))?;
    if certificate_object["mandate_id"].as_str() != Some(leaf_id) {
        return Err(invalid("session certificate selects another mandate"));
    }
    verify_blank_value_signature(certificate, context.actor.key(), "session certificate")?;

    let proof = exact_object(
        proof,
        &["aithos-session-proof-core", "operation_ref", "key", "sig"],
        "session proof",
    )?;
    if proof["aithos-session-proof-core"] != OPERATION_PROFILE {
        return Err(invalid("session proof has an unknown profile"));
    }
    let reference = operation_ref(&proof["operation_ref"], "session operation_ref")?;
    if !inventory.sessions.contains(&reference) {
        return Err(invalid(
            "session proof does not select a session-bound operation",
        ));
    }
    let projection = inventory.projection_by_ref[&reference]
        .as_object()
        .ok_or_else(|| invalid("session projection is invalid"))?;
    let session = projection["authority"]
        .as_object()
        .and_then(|authority| authority.get("session"))
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("session projection has no session fact"))?;
    let session_key = certificate_object["key"]
        .as_str()
        .ok_or_else(|| invalid("session certificate key is invalid"))?;
    if proof["key"].as_str() != Some(session_key) || session["key"].as_str() != Some(session_key) {
        return Err(invalid("session key correlation mismatch"));
    }
    let certificate_digest = sha256_prefixed(&canonical(certificate, "session certificate")?);
    if session["certificate_digest"].as_str() != Some(&certificate_digest) {
        return Err(invalid("session certificate digest mismatch"));
    }
    verify_omitted_signature(&Value::Object(proof.clone()), session_key, "session proof")?;
    Ok(reference)
}

fn verify_receipt_item(document: &Value, context: &K1cVerificationContext) -> Result<OperationRef> {
    let document = exact_object(
        document,
        &[
            "v",
            "family",
            "obligation",
            "operation_ref",
            "verdict",
            "at",
            "sig",
        ],
        "receipt",
    )?;
    if document["v"].as_u64() != Some(2)
        || document["family"] != "obligation"
        || document["verdict"] != "approve"
    {
        return Err(invalid("receipt profile or verdict is invalid"));
    }
    crate::gamma::ts_epoch(
        document["at"]
            .as_str()
            .ok_or_else(|| invalid("receipt timestamp is invalid"))?,
    )
    .map_err(|error| invalid(format!("receipt timestamp is invalid: {error}")))?;
    let reference = operation_ref(&document["operation_ref"], "receipt operation_ref")?;
    if !context
        .required_receipts
        .iter()
        .any(|required| required == &document["operation_ref"])
    {
        return Err(invalid("receipt does not discharge a required operation"));
    }
    verify_omitted_signature(
        &Value::Object(document.clone()),
        &context.receipt_key,
        "receipt",
    )?;
    Ok(reference)
}

fn verify_catalog_item(
    catalog: &Value,
    approval: &Value,
    context: &K1cVerificationContext,
    inventory: &OperationInventory<'_>,
) -> Result<String> {
    let catalog_object = exact_object(
        catalog,
        &[
            "aithos-connector-catalog-core",
            "connector",
            "catalog_version",
            "actions",
            "signature",
        ],
        "connector catalog",
    )?;
    if catalog_object["aithos-connector-catalog-core"] != OPERATION_PROFILE {
        return Err(invalid("connector catalog has an unknown profile"));
    }
    let connector = catalog_object["connector"]
        .as_str()
        .ok_or_else(|| invalid("connector catalog connector is invalid"))?;
    if !inventory.catalogs.contains(connector) {
        return Err(invalid("connector catalog is unrelated to this edition"));
    }
    let actions = catalog_object["actions"]
        .as_array()
        .filter(|actions| !actions.is_empty())
        .ok_or_else(|| invalid("connector catalog actions are invalid"))?;
    let mut action_names = Vec::new();
    for action in actions {
        let action = exact_object(action, &["name", "class"], "catalog action")?;
        let name = action["name"]
            .as_str()
            .ok_or_else(|| invalid("catalog action name is invalid"))?;
        if !matches!(action["class"].as_str(), Some("read" | "act" | "binding")) {
            return Err(invalid("catalog action class is invalid"));
        }
        action_names.push(name);
    }
    if action_names.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid(
            "connector catalog actions are not sorted and unique",
        ));
    }
    let catalog_signature = exact_object(
        &catalog_object["signature"],
        &["alg", "key", "value"],
        "catalog signature",
    )?;
    let catalog_key = catalog_signature["key"]
        .as_str()
        .ok_or_else(|| invalid("catalog signer is invalid"))?;
    verify_blank_value_signature(catalog, catalog_key, "connector catalog")?;
    let catalog_digest = sha256_prefixed(&canonical(catalog, "connector catalog")?);

    let approval_object = exact_object(
        approval,
        &[
            "aithos-connector-catalog-approval-core",
            "subject",
            "connector",
            "catalog_version",
            "catalog_digest",
            "approved_at",
            "signature",
        ],
        "catalog approval",
    )?;
    if approval_object["aithos-connector-catalog-approval-core"] != OPERATION_PROFILE
        || approval_object["subject"].as_str() != Some(&context.subject)
        || approval_object["connector"].as_str() != Some(connector)
        || approval_object["catalog_version"] != catalog_object["catalog_version"]
        || approval_object["catalog_digest"].as_str() != Some(&catalog_digest)
    {
        return Err(invalid(
            "catalog approval does not select the signed catalog",
        ));
    }
    crate::gamma::ts_epoch(
        approval_object["approved_at"]
            .as_str()
            .ok_or_else(|| invalid("catalog approval time is invalid"))?,
    )
    .map_err(|error| invalid(format!("catalog approval time is invalid: {error}")))?;
    let approval_signature = exact_object(
        &approval_object["signature"],
        &["alg", "key", "value"],
        "catalog approval signature",
    )?;
    if approval_signature["key"] != "#content" {
        return Err(invalid("catalog approval is not owner-content signed"));
    }
    verify_blank_value_signature(approval, &context.content_key, "catalog approval")?;
    let approval_digest = sha256_prefixed(&canonical(approval, "catalog approval")?);

    let mut selected = false;
    for facts in inventory.facts_by_ref.values() {
        let facts = facts
            .as_object()
            .and_then(|facts| facts["facts"].as_object())
            .ok_or_else(|| invalid("action facts are invalid"))?;
        if facts.get("connector").and_then(Value::as_str) != Some(connector) {
            continue;
        }
        let catalog_ref = facts
            .get("catalog_ref")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("action facts catalog_ref is invalid"))?;
        if catalog_ref["catalog_version"] != catalog_object["catalog_version"]
            || catalog_ref["catalog_digest"].as_str() != Some(&catalog_digest)
            || catalog_ref["approval_digest"].as_str() != Some(&approval_digest)
        {
            return Err(invalid("action facts pin different catalog evidence"));
        }
        selected = true;
    }
    if !selected {
        return Err(invalid("catalog evidence is unused"));
    }
    Ok(connector.to_owned())
}

fn verify_presentation_item(
    document: &Value,
    context: &K1cVerificationContext,
    inventory: &OperationInventory<'_>,
) -> Result<OperationRef> {
    let document = exact_object(
        document,
        &[
            "aithos-gamma-presentation-core",
            "subject",
            "operation_ref",
            "source_head",
            "request_digest",
            "entries",
            "at",
            "key",
            "sig",
        ],
        "Gamma presentation",
    )?;
    if document["aithos-gamma-presentation-core"] != PRESENTATION_PROFILE
        || document["subject"].as_str() != Some(&context.subject)
        || document["key"].as_str() != Some(context.actor.key())
    {
        return Err(invalid("Gamma presentation identity mismatch"));
    }
    let reference = operation_ref(
        &document["operation_ref"],
        "Gamma presentation operation_ref",
    )?;
    if !inventory.presentations.contains(&reference) {
        return Err(invalid(
            "Gamma presentation does not select a read.gamma operation",
        ));
    }
    let facts = inventory.facts_by_ref[&reference]
        .as_object()
        .and_then(|facts| facts["facts"].as_object())
        .ok_or_else(|| invalid("read.gamma facts are invalid"))?;
    if facts["source_head"].as_str() != Some(&context.gamma_source_head)
        || facts["request_digest"].as_str() != Some(&context.gamma_request_digest)
        || document["source_head"].as_str() != Some(&context.gamma_source_head)
        || document["request_digest"].as_str() != Some(&context.gamma_request_digest)
        || document["entries"] != Value::Array(context.gamma_result.clone())
    {
        return Err(invalid(
            "Gamma presentation differs from the re-executed query",
        ));
    }
    let entries = document["entries"]
        .as_array()
        .ok_or_else(|| invalid("Gamma presentation entries are invalid"))?;
    let mut ids = BTreeSet::new();
    if entries.iter().any(|entry| {
        !entry["id"]
            .as_str()
            .is_some_and(|id| ids.insert(id.to_owned()))
    }) {
        return Err(invalid("Gamma presentation contains duplicate entry ids"));
    }
    crate::gamma::ts_epoch(
        document["at"]
            .as_str()
            .ok_or_else(|| invalid("Gamma presentation time is invalid"))?,
    )
    .map_err(|error| invalid(format!("Gamma presentation time is invalid: {error}")))?;
    verify_omitted_signature(
        &Value::Object(document.clone()),
        context.actor.key(),
        "Gamma presentation",
    )?;
    Ok(reference)
}

fn validate_delegated_counts(value: &Value, expected: &Value) -> Result<()> {
    let counts = exact_object(
        value,
        &["aithos-delegated-counts-core", "root"],
        "delegated counts reference",
    )?;
    if counts["aithos-delegated-counts-core"] != DELEGATED_COUNTS_PROFILE
        || !counts["root"].as_str().is_some_and(is_bare_digest)
        || value != expected
    {
        return Err(invalid("delegated counts reference mismatch"));
    }
    Ok(())
}

fn validate_evidence(
    candidate: &Value,
    context: &K1cVerificationContext,
    inventory: &OperationInventory<'_>,
) -> Result<EvidenceSet> {
    let evidence: EvidenceSet = serde_json::from_value(candidate.clone())
        .map_err(|error| invalid(format!("evidence form is invalid: {error}")))?;
    if evidence.profile != EVIDENCE_PROFILE {
        return Err(invalid("evidence has an unknown profile"));
    }
    validate_delegated_counts(&evidence.delegated_counts, &context.delegated_counts)?;
    let item_bytes = evidence
        .items
        .iter()
        .map(item_jcs)
        .collect::<Result<Vec<_>>>()?;
    if item_bytes.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid(
            "evidence items are not strictly sorted and unique by JCS",
        ));
    }

    let mut authorship = BTreeSet::new();
    let mut sessions = BTreeSet::new();
    let mut receipts = BTreeSet::new();
    let mut catalogs = BTreeSet::new();
    let mut presentations = BTreeSet::new();
    for item in &evidence.items {
        match item {
            EvidenceItem::Authorship { document } => {
                if !authorship.insert(verify_authorship(document, context, inventory)?) {
                    return Err(invalid("duplicate authorship evidence"));
                }
            }
            EvidenceItem::Session { certificate, proof } => {
                if !sessions.insert(verify_session_item(certificate, proof, context, inventory)?) {
                    return Err(invalid("duplicate session evidence"));
                }
            }
            EvidenceItem::Receipt { document } => {
                if !receipts.insert(verify_receipt_item(document, context)?) {
                    return Err(invalid("duplicate receipt evidence"));
                }
            }
            EvidenceItem::Catalog { catalog, approval } => {
                if !catalogs.insert(verify_catalog_item(catalog, approval, context, inventory)?) {
                    return Err(invalid("duplicate catalog evidence"));
                }
            }
            EvidenceItem::Presentation { document } => {
                if !presentations.insert(verify_presentation_item(document, context, inventory)?) {
                    return Err(invalid("duplicate Gamma presentation evidence"));
                }
            }
        }
    }
    let required_receipts = context
        .required_receipts
        .iter()
        .map(|value| operation_ref(value, "required receipt operation_ref"))
        .collect::<Result<BTreeSet<_>>>()?;
    if authorship != inventory.authorship
        || sessions != inventory.sessions
        || receipts != required_receipts
        || catalogs != inventory.catalogs
        || presentations != inventory.presentations
    {
        return Err(invalid(
            "evidence set omits a required proof or contains an unrelated proof",
        ));
    }
    Ok(evidence)
}

struct CarrierLinkSpec<'a> {
    profile_key: &'a str,
    profile: &'a str,
    domain: &'a str,
    directory: &'a str,
}

fn verify_carrier_link(
    document: &Value,
    reference: &Value,
    spec: CarrierLinkSpec<'_>,
    files: &BTreeMap<String, String>,
    sidecars: &BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    let reference = CarrierRef::parse(
        reference,
        spec.profile_key,
        spec.profile,
        "carrier reference",
    )?;
    let bytes = canonical(document, "carrier document")?;
    let expected_digest = commitment(spec.domain, &bytes);
    if reference.digest != expected_digest {
        return Err(invalid("carrier reference does not select its document"));
    }
    let suffix = expected_digest
        .strip_prefix("sha256:")
        .expect("generated digest is prefixed");
    let path = format!("{}/{suffix}.json", spec.directory);
    if sidecars.get(&path).map(Vec::as_slice) != Some(bytes.as_slice()) {
        return Err(invalid("carrier sidecar path or canonical bytes mismatch"));
    }
    let file_hash = sha256_hex(&bytes);
    if files.get(&path) != Some(&file_hash) {
        return Err(invalid("manifest files pin differs from carrier bytes"));
    }
    Ok(())
}

/// Validate K1-C through one pure Core verdict.
pub fn verify_k1c_carriers(
    envelope: &K1cCarrierEnvelope,
    context: &K1cVerificationContext,
) -> Result<VerifiedK1cCarriers> {
    let changeset = validate_changeset(&envelope.changeset, context)?;
    let inventory = validate_operation_inventory(context)?;
    let evidence = validate_evidence(&envelope.evidence, context, &inventory)?;

    let publication = validate_publication(context, &envelope.changeset_ref)?;
    let carried_publication = operation_ref(&envelope.operation_ref, "manifest operation_ref")?;
    if carried_publication != publication {
        return Err(invalid(
            "manifest operation_ref does not select the publication operation",
        ));
    }
    if envelope.sidecars.len() != 2 {
        return Err(invalid(
            "candidate has an incomplete or extraneous carrier sidecar",
        ));
    }
    verify_carrier_link(
        &envelope.changeset,
        &envelope.changeset_ref,
        CarrierLinkSpec {
            profile_key: "aithos-changeset-core",
            profile: CHANGESET_PROFILE,
            domain: CHANGESET_DOMAIN,
            directory: "changesets",
        },
        &envelope.files,
        &envelope.sidecars,
    )?;
    verify_carrier_link(
        &envelope.evidence,
        &envelope.evidence_ref,
        CarrierLinkSpec {
            profile_key: "aithos-evidence-core",
            profile: EVIDENCE_PROFILE,
            domain: EVIDENCE_DOMAIN,
            directory: "evidence",
        },
        &envelope.files,
        &envelope.sidecars,
    )?;

    Ok(VerifiedK1cCarriers {
        changeset,
        evidence_count: evidence.items.len(),
    })
}

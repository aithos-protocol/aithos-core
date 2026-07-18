//! Pure CAT1 signed connector-catalog, owner approval and pin enforcement.
//!
//! Catalog signatures attest classification data. A distinct owner-content
//! signature approves that exact content address. Neither proof substitutes for
//! the other, and verified catalog classes are not a full authorization verdict.

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::did::DidDocument;
use crate::jcs;
use crate::wire;
use crate::{Error, Result};

const CATALOG_PROFILE_KEY: &str = "aithos-connector-catalog-core";
const CATALOG_PROFILE: &str = "1.0.0-draft.1";
const APPROVAL_PROFILE_KEY: &str = "aithos-connector-catalog-approval-core";
const APPROVAL_PROFILE: &str = "1.0.0-draft.1";
const MANDATE_PROFILE_KEY: &str = "aithos-mandate-core";
const MANDATE_DRAFT3: &str = "1.0.0-draft.3";

fn invalid_catalog(detail: impl Into<String>) -> Error {
    Error::InvalidCatalog(detail.into())
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
    error: fn(String) -> Error,
) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| error(format!("{label} is not an object")))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(error(format!("{label} has a non-exact member set")));
    }
    if object.values().any(Value::is_null) {
        return Err(error(format!("{label} contains null")));
    }
    Ok(object)
}

fn required_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
    error: fn(String) -> Error,
) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(format!("{label} has invalid {key}")))
}

fn is_identifier(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn is_version(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_lower_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_digest(value: &str) -> bool {
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

fn sha256_text(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn signature_block<'a>(
    value: &'a Value,
    expected_key: Option<&str>,
    error: fn(String) -> Error,
) -> Result<&'a Map<String, Value>> {
    let signature = exact_object(value, &["alg", "key", "value"], "signature", error)?;
    if signature["alg"].as_str() != Some("ed25519") {
        return Err(error("signature algorithm is not ed25519".into()));
    }
    if expected_key.is_some_and(|expected| signature["key"].as_str() != Some(expected)) {
        return Err(error("signature key selector mismatch".into()));
    }
    Ok(signature)
}

fn unsigned_document(document: &Value, error: fn(String) -> Error) -> Result<Value> {
    let mut unsigned = document.clone();
    let signature = unsigned
        .get_mut("signature")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("document signature is not an object".into()))?;
    signature.insert("value".into(), Value::String(String::new()));
    Ok(unsigned)
}

fn verify_signature(
    key: &str,
    message: &[u8],
    signature: &Value,
    error: fn(String) -> Error,
) -> Result<()> {
    let public = wire::multibase_to_ed25519_pub(key)
        .map_err(|_| error("invalid Ed25519 multibase key".into()))?;
    let public = VerifyingKey::from_bytes(&public)
        .map_err(|_| error("invalid Ed25519 public key".into()))?;
    let signature = signature
        .as_str()
        .filter(|value| is_lower_hex(value, 64))
        .ok_or_else(|| error("malformed Ed25519 signature".into()))?;
    let signature: [u8; 64] = hex::decode(signature)
        .map_err(|_| error("malformed Ed25519 signature".into()))?
        .try_into()
        .map_err(|_| error("malformed Ed25519 signature".into()))?;
    public
        .verify(message, &Signature::from_bytes(&signature))
        .map_err(|_| error("Ed25519 signature does not verify".into()))
}

fn verified_owner_did(value: &Value) -> Result<DidDocument> {
    let document: DidDocument = serde_json::from_value(value.clone())
        .map_err(|_| invalid_catalog("owner DID document does not parse"))?;
    document
        .verify()
        .map_err(|error| invalid_catalog(format!("owner DID is invalid: {error}")))?;
    Ok(document)
}

#[derive(Debug)]
pub struct VerifiedConnectorCatalog {
    connector: String,
    catalog_version: String,
    digest: String,
    actions: BTreeMap<String, String>,
}

impl VerifiedConnectorCatalog {
    #[must_use]
    pub fn connector(&self) -> &str {
        &self.connector
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Validate the closed signed catalog and its complete-document digest.
pub fn verify_connector_catalog(
    document: &Value,
    claimed_digest: &str,
) -> Result<VerifiedConnectorCatalog> {
    let catalog = exact_object(
        document,
        &[
            CATALOG_PROFILE_KEY,
            "connector",
            "catalog_version",
            "actions",
            "signature",
        ],
        "connector catalog",
        invalid_catalog,
    )?;
    if catalog[CATALOG_PROFILE_KEY].as_str() != Some(CATALOG_PROFILE) {
        return Err(invalid_catalog("unknown connector-catalog profile"));
    }
    let connector = required_text(catalog, "connector", "connector catalog", invalid_catalog)?;
    if !is_identifier(connector) {
        return Err(invalid_catalog("invalid connector id"));
    }
    let catalog_version = required_text(
        catalog,
        "catalog_version",
        "connector catalog",
        invalid_catalog,
    )?;
    if !is_version(catalog_version) {
        return Err(invalid_catalog("invalid catalog version"));
    }
    let rows = catalog["actions"]
        .as_array()
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| invalid_catalog("catalog actions must be a non-empty array"))?;
    let mut actions = BTreeMap::new();
    let mut order = Vec::new();
    for row in rows {
        let row = exact_object(row, &["name", "class"], "catalog action", invalid_catalog)?;
        let name = required_text(row, "name", "catalog action", invalid_catalog)?;
        if !is_identifier(name) {
            return Err(invalid_catalog("invalid catalog action name"));
        }
        let class = required_text(row, "class", "catalog action", invalid_catalog)?;
        if !matches!(class, "read" | "act" | "binding") {
            return Err(invalid_catalog("invalid catalog action class"));
        }
        if actions.insert(name.to_owned(), class.to_owned()).is_some() {
            return Err(invalid_catalog("duplicate catalog action"));
        }
        order.push(name);
    }
    if order.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid_catalog("catalog actions are not sorted"));
    }
    let signature = signature_block(&catalog["signature"], None, invalid_catalog)?;
    let signer = required_text(signature, "key", "catalog signature", invalid_catalog)?;
    let unsigned = unsigned_document(document, invalid_catalog)?;
    verify_signature(
        signer,
        &jcs::canonical_bytes(&unsigned)
            .map_err(|error| invalid_catalog(format!("catalog JCS failed: {error}")))?,
        &signature["value"],
        invalid_catalog,
    )?;
    let digest = sha256_text(
        &jcs::canonical_bytes(document)
            .map_err(|error| invalid_catalog(format!("catalog JCS failed: {error}")))?,
    );
    if claimed_digest != digest {
        return Err(invalid_catalog("catalog digest mismatch"));
    }
    Ok(VerifiedConnectorCatalog {
        connector: connector.to_owned(),
        catalog_version: catalog_version.to_owned(),
        digest,
        actions,
    })
}

#[derive(Debug)]
pub struct VerifiedCatalogApproval {
    subject: String,
    digest: String,
}

impl VerifiedCatalogApproval {
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Validate the distinct owner-content approval for one verified catalog.
pub fn verify_catalog_approval(
    document: &Value,
    claimed_digest: &str,
    catalog: &VerifiedConnectorCatalog,
    owner_did: &Value,
) -> Result<VerifiedCatalogApproval> {
    let approval = exact_object(
        document,
        &[
            APPROVAL_PROFILE_KEY,
            "subject",
            "connector",
            "catalog_version",
            "catalog_digest",
            "approved_at",
            "signature",
        ],
        "catalog approval",
        invalid_catalog,
    )?;
    let owner = verified_owner_did(owner_did)?;
    if approval[APPROVAL_PROFILE_KEY].as_str() != Some(APPROVAL_PROFILE) {
        return Err(invalid_catalog("unknown catalog-approval profile"));
    }
    if approval["subject"].as_str() != Some(owner.id.as_str()) {
        return Err(invalid_catalog("approval subject mismatch"));
    }
    if approval["connector"].as_str() != Some(catalog.connector.as_str()) {
        return Err(invalid_catalog("approval connector mismatch"));
    }
    if approval["catalog_version"].as_str() != Some(catalog.catalog_version.as_str()) {
        return Err(invalid_catalog("approval catalog version mismatch"));
    }
    if approval["catalog_digest"].as_str() != Some(catalog.digest.as_str()) {
        return Err(invalid_catalog("approval catalog digest mismatch"));
    }
    let approved_at = required_text(approval, "approved_at", "catalog approval", invalid_catalog)?;
    crate::gamma::ts_epoch(approved_at)
        .map_err(|_| invalid_catalog("approved_at is not a canonical RFC3339 Z instant"))?;
    let signature = signature_block(&approval["signature"], Some("#content"), invalid_catalog)?;
    let unsigned = unsigned_document(document, invalid_catalog)?;
    verify_signature(
        &owner.keys.content,
        &jcs::canonical_bytes(&unsigned)
            .map_err(|error| invalid_catalog(format!("approval JCS failed: {error}")))?,
        &signature["value"],
        invalid_catalog,
    )?;
    let digest = sha256_text(
        &jcs::canonical_bytes(document)
            .map_err(|error| invalid_catalog(format!("approval JCS failed: {error}")))?,
    );
    if claimed_digest != digest {
        return Err(invalid_catalog("approval digest mismatch"));
    }
    Ok(VerifiedCatalogApproval {
        subject: owner.id,
        digest,
    })
}

fn invalid_mandate(detail: String) -> Error {
    Error::InvalidMandate(detail)
}

fn validate_pin(
    value: &Value,
    catalog: &VerifiedConnectorCatalog,
    approval: &VerifiedCatalogApproval,
) -> Result<()> {
    let pin = exact_object(
        value,
        &[
            "connector",
            "catalog_version",
            "catalog_digest",
            "approval_digest",
        ],
        "catalog pin",
        invalid_mandate,
    )?;
    for field in ["catalog_digest", "approval_digest"] {
        if !pin[field].as_str().is_some_and(is_digest) {
            return Err(Error::InvalidMandate(format!("invalid {field}")));
        }
    }
    if pin["connector"].as_str() != Some(catalog.connector.as_str())
        || pin["catalog_version"].as_str() != Some(catalog.catalog_version.as_str())
        || pin["catalog_digest"].as_str() != Some(catalog.digest.as_str())
        || pin["approval_digest"].as_str() != Some(approval.digest.as_str())
    {
        return Err(Error::InvalidMandate(
            "catalog pin/evidence mismatch".into(),
        ));
    }
    Ok(())
}

fn business_connectors(perimeter: &Value) -> Result<BTreeSet<String>> {
    let perimeter = perimeter
        .as_array()
        .filter(|entries| !entries.is_empty())
        .ok_or_else(|| Error::InvalidMandate("perimeter is not a non-empty array".into()))?;
    let mut connectors = BTreeSet::new();
    for entry in perimeter {
        let entry = entry
            .as_str()
            .ok_or_else(|| Error::InvalidMandate("perimeter entry is not text".into()))?;
        let Some(action) = entry.strip_prefix("act.x.") else {
            continue;
        };
        let Some((connector, action)) = action.split_once('.') else {
            return Err(Error::InvalidMandate(
                "malformed connector perimeter".into(),
            ));
        };
        if action.contains('.')
            || !is_identifier(connector)
            || (action != "*" && !is_identifier(action))
        {
            return Err(Error::InvalidMandate(
                "malformed connector perimeter".into(),
            ));
        }
        if action != "config" {
            connectors.insert(connector.to_owned());
        }
    }
    Ok(connectors)
}

#[derive(Debug)]
pub struct VerifiedCatalogChain {
    depth: usize,
}

impl VerifiedCatalogChain {
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

/// Validate homogeneous draft.3 catalog pins and exact inheritance.
pub fn verify_catalog_chain(
    chain: &Value,
    catalog: &VerifiedConnectorCatalog,
    approval: &VerifiedCatalogApproval,
    owner_did: &Value,
) -> Result<VerifiedCatalogChain> {
    let owner = verified_owner_did(owner_did)
        .map_err(|error| Error::InvalidMandate(format!("invalid catalog owner: {error}")))?;
    if approval.subject != owner.id {
        return Err(Error::InvalidMandate(
            "catalog approval belongs to another owner".into(),
        ));
    }
    let chain = chain
        .as_array()
        .filter(|records| !records.is_empty())
        .ok_or_else(|| Error::InvalidMandate("catalog chain is empty".into()))?;
    let mut parent_id: Option<&str> = None;
    let mut root_pins: Option<Value> = None;
    let mut pinned_connectors = BTreeSet::new();
    for record in chain {
        let object = record
            .as_object()
            .ok_or_else(|| Error::InvalidMandate("catalog mandate is not an object".into()))?;
        let keys = [
            MANDATE_PROFILE_KEY,
            "id",
            "parent",
            "perimeter",
            "constraints",
        ];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(Error::InvalidMandate(
                "catalog mandate has a non-exact member set".into(),
            ));
        }
        if object
            .iter()
            .any(|(key, value)| key != "parent" && value.is_null())
        {
            return Err(Error::InvalidMandate(
                "catalog mandate contains null".into(),
            ));
        }
        if object[MANDATE_PROFILE_KEY].as_str() != Some(MANDATE_DRAFT3) {
            return Err(Error::InvalidMandate(
                "catalog authority requires homogeneous draft.3".into(),
            ));
        }
        let id = object["id"]
            .as_str()
            .filter(|id| is_mandate_id(id))
            .ok_or_else(|| Error::InvalidMandate("invalid catalog mandate id".into()))?;
        let constraints = exact_object(
            &object["constraints"],
            &["catalog_pins"],
            "catalog constraints",
            invalid_mandate,
        )?;
        let pins = constraints["catalog_pins"]
            .as_array()
            .filter(|pins| !pins.is_empty())
            .ok_or_else(|| {
                Error::InvalidMandate("catalog_pins must be a non-empty array".into())
            })?;
        let mut names = Vec::new();
        for pin in pins {
            validate_pin(pin, catalog, approval)?;
            let name = pin["connector"]
                .as_str()
                .ok_or_else(|| Error::InvalidMandate("invalid pin connector".into()))?;
            names.push(name);
        }
        if names.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Error::InvalidMandate(
                "catalog pins are not unique and sorted".into(),
            ));
        }
        let connectors = business_connectors(&object["perimeter"])?;
        match parent_id {
            None => {
                if !object["parent"].is_null() {
                    return Err(Error::InvalidMandate(
                        "root catalog mandate has a parent".into(),
                    ));
                }
                pinned_connectors = names.iter().map(|name| (*name).to_owned()).collect();
                if connectors != pinned_connectors {
                    return Err(Error::InvalidMandate(
                        "initial catalog pin coverage mismatch".into(),
                    ));
                }
                root_pins = Some(constraints["catalog_pins"].clone());
            }
            Some(expected) => {
                if object["parent"].as_str() != Some(expected) {
                    return Err(Error::InvalidMandate(
                        "catalog mandate parent mismatch".into(),
                    ));
                }
                if root_pins.as_ref() != Some(&constraints["catalog_pins"]) {
                    return Err(Error::InvalidMandate(
                        "catalog pins changed through attenuation".into(),
                    ));
                }
                if !connectors.is_subset(&pinned_connectors) {
                    return Err(Error::InvalidMandate(
                        "child uses an unpinned connector".into(),
                    ));
                }
            }
        }
        parent_id = Some(id);
    }
    Ok(VerifiedCatalogChain { depth: chain.len() })
}

fn invalid_facts(detail: String) -> Error {
    Error::InvalidOperationFacts(detail)
}

#[derive(Debug)]
pub struct VerifiedCatalogAction {
    class: String,
}

impl VerifiedCatalogAction {
    #[must_use]
    pub fn class(&self) -> &str {
        &self.class
    }
}

/// Validate one K1.2 action's catalog reference and derive its class.
pub fn verify_catalog_action_facts(
    facts: &Value,
    pin: &Value,
    catalog: &VerifiedConnectorCatalog,
    approval: &VerifiedCatalogApproval,
    owner_did: &Value,
) -> Result<VerifiedCatalogAction> {
    let owner = verified_owner_did(owner_did)
        .map_err(|error| Error::InvalidOperationFacts(format!("invalid catalog owner: {error}")))?;
    if approval.subject != owner.id {
        return Err(Error::InvalidOperationFacts(
            "catalog approval belongs to another owner".into(),
        ));
    }
    validate_pin(pin, catalog, approval)
        .map_err(|error| Error::InvalidOperationFacts(format!("invalid catalog pin: {error}")))?;
    let facts = exact_object(
        facts,
        &[
            "connector",
            "action",
            "catalog_ref",
            "args_hash",
            "budget",
            "purpose",
        ],
        "catalog action facts",
        invalid_facts,
    )?;
    let reference = exact_object(
        &facts["catalog_ref"],
        &["catalog_version", "catalog_digest", "approval_digest"],
        "catalog_ref",
        invalid_facts,
    )?;
    let expected = [
        ("catalog_version", catalog.catalog_version.as_str()),
        ("catalog_digest", catalog.digest.as_str()),
        ("approval_digest", approval.digest.as_str()),
    ];
    if expected
        .iter()
        .any(|(key, value)| reference[*key].as_str() != Some(*value))
    {
        return Err(Error::InvalidOperationFacts(
            "action catalog_ref does not match mandate pin".into(),
        ));
    }
    if facts["connector"].as_str() != Some(catalog.connector.as_str()) {
        return Err(Error::InvalidOperationFacts(
            "action connector does not match mandate pin".into(),
        ));
    }
    let action = facts["action"]
        .as_str()
        .ok_or_else(|| Error::InvalidOperationFacts("action is not text".into()))?;
    let class = catalog
        .actions
        .get(action)
        .ok_or_else(|| Error::InvalidOperationFacts("action is absent from catalog".into()))?;
    Ok(VerifiedCatalogAction {
        class: class.clone(),
    })
}

/// Apply exact/wildcard authority and the reserved binding co-sign rule.
#[must_use]
pub fn catalog_action_permitted(
    catalog: &VerifiedConnectorCatalog,
    action: &str,
    authority: &str,
    owner_co_sign: bool,
) -> bool {
    let Some(class) = catalog.actions.get(action) else {
        return false;
    };
    let exact = format!("act.x.{}.{}", catalog.connector, action);
    let wildcard = format!("act.x.{}.*", catalog.connector);
    if authority == exact {
        return class != "binding" || owner_co_sign;
    }
    authority == wildcard && matches!(class.as_str(), "read" | "act")
}

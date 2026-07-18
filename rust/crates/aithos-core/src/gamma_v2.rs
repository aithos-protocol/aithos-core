//! Pure Gamma-v2 admission and semantic replay.
//!
//! Gamma v1 remains represented by [`crate::gamma::Entry`]. This module is
//! additive: it validates the signed v2 evidence profile without reinterpreting
//! historical v1 bytes.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::gamma::{Entry, Kind};
use crate::{jcs, wire};
use crate::{Error, Result};

const OPERATION_PROFILE_KEY: &str = "aithos-operation-core";
const OPERATION_PROFILE: &str = "1.0.0-draft.1";
const OPERATION_DOMAIN: &str = "aithos-core/v1/operation-commitment";

fn invalid(detail: impl Into<String>) -> Error {
    Error::InvalidGammaEntry(detail.into())
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

fn exact_reference(value: &Value) -> Result<&Map<String, Value>> {
    let reference = value
        .as_object()
        .ok_or_else(|| invalid("operation_ref is not an object"))?;
    let keys = [OPERATION_PROFILE_KEY, "occurrence", "commitment"];
    if reference.len() != keys.len() || keys.iter().any(|key| !reference.contains_key(*key)) {
        return Err(invalid("operation_ref has a non-exact member set"));
    }
    if reference[OPERATION_PROFILE_KEY].as_str() != Some(OPERATION_PROFILE) {
        return Err(invalid("operation_ref has an unknown profile"));
    }
    if !reference["occurrence"]
        .as_str()
        .and_then(|value| value.strip_prefix("op_"))
        .is_some_and(is_ulid)
    {
        return Err(invalid("operation_ref has an invalid occurrence"));
    }
    if !reference["commitment"].as_str().is_some_and(is_commitment) {
        return Err(invalid("operation_ref has an invalid commitment"));
    }
    Ok(reference)
}

fn signature_block(value: &Value) -> Result<&Map<String, Value>> {
    let signature = value
        .as_object()
        .ok_or_else(|| invalid("signature is not an object"))?;
    let keys = ["alg", "key", "value"];
    if signature.len() != keys.len() || keys.iter().any(|key| !signature.contains_key(*key)) {
        return Err(invalid("signature has a non-exact member set"));
    }
    if signature["alg"].as_str() != Some("ed25519") {
        return Err(invalid("signature algorithm is not ed25519"));
    }
    let key = signature["key"]
        .as_str()
        .ok_or_else(|| invalid("signature key is not text"))?;
    wire::multibase_to_ed25519_pub(key)
        .map_err(|_| invalid("signature key is not an Ed25519 multibase key"))?;
    if !signature["value"]
        .as_str()
        .is_some_and(|value| is_lower_hex(value, 64))
    {
        return Err(invalid("signature value is not strict Ed25519 hex"));
    }
    Ok(signature)
}

fn canonical_operation_commitment(projection: &Value) -> Result<(&str, String)> {
    let projection = projection
        .as_object()
        .ok_or_else(|| Error::InvalidOperation("projection is not an object".into()))?;
    if projection[OPERATION_PROFILE_KEY].as_str() != Some(OPERATION_PROFILE) {
        return Err(Error::InvalidOperation(
            "projection has an unknown operation profile".into(),
        ));
    }
    let occurrence = projection["occurrence"]
        .as_str()
        .filter(|value| value.strip_prefix("op_").is_some_and(is_ulid))
        .ok_or_else(|| Error::InvalidOperation("projection occurrence is invalid".into()))?;
    let canonical = jcs::canonicalize(&Value::Object(projection.clone()))
        .map_err(|error| Error::InvalidOperation(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(OPERATION_DOMAIN.as_bytes());
    hasher.update([0]);
    hasher.update(canonical.as_bytes());
    Ok((
        occurrence,
        format!("sha256:{}", hex::encode(hasher.finalize())),
    ))
}

#[derive(Debug)]
pub struct VerifiedGammaV2Entry {
    kind: String,
    operation_ref: Option<Value>,
}

impl VerifiedGammaV2Entry {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn operation_ref(&self) -> Option<&Value> {
        self.operation_ref.as_ref()
    }
}

pub fn verify_gamma_v2_entry(
    entry: &Value,
    projection: Option<&Value>,
) -> Result<VerifiedGammaV2Entry> {
    let object = entry
        .as_object()
        .ok_or_else(|| invalid("Gamma-v2 entry is not an object"))?;
    let required = ["v", "id", "prev", "at", "kind", "signature"];
    let allowed = [
        "v",
        "id",
        "prev",
        "prevs",
        "at",
        "kind",
        "target",
        "authorized_by",
        "authorized_via",
        "payload",
        "body_enc",
        "operation_ref",
        "signature",
    ];
    if required.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !allowed.contains(&key.as_str()))
        || object.values().any(Value::is_null)
    {
        return Err(invalid("Gamma-v2 entry has a non-exact or null member"));
    }
    if object["v"].as_u64() != Some(2) {
        return Err(invalid("Gamma-v2 entry has an unsupported version"));
    }
    if !object["id"]
        .as_str()
        .and_then(|value| value.strip_prefix("gamma_"))
        .is_some_and(is_ulid)
    {
        return Err(invalid("Gamma-v2 entry has an invalid id"));
    }
    if !object["prev"]
        .as_str()
        .is_some_and(|value| value.is_empty() || is_commitment(value))
    {
        return Err(invalid("Gamma-v2 entry has an invalid prev"));
    }
    let at = object["at"]
        .as_str()
        .ok_or_else(|| invalid("Gamma-v2 entry at is not text"))?;
    crate::gamma::ts_epoch(at).map_err(|_| invalid("Gamma-v2 entry at is not canonical"))?;
    let kind_name = object["kind"]
        .as_str()
        .ok_or_else(|| invalid("Gamma-v2 entry kind is not text"))?;
    let kind = Kind::parse(kind_name)?;

    let operation_ref = object.get("operation_ref");
    match (kind, operation_ref) {
        (Kind::Heartbeat, None) => {
            if projection.is_some() {
                return Err(invalid("heartbeat has no operation projection"));
            }
        }
        (Kind::Heartbeat, Some(_)) => {
            return Err(invalid("heartbeat forbids operation_ref"));
        }
        (_, None) => {
            return Err(invalid("operation-bearing kind requires operation_ref"));
        }
        (_, Some(reference)) => {
            exact_reference(reference)?;
        }
    }

    let signature = signature_block(&object["signature"])?;
    let mut historical = entry.clone();
    historical["v"] = Value::from(1);
    historical
        .as_object_mut()
        .expect("entry was checked as an object")
        .remove("operation_ref");
    let historical: Entry = serde_json::from_value(historical)
        .map_err(|error| invalid(format!("Gamma-v2 entry shape is invalid: {error}")))?;
    historical.check_form()?;

    let mut unsigned = entry.clone();
    unsigned["signature"]["value"] = Value::String(String::new());
    let preimage = jcs::canonical_bytes(&unsigned)
        .map_err(|error| invalid(format!("Gamma-v2 JCS failed: {error}")))?;
    let key_bytes = wire::multibase_to_ed25519_pub(
        signature["key"]
            .as_str()
            .expect("signature key checked as text"),
    )
    .map_err(|_| invalid("signature key is not Ed25519"))?;
    let key = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|_| invalid("signature key is not Ed25519"))?;
    let signature_bytes: [u8; 64] = hex::decode(
        signature["value"]
            .as_str()
            .expect("signature value checked as text"),
    )
    .map_err(|_| invalid("signature value is not hex"))?
    .try_into()
    .map_err(|_| invalid("signature value is not 64 bytes"))?;
    key.verify(&preimage, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| invalid("Gamma-v2 signature does not verify"))?;

    if let Some(reference) = operation_ref {
        let projection = projection
            .ok_or_else(|| Error::InvalidOperation("operation projection is missing".into()))?;
        let (occurrence, commitment) = canonical_operation_commitment(projection)?;
        let reference = exact_reference(reference)?;
        if reference["occurrence"].as_str() != Some(occurrence)
            || reference["commitment"].as_str() != Some(commitment.as_str())
        {
            return Err(Error::InvalidOperation(
                "Gamma operation_ref does not select its projection".into(),
            ));
        }
    }

    Ok(VerifiedGammaV2Entry {
        kind: kind_name.to_owned(),
        operation_ref: operation_ref.cloned(),
    })
}

pub fn verify_gamma_profile_transition(
    parent_manifest: &str,
    parent_gamma: &str,
    child_manifest: &str,
    child_gamma: &str,
) -> Result<()> {
    let manifest_rank = |value| match value {
        "1.0.0-draft.1" => Some(1),
        "1.0.0-draft.2" => Some(2),
        _ => None,
    };
    let gamma_rank = |value| match value {
        "v1" => Some(1),
        "v2" => Some(2),
        _ => None,
    };
    let parent_manifest =
        manifest_rank(parent_manifest).ok_or_else(|| invalid("unknown parent manifest profile"))?;
    let child_manifest =
        manifest_rank(child_manifest).ok_or_else(|| invalid("unknown child manifest profile"))?;
    let parent_gamma =
        gamma_rank(parent_gamma).ok_or_else(|| invalid("unknown parent Gamma profile"))?;
    let child_gamma =
        gamma_rank(child_gamma).ok_or_else(|| invalid("unknown child Gamma profile"))?;
    if parent_manifest != parent_gamma
        || child_manifest != child_gamma
        || child_manifest < parent_manifest
    {
        return Err(invalid("Gamma profile transition is not monotone"));
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct GammaOccurrenceRegistry {
    accepted: BTreeMap<String, String>,
}

impl GammaOccurrenceRegistry {
    #[must_use]
    pub fn len(&self) -> usize {
        self.accepted.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accepted.is_empty()
    }

    pub fn admit(&mut self, entry: &VerifiedGammaV2Entry) -> Result<()> {
        let reference = entry
            .operation_ref()
            .ok_or_else(|| Error::InvalidOperation("entry has no operation_ref".into()))?;
        self.admit_reference(reference)
    }

    pub fn admit_reference(&mut self, reference: &Value) -> Result<()> {
        let reference = exact_reference(reference)
            .map_err(|error| Error::InvalidOperation(error.to_string()))?;
        let occurrence = reference["occurrence"]
            .as_str()
            .expect("occurrence checked as text");
        let commitment = reference["commitment"]
            .as_str()
            .expect("commitment checked as text");
        if let Some(existing) = self.accepted.get(occurrence) {
            let detail = if existing == commitment {
                "operation occurrence replay"
            } else {
                "operation occurrence equivocation"
            };
            return Err(Error::InvalidOperation(detail.into()));
        }
        self.accepted
            .insert(occurrence.to_owned(), commitment.to_owned());
        Ok(())
    }
}

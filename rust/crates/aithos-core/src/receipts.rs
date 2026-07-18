//! Pure R2/U1 receipt-v2 and draft.3 obligation-matcher validation.
//!
//! Receipts bind post- or pre-effect evidence to one already reconstructed W1
//! operation reference. They never participate in the pre-effect operation
//! commitment itself.

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde_json::{json, Map, Value};

use crate::jcs;
use crate::wire;
use crate::{Error, Result};

const OPERATION_PROFILE_KEY: &str = "aithos-operation-core";
const OPERATION_PROFILE: &str = "1.0.0-draft.1";
const MANDATE_PROFILE_KEY: &str = "aithos-mandate-core";
const MANDATE_DRAFT3: &str = "1.0.0-draft.3";

#[derive(Clone, Copy)]
enum Rejection {
    Obligation,
    Gamma,
    Mandate,
}

fn rejected(kind: Rejection, detail: impl Into<String>) -> Error {
    match kind {
        Rejection::Obligation => Error::GammaObligationUnsatisfied(detail.into()),
        Rejection::Gamma => Error::InvalidGammaEntry(detail.into()),
        Rejection::Mandate => Error::InvalidMandate(detail.into()),
    }
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    label: &str,
    kind: Rejection,
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

fn required_text<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    label: &str,
    kind: Rejection,
) -> Result<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| rejected(kind, format!("{label} has invalid {key}")))
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

fn is_occurrence(value: &str) -> bool {
    value.strip_prefix("op_").is_some_and(is_ulid)
}

fn timestamp(value: &Value, label: &str, kind: Rejection) -> Result<i64> {
    let text = value
        .as_str()
        .ok_or_else(|| rejected(kind, format!("{label} is not text")))?;
    crate::gamma::ts_epoch(text).map_err(|_| {
        rejected(
            kind,
            format!("{label} is not a canonical RFC3339 Z instant"),
        )
    })
}

fn duration(value: &Value, kind: Rejection) -> Result<i64> {
    let text = value
        .as_str()
        .ok_or_else(|| rejected(kind, "max_age is not a duration"))?;
    crate::gamma::parse_duration(text)
        .map_err(|_| rejected(kind, "max_age is not a canonical duration"))
}

fn verify_signature(
    keys: &[String],
    message: &[u8],
    signature: &Value,
    kind: Rejection,
) -> Result<()> {
    let signature = signature
        .as_str()
        .filter(|value| is_lower_hex(value, 64))
        .ok_or_else(|| rejected(kind, "malformed Ed25519 signature"))?;
    let signature_bytes: [u8; 64] = hex::decode(signature)
        .map_err(|_| rejected(kind, "malformed Ed25519 signature"))?
        .try_into()
        .map_err(|_| rejected(kind, "malformed Ed25519 signature"))?;
    let signature = Signature::from_bytes(&signature_bytes);
    for key in keys {
        let public = wire::multibase_to_ed25519_pub(key)
            .map_err(|_| rejected(kind, "malformed Ed25519 public key"))?;
        let public = VerifyingKey::from_bytes(&public)
            .map_err(|_| rejected(kind, "malformed Ed25519 public key"))?;
        if public.verify(message, &signature).is_ok() {
            return Ok(());
        }
    }
    Err(rejected(
        kind,
        "signature does not verify under a pinned key",
    ))
}

fn verify_operation_reference(value: &Value, kind: Rejection) -> Result<()> {
    let reference = exact_object(
        value,
        &[OPERATION_PROFILE_KEY, "occurrence", "commitment"],
        "operation_ref",
        kind,
    )?;
    if reference[OPERATION_PROFILE_KEY].as_str() != Some(OPERATION_PROFILE) {
        return Err(rejected(kind, "unknown operation_ref profile"));
    }
    if !reference["occurrence"].as_str().is_some_and(is_occurrence) {
        return Err(rejected(kind, "invalid operation occurrence"));
    }
    if !reference["commitment"].as_str().is_some_and(is_commitment) {
        return Err(rejected(kind, "invalid operation commitment"));
    }
    Ok(())
}

fn operation_tuple(context: &Value) -> Result<Value> {
    let kind = context["kind"]
        .as_str()
        .ok_or_else(|| Error::InvalidMandate("operation context has no kind".into()))?;
    let native = &context["native"];
    let tuple = match kind {
        "read" => json!({
            "kind": "read",
            "domain": native["domain"].clone(),
        }),
        "mutation" => json!({
            "kind": "mutation",
            "domain": native["domain"].clone(),
            "verb": native["verb"].clone(),
        }),
        "inference" | "grant" | "revoke" => json!({"kind": kind}),
        "rotate" => json!({
            "kind": "rotate",
            "domain": native["domain"].clone(),
        }),
        "publication" => json!({
            "kind": "publication",
            "mode": native["mode"].clone(),
        }),
        _ => {
            return Err(Error::InvalidMandate(
                "operation kind has no non-action matcher".into(),
            ))
        }
    };
    Ok(tuple)
}

fn verify_matcher(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::InvalidMandate("matcher is not an object".into()))?;
    let kind = required_text(object, "kind", "matcher", Rejection::Mandate)?;
    let (keys, valid) = match kind {
        "read" => (
            &["kind", "domain"][..],
            object
                .get("domain")
                .and_then(Value::as_str)
                .is_some_and(|domain| matches!(domain, "ethos" | "gamma" | "vault-config")),
        ),
        "mutation" => {
            let valid = matches!(
                (
                    object.get("domain").and_then(Value::as_str),
                    object.get("verb").and_then(Value::as_str),
                ),
                (Some("ethos"), Some("create" | "edit" | "delete" | "redact"))
                    | (
                        Some("structure"),
                        Some("create" | "rename" | "delete" | "move")
                    )
                    | (Some("vault-config"), Some("create" | "edit" | "delete"))
            );
            (&["kind", "domain", "verb"][..], valid)
        }
        "inference" | "grant" | "revoke" => (&["kind"][..], true),
        "rotate" => (
            &["kind", "domain"][..],
            object
                .get("domain")
                .and_then(Value::as_str)
                .is_some_and(|domain| {
                    matches!(domain, "ethos-zone" | "ethos-node" | "vault" | "identity")
                }),
        ),
        "publication" => (
            &["kind", "mode"][..],
            object
                .get("mode")
                .and_then(Value::as_str)
                .is_some_and(|mode| matches!(mode, "normal" | "merge" | "resolution")),
        ),
        _ => {
            return Err(Error::InvalidMandate(
                "unknown or action matcher kind".into(),
            ))
        }
    };
    exact_object(value, keys, "operation matcher", Rejection::Mandate)?;
    if !valid
        || object
            .values()
            .any(|item| item.as_str().is_none_or(str::is_empty))
    {
        return Err(Error::InvalidMandate(
            "invalid closed operation matcher".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct VerifiedObligation {
    profile: String,
    identifier: String,
    attestors: Vec<String>,
    document: Value,
}

impl VerifiedObligation {
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// Validate one historical action or draft.3 non-action obligation.
pub fn verify_obligation(profile: &str, value: &Value) -> Result<VerifiedObligation> {
    let object = value
        .as_object()
        .ok_or_else(|| Error::InvalidMandate("obligation is not an object".into()))?;
    let selectors = ["applies_to", "applies_to_operation"]
        .into_iter()
        .filter(|key| object.contains_key(*key))
        .collect::<Vec<_>>();
    if selectors.len() != 1 {
        return Err(Error::InvalidMandate(
            "obligation must carry exactly one selector".into(),
        ));
    }
    let mut keys = vec!["id", "check", "attestor", "verdict", selectors[0]];
    if object.contains_key("max_age") {
        keys.push("max_age");
    }
    let object = exact_object(value, &keys, "obligation", Rejection::Mandate)?;
    for key in ["id", "check", "verdict"] {
        required_text(object, key, "obligation", Rejection::Mandate)?;
    }
    let attestors = object["attestor"]
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| Error::InvalidMandate("invalid obligation attestor set".into()))?
        .iter()
        .map(|item| {
            let key = item
                .as_str()
                .ok_or_else(|| Error::InvalidMandate("obligation attestor is not text".into()))?;
            wire::multibase_to_ed25519_pub(key)
                .map_err(|_| Error::InvalidMandate("invalid obligation attestor key".into()))?;
            Ok(key.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    if attestors.iter().collect::<BTreeSet<_>>().len() != attestors.len() {
        return Err(Error::InvalidMandate(
            "duplicate obligation attestor".into(),
        ));
    }
    if let Some(max_age) = object.get("max_age") {
        duration(max_age, Rejection::Mandate)?;
    }
    if let Some(action) = object.get("applies_to") {
        if action.as_str().is_none_or(str::is_empty) {
            return Err(Error::InvalidMandate(
                "invalid historical action selector".into(),
            ));
        }
    } else {
        if profile != MANDATE_DRAFT3 {
            return Err(Error::InvalidMandate(
                "non-action matcher requires draft.3".into(),
            ));
        }
        verify_matcher(&object["applies_to_operation"])?;
    }
    if !matches!(profile, "1.0.0-draft.1" | "1.0.0-draft.2" | MANDATE_DRAFT3) {
        return Err(Error::InvalidMandate(
            "unknown obligation mandate profile".into(),
        ));
    }
    Ok(VerifiedObligation {
        profile: profile.to_owned(),
        identifier: object["id"]
            .as_str()
            .ok_or_else(|| Error::InvalidMandate("obligation id is not text".into()))?
            .to_owned(),
        attestors,
        document: value.clone(),
    })
}

/// Decide whether one verified obligation selects a reconstructed operation.
pub fn obligation_matches(obligation: &VerifiedObligation, context: &Value) -> Result<bool> {
    if let Some(matcher) = obligation.document.get("applies_to_operation") {
        if obligation.profile != MANDATE_DRAFT3 {
            return Err(Error::InvalidMandate(
                "non-action matcher is outside draft.3".into(),
            ));
        }
        return Ok(*matcher == operation_tuple(context)?);
    }
    if context["kind"].as_str() != Some("action") {
        return Ok(false);
    }
    let selector = obligation.document["applies_to"]
        .as_str()
        .ok_or_else(|| Error::InvalidMandate("invalid action selector".into()))?;
    let action = context["native"]["action_selector"]
        .as_str()
        .ok_or_else(|| Error::InvalidMandate("action context has no selector".into()))?;
    Ok(selector
        .strip_suffix('*')
        .map_or(selector == action, |prefix| action.starts_with(prefix)))
}

#[derive(Debug)]
pub struct VerifiedObligationChain {
    depth: usize,
}

impl VerifiedObligationChain {
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

/// Validate homogeneous draft.3 matcher obligations and add-only inheritance.
pub fn verify_obligation_chain(chain: &Value) -> Result<VerifiedObligationChain> {
    let chain = chain
        .as_array()
        .filter(|items| !items.is_empty())
        .ok_or_else(|| Error::InvalidMandate("obligation chain is empty".into()))?;
    let mut parent_id: Option<&str> = None;
    let mut inherited: BTreeMap<String, Value> = BTreeMap::new();
    for record in chain {
        let object = record
            .as_object()
            .ok_or_else(|| Error::InvalidMandate("matcher mandate is not an object".into()))?;
        let keys = [MANDATE_PROFILE_KEY, "id", "parent", "constraints"];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(Error::InvalidMandate(
                "matcher mandate has a non-exact member set".into(),
            ));
        }
        if object
            .iter()
            .any(|(key, value)| key != "parent" && value.is_null())
        {
            return Err(Error::InvalidMandate(
                "matcher mandate contains null".into(),
            ));
        }
        if object[MANDATE_PROFILE_KEY].as_str() != Some(MANDATE_DRAFT3) {
            return Err(Error::InvalidMandate(
                "matcher chain is not homogeneous draft.3".into(),
            ));
        }
        let id = required_text(object, "id", "matcher mandate", Rejection::Mandate)?;
        match parent_id {
            None if !object["parent"].is_null() => {
                return Err(Error::InvalidMandate(
                    "root matcher mandate has a parent".into(),
                ))
            }
            Some(expected) if object["parent"].as_str() != Some(expected) => {
                return Err(Error::InvalidMandate(
                    "matcher mandate parent mismatch".into(),
                ))
            }
            _ => {}
        }
        let constraints = exact_object(
            &object["constraints"],
            &["obligations"],
            "matcher constraints",
            Rejection::Mandate,
        )?;
        let obligations = constraints["obligations"]
            .as_array()
            .ok_or_else(|| Error::InvalidMandate("obligations are not an array".into()))?;
        let mut current = BTreeMap::new();
        for obligation in obligations {
            let verified = verify_obligation(MANDATE_DRAFT3, obligation)?;
            if current
                .insert(verified.identifier().to_owned(), obligation.clone())
                .is_some()
            {
                return Err(Error::InvalidMandate("duplicate obligation id".into()));
            }
        }
        for (identifier, obligation) in &inherited {
            if current.get(identifier) != Some(obligation) {
                return Err(Error::InvalidMandate(
                    "inherited obligation was dropped or altered".into(),
                ));
            }
        }
        inherited = current;
        parent_id = Some(id);
    }
    Ok(VerifiedObligationChain { depth: chain.len() })
}

#[derive(Debug)]
pub struct VerifiedR2Receipt {
    operation_ref: Value,
}

impl VerifiedR2Receipt {
    #[must_use]
    pub fn operation_ref(&self) -> &Value {
        &self.operation_ref
    }
}

/// Validate exactly one operation-bound R2 obligation receipt.
pub fn verify_r2_receipt(
    receipts: &Value,
    context: &Value,
    mandate_profile: &str,
    obligation: &Value,
) -> Result<VerifiedR2Receipt> {
    let receipts = receipts
        .as_array()
        .filter(|items| items.len() == 1)
        .ok_or_else(|| {
            Error::GammaObligationUnsatisfied("exactly one R2 receipt is required".into())
        })?;
    let obligation = verify_obligation(mandate_profile, obligation)
        .map_err(|error| Error::GammaObligationUnsatisfied(error.to_string()))?;
    if !obligation_matches(&obligation, context)
        .map_err(|error| Error::GammaObligationUnsatisfied(error.to_string()))?
    {
        return Err(Error::GammaObligationUnsatisfied(
            "obligation does not select this operation".into(),
        ));
    }
    let receipt = &receipts[0];
    let mut keys = vec![
        "v",
        "family",
        "operation_ref",
        "obligation",
        "verdict",
        "at",
        "sig",
    ];
    if receipt
        .as_object()
        .is_some_and(|object| object.contains_key("presented_digest"))
    {
        keys.push("presented_digest");
    }
    let receipt = exact_object(receipt, &keys, "R2 receipt", Rejection::Obligation)?;
    if receipt["v"].as_u64() != Some(2) {
        return Err(rejected(
            Rejection::Obligation,
            "R2 version is not JSON number 2",
        ));
    }
    if receipt["family"].as_str() != Some("obligation") {
        return Err(rejected(Rejection::Obligation, "wrong R2 family"));
    }
    verify_operation_reference(&receipt["operation_ref"], Rejection::Obligation)?;
    if receipt["operation_ref"] != context["operation_ref"] {
        return Err(rejected(Rejection::Obligation, "R2 operation_ref mismatch"));
    }
    if receipt["obligation"].as_str() != Some(obligation.identifier()) {
        return Err(rejected(Rejection::Obligation, "R2 obligation id mismatch"));
    }
    if receipt["verdict"] != obligation.document["verdict"] {
        return Err(rejected(Rejection::Obligation, "R2 verdict mismatch"));
    }
    let receipt_at = timestamp(&receipt["at"], "R2 at", Rejection::Obligation)?;
    let operation_at = timestamp(
        &context["projection"]["at"],
        "operation at",
        Rejection::Obligation,
    )?;
    if let Some(max_age) = obligation.document.get("max_age") {
        let max_age = duration(max_age, Rejection::Obligation)?;
        if operation_at.abs_diff(receipt_at) > max_age as u64 {
            return Err(rejected(Rejection::Obligation, "R2 receipt is stale"));
        }
    }
    if receipt
        .get("presented_digest")
        .is_some_and(|value| !value.as_str().is_some_and(is_commitment))
    {
        return Err(rejected(
            Rejection::Obligation,
            "invalid R2 presented_digest",
        ));
    }
    let mut unsigned = receipt.clone();
    unsigned.remove("sig");
    verify_signature(
        &obligation.attestors,
        &jcs::canonical_bytes(&Value::Object(unsigned))
            .map_err(|error| rejected(Rejection::Obligation, error.to_string()))?,
        &receipt["sig"],
        Rejection::Obligation,
    )?;
    Ok(VerifiedR2Receipt {
        operation_ref: receipt["operation_ref"].clone(),
    })
}

#[derive(Debug)]
pub struct VerifiedU1Receipt {
    actual_tokens: u64,
}

impl VerifiedU1Receipt {
    #[must_use]
    pub const fn actual_tokens(&self) -> u64 {
        self.actual_tokens
    }
}

/// Validate exactly one U1 action or inference usage receipt.
pub fn verify_u1_receipt(
    receipts: &Value,
    context: &Value,
    profile: &Value,
) -> Result<VerifiedU1Receipt> {
    let profile = exact_object(
        profile,
        &["id", "models", "require_attestation", "attestation_key"],
        "U1 budget profile",
        Rejection::Gamma,
    )?;
    required_text(profile, "id", "U1 budget profile", Rejection::Gamma)?;
    let models = profile["models"]
        .as_array()
        .filter(|models| !models.is_empty())
        .ok_or_else(|| rejected(Rejection::Gamma, "invalid U1 model list"))?
        .iter()
        .map(|model| {
            model
                .as_str()
                .filter(|model| !model.is_empty())
                .ok_or_else(|| rejected(Rejection::Gamma, "invalid U1 model"))
        })
        .collect::<Result<Vec<_>>>()?;
    if profile["require_attestation"].as_bool() != Some(true) {
        return Err(rejected(
            Rejection::Gamma,
            "U1 profile does not require attestation",
        ));
    }
    let attestation_key = required_text(
        profile,
        "attestation_key",
        "U1 budget profile",
        Rejection::Gamma,
    )?;
    wire::multibase_to_ed25519_pub(attestation_key)
        .map_err(|_| rejected(Rejection::Gamma, "invalid U1 attestation key"))?;
    let receipts = receipts
        .as_array()
        .filter(|items| items.len() == 1)
        .ok_or_else(|| rejected(Rejection::Gamma, "exactly one U1 receipt is required"))?;
    let kind = context["kind"]
        .as_str()
        .ok_or_else(|| rejected(Rejection::Gamma, "operation context has no kind"))?;
    let (receipt, actual_tokens) = match kind {
        "action" => {
            let receipt = exact_object(
                &receipts[0],
                &["v", "family", "operation_ref", "model", "tokens", "sig"],
                "U1 action receipt",
                Rejection::Gamma,
            )?;
            if receipt["family"].as_str() != Some("usage.action") {
                return Err(rejected(Rejection::Gamma, "wrong U1 action family"));
            }
            let model = required_text(receipt, "model", "U1 action receipt", Rejection::Gamma)?;
            if !models.contains(&model) {
                return Err(rejected(Rejection::Gamma, "U1 action model is not allowed"));
            }
            let tokens = receipt["tokens"]
                .as_u64()
                .ok_or_else(|| rejected(Rejection::Gamma, "U1 action tokens are not u64"))?;
            (receipt, tokens)
        }
        "inference" => {
            let receipt = exact_object(
                &receipts[0],
                &[
                    "v",
                    "family",
                    "operation_ref",
                    "tokens_in",
                    "tokens_out",
                    "sig",
                ],
                "U1 inference receipt",
                Rejection::Gamma,
            )?;
            if receipt["family"].as_str() != Some("usage.inference") {
                return Err(rejected(Rejection::Gamma, "wrong U1 inference family"));
            }
            let model = context["native"]["model"]
                .as_str()
                .ok_or_else(|| rejected(Rejection::Gamma, "inference context has no model"))?;
            if !models.contains(&model) {
                return Err(rejected(
                    Rejection::Gamma,
                    "U1 inference model is not allowed",
                ));
            }
            let tokens_in = receipt["tokens_in"]
                .as_u64()
                .ok_or_else(|| rejected(Rejection::Gamma, "U1 tokens_in is not u64"))?;
            let tokens_out = receipt["tokens_out"]
                .as_u64()
                .ok_or_else(|| rejected(Rejection::Gamma, "U1 tokens_out is not u64"))?;
            let total = tokens_in
                .checked_add(tokens_out)
                .ok_or_else(|| rejected(Rejection::Gamma, "U1 usage total overflows u64"))?;
            (receipt, total)
        }
        _ => {
            return Err(rejected(
                Rejection::Gamma,
                "U1 receipt on a non-usage operation",
            ))
        }
    };
    if receipt["v"].as_u64() != Some(2) {
        return Err(rejected(
            Rejection::Gamma,
            "U1 version is not JSON number 2",
        ));
    }
    verify_operation_reference(&receipt["operation_ref"], Rejection::Gamma)?;
    if receipt["operation_ref"] != context["operation_ref"] {
        return Err(rejected(Rejection::Gamma, "U1 operation_ref mismatch"));
    }
    let mut unsigned = receipt.clone();
    unsigned.remove("sig");
    verify_signature(
        &[attestation_key.to_owned()],
        &jcs::canonical_bytes(&Value::Object(unsigned))
            .map_err(|error| rejected(Rejection::Gamma, error.to_string()))?,
        &receipt["sig"],
        Rejection::Gamma,
    )?;
    Ok(VerifiedU1Receipt { actual_tokens })
}

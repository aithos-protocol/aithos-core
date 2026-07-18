//! Draft.2 K1-C assembly and deterministic keyless publication packages.
//!
//! Bundle owns layout, hashes and atomic Store installation. It delegates the
//! only semantic verdict to [`aithos_core::carriers::verify_k1c_carriers`].

use crate::manifest::{sha256_hex, Manifest, ManifestSigner, ManifestSpec, CORE_DRAFT2_VERSION};
use crate::{validate_store_key, Store};
use aithos_core::carriers::{
    derive_changeset, verify_k1c_carriers, K1cActor, K1cCarrierEnvelope, K1cVerificationContext,
    VerifiedK1cCarriers, CHANGESET_PROFILE, EVIDENCE_PROFILE,
};
use aithos_core::did::DidDocument;
use aithos_core::error::{Error, Result};
use aithos_core::jcs;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

const CHANGESET_DOMAIN: &str = "aithos-core/v1/changeset";
const EVIDENCE_DOMAIN: &str = "aithos-core/v1/evidence";

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidOperation(message.into())
}

fn manifest_error(message: impl Into<String>) -> Error {
    Error::InvalidDidDocument(format!("manifest: {}", message.into()))
}

fn io_error(error: std::io::Error) -> Error {
    Error::SealRejected(format!("keyless package store: {error}"))
}

fn canonical(value: &Value, label: &str) -> Result<Vec<u8>> {
    jcs::canonical_bytes(value).map_err(|error| invalid(format!("{label} JCS failed: {error}")))
}

fn commitment(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn carrier(
    document: &Value,
    profile_key: &str,
    profile: &str,
    domain: &str,
    directory: &str,
) -> Result<(Value, String, Vec<u8>)> {
    let bytes = canonical(document, "carrier")?;
    let digest = commitment(domain, &bytes);
    let suffix = digest
        .strip_prefix("sha256:")
        .expect("generated digest is prefixed");
    Ok((
        serde_json::json!({
            profile_key: profile,
            "digest": digest,
        }),
        format!("{directory}/{suffix}.json"),
        bytes,
    ))
}

fn authority_ids(context: &K1cVerificationContext) -> Result<Vec<String>> {
    context
        .actor
        .authority_references()
        .iter()
        .map(|reference| {
            reference["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid("authority reference id is invalid"))
        })
        .collect()
}

fn expected_prev_hash(context: &K1cVerificationContext) -> Result<String> {
    match (context.height, context.predecessors.as_slice()) {
        (1, []) => Ok(String::new()),
        (height, [predecessor]) if height > 1 => predecessor
            .as_str()
            .and_then(|digest| digest.strip_prefix("sha256:"))
            .filter(|digest| digest.len() == 64)
            .map(str::to_owned)
            .ok_or_else(|| invalid("publication predecessor is invalid")),
        _ => Err(invalid(
            "normal publication requires one predecessor, except genesis",
        )),
    }
}

fn expected_files(
    context: &K1cVerificationContext,
    sidecars: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for (path, bytes) in context.candidate_store.iter().chain(sidecars) {
        validate_store_key(path)
            .map_err(|error| invalid(format!("candidate Store key {path}: {error}")))?;
        if files.insert(path.clone(), sha256_hex(bytes)).is_some() {
            return Err(invalid(format!("duplicate candidate Store key: {path}")));
        }
    }
    Ok(files)
}

/// Typed in-memory form of one signed draft.2 candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft2Candidate {
    pub manifest: Manifest,
    pub changeset: Value,
    pub evidence: Value,
    pub sidecars: BTreeMap<String, Vec<u8>>,
}

impl Draft2Candidate {
    /// Parse the closed candidate boundary. Manifest form is rejected before
    /// any semantic replay or signature-based authority decision.
    pub fn from_value(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| manifest_error("candidate is not an object"))?;
        let keys = ["manifest", "changeset", "evidence", "sidecars"];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(manifest_error("candidate has a non-exact member set"));
        }
        let manifest: Manifest = serde_json::from_value(object["manifest"].clone())
            .map_err(|error| manifest_error(format!("form is invalid: {error}")))?;
        manifest.verify_form()?;
        let sidecars = object["sidecars"]
            .as_object()
            .ok_or_else(|| invalid("candidate sidecars are not an object"))?
            .iter()
            .map(|(path, bytes)| {
                bytes
                    .as_str()
                    .map(|bytes| (path.clone(), bytes.as_bytes().to_vec()))
                    .ok_or_else(|| invalid(format!("sidecar {path} is not UTF-8 text")))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            manifest,
            changeset: object["changeset"].clone(),
            evidence: object["evidence"].clone(),
            sidecars,
        })
    }

    pub fn to_value(&self) -> Result<Value> {
        let sidecars = self
            .sidecars
            .iter()
            .map(|(path, bytes)| {
                std::str::from_utf8(bytes)
                    .map(|bytes| (path.clone(), Value::String(bytes.to_owned())))
                    .map_err(|_| invalid(format!("sidecar {path} is not UTF-8")))
            })
            .collect::<Result<serde_json::Map<_, _>>>()?;
        Ok(serde_json::json!({
            "manifest": self.manifest,
            "changeset": self.changeset,
            "evidence": self.evidence,
            "sidecars": sidecars,
        }))
    }
}

/// Derive both carrier documents, address and pin their exact JCS bytes, sign
/// the manifest, then run the same Core verdict used by cold verification.
pub fn assemble_draft2_candidate(
    context: &K1cVerificationContext,
    evidence: Value,
    signer: ManifestSigner<'_>,
) -> Result<Draft2Candidate> {
    let changeset = serde_json::to_value(derive_changeset(context)?)
        .map_err(|error| invalid(format!("changeset encoding failed: {error}")))?;
    let (changeset_ref, changeset_path, changeset_bytes) = carrier(
        &changeset,
        "aithos-changeset-core",
        CHANGESET_PROFILE,
        CHANGESET_DOMAIN,
        "changesets",
    )?;
    let (evidence_ref, evidence_path, evidence_bytes) = carrier(
        &evidence,
        "aithos-evidence-core",
        EVIDENCE_PROFILE,
        EVIDENCE_DOMAIN,
        "evidence",
    )?;
    let sidecars = BTreeMap::from([
        (changeset_path, changeset_bytes),
        (evidence_path, evidence_bytes),
    ]);
    let files = expected_files(context, &sidecars)?;
    let manifest = Manifest::build_draft2(
        ManifestSpec {
            height: context.height,
            prev_hash: expected_prev_hash(context)?,
            created_at: context.publication_at.clone(),
            files,
            roots: BTreeMap::new(),
            gamma_roots: BTreeMap::new(),
            gamma_counts_root: String::new(),
            gamma_head: context.gamma_source_head.clone(),
            merges: Vec::new(),
            resolves_fork: String::new(),
            authorized_via: authority_ids(context)?,
        },
        context.publication_ref.clone(),
        changeset_ref,
        evidence_ref,
        signer,
    )?;
    let candidate = Draft2Candidate {
        manifest,
        changeset,
        evidence,
        sidecars,
    };
    verify_draft2_candidate(&candidate, context)?;
    Ok(candidate)
}

/// Bundle's sole layout-to-Core K1-C façade.
pub fn verify_draft2_candidate(
    candidate: &Draft2Candidate,
    context: &K1cVerificationContext,
) -> Result<VerifiedK1cCarriers> {
    candidate.manifest.verify_form()?;
    if candidate.manifest.version != CORE_DRAFT2_VERSION {
        return Err(manifest_error("candidate is not draft.2"));
    }
    match &context.actor {
        K1cActor::Owner { .. } if candidate.manifest.signature.key != "#root" => {
            return Err(manifest_error("owner manifest is not root-labelled"));
        }
        K1cActor::Grantee { key, .. } if candidate.manifest.signature.key != *key => {
            return Err(manifest_error(
                "grantee manifest signature key differs from its actor",
            ));
        }
        _ => {}
    }
    candidate
        .manifest
        .verify_actor_signature(context.actor.public_key())?;
    if candidate.manifest.edition.height != context.height
        || candidate.manifest.edition.prev_hash != expected_prev_hash(context)?
        || candidate.manifest.edition.created_at != context.publication_at
        || candidate.manifest.authorized_via != authority_ids(context)?
        || candidate.manifest.gamma_head != context.gamma_source_head
    {
        return Err(invalid(
            "manifest edition, authority, time, predecessor or Gamma head mismatch",
        ));
    }
    let files = expected_files(context, &candidate.sidecars)?;
    if candidate.manifest.files != files {
        return Err(invalid(
            "manifest files are not the complete candidate plus carrier pins",
        ));
    }
    let envelope = K1cCarrierEnvelope {
        changeset: candidate.changeset.clone(),
        evidence: candidate.evidence.clone(),
        operation_ref: candidate
            .manifest
            .operation_ref
            .clone()
            .ok_or_else(|| manifest_error("operation_ref is absent"))?,
        changeset_ref: candidate
            .manifest
            .changeset_ref
            .clone()
            .ok_or_else(|| manifest_error("changeset_ref is absent"))?,
        evidence_ref: candidate
            .manifest
            .evidence_ref
            .clone()
            .ok_or_else(|| manifest_error("evidence_ref is absent"))?,
        files: candidate.manifest.files.clone(),
        sidecars: candidate.sidecars.clone(),
    };
    verify_k1c_carriers(&envelope, context)
}

pub fn verify_draft2_candidate_value(
    candidate: &Value,
    context: &K1cVerificationContext,
) -> Result<VerifiedK1cCarriers> {
    verify_draft2_candidate(&Draft2Candidate::from_value(candidate)?, context)
}

/// A deterministic package of public/opaque objects and public replay inputs.
/// It is deliberately not a capability and contains no signer/opener.
#[derive(Debug, Clone)]
pub struct KeylessPublicationPackage {
    candidate: Draft2Candidate,
    context: K1cVerificationContext,
    objects: BTreeMap<String, Vec<u8>>,
}

impl KeylessPublicationPackage {
    #[must_use]
    pub fn candidate(&self) -> &Draft2Candidate {
        &self.candidate
    }

    #[must_use]
    pub fn context(&self) -> &K1cVerificationContext {
        &self.context
    }

    #[must_use]
    pub fn objects(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.objects
    }

    /// Deterministic digest over the complete public package, independent of
    /// object insertion order.
    pub fn digest(&self) -> Result<String> {
        let objects = self
            .objects
            .iter()
            .map(|(path, bytes)| (path.clone(), Value::String(hex::encode(bytes))))
            .collect::<serde_json::Map<_, _>>();
        let value = serde_json::json!({
            "aithos-keyless-publication": "1.0.0-draft.1",
            "candidate": self.candidate.to_value()?,
            "context": self.context,
            "objects_hex": objects,
        });
        Ok(format!(
            "sha256:{}",
            sha256_hex(&canonical(&value, "package")?)
        ))
    }

    pub fn verify_public_only(&self) -> Result<VerifiedK1cCarriers> {
        reject_private_shape(
            &serde_json::to_value(&self.context).map_err(|error| {
                invalid(format!("publication context encoding failed: {error}"))
            })?,
        )?;
        verify_draft2_candidate(&self.candidate, &self.context)
    }
}

/// Export a signed candidate and every object it pins. Extra public objects
/// are limited to DID/manifest history and must remain inside Store grammar.
pub fn export_keyless(
    candidate: Draft2Candidate,
    context: K1cVerificationContext,
    extra_public_objects: BTreeMap<String, Vec<u8>>,
) -> Result<KeylessPublicationPackage> {
    verify_draft2_candidate(&candidate, &context)?;
    let mut objects = context.candidate_store.clone();
    for (path, bytes) in &candidate.sidecars {
        if objects.insert(path.clone(), bytes.clone()).is_some() {
            return Err(invalid(format!("duplicate exported object: {path}")));
        }
    }
    let manifest_bytes = canonical(
        &serde_json::to_value(&candidate.manifest)
            .map_err(|error| manifest_error(format!("encoding failed: {error}")))?,
        "manifest",
    )?;
    objects.insert("manifest.json".into(), manifest_bytes.clone());
    objects.insert(
        format!("manifests/{}.json", candidate.manifest.edition.height),
        manifest_bytes,
    );
    for (path, bytes) in extra_public_objects {
        validate_store_key(&path)
            .map_err(|error| invalid(format!("extra package object {path}: {error}")))?;
        if objects.insert(path.clone(), bytes).is_some() {
            return Err(invalid(format!("duplicate exported object: {path}")));
        }
    }
    for path in objects.keys() {
        validate_store_key(path)
            .map_err(|error| invalid(format!("exported object {path}: {error}")))?;
    }
    let package = KeylessPublicationPackage {
        candidate,
        context,
        objects,
    };
    package.verify_public_only()?;
    Ok(package)
}

fn reject_private_shape(value: &Value) -> Result<()> {
    const FORBIDDEN: &[&str] = &[
        "seed",
        "private_key",
        "secret_key",
        "owner_keys",
        "dk",
        "credential",
        "plaintext",
        "capability",
    ];
    match value {
        Value::Object(object) => {
            for (name, child) in object {
                if FORBIDDEN.contains(&name.to_ascii_lowercase().as_str()) {
                    return Err(invalid("keyless package contains private material"));
                }
                reject_private_shape(child)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_private_shape(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Install a package into one fresh Store at one logical commit point.
pub fn import_keyless<S: Store>(store: &mut S, package: &KeylessPublicationPackage) -> Result<()> {
    if !store.list("").map_err(io_error)?.is_empty() {
        return Err(invalid("keyless import requires a fresh empty Store"));
    }
    store.begin_transaction().map_err(io_error)?;
    let result = (|| {
        for (path, bytes) in &package.objects {
            store.put(path, bytes).map_err(io_error)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        store.rollback_transaction().map_err(io_error)?;
        return Err(error);
    }
    store.commit_transaction().map_err(io_error)
}

fn candidate_from_store<S: Store>(
    store: &S,
    package: &KeylessPublicationPackage,
) -> Result<Draft2Candidate> {
    let manifest_bytes = store
        .get("manifest.json")
        .map_err(io_error)?
        .ok_or_else(|| manifest_error("manifest.json is missing"))?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| manifest_error(format!("form is invalid: {error}")))?;
    manifest.verify_form()?;
    let changeset_ref = manifest
        .changeset_ref
        .as_ref()
        .and_then(|reference| reference["digest"].as_str())
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or_else(|| manifest_error("changeset_ref is invalid"))?;
    let evidence_ref = manifest
        .evidence_ref
        .as_ref()
        .and_then(|reference| reference["digest"].as_str())
        .and_then(|digest| digest.strip_prefix("sha256:"))
        .ok_or_else(|| manifest_error("evidence_ref is invalid"))?;
    let changeset_path = format!("changesets/{changeset_ref}.json");
    let evidence_path = format!("evidence/{evidence_ref}.json");
    let changeset_bytes = store
        .get(&changeset_path)
        .map_err(io_error)?
        .ok_or_else(|| invalid("changeset sidecar is missing"))?;
    let evidence_bytes = store
        .get(&evidence_path)
        .map_err(io_error)?
        .ok_or_else(|| invalid("evidence sidecar is missing"))?;
    let changeset = serde_json::from_slice(&changeset_bytes)
        .map_err(|error| invalid(format!("changeset sidecar is invalid: {error}")))?;
    let evidence = serde_json::from_slice(&evidence_bytes)
        .map_err(|error| invalid(format!("evidence sidecar is invalid: {error}")))?;
    let candidate = Draft2Candidate {
        manifest,
        changeset,
        evidence,
        sidecars: BTreeMap::from([
            (changeset_path, changeset_bytes),
            (evidence_path, evidence_bytes),
        ]),
    };
    if candidate != package.candidate {
        return Err(invalid(
            "cold Store candidate differs from exported candidate",
        ));
    }
    Ok(candidate)
}

/// Reopen and verify from Store bytes with public package inputs only.
pub fn cold_verify<S: Store>(
    store: &S,
    package: &KeylessPublicationPackage,
) -> Result<VerifiedK1cCarriers> {
    let actual_paths = store.list("").map_err(io_error)?;
    let expected_paths = package.objects.keys().cloned().collect::<Vec<_>>();
    if actual_paths != expected_paths {
        return Err(invalid("cold Store has a missing or unpinned object"));
    }
    for (path, expected) in &package.objects {
        let actual = store
            .get(path)
            .map_err(io_error)?
            .ok_or_else(|| invalid(format!("cold Store object is missing: {path}")))?;
        if &actual != expected {
            return Err(invalid(format!(
                "cold Store object was substituted: {path}"
            )));
        }
    }
    let candidate = candidate_from_store(store, package)?;
    let allowed_paths = candidate
        .manifest
        .files
        .keys()
        .cloned()
        .chain([
            "manifest.json".to_owned(),
            format!("manifests/{}.json", candidate.manifest.edition.height),
        ])
        .collect::<BTreeSet<_>>();
    if actual_paths
        .iter()
        .any(|path| !allowed_paths.contains(path))
    {
        return Err(invalid("cold Store contains an unpinned object"));
    }
    for (path, expected_hash) in &candidate.manifest.files {
        let bytes = store
            .get(path)
            .map_err(io_error)?
            .ok_or_else(|| invalid(format!("pinned object is missing: {path}")))?;
        if &sha256_hex(&bytes) != expected_hash {
            return Err(invalid(format!("pinned object was substituted: {path}")));
        }
    }
    let height = candidate.manifest.edition.height;
    let history_path = format!("manifests/{height}.json");
    if store.get(&history_path).map_err(io_error)?.as_deref()
        != store.get("manifest.json").map_err(io_error)?.as_deref()
    {
        return Err(invalid("manifest.json is not the edition history tip"));
    }
    if height > 1 {
        let parent_path = format!("manifests/{}.json", height - 1);
        let parent_bytes = store
            .get(&parent_path)
            .map_err(io_error)?
            .ok_or_else(|| invalid("expected parent manifest is missing"))?;
        let parent: Manifest = serde_json::from_slice(&parent_bytes)
            .map_err(|error| manifest_error(format!("parent is invalid: {error}")))?;
        let did_bytes = store
            .get("did.json")
            .map_err(io_error)?
            .ok_or_else(|| invalid("cold verification DID document is missing"))?;
        let did: DidDocument = serde_json::from_slice(&did_bytes)
            .map_err(|error| invalid(format!("cold verification DID is invalid: {error}")))?;
        did.verify()?;
        if parent.authorized_via.is_empty() {
            parent.verify_signature(&did)?;
        } else {
            return Err(invalid(
                "cold parent uses delegated authority unsupported by this normal package",
            ));
        }
        if parent.chain_hash()? != candidate.manifest.edition.prev_hash {
            return Err(invalid("candidate names a different parent manifest"));
        }
    }
    verify_draft2_candidate(&candidate, package.context())
}

/// Test/support seam for proving fail-closed package mutations without
/// exposing any signing or opening capability.
pub fn package_with_objects(
    package: &KeylessPublicationPackage,
    objects: BTreeMap<String, Vec<u8>>,
) -> KeylessPublicationPackage {
    KeylessPublicationPackage {
        candidate: package.candidate.clone(),
        context: package.context.clone(),
        objects,
    }
}

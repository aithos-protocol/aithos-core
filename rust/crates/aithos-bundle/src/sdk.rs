//! Narrow SDK-facing orchestration types.
//!
//! This module does not introduce a new wire format. It turns an already
//! signed K1-C package into deterministic provider operations after running
//! the existing local verifier.

use std::collections::BTreeMap;

use aithos_core::error::{Error, Result};

use crate::publication::KeylessPublicationPackage;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidOperation(message.into())
}

/// A locally verified, manifest-last provider upload plan.
///
/// Object bytes are owned by the plan so it can safely cross an async or FFI
/// boundary after the signing session has been dropped. `manifests/*` is not
/// included: edition-history slots are provider-written on manifest commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationUploadPlan {
    expected_head: String,
    new_head: String,
    package_digest: String,
    artifacts: Vec<(String, Vec<u8>)>,
    manifest_bytes: Vec<u8>,
}

impl PublicationUploadPlan {
    /// Verify the complete package locally and derive the provider operation
    /// order. The demo path accepts normal/genesis publications; merge and
    /// resolution orchestration will receive an explicit multi-head API later.
    pub fn verified(package: &KeylessPublicationPackage) -> Result<Self> {
        let verdict = package.verify_for_cas()?;
        let expected_head = match verdict.cas.expected_predecessors.as_slice() {
            [] => "none".to_owned(),
            [head] => head.clone(),
            _ => {
                return Err(invalid(
                    "SDK v0 upload plan does not yet orchestrate multi-head publication",
                ));
            }
        };
        let manifest_bytes = package
            .objects()
            .get("manifest.json")
            .cloned()
            .ok_or_else(|| invalid("verified package has no manifest object"))?;
        let artifacts = package
            .objects()
            .iter()
            .filter(|(path, _)| path.as_str() != "manifest.json" && !path.starts_with("manifests/"))
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect::<Vec<_>>();
        Ok(Self {
            expected_head,
            new_head: verdict.cas.new_manifest_head,
            package_digest: verdict.cas.package_digest,
            artifacts,
            manifest_bytes,
        })
    }

    #[must_use]
    pub fn expected_head(&self) -> &str {
        &self.expected_head
    }

    #[must_use]
    pub fn new_head(&self) -> &str {
        &self.new_head
    }

    #[must_use]
    pub fn package_digest(&self) -> &str {
        &self.package_digest
    }

    #[must_use]
    pub fn artifacts(&self) -> &[(String, Vec<u8>)] {
        &self.artifacts
    }

    #[must_use]
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.manifest_bytes
    }

    /// Reconstruct the provider-independent object map, useful for adapters
    /// that perform their own batching while preserving manifest-last commit.
    #[must_use]
    pub fn artifact_map(&self) -> BTreeMap<String, Vec<u8>> {
        self.artifacts.iter().cloned().collect()
    }
}

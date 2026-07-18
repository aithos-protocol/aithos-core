//! Local mono-Ethos/mono-actor capability sessions.
//!
//! Capabilities are intentionally non-serializable and non-cloneable. Their
//! fields are private, bind one local session id, and expose only typed
//! protocol operations rather than generic sign/open/wrap or raw-key access.

use crate::bundle::Bundle;
use crate::manifest::{sha256_hex, ManifestSigner};
use crate::publication::{assemble_draft2_candidate, Draft2Candidate};
use crate::Store;
use aithos_core::carriers::{K1cActor, K1cVerificationContext};
use aithos_core::error::{Error, Result};
use aithos_core::gamma::Entry;
use aithos_core::keys::OwnerKeys;
use aithos_core::mandate::Mandate;
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use x25519_dalek::StaticSecret;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapabilityClass {
    Manifest,
    Gamma,
    Body,
    Header,
    Audit,
}

#[derive(Debug)]
struct SessionBinding {
    id: u64,
    class: CapabilityClass,
}

/// Narrow manifest signer. No generic byte-signing method exists.
pub struct ManifestSigningCapability<'a> {
    binding: SessionBinding,
    key: &'a SigningKey,
}

/// Narrow Gamma signer marker for the local orchestration layer.
pub struct GammaSigningCapability<'a> {
    binding: SessionBinding,
    _key: &'a SigningKey,
}

enum BodyOpeningSecret<'a> {
    Owner(&'a StaticSecret),
    Grantee(&'a SigningKey),
}

/// Narrow protected-body opener used only by typed Bundle reads.
pub struct BodyOpeningCapability<'a> {
    binding: SessionBinding,
    secret: BodyOpeningSecret<'a>,
}

/// Narrow header-wrapper marker; it never exposes a DK or KEX secret.
pub struct HeaderWrappingCapability<'a> {
    binding: SessionBinding,
    _owner_kex: &'a StaticSecret,
}

/// Narrow owner audit capability. It opens only sealed Gamma arguments
/// through Bundle's typed audit method and never exposes the vault DK.
pub struct AuditArgsCapability<'a> {
    binding: SessionBinding,
    owner_kex: &'a StaticSecret,
}

impl Drop for ManifestSigningCapability<'_> {
    fn drop(&mut self) {}
}

impl Drop for GammaSigningCapability<'_> {
    fn drop(&mut self) {}
}

impl Drop for BodyOpeningCapability<'_> {
    fn drop(&mut self) {}
}

impl Drop for HeaderWrappingCapability<'_> {
    fn drop(&mut self) {}
}

impl Drop for AuditArgsCapability<'_> {
    fn drop(&mut self) {}
}

/// One local session binds a single subject and actor. The actor's public
/// representation must equal the Core verification context at every effect.
pub struct LocalSession<'a> {
    id: u64,
    subject: String,
    actor: K1cActor,
    manifest_key: &'a SigningKey,
    gamma_key: &'a SigningKey,
    owner_kex: Option<&'a StaticSecret>,
}

impl<'a> LocalSession<'a> {
    #[must_use]
    pub fn owner(subject: impl Into<String>, owner: &'a OwnerKeys) -> Self {
        Self {
            id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            subject: subject.into(),
            actor: K1cActor::Owner {
                key: aithos_core::wire::ed25519_pub_to_multibase(
                    &owner.root_sign.verifying_key().to_bytes(),
                ),
            },
            manifest_key: &owner.root_sign,
            gamma_key: &owner.root_sign,
            owner_kex: Some(&owner.owner_kex),
        }
    }

    #[must_use]
    pub fn grantee(
        subject: impl Into<String>,
        key: &'a SigningKey,
        authority_chain: Vec<serde_json::Value>,
    ) -> Self {
        let public_key =
            aithos_core::wire::ed25519_pub_to_multibase(&key.verifying_key().to_bytes());
        Self {
            id: NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed),
            subject: subject.into(),
            actor: K1cActor::Grantee {
                key: public_key,
                authority_chain,
            },
            manifest_key: key,
            gamma_key: key,
            owner_kex: None,
        }
    }

    pub fn grantee_from_mandates(
        subject: impl Into<String>,
        key: &'a SigningKey,
        authority_chain: &[Mandate],
    ) -> Result<Self> {
        Ok(Self::grantee(
            subject,
            key,
            authority_references(authority_chain)?,
        ))
    }

    #[must_use]
    pub fn actor(&self) -> &K1cActor {
        &self.actor
    }

    #[must_use]
    pub fn manifest_capability(&self) -> ManifestSigningCapability<'a> {
        ManifestSigningCapability {
            binding: SessionBinding {
                id: self.id,
                class: CapabilityClass::Manifest,
            },
            key: self.manifest_key,
        }
    }

    #[must_use]
    pub fn gamma_capability(&self) -> GammaSigningCapability<'a> {
        GammaSigningCapability {
            binding: SessionBinding {
                id: self.id,
                class: CapabilityClass::Gamma,
            },
            _key: self.gamma_key,
        }
    }

    pub fn body_capability(&self) -> Result<BodyOpeningCapability<'a>> {
        Ok(BodyOpeningCapability {
            binding: SessionBinding {
                id: self.id,
                class: CapabilityClass::Body,
            },
            secret: match self.owner_kex {
                Some(owner_kex) => BodyOpeningSecret::Owner(owner_kex),
                None => BodyOpeningSecret::Grantee(self.manifest_key),
            },
        })
    }

    pub fn header_capability(&self) -> Result<HeaderWrappingCapability<'a>> {
        Ok(HeaderWrappingCapability {
            binding: SessionBinding {
                id: self.id,
                class: CapabilityClass::Header,
            },
            _owner_kex: self.owner_kex.ok_or_else(|| {
                Error::InvalidSession("actor has no owner header capability".into())
            })?,
        })
    }

    pub fn audit_capability(&self) -> Result<AuditArgsCapability<'a>> {
        Ok(AuditArgsCapability {
            binding: SessionBinding {
                id: self.id,
                class: CapabilityClass::Audit,
            },
            owner_kex: self.owner_kex.ok_or_else(|| {
                Error::InvalidSession("actor has no owner audit capability".into())
            })?,
        })
    }

    fn check(&self, binding: &SessionBinding, class: CapabilityClass) -> Result<()> {
        if binding.id != self.id || binding.class != class {
            return Err(Error::InvalidSession(
                "capability belongs to another session or class".into(),
            ));
        }
        Ok(())
    }

    /// The only signing surface for a draft.2 manifest.
    pub fn assemble_draft2(
        &self,
        capability: &ManifestSigningCapability<'_>,
        context: &K1cVerificationContext,
        evidence: serde_json::Value,
    ) -> Result<Draft2Candidate> {
        self.check(&capability.binding, CapabilityClass::Manifest)?;
        if context.subject != self.subject || context.actor != self.actor {
            return Err(Error::InvalidSession(
                "publication context escapes the session subject or actor".into(),
            ));
        }
        let signer = match &self.actor {
            K1cActor::Owner { .. } => ManifestSigner::Root(capability.key),
            K1cActor::Grantee { key, .. } => ManifestSigner::Delegate {
                key_multibase: key.clone(),
                sk: capability.key,
            },
        };
        assemble_draft2_candidate(context, evidence, signer)
    }

    /// Typed protected-content read. Public reads still pass through this
    /// method but require no secret operation.
    pub fn read_owner_section<S: Store>(
        &self,
        capability: &BodyOpeningCapability<'_>,
        bundle: &Bundle<S>,
        zone: Zone,
        display_path: &str,
    ) -> Result<String> {
        self.check(&capability.binding, CapabilityClass::Body)?;
        if bundle.did != self.subject {
            return Err(Error::InvalidSession(
                "body capability belongs to another Ethos".into(),
            ));
        }
        let BodyOpeningSecret::Owner(owner_kex) = &capability.secret else {
            return Err(Error::InvalidSession(
                "grantee body capability cannot perform an owner read".into(),
            ));
        };
        bundle.read_section_with_owner_kex(zone, display_path, owner_kex)
    }

    /// Typed grantee read. The public mandate chain must be the exact chain
    /// bound into this session; the private signing key never leaves the
    /// capability and cannot be used as a generic opening oracle.
    pub fn read_grantee_section<S: Store>(
        &self,
        capability: &BodyOpeningCapability<'_>,
        bundle: &Bundle<S>,
        authority_chain: &[Mandate],
        zone: Zone,
        display_path: &str,
        at: &str,
    ) -> Result<String> {
        self.check(&capability.binding, CapabilityClass::Body)?;
        if bundle.did != self.subject
            || authority_references(authority_chain)? != self.actor.authority_references()
        {
            return Err(Error::InvalidSession(
                "grantee read escapes the session Ethos or authority chain".into(),
            ));
        }
        let BodyOpeningSecret::Grantee(key) = &capability.secret else {
            return Err(Error::InvalidSession(
                "owner body capability cannot perform a grantee read".into(),
            ));
        };
        bundle.read_section_as_agent(authority_chain, key, zone, display_path, at)
    }

    /// Prove class/session binding without exposing a signing oracle.
    pub fn accepts_gamma_capability(&self, capability: &GammaSigningCapability<'_>) -> Result<()> {
        self.check(&capability.binding, CapabilityClass::Gamma)
    }

    /// Prove class/session binding without exposing a wrapping oracle.
    pub fn accepts_header_capability(
        &self,
        capability: &HeaderWrappingCapability<'_>,
    ) -> Result<()> {
        self.check(&capability.binding, CapabilityClass::Header)
    }

    /// Prove audit class/session binding without opening an entry.
    pub fn accepts_audit_capability(&self, capability: &AuditArgsCapability<'_>) -> Result<()> {
        self.check(&capability.binding, CapabilityClass::Audit)
    }

    /// Open one action's sealed arguments without exposing a general body
    /// opener or the vault derivation key.
    pub fn audit_action_args<S: Store>(
        &self,
        capability: &AuditArgsCapability<'_>,
        bundle: &Bundle<S>,
        entry: &Entry,
    ) -> Result<Value> {
        self.check(&capability.binding, CapabilityClass::Audit)?;
        if bundle.did != self.subject {
            return Err(Error::InvalidSession(
                "audit capability belongs to another Ethos".into(),
            ));
        }
        bundle.audit_action_args_with_owner_kex(capability.owner_kex, entry)
    }
}

fn authority_references(authority_chain: &[Mandate]) -> Result<Vec<Value>> {
    authority_chain
        .iter()
        .map(|mandate| {
            let document = serde_json::to_value(mandate).map_err(|error| {
                Error::InvalidSession(format!("mandate encoding failed: {error}"))
            })?;
            let bytes = aithos_core::jcs::canonical_bytes(&document)
                .map_err(|error| Error::InvalidSession(format!("mandate JCS failed: {error}")))?;
            Ok(serde_json::json!({
                "id": mandate.id,
                "certificate_digest": format!("sha256:{}", sha256_hex(&bytes)),
            }))
        })
        .collect()
}

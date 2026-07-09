//! The header object (spec §03): the bridge between the certificate plane
//! and the content plane. The only place a node key is ever stored — sealed.

use crate::error::{Error, Result};
use crate::seal::{line_aad, open_line, seal_line, wrap_aad, wrap_open, wrap_seal};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

pub const OWNER_LABEL: &str = "owner";

/// One recipient of a key version: routing label + kid + X25519 public key.
#[derive(Debug, Clone)]
pub struct Recipient {
    /// Stable routing label (`"owner"` or the grantee's multibase pubkey).
    pub to: String,
    pub kid: String,
    pub pubkey: XPublicKey,
}

impl Recipient {
    pub fn owner(pubkey: XPublicKey) -> Self {
        Recipient {
            to: OWNER_LABEL.to_owned(),
            kid: "owner-kex".to_owned(),
            pubkey,
        }
    }
}

/// One sealed line (§03.1, §03.8). `to`/`kid` are routing hints only —
/// the seal is what grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub to: String,
    pub kid: String,
    pub epk: String,
    pub n: String,
    pub c: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyVersion {
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub object: String,
    pub v: u32,
    pub node: String,
    /// Decimal version → sealed lines. Old versions retained per §03.5.
    pub key_versions: BTreeMap<String, KeyVersion>,
}

fn hex32(s: &str, what: &str) -> Result<[u8; 32]> {
    hex::decode(s)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| Error::SealRejected(format!("bad {what} encoding")))
}

fn hex24(s: &str, what: &str) -> Result<[u8; 24]> {
    hex::decode(s)
        .ok()
        .and_then(|v| <[u8; 24]>::try_from(v).ok())
        .ok_or_else(|| Error::SealRejected(format!("bad {what} encoding")))
}

/// I3: every key version MUST include the owner line.
fn check_owner_line(node: &str, recipients: &[Recipient]) -> Result<()> {
    if recipients.iter().any(|r| r.to == OWNER_LABEL) {
        Ok(())
    } else {
        Err(Error::MissingOwnerLine(node.to_owned()))
    }
}

fn build_lines(
    subject_did: &str,
    node: &str,
    version: u64,
    dk: &[u8; 32],
    recipients: &[Recipient],
    ephemerals: &[[u8; 32]],
    nonces: &[[u8; 24]],
) -> Vec<Line> {
    let aad = line_aad(subject_did, node, version);
    recipients
        .iter()
        .zip(ephemerals.iter().zip(nonces))
        .map(|(r, (e, n))| {
            let (epk, c) = seal_line(&StaticSecret::from(*e), &r.pubkey, dk, n, &aad);
            Line {
                to: r.to.clone(),
                kid: r.kid.clone(),
                epk: hex::encode(epk),
                n: hex::encode(n),
                c: hex::encode(c),
            }
        })
        .collect()
}

impl Header {
    /// Build version 1 of a node's header. Fail-closed on a missing owner
    /// line (I3). One ephemeral and one nonce per recipient, injected.
    pub fn build(
        subject_did: &str,
        node: &str,
        dk: &[u8; 32],
        recipients: &[Recipient],
        ephemerals: &[[u8; 32]],
        nonces: &[[u8; 24]],
    ) -> Result<Self> {
        check_owner_line(node, recipients)?;
        let mut key_versions = BTreeMap::new();
        key_versions.insert(
            "1".to_owned(),
            KeyVersion {
                lines: build_lines(subject_did, node, 1, dk, recipients, ephemerals, nonces),
            },
        );
        Ok(Header {
            object: "header".to_owned(),
            v: 1,
            node: node.to_owned(),
            key_versions,
        })
    }

    /// Grant = append one line to the current version (§03.3). O(1):
    /// every existing line is left byte-identical.
    pub fn append_line(
        &mut self,
        subject_did: &str,
        version: u64,
        dk: &[u8; 32],
        recipient: &Recipient,
        ephemeral: [u8; 32],
        nonce: [u8; 24],
    ) -> Result<()> {
        let aad = line_aad(subject_did, &self.node, version);
        let (epk, c) = seal_line(
            &StaticSecret::from(ephemeral),
            &recipient.pubkey,
            dk,
            &nonce,
            &aad,
        );
        let kv = self
            .key_versions
            .get_mut(&version.to_string())
            .ok_or_else(|| Error::SealRejected(format!("no key version {version}")))?;
        kv.lines.push(Line {
            to: recipient.to.clone(),
            kid: recipient.kid.clone(),
            epk: hex::encode(epk),
            n: hex::encode(nonce),
            c: hex::encode(c),
        });
        Ok(())
    }

    /// Rotate = new key version sealed to the survivors only (§03.4).
    /// The revoked simply has no line; old versions are retained (§03.5).
    pub fn rotate(
        &mut self,
        subject_did: &str,
        new_version: u64,
        new_dk: &[u8; 32],
        survivors: &[Recipient],
        ephemerals: &[[u8; 32]],
        nonces: &[[u8; 24]],
    ) -> Result<()> {
        check_owner_line(&self.node, survivors)?;
        self.key_versions.insert(
            new_version.to_string(),
            KeyVersion {
                lines: build_lines(
                    subject_did,
                    &self.node,
                    new_version,
                    new_dk,
                    survivors,
                    ephemerals,
                    nonces,
                ),
            },
        );
        Ok(())
    }

    /// Open one's line in a given version. Tries every line whose `kid`
    /// matches; the seal itself decides (routing hints grant nothing).
    pub fn open(
        &self,
        subject_did: &str,
        version: u64,
        kid: &str,
        secret: &StaticSecret,
    ) -> Result<[u8; 32]> {
        let aad = line_aad(subject_did, &self.node, version);
        let kv = self
            .key_versions
            .get(&version.to_string())
            .ok_or_else(|| Error::SealRejected(format!("no key version {version}")))?;
        for line in kv.lines.iter().filter(|l| l.kid == kid) {
            let epk = hex32(&line.epk, "epk")?;
            let n = hex24(&line.n, "nonce")?;
            let c = hex::decode(&line.c)
                .map_err(|_| Error::SealRejected("bad ciphertext encoding".to_owned()))?;
            if let Ok(dk) = open_line(secret, &epk, &c, &n, &aad) {
                return Ok(dk);
            }
        }
        Err(Error::SealRejected(format!(
            "no line opens for kid {kid} on {} v{version}",
            self.node
        )))
    }

    /// Parse-time validation: I3 on every version.
    pub fn validate(&self) -> Result<()> {
        for (v, kv) in &self.key_versions {
            if !kv.lines.iter().any(|l| l.to == OWNER_LABEL) {
                return Err(Error::MissingOwnerLine(format!("{} v{v}", self.node)));
            }
        }
        Ok(())
    }
}

/// Up-link / tag-view wrap object (§03.4 step 2bis, §03.8): DK' of a rotated
/// or bridged node, sealed under the via node's key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wrap {
    pub object: String,
    pub node: String,
    pub key_version: u64,
    pub via: String,
    pub n: String,
    pub c: String,
}

impl Wrap {
    pub fn seal(
        subject_did: &str,
        via: &str,
        via_key: &[u8; 32],
        node: &str,
        key_version: u64,
        dk: &[u8; 32],
        nonce: [u8; 24],
    ) -> Self {
        let aad = wrap_aad(subject_did, node, key_version);
        Wrap {
            object: "wrap".to_owned(),
            node: node.to_owned(),
            key_version,
            via: via.to_owned(),
            n: hex::encode(nonce),
            c: hex::encode(wrap_seal(via_key, dk, &nonce, &aad)),
        }
    }

    pub fn open(&self, subject_did: &str, via_key: &[u8; 32]) -> Result<[u8; 32]> {
        let aad = wrap_aad(subject_did, &self.node, self.key_version);
        let n = hex24(&self.n, "nonce")?;
        let c = hex::decode(&self.c)
            .map_err(|_| Error::SealRejected("bad ciphertext encoding".to_owned()))?;
        wrap_open(via_key, &c, &n, &aad)
    }
}

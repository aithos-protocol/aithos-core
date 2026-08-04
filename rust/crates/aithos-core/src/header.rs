//! The header object (spec §03): the bridge between the certificate plane
//! and the content plane. The only place a node key is ever stored — sealed.

use crate::error::{Error, Result};
use crate::seal::{line_aad, open_line, seal_line, wrap_aad, wrap_open, wrap_seal};
use crate::wire;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

pub const OWNER_LABEL: &str = "owner";

/// The `kid` of the owner line (§03.1): the subject's `owner_kex` in
/// multibase, byte-identical to `keys.kex` of its DID document (§01.4). The
/// owner line names its recipient key on the wire exactly as a grantee's line
/// does — that is what lets a verifier holding no key at all recognize it.
#[must_use]
pub fn owner_kid(owner_kex: &XPublicKey) -> String {
    wire::x25519_pub_to_multibase(owner_kex.as_bytes())
}

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
            kid: owner_kid(&pubkey),
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

/// I3 at build time: the recipient set MUST include the owner — the recipient
/// whose KEY is the subject's `owner_kex` (§03.1). The routing label decides
/// nothing. The kid is checked too, so a writer can never emit a header that
/// an edition verifier would reject (§00.2, §09.4).
fn check_owner_line(node: &str, recipients: &[Recipient], owner_kex: &XPublicKey) -> Result<()> {
    let kid = owner_kid(owner_kex);
    if recipients
        .iter()
        .any(|r| r.pubkey.as_bytes() == owner_kex.as_bytes() && r.kid == kid)
    {
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
    /// line (I3) — `owner_kex` is the subject's key as published in its DID
    /// document, and the owner line is the one sealed to it (§03.1). One
    /// ephemeral and one nonce per recipient, injected.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        subject_did: &str,
        node: &str,
        dk: &[u8; 32],
        owner_kex: &XPublicKey,
        recipients: &[Recipient],
        ephemerals: &[[u8; 32]],
        nonces: &[[u8; 24]],
    ) -> Result<Self> {
        Self::build_at(
            subject_did,
            node,
            1,
            dk,
            owner_kex,
            recipients,
            ephemerals,
            nonces,
        )
    }

    /// Build a node's header whose FIRST version is `version` — the moved
    /// node's case (§02.9): its header at the new canonical path opens at
    /// the post-rotation version, while the old-path file keeps the earlier
    /// versions. Same I3 fail-closed rule as [`Header::build`].
    #[allow(clippy::too_many_arguments)]
    pub fn build_at(
        subject_did: &str,
        node: &str,
        version: u64,
        dk: &[u8; 32],
        owner_kex: &XPublicKey,
        recipients: &[Recipient],
        ephemerals: &[[u8; 32]],
        nonces: &[[u8; 24]],
    ) -> Result<Self> {
        check_owner_line(node, recipients, owner_kex)?;
        let mut key_versions = BTreeMap::new();
        key_versions.insert(
            version.to_string(),
            KeyVersion {
                lines: build_lines(
                    subject_did,
                    node,
                    version,
                    dk,
                    recipients,
                    ephemerals,
                    nonces,
                ),
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
    #[allow(clippy::too_many_arguments)]
    pub fn rotate(
        &mut self,
        subject_did: &str,
        new_version: u64,
        new_dk: &[u8; 32],
        owner_kex: &XPublicKey,
        survivors: &[Recipient],
        ephemerals: &[[u8; 32]],
        nonces: &[[u8; 24]],
    ) -> Result<()> {
        check_owner_line(&self.node, survivors, owner_kex)?;
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

    /// Open the OWNER's line in a given version: the line whose `kid` is the
    /// subject's `owner_kex` in multibase (§03.1, §03.2). The kid is derived
    /// from the key held, never spelled out — a read path can no longer look
    /// up a label.
    pub fn open_owner(
        &self,
        subject_did: &str,
        version: u64,
        owner_kex: &StaticSecret,
    ) -> Result<[u8; 32]> {
        let kid = owner_kid(&XPublicKey::from(owner_kex));
        self.open(subject_did, version, &kid, owner_kex)
    }

    /// The owner's line in the LATEST version. Returns `(version, dk)`.
    pub fn open_owner_latest(
        &self,
        subject_did: &str,
        owner_kex: &StaticSecret,
    ) -> Result<(u64, [u8; 32])> {
        let kid = owner_kid(&XPublicKey::from(owner_kex));
        self.open_latest(subject_did, &kid, owner_kex)
    }

    /// Highest key version present (§03.4/§03.5): reads always target the
    /// newest lock.
    #[must_use]
    pub fn latest_version(&self) -> u64 {
        self.key_versions
            .keys()
            .filter_map(|k| k.parse::<u64>().ok())
            .max()
            .unwrap_or(1)
    }

    /// Open one's line in the LATEST version — the post-rotation reader path.
    /// Returns `(version, dk)`.
    pub fn open_latest(
        &self,
        subject_did: &str,
        kid: &str,
        secret: &StaticSecret,
    ) -> Result<(u64, [u8; 32])> {
        let v = self.latest_version();
        Ok((v, self.open(subject_did, v, kid, secret)?))
    }

    /// Rotation well-formedness (§03.4): the new version's recipient set MUST
    /// equal the previous version's minus the revoked (owner always kept). A
    /// smuggled-in recipient — one whose kid is absent from the prior version
    /// — makes the rotation invalid, fail-closed. `owner_kid` is the subject's
    /// `owner_kex` in multibase: the new version MUST carry the owner line as
    /// §03.1 defines it, not merely a line labelled `"owner"`.
    pub fn check_rotation(&self, new_version: u64, owner_kid: &str) -> Result<()> {
        if new_version <= 1 {
            return Ok(());
        }
        let err = |m: String| Error::GammaRevocationRejected(m);
        let prev = self
            .key_versions
            .get(&(new_version - 1).to_string())
            .ok_or_else(|| err(format!("{}: no predecessor version", self.node)))?;
        let new = self
            .key_versions
            .get(&new_version.to_string())
            .ok_or_else(|| err(format!("{}: missing new version", self.node)))?;
        let prev_kids: std::collections::BTreeSet<&str> =
            prev.lines.iter().map(|l| l.kid.as_str()).collect();
        for line in &new.lines {
            if !prev_kids.contains(line.kid.as_str()) {
                return Err(err(format!(
                    "{}: rotation smuggles in recipient {}",
                    self.node, line.kid
                )));
            }
        }
        if !new.lines.iter().any(|l| l.kid == owner_kid) {
            return Err(Error::MissingOwnerLine(format!(
                "{} v{new_version}",
                self.node
            )));
        }
        Ok(())
    }

    /// Parse-time validation, KEYLESS tier (§03.1): every key version MUST
    /// carry a line declaring the subject's `owner_kex` as its `kid`.
    /// `owner_kid` is that key in multibase — byte-identical to `keys.kex` of
    /// the subject's DID document, so an edition verifier passes it straight
    /// from the document it already read, holding no key at all.
    pub fn validate(&self, owner_kid: &str) -> Result<()> {
        for (v, kv) in &self.key_versions {
            if !kv.lines.iter().any(|l| l.kid == owner_kid) {
                return Err(Error::MissingOwnerLine(format!("{} v{v}", self.node)));
            }
        }
        Ok(())
    }

    /// `owner_kex`-BEARING tier (§03.1): the keyless check, plus the proof
    /// that the line declaring `owner_kex` actually opens under it. A line
    /// that names the owner's key but is sealed to another one passes every
    /// keyless verifier — that residual gap is the documented boundary of
    /// §03.1, and this is the check that closes it.
    pub fn validate_as_owner(&self, subject_did: &str, owner_kex: &StaticSecret) -> Result<()> {
        let kid = owner_kid(&XPublicKey::from(owner_kex));
        self.validate(&kid)?;
        for v in self.key_versions.keys() {
            let version: u64 = v
                .parse()
                .map_err(|_| Error::SealRejected(format!("{}: bad key version {v}", self.node)))?;
            self.open(subject_did, version, &kid, owner_kex)
                .map_err(|_| {
                    Error::MissingOwnerLine(format!(
                        "{} v{v}: the line declaring owner_kex does not open under it",
                        self.node
                    ))
                })?;
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

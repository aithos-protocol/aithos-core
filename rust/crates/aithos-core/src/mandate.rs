//! Mandates: the certificate plane (spec §04, §05).
//!
//! A mandate grants a keypair a perimeter, under constraints, for a window.
//! Everything here is pure: the verifier takes time `T` as a parameter and
//! reads only the documents it is handed.

use crate::did::{DidDocument, SignatureBlock};
use crate::error::{Error, Result};
use crate::ids::{validate_tag, Sid};
use crate::jcs;
use crate::keys::ed2x;
use crate::path::Zone;
use crate::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const MANDATE_VERSION: &str = "1.0.0-draft.1";

// ------------------------------------------------------------- perimeter

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verb {
    Read,
    Edit,
    Append,
    Delete,
    Write,
}

impl Verb {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "read" => Verb::Read,
            "edit" => Verb::Edit,
            "append" => Verb::Append,
            "delete" => Verb::Delete,
            "write" => Verb::Write,
            other => return Err(Error::InvalidMandate(format!("unknown verb {other}"))),
        })
    }

    fn as_str(self) -> &'static str {
        match self {
            Verb::Read => "read",
            Verb::Edit => "edit",
            Verb::Append => "append",
            Verb::Delete => "delete",
            Verb::Write => "write",
        }
    }

    /// Verb lattice (§04.2): read ⊑ edit ⊑ append ⊑ write, delete ⊑ write.
    fn covers(self, child: Verb) -> bool {
        use Verb::*;
        match (self, child) {
            (a, b) if a == b => true,
            (Write, _) => true,
            (Append, Read | Edit) => true,
            (Edit, Read) => true,
            _ => false,
        }
    }
}

/// One perimeter entry (§04.2). Selectors compose by intersection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PerimeterEntry {
    Ethos {
        verb: Verb,
        zone: Zone,
        /// Folder sid-path from the zone root; empty = the whole zone.
        dir: Vec<Sid>,
        /// Folder-local (or zone-root) tag restriction.
        tag: Option<String>,
    },
    Issue {
        depth: u32,
    },
}

impl PerimeterEntry {
    pub fn parse(s: &str) -> Result<Self> {
        let err = |m: &str| Error::InvalidMandate(format!("{m}: {s}"));
        if let Some(rest) = s.strip_prefix("issue") {
            let depth = match rest.strip_prefix("#depth=") {
                Some(n) => n.parse().map_err(|_| err("bad depth"))?,
                None if rest.is_empty() => 1,
                _ => return Err(err("bad issue entry")),
            };
            return Ok(PerimeterEntry::Issue { depth });
        }
        let (head, selector) = match s.split_once('#') {
            Some((h, sel)) => (h, Some(sel)),
            None => (s, None),
        };
        let (verb, zone) = head
            .split_once('.')
            .ok_or_else(|| err("want <verb>.<zone>"))?;
        let (verb, zone) = (Verb::parse(verb)?, Zone::parse(zone)?);
        let mut dir = Vec::new();
        let mut tag = None;
        if let Some(sel) = selector {
            for part in sel.split('&') {
                match part.split_once('=') {
                    Some(("dir", p)) => {
                        for seg in p.split('/').filter(|x| !x.is_empty()) {
                            dir.push(Sid::parse(seg)?);
                        }
                    }
                    Some(("tag", t)) => {
                        validate_tag(t)?;
                        tag = Some(t.to_owned());
                    }
                    _ => return Err(err("unknown selector")),
                }
            }
        }
        Ok(PerimeterEntry::Ethos {
            verb,
            zone,
            dir,
            tag,
        })
    }

    pub fn to_entry_string(&self) -> String {
        match self {
            PerimeterEntry::Issue { depth } => format!("issue#depth={depth}"),
            PerimeterEntry::Ethos {
                verb,
                zone,
                dir,
                tag,
            } => {
                let mut out = format!("{}.{}", verb.as_str(), zone.as_str());
                let mut sels = Vec::new();
                if !dir.is_empty() {
                    let p: Vec<String> = dir.iter().map(ToString::to_string).collect();
                    sels.push(format!("dir={}", p.join("/")));
                }
                if let Some(t) = tag {
                    sels.push(format!("tag={t}"));
                }
                if !sels.is_empty() {
                    out.push('#');
                    out.push_str(&sels.join("&"));
                }
                out
            }
        }
    }

    /// Containment (§04.2, §05.3): segment-list dir prefix, tag equality,
    /// verb lattice; an absent dimension covers any value of it.
    pub fn covers(&self, child: &PerimeterEntry) -> bool {
        match (self, child) {
            (PerimeterEntry::Issue { depth: n }, PerimeterEntry::Issue { depth: m }) => m < n,
            (
                PerimeterEntry::Ethos {
                    verb: pv,
                    zone: pz,
                    dir: pd,
                    tag: pt,
                },
                PerimeterEntry::Ethos {
                    verb: cv,
                    zone: cz,
                    dir: cd,
                    tag: ct,
                },
            ) => {
                pz == cz
                    && pv.covers(*cv)
                    && cd.len() >= pd.len()
                    && cd[..pd.len()] == pd[..]
                    && match (pt, ct) {
                        (None, _) => true,
                        (Some(a), Some(b)) => a == b,
                        (Some(_), None) => false,
                    }
            }
            _ => false,
        }
    }
}

/// The operation a verifier is asked about.
#[derive(Debug, Clone)]
pub struct Op<'a> {
    pub verb: Verb,
    pub zone: Zone,
    /// Folder sid-path of the target section.
    pub folders: &'a [Sid],
    /// Clear tags of the target section (empty for none).
    pub tags: &'a [String],
}

/// Does any leaf entry cover the operation?
pub fn covers_op(perimeter: &[PerimeterEntry], op: &Op<'_>) -> bool {
    perimeter.iter().any(|e| match e {
        PerimeterEntry::Ethos {
            verb,
            zone,
            dir,
            tag,
        } => {
            *zone == op.zone
                && verb.covers(op.verb)
                && op.folders.len() >= dir.len()
                && op.folders[..dir.len()] == dir[..]
                && tag.as_ref().is_none_or(|t| op.tags.contains(t))
        }
        PerimeterEntry::Issue { .. } => false,
    })
}

// --------------------------------------------------------------- mandate

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grantee {
    pub id: String,
    pub label: String,
    pub pubkey: String,
    pub kex_pubkey: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mandate {
    #[serde(rename = "aithos-mandate-core")]
    pub version: String,
    pub id: String,
    pub subject: String,
    pub parent: Option<String>,
    pub issued_by: String,
    pub grantee: Grantee,
    pub perimeter: Vec<String>,
    pub constraints: serde_json::Value,
    pub not_before: String,
    pub not_after: String,
    pub issued_at: String,
    pub nonce: String,
    pub signature: SignatureBlock,
}

pub struct MandateSpec<'a> {
    pub id: String,
    pub subject: String,
    pub grantee_id: String,
    pub grantee_label: String,
    pub grantee_pub: &'a VerifyingKey,
    pub perimeter: Vec<PerimeterEntry>,
    pub not_before: String,
    pub not_after: String,
    pub issued_at: String,
    pub nonce: String,
}

fn sign_doc(m: &mut Mandate, key: &SigningKey) -> Result<()> {
    m.signature.value = String::new();
    let bytes = jcs::canonical_bytes(m)?;
    m.signature.value = hex::encode(key.sign(&bytes).to_bytes());
    Ok(())
}

fn verify_sig(m: &Mandate, key: &VerifyingKey) -> Result<()> {
    let mut unsigned = m.clone();
    unsigned.signature.value = String::new();
    let sig: [u8; 64] = hex::decode(&m.signature.value)
        .ok()
        .and_then(|v| v.try_into().ok())
        .ok_or_else(|| Error::InvalidMandate(format!("{}: bad signature encoding", m.id)))?;
    key.verify(
        &jcs::canonical_bytes(&unsigned)?,
        &Signature::from_bytes(&sig),
    )
    .map_err(|_| Error::InvalidMandate(format!("{}: signature does not verify", m.id)))
}

fn grantee_block(spec: &MandateSpec<'_>) -> Grantee {
    Grantee {
        id: spec.grantee_id.clone(),
        label: spec.grantee_label.clone(),
        pubkey: wire::ed25519_pub_to_multibase(&spec.grantee_pub.to_bytes()),
        kex_pubkey: wire::x25519_pub_to_multibase(&ed2x(spec.grantee_pub).to_bytes()),
    }
}

impl Mandate {
    /// Root mandate: issued and signed by the owner's root key (§04.1).
    pub fn build_root(root_sign: &SigningKey, spec: &MandateSpec<'_>) -> Result<Self> {
        let mut m = Mandate {
            version: MANDATE_VERSION.to_owned(),
            id: spec.id.clone(),
            subject: spec.subject.clone(),
            parent: None,
            issued_by: format!("{}#root", spec.subject),
            grantee: grantee_block(spec),
            perimeter: spec
                .perimeter
                .iter()
                .map(PerimeterEntry::to_entry_string)
                .collect(),
            constraints: serde_json::json!({}),
            not_before: spec.not_before.clone(),
            not_after: spec.not_after.clone(),
            issued_at: spec.issued_at.clone(),
            nonce: spec.nonce.clone(),
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: "#root".to_owned(),
                value: String::new(),
            },
        };
        sign_doc(&mut m, root_sign)?;
        Ok(m)
    }

    /// Sub-mandate: minted and signed by the parent's grantee key (§05.2).
    pub fn build_sub(
        parent: &Mandate,
        parent_sk: &SigningKey,
        spec: &MandateSpec<'_>,
    ) -> Result<Self> {
        let mut m = Mandate {
            version: MANDATE_VERSION.to_owned(),
            id: spec.id.clone(),
            subject: spec.subject.clone(),
            parent: Some(parent.id.clone()),
            issued_by: parent.grantee.pubkey.clone(),
            grantee: grantee_block(spec),
            perimeter: spec
                .perimeter
                .iter()
                .map(PerimeterEntry::to_entry_string)
                .collect(),
            constraints: serde_json::json!({}),
            not_before: spec.not_before.clone(),
            not_after: spec.not_after.clone(),
            issued_at: spec.issued_at.clone(),
            nonce: spec.nonce.clone(),
            signature: SignatureBlock {
                alg: "ed25519".to_owned(),
                key: parent.grantee.pubkey.clone(),
                value: String::new(),
            },
        };
        sign_doc(&mut m, parent_sk)?;
        Ok(m)
    }

    /// Re-sign after amendment (re-issuance path, §04.1 widening note).
    pub fn resign(&mut self, key: &SigningKey) -> Result<()> {
        sign_doc(self, key)
    }

    pub fn parsed_perimeter(&self) -> Result<Vec<PerimeterEntry>> {
        self.perimeter
            .iter()
            .map(|s| PerimeterEntry::parse(s))
            .collect()
    }

    pub fn grantee_pub(&self) -> Result<VerifyingKey> {
        let bytes = wire::multibase_to_ed25519_pub(&self.grantee.pubkey)?;
        VerifyingKey::from_bytes(&bytes)
            .map_err(|_| Error::InvalidMandate(format!("{}: malformed grantee key", self.id)))
    }

    /// The kex binding is checked, never trusted (§04.1).
    fn check_kex(&self) -> Result<()> {
        let expected = wire::x25519_pub_to_multibase(&ed2x(&self.grantee_pub()?).to_bytes());
        if self.grantee.kex_pubkey != expected {
            return Err(Error::InvalidMandate(format!(
                "{}: kex_pubkey does not match ed2x(pubkey)",
                self.id
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- chains

/// Offline chain verification (§04.5 + §05.3) at injected time `at`.
/// Fail any ⇒ reject.
pub fn verify_chain(chain: &[Mandate], did_doc: &DidDocument, at: &str) -> Result<()> {
    let err = |m: String| Error::InvalidMandate(m);
    if chain.is_empty() {
        return Err(err("empty chain".into()));
    }
    did_doc.verify()?;

    // Root link.
    let root = &chain[0];
    if root.parent.is_some() {
        return Err(err(format!(
            "{}: root mandate must have no parent",
            root.id
        )));
    }
    if root.subject != did_doc.id || root.issued_by != format!("{}#root", did_doc.id) {
        return Err(err(format!(
            "{}: not issued by the subject's root",
            root.id
        )));
    }
    let root_key_bytes = wire::multibase_to_ed25519_pub(&did_doc.keys.root)?;
    let root_key =
        VerifyingKey::from_bytes(&root_key_bytes).map_err(|_| err("malformed root key".into()))?;
    verify_sig(root, &root_key)?;

    for (i, m) in chain.iter().enumerate() {
        m.check_kex()?;
        // Window at T, for every mandate in the chain (§04.5 step 3).
        if at < m.not_before.as_str() || at > m.not_after.as_str() {
            return Err(err(format!("{}: outside validity window at {at}", m.id)));
        }
        if i == 0 {
            continue;
        }
        let parent = &chain[i - 1];
        // Link identity (§05.3 rule 5).
        if m.parent.as_deref() != Some(parent.id.as_str()) {
            return Err(err(format!("{}: parent id mismatch", m.id)));
        }
        if m.subject != parent.subject {
            return Err(err(format!("{}: subject changes along the chain", m.id)));
        }
        if m.issued_by != parent.grantee.pubkey {
            return Err(err(format!("{}: not issued by its parent's grantee", m.id)));
        }
        if m.grantee.pubkey == m.issued_by {
            return Err(err(format!("{}: self-issued sub-mandate", m.id)));
        }
        verify_sig(m, &parent.grantee_pub()?)?;
        // Window containment (§05.3 rule 2).
        if m.not_before < parent.not_before || m.not_after > parent.not_after {
            return Err(err(format!("{}: window exceeds its parent's", m.id)));
        }
        // Issuing right and depth (§05.1, §05.3 rule 4).
        let parent_perimeter = parent.parsed_perimeter()?;
        let parent_depth = parent_perimeter
            .iter()
            .find_map(|e| match e {
                PerimeterEntry::Issue { depth } => Some(*depth),
                _ => None,
            })
            .ok_or_else(|| err(format!("{}: parent grants no issue right", m.id)))?;
        // Perimeter containment (§05.3 rule 1).
        for child_entry in m.parsed_perimeter()? {
            let ok = match &child_entry {
                PerimeterEntry::Issue { depth } => *depth < parent_depth,
                other => parent_perimeter.iter().any(|p| p.covers(other)),
            };
            if !ok {
                return Err(err(format!(
                    "{}: entry '{}' exceeds the parent perimeter",
                    m.id,
                    child_entry.to_entry_string()
                )));
            }
        }
    }
    Ok(())
}

/// Full verifier front door: chain valid at `at` AND the leaf covers `op`.
pub fn verify_op(chain: &[Mandate], did_doc: &DidDocument, at: &str, op: &Op<'_>) -> Result<()> {
    verify_chain(chain, did_doc, at)?;
    let leaf = chain.last().expect("non-empty");
    if !covers_op(&leaf.parsed_perimeter()?, op) {
        return Err(Error::InvalidMandate(format!(
            "{}: operation not covered by the leaf perimeter",
            leaf.id
        )));
    }
    Ok(())
}

//! Pure, prefix-sensitive semantic replay for historical Gamma entries.
//!
//! The state admits one signed entry at a time. Every decision is made only
//! from the already accepted prefix, the DID document and the supplied
//! certificate set. Link state, revocations and counters advance together only
//! after all semantic checks succeed.

use std::collections::{BTreeMap, BTreeSet};

use crate::constraints::verify_operation_constraints;
use crate::did::DidDocument;
use crate::gamma::{
    self, check_action_append, check_grant_append, verify_delegated_entry, Entry, GammaCounters,
    Kind,
};
use crate::mandate::{covers_op, covers_section_op, Mandate, Op, SectionOp, Verb};
use crate::path::{Leaf, NodePath};
use crate::revocation::{check_revoke_authority, Revocation};
use crate::{Error, Result};

#[derive(Debug)]
pub struct GammaReplayState {
    did_doc: DidDocument,
    certificates: BTreeMap<String, Mandate>,
    accepted: Vec<Entry>,
    hashes: BTreeMap<String, i64>,
    tips: BTreeSet<String>,
    last_hash: String,
    revocations: Vec<Revocation>,
    counters: BTreeMap<String, GammaCounters>,
}

impl GammaReplayState {
    #[must_use]
    pub fn new(did_doc: DidDocument, certificates: BTreeMap<String, Mandate>) -> Self {
        Self {
            did_doc,
            certificates,
            accepted: Vec::new(),
            hashes: BTreeMap::new(),
            tips: BTreeSet::new(),
            last_hash: String::new(),
            revocations: Vec::new(),
            counters: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn accepted_len(&self) -> usize {
        self.accepted.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accepted.is_empty()
    }

    pub fn head(&self) -> Result<String> {
        gamma::head(&self.accepted)
    }

    #[must_use]
    pub fn counters(&self) -> &BTreeMap<String, GammaCounters> {
        &self.counters
    }

    #[must_use]
    pub fn accepted(&self) -> &[Entry] {
        &self.accepted
    }

    fn chain_for_ids(&self, ids: &[String]) -> Result<Vec<Mandate>> {
        ids.iter()
            .map(|id| {
                self.certificates.get(id).cloned().ok_or_else(|| {
                    Error::InvalidMandate(format!("Gamma cites absent certificate {id}"))
                })
            })
            .collect()
    }

    fn certificate_chain(&self, id: &str) -> Result<Vec<Mandate>> {
        let mut reversed = Vec::new();
        let mut seen = BTreeSet::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            if !seen.insert(current.to_owned()) {
                return Err(Error::InvalidMandate(format!(
                    "certificate parent cycle at {current}"
                )));
            }
            let mandate = self.certificates.get(current).ok_or_else(|| {
                Error::InvalidMandate(format!("Gamma target certificate {current} is absent"))
            })?;
            reversed.push(mandate.clone());
            cursor = mandate.parent.as_deref();
        }
        reversed.reverse();
        Ok(reversed)
    }

    fn candidate_link_state(&self, entry: &Entry) -> Result<(String, i64, BTreeSet<String>)> {
        entry.check_form()?;
        let timestamp = gamma::ts_epoch(&entry.at)?;
        let hash = entry.chain_hash()?;
        if self.hashes.contains_key(&hash) {
            return Err(Error::InvalidGammaChain(format!(
                "{}: duplicate entry",
                entry.id
            )));
        }

        let mut tips = self.tips.clone();
        match &entry.prevs {
            Some(predecessors) => {
                for predecessor in predecessors {
                    if !tips.remove(predecessor) {
                        return Err(Error::InvalidGammaChain(format!(
                            "{}: merge prevs do not join the open tips",
                            entry.id
                        )));
                    }
                }
            }
            None if entry.prev.is_empty() => {}
            None => {
                let parent_at = self.hashes.get(&entry.prev).ok_or_else(|| {
                    Error::InvalidGammaChain(format!(
                        "{}: prev does not pin an accepted predecessor",
                        entry.id
                    ))
                })?;
                if timestamp < *parent_at {
                    return Err(Error::InvalidGammaChain(format!(
                        "{}: at goes backward",
                        entry.id
                    )));
                }
                tips.remove(&entry.prev);
            }
        }
        tips.insert(hash.clone());
        Ok((hash, timestamp, tips))
    }

    fn verify_clear_content_perimeter(&self, entry: &Entry, chain: &[Mandate]) -> Result<()> {
        let kind = entry.kind()?;
        let verb = match kind {
            Kind::SectionAdd => Verb::Append,
            Kind::SectionModify => Verb::Edit,
            Kind::SectionDelete | Kind::SectionRedact => Verb::Delete,
            Kind::EthosRead => Verb::Read,
            _ => return Ok(()),
        };
        let Some(target) = entry.target.as_deref() else {
            // Historical sealed Gamma-v1 bodies intentionally hide their SID.
            // Their physics proof is the successful key use at append-time.
            return Ok(());
        };
        let node = NodePath::parse(target)
            .map_err(|error| Error::InvalidMandate(format!("invalid mutation target: {error}")))?;
        let tags = entry
            .payload
            .as_ref()
            .and_then(|payload| payload.get("tags"))
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty delegated chain".into()))?;
        let perimeter = leaf.parsed_perimeter()?;
        let covered = match node.leaf {
            Leaf::Section(sid) => covers_section_op(
                &perimeter,
                &SectionOp {
                    verb,
                    zone: node.zone,
                    sid,
                    folders: &node.folders,
                    tags: &tags,
                },
            ),
            Leaf::Folder if kind == Kind::EthosRead => covers_op(
                &perimeter,
                &Op {
                    verb,
                    zone: node.zone,
                    folders: &node.folders,
                    tags: &tags,
                },
            ),
            _ => false,
        };
        if !covered {
            return Err(Error::InvalidMandate(format!(
                "{}: content operation exceeds the leaf perimeter",
                leaf.id
            )));
        }
        Ok(())
    }

    fn verify_semantics(&self, entry: &Entry) -> Result<Option<Revocation>> {
        let kind = entry.kind()?;
        let chain = match &entry.authorized_via {
            None => {
                gamma::verify_owner_entry(entry, &self.did_doc)?;
                None
            }
            Some(via) => {
                if kind == Kind::Heartbeat {
                    return Err(Error::InvalidGammaEntry(format!(
                        "{}: heartbeat must be owner-signed",
                        entry.id
                    )));
                }
                let chain = self.chain_for_ids(via)?;
                verify_delegated_entry(entry, &chain, &self.did_doc)?;
                crate::mandate::verify_chain_revocable(
                    &chain,
                    &self.did_doc,
                    &entry.at,
                    &self.revocations,
                )?;
                for mandate in &chain {
                    verify_operation_constraints(&mandate.constraints)?;
                }
                for mandate in chain.iter().skip(1) {
                    if !gamma::grant_logged(&self.accepted, &mandate.id) {
                        return Err(Error::GammaGrantNotLogged(mandate.id.clone()));
                    }
                }
                self.verify_clear_content_perimeter(entry, &chain)?;
                Some(chain)
            }
        };

        if matches!(kind, Kind::Action | Kind::Inference) {
            let chain = chain.as_deref().ok_or_else(|| {
                Error::InvalidOperation("metered operation has no delegated authority".into())
            })?;
            check_action_append(&self.accepted, entry, chain, &self.did_doc)?;
        }

        if kind == Kind::Grant {
            let target = entry.target.as_deref().ok_or_else(|| {
                Error::InvalidMandate("Gamma grant has no target certificate".into())
            })?;
            let target_chain = self.certificate_chain(target)?;
            crate::mandate::verify_chain(&target_chain, &self.did_doc, &entry.at)?;
            if let Some(chain) = chain.as_deref() {
                check_grant_append(
                    &self.accepted,
                    chain
                        .last()
                        .ok_or_else(|| Error::InvalidMandate("empty minting chain".into()))?,
                )?;
            } else if target_chain.len() != 1 {
                return Err(Error::InvalidMandate(
                    "owner grant target is not a root mandate".into(),
                ));
            }
        }

        if kind != Kind::Revoke {
            return Ok(None);
        }
        let target = entry
            .target
            .as_deref()
            .ok_or_else(|| Error::InvalidMandate("Gamma revoke has no target mandate".into()))?;
        let target_chain = self.certificate_chain(target)?;
        check_revoke_authority(chain.as_deref(), &target_chain)?;
        Ok(Some(Revocation {
            mandate_id: target.to_owned(),
            revoked_at: entry.at.clone(),
        }))
    }

    /// Admit one entry after replaying every structural and semantic gate
    /// against the exact accepted prefix. A rejection leaves every field
    /// unchanged.
    pub fn admit(&mut self, entry: &Entry) -> Result<()> {
        let (hash, timestamp, tips) = self.candidate_link_state(entry)?;
        let revocation = self.verify_semantics(entry)?;

        self.accepted.push(entry.clone());
        self.hashes.insert(hash.clone(), timestamp);
        self.tips = tips;
        self.last_hash = hash;
        if let Some(revocation) = revocation {
            self.revocations.push(revocation);
        }
        self.counters = gamma::counts_tally(&self.accepted);
        Ok(())
    }

    /// Require the admitted history to end in one resolved causal tip.
    pub fn finish(&self) -> Result<()> {
        if self.accepted.is_empty() {
            return Ok(());
        }
        if self.tips.len() != 1 {
            return Err(Error::InvalidGammaChain(format!(
                "unresolved fork inside the log: {} open tips",
                self.tips.len()
            )));
        }
        if !self.tips.contains(&self.last_hash) {
            return Err(Error::InvalidGammaChain(
                "the last entry is not the chain tip".into(),
            ));
        }
        Ok(())
    }
}

//! Pure merge, fork-resolution and semantic-counter decisions.
//!
//! This module owns no Store layout and performs no signing. Bundle supplies
//! the derived changed SID sets and public occurrence inventory, then uses the
//! verdict while assembling the ordinary K1-C publication package.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidOperation(message.into())
}

/// The single publisher authority of a merge or resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeAuthority {
    Owner,
    Grantee {
        chain_count: usize,
        covered_sids: BTreeSet<String>,
    },
}

fn verify_authority(authority: &MergeAuthority, changed_sids: &BTreeSet<String>) -> Result<()> {
    match authority {
        MergeAuthority::Owner => Ok(()),
        MergeAuthority::Grantee {
            chain_count,
            covered_sids,
        } if *chain_count == 1 && changed_sids.is_subset(covered_sids) => Ok(()),
        MergeAuthority::Grantee { .. } => Err(invalid(
            "merge publisher must present one chain covering every changed SID",
        )),
    }
}

/// Verify disjointness, deletion precedence and the one-publisher rule.
pub fn verify_disjoint_merge(
    left_changed_sids: &BTreeSet<String>,
    right_changed_sids: &BTreeSet<String>,
    deleted_sids: &BTreeSet<String>,
    authority: &MergeAuthority,
) -> Result<Vec<String>> {
    if let Some(sid) = left_changed_sids.intersection(right_changed_sids).next() {
        return Err(Error::EditionFork(format!(
            "same-node conflict on sid {sid}"
        )));
    }
    let touched = left_changed_sids
        .union(right_changed_sids)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !deleted_sids.is_subset(&touched) {
        return Err(invalid("deleted SID is absent from both branch changesets"));
    }
    verify_authority(authority, &touched)?;
    Ok(touched.difference(deleted_sids).cloned().collect())
}

/// Verify that one owner or one covering grantee resolves the complete fork.
pub fn verify_fork_resolution(
    touched_sids: &BTreeSet<String>,
    authority: &MergeAuthority,
) -> Result<()> {
    if touched_sids.is_empty() {
        return Err(invalid("fork resolution has no touched SID"));
    }
    verify_authority(authority, touched_sids)
}

/// One logical occurrence used to recompose post-merge counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticOccurrence {
    pub operation_ref: String,
    pub kind: String,
}

/// Complete semantic totals required by the CB13 cold replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticCounts {
    pub actions: u64,
    pub mutations: u64,
    pub consumptions: u64,
    pub direct_children: u64,
}

/// Union the shared prefix and both branches by operation reference. The same
/// reference may occur on both branches only with the same semantic kind.
pub fn recompose_counts(
    left: &[SemanticOccurrence],
    right: &[SemanticOccurrence],
) -> Result<SemanticCounts> {
    let mut unique = BTreeMap::new();
    for occurrence in left.iter().chain(right) {
        if occurrence.operation_ref.is_empty() {
            return Err(invalid("semantic occurrence has an empty operation_ref"));
        }
        if !matches!(occurrence.kind.as_str(), "action" | "mutation" | "grant") {
            return Err(invalid(format!(
                "unknown semantic occurrence kind: {}",
                occurrence.kind
            )));
        }
        if let Some(previous) =
            unique.insert(occurrence.operation_ref.clone(), occurrence.kind.clone())
        {
            if previous != occurrence.kind {
                return Err(invalid(
                    "one operation_ref is assigned two semantic occurrence kinds",
                ));
            }
        }
    }
    Ok(SemanticCounts {
        actions: unique.values().filter(|kind| *kind == "action").count() as u64,
        mutations: unique.values().filter(|kind| *kind == "mutation").count() as u64,
        consumptions: unique.len() as u64,
        direct_children: unique.values().filter(|kind| *kind == "grant").count() as u64,
    })
}

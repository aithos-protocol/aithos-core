//! Revocation (spec §06): the active-revocation set reconstructed from the
//! gamma `revoke` entries and the certificates alone — no owner-signed
//! aggregate, no server. Pure: entries and certs come in, verdicts go out.

use crate::error::{Error, Result};
use crate::gamma::Entry;
use crate::mandate::{covers, Mandate, PerimeterEntry};

/// A parsed, authority-checked revocation fact (§06.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revocation {
    pub mandate_id: String,
    pub revoked_at: String,
}

/// Reconstruct the revocation set from the log (§06.5). Each `revoke` entry
/// must already carry a verified signature (owner or delegated) — that check
/// lives with entry verification; here we read the facts it asserts.
#[must_use]
pub fn revocations(entries: &[Entry]) -> Vec<Revocation> {
    entries
        .iter()
        .filter(|e| e.kind == "revoke")
        .filter_map(|e| {
            Some(Revocation {
                mandate_id: e.target.clone()?,
                revoked_at: e.at.clone(),
            })
        })
        .collect()
}

/// Is any mandate in `chain` revoked at time `at` (§06.4 forward-only)? A
/// revocation bites at `revoked_at ≤ at`; artifacts strictly before stay
/// attributable.
pub fn chain_revoked_at(chain: &[Mandate], revs: &[Revocation], at: &str) -> Result<()> {
    for m in chain {
        for r in revs {
            if r.mandate_id == m.id && at >= r.revoked_at.as_str() {
                return Err(Error::MandateRevoked(format!(
                    "{} revoked at {}",
                    m.id, r.revoked_at
                )));
            }
        }
    }
    Ok(())
}

/// Authority to revoke `target` (§06.4): the signer is the owner, the
/// target's issuer, a transitive ancestor, or a watchdog whose `revoke`
/// perimeter covers the target's. `revoker_chain` is the (already verified)
/// certificate chain the entry travels with; `None` = owner-signed.
///
/// `target_chain` is the revoked mandate's own certificate chain (root
/// first) — needed to read its issuer and ancestry and its perimeter.
pub fn check_revoke_authority(
    revoker_chain: Option<&[Mandate]>,
    target_chain: &[Mandate],
) -> Result<()> {
    let reject = |m: &str| Error::GammaRevocationRejected(m.to_owned());
    let target = target_chain
        .last()
        .ok_or_else(|| reject("empty target chain"))?;

    // Owner-signed: the universal ancestor. Always authorized.
    let Some(revoker_chain) = revoker_chain else {
        return Ok(());
    };
    let revoker = revoker_chain
        .last()
        .ok_or_else(|| reject("empty revoker chain"))?;

    // Watchdog: a `revoke` perimeter entry covering the target's perimeter.
    let revoker_perimeter = revoker.parsed_perimeter()?;
    let has_revoke_right = revoker_perimeter
        .iter()
        .any(|e| matches!(e, PerimeterEntry::Revoke { .. }));
    if has_revoke_right && revoke_covers(&revoker_perimeter, target)? {
        return Ok(());
    }

    // Issuer: the revoker's leaf key minted the target directly.
    if target.issued_by == revoker.grantee.pubkey {
        return Ok(());
    }

    // Transitive ancestor: the revoker's leaf mandate id is in the target's
    // parent chain (everything before the target in its own chain).
    let ancestor_ids: Vec<&str> = target_chain
        .iter()
        .take(target_chain.len().saturating_sub(1))
        .map(|m| m.id.as_str())
        .collect();
    if ancestor_ids.contains(&revoker.id.as_str()) {
        return Ok(());
    }

    Err(reject(&format!(
        "{} may not revoke {}",
        revoker.id, target.id
    )))
}

/// Does any `revoke` entry of the revoker's perimeter cover the target's
/// ethos perimeter? (§06.7 attenuation — a watchdog revokes only what its
/// issuer could.)
fn revoke_covers(revoker_perimeter: &[PerimeterEntry], target: &Mandate) -> Result<bool> {
    let target_perimeter = target.parsed_perimeter()?;
    // Every ethos entry of the target must be covered by some revoke scope.
    for te in &target_perimeter {
        let ethos_like = matches!(
            te,
            PerimeterEntry::Ethos { .. } | PerimeterEntry::Act { .. }
        );
        if !ethos_like {
            continue;
        }
        let covered = revoker_perimeter.iter().any(|re| match re {
            // A bare `revoke` covers the issuer's whole revocable scope;
            // a scoped `revoke.<zone>#…` covers by the same lattice as reads.
            PerimeterEntry::Revoke { scope: None } => true,
            PerimeterEntry::Revoke { scope: Some(s) } => covers(s, te),
            _ => false,
        });
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}

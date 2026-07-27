//! A.4 deposit verification + A.5 CAS — « le serveur vérifie avant
//! d'accepter », en COMPOSANT les primitives core/bundle, sans recopier
//! une règle.
//!
//! Three deposits land with étape 4:
//! - [`deposit_manifest`] — PUT `manifest.json` + `If-Head` (publish, CAS
//!   obligatoire) : `Manifest::verify_form`, signature racine
//!   (`Manifest::verify_signature` sous le did.json stocké) ou déléguée
//!   (certs stockés + `mandate::verify_chain` à `edition.created_at` +
//!   `Manifest::verify_delegate_signature`), `height == stocké + 1`,
//!   `prev_hash == manifest_chain_hash` de la TABLE (A.5 — jamais un
//!   re-hash des octets stockés) ;
//! - [`deposit_gamma`] — POST `/gamma` + `If-Head` (une entrée) :
//!   `Entry::check_form` (parse strict, kinds du registre, `prevs`
//!   seulement sur merge), signature d'entrée déléguée à core
//!   (`gamma::verify_owner_entry` / `gamma::verify_delegated_entry` — ce
//!   dernier vérifie la chaîne à `entry.at` ET la couverture de l'action
//!   affichée), `prev == tête stockée`, append au segment UTC du mois
//!   d'`entry.at` ;
//! - [`deposit_cert`] — PUT `certs/<id>.json` : `id` == nom de fichier,
//!   `subject == <did>`, chaîne parente résolue AU DÉPÔT depuis les certs
//!   stockés et vérifiée par `mandate::verify_chain` à `now_serveur`
//!   (« valide au moment du dépôt », A.4).
//!
//! Every refusal is `artifact_invalid` + a `reason` from the CLOSED
//! registry below (A.7), or the CAS answers (`cas_required`,
//! `cas_mismatch` + tête courante). The server never repairs, never
//! rewrites, never arbitrates a fork — the CAS serializes, the losers
//! rebase (§02.6), the witness observes (annexe C).

use aithos_bundle::manifest::Manifest;
use aithos_core::did::DidDocument;
use aithos_core::gamma::Entry;
use aithos_core::mandate::Mandate;

use crate::envelope::Refusal;
use crate::heads::{HeadsRecord, HeadsTable};
use crate::objects::{ObjectStore, PutOnce};

/// The closed `reason` register carried by `artifact_invalid` (A.7 — a
/// short closed word, never free text, never an excerpt).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactReason {
    /// Parse, JCS canonicality, or profile form (draft.1/draft.2 carrier
    /// discipline, unknown version, entry form).
    Form,
    /// The artifact's own signature does not verify under its resolved key.
    Signature,
    /// The displayed authority chain cannot be resolved from the stored
    /// certs, does not match the presenting envelope, or fails core's
    /// chain verification.
    Chain,
    /// `edition.height`/`edition.prev_hash` do not chain the STORED head
    /// tuple (A.5 table).
    PrevHashMismatch,
    /// Cert `id` differs from the deposit filename.
    IdMismatch,
    /// Cert `subject` differs from the `<did>` of the path.
    SubjectMismatch,
    /// The gamma entry verification refused (signature or entry-level
    /// authority, delegated to core).
    EntrySignature,
    /// The gamma entry's `prev` does not pin the stored head.
    PrevMismatch,
    /// A segment replica whose content does not byte-preserve the stored
    /// segment as a prefix — a replica never rewrites history (A.4/A.5;
    /// the ONE reason added by the gate-contrat-5 arbitrage ④).
    PrefixMismatch,
    /// A byte-different deposit under a stored immutable id (certs,
    /// changesets, evidence) — the ⑧b write-once (étape 6): an identical
    /// re-deposit is idempotent, different bytes never rewrite history.
    /// Micro-redline A.7 carried to the étape-6 gate.
    ImmutableConflict,
}

impl ArtifactReason {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            ArtifactReason::Form => "form",
            ArtifactReason::Signature => "signature",
            ArtifactReason::Chain => "chain",
            ArtifactReason::PrevHashMismatch => "prev_hash_mismatch",
            ArtifactReason::IdMismatch => "id_mismatch",
            ArtifactReason::SubjectMismatch => "subject_mismatch",
            ArtifactReason::EntrySignature => "entry_signature",
            ArtifactReason::PrevMismatch => "prev_mismatch",
            ArtifactReason::PrefixMismatch => "prefix_mismatch",
            ArtifactReason::ImmutableConflict => "immutable_conflict",
        }
    }
}

/// How a deposit refuses: a plain registry code, the CAS answer with the
/// current truth, or `artifact_invalid` + closed reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepositRefusal {
    Plain(Refusal),
    /// `409` + the CURRENT head (`"none"` when nothing is stored);
    /// `height` on the manifest head only (A.7).
    CasMismatch {
        head: String,
        height: Option<u64>,
    },
    Artifact(ArtifactReason),
}

/// An accepted publish: the new head (`sha256:<hex>`) and height — the
/// values `/heads` will serve (étape 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestAccepted {
    pub head: String,
    pub height: u64,
}

/// An accepted append: the new gamma head (`sha256:<hex>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GammaAccepted {
    pub head: String,
}

/// `If-Head` grammar (A.5): `none` or `sha256:<64 hex minuscule>`.
/// Anything else can never equal a stored head — it answers the same
/// `cas_mismatch` as any stale head (fail-closed, no third grammar).
fn if_head_is_wellformed(value: &str) -> bool {
    value == "none"
        || value.strip_prefix("sha256:").is_some_and(|h| {
            h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

fn utf8_canonical(body: &[u8]) -> Result<&str, DepositRefusal> {
    core::str::from_utf8(body).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))
}

async fn stored_did_doc(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    fault: ArtifactReason,
) -> Result<DidDocument, DepositRefusal> {
    let bytes = objects
        .get(tenant, did, "did.json")
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
        .ok_or(DepositRefusal::Artifact(fault))?;
    serde_json::from_slice(&bytes).map_err(|_| DepositRefusal::Artifact(fault))
}

/// Load the chain the ids name, every link a STORED `certs/<id>.json`
/// (§04.9 world-readable artifacts) — any gap is a chain fault. Returns
/// the typed links AND their exact stored bytes (the draft.3 signature
/// check verifies the bytes, never a re-serialization).
async fn stored_chain(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    ids: &[String],
) -> Result<(Vec<Mandate>, Vec<Vec<u8>>), DepositRefusal> {
    let fault = DepositRefusal::Artifact(ArtifactReason::Chain);
    if ids.is_empty() {
        return Err(fault);
    }
    let mut chain = Vec::with_capacity(ids.len());
    let mut raws = Vec::with_capacity(ids.len());
    for id in ids {
        if !crate::pathmap::mandate_id_is_valid(id) {
            return Err(fault);
        }
        let bytes = objects
            .get(tenant, did, &format!("certs/{id}.json"))
            .await
            .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
            .ok_or_else(|| fault.clone())?;
        let mandate: Mandate = serde_json::from_slice(&bytes).map_err(|_| fault.clone())?;
        chain.push(mandate);
        raws.push(bytes);
    }
    Ok((chain, raws))
}

/// Chain verification COMPOSED for the wire checks (#9 and the A.4
/// deposits): core's typed `verify_chain` (§04.5 — link identity,
/// signatures, windows at `at`, kex, attenuation §05.3) wherever the
/// chain speaks a typed profile (draft.1/draft.2). A homogeneous
/// **draft.3 (K1-C)** chain is outside the typed grammar (core parses its
/// perimeter/constraint vocabulary only through the carrier machinery,
/// whose Value-level authority verifier is not exported): for those, the
/// A.4-LITERAL link checks — exactly the frozen p7 oracle's, plus the
/// §04.5 window at `at` — run here: contiguity root→leaf, constant
/// `subject == <did>`, root link `issued_by == <did>#root` signed under
/// the DID literal (`did_doc.keys.root` IS the literal, §01.4), each
/// sub-link signed by its parent's grantee key, signature verified over
/// the exact STORED bytes with `signature.value = ""`. Draft.3
/// kex/attenuation stay with the K1-C verifiers (client side, §3.1);
/// exporting core's own Value-level authority verifier so this fallback
/// disappears is an arbitrage of the étape-4 gate.
pub(crate) fn verify_chain_composed(
    chain: &[Mandate],
    raws: &[Vec<u8>],
    did_doc: &aithos_core::did::DidDocument,
    at: &str,
) -> Result<(), ()> {
    match aithos_core::mandate::verify_chain(chain, did_doc, at) {
        Ok(()) => return Ok(()),
        Err(_)
            if chain
                .iter()
                .all(|m| m.version == aithos_core::mandate::MANDATE_VERSION_DRAFT3) => {}
        Err(_) => return Err(()),
    }
    // Homogeneous draft.3 — the A.4-literal path.
    if chain.len() != raws.len() || chain.is_empty() {
        return Err(());
    }
    did_doc.verify().map_err(|_| ())?;
    let root_label = format!("{}#root", did_doc.id);
    for (i, (mandate, raw)) in chain.iter().zip(raws).enumerate() {
        // Window at `at` (§04.5 step 3): RFC 3339 Z compares as bytes.
        if crate::time::parse_rfc3339z_ms(&mandate.not_before).is_none()
            || crate::time::parse_rfc3339z_ms(&mandate.not_after).is_none()
            || at < mandate.not_before.as_str()
            || at > mandate.not_after.as_str()
        {
            return Err(());
        }
        if mandate.subject != did_doc.id {
            return Err(());
        }
        let (signer_multibase, expected_label) = if i == 0 {
            if mandate.parent.is_some() || mandate.issued_by != root_label {
                return Err(());
            }
            (did_doc.keys.root.clone(), "#root".to_owned())
        } else {
            let parent = &chain[i - 1];
            if mandate.parent.as_deref() != Some(parent.id.as_str())
                || mandate.issued_by != parent.grantee.pubkey
                || mandate.grantee.pubkey == mandate.issued_by
            {
                return Err(());
            }
            (parent.grantee.pubkey.clone(), parent.grantee.pubkey.clone())
        };
        verify_blank_value_signature(raw, &signer_multibase, &expected_label)?;
    }
    Ok(())
}

/// Ed25519 over the JCS of the EXACT stored document with
/// `signature.value = ""` — the shared §01.4 convention, applied to the
/// raw bytes (a Value round-trip preserves every member; nothing is
/// retyped, nothing dropped).
fn verify_blank_value_signature(
    raw: &[u8],
    signer_multibase: &str,
    expected_label: &str,
) -> Result<(), ()> {
    use ed25519_dalek::Verifier as _;
    let mut doc: serde_json::Value = serde_json::from_slice(raw).map_err(|_| ())?;
    let signature = doc.get("signature").and_then(|s| s.as_object()).ok_or(())?;
    if signature.get("alg").and_then(|v| v.as_str()) != Some("ed25519")
        || signature.get("key").and_then(|v| v.as_str()) != Some(expected_label)
    {
        return Err(());
    }
    let value_hex = signature
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or(())?
        .to_owned();
    doc["signature"]["value"] = serde_json::Value::String(String::new());
    let unsigned = serde_jcs::to_string(&doc).map_err(|_| ())?;
    let key_bytes =
        aithos_core::wire::multibase_to_ed25519_pub(signer_multibase).map_err(|_| ())?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&key_bytes).map_err(|_| ())?;
    let sig_bytes = hex::decode(&value_hex)
        .ok()
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .ok_or(())?;
    key.verify(
        unsigned.as_bytes(),
        &ed25519_dalek::Signature::from_bytes(&sig_bytes),
    )
    .map_err(|_| ())
}

// ------------------------------------------------------------- manifest

/// PUT `manifest.json` + `If-Head` — the A.4/A.5 publish order, verbatim
/// from the committed p7 oracle: CAS grammar first, then form, then
/// signature (root or delegated), then `height`/`prev_hash` against the
/// TABLE. `presented_chain` is the envelope principal's verified chain
/// (`None` = owner fragment): a delegated manifest must be deposited by
/// its own actor (§02.6.1 — one actor, one chain).
///
/// On accept the head swaps FIRST (the CAS is the serialization point —
/// a loser writes nothing), then the object persists byte-preserved at
/// `manifest.json` and `manifests/<height>.json` (the edition-history
/// key of the bundle layout; serving it needs the A.1 draft.2 redline).
pub async fn deposit_manifest(
    objects: &dyn ObjectStore,
    heads: &dyn HeadsTable,
    tenant: &str,
    did: &str,
    presented_chain: Option<&[Mandate]>,
    if_head: Option<&str>,
    body: &[u8],
) -> Result<ManifestAccepted, DepositRefusal> {
    // CAS grammar (A.5): required, then compared to the TABLE.
    let Some(if_head) = if_head else {
        return Err(DepositRefusal::Plain(Refusal::CasRequired));
    };
    let record = heads
        .read(tenant, did)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    let stored = record.clone().unwrap_or_default();
    let current_head = if stored.manifest_chain_hash.is_empty() {
        "none".to_owned()
    } else {
        format!("sha256:{}", stored.manifest_chain_hash)
    };
    if !if_head_is_wellformed(if_head) || if_head != current_head {
        return Err(DepositRefusal::CasMismatch {
            head: current_head,
            height: Some(stored.height),
        });
    }

    // Form (A.4): parse, JCS canonicality (A.1 — tout JSON signé est du
    // JCS), profile + carrier discipline (bundle's own rule).
    let text = utf8_canonical(body)?;
    let manifest: Manifest =
        serde_json::from_str(text).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    if serde_jcs::to_string(&manifest)
        .map(|canonical| canonical != text)
        .unwrap_or(true)
    {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    manifest
        .verify_form()
        .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;

    // Signature (A.4): root under the STORED did.json, or delegated —
    // stored certs + core `verify_chain` at the edition's own instant +
    // the bundle's delegate-signature check. No rule recopied.
    if manifest.signature.key == "#root" {
        let did_doc = stored_did_doc(objects, tenant, did, ArtifactReason::Signature).await?;
        manifest
            .verify_signature(&did_doc)
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Signature))?;
    } else {
        let chain_fault = DepositRefusal::Artifact(ArtifactReason::Chain);
        // One actor, one chain (§02.6.1): the depositor IS the edition's
        // delegate — the presenting envelope chain must be the manifest's.
        let presented: Vec<&str> = presented_chain
            .unwrap_or(&[])
            .iter()
            .map(|m| m.id.as_str())
            .collect();
        let displayed: Vec<&str> = manifest.authorized_via.iter().map(String::as_str).collect();
        if displayed.is_empty() || presented != displayed {
            return Err(chain_fault);
        }
        let (chain, raws) = stored_chain(objects, tenant, did, &manifest.authorized_via).await?;
        let did_doc = stored_did_doc(objects, tenant, did, ArtifactReason::Chain).await?;
        verify_chain_composed(&chain, &raws, &did_doc, &manifest.edition.created_at)
            .map_err(|()| chain_fault.clone())?;
        let leaf = chain.last().expect("stored_chain is non-empty");
        if leaf.grantee.pubkey != manifest.signature.key {
            return Err(chain_fault);
        }
        manifest
            .verify_delegate_signature(leaf)
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Signature))?;
    }

    // Height and prev chain the STORED TUPLE (A.5) — never a re-hash of
    // stored bytes. `merges`/`resolves_fork` are accepted AS-IS: the
    // store does not arbitrate (the letter-of-A.4 merge tension is
    // arbitrage ① of the gate; the committed vector fixes the compatible
    // case, `prev_hash == stored`).
    if manifest.edition.height != stored.height + 1
        || manifest.edition.prev_hash != stored.manifest_chain_hash
    {
        return Err(DepositRefusal::Artifact(ArtifactReason::PrevHashMismatch));
    }

    // Commit: swap the head (the serialization point), then persist the
    // exact bytes. A raced twin loses the swap and writes NOTHING.
    let new_hash = manifest
        .chain_hash()
        .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    let next = HeadsRecord {
        height: manifest.edition.height,
        manifest_chain_hash: new_hash.clone(),
        ..stored
    };
    if let Err(current) = heads
        .cas(tenant, did, record.as_ref(), next)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        let current = current.unwrap_or_default();
        return Err(DepositRefusal::CasMismatch {
            head: if current.manifest_chain_hash.is_empty() {
                "none".to_owned()
            } else {
                format!("sha256:{}", current.manifest_chain_hash)
            },
            height: Some(current.height),
        });
    }
    objects
        .put(tenant, did, "manifest.json", body.to_vec())
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    objects
        .put(
            tenant,
            did,
            &format!("manifests/{}.json", manifest.edition.height),
            body.to_vec(),
        )
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    Ok(ManifestAccepted {
        head: format!("sha256:{new_hash}"),
        height: manifest.edition.height,
    })
}

// ---------------------------------------------------------------- gamma

/// One gamma entry, verified as A.4 writes it: parse strict + JCS
/// canonicality + `Entry::check_form` (core's rule: kinds of the closed
/// register, `prevs` only on merge), then signature and entry-level
/// authority DELEGATED to core — owner entries via `verify_owner_entry`
/// (§07.2 — `#content`), delegated entries via `verify_delegated_entry`
/// (chain == the displayed `authorized_via`, verified at `entry.at`, leaf
/// signature, coverage of the displayed action). The chain's links are
/// the STORED certs. Shared by the one-entry append (POST `/gamma`) and
/// the segment replica (PUT, mode A) — the same verification, per entry.
async fn verify_gamma_entry_text(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    text: &str,
) -> Result<Entry, DepositRefusal> {
    let entry: Entry =
        serde_json::from_str(text).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    if serde_jcs::to_string(&entry)
        .map(|canonical| canonical != text)
        .unwrap_or(true)
    {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    entry
        .check_form()
        .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    let did_doc = stored_did_doc(objects, tenant, did, ArtifactReason::EntrySignature).await?;
    if let Some(via) = &entry.authorized_via {
        let (chain, _raws) = stored_chain(objects, tenant, did, via).await?;
        aithos_core::gamma::verify_delegated_entry(&entry, &chain, &did_doc)
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::EntrySignature))?;
    } else {
        aithos_core::gamma::verify_owner_entry(&entry, &did_doc)
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::EntrySignature))?;
    }
    Ok(entry)
}

/// POST `/gamma` + `If-Head` (une entrée) — CAS on the gamma head, entry
/// verification DELEGATED to core, append to the UTC month segment of
/// `entry.at`, transactional head advance.
pub async fn deposit_gamma(
    objects: &dyn ObjectStore,
    heads: &dyn HeadsTable,
    tenant: &str,
    did: &str,
    if_head: Option<&str>,
    body: &[u8],
) -> Result<GammaAccepted, DepositRefusal> {
    let Some(if_head) = if_head else {
        return Err(DepositRefusal::Plain(Refusal::CasRequired));
    };
    let record = heads
        .read(tenant, did)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    let stored = record.clone().unwrap_or_default();
    let current_head = if stored.gamma_head.is_empty() {
        "none".to_owned()
    } else {
        stored.gamma_head.clone()
    };
    if !if_head_is_wellformed(if_head) || if_head != current_head {
        return Err(DepositRefusal::CasMismatch {
            head: current_head,
            height: None,
        });
    }

    let entry = verify_gamma_entry_text(objects, tenant, did, utf8_canonical(body)?).await?;

    // `prev == tête stockée` (A.4) — the entry chains the TABLE.
    if entry.prev != stored.gamma_head {
        return Err(DepositRefusal::Artifact(ArtifactReason::PrevMismatch));
    }

    // Commit: swap the head, then append the exact line to the UTC month
    // segment of `entry.at` (RFC 3339 Z — the month is its prefix).
    let month = entry.at.get(..7).unwrap_or_default().to_owned();
    let segment_key = format!("gamma/{month}.jsonl");
    let new_head = entry
        .chain_hash()
        .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    let mut segments = stored.gamma_segments.clone();
    if !segments.contains(&month) {
        segments.push(month.clone());
        segments.sort();
    }
    let next = HeadsRecord {
        gamma_head: new_head.clone(),
        gamma_segment: month,
        gamma_segments: segments,
        ..stored
    };
    if let Err(current) = heads
        .cas(tenant, did, record.as_ref(), next)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        let current = current.unwrap_or_default();
        return Err(DepositRefusal::CasMismatch {
            head: if current.gamma_head.is_empty() {
                "none".to_owned()
            } else {
                current.gamma_head
            },
            height: None,
        });
    }
    let mut segment = objects
        .get(tenant, did, &segment_key)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
        .unwrap_or_default();
    segment.extend_from_slice(body);
    segment.push(b'\n');
    objects
        .put(tenant, did, &segment_key, segment)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    Ok(GammaAccepted { head: new_head })
}

// ---------------------------------------------------------------- certs

/// PUT `certs/<id>.json` — `id` == filename, `subject == <did>`, parent
/// chain resolved from the STORED certs and verified by core at
/// `now_serveur` (« résoluble et valide au moment du dépôt », A.4). The
/// deposited link is the leaf of the chain it closes.
pub async fn deposit_cert(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    cert_id: &str,
    now_zulu: &str,
    body: &[u8],
) -> Result<(), DepositRefusal> {
    let text = utf8_canonical(body)?;
    let mandate: Mandate =
        serde_json::from_str(text).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    let canonical_value: serde_json::Value =
        serde_json::from_str(text).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    if serde_jcs::to_string(&canonical_value)
        .map(|canonical| canonical != text)
        .unwrap_or(true)
    {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    if mandate.id != cert_id {
        return Err(DepositRefusal::Artifact(ArtifactReason::IdMismatch));
    }
    if mandate.subject != did {
        return Err(DepositRefusal::Artifact(ArtifactReason::SubjectMismatch));
    }

    // Resolve the parent chain from the STORED certs, root first. A
    // missing or malformed link is a chain fault; the depth bound is the
    // anti-abuse guard (a legitimate chain is bounded by `issue#depth`,
    // core's rule — this bound only stops storage-walk abuse).
    let chain_fault = DepositRefusal::Artifact(ArtifactReason::Chain);
    let single_link = mandate.parent.is_none();
    let mut chain = vec![mandate];
    let mut raws = vec![body.to_vec()];
    let mut parent_id = chain[0].parent.clone();
    let mut depth = 0usize;
    while let Some(id) = parent_id {
        depth += 1;
        if depth > 16 || !crate::pathmap::mandate_id_is_valid(&id) {
            return Err(chain_fault);
        }
        let bytes = objects
            .get(tenant, did, &format!("certs/{id}.json"))
            .await
            .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
            .ok_or_else(|| chain_fault.clone())?;
        let parent: Mandate = serde_json::from_slice(&bytes).map_err(|_| chain_fault.clone())?;
        parent_id = parent.parent.clone();
        chain.insert(0, parent);
        raws.insert(0, bytes);
    }

    // The whole chain — parents AND the deposited link — verifies at the
    // deposit instant (« valide au moment du dépôt », A.4), through the
    // same composed check as every other chain on this surface.
    let did_doc = stored_did_doc(objects, tenant, did, ArtifactReason::Signature).await?;
    verify_chain_composed(&chain, &raws, &did_doc, now_zulu).map_err(|()| {
        // A chain of one is its own root link: the only thing that can
        // fail is the deposited artifact itself (`signature`); a longer
        // chain names its parents (`chain`).
        if single_link {
            DepositRefusal::Artifact(ArtifactReason::Signature)
        } else {
            chain_fault.clone()
        }
    })?;
    match objects
        .put_once(tenant, did, &format!("certs/{cert_id}.json"), body.to_vec())
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        // ⑧b: first write wins, an identical re-deposit is idempotent,
        // a byte-different stored object is never rewritten.
        PutOnce::Stored | PutOnce::Identical => Ok(()),
        PutOnce::Conflict => Err(DepositRefusal::Artifact(ArtifactReason::ImmutableConflict)),
    }
}

// ------------------------------------------------------------- did.json

/// PUT `did.json` (étape 5, A.4): **genesis** — the first document of a
/// bound DID, accepted when it parses, `id == <did>` of the path
/// (== multibase of its own root, core's rule) and it self-verifies
/// (`DidDocument::verify`, §01.4) — the ENVELOPE was already resolved
/// against this same deposited document (the A.2 #7 genesis exception).
/// **Replacement** — the successor document (same id) verifies under the
/// STORED document's `succession` key (`#succession` — a stolen root can
/// never steal the identity's future, §01.4). Interim reading acted at
/// the gate contrat 5: the §10.4 epoch-artifact (`next_did`) question
/// stays a named arbitrage.
pub async fn deposit_did(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    body: &[u8],
) -> Result<(), DepositRefusal> {
    let text = utf8_canonical(body)?;
    let doc: DidDocument =
        serde_json::from_str(text).map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    if serde_jcs::to_string(&doc)
        .map(|canonical| canonical != text)
        .unwrap_or(true)
    {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    if doc.id != did {
        return Err(DepositRefusal::Artifact(ArtifactReason::IdMismatch));
    }
    match objects
        .get(tenant, did, "did.json")
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        // Genesis: id ↔ root binding + auto-signature, core's own check.
        None => doc
            .verify()
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Signature))?,
        // Replacement: only the stored succession key authorizes it.
        Some(stored_bytes) if stored_bytes == body => return Ok(()),
        Some(stored_bytes) => {
            let stored: DidDocument = serde_json::from_slice(&stored_bytes)
                .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
            // The successor's keys must still be well-formed material.
            for key in [&doc.keys.root, &doc.keys.content, &doc.keys.succession] {
                aithos_core::wire::multibase_to_ed25519_pub(key)
                    .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
            }
            verify_blank_value_signature(body, &stored.keys.succession, "#succession")
                .map_err(|()| DepositRefusal::Artifact(ArtifactReason::Signature))?;
        }
    }
    objects
        .put(tenant, did, "did.json", body.to_vec())
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    Ok(())
}

// -------------------------------------------------------------- replica

/// PUT `gamma/<YYYY-MM>.jsonl` + `If-Head` — the mode-A segment replica
/// (A.4/A.5): the stored segment must be a byte-exact PREFIX of the new
/// content (a replica never rewrites history — `prefix_mismatch`), every
/// ADDED entry is verified exactly like a POST `/gamma` entry and must
/// chain the running head, and the segment head follows the same CAS
/// rule. Commit order unchanged: CAS first (a loser writes nothing),
/// then the full byte-preserved segment persists.
pub async fn deposit_replica(
    objects: &dyn ObjectStore,
    heads: &dyn HeadsTable,
    tenant: &str,
    did: &str,
    month: &str,
    if_head: Option<&str>,
    body: &[u8],
) -> Result<GammaAccepted, DepositRefusal> {
    let Some(if_head) = if_head else {
        return Err(DepositRefusal::Plain(Refusal::CasRequired));
    };
    let record = heads
        .read(tenant, did)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    let stored = record.clone().unwrap_or_default();
    let current_head = if stored.gamma_head.is_empty() {
        "none".to_owned()
    } else {
        stored.gamma_head.clone()
    };
    if !if_head_is_wellformed(if_head) || if_head != current_head {
        return Err(DepositRefusal::CasMismatch {
            head: current_head,
            height: None,
        });
    }

    // Byte-exact prefix rule: the stored content never rewrites.
    let text = utf8_canonical(body)?;
    let segment_key = format!("gamma/{month}.jsonl");
    let stored_segment = objects
        .get(tenant, did, &segment_key)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
        .unwrap_or_default();
    let stored_text = core::str::from_utf8(&stored_segment)
        .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
    let Some(added) = text.strip_prefix(stored_text) else {
        return Err(DepositRefusal::Artifact(ArtifactReason::PrefixMismatch));
    };

    // Every added line is one JCS entry + '\n', verified like an append
    // and chaining the running head.
    if !added.is_empty() && !added.ends_with('\n') {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    let mut running_head = stored.gamma_head.clone();
    let mut verified = 0u32;
    for line in added.split_terminator('\n') {
        if line.is_empty() {
            return Err(DepositRefusal::Artifact(ArtifactReason::Form));
        }
        let entry = verify_gamma_entry_text(objects, tenant, did, line).await?;
        if entry.prev != running_head {
            return Err(DepositRefusal::Artifact(ArtifactReason::PrevMismatch));
        }
        if !entry.at.starts_with(month) {
            // A replica only carries its own month's entries (A.3).
            return Err(DepositRefusal::Artifact(ArtifactReason::Form));
        }
        running_head = entry
            .chain_hash()
            .map_err(|_| DepositRefusal::Artifact(ArtifactReason::Form))?;
        verified += 1;
    }

    // Idempotent replica (no new entry): nothing to swap, nothing to
    // rewrite — the current head is the answer.
    if verified == 0 {
        return Ok(GammaAccepted { head: current_head });
    }

    let mut segments = stored.gamma_segments.clone();
    if !segments.contains(&month.to_owned()) {
        segments.push(month.to_owned());
        segments.sort();
    }
    let next = HeadsRecord {
        gamma_head: running_head.clone(),
        gamma_segment: month.to_owned(),
        gamma_segments: segments,
        ..stored
    };
    if let Err(current) = heads
        .cas(tenant, did, record.as_ref(), next)
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        let current = current.unwrap_or_default();
        return Err(DepositRefusal::CasMismatch {
            head: if current.gamma_head.is_empty() {
                "none".to_owned()
            } else {
                current.gamma_head
            },
            height: None,
        });
    }
    objects
        .put(tenant, did, &segment_key, body.to_vec())
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?;
    Ok(GammaAccepted { head: running_head })
}

// ------------------------------------------------------------- sidecars

/// Which K1-C sidecar directory a deposit names (§02.6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarKind {
    Changeset,
    Evidence,
}

impl SidecarKind {
    fn domain(self) -> &'static str {
        match self {
            SidecarKind::Changeset => "aithos-core/v1/changeset",
            SidecarKind::Evidence => "aithos-core/v1/evidence",
        }
    }
}

/// PUT `changesets/<64hex>.json` / `evidence/<64hex>.json` (redline gate
/// 5): light form (JSON parsable) **+ content addressing** — the filename
/// must equal the K1-C digest recomputed on the deposited bytes
/// (`C(domain, bytes)`, §02.6.3/§04.5.1) or the deposit refuses
/// `id_mismatch`. NO semantic verification: the changeset/evidence/
/// manifest coherence is the verifier's (K1-B), never the store's.
pub async fn deposit_sidecar(
    objects: &dyn ObjectStore,
    tenant: &str,
    did: &str,
    kind: SidecarKind,
    hash: &str,
    body: &[u8],
) -> Result<(), DepositRefusal> {
    if serde_json::from_slice::<serde_json::Value>(body).is_err() {
        return Err(DepositRefusal::Artifact(ArtifactReason::Form));
    }
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(kind.domain().as_bytes());
    hasher.update([0u8]);
    hasher.update(body);
    if hex::encode(hasher.finalize()) != hash {
        return Err(DepositRefusal::Artifact(ArtifactReason::IdMismatch));
    }
    let dir = match kind {
        SidecarKind::Changeset => "changesets",
        SidecarKind::Evidence => "evidence",
    };
    match objects
        .put_once(tenant, did, &format!("{dir}/{hash}.json"), body.to_vec())
        .await
        .map_err(|_| DepositRefusal::Plain(Refusal::Unavailable))?
    {
        // ⑧b — content-addressing makes an honest conflict unreachable
        // (a different body is a different digest); whatever is stored
        // under an immutable name still never changes.
        PutOnce::Stored | PutOnce::Identical => Ok(()),
        PutOnce::Conflict => Err(DepositRefusal::Artifact(ArtifactReason::ImmutableConflict)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_register_is_closed_and_short() {
        for (reason, code) in [
            (ArtifactReason::Form, "form"),
            (ArtifactReason::Signature, "signature"),
            (ArtifactReason::Chain, "chain"),
            (ArtifactReason::PrevHashMismatch, "prev_hash_mismatch"),
            (ArtifactReason::IdMismatch, "id_mismatch"),
            (ArtifactReason::SubjectMismatch, "subject_mismatch"),
            (ArtifactReason::EntrySignature, "entry_signature"),
            (ArtifactReason::PrevMismatch, "prev_mismatch"),
            (ArtifactReason::PrefixMismatch, "prefix_mismatch"),
        ] {
            assert_eq!(reason.code(), code);
            assert!(code.len() <= 20 && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
        }
    }

    #[test]
    fn if_head_grammar_is_closed() {
        assert!(if_head_is_wellformed("none"));
        assert!(if_head_is_wellformed(&format!("sha256:{}", "a".repeat(64))));
        for bad in [
            "",
            "None",
            "sha256:",
            &format!("sha256:{}", "A".repeat(64)),
            &format!("sha256:{}", "a".repeat(63)),
            &format!("sha256:{}", "g".repeat(64)),
            "b3:abcd",
        ] {
            assert!(!if_head_is_wellformed(bad), "must reject: {bad}");
        }
    }
}

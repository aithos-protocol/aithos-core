//! Revocation over the bundle (spec §06): the revoke gamma entry, its
//! authority check against stored certificates, rotation of a folder (rung
//! 2) with re-encryption (rung 3) and the up-link wrap (§03.4 step 2bis),
//! and the revocation-aware read/act paths. The cryptographic cut lives
//! here; the pure verdicts live in aithos-core.

use crate::bundle::{Bundle, SectionRow, ZoneIndex, KV};
use crate::entropy::EntropySource;
use crate::grants::{hdr_file, wrap_file};
use crate::Store;
use aithos_core::derive::node_key;
use aithos_core::error::{Error, Result};
use aithos_core::gamma::{self, delegated_entry, owner_entry, Entry, EntrySpec, Kind};
use aithos_core::header::{Header, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::{ed2x, OwnerKeys};
use aithos_core::mandate::Mandate;
use aithos_core::path::{Leaf, NodePath, Zone};
use aithos_core::revocation::{check_revoke_authority, revocations, Revocation};
use aithos_core::wire;
use ed25519_dalek::{SigningKey, VerifyingKey};

impl<S: Store> Bundle<S> {
    // ------------------------------------------------------ the entry (rung 1)

    /// Owner revocation of a mandate (§06.4): one signed, anchored entry.
    pub fn log_revoke_owner(
        &mut self,
        owner: &OwnerKeys,
        mandate_id: &str,
        reason: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let entry = owner_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: self.gamma_head()?,
                prevs: None,
                at: now.to_owned(),
                kind: Kind::Revoke,
                target: Some(mandate_id.to_owned()),
                payload: Some(serde_json::json!({ "reason": reason })),
                body_enc: None,
            },
            &owner.content_sign,
        )?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// Delegated revocation (§06.4, §06.7): a revoker (issuer, ancestor, or
    /// watchdog) publishes the entry under its own chain. Authority is
    /// checked against the target's stored certificate before the append.
    pub fn log_revoke_as(
        &mut self,
        revoker_chain: &[Mandate],
        revoker_sk: &SigningKey,
        target_mandate_id: &str,
        reason: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let target_chain = self.cert_chain(target_mandate_id)?;
        check_revoke_authority(Some(revoker_chain), &target_chain)?;
        let via: Vec<String> = revoker_chain.iter().map(|m| m.id.clone()).collect();
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: self.gamma_head()?,
                prevs: None,
                at: now.to_owned(),
                kind: Kind::Revoke,
                target: Some(target_mandate_id.to_owned()),
                payload: Some(serde_json::json!({ "reason": reason })),
                body_enc: None,
            },
            via,
            revoker_sk,
        )?;
        // Verify signature + chain (at the entry's own time), then append.
        gamma::verify_delegated_entry(&entry, revoker_chain, &self.did_doc()?)?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// The active revocation set (§06.5): every `revoke` entry whose signer
    /// has authority over its target. Unauthorized entries are dropped
    /// (an honest verifier never honors a forged revocation).
    pub fn active_revocations(&self) -> Result<Vec<Revocation>> {
        let entries = self.gamma_entries()?;
        let mut out = Vec::new();
        for r in revocations(&entries) {
            // Re-find the entry to read its signer/chain.
            let entry = entries
                .iter()
                .find(|e| e.kind == "revoke" && e.target.as_deref() == Some(&r.mandate_id))
                .expect("revocations() derived from these entries");
            let authorized = match &entry.authorized_via {
                None => true, // owner-signed (§07.2); its signature is checked by gamma_verify
                Some(via) => {
                    let revoker_chain: Result<Vec<Mandate>> =
                        via.iter().map(|id| self.cert(id)).collect();
                    match (revoker_chain, self.cert_chain(&r.mandate_id)) {
                        (Ok(rc), Ok(tc)) => check_revoke_authority(Some(&rc), &tc).is_ok(),
                        _ => false,
                    }
                }
            };
            if authorized {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Load one certificate by id.
    pub(crate) fn cert(&self, id: &str) -> Result<Mandate> {
        self.get_json(&format!("certs/{id}.json"))
    }

    /// The full certificate chain of a mandate (root first), walking parents.
    pub(crate) fn cert_chain(&self, id: &str) -> Result<Vec<Mandate>> {
        let mut chain = Vec::new();
        let mut cursor = Some(id.to_owned());
        while let Some(cur) = cursor {
            let m = self.cert(&cur)?;
            cursor = m.parent.clone();
            chain.push(m);
        }
        chain.reverse();
        Ok(chain)
    }

    // ---------------------------------------------------- rotation (rung 2/3)

    /// Rotate a circle folder out of a revoked recipient (§03.4, §06.2):
    /// fresh DK', new header version sealed to every survivor + owner, an
    /// up-link wrap under the zone root, and (rung 3) re-encryption of every
    /// section under the folder. `revoked_kid` is the grantee's multibase
    /// Ed25519 pubkey (its header routing id).
    pub fn rotate_folder(
        &mut self,
        owner: &OwnerKeys,
        display_folder: &str,
        revoked_kid: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let zone = Zone::Circle;
        let folders = self.resolve_folder(zone, display_folder)?;
        let node = NodePath::folder(zone, folders.clone());
        let file = hdr_file(zone, &node);

        // Current folder header and version.
        let mut header: Header = self.get_json(&file)?;
        let old_v = header.latest_version();
        let new_v = old_v + 1;

        // Survivors = current lines minus the revoked (owner always kept).
        let kv = header
            .key_versions
            .get(&old_v.to_string())
            .ok_or_else(|| Error::SealRejected("no current header version".to_owned()))?;
        let mut survivors: Vec<Recipient> = Vec::new();
        for line in &kv.lines {
            if line.kid == revoked_kid {
                continue;
            }
            if line.to == "owner" {
                survivors.push(self.owner_kex_recipient()?);
            } else {
                let ed = wire::multibase_to_ed25519_pub(&line.to)?;
                let vk = VerifyingKey::from_bytes(&ed)
                    .map_err(|_| Error::SealRejected("bad survivor key".to_owned()))?;
                survivors.push(Recipient {
                    to: line.to.clone(),
                    kid: line.kid.clone(),
                    pubkey: ed2x(&vk),
                });
            }
        }

        // Fresh DK' and the new sealed version.
        let new_dk = ent.e32();
        let eph: Vec<[u8; 32]> = survivors.iter().map(|_| ent.e32()).collect();
        let nonces: Vec<[u8; 24]> = survivors.iter().map(|_| ent.e24()).collect();
        header.rotate(&self.did.clone(), new_v, &new_dk, &survivors, &eph, &nonces)?;
        header.check_rotation(new_v)?; // fail-closed: no smuggled recipient
        self.put_json(&file, &header)?;

        // Up-link wrap (§03.4 step 2bis): seal DK' under the zone-root key so
        // holders of the zone keep reading the folder by derivation.
        let zone_dk = self.zone_dk(zone, owner)?;
        let wrap = Wrap::seal(
            &self.did,
            &NodePath::zone_root(zone).to_string(),
            &zone_dk,
            &node.to_string(),
            new_v,
            &new_dk,
            ent.e24(),
        );
        self.put_json(&wrap_file(zone, &NodePath::zone_root(zone), &node), &wrap)?;

        // Rung 3 — re-encrypt every section under the folder under DK'.
        for (row_folders, sid) in self.sections_under(zone, &folders, None)? {
            let section = NodePath::section(zone, row_folders.clone(), sid);
            // New section key derived from DK' along the path below the folder.
            let rest = NodePath {
                zone,
                folders: row_folders[folders.len()..].to_vec(),
                leaf: Leaf::Section(sid),
            };
            let new_key = node_key(&new_dk, &rest);
            // Open the old blob with the old key (derive from the old folder DK).
            let old_folder_dk = node_key(&zone_dk, &node);
            let old_rest = NodePath {
                zone,
                folders: row_folders[folders.len()..].to_vec(),
                leaf: Leaf::Section(sid),
            };
            let old_key = node_key(&old_folder_dk, &old_rest);
            let file = format!("e/circle/blobs/{sid}.enc");
            let pt = self.open_blob_v(&file, &old_key, &section, old_v_of(old_v))?;
            let sha = self.put_blob_v(&file, &new_key, &section, new_v, &pt, ent)?;
            self.bump_section_version(sid, new_v, &sha)?;
        }
        Ok(())
    }

    /// Set a circle section's key_version and blob hash after re-encryption.
    fn bump_section_version(&mut self, sid: Sid, version: u64, blob_sha: &str) -> Result<()> {
        let mut index: ZoneIndex = self.get_json("e/circle/index.json")?;
        for row in &mut index.sections {
            if row.sid == sid.to_string() {
                row.key_version = version;
                row.blob_sha = blob_sha.to_owned();
            }
        }
        self.put_json("e/circle/index.json", &index)
    }

    /// The owner's view of a folder's CURRENT key: the header's owner line
    /// at its latest version when the node was ever granted or rotated,
    /// plain derivation from the zone root otherwise. Returns `(version, dk)`.
    pub(crate) fn owner_folder_key_latest(
        &self,
        owner: &OwnerKeys,
        node: &NodePath,
    ) -> Result<(u64, [u8; 32])> {
        if let Some(bytes) = self.store.get(&hdr_file(node.zone, node)).ok().flatten() {
            if let Ok(header) = serde_json::from_slice::<Header>(&bytes) {
                let (v, dk) = header.open_latest(&self.did, "owner-kex", &owner.owner_kex)?;
                return Ok((v, dk));
            }
        }
        Ok((KV, node_key(&self.zone_dk(node.zone, owner)?, node)))
    }

    // ---------------------------------------------------- move (§02.9)

    /// Move a circle folder under a new parent (§02.9): move IS a rotation,
    /// because derivation cannot be un-taught. Re-parents the index row (the
    /// sid is stable — every label below M is unchanged), seals a fresh DK'
    /// at M's NEW canonical path to exactly the previous line holders (a move
    /// cuts nobody with a direct line — cutting is what revocation is for),
    /// posts the up-link wrap under the NEW parent, and re-encrypts M's
    /// subtree at the new version. Old-parent holders are cut by physics
    /// (fresh key) and by policy (§04.2 nodal containment) at once.
    /// Fail-closed: never the zone root, never into M's own subtree, never
    /// beside a same-named sibling, never a same-parent no-op.
    pub fn move_folder(
        &mut self,
        owner: &OwnerKeys,
        display_folder: &str,
        new_parent_display: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let zone = Zone::Circle;
        let folders_m = self.resolve_folder(zone, display_folder)?;
        let Some(&m_last) = folders_m.last() else {
            return Err(Error::InvalidPath("cannot move the zone root".to_owned()));
        };
        let new_parent: Vec<Sid> = if new_parent_display.split('/').all(str::is_empty) {
            vec![]
        } else {
            self.resolve_folder(zone, new_parent_display)?
        };
        if new_parent.len() >= folders_m.len() && new_parent[..folders_m.len()] == folders_m[..] {
            return Err(Error::InvalidPath(
                "cannot move a folder into its own subtree".to_owned(),
            ));
        }
        let m_sid = m_last.to_string();
        let new_parent_sid = new_parent.last().map(ToString::to_string);

        let mut index: ZoneIndex = self.get_json("e/circle/index.json")?;
        let row = index
            .folders
            .iter()
            .find(|f| f.sid == m_sid)
            .ok_or_else(|| Error::InvalidPath(format!("no folder row for {m_sid}")))?;
        if row.parent_sid == new_parent_sid {
            return Err(Error::InvalidPath(
                "folder already sits under that parent".to_owned(),
            ));
        }
        let m_name = row.name.clone();
        if index
            .folders
            .iter()
            .any(|f| f.sid != m_sid && f.parent_sid == new_parent_sid && f.name == m_name)
        {
            return Err(Error::InvalidPath(format!(
                "a folder named {m_name} already sits under the destination"
            )));
        }

        let old_node = NodePath::folder(zone, folders_m.clone());
        let mut new_chain = new_parent.clone();
        new_chain.push(m_last);
        let new_node = NodePath::folder(zone, new_chain.clone());

        // M's current key and its full current line set (owner always there).
        let (old_v, old_dk, survivors) =
            match self.store.get(&hdr_file(zone, &old_node)).ok().flatten() {
                Some(bytes) => {
                    let header: Header = serde_json::from_slice(&bytes)
                        .map_err(|e| Error::SealRejected(format!("old header: {e}")))?;
                    let v = header.latest_version();
                    let dk = header.open(&self.did, v, "owner-kex", &owner.owner_kex)?;
                    let kv = header
                        .key_versions
                        .get(&v.to_string())
                        .ok_or_else(|| Error::SealRejected("no current version".to_owned()))?;
                    let mut survivors = Vec::new();
                    for line in &kv.lines {
                        if line.to == "owner" {
                            survivors.push(self.owner_kex_recipient()?);
                        } else {
                            let ed = wire::multibase_to_ed25519_pub(&line.to)?;
                            let vk = VerifyingKey::from_bytes(&ed)
                                .map_err(|_| Error::SealRejected("bad survivor key".to_owned()))?;
                            survivors.push(Recipient {
                                to: line.to.clone(),
                                kid: line.kid.clone(),
                                pubkey: ed2x(&vk),
                            });
                        }
                    }
                    (v, dk, survivors)
                }
                None => (
                    KV,
                    node_key(&self.zone_dk(zone, owner)?, &old_node),
                    vec![self.owner_kex_recipient()?],
                ),
            };
        let new_v = old_v + 1;

        // Re-parent the row: the sid is stable, only the spine changes.
        for f in &mut index.folders {
            if f.sid == m_sid {
                f.parent_sid = new_parent_sid.clone();
            }
        }
        self.put_json("e/circle/index.json", &index)?;

        // Fresh DK' sealed at the NEW canonical path to exactly the old line
        // set (§03.4 discipline across files: nobody added, nobody dropped).
        // The old header file stays put — an immutable record of the
        // versions sealed at the old address.
        let new_dk = ent.e32();
        let eph: Vec<[u8; 32]> = survivors.iter().map(|_| ent.e32()).collect();
        let nonces: Vec<[u8; 24]> = survivors.iter().map(|_| ent.e24()).collect();
        let header_new = Header::build_at(
            &self.did,
            &new_node.to_string(),
            new_v,
            &new_dk,
            &survivors,
            &eph,
            &nonces,
        )?;
        self.put_json(&hdr_file(zone, &new_node), &header_new)?;

        // Up-link wrap under the NEW parent (§02.9): its holders — and any
        // ancestor deriving the parent's key — read M through the wrap.
        let parent_node = NodePath::folder(zone, new_parent.clone());
        let (_, parent_key) = self.owner_folder_key_latest(owner, &parent_node)?;
        let wrap = Wrap::seal(
            &self.did,
            &parent_node.to_string(),
            &parent_key,
            &new_node.to_string(),
            new_v,
            &new_dk,
            ent.e24(),
        );
        self.put_json(&wrap_file(zone, &parent_node, &new_node), &wrap)?;

        // Re-encrypt M's subtree at new_v bound to the new path (eager,
        // like rotate_folder). The index is already re-parented: rows come
        // back with their new chains; the old chain is the old spine + rest.
        let versions: std::collections::BTreeMap<String, u64> = index
            .sections
            .iter()
            .map(|r| (r.sid.clone(), r.key_version))
            .collect();
        for (row_folders, sid) in self.sections_under(zone, &new_chain, None)? {
            let rest: Vec<Sid> = row_folders[new_chain.len()..].to_vec();
            let below = |leaf_folders: &[Sid]| NodePath {
                zone,
                folders: leaf_folders.to_vec(),
                leaf: Leaf::Section(sid),
            };
            let mut old_folders = folders_m.clone();
            old_folders.extend_from_slice(&rest);
            let old_section = NodePath::section(zone, old_folders, sid);
            let new_section = NodePath::section(zone, row_folders.clone(), sid);
            let old_key = node_key(&old_dk, &below(&rest));
            let new_key = node_key(&new_dk, &below(&rest));
            let row_v = versions.get(&sid.to_string()).copied().unwrap_or(KV);
            let file = format!("e/circle/blobs/{sid}.enc");
            let pt = self.open_blob_v(&file, &old_key, &old_section, row_v)?;
            let sha = self.put_blob_v(&file, &new_key, &new_section, new_v, &pt, ent)?;
            self.bump_section_version(sid, new_v, &sha)?;
        }
        Ok(())
    }

    // --------------------------------------------- revocation-aware reads

    /// Read a circle section at its stored key version, resolving the folder
    /// key from its header's matching version (post-rotation) or by owner
    /// derivation. Owner path.
    pub fn read_circle_section_versioned(
        &self,
        owner: &OwnerKeys,
        display_path: &str,
    ) -> Result<String> {
        let (row, folders) = self.resolve_clear(Zone::Circle, display_path)?;
        let sid = Sid::parse(&row.sid)?;
        let key = self.owner_section_key(owner, &folders, sid, row.key_version)?;
        let node = NodePath::section(Zone::Circle, folders, sid);
        let pt = self.open_blob_v(
            &format!("e/circle/blobs/{sid}.enc"),
            &key,
            &node,
            row.key_version,
        )?;
        let v: serde_json::Value = serde_json::from_slice(&pt)
            .map_err(|e| Error::SealRejected(format!("blob json: {e}")))?;
        Ok(v["md"].as_str().unwrap_or_default().to_owned())
    }

    /// Owner-side section key at a version: open the deepest ancestor folder
    /// header at that version (present after rotation) and derive down, else
    /// derive straight from the zone root.
    fn owner_section_key(
        &self,
        owner: &OwnerKeys,
        folders: &[Sid],
        sid: Sid,
        version: u64,
    ) -> Result<[u8; 32]> {
        let zone = Zone::Circle;
        for depth in (0..=folders.len()).rev() {
            let ancestor = NodePath::folder(zone, folders[..depth].to_vec());
            let Some(bytes) = self.store.get(&hdr_file(zone, &ancestor)).ok().flatten() else {
                continue;
            };
            let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
                continue;
            };
            // Open at the section's version if that ancestor carries it, else
            // at the ancestor's own latest.
            let v = if header.key_versions.contains_key(&version.to_string()) {
                version
            } else {
                header.latest_version()
            };
            if let Ok(base) = header.open(&self.did, v, "owner-kex", &owner.owner_kex) {
                let rest = NodePath {
                    zone,
                    folders: folders[depth..].to_vec(),
                    leaf: Leaf::Section(sid),
                };
                return Ok(node_key(&base, &rest));
            }
        }
        Ok(node_key(
            &self.zone_dk(zone, owner)?,
            &NodePath::section(zone, folders.to_vec(), sid),
        ))
    }

    /// A section row lookup helper (unused targets tolerated).
    #[allow(dead_code)]
    fn section_row(&self, display_path: &str) -> Result<SectionRow> {
        Ok(self.resolve_clear(Zone::Circle, display_path)?.0)
    }
}

/// Old blobs before the first rotation used KV; a folder rotated from v1
/// re-reads its sections at their prior version.
fn old_v_of(old_v: u64) -> u64 {
    if old_v == 0 {
        KV
    } else {
        old_v
    }
}

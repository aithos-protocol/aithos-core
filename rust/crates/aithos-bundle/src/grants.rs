//! Grants and delegation over the bundle (spec §04.3, §05.2): issuing a
//! mandate mints the certificate AND appends the header lines (and, for
//! dir&tag perimeters, builds the folder-local tag view with its wraps).
//! Agents read with their single keypair; the verifier gates every access.

use crate::bundle::{Bundle, FolderRow, ZoneIndex, KV};
use crate::entropy::EntropySource;
use crate::Store;
use aithos_core::derive::node_key;
use aithos_core::did::DidDocument;
use aithos_core::error::{Error, Result};
use aithos_core::header::{Header, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::keys::{grantee_kex_secret, OwnerKeys};
use aithos_core::mandate::{verify_op, Mandate, MandateSpec, Op, PerimeterEntry, Verb};
use aithos_core::path::{Leaf, NodePath, Zone};
use aithos_core::wire;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::BTreeMap;

/// One requested perimeter, in display terms (names). Resolution to sids
/// happens against the clear index at issuance time. The verb spans the
/// full §04.2 lattice: the delivered key is the same for every verb
/// (symmetric node keys — whoever can open can seal); the CERTIFICATE is
/// what separates a reader from a writer, and the verifier enforces it.
#[derive(Debug, Clone)]
pub struct GrantSpec {
    pub zone: Zone,
    pub verb: Verb,
    pub dir: String,
    pub tag: Option<String>,
}

pub(crate) fn hdr_file(zone: Zone, node: &NodePath) -> String {
    let digest = blake3::hash(node.to_string().as_bytes());
    format!(
        "e/{}/hdr/{}.json",
        zone.as_str(),
        hex::encode(&digest.as_bytes()[..12])
    )
}

pub(crate) fn wrap_file(zone: Zone, via: &NodePath, node: &NodePath) -> String {
    let digest = blake3::hash(format!("{via}\u{0}{node}").as_bytes());
    format!(
        "e/{}/wraps/{}.json",
        zone.as_str(),
        hex::encode(&digest.as_bytes()[..12])
    )
}

fn agent_recipient(pubkey: &VerifyingKey) -> Recipient {
    let mb = wire::ed25519_pub_to_multibase(&pubkey.to_bytes());
    Recipient {
        to: mb.clone(),
        kid: mb,
        pubkey: aithos_core::keys::ed2x(pubkey),
    }
}

impl<S: Store> Bundle<S> {
    pub(crate) fn did_doc(&self) -> Result<DidDocument> {
        self.get_json("did.json")
    }

    pub(crate) fn owner_kex_recipient(&self) -> Result<Recipient> {
        let doc = self.did_doc()?;
        let bytes = wire::multibase_to_x25519_pub(&doc.keys.kex)?;
        Ok(Recipient::owner(bytes.into()))
    }

    /// Resolve a display folder path to its sid chain (clear zones).
    pub fn resolve_folder(&self, zone: Zone, display: &str) -> Result<Vec<Sid>> {
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let mut parent: Option<String> = None;
        let mut chain = Vec::new();
        for seg in display.split('/').filter(|s| !s.is_empty()) {
            let f = index
                .folders
                .iter()
                .find(|f| f.name == seg && f.parent_sid == parent)
                .ok_or_else(|| Error::InvalidPath(format!("no folder {seg}")))?;
            chain.push(Sid::parse(&f.sid)?);
            parent = Some(f.sid.clone());
        }
        Ok(chain)
    }

    /// Every section whose folder chain starts with `dir` and which carries
    /// `tag` (if given). Returns (row folder-chain, section sid, tags).
    pub(crate) fn sections_under(
        &self,
        zone: Zone,
        dir: &[Sid],
        tag: Option<&str>,
    ) -> Result<Vec<(Vec<Sid>, Sid)>> {
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let by_sid: BTreeMap<&str, &FolderRow> =
            index.folders.iter().map(|f| (f.sid.as_str(), f)).collect();
        let mut out = Vec::new();
        for row in &index.sections {
            if let Some(t) = tag {
                if !row.tags.iter().any(|x| x == t) {
                    continue;
                }
            }
            // Rebuild the folder chain of this section.
            let mut chain_rev = Vec::new();
            let mut cursor = row.folder_sid.clone();
            while let Some(sid) = cursor {
                let f = by_sid
                    .get(sid.as_str())
                    .ok_or_else(|| Error::InvalidPath(format!("dangling folder {sid}")))?;
                chain_rev.push(Sid::parse(&f.sid)?);
                cursor = f.parent_sid.clone();
            }
            chain_rev.reverse();
            if chain_rev.len() >= dir.len() && chain_rev[..dir.len()] == dir[..] {
                out.push((chain_rev, Sid::parse(&row.sid)?));
            }
        }
        Ok(out)
    }

    fn add_line_on(
        &mut self,
        node: &NodePath,
        dk: &[u8; 32],
        recipient: &Recipient,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let file = hdr_file(node.zone, node);
        let did = self.did.clone();
        match self.store.get(&file).ok().flatten() {
            Some(bytes) => {
                let mut header: Header = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::SealRejected(format!("{file}: {e}")))?;
                header.append_line(&did, KV, dk, recipient, ent.e32(), ent.e24())?;
                self.put_json(&file, &header)
            }
            None => {
                let owner_line = self.owner_kex_recipient()?;
                let header = Header::build(
                    &did,
                    &node.to_string(),
                    dk,
                    &[owner_line, recipient.clone()],
                    &[ent.e32(), ent.e32()],
                    &[ent.e24(), ent.e24()],
                )?;
                self.put_json(&file, &header)
            }
        }
    }

    /// Owner-side key delivery for one perimeter entry (§04.3).
    fn deliver_entry(
        &mut self,
        owner: &OwnerKeys,
        recipient: &Recipient,
        zone: Zone,
        dir: &[Sid],
        tag: Option<&str>,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let zone_dk = self.zone_dk(zone, owner)?;
        match tag {
            None => {
                let node = NodePath::folder(zone, dir.to_vec());
                let dk = node_key(&zone_dk, &node);
                self.add_line_on(&node, &dk, recipient, ent)
            }
            Some(t) => {
                let anchor = NodePath::tag_view(zone, dir.to_vec(), t)?;
                let anchor_key = node_key(&zone_dk, &anchor);
                self.add_line_on(&anchor, &anchor_key, recipient, ent)?;
                // Bridge every matching section into the view (§02.9).
                let did = self.did.clone();
                for (folders, sid) in self.sections_under(zone, dir, Some(t))? {
                    let section = NodePath::section(zone, folders, sid);
                    let k_section = node_key(&zone_dk, &section);
                    let wrap = Wrap::seal(
                        &did,
                        &anchor.to_string(),
                        &anchor_key,
                        &section.to_string(),
                        KV,
                        &k_section,
                        ent.e24(),
                    );
                    let file = wrap_file(zone, &anchor, &section);
                    self.put_json(&file, &wrap)?;
                }
                Ok(())
            }
        }
    }

    /// Owner grant (§04.3): mint the root certificate AND deliver the keys.
    #[allow(clippy::too_many_arguments)]
    pub fn grant(
        &mut self,
        owner: &OwnerKeys,
        label: &str,
        agent_pub: &VerifyingKey,
        specs: &[GrantSpec],
        not_before: &str,
        not_after: &str,
        issue_depth: u32,
        ent: &mut dyn EntropySource,
    ) -> Result<Mandate> {
        let mut perimeter = Vec::new();
        let recipient = agent_recipient(agent_pub);
        for spec in specs {
            let dir = self.resolve_folder(spec.zone, &spec.dir)?;
            self.deliver_entry(owner, &recipient, spec.zone, &dir, spec.tag.as_deref(), ent)?;
            perimeter.push(PerimeterEntry::Ethos {
                verb: spec.verb,
                zone: spec.zone,
                dir,
                tag: spec.tag.clone(),
            });
        }
        if issue_depth > 0 {
            perimeter.push(PerimeterEntry::Issue { depth: issue_depth });
        }
        let mandate = Mandate::build_root(
            &owner.root_sign,
            &MandateSpec {
                id: format!(
                    "mandate_{}",
                    Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
                ),
                subject: self.did.clone(),
                constraints: MandateSpec::no_constraints(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.to_owned(),
                grantee_pub: agent_pub,
                perimeter,
                not_before: not_before.to_owned(),
                not_after: not_after.to_owned(),
                issued_at: not_before.to_owned(),
                nonce: hex::encode(ent.e16()),
            },
        )?;
        self.put_json(&format!("certs/{}.json", mandate.id), &mandate)?;
        Ok(mandate)
    }

    /// What an agent can compute for a node from its own lines: try headers
    /// on the node and every ancestor folder, then derive down (§02.5).
    pub(crate) fn agent_node_key(
        &self,
        kid: &str,
        kex: &x25519_dalek::StaticSecret,
        node: &NodePath,
    ) -> Result<[u8; 32]> {
        // 1. A header directly on the node itself.
        if let Some(k) = self.try_header(kid, kex, node)? {
            return Ok(k);
        }
        // 2. A header on any ancestor FOLDER, deepest first, then derive down
        //    the remaining path (folders[depth..] + the original leaf).
        for depth in (0..=node.folders.len()).rev() {
            let ancestor = NodePath::folder(node.zone, node.folders[..depth].to_vec());
            let Some(base) = self.try_header(kid, kex, &ancestor)? else {
                continue;
            };
            let rest = NodePath {
                zone: node.zone,
                folders: node.folders[depth..].to_vec(),
                leaf: node.leaf.clone(),
            };
            return Ok(node_key(&base, &rest));
        }
        Err(Error::SealRejected(format!("no reachable line for {node}")))
    }

    fn try_header(
        &self,
        kid: &str,
        kex: &x25519_dalek::StaticSecret,
        node: &NodePath,
    ) -> Result<Option<[u8; 32]>> {
        let Some(bytes) = self.store.get(&hdr_file(node.zone, node)).ok().flatten() else {
            return Ok(None);
        };
        let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
            return Ok(None);
        };
        Ok(header.open(&self.did, KV, kid, kex).ok())
    }

    /// Agent read: verifier first (§04.5), then key acquisition — direct
    /// lines/derivation, else through a granted tag view and its wrap.
    pub fn read_section_as_agent(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        display_path: &str,
        at: &str,
    ) -> Result<String> {
        let doc = self.did_doc()?;
        let revs = self.active_revocations()?;
        aithos_core::mandate::verify_chain_revocable(chain, &doc, at, &revs)?;
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        let op = Op {
            verb: Verb::Read,
            zone,
            folders: &folders,
            tags: &row.tags,
        };
        verify_op(chain, &doc, at, &op)?;

        let leaf = chain.last().expect("non-empty chain");
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
        let sid = Sid::parse(&row.sid)?;
        let section = NodePath::section(zone, folders.clone(), sid);

        let k_section = match self.agent_section_key(&kid, &kex, &folders, sid, row.key_version) {
            Ok(k) => k,
            Err(_) => {
                // Tag views granted to this leaf whose dir covers the section.
                let mut found = None;
                for entry in leaf.parsed_perimeter()? {
                    let PerimeterEntry::Ethos {
                        zone: ez,
                        dir,
                        tag: Some(t),
                        ..
                    } = entry
                    else {
                        continue;
                    };
                    if ez != zone
                        || folders.len() < dir.len()
                        || folders[..dir.len()] != dir[..]
                        || !row.tags.iter().any(|x| x == &t)
                    {
                        continue;
                    }
                    let anchor = NodePath::tag_view(zone, dir, &t)?;
                    let Ok(anchor_key) = self.agent_node_key(&kid, &kex, &anchor) else {
                        continue;
                    };
                    let wrap: Wrap = self.get_json(&wrap_file(zone, &anchor, &section))?;
                    found = Some(wrap.open(&self.did, &anchor_key)?);
                    break;
                }
                found
                    .ok_or_else(|| Error::SealRejected(format!("no key path to {display_path}")))?
            }
        };

        let pt = self.open_blob_v(
            &format!("e/{}/blobs/{}.enc", zone.as_str(), row.sid),
            &k_section,
            &section,
            row.key_version,
        )?;
        let v: serde_json::Value = serde_json::from_slice(&pt)
            .map_err(|e| Error::SealRejected(format!("blob json: {e}")))?;
        Ok(v["md"].as_str().unwrap_or_default().to_owned())
    }

    pub(crate) fn agent_section_key(
        &self,
        kid: &str,
        kex: &x25519_dalek::StaticSecret,
        folders: &[Sid],
        sid: Sid,
        version: u64,
    ) -> Result<[u8; 32]> {
        let zone = Zone::Circle;
        let section = NodePath::section(zone, folders.to_vec(), sid);
        // From the deepest folder this key holds a line on, walk DOWN to the
        // section: derive by default, step through an up-link wrap wherever
        // a rotation (§03.4, via the zone root) or a move (§02.9, via the
        // new parent) re-keyed a node along the way. Fail-closed: a wrap
        // that does not open with the walk's current key grants nothing.
        for depth in (0..=folders.len()).rev() {
            let ancestor = NodePath::folder(zone, folders[..depth].to_vec());
            let Some(bytes) = self.store.get(&hdr_file(zone, &ancestor)).ok().flatten() else {
                continue;
            };
            let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
                continue;
            };
            let v = if header.key_versions.contains_key(&version.to_string()) {
                version
            } else {
                header.latest_version()
            };
            let Ok(base) = header.open(&self.did, v, kid, kex) else {
                continue;
            };
            let mut k = base;
            for d in depth..folders.len() {
                let parent = NodePath::folder(zone, folders[..d].to_vec());
                let child = NodePath::folder(zone, folders[..=d].to_vec());
                // A wrap via the immediate parent wins over derivation:
                // past a fresh key, the derived value is stale by design.
                if let Ok(wrap) = self.get_json::<Wrap>(&wrap_file(zone, &parent, &child)) {
                    if let Ok(dk) = wrap.open(&self.did, &k) {
                        k = dk;
                        continue;
                    }
                }
                // Rotation wraps hang under the zone root (§03.4 step 2bis);
                // only the zone-root key itself opens them.
                if depth == 0 {
                    let zroot = NodePath::zone_root(zone);
                    if let Ok(wrap) = self.get_json::<Wrap>(&wrap_file(zone, &zroot, &child)) {
                        if let Ok(dk) = wrap.open(&self.did, &base) {
                            k = dk;
                            continue;
                        }
                    }
                }
                k = node_key(
                    &k,
                    &NodePath {
                        zone,
                        folders: vec![folders[d]],
                        leaf: Leaf::Folder,
                    },
                );
            }
            return Ok(node_key(
                &k,
                &NodePath {
                    zone,
                    folders: vec![],
                    leaf: Leaf::Section(sid),
                },
            ));
        }
        self.agent_node_key(kid, kex, &section)
    }

    /// Delegation (§05.2): the parent mints and signs the sub-mandate and
    /// delivers keys FROM ITS OWN ACCESS — best effort: physics refuses
    /// what policy would anyway (§05.4). The verifier remains the judge.
    #[allow(clippy::too_many_arguments)]
    pub fn delegate(
        &mut self,
        parent: &Mandate,
        parent_sk: &SigningKey,
        label: &str,
        child_pub: &VerifyingKey,
        specs: &[GrantSpec],
        not_before: &str,
        not_after: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Mandate> {
        let parent_kid = parent.grantee.pubkey.clone();
        let parent_kex = grantee_kex_secret(parent_sk);
        let recipient = agent_recipient(child_pub);
        let mut perimeter = Vec::new();
        for spec in specs {
            let dir = self.resolve_folder(spec.zone, &spec.dir)?;
            let node = NodePath::folder(spec.zone, dir.clone());
            // Physical attenuation: only deliverable if the parent holds it.
            if let Ok(dk) = self.agent_node_key(&parent_kid, &parent_kex, &node) {
                self.add_line_on(&node, &dk, &recipient, ent)?;
            }
            perimeter.push(PerimeterEntry::Ethos {
                verb: spec.verb,
                zone: spec.zone,
                dir,
                tag: spec.tag.clone(),
            });
        }
        let child = Mandate::build_sub(
            parent,
            parent_sk,
            &MandateSpec {
                id: format!(
                    "mandate_{}",
                    Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
                ),
                subject: self.did.clone(),
                constraints: MandateSpec::no_constraints(),
                grantee_id: format!("urn:aithos:agent:{label}"),
                grantee_label: label.to_owned(),
                grantee_pub: child_pub,
                perimeter,
                not_before: not_before.to_owned(),
                not_after: not_after.to_owned(),
                issued_at: not_before.to_owned(),
                nonce: hex::encode(ent.e16()),
            },
        )?;
        self.put_json(&format!("certs/{}.json", child.id), &child)?;
        Ok(child)
    }

    /// Owner-side key delivery for ONE perimeter entry, without minting a
    /// certificate (§04.3): what callers assembling a richer mandate by
    /// hand (constraints, act/gamma/issue/revoke entries) use to pair the
    /// certificate half with its header line.
    pub fn deliver_zone_line(
        &mut self,
        owner: &OwnerKeys,
        agent_pub: &VerifyingKey,
        zone: Zone,
        dir_display: &str,
        tag: Option<&str>,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let recipient = agent_recipient(agent_pub);
        let dir = self.resolve_folder(zone, dir_display)?;
        self.deliver_entry(owner, &recipient, zone, &dir, tag, ent)
    }

    // -------------------------------------------------- delegated writes
    //
    // Spec §04.2 (verb lattice), §04.3 (the line IS the pen), §07.2
    // (delegated mutations are log citizens like actions). Circle only
    // this pass, like the owner-side rewrite/delete above.

    /// The governing write key for a (possibly new) section, agent side:
    /// the deepest ancestor folder carrying a header governs, at the
    /// LATEST version it publishes; the agent must reach that version
    /// through its own lines and wraps — physics refuses a stale pen
    /// (§03.4: writing at v1 past a rotation would hand content back to
    /// whoever the rotation cut). Returns `(key_version, section_key)`.
    pub(crate) fn agent_current_section_key(
        &self,
        kid: &str,
        kex: &x25519_dalek::StaticSecret,
        folders: &[Sid],
        sid: Sid,
    ) -> Result<(u64, [u8; 32])> {
        let zone = Zone::Circle;
        // The governing version is clear header metadata: deepest ancestor
        // folder with a header, its latest published version. KV when the
        // whole path is derivation-only.
        let mut governing = KV;
        for depth in (0..=folders.len()).rev() {
            let ancestor = NodePath::folder(zone, folders[..depth].to_vec());
            let Some(bytes) = self.store.get(&hdr_file(zone, &ancestor)).ok().flatten() else {
                continue;
            };
            let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
                continue;
            };
            governing = header.latest_version();
            break;
        }
        let key = self.agent_section_key(kid, kex, folders, sid, governing)?;
        Ok((governing, key))
    }

    /// Shared preamble of every delegated write: chain valid and unrevoked
    /// at `now`, operation covered by the leaf perimeter (§04.5 steps 1–7
    /// for the certificate half; the key acquisition that follows is the
    /// physics half).
    fn check_delegated_write(
        &self,
        chain: &[Mandate],
        verb: Verb,
        zone: Zone,
        folders: &[Sid],
        tags: &[String],
        now: &str,
    ) -> Result<()> {
        if zone != Zone::Circle {
            return Err(Error::InvalidPath(
                "delegated writes: circle only this pass".to_owned(),
            ));
        }
        let doc = self.did_doc()?;
        let revs = self.active_revocations()?;
        aithos_core::mandate::verify_chain_revocable(chain, &doc, now, &revs)?;
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty chain".to_owned()))?;
        let op = Op {
            verb,
            zone,
            folders,
            tags,
        };
        if !aithos_core::mandate::covers_op(&leaf.parsed_perimeter()?, &op) {
            return Err(Error::InvalidMandate(format!(
                "{}: write not covered by the leaf perimeter",
                leaf.id
            )));
        }
        Ok(())
    }

    /// Delegated section creation (§04.2 `append` = create within
    /// perimeter). The blob is UNSIGNED (§02.11: owner signatures are the
    /// owner's; agent authorship is evidenced by the delegated gamma
    /// entry, signed by the grantee key under its chain). The folder must
    /// exist: an append perimeter grows content, never the tree shape.
    pub fn section_add_as_agent(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        spec: &crate::bundle::SectionSpec<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let crate::bundle::SectionSpec {
            zone,
            folder_path,
            name,
            title,
            tags,
            body,
            now,
        } = *spec;
        let folders = self.resolve_folder(zone, folder_path)?;
        self.check_delegated_write(chain, Verb::Append, zone, &folders, tags, now)?;
        let leaf = chain.last().expect("checked non-empty");
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
        let sid = Bundle::<S>::new_sid(ent);
        let node = NodePath::section(zone, folders.clone(), sid);
        let (kv, key) = self.agent_current_section_key(&kid, &kex, &folders, sid)?;
        let blob = serde_json::json!({ "md": body });
        let sha = self.put_blob_v(
            &format!("e/circle/blobs/{sid}.enc"),
            &key,
            &node,
            kv,
            &aithos_core::jcs::canonical_bytes(&blob)?,
            ent,
        )?;
        let index_path = "e/circle/index.json";
        let mut index: ZoneIndex = self.get_json(index_path)?;
        index.sections.push(crate::bundle::SectionRow {
            sid: sid.to_string(),
            name: name.to_owned(),
            folder_sid: folders.last().map(ToString::to_string),
            title: title.to_owned(),
            tags: tags.to_vec(),
            blob_sha: sha.clone(),
            key_version: kv,
            sig: None,
        });
        self.put_json(index_path, &index)?;
        self.log_delegated_mutation(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionAdd,
            &node,
            serde_json::json!({ "blob_sha": sha, "name": name }),
            now,
            ent,
        )?;
        Ok(())
    }

    /// Delegated rewrite of an existing circle section (§04.2 `edit`):
    /// new unsigned blob under the governing key, row updated in place,
    /// delegated `section.modify` logged (§07.2, §07.4).
    #[allow(clippy::too_many_arguments)]
    pub fn section_rewrite_as_agent(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        display_path: &str,
        body: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        self.check_delegated_write(chain, Verb::Edit, zone, &folders, &row.tags, now)?;
        let leaf = chain.last().expect("checked non-empty");
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
        let sid = Sid::parse(&row.sid)?;
        let node = NodePath::section(zone, folders.clone(), sid);
        let (kv, key) = self.agent_current_section_key(&kid, &kex, &folders, sid)?;
        let blob = serde_json::json!({ "md": body });
        let sha = self.put_blob_v(
            &format!("e/circle/blobs/{sid}.enc"),
            &key,
            &node,
            kv,
            &aithos_core::jcs::canonical_bytes(&blob)?,
            ent,
        )?;
        let index_path = "e/circle/index.json";
        let mut index: ZoneIndex = self.get_json(index_path)?;
        let entry = index
            .sections
            .iter_mut()
            .find(|r| r.sid == row.sid)
            .ok_or_else(|| Error::InvalidPath(format!("no section {display_path}")))?;
        entry.blob_sha = sha.clone();
        entry.key_version = kv;
        entry.sig = None;
        self.put_json(index_path, &index)?;
        self.log_delegated_mutation(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionModify,
            &node,
            serde_json::json!({ "blob_sha": sha }),
            now,
            ent,
        )?;
        Ok(())
    }

    /// Delegated deletion (§04.2 `delete`): the index row goes, the
    /// delegated `section.delete` is logged. Like the owner op, the sealed
    /// blob bytes stay — erasure is cryptographic (§06), a row-less blob
    /// is unreachable.
    pub fn section_delete_as_agent(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        display_path: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        self.check_delegated_write(chain, Verb::Delete, zone, &folders, &row.tags, now)?;
        let sid = Sid::parse(&row.sid)?;
        let node = NodePath::section(zone, folders, sid);
        // The pen must reach the node it strikes (physics half): a delete
        // is refused to a chain whose keys never covered the target.
        let leaf = chain.last().expect("checked non-empty");
        let kex = grantee_kex_secret(agent_sk);
        self.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?;
        let index_path = "e/circle/index.json";
        let mut index: ZoneIndex = self.get_json(index_path)?;
        index.sections.retain(|r| r.sid != row.sid);
        self.put_json(index_path, &index)?;
        self.log_delegated_mutation(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionDelete,
            &node,
            serde_json::json!({ "name": row.name }),
            now,
            ent,
        )?;
        Ok(())
    }
}

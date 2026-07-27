//! CB10 structural mutations.
//!
//! The wire lattice remains `read|edit|append|delete|write`.  This module
//! composes those existing rights for folder and metadata operations, then
//! commits every direct and derived local change with one Gamma occurrence in
//! the Store transaction supplied by [`Bundle::transaction`].

use crate::bundle::{Bundle, FolderRow, TreeEntry, ZoneIndex};
use crate::entropy::EntropySource;
use crate::Store;
use aithos_core::error::{Error, Result};
use aithos_core::header::{Header, Recipient, Wrap};
use aithos_core::ids::{validate_tag, Sid};
use aithos_core::keys::{ed2x, grantee_kex_secret};
use aithos_core::mandate::{covers_op, covers_section_op, Mandate, Op, SectionOp, Verb};
use aithos_core::path::{Leaf, NodePath, Zone};
use aithos_core::wire;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub enum StructuralOperation<'a> {
    ListFolder {
        zone: Zone,
        folder: &'a str,
        now: &'a str,
    },
    CreateFolder {
        zone: Zone,
        parent: &'a str,
        name: &'a str,
        now: &'a str,
    },
    RenameFolder {
        zone: Zone,
        folder: &'a str,
        new_name: &'a str,
        now: &'a str,
    },
    DeleteFolder {
        zone: Zone,
        folder: &'a str,
        recursive: bool,
        now: &'a str,
    },
    MoveFolder {
        zone: Zone,
        folder: &'a str,
        destination_parent: &'a str,
        now: &'a str,
    },
    EditSectionMetadata {
        zone: Zone,
        section: &'a str,
        name: Option<&'a str>,
        title: Option<&'a str>,
        tags: Option<&'a [String]>,
        now: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralOutcome {
    Listed(Vec<TreeEntry>),
    Created(Sid),
    Mutated,
}

impl<S: Store> Bundle<S> {
    fn structural_perimeter(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        now: &str,
    ) -> Result<Vec<aithos_core::mandate::PerimeterEntry>> {
        self.verify_current_grantee(chain, agent_sk, now)
    }

    fn structural_folder_gate(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        verb: Verb,
        zone: Zone,
        folders: &[Sid],
        now: &str,
    ) -> Result<()> {
        let perimeter = self.structural_perimeter(chain, agent_sk, now)?;
        if !covers_op(
            &perimeter,
            &Op {
                verb,
                zone,
                folders,
                tags: &[],
            },
        ) {
            return Err(Error::InvalidMandate(
                "structural folder operation exceeds the leaf perimeter".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_section_gate(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        verb: Verb,
        zone: Zone,
        folders: &[Sid],
        sid: Sid,
        tags: &[String],
        now: &str,
    ) -> Result<()> {
        let perimeter = self.structural_perimeter(chain, agent_sk, now)?;
        if !covers_section_op(
            &perimeter,
            &SectionOp {
                verb,
                zone,
                sid,
                folders,
                tags,
            },
        ) {
            return Err(Error::InvalidMandate(
                "structural section operation exceeds the leaf perimeter".into(),
            ));
        }
        Ok(())
    }

    fn clear_folder_row(
        &self,
        zone: Zone,
        display: &str,
    ) -> Result<(ZoneIndex, Vec<Sid>, FolderRow)> {
        if zone == Zone::Self_ {
            return Err(Error::InvalidPath(
                "self structure uses opaque exact-id operations".into(),
            ));
        }
        let folders = self.resolve_folder(zone, display)?;
        let sid = folders
            .last()
            .ok_or_else(|| Error::InvalidPath("the zone root is not a mutable folder".into()))?;
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let row = index
            .folders
            .iter()
            .find(|row| row.sid == sid.to_string())
            .cloned()
            .ok_or_else(|| Error::InvalidPath(format!("missing folder row {sid}")))?;
        Ok((index, folders, row))
    }

    fn structural_log_key(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        node: &NodePath,
    ) -> Result<Option<[u8; 32]>> {
        if node.zone == Zone::Public {
            return Ok(None);
        }
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty chain".into()))?;
        let kex = grantee_kex_secret(agent_sk);
        self.agent_node_key(&leaf.grantee.pubkey, &kex, node)
            .map(Some)
    }

    fn structural_actor_folder_key(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folders: &[Sid],
    ) -> Result<[u8; 32]> {
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty structural chain".into()))?;
        let kid = &leaf.grantee.pubkey;
        let kex = grantee_kex_secret(agent_sk);
        for depth in (0..=folders.len()).rev() {
            let ancestor = NodePath::folder(zone, folders[..depth].to_vec());
            let Some(bytes) = self
                .store
                .get(&crate::grants::hdr_file(zone, &ancestor))
                .map_err(|error| {
                    Error::SealRejected(format!("structural header read failed: {error}"))
                })?
            else {
                continue;
            };
            let header: Header = serde_json::from_slice(&bytes)
                .map_err(|error| Error::SealRejected(format!("structural header: {error}")))?;
            let Ok((_, base)) = header.open_latest(&self.did, kid, &kex) else {
                continue;
            };
            let mut key = base;
            for child_depth in depth..folders.len() {
                let parent = NodePath::folder(zone, folders[..child_depth].to_vec());
                let child = NodePath::folder(zone, folders[..=child_depth].to_vec());
                if let Ok(wrap) =
                    self.get_json::<Wrap>(&crate::grants::wrap_file(zone, &parent, &child))
                {
                    if let Ok(fresh) = wrap.open(&self.did, &key) {
                        key = fresh;
                        continue;
                    }
                }
                if depth == 0 {
                    let root = NodePath::zone_root(zone);
                    if let Ok(wrap) =
                        self.get_json::<Wrap>(&crate::grants::wrap_file(zone, &root, &child))
                    {
                        if let Ok(fresh) = wrap.open(&self.did, &base) {
                            key = fresh;
                            continue;
                        }
                    }
                }
                key = aithos_core::derive::node_key(
                    &key,
                    &NodePath {
                        zone,
                        folders: vec![folders[child_depth]],
                        leaf: Leaf::Folder,
                    },
                );
            }
            return Ok(key);
        }
        Err(Error::SealRejected(
            "the structural actor has no current key path to the folder".into(),
        ))
    }

    fn structural_recipients(
        &self,
        header: Option<&Header>,
        version: u64,
    ) -> Result<Vec<Recipient>> {
        let Some(header) = header else {
            return Ok(vec![self.owner_kex_recipient()?]);
        };
        let lines = &header
            .key_versions
            .get(&version.to_string())
            .ok_or_else(|| Error::SealRejected("missing structural key version".into()))?
            .lines;
        lines
            .iter()
            .map(|line| {
                if line.to == "owner" {
                    self.owner_kex_recipient()
                } else {
                    let bytes = wire::multibase_to_ed25519_pub(&line.to)?;
                    let verifying = VerifyingKey::from_bytes(&bytes)
                        .map_err(|_| Error::SealRejected("bad structural recipient".into()))?;
                    Ok(Recipient {
                        to: line.to.clone(),
                        kid: line.kid.clone(),
                        pubkey: ed2x(&verifying),
                    })
                }
            })
            .collect()
    }

    fn structural_tag_headers(&self, zone: Zone) -> Result<Vec<(Header, NodePath, String)>> {
        let mut headers = Vec::new();
        for path in self
            .store
            .list(&format!("e/{}/hdr/", zone.as_str()))
            .map_err(|error| {
                Error::SealRejected(format!("structural tag header list failed: {error}"))
            })?
        {
            let header: Header = self.get_json(&path)?;
            let node = NodePath::parse(&header.node)?;
            if let Leaf::TagView(tag) = &node.leaf {
                headers.push((header, node.clone(), tag.clone()));
            }
        }
        Ok(headers)
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_sync_metadata_tag_wraps(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        folders: &[Sid],
        sid: Sid,
        old_tags: &[String],
        new_tags: &[String],
        version: u64,
        section_key: &[u8; 32],
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty structural chain".into()))?;
        let kex = grantee_kex_secret(agent_sk);
        let section = NodePath::section(Zone::Circle, folders.to_vec(), sid);
        for (header, anchor, tag) in self.structural_tag_headers(Zone::Circle)? {
            if folders.len() < anchor.folders.len()
                || folders[..anchor.folders.len()] != anchor.folders
            {
                continue;
            }
            let before = old_tags.iter().any(|candidate| candidate == &tag);
            let after = new_tags.iter().any(|candidate| candidate == &tag);
            let path = crate::grants::wrap_file(Zone::Circle, &anchor, &section);
            if before && !after {
                if self
                    .store
                    .get(&path)
                    .map_err(|error| {
                        Error::SealRejected(format!("tag wrap presence failed: {error}"))
                    })?
                    .is_some()
                {
                    self.delete_object(&path)?;
                }
            } else if !before && after {
                let (_, anchor_key) = header.open_latest(&self.did, &leaf.grantee.pubkey, &kex)?;
                let wrap = Wrap::seal(
                    &self.did,
                    &anchor.to_string(),
                    &anchor_key,
                    &section.to_string(),
                    version,
                    section_key,
                    ent.e24(),
                );
                self.put_json(&path, &wrap)?;
            }
        }
        Ok(())
    }

    fn structural_list(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folder: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        if zone == Zone::Self_ {
            return Err(Error::InvalidPath(
                "self listing requires an opaque folder capability".into(),
            ));
        }
        let folders = self.resolve_folder(zone, folder)?;
        self.structural_folder_gate(chain, agent_sk, Verb::Read, zone, &folders, now)?;
        let node = NodePath::folder(zone, folders);
        let key = self.structural_log_key(chain, agent_sk, &node)?;
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{folder}/")
        };
        let entries = self
            .clear_zone_entries(zone)?
            .into_iter()
            .filter_map(|mut entry| {
                if folder.is_empty() {
                    return Some(entry);
                }
                let relative = entry.path.strip_prefix(&prefix)?.to_owned();
                entry.path = relative;
                Some(entry)
            })
            .collect();
        self.log_delegated_read(chain, agent_sk, &node, key.as_ref(), "list", now, ent)?;
        Ok(StructuralOutcome::Listed(entries))
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_create_folder(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        parent: &str,
        name: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        if zone == Zone::Self_ {
            return Err(Error::InvalidPath(
                "self folders are not addressable by display path".into(),
            ));
        }
        Self::gate_display_name(name)?;
        let parent_folders = self.resolve_folder(zone, parent)?;
        self.structural_folder_gate(chain, agent_sk, Verb::Append, zone, &parent_folders, now)?;
        let index_path = format!("e/{}/index.json", zone.as_str());
        let mut index: ZoneIndex = self.get_json(&index_path)?;
        let parent_sid = parent_folders.last().map(ToString::to_string);
        if index
            .folders
            .iter()
            .any(|row| row.parent_sid == parent_sid && row.name == name)
        {
            return Err(Error::InvalidPath(format!(
                "a folder named {name} already exists at the destination"
            )));
        }
        let sid = Self::new_sid(ent);
        index.folders.push(FolderRow {
            sid: sid.to_string(),
            name: name.to_owned(),
            parent_sid,
        });
        self.put_json(&index_path, &index)?;
        let mut child_folders = parent_folders;
        child_folders.push(sid);
        let child = NodePath::folder(zone, child_folders);
        let key = self.structural_log_key(chain, agent_sk, &child)?;
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionAdd,
            &child,
            key.as_ref(),
            serde_json::json!({
                "name": name,
                "structural": "folder.create",
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Created(sid))
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_rename_folder(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folder: &str,
        new_name: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        Self::gate_display_name(new_name)?;
        let (mut index, folders, row) = self.clear_folder_row(zone, folder)?;
        self.structural_folder_gate(chain, agent_sk, Verb::Edit, zone, &folders, now)?;
        if index.folders.iter().any(|candidate| {
            candidate.sid != row.sid
                && candidate.parent_sid == row.parent_sid
                && candidate.name == new_name
        }) {
            return Err(Error::InvalidPath(format!(
                "a folder named {new_name} already exists beside the target"
            )));
        }
        index
            .folders
            .iter_mut()
            .find(|candidate| candidate.sid == row.sid)
            .expect("resolved folder row")
            .name = new_name.to_owned();
        self.put_json(&format!("e/{}/index.json", zone.as_str()), &index)?;
        let node = NodePath::folder(zone, folders);
        let key = self.structural_log_key(chain, agent_sk, &node)?;
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionModify,
            &node,
            key.as_ref(),
            serde_json::json!({
                "name": new_name,
                "structural": "folder.rename",
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Mutated)
    }

    fn folder_chains(index: &ZoneIndex) -> Result<BTreeMap<String, Vec<Sid>>> {
        let by_sid: BTreeMap<&str, &FolderRow> = index
            .folders
            .iter()
            .map(|row| (row.sid.as_str(), row))
            .collect();
        let mut chains = BTreeMap::new();
        for row in &index.folders {
            let mut reverse = vec![Sid::parse(&row.sid)?];
            let mut cursor = row.parent_sid.as_deref();
            while let Some(parent) = cursor {
                let parent_row = by_sid
                    .get(parent)
                    .ok_or_else(|| Error::InvalidPath(format!("dangling folder {parent}")))?;
                reverse.push(Sid::parse(&parent_row.sid)?);
                cursor = parent_row.parent_sid.as_deref();
            }
            reverse.reverse();
            chains.insert(row.sid.clone(), reverse);
        }
        Ok(chains)
    }

    fn clear_section_display(
        index: &ZoneIndex,
        chains: &BTreeMap<String, Vec<Sid>>,
        sid: &str,
    ) -> Result<String> {
        let row = index
            .sections
            .iter()
            .find(|row| row.sid == sid)
            .ok_or_else(|| Error::InvalidPath(format!("missing section {sid}")))?;
        let names: BTreeMap<String, String> = index
            .folders
            .iter()
            .map(|folder| (folder.sid.clone(), folder.name.clone()))
            .collect();
        let mut display = row
            .folder_sid
            .as_ref()
            .and_then(|folder| chains.get(folder))
            .into_iter()
            .flatten()
            .map(|part| {
                names
                    .get(&part.to_string())
                    .map(String::as_str)
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        display.push(&row.name);
        Ok(display.join("/"))
    }

    fn delete_subtree_artifacts(
        &mut self,
        zone: Zone,
        root: &NodePath,
        section_sids: &BTreeSet<String>,
        public_paths: &[String],
    ) -> Result<()> {
        for sid in section_sids {
            match zone {
                Zone::Public => {}
                Zone::Circle => {
                    self.delete_object(&format!("e/circle/blobs/{sid}.enc"))?;
                }
                Zone::Self_ => unreachable!("clear-zone structural gate"),
            }
        }
        for display in public_paths {
            self.delete_object(&format!("e/public/{display}.md"))?;
        }
        let paths = self
            .store
            .list(&format!("e/{}/", zone.as_str()))
            .map_err(|error| Error::SealRejected(format!("structural list: {error}")))?;
        for path in paths {
            if path.contains("/hdr/") {
                let header: Header = self.get_json(&path)?;
                let node = NodePath::parse(&header.node)?;
                if root.covers(&node) {
                    self.delete_object(&path)?;
                }
            } else if path.contains("/wraps/") {
                let wrap: Wrap = self.get_json(&path)?;
                let node = NodePath::parse(&wrap.node)?;
                let via = NodePath::parse(&wrap.via)?;
                if root.covers(&node) || root.covers(&via) {
                    self.delete_object(&path)?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_delete_folder(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folder: &str,
        recursive: bool,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        let (mut index, folders, row) = self.clear_folder_row(zone, folder)?;
        let chains = Self::folder_chains(&index)?;
        let root = NodePath::folder(zone, folders.clone());
        let descendant_folders = chains
            .iter()
            .filter(|(_, chain)| chain.len() >= folders.len() && chain[..folders.len()] == folders)
            .map(|(sid, chain)| (sid.clone(), chain.clone()))
            .collect::<Vec<_>>();
        let descendant_ids = descendant_folders
            .iter()
            .map(|(sid, _)| sid.clone())
            .collect::<BTreeSet<_>>();
        let sections = index
            .sections
            .iter()
            .filter(|section| {
                section
                    .folder_sid
                    .as_ref()
                    .is_some_and(|folder_sid| descendant_ids.contains(folder_sid))
            })
            .cloned()
            .collect::<Vec<_>>();
        if !recursive && (descendant_folders.len() != 1 || !sections.is_empty()) {
            return Err(Error::InvalidPath(
                "non-empty folder deletion requires complete subtree coverage".into(),
            ));
        }
        for (_, chain_path) in &descendant_folders {
            self.structural_folder_gate(chain, agent_sk, Verb::Delete, zone, chain_path, now)?;
        }
        for section in &sections {
            let folder_chain = section
                .folder_sid
                .as_ref()
                .and_then(|sid| chains.get(sid))
                .cloned()
                .unwrap_or_default();
            self.structural_section_gate(
                chain,
                agent_sk,
                Verb::Delete,
                zone,
                &folder_chain,
                Sid::parse(&section.sid)?,
                &section.tags,
                now,
            )?;
        }
        // Capture the actor's proven node key before the subtree's headers
        // are removed. The same key seals the terminal Gamma evidence; trying
        // to reacquire it after deletion would turn an authorized delete into
        // a false refusal and rely on transaction rollback to hide the bug.
        let key = self.structural_log_key(chain, agent_sk, &root)?;
        let public_paths = if zone == Zone::Public {
            sections
                .iter()
                .map(|section| Self::clear_section_display(&index, &chains, &section.sid))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        let section_ids = sections
            .iter()
            .map(|section| section.sid.clone())
            .collect::<BTreeSet<_>>();
        index
            .folders
            .retain(|candidate| !descendant_ids.contains(&candidate.sid));
        index
            .sections
            .retain(|candidate| !section_ids.contains(&candidate.sid));
        self.put_json(&format!("e/{}/index.json", zone.as_str()), &index)?;
        self.delete_subtree_artifacts(zone, &root, &section_ids, &public_paths)?;
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionDelete,
            &root,
            key.as_ref(),
            serde_json::json!({
                "folder_count": descendant_ids.len(),
                "name": row.name,
                "section_count": section_ids.len(),
                "structural": "folder.delete",
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Mutated)
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_move_circle_folder(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        folder: &str,
        destination_parent: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        let zone = Zone::Circle;
        let (mut index, source, row) = self.clear_folder_row(zone, folder)?;
        let destination = self.resolve_folder(zone, destination_parent)?;
        if destination.len() >= source.len() && destination[..source.len()] == source {
            return Err(Error::InvalidPath(
                "cannot move a folder into its own descendant".into(),
            ));
        }
        self.structural_folder_gate(chain, agent_sk, Verb::Edit, zone, &source, now)?;
        self.structural_folder_gate(chain, agent_sk, Verb::Append, zone, &destination, now)?;
        let destination_sid = destination.last().map(ToString::to_string);
        if row.parent_sid == destination_sid {
            return Err(Error::InvalidPath(
                "folder already sits under that parent".into(),
            ));
        }
        if index.folders.iter().any(|candidate| {
            candidate.sid != row.sid
                && candidate.parent_sid == destination_sid
                && candidate.name == row.name
        }) {
            return Err(Error::InvalidPath(
                "destination sibling name collision".into(),
            ));
        }

        let old_node = NodePath::folder(zone, source.clone());
        let mut new_chain = destination.clone();
        let moved_sid = Sid::parse(&row.sid)?;
        new_chain.push(moved_sid);
        let new_node = NodePath::folder(zone, new_chain.clone());
        for (_, anchor, _) in self.structural_tag_headers(zone)? {
            if anchor.folders.len() >= source.len() && anchor.folders[..source.len()] == source {
                return Err(Error::InvalidPath(
                    "move requires a rotation manager while a tag view is anchored inside the subtree"
                        .into(),
                ));
            }
        }

        let source_header_path = crate::grants::hdr_file(zone, &old_node);
        let source_header = self
            .store
            .get(&source_header_path)
            .map_err(|error| {
                Error::SealRejected(format!("structural source header read failed: {error}"))
            })?
            .map(|bytes| {
                serde_json::from_slice::<Header>(&bytes)
                    .map_err(|error| Error::SealRejected(format!("source header: {error}")))
            })
            .transpose()?;
        let old_version = source_header
            .as_ref()
            .map(Header::latest_version)
            .unwrap_or(crate::bundle::KV);
        let old_key = self.structural_actor_folder_key(chain, agent_sk, zone, source.as_slice())?;
        let parent_key =
            self.structural_actor_folder_key(chain, agent_sk, zone, destination.as_slice())?;
        let recipients = self.structural_recipients(source_header.as_ref(), old_version)?;
        let new_version = old_version + 1;
        let new_key = ent.e32();

        let old_index = index.clone();
        index
            .folders
            .iter_mut()
            .find(|candidate| candidate.sid == row.sid)
            .expect("resolved circle folder")
            .parent_sid = destination_sid;
        self.put_json("e/circle/index.json", &index)?;

        let ephemerals = recipients.iter().map(|_| ent.e32()).collect::<Vec<_>>();
        let nonces = recipients.iter().map(|_| ent.e24()).collect::<Vec<_>>();
        let header = Header::build_at(
            &self.did,
            &new_node.to_string(),
            new_version,
            &new_key,
            &recipients,
            &ephemerals,
            &nonces,
        )?;
        self.put_json(&crate::grants::hdr_file(zone, &new_node), &header)?;
        let parent_node = NodePath::folder(zone, destination);
        let up_wrap = Wrap::seal(
            &self.did,
            &parent_node.to_string(),
            &parent_key,
            &new_node.to_string(),
            new_version,
            &new_key,
            ent.e24(),
        );
        self.put_json(
            &crate::grants::wrap_file(zone, &parent_node, &new_node),
            &up_wrap,
        )?;

        let versions = old_index
            .sections
            .iter()
            .map(|section| (section.sid.clone(), section.key_version))
            .collect::<BTreeMap<_, _>>();
        let mut rewritten = index.clone();
        for (folders, sid) in self.sections_under(zone, &new_chain, None)? {
            let rest = folders[new_chain.len()..].to_vec();
            let mut old_folders = source.clone();
            old_folders.extend_from_slice(&rest);
            let old_section = NodePath::section(zone, old_folders, sid);
            let new_section = NodePath::section(zone, folders.clone(), sid);
            let below = NodePath {
                zone,
                folders: rest,
                leaf: Leaf::Section(sid),
            };
            let old_section_key = aithos_core::derive::node_key(&old_key, &below);
            let new_section_key = aithos_core::derive::node_key(&new_key, &below);
            let stored_version = versions
                .get(&sid.to_string())
                .copied()
                .unwrap_or(crate::bundle::KV);
            let file = format!("e/circle/blobs/{sid}.enc");
            let plaintext =
                self.open_blob_v(&file, &old_section_key, &old_section, stored_version)?;
            let blob_sha = self.put_blob_v(
                &file,
                &new_section_key,
                &new_section,
                new_version,
                &plaintext,
                ent,
            )?;
            let section_row = rewritten
                .sections
                .iter_mut()
                .find(|section| section.sid == sid.to_string())
                .ok_or_else(|| Error::InvalidPath(format!("missing moved section {sid}")))?;
            section_row.key_version = new_version;
            section_row.blob_sha = blob_sha;

            for path in self.store.list("e/circle/wraps/").map_err(|error| {
                Error::SealRejected(format!("structural wrap list failed: {error}"))
            })? {
                let wrap: Wrap = self.get_json(&path)?;
                if wrap.node == old_section.to_string() {
                    self.delete_object(&path)?;
                }
            }
            let leaf = chain
                .last()
                .ok_or_else(|| Error::InvalidMandate("empty structural chain".into()))?;
            let kex = grantee_kex_secret(agent_sk);
            for (tag_header, anchor, tag) in self.structural_tag_headers(zone)? {
                if !section_row.tags.iter().any(|candidate| candidate == &tag)
                    || folders.len() < anchor.folders.len()
                    || folders[..anchor.folders.len()] != anchor.folders
                {
                    continue;
                }
                let (_, anchor_key) =
                    tag_header.open_latest(&self.did, &leaf.grantee.pubkey, &kex)?;
                let wrap = Wrap::seal(
                    &self.did,
                    &anchor.to_string(),
                    &anchor_key,
                    &new_section.to_string(),
                    new_version,
                    &new_section_key,
                    ent.e24(),
                );
                self.put_json(
                    &crate::grants::wrap_file(zone, &anchor, &new_section),
                    &wrap,
                )?;
            }
        }
        self.put_json("e/circle/index.json", &rewritten)?;
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionModify,
            &old_node,
            Some(&old_key),
            serde_json::json!({
                "destination": new_node.to_string(),
                "structural": "folder.move",
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Mutated)
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_move_folder(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folder: &str,
        destination_parent: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        if zone == Zone::Circle {
            return self.structural_move_circle_folder(
                chain,
                agent_sk,
                folder,
                destination_parent,
                now,
                ent,
            );
        }
        if zone != Zone::Public {
            return Err(Error::InvalidPath(
                "self folders are not addressable by display path".into(),
            ));
        }
        let (mut index, source, row) = self.clear_folder_row(zone, folder)?;
        let destination = self.resolve_folder(zone, destination_parent)?;
        if destination.len() >= source.len() && destination[..source.len()] == source {
            return Err(Error::InvalidPath(
                "cannot move a folder into its own descendant".into(),
            ));
        }
        self.structural_folder_gate(chain, agent_sk, Verb::Edit, zone, &source, now)?;
        self.structural_folder_gate(chain, agent_sk, Verb::Append, zone, &destination, now)?;
        let destination_sid = destination.last().map(ToString::to_string);
        if index.folders.iter().any(|candidate| {
            candidate.sid != row.sid
                && candidate.parent_sid == destination_sid
                && candidate.name == row.name
        }) {
            return Err(Error::InvalidPath(
                "destination sibling name collision".into(),
            ));
        }
        index
            .folders
            .iter_mut()
            .find(|candidate| candidate.sid == row.sid)
            .expect("resolved folder")
            .parent_sid = destination_sid;
        self.put_json("e/public/index.json", &index)?;
        let source_node = NodePath::folder(zone, source);
        let mut destination_node_chain = destination;
        destination_node_chain.push(Sid::parse(&row.sid)?);
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionModify,
            &source_node,
            None,
            serde_json::json!({
                "destination": NodePath::folder(zone, destination_node_chain).to_string(),
                "structural": "folder.move",
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Mutated)
    }

    #[allow(clippy::too_many_arguments)]
    fn structural_edit_metadata(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        section: &str,
        name: Option<&str>,
        title: Option<&str>,
        tags: Option<&[String]>,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        if zone == Zone::Self_ {
            return Err(Error::InvalidPath(
                "self metadata is available only through exact opaque content access".into(),
            ));
        }
        if let Some(name) = name {
            Self::gate_display_name(name)?;
        }
        if let Some(tags) = tags {
            for tag in tags {
                validate_tag(tag)?;
            }
        }
        let (resolved, folders) = self.resolve_clear(zone, section)?;
        let sid = Sid::parse(&resolved.sid)?;
        self.structural_section_gate(
            chain,
            agent_sk,
            Verb::Edit,
            zone,
            &folders,
            sid,
            &resolved.tags,
            now,
        )?;
        let index_path = format!("e/{}/index.json", zone.as_str());
        let mut index: ZoneIndex = self.get_json(&index_path)?;
        if let Some(new_name) = name {
            if index.sections.iter().any(|candidate| {
                candidate.sid != resolved.sid
                    && candidate.folder_sid == resolved.folder_sid
                    && candidate.name == new_name
            }) {
                return Err(Error::InvalidPath(
                    "a section with the new name already exists".into(),
                ));
            }
            if zone == Zone::Public && new_name != resolved.name {
                let body = self.get(&format!("e/public/{section}.md"))?;
                let parent = section.rsplit_once('/').map(|(parent, _)| parent);
                let display = parent
                    .map(|parent| format!("{parent}/{new_name}"))
                    .unwrap_or_else(|| new_name.to_owned());
                self.write_object(&format!("e/public/{display}.md"), &body)?;
                self.delete_object(&format!("e/public/{section}.md"))?;
            }
        }
        if zone == Zone::Circle {
            if let Some(new_tags) = tags {
                let leaf = chain
                    .last()
                    .ok_or_else(|| Error::InvalidMandate("empty structural chain".into()))?;
                let kex = grantee_kex_secret(agent_sk);
                let (version, section_key) =
                    self.agent_current_section_key(&leaf.grantee.pubkey, &kex, &folders, sid)?;
                if version != resolved.key_version {
                    return Err(Error::SealRejected(
                        "section metadata key version is not current".into(),
                    ));
                }
                self.structural_sync_metadata_tag_wraps(
                    chain,
                    agent_sk,
                    &folders,
                    sid,
                    &resolved.tags,
                    new_tags,
                    version,
                    &section_key,
                    ent,
                )?;
            }
        }
        let row = index
            .sections
            .iter_mut()
            .find(|candidate| candidate.sid == resolved.sid)
            .expect("resolved section");
        if let Some(name) = name {
            row.name = name.to_owned();
            row.sig = None;
        }
        if let Some(title) = title {
            row.title = title.to_owned();
        }
        if let Some(tags) = tags {
            row.tags = tags.to_vec();
        }
        self.put_json(&index_path, &index)?;
        let node = NodePath::section(zone, folders, sid);
        let key = self.structural_log_key(chain, agent_sk, &node)?;
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionModify,
            &node,
            key.as_ref(),
            serde_json::json!({
                "name": name,
                "structural": "section.metadata",
                "tags": tags.unwrap_or(&resolved.tags),
                "title_changed": title.is_some(),
            }),
            now,
            ent,
        )?;
        Ok(StructuralOutcome::Mutated)
    }

    /// Execute one composed structural operation.
    ///
    /// Clear-zone folder mutations and metadata changes are supported here.
    /// A circle move additionally requires both current source/destination
    /// key paths so the same actor can rotate and rewrap the changed
    /// boundary. `self` keeps using whole-zone or exact opaque-SID operations
    /// and never accepts display `dir`/`tag` claims.
    pub fn structural_operation(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        operation: StructuralOperation<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<StructuralOutcome> {
        self.transaction(|bundle| match operation {
            StructuralOperation::ListFolder { zone, folder, now } => {
                bundle.structural_list(chain, agent_sk, zone, folder, now, ent)
            }
            StructuralOperation::CreateFolder {
                zone,
                parent,
                name,
                now,
            } => bundle.structural_create_folder(chain, agent_sk, zone, parent, name, now, ent),
            StructuralOperation::RenameFolder {
                zone,
                folder,
                new_name,
                now,
            } => bundle.structural_rename_folder(chain, agent_sk, zone, folder, new_name, now, ent),
            StructuralOperation::DeleteFolder {
                zone,
                folder,
                recursive,
                now,
            } => {
                bundle.structural_delete_folder(chain, agent_sk, zone, folder, recursive, now, ent)
            }
            StructuralOperation::MoveFolder {
                zone,
                folder,
                destination_parent,
                now,
            } => bundle.structural_move_folder(
                chain,
                agent_sk,
                zone,
                folder,
                destination_parent,
                now,
                ent,
            ),
            StructuralOperation::EditSectionMetadata {
                zone,
                section,
                name,
                title,
                tags,
                now,
            } => bundle.structural_edit_metadata(
                chain, agent_sk, zone, section, name, title, tags, now, ent,
            ),
        })
    }
}

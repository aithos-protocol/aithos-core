//! Grants and delegation over the bundle (spec §04.3, §05.2): issuing a
//! mandate mints the certificate AND appends the header lines (and, for
//! dir&tag perimeters, builds the folder-local tag view with its wraps).
//! Agents read with their single keypair; the verifier gates every access.

use crate::bundle::{
    Bundle, ChildRef, Descriptor, FolderRow, GranteeContentOperation, GranteeContentOutcome,
    GranteeTarget, PublicAuthorship, SelfAccess, SelfIndex, SelfRow, SelfSection, TreeEntry,
    TreeEntryKind, ZoneIndex, KV,
};
use crate::entropy::EntropySource;
use crate::manifest::{sha256_hex, Manifest};
use crate::Store;
use aithos_core::derive::{derive_key, node_key};
use aithos_core::did::DidDocument;
use aithos_core::error::{Error, Result};
use aithos_core::header::{Header, Recipient, Wrap};
use aithos_core::ids::Sid;
use aithos_core::jcs;
use aithos_core::keys::{grantee_kex_secret, OwnerKeys};
use aithos_core::mandate::{
    covers_op, covers_section_op, Mandate, MandateSpec, Op, PerimeterEntry, SectionOp, Verb,
};
use aithos_core::path::{Leaf, NodePath, Zone};
use aithos_core::seal::{blob_aad, blob_open, blob_seal};
use aithos_core::wire;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::collections::BTreeMap;

fn io_err(error: std::io::Error) -> Error {
    Error::SealRejected(format!("store i/o: {error}"))
}

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

/// Display-level selector accepted by the generic CB8 grant façade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantSelector {
    Zone,
    Dir(String),
    Tag {
        dir: String,
        tag: String,
    },
    /// Owner-resolved display path; the signed perimeter carries only its SID.
    Id(String),
    /// Preallocated opaque `self` SID. It carries no structural selector.
    OpaqueId(Sid),
}

/// One generic grant request. Resolution and key delivery happen together.
#[derive(Debug, Clone)]
pub enum GenericGrantRequest {
    Ethos {
        verb: Verb,
        zone: Zone,
        selector: GrantSelector,
    },
    Act {
        connector: String,
        action: String,
    },
    Gamma(aithos_core::mandate::GammaQuery),
    Revoke,
}

impl GenericGrantRequest {
    #[must_use]
    pub fn ethos(verb: Verb, zone: Zone, selector: GrantSelector) -> Self {
        Self::Ethos {
            verb,
            zone,
            selector,
        }
    }

    #[must_use]
    pub fn act(connector: impl Into<String>, action: impl Into<String>) -> Self {
        Self::Act {
            connector: connector.into(),
            action: action.into(),
        }
    }

    #[must_use]
    pub fn gamma(query: aithos_core::mandate::GammaQuery) -> Self {
        Self::Gamma(query)
    }

    #[must_use]
    pub fn revoke() -> Self {
        Self::Revoke
    }
}

/// Exact physical key consequence of one generic grant request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantLineKind {
    None,
    ZoneRoot,
    Folder,
    ZoneTagView,
    FolderTagView,
    Section,
    ConnectorVault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantDelivery {
    pub authority: String,
    pub kind: GrantLineKind,
    pub node: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GenericGrantOutcome {
    pub mandate: Mandate,
    pub deliveries: Vec<GrantDelivery>,
}

#[derive(Debug, Clone)]
struct ResolvedClearSection {
    row: crate::bundle::SectionRow,
    folders: Vec<Sid>,
    display_path: String,
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
        Self::gate_display_path(display, true)?;
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

    fn put_self_access(
        &mut self,
        sid: Sid,
        key: &[u8; 32],
        actual_node: &NodePath,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let opaque_node = NodePath::section(Zone::Self_, Vec::new(), sid);
        let nonce = ent.e24();
        let aad = blob_aad(&self.did, &opaque_node.to_string(), KV);
        let ciphertext = blob_seal(key, actual_node.to_string().as_bytes(), &nonce, &aad);
        let mut index: SelfIndex = self.get_json("e/self/index.json")?;
        let row = index
            .blobs
            .iter_mut()
            .find(|row| row.sid == sid.to_string())
            .ok_or_else(|| Error::InvalidPath(format!("no self SID {sid}")))?;
        row.access = Some(SelfAccess {
            n: hex::encode(nonce),
            c: hex::encode(ciphertext),
        });
        self.put_json("e/self/index.json", &index)
    }

    pub(crate) fn open_self_access(&self, sid: Sid, key: &[u8; 32]) -> Result<NodePath> {
        let index: SelfIndex = self.get_json("e/self/index.json")?;
        let access = index
            .blobs
            .iter()
            .find(|row| row.sid == sid.to_string())
            .and_then(|row| row.access.as_ref())
            .ok_or_else(|| Error::SealRejected(format!("no exact self access for {sid}")))?;
        let nonce: [u8; 24] = hex::decode(&access.n)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| Error::SealRejected("invalid self access nonce".into()))?;
        let ciphertext = hex::decode(&access.c)
            .map_err(|_| Error::SealRejected("invalid self access ciphertext".into()))?;
        let opaque_node = NodePath::section(Zone::Self_, Vec::new(), sid);
        let aad = blob_aad(&self.did, &opaque_node.to_string(), KV);
        let plaintext = blob_open(key, &ciphertext, &nonce, &aad)?;
        let target = std::str::from_utf8(&plaintext)
            .map_err(|_| Error::SealRejected("self access target is not UTF-8".into()))?;
        let node = NodePath::parse(target)?;
        if node.zone != Zone::Self_ || node.leaf != Leaf::Section(sid) {
            return Err(Error::SealRejected(
                "self access target does not match its opaque SID".into(),
            ));
        }
        Ok(node)
    }

    fn deliver_exact_section(
        &mut self,
        owner: &OwnerKeys,
        recipient: &Recipient,
        zone: Zone,
        folders: &[Sid],
        sid: Sid,
        ent: &mut dyn EntropySource,
    ) -> Result<NodePath> {
        let actual_node = NodePath::section(zone, folders.to_vec(), sid);
        let key = match zone {
            Zone::Circle => self.owner_current_section_key(owner, folders, sid)?.1,
            Zone::Self_ => node_key(&self.zone_dk(zone, owner)?, &actual_node),
            Zone::Public => {
                return Err(Error::InvalidPath(
                    "public sections require no delivered key line".into(),
                ));
            }
        };
        let node = if zone == Zone::Self_ {
            self.put_self_access(sid, &key, &actual_node, ent)?;
            NodePath::section(zone, Vec::new(), sid)
        } else {
            actual_node
        };
        self.add_line_on(&node, &key, recipient, ent)?;
        Ok(node)
    }

    fn deliver_preallocated_self(
        &mut self,
        owner: &OwnerKeys,
        recipient: &Recipient,
        sid: Sid,
        ent: &mut dyn EntropySource,
    ) -> Result<NodePath> {
        let node = NodePath::section(Zone::Self_, Vec::new(), sid);
        let key = node_key(&self.zone_dk(Zone::Self_, owner)?, &node);
        self.add_line_on(&node, &key, recipient, ent)?;
        Ok(node)
    }

    fn deliver_connector_line(
        &mut self,
        owner: &OwnerKeys,
        recipient: &Recipient,
        connector: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<String> {
        Self::gate_display_name(connector)?;
        let node = format!("/x/{connector}");
        let file = format!("e/x/{connector}/header.json");
        match self.store.get(&file).map_err(io_err)? {
            Some(bytes) => {
                let mut header: Header = serde_json::from_slice(&bytes)
                    .map_err(|error| Error::SealRejected(format!("{file}: {error}")))?;
                let version = header.latest_version();
                let key = header.open(&self.did, version, "owner-kex", &owner.owner_kex)?;
                header.append_line(&self.did, version, &key, recipient, ent.e32(), ent.e24())?;
                self.put_json(&file, &header)?;
            }
            None => {
                // Config is a random connector-local capability, not a KDF
                // descendant of the historical audit root. An audit holder
                // therefore cannot derive it.
                let key = ent.e32();
                let header = Header::build(
                    &self.did,
                    &node,
                    &key,
                    &[self.owner_kex_recipient()?, recipient.clone()],
                    &[ent.e32(), ent.e32()],
                    &[ent.e24(), ent.e24()],
                )?;
                self.put_json(&file, &header)?;
            }
        }
        Ok(node)
    }

    #[allow(clippy::too_many_arguments)]
    /// Resolve one owner grant, write its certificate and deliver every
    /// required protected key, without publishing a manifest. This is the
    /// assembly half used by clients that must place the grant and its Gamma
    /// event inside the same externally signed draft.2 package.
    pub fn prepare_generic_grant(
        &mut self,
        owner: &OwnerKeys,
        label: &str,
        agent_pub: &VerifyingKey,
        requests: &[GenericGrantRequest],
        not_before: &str,
        not_after: &str,
        issue_depth: u32,
        ent: &mut dyn EntropySource,
    ) -> Result<GenericGrantOutcome> {
        let recipient = agent_recipient(agent_pub);
        let mut perimeter = Vec::new();
        let mut deliveries = Vec::new();
        for request in requests {
            let (entry, kind, node) = match request {
                GenericGrantRequest::Gamma(query) => (
                    PerimeterEntry::Gamma {
                        dir: query.dir.clone(),
                        id: query.id,
                        tag: query.tag.clone(),
                        kind: query.kind.clone(),
                        action: query.action.clone(),
                        since: query.since.clone(),
                        until: query.until.clone(),
                    },
                    GrantLineKind::None,
                    None,
                ),
                GenericGrantRequest::Revoke => (
                    PerimeterEntry::Revoke { scope: None },
                    GrantLineKind::None,
                    None,
                ),
                GenericGrantRequest::Act { connector, action } => {
                    Self::gate_display_name(connector)?;
                    Self::gate_display_name(action)?;
                    let entry = PerimeterEntry::Act {
                        connector: connector.clone(),
                        action: Some(action.clone()),
                    };
                    if action == "config" {
                        let node =
                            self.deliver_connector_line(owner, &recipient, connector, ent)?;
                        (entry, GrantLineKind::ConnectorVault, Some(node))
                    } else {
                        (entry, GrantLineKind::None, None)
                    }
                }
                GenericGrantRequest::Ethos {
                    verb,
                    zone,
                    selector,
                } => match selector {
                    GrantSelector::Zone => {
                        let entry = PerimeterEntry::Ethos {
                            verb: *verb,
                            zone: *zone,
                            dir: Vec::new(),
                            tag: None,
                        };
                        if *zone == Zone::Public {
                            (entry, GrantLineKind::None, None)
                        } else {
                            self.deliver_entry(owner, &recipient, *zone, &[], None, ent)?;
                            (
                                entry,
                                GrantLineKind::ZoneRoot,
                                Some(NodePath::zone_root(*zone).to_string()),
                            )
                        }
                    }
                    GrantSelector::Dir(display) => {
                        let dir = if *zone == Zone::Self_ {
                            self.resolve_self_folder(display, &owner.owner_kex)?
                        } else {
                            self.resolve_folder(*zone, display)?
                        };
                        let entry = PerimeterEntry::Ethos {
                            verb: *verb,
                            zone: *zone,
                            dir: dir.clone(),
                            tag: None,
                        };
                        if *zone == Zone::Public {
                            (entry, GrantLineKind::None, None)
                        } else {
                            self.deliver_entry(owner, &recipient, *zone, &dir, None, ent)?;
                            (
                                entry,
                                GrantLineKind::Folder,
                                Some(NodePath::folder(*zone, dir).to_string()),
                            )
                        }
                    }
                    GrantSelector::Tag { dir: display, tag } => {
                        if *zone == Zone::Self_ {
                            return Err(Error::InvalidPath(
                                "self tag delivery requires an exact id in CB8".into(),
                            ));
                        }
                        let dir = self.resolve_folder(*zone, display)?;
                        let entry = PerimeterEntry::Ethos {
                            verb: *verb,
                            zone: *zone,
                            dir: dir.clone(),
                            tag: Some(tag.clone()),
                        };
                        if *zone == Zone::Public {
                            (entry, GrantLineKind::None, None)
                        } else {
                            self.deliver_entry(owner, &recipient, *zone, &dir, Some(tag), ent)?;
                            let kind = if dir.is_empty() {
                                GrantLineKind::ZoneTagView
                            } else {
                                GrantLineKind::FolderTagView
                            };
                            (
                                entry,
                                kind,
                                Some(NodePath::tag_view(*zone, dir, tag)?.to_string()),
                            )
                        }
                    }
                    GrantSelector::Id(display_path) => {
                        let (folders, sid) = if *zone == Zone::Self_ {
                            self.self_resolve(display_path, &owner.owner_kex)?
                        } else if *zone == Zone::Public
                            && self
                                .store
                                .get("indices/public.json")
                                .map_err(io_err)?
                                .is_some()
                        {
                            let index: crate::bundle::K1cPublicIndex =
                                self.get_json("indices/public.json")?;
                            let matching = index
                                .sections
                                .iter()
                                .filter(|row| row.path == *display_path)
                                .collect::<Vec<_>>();
                            let [row] = matching.as_slice() else {
                                return Err(Error::InvalidPath(format!(
                                    "K1-C public path is absent or ambiguous: {display_path}"
                                )));
                            };
                            (Vec::new(), Sid::parse(&row.sid)?)
                        } else {
                            let (row, folders) = self.resolve_clear(*zone, display_path)?;
                            (folders, Sid::parse(&row.sid)?)
                        };
                        let entry = PerimeterEntry::EthosId {
                            verb: *verb,
                            zone: *zone,
                            id: sid,
                        };
                        if *zone == Zone::Public {
                            (entry, GrantLineKind::None, None)
                        } else {
                            let node = self.deliver_exact_section(
                                owner, &recipient, *zone, &folders, sid, ent,
                            )?;
                            (entry, GrantLineKind::Section, Some(node.to_string()))
                        }
                    }
                    GrantSelector::OpaqueId(sid) => {
                        if *zone != Zone::Self_ {
                            return Err(Error::InvalidPath(
                                "opaque preallocated ids are self-only".into(),
                            ));
                        }
                        let entry = PerimeterEntry::EthosId {
                            verb: *verb,
                            zone: *zone,
                            id: *sid,
                        };
                        let node = self.deliver_preallocated_self(owner, &recipient, *sid, ent)?;
                        (entry, GrantLineKind::Section, Some(node.to_string()))
                    }
                },
            };
            let authority = entry.to_entry_string();
            perimeter.push(entry);
            deliveries.push(GrantDelivery {
                authority,
                kind,
                node,
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
        let certificate = aithos_core::jcs::canonical_bytes(&mandate)?;
        self.write_object(&format!("certs/{}.json", mandate.id), &certificate)?;
        Ok(GenericGrantOutcome {
            mandate,
            deliveries,
        })
    }

    /// Resolve, deliver, journal and publish one generic owner grant.
    #[allow(clippy::too_many_arguments)]
    pub fn grant_generic(
        &mut self,
        owner: &OwnerKeys,
        label: &str,
        agent_pub: &VerifyingKey,
        requests: &[GenericGrantRequest],
        not_before: &str,
        not_after: &str,
        issue_depth: u32,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<GenericGrantOutcome> {
        self.transaction(|bundle| {
            let outcome = bundle.prepare_generic_grant(
                owner,
                label,
                agent_pub,
                requests,
                not_before,
                not_after,
                issue_depth,
                ent,
            )?;
            bundle.log_owner_grant(owner, &outcome.mandate.id, now, ent)?;
            bundle.publish(owner, now)?;
            Ok(outcome)
        })
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
        let certificate = aithos_core::jcs::canonical_bytes(&mandate)?;
        self.write_object(&format!("certs/{}.json", mandate.id), &certificate)?;
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
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        let sid = Sid::parse(&row.sid)?;
        self.check_grantee_section(
            chain,
            agent_sk,
            Verb::Read,
            zone,
            sid,
            &folders,
            &row.tags,
            at,
            false,
        )?;

        let leaf = chain.last().expect("non-empty chain");
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
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

    /// Delegated-session read (gateway lot 1): the CERTIFICATE half is
    /// the session chain (leaf grantee = the session signer, usually
    /// the gateway key), verified exactly like an agent read; the
    /// PHYSICS half may come from any of the `physics` candidates —
    /// the session key's own line when one was delivered, or the
    /// custodian agent's line (the gateway holds both keys). Whatever
    /// candidate opens the body, the authority cited is the session
    /// chain and nothing else.
    #[allow(clippy::too_many_lines)]
    pub fn read_section_as_delegated_session(
        &self,
        chain: &[Mandate],
        session_sk: &SigningKey,
        physics: &[(String, x25519_dalek::StaticSecret)],
        zone: Zone,
        display_path: &str,
        at: &str,
    ) -> Result<String> {
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        let sid = Sid::parse(&row.sid)?;
        self.check_grantee_section(
            chain,
            session_sk,
            Verb::Read,
            zone,
            sid,
            &folders,
            &row.tags,
            at,
            false,
        )?;
        let leaf = chain.last().expect("non-empty chain");
        let section = NodePath::section(zone, folders.clone(), sid);
        let mut k_section = None;
        for (kid, kex) in physics {
            if let Ok(k) = self.agent_section_key(kid, kex, &folders, sid, row.key_version) {
                k_section = Some(k);
                break;
            }
            // Tag views granted to this leaf whose dir covers the section.
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
                let Ok(anchor_key) = self.agent_node_key(kid, kex, &anchor) else {
                    continue;
                };
                let wrap: Wrap = self.get_json(&wrap_file(zone, &anchor, &section))?;
                k_section = Some(wrap.open(&self.did, &anchor_key)?);
                break;
            }
            if k_section.is_some() {
                break;
            }
        }
        let k_section = k_section
            .ok_or_else(|| Error::SealRejected(format!("no key path to {display_path}")))?;
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

    /// Read one opaque self section by stable SID under the grantee's current
    /// exact mandate, without resolving or exposing the sealed self tree.
    pub fn read_self_section_as_agent(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        sid: Sid,
        at: &str,
    ) -> Result<String> {
        self.check_grantee_section(
            chain,
            agent_sk,
            Verb::Read,
            Zone::Self_,
            sid,
            &[],
            &[],
            at,
            false,
        )?;
        let (node, key) = self.self_section_with_agent(chain, agent_sk, sid)?;
        let plaintext = self.open_blob(&format!("e/self/blobs/{sid}.enc"), &key, &node)?;
        let section: SelfSection = serde_json::from_slice(&plaintext)
            .map_err(|error| Error::SealRejected(format!("self section: {error}")))?;
        Ok(section.md)
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
        let certificate = aithos_core::jcs::canonical_bytes(&child)?;
        self.write_object(&format!("certs/{}.json", child.id), &certificate)?;
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
            authorship: None,
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

    // ------------------------------------------ CB9 delegated content API

    pub(crate) fn verify_current_grantee(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        now: &str,
    ) -> Result<Vec<PerimeterEntry>> {
        let doc = self.did_doc()?;
        aithos_core::mandate::verify_chain_revocable(
            chain,
            &doc,
            now,
            &self.active_revocations()?,
        )?;
        for mandate in chain {
            aithos_core::constraints::verify_operation_constraints(&mandate.constraints)?;
        }
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty chain".into()))?;
        if leaf.grantee_pub()? != agent_sk.verifying_key() {
            return Err(Error::InvalidMandate(
                "the operation key is not the leaf grantee key".into(),
            ));
        }
        leaf.parsed_perimeter()
    }

    fn check_grantee_folder(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folders: &[Sid],
        now: &str,
    ) -> Result<()> {
        let perimeter = self.verify_current_grantee(chain, agent_sk, now)?;
        if !covers_op(
            &perimeter,
            &Op {
                verb: Verb::Read,
                zone,
                folders,
                tags: &[],
            },
        ) {
            return Err(Error::InvalidMandate(
                "list is not covered by the leaf perimeter".into(),
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn check_grantee_section(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        verb: Verb,
        zone: Zone,
        sid: Sid,
        folders: &[Sid],
        tags: &[String],
        now: &str,
        mutation: bool,
    ) -> Result<()> {
        let perimeter = self.verify_current_grantee(chain, agent_sk, now)?;
        let covered = covers_section_op(
            &perimeter,
            &SectionOp {
                verb,
                zone,
                sid,
                folders,
                tags,
            },
        );
        let self_operation = SectionOp {
            verb,
            zone,
            sid,
            folders,
            tags,
        };
        let self_mutation_is_narrow = !mutation
            || zone != Zone::Self_
            || perimeter.iter().any(|entry| match entry {
                PerimeterEntry::Ethos {
                    zone: granted_zone,
                    dir,
                    tag,
                    ..
                } => {
                    *granted_zone == Zone::Self_
                        && dir.is_empty()
                        && tag.is_none()
                        && covers_section_op(std::slice::from_ref(entry), &self_operation)
                }
                PerimeterEntry::EthosId {
                    zone: granted_zone,
                    id,
                    ..
                } => {
                    *granted_zone == Zone::Self_
                        && *id == sid
                        && covers_section_op(std::slice::from_ref(entry), &self_operation)
                }
                _ => false,
            });
        if !covered || !self_mutation_is_narrow {
            return Err(Error::InvalidMandate(
                "content operation is not covered by the leaf perimeter".into(),
            ));
        }
        Ok(())
    }

    fn resolved_clear_section(
        &self,
        zone: Zone,
        target: GranteeTarget<'_>,
    ) -> Result<ResolvedClearSection> {
        if zone == Zone::Self_ {
            return Err(Error::InvalidPath(
                "self sections use an opaque id target".into(),
            ));
        }
        match target {
            GranteeTarget::Display(display_path) => {
                let (row, folders) = self.resolve_clear(zone, display_path)?;
                Ok(ResolvedClearSection {
                    row,
                    folders,
                    display_path: display_path.to_owned(),
                })
            }
            GranteeTarget::Id(sid) => {
                let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
                let row = index
                    .sections
                    .iter()
                    .find(|row| row.sid == sid.to_string())
                    .cloned()
                    .ok_or_else(|| Error::InvalidPath(format!("no section SID {sid}")))?;
                let mut reverse = Vec::new();
                let mut cursor = row.folder_sid.clone();
                while let Some(folder_sid) = cursor {
                    let folder = index
                        .folders
                        .iter()
                        .find(|folder| folder.sid == folder_sid)
                        .ok_or_else(|| {
                            Error::InvalidPath(format!("dangling folder SID {folder_sid}"))
                        })?;
                    reverse.push((Sid::parse(&folder.sid)?, folder.name.clone()));
                    cursor = folder.parent_sid.clone();
                }
                reverse.reverse();
                let folders = reverse.iter().map(|(sid, _)| *sid).collect::<Vec<_>>();
                let mut names = reverse
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>();
                names.push(row.name.clone());
                Ok(ResolvedClearSection {
                    row,
                    folders,
                    display_path: names.join("/"),
                })
            }
            GranteeTarget::FolderIds(_) => Err(Error::InvalidPath(
                "a section target cannot be a folder chain".into(),
            )),
        }
    }

    fn resolved_clear_folder(
        &self,
        zone: Zone,
        target: GranteeTarget<'_>,
    ) -> Result<(Vec<Sid>, String)> {
        match target {
            GranteeTarget::Display(display) => {
                Ok((self.resolve_folder(zone, display)?, display.to_owned()))
            }
            GranteeTarget::FolderIds(folders) => Ok((folders.to_vec(), String::new())),
            GranteeTarget::Id(_) => Err(Error::InvalidPath(
                "a folder target cannot be a section id".into(),
            )),
        }
    }

    fn clear_entries_below(&self, zone: Zone, display: &str) -> Result<Vec<TreeEntry>> {
        let prefix = if display.is_empty() {
            String::new()
        } else {
            format!("{display}/")
        };
        Ok(self
            .clear_zone_entries(zone)?
            .into_iter()
            .filter_map(|mut entry| {
                if display.is_empty() {
                    return Some(entry);
                }
                if entry.path == display {
                    return None;
                }
                let relative = entry.path.strip_prefix(&prefix)?.to_owned();
                entry.path = relative;
                Some(entry)
            })
            .collect())
    }

    fn self_descriptor_with_agent_key(
        &self,
        chain: &[Sid],
        content_key: &[u8; 32],
    ) -> Result<Descriptor> {
        if chain.is_empty() {
            let root_key = derive_key("aithos-core/v1/self-root", content_key);
            self.read_desc(
                "e/self/root.enc",
                &root_key,
                &NodePath::zone_root(Zone::Self_),
            )
        } else {
            let node = NodePath::folder(Zone::Self_, chain.to_vec());
            self.read_desc(
                &format!(
                    "e/self/blobs/{}.enc",
                    chain.last().expect("non-empty chain")
                ),
                content_key,
                &node,
            )
        }
    }

    fn self_list_with_agent_key(
        &self,
        chain: &[Sid],
        content_key: &[u8; 32],
        prefix: &str,
        out: &mut Vec<TreeEntry>,
    ) -> Result<()> {
        let descriptor = self.self_descriptor_with_agent_key(chain, content_key)?;
        for child in descriptor.children {
            let sid = Sid::parse(&child.sid)?;
            if !self.self_sid_active(sid)? {
                continue;
            }
            if child.kind == "d" {
                let relative = NodePath {
                    zone: Zone::Self_,
                    folders: vec![sid],
                    leaf: Leaf::Folder,
                };
                let child_key = node_key(content_key, &relative);
                let mut child_chain = chain.to_vec();
                child_chain.push(sid);
                let child_descriptor =
                    self.self_descriptor_with_agent_key(&child_chain, &child_key)?;
                let path = format!("{prefix}{}", child_descriptor.name);
                out.push(TreeEntry {
                    path: path.clone(),
                    kind: TreeEntryKind::Folder,
                });
                self.self_list_with_agent_key(&child_chain, &child_key, &format!("{path}/"), out)?;
            } else {
                let relative = NodePath {
                    zone: Zone::Self_,
                    folders: Vec::new(),
                    leaf: Leaf::Section(sid),
                };
                let section_key = node_key(content_key, &relative);
                let node = NodePath::section(Zone::Self_, chain.to_vec(), sid);
                let plaintext =
                    self.open_blob(&format!("e/self/blobs/{sid}.enc"), &section_key, &node)?;
                let section: SelfSection = serde_json::from_slice(&plaintext)
                    .map_err(|error| Error::SealRejected(format!("self section: {error}")))?;
                out.push(TreeEntry {
                    path: format!("{prefix}{}", section.name),
                    kind: TreeEntryKind::Section,
                });
            }
        }
        Ok(())
    }

    fn find_self_with_zone_key(
        &self,
        wanted: Sid,
        chain: &[Sid],
        content_key: &[u8; 32],
    ) -> Result<Option<(NodePath, [u8; 32])>> {
        let descriptor = self.self_descriptor_with_agent_key(chain, content_key)?;
        for child in descriptor.children {
            let sid = Sid::parse(&child.sid)?;
            if !self.self_sid_active(sid)? {
                continue;
            }
            if child.kind == "s" {
                let relative = NodePath {
                    zone: Zone::Self_,
                    folders: Vec::new(),
                    leaf: Leaf::Section(sid),
                };
                let key = node_key(content_key, &relative);
                if sid == wanted {
                    return Ok(Some((
                        NodePath::section(Zone::Self_, chain.to_vec(), sid),
                        key,
                    )));
                }
            } else {
                let relative = NodePath {
                    zone: Zone::Self_,
                    folders: vec![sid],
                    leaf: Leaf::Folder,
                };
                let key = node_key(content_key, &relative);
                let mut child_chain = chain.to_vec();
                child_chain.push(sid);
                if let Some(found) = self.find_self_with_zone_key(wanted, &child_chain, &key)? {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }

    fn self_section_with_agent(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        sid: Sid,
    ) -> Result<(NodePath, [u8; 32])> {
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidMandate("empty chain".into()))?;
        let kid = &leaf.grantee.pubkey;
        let kex = grantee_kex_secret(agent_sk);
        let opaque = NodePath::section(Zone::Self_, Vec::new(), sid);
        if let Ok(key) = self.agent_node_key(kid, &kex, &opaque) {
            if let Ok(actual) = self.open_self_access(sid, &key) {
                return Ok((actual, key));
            }
            if self.self_sid_active(sid)? {
                return Ok((opaque, key));
            }
        }
        let zone = NodePath::zone_root(Zone::Self_);
        let zone_key = self.agent_node_key(kid, &kex, &zone)?;
        self.find_self_with_zone_key(sid, &[], &zone_key)?
            .ok_or_else(|| Error::SealRejected(format!("self SID {sid} is unreachable")))
    }

    fn sign_public_authorship(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        sid: Sid,
        content_hash: &str,
    ) -> Result<PublicAuthorship> {
        let latest: Manifest = self.get_json("manifest.json")?;
        let via = chain
            .iter()
            .map(|mandate| mandate.id.clone())
            .collect::<Vec<_>>();
        let key = wire::ed25519_pub_to_multibase(&agent_sk.verifying_key().to_bytes());
        let reference_seed = serde_json::json!({
            "authorized_via": via,
            "content_hash": content_hash,
            "edition": latest.edition.height + 1,
            "key": key,
            "sid": sid.to_string(),
            "subject": self.did,
            "zone": "public",
        });
        let operation_ref = format!(
            "sha256:{}",
            sha256_hex(&jcs::canonical_bytes(&reference_seed)?)
        );
        let mut authorship = PublicAuthorship {
            subject: self.did.clone(),
            zone: "public".into(),
            sid: sid.to_string(),
            content_hash: content_hash.to_owned(),
            operation_ref,
            edition: latest.edition.height + 1,
            authorized_via: chain.iter().map(|mandate| mandate.id.clone()).collect(),
            key,
            sig: String::new(),
        };
        authorship.sig = hex::encode(
            agent_sk
                .sign(&jcs::canonical_bytes(&authorship)?)
                .to_bytes(),
        );
        Ok(authorship)
    }

    fn public_authorship_hash(authorship: &PublicAuthorship) -> Result<String> {
        Ok(sha256_hex(&jcs::canonical_bytes(authorship)?))
    }

    /// Verify every delegated public authorship record against the pinned
    /// edition, the leaf key and its matching delegated Gamma evidence.
    pub fn verify_public_authorship(&self) -> Result<()> {
        self.gamma_verify()?;
        let latest: Manifest = self.get_json("manifest.json")?;
        let index: ZoneIndex = self.get_json("e/public/index.json")?;
        let entries = self.gamma_entries()?;
        for row in index.sections.iter().filter(|row| row.authorship.is_some()) {
            let authorship = row.authorship.as_ref().expect("filtered");
            if row.sig.is_some()
                || authorship.subject != self.did
                || authorship.zone != "public"
                || authorship.sid != row.sid
                || authorship.content_hash != row.blob_sha
                || authorship.edition != latest.edition.height
                || authorship.authorized_via.is_empty()
            {
                return Err(Error::InvalidOperation(
                    "delegated public authorship fields disagree".into(),
                ));
            }
            let key_bytes = wire::multibase_to_ed25519_pub(&authorship.key)?;
            let key = VerifyingKey::from_bytes(&key_bytes)
                .map_err(|_| Error::InvalidOperation("invalid authorship key".into()))?;
            let signature: [u8; 64] = hex::decode(&authorship.sig)
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or_else(|| Error::InvalidOperation("invalid authorship signature".into()))?;
            let mut unsigned = authorship.clone();
            unsigned.sig.clear();
            key.verify(
                &jcs::canonical_bytes(&unsigned)?,
                &Signature::from_bytes(&signature),
            )
            .map_err(|_| {
                Error::InvalidOperation("public authorship signature does not verify".into())
            })?;
            let commitment = Self::public_authorship_hash(authorship)?;
            let evidence = entries.iter().find(|entry| {
                entry
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("authorship"))
                    .and_then(serde_json::Value::as_str)
                    == Some(commitment.as_str())
            });
            let evidence = evidence.ok_or_else(|| {
                Error::InvalidOperation("public authorship is not committed by Gamma".into())
            })?;
            if evidence.authorized_via.as_ref() != Some(&authorship.authorized_via)
                || evidence.signature.key != authorship.key
            {
                return Err(Error::InvalidOperation(
                    "public authorship actor or chain disagrees with Gamma".into(),
                ));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn grantee_create_clear(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        folder: GranteeTarget<'_>,
        preallocated_sid: Option<Sid>,
        name: &str,
        title: &str,
        tags: &[String],
        body: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Sid> {
        Self::gate_display_name(name)?;
        let (folders, folder_display) = self.resolved_clear_folder(zone, folder)?;
        let sid = preallocated_sid.unwrap_or_else(|| Self::new_sid(ent));
        self.check_grantee_section(
            chain,
            agent_sk,
            Verb::Append,
            zone,
            sid,
            &folders,
            tags,
            now,
            true,
        )?;
        let display_path = if folder_display.is_empty() {
            name.to_owned()
        } else {
            format!("{folder_display}/{name}")
        };
        let node = NodePath::section(zone, folders.clone(), sid);
        match zone {
            Zone::Public => {
                let content_hash = sha256_hex(body.as_bytes());
                let authorship =
                    self.sign_public_authorship(chain, agent_sk, sid, &content_hash)?;
                let authorship_hash = Self::public_authorship_hash(&authorship)?;
                self.write_object(&format!("e/public/{display_path}.md"), body.as_bytes())?;
                let mut index: ZoneIndex = self.get_json("e/public/index.json")?;
                index.sections.push(crate::bundle::SectionRow {
                    sid: sid.to_string(),
                    name: name.to_owned(),
                    folder_sid: folders.last().map(ToString::to_string),
                    title: title.to_owned(),
                    tags: tags.to_vec(),
                    blob_sha: content_hash.clone(),
                    key_version: KV,
                    sig: None,
                    authorship: Some(authorship),
                });
                self.put_json("e/public/index.json", &index)?;
                self.log_delegated_mutation_with_key(
                    chain,
                    agent_sk,
                    aithos_core::gamma::Kind::SectionAdd,
                    &node,
                    None,
                    serde_json::json!({
                        "authorship": authorship_hash,
                        "blob_sha": content_hash,
                        "name": name,
                        "tags": tags,
                    }),
                    now,
                    ent,
                )?;
            }
            Zone::Circle => {
                let leaf = chain.last().expect("current chain checked");
                let kex = grantee_kex_secret(agent_sk);
                let (version, key) =
                    self.agent_current_section_key(&leaf.grantee.pubkey, &kex, &folders, sid)?;
                let blob = serde_json::json!({ "md": body });
                let hash = self.put_blob_v(
                    &format!("e/circle/blobs/{sid}.enc"),
                    &key,
                    &node,
                    version,
                    &jcs::canonical_bytes(&blob)?,
                    ent,
                )?;
                let mut index: ZoneIndex = self.get_json("e/circle/index.json")?;
                index.sections.push(crate::bundle::SectionRow {
                    sid: sid.to_string(),
                    name: name.to_owned(),
                    folder_sid: folders.last().map(ToString::to_string),
                    title: title.to_owned(),
                    tags: tags.to_vec(),
                    blob_sha: hash.clone(),
                    key_version: version,
                    sig: None,
                    authorship: None,
                });
                self.put_json("e/circle/index.json", &index)?;
                self.log_delegated_mutation_with_key(
                    chain,
                    agent_sk,
                    aithos_core::gamma::Kind::SectionAdd,
                    &node,
                    Some(&key),
                    serde_json::json!({ "blob_sha": hash, "name": name, "tags": tags }),
                    now,
                    ent,
                )?;
            }
            Zone::Self_ => unreachable!("clear create excludes self"),
        }
        Ok(sid)
    }

    #[allow(clippy::too_many_arguments)]
    fn grantee_create_self(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        folder: GranteeTarget<'_>,
        preallocated_sid: Option<Sid>,
        name: &str,
        title: &str,
        tags: &[String],
        body: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Sid> {
        let folders = match folder {
            GranteeTarget::FolderIds(folders) if folders.is_empty() => folders,
            _ => {
                return Err(Error::InvalidPath(
                    "CB9 self creation is opaque and root-anchored".into(),
                ));
            }
        };
        let sid = preallocated_sid.unwrap_or_else(|| Self::new_sid(ent));
        self.check_grantee_section(
            chain,
            agent_sk,
            Verb::Append,
            Zone::Self_,
            sid,
            folders,
            tags,
            now,
            true,
        )?;
        let leaf = chain.last().expect("current chain checked");
        let kex = grantee_kex_secret(agent_sk);
        let node = NodePath::section(Zone::Self_, Vec::new(), sid);
        let key = self.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?;
        let section = SelfSection {
            kind: "section".into(),
            name: name.to_owned(),
            title: title.to_owned(),
            tags: tags.to_vec(),
            md: body.to_owned(),
        };
        let hash = self.put_blob(
            &format!("e/self/blobs/{sid}.enc"),
            &key,
            &node,
            &jcs::canonical_bytes(&section)?,
            ent,
        )?;
        let mut index: SelfIndex = self.get_json("e/self/index.json")?;
        if index.blobs.iter().any(|row| row.sid == sid.to_string()) {
            return Err(Error::InvalidPath(format!("self SID {sid} already exists")));
        }
        index.blobs.push(SelfRow {
            sid: sid.to_string(),
            key_version: KV,
            access: None,
        });
        self.put_json("e/self/index.json", &index)?;
        let zone_node = NodePath::zone_root(Zone::Self_);
        if let Ok(zone_key) = self.agent_node_key(&leaf.grantee.pubkey, &kex, &zone_node) {
            let root_key = derive_key("aithos-core/v1/self-root", &zone_key);
            let mut root = self.read_desc("e/self/root.enc", &root_key, &zone_node)?;
            root.children.push(ChildRef {
                kind: "s".into(),
                sid: sid.to_string(),
            });
            self.write_desc("e/self/root.enc", &root_key, &zone_node, &root, ent)?;
        }
        self.log_delegated_mutation_with_key(
            chain,
            agent_sk,
            aithos_core::gamma::Kind::SectionAdd,
            &node,
            Some(&key),
            serde_json::json!({ "blob_sha": hash }),
            now,
            ent,
        )?;
        Ok(sid)
    }

    /// One operation path for delegated public/circle/self content.
    ///
    /// Every call rechecks the current chain, revocations and constraints.
    /// Reads are journalized; mutations commit state and Gamma in one CB7
    /// transaction. No owner key or owner-signature fallback is accepted.
    pub fn grantee_content_operation(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        operation: GranteeContentOperation<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<GranteeContentOutcome> {
        self.transaction(|bundle| match operation {
            GranteeContentOperation::List { target, now } => {
                if zone == Zone::Self_ {
                    let folders = match target {
                        GranteeTarget::FolderIds(folders) => folders,
                        _ => {
                            return Err(Error::InvalidPath(
                                "self listing needs an opaque folder chain".into(),
                            ));
                        }
                    };
                    bundle.check_grantee_folder(chain, agent_sk, zone, folders, now)?;
                    let leaf = chain.last().expect("current chain checked");
                    let kex = grantee_kex_secret(agent_sk);
                    let node = NodePath::folder(zone, folders.to_vec());
                    let content_key = bundle.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?;
                    let mut entries = Vec::new();
                    bundle.self_list_with_agent_key(folders, &content_key, "", &mut entries)?;
                    let log_key = if folders.is_empty() {
                        derive_key("aithos-core/v1/self-root", &content_key)
                    } else {
                        content_key
                    };
                    bundle.log_delegated_read(
                        chain,
                        agent_sk,
                        &node,
                        Some(&log_key),
                        "list",
                        now,
                        ent,
                    )?;
                    Ok(GranteeContentOutcome::Listed(entries))
                } else {
                    let (folders, display) = bundle.resolved_clear_folder(zone, target)?;
                    bundle.check_grantee_folder(chain, agent_sk, zone, &folders, now)?;
                    let node = NodePath::folder(zone, folders.clone());
                    let key = if zone == Zone::Circle {
                        let leaf = chain.last().expect("current chain checked");
                        let kex = grantee_kex_secret(agent_sk);
                        Some(bundle.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?)
                    } else {
                        None
                    };
                    let entries = bundle.clear_entries_below(zone, &display)?;
                    bundle.log_delegated_read(
                        chain,
                        agent_sk,
                        &node,
                        key.as_ref(),
                        "list",
                        now,
                        ent,
                    )?;
                    Ok(GranteeContentOutcome::Listed(entries))
                }
            }
            GranteeContentOperation::Read { target, now } => {
                if zone == Zone::Self_ {
                    let GranteeTarget::Id(sid) = target else {
                        return Err(Error::InvalidPath(
                            "self reads need an opaque section id".into(),
                        ));
                    };
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Read,
                        zone,
                        sid,
                        &[],
                        &[],
                        now,
                        false,
                    )?;
                    let (node, key) = bundle.self_section_with_agent(chain, agent_sk, sid)?;
                    let plaintext =
                        bundle.open_blob(&format!("e/self/blobs/{sid}.enc"), &key, &node)?;
                    let section: SelfSection = serde_json::from_slice(&plaintext)
                        .map_err(|error| Error::SealRejected(format!("self section: {error}")))?;
                    bundle.log_delegated_read(
                        chain,
                        agent_sk,
                        &node,
                        Some(&key),
                        "read",
                        now,
                        ent,
                    )?;
                    Ok(GranteeContentOutcome::Read(section.md))
                } else {
                    let resolved = bundle.resolved_clear_section(zone, target)?;
                    let sid = Sid::parse(&resolved.row.sid)?;
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Read,
                        zone,
                        sid,
                        &resolved.folders,
                        &resolved.row.tags,
                        now,
                        false,
                    )?;
                    let node = NodePath::section(zone, resolved.folders.clone(), sid);
                    let (body, key) = if zone == Zone::Public {
                        (
                            Bundle::<S>::public_read(&bundle.store, &resolved.display_path)?,
                            None,
                        )
                    } else {
                        let leaf = chain.last().expect("current chain checked");
                        let kex = grantee_kex_secret(agent_sk);
                        let key = bundle.agent_section_key(
                            &leaf.grantee.pubkey,
                            &kex,
                            &resolved.folders,
                            sid,
                            resolved.row.key_version,
                        )?;
                        let plaintext = bundle.open_blob_v(
                            &format!("e/circle/blobs/{sid}.enc"),
                            &key,
                            &node,
                            resolved.row.key_version,
                        )?;
                        let value: serde_json::Value =
                            serde_json::from_slice(&plaintext).map_err(|error| {
                                Error::SealRejected(format!("circle blob: {error}"))
                            })?;
                        (
                            value["md"].as_str().unwrap_or_default().to_owned(),
                            Some(key),
                        )
                    };
                    bundle.log_delegated_read(
                        chain,
                        agent_sk,
                        &node,
                        key.as_ref(),
                        "read",
                        now,
                        ent,
                    )?;
                    Ok(GranteeContentOutcome::Read(body))
                }
            }
            GranteeContentOperation::Create {
                folder,
                preallocated_sid,
                name,
                title,
                tags,
                body,
                now,
            } => {
                let sid = if zone == Zone::Self_ {
                    bundle.grantee_create_self(
                        chain,
                        agent_sk,
                        folder,
                        preallocated_sid,
                        name,
                        title,
                        tags,
                        body,
                        now,
                        ent,
                    )?
                } else {
                    bundle.grantee_create_clear(
                        chain,
                        agent_sk,
                        zone,
                        folder,
                        preallocated_sid,
                        name,
                        title,
                        tags,
                        body,
                        now,
                        ent,
                    )?
                };
                Ok(GranteeContentOutcome::Created(sid))
            }
            GranteeContentOperation::Edit { target, body, now } => {
                if zone == Zone::Self_ {
                    let GranteeTarget::Id(sid) = target else {
                        return Err(Error::InvalidPath(
                            "self edits need an opaque section id".into(),
                        ));
                    };
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Edit,
                        zone,
                        sid,
                        &[],
                        &[],
                        now,
                        true,
                    )?;
                    let (node, key) = bundle.self_section_with_agent(chain, agent_sk, sid)?;
                    let plaintext =
                        bundle.open_blob(&format!("e/self/blobs/{sid}.enc"), &key, &node)?;
                    let mut section: SelfSection = serde_json::from_slice(&plaintext)
                        .map_err(|error| Error::SealRejected(format!("self section: {error}")))?;
                    section.md = body.to_owned();
                    let hash = bundle.put_blob(
                        &format!("e/self/blobs/{sid}.enc"),
                        &key,
                        &node,
                        &jcs::canonical_bytes(&section)?,
                        ent,
                    )?;
                    bundle.log_delegated_mutation_with_key(
                        chain,
                        agent_sk,
                        aithos_core::gamma::Kind::SectionModify,
                        &node,
                        Some(&key),
                        serde_json::json!({ "blob_sha": hash }),
                        now,
                        ent,
                    )?;
                } else {
                    let resolved = bundle.resolved_clear_section(zone, target)?;
                    let sid = Sid::parse(&resolved.row.sid)?;
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Edit,
                        zone,
                        sid,
                        &resolved.folders,
                        &resolved.row.tags,
                        now,
                        true,
                    )?;
                    let node = NodePath::section(zone, resolved.folders.clone(), sid);
                    if zone == Zone::Public {
                        let content_hash = sha256_hex(body.as_bytes());
                        let authorship =
                            bundle.sign_public_authorship(chain, agent_sk, sid, &content_hash)?;
                        let authorship_hash = Self::public_authorship_hash(&authorship)?;
                        bundle.write_object(
                            &format!("e/public/{}.md", resolved.display_path),
                            body.as_bytes(),
                        )?;
                        let mut index: ZoneIndex = bundle.get_json("e/public/index.json")?;
                        let row = index
                            .sections
                            .iter_mut()
                            .find(|row| row.sid == sid.to_string())
                            .ok_or_else(|| Error::InvalidPath(format!("no section SID {sid}")))?;
                        row.blob_sha = content_hash.clone();
                        row.sig = None;
                        row.authorship = Some(authorship);
                        bundle.put_json("e/public/index.json", &index)?;
                        bundle.log_delegated_mutation_with_key(
                            chain,
                            agent_sk,
                            aithos_core::gamma::Kind::SectionModify,
                            &node,
                            None,
                            serde_json::json!({
                                "authorship": authorship_hash,
                                "blob_sha": content_hash,
                                "tags": resolved.row.tags,
                            }),
                            now,
                            ent,
                        )?;
                    } else {
                        let leaf = chain.last().expect("current chain checked");
                        let kex = grantee_kex_secret(agent_sk);
                        let (version, key) = bundle.agent_current_section_key(
                            &leaf.grantee.pubkey,
                            &kex,
                            &resolved.folders,
                            sid,
                        )?;
                        let hash = bundle.put_blob_v(
                            &format!("e/circle/blobs/{sid}.enc"),
                            &key,
                            &node,
                            version,
                            &jcs::canonical_bytes(&serde_json::json!({ "md": body }))?,
                            ent,
                        )?;
                        let mut index: ZoneIndex = bundle.get_json("e/circle/index.json")?;
                        let row = index
                            .sections
                            .iter_mut()
                            .find(|row| row.sid == sid.to_string())
                            .ok_or_else(|| Error::InvalidPath(format!("no section SID {sid}")))?;
                        row.blob_sha = hash.clone();
                        row.key_version = version;
                        row.sig = None;
                        row.authorship = None;
                        bundle.put_json("e/circle/index.json", &index)?;
                        bundle.log_delegated_mutation_with_key(
                            chain,
                            agent_sk,
                            aithos_core::gamma::Kind::SectionModify,
                            &node,
                            Some(&key),
                            serde_json::json!({
                                "blob_sha": hash,
                                "tags": resolved.row.tags,
                            }),
                            now,
                            ent,
                        )?;
                    }
                }
                Ok(GranteeContentOutcome::Mutated)
            }
            GranteeContentOperation::Delete { target, now } => {
                if zone == Zone::Self_ {
                    let GranteeTarget::Id(sid) = target else {
                        return Err(Error::InvalidPath(
                            "self deletes need an opaque section id".into(),
                        ));
                    };
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Delete,
                        zone,
                        sid,
                        &[],
                        &[],
                        now,
                        true,
                    )?;
                    let (node, key) = bundle.self_section_with_agent(chain, agent_sk, sid)?;
                    let mut index: SelfIndex = bundle.get_json("e/self/index.json")?;
                    index.blobs.retain(|row| row.sid != sid.to_string());
                    bundle.put_json("e/self/index.json", &index)?;
                    bundle.log_delegated_mutation_with_key(
                        chain,
                        agent_sk,
                        aithos_core::gamma::Kind::SectionDelete,
                        &node,
                        Some(&key),
                        serde_json::json!({}),
                        now,
                        ent,
                    )?;
                } else {
                    let resolved = bundle.resolved_clear_section(zone, target)?;
                    let sid = Sid::parse(&resolved.row.sid)?;
                    bundle.check_grantee_section(
                        chain,
                        agent_sk,
                        Verb::Delete,
                        zone,
                        sid,
                        &resolved.folders,
                        &resolved.row.tags,
                        now,
                        true,
                    )?;
                    let node = NodePath::section(zone, resolved.folders, sid);
                    let key = if zone == Zone::Circle {
                        let leaf = chain.last().expect("current chain checked");
                        let kex = grantee_kex_secret(agent_sk);
                        Some(bundle.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?)
                    } else {
                        None
                    };
                    let index_path = format!("e/{}/index.json", zone.as_str());
                    let mut index: ZoneIndex = bundle.get_json(&index_path)?;
                    index.sections.retain(|row| row.sid != sid.to_string());
                    bundle.put_json(&index_path, &index)?;
                    if zone == Zone::Public {
                        bundle.delete_object(&format!("e/public/{}.md", resolved.display_path))?;
                    }
                    bundle.log_delegated_mutation_with_key(
                        chain,
                        agent_sk,
                        aithos_core::gamma::Kind::SectionDelete,
                        &node,
                        key.as_ref(),
                        serde_json::json!({
                            "name": resolved.row.name,
                            "tags": resolved.row.tags,
                        }),
                        now,
                        ent,
                    )?;
                }
                Ok(GranteeContentOutcome::Mutated)
            }
        })
    }
}

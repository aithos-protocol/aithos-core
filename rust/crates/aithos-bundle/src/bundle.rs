//! Bundle orchestration (spec §02.3): indexes, sealed blobs, headers,
//! DID document and the signed edition chain, over any [`Store`].
//!
//! Blob files are `nonce(24) ‖ ciphertext`. Randomness and timestamps are
//! always injected by the caller.

use crate::entropy::EntropySource;
use crate::manifest::{sha256_hex, Manifest};
use crate::Store;
use aithos_core::derive::node_key;
use aithos_core::did::DidDocument;
use aithos_core::error::{Error, Result};
use aithos_core::header::{Header, Recipient};
use aithos_core::ids::Sid;
use aithos_core::jcs;
use aithos_core::keys::OwnerKeys;
use aithos_core::path::{NodePath, Zone};
use aithos_core::seal::{blob_aad, blob_open, blob_seal};
use ed25519_dalek::Signer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const KV: u64 = 1; // single key version until step G (revocation rotates)

// ---------------------------------------------------------------- indexes

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ZoneIndex {
    pub folders: Vec<FolderRow>,
    pub sections: Vec<SectionRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderRow {
    pub sid: String,
    pub name: String,
    pub parent_sid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionRow {
    pub sid: String,
    pub name: String,
    pub folder_sid: Option<String>,
    pub title: String,
    pub tags: Vec<String>,
    pub blob_sha: String,
    pub key_version: u64,
    /// Owner content signature (§02.11) — in the open for public rows only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SelfIndex {
    pub blobs: Vec<SelfRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfRow {
    pub sid: String,
    pub key_version: u64,
}

/// Sealed `self` folder descriptor (§02.8).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Descriptor {
    kind: String,
    name: String,
    children: Vec<ChildRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChildRef {
    kind: String, // "d" | "s"
    sid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelfSection {
    kind: String,
    name: String,
    title: String,
    tags: Vec<String>,
    md: String,
}

// ----------------------------------------------------------------- bundle

/// Parameters of one section creation (the step-F params struct).
#[derive(Debug, Clone, Copy)]
pub struct SectionSpec<'a> {
    pub zone: Zone,
    pub folder_path: &'a str,
    pub name: &'a str,
    pub title: &'a str,
    pub tags: &'a [String],
    pub body: &'a str,
    /// Injected timestamp of the mutation's gamma entry (§07.1).
    pub now: &'a str,
}

pub struct Bundle<S: Store> {
    pub store: S,
    pub did: String,
}

impl<S: Store> std::fmt::Debug for Bundle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bundle({})", self.did)
    }
}

fn io_err(e: std::io::Error) -> Error {
    Error::SealRejected(format!("store i/o: {e}"))
}

fn zone_str(zone: Zone) -> &'static str {
    zone.as_str()
}

fn owner_content_sig(
    owner: &OwnerKeys,
    zone: Zone,
    display_path: &str,
    sid: &str,
    body: &str,
) -> Result<String> {
    // §02.11: owner signatures cover JCS of {zone, path, sid, body_hash}.
    let payload = serde_json::json!({
        "body_hash": sha256_hex(body.as_bytes()),
        "path": display_path,
        "sid": sid,
        "zone": zone_str(zone),
    });
    Ok(hex::encode(
        owner
            .content_sign
            .sign(&jcs::canonical_bytes(&payload)?)
            .to_bytes(),
    ))
}

impl<S: Store> Bundle<S> {
    // ------------------------------------------------------------- io

    pub(crate) fn get(&self, path: &str) -> Result<Vec<u8>> {
        self.store
            .get(path)
            .map_err(io_err)?
            .ok_or_else(|| Error::SealRejected(format!("missing file: {path}")))
    }

    pub(crate) fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        serde_json::from_slice(&self.get(path)?)
            .map_err(|e| Error::SealRejected(format!("{path}: {e}")))
    }

    pub(crate) fn put_json<T: Serialize>(&mut self, path: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|e| Error::SealRejected(format!("{path}: {e}")))?;
        self.store.put(path, &bytes).map_err(io_err)
    }

    pub(crate) fn put_blob(
        &mut self,
        file: &str,
        key: &[u8; 32],
        node: &NodePath,
        plaintext: &[u8],
        ent: &mut dyn EntropySource,
    ) -> Result<String> {
        self.put_blob_v(file, key, node, KV, plaintext, ent)
    }

    /// Seal a blob at an explicit key version (rotation, §03.4/§06).
    pub(crate) fn put_blob_v(
        &mut self,
        file: &str,
        key: &[u8; 32],
        node: &NodePath,
        version: u64,
        plaintext: &[u8],
        ent: &mut dyn EntropySource,
    ) -> Result<String> {
        let nonce = ent.e24();
        let aad = blob_aad(&self.did, &node.to_string(), version);
        let c = blob_seal(key, plaintext, &nonce, &aad);
        let mut file_bytes = nonce.to_vec();
        file_bytes.extend_from_slice(&c);
        self.store.put(file, &file_bytes).map_err(io_err)?;
        Ok(sha256_hex(&file_bytes))
    }

    pub(crate) fn open_blob(&self, file: &str, key: &[u8; 32], node: &NodePath) -> Result<Vec<u8>> {
        self.open_blob_v(file, key, node, KV)
    }

    /// Open a blob sealed at an explicit key version.
    pub(crate) fn open_blob_v(
        &self,
        file: &str,
        key: &[u8; 32],
        node: &NodePath,
        version: u64,
    ) -> Result<Vec<u8>> {
        let bytes = self.get(file)?;
        if bytes.len() < 25 {
            return Err(Error::SealRejected(format!("blob too short: {file}")));
        }
        let nonce: [u8; 24] = bytes[..24].try_into().expect("checked");
        let aad = blob_aad(&self.did, &node.to_string(), version);
        blob_open(key, &bytes[24..], &nonce, &aad)
    }

    // ------------------------------------------------------------- init

    pub fn init(
        store: S,
        owner: &OwnerKeys,
        succession_pub: &ed25519_dalek::VerifyingKey,
        ent: &mut dyn EntropySource,
        now: &str,
    ) -> Result<Self> {
        let doc = DidDocument::build(
            owner,
            succession_pub,
            vec!["file://local".to_owned()],
            "gamma/".to_owned(),
        )?;
        let mut bundle = Bundle {
            store,
            did: doc.id.clone(),
        };
        bundle.put_json("did.json", &doc)?;

        // Encrypted zone roots: random DK sealed to the owner (I3).
        for zone in [Zone::Circle, Zone::Self_] {
            let dk = ent.e32();
            let node = format!("/e/{}", zone.as_str());
            let header = Header::build(
                &bundle.did.clone(),
                &node,
                &dk,
                &[Recipient::owner(owner.owner_kex_pub())],
                &[ent.e32()],
                &[ent.e24()],
            )?;
            bundle.put_json(&format!("e/{}/header.json", zone.as_str()), &header)?;
        }
        // Vault root (§08.2): audit keys for sealed action args live here.
        {
            let dk = ent.e32();
            let header = Header::build(
                &bundle.did.clone(),
                "/x",
                &dk,
                &[Recipient::owner(owner.owner_kex_pub())],
                &[ent.e32()],
                &[ent.e24()],
            )?;
            bundle.put_json("e/x/header.json", &header)?;
        }

        bundle.put_json("e/public/index.json", &ZoneIndex::default())?;
        bundle.put_json("e/circle/index.json", &ZoneIndex::default())?;
        bundle.put_json("e/self/index.json", &SelfIndex::default())?;

        // Sealed, empty self root descriptor (§02.8).
        let self_dk = bundle.zone_dk(Zone::Self_, owner)?;
        let root_key = aithos_core::derive::derive_key("aithos-core/v1/self-root", &self_dk);
        let desc = Descriptor {
            kind: "folder".to_owned(),
            name: String::new(),
            children: vec![],
        };
        let node = NodePath::zone_root(Zone::Self_);
        let pt = jcs::canonical_bytes(&desc)?;
        bundle.put_blob("e/self/root.enc", &root_key, &node, &pt, ent)?;

        bundle.publish_at(owner, now, 1)?;
        Ok(bundle)
    }

    /// Open an existing bundle: the DID document names the subject.
    pub fn open(store: S) -> Result<Self> {
        let doc: DidDocument = serde_json::from_slice(
            &store
                .get("did.json")
                .map_err(io_err)?
                .ok_or_else(|| Error::SealRejected("missing did.json".to_owned()))?,
        )
        .map_err(|e| Error::SealRejected(format!("did.json: {e}")))?;
        doc.verify()?;
        Ok(Bundle { store, did: doc.id })
    }

    pub fn zone_dk(&self, zone: Zone, owner: &OwnerKeys) -> Result<[u8; 32]> {
        let header: Header = self.get_json(&format!("e/{}/header.json", zone.as_str()))?;
        header.validate()?;
        header.open(&self.did, KV, "owner-kex", &owner.owner_kex)
    }

    /// Vault root DK (§08.2) — parent of the per-connector audit keys.
    pub fn vault_dk(&self, owner: &OwnerKeys) -> Result<[u8; 32]> {
        let header: Header = self.get_json("e/x/header.json")?;
        header.validate()?;
        header.open(&self.did, KV, "owner-kex", &owner.owner_kex)
    }

    /// Owner write-side key for a NEW circle section: the deepest ancestor
    /// folder carrying a header governs — its owner line at its LATEST
    /// version, derived down (§03.4/§02.9: a rotated or moved ancestor's
    /// fresh key must reach every new write below it). Plain zone derivation
    /// at v1 when no ancestor was ever granted or rotated.
    /// Returns `(key_version, section_key)`.
    pub(crate) fn owner_current_section_key(
        &self,
        owner: &OwnerKeys,
        folders: &[Sid],
        sid: Sid,
    ) -> Result<(u64, [u8; 32])> {
        let zone = Zone::Circle;
        for depth in (0..=folders.len()).rev() {
            let ancestor = NodePath::folder(zone, folders[..depth].to_vec());
            let file = crate::grants::hdr_file(zone, &ancestor);
            let Some(bytes) = self.store.get(&file).ok().flatten() else {
                continue;
            };
            let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
                continue;
            };
            let (v, base) = header.open_latest(&self.did, "owner-kex", &owner.owner_kex)?;
            let rest = NodePath {
                zone,
                folders: folders[depth..].to_vec(),
                leaf: aithos_core::path::Leaf::Section(sid),
            };
            return Ok((v, node_key(&base, &rest)));
        }
        Ok((
            KV,
            node_key(
                &self.zone_dk(zone, owner)?,
                &NodePath::section(zone, folders.to_vec(), sid),
            ),
        ))
    }

    // --------------------------------------------------- folders/sections

    fn new_sid(ent: &mut dyn EntropySource) -> Sid {
        Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
    }

    /// mkdir -p semantics; returns the folder sid chain.
    pub fn ensure_folder(
        &mut self,
        zone: Zone,
        display_path: &str,
        owner: &OwnerKeys,
        ent: &mut dyn EntropySource,
    ) -> Result<Vec<Sid>> {
        match zone {
            Zone::Self_ => self.ensure_self_folder(display_path, owner, ent),
            _ => {
                let index_path = format!("e/{}/index.json", zone.as_str());
                let mut index: ZoneIndex = self.get_json(&index_path)?;
                let mut parent: Option<String> = None;
                let mut chain = Vec::new();
                for seg in display_path.split('/').filter(|s| !s.is_empty()) {
                    let found = index
                        .folders
                        .iter()
                        .find(|f| f.name == seg && f.parent_sid == parent)
                        .map(|f| f.sid.clone());
                    let sid = match found {
                        Some(s) => s,
                        None => {
                            let sid = Self::new_sid(ent).to_string();
                            index.folders.push(FolderRow {
                                sid: sid.clone(),
                                name: seg.to_owned(),
                                parent_sid: parent.clone(),
                            });
                            sid
                        }
                    };
                    chain.push(Sid::parse(&sid)?);
                    parent = Some(sid);
                }
                self.put_json(&index_path, &index)?;
                Ok(chain)
            }
        }
    }

    /// One section to create — the params struct promised at step F.
    /// `now` timestamps the gamma entry every mutation MUST leave (§07.4).
    pub fn section_add(
        &mut self,
        spec: &SectionSpec<'_>,
        owner: &OwnerKeys,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let SectionSpec {
            zone,
            folder_path,
            name,
            title,
            tags,
            body,
            now,
        } = *spec;
        let display_path = if folder_path.is_empty() {
            name.to_owned()
        } else {
            format!("{folder_path}/{name}")
        };
        let (node, blob_sha) = match zone {
            Zone::Public => {
                let sid = Self::new_sid(ent);
                let folders = self.ensure_folder(zone, folder_path, owner, ent)?;
                let file = format!("e/public/{display_path}.md");
                self.store.put(&file, body.as_bytes()).map_err(io_err)?;
                let sig = owner_content_sig(owner, zone, &display_path, &sid.to_string(), body)?;
                let index_path = "e/public/index.json";
                let mut index: ZoneIndex = self.get_json(index_path)?;
                index.sections.push(SectionRow {
                    sid: sid.to_string(),
                    name: name.to_owned(),
                    folder_sid: folders.last().map(ToString::to_string),
                    title: title.to_owned(),
                    tags: tags.to_vec(),
                    blob_sha: sha256_hex(body.as_bytes()),
                    key_version: KV,
                    sig: Some(sig),
                });
                self.put_json(index_path, &index)?;
                (
                    NodePath::section(zone, folders, sid),
                    sha256_hex(body.as_bytes()),
                )
            }
            Zone::Circle => {
                let folders = self.ensure_folder(zone, folder_path, owner, ent)?;
                let sid = Self::new_sid(ent);
                let node = NodePath::section(zone, folders.clone(), sid);
                // New content seals under the governing ancestor's CURRENT
                // key at its CURRENT version (§03.4, §02.9): writing at v1
                // past a rotated or moved folder would hand the content back
                // to whoever the rotation cut.
                let (kv, key) = self.owner_current_section_key(owner, &folders, sid)?;
                let sig = owner_content_sig(owner, zone, &display_path, &sid.to_string(), body)?;
                let blob = serde_json::json!({ "md": body, "sig": sig });
                let sha = self.put_blob_v(
                    &format!("e/circle/blobs/{sid}.enc"),
                    &key,
                    &node,
                    kv,
                    &jcs::canonical_bytes(&blob)?,
                    ent,
                )?;
                let index_path = "e/circle/index.json";
                let mut index: ZoneIndex = self.get_json(index_path)?;
                index.sections.push(SectionRow {
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
                (node, sha)
            }
            Zone::Self_ => {
                let folders = self.ensure_self_folder(folder_path, owner, ent)?;
                let sid = Self::new_sid(ent);
                let node = NodePath::section(zone, folders.clone(), sid);
                let key = node_key(&self.zone_dk(zone, owner)?, &node);
                // §02.11: self content is NEVER signed — deniable by default.
                let section = SelfSection {
                    kind: "section".to_owned(),
                    name: name.to_owned(),
                    title: title.to_owned(),
                    tags: tags.to_vec(),
                    md: body.to_owned(),
                };
                let sha = self.put_blob(
                    &format!("e/self/blobs/{sid}.enc"),
                    &key,
                    &node,
                    &jcs::canonical_bytes(&section)?,
                    ent,
                )?;
                self.self_add_child(&folders, "s", &sid.to_string(), owner, ent)?;
                let mut index: SelfIndex = self.get_json("e/self/index.json")?;
                index.blobs.push(SelfRow {
                    sid: sid.to_string(),
                    key_version: KV,
                });
                self.put_json("e/self/index.json", &index)?;
                (node, sha)
            }
        };
        // §07.4: a mutation without its gamma entry is unauthorized.
        self.log_owner_mutation(
            owner,
            aithos_core::gamma::Kind::SectionAdd,
            &node,
            serde_json::json!({ "blob_sha": blob_sha, "name": name }),
            now,
            ent,
        )
    }

    // ------------------------------------------------------ self plumbing

    fn self_root_key(&self, owner: &OwnerKeys) -> Result<[u8; 32]> {
        Ok(aithos_core::derive::derive_key(
            "aithos-core/v1/self-root",
            &self.zone_dk(Zone::Self_, owner)?,
        ))
    }

    fn read_desc(&self, file: &str, key: &[u8; 32], node: &NodePath) -> Result<Descriptor> {
        serde_json::from_slice(&self.open_blob(file, key, node)?)
            .map_err(|e| Error::SealRejected(format!("descriptor: {e}")))
    }

    fn write_desc(
        &mut self,
        file: &str,
        key: &[u8; 32],
        node: &NodePath,
        desc: &Descriptor,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let pt = jcs::canonical_bytes(desc)?;
        self.put_blob(file, key, node, &pt, ent)?;
        Ok(())
    }

    fn ensure_self_folder(
        &mut self,
        display_path: &str,
        owner: &OwnerKeys,
        ent: &mut dyn EntropySource,
    ) -> Result<Vec<Sid>> {
        let self_dk = self.zone_dk(Zone::Self_, owner)?;
        let mut chain: Vec<Sid> = Vec::new();
        for seg in display_path.split('/').filter(|s| !s.is_empty()) {
            let (desc_file, desc_key, desc_node) = self.self_desc_location(&chain, owner)?;
            let mut desc = self.read_desc(&desc_file, &desc_key, &desc_node)?;
            let mut found = None;
            for child in desc.children.iter().filter(|c| c.kind == "d") {
                let child_sid = Sid::parse(&child.sid)?;
                let mut child_chain = chain.clone();
                child_chain.push(child_sid);
                let child_node = NodePath::folder(Zone::Self_, child_chain.clone());
                let child_key = node_key(&self_dk, &child_node);
                let child_desc = self.read_desc(
                    &format!("e/self/blobs/{child_sid}.enc"),
                    &child_key,
                    &child_node,
                )?;
                if child_desc.name == seg {
                    found = Some(child_sid);
                    break;
                }
            }
            let sid = match found {
                Some(s) => s,
                None => {
                    let sid = Self::new_sid(ent);
                    let mut child_chain = chain.clone();
                    child_chain.push(sid);
                    let child_node = NodePath::folder(Zone::Self_, child_chain);
                    let child_key = node_key(&self_dk, &child_node);
                    self.write_desc(
                        &format!("e/self/blobs/{sid}.enc"),
                        &child_key,
                        &child_node,
                        &Descriptor {
                            kind: "folder".to_owned(),
                            name: seg.to_owned(),
                            children: vec![],
                        },
                        ent,
                    )?;
                    let mut index: SelfIndex = self.get_json("e/self/index.json")?;
                    index.blobs.push(SelfRow {
                        sid: sid.to_string(),
                        key_version: KV,
                    });
                    self.put_json("e/self/index.json", &index)?;
                    desc.children.push(ChildRef {
                        kind: "d".to_owned(),
                        sid: sid.to_string(),
                    });
                    self.write_desc(&desc_file, &desc_key, &desc_node, &desc, ent)?;
                    sid
                }
            };
            chain.push(sid);
        }
        Ok(chain)
    }

    fn self_desc_location(
        &self,
        chain: &[Sid],
        owner: &OwnerKeys,
    ) -> Result<(String, [u8; 32], NodePath)> {
        if chain.is_empty() {
            Ok((
                "e/self/root.enc".to_owned(),
                self.self_root_key(owner)?,
                NodePath::zone_root(Zone::Self_),
            ))
        } else {
            let node = NodePath::folder(Zone::Self_, chain.to_vec());
            let key = node_key(&self.zone_dk(Zone::Self_, owner)?, &node);
            let sid = chain.last().expect("non-empty");
            Ok((format!("e/self/blobs/{sid}.enc"), key, node))
        }
    }

    fn self_add_child(
        &mut self,
        folders: &[Sid],
        kind: &str,
        sid: &str,
        owner: &OwnerKeys,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let (file, key, node) = self.self_desc_location(folders, owner)?;
        let mut desc = self.read_desc(&file, &key, &node)?;
        desc.children.push(ChildRef {
            kind: kind.to_owned(),
            sid: sid.to_owned(),
        });
        self.write_desc(&file, &key, &node, &desc, ent)
    }

    // ---------------------------------------------------------- reading

    pub(crate) fn resolve_clear(
        &self,
        zone: Zone,
        display_path: &str,
    ) -> Result<(SectionRow, Vec<Sid>)> {
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let mut segs: Vec<&str> = display_path.split('/').filter(|s| !s.is_empty()).collect();
        let name = segs
            .pop()
            .ok_or_else(|| Error::InvalidPath(display_path.to_owned()))?;
        let mut parent: Option<String> = None;
        let mut chain = Vec::new();
        for seg in segs {
            let f = index
                .folders
                .iter()
                .find(|f| f.name == seg && f.parent_sid == parent)
                .ok_or_else(|| Error::InvalidPath(format!("no folder {seg}")))?;
            chain.push(Sid::parse(&f.sid)?);
            parent = Some(f.sid.clone());
        }
        let row = index
            .sections
            .iter()
            .find(|s| s.name == name && s.folder_sid == parent)
            .ok_or_else(|| Error::InvalidPath(format!("no section {name}")))?
            .clone();
        Ok((row, chain))
    }

    pub fn read_section(
        &self,
        zone: Zone,
        display_path: &str,
        owner: &OwnerKeys,
    ) -> Result<String> {
        match zone {
            Zone::Public => Self::public_read(&self.store, display_path),
            Zone::Circle => {
                let (row, chain) = self.resolve_clear(zone, display_path)?;
                let sid = Sid::parse(&row.sid)?;
                let node = NodePath::section(zone, chain, sid);
                let key = node_key(&self.zone_dk(zone, owner)?, &node);
                let pt = self.open_blob(&format!("e/circle/blobs/{sid}.enc"), &key, &node)?;
                let v: serde_json::Value = serde_json::from_slice(&pt)
                    .map_err(|e| Error::SealRejected(format!("blob json: {e}")))?;
                Ok(v["md"].as_str().unwrap_or_default().to_owned())
            }
            Zone::Self_ => {
                let (chain, sid) = self.self_resolve(display_path, owner)?;
                let node = NodePath::section(zone, chain, sid);
                let key = node_key(&self.zone_dk(zone, owner)?, &node);
                let pt = self.open_blob(&format!("e/self/blobs/{sid}.enc"), &key, &node)?;
                let s: SelfSection = serde_json::from_slice(&pt)
                    .map_err(|e| Error::SealRejected(format!("self blob: {e}")))?;
                Ok(s.md)
            }
        }
    }

    /// Keyless public read (§02.1): resolve through the clear index, read the
    /// markdown file, check its hash against the pinned index row.
    pub fn public_read(store: &S, display_path: &str) -> Result<String> {
        let index: ZoneIndex = serde_json::from_slice(
            &store
                .get("e/public/index.json")
                .map_err(io_err)?
                .ok_or_else(|| Error::SealRejected("missing public index".to_owned()))?,
        )
        .map_err(|e| Error::SealRejected(format!("public index: {e}")))?;
        let body = store
            .get(&format!("e/public/{display_path}.md"))
            .map_err(io_err)?
            .ok_or_else(|| Error::InvalidPath(display_path.to_owned()))?;
        let name = display_path.rsplit('/').next().unwrap_or(display_path);
        let row = index
            .sections
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| Error::InvalidPath(format!("no public section {name}")))?;
        if row.blob_sha != sha256_hex(&body) {
            return Err(Error::SealRejected(format!(
                "public section {display_path} does not match its pinned hash"
            )));
        }
        String::from_utf8(body).map_err(|_| Error::SealRejected("not utf-8".to_owned()))
    }

    fn self_resolve(&self, display_path: &str, owner: &OwnerKeys) -> Result<(Vec<Sid>, Sid)> {
        let self_dk = self.zone_dk(Zone::Self_, owner)?;
        let mut segs: Vec<&str> = display_path.split('/').filter(|s| !s.is_empty()).collect();
        let name = segs
            .pop()
            .ok_or_else(|| Error::InvalidPath(display_path.to_owned()))?;
        let mut chain: Vec<Sid> = Vec::new();
        for seg in segs {
            let (file, key, node) = self.self_desc_location(&chain, owner)?;
            let desc = self.read_desc(&file, &key, &node)?;
            let mut next = None;
            for child in desc.children.iter().filter(|c| c.kind == "d") {
                let child_sid = Sid::parse(&child.sid)?;
                let mut cc = chain.clone();
                cc.push(child_sid);
                let cn = NodePath::folder(Zone::Self_, cc);
                let ck = node_key(&self_dk, &cn);
                if self
                    .read_desc(&format!("e/self/blobs/{child_sid}.enc"), &ck, &cn)?
                    .name
                    == seg
                {
                    next = Some(child_sid);
                    break;
                }
            }
            chain.push(next.ok_or_else(|| Error::InvalidPath(format!("no self folder {seg}")))?);
        }
        let (file, key, node) = self.self_desc_location(&chain, owner)?;
        let desc = self.read_desc(&file, &key, &node)?;
        for child in desc.children.iter().filter(|c| c.kind == "s") {
            let child_sid = Sid::parse(&child.sid)?;
            let sn = NodePath::section(Zone::Self_, chain.clone(), child_sid);
            let sk = node_key(&self_dk, &sn);
            let pt = self.open_blob(&format!("e/self/blobs/{child_sid}.enc"), &sk, &sn)?;
            let s: SelfSection = serde_json::from_slice(&pt)
                .map_err(|e| Error::SealRejected(format!("self blob: {e}")))?;
            if s.name == name {
                return Ok((chain, child_sid));
            }
        }
        Err(Error::InvalidPath(format!("no self section {name}")))
    }

    /// Reconstruct the display tree of a zone (owner-side).
    pub fn zone_tree(&self, zone: Zone, owner: &OwnerKeys) -> Result<Vec<String>> {
        match zone {
            Zone::Self_ => {
                let mut out = Vec::new();
                self.self_walk(&[], "", owner, &mut out)?;
                Ok(out)
            }
            _ => {
                let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
                let mut paths: BTreeMap<Option<String>, String> = BTreeMap::new();
                paths.insert(None, String::new());
                let mut out = Vec::new();
                // folders are appended parents-first by construction
                let mut known: BTreeMap<String, String> = BTreeMap::new();
                for f in &index.folders {
                    let prefix = match &f.parent_sid {
                        None => String::new(),
                        Some(p) => format!("{}/", known.get(p).cloned().unwrap_or_default()),
                    };
                    let path = format!("{prefix}{}", f.name);
                    known.insert(f.sid.clone(), path.clone());
                    out.push(path);
                }
                for s in &index.sections {
                    let prefix = match &s.folder_sid {
                        None => String::new(),
                        Some(p) => format!("{}/", known.get(p).cloned().unwrap_or_default()),
                    };
                    out.push(format!("{prefix}{}", s.name));
                }
                Ok(out)
            }
        }
    }

    fn self_walk(
        &self,
        chain: &[Sid],
        prefix: &str,
        owner: &OwnerKeys,
        out: &mut Vec<String>,
    ) -> Result<()> {
        let self_dk = self.zone_dk(Zone::Self_, owner)?;
        let (file, key, node) = self.self_desc_location(chain, owner)?;
        let desc = self.read_desc(&file, &key, &node)?;
        for child in &desc.children {
            let child_sid = Sid::parse(&child.sid)?;
            if child.kind == "d" {
                let mut cc = chain.to_vec();
                cc.push(child_sid);
                let cn = NodePath::folder(Zone::Self_, cc.clone());
                let ck = node_key(&self_dk, &cn);
                let d = self.read_desc(&format!("e/self/blobs/{child_sid}.enc"), &ck, &cn)?;
                let path = format!("{prefix}{}", d.name);
                out.push(path.clone());
                self.self_walk(&cc, &format!("{path}/"), owner, out)?;
            } else {
                let sn = NodePath::section(Zone::Self_, chain.to_vec(), child_sid);
                let sk = node_key(&self_dk, &sn);
                let pt = self.open_blob(&format!("e/self/blobs/{child_sid}.enc"), &sk, &sn)?;
                let s: SelfSection = serde_json::from_slice(&pt)
                    .map_err(|e| Error::SealRejected(format!("self blob: {e}")))?;
                out.push(format!("{prefix}{}", s.name));
            }
        }
        Ok(())
    }

    /// Rename a folder: metadata only, never re-keys (§02.9).
    pub fn rename_folder(
        &mut self,
        zone: Zone,
        display_path: &str,
        new_name: &str,
        owner: &OwnerKeys,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        match zone {
            Zone::Self_ => {
                let chain = self.ensure_self_folder(display_path, owner, ent)?;
                let (file, key, node) = self.self_desc_location(&chain, owner)?;
                let mut desc = self.read_desc(&file, &key, &node)?;
                desc.name = new_name.to_owned();
                self.write_desc(&file, &key, &node, &desc, ent)
            }
            _ => {
                let index_path = format!("e/{}/index.json", zone.as_str());
                let mut index: ZoneIndex = self.get_json(&index_path)?;
                let mut parent: Option<String> = None;
                let mut target = None;
                for seg in display_path.split('/').filter(|s| !s.is_empty()) {
                    let f = index
                        .folders
                        .iter()
                        .find(|f| f.name == seg && f.parent_sid == parent)
                        .ok_or_else(|| Error::InvalidPath(format!("no folder {seg}")))?;
                    parent = Some(f.sid.clone());
                    target = Some(f.sid.clone());
                }
                let sid = target.ok_or_else(|| Error::InvalidPath(display_path.to_owned()))?;
                for f in &mut index.folders {
                    if f.sid == sid {
                        f.name = new_name.to_owned();
                    }
                }
                self.put_json(&index_path, &index)
            }
        }
    }

    // --------------------------------------------------------- editions

    fn all_pinned_files(&self, exclude_latest: u64) -> Result<BTreeMap<String, String>> {
        let mut files = BTreeMap::new();
        for path in self.store.list("").map_err(io_err)? {
            if path == "manifest.json" || path == format!("manifests/{exclude_latest}.json") {
                continue;
            }
            files.insert(path.clone(), sha256_hex(&self.get(&path)?));
        }
        Ok(files)
    }

    fn publish_at(&mut self, owner: &OwnerKeys, now: &str, height: u64) -> Result<()> {
        let prev_hash = if height == 1 {
            String::new()
        } else {
            let prev: Manifest = self.get_json(&format!("manifests/{}.json", height - 1))?;
            prev.chain_hash()?
        };
        let files = self.all_pinned_files(height)?;
        let gamma_head = self.gamma_head()?;
        let manifest = Manifest::build(
            &owner.root_sign,
            height,
            prev_hash,
            now.to_owned(),
            files,
            gamma_head,
        )?;
        self.put_json(&format!("manifests/{height}.json"), &manifest)?;
        self.put_json("manifest.json", &manifest)
    }

    pub fn publish(&mut self, owner: &OwnerKeys, now: &str) -> Result<()> {
        let latest: Manifest = self.get_json("manifest.json")?;
        self.publish_at(owner, now, latest.edition.height + 1)
    }

    /// Offline verification: DID document, every manifest signature, the
    /// hash chain, and the pinned files of the latest edition.
    pub fn verify(&self) -> Result<()> {
        let err = |m: String| Error::SealRejected(format!("edition: {m}"));
        let doc: DidDocument = self.get_json("did.json")?;
        doc.verify()?;
        let latest: Manifest = self.get_json("manifest.json")?;
        let mut prev: Option<Manifest> = None;
        for h in 1..=latest.edition.height {
            let m: Manifest = self.get_json(&format!("manifests/{h}.json"))?;
            m.verify_signature(&doc)?;
            if m.edition.height != h {
                return Err(err(format!("height mismatch at {h}")));
            }
            match &prev {
                None => {
                    if !m.edition.prev_hash.is_empty() {
                        return Err(err("edition 1 must have an empty prev_hash".into()));
                    }
                }
                Some(p) => {
                    if m.edition.prev_hash != p.chain_hash()? {
                        return Err(err(format!("broken chain at height {h}")));
                    }
                }
            }
            prev = Some(m);
        }
        if prev.as_ref() != Some(&latest) {
            return Err(err("manifest.json is not the chain tip".into()));
        }
        // Pinned files of the latest edition.
        for (path, sha) in &latest.files {
            let bytes = self.get(path)?;
            if &sha256_hex(&bytes) != sha {
                return Err(err(format!("pinned file altered: {path}")));
            }
        }
        // No unpinned strays besides the manifest itself.
        for path in self.store.list("").map_err(io_err)? {
            if path != "manifest.json"
                && path != format!("manifests/{}.json", latest.edition.height)
                && !latest.files.contains_key(&path)
            {
                return Err(err(format!("unpinned file: {path}")));
            }
        }
        // Gamma (§02.7, §07.1): the chain verifies and the manifest pins
        // its tip — the edition and the log move together.
        let entries = self.gamma_entries()?;
        aithos_core::gamma::verify_links(&entries)?;
        if aithos_core::gamma::head(&entries)? != latest.gamma_head {
            return Err(err("manifest gamma_head does not pin the log tip".into()));
        }
        Ok(())
    }

    /// Every `self` section node, owner-side (descriptor walk, §02.8).
    pub(crate) fn self_section_nodes(&self, owner: &OwnerKeys) -> Result<Vec<NodePath>> {
        let mut out = Vec::new();
        self.self_collect_sections(&[], owner, &mut out)?;
        Ok(out)
    }

    fn self_collect_sections(
        &self,
        chain: &[Sid],
        owner: &OwnerKeys,
        out: &mut Vec<NodePath>,
    ) -> Result<()> {
        let (file, key, node) = self.self_desc_location(chain, owner)?;
        let desc = self.read_desc(&file, &key, &node)?;
        for child in &desc.children {
            let child_sid = Sid::parse(&child.sid)?;
            if child.kind == "d" {
                let mut cc = chain.to_vec();
                cc.push(child_sid);
                self.self_collect_sections(&cc, owner, out)?;
            } else {
                out.push(NodePath::section(Zone::Self_, chain.to_vec(), child_sid));
            }
        }
        Ok(())
    }
}

//! Merkle state tree over the bundle files (spec §02.10, pass H1).
//!
//! Recomputable by ANY verifier from the files alone: index rows, header
//! files and tag-view wraps go in; four roots (public, circle, self, vault)
//! come out and ride the signed manifest BESIDE the flat pins (decided
//! 2026-07-11). Node labels are sid-based — no name ever leaks into a
//! label or a self proof.

use std::collections::BTreeMap;

use aithos_core::ids::Sid;
use aithos_core::merkle::{h_leaf, mroot, mroot_path, Proof, ProofStep};
use aithos_core::path::{NodePath, Zone};
use serde_json::Value;

use aithos_core::error::{Error, Result};

use crate::bundle::{Bundle, FolderRow, SelfIndex, ZoneIndex};
use crate::grants::{hdr_file, wrap_file};
use crate::Store;

const ZEROS: [u8; 32] = [0u8; 32];

fn io_err(e: std::io::Error) -> Error {
    Error::SealRejected(format!("store: {e}"))
}

/// The full recomputed tree: per-label node hashes plus the four roots.
/// `nodes` is what the per-edition sidecar persists for root-descent diffs.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StateTree {
    /// `<zone>:<label>` → hex node hash (labels are sid/tag based).
    pub nodes: BTreeMap<String, String>,
    /// `public` / `circle` / `self` / `vault` → hex root.
    pub roots: BTreeMap<String, String>,
}

/// Internal per-zone build artifacts, kept to assemble proofs.
struct ZoneBuild {
    root: [u8; 32],
    /// folder label → sorted kid hashes (the mroot input).
    kids: BTreeMap<String, Vec<[u8; 32]>>,
    /// child label → (parent folder label, index in the parent's kids).
    place: BTreeMap<String, (String, usize)>,
    /// folder label → payload prefix bytes (`row/label ‖ header_hash`).
    prefix: BTreeMap<String, Vec<u8>>,
    nodes: BTreeMap<String, [u8; 32]>,
}

/// `(label, hash)` leaves of a flat zone, plus the zone root.
type FlatZone = (Vec<(String, [u8; 32])>, [u8; 32]);

fn jcs_of<T: serde::Serialize>(v: &T) -> Result<Vec<u8>> {
    aithos_core::jcs::canonical_bytes(v)
}

impl<S: Store> Bundle<S> {
    /// `BLAKE3(JCS(header.json))` if the node was ever granted, else zeros.
    fn header_hash_at(&self, path: &str) -> Result<[u8; 32]> {
        match self.store.get(path).map_err(io_err)? {
            None => Ok(ZEROS),
            Some(bytes) => {
                let v: Value = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::SealRejected(format!("header json: {e}")))?;
                Ok(*blake3::hash(&jcs_of(&v)?).as_bytes())
            }
        }
    }

    fn node_header_hash(&self, zone: Zone, node: &NodePath) -> Result<[u8; 32]> {
        self.header_hash_at(&hdr_file(zone, node))
    }

    /// Build one hierarchical zone (`public`/`circle`).
    fn zone_build(&self, zone: Zone) -> Result<ZoneBuild> {
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let mut all_tags: Vec<String> = index
            .sections
            .iter()
            .flat_map(|s| s.tags.iter().cloned())
            .collect();
        all_tags.sort();
        all_tags.dedup();

        let mut b = ZoneBuild {
            root: ZEROS,
            kids: BTreeMap::new(),
            place: BTreeMap::new(),
            prefix: BTreeMap::new(),
            nodes: BTreeMap::new(),
        };
        let z = zone.as_str();
        b.root = self.folder_node(zone, &index, &all_tags, None, &mut Vec::new(), &mut b, z)?;
        Ok(b)
    }

    /// Recursively hash one folder node (`parent_sid == None` = zone root).
    #[allow(clippy::too_many_arguments)]
    fn folder_node(
        &self,
        zone: Zone,
        index: &ZoneIndex,
        all_tags: &[String],
        parent_sid: Option<&FolderRow>,
        chain: &mut Vec<Sid>,
        b: &mut ZoneBuild,
        z: &str,
    ) -> Result<[u8; 32]> {
        let my_label = match parent_sid {
            None => format!("{z}:z"),
            Some(_) => format!(
                "{z}:{}",
                chain
                    .iter()
                    .map(|s| format!("d/{s}"))
                    .collect::<Vec<_>>()
                    .join("/")
            ),
        };
        let my_sid = parent_sid.map(|f| f.sid.clone());

        // (sort key, label, hash) — kind order d < s < t via the key prefix.
        let mut kids: Vec<(String, String, [u8; 32])> = Vec::new();

        let subfolders: Vec<FolderRow> = index
            .folders
            .iter()
            .filter(|f| f.parent_sid == my_sid)
            .cloned()
            .collect();
        for f in subfolders {
            chain.push(Sid::parse(&f.sid)?);
            let h = self.folder_node(zone, index, all_tags, Some(&f), chain, b, z)?;
            let label = format!(
                "{z}:{}",
                chain
                    .iter()
                    .map(|s| format!("d/{s}"))
                    .collect::<Vec<_>>()
                    .join("/")
            );
            chain.pop();
            kids.push((format!("d\u{0}{}", f.sid), label, h));
        }
        for row in index.sections.iter().filter(|s| s.folder_sid == my_sid) {
            let sid = Sid::parse(&row.sid)?;
            let node = NodePath::section(zone, chain.clone(), sid);
            let hh = self.node_header_hash(zone, &node)?;
            let mut payload = jcs_of(row)?;
            payload.extend_from_slice(&hh);
            let h = h_leaf(&payload);
            let label = format!("{my_label}/s/{}", row.sid).replace(":z/s/", ":s/");
            kids.push((format!("s\u{0}{}", row.sid), label, h));
        }
        for tag in all_tags {
            let anchor = NodePath::tag_view(zone, chain.clone(), tag)?;
            let anchor_hdr = hdr_file(zone, &anchor);
            if self.store.get(&anchor_hdr).map_err(io_err)?.is_none() {
                continue; // no tag view anchored at this folder
            }
            let hh = self.header_hash_at(&anchor_hdr)?;
            let mut wraps: Vec<(String, [u8; 32])> = Vec::new();
            for (schain, ssid) in self.sections_under(zone, chain, Some(tag))? {
                let snode = NodePath::section(zone, schain, ssid);
                let wfile = wrap_file(zone, &anchor, &snode);
                if let Some(bytes) = self.store.get(&wfile).map_err(io_err)? {
                    let v: Value = serde_json::from_slice(&bytes)
                        .map_err(|e| Error::SealRejected(format!("wrap json: {e}")))?;
                    let mut wp = ssid.to_string().into_bytes();
                    wp.push(0);
                    wp.extend_from_slice(blake3::hash(&jcs_of(&v)?).as_bytes());
                    wraps.push((ssid.to_string(), h_leaf(&wp)));
                }
            }
            wraps.sort();
            let wroot = mroot(&wraps.iter().map(|(_, h)| *h).collect::<Vec<_>>());
            let mut payload = format!("t/{tag}").into_bytes();
            payload.extend_from_slice(&hh);
            payload.extend_from_slice(&wroot);
            let h = h_leaf(&payload);
            let label = format!("{my_label}/t/{tag}").replace(":z/t/", ":t/");
            kids.push((format!("t\u{0}{tag}"), label, h));
        }

        kids.sort_by(|a, b| a.0.cmp(&b.0));
        let kid_hashes: Vec<[u8; 32]> = kids.iter().map(|(_, _, h)| *h).collect();
        for (i, (_, label, h)) in kids.iter().enumerate() {
            b.place.insert(label.clone(), (my_label.clone(), i));
            b.nodes.insert(label.clone(), *h);
        }
        b.kids.insert(my_label.clone(), kid_hashes.clone());

        // This folder's own payload prefix: row (or literal zone label) ‖ header.
        let mut prefix = match parent_sid {
            None => {
                let mut p = format!("z/{z}").into_bytes();
                let hh = self.header_hash_at(&format!("e/{z}/header.json"))?;
                p.extend_from_slice(&hh);
                p
            }
            Some(row) => {
                let node = NodePath::folder(zone, chain.clone());
                let hh = self.node_header_hash(zone, &node)?;
                let mut p = jcs_of(row)?;
                p.extend_from_slice(&hh);
                p
            }
        };
        let node_hash = {
            prefix.extend_from_slice(&mroot(&kid_hashes));
            // prefix now holds the FULL payload; re-derive the clean prefix
            let full = prefix.clone();
            prefix.truncate(full.len() - 32);
            h_leaf(&full)
        };
        b.prefix.insert(my_label.clone(), prefix);
        b.nodes.insert(my_label, node_hash);
        Ok(node_hash)
    }

    /// Flat `self` root: leaves sorted by sid, `mroot` directly. Headers of
    /// self nodes are NOT folded in H1 (outside verifiers cannot map them
    /// without the sealed descriptors — assumed debt).
    fn self_build(&self) -> Result<FlatZone> {
        let index: SelfIndex = self.get_json("e/self/index.json")?;
        let mut leaves: Vec<(String, [u8; 32])> = Vec::new();
        for row in &index.blobs {
            let mut payload = jcs_of(row)?;
            payload.extend_from_slice(&ZEROS);
            leaves.push((row.sid.clone(), h_leaf(&payload)));
        }
        leaves.sort();
        let root = mroot(&leaves.iter().map(|(_, h)| *h).collect::<Vec<_>>());
        Ok((leaves, root))
    }

    /// Flat vault root: every `e/x/**/header.json`, labeled and sorted by
    /// storage path.
    fn vault_build(&self) -> Result<FlatZone> {
        let mut leaves: Vec<(String, [u8; 32])> = Vec::new();
        for path in self.store.list("e/x/").map_err(io_err)? {
            if !path.ends_with("header.json") {
                continue;
            }
            let hh = self.header_hash_at(&path)?;
            let mut payload = path.clone().into_bytes();
            payload.push(0);
            payload.extend_from_slice(&hh);
            leaves.push((path, h_leaf(&payload)));
        }
        leaves.sort();
        let root = mroot(&leaves.iter().map(|(_, h)| *h).collect::<Vec<_>>());
        Ok((leaves, root))
    }

    /// Recompute the whole state tree from the files alone (§02.10).
    pub fn state_tree(&self) -> Result<StateTree> {
        let mut t = StateTree::default();
        for zone in [Zone::Public, Zone::Circle] {
            let zb = self.zone_build(zone)?;
            for (label, h) in &zb.nodes {
                t.nodes.insert(label.clone(), hex::encode(h));
            }
            t.roots
                .insert(zone.as_str().to_owned(), hex::encode(zb.root));
        }
        let (self_leaves, self_root) = self.self_build()?;
        for (sid, h) in &self_leaves {
            t.nodes.insert(format!("self:s/{sid}"), hex::encode(h));
        }
        t.roots.insert("self".into(), hex::encode(self_root));
        let (vault_leaves, vault_root) = self.vault_build()?;
        for (path, h) in &vault_leaves {
            t.nodes.insert(format!("vault:{path}"), hex::encode(h));
        }
        t.roots.insert("vault".into(), hex::encode(vault_root));
        Ok(t)
    }

    /// Build the v1 inclusion proof for a section (`public`/`circle`) by
    /// display path — claimed row bytes first, then sibling and parent
    /// steps to the zone root (§02.10).
    pub fn prove_section(&self, zone: Zone, display_path: &str) -> Result<Proof> {
        let (row, chain) = self.resolve_clear(zone, display_path)?;
        let zb = self.zone_build(zone)?;
        let z = zone.as_str();
        let dir = chain
            .iter()
            .map(|s| format!("d/{s}"))
            .collect::<Vec<_>>()
            .join("/");
        let label = if dir.is_empty() {
            format!("{z}:s/{}", row.sid)
        } else {
            format!("{z}:{dir}/s/{}", row.sid)
        };
        let sid = Sid::parse(&row.sid)?;
        let node = NodePath::section(zone, chain, sid);
        let hh = self.node_header_hash(zone, &node)?;
        let mut payload = jcs_of(&row)?;
        payload.extend_from_slice(&hh);
        self.prove_from(&zb, &label, payload)
    }

    /// One flat `self` proof: the claimed row plus sibling hashes only —
    /// no name, no path, no structure.
    pub fn prove_self(&self, sid: &str) -> Result<Proof> {
        let (leaves, root) = self.self_build()?;
        let idx = leaves
            .iter()
            .position(|(s, _)| s == sid)
            .ok_or_else(|| Error::InvalidPath(format!("no self blob {sid}")))?;
        let index: SelfIndex = self.get_json("e/self/index.json")?;
        let row = index
            .blobs
            .iter()
            .find(|r| r.sid == sid)
            .expect("position found above");
        let mut payload = jcs_of(row)?;
        payload.extend_from_slice(&ZEROS);
        let hashes: Vec<[u8; 32]> = leaves.iter().map(|(_, h)| *h).collect();
        Ok(Proof {
            payload: hex::encode(payload),
            steps: mroot_path(&hashes, idx),
            root: hex::encode(root),
        })
    }

    /// Climb from a labeled leaf to the zone root using the build maps.
    fn prove_from(&self, zb: &ZoneBuild, label: &str, payload: Vec<u8>) -> Result<Proof> {
        let mut steps: Vec<ProofStep> = Vec::new();
        let mut cursor = label.to_owned();
        while let Some((parent, idx)) = zb.place.get(&cursor) {
            let hashes = &zb.kids[parent];
            steps.extend(mroot_path(hashes, *idx));
            steps.push(ProofStep::Wrap {
                pre: hex::encode(&zb.prefix[parent]),
                post: String::new(),
            });
            cursor = parent.clone();
        }
        Ok(Proof {
            payload: hex::encode(payload),
            steps,
            root: hex::encode(zb.root),
        })
    }
}

/// Root-descent diff between two persisted trees: labels added, removed,
/// or changed. Ancestor folders change with their children — the caller
/// filters kinds if it wants leaves only.
#[must_use]
pub fn tree_diff(old: &StateTree, new: &StateTree) -> BTreeMap<String, &'static str> {
    let mut out = BTreeMap::new();
    for (label, h) in &new.nodes {
        match old.nodes.get(label) {
            None => {
                out.insert(label.clone(), "added");
            }
            Some(o) if o != h => {
                out.insert(label.clone(), "changed");
            }
            _ => {}
        }
    }
    for label in old.nodes.keys() {
        if !new.nodes.contains_key(label) {
            out.insert(label.clone(), "removed");
        }
    }
    out
}

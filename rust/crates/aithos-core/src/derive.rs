//! BLAKE3 one-way derivation (spec §00.3, §01.3, §02.5).
//!
//! Every context string is unique per purpose; contexts never overlap.

use crate::ids::Sid;

pub const CTX_PREFIX: &str = "aithos-core/v1/";

// §01.1 — owner key derivation contexts.
pub const CTX_ROOT_SIGN: &str = "aithos-core/v1/root-sign";
pub const CTX_CONTENT_SIGN: &str = "aithos-core/v1/content-sign";
pub const CTX_OWNER_KEX: &str = "aithos-core/v1/owner-kex";

/// One derivation step (§01.3): `child = derive(label, parent)`.
/// One-way by construction; holding a child never yields the parent.
#[must_use]
pub fn derive_key(context: &str, key_material: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key(context, key_material)
}

// §02.5 — content-tree segment labels. Labels use sids, never names,
// so renaming re-keys nothing (§02.9).

/// Label of a child folder segment: `aithos-core/v1/d/<sid>`.
#[must_use]
pub fn folder_label(sid: &Sid) -> String {
    format!("{CTX_PREFIX}d/{sid}")
}

/// Label of a section segment: `aithos-core/v1/s/<sid>`.
#[must_use]
pub fn section_label(sid: &Sid) -> String {
    format!("{CTX_PREFIX}s/{sid}")
}

/// Label of a tag-view anchor: `aithos-core/v1/t/<tag>`.
#[must_use]
pub fn tag_label(tag: &str) -> String {
    format!("{CTX_PREFIX}t/{tag}")
}

/// Walk a canonical path from the zone-root key: one derivation per segment
/// (§02.5). Reading at depth *d* costs *d* BLAKE3 calls; holding any folder's
/// key yields its entire subtree and nothing else (one-way).
#[must_use]
pub fn node_key(zone_dk: &[u8; 32], path: &crate::path::NodePath) -> [u8; 32] {
    let mut key = *zone_dk;
    for sid in &path.folders {
        key = derive_key(&folder_label(sid), &key);
    }
    match &path.leaf {
        crate::path::Leaf::Folder => key,
        crate::path::Leaf::Section(sid) => derive_key(&section_label(sid), &key),
        crate::path::Leaf::TagView(tag) => derive_key(&tag_label(tag), &key),
    }
}

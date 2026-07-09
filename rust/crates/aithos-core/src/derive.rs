//! BLAKE3 one-way derivation (spec §00.3, §01.3, §02.5).
//!
//! Every context string is unique per purpose; contexts never overlap.

use crate::ids::Sid;

pub const CTX_PREFIX: &str = "aithos-core/v1/";

// §01.1 — owner key derivation contexts.
pub const CTX_ROOT_SIGN: &str = "aithos-core/v1/root-sign";
pub const CTX_SPHERE_PUBLIC: &str = "aithos-core/v1/sphere/public";
pub const CTX_SPHERE_CIRCLE: &str = "aithos-core/v1/sphere/circle";
pub const CTX_SPHERE_SELF: &str = "aithos-core/v1/sphere/self";
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

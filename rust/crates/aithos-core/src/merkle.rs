//! Merkle state roots (spec §02.10, pass H1) — pure hashing: BLAKE3
//! domain-separated leaves and nodes, left-heavy `mroot`, and the v1 proof
//! wire. No I/O — callers feed bytes, the tree answers hashes.
//!
//! Wire conventions graved 2026-07-11: `mroot` splits left = ⌈n/2⌉; child
//! sort `d < s < t` then sid/tag; zone root uses its literal label
//! `z/<zone>` in the row's place; flat zones are `mroot(leaves)` directly.
//! Domain separation is the splicing defense: a leaf can never pose as an
//! interior node, nor the reverse.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

const LEAF_DOMAIN: &[u8] = b"aithos-core/v1/mk-leaf\x00";
const NODE_DOMAIN: &[u8] = b"aithos-core/v1/mk-node\x00";

/// The empty root — thirty-two zero bytes.
pub const EMPTY_ROOT: [u8; 32] = [0u8; 32];

/// `H_leaf(p) = BLAKE3("aithos-core/v1/mk-leaf" ‖ 0x00 ‖ p)`.
#[must_use]
pub fn h_leaf(payload: &[u8]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(LEAF_DOMAIN);
    h.update(payload);
    *h.finalize().as_bytes()
}

/// `H_node(l, r) = BLAKE3("aithos-core/v1/mk-node" ‖ 0x00 ‖ l ‖ r)`.
#[must_use]
pub fn h_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    h.update(NODE_DOMAIN);
    h.update(left);
    h.update(right);
    *h.finalize().as_bytes()
}

/// Balanced binary root over an already-sorted list: empty ⇒ zeros, one ⇒
/// itself, else `H_node` over the left-heavy split (left = ⌈n/2⌉).
#[must_use]
pub fn mroot(hashes: &[[u8; 32]]) -> [u8; 32] {
    match hashes.len() {
        0 => EMPTY_ROOT,
        1 => hashes[0],
        n => {
            let mid = n.div_ceil(2);
            h_node(&mroot(&hashes[..mid]), &mroot(&hashes[mid..]))
        }
    }
}

/// The sibling steps that carry item `idx` of an already-sorted list to
/// `mroot(hashes)` — innermost first, ready to replay.
#[must_use]
pub fn mroot_path(hashes: &[[u8; 32]], idx: usize) -> Vec<ProofStep> {
    fn rec(h: &[[u8; 32]], i: usize, out: &mut Vec<ProofStep>) {
        if h.len() <= 1 {
            return;
        }
        let mid = h.len().div_ceil(2);
        if i < mid {
            rec(&h[..mid], i, out);
            out.push(ProofStep::Node {
                side: Side::Right,
                hash: hex::encode(mroot(&h[mid..])),
            });
        } else {
            rec(&h[mid..], i - mid, out);
            out.push(ProofStep::Node {
                side: Side::Left,
                hash: hex::encode(mroot(&h[..mid])),
            });
        }
    }
    let mut out = Vec::new();
    rec(hashes, idx, &mut out);
    out
}

/// One step of the v1 proof wire (spec §02.10): a sibling hash inside a
/// balanced tree, or the parent payload folding the running hash back
/// into a leaf.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProofStep {
    /// `cur = H_node(sibling, cur)` (side "left") or `H_node(cur, sibling)`
    /// (side "right" — the sibling sits to the right).
    Node { side: Side, hash: String },
    /// `cur = H_leaf(pre ‖ cur ‖ post)` — the parent node's own payload.
    Wrap { pre: String, post: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

/// An inclusion proof: the claimed leaf payload (clear bytes, hex) and the
/// ordered steps to the pinned root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// Hex of the claimed leaf payload — `JCS(row) ‖ header_hash [‖ mroot]`.
    pub payload: String,
    pub steps: Vec<ProofStep>,
    /// Hex of the root this proof claims to reach.
    pub root: String,
}

fn hex32(s: &str, what: &str) -> Result<[u8; 32]> {
    hex::decode(s)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or_else(|| Error::MerkleProofInvalid(format!("bad {what} encoding")))
}

/// Replay the steps from a starting hash. Fail-closed on malformed input.
pub fn run_proof(start: [u8; 32], steps: &[ProofStep]) -> Result<[u8; 32]> {
    let mut cur = start;
    for step in steps {
        cur = match step {
            ProofStep::Node { side, hash } => {
                let sib = hex32(hash, "sibling hash")?;
                match side {
                    Side::Left => h_node(&sib, &cur),
                    Side::Right => h_node(&cur, &sib),
                }
            }
            ProofStep::Wrap { pre, post } => {
                let pre = hex::decode(pre)
                    .map_err(|_| Error::MerkleProofInvalid("bad wrap pre".into()))?;
                let post = hex::decode(post)
                    .map_err(|_| Error::MerkleProofInvalid("bad wrap post".into()))?;
                let mut payload = pre;
                payload.extend_from_slice(&cur);
                payload.extend_from_slice(&post);
                h_leaf(&payload)
            }
        };
    }
    Ok(cur)
}

/// Verify a v1 proof: start from the CLAIMED payload bytes — the verifier
/// recomputes the leaf itself, so a forged interior hash presented as a
/// leaf dies on the domain string — replay, compare to the pinned root.
pub fn verify_proof(proof: &Proof, pinned_root: &[u8; 32]) -> Result<()> {
    let payload = hex::decode(&proof.payload)
        .map_err(|_| Error::MerkleProofInvalid("bad payload encoding".into()))?;
    let claimed_root = hex32(&proof.root, "root")?;
    if &claimed_root != pinned_root {
        return Err(Error::MerkleProofInvalid(
            "proof root is not the pinned root".into(),
        ));
    }
    let got = run_proof(h_leaf(&payload), &proof.steps)?;
    if got != *pinned_root {
        return Err(Error::MerkleProofInvalid(
            "replayed root does not match the pinned root".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_singleton_mroot() {
        assert_eq!(mroot(&[]), EMPTY_ROOT);
        let x = h_leaf(b"x");
        assert_eq!(mroot(&[x]), x);
    }

    #[test]
    fn left_heavy_odd_split() {
        let (a, b, c) = (h_leaf(b"a"), h_leaf(b"b"), h_leaf(b"c"));
        assert_eq!(mroot(&[a, b, c]), h_node(&h_node(&a, &b), &c));
    }

    #[test]
    fn mroot_path_replays_for_every_index() {
        let hashes: Vec<[u8; 32]> = (0u8..7).map(|i| h_leaf(&[i])).collect();
        let root = mroot(&hashes);
        for (i, h) in hashes.iter().enumerate() {
            let steps = mroot_path(&hashes, i);
            assert_eq!(run_proof(*h, &steps).unwrap(), root, "index {i}");
        }
    }

    #[test]
    fn domains_separate_leaf_from_node() {
        let (a, b) = (h_leaf(b"a"), h_leaf(b"b"));
        let mut spliced = Vec::new();
        spliced.extend_from_slice(&a);
        spliced.extend_from_slice(&b);
        assert_ne!(h_leaf(&spliced), h_node(&a, &b));
    }
}

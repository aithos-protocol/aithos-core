//! ECIES line seals and symmetric wraps (spec §03.8).
//!
//! All randomness — ephemerals, nonces — is injected by the caller (§00 purity
//! rule): sealing is deterministic given its inputs, which is what lets the
//! conformance vectors cross-check randomized crypto byte-for-byte.

use crate::derive::derive_key;
use crate::error::{Error, Result};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};

const PURPOSE_HEADER_LINE: &[u8] = b"aithos-core/v1/header-line";
const PURPOSE_WRAP: &[u8] = b"aithos-core/v1/tagwrap";
const KEK_INFO: &[u8] = b"aithos-core/v1/hdr-kek";
pub const CTX_WRAP_KEY: &str = "aithos-core/v1/wrap";

fn aad(purpose: &[u8], subject_did: &str, node: &str, key_version: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(purpose.len() + subject_did.len() + node.len() + 24);
    out.extend_from_slice(purpose);
    out.push(0);
    out.extend_from_slice(subject_did.as_bytes());
    out.push(0);
    out.extend_from_slice(node.as_bytes());
    out.push(0);
    out.extend_from_slice(key_version.to_string().as_bytes());
    out
}

/// AAD of a header line (§03.8): purpose ‖ did ‖ node ‖ key_version.
#[must_use]
pub fn line_aad(subject_did: &str, node: &str, key_version: u64) -> Vec<u8> {
    aad(PURPOSE_HEADER_LINE, subject_did, node, key_version)
}

/// AAD of a wrap (§03.8): purpose ‖ did ‖ wrapped node ‖ key_version.
#[must_use]
pub fn wrap_aad(subject_did: &str, wrapped_node: &str, key_version: u64) -> Vec<u8> {
    aad(PURPOSE_WRAP, subject_did, wrapped_node, key_version)
}

fn kek(shared: &[u8; 32], epk: &XPublicKey, recipient: &XPublicKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let info = [KEK_INFO, &[0u8], epk.as_bytes(), recipient.as_bytes()].concat();
    let mut okm = [0u8; 32];
    hk.expand(&info, &mut okm).expect("32 bytes is valid");
    okm
}

/// Seal a node key to one recipient. Returns `(epk, ciphertext)`.
pub fn seal_line(
    ephemeral: &StaticSecret,
    recipient: &XPublicKey,
    dk: &[u8; 32],
    nonce: &[u8; 24],
    aad: &[u8],
) -> ([u8; 32], Vec<u8>) {
    let epk = XPublicKey::from(ephemeral);
    let shared = ephemeral.diffie_hellman(recipient).to_bytes();
    let cipher = XChaCha20Poly1305::new((&kek(&shared, &epk, recipient)).into());
    let c = cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: dk, aad })
        .expect("encryption is infallible for these sizes");
    (epk.to_bytes(), c)
}

/// Open a line with the recipient's secret. Fail-closed: any mismatch —
/// wrong key, wrong AAD (node/version/did), corrupted byte — rejects.
pub fn open_line(
    recipient_secret: &StaticSecret,
    epk: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8; 24],
    aad: &[u8],
) -> Result<[u8; 32]> {
    let epk = XPublicKey::from(*epk);
    let recipient_pub = XPublicKey::from(recipient_secret);
    let shared = recipient_secret.diffie_hellman(&epk).to_bytes();
    let cipher = XChaCha20Poly1305::new((&kek(&shared, &epk, &recipient_pub)).into());
    let pt = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::SealRejected("line does not open".to_owned()))?;
    pt.try_into()
        .map_err(|_| Error::SealRejected("bad plaintext length".to_owned()))
}

/// Symmetric wrap (tag view & up-link, §03.4/§03.8): DK' sealed under a key
/// derived from the via node's key (anchor or parent).
pub fn wrap_seal(via_key: &[u8; 32], dk: &[u8; 32], nonce: &[u8; 24], aad: &[u8]) -> Vec<u8> {
    let key = derive_key(CTX_WRAP_KEY, via_key);
    let cipher = XChaCha20Poly1305::new((&key).into());
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: dk, aad })
        .expect("encryption is infallible for these sizes")
}

pub fn wrap_open(
    via_key: &[u8; 32],
    ciphertext: &[u8],
    nonce: &[u8; 24],
    aad: &[u8],
) -> Result<[u8; 32]> {
    let key = derive_key(CTX_WRAP_KEY, via_key);
    let cipher = XChaCha20Poly1305::new((&key).into());
    let pt = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::SealRejected("wrap does not open".to_owned()))?;
    pt.try_into()
        .map_err(|_| Error::SealRejected("bad plaintext length".to_owned()))
}

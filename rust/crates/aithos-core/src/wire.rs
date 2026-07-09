//! Wire encodings — **frozen at step 0** (docs/EXECUTION-PLAN.md).
//!
//! Public keys travel as multibase base58btc (`z…`) over a multicodec
//! prefix, did:key style: `z6Mk…` for Ed25519, `z6LS…` for X25519.
//! Hex is reserved for internal vector plumbing; anything signed,
//! published, or stored uses these encodings.

use crate::error::{Error, Result};

const ED25519_PUB: [u8; 2] = [0xed, 0x01];
const X25519_PUB: [u8; 2] = [0xec, 0x01];

fn encode(prefix: [u8; 2], key: &[u8; 32]) -> String {
    let mut bytes = Vec::with_capacity(34);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(key);
    format!("z{}", bs58::encode(bytes).into_string())
}

fn decode(prefix: [u8; 2], s: &str) -> Result<[u8; 32]> {
    let err = || Error::InvalidMultibase(s.to_owned());
    let b58 = s.strip_prefix('z').ok_or_else(err)?;
    let bytes = bs58::decode(b58).into_vec().map_err(|_| err())?;
    if bytes.len() != 34 || bytes[..2] != prefix {
        return Err(err());
    }
    Ok(bytes[2..].try_into().expect("length checked"))
}

/// Ed25519 public key → `z6Mk…` (multicodec `ed25519-pub`, 0xed01).
#[must_use]
pub fn ed25519_pub_to_multibase(key: &[u8; 32]) -> String {
    encode(ED25519_PUB, key)
}

/// X25519 public key → `z6LS…` (multicodec `x25519-pub`, 0xec01).
#[must_use]
pub fn x25519_pub_to_multibase(key: &[u8; 32]) -> String {
    encode(X25519_PUB, key)
}

/// Fail-closed decode: wrong base, wrong length, or wrong codec → rejected.
pub fn multibase_to_ed25519_pub(s: &str) -> Result<[u8; 32]> {
    decode(ED25519_PUB, s)
}

pub fn multibase_to_x25519_pub(s: &str) -> Result<[u8; 32]> {
    decode(X25519_PUB, s)
}

/// `did:aithos:<multibase(root_sign.pub)>` (spec §01.4).
#[must_use]
pub fn did_aithos(root_sign_pub: &[u8; 32]) -> String {
    format!("did:aithos:{}", ed25519_pub_to_multibase(root_sign_pub))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_both_codecs() {
        let key = [42u8; 32];
        let ed = ed25519_pub_to_multibase(&key);
        let x = x25519_pub_to_multibase(&key);
        assert!(ed.starts_with("z6Mk"), "ed25519 prefix: {ed}");
        assert!(x.starts_with("z6LS"), "x25519 prefix: {x}");
        assert_eq!(multibase_to_ed25519_pub(&ed).unwrap(), key);
        assert_eq!(multibase_to_x25519_pub(&x).unwrap(), key);
    }

    #[test]
    fn decode_fails_closed() {
        let key = [42u8; 32];
        let ed = ed25519_pub_to_multibase(&key);
        // wrong codec for the requested kind
        assert!(multibase_to_x25519_pub(&ed).is_err());
        // missing multibase prefix
        assert!(multibase_to_ed25519_pub(&ed[1..]).is_err());
        // corrupted payload
        assert!(multibase_to_ed25519_pub("z6Mk-not-base58-!!").is_err());
        // truncated
        let short = encode(super::ED25519_PUB, &key);
        assert!(multibase_to_ed25519_pub(&short[..short.len() - 4]).is_err());
    }
}

//! RFC 8785 (JCS) canonicalization — **frozen at step 0**.
//!
//! The ONLY serialization ever signed or hashed (spec §00.3). Every
//! signature and every hash in the protocol goes through [`canonicalize`];
//! no ad-hoc `serde_json::to_string` may ever be signed.

use crate::error::{Error, Result};
use serde::Serialize;

/// Canonical JSON per RFC 8785: sorted keys, ECMAScript number formatting,
/// minimal escapes. Deterministic across platforms and languages.
pub fn canonicalize<T: Serialize>(value: &T) -> Result<String> {
    serde_jcs::to_string(value).map_err(|e| Error::Canonicalization(e.to_string()))
}

/// Canonical bytes, ready for hashing or signing.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    canonicalize(value).map(String::into_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_recursively() {
        let v = json!({"b": 2, "a": 1, "nested": {"z": null, "y": [true, false]}});
        assert_eq!(
            canonicalize(&v).unwrap(),
            r#"{"a":1,"b":2,"nested":{"y":[true,false],"z":null}}"#
        );
    }

    #[test]
    fn numbers_follow_ecmascript_formatting() {
        // RFC 8785 §3.2.2.3: serialize like ECMAScript Number#toString.
        let v = json!({"a": 4.50, "b": 2e-3, "c": 1e30, "d": 10.0});
        assert_eq!(
            canonicalize(&v).unwrap(),
            r#"{"a":4.5,"b":0.002,"c":1e+30,"d":10}"#
        );
    }

    #[test]
    fn stable_across_insertion_order() {
        let v1 = json!({"x": 1, "y": 2});
        let v2 = json!({"y": 2, "x": 1});
        assert_eq!(canonicalize(&v1).unwrap(), canonicalize(&v2).unwrap());
    }
}

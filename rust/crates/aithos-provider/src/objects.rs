//! Object storage seam — the `/t/<tenant>/<did>/<chemin>` layout.
//!
//! **P1 shape:** an in-process map, enough for the signed hello and the
//! byte-exact vector replay. P2 replaces the backend with the S3 layout
//! (`modules/store-api` provisions the bucket) behind this same seam,
//! plus the A.4 artifact verification and the A.5 CAS heads — the seam
//! deliberately has no conditional write yet, so nothing in P1 can
//! pretend to be the CAS.
//!
//! The store holds ciphertext and public artifacts as opaque bytes: it
//! never parses a blob, never rewrites an artifact (doctrine §3.1).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// Object-safe async seam, house style (`credentials.rs`).
pub trait ObjectStore: Send + Sync {
    /// Read the object at `(tenant, did, chemin)`; `None` = absent.
    fn get<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + 'a>>;

    /// Store the object, byte-preserved. P1 callers only reach this after
    /// the envelope verified and the pathmap covered the write.
    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

    /// Every stored chemin of one `(tenant, did)`, sorted lexicographic
    /// (étape 5 — the `?list=` and `/sync` surfaces; the S3 backend maps
    /// this onto ListObjectsV2). A plain read, DELIBERATELY no filter
    /// semantics here: the coarse perimeter filtering is the path-map's
    /// (`covers()`), never the storage layer's.
    fn list<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
    ) -> Pin<Box<dyn Future<Output = Vec<String>> + Send + 'a>>;
}

/// The P1 in-memory backend. Per-instance and ephemeral by design — a dev
/// skeleton, stated as such in the deployment README; S3 lands with P2.
#[derive(Default)]
pub struct MemObjects {
    map: Mutex<HashMap<(String, String, String), Vec<u8>>>,
}

impl MemObjects {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ObjectStore for MemObjects {
    fn get<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
    ) -> Pin<Box<dyn Future<Output = Option<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            self.map
                .lock()
                .expect("object map poisoned")
                .get(&(tenant.to_owned(), did.to_owned(), chemin.to_owned()))
                .cloned()
        })
    }

    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            self.map.lock().expect("object map poisoned").insert(
                (tenant.to_owned(), did.to_owned(), chemin.to_owned()),
                bytes,
            );
        })
    }

    fn list<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
    ) -> Pin<Box<dyn Future<Output = Vec<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut paths: Vec<String> = self
                .map
                .lock()
                .expect("object map poisoned")
                .keys()
                .filter(|(t, d, _)| t == tenant && d == did)
                .map(|(_, _, chemin)| chemin.clone())
                .collect();
            paths.sort();
            paths
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_roundtrip_byte_preserved() {
        let store = MemObjects::new();
        let bytes = b"# hello\n".to_vec();
        futures::executor::block_on(store.put(
            "acme",
            "did:aithos:zX",
            "e/public/hello.md",
            bytes.clone(),
        ));
        assert_eq!(
            futures::executor::block_on(store.get("acme", "did:aithos:zX", "e/public/hello.md")),
            Some(bytes)
        );
        // Absent object, absent tenant, absent DID: all None — the caller
        // turns that into `not_found` only inside a covered perimeter.
        assert_eq!(
            futures::executor::block_on(store.get("acme", "did:aithos:zX", "e/public/other.md")),
            None
        );
        assert_eq!(
            futures::executor::block_on(store.get("ghost", "did:aithos:zX", "e/public/hello.md")),
            None
        );
    }
}

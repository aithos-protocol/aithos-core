//! Object storage seam — the `/t/<tenant>/<did>/<chemin>` layout.
//!
//! **Étape 6 shape:** the seam speaks `Result` — a backend that cannot
//! answer refuses (`503 unavailable`, the nonce precedent), it NEVER
//! invents an absence (fail-closed doctrine). Two backends live behind
//! it: the in-process map (dev/tests, replay) and the S3 layout
//! (`modules/store-api` provisions the bucket; key = `t/<tenant>/<did>/
//! <chemin>`). The seam still deliberately has no head CAS — the A.5
//! serialization point is [`crate::heads::HeadsTable`] alone; the only
//! conditional write here is [`ObjectStore::put_once`], the ⑧b
//! write-once of immutable classes (arbitrage gate 4, acté 2026-07-20).
//!
//! The store holds ciphertext and public artifacts as opaque bytes: it
//! never parses a blob, never rewrites an artifact (doctrine §3.1).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// The backend cannot answer. Fixed cause only: backend/SDK messages can
/// carry request detail, they never cross into our error type (A.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreUnavailable;

/// Boxed seam future, house style — also what keeps clippy's
/// type-complexity lint honest about the object-safe signatures.
pub type StoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, StoreUnavailable>> + Send + 'a>>;

/// The ⑧b write-once verdict of [`ObjectStore::put_once`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutOnce {
    /// Nothing was stored under this chemin: the object is now stored.
    Stored,
    /// The stored object already carries EXACTLY these bytes: idempotent
    /// accept, nothing rewritten.
    Identical,
    /// The stored object differs byte-wise: the deposit refuses —
    /// an immutable object never changes under its name (⑧b).
    Conflict,
}

/// Object-safe async seam, house style (`credentials.rs`).
pub trait ObjectStore: Send + Sync {
    /// Read the object at `(tenant, did, chemin)`; `Ok(None)` = absent,
    /// `Err` = the backend could not answer (the caller refuses 503 —
    /// an unreachable store never fabricates a `not_found`).
    fn get<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
    ) -> StoreFuture<'a, Option<Vec<u8>>>;

    /// Store the object, byte-preserved. Callers only reach this after
    /// the envelope verified and the pathmap covered the write.
    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, ()>;

    /// The ⑧b write-once deposit: store iff nothing is stored under this
    /// chemin; an identical re-deposit is idempotent; different bytes
    /// conflict. The S3 backend backs the absence check with a
    /// conditional `If-None-Match` PUT — multi-instance safe without ever
    /// pretending to be the A.5 CAS.
    fn put_once<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, PutOnce>;

    /// Every stored chemin of one `(tenant, did)`, sorted lexicographic
    /// (étape 5 — the `?list=` and `/sync` surfaces; the S3 backend maps
    /// this onto ListObjectsV2). A plain read, DELIBERATELY no filter
    /// semantics here: the coarse perimeter filtering is the path-map's
    /// (`covers()`), never the storage layer's.
    fn list<'a>(&'a self, tenant: &'a str, did: &'a str) -> StoreFuture<'a, Vec<String>>;
}

/// The in-memory backend. Per-instance and ephemeral by design — dev,
/// tests and the byte-exact replay; S3 is the deployed backend (étape 6).
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
    ) -> StoreFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            Ok(self
                .map
                .lock()
                .expect("object map poisoned")
                .get(&(tenant.to_owned(), did.to_owned(), chemin.to_owned()))
                .cloned())
        })
    }

    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, ()> {
        Box::pin(async move {
            self.map.lock().expect("object map poisoned").insert(
                (tenant.to_owned(), did.to_owned(), chemin.to_owned()),
                bytes,
            );
            Ok(())
        })
    }

    fn put_once<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, PutOnce> {
        Box::pin(async move {
            let mut map = self.map.lock().expect("object map poisoned");
            let key = (tenant.to_owned(), did.to_owned(), chemin.to_owned());
            Ok(match map.get(&key) {
                None => {
                    map.insert(key, bytes);
                    PutOnce::Stored
                }
                Some(stored) if *stored == bytes => PutOnce::Identical,
                Some(_) => PutOnce::Conflict,
            })
        })
    }

    fn list<'a>(&'a self, tenant: &'a str, did: &'a str) -> StoreFuture<'a, Vec<String>> {
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
            Ok(paths)
        })
    }
}

/// The deployed backend (étape 6): the S3 layout `t/<tenant>/<did>/
/// <chemin>` in one versioned bucket (module Terraform `store-api`,
/// DR = A5 same-region versioning). Proven against the real service at
/// the deploy gate — in-process suites run on [`MemObjects`], the seams
/// carry the contract.
pub struct S3Objects {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3Objects {
    pub fn new(client: aws_sdk_s3::Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    fn key(tenant: &str, did: &str, chemin: &str) -> String {
        format!("t/{tenant}/{did}/{chemin}")
    }
}

impl ObjectStore for S3Objects {
    fn get<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
    ) -> StoreFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            use aws_sdk_s3::error::SdkError;
            use aws_sdk_s3::operation::get_object::GetObjectError;
            let got = self
                .client
                .get_object()
                .bucket(&self.bucket)
                .key(Self::key(tenant, did, chemin))
                .send()
                .await;
            match got {
                Ok(out) => match out.body.collect().await {
                    Ok(data) => Ok(Some(data.into_bytes().to_vec())),
                    Err(_) => Err(StoreUnavailable),
                },
                Err(SdkError::ServiceError(e))
                    if matches!(e.err(), GetObjectError::NoSuchKey(_)) =>
                {
                    Ok(None)
                }
                Err(_) => Err(StoreUnavailable),
            }
        })
    }

    fn put<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, ()> {
        Box::pin(async move {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(Self::key(tenant, did, chemin))
                .body(aws_sdk_s3::primitives::ByteStream::from(bytes))
                .send()
                .await
                .map(|_| ())
                .map_err(|_| StoreUnavailable)
        })
    }

    fn put_once<'a>(
        &'a self,
        tenant: &'a str,
        did: &'a str,
        chemin: &'a str,
        bytes: Vec<u8>,
    ) -> StoreFuture<'a, PutOnce> {
        Box::pin(async move {
            // `If-None-Match: *` — the store-side insert-if-absent. On the
            // precondition failure the stored object is re-read and
            // compared byte-wise: identical = idempotent, else conflict.
            let put = self
                .client
                .put_object()
                .bucket(&self.bucket)
                .key(Self::key(tenant, did, chemin))
                .if_none_match("*")
                .body(aws_sdk_s3::primitives::ByteStream::from(bytes.clone()))
                .send()
                .await;
            match put {
                Ok(_) => Ok(PutOnce::Stored),
                Err(err) => {
                    let precondition = err
                        .raw_response()
                        .is_some_and(|r| r.status().as_u16() == 412);
                    if !precondition {
                        return Err(StoreUnavailable);
                    }
                    match self.get(tenant, did, chemin).await? {
                        Some(stored) if stored == bytes => Ok(PutOnce::Identical),
                        // Absent-after-412 is a raced delete: refuse
                        // rather than guess (fail-closed).
                        _ => Ok(PutOnce::Conflict),
                    }
                }
            }
        })
    }

    fn list<'a>(&'a self, tenant: &'a str, did: &'a str) -> StoreFuture<'a, Vec<String>> {
        Box::pin(async move {
            let prefix = format!("t/{tenant}/{did}/");
            let mut paths = Vec::new();
            let mut token: Option<String> = None;
            loop {
                let page = self
                    .client
                    .list_objects_v2()
                    .bucket(&self.bucket)
                    .prefix(&prefix)
                    .set_continuation_token(token.take())
                    .send()
                    .await
                    .map_err(|_| StoreUnavailable)?;
                for object in page.contents() {
                    if let Some(key) = object.key() {
                        if let Some(chemin) = key.strip_prefix(&prefix) {
                            paths.push(chemin.to_owned());
                        }
                    }
                }
                match page.next_continuation_token() {
                    Some(next) => token = Some(next.to_owned()),
                    None => break,
                }
            }
            // ListObjectsV2 pages are already UTF-8-lexicographic; the
            // sort keeps the seam's promise independent of the backend.
            paths.sort();
            Ok(paths)
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
        ))
        .unwrap();
        assert_eq!(
            futures::executor::block_on(store.get("acme", "did:aithos:zX", "e/public/hello.md")),
            Ok(Some(bytes))
        );
        // Absent object, absent tenant, absent DID: all None — the caller
        // turns that into `not_found` only inside a covered perimeter.
        assert_eq!(
            futures::executor::block_on(store.get("acme", "did:aithos:zX", "e/public/other.md")),
            Ok(None)
        );
        assert_eq!(
            futures::executor::block_on(store.get("ghost", "did:aithos:zX", "e/public/hello.md")),
            Ok(None)
        );
    }

    #[test]
    fn put_once_is_write_once() {
        let store = MemObjects::new();
        let first = b"{\"a\":1}".to_vec();
        assert_eq!(
            futures::executor::block_on(store.put_once(
                "acme",
                "did:aithos:zX",
                "certs/m.json",
                first.clone()
            )),
            Ok(PutOnce::Stored)
        );
        // Identical bytes: idempotent, nothing rewritten.
        assert_eq!(
            futures::executor::block_on(store.put_once(
                "acme",
                "did:aithos:zX",
                "certs/m.json",
                first.clone()
            )),
            Ok(PutOnce::Identical)
        );
        // Different bytes under the same name: the ⑧b conflict.
        assert_eq!(
            futures::executor::block_on(store.put_once(
                "acme",
                "did:aithos:zX",
                "certs/m.json",
                b"{ \"a\": 1 }".to_vec()
            )),
            Ok(PutOnce::Conflict)
        );
        assert_eq!(
            futures::executor::block_on(store.get("acme", "did:aithos:zX", "certs/m.json")),
            Ok(Some(first))
        );
    }
}

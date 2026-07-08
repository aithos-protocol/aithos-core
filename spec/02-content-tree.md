# 2 — Content tree

> **Status: DRAFT.** Zones, sections, tags, how they map to nodes and keys, and how
> editions chain without a server.

## 2.1 Nodes and paths

The content tree, with canonical paths (also the derivation labels):

```
/e/public                     zone (plaintext — no key, no header)
/e/circle                     zone node (DK, header)
/e/circle/ns/<ns>             namespace subtree (derived key)
/e/circle/t/<tag>             tag view (derived key; bridges via tag wraps §03)
/e/circle/s/<id>              section (derived key; via ns node if id is namespaced)
/e/self …                     same shape as circle
/x/<connector>                vault node (DK, header) — §08
```

`public` is plaintext by design; it has section addressability but no cryptography.
`circle` and `self` are encrypted zones, each a node with its own DK and header.

## 2.2 Sections

A section is `{id, title, tags[], body}`. `id ∈ [a-z0-9_:-]{1,64}`, at most one `:`
splitting an optional namespace (`<ns>:<local>`, `ns ∈ [a-z0-9_-]{1,32}`). A
namespaced id MUST derive through its `ns` node (§2.5) — this is what makes `#ns=`
grants and namespace-scoped authoring work. Titles/tags: `public`/`circle` store them
clear in the index; `self` seals them inside the blob.

## 2.3 Bundle layout

```
manifest.json            signed, linear-chained (§2.6)
did.json                 §01.4
e/public/<id>.md         plaintext sections
e/circle/index.json      clear index: [{id,title,tags,blob_sha,key_version,gamma_ref}]
e/circle/<id>.enc        AEAD blob (purpose "blob")
e/circle/header.json     §03
e/self/index.json        reduced index: [{id, key_version, gamma_ref}] (no title/tags)
e/self/<id>.enc          AEAD blob; title+tags sealed inside
e/self/header.json
x/<id>/…                 vault, §08
certs/<mandate_id>.json  §04 (public)
gamma/gamma.jsonl        §07
```

Sharding of large indexes is permitted (deterministic, by `sha256(id)`) but omitted
here for clarity; it does not affect keys or headers.

## 2.4 Blob format

Encrypted zones: blob plaintext is JCS of `{title, tags, md}` for `self`
(title/tags sealed) or `{md}` for `circle` (title/tags already clear in index).
Ciphertext = `XChaCha20-Poly1305(K_section, nonce, plaintext)`, AAD purpose `blob`,
bound to `subject_did ‖ /e/<zone>/s/<id> ‖ key_version`. Public: raw markdown file,
integrity by `sha256` in the index.

## 2.5 Derivation of a section key

```
DK_zone           = current node key of /e/<zone> (from its header, §03)
plain id:   K_sec = derive("aithos-core/v1/s/"+id,    DK_zone)
namespaced: K_ns  = derive("aithos-core/v1/ns/"+ns,   DK_zone)
            K_sec = derive("aithos-core/v1/s/"+local, K_ns)
```

`key_version` in the index tells a reader which DK generation the blob was written
under; the header carries every live version (§03.4) so readers resolve any of them.

## 2.6 Editions and the fork rule (serverless integrity)

The manifest carries `edition: {height, prev_hash, created_at}` and is signed by the
owner root (or a delegate + `authorized_by`, §05). `prev_hash` = SHA-256 of the prior
manifest's JCS with `signature=""`. Editions form a **linear chain**: height strictly
increases, each pins its predecessor.

Without a server, two devices could sign competing height-N editions (a fork).
Resolution rule, enforceable by any verifier:

- An edition is valid only if it extends the longest chain the verifier has seen and
  its `prev_hash` matches.
- A **fork** (two valid editions at the same height with different hashes) is resolved
  by the next **owner-root-signed** edition naming the winning `prev_hash` in a
  `resolves_fork` field. Delegates MUST NOT resolve forks.
- Verifiers presented with an unresolved fork MUST refuse to treat either branch as
  canonical for delegated writes and surface the conflict.

This keeps integrity owner-anchored without an online arbiter; a mirror that serializes
writes is a convenience (§00), not a requirement.

## 2.7 Gamma anchoring

The manifest pins `gamma_head` = SHA-256 of the last gamma entry (§07). An edition and
its gamma head move together; a verifier checks that every section's `gamma_ref`
resolves in the log and that the head matches. This binds "what the bundle says" to
"what the log recorded," including delegated authorship and action accounting.

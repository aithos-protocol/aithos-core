# 2 — Content tree

> **Status: DRAFT.** Zones, folders, sections, tags: how an unlimited-depth tree maps
> to nodes and keys, and how editions chain without a server.

## 2.1 Nodes and paths

A zone is simply the **root folder** of its tree. Every folder is a node like any
other, recursing without depth limit; every folder may freely contain subfolders and
sections, and anchors its own local tag views. Canonical paths (also the derivation
spine):

```
/e/public                       zone = root folder (plaintext — no key, no header)
/e/circle                       zone = root folder (DK, header)
/e/circle/d/<sid>               folder — recursive, unlimited depth (derived key)
/e/circle/d/<sid>/d/<sid>/…     …folders contain folders and sections freely
/e/…/t/<tag>                    tag view anchored at ANY folder, zone root included
                                (derived anchor; sections bridge in via wraps, §2.9)
/e/…/s/<sid>                    section (derived key, under its folder)
/e/self …                       same shape as circle; paths opaque (§2.8)
/x/<connector>                  vault node (DK, header) — §08
```

The marker segments `d`/`t`/`s` provide domain separation; `<sid>` is the node's
stable identifier (§2.2). Human-readable names never appear in canonical paths — a
display path like `circle/enfance/cicatrices/1234` is resolved through metadata.
`public` is plaintext by design: real named folders on disk, addressability, no
cryptography. `circle` and `self` are encrypted zones, each rooted in a node with its
own DK and header.

## 2.2 Folders, sections, sids

Every folder and every section has two identifiers:

- **sid** — a ULID, globally unique, assigned at creation, **never changed**. The sid
  is the derivation label (§2.5) and the blob filename. Because keys hang off sids,
  renaming anything never re-keys anything.
- **name** — the human segment (`enfance`, `cicatrices`, `1234`);
  `[a-z0-9_-]{1,64}`, unique among its siblings. Pure metadata: clear in the index
  for `public`/`circle`, sealed for `self` (§2.8).

A section is `{sid, name, title, tags[], body}`; a folder is `{sid, name, children}`.
The legacy namespaced id `gmail:0042` is sugar for folder `gmail/`, section `0042` —
namespaces are just depth-1 folders and no separate `ns` concept survives. Display
paths (`zone/enfance/cicatrices/1234`) resolve names→sids through the index
(`public`/`circle`) or through sealed folder descriptors (`self`).

Mutating a section's **tag set** is an edit of the section, never of the tag view: a
`tag=` grant covers the sections currently carrying the tag, NOT re-labelling. Adding
or removing a tag requires an `id=`, `dir=` or zone-level edit perimeter on the
section itself. And when a repair pass creates a missing tag wrap for a section newly
carrying a tag, it MUST first validate the author of that tag mutation (a covering
edit perimeter at the mutation entry's `at`, per gamma) and fail closed: an
unauthorized re-label is never bridged into a tag view.

## 2.3 Bundle layout

```
manifest.json              signed, linear-chained (§2.6)
did.json                   §01.4
e/public/<name-path>.md    plaintext sections, real named folder tree on disk
e/circle/index.json        clear tree: folders [{sid,name,parent_sid}] + sections
                           [{sid,name,folder_sid,title,tags,blob_sha,key_version,gamma_ref}]
e/circle/blobs/<sid>.enc   AEAD blob (purpose "blob")
e/circle/hdr/<node>.json   one per granted node (§03), sid-addressed path
e/self/index.json          FLAT opaque list: [{sid,key_version,gamma_ref}] — nothing else
e/self/blobs/<sid>.enc     sections AND sealed folder descriptors, indistinguishable
e/self/hdr/<node>.json     granted self nodes, sid-addressed
x/<id>/…                   vault, §08
certs/<mandate_id>.json    §04 (public)
gamma/gamma.jsonl          §07
```

Sharding of large indexes is permitted (deterministic, by `sha256(sid)`) but omitted
here for clarity; it does not affect keys or headers.

## 2.4 Blob format

Encrypted zones: blob plaintext is JCS of `{md}` for `circle` sections (name/title/
tags already clear in the index), `{kind:"section", name, title, tags, md}` for
`self` sections, or `{kind:"folder", name, children:[sids]}` for `self` folder
descriptors (§2.8). Ciphertext = `XChaCha20-Poly1305(K_node, nonce, plaintext)`, AAD
purpose `blob`, bound to `subject_did ‖ canonical sid-path ‖ key_version`. Public:
raw markdown file, integrity by `sha256` in the index.

## 2.5 Derivation along the path

```
K(zone root)     = current DK of /e/<zone> (from its header, §03)
K(child folder)  = derive("aithos-core/v1/d/"+sid, K(parent folder))
K(tag anchor)    = derive("aithos-core/v1/t/"+tag, K(folder))
K(section)       = derive("aithos-core/v1/s/"+sid, K(folder))
```

One BLAKE3 `derive_key` per path segment: reading at depth *d* costs *d* derivations
— nanoseconds; depth is architecturally unlimited. Holding any folder's key yields
its entire subtree, present and future; never anything above or beside it (one-way).
Any node below the zone root can ALSO carry its own header (random DK + up-link wrap,
§03.4): derivation is the default route, a header line the granted one; both resolve.

`key_version` in the index tells a reader which DK generation the blob was written
under; the header carries every live version (§03.4) so readers resolve any of them.

## 2.6 Editions and the fork rule (serverless integrity)

The manifest carries `edition: {height, prev_hash, created_at}`, the state roots
`roots: {public, circle, self, vault}` (§2.10), the log tip `gamma_head` (§2.7), and
is signed by the owner root (or a delegate + `authorized_by`, §05). `prev_hash` =
SHA-256 of the prior manifest's JCS with `signature=""`. Editions form a **linear chain**: height strictly
increases, each pins its predecessor.

Without a server, two authors could sign competing height-N editions. Resolution
rules, enforceable by any verifier:

- An edition is valid only if it extends the longest chain the verifier has seen and
  its `prev_hash` matches.
- **Disjoint merge (deterministic, arbiter-free).** Two competing editions at the
  same height whose changesets touch **disjoint node sets** are not a conflict: any
  party MAY publish the merge edition at height+1 — both changesets applied, parents
  ordered by ascending edition hash, listed in a `merges: [hash_a, hash_b]` field —
  and every verifier computes the same result. Multi-agent contention on different
  nodes therefore never waits for anyone.
- A **fork** proper (same-node conflict, or irreconcilable merges) is resolved by the
  **nearest common manager**: the closest authority whose perimeter covers every node
  touched by both branches. A delegate qualifies within its own authority; the owner
  root always qualifies and is the last resort. The resolving edition names the
  winning `prev_hash` in `resolves_fork`; verifiers accept it under the same
  authority check as a write to those nodes.
- Verifiers presented with an unresolved fork MUST refuse to treat either branch as
  canonical for delegated writes and surface the conflict.

This keeps integrity authority-anchored without an online arbiter — and without an
owner availability dependency (§00.5); a mirror that serializes writes is a
convenience (§00), not a requirement.

## 2.7 Gamma anchoring

The manifest pins `gamma_head` = SHA-256 of the last gamma entry (§07). An edition and
its gamma head move together; a verifier checks that every section's `gamma_ref`
resolves in the log and that the head matches. This binds "what the bundle says" to
"what the log recorded," including delegated authorship and action accounting.

## 2.8 `self` structure secrecy

In `self`, the tree itself is confidential. On disk and in the index, `self` is a
flat sea of opaque sids — sections and folder descriptors indistinguishable; names,
titles, tags, parent/child links all live **inside** ciphertext. Each `self` folder
has a small sealed **descriptor** blob under its own key listing `{name,
children:[sids]}`; an authorized reader reconstructs exactly the sub-tree it can
open, top-down from the deepest node it holds, and nothing else. Headers and gamma
targets use sid-paths, so granting or editing a `self` node leaks no structure
either. Consequence (same honest limit as sealed tags, §10.7): on `self`, `dir=` and
`tag=` perimeters are enforceable for **reading** (keys are physics) but not
verifier-checkable for **writing** — write perimeters on `self` use `id=` or
zone-level grants.

## 2.9 Tag views, rename, move

**Tag views at any folder.** A tag view `…/t/<tag>` is an anchor node derived from
its folder (§2.5). It grants nothing by derivation downward — sections enter it by
**wrap**: the folder's manager seals `wrap(K_section)` under the anchor key when a
section under that folder carries the tag (fail-closed authorship check, §2.2). A
zone-root view spans the whole zone; a folder-local view spans that subtree only. One
header line on the anchor is thus the O(1) grant "read what is tagged `toto` under
this folder, now and in the future".

**Rename is free.** Names are metadata (§2.2): renaming a folder or section edits an
index row / descriptor, re-keys nothing, moves no bytes.

**Move is a rotation.** Derivation cannot be un-taught: whoever held the old parent
can derive the moved node's old key forever. Moving node M under a new parent is
therefore the rotation of M (fresh DK', §03.4) plus its up-link wrap posted under the
**new** parent, survivors re-sealed as usual: old-parent holders are cut
cryptographically, new-parent holders derive through the wrap. Cost ∝ M's granted
headers (+ re-encryption of M's subtree if incident-grade); the lazy variant is
tolerated as hygiene (§06.8).

## 2.10 Merkle state roots (verifiable partial reads)

Each edition's manifest pins one **state root** per zone, plus the vault, next to
`gamma_head` (§2.6). A reader verifies any single row, header, or subtree against
the signed manifest in O(log n) — without ever fetching an index — and any mirror
can serve such proofs without being trusted.

Hashing (BLAKE3, domain-separated so a leaf can never be spliced as an interior
node):

```
H_leaf(p)      = BLAKE3("aithos-core/v1/mk-leaf" ‖ 0x00 ‖ p)
H_node(l, r)   = BLAKE3("aithos-core/v1/mk-node" ‖ 0x00 ‖ l ‖ r)
mroot(list)    = balanced binary H_node tree over the sorted list; 32×0x00 if empty
header_hash(N) = BLAKE3(JCS(header.json)) if N was ever granted, else 32×0x00
```

Node hashes — `public`/`circle` mirror the folder tree (§2.1), children sorted by
`(kind, sid|tag)`:

```
section:   H_leaf( JCS(index_row)  ‖ header_hash )
tag view:  H_leaf( "t/"+tag        ‖ header_hash ‖ mroot(wraps, by section sid) )
folder:    H_leaf( JCS(folder_row) ‖ header_hash ‖ mroot(children node hashes) )
zone root: the node hash of the root folder
```

The header (and, for tag views, the wrap set) is **folded into its node's hash**:
one proof attests the index row, the current header version, and the wraps at once;
a grant or rotation naturally bumps the node's path to the root. `self` and the
vault are **flat** (§2.8): leaves `H_leaf(JCS(index_row) ‖ header_hash)` sorted by
sid — proofs reveal sibling hashes only, never structure.

Proofs and costs. An inclusion proof interleaves sibling steps (inside a folder's
balanced tree) with parent steps (the parent folder's own payload), ending at the
zone root: size and verify time O(depth × log₂ fanout) — roughly 25 hashes, under a
kilobyte, for a million sections at fanout 100. A `dir=` grantee checks its whole
perimeter against its folder's children root plus one path to the signed root: it
verifies everything it can see and nothing it cannot. Each edit recomputes only its
own path (microseconds); a disjoint merge (§2.6) recomputes the two touched paths
and every verifier reproduces identical roots. Two editions diff by root descent:
sync in O(changed × log n).

Limit (honest): a Merkle proof shows **inclusion in a signed edition**, never
freshness. Staleness stays bounded by `freshness` (§04.4) and the edition + gamma
chains (§2.6, §07); a mirror can withhold, not forge.

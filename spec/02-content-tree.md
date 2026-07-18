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

> **CB1 conformance-hardening decision — validated at the human protocol gate on
> 2026-07-18; no new grammar.**
> Untrusted display paths are relative to their already-selected logical zone and
> enforce the human-name grammar of §2.2. They reject a leading absolute prefix,
> empty or dot segments, traversal, nonconforming names, and any resolution that
> would escape that zone before store access. Store keys are likewise relative and
> confined, but obey the exact canonical layout of §2.3 (whose fixed filenames and
> extensions are not human names), not the §2.2 name grammar. The logical canonical
> paths of §2.1 keep their leading `/e/...` or `/x/...`; they are not display-path
> inputs. `FsStore` anchors its opened canonical root and refuses any symlink,
> junction, reparse point, or equivalent indirection whose resolution would leave
> that root, before read, write, list, edition load, staging publication, or
> recovery. A signed manifest cannot legitimize an escape or out-of-layout object.
> The invariant is observable confinement and prescribes no particular syscall.
> This is enforcement of the existing grammar and layout, not a new path or wire
> form.

## 2.4 Blob format

Encrypted zones: blob plaintext is JCS of `{md, sig?}` for `circle` sections
(name/title/tags already clear in the index; `sig` present for owner-authored
sections, §2.11), `{kind:"section", name, title, tags, md}` for `self` sections
(never signed, §2.11), or `{kind:"folder", name, children:[sids]}` for `self`
folder descriptors (§2.8). Ciphertext = `XChaCha20-Poly1305(K_node, nonce, plaintext)`, AAD
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
  owner, or one leaf grantee whose single chain covers every typed operation and
  change on all nodes touched by both parents, MAY publish the merge edition at
  height+1 — both changesets applied, parents ordered by ascending edition hash,
  listed in a
  `merges: [hash_a, hash_b]` field — and every verifier computes the same result.
  Multi-agent contention on different nodes therefore needs no online arbiter, but
  it never bypasses authority.
- A **fork** proper (same-node conflict, or irreconcilable merges) is resolved by the
  **nearest common manager**: the closest authority whose perimeter covers every node
  touched by both branches. A delegate qualifies within its own authority; the owner
  root always qualifies and is the last resort. The resolving edition names the
  winning `prev_hash` in `resolves_fork`; verifiers accept it under the same
  authority check as a write to those nodes.
- Verifiers presented with an unresolved fork MUST refuse to treat either branch as
  canonical for delegated writes and surface the conflict.

Wire conventions (graved 2026-07-11, pass I — they condition the signed bytes):

- **Merge edition.** `prev_hash` pins the parent with the LOWEST edition hash;
  `merges: [hash_a, hash_b]` (ascending) rides beside it — additive like the
  §2.10 roots, absent from pre-I editions whose chain hashes are untouched. A
  linear-chain walker sees a valid link; a merge-aware verifier ALSO demands:
  both parents at the same height sharing the same grandparent (`prev_hash`),
  disjoint changesets (below), and a merged state it reproduces byte-for-byte
  (content roots §2.10 and gamma roots §7.10 recommitted over the merge).
- **Changesets and disjointness.** A parent's changeset is its §2.10
  root-descent diff against the common ancestor; two changesets are disjoint
  iff their touched node-label sets do not intersect. A shared **index file**
  does not break disjointness: index rows merge **3-way by sid** — base = the
  common ancestor's index; a row changed on one branch is taken from that
  branch; rows added on either side are unioned; deletions hold (a row absent
  from the changing branch stays absent — no resurrection); the existing sort
  orders the result and JCS makes it byte-identical for every merger. The SAME
  sid changed on both branches IS a same-node conflict — a fork, never merged.
- **Fork resolution.** The resolving edition carries `resolves_fork:
  <winning prev_hash>` (additive), is signed by the nearest common manager —
  an authority whose perimeter covers every node touched by BOTH branches
  (§04.2 nodal coverage; a delegate qualifies only inside its own authority,
  the owner root always qualifies) — and its content extends the winning
  branch. Verifiers accept it under the same authority check as a write to
  those nodes; the losing branch's delegated writes are surfaced, never
  silently replayed.

### 2.6.1 Normal delegated editions (D3)

Edition v1 has exactly one actor. The actor is either the owner, using its local
capability without a mandate, or the leaf grantee, proving possession of its key and
presenting exactly one valid chain. Every content, structure, header, Gamma, root,
and manifest change in a grantee edition MUST be explained by that same actor and
chain. If two chains are needed, they produce separate editions; v1 has no aggregate
multi-chain edition.

For a merge or resolution, the single actor/chain is the publisher and authority of
the new edition: it covers the complete derived changeset. Already-verified entries
and authorship on each parent retain their historical actors; the publisher neither
rewrites nor impersonates them. Only the new Gamma delta and the new edition
signature/publication authority are single-actor.

The verifier derives the typed changeset from the pinned parent state and the
candidate state; it never trusts a caller-asserted list. It checks the expected
parent and height, every changed object and removal, the corresponding canonical
operation and Gamma entry, the chain and certificate hashes needed for cold
verification, the recomputed roots and Gamma head, and the actor signature. An
unexplained change, an omitted change, an extra Gamma consumption, or a different
actor inside the candidate's new delta invalidates the edition even if all hashes
and links are structurally valid.
K1-B fixes the three additive draft2 manifest member names and carrier split in
§2.6.2. Their remaining nested byte tables stay independently gated.

A grantee signs as itself, never as the owner. The owner is absent from a normal
delegated publication unless an explicitly applicable obligation requires an owner
`co_sign` receipt; that receipt attests the operation and does not make the owner the
edition actor.

This keeps integrity authority-anchored without an online arbiter — and without an
owner availability dependency (§00.5); a mirror that serializes writes is a
convenience (§00), not a requirement.

### 2.6.2 Draft2 operation, changeset, and evidence carriers (K1-B)

> **K1-B carrier architecture — human-validated on 2026-07-18.**

Every newly issued manifest with `aithos-core: "1.0.0-draft.2"` has these three
additional signed top-level members:

- `operation_ref` is the exact W1 reference of this normal, merge, or resolution
  publication occurrence;
- `changeset_ref` content-addresses the one derived closed changeset for the
  candidate relative to its applicable parent state or states;
- `evidence_ref` content-addresses the closed public evidence set needed to replay
  the contained occurrences and publisher authority without private capabilities.

The manifest signature covers all three references. A draft2 manifest missing one,
carrying `null`, or carrying an unknown or malformed reference fails closed. A
draft1 manifest forbids all three and remains byte-identical under historical
verification.

The changeset is derived, never caller-asserted. It binds the applicable parent
references, contained operation references in deterministic causal order, their
logical before/after transitions, and every deterministic store consequence needed
to explain the candidate bytes. It includes the contained operation references but
excludes the publication's own `operation_ref`, the candidate manifest hash, and
anything transitively derived from them. The publication facts may therefore commit
the completed changeset without a cycle. A missing transition, unexplained byte,
extra operation, or mismatched consequence invalidates the publication.

The evidence set carries only public proof material: delegated authorship, SC1
certificate and session proof when applicable, R2/U1 receipts, approved-catalog
evidence, and any explicitly signed read presentation. Evidence never grants
authority by itself; every item is cross-checked against its exact `operation_ref`,
the reconstructed facts, authority chain, candidate manifest, and derived
changeset. Private content, credentials, DKs, private keys, and protected plaintext
are forbidden.

K1-B fixes these three manifest member names, their required/forbidden profile
presence, the changeset/evidence separation, and the acyclic dependency direction.
The exact `changeset_ref` and `evidence_ref` member sets, sidecar profiles, digest
preimages, array ordering within equal causal positions, public authorship and
presentation objects, and canonical sidecar paths remain reserved. No producer may
emit a draft2 manifest until those tables and vectors are independently approved.

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

Keyless verification of a `self` mutation uses opaque state evidence only. Create
proves prior absence and subsequent inclusion of an authorized preallocated SID (or
uses zone-wide append/write authority); edit proves replacement of the same SID;
delete proves its removal. The evidence binds the prior and next commitments,
operation, Gamma entry, chain, roots, and edition without exposing a name, path,
title, tag, body, folder relation, or key. A signed assertion without a proof tied to
the prior state is insufficient.

K1.1-B represents a present logical state by the exact protected object-set
commitment of §4.5.1 and absence by the closed `{"state":"absent"}` variant. The
state-fact document contains only domain-separated commitments to canonical store
keys and exact stored bytes; it carries no clear target or store key and creates no
new public sidecar. The later family-specific proof table MUST bind the opaque
target to exactly that committed object set before any operation commitment can be
produced.

For a `self` folder delete or move, that opaque proof covers the exact set of
affected commitments and the authority for each of them. Delete proves coverage and
removal of every descendant commitment; move proves source edit authority,
destination append/write authority, and every required rotation, rewrap, and
re-encryption consequence. The proof exposes neither the relationships among those
commitments nor their names or contents. Its additive signed encoding is reserved
for independent CB2 vectors.

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

Certificates follow the node, not its address (§04.2 nodal `dir` containment): a
mandate granted **on M itself** — a direct header line, re-sealed as a survivor —
keeps both its key and its coverage at M's new address; a mandate on the **old
parent** loses M's subtree at verification time too, so policy and physics agree.
Move re-parents M's index row; sids are stable, so every derivation label below M
is unchanged and only M's own key is fresh. M's rotated header (and each
re-encrypted body) binds M's **new** canonical path; the old header file stays in
place, an immutable record of the versions sealed at the old address. Fail-closed
bounds: a move never crosses zones, never targets M itself or a descendant (no
cycles), and never lands beside a same-named sibling. Move cuts by consequence,
not by intent — cutting someone is what revocation is for (§06); moving M into a
granted subtree shares it with that subtree's holders, moving it out un-shares it,
exactly like a physical object changing drawers.

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

Wire conventions (graved 2026-07-11, pass H1 — they condition the hashed bytes):

- `mroot` recursion: `mroot([]) = 32×0x00`; `mroot([x]) = x`; else
  `H_node(mroot(left), mroot(right))` with `left` = the first ⌈n/2⌉ items
  (left-heavy split). No duplication, no promotion.
- Children sort inside a folder: by `(kind, key)`, kind order `"d" < "s" < "t"`
  (folder, section, tag view), key = sid for d/s, the tag string for t. Flat
  zones sort by sid.
- Tag-view wraps: `mroot` over `H_leaf(section_sid ‖ 0x00 ‖ BLAKE3(JCS(wrap)))`,
  sorted by section sid.
- Zone root node: the zone root has no index row — its literal label takes the
  row's place: `H_leaf("z/"+zone ‖ header_hash(zone root) ‖ mroot(children))`.
  Flat zones (`self`, the vault) have no folder payload at all:
  `root = mroot(leaves)` directly, leaves sorted by sid (vault: by node label).
- Roots ride the manifest **beside** the flat file pins (additive, decided
  2026-07-11): `roots: {public, circle, self, vault}` next to `gamma_head`;
  the flat pins keep covering byte-rollback of sealed `self` blobs (§2.8 rows
  carry no blob hash by design).
- Proof wire (v1): the verifier starts from the CLAIMED bytes —
  `cur = H_leaf(JCS(row) ‖ header_hash [‖ mroot…])` — then applies ordered
  steps: `{"node":{"side":"left"|"right","hash":"<hex>"}}` →
  `cur = H_node(sibling, cur)` / `H_node(cur, sibling)`; and
  `{"wrap":{"pre":"<hex>","post":"<hex>"}}` → `cur = H_leaf(pre ‖ cur ‖ post)`
  (the parent folder's own payload folding). The proof verifies iff the final
  `cur` equals the pinned zone root. Domain separation does the splicing
  defense: a node hash fed where a leaf is expected (or the reverse) changes
  the domain string and the root dies.

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

## 2.11 Signature policy per zone (owner content)

The owner signs with a single pen, `content_sign` (§01.1); the **audience lives in
the signed payload, never in the key**. Owner content signatures always cover JCS of
`{zone, path, sid, body_hash}` — stripping or altering the placement breaks the
signature, so a detached artifact carries its own truth. A verifier rejects any
owner signature whose embedded placement does not match where the object actually
sits (fail-closed). What differs per zone is *where* the signature lives:

- **`public` — signed, in the open.** The signature ships in the index row and MAY
  travel as a sidecar with the raw markdown: public content is made to circulate
  detached, carrying proof of authorship *and of publication intent*.
- **`circle` — signed, inside the seal.** The signature is part of the sealed blob
  plaintext: only readers of the section can verify it. Authenticity for the
  audience — and a member who leaks the plaintext leaks the proof with it, a stated
  limit (§10.7): audience authenticity and leak-deniability are mutually exclusive.
- **`self` — never signed.** Integrity comes from AEAD + gamma anchoring (§2.7);
  authorship attribution lives in the owner-signed gamma entry over the opaque sid.
  A leaked plaintext alone proves nothing — **deniable by default**.

**Selective disclosure (the official inverse).** The owner can convert deniability
into proof, per section, at will: revealing one section key lets anyone verify
ciphertext → signed gamma entry → edition chain, proving authorship and date for
that section only. Scoped, irreversible for that section, and always the owner's
move — never the thief's.

Agent-authored content is never signed with owner keys, in any zone: the agent
signs its gamma entry with its own keypair under its chain (§07.2). "I said it"
versus "my agent said it in my name" is a cryptographic boundary that no mandate
scoping error can blur.

For `public`, that boundary also travels with the content. A grantee-authored
mutation carries the grantee's signature bound to the content hash, SID, canonical
operation, edition, and leaf `authorized_via`; Gamma and the manifest commit that
proof. Cold verification therefore distinguishes owner authorship from delegated
authorship without a private key. Product presentation MAY show the grantee and its
authorization chain, but MUST NOT label that content as directly owner-signed.

## 2.12 Local transaction and keyless verification boundary

> **CB1 decisions G-B and G-D — validated at the human protocol gate on
> 2026-07-18.**

**Local transaction (G-B).** A mutation is calculated against an immutable snapshot
in an overlay, submitted to the pure Core verdict, reduced to a deterministic
write-set, and only then committed. Business helpers never write canonical objects
directly. Every transaction has one logical linearization point after Core
validation. Rejection or failure before that point leaves the canonical bundle
byte-for-byte unchanged: no advanced manifest or Gamma head, partial index, header,
wrap, blob, or orphan from the failed local mutation.

**External effects (K1-B).** A connector action or inference stages its Gamma and
evidence in that same non-canonical overlay, obtains pre-effect authorization,
performs the external effect, adds post-effect usage evidence when applicable, and
only then reaches the local linearization point (§08.1). The pre-effect check is
permission to execute, not accepted-history admission. Final append-time acceptance
and cold replay receive the same completed public facts and use the same Core
semantics.

If execution is refused or the external effect reports failure, the overlay is
discarded and the canonical bundle stays byte-identical. If the process loses state
after the external effect but before local commit, local atomicity cannot undo the
remote fact: retry requires connector-side reconciliation of the original
occurrence. No canonical `pending` object or inferred second occurrence is created.

`MemStore` commits by atomically replacing its canonical state. `FsStore` prepares in
recoverable staging physically outside the canonical bundle directory and uses a
Store-local recoverable linearization mechanism. Any internal generation metadata,
commit marker, or reference is outside the canonical bundle namespace, §2.3 layout,
manifest, pins, and signed wire; it only selects which complete staged state the
Store exposes as the canonical view. The contract does not require a non-portable
multi-file syscall. Readers, reopen, and recovery observe either the complete old
state or the complete new state, never a mixture. A crash or lost acknowledgement
at the linearization boundary may require discovering the committed outcome from
the canonical manifest/head; scratch is cleaned or recoverably resolved. The sole
orphan exception is D3's explicit preloading of unreferenced opaque
content-addressed publication blobs, outside the local transaction. Such blobs are
never reachable canonical state.

**Keyless façade (G-D).** Bundle is the only public assembly boundary: it decodes and
validates layout, version, hashes, references, reachability, and proof shape, then
passes typed public artifacts to Core's pure semantic verifier. Append-time and
cold-time feed the same facts to that verifier and obtain the same verdict. Exporting
an edition into a fresh `MemStore` or `FsStore` and reopening it without owner or
grantee private capabilities MUST be sufficient to verify owner and delegated
history.

A future provider may call this one Bundle façade and then perform only opaque
storage, transport, and its own CAS. It receives no content key or protected
plaintext and MUST NOT copy or reimplement perimeter, mandate, constraint,
revocation, Gamma, changeset, or authorship semantics.

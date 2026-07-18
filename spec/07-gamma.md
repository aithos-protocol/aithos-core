# 7 — Gamma log

> **Status: DRAFT.** The subject's tamper-evident record of every mutation and every
> agentic action. It is both the history spine (as in Aithos v0.x) and the enforcement
> substrate for counting constraints (I5).

## 7.1 Chain

`gamma/<YYYY-MM>.jsonl` — one JSON entry per line, SHA-256 hash-chained, segmented
by UTC month of `at` (a month with no entries has no file; `prev` crosses segment
boundaries transparently). Segmentation buys date-range access in O(segments
touched) and leaves room for per-segment keys later; the chain, not the file
layout, is the truth. The manifest pins every segment's hash plus `gamma_head`
(§02.7).

An entry has two layers: a **clear counting header** — the fields any offline
verifier needs to check the chain and tally budgets (§7.4) — and, for content
mutations, a **sealed body** carrying everything else (§7.3):

```jsonc
{ "v": 1,
  "id": "gamma_01JZ…",                       // ULID
  "prev": "sha256:…",                        // hash of the previous entry's JCS
  "at": "2026-07-08T10:12:00Z",
  "kind": "section.add" | "section.modify" | "section.delete" | "section.redact"
        | "action" | "inference" | "ethos.read"
        | "heartbeat" | "grant" | "revoke" | "rotate" | "merge",   // registry §7.9
  "target": "x.gmail" | "mandate_01JZ…",     // clear for action/structural kinds only;
                                             // for section.* kinds it lives in body_enc
  "authorized_by": "mandate_01JZ…",          // omitted for owner-signed entries
  "authorized_via": ["mandate_root…","mandate_leaf…"],   // chain, for delegated/agentic
  "payload": { … },                          // clear: action & structural kinds (§7.3)
  "body_enc": { "hint": "…", "n": "…", "c": "…" },       // sealed: section.* kinds (§7.3)
  "signature": { "alg": "ed25519", "key": "<owner key URL | grantee pubkey>", "value": "…" } }
```

The signature covers the whole entry (JCS, ciphertext included), so a sealed body
is pinned by the same signature that pins the header. Any past-entry alteration
breaks `prev` and every downstream signature — a write-once log. Redaction is the
public, logged `section.redact` (never silent deletion).

**Committed roots (graved as §7.10, pass H2).** Gamma state roots ride the
manifest — per-segment roots and a counts trie — turning budget checks into
O(log n) Merkle proofs and mirror answers into provably complete ones. A future
profile may still seal today's clear counting fields (`kind`, `authorized_via`,
action names) and verify counts against the committed roots alone; the `v` field
exists so that transition is a version bump, not a fork (deferred, 2026-07-11).

### 7.1.1 Gamma v2 as operation evidence

> **W1.1 protocol decision — H1 + E1 + M1, human-validated on
> 2026-07-18.**

The Gamma v1 envelope above is historical and remains byte-identical. Gamma v2
keeps the same chain, signature, confidentiality, kind-registry and per-kind
payload/body rules. For an operation-bearing entry it adds this clear, signed
top-level member:

```jsonc
"operation_ref": {
  "aithos-operation-core": "1.0.0-draft.1",
  "occurrence": "op_<ULID>",
  "commitment": "sha256:<64 lowercase hex>"
}
```

The object is exactly the closed reference of §4.5.1. It is covered by the entry
signature and is never moved into `payload` or `body_enc`.

Presence is fail-closed:

| Entry profile and kind | `operation_ref` |
|---|---|
| Gamma v1, every kind | forbidden |
| Gamma v2 `section.add/modify/delete/redact` | required |
| Gamma v2 `ethos.read` | required |
| Gamma v2 `action` / `inference` | required |
| Gamma v2 `grant` / `revoke` / `rotate` / `merge` | required |
| Gamma v2 `heartbeat` | forbidden |

Missing a required reference, adding a forbidden one, an extra or missing member
inside it, a malformed occurrence or digest, or an unknown operation profile
invalidates the entry. The closed registry remains §7.9.2's: W1.1 introduces no
`publication`, `resolution`, or `gamma.read` kind.

A Gamma append is native evidence for its underlying operation, not another
operation. It carries that operation's already allocated `operation_ref`; neither
the append nor `gamma_<ULID>` allocates or substitutes another occurrence. One
occurrence may be correlated with other applicable native evidence, but at most one
operation-bearing Gamma entry may consume it. A second Gamma entry reusing the same
occurrence and commitment is replay. Reusing the occurrence with a different
commitment is equivocation. Both are rejected before the candidate joins accepted
history or affects an accepted tally, including when first discovered while joining
branches.

The manifest profile governs only the delta newly introduced relative to every
parent. A manifest with `aithos-core: "1.0.0-draft.1"` introduces Gamma v1 entries
only. A manifest with `aithos-core: "1.0.0-draft.2"` introduces Gamma v2 entries
only and MAY retain any reachable, byte-identical Gamma v1 ancestry.

Version order is checked on every causal edge. A draft.1 manifest may lead to
draft.1 or draft.2; a draft.2 manifest may lead only to draft.2. A Gamma v1
predecessor may lead to v1 or v2; a Gamma v2 predecessor may lead only to v2.
For a merge the rule applies independently to both manifest parents and both Gamma
predecessors. Missing or unknown profiles and versions fail closed. Therefore each
causal path transitions at most once, no v2 material is accepted under draft.1, and
no historical entry is rewritten or assigned a synthetic reference.

## 7.2 Who may sign an entry

- **Owner** entries: signed by `content_sign` (§01.1); no `authorized_by`. The
  signed entry embeds its `target`, so placement is bound by the signature itself
  (§02.11).
- **Delegated** entries (mutations or actions by an agent): signed by the leaf grantee
  key (and the session key if `session_bind`), carrying `authorized_by` = leaf id and
  `authorized_via` = full chain. Verified by §04.5 + §05.3 at the entry's `at`.

## 7.3 Two-layer confidentiality

**Sealed bodies (content mutations).** For every `section.*` entry on a keyed
zone (`circle`, `self` — `public` has no zone key and its mutations stay clear,
target and payload at the top level like structural kinds), the body
`{target, payload}` is AEAD under the **target node's content key** (derivation
purpose `gamma-body`): the log reveals *that* someone acted at some time under
some mandate, but *what was touched and what changed* is readable only by those
who can read the node itself. The target sid-path moving inside the ciphertext is
what keeps activity from leaking structure — on every zone, not just `self`.

Because the target is sealed, the body carries a clear `hint`: a recognition tag
deterministically derivable **only by holders of the target node's key**
(derivation purpose `gamma-hint`). A reader precomputes hints for the nodes it
holds and matches entries in O(1); a stranger learns nothing but equality of
hidden targets. (Same design stance as `self` structure secrecy, §02.8.)

**Clear payloads (action & structural kinds).** `action`, `inference`, `grant`,
`revoke`, `rotate`, `heartbeat` and `merge` entries carry clear payloads of ids,
versions, sequence numbers, counters and hashes — no secrets by construction
(§7.4's action payload hashes its args). They stay clear because third-party
verifiers must count them (budgets, children, liveness) before committed count
roots exist (forward note, §7.1). An `action` entry MAY additionally carry a
sealed `body_enc` holding its full arguments for a-posteriori audit (§7.9.3):
the clear `args_hash` pins the sealed args, the counting skeleton never grows.

**Who reads what (defaults).** The owner derives every node key from S and reads
everything. A grantee's subtree read grant (§04.3 header lines) opens exactly the
bodies sealed under its perimeter's nodes — gamma reading rides the content key
physics, no new key material. Everyone else — including agents that only *push*
entries — sees the counting skeleton and nothing more. Appending never requires
reading: an author needs only `gamma_head` (manifest, §02.7) to chain correctly.
The `read.gamma` perimeter entry (§04.2) is the certificate half that makes log
reading a *granted* right; for key-holders it is policy (an honest verifier
refuses out-of-perimeter queries), physics for everyone else — the same honest
split as sealed-zone writes (§04.2).

**Crypto-erasure.** Destroying a node's key (rotation without re-wrap, §06)
retroactively blinds every sealed body under it while the chain stays intact —
erasure of content without falsification of history.

## 7.4 Action accounting (I5, the agentic meter)

Every connector action taken under a mandate MUST append an `action` entry:

```jsonc
{ "kind": "action", "at": "…", "target": "x.gmail",
  "authorized_by": "mandate_01JZ…", "authorized_via": [ … ],
  "payload": { "action": "reply", "args_hash": "sha256:…", "purpose_ref": "…",
               "checks": [ … ]? },   // obligation receipts §04.12 (owner co_sign, guardrail, approval)
  "signature": { "key": "<grantee pubkey>", … } }
```

Consequences, all verifier-checkable offline:

- `max_actions: N` ⇒ count entries whose `authorized_via` **contains** this mandate id
  (subtree count: a descendant's action consumes every ancestor's budget); the N+1-th
  is invalid. A delegate can never multiply its parent's budget by issuing children.
- `max_children: N` ⇒ count `grant` entries whose `authorized_by` is this mandate.
  Minting a sub-mandate MUST append its `grant` entry — otherwise `issue` would be a
  silent action, contradicting I5.
- `max_actions_per: {window, N}` ⇒ same subtree count, over a **rolling window** on
  `at` (never a calendar reset: "≤ N in *any* window" is stricter and needs no phase
  anchor).
- `rate_limit: {action, window, N}` ⇒ the windowed count filtered on the entry's
  clear `payload.action` — a per-action-kind budget. Composing mandates gives the
  same effect structurally: a mandate whose perimeter covers a single
  `act.x.<c>.<action>` makes *any* of its counters de-facto per-action.
- `budgets` (§04.11) ⇒ every `action`/`inference` entry cites `budget_ref`;
  per profile, count citing actions against `max_actions` and tally declared
  `tokens` (actions) plus `tokens_in + tokens_out` (inferences) against
  `token_budget` — subtree counts, per budgets-bearing mandate in the chain.
  Attested `tokens` (§04.11.1) override declarations in the tally.
- `binding`/`counter_sign` and any `obligations` (§04.12) ⇒ entry MUST carry a
  valid receipt in `checks[]` (owner co_sign, guardrail pass, approval) or it is
  invalid (and any effect it claims is unattributable).
- `purpose` ⇒ entry cites `purpose_ref`; audit trails intent.

The log is therefore the **counter** the serverless design otherwise lacks: state that
would need a server becomes append-only evidence anyone can tally. An action asserted
without its entry is, by I5, unauthorized.

## 7.5 Heartbeat entries

`kind:"heartbeat"` entries are owner-content-signed liveness beacons (§04.8), clear
payload `{seq}`. Heartbeat-bound mandates are valid only while the latest beacon is
within `every+grace` of `T`. Cheap (one tiny entry per period), and they double as a
freshness anchor for offline verifiers.

Under manifest draft.2, every newly emitted beacon is Gamma v2 but carries no
`operation_ref`. A heartbeat is an owner-signed liveness fact, not a canonical
operation occurrence or an additional mandate consumption. It may nevertheless be
the historical Gamma head from which the next operation is projected. Historical
Gamma v1 beacons remain unchanged.

## 7.6 Ordering without a server

Entries are appended in edition order; the manifest's `gamma_head` fixes the log's tip
per edition, and the edition chain (§02.6) is what serializes concurrent authors.

Concurrent appends (two authors extending the same tip) produce two sub-chains; the
disjoint-merge edition (§02.6) reconciles them with a `kind:"merge"` entry whose
`prevs: [head_a, head_b]` references both tips — the only entry kind with two
predecessors — signed by the merging party. Existing entries are never rewritten
(their signatures pin them): the log is a chain that may briefly fork and re-join at
explicit, signed merge points. Verifiers treat every entry reachable from the pinned
`gamma_head` as canonical; counts (§7.4) tally over that reachable set.

A fork in editions is a fork in the log; fork resolution (§02.6 — nearest common
manager, owner as last resort) selects the canonical log tip. A delegate can resolve
only forks entirely inside its own authority, so a compromised delegate cannot
rewrite log order beyond its perimeter — and never past entries, which stay pinned by
hash.

Wire (graved 2026-07-11, pass I): a merge entry keeps `prev` = the tip of the
parent with the LOWEST edition hash — the same parent the manifest's
`prev_hash` pins (§02.6) — and carries the additive `prevs: [head_a, head_b]`,
the only entry kind allowed the field (fail-closed form check). The merged
segment file lists parent A's sub-chain first (A = lowest edition hash), then
parent B's, then the merge entry — existing entries byte-identical, never
rewritten. `at` monotonicity is relaxed at the join (the signed merge entry
documents it); chain truth stays the `prev`/`prevs` links. The merge edition
recommits the §7.10 segment roots and counts trie over the merged layout, and
every verifier reproduces them from the files alone.

**Mixed-profile migration merge (M1).** Competing draft.1/v1 and draft.2/v2
branches MAY be joined by a draft.2 edition whose new `kind:"merge"` entry is
Gamma v2 and carries the merge publication's `operation_ref`. Both parent histories
remain byte-identical. Monotonicity is causal, over each parent edge and each
`prevs` edge; the deterministic merged segment layout MAY place a retained v1 line
after a v2 line physically without creating a v2→v1 causal downgrade. A merge that
would produce a draft.1 child of a draft.2 parent, or a v1 child of a v2
predecessor, is invalid.

## 7.7 Freshness anchor (anti-backdating)

Chained entries cannot be backdated once published — `prev` pins their order. The
residual trick is authoring an artifact *off-log* with an old `at` and presenting it
later. To bound it, every opposable artifact presented outside the log — an action
request, a co-signature, a chain presentation — MUST embed a recent `gamma_head` as
its anchor; a verifier rejects an artifact whose anchor is older than its freshness
tolerance (`freshness`, §04.4). Backdating is thereby bounded to the freshness
window.

**Honest limit — double counting inside the window.** Two verifiers can each honor
the N-th action of a `max_actions` budget within the freshness window, before either
entry propagates. Same bound as revocation propagation (§10.7): a stated limit of the
serverless design, not a bug.

## 7.8 Querying

**The chain is the truth; indexes are caches.** Any query index — by date, kind,
target, tag, mandate — is unsigned, rebuildable from the chain, local to whoever
built it, and never a trust party. Queries hit indexes; audits and verification
hit the chain. A wrong index can waste time, never forge history.

- **Date ranges** resolve to segments by filename (§7.1) before any entry is read.
- **Kind / mandate / signer** filters read only clear headers — streamable without
  keys.
- **Target / tag filters** are key-gated by construction: the querier matches
  sealed bodies via its `gamma-hint` tags (§7.3), then joins tags through the tree
  index it can read (clear on `public`/`circle`, sealed on `self`). Tag semantics
  are *current-tag* (join at query time); an audit that needs *tag-at-action-time*
  puts the snapshot inside the sealed body when logging.
- The **owner** (all keys from S) materialises full local views; a grantee's view
  is exactly its perimeter; a stranger's view is the counting skeleton.
- **Third-party query service** — a mirror answering filtered queries with
  inclusion *and completeness* proofs — rides the committed gamma roots (§7.10):
  the proven per-mandate counts say how many entries exist, the segment roots
  pin which. Withholding now breaks the counts, forging still breaks the roots.

The certificate half of log access is the `read.gamma` perimeter entry (§04.2);
its physics half is the node-key material the grantee already holds (§7.3).

A local `read.gamma` query checks its perimeter at operation time but emits no
signed protocol artifact, appends no Gamma entry, persists no `operation_ref`, and
cannot later be cold-replayed or counted. `log_reads` and `kind:"ethos.read"` remain
the journal contract for Ethos content reads; they do not recursively journal access
to the Gamma log.

If a query result is made opposable as an explicitly signed presentation, that
presentation is one canonical read/presentation occurrence and its signed evidence
carries the occurrence's `operation_ref`. The exact presentation carrier is outside
this Gamma profile. Producing it does not create a `gamma.read` entry or turn the
Gamma append into another occurrence.

## 7.9 Inference metering, the kind registry, sealed args

> Decided 2026-07-10 (F+).

### 7.9.1 `inference` entries

Every LLM call made under a mandate MUST append one light `inference` entry,
written by the container (§08), clear payload:

```jsonc
{ "kind": "inference", "target": "x.llm",
  "authorized_by": "mandate_…", "authorized_via": [ … ],
  "payload": { "provider": "anthropic", "model": "claude-haiku",
               "tokens_in": 1200, "tokens_out": 300,
               "budget_ref": "haiku", "receipt": { … }? } }
```

**Prompt and response text NEVER enter gamma.** They live in the agent cache
(`/k/`, outside the canonical bundle); the log carries only the meter. Volume
is absorbed by monthly segmentation and the light clear format. Inference
tallies feed `budgets` (§04.11) exactly like action tokens.

### 7.9.2 Kind registry (normative)

Kinds are a closed registry — an unregistered kind fails the entry (fail-
closed). Naming: `<domain>.<verb>`, lowercase.

| Kind | Class | Payload |
|---|---|---|
| `section.add/modify/delete/redact` | `ethos.write` | sealed body (keyed zones) |
| `ethos.read` | `ethos.read` | sealed body naming the section read |
| `action` | `act` | clear: action, args_hash, budget_ref?, tokens?, receipt?, checks?[] (§04.12) (+ sealed args body, §7.9.3) |
| `inference` | `act` | clear counters (§7.9.1) |
| `grant` / `revoke` / `rotate` / `merge` | structural | clear ids/versions |
| `heartbeat` | liveness | clear `{seq}` |

**Classes** are query-level groupings: filtering on `kind=ethos.write` matches
every `section.*` entry — wire kinds do not change (frozen vectors stay
frozen). `ethos.read` entries exist only under a `log_reads` mandate (§04.4):
reading is not journalized by default (I5 logs *acts*, not looks), and physics
cannot force a reader's pen — the flag makes read-logging a contractual duty,
checkable on presentations, honest about silent reads.

### 7.9.3 Sealed args (verifiable a-posteriori audit)

Where `action_params` predicates (§04.4) matter, the acting agent seals the
full argument object into the entry's `body_enc` — the F two-layer envelope
reused: clear counting header + `args_hash`, sealed `{target: "x.<connector>",
payload: <args>}` under the **connector's audit key** (the vault node
`x/<connector>`, owner-derivable, grantable like any node). The owner — or an
audit mandate holding the key — reopens the args, recomputes `args_hash`, and
re-evaluates the predicates against the mandate. A stranger sees the hash and
nothing else. Mismatched hash = tampered audit trail = rejection.

## 7.10 Committed gamma roots (proofs over the log)

> Decided 2026-07-11 (H2) — the §7.1 forward note, graved. Hashing and proof
> conventions are §2.10's, reused byte-for-byte: same `H_leaf`/`H_node`
> domains, same left-heavy `mroot`, same v1 proof wire (`node` steps only —
> the log folds no headers). Sealing the clear counting fields stays a future
> profile behind the `v` bump.

Each edition's manifest commits, beside `gamma_head` (§2.7) — additive like
§2.10's roots, absent from pre-H2 editions whose chain hashes are untouched:

```jsonc
"gamma_roots":       { "2026-07": { "root": "<hex>", "n": 42 }, … },  // one per non-empty segment
"gamma_counts_root": "<hex>"                                          // the counts trie
```

**Segment roots.** Per segment (§7.1), in **chain order** — the log's order is
the truth, nothing is sorted:

```
root = mroot([ H_leaf(JCS(entry)) for every entry in the segment ])
n    = the segment's entry count
```

An entry's inclusion proof is the v1 wire against its segment root: claimed
payload = the entry's JCS bytes, `node` steps up. Committing `n` gives
enumeration nowhere to hide: a mirror serving a whole segment must produce
exactly `n` entries whose recomputed `mroot` is the pinned root — one omission
and the root dies. Single entries keep O(log n) random access; across
segments, `prev` stays the binding truth.

**The counts trie.** One leaf per mandate the reachable chain (§7.6) has ever
counted — the §7.4 meters, post-override semantics (attested tokens beat
declarations):

```jsonc
{ "entries": 5,                    // ALL kinds whose authorized_via contains the id — the audit total
  "actions": 3,                    // `action` entries whose authorized_via contains the id (§7.4 subtree rule)
  "children": 1,                   // `grant` entries whose authorized_by is the id
  "budgets": { "haiku": { "actions": 2, "tokens": 4200 } } }
                                   // per cited budget_ref, same subtree rule: actions = `action`
                                   // entries citing it; tokens = the §04.11 tally over
                                   // `action` + `inference` entries citing it
```

Zero counters and empty maps are omitted; a mandate with nothing counted has
no leaf (owner-signed entries carry no `authorized_via` and feed no leaf).
Leaves sort by mandate id, byte order:

```
leaf              = H_leaf( mandate_id ‖ 0x00 ‖ JCS(counters) )
gamma_counts_root = mroot(leaves)     // 32×0x00 when nothing was ever counted
```

A **count proof** is the v1 wire against `gamma_counts_root`, claimed payload
= `mandate_id ‖ 0x00 ‖ JCS(counters)`. Every *total* cap of §7.4/§04.11 —
`max_actions`, `max_children`, budget action caps, token budgets — becomes one
O(log n) proof instead of a raw tally. Rolling windows (`max_actions_per`,
`rate_limit`) are not trie-able — no static counter carries a sliding window;
a verifier scans just the touched segments, enumeration-complete under their
committed `root`+`n`.

**Absence.** Sorted leaves make "never counted" provable: show two leaves
**adjacent in the tree** whose ids bracket the queried id. Adjacency is
checkable from the two proofs alone — above their divergence level the step
lists are identical; at it, each proof's sibling hash equals the replay of the
other proof's lower steps; below it, the left proof carries only
`side:"left"` steps (rightmost leaf of the left subtree) and the right proof
only `side:"right"` (leftmost of the right). At the rims: a proof of all
`side:"right"` steps is the first leaf, all `side:"left"` the last — brackets
for ids before or past the range. The empty trie is absence of everything.

**Completeness for mirrors (§7.8, closed).** "Every action under mandate M"
is now provable: the count leaf fixes k (`actions` — or `entries` for the
full audit trail), then k inclusion proofs of pairwise-distinct entries, each
visibly carrying M in its clear `authorized_via`. Distinctness is byte-level:
one tree position cannot verify two different payloads, so k distinct entries
at k verifying proofs are k real log lines. Withholding breaks the count,
forging breaks the root — before H2 only the second was true. Date-ranged
queries compose by segment (§7.8), each enumeration-complete.

Appending is untouched — an author still needs only `gamma_head`. The
appender-side checks (§7.4) keep their raw tallies: they hold the log, same
bytes, same result. Publish recomputes the roots; verification recomputes
them from the files and compares, like every §2.10 root — a trie that
disagrees with the raw tallies of the reachable chain is an invalid edition.

Honest limits: a proof shows inclusion in a signed edition, never freshness
(§7.7 bounds staleness, double counting inside the window included).
Committed roots shrink what a verifier must fetch — they do not change the
serverless trust model.

**W1.1 leaves H2 raw.** Segment roots continue to hash every exact Gamma line JCS;
a v2 line therefore includes its signed `operation_ref` when required, while a v2
heartbeat does not. The segment count `n` still counts the physical lines.

The counts-trie schema and tally inputs remain exactly those defined above:
`kind`, `authorized_by`, `authorized_via`, and the existing payload counters.
`occurrence`, `commitment`, and evidence outside Gamma are not count keys.
Implementations MUST NOT group or deduplicate Gamma lines by `operation_ref`.
Two valid occurrences with identical effects remain two raw lines; a replay or
equivocation instead invalidates the edition even if its mechanically recomputed
roots match. A heartbeat contributes to its segment root and `n`, but an
owner-signed heartbeat still creates no mandate count leaf. W1.1 added no mutation
or total-consumption field to H2 and changed no historical H2 root or proof; D7
uses the separate trie below rather than reinterpreting that historical wire.

### 7.10.1 Delegated occurrence counts

> **D7-CB2 counter wire — human-validated on 2026-07-18.**

`gamma_counts_root` remains exactly the historical raw-Gamma counter trie above.
Its `entries` field counts lines, not canonical occurrences, and MUST NOT be used
as `max_consumptions`. Some countable occurrences have evidence outside Gamma, and
several views may evidence one occurrence.

The draft2 public evidence set therefore carries one closed reference:

```json
"delegated_counts": {
  "aithos-delegated-counts-core": "1.0.0-draft.1",
  "root": "<64 lowercase hex>"
}
```

The exact enclosing evidence-set table and sidecar path remain separately gated;
the nested object above has exactly two non-null members. `root` is a bare
lowercase 32-byte SHA-256/Merkle hex value, consistent with existing root wire.

One leaf exists per mandate whose verified subtree has a non-zero new count:

```json
{ "mutations": 2, "consumptions": 7 }
```

`mutations` and `consumptions` are unsigned integers. Zero fields are omitted; an
empty counter object has no leaf. Leaves sort by exact mandate id and reuse the H2
Merkle primitives byte-for-byte:

```text
leaf = H_leaf(mandate_id || 0x00 || RFC8785-JCS(counters))
root = mroot(sorted leaves)
```

Mutation and total deltas are the exact D7 table in §4.4. A verifier first validates
and correlates every operation reference and its native evidence, then deduplicates
by occurrence, then increments each mandate in that grantee actor's
`authorized_via` chain. Reusing an occurrence with a different commitment is
equivocation and fails before tallying. The same occurrence seen through several
valid views contributes once.

A proof reuses the v1 Merkle proof wire with claimed payload
`mandate_id || 0x00 || JCS(counters)`. Absence uses the existing adjacent-leaf
proof. The root is recomputed during append-time authorization and cold replay from
the same accepted occurrence set; an injected tally, missing evidence view,
duplicate occurrence, or unknown profile fails closed.

Malformed `delegated_counts` material, an invalid leaf or proof, a tally mismatch,
or an occurrence-correlation failure returns
`Error::InvalidDelegatedCounts(String)`. A malformed or non-attenuating
`max_mutations` / `max_consumptions` certificate remains
`Error::InvalidMandate(String)`.

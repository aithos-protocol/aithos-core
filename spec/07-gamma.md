# 7 — Gamma log

> **Status: DRAFT.** The subject's tamper-evident record of every mutation and every
> agentic action. It is both the history spine (as in Aithos v0.x) and the enforcement
> substrate for counting constraints (I5).

## 7.1 Chain

`gamma/gamma.jsonl` — one JSON entry per line, SHA-256 hash-chained:

```jsonc
{ "id": "gamma_01JZ…",                       // ULID
  "prev": "sha256:…",                        // hash of the previous entry's JCS
  "at": "2026-07-08T10:12:00Z",
  "kind": "section.add" | "section.modify" | "section.delete" | "section.redact"
        | "action" | "heartbeat" | "grant" | "revoke" | "rotate" | "merge",
  "target": "/e/circle/s/gmail:0042",        // node path, when applicable
  "authorized_by": "mandate_01JZ…",          // omitted for owner-sphere-signed entries
  "authorized_via": ["mandate_root…","mandate_leaf…"],   // chain, for delegated/agentic
  "payload_enc": { "n": "…", "c": "…" } | "payload": { … },   // §7.3
  "signature": { "alg": "ed25519", "key": "<sphere URL | grantee pubkey>", "value": "…" } }
```

The manifest pins `gamma_head` (§02.7). Any past-entry alteration breaks `prev` and
every downstream signature — a write-once log. Redaction is the public, logged
`section.redact` (never silent deletion).

## 7.2 Who may sign an entry

- **Owner** entries: signed by the relevant sphere key; no `authorized_by`.
- **Delegated** entries (mutations or actions by an agent): signed by the leaf grantee
  key (and the session key if `session_bind`), carrying `authorized_by` = leaf id and
  `authorized_via` = full chain. Verified by §04.5 + §05.3 at the entry's `at`.

## 7.3 Payload encryption

For a mutation on an encrypted node, `payload_enc` is AEAD under the **target
section's content key** (purpose `gamma-payload`): the log reveals *that* a section
changed and *by whom under which mandate*, but its content is readable only by those
who can read the section itself. Public-zone and structural entries (grant/revoke/
rotate/heartbeat) use clear `payload` (ids, versions, mandate ids — no secrets).

## 7.4 Action accounting (I5, the agentic meter)

Every connector action taken under a mandate MUST append an `action` entry:

```jsonc
{ "kind": "action", "at": "…", "target": "x.gmail",
  "authorized_by": "mandate_01JZ…", "authorized_via": [ … ],
  "payload": { "action": "reply", "args_hash": "sha256:…",
               "purpose_ref": "…", "co_sign": { … }? },   // co_sign iff binding
  "signature": { "key": "<grantee pubkey>", … } }
```

Consequences, all verifier-checkable offline:

- `max_actions: N` ⇒ count entries whose `authorized_via` **contains** this mandate id
  (subtree count: a descendant's action consumes every ancestor's budget); the N+1-th
  is invalid. A delegate can never multiply its parent's budget by issuing children.
- `max_children: N` ⇒ count `grant` entries whose `authorized_by` is this mandate.
  Minting a sub-mandate MUST append its `grant` entry — otherwise `issue` would be a
  silent action, contradicting I5.
- `max_actions_per/{rate_limit}` ⇒ windowed count over `at`.
- `binding`/`counter_sign` ⇒ entry MUST carry a valid `co_sign` (§04.6) or it is
  invalid (and any effect it claims is unattributable).
- `purpose` ⇒ entry cites `purpose_ref`; audit trails intent.

The log is therefore the **counter** the serverless design otherwise lacks: state that
would need a server becomes append-only evidence anyone can tally. An action asserted
without its entry is, by I5, unauthorized.

## 7.5 Heartbeat entries

`kind:"heartbeat"` entries are owner-sphere-signed liveness beacons (§04.8), clear
payload `{seq}`. Heartbeat-bound mandates are valid only while the latest beacon is
within `every+grace` of `T`. Cheap (one tiny entry per period), and they double as a
freshness anchor for offline verifiers.

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

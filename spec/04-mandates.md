# 4 — Mandates

> **Status: DRAFT.** The certificate plane. A mandate authorizes a grantee keypair to
> read/author a perimeter and take agentic actions, under constraints, for a window.
> This chapter defines the document, the full agentic constraint vocabulary, and the
> offline verifier.

## 4.1 Document

```jsonc
{ "aithos-mandate-core": "1.0.0-draft.2",
  "id": "mandate_01JZ…",                     // mandate_<ULID>
  "subject": "did:aithos:z6Mkr…",            // whose ethos; constant along a chain
  "parent": null,                            // non-null ⇒ sub-mandate (§05)
  "issued_by": "did:aithos:z6Mkr…#root",     // root DID URL (root) OR grantee pubkey (sub)
  "grantee": { "id": "urn:aithos:agent:gmail@laptop",
               "label": "Gmail agent",
               "pubkey": "z6MkGrantee…",       // Ed25519 — REQUIRED
               "kex_pubkey": "z6LSGrantee…" }, // X25519 — REQUIRED, = ed2x(pubkey)
  "perimeter": [ "read.circle#tag=test", "edit.circle#tag=test",
                 "read.public#dir=01J…X/01J…4&tag=toto",   // folder-local tag view
                 "read.self#id=01J…A",
                 "act.x.gmail.reply", "issue#depth=1" ],
  "constraints": { … },                       // §4.4
  "not_before": "2026-07-08T00:00:00Z",
  "not_after":  "2026-07-15T00:00:00Z",
  "issued_at":  "2026-07-08T09:00:00Z",
  "nonce": "rNlx4L9k3qBp",
  "signature": { "alg": "ed25519", "key": "…", "value": "…" } }
```

### 4.1.1 Version profiles and migration

> **T1 protocol decision — human-validated on 2026-07-18.**

`"1.0.0-draft.1"` remains a supported historical verification profile. A verifier
MUST preserve its signed bytes and apply the attenuation semantics frozen by the
historical vectors; in particular, the E+ case in which a child omits its parent's
`max_children` remains valid under `draft.1`.

`"1.0.0-draft.2"` is the current issuance profile. It changes only the versioned
rule identified in T1: a parent's `max_children` is non-droppable, including when
the child is a chain leaf. It does not reinterpret any `draft.1` certificate.

> **K1-B mandate migration decision — human-validated on 2026-07-18.**

Mandate `"1.0.0-draft.3"` is the first profile permitted to carry the approved
connector-catalog pins and the typed non-action obligation matcher. It is a new
homogeneous issuance profile, not a companion certificate, sidecar extension, or
reinterpretation of `draft.1`/`draft.2`. Migration reissues the complete chain with
fresh mandate ids and normal Gamma v2 `grant` occurrences.

Every catalog binding applicable at a parent is non-droppable: each child keeps the
same exact approved catalog reference or selects a strictly narrower set already
permitted by its parent. A changed catalog digest, version, class assignment, owner
approval, or action set requires new draft3 authority; it never widens an existing
chain. Catalog signature and owner approval remain distinct proofs (§08.1).

The complete draft3 member names, catalog-reference object, typed matcher,
attenuation encoding, and signature vectors remain reserved. Until those tables are
human-validated, `draft.3` is not issuable and an emitter MUST NOT guess its bytes.

A delegation chain is version-homogeneous: every certificate from root through
leaf carries the same `aithos-mandate-core` value. Any link between different
profiles is invalid before attenuation is evaluated. Migration is not an in-place
edit: the authorities reissue the complete chain in issuer order under the selected
newer profile, producing fresh certificates and normal `grant` Gamma records.
Existing `draft.1` and `draft.2` certificates, signatures, and historical vectors
remain byte-identical and continue to verify under their declared profile until
their ordinary expiry or revocation.

**Form is verified before signature trust (T3).** A verifier first validates the
supported `aithos-mandate-core` profile; mandate, subject, parent, issuer, and
grantee identifier forms; both public-key encodings and their conversion; a nonce
that is a non-empty string; parseable RFC 3339 Zulu timestamps and a non-inverted
validity window; and the complete perimeter grammar. `signature.alg` is exactly
`ed25519`. On a root mandate, `issued_by == subject + "#root"` and
`signature.key == "#root"`; on a child, both `issued_by` and `signature.key` equal
the parent grantee public key. `issue#depth=0`, duplicate `dir`/`tag`/`id`
selectors, and any entry combining `id` with another selector are invalid form.
Only a form-valid document is canonicalized and submitted to signature
verification. The current spec defines no tighter nonce alphabet or length; any
tighter signed-byte rule requires the independent T3 vector in CB2 and MUST NOT be
invented by one implementation.

`kex_pubkey` MUST equal the Ed25519→X25519 conversion of `pubkey` under the normative
map (§01.2); a mismatch invalidates the mandate. Header lines seal to `kex_pubkey` —
nothing is left implicit, yet the grantee still owns exactly one keypair (the owner's
`owner_kex` is already explicit; this symmetrizes the grantees).

The grantee's content keys are **not** in the mandate — they live in headers (§03).
The mandate is a pure certificate: publishable, verifiable, revocable. (A convenience
delivery bundle MAY ship the mandate together with the header lines the grantee needs,
but the lines are authoritative in the bundle, not in the certificate.)

**Widening an existing agent.** Mandates are immutable (I2). To extend a grantee's
perimeter, issue a **fresh mandate to the same keypair**, append the missing header
lines (O(1) each, §03.3), and revoke the old certificate politically only (rung 1,
§06) — no rotation: the same keypair legitimately keeps its existing lines. The gamma
records the widening (`grant` + `revoke` entries). Multiple simultaneous mandates for
one keypair are valid; a verifier evaluates whichever chain is presented.

## 4.2 Perimeter grammar

```
perimeter-entry :=
    <verb> "." <zone> [ "#" <selector> ]        ethos
  | "act." "x." <connector> "." <action-pat>    connector action
  | "issue" [ "#depth=" <n> ]                   delegation right (§05)
  | "revoke" [ "." <zone> [ "#" <selector> ] ]  revocation right, certificate half only (§06.7)
  | "read.gamma" [ "#" <gamma-selector> ]       log read (§07.8) — read is gamma's only verb

verb       := read | edit | append | delete | write
zone       := public | circle | self
selector   := sel *( "&" sel )                  conjunction = intersection
sel        := dir=<sid-path> | id=<sid> | tag=<tag>
sid-path   := <sid> *( "/" <sid> )              folder path from the zone root
action-pat := <action> | "*"
gamma-selector := gsel *( "&" gsel )
gsel       := dir=<sid-path> | id=<sid> | tag=<tag>
            | kind=<kind> | action=<action> | since=<ts> | until=<ts>
```

At most one `dir`, one `tag`, one `id` per entry. Semantics: `dir=` alone = the whole
subtree; `tag=` alone = the zone-root tag view; `dir=…&tag=…` = the folder-local tag
view (§02.9) — read what carries the tag under that folder, now and in the future;
`id=` = one section (sids are global, so `id` composes with nothing). No selector =
the whole zone. `covers()` is per-dimension: `dir` containment is **nodal** — a
`dir` names its granted folder by the **terminal sid** of the recorded sid-path
(the leading segments are the folder's address at issuance, kept for audit, never
a constraint). An empty `dir` is the zone root and covers the zone. A non-empty
`dir` covers a target iff the target's chain **passes through the terminal sid**:
for an operation, the target's *current* resolved chain; for entry-vs-entry
containment (§05.3), the child entry's recorded sid-path. On a tree that never
moved this is exactly segment-list containment (`dir=a/b` covers `a/b` and
`a/b/c`, never `a/bc` — sids are unique); it diverges only after a move (§02.9):
the perimeter follows the **node**, not its address, so a directly granted folder
keeps its grants when moved, while grants on the old parent no longer cover it.
An absent dimension covers any value of it (no-`tag` covers any `tag`); `tag`
covers only the equal `tag`. `gamma-selector` `dir`s are **not** nodal: they
filter log entries by their recorded, historical coordinates (§07.8).
(The former `ns=` selector is gone: a namespace is a depth-1 folder, §02.2.)

Verb lattice (normative): `read ⊑ edit ⊑ append ⊑ write`,
`read ⊑ delete ⊑ write`; `delete` is otherwise incomparable with `edit` and
`append`. Operationally, create requires `append` or `write`; editing an existing
object accepts `edit`, `append`, or `write`; deletion accepts `delete` or `write`
and always includes read authority; `write` is full CRUD. There is no wire verb
`create`. Multiple entries union per verb. A selector matching nothing yet is a
valid forward-looking grant. Enforceability of a write perimeter:
`id=`/`dir=` are clear placements on `public`/`circle` (hard); `tag=` writes are hard
on `public`/`circle` (clear tags, §07 authorship cross-check). On `self`, structure
and tags are sealed (§02.8): `dir=` and `tag=` perimeters there are **read-only**;
`self` writes use `id=` or zone-level grants.

The same verbs cover sections and folders (D6): `read` lists and presents only
covered objects. Rename, title, body, and tag changes are edits. A move requires
edit authority on the source node and append/write on the destination; deleting a
folder requires delete/write coverage for the folder and every affected descendant.
Index rows, tag views, reindexing, required rotation, rewrap, and re-encryption are
deterministic consequences committed in the same transaction, not extra silent
mutations. Trust-root, succession, and recovery rotation remain owner-only;
`issue`, `revoke`, and vault `.config` retain their dedicated rights and are never
implied by `write.<zone>`.

`read.gamma` grants log reading (§07.3, §07.8): appending never needs it (any
mandate's actions imply their own entries), and by default nobody but the owner
reads. `covers()` extends per-dimension as above: `dir`/`id`/`tag` scope entries by
their (sealed) targets, `kind=` covers only the equal kind, `action=` only the
equal clear `payload.action` (audit "replies only"), `since`/`until` bound the
entries' `at`. Enforcement is split honestly: `dir`/`id`/`tag` are **physics**
(the sealed bodies only open under node keys the grantee holds, §07.3);
`kind`/`since`/`until` are certificate policy — binding for honest verifiers,
key-holders can physically scan what their keys open. Attenuation (§05.3) applies
dimension-wise like everywhere else.

Wildcard (normative `covers` rule): `act.x.<c>.*` covers every action the connector's
manifest (§08.1) classes as `read` or `act`; it NEVER covers an action classed
`binding` — binding actions must be named explicitly (and still obey the
`counter_sign`/`binding` constraints, §4.6). A `revoke` entry conveys no key and no
read: only the authority to publish revocation entries for mandates whose perimeter
it covers; attenuation applies (§06.7). A bare `revoke` covers the issuer's own
revocable scope.

## 4.3 What each perimeter entry needs at the key layer

| Entry | Header line the grantee needs |
|---|---|
| `read.public` | none (plaintext) |
| `*.circle` / `*.self` (no selector) | line on `/e/<zone>` (root folder) |
| `*#dir=<p>` | line on the folder node `/e/<zone>/d/…` — opens the whole subtree by derivation |
| `*#tag=<t>` | line on the zone-root view `/e/<zone>/t/<t>` (+ wraps bridge sections, §02.9) |
| `*#dir=<p>&tag=<t>` | line on the folder-local view `/e/<zone>/d/…/t/<t>` (+ wraps, §02.9) |
| `*#id=<x>` | line on the section node `/e/<zone>/…/s/<x>` |
| `act.x.<c>.<a>` | none (action authorization only; config needs vault line, §08) |
| `issue` | none |
| `revoke` | none — by design; the political cut carries no key (§06.7) |

Issuance = mint the certificate **and** append the needed header lines (§03.3). Both
are owner/delegate-local, offline.

## 4.4 Agentic constraint vocabulary (normative)

`constraints` is always an object. Every known key is parsed and validated in its
typed form, including on a directly owner-issued root; a malformed known value is
invalid. For forward compatibility, an unknown key/value is preserved and tolerated
only when that root mandate is also the chain leaf. A mandate carrying it cannot be
a delegation parent, and an unknown key on any parent/child link is invalid because
its attenuation law is unknown.

> **CB1 decision G-E — validated at the human protocol gate on 2026-07-18.**
> Structural tolerance is not permission to consume. The current opaque key/value
> form has no Core-understood applicability envelope, so an unknown root-leaf
> extension cannot prove itself non-applicable: every attempted consumption under
> that mandate is refused with a typed “extension not understood” decision class.
> That phrase is conceptual in CB1, not a frozen wire/API error name. Structural
> tolerance permits parse, preserve, and audit only. A later approved protocol
> version, frozen by independent vectors, may define a signed typed extension
> envelope whose applicability, attenuation, and enforcement Core understands. Only
> that future version may permit proven non-applicability; existing mandate bytes
> are never reinterpreted. The refusal is exposed to the caller's operational audit
> but appends no Gamma entry, changes no canonical state, and consumes no counter. No
> Bundle or downstream surface may convert it into Allow or claim the extension was
> enforced.

Each known key states its **enforcement tier**: **V** verifier (offline, from files)
or **X** executor/tool-host (runtime). (Counter-signature, once its own tier **C**,
is now the owner instance of an obligation — tier V, §4.12.)

| Key | Meaning | Tier |
|---|---|---|
| `not_before` / `not_after` | validity window (top-level, but listed for completeness) | V |
| `max_actions: N` | at most N gamma action entries carry this mandate in `authorized_via` over its life — a **subtree** count, see below | V (count §07) |
| `max_children: N` | at most N direct sub-mandates ever issued under this mandate (bounds delegation *width*; `depth` bounds only length, §05.7) | V (count §07) |
| `max_sessions: N` | at most N session keys simultaneously certified by the grantee's long-term key (§4.7) — blocks silent duplication of one mandate across N machines | V |
| `max_actions_per: {window,N}` | ≤ N actions per rolling window (`"1h"`,`"1d"`) | V+X |
| `rate_limit: {action,window,N}` | per-action-kind rate cap | V+X |
| `active_windows: [window]` | union of absolute arithmetic windows (§4.10); acting outside every window is invalid — replaces the former `active_hours` (calendar/timezone live in the issuing tool, never in the verifier) | V |
| `budgets: [profile]` | OR-composed budget profiles (§4.11): model list, token budget, windows, action cap, attestation hook; actions and inferences MUST cite a `budget_ref` | V+X |
| `log_reads: true` | the grantee MUST journalize its reads as `ethos.read` entries (§07.9); off by default — reading is not logged under I5. Physics cannot force a reader's pen: an honest verifier of a *read presentation* requires the entry; a silent read stays possible and is exactly what this flag makes contractually visible | V+X |
| `obligations: [obligation]` | listed actions may consume only if carrying a valid signed receipt from a pinned attestor whose verdict satisfies the predicate (§4.12): guardrail pass, human approval, dual control | V |
| `counter_sign: [actions]` | shorthand for the **owner-approval** obligation: listed actions require a fresh owner co-signature (§4.6, §4.12) | V |
| `binding: [actions]` | legacy mandate shorthand that adds the owner `co_sign` obligation to the named actions; it never classifies an action (§4.6, §08.1) | V |
| `domains: [patterns]` | connector actions may touch only these domains/recipients | X |
| `action_params: {action: predicates}` | per-action argument predicates (allow-listed recipients, subject patterns, no-attachments, numeric caps) — generalizes `domains`. Enforced on the real args by the container (X); **auditable at V** through the sealed args body of the entry (§07.9): the owner reopens the args and re-evaluates the predicates | X (+V audit) |
| `disclose_agency: true` | the agent MUST identify itself as an agent in every outbound communication of a connector action (transparency; EU-AI-Act-aligned) | X |
| `notify: [events]` | out-of-band owner alert on the listed events; best effort, never a validity condition | X |
| `purpose: "<text>" ` | signed statement of intent bound through `authorized_via` to every delegated consumption; existing actions also cite it; audited | V+X |
| `session_bind: <pubkey>` | delegated consumptions valid only from this ephemeral session key (§4.7) | V |
| `heartbeat: {every, grace}` | mandate valid only if owner liveness beacon < every+grace old (§4.8) | V |
| `freshness: <duration>` | verifier MUST have revocation state newer than this (§06.5) | V |
| `spend_cap: {unit, amount}` | cumulative external spend ceiling (audited; for future paid actions) | X |
| `first_party_only: true` | grantee id MUST be under the subject's control (self-agent) | V |

These compose; the effective constraint is the **conjunction**. Sub-mandates may only
tighten (§05.3).

`max_actions` counts over the **subtree**: an action entry counts against its leaf
mandate and against every ancestor named in its `authorized_via` chain (§07.4). A
delegate can therefore never multiply its parent's budget by issuing children.
Corollary: minting a sub-mandate is itself a `grant` gamma entry (§07) — issuance is
never a silent action (I5) — and that entry is what `max_children` counts.
`max_children` bounds only children whose `parent` is that exact mandate. Under
`draft.2`, if present on a parent it is non-droppable: every child repeats it with
a value less than or equal to the parent's, and omission is a widening even for a
chain leaf. Under historical `draft.1`, omission retains the valid per-level-width
meaning frozen by E+; the parent still counts its own direct children, while the
child has no width cap of its own. It is not a subtree-descendant counter.

`max_actions` and its rate derivatives remain connector-action meters; content
mutation never consumes them. D7 additionally requires one explicit Ethos-mutation
limit/counter and one explicit total-delegated-consumption limit/counter. Their
conceptual counting boundaries are fixed by §4.13. Wire names, encodings, and
concrete migration mechanics remain reserved until CB2 supplies independent
vectors: the mutation and total-consumption meters are validated semantics but do
not exist in the current wire. A counter schema becomes enforceable only through an
explicitly versioned signed protocol contract freezing leaf encoding, roots, replay,
and migration. Historical artifacts are evaluated under their declared historical
protocol version and remain byte-identical; existing versions, `max_actions`, Gamma
kinds, and count roots are never reinterpreted to simulate a new meter. Added meter
material under an old or unversioned schema, and an unknown counter-schema version,
fail closed. Implementations MUST NOT synthesize new committed bytes for historical
artifacts or silently infer either new meter.

## 4.5 Verifier algorithm (offline)

To verify grantee G may do `OP` on subject `DID` at time `T` from a presented chain:

```
1. Resolve DID document; verify root signature; get the owner public keys +
   revocation pointer.
2. For the root mandate: issued_by is the root key URL; verify signature; subject==DID.
3. not_before ≤ T ≤ not_after for every mandate in the chain.
4. Revocation: none of the chain's ids is revoked at T (§06.5); freshness honored.
5. Chain attenuation (§05.3) holds link by link.
6. OP ∈ effective perimeter of the leaf (verb lattice + selector match).
7. Constraints tier V all pass (counts via gamma §07, session_bind, heartbeat,
   obligations §4.12 — incl. counter_sign co-signatures — …).
8. Proof of possession: the presented request or evidence proves possession of
   `leaf.grantee.pubkey`. A W1 operation subject to an effective `session_bind`
   additionally proves possession of that exact session key; both independent
   proofs bind the same exact `operation_ref` (§4.7).
9. Tier X constraints are handed to the executor (real-arg predicates, model
   truth). Obligations — including binding/counter_sign co-signatures — are
   **tier V**, already enforced at step 7 from the signed receipts in the log.
```

Fail any ⇒ reject. Every step reads files (DID doc, certs, gamma — revocation state
included, §06.5); none needs a live server.

### 4.5.1 Canonical operation occurrence and commitment

> **W1 protocol decision — human-validated on 2026-07-18.**

A canonical-operation commitment is an independent protocol profile. It does not
change a mandate profile, reinterpret a Gamma v1 entry, or derive an identifier
from an existing Gamma id, `args_hash`, edition hash, or receipt.

Its projection has exactly these top-level members:

```jsonc
{ "aithos-operation-core": "1.0.0-draft.1",
  "occurrence": "op_<ULID>",
  "subject": "did:aithos:…",
  "at": "2026-07-18T12:00:00Z",
  "history_heads": ["sha256:…"],
  "authority": { … },
  "operation": { … } }
```

`occurrence` is allocated before any native evidence is signed. Its ULID timestamp
is ordering convenience only and conveys no authority or trusted time. Two
otherwise identical effects with different occurrence anchors are distinct
operations.

Every `history_heads` member is `sha256:<64 lowercase hex>`. The array is empty
when no prior Gamma head exists, carries one head for an ordinary operation, or
two distinct heads sorted by ASCII bytes for a merge. Candidate entry hashes and
the resulting Gamma head are outputs and never projection inputs.

> **W1.1 A1/SC1 authority decision — human-validated on 2026-07-18.**

`authority` is a closed owner-or-grantee variant. The owner variant is exactly:

```json
{"actor":"owner"}
```

The grantee variant without an applicable session is exactly:

```json
{
  "actor": "grantee",
  "key": "z6Mk…",
  "authorized_by": "mandate_…",
  "authorized_via": [
    {
      "id": "mandate_…",
      "certificate_digest": "sha256:<64 lowercase hex>"
    }
  ]
}
```

When the verified chain has an effective `session_bind`, the same object has the
additional exact member:

```json
"session": {
  "key": "z6Mk…",
  "certificate_digest": "sha256:<64 lowercase hex>"
}
```

No other `authority` member is permitted. `authorized_via` is non-empty, contains
one element per mandate in issuer-to-leaf order, and contains no duplicate id.
Every element's `id` equals the id inside the corresponding certificate;
`authorized_by` equals the final id; and `key` equals that leaf certificate's
`grantee.pubkey`. The certificates form the exact homogeneous chain verified for
`subject` at the projection's `at`.

For any complete signed mandate or SC1 session certificate `C`:

```text
certificate_digest(C) =
  "sha256:" ||
  lowercase_hex(SHA-256(RFC8785-JCS(C)))
```

The digest covers the complete signed certificate, including its signature value;
it does not cover arbitrary non-canonical storage bytes. Duplicate JSON members,
a malformed certificate, a non-lowercase digest, a digest mismatch, or a
certificate whose embedded identity does not equal the accompanying `id` fails
closed.

`session` is present if and only if an effective `session_bind` applies.
`session.key` equals that exact bound Ed25519 public key and
`session.certificate_digest` pins the complete signed SC1 certificate described in
§4.7. Without `session_bind`, `session` is omitted; `null`, an empty object, or a
volunteered session fact is invalid.

> **K1-B operation-reference architecture — human-validated on 2026-07-18.**

`operation` is exactly this closed two-member object:

```jsonc
"operation": {
  "kind": "read",
  "facts_ref": {
    "aithos-operation-facts-core": "1.0.0-draft.1",
    "digest": "sha256:<64 lowercase hex>"
  }
}
```

`kind` is exactly one of `read`, `mutation`, `action`, `inference`, `grant`,
`revoke`, `rotate`, or `publication`. It selects one closed facts family:

- `read` covers an Ethos read, an explicitly signed Gamma presentation, or a
  vault-config read. Its closed facts bind the exact source edition or Gamma head
  plus the applicable SID, canonical query-request digest, or vault record-key
  commitment;
- `mutation` covers an Ethos, structural, or vault-config mutation. Its closed facts
  select domain `ethos`, `structure`, or `vault-config`, the exact verb and target,
  applicable source/destination, and before/after state;
- `action` binds every applicable connector, exact action, approved-catalog,
  argument, budget, and purpose fact known before effect;
- `inference` binds its provider, model, private-request commitment, applicable
  budget, and purpose facts without inventing an `args_hash`;
- `grant`, `revoke`, and `rotate` bind their affected identifiers and applicable
  certificate or state commitments;
- `publication` selects mode `normal`, `merge`, or `resolution` and binds height,
  predecessors or winner, factual changeset commitment, and contained operation
  references.

`facts_ref` has exactly the two members shown. No member is nullable or optional.
Its profile is exactly `"1.0.0-draft.1"` and its digest has the same strict lowercase
SHA-256 text form as other W1 digests. A missing or extra member, another kind,
unknown facts profile, malformed digest, or facts document whose selected family
does not equal `operation.kind` fails before commitment comparison.

Actual `tokens`, `tokens_in`, and `tokens_out` remain post-effect facts: they MUST
NOT enter the pre-effect `action` or `inference` facts document or its digest and
are bound afterward by the applicable usage receipt v2 of §4.12.1.

> **K1.1-B facts and protected-state commitments — human-validated on
> 2026-07-18.**

The document selected by `facts_ref` is exactly this closed three-member object,
where `facts` is the closed object selected by `kind`:

```json
{
  "aithos-operation-facts-core": "1.0.0-draft.1",
  "kind": "mutation",
  "facts": {}
}
```

The profile literal MUST equal the profile in the enclosing `facts_ref`, and `kind`
MUST equal `operation.kind`. No member is optional or nullable, and no extra member
is permitted. The exact members of each selected `facts` object remain independently
gated by K1.2.

For any byte string `x`, define the domain-separated SHA-256 commitment:

```text
C(domain, x) =
  "sha256:" ||
  lowercase_hex(
    SHA-256(
      ASCII(domain)
      || 0x00
      || x
    )
  )
```

If `F` is the complete operation-facts document above, its reference digest is
exactly:

```text
facts_ref.digest =
  C("aithos-core/v1/operation-facts", RFC8785-JCS(F))
```

A logical before or after state is one of exactly two closed variants. Absence has
one member and no state document:

```json
{"state":"absent"}
```

Presence has exactly these two members:

```json
{
  "state": "present",
  "state_ref": {
    "aithos-state-fact-core": "1.0.0-draft.1",
    "digest": "sha256:<64 lowercase hex>"
  }
}
```

The nested `state_ref` has exactly the two members shown. The state-fact document
it selects is exactly:

```json
{
  "aithos-state-fact-core": "1.0.0-draft.1",
  "objects": [
    {
      "key_commitment": "sha256:<64 lowercase hex>",
      "byte_commitment": "sha256:<64 lowercase hex>"
    }
  ]
}
```

For each affected canonical store object with exact UTF-8 store key `K` and exact
stored bytes `B`:

```text
key_commitment  = C("aithos-core/v1/state-key", UTF8(K))
byte_commitment = C("aithos-core/v1/state-bytes", B)
```

`objects` is non-empty, contains each affected current store key exactly once, and
is sorted by ascending lexicographic order of the exact ASCII `key_commitment`
string. A duplicate `key_commitment`, a missing or unrelated object, an empty array,
an unsorted array, or an extra object member fails closed. Equal stored bytes under
different keys MAY have equal `byte_commitment` values.

If `S` is the complete state-fact document, the reference digest is exactly:

```text
state_ref.digest =
  C("aithos-core/v1/state-fact", RFC8785-JCS(S))
```

The state-fact object carries commitments only: no clear store key, path, SID,
vault record name, target, protected content, credential, DK, or private key. This
gate defines a logical committed document and its reference, not a public sidecar
path or a new disclosure rule. Only the references required by the applicable
public carrier are public. The selected K1.2 family binds the operation target to
the exact expected store keys; opaque `self` and vault evidence proves their
correspondence without disclosing those keys.

> **K1.2-R-B closed read facts — human-validated on 2026-07-18.**

When `operation.kind` is `read`, the selected `facts` object is exactly one of
three closed variants. An Ethos read has exactly these four members:

```json
{
  "domain": "ethos",
  "zone": "circle",
  "sid": "01J...",
  "source_edition": "sha256:<64 lowercase hex>"
}
```

`zone` is exactly `public`, `circle`, or `self`; `sid` is the canonical SID of the
section read. `source_edition` identifies the exact signed manifest whose state is
read. If `M0` is that manifest with only `signature.value` replaced by the empty
string, then:

```text
source_edition =
  "sha256:" ||
  lowercase_hex(SHA-256(RFC8785-JCS(M0)))
```

This is the existing manifest chain-hash preimage and digest with only the
`sha256:` textual prefix added; it does not rehash or reinterpret `prev_hash`.

An explicitly signed Gamma presentation has exactly these three members:

```json
{
  "domain": "gamma",
  "source_head": "sha256:<64 lowercase hex>",
  "request_digest": "sha256:<64 lowercase hex>"
}
```

`source_head` is the exact non-empty Gamma head against which the presented result
was selected. Let `Q` be the exact canonical UTF-8 `read.gamma` perimeter-entry
string representing the query. It uses the already-defined gamma-selector grammar
and canonical selector order `dir`, `id`, `tag`, `kind`, `action`, `since`,
`until`; each dimension occurs at most once, omitted dimensions are absent, and
the unfiltered query is exactly `read.gamma`. Then:

```text
request_digest =
  C("aithos-core/v1/gamma-read-request", UTF8(Q))
```

`Q` is query intent, not a signed carrier. It contains no `operation_ref`,
signature, result, or presentation digest, so the operation commitment remains
acyclic. The later signed presentation evidence binds the exact same `Q`,
`source_head`, and `operation_ref`.

A vault-config read has exactly these four members:

```json
{
  "domain": "vault-config",
  "connector": "mail",
  "record_key": "sha256:<64 lowercase hex>",
  "source_edition": "sha256:<64 lowercase hex>"
}
```

`connector` is the exact canonical connector identifier covered by the current
indivisible `.config` capability. `record_key` is the K1.1-B `key_commitment` of
the affected record's canonical store key; the clear vault record name is
forbidden. `source_edition` uses the same exact manifest chain hash as the Ethos
variant. The later opaque evidence binds that record commitment to the vault state
root without disclosing the store key, record name, credential, or plaintext.
This variant does not split `.config` into separately grantable read and write
rights and introduces no connector catalog action or new operation kind.

A local read without signed evidence still performs its operation-time authority
check but emits no operation-facts document, persists no `operation_ref`, and is
not cold-replayable or countable. An Ethos read journalized because `log_reads`
applies, an explicitly signed Gamma presentation, or signed vault-config read
evidence allocates exactly one `read` occurrence. Native evidence reuses that
occurrence and never creates another consumption.

Only `facts_ref` is required in the public W1 projection. The selected read facts,
including a `self` SID, Gamma query digest, or vault record-key commitment, remain
protected unless an independently approved evidence carrier requires them.
An unknown domain, null, missing or extra member, unknown zone, non-canonical SID,
malformed or mismatched source reference, non-canonical query spelling, mismatched
request digest, mismatched vault record-key commitment, clear display path, or
clear vault record name fails closed as `Error::InvalidOperationFacts(String)`
before an operation commitment is emitted.

> **K1.2-M-B closed mutation facts — human-validated on 2026-07-18.**

When `operation.kind` is `mutation`, the selected `facts` object is exactly one of
the following five closed variants. An Ethos section mutation always has these
seven members:

```json
{
  "domain": "ethos",
  "verb": "edit",
  "zone": "circle",
  "sid": "01J...",
  "dir": ["01J...", "01J..."],
  "before": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  },
  "after": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  }
}
```

`verb` is exactly `create`, `edit`, `delete`, or `redact`; `zone` is exactly
`public`, `circle`, or `self`; `sid` is the target section's canonical SID. `dir`
is the canonical root-to-leaf SID array of the target's parent folders: for
`create`, the requested destination parent; for every other verb, the current
resolved parent. The empty array means the zone root. Display names, titles, tags,
and body facts are not duplicated here: the applicable present state commits their
exact stored representation.

A structural `create` has exactly these eight members:

```json
{
  "domain": "structure",
  "verb": "create",
  "zone": "circle",
  "node_kind": "folder",
  "sid": "01J...",
  "destination": ["01J..."],
  "before": {"state":"absent"},
  "after": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  }
}
```

A structural `rename` or `delete` has exactly these eight members:

```json
{
  "domain": "structure",
  "verb": "rename",
  "zone": "circle",
  "node_kind": "folder",
  "sid": "01J...",
  "source": ["01J..."],
  "before": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  },
  "after": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  }
}
```

A structural `move` has exactly these nine members:

```json
{
  "domain": "structure",
  "verb": "move",
  "zone": "circle",
  "node_kind": "folder",
  "sid": "01J...",
  "source": ["01J..."],
  "destination": ["01J..."],
  "before": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  },
  "after": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  }
}
```

For the structural family, `verb` is exactly `create`, `rename`, `delete`, or
`move`. `node_kind` uses the existing `folder` or `section` literals. `create` and
`delete` admit `folder` only; section creation and deletion use the Ethos family.
`rename` and `move` admit either literal. `source` is the current parent-folder SID
array; `destination` is the requested destination parent-folder SID array. The
stable target `sid` is not repeated inside either array. Core reconstructs a
folder's resolved target chain as `parent-array + sid`; a section's `sid` remains
its exact `id=` coordinate under the parent array.

A vault-config mutation has exactly these six members:

```json
{
  "domain": "vault-config",
  "verb": "edit",
  "connector": "mail",
  "record_key": "sha256:<64 lowercase hex>",
  "before": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  },
  "after": {
    "state": "present",
    "state_ref": {
      "aithos-state-fact-core": "1.0.0-draft.1",
      "digest": "sha256:<64 lowercase hex>"
    }
  }
}
```

Its `verb` is exactly `create`, `edit`, or `delete`. `connector` is the exact
canonical connector identifier covered by the reserved `.config` capability.
`record_key` is exactly the K1.1-B `key_commitment` of the affected record's
canonical store key; the clear vault record name is forbidden. The commitment MUST
occur in every applicable present state's `objects` array and MUST equal the
independently derived affected key for an absent side of a create or delete.

The state transition matrix is closed:

| Family verb | `before` | `after` |
|---|---|---|
| every `create` | `absent` | `present` |
| every `delete` | `present` | `absent` |
| Ethos `edit` / `redact` | `present` | `present` |
| structure `rename` / `move` | `present` | `present` |
| vault-config `edit` | `present` | `present` |

Every `present` to `present` transition MUST have different `state_ref.digest`
values; a no-op is not a mutation occurrence. Each SID array contains canonical SID
strings in root-to-leaf order, contains no duplicate SID, and excludes the target
`sid`. `source` and `destination` are forbidden outside their exact variants;
`null`, an unknown domain, verb, zone or node kind, a missing or extra member, a
cross-zone structural move, a destination inside the moved node, an invalid state
transition, or a mismatched record-key commitment fails closed before an operation
commitment is emitted.

The operation-facts document is protected according to its operation context.
Only `facts_ref` is required in the public W1 projection. In `self`, `dir`,
`source`, and `destination` are committed private facts, never public proof
coordinates and never write authority by themselves: write authorization remains
exact `id=` or zone-wide, and the opaque state proof binds the claimed transition.

A1 fixes the complete `authority` member set, absence rules, digest input, and
reconstruction equalities above. K1-B fixes the complete `operation` and
`facts_ref` member sets, the kind registry, and the family split above. K1.1-B fixes
the operation-facts envelope and digest, both logical-state variants, the state-fact
envelope and object-entry table, all four digest domains, and the state-object
ordering and privacy rules. K1.2-R-B fixes the three read-family member sets, their
source references, canonical Gamma request digest, signed-evidence occurrence
boundary, vault record-key binding, privacy, and failure rules. K1.2-M-B fixes
every mutation-family member set, its domain/verb/node registries, coordinate
semantics, state-transition matrix, vault record-key binding, and failure rules.
The other selected-family `facts` member tables, exact proof encodings and
target-to-store-key derivations, changeset, catalog, SC1, receipt, authorship,
presentation, and carrier bytes remain reserved until their own independent tables
are human-validated. No producer may invent those remaining bytes or emit a
completed operation commitment before then.

The public reference has exactly this shape:

```jsonc
"operation_ref": {
  "aithos-operation-core": "1.0.0-draft.1",
  "occurrence": "op_<ULID>",
  "commitment": "sha256:<64 lowercase hex>"
}
```

For a completed projection `P`:

```text
commitment =
  "sha256:" ||
  lowercase_hex(
    SHA-256(
      ASCII("aithos-core/v1/operation-commitment")
      || 0x00
      || RFC8785-JCS(P)
    )
  )
```

Every applicable append-time, Gamma, authorship, and edition view of one occurrence
MUST carry the same `operation_ref`, and a verifier MUST independently reconstruct
the projection from that view. A mismatched version, occurrence, commitment, native
fact, or authority fact invalidates the view.

Several native view types may evidence one occurrence without creating another
consumption. Two distinct consuming Gamma entries reusing one occurrence or
operation-bound receipt are instead a replay and the later candidate is rejected
before admission or tally. Reusing an occurrence with a different commitment is
equivocation and is invalid.

The projection contains no private argument, body, credential, content key, or
private key. It also contains no digest or signature of a carrier containing its
own `operation_ref`, and no digest transitively dependent on that carrier. In
particular, a publication projection cannot include its current manifest or edition
hash. Predecessor hashes and independently defined factual state or changeset
commitments are not self-references.

A receipt, its `sig`, actual `tokens`, `tokens_in`, or `tokens_out`, and any digest
derived from them are likewise never inputs to the pre-effect operation projection.

Historical artifacts remain byte-identical and are verified solely under their
declared historical profiles. A verifier MUST NOT synthesize an `operation_ref` for
them. Commitment material under a historical, absent, or unknown profile is refused
closed.

## 4.6 Counter-signature and binding actions — the owner-approval obligation

The approved connector manifest is the sole source of the canonical `binding`
**class** (§08.1). A classed-binding action must be named exactly and accompanied by
the owner `co_sign` receipt. The mandate constraints `counter_sign:[actions]` and
legacy `binding:[actions]` do not classify or reclassify an action; both only tighten
the named actions by adding that same owner-approval obligation. Thus a catalog
`binding` action always needs `co_sign`, while a catalog `read`/`act` action may also
need it because the mandate explicitly says so.

The agent prepares the action, obtains the owner's live co-signature (out of band —
the human approves), then emits it with the Gamma entry. This is how "the AI may act,
but a commitment needs me in the loop" is expressed.

Both mandate shorthands desugar to the reserved obligation id **`co_sign`** —
attestor = the owner content key, `verdict: "approve"`, `max_age` = Δ_cosign
(normative default **5 minutes**).

The historical v1 receipt signs
`{obligation: "co_sign", mandate_id, action, args_hash, verdict,
presented_digest?, at}`. Its `args_hash`, mandate, action, and freshness bindings
prevent transfer to different signed facts and bound later reuse, but do **not**
make it one-shot by occurrence: v1 carries no occurrence identity. A second v1
candidate with the same leaf, action, arguments, and still-fresh receipt cannot be
rejected merely as receipt replay. Existing v1 bytes and verification semantics are
never reinterpreted.

A newly committed operation under the W1 profile instead uses the
operation-bound receipt v2 of §4.12.1. Enforcement remains **tier V** (§4.5 step 7):
the receipt is verified offline and is never delegated to a runtime executor.

## 4.7 Session binding

> **W1.1 SC1 double-proof decision — human-validated on 2026-07-18.**

`session_bind: <pubkey>` binds every delegated W1 consumption to that exact
ephemeral Ed25519 session key. The public session certificate is a separately
versioned, closed profile identified by
`"aithos-session-core": "1.0.0-draft.1"` (SC1).

Semantically, SC1 certifies for the operation subject and leaf mandate that the
leaf's long-term grantee key authorized the exact `session.key` for a short
validity interval containing the operation's `at`. Its signature MUST verify under
`authority.key`, and its complete signed certificate MUST reproduce
`authority.session.certificate_digest`. SC1 conveys no perimeter or authority of
its own: the mandate chain remains the sole authority source.

A session-bound operation requires two independent possession proofs:

1. the ordinary leaf proof under `authority.key`; and
2. a session proof under `authority.session.key`.

Both proofs MUST bind the same exact `operation_ref`. The leaf signature on the
SC1 certificate is authorization of the session key and does not replace either
operation proof. A missing proof, a proof for another occurrence or commitment, a
wrong key, an expired certificate, a subject or leaf mismatch, a bad certificate
signature, or a certificate-digest mismatch fails closed.

W1.1 does not fix SC1's remaining JSON member names, its complete member set, the
exact signature block or signed preimage, the carrier location of the certificate,
or the member name and signed preimage of the second operation proof. Those bytes
require a later human-validated carrier table and independent vector. Until then,
no implementation may invent `session_signature` bytes or claim a session-bound
carrier wire-complete.

SC1 also does not define session issuance, replacement, revocation, expiry
indexing, or the public set of simultaneously active sessions. Consequently the
`max_sessions` lifecycle and counter remain reserved and fail closed until their
own versioned wire is approved.

Historical artifacts remain byte-identical and are verified solely under their
declared historical profiles. A verifier MUST NOT synthesize A1 session facts,
SC1 certificates, or session proofs for them.

## 4.8 Heartbeat / dead-man switch

`heartbeat: {every: "24h", grace: "6h"}` makes the mandate valid only while the owner
publishes a signed liveness beacon (a tiny gamma entry, §07) at least every `every`.
If the owner goes dark beyond `every+grace`, all heartbeat-bound mandates auto-suspend
— agents stop acting without any explicit revocation. The owner's return (next beacon)
resumes them. This bounds autonomous action to owner presence, a core agentic safety.

**Head mandate — normative default (decided).** A *head mandate* — broad perimeter,
long validity, issued directly by the owner to operate unattended (absentee-owner
profile, §00.5) — SHOULD carry a long-period heartbeat, default
`heartbeat: {every: "30d", grace: "72h"}`, configurable. It is the only mandate in
the tree with no present ancestor — nobody is watching it — and expiry/heartbeat are
the only cuts that require nobody to show up. The dead-man bound turns a stolen or
rogue head key from an unbounded impersonation into at most ~one period (§10.8).
Beacons are owner-content-signed (§07.5) from an owner device: grantees never hold
owner keys, so a head agent can never beacon for itself. Suspension cuts every
delegated protocol consumption — mutation, connector action, grant, revoke, and
publication — while already-held decryption material remains subject to the honest
physics limit. The accompanying rotation is lazy hygiene (§06.8). Declining the
heartbeat — a true "issue and vanish" head mandate — is permitted but MUST be
treated as an assumed risk: revocation then waits on the owner's return or the
succession key (§01.1).

## 4.9 Storage and transport

Certificates live at `certs/<id>.json`, are world-readable, and MAY be transported
by any channel. The signature is what matters, not secrecy — but a certificate reveals
which agents a subject trusts, so treat its distribution as mildly sensitive.

## 4.10 Absolute active windows

> Decided 2026-07-10 (F+). Time in the verifier is interval arithmetic — no
> timezone, no DST, no calendar. "Every Thursday 14–18 Paris time" is the
> ISSUING TOOL's problem; it expands to arithmetic windows at grant time.

One window:

```jsonc
{ "anchor": "2026-07-02T14:00:00Z",   // RFC 3339 Z — start of occurrence 0
  "duration": "4h",                   // §07 duration grammar (<n>d|h|m|s)
  "period": "7d",                     // optional: repeats every period
  "until": "2026-09-01T00:00:00Z",    // optional: no occurrence STARTS after this
  "count": 8 }                        // optional: at most `count` occurrences
```

Semantics (normative):

- Occurrence *k* (k ≥ 0) is the half-open interval
  `[anchor + k·period, anchor + k·period + duration)` — **start inclusive,
  end exclusive**. No `period` ⇒ only k = 0. `until` bounds occurrence
  *starts*; `count` bounds *k < count*; both may combine (conjunction).
- `T` satisfies a window iff it falls in some occurrence; several windows in
  `active_windows` compose as a **union**; the whole constraint conjoins with
  the validity window and every other constraint (rolling limits stay a
  distinct mechanism: relative sliding durations vs absolute slots).
- Wire format: instants are RFC 3339 Zulu strings, never epoch integers —
  RFC 8785 serializes numbers as IEEE 754 doubles, so a nanosecond epoch
  (> 2^53) would silently lose precision inside signed bytes. Arithmetic is
  exact integer seconds internally; finer granularity would be a `v` bump.
- **Attenuation (§05.3).** A sub-mandate's windows may only tighten: every
  occurrence of every child window, clipped to the child's validity window,
  MUST be contained in some occurrence of a parent window (parent without
  `active_windows` covers anything). Verification enumerates the child's
  occurrences — finite, since validity windows are — and fails **closed**
  above an implementation bound (an unverifiable containment is a rejection,
  never a pass).

## 4.11 Budget profiles

> Decided 2026-07-10 (F+). One mandate, several ways to pay: profiles
> compose with OR, everything inside a profile conjoins.

```jsonc
"budgets": [
  { "id": "haiku",
    "models": ["claude-haiku"],          // allow-list; absent = any model
    "token_budget": 10000,               // lifetime, subtree-counted (§07.4)
    "active_windows": [ … ],             // §4.10, scoped to this profile
    "max_actions": 1,                    // actions citing this profile
    "require_attestation": true,         // §4.11.1
    "attestation_key": "z6Mk…" },        // provider key the receipts must bear
  { "id": "gemma", "models": ["gemma"], "token_budget": 25000 } ]
```

- When a mandate carries `budgets`, every `action` and `inference` entry
  under it MUST cite `budget_ref: "<profile id>"` — an uncited entry, or one
  citing an unknown id, is invalid. The cited profile must be satisfied in
  full: model in list, `T` inside the profile's windows, action count and
  token tally not exhausted.
- **Tallies are subtree counts over gamma** (§07.4): actions citing the
  profile count against `max_actions`; declared `tokens` of actions plus
  `tokens_in + tokens_out` of `inference` entries citing the profile count
  against `token_budget`. The check applies **per budgets-bearing mandate in
  the chain** — a delegate never multiplies an ancestor's budget.
- **Enforcement tiers, stated plainly:** the verifier counts *declared*
  values (V). The truth of model and token numbers is the container's duty
  (X, §08): credentials live in the vault, the container builds the request,
  reads real usage, refuses at the budget. A-posteriori reconciliation
  (provider invoice vs log) closes the loop.

### 4.11.1 Attestation receipts

The optional bridge from X back to V is a provider-signed usage receipt. The
historical v1 action shape is:

```jsonc
"receipt": { "args_hash": "sha256:…",   // MUST equal the entry's args_hash
             "model": "claude-haiku",   // MUST equal the entry's model
             "tokens": 8412,            // real usage; OVERRIDES the declaration
             "sig": "<hex ed25519 over JCS of the three fields above>" }
```

Under the historical v1 profile, `require_attestation: true` rejects a citing entry
without a receipt that verifies under the profile's `attestation_key`.

For the historical v1 shape, `args_hash` prevents substitution of different
arguments; it does not identify an occurrence and therefore does not make the
receipt single-use. The same signed usage statement can verify more than one v1
entry carrying the same hash, model, and usage facts, subject to every other
applicable rule. Historical verification preserves that limitation.

U1 defines two closed usage-receipt v2 families: one for an `action` occurrence and
one for an `inference` occurrence. The applicable family is determined by the
independently reconstructed operation; a receipt cannot relabel one family as the
other. Both follow the common `v`, `operation_ref`, `sig`, signature, closure, and
downgrade rules of §4.12.1.

Actual usage is learned after the effect. The action family binds its post-effect
usage to the already-fixed action occurrence; the inference family binds its
post-effect `tokens_in` and `tokens_out` to the already-fixed inference occurrence.
Neither family changes or extends the pre-effect operation commitment.

The inference family is not the historical action receipt with renamed counters. A
verifier MUST NOT synthesize a missing native `args_hash` from a prompt, response,
receipt, or other artifact merely to make an inference fit the action family.

Under U1, `require_attestation: true` rejects a final accepted evidence set without
the applicable valid usage receipt. Where that receipt is valid, its actual usage
overrides the corresponding declaration for tallying. The exact remaining members
of the action and inference families are reserved for the closed receipt member
table; no implementation may infer them from v1.

Usage receipts meter post-effect usage. Pre-effect gating receipts — guardrail,
approval, and `counter_sign` — are obligations (§4.12).

## 4.12 Obligations (the general gate)

> Decided 2026-07-10. `counter_sign` (§4.6) and the token receipt (§4.11.1) were
> two instances of one shape: a **signed statement, bound to a specific action,
> checked at gamma-append, recorded in the log**. §4.12 names that primitive so
> guardrails, human approval and dual control all reuse it — one mechanism, N
> enforcement types, all provable from files alone.

An **obligation** attaches a discharge requirement to a permit: an in-scope action
may *consume* (append its `action` entry) only if it carries a valid **receipt**
from a pinned attestor whose verdict satisfies the predicate.

The signed v1 shape below is historical and targets connector actions only. Its
existing `action` and `args_hash` fields MUST NOT be reinterpreted as an occurrence,
an operation commitment, or a receipt-v2 discriminator.

Under mandate `draft.2`, obligations still have only the existing connector-action
matcher. W1 supplies a common operation identity and R2 binds a receipt to that
identity; neither defines how an obligation declaration matches a mutation, grant,
revoke, vault-config operation, publication, or any other non-action operation.

A closed non-action matcher is reserved for mandate `draft.3`. A `draft.2`
declaration cannot become applicable to a non-action operation merely because the
candidate carries an `operation_ref` or a receipt with `v: 2`. A later `draft.3`
matcher MUST NOT reinterpret existing `draft.1` or `draft.2` mandate bytes.

```jsonc
"obligations": [
  { "id": "publish-approval",
    "check": "human.approve",             // opaque check id; the LOGIC lives in the attestor
    "attestor": ["z6MkApprover…"],        // pinned key(s); a valid receipt from ANY satisfies
    "applies_to": "act.x.social.publish", // action pattern (perimeter grammar, §4.2)
    "verdict": "approve",                 // required value
    "max_age": "5m" } ]                   // optional receipt freshness vs entry.at
```

The **receipt** rides in the action entry (`checks: [...]`, §07.4):

```jsonc
"checks": [
  { "obligation": "publish-approval",
    "args_hash": "sha256:…",              // MUST equal the entry's args_hash
    "verdict": "approve",
    "presented_digest": "sha256:…",       // optional: hash of what was shown (WYSIWYS)
    "at": "2026-07-10T14:02:11Z",
    "sig": "<ed25519 over JCS of {obligation, mandate_id, action, args_hash, verdict, presented_digest?, at}>" } ]
```

**Historical v1 verifier rule (tier V, offline).** For every v1 `action` entry, for
every obligation in the chain whose `applies_to` covers the entry's action, the
entry MUST carry a `checks[]` receipt with matching `obligation`, `args_hash` equal
to the entry's, `verdict` satisfying the predicate, `sig` verifying under a pinned
`attestor`, and — if `max_age` is set —
`|entry.at − receipt.at| ≤ max_age`. Any failure invalidates the entry.

The signed payload
`{obligation, mandate_id, action, args_hash, verdict, presented_digest?, at}`
binds the leaf mandate, action, arguments, verdict, presentation, and freshness. It
blocks cross-mandate, cross-action, altered-argument, and stale transfer, but it does
not block an exact replay within the freshness window because v1 has no occurrence
identity. `mandate_id` remains the entry's `authorized_by`, so the historical
receipt never transfers between sibling sub-mandates. When present,
`presented_digest` remains inside the signed set.

### 4.12.1 Closed operation-bound receipts v2 (R2 and U1)

> **R2/U1 protocol decision — human-validated on 2026-07-18.**

Every receipt v2 is a closed object with the common top-level members `v`,
`operation_ref`, and `sig`, plus exactly the members of one selected closed receipt
family. `v` is the JSON number `2`. `operation_ref` is the exact W1 reference of the
independently reconstructed operation. `sig` is the receipt signature. A missing or
unknown version, malformed or mismatched reference, wrong family, or member outside
the selected closed family fails before signature trust.

For every receipt-v2 family, the Ed25519 signed message is exactly:

```text
RFC8785-JCS(receipt object with its top-level `sig` member omitted)
```

`sig` is omitted from the signed object, not replaced by an empty string or `null`.
The complete nested `operation_ref` and every selected-family member are inside the
signed JCS.

R2 defines the closed obligation-receipt v2 family. It is pre-effect gating evidence
from the pinned attestor and binds one obligation discharge to one exact operation
occurrence. Under mandate `draft.2`, R2 can discharge only an obligation selected by
the existing connector-action matcher. It does not define or imply a non-action
matcher.

U1 defines the two closed post-effect usage-receipt v2 families of §4.11.1:
`action` and `inference`. Their actual usage is signed against the already-fixed
`operation_ref` and never enters or changes the pre-effect operation commitment.

Receipt versions do not downgrade. Historical v1 receipts retain their exact bytes
and verify only under their historical carrier/profile. A receipt required for a W1
operation is v2: a v1-only receipt, absent `v`, `v != 2`, or unknown version fails
closed. A v2 receipt presented as historical v1 material also fails closed. No
verifier upgrades v1, synthesizes `operation_ref`, or treats a missing version as
v2.

Copying one receipt across the applicable native evidence views of the same
occurrence is correlation, not another consumption. Reusing it for a second
consuming occurrence, changing its `operation_ref`, or pairing it with facts that
reconstruct a different operation is replay or mismatch and fails closed.

The exact members beyond the approved common `v`, `operation_ref`, and `sig` remain
reserved for the R2 obligation family and both U1 usage families. Until their closed
member tables are human-validated and independently vectored, no producer may emit
a complete receipt v2 and no implementation may borrow, omit, or invent members
from the historical v1 action shape.

**The attestor holds the logic; the protocol holds a signature.** `check` is opaque
to the verifier — PII guardrail, policy engine, or a human tapping *approve* is
off-protocol. The protocol verifies *that a pinned key signed a bound verdict*,
never *why*. The core stays a signature checker, not a workflow engine.

**Instances (one primitive, three uses).**
- **Guardrail** — `attestor` = a gateway guardrail adapter's key, `verdict:
  "pass"`. The adapter calls the guardrail (Lakera/NeMo/…) and signs the verdict.
- **Human approval (Model 1)** — `attestor` = an approver's device-held key,
  `verdict: "approve"`; `presented_digest` binds WYSIWYS, `max_age` keeps it fresh.
  `counter_sign`/`binding` (§4.6) are this with attestor = owner.
- **Dual control** — `attestor` = a second agent's grantee key (four-eyes).

**Discharge order.** Authorize first (`covers_act`, pure, §4.5); then discharge —
run the check / obtain the signature (I/O, gateway-side, never the agent); then
append+consume with the receipt. A blocked or missing receipt consumes nothing:
the core **rejects the append** (fail-closed, a dedicated error) — the *refusal
log* is the gateway's operational duty, off-protocol (decided 2026-07-10).
Waiting on a human is a **pre-condition, not a deferred duty**: the log only
ever holds the committed action carrying its receipt, or nothing (§07 has no
"pending" state), so verification stays deterministic offline.

**Attenuation.** A sub-mandate may only ADD obligations, never drop a parent's
(§05.3): delegation can tighten a gate, never strip it.

*M-of-N (quorum of approvers) is reserved: the `attestor` set already carries the
keys; a future `quorum: k` on the obligation turns OR-across-set into k-of-n.*

## 4.13 Constraint applicability matrix (D7)

This matrix is normative for the pure verdict and deliberately does not assign wire
names to the two new meters. Operation columns mean: **R** = an authorized read
presentation, **M** = an Ethos or structural mutation, **A** = a connector action
(including an inference where the row says so), **Cfg-R** = reserved vault config
read, **Cfg-M** = reserved vault config create/edit/delete, both outside the
business action catalog under G-A, **G** = grant/sub-grant, **Rev** =
revocation/rotation authority, and **Pub** = edition publication/merge/resolution.
`Cfg-R` and `Cfg-M` are applicability columns only: the current mandate version has
one indivisible exact `.config` authority, not two separately grantable rights.

Symbols: **A** = applicable from public protocol facts; **P** = applicable and a
signed public proof/receipt is required for cold acceptance; **X** = the fact is
known to an executor, so keyless acceptance requires an approved public
attestation/receipt when validity depends on it; **B** = best-effort side effect,
never a validity condition; **F** = fail closed unconditionally under the current
wire; suffix **W** = semantic applicability is validated and fixed, but its public
encoding/proof is reserved for CB2 and cannot yet yield Allow; suffix
**?** = applies only under the condition stated below; **—** = non-applicable by
definition.

The cells describe a **grantee** consumption. Owner-local treatment is the separate
actor row below and never consumes these mandate constraints or counters.

| Constraint or fact family | R | M | A | Cfg-R | Cfg-M | G | Rev | Pub |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Form, subject, proof of possession, chain, perimeter, revocation | P | P | P | P | P | P | P | P |
| `not_before` / `not_after`, `active_windows` | A | A | A | A | A | A | A | A |
| `freshness`, `heartbeat` | P | P | P | P | P | P | P | P |
| `session_bind`, `max_sessions` | P-W | P-W | P-W | P-W | P-W | P-W | P-W | P-W |
| `first_party_only` | A | A | A | A | A | A | A | A |
| `purpose` | A | A | A | A | A | A | A | A |
| Explicitly applicable `obligations` (including a `co_sign` instance) | P-W* | P-W* | P* | P-W* | P-W* | P-W* | P-W* | P-W* |
| Canonical catalog `binding` class → exact right + `co_sign` | — | — | P | — | — | — | — | — |
| Mandate `counter_sign` / legacy `constraints.binding` shorthand | — | — | P | — | — | — | — | — |
| `max_actions`, `max_actions_per`, `rate_limit` | — | — | P | — | — | — | — | — |
| Explicit Ethos-mutation meter reserved by D7 | — | P-W | — | — | — | — | — | — |
| Explicit total delegated-consumption meter reserved by D7 | P-W† | P-W | P-W | P-W† | P-W | P-W | P-W | P-W# |
| `max_children` | — | — | — | — | — | P‡ | — | — |
| `budgets` profiles, model/token/action totals, usage attestation | — | — | P/X§ | — | — | — | — | — |
| `domains` | — | — | X? | — | — | — | — | — |
| `action_params` | — | — | X? | — | — | — | — | — |
| `spend_cap` | — | — | X? | — | — | — | — | — |
| `disclose_agency` | — | — | X? | — | — | — | — | — |
| `notify` | B? | B? | B? | B? | B? | B? | B? | B? |
| `log_reads` | P† | — | — | P-W† | — | — | — | — |
| Approved connector catalog version/class pin and wildcard rule | — | — | P | — | — | — | — | — |
| Unknown root-leaf extension under G-E | F¶ | F¶ | F¶ | F¶ | F¶ | F¶ | F¶ | F¶ |

`*` Under mandate `draft.2`, an obligation applies only through the existing signed
connector-action matcher. `operation_ref` correlates operation evidence and R2
receipts; it is not itself a matcher. Non-action applicability is reserved for a
closed mandate `draft.3` matcher and MUST NOT be inferred from current fields.
Existing connector-action and v1 receipt bytes remain unchanged.

`†` A private read that leaves no protocol artifact cannot be cold-replayed or
metered by physics. A Bundle Ethos or config read API still checks the chain at
operation time; a read becomes replayable/countable only when `log_reads` or an
explicit presentation produces signed evidence. The config-read evidence encoding
is WIP; config mutations are always journalized independently of `log_reads`.

`#` **CB1 counting decision — validated at the human protocol gate on
2026-07-18:** a grantee publication, merge, or resolution contributes exactly one
logical total-consumption unit for its publisher authority in addition to the
already-counted, semantically distinct contained operations. Counters count logical
protocol consumptions, not artifacts: Gamma evidence and an edition reference for
the same mutation count once, and an existing `kind:"merge"` entry plus its merge
publication envelope count once when they express the same publisher decision. A
resolution envelope and any canonical evidence for that same resolution likewise
count once; no distinct resolution kind is implied. A manifest/changeset proof and
any other canonical representation of the same logical publication authority are
evidence for that unit, never additional units. The public correlation
representation is `operation_ref` (§4.5.1). It correlates native evidence for one
logical publication occurrence without creating a Gamma kind, changing historical
tallies, or assigning names to the reserved meters.

`‡` `max_children` counts direct children of the exact minting mandate only, and is
checked before the grant becomes usable. It never counts all descendants.

`§` Budgets cover connector actions and inference entries exactly as §4.11 says,
never Ethos mutations. Public attestation is mandatory whenever the selected
profile requires it or a keyless verifier otherwise cannot establish a required
executor fact.

`?` `domains` applies only when an action addresses a domain/recipient;
`action_params` only to parameters for which the mandate declares predicates;
`spend_cap` only to paid external effects; `disclose_agency` only to outbound
communications; `notify` only to a listed event. Otherwise that family is
non-applicable. An applicable X fact needs approved public evidence for keyless
acceptance; notification delivery remains best effort and never changes validity.

`¶` Under the current G-E wire, Core has no understood applicability envelope and
therefore returns the typed “extension not understood” refusal for every
consumption. This cell can never become an implicit Allow. Only a later approved
version carrying a typed applicability envelope may establish non-applicability.
Existing mandate bytes are never reinterpreted by that future version.

Actor and time-mode rules apply to every row:

| Actor | Append-time | Cold-time |
|---|---|---|
| Owner | Local narrow capability; operation is authorized without a mandate, journalized, and consumes no mandate counter or constraint. | Verify owner signature, canonical operation, Gamma, changeset, and state transition; never synthesize a mandate consumption. |
| Grantee | One local actor capability plus exactly one valid chain; evaluate every applicable row before canonical effect and commit its required evidence. | Replay the same pure operation and rows from public artifacts against the historical prefix; missing executor proof or private fact required for validity fails closed. |

Publication does not launder contained operations: its one actor/chain is checked
both for publication authority and for every derived change (§02.6.1). Append-time
and cold-time MUST call the same pure rule with an injected time and historical
prefix; a helper that exposes only a successful partial check is not an
authorization API.

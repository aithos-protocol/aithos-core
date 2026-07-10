# 4 — Mandates

> **Status: DRAFT.** The certificate plane. A mandate authorizes a grantee keypair to
> read/author a perimeter and take agentic actions, under constraints, for a window.
> This chapter defines the document, the full agentic constraint vocabulary, and the
> offline verifier.

## 4.1 Document

```jsonc
{ "aithos-mandate-core": "1.0.0-draft.1",
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

Verb lattice (normative): `read ⊑ edit ⊑ append ⊑ write`, `delete ⊑ write`; every
mutation verb implies `read` on its perimeter. `append` = create + edit within
perimeter; `write` = full CRUD. Multiple entries union per verb. A selector matching
nothing yet is a valid forward-looking grant. Enforceability of a write perimeter:
`id=`/`dir=` are clear placements on `public`/`circle` (hard); `tag=` writes are hard
on `public`/`circle` (clear tags, §07 authorship cross-check). On `self`, structure
and tags are sealed (§02.8): `dir=` and `tag=` perimeters there are **read-only**;
`self` writes use `id=` or zone-level grants.

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

`constraints` is an object; unknown keys MUST NOT cause rejection but MAY cause a
tool host to refuse an action it cannot enforce. Each key states its **enforcement
tier**: **V** verifier (offline, from files) or **X** executor/tool-host (runtime).
(Counter-signature, once its own tier **C**, is now the owner instance of an
obligation — tier V, §4.12.)

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
| `binding: [actions]` | actions that constitute a commitment; implies counter_sign | V |
| `domains: [patterns]` | connector actions may touch only these domains/recipients | X |
| `action_params: {action: predicates}` | per-action argument predicates (allow-listed recipients, subject patterns, no-attachments, numeric caps) — generalizes `domains`. Enforced on the real args by the container (X); **auditable at V** through the sealed args body of the entry (§07.9): the owner reopens the args and re-evaluates the predicates | X (+V audit) |
| `disclose_agency: true` | the agent MUST identify itself as an agent in every outbound communication of a connector action (transparency; EU-AI-Act-aligned) | X |
| `notify: [events]` | out-of-band owner alert on the listed events; best effort, never a validity condition | X |
| `purpose: "<text>" ` | signed statement of intent; actions cite it; audited | V+X |
| `session_bind: <pubkey>` | actions valid only from this ephemeral session key (§4.7) | V |
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
8. Proof of possession: the presented request/entry is signed by leaf.grantee.pubkey.
9. Tier X constraints are handed to the executor (real-arg predicates, model
   truth). Obligations — including binding/counter_sign co-signatures — are
   **tier V**, already enforced at step 7 from the signed receipts in the log.
```

Fail any ⇒ reject. Every step reads files (DID doc, certs, gamma — revocation state
included, §06.5); none needs a live server.

## 4.6 Counter-signature (binding actions) — the owner-approval obligation

`counter_sign`/`binding` are the **owner instance of an obligation** (§4.12):
listed actions may consume only if accompanied by a `co_sign` — a receipt whose
attestor is the owner content key and whose verdict is *approve*. The agent
prepares the action, obtains the owner's live co-signature (out of band — the
human approves), then emits it with the gamma entry. This is how "the AI may
act, but a commitment needs me in the loop" is expressed.

The `co_sign` receipt signs `{mandate_id, action, args_hash, at}` — the general
obligation payload (§4.12) specialized to owner-approve (its `obligation`/`verdict`
implicit), so every anti-replay property carries over identically: one-shot
(nonce-bound), logged, and fresh-bound — valid only if
`|entry.at − co_sign.at| ≤ Δ_cosign` (normative default **5 minutes**), so a
stored "fresh" co-signature cannot be replayed later. `binding` additionally
marks the action as a commitment (implies `counter_sign`). Enforcement is
**tier V** (§4.5 step 7): a signed file artifact verified offline like any
obligation, not handed to a runtime executor.

## 4.7 Session binding

`session_bind: <pubkey>` ties the mandate to an ephemeral **session key** the grantee
generates per run. Every action entry must also be signed by the session key, and the
session key is itself certified by the grantee's long-term key for a short window.
Effect: exfiltrating the mandate file without the live session key yields nothing —
useful for agents whose mandate persists but whose runtime is ephemeral.

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
owner keys, so a head agent can never beacon for itself. Suspension cuts *action*
only; the accompanying rotation is lazy hygiene (§06.8). Declining the heartbeat — a
true "issue and vanish" head mandate — is permitted but MUST be treated as an
assumed risk: revocation then waits on the owner's return or the succession key
(§01.1).

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

The optional bridge from X back to V: a provider-signed usage receipt.

```jsonc
"receipt": { "args_hash": "sha256:…",   // MUST equal the entry's args_hash
             "model": "claude-haiku",   // MUST equal the entry's model
             "tokens": 8412,            // real usage; OVERRIDES the declaration
             "sig": "<hex ed25519 over JCS of the three fields above>" }
```

A profile with `require_attestation: true` rejects any citing entry without
a receipt that verifies under the profile's `attestation_key`. The
`args_hash` binding makes receipts single-use: a receipt never transfers to
another action. Where receipts exist, tallies use the attested `tokens`,
not the declaration. (This receipt *meters*; the *gating* receipts that share
its crypto skeleton — guardrail, approval, counter_sign — are obligations, §4.12.)

## 4.12 Obligations (the general gate)

> Decided 2026-07-10. `counter_sign` (§4.6) and the token receipt (§4.11.1) were
> two instances of one shape: a **signed statement, bound to a specific action,
> checked at gamma-append, recorded in the log**. §4.12 names that primitive so
> guardrails, human approval and dual control all reuse it — one mechanism, N
> enforcement types, all provable from files alone.

An **obligation** attaches a discharge requirement to a permit: an in-scope action
may *consume* (append its `action` entry) only if it carries a valid **receipt**
from a pinned attestor whose verdict satisfies the predicate.

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

**Verifier rule (tier V, offline).** For every `action` entry, for every obligation
in the chain whose `applies_to` covers the entry's action, the entry MUST carry a
`checks[]` receipt with matching `obligation`, `args_hash` equal to the entry's,
`verdict` satisfying the predicate, `sig` verifying under a pinned `attestor`, and
— if `max_age` is set — `|entry.at − receipt.at| ≤ max_age`. Any failure invalidates
the entry. The signed payload `{obligation, mandate_id, action, args_hash, verdict,
presented_digest?, at}` is a **superset** of the §4.6 `co_sign` payload: binding
`args_hash` makes the receipt single-use, binding `mandate_id`+`action` blocks
cross-mandate and cross-action replay. When present, `presented_digest` sits inside
the signed set, so the rendered-vs-executed (WYSIWYS) binding cannot be altered
after the fact; comparing it to a re-render is an off-protocol audit step.

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
append+consume with the receipt. A blocked or missing receipt consumes nothing and
is logged as a refusal. Waiting on a human is a **pre-condition, not a deferred
duty**: the log only ever holds the committed action carrying its receipt, or
nothing (§07 has no "pending" state), so verification stays deterministic offline.

**Attenuation.** A sub-mandate may only ADD obligations, never drop a parent's
(§05.3): delegation can tighten a gate, never strip it.

*M-of-N (quorum of approvers) is reserved: the `attestor` set already carries the
keys; a future `quorum: k` on the obligation turns OR-across-set into k-of-n.*

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
            | kind=<kind> | since=<ts> | until=<ts>
```

At most one `dir`, one `tag`, one `id` per entry. Semantics: `dir=` alone = the whole
subtree; `tag=` alone = the zone-root tag view; `dir=…&tag=…` = the folder-local tag
view (§02.9) — read what carries the tag under that folder, now and in the future;
`id=` = one section (sids are global, so `id` composes with nothing). No selector =
the whole zone. `covers()` is per-dimension: `dir` containment is by **segment list**
(`dir=a/b` covers `a/b` and `a/b/c`, never `a/bc`); an absent dimension covers any
value of it (no-`tag` covers any `tag`); `tag` covers only the equal `tag`.
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
their (sealed) targets, `kind=` covers only the equal kind, `since`/`until` bound
the entries' `at`. Enforcement is split honestly: `dir`/`id`/`tag` are **physics**
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
tier**: **V** verifier (offline, from files), **X** executor/tool-host (runtime), **C**
counter-signature.

| Key | Meaning | Tier |
|---|---|---|
| `not_before` / `not_after` | validity window (top-level, but listed for completeness) | V |
| `max_actions: N` | at most N gamma action entries carry this mandate in `authorized_via` over its life — a **subtree** count, see below | V (count §07) |
| `max_children: N` | at most N direct sub-mandates ever issued under this mandate (bounds delegation *width*; `depth` bounds only length, §05.7) | V (count §07) |
| `max_sessions: N` | at most N session keys simultaneously certified by the grantee's long-term key (§4.7) — blocks silent duplication of one mandate across N machines | V |
| `max_actions_per: {window,N}` | ≤ N actions per rolling window (`"1h"`,`"1d"`) | V+X |
| `rate_limit: {action,window,N}` | per-action-kind rate cap | V+X |
| `active_hours: {tz, ranges[]}` | actions only within given local time ranges | X |
| `counter_sign: [actions]` | listed actions require a fresh owner co-signature (§4.6) | C |
| `binding: [actions]` | actions that constitute a commitment; implies counter_sign | C |
| `domains: [patterns]` | connector actions may touch only these domains/recipients | X |
| `action_params: {action: predicates}` | per-action argument predicates (e.g. reply only on an existing thread, no attachments, recipient cap) — generalizes `domains` | X |
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
7. Constraints tier V all pass (counts via gamma §07, session_bind, heartbeat, …).
8. Proof of possession: the presented request/entry is signed by leaf.grantee.pubkey.
9. Tier X/C constraints are handed to the executor; a binding/counter_sign action
   without a valid owner co-signature (§4.6) is rejected here.
```

Fail any ⇒ reject. Every step reads files (DID doc, certs, gamma — revocation state
included, §06.5); none needs a live server.

## 4.6 Counter-signature (binding actions)

An action listed in `counter_sign`/`binding` is valid only if accompanied by a
`co_sign`: the owner content key signing `{mandate_id, action, args_hash, at}`. The
agent prepares the action, obtains the owner's live co-signature (out of band — the
human approves), then emits it with the gamma entry. This is how "the AI may act, but
a commitment needs me in the loop" is expressed. Counter-signatures are one-shot
(nonce-bound) and logged. They are also fresh-bound: a `co_sign` is valid only if
`|entry.at − co_sign.at| ≤ Δ_cosign` (normative default **5 minutes**) — a stored
"fresh" co-signature cannot be replayed later.

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

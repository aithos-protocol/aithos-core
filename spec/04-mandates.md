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
  "issued_by": "did:aithos:z6Mkr…#self",     // sphere DID URL (root) OR grantee pubkey (sub)
  "grantee": { "id": "urn:aithos:agent:gmail@laptop",
               "label": "Gmail agent",
               "pubkey": "z6MkGrantee…" },    // Ed25519 — REQUIRED
  "perimeter": [ "read.circle#tag=test", "edit.circle#tag=test",
                 "read.public", "read.self#id=sec_a",
                 "act.x.gmail.reply", "issue#depth=1" ],
  "constraints": { … },                       // §4.4
  "not_before": "2026-07-08T00:00:00Z",
  "not_after":  "2026-07-15T00:00:00Z",
  "issued_at":  "2026-07-08T09:00:00Z",
  "nonce": "rNlx4L9k3qBp",
  "signature": { "alg": "ed25519", "key": "…", "value": "…" } }
```

The grantee's content keys are **not** in the mandate — they live in headers (§03).
The mandate is a pure certificate: publishable, verifiable, revocable. (A convenience
delivery bundle MAY ship the mandate together with the header lines the grantee needs,
but the lines are authoritative in the bundle, not in the certificate.)

## 4.2 Perimeter grammar

```
perimeter-entry :=
    <verb> "." <zone> [ "#" <selector> ]        ethos
  | "act." "x." <connector> "." <action>        connector action
  | "issue" [ "#depth=" <n> ]                   delegation right (§05)

verb     := read | edit | append | delete | write
zone     := public | circle | self
selector := ns=<ns> | id=<section_id> | tag=<tag>
```

Verb lattice (normative): `read ⊑ edit ⊑ append ⊑ write`, `delete ⊑ write`; every
mutation verb implies `read` on its perimeter. `append` = create + edit within
perimeter; `write` = full CRUD. Multiple entries union per verb. A selector matching
nothing yet is a valid forward-looking grant. Enforceability of a write perimeter:
`id=`/`ns=` are clear in every zone (hard); `tag=` writes are hard on `public`/`circle`
(clear tags, §07 authorship cross-check) and **read-only** on `self` (sealed tags).

## 4.3 What each perimeter entry needs at the key layer

| Entry | Header line the grantee needs |
|---|---|
| `read.public` | none (plaintext) |
| `*.circle` / `*.self` (no selector) | line on `/e/<zone>` |
| `*#ns=<ns>` | line on `/e/<zone>/ns/<ns>` |
| `*#tag=<t>` | line on `/e/<zone>/t/<t>` (+ tag wraps bridge sections, §03/§07) |
| `*#id=<x>` | line on `/e/<zone>/s/<x>` |
| `act.x.<c>.<a>` | none (action authorization only; config needs vault line, §08) |
| `issue` | none |

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
| `max_actions: N` | at most N gamma action entries name this mandate over its life | V (count §07) |
| `max_actions_per: {window,N}` | ≤ N actions per rolling window (`"1h"`,`"1d"`) | V+X |
| `rate_limit: {action,window,N}` | per-action-kind rate cap | V+X |
| `active_hours: {tz, ranges[]}` | actions only within given local time ranges | X |
| `counter_sign: [actions]` | listed actions require a fresh owner co-signature (§4.6) | C |
| `binding: [actions]` | actions that constitute a commitment; implies counter_sign | C |
| `domains: [patterns]` | connector actions may touch only these domains/recipients | X |
| `purpose: "<text>" ` | signed statement of intent; actions cite it; audited | V+X |
| `session_bind: <pubkey>` | actions valid only from this ephemeral session key (§4.7) | V |
| `heartbeat: {every, grace}` | mandate valid only if owner liveness beacon < every+grace old (§4.8) | V |
| `freshness: <duration>` | verifier MUST have revocation state newer than this (§06.5) | V |
| `spend_cap: {unit, amount}` | cumulative external spend ceiling (audited; for future paid actions) | X |
| `first_party_only: true` | grantee id MUST be under the subject's control (self-agent) | V |

These compose; the effective constraint is the **conjunction**. Sub-mandates may only
tighten (§05.3).

## 4.5 Verifier algorithm (offline)

To verify grantee G may do `OP` on subject `DID` at time `T` from a presented chain:

```
1. Resolve DID document; verify root signature; get sphere keys + revocation pointer.
2. For the root mandate: issued_by is a sphere URL; verify signature; subject==DID.
3. not_before ≤ T ≤ not_after for every mandate in the chain.
4. Revocation: none of the chain's ids is revoked at T (§06.5); freshness honored.
5. Chain attenuation (§05.3) holds link by link.
6. OP ∈ effective perimeter of the leaf (verb lattice + selector match).
7. Constraints tier V all pass (counts via gamma §07, session_bind, heartbeat, …).
8. Proof of possession: the presented request/entry is signed by leaf.grantee.pubkey.
9. Tier X/C constraints are handed to the executor; a binding/counter_sign action
   without a valid owner co-signature (§4.6) is rejected here.
```

Fail any ⇒ reject. Every step reads files (DID doc, certs, gamma, revocation list);
none needs a live server.

## 4.6 Counter-signature (binding actions)

An action listed in `counter_sign`/`binding` is valid only if accompanied by a
`co_sign`: the owner sphere key signing `{mandate_id, action, args_hash, at}`. The
agent prepares the action, obtains the owner's live co-signature (out of band — the
human approves), then emits it with the gamma entry. This is how "the AI may act, but
a commitment needs me in the loop" is expressed. Counter-signatures are one-shot
(nonce-bound) and logged.

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

## 4.9 Storage and transport

Certificates live at `certs/<id>.json`, are world-readable, and MAY be transported
by any channel. The signature is what matters, not secrecy — but a certificate reveals
which agents a subject trusts, so treat its distribution as mildly sensitive.

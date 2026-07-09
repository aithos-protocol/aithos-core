# 6 — Revocation

> **Status: DRAFT.** The ladder in full: what each rung cuts, what it costs, and how
> it works with no server in any trust role.

## 6.1 The ladder

| Rung | Act | Cuts the target from | Cost | Others |
|---|---|---|---|---|
| 0 Expiry | nothing; `not_after` in the cert | future, at expiry | 0 | — |
| 1 Cert revocation | signed revocation entry (§6.4) | future, at `revoked_at`, at any honoring verifier/host | 1 tiny doc | none touched |
| 2 Rotation | new DK, header without his line | future content cryptographically | 1 header | none touched (I2) |
| 3 Re-encryption | rewrite bodies under new DK | existing content cryptographically | one pass over the node's bytes | none touched |
| 4 Supersession + GC | drop old editions/versions | the past, except what he exfiltrated | storage churn | none touched |

Rungs compose: a hard incident revocation is 1+2+3(+4). A soft "no longer needed" is
often just 0 or 1. Rung 3's cost is proportional to the **reach of the revoked key**,
not to the number of keys in the system (§06.3).

Actor: rungs 1–3 are executed by the **revoker itself** — owner or authorized
ancestor — in one atomic edition (§6.2). It is present (it signs the entry) and it is
a holder (attenuation, §05.5): no standing actor, no key reserve, no third party is
ever involved.

## 6.2 Procedure (owner or authorized ancestor)

```
revoke(mandate M, mode) — by the owner or an authorized ancestor, in ONE edition:
  1. Write the revocation entry for M.id (§6.4); anchor it in the gamma (§6.5).
  2. if mode ≥ rotate:
       for each node N in M's perimeter that the revoker has authority over:
         DK' ← random; version++
         header[N].new = { lines: reseal DK' to all survivors + owner }   # not M
         post the derivation up-link wrap for N (§03.4 step 2bis)
         if mode ≥ reencrypt: rewrite N's blobs under keys derived from DK'
  3. Cascade: mark M's descendants revoked (their chains break anyway).
  4. Publish one edition (height+1) carrying the entry, headers, blobs, gamma head.
```

Survivors do nothing; their next read opens the new lines (or the up-link wrap, for
derivation readers). The revoked, absent from the new version and unable to derive
DK', is out of all content not yet in his hands. The revoker is present (it signs)
and a holder (attenuation): no reserve, no custodian, no staged keys.

## 6.3 Cost worked (the "high branch" case)

Revoke a mandate sitting just under the owner, holding a zone with 1000 sections and 8
sub-grants below it: **one** re-encryption pass over the zone's bytes (XChaCha20 at
GB/s — seconds of CPU, parallelizable; upload dominates), plus reissuing the 9 headers
of the affected nodes (microseconds, hundreds of bytes). Never one pass per key. If the
8 sub-grants were issued *by* the revoked mandate, they cascade; the owner re-adopts
the ones to keep (one line each). Revoking a narrow leaf instead re-encrypts only its
small reach — least privilege buys cheap revocation.

## 6.4 Revocation entry

```jsonc
{ "aithos-revocation-core": "1.0.0-draft.1",
  "mandate_id": "mandate_01JZ…",
  "subject": "did:aithos:z6Mkr…",
  "revoked_by": "did:aithos:z6Mkr…#self",   // sphere URL, or an ancestor grantee pubkey
  "revoked_at": "2026-07-10T12:00:00Z",
  "reason": "device_lost",
  "signature": { … } }
```

Forward-only: artifacts dated `< revoked_at` remain attributable; `≥ revoked_at` are
invalid even if well-formed. Authority: `revoked_by` MUST be the mandate's issuer or a
transitive ancestor (I4), checkable from certs.

## 6.5 Revocation state without a server

Revocation state is **not** an owner-signed aggregate — that would be a hidden
availability dependency on the owner, incompatible with the absentee-owner profile
(§00.5). Instead:

- Each revocation entry is individually verifiable from its own signature plus the
  signer's certificate chain (I4): anyone can check that `revoked_by` is the target's
  issuer or a transitive ancestor. No owner-signed artifact is required.
- Anti-rollback comes from the gamma chain: revocations are `revoke` gamma entries
  (§07), hash-chained and pinned by each edition's `gamma_head`. Withholding one
  means withholding the log tip — detectable equivocation (§10.6), never silent.
- A verifier reconstructs the active revocation set from the gamma and the
  certificates alone. A convenience `revocations.json` index MAY be published, but it
  is derived data, never authoritative.

Offline verifiers use the freshest state they hold; the mandate's `freshness`
constraint (§04.4) sets how stale is tolerable. Short TTLs remain the robust answer
for fully-offline verification — a certificate that must be checked against live
revocation should be short-lived by design. A mirror MAY additionally refuse revoked
chains at fetch time (defense in depth), but the cryptographic cut is rungs 2–3, not
the mirror.

## 6.6 Interaction with immutable credentials (I2)

No rung ever edits a grantee's keypair or certificate. "Revoking a key" always means:
stop sealing the current DK to it (rotation) and/or move the content beyond its reach
(re-encryption) and/or record its cert as revoked. The grantee keeps its now-useless
keypair; the world simply stops being openable by it.

## 6.7 Watchdog: the action cut needs no key

A mandate MAY carry a `revoke` perimeter entry (§04.2) while holding **no content key
at all** (no header line anywhere). Its bearer — a daemon, a Lambda, a phone app —
can publish revocation entries for any mandate whose perimeter its `revoke` scope
covers (attenuation applies: it can only be granted `revoke` over what its issuer
could itself revoke), cutting the revoked party's *actions* instantly at every
honoring verifier. It can neither read a byte nor rotate a lock. Rotation — the
future-read cut — is then executed by a manager-holder on notification, or as lazy
hygiene (§6.8). Compromising the watchdog exposes no content; the worst abuse is a
revocation DoS, bounded to its perimeter, attributable (signed), and repaired at one
line per victim (re-grant, §03.3).

## 6.8 Expiry and heartbeat suspension: the read lingers

`not_after` and heartbeat suspension (§04.8) cut the *action* with no intervention —
every verifier rejects — but they turn no lock: the expired party's line remains in
the current key version and the DK is unchanged, so it could still **read** content
written under that DK until the node is next rotated. These cases being non-urgent by
nature (no incident), the accompanying rotation is **lazy hygiene**, executable by any
manager of the node in passing (recursive maintenance, §00.5) — a rule of upkeep, not
an emergency mechanism. Incident-class cuts use the full ladder (§6.2) instead.

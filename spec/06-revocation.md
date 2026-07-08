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

## 6.2 Procedure (owner or authorized ancestor)

```
revoke(mandate M, mode):
  1. Write revocation entry for M.id (§6.4); publish to the revocation list + gamma.
  2. if mode ≥ rotate:
       for each node N in M's perimeter that M's issuer has authority over:
         DK' ← random; version++
         header[N].new = { lines: reseal DK' to all survivors + owner }   # not M
         if mode ≥ reencrypt: rewrite N's blobs under keys derived from DK'
       cascade: mark M's descendants revoked (their chains break anyway).
  3. Publish one edition (height+1) carrying the new headers/blobs and gamma head.
```

Survivors do nothing; their next read opens the new lines. The revoked, absent from
the new version and unable to derive DK', is out of all content not yet in his hands.

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

The subject maintains a signed **revocation list** (all active revocations, owner-root
signed, monotonic `seq` + `issued_at`), published wherever the bundle lives and pinned
by the DID document. Offline verifiers use the freshest list they hold; the mandate's
`freshness` constraint (§04.4) sets how stale is tolerable. Short TTLs remain the
robust answer for fully-offline verification — a certificate that must be checked
against live revocation should be short-lived by design. A mirror MAY additionally
refuse revoked chains at fetch time (defense in depth), but the cryptographic cut is
rungs 2–3, not the mirror.

## 6.6 Interaction with immutable credentials (I2)

No rung ever edits a grantee's keypair or certificate. "Revoking a key" always means:
stop sealing the current DK to it (rotation) and/or move the content beyond its reach
(re-encryption) and/or record its cert as revoked. The grantee keeps its now-useless
keypair; the world simply stops being openable by it.

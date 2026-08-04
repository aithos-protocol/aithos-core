# 9 — CLI and conformance

> **Status: DRAFT.** The reference surface, the vectors an implementation must pass,
> and the performance targets that make "performant" testable.

## 9.1 CLI surface (reference)

Everything is local; no command needs a network to be correct.

```
aithos-core init                         # generate S, DID doc, empty bundle
aithos-core device add|remove <label>

aithos-core folder add|rename|move <zone> <path>          # move = rotation, §02.9
aithos-core section add|edit|delete <zone> <path> [--title] [--tags] [--body-file]
aithos-core zone show <zone>                # display paths (names); CLI resolves sids

aithos-core grant <grantee> \            # mint mandate + append header lines
    --perimeter "read.circle#dir=projets/perso&tag=toto,edit.circle#tag=test,act.x.gmail.reply" \
    --ttl 7d [--max-actions 50] [--counter-sign act.x.gmail.send] \
    [--heartbeat 24h/6h] [--session-bind] [--domains example.com] \
    [--issue depth=1]
aithos-core delegate <parent-mandate> <grantee> --perimeter … --ttl …   # sub-mandate
aithos-core revoke <mandate> [--mode cert|rotate|reencrypt|purge] [--reason …]
aithos-core adopt <grantee> --perimeter …                # re-adopt after cascade

aithos-core action <connector> <action> --args-file … [--co-sign]   # emits gamma entry
aithos-core heartbeat                                    # owner liveness beacon

aithos-core verify <mandate|chain> [--at T]              # offline verifier (§04.5)
aithos-core log show|verify                              # gamma chain
aithos-core edition publish|verify                       # edition chain + fork rule
aithos-core prove <zone> <path>                          # inclusion proof (§02.10)
aithos-core edition diff <h1> <h2>                       # root-descent diff (§02.10)
```

## 9.2 Test vectors (normative at promotion)

`vectors/` MUST cover, from a fixed `S`: DID doc; the owner keys (root, content,
kex); a node DK and
a deep-path derivation (zone → folder → folder → section, one derive per segment); a
header seal/open for owner and a grantee; a tag wrap open; a mandate sign/verify; a
chain of depth 2 with attenuation;
a revocation rotation (old line absent, survivor line opens new DK); a gamma entry
sign/verify and a `max_actions` count; an edition prev_hash and a fork resolution.
Both success and every fail-closed case (unauthorized revocation, over-wide
sub-mandate, N+1 action, expired heartbeat) get a vector. I3 gets its own family
(§03.1): a header whose every key version carries the owner line → valid; a header
one of whose key versions carries no owner line at all → the edition is rejected; a
header whose line labelled `"owner"` is sealed to a key that is not the subject's
`owner_kex` → rejected; a header whose owner line is not labelled `"owner"` but is
sealed to `owner_kex` → valid, proving the label decides nothing in either
direction. Each case states which verifier tier it binds: keyless (edition
verification) or `owner_kex`-bearing. Session-2 additions MUST
also be covered: an up-link wrap open after rotation (and rejection of a wrap by a
non-holder of the parent); a disjoint-edition merge and a nearest-common-manager fork
resolution (and rejection of an out-of-authority resolver); a watchdog cert-only
revocation (no line, no read); a backdated artifact outside the freshness anchor →
rejected; a subtree `max_actions` count via `authorized_via` (child action consumes
parent budget); a `max_children` count via `grant` entries; an unauthorized tag
re-label not bridged by the repair pass; a `kex_pubkey` mismatch → invalid mandate.
Merkle vectors: a section inclusion proof verifying against the manifest roots; a
`dir=` subtree check against its folder's children root; a `self` proof revealing
sibling hashes only; a grant then a rotation each bumping the node's proof path; an
edition diff located by root descent; a leaf spliced as interior node → rejected
(domain separation); a proof against a stale root → rejected by freshness.
Signature-policy vectors (§02.11): an owner public signature with embedded
placement verifying detached; a placement-mismatched owner signature → rejected; a
circle signature verifying only after unsealing; a self blob carrying no content
signature, its authorship resolved via the gamma entry; a selective disclosure
round (reveal one section key → authorship + date proven for that section only).
Tree vectors: a folder grant opening its whole subtree by derivation; a folder-local
`dir&tag` view where the tagged section opens, the untagged sibling does not, and a
late-tagged section bridges in by wrap; `covers()` segment-boundary rejection
(`dir=a/b` vs `dir=a/bc`); a move-as-rotation cutting an old-parent holder while the
new parent derives through the up-link; a `self` round-trip proving index, headers
and gamma targets leak no names while an authorized reader reconstructs its subtree
from sealed descriptors.

## 9.3 Performance targets

Measured on a laptop, bundle on local disk:

| Operation | Target |
|---|---|
| `grant` (mint cert + N header lines) | < 20 ms |
| `delegate` (sub-mandate) | < 20 ms |
| add a reader to an existing node (1 line) | < 5 ms |
| `verify` a depth-2 chain (offline) | < 10 ms |
| revoke rung 2 (rotate headers, no re-encrypt) | < 50 ms |
| revoke rung 3 on a 1000-section zone (re-encrypt) | < 5 s CPU, parallelizable |
| read a section (open header → derive → decrypt) | < 2 ms |
| gamma append + chain verify (10k entries) | < 200 ms full verify |
| inclusion proof verify (1M sections) | < 1 ms |
| state-root update on one edit (1M sections) | < 1 ms |

## 9.4 Conformance levels

- **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
  verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed, and
  MUST reject an edition pinning a header that violates I3 (§03.1) — without holding
  any key, and on every `aithos-core` manifest profile.
- **Core issuer**: the above + mint/delegate/revoke + header rotation with the
  authority checks of §05.5. MUST refuse to sign an over-wide sub-mandate (pre-flight
  §05.3) and an unauthorized header rotation.
- **Agent host**: the above (reader) + action execution with tier-X/C constraint
  enforcement and mandatory gamma action entries (I5).

An implementation states which levels it claims; the vectors gate each.

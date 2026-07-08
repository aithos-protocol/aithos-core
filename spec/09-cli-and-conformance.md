# 9 — CLI and conformance

> **Status: DRAFT.** The reference surface, the vectors an implementation must pass,
> and the performance targets that make "performant" testable.

## 9.1 CLI surface (reference)

Everything is local; no command needs a network to be correct.

```
aithos-core init                         # generate S, DID doc, empty bundle
aithos-core device add|remove <label>

aithos-core section add|edit|delete <zone> <id> [--title] [--tags] [--body-file]
aithos-core zone show <zone>

aithos-core grant <grantee> \            # mint mandate + append header lines
    --perimeter "read.circle#tag=test,edit.circle#tag=test,act.x.gmail.reply" \
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
```

## 9.2 Test vectors (normative at promotion)

`vectors/` MUST cover, from a fixed `S`: DID doc; sphere/owner-kex keys; a node DK and
its section-key derivations (plain + namespaced); a header seal/open for owner and a
grantee; a tag wrap open; a mandate sign/verify; a chain of depth 2 with attenuation;
a revocation rotation (old line absent, survivor line opens new DK); a gamma entry
sign/verify and a `max_actions` count; an edition prev_hash and a fork resolution.
Both success and every fail-closed case (unauthorized revocation, over-wide
sub-mandate, N+1 action, expired heartbeat) get a vector.

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

## 9.4 Conformance levels

- **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
  verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed.
- **Core issuer**: the above + mint/delegate/revoke + header rotation with the
  authority checks of §05.5. MUST refuse to sign an over-wide sub-mandate (pre-flight
  §05.3) and an unauthorized header rotation.
- **Agent host**: the above (reader) + action execution with tier-X/C constraint
  enforcement and mandatory gamma action entries (I5).

An implementation states which levels it claims; the vectors gate each.

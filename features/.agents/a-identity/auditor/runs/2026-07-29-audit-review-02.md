# Identity audit — correction review round 2

## Run identity

| Field | Value |
|---|---|
| Date | 2026-07-29 |
| Run type | independent correction review |
| Role | `audit-a-identity` |
| Review unit | `AID-R2-PROVIDER` |
| Candidate | `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` |
| Worktree | clean at start |
| Assigned scope | `AID-001` Provider remainder only |
| Outside scope | `AID-002`, `AID-003`, `AID-004`, `AID-005` |

## Pass A — frozen history-blind verdict

**Frozen verdict: `ACCEPT` the AID-001 Provider remainder.**

This section was frozen before opening the correction diff, public audit,
prior auditor/corrector conclusions, or Git history.

### Inputs and contamination status

Inputs were limited to the candidate's current feature, Provider scenarios and
steps, P9 vectors, current production paths, Core DID verifier, storage seams,
the feature domain, and the binding protocol decision.

The primary review context accidentally displayed a few matching lines from
the public audit during an over-broad code search. Its provisional trace is
therefore treated as contaminated and is not the frozen proof. A fresh
independent review unit, given no history or prior conclusions, reproduced and
froze the verdict recorded here. That unit reported no substantive
contamination: its initial routing lookup exposed revision identifiers and
report paths only, not their contents or conclusions.

### Current-code trace

1. `service.rs` serializes `PUT did.json` under the per-tenant/DID deposit lock
   and dispatches exclusively to `artifacts::deposit_did`.
2. `deposit_did` validates UTF-8, exact JCS, closed `DidDocument`
   deserialization, `doc.id == <path DID>`, and the shared strict
   `DidDocument::verify` verdict before any persistence call.
3. `DidDocument::verify` requires the supported version and algorithm,
   `signature.key == "#root"`, the expected key codecs, DID/root binding, and
   a valid root signature. A succession-signed DID document is therefore
   rejected before storage.
4. A byte-different same-DID document that is correctly root-signed reaches
   `ObjectStore::put_once` and is refused as `immutable_conflict`.
5. `MemObjects` implements the comparison and first write under one mutex. The
   S3 implementation uses `If-None-Match: *`, then byte-compares after a 412.
   The filesystem backend uses `create_new(true)`. None rewrites the stored
   object on conflict.
6. The negative Cucumber assertions re-read storage: refused genesis leaves
   `did.json` absent; refused replacement leaves the original root-signed
   bytes unchanged.
7. No Provider source references `EpochTransition` or
   `verify_succession`. There is no parallel or partial Provider path that
   accepts a successor. Because no canonical triplet transport and cross-DID
   atomic storage contract exist, refusing byte-different same-DID replacement
   is the fail-closed behavior required by the binding decision.
8. The future full-triplet primitive remains
   `EpochTransition::verify_succession`, which verifies both documents, the
   declaration, the `next_did` binding, and distinct identities.

### Selected scenarios and assertions

The Provider runner excludes only `@wip`; all six current `@did` scenarios are
selected:

| Scenario | Frozen result |
|---|---|
| root-signed genesis | accepted |
| genesis document names another DID | `id_mismatch`, nothing stored |
| genesis envelope uses another signer | `signature_invalid`, nothing stored |
| root-signed genesis has unsupported semantics | `signature`, nothing stored |
| same-DID document signed by succession | `signature`, original unchanged |
| root-signed byte-different same-DID document | `immutable_conflict`, original unchanged |

### Independently reproduced Pass A commands

| Command | Result |
|---|---|
| `python3 vectors/verify-p9.py` | `P9 ok (58 checks, 32 cases)` |
| `cargo test -p aithos-provider --test cucumber -- --tags @did` | 6 scenarios passed, 47 steps passed |
| `cargo test -p aithos-provider --test vectors_replay p9_cases_replay_wire_exact_against_the_real_binary` | 1 passed |
| `cargo test -p aithos-provider --lib put_once` | 1 passed |
| `cargo test -p aithos-core --test a2_did` | 6 passed |

The primary review context independently reproduced the same Provider,
P9/Core, and partial-effect results from a Git archive of the exact candidate
in an isolated sibling layout. Its first direct worktree attempt was blocked
before compilation by a Cargo path-package collision; no behavioral test ran
in that failed attempt. The binary replay initially hit the sandbox's
loopback restriction, then passed unchanged with local-loopback permission.

### Pass A limits

- This accepts the Provider's fail-closed remainder, not the existence of a
  Provider identity-epoch transition feature.
- There is no integration test against live S3; the conditional non-rewrite is
  established by code trace.
- The binary replay proves wire responses. The Cucumber scenarios separately
  prove absence of partial writes against `MemObjects`.

## Pass B — historical and differential review

Pass B started only after the Pass A section above had been persisted. It
opened the protocol decision, domain state, public audit, round 1 review,
round 2 corrector report, Git history, and the exact immutable range:

```text
dfb79c87120caeb26737c81babd5cc2ad0dc0a3c..
e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1
```

The range changes 11 files with 342 insertions and 128 deletions. The only
production behavior change is in Provider DID deposit:

- the baseline's parallel succession verifier and ordinary overwrite are
  removed;
- the candidate composes strict Core `DidDocument::verify` with
  `ObjectStore::put_once`;
- `Stored` and byte-identical `Identical` are successful;
- byte-different `Conflict` is exposed as `immutable_conflict`.

The remaining production-file changes are documentation. The feature, Provider
Cucumber steps, P9 vectors, vector verifier, and publication documentation all
describe the same fail-closed contract.

### Differential detectability

The two new replacement scenarios detect the removed behavior rather than
merely restating candidate internals:

1. on the baseline, the succession-signed same-DID replacement reached the
   parallel verifier and returned success instead of the required signature
   refusal;
2. on the baseline, the root-signed byte-different same-DID document was
   checked as a succession replacement and returned a signature error instead
   of the required immutable conflict.

The corrector's baseline RED run records 151 selected Provider scenarios:
149 passed and those two failed. The candidate selects 152 scenarios and all
pass. Pass B also confirmed statically that the new expectations exercise the
branches removed by the candidate. The negative `Then` steps re-read the
object store, so a response-only implementation could not satisfy them while
leaving a partial write.

P9 independently establishes that the succession refusal fixture really is
signed by the succession key and that the conflicting replacement is a valid
root-signed DID document. The vector update therefore does not manufacture
the expected Provider errors with invalid inputs.

### Reproduced candidate gates

Tests were run from an archive of the exact candidate with the adjacent
`aithos-client` dependency archived at
`c6f615123ca3dc83708ba029b898375409551719`.

| Gate | Result |
|---|---|
| Provider Cucumber, full | 152 scenarios passed, 1,004 steps passed |
| Provider Cucumber, `@did` | 6 scenarios passed, 47 steps passed |
| Provider P9 binary replay | 1 passed |
| Provider `put_once` unit filter | 1 passed |
| Core A1 + A2 | 4 + 6 passed |
| Bundle surface tests | 2 passed |
| Bundle Cucumber, full | 836 scenarios passed, 3,568 steps passed |
| `python3 vectors/verify-p9.py` | `P9 ok (58 checks, 32 cases)` |
| `git diff --check` | passed |

A primary-context full-workspace replay exhausted temporary disk space while
linking Provider/Gateway test binaries and is environmentally inconclusive;
it did not report a behavioral test failure. The corrector records a green
full-workspace gate on the same candidate. `cargo fmt --all -- --check` still
reports only `rust/crates/aithos-gateway/src/core_bridge.rs`; that file has the
same blob (`774672a0e2d4db1e866d3eb1d85106e53f684f80`) at baseline and
candidate, so the formatting defect is outside this correction.

### Impact candidates for the next role

The accepted change may require targeted impact classification for:

- Provider `store-publication.feature` and the `deposit_did` call from
  `service.rs`;
- P9 consumers in Provider Cucumber, binary replay, and red replay;
- Provider read/publication documentation that described same-DID rotation;
- semantically adjacent identity-rotation features, even though no shared
  transition implementation changed.

`ObjectStore::put_once` itself is reused, not changed. No Provider production
path references `EpochTransition` or `verify_succession`.

## Reconciliation and conclusion

Pass B strengthens rather than overturns Pass A. The diff removes exactly the
parallel verifier and rewrite behavior that the current-code trace found
absent. New regressions are RED against the old behavior, GREEN against the
candidate, and assert the persistent effect. The protocol decision and public
documentation agree with the implementation.

**Final conclusion: `REVIEW_ACCEPTED`.**

| Finding | Disposition |
|---|---|
| `AID-001` Provider remainder | `VERIFIED` |
| `AID-002` | unchanged, `VERIFIED` within pilot scope |
| `AID-005` | unchanged, `VERIFIED` within pilot scope |
| `AID-003`, `AID-004` | unchanged, `OPEN` |

This conclusion does not claim that Provider implements epoch-transition
acceptance. It verifies that the invalid same-DID path is removed and that the
current surface refuses replacement without partial persistence until a
canonical triplet transport and atomic cross-DID contract are specified.

Lifecycle state advances to `IMPACT_REVIEW_REQUESTED`. The Gherkin impact
review is the next role; it is not part of this audit run.

# Global Gherkin impact review — `a-identity`

## Run identity

| Field | Value |
|---|---|
| Date | 2026-07-29 |
| Run type | cross-feature impact review |
| Role | `review-gherkin-impacts` |
| Review unit | `AID-R2-GLOBAL-IMPACTS` |
| Observed revision | `b48386a6898fa444873968c3f43dfdb860ce0a03` |
| Immutable baseline | `dfb79c87120caeb26737c81babd5cc2ad0dc0a3c` |
| Accepted candidate | `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` |
| Source feature | `features/a-identity.feature` |
| Source public audit | `docs/audits/features/a-identity.md` |
| Accepted review | `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-02.md` |
| Initial worktree state | clean |
| Scope | other features, runners, helpers, APIs, formats, vectors, and specification sections |
| Result | no `FULL_AUDIT`; targeted follow-ups only |

## Entry conditions

The entry conditions of `review-gherkin-impacts` are satisfied:

1. the domain and orchestrator states request an impact review;
2. the independent auditor conclusion is `REVIEW_ACCEPTED`;
3. `AID-001` Provider remainder is `VERIFIED`, not merely
   `IMPLEMENTED`;
4. the immutable revisions above both resolve to commits.

This is not a two-pass semantic audit. The impact-review skill explicitly
starts from the accepted audit, review reports, and diff, so no history-blind
Pass A exists or is claimed. The accepted review's frozen Pass A and
differential Pass B remain the behavioral proof. This report performs only
cross-surface dependency analysis and is intentionally history-aware.

## Accepted change

The exact range changes 11 files, with 342 insertions and 128 deletions.

The only production behavior change is
`aithos_provider::artifacts::deposit_did`:

- every incoming `did.json` now passes strict Core
  `DidDocument::verify`;
- persistence uses the existing atomic `ObjectStore::put_once`;
- a first deposit and byte-identical re-deposit succeed;
- a succession-signed same-DID document is refused with
  `artifact_invalid/signature`;
- a strict-Core-valid but byte-different same-DID document is refused with
  `artifact_invalid/immutable_conflict`.

The range does **not** change:

- `DidDocument`, `EpochTransition`, or
  `EpochTransition::verify_succession`;
- the `ObjectStore` or `PutOnce` API or any backend implementation;
- `RemoteStore`'s typed wire-error format;
- A1/A2 vectors or the top-level Identity step implementation;
- Gateway replication helpers.

Other changes align Provider scenarios, steps, P9 generation/replay, the
Provider specification, and audit documentation with that behavior.

## Search inventory

The repository contains 53 Gherkin files:

| Suite | Files |
|---|---:|
| Top-level Core/Bundle features | 18 |
| Gateway features | 24 |
| Provider features | 11 |

Searches covered:

- all `.feature` files and their runner locations;
- changed symbols `deposit_did`, `ArtifactReason::ImmutableConflict`,
  `ObjectStore::put_once`, and `PutOnce`;
- `did.json`, `#succession`, same-DID replacement, identity rotation,
  `EpochTransition`, and `verify_succession`;
- old and new P9 fixture/case names;
- P9 generators, independent verifier, binary replay, and red replay;
- Provider callers and wire-error mapping;
- Gateway `replicate_paths`, `replicate_owner_history`, and owner tooling;
- identity and rotation sections in `spec/01-identity-and-keys.md`,
  `spec/04-mandates.md`, and `spec/10-threat-model.md`.

The searches distinguish historical/archive wording from current executable
or normative dependencies. Files under `docs/archive/` and the explicitly
unverified topology research retain historical text but do not drive a
runner, format, or public contract.

## Gherkin classification

### `TARGETED`

| Feature | Evidence | Recommendation |
|---|---|---|
| `rust/crates/aithos-provider/tests/features/store/store-publication.feature` | This is the sole Gherkin feature that calls the changed `deposit_did` path. Its six `@did` scenarios cover root-signed genesis, three genesis refusals with no write, succession-signed replacement refusal, and root-signed immutable conflict with byte preservation. | Targeted surface, already fully exercised and accepted by the source review. Do not restart it unless new evidence contradicts the accepted results. |

### `NONE`

| Feature set | Count | Evidence |
|---|---:|---|
| Other top-level features: `b-derivation`, `c-headers`, `d-bundle`, `e-mandate-sections`, `e-mandates`, `f-gamma`, `f-plus-constraints`, `g-plus-obligations`, `g-revocation`, `h-merkle`, `h2-gamma-roots`, `i-concurrency`, `k-integration`, `l-delegated-writes`, `m-delegated-editions`, `n-structural-mutations`, `o-connector-classes-vault` | 17 | None calls Provider DID deposit or consumes P9. The semantically adjacent `f-gamma` identity-rotation row already requires distinct `previous_did`/`next_did` plus `transition_digest`; its Core `operation.rs` and CB2 vector path are unchanged. Other uses of “rotation” concern content, header, vault, revocation, or connector domains. |
| Gateway Gherkin features | 24 | No Gateway feature selects Provider same-DID replacement or P9. The enrollment feature's “Gateway identity rotates” concerns the Gateway process identity digest, not an Aithos DID epoch. No Gateway runner step depends on the removed Provider verifier. |
| Other Provider Gherkin features | 10 | They preload or read the unchanged root-signed P1 `did.json`, or exercise unrelated relay, remote-read, ACME, control, cold-roundtrip, hello, tunnel, or witness behavior. `store-reads.feature` uses other P9 cases but does not index the renamed replacement fixture or deposit a replacement. |

`features/a-identity.feature` is the source feature, not a cross-feature
candidate. Its executable scenarios did not change in this range; only its
round-status comment changed.

### `FULL_AUDIT`

None.

The correction changes one Provider call path, not a shared helper, Core API,
wire schema, identity format, or cross-runner invariant. `put_once` is newly
used by `deposit_did` but its contract and all three backends are unchanged.

## Other targeted surfaces

These are targeted follow-ups, not reasons to restart a Gherkin audit.

### IMP-AID-01 — normative threat-model wording

**Classification: `TARGETED`.**

`spec/10-threat-model.md:40-41` still says to publish a new DID document
“signed by the cold succession key.” Read literally, that conflicts with:

- `spec/01-identity-and-keys.md`, where `did.json` carries `#root` and the
  epoch transition is the only artifact succession signs;
- the binding AID-001 decision;
- the accepted Provider behavior;
- `spec/04-mandates.md`, where identity rotation binds distinct DIDs through
  the complete transition digest.

Manual recommendation: clarify §10.4 so the successor document is root-signed
and the cold succession key authorizes it through the separate transition.
This is a normative wording correction; it does not require a full feature
audit because the executable identity and Gamma contracts already agree.

### IMP-AID-02 — Gateway owner replication

**Classification: `TARGETED`.**

`GatewayStore::replicate_now`/`replicate_paths` re-PUT `did.json` during a
full sweep. Exact bytes remain idempotent under the accepted Provider change.
`replicate_owner_history` is already stronger: it reads the remote document,
skips an equal JSON value, and returns `AlreadyExists` before writing when the
remote document differs.

No Gateway code depends on succession-signed replacement, and the generic
`RemoteStore` error already preserves `artifact_invalid` and its closed
`reason`. A small regression would nevertheless protect this integration
boundary:

1. a repeated full sweep with identical `did.json` remains successful;
2. a byte-different same-DID document yields
   `immutable_conflict` and leaves the remote bytes unchanged;
3. owner-history replay continues to stop a mismatch client-side.

There is no existing Gateway Gherkin scenario for this exact boundary, so the
recommendation is a targeted helper/E2E regression, not a Gateway feature
restart.

### IMP-AID-03 — P9 format and runners

**Classification: `TARGETED`, already covered.**

P9 changes are localized:

- `fixtures.rotation` becomes `fixtures.same_did_replacement`;
- `did_rotation_ok` and `did_rotation_root_signer` are replaced by the two
  explicit refusal cases;
- one signed unsupported-version genesis case is added;
- the independent verifier now uses `cryptography` and checks 58 assertions
  over 32 cases.

Repository consumers are compatible:

- Provider Cucumber indexes the new fixture names;
- the Rust binary replay and `red-replay-p9.py` iterate cases generically;
- `store-reads.feature` consumes unaffected P9 cases;
- no non-archive code or runner retains an old P9 case or fixture name.

The accepted review reproduced Provider Cucumber and binary replay. This run
also reproduced `P9 ok (58 checks, 32 cases)`. No other vector family is
affected.

## Commands and results

| Command or search | Result |
|---|---|
| `git status --porcelain=v1 -uall` | clean at start |
| `git rev-parse HEAD` | `b48386a6898fa444873968c3f43dfdb860ce0a03` |
| `git cat-file -t <baseline>` / `<candidate>` | both `commit` |
| `git diff --stat <baseline>..<candidate>` | 11 files, 342 insertions, 128 deletions |
| `git diff --name-status <baseline>..<candidate>` | exact 11-file inventory recorded above |
| `git diff --check <baseline>..<candidate>` | passed |
| `rg --files -g '*.feature'` plus grouped inventory | 53 feature files: 18 top-level, 24 Gateway, 11 Provider |
| symbol/phrase searches described above | one direct Gherkin consumer; no stale executable P9 name; no changed shared Core or storage API |
| `python3 vectors/verify-p9.py` | `P9 ok (58 checks, 32 cases)` |

Behavioral Rust gates were not rerun by the impact reviewer. The accepted
review already reproduced the exact candidate's targeted Provider, P9,
Core, Bundle, and Cucumber gates. This report adds dependency evidence, not a
second correction review.

## Findings handled and limits

Handled:

- all 52 Gherkin files outside the source feature were classified;
- all changed production symbols, step definitions, P9 formats, and runner
  consumers were traced;
- semantically adjacent identity rotation and normative sections were checked;
- two targeted follow-ups outside the accepted correction were identified.

Limits:

- no live S3 or deployed Provider was exercised;
- no unversioned external consumer of P9 can be discovered from this
  repository;
- the future previous-document/transition/successor triplet transport remains
  undefined, exactly as recorded by the accepted audit;
- this review does not decide or implement that future transport.

## Manual recommendation and lifecycle

Do not start a `FULL_AUDIT` and do not automatically restart any feature.

Manually schedule:

1. the §10.4 wording clarification in `spec/10-threat-model.md`;
2. the narrow Gateway replication regressions above.

The local impact skill requires this report but does not explicitly authorize
the impact reviewer to mutate domain/orchestrator state. Therefore this run
does not change either `STATE.md`. After a human accepts this report, the
orchestrator may mark the `a-identity` domain and global impact-review state
`COMPLETE`. No further audit skill is recommended from the evidence in this
run.

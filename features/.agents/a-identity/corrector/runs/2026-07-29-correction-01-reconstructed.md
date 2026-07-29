# Reconstructed conclusion — Identity correction, round 1

| Field | Value |
|---|---|
| Type | `RECONSTRUCTED` |
| Source role | external correction agent |
| Commit date | 2026-07-29 |
| Baseline | `be2d098` |
| Candidate commit | `56436f3` |
| Branch | `fix/aid-001-002-005-identity-fail-closed` |
| Claimed findings | `AID-001`, `AID-002`, most of `AID-005` |
| Result | `REVIEW_REQUESTED` |

## Provenance

This conclusion was reconstructed from commit `56436f3`, its message and diff,
and results written by the corrector into the public audit. It is not an
independent review and does not move any finding to `VERIFIED`.

## Observable candidate corrections

### AID-001

- explicit validation of DID version and signature metadata;
- validation of all four keys with their expected codecs;
- closed wire schemas that reject unknown members;
- correctly re-signed negative cases that isolate semantics.

### AID-002

- separation between `verify_declaration(prev)` and
  `verify_succession(prev, next)`;
- validation of both DID documents;
- binding of `prev_did` and `next_did` to the presented documents;
- rejection of a transition to the same identity;
- a `Then` step that actually passes the successor document.

### AID-005

- claimed increase from 9 to 30 scenarios;
- added wire cases, correctly signed but invalid documents, and incorrectly
  bound transitions;
- added Bundle/WASM surface tests;
- claimed byte-identical preservation of positive A2 vectors.

## Diff

```text
7 files changed
1130 insertions
158 deletions

M docs/audits/features/README.md
M docs/audits/features/a-identity.md
M features/a-identity.feature
A rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs
M rust/crates/aithos-bundle/tests/cucumber.rs
M rust/crates/aithos-core/src/did.rs
M rust/crates/aithos-core/tests/a2_did.rs
```

## Results reported by the corrector

The auditor must reproduce these results:

```text
Before correction:
workspace 627 tests
bundle cucumber 815 scenarios

RED:
a2_did: 3 expected semantic failures
cucumber: 18 of 21 new scenarios failed

After correction:
workspace 632 tests, 0 failures
bundle cucumber 836 scenarios, 0 failures
```

The corrector reported a pre-existing `cargo fmt --check` deviation in
`aithos-gateway/src/core_bridge.rs`. The reviewer must verify that
classification without silently including it in this correction.

## Declared out of scope

- `AID-003`: not addressed;
- `AID-004`: not addressed;
- no cold-custody claim;
- no independent validation of this conclusion.

## Requested handoff

Run `audit-a-identity` in `review` mode on `be2d098..56436f3`. Accept or reject
AID-001, AID-002, and AID-005 separately. Promote a finding to `VERIFIED` only
after reproducing the evidence and inspecting the public surfaces.

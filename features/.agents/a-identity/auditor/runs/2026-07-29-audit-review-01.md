# Conclusion — independent Identity review, round 1

| Field | Value |
|---|---|
| Type | `REVIEW` |
| Role | `audit-a-identity` auditor |
| Date | 2026-07-29 |
| Review branch | `codex/review-a-identity` |
| Observed HEAD | `0601b9f9106988385c2b38ed9d4a2e2370ab728a` |
| Audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Candidate commit | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Inspected sibling client | `c6f615123ca3dc83708ba029b898375409551719` |
| Initial worktree state | clean |
| Result | `DECISION_REQUIRED` |
| Blocking prerequisite | Provider `did.json` replacement semantics |

## Method provenance

This review predates the formal two-pass rule now defined in
`features/.agents/PROCESS.md`.

The auditor independently inspected current Rust paths, public surfaces, and
test behavior rather than trusting the corrector's report. However, the
baseline, candidate diff, prior findings, and correction context were already
known before those code traces were written. This run is therefore
**history-aware and Pass-A-contaminated** under the newer process. It must not
be relabeled as a clean history-blind Pass A after the fact.

The technical evidence and reproduced results below remain valid. A future
round that claims full compliance with the new process must start fresh review
units, freeze their current-code verdicts, and only then inspect this run and
the Git diff.

## Verdict

| Finding | Review verdict | Reason |
|---|---|---|
| `AID-001` | `DECISION_REQUIRED` | Core and ordinary consumers are hardened. Provider `artifacts::deposit_did` preserves distinct same-DID replacement semantics; whether that remains Provider-specific or adopts §10.4 is a protocol-owner decision. |
| `AID-002` | `VERIFIED` | The previous/transition/successor triplet is received and validated; bindings, signatures, metadata, and distinct identities are covered. |
| `AID-005` | `VERIFIED_WITHIN_PILOT_SCOPE` | All 21 added scenarios are honest, selected, and green. The ceremony depends on out-of-round AID-003/AID-004; independent negative vectors and an automated count gate are improvements rather than prerequisites for this pilot. |
| `AID-003` | not addressed | Outside round 1 correction; remains open. |
| `AID-004` | not addressed | Outside round 1 correction; remains open. |

## Reviewed diff

The exact `be2d098..56436f3` diff contains 7 files, 1,130 insertions, and
158 deletions:

- `rust/crates/aithos-core/src/did.rs`:
  `DidDocument::verify`, `EpochTransition::{verify_declaration,
  verify_succession}`, closed serde schemas, and signature constants;
- `rust/crates/aithos-core/tests/a2_did.rs`:
  three additional AID-001/AID-002 tests;
- `rust/crates/aithos-bundle/tests/cucumber.rs`:
  Identity steps and scenario-specific verdicts;
- `rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs`:
  Bundle and mandate-chain/WASM replay;
- `features/a-identity.feature`: 21 additional examples/scenarios;
- `docs/audits/features/{README.md,a-identity.md}`: correction evidence.

`git diff --check be2d098..56436f3` is clean.

## AID-001

### Accepted evidence

`DidDocument::verify` now validates:

- `DID_VERSION`, `ed25519`, and `#root`;
- root, content, and succession as Ed25519 keys with `VerifyingKey`
  construction;
- key exchange with the X25519 codec;
- the `id ↔ root` binding and root signature;
- unknown wire-member rejection on `DidDocument`, `DidKeys`, and
  `SignatureBlock`.

The following ordinary consumption paths converge on that verdict:

- `Bundle::open` and `Bundle::verify`;
- WASM `verify_mandate_chain` through `mandate::verify_chain`;
- Catalog `verified_owner_did`;
- Gateway, including `Bundle::open` and control snapshots;
- sibling client `c6f6151`, whose DID loads call `DidDocument::verify`.

### Protocol decision required

Provider `did.json` replacement remains a parallel path:

- `artifacts::deposit_did` accepts replacement under the stored document's
  succession authority and does not call `doc.verify()`;
- it does not validate `doc.version` or `doc.keys.kex`;
- it decodes root/content/succession but does not construct their
  `VerifyingKey` values;
- P9 fixture `did_rotation_ok` confirms that a `#succession` document is
  persisted even though the Core verdict requires `#root`.

This surface can durably commit an object that Core/Bundle consumers reject on
reopen. P9, however, codifies same-DID succession distinct from the §10.4
epoch transition. Choosing between those semantics is a protocol decision.
The Core portion of AID-001 is accepted, but no Provider correction should be
requested before an explicit decision.

## AID-002

`EpochTransition::verify_succession(prev_doc, next_doc)`:

- calls the strict validator on both documents;
- validates version, algorithm, and `#succession`;
- binds `prev_did` and `next_did` to the presented documents;
- rejects identical identities;
- verifies the signature under the previous succession authority.

The step “transition is signed by the succession key” now passes `next_doc`.
Each of the 10 Outline defects and the root-claiming-`#succession` case builds
its own transition/document and consumes its own result.

Provider same-DID replacement remains explicitly named as a different
operation and does not claim to implement §10.4. No production caller used the
old `EpochTransition::verify`.

Verdict: AID-002 moves to `VERIFIED`.

## AID-005

The delivered evidence is real:

- the post-signature alteration scenario now has a precise name;
- 7 correctly re-signed but invalid documents;
- 3 unknown wire-member cases;
- a positive `Then` that verifies the full triplet;
- 10 incorrectly bound transitions and 1 root signature claiming succession;
- no Identity step is empty, proxy, `@wip`, or backed by a `OnceLock`.

Remaining initial-audit requests are:

1. a ceremony scenario through the real identity-creation surface — dependent
   on out-of-round AID-003/AID-004;
2. independently generated A2 negative vectors — robustness improvement;
3. a targeted gate that fails unless exactly 30 scenarios run — tooling
   improvement. This review manually verified the count.

The corrector also reported 3 A2 RED tests and 18 RED scenarios through
temporary shims. Those shims are not versioned; the auditor did not change
Rust to reconstruct the exact counts. The baseline diff statically establishes
the old defects, but not those counts. This limitation does not undermine the
corrected scenarios or their GREEN gates.

Verdict: AID-005 moves to `VERIFIED_WITHIN_PILOT_SCOPE`.

## Commands actually executed

### Direct-worktree limitation

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
EXIT=101
package collision in the lockfile:
aithos-bundle from the review worktree and aithos-bundle from the main worktree
```

Sibling client `c6f6151` references `../aithos-core`. To test the immutable
candidate without modifying Rust or Cargo files, exact Git archives of
`56436f3` and `c6f6151` were extracted under a temporary sibling layout.

### Targeted gates on exact archives

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
EXIT=0
a1_genesis: 4 passed
a2_did:     6 passed

cargo test -p aithos-bundle --test aid_identity_surfaces
EXIT=0
2 passed

cargo test -p aithos-bundle --test cucumber
EXIT=0
18 features
114 rules
836 scenarios (836 passed)
3568 steps (3568 passed)
```

The output enumerated all 30 Identity scenarios and their 93 steps as passed.

### Workspace gate

```text
cargo test --workspace --no-fail-fast
EXIT=101
28 targets failed
```

Identity targets were green. The 28 failures occurred when
CLI/Gateway/Provider tests attempted to open local sockets or services:
`Operation not permitted`. An outside-sandbox retry was requested and denied
by execution policy. The workspace gate is environmentally inconclusive and
is not presented as green.

The non-network Provider runner passed 151/151 scenarios and 992/992 steps,
including P9 `did_rotation_ok`, which informs AID-001.

### Formatting

```text
cargo fmt --all -- --check
EXIT=1
rust/crates/aithos-gateway/src/core_bridge.rs:1355
```

The `core_bridge.rs` blob is identical in baseline and candidate
(`774672a0e2d4db1e866d3eb1d85106e53f684f80`). The deviation is pre-existing
and outside the Identity diff.

## Limits

- This run is history-aware and cannot serve as a clean Pass A.
- The workspace gate could not be rerun outside the sandbox.
- The auditor did not rerun Clippy.
- RED counts obtained with temporary shims are reported, not reproduced.
- No Rust file was modified.
- AID-003 and AID-004 were not closed, corrected, or broadened.

## Handoff

Route first to the protocol owner:

1. decide whether Provider `did.json` replacement remains a Provider-specific
   same-DID succession operation or adopts the §10.4 epoch transition;
2. only then route to `correct-a-identity`, round 2, baseline `56436f3`, if a
   correction is required;
3. leave AID-002 and AID-005 unchanged and verified within pilot scope;
4. do not address AID-003/AID-004 in this round.

---

**Annotation (2026-08-02, per the b-derivation impact review, rec. 4).** The
`EXIT=0` lines of the gate transcripts above are not proof: at this run's
revision the `aithos-bundle` Cucumber runner exited 0 regardless of failures
(`BDER-011`, pre-existing, fixed and `VERIFIED` on 2026-07-30). The printed
scenario and step counters that follow each `EXIT=0` line remain valid
evidence. This report is otherwise unmodified.

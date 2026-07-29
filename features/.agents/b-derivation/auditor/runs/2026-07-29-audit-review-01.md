# Correction review 01 — `b-derivation.feature`

| Field | Value |
|---|---|
| Run type | Correction review, round 1 |
| Role | `audit-b-derivation` (review mode) |
| Date | 2026-07-29 |
| Immutable baseline | `fa8fa797b897a762a0dfd7fc20910f053ce349ed` |
| Correction commit | `3d6fa51aaf9049e0deb81873242103c49f86de08` |
| Candidate reviewed (tip) | `1ab331a6c8806cd9c2e7845a452501c60d9dd72c` |
| Review branch | `codex/review-b-derivation`, created from `1ab331a` |
| Worktree state | Clean at `1ab331a` before and after the review; no Rust file modified in the repository |
| Scope | `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`, `BDER-005`, `BDER-009` |
| Out of scope | `BDER-006` (decision), `BDER-007`, `BDER-008`, `BDER-010` |
| Outcome | Six findings accepted `VERIFIED`; two new findings opened (`BDER-011`, `BDER-012`) |

## Contamination disclosure — read this before the Pass A verdicts

The feature auditor's own working context was contaminated before Pass A could
start: it had already read `STATE.md` in full (including the assigned-findings
list and the corrector's framing of the reference mutant), the tail of
`corrector/runs/2026-07-29-correction-01.md`, and the correction commit
messages. Under `PROCESS.md` ("If they are already present in the active
context, disclose that contamination and use a fresh review unit when
practical") Pass A was therefore **not** executed by the feature auditor.

Pass A was delegated to four fresh review units, each with no prior context,
operating on a sanitized export of the candidate:

- source: `git archive 1ab331a`, so **no `.git` directory exists** in the tree
  the units read — history-blindness is enforced by construction, not by
  instruction;
- `docs/audits/` removed entirely;
- every `features/.agents/*/runs/` directory removed;
- the corrector role directory removed;
- `STATE.md` replaced by a routing-only stub (mode, revision, assigned scope,
  output path — the four fields `PROCESS.md` allows).

The units received the feature file, the step definitions, the production code,
the spec, the vectors and `DOMAIN.md`. Their verdicts were returned and frozen
before the feature auditor opened the diff, the corrector's report, or the
public audit for Pass B.

Residual contamination the units could not be shielded from: the candidate's
`.feature` file itself carries `@audit-implemented @bder-00N` tags and pointer
comments. Those are part of the artifact under review and could not be removed
without altering it. No unit was told what any finding claims.

Review units:

| Unit | Scope |
|---|---|
| A1 | `Rule: Derivation is deterministic and per-segment` (scenarios 1-2) |
| A2 | `Rule: Holding a folder yields its subtree, nothing else` (scenarios 3-5) |
| A3 | `Rule: Tag views anchor at folders` (scenario 6) |
| A4 | Shared-state / integration pass: World, shared steps, fixtures, global state, parallel production surfaces, runner configuration |

## Environment limit, disclosed

The reviewing workstation exposes no Rust toolchain to this role (`which cargo`
returns nothing). Every gate below was executed on a Linux `x86_64` container
holding a `git archive` export of `1ab331a` plus the sibling `aithos-client`
path dependency at `c6f6151`, with `rustc 1.95.0`. The tree under test is
byte-identical to the tracked content of `1ab331a`. The same limit was declared
by the corrector for its own gates; this review reproduces the evidence in an
independent container, from an independent export, but not on independent
hardware.

## Feature gate — once, on the immutable candidate

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @b-derivation

1 feature
3 rules
6 scenarios (6 passed)
30 steps (30 passed)
```

The block was counted by name, per `DOMAIN.md`. **The count in `DOMAIN.md` is
now stale**: it instructs the auditor to confirm "three Rules, six scenarios and
21 steps". The candidate legitimately executes 30 steps (nine assertions added,
no scenario removed). `DOMAIN.md` is corrected by this review.

No unfiltered Cucumber run, no workspace run, no `cargo fmt` gate was executed
by this role.

## Pass A — frozen verdicts (history-blind, four fresh units)

| # | Scenario | Frozen Pass A verdict |
|---:|---|---|
| 1 | The same path always yields the same key | `SEMANTIC_FALSE_POSITIVE` **on enforcement grounds only** — unit A1 states explicitly that the assertion content is `PROVEN`-grade and that the sole defect is that the harness cannot fail |
| 2 | Sibling nodes get unrelated keys | `SEMANTIC_FALSE_POSITIVE` — enforcement, plus a semantic residue: "under any production label" is realised as 21 fixture-local samples |
| 3 | A folder holder derives every descendant | `PARTIAL` — proves prefix-composability and shape distinctness; no external anchor on the grandchild and tag-anchor routes |
| 4 | A folder holder cannot reach sideways | `PROVEN` |
| 5 | Renaming never re-keys | `PROVEN` |
| 6 | A folder-local tag view is its own lock | `PARTIAL`, with the scope question frozen as `DECISION_REQUIRED` |

Three of the four units independently and unprompted reported the same
structural defect: `rust/crates/aithos-bundle/tests/cucumber.rs:19716` calls
`Cucumber::filter_run`, not `filter_run_and_exit`, under `harness = false`, so
the binary returns `()` and exits 0 regardless of step outcomes. That claim is
the origin of finding `BDER-011` and was verified empirically in Pass B.

Unit A4 additionally reported, and this review confirms by inspection:

- `node_keys` is a `Vec<[u8; 32]>`, not a map — the key-collision hazard the
  unit was asked to look for does not exist; the length preconditions added by
  `BDER-009` are present at both readers;
- `ProtocolWorld` derives `Default`, so no b-derivation World field can leak
  across scenarios;
- no `OnceLock`, cache, memo or filesystem state touches the derivation path,
  so the determinism assertion of scenario 1 is not vacuous for that reason;
- `folder_label`, `section_label` and `tag_label` each have exactly one
  production call site, inside `node_key`; no `aithos-bundle` or `aithos-core`
  surface re-implements the per-segment walk;
- `@wip` occurs in no feature file, so the runner's filter closure currently
  excludes nothing.

## Pass B — differential evidence

### The corrector's byte-identity claim holds

Independently verified by direct comparison of the two exports, not by reading
the report:

```text
IDENTICAL  rust/crates/aithos-core/src/derive.rs
IDENTICAL  rust/crates/aithos-core/src/path.rs
IDENTICAL  rust/crates/aithos-core/src/ids.rs
IDENTICAL  rust/crates/aithos-bundle/src/bundle.rs
IDENTICAL  rust/crates/aithos-bundle/src/structure.rs
IDENTICAL  rust/crates/aithos-bundle/src/grants.rs
IDENTICAL  vectors/b2-derivation.json
```

The full `fa8fa79..1ab331a` changed-file set is exactly the six files the
corrector declared: the two audit documents, `STATE.md`, the new corrector run
report, `features/b-derivation.feature`, and
`rust/crates/aithos-bundle/tests/cucumber.rs`. No undeclared change.

### `BDER-011` is pre-existing, not a regression of this round

`fn main()` in `rust/crates/aithos-bundle/tests/cucumber.rs` is **byte-identical
between `fa8fa79` and `1ab331a`**. The correction neither caused nor worsened
the harness defect. It is therefore not grounds to reject any assigned finding,
and it is not this corrector's to fix.

### Mutant battery — independently re-run on the candidate

Mutants were applied to a throwaway copy of the export, never to the
repository; `derive.rs` and `bundle.rs` were restored byte-for-byte afterwards
and the repository worktree was never mutated.

| Mutant | Scenarios failed / 6 | Corrector's claim |
|---|---:|---:|
| M1 — `node_key` returns a constant | 5 | 5 |
| M2 — `node_key` ignores the path | 5 | 5 |
| M3 — monolithic hash over the whole path | 4 | 3 |
| M4 — 31 bytes of the zone key copied into every node key | 4 | 4 |
| M5a — per-segment `parent XOR blake3(label)` **in `node_key`** | 4 | 4 |
| M5b — `derive_key` itself replaced by `parent XOR blake3(ctx)` | 3 | not reported |
| R1 — rename implemented as delete-and-recreate with a fresh sid | 1 | 1 |

M3 differs by one scenario from the corrector's figure; the two M3 instances
are not the same mutant (this review hashes the canonical path string in one
derivation), so this is a difference of instance, not a contradiction. Every
other reproduced figure matches the corrector's claim exactly.

**M5b is this review's addition and it matters.** The reference mutant named in
`STATE.md` is described as a `node_key` implementation; applied there (M5a) it
kills four scenarios including "A folder holder derives every descendant". If
instead the invertible step replaces `derive_key` itself (M5b), that scenario
**survives**, because its `Then` compares `derive_key(section_label, folder_key)`
against `node_key(zone, full_path)` — both sides move together under a mutated
primitive. Unit A2 predicted exactly this before seeing any of it. The
consequence is recorded as part of `BDER-012`, not as a rejection: `BDER-005`'s
closure criterion is written against the audit's own M1-M5 set, and against
that set the scenario scores 5 / 5.

Under M5a, M5b and R1 the failures are visible in the runner's output but
`cargo test` still exits 0 — observed three separate times. This is the
empirical proof of `BDER-011`.

### RED claims

- `BDER-004`'s RED claim was reproduced end to end: `Bundle::rename_folder` was
  mutated into a delete-and-recreate that mints a fresh folder sid and
  re-parents children and sections onto it. The scenario fails on
  `Then the derived key of "projets/intime/note1" is unchanged` with
  `assertion left == right failed: rename must never re-key`, and it is the only
  scenario in the feature that fails. Display-path resolution still succeeds
  under that mutant, so the scenario is catching the key movement, not a
  resolution error.
- `BDER-001`, `BDER-002`, `BDER-003` and `BDER-005`'s RED claims are the mutant
  columns above; each closure criterion is met by directly observed runs.
- `BDER-009`'s claim is structural and was verified by reading the two readers:
  `b2_pair` asserts `node_keys.len() == 2` before indexing, `anchors_distinct`
  asserts `len() == 3` before the set comparison.

## Reconciliation, finding by finding

### `BDER-001` — `VERIFIED`

Closure criterion: "M1, M2, M3 and M5 make this scenario fail. The B2 vector is
unchanged." Observed: M1, M2, M3, M4, M5a and M5b all fail this scenario, every
one of them on `And the key equals the B2 vector's deep section key byte for
byte`. The vector is byte-identical. The second path is genuinely rebuilt
through `NodePath::parse`, the zone fixture is the vector's `zone_dk_hex`, and
the literal label forms `aithos-core/v1/d/<sid>` and `aithos-core/v1/s/<sid>`
are pinned against test-side literals.

Pass A froze this scenario at `SEMANTIC_FALSE_POSITIVE` **on enforcement
grounds alone**, stating that it would otherwise freeze at `PROVEN`.
Reconciliation: the enforcement defect is `BDER-011`, it is pre-existing and
repo-wide, and it is not what `BDER-001` was about. The finding is accepted.

Recorded caveat, from unit A1: the two `derivations` counters in
`chain_is_per_segment` are incremented by the test's own loop and observe
nothing about how many `derive_key` calls `node_key` performed. The structural
force of that step comes entirely from `assert_eq!(key, first)`, which
reconstructs the chain segment by segment. That is enough, but the counters
should not be read as evidence.

### `BDER-002` — `VERIFIED`, with `BDER-012` opened on the residue

Closure criterion: "M4 and M5 make this scenario fail." Observed: M1, M2, M3,
M4, M5a and M5b all fail it. The corrector implemented literally what the audit
prescribed, including the `unrelated_identities`-style shared-window check.

Residue, recorded as `BDER-012` and not as a rejection: the `Then` phrase
"under any production label" is universally quantified while
`b2_production_labels` enumerates 21 labels over a space bounded only by the
ULID and tag alphabets; the search is forward-only; only the first sibling is
anchored to the vector, the second has no expected value; and the leak window
is 16 bytes, so a 15-byte leak of parent material would pass. Unit A1 built two
concrete mutants that break the scenario's stated outcome while passing every
one of its assertions.

### `BDER-003` — `VERIFIED`

Closure criterion: "M5 makes this scenario fail, and the scenario fails if the
held key is not folder 1's." Observed: M5a and M5b both fail it, on
`Then the held key is exactly the first folder's key` — that is, on the positive
control the finding asked for, whose assertion
`assert_eq!(hex::encode(held), folder1_key_hex)` fails by construction for any
substituted key. The `Given` now has a body and the `When` and `Then` read it.
The explored space is stated and asserted: `assert_eq!(paths.len(), 13_332)`.
The upward assertion exists and unit A2 verified analytically that M5a dies on
it at depth 1 independently of the positive control.

Recorded caveat: the sideways search runs to depth 3 (13 332 paths) while the
upward search is depth 1 (42 probes). Folded into `BDER-012`.

### `BDER-004` — `VERIFIED`

Closure criterion: "The `When` traverses a real rename production surface, and
the `Then` re-reads the section after rename." Both hold, traced by unit A2 and
confirmed here: `Bundle::rename_folder` at `bundle.rs:1534` mutates only
`FolderRow::name`, the edition is genuinely republished, and
`derived_key_unchanged` re-resolves the section from the stored index **at its
new display path**, pins the section sid, and recomputes the key over the
re-resolved folder chain. A no-op rename would panic on the `expect`. The R1
probe closes the RED claim.

Recorded caveat: four of the six steps are shared verbatim with
`d-bundle.feature:40-43`, and the final `Then` is a borrowed round-trip verdict
that asserts a body string, not a key. The scenario is not a `PROXY` — its two
derivation-specific steps execute its own case — but its derivation content
lives entirely in `derived_key_unchanged`. The reviewer confirms the shared-step
reuse the corrector flagged for confirmation: it is acceptable, and it is what
the initial audit explicitly asked for.

### `BDER-005` — `VERIFIED`

Closure criterion: "At least three distinct descendant shapes, still 5 mutants
out of 5." Both hold: section, grandchild section under a sub-folder, and tag
anchor; and M1, M2, M3, M4, M5a all fail this scenario. The cross-route
comparison the initial audit praised is preserved.

Recorded caveat: under M5b the scenario survives, because both sides of its
comparison go through the same primitive. Pass A froze it at `PARTIAL` for the
related reason that the grandchild and tag-anchor routes have no byte-exact
anchor, while `deep_section_key_hex` sits one line away. Folded into
`BDER-012`. The "future descendant" half of invariant 8 remains uncovered, as
the initial audit itself conceded — `node_key` is a pure function of a path and
has no notion of node existence; that claim belongs to `e-mandates.feature`.

### `BDER-009` — `VERIFIED`

Closure criterion: "Any future step composition fails loudly instead of
comparing the wrong pair." Both readers now state their precondition. Unit A4's
independent enumeration of the accumulator confirms the shape is pinned at both
readers and that no b-derivation World field is shared with another feature.

Recorded caveat: `anchors_distinct` remains value-free — it pins the count but
never says which entry is which. That is `BDER-006` territory, not `BDER-009`.

### `BDER-006` — unchanged, `DECISION_REQUIRED`

Untouched by the correction, as instructed. Unit A3 reached the audit's two
options independently and confirmed the ambiguity is genuine: the derivation
fact lives in `spec/02-content-tree.md` §2.5 and `DOMAIN.md`'s tag clause is
derivation-only (option A), while §2.9 states the anchor and the wrap bridge as
one fact and `DOMAIN.md` invariant 9 carries both halves in one sentence
(option B).

One new fact for the decision owner: `DOMAIN.md` routes "tag-view rebuild and
the wraps that populate an anchor" to `d-bundle.feature`, and **that feature
contains no tag-view or wrap scenario**. Under option A, the wrap half of §2.9
is routed to a destination that does not currently cover it; its only executable
coverage is the positive-direction behaviour in `e-mandates.feature:28-32` and
`:48-52`. Option A therefore leaves a real gap unless `d-bundle` is extended in
the same movement.

## New findings opened by this review

### `BDER-011` — the `aithos-bundle` Cucumber gate cannot report failure

**P1 — OPEN — pre-existing at `fa8fa79` — repo-wide, not `b-derivation`-specific.**

`rust/crates/aithos-bundle/tests/cucumber.rs:19716` calls
`ProtocolWorld::cucumber().filter_run(...)` and discards the returned writer.
With `harness = false`, `main` returns `()` and the process exits 0 whatever the
steps did. `.fail_on_skipped()` is also absent, so an unmatched step is not an
error either. The two sibling runners in the same repository do it correctly:
`aithos-gateway/tests/cucumber.rs` and `aithos-provider/tests/cucumber.rs` both
use `.fail_on_skipped().filter_run_and_exit(...)`.

Observed three times in this review: with four scenarios failing (M5a), with
three failing (M5b) and with one failing (R1), `cargo test ... --test cucumber`
exited 0 each time.

Consequences, stated plainly:

- the canonical feature gate of `DOMAIN.md` proves nothing by its exit code;
  only the printed scenario/step counts carry information, which is why
  `PROCESS.md` and `DOMAIN.md` already require counting the block by name — that
  instruction is currently the only thing standing between this suite and a
  silent green;
- the same applies to the corrector's global Cucumber gate and to the workspace
  gate for this test target, and to CI, which runs `cargo test --workspace`;
- this affects all 18 features, not `b-derivation` alone.

Not attributable to this correction and out of its assigned scope. Remediation
is a repo-wide change that may turn currently "green" scenarios red across
features; it must be scoped by the orchestrator, not by a `b-derivation`
corrector. Routed to the impact review as its first item.

### `BDER-012` — the corrected negatives remain bounded samples, and one comment overstates its authority

**P3 — OPEN — not assigned to a round.**

The corrections are real and measurable; this finding is about what remains
after them, so that the next round starts from an honest baseline.

1. Scenario 2's `Then` says "under any production label" while the search is 21
   fixture-local labels, forward-only. Scenario 4's equivalent phrase is honest
   because its step asserts the size of the explored space; scenario 2's is not
   qualified anywhere.
2. Scenario 2 anchors only the first sibling to the vector. The second sibling
   has no expected value, so a mutation confined to it passes.
3. The zone-leak check uses a 16-byte contiguous window; a 15-byte leak of
   parent material passes.
4. The upward containment search of scenario 4 is depth 1 while its sideways
   search is depth 3.
5. Scenario 3's grandchild and tag-anchor routes compare production against
   production with no external anchor, which is why M5b survives it.
6. `cucumber.rs:149-152` states that only vector fields corroborated by an
   independent generator are used as external authority; `cucumber.rs:12190-12194`
   then uses `sibling_section_key_hex`, which no `vectors/gen-*.py` recomputes.
   The value's mutant-killing power is real, but the comment's claim is not
   accurate. This is adjacent to `BDER-007` and `BDER-008` and should be closed
   with them.

## Documents and files changed by this review

| File | Change |
|---|---|
| `docs/audits/features/b-derivation.md` | Six findings moved to `VERIFIED` with the evidence reproduced here; new `BDER-011` and `BDER-012`; the correction-candidate section rewritten as a reviewed section |
| `docs/audits/features/README.md` | Index row |
| `features/b-derivation.feature` | `@audit-implemented` markers removed from the five scenarios whose findings are `VERIFIED`; scenario 2 keeps a marker for `BDER-012`; a feature-level comment records `BDER-011`; scenario 6 keeps `@audit-partial @bder-006` |
| `features/.agents/b-derivation/DOMAIN.md` | Gate step count corrected from 21 to 30 |
| `features/.agents/b-derivation/STATE.md` | `IMPACT_REVIEW_REQUESTED` |
| This report | New |

No Rust file and no vector was modified by this role.

## Limits of this conclusion

- Seven mutants are seven points in an infinite space. The figures above are
  evidence of discriminating power, not a calibrated mutation score.
- `BDER-011` was proven by observing exit code 0 alongside printed failures. The
  `cucumber` 0.21 source was read in the container's registry to confirm
  `filter_run` does not exit, but the reading of every writer path in that crate
  was not exhaustive.
- The 13 332-path enumeration bounds only the label space the production code
  can itself build from a held key. The universal negative of `BDER-003` rests
  on BLAKE3 one-wayness and is not testable in principle.
- Pass A was executed by fresh units, not by the feature auditor; the feature
  auditor owns the aggregation, Pass B, and the reconciliation above, and its
  own context was contaminated from the start, as disclosed.
- The gates were run in a container on an export, not on the reviewing
  workstation. Results are reproducible on any host with the pinned toolchain.
- This review did not run unfiltered Cucumber, the workspace suite, or
  `cargo fmt`. The corrector's report of a pre-existing `cargo fmt` failure at
  `aithos-gateway/src/core_bridge.rs:1355` is repeated here as a claim, not as
  reproduced evidence.

## Next action

`review-gherkin-impacts` (orchestrator), against `fa8fa79..1ab331a`, with two
inputs it must not lose:

1. `BDER-011` first — it is a shared-harness defect affecting all 18 features
   and it must be scoped before any further correction round claims a green
   gate as evidence;
2. the changed surface of this round is confined to
   `rust/crates/aithos-bundle/tests/cucumber.rs` and
   `features/b-derivation.feature`; the four step phrases now shared with
   `d-bundle.feature:40-43` are the one place where this feature reaches into
   another's step set, and they are unmodified reads.

`BDER-006` remains `DECISION_REQUIRED` for its human owner and does not block
the impact review. `BDER-007`, `BDER-008`, `BDER-010` and the new `BDER-012`
remain `OPEN`.

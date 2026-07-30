# Run — initial audit of `c-headers.feature`

| Field | Value |
|---|---|
| Run type | initial audit, round 1 |
| Role | `audit-c-headers` (auditor) |
| Date | 2026-07-30 |
| Branch | `codex/audit-c-headers` |
| `main` base | `240c6589986af6115530c90a7aa8646c2c44b68f` |
| Observed revision | `3803fe806702143d5bb887b5ddc33fd3e0526285` |
| Worktree state | clean except the pre-existing untracked `_to_delete/` |
| Scope | the eight existing scenarios of `features/c-headers.feature`; four `Rule` blocks |
| Public audit | `docs/audits/features/c-headers.md` |
| Frozen Pass A | `pass-a/RU-1.md` … `pass-a/RU-4.md` (this directory) |
| Next action | correction — lots 1 and 2 first |
| Expected skill | `.agents/c-headers/corrector/correct-c-headers/SKILL.md` |

The observed revision differs from the `main` base only by the commit that
created this feature's agent domain (`DOMAIN.md`, `STATE.md`, the two
specialized skills, and the `AGENTS.md` routing block). No `features/` or
`rust/` file audited here differs between the two revisions.

## Preparation

`git status`, the branch list and the worktree list were inspected without
being changed. The previous accepted feature cycle (`b-derivation`) is
integrated into local `main`: `7854895 docs(derivation): record the
b-derivation impact review` is an ancestor of `240c658`. Two later commits
(`8d05c7d`, `240c658`) are unrelated to this feature. `codex/audit-c-headers`
was created from that exact `main`.

Three sibling worktrees exist for earlier cycles and were left untouched. A
pre-existing untracked `_to_delete/` directory was left in place; stale Git
lock files encountered during the run were moved into it rather than deleted,
per the repository's existing convention.

`features/.agents/scripts/verify-feature-tags.sh` → `feature tags ok (18 files)`.
The canonical tag is `@c-headers` (`features/c-headers.feature:1`).

## Review units and Pass A isolation

Four units, one per Gherkin `Rule`, matching the split named in `STATE.md`:

| Unit | Rule | Scenarios | Pass A verdicts |
|---|---|---|---|
| RU-1 | A line seals the node key to exactly one recipient | 4 | 1 `PROVEN`, 3 `PARTIAL` |
| RU-2 | The owner line is mandatory (I3) | 1 | 1 `PROVEN` |
| RU-3 | Grant is one appended line, touching nobody | 1 | 1 `PARTIAL` |
| RU-4 | Rotation cuts the revoked and re-links the parent | 2 | 1 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE` |

**Pass A isolation was enforced structurally, not by discipline.** Each unit ran
as a fresh agent against a `git archive` extract of the observed revision with
**no `.git` directory present**. Reading history was therefore impossible, not
merely forbidden. Each unit received: the contract, `PROCESS.md`, the shared
audit skill, the specialized skill, `DOMAIN.md`, and the routing fields of
`STATE.md`. None received another unit's verdict or any prior conclusion. No
prior conclusion exists for this feature — this is round 1, and
`docs/audits/features/README.md` carries no `c-headers` row.

Each unit was instructed not to run cargo; the integrating auditor owns the
single canonical gate run for this revision.

All four Pass A reports were written and frozen before Pass B began. They are
reproduced verbatim in `pass-a/`.

### Contamination status

- **Integrating auditor:** disclosed. The recent commit log was read while
  preparing the branch and writing `DOMAIN.md`, before the units ran. This does
  not touch the Pass A verdicts, which were produced in isolated contexts with
  no history available. Pass B is history-aware by definition.
- **RU-1:** uncontaminated. Read `docs/audits/features/README.md` for process
  convention; its index rows name `a-identity` and `b-derivation` verdicts, no
  `c-headers` row exists.
- **RU-2:** uncontaminated. Did not open any per-feature audit note.
- **RU-3:** uncontaminated for `c-headers`; disclosed incidental exposure to the
  same README index rows.
- **RU-4:** uncontaminated for `c-headers`; disclosed three incidental exposures
  to *other features'* identifiers — the README index, the `BDER-011` caveat
  supplied as routing context in `DOMAIN.md`/`STATE.md`, and `BDER-003`/`004`
  comments on unrelated World fields at `cucumber.rs:479-485`.

No unit was contaminated by a `CHDR-*` conclusion, because none existed
anywhere.

## Commands and results

Static check, once:

```
features/.agents/scripts/verify-feature-tags.sh
→ feature tags ok (18 files)
```

Canonical feature gate, once on the immutable observed revision:

```
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers
→ 1 feature
  4 rules
  8 scenarios (8 passed)
  28 steps (28 passed)
```

Every scenario is named individually in the runner output and every step is
ticked. The counts match the feature file exactly: four `Rule` blocks, eight
scenarios, 28 steps.

**The exit code was not used.** `GATE_EXIT=0` was observed and is recorded here
only for completeness. `BDER-011` is open on this baseline —
`rust/crates/aithos-bundle/tests/cucumber.rs:19730` calls `filter_run`, not
`filter_run_and_exit`, under `harness = false` — so the runner exits `0` even
when scenarios fail. The printed block is the evidence; the exit code is not.

No unfiltered Cucumber, broad regression, or workspace gate was run. No focused
test was run: no semantic contradiction required one.

### Execution environment, disclosed

The `rust/` workspace member `aithos-gateway` has a path dependency on a
sibling `aithos-client` checkout, and no Rust toolchain is available on the
mounted-device path. The gate was therefore executed in a container against a
`git archive` extract of `3803fe8` plus an extract of the sibling repository at
`c6f6151`. The extract is byte-equal to the audited revision by construction.
This affects reproducibility instructions, not the verdict; a reviewer with a
local toolchain reproduces the same command directly in the worktree.

## Pass B — history and differential evidence

Inputs: `git log --oneline --decorate -20`; `git log -p` for
`features/c-headers.feature`, `rust/crates/aithos-core/src/header.rs`,
`rust/crates/aithos-core/src/seal.rs`,
`rust/crates/aithos-core/tests/c1_header_seal.rs`,
`vectors/c1-header-seal.json`; `git log --stat` and `git log -L` over the
`c-headers` step ranges of `rust/crates/aithos-bundle/tests/cucumber.rs`.

Only six commits in 375 touch this feature's surface.

### B1 — the scenarios were written feature-first, then unblocked wholesale

`168d824 feature first: headers and seals contract (C, @wip)` (9 July 2026)
created all eight scenarios with **every scenario tagged `@wip`** and no step
definitions. `04f0eca step C complete: header seals, I3, grant, rotation,
up-link wrap (C1/C2)`, the same day, added `seal.rs`, `header.rs`, the step
definitions, the spec §3.8 text and the C1/C2 vectors — and removed all eight
`@wip` tags in that single commit.

This is the origin of the pattern the audit found. The scenarios were authored
as a contract before an implementation existed; the step definitions were then
written to make that contract green. Where a scenario's claim was structural
(`gets no line`, `every other line`, `restores derivation`) the step that made
it green asserted the nearest available *behavioral* consequence. Nothing
subsequently revisited the choice.

`04f0eca`'s own message claims "header.rs: … `append_line` leaves every other
line byte-identical" — an accurate statement about the *implementation*, which
the audit confirms. `CHDR-010` is that the *scenario* proves it against a
header with one other line.

### B2 — the step definitions have not changed since the day they were written

`git log -L` over the `Given`, `When` and `Then` ranges shows the c-headers
step bodies byte-identical from `04f0eca` to `3803fe8`. Only surrounding line
numbers moved. `f1ab74a test(features): add targeted audit gates` (29 July)
added the `@c-headers` tag and nothing else. No correction, refactor or review
has touched these steps in the three weeks since they were authored.

This matters for the verdicts: there is no history in which a stronger
assertion was weakened, and no prior reviewer's intent to reconstruct. The
Pass A reading of the current code is the whole story.

### B3 — the same defect class was found and fixed one Rule away, and this block was not revisited

`3d6fa51 test(derivation): honest assertions for BDER-001..005 and BDER-009`
strengthened the `b-derivation` `Then`s that sit **immediately above** the
c-headers block in the same file. It added, for example, to `anchors_distinct`:

```rust
// BDER-009: the cardinal reader states its precondition too.
assert_eq!(w.node_keys.len(), 3, "this Then reads exactly three derivations");
```

— i.e. exactly the correction `CHDR-008` proposes for `stranger_recovers_nothing`
(tie the assertion to the cardinality of what it reads) and `CHDR-004`
proposes for `revoked_cannot_open` (state the precondition the assertion
depends on). The diff stops at `anchors_distinct`; `owner_opens` begins on the
next line and was left untouched.

This is the strongest differential evidence in the run. The defect family this
audit reports is not a novel interpretation: it is the family this repository's
own process already found, accepted and corrected in the adjacent feature one
day earlier. The c-headers block was simply out of that correction's assigned
scope.

### B4 — `check_rotation` and `build_at` postdate the scenarios

`Header::check_rotation` was introduced by `4638a57 step G complete: revocation
ladder` and `Header::build_at` by `97d7187 step G closed: move-as-rotation`,
both **after** `04f0eca` unblocked the c-headers scenarios.

This explains `CHDR-003` without excusing it: the function that owns the
"rotation cuts the revoked" well-formedness contract did not exist when the
Rule claiming that contract was made green, and nobody returned to wire them
together. `Header::rotate` still calls only `check_owner_line`; every
production caller compensates by calling `check_rotation` itself
(`revoke.rs:199`, `vault.rs:400`). The gap is in the Gherkin's reach, not in
the product.

### B5 — the vectors were generated independently, and predate nothing

`04f0eca` introduced `vectors/c1-header-seal.json` "generated independently
(PyNaCl + manual RFC5869 HKDF): Rust matches Python byte-for-byte on epk +
ciphertexts", together with `c1_header_seal.rs`. The vector's inputs have never
matched the Gherkin fixtures — the two were written in the same commit with
different constants, and `CHDR-009` is that no Gherkin path was ever wired to
them, not that a wiring was lost.

### Reconciliation

Pass B changed no Pass A verdict. It supplies motive for four of them:

| Pass A verdict | Pass B effect |
|---|---|
| RU-1: 1 `PROVEN`, 3 `PARTIAL` | unchanged; B1 explains the assertion shapes, B3 shows the correction pattern already accepted in this repo |
| RU-2: `PROVEN` | unchanged; B1 confirms the `Given` was decorative from the first commit |
| RU-3: `PARTIAL` | unchanged; B1 confirms `04f0eca` claimed the byte-identity property for the *implementation*, which holds — the scenario's single-line fixture is the gap |
| RU-4: `PARTIAL` + `SEMANTIC_FALSE_POSITIVE` | unchanged; B4 explains why `check_rotation` is never called, and B1 explains why the up-link scenario is a constructor round-trip: it was written before any derivation machinery it could have used was wired into the World |

Nothing in the history upgrades a scenario to `PROVEN`, and per `PROCESS.md`
historical intent alone could not do so anyway. The `SEMANTIC_FALSE_POSITIVE`
on scenario 8 is confirmed, not softened: the commit that unblocked it claimed
"up-link wrap restores parent derivation" as delivered, and the scenario
asserting that claim performs no derivation.

## Shared-state and cross-scenario integration pass

Performed last, over the four frozen reports and the shared fixture layer.

- **World isolation holds.** `ProtocolWorld` is `#[derive(Debug, Default,
  World)]` (`cucumber.rs:459`) and cucumber 0.21.1 constructs a fresh world per
  scenario. `header`, `saved_line`, `opened`, `wrap_obj` and `rejection` cannot
  survive a scenario boundary. No `OnceLock` cache in the file is touched by any
  header step. **No cross-scenario leakage was found.**
- **`opened` is written only by `open_into`** (`cucumber.rs:7396-7404`), whose
  three call sites are all c-headers `When`s. `opening_rejected` reads `.last()`
  while `stranger_recovers_nothing` reads all of it — a divergence to keep in
  mind if a correction adds an earlier push in the same scenario (it will: the
  positive control of `CHDR-007` does exactly that, so that correction must
  update both readers coherently).
- **`saved_line` has one writer and one reader**, both inside this feature, and
  the writer is the `Given` the scenario actually runs. RU-3's byte-identity
  assertion is correctly ordered.
- **Shared step functions are the main structural risk.**
  `sealed_header_owner_only` registers two `Given` phrases; `grantee_opens` and
  `opening_rejected` each register two `Then` phrases. Their hardcoded
  constants are correct for every scenario that uses them *today*, by
  coincidence of fixture naming rather than by construction. `CHDR-007` and
  `CHDR-010` both require splitting or reworking one of these shared functions;
  the corrector must not assume a change is local.
- **`rejection` is shared with non-header features** (`cucumber.rs:7796` writes
  it from an identity step; `:12511` reads it with another substring match).
  Safe today because the world resets, but `CHDR-014`'s typed-error correction
  should not widen that field's contract.
- **The ephemeral/nonce indices are a de-facto global allocation table** across
  this feature: `eph(1..3)` the fixtures, `eph(4)` the replay decoy, `eph(5)`
  the append, `eph(6..7)` the rotation, `non(9)` the wrap. Corrections that add
  recipients must extend rather than renumber.
- **Production surfaces do not bypass the verdicts.** `Header` is the only
  production consumer of the line primitives; every grant path reaches
  `append_line`; both rotation surfaces call `check_rotation`; the content-tree
  rotation posts the up-link wrap. The weaknesses found are in the evidence
  layer, not in `aithos-core` or `aithos-bundle`.

## Adversarial verification pass

After the audit was drafted, its four highest-severity claims (`CHDR-001`,
`CHDR-002`, `CHDR-003`, `CHDR-006`) were handed to an independent verifier
instructed to refute them, defaulting to "refuted" on any claim it could not
independently confirm. **All four survived.** Two were settled empirically:

- `CHDR-003`: `check_rotation`'s body replaced by an unconditional `panic!` →
  the gate still printed `8 scenarios (8 passed) / 28 steps (28 passed)`.
- `CHDR-006`: `key_version` removed from `line_aad` (shared `aad()` helper, and
  therefore `wrap_aad`/`blob_aad`, left untouched) → the feature gate stayed
  green, and so did the **unfiltered** suite: `18 features / 114 rules / 836
  scenarios (836 passed) / 3577 steps (3577 passed)`. The only failure in the
  workspace was `c1_header_seal::c1_owner_and_grantee_lines`.

Both mutations were reverted; restoration was verified by `diff` and `md5sum`
against pre-edit backups and by a clean-tree re-run.

The verifier returned three wording corrections, all applied to the public
audit before it was committed:

1. `CHDR-001`'s "no derivation" was literally false — `wrap_seal`/`wrap_open`
   do call `derive_key(CTX_WRAP_KEY, …)` (`seal.rs:136-137`, `:150`). Corrected
   to "no **content-tree** derivation: no `node_key` walk, no
   `folder_label`/`section_label` step, no parent→child link".
2. `CHDR-001`'s "no parent" was overstated — `via = "/e/circle"` *is* the
   textual path-parent of `CHILD_NODE`. Corrected to state that the relation is
   inert: `via` never enters the AAD and is read by neither `Wrap::open` nor the
   `Then`.
3. `CHDR-003`'s "would stay green if `check_rotation` were deleted" was false as
   written — deletion breaks compilation at four call sites. Corrected to
   "stays green when its body is neutralised", which is what was measured.

The verifier also found the audit **understated** `CHDR-006` (blast radius is
the whole suite, not one Rule) and surfaced one new finding, `CHDR-016`: the
repository's only explicit key-version-binding negative test
(`c1_header_seal.rs:105-107`) passed *vacuously* under the mutation, because it
asserts `is_err()` on a ciphertext whose baseline openability is established in
a different test function. Version binding is therefore defended by exactly one
byte cross-check and by nothing behavioral — including by the test written to
defend it.

**Disclosed deviation.** `PROCESS.md` forbids the auditor from running
unfiltered Cucumber. That run happened, inside the refutation pass, to measure
a mutation's blast radius. It is reported as a measurement and is not offered
as a regression-gate claim; the corrector still owns the global gates.

## Findings

Sixteen findings, `CHDR-001` … `CHDR-016`, all `OPEN`, detailed in
`docs/audits/features/c-headers.md` §6 with evidence, expected behavior and
closure criteria. `CHDR-015` is `DECISION_REQUIRED`.

Mapping from the frozen Pass A identifiers:

| Public | Pass A origin |
|---|---|
| `CHDR-001` | RU4-a |
| `CHDR-002` | RU4-b |
| `CHDR-003` | RU4-c |
| `CHDR-004` | RU4-d |
| `CHDR-005` | RU4-e |
| `CHDR-006` | RU1-a |
| `CHDR-007` | RU1-b |
| `CHDR-008` | RU1-c |
| `CHDR-009` | RU1-d |
| `CHDR-010` | RU3-a |
| `CHDR-011` | RU3-b |
| `CHDR-012` | RU3-c |
| `CHDR-013` | RU2-a, RU1-e, and RU-4's third empty `Given` |
| `CHDR-014` | RU2-b (absorbing RU2-c and RU2-d as informational notes) |
| `CHDR-015` | RU2-e |
| `CHDR-016` | none — surfaced by the adversarial verification pass |

## Gherkin markers

Audit comments and `@audit-chdr-*` tags were added to the six scenarios
carrying an unresolved `PARTIAL` or `SEMANTIC_FALSE_POSITIVE` finding, and — a
deliberate reading of `PROCESS.md` — also to the two `PROVEN` scenarios that
carry an actionable open finding (`CHDR-009`/`013` on scenario 1,
`CHDR-013`/`014` on scenario 5). The marker text states the status in both
cases, so a `PROVEN` scenario is not mislabelled. The alternative — recording
those three findings only in the public audit — would hide actionable work from
anyone reading the executable contract.

Every marker is to be removed when its finding is independently `VERIFIED`.

## Findings not handled

None deferred. The audit covered all eight scenarios in its assigned scope.

Deliberately **not** reported as findings, per the pilot's "absence of a
scenario is out of scope" rule:

- retention of old key versions after rotation (spec §3.5) — no scenario claims
  it;
- the smuggled-recipient rejection of `check_rotation` — no scenario claims it
  (`CHDR-003` is about the scenario that *does* claim the cut not reaching the
  function, which is different);
- rejection of an up-link wrap posted by a non-holder of the parent — claimed
  and exercised by `g-revocation`, not here;
- `append_line`'s `no key version` error branch;
- header hygiene (spec §3.6).

## Cross-domain observations for the impact review

Not findings of this feature; recorded so they are not lost.

1. **`grants.rs:289` appends at the hardcoded `KV = 1`** (`bundle.rs:25`,
   commented "single key version until step G (revocation rotates)") while
   `grants.rs:460` and `session.rs:364` use `latest_version()`/`open_latest`.
   Because §3.5 retains old versions, after a rotation to v2 `add_line_on` would
   still find `"1"` and append there, handing a new grantee the superseded key
   rather than `key_versions[current]`. Belongs to the grants/revocation
   domains.
2. **`vault.rs:347-410` rotates without posting an up-link wrap**, unlike
   `revoke.rs:203-213`. Plausibly by design — a vault-config node has no derived
   parent in the content tree — but it should be stated somewhere. Belongs to
   `o-connector-classes-vault`.
3. **`Header::open` derives its AAD `node` from the document's own `self.node`
   field**, not from the store path. Only `vault.rs:115` validates that field
   against an expected node. Whether a whole header relocated to another node's
   path is rejected is therefore decided outside `aithos-core` and is untested
   here.
4. **I3 is a label-presence check.** `check_owner_line` and `validate` test
   `to == "owner"`, while spec §3.1 says `to` is a routing hint and "the seal is
   what grants". A header whose only owner line seals to an attacker's key
   satisfies both. No scenario claims otherwise, so this is context rather than
   a finding — but it bears on how `CHDR-015` is decided.
5. **Under `--tags @c-headers` the CLI tag expression replaces the `@wip`
   closure entirely** (cucumber 0.21.1). The tagged gate would run a `@wip`
   scenario of this feature; none exists today, but the tagged and unfiltered
   runs are not equivalent filters. Relevant to `BDER-011`'s remediation.

## Limits of this conclusion

- The audit establishes what the eight scenarios prove, not whether the header
  implementation is correct beyond what they touch. Every trace that reached
  `aithos-core` found it faithful to spec §03, but that is a by-product of
  tracing the scenarios, not an audit of `header.rs`.
- Mutation experiments were run for `CHDR-003` and `CHDR-006` only, during the
  adversarial verification pass. Every other statement of the form "a regression
  that dropped X would leave the scenario green" is derived from reading the
  assertions and the control flow, not from a reproduced RED run. The
  corrector's RED tests will supply that proof; the "Expected RED" column of the
  implementation plan states what each must demonstrate.
- Step-phrase ambiguity is excluded by inspection of every `regex =` / `expr =`
  step, not by execution; cucumber reports ambiguity only at runtime, and the
  gate ran clean.
- Surface inspection enumerated the header call sites in `aithos-bundle`,
  `aithos-cli` and `aithos-gateway` and checked them for bypass of the audited
  invariants. It did not audit those surfaces, and it does not prove that no
  other path validates a header before those consumers are reached.
- The gate was run against an extract in a container rather than in the
  worktree, for the toolchain reason disclosed above.

## Next action

Set `STATE.md` to `CORRECTION_REQUESTED` and route to
`.agents/c-headers/corrector/correct-c-headers/SKILL.md`, assigning lots 1 and
2 first (`CHDR-001`, `CHDR-005`, `CHDR-013` for scenario 8; `CHDR-002`,
`CHDR-003`, `CHDR-004` for scenario 7).

`CHDR-015` is `DECISION_REQUIRED` and belongs to the human protocol owner. It
does not block the correction lots, none of which touches `Bundle::verify`.

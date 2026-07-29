# Conclusion — initial audit of `b-derivation.feature`

| Field | Value |
|---|---|
| Run type | initial audit, native (not reconstructed) |
| Role | semantic feature auditor (`audit-b-derivation`) |
| Date | 2026-07-29 |
| Observed revision | `891c808`, branch `codex/gherkin-agent-pilot` |
| Worktree state | clean at run start |
| Public audit | `docs/audits/features/b-derivation.md` |
| Result | `CORRECTION_REQUESTED` |

## Scope and review units

`features/b-derivation.feature` only — 3 Rules, 6 scenarios. Semantic truth of
existing scenarios. No search for missing scenarios. No production change.

| Unit | Rule | Scenarios |
|---|---|---|
| U1 | Derivation is deterministic and per-segment | 2 |
| U2 | Holding a folder yields its subtree, nothing else | 3 |
| U3 | Tag views anchor at folders | 1 |

## Pass A — history-blind

Each unit ran in a **fresh agent with no shared context**, against a
source-only export of `891c808` produced by `git archive HEAD`. That export
contains **no `.git` directory**, so history-blindness was structural rather
than declarative: the units could not have read history had they tried. Each
unit was given the contract, the step definitions, the production code, the
specification, and the vectors, and was forbidden `docs/audits/**` and
`features/.agents/a-identity/**`.

Frozen Pass A verdicts, before any historical material:

| Scenario | Frozen Pass A verdict |
|---|---|
| The same path always yields the same key | `SEMANTIC_FALSE_POSITIVE` |
| Sibling nodes get unrelated keys | `SEMANTIC_FALSE_POSITIVE` |
| A folder holder derives every descendant | `PARTIAL` |
| A folder holder cannot reach sideways | `PARTIAL` |
| Renaming never re-keys | `SEMANTIC_FALSE_POSITIVE` |
| A folder-local tag view is its own lock | `PARTIAL` |

### Contamination

Two disclosures, both material and both recorded rather than smoothed over.

1. **`DOMAIN.md` was absent from the Pass A export.** The source export was
   taken before the `b-derivation` domain was scaffolded, so all three units
   report that `features/.agents/b-derivation/DOMAIN.md` and `STATE.md` did not
   exist. They fell back to `spec/01-identity-and-keys.md` §1.3 and
   `spec/02-content-tree.md` §2.2/§2.5/§2.9 as the normative contract. This is
   a *deficit* of curated input, not a contamination: no forbidden material
   reached them. The invariants they derived independently from the spec match
   the ten invariants later written into `DOMAIN.md`. The integration pass and
   the challenger both ran with `DOMAIN.md` present.
2. **The integration pass leaked directory names.** An orientation
   `ls -R features/.agents/` printed the file *names* under
   `a-identity/`, including its run-report filenames. No content was opened and
   no finding derives from them. The agent flagged this at the time.

## Integration pass — shared state and surfaces

Run separately and last, as the process requires.

- **World lifecycle.** A fresh `ProtocolWorld` per scenario, proven three ways:
  the absence of `#[world(init)]` in `cucumber.rs`, the cucumber-rs
  `run_scenario` construction path, and — decisively — the fact that scenario 2
  asserts `!=` on the same indices where scenario 1 asserts `==`, so any
  carry-over would make the suite red.
- **`OnceLock` / caches.** Eight `OnceLock` acceptance verdicts exist at
  `cucumber.rs:972-979`, consumed by eight regex-alternation steps. They are a
  live `PROXY` surface — and it lies entirely in the CB4/CB5/CB6/CB7/CB10
  families of other features. **No b-derivation scenario touches it.** The
  decisive negative proof is that cucumber-rs raises `AmbiguousMatch` on
  overlapping steps and the suite is green, so no CB* regex also matches a
  b-derivation phrase. `aithos-core::derive` has zero statics and zero caching.
- **Step ownership.** All 18 Gherkin phrases and all 4 World fields used by
  this feature are exclusive to it. Zero cross-feature reach for any correction
  that stays inside the steps, the fixtures, or the feature file.
- **Production surfaces.** `folder_label`, `section_label` and `tag_label` have
  exactly one call site each in the whole workspace — inside `node_key` itself.
  Every content-tree key in every crate flows through `node_key`. **No bypass,
  no parallel derivation, no contradiction of the invariants.**
- **Vector integrity.** `folder1_key_hex` is corroborated by five Python
  generators, `deep_section_key_hex` by one; `sibling_section_key_hex` and both
  tag anchors by none. The `t/<tag>` label has no implementation anywhere
  outside `derive.rs:54`. Recorded as BDER-007.

## Challenger review

The three `SEMANTIC_FALSE_POSITIVE` verdicts were handed to an independent
adversarial reviewer instructed to refute them. It mutated `node_key` in a
throwaway copy of the workspace and measured per-scenario kill rates.

- All three verdicts **UPHELD**.
- **BDER-002's evidence was replaced.** The Pass A unit rested it on a mutant
  copying 31 zone-key bytes — not a credible regression, and loudly caught
  elsewhere. The challenger replaced it with M5, a per-segment
  `parent XOR blake3(label)` step: a plausible "cheaper KDF" refactor that
  destroys one-wayness entirely while **813 of 815 BDD scenarios stay green**.
- **BDER-003 was escalated from `PARTIAL` to `SEMANTIC_FALSE_POSITIVE`.** Under
  M5 the `Then` sentence "no derivation from it yields the second folder's
  section key" is demonstrably false while the scenario is green: the folder-1
  holder recovers the zone key by one XOR and reaches the sibling section key
  exactly. This is current-code, reproducible-test evidence, so the escalation
  is admissible under the evidence model.
- **BDER-001's justification was reworded.** "Tautology `f(x) == f(x)`" is
  rebuttable — a constant function is also deterministic, so no mutant makes
  the `Then` sentence false. Replaced by the measured 0/5 kill rate and the
  reused-`NodePath` observation.
- **A mitigation was added to every finding**, absent from Pass A: no protocol
  invariant here is unguarded. The vectors are 5/5 effective and the rename
  invariant is proven end-to-end by `d-bundle.feature:38-41`. The public audit
  states this prominently so it is not read as "derivation is untested".

## Pass B — historical and differential

Run on the real repository, after every Pass A verdict was frozen.

Only two commits have ever touched this feature:

```text
53d0751  2026-07-09 10:45  feature first: content-tree derivation contract (B, @wip)
1b7d258  2026-07-09 11:58  step B complete: content-tree derivation (B2)
```

- The six scenarios were **never stronger and later weakened**. `53d0751`
  introduced them tagged `@wip`; `1b7d258`, 73 minutes later, implemented the
  steps and removed the tags. The assertions have had their present shape since
  the day they were written.
- `e3608b8` (2026-07-27) touched the step block, but the diff is a pure
  relocation — identical added and removed lines. No semantic change.
- `derive.rs`, `b2_derivation.rs` and `vectors/b2-derivation.json` have not
  been modified since `1b7d258`.
- **The commit message documents the circularity of BDER-004 verbatim:**
  *"rename-never-rekeys locked at the API level (names are not inputs)"*. The
  no-op `When` was deliberate, and the recorded justification is exactly the
  circular one Pass A identified. Historical intent corroborates the verdict
  instead of challenging it.
- The same message claims the B2 vector was *"generated independently (Python
  blake3)"*. `git log --all --diff-filter=AD -- 'vectors/gen-b*'` is empty:
  **no B2 generator has ever existed in this repository.** The vector, the
  Gherkin fixtures and the conformance test were all created in that one
  commit, by one author, the same morning. Recorded as BDER-008.
- The message also records the state at the time: *"6 scenarios implemented and
  untagged: 15 scenarios / 51 steps green"*. The suite is now 815 scenarios.
  This feature has ridden green through 800 added scenarios without ever being
  strengthened.
- `1b7d258` added the CLI verb `node-key … --zone-dk-hex` *"for manual
  determinism checks"* — the author wanted a manual check outside the
  scenarios, which is itself a contemporaneous signal about how much the
  scenarios were trusted to prove.

## Reconciliation

Pass B produced **no new current-code or reproducible-test evidence** that
would upgrade any scenario. It corroborates Pass A on all six and closes the
provenance question behind BDER-008.

| Scenario | Pass A (frozen) | Reconciled | Why it changed |
|---|---|---|---|
| The same path always yields the same key | `SEMANTIC_FALSE_POSITIVE` | `SEMANTIC_FALSE_POSITIVE` | unchanged; justification reworded after challenge |
| Sibling nodes get unrelated keys | `SEMANTIC_FALSE_POSITIVE` | `SEMANTIC_FALSE_POSITIVE` | unchanged; evidence replaced by M5 |
| A folder holder derives every descendant | `PARTIAL` | `PARTIAL` | unchanged; gap narrowed from "future" to "one shape" |
| A folder holder cannot reach sideways | `PARTIAL` | `SEMANTIC_FALSE_POSITIVE` | **escalated** on challenger's M5 probe |
| Renaming never re-keys | `SEMANTIC_FALSE_POSITIVE` | `SEMANTIC_FALSE_POSITIVE` | unchanged; Pass B corroborates via commit message |
| A folder-local tag view is its own lock | `PARTIAL` | `PARTIAL` | unchanged; scope question raised to `DECISION_REQUIRED` |

No unresolved contradiction between passes. No review unit is contaminated in a
way that invalidates its verdict.

## Commands and results

```text
cargo test -p aithos-bundle --test cucumber
18 features / 112 rules / 815 scenarios (815 passed) / 3505 steps (3505 passed)
b-derivation block: 1 feature, 3 rules, 6 scenarios, 21 steps, all green

cargo test -p aithos-core --test b2_derivation
2 passed

cargo test --workspace --no-fail-fast
green at baseline (run by the challenger)

Mutant kill rates on node_key (throwaway copy, never the repository):
  M1 constant           b2 FAIL,  9/815 BDD scenarios fail
  M2 ignore path        b2 FAIL,  7/815
  M3 monolithic hash    b2 FAIL, 71/815
  M4 31 zone bytes      b2 FAIL, 71/815
  M5 XOR step           b2 FAIL,  2/815   <- one-wayness fully destroyed
```

Exhaustive enumeration of 13 332 labelled derivations reachable from folder 1's
key: 0 hits on the sibling section key under the real implementation. The
scenario explores 3.

## Findings

| Id | Verdict | Round 1 |
|---|---|---|
| `BDER-001` | `SEMANTIC_FALSE_POSITIVE` | assigned |
| `BDER-002` | `SEMANTIC_FALSE_POSITIVE` | assigned |
| `BDER-003` | `SEMANTIC_FALSE_POSITIVE` | assigned — most serious |
| `BDER-004` | `SEMANTIC_FALSE_POSITIVE` | assigned |
| `BDER-005` | `PARTIAL` | assigned |
| `BDER-009` | latent fragility | assigned |
| `BDER-006` | `DECISION_REQUIRED` | **not** assigned |
| `BDER-007` | open | not assigned |
| `BDER-008` | open | not assigned |
| `BDER-010` | informational | not assigned |

## Affected files and symbols

Written by this run: `docs/audits/features/b-derivation.md`,
`docs/audits/features/README.md` (index row), `features/b-derivation.feature`
(markers only, no scenario text changed), this report,
`features/.agents/b-derivation/STATE.md`.

Named for the correction, not modified here:
`rust/crates/aithos-bundle/tests/cucumber.rs` (steps at `:7332-7365`,
`:7673-7726`, `:11629-11685`; honest rename step at `:7892`),
`vectors/b2-derivation.json` (values frozen; provenance only).

**No Rust file was modified by this audit.**

## Limits of this conclusion

- Pass A ran without `DOMAIN.md`, as disclosed above. A future round starting
  from the now-present domain file may frame the same evidence differently.
- The mutant set is five points in an infinite space. "0 of 5" and "2 of 5" are
  evidence of weakness, not a calibrated mutation score.
- The universal negative in BDER-003 is not testable in principle — it rests on
  BLAKE3 one-wayness. The 13 332-path enumeration bounds only the label space
  the code itself can build.
- Workspace failure counts under M1-M4 were inferred from the vector tests that
  necessarily fail; only baseline and M5 were measured with a full workspace run.
- The neighbouring surfaces (`c-headers`, move/rotation, the tag-view wrap path)
  were inspected for bypasses, not audited. The claim "no invariant is
  unguarded" is bounded to the six scenarios and the surfaces actually read.

## Next action

Run `correct-b-derivation` for `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`,
`BDER-005` and `BDER-009`.

Do not touch `BDER-006` before its scope decision, and do not modify
`derive.rs`: no finding in this note warrants a production change, and any
behavioral change there is `FULL_AUDIT` for the 17 other features.

Then request an independent review from `audit-b-derivation`.

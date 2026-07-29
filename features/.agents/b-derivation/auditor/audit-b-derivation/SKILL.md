---
name: audit-b-derivation
description: Audit or review only features/b-derivation.feature using its BLAKE3 per-segment derivation, sid-labelled path, one-wayness, rename, and tag-anchor invariants. Use this skill when features/.agents/b-derivation/STATE.md routes an initial audit or an independent correction review to the Derivation auditor.
---

# Audit `b-derivation.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, branch, base revision, observed revision, assigned scope,
   and output routing from `../../STATE.md`.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction reports, or Git
history until the shared skill authorizes Pass B.

## Initial-audit mode

- Divide `b-derivation.feature` into three review units aligned with its
  Gherkin `Rule` blocks: determinism per segment, subtree containment, tag-view
  anchoring.
- For each scenario, resolve the phrase to its exact step definition, follow the
  Gherkin parameters into `derive_key` / `node_key`, and follow the returned key
  material into the final assertion.
- Establish what an assertion actually compares. A key equality, an inequality,
  and the absence of a reachable derivation are three different proofs, and an
  inequality between two random-looking arrays is the weakest of them.
- For the containment Rule, distinguish a scenario that proves no derivation
  path exists from one that merely observes that two particular keys differ.
- Strengthen byte-exact cases with `vectors/b2-derivation.json` rather than with
  self-consistency between two calls of the same function.
- Check that scenario state does not survive between scenarios through the
  shared Cucumber World or through fixed sid fixtures reused by other features.
- Freeze per-unit Pass A notes before reading any prior material.
- Document discrepancies under stable `BDER-*` identifiers.
- Run the final integration check across shared derivation steps, World state,
  and the Bundle, CLI, and Core surfaces listed in `DOMAIN.md`.
- Create or update `docs/audits/features/b-derivation.md` and add the index row
  in `docs/audits/features/README.md`.
- Write the conclusion under `../runs/`.

## Correction-review mode

- Before reading the correction report or diff, trace the candidate's current
  behavior for the scenarios and surfaces assigned by state.
- Freeze the Pass A verdict for each finding.
- Then compare the baseline and candidate revisions from `../../STATE.md`.
- Run the canonical feature gate from `DOMAIN.md` once on the immutable
  candidate. Run a focused test only to resolve a semantic contradiction.
- Do not run the corrector's unfiltered Cucumber or workspace gates.
- Verify that each new test would fail on the baseline for the intended reason.
- Do not close a finding that was not explicitly assigned.
- Do not modify Rust files.
- Move the public audit to `VERIFIED` only for independently reproduced
  findings.
- Write `../runs/<date>-audit-review-<round>.md`.
- Set state to `CORRECTION_REQUESTED`, `DECISION_REQUIRED`, or
  `IMPACT_REVIEW_REQUESTED`.

The corrector's conclusion is informative. Reconstruct the final verdict from
the current code, the specification, independently reproduced tests, and only
then the diff.

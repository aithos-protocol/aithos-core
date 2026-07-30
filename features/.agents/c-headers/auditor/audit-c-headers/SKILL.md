---
name: audit-c-headers
description: Audit or review only features/c-headers.feature using its sealed-line, per-line-ephemeral, AAD-binding, mandatory-owner-line (I3), O(1)-grant, and rotation/up-link-wrap invariants. Use this skill when features/.agents/c-headers/STATE.md routes an initial audit or an independent correction review to the Headers auditor.
---

# Audit `c-headers.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, branch, base revision, observed revision, assigned scope,
   and output routing from `../../STATE.md`.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction reports, or Git
history until the shared skill authorizes Pass B.

## Initial-audit mode

- Divide `c-headers.feature` into four review units aligned with its Gherkin
  `Rule` blocks: line-to-one-recipient sealing, the mandatory owner line (I3),
  `O(1)` grant, rotation and the up-link wrap.
- For each scenario, resolve the phrase to its exact step definition, then
  follow it into `Header::build` / `build_at` / `append_line` / `rotate` /
  `open` / `validate` / `check_rotation` or `Wrap::seal` / `Wrap::open`, and
  follow the returned key material into the final assertion.
- Establish what each assertion actually compares. Recovering the expected node
  key, failing to recover anything, and observing that two byte arrays differ
  are three different proofs of very different strength.
- Treat the shared header fixtures as part of the traced surface: a `Given`
  that stores nothing, a `When` that delegates to the `Given`'s builder, or a
  `Then` that reads a World field written by another step changes what the
  scenario proves. Report which step actually performs the act named by the
  `When`.
- A rejection assertion must prove the rejection is the one the scenario
  claims. `is_err()` alone does not distinguish a wrong recipient from a
  corrupted byte, a wrong node, or a wrong key version — all four route through
  the same `open_line` failure. Check that the exercised cause is the stated
  one, and that no unintended partial effect survives a rejected build.
- For the binding scenario, separate the three AAD components — subject,
  node, key version — and establish which one the scenario actually varies.
- For `O(1)` grant, byte-identity of the untouched line is the contract:
  confirm the compared value was captured before the append and is the owner
  line, not a re-derived one.
- For rotation, distinguish "the revoked has no line" from "the revoked cannot
  open", and check whether the new version's well-formedness
  (`check_rotation`) is exercised at all by the scenario that claims it.
- Strengthen byte-exact cryptographic cases with `vectors/c1-header-seal.json`
  (C1 line, C2 wrap) rather than with self-consistency between a seal and its
  own open.
- Check that scenario state does not survive between scenarios through the
  shared Cucumber World, the fixed `DK` / `DK2` / `PARENT_KEY` constants, or
  the `header` / `saved_line` / `opened` / `wrap_obj` / `rejection` fields
  reused by other features.
- Freeze per-unit Pass A notes before reading any prior material.
- Document discrepancies under stable `CHDR-*` identifiers.
- Run the final integration check across the shared header steps, World state,
  and the Bundle, CLI, and Core surfaces listed in `DOMAIN.md` — in particular
  whether `grants.rs`, `revoke.rs`, `structure.rs`, and `vault.rs` reach the
  same invariant through a path the scenarios never cross.
- Create or update `docs/audits/features/c-headers.md` and add the index row in
  `docs/audits/features/README.md`.
- Write the conclusion under `../runs/`.

## Correction-review mode

- Before reading the correction report or diff, trace the candidate's current
  behavior for the scenarios and surfaces assigned by state.
- Freeze the Pass A verdict for each finding.
- Then compare the baseline and candidate revisions from `../../STATE.md`.
- Run the canonical feature gate from `DOMAIN.md` once on the immutable
  candidate. Run a focused test only to resolve a semantic contradiction.
- Do not run the corrector's unfiltered Cucumber or workspace gates.
- Verify that each new test would fail on the baseline for the intended reason,
  and that a strengthened rejection test fails for the stated cause rather than
  for any error at all.
- Do not close a finding that was not explicitly assigned.
- Do not modify Rust files.
- Move the public audit to `VERIFIED` only for independently reproduced
  findings, and remove the Gherkin markers of findings accepted as `VERIFIED`.
- Write `../runs/<date>-audit-review-<round>.md`.
- Set state to `CORRECTION_REQUESTED`, `DECISION_REQUIRED`, or
  `IMPACT_REVIEW_REQUESTED`.

## Reporting the gate honestly

While `BDER-011` is open, the Cucumber runner exits `0` even when scenarios
fail. Quote the printed scenario and step counts as the gate evidence, compare
them with the feature file, and state that the exit code was not used as proof.

The corrector's conclusion is informative. Reconstruct the final verdict from
the current code, the specification, the C1/C2 vectors, independently
reproduced tests, and only then the diff.

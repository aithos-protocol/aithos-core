---
name: audit-a-identity
description: Audit or review only features/a-identity.feature using its genesis, DID, succession, and epoch-transition invariants. Use this skill when features/.agents/a-identity/STATE.md routes an initial audit or independent correction review to the Identity auditor.
---

# Audit `a-identity.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, revision, assigned scope, and output routing from
   `../../STATE.md`.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction report, or Git
history until the shared skill authorizes Pass B.

## Initial-audit mode

- Divide `a-identity.feature` into review units aligned with its Gherkin
  `Rule` blocks; split a Rule further if it exceeds six materially different
  scenarios.
- Trace all scenarios through the Identity invariants in `DOMAIN.md`.
- Freeze per-unit Pass A notes before reading any prior audit material.
- Document discrepancies under stable `AID-*` identifiers.
- Run the final integration check across shared Identity steps and state.
- Write the conclusion under `../runs/`.

## Correction-review mode

- Before reading the correction report or diff, trace the candidate's current
  behavior for the scenarios and surfaces assigned to AID-001, AID-002, and
  AID-005.
- Freeze the Pass A verdict for each finding.
- Then compare the baseline and candidate revisions from `STATE.md`.
- Rerun the applicable gates from `DOMAIN.md`.
- Check the relevant Bundle, WASM/client, Gateway, and Provider paths.
- Do not close or correct AID-003 or AID-004.
- Do not modify Rust files.
- Move the public audit to `VERIFIED` only for independently reproduced
  findings.
- Write `../runs/<date>-audit-review-<round>.md`.
- Set state to `CORRECTION_REQUESTED`, `DECISION_REQUIRED`, or
  `IMPACT_REVIEW_REQUESTED`.

The corrector's conclusion is informative. Reconstruct the final verdict from
the current code, protocol, independently reproduced tests, and only then the
diff.

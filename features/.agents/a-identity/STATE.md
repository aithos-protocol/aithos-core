# Domain state — `a-identity`

| Field | Value |
|---|---|
| Status | `REVIEW_REQUESTED` |
| Expected mode | `review` |
| Round | 1 |
| Audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Correction commit | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Corrector branch | `fix/aid-001-002-005-identity-fail-closed` |
| Candidate findings | `AID-001`, `AID-002`, `AID-005` |
| Findings outside correction | `AID-003`, `AID-004` |
| Next role | `audit-a-identity` |
| Expected conclusion | `auditor/runs/<date>-audit-review-01.md` |

## Inputs

- public audit: `docs/audits/features/a-identity.md`;
- reconstructed initial audit:
  `auditor/runs/2026-07-29-audit-initial-reconstructed.md`;
- reconstructed correction:
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.

## Current instruction

Review the correction commit against the baseline. Do not implement changes.
Reproduce the evidence, accept or reject each finding independently, then
update this state.

Commit `56436f3` is an immutable input. Any later modification must produce a
new commit and a new round.

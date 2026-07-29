# Domain state — `a-identity`

| Field | Value |
|---|---|
| Status | `DECISION_REQUIRED` |
| Expected mode | `manual protocol decision` |
| Round | 2 |
| Initial audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Reviewed commit | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Round 2 correction baseline | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Reviewed branch | `fix/aid-001-002-005-identity-fail-closed` |
| Verified findings | `AID-002`, `AID-005` (pilot scope) |
| Finding awaiting decision | `AID-001` |
| Findings outside correction | `AID-003`, `AID-004` |
| Blocking prerequisite | Provider `did.json` replacement semantics |
| Next role | protocol owner |
| Expected conclusion | Provider decision, then `corrector/runs/<date>-correction-02.md` if correction is required |

## Inputs

- public audit: `docs/audits/features/a-identity.md`;
- reconstructed initial audit:
  `auditor/runs/2026-07-29-audit-initial-reconstructed.md`;
- reconstructed correction:
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`;
- independent round 1 review:
  `auditor/runs/2026-07-29-audit-review-01.md`.

## Current instruction

First obtain an explicit protocol decision for AID-001:

- decide whether Provider same-DID `did.json` replacement remains a
  Provider-specific succession operation or adopts the §10.4 epoch
  transition;
- start correction round 2 only after that decision, and only to implement
  the selected semantics;
- leave AID-002 and AID-005 unchanged; they are now `VERIFIED` within the
  pilot scope;
- do not correct AID-003 or AID-004 in this round.

Commit `56436f3` remains an immutable input and becomes the baseline for round
2. Any correction must produce a new commit.

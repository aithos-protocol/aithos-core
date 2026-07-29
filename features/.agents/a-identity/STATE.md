# Domain state — `a-identity`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` |
| Expected mode | `correction` |
| Round | 2 |
| Initial audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Reviewed round 1 candidate | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Round 2 protocol baseline | `083c1a197a39f7a8efde957eddf5af05b825e3ea` |
| Protocol decision | `decisions/2026-07-29-aid-001-provider-epoch-transition.md` |
| Assigned finding | `AID-001` Provider remainder only |
| Findings already verified | `AID-002`, `AID-005` (pilot scope) |
| Findings outside correction | `AID-003`, `AID-004` |
| Next role | `correct-a-identity` |
| Expected conclusion | `corrector/runs/2026-07-29-correction-02.md` |

## Inputs

- public audit: `docs/audits/features/a-identity.md`;
- binding protocol decision:
  `decisions/2026-07-29-aid-001-provider-epoch-transition.md`;
- independent round 1 review:
  `auditor/runs/2026-07-29-audit-review-01.md`;
- reconstructed round 1 correction:
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.

## Required round 2 outcome

Implement only the decided Provider semantics for AID-001:

1. a DID document remains root-signed and must pass strict Core verification;
2. succession signs only a separate `EpochTransition`;
3. a new root creates a distinct successor DID;
4. same-DID succession-signed `did.json` replacement is refused;
5. Provider reuses Core triplet verification for epoch acceptance;
6. every refusal proves the absence of partial persistent effects;
7. P9 vectors and scenarios express the decided behavior.

If a canonical public storage/transport path for the transition cannot be
derived from the current protocol without inventing new wire semantics, remove
the invalid same-DID path, keep the behavior fail-closed, and report the
remaining transport decision explicitly.

Do not change AID-002/AID-005 and do not address AID-003/AID-004.

## Handoff requirement

The corrector must record its immutable starting commit before its first code
change, produce a distinct candidate commit, mark AID-001 at most
`IMPLEMENTED`, set this state to `REVIEW_REQUESTED`, and request a fresh
two-pass review from `audit-a-identity`.

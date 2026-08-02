# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` |
| Expected mode | `correction` (round 2) |
| Round | 2 (open) — round 1 closed and integrated |
| Round 1 integration | accepted by the human owner on 2026-08-02; audit branch content on `main` (`3d6fa51`, `ae88f7f`, `1ab331a`, impact review `7854895`) |
| Impact review | `orchestrator/runs/2026-07-29-b-derivation-impact-review.md` — accepted 2026-08-02, no `FULL_AUDIT`, one `TARGETED` (`d-bundle`) |
| `BDER-011` | closed — cross-cutting lot done, `VERIFIED` 2026-07-30 (`78c06ba`, `090d11a`, `c630753`) |
| Decisions recorded | `decisions/2026-08-02-bder-006-tag-view-rule-scope.md` (option A + mandatory `d-bundle` extension) ; `decisions/2026-08-02-bder-008-b2-provenance.md` (honest provenance claim, generator deferred) |
| Findings `VERIFIED` | `BDER-001`…`BDER-005`, `BDER-009`, `BDER-011` |
| Findings assigned to round 2 | `BDER-006` (retitle the tag-view `Rule`, per decision A), `BDER-008` (rewrite the B2 vector `description`, values frozen) |
| Findings open, not assigned | `BDER-007` (closes only via the future independent B2 generator lot), `BDER-010` (informative — doc comment on `node_key` only), `BDER-012` (bounded negatives; future round) |
| Next role | corrector — branch `codex/fix-b-derivation-bder-006-008-decisions` from current `main` |
| After correction | independent review (auditor), then impact review per `PROCESS.md` |

## Inputs

- decisions: `decisions/2026-08-02-*.md` (the corrector reads only the assigned findings and their decisions);
- public audit: `docs/audits/features/b-derivation.md`;
- round 1 review: `auditor/runs/2026-07-29-audit-review-01.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Round 2 scope is exactly the two decided corrections:

1. **BDER-006 (decision A):** reformulate the title of the `Rule`
   « Tag views anchor at folders » so it promises derivation only, not the
   §02.9 anchoring semantics. No behavioral change. The `@audit-partial
   @bder-006` marker is removed only by the review that verifies the round.
   The mandatory counterpart — tag-view/`wrap` scenarios — belongs to the
   `d-bundle` targeted follow-up, not to this round.
2. **BDER-008 (decision):** rewrite the `description` of
   `vectors/b2-derivation.json` to state the real provenance and the exact
   corroboration status of each field. **No value changes.**

Out of scope for round 2: `BDER-007` (future generator lot), `BDER-010`
(informative), `BDER-012` (future round). Do not touch `d-bundle.feature`.

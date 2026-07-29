# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` |
| Expected mode | `correction` |
| Round | 1 |
| Audit baseline | `891c808` (branch `codex/gherkin-agent-pilot`, clean worktree) |
| Correction commit | none yet |
| Corrector branch | to create, e.g. `fix/bder-001-005-derivation-honest-assertions` |
| Assigned findings | `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`, `BDER-005`, `BDER-009` |
| Findings outside correction | `BDER-006` (decision), `BDER-007`, `BDER-008`, `BDER-010` |
| Next role | `correct-b-derivation` |
| Expected conclusion | `corrector/runs/<date>-correction-01.md` |

## Inputs

- public audit: `docs/audits/features/b-derivation.md`;
- initial audit: `auditor/runs/2026-07-29-audit-initial.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Implement only the six assigned findings. Every one of them is a defect in what
the scenarios prove, not in `aithos-core::derive` — **do not modify
`derive.rs`**; the audit found no production defect, every label function has a
single call site inside `node_key`, and any behavioural change there is
`FULL_AUDIT` for the 17 other features.

Demonstrate each defect with a RED test first. The reference adversarial probe
is a per-segment `parent XOR blake3(label)` implementation of `node_key`, under
which one-wayness is entirely destroyed while 813 of 815 BDD scenarios stay
green. A correction is only credible if that mutant makes the corrected
scenarios fail.

Do not touch `BDER-006`: its closure requires a scope decision between a pure
derivation Rule and a Rule covering the wrap bridge. A corrector must not make
that choice implicitly.

Preserve `vectors/b2-derivation.json` values byte for byte.

`891c808` is an immutable input. Any later modification must produce a new
commit and a new round.

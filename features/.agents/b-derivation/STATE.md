# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` |
| Expected mode | `correction` |
| Round | 1 |
| Canonical audit branch | `codex/audit-b-derivation` |
| Initial audit baseline | `891c808` (historical branch `codex/gherkin-agent-pilot`, clean worktree) |
| Current `main` integration baseline | `5c3a61852dee0886fb6fff008a6304e8ea2c71bb` |
| Rebased audit record | `9c3c9bc` |
| Integration assessment | A-Identity impact review classified `b-derivation` as `NONE`; no audited Derivation behavior changed |
| Correction commit | none yet |
| Corrector branch | create `codex/fix-b-derivation-bder-001-005-honest-assertions` from the current canonical audit-branch HEAD and record its exact baseline before changing files |
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

`891c808` remains the immutable revision against which the initial semantic
evidence was collected. The audit record was replayed onto the accepted
A-Identity `main` integration baseline only after the A-Identity impact review
classified `b-derivation` as `NONE`. This preserves the original evidence
instead of relabeling the newer tree as freshly audited.

Before the first correction, resolve and record the exact current
`codex/audit-b-derivation` HEAD as the correction baseline. Any relevant
behavioral change after that point requires a new audit round.

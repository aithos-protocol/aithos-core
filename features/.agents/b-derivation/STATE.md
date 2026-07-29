# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `IMPACT_REVIEW_REQUESTED` |
| Expected mode | `impact review` |
| Round | 1 (closed) |
| Canonical audit branch | `codex/audit-b-derivation` |
| Initial audit baseline | `891c808` (historical branch `codex/gherkin-agent-pilot`, clean worktree) |
| Current `main` integration baseline | `5c3a61852dee0886fb6fff008a6304e8ea2c71bb` |
| Rebased audit record | `9c3c9bc` |
| Correction baseline (frozen, immutable) | `fa8fa797b897a762a0dfd7fc20910f053ce349ed` |
| Correction commit | `3d6fa51aaf9049e0deb81873242103c49f86de08` |
| Reviewed candidate | `1ab331a6c8806cd9c2e7845a452501c60d9dd72c` |
| Corrector branch | `codex/fix-b-derivation-bder-001-005-honest-assertions` |
| Review branch | `codex/review-b-derivation` |
| Correction run | `corrector/runs/2026-07-29-correction-01.md` |
| Review run | `auditor/runs/2026-07-29-audit-review-01.md` |
| Findings `VERIFIED` | `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`, `BDER-005`, `BDER-009` |
| Findings opened by the review | `BDER-011` (P1, repo-wide, pre-existing), `BDER-012` (P3) |
| Findings still open | `BDER-006` (decision), `BDER-007`, `BDER-008`, `BDER-010`, `BDER-011`, `BDER-012` |
| Next role | `review-gherkin-impacts` (orchestrator) |
| Expected conclusion | `orchestrator/runs/<date>-b-derivation-impact-review.md` |

## Inputs

- public audit: `docs/audits/features/b-derivation.md`;
- initial audit: `auditor/runs/2026-07-29-audit-initial.md`;
- correction: `corrector/runs/2026-07-29-correction-01.md`;
- review: `auditor/runs/2026-07-29-audit-review-01.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Run the impact review over `fa8fa79..1ab331a`. The changed surface is confined
to `rust/crates/aithos-bundle/tests/cucumber.rs` and
`features/b-derivation.feature`; no production file and no vector changed, which
the review verified byte for byte rather than taking on report.

Two inputs must not be lost:

1. **`BDER-011` comes first.** The `aithos-bundle` Cucumber runner calls
   `filter_run` instead of `filter_run_and_exit` under `harness = false`, so it
   exits 0 even when scenarios fail — observed three times during the review.
   This is pre-existing at `fa8fa79`, affects all 18 features, and makes every
   exit-code-based gate claim in this pilot — including the corrector's global
   Cucumber and workspace gates, and CI — non-evidence. Scope its remediation
   before any further round claims a green gate as proof. Fixing it may turn
   currently "green" scenarios red in other features; that is the point of
   scoping it here rather than inside one feature's correction.
2. The four step phrases now shared with `d-bundle.feature:40-43`
   (`a published bundle with section ... in circle ...`, `the folder ... is
   renamed to ...`, `the edition is republished`, `the owner reads the same
   section at ...`) are the one place where this feature reaches into another's
   step set. They are unmodified reads, deliberately reused on the initial
   audit's instruction, and the review accepted that reuse.

Do not modify or restart any feature during the impact review.

`BDER-006` remains `DECISION_REQUIRED` and belongs to its human owner. It does
not block the impact review. The review added one fact for that decision:
`d-bundle.feature` contains no tag-view or `wrap` scenario, so option A currently
routes half of spec §02.9 to a destination that does not cover it.

Integration into local `main` happens only after the impact review is accepted,
per `PROCESS.md`. Unresolved findings — `BDER-006`, `BDER-007`, `BDER-008`,
`BDER-010`, `BDER-011`, `BDER-012` — survive that integration in the public
audit, in this state, and in the live Gherkin markers.

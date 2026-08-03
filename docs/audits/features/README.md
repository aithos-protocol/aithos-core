# Gherkin feature implementation audits

This directory contains one living audit note per `features/*.feature` file.
The objective is to distinguish precisely between:

1. a scenario that is actually executed;
2. a scenario that calls real production code;
3. a scenario that proves everything its text claims;
4. a capability that remains compliant on the real surfaces that use it.

A green runner does not establish all four levels by itself.

## Convention

- One stable file per feature: `a-identity.md`, `b-derivation.md`, and so on.
- Every audit records its date and observed Git revision.
- The audit describes the observed on-disk state. A dirty worktree is
  disclosed and is never presented as a clean reproducible baseline.
- Every discrepancy receives a stable feature-derived identifier:
  `AID-001`, `BDER-001`, `CHDR-001`, and so on.
- Findings are not deleted after correction. Their state moves from `OPEN` to
  `IMPLEMENTED`, then to `VERIFIED`, with closure evidence.
- Every audit separates a frozen history-blind current-code pass from the
  later Git/history pass. See `features/.agents/PROCESS.md`.

## Coverage statuses

| Status | Meaning |
|---|---|
| `PROVEN` | Scenario inputs drive a production API and its assertions verify the exact stated outcome. |
| `PARTIAL` | Part of the contract is real, but a stated boundary or invariant is not exercised. |
| `SEMANTIC_FALSE_POSITIVE` | The scenario passes without proving the outcome it claims. |
| `NOT_COVERED` | No selected scenario carries the requirement. |
| `PROXY` | The scenario reuses a global verdict without executing its own case. |

## Required structure

Every note contains:

1. **Metadata** — feature, date, revision, worktree state, and scope.
2. **Method provenance** — review units, Pass A isolation, contamination
   status, Pass B inputs, and reconciliation.
3. **Verdict** — concise outcome and exact scenario/step counts.
4. **Reproduced evidence** — commands and observed results.
5. **Scenario matrix** — status and production path for every scenario.
6. **Ordered findings** — impact, evidence, and expected behavior.
7. **Implementation plan** — minimal change, expected RED tests, and closure
   criteria.
8. **Decisions required** — protocol or product choices that code must not
   decide silently.
9. **Definition of done** — common gates required to close the note.

## Evidence rules

A scenario is `PROVEN` only when:

- no `@wip` tag or filter excludes it;
- the runner executes a non-zero, expected scenario count;
- the `When` calls production code or a real public facade;
- the Gherkin parameters reach that call;
- the `Then` verifies the scenario-specific result, not a global success;
- a rejected mutation proves the absence of partial effects;
- stated boundaries — wire parsing, fresh store, reopen, network, restart —
  are actually crossed;
- structural cryptographic cases are backed by independent vectors when
  byte-exact compliance is required;
- the verdict first survives a current-code trace without Git history or
  previous conclusions, then a separate differential review.

Unit tests, vectors, and Gherkin scenarios are complementary. None may be
presented as a silent substitute for another.

## Index

| Feature | Note | Current verdict |
|---|---|---|
| `a-identity.feature` | [`a-identity.md`](a-identity.md) | Round 2 audit and impact review complete; AID-001/002/005 verified within pilot scope; AID-003 open; AID-004 decision required |
| `b-derivation.feature` | [`b-derivation.md`](b-derivation.md) | Rounds 1 and 2 reviews accepted: BDER-001/002/003/004/005/009 `VERIFIED`; BDER-011 (harness, repo-wide) `VERIFIED` 2026-07-30; BDER-006 and BDER-008 decided 2026-08-02 and `VERIFIED` by the independent round-2 review (candidate `4f5921e`); BDER-007, BDER-010, BDER-012 open, BDER-013 opened by that review; `REVIEW_ACCEPTED`, impact review pending |
| `c-headers.feature` | [`c-headers.md`](c-headers.md) | Round 1 initial audit, orchestrated run `2026-08-03-r1` on `a2087f2`: 2 `PROVEN`, 5 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE`; 23 active findings (P1 ×1, P2 ×9, P3 ×13), CHDR-003 and CHDR-008 withdrawn, CHDR-022 and CHDR-023 requalified; CHDR-007 and CHDR-012 `DECISION_REQUIRED` and under the disclosure gate (blocking condition 9); `AUDIT_INITIAL` — **identifier collision**: the public branch `codex/audit-c-headers` (`af32734`) already assigns `CHDR-001`…`CHDR-016` to different statements, so every `CHDR-*` reference must name its source |

---
feature: c-headers
status: BLOCKED
mode: audit
round: 1
base_main: a2087f2392389fb17e0bc0ba9e20a164d53766d8
audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8
candidate_revision: null
branch: codex/audit-c-headers-r2
assigned_findings: []
open_findings: [CHDR-001, CHDR-002, CHDR-007, CHDR-009, CHDR-012, CHDR-013, CHDR-014, CHDR-016, CHDR-019, CHDR-021, CHDR-025]
rejection_count: {}
blocked: {conditions: '6 9 1 7', since: 2026-08-03, owner: human}
last_transition: 2026-08-03T13:05:00+00:00
---

# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `BLOCKED` — the initial audit ran to its integration pass on run `2026-08-03-r1` and stopped there. Four blocking conditions are open in `../orchestrator/BLOCKED.md`: 6 (two warden invalidations), 9 (disclosure gate on two findings), 1 (`DECISION_REQUIRED` on the same two), 7 (agent budget exceeded). The branch is **not** pushed: pushing would publish the text the warden flagged |
| Expected mode | `audit` — initial audit, round 1 |
| Round | 1 |
| Base of round 1 | not frozen (`base_main: a2087f2392389fb17e0bc0ba9e20a164d53766d8`) — the role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | not frozen (`audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8`) |
| Candidate revision | none (`candidate_revision: null`) — no correction exists |
| Canonical branch | `codex/audit-c-headers-r2` |
| Why not `codex/audit-c-headers` | that name is reserved: `../orchestrator/QUEUE.yaml:55-56` registers it as this feature's **yardstick**, prior manual work that is a Pass B input and a milestone comparison only, never a Pass A input |
| Correction branch | to be created as `codex/fix-c-headers-<finding-or-scope>` from the immutable audited revision, once one exists |
| Canonical tag | `@c-headers` (`features/c-headers.feature:1`) |
| Expected gate selection | 1 feature / 4 rules / 8 scenarios / 28 steps |
| Findings | 27 identifiers, 23 active: 1 P1, 9 P2, 13 P3; 2 withdrawn, 2 requalified out of scope. Adversarial panel run on all 16 frozen P1/P2 findings, 3 refuters each: 8 survived, 8 refuted and reconciled by the integration pass. Two are embargoed and appear by identifier and neutral title only |
| Public audit | `docs/audits/features/c-headers.md` — written by run `2026-08-03-r1`; index row added. Two findings appear by identifier and neutral title only, per the disclosure gate |
| Gherkin markers | 6 scenarios carry markers for unresolved findings; gate re-run after insertion is green and unchanged at 1/4/8/28 (`ev-c30fa81e`) |
| Recorded follow-up owed | `TARGETED` from the accepted b-derivation round-2 impact review (`../orchestrator/QUEUE.yaml:61-62`, `../orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`): the independent-generation claim of `vectors/c1-header-seal.json` and `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3`. Evidence class, not behaviour |
| Next role | **the human owner** — `../orchestrator/BLOCKED.md`. No agent role may proceed |
| Blocked | yes — conditions 6, 9, 1, 7 |

## Inputs

- process: `../PROCESS.md`;
- domain rules and routing: `../../AGENTS.md`;
- domain: `DOMAIN.md`;
- shared skills: `../shared/audit-gherkin-feature/SKILL.md`,
  `../shared/correct-gherkin-feature/SKILL.md`;
- queue and recorded follow-ups: `../orchestrator/QUEUE.yaml`;
- ledger format and the restricted frontmatter grammar:
  `../orchestrator/LEDGER.md`.

## Current instruction

Run the initial audit of `features/c-headers.feature` with
`auditor/audit-c-headers/SKILL.md`, in the mode named by the `mode` field
above.

Before collecting any evidence, freeze the revision and record it in
`base_main` and `audit_revision`. Nothing in this file may be read as evidence
about the feature's behaviour: this domain was bootstrapped without running a
single gate and without reading any history.

The yardstick branch `codex/audit-c-headers` and any material it carries are
Pass B inputs only. If it is opened before Pass A is frozen, the review unit is
contaminated and must be restarted per `../PROCESS.md`, section "Pass A —
current code, history-blind".

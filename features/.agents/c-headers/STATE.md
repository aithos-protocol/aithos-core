---
feature: c-headers
status: READY
mode: audit
round: 1
base_main: null
audit_revision: null
candidate_revision: null
branch: codex/audit-c-headers-r2
assigned_findings: []
open_findings: []
rejection_count: {}
blocked: null
last_transition: 2026-08-03
---

# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `READY` — the domain is bootstrapped; no audit has been performed on this feature by any agent role |
| Expected mode | `audit` — initial audit, round 1 |
| Round | 1 |
| Base of round 1 | not frozen (`base_main: null`) — the role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | not frozen (`audit_revision: null`) |
| Candidate revision | none (`candidate_revision: null`) — no correction exists |
| Canonical branch | `codex/audit-c-headers-r2` |
| Why not `codex/audit-c-headers` | that name is reserved: `../orchestrator/QUEUE.yaml:55-56` registers it as this feature's **yardstick**, prior manual work that is a Pass B input and a milestone comparison only, never a Pass A input |
| Correction branch | to be created as `codex/fix-c-headers-<finding-or-scope>` from the immutable audited revision, once one exists |
| Canonical tag | `@c-headers` (`features/c-headers.feature:1`) |
| Expected gate selection | 1 feature / 4 rules / 8 scenarios / 28 steps |
| Findings | none. `assigned_findings` and `open_findings` are empty because no audit has run. Findings will take stable `CHDR-*` identifiers (`docs/audits/features/README.md:20`) |
| Public audit | `docs/audits/features/c-headers.md` — does not exist yet; the initial audit creates it and adds its index row in `docs/audits/features/README.md` |
| Gherkin markers | none in `features/c-headers.feature` |
| Recorded follow-up owed | `TARGETED` from the accepted b-derivation round-2 impact review (`../orchestrator/QUEUE.yaml:61-62`, `../orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`): the independent-generation claim of `vectors/c1-header-seal.json` and `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3`. Evidence class, not behaviour |
| Next role | the initial auditor, via `auditor/audit-c-headers/SKILL.md` |
| Blocked | no |

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

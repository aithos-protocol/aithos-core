---
feature: c-headers
status: CORRECTION_REQUESTED
mode: correction
round: 1
base_main: a2087f2392389fb17e0bc0ba9e20a164d53766d8
audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8
candidate_revision: null
branch: codex/audit-c-headers-r2
assigned_findings: [CHDR-007, CHDR-012]
open_findings: [CHDR-001, CHDR-002, CHDR-007, CHDR-009, CHDR-012, CHDR-013, CHDR-014, CHDR-016, CHDR-019, CHDR-021, CHDR-025]
rejection_count: {}
blocked: null
last_transition: 2026-08-03T14:10:00+00:00
---

# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` — the initial audit completed on runs `2026-08-03-r1` and `-r2`. **All four blocking conditions are now closed.** Conditions 9, 6 and 7 by the disclosure and budget ruling of 2026-08-03; condition 1 by `decisions/2026-08-03-chdr-007-012-i3-authority.md`, which rules reading A on both findings: I3 binds the recipient key, not the label, and it binds the edition verifier |
| Expected mode | `correction` — lot B, `CHDR-007` and `CHDR-012`, after the owner ruling of 2026-08-03 |
| Round | 1 |
| Base of round 1 | not frozen (`base_main: a2087f2392389fb17e0bc0ba9e20a164d53766d8`) — the role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | not frozen (`audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8`) |
| Candidate revision | none (`candidate_revision: null`) — no correction exists |
| Canonical branch | `codex/audit-c-headers-r2` |
| Why not `codex/audit-c-headers` | that name is reserved: `../orchestrator/QUEUE.yaml:55-56` registers it as this feature's **yardstick**, prior manual work that is a Pass B input and a milestone comparison only, never a Pass A input |
| Correction branch | **lot B**: `codex/fix-c-headers-i3-authority`, from the immutable audited revision `a2087f2`, carrying `CHDR-007` and `CHDR-012` together. **Lot A** follows on its own branch with the nine test-semantics findings, after lot B, so the shared fixtures migrate once |
| Canonical tag | `@c-headers` (`features/c-headers.feature:1`) |
| Expected gate selection | 1 feature / 4 rules / 8 scenarios / 28 steps |
| Decision on record | `decisions/2026-08-03-chdr-007-012-i3-authority.md` — reading A on `CHDR-007` and `CHDR-012`. Consequences: a specification lot before any code, five public signatures of `aithos-core` change, major version bump |
| Findings | 27 identifiers, 23 active: 1 P1, 9 P2, 13 P3; 2 withdrawn, 2 requalified out of scope. Adversarial panel run on all 16 frozen P1/P2 findings, 3 refuters each: 8 survived, 8 refuted and reconciled against current-code evidence by the integration pass. Nine are assigned to the corrector. `CHDR-007` and `CHDR-012` are `DECISION_REQUIRED` and assigned to nobody |
| Public audit | `docs/audits/features/c-headers.md` — written by run `2026-08-03-r1`, completed by `-r2` after the owner lifted the embargo. Every finding is stated in full |
| Gherkin markers | 6 scenarios carry markers for unresolved findings; gate re-run after each marker edit is green and unchanged at 1/4/8/28 (`ev-c30fa81e`, then `ev-91717a6d`) |
| Recorded follow-up owed | `TARGETED` from the accepted b-derivation round-2 impact review (`../orchestrator/QUEUE.yaml:61-62`, `../orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`): the independent-generation claim of `vectors/c1-header-seal.json` and `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3`. Evidence class, not behaviour |
| Next role | **the human owner first**: the specification lot required by the decision (`spec/03-headers.md` §3.1 and §3.4, `spec/00-overview.md` §0.2, `spec/09-cli-and-conformance.md` §9.2 and its vector) must land before any code. A corrector must not code against a text that still has to be written. Then the corrector, via `corrector/correct-c-headers/SKILL.md`, on `assigned_findings` only |
| Blocked | no. All four conditions raised by runs `-r1` and `-r2` are resolved in `../orchestrator/BLOCKED.md`. The gap noted on 2026-08-03 stands and is recorded for the train's own backlog: `train-status.py` rejects a `blocked` entry on any status other than `BLOCKED`, so a feature that proceeds while some of its findings await a human ruling has no representation in the frontmatter |

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

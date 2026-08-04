---
feature: c-headers
status: CORRECTION_REQUESTED
mode: correction
round: 2
base_main: 2f2d55d
audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8
candidate_revision: 9dc58895b5c822d13ea5daf8c25302ccd657b668
branch: codex/fix-c-headers-lot-a
assigned_findings: [CHDR-001, CHDR-002, CHDR-009, CHDR-013, CHDR-014, CHDR-016, CHDR-019, CHDR-021, CHDR-025]
open_findings: [CHDR-001, CHDR-002, CHDR-009, CHDR-013, CHDR-014, CHDR-016, CHDR-019, CHDR-021, CHDR-025, CHDR-028, CHDR-029, CHDR-030]
rejection_count: {}
blocked: null
last_transition: 2026-08-04T08:10:00+00:00
---

# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` — the initial audit completed on runs `2026-08-03-r1` and `-r2`. **All four blocking conditions are now closed.** Conditions 9, 6 and 7 by the disclosure and budget ruling of 2026-08-03; condition 1 by `decisions/2026-08-03-chdr-007-012-i3-authority.md`, which rules reading A on both findings: I3 binds the recipient key, not the label, and it binds the edition verifier |
| Expected mode | `review` — lot B is implemented and awaits an independent reviewer. `CHDR-007` and `CHDR-012` are `IMPLEMENTED`, never `VERIFIED`: only the reviewer may raise them |
| Round | 1 |
| Base of round 1 | not frozen (`base_main: 2f2d55d`) — the role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | not frozen (`audit_revision: a2087f2392389fb17e0bc0ba9e20a164d53766d8`) |
| Candidate revision | `9dc5889` on `codex/fix-c-headers-i3-authority` — lot B, `CHDR-007` and `CHDR-012` at `IMPLEMENTED`. This is the revision the independent reviewer extracts, without `.git` and without the corrector's run report, until its behavioural verdict is frozen |
| Canonical branch | `codex/audit-c-headers-r2` |
| Why not `codex/audit-c-headers` | that name is reserved: `../orchestrator/QUEUE.yaml:55-56` registers it as this feature's **yardstick**, prior manual work that is a Pass B input and a milestone comparison only, never a Pass A input |
| Correction branch | **lot B**: `codex/fix-c-headers-i3-authority`, based on `5be3047` whose `rust/` tree is byte-identical to the audited revision `a2087f2`. Carries `CHDR-007` and `CHDR-012`. Run `2026-08-04-r1`, sixteen gates, RED before GREEN in ledger order. **Lot A** follows on its own branch with the nine test-semantics findings |
| Canonical tag | `@c-headers` (`features/c-headers.feature:1`) |
| Expected gate selection | 1 feature / 4 rules / 8 scenarios / 28 steps |
| Decision on record | `decisions/2026-08-03-chdr-007-012-i3-authority.md` — reading A on `CHDR-007` and `CHDR-012`. Consequences: a specification lot before any code, five public signatures of `aithos-core` change, major version bump |
| Findings | 27 identifiers, 23 active: 1 P1, 9 P2, 13 P3; 2 withdrawn, 2 requalified out of scope. Adversarial panel run on all 16 frozen P1/P2 findings, 3 refuters each: 8 survived, 8 refuted and reconciled against current-code evidence by the integration pass. Nine are assigned to the corrector. `CHDR-007` and `CHDR-012` are `DECISION_REQUIRED` and assigned to nobody |
| Public audit | `docs/audits/features/c-headers.md` — written by run `2026-08-03-r1`, completed by `-r2` after the owner lifted the embargo. Every finding is stated in full |
| Gherkin markers | 6 scenarios carry markers for unresolved findings; gate re-run after each marker edit is green and unchanged at 1/4/8/28 (`ev-c30fa81e`, then `ev-91717a6d`) |
| Recorded follow-up owed | `TARGETED` from the accepted b-derivation round-2 impact review (`../orchestrator/QUEUE.yaml:61-62`, `../orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`): the independent-generation claim of `vectors/c1-header-seal.json` and `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3`. Evidence class, not behaviour |
| Next role — superseded | the **independent reviewer**, via `auditor/audit-c-headers/SKILL.md` in review mode. It receives an extract of the candidate without `.git` and **without the corrector's run report** until its behavioural verdict is frozen (`PROCESS.md`, § *Material isolation of Pass A*). Two debts are named for it: the markers at `features/c-headers.feature:47-55` still read `DECISION_REQUIRED … neither is assigned to a corrector`, which the decision of 2026-08-03 made false; and the marker block also carries lot A identifiers, so the edit is not a simple deletion |
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

## Awaiting the owner — the fil stops here by design

The cycle has reached the one step no agent role performs. `PROCESS.md`
§ *Impact review* and the chantier both reserve integration into local `main` to
the human owner, and the fil never pushes `main`. `train.py` agrees:
`IMPACT_REVIEW_REQUESTED` leads only to `INTEGRATION` or `BLOCKED`.

**The gesture.** Merge `codex/fix-c-headers-i3-authority` into `main` and push
it. Fourteen commits ahead of `origin/main`, which still sits at `a2087f2`.

**A structural gap found on trying to continue.** Lot A — the nine
test-semantics findings still open — cannot be started from here. The state
machine models **one** correction per round: `AUDIT_INITIAL` →
`CORRECTION_REQUESTED` → `REVIEW_REQUESTED` → `REVIEW_ACCEPTED` →
`IMPACT_REVIEW_REQUESTED` → `INTEGRATION` → `COMPLETE`, and `COMPLETE` is
terminal with reopening forbidden. A feature whose audit yields findings in two
lots has no representation: the second lot must become a round 2 opened from an
integrated `main`, which is the `b-derivation` pattern. That is workable, but it
means an audit cannot hand a corrector two independent lots without a human
integration between them. Recorded rather than worked around.

## Round 2 — OPENED 2026-08-04. What follows is the plan it was opened on.

Written here rather than held in a session, per the chantier's rule that the
orchestrator has no memory: a cold session resumes from this file alone.

**Trigger.** `origin/main` moves off `a2087f2`. Nothing else. The owner
performs the merge; no agent role does.

**Then, in order.**

1. Open round 2: `round: 2`, `mode: correction`, `status: CORRECTION_REQUESTED`,
   `base_main` = the new `main`, `branch: codex/fix-c-headers-lot-a`.
2. `assigned_findings: [CHDR-001, CHDR-002, CHDR-009, CHDR-013, CHDR-014,
   CHDR-016, CHDR-019, CHDR-021, CHDR-025]` — **lot A**, the nine
   test-semantics findings. They touch assertions in
   `rust/crates/aithos-bundle/tests/cucumber.rs` and the C-family tests; no
   production code is in scope.
3. `CHDR-007` and `CHDR-012` are `VERIFIED` and do not return.
   `CHDR-028`, `CHDR-029`, `CHDR-030` are **not** assigned: `CHDR-028` is held
   by the disclosure gate, and the other two were classed by the impact review
   as belonging to `g-revocation`, `n-structural-mutations` and
   `o-connector-classes-vault`, not here.
4. Corrector, then the gates run by the orchestrator, then an independent
   reviewer on a candidate extract without `.git` and without the corrector's
   report — the same shape that worked on lot B.

**Why lot A comes second and not first.** The fixtures it edits migrated to the
new signature during lot B. Doing it first would have opened `cucumber.rs`
twice.

**One thing the corrector must be told, and it is new.** `features/AGENTS.md`
now carries the § *Project stage* section: nothing is deployed, no edition has
been published, so backward compatibility is not a cost and must not be weighed.
Lot A is where that first bites — several of its findings ask for stronger
assertions that would have been softened to spare a past that has no content.

---
feature: b-derivation
status: COMPLETE
mode: null
round: 2
base_main: 513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3
audit_revision: 513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3
candidate_revision: 4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41
branch: codex/fix-b-derivation-bder-006-008-decisions
assigned_findings: []
open_findings: [BDER-007, BDER-010, BDER-012, BDER-013]
rejection_count: {}
blocked: null
last_transition: 2026-08-03
---

# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `COMPLETE` (agent side of the round-2 cycle) — impact review written; human acceptance and the integration of the branch into local `main` remain pending and are performed by no agent role |
| Expected mode | none — no agent role is awaited on this feature |
| Round | 2 (correction reviewed and accepted by an independent auditor on 2026-08-02) |
| Base of round 2 | local `main` `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` (round-1 closure + both decision records) |
| Correction branch | `codex/fix-b-derivation-bder-006-008-decisions` |
| Baseline (immutable) | `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` |
| Candidate (immutable) | `4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41` — **accepted** |
| Reviewed range | `513b366..4f5921e` — one commit, three files, four lines, no production code. The branch also carries this round's documentation commits (corrector run, this file, public audit) and the review's own commits on top of the candidate; they are not part of the behavioural candidate. |
| Corrector run | `corrector/runs/2026-08-02-correction-02.md` |
| Review run | `auditor/runs/2026-08-02-audit-review-02.md` — Pass A frozen in its own commit (`9c52a7a`) before any history was opened |
| Round 1 integration | accepted by the human owner on 2026-08-02; audit branch content on `main` (`3d6fa51`, `ae88f7f`, `1ab331a`, impact review `7854895`) |
| Impact review, round 1 | `orchestrator/runs/2026-07-29-b-derivation-impact-review.md` — accepted 2026-08-02, no `FULL_AUDIT`, one `TARGETED` (`d-bundle`), widened by decision `BDER-006` to owe the tag-view/`wrap` scenarios |
| Impact review, round 2 | `orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md` (2026-08-03, range `513b366..4f5921e`) — **no `FULL_AUDIT`**, no undecided classification; five `TARGETED` features (`a-identity`, `c-headers`, `d-bundle`, `e-mandates`, `n-structural-mutations`) plus two cross-cutting non-feature targets; two premise corrections recorded below |
| `BDER-011` | closed — cross-cutting lot done, `VERIFIED` 2026-07-30 (`78c06ba`, `090d11a`, `c630753`) |
| Decisions recorded | `decisions/2026-08-02-bder-006-tag-view-rule-scope.md` (option A + mandatory `d-bundle` extension) ; `decisions/2026-08-02-bder-008-b2-provenance.md` (honest provenance claim, generator deferred) |
| Findings `VERIFIED` | `BDER-001`…`BDER-005`, `BDER-006`, `BDER-008`, `BDER-009`, `BDER-011` |
| Findings open, not assigned to a round | `BDER-007` (closes only via the future independent B2 generator lot), `BDER-010` (informative — doc comment on `node_key` only), `BDER-012` (bounded negatives; future round), `BDER-013` (new — the retracted provenance claim survives in `rust/crates/aithos-core/tests/b2_derivation.rs:2`) |
| Gherkin markers | `@audit-partial @bder-006` and its adjacent comment removed by the review; `@audit-partial @bder-012` remains |
| Next role | none (agent side). Human owner: accept the round-2 impact review, rule on its follow-ups 3 and 5, then decide on integration |
| After the impact review | integration of the feature branch into local `main` is a human decision; it is not performed by any agent role |

## Inputs

- decisions: `decisions/2026-08-02-*.md`;
- review conclusion (authoritative for this round):
  `auditor/runs/2026-08-02-audit-review-02.md`;
- corrector conclusion (a claim, verified by the review):
  `corrector/runs/2026-08-02-correction-02.md`;
- public audit: `docs/audits/features/b-derivation.md`;
- round 1 review: `auditor/runs/2026-07-29-audit-review-01.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Round 2 is accepted and its global impact review is written. `BDER-006` and
`BDER-008` were accepted **separately**, each against its own written closure
criterion, on independently reproduced evidence. No further correction is
requested on this feature, and **no agent role is awaited on it**.

What remains is human: accept
`orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md`, rule on the two
premise corrections it records (below), and decide whether to integrate
`codex/fix-b-derivation-bder-006-008-decisions` into local `main`. No agent role
performs that merge.

### What the correction review established

1. **`BDER-006` → `VERIFIED`.** The baseline title « Tag views anchor at
   folders » was written in the vocabulary of spec §02.9 while its single
   scenario proves only derivations; the candidate title « Each tag anchor is a
   distinct derivation » claims only what §02.5 governs and what the traced
   scenario proves (`cucumber.rs:7541`, `:8079`, `:12295` →
   `aithos-core::derive::node_key` with the production `t/<tag>` label, arity
   guard then three-way distinctness). Feature gate reproduced once on the
   immutable candidate: 1 feature / 3 rules / 6 scenarios / 30 steps.
2. **`BDER-008` → `VERIFIED`.** Every assertion of the rewritten `description`
   was checked independently: five named Python cross-checks located line by
   line, one for `deep_section_key_hex`, no external witness for the other five
   fields, no `gen-b2*` on any branch, all three B2 artefacts born in `1b7d258`,
   and `description` the only field that moved. All five expected keys were
   recomputed from scratch in Python `blake3`: five matches. The
   `ownership.json` re-pin is the intended reading of README rule 3 — the review
   rules on this explicitly.
3. **`BDER-013` opened** on the residue `BDER-008` leaves behind:
   `rust/crates/aithos-core/tests/b2_derivation.rs:2` still reads « Expected
   values generated independently (Python blake3) ». Disclosed by the corrector,
   correctly outside its one-action mandate, now tracked under its own stable
   identifier. `OPEN`, P3.

### Carried forward, and who owns it

- **`d-bundle` widened `TARGETED` debt** — the review confirmed independently
  that `d-bundle.feature` still contains no tag-view and no `wrap` scenario, so
  the behavioural half of spec §02.9 remains unproven anywhere in the executable
  corpus. Without it, decision A degenerates into "A alone", which is explicitly
  not what was decided. Owner: the `d-bundle` cycle, via the orchestrator's
  tracked follow-ups. **Qualified on 2026-08-03** by the round-2 impact review
  §4.3: part of that behavioural half *is* exercised, by `e-mandates`; the
  residual debt and its owner must be re-arbitrated by the decision owner.
- **`verify-feature-tags.sh` is red repo-wide** — it exits 1 on both `513b366`
  and `4f5921e` because `features/gateway-delegated-client-surfaces.feature`
  starts with `@wip @g4 @wasm @cli`. `PROCESS.md` makes that script mandatory
  before any audit, correction or review, so every role on every feature is
  currently obliged to run a gate none can pass. Pre-existing, unrelated to this
  feature, not worked around and not repaired by the review. **Needs a process
  decision.** Owner: the process owner (Mathieu), via the orchestrator.
- **`BDER-007`, `BDER-010`, `BDER-012`, `BDER-013`** remain open and visible.
- **No gate has ever run on the workstation** for this feature: the device VM
  has no Rust toolchain. Both the corrector and the reviewer ran gates on
  `git archive` exports of the immutable revisions, SHA-256 verified on both
  sides. The reviewer additionally found and discarded a first gate result that
  had silently reused a pre-existing container build; every number in the review
  report comes from a clean rebuild. Any role reproducing this must use a fresh
  `CARGO_TARGET_DIR`.

### What the round-2 impact review established (2026-08-03)

`orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md`: **no
`FULL_AUDIT`, no undecided classification.** Five `TARGETED` features
(`a-identity`, `c-headers`, `d-bundle`, `e-mandates`,
`n-structural-mutations`), thirteen `NONE`, plus two cross-cutting non-feature
targets (the `ownership.json` pin / README rule 3 reading, and the repo-wide
provenance claims). None of the code, Gherkin or Python consumers of
`vectors/b2-derivation.json` can observe the change: neither `serde` struct
(`b2_derivation.rs:9-21`, `cucumber.rs:154-163`) declares `description` or
forbids unknown fields, the five Python cross-checks read fields by key, and the
twelve B2-bearing step phrases remain exclusive to `b-derivation.feature`.
`b2-derivation.json` is `owner: core` and **not** `shared: true`, so the re-pin
cannot reach the `aithos-service` repository.

Two premise corrections, which do **not** reopen `BDER-006` or `BDER-008` — both
closure criteria are met — but which the human owner should rule on:

1. **`BDER-013` is not specific to B2.** `a1-genesis.json`, `a2-did.json`,
   `c1-header-seal.json` and `e1-mandate.json` claim independent generation while
   no `gen-a*`, `gen-c1*` or `gen-e1*` has ever existed on any branch; the claim
   is repeated by their four Rust test headers, by `docs/CONFORMANCE.md:48-50`
   (universal, and its §09.2 table credits `b2-derivation` and « `b2` anchors »),
   by `README.md:18-19` and by `docs/audits/features/a-identity.md:297`.
   `BDER-013`'s written closure criterion cannot be met without at least
   `docs/CONFORMANCE.md`.
2. **The §02.9 premise behind the widened `d-bundle` debt is inexact.**
   `e-mandates.feature:28-32` and `:49-53` traverse
   `aithos-bundle::grants.rs:324-345` (tag-view anchor, then
   `// Bridge every matching section into the view (§02.9).` wrap sealing) and
   `:884-893` / `:965-973` (the section key is obtained by opening that wrap,
   subtree- and tag-filtered), with real reads as assertions. What is genuinely
   unproven anywhere is the zone-root view's coverage of the whole zone and an
   explicit « an anchor derives nothing downward » negative. Re-scoping the
   `d-bundle` obligation is the decision owner's call, not an agent's.

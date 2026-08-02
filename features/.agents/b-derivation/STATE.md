# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `REVIEW_ACCEPTED` |
| Expected mode | impact review (`review-gherkin-impacts`) on the accepted round-2 range |
| Round | 2 (correction reviewed and accepted by an independent auditor on 2026-08-02) |
| Base of round 2 | local `main` `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` (round-1 closure + both decision records) |
| Correction branch | `codex/fix-b-derivation-bder-006-008-decisions` |
| Baseline (immutable) | `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` |
| Candidate (immutable) | `4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41` — **accepted** |
| Reviewed range | `513b366..4f5921e` — one commit, three files, four lines, no production code. The branch also carries this round's documentation commits (corrector run, this file, public audit) and the review's own commits on top of the candidate; they are not part of the behavioural candidate. |
| Corrector run | `corrector/runs/2026-08-02-correction-02.md` |
| Review run | `auditor/runs/2026-08-02-audit-review-02.md` — Pass A frozen in its own commit (`9c52a7a`) before any history was opened |
| Round 1 integration | accepted by the human owner on 2026-08-02; audit branch content on `main` (`3d6fa51`, `ae88f7f`, `1ab331a`, impact review `7854895`) |
| Impact review | `orchestrator/runs/2026-07-29-b-derivation-impact-review.md` — accepted 2026-08-02, no `FULL_AUDIT`, one `TARGETED` (`d-bundle`), widened by decision `BDER-006` to owe the tag-view/`wrap` scenarios; that widening is recorded in `orchestrator/STATE.md` and confirmed outstanding by this review |
| `BDER-011` | closed — cross-cutting lot done, `VERIFIED` 2026-07-30 (`78c06ba`, `090d11a`, `c630753`) |
| Decisions recorded | `decisions/2026-08-02-bder-006-tag-view-rule-scope.md` (option A + mandatory `d-bundle` extension) ; `decisions/2026-08-02-bder-008-b2-provenance.md` (honest provenance claim, generator deferred) |
| Findings `VERIFIED` | `BDER-001`…`BDER-005`, `BDER-006`, `BDER-008`, `BDER-009`, `BDER-011` |
| Findings open, not assigned to a round | `BDER-007` (closes only via the future independent B2 generator lot), `BDER-010` (informative — doc comment on `node_key` only), `BDER-012` (bounded negatives; future round), `BDER-013` (new — the retracted provenance claim survives in `rust/crates/aithos-core/tests/b2_derivation.rs:2`) |
| Gherkin markers | `@audit-partial @bder-006` and its adjacent comment removed by the review; `@audit-partial @bder-012` remains |
| Next role | global impact reviewer — `review-gherkin-impacts` on `513b366..4f5921e` |
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

Round 2 is accepted. `BDER-006` and `BDER-008` were accepted **separately**, each
against its own written closure criterion, on independently reproduced evidence.
No further correction is requested on this feature.

The next role is the global impact reviewer, on the range `513b366..4f5921e`.
It does not reopen any finding and does not restart any audit.

### What the review established

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
  tracked follow-ups.
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

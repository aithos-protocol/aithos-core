# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `REVIEW_REQUESTED` |
| Expected mode | `review` (round 2 candidate) |
| Round | 2 (correction done, awaiting independent review) |
| Base of round 2 | local `main` `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` (round-1 closure + both decision records) |
| Correction branch | `codex/fix-b-derivation-bder-006-008-decisions` |
| Baseline (immutable) | `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` |
| Candidate (immutable) | `4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41` |
| Reviewable range | `513b366..4f5921e` — the two corrections, three files, no production code. The branch also carries this round's documentation commits (run report, this file, public audit) on top of the candidate; they are not part of the behavioural candidate. |
| Corrector run | `corrector/runs/2026-08-02-correction-02.md` |
| Round 1 integration | accepted by the human owner on 2026-08-02; audit branch content on `main` (`3d6fa51`, `ae88f7f`, `1ab331a`, impact review `7854895`) |
| Impact review | `orchestrator/runs/2026-07-29-b-derivation-impact-review.md` — accepted 2026-08-02, no `FULL_AUDIT`, one `TARGETED` (`d-bundle`), widened by decision `BDER-006` to owe the tag-view/`wrap` scenarios |
| `BDER-011` | closed — cross-cutting lot done, `VERIFIED` 2026-07-30 (`78c06ba`, `090d11a`, `c630753`) |
| Decisions recorded | `decisions/2026-08-02-bder-006-tag-view-rule-scope.md` (option A + mandatory `d-bundle` extension) ; `decisions/2026-08-02-bder-008-b2-provenance.md` (honest provenance claim, generator deferred) |
| Findings `VERIFIED` | `BDER-001`…`BDER-005`, `BDER-009`, `BDER-011` |
| Findings `IMPLEMENTED` (round 2, to review) | `BDER-006` (`Rule` retitled), `BDER-008` (B2 `description` rewritten, values frozen) |
| Findings open, not assigned | `BDER-007` (closes only via the future independent B2 generator lot), `BDER-010` (informative — doc comment on `node_key` only), `BDER-012` (bounded negatives; future round) |
| Next role | auditor — `audit-b-derivation` in `review` mode, independent, against `4f5921e` |
| After review | if accepted: remove the `@audit-partial @bder-006` marker, integrate, then impact review per `PROCESS.md` |

## Inputs

- decisions: `decisions/2026-08-02-*.md`;
- corrector conclusion (a claim to verify, not evidence):
  `corrector/runs/2026-08-02-correction-02.md`;
- public audit: `docs/audits/features/b-derivation.md`;
- round 1 review: `auditor/runs/2026-07-29-audit-review-01.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Independent review of round 2, on the range `513b366..4f5921e`. The candidate
touches three files and no production code:

1. **BDER-006** — `features/b-derivation.feature:58`, `Rule` title only:
   « Tag views anchor at folders » → « Each tag anchor is a distinct
   derivation ». No scenario, step, tag or comment changed. Verify the new
   title promises nothing beyond what the Rule's single scenario proves, and
   that no §02.9 anchoring semantics is implied.
2. **BDER-008** — `vectors/b2-derivation.json`, `description` only. Verify that
   every other key is byte-identical, and that the stated provenance and the
   per-field corroboration status match what the repository actually contains.
3. Mechanical consequence — `vectors/ownership.json`: the pinned SHA-256 of
   `b2-derivation.json` is re-pinned in the same change (`73a4740d…` →
   `ec5be797…`) and `updated` re-dated, without which
   `aithos_bundle::vectors_ownership::vectors_match_their_pinned_digests` is
   red. Rule explicitly on whether this is the intended reading of README
   rule 3.

Accept or reject `BDER-006` and `BDER-008` separately. Remove
`@audit-partial @bder-006` and its adjacent comment only for a finding accepted
as `VERIFIED`.

Three divergences are declared in the corrector's report and need an explicit
verdict:

- `features/.agents/scripts/verify-feature-tags.sh` is **red at the baseline**
  and stays red: `features/gateway-delegated-client-surfaces.feature` starts
  with `@wip @g4 @wasm @cli`. Pre-existing since `48ac462` (SPL-1, 2026-07-30),
  unrelated to this feature, not fixed by this round. It blocks the mandatory
  pre-gate of every feature audit and needs its own decision.
- `rust/crates/aithos-core/tests/b2_derivation.rs:2` still carries the same
  retracted claim (« Expected values generated independently (Python
  blake3) »), left untouched because it is outside the assigned scope.
- gates were executed on a container export of the candidate: the device's
  Linux VM has no Rust toolchain.

Out of scope for round 2: `BDER-007` (future generator lot), `BDER-010`
(informative), `BDER-012` (future round). `d-bundle.feature` is untouched and
its widened `TARGETED` follow-up still owes the tag-view/`wrap` scenarios —
without them decision A degenerates into "A alone".

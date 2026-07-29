# Domain state — `b-derivation`

| Field | Value |
|---|---|
| Status | `REVIEW_REQUESTED` |
| Expected mode | `review` |
| Round | 1 |
| Canonical audit branch | `codex/audit-b-derivation` |
| Initial audit baseline | `891c808` (historical branch `codex/gherkin-agent-pilot`, clean worktree) |
| Current `main` integration baseline | `5c3a61852dee0886fb6fff008a6304e8ea2c71bb` |
| Rebased audit record | `9c3c9bc` |
| Correction baseline (frozen, immutable) | `fa8fa797b897a762a0dfd7fc20910f053ce349ed` |
| Correction commit | `3d6fa51aaf9049e0deb81873242103c49f86de08` |
| Correction candidate (tip) | this record's own commit, child of `3d6fa51` |
| Corrector branch | `codex/fix-b-derivation-bder-001-005-honest-assertions` |
| Correction run | `corrector/runs/2026-07-29-correction-01.md` |
| Findings moved to `IMPLEMENTED` | `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`, `BDER-005`, `BDER-009` |
| Findings still open | `BDER-006` (decision), `BDER-007`, `BDER-008`, `BDER-010` |
| Next role | `audit-b-derivation` in `review` mode |
| Expected conclusion | `auditor/runs/<date>-audit-review-01.md` |

## Inputs

- public audit: `docs/audits/features/b-derivation.md`;
- initial audit: `auditor/runs/2026-07-29-audit-initial.md`;
- correction: `corrector/runs/2026-07-29-correction-01.md`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Review the candidate independently. Run the canonical feature gate once on the
candidate, complete a history-blind Pass A against its current code, and
**freeze that verdict before reading the correction diff or the corrector's
report**. Then run Pass B on `fa8fa79..3d6fa51`, plus this record's commit,
which only fills in the revisions the correction could not know before it
existed. Do not rerun the global
Cucumber or workspace gates: the corrector owns them, and their results are in
its report as a claim to check, not as evidence.

Accept or reject `BDER-001`, `BDER-002`, `BDER-003`, `BDER-004`, `BDER-005`
and `BDER-009` separately. Mark `VERIFIED` only what you reproduced yourself,
and remove the `@audit-implemented` / `@bder-*` markers only for those.

Points the review must probe rather than take on trust:

1. The correction claims M5 now fails four of the six scenarios, up from two.
   The reference probe is a per-segment `parent XOR blake3(label)` `node_key`.
   Reproduce it on a throwaway copy; the claimed mechanism is that replaying a
   *public* label undoes an invertible step, so the enumeration and the upward
   assertions of BDER-003 must be the assertions that catch it.
2. The rename scenario kills no `node_key` mutant by construction. Decide
   whether the R1 probe (rename implemented as delete-and-recreate with a
   fresh sid) is the right risk to guard, or whether BDER-004 demands more.
3. The rename scenario now consumes four step phrases shared with `d-bundle`.
   This is the one place where the correction leaves this feature's previously
   exclusive step set. Confirm the shared-step reach is acceptable, and that
   composing those `Given`s cannot shift the `node_keys` accumulator that
   BDER-009 pins.
4. The corrected fixtures read `vectors/b2-derivation.json`. Confirm the file
   is unchanged in values, and that no assertion took authority from
   `tag_anchor_*_hex`, which BDER-007 shows has no witness outside
   `derive.rs:54`.

## Divergences recorded by the correction

- The canonical audit branch head was `fa8fa79`, not the `9c3c9bc` this state
  previously recorded. `fa8fa79` is the frozen correction baseline.
- The suite has grown to 836 scenarios / 3568 steps at baseline, against the
  815 / 3505 observed on `891c808`. Suite-wide mutant counts from the initial
  audit are no longer directly comparable; per-scenario verdicts inside
  `b-derivation` were re-measured from scratch.
- The gates were executed on a Linux `aarch64` container holding a
  `git archive` export of `fa8fa79`, because the workstation exposes no Rust
  toolchain to the corrector role. An independent reviewer with a local
  toolchain should re-run the feature gate on the candidate commit itself.

`891c808` remains the immutable revision of the initial semantic evidence.
`fa8fa79` is the immutable baseline of this correction round.

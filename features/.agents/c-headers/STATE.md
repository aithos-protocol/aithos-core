# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `CORRECTION_REQUESTED` |
| Expected mode | `correction` |
| Round | 1 (open) |
| Canonical audit branch | `codex/audit-c-headers` |
| `main` base revision | `240c6589986af6115530c90a7aa8646c2c44b68f` |
| Audited revision (frozen, immutable) | `3803fe806702143d5bb887b5ddc33fd3e0526285` |
| Worktree state at audit | clean except the pre-existing untracked `_to_delete/` |
| Gate evidence | 1 feature / 4 rules / 8 scenarios (8 passed) / 28 steps (28 passed) |
| Findings | `CHDR-001` … `CHDR-016`, all `OPEN` |
| Findings `DECISION_REQUIRED` | `CHDR-015` |
| Verdict | 2 `PROVEN`, 5 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE` |
| Audit run | `auditor/runs/2026-07-30-audit-initial.md` |
| Frozen Pass A | `auditor/runs/pass-a/RU-1.md` … `RU-4.md` |
| Public audit | `docs/audits/features/c-headers.md` |
| Next role | `correct-c-headers` (corrector) |
| Expected conclusion | `corrector/runs/<date>-correction-01.md` |

## Inputs

- public audit: `docs/audits/features/c-headers.md`;
- initial audit: `auditor/runs/2026-07-30-audit-initial.md`;
- frozen Pass A units: `auditor/runs/pass-a/`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`.

## Current instruction

Correct the findings assigned below, from a
`codex/fix-c-headers-<scope>` branch created from the immutable audited
revision `3803fe8`. Do not work on `main` or on the canonical audit branch.

### Assigned now — lot 1, then lot 2

**Lot 1 — scenario 8, the up-link wrap.** `CHDR-001`, `CHDR-005`, `CHDR-013`.
This is the only `SEMANTIC_FALSE_POSITIVE` in the feature and it sits on the
one mechanism by which derivation readers survive rung-2 revocation. Build real
state in the `Given`: derive `K_P`, derive the pre-rotation child key from it,
rotate the child's header to a fresh key, wrap *that* key. Have the `Then`
recover `K_P` by derivation before opening the wrap, and assert that the
pre-rotation derived child key no longer opens the new version.

**Lot 2 — scenario 7, the revocation cut.** `CHDR-002`, `CHDR-003`, `CHDR-004`.
Add the structural check on `key_versions["2"].lines` and call
`check_rotation(2)` in the existing `Then`, so the scenario proves the
structural claim its title makes and cannot pass if the rotation did not
happen.

### Assigned after lots 1 and 2 land

`CHDR-007`, `CHDR-006`, `CHDR-010`, `CHDR-011`, `CHDR-012`, `CHDR-008`,
`CHDR-009`, `CHDR-016`, `CHDR-013` (scenarios 1 and 5), `CHDR-014` — in the
order given by §7 of the public audit.

`CHDR-006` and `CHDR-016` belong together: dropping `key_version` from
`line_aad` was measured to leave all 836 scenarios of the whole suite green,
and the one core test written to catch it passes vacuously. Fix both or the
mutation stays undetectable behaviorally.

### Not assigned

`CHDR-015` is `DECISION_REQUIRED` and belongs to the human protocol owner:
whether I3 is an edition-level invariant enforced by `Bundle::verify`, a
construction-time invariant with the spec text narrowed to match, or a
read-path check. **Do not choose this implicitly and do not touch
`Bundle::verify` in this round.** It does not block lots 1 and 2.

## Constraints specific to this round

1. **No production change is required.** Every finding except `CHDR-015` is
   corrected in `rust/crates/aithos-bundle/tests/cucumber.rs`, in
   `rust/crates/aithos-core/tests/c1_header_seal.rs` (`CHDR-009`), or in the
   Gherkin. The audit found the header implementation faithful to spec §03
   everywhere it traced. If a correction seems to need a change in
   `aithos-core`, stop and report rather than widening scope.

2. **The gate's exit code proves nothing.** `BDER-011` is open: the runner
   calls `filter_run`, so it exits `0` even when scenarios fail. Read the
   printed scenario/step counts, and say so in the run report.

3. **The shared step functions are not local.** `sealed_header_owner_only`
   registers two `Given` phrases; `grantee_opens` and `opening_rejected` each
   register two `Then` phrases. `CHDR-007` and `CHDR-010` both require
   splitting or reworking one of them — changing a shared body silently changes
   another scenario. Check every registered phrase before editing.

4. **`opened` has two readers with different semantics.** `opening_rejected`
   reads `.last()`; `stranger_recovers_nothing` reads all of it. The positive
   control required by `CHDR-007` adds an earlier push in the same scenario, so
   both readers must be updated coherently.

5. **Extend, never renumber, the ephemeral/nonce fixtures.** `eph(1..3)` are
   the base fixtures, `eph(4)` the replay decoy, `eph(5)` the append,
   `eph(6..7)` the rotation, `non(9)` the wrap. New recipients take new
   indices.

6. **Preserve the C1/C2 vectors byte for byte.** `CHDR-009` extends
   `vectors/c1-header-seal.json`'s use, it does not change its values. Never
   change an AAD purpose string, the KEK `info` layout, or the `key_version`
   encoding without a normative decision.

7. **Each finding needs a RED test that fails on `3803fe8` for the intended
   reason.** §7 of the public audit states the expected RED for each lot.
   Document both RED and GREEN.

Move findings at most to `IMPLEMENTED`. Never `VERIFIED`. Request review from
`audit-c-headers` and set this state to `REVIEW_REQUESTED` with the immutable
baseline and candidate revisions.

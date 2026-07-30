# Domain state — `c-headers`

| Field | Value |
|---|---|
| Status | `AUDIT_INITIAL` |
| Expected mode | `initial audit` |
| Round | 1 (open) |
| Canonical audit branch | `codex/audit-c-headers` |
| `main` base revision | `240c6589986af6115530c90a7aa8646c2c44b68f` |
| Observed revision | `240c6589986af6115530c90a7aa8646c2c44b68f` |
| Worktree state | clean except the pre-existing untracked `_to_delete/` |
| Assigned scope | the eight existing scenarios of `features/c-headers.feature`, four `Rule` blocks |
| Finding prefix | `CHDR-*` |
| Next role | `audit-c-headers` (auditor, initial-audit mode) |
| Expected conclusion | `auditor/runs/<date>-audit-initial.md` |
| Expected public audit | `docs/audits/features/c-headers.md` |

## Inputs

- contract: `features/c-headers.feature`;
- domain: `DOMAIN.md`;
- process: `../PROCESS.md`;
- shared skill: `../shared/audit-gherkin-feature/SKILL.md`.

No prior audit, correction report, or finding exists for this feature. Pass A
starts uncontaminated; keep it that way.

## Current instruction

Run the initial audit of `c-headers.feature` on the observed revision.

Review units, one per Gherkin `Rule`:

1. `RU-1` — a line seals the node key to exactly one recipient
   (four scenarios: owner/grantee open, non-recipient, corrupted line,
   node/version binding);
2. `RU-2` — the owner line is mandatory, I3 (one scenario);
3. `RU-3` — grant is one appended line, touching nobody (one scenario);
4. `RU-4` — rotation cuts the revoked and re-links the parent
   (two scenarios).

Freeze each unit's Pass A notes before reading any other unit's verdict, the
Git history, or the surfaces' history. Then run Pass B and the shared-state
integration pass last, over the header fixtures and World fields named in
`DOMAIN.md`.

Two constraints specific to this baseline:

1. **The gate's exit code proves nothing.** `BDER-011` is open: the runner
   calls `filter_run`, so it exits `0` even when scenarios fail. Report the
   printed scenario/step counts as the evidence, and state explicitly that the
   exit code was not used.
2. **The fixtures are part of the traced surface.** Several `Given`/`When`
   phrases of this feature delegate to shared helpers and fixed constants
   rather than to per-scenario inputs. Establish for each scenario what the
   step actually executes and what its assertion actually compares, and
   distinguish a scenario that proves its own case from one that consumes a
   verdict established elsewhere.

Do not implement any correction. Do not run unfiltered Cucumber, broad
regression, or workspace gates. Set the state to `CORRECTION_REQUESTED`,
`DECISION_REQUIRED`, or — if every scenario is `PROVEN` —
`IMPACT_REVIEW_REQUESTED`, and name the next role.

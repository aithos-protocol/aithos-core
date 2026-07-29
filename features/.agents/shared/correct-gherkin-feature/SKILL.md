---
name: correct-gherkin-feature
description: Implement corrections requested by an already documented semantic Gherkin audit. Use this skill when a feature STATE explicitly requests fixes that make existing scenarios consistent with their contract and the protocol, with RED tests, minimal production changes, GREEN evidence, and a mandatory handoff to the independent auditor.
---

# Correct an audited Gherkin feature

## Preparation

1. Read `../../PROCESS.md` completely.
2. Inspect Git status, branches, and worktrees without changing them.
3. Read the domain, state, public audit, and latest auditor run.
4. Confirm that state explicitly requests a correction and names an immutable
   audited feature revision.
5. Create or resume
   `codex/fix-<feature-name>-<finding-or-scope>` from that exact revision.
   Never correct directly on `main` or on the canonical audit branch.
6. Freeze the baseline before the first modification and record the parent
   feature branch.
7. Run `features/.agents/scripts/verify-feature-tags.sh` and identify the
   canonical `@<feature-name>` tag.
8. Limit the work to the assigned findings.

Stop without changing code if state is `DECISION_REQUIRED`.

## Execution

1. Reproduce each defect on the identified production path.
2. Write a RED test that isolates the incorrect semantics.
3. Confirm that the test fails for the intended reason.
4. Implement the smallest correction in the layer that owns the invariant.
5. Avoid parallel verifiers and test-specific patches.
6. Repeat only the focused RED/GREEN tests while the implementation changes.
7. Run the canonical feature-tag gate once after the final change.
8. Run relevant regressions, then one unfiltered Cucumber/workspace gate before
   handoff when required by the domain.
9. Prove the absence of partial effects for every rejected mutation.
10. Format and inspect the diff.

Add only the scenarios or tests needed to prove the assigned findings. Do not
start a general coverage project or address an unassigned finding.

This execution role owns the global regression proof. The independent auditor
will rerun only the canonical feature gate and any focused test needed to
resolve a semantic contradiction.

## Documentation and handoff

- Document RED and GREEN results.
- Enumerate changed files, symbols, formats, and surfaces.
- Report any divergence between the audit and observed reality.
- Move a finding at most to `IMPLEMENTED`.
- Never use `VERIFIED`.
- Write a dated conclusion in the corrector's `runs` directory.
- Explicitly request review from the specialized auditor skill.
- Set state to `REVIEW_REQUESTED` with immutable baseline and candidate
  revisions.
- Leave integration into the canonical feature branch and `main` to the
  independent acceptance workflow.

---
name: audit-gherkin-feature
description: Audit the semantic truth of an existing Gherkin feature or independently review a correction produced by that audit. Use this skill to verify, through a history-blind code trace followed by a separate Git/differential pass, that passing scenarios are selected, propagate their parameters, call real production code, and prove their exact contract.
---

# Audit a Gherkin feature

## Preparation

1. Read `../../PROCESS.md` completely.
2. Read the specialized feature domain.
3. Read only the routing fields from its state: mode, revision, assigned
   scope, and output path.
4. Record the revision, branch, worktree state, and requested mode.
5. Do not search for entirely missing scenarios.

Do not read prior audit conclusions, corrector reports, commit history, or
diffs before the history-blind pass is frozen. If they are already present in
the active context, disclose that contamination and use a fresh review unit
when practical.

## Initial audit

### Pass A — history-blind current-code proof

1. Inventory Rules, Scenarios, Outlines, Examples, and tags.
2. Divide the feature into independent review units as defined by the process.
3. Confirm that the runner selects and executes the feature.
4. Resolve every phrase to its exact step definition.
5. Trace every scenario parameter into the production call graph.
6. Follow return values and scenario state into the exact assertion.
7. Inspect stated boundaries: wire parsing, stores, reopen/restart, network,
   signatures, mutations, and absence of partial effects.
8. Compare current behavior with the Gherkin text and normative protocol.
9. Strengthen byte-exact cryptographic cases with relevant vectors.
10. Freeze a per-scenario provisional verdict with direct evidence.

Do not classify a scenario as `PROVEN` because a function exists, a vector has
a similar name, or the global runner is green.

### Pass B — history and differential evidence

1. Read the prior public audit and run reports, if any.
2. Inspect relevant Git history and exact revision ranges.
3. Check whether history reveals missed paths, regressions, or intent.
4. Re-open the current code trace for any new path.
5. Reconcile Pass A and Pass B explicitly.
6. Run the final shared-state and cross-scenario integration check.

## Correction review

1. In a clean review unit, inspect the candidate's scenario, steps, production
   paths, protocol, and public surfaces without reading the correction diff or
   corrector conclusion.
2. Freeze Pass A behavioral verdicts for each assigned finding.
3. Read the immutable baseline and candidate revisions from state.
4. Inspect the exact diff and corrector report.
5. Map each change to a finding and its closure criteria.
6. verify that new tests would detect the old behavior.
7. Reproduce the announced gates in a clean context.
8. Check public paths, rejection paths, and partial effects.
9. Search for parallel bypasses in domain surfaces.
10. Accept or reject each finding independently.
11. Use `DECISION_REQUIRED` if closure requires a protocol or product choice.

Treat the corrector's conclusion as a handoff to verify, never as proof.

## Outputs

- Update the public audit.
- Keep finding identifiers stable.
- Add or update required Gherkin markers.
- Write a dated conclusion under the specialized role's `runs` directory.
- Include frozen Pass A verdicts, Pass B evidence, and reconciliation.
- Update state with the next action.
- On acceptance, enumerate files, symbols, formats, specification sections,
  and surfaces that may have cross-feature impact.
- On a required decision, present competing behaviors, evidence, and expected
  owner without choosing for them.

Do not implement production corrections during an audit or review.

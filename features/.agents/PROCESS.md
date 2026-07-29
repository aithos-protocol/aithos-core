# Manual process — semantic truth of Gherkin features

## Objective

Determine whether every existing passing scenario:

1. is actually selected and executed;
2. passes its parameters to the intended steps;
3. reaches a concrete production path;
4. asserts exactly the outcome stated by the scenario;
5. remains consistent with the Aithos protocol and its real public surfaces.

A green runner is necessary evidence, but it is never sufficient proof.

## Current scope

Include:

- empty, generic, or proxy steps;
- ignored Gherkin parameters;
- assertions weaker than the `Then`;
- one global result reused for multiple cases;
- real implementations that contradict the scenario or protocol;
- production surfaces that bypass the exercised verdict;
- RED tests required to make an existing scenario honest.

Exclude:

- general searches for behavior not described by an existing scenario;
- product or protocol enrichment unrelated to a semantic false positive;
- opportunistic refactoring;
- fixes in another domain unless the current finding requires them.

## Evidence hierarchy

Use this order when evidence conflicts:

1. the Gherkin scenario and its cited normative protocol requirements define
   the contract;
2. the current executable code establishes the behavior that actually exists;
3. independently reproduced tests establish which paths and assertions run;
4. Git history and prior reports explain how the state was reached.

Git history is context, not proof. A commit message, a previous verdict, or a
corrector's report cannot establish current behavior by itself.

## Required two-pass audit

Every initial audit and every correction review has two distinct passes.
The history-blind result must be written and frozen before the historical pass
begins.

### Pass A — current code, history-blind

Allowed inputs:

- the selected Gherkin scenario or review unit;
- its step definitions and runner configuration;
- the current production code reachable from those steps;
- current public surfaces, protocol specification, and normative vectors;
- `DOMAIN.md`;
- only the routing fields of `STATE.md`: mode, revision, assigned scope, and
  expected output path.

Before freezing the Pass A result, do **not** read:

- `git log`, `git show`, `git diff`, `git blame`, or commit messages;
- previous audit findings or conclusions;
- corrector run conclusions;
- the expected interpretation of a previous reviewer.

For each scenario, trace the behavior function by function:

1. confirm selection by the runner;
2. resolve every `Given`, `When`, and `Then` to its exact step definition;
3. follow each parameter into production calls and state mutations;
4. follow return values and stored state into the final assertion;
5. inspect rejection paths and prove the absence of unintended partial effects;
6. inspect the real public surfaces that claim the same invariant;
7. compare the observed behavior with the scenario and normative protocol;
8. record a provisional verdict and direct code/test evidence.

Freeze the Pass A section in the run report before starting Pass B. If a tool
or prior context already exposed history or old conclusions, disclose the
contamination and start a fresh review unit when practical.

### Pass B — historical and differential analysis

Only after Pass A is frozen:

1. inspect the exact baseline and candidate revisions;
2. read the relevant diff, commit messages, prior runs, and public findings;
3. verify RED/GREEN claims and determine whether tests would detect the old
   behavior;
4. use history to identify missed call paths, regressions, or intent;
5. re-open the current code trace when new evidence appears;
6. record agreements and disagreements with Pass A explicitly.

Pass B may strengthen or challenge a verdict only through newly identified
current-code or reproducible-test evidence. Historical intent alone cannot
upgrade a scenario to `PROVEN` or a correction to `VERIFIED`.

### Reconciliation

The final verdict must state:

- the frozen Pass A verdict;
- relevant Pass B evidence;
- the reconciled verdict and why it changed, if it changed;
- any unresolved contradiction or contaminated review unit.

## Review-unit isolation and impartiality

One uninterrupted run across an entire feature can create anchoring and
consistency bias: an early interpretation may silently become the template for
later scenarios. One isolated agent per scenario, however, can miss shared
steps, process-wide state, caches, and cross-scenario coupling.

Use this pragmatic model:

1. one feature auditor owns the complete inventory and final integration;
2. execute Pass A in fresh review units, preferably one Gherkin `Rule` or one
   coherent risk cluster of roughly three to six scenarios per unit;
3. give each fresh unit raw contract/code/spec inputs, never another unit's
   verdict or Git history;
4. freeze every unit's findings before aggregation;
5. run a separate integration pass over shared steps, helpers, mutable global
   state, `OnceLock`/cache behavior, hooks, and public surfaces;
6. give security-critical, `PARTIAL`, `SEMANTIC_FALSE_POSITIVE`, or disputed
   cases an independent challenger review when the risk justifies it.

For the initial manual pilot, a single agent is acceptable only if it enforces
the hard Pass A/Pass B barrier, writes separate per-Rule or risk-cluster notes,
and performs the shared-state integration pass last. A later orchestrator may
spawn fresh agents for the review units without changing the evidence model.

## Artifacts

| Artifact | Purpose |
|---|---|
| `features/<feature>.feature` | Contract and concise audit markers |
| `docs/audits/features/<feature>.md` | Public technical audit and stable findings |
| `.agents/<feature>/DOMAIN.md` | Durable domain knowledge |
| `.agents/<feature>/STATE.md` | Current stage and immutable revisions |
| `.agents/<feature>/<role>/runs/*.md` | Dated conclusions and handoffs |

The public audit is the stable technical source of truth. Run reports explain
who did what, on which revision, with which evidence, and what must happen
next.

## Manual lifecycle

```text
AUDIT_INITIAL
  → CORRECTION_REQUESTED
  → REVIEW_REQUESTED
      → CORRECTION_REQUESTED
      → or DECISION_REQUIRED
           → CORRECTION_REQUESTED
           → or REVIEW_ACCEPTED
      → or REVIEW_ACCEPTED
           → IMPACT_REVIEW_REQUESTED
           → COMPLETE
```

### Initial audit

The auditor:

1. freezes the revision and worktree state;
2. inventories the scenarios and divides them into review units;
3. completes and freezes history-blind Pass A for every unit;
4. completes Pass B and the shared-state integration pass;
5. classifies every scenario;
6. adds comments only to problematic scenarios;
7. writes or updates the public audit;
8. writes a dated conclusion;
9. sets `STATE.md` to `CORRECTION_REQUESTED`.

### Correction

The corrector:

1. reads only the explicitly assigned findings;
2. demonstrates each defect with a RED test when possible;
3. implements the smallest correction;
4. reruns targeted gates and relevant regressions;
5. documents the diff and exact results;
6. marks findings at most `IMPLEMENTED`;
7. requests an independent review;
8. sets `STATE.md` to `REVIEW_REQUESTED`.

### Correction review

The auditor:

1. performs history-blind Pass A against the candidate's current code;
2. freezes the behavioral verdict before reading the correction diff or run;
3. performs Pass B on the exact `baseline..candidate` range;
4. treats the corrector's conclusion as a claim to verify, not evidence;
5. reruns tests in a clean context;
6. checks public surfaces and partial effects;
7. accepts or rejects each finding separately;
8. marks `VERIFIED` only after independent proof;
9. records affected files, symbols, formats, and surfaces.

A rejected review returns to the corrector. After three rejections for the
same finding, stop the automatic cycle and request human direction.

### Decision required

Use `DECISION_REQUIRED` when a finding cannot be closed without choosing
between competing protocol, security, or product semantics.

In that state:

1. the auditor documents the competing behaviors and evidence;
2. no corrector chooses the semantics implicitly;
3. `STATE.md` names the human decision owner as the next role;
4. the decision is recorded before a new round;
5. the state then becomes `CORRECTION_REQUESTED` or `REVIEW_ACCEPTED`.

### Impact review

Only after acceptance, the global reviewer:

1. reads the accepted audit, run reports, and diff;
2. searches other features for shared steps, helpers, symbols, formats,
   vectors, or specification sections;
3. classifies each impact as `NONE`, `TARGETED`, or `FULL_AUDIT`;
4. writes a global report;
5. does not modify or restart any feature.

The decision to restart an audit remains manual.

## Evidence statuses

| Status | Meaning |
|---|---|
| `PROVEN` | The scenario exercises and proves its exact contract |
| `PARTIAL` | A stated boundary or invariant is not exercised |
| `SEMANTIC_FALSE_POSITIVE` | The scenario passes without proving its stated outcome |
| `PROXY` | The scenario consumes a shared verdict without executing its own case |
| `IMPLEMENTED` | A candidate correction exists and requires review |
| `VERIFIED` | The auditor independently reproduced and accepted the correction |
| `DECISION_REQUIRED` | A human owner must decide semantics before correction |

## Required run conclusion

Every run report states:

- run type and role;
- date;
- observed revision, baseline, and candidate when applicable;
- worktree state;
- scope and review-unit identifiers;
- Pass A inputs, traces, frozen provisional verdicts, and contamination status;
- Pass B inputs, differential evidence, and reconciliation;
- exact commands and results;
- findings handled and not handled;
- affected files and symbols;
- limits of the conclusion;
- next action and expected skill.

A report reconstructed after the fact must be marked `RECONSTRUCTED`. It
separates directly observable facts from results merely reported by another
agent. A reconstructed report cannot retroactively claim an uncontaminated
history-blind Pass A.

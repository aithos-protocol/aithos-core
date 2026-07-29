---
name: review-gherkin-impacts
description: Analyze possible cross-feature effects of a Gherkin correction already accepted by its specialized auditor. Use this skill after a VERIFIED review to compare the accepted diff with other features, steps, helpers, APIs, formats, vectors, and specification sections, then write a manual report without modifying or restarting other audits.
---

# Review cross-feature impacts

## Entry conditions

1. Read `../../../PROCESS.md` completely.
2. Require an auditor conclusion containing `REVIEW_ACCEPTED`.
3. Require immutable baseline and accepted candidate revisions.
4. Stop if the correction is only `IMPLEMENTED`.

## Analysis

1. Inspect the accepted diff.
2. Extract changed files, functions, types, steps, formats, and vectors.
3. Search for their use in all runners and `.feature` files.
4. Cross-reference specification sections cited by other audits.
5. Distinguish textual proximity from a semantic dependency.
6. Classify each feature:
   - `NONE`: no credible dependency;
   - `TARGETED`: a few specific scenarios should be reviewed;
   - `FULL_AUDIT`: a shared helper, API, format, or invariant changed.

Do not rerun feature, global Cucumber, or workspace gates. The accepted
review already owns independent behavioral evidence; this role performs
dependency analysis and recommends any follow-up gates.

## Output

Write a dated report under `../runs/` containing:

- baseline and accepted candidate;
- canonical feature branch and its recorded `main` base;
- source audit and review;
- changed elements;
- searches performed;
- potentially affected features and evidence;
- manual recommendation.

Do not change code, audits, or feature files. Do not launch another agent.
After human acceptance, the orchestrator may mark the cycle complete and
integrate the canonical feature branch into local `main`. Start the next
feature branch only from that updated `main`.

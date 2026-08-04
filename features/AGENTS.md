# Gherkin feature domain

These instructions apply to all work started from `features/`.

## Project stage — read this before costing anything as a breaking change

**As of 2026-08-04, `aithos-core` is `0.1.0-alpha.1`. Nothing is deployed, no
edition has been published by anyone, and there is no user whose data a change
could invalidate.** Verified rather than assumed: `git ls-files` returns no
serialised `header.json` or `hdr/*.json`, and the crate version is in
`rust/Cargo.toml`.

Two consequences, and they are meant to save you work:

1. **Do not weigh backward compatibility.** A change that would break an
   existing holder of data breaks nobody. Do not propose a migration path, a
   grandfather clause, a legacy profile, or a compatibility shim to protect a
   population that does not exist. Say plainly that the cost is nil today, and
   move on.
2. **Do not soften a correction to spare the past.** The right shape now is the
   one you would choose with no history at all. The window in which that is free
   is exactly this one, and it closes on its own.

What still counts, and is not covered by the above: a change that breaks the
**repository's own** tests, vectors, or pinned digests is a real cost and you
cost it normally. And a rule the implementation cannot satisfy is a defect
whatever the user count — an unsatisfiable obligation is a trap, not a
tightening, and it stays a defect at zero users.

**This section expires.** It stops being true the day a first edition is
published outside this repository, or the crate leaves `alpha`. Whoever
publishes that edition removes this section in the same change; until then any
role may rely on it. If you find it still here while either condition has
passed, that is itself a finding — report it rather than obey it.

## Branch isolation

- Start each initial feature audit on
  `codex/audit-<feature-name>`, created from the current local `main`.
- Use one feature per branch and a dedicated worktree when another task is
  active; never audit on `main` or on a shared pilot branch.
- Record the exact `main` base and audit revision in the domain state and run
  report. Never silently rebase already collected audit evidence.
- Create correction branches as
  `codex/fix-<feature-name>-<finding-or-scope>` from the immutable audited
  feature revision.
- After independent review and impact acceptance, integrate the feature branch
  into local `main` before starting the next feature.
- Preserve unresolved findings and their live Gherkin markers across that
  integration.

## Feature identity and test selection

- Every `features/<name>.feature` starts with the unique tag `@<name>`.
- Run `features/.agents/scripts/verify-feature-tags.sh` before feature work.
- Use `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test
  cucumber -- --tags @<name>` for the feature gate.
- The auditor runs that feature gate once per immutable revision, not after
  every scenario, and never runs unfiltered Cucumber or the workspace.
- The corrector/execution agent owns focused RED/GREEN, relevant regressions,
  and one unfiltered Cucumber/workspace gate before handoff.

Every new feature agent must reuse the shared audit/correction skills and
declare its canonical branch, canonical tag, focused tests, relevant
regressions, and final global gates in `DOMAIN.md`.

## Mandatory routing

Before auditing or correcting a feature:

1. read `.agents/PROCESS.md`;
2. locate its domain under `.agents/<feature-name>/`;
3. read `DOMAIN.md` and the routing fields in `STATE.md`;
4. load the specialized skill named by `STATE.md`;
5. respect the requested role and do not anticipate the next stage.

For `a-identity.feature`:

- audit or review:
  `.agents/a-identity/auditor/audit-a-identity/SKILL.md`;
- correction:
  `.agents/a-identity/corrector/correct-a-identity/SKILL.md`.

For `b-derivation.feature`:

- audit or review:
  `.agents/b-derivation/auditor/audit-b-derivation/SKILL.md`;
- correction:
  `.agents/b-derivation/corrector/correct-b-derivation/SKILL.md`.

For `c-headers.feature`:

- audit or review:
  `.agents/c-headers/auditor/audit-c-headers/SKILL.md`;
- correction:
  `.agents/c-headers/corrector/correct-c-headers/SKILL.md`.

For `g4-client-surfaces.feature`:

- audit or review:
  `.agents/g4-client-surfaces/auditor/audit-g4-client-surfaces/SKILL.md`;
- correction:
  `.agents/g4-client-surfaces/corrector/correct-g4-client-surfaces/SKILL.md`.

## Role boundaries

- The auditor inspects, classifies, documents, and reviews. It does not correct
  production code or run global regression suites.
- The corrector implements only the requested findings. It may mark a finding
  `IMPLEMENTED`, never `VERIFIED`, and owns the final regression gates.
- The impact reviewer runs only after an accepted review. It reports other
  features that may be affected without changing or restarting them.
- If a review exposes a protocol or product choice, set the domain to
  `DECISION_REQUIRED`. A corrector must not make that choice implicitly.

The current audit scope is limited to the semantic truth of existing passing
scenarios. Searching for entirely missing scenarios is out of scope.
Additional tests needed to prove a requested correction remain in scope.

Live audit comments in `.feature` files point to the public audits under
`docs/audits/features/`. Remove those comments when the finding is verified;
the resolved history remains in the public audit, dated run reports, and Git.
Operational conclusions and handoffs stay under `.agents/`.

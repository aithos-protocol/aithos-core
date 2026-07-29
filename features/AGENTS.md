# Gherkin feature domain

These instructions apply to all work started from `features/`.

## Feature identity and test selection

- Every `features/<name>.feature` starts with the unique tag `@<name>`.
- Run `features/.agents/scripts/verify-feature-tags.sh` before feature work.
- Use `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test
  cucumber -- --tags @<name>` for the feature gate.
- Run that feature gate once per immutable revision, not after every scenario.
- Run the unfiltered Cucumber and workspace gates once at final integration,
  never once per scenario, Rule, review unit, or audit Pass.

Every new feature agent must reuse the shared audit/correction skills and
declare its canonical tag, focused tests, relevant regressions, and final
global gates in `DOMAIN.md`.

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

## Role boundaries

- The auditor inspects, classifies, documents, and reviews. It does not correct
  production code.
- The corrector implements only the requested findings. It may mark a finding
  `IMPLEMENTED`, never `VERIFIED`.
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

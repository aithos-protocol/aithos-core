# Gherkin feature domain

These instructions apply to all work started from `features/`.

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

Comments in `.feature` files point to the public audits under
`docs/audits/features/`. Operational conclusions and handoffs stay under
`.agents/`.

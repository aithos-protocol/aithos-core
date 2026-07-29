# Reconstructed conclusion — initial audit of `a-identity.feature`

| Field | Value |
|---|---|
| Type | `RECONSTRUCTED` |
| Source role | semantic auditor |
| Audit date | 2026-07-29 |
| Observed revision | `2fee855`, with a reported dirty worktree |
| Documentation commit | `be2d098` |
| Public audit | `docs/audits/features/a-identity.md` |
| Result | `CORRECTION_REQUESTED` |

## Provenance and two-pass limitation

This conclusion was reconstructed after the audit from commit `be2d098`,
feature comments, the public audit, and recorded results. It was not produced
natively by `audit-a-identity`.

The original work traced the current Rust paths and used Git state for
reproducibility, but it did not freeze formally isolated history-blind review
units before consulting existing context. It must therefore not be presented
retroactively as an uncontaminated Pass A under the newer two-pass process.
The observable technical findings remain valid inputs to a fresh review.

## Audit conclusion

All nine scenarios were selected and executed real Rust production code. No
step was empty, tagged `@wip`, or replaced by a global `OnceLock` verdict.

The green result did not establish the complete contract:

- 6 scenarios were `PROVEN`;
- 2 scenarios were `PARTIAL`;
- 1 scenario was a `SEMANTIC_FALSE_POSITIVE`.

## Open findings

| Finding | Verdict | Requested correction |
|---|---|---|
| `AID-001` | `PARTIAL` | Close the DID schema and validate version, signature metadata, and all four key codecs |
| `AID-002` | `SEMANTIC_FALSE_POSITIVE` | Actually verify the previous document, transition, and presented successor document |
| `AID-003` | `PARTIAL` | Remove succession derivation from the owner master secret |
| `AID-004` | `DECISION_REQUIRED` | Define and enforce genuinely cold custody |
| `AID-005` | insufficient proof | Add the tests required to demonstrate AID-001 and AID-002 |

AID-005 is included only as correction evidence for existing scenarios, not as
a general search for missing tests.

## Recorded evidence

```text
Targeted runner:
1 feature
6 rules
9 scenarios (9 passed)
30 steps (30 passed)

cargo test -p aithos-core --test a1_genesis --test a2_did
a1_genesis: 4 passed
a2_did:     3 passed
```

Temporary negative probes reported:

```text
signed malformed non-root keys accepted: true
signed wrong version/alg/fragment accepted: true
unknown unsigned wire field ignored and accepted: true
transition to malformed DID accepted: true
transition to same DID accepted: true
```

The temporary probes were not retained in the repository. Durable RED tests
were part of the requested correction.

## Produced artifacts

- audit comments and tags in `features/a-identity.feature`;
- public audit `docs/audits/features/a-identity.md`;
- stable identifiers AID-001 through AID-005;
- closure criteria and expected RED tests.

## Requested handoff

Run `correct-a-identity` for AID-001, AID-002, and the AID-005 evidence.
Do not address AID-003/AID-004 without an architecture decision. Then request
an independent review from `audit-a-identity`.

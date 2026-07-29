# Domain state — `a-identity`

| Field | Value |
|---|---|
| Status | `COMPLETE` (round 2 audit and impact-review cycle) |
| Expected mode | none |
| Round | 2 |
| Initial audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Reviewed round 1 candidate | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Round 2 protocol baseline | `083c1a197a39f7a8efde957eddf5af05b825e3ea` |
| Immutable correction baseline | `dfb79c87120caeb26737c81babd5cc2ad0dc0a3c` |
| Candidate commit | `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` |
| Protocol decision | `decisions/2026-07-29-aid-001-provider-epoch-transition.md` |
| Assigned finding | `AID-001` Provider remainder only |
| Candidate finding status | `AID-001` Provider remainder is `VERIFIED` |
| Findings already verified | `AID-002`, `AID-005` (pilot scope) |
| Findings outside correction | `AID-003`, `AID-004` |
| Next role | manual follow-up owner for `AID-003` / `AID-004` or a new explicitly requested round |
| Correction conclusion | `corrector/runs/2026-07-29-correction-02.md` |
| Accepted review conclusion | `auditor/runs/2026-07-29-audit-review-02.md` |
| Accepted impact review | `../orchestrator/runs/2026-07-29-a-identity-impact-review.md` |

## Inputs

- public audit: `docs/audits/features/a-identity.md`;
- binding protocol decision:
  `decisions/2026-07-29-aid-001-provider-epoch-transition.md`;
- independent round 1 review:
  `auditor/runs/2026-07-29-audit-review-01.md`;
- reconstructed round 1 correction:
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.

## Delivered round 2 outcome

Candidate `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` implements only
the decided Provider semantics for AID-001:

1. every incoming DID document remains root-signed and passes strict Core
   `DidDocument::verify`;
2. succession-signed same-DID `did.json` is refused;
3. a strict-Core-valid but byte-different same-DID document is refused
   `immutable_conflict`;
4. `did.json` storage is atomic write-once and refusal scenarios prove genesis
   absence or byte-exact preservation of the stored document;
5. P9 vectors and Provider scenarios express the decided behavior.

No canonical public storage/transport path for the complete previous document /
`EpochTransition` / successor document triplet can be derived from the current
Provider protocol. The candidate therefore removes the invalid same-DID path
and stays fail-closed. A future public epoch-acceptance surface must define:

1. its artifact URI or request form;
2. the control-plane binding change to the successor DID;
3. one atomic cross-DID commit for transition plus successor document;
4. CAS, replay, and single-use semantics.

That future surface must reuse Core
`EpochTransition::verify_succession(prev_doc, next_doc)` before committing any
part of the triplet.

`AID-002` and `AID-005` were not changed. `AID-003` and `AID-004` remain
outside this correction.

## Accepted review

`audit-a-identity` reviewed the exact immutable range:

```text
dfb79c87120caeb26737c81babd5cc2ad0dc0a3c..
e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1
```

The fresh history-blind Pass A accepted the Provider remainder. Pass B
reproduced the candidate gates, established that the new scenarios detect the
baseline behavior, reconciled the protocol decision and exact diff, and
retained the verdict. AID-001 is `VERIFIED` for the assigned Provider
remainder.

The canonical Provider transport/storage contract for the previous document /
`EpochTransition` / successor document triplet remains undefined and
fail-closed. `AID-003` and `AID-004` remain open.

## Completed impact review

The accepted impact review classified all other Gherkin surfaces and found no
`FULL_AUDIT` dependency. It recommends only the separately tracked §10.4
wording clarification and narrow Gateway replication regressions.

This completes the round 2 audit and impact-review cycle. It does not relabel
`AID-003` or `AID-004`: they remain visible, unresolved findings and require a
new explicit decision/correction round before their markers may be removed.

# Domain state — `a-identity`

| Field | Value |
|---|---|
| Status | `REVIEW_REQUESTED` |
| Expected mode | `review` |
| Round | 2 |
| Initial audit baseline | `be2d098eeb79107c861462a6433df9ef45871265` |
| Reviewed round 1 candidate | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Round 2 protocol baseline | `083c1a197a39f7a8efde957eddf5af05b825e3ea` |
| Immutable correction baseline | `dfb79c87120caeb26737c81babd5cc2ad0dc0a3c` |
| Candidate commit | `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` |
| Protocol decision | `decisions/2026-07-29-aid-001-provider-epoch-transition.md` |
| Assigned finding | `AID-001` Provider remainder only |
| Candidate finding status | `AID-001` is `IMPLEMENTED`, pending independent review |
| Findings already verified | `AID-002`, `AID-005` (pilot scope) |
| Findings outside correction | `AID-003`, `AID-004` |
| Next role | `audit-a-identity` |
| Correction conclusion | `corrector/runs/2026-07-29-correction-02.md` |
| Expected review conclusion | `auditor/runs/2026-07-29-audit-review-02.md` |

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

## Review handoff

Run `audit-a-identity` in `review` mode on the exact immutable range:

```text
dfb79c87120caeb26737c81babd5cc2ad0dc0a3c..
e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1
```

The reviewer must begin with fresh history-blind Pass A units and freeze those
results before reading the correction report, prior audits, protocol decision,
or `baseline..candidate` diff. Pass B may then inspect this handoff and exact
range. Do not promote AID-001 above `IMPLEMENTED` until that two-pass review is
complete.

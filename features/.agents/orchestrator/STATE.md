# Gherkin impact-review state

| Field | Value |
|---|---|
| Status | `COMPLETE` (b-derivation round 1 cycle) |
| Source feature | `b-derivation.feature` |
| Impact report | `runs/2026-07-29-b-derivation-impact-review.md` |
| Human acceptance | 2026-08-02 (Mathieu) |
| Integration | audit branch content on local `main` (correction `3d6fa51`, review `ae88f7f`, candidate tip `1ab331a`, impact review `7854895`) |
| `BDER-011` cross-cutting lot | done — fix `78c06ba`, merge `090d11a`, `VERIFIED` `c630753` (2026-07-30) |
| Decisions recorded | `b-derivation/decisions/2026-08-02-bder-006-tag-view-rule-scope.md` ; `b-derivation/decisions/2026-08-02-bder-008-b2-provenance.md` |
| Next cycle | `b-derivation` round 2 correction (`BDER-006` retitle + `BDER-008` provenance), then independent review, then impact review |

## Tracked follow-ups

1. **`d-bundle` targeted follow-up (widened by the BDER-006 decision):** its
   future cycle must record the co-owned steps (impact report §9.5) **and add
   the tag-view/`wrap` scenarios proving the behavioral half of spec §02.9**.
   Restarting that audit remains a manual decision.
2. **Future B2 generator lot:** commit an independent, named
   `gen-b2-derivation.py` (closes `BDER-007`), and wire the existing B2
   cross-check guards of `gen-f/g/h/h2/i` into CI. No deadline imposed by this
   cycle (see the BDER-008 decision).
3. Remaining from the impact report: record the Gherkin layer's dependency on
   `vectors/b2-derivation.json` in `vectors/README.md` or the `b-derivation`
   `DOMAIN.md`; verify that the a-identity `DOMAIN.md` alignment and the
   `EXIT=0` annotation were carried by the BDER-011 lot, and do them if not.

## Previous cycles

- `a-identity` round 2 accepted 2026-07-29
  (`runs/2026-07-29-a-identity-impact-review.md`, no `FULL_AUDIT`); `AID-003`
  and `AID-004` remain open; follow-ups in
  `docs/HANDOFF-A-IDENTITY-IMPACT-FOLLOWUPS-2026-07-29.md`.
- `b-derivation` round 1 accepted 2026-08-02 (this file); open findings
  `BDER-006`/`BDER-008` assigned to round 2, `BDER-007`/`BDER-010`/`BDER-012`
  survive visibly per `PROCESS.md`.

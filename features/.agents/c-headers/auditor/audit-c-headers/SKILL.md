---
name: audit-c-headers
description: Audit or review only features/c-headers.feature using its sealed-line, owner-line (I3), O(1) grant, rotation, and up-link wrap invariants. Use this skill when features/.agents/c-headers/STATE.md routes an initial audit or an independent correction review to the Headers auditor.
---

# Audit `c-headers.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, branch, base revision, observed revision, assigned scope,
   and output routing from `../../STATE.md`.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction reports, or Git
history until the shared skill authorizes Pass B. The yardstick branch
`codex/audit-c-headers` (`../../../orchestrator/QUEUE.yaml:55-56`) is a Pass B
input and a milestone comparison only. Opening it before the freeze
contaminates the review unit; disclose it and restart the unit.

## Initial-audit mode

- Divide `c-headers.feature` into four review units aligned with its Gherkin
  `Rule` blocks: one line seals to exactly one recipient (four scenarios); the
  owner line is mandatory (one); grant is one appended line (one); rotation
  cuts the revoked and re-links the parent (two).
- The canonical gate must select 1 feature / 4 rules / 8 scenarios / 28 steps.
  A different count means the contract you traced is not the contract that ran.
- For each scenario, resolve the phrase to its exact step definition, follow the
  Gherkin parameters into `Header::build` / `build_at` / `append_line` /
  `rotate` / `open` / `open_latest` / `check_rotation` / `validate` and
  `Wrap::seal` / `Wrap::open`, and follow the returned key material into the
  final assertion.
- Three `Given` bodies are empty and the fixtures are module constants
  (`cucumber.rs:258-285`). Establish, for each such scenario, whether the
  `Given` clause named in the Gherkin actually constrains anything, or whether
  the `When` reconstructs the whole state on its own.
- Distinguish the three proofs a negative scenario can offer: the AEAD tag
  rejecting, an absent line, and a routing filter finding no candidate. Only
  the first proves the seal is what grants (`spec/03-headers.md:33-35`). A
  rejection observed through `open_into` says only that `Header::open` returned
  `Err`; establish *which* mechanism produced it.
- For "a line is bound to its node and version", check that the AAD really
  carries `subject_did ‖ node ‖ key_version`
  (`aithos_core::seal::line_aad`) and that the replay scenario changes the node
  and nothing else. A rejection caused by a mismatched ephemeral or nonce would
  prove nothing about binding.
- For "the owner line is byte-identical to before", check what `saved_line`
  was captured from and when, and that the comparison is over the whole `Line`
  struct rather than one field.
- For I3, the assertion checks that the error message contains `I3`
  (`cucumber.rs:12347`). Decide whether asserting on a `Display` string proves
  the invariant, and whether the parse-time path `Header::validate` — the one
  every reader calls — is exercised at all by this feature.
- For rotation, `Header::rotate` and `Header::check_rotation` are two different
  guards (`header.rs:192`, `:275`). Establish which of them the scenarios
  actually reach, and whether the "smuggled recipient" rule of
  `spec/03-headers.md:93-96` is exercised anywhere inside this feature.
- For the up-link wrap, the scenario builds a `Wrap` and opens it under the
  same in-memory `PARENT_KEY` constant. Establish whether that proves the wrap
  "restores derivation for the parent holder" or only that the AEAD
  round-trips, and whether the parent key is ever obtained by real derivation
  (`derive_key`) rather than by fixture.
- Strengthen byte-exact cases with `vectors/c1-header-seal.json` rather than
  with self-consistency between two calls of the same function. Note that no
  step of this feature reads that vector: byte-exactness lives in
  `rust/crates/aithos-core/tests/c1_header_seal.rs`. Say so explicitly rather
  than crediting the feature with the vector's proof.
- Check that scenario state does not survive between scenarios through the
  shared Cucumber World: `header`, `saved_line`, `opened`, `wrap_obj`, and the
  shared `rejection` field, plus the step functions reused under two Gherkin
  phrases.
- Freeze per-unit Pass A notes before reading any prior material.
- Document discrepancies under stable `CHDR-*` identifiers
  (`docs/audits/features/README.md:20`).
- Run the final integration check across the shared header steps, World state,
  and the Bundle, CLI, and Core surfaces listed in `DOMAIN.md` — in particular
  the paths that call `validate` before `open` and the ones that do not.
- Create `docs/audits/features/c-headers.md` and add the index row in
  `docs/audits/features/README.md`.
- Write the conclusion under `../runs/`.

## Correction-review mode

- Before reading the correction report or diff, trace the candidate's current
  behavior for the scenarios and surfaces assigned by state.
- Freeze the Pass A verdict for each finding.
- Then compare the baseline and candidate revisions from `../../STATE.md`.
- Run the canonical feature gate from `DOMAIN.md` once on the immutable
  candidate. Run a focused test from `c1_header_seal` only to resolve a
  semantic contradiction.
- Do not run the corrector's unfiltered Cucumber or workspace gates.
- Verify that each new test would fail on the baseline for the intended reason.
- Check that no correction weakened a fail-closed path: a header that opens
  where it previously rejected is a regression even if a scenario turns green.
- Check the rotation and wrap consumers named in `DOMAIN.md`
  (`revoke.rs`, `structure.rs`, `vault.rs`, `session.rs`, `log.rs`,
  `grants.rs`, the two CLI commands) for a parallel bypass of the corrected
  verdict.
- Do not close a finding that was not explicitly assigned.
- Do not modify Rust files, `spec/`, or `vectors/`.
- Advance a finding in the public audit to the top acceptance status of the
  `PROCESS.md` "Evidence statuses" table only for findings you reproduced
  independently. Never advance one on the corrector's word.
- Write `../runs/<date>-audit-review-<round>.md`.
- Set state to `CORRECTION_REQUESTED`, `DECISION_REQUIRED`, or
  `IMPACT_REVIEW_REQUESTED`.

The corrector's conclusion is informative. Reconstruct the final verdict from
the current code, `spec/03-headers.md`, independently reproduced tests, and
only then the diff.

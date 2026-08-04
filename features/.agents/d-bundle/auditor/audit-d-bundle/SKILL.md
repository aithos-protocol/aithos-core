---
name: audit-d-bundle
description: Audit or review only features/d-bundle.feature using its bundle and edition invariants — a linear hash-pinned edition chain verifiable offline, a keyless public zone, a structureless self zone, owner parity across the three zones without a mandate, one logical commit point for state and Gamma, and narrow purpose-bound capabilities with confined paths. Use this skill when features/.agents/d-bundle/STATE.md routes an initial audit or an independent correction review to the Bundle auditor.
---

# Audit `d-bundle.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, branch, base revision, observed revision, assigned scope,
   and output routing from `../../STATE.md`, plus its § *Recorded follow-ups
   this feature already owes* — those name debts, not verdicts, and are routing
   material.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction reports, or Git
history until the shared skill authorizes Pass B. This feature has **no
yardstick branch**: `../../../orchestrator/QUEUE.yaml` registers one for
`c-headers` only. Its ordinary Pass B material is the diff and the four
impact-review reports of other features that name it. Opening any of them before
the freeze contaminates the review unit; disclose it and restart the unit.
`origin/codex/bundle-publication-performance` is an unrelated product branch,
not a yardstick, and never an input.

**Run no gate yourself.** Name the exact command from `../../DOMAIN.md`,
§ *Gate pyramid*, and stop. The orchestrator runs it, hashes the transcript and
journals it under an `evidence_id` (`../../../orchestrator/LEDGER.md`). Cite
the `evidence_id`. A command with no matching ledger entry is not evidence.

## Initial-audit mode

- Divide the feature into review units aligned with its seven `Rule` blocks,
  per `../../PROCESS.md`, § *Review-unit isolation and impartiality*. Seven
  units is within the three-to-six-scenario guidance for the first four, but
  three of the `Rule`s are large `Scenario Outline`s and one unit per `Rule`
  keeps the fixture shared inside a unit rather than across units:
  **RU-1** editions chain and verify offline (`:8`, four scenarios) ·
  **RU-2** content round-trips through the sealed store (`:32`, two) ·
  **RU-3** the public zone reads without any key (`:45`, one) ·
  **RU-4** the self zone leaks no structure (`:53`, one) ·
  **RU-5** owner parity across the three zones (`:61`, one outline, 15 rows) ·
  **RU-6** one transaction for state and Gamma (`:89`, two outlines, 12 + 2
  rows) · **RU-7** narrow capabilities and confined paths (`:129`, two
  outlines, 4 + 10 rows). Then run the shared-state integration pass over all
  seven.
- The count of the file on disk is 1 feature / 7 rules / 51 expanded scenarios /
  299 steps. Establish what the gate actually selected before tracing anything.
  A different count means the contract you traced is not the contract that ran,
  and `../../../orchestrator/LEDGER.md:44-52` treats exit 0 with zero scenarios
  selected as **red**. Record the transcript; do not substitute `DOMAIN.md`'s
  arithmetic for it.
- Resolve every phrase to its exact step definition. `DOMAIN.md`, § *Shared
  steps, fixtures, and helpers*, records that all 61 step lines resolve and
  names the shared definitions; **reproduce that search rather than trusting
  it**, and establish what `.fail_on_skipped()` (`cucumber.rs:20029`) would do
  to a phrase that did not.
- **Three step definitions carry more than one Gherkin phrase**, and one carries
  four: `a_published_bundle` (`cucumber.rs:7706`, two `Given` phrases),
  `publish_edition` (`:8343`, two `When` phrases), `edition_verifies`
  (`:12697`, two `Then` phrases — `edition 1 verifies offline` and `its
  integrity checks against the signed edition`), and `d_capability_boundary_holds`
  (`:8477-8481`, one `regex =` alternative covering `:136`, `:137`, `:138`,
  `:139`). For each, establish whether the two contracts are the same contract.
  `a bundle with two editions` and `a published bundle` reaching the same body
  is a statement about the fixture, and `edition 1 verifies offline` and `its
  integrity checks against the signed edition` sit in different `Rule`s.
- **One `Then` is declared `#[then(expr = "{string}")]`** — `d_capability_result`
  (`:8450`), whose entire pattern is the `<observable_result>` `Examples`
  column. Establish what that pattern matches across the whole suite, not only
  in this feature, and what it means for a scenario's verdict that the expected
  outcome arrives as a string compared against a string the same helper
  produced.
- For RU-1, distinguish the three ways an edition can be refused: a broken
  `prev_hash`, an altered pinned file, and an I3-violating pinned header.
  `Bundle::verify` (`bundle.rs:1691`) does all three in one function; a
  `verify().is_err()` assertion (`:12738`) says only that one of them fired.
  Establish which. And establish whether the tamper step
  (`alter_pinned_file`, `:8349`, flipping a bit in `e/circle/index.json`
  through the `pub` `store` field) reaches the pinned-file check or an earlier
  parse failure.
- For RU-2, the rename scenario is where three features meet. `rename_the_folder`
  (`:8394`), `publish_edition` (`:8343`) and `reads_at_new_path` (`:12748`) are
  named by `bder-006-d-bundle` as co-owned steps. Establish whether reading at
  the new path proves that the key followed the sid, or only that the read
  succeeded — the two differ exactly when the section key is re-derived from the
  new label.
- For RU-3, `stranger_reads_public` (`:8405`) calls
  `Bundle::<MemStore>::public_read` with no `OwnerKeys` in the step. Establish
  whether "no key at all" is proved by the absence of a key in the step's
  arguments or by the function's inability to use one, and whether
  `its integrity checks against the signed edition` — which resolves to the
  shared `edition_verifies` — checks the section's own signature (`spec/02.11`:
  the public signature ships in the index row) or the edition chain.
- For RU-4, `self_leaks_nothing` (`:12765`) tests five hard-coded needles
  against the concatenation of every object under `e/self/`
  (`inspect_self_zone`, `:8414`). Establish what a leak that is not one of those
  five strings would do, and whether the concatenation covers the index, the
  descriptors and the blobs or only some of them. Then establish whether
  `owner_reconstructs_tree` (`:12781`) reconstructs from sealed descriptors, as
  the `Then` says, or from a clear index.
- For RU-5, the `Given` `core_owner_fixture` (`:11491`) sets one boolean and
  the real fixture is built inside `core_owner_scenario` (`:3361`). Establish,
  for each of the fifteen rows, whether the Gherkin `<zone>` and `<operation>`
  reach production and what `core_owner_gamma` (`:11528`) proves: it asserts
  `gamma_delta` equals one for `create|edit|delete` and zero otherwise, and
  `mandate_counter_delta == 0`. Decide whether a counter delta of zero over a
  path that never touches a mandate proves "without consuming mandate counters".
- For RU-6, the twelve failure rows and the two success rows all funnel through
  `core_atomic_failure_scenario` (`:1822`) / `core_atomic_success_scenario`
  (`:1936`) and are read back through one `CoreAtomicObservation` (`:313`).
  Four different `Then`s reduce to `canonical_unchanged` or
  `partial_state_observed` — `core_atomic_unchanged` (`:11393`),
  `core_atomic_old_head` (`:11407`), `core_atomic_no_failed_artifact`
  (`:11416`), `core_atomic_staging_clean` (`:11422`). Establish for each
  whether the Gherkin clause it carries is narrower than the boolean it reads:
  "no failed-mutation blob, index, header, wrap or Gamma entry exists" names
  five artifact kinds, and `canonical_unchanged` names none.
- Still in RU-6: `core_atomic_boundary` (`:11355`) branches on
  `w.core_revocation_failure_boundary == "__fixture__"` (`:562`), a World field
  another feature owns. That is process-wide state inside a `Given` of this
  feature. Take it to the integration pass and establish the ordering under
  which it can fire.
- For RU-7, the four capability rows set `cross_class_substitution_refused`
  from `core_capability_api_is_narrow()` (`:2053-2058`), which is
  `include_str!("../src/session.rs")` searched for three literals.
  `../../../orchestrator/QUEUE.yaml` records that class as
  `chdr-lota-source-text-assertions` and names this exact site; its scope limit
  — counted, not classified — applies. Establish what class of regression such
  an assertion can and cannot detect, and do that for this site specifically
  rather than adopting the queue's verdict about another one.
- Still in RU-7: the ten path rows include four requiring real symlinks, and
  `core_path_fs_scenario` exists twice — `#[cfg(unix)]` at `:3202` and
  `#[cfg(not(unix))]` at `:3340`, the second returning an `Err`. Establish what
  the non-Unix arm does to a scenario's verdict, and whether the observation's
  `rejected` and `!outside_access_observed` distinguish "rejected by the
  grammar before any store access" from "the store access happened and returned
  nothing".
- Check whether any verdict this feature consumes is one of the five
  process-global `OnceLock` verdicts (`cucumber.rs:1119-1128`, helpers
  `:7295-7350`). `QUEUE.yaml`'s `chdr-lota-proxy-verdicts` does not list this
  feature and `DOMAIN.md` records the search that agrees; confirm it against
  the code rather than against either.
- Strengthen byte-exact cases with the vectors rather than with self-consistency
  between two calls of the same function. Four vectors are read by this
  feature's steps (`DOMAIN.md`, § *Vectors involved*); establish, per scenario,
  whether the vector supplies an oracle or only a row-existence guard —
  `core_atomic_failure_scenario` (`:1822-1836`) and `core_owner_scenario`
  (`:3361-3383`) both begin by refusing a row that is absent from the vector,
  which is a different thing from checking a result against it. Say so
  explicitly rather than crediting the feature with the vector's proof.
- Establish what `Bundle`'s `pub` `store` field (`bundle.rs:284`) means for
  every scenario whose fixture or `When` writes through it: a write that
  bypasses `validate_store_key` is not the production path.
- **Discharge the recorded follow-ups of `../../STATE.md` explicitly.** There
  are seven naming this feature, and two of them require a statement this cycle
  alone can make: `chdr-016-grant-path` must say whether `d-bundle` or
  `g-revocation` carries `CHDR-016`, and `chdr-028` falls to this cycle because
  `d-bundle` precedes `k-integration` in the queue. For each follow-up, state
  whether it yields a finding of this feature, a debt routed elsewhere, or
  nothing. A follow-up left unmentioned in the run report is a follow-up
  dropped.
- The three lifted embargoes — `CHDR-028`, `SC-12`, the code half of `SC-05` —
  are **published in full** and may be cited. Do not re-embargo them. Read
  `docs/audits/features/c-headers.md` §6bis in Pass B, never inside a Pass A
  unit, and record where you read it.
- Freeze per-unit Pass A notes before reading any prior material.
- Document discrepancies under stable `DBND-*` identifiers.
- Run the final integration check across the shared World state
  (`bundle`, `read_body`, `inspected`, the four `core_*_observation` fields, and
  `core_revocation_failure_boundary`), the shared step definitions, and the
  Bundle, CLI and Core surfaces listed in `DOMAIN.md`, § *Public surfaces* — in
  particular the three edition verifiers and which of them call
  `verify_pinned_headers`.
- Create `docs/audits/features/d-bundle.md` and add the index row in
  `docs/audits/features/README.md`.
- Write the conclusion under `../runs/`.

## Correction-review mode

- Before reading the correction report or diff, trace the candidate's current
  behaviour for the scenarios and surfaces assigned by state.
- Freeze the Pass A verdict for each finding.
- Then compare the baseline and candidate revisions from `../../STATE.md`.
- Name the canonical feature gate from `DOMAIN.md` once on the immutable
  candidate, and a focused test only to resolve a semantic contradiction. Name
  neither the corrector's unfiltered Cucumber nor its workspace gate: the
  auditor does not reproduce global gates.
- Verify that each new test would fail on the baseline **for the intended
  reason**. Where the correction is test-semantics rather than production
  behaviour, the corrector owes a named mutant published as an exact patch, run
  in both directions; see `../../corrector/correct-d-bundle/SKILL.md`,
  § *Proving a test-semantics correction*. Re-run the mutant yourself — name it
  to the orchestrator — rather than accepting a kill count on the corrector's
  word. A mutant stated only in prose is not reproducible and is not evidence.
- Check that no correction weakened a fail-closed path. On these surfaces the
  fail-closed paths are: the edition chain walk and its `prev_hash` check, the
  pinned-file re-hash, the unpinned-stray refusal, the I3 pass over pinned
  headers, the display-path and store-key grammars, the `FsStore` symlink
  refusals, the transaction rollback and recovery, the class binding of each
  narrow capability, and the refusal to accept or return seed material. An
  edition that verifies where it previously refused, or a path that resolves
  where it previously rejected, is a regression even if a scenario turns green.
- Check the consumers named in `DOMAIN.md` — `publication.rs`, `sdk.rs`,
  `structure.rs`, `revoke.rs`, `merge.rs`, `session.rs`, `log.rs`, and the
  edition CLI commands — for a parallel bypass of the corrected verdict. A
  guard added to `Bundle::verify` and not to `cold_verify`, or the reverse, is
  exactly the shape `CHDR-028` describes.
- If the correction touched `aithos-core`, require the `wasm32-unknown-unknown`
  check among the corrector's global gates: `aithos-wasm` depends on
  `aithos-core` and no native test sees the browser target.
- If the correction touched a vector, require the generator `--check` and the
  `vectors/ownership.json` re-pin. `cb2-draft2-carriers.json` is
  `shared: true` with `service_consumers: [aithos-provider]`, so its re-pin is a
  cross-repository cost and must be reported as one.
- Do not close a finding that was not explicitly assigned.
- Do not modify Rust files, `spec/`, or `vectors/`.
- Advance a finding in the public audit to the top acceptance status of the
  `../../PROCESS.md` "Evidence statuses" table only for findings you reproduced
  independently. Never advance one on the corrector's word.
- Remove the Gherkin markers of findings you accept as verified, and rewrite
  rather than strip a marker whose finding is only partly closed.
- Write `../runs/<date>-audit-review-<round>.md`.
- Set state to `CORRECTION_REQUESTED`, `DECISION_REQUIRED`, or
  `IMPACT_REVIEW_REQUESTED`.

The corrector's conclusion is informative. Reconstruct the final verdict from
the current code, the specification sections routed in `DOMAIN.md`,
independently reproduced tests, and only then the diff.

## Two rules that apply to every mode

**Project stage.** `features/AGENTS.md` § *Project stage* holds: nothing is
deployed, no edition has been published, so backward compatibility is not a cost
and must not be weighed. This is the domain where that bites hardest — a change
to the manifest wire, the bundle layout or the at-rest format breaks no holder,
because there is none. Do not write a finding whose severity rests on
invalidating an existing edition, and do not accept a correction softened to
preserve one. Breaking this repository's own tests, vectors or pinned digests is
a real cost and is costed normally. If a first edition has been published outside
this repository, or the crate has left `alpha`, and that section is still
present, report that as a finding rather than obeying it.

**Disclosure gate.** If a finding describes an exploitable weakness for which no
fix exists yet, write an **identifier and a neutral title only** into every
tracked file — the public audit, the Gherkin marker, the run report, this domain
— and raise the full statement to the orchestrator separately. Never the full
text in a tracked file. Blocking condition 9. Assess it in every pass and record
that you assessed it, including when the answer is nothing. `CHDR-028`, `SC-12`
and the code half of `SC-05` are **not** subject to it: the owner published all
three in full on 2026-08-04 and they are not to be re-embargoed.

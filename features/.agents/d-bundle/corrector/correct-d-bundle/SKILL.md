---
name: correct-d-bundle
description: Correct only the bundle and edition findings explicitly assigned by features/.agents/d-bundle/STATE.md. Use this skill after a d-bundle audit to change edition verification, the manifest wire, the store layout and its path grammars, the local transaction boundary, owner-local content operations, narrow capabilities or their tests, without broadening scope, without weakening a fail-closed verifier, and without self-verifying the correction.
---

# Correct `d-bundle.feature`

1. Read `../../../shared/correct-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` and `../../STATE.md` completely.
3. Read the public audit and the auditor's latest conclusion.
4. Address only the findings assigned by state.

Stop without changing code if state is `DECISION_REQUIRED`.

**Run no gate yourself.** Name the exact command from `../../DOMAIN.md`,
§ *Gate pyramid*, and stop. The orchestrator runs it, hashes the transcript and
journals it under an `evidence_id` (`../../../orchestrator/LEDGER.md`). Cite
the `evidence_id` and the printed counters, not only the exit code. A command
with no matching ledger entry is not evidence.

## Domain rules

- **A verifier is fail-closed or it is not a verifier.** `Bundle::verify`
  (`rust/crates/aithos-bundle/src/bundle.rs:1691`) walks the whole chain, checks
  every `prev_hash` against the predecessor's `chain_hash()`, re-hashes every
  pinned file, refuses an unpinned stray, and runs `verify_pinned_headers`
  (`:302-320`). Never make one of those checks conditional, best-effort, or
  skippable by a flag. A correction that lets an edition verify where it
  previously refused is a regression whatever a scenario prints.
- **There are three edition verifiers and they must not diverge.**
  `Bundle::verify` (`bundle.rs:1691`), `publication::cold_verify`
  (`publication.rs:836`) and the keyless package path
  `verify_public_only` (`:586`) / `verify_for_cas` (`:643`) /
  `verify_draft2_candidate` (`:469`), consumed as an acceptance verdict by
  `PublicationUploadPlan::verified` (`sdk.rs:35`). A guard added to one and not
  the others is exactly the defect `chdr-028` describes and this cycle owes.
  When you add a check, state in the run report which of the three received it
  and why the others did or did not.
- **Never change the manifest wire without a normative decision.**
  `prev_hash` is SHA-256 of the prior manifest's JCS with `signature=""`
  (`spec/02-content-tree.md` §2.6, `manifest.rs:98`); the roots, `gamma_head`,
  `gamma_roots` and `gamma_counts_root` are signed bytes. Adding, removing or
  reordering a manifest field changes `chain_hash()` and invalidates every
  vector that pins a manifest. It is a wire change, not a refactor.
- **The store-key and display-path grammars are closed.**
  `validate_store_key` (`lib.rs:142`) and `validate_display_path` (`:89`) are
  the confinement boundary of `spec/02-content-tree.md` §2.3 and §2.12.
  Widening either to make a test pass is the wrong direction; and
  `vectors/cb2-store-key-consumer-neutrality.json` pins that the grammar names
  no consumer. Add a form only with a normative decision, and never a
  consumer-named prefix.
- **One logical commit point, and nothing canonical before it.**
  `spec/02-content-tree.md` §2.12: a mutation is computed in an overlay against
  an immutable snapshot and reduced to a deterministic write-set. Business
  helpers never write canonical objects directly. If a correction needs a new
  write, it goes through `Bundle::transaction` (`bundle.rs:421`) and the `Store`
  transaction methods (`lib.rs:277-302`), not through the `pub` `store` field.
  The `pub` field exists and tests use it; production code that uses it
  bypasses `validate_store_key` and every other invariant.
- **A capability stays narrow, typed and purpose-bound.**
  `spec/01-identity-and-keys.md:140-166`: "A generic `sign(bytes)`,
  decrypt-bytes, cross-context opening, or wrap-bytes oracle is not a compliant
  Bundle API, and a capability for one artifact class cannot substitute for
  another", and "Stable APIs MUST NOT require a raw seed or private key when the
  narrow operation suffices, and MUST NOT expose private material as an output."
  Do not add `pub fn sign(`, `pub fn open(` or `pub fn wrap(` to
  `rust/crates/aithos-bundle/src/session.rs`; and do not "fix" a finding about
  that rule by editing the source text a test greps for.
- **Put the invariant in the layer that owns it.** Header and seal rules belong
  to `aithos-core` (`src/header.rs`, `src/seal.rs`); layout, I/O and edition
  assembly belong to `aithos-bundle`; a rule that lives only in `cucumber.rs`
  protects nobody. `aithos-bundle` is "the only crate in the workspace allowed
  to touch I/O; `aithos-core` stays pure" (`lib.rs:2-6`) — do not put I/O in
  Core to make a correction convenient.
- **Keep randomness injected.** Entropy arrives through
  `rust/crates/aithos-bundle/src/entropy.rs` (`SeqEntropy` in tests, `OsEntropy`
  in the CLI); generating a nonce, an ephemeral or a sid inside a bundle
  function destroys the byte-exact vectors and the deterministic fixtures.
- **Preserve the byte-exact vectors** unless a normative decision says
  otherwise, and extend them rather than replace them. Any change to a vector
  re-pins its `sha256` in `vectors/ownership.json`, enforced by
  `rust/crates/aithos-bundle/tests/vectors_ownership.rs`, and requires that
  vector's generator `--check`. `vectors/cb2-draft2-carriers.json` is
  `shared: true` with `service_consumers: [aithos-provider]`, so its re-pin is a
  cross-repository cost and must be reported as one.
- Do not fix the revocation, structural-mutation, Merkle, Gamma, concurrency or
  delegated-edition features unless the assigned finding requires it; report the
  impact instead. The bundle API is consumed by `structure.rs`, `revoke.rs`,
  `vault.rs`, `session.rs`, `log.rs`, `merge.rs`, `grants.rs`,
  `publication.rs`, `sdk.rs` and eleven CLI commands — a signature change there
  is a cross-feature change.
- Add only the scenarios or tests needed to prove the assigned findings.

## Proving a test-semantics correction

Read this section before writing the first test. It exists because the rule it
states is recorded in `../../../orchestrator/QUEUE.yaml` as written in **no**
normative file (`chdr-lota-mutation-protocol`), and because
`../../../shared/correct-gherkin-feature/SKILL.md` execution steps 1-3
presuppose a defect on a production path.

**A test-semantics lot has no honest RED against unmutated production code.**
When the finding is that a scenario proves less than it claims — not that
production behaves wrongly — a strengthened assertion over correct code is
**green the moment it is written**. Reporting that as a RED/GREEN pair is
false. So is reporting it as GREEN and calling the work proved: a test whose
necessity is unproven is the very defect the lot exists to remove.

**The only honest proof is a named mutant, run twice.**

1. Name the mutant. Give it a stable label (`M1`, `M2`, …) scoped to the run.
2. State exactly which production behaviour it breaks and in which direction.
3. Run it **without** the new assertions: the old assertion must be **green**.
   That is what shows the old one blind.
4. Run it **with** the new assertions: the new one must be **red**. That is
   what shows the new one catches.
5. Both halves are separate gates with separate `evidence_id`s. One half proves
   nothing: a mutant that is red with the new test but was already red without
   it proves the old test was not blind, and a mutant green in both halves
   proves the new test does not catch it.
6. One mutant per assertion whose necessity you claim. If one mutant flips two
   new tests, you have not separated the two gates by evidence, and you owe a
   narrower mutant for each.

**Publish every mutant as an exact patch, never as prose.** A mutant named in
prose cannot be re-run and cannot be checked for direction. This is recorded
with two measured costs in `../../../orchestrator/QUEUE.yaml` under
`chdr-lota-mutants-as-patches`: a review once replayed a mutant at a different
kill count than the corrector reported and had to explain it as "a different
mutant instance", and an impact review once pointed a mutant the wrong way and
needed the complement run before the finding appeared. So:

- the run report carries the mutant as a **unified diff**, applicable with
  `git apply`, against the named revision;
- it names the file and symbol the diff touches;
- it states the expected direction of each half before the transcript;
- it cites the `evidence_id` of both halves.

**Revert every mutant.** A mutant is never committed. Confirm the tree is clean
of all of them before the final gates, and say so in the run report.

**On these surfaces specifically — where a mutant measures nothing.**

- **A source-text assertion is not behavioural.**
  `core_capability_api_is_narrow()` (`cucumber.rs:2053-2058`) decides the
  narrowness half of all four capability rows by
  `include_str!("../src/session.rs")` searched for `pub fn sign(`,
  `pub fn open(` and `pub fn wrap(`. Five `aithos-bundle` test binaries hold 51
  more assertions of that shape (`cb2_bundle_boundaries.rs` 16,
  `cb2_bundle_authority_flows.rs` 15, `cb2_bundle_structure_vault.rs` 10,
  `cb2_bundle_concurrency_final.rs` 7, `cb2_draft2_carriers.rs` 3), recorded as
  `chdr-lota-source-text-assertions`. A mutant that renames a symbol, edits a
  comment or reflows a line will flip such an assertion while proving nothing
  about behaviour — and, worse, a predicate inverted without touching its
  surrounding text will not flip it at all. Aim the mutant at the predicate.
- **A help-string assertion is not behavioural either.** The same rule applies
  to any `help.contains("--flag")` in `rust/crates/aithos-cli/tests/`.
- **`assert!(observation.canonical_unchanged)` is one boolean behind four
  different `Then`s** (`cucumber.rs:11393`, `:11407`, `:11416`, `:11422`). A
  mutant that flips it flips all four at once, which by rule 6 above means you
  owe a narrower mutant per assertion — for example one that leaves a header
  behind but no blob, to separate "no failed-mutation blob, index, header, wrap
  or Gamma entry" from "byte-for-byte identical".
- **Good mutation targets on this domain**, because they are predicates on a
  production path: drop the `prev_hash` comparison in `Bundle::verify`; skip
  one file in the pinned-file re-hash loop; remove the `verify_pinned_headers`
  call; make `validate_store_key` accept a `..` segment; make
  `rollback_transaction` a no-op on `MemStore`; let `FsStore` follow an
  intermediate symlink; return `Ok` from a capability's class check; let
  `owner_content_operation` skip its Gamma append.

## Gates

- Focused RED/GREEN while the implementation changes — name the narrowest of:

  ```text
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb7_transaction_contracts
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb8_owner_grants
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb12_publication_package
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_boundaries
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_authority_flows
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_draft2_carriers
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_version_coexistence
  ```

  Use `-- --exact <name>` to pin a single test; the names on disk are listed in
  `../../DOMAIN.md`, § *Focused tier*.

- Canonical feature gate once after the final change:

  ```text
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
  ```

  Record the printed counters and read `../../DOMAIN.md`, § *Reading the
  counters*, before interpreting them. The file on disk expands to 1 feature /
  7 rules / 51 scenarios / 299 steps; a different count means the contract that
  ran is not the contract you changed.

- Relevant regressions, the vector `--check` commands, and the final global
  gates: exactly the commands in `../../DOMAIN.md`, § *Gate pyramid*. Three
  things there are not optional and are easy to forget:
  `cargo clippy --workspace --all-targets … -- -D warnings`, which CI enforces
  with `-D warnings`; `--no-fail-fast` on **every** multi-binary invocation,
  because without it `cargo test` aborts at the first failing binary and the
  regression silently under-reports; and
  `cargo check -p aithos-wasm --target wasm32-unknown-unknown …` whenever the
  correction touched `aithos-core`, since `aithos-wasm` depends on it and no
  native test sees the browser target.

- One scope limit to state in the run report rather than discover later: the
  `remote` feature is enabled by no gate in this repository, so
  `rust/crates/aithos-bundle/src/remote.rs` is not compiled by
  `cargo test --workspace`. If a correction changes the `Store` trait or a
  signature `RemoteStore` implements, say so explicitly — no declared gate will.

## Project stage

`features/AGENTS.md` § *Project stage* holds. Nothing is deployed, **no edition
has been published by anyone**, and this is the domain where that sentence is
literal. So:

- do not propose a migration path, a legacy manifest profile, a grandfather
  clause or a compatibility shim for the bundle layout, the manifest wire or the
  at-rest format — there is no holder of an edition to protect, and the cost of
  a clean break is nil today;
- do not soften a correction to spare the past; the right shape now is the one
  you would choose with no history at all;
- but do cost normally any change that breaks this repository's own tests,
  vectors or pinned digests, and treat a rule the implementation cannot satisfy
  as a defect whatever the user count.

If a first edition has been published outside this repository, or the crate has
left `alpha`, and that section is still present, report it rather than obey it.

## Disclosure gate

If a correction, a mutant, or a run report would describe an exploitable
weakness for which no fix exists yet, write an **identifier and a neutral title
only** into every tracked file and raise the full statement to the orchestrator
separately. Never the full text in a tracked file. Blocking condition 9. A
mutant patch is a tracked artefact and is covered by this rule: if publishing
the patch is itself the disclosure, name the mutant and withhold the diff,
raising it separately.

`CHDR-028`, `SC-12` and the code half of `SC-05` were published in full by owner
ruling on 2026-08-04 (`../../../orchestrator/BLOCKED.md`). Cite them freely; do
not re-embargo them.

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, every mutant as a patch with both halves'
  `evidence_id`s, RED, GREEN, and changed files.
- State which of the three edition verifiers each new guard reached.
- Report any divergence between the audit and observed reality, including a
  mutant the audit stated that turns out wrong on the code.
- State explicitly that the tree is clean of every mutant.
- Report any vector re-pin, and name the cross-repository cost when the vector
  is `shared: true`.
- Move findings at most to `IMPLEMENTED`. Never `VERIFIED`.
- Request review from `audit-d-bundle`.
- Set `STATE.md` to `REVIEW_REQUESTED` with the immutable baseline and
  candidate revisions.

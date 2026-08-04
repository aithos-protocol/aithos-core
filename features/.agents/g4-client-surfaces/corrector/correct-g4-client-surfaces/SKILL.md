---
name: correct-g4-client-surfaces
description: Correct only the client-surface findings explicitly assigned by features/.agents/g4-client-surfaces/STATE.md. Use this skill after a g4-client-surfaces audit to change the WASM binding, the CLI delegated-session ceremony, their custody discipline or their tests, without broadening scope, without inventing protocol logic in a client, and without self-verifying the correction.
---

# Correct `g4-client-surfaces.feature`

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

- **A client surface holds no protocol logic.** `aithos-wasm` is declared thin
  by its own crate doc — "no logic lives here, only (de)serialization at the JS
  boundary" (`rust/crates/aithos-wasm/src/lib.rs:2-4`) — and `aithos-cli`
  delegates to it. Put an invariant in `aithos-core` (`src/mandate.rs`,
  `src/jcs.rs`, `src/gamma.rs`, `src/wire.rs`), not in the binding and not in a
  step definition. A rule that lives only in `aithos-wasm` protects the browser
  and the CLI and no other Core consumer; a rule that lives only in
  `cucumber.rs` protects nobody.
- **Never canonicalise or hash outside Core's rule.**
  `rust/crates/aithos-core/src/jcs.rs:1-5`: RFC 8785 JCS is the only
  serialization ever signed or hashed, and no ad-hoc `serde_json::to_string`
  may be signed. Every existing signature site in these surfaces goes through
  `serde_jcs::to_vec` and `aithos_core::gamma::sha256_hex`; keep it that way.
  Changing what enters a preimage — for example the domain separator
  `b"aithos-gateway/mcp-ceremony/v1\x00"` (`lib.rs:325`) — is a wire change,
  not a refactor.
- **Keep randomness injected.** The core never draws randomness
  (`aithos-core/src/keys.rs:57`), and `rust/Cargo.toml` keeps `getrandom` out
  of the `wasm32` graph on purpose. Every id, nonce and timestamp is a caller
  input in the WASM request shape (`lib.rs:90-106`). Generating one inside the
  binding would break the browser target and destroy determinism.
- **Never widen what a client returns.** No function may return person seed,
  session seed, or any private key material. `DelegateSeed`'s `Drop`
  (`lib.rs:39-45`) zeroizes the caller's buffer; a new function takes the same
  discipline or it does not ship.
- **Never move a secret onto `argv`.** `--signer-stdin` and stdin master-seed
  custody are the contract (`cmd/oauth.rs:1-2`, `cmd/owner.rs:6-8`,
  `delegated_oauth.rs:227-243`, `cmd/owner.rs:193-203`). If a correction needs
  a new secret input, it arrives on stdin, a file descriptor, or the custody
  interface — never a flag.
- **Keep every ceremony path fail-closed.** The guards are listed in the
  auditor skill; a correction must not leave a ceremony that completes where it
  previously refused.
- **Preserve the byte-exact vectors** unless a normative decision says
  otherwise, and extend them rather than replace them. Any change to
  `vectors/cb14-delegated-session-chain.json` or
  `vectors/cb15-external-delegated-grant.json` requires re-pinning its `sha256`
  in `vectors/ownership.json`, enforced by
  `rust/crates/aithos-bundle/tests/vectors_ownership.rs`, and running that
  vector's generator `--check`. `cb14` is `shared: true` with
  `service_consumers: [aithos-gateway]`, so its re-pin is a cross-repository
  cost and must be reported as one.
- **The CLI surface is pinned by a test.** `rust/crates/aithos-cli/tests/cli_surface.rs`
  asserts command names, flags, help text and exit codes unchanged
  (`src/main.rs:5-6`). Breaking it is a real repository cost, costed normally.
  Breaking an *external* consumer is not a cost — see § *Project stage* below.
- Do not fix the mandate, revocation, Gamma, or bundle-session features unless
  the assigned finding requires it; report the impact instead.
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

**On these surfaces specifically.** A mutant that only changes a help string,
or only changes a `//` comment, is not a behavioural mutant: assertions of the
form `help.contains("--signer-stdin")` or
`include_str!(src)…contains(literal)` will catch it while proving nothing about
behaviour. Aim a mutant at the predicate — invert the `--signer-stdin` guard,
drop the `same_origin` check, widen the eight-hour bound, remove the
`ed2x` binding check, delete the `issue`-in-perimeter refusal — and prove the
test catches *that*.

## Gates

- Focused RED/GREEN while the implementation changes — name the narrowest of:

  ```text
  cargo test --manifest-path rust/Cargo.toml -p aithos-cli --test delegated_oauth
  cargo test --manifest-path rust/Cargo.toml -p aithos-cli --test cli_surface
  cargo test --manifest-path rust/Cargo.toml -p aithos-wasm --lib
  cargo test --manifest-path rust/Cargo.toml -p aithos-core --test cb14_delegated_session_chain
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb15_external_delegated_grant
  ```

  `aithos-wasm` has no integration-test binary; `--lib` is the only selector
  that runs its test. Use `-- --exact <name>` to pin a single test.

- Canonical feature gate once after the final change:

  ```text
  cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g4-client-surfaces
  ```

  Record the printed counters, and read `../../DOMAIN.md`, § *Reading the
  counters*, before interpreting them: the feature's tag line carries `@wip`
  and the runner filters it, so a zero-selection result is a recorded red under
  `../../../orchestrator/LEDGER.md:44-52`, not a pass.

- Relevant regressions, the vector `--check` commands, and the final global
  gates: exactly the commands in `../../DOMAIN.md`, § *Gate pyramid*. Two of
  them are not optional here and are easy to forget:
  `cargo clippy --workspace --all-targets … -- -D warnings`, which CI enforces,
  and `cargo check -p aithos-wasm --target wasm32-unknown-unknown …`, which is
  the only gate that sees a change breaking the browser target.
  Every multi-binary invocation carries `--no-fail-fast`: without it `cargo
  test` aborts at the first failing binary and the regression silently
  under-reports.

## Project stage

`features/AGENTS.md` § *Project stage* holds. Nothing is deployed, no edition
has been published, `aithos-wasm` is `publish = false` and packaged only
locally. So:

- do not propose a migration path, a legacy flag, a deprecation window or a
  compatibility shim for the WASM or CLI surface — there is no external holder
  to protect, and the cost of a clean break is nil today;
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

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, every mutant as a patch with both halves'
  `evidence_id`s, RED, GREEN, and changed files.
- Report any divergence between the audit and observed reality, including a
  mutant the audit stated that turns out wrong on the code.
- State explicitly that the tree is clean of every mutant.
- Move findings at most to `IMPLEMENTED`. Never `VERIFIED`.
- Request review from `audit-g4-client-surfaces`.
- Set `STATE.md` to `REVIEW_REQUESTED` with the immutable baseline and
  candidate revisions.

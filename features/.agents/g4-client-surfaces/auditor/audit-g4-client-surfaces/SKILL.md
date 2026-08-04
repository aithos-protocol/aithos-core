---
name: audit-g4-client-surfaces
description: Audit or review only features/g4-client-surfaces.feature using its client-surface invariants — the WASM/browser binding and the CLI both call aithos-core for every mandate, signature and canonicalisation, return no seed material, and keep the signer out of argv. Use this skill when features/.agents/g4-client-surfaces/STATE.md routes an initial audit or an independent correction review to the client-surfaces auditor.
---

# Audit `g4-client-surfaces.feature`

1. Read `../../../shared/audit-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` completely.
3. Read only mode, branch, base revision, observed revision, assigned scope,
   and output routing from `../../STATE.md`, plus its § *Recorded follow-ups
   this feature already owes* — those name debts, not verdicts, and are routing
   material.
4. Execute only the mode requested by state.

Do not read the public audit, prior run conclusions, correction reports, or Git
history until the shared skill authorizes Pass B. This feature has **no
yardstick branch**: `../../../orchestrator/QUEUE.yaml:96-97` registers one for
`c-headers` only. Its ordinary Pass B material is the diff and the
impact-review reports of other features that name it, and those are Pass B
inputs. Opening any of them before the freeze contaminates the review unit;
disclose it and restart the unit.

**Run no gate yourself.** Name the exact command from `../../DOMAIN.md`,
§ *Gate pyramid*, and stop. The orchestrator runs it, hashes the transcript and
journals it under an `evidence_id` (`../../../orchestrator/LEDGER.md`). Cite
the `evidence_id`. A command with no matching ledger entry is not evidence.

## Initial-audit mode

- `g4-client-surfaces.feature` has **no `Rule` block**. Divide it into two
  review units by surface and risk cluster, per `../../PROCESS.md`,
  § *Review-unit isolation and impartiality*:
  **RU-1, WASM and browser** — scenarios 1 and 2 (`:6-16`);
  **RU-2, CLI** — scenarios 3 and 4 (`:18-27`).
  Then run the shared-state integration pass over both, because scenario 3
  asserts that the CLI "executes the same verify build and sign primitives as
  WASM" — that clause is a claim *across* the two units and belongs to the
  integration pass, not to either unit alone.
- Before tracing, establish what the canonical feature gate actually selected.
  `features/g4-client-surfaces.feature:1` carries `@wip`, and the runner
  filters `wip` at feature, rule and scenario level
  (`rust/crates/aithos-bundle/tests/cucumber.rs:20034-20038`).
  `../../../orchestrator/LEDGER.md:44-52` treats exit 0 with zero scenarios
  selected as **red**, with an `anomaly`, raising blocking condition 3. Record
  the transcript. Do not restate `DOMAIN.md`'s description of the filter as if
  it were the gate result, and do not proceed as though a contract you could
  not observe running had been observed running.
- If the gate selects nothing, say so plainly and state what that does to every
  verdict you can still reach. A scenario the runner never executes cannot be
  `PROVEN` under `docs/audits/features/README.md`, § *Evidence rules*, whose
  first bullet is "no `@wip` tag or filter excludes it". Classify from the
  closed table of `../../PROCESS.md`, § *Evidence statuses*, and do not invent
  a status.
- Resolve every phrase to its exact step definition. `DOMAIN.md`, § *Shared
  steps, fixtures, and helpers*, records the search that found none in
  `rust/crates/aithos-bundle/tests/cucumber.rs`; **reproduce that search rather
  than trusting it**, and establish what `.fail_on_skipped()`
  (`cucumber.rs:20029`) does to an unresolved phrase when the feature is
  selected. A domain file is a document, and `../../PROCESS.md`,
  § *Evidence hierarchy*, puts current executable code above any document.
- For scenario 1, the four functions named in the Gherkin are
  `delegate_pubkey`, `verify_mandate_chain`, `build_session_submandate` and
  `sign_ceremony_challenge`. Trace each into
  `rust/crates/aithos-wasm/src/lib.rs` and out into `aithos-core`. Establish
  which parts of the behaviour are Core's (`Mandate::build_sub`,
  `verify_chain`, `verify_chain_revocable`) and which are added by the binding
  itself — the subject check `:131-133`, the `issue`-in-perimeter refusal
  `:134-141`, the eight-hour lifetime bound `:142-148`, the `ed2x` binding of
  `gateway_kex_pub` to `gateway_pub` `:153-159`. The scenario says "every
  mandate and signature is produced or verified by aithos-core"; a rule that
  exists only in the binding is a rule no other Core consumer enforces, and
  whether that contradicts the clause is the question.
- For "no function returns person or session seed material", enumerate the
  whole exported surface, not the four named functions: seven free
  `#[wasm_bindgen]` functions and `DelegateSigner` with four methods
  (`DOMAIN.md`, § *Public surfaces*). Establish what each returns. Then
  establish what `DelegateSeed`'s `Drop` (`lib.rs:39-45`) does and does not
  cover — a caller buffer is zeroized, but decide whether any intermediate the
  binding constructs escapes that.
- For scenario 2, the browser half has **no production code in this
  repository** (`DOMAIN.md`, § *What is not in this repository*, with its
  search). Reproduce that search. Then decide, and state explicitly, whether
  each clause of scenario 2 is a claim about `aithos-wasm` — which is here —
  or about a browser application — which is not. Do not credit this repository
  with proof of a clause whose subject lives elsewhere, and do not classify a
  clause as failing here when its subject is out of scope; name the correct
  owner instead.
- For scenario 3, the CLI ceremony is `authorize_delegated`
  (`rust/crates/aithos-cli/src/delegated_oauth.rs:296-618`). Follow the signer
  from `read_signer_seed` (`:227-243`) through `DelegateSigner::new` (`:306`)
  to every `aithos_wasm::*` call, and follow the printed verdict block
  (`:501-525`, `:615-617`) to establish what is public and what is redacted.
  Distinguish three different proofs the "same primitives as WASM" clause can
  receive: a source-level fact that `aithos-cli` calls `aithos_wasm`; an
  executed test that runs the CLI binary; and an assertion that the two produce
  the same bytes. Only the last proves sameness of output.
- For scenario 4, "production session commands" is a set, not a command.
  Establish which commands the scenario covers before judging any of them.
  `aithos owner` has nine subcommands, seven of which carry
  `--master-seed-hex` documented "DEV ONLY on the command line"
  (`cmd/owner.rs:29`, `:54`, `:64`, `:116`, `:131`, `:149`, `:168`) and two of
  which read the seed from stdin (`:84`, `:102`, via `decode_master_stdin`
  `:193-203`). `aithos oauth authorize-delegated` refuses to start without
  `--signer-stdin` and `--approve` (`delegated_oauth.rs:297-299`). Decide
  whether "refuses it before any protocol or network effect" is satisfied by an
  argument that does not exist, by a runtime refusal, or only by a refusal
  proved to precede the first side effect — and check where in the call order
  each refusal actually sits.
- Distinguish an assertion about a **help string** from an assertion about
  **behaviour**. `cli_surface.rs:26-39` asserts that `--seed` and
  `--private-key` are absent from `--help` output. Establish what class of
  regression such an assertion can and cannot detect. The same question applies
  to any assertion of the form `include_str!(src)…contains(literal)`;
  `../../../orchestrator/QUEUE.yaml` records that class as
  `chdr-lota-source-text-assertions`, and its scope limit — counted, not
  classified — applies to anything you meet here.
- Strengthen byte-exact cases with the vectors rather than with
  self-consistency between two calls of the same function.
  `vectors/cb15-external-delegated-grant.json` is read by three different
  consumers (`DOMAIN.md`, § *Vectors involved*); establish which of them, if
  any, a Gherkin step of **this** feature reaches, and say so explicitly rather
  than crediting the feature with a vector's proof.
- Check whether any verdict this feature would consume is one of the five
  process-global `OnceLock` verdicts (`cucumber.rs:1119-1129`, helpers
  `:7295-7346`). `QUEUE.yaml`'s `chdr-lota-proxy-verdicts` does not list this
  feature; confirm that against the code rather than against the list.
- Discharge the recorded follow-ups of `../../STATE.md` explicitly. `header-seal`
  and `header-open` are unexercised by `cli_surface.rs` per `chdr-i3-g4-cli`;
  reproduce that search, and state for each whether it yields a finding of this
  feature, a debt routed elsewhere, or nothing. A follow-up left unmentioned in
  the run report is a follow-up dropped.
- Freeze per-unit Pass A notes before reading any prior material.
- Document discrepancies under stable `G4CS-*` identifiers.
- Run the final integration check across the WASM binding, the CLI, the
  `aithos-core` symbols both call, and the Bundle-side consumers listed in
  `DOMAIN.md`, § *Public surfaces*.
- Create `docs/audits/features/g4-client-surfaces.md` and add the index row in
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
  in both directions; see `../../corrector/correct-g4-client-surfaces/SKILL.md`,
  § *Proving a test-semantics correction*. Re-run the mutant yourself — name it
  to the orchestrator — rather than accepting a kill count on the corrector's
  word. A mutant stated only in prose is not reproducible and is not evidence.
- Check that no correction weakened a fail-closed path. On these surfaces the
  fail-closed paths are: the `--signer-stdin` / `--approve` guard, the
  existing-token-file refusal, the two same-origin checks, the binding-mismatch
  check, the callback origin and `state` checks, the eight-hour lifetime bound,
  the `issue`-in-perimeter refusal, the `gateway_kex`/`gateway_pub` binding,
  and the unsigned-grant shape check. A ceremony that completes where it
  previously refused is a regression even if a scenario turns green.
- Check the consumers named in `DOMAIN.md` — `aithos-bundle/src/session.rs`,
  `aithos-owner/src/lib.rs`, `docker/npm-smoke.mjs` — for a parallel bypass of
  the corrected verdict.
- If the correction touched `aithos-wasm`, require the
  `wasm32-unknown-unknown` check among the corrector's global gates: a
  dependency change that compiles natively can still break the browser target,
  and that is the only gate that sees it.
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
deployed, no edition is published, so backward compatibility is not a cost and
must not be weighed. Do not write a finding whose severity rests on breaking an
external consumer of `@aithos/core` or of the `aithos` CLI surface — there is
none. Breaking this repository's own tests, vectors or pinned digests is a real
cost and is costed normally. If a first edition has been published outside this
repository, or the crate has left `alpha`, and that section is still present,
report that as a finding rather than obeying it.

**Disclosure gate.** If a finding describes an exploitable weakness for which
no fix exists yet, write an **identifier and a neutral title only** into every
tracked file — the public audit, the Gherkin marker, the run report, this
domain — and raise the full statement to the orchestrator separately. Never the
full text in a tracked file. Blocking condition 9. Assess it in every pass and
record that you assessed it, including when the answer is nothing.

# Domain — `g4-client-surfaces.feature`

## Contract

This feature covers the two **client** surfaces of the closed delegated-session
protocol — the WASM/browser binding and the CLI — and the single claim that
both of them are thin: they call `aithos-core` for every mandate, every
signature and every canonicalisation, and neither of them re-implements JCS,
attenuation, or key handling
(`features/g4-client-surfaces.feature:1-4`):

- the WASM API exposes the closed delegated-session surface and returns no
  person or session seed material (scenario 1);
- browser custody is pubkey-first: only public keys, signed protocol objects
  and the ceremony proof leave the browser, and plaintext key material is
  zeroized without entering the URL, DOM storage or logs (scenario 2);
- the CLI reads its signer outside `argv` — stdin, a file descriptor or a
  custody interface — prints only URLs, public keys, ids and redacted
  verdicts, and executes the same verify/build/sign primitives as WASM
  (scenario 3);
- a production delegated-session command refuses private seed material
  supplied in process arguments **before** any protocol or network effect
  (scenario 4).

Shape of the contract as it sits on disk at `c406bbf`, counted from
`features/g4-client-surfaces.feature`: **1 feature / 0 rules / 4 scenarios /
15 steps**. This is a count of the file, not a gate result — see § *Reading the
counters*.

Tag line: `@g4-client-surfaces @wip @g4 @wasm @cli`
(`features/g4-client-surfaces.feature:1`). The canonical tag is
`@g4-client-surfaces`; `@g4`, `@wasm` and `@cli` are surface markers and `@wip`
is the runner's exclusion tag (`features/.agents/PROCESS.md:52-59`).

There is no public audit note for this feature yet:
`docs/audits/features/` contains exactly `README.md`, `a-identity.md`,
`b-derivation.md` and `c-headers.md` (`ls docs/audits/features`, whole
directory). `docs/audits/features/README.md` reserves **no** identifier family
for it — its index table has three rows, for `a-identity`, `b-derivation` and
`c-headers` only. This domain therefore prescribes **`G4CS-*`** as the stable
finding family, and the auditor creates
`docs/audits/features/g4-client-surfaces.md` plus its index row.
`G4CS-` is chosen over the shorter `G4-` because `G4` alone is the programme
name used throughout `docs/` (`docs/DEMO-INTEGREE-G4-SHEETS.md`,
`docs/ADR-OLR-OAUTH-LIBS-STANDARD-2026-07-22.md:8`, `:23`, `:47`, and others),
so a `G4-001` reference would not be greppable. Neither string is in use:
`grep -rn "G4CS-\|G4-0" .` over the whole worktree excluding `.git` returns 0
lines.

## Project stage — carried forward from `features/AGENTS.md`

`features/AGENTS.md` § *Project stage* applies in full to every role on this
feature, and is the normative text; what follows is a pointer, not a
replacement.

As of 2026-08-04 `aithos-core` is `0.1.0-alpha.1` (`rust/Cargo.toml`,
`[workspace.package] version`), nothing is deployed and no edition has been
published. Consequences for this domain:

1. **Do not weigh backward compatibility.** No migration path, grandfather
   clause, legacy profile or compatibility shim is owed to any holder of data,
   because there is none. This bites here in particular: the WASM binding is
   `publish = false` and packaged only locally
   (`rust/crates/aithos-wasm/Cargo.toml`, and the crate doc at
   `src/lib.rs:2-4`), and the CLI surface is asserted unchanged by
   `rust/crates/aithos-cli/tests/cli_surface.rs` — that test is a
   *repository* cost and is counted normally, but no external consumer is.
2. **Do not soften a correction to spare the past.**

What still counts: a change that breaks this repository's own tests, vectors
or pinned digests is a real cost. And a rule the implementation cannot satisfy
is a defect at zero users.

**That section expires** on the first published edition outside this repository
or when the crate leaves `alpha`. A role that finds it still present after
either has happened reports that fact rather than obeying it
(`features/AGENTS.md:30-34`). `QUEUE.yaml` carries the same switch as
`policy.backward_compatibility_required: false` with the same expiry.

## Branch and evidence

- Canonical audit branch: `codex/audit-g4-client-surfaces`, the
  `PROCESS.md` default (`features/.agents/PROCESS.md:17-18`). **The name is
  free.** `features/.agents/orchestrator/QUEUE.yaml:96-97` registers a
  `yardsticks:` entry for `c-headers` only, and `git branch -a --list` lists
  eight refs (`codex/audit-c-headers-r2`, `main`, `origin/HEAD`,
  `origin/codex/audit-c-headers`, `origin/codex/bundle-publication-performance`,
  `origin/codex/gateway-demo-companion`,
  `origin/codex/gse-0-extension-registry-wip`,
  `origin/codex/olr-oauth-libs-upstream`, `origin/main`), none of which
  mentions `g4`. This feature has **no yardstick**: there is no prior manual
  audit branch to treat as a Pass B input.
- Corrections use a dedicated `codex/fix-g4-client-surfaces-<finding-or-scope>`
  descendant branch of the immutable audited revision
  (`features/AGENTS.md:44-46`, `features/.agents/PROCESS.md:39-41`).
- `base_main` and `audit_revision` are `null` in `STATE.md`: no revision has
  been frozen for this feature. The role that opens the round freezes them
  there and in its run report.
- **No gate has been run for this feature by this role.** Every number in this
  file is either a count of a file on disk, stated as such, or a command to
  run — never a result.

## Protocol invariants

Numbered claims, each with the normative text that governs it and the symbol
that implements it. These are the obligations the scenarios name; whether a
scenario proves its own is the auditor's question, not this file's.

1. **The client never draws randomness; entropy is caller-supplied.**
   `spec/00-overview.md` and the crate contract
   (`rust/crates/aithos-core/src/keys.rs:57`, "built from caller-supplied
   entropy — the core never draws randomness"; `rust/Cargo.toml` pins
   `chacha20poly1305` with `default-features = false` precisely so that
   `getrandom` stays out of the `wasm32` graph). In WASM every seed, id and
   nonce is a parameter: `genesis_pubkeys(seed)`
   (`rust/crates/aithos-wasm/src/lib.rs:28`), and
   `SessionSubmandateRequest` carries `id`, `nonce`, `not_before`,
   `not_after`, `issued_at` as inputs (`:90-106`).
2. **No client function returns seed material.** The WASM binding wraps every
   caller buffer in `DelegateSeed`, whose `Drop` fills it with zero
   (`lib.rs:39-45`), and returns only public multibase keys, signed protocol
   objects, and digests (`delegate_pubkey` `:58`; `sign_ceremony_challenge`
   `:307` returning `{aithos-mcp-ceremony, digest, delegate_pub, sig}`
   `:327-335`).
3. **JCS is Core's, not the client's.** `spec/00-overview.md` §0.3 and
   `rust/crates/aithos-core/src/jcs.rs:1-5` — "the ONLY serialization ever
   signed or hashed … no ad-hoc `serde_json::to_string` may ever be signed".
   The WASM binding canonicalises through `serde_jcs::to_vec` at
   `lib.rs:228`, `:232`, `:259`, `:298`, `:322` and hashes through
   `aithos_core::gamma::sha256_hex` (`:252`, `:256`, `:264`, `:324`). The
   stated design intent is at `lib.rs:214-215`: "Construct the exact gateway
   challenge and its presentation digest in Rust, so JavaScript never
   implements JCS or hashes a non-canonical leaf."
4. **Attenuation rules are Core's.** `spec/05-delegation.md` §5.3 (attenuation
   invariants, verifier, per link child→parent) and §5.4 — implemented by
   `aithos_core::mandate::Mandate::build_sub`
   (`rust/crates/aithos-core/src/mandate.rs:955`) and
   `verify_chain` / `verify_chain_revocable` (`:1022`, `:1029`). The WASM
   binding calls them (`lib.rs:83`, `:86`, `:169`) and adds surface-level
   pre-checks of its own before doing so: same subject as the parent
   (`:131-133`), no `issue` entry in a session perimeter (`:134-141`),
   lifetime bounded at eight hours (`:142-148`), `gateway_kex` bound to
   `gateway_pub` through `ed2x` (`:153-159`).
5. **`session_bind` binds every delegated consumption to one ephemeral session
   key**, and the SC1 certificate profile is closed
   (`spec/04-mandates.md:1261-1310`, § 4.7). In WASM the session key enters
   the leaf as `constraints["session_bind"]` (`lib.rs:168`) after the
   `session_pub` has been parsed as a real Ed25519 key (`:160-161`).
6. **The delegated grant is the existing Gamma v1 wire, signed outside
   Bundle.** `spec/07-gamma.md:115` — a grant entry carries `authorized_by` =
   leaf id and `authorized_via`. `sign_delegated_grant`
   (`lib.rs:274`, `:280-302`) refuses anything that is not an unsigned `grant`
   entry addressed to this exact signer (`:284-294`), calls `Entry::check_form`
   (`:295-297`), and signs JCS bytes.
7. **The CLI signer is read outside `argv`.** `rust/crates/aithos-cli/src/cmd/oauth.rs:1-2`
   and `src/main.rs:33` state the rule; `read_signer_seed`
   (`src/delegated_oauth.rs:227-243`) reads 32 hex bytes from stdin into a
   `Zeroizing` buffer and zeroizes both the string and the decoded vector.
   `authorize_delegated` refuses to start unless `--signer-stdin` and
   `--approve` are both set (`:296-299`), before any HTTP call.
8. **The CLI executes the same primitives as WASM.** `aithos-cli` depends on
   `aithos-wasm` as an ordinary Rust crate and calls the very same functions:
   `DelegateSigner::new` (`delegated_oauth.rs:306`),
   `aithos_wasm::verify_mandate_chain` (`:420`, `:454`),
   `signer.build_session_submandate` (`:449`),
   `aithos_wasm::build_ceremony_challenge` (`:483`),
   `signer.sign_delegated_grant` (`:480`),
   `signer.sign_ceremony_challenge` (`:528`).
9. **The CLI prints only public material.** The verdict block is
   `delegated_oauth.rs:501-525` and `:615-617`; tokens and the one-shot code
   are held in `Zeroizing` (`:564`, `:582`, `:589`) and written to a `0o600`
   file (`write_tokens`, `:278-294`, mode at `:285`).
10. **Owner-side session commands read the master seed from stdin.**
    `rust/crates/aithos-cli/src/cmd/owner.rs:6-8` states the discipline. Of the
    nine `OwnerCommand` variants (`:29`, `:54`, `:64`, `:84`, `:102`, `:116`,
    `:131`, `:149`, `:168`), seven carry a `--master-seed-hex` argument
    documented "DEV ONLY on the command line"; exactly two —
    `GrantSessionDelegate` (`:84`) and `RevokeMandate` (`:102`) — carry none
    and call `decode_master_stdin` (`:193-203`) at `:303` and `:326`.
11. **The CLI reference surface of the specification is §9.1**
    (`spec/09-cli-and-conformance.md:6-35`), which is `DRAFT` and describes an
    `aithos-core …` command vocabulary; the shipped binary is `aithos`
    (`src/main.rs:15`). Reconciling the two is a contract question for the
    auditor, not a fact this file settles.
12. **Vectors are normative at promotion** (`spec/09-cli-and-conformance.md:37`,
    § 9.2), and every vector is pinned in `vectors/ownership.json`, enforced by
    `rust/crates/aithos-bundle/tests/vectors_ownership.rs`.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/g4-client-surfaces.feature` |
| Cucumber runner and its `@wip` filter | `rust/crates/aithos-bundle/tests/cucumber.rs:20017-20040` |
| WASM binding (whole surface) | `rust/crates/aithos-wasm/src/lib.rs` |
| WASM crate manifest (`publish = false`, `cdylib`+`rlib`, no `getrandom`) | `rust/crates/aithos-wasm/Cargo.toml` |
| WASM test fixture loader | `rust/crates/aithos-wasm/tests/fixtures/vectors.rs`, `#[path]`-included at `src/lib.rs:20-22` |
| CLI entry point and command inventory | `rust/crates/aithos-cli/src/main.rs` |
| CLI delegated ceremony (the whole G4 client flow) | `rust/crates/aithos-cli/src/delegated_oauth.rs` |
| CLI `oauth` subcommand surface | `rust/crates/aithos-cli/src/cmd/oauth.rs` |
| CLI owner-side ceremonies and stdin custody | `rust/crates/aithos-cli/src/cmd/owner.rs` |
| CLI local custody / key-store | `rust/crates/aithos-cli/src/custody.rs` |
| CLI §03 surfaces named by the recorded follow-up | `rust/crates/aithos-cli/src/cmd/header_seal.rs`, `.../header_open.rs` |
| Mandate build and chain verification | `rust/crates/aithos-core/src/mandate.rs` |
| Canonicalisation | `rust/crates/aithos-core/src/jcs.rs` |
| Wire encodings (multibase ↔ raw pubkeys) | `rust/crates/aithos-core/src/wire.rs` |
| Gamma entries, `ts_epoch`, `sha256_hex` | `rust/crates/aithos-core/src/gamma.rs` |
| Revocations applied to a chain | `rust/crates/aithos-core/src/revocation.rs` |
| Owner keys, `ed2x`, `MasterSeed` | `rust/crates/aithos-core/src/keys.rs` |
| DID document | `rust/crates/aithos-core/src/did.rs` |
| Owner-side ceremony library | `rust/crates/aithos-owner/src/lib.rs` |
| CLI surface test | `rust/crates/aithos-cli/tests/cli_surface.rs` |
| CLI delegated-ceremony end-to-end test | `rust/crates/aithos-cli/tests/delegated_oauth.rs` |
| SC1 / session-chain contract test | `rust/crates/aithos-core/tests/cb14_delegated_session_chain.rs` |
| External delegated-grant contract test | `rust/crates/aithos-bundle/tests/cb15_external_delegated_grant.rs` |
| Session-proof contract test | `rust/crates/aithos-core/tests/cb2_session_proof.rs` |
| Vector pin enforcement | `rust/crates/aithos-bundle/tests/vectors_ownership.rs` |
| npm smoke over the packed WASM package | `docker/npm-smoke.mjs` |
| CI, including the `wasm` job | `.github/workflows/ci.yml` |
| Specification | `spec/04-mandates.md` § 4.7 (session binding, SC1), § 4.5.1 ; `spec/05-delegation.md` § 5.1–§ 5.5 ; `spec/07-gamma.md` § 7 (grant entries, `authorized_by` / `authorized_via`) ; `spec/09-cli-and-conformance.md` § 9.1, § 9.2, § 9.4 ; `spec/00-overview.md` § 0.3 (JCS, AAD purposes) ; `spec/10-threat-model.md` § 10.5, § 10.7 |

### Vectors involved

This domain **owns no vector of its own**. Search: `ls vectors` lists 41
`*.json` files (40 vectors plus the `ownership.json` manifest), 29 `gen-*.py`
generators and `README.md`; none is named for `g4` or
for client surfaces (`ls vectors | grep -i "g4\|client\|surface"` returns
nothing). The two vectors whose Rust consumers declare a G4 lineage in their own
header comment are:

- `vectors/cb14-delegated-session-chain.json` — consumed by
  `rust/crates/aithos-core/tests/cb14_delegated_session_chain.rs`, whose
  `//!` reads "G4/P7 vectors-first contract for SC1 over a verified non-root
  leaf" (`:1-2`). Pinned `owner: core`, `shared: true`,
  `service_consumers: [aithos-gateway]`, `sha256: 1a744d4f…`
  (`vectors/ownership.json`). Generator
  `vectors/gen-cb14-delegated-session-chain.py`, which **has** a `--check`
  mode (`:294`, `:298`).
- `vectors/cb15-external-delegated-grant.json` — consumed by
  `rust/crates/aithos-bundle/tests/cb15_external_delegated_grant.rs`
  ("G4/P7 contract for a gateway-prepared Gamma grant signed outside Bundle",
  `:1-2`), by the WASM unit test (`aithos-wasm/src/lib.rs:394-408`) and by the
  CLI end-to-end test (`aithos-cli/tests/delegated_oauth.rs:74-76`). Pinned
  `owner: core`, `sha256: bf7a7308…`. Generator
  `vectors/gen-cb15-external-delegated-grant.py`, `--check` at `:137`, `:141`.
- `vectors/cb2-session-proof.json` — the SC1 double-proof vector, pinned
  `owner: core`, `shared: true`. Consumed by
  `aithos-core/tests/{cb2_session_proof,cb4_operation_contracts,cb5_evidence_contracts}.rs`
  and `aithos-bundle/tests/{cb2_bundle_authority_flows,cb2_draft2_carriers}.rs`,
  and included into the Cucumber runner as `CB4_SESSION`
  (`cucumber.rs:86`). Adjacent to this domain through § 4.7, not owned by it.

Any change to a vector re-pins its `sha256` in `vectors/ownership.json`;
`shared: true` means the digest is also pinned by the `aithos-service`
repository, so a re-pin there is a cross-repository cost.

## Shared steps, fixtures, and helpers

**None resolve this feature's phrases.** Absence claim with its search:

- Scope of the search: `rust/crates/aithos-bundle/tests/cucumber.rs`, the sole
  step-definition file the runner registers (`fn main`, `:20017-20040`), and
  then the whole of `rust/` as a widening.
- Command: `grep -rn -F "<fragment>" rust/crates/aithos-bundle/tests/cucumber.rs`
  and `grep -rn -F "<fragment>" rust/`, for each of these seventeen fragments
  taken verbatim from `features/g4-client-surfaces.feature`: *caller-supplied
  entropy* · *unlocked local signer* · *delegate_pubkey* · *person or session
  seed material* · *encrypted person keystore* · *browser-local custody* ·
  *WYSIWYS ceremony is signed* · *ceremony proof are posted* · *zeroized* ·
  *URL DOM storage* · *custody interface* · *scripted delegated-session flow* ·
  *redacted verdicts* · *same verify build and sign primitives* · *production
  delegated-session command* · *private seed material is supplied in process
  arguments* · *before any protocol or network effect*.
- Result in `cucumber.rs`: **0 matches for all seventeen**. Result over `rust/`:
  0 for fifteen of them; *caller-supplied entropy* matches once, at
  `rust/crates/aithos-core/src/keys.rs:57`, a doc comment; *process arguments*
  matches twice, at `rust/crates/aithos-cli/src/main.rs:33` and
  `rust/crates/aithos-cli/src/cmd/oauth.rs:2`, both doc comments. No match is a
  `#[given]`, `#[when]` or `#[then]` attribute.

The auditor must therefore resolve every phrase against the runner itself, and
`.fail_on_skipped()` (`cucumber.rs:20029`) is the mechanism that decides what an
unresolved phrase does when the feature is actually selected. This file states
the mechanism; it does not state what the gate will print.

The `ProtocolWorld` fixtures, the five process-global `OnceLock` verdicts at
`cucumber.rs:1119-1129` and the shared helpers at `:7295-7346` are inputs to the
integration pass whether or not this feature reaches them.

## Public surfaces that claim the same invariants

Inspect these to check whether they bypass or contradict the client-surface
verdict, not to audit them.

- **WASM, whole surface** — `rust/crates/aithos-wasm/src/lib.rs`: seven free
  `#[wasm_bindgen]` functions (`genesis_pubkeys` `:28`, `delegate_pubkey` `:58`,
  `verify_mandate_chain` `:69`, `build_session_submandate` `:112`,
  `build_ceremony_challenge` `:217`, `sign_delegated_grant` `:274`,
  `sign_ceremony_challenge` `:307`) and one exported struct `DelegateSigner`
  (`:342`) with four methods (`:349`, `:356`, `:360`, `:368`, `:372`). The
  feature's scenario 1 names four of them by name; the other four are surface
  the same scenario does not name.
- **CLI delegated ceremony** — `rust/crates/aithos-cli/src/delegated_oauth.rs`,
  `authorize_delegated` (`:296-618`): the argument guard (`:297-299`), the
  existing-output refusal (`:300-302`), origin checks on the protected resource
  and the authorization server (`:320-332`), the binding-mismatch check
  (`:399-411`), the two chain verifications (`:420`, `:454`), the callback
  origin/path and `state` checks (`:546-563`).
- **CLI owner ceremonies** — `rust/crates/aithos-cli/src/cmd/owner.rs`; see
  invariant 10 for the seven-versus-two split.
- **CLI local custody** — `rust/crates/aithos-cli/src/custody.rs`:
  `load_keys` / `save_keys` (`:153`, `:186`), `write_private` (`:226`),
  `validate_seed` (`:215`), and the Vault KV2 backend (`:239-337`).
  `KeyMaterial::drop` zeroizes both seed strings (`:55-60`).
- **CLI §03 surfaces** — `cmd/header_seal.rs` (injects one ephemeral and one
  nonce per line at the surface; `--owner-kex-hex` is `REQUIRED` and documented
  as what I3 defines, `:12-23`) and `cmd/header_open.rs`. Named by the recorded
  follow-up `chdr-i3-g4-cli`; see `STATE.md`.
- **Bundle delegated-session path** —
  `rust/crates/aithos-bundle/src/session.rs`, in particular
  `grantee_from_mandates` (`:147`) and the capability accessors (`:160-241`):
  the server-side consumer of what these clients emit.
- **Owner library** — `rust/crates/aithos-owner/src/lib.rs`
  (`owner_grant_session_delegate`, `owner_revoke_mandate_id`), called by
  `cmd/owner.rs:305` and `:328`.
- **npm consumer** — `docker/npm-smoke.mjs`, the only JavaScript in this
  repository that imports the packed `@aithos/core` package. It exercises
  `genesis_pubkeys` against `vectors/a1-genesis.json` and nothing else.

### What is *not* in this repository

The browser application half of scenario 2 — the encrypted person keystore, DOM
storage, the URL, the log surface — has **no production code here**. Absence
claim with its search: `git ls-files | grep -Ei '\.(js|ts|jsx|tsx|html|mjs|cjs)$'`
returns exactly two tracked files, `docker/npm-smoke.mjs` and
`ui-mockup/index.html`; a case-insensitive `grep -n "aithos/core\|wasm\|keystore\|delegate"`
over `ui-mockup/index.html` (873 lines) returns nothing. The client, SDK and
dashboard live in the sibling repositories `aithos-client`, `aithos-sdk` and
`aithos-sdk-example` (`docs/archive/HANDOFF-CLIENT-SDK-G4-INTEGRATION-2026-07-22.md:9-13`).
`rust/Cargo.toml` declares `aithos-client` in `[workspace.dependencies]` at
`path = "../../aithos-client/crates/aithos-client"` — a sibling checkout of the
repository root, which `.github/workflows/ci.yml:23-28` pins at
`c6f615123ca3dc83708ba029b898375409551719` — but **no workspace member inherits
it**: `grep -rn "aithos-client" rust/crates/*/Cargo.toml` and
`grep -rn "aithos_client" rust/crates --include=*.rs` both return nothing.
Whether a gate run needs that sibling checkout present is an operational fact
for the orchestrator to observe, not one this file asserts.

An auditor establishes what this means for the scenario. This section records
where the code is and is not; it draws no conclusion about coverage.

## Known coupling with other features

- `c-headers.feature` — **`COMPLETE`**, and the source of the two recorded
  follow-ups this feature owes. Its two CLI surfaces, `header-seal` and
  `header-open`, are in this domain's territory; its domain file records the
  same seam from the other side
  (`features/.agents/c-headers/DOMAIN.md:200-205`). Full text of the debts in
  `STATE.md`.
- `e-mandates.feature` and `e-mandate-sections.feature` — mandate construction
  and attenuation, the Core layer these clients call
  (`aithos_core::mandate`). A change in `build_sub` or `verify_chain` reaches
  both WASM and CLI.
- `g-revocation.feature` — `verify_mandate_chain` applies revocations through
  `aithos_core::revocation::revocations` (`wasm/src/lib.rs:83`), so the
  revocation semantics of a chain are exercised through this client surface
  too.
- `l-delegated-writes.feature` and `m-delegated-editions.feature` — the
  server-side consumers of the session leaf and the delegated grant these
  clients produce.
- `f-gamma.feature` — the grant entry `sign_delegated_grant` signs is a Gamma
  v1 entry, and `Entry::check_form` (`lib.rs:296`) is Gamma's own guard.
- `a-identity.feature` — `genesis_pubkeys` (`lib.rs:28`) is the WASM projection
  of `OwnerKeys::genesis`, and `docker/npm-smoke.mjs` cross-checks it against
  `vectors/a1-genesis.json`. `a-identity` is `COMPLETE`.
- `k-integration.feature` — cold verification of what these ceremonies emit.

Textual proximity is not a semantic dependency. Inspect these to check whether
they bypass or contradict this feature's verdict, not to audit them.

## Gate pyramid

Canonical feature tag: `@g4-client-surfaces`
(`features/g4-client-surfaces.feature:1`).

**No role in this train executes any of these commands itself.** A role names
the exact command and stops; the orchestrator runs it, hashes the transcript,
journals it under an `evidence_id` and returns the text
(`features/.agents/orchestrator/LEDGER.md:23-53`). A report citing a command
with no matching ledger entry is invalid.

Run the static check from the repository root:

```text
features/.agents/scripts/verify-feature-tags.sh
```

Mandatory before any audit, correction or review (`features/AGENTS.md:55`,
`features/.agents/PROCESS.md:58-59`). It is also CI's first step
(`.github/workflows/ci.yml:21-22`). It requires line 1 of every
`features/*.feature` to be a tag line containing `@<stem>` and line 2 to start
with `Feature:`; `@wip` alongside the canonical tag is explicitly permitted
(the script's own comment, `:19-22`).

Run every Cargo command from the repository root with the workspace manifest.

### Auditor evidence — feature tier, once per immutable revision

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g4-client-surfaces
```

The auditor runs no unfiltered Cucumber, broad regression, or workspace gate. It
may name one exact focused test only to resolve a semantic contradiction.
Read § *Reading the counters* before interpreting this gate's output.

### Focused tier — the exact tests that reach these surfaces

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-cli --test delegated_oauth
cargo test --manifest-path rust/Cargo.toml -p aithos-cli --test cli_surface
cargo test --manifest-path rust/Cargo.toml -p aithos-wasm --lib
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test cb14_delegated_session_chain
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb15_external_delegated_grant
```

Name a single test with `-- --exact <name>` when one semantic contradiction is
at stake. The test names as they stand on disk:

- `aithos-cli --test delegated_oauth`: one test,
  `delegated_oauth_flow_uses_stdin_core_signing_and_a_private_token_file`
  (`tests/delegated_oauth.rs:73`).
- `aithos-cli --test cli_surface`: 24 tests (`grep -c "^#\[test\]"`); the three
  closest to this domain
  are `delegated_oauth_surface_accepts_only_out_of_argv_signer_custody`
  (`:26`), `owner_session_delegate_and_revocation_keep_the_master_seed_off_argv`
  (`:1511`), and `secrets_never_leak_into_outputs_or_certificates` (`:1022`).
- `aithos-wasm --lib`: one test,
  `ceremony_surface_is_deterministic_and_never_returns_a_seed`
  (`src/lib.rs:382`). **`aithos-wasm` has no integration-test binary.** Search:
  `ls rust/crates/aithos-wasm/tests` contains only `fixtures/`, whose
  `vectors.rs` is `#[path]`-included into `src/lib.rs` under `#[cfg(test)]`
  (`:20-22`); there is no `rust/crates/aithos-wasm/tests/*.rs`. `--lib` is
  therefore the only selector that runs it.
- `aithos-core --test cb14_delegated_session_chain`: `cb14_vector_hash_is_frozen`
  (`:53`), `cb14_verified_non_root_leaf_reuses_sc1_and_both_proofs` (`:61`),
  `cb14_chain_revocation_leaf_selection_proof_and_time_fail_closed` (`:69`).
- `aithos-bundle --test cb15_external_delegated_grant`:
  `cb15_vector_hash_is_frozen` (`:31`),
  `cb15_existing_grant_wire_is_accepted_without_delegate_key_custody` (`:39`),
  `cb15_binding_head_time_and_signature_fail_closed` (`:44`).

**There is no test binary named for this feature.** Search:
`ls rust/crates/*/tests/*.rs` over all five crates lists 38 + 19 + 2 + 0 + 0
files; `grep -Ei "g4|client|surface"` over that list matches only
`aithos-bundle/tests/aid_identity_surfaces.rs` (an `a-identity` artefact) and
`aithos-cli/tests/cli_surface.rs`. No file is named `g4*`.

### Relevant regressions — corrector, after the final correction

**Every multi-binary invocation carries `--no-fail-fast`.** `cargo test` aborts
at the first failing test binary; without the flag a multi-binary regression
silently under-reports, which is exactly the defect
`features/.agents/orchestrator/QUEUE.yaml` records as
`chdr-lota-clippy-and-fail-fast`.

```text
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-cli --test cli_surface --test delegated_oauth
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-core --test cb14_delegated_session_chain --test cb2_session_proof --test cb4_operation_contracts
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb15_external_delegated_grant --test vectors_ownership
cargo test --manifest-path rust/Cargo.toml -p aithos-wasm --lib
```

Why each: `cli_surface` pins the CLI surface itself — flag names, help text,
absence of `--seed` and `--private-key` on the delegated ceremony
(`:34-38`), the master seed off `argv` (`:1520-1524`); `delegated_oauth` is the
only end-to-end exercise of the ceremony, over a loopback gateway;
`cb14_delegated_session_chain` covers SC1 over a verified non-root leaf;
`cb2_session_proof` and `cb4_operation_contracts` cover the session-bound
operation contract these leaves feed; `cb15_external_delegated_grant` covers the
grant wire `sign_delegated_grant` produces; `vectors_ownership` fails if any
vector moves without its `sha256` pin being updated. The `aithos-wasm --lib`
line stays single-binary and needs no `--no-fail-fast`; it is listed separately
rather than folded in, so its absence would be visible.

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

### Vector `--check` — corrector, only if a vector is touched

```text
python3 vectors/gen-cb14-delegated-session-chain.py --check
python3 vectors/gen-cb15-external-delegated-grant.py --check
```

Both generators have a `--check` mode
(`gen-cb14-delegated-session-chain.py:294`, `:298`;
`gen-cb15-external-delegated-grant.py:137`, `:141`). **No CI step runs Python**
— `.github/workflows/ci.yml` has two jobs and its steps are checkout, the
feature-tag pre-gate, toolchain, `fmt`, `clippy`, `cargo test`, and the wasm
`cargo check`. So a vector `--check` is verified only when a role names it.
`QUEUE.yaml`'s `chdr-lota-vector-generators` records the wider debt and says it
is owed by "the first cycle to touch a vector".

### Final global gates — corrector, once before review handoff

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings
cargo check -p aithos-wasm --target wasm32-unknown-unknown --manifest-path rust/Cargo.toml
```

Two of these five are named deliberately and are new relative to the older
domain files:

- **`clippy`.** CI enforces it with `-D warnings`
  (`.github/workflows/ci.yml:34-35`) and, per `QUEUE.yaml`'s
  `chdr-lota-clippy-and-fail-fast`, no `DOMAIN.md` in this repository named it
  before this one. A correction that is green on `cargo test` and red on
  `clippy` is red.
- **The `wasm32-unknown-unknown` check.** This domain owns a `cdylib` whose
  target is the browser. CI runs it as a separate job
  (`.github/workflows/ci.yml:39-59`) and a native `cargo test` does **not**
  cover it: a change that pulls `getrandom` or any non-`wasm32` dependency into
  `aithos-wasm` compiles natively and fails only here. `rust/Cargo.toml`
  documents that constraint on the `chacha20poly1305` entry.

`cargo test --workspace` in CI (`ci.yml:37`) carries no `--no-fail-fast`, so CI
under-reports a multi-binary failure the same way; the corrector's own workspace
gate above does carry it, and the two are therefore not interchangeable
evidence.

### Reading the counters

`features/g4-client-surfaces.feature:1` carries `@wip`. The runner filters
`wip` at all three levels — feature, rule and scenario — before executing
anything (`rust/crates/aithos-bundle/tests/cucumber.rs:20034-20038`, and the
comment at `:20030-20033` names this feature as the example). **On the current
revision the canonical feature gate therefore selects zero scenarios of this
feature**, whatever `--tags @g4-client-surfaces` is given.

`features/.agents/orchestrator/LEDGER.md:44-52` is explicit about how that is
recorded: `green` is computed and never asserted, and "exit 0 with zero
scenarios selected — the tag matched nothing" is one of four disagreements
treated as **red**, carrying an `anomaly` field and raising blocking condition
3. Printed counters bind as tightly as the exit code.

The consequence for the role that opens the round is procedural and this file
states it as such: run the gate, record what it actually printed, and do not
substitute this paragraph for the transcript. This paragraph describes a filter
in a source file; it is not evidence about the feature, and no role may cite it
as a gate result. What follows from a zero-selection gate — for the audit, for
the lifecycle, for whether the contract can be traced at all — is the auditor's
determination, and `PROCESS.md`'s evidence hierarchy puts the current
executable code above any document, this one included.

The reference counts of the file on disk at `c406bbf`, for comparison with
whatever a future gate prints: **1 feature / 0 rules / 4 scenarios / 15 steps**.

Never restate a gate result read from a document. Run the gate, or cite the
ledger entry of a run. A written record of a past gate is history, and
`PROCESS.md` § *Evidence hierarchy* is explicit that history is context, not
proof.

## Pilot limits

Audit only the semantic truth of the four existing scenarios. Do not design new
general scenarios, and do not extend the audit into the mandate, revocation,
Gamma, or bundle-session features — report the impact instead. Do not audit the
sibling `aithos-client` / `aithos-sdk` repositories; they are outside this
repository and outside this train. Findings take stable `G4CS-*` identifiers.

## Open questions this domain could not resolve

1. **`PROCESS.md` does not contain four sections that other tracked files cite
   by name.** `features/.agents/orchestrator/BLOCKED.md:7-8` cites `PROCESS.md`,
   section "Blocking conditions", as a closed list;
   `features/.agents/orchestrator/LEDGER.md:5-7` cites section "Orchestrated
   gate execution"; `features/.agents/c-headers/STATE.md:29` and `:65-69` cite
   § *Material isolation of Pass A*; the refutation panel is configured in
   `QUEUE.yaml` (`policy.refuters_per_finding: 3`) and the disclosure gate as
   `policy.disclosure_gate: true`, both without a normative section to point
   at. At `c406bbf` the file has eleven `##` headings — Objective, Feature
   branch lifecycle, Feature targeting and gate pyramid, Current scope,
   Evidence hierarchy, Required two-pass audit, Review-unit isolation and
   impartiality, Artifacts, Manual lifecycle, Evidence statuses, Required run
   conclusion — and none of the four. Roles on this feature obey the rules as
   they are stated in `QUEUE.yaml`, `LEDGER.md` and `BLOCKED.md`, and report
   the gap rather than inventing the missing text.
2. **Which specification section this feature implements is not settled by any
   file.** `spec/09-cli-and-conformance.md` § 9.1 is `DRAFT` and describes an
   `aithos-core …` vocabulary the shipped `aithos` binary does not use; no
   `spec/*.md` file contains the words `wasm`, `browser` or `WYSIWYS` except
   `04-mandates.md` (`grep -rn -i "wasm\|browser\|WYSIWYS" spec/*.md` returns
   four lines, all in `spec/04-mandates.md` and all about the optional WYSIWYS
   digest of an approval receipt: `:1580`, `:1628`, `:1646`, `:1739`). The
   routing above therefore points at § 4.7, § 5.x, § 7 and § 9.1-9.2 as the
   sections whose *rules* these surfaces execute, and records that no section
   governs the client surfaces as such. Whether that is a specification gap is
   a finding for the auditor, not a fact this file asserts.
3. **`docs/audits/features/README.md` reserves no identifier family for this
   feature.** `G4CS-*` is prescribed here and is unused repository-wide
   (search recorded in § *Contract*), but no existing tracked file blesses it.
   The auditor adds the index row when it creates the public audit.

# Implementation audit — `a-identity.feature`

## Metadata

| Field | Value |
|---|---|
| Audited feature | `features/a-identity.feature` |
| Date | 2026-07-29 |
| Observed Git revisions | `2fee855` for the initial audit; `be2d098..56436f3` reviewed from `0601b9f` |
| Observed state | `codex/review-a-identity` was clean before review; immutable candidate `56436f3` was replayed from an exact archive with sibling dependency `aithos-client` at `c6f6151` |
| Primary runner | `aithos-bundle --test cucumber` |
| Primary implementation | `aithos-core::{keys,did,derive,wire}` |
| Inspected surfaces | Core, Bundle, CLI, WASM, Gateway, Client, and Provider where Identity requirements apply |
| Note status | **PROTOCOL DECISION REQUIRED** — AID-002 and AID-005 are `VERIFIED` within pilot scope; AID-001 awaits a Provider decision; AID-003 and AID-004 remain `OPEN` |

## Method provenance

The initial audit traced real Rust production paths and established that all
nine original scenarios executed. The round 1 correction review independently
inspected current code, public surfaces, and test results rather than trusting
the corrector's claims.

Both runs, however, predate the formal two-pass process now recorded in
`features/.agents/PROCESS.md`. Their auditors knew Git context and existing
findings before their code traces were frozen. They are therefore
**history-aware**, not clean history-blind Pass A runs. This limitation does
not erase their direct code/test evidence, but it must not be hidden or
retroactively relabeled.

Any future review that claims full two-pass compliance must:

1. create fresh review units per `Rule` or small risk cluster;
2. trace current scenario → step → production call → result → assertion
   without reading this note, prior runs, or Git history;
3. freeze provisional verdicts;
4. only then inspect the baseline, candidate diff, and prior reports;
5. reconcile the two passes and separately inspect shared state/helpers.

## Verdict

### Independent round 1 correction review

- **AID-001 — `DECISION_REQUIRED`.** Core, closed-wire parsing, Bundle,
  WASM/mandates, Catalog, Gateway, and Client satisfy the targeted strict DID
  criteria. Provider `artifacts::deposit_did` preserves a parallel
  same-DID replacement verifier: it accepts `#succession`, does not validate
  version or key exchange, and does not construct `VerifyingKey` values for
  the incoming Ed25519 fields. It may persist a document that
  `DidDocument::verify` later rejects. P9 explicitly codifies that behavior,
  so a corrector must not silently choose between same-DID Provider
  replacement and §10.4 epoch transition.
- **AID-002 — `VERIFIED`.** `verify_succession(prev, next)` validates both
  documents, transition metadata and signature, DID bindings, and distinct
  identities. The Gherkin `Then` passes the real `next_doc`; every negative
  consumes its own verdict.
- **AID-005 — `VERIFIED_WITHIN_PILOT_SCOPE`.** All 21 added Gherkin cases are
  honest, selected, and green. A real creation ceremony belongs to
  out-of-round AID-003/AID-004. Independently generated negative vectors and
  an automated exact-count gate remain useful improvements, not prerequisites
  for the pilot's semantic correction.
- **AID-003 and AID-004 — `OPEN`.** Neither was changed by round 1.

### Original nine-scenario audit

All nine scenarios were selected and executed real Rust production code. No
step was empty, mocked, tagged `@wip`, or replaced by a global `OnceLock`
verdict.

The original green result still failed to prove the full contract:

- 6 scenarios were `PROVEN` at the exact level exercised;
- 2 scenarios were `PARTIAL`;
- 1 scenario was a `SEMANTIC_FALSE_POSITIVE`;
- strict DID failure, epoch transition, succession independence, and custody
  had implementation/proof gaps.

## Reproduced evidence

### Exact candidate archives

The review worktree could not resolve the lockfile directly because sibling
`aithos-client` referenced the main `../aithos-core`, producing two
same-name/version `aithos-bundle` packages. The reviewer therefore tested
exact Git archives of candidate `56436f3` and client `c6f6151` in the expected
sibling layout without changing repository files.

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
  a1_genesis: 4 passed
  a2_did:     6 passed

cargo test -p aithos-bundle --test aid_identity_surfaces
  2 passed

cargo test -p aithos-bundle --test cucumber
  18 features
  836 scenarios (836 passed)
  3568 steps (3568 passed)
```

The Cucumber output enumerated all **30 Identity scenarios** and their 93
steps as executed and passed.

```text
cargo test --workspace --no-fail-fast
  EXIT=101
  28 targets failed with `Operation not permitted`
```

Identity targets were green. Failures occurred when CLI/Gateway/Provider tests
attempted to open local sockets or services forbidden by the sandbox. An
outside-sandbox retry was denied by execution policy. This workspace gate is
**environmentally inconclusive**, not green.

The non-network Provider runner passed 151/151 scenarios and 992/992 steps,
including P9 `did_rotation_ok`, which is material to AID-001.

```text
cargo fmt --all -- --check
  EXIT=1
  rust/crates/aithos-gateway/src/core_bridge.rs:1355
```

That blob is byte-identical in baseline and candidate (`774672a0…`), so the
format deviation is pre-existing and outside the correction diff. The reviewer
did not rerun Clippy.

### Corrector-reported RED/GREEN evidence

The correction handoff reported the following on Linux with
`rustc 1.95.0` and sibling client `c6f6151`.

Baseline `be2d098`:

```text
cargo test --workspace --no-fail-fast
  627 unit/integration tests passed
  aithos-bundle cucumber: 815 scenarios passed, 3505 steps
```

RED against original semantics, using temporary compatibility shims:

```text
cargo test -p aithos-core --test a2_did
  FAILED: 3 passed; 3 failed

cargo test -p aithos-bundle --test cucumber
  836 scenarios: 818 passed, 18 failed
```

The three Core failures were:

- `aid001_signed_but_semantically_invalid_documents_are_rejected`;
- `aid001_unknown_wire_members_are_refused_not_dropped`;
- `aid002_transition_binds_the_presented_successor_document`.

Eighteen of 21 new scenarios failed under the old semantics. The other three
— foreign succession authority, mismatched `prev_did`, and root claiming
`#succession` — were already rejected and serve as regressions.

Candidate GREEN:

```text
cargo test --workspace --no-fail-fast
  632 unit/integration tests passed
  aithos-bundle cucumber: 836 scenarios passed, 3568 steps
```

The temporary RED shims are not versioned, so the independent reviewer could
not reproduce their exact counts without modifying Rust. The counts remain
reported evidence. The baseline code and diff still establish the old defects
statically, while the candidate GREEN gates were reproduced independently.

### Initial audit evidence

```text
Targeted Gherkin:
1 feature
6 rules
9 scenarios (9 passed)
30 steps (30 passed)

cargo test -p aithos-core --test a1_genesis --test a2_did
a1_genesis: 4 passed
a2_did:     3 passed
```

Temporary public-API probes accepted each of the following before correction:

```text
signed malformed non-root keys accepted: true
signed wrong version/alg/fragment accepted: true
unknown unsigned wire field ignored and accepted: true
transition to malformed DID accepted: true
transition to same DID accepted: true
```

These are now versioned rejection cases. The temporary probe itself was not
retained.

## Evidence map

| Subject | Primary source | Audit role |
|---|---|---|
| Gherkin contract | [`features/a-identity.feature`](../../../features/a-identity.feature) | Stated tested behavior |
| Steps | [`aithos-bundle/tests/cucumber.rs`](../../../rust/crates/aithos-bundle/tests/cucumber.rs) | Executed inputs, calls, state, and assertions |
| Genesis | [`aithos-core/src/keys.rs`](../../../rust/crates/aithos-core/src/keys.rs) | `MasterSeed`, `OwnerKeys`, succession |
| DID and transition | [`aithos-core/src/did.rs`](../../../rust/crates/aithos-core/src/did.rs) | Construction and verification |
| Independent vectors | [`a1-genesis.json`](../../../vectors/a1-genesis.json), [`a2-did.json`](../../../vectors/a2-did.json) | Byte-exact positive proofs |
| Bundle opening | [`aithos-bundle/src/bundle.rs`](../../../rust/crates/aithos-bundle/src/bundle.rs) | Real `did.json` consumption |
| Gateway creation | [`aithos-gateway/src/core_bridge.rs`](../../../rust/crates/aithos-gateway/src/core_bridge.rs) | Effective succession derivation/ceremony |
| CLI custody | [`aithos-cli/src/main.rs`](../../../rust/crates/aithos-cli/src/main.rs), [`custody.rs`](../../../rust/crates/aithos-cli/src/custody.rs) | Owner and succession secret storage |
| Provider deposit | [`aithos-provider/src/artifacts.rs`](../../../rust/crates/aithos-provider/src/artifacts.rs) | Current same-DID `did.json` replacement semantics |

## Scenario matrix

Statuses below describe the candidate after correction. “Before” records the
initial verdict.

| # | Scenario | Before | After | Observation |
|---:|---|---|---|---|
| 1 | Same seed → same identity | `PROVEN` | `PROVEN` | Two real `OwnerKeys::genesis` calls; A1 also fixes all three public outputs. |
| 2 | Different seeds → no shared public key | `PROVEN` | `PROVEN` | Distinct seeds reach derivation; the general property rests on BLAKE3, not exhaustive fixtures. |
| 3 | Three keys pairwise distinct | `PROVEN` | `PROVEN` | Real keys are compared and originate from distinct derivation contexts. |
| 4 | Seed exactly 32 bytes | `PROVEN` | `PROVEN` | `MasterSeed::from_slice` enforces `[u8; 32]`; A1 covers 31 and 33 bytes. |
| 5 | Succession independent and cold | `PARTIAL` | `PARTIAL` | Unchanged; AID-003/AID-004 are outside round 1. |
| 6 | DID lists four public keys | `PROVEN` | `PROVEN` | Calls `DidDocument::build`; A2 fixes positive JCS byte-for-byte. |
| 7 | DID altered after signing fails closed | `PARTIAL` | `PROVEN` | Renamed to its exact proof: post-signature alteration. Other defects have separate scenarios. |
| 8 | Signed but semantically invalid DID (Outline ×7) | — | `PROVEN` | New: malformed/wrong-codec keys and unsupported version/algorithm/fragment, correctly re-signed. |
| 9 | Unknown DID wire member (Outline ×3) | — | `PROVEN` | New: top-level, `keys`, and `signature` members rejected during deserialization. |
| 10 | Succession-signed transition accepts successor | `SEMANTIC_FALSE_POSITIVE` | `PROVEN` | `Then` now passes `next_doc` to `verify_succession`. |
| 11 | Anything else, including root, is rejected | `PROVEN` | `PROVEN` | Unchanged. |
| 12 | Transition does not bind successor (Outline ×10) | — | `PROVEN` | New: mismatched/tampered/malformed/same/non-Aithos DIDs, foreign authorities, version, and algorithm. |
| 13 | Root signs while claiming succession fragment | — | `PROVEN` | New Gherkin regression; A2 already carried this case. |

## Findings and required implementation

### AID-001 — Strict, closed DID verification

**Priority: P1 — `DECISION_REQUIRED`, round 1**

#### Before correction

`DidDocument::verify` validated only root decoding, the `id == did:aithos:<root>`
binding, and the root signature. It did not validate:

- content as Ed25519;
- key exchange as X25519;
- succession as Ed25519;
- `aithos-did-core == DID_VERSION`;
- `signature.alg == "ed25519"`;
- `signature.key == "#root"`;
- absence of unknown JSON members.

Without closed serde schemas, an unknown wire member could be dropped before
verified JCS reconstruction.

#### Delivered correction

- [x] `deny_unknown_fields` on `DidDocument`, `DidKeys`, `SignatureBlock`, and
  `EpochTransition`.
- [x] Explicit DID version, signature algorithm, and `#root` validation before
  signature verification.
- [x] Expected codecs for all four keys; Ed25519 fields construct
  `VerifyingKey` values.
- [x] Existing `id ↔ root` binding and Ed25519 verification preserved.
- [x] Distinct `InvalidDidDocument` errors by defect family.
- [x] Bundle, mandate/WASM, Catalog, Gateway, and Client ordinary paths checked
  against the Core verdict.
- [x] Negative Core, wire, Bundle, and mandate-chain surface tests.

#### Outstanding protocol decision

Provider `artifacts::deposit_did` intentionally verifies replacement under
the stored document's `#succession` authority rather than calling
`DidDocument::verify` on the incoming document. P9 `did_rotation_ok` fixes
that behavior. It does not validate version/key exchange or all incoming
Ed25519 points and can persist a document Core later rejects.

The protocol owner must choose whether:

1. same-DID Provider replacement remains a distinct protocol operation with
   its own fully specified verifier and reopen guarantees; or
2. Provider adopts the §10.4 transition to a different DID.

No round 2 correction should begin until this is decided.

### AID-002 — Bind transition to the actual successor document

**Priority: P1 — `VERIFIED`, round 1**

#### Before correction

`EpochTransition::verify(&prev_doc)` did not receive a successor document. It
could accept a signed declaration with malformed, same, absent, or unrelated
`next_did`. The Gherkin step built `next_doc` but passed only its textual ID,
so “the successor DID document is accepted” proved no successor document.

#### Delivered correction

The ambiguous `verify(prev)` API is removed:

- `verify_declaration(&prev_doc)` verifies only the declaration and is named
  accordingly;
- `verify_succession(&prev_doc, &next_doc)` is the full §10.4 verdict.

The full verifier:

- [x] strictly verifies both documents;
- [x] binds `prev_did == prev_doc.id`;
- [x] binds `next_did == next_doc.id`;
- [x] rejects identical previous and successor identities;
- [x] requires a decodable `did:aithos:` next root;
- [x] validates transition version, algorithm, and fragment;
- [x] verifies under the previous document's succession key.

The versioned negatives cover malformed/non-Aithos/same next DIDs, mismatched
presented successors, post-signature mutation, correctly re-signed malformed
successors, root signatures, foreign succession, mismatched previous DID, and
unsupported transition version/algorithm. Positive A2 JCS remains
byte-identical.

### AID-003 — Remove succession derivation from the owner master

**Priority: P1 — `OPEN`**

Pure Core is correctly shaped: `succession_from_entropy` does not receive
`MasterSeed`, and primary Gateway onboarding draws two entropy values.
However, `owner_init_journal` and `owner_init_context` call
`derived_succession(master, kind, label)`. Compromising the owner master
therefore also compromises the recovery authority.

Required work:

- [ ] remove `derived_succession(master, ...)`;
- [ ] require independent succession entropy or capability for journal/context
  creation;
- [ ] never expose the owner master to the succession-producing component;
- [ ] define the public reference/custody format needed for deterministic
  multi-Ethos creation without reintroducing derivation from `S`;
- [ ] add an architecture guard against future owner-secret derivation.

Expected RED tests:

- [ ] same owner master plus two succession entropies → same owner keys,
  different succession keys;
- [ ] changed owner master without reused succession custody → no implicit
  succession authority;
- [ ] Gateway journal/context creation explicitly requires an independent
  succession source.

Closure requires that no production path recalculate the private succession
key from `S`, an enterprise master, or an owner-derived key.

### AID-004 — Define and enforce cold custody

**Priority: P1 — `DECISION_REQUIRED`**

Managed CLI mode stores `master_seed_hex` and `succession_seed_hex` in the
same `KeyMaterial` object and backend. That does not establish “paper, HSM, or
threshold custody, never on a device that runs agents.” Core also returns a
generic Ed25519 `SigningKey`, whose type allows arbitrary signing.

After a protocol/product decision:

- [ ] define operationally what “cold” requires: separate backend, HSM,
  external capability, one-shot export, or explicitly operator-managed policy;
- [ ] separate succession from `KeyMaterial` if backend separation is
  normative;
- [ ] prefer a bounded `SuccessionSigner` that signs only `EpochTransition`;
- [ ] reject configurations that claim cold custody while sharing active
  master custody, if separation is normative;
- [ ] clarify that “only S is backed up” concerns owner-derived keys, while
  succession remains a separately held secret.

Closure requires a testable rule enforced by real surfaces, not only a
comment in `keys.rs`.

### AID-005 — Strengthen the Gherkin contract and vectors

**Priority: P2 — `VERIFIED_WITHIN_PILOT_SCOPE`, round 1**

Delivered:

- [x] precise post-signature-alteration scenario name;
- [x] 7-row Outline for correctly signed but semantically invalid DIDs;
- [x] 3-row Outline for unknown wire members;
- [x] transition `Then` verifies previous/transition/successor;
- [x] 10 AID-002 negatives plus root claiming `#succession`;
- [x] each row builds its own defect, calls production code, and asserts its
  own result.

Remaining, non-blocking for this pilot:

- [ ] real succession ceremony through identity creation — depends on open
  AID-003/AID-004 and would currently freeze the behavior to be changed;
- [ ] independently generated Python negative A2 vectors — strengthens
  cross-implementation confidence;
- [ ] automated exact Identity scenario-count gate — useful shared runner
  tooling; the reviewer manually verified 30 Identity and 836 Bundle
  scenarios.

No Identity step is empty, proxy, mocked, `@wip`, or backed by a global
`OnceLock`. A1/A2 remain complementary evidence and positive A2 is unchanged.

## Decisions required

1. **Transition API — DECIDED 2026-07-29.** Remove ambiguous `verify(prev)`;
   retain `verify_declaration(prev)` for declaration-only proof and
   `verify_succession(prev, next)` for the full successor verdict.
2. **Provider — OPEN.** Keep same-DID `did.json` replacement as a distinct
   operation, or adopt epoch transition to a new DID?
3. **Cold custody — OPEN.** Is separation technically enforced by Aithos or
   documented as an operator responsibility?
4. **Bounded capability — OPEN.** Must the private succession key stay behind
   an interface limited to epoch-transition signing?

These choices belong in the specification or an architecture decision before
implementation fixes them implicitly.

## Definition of done

This audit may become fully `VERIFIED` when:

- [ ] AID-001 through AID-005 are closed or explicitly decided out of scope;
  AID-002/AID-005 are verified within pilot scope, AID-001 awaits a protocol
  decision, and AID-003/AID-004 remain open;
- [x] no `a-identity.feature` scenario is `@wip`, proxy, or empty;
- [ ] an automated targeted gate enforces the expected Identity count; the
  current count of 30 was verified manually;
- [x] positive A1/A2 tests remain byte-exact;
- [x] new negatives are RED against old semantics and GREEN after correction,
  subject to the disclosed non-versioned-shim limitation;
- [ ] Core, Bundle, WASM, Gateway, and Provider share the decided DID verdict;
  Provider replacement is currently parallel;
- [x] targeted A1/A2 tests pass (4 + 6);
- [x] Bundle Cucumber passes (836 scenarios, 3,568 steps);
- [ ] workspace and Clippy gates pass in a capable environment; workspace was
  sandbox-inconclusive, Clippy was not rerun, and formatting differs only in a
  pre-existing out-of-diff Gateway file;
- [x] exact revisions and results are recorded here;
- [ ] any future full review follows the new isolated Pass A / Pass B process.

## History

| Date | State | Note |
|---|---|---|
| 2026-07-29 | `PROCESS_DISCLOSURE` | Audit artifacts translated to English. The new two-pass process is adopted; initial and round 1 runs are explicitly recorded as history-aware rather than retroactively claimed as clean Pass A. |
| 2026-07-29 | `DECISION_REQUIRED — ROUND 1` | Independent `be2d098..56436f3` review: AID-002/AID-005 verified within pilot scope; AID-001 awaits explicit Provider semantics. Targeted gates: A1 4, A2 6, surfaces 2, Cucumber 836/836 including 30 Identity. Workspace sandbox-inconclusive; pre-existing formatting issue outside diff. |
| 2026-07-29 | `PARTIALLY_CLOSED` | Candidate correction hardened `DidDocument::verify`, closed wire schemas, replaced ambiguous transition verification, grew feature from 9 to 30 scenarios, and added public-surface replay. AID-003/AID-004 remain open. |
| 2026-07-29 | `ANNOTATED` | Inline non-excluding markers added: `@audit-partial` for AID-001/AID-003/AID-004 and `@audit-false-positive` for AID-002; targeted run remained 9 scenarios / 30 steps green. |
| 2026-07-29 | `OPEN` | Initial audit: nine green scenarios, three main implementation gaps, and two proof-strengthening requirements. |

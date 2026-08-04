# Conclusion — correction lot B of `c-headers.feature`: I3 authority

| Field | Value |
|---|---|
| Run type | correction, round 1, lot B, native (not `RECONSTRUCTED`) |
| Role | `correct-c-headers` (`corrector/correct-c-headers/SKILL.md`) |
| Date | 2026-08-04 |
| Orchestrator run journal | `../../../orchestrator/runs/2026-08-04-r1/` |
| Correction branch | `codex/fix-c-headers-i3-authority` |
| Observed revision / baseline | `5be3047a0665d6d6415ec263bd95e044be04c15a` (`spec: apply the I3 authority lot — variant A, retroactive obligation`) |
| Audited revision | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` — `rust/` is byte-identical between `a2087f2` and `5be3047`; the delta is `spec/`, `vectors/` and agent files only |
| Candidate commit | **none** — this role does not commit. The candidate is the working tree at `5be3047` plus the uncommitted diff described below |
| Worktree state | at run end: 13 modified tracked files, 2 new test files, 1 untracked orchestrator run directory. `vectors/` byte-identical to `5be3047` (`git diff --stat 5be3047 -- vectors/` empty). `spec/`, `STATE.md`, `PROCESS.md`, `QUEUE.yaml`, `BLOCKED.md`, `docs/audits/` and `features/c-headers.feature` untouched |
| Scope | `assigned_findings: [CHDR-007, CHDR-012]` and nothing else |
| Review units | U-CHDR-012 — the field on which the owner line is identified, at all four I3 control points and on the wire; U-CHDR-007 — I3 at the edition tier, on both edition verifiers |
| Findings handled | `CHDR-007`, `CHDR-012` → **`IMPLEMENTED`** |
| Findings not handled | the nine of lot A (`CHDR-001`, `-002`, `-009`, `-013`, `-014`, `-016`, `-019`, `-021`, `-025`) — untouched by mandate; `CHDR-024` (`check_rotation` weaker than `spec/03-headers.md:93-96`) — explicitly not assigned by the decision |
| Result | `REVIEW_REQUESTED` |

## Two-pass model and contamination status

The Pass A / Pass B barrier of `PROCESS.md` is the **auditor's** evidence model.
This run does not claim it and is contaminated by mandate: it read the public
audit §6, the decision record of 2026-08-03, `STATE.md`, `DOMAIN.md`, the
applied spec and the C3 vector before touching anything. No history-blind
provisional verdict was frozen here and none is reported.

What is claimed instead is checkable: **this role executed nothing.** Every
result below was produced by the orchestrator and reported back with an
`evidence_id`. No result is asserted that does not carry one, and no
`evidence_id` appears here that the orchestrator did not send. Facts
established by reading the tree (`git show`, `grep`) are marked as such.

## Decision this correction implements, and what it left to the corrector

`decisions/2026-08-03-chdr-007-012-i3-authority.md`, reading A on both findings:
I3 binds the **key**, not the label (`CHDR-012`), and it binds the **edition
verifier** (`CHDR-007`). The spec lot `SI3-1..SI3-10` was already applied at
`5be3047`, so this correction was written against a text already arrested —
`spec/00-overview.md` §0.2, `spec/03-headers.md` §3.1/§3.2/§3.4,
`spec/09-cli-and-conformance.md` §9.2/§9.4.

Two points the decision left open and this run closed, both reported for review
rather than presented as settled:

- **the form of the edition check.** The decision suggested `state.rs:59-67`
  (the enumeration `verify` already pays). This run instead iterates
  `manifest.files` in `Bundle::verify`, because `spec/00-overview.md` §0.2 says
  literally « MUST parse every header the **edition pins** », and because
  keeping `state_tree()` permissive is what lets a writer publish an invalid
  edition that the verifier then refuses — the exact shape of the RED
  (`ev-47ec8aac`). Placing the check in `state_tree()` would have made
  `publish` fail-closed and the finding unprovable at the edition tier;
- **`publication::cold_verify`.** The decision names only `Bundle::verify`, but
  the closure criterion of `CHDR-007` in the public audit names **both**, and
  `spec/09-cli-and-conformance.md` §9.4 binds « every `aithos-core` manifest
  profile ». It was included. Guarded so an edition pinning no header requires
  no `did.json` — which is why the synthetic stores of `cb12` are unaffected.

## The RED → GREEN sequence, in journal order

The order is the proof that this correction corrects something. Both REDs were
taken on the untouched baseline, before any production edit.

| # | `evidence_id` | Command | Result |
|---|---|---|---|
| 1 | `ev-15f8f483` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c3_owner_line` | **RED**, exit 101, 2 passed / 3 failed |
| 2 | `ev-47ec8aac` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition` | **RED**, exit 101, 0 passed / 3 failed |
| 3 | `ev-9f82e070` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c3_owner_line` | **GREEN**, 6/6 |
| 4 | `ev-b925a0cf` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition` | **GREEN**, 3/3 |

What each RED established, on the audited code:

- `ev-15f8f483` — `Recipient::owner` produced `kid: "owner-kex"` where §3.1
  requires `z6LSeYCJg2G3i6zEiYd2bvnacfR8EnQoUUv3315nBbJL85sS`; `validate()`
  **accepted** `owner_label_foreign_key` (a line labelled `"owner"` sealed to a
  stranger) and **rejected** `unlabelled_owner_line` (a line sealed to
  `owner_kex` but labelled otherwise), with
  `I3 violated — header without an owner line: /e/circle v1`. That is
  `CHDR-012` reproduced in both directions;
- `ev-47ec8aac` — `Bundle::verify` returned **`Ok(())`** on an edition whose
  pinned header had lost its owner line, and on one whose `"owner"`-labelled
  line declared a stranger's key. Pins, Merkle roots and signature were all
  recomputed over the mutilated header, so only I3 separated it from a valid
  edition. `CHDR-007` ceased to be a reading of code and became an observed
  behaviour.

## Fact established by execution, not by reading

On `c3_positive_owner_line_is_byte_exact` (`ev-15f8f483`), the built and the
expected `Line` differ **only by `kid`** — `epk`, `n` and `c` identical
character for character. Variant A therefore re-derives **no ciphertext** and
invalidates **no byte-pinned vector**: `kid` is absent from the line AAD
(`seal.rs`, purpose `header-line`, bound to `subject_did ‖ node ‖ key_version`).
The commit message of `5be3047` asserted this from a file reading; it is now
established by execution. It is the reason `c1-header-seal.json` and its pin
never moved, and it should be treated as the load-bearing fact of this lot.

## Exact commands and results — every gate, each by its own `evidence_id`

| `evidence_id` | Command | Result |
|---|---|---|
| `ev-15f8f483` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c3_owner_line` | RED 2/3 (baseline) |
| `ev-47ec8aac` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition` | RED 0/3 (baseline) |
| `ev-9f82e070` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c3_owner_line` | GREEN 6/6 |
| `ev-b925a0cf` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition` | GREEN 3/3 |
| `ev-b19b0db3` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal` | GREEN 3/3, unchanged |
| `ev-2b8ccdc0` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers` | GREEN, **1 feature / 4 rules / 8 scenarios / 28 steps** — the count `DOMAIN.md` requires |
| `ev-fa196226` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal --test g2_rotation --test g3_move --test b2_derivation` | GREEN |
| `ev-6469eead` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb10_structure_vault --test vectors_ownership` | GREEN |
| `ev-8d23a708` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber` | GREEN, 18 features / 114 rules / 836 scenarios / 3577 steps |
| `ev-8eab8e17` | `cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast` | **RED** — `cb2_bundle_structure_vault_historical_hashes_preexisting_green`, `cb2_bundle_structure_vault.rs:133`. See §`g2-rotation.json` |
| `ev-e3b0c442` | `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` | GREEN after `cargo fmt --all`; the pre-fmt red hit six sites and was journalled as purely mechanical |
| `ev-f4579eab` | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test g2_rotation` | GREEN 4/4, after the return to (c′) |
| `ev-6608a56c` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test vectors_ownership` | GREEN, after the return to (c′) |
| `ev-88c136d4` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_structure_vault --test cb2_bundle_concurrency_final` | GREEN — `historical_hashes` back on `be223ff1…`; direct control of `ev-8eab8e17` |
| `ev-8bfeccca` | `cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast` | **GREEN**, 836 scenarios / 3577 steps |
| `ev-03c0fdfc` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber` | GREEN, 18 / 114 / 836 / 3577 |

`ev-8eab8e17` is reported as a red of this run and is **not** erased by
`ev-8bfeccca`. It is the gate that priced the `g2-rotation.json` question.

## The `g2-rotation.json` episode — written as it happened

`vectors/g2-rotation.json` froze the literal `"owner-kex"` in `old_kids` and
`expected_survivor_kids`. Rule 3 of `vectors/README.md` (« frozen once green »)
is mechanised on the **bytes** (`ownership.json` pins the sha256,
`vectors_ownership::vectors_match_their_pinned_digests` enforces it) but stated
on the **values** (« a spec change that would alter one requires a new vector id
and an explicit spec redline »). G2 is exactly where the two diverge.

Three paths were instructed and reported before any edit: **(a)** in-place v2 —
the repo's only real precedent, `78d429a`, which revised `a1-genesis.json` in
place, id `A1` kept, redline named in `description`; **(b)** new vector id — the
letter of rule 3, leaving a frozen orphan still asserting the pre-variant-A
wire; **(c′)** touch nothing — G2's kids are synthetic routing identities,
`zAGENT1` and `zAGENT2` no more real keys than `owner-kex`, and
`check_rotation` now receives the expected owner kid from its caller, so the
vector stays true in its own fiction.

Then, in order:

1. this role **recommended (c′)** and named its weakness — the literal keeps
   displaying a wire form the code no longer produces;
2. **the orchestrator ruled against that recommendation and imposed (a)**, on
   the ground that a conformance vector displaying a dead wire form would
   recreate in miniature what the audit reproaches the repo. The cost was
   assessed on `ownership.json` alone;
3. (a) was implemented: `gen-g.py` deriving the kid from `SEED`, the vector
   revised in place with the redline `5be3047` named in `description`,
   `ownership.json` re-pinned. Focused and regression gates went green;
4. **the workspace gate `ev-8eab8e17` contradicted the ruling.** A second pin of
   `g2-rotation.json` exists in `historical_vector_sha256` of
   `cb2-bundle-structure-vault.json`, and it cascades four levels deep — ten
   artefacts, all in the CB2 qualification tranche, all outside
   `CHDR-007` / `CHDR-012`, i.e. blocking condition 8 of `PROCESS.md`;
5. **the orchestrator reverted to (c′)**, on the perimeter argument rather than
   the cost one, and recorded the reversal as its own.

`vectors/` is now byte-identical to `5be3047`. The knowledge is kept where a
future reader will meet it: a `G2_OWNER_KID` constant in `g2_rotation.rs`
carrying the fiction argument **and** the warning not to reopen (a) without
pricing it first. (a) is not abandoned — it is removed from this lot.

## Corpus-pinning cost inventory — autonomous maintenance item for the owner

Revising one promoted vector of this repo costs four levels of pinning. Nobody
had established this figure; it outlives this cycle and is the reason (a) must
be a change whose subject is the CB2 cascade, not a side effect of an I3 fix.

| Level | Artefact | What must change |
|---|---|---|
| 0 | `vectors/g2-rotation.json` | the vector; its `ownership.json` sha256 |
| 1 | `vectors/cb2-bundle-structure-vault.json` | its `historical_vector_sha256["g2-rotation.json"]` — **derived**, recomputed at generation by `gen-cb2-bundle-structure-vault.py:351`, not hand-frozen |
| 1 | `rust/crates/aithos-bundle/tests/cb2_bundle_structure_vault.rs` | the `VECTOR_SHA256` constant of that oracle |
| 2 | `vectors/cb2-bundle-concurrency-final.json` | its `historical_vector_sha256["cb2-bundle-structure-vault.json"]` |
| 2 | `rust/crates/aithos-bundle/tests/cb2_bundle_concurrency_final.rs` | its own `VECTOR_SHA256` constant |
| 3 | `vectors/cb2-core-bundle-red-ledger.json` | `sha256` **and** `consumer_sha256` for **two** families (`CB2-BUNDLE-STRUCTURE-VAULT-1`, `CB2-BUNDLE-CONCURRENCY-FINAL-1`); `generator_sha256` too if a generator moves |
| — | `vectors/ownership.json` | **four** entries: `g2-rotation.json`, `cb2-bundle-structure-vault.json`, `cb2-bundle-concurrency-final.json`, `cb2-core-bundle-red-ledger.json` |

Corrections to two beliefs held during the cycle, both established by reading
the tree: `cb2-bundle-authority-flows.json` and `cb2-bundle-boundaries.json`
carry the field but **do not** name `g2-rotation.json` — they are themselves
inputs of structure-vault's map; and `historical_vector_sha256` is **not** a
frozen record of a past state, so updating it erases nothing — it is a
generation-time snapshot of sibling digests, and the test that fired did exactly
its job.

## Affected files and symbols

`aithos-core/src/header.rs` — new `pub fn owner_kid(&XPublicKey) -> String`;
`Recipient::owner` now names its key; `check_owner_line` compares `r.pubkey` to
`owner_kex` **and** the derived kid, so a writer can no longer emit a header an
edition verifier would reject. **Five public signatures changed**, as the
decision predicted: `build`, `build_at`, `rotate` take `owner_kex:
&XPublicKey`; `validate(&self, owner_kid: &str)` and `check_rotation(&self, v,
owner_kid: &str)` compare a kid. Two additions: `validate_as_owner` — the
`owner_kex`-bearing tier of §3.1 — and `open_owner` / `open_owner_latest`, which
derive the kid from the key held instead of spelling it out.

`aithos-bundle/src/bundle.rs` — `is_header_file`, `verify_pinned_headers`, and
the call in `Bundle::verify` (**`CHDR-007`**); `Header::build` sites of `init`;
`zone_dk_with_owner_kex`, `vault_dk`, `owner_current_section_key_with_kex`.
`publication.rs` — the same pass in `cold_verify`. `grants.rs` — new
`owner_kex_pub()`, `owner_kex_recipient()` derived from it. Thirteen read sites
that looked up the literal `"owner-kex"` moved to `open_owner*`
(`grants`, `vault`, `revoke` ×4, `log`, `bundle` ×3, `session`). The **three**
sites that decided which line to replace from `line.to == "owner"`
(`revoke.rs`, `structure.rs`, `vault.rs`) now compare the kid — that is
mitigation 2 of `CHDR-012`, which trusted the same label. `aithos-cli`:
`header-seal` takes a required `--owner-kex-hex` and builds the owner line
itself; `header-open` takes `--owner-kid`. No `"owner-kex"` literal remains in
the repository.

New tests: `aithos-core/tests/c3_owner_line.rs` (6 tests, all five C3 cases at
both tiers) and `aithos-bundle/tests/c3_owner_line_edition.rs` (3 tests;
`no_owner_line_at_all` consumed a **second** time at the edition tier, as
`CHDR-007` requires). `cucumber.rs`: **fixture migration only** — the
orchestrator verified that no line beginning `assert`, `panic!` or `expect(`
moved. `g2_rotation.rs`: `line(kid, owner_kid)` / `header_with(kids,
owner_kid)`, the trace of the new signature, plus `G2_OWNER_KID`.

Total: 348 insertions, 81 deletions, 13 files, plus 2 new test files.

## Limits of this conclusion

- **This role executed nothing.** Every gate result is reported by the
  orchestrator under an `evidence_id`. This report cannot attest that those
  commands ran; it attests that no result is claimed without one;
- `IMPLEMENTED` is the ceiling. **`VERIFIED` is not claimed and cannot be**:
  it belongs to the independent reviewer, on an extract of the candidate
  without `.git` and without this report;
- no commit exists. The candidate is a working tree; the reviewer's extract
  must be taken from it, not from a revision;
- the edition check is placed in `Bundle::verify` and `cold_verify` and not on
  read paths. The third reading opened by `CHDR-007` — validation on read paths
  only — is therefore **not** implemented, and the decision did not retain it;
- `CHDR-024` is untouched, as the decision instructs. `check_rotation` remains
  an inclusion where `spec/03-headers.md:93-96` demands an equality;
- headers written before this change carry `kid: "owner-kex"` and would now
  fail `verify`. No such artefact exists in the tree (checked by `grep`), and
  every fixture is built from scratch, but a bundle produced by an older binary
  is not readable by this one. That is the retroactive obligation the owner
  arbitrated, not a regression — it belongs in the impact review;
- the impact review across `g-revocation`, `d-bundle`,
  `n-structural-mutations`, `o-connector-classes-vault` remains owed and is a
  human gesture (`PROCESS.md`, § *Impact review*).

## Reviewer's debt — named, not silently left

The markers of `features/c-headers.feature:47-55` still read
`CHDR-007 and CHDR-012 — DECISION_REQUIRED … neither is assigned to a
corrector`. That has been false since the decision of 2026-08-03. This run did
**not** edit them: the same block also carries `@chdr-009 @chdr-010 @chdr-011`
of lot A, and the marker lifecycle belongs to the reviewer, with a gate re-run
after each edit (`STATE.md`). It is recorded here as the reviewer's debt.

## Next action and expected skill

Independent review by `audit-c-headers`, on `CHDR-007` and `CHDR-012` only.
`STATE.md` moves to `REVIEW_REQUESTED` — **by the orchestrator, not by this
role**. Findings are at `IMPLEMENTED` in this report and nowhere else; the
public audit belongs to the auditor. Lot A follows on its own branch: the three
`Recipient::owner` fixtures of `cucumber.rs` and those of `g2_rotation.rs` have
now migrated, so that file need only be opened once more.

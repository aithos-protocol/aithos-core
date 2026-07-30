# RU-2 Pass A (frozen)

> Frozen 2026-07-30 before Pass B began. Review unit: `Rule: The owner line is
> mandatory (I3)`. Executed by an isolated agent against a source-only extract
> of `3803fe8` with no `.git` present.

## Contamination status

Uncontaminated. Inputs read: `features/.agents/PROCESS.md`, `features/.agents/shared/audit-gherkin-feature/SKILL.md`, `features/.agents/c-headers/auditor/audit-c-headers/SKILL.md`, `features/.agents/c-headers/DOMAIN.md`, the routing/instruction block of `features/.agents/c-headers/STATE.md`, `features/c-headers.feature`, `spec/03-headers.md`, and current Rust sources. No `git` invocation was made (the extract has no `.git`). `docs/audits/features/a-identity.md`, `b-derivation.md`, and `c-headers.md` were **not** read. No prior audit verdict, corrector report, commit message, or diff entered context. No `cargo` command was run.

One non-Cargo static check was run (read-only): `features/.agents/scripts/verify-feature-tags.sh` → `feature tags ok (18 files)`.

## Selection evidence

- Runner: `rust/crates/aithos-bundle/tests/cucumber.rs:19724-19734`. `main` points the runner at `concat!(env!("CARGO_MANIFEST_DIR"), "/../../../features")` — the whole `features/` directory — and calls `ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| !scenario.tags.iter().any(|t| t == "wip"))`.
- `features/c-headers.feature:1` carries the canonical `@c-headers` feature tag; no scenario in the file carries `@wip`, so the `Rule: The owner line is mandatory (I3)` scenario at `features/c-headers.feature:33` is selected by the filter.
- Selection is by directory scan, not by an explicit feature list, so nothing excludes this file.
- `filter_run` (not `filter_run_and_exit`) under `harness = false` means the process exit code is not gate evidence — this matches the `BDER-011` caveat in `DOMAIN.md`. **I did not run the gate**; the orchestrator owns the single canonical run. Expected counts for the whole feature at this revision: 4 Rules / 8 scenarios / 28 steps (I recounted the file and it matches `DOMAIN.md`); RU-2 contributes 1 scenario and 3 steps.
- Step-phrase uniqueness: each of the three phrases resolves to exactly one attribute in `cucumber.rs`, and none of the `regex =` / `expr =` steps in that file can also match them. Runtime ambiguity therefore appears impossible, but I could not confirm it by execution — see *Limits*.

## Scenario matrix

| Scenario | Status | Production path | What the assertion actually compares |
|---|---|---|---|
| A header without an owner line is invalid | `PROVEN` (for its own literal claim), with findings | `Header::build` → `Header::build_at` → `check_owner_line` → `Err(Error::MissingOwnerLine)` — `rust/crates/aithos-core/src/header.rs:108-133`, `:71-77`; error text `rust/crates/aithos-core/src/error.rs:59-60` | Two things, in two different steps: (1) `cucumber.rs:8133` — the `When` panics if `Header::build` returns `Ok`; (2) `cucumber.rs:12348-12349` — the `Then` asserts the stringified error `contains("I3")`. Nothing compares the error *type*, and nothing compares the built header. |

## Per-scenario trace

### A header without an owner line is invalid

**Steps**

- `Given a node key and a single grantee recipient` → `rust/crates/aithos-bundle/tests/cucumber.rs:7575-7576`:
  ```rust
  #[given("a node key and a single grantee recipient")]
  fn single_grantee(_w: &mut ProtocolWorld) {}
  ```
  The body is empty and the World parameter is `_w` — the step does not read or write a single World field. It establishes **no state at all**.
- `When a header is built without the owner line` → `cucumber.rs:8123-8136`. Calls `Header::build(DID_C, NODE_A, &DK, &[grantee_rec("g1", 0x21)], &[eph(1)], &[non(1)])`; on `Ok(_)` it `panic!("a header without an owner line must be rejected")` (`:8133`); on `Err(e)` it stores `w.rejection = Some(e.to_string())` (`:8134`).
- `Then the header is rejected as invalid` → `cucumber.rs:12346-12350`:
  ```rust
  let msg = w.rejection.as_deref().unwrap();
  assert!(msg.contains("I3"), "rejection must name I3: {msg}");
  ```

**Parameter flow**

There is none from the Gherkin. The `Given` phrase names "a node key and a single grantee recipient"; the actual node key and recipient are hard-coded literals *inside the `When`*: `DK = [0x77; 32]` (`cucumber.rs:263`), `NODE_A = "/e/circle"` (`:261`), `DID_C = "did:aithos:test-header"` (`:259`), `grantee_rec("g1", 0x21)` → `Recipient { to: "g1", kid: "g1", pubkey: X25519(0x21…) }` (`:273-279`), `eph(1) = [0x41; 32]` (`:280-282`), `non(1) = [0x61; 24]` (`:283-285`).

So the precondition is **re-created inside the `When`**, not established by the `Given`. Consequence: the scenario's stated setup happens to coincide with what is exercised only because the `When`'s literals happen to be one node key and one non-owner recipient. The `Given` line is decorative — editing it (say, to "two recipients including the owner") would change the contract text while changing nothing executable, and the scenario would still pass. This is exactly the "a `Given` that stores nothing" case `DOMAIN.md` tells this audit to report.

**Assertion**

The load-bearing proof is split across two steps:

1. `cucumber.rs:8133` (in the `When`) is what actually proves *rejection*. If `Header::build` ever returned `Ok`, that panic fails the scenario.
2. `cucumber.rs:12348-12349` (the `Then`) proves only that *some* error occurred whose `Display` text contains the two characters `I3`.

The `Then` is nevertheless fail-closed on its own: `w.rejection.as_deref().unwrap()` panics when `rejection` is `None`, and `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`cucumber.rs:459`), so `rejection` starts as `None` for every scenario. Removing the `When`'s panic would therefore still fail the scenario if `build` succeeded. That materially limits the severity of the split-assertion issue, but the fact remains that the `Then` alone does not distinguish "rejected" from "rejected for the right reason by the right type".

Rejection is checked by **`Display` substring, not by error type**. The typed error is destroyed at the World boundary — `cucumber.rs:8134` stores `e.to_string()`, so the `Then` is structurally incapable of matching `Error::MissingOwnerLine`, even though `Error` derives `PartialEq` (`rust/crates/aithos-core/src/error.rs:6`) and the variant is public (`:59-60`). The matched substring is `"I3"`, satisfied by the `thiserror` template `"I3 violated — header without an owner line: {0}"`.

Could the substring pass on an unrelated error? Not on the traced path: `Header::build` (`header.rs:108-118`) delegates to `build_at` (`:124-152`), whose only fallible statement is `check_owner_line(node, recipients)?` at `:133`; `build_lines` (`:79-101`) is infallible. So `MissingOwnerLine` is the sole reachable error and it is the only variant in `error.rs` whose message template contains `I3` (`I5` at `:73` is the nearest neighbour). The assertion is therefore *currently* exact but *contractually* brittle in both directions: rewording the message (e.g. dropping the "I3" prefix) fails the scenario while the invariant still holds (safe false negative), and conversely any future fallible check added to `build_at` whose payload string happened to embed `I3` — or a node path containing `I3`, since the payload is `node.to_owned()` (`header.rs:75`) — would satisfy the assertion without the owner check having run at all.

**Spec comparison**

`spec/03-headers.md:36-37`: "**I3:** every `key_versions[*].lines` MUST include the owner line. An edition whose any header violates this is invalid."

The scenario covers the first sentence at one enforcement point (construction) and does not reach the second sentence at all. Note also *how* I3 is enforced: `check_owner_line` (`header.rs:71-77`) tests `r.to == OWNER_LABEL` — i.e. the self-declared routing label — and `Header::validate` (`:308-315`) tests `l.to == OWNER_LABEL` on parsed lines. `spec/03-headers.md:33-35` states that `to` is "a routing hint only — the seal is what grants". So I3 as implemented is a label-presence check, not a proof that the owner can open the header. The scenario proves exactly the label-presence semantics, no more.

**Provisional verdict + evidence**

`PROVEN` for the scenario's literal contract, with findings.

Evidence: the `When` calls real production code with no test double (`cucumber.rs:8125-8132` → `header.rs:108`); the rejection path is the real fail-closed I3 gate (`header.rs:133` → `:71-77` → `error.rs:59-60`); the scenario cannot pass unless `Header::build` genuinely returns `Err` (`cucumber.rs:8133` and the `unwrap()` at `:12348` are both fail-closed); and the error observed is the I3 error and no other, because it is the only one reachable from `build_at`.

It is not `SEMANTIC_FALSE_POSITIVE`: the stated outcome is reached and cannot be faked by the current code. It is not `PROXY`: it executes its own case rather than consuming a verdict written by another step. I considered `PARTIAL` on the grounds that the `Rule` header claims I3 generally while the scenario only touches build time; I rejected the downgrade because the scenario's own `When`/`Then` text is explicitly about building, and `DOMAIN.md` puts "absence of a scenario" out of scope. The coverage gap is recorded below as a scope note rather than as a verdict downgrade.

## I3 enforcement-point map

| Point | Code | Semantics | Reached by RU-2 | Reached by any other `c-headers` scenario |
|---|---|---|---|---|
| Build | `header.rs:133` (`build_at`, via `build` at `:108-118`) — `check_owner_line(node, recipients)` on the `Recipient` slice, first statement, before any sealing | `Err(Error::MissingOwnerLine(node))` | **Yes — this is the only point RU-2 exercises** | `Header::build` is called by the `Given`s at `cucumber.rs:8104` / `8113` / `8127` (RU-1, RU-3) and `Header::build_at` by no c-headers step, but always with `owner_rec()` present, so only the *passing* branch |
| Rotate | `header.rs:201` (`Header::rotate`) — same `check_owner_line`, on `survivors` | `Err(Error::MissingOwnerLine(node))` | No | Only the passing branch. `cucumber.rs:8147-8161` (`the node is rotated without the first grantee`, feature line 50) rotates with `&[owner_rec(), grantee_rec("g2", 0x22)]` and `.unwrap()`s. The rejection branch of rotate-time I3 has **no** scenario in this feature |
| Parse / validate | `header.rs:307-316` (`Header::validate`) — `l.to == OWNER_LABEL` on every `key_versions[*]`; and `header.rs:297-303` inside `check_rotation`, which re-checks I3 on the new version | `Err(Error::MissingOwnerLine("{node} v{v}"))` | No | **No c-headers step calls `Header::validate` at all.** A repo-wide grep of `cucumber.rs` for `.validate()` on a header returns nothing; the single `check_rotation` call in the runner is at `cucumber.rs:15260`, inside a `g-revocation` smuggled-recipient step, not a c-headers one |

So all three enforcement points exist in production, and the Gherkin layer of this feature exercises exactly one of them, on one branch.

## Surface inspection

`Header::validate` callers (the parse/validate enforcement point) across the workspace:

- `rust/crates/aithos-bundle/src/bundle.rs:630` (`zone_dk_with_owner_kex`) and `:637` (`vault_dk`) — validated read paths.
- `rust/crates/aithos-bundle/src/session.rs:363`, `rust/crates/aithos-bundle/src/log.rs:425`.
- `rust/crates/aithos-cli/src/main.rs:1397` (`header_open_cmd`).
- `rust/crates/aithos-gateway/src/core_bridge.rs:581`, `:5516`, `:5608`.

Header read sites in the bundle that deserialize a `Header` and **do not** call `validate` before using it:

- `rust/crates/aithos-bundle/src/grants.rs:287` and `:456` (the grant/append paths — a header is parsed, `append_line`d and written back), plus the scan sites `:827`, `:1037`, `:1197`.
- `rust/crates/aithos-bundle/src/structure.rs:199`, `:284`, `:573`, `:751`.
- `rust/crates/aithos-bundle/src/revoke.rs:155` (the rotation path — `check_rotation(new_v)` at `:199` does re-check I3 on the *new* version, but the parsed predecessor is never `validate`d), `:289`, `:365`, `:510`.
- `rust/crates/aithos-bundle/src/vault.rs:114`, `:335`, `:358` (`check_rotation(new_version)` at `:400` again covers only the new version).
- `rust/crates/aithos-bundle/src/bundle.rs:670`.

Edition-level claim (`spec/03-headers.md:36-37`, "An edition whose any header violates this is invalid"):

- `rust/crates/aithos-bundle/src/state.rs:59-68` (`header_hash_at`) parses `header.json` as an untyped `serde_json::Value` and hashes its JCS bytes into the Merkle state tree. It never constructs a `Header` and never calls `validate`. An I3-violating header is therefore hashed and committed like any other file.
- `rust/crates/aithos-bundle/src/bundle.rs:1594-1616` (`publish_artifacts`) and `:1618-1640` (`publish_at`) build and sign the manifest from those hashes with no header validation.
- `rust/crates/aithos-bundle/src/bundle.rs:1652-1758` (`Bundle::verify`, the offline edition verifier) checks the DID document, every manifest signature, the hash chain, pinned-file digests, stray files, the gamma chain and head, and the recomputed Merkle/gamma roots — and **never calls `Header::validate`** on any header in the edition.

Consequence, stated as surface inspection and not as an audit of those features: a header lacking an owner line cannot be *created* through `Header::build` / `build_at` / `rotate` (all three go through `check_owner_line`), but one that arrives by any other route — a hand-edited or externally produced `header.json`, an imported bundle, a future writer that constructs `Header` by literal or by `serde` deserialization — will be hashed into the state tree, pinned, signed into a manifest, and pass `Bundle::verify` unchallenged. It is only rejected later, and only if the reader happens to come through `bundle.rs:630`/`:637`, `session.rs:363`, `log.rs:425`, the CLI, or the gateway. The grant, structure, revoke and vault read paths listed above would consume it without complaint. So yes — a header violating I3 can both enter and survive an edition through paths this scenario never crosses.

## Candidate findings

**CHDR-RU2-a — the `Given` establishes nothing; the `When` re-creates the precondition. Severity: low (contract fidelity).**
Evidence: `rust/crates/aithos-bundle/tests/cucumber.rs:7575-7576` is an empty body taking `_w`; the node key and recipient actually used are literals at `cucumber.rs:8127-8131` (`&DK`, `&[grantee_rec("g1", 0x21)]`). Impact: the Gherkin precondition at `features/c-headers.feature:34` is unexecutable text. Changing it cannot change the test, and the test's real inputs cannot be read from the contract. This is the "empty, generic, or proxy step" case explicitly in scope per `PROCESS.md`. Expected behavior: the `Given` should place the node key and the single grantee `Recipient` in the World, and the `When` should consume them. Smallest correction: give `ProtocolWorld` (or reuse existing fields) a node-key + recipient slot, write it in `single_grantee`, and have `build_without_owner` read it instead of the literals.

**CHDR-RU2-b — the rejection is asserted by `Display` substring, and the typed error is discarded before the `Then` can see it. Severity: low-medium (proof strength / brittleness).**
Evidence: `cucumber.rs:8134` stores `e.to_string()`; `cucumber.rs:12348-12349` asserts `msg.contains("I3")`. The typed variant `Error::MissingOwnerLine` is public and `Error` derives `PartialEq` (`rust/crates/aithos-core/src/error.rs:6`, `:59-60`), so a typed assertion is available and is not used. Impact: the assertion is coupled to the prose of `error.rs:59` rather than to the invariant. It currently cannot pass on a wrong error, because `build_at` (`header.rs:124-152`) has exactly one fallible statement — but that is a property of today's code, not of the test. Any new fallible check in `build_at`, or a node path containing the literal `I3` (the payload is `node.to_owned()`, `header.rs:75`), would let the assertion pass without I3 having been the cause; and rewording the message breaks the scenario while the invariant holds. `DOMAIN.md` requires precisely that a rejection assertion prove *the claimed* rejection. Expected behavior: assert the variant. Smallest correction: store the typed error (e.g. an `Option<Error>` World field, or `Option<Result<Header, Error>>`) in `build_without_owner`, and in `header_invalid` assert `matches!(err, Error::MissingOwnerLine(ref n) if n == NODE_A)`.

**CHDR-RU2-c — the assertion that carries the scenario's outcome lives in the `When`, not the `Then`. Severity: informational.**
Evidence: `cucumber.rs:8133` (`panic!("a header without an owner line must be rejected")`) is the step that proves "rejected"; `cucumber.rs:12346-12350` only qualifies the message. Impact: reading `features/c-headers.feature:35-36` alone misattributes the proof. Mitigating fact, verified: the `Then` is independently fail-closed because `w.rejection.as_deref().unwrap()` panics on `None` and `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`cucumber.rs:459`), so a scenario in which `build` succeeded would still fail. I report this for accuracy of the trace rather than as a defect requiring correction; folding CHDR-RU2-b's typed assertion into the `Then` also resolves it naturally.

**CHDR-RU2-d — no assertion of absence of partial effects. Severity: informational (structurally satisfied today).**
Evidence: `header_invalid` (`cucumber.rs:12346-12350`) asserts nothing about surviving state, unlike the structurally analogous identity step at `cucumber.rs:12511-12524`, which pairs its rejection check with `assert!(w.identities.is_empty(), "no identity may exist after rejection")`. Verified by trace that nothing survives regardless: `check_owner_line` is the *first* statement of `build_at` (`header.rs:133`), before `build_lines`, so no line is ever sealed; `Header::build` is a pure constructor that touches no store, no filesystem and no World; and `build_without_owner` mutates only `w.rejection`, leaving `w.header` at its `Default` `None` (the `Given` writes nothing). So the absence of partial effects is *structural*, not *asserted*. Smallest correction, if wanted: add `assert!(w.header.is_none())` to the `Then`.

**CHDR-RU2-e — I3 is not enforced at the edition level, contrary to `spec/03-headers.md:36-37`. Severity: medium, but out of RU-2's contract scope.**
Evidence: `rust/crates/aithos-bundle/src/bundle.rs:1652-1758` (`Bundle::verify`) contains no `Header::validate` call; `rust/crates/aithos-bundle/src/state.rs:59-68` hashes header files as untyped JSON into the Merkle tree; the grant/structure/revoke/vault read sites enumerated in *Surface inspection* parse headers without validating. Expected behavior per spec: an edition containing any I3-violating header is invalid. I flag this as a **surface observation for the feature auditor**, not as a defect of this scenario: `features/c-headers.feature:33-36` claims only build-time rejection, and `DOMAIN.md` places absence-of-scenario out of scope. It is recorded because the task explicitly asked whether an I3-violating header can enter or survive an edition through a path this scenario never crosses. It does not qualify RU-2's own verdict.

**Not a finding, recorded as a semantic note:** I3 is enforced against the self-declared `to` label (`header.rs:71-77` on `Recipient.to`; `header.rs:308-315` on `Line.to`), while `spec/03-headers.md:33-35` states that `to` is a routing hint and "the seal is what grants". A header whose only "owner" line seals to an attacker's public key satisfies both `check_owner_line` and `validate`. RU-2 proves label-presence enforcement and nothing stronger; the scenario does not claim more, so this is scope-adjacent context for the integrator, not a defect of the scenario.

## Shared-state observations for the integrator

- `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`cucumber.rs:459-462`), so every scenario gets a fresh `Default` World. `rejection` starts `None`; there is no cross-scenario carry-over into RU-2 by that route.
- `rejection` is a genuinely shared World field: written by `cucumber.rs:7796` (`When I try to derive the owner keys`, an identity/derivation step) and by `cucumber.rs:8134` (RU-2); read by `cucumber.rs:12348` (RU-2) and by `cucumber.rs:12511-12524` (`Then genesis is rejected with {string}`). Both readers use substring matching. Because the World resets per scenario this is currently safe, but any future `before`-hook World reuse or a scenario that chained both writers would make the two substring readers indistinguishable. Flag for the integration pass.
- RU-2 leaves `w.header`, `w.saved_line`, `w.opened` and `w.wrap_obj` untouched (`single_grantee` does not take a mutable World reference in practice — the parameter is `_w`; `build_without_owner` writes only `rejection`). RU-2 therefore contributes nothing to cross-scenario coupling and consumes nothing from other units.
- The fixed constants RU-2 uses (`DID_C:259`, `NODE_A:261`, `DK:263`) and the helpers `grantee_rec:273`, `eph:280`, `non:283` are the same fixtures RU-1, RU-3 and RU-4 build on. In particular `eph(1)`/`non(1)` are reused by the RU-1/RU-3 `Given`s at `cucumber.rs:8104` and `:8113`; since RU-2's build always fails before sealing, no ephemeral/nonce collision is observable here, but the integrator should note that the same `(eph, non)` pair is used to seal different key material across scenarios.
- Production contrast worth one line for the integrator: on the real grant path (`rust/crates/aithos-bundle/src/grants.rs:294-300`, `:468-474`) the entropy `ent.e32()` / `ent.e24()` is drawn *as arguments*, i.e. before `Header::build` runs `check_owner_line`. Those call sites always include `owner_kex_recipient()`, so the I3 branch is unreachable there and no entropy is wasted in practice — but the ordering differs from the scenario's, which draws nothing.

## Limits of this conclusion

- **The gate was not run.** Per the assignment the orchestrator owns the single canonical `--tags @c-headers` run, and no `cargo` command was executed here. Selection is established by reading `cucumber.rs:19724-19734` and the feature tags, not by observing an execution. That the scenario actually executes, in the expected order, with these three steps and no ambiguity, is *inferred* from the source and remains unconfirmed by execution.
- **Step-ambiguity is unverified by execution.** Each phrase matches exactly one attribute by grep, and no `expr =` / `regex =` step in `cucumber.rs` appears capable of also matching them, but cucumber-rs reports ambiguity only at runtime.
- **Pass A only.** No history, no prior audit, no commit messages. Whether these step shapes are an artifact of a known past correction, and whether any of CHDR-RU2-a…e was previously raised, cannot be established here and is left to Pass B.
- **Verdict formed on RU-2 alone.** Statements about RU-1/RU-3/RU-4 in the enforcement-point map are limited to reading their scenario titles and the step definitions they call, as authorized, and are not audits of those units.
- The surface inspection (CHDR-RU2-e) is a grep-and-read survey of `bundle.rs`, `grants.rs`, `revoke.rs`, `structure.rs`, `vault.rs`, `state.rs`, `session.rs`, `log.rs`, the CLI and the gateway. I did not exhaustively prove that no *other* code path validates a header before those consumers are reached, only that none of the enumerated call sites does so itself and that `Bundle::verify` does not.

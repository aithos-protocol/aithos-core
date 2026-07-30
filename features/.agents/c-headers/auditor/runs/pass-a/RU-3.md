# RU-3 Pass A (frozen)

> Frozen 2026-07-30 before Pass B began. Review unit: `Rule: Grant is one
> appended line, touching nobody`. Executed by an isolated agent against a
> source-only extract of `3803fe8` with no `.git` present.

## Contamination status

Uncontaminated for RU-3. No Git data exists in the extract (`/work/aithos-core` has no `.git`); no `git log`/`show`/`diff`/`blame` was attempted. `docs/audits/features/a-identity.md` and `b-derivation.md` were not opened. `docs/audits/features/c-headers.md` was not opened and does not appear in the index.

One disclosure: reading the evidence rules in `docs/audits/features/README.md` (explicitly authorised) exposed the index table at lines 74-78, which names prior verdicts for `a-identity` (AID-001…005) and `b-derivation` (BDER-001…012). Those are other features; no c-headers row exists, so no prior conclusion about RU-3 reached me. `DOMAIN.md` and `STATE.md` also state `BDER-011` (runner exits 0 regardless) as a baseline constraint — that is routing/gate context supplied by the assignment, not a prior c-headers verdict.

No cargo command was run. No file was modified.

## Selection evidence

- Feature tag: `features/c-headers.feature:1` is `@c-headers`.
- Runner: `rust/crates/aithos-bundle/tests/cucumber.rs:19724-19734`. It resolves the feature directory as `concat!(env!("CARGO_MANIFEST_DIR"), "/../../../features")` → `features/`, and calls `ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| !scenario.tags.iter().any(|t| t == "wip"))`.
- The only exclusion is the `wip` tag. `grep -n wip features/c-headers.feature` returns nothing (exit 1), so the RU-3 scenario at `features/c-headers.feature:40-44` is not filtered out and is scheduled for execution.
- `harness = false` for this test target (`rust/crates/aithos-bundle/Cargo.toml:45-46`), and `filter_run` (not `filter_run_and_exit`) is used — consistent with the stated `BDER-011` condition that the process exit code carries no pass/fail signal. I did not use any exit code as evidence.
- Step-definition uniqueness: the four phrases of this scenario appear exactly once each as literal `#[given]`/`#[when]`/`#[then]` attributes; no `regex =` or `expr =` step in the file can also match them (`regex` steps are at lines 7827, 7852, 7931, 8823, 11537 and match unrelated anchored patterns). No ambiguous-match risk.
- Cross-feature phrase reuse: `grep` over `features/` shows all four phrases occur only in `c-headers.feature`.

Limit: the *executed* scenario/step counts (the only gate evidence available under `BDER-011`) require the orchestrator's canonical run. Selection here is established statically.

## Scenario matrix

| Scenario | Status | Production path | What the assertion actually compares |
|---|---|---|---|
| Granting a new reader leaves every other line untouched (`features/c-headers.feature:40`) | `PARTIAL` | `Header::append_line` (`rust/crates/aithos-core/src/header.rs:159-177`) → `seal_line` (`rust/crates/aithos-core/src/seal.rs:92-106`) → `line_aad` (`seal.rs:35-37`); read-back via `Header::open` (`header.rs:222-246`) → `open_line` (`seal.rs:110-132`) | (1) `assert_eq!(dk, DK)` where `dk` is what `Header::open(DID_C, 1, "g1", xsk(0x21))` recovers and `DK` is the same constant the `When` handed to `append_line` (cucumber.rs:12325-12331); (2) `assert_eq!(owner_line, w.saved_line…)` — full derived-`PartialEq` equality of the `Line` struct (`to`, `kid`, `epk`, `n`, `c`) between the header's current owner line and a clone captured before the append (cucumber.rs:12353-12360) |

## Per-scenario trace

### Granting a new reader leaves every other line untouched

**Steps**

- `Given a sealed header for the owner` → `rust/crates/aithos-bundle/tests/cucumber.rs:7568` (attribute) on `fn sealed_header_owner_only` at `:7569-7573`. This phrase shares one function body with `a sealed header for the owner on one node` (`:7567`, used by the RU-1 node/version-binding scenario). **The same body runs for both.** It executes:
  - `:7570` `Header::build(DID_C, NODE_A, &DK, &[owner_rec()], &[eph(1)], &[non(1)]).unwrap()` — a single-recipient header, version `"1"`, one line.
  - `:7571` `w.saved_line = Some(header.key_versions["1"].lines[0].clone())` — the snapshot, taken **before** `w.header` is even populated, therefore strictly before the `When`.
  - `:7572` `w.header = Some(header)`.
- `When a line for a new grantee is appended` → `cucumber.rs:8138` on `fn append_grantee_line` at `:8139-8145`: `w.header.as_mut().unwrap().append_line(DID_C, 1, &DK, &grantee_rec("g1", 0x21), eph(5), non(5)).unwrap()`.
- `Then the new grantee opens the node key` → `cucumber.rs:12323` on `fn grantee_opens` at `:12324-12332`. **Shared**: the same function also serves `the grantee opens the header and recovers the node key` (`:12322`), used by the RU-1 scenario `Owner and grantee each open their line` (`features/c-headers.feature:14`).
- `And the owner line is byte-identical to before` → `cucumber.rs:12352` on `fn owner_line_untouched` at `:12353-12361`. This phrase is not shared.

**Parameter flow**

The Gherkin carries no parameters; every value is a fixed fixture (`cucumber.rs:259-285`):

- `DID_C = "did:aithos:test-header"` (`:259`), `NODE_A = "/e/circle"` (`:260`), `DK = [0x77; 32]` (`:263`).
- `owner_rec()` (`:270-272`) = `Recipient::owner(XPublicKey::from(&xsk(0x0A)))`, i.e. `to = "owner"`, `kid = "owner-kex"` (`header.rs:22-28`).
- `grantee_rec("g1", 0x21)` (`:273-279`) = `to = "g1"`, `kid = "g1"`, pubkey from secret `[0x21; 32]`.
- `eph(5) = [0x45; 32]`, `non(5) = [0x65; 24]` (`:280-285`) — distinct from the owner line's `eph(1)`/`non(1)`.

Inside `append_line` (`header.rs:159-177`): `line_aad(DID_C, self.node = "/e/circle", 1)` is recomputed; `seal_line` produces `(epk, c)` from the *new* ephemeral only; `key_versions.get_mut("1")` then `kv.lines.push(Line { to: "g1", kid: "g1", epk, n, c })`. The function reads no field of any existing `Line` — the O(1) property is visible in the code, not in the assertions.

**Assertions (one block each)**

*Block 1 — `the new grantee opens the node key` (`cucumber.rs:12324-12332`).*
`Header::open(DID_C, 1, "g1", &xsk(0x21))` (`header.rs:222-246`) recomputes the same `line_aad`, iterates `key_versions["1"].lines` filtered by `l.kid == "g1"`, hex-decodes `epk`/`n`/`c` and calls `open_line`. Only the appended line has `kid == "g1"` (the pre-existing line has `kid == "owner-kex"`), so this genuinely exercises the newly appended line and proves it is a well-formed ECIES seal under the version-1 AAD. `assert_eq!(dk, DK)` then compares the recovered 32 bytes with the constant.

On the shared-function question: the constants `("g1", 0x21, DK, version 1)` are **correct for this scenario**, not merely for the RU-1 scenario the function was written for. In RU-3 the `When` at `cucumber.rs:8143` appends exactly `grantee_rec("g1", 0x21)` under key `&DK` at version `1`, so kid, secret byte, expected key and key version all line up. The shared function is safe here — but only by coincidence of fixture naming, and there is nothing in either call site that enforces the agreement.

Weakness of this block: the `When` *supplies* `&DK` to `append_line`, and the `Then` checks that `DK` comes back. Spec §3.3 step 1 is "Open the node's current DK (own line)" — that half is never executed by this scenario; `Header::append_line` takes `dk` as a parameter by design, so the open-then-seal composition lives in the callers (`LocalSession::append_header_recipient`, `rust/crates/aithos-bundle/src/session.rs:354-366`, does it correctly). This is a seal/open self-consistency check, not vector-anchored; `vectors/c1-header-seal.json` contains a C1 owner line and grantee line at version 1 for a fixed DID/node but no append case, so it cannot strengthen this block.

*Block 2 — `the owner line is byte-identical to before` (`cucumber.rs:12353-12360`).*
- Origin of "before": `w.saved_line`, written at `cucumber.rs:7571` inside the very `Given` this scenario runs. `grep` over the whole runner shows exactly three occurrences of `saved_line`: the field declaration (`:488`), that single write (`:7571`), and this single read (`:12360`). **The saving `Given` and the running `Given` are the same step definition** — no defect here. `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`:459-460`), so each scenario receives a fresh, `Default`-initialised World; `saved_line` cannot leak in from another scenario.
- It is the owner's line: `lines[0]` of a header built from `&[owner_rec()]` — the only recipient — and `build_lines` (`header.rs:76-100`) preserves recipient order via `recipients.iter().zip(...)`. Its `to` is `"owner"` (`header.rs:23-27`), which is what the `Then` searches for.
- It is a genuine equality, not a re-derivation: `saved_line` is a `Line` *clone* held in the World across the `When`. The compared value is the struct the header currently holds. `Line` derives `PartialEq` (`header.rs:32-39`) over all five fields — `to`, `kid`, `epk`, `n`, `c` — and `epk`/`n`/`c` are hex strings of the full ciphertext material, so this is byte-identity of the entire serialized line, not a label check. It could not be satisfied by a silent rebuild: `append_line` never receives `eph(1)`/`non(1)`, and `seal_line` is deterministic only given those inputs, so a rebuilt owner line would necessarily differ in `epk` and `n`. This block has real force.
- What it does not check: nothing asserts `key_versions["1"].lines.len() == 2`, nothing asserts the owner line is still at index 0, and `find(|l| l.to == "owner")` is position-agnostic. An implementation that inserted the new line at the front, duplicated lines, or dropped an unrelated line would still satisfy this assertion.

**Spec comparison**

Spec §3.3 (`spec/03-headers.md:46-59`) states the contract as three steps and one outcome sentence: "Content untouched, other lines untouched, DK unchanged." Spec §3.1 (`:31-34`) supplies the O(1) rationale: each line carries "its own ephemeral (`epk` stored in the line — this is what keeps grant O(1): appending a line never touches, nor needs, another line's ephemeral)".

Against that:

- "other lines untouched" — exercised for **one** line only. The `Given` at `cucumber.rs:7570` seals to `&[owner_rec()]`, a single recipient, so `key_versions["1"].lines` contains exactly one pre-existing entry. "Every other line untouched" in the scenario title degenerates to "the only other line is untouched". The scenario cannot distinguish an O(1) push from an O(n) rebuild-and-reseal of the remaining recipients, because with n=1 there is no remainder to disturb and no ordering to perturb. A sibling fixture that would give n=2 already exists and is unused here: `sealed_header_owner_grantee` at `cucumber.rs:7552-7565`.
- "DK unchanged" — not asserted. No step re-opens the owner line after the append. It is *logically entailed* by block 2 (identical ciphertext, and `append_line` mutates neither `self.node` nor the version key, so the version-1 AAD is unchanged), but entailment is not the assertion the process asks for, and the `Then` list would not detect a regression that changed the owner's recoverable key while preserving the line bytes — which is impossible today, but is exactly the kind of invariant the scenario claims to guard.
- "appending never needs another line's ephemeral" — true by construction: `append_line`'s signature takes one `ephemeral`/`nonce` and its body (`header.rs:161-176`) reads no existing `Line`. That is code evidence, not scenario evidence; no assertion would fail if the signature grew a dependency on the existing lines.
- "Content untouched" — out of this scenario's reach; the fixture header has no associated content blob. Not claimed by the Gherkin text either.
- §3.3's parenthetical about adding lines to old versions (§3.5) is not claimed by the Gherkin. Out of scope.

**Error path**

`Header::append_line`'s rejection branch — `.ok_or_else(|| Error::SealRejected(format!("no key version {version}")))` at `header.rs:171-172` — is not reached: the `When` passes version `1`, which the `Given` created. Per the process's "absence of a scenario is out of scope" rule and `DOMAIN.md`'s pilot limits, I record this as **uncovered scope, not a finding**.

**Provisional verdict + evidence**

`PARTIAL`.

The scenario is selected, reaches real production code (`Header::append_line`), passes its fixture values into that call, and its second `Then` is a genuine, non-trivial byte-identity check on a snapshot taken before the mutation. It is not a false positive. But two elements of its stated contract are not exercised: the "every other line / touching nobody / O(1)" plurality (one pre-existing line only, `cucumber.rs:7570`) and the "DK unchanged" clause of §3.3 (no post-append owner open). Per the README evidence rules, "the `Then` verifies the scenario-specific result" holds, while "stated boundaries are actually crossed" does not.

## Surface inspection

Every real grant path in the bundle appends through `Header::append_line`; I found no parallel path that rebuilds a version to add a recipient, so nothing silently breaks the byte-identity contract proved here.

Complete caller set of `append_line` (repo-wide grep):

- `rust/crates/aithos-bundle/src/grants.rs:289` — `add_line_on`: loads the stored header, `header.append_line(&did, KV, dk, recipient, ent.e32(), ent.e24())`, writes it back (`:290`). The `None` arm at `:293-301` builds a *new* header for a node that has none, which is the §3.1 "never individually granted" case, not a grant on an existing header.
- `rust/crates/aithos-bundle/src/grants.rs:460` — `deliver_connector_line`: the faithful §3.3 shape — `latest_version()` (`:458`), `header.open(&self.did, version, "owner-kex", &owner.owner_kex)` (`:459`), then `append_line` at the same version.
- `rust/crates/aithos-bundle/src/log.rs:446` — vault audit recipient, appends to `e/x/header.json`.
- `rust/crates/aithos-bundle/src/session.rs:365` — `LocalSession::append_header_recipient`: `header.validate()` then `header.open_latest(&self.subject, "owner-kex", …)` then `append_line` at the recovered version. This is the closest match to spec §3.3 steps 1-3 and is *not* the path the scenario exercises.

Non-grant header writers, checked and cleared as bypasses:

- `rust/crates/aithos-bundle/src/structure.rs:777` — `Header::build_at` for a moved node. This does rebuild every recipient line (`structural_recipients` at `:242-273` reconstructs the recipient set from the old lines), but it is a move to a new node path at a new version, so a new AAD makes fresh lines mandatory; byte-identity cannot and should not hold. Different feature (`n-structural-mutations`).
- `rust/crates/aithos-bundle/src/vault.rs:392` and `revoke.rs:198` — `Header::rotate`, RU-4/`g-revocation` territory.
- `rust/crates/aithos-bundle/src/bundle.rs:552,565` — genesis `Header::build` for the zone roots and `/x`.

One divergence worth handing to the integrator, **not** an RU-3 finding: `grants.rs:289` appends at the hardcoded `KV` constant (`bundle.rs:25`, `pub(crate) const KV: u64 = 1; // single key version until step G (revocation rotates)`), whereas `grants.rs:460` and `session.rs:364` use `latest_version()`/`open_latest`. Since §3.5 retains old versions, after a rotation to v2 `add_line_on` would still find `"1"` present and append there, handing the new grantee the superseded key rather than `key_versions[current]` as §3.3 requires. That belongs to the grants/revocation domains; I report it and do not audit it.

## Candidate findings

**CHDR-RU3-a — "touching nobody" is exercised against a single-line header** (severity: medium)

- Evidence: `features/c-headers.feature:38` (`Rule: Grant is one appended line, touching nobody`) and `:40` (`…leaves every other line untouched`) versus `rust/crates/aithos-bundle/tests/cucumber.rs:7570`, where the `Given` seals to `&[owner_rec()]` — exactly one recipient — and `:7571`, which snapshots `lines[0]` only. The `Then` at `:12353-12360` checks that one line.
- Impact: the scenario proves "the one existing line is unchanged", not "every other line is untouched" and not O(1). An implementation that re-sealed all non-owner lines on every grant, or that reordered `lines`, would pass unchanged. The O(1) claim rests on `header.rs:159-177` reading no existing line — code evidence the scenario does not test.
- Expected behavior per spec: §3.1 (`spec/03-headers.md:31-34`) and §3.3 (`:46-59`) — appending "never touches, nor needs, another line's ephemeral"; "other lines untouched".
- Smallest correction: point the scenario's `Given` at a header with at least two pre-existing recipients — `sealed_header_owner_grantee` (`cucumber.rs:7552-7565`) already builds owner + `g1` — snapshot the whole `lines` vector into a `saved_lines: Vec<Line>` World field, append a *different* grantee (e.g. `grantee_rec("g2", 0x22)`), and assert in the `Then` both `lines[..saved.len()] == saved[..]` (prefix equality, which also pins order) and `lines.len() == saved.len() + 1`. Note this requires the Gherkin `Given` line to change, since the current phrase is shared with the RU-1 binding scenario at `features/c-headers.feature:26`.

**CHDR-RU3-b — no assertion covers "DK unchanged", and §3.3 step 1 is never executed** (severity: low-medium)

- Evidence: the scenario's two `Then`s are `cucumber.rs:12324-12332` and `:12353-12360`. Neither re-opens the owner line after the append. The `When` at `:8143` injects the constant `&DK` into `append_line`, so `assert_eq!(dk, DK)` at `:12331` returns the value the test itself supplied; the "Open the node's current DK (own line)" step of §3.3 (`spec/03-headers.md:52`) is not exercised anywhere in this scenario, although `session.rs:364` implements it.
- Impact: "DK unchanged" is entailed by the byte-identity assertion rather than checked, and the scenario gives no coverage to the open-then-seal composition that a real grant must perform. As written, the first `Then` is a seal/open self-consistency check on freshly injected key material.
- Expected behavior per spec: §3.3 (`spec/03-headers.md:46-59`) — steps 1-3 and "DK unchanged".
- Smallest correction: add one `Then` re-opening the owner line at version 1 (`header.open(DID_C, 1, "owner-kex", &xsk(0x0A))`, asserting `== DK`); optionally have the `When` obtain its `dk` by opening the owner line first, matching §3.3 step 1 and `LocalSession::append_header_recipient`.

**CHDR-RU3-c — the byte-identity `Then` is position- and cardinality-blind** (severity: low)

- Evidence: `cucumber.rs:12355-12360` locates the owner line with `.find(|l| l.to == "owner")` and asserts nothing about `lines.len()` or index.
- Impact: an `append_line` that inserted at position 0, duplicated a line, or removed an unrelated line would still satisfy the assertion. Harmless against today's `kv.lines.push(...)` (`header.rs:173`), but the assertion does not pin the "one appended line" half of the Rule title.
- Expected behavior per spec: §3.3 step 3, "Append it to `key_versions[current].lines`" (`spec/03-headers.md:55`).
- Smallest correction: folded into CHDR-RU3-a's prefix-plus-length assertion.

Recorded as uncovered scope rather than findings, per `PROCESS.md` and `DOMAIN.md`: the `no key version {version}` rejection branch (`header.rs:171-172`) and §3.5's "add a line to the old versions too" clause.

## Shared-state observations for the integrator

- **Multi-phrase `Given`**: `sealed_header_owner_only` (`cucumber.rs:7567-7573`) carries two attributes — `a sealed header for the owner on one node` (RU-1 binding scenario, `features/c-headers.feature:26`) and `a sealed header for the owner` (RU-3, `:41`). One body, so RU-3's `Given` does write `saved_line`; but the RU-1 binding scenario also writes `saved_line` and never reads it. Any future change to this body for RU-1's benefit silently changes RU-3's "before" snapshot, and vice versa.
- **Multi-phrase `Then`**: `grantee_opens` (`cucumber.rs:12322-12332`) serves both `the grantee opens the header and recovers the node key` (RU-1) and `the new grantee opens the node key` (RU-3). It hardcodes kid `"g1"`, secret `0x21`, version `1`, expected `DK`. Correct for RU-3 today only because the RU-3 `When` (`:8143`) happens to append that same recipient at that same version with that same key. Nothing enforces the agreement; if either scenario's fixture changes, one of the two silently stops testing what its text says. Same pattern exists at `:12340-12344` (`opening_rejected` serving two RU-1 phrases).
- **World fields**: `saved_line: Option<Line>` (`cucumber.rs:488`) has exactly one writer (`:7571`) and one reader (`:12360`), both inside c-headers. `header: Option<Header>` (`:487`) is shared far more widely. `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`:459-460`), so state is per-scenario; no cross-scenario carry-over via `saved_line`, `header`, `opened`, `wrap_obj` or `rejection`.
- **Fixture constants** `DK`, `DK2`, `PARENT_KEY`, `DID_C`, `NODE_A` (`cucumber.rs:259-265`) are module-level `const`, i.e. compile-time immutable — no mutable global state, no `OnceLock` involved in this unit.
- **Surface hand-off**: the `KV`-versus-`latest_version()` divergence in `grants.rs:289` described above.

## Limits of this conclusion

- Static selection only. I did not run the Cucumber gate (the orchestrator owns the single canonical run), so the *executed* c-headers scenario/step counts — the only gate evidence available while `BDER-011` is open — are unverified by me. `DOMAIN.md` expects four `Rule` blocks, eight scenarios, 28 steps; the feature file as read matches that shape (4 Rules, 8 Scenarios, 28 steps counted from `features/c-headers.feature`), but confirmation against printed counts is outstanding.
- No focused or mutation test was run, so claims that a given weakened implementation "would still pass" are derived from reading the assertions, not from an executed RED proof. Specifically, CHDR-RU3-a and CHDR-RU3-c are argued from assertion text; a mutation of `header.rs:173` (e.g. `insert(0, …)`) would confirm CHDR-RU3-c empirically.
- Pass A only: no history, no prior audit, no corrector report. Pass B may reveal that some of these gaps are known and deliberate.
- Verdict formed on RU-3 alone. The shared `Then` at `cucumber.rs:12322-12332` and the shared `Given` at `:7567-7568` also serve RU-1 and RU-2 scenarios I did not audit; their correctness there is not asserted here.
- Surface inspection covered `grants.rs`, `bundle.rs`, `structure.rs`, `vault.rs`, `log.rs`, `session.rs`, `revoke.rs` for `append_line` / `Header::build*` / `rotate` / `key_versions` writes. The CLI (`rust/crates/aithos-cli/src/main.rs`) and `aithos-client` were not inspected; the assignment scoped the surface check to the four bundle files plus reported bypasses.

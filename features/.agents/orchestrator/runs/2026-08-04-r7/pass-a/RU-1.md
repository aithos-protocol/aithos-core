# PASS A — `d-bundle`, RU-1 — `Rule: Editions chain and verify offline`

Unit: `features/d-bundle.feature:8`, four scenarios (`:10`, `:16`, `:22`, `:27`),
fourteen steps (`:11`–`:14`, `:17`–`:20`, `:23`–`:25`, `:28`–`:30`).

Material: `/root/work/passA-d-bundle/RU-1`, a `git archive` of `d9120d7`.
`/root/work/aithos-core` was not opened. **No gate, no test and no `cargo`
command was run by me.** Every behavioural claim below is either (a) a reading
of a function body, cited `file:line`, or (b) a **prediction** marked as such,
for which I name the exact mutant and ask the orchestrator to run it. Nothing
here is presented as a measured behaviour, because no evidence id has been
handed to me yet.

Finding identifiers in this report are numbered **`DBND-101` upward**, per the
brief's instruction that each of the seven Pass A auditors take a distinct
hundred so the integration pass can renumber without collision.

---

## 0. Resolution map (reproduced, not trusted)

All six step definitions my fourteen Gherkin lines resolve to, traced by
matching each phrase against every `#[given]` / `#[when]` / `#[then]` attribute
in `rust/crates/aithos-bundle/tests/cucumber.rs`:

| Gherkin line | Phrase | Definition |
|---|---|---|
| `:11` | `a fresh identity` | `a_fresh_identity`, `cucumber.rs:7696-7699` |
| `:12` | `I initialise its bundle` | `initialise_bundle`, `cucumber.rs:8322-8336` |
| `:13` | `edition 1 verifies offline` | `edition_verifies`, `cucumber.rs:12697-12701` — **shared attribute**, also carries `its integrity checks against the signed edition` (`:12698`, RU-3's `:51`) |
| `:14` | `the manifest pins the DID document` | `manifest_pins_did`, `cucumber.rs:12703-12718` |
| `:17` | `an initialised bundle` | `an_initialised_bundle`, `cucumber.rs:7701-7704` |
| `:18` | `I create circle folder … tagged …` | `create_circle_content`, `cucumber.rs:8338-8341` |
| `:19` | `I publish the edition` | `publish_edition`, `cucumber.rs:8343-8347` — **shared attribute**, also carries `the edition is republished` (`:8344`, RU-2's `:42`) |
| `:20` | `edition 2 verifies and pins edition 1 as its predecessor` | `edition_two_verifies`, `cucumber.rs:12720-12736` |
| `:23` | `a published bundle` | `a_published_bundle`, `cucumber.rs:7706-7712` — **shared attribute**, also carries `a bundle with two editions` (`:7707`) |
| `:24` | `one byte of a pinned file is altered` | `alter_pinned_file`, `cucumber.rs:8349-8355` |
| `:25` | `edition verification is rejected` | `edition_rejected`, `cucumber.rs:12738-12741` |
| `:28` | `a bundle with two editions` | `a_published_bundle`, `cucumber.rs:7706-7712` — **same function as `:23`** |
| `:29` | `the newest manifest claims a wrong predecessor hash` | `wrong_predecessor`, `cucumber.rs:8357-8379` |
| `:30` | `edition verification is rejected` | `edition_rejected`, `cucumber.rs:12738-12741` |

World helpers reached: `ProtocolWorld::init_bundle` (`:7358-7371`),
`add_circle_section` (`:7374-7395`), `publish_bundle` (`:7397-7400`),
`latest_manifest` (`:7402-7410`), `owner` (`:7452-7455`).

Production surfaces reached: `Bundle::init`
(`rust/crates/aithos-bundle/src/bundle.rs:558-640`, which ends at `:628`
`bundle.publish_at(owner, now, 1)` — **initialisation does publish edition 1**,
which settles the question INVENTORY § 4.1 raised about S1's title saying
"publishes" where the step says "initialise"), `Bundle::publish` (`:1678-1681`)
→ `publish_at` (`:1657-1676`) → `publish_artifacts` (`:1631-1655`) →
`all_pinned_files` (`:1616-1625`), `Bundle::verify` (`:1691-1795`),
`Manifest::build` / `build_spec` / `chain_hash` / `verify_signature`
(`src/manifest.rs:100-124`, `:126-158`, `:93-95`, `:240-257`),
`verify_pinned_headers` (`src/bundle.rs:302-321`),
`Bundle::state_tree` → `src/state.rs`.

**Proxy-verdict check, search reproduced rather than inherited.** The brief
warns of step bodies that consume a shared process-lifetime cached verdict.
`grep -n "get_or_init\|OnceLock" rust/crates/aithos-bundle/tests/cucumber.rs`
returns the eight `OnceLock` statics at `:1119-1129` and their nine
`get_or_init` call sites at `:7290`, `:7296`, `:7307`, `:7316`, `:7324`,
`:7334`, `:7342`, `:7350`. **None of the six functions in the table above
contains a `*_result` call or reads a `cb*_result` World field.** Every one of
my fourteen steps executes its own arrangement. (`DOMAIN.md:447-459` asserts the
same and asks to be re-checked rather than trusted; I re-checked it and agree.)

**Source-text-assertion check.** `grep -n "include_str!"` over the six bodies
and their transitive helpers: no hit. The one `include_str!` site DOMAIN.md
records (`core_capability_api_is_narrow`, `cucumber.rs:2053-2058`) is reached
only from RU-7, not from this unit.

**Runner check.** `cucumber.rs:20016-20038` uses `.fail_on_skipped()` and
`filter_run_and_exit`; the only filter is `@wip`, and `d-bundle.feature` carries
one tag, `@d-bundle` (`:1`), with no tag on any Rule or Scenario. So all four of
my scenarios do execute and an unresolved phrase would be an error, not a silent
skip.

---

## 1. Scenario `:10` — *Initialising a bundle publishes a verifiable first edition*

**Claim.** Initialisation, on its own, produces a **first** edition that can be
verified **offline**, and that edition's manifest **pins** the DID document.

**What executes.**

- `:11` `a_fresh_identity` (`cucumber.rs:7696-7699`) is one statement:
  `w.seeds.push((0u8..32).collect());`. It creates no identity object. The
  `OwnerKeys` are derived inside the `When` at `cucumber.rs:8324`
  (`let owner = w.owner(0);` → `OwnerKeys::genesis`, `:7452-7455`).
- `:12` `initialise_bundle` (`:8322-8336`) calls
  `Bundle::init(MemStore::default(), &owner, &succession.verifying_key(), &mut w.ent, NOW)`
  and stores it in `w.bundle`. `Bundle::init` (`bundle.rs:558`) writes
  `did.json`, two zone headers, the vault header, three indexes, the sealed
  self root descriptor, and finishes with `publish_at(owner, now, 1)`
  (`:628`) — so edition 1 exists.
- `:13` `edition_verifies` (`:12697-12701`) is
  `w.bundle.as_ref().unwrap().verify().expect("edition valid");` and nothing
  else.
- `:14` `manifest_pins_did` (`:12703-12718`) reads `manifest.json` through
  `latest_manifest()` (`:7402`), reads `did.json` from the same store, and
  asserts `manifest.files["did.json"] == sha256_hex(did_bytes)`.

**Do the two meet?** Partly.

- *"publishes … a first edition"* — **established**, by `bundle.rs:628`. The
  title's "publishes" is not a lie: `init` calls the same `publish_at` that
  `publish` does.
- *"first"* — **not asserted**. `edition_verifies` (`:12697`) makes no claim
  about `edition.height`. Compare `edition_two_verifies` (`:12720`), which does
  assert `latest.edition.height == 2` at `:12724`. The ordinal in this
  scenario's `Then` is decoration. → **`DBND-105`**.
- *"offline"* — **not asserted**. `verify()` is called on the live `Bundle`
  object the `When` built, over the same in-process `MemStore`. Nothing is
  exported, nothing is reopened, and no assertion distinguishes "verified from
  the files" from "verified from whatever this object holds". → **`DBND-105`**.
- *"the manifest pins the DID document"* — asserted, but the assertion is
  **strictly subsumed** by the preceding step: no store state exists in which
  `:14` fails and `:13` passes. → **`DBND-104`**.

---

## 2. Scenario `:16` — *Every publication extends the chain*

**Claim.** Publication appends to the edition chain rather than replacing or
branching it; edition 2 verifies and pins edition 1.

**What executes.**

- `:17` `an_initialised_bundle` (`:7701-7704`) → `init_bundle()` (`:7358-7371`):
  pushes a seed, derives the owner, `Bundle::init` → edition 1.
- `:18` `create_circle_content` (`:8338-8341`) → `add_circle_section`
  (`:7374-7395`): `ensure_folder(Zone::Circle, "projets/perso", …)` then
  `section_add(SectionSpec{ zone: Circle, folder_path, name, title: "note",
  tags: ["toto"], body: BODY, now: NOW }, …)`.
- `:19` `publish_edition` (`:8343-8347`) → `publish_bundle()` (`:7397-7400`) →
  `Bundle::publish` (`bundle.rs:1678`) → `publish_at(owner, now, 2)`, which at
  `:1658-1662` reads `manifests/1.json` and sets
  `prev_hash = prev.chain_hash()?`.
- `:20` `edition_two_verifies` (`:12720-12736`): `verify().expect(…)`; then
  `assert_eq!(latest.edition.height, 2)`; then deserialises `manifests/1.json`
  and `assert_eq!(latest.edition.prev_hash, first.chain_hash().unwrap())`.

**Do the two meet?** For the *singular* claim, yes — and this is the strongest
scenario in the unit.

The `Then` is not a round-trip on its own `When`: `chain_hash()`
(`manifest.rs:93-95`) is recomputed independently from `manifests/1.json`'s
stored bytes, over `unsigned_jcs()` (`:88-92`), and compared against what
`publish_at` wrote. A mutant that makes `publish_at` stop pinning
(`bundle.rs:1658-1662` → `let prev_hash = String::new();` unconditionally) is
predicted to turn this scenario **red** at the first assertion, via
`verify()`'s `bundle.rs:1726-1730` arm, while leaving scenario `:10` green
(edition 1's `prev_hash` is legitimately empty). I ask for that mutant as
**M5**, precisely because a scenario I intend to call sound should be shown
capable of dying.

Two weaknesses, neither fatal:

- The third assertion (`prev_hash == first.chain_hash()`) is **subsumed** by
  the first: `verify()` walks `1..=latest.edition.height` and performs the same
  comparison at `bundle.rs:1726-1730`. The only assertion in `:20` that
  `verify()` does not already make is `height == 2`.
- *"Every publication"* is demonstrated on **one** publication, and *"rather
  than branching"* on **zero** forks. → **`DBND-106`**.

---

## 3. Scenario `:22` — *A tampered file fails the edition*

**Claim.** Altering a file that the edition **pins** makes that edition fail
verification.

**What executes.**

- `:23` `a_published_bundle` (`:7706-7712`): `init_bundle()` (edition 1),
  `add_circle_section("projets/perso","note1","toto")`, `publish_bundle()`
  (edition 2). **This `Given` never verifies anything.**
- `:24` `alter_pinned_file` (`:8349-8355`):

  ```rust
  let mut bytes = bundle.store.get("e/circle/index.json").unwrap().unwrap();
  bytes[10] ^= 1;
  bundle.store.put("e/circle/index.json", &bytes).unwrap();
  ```

  It writes through the `pub store` field (`bundle.rs:284`), bypassing
  `validate_store_key` — the same surface `docs/audits/features/c-headers.md`
  §6bis records `c3_owner_line_edition.rs:239-246` using.
- `:25` `edition_rejected` (`:12738-12741`):
  `assert!(w.bundle.as_ref().unwrap().verify().is_err());` — the error is never
  inspected.

**Do the two meet?** The claim is reached, but the *proof* does not isolate it,
in two independent ways.

**(a) The rejection has at least two other sufficient causes.** In `verify()`
the flat-pin loop is `bundle.rs:1749-1755`. Downstream of it, `:1780-1789`
recomputes the Merkle state roots and compares them to `latest.roots`; that
recomputation starts at `state.rs:76`, `let index: ZoneIndex =
self.get_json("e/circle/index.json")?`. `latest.roots` is non-empty for this
fixture, because `publish_artifacts` (`bundle.rs:1631-1655`) always sets
`roots: tree.roots`. So if the flat-pin loop were deleted outright, verify()
would still error — either because the mutated bytes no longer deserialise as
`ZoneIndex` (`bundle.rs:29-33`), or because the recomputed roots differ from the
pinned ones. Either branch reaches `is_err()`. This scenario cannot tell the
difference. → **`DBND-102`**, mutant **M2**.

  (Arithmetic aside, which the mutant does not depend on: for
  `serde_json::to_vec_pretty` of a `ZoneIndex` — the writer `put_json` uses,
  `bundle.rs:410-414` — the first bytes are `{`, `\n`, two spaces, then
  `"folders"`, so index 10 is the `r` of that key and the flip renames the
  field. If so, the file no longer deserialises at all, which is a *schema*
  corruption, not the byte-level content rollback the pins exist for. I flag
  this as a prediction, not a fact; `DBND-102` stands either way.)

**(b) The tamper avoids the only file class the flat pins uniquely cover.**
`manifest.rs:33-35` states the design reason for keeping flat pins beside the
Merkle roots, verbatim:

> `/// Flat file pins — kept BESIDE the Merkle roots (decided 2026-07-11):`
> `/// they still cover byte-rollback of sealed self blobs (§02.8).`

Search backing the absence claim: `grep -n "blob"
rust/crates/aithos-bundle/src/state.rs` returns exactly three hits — `:227`
(`for row in &index.blobs`), `:318`, `:321` — all of which read `SelfIndex`
**index rows**, none of which reads any `blobs/*.enc` byte. Scope: the whole of
the state-tree builder, the only recomputation `verify()` performs besides the
gamma roots. So a byte flipped inside `e/circle/blobs/<sid>.enc` or
`e/self/blobs/<sid>.enc` is caught by the flat pin and by **nothing else** —
and that is precisely the tamper the scenario does not perform.

**(c) No positive control.** `:23` never establishes that this bundle verified
before `:24`. → **`DBND-103`**.

---

## 4. Scenario `:27` — *A broken chain fails closed*

**Claim.** A chain whose predecessor linkage is wrong is rejected **by
default** — deny rather than warn-and-continue.

**What executes.**

- `:28` `a_published_bundle` (`:7706-7712`) — **the same function as `:23`**.
  Edition 1 from `init`, edition 2 from `publish`. The phrase "a bundle with two
  editions" is honest: the fixture does have two.
- `:29` `wrong_predecessor` (`:8357-8379`): reads `manifest.json` (height 2),
  builds a **new, validly root-signed** `Manifest` at `height + 1 = 3` with
  `prev_hash = "0".repeat(64)`, copying `latest.files`, `latest.roots`,
  `latest.gamma_roots`, `latest.gamma_counts_root`, `latest.gamma_head`
  unchanged, and writes it to both `manifests/3.json` and `manifest.json`.
- `:30` `edition_rejected` (`:12738-12741`): `verify().is_err()`.

**Do the two meet?** The claim is reached in the unmutated tree — reading
`verify()` in execution order, the first error the forgery can produce is
`bundle.rs:1726-1730`, `"broken chain at height 3"` — but the *assertion* does
not depend on it, and this is the finding I regard as the unit's most serious.

The forged manifest reuses `latest.files`. That map was produced by
`all_pinned_files(exclude_latest = 2)` (`bundle.rs:1616-1625`), whose skip
condition is

```rust
if path == "manifest.json" || path == format!("manifests/{exclude_latest}.json") {
```

so **`manifests/2.json` is absent from edition 2's own `files` map, by
construction.** That was harmless while the tip was height 2, because
`verify()`'s unpinned-stray check (`bundle.rs:1760-1768`) exempts
`manifests/{latest.edition.height}.json`. The forgery moves the tip to height
**3**, so the exemption now names `manifests/3.json`, and `manifests/2.json`
becomes an unpinned stray.

Consequently: **delete the chain-linkage comparison entirely and `verify()`
still returns `Err` on this fixture** — `"unpinned file: manifests/2.json"`.
The `Then` asserts only `is_err()`, so the scenario is predicted to stay
**green** under a mutant that removes the very check its name is about. →
**`DBND-101`**, mutant **M1**.

Separately, "fails closed" as a claim about the *failure mode* — deny-by-default
rather than warn-and-degrade — is not distinguished from any other rejection by
`is_err()`. INVENTORY § 4.1 raised this from the text alone; the code confirms
there is nothing behind it. It is folded into `DBND-101`'s closure criterion
rather than numbered separately, because one error-identity assertion discharges
both.

---

## 5. Findings

### `DBND-101` — P2 — S4's rejection is over-determined; the chain check it names is not load-bearing for the assertion

**Statement.** In `features/d-bundle.feature:27-30`, `wrong_predecessor`
(`cucumber.rs:8357-8379`) publishes a forged tip at `height + 1` whose `files`
map is copied verbatim from the previous edition. Because
`all_pinned_files` (`bundle.rs:1616-1625`) excludes `manifests/{height}.json`
from that edition's own pins, advancing the tip makes `manifests/2.json` an
unpinned stray under `verify()`'s stray check (`bundle.rs:1760-1768`). The
`Then` (`edition_rejected`, `cucumber.rs:12738-12741`) asserts only
`verify().is_err()`. The scenario is therefore predicted to remain green with
the predecessor-hash comparison (`bundle.rs:1726-1730`) removed — a defect that
would let a bundle with a fabricated chain link verify, which is exactly what
the scenario name promises to prevent.

**Evidence.** Reading, `file:line` as above. **Behavioural confirmation
requested: mutant M1 (§ 7), predicted GREEN where the scenario name predicts
RED.** No evidence id has been issued to me; this finding is filed as a
prediction and must be marked confirmed or withdrawn once M1 runs.

**Closure criterion.** Both of:
1. `edition_rejected` — or a chain-specific successor step — asserts the error
   *identity*, e.g. that `verify().unwrap_err().to_string()` contains
   `broken chain at height 3`; and
2. `wrong_predecessor` removes the confound, by inserting
   `manifests/2.json` with its true `sha256_hex` into `forged.files` before
   signing, so that the wrong `prev_hash` is the only remaining cause of
   rejection.
Then M1 must turn the scenario RED. An implementer can check this without
asking me anything: apply M1, run the unit, observe red.

---

### `DBND-102` — P2 — S3 never exercises what "pinned" uniquely means

**Statement.** `alter_pinned_file` (`cucumber.rs:8349-8355`) tampers with
`e/circle/index.json`, a file that `verify()` re-derives twice: once through
the flat pins (`bundle.rs:1749-1755`) and again through the Merkle state-root
recomputation (`bundle.rs:1780-1789` → `state.rs:76`). Deleting the flat-pin
loop is predicted to leave the scenario green. Meanwhile the file class the
flat pins were deliberately retained for — sealed blobs, per `manifest.rs:33-35`
verbatim: *"Flat file pins — kept BESIDE the Merkle roots (decided 2026-07-11):
they still cover byte-rollback of sealed self blobs (§02.8)."* — is never
touched by any RU-1 step. Search backing the absence:
`grep -n "blob" rust/crates/aithos-bundle/src/state.rs` → `:227`, `:318`,
`:321`, all `SelfIndex` index rows, none reading `.enc` bytes; scope, the whole
state-tree builder. Repository-wide search for a byte-flip tamper:
`grep -rn "\^= 1" rust/ --include=*.rs` → three hits,
`cucumber.rs:8353` (this step), `aithos-core/tests/c1_header_seal.rs:110` and
`:112` (a header ciphertext, not a bundle blob, and not an edition check). So
byte-rollback of a sealed blob is asserted nowhere I can find.

**Evidence.** As above. **Behavioural confirmation requested: mutant M2 (§ 7),
predicted GREEN.** Filed as a prediction pending an evidence id.

**Closure criterion.** `alter_pinned_file` flips a byte inside a sealed blob —
`e/circle/blobs/<sid>.enc` or `e/self/blobs/<sid>.enc`, resolved from the index
rather than hard-coded — and `edition_rejected` asserts the error contains
`pinned file altered:` followed by that path. With that change, M2 must turn the
scenario RED and M1 must leave it green. If the project prefers to keep the
index tamper as well, add it as a second scenario; the blob tamper is the one
that discharges the finding.

---

### `DBND-103` — P3 — S3 and S4 carry no positive control

**Statement.** Both negatives assert only `verify().is_err()`
(`cucumber.rs:12738-12741`), and their shared `Given` (`a_published_bundle`,
`cucumber.rs:7706-7712`) never establishes that the bundle verified before the
mutation. A regression that makes `Bundle::verify` reject unconditionally, for
a reason having nothing to do with chains or pins, keeps both scenarios green.

Severity is P3 rather than P2 because a control does exist, one Rule-block away
and on a byte-identical arrangement: scenario `:16` builds the same
init + `add_circle_section` + `publish` fixture and asserts `verify()` succeeds
(`cucumber.rs:12721`). The control is real but it is *cross-scenario* — it is
lost by any run filtered to `:22`/`:27`, by a fixture edit that touches only the
`a_published_bundle` path, and by a reader auditing either scenario alone.

**Evidence.** Reading. **Behavioural illustration requested: mutant M3 (§ 7),
predicted to leave `:22` and `:27` GREEN while turning `:10` and `:16` RED.**

**Closure criterion.** Each of the two negative scenarios establishes its own
control before mutating — either a Gherkin step (`And edition verification
currently succeeds`) between the `Given` and the `When`, or an
`assert!(bundle.verify().is_ok(), …)` as the first statement of
`alter_pinned_file` and `wrong_predecessor`. Verified by: apply M3, observe all
four scenarios red.

---

### `DBND-104` — P3 — `the manifest pins the DID document` adds no detection power to the step before it

**Statement.** `manifest_pins_did` (`cucumber.rs:12703-12718`) asserts
`manifest.files["did.json"] == sha256_hex(store["did.json"])`. Every store state
falsifying that assertion is already rejected by the immediately preceding
`edition_verifies` (`cucumber.rs:12697-12701` → `Bundle::verify`): a wrong hash
trips the flat-pin loop (`bundle.rs:1749-1755`); an absent `did.json` key trips
the unpinned-stray check (`bundle.rs:1760-1768`, whose only exemptions are
`manifest.json` and `manifests/{height}.json`). The step is therefore a
tautology in position, and it never demonstrates the property its name suggests
— that an edition is *bound* to that DID document, i.e. that substituting a
differently-keyed `did.json` breaks verification.

**Evidence.** Reading. **Behavioural confirmation requested: mutants M4a and
M4b (§ 7).** M4a (drop `did.json` from `all_pinned_files`) and M4b (poison its
pinned hash) are both predicted to turn the scenario RED — but *at step `:13`,
not at step `:14`*, which is the finding.

**Closure criterion.** Replace or supplement `:14` with a demonstration:
after `Bundle::init`, overwrite `did.json` through the `pub store` field with a
second, internally-consistent, differently-rooted DID document, and assert
`verify()` errors naming `pinned file altered: did.json`. Verified by: the new
assertion fails if `did.json` is removed from `all_pinned_files`' output while
the stray check is also relaxed for it.

---

### `DBND-105` — P3 — `edition 1 verifies offline` asserts neither the ordinal nor the offline property

**Statement.** `edition_verifies` (`cucumber.rs:12697-12701`) is a bare
`verify().expect(…)`. It makes no assertion about `edition.height`, so the "1"
in the step text is unchecked — contrast `edition_two_verifies`
(`cucumber.rs:12724`), which does assert its ordinal. And it calls `verify()` on
the live `Bundle` value the `When` constructed, over the same in-process
`MemStore`; nothing is serialised out and reopened.

The normative sentence the Rule title leans on is `spec/02-content-tree.md`
§2.12, *Keyless façade (G-D)*, quoted verbatim to its end:

> **Keyless façade (G-D).** Bundle is the only public assembly boundary: it
> decodes and validates layout, version, hashes, references, reachability, and
> proof shape, then passes typed public artifacts to Core's pure semantic
> verifier. Append-time and cold-time feed the same facts to that verifier and
> obtain the same verdict. Exporting an edition into a fresh `MemStore` or
> `FsStore` and reopening it without owner or grantee private capabilities MUST
> be sufficient to verify owner and delegated history.

The scenario exports nothing and reopens nothing, so the `MUST` is untested
here. I record that `Bundle::verify` (`bundle.rs:1691-1795`) takes no key
argument and reads only `self.store` and `self.did`, so I have no reason to
believe the property is *violated* — the finding is that the step whose text is
the only occurrence of "offline" in the feature does not demonstrate it.
INVENTORY § 4.10 flagged the same textual asymmetry ("The offline qualifier is
asserted in one of four scenarios"); I confirm it against the code.

**Evidence.** Reading and the spec quotation above. No mutant required for the
ordinal half. For the offline half, note that the repository already has the
posture: `core_owner_reopens` (`cucumber.rs:11546-11556`) backs RU-5's
`the resulting edition reopens and verifies from a fresh local store`
(`d-bundle.feature:70`). So closure costs a helper, not a design.

**Closure criterion.** `edition_verifies` — or a successor step for `:13` only,
since the attribute is shared with RU-3's `:51` and RU-3's claim is different —
asserts `latest_manifest().edition.height == 1`, and additionally rebuilds a
fresh `MemStore` from the store's key/value pairs, wraps it in a new `Bundle`,
and calls `verify()` on that. Note for the implementer: **do not simply add
assertions to `edition_verifies`**, because `cucumber.rs:12698` also binds
RU-3's `its integrity checks against the signed edition`, where a `height == 1`
assertion would be wrong.

---

### `DBND-106` — P3 — "Every publication" and the Rule's linearity are demonstrated on one publication and no fork

**Statement.** Scenario `:16` performs exactly one publication and asserts one
link. The Feature description (`d-bundle.feature:4-5`) says "Editions form a
linear, hash-pinned chain", and the Rule title says "Editions chain"; no
scenario in RU-1 constructs a second successor to the same edition.
`spec/02-content-tree.md` §2.6 is the normative ground, quoted verbatim
including its conditional clauses:

> Editions form a **linear chain**: height strictly increases, each pins its
> predecessor.
>
> Without a server, two authors could sign competing height-N editions.
> Resolution rules, enforceable by any verifier:
>
> - An edition is valid only if it extends the longest chain the verifier has
>   seen and its `prev_hash` matches.

`Bundle::verify` enforces the second half of that bullet — `prev_hash` matches
(`bundle.rs:1726-1730`) and heights are contiguous (`bundle.rs:1721-1723`,
`if m.edition.height != h`) — but "extends the longest chain the verifier has
seen" is a statement about a verifier holding *more than one* candidate, and
nothing in RU-1 puts a verifier in that position. This is a scope observation,
not a claim that the property is unimplemented elsewhere: §2.6's merge and fork
machinery is exercised by other units, and I did not audit it.

**Evidence.** Reading and the spec quotation. INVENTORY § 4.2 and § 4.9 raised
both halves from the text alone; I confirm them against the code and add the
`bundle.rs` line for what `verify()` actually enforces.

**Closure criterion.** Scenario `:16` publishes a **second** edition and asserts
height 3 pinning edition 2, so that "every" has at least two instances and the
assertion is not a fixture constant; and either RU-1 gains a scenario that
constructs two competing height-N editions and asserts the verifier refuses to
treat either as canonical, or the Rule title drops the universal reading and the
feature names the Rule that carries linearity.

---

## 6. Recorded follow-ups that fall in this unit

`features/.agents/d-bundle/STATE.md` quotes seven `QUEUE.yaml` follow-ups. **Two
of them land on RU-1's surfaces.** I cite them; I do not re-open them and I did
not count them among my findings.

1. **`chdr-i3-d-bundle`** (`STATE.md:117`), quoted there verbatim as
   *"an edition pinning an I3-violating header must be refused by verify, and
   publish carries no such guard (CHDR-034, CHDR-030)"*. Both named surfaces are
   executed by this unit: `Bundle::publish` (`bundle.rs:1678`) by step `:19`,
   and `Bundle::verify` (`bundle.rs:1691`) — including its I3 tier
   `verify_pinned_headers` (`bundle.rs:302-321`), called at `bundle.rs:1759` —
   by all four `Then`s. **No RU-1 scenario asserts anything about the I3 owner
   line.** The debt is correctly recorded as owed by this cycle; nothing in RU-1
   discharges it and nothing in RU-1 contradicts it. Related surface note: this
   unit's two mutation steps (`alter_pinned_file`, `wrong_predecessor`) write
   through the `pub store` field (`bundle.rs:284`), the same injection path
   `docs/audits/features/c-headers.md` §6bis records for
   `c3_owner_line_edition.rs:239-246`.

2. **`bder-006-d-bundle`** (`STATE.md:159`). Its accepted round-2 narrowing
   names three co-owned steps, *"`rename_the_folder`, `publish_edition` and
   `reads_at_new_path` — all three are `d-bundle` steps (`cucumber.rs:8394`,
   `:8343`, `:12748`)"*. **`publish_edition` (`cucumber.rs:8343`) is RU-1's step
   `:19`**, sharing its attribute with RU-2's `the edition is republished`
   (`:8344` / `d-bundle.feature:42`). I record the co-ownership as the round-1
   impact report §9.5 asks. The re-arbitration `STATE.md:180-190` describes is
   not mine to make and I made none. The same entry notes the feature has no
   tag-view scenario; RU-1 has none either — step `:18` sets the tag `"toto"`
   (`cucumber.rs:7387`, `tags: &[tag.to_owned()]`) and no RU-1 step reads it
   back, which matches INVENTORY § 4.10's observation.

`chdr-028`, `chdr-016-grant-path`, `b-derivation-round-2-targeted`,
`chdr-i3-targeted` and `chdr-lota-vector-generators` do not fall in this unit:
their surfaces are `publication.rs` / `sdk.rs` (`chdr-028`), `grants.rs`
(`chdr-016-grant-path`), and `vectors/` (`chdr-lota-vector-generators` —
**this unit touches no vector**, so its condition is not triggered here; search:
no RU-1 step body references `vectors/` or any `*.json` fixture outside the
bundle store).

`docs/audits/features/a-identity.md`, `b-derivation.md` and `c-headers.md` were
read for what they name; the only one naming an RU-1 surface is `c-headers.md`
§6bis, through `CHDR-034` / `CHDR-030` / the `pub store` note, all covered in
item 1.

`features/AGENTS.md` § *Project stage* was honoured: nothing above is softened
for backward compatibility, no migration path is proposed, and the closure
criteria change fixtures and assertions freely. I also checked the section's
own expiry condition — no edition has been published (`CHANGELOG.md`, the crate
versions, and the absence of any published-edition marker), so the section has
not expired and there is nothing to report about it.

---

## 7. Commands I ask the orchestrator to run

I ran none of these. Each is a patch to apply, a run, and a revert. The run
command is the same throughout; if the cucumber CLI in this workspace does not
accept `--name`, drop the filter and read the four scenario results out of the
full run.

**R** (the run):

```
cd rust && cargo test -p aithos-bundle --test cucumber -- --name 'Initialising a bundle publishes a verifiable first edition|Every publication extends the chain|A tampered file fails the edition|A broken chain fails closed'
```

Fallback if `--name` is rejected: `cd rust && cargo test -p aithos-bundle --test cucumber`.

**E1 — baseline.** R, unmutated. Expected: four scenarios pass. Needed as the
reference for every mutant below.

**M1 — neuter the chain link** (for `DBND-101`). File
`rust/crates/aithos-bundle/src/bundle.rs`, lines 1726-1730.

before:
```rust
                Some(p) => {
                    if m.edition.prev_hash != p.chain_hash()? {
                        return Err(err(format!("broken chain at height {h}")));
                    }
                }
```
after:
```rust
                Some(p) => {
                    let _ = p.chain_hash()?;
                }
```
Run R. **My prediction: `A broken chain fails closed` PASSES.** (The other three
should also pass; if `Every publication extends the chain` fails, tell me — that
would mean the stray-confound analysis is wrong somewhere and `DBND-101` needs
rework.)

**M2 — delete the flat-pin loop** (for `DBND-102`). Same file, lines 1749-1755.

before:
```rust
        // Pinned files of the latest edition.
        for (path, sha) in &latest.files {
            let bytes = self.get(path)?;
            if &sha256_hex(&bytes) != sha {
                return Err(err(format!("pinned file altered: {path}")));
            }
        }
```
after:
```rust
        // Pinned files of the latest edition.
```
Run R. **My prediction: `A tampered file fails the edition` PASSES.**

**M3 — verify rejects unconditionally** (for `DBND-103`). Same file, insert
immediately after line 1692 (`let err = |m: String| …;`):

```rust
        return Err(err("M3 probe".into()));
```
Run R. **My prediction: `:22` and `:27` PASS; `:10` and `:16` FAIL.**

**M4a — drop the DID pin** (for `DBND-104`). Same file, line 1619.

before:
```rust
            if path == "manifest.json" || path == format!("manifests/{exclude_latest}.json") {
```
after:
```rust
            if path == "manifest.json" || path == "did.json" || path == format!("manifests/{exclude_latest}.json") {
```
Run R. **My prediction: `:10` FAILS — but at step `:13` (`edition 1 verifies
offline`, panic message `edition valid`), not at step `:14`.** Please report the
panic message and which step number the harness attributes it to; that
attribution is the finding.

**M4b — poison the DID pin.** Same file, after line 1623 (`}` closing the loop),
insert before `Ok(files)`:

```rust
        if let Some(v) = files.get_mut("did.json") { *v = "00".repeat(32); }
```
Run R. **Same prediction as M4a**, and for the same reason; M4b is the cleaner
of the two because it creates no stray.

**M5 — stop pinning the predecessor** (control mutant, for § 2 — I expect this
one to *kill* scenario `:16`, and I want that on record). Same file, lines
1658-1662.

before:
```rust
        let prev_hash = if height == 1 {
            String::new()
        } else {
            let prev: Manifest = self.get_json(&format!("manifests/{}.json", height - 1))?;
            prev.chain_hash()?
        };
```
after:
```rust
        let prev_hash = String::new();
```
Run R. **My prediction: `:16` FAILS, `:10` PASSES.** If `:16` passes, that is a
P1 I have not found and I want to know immediately.

**E6 — the `DBND-102` closure demonstration** (fixture change, not a mutant).
File `rust/crates/aithos-bundle/tests/cucumber.rs`, lines 8349-8355; replace the
body of `alter_pinned_file` so it tampers a sealed blob instead of the index —
list the store, take the first key matching `e/circle/blobs/` and ending `.enc`,
flip its last byte, put it back. Then run R **twice**: once unmutated (expect
`:22` still passes) and once under M2 (expect `:22` now **fails**). That pair is
what turns `DBND-102` from an argument into a measurement. If the orchestrator
would rather not author a patch to the test file, tell me and I will write the
exact replacement text.

Order I would run them in, cheapest signal first: E1, M1, M2, M5, M3, M4b, E6.
M4a is optional if M4b runs.

---

## 8. What I attacked and could not break

- **`edition_two_verifies` is not a round-trip on its own `When`.** My first
  suspicion about `:20` was `open(seal(x)) == x` in disguise: the `When`
  publishes, the `Then` checks the publication. It is not.
  `first.chain_hash()` (`manifest.rs:93-95`) recomputes SHA-256 over
  `unsigned_jcs()` of `manifests/1.json`'s **stored bytes**, deserialised
  independently at `cucumber.rs:12726-12734`, and compares it with what
  `publish_at` wrote into edition 2. A `publish_at` that forgets to pin dies on
  it. **M5 is the mutant I designed expecting it to be a no-op; I now expect it
  to be caught, and I am asking for it to be run anyway.**
- **I could not make `:16`'s `height == 2` assertion vacuous.** I looked for a
  path where `Bundle::init` yields something other than height 1 (it is a
  literal, `bundle.rs:628`) and for a `publish` that could skip or repeat a
  height (`publish` reads the tip and adds one, `bundle.rs:1679-1680`;
  `verify()` independently rejects non-contiguity at `bundle.rs:1721-1723`). The
  ordinal is load-bearing and correct.
- **`a fresh identity` announces less than it says, but constructs nothing
  false.** `cucumber.rs:7696-7699` pushes only a seed and leaves key derivation
  to the `When` (`:8324`) — the "Given that leaves the arrangement inside the
  When" shape. I tried to make a finding of it and could not: the seed *is* the
  identity material, `OwnerKeys::genesis` is deterministic in it, and the seed
  `(0u8..32).collect()` is byte-identical to the one `init_bundle()` (`:7359`)
  pushes for every other scenario in the unit, so no scenario is silently
  running on a different identity than its neighbours. Noted, not filed.
- **The unit is not proxy-driven.** I expected to find one of the 19 cached
  step definitions among my six and did not; the `OnceLock` search is reproduced
  in § 0 with its scope. My steps execute their own parameters.
- **No source-text assertion in this unit.** `include_str!` appears in the
  harness but on no path reachable from my fourteen phrases.
- **No `Scenario Outline`, no `Examples`, no degenerate quantifier over a
  table.** RU-1 is four plain scenarios; the quantifier problem here is
  `DBND-106`'s "Every publication", which is a *scenario-count* weakness, not a
  fixture-cardinality one.
- **No normative case declared by a consumerless vector.** Search:
  `grep -rn "must_fail" vectors/*.json` returns hits only in
  `c3-owner-line.json` and `cb2-connector-catalog.json`; neither is loaded by
  any RU-1 step (no RU-1 step body references `vectors/`).
- **`verify()` is genuinely keyless.** I went looking for an owner key or a
  cached plaintext smuggled into the verification path so I could attack
  `DBND-105` harder. `Bundle::verify` (`bundle.rs:1691-1795`) takes `&self` and
  no key; it touches `self.store` and the DID document it reads from the store;
  it opens no blob. `DBND-105` is therefore about an unasserted property, not a
  violated one, and I have said so in the finding rather than inflating it.

---

## 9. What I could not verify, and why

- **Every behavioural prediction in this report is unconfirmed.** I ran no gate,
  no test and no `cargo` command, and no evidence id has been handed to me.
  `DBND-101`, `DBND-102`, `DBND-103` and `DBND-104` each rest on a mutant
  prediction (M1, M2, M3, M4a/M4b). If a mutant comes back opposite to my
  prediction, the corresponding finding must be withdrawn or rewritten, not
  argued. I would rather that happen than have the prediction stand as evidence.
- **The byte-10 arithmetic in § 3 is a prediction about `serde_json`'s
  pretty-printer**, not a measurement. I deliberately built `DBND-102` so it
  does not depend on it.
- **Whether the four scenarios currently pass at all.** E1 is requested for
  exactly this reason. Everything above assumes a green baseline; if the
  baseline is not green, the whole shape of the audit changes.
- **Whether the confound in `DBND-101` also affects other features' uses of
  `edition_rejected`.** The step definition (`cucumber.rs:12738`) is bound to two
  Gherkin lines, both inside RU-1 (`:25`, `:30`). But `Bundle::verify` is called
  from many other harnesses, and I confined my reading to this unit. An
  integration pass should check whether the `manifests/{h}.json`
  self-exclusion in `all_pinned_files` (`bundle.rs:1619`) creates the same
  over-determination anywhere a test advances the tip by hand.
- **Whether `verify()`'s "extends the longest chain the verifier has seen"
  obligation (§2.6) is discharged anywhere.** I read `verify()` and found the
  `prev_hash` and contiguity halves only. The merge and fork-resolution
  machinery (`verify_merge_edition`, `verify_resolution_edition`,
  `merge.rs`) plainly exists and is plainly out of RU-1's scope; I did not audit
  it and `DBND-106` claims nothing about it.
- **Anything in the other six units.** I read RU-2..RU-7's Gherkin only far
  enough to establish the three shared step attributes named in § 0
  (`:12698`, `:8344`, `:7707`) and the existence of
  `core_owner_reopens` as a precedent for `DBND-105`'s closure. Where a shared
  attribute means another auditor is looking at the same function from a
  different claim, I have said so rather than adjudicating it.
- **The `chdr-016-grant-path` question of which cycle carries it.** `STATE.md`
  says this cycle must state it in its run report. That is a cycle-level
  decision, not a Pass A unit finding, and RU-1's surfaces do not include
  `Bundle::grant`. Routed, not settled.

---

## 10. Disclosure gate

Blocking condition 9 was considered and does not apply. All six findings
describe **test-semantics gaps** — assertions that would not catch a defect —
not exploitable weaknesses in shipped behaviour, and each carries a closure
criterion that is itself the fix. Nothing in this unit is embargoed and nothing
has been withheld from this file.

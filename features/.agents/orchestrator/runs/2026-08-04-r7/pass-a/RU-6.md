# Pass A — `RU-6`

**Unit.** `Rule: A local mutation commits state and Gamma as one transaction`,
`features/d-bundle.feature:89`. Two authored blocks, 14 expanded scenarios,
108 steps.

**Material.** `/root/work/passA-d-bundle/RU-6`, a `git archive` of `d9120d7`,
no `.git`. `/root/work/aithos-core` was not opened. No gate, test or `cargo`
command was run by me. **No behavioural claim below cites an `evidence_id`,
because none has been issued to me yet** — every statement in this report is
either a quotation of text I read, or a prediction I am asking to have
falsified. § 6 lists exactly what stays unverified until the commands in § 7
are run, and no finding's severity is asserted as final before then.

**Finding family.** `DBND-6xx`, per my instructions. I could not coordinate
numbering with the other six auditors.

---

## 0. The step-to-definition map

Every one of the 14 steps of the two blocks, traced to its body. All
`file:line` in `rust/crates/aithos-bundle/tests/cucumber.rs` unless stated.

| Feature line | Step text (abbrev.) | Definition | Body reduces to |
|---|---|---|---|
| 92 / 117 | `a published "<store>" bundle snapshotted byte for byte` | `core_atomic_fixture` `:11346` | assigns 2 strings, clears 3 `Option`s. **Constructs nothing.** |
| 93 | `an injected failure at "<boundary>"` | `core_atomic_boundary` `:11355` | `core_atomic_boundary = Some(boundary)` (after a branch on `core_revocation_failure_boundary`) |
| 94 | `the owner attempts a valid mutation and publication` | `core_atomic_failure_attempt` `:11364` → `core_atomic_failure_scenario` `:1822` | the whole arrangement **and** the whole act |
| 95 | `the mutation is refused before canonical effect` | `core_atomic_refused` `:11386` | `mutation_refused && injected_once` |
| 96 | `the canonical bundle is byte-for-byte identical to the snapshot` | `core_atomic_unchanged` `:11393` | `canonical_unchanged` (RU-6 branch) |
| 97 | `re-reading or reopening the "<store>" observes the old manifest and Gamma head` | `core_atomic_old_head` `:11407` | `store == store` ∧ `reopened` ∧ `canonical_unchanged` |
| 98 | `no failed-mutation blob, index, header, wrap or Gamma entry exists …` | `core_atomic_no_failed_artifact` `:11416` | `canonical_unchanged` |
| 99 | `staging remains non-canonical and is cleaned or recoverably resolved …` | `core_atomic_staging_clean` `:11422` | `!partial_state_observed` |
| 118 | `the owner commits a valid circle edit` | `core_atomic_success_attempt` `:11373` → `core_atomic_success_scenario` `:1936` | the whole arrangement **and** the whole act |
| 119 | `one deterministic write-set advances content, roots, manifest and Gamma` | `core_atomic_complete_write_set` `:11427` | `complete_new_state` |
| 120 | `normal completion exposes the complete new state at one logical commit point` | `core_atomic_linearized` `:11432` | `!mutation_refused` ∧ `complete_new_state` |
| 121 | `a crash or lost acknowledgement at that point resolves to …` | `core_atomic_recovery` `:11439` | `reopened` |
| 122 | `no reader or reopen observes an individual file replacement or partial edition` | `core_atomic_no_partial_state` `:11444` | `!partial_state_observed` |

Producers of `CoreAtomicObservation` (`:313`):
`core_atomic_failure_mem` `:1760`, `core_atomic_failure_fs` `:1791`,
`core_atomic_success_mem` `:1863`, `core_atomic_success_fs` `:1899`, all over
the shared fixture `core_atomic_bundle` `:1699`, the fault store
`CoreAtomicFaultStore` `:1509`, the fault enum `CoreAtomicFault` `:1463`
(`parse` `:1476`, `matches_write` `:1493`) and the snapshot helper
`cb7_store_snapshot` `:1375`.

**This unit contains no proxy step.** Search performed, not inherited: the five
process-lifetime `OnceLock` verdicts are `CB4_ACCEPTANCE`,
`CB5_CATALOG_ACCEPTANCE`, `CB6_ACCEPTANCE`, `CB7_ACCEPTANCE`,
`CB10_ACCEPTANCE` (`cucumber.rs:1119-1128`); their `*_result` helpers are
`:7295-7350`; `grep -n "cb7_result" cucumber.rs` yields one call site, `:9592`,
inside `o_catalog_overlay_fixture` `:9585`, whose regex alternatives are
`o-connector-classes-vault` phrases. **No step body reached by a line of
`Rule:` at `:89` calls any `*_result` helper.** Every one of the 14 step bodies
above reads only `CoreAtomicObservation`, which is rebuilt per scenario by the
`When`. I reproduced this rather than trusting `DOMAIN.md`'s claim of it; it
holds.

**This unit contains no source-text assertion.** Search: the
`INCLUDE_STR`-backed constants asserted with `.contains(` in the Gherkin layer
are `core_capability_api_is_narrow()` `:2053-2058`, reached from
`features/d-bundle.feature:137` — that is `RU-7a`, not mine. Nothing in
`:11346-11447` or in `:1699-1940` reads a source file.
(`cb2_bundle_boundaries.rs:463-475` does assert
`BUNDLE_LIB_SOURCE.contains("fn commit_transaction(")`, but that is a focused
binary, not a step of this Rule.)

**Neither `Examples` grid is inert.** `<store>` reaches a `match` at `:1938`
and `:1834`; `<boundary>` reaches `CoreAtomicFault::parse` at `:1839`, and an
unknown value is an `Err`, not a silent pass. The rows execute different bytes
— but not as many different bytes as six boundary names suggest; see
`DBND-604`.

---

## 1. Scenario by scenario

### S10 — `Failure before the logical commit point preserves the old bundle byte for byte` (`:91`), 12 rows, 96 steps

**Claim of the name.** Any failure occurring before the commit point leaves the
bundle bit-identical to its pre-mutation state.

**What executes.** `core_atomic_failure_scenario` `:1822` validates that the
`(store, boundary)` pair exists as a row of
`vectors/cb2-bundle-boundaries.json → transaction.failure_cases`, parses the
boundary into a `CoreAtomicFault`, and dispatches to `:1760` / `:1791`. Each of
those: builds a real bundle by `Bundle::init` + one published circle section
(`core_atomic_bundle` `:1699`); snapshots it (`before`); wraps the store in
`CoreAtomicFaultStore`; reopens through `Bundle::open`; runs a real
`owner_content_operation(Zone::Circle, Create{ folder_path: "atomic/nested" })`
(`:1743-1758`); records `mutation_refused = result.is_err()` and
`injected_once = injected.get() == 1`; snapshots again (`after`); drops the
bundle; **reopens the unwrapped store, calls `Bundle::verify()`**, snapshots a
third time; and sets
`canonical_unchanged = before == after && before == reopened_snapshot`
(`:1774`, `:1809`).

**Do claim and execution meet?** For the "old state preserved" half — **yes,
and this is the strongest work in the unit.** The mutation is real, the failure
is real (an `io::Error` from a `Store` decorator, not a mocked verdict), the
comparison is a full key→bytes map equality taken three times, and the reopen
runs the product's own `verify()`. `injected_once` is a genuine guard: a
boundary whose predicate never matched would leave `injected == 0` and fail
line 95, so no row can pass by never firing.

Four defects, each qualified below rather than asserted: the four `Then` steps
at 96–99 collapse to one predicate (`DBND-602`); the snapshot's scope excludes
exactly the artifacts line 99 names (`DBND-603`); six boundary names resolve to
at most four distinct injection points (`DBND-604`); and the `Given` at 92
constructs nothing (`DBND-605`).

### S11 — `A successful local transaction publishes content and Gamma together` (`:116`), 2 rows, 12 steps

**Claim of the name.** On success, content and Gamma become visible as one
atomic step, never one before the other.

**What executes.** `core_atomic_success_scenario` `:1936` → `:1863` / `:1899`.
Same fixture, then one real
`owner_content_operation(Zone::Circle, Edit{ display_path: "projects/note" })`,
which is **not** wrapped in any fault store and **is not interrupted anywhere**.
`complete_new_state = core_atomic_write_set_is_complete(&before, &after)`
(`:1847`), which requires `before != after` and requires a changed object under
each of `e/circle/blobs/`, `e/circle/index.json`, `gamma/`, and
`manifest.json`. Then drop, reopen, `verify()`, third snapshot, and
`reopened = reopened_snapshot == after`,
`partial_state_observed = reopened_snapshot != after` (`:1894-1895`,
`:1931-1932`).

**Do claim and execution meet?** Only for the "together" half read as
*co-occurrence*, never as *atomicity*.

- Line 119 is the one genuinely valuable assertion here: it is a **positive
  control that Gamma advanced in the same operation as the content**, and it is
  the only place in the whole feature file where `gamma/` is required to have
  changed. Credit where due.
- Line 120 adds nothing to 119 and its first conjunct is a tautology:
  `assert!(!observation.mutation_refused)` (`:11434`) against a struct literal
  that hardcodes `mutation_refused: false` at `:1890` and `:1927`. Removing the
  product's entire refusal path could not make that conjunct fail.
- Line 121 asserts `reopened`; line 122 asserts `!partial_state_observed`.
  Those two fields are `x` and `!x` of the same expression
  (`reopened_snapshot == after`). **Two `Then` steps, one bit.**
- Line 121's subject — "a crash or lost acknowledgement at that point" — is
  never induced. `DBND-601`.

---

## 2. The atomicity section

The Rule says two things commit **as one**. An atomicity claim is proved only
by observing the *interrupted* state. For each scenario: the non-atomic state
it would have to distinguish, and whether any assertion does.

**Normative ground, quoted verbatim to the end of the sentence,
`spec/02-content-tree.md` § 2.12 (`:671-702`):**

> "A mutation is calculated against an immutable snapshot in an overlay,
> submitted to the pure Core verdict, reduced to a deterministic write-set, and
> only then committed."

> "Rejection or failure before that point leaves the canonical bundle
> byte-for-byte unchanged: no advanced manifest or Gamma head, partial index,
> header, wrap, blob, or orphan from the failed local mutation."

> "`MemStore` commits by atomically replacing its canonical state. `FsStore`
> prepares in recoverable staging physically outside the canonical bundle
> directory and uses a Store-local recoverable linearization mechanism."

> "Readers, reopen, and recovery observe either the complete old state or the
> complete new state, never a mixture."

> "A crash or lost acknowledgement at the linearization boundary may require
> discovering the committed outcome from the canonical manifest/head; scratch
> is cleaned or recoverably resolved."

So the specification makes **three** claims, and RU-6's two blocks are the
Gherkin for all three:

1. *Pre-commit failure ⇒ old state.* → S10.
2. *Readers, reopen, and recovery see old-or-new, never a mixture.* → line 122.
3. *A crash at the linearization boundary is resolved from the manifest/head;
   scratch is cleaned.* → lines 121 and 99.

And "Gamma" — undefined in the feature file, defined in `spec/07-gamma.md`
§ 7.1 (`:6-13`): "`gamma/<YYYY-MM>.jsonl` — one JSON entry per line, SHA-256
hash-chained, segmented by UTC month of `at` … The manifest pins every
segment's hash plus `gamma_head` (§02.7)." That settles the feature file's
eight uses: the *entry* is a line of that file, the *head* is the manifest
field `gamma_head` = "SHA-256 of the last gamma entry"
(`spec/02-content-tree.md:468`), the fourth peer of content/roots/manifest at
feature line 119 is the `gamma/` object tree, and "Gamma validation" as a
*stage* is a fiction of the `Examples` column — no such stage exists in the
code; `CoreAtomicFault::GammaValidation` (`:1499`) is defined as
`path.starts_with("gamma/")`, i.e. the first **write** to the Gamma tree, not a
validation. The single most load-bearing undefined noun in the file is
definable from the spec in one sentence, and the file does not say it.

### S10 — 12 rows

**Non-atomic state to distinguish.** A canonical bundle containing any proper
subset of {new blob, new index, new header, new wrap, new Gamma entry, advanced
manifest, advanced `gamma_head`} — the state that exists if the mutation's
writes are applied piecemeal instead of as one write-set.

**Does any assertion distinguish it?** **Yes.** `canonical_unchanged` is
`before == after && before == reopened_snapshot` over the complete
`BTreeMap<String, Vec<u8>>` returned by `store.list("")` + `store.get()`. Every
one of those seven artifacts is a key in that map, so any proper subset landing
canonically breaks the equality. This is a real interrupted-state observation
and it is the only one in the unit.

The row that matters most is `Gamma validation` (feature `:107`, `:113`). Its
fault (`:1499`) fires on the first write to `gamma/`, which in the Circle
create path is reached **after** `ensure_folder`'s `put_json("e/circle/index.json")`
(`bundle.rs:770`) and after `put_blob_v("e/circle/blobs/<sid>.enc")`
(`bundle.rs:836`). So content is already staged and the Gamma append is the
thing that fails — this row is a direct test of *"state and Gamma as one
transaction"* in the exact direction the Rule names. I want its injection path
confirmed by evidence, not by my reading; command C3 in § 7.

**But the interruption is always before the store commits.** `CoreAtomicFaultStore::commit_transaction`
(`:1570-1575`) returns `self.injection_error()` **before** delegating to
`self.inner.commit_transaction()`. So even the two rows whose boundary name
says "before state replacement" / "before commit marker or reference" stop at
the door of the commit; `Bundle::transaction` (`bundle.rs:421-436`) then calls
`rollback_transaction`. **No row of S10 ever lets the commit mechanism start
and then interrupts it.** That is faithful to the scenario's own name — but it
means claim 2 and claim 3 of the spec are untouched by S10.

### S11 — 2 rows

**Non-atomic state to distinguish.** A post-interruption state in which the
content half of the write-set is canonical and the Gamma/manifest half is not,
or the reverse — precisely what an `FsStore` whose linearization is *not* a
single pointer flip would leave, and precisely what line 121 names.

**Does any assertion distinguish it?** **No.**

The only two predicates in S11 are `complete_new_state` (lines 119, 120) and
`reopened_snapshot == after` (lines 121, 122). Both are computed after a
**successful, uninterrupted** `owner_content_operation` returned `Ok`. There is
no fault store, no drop mid-commit, no second process, no reader concurrent
with the commit. Nothing in `:1863-1934` can produce a torn state, so nothing
in `:11427-11447` can observe one.

Formally: let *T* = the state after a torn commit in which `e/circle/blobs/…`
and `e/circle/index.json` are canonical but `gamma/2026-07.jsonl` and
`manifest.json` are not. S11 never reaches *T*, because the only path to *T* is
an interruption inside `commit_transaction`, and S11 injects none. Therefore no
assertion of S11 discriminates *T* from the correct state. **Line 121 is
satisfied by any implementation whatsoever whose successful commit is durable
across a reopen, including one with no crash recovery at all.**

### Answer to the question the Rule asks

**Atomicity is proved in one direction and not the other.**

- *Failure before the commit leaves nothing behind* — proved, by 12 rows of
  real fault injection against a real byte comparison, subject to the scope
  gap in `DBND-603`.
- *The commit itself is indivisible; a crash at the linearization point
  resolves to old-or-new* — **not proved anywhere in this unit.** The word
  "transaction" in the Rule title is carried, for its whole second half, by two
  `Then` steps that assert one bit about an uninterrupted run.

---

## 3. Findings

### `DBND-601` — P2 — Line 121 asserts crash recovery and no scenario induces a crash; the vector row that defines the state has no behavioural consumer

**Statement.** `features/d-bundle.feature:121` — *"a crash or lost
acknowledgement at that point resolves to the complete old or complete new
state from the canonical manifest and Gamma head"* — resolves to
`core_atomic_recovery` (`cucumber.rs:11439-11442`), whose entire body is
`assert!(core_atomic_observation(w).reopened)`. In the only scenario that
reaches it, `reopened` is `reopened_snapshot == after` (`:1894`, `:1931`),
computed after an `owner_content_operation` that returned `Ok` and was never
interrupted. The scenario's only `When` (`:118`) is `the owner commits a valid
circle edit`; its sibling outline at `:91` has an injected-failure `Given`
(`:93`), this one has none. The assertion therefore states: *after a successful
commit, reopening yields the same bytes.* That is durability across reopen. It
is not atomicity at the linearization boundary, and it is satisfied by an
implementation with no crash recovery at all.

The four states the claim ranges over are enumerated normatively in
`vectors/cb2-bundle-boundaries.json → transaction.recovery_cases`:
`no-staging`, `prepared-not-linearized`, `linearization-reference-durable`, and
`acknowledgement-lost` (whose `internal_state` is *"new reference durable,
caller did not receive success"* and whose `scratch_resolution` is *"discover
outcome from manifest and Gamma head"* — the sentence of feature line 121, in
the vector, word for word). **The only consumer of `recovery_cases` in the
repository is `cb2_bundle_boundaries.rs:342-359`, which asserts that the array
has four entries and that two carry `visible_snapshot: "old"` and two
`"new"`.** It counts JSON. It drives the system into none of the four states.
Search: `grep -rn "recovery_cases" --include=*.rs --include=*.py .` returns
`gen-cb2-bundle-boundaries.py:269`, `:666` (producers) and
`cb2_bundle_boundaries.rs:342` (that shape check). Layer: the Gherkin harness,
the focused binaries and the vector generators; scope: whole archive.

This is a **normative case declared by a vector with no consumer**, and the
Gherkin line that would consume it asserts something else.

**Qualification, stated because it changes the severity.**
`cb7_transaction_contracts.rs:218-236` *does* exercise two of the four states
behaviourally — it stages, drops the store without committing, reopens,
recovers and asserts `old`; then stages, commits, drops, reopens, recovers and
asserts `new` (the comment reads `"recover acknowledged-or-lost commit"`). So
the property is not wholly untested in the repository. Three things keep this a
finding: (a) that binary is not run by the feature-tier gate
(`cargo test … --test cucumber -- --tags @d-bundle` runs the Gherkin binary
only); (b) it operates on raw snapshots taken from the vector via
`replace_transaction`, never on the write-set the *Bundle* produces, so it
never shows that **content and Gamma** survive together; and (c) it too
simulates the crash by dropping the store *between* operations — no test
anywhere in the archive interrupts `commit_transaction` mid-execution. Search
for (c): `grep -rn "commit_transaction" rust/crates/*/tests/*.rs
rust/crates/*/src/*.rs` returns 11 sites; the only fault-injecting one is
`cucumber.rs:1570`, which errors **before** delegating.

**Evidence.** Text read: `cucumber.rs:11439`, `:1894`, `:1931`, `:1863-1934`,
`:1570-1575`; `cb2_bundle_boundaries.rs:342-359`;
`cb7_transaction_contracts.rs:191-236`;
`vectors/cb2-bundle-boundaries.json → transaction.recovery_cases`;
`spec/02-content-tree.md:698-702`. **No `evidence_id` — commands C1, C2 and M1
in § 7 are what would establish it behaviourally.** M1 is decisive: gut
`FsStore::recover_transaction`, the only crash-recovery code in the product,
and I predict all 14 scenarios of this unit stay green.

**Closure criterion.** Either (i) `:116`'s outline gains a `Given` that injects
a failure *inside* the store's linearization — the natural home is a new
`CoreAtomicFault` variant that lets `commit_transaction` write the generation
pointer and then errors — and lines 121/122 assert that the reopened snapshot
equals `before` **or** equals `after` and never a proper mixture, with the
manifest's `gamma_head` and the `gamma/` tree read explicitly rather than
inferred from map equality; **or** (ii) line 121 is deleted from the feature
file and the claim is carried by a scenario that actually induces the state.
Closure requires a RED demonstration: mutant M1 must turn at least one scenario
of this unit red.

### `DBND-602` — P3 — Nine `Then` steps across the unit assert five distinct bits; two of them are tautologies against hardcoded struct fields

**Statement.** The unit's 108 steps reduce to a small assertion set.

In S10, lines 96, 97, 98 and 99 all reduce to `canonical_unchanged`:
`:11393` asserts it; `:11407` asserts `store == store && reopened && canonical_unchanged`;
`:11416` asserts it; `:11422` asserts `!partial_state_observed`, and
`partial_state_observed` is *defined* as `!canonical_unchanged` at `:1787` and
`:1818`. Four `Then` lines with four different English sentences, one boolean.
Within `:11407`, `assert!(observation.reopened)` is a **tautology**: `reopened`
is the literal `true` at `:1786` and `:1817`. Line 97 names "the old manifest
and Gamma head" and reads neither: nothing in the unit opens `manifest.json`,
reads the `gamma_head` field, or walks the `gamma/` chain. Map equality is
strictly stronger, so the assertion is sound — but the sentence describes an
observation the code does not make, and if the snapshot's scope is ever
narrowed the sentence will silently stop being covered.

In S11, line 120's `assert!(!observation.mutation_refused)` (`:11434`) is a
**tautology**: `mutation_refused: false` is a literal at `:1890` and `:1927`.
Its second conjunct duplicates line 119. Lines 121 and 122 assert `x` and `!x`
of the same expression.

**Evidence.** `cucumber.rs:11386-11447` (all nine bodies), `:1779-1788`,
`:1810-1819`, `:1887-1896`, `:1924-1933`.

**Closure criterion.** Either the redundant lines are removed, or each is given
a distinct observation: line 97 reads `manifest.json`'s `edition.height` and
`gamma_head` and compares them to the pre-mutation values; line 98 enumerates
the five artifact classes it names and asserts each has no new key; line 99 is
addressed by `DBND-603`; line 120 asserts a *count* (one visible transition,
matching the vector's `linearization_count: 1`, which today has no behavioural
consumer either).

### `DBND-603` — P2 — Line 99 claims no local-mutation orphan; the snapshot it is asserted against cannot see one, and a snapshot that can exists in the same file

**Statement.** `features/d-bundle.feature:99` — *"staging remains non-canonical
and is cleaned or recoverably resolved with no local-mutation orphan"* —
resolves to `core_atomic_staging_clean` (`:11422`), body
`assert!(!core_atomic_observation(w).partial_state_observed)`, i.e.
`canonical_unchanged`, i.e. the same map equality as line 96.

That map is `cb7_store_snapshot` (`:1375-1389`): `store.list("")` then
`store.get(path)` for each key. For `FsStore` both resolve through
`canonical_base()` (`lib.rs:527-540`), which returns the **generation
directory** named by the `.aithos-current` pointer. Everything the sentence is
about is therefore outside its range: the staging generations under
`.aithos-generations/` (`lib.rs:419-421`), the pointer `.aithos-current`
(`:423`), the mirror marker `.aithos-mirror-current` (`:427`), the transient
`.aithos-current.tmp-*` / `.aithos-mirror-current.tmp-*` files
(`:490-524`), and the compatibility mirror materialized under the root
(`:652-684`). A leaked staging generation — the textbook local-mutation orphan
— changes none of the bytes this assertion compares.

A helper that *would* see it is 1857 lines away in the same file:
`core_path_raw_snapshot` (`:3232-3288`) walks the raw tree, records symlink
targets and directories, and is used by `RU-7b` (`:3300`, `:3324`) — the Rule
about paths, where orphans are not the claim. The unit that claims "no orphan"
uses the blind snapshot; the unit that does not claim it uses the sighted one.

Two further sentences of § 2.12 land on this same line and are unasserted:
*"`FsStore` prepares in recoverable staging physically outside the canonical
bundle directory"* and *"Any internal generation metadata, commit marker, or
reference is outside the canonical bundle namespace, §2.3 layout, manifest,
pins, and signed wire"*. The vector states both as booleans —
`staging_outside_canonical_namespace: true`,
`internal_generation_metadata_is_not_wire: true` — and their **only** consumer
is `cb2_bundle_boundaries.rs:326-329`, `assert_eq!(transaction[…], true)`,
which compares a JSON literal to itself. Search:
`grep -rn "staging_outside_canonical_namespace" --include=*.rs --include=*.py .`
→ `gen-cb2-bundle-boundaries.py:620` and `cb2_bundle_boundaries.rs:326` only.

**Evidence.** `cucumber.rs:11422`, `:1375-1389`, `:1787`, `:1818`, `:3232-3288`;
`lib.rs:419-427`, `:490-540`, `:652-684`, `:899-905`;
`cb2_bundle_boundaries.rs:325-329`; `spec/02-content-tree.md:693-702`. Mutants
M3 and M4 in § 7 are the test: leak the staging directory, and I predict the
unit stays green.

**Closure criterion.** Line 99 asserts against a raw-tree snapshot — reuse
`core_path_raw_snapshot` (`:3232`) — taken before the mutation and after the
reopen, and asserts (a) no `.aithos-generations/` entry other than the active
one survives the reopen, and (b) no key of the raw tree that is not in the
canonical view carries any byte of the refused mutation. Mutant M3 must turn
the six `FsStore` rows of `:91` red.

### `DBND-604` — P3 — Six boundary names resolve to at most four distinct injection points; two Examples pairs execute identical bytes

**Statement.** `CoreAtomicFault::parse` (`:1476-1491`) maps **both**
`"before state replacement"` (feature `:108`, MemStore) and
`"before commit marker or reference"` (feature `:114`, FsStore) to the single
variant `Self::StateReplacement`. The `INVENTORY` observed that the grid is not
a cross-product and inferred the two stores might have different commit points;
the code says the opposite — they are one fault under two names, and the
asymmetry of the grid encodes nothing.

`matches_write` (`:1493-1505`) then collapses two more. `Self::Cryptography =>
true` matches **every** write — the comment at `:1495-1496` concedes it ("The
first candidate write is the boundary at which completed cryptographic
preparation crosses into the transactional store"), so the row named
`cryptography` injects at the first store write, not in cryptography. In the
Circle create path that first write is
`ensure_folder`'s `put_json("e/circle/index.json")` (`bundle.rs:770`) — which
is also the first path matching `Self::IndexPreparation`
(`path.ends_with("index.json")`). If that ordering holds, rows `cryptography`
and `index preparation` inject at the identical call and execute identical
bytes, in both stores: four rows, one execution.

`Self::HeaderOrWrap` is stranger. Its `matches_write` predicate is
`path.ends_with("header.json") || path.contains("/wrap")`, and the owner's
Circle `section_add` writes **neither**: headers live at `e/<zone>/header.json`
(written only by `Bundle::init`, `bundle.rs:598`) and at `e/<zone>/hdr/<digest>.json`,
wraps at `e/<zone>/wraps/<id>.json` (`grants.rs:143`, `:152`) — both written by
the *grant* path, not by `section_add`. The injection for rows `:106` and
`:112` therefore comes from the `get` override at `:1534-1541`, i.e. from a
**read** of `e/circle/header.json` during key derivation. So the boundary named
"header or wrap" interrupts no header write and no wrap write.

That matters beyond tidiness, because it is a debt this feature already owes.
`QUEUE.yaml`'s `bder-006-d-bundle`, quoted in
`features/.agents/d-bundle/STATE.md`, records the accepted round-2 impact
review's reading of exactly these lines: *"Le mot `wrap` y apparaît quatre
fois, jamais comme pontage d'ancre : `:98`, `:106`, `:112` l'énumèrent parmi
les artefacts qu'une mutation échouée ne doit pas laisser"*, and
`features/.agents/c-headers/DOMAIN.md:223-226` names the seam from the other
side as *"`d-bundle.feature` — atomicity of header and wrap writes (`:98`,
`:106`, `:112`)"*. **The debt is live and my unit is where it lands:** the two
`Examples` rows that name header-and-wrap atomicity interrupt a read, and the
`Then` at `:98` that enumerates `header, wrap` among the forbidden leftovers
asserts a map equality over a mutation that writes no header and no wrap, so
those two conjuncts are vacuously satisfied.

**Evidence.** `cucumber.rs:1476-1505`, `:1534-1541`, `:1560-1575`, `:1743-1758`;
`bundle.rs:734-773` (`ensure_folder`), `:825-860` (Circle `section_add`),
`:290-296`; `grants.rs:143`, `:152`. The ordering claim needs C3.

**Closure criterion.** Either the grid loses the rows that do not name a
distinct injection point, or the fixture mutation is changed to one that writes
a header and a wrap (a grant, or a rotation) so that `header or wrap` interrupts
what its name says, and `before state replacement` / `before commit marker or
reference` are given two distinct fault variants matching the two stores'
actually distinct mechanisms (`MemStore::commit_transaction` `lib.rs:375-381`
replaces a map; `FsStore::commit_transaction` `:869-896` flips a pointer file).

### `DBND-605` — P3 — The `Given` at `:92`/`:117` announces a published, snapshotted bundle and constructs nothing

**Statement.** `core_atomic_fixture` (`:11346-11353`) is five assignments:
`core_atomic_store = store`, `core_atomic_boundary = None`,
`core_atomic_observation = None`, `core_path_store = store`,
`core_path_observation = None`. No bundle is initialised, nothing is published,
and no snapshot is taken. The entire arrangement — `Bundle::init`, the fixture
section, the publication, and the `before` snapshot — lives inside
`core_atomic_bundle` (`:1699-1738`), called from the `When`'s scenario
functions at `:1761`, `:1796`, `:1864`, `:1900`.

The behaviour is real, so this is not a vacuity; it is a locality defect with
one concrete consequence. Because the `Given` verifies nothing, a `<store>`
value that no branch handles is not caught at `Given` time; it surfaces as an
`Err` from the `match` at `:1938` / `:1834`, which `core_atomic_observation`
(`:11378-11384`) turns into a `panic!` inside the **first `Then`**. A scenario
whose arrangement is impossible fails at line 95, reporting an assertion
failure where the truth is a missing fixture.

**Evidence.** `cucumber.rs:11346-11353`, `:1699-1738`, `:11378-11384`,
`:1834-1845`, `:1936-1942`.

**Closure criterion.** The `Given` builds the bundle and takes the snapshot,
storing both in the world; the `When` performs only the act. This also fixes
the shared-step verdict in § 4.

---

## 4. Verdict on the `Given` and `Then` shared with `RU-7`

I hold `RU-6`; I read `RU-7`'s uses as instructed.

### The shared `Given` — feature `:92`, `:117`, `:149` → `core_atomic_fixture` (`:11346`)

**Serves both, weakly, and only because it does nothing.** The body sets the
`core_atomic_*` fields for RU-6 and the `core_path_*` fields for RU-7b in the
same five lines, and clears both observations. It is a router, not an
arrangement. RU-7b's own `When` (`core_path_attempt`, `:11449`) rebuilds the
fixture from scratch via `core_path_mem_scenario` (`:3117`) or
`core_path_fs_scenario` (`:3201`), each of which calls the same
`core_atomic_bundle` (`:1699`). So the three feature lines share a sentence and
a helper but not a fixture instance.

One live hazard, recorded rather than claimed: `core_atomic_boundary` (`:11355`)
is **not** unconditional. It branches on
`w.core_revocation_failure_boundary == "__fixture__"` and, when that holds,
writes the boundary into the *revocation* field instead of
`core_atomic_boundary`. The sentinel is set at `:11920`, in
`g-revocation`'s `Given` (`a published bundle snapshotted byte for byte before
revocation`, `:11918`). If `ProtocolWorld` were ever shared across scenarios,
RU-6's `:93` would silently write into another feature's field and
`core_atomic_boundary` would stay `None`, making the `When` at `:11364` panic
on `.expect("CORE-OWN-002 injected boundary")`. Cucumber's `World` is
per-scenario, so I expect this to be inert; I did not verify the world's
lifetime and I am not asserting a defect. **A step body branching on another
feature's sentinel is a coupling no reader of the feature file can see**, and it
is worth recording for Pass B.

### The shared `Then` — feature `:96` and `:152` → `core_atomic_unchanged` (`:11393-11405`)

**It serves `RU-6`. It is vacuous in `RU-7`.** This is the discrimination loss
the brief predicted, and it is asymmetric.

The body is a dispatcher:

```rust
if let Some(observation) = &w.core_path_observation {
    assert!(observation.…canonical_unchanged);   // RU-7b
} else {
    assert!(core_atomic_observation(w).canonical_unchanged);   // RU-6
}
```

In **RU-6** the branch is load-bearing. The `When` attempted a real mutation
that stages a blob, an index, a Gamma line and a manifest; if any of that
became canonical, `before == after` fails. The assertion can fail, and mutant
M2 in § 7 is designed to make it.

In **RU-7b** it cannot fail. All ten rows perform a **read**, never a write:
`core_path_mem_scenario` (`:3128-3137`) issues
`OwnerContentOperation::Read`; `core_path_fs_scenario` (`:3302-3319`) issues
either `OwnerContentOperation::Read` or a bare `bundle.store.get(invalid_input)`.
`canonical_unchanged` is `before == after` around an operation that has no write
path at all. **No defect in path confinement — none — can make feature line 152
red.** It is a vacuous positive, and it is vacuous *because* the sentence was
written for a Rule about mutation and reused by a Rule about reads.

RU-7b's real assertion lives in its other `Then`, `:151` →
`core_path_refused_before_access` (`:11467-11480`), which checks `rejected` and
`!outside_access_observed`, and `outside_access_observed` (`:3325-3330`) is a
genuine escape detector — it compares a raw snapshot of the *outside* directory
and checks whether the returned bytes equal the planted escape payload. That
step discriminates. The shared one does not.

**So: the shared body serves one of the two claims.** For RU-6 it is the
unit's principal assertion. For RU-7 it is decoration. The auditor holding
RU-7 alone would see `And the canonical bundle is byte-for-byte identical to
the snapshot` at `:152`, trace it to a body that says `canonical_unchanged`,
and have no way to see that in their Rule the field is computed around a read.
That is the whole reason the two uses had to be read together.

**Recommendation, offered not asserted.** Feature line 152 should either be
deleted from `:148`'s outline — RU-7b's claim is confinement, and `:151`
carries it — or RU-7b should gain a row whose `When` is a *write* through an
untrusted key, at which point `:152` becomes load-bearing there too. I flag
this for the RU-7 auditor and for Pass B; it is their finding to number, not
mine.

---

## 5. What I attacked and could not break

Recorded so the effort is visible and so a later pass does not re-spend it.

- **I looked for a proxy step and found none.** All five `OnceLock` verdicts
  and their `*_result` helpers were located (`:1119-1128`, `:7295-7350`) and
  every call site enumerated; the only `cb7_result` call is `:9592`, inside an
  `o-connector-classes-vault` fixture. Every one of my 14 step bodies reads
  only `CoreAtomicObservation`, which the `When` rebuilds. `DOMAIN.md` asserts
  this; I reproduced it and it holds.
- **I looked for a source-text assertion in this unit and found none.** The
  one in the feature's Gherkin layer, `core_capability_api_is_narrow()`
  (`:2053-2058`), is reached from `:137` — RU-7a.
- **I looked for an inert `Examples` grid and found none.** Both `<store>` and
  `<boundary>` reach `match`/`parse` that `Err` on an unknown value.
  `DBND-604` is that some values coincide, not that none is read.
- **I tried to make S10 vacuous and could not.** The obvious attack is "the
  fault never fires, so nothing was ever written, so `before == after`
  trivially". `injected_once` (`:1773`, `:1800`, asserted at `:11390`) closes
  it: `injected` can only be incremented by `injection_error()` (`:1526`), so
  `injected == 1` means the boundary genuinely fired. And at least one write
  (`ensure_folder`'s index put, `bundle.rs:770`) lands in the overlay before
  most boundaries fire, so the rollback has something to discard.
- **I tried to make S10's byte comparison shallow and could not, within its
  scope.** `cb7_store_snapshot` reads every key `list("")` returns and every
  byte of each. Inside the canonical namespace it is genuinely byte-for-byte.
  `DBND-603` is about the namespace's edge, not about the comparison.
- **I tried to find a `Then` that round-trips its own `When` and found only a
  partial one.** Line 95's `mutation_refused` does restate the `When`'s
  failure, but its second conjunct (`injected_once`) is independent, and lines
  96–99 assert about state the `When` did not report.
- **I tried to find a vacuous negative and found the opposite.** Line 119 is a
  real positive control — `core_atomic_write_set_is_complete` (`:1847-1861`)
  requires four independent object classes, including `gamma/`, to have
  changed. Without it the unit would be all-negative; with it, S10's negatives
  are anchored. This is the best-constructed step in the unit and I say so.
- **I checked whether S10's reopen is cosmetic. It is not.** Both failure
  producers call the product's own `Bundle::verify()` (`:1770`, `:1805`) before
  taking the third snapshot, so a bundle left internally inconsistent by the
  rollback would fail there, not silently pass.

---

## 6. What I could not verify, and why

- **Everything behavioural.** No `evidence_id` has been issued to me. Every
  claim above is a reading of text. In particular I have **not** observed that
  the 14 scenarios pass today, and my four mutant predictions are predictions.
- **The write order inside the Circle create path**, and therefore whether
  `cryptography` and `index preparation` inject at the identical call
  (`DBND-604`). I read `ensure_folder` (`bundle.rs:734-773`) and `section_add`
  (`:778-895`) and believe the first write is
  `put_json("e/circle/index.json")`, but `Bundle::open` on the wrapped store
  and `owner_current_section_key` may interleave reads I did not trace.
  Command C3 settles it.
- **Whether `header or wrap` injects on the read I think it does.** I inferred
  it from the absence of any matching write in the owner path plus the
  existence of the `get` override at `:1534`. C3 settles it.
- **The lifetime of `ProtocolWorld`.** I did not establish whether cucumber
  reconstructs it per scenario, so the `core_revocation_failure_boundary`
  sentinel coupling in § 4 is recorded as a hazard, not a finding.
- **Whether the RU-6 gate is affected by the `#[cfg(not(unix))]` fallback.**
  `core_path_fs_scenario` exists twice (`:3201` unix, `:3340` non-unix, the
  latter always `Err`). That is RU-7b's problem, but it shares my `Given`; on a
  non-unix runner RU-7b's ten rows would panic while RU-6's fourteen would
  pass. I did not determine the runner's platform.
- **`docs/audits/features/*.md`.** `features/.agents/d-bundle/STATE.md:338-351`
  forbids opening `docs/audits/` before Pass A is frozen and states that the
  summaries quoted in `STATE.md` and `DOMAIN.md` "are the only form in which a
  Pass A unit may see them". I obeyed that: I ran a filename-and-line search
  (`grep -rn "d-bundle" docs/audits/features/*.md`) to establish **whether**
  any published audit names my unit, and quoted only the passages `STATE.md`
  and `DOMAIN.md` themselves reproduce. **One does name my unit** — the
  `bder-006-d-bundle` debt and the `c-headers` domain note, both cited in
  `DBND-604`, which name feature lines `:98`, `:106`, `:112` exactly. No other
  published audit names anything in `RU-6`; `a-identity.md` and the remaining
  `b-derivation.md` / `c-headers.md` references point at `:38-41`, `:138`,
  `:146` and the `chdr-028` publication surfaces, all outside this unit.
- **Whether the two blocks' 108 steps are all executed.** The harness runs
  `filter_run_and_exit` with `fail_on_skipped()` (`cucumber.rs:20017-20039`)
  and no `@wip` tag exists on this Rule, so I expect all 14 scenarios to run;
  C1's counters would confirm it.

---

## 7. Commands I want run

Named exactly, in the form `DOMAIN.md` § *Gate pyramid* prescribes, from the
repository root. I run none of them.

**C0 — mandatory static check, before anything else.**

```text
features/.agents/scripts/verify-feature-tags.sh
```

**C1 — feature tier, the baseline for every mutant below.**

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

**C2 — the focused binary that holds the recovery coverage `DBND-601` says the
Gherkin lacks.** One binary, to resolve one semantic contradiction: whether
`recovery_cases` is exercised anywhere.

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb7_transaction_contracts
```

**C3 — instrumentation, to settle `DBND-604` and the two "could not verify"
items.** Not a mutant; a probe. Apply, run C1, read stderr, revert.

```diff
--- a/rust/crates/aithos-bundle/tests/cucumber.rs
+++ b/rust/crates/aithos-bundle/tests/cucumber.rs
@@ -1524,6 +1524,7 @@ impl<S> CoreAtomicFaultStore<S> {
     fn injection_error<T>(&self) -> io::Result<T> {
         self.injected.set(self.injected.get() + 1);
+        eprintln!("CORE-OWN-002 PROBE fault={:?}", self.fault);
         Err(io::Error::other(format!(
             "CORE-OWN-002 injected {:?} failure",
             self.fault
```

plus, at the two injection sites, the path:

```diff
@@ -1533,6 +1534,7 @@ impl<S: Store> Store for CoreAtomicFaultStore<S> {
     fn get(&self, path: &str) -> io::Result<Option<Vec<u8>>> {
         if self.injected.get() == 0
             && self.fault == CoreAtomicFault::HeaderOrWrap
             && (path.ends_with("header.json") || path.contains("/wrap"))
         {
+            eprintln!("CORE-OWN-002 PROBE site=get path={path}");
             return self.injection_error();
         }
@@ -1552,6 +1554,7 @@ impl<S: Store> Store for CoreAtomicFaultStore<S> {
     fn put(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
         if self.injected.get() == 0 && self.fault.matches_write(path) {
+            eprintln!("CORE-OWN-002 PROBE site=put path={path}");
             return self.injection_error();
         }
```

Run with `-- --tags @d-bundle --nocapture`. Twelve `PROBE` lines are expected,
one per row of `:91`. **What I predict:** `cryptography` and `index preparation`
print the same `path=e/circle/index.json` with `site=put`; `header or wrap`
prints `site=get path=e/circle/header.json`; `Gamma validation` prints
`site=put path=gamma/2026-07.jsonl`; `before state replacement` and
`before commit marker or reference` print no `site=` line at all (they fire in
`commit_transaction`, which has no probe) and identical `fault=StateReplacement`.

**M1 — the decisive mutant for `DBND-601`.** Gut the product's only
crash-recovery path. Run C1 with and without.

```diff
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -906,67 +906,4 @@ impl Store for FsStore {
     fn recover_transaction(&mut self) -> io::Result<()> {
-        self.rollback_transaction()?;
-        Self::ensure_plain_directory(&self.root)?;
-        let active = self.read_pointer()?;
-        …                      // the whole body, lib.rs:907-970
-        Ok(())
+        self.transaction = None;
+        Ok(())
     }
```

(Replace the entire body of `FsStore::recover_transaction` —
`lib.rs:906-972`, from `self.rollback_transaction()?;` at `:907` through the
`Ok(())` at `:971` — with the two lines shown. `MemStore::recover_transaction`
at `lib.rs:388-391` is left alone, so only the `FsStore` rows are affected.)

**Prediction: all 14 scenarios of `RU-6` stay green.** If that holds, feature
line 121 — the only line in the whole file that mentions crash recovery — is
green against a store with no crash recovery, and `DBND-601` is established at
P2 without further argument. If instead a scenario goes red, `DBND-601`
downgrades and I will say so.

**M2 — the positive control for `S10`, and the true non-atomicity mutant.**
Make `MemStore`'s rollback commit what it should discard. This is exactly
"state and Gamma were not one transaction".

```diff
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -383,6 +383,8 @@ impl Store for MemStore {
     fn rollback_transaction(&mut self) -> io::Result<()> {
-        self.overlay = None;
+        if let Some(overlay) = self.overlay.take() {
+            self.objects = overlay;
+        }
         Ok(())
     }
```

(`lib.rs:383-386`. Leave `MemStore::recover_transaction` at `:388-391`
untouched, so the mutant is confined to the rollback path S10 exercises.)

**Prediction: the six `MemStore` rows of `:91` go RED at feature line 96.**
This must hold. If any stays green, `DBND-602` escalates from P3 to P1 and the
unit proves nothing at all. **Run this one first** — it is the control that
licenses every positive statement I made in § 5.

**M3 — the mutant for `DBND-603`.** Leak the staging generation instead of
removing it.

```diff
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -899,6 +899,4 @@ impl Store for FsStore {
     fn rollback_transaction(&mut self) -> io::Result<()> {
-        if let Some(transaction) = self.transaction.take() {
-            Self::remove_internal_path(&transaction.staging)?;
-        }
+        self.transaction = None;
         Ok(())
     }
```

(`lib.rs:899-904`.)

**Prediction: `RU-6` stays green** — the leaked generation is outside
`canonical_base()`, so `cb7_store_snapshot` cannot see it. Note that the reopen
inside the scenario would still sweep it via `recover_transaction`, so a
sceptic can say recovery cleaned up. **M4 removes that defence: apply M1 and M3
together.** Under M1+M3 the staging directory leaks permanently and nothing
cleans it, and I predict `RU-6` is *still* green. That pair is the closure test
for `DBND-603`.

**M5 — the mutant for line 120's "one logical commit point".** Split the
linearization so the compatibility mirror advances before the canonical
pointer, creating a window in which a reader of the root sees new content while
the canonical view still resolves to the old generation.

```diff
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -886,10 +886,10 @@ impl Store for FsStore {
-        self.write_generation_marker(&pointer, ".aithos-current.tmp", &transaction.generation)?;
         let active = transaction.staging.clone();
         let generation = transaction.generation.clone();
         self.transaction = None;
         self.materialize_compatibility_mirror(&active)?;
+        self.write_generation_marker(&pointer, ".aithos-current.tmp", &generation)?;
         self.write_generation_marker(
             &self.mirror_marker_path(),
             ".aithos-mirror-current.tmp",
```

(`lib.rs:886-890`: move the pointer write from `:886` to just after
`materialize_compatibility_mirror` at `:890`, rebinding it to the already-cloned
`generation` so the borrow of `transaction` ends before `:889`.)

**Prediction: `RU-6` stays green.** No scenario observes the store between
those two writes, so the claim "at one logical commit point" survives having
two.

**M6 — the positive control for line 119, so I am not crediting it on faith.**
Suppress the Gamma append on the owner mutation path.

```diff
--- a/rust/crates/aithos-bundle/src/log.rs
+++ b/rust/crates/aithos-bundle/src/log.rs
@@ -198,6 +198,7 @@ impl<S: Store> Bundle<S> {
         ent: &mut dyn EntropySource,
     ) -> Result<()> {
+        return Ok(());
         let prev = self.gamma_head()?;
```

(`log.rs:190-198` is the signature; insert the early return immediately after
the `) -> Result<()> {` at `:198`. Expect an `unreachable_code` warning; if the
build denies warnings, use `if true { return Ok(()); }` instead.)

**Prediction: the two rows of `:116` go RED at feature line 119** (the
`gamma/`-changed conjunct of `core_atomic_write_set_is_complete`), confirming
that line 119 genuinely requires Gamma to advance with the content. This will
also redden `RU-5` and other features; run it under `--tags @d-bundle` and read
which scenarios fail, not just the exit code.

---

## 8. Disclosure gate

**No finding in this report meets blocking condition 9.** None of
`DBND-601`…`DBND-605` states an exploitable weakness: they are gaps between a
Gherkin sentence and the assertion behind it, in a repository at alpha stage
with nothing deployed and no edition published (`features/AGENTS.md`,
§ *Project stage*). The full statements are therefore written here in this
tracked file, as they should be. Nothing is held back into my final message.

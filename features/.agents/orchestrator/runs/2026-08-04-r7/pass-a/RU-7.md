# Pass A — `RU-7` — Rule: *Local capabilities and paths stay narrow*

Unit: `features/d-bundle.feature:129`. Two authored blocks, **14 expanded
scenarios, 72 steps** (8 step lines × 4 rows = 32; 4 step lines × 10 rows = 40).
Finding family for this auditor: **`DBND-7xx`** (I could not coordinate
numbering with the other six Pass A auditors; the 7xx block is mine by the
assignment, and a collision with another auditor's number is possible only if
they ignored the same instruction).

Material: `/root/work/passA-d-bundle/RU-7`, a `git archive` of `d9120d7` with no
`.git`. I did not open `/root/work/aithos-core`. I ran no gate, no test and no
`cargo` command. **Every claim below about *what the code says* is a reading of
the archive; every claim about *what a run does* is a prediction I am asking to
have executed** — the requested commands are in § 8. No claim in this report
cites an `evidence_id`, because I was given none; § 8 lists what I need.

Step definitions for this feature live in one file:
`/root/work/passA-d-bundle/RU-7/rust/crates/aithos-bundle/tests/cucumber.rs`
(20 040 lines). All `cucumber.rs:N` references below are to that path.

---

## 1. Parameter-reachability table

Every step line of both outlines, its definition, whether the definition binds
the `Examples` parameters, and whether it *uses* them.

### 1a. `Scenario Outline` at `:131` — 4 rows, 8 step lines

| Feature line | Step text | Definition | Binds params? | Uses them? |
|---|---|---|---|---|
| `:132` | `Given one Ethos-and-actor session backed by a purpose-bound opaque "<capability>" capability` | `d_narrow_capability`, `cucumber.rs:8428-8434` | yes — `capability: String` | **stored only.** `w.core_capability = capability` (`:8430`); read later by `:8442` and `:8444`. Constructs no session, no key, no capability object. |
| `:133` | `When Bundle submits the typed "<protocol_object>" that needs "<capability>"` | `d_typed_capability_operation`, `:8436-8448` | yes — both | **yes.** `assert_eq!(capability, w.core_capability)` (`:8442`) is a feature-file self-consistency check; `core_capability_scenario(&w.core_capability, &w.core_capability_object)` (`:8444`) dispatches on the pair at `:3104-3115` to four distinct functions. |
| `:134` | `Then "<observable_result>"` | `d_capability_result`, `#[then(expr = "{string}")]`, `:8450-8462` | yes — `observable: String` | **yes, but only as a string.** `assert_eq!(observation.observable_result, observable)` (`:8460`) compares the `Examples` cell to a literal the harness itself wrote (`:2101`, `:3016`, `:3044`, `:3095`). See `DBND-701`. |
| `:135` | `And using that capability for "<mismatched_object>" is refused` | `d_mismatched_capability_refused`, `:8464-8475` | yes — `object: String` | **NO.** `w.core_capability_mismatch = object` (`:8466`); the field is declared at `:541`, cleared at `:8432`, written at `:8466`, and **read nowhere** (exhaustive search of `cucumber.rs` for `core_capability_mismatch` returns exactly those three lines). The asserted boolean was computed by the `When` without ever seeing this column. See `DBND-702`. |
| `:136` | `And arbitrary bytes or a mismatched Ethos, actor, purpose, node, version or recipient are refused` | `d_capability_boundary_holds`, one `regex =` alternation at `:8477-8490` | no parameters | n/a — same three assertions for all four rows. |
| `:137` | `And a capability for another protocol artifact class cannot substitute` | same, `:8477-8490` | no parameters | n/a |
| `:138` | `And no universal sign, open or wrap capability is exposed` | same, `:8477-8490` | no parameters | n/a |
| `:139` | `And no seed or private key is accepted or returned by the bundle operation` | same, `:8477-8490` | no parameters | n/a |

`:136`–`:139` are four Gherkin sentences behind **one** step function whose body
is three `assert!`s (`:8487-8489`), identical for all four rows and identical for
all four sentences. No sentence has an assertion of its own.

### 1b. `Scenario Outline` at `:148` — 10 rows, 4 step lines

| Feature line | Step text | Definition | Binds params? | Uses them? |
|---|---|---|---|---|
| `:149` | `Given a published "<store>" bundle snapshotted byte for byte` | `core_atomic_fixture`, `:11346-11353` | yes — `store: String` | **stored only.** Four world-field assignments; no bundle, no publication, no snapshot. Shared verbatim with `RU-6` (`:92`, `:117`). See `DBND-710`. |
| `:150` | `When a caller supplies "<invalid_input>" as a "<input_kind>" under "<filesystem_condition>"` | `core_path_attempt`, `:11449-11465` | yes — all three | **yes.** All three reach `core_path_scenario` (`:3348-3359`) → `core_path_mem_scenario` (`:3117`) or `core_path_fs_scenario` (`:3202`), where `input_kind` selects a branch (`:3121`, `:3302`), `filesystem_condition` selects a `match` arm (`:3232-3298`), and `invalid_input` is the value passed to the operation. |
| `:151` | `Then the operation is rejected before any out-of-root store access` | `core_path_refused_before_access`, `:11467-11480` | no parameters (reads world) | reads `w.core_path_store` / `_input_kind` / `_invalid_input` back for echo-equality (`:11475-11477`); the load-bearing assertions are `observation.rejected` and `!observation.outside_access_observed`. |
| `:152` | `And the canonical bundle is byte-for-byte identical to the snapshot` | `core_atomic_unchanged`, `:11393-11405` | no parameters | branches on which world field the `When` filled. Shared verbatim with `RU-6` (`:96`). See `DBND-711`. |

### 1c. Verdict on the primed failure mode

**The failure mode this unit was built for is not present here.** The 19
`OnceLock`-cached shared verdicts (`chdr-lota-proxy-verdicts`) do not reach this
Rule; `d-bundle` is not among the nine features that queue entry lists, and
`d-bundle/DOMAIN.md:445-450` records the same search performed against the code.
I reproduced it: neither `core_capability_scenario` nor `core_path_scenario`
touches a `static`; each call builds its fixture from scratch
(`core_atomic_bundle`, `:1699`; `Cb7TempRoot::new`, `:1425`).

**All 14 rows execute distinct bytes.** RU-7a's four rows dispatch to four
different functions (`:3109-3112`). RU-7b's ten rows take six different `match`
arms plus four distinct `invalid_input` strings through one MemStore branch.

But *executing distinct bytes* and *producing distinct verdicts* are different
things, and three columns fail the second test:

- **`mismatched_object` (`:143-146`) reaches no executing code at all** — the
  one genuinely dead column. Mutant M2 below.
- **`observable_result` (`:143-146`) reaches code but not behaviour** — it is
  compared to a harness constant, never evaluated as a proposition.
- **`filesystem_condition` for row `:163` reaches a fixture that production never
  sees**, because the key is rejected on grammar first. Mutant M5 below.

---

## 2. Source-text-assertion audit

Every assertion executed by this unit, classified.

| # | Assertion (`cucumber.rs`) | Backs feature line | Class |
|---|---|---|---|
| A1 | `assert_eq!(capability, w.core_capability)` `:8442` | `:133` | **feature-file self-consistency.** Compares two `Examples` cells of the same row. No production code observed. |
| A2 | `assert_eq!(observation.capability, w.core_capability)` `:8458` | `:134` | **harness-constant echo.** `observation.capability` is a literal written at `:2099`, `:3014`, `:3042`, `:3093`. |
| A3 | `assert_eq!(observation.protocol_object, w.core_capability_object)` `:8459` | `:134` | **harness-constant echo** (`:2100`, `:3015`, `:3043`, `:3094`). |
| A4 | `assert_eq!(observation.observable_result, observable)` `:8460` | `:134` | **harness-constant echo** (`:2101`, `:3016`, `:3044`, `:3095`). Not behavioural. |
| A5 | `assert!(observation.operation_succeeded)` `:8461` | `:134` | **behavioural**, and the only behavioural conjunct of `:134`. Its computation differs per row: `:2091-2094` (JCS-canonical candidate equals the pinned vector), `:3011` (`gamma::verify_owner_entry` against `did.json`), `:3045` (`opened == "before atomic mutation"`), `:3084-3086` (`header.open_latest` yields the original DK). |
| A6 | `assert!(…mismatched_object_refused)` `:8467-8474` | `:135` | **behavioural but mis-aimed.** Rows `:143`/`:144`: aliased to `mismatched_session_refused` (`:2103`, `:3018`). Row `:145`: genuine — `read_owner_section(…, "projects/sibling")` (`:3036`) hits the node-path binding at `session.rs:275-282`. Row `:146`: `header.open_latest(…, &wrong_secret).is_err()` (`:3088-3090`) — a decryption failure that never touches the capability. |
| A7 | `assert!(observation.mismatched_session_refused)` `:8487` | `:136` | **behavioural, degenerate fixture.** See `DBND-705`. |
| A8 | `assert!(observation.cross_class_substitution_refused)` `:8488` | `:137`, `:138` | **SOURCE-TEXT.** `core_capability_api_is_narrow()`, `:2053-2058`: `include_str!("../src/session.rs")` tested with `!contains("pub fn sign(")`, `!contains("pub fn open(")`, `!contains("pub fn wrap(")`. |
| A9 | `assert!(!observation.secret_material_exposed)` `:8489` | `:139` | **CONSTANT.** `secret_material_exposed` is written `false` at `:2106`, `:3021`, `:3049`, `:3100` and at no other line in the file. The assertion is `assert!(!false)`. |
| A10 | `assert_eq!(observation.store, w.core_path_store)` `:11475` | `:151` | **harness-constant echo** (`:3140`, `:3327`). |
| A11 | `assert_eq!(observation.input_kind, …)` `:11476`, `assert_eq!(observation.invalid_input, …)` `:11477` | `:151` | **harness-constant echo** — the observation copies its own inputs back (`:3141-3142`, `:3328-3329`). |
| A12 | `assert!(observation.rejected)` `:11478` | `:151` | **behavioural.** MemStore rows: `.is_err()` of a read that would fail anyway — see `DBND-706`. FsStore rows: genuinely discriminating. |
| A13 | `assert!(!observation.outside_access_observed)` `:11479` | `:151` | **CONSTANT for the four MemStore rows** (`outside_access_observed: false`, `:3145`); behavioural for the six FsStore rows (`:3332-3335`), except row `:161` where the detector is unreachable by construction (`DBND-712`). |
| A14 | `assert!(…canonical_unchanged)` `:11396-11403` | `:152` | **behavioural, but three different comparisons under one sentence** — see `DBND-711`; and its baseline is taken after the attack fixture — see `DBND-710`. |

**Source-text assertions reached by this unit: one (A8), and it is the one the
repository already knows about.** Repo-wide search for the shape inside the
Gherkin layer: `grep -rn 'include_str!("../src/' rust/crates/aithos-bundle/tests/cucumber.rs`
returns exactly one line, `cucumber.rs:2054` — the site in my unit. The other 51
`_SOURCE.contains(` sites are in five non-Gherkin test binaries
(`cb2_bundle_boundaries.rs`, `cb2_bundle_authority_flows.rs`,
`cb2_draft2_carriers.rs`, `cb2_bundle_structure_vault.rs`,
`cb2_bundle_concurrency_final.rs`); none of them is executed by any scenario of
this Rule. `cb2_bundle_boundaries.rs:458-460` runs the *same three* greps as A8
from a plain `#[test]`, so A8 is a duplicate of an existing unit test wearing a
scenario's clothes.

**Constant assertions reached by this unit: two (A9, and A13 on four rows).**
Both are compile-time-known and cannot fail for any reason.

---

## 3. Per-scenario: the claim, what executes, whether they meet

### `:131` — *A bundle operation uses only its narrow opaque cryptographic capability*

**Normative ground, quoted verbatim to the end of each sentence**
(`spec/01-identity-and-keys.md`, § 1.6 *Local cryptographic capability
boundary*, `:150-157` and `:164-166`):

> Every stable capability is bound to one typed protocol purpose and context. It
> accepts a typed object or request rather than arbitrary caller-selected bytes and
> binds the expected subject, domain, Ethos, actor and, where relevant, node path, key
> version, and recipient before performing cryptography. A generic `sign(bytes)`,
> decrypt-bytes, cross-context opening, or wrap-bytes oracle is not a compliant Bundle
> API, and a capability for one artifact class cannot substitute for another;
> lower-level raw primitives may remain an implementation detail behind that
> boundary.

> The current local key implementation may back these capabilities directly. Stable
> APIs MUST NOT require a raw seed or private key when the narrow operation suffices,
> and MUST NOT expose private material as an output.

and (`:145-148`):

> Possessing such a capability is never sufficient authority: an owner capability is
> valid only in an owner-local session, and a grantee capability still requires proof
> of possession plus one valid mandate chain for the operation.

**What executes.** `core_capability_scenario` (`:3104-3115`) dispatches
`(capability, protocol_object)` to four fixtures. Each builds a `LocalSession`,
takes one typed capability handle, performs one typed operation, then builds a
`CoreCapabilityObservation` (`:325-334`) with eight fields, of which **one is a
literal (`secret_material_exposed: false`), three are literal echoes of the
`Examples` row (`capability`, `protocol_object`, `observable_result`), one is a
grep (`cross_class_substitution_refused`), and three are computed
(`operation_succeeded`, `mismatched_object_refused`,
`mismatched_session_refused`).**

**The production narrowness mechanism** is `LocalSession::check`
(`rust/crates/aithos-bundle/src/session.rs:233-240`):

```
fn check(&self, binding: &SessionBinding, class: CapabilityClass) -> Result<()> {
    if binding.id != self.id || binding.class != class {
```

plus the per-method context guards at `session.rs:250-254` (subject and actor),
`:275-282` (subject, zone, display path), `:304-312` (subject, zone, display
path, authority chain) and `:382-386` (subject).

**Do claim and execution meet?**

- The typed-object half meets. The four handles are real, non-`Clone`,
  non-serialisable, private-field structs (`session.rs:41-76`), each consumed by
  exactly one typed method. `operation_succeeded` is a genuine positive control
  in every row — this outline is *not* a vacuous negative, and I say so in § 6.
- The `binding.id` half meets, for all four rows.
- **`binding.class` — the "purpose" and "artifact class" half — meets nothing.**
  `CapabilityClass` (`session.rs:26`) and `SessionBinding` (`:35`) are private;
  every capability struct's `binding` field is private; and every call site
  passes the class that the parameter's own type already fixes (`:249`, `:274`,
  `:303`, `:323`, `:334`, `:348`, `:362`, `:370`, `:381`). No caller inside or
  outside the crate can present a wrong-class capability, so `binding.class !=
  class` is **unreachable**. The Gherkin line that names this property (`:137`)
  is proven by a grep. `DBND-704`.
- **The Ethos/actor/subject guards are never reached with a mismatch.** All four
  fixtures build `session` and `other` from identical arguments. `DBND-705`.
- **`:139` is proven by `assert!(!false)`.** `DBND-703`.
- **`:134`'s four English sentences are compared as strings, not evaluated.**
  `DBND-701`.
- The spec's "Possessing such a capability is never sufficient authority"
  sentence has no step in this outline. Search: the words *sufficient*,
  *authority*, *proof of possession* and *mandate chain* appear nowhere in
  `features/d-bundle.feature:129-146`.

### `:148` — *An untrusted path or Store key can never escape its selected root*

**Normative ground, quoted verbatim** (`spec/02-content-tree.md:84-99`, the
blockquote headed "**CB1 conformance-hardening decision — validated at the human
protocol gate on 2026-07-18; no new grammar.**"):

> Untrusted display paths are relative to their already-selected logical zone and
> enforce the human-name grammar of §2.2. They reject a leading absolute prefix,
> empty or dot segments, traversal, nonconforming names, and any resolution that
> would escape that zone before store access. Store keys are likewise relative and
> confined, but obey the exact canonical layout of §2.3 (whose fixed filenames and
> extensions are not human names), not the §2.2 name grammar. The logical canonical
> paths of §2.1 keep their leading `/e/...` or `/x/...`; they are not display-path
> inputs. `FsStore` anchors its opened canonical root and refuses any symlink,
> junction, reparse point, or equivalent indirection whose resolution would leave
> that root, before read, write, list, edition load, staging publication, or
> recovery. A signed manifest cannot legitimize an escape or out-of-layout object.
> The invariant is observable confinement and prescribes no particular syscall.

**What executes, row by row.** Production: `validate_display_path`
(`src/lib.rs:89-96`), `validate_store_key` (`:142-231`, a closed grammar),
`FsStore::checked_join` (`:553-579`, which calls `validate_store_key` **first**
and only then walks each segment with `symlink_metadata`).

| Row | Input | Path actually taken | Does the `filesystem_condition` label match? |
|---|---|---|---|
| `:156` MemStore `../circle/secret` | display path | `resolve_clear` → `gate_display_path` → `relative_segments` rejects `..` — **or**, absent the validator, index lookup fails | ok, but see `DBND-706` |
| `:157` MemStore `/absolute/section` | display path | same | ok, `DBND-706` |
| `:158` MemStore `folder/./section` | display path | same | ok, `DBND-706` |
| `:159` MemStore `folder//section` | display path | same. Note `bundle.rs:1196` filters empty segments, so this form is silently normalised if the validator is absent | ok, `DBND-706` |
| `:160` FsStore `folder/link-out/section` | display path, symlink outside zone | key `e/public/folder/link-out/section.md` passes the grammar via `lib.rs:157-160`; `checked_join` finds the symlink at the **intermediate** component | **yes — the one genuine intermediate-symlink row** |
| `:161` FsStore `../../outside` | Store key | `validate_store_key` rejects `..` in `relative_segments` | rejection ok; the escape *detector* is dead (`DBND-712`) |
| `:162` FsStore `e/circle/unlisted-object.json` | Store key | `validate_store_key` rejects: 3 segments, not in the literal list, no matching arm | genuine **out-of-layout** test, not out-of-root (`DBND-712`) |
| `:163` FsStore `e/circle/link-out/index.json` | Store key, "intermediate link-out targets outside root" | `validate_store_key` rejects on **grammar** before `checked_join` walks anything. The symlink installed at `:3244-3253` is never consulted | **NO — `DBND-708`** |
| `:164` FsStore `e/circle/index.json` | Store key, final component links outside root | key is in the literal list (`lib.rs:148`); grammar passes; `checked_join` finds the final-component symlink | yes |
| `:165` FsStore `manifest.json` | "cold-load key", signed manifest links outside root | key is in the literal list (`lib.rs:144`); grammar passes; `checked_join` finds the symlink. **But the operation executed is `bundle.store.get(…)`, not any cold-load API** | partially — `DBND-709` |

**Do claim and execution meet?** For the Store-key half, largely yes: the six
FsStore rows are genuinely discriminating, and rows `:160`, `:164`, `:165` are
real symlink-confinement tests against a real production walk. For the
display-path half, no: `DBND-706`. And the outline covers **one** of the six
surfaces the spec sentence enumerates ("before read, write, list, edition load,
staging publication, or recovery") — every row is a read. Search:
`OwnerContentOperation::` inside `cucumber.rs:3117-3337` returns three sites,
`:3131` `Read`, `:3217` `Create` (fixture setup only), `:3306` `Read`; the
non-display branch is `bundle.store.get(…)` at `:3317-3321`.

Also unexercised: the spec's "nonconforming names" clause. None of the ten
`invalid_input` values contains an uppercase byte, a non-ASCII byte, or a
segment longer than the 64-byte bound of `name_accepted` (`lib.rs:41-47`).
Search scope: `features/d-bundle.feature:156-165`, all ten cells read.

---

## 4. Findings

### `DBND-701` — P2 — `Then "<observable_result>"` compares strings, not propositions

**Statement.** `features/d-bundle.feature:134` puts the whole assertion in an
`Examples` column. The step definition compares that cell to a string literal
the harness itself wrote three thousand lines earlier. No row's English sentence
is evaluated as a claim about the system.

**Evidence.** `d_capability_result`, `cucumber.rs:8450-8462`. The comparison is
`assert_eq!(observation.observable_result, observable)` (`:8460`);
`observation.observable_result` is set to a literal at `:2101`, `:3016`, `:3044`
and `:3095`. The only behavioural conjunct is `assert!(observation.operation_succeeded)`
(`:8461`), a single boolean whose *definition* differs per row and whose meaning
the sentence does not constrain. Row `:145`'s sentence is "the expected plaintext
is recovered **only locally**"; `operation_succeeded` for that row is
`opened == "before atomic mutation"` (`:3045`) — it tests recovery and says
nothing about locality. Row `:146`'s sentence is "**only** the intended recipient
opens the wrapped key"; `operation_succeeded` is `header.open_latest(…, &intended_secret)`
yielding the DK (`:3084-3086`) — the *only* is carried by
`mismatched_object_refused` (`:3088-3090`), which is a separate step's assertion.

**Closure criterion.** `:134` becomes a fixed sentence naming the property, and
each row's positive control is computed from the operation's own output (verify
the produced signature against the DID verifying key inside the row that claims
it; assert non-derivability of the plaintext outside the session for the `open`
row).

### `DBND-702` — P2 — the `mismatched_object` column reaches no executing code

**Statement.** `features/d-bundle.feature:135` binds `<mismatched_object>` and
throws it away. For the two `sign` rows the boolean it asserts is an alias of the
boolean `:136` asserts, so two Gherkin lines are one proof counted twice.

**Evidence.** `d_mismatched_capability_refused`, `cucumber.rs:8464-8475`. The
parameter is written to `w.core_capability_mismatch` (`:8466`); exhaustive search
of the file for that identifier returns three lines — the declaration (`:541`),
a `.clear()` (`:8432`) and that write. **It is never read.** The asserted field
was computed by the `When`, which never received the column:
`core_capability_scenario` takes `(capability, protocol_object)` only
(`:3104-3107`). For rows `:143`/`:144` the field is literally assigned from the
session-mismatch boolean — `mismatched_object_refused: mismatched_session_refused`
(`:2103`, `:3018`) — so "using that capability for a Gamma entry is refused" is
proven by "a second identical session is refused". For row `:146` it is
`header.open_latest(subject, "delegate-kex", &wrong_secret).is_err()`
(`:3088-3090`), a wrong-X25519-secret decryption failure in which the capability
plays no part.

Only row `:145` is honest: `read_owner_section(&capability, &bundle,
Zone::Circle, "projects/sibling")` (`:3036`) reaches the real node-path binding
at `session.rs:275-282`, which fires before any store access.

**Mutant M2** (below): change the cell to a string that exists nowhere in the
repository; predict green with identical counters.

**Closure criterion.** The mismatched object must be *presented to the same
capability handle* and the refusal must be distinguishable from the
session-mismatch refusal (different `Error` variant or different message,
asserted).

### `DBND-703` — P1 — `:139` "no seed or private key is accepted or returned" is `assert!(!false)`

**Statement.** The Gherkin line that carries the spec's strongest MUST NOT in
this Rule is asserted against a field that is a compile-time constant.

**Evidence.** `secret_material_exposed` is declared at `cucumber.rs:333` and
written at exactly four lines — `:2106`, `:3021`, `:3049`, `:3100` — each
`secret_material_exposed: false`. Exhaustive search of the file returns those
four plus the declaration and the assertion `assert!(!observation.secret_material_exposed)`
at `:8489`. The assertion cannot fail for a behavioural reason, or any reason.

**Normative ground** (`spec/01-identity-and-keys.md:164-166`, quoted in full in
§ 3): "Stable APIs MUST NOT require a raw seed or private key when the narrow
operation suffices, and MUST NOT expose private material as an output."

**Why P1 and not P2.** A real defect could ship under this. If someone added a
public accessor returning `manifest_key`, `gamma_key` or `owner_kex` from
`LocalSession` or from any capability struct, no scenario of this Rule would
notice, and `:139` would still read as a green proof that no such thing exists.
The claim happens to be true at `d9120d7` — I read all 19 `pub fn` of
`session.rs` (`:111`–`:375`) and none returns key material — but the proof is
absent in exactly the way that lets the defect ship.

**Mutant M3** (below).

**Closure criterion.** `secret_material_exposed` is computed, not assigned:
either from an executed attempt (a typed call that would have to accept a seed),
or the line is deleted from the outline and the property is discharged by a
compile-fail test (`trybuild`) that the domain file names.

### `DBND-704` — P2 — `:137` and `:138` are decided by a grep of one source file, and `:137`'s production guard is unreachable

**Statement.** `cross_class_substitution_refused` — the sole evidence for two
Gherkin lines across all four rows — is the return of a function that reads
`src/session.rs` as text and looks for three literals.

**Evidence.** `core_capability_api_is_narrow()`, `cucumber.rs:2053-2058`:

```
let source = include_str!("../src/session.rs");
!source.contains("pub fn sign(")
    && !source.contains("pub fn open(")
    && !source.contains("pub fn wrap(")
```

Consumed at `:2105`, `:3020`, `:3048`, `:3099`; asserted at `:8488` by the
`regex =` step covering `:136`–`:139`. This site is already recorded by
`features/.agents/orchestrator/QUEUE.yaml`, key
`chdr-lota-source-text-assertions`, quoted at `features/.agents/d-bundle/STATE.md:256-277`,
which calls it "inside the Gherkin layer and … the worst" and notes its scope
limit is "counted, not classified". **This report classifies it: defective.**
`features/.agents/d-bundle/DOMAIN.md:425-432` routes it and explicitly leaves the
determination to me.

Three independent weaknesses:

1. **It is scoped to one file.** A universal `pub fn sign(` added to
   `src/sdk.rs`, `src/bundle.rs` or `src/vault.rs` is invisible to it. Search
   for the shape repo-wide: `grep -rn 'include_str!("../src/' rust/crates/aithos-bundle/tests/cucumber.rs`
   returns only `:2054`, so no companion grep covers the other modules from the
   Gherkin layer.
2. **It matches a formatting, not a property.** `pub fn sign_any(`,
   `pub  fn sign(`, or a `pub fn sign(` reached through a trait `impl` all pass.
3. **It duplicates an existing plain unit test.** `cb2_bundle_boundaries.rs:458-460`
   runs the identical three greps from a `#[test]`. The scenario adds nothing.

**And the guard `:137` names is dead code.** `binding.class != class`
(`session.rs:234`) can never be true: `CapabilityClass` (`:26`) and
`SessionBinding` (`:35`) are private items; each capability struct's `binding`
field is private to the `session` module; and each of the nine `self.check(…)`
call sites (`:249`, `:274`, `:303`, `:323`, `:334`, `:348`, `:362`, `:370`,
`:381`) passes the class its own parameter type already fixes. Cross-class
substitution is prevented by the type system, which is *stronger* than the
runtime guard — but neither the guard nor the type argument is what the scenario
asserts. The scenario asserts a grep.

**Mutants M4a, M4b** (below).

**Closure criterion.** Either (a) `:137`/`:138` are discharged behaviourally —
which requires a test-only path that can construct a wrong-class binding — or
(b) they are removed from the outline, the type-level argument is written into
`DOMAIN.md`, and a `trybuild` compile-fail case is added. The grep must not
remain the deciding evidence in either case.

### `DBND-705` — P2 — `:136`'s six-way mismatch enumeration is proven by two identical sessions

**Statement.** "arbitrary bytes or a mismatched Ethos, actor, purpose, node,
version or recipient are refused" is proven by `mismatched_session_refused`,
computed from a second session that differs from the first in **nothing except a
process-monotonic integer**. Arbitrary bytes are never submitted.

**Evidence.** The four fixtures:

- `core_manifest_capability_scenario`, `cucumber.rs:2075-2084` — `session` and
  `other` are both `LocalSession::grantee(context.subject.clone(), &signer,
  context.actor.authority_references().to_vec())`, argument for argument
  identical.
- `core_gamma_capability_scenario`, `:2985-2986` — both
  `LocalSession::owner(bundle.did.clone(), &owner)`.
- `core_body_capability_scenario`, `:3027-3028` — same.
- `core_header_capability_scenario`, `:3059-3060` — same.

The only difference is `LocalSession::id`, assigned from
`NEXT_SESSION_ID.fetch_add(1, …)` (`session.rs:23`, `:113`, `:135`). So `:136`
tests session-instance binding and nothing else. The guards its sentence names —
`context.subject != self.subject || context.actor != self.actor`
(`session.rs:250-254`), `bundle.did != self.subject` (`:275`, `:304`, `:382`),
`capability.zone != zone` (`:276`, `:305`), key version, recipient — are never
reached with a mismatched value from this outline. The one exception is row
`:145`, whose `mismatched_object_refused` does reach the display-path binding at
`:277`; that covers "node" for one row and is credited in § 6.

"Arbitrary bytes": no step in `:131-146` submits a byte string. The typed
methods have no byte-taking parameter (`session.rs:243`, `:267`, `:329`, `:354`),
so this conjunct has nothing to execute against and is vacuous by construction.

**Closure criterion.** One row per named dimension, each built from a session or
context that differs in exactly that dimension, with the refusal's `Error`
variant asserted so a different refusal cannot stand in.

### `DBND-706` — P2 — the four MemStore rows of `:148` survive deletion of display-path validation

**Statement.** For the four MemStore rows, `rejected` cannot distinguish "the
confinement grammar refused this path" from "no such section exists", and the
out-of-root detector is a hardcoded `false`. The rows stay green with
`validate_display_path` removed.

**Evidence.** `core_path_mem_scenario`, `cucumber.rs:3117-3147`. It computes
`rejected = bundle.owner_content_operation(Zone::Circle, OwnerContentOperation::Read
{ display_path: invalid_input }, …).is_err()` (`:3128-3137`) and sets
`outside_access_observed: false` (`:3145`) — a literal, asserted at `:11479`.
`canonical_unchanged: before == after` (`:3144`) is trivially true because a read
does not mutate.

The fixture `core_atomic_bundle` (`:1699-1733`) publishes exactly one circle
section, `projects/note`. None of `../circle/secret`, `/absolute/section`,
`folder/./section`, `folder//section` names an existing section. With the
validator neutered, `Bundle::resolve_clear` (`src/bundle.rs:1193-1218`) splits on
`/`, **filters empty segments** (`:1196` — so `folder//section` is silently
normalised to `folder/section`), and returns
`Err(Error::InvalidPath("no folder …"))` for all four. `.is_err()` is still true.

**Normative ground** (`spec/02-content-tree.md:86-89`): "Untrusted display paths
are relative to their already-selected logical zone and enforce the human-name
grammar of §2.2. They reject a leading absolute prefix, empty or dot segments,
traversal, nonconforming names, and any resolution that would escape that zone
before store access."

**Mutant M1** (below): replace `validate_display_path`'s body with `Ok(())`;
predict the whole `:148` outline stays 10/10 green.

**Closure criterion.** Assert the *kind* of refusal — `io::ErrorKind::InvalidInput`
from `invalid_path` (`lib.rs:33-35`) or `PermissionDenied` from
`confinement_error` (`:37-39`) — rather than `.is_err()`, and add rows whose
display path is valid-but-absent so the two failure modes are separated.

### `DBND-707` — P2 — `:148` is a vacuous negative: no positive control anywhere

**Statement.** All ten rows assert rejection. No row supplies a valid display
path or a valid Store key. A defect that rejected *every* input would keep the
outline green.

**Evidence.** Search: `grep -n 'core_path_scenario' cucumber.rs` returns the
definition (`:3348`) and exactly one call site (`:11459`), so the only inputs
this code ever sees are the ten `Examples` cells at
`features/d-bundle.feature:156-165`, all of which are designed to fail. Layer:
the Gherkin harness; the plain unit tests in `cb2_store_key_consumer_neutrality.rs`
and `cb2_bundle_boundaries.rs` are outside this Rule's proof and are not reached
by any scenario of it.

Contrast `:131`, which *does* have a per-row positive control
(`operation_succeeded`, A5). The two halves of this Rule are not built to the
same standard.

**Closure criterion.** At least two rows — one per store — with a valid input
and a `Then` asserting success, sharing the same step definitions.

### `DBND-708` — P2 — row `:163` never reaches the symlink check its `filesystem_condition` names

**Statement.** `| FsStore | Store key | e/circle/link-out/index.json |
intermediate link-out targets outside root |` is rejected by the store-key
grammar before `checked_join` walks a single component. The symlink the fixture
installs is never consulted. The outline's only row labelled for the
intermediate-symlink case does not test it.

**Evidence.** `FsStore::checked_join` (`src/lib.rs:553-579`) calls
`validate_store_key(key)?` as its first statement (`:554`); the per-segment
`symlink_metadata` loop is `:564-577`. `validate_store_key` (`:142-231`) admits
`e/circle/index.json` by literal (`:148`) but has no arm for a four-segment
`e/circle/<name>/index.json`: the `e/*/blobs/*.enc` arm requires
`segments[2] == "blobs"` (`:164`), the `hdr` arm `segments[2] == "hdr"` (`:169`),
the `wraps` arm `segments[2] == "wraps"` (`:176`). So the key is refused at
`:227-229` with `"path is outside the canonical Bundle object grammar"`. The
symlink installed at `cucumber.rs:3244-3253` and the file
`b"escaped intermediate"` are dead fixture.

Row `:160` *does* exercise the intermediate-symlink walk — its key
`e/public/folder/link-out/section.md` passes the grammar through the
`e/public/…​.md` arm (`lib.rs:157-160`). So the property is covered, by a row
carrying a different label.

**Mutant M5** (below): delete the symlink arm of `checked_join`; predict `:163`
stays green while `:160`, `:164`, `:165` go red.

**Closure criterion.** Relabel `:163`'s `filesystem_condition` as a grammar
condition, and give the intermediate-symlink case a second row whose key is
inside the grammar.

### `DBND-709` — P2 — `cold-load key` is a label with no distinct code path, and the spec's other five surfaces are untested

**Statement.** The scenario's name enumerates two input kinds; the table supplies
three. The third, `cold-load key` (`:165`), executes byte-for-byte what a
`Store key` executes — a plain `Store::get`. No cold-load or edition-load API is
invoked. Meanwhile the spec sentence this outline is built on names six surfaces
and the outline exercises one.

**Evidence.** `core_path_fs_scenario` branches on `input_kind == "display path"`
(`cucumber.rs:3302`); every other value falls through to
`bundle.store.get(invalid_input)` (`:3317-3321`). The `match` arm for
`("cold-load key", …)` (`:3266-3277`) differs from its neighbours only in which
file it symlinks. Search: `grep -n 'cold_verify\|import_keyless\|export_keyless'
cucumber.rs` returns `:20` (the `use`), `:2282`, `:2783`, `:2843`, `:2845`,
`:2851`, `:2854`, `:2883`, `:2885` — all inside the keyless/cold publication
scenario family, **none inside `core_path_mem_scenario`, `core_path_fs_scenario`
or `core_path_scenario` (`:3117-3359`)**.

**Normative ground** (`spec/02-content-tree.md:93-96`): "`FsStore` anchors its
opened canonical root and refuses any symlink, junction, reparse point, or
equivalent indirection whose resolution would leave that root, before read,
write, list, edition load, staging publication, or recovery." Search of
`cucumber.rs:3117-3337` for `OwnerContentOperation::` returns `:3131` `Read`,
`:3217` `Create` (fixture construction, not the operation under test) and
`:3306` `Read`; the non-display branch is `store.get`. **Five of six surfaces —
write, list, edition load, staging publication, recovery — have no row.**

The spec's "nonconforming names" clause is also unexercised: none of the ten
`invalid_input` cells contains an uppercase byte, a non-ASCII byte, or a segment
over the 64-byte bound of `name_accepted` (`lib.rs:41-47`).

**Closure criterion.** Either row `:165` calls `publication::cold_verify` /
`import_keyless` and the scenario name is widened to three input kinds, or the
row is relabelled `Store key`. Separately, five rows for the five uncovered
surfaces, and one row for a nonconforming name.

### `DBND-710` — P2 — the shared `Given` constructs nothing, and `:152`'s baseline is post-attack

**Statement.** `Given a published "<store>" bundle snapshotted byte for byte`
(`:149`, and `:92`/`:117` in `RU-6`) publishes no bundle and takes no snapshot.
Worse, the snapshot `:152` actually compares against is taken **after** the
attack fixture has been installed, so for five rows it already contains the
attacker's artifacts.

**Evidence.** `core_atomic_fixture`, `cucumber.rs:11346-11353`: four world-field
assignments, `return`. The bundle is built inside the `When`
(`core_atomic_bundle` at `:3126` / `:3211`). The FsStore baseline
`let before = core_path_raw_snapshot(root.path())?` is at `:3300`, **after** the
fixture `match` at `:3232-3298` has renamed real objects aside and installed
symlinks (rows `:160`, `:163`, `:164`, `:165`) or written an unlisted file
(row `:162`). `:152` therefore asserts that a tampered tree equals itself.

For row `:164` this is materially odd: `active/e/circle/index.json` — a real
canonical object — has been renamed to `.index-original` and replaced by a
symlink, and the "canonical bundle" that `:152` finds "byte-for-byte identical
to the snapshot" is that replaced tree.

**Closure criterion.** Take the raw snapshot before the fixture mutation and
compare against it, or reword `:149`/`:152` to say what is actually compared.

### `DBND-711` — P3 — the shared `Then` is one sentence over three comparisons, selected by a routing hint

**Statement.** `the canonical bundle is byte-for-byte identical to the snapshot`
(`:96` in `RU-6`, `:152` here) resolves to a body that decides which claim to
check by asking which world field the `When` happened to fill.

**Evidence.** `core_atomic_unchanged`, `cucumber.rs:11393-11405`:
`if let Some(observation) = &w.core_path_observation { … } else { … core_atomic_observation(w) … }`.
The routing is load-bearing — `core_atomic_fixture` nulls both fields
(`:11350`, `:11352`), and without the branch `RU-7b` would panic on the missing
`core_atomic_observation`. The three `canonical_unchanged` values it may read are
computed by three different comparators over three different baselines:

- `RU-6` (both outlines): `cb7_store_snapshot` (`:1375-1388`) — `store.list("")`
  then `get` each key, i.e. **only grammar-valid, listed objects**.
- `RU-7b` MemStore: `cb7_store_snapshot` again (`:3127`, `:3138`).
- `RU-7b` FsStore: `core_path_raw_snapshot` (`:3149-3193`) — a raw `read_dir`
  walk that records directories, dotfiles and symlink targets.

A defect that leaves an out-of-grammar file in the tree is invisible to the
first and second and visible to the third. One English sentence, three
propositions.

**Closure criterion.** Two step texts, or one comparator.

### `DBND-712` — P3 — `:151`'s "out-of-root" wording does not fit two rows, and one row's escape detector is unreachable

**Statement.** Row `:162` is an in-root object; the property it tests is
out-of-*layout*, not out-of-root. Row `:161`'s `outside_access_observed`
detector cannot fire under any implementation.

**Evidence.** Row `:162`: the fixture writes the file at
`active.join(invalid_input)` (`cucumber.rs:3281`) — inside the store root — and
sets `expected_escape_bytes` to its contents (`:3286`). `outside_before !=
outside_after` (`:3332`) compares an untouched sibling directory. The property
actually proven is the spec's own separate sentence (`spec/02-content-tree.md:96`):
"A signed manifest cannot legitimize an escape or out-of-layout object."

Row `:161`: `expected_escape_bytes` is written to
`outside.path().join("outside")` (`:3289`). `Cb7TempRoot::new` (`:1425-1442`)
names its directories `aithos-cb7-cucumber-<pid>-core-path-store-<n>` and
`aithos-cb7-cucumber-<pid>-core-path-outside-<n>` under a common base. No
resolution of the literal key `../../outside` from the store base can name the
second. So for that row `outside_access_observed` is `false` regardless, and
`rejected` carries the whole assertion.

**Closure criterion.** Split `:151` into an out-of-layout `Then` and an
out-of-root `Then`; for `:161`, place the escape target where `../../outside`
would actually resolve.

### `DBND-713` — P3 — `#[then(expr = "{string}")]` is an unbounded wildcard over the whole suite

**Statement.** `d_capability_result` (`cucumber.rs:8450`) matches *any* `Then`
whose entire text is one quoted string, in any of the 18 feature files the
runner loads.

**Evidence.** The runner loads the whole `features/` directory
(`cucumber.rs:20017-20039`) with `.fail_on_skipped()`. Search:
`grep -rn 'Then "' features/*.feature` returns exactly one line,
`features/d-bundle.feature:134`. So there is no ambiguity today. Any future
`Then "…"` written anywhere in the suite binds silently to this body and fails
with `CORE-OWN-003 observation`, pointing an author at a Rule they have never
read.

**Closure criterion.** Anchor the step, e.g. `#[then(expr = "the capability
result is {string}")]`, and give `:134` the corresponding prefix.

### `DBND-714` — P3 — "narrow" carries two unrelated senses across `RU-5` and `RU-7`

**Statement.** `RU-5`'s "the narrow owner capability" (`:67`) and `RU-7`'s "its
narrow opaque cryptographic capability" (`:131`, and the Rule title at `:129`)
name different things, and no step body relates them.

**Evidence.** `:67` resolves to `core_owner_succeeds` (`cucumber.rs:11506` ff.),
which reads a `CoreOwnerObservation` (`:303-310`) whose fields are `zone`,
`operation`, `outcome`, `gamma_delta`, `mandate_counter_delta`, `reopened`. No
capability object exists in that struct or that code path; "narrow" there means
*authority scope* — owner-local, no mandate, no counter consumed. `:131`
resolves to `CoreCapabilityObservation` (`:325-334`), whose subject is the *API
surface*. The spec has both senses in two different files:
`spec/04-mandates.md:1861` — "Owner | Local narrow capability; operation is
authorized without a mandate, journalized, and consumes no mandate counter or
constraint." — versus `spec/01-identity-and-keys.md:150-157`, quoted in § 3.

**Closure criterion.** Distinct wording in the two Rules, and a `DOMAIN.md`
glossary entry fixing each sense to its spec sentence.

### `DBND-715` — P3 — the Rule's two halves share nothing but the word "narrow"

Stated as a finding because it is a defect of the Rule, not only an observation.
See § 5 for the full argument and the disposition I recommend.

### Recorded follow-ups: what this unit touches

Of the seven `QUEUE.yaml` entries naming `d-bundle` (quoted at
`features/.agents/d-bundle/STATE.md:71-217`), **exactly one lands in this unit**:
`chdr-lota-source-text-assertions`, whose text names
`cucumber.rs:2053-2058 core_capability_api_is_narrow()` and says the site is
"counted, not classified". `DBND-704` classifies it. The remaining six —
`chdr-028`, `chdr-i3-d-bundle`, `chdr-016-grant-path`, `bder-006-d-bundle`,
`b-derivation-round-2-targeted`, `chdr-i3-targeted`,
`chdr-lota-vector-generators` — concern I3 header pinning, the grant path, tag
views and vector generators, and no step of `:129-165` reaches any of them.
`bder-006-d-bundle`'s quoted text explicitly places `d-bundle`'s four uses of
`wrap` and notes that `:138`/`:146` are `Examples` rows of *this* Rule and are
**not** anchor bridging (`STATE.md:164-171`) — I confirm that reading:
`:146`'s `wrap` is `HeaderWrappingCapability`, unrelated to tag-view anchors.

Of the three published audits (`docs/audits/features/{a-identity,b-derivation,c-headers}.md`),
**none names any step, function or line of this unit.** Search:
`grep -n 'core_path\|core_capability\|narrow capab' docs/audits/features/*.md`
returns nothing; their `d-bundle` mentions are the re-routings and debts already
listed above.

---

## 5. Verdicts asked for

### Are the two blocks one subject? **No.**

They share no world field, no helper, no production module, no spec section and
no vocabulary.

| | `:131` (authority) | `:148` (reach) |
|---|---|---|
| World fields | `core_capability*` (`cucumber.rs:539-542`) | `core_path*` (`:543-547`) |
| Harness entry | `core_capability_scenario` (`:3104`) | `core_path_scenario` (`:3348`) |
| Production under test | `src/session.rs` | `src/lib.rs` (`validate_*`, `FsStore`), `src/bundle.rs` |
| Spec ground | `spec/01-identity-and-keys.md` § 1.6, the "CB1 decision G-C" blockquote | `spec/02-content-tree.md`, the "CB1 conformance-hardening decision" blockquote |
| Shared step definitions | none | `:149`/`:152` shared with `RU-6`, not with `:131` |

The intersection of the two blocks' step-definition sets is **empty**. The only
join is the Rule title, and even the title's conjunction is asymmetric:
"capabilities … stay narrow" is about what a handle may *do*, "paths stay
narrow" is about what a string may *address*. I1 declined to split on size
grounds and asked Pass A to test the premise; the premise is false.

**But I do not recommend a split, and this is the finding (`DBND-715`).**
Splitting produces two Rules each of which still has the defects above. The
useful correction is the opposite: the title currently lets each half borrow the
other's credibility. A reader who sees `:148`'s six genuine FsStore rows
concludes the Rule is well proven and does not notice that `:131`'s four rows
rest on a grep, a constant and a dead column. **Renaming is the cheap part;
`DBND-701`–`DBND-705` are what the Rule owes.**

### Do the step bodies shared with `RU-6` serve both claims?

**The `Given` (`:92`, `:117`, `:149` → `core_atomic_fixture`, `:11346`) serves
neither.** It announces "a published `<store>` bundle snapshotted byte for byte"
and performs four world-field assignments. For `RU-6` the bundle and snapshot are
built by `core_atomic_failure_scenario` / `core_atomic_success_scenario` inside
the `When`; for `RU-7b` by `core_path_*_scenario` inside the `When`. In both
Rules the sentence is false of the step at the moment it runs. This is a step
that "announces one state and constructs another" in the purest form — it
constructs nothing. `DBND-710`.

**The `Then` (`:96`, `:152` → `core_atomic_unchanged`, `:11393`) serves `RU-6`
and does not serve `RU-7b`.** For `RU-6` it *is* the atomicity claim, and its
comparator (`cb7_store_snapshot`, listed-object equality) is the right shape for
"a failed mutation left nothing behind". For `RU-7b` it is a side condition on a
read — trivially true, since no row performs a mutation — and its FsStore
comparator is a different function with a different notion of "byte" and a
baseline taken after the attack. So: one sentence, `RU-6`'s meaning, reused where
it costs nothing and proves nothing. This is precisely the shape the brief warned
about — a step body written for one Rule and reused by another — and the
discrimination it loses is `RU-7b`'s, not `RU-6`'s. `DBND-711`.

Note the direction matters for the correction: fixing `core_atomic_unchanged`
for `RU-7b` must not weaken `RU-6`, which is the larger unit (108 steps) and the
one where the assertion carries weight.

---

## 6. What I attacked and could not break

Recorded so that the report is not read as uniformly negative. Each of these is a
place where I expected a defect of the named class and did not find one.

1. **The primed failure mode is absent.** I expected the 19 shared
   `OnceLock`-cached verdicts. Neither outline touches a `static`; every row
   builds its fixture fresh. All 14 rows execute distinct bytes. I say this
   plainly because a Pass A that finds what it was told to find is worth less.

2. **`:131` is not a vacuous negative.** Every row carries a real positive
   control, `operation_succeeded` (`:8461`), computed four different ways
   (`:2091-2094`, `:3011`, `:3045`, `:3084-3086`). Row `:143` compares a
   JCS-canonical draft.2 candidate against a pinned vector; row `:144` calls
   `aithos_core::gamma::verify_owner_entry` against the real `did.json` read out
   of the store. These are not decorations. (`:148` *is* a vacuous negative —
   `DBND-707` — which is why the contrast is worth stating.)

3. **The production path-confinement code is strong, and I tried to find a hole
   in it.** `validate_store_key` (`lib.rs:142-231`) is a genuinely closed
   grammar, not a traversal blacklist — a key must match one of about fifteen
   exact forms. `FsStore::checked_join` (`:553-579`) validates first, then walks
   every prefix of the key with `symlink_metadata`, rejecting any symlink
   component including the final one, and treats `NotFound` as acceptable so the
   walk is not defeated by ordering. `collect_from` (`:581-635`) applies the same
   check to listings and re-validates every discovered key. I found no path form
   that reaches outside the root. **Nothing in this report is a claim that the
   production confinement is broken.**

4. **The capability handles are genuinely narrow at the type level.** All five
   structs (`session.rs:41-76`) have private fields, no `Clone`, no `Serialize`,
   and an explicit `Drop`. `CapabilityClass` and `SessionBinding` are private
   items. `read_owner_section` really does bind subject, zone and display path
   before touching the store (`:275-282`), and `read_grantee_section` adds the
   exact authority chain (`:304-312`). There is no `pub fn` on `LocalSession`
   returning key material — I read all 19.

5. **Row `:145` is honest.** Its `mismatched_object_refused` reaches a real
   binding at `session.rs:275-282` and would fail if the display-path binding
   were removed. It is the one row of `:131` whose mismatch dimension is executed.

6. **Rows `:160`, `:164`, `:165` are honest.** They reach `checked_join`'s
   symlink walk at an intermediate component, a final component and the signed
   manifest respectively, with escape-detection that would fire if the walk were
   removed. Mutant M5 is designed to confirm exactly this and to isolate `:163`
   as the row that would not.

7. **`d_capability_result` is not currently ambiguous.** I expected the
   `{string}` wildcard to collide with another feature. Search across all 18
   feature files found exactly one bare-quoted `Then`. `DBND-713` is a latent
   hazard, not a live one, and is rated P3 accordingly.

---

## 7. What I could not verify, and why

1. **Everything in § 4 that predicts a run outcome.** I ran nothing. Every
   "predict green" below is an argument from reading, and each is falsifiable by
   one named command in § 8. **No finding in this report cites an `evidence_id`,
   because none was issued to me.** Findings `DBND-701` through `DBND-705`,
   `DBND-708`, `DBND-712` and `DBND-713` rest on code reading alone and I regard
   them as sound on that basis (they are statements about what the source says).
   `DBND-706`, `DBND-707`, `DBND-709`, `DBND-710` and `DBND-711` assert what a
   run would do and **must not be published as confirmed until M1, M2, M3, M4a,
   M4b and M5 have been executed and journalled.**

2. **Whether the four `:131` rows would still pass on a non-Unix host.**
   `core_path_fs_scenario` has a `#[cfg(not(unix))]` twin returning
   `Err("CORE-OWN-004 symlink scenarios require Unix")` (`cucumber.rs:3339-3346`),
   which `core_path_refused_before_access` turns into a panic (`:11474`). So six
   of the ten `:148` rows fail outright off Unix. Whether CI is Unix-only I did
   not establish; `.github/workflows/ci.yml` is in the archive but the CI matrix
   is outside my unit and I did not read it as evidence.

3. **Whether `binding.class` is reachable through any `unsafe`, macro or
   test-only path I did not search.** My reachability argument for `DBND-704`
   rests on Rust module privacy and on the nine `self.check` call sites in
   `session.rs`. I searched that file exhaustively; I did not audit the whole
   workspace for a `mem::transmute` or a `#[cfg(test)]` constructor. If one
   exists, `DBND-704`'s "dead code" half weakens (its "grep is the evidence" half
   does not).

4. **What "narrow" means normatively for `RU-5`.** I read
   `spec/04-mandates.md:1861` and `spec/01-identity-and-keys.md` §§ 1.5–1.6, but
   `RU-5` is another auditor's unit and I did not audit its step bodies beyond
   the one line needed for `DBND-714`.

5. **Whether the four `:131` `Examples` rows are the complete artifact-class
   set.** `features/.agents/d-bundle/DOMAIN.md:797-806` already records that no
   spec section governs this table — "the four rows of
   `features/d-bundle.feature:142-146` are a closed table with no normative
   counterpart this file could find" — and routes the question to the auditor. I
   confirm the search (`grep -rn "narrow\|opaque capability\|purpose-bound"
   spec/*.md`, eleven lines, six files) and confirm that no spec sentence
   enumerates the artifact classes. But `session.rs` exposes a **fifth**
   capability class, `AuditArgsCapability` (`:73`, `:221`, `:369`, `:375`), which
   the `Examples` table does not name. Whether that is a coverage gap or a
   correct exclusion I cannot settle: it needs the D9 audit/config derivation
   topology, which `spec/01-identity-and-keys.md:167-169` says is "reserved for
   the CB2 vectors". **Routed, not settled.**

6. **`PROCESS.md` does not contain the section my instructions cite.** I was told
   "Do not open `/root/work/aithos-core`. `PROCESS.md` § *Material isolation of
   Pass A*." That section does not exist. `features/.agents/PROCESS.md` has
   eleven `##` headings — Objective, Feature branch lifecycle, Feature targeting
   and gate pyramid, Current scope, Evidence hierarchy, Required two-pass audit,
   Review-unit isolation and impartiality, Artifacts, Manual lifecycle, Evidence
   statuses, Required run conclusion — and no occurrence of "Material isolation",
   "disclosure" or "blocking condition". `d-bundle/DOMAIN.md:790-796` and
   `g4-client-surfaces/DOMAIN.md:577-591` already record this gap. **I obeyed the
   instruction as given and did not open the other tree**; I report the citation
   gap rather than treat the rule as non-existent.

---

## 8. Commands I need run

None of these is a gate. Each is a mutant plus a run, and each is stated as an
applicable change with file, line, before and after. **I need each run twice —
baseline and mutant — with counters, so that "identical counters" is a measured
statement and not my prediction.**

**Baseline (needed first, once, for all six):**

```
cd rust && cargo test -p aithos-bundle --test cucumber -- --no-fail-fast
```

Requested: full stdout with the feature/rule/scenario/step counters, and the
per-scenario result lines for `features/d-bundle.feature:131` and `:148`.

### M1 — does `:148` notice the display-path validator disappearing? (`DBND-706`, `DBND-707`)

File `rust/crates/aithos-bundle/src/lib.rs`, lines 89–96.

```diff
-pub fn validate_display_path(value: &str) -> io::Result<()> {
-    let segments = relative_segments(value)?;
-    if segments.iter().all(|segment| name_accepted(segment)) {
-        Ok(())
-    } else {
-        Err(invalid_path("display path contains an unsupported name"))
-    }
-}
+pub fn validate_display_path(_value: &str) -> io::Result<()> {
+    Ok(())
+}
```

**Prediction: `features/d-bundle.feature:148` stays 10/10 green, counters
identical to baseline.** If any row of `:148` goes red I am wrong and
`DBND-706` is withdrawn. (Other features may go red; that is not this
prediction. I need the per-scenario lines for `:148`, not just the totals.)

### M2 — is the `mismatched_object` column dead? (`DBND-702`)

File `features/d-bundle.feature`, line 143. Replace the third column value.

```diff
-        | sign       | domain-tagged edition manifest          | Gamma entry                             | the signature verifies against the public key    |
+        | sign       | domain-tagged edition manifest          | zzz-no-such-object-anywhere-in-this-repo | the signature verifies against the public key    |
```

**Prediction: green, counters identical.** Control, in the same run if you can
take two mutants at once — line 143 column 4 changed instead:

```diff
-        | sign       | domain-tagged edition manifest          | Gamma entry                             | the signature verifies against the public key    |
+        | sign       | domain-tagged edition manifest          | Gamma entry                             | zzz-no-such-result                               |
```

**Prediction: RED at `cucumber.rs:8460`.** The pair together is the finding: one
column reaches an assertion, the other reaches nothing.

### M3 — does `:139` notice a seed accessor appearing? (`DBND-703`)

File `rust/crates/aithos-bundle/src/session.rs`, after line 162 (inside
`impl<'a> LocalSession<'a>`).

```diff
     #[must_use]
     pub fn actor(&self) -> &K1cActor {
         &self.actor
     }
+
+    #[must_use]
+    pub fn manifest_private_key(&self) -> &SigningKey {
+        self.manifest_key
+    }
```

**Prediction: all four rows of `:131` stay green, counters identical.** This is
the mutant that widens the capability, and if the unit stays green under it that
is the whole of `DBND-703`.

### M4a — is the narrowness grep defeated by a rename? (`DBND-704`)

Same file, same location.

```diff
+    #[must_use]
+    pub fn sign_any(&self, message: &[u8]) -> [u8; 64] {
+        use ed25519_dalek::Signer;
+        self.manifest_key.sign(message).to_bytes()
+    }
```

**Prediction: all four rows of `:131` stay green.** A universal byte-signing
oracle now exists on `LocalSession`, and `spec/01-identity-and-keys.md:153-155`
says in terms that "A generic `sign(bytes)` … oracle is not a compliant Bundle
API". The grep at `cucumber.rs:2055` looks for `pub fn sign(` and this is
`pub fn sign_any(`.

### M4b — is it defeated by moving files? (`DBND-704`)

Same addition as M4a but named `pub fn sign(` exactly, placed in
`rust/crates/aithos-bundle/src/sdk.rs` rather than `session.rs`.

**Prediction: all four rows of `:131` stay green.** Control: the same
`pub fn sign(` added to `session.rs` should go RED, which establishes that the
grep has teeth for exactly one literal in exactly one file and none elsewhere.

### M5 — which rows actually reach the symlink walk? (`DBND-708`)

File `rust/crates/aithos-bundle/src/lib.rs`, inside `checked_join`, lines
566–577.

```diff
         for segment in key.split('/') {
             path.push(segment);
-            match std::fs::symlink_metadata(&path) {
-                Ok(metadata) if metadata.file_type().is_symlink() => {
-                    return Err(confinement_error(format!(
-                        "store path crosses a symlink: {}",
-                        path.display()
-                    )));
-                }
-                Ok(_) => {}
-                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
-                Err(error) => return Err(error),
-            }
         }
```

**Prediction, per row of `:148`:** `:160` RED, `:163` **GREEN**, `:164` RED,
`:165` RED, `:161` green, `:162` green, all four MemStore rows green. The
isolated survival of `:163` — the only row whose `filesystem_condition` says
"intermediate link-out targets outside root" — is `DBND-708` in one line.

### M6 — is the `:152` baseline post-attack? (`DBND-710`)

No production change. File
`rust/crates/aithos-bundle/tests/cucumber.rs`: move the two baseline lines

```
    let before = core_path_raw_snapshot(root.path())?;
    let outside_before = core_path_raw_snapshot(outside.path())?;
```

from line 3300–3301 to immediately after line 3231
(`let active = core_path_active_generation(root.path())?;`), i.e. before the
`expected_escape_bytes` match.

**Prediction: rows `:160`, `:163`, `:164`, `:165` and `:162` go RED** at
`cucumber.rs:11396`, because `canonical_unchanged` would then be comparing the
published tree to the tampered tree. That is not a defect to fix by reverting —
it is the demonstration that `:152` currently asserts nothing about the
*published* bundle, which is what its `Given` claims.

---

## 9. Disclosure gate

**Nothing in this report is embargoed, and I have withheld nothing from this
file.**

The gate applies to a finding whose statement describes an exploitable weakness
for which no fix exists. I checked each of mine against that test. Every one of
`DBND-701`–`DBND-715` is a statement about **the proof**, not about the
behaviour: it says a scenario would stay green under a defect, not that the
defect is present. The production surfaces this Rule covers — the closed
store-key grammar (`lib.rs:142-231`), the per-component symlink walk
(`:553-579`), the listing re-validation (`:581-635`), the session-id and context
bindings (`session.rs:233-240`, `:250-254`, `:275-282`, `:304-312`) — I read in
full and found intact at `d9120d7`. § 6 records that positively.

The one finding that comes closest to the gate is `DBND-703`, whose mutant M3
*creates* an exposure in order to show the assertion cannot see it. Publishing
M3 discloses nothing an attacker could use: it is a patch that adds a method to
a crate the attacker would already have to be able to modify. Publishing it is
also necessary — the finding is not checkable otherwise, and
`chdr-lota-mutants-as-patches` (`STATE.md:245-254`) requires every mutant to be
published as an applicable diff.

If Pass B or the refutation panel disagrees on `DBND-703`, the redaction that
preserves the finding is to publish identifier, severity and the neutral title
"a secret-exposure assertion is backed by a constant", and hold M3's diff. I do
not recommend it, but I flag it as the only candidate.

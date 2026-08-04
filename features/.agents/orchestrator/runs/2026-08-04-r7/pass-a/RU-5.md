# Pass A — RU-5 — `Rule: Owner operations have durable parity across all three zones`

Unit: `features/d-bundle.feature:61`. One authored `Scenario Outline` (`:63`),
six step lines (`:64`–`:69`), fifteen `Examples` rows (`:73`–`:87`) → 15
scenarios, 90 steps.

Material: `/root/work/passA-d-bundle/RU-5`, `git archive` of `d9120d7`, no
`.git`. `/root/work/aithos-core` was not opened. No gate, test or `cargo`
command was run by me; every behavioural claim below is marked either
**[source]** (read from the archive) or **[needs evidence_id]** (a prediction
awaiting an orchestrator run). Findings are numbered `DBND-5xx` per the
instruction that Pass A auditors cannot coordinate numbering.

---

## 1. Parameter-reachability table

The failure mode I was told to test first — a shared, process-lifetime cached
verdict consumed by `".*"` regexes with no parameter — **does not hold here.**
Both `Examples` columns reach production code. The control mutants that would
prove it are M1a/M1b in §7.

| # | Feature line | Step text | Step definition | Takes `<zone>` | Takes `<operation>` | Uses it |
|---|---|---|---|---|---|---|
| 1 | `:64` | `Given an owner-local bundle session for zone "<zone>"` | `core_owner_zone` — `rust/crates/aithos-bundle/tests/cucumber.rs:11484` | **yes**, `zone: String` | no | **yes, transitively.** Body stores `w.core_owner_zone = zone` and clears the observation. The value is read by step 3 and passed to `core_owner_scenario` (`:3361`), which maps it to `Zone::Public\|Circle\|Self_` (`:3362-3367`) and hands it to production. An unmapped value returns `Err("CORE-OWN-001 unknown zone {other}")`. |
| 2 | `:65` | `And a published existing folder and section in that zone` | `core_owner_fixture` — `cucumber.rs:11491` | no | no | **nothing.** Whole body: `w.core_owner_fixture_ready = true;`. See DBND-508. |
| 3 | `:66` | `When the owner performs "<operation>" through the common bundle operation` | `core_owner_operation` — `cucumber.rs:11496` | reads it from World | **yes**, `operation: String` | **yes.** Asserts the fixture flag, then calls `core_owner_scenario(&w.core_owner_zone, &w.core_owner_operation)` (`:3500-3503`). Unknown operations return `Err` at `:3369`. `<operation>` selects one of five arms at `:3420-3459`, each a distinct `OwnerContentOperation` variant into `Bundle::owner_content_operation` (`rust/crates/aithos-bundle/src/bundle.rs:444`). |
| 4 | `:67` | `Then the operation succeeds from the narrow owner capability without a mandate` | `core_owner_succeeds` — `cucumber.rs:11506` | no | no | reads `w.core_owner_zone`, `w.core_owner_operation` and the observation. All three comparisons are round-trips — see DBND-505. This is the step whose `unwrap_or_else(panic!)` (`:11511-11512`) surfaces every `Err` the `When` produced. |
| 5 | `:68` | `And every mutation is journalized without consuming mandate counters` | `core_owner_gamma` — `cucumber.rs:11528` | no | no | reads `observation.gamma_delta` and `observation.operation`; asserts `observation.mandate_counter_delta == 0` (`:11543`) against a field written as the literal `0` at `:3549`. See DBND-501. |
| 6 | `:69` | `And the resulting edition reopens and verifies from a fresh local store` | `core_owner_reopens` — `cucumber.rs:11546` | no | no | reads `observation.reopened`, written as the literal `true` at `:3550` — but only reachable after a real `Bundle::open` + `verify()` at `:3505-3510`, both of which return `Err` on failure. See DBND-507. |

**Neither parameter is consumed by a `OnceLock`.** Search: `OnceLock` in
`cucumber.rs` yields eight process-global acceptance verdicts at `:1119-1128`
(`CB4_ACCEPTANCE`, `CB5_CONSTRAINTS_ACCEPTANCE`, `CB5_COUNTS_ACCEPTANCE`,
`CB5_RECEIPTS_ACCEPTANCE`, `CB5_CATALOG_ACCEPTANCE`, `CB6_ACCEPTANCE`,
`CB7_ACCEPTANCE`, `CB10_ACCEPTANCE`). There is no `CB8_*` entry, and
`core_owner_scenario` (`:3361-3552`, read in full) references none of them.
`ProtocolWorld` is `#[derive(Debug, Default, World)]` (`:467`), so cucumber-rs
constructs a fresh World per scenario; no state crosses the fifteen rows.

---

## 2. Which rows are distinct executions

**All fifteen rows execute distinct bytes.** Each row builds its own
`Bundle<FsStore>` under `Cb7TempRoot::new("core-owner-{zone}-{operation}")`
(`:3383`), publishes its own fixture section in its own zone, and drives a
distinct `OwnerContentOperation` variant. Grouping by what actually differs:

### Group A — zone dispatch (3 × 5)
`<zone>` reaches `SectionSpec.zone` (`:3396`) and
`Bundle::owner_content_operation(zone, …)`. The three zones take genuinely
different production paths inside the crate: `Zone::Public` reads through
`Bundle::public_read` (`bundle.rs:1237`), `Zone::Circle` through
`resolve_clear` + `owner_current_section_key_with_kex` + `open_blob_v`
(`:1239-1248`), `Zone::Self_` through `self_resolve` + `zone_dk_with_owner_kex`
(`:1251-1257`). Distinct.

### Group B — the nine mutating rows (`create`, `edit`, `delete` × 3 zones)
Genuinely differential. Each asserts a *changed* post-state after a fresh
reopen: `create` → `read_section(zone,"projects/new") == "created body"`
(`:3512-3518`); `edit` → `"projects/note" == "after"` (`:3519-3526` region);
`delete` → `read_section(...).is_ok()` must be false (`:3527-3531` region).
`gamma_delta` must be exactly 1. This is the strongest part of the unit.

### Group C — the six non-mutating rows (`list`, `read` × 3 zones)
Not vacuous, but mislabelled by three of the six `Then`s.
- `gamma_delta` must be **0** — the vector's `journalized` is `false` for all
  six (`vectors/cb2-bundle-authority-flows.json`, the six `list`/`read`
  cases), and `:3538` compares it against the *measured* delta. So "every
  mutation is journalized" is, on these rows, the meaningful negative "these
  do not journalize". Not vacuous.
- The reopen asserts the body is still `"before"` (`:3517-3524`) — a real
  non-mutation check.
- But "the resulting edition" has no referent on these rows (DBND-507), and
  `public/list`, `public/read`, `circle/list` execute paths that never receive
  the owner capability at all (DBND-502).

### Group D — the three keyless rows
`public/list`, `public/read`, `circle/list`. **[source]**
`zone_entries_with_owner_kex` (`bundle.rs:1430-1443`) routes every zone except
`Self_` to `clear_zone_entries(zone)`, documented at `:1454` as "Reconstruct
typed public/circle display entries **without a content key**" — `owner_kex` is
not passed. `read_section_with_owner_kex` (`:1236-1237`) routes `Zone::Public`
to `Self::public_read(&self.store, display_path)` — `owner_kex` is not passed;
this is the same function RU-3's stranger scenario calls
(`features/d-bundle.feature:51-53`). These three rows are distinct executions
of production code, but they carry no owner-authority content.

---

## 3. Findings

### DBND-501 — P1 — "without consuming mandate counters" is asserted against a literal, and the protocol's own observable for the claim is never read

**Statement.** `core_owner_gamma` (`cucumber.rs:11528`) carries the only
occurrence of the mandate-counter claim in the whole feature. Its unique
assertion is `assert_eq!(observation.mandate_counter_delta, 0)` (`:11543`).
`CoreOwnerObservation.mandate_counter_delta` is written as the literal `0` at
`:3549` and is never computed from anything. It is `assert_eq!(0, 0)`.

**Evidence [source].**
- Search `mandate_counter_delta`, scope whole archive, all layers: 19 hits.
  Fifteen are the constant `0` inside `vectors/cb2-bundle-authority-flows.json`
  and one in its generator (`gen-cb2-bundle-authority-flows.py:133`). Three are
  Rust: the struct field (`cucumber.rs:308`), the literal
  (`cucumber.rs:3549`), and comparisons to the constant `0`
  (`cucumber.rs:3532`, `:10680`, `:11543`;
  `cb2_bundle_authority_flows.rs:175`). **No site computes it.**
- Search `delegated_count`, `max_consumptions`, `max_mutations`, `consumption`,
  scope `rust/crates/aithos-bundle/src/`: **zero hits.** The crate that owns
  `owner_content_operation` has no counter machinery, so nothing in the
  execution path could have been measured.
- The claim *is* observable, and the protocol says where.
  `spec/07-gamma.md:173`, verbatim to end of line: "`max_actions: N` ⇒ count
  entries whose `authorized_via` **contains** this mandate id". `:404`,
  verbatim: "no leaf (owner-signed entries carry no `authorized_via` and feed
  no leaf)." `:111`, verbatim: "- **Owner** entries: signed by `content_sign`
  (§01.1); no `authorized_by`. The". Mandate counters are counted off
  `authorized_via` on Gamma entries.
- `spec/04-mandates.md:1861`, verbatim to the end of the cell: "| Owner | Local
  narrow capability; operation is authorized without a mandate, journalized,
  and consumes no mandate counter or constraint. | Verify owner signature,
  canonical operation, Gamma, changeset, and state transition; never synthesize
  a mandate consumption. |" And `:313`, verbatim to the end of the sentence:
  "Owner-local operations are journalized where required but increment neither
  delegated counter."
- The enforcement function exists: `aithos-core/src/gamma.rs:494`,
  `pub fn verify_owner_entry`, doc comment `/// Owner entry check (§07.2): no
  mandate fields, `#content` signature.`, body `:496-498`: `if
  entry.authorized_by.is_some() || entry.authorized_via.is_some() { return
  Err(err("owner entries carry no mandate")); }`.
- **`Bundle::verify()` never calls it.** `bundle.rs:1691-1790`, read in full:
  the Gamma section calls only `aithos_core::gamma::verify_links(&entries)`
  (`:1770`) and the `gamma_head` pin. `verify_links`
  (`aithos-core/src/gamma.rs:420-…`) calls `check_form` and link discipline;
  `check_form` (`:170-238`, read in full) inspects `v`, `id`, `at`, `kind`,
  `prevs`, `payload`/`body_enc` — **never `authorized_by` or
  `authorized_via`**, and never a signature.
- Search `verify_owner_entry`, scope `rust/crates/*/src/`: two hits — its own
  definition and `gamma_replay.rs:289`, reached from `log.rs:860`
  (`GammaReplayState::new` … `replay.admit(entry)`), a separate replay entry
  point that `Bundle::verify()` does not call. So neither the step, nor the
  observation, nor the reopen's `verify()` can see an owner mutation that
  carries a mandate chain.

**Failure scenario.** An owner mutation begins appending a Gamma entry that
carries `authorized_via: ["mandate_X"]`. Per `spec/07-gamma.md:173` that entry
now counts against `mandate_X`'s `max_actions`. `gamma_entries().len()` is
unchanged, `check_form` passes, `verify_links` passes, `Bundle::verify()`
passes, `mandate_counter_delta` is still the literal `0`. **All fifteen rows
stay green.**

**Severity note.** I rate this **P1** rather than P2 because the proof is
absent in the specific way the severity rubric names — the invariant has a
dedicated enforcement function that no verification path in this unit reaches,
so a real defect would ship past all fifteen scenarios. It downgrades to **P2**
if mutant M4 comes back RED, i.e. if some layer I did not find refuses it.
**[needs evidence_id: M4]**

**Closure criterion.** `core_owner_scenario` captures the Gamma entries
appended by the operation (it already holds `gamma_before`/`gamma_after`) and
asserts, for each appended entry: `entry.authorized_by.is_none() &&
entry.authorized_via.is_none() && entry.signature.key == "#content"` — or,
equivalently and better, calls `aithos_core::gamma::verify_owner_entry(entry,
&did_doc)` on it. `mandate_counter_delta` is then either derived from
`authorized_via` occurrences or deleted as a field.

---

### DBND-502 — P2 — three of the fifteen rows satisfy "succeeds from the narrow owner capability" on paths that never receive a capability

**Statement.** For `public/list`, `public/read` and `circle/list`, the executed
production code does not take `owner_kex`. The `Then` at `:67` is satisfied by
a keyless path — one that this same feature file elsewhere proves a *stranger*
can walk.

**Evidence [source].**
- `bundle.rs:1430-1443` `zone_entries_with_owner_kex`: `match zone { Zone::Self_
  => self.self_walk(&[], "", owner_kex, &mut out), _ =>
  self.clear_zone_entries(zone) }`. The `_` arm covers Public and Circle and
  **discards `owner_kex`**. Doc comment on `clear_zone_entries`, `:1454`,
  verbatim: "/// Reconstruct typed public/circle display entries without a
  content key."
- `bundle.rs:1236-1237` `read_section_with_owner_kex`: `match zone {
  Zone::Public => Self::public_read(&self.store, display_path), …` — the Public
  arm **discards `owner_kex`**.
- `features/d-bundle.feature:51-53` (RU-3) asserts a stranger with no key reads
  public content through the same surface. The two rules therefore assert the
  same executed behaviour on `public/read` while claiming opposite things about
  the capability.

**Failure scenario.** `owner_content_operation` stops consulting the owner
capability entirely — e.g. a refactor makes `Zone::Circle` and `Zone::Self_`
reads fall back to a clear index. `public/list`, `public/read` and
`circle/list` remain green with no observable difference, and the Rule keeps
claiming those three rows prove owner capability.

**Closure criterion.** Either (a) restate the `Then` so it does not claim a
capability on rows whose zone has no content key, or (b) add a negative control
to `core_owner_scenario`: run the same operation with an unrelated
`OwnerKeys` and require refusal — for the rows where refusal is the correct
answer — while explicitly recording, per row, which of the fifteen are
capability-bearing. Mutant M3 (§7) enumerates them.

---

### DBND-503 — P2 — "narrow owner capability" and "owner-local bundle session" are terms of art the unit never touches

**Statement.** No capability object and no session are constructed anywhere in
RU-5. `Bundle::owner_content_operation` (`bundle.rs:444-449`) takes `owner:
&OwnerKeys` — the whole private key set: `root_sign`, `content_sign`,
`owner_kex` (`aithos-core/src/keys.rs:28-38`). The crate's actual narrow-
capability surface is `rust/crates/aithos-bundle/src/session.rs`, whose header
comment reads, verbatim to the end of the sentence: "Capabilities are
intentionally non-serializable and non-cloneable. Their fields are private,
bind one local session id, and expose only typed protocol operations rather
than generic sign/open/wrap or raw-key access." It defines
`ManifestSigningCapability`, `GammaSigningCapability`, `BodyOpeningCapability`,
`HeaderWrappingCapability`, `AuditArgsCapability` (`session.rs:40-74`).

**Evidence [source].** Search `LocalSession`, scope
`core_owner_scenario` (`cucumber.rs:3361-3552`, read in full): zero hits. The
type is imported at `cucumber.rs:24` and used by other units. The `Given` at
`:64` announces "an owner-local bundle session" and its definition
(`:11484-11489`) stores a string.

**Failure scenario.** `owner_content_operation` starts requiring, or leaking,
raw key material beyond what the operation needs — e.g. a read arm that takes
`root_sign`. Nothing in RU-5 changes: it already passes everything.

**Closure criterion.** Either route the fifteen rows through `LocalSession` and
its typed capabilities so the word "narrow" has an executed referent, or delete
"narrow owner capability" from `:67` and let RU-7
(`features/d-bundle.feature:129`) own the narrowness claim alone. This finding
is adjacent to, but distinct from, RU-7's scope: RU-7 tests the capability
classes; RU-5 asserts the owner *content* surface uses them, and does not.

---

### DBND-504 — P2 — "journalized" is proved by cardinality alone

**Statement.** The only journal evidence in the unit is
`bundle.gamma_entries()?.len()` before and after (`cucumber.rs:3414-3419`,
`:3477-3480`) and the delta comparison at `:3537-3542`. `Entry`
(`aithos-core/src/gamma.rs:115-139`) carries `kind`, `target`,
`authorized_by`, `authorized_via`, `payload`, `body_enc`, `signature` — none is
read.

**Failure scenario.** An owner `edit` in the circle zone appends an entry with
`kind` of a `create`, or `target` naming a different node, or a sealed body
under the wrong node key. Count delta is still 1. All nine mutating rows stay
green. The step still reads "every mutation is journalized".

**Closure criterion.** Assert on the appended entry: `kind` matches the
operation (`Kind::…` per `spec/07-gamma.md`), and for the public zone
`target == node.to_string()` with a clear payload, for keyed zones
`target.is_none()` with `body_enc.is_some()` — the discipline `check_form`
already encodes at `gamma.rs:200-216` but does not tie to the operation that
produced it.

---

### DBND-505 — P2 — all three `Then` steps are aggregate-redundant with the `When`; deleting two of them changes nothing

**Statement.** Every substantive check in this unit lives inside
`core_owner_scenario`, which returns `Err` and is unwrapped-with-panic in the
*first* `Then` (`cucumber.rs:11511-11512`). The three `Then` step bodies add,
between them, one assertion not already made by the helper — and that one is
DBND-501's `assert_eq!(0, 0)`.

**Evidence [source], step by step.**
- `core_owner_succeeds` (`:11506`): `assert_eq!(observation.zone,
  w.core_owner_zone)` where `observation.zone = zone_name.to_owned()` at
  `:3545` and `zone_name` *is* `w.core_owner_zone` — a `Then` round-tripping on
  its own `When`. Same for `operation` (`:3546`). The third,
  `assert_eq!(observation.outcome, f(w.core_owner_operation))`, recomputes the
  exact mapping the helper already applied at `:3466-3477`, where a mismatch
  returns `Err`.
- `core_owner_gamma` (`:11528`): the `gamma_delta` half duplicates `:3538-3542`
  (helper compares the same measured delta against the vector). It retains
  residual value only against *joint* vector-and-production drift. The
  mandate-counter half is DBND-501.
- `core_owner_reopens` (`:11546`): asserts `observation.reopened`, the literal
  `true` at `:3550`, reachable only past `Bundle::open` (`:3505`) and
  `verify()` (`:3508`) which already `?`-propagate.

**Failure scenario.** A corrector rewrites `:67`–`:69` to say anything at all —
including something false — and the suite reports 15/15 green, because the
Gherkin text carries no independent verification weight. Predicted by mutant M2
(§7): deleting feature lines `:68` and `:69` leaves 15/15 green.

**Closure criterion.** Move at least one distinct, load-bearing assertion into
each `Then`, or collapse the three phrases into one `Then` that honestly says
what is checked. The Gherkin should not name three properties when the harness
checks them in one place and one of the three is a constant.

---

### DBND-506 — P3 — five of the six per-row fields of `owner_cases` have no behavioural consumer

**Statement.** `vectors/cb2-bundle-authority-flows.json` `owner_cases` carries
`id`, `zone`, `operation`, `expected`, `mandate_required`,
`mandate_counter_delta`, `journalized`, `fresh_store_reopen` per row. Only
`journalized` is ever compared against a measured value.

**Evidence [source], per field, search scope whole archive, all `.rs` and
`.py`.**
- `journalized` → `cucumber.rs:3529` → compared to the measured `gamma_delta`
  at `:3538`. **Real consumer.**
- `mandate_required`, `mandate_counter_delta` → `cucumber.rs:3532` compares
  them to the Rust literals `false` and `0`;
  `cb2_bundle_authority_flows.rs:174-175` does the same. Constant vs constant.
- `expected` → only `cb2_bundle_authority_flows.rs:172`,
  `assert_eq!(case["expected"], "accepted")`. Constant vs constant. RU-5's own
  path never reads it.
- `fresh_store_reopen` → only `cb2_bundle_authority_flows.rs:176`,
  `assert_eq!(case["fresh_store_reopen"], true)`. Never compared to
  `observation.reopened`.
- `zone`, `operation` → used as the lookup key at `cucumber.rs:3374-3380`.

`cb2_bundle_owner_parity_matrix_preexisting_green`
(`cb2_bundle_authority_flows.rs:163-183`) is named as a parity matrix and
asserts only that the JSON says what the JSON says, plus that the fifteen
`(zone, operation)` pairs are distinct. It is a vector-shape test.

**Failure scenario.** The vector is edited to declare `expected: "refused"` or
`mandate_required: true` for an owner row — a normative statement that owner
operations need a mandate. `cb2_bundle_authority_flows.rs:172-175` goes red on
the literal comparison, but nothing behavioural changes and RU-5's fifteen rows
are unaffected: the vector's normative content never reaches an execution.

**Closure criterion.** Either compare each field to a measured value —
`fresh_store_reopen` against `observation.reopened`, `expected` against the
accept/refuse verdict — or drop the fields and stop presenting the file as a
normative matrix.

---

### DBND-507 — P3 — "the resulting edition" has no referent on six rows, and "a fresh local store" is the same directory

**Statement.** `list` and `read` produce no edition; `core_owner_scenario`
reopens and verifies the *fixture's* edition. And "fresh local store" is a new
`FsStore::new(root.path())` handle over the same temporary directory
(`cucumber.rs:3505`), not an independently populated store.

**Evidence [source].** `owner_content_operation`'s `List` and `Read` arms
(`bundle.rs:452-458`) do no `transaction` and no `publish`. `:3505-3510`
reopens `root.path()` — the directory the fixture wrote.

**Assessment.** Not vacuous: the reopen genuinely round-trips through the
filesystem after `drop(bundle)` (`:3503`) and additionally asserts the body is
still `"before"` on those six rows. But the step name over-claims on 6/15 rows,
and "fresh" is weaker than a store populated from published bytes alone (the
kind of freshness `export_keyless`/`import_keyless`, imported at
`cucumber.rs:21`, would give).

**Closure criterion.** Split the phrase: on mutating rows keep "the resulting
edition"; on non-mutating rows say what is actually checked ("the bundle
reopens unchanged"). If "fresh local store" is meant to exclude the writer's
directory, use the keyless export/import path.

---

### DBND-508 — P3 — the two `Given`s announce state they do not construct

**Statement.** `core_owner_zone` (`:11484`) stores a string;
`core_owner_fixture` (`:11491`) sets one boolean. Neither creates a session,
a folder, a section or an edition. The publish the second one announces happens
inside the `When`, at `cucumber.rs:3389-3411`.

**Evidence [source].** Bodies read in full; `core_owner_fixture` is three
lines, one of them the assignment. `features/.agents/d-bundle/DOMAIN.md:373-374`
records the same observation ("`core_owner_fixture` (`:11491`, body sets one
boolean)"), so this is a known shape, not a discovery — I confirm it and state
its consequence.

**Consequence.** A failure to build or publish the fixture is reported as
`CORE-OWN-001 {zone}-{operation} fixture failed` from inside the `When`
(`:3412`) and surfaces at the *first `Then`*, attributed to the operation.
"Given" and "When" failures are indistinguishable in the report.

**Closure criterion.** Build the bundle in the `Given` and store it in the
World, or rename the `Given` so it does not claim a published state that does
not yet exist at that point in the scenario.

---

### DBND-509 — P3 — "durable parity across all three zones" is never checked comparatively, and "parity" is not a term of this protocol

**Statement.** No step and no helper compares one zone's observation to
another's. `ProtocolWorld` is `Default`-constructed per scenario (`:467`), so
the fifteen observations never coexist. Parity is asserted only extensionally:
fifteen independent rows share one predicate.

**Evidence [source].** Search `parity`, scope `spec/`, case-insensitive:
**zero hits.** Scope whole archive: 15 in `vectors/`, 10 in `rust/`, 10 in
`features/`, 4 in `docs/`, and the `INVENTORY.md` lines. In `rust/` the hits
are: `Cargo.toml:61` (lockfile parity, unrelated), three comment/label uses in
`cb9_delegated_content.rs`, the test name
`cb8_owner_grants.rs:106
cb8_owner_operation_surface_has_durable_parity_for_all_fifteen_pairs`, the file
header `cb8_owner_grants.rs:1` "CB8 durable owner parity and exact generic
grant delivery", `cb2_bundle_authority_flows.rs:163`, `:346`, and the section
comment `cucumber.rs:11482`. **Every one is a name or a comment; none is a
definition.**

**Closure criterion.** Either define what parity ranges over in `DOMAIN.md` or
the spec and check it (a comparative step: the three zones' observations are
equal on a named tuple), or retitle the Rule to what the fifteen rows prove.
See §4.

---

## 4. What "durable parity" turns out to mean

**Nothing normative. It is not a protocol term.**

- Search `parity`, scope `spec/` (all eleven files), case-insensitive: **zero
  hits.** The Rule's central word does not occur in the specification.
- Search `durable`, scope `spec/`: zero hits in the sense used here; the
  matches in the archive are `docs/` gateway/OAuth material ("state durable",
  `gateway-oauth-durable.feature`) and are a different subject.
- Neither word occurs in any *step* of `features/d-bundle.feature` — confirming
  the `INVENTORY.md` I1 flag rather than inheriting it. Search: `durable` and
  `parity` over the 165 lines of the file → only `:61`, the Rule title.

What the phrase operationally reduces to, from the code that executes:

1. **"parity"** = the same three-part predicate holds on each of fifteen
   `(zone, operation)` pairs, checked independently. The predicate is:
   (a) the typed outcome variant matches the operation and carries the expected
   content (`cucumber.rs:3466-3477`); (b) the Gamma entry *count* delta equals
   the vector's `journalized` flag (`:3529-3542`); (c) a fresh `Bundle::open` +
   `verify()` + content read-back succeeds (`:3505-3531`). Nothing compares one
   zone to another.
2. **"durable"** = (c), and only (c) — the effect survives dropping the
   `Bundle` and reopening the `FsStore`. This is the genuine, load-bearing part
   of the unit, and I could not break it (see §5).

The nearest thing to a definition anywhere is the Rust test name
`cb8_owner_grants.rs:106`, and the DOMAIN.md paraphrase at
`features/.agents/d-bundle/DOMAIN.md:21` ("local capability, without a mandate
and without consuming mandate counters"), which is itself a paraphrase of
`spec/04-mandates.md:1861` — a row that says nothing about parity across zones.
The normative content the Rule *does* have is that owner row, quoted verbatim
in DBND-501, and `spec/04-mandates.md:313`. Of its three clauses — authorized
without a mandate / journalized / consumes no mandate counter — the unit checks
the second by cardinality (DBND-504), and neither of the other two
(DBND-501, DBND-503).

---

## 5. What I attacked and could not break

1. **The cached-verdict hypothesis — the failure mode I was told to test
   first.** It does not apply to RU-5. Both `Examples` columns are typed step
   parameters (`cucumber.rs:11484`, `:11496`), both are stored in the World,
   both are passed to `core_owner_scenario` (`:3500-3503`), and both are
   destructured there into control flow that returns `Err` on an unknown value
   (`:3362-3370`). No `OnceLock` is in the path; no `".*"` regex; no shared
   process-lifetime verdict. `#[derive(Default, World)]` (`:467`) guarantees a
   fresh World per scenario. **[needs evidence_id: M1a, M1b]** to close.

2. **Fifteen scenarios, fifteen distinct fixtures.** Each row gets
   `Cb7TempRoot::new("core-owner-{zone}-{operation}")` (`:3383`) — no shared
   directory, no cross-row leakage.

3. **The nine mutating rows are differentially proved.** `create`, `edit` and
   `delete` each assert a *changed* post-state after a real filesystem reopen,
   with `delete` asserting non-readability. A production change that silently
   stopped applying the mutation would go red on all nine. This is real proof
   and I could not construct a defect it misses on the effect axis.

4. **The `journalized` vector field genuinely gates.** `:3529` reads it,
   `:3538` compares it to the *measured* Gamma delta. It is the one field of
   `owner_cases` with a behavioural consumer, and it makes the six
   `list`/`read` rows carry a real negative (reads do not journalize) rather
   than a vacuous one. Mutant M5 (§7) is the control.

5. **The reopen is a real filesystem round-trip.** `drop(bundle)` at `:3503`
   precedes `Bundle::open(FsStore::new(root.path()))` at `:3505`; no in-memory
   state carries. `verify()` (`bundle.rs:1691-1790`) walks every manifest in
   the chain, re-hashes every pinned file, checks I3 on pinned headers, refuses
   unpinned strays, and recomputes the Merkle and Gamma roots. This is a
   substantial verification and the unit gets its full strength for free.

6. **The published audits name nothing in my unit.** Search `CORE-OWN-001`,
   `owner parity`, `durable parity`, `mandate counter`, scope
   `docs/audits/` (`README.md`, `a-identity.md`, `b-derivation.md`,
   `c-headers.md`): **zero hits.** None of the three finished features'
   accepted audits names RU-5's steps, helper or vector.

7. **Of the seven recorded follow-ups, none lands in RU-5.**
   `features/.agents/d-bundle/STATE.md` items 1–7: `spec-cons-12` (spec
   consistency, lifted), `chdr-i3-d-bundle` (CHDR-034/030 — `Bundle::publish`
   and `Header::validate_as_owner`; RU-5 touches neither), `chdr-016-grant-path`
   (`grants.rs:739` — RU-5 issues no grant), `bder-006-d-bundle` (explicitly
   scoped by the accepted round-2 review to tag-view and wrap, and its named
   step coupling — `rename_the_folder`, `publish_edition`, `reads_at_new_path`
   — is RU-2's), `b-derivation-round-2-targeted` and `chdr-i3-targeted` (list
   the feature, content is items 4 and 2), `chdr-lota-vector-generators`
   (**conditional and it does bind**: RU-5 consumes
   `vectors/cb2-bundle-authority-flows.json`, whose generator
   `gen-cb2-bundle-authority-flows.py` **has** a `--check` mode at `:383-387`
   and is not among the nine without one — see the command in §7).

---

## 6. What I could not verify, and why

1. **Every behavioural prediction in this report.** I ran no command. DBND-501,
   DBND-502 and DBND-505 each state a predicted run outcome; each is marked
   **[needs evidence_id]** and each has a named mutant in §7. Until those are
   run, the findings rest on source reading only — which the method permits for
   absence claims with their search, but not for "the suite stays green".

2. **Whether `Bundle::verify()` has an indirect path to
   `verify_owner_entry`.** I read `bundle.rs:1691-1790` in full and searched
   `verify_owner_entry` across `rust/crates/*/src/` (two hits: its definition
   and `gamma_replay.rs:289`). I did not exhaustively trace whether any callee
   of `verify()` reaches `GammaReplayState`; `log.rs:860` is the only
   construction site and it sits in a separate `pub fn`. Mutant M4 settles it
   empirically. If M4 comes back RED, DBND-501 downgrades to P2 and I was
   wrong about the reach, not about the assertion being a literal.

3. **Whether `core_owner_scenario`'s three keyless rows would still pass with a
   wrong owner key** — DBND-502's failure prediction. I established from source
   that `owner_kex` is not passed on those paths (`bundle.rs:1237`, `:1441`);
   whether some *other* part of the row (the reopen's `verify()`, the read-back
   at `:3517`) catches a wrong key is what mutant M3 measures. The read-back at
   `:3517` uses `&owner` — the original — so I expect it not to catch it, but I
   did not prove it.

4. **Cross-feature interactions.** `cucumber.rs` is 20,040 lines with 19 step
   definitions cited by `INVENTORY.md` as sharing a cached verdict. I verified
   RU-5 uses none of the eight `OnceLock`s, but I did not audit whether another
   feature's step could execute between RU-5's steps. Cucumber-rs runs
   scenarios with a fresh World; concurrency across scenarios is an integration-
   pass question. `INVENTORY.md` and the d-bundle SKILL.md both route the
   `core_revocation_failure_boundary` cross-feature coupling to RU-6, not here.

5. **`chdr-016-grant-path`'s "which of the two carries it".** RU-5 touches no
   grant surface (`grants.rs` is not in the execution path;
   `GenericGrantRequest` appears only in the CB9 delegated helpers at
   `cucumber.rs:3554+`). I have no evidence from this unit that would let the
   cycle assign it, and assigning it is not this unit's call.

---

## 7. Commands I want run

Baseline first, then one mutant per line, each reverted before the next.
I need per-scenario and per-step counters from every run, not just the exit
code.

**B1 — baseline.**
```
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
```
Expect green. I need the reported scenario and step totals so the mutant runs
have something to differ from.

**B2 — vector regeneration baseline** (discharges the `chdr-lota-vector-generators`
condition for the one vector this unit touches).
```
python3 vectors/gen-cb2-bundle-authority-flows.py --check
```

**M1a — parameter reachability, zone.** `features/d-bundle.feature:87`
before: `        | self   | delete    |`
after:  `        | vault  | delete    |`
Predict **RED**, exactly one scenario failing, panic text containing
`CORE-OWN-001 unknown zone vault`. Fourteen rows still pass. This is the
control that proves `<zone>` reaches executing code.

**M1b — parameter reachability, operation.** `features/d-bundle.feature:73`
before: `        | public | list      |`
after:  `        | public | enumerate |`
Predict **RED**, one scenario, `CORE-OWN-001 unknown operation enumerate`.

**M2 — the two trailing `Then`s are not load-bearing (DBND-505).** Delete
`features/d-bundle.feature:68` and `:69` (the two `And` lines). Predict
**GREEN, 15/15**, with the step total dropping by exactly 30 (90 → 60) and no
other change. If green, the two Gherkin phrases carry no verification weight.

**M3 — which rows are capability-bearing (DBND-502).** In
`rust/crates/aithos-bundle/tests/cucumber.rs`, immediately before line 3420
(`let outcome = match operation {`), insert:
```rust
    let probe = OwnerKeys::genesis(
        &MasterSeed::from_slice(&[0x59; 32]).expect("probe seed"),
    );
```
then replace `&owner,` with `&probe,` at lines **3422** (inside the `List` arm,
the inline `…, &owner, &mut entropy)`), **3429**, **3442**, **3452** and
**3461** — the five `owner_content_operation` call sites only. Leave the
fixture (`:3400`), the reopen and every `read_section` on `&owner`.
Predict: `public/list`, `public/read`, `circle/list` stay **GREEN**; the other
twelve go **RED**. Any row that stays green is a row whose `Then` "succeeds
from the narrow owner capability" is satisfied without a capability.

**M4 — mandate-counter blindness (DBND-501).** In
`rust/crates/aithos-core/src/gamma.rs`, `owner_entry` at `:300-305`,
before: 
```rust
    let mut e = spec.into_entry("#content".to_owned());
    sign_entry(&mut e, content_sign)?;
```
after:
```rust
    let mut e = spec.into_entry("#content".to_owned());
    e.authorized_by = Some("mandate_passA_probe".to_owned());
    e.authorized_via = Some(vec!["mandate_passA_probe".to_owned()]);
    sign_entry(&mut e, content_sign)?;
```
Run **both**:
```
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml -p aithos-core --no-fail-fast
```
Predict: RU-5's fifteen rows **GREEN** (count delta unchanged; `check_form`
and `verify_links` never read these fields; `Bundle::verify()` never calls
`verify_owner_entry`), while `aithos-core`'s `f1_gamma.rs:187-188` goes
**RED**. That asymmetry is the finding. If RU-5 goes red, tell me which step
and with what message — DBND-501 then downgrades to P2 and I want the path I
missed.

**M5 — the one vector field with a consumer (DBND-506).** In
`vectors/cb2-bundle-authority-flows.json`, the `owner-public-create` case,
before: `      "journalized": true,`  after: `      "journalized": false,`
Predict exactly one scenario **RED**, message containing
`CORE-OWN-001 public-create Gamma delta 1`, and
`python3 vectors/gen-cb2-bundle-authority-flows.py --check` **RED**. Confirms
`journalized` gates and the generator's `--check` guards the file.

**M6 — optional, the `expected` field has no consumer (DBND-506).** Same file,
`owner-public-create`, `"expected": "accepted"` → `"expected": "refused"`.
Predict: RU-5's fifteen rows **GREEN** (the field is never read on that path);
`cb2_bundle_authority_flows.rs:172` **RED**. Confirms the field is a shape
assertion, not a normative case with an executor.

---

## 8. Disclosure gate

No finding in this report states an exploitable weakness for which no fix
exists. DBND-501 is the most severe and describes a *proof* gap in an alpha
with nothing deployed and no edition published
(`features/AGENTS.md`, § *Project stage*); production currently satisfies the
invariant (`owner_entry`, `gamma.rs:300-305`, sets neither mandate field), the
enforcement function already exists (`gamma.rs:494`), and the closure criterion
is a two-line assertion. Blocking condition 9 is **not** engaged. Nothing in
this unit is embargoed and the full statements are in this tracked file.

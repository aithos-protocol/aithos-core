# Correction run — `d-bundle`, lot A, round 1

| Field | Value |
|---|---|
| Feature | `d-bundle` |
| Role | corrector, `corrector/correct-d-bundle/SKILL.md` |
| Date | 2026-08-05 |
| Branch | `codex/fix-d-bundle-lot-a` |
| Base revision | `7b6fb9b` |
| Candidate revision | **not yet committed** — the orchestrator commits |
| Baseline gate | `ev-e62ef987`, GREEN, 1 feature / 7 rules / 51 scenarios / 299 steps |
| Lot | the seventeen `assigned_findings` of `STATE.md`: `DBND-001`, `-002`, `-003`, `-007`, `-008`, `-013`, `-014`, `-018`, `-019`, `-025`, `-026`, `-029`, `-031`, `-032`, `-033`, `-034`, `-040` |
| Gates run by this role | **none.** Every command below is named for the orchestrator |

## 0. What this lot is, and why there is no RED half

This lot is test semantics. No production behaviour in it is wrong; what is
missing is proof. A strengthened assertion over correct code is green the moment
it is written, so a green suite proves nothing about this work. Every claim of
necessity below therefore rests on a **named mutant, published as an applicable
unified diff, run twice** — once against the baseline tree without these changes
(the old assertion must be **green**, which is what shows it blind) and once
with them (the new assertion must be **red**, which is what shows it catches).

Nine of the sixteen mutants below are **reused, not invented**: they are the
transcripts the audit already holds, restated as patches so they can be re-run
and pointed. Where I reuse one I say so and cite the existing `evidence_id` as
the *without* half; the orchestrator still needs to run the *with* half.

**I formatted every hunk by hand and ran no formatter.** `cargo fmt --all --
--check` is likely red on layout alone. Please run
`cargo fmt --manifest-path rust/Cargo.toml --all` (write mode) before the
`--check` gate, and treat any resulting diff as formatting-only.

## 1. Files changed

| File | Why |
|---|---|
| `rust/crates/aithos-bundle/tests/cucumber.rs` | every assertion change in the lot |
| `features/d-bundle.feature` | **one line**, `:133`, for `DBND-019` — argued in §3 |

Nothing else. No production source, no vector, no `docs/audits/`, no `STATE.md`,
no `QUEUE.yaml`, no `PROCESS.md`, nothing under `features/.agents/orchestrator/`.

**No vector was touched**, so `chdr-lota-vector-generators` does not bind this
cycle and no `--check` is owed. No `sha256` in `vectors/ownership.json` moves and
there is no cross-repository cost from `cb2-draft2-carriers.json`.

**No production signature changed**, so the uncompiled `remote` feature
(`rust/crates/aithos-bundle/src/remote.rs`, in no gate) is unaffected. Nothing
in `aithos-core` changed, so the `wasm32-unknown-unknown` check is not owed by
the candidate — it **is** owed by three of the mutants (`M4`, `M4b`, `M8`,
`M8b`), which touch `aithos-core`; those are reverted before the final gates.

## 2. The counts

The contract still expands to **1 feature / 7 rules / 51 scenarios / 299 steps**.
The one feature-file edit replaces one `Then` line with one `Then` line: no
scenario, no row, no step is added or removed.

Three of the seventeen have a closure criterion the audit itself writes as a
count-moving change. I did **not** make those changes. They are proposed in §7
with their exact cost, for the orchestrator to rule on.

## 3. The one contract change, argued on its merits — `DBND-019`

`features/d-bundle.feature:133`, before and after:

```diff
-      Then the operation succeeds from the narrow owner capability without a mandate
+      Then the operation succeeds without a mandate, and the narrow owner capability is required exactly where the zone is keyed
```

**Why this is not a softening.** `DBND-019` is the finding that three of the
fifteen rows — `public/list`, `public/read`, `circle/list` — satisfy *"succeeds
from the narrow owner capability"* on paths that never receive a capability.
`zone_entries_with_owner_kex` (`bundle.rs:1430-1443`) routes every zone but
`self` to `clear_zone_entries`, whose own doc comment says *"without a content
key"*; `read_section_with_owner_kex` (`:1236-1237`) routes `Zone::Public` to
`public_read`, the exact function `features/d-bundle.feature:95` hands a
stranger with no key at all. Production is right and the sentence is wrong.

There is no code change that can make those three rows die under
`ev-b6a36f72`, because the mutant swaps the owner keys at the call sites and
those paths do not read a key. Option (b) of the audit's closure criterion —
"add the negative control … while recording, per row, which of the fifteen are
capability-bearing" — is implemented in full, but on its own it would leave
three rows asserting a false sentence. The audit's option (a) is
*"restate the `Then` so it does not claim a capability on rows whose zone has
no content key"*, and that is what this line does.

It is a `Given`-announces-one-thing-code-does-another defect of the class this
audit tracks, moved to match its fixture — not a contract weakened to spare an
implementation. The new sentence is **stronger**, not weaker: the old one was
satisfied by any success at all, the new one is falsified in *both* directions,
by a keyless row that starts demanding a key and by a keyed row that stops.
Closure per the audit is met: no row of `:129` both survives `ev-b6a36f72` and
asserts the old `:133`, because the old `:133` no longer exists.

**The counts do not move.** One `Then` line replaces one `Then` line.

## 4. Finding by finding

### `DBND-001` — `IMPLEMENTED`

Two edits, both required, both in the audit's closure criterion.

1. `wrong_predecessor` (`cucumber.rs`) removes the confound: it inserts
   `manifests/{height}.json` with its true `sha256_hex` into the forged
   manifest's `files` before signing. `all_pinned_files` excludes an edition's
   own manifest from its pins, so advancing the tip made the superseded manifest
   an **unpinned stray**, and the stray check rejected the edition whether or not
   the chain link was ever compared. The wrong `prev_hash` is now the only
   remaining cause of rejection.
2. `edition_rejected` asserts the error **identity**, not `is_err()`. The `When`
   that broke the edition declares the rejection it created
   (`w.expected_verify_error`); the `Then` requires the produced error to contain
   it. That discharges `fails closed` — a claim about the failure *mode* — which
   `is_err()` could not distinguish from any other rejection, and which the
   audit folds into this finding rather than numbering separately.

Mutant: **M1**, reused (`ev-d1fc33b5`, GREEN 51/51 without the change).

### `DBND-002` — `IMPLEMENTED`

`alter_pinned_file` now flips a byte **inside a sealed blob**,
`e/circle/blobs/<sid>.enc`, resolved from `e/circle/index.json` rather than
hard-coded, and `edition_rejected` asserts `pinned file altered: <that path>`.
The old tamper landed on `e/circle/index.json`, which `verify()` re-derives a
second time through the Merkle state-root recomputation (`state.rs:76`), so the
whole flat-pin loop could be deleted with the gate green. The flat pins were
kept beside the Merkle roots for exactly one reason — `manifest.rs:33-35`,
*"they still cover byte-rollback of sealed self blobs (§02.8)"* — and that is
now the file class the scenario tampers.

Mutants: **M2**, reused (`ev-de2706a8`, GREEN 51/51). Cross-check named in §6:
**M1** must leave `:40` green, so the two scenarios stop proving each other.

### `DBND-003` — `IMPLEMENTED`

The step was split first, as §8.1 of the audit requires: one function carried
**both** `#[then]` attributes across two Rules, and the correct assertion for one
is wrong for the other.

- Limb B, `edition_one_verifies_offline` (`:20`): asserts
  `latest_manifest().edition.height == 1` — the ordinal that was pure
  decoration — and then demonstrates *offline* the way spec 02.12 § *Keyless
  façade (G-D)* words it: every object is copied into a **fresh `MemStore`**,
  reopened as a new `Bundle`, and verified there, with no owner or grantee
  private capability anywhere. Nothing was exported and nothing reopened before.
- Limb A, `public_read_integrity_checks` (`:97`): the word *its* now has a
  referent. The step asserts the read succeeded, that the index row it resolved
  pins the bytes it returned, that the index carrying that pin is itself pinned
  by the signed manifest — and then **tampers** the public body and requires the
  same keyless read to be refused, restoring the bytes afterwards. The only
  integrity check the keyless read performs is `row.blob_sha != sha256_hex(&body)`
  (`bundle.rs:1280`), and deleting it moved nothing.

Mutant: **M3**, reused (`ev-c7f65638`, GREEN 51/51).

**Limb B is not covered by a mutant** and I say so rather than imply otherwise.
The audit records limb B as *on the record alone*; a mutant that would kill the
fresh-store reopen (e.g. making `Bundle::open` trust a cached value) has no
target in this code, because `Bundle::open` reads only the store. What limb B
gains is a real export/reopen where there was none; what it does not gain is a
transcript.

### `DBND-007` — `IMPLEMENTED` in code, `PARTIAL` against the audit's own criterion

`body_intact` keeps the round-trip assertion exactly as it was and adds beside it
the half the Rule's own name promised: it lists `e/circle/blobs/`, requires the
listing to be non-empty, concatenates the resident bytes and asserts that neither
`BODY` nor the section name, title or tag appears in them — the same shape
`inspect_self_zone` already used for `e/self/`.

Mutant: **M4 + M4b together**, reused (`ev-23aeba39`, RED 50/51 without the
change — the single casualty is `:109`, in a different Rule; **both RU-2
scenarios were green against a store that seals nothing**).

**What is not done.** The audit also asks for *"the corresponding `Then` line to
the Rule, so the contract carries the obligation and not only the code."* That
adds a step and moves the count to 300. Proposed in §7.1; not made.

### `DBND-008` — `IMPLEMENTED` in code, `PARTIAL` against the audit's own criterion

`rename_the_folder` captures the section's **sid** and **`blob_sha`** and its old
display path *before* the rename; `reads_at_new_path` then asserts all three of
the scenario's unobserved limbs: the sid is unchanged (spec 02.2, *"assigned at
creation, never changed"*), the `blob_sha` is byte-identical (spec 02.9,
*"re-keys nothing, moves no bytes"*), and the old display path **stops
resolving** (spec 02.2, *"unique among its siblings"*).

Mutant: **M5**, reused (`ev-f7261aa9`, GREEN 51/51).

**What is not done.** The audit asks that each new obligation be *"quoted by the
Gherkin line that carries it"*. Three new `Then` lines move the count. Proposed
in §7.2; not made.

### `DBND-013` — `IMPLEMENTED`

`inspect_self_zone` now accumulates the store **key** alongside the value — the
old body pushed the value and dropped the key — and its scope is the whole store
minus a **named, justified** allow-list of objects the protocol publishes in the
clear by design: `manifest.json`, `did.json`, `e/public/**`, `e/circle/**`,
`certs/**`. Spec 02.8 is the authority for the zone half, verbatim: a name is
*"Pure metadata: clear in the index for `public`/`circle`, sealed for `self`"*.
`gamma/gamma.jsonl`, `manifests/**` and every other object are now in scope, so
the Gherkin word *anywhere* means what it says.

I widened the audit's suggested allow-list by one entry, `e/circle/**`, and the
reason is normative rather than convenient: circle names are clear by the same
sentence that makes public names clear. The cost is stated: a self name that
somehow reached a **circle** index row would not be caught. It cannot —
`validate_store_key` is a closed allow-list and a self name is not a store key —
but the gap is real and named.

Mutant: **M6**, reused (`ev-f1718be8`, GREEN 51/51).

### `DBND-014` — `IMPLEMENTED`

The lower bound the audit asks for, tied to what the `Given` actually creates:
`inspect_self_zone` requires `e/self/index.json` among the inspected objects, at
least two `e/self/blobs/*` — the folder descriptor and the section — and at
least four self objects in total. `self_leaks_nothing` additionally refuses an
empty haystack before running its five `!contains`.

Mutant: **M7**, reused (`ev-0b4e1076`, GREEN 51/51 — *the scenario passes having
inspected nothing*).

### `DBND-018` — **P1** — `IMPLEMENTED`

`mandate_counter_delta` is no longer the literal `0`. `core_owner_scenario`
captures the **whole** `gamma_entries()` before and after, refuses a run that
rewrote the existing prefix, and takes the appended slice. From it:

- `mandate_counter_delta` is **counted**: the number of appended entries
  carrying a non-empty `authorized_via`. That is the protocol's own observable,
  `spec/07-gamma.md:173`, *"count entries whose `authorized_via` contains this
  mandate id"*. An owner entry that carried one would count against a mandate's
  `max_actions`, which is exactly the thing `:134` says never happens.
- every appended entry is put through
  `aithos_core::gamma::verify_owner_entry(entry, &did_doc)` — the dedicated
  enforcement function at `aithos-core/src/gamma.rs:494` that `Bundle::verify`
  never reaches, because `verify_links` calls only `check_form` and `check_form`
  reads neither `authorized_by` nor `authorized_via`. Its refusal is carried on
  the observation and asserted at `:134`.

Mutants: **M8**, reused (`ev-19a635cf`, RED 50/51 without the change — **all
fifteen RU-5 rows green**, the one casualty being in RU-7 on another clause),
and **M8b**, new and narrower.

**Honest limit, stated because skill rule 6 requires it.** M8 flips both new
assertions at once — the counted delta and `verify_owner_entry` — and no mutant
can separate them in that direction, because the counter counts `authorized_via`
and `verify_owner_entry` rejects `authorized_via`: they are two readings of one
predicate. I therefore claim them as **one gate, not two**. **M8b** stamps only
`authorized_by`, which the counter cannot see and `verify_owner_entry` does, and
it is the narrower mutant that separates that limb. I do not claim a mutant that
separates the counter from `verify_owner_entry` in the other direction; none
exists.

### `DBND-019` — `IMPLEMENTED`

The contract change is argued in §3. The code half:
`core_owner_scenario` runs a **negative control** — the identical operation,
against an identical fixture, driven by an unrelated `OwnerKeys` — and records
`stranger_refused`: true when the stranger cannot produce a valid edition,
meaning the call is refused *or* the bundle it leaves behind no longer verifies.
`core_owner_succeeds` asserts `stranger_refused == core_owner_row_is_keyed(zone,
operation)`, and the partition is declared in one place with the code that makes
it so cited beside it.

Mutant: **M9**, new — `owner_current_section_key_with_kex` falls back to a
zero base key when `open_owner_latest` fails, so a stranger's KEX opens keyed
content. That is a genuine confinement weakening and the new assertion catches
it on the keyed rows; the old assertion cannot.

I did **not** reuse `ev-b6a36f72` as this finding's mutant, because it is a
mutant of the harness, not of production, and because its outcome is already the
finding's evidence rather than its closure test.

### `DBND-025` — `IMPLEMENTED`

The scenario at `:199` carried the only line in the feature that mentions crash
recovery and induced no crash: its sibling outline has an injected-failure
`Given`, this one has none, so `core_atomic_recovery`'s
`assert!(observation.reopened)` said only *after a successful commit, reopening
yields the same bytes*. That is durability, not atomicity at the linearization
boundary.

A crash is now induced, on its own fixture, through a new `CoreAtomicFault`
variant. `CoreAtomicFault::Crash` fails the store's linearization call **and
swallows the rollback**, because a dead process unwinds nothing — that is the
whole difference between an orderly refusal, which the `:168` outline already
covers, and a crash, which nothing did. The reopen must then discover the
outcome for itself, and `core_atomic_recovery` asserts it resolved to **one
complete state**: `verify()` passes, the canonical manifest's `gamma_head` is
read explicitly and compared against the Gamma tree actually on disk rather than
inferred from map equality, the snapshot equals the complete old state, and
(FsStore) nothing of the dead attempt survives under `.aithos-generations/`.

A second variant, `CoreAtomicFault::AcknowledgementLost`, is added and wired into
the fault store — it lets the inner commit run to completion and only then
errors, which is `acknowledgement-lost` from
`vectors/cb2-bundle-boundaries.json → transaction.recovery_cases`. It is **not**
yet driven by a scenario; it exists so the second of the four normative recovery
states has a driver at all, and I flag it as unused-by-assertion rather than
claim it.

Mutant: **M16**, reused (`ev-7caa8332`, GREEN 51/51 — the whole `FsStore`
recovery path replaced by `self.transaction = None`).

**One divergence from the audit, and I think the audit is wrong here.**
`DBND-025`'s closure sentence says *"`ev-7caa8332` must turn at least one
scenario of `:89` red"*. `:89` in the audited numbering is the **failure**
outline; the finding's own scenario reference, three lines above, is `:116`, the
**success** outline, which is where the crash-recovery sentence lives. My change
turns a row of the success outline red under M16, not the failure outline. I
read `:89` as a slip for `:116` and say so rather than contort the fix to match a
line number.

### `DBND-026` — `IMPLEMENTED`

`CoreAtomicObservation` gains `staging_orphan_observed`, read off the **raw
tree** after the reopen: does `.aithos-generations/` still hold a generation
other than the one `.aithos-current` names. `core_atomic_staging_clean` asserts
it is false.

This is the observable `:176` is named for and the only one in the Rule that can
see it. `canonical_unchanged` compares three `cb7_store_snapshot` maps, and for
`FsStore` both `list` and `get` resolve through `canonical_base()`, which with no
transaction active returns the **active generation directory**. Everything the
sentence is about lives outside that range. The compatibility mirror does not
save it either: `collect_from` (`lib.rs:602-609`) skips every top-level component
beginning `.aithos-`, which is exactly where a leaked generation lives — the
fact the round-2 refutation verified as far as *"it calls `collect_from`"* and
stopped one function short.

Mutant: **M17 + M16 applied together**, reused — this is the pair `ev-f7ee3968`
(`RU-6.md` § M4 = M1+M3), GREEN 51/51 / 299/299, which the auditor named as this
finding's own closure test before any transcript existed. **I am reusing it, not
inventing.**

**Separation limit, per skill rule 6.** `M16` alone flips **only** the
`DBND-025` gate: in the failure outline the rollback still runs, the staging
directory is removed, and `staging_orphan_observed` stays false. So `DBND-025`
is separated by evidence. `DBND-026` is **not** separable in the other
direction — `M17` alone leaves the reopen's sweep intact and kills nothing, which
is precisely why the auditor specified a pair — so the mutant that demonstrates
`DBND-026`'s assertion necessarily also flips `DBND-025`'s. I claim one gate
cleanly separated and one demonstrated only by a mutant that flips both, and I
do not know of a narrower one.

**Ordering note the audit raises and I did not honour.** `DBND-026`'s closure
says a pre-fixture snapshot must exist, which is `DBND-023`'s closure criterion,
*"so `DBND-023` is done first"*. `DBND-023` is a P3, not in my lot, and touching
it is blocking condition 8. I did not need it: the snapshot is taken inside the
scenario helper, which builds its own fixture, so no `Given` had to change. The
dependency the audit states is real for the Gherkin-level fix and not for this
one.

### `DBND-029` — **P1** — `IMPLEMENTED`, and **the audit's closure criterion cannot be met at this tier**

`secret_material_exposed` is computed, at all four sites, by
`core_capability_secret_material_exposed`. Both halves of the clause are executed:

- *returned* — nothing the narrow operation produced may carry private material.
  Every row scans the bytes it actually produced (the draft.2 candidate, the
  signed Gamma entry, the opened plaintext, the serialized Header) for the
  relevant private keys in raw and hex form. Spec 01.6: *"MUST NOT expose private
  material as an output."*
- *accepted* — a session that holds no owner private material must be **refused**
  an owner-only narrow capability, not handed one built from substitute bytes.
  `LocalSession::grantee` is exactly such a session (`owner_kex: None`), and
  `header_capability()` / `audit_capability()` are the two mints that need it.
  Spec 01.6: *"Stable APIs MUST NOT require a raw seed or private key when the
  narrow operation suffices."*

Mutant: **M12**, new — `header_capability()` synthesises a substitute
`StaticSecret` instead of refusing a session that holds none.

**And now the part the audit got wrong.** `DBND-029`'s closure criterion ends
*"Closed when `ev-ed18d7ef` turns a row of `:131` red."* `ev-ed18d7ef` adds a
public `manifest_private_key()` accessor to `LocalSession` and calls it from
nowhere. **No runtime assertion in the Gherkin layer can ever catch that**: the
existence of an uncalled `pub fn` is compile-time information, there is no
reflection in Rust, and a harness cannot call a method that does not exist in the
tree it is compiled against. The only instruments that can are (a) a source-text
assertion — the class `DBND-032` condemns, in the same Rule, decided by the same
pass — or (b) a `trybuild` compile-fail test, which is a separate binary and
therefore turns *itself* red, never `:131`.

So the criterion as written is unsatisfiable, and its two escape routes
contradict each other across two findings of the same lot. What I have done is
the strongest honest thing available at this tier and it is a real improvement:
the assertion is no longer `assert!(!false)`, it is computed from two executed
observables, and M12 proves it catches a genuine weakening of the capability
surface. **It does not kill `ev-ed18d7ef` and I am not claiming it does.** §7.4
proposes the trybuild route, which does.

### `DBND-031` — `IMPLEMENTED` for its stated closure test, with a limit

The `<mismatched_object>` column now reaches executing code. Each row's executed
attempt **names** the object it presented (`CoreCapabilityObservation.
mismatched_object`), and `d_mismatched_capability_refused` asserts the Gherkin
cell equals that name before asserting the refusal. A cell naming something the
row never presents is a hard failure.

Mutant: **M13**, reused (`ev-3fa9d172`, GREEN 51/51 — the cell replaced by a
string that exists nowhere; control `ev-1eefbb66`, RED 50/51, on the neighbouring
`observable_result` cell of the same row).

**The limit, stated rather than papered over.** The audit's deeper ask — *"the
mismatched object is presented to the same capability handle, and the refusal is
distinguishable from the session-mismatch refusal"* — is met on **row `:241`
only**, the row the audit already credits, where `read_owner_section` refuses on
the node-path binding at `session.rs:275-282` before any store access. On rows
`:239`, `:240` and `:242` there is no runtime object-class refusal to compute,
because `CapabilityClass` and `SessionBinding` are private items and the guard at
`session.rs:234` is unreachable — which is `DBND-032`'s fourth limb, already on
the record. I refused to compute a boolean that would imply otherwise; instead
each of those three sites now carries, in the source, a statement of what its
refusal actually proves. Closing the rest needs a production change or a contract
restatement, and neither is in my lot.

### `DBND-032` — **NOT IMPLEMENTED**, deliberately

`cross_class_substitution_refused` is still `core_capability_api_is_narrow()`,
unchanged, and I want to explain why I left it alone rather than pretend.

The closure criterion says: *"Either (a) `:137`/`:138` are discharged
behaviourally, which requires a test-only path able to construct a wrong-class
binding, or (b) they are removed from the outline, the type-level argument is
written into `DOMAIN.md`, and a `trybuild` compile-fail case is added. **The grep
must not remain the deciding evidence in either case**, and `ev-794d59c3` must
turn `:131` red."*

- **(a) is out of my scope and against my domain rules.** A test-only path able
  to construct a wrong-class binding means adding a public or feature-gated
  constructor to `rust/crates/aithos-bundle/src/session.rs` — widening the very
  capability surface the finding is about. `correct-d-bundle/SKILL.md` § *Domain
  rules* forbids it in terms.
- **(b) moves the counts** (two `Then` lines removed from a four-row outline is
  −8 steps) and edits `DOMAIN.md`.
- **Widening the grep is not closure.** I drafted it and threw it away. Scoping
  it to every `src/*.rs` breaks immediately on `Bundle::open` (`bundle.rs:644`),
  a legitimate `pub fn open(`; normalising the name to catch `sign_any` breaks on
  `sign_owner_gamma_entry`, a legitimate narrow API. Every variant I could write
  is either false-positive on correct code or whack-a-mole on spellings — and all
  of them leave the grep as the deciding evidence, which the criterion forbids in
  both branches. It would have killed `ev-794d59c3` and proved nothing, which is
  precisely the failure mode this lot exists to remove.

So I have made this finding no better, and I say so. §7.4 proposes the route
that closes it.

### `DBND-033` — `IMPLEMENTED`

`CorePathObservation` gains `rejection_reason`, and
`core_path_refused_before_access` asserts, for the four `MemStore` display-path
rows, that the refusal came from the **confinement grammar** —
`validate_display_path` / `relative_segments` — and not from a lookup that
happened to miss. `.is_err()` could not tell the two apart: the fixture publishes
exactly one circle section, none of the four cells names it, and with the
validator neutered all four still failed inside `resolve_clear` on
`Error::InvalidPath("no folder …")`.

The audit asks for `io::ErrorKind`. That kind is **erased** on the way out:
`Bundle::gate_display_path` maps the `io::Error` through `io_err`
(`bundle.rs:338`) into `Error::SealRejected(format!("store i/o: {e}"))`, which
keeps the message and drops the kind. Asserting the message is the strongest
available form; recovering the kind would mean changing a production error type,
which is a cross-feature change and not in my lot. Stated so the reviewer does
not read the message assertion as laziness.

Mutant: **M14**, reused (`ev-2d2ebd1b`, GREEN 51/51, **all ten confinement rows
included**).

### `DBND-034` — **NOT IMPLEMENTED**

The closure criterion is *"At least two rows — one per store — with a valid input
and a `Then` asserting success, sharing the same step definitions."* Two new
`Examples` rows on a four-step outline is +2 scenarios and +8 steps: 53 scenarios
/ 307 steps. That is a count move and I did not make it. Proposed in §7.3.

I want to record that this finding is **not** closed by the `DBND-033` work even
though the two share `ev-2d2ebd1b`. The audit deliberately kept them separate
(§8.1, *"a merge deliberately not made"*) precisely so a corrector could not
close half and mark it done, and it was right: asserting the *kind* of refusal
does nothing about a suite in which every row asserts rejection.

### `DBND-040` — `IMPLEMENTED`

`core_owner_scenario` records the wire `kind` of every appended entry, and
`core_owner_gamma` asserts it equals the kind the operation under test must
produce — `section.add` for `create`, `section.modify` for `edit`,
`section.delete` for `delete` — as a per-row expected value driven from the
`<operation>` column of the `Examples` grid, never read back out of the entry.
Read and list rows must append nothing.

Mutant: **M15**, reused (`ev-f18d4843`, GREEN 51/51 / 299/299 — an owner `edit`
journalizing under a `create`'s kind).

I honoured the narrowing. Nothing here re-asserts the half the refuter falsified:
`check_form` **does** read `kind`, `target`, `payload` and `body_enc`, and a
circle entry naming a different node's `target` **is** rejected at write time in
`gamma_append`. Neither claim is made. What is asserted is the surviving centre —
`check_form` validates the entry's *shape* and never ties it to the operation
that produced it, so a well-formed lie passes.

**No vector field was added.** `vectors/cb2-bundle-authority-flows.json →
owner_cases` has no `kind` column and I did not add one, precisely to avoid a
re-pin and a `--check`; the expectation is a function of the Gherkin grid, which
is what the criterion asks for.

## 5. The mutants, as an ordered list of applicable patches

Every diff below applies with `git apply` against the **candidate** tree
(base `7b6fb9b` plus this correction). None touches a test file except `M13`,
which is a Gherkin cell and is the audit's own. **Revert each after its run.**

The *without* column is the transcript that already exists in the ledger for the
reused ones and shows the OLD assertion blind; the orchestrator still owes the
*with* half for every row.

### 1. `M1` — DBND-001

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::verify`
- **What it breaks:** the predecessor-hash comparison deleted from the chain loop
- **Predicted WITHOUT this correction:** **reused** — `ev-d1fc33b5`, GREEN 51/51
- **Predicted WITH this correction:** `:52` RED on the error identity; `:40` must stay GREEN

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..5c006eb 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -1724,9 +1724,7 @@ impl<S: Store> Bundle<S> {
                     }
                 }
                 Some(p) => {
-                    if m.edition.prev_hash != p.chain_hash()? {
-                        return Err(err(format!("broken chain at height {h}")));
-                    }
+                    let _ = p;
                 }
             }
             if !m.merges.is_empty() {
```

### 2. `M2` — DBND-002

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::verify`
- **What it breaks:** the whole flat-pin re-hash loop deleted
- **Predicted WITHOUT this correction:** **reused** — `ev-de2706a8`, GREEN 51/51
- **Predicted WITH this correction:** `:40` RED — the sealed blob is re-derived by nothing else

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..0802a42 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -1747,12 +1747,6 @@ impl<S: Store> Bundle<S> {
             return Err(err("manifest.json is not the chain tip".into()));
         }
         // Pinned files of the latest edition.
-        for (path, sha) in &latest.files {
-            let bytes = self.get(path)?;
-            if &sha256_hex(&bytes) != sha {
-                return Err(err(format!("pinned file altered: {path}")));
-            }
-        }
         // I3 (§00.2, §03.1, §09.4): every header this edition pins MUST carry
         // the owner line — the line whose recipient key is the subject's
         // owner_kex — in every key version. No key is needed to see it.
```

### 3. `M3` — DBND-003 limb A

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::public_read`
- **What it breaks:** the `blob_sha` guard deleted from the keyless read
- **Predicted WITHOUT this correction:** **reused** — `ev-c7f65638`, GREEN 51/51
- **Predicted WITH this correction:** `:93` RED — the tampered body is read successfully

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..f51810c 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -1280,11 +1280,7 @@ impl<S: Store> Bundle<S> {
             .iter()
             .find(|s| s.name == name)
             .ok_or_else(|| Error::InvalidPath(format!("no public section {name}")))?;
-        if row.blob_sha != sha256_hex(&body) {
-            return Err(Error::SealRejected(format!(
-                "public section {display_path} does not match its pinned hash"
-            )));
-        }
+        let _ = &row.blob_sha;
         String::from_utf8(body).map_err(|_| Error::SealRejected("not utf-8".to_owned()))
     }
```

### 4. `M4 + M4b` — DBND-007

- **File / symbol:** `rust/crates/aithos-core/src/seal.rs`, `blob_seal / blob_open`
- **What it breaks:** reduced to a length-preserving identity: a cleartext store. **Apply both patches together**
- **Predicted WITHOUT this correction:** **reused** — `ev-23aeba39`, RED 50/51, the single casualty in a DIFFERENT Rule (`:109`); both RU-2 scenarios GREEN
- **Predicted WITH this correction:** `:65` RED — `BODY` resident in `e/circle/blobs/`

```diff
diff --git a/rust/crates/aithos-core/src/seal.rs b/rust/crates/aithos-core/src/seal.rs
index 26d98cb..c188f9d 100644
--- a/rust/crates/aithos-core/src/seal.rs
+++ b/rust/crates/aithos-core/src/seal.rs
@@ -50,16 +50,10 @@ pub fn blob_aad(subject_did: &str, node: &str, key_version: u64) -> Vec<u8> {
 
 /// Seal a content blob under the node's key (§02.4).
 pub fn blob_seal(node_key: &[u8; 32], plaintext: &[u8], nonce: &[u8; 24], aad: &[u8]) -> Vec<u8> {
-    let cipher = XChaCha20Poly1305::new(node_key.into());
-    cipher
-        .encrypt(
-            XNonce::from_slice(nonce),
-            Payload {
-                msg: plaintext,
-                aad,
-            },
-        )
-        .expect("encryption is infallible for these sizes")
+    let _ = (node_key, nonce, aad);
+    let mut out = plaintext.to_vec();
+    out.extend_from_slice(&[0u8; 16]);
+    out
 }
 
 pub fn blob_open(
```

```diff
diff --git a/rust/crates/aithos-core/src/seal.rs b/rust/crates/aithos-core/src/seal.rs
index 26d98cb..2705df9 100644
--- a/rust/crates/aithos-core/src/seal.rs
+++ b/rust/crates/aithos-core/src/seal.rs
@@ -68,16 +68,8 @@ pub fn blob_open(
     nonce: &[u8; 24],
     aad: &[u8],
 ) -> Result<Vec<u8>> {
-    let cipher = XChaCha20Poly1305::new(node_key.into());
-    cipher
-        .decrypt(
-            XNonce::from_slice(nonce),
-            Payload {
-                msg: ciphertext,
-                aad,
-            },
-        )
-        .map_err(|_| Error::SealRejected("blob does not open".to_owned()))
+    let _ = (node_key, nonce, aad);
+    Ok(ciphertext[..ciphertext.len().saturating_sub(16)].to_vec())
 }
 
 fn kek(shared: &[u8; 32], epk: &XPublicKey, recipient: &XPublicKey) -> [u8; 32] {
```

### 5. `M5` — DBND-008

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::rename_folder`
- **What it breaks:** the rename appends an alias row; the old name survives and one sid carries two live display paths
- **Predicted WITHOUT this correction:** **reused** — `ev-f7261aa9`, GREEN 51/51
- **Predicted WITH this correction:** `:77` RED — the old display path still resolves

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..00e492d 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -1601,10 +1601,17 @@ impl<S: Store> Bundle<S> {
                     target = Some(f.sid.clone());
                 }
                 let sid = target.ok_or_else(|| Error::InvalidPath(display_path.to_owned()))?;
-                for f in &mut index.folders {
-                    if f.sid == sid {
-                        f.name = new_name.to_owned();
-                    }
+                let alias = index
+                    .folders
+                    .iter()
+                    .find(|f| f.sid == sid)
+                    .map(|f| crate::bundle::FolderRow {
+                        sid: f.sid.clone(),
+                        name: new_name.to_owned(),
+                        parent_sid: f.parent_sid.clone(),
+                    });
+                if let Some(alias) = alias {
+                    index.folders.push(alias);
                 }
                 self.put_json(&index_path, &index)
             }
```

### 6. `M6` — DBND-013

- **File / symbol:** `rust/crates/aithos-bundle/src/log.rs`, `Bundle::log_owner_mutation`
- **What it breaks:** the self zone logged like the public zone: the section name travels in clear inside the signed Gamma log
- **Predicted WITHOUT this correction:** **reused** — `ev-f1718be8`, GREEN 51/51
- **Predicted WITH this correction:** `:109` RED — a needle found in `gamma/gamma.jsonl`

```diff
diff --git a/rust/crates/aithos-bundle/src/log.rs b/rust/crates/aithos-bundle/src/log.rs
index 60d379f..cb7029f 100644
--- a/rust/crates/aithos-bundle/src/log.rs
+++ b/rust/crates/aithos-bundle/src/log.rs
@@ -198,7 +198,7 @@ impl<S: Store> Bundle<S> {
     ) -> Result<()> {
         let prev = self.gamma_head()?;
         let spec = match node.zone {
-            Zone::Public => EntrySpec {
+            Zone::Public | Zone::Self_ => EntrySpec {
                 id: self.next_gamma_id(ent),
                 prev,
                 prevs: None,
```

### 7. `M7` — DBND-014

- **File / symbol:** `rust/crates/aithos-bundle/src/lib.rs`, `MemStore::list`
- **What it breaks:** the `e/self` prefix invisible to listing; every byte left in place
- **Predicted WITHOUT this correction:** **reused** — `ev-0b4e1076`, GREEN 51/51
- **Predicted WITH this correction:** `:109` RED on the lower bound — the scenario can no longer pass having inspected nothing

```diff
diff --git a/rust/crates/aithos-bundle/src/lib.rs b/rust/crates/aithos-bundle/src/lib.rs
index 07db477..10f735b 100644
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -351,6 +351,7 @@ impl Store for MemStore {
             .visible_objects()
             .keys()
             .filter(|k| k.starts_with(prefix))
+            .filter(|k| !k.starts_with("e/self"))
             .cloned()
             .collect())
     }
```

### 8. `M8` — DBND-018

- **File / symbol:** `rust/crates/aithos-core/src/gamma.rs`, `owner_entry`
- **What it breaks:** every owner entry declares `authorized_by` AND `authorized_via`
- **Predicted WITHOUT this correction:** **reused** — `ev-19a635cf`, RED 50/51 with **all fifteen RU-5 rows GREEN**; the casualty is RU-7, another clause
- **Predicted WITH this correction:** at least one row of `:129` RED — counted `mandate_counter_delta` is 1 and `verify_owner_entry` refuses

```diff
diff --git a/rust/crates/aithos-core/src/gamma.rs b/rust/crates/aithos-core/src/gamma.rs
index 4ba25c4..2637817 100644
--- a/rust/crates/aithos-core/src/gamma.rs
+++ b/rust/crates/aithos-core/src/gamma.rs
@@ -299,6 +299,8 @@ impl EntrySpec {
 /// Owner entry (§07.2): signed by `content_sign`, no mandate attached.
 pub fn owner_entry(spec: EntrySpec, content_sign: &SigningKey) -> Result<Entry> {
     let mut e = spec.into_entry("#content".to_owned());
+    e.authorized_by = Some("mandate_x".to_owned());
+    e.authorized_via = Some(vec!["mandate_x".to_owned()]);
     sign_entry(&mut e, content_sign)?;
     e.check_form()?;
     Ok(e)
```

### 9. `M8b` — DBND-018, narrower

- **File / symbol:** `rust/crates/aithos-core/src/gamma.rs`, `owner_entry`
- **What it breaks:** every owner entry declares `authorized_by` **only**
- **Predicted WITHOUT this correction:** **new** — expected GREEN 51/51: the counter counts `authorized_via`, and nothing else in the feature reads `authorized_by`
- **Predicted WITH this correction:** at least one row of `:129` RED — `verify_owner_entry` refuses where the counter cannot see. This is the mutant that separates the two limbs

```diff
diff --git a/rust/crates/aithos-core/src/gamma.rs b/rust/crates/aithos-core/src/gamma.rs
index 4ba25c4..ed8b889 100644
--- a/rust/crates/aithos-core/src/gamma.rs
+++ b/rust/crates/aithos-core/src/gamma.rs
@@ -299,6 +299,7 @@ impl EntrySpec {
 /// Owner entry (§07.2): signed by `content_sign`, no mandate attached.
 pub fn owner_entry(spec: EntrySpec, content_sign: &SigningKey) -> Result<Entry> {
     let mut e = spec.into_entry("#content".to_owned());
+    e.authorized_by = Some("mandate_x".to_owned());
     sign_entry(&mut e, content_sign)?;
     e.check_form()?;
     Ok(e)
```

### 10. `M9` — DBND-019

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::owner_current_section_key_with_kex`
- **What it breaks:** a failed owner-line open falls back to a zero base key, so a stranger's KEX opens keyed content
- **Predicted WITHOUT this correction:** **new** — expected GREEN 51/51: the owner path is unaffected and no scenario runs a stranger against a keyed zone
- **Predicted WITH this correction:** the keyed rows of `:129` RED — `stranger_refused` is false where the partition says it must be true

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..e8eb30a 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -707,7 +707,9 @@ impl<S: Store> Bundle<S> {
             let Ok(header) = serde_json::from_slice::<Header>(&bytes) else {
                 continue;
             };
-            let (v, base) = header.open_owner_latest(&self.did, owner_kex)?;
+            let (v, base) = header
+                .open_owner_latest(&self.did, owner_kex)
+                .unwrap_or((KV, [0u8; 32]));
             let rest = NodePath {
                 zone,
                 folders: folders[depth..].to_vec(),
```

### 11. `M12` — DBND-029

- **File / symbol:** `rust/crates/aithos-bundle/src/session.rs`, `LocalSession::header_capability`
- **What it breaks:** a session holding no owner KEX is handed a capability built from a substitute secret instead of being refused
- **Predicted WITHOUT this correction:** **new** — expected GREEN 51/51: `secret_material_exposed` is the literal `false` at all four sites
- **Predicted WITH this correction:** the `wrap` row of `:227` RED — the keyless session is no longer refused an owner-only capability

```diff
diff --git a/rust/crates/aithos-bundle/src/session.rs b/rust/crates/aithos-bundle/src/session.rs
index 3c67413..7730266 100644
--- a/rust/crates/aithos-bundle/src/session.rs
+++ b/rust/crates/aithos-bundle/src/session.rs
@@ -207,14 +207,16 @@ impl<'a> LocalSession<'a> {
     }
 
     pub fn header_capability(&self) -> Result<HeaderWrappingCapability<'a>> {
+        static SUBSTITUTE: std::sync::OnceLock<StaticSecret> = std::sync::OnceLock::new();
         Ok(HeaderWrappingCapability {
             binding: SessionBinding {
                 id: self.id,
                 class: CapabilityClass::Header,
             },
-            owner_kex: self.owner_kex.ok_or_else(|| {
-                Error::InvalidSession("actor has no owner header capability".into())
-            })?,
+            owner_kex: match self.owner_kex {
+                Some(owner_kex) => owner_kex,
+                None => SUBSTITUTE.get_or_init(|| StaticSecret::from([0u8; 32])),
+            },
         })
     }
```

### 12. `M13` — DBND-031

- **File / symbol:** `features/d-bundle.feature`, `Examples row `:239``
- **What it breaks:** the `mismatched_object` cell replaced by a string that exists nowhere
- **Predicted WITHOUT this correction:** **reused** — `ev-3fa9d172`, GREEN 51/51. Control on the same row: `ev-1eefbb66`, `observable_result` replaced, RED 50/51
- **Predicted WITH this correction:** `:227` RED — the cell no longer names the object the executed attempt presented

```diff
diff --git a/features/d-bundle.feature b/features/d-bundle.feature
index ad4d6f0..0393002 100644
--- a/features/d-bundle.feature
+++ b/features/d-bundle.feature
@@ -236,7 +236,7 @@ Feature: Bundle and editions
 
       Examples:
         | capability | protocol_object                         | mismatched_object                       | observable_result                                |
-        | sign       | domain-tagged edition manifest          | Gamma entry                             | the signature verifies against the public key    |
+        | sign       | domain-tagged edition manifest          | a carrier that exists nowhere           | the signature verifies against the public key    |
         | sign       | domain-tagged Gamma entry               | edition manifest                        | the signature verifies against the public key    |
         | open       | node-and-version-bound sealed body      | body from a sibling node or version      | the expected plaintext is recovered only locally |
         | wrap       | node-version-and-recipient header line  | line for another node or recipient       | only the intended recipient opens the wrapped key |
```

### 13. `M14` — DBND-033

- **File / symbol:** `rust/crates/aithos-bundle/src/lib.rs`, `validate_display_path`
- **What it breaks:** reduced to `Ok(())`
- **Predicted WITHOUT this correction:** **reused** — `ev-2d2ebd1b`, GREEN 51/51, all ten confinement rows included
- **Predicted WITH this correction:** the four `MemStore` rows of `:256` RED — the refusal is `no folder ..`, not the confinement grammar

```diff
diff --git a/rust/crates/aithos-bundle/src/lib.rs b/rust/crates/aithos-bundle/src/lib.rs
index 07db477..73ce181 100644
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -87,12 +87,8 @@ fn relative_segments(value: &str) -> io::Result<Vec<&str>> {
 /// lowercase ASCII names separated by `/`, with no empty, dot, parent,
 /// absolute, backslash, or NUL form.
 pub fn validate_display_path(value: &str) -> io::Result<()> {
-    let segments = relative_segments(value)?;
-    if segments.iter().all(|segment| name_accepted(segment)) {
-        Ok(())
-    } else {
-        Err(invalid_path("display path contains an unsupported name"))
-    }
+    let _ = value;
+    Ok(())
 }
 
 fn manifest_stem_accepted(stem: &str) -> bool {
```

### 14. `M15` — DBND-040

- **File / symbol:** `rust/crates/aithos-bundle/src/bundle.rs`, `Bundle::section_rewrite`
- **What it breaks:** an owner `edit` journalizes under a `create`'s kind: `Kind::SectionModify` → `Kind::SectionAdd`
- **Predicted WITHOUT this correction:** **reused** — `ev-f18d4843`, GREEN 51/51 / 299/299
- **Predicted WITH this correction:** the `edit` rows of `:129` RED — the journal no longer names the operation that produced it

```diff
diff --git a/rust/crates/aithos-bundle/src/bundle.rs b/rust/crates/aithos-bundle/src/bundle.rs
index 15de876..51227c1 100644
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ -974,7 +974,7 @@ impl<S: Store> Bundle<S> {
         };
         self.log_owner_mutation(
             owner,
-            aithos_core::gamma::Kind::SectionModify,
+            aithos_core::gamma::Kind::SectionAdd,
             &node,
             serde_json::json!({ "blob_sha": sha }),
             now,
```

### 15. `M16` — DBND-025

- **File / symbol:** `rust/crates/aithos-bundle/src/lib.rs`, `FsStore::recover_transaction`
- **What it breaks:** the whole crash-recovery path replaced by `self.transaction = None`
- **Predicted WITHOUT this correction:** **reused** — `ev-7caa8332`, GREEN 51/51
- **Predicted WITH this correction:** the `FsStore` row of `:199` RED — the crashed attempt's staging generation survives the reopen

```diff
diff --git a/rust/crates/aithos-bundle/src/lib.rs b/rust/crates/aithos-bundle/src/lib.rs
index 07db477..d51febe 100644
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -904,70 +904,7 @@ impl Store for FsStore {
     }
 
     fn recover_transaction(&mut self) -> io::Result<()> {
-        self.rollback_transaction()?;
-        Self::ensure_plain_directory(&self.root)?;
-        let active = self.read_pointer()?;
-        let generations = self.generations_dir()?;
-        match std::fs::symlink_metadata(&generations) {
-            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
-                return Err(confinement_error(
-                    "transaction generations root is not a plain directory",
-                ));
-            }
-            Ok(_) => {
-                for entry in std::fs::read_dir(&generations)? {
-                    let entry = entry?;
-                    let name = entry
-                        .file_name()
-                        .to_str()
-                        .ok_or_else(|| invalid_path("generation name is not UTF-8"))?
-                        .to_owned();
-                    if !name.starts_with("generation-")
-                        || !name
-                            .bytes()
-                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
-                    {
-                        return Err(confinement_error(
-                            "unexpected object in transaction generations root",
-                        ));
-                    }
-                    if active.as_deref() != Some(name.as_str()) {
-                        Self::remove_internal_path(&entry.path())?;
-                    }
-                }
-            }
-            Err(error) if error.kind() == io::ErrorKind::NotFound => {
-                if active.is_some() {
-                    return Err(io::Error::new(
-                        io::ErrorKind::InvalidData,
-                        "transaction pointer references a missing generations root",
-                    ));
-                }
-            }
-            Err(error) => return Err(error),
-        }
-        for entry in std::fs::read_dir(&self.root)? {
-            let entry = entry?;
-            if entry.file_name().to_str().is_some_and(|name| {
-                name.starts_with(".aithos-current.tmp-")
-                    || name.starts_with(".aithos-mirror-current.tmp-")
-            }) {
-                Self::remove_internal_path(&entry.path())?;
-            }
-        }
-        if let Some(active_generation) = active {
-            let canonical = self.canonical_base()?;
-            if self.read_mirror_marker()?.as_deref() == Some(active_generation.as_str()) {
-                self.reconcile_compatibility_mirror(&canonical)?;
-            } else {
-                self.materialize_compatibility_mirror(&canonical)?;
-                self.write_generation_marker(
-                    &self.mirror_marker_path(),
-                    ".aithos-mirror-current.tmp",
-                    &active_generation,
-                )?;
-            }
-        }
+        self.transaction = None;
         Ok(())
     }
```

### 16. `M17 + M16` — DBND-026

- **File / symbol:** `rust/crates/aithos-bundle/src/lib.rs`, `FsStore::rollback_transaction and FsStore::recover_transaction`
- **What it breaks:** rollback leaks the staging generation permanently AND nothing sweeps it on reopen. **Apply both patches together** — this is the auditor's own M1+M3 pair
- **Predicted WITHOUT this correction:** **reused** — `ev-f7ee3968`, GREEN 51/51 / 299/299
- **Predicted WITH this correction:** at least one `FsStore` row of `:168` RED on `staging_orphan_observed`

```diff
diff --git a/rust/crates/aithos-bundle/src/lib.rs b/rust/crates/aithos-bundle/src/lib.rs
index 07db477..932354f 100644
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -897,9 +897,7 @@ impl Store for FsStore {
     }
 
     fn rollback_transaction(&mut self) -> io::Result<()> {
-        if let Some(transaction) = self.transaction.take() {
-            Self::remove_internal_path(&transaction.staging)?;
-        }
+        self.transaction = None;
         Ok(())
     }
```

```diff
diff --git a/rust/crates/aithos-bundle/src/lib.rs b/rust/crates/aithos-bundle/src/lib.rs
index 07db477..d51febe 100644
--- a/rust/crates/aithos-bundle/src/lib.rs
+++ b/rust/crates/aithos-bundle/src/lib.rs
@@ -904,70 +904,7 @@ impl Store for FsStore {
     }
 
     fn recover_transaction(&mut self) -> io::Result<()> {
-        self.rollback_transaction()?;
-        Self::ensure_plain_directory(&self.root)?;
-        let active = self.read_pointer()?;
-        let generations = self.generations_dir()?;
-        match std::fs::symlink_metadata(&generations) {
-            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
-                return Err(confinement_error(
-                    "transaction generations root is not a plain directory",
-                ));
-            }
-            Ok(_) => {
-                for entry in std::fs::read_dir(&generations)? {
-                    let entry = entry?;
-                    let name = entry
-                        .file_name()
-                        .to_str()
-                        .ok_or_else(|| invalid_path("generation name is not UTF-8"))?
-                        .to_owned();
-                    if !name.starts_with("generation-")
-                        || !name
-                            .bytes()
-                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
-                    {
-                        return Err(confinement_error(
-                            "unexpected object in transaction generations root",
-                        ));
-                    }
-                    if active.as_deref() != Some(name.as_str()) {
-                        Self::remove_internal_path(&entry.path())?;
-                    }
-                }
-            }
-            Err(error) if error.kind() == io::ErrorKind::NotFound => {
-                if active.is_some() {
-                    return Err(io::Error::new(
-                        io::ErrorKind::InvalidData,
-                        "transaction pointer references a missing generations root",
-                    ));
-                }
-            }
-            Err(error) => return Err(error),
-        }
-        for entry in std::fs::read_dir(&self.root)? {
-            let entry = entry?;
-            if entry.file_name().to_str().is_some_and(|name| {
-                name.starts_with(".aithos-current.tmp-")
-                    || name.starts_with(".aithos-mirror-current.tmp-")
-            }) {
-                Self::remove_internal_path(&entry.path())?;
-            }
-        }
-        if let Some(active_generation) = active {
-            let canonical = self.canonical_base()?;
-            if self.read_mirror_marker()?.as_deref() == Some(active_generation.as_str()) {
-                self.reconcile_compatibility_mirror(&canonical)?;
-            } else {
-                self.materialize_compatibility_mirror(&canonical)?;
-                self.write_generation_marker(
-                    &self.mirror_marker_path(),
-                    ".aithos-mirror-current.tmp",
-                    &active_generation,
-                )?;
-            }
-        }
+        self.transaction = None;
         Ok(())
     }
```

## 6. Gates I want run, in order

**I ran none of these.** Each is named exactly as `DOMAIN.md` § *Gate pyramid*
writes it.

### 6.0 Before anything, because I hand-formatted every hunk

```text
cargo fmt --manifest-path rust/Cargo.toml --all
```

### 6.1 The candidate, feature tier — the GREEN half

```text
bash features/.agents/scripts/verify-feature-tags.sh
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

Expected: **1 feature / 7 rules / 51 scenarios / 299 steps, green.** A different
count means the contract that ran is not the one I changed.

**This is the half that proves nothing about my work**, and I am naming it only
because a red here is a defect in my code, not evidence about the assertions.

### 6.2 The sixteen mutant pairs

For each row of §5, in order: apply the patch(es), run

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

against the candidate, journal it, **revert the patch**. For the nine reused
mutants the *without* half is already in the ledger and does not need re-running;
for `M8b`, `M9`, `M12` the *without* half must be run against `7b6fb9b` first.

`M4`, `M4b`, `M8` and `M8b` touch `aithos-core`. If any of them is left in the
tree when a global gate runs, the wasm check will see it. They are reverted.

### 6.3 Cross-feature — this is not optional and it is where I expect trouble

Six of the steps I changed are **co-owned with other features**, which is the
`bder-006-d-bundle` coupling the queue already records:

| Step | Also used by |
|---|---|
| `edition 1 verifies offline` | `k-integration.feature:23` |
| `edition verification is rejected` | `k-integration.feature:108`, `:111` |
| `one byte of a pinned file is altered` | `k-integration.feature:107` |
| `the newest manifest claims a wrong predecessor hash` | `k-integration.feature:110` |
| `I inspect every file of the self zone as a stranger` + `no folder name … appears anywhere` | `k-integration.feature:140-141` |
| `the folder … is renamed to …` / `the edition is republished` / `the owner reads the same section at …` | `b-derivation.feature:53-56` |

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @k-integration
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @b-derivation
```

**Where I expect a red, and what it would mean.** `k-integration.feature:140-141`
runs the widened self-zone search over the K bundle, whose self folder is named
`sante` — one of the five needles the d-bundle fixture hard-codes (`DBND-015`, a
P3 not in my lot, is exactly that hard-coding). If the K walkthrough leaves the
string `sante` in the clear anywhere outside the allow-list, that scenario goes
red. **If it does, do not assume my change is wrong**: the widened search is
what spec 02.8 § *anywhere* means, and a hit there is either a real self-structure
leak in the K path — a finding, not a defect of mine — or a fixture-vocabulary
collision. Hand me the transcript and I will say which.

### 6.4 Relevant regressions

```text
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb7_transaction_contracts --test cb8_owner_grants --test cb12_publication_package --test c3_owner_line_edition
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb2_bundle_boundaries --test cb2_bundle_authority_flows --test cb2_draft2_carriers --test cb2_bundle_version_coexistence --test cb2_store_key_consumer_neutrality
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb10_structure_vault --test cb13_concurrency_final --test i1_concurrency --test vectors_ownership
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-core --test c3_owner_line --test h1_merkle --test h2_gamma_roots --test f1_gamma
```

These should be untouched: nothing outside `cucumber.rs` and one Gherkin line
changed. `vectors_ownership` in particular must stay green, because no vector
moved.

### 6.5 Final global gates

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings
```

`cargo check -p aithos-wasm --target wasm32-unknown-unknown` is **not** owed:
the correction touches no `aithos-core` file.

One scope limit, recorded rather than discovered later: `QUEUE.yaml`'s
`chdr-lota-global-gate-resolution` (`ev-f818dc4b`) established that the global
Cucumber counter cannot distinguish a tree whose CB10 oracle checks nothing from
a correct one. That damage is unrepaired and it bounds how much §6.5's first two
commands prove about **this** work. The mutant pairs of §6.2 are the evidence
that does not have that problem.

## 7. What I did not do, and what it would cost

Four count-moving or scope-crossing changes. **The decisions are the
orchestrator's and the owner's, not mine.**

### 7.1 `DBND-007` — the Gherkin line the audit asks for

```gherkin
    Scenario: The owner reads back what was written
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner reads "projets/perso/note1" from circle
      Then the section body comes back intact
+     And the circle blobs resident in the store carry no clear body, name, title or tag
```

Cost: **299 → 300 steps.** The assertion already exists in the step body; this
only moves the obligation into the contract, which is what the audit means by
*"so the contract carries the obligation and not only the code."*

### 7.2 `DBND-008` — the three obligations, quoted by the lines that carry them

```gherkin
    Scenario: Display paths resolve through names, keys through sids
      Given a published bundle with section "note1" in circle "projets/perso"
      When the folder "perso" is renamed to "intime"
      And the edition is republished
      Then the owner reads the same section at "projets/intime/note1"
+     And the section keeps the sid and the sealed bytes it had before the rename
+     And a read at "projets/perso/note1" is refused
```

Cost: **299 → 301 steps.** Note this scenario's steps are co-owned with
`b-derivation.feature:50-56`, so the two new lines would be d-bundle-only while
the four existing ones stay shared — which is itself the `bder-006-d-bundle`
co-ownership record this cycle owes.

### 7.3 `DBND-034` — the positive control

Two `Examples` rows with a valid input plus a `Then` asserting success. On a
four-step outline that is **51 → 53 scenarios and 299 → 307 steps**, and it needs
a second `Then` sentence because the existing one asserts rejection. This is the
largest of the four and the one I am least willing to make unilaterally.

### 7.4 `DBND-029` and `DBND-032` — the `trybuild` route

Both findings' honest closure is the same instrument, and neither can be
discharged by a runtime assertion:

- remove `:234` and `:235` from the outline (**−8 steps**, 299 → 291);
- add `trybuild` as a dev-dependency of `aithos-bundle` and a compile-fail
  binary asserting that `session.manifest_private_key()`, direct field access to
  `manifest_key` / `gamma_key` / `owner_kex`, a wrong-class capability handoff,
  and `Clone`/`Serialize` on a capability struct all **fail to compile**;
- write the type-level argument (`CapabilityClass` and `SessionBinding` are
  private, the nine `self.check(…)` sites each pass the class their parameter
  type already fixes, so `session.rs:234` is unreachable) into `DOMAIN.md`;
- name the new binary in `DOMAIN.md` § *Focused tier*.

`trybuild` is not currently a dependency anywhere in the workspace and I did not
add it: a new dev-dependency needs a network fetch and is a decision above my
line. **This is the only route by which `ev-ed18d7ef` and `ev-794d59c3` become
red**, and until it is taken those two findings are open whatever else is done.

## 8. The three recorded debts — flagged, not absorbed

None of the three is in my lot and **I am closing none of them.**

### `chdr-028` — **my work touches its surfaces and does not discharge it**

`chdr-028` is a production defect: `verify_draft2_candidate`
(`publication.rs:469`) checks nothing on I3, so `verify_public_only` (`:586`),
`verify_for_cas` (`:643`) and `PublicationUploadPlan::verified` (`sdk.rs:35`)
accept a package pinning an I3-violating header where `cold_verify` refuses it.
Its closure criterion is a call to `verify_pinned_headers` with a RED test.

**Which of the three edition verifiers my work reached, since the skill requires
me to say:** `Bundle::verify` (`bundle.rs:1691`) only, and only as the *subject*
of new assertions — I added no guard to it and changed no line of it.
`publication::cold_verify` and the keyless package path
(`verify_public_only` / `verify_for_cas` / `verify_draft2_candidate`) are
**untouched**. So the divergence `chdr-028` names is exactly as wide after this
correction as before it, and my `DBND-003` limb B (a fresh-store reopen through
`Bundle::open` + `Bundle::verify`) does **not** exercise the keyless package
path at all. Nothing here narrows it and nothing here widens it.

### `chdr-016-grant-path` — **the ownership decision is yours, and here is the evidence**

The queue says *"whichever of `d-bundle` or `g-revocation` opens first states
which one carries it"*, and `d-bundle` is at position 2 against `g-revocation` at
9. `STATE.md` is explicit that the bootstrapper did not decide it and that
assigning scope without evidence is not bootstrapping. It is not mine either —
you said so, and the skill agrees.

What I can give you is the evidence, from having read the surfaces:

- the debt is about `Bundle::grant` (`grants.rs:739`) → `deliver_entry` (`:754`,
  body `:308-341`) → `add_line_on` (`:276-305`) implementing neither step 1 nor
  step 3 of spec 03.3, against `Session::append_header_recipient`
  (`session.rs:354-366`) as the conformant comparison;
- **no scenario of `features/d-bundle.feature` touches any of those symbols.**
  The word `wrap` appears four times in the file and never as a grant path:
  `:175` enumerates it among the artifacts a failed mutation must not leave, and
  `:242` is a row of the narrow-capability `Examples`. `grant` appears nowhere;
- `session.rs:354-366`, `append_header_recipient`, **is** in this feature's
  trace — it is the executed operation of the `wrap` row of `:227` — but as the
  *comparison* side of `CHDR-016`, not the defective side;
- the defective side, `grants.rs`, is reached by no step of this feature.

**My reading, offered as input and not as a decision:** the surface belongs to
`g-revocation`, because the grant path is where a grant is issued and cut, and
`d-bundle` exercises only the conformant comparison. `d-bundle` carrying it would
mean opening `grants.rs` with no scenario that reaches it.

### `bder-006-d-bundle` — **my work touches it and I am not closing it**

The debt is *"tag-view and wrap scenarios owed by the d-bundle cycle"*, narrowed
by the accepted round-2 review to "the zone-root view's coverage of the whole
zone, and an explicit « an anchor derives nothing downward » negative", plus the
**co-owned-steps record** (round-1 impact report §9.5), which is owed either way
and which the pending `BDER-006` re-arbitration does not affect.

I add nothing to the tag-view or wrap side. I **do** touch the co-owned steps the
record is about: `rename_the_folder` (`cucumber.rs:8956`), `publish_edition` (`:8859`) and
`reads_at_new_path` (`:13671`) all changed, and all three are `b-derivation`
steps too. §6.3 above is the co-ownership evidence in the form a record needs —
step, owner, other consumer — but the record itself belongs to whoever discharges
`bder-006-d-bundle`, and that is not this lot. The re-arbitration is the owner's.

## 9. Where I think the audit is wrong

Three places, and I would rather be told I am wrong than leave them unsaid.

1. **`DBND-029`'s closure criterion is unsatisfiable.** *"Closed when
   `ev-ed18d7ef` turns a row of `:131` red"* asks a runtime assertion to detect
   an uncalled `pub fn`. Nothing at that tier can. The finding's own escape
   hatch — a `trybuild` compile-fail test — turns a different binary red, never
   `:131`. And the only instrument that *could* make `:131` red on an added
   symbol is a source-text assertion, which is the class `DBND-032` condemns four
   findings later in the same Rule. The two criteria contradict each other. §7.4
   is the reconciliation.

2. **`DBND-025`'s closure line names the wrong scenario.** The finding's body
   says scenario `:116` (the success outline, where the crash-recovery sentence
   lives); its closure sentence says *"`ev-7caa8332` must turn at least one
   scenario of `:89` red"*, and `:89` is the failure outline. I read it as a slip
   and fixed the scenario the sentence is about.

3. **`DBND-026`'s stated ordering dependency does not hold for the code fix.**
   *"a pre-fixture snapshot must exist, which is `DBND-023`'s closure criterion,
   so `DBND-023` is done first"* — true if the fix is made in the Gherkin
   `Given`, false here: `core_atomic_failure_fs` builds its own fixture and can
   snapshot the raw tree without any `Given` changing. Since `DBND-023` is a P3
   deliberately not assigned, a corrector obeying that sentence literally would
   have had to breach blocking condition 8 to close a P2. Worth striking.

One thing the audit got right that I want to record, because I tried to break it
and could not: **`DBND-034` really is separate from `DBND-033`**, and §8.1's
"a merge deliberately not made" was the right call. I closed `DBND-033` without
moving `DBND-034` one inch, which is exactly the outcome the split was designed
to make visible.

## 10. Handoff

- **Tree is clean of every mutant.** No mutant is committed, and none is present
  in the working tree: `git status --short` shows only
  `features/d-bundle.feature` and
  `rust/crates/aithos-bundle/tests/cucumber.rs` modified. Each patch in §5 was
  produced by applying the edit, capturing `git diff`, and immediately running
  `git checkout --` on the file.
- **No vector re-pin.** No file under `vectors/` was touched, so no
  `vectors/ownership.json` digest moves, no generator `--check` is owed, and
  there is no cross-repository cost from `cb2-draft2-carriers.json`
  (`shared: true`, `service_consumers: [aithos-provider]`).
- **Findings move at most to `IMPLEMENTED`.** Fourteen of the seventeen:
  `DBND-001`, `-002`, `-003`, `-007`, `-008`, `-013`, `-014`, `-018`, `-019`,
  `-025`, `-026`, `-029`, `-031`, `-033`, `-040`. **Not** `DBND-032` and
  `DBND-034`, which stay `OPEN` with the reasons in §4 and the routes in §7.
  Nothing is marked `VERIFIED`.
- **Review requested from `audit-d-bundle`.**
- **`STATE.md` not edited.** The skill's handoff step says to set it to
  `REVIEW_REQUESTED` with the baseline and candidate revisions; the orchestrator's
  instruction for this round forbids editing `STATE.md`, and the orchestrator's
  instruction wins. The values it needs: `status: REVIEW_REQUESTED`,
  `base_main: 7b6fb9b`, `candidate_revision: <the commit you make>`,
  `branch: codex/fix-d-bundle-lot-a`.
- **Disclosure gate: not engaged, and assessed rather than inherited.** Nothing
  in this report or in any mutant states an exploitable weakness for which no fix
  exists. `M12` and `M9` describe real weakenings, but each is a patch to a crate
  an attacker would already need write access to, each is accompanied by the
  assertion that catches it, and the property holds in the unmutated tree.
  `CHDR-028`, `SC-12` and the code half of `SC-05` were published in full by
  owner ruling on 2026-08-04 and are cited freely, not re-embargoed. **Nothing is
  withheld and I am raising nothing separately.**

---

## 11. Addendum — re-emission after the runner incident, 2026-08-05

`rust/crates/aithos-bundle/tests/cucumber.rs` was reverted to `HEAD` by the
orchestrator's mutant runner, which reverted with `git checkout -- rust/` against
a baseline that was `HEAD` **plus** this uncommitted correction. The orchestrator
stated the error plainly and it is recorded here because the class matters: a
tool made a claim about state that did not match what was there, which is the
defect class this audit exists to find.

**The work is re-emitted in full.** Every one of the forty hunks was replayed as
an exact string replacement with the anchor asserted to match **exactly once**;
a drifted or duplicated anchor would have aborted the replay rather than
silently half-applying. Post-checks: brace/paren/bracket deltas all zero; every
new struct field both written and read (no dead-code lint); every new helper
defined and called; the `#[then]` attribute byte-compared against
`features/d-bundle.feature:133`; `104` deletions, identical to the destroyed
version.

**What is reconstructed rather than recalled, per the orchestrator's request.**
The *semantics* of every hunk are recalled exactly. What differs from the
destroyed version is **line wrapping in eleven places** — `alter_pinned_file`'s
`from_slice` call, `wrong_predecessor`'s `format!`, `inspect_self_zone`'s
`CLEAR_PREFIXES.iter().any(…)`, `public_read_integrity_checks`'s `let index:
ZoneIndex`, `core_owner_stranger_refused`'s signature, both owner `Then` asserts,
`core_atomic_state_is_complete`'s `map_err`, `core_atomic_staging_orphan`'s
`format!`, `core_path_refused_before_access`'s `GRAMMAR_REFUSALS.iter()`, and the
`wrap` row's `secret_material_exposed:`. I wrote these in the shape `cargo fmt`
produced last time (`ev-e3b0c442`, green) rather than in my original hand
formatting, which is why the insertion count is 1043 against 1049. **No
behaviour differs.** Nothing else is reconstruction.

### A second contamination, found during re-emission

**`M13` was still live in the tree.** The runner's revert was scoped to `rust/`,
so the `M13` mutant — the `mismatched_object` cell of
`features/d-bundle.feature:239` replaced by `a carrier that exists nowhere`,
run as `ev-2dab8b1a` — was **never reverted**. I found it because `M13.patch`
stopped applying, and I have reverted it: the feature file's only difference from
`7b6fb9b` is again the single `DBND-019` line at `:133`.

**This needs checking in the ledger, not by me.** Any gate journalled after
`ev-2dab8b1a` ran against a tree carrying a live Gherkin mutant. Please confirm
the seven green transcripts (`ev-69cb6844`, `ev-da59b0a8`, `ev-26f960b4`,
`ev-d60e3925`, `ev-52bf79f8`, `ev-e3b0c442`, `ev-25124cf1`) are all
timestamped **before** `ev-2dab8b1a`. If any is after, it is contaminated and
must be re-run. **Revert scope must be the whole worktree, not `rust/`** — this
lot's mutants touch `features/` (`M13`), `rust/crates/aithos-core/`
(`M4`, `M4b`, `M8`, `M8b`) and `rust/crates/aithos-bundle/`.

### Why `M3`'s patch was corrupt on extraction, and the fix

`error: corrupt patch at line 17` is not a defect in the patch. Line 17 of
`M3.patch` is a **blank context line**, which in unified-diff format is a single
space `" "`. Extracting it from the Markdown of §5 stripped that trailing space
to `""`, and `git apply` rejects a context line with no leading marker. **Eight
of the seventeen patches carry such a line** — `M3`, `M4`, `M4b`, `M12`, `M13`,
`M14`, `M16`, `M17` — so seven more were waiting to fail the same way.

The fix is to stop round-tripping through Markdown. Every patch is now on disk at
**`features/.agents/d-bundle/corrector/runs/mutants/*.patch`**, and each has been
verified against the re-emitted tree with `git apply --check`; both pairs
(`M4`+`M4b`, `M17`+`M16`) verified to compose. Apply from those files, not from
§5. The §5 diffs remain as the human-readable record.

### `ev-471decd3` answered — this is the M1 with-patch half, and it is correct

Yes: the single failure is exactly the one `M1` predicts, and the transcript
carries the cross-check as well.

- **51 scenarios, 50 passed, 1 failed; 299 steps, 298 passed, 1 failed.**
- The failure is `features/d-bundle.feature:55`, `Then edition verification is
  rejected`, inside `Scenario: A broken chain fails closed`, panicking on
  `the edition must be rejected`. With the predecessor-hash comparison deleted
  **and** the `manifests/2.json` confound removed, `verify()` returns `Ok`, and
  the new `expect_err` fires. That is `DBND-001`'s closure criterion, met.
- **The cross-check I named in §5 row 1 holds in the same transcript:**
  `Scenario: A tampered file fails the edition` (`:43`) is **green** under `M1`.
  The two scenarios have stopped proving each other — `M1` now kills the chain
  scenario and only the chain scenario.

Against `ev-d1fc33b5` (GREEN 51/51, the same mutant without this correction), the
pair is complete and `DBND-001` is proved. **This is the lot's first fully
measured finding.**

### The six void runs, confirmed void by a stronger fact

The identical `36 passed / 15 failed` across `ev-5e710193`, `ev-838d2fe8`,
`ev-98348822`, `ev-c7bd7f23`, `ev-2dab8b1a` and `ev-1be01535` is not merely
suspicious — the transcripts say why in terms. Every one of the fifteen failures
reads:

```text
Step failed:
Defined: root/work/aithos-core/features/d-bundle.feature:133:7
Step doesn't match any function
```

That is `fail_on_skipped()` (`cucumber.rs`, `fn main`) catching the orphaned
`DBND-019` Gherkin line after the step definition behind it was reverted away.
The mutants contributed nothing to those numbers. Journalling them as **void**
rather than deleting them is the right call and I concur with it.

### One counter I want checked against the fingerprint

`ev-471decd3` reports `Matched: crates/aithos-bundle/tests/cucumber.rs:13606`
for `edition_rejected`. After re-emission that symbol sits at a slightly
different line, because of the eleven wrapping differences above. **A line-number
difference in a `Matched:` trace is expected and is not drift.** The fingerprints
that must match exactly are the ones the orchestrator named: `@d-bundle` at
**1 / 7 / 51 / 299** and the full suite at **18 / 114 / 836 / 3577**.

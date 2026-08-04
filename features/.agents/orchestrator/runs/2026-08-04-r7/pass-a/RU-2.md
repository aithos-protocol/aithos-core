# Pass A — `d-bundle`, review unit `RU-2`

**Rule:** `Content round-trips through the sealed store` — `features/d-bundle.feature:32`
**Scenarios:** 2 authored / 2 expanded, 7 steps.
**Material:** `/root/work/passA-d-bundle/RU-2`, a `git archive` of `d9120d7`, no `.git`.
**Finding family:** `DBND-`. I cannot coordinate numbering with the six other Pass A
auditors, so every finding here is numbered **`DBND-2xx`**, as instructed.

**Evidence status.** I ran no gate, no test and no `cargo` command. Consequently
**this report contains no behavioural claim.** Every statement below is one of:
(a) a quotation of a file at a `file:line`, (b) a search with its exact scope, or
(c) a **prediction**, marked as such, attached to a named command in §7 that the
orchestrator must run before the prediction becomes evidence. No `evidence_id`
exists yet for this unit; each finding's *Evidence* block names the transcript it
still needs.

**What I deliberately did not open.** `features/.agents/d-bundle/STATE.md`,
§ *Current instruction*, forbids reading `docs/audits/`, `git` history or other
features' run reports before the Pass A result is frozen, and singles out
`docs/audits/features/c-headers.md` §6bis as Pass B material that "must not be
opened inside a Pass A review unit". I honoured that: I opened no file under
`docs/audits/`. The recorded follow-ups are quoted verbatim inside `STATE.md`
itself, which that same section states is "the only form in which a Pass A unit
may see them", and that is the form I used. I also did not open
`/root/work/aithos-core`.

---

## 0. The unit as written

```gherkin
32:  Rule: Content round-trips through the sealed store
33:
34:    Scenario: The owner reads back what was written
35:      Given a published bundle with section "note1" in circle "projets/perso"
36:      When the owner reads "projets/perso/note1" from circle
37:      Then the section body comes back intact
38:
39:    Scenario: Display paths resolve through names, keys through sids
40:      Given a published bundle with section "note1" in circle "projets/perso"
41:      When the folder "perso" is renamed to "intime"
42:      And the edition is republished
43:      Then the owner reads the same section at "projets/intime/note1"
```

All seven steps resolve to `rust/crates/aithos-bundle/tests/cucumber.rs`, the sole
step file the runner registers (`fn main`, `cucumber.rs:20017`). **None of the seven
is a proxy step.** Search: I read the body of each of the seven definitions
(`:7714`, `:8382`, `:12743`, `:8394`, `:8343`, `:12748`) and checked each against the
`OnceLock` cached-verdict idiom used elsewhere in the same file
(`CB7_ACCEPTANCE.get_or_init` at `:7340`, `CB10_ACCEPTANCE.get_or_init` at `:7349`).
No step of `RU-2` reads a cached verdict, a shared `*_observation` field, or an
`include_str!` source constant. Every one of the seven executes its own arrangement
or its own assertion against a live `Bundle<MemStore>`. The `chdr-lota-proxy-verdicts`
and `chdr-lota-source-text-assertions` classes named in `STATE.md` §8 do **not**
reach this unit. (They do reach `RU-7`; `STATE.md` §8 says so, and that is `RU-7`'s
auditor's business.)

There is no in-memory shortcut for the read either: `pub struct Bundle<S: Store>`
(`rust/crates/aithos-bundle/src/bundle.rs:283-286`) holds exactly two fields,
`pub store: S` and `pub did: String` — no cache, no staged tree. Every read in this
unit therefore does traverse serialized store bytes. That is a real property and I
record it in §4.

---

## 1. Scenario S5 — `The owner reads back what was written` (`:34`)

### The claim

The Rule and the scenario name together assert: content the owner wrote is
recoverable by the owner, unchanged, **through the sealed store**. `INVENTORY.md`
restates it neutrally as "Content written by the owner is recoverable by the owner
unchanged" (`INVENTORY.md:153`) — a restatement that, correctly, drops the word
*sealed*, because the scenario name does not carry it. The Rule name does.

### What executes

**`Given a published bundle with section "note1" in circle "projets/perso"`**
→ `published_with_section`, `cucumber.rs:7714-7719`.

```rust
#[given(expr = "a published bundle with section {string} in circle {string}")]
fn published_with_section(w: &mut ProtocolWorld, name: String, folder: String) {
    w.init_bundle();
    w.add_circle_section(&folder, &name, "toto");
    w.publish_bundle();
}
```

The two captures are correctly ordered against the Gherkin (`"note1"` → `name`,
`"projets/perso"` → `folder`) and correctly re-ordered at the call site. The
arrangement is genuinely in the `Given`: `init_bundle` (`:7358`) builds a real
`Bundle::init` over `MemStore`; `add_circle_section` (`:7374-7396`) calls
`ensure_folder` then `section_add` with `body: BODY`; `publish_bundle` (`:7397-7400`)
calls `Bundle::publish`. This is **not** the "announces one state and constructs
another" failure mode: what it announces is what it builds. The tag `"toto"` is
hard-coded rather than announced, which is harmless here because no step of `RU-2`
reads a tag.

**`When the owner reads "projets/perso/note1" from circle`**
→ `owner_reads_circle`, `cucumber.rs:8382-8392`. It calls
`Bundle::read_section(Zone::Circle, path, owner)` and stores the `Result` in
`w.read_body`, mapping the error to a `String`. The error is preserved, not
swallowed.

The production path is real and traverses the store. `read_section`
(`bundle.rs:1220-1226`) delegates to `read_section_with_owner_kex`
(`:1230-1259`), whose `Zone::Circle` arm:

1. `resolve_clear` (`:1193-1217`) reads `e/circle/index.json` **from the store**
   (`get_json` → `store.get`, `:405` → `:398`) and walks the display path
   `projets` → `perso` matching `f.name == seg && f.parent_sid == parent`,
   yielding a chain of **sids** and the section row;
2. `owner_current_section_key_with_kex` (`:694-725`) reads the governing header
   from the store and derives the section key from the sid chain;
3. `open_blob_v` (`:540-555`) reads `e/circle/blobs/{sid}.enc` from the store,
   splits the 24-byte nonce and calls `blob_open`
   (`rust/crates/aithos-core/src/seal.rs:65-81`, XChaCha20-Poly1305 with AAD
   `purpose ‖ did ‖ canonical sid-path ‖ key_version`);
4. it parses the plaintext as JSON and returns `v["md"]`.

**`Then the section body comes back intact`** → `body_intact`,
`cucumber.rs:12743-12746`:

```rust
assert_eq!(w.read_body.as_ref().unwrap().as_deref(), Ok(BODY));
```

`BODY` is a module constant, `cucumber.rs:68`:
`"Le corps de la note, ephemere et precieux."`.

### Do the two meet?

**Partly, and the gap is exactly the Rule's own adjective.**

What the assertion does establish, and this is worth stating because the train's
first named failure mode is precisely the opposite: the `Then` does **not**
round-trip on its own `When`. It compares against a *literal constant* fixed at
`cucumber.rs:68`, not against a value re-derived from the `When`. Had it read
`assert_eq!(read, seal_then_open(read))` the claim would be empty; it does not. The
comparison is anchored, the write and the read are separate calls, and the byte
path between them is a serialized store. So "content is recoverable unchanged" —
the scenario's own name — **is** demonstrated.

What is **not** demonstrated is the Rule's word **sealed**. Nothing in this
scenario, and — search below — nothing anywhere in `features/d-bundle.feature`,
distinguishes a sealed circle store from a cleartext one. The assertion is
satisfied by `open(seal(x)) == x` *for any pair of mutually inverse functions*,
including the identity pair. Sealing is load-bearing for the Rule's name and is
untested by the Rule's scenarios. That is `DBND-201`.

Two further narrownesses, both real, neither fatal:

- The word **published** in the `Given` is inert for this scenario's assertion.
  `read_section` needs `e/circle/index.json`, the zone header and the blob — all
  three written by `init_bundle` and `add_circle_section`, none by
  `publish_bundle`. `Bundle::init` publishes edition 1 through `publish_at`
  directly (`bundle.rs:629`, reached from `init`), not through `publish`
  (`:1678-1681`), so even the first manifest does not depend on the `Given`'s third
  line. This is `DBND-203`, and it is the same coupling the recorded follow-up
  `bder-006-d-bundle` already names.
- There is **no negative control anywhere in this Rule**. Every assertion of `RU-2`
  is positive. The feature's only tamper scenario, `A tampered file fails the
  edition` (`:24`, `RU-1`), flips a bit of `e/circle/index.json`
  (`alter_pinned_file`, `cucumber.rs:8349-8355`) — never a blob. No scenario in the
  file substitutes, truncates or replays a circle blob. This is `DBND-205`.

---

## 2. Scenario S6 — `Display paths resolve through names, keys through sids` (`:39`)

### The claim

Two addressing layers exist and are independent: the human-visible path resolves by
**name**, the storage key resolves by **sid**, so renaming a name does not move the
underlying object. `INVENTORY.md:154` restates it the same way.

The normative ground, quoted verbatim to its end from
`spec/02-content-tree.md`:

> **§2.2, lines 36-38** — "**sid** — a ULID, globally unique, assigned at creation,
> **never changed**. The sid is the derivation label (§2.5) and the blob filename.
> Because keys hang off sids, renaming anything never re-keys anything."

> **§2.2, lines 39-41** — "**name** — the human segment (`enfance`, `cicatrices`,
> `1234`); `[a-z0-9_-]{1,64}`, unique among its siblings. Pure metadata: clear in
> the index for `public`/`circle`, sealed for `self` (§2.8)."

> **§2.9, lines 528-529** — "**Rename is free.** Names are metadata (§2.2):
> renaming a folder or section edits an index row / descriptor, re-keys nothing,
> moves no bytes."

The scenario's claim is therefore a conjunction of four normative obligations:
(i) display paths resolve names→sids; (ii) the sid is never changed;
(iii) rename re-keys nothing; (iv) rename moves no bytes. Plus the implicit
uniqueness obligation from §2.2 line 40, "unique among its siblings".

### What executes

**`Given`** — identical to S5, `cucumber.rs:7714`.

**`When the folder "perso" is renamed to "intime"`** → `rename_the_folder`,
`cucumber.rs:8394-8403`:

```rust
#[when(expr = "the folder {string} is renamed to {string}")]
fn rename_the_folder(w: &mut ProtocolWorld, name: String, new_name: String) {
    let owner = w.owner(0);
    let full = format!("projets/{name}");
    w.bundle.as_mut().unwrap()
        .rename_folder(Zone::Circle, &full, &new_name, &owner, &mut w.ent)
        .unwrap();
}
```

The parent segment `projets/` is **hard-coded in the step body**. The step's first
`{string}` is only ever the leaf segment. See `DBND-204`.

Production: `Bundle::rename_folder`, `bundle.rs:1571-1611`. Its non-`self` arm
loads `e/{zone}/index.json`, walks the display path by name to a sid, then:

```rust
for f in &mut index.folders {
    if f.sid == sid {
        f.name = new_name.to_owned();
    }
}
self.put_json(&index_path, &index)
```

The implementation is conformant: metadata only, sid untouched, no blob touched.
The doc comment at `:1570` even says so — "Rename a folder: metadata only, never
re-keys (§02.9)."

**`And the edition is republished`** → `publish_edition`, `cucumber.rs:8343-8347`.
One `fn` carrying two `#[when]` phrases (`I publish the edition` at `:8343`,
`the edition is republished` at `:8344`); body is `w.publish_bundle()`.

**`Then the owner reads the same section at "projets/intime/note1"`**
→ `reads_at_new_path`, `cucumber.rs:12748-12758`:

```rust
let body = w.bundle.as_ref().unwrap()
    .read_section(Zone::Circle, &path, &owner)
    .expect("readable at the renamed path");
assert_eq!(body, BODY);
```

### Do the two meet?

**No. The demonstration reaches one of the four obligations and is blind to the
other three.**

What it *does* reach: obligation (i), and — indirectly but genuinely — the
contrapositive of "keys hang off sids". Because `owner_current_section_key_with_kex`
derives the section key from the sid chain returned by `resolve_clear`, a defect in
which the key derivation consumed the *name* instead of the sid would make the post-
rename `open_blob_v` fail its AEAD tag, and the `.expect(...)` at `:12753` would
panic. So "the key does not depend on the display name" is proven. That is real and
I credit it.

What it does not reach:

- **Obligation (ii), the sid is never changed.** The `Then` re-resolves everything
  from scratch through the index. A rename that minted a fresh sid, re-sealed the
  body under the new node key and rewrote the row would resolve, decrypt and return
  `BODY` — green. Nothing captures the sid before the rename and compares it after.
- **Obligations (iii) and (iv), "re-keys nothing, moves no bytes".** A rename that
  additionally re-sealed the child blob — same key, fresh nonce, updated
  `blob_sha` — changes every stored byte of the content and is invisible to this
  scenario. §2.9's sentence is a *cost and cut* obligation: re-keying on rename
  would silently cut existing grant holders and re-encrypt a subtree. The scenario
  asserts nothing that could see it.
- **The uniqueness obligation.** Nothing asserts that `projets/perso/note1` stops
  resolving. A `rename_folder` that *appended an alias row* instead of mutating the
  existing one would leave one sid carrying two names — both display paths live —
  and the scenario would be green. A rename that renames nothing passes the rename
  scenario.

This is `DBND-202`, and it is the substantive finding of this unit.

One structural remark that shapes the severity. The `Then` at `:12748` performs its
own read; it does not consume `w.read_body`. The scenario's two `When` steps are
therefore pure arrangement and the acting step is the `Then`. That is not by itself
a defect — but it means S6's assertion is *strictly* the assertion of S5 plus a
different path string. **S5 proves nothing S6 does not already prove**, except that
S5 additionally exercises the `owner_reads_circle` step wrapper (`:8382`). The Rule
has two scenarios and, at the level of what is asserted, roughly one and a bit.

---

## 3. Findings

### `DBND-201` — P2 — the Rule's word "sealed" is asserted by neither of its scenarios

**Statement.** `RU-2` is named *Content round-trips through the sealed store*. Both
of its assertions (`body_intact`, `cucumber.rs:12745`; `reads_at_new_path`,
`:12756`) compare a decrypted body to the constant `BODY` (`:68`). Both are
satisfied by any pair of mutually inverse write/read functions, the identity pair
included. Nothing in either scenario, and nothing anywhere in
`features/d-bundle.feature`, observes the bytes actually resident in `e/circle/`.
An implementation that wrote circle bodies in clear would satisfy this Rule, and
would satisfy it *while the Rule's own name asserted the opposite*.

**Absence claim, with its search.** Layer: the Gherkin step layer,
`rust/crates/aithos-bundle/tests/cucumber.rs` (20 040 lines, the sole registered
step file per `fn main`, `:20017-20040`).
Search 1 — every assertion in that file that inspects raw store bytes for a
plaintext needle: `grep -n "inspected.contains\|all.contains\|raw.contains"` →
**exactly one hit**, `:12775`, inside `self_leaks_nothing`.
Search 2 — every step that enumerates a zone prefix out of the store:
`grep -n 'store.list("e/'` → **exactly one hit**, `:8418`, inside
`inspect_self_zone`, and its argument is the literal `"e/self/"`.
Therefore the whole Gherkin layer contains one opacity assertion, it belongs to
`RU-4`, and it never looks at the circle zone.
Search 3 — repository-wide, `grep -rn "e/circle" rust/` → 87 hits, 39 of them in
`rust/crates/*/tests/*.rs`; I read the two that touch blob bytes
(`cucumber.rs:5656`, a delete-effect check; `cb7_transaction_contracts.rs:329-333`,
a symlink-escape check) and neither asserts opacity.
Search 4 — the nearest thing repository-wide is
`rust/crates/aithos-cli/tests/cli_surface.rs:283-288`, which asserts a body is
absent from the **Gamma log**, not from the blob, and uses its own `BODY`
(`cli_surface.rs:19`).

**Evidence required.** Mutant **M1** in §7 (`blob_seal`/`blob_open` reduced to a
length-preserving identity). **Prediction:** `d-bundle.feature:34` and `:39` both
stay **green**; the only Gherkin scenario that turns red is `d-bundle.feature:55`
(`RU-4`). If M1 comes back green on `:34`/`:39` this finding is confirmed; if it
comes back red on either, I am wrong and withdraw it.

**Closure criterion.** Add to `RU-2` an assertion that reads the raw bytes of
`e/circle/blobs/` out of the store — as `inspect_self_zone` (`:8414-8423`) already
does for `e/self/` — and asserts that neither `BODY` nor the section's title appears
in them; and add the corresponding `Then` line to the Rule so the contract, not only
the code, carries the obligation. The finding is closed when mutant M1 turns
`d-bundle.feature:32`'s Rule red.

**Not embargoed.** The statement describes an untested property, not an exploitable
weakness: the production code at `bundle.rs:837-844` and `seal.rs:52-63` does seal
correctly, and the fix is a test-side assertion that exists today for another zone.
Disclosure gate not engaged.

---

### `DBND-202` — P2 — S6 cannot distinguish a rename from an alias, a re-key or a byte move

**Statement.** `Display paths resolve through names, keys through sids`
(`d-bundle.feature:39`) asserts one thing: that after `rename_folder`, a read at the
new display path returns `BODY` (`reads_at_new_path`, `cucumber.rs:12748-12758`).
`spec/02-content-tree.md` §2.9 line 528 requires that renaming "edits an index row /
descriptor, **re-keys nothing, moves no bytes**", and §2.2 line 36 requires the sid
"assigned at creation, **never changed**". Three of those four obligations are
unobserved:

1. no assertion captures the folder or section **sid** before the rename and
   compares it after — a rename that minted a fresh sid and re-sealed under the new
   node key resolves and returns `BODY`;
2. no assertion compares the **blob bytes** or `blob_sha` before and after — a
   rename that re-sealed the child section under the same key with a fresh nonce
   changes every content byte and is invisible;
3. no assertion checks that the **old display path stops resolving** — a
   `rename_folder` that appended an alias `FolderRow` instead of mutating the
   existing one leaves one sid under two names, both live, violating §2.2's "unique
   among its siblings", and the scenario is green.

**Evidence required.** Mutants **M2** (alias instead of rename) and **M3**
(rename re-seals the child blob under a fresh nonce) in §7. **Prediction:**
`d-bundle.feature:39` stays **green** under both.

**Closure criterion.** `RU-2` gains, in the Gherkin and in the step bodies, three
observations that today do not exist: the section's sid is recorded in the `Given`
and asserted unchanged after the rename; the `blob_sha` of the section row (or the
blob bytes themselves) is recorded and asserted byte-identical; and a `Then` line
asserts that a read at `projets/perso/note1` is now refused. The finding is closed
when M2 and M3 both turn `d-bundle.feature:39` red, and when the Gherkin line
carrying each new obligation quotes it — the scenario name already promises "keys
through sids", so the sid assertion in particular belongs to this scenario and
nowhere else.

---

### `DBND-203` — P3 — both scenarios arrange a publication that neither assertion depends on

**Statement.** S5's `Given` ends with `publish_bundle` (`cucumber.rs:7718`) and S6
spends a whole `When` line on `the edition is republished`
(`d-bundle.feature:42` → `publish_edition`, `cucumber.rs:8343-8347`). Neither
assertion can observe either call. `read_section` consumes `e/circle/index.json`,
the zone header and `e/circle/blobs/{sid}.enc`; all three are written by
`init_bundle` and `add_circle_section`/`rename_folder`, none by
`Bundle::publish` (`bundle.rs:1678-1681`). Edition 1 itself is written by
`publish_at` from inside `Bundle::init`, not by `publish`. So the word *published*
in `RU-2` is decoration, and a reader of the Gherkin is entitled to believe the
round-trip is a round-trip *through a published edition*, which it is not.

This is the same step coupling that `features/.agents/d-bundle/STATE.md` §4 already
records as owed, quoting the accepted `b-derivation` round-2 impact review:
"le couplage de pas ouvert par la ronde 1 et inchangé par la ronde 2 :
`rename_the_folder`, `publish_edition` et `reads_at_new_path` — les trois sont des
pas `d-bundle` (`cucumber.rs:8394`, `:8343`, `:12748`)", under the `QUEUE.yaml` key
**`bder-006-d-bundle`**. All three of those steps are S6's steps. `RU-2` is the unit
that owes it. I name it here; the scope re-arbitration `STATE.md` §4 reserves to the
owner of the `BDER-006` decision is not mine and I do not make it.

**Evidence required.** Mutant **M4** in §7 (`Bundle::publish` short-circuited to
`Ok(())`). **Prediction:** `d-bundle.feature:34` and `:39` stay **green**;
`d-bundle.feature:16`, `:22`, `:27` (`RU-1`) turn red.

**Closure criterion.** Either the publication is made load-bearing — the `Then`
re-opens the bundle from the published manifest, or asserts the read's blob against
the pinned `files` entry of the latest edition — or the `Given` drops to
`an initialised bundle` and S6 drops line `:42`, so the contract stops promising a
publication it does not use. The finding is closed when M4 turns at least one
scenario of `RU-2` red, or when `RU-2` no longer mentions publication.

---

### `DBND-204` — P3 — the rename step's first parameter is decorative; the parent is hard-coded

**Statement.** `rename_the_folder` (`cucumber.rs:8394-8403`) builds its target as
`format!("projets/{name}")`. The Gherkin phrase
`the folder {string} is renamed to {string}` advertises a general step over any
folder; the body can only ever address a depth-2 folder whose parent is literally
`projets`. Written as `When the folder "projets" is renamed to "travaux"` the step
would construct `projets/projets` and the `.unwrap()` at `:8402` would panic on an
`InvalidPath`. Consequences for `RU-2`: the rename exercised is always of the
section's **direct parent**, so renaming a non-leaf ancestor — the case where the
sid chain has an untouched element above the renamed one, and where §2.5's
per-segment derivation is actually interesting — is never exercised; and the step is
unusable by any future scenario without editing its body.

**Closure criterion.** The step takes the full display path as its parameter
(`When the folder "projets/perso" is renamed to "intime"`) and the body passes it
through unchanged; and `RU-2` gains one row or one scenario renaming a non-leaf
ancestor, asserting the section still reads at the rewritten path.

---

### `DBND-205` — P3 — the Rule has no negative control of any kind

**Statement.** `RU-2`'s seven steps contain zero refusals. The Rule asserts a
sealed store round-trips, and never once asserts that a store that should not
round-trip does not. Concretely, the circle read path never compares the blob it
opened against the `blob_sha` pinned in the index row — `read_section_with_owner_kex`
(`bundle.rs:1237-1250`) opens the AEAD and parses, and that is all; contrast
`public_read` (`:1264-1290`), which does compare `row.blob_sha` against
`sha256_hex(&body)` at `:1284-1288` and refuses. Whether the circle asymmetry is a
production defect is out of scope for a Pass A (AEAD binds `did ‖ sid-path ‖
key_version`, so a substituted blob must be a genuine ciphertext of *that* node at
*that* version — a replay of an earlier body, not arbitrary content). What is in
scope is that `RU-2` could not tell either way, and the feature's only tamper
scenario (`d-bundle.feature:22` → `alter_pinned_file`, `cucumber.rs:8349-8355`)
flips a byte of `e/circle/index.json` and never of a blob.

**Closure criterion.** `RU-2` gains one scenario in which a circle blob is replaced
by a byte-valid earlier ciphertext of the same node at the same key version, and the
owner read — not `verify()` — is asserted to refuse or to be detectably stale. If
the protocol's answer is that the circle read is *not* obliged to bind the blob to
the signed edition and that `verify()` is the only such gate, then
`spec/02-content-tree.md` says so in a sentence that can be quoted, and this finding
closes as a spec clarification instead. **This is a protocol question and I did not
settle it from code: §2.4 (lines 101-109) specifies the blob format and §2.11 the
signature policy, and neither states whether a read must re-check the pin.** Routed,
not decided.

---

## 4. What I attacked and could not break

- **The `Then` round-tripping on its own `When`.** The named first failure mode.
  It is not present. `body_intact` (`:12745`) compares against the module constant
  `BODY` (`:68`), and `reads_at_new_path` (`:12756`) against the same constant. The
  written value and the compared value are both anchored to one literal, and the
  write and the read are separate calls through separate code paths
  (`section_add` → `put_blob_v`; `resolve_clear` → `open_blob_v`). No self-inverse
  tautology at the assertion level. What *is* tautological is the sealing layer
  underneath — that is `DBND-201`, a different and narrower charge.
- **An in-memory shortcut.** I hypothesised the read might be served from a cached
  tree rather than from the store. `pub struct Bundle` (`bundle.rs:283-286`) has two
  fields, `store` and `did`. `write_object` (`:388-391`) goes straight to
  `store.put`; `get` (`:398-403`) straight to `store.get`. There is no overlay on
  this path. The round-trip really does cross serialized bytes.
- **A proxy step.** All seven bodies read; none consumes a `OnceLock` verdict or a
  shared observation struct. Contrast `RU-5`/`RU-6`/`RU-7`, which do
  (`cb7_result`/`cb10_result`, `:7340`/`:7349`).
- **A source-text assertion.** None in this unit. Search, two layers:
  `grep -n "_SOURCE.contains(" cucumber.rs` → **no hit anywhere in the step file**
  (the 51 counted instances named by `chdr-lota-source-text-assertions` live in the
  five `cb2_*` test binaries, which use that constant naming); and
  `grep -n "include_str!" cucumber.rs` → 30-odd hits, all at `:78-90`+ and all
  pointing at `../../../../vectors/*.json`, **except one**: `:2054`,
  `include_str!("../src/session.rs")` inside `core_capability_api_is_narrow()`
  (`:2053-2058`), whose body is
  `!source.contains("pub fn sign(") && !source.contains("pub fn open(") && !source.contains("pub fn wrap(")`.
  That is the site `STATE.md` §8 flags, it serves `RU-7`, and no step of `RU-2`
  reaches it.
- **A miswired `Given`.** `published_with_section` (`:7714`) takes `(name, folder)`
  in that order and calls `add_circle_section(&folder, &name, "toto")`. Against the
  Gherkin `section "note1" in circle "projets/perso"` this is correct. I checked it
  because the parameter order is inverted between the phrase and the signature, and
  it survives.
- **A degenerate fixture.** The quantifiers here are singular by construction
  ("the section body", "the same section"), so there is no cardinality to be
  vacuous about. The fixture is thin — one folder chain of depth 2, one section, one
  tag — but thin is not degenerate, and `DBND-204` records the depth limitation
  where it actually bites.
- **A vacuous negative.** There are no negatives at all in this unit, which is
  `DBND-205`, not a vacuous-negative finding.
- **A vector with no consumer.** `RU-2` consumes no vector at all, so the class
  cannot arise here. Search, at the step layer: I read the seven step bodies
  (`:7714`, `:8382`, `:8394`, `:8343`, `:12743`, `:12748`) and the three
  `ProtocolWorld` helpers they call (`init_bundle` `:7358`, `add_circle_section`
  `:7374`, `publish_bundle` `:7397`); none references any `include_str!` constant
  and none opens a file. The vector constants of `cucumber.rs` are declared at
  `:78-90`+ and are consumed by the `CB*` scenarios of `RU-5`/`RU-6`/`RU-7`. The
  conditional follow-up `chdr-lota-vector-generators` (`STATE.md` §7) binds "the
  first cycle to touch a vector" and is **not** triggered by this unit — whether the
  feature as a whole triggers it is settled by `RU-5`/`RU-7`, not here.

---

## 5. What I could not verify, and why

- **Every prediction in §3.** I ran nothing. Five findings rest on predicted mutant
  outcomes and none is evidence until the orchestrator returns the transcripts named
  in §7 under an `evidence_id`. If M1 turns `:34` red, `DBND-201` falls. If M2 or M3
  turns `:39` red, `DBND-202` narrows or falls. I have written each prediction so it
  is falsifiable in one run.
- **Whether `RU-2`'s two scenarios currently pass.** I have no baseline. `DBND-203`
  in particular assumes the suite is green today; if it is not, the reasoning about
  what M4 changes is unsound. Command **E0** in §7 is the baseline and must be run
  first.
- **Whether M1, M2 and M3 compile.** I wrote them against the code at
  `bundle.rs:1571-1611`, `:837-844`, `:1237-1250` and `seal.rs:52-81`, and checked
  the types they depend on (`FolderRow`/`SectionRow` both derive `Clone`,
  `bundle.rs:35` and `:42`; `resolve_folder` exists at `grants.rs:185` and returns
  `Result<Vec<Sid>>`). I could not compile them. If a hunk fails to apply or to
  build, that is a defect of my patch and not a result.
- **Whether the circle read is *obliged* to re-check `blob_sha`.** `DBND-205`.
  §2.4 and §2.11 of `spec/02-content-tree.md` specify the blob format and the
  per-zone signature policy; neither contains a sentence about the read-side pin
  check. A protocol claim may not rest on a code search, so I routed it rather than
  asserting a defect.
- **The `chdr-016-grant-path` arbitration** (`STATE.md` §3), which requires this
  cycle to state whether `d-bundle` or `g-revocation` carries it. That is a
  feature-level decision on `Bundle::grant` (`grants.rs:739`); no step of `RU-2`
  touches the grant path, so this unit contributes nothing to it and does not decide
  it.

---

## 6. Verdict on I1's `RU-2`/`RU-3`/`RU-4` asymmetry hypothesis

`INVENTORY.md:110-119` proposed that the three Rules are one subject seen once per
zone, that "no zone asserts all three properties, integrity against the signed
edition is asserted only for public, and body round-trip is asserted only for
circle", and that the asymmetry is invisible to an auditor reading them apart.

**The asymmetry is real, and it is worse than I1 could see from the Gherkin.** The
matrix, from the step bodies:

| property | circle (`RU-2`) | public (`RU-3`) | self (`RU-4`) |
|---|---|---|---|
| body round-trips to a fixed constant | **yes**, `:12745` + `:12756` | **yes**, `:12762` (`PUB_BODY`) | **no** — `owner_reconstructs_tree` (`:12781`) asserts the *tree*, not a body |
| stored bytes observed for opacity | **no** | n/a (public is clear by design) | **yes**, `:12775` — but five hard-coded needles, all names/title/tag, and `SELF_BODY` is **not** among them |
| read bound to the signed edition | **no** | **weakly**, see below | **no** |
| display-path stability under rename | **yes**, `:12756` | **no** | **no** |

Two corrections to I1's reading, both in the direction of *more* asymmetry:

1. **"Integrity against the signed edition is asserted only for public" overstates
   what `RU-3` asserts.** The step `its integrity checks against the signed edition`
   (`d-bundle.feature:51`) resolves to `edition_verifies`, `cucumber.rs:12697-12701`
   — one `fn` carrying two phrases, the other being `RU-1`'s
   `edition 1 verifies offline` (`:12697`) — and its entire body is
   `w.bundle.as_ref().unwrap().verify().expect("edition valid")`. It verifies the
   *edition*; it never touches `w.read_body`. Nothing binds the value the stranger
   just read to the manifest that was just verified. So public does not really
   assert "this read is integrity-checked against the edition" either — it asserts
   "an edition exists and verifies", which `RU-1` already asserted with the same
   function. **That belongs to `RU-3`'s auditor and I leave it there**, but it
   changes the shape of the asymmetry: the property I1 thought lived in exactly one
   zone lives, on this evidence, in none.
2. **The opacity asymmetry is the load-bearing one, and it is measurable.** The
   whole Gherkin layer contains exactly one assertion over raw store bytes
   (`:12775`), fed by exactly one enumeration of the store (`:8418`, literal
   `"e/self/"`). That single assertion is all that stands between this repository's
   Gherkin contract and a bundle whose encrypted zones are stored in clear —
   and it will not even catch a leaked *body*, since its five needles are
   `"enfance"`, `"cicatrices"`, `"blessure"`, `"cicatrice au genou"`, `"sante"` and
   `SELF_BODY` is absent from the list. Mutant M1 is designed to measure exactly
   this: I predict it leaves `RU-2` and `RU-3` green and turns only `RU-4` red.

**So: yes, the asymmetry I1 suspected exists, and reading `RU-2` alone I would have
found `DBND-201` anyway — but I would not have known that no other zone covers it,
and I would have graded it P3 instead of P2.** I1's recommendation to give the three
Rules to one auditor was correct in substance. The parts of the asymmetry that live
in `RU-3` (the shared `edition_verifies` doing no read-binding) and in `RU-4` (the
needle list omitting `SELF_BODY`) I have named and deliberately not adopted; they
belong to those units' auditors, who will see them from their own side.

---

## 7. Commands I want run

I ran none of these. Each needs an `evidence_id`; I will cite them in the frozen
report.

### `E0` — baseline, before any mutant

```
cargo test -p aithos-bundle --test cucumber --no-fail-fast
```

I need the per-scenario verdicts for `features/d-bundle.feature:34` and `:39`, and
the run's totals. The runner (`cucumber.rs:20017-20040`) takes no filter argument,
so the whole suite runs; the transcript names each scenario.

### `E1` — mutant **M1**, sealing reduced to a length-preserving identity

Apply, run `E0`'s command, report the verdicts of `d-bundle.feature:34`, `:39`,
`:47`, `:55`, then revert.

```diff
--- a/rust/crates/aithos-core/src/seal.rs
+++ b/rust/crates/aithos-core/src/seal.rs
@@
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
+    // MUTANT M1: identity seal, length-preserving (16-byte tag kept as zeros).
+    let _ = (node_key, nonce, aad);
+    let mut out = plaintext.to_vec();
+    out.extend_from_slice(&[0u8; 16]);
+    out
 }
 
 pub fn blob_open(
     node_key: &[u8; 32],
     ciphertext: &[u8],
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
+    // MUTANT M1: identity open.
+    let _ = (node_key, nonce, aad);
+    Ok(ciphertext[..ciphertext.len().saturating_sub(16)].to_vec())
 }
```

**Prediction:** `:34` green, `:39` green, `:47` green, `:55` **red**. Confirms
`DBND-201` and settles §6.

### `E2` — mutant **M2**, rename appends an alias instead of renaming

```diff
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ fn rename_folder, non-self arm, bundle.rs:1605-1609
-                for f in &mut index.folders {
-                    if f.sid == sid {
-                        f.name = new_name.to_owned();
-                    }
-                }
+                // MUTANT M2: alias instead of rename — the old name survives.
+                let mut alias = index
+                    .folders
+                    .iter()
+                    .find(|f| f.sid == sid)
+                    .expect("target folder row")
+                    .clone();
+                alias.name = new_name.to_owned();
+                index.folders.push(alias);
```

**Prediction:** `d-bundle.feature:39` **green**. Confirms the third limb of
`DBND-202`.

### `E3` — mutant **M3**, rename re-seals the child blob (bytes move, key unchanged)

```diff
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ fn rename_folder, non-self arm, bundle.rs:1587 (immediately after the match arm opens)
             _ => {
+                // MUTANT M3: rename moves bytes — every direct child section of
+                // the renamed folder is re-sealed under the SAME key with a
+                // fresh nonce. §2.9 forbids this; the scenario cannot see it.
+                let m3_chain = self.resolve_folder(zone, display_path)?;
+                if zone == Zone::Circle {
+                    let m3_index: ZoneIndex =
+                        self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
+                    let m3_parent = m3_chain.last().map(ToString::to_string);
+                    let m3_rows: Vec<SectionRow> = m3_index
+                        .sections
+                        .iter()
+                        .filter(|s| s.folder_sid == m3_parent)
+                        .cloned()
+                        .collect();
+                    for row in m3_rows {
+                        let ssid = Sid::parse(&row.sid)?;
+                        let node = NodePath::section(zone, m3_chain.clone(), ssid);
+                        let (kv, key) =
+                            self.owner_current_section_key(owner, &m3_chain, ssid)?;
+                        let pt = self.open_blob_v(
+                            &format!("e/circle/blobs/{ssid}.enc"), &key, &node, kv)?;
+                        let sha = self.put_blob_v(
+                            &format!("e/circle/blobs/{ssid}.enc"), &key, &node, kv, &pt,
+                            ent)?;
+                        let mut m3_write: ZoneIndex =
+                            self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
+                        for s in &mut m3_write.sections {
+                            if s.sid == row.sid {
+                                s.blob_sha = sha.clone();
+                            }
+                        }
+                        self.put_json(&format!("e/{}/index.json", zone.as_str()), &m3_write)?;
+                    }
+                }
                 let index_path = format!("e/{}/index.json", zone.as_str());
```

**Prediction:** `d-bundle.feature:39` **green**. Confirms the "moves no bytes" limb
of `DBND-202`. This is the most invasive of the four patches; if it fails to build,
`E2` alone still carries the finding.

### `E4` — mutant **M4**, publication is a no-op

```diff
--- a/rust/crates/aithos-bundle/src/bundle.rs
+++ b/rust/crates/aithos-bundle/src/bundle.rs
@@ bundle.rs:1678-1681
     pub fn publish(&mut self, owner: &OwnerKeys, now: &str) -> Result<()> {
-        let latest: Manifest = self.get_json("manifest.json")?;
-        self.publish_at(owner, now, latest.edition.height + 1)
+        // MUTANT M4: publication does nothing. Edition 1 still exists because
+        // Bundle::init calls publish_at directly.
+        let _ = (owner, now);
+        Ok(())
     }
```

**Prediction:** `d-bundle.feature:34` **green**, `:39` **green**, `:16`/`:22`/`:27`
**red**. Confirms `DBND-203`.

### `E5` — a search I want run rather than trusted from my own grep

```
rg -n --no-heading 'contains\(' rust/crates/aithos-bundle/tests/cucumber.rs
```

I want the orchestrator's own transcript of every `.contains(` in the step file, so
`DBND-201`'s absence claim rests on a journalled search and not on mine.

---

*End of Pass A, `RU-2`. Findings: `DBND-201` (P2), `DBND-202` (P2), `DBND-203` (P3),
`DBND-204` (P3), `DBND-205` (P3). Nothing in this report is embargoed.*

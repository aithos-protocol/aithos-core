# Independent review — correction lot A, `d-bundle`

| Field | Value |
|---|---|
| Role | independent reviewer, Pass B of the correction cycle |
| Material | `git archive` extract of `2749eb2`, no `.git`, no run journal, no ledger, no mutant patch, **not the corrector's run report** |
| Not opened | `/root/work/aithos-core`. Nothing in this note derives from it |
| Gates run by this role | **none**. This role ran no `cargo` command, no test and no gate. Every transcript below was produced by the orchestrator, hashed and journalled under an `evidence_id`, and handed back |
| Findings judged | 17 — `DBND-001`, `-002`, `-003`, `-007`, `-008`, `-013`, `-014`, `-018`, `-019`, `-025`, `-026`, `-029`, `-031`, `-032`, `-033`, `-034`, `-040` |
| Result | **14 `VERIFIED`, 3 `NOT_VERIFIED`**, of which one is a correct refusal. **Frozen** after four evidence rounds |
| The two P1 | **both `VERIFIED`, both measured** — `DBND-018` by `ev-d8db2142`, `DBND-029` by `ev-fd348a12` and `ev-a03ee659` |
| New findings | `DBND-041` (regression, P2), `DBND-042` (P2), `DBND-043` (P2), `DBND-044` (P3) — §5 |

---

## 0. Evidence in hand

### 0.1 Baselines

| `evidence_id` | Verdict | Counters | What it establishes |
|---|---|---|---|
| `ev-728ad986` | GREEN | 1 feature / 7 rules / **53 scenarios / 307 steps** | the candidate baseline every mutant below differs from. The counters are the ones `DBND-034` declares, which is the proof of selection and of execution |
| `ev-766bfb31` | GREEN | 18 features / 114 rules / **838 scenarios / 3585 steps** | the unfiltered cucumber gate |
| `ev-b2a58b93` | GREEN | — | the workspace gate |

**`ev-766bfb31` settles the largest unverified risk in the first draft of this
review, and it settles it in the corrector's favour.** Five step phrases this lot
strengthened are shared with other feature files — `edition verification is
rejected` (`k-integration.feature:108`, `:111`), `edition 1 verifies offline`
(`:23`), `no folder name, section name, title or tag appears anywhere` (`:141`),
`the owner reads the same section at {string}` (`b-derivation.feature:56`) and
`the folder {string} is renamed to {string}` (`:54`). Each now carries assertions
it did not carry at `d9120d7`, and `k-integration`'s self folder is named
`sante`, which is one of the five needles the whole-store scan of
`inspect_self_zone` now searches. **None of it fires.** `b-derivation`'s
*Renaming never re-keys* silently inherits the sid, `blob_sha` and old-path
assertions and stays green; `k-integration`'s two tamper steps supply the
expectation `edition_rejected` now demands.

That is a real property of the lot and it was not free: the corrector chose
strengthenings that generalise. It is also the one thing here neither role could
have established by reading, and it is recorded as a positive result rather than
as the absence of a failure.

### 0.2 The thirteen mutants, and what their cardinalities decompose to

Baseline 53. Casualties are `53 − passed`.

| Mutant | `evidence_id` | Result | Casualties | Decomposition |
|---|---|---|---|---|
| `mut-001` — `bundle.rs:1726` prev-hash compare → `false` | `ev-daa381e8` | 52/53 | 1 | **forced**: `A broken chain fails closed` |
| `mut-002` — `bundle.rs:1750-1755` flat-pin loop deleted | `ev-ba21afb1` | 52/53 | 1 | **forced**: `A tampered file fails the edition` |
| `mut-003` — `bundle.rs:1280` `blob_sha` guard → `false` | `ev-7f7b6242` | 52/53 | 1 | **forced**: `A stranger reads public content with no key at all` |
| `mut-007` — `seal.rs` seal/open → identity | `ev-bc350999` | 39/53 | 14 | **ill-formed mutant, this reviewer's error** — §0.5 |
| `mut-008` — rename appends an alias | `ev-10d0056a` | 52/53 | 1 | **forced**: `Display paths resolve through names, keys through sids` |
| `mut-013` — `log.rs:201` self logs like public | `ev-2ce33188` | 52/53 | 1 | **forced**: `Self is a flat sea of opaque blobs` |
| `mut-014` — `lib.rs:348` `e/self` hidden from `list` | `ev-42e2cf60` | 51/53 | 2 | both **identified by re-read** — §0.5 |
| `mut-018` — `gamma.rs:300` every owner entry stamped | `ev-d8db2142` | 43/53 | 10 | **9 on the P1's line + 1 elsewhere**, verbatim — §0.5 |
| `mut-025` — `lib.rs:906` `recover_transaction` gutted | `ev-b5aeca70` | 46/53 | 7 | **6 on `:176` + 1 on `:204`** — §0.5 |
| `mut-026` — the M1+M3 pair | `ev-50277b00` | 46/53 | 7 | identical, step for step — §0.5 |
| `mut-031` — `:239` `mismatched_object` cell replaced | `ev-1dc33e60` | 52/53 | 1 | **forced**: row 1 of the capability outline |
| `mut-033` — `validate_display_path` → `Ok(())` | `ev-7cef322a` | 49/53 | 4 | **forced**: the four rejecting `MemStore` rows |
| `mut-040` — `bundle.rs:975` `SectionModify` → `SectionAdd` | `ev-8ad32358` | 50/53 | 3 | **forced**: the three `edit` rows |

Fourth round, the four gating mutants. Every one matched its prediction, and
three matched it to the exact failure this review named in advance.

| Mutant | `evidence_id` | Result | Casualties | Named in advance |
|---|---|---|---|---|
| `mut-007b` — `seal.rs` cleartext store, 16-byte trailer kept | `ev-ce878a3a` | 51/53 | 2 | **the message**: `the circle zone is sealed: '…' must not be resident in e/circle/blobs/` |
| `mut-029b` — `audit_capability` mints on substitute bytes | `ev-fd348a12` | 49/53 | 4 | all four capability rows, on `:232` not `:235` |
| `mut-029a` — the gamma entry carries its own signing seed | `ev-a03ee659` | 52/53 | 1 | the gamma row only, `operation_succeeded` intact |
| `mut-019c` — `zone_dk_with_owner_kex` returns a constant | `ev-97242336` | 50/53 | 3 | **exactly three**: `circle/read`, `self/read`, `self/list` |

**"Forced" is a claim about the code, not about the transcript.** It means: given
the mutant, exactly one scenario — or exactly one identified set — *can* fail, and
the casualty count matches that set's size. Where a count matches a set that is
not forced, the count is **consistent with** the prediction and does not
establish it. Those cases are named as such and are the subject of §6.1. This
review does not accept a cardinality in place of an identity when the identity is
not forced, and applies that rule to `DBND-026` at cost.

**`ev-d8db2142` is the cleanest result of the campaign and it carries a P1.** The
audit's own run of this edit (`ev-19a635cf`) was 50/51, the single casualty in
RU-7, *"on another clause entirely"*. The candidate run is 43/53 — **ten
casualties**. Subtract that same pre-existing RU-7 casualty, which the mutant
still produces and which no part of this lot touched: **nine**. The mutating rows
of the owner-parity outline are exactly nine. The arithmetic closes with no
remainder, on the finding whose whole content was *fifteen scenarios do not see
this*.

### 0.3 The two GREEN predictions, and why they are worth more than a red

| Mutant | `evidence_id` | Result | What it establishes |
|---|---|---|---|
| `mut-029c` — `pub fn manifest_private_key` added, called nowhere | `ev-b8f499f5` | **GREEN 53/307** | `DBND-029`'s closure criterion is **unsatisfiable**, not merely unmet |
| `mut-032` — `pub fn sign_any` added, called nowhere | `ev-a3932961` | **GREEN 53/307** | `DBND-032` is untouched by the lot, which is what its refusal rests on |

Both had to be run for the same reason: a claim that *no assertion at this tier
can see this* is a behavioural claim, and a behavioural claim carries a
transcript, not an argument. Without `ev-b8f499f5` the ruling in §1 `DBND-029`
would be this reviewer's reasoning about what a cucumber step can observe; with
it, it is a measurement. Without `ev-a3932961` the corrector's refusal would rest
on its own account of what it did not do.

### 0.4 The revision is test-only, and the counts moved once

Nine production anchors the audit cites hold at the byte it cites —
`bundle.rs:1280`, `:1605`, `:1726`, `:975`; `lib.rs:348`, `:906`;
`session.rs:234`; `gamma.rs:494`; and `session.rs`'s `pub fn` count at 18, a
number the audit itself had to correct from Pass A's 19. The lot changed tests,
not product.

Reconstructing the audit's §5.2 offset table against the delivered file: twelve
of thirteen blocks land on the predicted lines exactly; the thirteenth carries
exactly two extra `Examples` rows, `features/d-bundle.feature:268` and `:274`.
8 + 15 + 12 + 2 + 4 + **12** = **53**; 29 + 90 + 96 + 12 + 32 + **48** = **307**.
`ev-728ad986` reports those numbers. **No count move anywhere else in the lot.**

One existing line's *text* changed, at `features/d-bundle.feature:133`. It is
judged in §1 `DBND-019` and §5 `DBND-041`, and its authorship is stated there.

---

### 0.5 The second evidence round — four re-reads and two controls

The re-reads cost no run and one of them settled a verdict. What follows is what
each changed, including where it moved a verdict against the corrector and where
it exposed a mutant of this reviewer's that should never have been specified.

**`ev-50277b00` — the discriminator, and it lands.** Four failing scenarios, all
`Failure before the logical commit point preserves the old bundle byte for byte`,
all on the same step and with the same first assertion message:

> `✘ And staging remains non-canonical and is cleaned or recoverably resolved with no local-mutation orphan`
> `Step panicked. Captured output: FsStore: a staging generation other than the active one survived the reopen`

That is verbatim the panic message of `core_atomic_staging_clean`
(`cucumber.rs:12121-12125`) — `DBND-026`'s new observable, named in advance as
the string that would decide the verdict, and it is the *first* assertion
message, not a later one. **`DBND-026` → `VERIFIED`.**

**`ev-b5aeca70` is identical to it, step for step.**

**The arithmetic did not close, and chasing it retracted a ground of mine.** The
first enumeration named four failures against a counter line of 46 of 53 —
**seven** — and a step line of 306 of 307, which reconciled with neither. On that
enumeration this review wrote a third ground against `DBND-025`: that under a
mutant gutting the whole `FsStore` crash-recovery path, the step asserting crash
recovery stayed green. **The verbatim summary shows that ground is false.**

```
[Summary]
1 feature
7 rules
53 scenarios (46 passed, 7 failed)
306 steps (299 passed, 7 failed)

6 ✘  And staging remains non-canonical and is cleaned or recoverably resolved with no local-mutation orphan
1 ✘  And a crash or lost acknowledgement at that point resolves to the complete old or complete new state
     from the canonical manifest and Gamma head
```

**The seventh failure is `:204` — `DBND-025`'s own line — and it fires.**
46 + 7 = 53; 306 reconciles as six scenarios failing their last step plus one
failing earlier. Nothing was lost between the runner and the ledger; the
truncation happened between the transcript and the reader.

**The instinct to demand the verbatim line was right and the conclusion drawn
before it arrived was wrong.** Ground three is retracted in §1 `DBND-025` and the
verdict moves with it — ruled here, before the next batch runs, so that it cannot
be read as having been extracted by one.

**One unprompted corroboration.** Both transcripts carry

```
warning: methods `read_mirror_marker` and `reconcile_compatibility_mirror` are never used
   --> crates/aithos-bundle/src/lib.rs:461:8
```

which is the compiler stating what the audit's §7 established by reading: under
this pair, route three of `DBND-026`'s round-2 refutation — the compatibility
mirror that was supposed to make a leaked generation visible — has no caller at
all. It also retires a risk this review flagged in its first draft: the pair
compiles, so `deny(warnings)` is not in force under the gate profile and the
experiment is admissible.

**`ev-bc350999` — `mut-007` was ill-formed, and that is this reviewer's error.**
Three of the fourteen casualties are informative and all three say the same
thing:

- `Then the section body comes back intact` — `assertion left == right failed`.
  That is the bare `assert_eq!` panic header. Every new opacity assertion in
  `body_intact` carries a custom message (*"the circle zone is sealed: '…' must
  not be resident in e/circle/blobs/"*). **The scenario died on the round-trip
  comparison that predates this lot, not on anything the corrector added.**
- `Then the owner reads the same section at "projets/intime/note1"` —
  `SealRejected("blob does not open")`, i.e. `seal.rs:80`. The round trip is
  broken, not merely transparent.
- `Given a bundle with a self folder "enfance/cicatrices" containing section
  "blessure"` — **fails in its `Given`**: the fixture cannot be built.

The cause is now plain from the code. `blob_seal` (`aithos-core/src/seal.rs:52-63`)
returns XChaCha20-Poly1305 ciphertext **plus a 16-byte tag**, so a
"length-preserving identity" is not a cleartext store — it is a store whose
envelopes no longer parse. `ev-23aeba39`, the mutant `DBND-007`'s criterion
names, left `:34` **green** at 51/51; this one kills it, and kills fixtures.
**They are not the same experiment**, so the criterion has not been tested with
the mutant it names, and the corrector's new opacity assertion has still never
been shown to catch anything. The well-formed replacement is `mut-007b` (§6.2),
which keeps the 16-byte trailer and the round trip and puts the plaintext in the
blob.

**`ev-d8db2142` — the P1's decomposition, verbatim, with no remainder.**

```
[Summary]
1 feature
7 rules
53 scenarios (43 passed, 10 failed)
293 steps (283 passed, 10 failed)

9 ✘  And every mutation is journalized without consuming mandate counters
1 ✘  Then "the signature verifies against the public key"
```

Nine on `features/d-bundle.feature:134` — the P1's own line, one per mutating row
— and one elsewhere, which is the RU-7 casualty the audit's own run of this edit
already produced. The 9 + 1 inferred from the counters is the 9 + 1 in the
failure list, now read rather than deduced.

**`ev-42e2cf60` — the second casualty is explained, and it is a gift nobody asked
for.** The two failures are `When I inspect every file of the self zone as a
stranger` (the target) and `Then edition 1 verifies offline` (collateral). The
mechanism is exact: `edition_one_verifies_offline` (`cucumber.rs:13537-13545`)
rebuilds the fresh `MemStore` from `bundle.store.list("")`, so with every
`e/self` key hidden from `list` the copy lacks `e/self/index.json` and
`e/self/root.enc`, while `latest.files` still pins them — and the **cold**
`verify()` fails on a pin it cannot resolve. The **live** `verify()` three lines
above passes, because `Bundle::verify`'s pin loop reads through `get`, which the
mutant does not touch.

Nothing else in that function can produce that asymmetry. **So `ev-42e2cf60`
incidentally proves `DBND-003` limb B's cold-reopen assertion is live** — the
limb the audit explicitly left unmeasured as a budget decision, and for which
this review had offered only `mut-003b`. It is recorded in §1 `DBND-003` as a
transcript that gave more than it was asked for.

**`ev-6116bf38` — `mut-034a`, 3 passed / 50 failed. Too broad, and again this
reviewer's specification.** `validate_display_path` sits on nearly every path in
the crate, so 50 casualties measure the crate, not the control. It nonetheless
carries the one datum that gates: the `MemStore` positive row
`Then the operation is "resolved"` goes **red**. And it produced a result nobody
designed for — the four rejecting `MemStore` rows also went red **on the message,
not the verdict**, because `invalid_path("refused")` is none of the three grammar
strings `DBND-033`'s new assertion requires. `DBND-033`'s closure biting on a
mutant aimed elsewhere is independent corroboration that it discriminates
provenance and not merely failure.

**`ev-fd96b8a3` — `mut-034b`, 24 passed / 29 failed, and this one discriminates
cleanly.** Every `MemStore` row green **including its positive `resolved` row**,
because `MemStore` never touches `FsStore::get`; every `FsStore` row red
**including its positive `resolved` row**. Each of the two positive controls has
now been shown capable of going red, each by a mutant that breaks its own store's
read path and leaves the other store's rows untouched. **`DBND-034` →
`VERIFIED`.**

**Four of this reviewer's mutant specifications were too coarse** — `mut-007`,
`mut-019a`, `mut-019b`, `mut-034a`. Two of them still delivered their gating
fact; two did not. The orchestrator's refusal to keep guessing at one-line
descriptions is vindicated by the record, and §6.2 gives the remaining three as
unified diffs against named lines.

## 1. The seventeen findings

### `DBND-001` — P2 — *S4's rejection is over-determined* — **`VERIFIED`**

**Evidence: `ev-daa381e8`, 52/53.**

Both limbs of the criterion are on the record. `wrong_predecessor`
(`cucumber.rs:8920-8961`) reads the superseded manifest out of the store and
inserts `manifests/{height}.json` into `forged.files` with its true `sha256_hex`
before signing, so the unpinned-stray path (`bundle.rs:1759-1766`) can no longer
fire on that object. `edition_rejected` (`:13663-13677`) no longer asserts
`is_err()`: it takes the error, requires the `When` to have declared
`w.expected_verify_error`, and asserts containment. The expectation is
`broken chain at height {h}` (`:8955`), verbatim what `bundle.rs:1727` emits.

**Why one casualty is enough.** Under the mutant `verify()` stops rejecting the
forgery, so `edition_rejected`'s `expect_err` panics: that scenario **must** go
red. Nothing else depends on the comparison — `edition_two_verifies`
(`:13640-13655`) reads `latest.edition.prev_hash` off the manifest directly, not
through `verify()`. One casualty, and it is the only candidate.

The design exceeds the criterion: the expectation is written by the `When` that
created the fault, so the two rejection scenarios can no longer borrow each
other's error.

---

### `DBND-002` — P2 — *S3 never exercises what "pinned" uniquely means* — **`VERIFIED`**

**Evidence: `ev-ba21afb1`, 52/53, **and** `ev-daa381e8`, 52/53, as the
cross-check the criterion requires.**

`alter_pinned_file` (`cucumber.rs:8898-8917`) parses `e/circle/index.json`, takes
the first section's `sid` **from the index rather than hard-coded**, flips byte 10
of `e/circle/blobs/{sid}.enc` and declares `pinned file altered: {blob_path}` —
verbatim `bundle.rs:1753`. The pin loop runs before the stray check, before
`verify_links` and before the Merkle recomputation, so it is the first error.

**The criterion is a conjunction and both halves are measured.** It asks that the
pin-loop mutant turn the tamper scenario red *while* the chain mutant leaves it
green. `ev-ba21afb1` gives the first: one casualty, forced, because with the pin
loop gone nothing re-derives a sealed blob's bytes — re-run of the audit's
search, `grep -n "blob" rust/crates/aithos-bundle/src/state.rs` → `:227`, `:318`,
`:321`, all `index.blobs` **row** reads, none opening a `blobs/*.enc` byte.
`ev-daa381e8` gives the second: its single casualty is forced to be the chain
scenario, so the tamper scenario survived the chain mutant. The two transcripts
together discharge a criterion neither could discharge alone.

---

### `DBND-003` — P2 — *one bare `verify()` under two sentences in two Rules* — **`VERIFIED`**

**Evidence: `ev-7f7b6242`, 52/53.**

The split happened first, which is what the criterion demanded:
`cucumber.rs:13530` and `:13563` are two attributes on two functions.

*Limb A* — `public_read_integrity_checks` (`:13564-13615`) gives the word *its* a
referent: it asserts the recorded `w.read_body` is the body under test, resolves
the row out of `e/public/index.json`, asserts `row.blob_sha == sha256_hex(&pristine)`,
asserts the signed manifest pins that index, then tampers one byte, re-reads
keylessly, restores the byte and asserts the re-read was refused. Under the
mutant the tampered read succeeds and the assertion fires — forced, and the
single casualty matches.

**The mutant is a check-deletion and it is paired with an input the check would
have rejected**, supplied by the assertion itself at `cucumber.rs:13610-13614`.
That is the rule this cycle learned, applied.

*Limb B* — `edition_one_verifies_offline` (`:13531-13551`) keeps `verify()`, adds
`assert_eq!(w.latest_manifest().edition.height, 1)`, and rebuilds the store into
a fresh `MemStore` for a keyless reopen, which is how §2.12 *Keyless façade (G-D)*
words it. No mutant ran against limb B and the audit's criterion demands none —
it recorded that as a budget decision in `VERDICTS.md` § D, not an oversight.

**It is nevertheless measured, by a transcript aimed at something else.**
`ev-42e2cf60` (`mut-014`, `e/self` hidden from `MemStore::list`) kills this
scenario as collateral, and the mechanism can only be limb B: the fresh
`MemStore` is rebuilt from `bundle.store.list("")` (`cucumber.rs:13537-13545`),
so the copy lacks `e/self/index.json` and `e/self/root.enc` while `latest.files`
still pins them, and the **cold** `verify()` fails on a pin it cannot resolve —
while the **live** `verify()` three lines above passes, because `Bundle::verify`'s
pin loop reads through `get`, which the mutant does not touch. Nothing else in
that function produces that asymmetry. **Limb B's cold-reopen assertion is
live**, and `mut-003b` is no longer wanted for it. This is the second thing this
campaign's transcripts gave that nobody asked them for.

**Did the split weaken the other Rule?** No — the specific thing to check. Both
new bodies retain `verify()` (`:13532`, `:13575`). RU-1 gained the ordinal and
the cold reopen; RU-3 gained four assertions and a tamper. Neither Rule lost a
check it had.

---

### `DBND-007` — P2 — *the Rule's word "sealed" is asserted by neither of its scenarios* — **`VERIFIED`**

**Evidence: `ev-ce878a3a`, 51/53, and the failure carries the message this review
named as the only one that would discharge the clause.**

```
✘  Then the section body comes back intact
   Defined: features/d-bundle.feature:68:7
   Matched: cucumber.rs:13690:1
   Step panicked. Captured output: the circle zone is sealed:
   'Le corps de la note, ephemere et precieux.' must not be resident in e/circle/blobs/
```

**This finding took three rounds and the first two were wrong in opposite
directions**, which is worth stating because the sequence is the argument.

*Round two* graded it `NOT_VERIFIED` on the Gherkin conjunct alone.
*Round three* added a second and much heavier ground: `ev-bc350999` had turned
the scenario red on `assertion left == right failed` — the bare `assert_eq!`
header, i.e. the round-trip comparison that predates the lot — so the corrector's
opacity assertion had never been shown to catch anything, and the mutant that ran
was not `ev-23aeba39` at all. That ground was sound, and its cause was this
reviewer's: `blob_seal` (`aithos-core/src/seal.rs:52-63`) returns ciphertext
**plus a 16-byte Poly1305 tag**, so "length-preserving identity" produced a store
whose envelopes stopped parsing rather than a cleartext store — it broke round
trips and broke fixtures.

*Round four* ran the well-formed replacement. `mut-007b` keeps the 16-byte
trailer and the round trip and puts the plaintext in the blob, and the scenario
fails **on the corrector's assertion, with the corrector's message**. The
sibling opacity assertion fires in the same run — `Then no folder name, section
name, title or tag appears anywhere` panicking on `self zone leaked the string
'enfance'` — so both opacity surfaces the lot added are load-bearing under a
cleartext store.

**What the assertion is.** `body_intact` (`cucumber.rs:13691-13718`) keeps the
round-trip comparison and adds beside it `store.list("e/circle/blobs/")`, an
`assert!(!blobs.is_empty())` lower bound, concatenation of every blob's bytes,
and four `!contains` over `BODY`, `"note1"`, `"note"`, `"toto"`. The lower bound
is the control the audit had to ask for separately under `DBND-014`, supplied
here unprompted — and it is what stops the mutant being a check-deletion measured
against an empty haystack.

**The Gherkin conjunct is still unmet, and it is filed rather than held.**
`features/d-bundle.feature:57-68` is byte-identical to the audited Rule; no
`Then` line was added, because adding one moves the step count off 307 and the
lot grants one exception, to `DBND-034`. That obligation is now **`DBND-044`**,
P3.

**Why this closes and `DBND-008` does not**, since both have an unmet Gherkin
obligation and a reader is entitled to the distinction. Two reasons, and the
second is the substantive one. (1) `DBND-007`'s closure sentence is *"Closed when
the `ev-23aeba39` mutant turns a scenario of `:32` red"*, and the Gherkin
requirement sits in the preceding sentence; `DBND-008`'s closure sentence
conjoins them explicitly — *"Closed when `ev-f7261aa9` turns `:39` red **and**
each new obligation is quoted by the Gherkin line that carries it"*. (2) More to
the point: `DBND-007` added **one** assertion and it is measured, while
`DBND-008` added **three** and only one has ever been shown to fire. A finding
whose new proof is complete and whose contract lags is a different animal from
one whose new proof is two-thirds unexercised.

---

### `DBND-008` — P2 — *S6 cannot distinguish a rename from an alias, a re-key or a byte move* — **`NOT_VERIFIED`**

**Evidence: `ev-10d0056a`, 52/53, one casualty, forced.**

The criterion, verbatim, with its conjunction:

> RU-2 gains, **in the Gherkin and in the step bodies**, three observations that
> do not exist today … Closed when `ev-f7261aa9` turns `:39` red **and each new
> obligation is quoted by the Gherkin line that carries it**.

**The step-body work is the best in the lot.** `rename_the_folder`
(`cucumber.rs:8978-9013`) captures, before the rename, the section's `sid`, its
`blob_sha` and its old display path, resolving all three out of the index rather
than hard-coding them. `reads_at_new_path` (`:13721-13755`) adds the three
missing limbs, each citing the spec sentence it discharges: `row.sid` unchanged
(§2.2 *never changed*), `row.blob_sha` unchanged (§2.9 *moves no bytes*), and
`read_section(old).is_err()` (§2.2 *unique among its siblings*).

**One casualty, forced**: within the feature gate the rename scenario is the only
one that renames anything, and under an alias-appending rename the old-path limb
must fail. The mutant clause of the criterion is met.

**The Gherkin conjunct is not.** `features/d-bundle.feature:77-81` is unchanged;
the scenario still carries one `Then` and its four-part name is discharged by a
step body a reader of the contract cannot see. Same conflict as `DBND-007`, same
resolution owed.

**Two of the three new limbs are unproven.** The single casualty is the old-path
limb; nothing yet shows the sid or `blob_sha` assertions can fail. `mut-008b` and
`mut-008c` belong with the re-closure, not with this pass — the verdict is
already `NOT_VERIFIED` on other grounds and two runs spent hardening a finding
that is not closing is not where the campaign decides anything.

**A credit, now measured.** `b-derivation.feature:56` shares the `Then` phrase
behind the same `When`, so *Renaming never re-keys* silently inherits all three
new assertions — and `ev-766bfb31` shows it green. A strengthening of a
neighbouring feature, delivered free and confirmed.

---

### `DBND-013` — P2 — *RU-4's absence assertion searches one of four normative layers* — **`VERIFIED`**

**Evidence: `ev-2ce33188`, 52/53, one casualty, forced.**

`inspect_self_zone` (`cucumber.rs:9049-9107`) iterates `store.list("")` — the
whole store — minus a declared allow-list, and **accumulates the key alongside
the value** (`all.push_str(&path)`, `:9075`), which is the exact sentence the
audit said was missing: *"pushes the value and drops the key"*. `gamma/gamma.jsonl`
and `manifests/**` are in scope for the first time, which is the layer the mutant
travels through. Under it the section name appears in clear inside the signed
Gamma log; that scenario is the only one in the feature searching for it, so the
casualty is forced.

**Residual, recorded rather than waved through.** The audit named `manifest.json`,
`did.json`, `e/public/**`, `certs/**`. The corrector added `e/circle/`. It is
justified — `spec/02-content-tree.md` §2.8, verbatim to the end of the clause:
*"**name** — the human segment (`enfance`, `cicatrices`, `1234`);
`[a-z0-9_-]{1,64}`, unique among its siblings. Pure metadata: clear in the index
for `public`/`circle`, sealed for `self` (§2.8)."* — and this fixture's circle
zone is empty, so nothing is lost here. It does narrow *anywhere* by one whole
zone: a self name that leaked into a circle index row would be invisible. One
ledger line, not a defect against the criterion.

---

### `DBND-014` — P2 — *the negative has no positive control, and the second `Then` is not one* — **`VERIFIED`**

**Evidence: `ev-42e2cf60`, 51/53, two casualties, one of them forced.**

The lower bound is asserted at both ends and tied to what the `Given` builds: in
the `When` (`cucumber.rs:9082-9104`) `e/self/index.json` must be among the
inspected keys, at least two `e/self/blobs/` objects must be, and
`self_objects.len() >= 4`; in the `Then` (`:13763-13770`)
`assert!(!w.inspected_keys.is_empty())` runs **before** the five `!contains`.
That is the criterion's *"better"* form, not its minimum.

**Forcing.** The mutant hides every `e/self` key from `MemStore::list`. The step
now calls `list("")`, so the mutant was specified to filter on the key rather
than the prefix argument — otherwise it is not the same experiment. With the keys
gone, `self_objects` is empty and *the self zone index must be among the objects
inspected* cannot pass. That scenario is red by construction.

**The second casualty is now identified and the debt retired.**
`ev-42e2cf60`'s two failures are `When I inspect every file of the self zone as a
stranger` — the target — and `Then edition 1 verifies offline`, collateral from
dropping the `e/self` keys out of the store copy `edition_one_verifies_offline`
rebuilds. Mechanism in §0.5; it costs this finding nothing and it earns
`DBND-003` limb B a measurement.

---

### `DBND-018` — **P1** — *"without consuming mandate counters" is `assert_eq!(0, 0)`* — **`VERIFIED`**

**Evidence: `ev-d8db2142`, 43/53, ten casualties decomposing exactly as 9 + 1.**

The test was whether the fix *computes* rather than re-declares, and whether the
computation is reachable.

`core_owner_scenario` (`cucumber.rs:3966-4014`) slices
`appended = &gamma_after_entries[gamma_before..]`, after a guard that the
existing prefix was not rewritten (`:3969-3976`). From that slice, and only from
it:

```rust
let mandate_counter_delta = appended
    .iter()
    .filter(|entry| entry.authorized_via.as_ref().is_some_and(|via| !via.is_empty()))
    .count();
```

and `verify_owner_entry` — `aithos-core/src/gamma.rs:494-500`, the protocol's own
enforcement, which `Bundle::verify` never reaches — called on every appended
entry, its refusal carried rather than swallowed. The literal `0` at the old
`:3549` is gone. The counter is derived from the observable `spec/07-gamma.md:173`
names: *"`max_actions: N` ⇒ count entries whose `authorized_via` **contains** this
mandate id"*.

**Reachability, which is the half a computed observable can still fail.**
`appended` is empty on the six `list`/`read` rows, where both assertions are
trivially satisfied — correctly, and the step says so by asserting
`journal_kinds.is_empty()` there instead. On the **nine mutating rows** the slice
holds one entry and both assertions bite.

**The transcript closes it with no remainder.** Ten casualties minus the one the
audit's own run of this edit already produced in RU-7 is **nine** — the nine
mutating rows, exactly. The criterion asks for one row; the fix delivers nine,
and the arithmetic identifies them without needing the names.

**Both P1 of this lot were the same shape and they end differently.** This one
closes on a measurement. The other is below.

---

### `DBND-019` — P2 — *three of fifteen rows satisfy the capability clause on keyless paths* — **`NOT_VERIFIED`**

**Two things are true at once and neither cancels the other.**

**First, the work is in the tree.** `features/.agents/d-bundle/STATE.md:29`
records this finding as not implemented and refused. The candidate contains both
branches of its *either/or* criterion:

- **option (a)**, restate the `Then`: `features/d-bundle.feature:133` now reads
  *"the operation succeeds without a mandate, and the narrow owner capability is
  required exactly where the zone is keyed"*;
- **option (b)**, the negative control: `core_owner_stranger_refused`
  (`cucumber.rs:3901-3915`) rebuilds an equivalent fixture, drives the identical
  `core_owner_invoke` with an unrelated `OwnerKeys::genesis`, and reports
  `outcome.is_err() || bundle.verify().is_err()`; `core_owner_succeeds`
  (`:12292-12318`) asserts `stranger_refused == keyed` per row.

**Second, the sentence delivered is false on three rows.**
`core_owner_row_is_keyed` (`cucumber.rs:3777-3782`) returns `true` for
`public/create`, `public/edit` and `public/delete`, and the step then requires a
stranger to be refused on them. `spec/07-gamma.md:120-121`, verbatim to the end
of the sentence including the parenthetical:

> **Sealed bodies (content mutations).** For every `section.*` entry on a keyed
> zone (`circle`, `self` — `public` has no zone key and its mutations stay
> clear, target and payload at the top level like structural kinds), the body
> `{target, payload}` is AEAD under the **target node's content key**
> (derivation purpose `gamma-body`): the log reveals *that* someone acted at
> some time under some mandate, but *what was touched and what changed* is
> readable only by those who can read the node itself.

*Keyed zone* is a defined term whose extension is `{circle, self}`; `public` is
named in the same sentence as having no zone key. The code agrees:
`bundle.rs:801-806` and `:914-919` write a public body with
`self.write_object(&file, body.as_bytes())` — cleartext — and sign it with
`owner_content_sig`; `zone_dk` is never called for `Zone::Public`. What a
stranger's `OwnerKeys` breaks on those rows is the **edition signature**, checked
against `did.json` — an authority requirement, not a zone key. The assertion
passes for a reason the sentence does not name.

The corrector's own doc comment states the partition it meant — *"which of the
fifteen rows actually walk a path that consumes the owner's content key"*
(`:3763-3765`) — under which `public/create` would be `false`. The helper
contradicts its own comment as well as the spec.

**Authorship, stated because it changes who owes the repair and not what is
owed.** The orchestrator has disclosed that the sentence at `:133` is **its own**
editorial choice, not the corrector's: reverting would have restored a sentence
the repository had already measured false, and it chose to keep the replacement.
That disclosure is accepted and carried here on the record, with two consequences.

1. **The charge of a convenient refusal is withdrawn for `DBND-019`.** The first
   draft read the refusal label against the tree and inferred that the corrector
   had done the work and reported otherwise. With the sentence attributed to the
   orchestrator, a coherent and honest account exists — the corrector declined the
   finding as framed and the orchestrator carried option (a) editorially.
   Withdrawing an accusation the evidence no longer supports is not a courtesy;
   it is the same discipline as retracting a mutant, which this review also does
   below.
2. **The substance is unchanged.** A finding cannot be recorded as refused while
   both branches of its criterion sit in the candidate, and the delivered
   sentence is false under the protocol's own vocabulary on three of fifteen
   rows. `NOT_VERIFIED` stands, the ledger entry needs correcting either way, and
   the sentence needs one clause. See `DBND-041`.

**The negative control is real, and that is now measured.** `ev-97242336`
(`mut-019c`, `zone_dk_with_owner_kex` returning a constant) is 50/53 with
**exactly three** failures, all on `features/d-bundle.feature:133`: `circle/read`,
`self/read` and `self/list` — the three rows where a stranger now succeeds and
the operation mutates nothing, so `verify()` still passes and `stranger_refused`
goes false against `keyed = true`. This review predicted those three by name and
said any other result would mean the control measures something other than what
its name says. Option (b) of the criterion is not merely present; it bites.

**Which sharpens `DBND-041` rather than softening this verdict.** The step is
live, and what it asserts on `public/create`, `public/edit` and `public/delete`
is that the capability is required there — while the sentence it is attached to
says the opposite. A dead assertion asserting a falsehood is a documentation
error; a live one is a contract that contradicts its own proof.

**Two of this reviewer's own mutants are withdrawn as ill-formed, and that is
this reviewer's error to carry.** `mut-019b` routed `Zone::Self_` to
`clear_zone_entries`, which returns `Err` for `Zone::Self_` outright
(`bundle.rs:1445-1449`) — it would have killed the row for the owner as well as
the stranger and measured nothing about the partition. `mut-019a` is contrived in
the other direction for the same reason. Both are replaced by one well-formed
mutant, `mut-019c`, in §6.2.

---

### `DBND-025` — P2 — *line 121 asserts crash recovery and no scenario induces a crash* — **`VERIFIED`**

**Evidence: `ev-b5aeca70`, seven failures, and the seventh is this finding's own
Gherkin line.**

**A ground of this review is retracted here, in full, before it could be
retracted by a run.** The previous draft carried a third ground: that under
`mut-025` all named failures were on `:176` — `DBND-026`'s line — and none on
`:204`, so the new `Crash` fixture did not fire and the finding stood unrepaired.
The verbatim summary shows six failures on `:176` **and one on `:204`**. The
crash fixture fires. **The ground was built on a truncated failure list, and it
is dead.** It is left visible rather than deleted, because a reviewer who
demanded the verbatim line and then quietly dropped what it refuted would be
running the same trick this audit exists to catch.

**What the transcript now establishes, and it is the strongest form of the
test.** At `d9120d7` this exact mutant was **green, 51/51** (`ev-7caa8332`) — the
audit's own confirming transcript for the finding. The step at `:204` then
asserted only `assert!(core_atomic_observation(w).reopened)`, and `reopened`
stayed true. It now carries one further assertion,
`assert!(observation.crash_resolved_completely)` (`cucumber.rs:12147-12153`), and
the same mutant turns the same step **red**. **Nothing else in that step changed,
so the assertion that fired is the one the corrector added.** That is exactly the
test `DBND-007` fails and `DBND-034` needed two runs to pass: not *is the
scenario red*, but *is it red because of the new thing*.

The machinery behind it is real. `CoreAtomicFault::Crash` (`cucumber.rs:1588-1596`)
errors at the linearization call (`:1697-1701`) and **swallows the rollback**
(`:1716-1720`) — *"a dead process unwinds nothing"*, which is the difference
between a crash and a refusal and is what the whole tree lacked.
`core_atomic_state_is_complete` (`:1904-1929`) replaces map equality with the
protocol's own definition of a whole edition: `verify()` on the reopened bundle
plus `manifest.gamma_head == reopened.gamma_head()`, read explicitly out of
`manifest.json` and the gamma tree — which is the criterion's *"with the
manifest's `gamma_head` and the `gamma/` tree read explicitly rather than
inferred from map equality"*, met literally.

**The finding's own statement is answered.** *"No scenario induces a crash"* was
true at `d9120d7` and is false at `2749eb2`: a crash is induced, and when
recovery is broken the line that claims recovery goes red.

**Two residuals, and one of them is already a numbered finding.**

1. **The *"or lost acknowledgement"* disjunct still has no fixture.**
   `features/d-bundle.feature:204` names two states and only one is driven;
   `CoreAtomicFault::AcknowledgementLost` is written, documented as driving the
   second, and constructed nowhere. That is **`DBND-042`**, P2, open, and it now
   carries the whole of what is left here. Filing it separately rather than
   holding `DBND-025` open for it is the same bookkeeping this review applied to
   `DBND-031` and `DBND-043`: one closed finding, one open finding covering
   exactly the remainder, and no closed finding carrying an open obligation.

2. **The crash is induced from a step helper, not from the Gherkin.** The
   criterion's option (i) opens *"`:116`'s outline gains a `Given` that injects a
   failure inside the store's linearization"*, and the outline at `:199` still
   reads `Given a published "<store>" bundle snapshotted byte for byte` with no
   injected-failure `Given`; `core_atomic_crash_mem`/`_fs` are reached as a
   second phase of the success scenario's helper. Adding that `Given` moves the
   step count by two, which the lot forbids outside `DBND-034`. **This is the
   third finding in the lot to hit that conflict** — `DBND-007` and `DBND-008`
   are the others — and it is recorded once, in §4, as one shared debt rather
   than three times as three defects.

**Why the residuals do not hold the verdict open, stated so the standard is
visible and can be applied against me.** The closure clause — *"`ev-7caa8332`
must turn at least one scenario of `:89` red"* — is met and measured, and met by
this finding's own line and its own new observable. Where a residual is a
separable obligation it gets its own number and stays open; where it is the
finding's own claim going unproved, the finding stays open. `DBND-042` is the
first kind. `DBND-007`'s silent opacity assertion is the second, and that is why
these two findings end differently despite both having an unmet Gherkin conjunct.

---

### `DBND-026` — P2 — *line 99's snapshot cannot see a local-mutation orphan* — **`VERIFIED`**

**Evidence: `ev-50277b00`, four named failures, all on the step this finding
added, with the message this review named in advance as its discriminator.**

**The design is right.** `core_atomic_staging_orphan` (`cucumber.rs:1883-1901`)
reads `.aithos-current`, enumerates `.aithos-generations/` and reports any
generation that is not the active one — walking the raw tree with `std::fs`,
which is the only way past `collect_from`'s skip of every top-level component
beginning `.aithos-` (`lib.rs:602-609`). It is computed for the six `FsStore`
rows (`:2084-2085`) and is a named, reasoned constant `false` for the six
`MemStore` rows (`:2058-2060`), which is the right way to write a
per-store-class constant. `core_atomic_staging_clean` (`:12115-12126`) asserts it
separately from `partial_state_observed`.

**Why the verdict is `VERIFIED`.** The criterion closes *when the pair turns at
least one `FsStore` row of `:91` red*. Four rows of that outline are red, all on
`:176` — the step `core_atomic_staging_clean` owns — and the first assertion
message is verbatim its panic string, `FsStore: a staging generation other than
the active one survived the reopen` (`cucumber.rs:12121-12125`).

**That is the discriminator this review named before seeing it, and it was named
against the alternative that would have refuted it.** The rival reading was that
the rows die because `Bundle::open(FsStore::new(root))` now fails outright — with
`recover_transaction` gutted, `ensure_plain_directory` and the pointer read are
gone — in which case `core_atomic_failure_fs` returns `Err`, the observation
panics on `CORE-OWN-002 FsStore reopen failed`, and `staging_orphan_observed` is
never evaluated. The message rules that out: the reopen completed, the raw tree
was walked, the leaked generation was found. **The new observable is reached and
it bites.**

**On the orchestrator's question, answered as asked.** Separating `mut-025` from
`mut-026` was never what this verdict needed: the criterion names the pair, the
pair ran, and reporting one measurement covering two findings is correct and is
endorsed. What it needed was the failure line, and a re-read supplied it at zero
cost.

**The coupling's direction is now known, and it is not what the previous draft
said.** Both mutants produce the same seven failures: six on this finding's line
and one on `DBND-025`'s. So the two findings' observables are **both** live and
**both** caught by the same pair — which is why no mutant separates them, exactly
as the corrector predicted in advance. That is a property of the code, not a
weakness of either repair.

**And the compiler corroborates the finding's own mechanism, unprompted.** Both
transcripts carry `warning: methods read_mirror_marker and
reconcile_compatibility_mirror are never used` at `lib.rs:461`. Route three of
the round-2 refutation — the compatibility mirror that was supposed to make a
leaked generation visible — has no caller under this pair. The audit reached that
by reading `collect_from`'s `.aithos-` skip; the compiler now says it outright.

**Two departures from the criterion, both defensible, both recorded.** (1) The
criterion asks for a raw snapshot before the mutation and again after the reopen,
with an ordering dependency on `DBND-023`, a P3 not in this lot; the corrector
compares surviving generations against the live pointer after the reopen, which
answers the same question and drops the dependency. That is better than what was
asked. (2) Limb (b) — *"no key of the raw tree absent from the canonical view
carries any byte of the refused mutation"* — is not implemented; limb (a) is what
the mutant measures, so it does not block, and it stays open.

---

### `DBND-029` — **P1** — *`:139` is `assert!(!false)`* — **`VERIFIED`**

**Evidence: `ev-fd348a12` 49/53 and `ev-a03ee659` 52/53 — both limbs of spec 01.6
measured. `ev-b8f499f5`, GREEN 53/307, stands unchanged as the measurement that
the audit's closure criterion is unsatisfiable. A finding can be closed and its
criterion still be wrong, and both facts belong in the ledger.**

#### The audit's closure criterion is unsatisfiable, and this is measured

The criterion, verbatim:

> `secret_material_exposed` is **computed**, not assigned: either from an
> executed attempt — a typed call that would have to accept a seed — or `:139`
> is deleted from the outline and the property is discharged by a compile-fail
> test (`trybuild`) that `DOMAIN.md` names. Closed when `ev-ed18d7ef` turns a
> row of `:131` red.

`ev-b8f499f5` re-runs that mutant against the **corrected** candidate — a
`pub fn manifest_private_key()` added to `LocalSession` and called nowhere — and
the gate is green at 53/307. Three facts settle it:

1. The existence of an uncalled `pub fn` is not a runtime fact of any execution.
   No value differs, no call is made, no error is produced, so **no assertion
   over executed behaviour can distinguish the two trees.** `ev-b8f499f5` is that
   statement, run.
2. The criterion's first branch measures the APIs the test *calls*; it cannot
   measure one nobody calls. Satisfying it does not turn the mutant red.
3. The second branch deletes `:139` from the outline, after which no row of
   `:131` can go red at all. Branch and closing clause contradict each other by
   construction.

Only compile time can see it — `trybuild`, a `cargo-public-api` snapshot, or a
source scan — and a source scan is exactly what `DBND-032`, in the same Rule,
forbids as deciding evidence. **The audit asked for something impossible at the
tier it named**, and `DBND-032` option (b) carries the identical self-defeating
conjunction one finding away. Both were written by attaching a standard closing
clause to a criterion whose content had already made that clause unreachable.

**Recommendation to the audit**, which has already corrected itself three times
this cycle and is not above a fourth: replace the closing clause on both findings
with *"closed when a named mutant that changes what the operation executes turns
a row of `:131` red, and the API-surface limb is discharged at compile time by a
check named in `DOMAIN.md`."*

#### Why the verdict is `VERIFIED`, and by what route

**The first draft graded this `VERIFIED` on substance; the second withdrew it.**
The standard that forced the withdrawal is one this review insisted on for
everyone else — *where the corrector's assertion cannot be shown to catch
anything, the finding is `NOT_VERIFIED` however good the code looks* — and at
that point not one transcript showed `secret_material_exposed` failing for any
reason. Two mutants have now shown it, one per limb of the specification
sentence, so the verdict returns on evidence rather than on the reading it first
rested on.

**The *accepted* limb — `ev-fd348a12`, 49/53, four failures, all four capability
rows.** `audit_capability` made to mint on substitute bytes for a session holding
no owner private material turns every row of the outline red. Spec 01.6,
*"Stable APIs MUST NOT require a raw seed or private key when the narrow
operation suffices"*, read from the side that can be executed: a capability
handed out on substitute bytes is the failure that sentence is about.

**The *returned* limb — `ev-a03ee659`, 52/53, one failure, the gamma row, on the
same step.** The entry carries its own signing seed and still verifies, so
`operation_succeeded` held and the byte scan is what fired. Spec 01.6, *"MUST NOT
expose private material as an output."* **The single casualty is itself the
result**: had the leak broken `operation_succeeded`, the row would have died at
step 1 on a different Gherkin line and the green on the secret assertion would
have meant nothing. That is why this mutant's first form — injecting into
`assemble_draft2` — was withdrawn by this reviewer before it ran.

**What the computation is, restated now that it is load-bearing.**

`core_capability_secret_material_exposed` (`cucumber.rs:2368-2384`) is a real
computation — a 32-byte window scan plus a hex scan of the bytes each operation
produced, against per-row secret sets (grantee seed `:2455`; the owner's
`root_sign`/`content_sign`/`owner_kex` via `:2387-2394`; the wrapped DK and the
recipient's KEX secret for the wrap row, `:3464-3466`) — and a second limb
requiring a `LocalSession::grantee` holding no owner private material to be
**refused** `header_capability()` and `audit_capability()`. The literal `false` at
four sites is gone.

**Both halves of that computation have now been made to fire**, which is what
separates this from the state it was in one round ago: `ev-fd348a12` on the
grantee-refusal limb, `ev-a03ee659` on the byte-scan limb. `ev-b8f499f5` remains
green *by design* and remains the reason the audit's criterion cannot be the
thing that closes this — but it is no longer the only transcript this finding
has.

**The corrector's conduct here is credited and is not what is being judged.** It
declared the limit at the point of the fix, in the code (`:2362-2366`): *"No
runtime assertion can see a `pub fn` that was merely ADDED and never called …
`ev-ed18d7ef` is therefore NOT killed by this."* Declaring a limit rather than
claiming closure over it is right, and it is why this finding is one measurement
from closing instead of needing redesign.

**A residual, predicted and now confirmed by both gating transcripts.** The
four boundary lines `:232`–`:235` share one step body,
`d_capability_boundary_holds` (`cucumber.rs:9166-9182`), whose asserts run in the
order `mismatched_session_refused`, `cross_class_substitution_refused`,
`!secret_material_exposed`, and `:232` is evaluated first. So both mutants report
their failures on *arbitrary bytes or a mismatched Ethos, actor, purpose…* and
never on `:235`, the P1's own contract line. **The line that carries the claim
can never be the line a transcript names.**

It is not a defect in the repair — all four asserts run in the same body on every
row, so nothing is weakened — and it does not change this verdict. It is a
reporting property of the shared step body, adjacent to `DBND-039` (P3, outside
this lot), and it is **recorded rather than numbered**: it costs no proof, only
attribution. If the next audit round wants it numbered it is P3 and belongs with
`DBND-039`. The practical instruction is short: **a reader of `ev-fd348a12` or
`ev-a03ee659` must not conclude from `:232` that `:235` is sound**, and must not
conclude from a future green on `:232` that `:235` was exercised.

---

### `DBND-031` — P2 — *the `mismatched_object` column reaches no executing code* — **`VERIFIED`**

**Evidence: `ev-1dc33e60`, 52/53, one casualty, forced.**

`d_mismatched_capability_refused` (`cucumber.rs:9150-9163`) asserts
`observation.mismatched_object == w.core_capability_mismatch`, where the
observation's field is set by the scenario function that executed the attempt
(`:2452`, `:3373`, `:3406`, `:3469`). The mutant replaces one row's cell with a
string that names nothing; only that row can fail, and one did.

**What survives is raised separately as `DBND-043`.** The criterion's first
sentence asks that the mismatched object be *presented to the same capability
handle* and the refusal be *distinguishable from the session-mismatch refusal*.
On three rows of four it is not: `mismatched_object_refused:
mismatched_session_refused` on both `sign` rows (`:2453`, `:3375`) is literally
the same boolean — the audit's own *"two Gherkin lines, one proof counted twice"*,
unchanged — and the `wrap` row (`:3457`) is still a wrong-X25519-secret
decryption failure in which the capability plays no part. Only the `open` row
(`:3399`) executes an object-dimension refusal, and the audit already credited it.

**The corrector's reason is sound, and it is why the split between this finding
and `DBND-032` is principled rather than convenient** — the first thing this
review looked for. The object dimension is type-enforced; `CapabilityClass` is
private, `session.rs:234`'s guard is unreachable, and no wrong-class object can be
presented without a test-only constructor in the production crate. That is the
same blocker as `DBND-032`. **The difference is that `DBND-031`'s closure clause
is satisfiable and was satisfied, while `DBND-032`'s is not satisfiable at all.**
Naming the boolean honestly instead of dressing it as an object-class refusal is
an improvement in the record. It is not the fix the criterion described, so the
residual is filed as its own finding rather than buried inside a closed one.

---

### `DBND-032` — P2 — *`:137` and `:138` are decided by a grep of one source file* — **`NOT_VERIFIED`. Refusal holds.**

**Evidence: `ev-a3932961`, GREEN 53/307.**

Nothing changed. `core_capability_api_is_narrow` (`cucumber.rs:2396-2401`) is
byte-identical to the function the audit condemned. Search, scope the Gherkin
layer: `grep -n 'include_str!("../src/' rust/crates/aithos-bundle/tests/cucumber.rs`
→ **one line, `:2397`**; no companion covers the other modules.
`grep -rn "DBND-032" rust/` → **zero hits**: alone among the seventeen, this
finding left no trace in the tree. `ev-a3932961` puts that on the record rather
than leaving it inferred. Full ruling in §2.

---

### `DBND-033` — P2 — *the four `MemStore` rows survive deletion of display-path validation* — **`VERIFIED`**

**Evidence: `ev-7cef322a`, 49/53, four casualties, forced.**

`core_path_verdict` (`cucumber.rs:12187-12255`) asserts, for `MemStore`
display-path rows, that the rejection reason contains one of three grammar
refusals — *"path must be a non-empty relative POSIX path"*, *"path contains an
empty, dot, or parent segment"*, *"display path contains an unsupported name"* —
rather than `.is_err()`. With the validator neutered the four bad cells still
fail, but in `resolve_clear` on `Error::InvalidPath("no folder …")`, which matches
none of the three.

**The mutant is a check-deletion and its paired inputs are named**: the four
grammar-violating cells at `features/d-bundle.feature:264-267`, exactly the
inputs the check would have rejected. Deleting a guard with nothing to trip it
measures nothing; this trips it four times.

**Four casualties, and the fifth `MemStore` row survived** — the correct
signature: removing validation does not stop a valid path from resolving. The
count discriminates the two halves of the outline rather than merely summing
them.

**Residuals, recorded.** (1) The criterion's second clause — *"add rows whose
display path is valid-but-absent so the two failure modes are separated"* — is
not done and is not straightforwardly doable: the verdict vocabulary is
`rejected` | `resolved`, and a valid-but-absent path is `rejected` by a lookup
miss that the new assertion would reject as the wrong refusal. Separating it
needs a third verdict. Asserting the refusal *kind* already separates the two
modes on the rows that exist, so this does not block closure. (2)
`outside_access_observed` is still the literal `false` for `MemStore` (`:3530`) —
correct, there is no filesystem to escape, and it is computed for real on the
`FsStore` rows (`:3752-3756`).

---

### `DBND-034` — P2 — *`:148` is a vacuous negative: no positive control anywhere* — **`VERIFIED`**

**Evidence: `ev-fd96b8a3` (`mut-034b`, 24/53) and `ev-6116bf38` (`mut-034a`,
3/53). Each of the two positive controls has been shown capable of going red.**

**The orchestrator's instinct that these two gate was right, this review adopted
it, and the runs discharge it.**

The structural work is done and is better than the criterion asked. Two rows, one
per store, sharing the identical `Given`, `When` and `Then` as the ten negatives
rather than sitting beside them as a separate test:
`features/d-bundle.feature:268` and `:274`. `resolved` is not *did not error* —
`core_path_verdict` (`:12242-12251`) requires `resolved_matches_canonical`, and
that field is computed against the value the fixture published: for `MemStore`,
`Ok(OwnerContentOutcome::Read(body)) if body == "before atomic mutation"`
(`:3519-3522`); for `FsStore`, bytes that parse as a `ZoneIndex` carrying the
section named `note` (`:3720-3723`). The `FsStore` control reuses **the same
Store key** the outline rejects one row above under a symlink condition (`:273`
vs `:274`), isolating the filesystem condition as the only difference. The
`input` column was renamed from `invalid_input` because two of its cells are now
valid.

**And structure alone was not a measurement**, which is why this review withheld
the verdict for one round. `ev-7cef322a`'s four casualties were all rejecting
rows and both new rows stayed green — correct, and proof of nothing: a row that
passes is not a row that can fail. **A positive control nobody has shown can go
red is a vacuous positive**, which is `DBND-034`'s own finding one level up.

**`ev-fd96b8a3` settles it, and it discriminates rather than merely killing.**
Under `FsStore::get → Ok(None)`: every `MemStore` row green **including its
positive `resolved` row**, because `MemStore` never touches that function; every
`FsStore` row red **including its positive `resolved` row**. The mutant partitions
the outline exactly along the store column, which is the signature of a control
wired to its own store's read path and not to something ambient.
`ev-6116bf38` supplies the same fact for the `MemStore` positive row, though at
3 passed of 53 it is far too broad to discriminate anything else — `mut-034a` was
this reviewer's specification and it measured the crate rather than the control.

**A note on the criterion, kept because the distinction was drawn against this
finding and should not vanish now that it closes.** Uniquely among the seventeen,
this criterion carries no *"closed when mutant X is red"* clause — it asks only
for *"At least two rows — one per store — with a valid input and a `Then`
asserting success."* Read literally it was discharged a round ago. The lot's own
standard, which this review applied to the other sixteen, is that a closure is
proved by a named mutant; for this finding of all of them, that standard **is**
the finding's content. Both readings now agree.

*This is not the same judgement as `DBND-003` limb B, where no mutant ran and the
verdict is `VERIFIED`. There the audit deliberately recorded the absence as a
budget decision and the finding's confirmed state rests on the limb that was
measured. Here the whole content of the finding is "this suite has no positive
control", so an unfalsifiable control does not answer it at all.*

---

### `DBND-040` — P2 — *"journalized" is proved by cardinality alone* — **`VERIFIED`**

**Evidence: `ev-8ad32358`, 50/53, three casualties, forced.**

`journal_kinds` (`cucumber.rs:4016`) collects the wire `kind` of every appended
entry; `core_owner_expected_kind` (`:3786-3793`) maps the `<operation>` column of
the `Examples` grid to the kind spec 07.1 gives it; `core_owner_gamma`
(`:12331-12348`) asserts equality on the nine mutating rows and emptiness on the
six that journalize nothing. The strings match `aithos-core/src/gamma.rs:63-65`,
so the comparison is against the wire form and not an enum discriminant. The
expected value is a function of the grid column, never read back out of the entry
under test.

**Three casualties, forced, and the number is the argument.** The mutant changes
`SectionModify` → `SectionAdd` at the single `edit` site (`bundle.rs:975`), so
exactly the three `edit` rows can fail — and exactly three did. Under the old
cardinality-only check the delta was still 1 and all nine rows stayed green
(`ev-f18d4843`, 51/51). The per-row mapping is live and driven from the column.

`mut-040b` and `mut-040c` would cover the `create` and `delete` arms. They walk
the identical `core_owner_expected_kind` / `journal_kinds` comparison that
`ev-8ad32358` already turned red, so they are extra limbs of a mutant that landed
and are classified in §6.4 as corroborating, not gating.

---

## 2. The two refusals, judged on their merits

### `DBND-032` — the refusal holds. The finding stays `OPEN` at P2.

The closure criterion is unsatisfiable, in the same structural way as
`DBND-029`'s and for the same reason.

- **(a) discharge `:137`/`:138` behaviourally** requires a test-only path able to
  construct a wrong-class binding. `CapabilityClass` and `SessionBinding` are
  private items of `session.rs`; every capability struct's `binding` field is
  private; the nine `self.check(…)` sites each pass the class their parameter
  type already fixes. So (a) means **adding a constructor to the production crate
  whose only purpose is to make an unreachable guard reachable** — weakening a
  type-level guarantee in order to test the weaker runtime one. And even done, it
  does not turn the mutant red: `ev-a3932961` shows an added `sign_any` leaving
  the gate green at 53/307, and a behavioural cross-class test does not observe an
  added API.
- **(b) remove the two lines, write the type argument into `DOMAIN.md`, add a
  `trybuild` case.** Removing two step lines from a four-row outline moves the
  step count by 8, which the lot forbids outside `DBND-034`. And a line removed
  from the outline cannot make a row of `:131` red, so the closing clause
  contradicts its own branch.

**A criterion no admissible action satisfies is not a criterion a corrector can
be held to.** Refusing it with reasons is the correct move — better than
broadening the grep to more files, which would have looked like progress while
leaving a string search as deciding evidence, against the criterion's own
explicit prohibition.

**What the refusal does not do, and this must not be lost.** The finding is
correct and untouched. Two lines of the contract — one naming the
specification's strongest structural prohibition in this Rule — are decided by
`!source.contains("pub fn sign(")` over one file.
`spec/01-identity-and-keys.md` §1.6, verbatim to the end of the sentence
including the trailing clause:

> Every stable capability is bound to one typed protocol purpose and context. It
> accepts a typed object or request rather than arbitrary caller-selected bytes
> and binds the expected subject, domain, Ethos, actor and, where relevant, node
> path, key version, and recipient before performing cryptography. A generic
> `sign(bytes)`, decrypt-bytes, cross-context opening, or wrap-bytes oracle is
> not a compliant Bundle API, and a capability for one artifact class cannot
> substitute for another; lower-level raw primitives may remain an
> implementation detail behind that boundary.

`DBND-032` is `NOT_VERIFIED` and **stays `OPEN` at P2**. The debt is the audit's:
it owes a satisfiable criterion. The satisfiable form is cheap — a compile-time
public-API check over the whole crate rather than one file, named in
`DOMAIN.md`, with the two Gherkin lines pointing at it and a count exception for
their removal. That is a decision for the audit and the owner, not for a
corrector working a fixed lot.

### `DBND-019` — the refusal does not hold on the record. The charge of convenience is withdrawn.

Both branches of an *either/or* criterion sit in the candidate: the restated
`Then` at `features/d-bundle.feature:133`, and `core_owner_stranger_refused` with
its per-row `stranger_refused == keyed` assertion. **A finding cannot be recorded
as refused while both branches of its criterion are in the tree**, and the ledger
entry needs correcting regardless of who wrote which half.

**The convenience charge is withdrawn.** The first draft inferred from the label
that the corrector had done the work and reported otherwise. The orchestrator has
since disclosed that the sentence at `:133` is its own editorial choice. With
that on the record a coherent and honest account exists, and the accusation is
not supported by the evidence. It is withdrawn here in the same terms it was made.

**What survives is not an accusation but a defect**: the delivered sentence is
false under `spec/07-gamma.md:120-121` on three of fifteen rows, and the step
asserts its contrary on those rows. That is `DBND-041`.

**A note on which refusal was the risky one.** The brief warned that a convenient
refusal would be the easiest thing in this lot to miss, and the most likely hiding
place — `DBND-031` accepted and `DBND-032` refused on what looks like the same
type-system blocker — was tested and is clean. On the evidence now in hand,
neither refusal was convenient: one is correct on an impossible criterion, and
the other is a bookkeeping contradiction with an editorial cause.

---

## 3. What I attacked and could not break

**The P1 pair's computations are real, and one of them is now measured.** The
attack was: *is the new observable a computation over an empty set — the old
vacuity in a new coat?* For `DBND-018` it is not: `appended` is a real slice of
real entries on nine rows, the prefix-rewrite guard (`cucumber.rs:3969-3976`)
stops it being silently misaligned, and `ev-d8db2142`'s 9 + 1 decomposition
closes with no remainder. For `DBND-029` the computation is real too — but *real*
and *demonstrated* are different words, and §1 says so.

**The cross-feature blast radius.** Attacked as the largest unverified risk in
the first draft, on five shared step phrases and one shared needle.
`ev-766bfb31` clears it at 838/3585. Neither role should have assumed this and
neither did.

**The count discipline.** Attacked by reconstructing §5.2's offset table block by
block, expecting a quiet extra line — a `Then` slipped in for `DBND-007` or
`DBND-008`, a row dropped, a step reworded. Twelve of thirteen blocks land to the
line; the thirteenth carries exactly `DBND-034`'s two rows. The arithmetic
reaches 53/307 independently and `ev-728ad986` reports it.

**The production tree.** Nine cited anchors checked, expecting drift that would
mean the lot had touched the product. All nine hold, including `session.rs`'s 18
`pub fn`.

**The `DBND-003` split**, attacked for the failure mode the brief named — that
splitting a shared body drops an obligation from the Rule that got less
attention. Both new bodies retain `verify()`; neither Rule lost anything.

**`DBND-031` versus `DBND-032`**, attacked as a possible double standard. It is
not: one closure clause is satisfiable and satisfied, the other is unsatisfiable.

**`DBND-026`'s departure from its criterion**, attacked as a shortcut. The
substitute answers the same question, drops a dependency on a P3 outside the lot,
and cannot be fooled by `collect_from`'s `.aithos-` skip. It is a better answer
than the one asked for — a separate matter from whether it has been shown to fire.

**`mut-033`'s discrimination.** Attacked by asking whether the four casualties
might include the new positive row. They do not: 49/53 with four rejecting rows
dead and the `MemStore` `resolved` row alive is the correct signature, and the
wrong one would have been visible in the count. `ev-6116bf38` then corroborated
it from an unexpected direction: under a validator that rejects *everything*, the
four rejecting rows go red **on the message**, because `invalid_path("refused")`
is none of the three grammar strings. The assertion discriminates refusal
*provenance*, not merely failure — which is what `DBND-033` asked for and more
than the criterion's `io::ErrorKind` discriminator would have given, since a
lookup miss and a grammar refusal both surface as `InvalidInput`.

**`DBND-026`'s new observable, attacked on the reading that would have refuted
it.** Before the re-read this review named the rival hypothesis — the rows die on
`CORE-OWN-002 FsStore reopen failed`, with `staging_orphan_observed` never
evaluated — and staked the verdict on which message appeared. The transcript
returned the corrector's message, first, on all six. The hypothesis was
falsifiable, stated in advance, and failed.

**`DBND-025`'s crash fixture, attacked hardest of all and it held.** This review
wrote a ground asserting the fixture does not fire, and the ground was wrong.
What survived the attack is the sharpest fact about that repair: the same mutant
that was **green 51/51** at `d9120d7` now turns `:204` red, and the only thing
that changed in that step is the assertion the corrector added. An attack that
fails this cleanly is worth more to the record than one that succeeds.

---

## 4. What I could not verify, and why

1. **One ground of this review was false and is retracted.** `DBND-025`'s ground
   three — that the crash step stayed green — was built on a failure list
   truncated in transmission. The verbatim summary shows `:204` red.
   `DBND-025` → `VERIFIED`. The general lesson is not about the orchestrator's
   reading: it is that **this review twice drew a conclusion from a partial fact
   and stated it as a finding**, and only the second time did the demand for the
   verbatim line arrive before the conclusion was published.

2. **Three items listed here in earlier rounds are now discharged and are named
   so the freeze is not read as having quietly dropped them.**
   `DBND-007`'s opacity assertion has been observed to fail (`ev-ce878a3a`);
   `DBND-029`'s assertion has been observed to fail on both limbs (`ev-fd348a12`,
   `ev-a03ee659`); `DBND-019`'s negative control has been observed to fail on
   exactly the three rows predicted (`ev-97242336`), so the credit §3 gives it is
   earned.

3. **Two of `DBND-008`'s three new limbs** (sid, `blob_sha`) are unproven;
   `ev-10d0056a`'s single casualty is the old-path limb. This is the **only**
   place in the lot where a delivered assertion remains unexercised, and
   `mut-008b`/`mut-008c` would settle it.

4. **`DBND-042` has no transcript** and cannot get one until
   `CoreAtomicFault::AcknowledgementLost` is constructed; `mut-025b` is the
   mutant that would decide it once it is.

5. **The residual under `DBND-029`** — `:235` can never be the line a transcript
   names — is recorded, not numbered, and is the one observation in this review
   left deliberately without an identifier. The reason is in the finding block.

6. **Four of this reviewer's own mutant specifications were too coarse** —
   `mut-007`, `mut-019a`, `mut-019b`, `mut-034a` — plus `mut-029a`'s first form,
   withdrawn before it ran because it would have broken `operation_succeeded` on
   step 1 and made its own green meaningless. Two of the five still delivered
   their gating fact; three did not. Specifying a mutant from reading, without a
   compile, is a real source of error in this campaign and the record carries it.

7. **Three findings collide with the count freeze and it is recorded once here
   rather than three times.** `DBND-007` and `DBND-008` need a `Then` line;
   `DBND-025` needs an injected-failure `Given`. Each would move the step count
   off 307, which the lot forbids outside `DBND-034`. The resolution is a second
   named exception or an amended criterion, and it is an owner decision, not a
   corrector's.

8. **The corrector's stated reasons for the two refusals.** The run report is not
   in this extract. `DBND-032`'s refusal is judged sound on the criterion's own
   text, independently of the reason given.

---

## 5. New findings and regressions

Continuing from `DBND-040`; no retired identifier is reused.

### `DBND-041` — `OPEN`, **P2** — REGRESSION — the rewritten `:133` claims a partition the protocol's own defined term contradicts

**Site.** `features/d-bundle.feature:133`; `cucumber.rs:3777-3782`, `:12311-12317`.

**Authorship, stated plainly at the orchestrator's own request and because the
record is worth more than anyone's comfort.** This line is the **orchestrator's**,
not the corrector's. Faced with a sentence the repository had already measured
false (`ev-b6a36f72`), it declined to revert and kept a replacement. **The choice
was right and the ground given for it was insufficient.** Reverting would have
restored a measured falsehood, so keeping was correct — but *not reverting* does
not establish that the replacement is true, and the replacement was never checked
against the term the specification defines. That is a reasoning error, not bad
luck, and the corrector is not answerable for it.

**Evidential state: confirmed by transcript.** `ev-97242336` turns
`features/d-bundle.feature:133` red on three rows and green on the twelve others,
which establishes that the step is live and its partition load-bearing. That
partition classes `public/create`, `public/edit` and `public/delete` as
*capability-required*, and the sentence attached to it says they are not. This
finding was raised on the record alone; it is now a live assertion contradicting
its own contract line, not a reading.

**Statement.** The line reads *"…the narrow owner capability is required exactly
where the zone is keyed"*. `spec/07-gamma.md:120-121`, verbatim to the end of the
sentence including the parenthetical:

> **Sealed bodies (content mutations).** For every `section.*` entry on a keyed
> zone (`circle`, `self` — `public` has no zone key and its mutations stay
> clear, target and payload at the top level like structural kinds), the body
> `{target, payload}` is AEAD under the **target node's content key**
> (derivation purpose `gamma-body`): the log reveals *that* someone acted at
> some time under some mandate, but *what was touched and what changed* is
> readable only by those who can read the node itself.

and `spec/07-gamma.md:333`, verbatim to the end of the cell:

> | `section.add/modify/delete/redact` | `ethos.write` | sealed body (keyed zones) |

The keyed zones are `circle` and `self`. `core_owner_row_is_keyed` returns `true`
for `public/create`, `public/edit`, `public/delete`, and `core_owner_succeeds`
then asserts `stranger_refused == true` on them. The Gherkin says *not required
here*; the step asserts *required here*. Code confirms the spec:
`bundle.rs:801-806` and `:914-919` write public bodies with
`self.write_object(&file, body.as_bytes())` and sign with `owner_content_sig`;
`zone_dk` is never called for `Zone::Public`. The assertion passes because a
stranger breaks the **edition signature**, not a zone key.

**Ruling: fix inside this lot.** Four reasons, and the count one is decisive.

1. **It changes no count.** One existing line's text, no line added, no line
   removed: 53/307 holds, so the repair does not touch the single thing the lot's
   discipline protects, and it needs no owner exception. Deferring costs more
   than doing it — the next lot would have to re-establish the count baseline for
   a one-clause edit.
2. **The repository has now shipped a false sentence on this line twice.** The
   first was measured false; the second is demonstrably false against a defined
   term. Leaving it open means a third reader arrives at a contract line that
   contradicts the step beneath it.
3. **`features/AGENTS.md` § *Project stage*: alpha, nothing deployed, no edition
   published.** There is no compatibility cost and no reason to defer.
4. **The gate is owed anyway.** A Gherkin edit invalidates §5.2's *Current*
   column and the pinned digest, and `mut-041` is owed regardless — so the
   sequence in §6.3 costs one re-pin and one gate re-run whether this is done now
   or later, and doing it now spends them once.

**It must not be a bare word swap**, or the same mismatch returns in the opposite
direction. The sentence and the helper move together:

- `features/d-bundle.feature:133` → *"the operation succeeds without a mandate,
  and the narrow owner capability is required exactly where the zone is keyed or
  the edition is mutated"*;
- `core_owner_row_is_keyed` → `core_owner_row_requires_owner_authority`, its doc
  comment corrected to name the two mechanisms it actually unions: the zone
  content key for `circle`/`self`, and the edition signature for every mutating
  row.

**Closure criterion.** The sentence is true of all fifteen rows as written, the
helper's name and comment describe what it computes, **and** `mut-041` turns the
three `public` mutating rows red — the first evidence that the authority limb of
the partition bites at all.

**`mut-041` is delivered as a verified patch file**,
`reviewer-mutants/mut-041.patch`, sha256
`5ba56bc40b5ba84c501aab733aef208102c462074fb669fa48d331f5abdae719`, checked with
`patch -p1 --dry-run` and applying with no fuzz. It reduces
`Manifest::verify_signature` (`manifest.rs:240-254`) to `Ok(())` while keeping
the call, so a stranger's public mutation produces an edition that verifies:
public bodies are written in clear (`bundle.rs:801-806`) and the public row
signature is verified by nothing in the tree (the audit's `DBND-012`), so the
manifest signature is the only thing refusing those three rows today.

**Run it twice, and the first run is worth more than the second.** Applied
**before** the `:133` edit it produces this finding's own transcript — three
public mutating rows red, i.e. the step demanding the capability exactly where
the sentence says it is not required. Applied **after**, it confirms the
corrected sentence is true of all fifteen. Predicted casualties: exactly three,
`public/create`, `public/edit`, `public/delete`, on `:133`. And `mut-019c`
re-run after the edit must still give exactly its own three and no more — that is
the check that the corrected predicate was written as a union and did not loosen
the keyed-zone half.

**Disclosure gate.** Not engaged: a test-semantics mismatch over code that holds,
no exploitable weakness, fix named.

### `DBND-042` — `OPEN`, **P2** — `CoreAtomicFault::AcknowledgementLost` is built, documented as driving the `acknowledgement-lost` case, and constructed nowhere

**Site.** `cucumber.rs:1571-1587`, `:1624`, `:1706-1709`. Introduced by this lot,
inside the `DBND-025` closure.

**Absence claim, with its search.** Layer: the Gherkin harness, the only
registered step file. Scope: the whole tree. `grep -rn "AcknowledgementLost" rust/`
→ five hits, all in `cucumber.rs`: the declaration `:1587` and two `match` arms
`:1624`, `:1706`. Scope: the constructors. `CoreAtomicFault::parse` (`:1601-1613`)
maps six boundary strings, none to it; the two post-commit-point injection sites
(`:1938`, `:1976`) both pass `Crash`. Because it appears in live `match` arms, no
dead-code warning fires and the compiler will not tell anyone.

**Consequence, and it is sharpened by `DBND-025` closing.**
`features/d-bundle.feature:204` names two states. The **crash** disjunct is now
driven and measured — `ev-b5aeca70` turns that line red when recovery is broken.
The **lost-acknowledgement** disjunct is not: the branch where the outcome *is*
committed and the caller never learned it, the one obliging the reopen to
discover `after` from the canonical manifest and Gamma head, is unreachable. So
`crash_resolved_completely` observes the *equals before* case and never the
*equals after* case, and the `acknowledgement-lost` row of `recovery_cases`
remains counted by `cb2_bundle_boundaries.rs:342` and driven by nothing.

**This finding now carries the whole of what is left on that Gherkin line.** It
is a narrower charge than the one the previous draft made against `DBND-025` — a
written-and-unreachable enum variant, not a fixture that fails to fire — and it
should be read as such.

**Closure criterion.** A third phase beside `core_atomic_crash_mem`/`_fs`
constructing `AcknowledgementLost`, asserting `Err`, reopening, and asserting the
reopened state equals the committed state with no staging orphan. Closed when
`mut-025b` turns a row of `:199` red.

### `DBND-043` — `OPEN`, **P2** — the mismatched-object refusal is still the session-mismatch refusal on three of four rows

**Site.** `cucumber.rs:2453`, `:3375`, `:3457`. Residual of `DBND-031`, filed
separately so a closed finding does not carry an open obligation.

`features/d-bundle.feature:231` and `:232` still rest on one proof on three of
four rows. Only the `open` row (`:3399`) executes an object-dimension refusal.

**Why it is filed rather than folded in.** `DBND-031`'s closure clause is met
(`ev-1dc33e60`) and the corrector's rationale for the deviation is sound: the
object dimension is type-enforced and `session.rs:234` is unreachable, so
presenting a wrong-class object needs a test-only constructor in the production
crate. That is the same blocker as `DBND-032` and should be routed with it.

**Closure criterion.** Discharged together with `DBND-032` by whatever
compile-time mechanism replaces the grep, or by a `#[cfg(test)]` constructor in
`session.rs` making the class guard reachable — in which case this and
`DBND-032` option (a) close at once.

### `DBND-044` — `OPEN`, **P3** — two obligations proved in code are absent from the contract, blocked by the count freeze

**Sites.** `features/d-bundle.feature:57-68` (RU-2) and `:199-205` (RU-6's
success outline).

**Statement.** Two closure criteria in this lot asked for a Gherkin line as well
as a step body, and in both cases the step body was delivered and **measured**
while the contract line was not written.

- `DBND-007` asked to *"add the corresponding `Then` line to the Rule, so the
  contract carries the obligation and not only the code."* The opacity assertion
  exists and fires (`ev-ce878a3a`); the Rule still promises `sealed` only in its
  title.
- `DBND-025`'s option (i) opens *"`:116`'s outline gains a `Given` that injects a
  failure inside the store's linearization."* The crash is induced and fires
  (`ev-b5aeca70`); it is induced from a step helper, and the outline's `Given`
  still reads `a published "<store>" bundle snapshotted byte for byte`.

**Why the corrector did neither, and why that was right.** Each line moves the
step count off 307, and the lot grants exactly one count exception, to
`DBND-034`. Obeying the freeze was correct; what was missing was surfacing the
conflict rather than resolving it silently.

**Why P3 and not P2.** This is the *inverse* of the defect class this audit
tracks. A `Given` or `Then` that announces more than the fixture builds misleads
a reader into believing something is proved; here the fixture proves more than
the contract announces. It under-advertises. Nothing is unproved and no reader
can be misled into false confidence — only into missing a guarantee that exists.

**Closure criterion.** One `Then` line under RU-2 quoting the opacity obligation
and one injected-failure `Given` on RU-6's success outline, with the resulting
count move taken as a named owner exception in the manner of `DBND-034` — or the
two criteria amended to drop the contract clause, with the reason recorded.
**This is an owner decision, not a corrector's**, and it is one decision covering
both sites plus `DBND-008`'s conjoined clause, not three defects.

---

## 6. What is still wanted

### 6.1 Nothing on reconciliation

The verbatim summaries were supplied and are quoted in §0.5. They retracted a
ground of this review (`DBND-025`) and confirmed the P1 decomposition
(`ev-d8db2142`). The suspicion that numbers were being lost between the runner
and the ledger is **withdrawn**: 46 + 7 = 53 and 306 reconciles exactly.

### 6.2 Nothing on the gating batch

All five gating mutants have run — `mut-007b` (`ev-ce878a3a`), `mut-029b`
(`ev-fd348a12`), `mut-029a` (`ev-a03ee659`), `mut-019c` (`ev-97242336`), and
`mut-034a`/`mut-034b` (`ev-6116bf38`, `ev-fd96b8a3`) in the round before. Every
one matched its prediction. **No further run is wanted for any of the seventeen
findings.**

### 6.3 For `DBND-041`, whose fix is the corrector's

`reviewer-mutants/mut-041.patch` — sha256
`5ba56bc40b5ba84c501aab733aef208102c462074fb669fa48d331f5abdae719`, verified with
`patch -p1 --dry-run`, applies with no fuzz.

| step | patch | predicts |
|---|---|---|
| 1 | `mut-041` **before** the `:133` edit | three rows red — `public/create`, `public/edit`, `public/delete` — on `:133`. **This is `DBND-041`'s own transcript** and converts it from a reading to a measurement |
| 2 | the corrector's edit: `:133` gains *"or the edition is mutated"*, `core_owner_row_is_keyed` → `core_owner_row_requires_owner_authority` with its doc comment naming both mechanisms | feature gate GREEN **53/307** — the count must not move |
| 3 | `mut-041` **after** the edit | the same three rows red, now under a sentence that is true of all fifteen |
| 4 | `mut-019c` re-run after the edit | still **exactly three** — `circle/read`, `self/read`, `self/list`. This is the check that the corrected predicate is a union and did not loosen the keyed-zone half |

The §5.2 *Current* column and the feature file's sha256 in §1 of the audit need
re-pinning after the Gherkin edit; that is the check the audit itself names.

### 6.4 Deferred, with the reason

| id | for | why not now |
|---|---|---|
| `mut-008b`, `mut-008c` | `DBND-008`'s sid and `blob_sha` limbs | `DBND-008` is the one finding still open on its own proof; these belong with its re-closure and are the runs that would settle it |
| `mut-025b` | `DBND-042` | recovery sweeps correctly but the canonical manifest's `gamma_head` is left stale, so no orphan exists and `core_atomic_state_is_complete` is the only thing that can fire. Belongs with `DBND-042`'s closure |
| `mut-040b`, `mut-040c` | `DBND-040` create/delete arms | extra limbs of a mutant that already landed; not wanted |
| `mut-003b` | `DBND-003` limb B | **not wanted**: `ev-42e2cf60` proved limb B live as collateral |

### 6.5 A practice this review adopts, having caused the problem twice

Four mutant diffs written in prose in the previous round failed `git apply`: the
Markdown round-trip stripped the leading space that marks a context line, and the
hunk headers miscounted. `mut-041` above was therefore **generated
programmatically from the file and verified before being sent**, and that is the
form every future mutant from this role will take: a patch file under a named
path, with its sha256, checked to apply with no fuzz. The orchestrator should
reject a mutant that arrives any other way, including from this role — a
specification that cannot be applied mechanically is a specification that gets
re-typed by hand, and this cycle has now lost time to exactly that twice.

---

## 7. Verdicts — frozen

| Finding | P | Verdict | Evidence | Decided by |
|---|---|---|---|---|
| `DBND-001` | P2 | **`VERIFIED`** | `ev-daa381e8` 52/53 | confound removed **and** error identity asserted; casualty forced |
| `DBND-002` | P2 | **`VERIFIED`** | `ev-ba21afb1` + `ev-daa381e8` | tamper inside the sealed blob; the criterion's cross-check holds |
| `DBND-003` | P2 | **`VERIFIED`** | `ev-7f7b6242`; limb B by `ev-42e2cf60` | step split; limb A tampers and requires refusal; limb B's cold reopen proved as collateral |
| `DBND-007` | P2 | **`VERIFIED`** | `ev-ce878a3a` 51/53 | red **on the corrector's own message** — *the circle zone is sealed: '…' must not be resident in e/circle/blobs/*. Contract line → `DBND-044` |
| `DBND-008` | P2 | `NOT_VERIFIED` | `ev-10d0056a` 52/53 | the Gherkin conjunct sits inside the closure sentence and is unmet; two of three new limbs never shown to fire |
| `DBND-013` | P2 | **`VERIFIED`** | `ev-2ce33188` 52/53 | whole store minus a spec-justified allow-list, keys accumulated |
| `DBND-014` | P2 | **`VERIFIED`** | `ev-42e2cf60` 51/53 | lower bound at both ends; casualty forced; second casualty identified |
| `DBND-018` | **P1** | **`VERIFIED`** | `ev-d8db2142` 43/53 | computed from `appended`; `verify_owner_entry` called; **9 on `:134` + 1 elsewhere**, read off the failure list |
| `DBND-019` | P2 | `NOT_VERIFIED` | `ev-97242336` 50/53 | both branches in the tree against a "refused" label; the control bites, and what it asserts on three rows contradicts the sentence (`DBND-041`) |
| `DBND-025` | P2 | **`VERIFIED`** | `ev-b5aeca70` | `:204` red under a mutant **green 51/51** at `d9120d7`; the only new thing in that step is `crash_resolved_completely`. Remainder → `DBND-042`, `DBND-044` |
| `DBND-026` | P2 | **`VERIFIED`** | `ev-50277b00` | six rows red on `:176` with *a staging generation other than the active one survived the reopen* — the named discriminator, first message |
| `DBND-029` | **P1** | **`VERIFIED`** | `ev-fd348a12` 49/53, `ev-a03ee659` 52/53 | both limbs of spec 01.6 measured — *accepted* on all four rows, *returned* on the gamma row with `operation_succeeded` intact |
| `DBND-031` | P2 | **`VERIFIED`** | `ev-1dc33e60` 52/53 | the cell must name the object the attempt presented; residual → `DBND-043` |
| `DBND-032` | P2 | `NOT_VERIFIED` — **refusal holds** | `ev-a3932961` GREEN | criterion unsatisfiable under both branches; finding stays `OPEN` |
| `DBND-033` | P2 | **`VERIFIED`** | `ev-7cef322a` 49/53; corroborated by `ev-6116bf38` | refusal *kind* asserted; discriminates provenance, not merely failure |
| `DBND-034` | P2 | **`VERIFIED`** | `ev-fd96b8a3` 24/53, `ev-6116bf38` 3/53 | both positive controls shown capable of red, partitioned along the store column |
| `DBND-040` | P2 | **`VERIFIED`** | `ev-8ad32358` 50/53 | wire `kind` per row, driven from the `Examples` column; three casualties forced |

**14 `VERIFIED`, 3 `NOT_VERIFIED`. Both P1 closed, both on transcripts.**

The three that remain open are three different things and should not be read as
one number:

- **`DBND-032`** — the corrector refused it and the refusal is **correct**. The
  criterion is unsatisfiable under both its branches, `ev-a3932961` measures that
  the finding is untouched, and the debt is the audit's: it owes a satisfiable
  criterion. Nothing here is owed by the corrector.
- **`DBND-008`** — the only finding still open on its own proof. Two of its three
  new limbs have never been shown to fire, and its closure sentence conjoins the
  Gherkin requirement explicitly. `mut-008b` and `mut-008c` would settle it.
- **`DBND-019`** — a bookkeeping contradiction (both branches present against a
  "refused" label) plus a contract line that needs one clause, which is
  `DBND-041` and is ruled fixable inside this lot at no count cost.

**The question that decided this review, stated once because it decided verdicts
in both directions:** *is the scenario red because of the thing the corrector
added?* `DBND-007`, `DBND-025`, `DBND-026`, `DBND-029`, `DBND-033` and `DBND-034`
answer yes on a transcript, and four of those six only after a mutant of this
reviewer's had to be withdrawn and rebuilt. `DBND-008` cannot answer it for two
of its three limbs. Cardinality never answered it; the failure line always did.

**Verdict movement across four evidence rounds**, recorded so the freeze is not
mistaken for a first impression: `DBND-026`, `DBND-034`, `DBND-025`, `DBND-007`
and `DBND-029` all moved to `VERIFIED` from an earlier `NOT_VERIFIED`, and
`DBND-029` had first moved the other way from an over-generous first draft. Two
grounds this review published were later retracted on evidence — `DBND-025`'s
ground three and the charge of a convenient refusal on `DBND-019` — and both
retractions are left visible at the point they were made.

**Four new findings**: `DBND-041` (P2, regression, ruled fixable inside this lot
at no count cost), `DBND-042` (P2, a written and unreachable fault variant),
`DBND-043` (P2, `DBND-031`'s residual), `DBND-044` (P3, two proved obligations
absent from the contract, blocked by the count freeze).

**Disclosure gate.** Assessed for all seventeen findings and the four new ones.
Not engaged by any: every statement describes a proof gap over code that holds at
this revision, every fix is named, and no statement would describe an exploitable
weakness with no fix. Nothing in this note is withheld and nothing is carried
separately.

---

*Frozen at `2749eb2`. Baseline `ev-728ad986` — 1 feature / 7 rules / 53 scenarios
/ 307 steps. Global gate `ev-766bfb31`, workspace `ev-b2a58b93`. Twenty-one
transcripts cited; no run claimed that is not journalled; no gate executed by
this role.*

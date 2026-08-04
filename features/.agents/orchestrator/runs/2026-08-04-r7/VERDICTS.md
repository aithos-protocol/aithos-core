# Post-Pass-A verdicts — what the orchestrator ran, and what it decided

This file is the orchestrator's, not an auditor's. It records every measurement
and every ruling made **after** `frozen.json` was hashed, so Pass B can tell a
finding that was confirmed from one that was merely asserted.

`frozen.json` sha256 `d3f8f33324c48e3c12bfd425b19238b0ba80bc502a662a5255073e980c3a685b`.
Baselines: feature gate `ev-5f523aae`, green, 1 feature / 7 rules / 51 scenarios
/ 299 steps. Every mutant below was applied to a clean tree at `d9120d7` and
reverted before the next.

## A. Findings CONFIRMED by a mutant transcript — 15

Each auditor named the mutant and its predicted outcome **before any transcript
existed**. Sixteen of seventeen landed exactly.

| Finding | Sev | Mutant | Result |
|---|---|---|---|
| `DBND-101` | P2 | `bundle.rs:1726` — predecessor-hash check deleted from `verify` | `ev-d1fc33b5` **green 51/51** |
| `DBND-102` | P2 | `bundle.rs:1749` — the whole flat-pin loop deleted | `ev-de2706a8` **green 51/51** |
| `DBND-201` | P2 | `seal.rs` — `blob_seal`/`blob_open` reduced to a length-preserving identity, i.e. a cleartext store | `ev-23aeba39` **50/51**; the single casualty is RU-4's scenario 8, in a different Rule |
| `DBND-202` | P2 | `bundle.rs:1605` — rename appends an alias, the old name survives | `ev-f7261aa9` **green 51/51** |
| `DBND-301` | P2 | `bundle.rs:1280` — the `blob_sha` guard deleted from the keyless public read | `ev-c7f65638` **green 51/51** |
| `DBND-401` | P2 | `log.rs:201` — the self zone logs like the public zone, section name in clear in the signed Gamma log | `ev-f1718be8` **green 51/51** |
| `DBND-402` | P2 | `lib.rs:348` — the self prefix invisible to listing, every byte left in place | `ev-0b4e1076` **green 51/51**, the scenario passes having inspected nothing |
| `DBND-501` | **P1** | `gamma.rs:300` — every owner entry declares `authorized_by` and `authorized_via` | `ev-19a635cf` — **all fifteen RU-5 rows green**; the one casualty is in RU-7 on another clause. `ev-bec6b91e` shows `aithos-core`'s `entries_rebuild_byte_for_byte` red |
| `DBND-502` | P2 | `cucumber.rs:3420` — the five `owner_content_operation` call sites take a stranger key | `ev-b6a36f72` — 12 of 15 rows die, **three survive**: two `list`, one `read`, exactly the composition predicted |
| `DBND-601` | P2 | `lib.rs:906` — the entire `FsStore` crash-recovery path replaced by `self.transaction = None` | `ev-7caa8332` **green 51/51** |
| `DBND-702` | P2 | `d-bundle.feature:143` — the `mismatched_object` cell replaced by a string that exists nowhere | `ev-3fa9d172` **green 51/51**. Control, same row, `observable_result` cell: `ev-1eefbb66` **red 1/51**. The pair is the finding |
| `DBND-703` | **P1** | `session.rs` — a public `manifest_private_key()` accessor added to the capability surface | `ev-ed18d7ef` **green 51/51** |
| `DBND-704` | P2 | `session.rs` — a `sign_any()` universal byte-signing oracle, named around the grep | `ev-794d59c3` **green 51/51** |
| `DBND-706` | P2 | `lib.rs` — `validate_display_path` reduced to `Ok(())` | `ev-2d2ebd1b` **green 51/51**, all ten confinement rows included |
| `DBND-707` | P2 | same mutant as `DBND-706` | `ev-2d2ebd1b` |

### Controls, run so that a green would expose something the auditors missed

| Control | Result |
|---|---|
| `bundle.rs:1658` — `publish` stops pinning its predecessor | `ev-5474b889` **red, 31 of 51**. Not green: no hidden P1 |
| `lib.rs:383` — `MemStore` rollback **commits** what it should discard | `ev-f0125e0b` **red, 4 of 51**. The RU-6 auditor predicted six and staked its P3 on any survivor. Four is enough to license its positive statements, so nothing escalates — but **two rows it believed `MemStore`-backed are not**, and that is its error about its own fixture. Pass B must not repeat the six |
| `d-bundle.feature:87` zone `self` → `vault` | `ev-f0658ee9` **red 1/51** |
| `d-bundle.feature:73` operation `list` → `enumerate` | `ev-de8fa887` **red 1/51** |

The last two matter beyond their finding. Three auditors were primed to test
first whether their `Scenario Outline` rows execute distinct bytes, because this
repository carries 360 Gherkin lines resolving to 19 cached-verdict step
definitions. **Both columns of the largest outline reach code that errs on an
unknown value.** The proxy class does not reach this feature, and that is now
measured on both columns rather than inferred from reading. Record it as a
negative result, not as an absence.

## B. Findings REFUTED by the adversarial panel — 6, and they are dead

The owner ruled the panel runs only on the ten P1/P2 findings carrying **no**
confirmed mutant: the other fifteen are settled by measurement and a vote adds
nothing to a transcript. Each refuter saw the code and **one** finding statement
— not the report, not the other findings, not the mutant results, not the
author, and not that any other refuter exists. Each refutation below reduces to
a single fact the orchestrator then verified in the source.

**`DBND-302`** — *`public_read_k1c` has no consumer.* The function has zero call
sites, but it requires `indices/public.json`, and **no code path in this
repository ever writes that object** — four read sites, zero writes. Every
draft.2 edition this repository can produce carries `public/sections/<sid>.md`
and no public index. An uncalled function whose input does not exist proves
nothing about the Rule.

**`DBND-504`** — *"journalized" is proved by cardinality alone.* False. `Bundle::verify`
calls `gamma::verify_links` (`bundle.rs:1772`) which calls `Entry::check_form`
on every entry (`gamma.rs:428`), and `check_form` reads `kind`, `target`,
`payload`, `body_enc`. The finding's own worked example — a circle entry with a
`target` naming a different node — is rejected at write time in `gamma_append`,
before the count is ever taken.

**`DBND-505`** — *the three `Then`s are redundant; deleting two changes nothing.*
The arithmetic is wrong. The helper compares `gamma_delta` against the **vector
field** `case["journalized"]` (`cucumber.rs:3529`, `:3538`); the `Then` compares
it against a **hardcoded predicate** on the operation name (`:11536`). Two
different sources, and nothing cross-checks them — so the step called redundant
is the only thing in the suite tying *"every mutation is journalized"* to
anything real.

**`DBND-705`** — *the mismatch enumeration is proved by two identical sessions,
and "arbitrary bytes" is vacuous by construction.* The identical-sessions core
is **true**. What is built on it is not: `session.rs:354`
`append_header_recipient` takes `ephemeral: [u8; 32]` and `nonce: [u8; 24]`, and
`cucumber.rs:3082` — inside the fixture this outline drives — submits
`[0x76; 32]` and `[0x77; 24]` on the success path.

**`DBND-708`** — *row `:163` never reaches the symlink check.* True of that row,
and irrelevant: the sibling row at `d-bundle.feature:160`,
`| FsStore | display path | folder/link-out/section |`, installs its symlink at
the **intermediate** component and drives a key the grammar accepts, so
`checked_join`'s per-segment walk is the sole defence making that row pass. The
case is tested. At most one row is mislabelled, and it passes for a correct
reason.

**`DBND-710`** — *the baseline is snapshotted after the fixture, so it compares a
tampered tree to itself.* The ordering is stated correctly and the conclusion is
inverted. The fixture **renames real bundle files and replaces them with
symlinks inside the snapshot's own range** (`cucumber.rs:3256`, `:3268`).
Snapshotting before the fixture would make `before == after` false for every
`FsStore` row **against perfectly correct code**, failing six of ten rows
unconditionally. The current order is the only one under which the assertion can
ever pass, and the only one that measures the operation rather than the test's
own attack input.

## C. Findings that SURVIVED the panel — 4

Each survived attacks that turned on the attacker. These are not "unrefuted for
lack of effort".

- **`DBND-503`** — the refuter went looking for a reading of *narrow* that would
  give the phrase a referent and found `spec/01-identity-and-keys.md:142-168`
  defining it **against** the claim's target: *"A protocol operation receives
  only the narrow opaque capability it needs… Stable APIs MUST NOT require a raw
  seed or private key when the narrow operation suffices."*
- **`DBND-603`** — the refuter invented a sixth attack the brief had not
  suggested, the live-handle route, and killed it itself. It corrected two
  cosmetic errors in the finding — *two maps* where three are compared, and
  *1857 lines* where the arithmetic gives 1774 — and said plainly that neither
  is load-bearing. **Pass B must carry both corrections into the published
  text.**
- **`DBND-701`** — the refuter found evidence the claimant had **not** used and
  which cuts the same way: for row `:143`, `operation_succeeded` is a
  byte-equality against a golden JSON vector with **no signature verification at
  all**, under a sentence promising *"the signature verifies against the public
  key"*. It also found that `:136`, `:137`, `:138` and `:139` — four distinct
  English sentences — all bind to one step function asserting the same three
  booleans.
- **`DBND-709`** — the refuter established that `spec/02-content-tree.md:94-96`
  enumerates exactly **six** confinement surfaces, verbatim — *"before read,
  write, list, edition load, staging publication, or recovery"* — and that the
  outline exercises one. Its steelman failed because `cold_verify`'s first store
  contact is `list`, whose confinement lives in different code from `get`. It
  also conceded one ancillary sentence: the crate's own harnesses classify
  `cold-load key` **inside** the store-key family, so the naming complaint is
  weaker than the coverage complaint.

## D. The 23 P3 findings

Untested by mutant and unexamined by the panel, by design — the panel budget was
spent where it changes what a corrector does. They stand as Pass A left them and
Pass B rules on them on the record alone. Several are cross-unit and at least
three pairs look like the same defect seen from two units; reconciling them is
Pass B's job, not a re-audit.

## E. Two cross-unit convergences reached independently, and one collision

Recorded in `frozen.json` before any of the above:

1. **RU-2 and RU-3**, separately, found that the step asserting *"its integrity
   checks against the signed edition"* is one shared function whose body never
   touches the value read — so the property is asserted in **no** zone rather
   than in one.
2. **RU-6 and RU-7** examined the `Given` and `Then` they share and agreed from
   opposite sides: it is RU-6's principal assertion and **vacuous** in RU-7,
   because every RU-7b row performs a read and the assertion compares a tree to
   itself around an operation with no write path.
3. **RU-5 and RU-7** both reported that *narrow* carries two senses across the
   two Rules with no bridge.

`edition_verifies` (`cucumber.rs:12697`) carries **both** RU-3's line 51 and
RU-1's line 13 — a step-body collision across units that no single auditor could
see.

# INVENTORY — `d-bundle` (role I1)

Input: `/root/work/i1-d-bundle/d-bundle.feature`, 165 lines,
sha256 `59a6f361598de459fa063e7bff9915427c5e3d70423c20204d26f294b618c8b5`.
No other material was opened. No gate, test or `cargo` command was run.
Everything below is derived from the text of that file alone.

## 0. Counts — agreement with the published contract

| Quantity | Published | My reading | Agreement |
|---|---|---|---|
| Features | 1 | 1 | yes |
| `Rule:` blocks | 7 | 7 | yes |
| Authored scenario blocks | 13 | 13 | yes |
| Expanded scenarios | 51 | 51 | yes |
| Steps (expanded) | 299 | 299 | yes |

**No disagreement to report.** Expansion arithmetic, so it can be re-checked
without re-deriving it: 8 plain scenarios contribute 8; the five outlines
contribute 15 + 12 + 2 + 4 + 10 = 43; 8 + 43 = 51. Steps: 29 authored step
lines across the 8 plain scenarios, plus (6×15) + (8×12) + (6×2) + (8×4) +
(4×10) = 90 + 96 + 12 + 32 + 40 = 270 across the outlines; 29 + 270 = 299.
The file contains 61 authored step lines in total.

Structural facts about the file as a whole:

- One file-level tag, `@d-bundle` (line 1). No tag on any Rule or Scenario.
  No `@wip`, `@skip`, `@ignore` or equivalent anywhere.
- No `Background:` block, at feature level or inside any Rule.
- No Rule carries a description; only the Feature does (lines 3–6).
- All five `Examples:` blocks are anonymous (no `Examples: <name>`), one block
  per outline, so no outline has more than one Examples table.
- Every `<placeholder>` used in a step is bound by a column of that outline's
  Examples header, and every column is consumed by at least one step. There are
  no orphaned columns and no unbound placeholders. (Checked mechanically.)
- No step line is empty, and no `Given` is empty.

---

## 1. Review units

The default of one unit per `Rule:` was checked rather than assumed. I keep it:
**seven units, RU-1 … RU-7, in file order, 1:1 with the Rule blocks.** The
adjudication of the two candidates that argued against that default is recorded
in § 1.8; both were resolved in favour of keeping the 1:1 map, with a
co-reading instruction attached rather than a renumbering, so that downstream
Pass A agents can cite `RU-n` and a reader can find the Rule by name without a
translation table.

### RU-1 — `Rule: Editions chain and verify offline` (line 8)

4 authored / 4 expanded scenarios, 14 steps.

- `Initialising a bundle publishes a verifiable first edition` (10)
- `Every publication extends the chain` (16)
- `A tampered file fails the edition` (22)
- `A broken chain fails closed` (27)

### RU-2 — `Rule: Content round-trips through the sealed store` (line 32)

2 authored / 2 expanded scenarios, 7 steps.

- `The owner reads back what was written` (34)
- `Display paths resolve through names, keys through sids` (39)

### RU-3 — `Rule: The public zone reads without any key` (line 45)

1 authored / 1 expanded scenario, 4 steps.

- `A stranger reads public content with no key at all` (47)

### RU-4 — `Rule: The self zone leaks no structure` (line 53)

1 authored / 1 expanded scenario, 4 steps.

- `Self is a flat sea of opaque blobs` (55)

### RU-5 — `Rule: Owner operations have durable parity across all three zones` (line 61)

1 authored / 15 expanded scenarios, 90 steps.

- `The local owner performs every content operation without a mandate` (63),
  Scenario Outline, Examples header `| zone | operation |`, 15 rows
  (3 zones × 5 operations, a complete cross-product).

### RU-6 — `Rule: A local mutation commits state and Gamma as one transaction` (line 89)

2 authored / 14 expanded scenarios, 108 steps.

- `Failure before the logical commit point preserves the old bundle byte for byte`
  (91), Scenario Outline, Examples header `| store | boundary |`, 12 rows.
- `A successful local transaction publishes content and Gamma together` (116),
  Scenario Outline, Examples header `| store |`, 2 rows.

### RU-7 — `Rule: Local capabilities and paths stay narrow` (line 129)

2 authored / 14 expanded scenarios, 72 steps.

- `A bundle operation uses only its narrow opaque cryptographic capability`
  (131), Scenario Outline, Examples header
  `| capability | protocol_object | mismatched_object | observable_result |`,
  4 rows.
- `An untrusted path or Store key can never escape its selected root` (148),
  Scenario Outline, Examples header
  `| store | input_kind | invalid_input | filesystem_condition |`, 10 rows.

### 1.8 Splits and merges considered

**Merge candidate, rejected on numbering but accepted on reading order:
RU-2 + RU-3 + RU-4.** These three Rules are one subject seen three times: what
a read returns in each of the three zones. Circle (RU-2) asserts body
round-trip and display-path stability; public (RU-3) asserts clear readability
plus integrity against the signed edition; self (RU-4) asserts name opacity plus
owner reconstructability. Read apart, each looks complete. Read together, the
asymmetry is visible immediately — no zone asserts all three properties, integrity
against the signed edition is asserted only for public, and body round-trip is
asserted only for circle. That asymmetry is the kind of thing an auditor loses
by reading them apart, so **RU-2, RU-3 and RU-4 should be assigned to a single
auditor and read as one sitting**, even though they stay separately numbered.
I did not merge the numbering because the three Rule titles make three distinct
promises and Pass A will want to quote them individually.

**Split candidate, rejected: RU-7.** Its two outlines share the word "narrow"
and essentially nothing else. Line 131's outline is about *authority* — which
cryptographic operation a capability handle may perform, on which typed object.
Line 148's outline is about *reach* — which storage location a supplied path or
key may address. Their vocabularies are near-disjoint (`Ethos`, `actor`,
`purpose`, `recipient`, `sign/open/wrap` vs `root`, `symlink`, `display path`,
`Store key`). I did not split because 14 expanded scenarios is not too large to
hold, and because the Rule's own title asserts these two narrownesses are one
subject — which is itself a claim Pass A may want to test. **Treat RU-7 as one
unit with two sub-subjects, RU-7a (line 131) and RU-7b (line 148).**

**Size check on RU-5 and RU-6.** RU-5 is one outline of 15 rows and 90 steps;
RU-6 is 14 rows and 108 steps — the largest unit by step count. Neither needs
splitting: each is driven by a single Examples grid whose rows differ only in
tabulated values, so an auditor reads one step body and varies the data.

---

## 2. Per-scenario shape

`G/W/T` counts attribute each `And` to the keyword it continues.
"Claim" restates, neutrally, what the scenario's *name* asserts — not what its
steps do. Pass A compares the two; I am only stating the first half.

| # | Unit | Line | Name | Kind | Examples | Rows | Steps (G/W/T) | Claim the name makes |
|---|---|---|---|---|---|---|---|---|
| S1 | RU-1 | 10 | Initialising a bundle publishes a verifiable first edition | Scenario | — | 1 | 4 (1/1/2) | Bundle initialisation, on its own, produces a first edition that can be verified. |
| S2 | RU-1 | 16 | Every publication extends the chain | Scenario | — | 1 | 4 (1/2/1) | Publication always appends to the edition chain rather than replacing or branching it. |
| S3 | RU-1 | 22 | A tampered file fails the edition | Scenario | — | 1 | 3 (1/1/1) | Altering a file that the edition pins makes that edition fail verification. |
| S4 | RU-1 | 27 | A broken chain fails closed | Scenario | — | 1 | 3 (1/1/1) | A chain whose predecessor linkage is wrong is rejected by default rather than accepted with a warning or degraded result. |
| S5 | RU-2 | 34 | The owner reads back what was written | Scenario | — | 1 | 3 (1/1/1) | Content written by the owner is recoverable by the owner unchanged. |
| S6 | RU-2 | 39 | Display paths resolve through names, keys through sids | Scenario | — | 1 | 4 (1/2/1) | Two distinct addressing layers exist: human-visible paths resolve by name, storage keys resolve by sid, so renaming a name does not move the underlying object. |
| S7 | RU-3 | 47 | A stranger reads public content with no key at all | Scenario | — | 1 | 4 (1/1/2) | Public-zone content is readable by a party holding no key whatsoever. |
| S8 | RU-4 | 55 | Self is a flat sea of opaque blobs | Scenario | — | 1 | 4 (1/1/2) | To an outside observer the self zone presents as undifferentiated opaque objects with no structure inferable from it. |
| S9 | RU-5 | 63 | The local owner performs every content operation without a mandate | Scenario Outline | `zone \| operation` | 15 | 6 (2/1/3) | The local owner can perform the whole content-operation set in every zone using only local authority, with no mandate involved. |
| S10 | RU-6 | 91 | Failure before the logical commit point preserves the old bundle byte for byte | Scenario Outline | `store \| boundary` | 12 | 8 (2/1/5) | Any failure occurring before the commit point leaves the bundle bit-identical to its pre-mutation state. |
| S11 | RU-6 | 116 | A successful local transaction publishes content and Gamma together | Scenario Outline | `store` | 2 | 6 (1/1/4) | On success, content and Gamma become visible as one atomic step, never one before the other. |
| S12 | RU-7 | 131 | A bundle operation uses only its narrow opaque cryptographic capability | Scenario Outline | `capability \| protocol_object \| mismatched_object \| observable_result` | 4 | 8 (1/1/6) | Each bundle operation is confined to a single purpose-bound opaque capability and cannot reach any broader cryptographic authority. |
| S13 | RU-7 | 148 | An untrusted path or Store key can never escape its selected root | Scenario Outline | `store \| input_kind \| invalid_input \| filesystem_condition` | 10 | 4 (1/1/2) | No caller-supplied path or storage key, however malformed, can cause access outside the selected root. |

### Examples-table value distributions

- **S9** (`zone | operation`): `public`/`circle`/`self` × 5 each;
  `list`/`read`/`create`/`edit`/`delete` × 3 each. Complete 3 × 5 cross-product.
- **S10** (`store | boundary`): `MemStore` 6, `FsStore` 6. Boundaries
  `cryptography`, `blob preparation`, `index preparation`, `header or wrap`,
  `Gamma validation` appear twice each (once per store); `before state
  replacement` appears once (MemStore only) and `before commit marker or
  reference` once (FsStore only). **Not a full cross-product**: five shared
  boundaries plus one store-specific sixth.
- **S11** (`store`): `MemStore`, `FsStore`.
- **S12**: `capability` = `sign` ×2, `open` ×1, `wrap` ×1.
  `observable_result` repeats verbatim across the two `sign` rows.
- **S13**: `store` = `MemStore` 4, `FsStore` 6. `input_kind` = `display path` 5,
  `Store key` 4, `cold-load key` 1. `filesystem_condition` = `no filesystem
  indirection` 6, and four distinct link/symlink conditions once each. All ten
  `invalid_input` values are distinct.

---

## 3. Vocabulary

Terms the file uses as though already defined. "Defined in file?" means: does
any line of this file say what the term means, as opposed to merely using it.

### 3.1 Load-bearing and undefined

| Term | First appears | Defined in file? | Usage across the file |
|---|---|---|---|
| **Gamma** (capital G) | line 89 (RU-6 title) | No | Eight uses, all in RU-6 and RU-7a: "Gamma head" (97, 121), "Gamma entry" (98, 143, 144), "Gamma validation" (107, 113), "content, roots, manifest and Gamma" (119). It is variously an *entry*, a *head*, a *validation stage*, and a fourth peer of content/roots/manifest. The file never says whether Gamma is a log, a chain, a counter, or an index. It carries the whole of RU-6's atomicity claim. **The single most load-bearing undefined noun in the file.** |
| **mandate**, **mandate counters** | 67, 68 | No | Occurs only in RU-5, three times: "without a mandate" (67), "without consuming mandate counters" (68), and in the scenario name (63). Nothing else in the file mentions mandates, so the file gives no way to tell what a mandate is, what a counter counts, or what consuming one would look like. RU-5's entire value proposition rests on it. |
| **canonical** / **non-canonical** / **staging** | 95 | No | "canonical effect" (95), "the canonical bundle" (96, 98, 152), "the canonical manifest" (121), "staging remains non-canonical" (99). The canonical/staging boundary is the load-bearing distinction of RU-6 — the byte-for-byte guarantee is asserted about the *canonical* bundle only, explicitly leaving staging free to differ. Where the boundary lies is never stated. |
| **narrow** | 61-adjacent; first in text at 67 | No | Two apparently different objects: "the narrow owner capability" (67, RU-5) and "its narrow opaque cryptographic capability" (131, RU-7a; Rule title 129). Whether these are the same capability, one a subset of the other, or unrelated homonyms is not fixed by the text. **A term used in two Rules with possibly two senses.** |
| **durable parity** | 61 (RU-5 title only) | No | Neither "durable" nor "parity" appears in any step of the file. The title asserts a cross-zone equivalence; the steps assert per-row properties and never compare one zone's outcome to another's. What "parity" ranges over — same authority? same journal shape? same latency? — is not stated. |
| **sid** | 39 (scenario name only) | No | **Appears exactly once in the file, in a scenario title, and in no step.** The title contrasts it with "names", implying a stable identifier layer beneath display paths, but the term is never used again. |
| **Ethos** (capital E) | 132 | No | "one Ethos-and-actor session", and as a mismatch dimension at 136 ("a mismatched Ethos, actor, purpose, node, version or recipient"). Capitalised like a proper noun / type name. Nothing in the file says what an Ethos is or how it relates to the identity of line 11. |
| **logical commit point** | 91 (name), 120 | No | RU-6 turns on it. Line 120 says the new state is exposed "at one logical commit point"; line 91 speaks of failure "before" it; line 121 of a crash "at that point". Where it is, and whether it is the same point in MemStore and FsStore, is not stated — and the Examples of S10 name two *different* store-specific last boundaries ("before state replacement" vs "before commit marker or reference"), which suggests it is not. |
| **root** vs **zone** | 151 / 160 | No | S13's `Then` says "out-of-root store access" (151); its Examples say variously "outside the zone" (160) and "outside root" (163, 164, 165). Whether the zone boundary and the root boundary coincide is not stated. Compounded by six rows being `no filesystem indirection` — for MemStore there is no filesystem, so "root" must mean a key-namespace prefix there and a directory there-not. |

### 3.2 Terms used consistently, or defined enough by context

| Term | Where | Note |
|---|---|---|
| **bundle** | throughout | Defined loosely by the Feature description (3–6) as "the subject's entire state as files: indexes, sealed blobs, headers, DID document, and a signed manifest". **But see § 4.1 — line 133 uses `Bundle` capitalised as an actor, which is a second sense.** |
| **edition** | 8, 13, 20, … | Used consistently as a numbered, chained, verifiable snapshot. Ordinals are used (`edition 1`, `edition 2`), and "predecessor" pinning is the chain link. Consistent. |
| **zone** | 45, 53, 61, 64 | Three values are enumerated by usage: `public`, `circle`, `self`. Never listed as a closed set in prose, but S9's Examples supply the enumeration. Consistent. |
| **manifest** | 14, 29, 97, 119, 121, 143, 165 | Consistently the signed top-level document; "the newest manifest" (29) and "the old manifest" (97) imply one per edition. |
| **section** / **folder** | 18, 35, 48, 56 | Consistently the two content-tree levels. `folder` nests (`projets/perso`, `enfance/cicatrices`). |
| **display path** | 39 (name), 156–160 | The human-visible addressing form. Consistent between RU-2 and RU-7b. |
| **Store key** | 161–164 | Capital-S `Store`, the storage-layer addressing form, contrasted with display path. Consistent within S13. |
| **MemStore** / **FsStore** | 103–114, 126–127, 156–165 | The two store backends, used as a data dimension in three of the five outlines. Consistent. |
| **sealed** | 32, 59, 145 | "sealed store", "sealed descriptors", "sealed body". Three different sealed things; the adjective is consistent in meaning (encrypted-at-rest) but its noun varies. |
| **journalized** | 68 | Single use. Implies a journal exists; the journal is named nowhere else in the file. |
| **write-set** | 119 | Single use, "one deterministic write-set". |
| **roots** (plural, lowercase) | 119 | "content, roots, manifest and Gamma" — a *fourth* sense of root, distinct from the filesystem root of S13 and from the zone. Single use. |
| **cold-load key** | 165 | Single use, in one Examples cell only. A third `input_kind` alongside display path and Store key; never appears in a step or a title. |
| **protocol artifact class** | 137 | Single use. Implies a taxonomy of artifact classes that the file does not enumerate. |
| **domain-tagged** | 143, 144 | Applied to manifest and Gamma entry. Implies domain separation in signing; never explained. |
| **trust party** | 5 | Feature description only: "a server is never a trust party". No step mentions a server or a trust party. |
| **DID document** | 4, 14 | Named in the description and pinned by S1. Never otherwise exercised. |

### 3.3 Agent labels — four, with no stated relation

The file names its actors four ways and never fixes their relationships:

- **"I"** — lines 12, 18, 19, 57.
- **"the owner"** — lines 36, 43, 66, 94, 118, and titles 34, 63.
- **"a stranger"** — lines 49, 57, and title 47.
- **"a caller"** — line 150.

Line 57 mixes two in one step: `When I inspect every file of the self zone as a
stranger`. Whether "I" is the owner in lines 12/18/19 and a stranger in 57, and
whether "a caller" in 150 is the owner or an untrusted third party, is not
determined by the text. The question matters for RU-7b in particular, where the
whole claim is about what an *untrusted* input can do.

---

## 4. Structural observations

These are observations about the text. They are pointers for Pass A, not
findings, and are deliberately not numbered in any finding series. Each is
followed, where the natural next thought would require the code, by the
question Pass A should ask instead.

### 4.1 Name-versus-text: what the title claims and what the steps say

- **S6 (39) — "keys through sids".** The scenario name makes a two-part claim.
  Its `When` renames a folder and republishes; its single `Then` (43) asserts the
  owner reads the same section at the new display path. **No step mentions a key
  or a sid.** The second half of the name has no corresponding step text.
  *Question for Pass A: does the step at 43 establish anything about the storage
  key's stability, or only about the display path's resolution?*
- **S8 (55) — "flat sea".** Neither `Then` asserts flatness. Line 58 asserts
  absence of four kinds of name; line 59 asserts owner reconstructability.
  Uniformity of the file layout, blob count, blob size and blob ordering — all
  natural readings of "flat sea" and all structure-leaking channels — are absent
  from the text. *Question: which of the properties in "flat sea of opaque
  blobs" does the assertion set actually reach?*
- **S8 (55) — assertions about things the `Given` never creates.** The `Given`
  (56) establishes a self folder `enfance/cicatrices` and a section `blessure`.
  It creates **no title and no tag**. Line 58 nevertheless asserts that no
  "title or tag" appears anywhere. Two of the four asserted absences have no
  antecedent in the scenario. *Question: is there a fixture-level title/tag that
  the feature text does not mention, and if not, are those two conjuncts
  vacuously true?*
- **S4 (27) — "fails closed".** The `Then` (30) is `edition verification is
  rejected`, textually identical to S3's `Then` (25). "Fails closed" is a claim
  about the *failure mode* — deny-by-default rather than warn-and-continue — and
  rejection alone does not distinguish it from any other rejection. The `Then`
  also does not say *which* edition's verification is rejected, though the
  `Given` (28) establishes two.
- **S1 (10) — "Initialising … publishes".** The `When` is `I initialise its
  bundle` (12); no step publishes. The name asserts that initialisation is (or
  entails) a publication. RU-1's next scenario (16) treats publication as a
  separate act with its own `When` (19). *Question: is initialise-then-edition-1
  the same operation as publish, and does the file's use of "publishes" in a
  title where the step says "initialise" reflect the implementation or elide a
  step?*
- **S13 (148) — "path or Store key" versus three input kinds.** The name
  enumerates two input kinds. The Examples supply three: `display path` (5 rows),
  `Store key` (4 rows), and `cold-load key` (1 row, line 165). The tenth row is
  outside the name's enumeration.
- **S12 (131) / RU-7 title.** The Rule title says "capabilities **and paths**";
  S12 covers capabilities and S13 covers paths. The title is the only place the
  two are joined.

### 4.2 Quantifiers whose scope the scenario does not fix

- **S2 (16) — "Every publication extends the chain".** The scenario performs
  exactly one publication and asserts about exactly one edition (`edition 2`).
  "Every" has no antecedent bounding the population.
- **S9 (63) — "every content operation".** The Examples enumerate five
  operations. Whether five is the whole set is not stated anywhere in the file.
  Likewise line 68's "every mutation".
- **S8 (55) — "every file of the self zone" (57) and "anywhere" (58).** Neither
  is bounded by the text. *Question: how many files does the self-zone fixture
  actually contain, and does "anywhere" range over file contents, file names, or
  both?* (I cannot answer this; it is code.)
- **S8 (55) — "the full tree" (59).** The `Given` builds one two-level folder
  path with one section. What "full" ranges over is not fixed.
- **S10 (91) — "no failed-mutation blob, index, header, wrap or Gamma entry
  exists in the canonical bundle" (98).** A five-way universal negative over the
  whole bundle.
- **S11 (116) — "no reader or reopen observes …" (122).** Universal over readers;
  the scenario introduces no reader.
- **S12 (131) — lines 136–139.** Four blanket assertions that do **not** vary
  with the Examples row and therefore repeat verbatim across all four rows:
  "arbitrary bytes or a mismatched Ethos, actor, purpose, node, version or
  recipient are refused" (136), "a capability for another protocol artifact class
  cannot substitute" (137), "no universal sign, open or wrap capability is
  exposed" (138), "no seed or private key is accepted or returned" (139). Lines
  138 and 139 are existence-denials over the whole API surface, asserted from
  inside a scenario that exercises one capability. *Question: what does the step
  behind 138 enumerate in order to conclude that no universal capability is
  exposed?*
- **S3 (22) — "a pinned file" (24).** Indefinite article; no Examples row and no
  `Given` clause selects which pinned file is altered, and the `Given` (23) does
  not say how many pinned files exist.

### 4.3 `Then` steps asserting conditions the `When` does not create

- **S11 (116), line 121.** `And a crash or lost acknowledgement at that point
  resolves to the complete old or complete new state from the canonical manifest
  and Gamma head`. The scenario's only `When` (118) is `the owner commits a valid
  circle edit` — **no crash and no lost acknowledgement is injected anywhere in
  this scenario.** The `Then` asserts a counterfactual. Note that the sibling
  outline S10 *does* have an injected-failure `Given` (93); S11 does not.
  *Question for Pass A: does the step behind 121 itself induce a crash, does it
  reason about the written artifacts statically, or does it assert nothing
  observable?* This is the observation I would put first.
- **S11 (116), line 120.** "normal completion exposes the complete new state at
  one logical commit point" — "normal completion" is an unqualified hypothetical
  in the same sense.

### 4.4 Disjunctive `Then` steps

A `Then` whose predicate is a disjunction is satisfied by either branch, so the
text alone does not say which branch the system must take.

- Line 99: `staging remains non-canonical and is cleaned **or** recoverably
  resolved with no local-mutation orphan`.
- Line 121: `resolves to the complete old **or** complete new state`.
- Line 122: `no reader **or** reopen observes an individual file replacement
  **or** partial edition`.
- Line 97: `re-reading **or** reopening the "<store>" observes …`.
- Line 138: `no universal sign, open **or** wrap capability is exposed`.

### 4.5 Examples rows that vary a value the `Then` steps do not distinguish

- **S9 (63) — `list` and `read` rows against mutation-shaped assertions.** Six of
  the fifteen rows carry an `operation` of `list` or `read`. For those rows,
  line 68 (`every mutation is journalized without consuming mandate counters`)
  quantifies over an empty set, and line 69 (`the resulting edition reopens and
  verifies from a fresh local store`) presupposes a resulting edition that a
  non-mutating operation would not produce. The `Then` block is written as though
  every row mutates. *Question: for `list` and `read`, what does the step behind
  69 verify — a newly produced edition, or the pre-existing one from the
  `Given`?*
- **S12 (131) — `observable_result` repeats.** Rows 143 and 144 carry the same
  `observable_result` string, `the signature verifies against the public key`,
  and differ only in `protocol_object`/`mismatched_object`. The `Then` at 134 is
  therefore textually identical for both rows.
- **S13 (148) — rows whose `invalid_input` is not itself invalid.** Rows 163,
  164 and 165 supply `e/circle/link-out/index.json`, `e/circle/index.json` and
  `manifest.json`. The last two are well-formed, in-root-looking keys; their
  invalidity lives entirely in the `filesystem_condition` column ("final index
  component links outside root", "signed manifest component links outside root").
  The `When` (150) reads as though `<invalid_input>` carries the attack.
- **S13 (148) — row 162 against the `Then` at 151.** `e/circle/unlisted-object.json`
  under `no filesystem indirection` is, on its face, a key *inside* the root that
  is merely not listed. The `Then` asserts rejection "before any **out-of-root**
  store access". For this row the asserted property and the row's fault mode do
  not obviously align. *Question: is the rejection for row 162 an out-of-root
  rejection or an unlisted-object rejection, and does one step definition cover
  both?*
- **S10 (91) — the grid is not a cross-product.** The sixth boundary differs by
  store (`before state replacement` for MemStore, `before commit marker or
  reference` for FsStore). The two stores are therefore not tested at the same
  final boundary, which is consistent with them having different commit points —
  a point the text asserts nothing about.

### 4.6 Step text repeated across scenarios (shared step definitions implied)

Four step texts occur more than once and would, in Gherkin, resolve to one step
definition each:

| Step text | Lines | Spans |
|---|---|---|
| `a published "<store>" bundle snapshotted byte for byte` | 92, 117, 149 | **crosses RU-6 and RU-7** (both outlines of RU-6 plus S13) |
| `the canonical bundle is byte-for-byte identical to the snapshot` | 96, 152 | **crosses RU-6 and RU-7** |
| `edition verification is rejected` | 25, 30 | within RU-1 (S3 and S4) |
| `a published bundle with section "note1" in circle "projets/perso"` | 35, 40 | within RU-2 (S5 and S6) |

The first two mean RU-7b's confinement guarantee is expressed in RU-6's
transactional vocabulary and shares its fixture and its assertion. An auditor
assigned RU-7 alone will not see the other two uses. **Pass A should be told
that RU-6 and RU-7b share a `Given` and a `Then`.**

Near-duplicates that are *not* textually identical and so would not share a
definition: `a published bundle` (23) vs `a published bundle with …` (35, 40, 48)
vs `a bundle with two editions` (28) vs `an initialised bundle` (17) vs `a bundle
with a self folder …` (56) — five distinct bundle-state `Given`s in eight plain
scenarios.

### 4.7 `Then` steps that restate their `When`

- **S5 (34).** `When the owner reads "projets/perso/note1" from circle` (36) /
  `Then the section body comes back intact` (37). The `Then` adds "intact" over
  the `When`'s "reads"; the whole assertive content of the scenario is the word
  "intact", which the file does not define (identical bytes? identical after
  canonicalisation?).
- **S7 (47).** `When a stranger with no key reads "profil/bio" from public` (49)
  / `Then the section body is readable in clear` (50). The `When` already asserts
  a successful read by a keyless stranger; the first `Then` largely restates it.
  The second `Then` (51) — integrity against the signed edition — is the one that
  adds a distinct property.
- **S9 (63).** `When the owner performs "<operation>" through the common bundle
  operation` (66) / `Then the operation succeeds from the narrow owner capability
  without a mandate` (67). "Performs" in the `When` and "succeeds" in the `Then`
  overlap; the added content is "from the narrow owner capability without a
  mandate", both undefined terms (§ 3.1).

### 4.8 Parameterisation of an assertion as data

Line 134 is `Then "<observable_result>"` — **the entire assertion is a
placeholder**, its text supplied by an Examples column. This is the only step in
the file whose predicate is wholly data-driven. The four values are natural-
language sentences ("the signature verifies against the public key", "the
expected plaintext is recovered only locally", "only the intended recipient opens
the wrapped key"). *Question for Pass A: how does one step definition dispatch on
four different English sentences, and does each sentence map to a distinct
assertion?*

Related: line 145's `mismatched_object` value is `body from a sibling node or
version` and 146's is `line for another node or recipient` — these are
descriptions of a class of object rather than an object, and both are internally
disjunctive.

### 4.9 Feature-level claims with no scenario

The Feature description (3–6) makes claims the scenario set does not name:

- **"a server is never a trust party"** (5). The word *server* appears nowhere
  else in the file. No scenario involves a remote party, a network, or a server.
- **"indexes, sealed blobs, headers, DID document, and a signed manifest"** (3–4)
  as the enumeration of bundle contents. `index` appears at 98, 105, 162–164;
  `header` at 98, 106, 146; `DID document` only at 14; `blob` at 98, 104 and in
  S8's title. There is no scenario about the bundle's file inventory as such.
- **"Editions form a linear, hash-pinned chain"** (4–5). *Linearity* — the
  absence of branching — is asserted by the description and by RU-1's title, but
  no scenario constructs a fork. S4 (27) tests a *wrong* predecessor hash, not a
  *second* successor to the same edition.

### 4.10 Smaller textual notes

- **S2 (16), line 18** creates a section `tagged "toto"`. The tag is never
  mentioned again in that scenario or anywhere else except S8's line 58, which
  asserts tags do *not* appear in the self zone. The tag set in 18 is a circle-zone
  tag and is asserted about nowhere.
- **S2 (16), line 20** hardcodes `edition 2`, which depends on `an initialised
  bundle` (17) being at exactly edition 1 — a coupling to S1's outcome expressed
  only by the ordinal.
- **S6 (39), line 41** renames `the folder "perso"` where the `Given` (40)
  named the path `projets/perso`. The step addresses the leaf component by bare
  name; the file does not say whether folder names are unique across the tree.
- **S6 (39), line 43** — "the owner reads **the same section**". The sameness
  criterion is unstated (same body? same identity? same sid?), and this is the
  step that would have to carry the title's sid claim.
- **RU-1 and "offline".** The Rule title (8) says editions "verify offline", and
  S1's `Then` (13) says `edition 1 verifies offline`. S2, S3 and S4 say only
  "verifies" / "verification". The offline qualifier is asserted in one of four
  scenarios.
- **Alignment/whitespace.** Lines 145–146 and 164–165 have column padding that
  differs from their neighbours (line 165's `cold-load key` widens the second
  column; 164–165's fourth column trails an extra space). Cosmetic; noted only
  so that a diff-based reader does not mistake it for content.
- **Line 108 and 114** are visibly wider than their sibling rows because their
  boundary strings are longer; no content implication.
- **Language.** Content identifiers are French (`projets/perso`, `note1`,
  `toto`, `bio`, `profil`, `intime`, `enfance/cicatrices`, `blessure`) while all
  prose is English. Consistent within the file.

---

## 5. What I could not determine from the text alone

This section is long by design. Almost everything that decides whether this
feature's 51 scenarios are worth their 299 steps is invisible in the text, and
naming the invisible parts precisely is the point of this inventory.

**Whether any step does what its sentence says.** Every observation in § 4 is
about the relationship between one piece of text and another. Not one of them is
a statement about behaviour. In particular I cannot tell, for any step, whether
it asserts, whether it asserts what it says, or whether it is a no-op.

**Step-definition sharing beyond textual identity.** § 4.6 lists the four
textually identical repeats. Gherkin step definitions match by *regex*, so
steps that differ textually may still share one definition, and a parameterised
definition may absorb many of the 61 authored step lines. The true sharing graph
is in the step-definition files, which I did not open. *Pass A should build the
real step-to-definition map; my table is only its textual lower bound.*

**Line 134's dispatch.** Whether `Then "<observable_result>"` resolves to four
distinct assertions, one assertion with a switch, or a single string comparison.

**Fixture contents, and therefore every quantifier in § 4.2.** How many pinned
files S3's bundle has; how many files S8's self zone contains; whether S8's
fixture carries a title or a tag; how many editions "a published bundle" has;
whether S9's five operations are the whole content-operation set.

**The three-way meaning of "root".** Whether the MemStore root, the FsStore
directory root, the zone boundary, and line 119's "roots" are one concept, two,
or four. The text uses all four words; only the code says which are the same.

**Whether S9's `list`/`read` rows assert anything.** § 4.5. The two mutation-shaped
`Then` steps applied to six non-mutating rows may be vacuously satisfied, may be
skipped, or may be genuinely meaningful in a way the text does not reveal.

**Whether S11's line 121 induces a crash.** § 4.3. This is the largest single
gap between what a `Then` claims and what its scenario's `When` sets up, and it
is entirely unresolvable from the text.

**What "injected failure at `<boundary>`" injects.** S10's six boundary names are
labels. Whether they correspond to distinct code paths, whether the injection
happens before or after the named stage, and whether the MemStore and FsStore
sixth boundaries ("before state replacement", "before commit marker or
reference") are the same point under two names, are all code questions.

**Whether "byte-for-byte identical to the snapshot" is checked byte-for-byte.**
Lines 96 and 152 make the strongest assertion in the file. The comparison's
scope — whole bundle, canonical subtree only, metadata included or excluded —
is not in the text, and § 3.1 records that "canonical" is undefined, so the
assertion's *subject* is undefined too.

**Whether the negative assertions at 136–139 are enumerative.** "No universal
sign, open or wrap capability is exposed" (138) and "no seed or private key is
accepted or returned" (139) are claims about the shape of an API. Whether the
steps behind them inspect a type surface, attempt a list of forbidden calls, or
assert nothing, is invisible here.

**Whether "durable parity" is checked.** § 3.1: no step contains either word.
Whether some step compares zone outcomes to each other, or whether parity is
merely implied by the Examples grid covering three zones, cannot be told.

**What Gamma is.** § 3.1. Without it, RU-6's two outlines — 14 of the 51
scenarios and 108 of the 299 steps, the largest unit in the feature — cannot be
assessed for adequacy at all.

**What a mandate is.** § 3.1. Same problem for RU-5's 15 scenarios.

**Whether the sid layer exists.** § 3.1 and § 4.1. The word appears once, in a
title. Either there is a sid concept the feature file forgot to exercise, or the
title names something the implementation does not have. The text cannot say
which.

**Whether S13's ten rows exercise ten distinct rejection paths.** Four rows are
classic path-traversal forms, one is an absolute path, two are normalisation
forms (`./`, `//`), three involve symlinks at different components, one is an
unlisted in-root object. Whether these hit ten code paths or one early
normaliser is a code question — but § 4.5's note that row 162's fault mode does
not match the `Then`'s "out-of-root" wording is a *textual* pointer to where to
start looking.

**Coverage of the Feature description's claims.** § 4.9 shows that "a server is
never a trust party" has no scenario. Whether that claim is tested elsewhere in
the suite, or is untested, is outside this file.

**Whether "I", "the owner", "a stranger" and "a caller" are the same or
different principals** (§ 3.3), and in particular whether S13's "a caller" is
authenticated. If it is the owner, S13 tests confinement of a trusted caller's
malformed input; if it is untrusted, it tests an attack. The two are different
guarantees and the text does not choose.

**Prior findings and follow-ups.** I was deliberately not told what `d-bundle`
owes. Nothing in this inventory was written toward or away from them.

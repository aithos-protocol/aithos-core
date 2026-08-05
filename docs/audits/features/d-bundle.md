# Implementation audit — `d-bundle.feature`

## 1. Metadata

| Field | Value |
|---|---|
| Audited feature | `features/d-bundle.feature` (`@d-bundle`) |
| Round | 1 — initial audit, orchestrated mode |
| Date | 2026-08-04 |
| Observed revision | `d9120d7e0d154cee517b983bf7b6cac0cf8e8096` (`d9120d7`) |
| Branch | `codex/audit-d-bundle` |
| Orchestrated run | `2026-08-04-r7` (`features/.agents/orchestrator/runs/2026-08-04-r7/`) |
| Worktree state | `rust/`, `spec/` and `vectors/` are **byte-identical** to `d9120d7`: `git diff d9120d7..HEAD -- rust/ spec/ vectors/` is empty. `features/d-bundle.feature` is **not**: commit `b19c7fd` inserted 99 marker lines into it and this note's round-2 and round-3 marker edits bring the total to 108, changing no scenario, rule, step or `Examples` row, and moving every line number below the first insertion. Round 1 of this note claimed the audited bytes and the current bytes were the same bytes; that was false for the feature file and is corrected here (§8.4) |
| Feature file, audited | `d9120d7:features/d-bundle.feature`, 165 lines, sha256 `59a6f361598de459fa063e7bff9915427c5e3d70423c20204d26f294b618c8b5` — the numbering used by `INVENTORY.md`, by all six Pass A reports and by `frozen.json` |
| Feature file, current | `features/d-bundle.feature` as delivered by this note, 273 lines, sha256 `2780ed65348776e64406d2d2d9a63932c360b00b25cb122351bd4deb2abf0deb` |
| Line-citation convention | **every `d-bundle.feature` line number in this note is a line of the audited file**, never of the current one. §5.2 translates each to its current line. See the block below |
| Scope | the semantic truth of the 51 scenarios of `d-bundle.feature`; seven `Rule` blocks |
| Finding prefix | `DBND-*` (`docs/audits/features/README.md`, § *Convention*) |
| Domain | `features/.agents/d-bundle/DOMAIN.md` |
| Pass A freeze | `pass-a/frozen.json`, sha256 `d3f8f33324c48e3c12bfd425b19238b0ba80bc502a662a5255073e980c3a685b` |
| Post-freeze measurements | `VERDICTS.md` (orchestrator), `EVIDENCE.md` (run `2026-08-04-r7`) |
| Language | English, matching `a-identity.md`, `b-derivation.md` and the six Pass A reports. The structure is `c-headers.md`'s; only the language differs |
| Warden | `features/.agents/orchestrator/runs/2026-08-04-r7/warden.md`. Round 1 of this note was **`INVALIDATED`**; all six breaches were real and are repaired here, each named at the point of repair |

### The authorities this note rests on, and where they are

Round 1 of this note asserted that the Pass A reports were committed in this
repository. **At the time of writing they were not**, and the warden invalidated
the cycle for it: a reader could not have followed a single citation back to
what an auditor actually wrote. They are committed now, each with its sha256 in
the run ledger, and this note cites these paths and no others.

| Authority | Path under `features/.agents/orchestrator/runs/2026-08-04-r7/` |
|---|---|
| Pass A freeze | `pass-a/frozen.json` |
| Pass A reports (six, covering seven units) | `pass-a/RU-1.md`, `RU-2.md`, `RU-3.md` (holds RU-3 **and** RU-4), `RU-5.md`, `RU-6.md`, `RU-7.md` |
| Structural inventory (role I1) | `INVENTORY.md` |
| Post-freeze measurements and panel rulings | `VERDICTS.md` |
| Evidence index | `EVIDENCE.md`; transcripts under `evidence/` |
| Warden verdict on round 1 of this note | `warden.md` |

### How this note cites the feature file, and why

The marker commit `b19c7fd` inserted 99 lines into `features/d-bundle.feature`
and shifted every line below the first insertion. Round 1's line citations were
written against the audited file and published against the marked one, so they
pointed at the wrong text: `:139`, the clause carrying a P1, had become an
`Examples` row, and `:148`, the confinement outline, had become the row `| self |
read |`. That is the warden's second breach.

**Re-anchoring to `HEAD` would fix it once and break it again**, because the
correction lots of §11 will move these lines a second time and every later
marker edit a third. So this note does the opposite, and says so here rather
than leaving a reader to discover it:

> **Every `features/d-bundle.feature` line number in this note — whether written
> `features/d-bundle.feature:99` or, inside a citation cluster, as a bare
> `` `:99` `` — is a line of the file *as audited*, at `d9120d7`, sha256
> `59a6f361…`. It is never a line of the current file.** That numbering is the
> one `INVENTORY.md`, all six Pass A reports and `frozen.json` use, so a reader
> holding any of them can follow a citation straight through. **§5.2 translates
> every audited line to its line in the current file**, and the translation is a
> constant offset per scenario block, so it stays correct without being rewritten
> when the markers move again.

Line numbers into `rust/`, `spec/` and `vectors/` are unaffected: those trees are
byte-identical to `d9120d7`.

### Renumbering

Pass A ran six blind auditors over seven units, unable to coordinate
identifiers, so the frozen findings are numbered `DBND-101`…`DBND-715` by
hundred-block. This note renumbers them into one `DBND-001`… series, ordered by
review unit and then by severity, and publishes the complete old→new map in
§6.0.

**Rounds 2 and 3 changed the membership and never renumbered.** Round 2 removed
four findings; round 3 restored two of them by measurement. The series runs
`DBND-001`…`DBND-039` with **three absent** — `DBND-020`, `DBND-030`,
`DBND-035` — plus `DBND-026`, un-retired under its own number, and `DBND-040`,
appended out of unit order because it never held a reconciled identifier. §6.0
states the ordering rule. Renumbering at any of these steps would defeat the
purpose of §6.0: rounds 1 and 2 are committed and a reader will arrive carrying
their numbers, exactly as readers arrive carrying the Pass A numbers. **No
`DBND-*` identifier retired at any stage is ever reused for anything else.**

## 2. Method provenance

Orchestrated mode. Pass A isolation was **material**: each of the seven review
units ran against a `git archive` extract of `d9120d7` with no `.git` directory,
no run journal, no ledger and no prior verdict on any feature. No Pass A agent
executed a gate; the orchestrator alone runs gates, writes transcripts and
issues an `evidence_id`. This Pass B role likewise ran **no gate, no test and no
`cargo` command**: every behavioural claim below cites an `evidence_id` from
`EVIDENCE.md`, and every claim that does not cite one says so in those words.

| Unit | `Rule` | Line | Scenarios | Steps | Pass A contamination |
|---|---|---|---|---|---|
| RU-1 | Editions chain and verify offline | `:8` | 4 | 14 | none |
| RU-2 | Content round-trips through the sealed store | `:32` | 2 | 7 | none |
| RU-3 | The public zone reads without any key | `:45` | 1 | 4 | none |
| RU-4 | The self zone leaks no structure | `:53` | 1 | 4 | none |
| RU-5 | Owner operations have durable parity across all three zones | `:61` | 15 | 90 | none |
| RU-6 | A local mutation commits state and Gamma as one transaction | `:89` | 14 | 108 | none |
| RU-7 | Local capabilities and paths stay narrow | `:129` | 14 | 72 | none |

**Six auditors, seven units.** RU-3 and RU-4 were held by one auditor and read
as one sitting, per `INVENTORY.md` § 1.8; their single report is
`pass-a/RU-3.md`, and no `RU-4.md` exists. `pass-a/frozen.json` says *"Seven
auditors"* and is **wrong**. It is hashed, cited by that hash, and therefore
**not edited**: a freeze that can be corrected after the fact is not a freeze.
The ledger carries the superseding correction and this note states the true
number wherever it states a number. Round 1 of this note disclosed the six
correctly here and then said "seven" in §6.1 and twice in §15; that is the
warden's third breach, repaired at all three sites.

**What happened after the freeze, in order.** The 48 frozen findings were
measured in three ways, and the three are kept visibly distinct on every finding
block below because they are not worth the same thing:

1. **Nineteen distinct code edits** were applied to a clean tree at `d9120d7`
   and reverted before the next, producing **twenty transcripts** — eighteen
   experiments under the ledger's own grouping, since one mutant
   (`gamma.rs:300`) was measured by two commands and one entry
   (*RU-5 parameter reachability*) is two separate edits. All but one landed
   exactly as predicted. Fifteen findings are **confirmed by transcript**.
   Round 1 of this note reported *"seventeen mutants"*, inheriting an
   undercount from the ledger's prose; that is the warden's fourth breach,
   corrected here and in §4.2 and §8.5. No evidence identifier, verdict or
   counter changes with it.
2. **An adversarial panel** was run, by owner ruling, only on the ten P1/P2
   findings carrying no confirmed mutant — the other fifteen are settled by a
   transcript and a vote adds nothing to one. Each refuter saw the code and
   **one** finding statement: not the report, not the other findings, not the
   mutant results, not the author, and not that any other refuter existed. Each
   refuter was instructed to refute and to answer *refuted* when uncertain.
   Round 1 reported six killed and four surviving. The warden then ruled the
   four **under-panelled**, on the orchestrator's own stated rule that a
   refutation resting on judgement rather than on a checkable fact gets the full
   panel — a rule that had not been applied to them. A second refuter was run on
   all four and all four fell on facts in the source. Round 3 then **tested three
   of the ten refutations against the gate instead of reading them**, and two of
   the three did not survive their own transcript. The panel result is
   **ten examined, eight refuted, two restored by measurement** (§7).
   **No finding in this note carries the evidential state *survived the
   adversarial panel*, because none survived one** — the two restorations were
   restored by a mutant, not by outlasting an attack, and they carry
   *confirmed by transcript* like any other measured finding.
3. **The 23 P3 findings had neither.** That was a budget decision, recorded in
   `VERDICTS.md` § D — the panel budget was spent where it changes what a
   corrector does. This note rules on them **on the record alone** and says so
   on each one. It does not pretend they were tested.

**Contamination disclosure for this pass.** The Pass B role reads git history,
the specification, the other features' published audits, `STATE.md`,
`QUEUE.yaml`, the mutant transcripts and the panel verdicts. That is the
definition of Pass B. None of it was visible to any Pass A unit. Numbers taken
from Pass A, from the orchestrator and from round 1 of this note that did not
survive re-checking are corrected in §8.4 rather than repeated.

**What the warden checked and found holding**, recorded because an invalidation
notice is not a verdict on everything it did not overturn: all 148 evidence
identifiers resolve; all 33 transcripts match their digests; **zero `ev-` tokens
appear in any Pass A report**, and no transcript of any kind exists anywhere in
the Pass A window, so no auditor could have been issued evidence even had it
asked; isolation is clean across all seven extracts, with no `.git` and no run
journal in any of them; the freeze was written 6 min 15 s before the first
mutant, hashes match, single commit, never amended; the seven units sum to
51 scenarios and 299 steps and the 48 identifiers partition cleanly across them;
all 22 counters cross-check against the ledger; and sixteen tables, 191 table
lines and all thirteen Gherkin markers were read separately from the prose for
the disclosure gate.

## 3. Verdict

**The production code this feature covers was read and found intact at
`d9120d7`, and no finding in this note asks for a correction to
`aithos-core`.** What is weak is not the product; it is the proof. Four facts
carry the round.

1. **Three scenarios pass without proving what they claim.** `A broken chain
   fails closed` stays green with the predecessor-hash comparison deleted
   (`ev-d1fc33b5`, 51/51). `Display paths resolve through names, keys through
   sids` stays green with `rename_folder` reduced to appending an alias, the old
   name surviving (`ev-f7261aa9`, 51/51). `Self is a flat sea of opaque blobs`
   stays green with the `e/self/` prefix made invisible to listing — **the
   scenario passes having inspected nothing** (`ev-0b4e1076`, 51/51).

2. **Two clauses of the contract are asserted against compile-time constants.**
   `without consuming mandate counters` (`:68`) is `assert_eq!(0, 0)`:
   `mandate_counter_delta` is written as the literal `0` at `cucumber.rs:3549`
   and computed nowhere. `no seed or private key is accepted or returned`
   (`:139`) is `assert!(!false)`: `secret_material_exposed` is written `false` at
   exactly four sites and nowhere else. Both are P1, and both are confirmed by a
   mutant that widens the surface and leaves the gate green (`ev-19a635cf`,
   `ev-ed18d7ef`).

3. **The largest unit is also the best-proved one, and it is proved by real
   fault injection.** RU-6's `Failure before the logical commit point preserves
   the old bundle byte for byte` drives twelve rows through a `Store` decorator
   that returns a genuine `io::Error`, compares a complete key→bytes map three
   times, and re-runs the product's own `Bundle::verify()` after a reopen. The
   control that licenses it — `MemStore` rollback made to **commit** what it
   should discard — is red (`ev-f0125e0b`). Credit where it is due: this is the
   only interrupted-state observation in the feature.

4. **A negative result, measured rather than inferred.** This repository carries
   360 Gherkin lines resolving to 19 cached-verdict step definitions, and three
   auditors were primed to test first whether their `Scenario Outline` rows
   execute distinct bytes. RU-5 reports 15 of 15 distinct, RU-7 14 of 14, RU-6
   found real fault injection. **Both columns of the largest outline were then
   measured**: `zone` `self`→`vault` is red 1/51 (`ev-f0658ee9`) and `operation`
   `list`→`enumerate` is red 1/51 (`ev-de8fa887`). The proxy class does not reach
   this feature. That is recorded as a negative result, not as an absence
   (§10).

**Reconciled count: 37 findings — 2 P1, 15 P2, 20 P3** — from 48 frozen: **eight
removed by the panel and two restored by measurement** (§7), five merged into two
(§8.1), one split into two (§8.2). Nothing was invented in Pass B; four findings
carry a transcript they did not have at the freeze, two of them from a control
run designed for something else (§8.5).

**The panel examined ten findings, and the count moved three times.** Round 1 of
this note published six removals and four survivors. The warden ruled the four
under-panelled and a second refuter killed all four, so round 2 published ten
removals and drew the inference in bold: *on this feature, a P1 or P2 finding
that met a serious adversary did not survive it.* **That inference was wrong, and
it was wrong because it was drawn from citations rather than from transcripts.**
Two of the ten refutations were then tested and both failed: `DBND-026` is
restored under its own number by `ev-f7ee3968`, and `DBND-504` is restored,
narrowed, as `DBND-040` by `ev-f18d4843`. **Ten tested, eight refuted, two
restored by measurement.** The sentence that replaces the wrong inference is
narrower and is a rule rather than a mood, and it is in §8.5.2.

Of the 37 findings published here, **seventeen are confirmed by a mutant** and
**twenty rest on reading alone**. The corrector's lot — every P1 and P2 — is
**seventeen findings**, and every one of them carries a transcript.

**Two debts this cycle owes but does not open**: `chdr-028` and
`chdr-016-grant-path`. `d-bundle` is first in line for both by the `order:` list
and neither is discharged here; §9 records why and what would discharge them.

### Exact counters

Cited by `evidence_id`, never copied from a document.

```
ev-6a76a789 — 1 feature / 7 rules / 51 scenarios (51 passed) / 299 steps (299 passed)
ev-5f523aae — 1 feature / 7 rules / 51 scenarios (51 passed) / 299 steps (299 passed)
```

`ev-5f523aae` is the baseline against which every mutant below differs. Its
counters match the contract file exactly — 7 `Rule`, 13 authored scenario blocks
expanding to 51, 299 steps — which is the proof of selection and of execution.
The expansion arithmetic, so it can be re-checked without re-deriving it: 8
plain scenarios contribute 8; the five outlines contribute 15 + 12 + 2 + 4 + 10
= 43; 8 + 43 = 51. Steps: 29 authored step lines across the 8 plain scenarios,
plus (6×15) + (8×12) + (6×2) + (8×4) + (4×10) = 270; 29 + 270 = 299.

**No scenario of this feature is tagged.** `main()`
(`cucumber.rs:20017-20040`) calls `fail_on_skipped()` then
`filter_run_and_exit`, and its filter excludes only `@wip`, at feature, rule and
scenario level. `d-bundle.feature` carried one tag, `@d-bundle`, at freeze time;
this note adds `@audit-partial` and `@dbnd-*` marker tags, which no filter
reads. An unresolved step phrase in this feature is an error, not a silent skip.

## 4. Reproduced evidence

The auditor role executes no gate in orchestrated mode (`PROCESS.md`
§ *Orchestrated gate execution*). Ownership of the gate does not move; only its
execution does. Transcripts:
`features/.agents/orchestrator/runs/2026-08-04-r7/evidence/`.

### 4.1 Baseline and controls

| `evidence_id` | Verdict | Counters | What it establishes |
|---|---|---|---|
| `ev-6a76a789` | GREEN | 51/51 | the frozen baseline cited by `frozen.json` |
| `ev-5f523aae` | GREEN | 51/51 | the run-`r7` baseline every mutant differs from |
| `ev-5474b889` | RED | 20/51, 31 failed | `publish` stops pinning its predecessor. **Not green: there is no hidden P1 behind `DBND-001`** |
| `ev-f0125e0b` | RED | 47/51, 4 failed | `MemStore` rollback **commits** what it should discard. Licenses every positive statement about RU-6's byte comparison |
| `ev-f0658ee9` | RED | 50/51 | `d-bundle.feature:87`, `zone` `self` → `vault` |
| `ev-de8fa887` | RED | 50/51 | `d-bundle.feature:73`, `operation` `list` → `enumerate` |
| `ev-1eefbb66` | RED | 50/51 | `d-bundle.feature:143`, `observable_result` cell replaced. Control for `DBND-031` |
| `ev-bec6b91e` | RED | — | `aithos-core`'s `entries_rebuild_byte_for_byte` under the `DBND-018` mutant |

### 4.2 The confirming mutants — fourteen rows, fifteen findings

Each was named with its predicted outcome by its auditor **before any transcript
existed**, in a report frozen under `frozen.json`.

| Mutant | `evidence_id` | Result | Confirms |
|---|---|---|---|
| `bundle.rs:1726` — predecessor-hash check deleted from `verify` | `ev-d1fc33b5` | GREEN 51/51 | `DBND-001` |
| `bundle.rs:1749` — the whole flat-pin loop deleted | `ev-de2706a8` | GREEN 51/51 | `DBND-002` |
| `bundle.rs:1280` — the `blob_sha` guard deleted from the keyless public read | `ev-c7f65638` | GREEN 51/51 | `DBND-003` |
| `seal.rs` — `blob_seal`/`blob_open` reduced to a length-preserving identity | `ev-23aeba39` | RED 50/51 | `DBND-007` |
| `bundle.rs:1605` — rename appends an alias, the old name survives | `ev-f7261aa9` | GREEN 51/51 | `DBND-008` |
| `log.rs:201` — the self zone logs like the public zone | `ev-f1718be8` | GREEN 51/51 | `DBND-013` |
| `lib.rs:348` — the self prefix invisible to listing, every byte left in place | `ev-0b4e1076` | GREEN 51/51 | `DBND-014` |
| `gamma.rs:300` — every owner entry declares `authorized_by`/`authorized_via` | `ev-19a635cf` + `ev-bec6b91e` | RED 50/51 / RED | `DBND-018` |
| `cucumber.rs:3420` — the five `owner_content_operation` call sites take a stranger key | `ev-b6a36f72` | RED 39/51 | `DBND-019` |
| `lib.rs:906` — the whole `FsStore` crash-recovery path replaced by `self.transaction = None` | `ev-7caa8332` | GREEN 51/51 | `DBND-025` |
| `session.rs` — a public `manifest_private_key()` accessor added | `ev-ed18d7ef` | GREEN 51/51 | `DBND-029` |
| `d-bundle.feature:143` — the `mismatched_object` cell replaced by a string that exists nowhere | `ev-3fa9d172` | GREEN 51/51 | `DBND-031` |
| `session.rs` — a `sign_any()` universal byte-signing oracle, named around the grep | `ev-794d59c3` | GREEN 51/51 | `DBND-032` |
| `lib.rs` — `validate_display_path` reduced to `Ok(())` | `ev-2d2ebd1b` | GREEN 51/51 | `DBND-033`, `DBND-034` |

**Campaign totals, corrected.** Nineteen distinct code edits over twenty
transcripts, eighteen experiments under the ledger's grouping: `gamma.rs:300` is
one edit measured by two commands (`ev-19a635cf`, `ev-bec6b91e`), and the
*RU-5 parameter reachability* entry is two edits (`ev-f0658ee9`, `ev-de8fa887`).
The table above has **fourteen rows and fifteen evidence identifiers**, one row
citing two, and it confirms fifteen findings because `ev-2d2ebd1b` confirms two.
Round 1 headed it *"the fifteen confirming mutants"* over fourteen rows and
reported the campaign as seventeen; both are corrected here (§8.4).

**The single partial.** `ev-23aeba39` is the one edit that did not land
exactly. Its author predicted `:34`, `:39` and `:47` green and `:55` red; the run
is 50/51 with the single casualty in RU-4 — which is the prediction, but it is
recorded as partial because the prediction named four scenarios and the
transcript resolves one. The finding it serves (`DBND-007`) is confirmed in the
direction that matters: the two RU-2 scenarios stay green against a cleartext
store.

**Two mutants were designed by their authors expecting to be caught, and were.**
`ev-5474b889` (`publish` stops pinning) and `ev-f0125e0b` (`MemStore` rollback
commits) are both red. An audit that only runs mutants it expects to survive is
measuring its own confidence.

No run other than those in `EVIDENCE.md` is claimed. Every behavioural statement
in this note either cites one of them or is labelled *on the record alone*.

## 5. Scenario matrix

Statuses per `docs/audits/features/README.md`, § *Coverage statuses*. "What the
assertion actually compares" is the column a green runner does not give you.

| # | Line | Scenario | Rows | Status | Production path | What the assertion actually compares |
|---|---|---|---|---|---|---|
| S1 | `:10` | Initialising a bundle publishes a verifiable first edition | 1 | `PARTIAL` | `Bundle::init` (`bundle.rs:558-640`) → `publish_at` (`:628`) → `Bundle::verify` | a bare `verify().expect(…)` on the live object over the same in-process `MemStore`; the ordinal `1` and the word `offline` reach no assertion; `:14` is subsumed by `:13` |
| S2 | `:16` | Every publication extends the chain | 1 | `PARTIAL` | `Bundle::publish` → `publish_at` → `Manifest::chain_hash` | `height == 2` plus a `prev_hash` recomputed independently from `manifests/1.json`'s stored bytes. The strongest scenario in the feature; killed by `ev-5474b889` |
| S3 | `:22` | A tampered file fails the edition | 1 | `PARTIAL` | `store.put` through the `pub store` field; `Bundle::verify` | `verify().is_err()`, error never inspected. The tampered object is re-derived twice, so the rejection has causes other than the pin (`ev-de2706a8` green) |
| S4 | `:27` | A broken chain fails closed | 1 | **`SEMANTIC_FALSE_POSITIVE`** | forged tip at `height+1`; `Bundle::verify` | `verify().is_err()`, produced by an unpinned-stray check, not by the chain link. `ev-d1fc33b5` green with the chain comparison deleted |
| S5 | `:34` | The owner reads back what was written | 1 | `PARTIAL` | `read_section` → `resolve_clear` → `open_blob_v` → `blob_open` | the returned body against the module constant `BODY` — anchored, not a round-trip on its own `When`. Nothing observes the resident bytes (`ev-23aeba39` green) |
| S6 | `:39` | Display paths resolve through names, keys through sids | 1 | **`SEMANTIC_FALSE_POSITIVE`** | `Bundle::rename_folder` (`bundle.rs:1571-1611`) | a read at the new path equals `BODY`. No sid, no `blob_sha`, no check that the old path stopped resolving. `ev-f7261aa9` green with the old name surviving |
| S7 | `:47` | A stranger reads public content with no key at all | 1 | `PARTIAL` | `Bundle::public_read` (`bundle.rs:1264-1289`), signature admits no key | the body against `PUB_BODY` — real. The second `Then` is a whole-bundle `verify()` that never touches the value read (`ev-c7f65638` green) |
| S8 | `:55` | Self is a flat sea of opaque blobs | 1 | **`SEMANTIC_FALSE_POSITIVE`** | `store.list("e/self/")` + `store.get`; `zone_tree` → `self_walk` | five `!contains` over a `String` the scenario never constrains. `ev-0b4e1076`: green with `w.inspected == ""` — it passes having inspected nothing |
| S9 | `:63` | The local owner performs every content operation without a mandate | 15 | `PARTIAL` | `Bundle::owner_content_operation` (`bundle.rs:444`), five variants × three zones | nine mutating rows assert a changed post-state after a real `FsStore` reopen — genuinely differential. Twelve of fifteen rows bind the owner capability; three do not (`ev-b6a36f72`). The mandate clause is `assert_eq!(0, 0)` |
| S10 | `:91` | Failure before the logical commit point preserves the old bundle byte for byte | 12 | `PARTIAL` | `CoreAtomicFaultStore` decorator; `owner_content_operation`; `Bundle::verify` after reopen | a complete key→bytes map compared three times, `before == after && before == reopened`. Real interrupted-state observation; scope stops at `canonical_base()` |
| S11 | `:116` | A successful local transaction publishes content and Gamma together | 2 | `PARTIAL` | same fixture, one uninterrupted `Edit`; no fault store | `:119` requires four independent object classes including `gamma/` to have changed — a real positive control. `:120`–`:122` are two tautologies and one bit asserted twice, all after an uninterrupted run |
| S12 | `:131` | A bundle operation uses only its narrow opaque cryptographic capability | 4 | `PARTIAL` | `LocalSession` + four typed capability handles (`session.rs:41-76`) | a real per-row positive control (`operation_succeeded`), plus three harness-constant echoes, one grep of `session.rs` as text, one `assert!(!false)` and one column that reaches no code |
| S13 | `:148` | An untrusted path or Store key can never escape its selected root | 10 | `PARTIAL` | `validate_display_path`, `validate_store_key`, `FsStore::checked_join` (`lib.rs:553-579`) | six `FsStore` rows genuinely discriminate against a real per-segment `symlink_metadata` walk. Four `MemStore` rows cannot tell a grammar refusal from an absent section (`ev-2d2ebd1b` green) |

**Totals: 0 `PROVEN`, 10 `PARTIAL`, 3 `SEMANTIC_FALSE_POSITIVE`, 0
`NOT_COVERED`, 0 `PROXY`.**

**Why no scenario is `PROXY`.** `PROXY` designates a scenario that consumes a
shared verdict without executing its own case. Every one of the 13 authored
blocks builds its own fixture and executes its own parameters; the eight
process-lifetime `OnceLock` verdicts of `cucumber.rs:1119-1129` are reached by no
step of this feature. Three auditors reproduced that search independently rather
than inheriting it from `DOMAIN.md`, and it is now measured on both columns of
the largest outline. §10 records it as the negative result it is.

### 5.1 Row-level differences the block-level status hides

| Outline | Rows | Measured difference |
|---|---|---|
| S9 (`:63`) | `public/list`, `public/read`, `circle/list` | the three rows that survive `ev-b6a36f72`, a stranger key at all five `owner_content_operation` call sites. Their executed path never receives `owner_kex`: `zone_entries_with_owner_kex` routes every zone but `Self_` to `clear_zone_entries` (`bundle.rs:1430-1443`), and `read_section_with_owner_kex` routes `Zone::Public` to `public_read` (`:1236-1237`). Twelve of fifteen rows died; these three are the composition the auditor predicted, row for row |
| S10 (`:91`) | `MemStore \| cryptography`, `MemStore \| index preparation` | the two rows that survive `ev-f0125e0b`. Both faults fire on the same first write, `e/circle/index.json`, before it reaches the overlay, so a rollback made to commit has nothing to commit. The other four `MemStore` rows die — three at `:95` with `pinned file altered: e/circle/index.json`, one at `:96` on the byte comparison |
| S13 (`:148`) | the four `MemStore` rows | survive `ev-2d2ebd1b` because `resolve_clear` returns `Err(InvalidPath)` for a path naming no existing section whether or not the grammar refused it first |
| S13 (`:148`) | `:160`, `:164`, `:165` | reach `checked_join`'s per-segment `symlink_metadata` walk at an intermediate component, a final component and the signed manifest. Genuinely discriminating; the panel established this against `DBND-708` (§7) |

### 5.2 Line index — audited line to current line

Every `d-bundle.feature` line number in this note is a line of the **audited**
file (`d9120d7`, 165 lines, sha256 `59a6f361598de459fa063e7bff9915427c5e3d70423c20204d26f294b618c8b5`).
The marker edits — `b19c7fd`, plus this note's round-2 revision of four blocks
and round-3 revision of two — insert 108 comment and tag lines above scenario
headers and change nothing else, so the shift is a **constant offset within each
block**. To translate any cited line, find the block it falls in and add the
offset. The current file is 273 lines, sha256
`2780ed65348776e64406d2d2d9a63932c360b00b25cb122351bd4deb2abf0deb`.

| Block | Audited | Current | Offset |
|---|---|---|---|
| Feature header and description | `:1`–`:7` | `:1`–`:7` | +0 |
| `Rule:` Editions chain and verify offline | `:8` | `:8` | +0 |
| `Scenario:` Initialising a bundle publishes a verifiable first edition | `:10`–`:14` | `:17`–`:21` | **+7** |
| `Scenario:` Every publication extends the chain | `:16`–`:20` | `:28`–`:32` | **+12** |
| `Scenario:` A tampered file fails the edition | `:22`–`:25` | `:40`–`:43` | **+18** |
| `Scenario:` A broken chain fails closed | `:27`–`:30` | `:52`–`:55` | **+25** |
| `Rule:` Content round-trips through the sealed store | `:32` | `:57` | **+25** |
| `Scenario:` The owner reads back what was written | `:34`–`:37` | `:65`–`:68` | **+31** |
| `Scenario:` Display paths resolve through names, keys through sids | `:39`–`:43` | `:77`–`:81` | **+38** |
| `Rule:` The public zone reads without any key | `:45` | `:83` | **+38** |
| `Scenario:` A stranger reads public content with no key at all | `:47`–`:51` | `:93`–`:97` | **+46** |
| `Rule:` The self zone leaks no structure | `:53` | `:99` | **+46** |
| `Scenario:` Self is a flat sea of opaque blobs | `:55`–`:59` | `:109`–`:113` | **+54** |
| `Rule:` Owner operations have durable parity across all three zones | `:61` | `:115` | **+54** |
| `Scenario Outline:` The local owner performs every content operation without a mandate, with its `Examples` | `:63`–`:87` | `:129`–`:153` | **+66** |
| `Rule:` A local mutation commits state and Gamma as one transaction | `:89` | `:155` | **+66** |
| `Scenario Outline:` Failure before the logical commit point preserves the old bundle byte for byte, with its `Examples` | `:91`–`:114` | `:168`–`:191` | **+77** |
| `Scenario Outline:` A successful local transaction publishes content and Gamma together, with its `Examples` | `:116`–`:127` | `:199`–`:210` | **+83** |
| `Rule:` Local capabilities and paths stay narrow | `:129` | `:212` | **+83** |
| `Scenario Outline:` A bundle operation uses only its narrow opaque cryptographic capability, with its `Examples` | `:131`–`:146` | `:227`–`:242` | **+96** |
| `Scenario Outline:` An untrusted path or Store key can never escape its selected root, with its `Examples` | `:148`–`:165` | `:256`–`:273` | **+108** |

The two citations the warden named are the worked examples. `:139` — *no seed or
private key is accepted or returned by the bundle operation*, the clause carrying
the P1 `DBND-029` — is **`:235`** in the current file; read as a current line it
lands on an `Examples` row. `:148` — the head of the confinement outline — is
**`:256`**; read as a current line it lands inside the `Examples` grid of a
different Rule. Both citations are correct as audited lines and both were
misleading as published. That is why the convention is now stated rather than
assumed.

**This table has moved at every round of this note** — four marker blocks
rewritten in round 2 to drop the findings the panel killed, two rewritten in
round 3 to restore two of them — and it will move again for the same reason. The
audited column never moves. That is the point of the convention: a citation is
stable, a line number is not.

**This table does not need rewriting when the markers move again.** The
audited file is immutable at `d9120d7`; only the *Current* and *Offset* columns
describe `HEAD`, and both are regenerable by diffing the two revisions.
**Any edit to `features/d-bundle.feature` invalidates the *Current* column until
the digest above is re-pinned** — that is the check, and it is cheap: if the
file's sha256 is not the one in §1, do not trust the right-hand columns.

## 6. Findings

Every block carries its **evidential state** at the top, before the statement,
so that no reader has to infer it. Round 1 of this note defined three states.
**There are now two, and the reason the third is gone is itself a result:**

- **confirmed by transcript** — a named mutant ran and the prediction landed; the
  `evidence_id` is cited. **Seventeen findings** — the fifteen of the original
  campaign plus `DBND-026` and `DBND-040`, both restored in round 3 by a mutant
  that falsified the refutation which had removed them.
- **on the record alone** — no mutant ran against it and the panel did not
  examine it. Stated in those words. Twenty findings.
- ~~**survived the adversarial panel**~~ — **this state no longer exists and no
  finding holds it.** Round 1 gave it to four findings. The warden ruled them
  under-panelled against the orchestrator's own rule — a refutation resting on
  judgement rather than on a checkable fact gets the full panel — and a second
  refuter killed all four on facts in the source. Ten findings were examined by
  the panel; **eight are refuted and two were restored by measurement**, and all
  ten are recorded in §7. The category is struck rather than left standing with
  an empty membership, because a reader scanning for it is entitled to learn that
  nothing earned it — **including the two restorations, which were not restored
  by surviving an attack but by a transcript that falsified the attack.**

`OPEN` is the state of every finding in this note: this is round 1 and no
corrector has run.

### 6.0 Renumbering — the complete map

Pass A → this note. Ordered by unit, then by severity within the unit. The
`DBND-1xx`…`DBND-7xx` identifiers are retired and will not be reused.

| Pass A id | Unit | Sev | This note | Disposition |
|---|---|---|---|---|
| `DBND-101` | RU-1 | P2 | **`DBND-001`** | carried |
| `DBND-102` | RU-1 | P2 | **`DBND-002`** | carried |
| `DBND-105` | RU-1 | P3 | **`DBND-003`** | **merged** with `DBND-301`; severity P2 |
| `DBND-301` | RU-3 | P2 | **`DBND-003`** | **merged** with `DBND-105` |
| `DBND-103` | RU-1 | P3 | **`DBND-004`** | carried |
| `DBND-104` | RU-1 | P3 | **`DBND-005`** | carried |
| `DBND-106` | RU-1 | P3 | **`DBND-006`** | carried |
| `DBND-201` | RU-2 | P2 | **`DBND-007`** | carried |
| `DBND-202` | RU-2 | P2 | **`DBND-008`** | carried |
| `DBND-203` | RU-2 | P3 | **`DBND-009`** | carried |
| `DBND-204` | RU-2 | P3 | **`DBND-010`** | carried |
| `DBND-205` | RU-2 | P3 | **`DBND-011`** | carried, protocol question settled in §8.3 |
| `DBND-302` | RU-3 | P2 | — | **removed by the panel** (§7) |
| `DBND-303` | RU-3 | P3 | **`DBND-012`** | carried, spec ground strengthened |
| `DBND-401` | RU-4 | P2 | **`DBND-013`** | carried |
| `DBND-402` | RU-4 | P2 | **`DBND-014`** | carried |
| `DBND-403` | RU-4 | P3 | **`DBND-015`** | carried |
| `DBND-404` | RU-4 | P3 | **`DBND-016`** | carried |
| `DBND-405` | RU-4 | P3 | **`DBND-017`** | carried |
| `DBND-501` | RU-5 | **P1** | **`DBND-018`** | carried |
| `DBND-502` | RU-5 | P2 | **`DBND-019`** | carried |
| `DBND-503` | RU-5 | P2 | ~~`DBND-020`~~ | merged with `DBND-714`/`DBND-715`, then **removed by the panel, round 2** (§7). Identifier retired |
| `DBND-714` | RU-7 | P3 | ~~`DBND-020`~~ | merged into `DBND-503`, removed with it (§7) |
| `DBND-715` | RU-7 | P3 | ~~`DBND-020`~~ | merged into `DBND-503`, removed with it (§7) |
| `DBND-504` | RU-5 | P2 | **`DBND-040`** | removed by the panel round 1, **restored by measurement round 3** and **narrowed** (`ev-f18d4843`, §7). It never held a reconciled identifier, so this is not un-retirement: it enters the series **appended, out of unit order** |
| `DBND-505` | RU-5 | P2 | — | **removed by the panel** (§7) |
| `DBND-506` | RU-5 | P3 | **`DBND-021`** | carried, **narrowed** (§8.3) |
| `DBND-507` | RU-5 | P3 | **`DBND-022`** | carried |
| `DBND-508` | RU-5 | P3 | **`DBND-023`** | **merged** with `DBND-605` |
| `DBND-605` | RU-6 | P3 | **`DBND-023`** | **merged** with `DBND-508` |
| `DBND-509` | RU-5 | P3 | **`DBND-024`** | carried |
| `DBND-601` | RU-6 | P2 | **`DBND-025`** | carried |
| `DBND-603` | RU-6 | P2 | **`DBND-026`** | removed by the panel round 2, **restored by measurement round 3 under its own number** (`ev-f7ee3968`, §7). **Un-retired**: the identifier was never reused, so it came back to itself. Its two round-1 refuter corrections are kept in the block |
| `DBND-602` | RU-6 | P3 | **`DBND-027`** | carried, **narrowed** (§8.3) |
| `DBND-604` | RU-6 | P3 | **`DBND-028`** | carried, **gained a transcript** (§8.5) |
| `DBND-703` | RU-7 | **P1** | **`DBND-029`** | carried |
| `DBND-701` | RU-7 | P2 | ~~`DBND-030`~~ | **removed by the panel, round 2** (§7). Identifier retired |
| `DBND-702` | RU-7 | P2 | **`DBND-031`** | carried |
| `DBND-704` | RU-7 | P2 | **`DBND-032`** | carried |
| `DBND-705` | RU-7 | P2 | — | **removed by the panel** (§7) |
| `DBND-706` | RU-7 | P2 | **`DBND-033`** | carried |
| `DBND-707` | RU-7 | P2 | **`DBND-034`** | carried |
| `DBND-708` | RU-7 | P2 | — | **removed by the panel** (§7) |
| `DBND-709` | RU-7 | P2 | ~~`DBND-035`~~ | **removed by the panel, round 2** (§7). Identifier retired |
| `DBND-710` | RU-7 | P2 | — | **removed by the panel** (§7) |
| `DBND-711` | RU-7 | P3 | **`DBND-036`** | carried, **narrowed** — a limb resting on the killed `DBND-710` is struck (§8.3) |
| `DBND-712` | RU-7 | P3 | **`DBND-037`**, **`DBND-038`** | **split** (§8.2) |
| `DBND-713` | RU-7 | P3 | **`DBND-039`** | carried |

48 in, **37 out**: 10 examined by the panel of which **8 removed and 2
restored by measurement**, 5 merged into 2, 1 split into 2.

**Ordering of the series, ruled and recorded so that nobody later "fixes" it.**
`DBND-001`…`DBND-039` are ordered by review unit and then by severity, as §6.0
says. **`DBND-040` is out of that order by restoration**: it belongs to RU-5 and
would sit beside `DBND-018`, but slotting it there would renumber every finding
after it, and those numbers are already published in round 1 and round 2, quoted
in `STATE.md`, written into the Gherkin markers and cited in `ev-1ad4fe50`.
**A restored finding is appended, never slotted.** The same rule applies to any
future restoration, and the ordering claim above should be read as *unit-ordered
through `039`*.

**Three identifiers of this note's own series are retired: `DBND-020`,
`DBND-030` and `DBND-035`.** They were published in round 1 (`b19c7fd`) and
removed in round 2, and those numbers are never reused: a reader arriving with
round 1 in hand must be able to look up a number and find out that it died,
which is the whole reason this table exists for the Pass A numbers.

**`DBND-026` was retired in round 2 and un-retired in round 3.** It came back
under its own number rather than a fresh one, because the number had not been
reused and because §6.0's row for it is the place a reader looks; giving a
restored finding a new identifier would have cost a row in the 48-row map and
told the reader nothing true. **Reader's shortcut: if a `DBND-0xx` number appears
nowhere in §6, it is in §7 — and §7 says whether it stayed there.**

*A caution about one of them, recorded because it cost real work.* The
orchestrator's remediation brief identified the four newly refuted findings by
their Pass A numbers and, for two of them, by the wrong reconciled identifier —
`DBND-701` was given as `DBND-035` and `DBND-709` as `DBND-039`. The Pass A
numbers are authoritative and this note holds the mapping, so the refutations
were applied to `DBND-030` and `DBND-035`. The arithmetic confirms it
independently: the brief's own target of 2 P1 / 13 P2 / 20 P3 is reached only by
removing four P2s, and `DBND-039` is a P3. Had the identifiers been followed
literally, a P3 nobody attacked would have been removed and a P2 the panel
killed would still be open.

### 6.1 Disclosure barrier — assessed by this pass, not inherited

`aithos-core` is public and this branch will be pushed to it. Blocking condition
9 retains, from every tracked file, the *statement* of a finding that would
describe an exploitable weakness for which no fix exists.

All six Pass A auditors assessed the condition independently, across all seven
units, and all six raised nothing, each recording why (`frozen.json`, field
`disclosure` — whose own text says "seven auditors" and is wrong on that count,
§2). **This
pass re-assessed rather than inherited it**, because a barrier that is only ever
inherited has stopped being a barrier. Three candidates were examined in full;
none is retained. The reasoning, the searches and the tables re-checked
separately from the prose are in §15.

**Nothing in this note is embargoed. Nothing has been withheld from this file.**

**Three statements previously under embargo elsewhere in this repository were
published in full on 2026-08-04 by owner ruling** — `chdr-028`, `spec-cons-12`
and the code edge of `spec-cons-05`. They are cited here where they bind
(§9) and are **not** re-embargoed. That ruling is a publication decision on
three named statements; it does not relax condition 9 and was not read as
relaxing it.

---

### `DBND-001` — `OPEN`, P2 — S4's rejection is over-determined; the chain check its name is about is not load-bearing

> **Evidential state: confirmed by transcript.** `ev-d1fc33b5` — the
> predecessor-hash comparison deleted from `Bundle::verify` (`bundle.rs:1726`)
> leaves the gate **green, 51/51**. The scenario named `A broken chain fails
> closed` does not notice the chain check disappearing. Control in the same
> family: `ev-5474b889`, `publish` stops pinning its predecessor, **red, 20/51**
> — so the property itself is defended elsewhere in the suite and there is no
> hidden P1 behind this finding.

**Scenario `:27` / RU-1.**

**Statement.** `wrong_predecessor` (`cucumber.rs:8357-8379`) publishes a forged
tip at `height + 1` whose `files` map is copied verbatim from the previous
edition. `all_pinned_files` (`bundle.rs:1616-1625`) excludes
`manifests/{exclude_latest}.json` from that edition's own pins, so advancing the
tip to height 3 makes `manifests/2.json` an unpinned stray under `verify()`'s
stray check (`bundle.rs:1760-1768`), whose only exemptions are `manifest.json`
and `manifests/{latest.edition.height}.json`. The `Then` (`edition_rejected`,
`cucumber.rs:12738-12741`) asserts only `verify().is_err()` and never inspects
the error. In the unmutated tree the first error the forgery produces is
`"broken chain at height 3"`; the assertion does not depend on it.

`fails closed` — a claim about the *failure mode*, deny-by-default rather than
warn-and-continue — is not distinguished from any other rejection by
`is_err()`. It is folded into this finding's closure criterion rather than
numbered separately, because one error-identity assertion discharges both.

**Spec reference**, `spec/02-content-tree.md` § 2.6, quoted verbatim to the end
of the sentence and including the conditional clause:

> Editions form a **linear chain**: height strictly increases, each pins its
> predecessor.
>
> Without a server, two authors could sign competing height-N editions.
> Resolution rules, enforceable by any verifier:
>
> - An edition is valid only if it extends the longest chain the verifier has
>   seen and its `prev_hash` matches.

**Closure criterion.** Both of: (1) `edition_rejected`, or a chain-specific
successor step, asserts the error *identity* — `verify().unwrap_err().to_string()`
contains `broken chain at height 3`; and (2) `wrong_predecessor` removes the
confound by inserting `manifests/2.json` with its true `sha256_hex` into
`forged.files` before signing, so the wrong `prev_hash` is the only remaining
cause of rejection. Then the `ev-d1fc33b5` mutant must turn `:27` **red**. An
implementer can check that without asking anyone: apply it, run the feature
gate, observe red.

**Pass A origin.** `DBND-101`, RU-1, mutant M1.

---

### `DBND-002` — `OPEN`, P2 — S3 never exercises what "pinned" uniquely means

> **Evidential state: confirmed by transcript.** `ev-de2706a8` — the entire
> flat-pin loop deleted from `Bundle::verify` (`bundle.rs:1749`) leaves the gate
> **green, 51/51**.

**Scenario `:22` / RU-1.**

**Statement.** `alter_pinned_file` (`cucumber.rs:8349-8355`) flips a byte of
`e/circle/index.json`, a file `verify()` re-derives twice: once through the flat
pins (`bundle.rs:1749-1755`) and again through the Merkle state-root
recomputation (`:1780-1789` → `state.rs:76`, `let index: ZoneIndex =
self.get_json("e/circle/index.json")?`). Deleting the flat-pin loop therefore
leaves the scenario green, which `ev-de2706a8` measures. Meanwhile the file
class the flat pins were deliberately retained for is never touched by any step
of this feature. `manifest.rs:33-35`, verbatim:

> `/// Flat file pins — kept BESIDE the Merkle roots (decided 2026-07-11):`
> `/// they still cover byte-rollback of sealed self blobs (§02.8).`

**Absence claim, with its search.** Layer: the state-tree builder, the only
recomputation `verify()` performs besides the gamma roots. Scope: the whole of
`rust/crates/aithos-bundle/src/state.rs`. Search: `grep -n "blob" state.rs` → three
hits, `:227` (`for row in &index.blobs`), `:318`, `:321`, all reading `SelfIndex`
**index rows**, none reading any `blobs/*.enc` byte. Repository-wide search for a
byte-flip tamper: `grep -rn "\^= 1" rust/ --include=*.rs` → three hits,
`cucumber.rs:8353` (this step), `aithos-core/tests/c1_header_seal.rs:110` and
`:112` (a header ciphertext, not a bundle blob, and not an edition check). So
byte-rollback of a sealed blob is asserted nowhere.

**Closure criterion.** `alter_pinned_file` flips a byte inside a sealed blob —
`e/circle/blobs/<sid>.enc` or `e/self/blobs/<sid>.enc`, resolved from the index
rather than hard-coded — and `edition_rejected` asserts the error contains
`pinned file altered:` followed by that path. With that change the `ev-de2706a8`
mutant must turn `:22` red while the `ev-d1fc33b5` mutant leaves it green. If the
project wants to keep the index tamper too, add it as a second scenario; the blob
tamper is the one that discharges this.

**Pass A origin.** `DBND-102`, RU-1, mutant M2.

---

### `DBND-003` — `OPEN`, P2 — one bare `verify()` discharges two different sentences in two Rules, and neither sentence's own content is asserted

> **Evidential state, stated per limb because the two limbs are not worth the
> same.**
> **Limb A (RU-3, `:51`) — confirmed by transcript.** `ev-c7f65638` — the
> `blob_sha` guard deleted from the keyless public read (`bundle.rs:1280`)
> leaves the gate **green, 51/51**: the only integrity check the keyless read
> performs can be removed and the step that promises integrity does not move.
> **Limb B (RU-1, `:13`) — on the record alone.** No mutant ran against the
> ordinal or the offline half and the panel did not examine it. That is a budget
> decision recorded in `VERDICTS.md` § D, not an oversight.

**Scenarios `:10` and `:47` / RU-1 and RU-3. This is a merged finding — see
§8.1.**

**Statement.** `edition_verifies` (`cucumber.rs:12697-12701`) carries **two**
`#[then]` attributes:

```rust
#[then("edition 1 verifies offline")]
#[then("its integrity checks against the signed edition")]
fn edition_verifies(w: &mut ProtocolWorld) {
    w.bundle.as_ref().unwrap().verify().expect("edition valid");
}
```

That is the whole body. It is the same function under two sentences in two
different `Rule` blocks, and neither sentence's specific content is reached.

*Limb A, `d-bundle.feature:51`, "**its** integrity checks against the signed
edition".* The body never touches `w.read_body`, never looks at
`e/public/index.json`, and never looks at the row the `When` resolved. The word
*its* has no referent in the executing code, and the step passes identically on a
bundle containing no public section at all — which is exactly what it does at
`:13`. The property line 51 names is real in the code (`public_read` checks
`row.blob_sha != sha256_hex(&body)` at `bundle.rs:1280-1284`, and `verify` pins
`e/public/index.json` among `latest.files`), but the scenario asserts neither
link, and `ev-c7f65638` measures that the first link can be deleted without the
gate moving.

*Limb B, `d-bundle.feature:13`, "edition **1** verifies **offline**".* No
assertion is made about `edition.height` — contrast `edition_two_verifies`
(`cucumber.rs:12724`), which does assert its ordinal — so the `1` is decoration.
And `verify()` is called on the live `Bundle` value the `When` constructed, over
the same in-process `MemStore`: nothing is exported and nothing is reopened.

**Spec reference**, `spec/02-content-tree.md` § 2.12, *Keyless façade (G-D)*,
quoted verbatim to the end of the sentence:

> **Keyless façade (G-D).** Bundle is the only public assembly boundary: it
> decodes and validates layout, version, hashes, references, reachability, and
> proof shape, then passes typed public artifacts to Core's pure semantic
> verifier. Append-time and cold-time feed the same facts to that verifier and
> obtain the same verdict. Exporting an edition into a fresh `MemStore` or
> `FsStore` and reopening it without owner or grantee private capabilities MUST
> be sufficient to verify owner and delegated history.

The scenario exports nothing and reopens nothing, so that `MUST` is untested
here. `Bundle::verify` (`bundle.rs:1691-1795`) takes no key argument and reads
only `self.store` and `self.did`, so there is no reason to believe the property
is *violated*; the finding is that the only occurrence of the word *offline* in
the feature does not demonstrate it.

**Closure criterion.** **Split the step first.** `:13` and `:51` must stop
sharing a body, because the correct assertion for one is wrong for the other.
Then: `:13`'s successor asserts `latest_manifest().edition.height == 1` and
rebuilds a fresh `MemStore` from the store's key/value pairs, wraps it in a new
`Bundle` and calls `verify()` on that; `:51`'s successor tampers — one byte of
`e/public/profil/bio.md`, or `blob_sha` in `e/public/index.json` — and asserts
the keyless read is refused, or asserts that the hash the read checked is the one
the signed manifest pins. The `ev-c7f65638` mutant must then turn `:47` red. The
repository already has the posture for the reopen half: `core_owner_reopens`
(`cucumber.rs:11546-11556`) backs `:70`, so closure costs a helper, not a design.

**Pass A origin.** `DBND-301` (RU-3, mutant M-C) merged with `DBND-105` (RU-1).
RU-2's auditor reached limb A independently from its own unit and deliberately
left it to RU-3 (`pass-a/RU-2.md` §6.1); that is the first of the three
convergences recorded in `VERDICTS.md` § E, and the shared body is the
step-collision recorded there.

---

### `DBND-004` — `OPEN`, P3 — S3 and S4 carry no positive control

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it — the panel was run only on P1/P2 findings without a
> confirmed mutant. This ruling is made on the record.

**Scenarios `:22`, `:27` / RU-1.**

**Statement.** Both negatives assert only `verify().is_err()`
(`cucumber.rs:12738-12741`), and their shared `Given` (`a_published_bundle`,
`cucumber.rs:7706-7712`) never establishes that the bundle verified before the
mutation. A regression making `Bundle::verify` reject unconditionally, for a
reason having nothing to do with chains or pins, keeps both scenarios green.

**Why P3 and not P2, kept from Pass A and re-checked here.** A control does
exist, one `Rule` block away and on a byte-identical arrangement: `:16` builds
the same `init` + `add_circle_section` + `publish` fixture and asserts `verify()`
succeeds (`cucumber.rs:12721`). The control is real, and it is *cross-scenario* —
lost by any run filtered to `:22`/`:27`, by a fixture edit touching only the
`a_published_bundle` path, and by a reader auditing either scenario alone.

**Closure criterion.** Each negative scenario establishes its own control before
mutating: a Gherkin step (`And edition verification currently succeeds`) between
the `Given` and the `When`, or an `assert!(bundle.verify().is_ok(), …)` as the
first statement of `alter_pinned_file` and `wrong_predecessor`.

**Proposed and unrun.** A mutant making `Bundle::verify` return `Err`
unconditionally is predicted to leave `:22` and `:27` green while turning `:10`
and `:16` red. **That mutant has not been run and is stated here as proposed and
unrun.**

**Pass A origin.** `DBND-103`, RU-1, mutant M3 (never run).

---

### `DBND-005` — `OPEN`, P3 — `the manifest pins the DID document` adds no detection power to the step before it

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:10` / RU-1.**

**Statement.** `manifest_pins_did` (`cucumber.rs:12703-12718`) asserts
`manifest.files["did.json"] == sha256_hex(store["did.json"])`. Every store state
falsifying that assertion is already rejected by the immediately preceding
`edition_verifies`: a wrong hash trips the flat-pin loop
(`bundle.rs:1749-1755`); an absent `did.json` key trips the unpinned-stray check
(`bundle.rs:1760-1768`, whose only exemptions are `manifest.json` and
`manifests/{height}.json`). The step is a tautology *in position*, and it never
demonstrates the property its name suggests — that an edition is **bound** to
that DID document, i.e. that substituting a differently-keyed `did.json` breaks
verification.

**Closure criterion.** Replace or supplement `:14` with a demonstration: after
`Bundle::init`, overwrite `did.json` through the `pub store` field with a second,
internally consistent, differently-rooted DID document, and assert `verify()`
errors naming `pinned file altered: did.json`.

**Proposed and unrun.** Two mutants would locate the finding precisely —
dropping `did.json` from `all_pinned_files`' output, and poisoning its pinned
hash — both predicted to turn `:10` red *at step `:13`, not at step `:14`*, which
is the finding. **Neither has been run; both are stated here as proposed and
unrun.**

**Pass A origin.** `DBND-104`, RU-1, mutants M4a/M4b (never run).

---

### `DBND-006` — `OPEN`, P3 — "Every publication" and the Rule's linearity are demonstrated on one publication and no fork

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:16` / RU-1.**

**Statement.** `:16` performs exactly one publication and asserts one link. The
Feature description (`:4-5`) says editions form "a linear, hash-pinned chain" and
the Rule title says "Editions chain"; no scenario in RU-1 constructs a second
successor to the same edition. `:27` tests a *wrong* predecessor hash, not a
*second* successor.

`Bundle::verify` enforces one half of the normative bullet — `prev_hash` matches
(`bundle.rs:1726-1730`) and heights are contiguous (`:1721-1723`) — but "extends
the longest chain the verifier has seen" is a statement about a verifier holding
more than one candidate, and nothing in RU-1 puts a verifier in that position.
This is a scope observation, not a claim that the property is unimplemented:
§ 2.6's merge and fork machinery (`verify_merge_edition`,
`verify_resolution_edition`, `merge.rs`) plainly exists and was not audited by
this unit.

**Spec reference.** `spec/02-content-tree.md` § 2.6, quoted in full under
`DBND-001`, including the resolution bullet.

**Closure criterion.** `:16` publishes a **second** edition and asserts height 3
pinning edition 2, so "every" has at least two instances and the assertion is not
a fixture constant; and either RU-1 gains a scenario constructing two competing
height-N editions and asserting the verifier refuses to treat either as
canonical, or the Rule title drops the universal reading and the feature names
the Rule that carries linearity.

**Pass A origin.** `DBND-106`, RU-1.

---

### `DBND-007` — `OPEN`, P2 — the Rule's word "sealed" is asserted by neither of its scenarios

> **Evidential state: confirmed by transcript.** `ev-23aeba39` —
> `blob_seal`/`blob_open` reduced to a length-preserving identity, i.e. a
> cleartext store: **50/51**, and the single casualty is `:55`, in a different
> `Rule`. Both scenarios of RU-2 stay green against a store that seals nothing.

**Scenarios `:34`, `:39` / RU-2.**

**Statement.** RU-2 is named *Content round-trips through the sealed store*.
Both of its assertions — `body_intact` (`cucumber.rs:12745`) and
`reads_at_new_path` (`:12756`) — compare a decrypted body to the module constant
`BODY` (`:68`). Both are satisfied by any pair of mutually inverse write/read
functions, the identity pair included. Nothing in either scenario, and nothing
anywhere in `features/d-bundle.feature`, observes the bytes actually resident in
`e/circle/`. An implementation writing circle bodies in clear satisfies this
Rule **while the Rule's own name asserts the opposite**.

**Absence claim, with its search.** Layer: the Gherkin step layer,
`rust/crates/aithos-bundle/tests/cucumber.rs` (20 040 lines, the sole registered
step file per `fn main`, `:20017-20040`).
*Search 1* — every assertion in that file inspecting raw store bytes for a
plaintext needle: `grep -n "inspected.contains\|all.contains\|raw.contains"` →
**exactly one hit**, `:12775`, inside `self_leaks_nothing`.
*Search 2* — every step enumerating a zone prefix out of the store:
`grep -n 'store.list("e/'` → **exactly one hit**, `:8418`, inside
`inspect_self_zone`, argument the literal `"e/self/"`.
The whole Gherkin layer therefore contains one opacity assertion, it belongs to
RU-4, and it never looks at the circle zone.
*Search 3* — repository-wide, `grep -rn "e/circle" rust/` → 87 hits, 39 in
`rust/crates/*/tests/*.rs`; the two touching blob bytes (`cucumber.rs:5656`, a
delete-effect check; `cb7_transaction_contracts.rs:329-333`, a symlink-escape
check) assert no opacity.

**What is *not* wrong here, stated because the opposite failure mode was looked
for and is absent.** The `Then` does not round-trip on its own `When`: it
compares against a literal fixed at `cucumber.rs:68`, not against a value
re-derived from the `When`. The write and the read are separate calls through
separate code paths (`section_add` → `put_blob_v`; `resolve_clear` →
`open_blob_v`), and `pub struct Bundle` (`bundle.rs:283-286`) holds exactly two
fields, `store` and `did` — no cache, no staged tree — so the round trip really
does cross serialized bytes. *Content is recoverable unchanged* is demonstrated.
Only *sealed* is not.

**Closure criterion.** Add to RU-2 an assertion reading the raw bytes of
`e/circle/blobs/` out of the store — as `inspect_self_zone` (`:8414-8423`)
already does for `e/self/` — asserting that neither `BODY` nor the section title
appears in them; and add the corresponding `Then` line to the Rule, so the
contract carries the obligation and not only the code. Closed when the
`ev-23aeba39` mutant turns a scenario of `:32` red.

**Pass A origin.** `DBND-201`, RU-2, mutant M1.

---

### `DBND-008` — `OPEN`, P2 — S6 cannot distinguish a rename from an alias, a re-key or a byte move

> **Evidential state: confirmed by transcript.** `ev-f7261aa9` — `rename_folder`
> made to append an alias row instead of mutating the existing one, so the old
> name survives and one sid carries two live display paths: gate **green,
> 51/51**. A rename that renames nothing passes the rename scenario.

**Scenario `:39` / RU-2.**

**Statement.** `Display paths resolve through names, keys through sids` asserts
one thing: that after `rename_folder`, a read at the new display path returns
`BODY` (`reads_at_new_path`, `cucumber.rs:12748-12758`). The scenario name makes
a four-part normative claim and three parts are unobserved:

1. no assertion captures the folder or section **sid** before the rename and
   compares it after — a rename minting a fresh sid and re-sealing under the new
   node key resolves and returns `BODY`;
2. no assertion compares the **blob bytes** or `blob_sha` before and after — a
   rename re-sealing the child section under the same key with a fresh nonce
   changes every content byte and is invisible;
3. no assertion checks that the **old display path stops resolving** — the limb
   `ev-f7261aa9` measures.

**Spec reference**, `spec/02-content-tree.md` § 2.2 and § 2.9, verbatim to the
end of each sentence:

> - **sid** — a ULID, globally unique, assigned at creation, **never changed**. The sid
>   is the derivation label (§2.5) and the blob filename. Because keys hang off sids,
>   renaming anything never re-keys anything.
> - **name** — the human segment (`enfance`, `cicatrices`, `1234`);
>   `[a-z0-9_-]{1,64}`, unique among its siblings. Pure metadata: clear in the index
>   for `public`/`circle`, sealed for `self` (§2.8).

> **Rename is free.** Names are metadata (§2.2): renaming a folder or section edits an
> index row / descriptor, re-keys nothing, moves no bytes.

*"Unique among its siblings"* is the obligation the alias mutant violates and the
scenario does not see. *"re-keys nothing, moves no bytes"* is a cost-and-cut
obligation: re-keying on rename would silently cut existing grant holders and
re-encrypt a subtree.

**What the scenario does prove, credited.** Because
`owner_current_section_key_with_kex` derives the section key from the sid chain
returned by `resolve_clear`, a defect in which key derivation consumed the *name*
instead of the sid would fail the AEAD tag in `open_blob_v` and panic the
`.expect(…)` at `:12753`. *The key does not depend on the display name* is
proved. The implementation is conformant — `bundle.rs:1571-1611`, metadata only,
sid untouched, doc comment at `:1570` says so — and this finding is about the
proof.

**Closure criterion.** RU-2 gains, in the Gherkin **and** in the step bodies,
three observations that do not exist today: the section's sid is recorded in the
`Given` and asserted unchanged after the rename; the `blob_sha` of the section
row (or the blob bytes) is recorded and asserted byte-identical; and a `Then`
asserts that a read at `projets/perso/note1` is now refused. Closed when
`ev-f7261aa9` turns `:39` red and each new obligation is quoted by the Gherkin
line that carries it — the scenario name already promises "keys through sids", so
the sid assertion belongs to this scenario and nowhere else.

**Pass A origin.** `DBND-202`, RU-2, mutants M2/M3.

---

### `DBND-009` — `OPEN`, P3 — both scenarios arrange a publication that neither assertion depends on

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenarios `:34`, `:39` / RU-2.**

**Statement.** S5's `Given` ends with `publish_bundle` (`cucumber.rs:7718`) and
S6 spends a whole `When` line on `the edition is republished` (`:42` →
`publish_edition`, `cucumber.rs:8343-8347`). Neither assertion can observe either
call: `read_section` consumes `e/circle/index.json`, the zone header and
`e/circle/blobs/{sid}.enc`, all three written by `init_bundle` and
`add_circle_section`/`rename_folder`, none by `Bundle::publish`
(`bundle.rs:1678-1681`). Edition 1 itself is written by `publish_at` from inside
`Bundle::init` (`:628`), not by `publish`. The word *published* in this Rule is
decoration, and a reader of the Gherkin is entitled to believe the round trip
runs *through a published edition*, which it does not.

**This is the step coupling the repository already owes.** `bder-006-d-bundle`
(`QUEUE.yaml`; quoted in `features/.agents/d-bundle/STATE.md` § 4) names
`rename_the_folder`, `publish_edition` and `reads_at_new_path` — `cucumber.rs:8394`,
`:8343`, `:12748` — as co-owned steps whose record `d-bundle` owes "either way".
§9 makes that record.

**Closure criterion.** Either the publication is made load-bearing — the `Then`
reopens the bundle from the published manifest, or asserts the read's blob
against the pinned `files` entry of the latest edition — or the `Given` drops to
`an initialised bundle` and S6 drops line `:42`, so the contract stops promising
a publication it does not use.

**Proposed and unrun.** A mutant short-circuiting `Bundle::publish` to `Ok(())`
is predicted to leave `:34` and `:39` green while turning `:16`, `:22` and `:27`
red. **It has not been run and is stated here as proposed and unrun.**

**Pass A origin.** `DBND-203`, RU-2, mutant M4 (never run).

---

### `DBND-010` — `OPEN`, P3 — the rename step's first parameter is decorative; the parent is hard-coded

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. The finding is a statement about what the source
> says, and the source is quoted.

**Scenario `:39` / RU-2.**

**Statement.** `rename_the_folder` (`cucumber.rs:8394-8403`) builds its target as
`format!("projets/{name}")`. The phrase `the folder {string} is renamed to
{string}` advertises a general step over any folder; the body can only ever
address a depth-2 folder whose parent is literally `projets`. Written as `When
the folder "projets" is renamed to "travaux"` the step would construct
`projets/projets` and the `.unwrap()` at `:8402` would panic on an
`InvalidPath`. Two consequences: the rename exercised is always of the section's
**direct parent**, so renaming a non-leaf ancestor — the case where the sid chain
has an untouched element above the renamed one, and where § 2.5's per-segment
derivation is actually interesting — is never exercised; and the step is unusable
by any future scenario without editing its body.

**Closure criterion.** The step takes the full display path as its parameter
(`When the folder "projets/perso" is renamed to "intime"`) and the body passes it
through unchanged; and RU-2 gains one scenario renaming a non-leaf ancestor,
asserting the section still reads at the rewritten path.

**Pass A origin.** `DBND-204`, RU-2.

---

### `DBND-011` — `OPEN`, P3 — the Rule has no negative control of any kind

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. Pass A routed a protocol question with it rather than
> deciding it from code; **this pass answers that question from the
> specification** — see below and §8.3.

**Scenarios `:34`, `:39` / RU-2.**

**Statement.** RU-2's seven steps contain zero refusals. The Rule asserts a
sealed store round-trips and never once asserts that a store which should not
round-trip does not. Concretely: the circle read path never compares the blob it
opened against the `blob_sha` pinned in the index row —
`read_section_with_owner_kex` (`bundle.rs:1237-1250`) opens the AEAD and parses,
and that is all — whereas `public_read` (`:1264-1290`) does compare
`row.blob_sha` against `sha256_hex(&body)` at `:1284-1288` and refuses. The
feature's only tamper scenario (`:22` → `alter_pinned_file`) flips a byte of
`e/circle/index.json` and never of a blob.

**The protocol question Pass A routed, answered here.** RU-2 asked whether a
circle read is *obliged* to re-check the pin, and declined to settle it from code.
`spec/02-content-tree.md` § 2.11, verbatim to the end of the sentence:

> The owner signs with a single pen, `content_sign` (§01.1); the **audience lives in
> the signed payload, never in the key**. Owner content signatures always cover JCS of
> `{zone, path, sid, body_hash}` — stripping or altering the placement breaks the
> signature, so a detached artifact carries its own truth. A verifier rejects any
> owner signature whose embedded placement does not match where the object actually
> sits (fail-closed).

and, for the circle zone specifically:

> - **`circle` — signed, inside the seal.** The signature is part of the sealed blob
>   plaintext: only readers of the section can verify it. Authenticity for the
>   audience — and a member who leaks the plaintext leaks the proof with it, a stated
>   limit (§10.7): audience authenticity and leak-deniability are mutually exclusive.

**The specification places the circle signature *inside* the seal and makes no
read-side pin obligation for the circle zone.** The asymmetry between
`public_read`'s `blob_sha` check and the circle read's absence of one is
therefore *conformant*, not a defect: for circle, the AEAD already binds `did ‖
canonical sid-path ‖ key_version`, so a substituted blob must be a genuine
ciphertext of that node at that version — a replay of an earlier body, not
arbitrary content. **This finding is consequently narrowed to what it always
was: the Rule cannot tell either way, and the replay case is asserted nowhere.**
It does not allege a production defect and no longer routes a protocol question.

**Closure criterion.** RU-2 gains one scenario in which a circle blob is replaced
by a byte-valid **earlier ciphertext of the same node at the same key version**,
and the owner read — not `verify()` — is asserted to refuse or to be detectably
stale. If the project's answer is that only `verify()` gates replay, the scenario
belongs under RU-1 instead and this Rule's title states that it covers
round-trip and not freshness.

**Pass A origin.** `DBND-205`, RU-2.

---

### `DBND-012` — `OPEN`, P3 — the public row signature § 2.11 promises is written and verified by nothing

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. Pass A rated it P3 because RU-3's Rule is about
> keylessness, not authorship; **this pass keeps P3 for the same scope reason and
> records that the severity is a scope judgement, not a strength judgement.**
> The finding was re-checked against the specification, which is stronger than
> Pass A had it.

**Scenario `:47` / RU-3 — routed, see the closure criterion.**

**Statement.** `owner_content_sig` (`bundle.rs:346-366`) produces a signature over
JCS of `{zone, path, sid, body_hash}` and stores it in `SectionRow.sig`
(`bundle.rs:806` for public creation; also `:835`, `:921`, `:939`). **No code in
`rust/` verifies `SectionRow.sig`.**

**Absence claim, with its searches.** Layer: production sources and vectors.
Scope: `rust/` and `vectors/`.
`grep -rn "owner_content_sig\|body_hash" --include=*.rs rust/` → five production
sites for the former (`bundle.rs:346` definition, `:353`, `:355` its own body,
plus the producers at `:806`/`:835`/`:921`/`:939`); the `body_hash` hits at
`bundle.rs:109`, `:125` and `:1322` all belong to `K1cPublicSectionRow`, a
different field of the draft.2 carrier.
`grep -rn "row.sig" --include=*.rs rust/crates/` → `grants.rs:1843`
(`verify_public_authorship`, which handles the *delegated* `authorship` record and
requires `row.sig.is_none()`) and three assignment sites, `grants.rs:2398`,
`:2440`, `structure.rs:1066`.
`grep -rln "body_hash" vectors/` → `cb2-draft2-carriers.json`,
`cb2-gamma-v2-replay.json` and their two generators, all using the draft.2
`indices/public.json` row shape (`vectors/cb2-draft2-carriers.json:105`), not the
`{zone, path, sid, body_hash}` payload.

**Spec reference**, `spec/02-content-tree.md` § 2.11, quoted verbatim to the end
of the sentence, including the parenthetical:

> The owner signs with a single pen, `content_sign` (§01.1); the **audience lives in
> the signed payload, never in the key**. Owner content signatures always cover JCS of
> `{zone, path, sid, body_hash}` — stripping or altering the placement breaks the
> signature, so a detached artifact carries its own truth. A verifier rejects any
> owner signature whose embedded placement does not match where the object actually
> sits (fail-closed).

> - **`public` — signed, in the open.** The signature ships in the index row and MAY
>   travel as a sidecar with the raw markdown: public content is made to circulate
>   detached, carrying proof of authorship *and of publication intent*.

*"A verifier rejects…"* names a verifier. There is none. That sentence is
stronger than the ground Pass A cited, and it is why this pass records the
severity as a scope judgement: within RU-3's Rule the finding is P3, but the
sentence it now rests on is not about keylessness at all.

**Why this does not engage the disclosure gate**, assessed and not inherited: the
placement binding the missing verifier would provide is independently held by two
mechanisms present at `d9120d7` — the manifest's flat pins bind path → sha256
inside a signed edition (`bundle.rs:1749-1755`), and `public_read` refuses a body
whose hash does not match the row (`:1280-1284`). A fix exists and is named. See
§15.

**Closure criterion.** Either a verifier for the owner content signature exists
and a scenario exercises it, or `owner_content_sig`'s output is removed from the
public index row and § 2.11 is amended. **Routing:** the obligation is § 2.11's,
which no `Rule` of `d-bundle` names. This note opens the finding and does **not**
assign it to this feature's corrector; the orchestrator routes it to whichever
cycle owns § 2.11.

**Pass A origin.** `DBND-303`, RU-3.

---

### `DBND-013` — `OPEN`, P2 — RU-4's absence assertion searches one of four normative layers

> **Evidential state: confirmed by transcript.** `ev-f1718be8` — `log.rs:201`
> made to log the self zone like the public zone, so the section name travels in
> clear inside the signed Gamma log: gate **green, 51/51**. The scenario that
> asserts no section name "appears anywhere" does not see a section name in the
> open.

**Scenario `:55` / RU-4.**

**Statement.** `:58` says "appears **anywhere**". What executes searches the
*contents* of store objects whose key begins `e/self/`, and nothing else
(`inspect_self_zone`, `cucumber.rs:8414-8424`; `self_leaks_nothing`,
`:12765-12779`). Store keys are never examined — `all.push_str(…store.get(&path)…)`
pushes the value and drops the key. `gamma/gamma.jsonl`,
`manifests/index-self-<h>.json` and every object outside the prefix are never
opened.

**Spec reference**, `spec/02-content-tree.md` § 2.8, verbatim to the end of the
sentence:

> In `self`, the tree itself is confidential. On disk and in the index, `self` is a
> flat sea of opaque sids — sections and folder descriptors indistinguishable; names,
> titles, tags, parent/child links all live **inside** ciphertext. Each `self` folder
> has a small sealed **descriptor** blob under its own key listing `{name,
> children:[sids]}`; an authorized reader reconstructs exactly the sub-tree it can
> open, top-down from the deepest node it holds, and nothing else. Headers and gamma
> targets use sid-paths, so granting or editing a `self` node leaks no structure
> either.

The claim is four-sited — on disk, in the index, in headers, in gamma targets —
and the scenario reaches one and a half: the index (`e/self/index.json` matches
the prefix, so a name added to `SelfRow` would be caught) and, nominally, headers
(`e/self/hdr/<sid>.json` matches the prefix but the fixture grants nothing, so
no header exists to inspect). Store keys and gamma are outside the search.

**Why P2 and not P1.** The property is *true* at this revision, and true because
of two mechanisms the scenario never touches. `validate_store_key`
(`rust/crates/aithos-bundle/src/lib.rs:142-165`) is a closed allow-list under
which the only `e/self/` keys accepted are `e/self/index.json`,
`e/self/root.enc`, `e/self/blobs/<26-char-Crockford-sid>.enc` and
`e/self/hdr/<sid|root|short-hash>.json` — a display name cannot become a self
store key at all, and `MemStore::put` calls it (`lib.rs:341`), so the rejection
is at the store boundary and not at a caller. And `log_owner_mutation`
(`log.rs:190-228`) seals the payload for every non-public zone (`:211-224`,
`body_enc: Some(body)`, `target: None`). The defect is in the proof, not the
code — but the proof is where a regression in either mechanism would have to be
caught, and `ev-f1718be8` measures that it would not be.

**Closure criterion.** `inspect_self_zone` accumulates the store **key** alongside
the value, and its scope becomes `store.list("")` minus a named, justified
allow-list of objects permitted to be clear (`manifest.json`, `did.json`,
`e/public/**`, `certs/**`) rather than an `e/self/` prefix; the `Then` then
genuinely reads "anywhere". Alternatively a second scenario under this Rule
asserts the gamma and manifest layers explicitly. Closed when `ev-f1718be8` turns
`:55` red.

**Pass A origin.** `DBND-401`, RU-3/RU-4 report, mutant M-A.

---

### `DBND-014` — `OPEN`, P2 — the negative has no positive control, and the second `Then` is not one

> **Evidential state: confirmed by transcript.** `ev-0b4e1076` — `lib.rs:348`
> made to hide the `e/self` prefix from `MemStore::list` while leaving every byte
> in place: gate **green, 51/51**, and, in the orchestrator's words, *the
> scenario passes having inspected nothing*.

**Scenario `:55` / RU-4.**

**Statement.** Nothing asserts that `w.inspected` is non-empty, or that it
contains the objects holding the self state, or how many there are.
`self_leaks_nothing` (`:12765-12779`) is five `!contains` over a `String` the
scenario never constrains: `store.list("e/self/")` returning `[]` yields
`w.inspected == ""`, and `""` contains none of the five needles, so all five
assertions pass. The natural candidate for a control — `:59`, `And the owner
still reconstructs the full tree` — is **not** one: `zone_tree` reaches the
descriptors through `store.get` at explicitly computed paths (`bundle.rs:1092`,
`:1100-1104`, `:1119`), never through `store.list`, so a regression confined to
listing leaves both `Then`s green.

**The sharp form of the failure, and the reason this is the feature's clearest
`SEMANTIC_FALSE_POSITIVE`.** No step body in RU-3 or RU-4 can distinguish *the
self zone protected its structure* from *the self zone was not inspected*. The
assertion discriminates by **fixture vocabulary** — the five needles are the self
fixture's own strings, and the public fixture uses different ones — never by
**zone mechanism**.

**Closure criterion.** `inspect_self_zone` asserts a lower bound tied to the
fixture: at minimum `assert!(!all.is_empty())`, better `assert!(paths.len() >= 4)`
naming `index.json`, `root.enc` and the two blobs — folder descriptor and section
— the `Given` creates. Closed when `ev-0b4e1076` turns `:55` red.

**Pass A origin.** `DBND-402`, RU-3/RU-4 report, mutant M-B.

---

### `DBND-015` — `OPEN`, P3 — the `Given` announces one state and constructs a larger one; the needles are hard-coded

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:55` / RU-4.**

**Statement.** `:56` names a folder and a section. `bundle_with_self`
(`cucumber.rs:7745-7767`) additionally supplies `title: "cicatrice au genou"` and
`tags: &["sante"]`, which are the antecedents of two of `:58`'s four conjuncts;
and `self_leaks_nothing` (`:12765-12779`) hard-codes all five needles instead of
deriving them from the `Given`'s arguments. Changing the Gherkin strings at `:56`
silently decouples the assertion from the fixture; dropping the title or tags
from `:7745` silently makes two conjuncts vacuous. Neither edit produces a
Gherkin-level signal.

**This settles an `INVENTORY.md` question in the direction it did not consider.**
`INVENTORY.md` § 4.1 asked whether `:58`'s "title or tag" conjuncts are vacuous,
their `Given` never creating either. **They are not vacuous** — the antecedents
exist. But they exist **only in Rust**, and the coupling is invisible from the
feature file and unenforced from either side. Not vacuous, unanchored.

**Closure criterion.** The `Given` sentence names the title and the tag (`…
containing section "blessure" titled "cicatrice au genou" tagged "sante"`), the
step takes them as arguments, and the `Then` derives its needles from the world
state the `Given` recorded rather than from literals.

**Pass A origin.** `DBND-403`, RU-3/RU-4 report.

---

### `DBND-016` — `OPEN`, P3 — "flat sea of opaque blobs" reaches no assertion at all

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:55` / RU-4.**

**Statement.** The scenario name quotes the specification's own phrase. Neither
`Then` asserts indistinguishability, uniform key shape, uniform count or uniform
size. The property holds — a self section goes to `e/self/blobs/<sid>.enc` via
`put_blob` (`bundle.rs:866-872`) and a self folder descriptor goes to
`e/self/blobs/<sid>.enc` via `write_desc` (`:1118-1126`), the same shape through
the same allow-list branch (`lib.rs:161-165`) — but no step observes it.
`INVENTORY.md` § 4.1 asked which properties of "flat sea" the assertion set
reaches; the answer is none of them.

**Spec reference.** `spec/02-content-tree.md` § 2.8, quoted in full under
`DBND-013`; the normative clause is *"sections and folder descriptors
indistinguishable"*.

**Why this is not folded into `DBND-013`.** `DBND-013` is about **where the
search looks**; this is about a property that **no search anywhere would
establish**, because it is a statement about the shape of the set of objects and
not about their contents. The two closure criteria are different edits.

**Closure criterion.** A `Then` under this Rule asserts that every key returned
by `store.list("e/self/blobs/")` matches `<sid>.enc`, and that a stranger cannot
partition that set into sections and folders — that the fixture's two blobs are
indistinguishable by key shape and by any clear field.

**Pass A origin.** `DBND-404`, RU-3/RU-4 report.

---

### `DBND-017` — `OPEN`, P3 — no self body is ever read in RU-2, RU-3 or RU-4

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:55` / RU-4, and the RU-2/RU-3/RU-4 group.**

**Statement.** `:59` asserts the owner reconstructs "the full tree";
`owner_reconstructs_tree` (`cucumber.rs:12781-12799`) asserts three **display
paths** returned by `zone_tree`, whose body maps entries to `entry.path`
(`bundle.rs:1422-1427`). No step of RU-2, RU-3 or RU-4 calls `read_section` on
`Zone::Self_`, so the `Zone::Self_` arm of `read_section_with_owner_kex`
(`bundle.rs:1249-1257`) — the code that opens a self blob and returns
`SelfSection::md` — is exercised by none of the three Rules that `INVENTORY.md`
§ 1.8 grouped as one subject seen three times.

**Scope of the absence claim.** The fifteen step definitions reached by the
eleven authored step lines of `features/d-bundle.feature:32-59`, read in full. No
claim is made about RU-5, whose Examples include `| self | read |`.

**The three-zone asymmetry, corrected from `INVENTORY.md`.** `INVENTORY.md`
§ 1.8 proposed that body round-trip is asserted only for circle. That is
**wrong**: `body_intact` (`:12743`) and `public_body_readable` (`:12762`) are the
same assertion against `BODY` and `PUB_BODY` respectively — the two Gherkin
sentences read differently ("comes back intact" versus "is readable in clear")
and the bodies are twins. The real asymmetry, invisible from the feature text, is
this one: circle = body, no structure claim; public = body, no structure claim,
plus a nominal integrity claim that does not execute (`DBND-003`); self =
structure, no body. A clean 2/1 split with the self zone alone lacking the
property the other two share.

**Closure criterion.** Either RU-2's Rule scope is stated as circle-only, or a
scenario reads a self section body back and asserts it equals `SELF_BODY`.

**Pass A origin.** `DBND-405`, RU-3/RU-4 report.

---

### `DBND-018` — `OPEN`, **P1** — "without consuming mandate counters" is `assert_eq!(0, 0)`, and the protocol's own observable for the claim is never read

> **Evidential state: confirmed by transcript.** `ev-19a635cf` — `gamma.rs:300`
> made to stamp `authorized_by` and `authorized_via` on **every** owner entry:
> **all fifteen RU-5 rows green**; the run is 50/51 and the one casualty is in
> RU-7, on another clause entirely. `ev-bec6b91e` shows `aithos-core`'s
> `entries_rebuild_byte_for_byte` **red** under the same mutant. That asymmetry
> is the finding: another crate's unit test sees it and this feature's fifteen
> scenarios do not.

**Scenario `:63`, all 15 rows / RU-5.**

**Statement.** `core_owner_gamma` (`cucumber.rs:11529`) carries the only
occurrence of the mandate-counter claim in the whole feature. Its unique
assertion for that clause is `assert_eq!(observation.mandate_counter_delta, 0)`.
`CoreOwnerObservation.mandate_counter_delta` is written as the literal `0` at
`cucumber.rs:3549` and is never computed from anything. It is `assert_eq!(0, 0)`.

**Evidence, with searches.**
*Search 1*, `mandate_counter_delta`, scope the whole tree, all layers: 19 hits.
Fifteen are the constant `0` inside `vectors/cb2-bundle-authority-flows.json`
and one in its generator (`gen-cb2-bundle-authority-flows.py:133`). Three are
Rust: the struct field (`cucumber.rs:308`), the literal (`:3549`), and
comparisons to the constant `0` (`:3532`, `:10680`, `:11543`;
`cb2_bundle_authority_flows.rs:175`). **No site computes it.**
*Search 2*, `delegated_count`, `max_consumptions`, `max_mutations`,
`consumption`, scope `rust/crates/aithos-bundle/src/`: **zero hits.** The crate
owning `owner_content_operation` has no counter machinery, so nothing in the
execution path could have been measured.
*Search 3*, `verify_owner_entry`, scope `rust/crates/*/src/`: two hits — its own
definition (`aithos-core/src/gamma.rs:494`) and `gamma_replay.rs:289`, reached
from `log.rs:860`, a separate replay entry point. **`Bundle::verify` does not
call it.** `bundle.rs:1691-1790`, read in full: the Gamma section calls only
`aithos_core::gamma::verify_links(&entries)` (`:1772`) and the `gamma_head` pin;
`verify_links` calls `check_form`, which inspects `v`, `id`, `at`, `kind`,
`prevs`, `payload`/`body_enc` and **never** `authorized_by` or `authorized_via`.

**The enforcement function exists and is unreached from here.**
`aithos-core/src/gamma.rs:494`, `pub fn verify_owner_entry`, doc comment
`/// Owner entry check (§07.2): no mandate fields, #content signature.`, body
`:496-498`: `if entry.authorized_by.is_some() || entry.authorized_via.is_some() {
return Err(err("owner entries carry no mandate")); }`.

**Spec reference.** `spec/04-mandates.md:1861`, verbatim to the end of the cell:

> | Owner | Local narrow capability; operation is authorized without a mandate, journalized, and consumes no mandate counter or constraint. | Verify owner signature, canonical operation, Gamma, changeset, and state transition; never synthesize a mandate consumption. |

`spec/04-mandates.md:313`, verbatim to the end of the sentence:

> are journalized where required but increment neither delegated counter.

`spec/07-gamma.md:173`, verbatim to the end of the line — this is the sentence
that makes the claim *observable*:

> - `max_actions: N` ⇒ count entries whose `authorized_via` **contains** this mandate id

and `spec/07-gamma.md:404`, verbatim:

> no leaf (owner-signed entries carry no `authorized_via` and feed no leaf).

**Failure scenario.** An owner mutation begins appending a Gamma entry carrying
`authorized_via: ["mandate_X"]`. Per `spec/07-gamma.md:173` that entry now counts
against `mandate_X`'s `max_actions`. `gamma_entries().len()` is unchanged,
`check_form` passes, `verify_links` passes, `Bundle::verify()` passes,
`mandate_counter_delta` is still the literal `0`. All fifteen rows stay green —
and `ev-19a635cf` is that scenario, run.

**Why P1.** The invariant has a dedicated enforcement function that no
verification path in this unit reaches, so a real defect ships past all fifteen
scenarios. Pass A staked the severity: *"it downgrades to P2 if the mutant comes
back RED, i.e. if some layer I did not find refuses it."* The mutant came back
with all fifteen rows **green**. The stake is settled in the finding's favour and
P1 stands.

**Closure criterion.** `core_owner_scenario` captures the Gamma entries appended
by the operation — it already holds `gamma_before`/`gamma_after` — and asserts,
for each appended entry, `entry.authorized_by.is_none() &&
entry.authorized_via.is_none() && entry.signature.key == "#content"`; or, better
and equivalently, calls `aithos_core::gamma::verify_owner_entry(entry, &did_doc)`
on it. `mandate_counter_delta` is then either derived from `authorized_via`
occurrences or deleted as a field. Closed when `ev-19a635cf` turns at least one
row of `:63` red.

**Pass A origin.** `DBND-501`, RU-5, mutant M4.

---

### `DBND-019` — `OPEN`, P2 — three of the fifteen rows satisfy "succeeds from the narrow owner capability" on paths that never receive a capability

> **Evidential state: confirmed by transcript.** `ev-b6a36f72` — the five
> `owner_content_operation` call sites (`cucumber.rs:3420` region) made to take a
> stranger `OwnerKeys`: **12 of 15 rows die, three survive**, and the three are
> `public/list`, `public/read` and `circle/list` — exactly the composition
> predicted, row for row, before any transcript existed.

**Scenario `:63`, rows `public/list`, `public/read`, `circle/list` / RU-5.**

**Statement.** For those three rows the executed production code does not take
`owner_kex`. The `Then` at `:67` — *the operation succeeds from the narrow owner
capability without a mandate* — is satisfied by a keyless path, one this same
feature file elsewhere proves a **stranger** can walk.

**Evidence.** `zone_entries_with_owner_kex` (`bundle.rs:1430-1443`):
`match zone { Zone::Self_ => self.self_walk(&[], "", owner_kex, &mut out), _ =>
self.clear_zone_entries(zone) }` — the `_` arm covers Public and Circle and
discards `owner_kex`. Doc comment on `clear_zone_entries` (`:1454`), verbatim:
*"Reconstruct typed public/circle display entries without a content key."*
`read_section_with_owner_kex` (`:1236-1237`): the `Zone::Public` arm is
`Self::public_read(&self.store, display_path)` and discards `owner_kex` — the
same function `:49` gives a keyless stranger. Two `Rule` blocks therefore assert
the same executed behaviour on `public/read` while claiming opposite things about
the capability.

**Closure criterion.** Either (a) restate the `Then` so it does not claim a
capability on rows whose zone has no content key, or (b) add the negative control
to `core_owner_scenario` — run the same operation with an unrelated `OwnerKeys`
and require refusal where refusal is correct — while recording, per row, which of
the fifteen are capability-bearing. `ev-b6a36f72` has already enumerated them:
twelve are, three are not. Closed when no row of `:63` both survives that mutant
and asserts `:67`.

**Pass A origin.** `DBND-502`, RU-5, mutant M3.

---

### `DBND-021` — `OPEN`, P3 — four of the per-row fields of `owner_cases` have no behavioural consumer

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. **Narrowed by this pass** — see §8.3.

**Scenario `:63` / RU-5, and `vectors/cb2-bundle-authority-flows.json`.**

**Statement.** `owner_cases` carries `id`, `zone`, `operation`, `expected`,
`mandate_required`, `mandate_counter_delta`, `journalized`, `fresh_store_reopen`
per row. Only `journalized` is ever compared against a **measured** value.

| Field | Consumer | Verdict |
|---|---|---|
| `journalized` | `cucumber.rs:3529` → compared to the measured `gamma_delta` at `:3538` | **real consumer** |
| `mandate_required` | `cucumber.rs:3532` and `cb2_bundle_authority_flows.rs:174` compare it to the Rust literal `false` | constant vs constant |
| `expected` | only `cb2_bundle_authority_flows.rs:172`, `assert_eq!(case["expected"], "accepted")` | constant vs constant; RU-5's own path never reads it |
| `fresh_store_reopen` | only `cb2_bundle_authority_flows.rs:176`, `assert_eq!(case["fresh_store_reopen"], true)` | never compared to `observation.reopened` |
| `zone`, `operation` | the lookup key at `cucumber.rs:3374-3380` | structural, correct |

`cb2_bundle_owner_parity_matrix_preexisting_green`
(`cb2_bundle_authority_flows.rs:163-183`) is named as a parity matrix and asserts
only that the JSON says what the JSON says, plus that the fifteen `(zone,
operation)` pairs are distinct. It is a vector-shape test.

**Narrowed.** Pass A listed five fields; `mandate_counter_delta` is one of them
and is the subject of `DBND-018`, which is P1 and confirmed by transcript.
Keeping it here would give one defect two closure criteria and two owners, so it
is **struck from this finding** and belongs to `DBND-018` alone. Four fields
remain.

**Failure scenario.** The vector is edited to declare `expected: "refused"` or
`mandate_required: true` for an owner row — a normative statement that owner
operations need a mandate. `cb2_bundle_authority_flows.rs:172-175` goes red on
the literal comparison, nothing behavioural changes, and RU-5's fifteen rows are
unaffected: the vector's normative content never reaches an execution.

**What *is* proved here, credited.** `journalized` genuinely gates. It is read at
`:3529` and compared to the *measured* Gamma delta at `:3538`, which makes the six
`list`/`read` rows carry a real negative — reads do not journalize — rather than
a vacuous one.

**Closure criterion.** Either compare each remaining field to a measured value —
`fresh_store_reopen` against `observation.reopened`, `expected` against the
accept/refuse verdict — or drop the fields and stop presenting the file as a
normative matrix.

**Pass A origin.** `DBND-506`, RU-5, mutants M5/M6 (never run).

---

### `DBND-022` — `OPEN`, P3 — "the resulting edition" has no referent on six rows, and "a fresh local store" is the same directory

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it.

**Scenario `:63`, the six `list`/`read` rows / RU-5.**

**Statement.** `list` and `read` produce no edition; `owner_content_operation`'s
`List` and `Read` arms (`bundle.rs:452-458`) run no transaction and no publish,
so on those six rows `core_owner_scenario` reopens and verifies the **fixture's**
edition, not a resulting one. And "a fresh local store" is a new
`FsStore::new(root.path())` handle over the same temporary directory
(`cucumber.rs:3505`), not an independently populated store.

**Not vacuous, and this is why the severity is P3.** The reopen genuinely round
trips through the filesystem after `drop(bundle)` (`:3503`), and on those six
rows it additionally asserts the body is still `"before"` — a real non-mutation
check. The step name over-claims on 6 of 15 rows, and "fresh" is weaker than a
store populated from published bytes alone, which is the freshness
`export_keyless`/`import_keyless` (imported at `cucumber.rs:21`) would give.

**Closure criterion.** Split the phrase: on mutating rows keep "the resulting
edition"; on non-mutating rows say what is checked ("the bundle reopens
unchanged"). If "fresh local store" is meant to exclude the writer's directory,
use the keyless export/import path.

**Pass A origin.** `DBND-507`, RU-5.

---

### `DBND-023` — `OPEN`, P3 — the `Given` steps of three Rules announce a published, snapshotted state and construct nothing

> **Evidential state, per limb.**
> **The statement is on the record alone** — no mutant ran against it and the
> panel did not examine it.
> **Its stated consequence is confirmed by transcript.** `ev-f0125e0b` shows the
> exact shape: three `MemStore` rows of `:91` report `✘ Then the mutation is
> refused before canonical effect` at `d-bundle.feature:95`, matched
> `cucumber.rs:11386`, with captured output `CORE-OWN-002 MemStore verify
> failed: seal rejected: edition: pinned file altered: e/circle/index.json` — a
> **fixture-side failure surfacing as an assertion failure inside the first
> `Then`**, attributed to a step that asserted nothing wrong.

**Scenarios `:63`, `:91`, `:116`, `:148` / RU-5, RU-6 and RU-7b. This is a merged
finding — see §8.1.**

**Statement.** Two step functions, three Rules, one defect.

*`core_atomic_fixture` (`cucumber.rs:11346-11353`)*, bound to `Given a published
"<store>" bundle snapshotted byte for byte` at `:92`, `:117` **and** `:149`, is
five assignments: `core_atomic_store = store`, `core_atomic_boundary = None`,
`core_atomic_observation = None`, `core_path_store = store`,
`core_path_observation = None`. No bundle is initialised, nothing is published,
no snapshot is taken. The whole arrangement — `Bundle::init`, the fixture
section, the publication and the `before` snapshot — lives inside
`core_atomic_bundle` (`:1699-1738`), called from the `When`'s scenario functions
at `:1761`, `:1796`, `:1864`, `:1900`, and for RU-7b from `core_path_mem_scenario`
(`:3126`) / `core_path_fs_scenario` (`:3211`). The sentence is false of the step
at the moment it runs, in all three Rules. It is a router, not an arrangement.

*`core_owner_zone` (`cucumber.rs:11484`) and `core_owner_fixture` (`:11491`)*,
bound to `:64` and `:65`, store one string and set one boolean. Neither creates a
session, a folder, a section or an edition; the publish the second announces
happens inside the `When`, at `cucumber.rs:3389-3411`.
`features/.agents/d-bundle/DOMAIN.md:373-374` already records the shape
(`"core_owner_fixture (:11491, body sets one boolean)"`), so this is a known
shape and what is new is its consequence.

**Consequence, and it is measured.** Because the `Given` verifies nothing, a
failure of the arrangement is reported from inside the `When` — `CORE-OWN-001
{zone}-{operation} fixture failed` at `:3412`, `CORE-OWN-002 …` at `:1834`/`:1938`
— and surfaces at the **first `Then`**, because `core_owner_observation` /
`core_atomic_observation` (`:11378-11384`) turn the `Err` into a `panic!` there.
A scenario whose arrangement is impossible fails on an assertion line, reporting
an assertion failure where the truth is a missing or broken fixture. `Given` and
`When` failures are indistinguishable in the report. `ev-f0125e0b` prints three
instances of exactly that.

**Closure criterion.** The `Given` builds the bundle and takes the snapshot,
storing both in the world; the `When` performs only the act. Applied to both step
functions, since the defect and its consequence are identical. This also removes
the routing hazard `DBND-036` describes. *(Round 1 also made it a precondition
for `DBND-026`'s closure; `DBND-026` was removed by the panel in round 2, §7, so
that dependency is gone and this finding now closes on its own.)*

**Recorded hazard, not claimed as a defect.** `core_atomic_boundary` (`:11355`)
is **not** unconditional: it branches on `w.core_revocation_failure_boundary ==
"__fixture__"` and, when that holds, writes the boundary into the *revocation*
field instead of `core_atomic_boundary`. The sentinel is set at `:11920`, in
`g-revocation`'s `Given`. Cucumber constructs a fresh `World` per scenario —
`ProtocolWorld` is `#[derive(Debug, Default, World)]` (`:467`) — so this is
inert. It is recorded because **a step body branching on another feature's
sentinel is a coupling no reader of the feature file can see.**

**Pass A origin.** `DBND-508` (RU-5) merged with `DBND-605` (RU-6). RU-7's
`DBND-710` made the same observation as its uncontested first limb; that finding
is removed on its second limb (§7) and nothing it established is lost, because
this finding carries it.

---

### `DBND-024` — `OPEN`, P3 — "durable parity across all three zones" is never checked comparatively, and "parity" is not a term of this protocol

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. The absence claims below are searches, and each
> carries its scope.

**Rule title `:61` / RU-5.**

**Statement.** No step and no helper compares one zone's observation to
another's. `ProtocolWorld` is `Default`-constructed per scenario (`:467`), so the
fifteen observations never coexist. Parity is asserted only extensionally:
fifteen independent rows share one predicate.

**Absence claims, with their searches.**
`parity`, scope `spec/` (all eleven files), case-insensitive: **zero hits.** The
Rule's central word does not occur in the specification.
`durable`, scope `spec/`: zero hits in the sense used here.
`durable` and `parity`, scope the 165 lines of `features/d-bundle.feature`: only
`:61`, the Rule title. Neither word appears in any *step*.
`parity`, scope `rust/`: `Cargo.toml:61` (lockfile parity, unrelated), three
comment/label uses in `cb9_delegated_content.rs`, the test name
`cb8_owner_grants.rs:106`, the file header `cb8_owner_grants.rs:1`,
`cb2_bundle_authority_flows.rs:163` and `:346`, and the section comment
`cucumber.rs:11482`. **Every one is a name or a comment; none is a definition.**

**What the phrase operationally reduces to.** *Parity* = the same three-part
predicate holds on each of fifteen `(zone, operation)` pairs, checked
independently: the typed outcome variant matches the operation and carries the
expected content (`cucumber.rs:3466-3477`); the Gamma entry *count* delta equals
the vector's `journalized` flag (`:3529-3542`); a fresh `Bundle::open` +
`verify()` + content read-back succeeds (`:3505-3531`). *Durable* = the third of
those, and only the third — the effect survives dropping the `Bundle` and
reopening the `FsStore`. **That third part is genuine and load-bearing**, and
`verify()` on the reopened store walks every manifest in the chain, re-hashes
every pinned file, checks I3 on pinned headers, refuses unpinned strays and
recomputes the Merkle and Gamma roots. The unit gets that strength for free and
this note credits it.

**Closure criterion.** Either define what parity ranges over — in `DOMAIN.md` or
in the specification — and check it with a comparative step asserting the three
zones' observations equal on a named tuple, or retitle the Rule to what the
fifteen rows prove.

**Pass A origin.** `DBND-509`, RU-5.

---

### `DBND-025` — `OPEN`, P2 — line 121 asserts crash recovery and no scenario induces a crash

> **Evidential state: confirmed by transcript.** `ev-7caa8332` — the **entire**
> `FsStore` crash-recovery path (`lib.rs:906`, `recover_transaction`) replaced by
> `self.transaction = None`: gate **green, 51/51**. The only line in the whole
> feature that mentions crash recovery is green against a store with no crash
> recovery.

**Scenario `:116`, both rows / RU-6.**

**Statement.** `:121` — *"a crash or lost acknowledgement at that point resolves
to the complete old or complete new state from the canonical manifest and Gamma
head"* — resolves to `core_atomic_recovery` (`cucumber.rs:11439-11442`), whose
entire body is `assert!(core_atomic_observation(w).reopened)`. In the only
scenario reaching it, `reopened` is `reopened_snapshot == after` (`:1894`,
`:1931`), computed after an `owner_content_operation` that returned `Ok` and was
never interrupted. The scenario's only `When` (`:118`) is *the owner commits a
valid circle edit*; its sibling outline at `:91` has an injected-failure `Given`
(`:93`), this one has none. The assertion therefore states: *after a successful
commit, reopening yields the same bytes.* That is durability across reopen. It is
not atomicity at the linearization boundary.

**Spec reference**, `spec/02-content-tree.md` § 2.12, *Local transaction (G-B)*,
quoted verbatim to the end of each sentence:

> `MemStore` commits by atomically replacing its canonical state. `FsStore` prepares in
> recoverable staging physically outside the canonical bundle directory and uses a
> Store-local recoverable linearization mechanism. Any internal generation metadata,
> commit marker, or reference is outside the canonical bundle namespace, §2.3 layout,
> manifest, pins, and signed wire; it only selects which complete staged state the
> Store exposes as the canonical view. The contract does not require a non-portable
> multi-file syscall. Readers, reopen, and recovery observe either the complete old
> state or the complete new state, never a mixture. A crash or lost acknowledgement
> at the linearization boundary may require discovering the committed outcome from
> the canonical manifest/head; scratch is cleaned or recoverably resolved.

The specification makes three claims and RU-6's two blocks are the Gherkin for
all three. Claim 1, *pre-commit failure ⇒ old state*, is proved by `:91`. Claims
2 and 3 — *never a mixture*, and *a crash at the boundary resolves from the
manifest/head* — are carried by `:121` and `:122`, and `ev-7caa8332` measures
that neither is proved.

**The four states the claim ranges over are enumerated normatively and have no
behavioural consumer.** `vectors/cb2-bundle-boundaries.json →
transaction.recovery_cases` lists `no-staging`, `prepared-not-linearized`,
`linearization-reference-durable` and `acknowledgement-lost`, the last with
`internal_state` *"new reference durable, caller did not receive success"* and
`scratch_resolution` *"discover outcome from manifest and Gamma head"* — the
sentence of feature line 121, word for word. Search: `grep -rn "recovery_cases"
--include=*.rs --include=*.py .`, scope the whole tree → `gen-cb2-bundle-boundaries.py:269`,
`:666` (producers) and `cb2_bundle_boundaries.rs:342` (a shape check asserting the
array has four entries, two `visible_snapshot: "old"` and two `"new"`). **It
counts JSON. It drives the system into none of the four states.**

**Qualification, stated because it changes the severity rather than being hidden
to protect it.** `cb7_transaction_contracts.rs:218-236` *does* exercise two of
the four states behaviourally: it stages, drops the store without committing,
reopens, recovers and asserts `old`; then stages, commits, drops, reopens,
recovers and asserts `new`. The property is not wholly untested in the
repository (`ev-dd18154c`, that binary, green). Three things keep it a finding:
(a) that binary is not run by the feature-tier gate, which runs the Gherkin
binary only; (b) it operates on raw snapshots taken from the vector via
`replace_transaction`, never on the write-set the *Bundle* produces, so it never
shows that **content and Gamma** survive together; (c) it too simulates the crash
by dropping the store *between* operations. Search for (c): `grep -rn
"commit_transaction" rust/crates/*/tests/*.rs rust/crates/*/src/*.rs` → 11 sites;
the only fault-injecting one is `cucumber.rs:1570`, which errors **before**
delegating. **No test anywhere in the tree interrupts `commit_transaction`
mid-execution.**

**Closure criterion.** Either (i) `:116`'s outline gains a `Given` that injects a
failure *inside* the store's linearization — the natural home is a new
`CoreAtomicFault` variant letting `commit_transaction` write the generation
pointer and then error — and `:121`/`:122` assert that the reopened snapshot
equals `before` **or** equals `after` and never a proper mixture, with the
manifest's `gamma_head` and the `gamma/` tree read explicitly rather than
inferred from map equality; **or** (ii) `:121` is deleted from the feature file
and the claim is carried by a scenario that induces the state. Either way,
`ev-7caa8332` must turn at least one scenario of `:89` red.

**Pass A origin.** `DBND-601`, RU-6, mutant M1.

---

### `DBND-026` — `OPEN`, P2 — line 99 claims no local-mutation orphan; the snapshot it is asserted against cannot see one, and a snapshot that can exists in the same file

> **Evidential state: confirmed by transcript. Restored in round 3 after being
> removed in round 2 — see §7.** `ev-f7ee3968` — the mutant pair the auditor
> itself named as this finding's closure test, `RU-6.md` § M1 **and** § M3
> applied together: `FsStore::rollback_transaction` reduced to
> `self.transaction = None`, so the staging generation leaks permanently, **and**
> `FsStore::recover_transaction` gutted, so nothing sweeps it on reopen. Feature
> gate **green, 51/51, 299/299**. A leaked staging generation is invisible to
> every one of the fifty-one scenarios, exactly as predicted at the freeze.

**Scenario `:91`, the six `FsStore` rows / RU-6.**

**Two corrections carried from the round-1 refuter, in the published text.**
1. Pass A wrote that the assertion compares **two maps**. It compares **three**:
   `canonical_unchanged = before == after && before == reopened_snapshot`
   (`cucumber.rs:1774`, `:1809`).
2. Pass A wrote that the sighted helper is **1857 lines away**. The arithmetic
   gives **1774**: `cb7_store_snapshot` is at `cucumber.rs:1375` and
   `core_path_raw_snapshot` at `:3149` — Pass A had cited `:3232`, which is the
   start of the fixture `match`, not the helper. Verified against the tree.

**Statement.** `:99` — *"staging remains non-canonical and is cleaned or
recoverably resolved with no local-mutation orphan"* — resolves to
`core_atomic_staging_clean` (`cucumber.rs:11423`), body
`assert!(!core_atomic_observation(w).partial_state_observed)`, i.e.
`canonical_unchanged`, i.e. the same map comparison as `:96`.

Those maps are `cb7_store_snapshot` (`:1375-1389`): `store.list("")`, then
`store.get(path)` for each key. For `FsStore` both resolve through
`canonical_base()` (`lib.rs:527-540`), which — with no transaction active, which
is the state at every one of the three snapshots — returns the **generation
directory** named by the `.aithos-current` pointer. Everything the sentence is
about is outside its range: the staging generations under `.aithos-generations/`
(`lib.rs:419-421`), the pointer `.aithos-current` (`:423`), the mirror marker
(`:427`), the transient `.tmp-*` files (`:490-524`), and the compatibility
mirror under the root (`:652-684`). **A leaked staging generation — the textbook
local-mutation orphan — changes none of the bytes this assertion compares**, and
`ev-f7ee3968` measures it.

A helper that *would* see it is 1774 lines away in the same file:
`core_path_raw_snapshot` (`cucumber.rs:3149-3193`) walks the raw tree, records
directories and symlink targets, and is used by RU-7b (`:3300`, `:3324`) — the
Rule about paths, where orphans are not the claim. The unit that claims *no
orphan* uses the blind snapshot; the unit that does not claim it uses the
sighted one.

**Two further sentences of § 2.12 land on this line and are unasserted.** Quoted
verbatim under `DBND-025`: *"`FsStore` prepares in recoverable staging physically
outside the canonical bundle directory"* and *"Any internal generation metadata,
commit marker, or reference is outside the canonical bundle namespace, §2.3
layout, manifest, pins, and signed wire"*. The vector states both as booleans —
`staging_outside_canonical_namespace: true`,
`internal_generation_metadata_is_not_wire: true` — and their **only** consumer is
`cb2_bundle_boundaries.rs:326-329`, comparing a JSON literal to itself. Search:
`grep -rn "staging_outside_canonical_namespace" --include=*.rs --include=*.py .`
→ `gen-cb2-bundle-boundaries.py:620` and `cb2_bundle_boundaries.rs:326` only.

**Why the round-2 refutation failed, in full, because it is instructive.** The
refuter offered three routes by which a leak would become visible.
*Route one* — `canonical_base()`'s first branch returns `transaction.staging`,
which is under `.aithos-generations/` — is a true statement about the function
that does not apply at snapshot time, because that branch fires only while
`self.transaction` is `Some` and all three snapshots are taken with it `None`.
Round 2 of this note said so, against its own removal.
*Route two* — `recover_transaction` (`lib.rs:906-909`) reads the pointer on the
reopen path and its `Err` propagates into a panic — is **the auditor's own
conceded caveat**, and the auditor had already named its answer: § M1, gutting
`recover_transaction`, which is half of the pair `ev-f7ee3968` runs.
*Route three* — `reconcile_compatibility_mirror` (`lib.rs:686-692`) collects
every key under the store root and reconciles it against the active generation
before `reopened_snapshot` is taken — **is structurally blind, and nobody
checked.** It collects through `collect_from` (`lib.rs:581`), whose loop at
`:602-609` is:

```rust
if relative
    .components()
    .next()
    .and_then(|component| component.as_os_str().to_str())
    .is_some_and(|name| name.starts_with(".aithos-"))
{
    continue;
}
```

**Every top-level component beginning `.aithos-` is skipped — which is exactly
where the leaked generation lives.** The refutation was verified as far as *"it
calls `collect_from`"* and stopped one function short. `ev-f7ee3968` adds a
second, blunter fact: under the M1+M3 pair the compiler reports
`reconcile_compatibility_mirror` **never used** (transcript line 365), because M1
removes its only caller. Route three is not merely blind under that mutant; it is
not reached.

**Closure criterion.** `:99` asserts against a raw-tree snapshot — reuse
`core_path_raw_snapshot` (`cucumber.rs:3149`) — taken **before** the mutation and
again after the reopen, and asserts (a) no `.aithos-generations/` entry other
than the active one survives the reopen, and (b) no key of the raw tree absent
from the canonical view carries any byte of the refused mutation. Closed when
`ev-f7ee3968` turns at least one `FsStore` row of `:91` red. Note the ordering
dependency: a pre-fixture snapshot must exist, which is `DBND-023`'s closure
criterion, so `DBND-023` is done first.

**Pass A origin.** `DBND-603`, RU-6, mutants M3 and M4 — **both now run, as the
pair `ev-f7ee3968`**. A blind auditor specified the mutant that would decide its
own finding, and specified it as a pair precisely because M3 alone leaves a
defence standing. Nothing in the panel round improved on that.

---

### `DBND-027` — `OPEN`, P3 — nine `Then` steps across the unit assert four distinct bits, two of them against hardcoded struct fields

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. **Narrowed by this pass** — see §8.3. Pass A staked
> this finding's severity on a control run; the stake and its outcome are
> published below rather than dropped.

**Scenarios `:91` and `:116` / RU-6.**

**Statement.** The unit's 108 steps reduce to a small assertion set.

In `:91`, lines `:96`, `:97`, `:98` and `:99` all reduce to
`canonical_unchanged`: `:11393` asserts it; `:11407` asserts `store == store &&
reopened && canonical_unchanged`; `:11416` asserts it; `:11422` asserts
`!partial_state_observed`, and `partial_state_observed` is *defined* as
`!canonical_unchanged` at `:1787` and `:1818`. Four `Then` lines, four different
English sentences, one boolean. Inside `:11407`, `assert!(observation.reopened)`
is a **tautology** — `reopened` is the literal `true` at `:1786` and `:1817`. And
`:97` names *"the old manifest and Gamma head"* and reads neither: nothing in the
unit opens `manifest.json`, reads the `gamma_head` field, or walks the `gamma/`
chain. Map equality is strictly stronger, so the assertion is **sound**; the
sentence simply describes an observation the code does not make, and if the
snapshot's scope is ever narrowed the sentence will silently stop being covered.

In `:116`, line `:120`'s `assert!(!observation.mutation_refused)` (`:11434`) is a
**tautology**: `mutation_refused: false` is a literal at `:1890` and `:1927`.
Its second conjunct duplicates `:119`. Lines `:121` and `:122` assert `x` and
`!x` of the same expression (`reopened_snapshot == after`) — two `Then` steps,
one bit.

**Narrowed.** Pass A counted the `:121` limb here as well. `:121` is the whole
subject of `DBND-025`, which is P2 and confirmed by transcript; keeping it in two
findings would give it two closure criteria. It is **struck from this finding**
and belongs to `DBND-025`.

**The stake, and its outcome — published because an audit that hides a
miscalibrated prediction is worth less than one that shows it.** Pass A wrote:
*"the six `MemStore` rows of `:91` go RED at feature line 96. This must hold. If
any stays green, `DBND-602` escalates from P3 to P1 and the unit proves nothing
at all."* The control ran: `ev-f0125e0b`, `MemStore::rollback_transaction` made to
**commit** the overlay. **Four of the six went red, not six.** Read strictly the
stake fires; it does not, and the reason is not that the finding was wrong but
that the prediction was miscalibrated by another of the same auditor's own
findings. The two survivors are `MemStore | cryptography` and `MemStore | index
preparation`, and both survive because their fault fires on the *same first
write*, `e/circle/index.json`, before it reaches the overlay — so a rollback made
to commit has nothing to commit. That is `DBND-028`, measured. **Four reds are
enough to license every positive statement made about RU-6's byte comparison, so
nothing escalates; this finding stays P3.**

**Closure criterion.** Either the redundant lines are removed, or each is given a
distinct observation: `:97` reads `manifest.json`'s `edition.height` and
`gamma_head` and compares them to the pre-mutation values; `:98` enumerates the
five artifact classes it names and asserts each has no new key; `:99` is
addressed by nothing in this note, `DBND-026` having been removed by the panel
(§7) — so `:99` remains a `Then` reducing to the same boolean as `:96`, and that
much of this finding still covers it; `:120` asserts a *count* — one visible
transition,
matching the vector's `linearization_count: 1`, which today has no behavioural
consumer either.

**Pass A origin.** `DBND-602`, RU-6, control mutant M2 (`ev-f0125e0b`).

---

### `DBND-028` — `OPEN`, P3 — six boundary names resolve to at most four distinct injection points, and two `Examples` pairs are indistinguishable

> **Evidential state: partly confirmed by transcript, and it was confirmed by a
> control run designed for something else — see §8.5.** `ev-f0125e0b` shows
> `MemStore | cryptography` and `MemStore | index preparation` as **the only two
> of six `MemStore` rows to survive** a mutation of `MemStore::rollback_transaction`,
> while `blob preparation`, `header or wrap` and `Gamma validation` die at `:95`
> and `before state replacement` dies at `:96`. **The two rows are measurably
> indistinguishable on that observable and the other four are not.** The stronger
> claim — that they inject at the *identical call* — remains **on the record
> alone**; the probe that would settle it is named below as proposed and unrun.

**Scenario `:91` / RU-6.**

**Statement, in three parts.**

1. **`CoreAtomicFault::parse` (`cucumber.rs:1476-1491`) maps both `"before state
   replacement"` (`:108`, MemStore) and `"before commit marker or reference"`
   (`:114`, FsStore) to the single variant `Self::StateReplacement`.**
   `INVENTORY.md` § 4.5 observed the grid is not a cross-product and inferred the
   two stores might have different commit points; the code says the opposite —
   one fault under two names, and the asymmetry of the grid encodes nothing.
   Verified against the current tree by this pass.
2. **`matches_write` (`:1493-1505`) collapses two more.** `Self::Cryptography =>
   true` matches **every** write — the comment at `:1495-1496` concedes it — so
   the row named `cryptography` injects at the first store write, not in
   cryptography. In the Circle create path that first write is `ensure_folder`'s
   `put_json("e/circle/index.json")` (`bundle.rs:770`), which is also the first
   path matching `Self::IndexPreparation` (`path.ends_with("index.json")`). Four
   rows, one execution — and `ev-f0125e0b` measures the two `MemStore` halves of
   that pair behaving identically where the other four rows do not.
3. **`Self::HeaderOrWrap`'s predicate is `path.ends_with("header.json") ||
   path.contains("/wrap")`, and the owner's Circle `section_add` writes
   neither.** Headers live at `e/<zone>/header.json` (written only by
   `Bundle::init`, `bundle.rs:598`) and at `e/<zone>/hdr/<digest>.json`; wraps at
   `e/<zone>/wraps/<id>.json` (`grants.rs:143`, `:152`) — both written by the
   *grant* path. The injection for `:106` and `:112` therefore comes from the
   `get` override at `cucumber.rs:1534-1541`, i.e. from a **read** of
   `e/circle/header.json` during key derivation. The boundary named "header or
   wrap" interrupts no header write and no wrap write. `ev-f0125e0b` is
   consistent with this: the `header or wrap` row dies with `pinned file altered:
   e/circle/index.json`, so the index write had already landed when the fault
   fired on a read.

**This is a debt the feature already owes, and this unit is where it lands.**
`bder-006-d-bundle`, quoted in `features/.agents/d-bundle/STATE.md` § 4, records
the accepted round-2 impact review's reading of exactly these lines: *"Le mot
`wrap` y apparaît quatre fois, jamais comme pontage d'ancre : `:98`, `:106`,
`:112` l'énumèrent parmi les artefacts qu'une mutation échouée ne doit pas
laisser"*, and `features/.agents/c-headers/DOMAIN.md:223-226` names the seam from
the other side as *"`d-bundle.feature` — atomicity of header and wrap writes
(`:98`, `:106`, `:112`)"*. The two rows that name header-and-wrap atomicity
interrupt a read, and the `Then` at `:98` enumerating `header, wrap` among the
forbidden leftovers asserts a map equality over a mutation that writes neither —
so those two conjuncts are vacuously satisfied. §9 records the debt.

**Closure criterion.** Either the grid loses the rows that do not name a distinct
injection point, or the fixture mutation is changed to one that writes a header
and a wrap — a grant, or a rotation — so `header or wrap` interrupts what its
name says; and `before state replacement` / `before commit marker or reference`
are given two distinct fault variants matching the two stores' genuinely distinct
mechanisms (`MemStore::commit_transaction`, `lib.rs:375-381`, replaces a map;
`FsStore::commit_transaction`, `:869-896`, flips a pointer file).

**Proposed and unrun.** An instrumentation probe — an `eprintln!` in
`CoreAtomicFaultStore::injection_error` (`cucumber.rs:1526`) and at the two
injection sites (`:1534`, `:1554`), run under `--nocapture` — would print twelve
lines, one per row of `:91`, and settle part 2's *identical call* claim and part
3's read-site claim as measurements. **It is a test-side probe, not a production
mutant, it has not been run, and it is stated here as proposed and unrun.**

**Pass A origin.** `DBND-604`, RU-6, probe C3 (never run).

---

### `DBND-029` — `OPEN`, **P1** — `:139` "no seed or private key is accepted or returned" is `assert!(!false)`

> **Evidential state: confirmed by transcript.** `ev-ed18d7ef` — a public
> `manifest_private_key()` accessor added to `LocalSession`, returning the
> signing key: gate **green, 51/51**. The Gherkin line that carries the
> specification's strongest MUST NOT in this Rule does not notice a private-key
> accessor appearing on the capability surface.

**Scenario `:131`, all four rows / RU-7a.**

**Statement.** `secret_material_exposed` is declared at `cucumber.rs:333` and
written at exactly four lines — `:2106`, `:3021`, `:3049`, `:3100` — each
`secret_material_exposed: false`. Exhaustive search of the file returns those
four, the declaration, and the assertion
`assert!(!observation.secret_material_exposed)` at `:8489`. Re-run against the
current tree by this pass: six hits total, and none of them a computation. The
assertion cannot fail for a behavioural reason, or for any reason.

**Spec reference**, `spec/01-identity-and-keys.md` § 1.6, verbatim to the end of
the sentence:

> The current local key implementation may back these capabilities directly. Stable
> APIs MUST NOT require a raw seed or private key when the narrow operation suffices,
> and MUST NOT expose private material as an output.

**Why P1 and not P2.** A real defect ships under this. If someone added a public
accessor returning `manifest_key`, `gamma_key` or `owner_kex` from
`LocalSession` or from any capability struct, no scenario of this Rule would
notice, and `:139` would still read as a green proof that no such thing exists.
`ev-ed18d7ef` is that defect, added and run: green.

**The claim happens to be true at `d9120d7`**, and this pass verified it rather
than inheriting it: `grep -n "pub fn" rust/crates/aithos-bundle/src/session.rs`
returns **18** functions (`:111`–`:375`), of which three are constructors, one
returns `&K1cActor` — public identity — five return capability handles, and none
returns key material. The capability structs (`session.rs:41-76`) have private
fields, no `Clone`, no `Serialize` and an explicit `Drop`. The proof is absent in
exactly the way that lets the defect ship; the defect is not present. *(Pass A
counted 19 `pub fn`; the current tree has 18. Corrected here rather than
repeated — see §8.4.)*

**Closure criterion.** `secret_material_exposed` is **computed**, not assigned:
either from an executed attempt — a typed call that would have to accept a seed —
or `:139` is deleted from the outline and the property is discharged by a
compile-fail test (`trybuild`) that `DOMAIN.md` names. Closed when `ev-ed18d7ef`
turns a row of `:131` red.

**Disclosure gate.** Assessed by this pass, not inherited, and not engaged: the
statement describes a proof gap, the property holds at the audited revision, the
mutant is a patch to a crate an attacker would already need write access to, and
a fix exists and is named. Full reasoning in §15.

**Pass A origin.** `DBND-703`, RU-7, mutant M3.

---

### `DBND-031` — `OPEN`, P2 — the `mismatched_object` column reaches no executing code

> **Evidential state: confirmed by transcript, and the pair is the finding.**
> `ev-3fa9d172` — the `mismatched_object` cell at `d-bundle.feature:143` replaced
> by a string that exists nowhere in the repository: gate **green, 51/51**.
> Control, same row, `observable_result` cell replaced instead: `ev-1eefbb66`,
> **red, 50/51**. One column of the same table row reaches an assertion; the
> other reaches nothing.

**Scenario `:131`, line `:135` / RU-7a.**

**Statement.** `:135` binds `<mismatched_object>` and throws it away.
`d_mismatched_capability_refused` (`cucumber.rs:8464-8475`) writes the parameter
to `w.core_capability_mismatch` (`:8466`); exhaustive search of the file for that
identifier returns three lines — the declaration (`:541`), a `.clear()`
(`:8432`) and that write. **It is never read.** The boolean the step asserts was
computed by the `When`, which never received the column:
`core_capability_scenario` takes `(capability, protocol_object)` only
(`:3104-3107`).

Worse, for rows `:143` and `:144` the field is literally assigned from the
session-mismatch boolean — `mismatched_object_refused: mismatched_session_refused`
(`:2103`, `:3018`) — so *"using that capability for a Gamma entry is refused"* is
proven by *"a second session is refused"*: two Gherkin lines, one proof counted
twice. For row `:146` it is `header.open_latest(subject, "delegate-kex",
&wrong_secret).is_err()` (`:3088-3090`), a wrong-X25519-secret decryption failure
in which the capability plays no part.

**One row is honest, and it is credited.** Row `:145`:
`read_owner_section(&capability, &bundle, Zone::Circle, "projects/sibling")`
(`:3036`) reaches the real node-path binding at `session.rs:275-282`, which fires
before any store access. It is the one row of `:131` whose mismatch dimension is
executed, and it would fail if the display-path binding were removed.

**Closure criterion.** The mismatched object is *presented to the same capability
handle*, and the refusal is distinguishable from the session-mismatch refusal — a
different `Error` variant or a different message, asserted. Closed when
`ev-3fa9d172` turns `:131` red.

**Pass A origin.** `DBND-702`, RU-7, mutant M2 and its control.

---

### `DBND-032` — `OPEN`, P2 — `:137` and `:138` are decided by a grep of one source file

> **Evidential state: confirmed by transcript.** `ev-794d59c3` — a
> `sign_any()` universal byte-signing oracle added to `LocalSession`, **named
> around the grep**: gate **green, 51/51**. The specification says in terms that
> such an oracle is not a compliant Bundle API; the assertion behind the two
> Gherkin lines that say so is a string search for `pub fn sign(`.

**Scenario `:131`, lines `:137` and `:138` / RU-7a.**

**Statement.** `cross_class_substitution_refused` — the sole evidence for two
Gherkin lines across all four rows — is the return of
`core_capability_api_is_narrow()` (`cucumber.rs:2053-2058`), which reads
`src/session.rs` **as text**:

```rust
let source = include_str!("../src/session.rs");
!source.contains("pub fn sign(")
    && !source.contains("pub fn open(")
    && !source.contains("pub fn wrap(")
```

Consumed at `:2105`, `:3020`, `:3048`, `:3099`; asserted at `:8488`.

Three independent weaknesses:

1. **It is scoped to one file.** A universal `pub fn sign(` added to `src/sdk.rs`,
   `src/bundle.rs` or `src/vault.rs` is invisible to it. Search for the shape,
   scope the Gherkin layer: `grep -rn 'include_str!("../src/'
   rust/crates/aithos-bundle/tests/cucumber.rs` → **exactly one line, `:2054`**.
   No companion grep covers the other modules.
2. **It matches a formatting, not a property.** `pub fn sign_any(`, `pub  fn
   sign(`, or a `pub fn sign(` reached through a trait `impl` all pass —
   `ev-794d59c3` is the first of those three, run.
3. **It duplicates an existing plain unit test.** `cb2_bundle_boundaries.rs:458-460`
   runs the identical three greps from a `#[test]`. The scenario adds nothing the
   repository did not already have.

**A fourth limb, stated separately because its evidential state is different.**
The runtime guard `:137` names is unreachable: `binding.class != class`
(`session.rs:234`) can never be true, because `CapabilityClass` (`:26`) and
`SessionBinding` (`:35`) are private items, each capability struct's `binding`
field is private to the module, and each `self.check(…)` call site (`:249`,
`:274`, `:303`, `:323`, `:334`, `:348`, `:362`, `:370`, `:381`) passes the class
its own parameter type already fixes. Cross-class substitution is prevented by
the type system, which is **stronger** than the runtime guard — but neither the
guard nor the type argument is what the scenario asserts. **This limb is on the
record alone**, and Pass A recorded its own uncertainty about it: the argument
rests on Rust module privacy and on the nine `self.check` sites in one file, and
no workspace-wide search for a `mem::transmute` or a `#[cfg(test)]` constructor
was made. If one exists, the dead-code limb weakens; the grep limb does not.

**Spec reference**, `spec/01-identity-and-keys.md` § 1.6, verbatim to the end of
the sentence including the trailing clause:

> Every stable capability is bound to one typed protocol purpose and context. It
> accepts a typed object or request rather than arbitrary caller-selected bytes and
> binds the expected subject, domain, Ethos, actor and, where relevant, node path, key
> version, and recipient before performing cryptography. A generic `sign(bytes)`,
> decrypt-bytes, cross-context opening, or wrap-bytes oracle is not a compliant Bundle
> API, and a capability for one artifact class cannot substitute for another;
> lower-level raw primitives may remain an implementation detail behind that
> boundary.

**Prior routing, now discharged.** This site is recorded by `QUEUE.yaml`, key
`chdr-lota-source-text-assertions`, quoted at
`features/.agents/d-bundle/STATE.md:256-277`, which calls it *"inside the Gherkin
layer and … the worst"* and notes its scope limit is *"counted, not classified"*.
**It is now classified: defective, and the classification is measured**
(`ev-794d59c3`).

**Closure criterion.** Either (a) `:137`/`:138` are discharged behaviourally,
which requires a test-only path able to construct a wrong-class binding, or (b)
they are removed from the outline, the type-level argument is written into
`DOMAIN.md`, and a `trybuild` compile-fail case is added. The grep must not
remain the deciding evidence in either case, and `ev-794d59c3` must turn `:131`
red.

**Pass A origin.** `DBND-704`, RU-7, mutants M4a/M4b.

---

### `DBND-033` — `OPEN`, P2 — the four `MemStore` rows of `:148` survive deletion of display-path validation

> **Evidential state: confirmed by transcript.** `ev-2d2ebd1b` —
> `validate_display_path` reduced to `Ok(())`: gate **green, 51/51**, all ten
> confinement rows included.

**Scenario `:148`, rows `:156`–`:159` / RU-7b.**

**Statement.** For the four `MemStore` rows, `rejected` cannot distinguish *the
confinement grammar refused this path* from *no such section exists*, and the
out-of-root detector is a hardcoded `false`.

`core_path_mem_scenario` (`cucumber.rs:3117-3147`) computes `rejected =
bundle.owner_content_operation(Zone::Circle, OwnerContentOperation::Read {
display_path: invalid_input }, …).is_err()` (`:3128-3137`), sets
`outside_access_observed: false` (`:3145`) — a literal, asserted at `:11479` —
and `canonical_unchanged: before == after` (`:3144`), trivially true because a
read does not mutate. The fixture `core_atomic_bundle` (`:1699-1733`) publishes
exactly one circle section, `projects/note`. None of `../circle/secret`,
`/absolute/section`, `folder/./section`, `folder//section` names an existing
section, so with the validator neutered `Bundle::resolve_clear`
(`bundle.rs:1193-1218`) splits on `/`, **filters empty segments** at `:1196` — so
`folder//section` is silently normalised to `folder/section` — and returns
`Err(Error::InvalidPath("no folder …"))` for all four. `.is_err()` is still true.

**Spec reference**, `spec/02-content-tree.md`, the blockquote headed *"CB1
conformance-hardening decision — validated at the human protocol gate on
2026-07-18; no new grammar."*, quoted verbatim to the end of the sentence:

> Untrusted display paths are relative to their already-selected logical zone and
> enforce the human-name grammar of §2.2. They reject a leading absolute prefix,
> empty or dot segments, traversal, nonconforming names, and any resolution that
> would escape that zone before store access.

**Closure criterion.** Assert the *kind* of refusal —
`io::ErrorKind::InvalidInput` from `invalid_path` (`lib.rs:33-35`), or
`PermissionDenied` from `confinement_error` (`:37-39`) — rather than `.is_err()`,
and add rows whose display path is valid-but-absent so the two failure modes are
separated. Closed when `ev-2d2ebd1b` turns a `MemStore` row of `:148` red.

**Pass A origin.** `DBND-706`, RU-7, mutant M1.

---

### `DBND-034` — `OPEN`, P2 — `:148` is a vacuous negative: no positive control anywhere

> **Evidential state: confirmed by transcript.** `ev-2d2ebd1b`, the same run —
> the display-path validator reduced to `Ok(())` leaves all ten rows green,
> which is what a suite with no positive control looks like from the outside.

**Scenario `:148`, all ten rows / RU-7b.**

**Statement.** All ten rows assert rejection. No row supplies a valid display
path or a valid Store key. A defect that rejected **every** input would keep the
outline green.

**Absence claim, with its search.** `grep -n 'core_path_scenario' cucumber.rs` →
the definition (`:3348`) and exactly one call site (`:11459`), so the only inputs
this code ever sees are the ten `Examples` cells at
`features/d-bundle.feature:156-165`, all designed to fail. Layer: the Gherkin
harness. The plain unit tests in `cb2_store_key_consumer_neutrality.rs` and
`cb2_bundle_boundaries.rs` are outside this Rule's proof and are reached by no
scenario of it.

**Contrast, which is why this is filed separately from `DBND-033`.** `:131`
*does* have a per-row positive control (`operation_succeeded`, `cucumber.rs:8461`),
computed four different ways. The two halves of this Rule are not built to the
same standard. And the two findings ask for different edits:
`DBND-033` asks that the *kind* of refusal be asserted; this one asks that
something be allowed to succeed. Merging them would give a corrector one finding
with two closure criteria and no way to close half of it.

**Closure criterion.** At least two rows — one per store — with a valid input and
a `Then` asserting success, sharing the same step definitions.

**Pass A origin.** `DBND-707`, RU-7, mutant M1.

---

### `DBND-036` — `OPEN`, P3 — one sentence, three comparators, selected by a routing hint

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. **Narrowed by this pass**: a limb of the Pass A
> statement rested on `DBND-710`, which the panel killed, and it is struck below
> with the killing fact printed — see §8.3 and §7.

**Scenarios `:96` and `:152` / RU-6 and RU-7b.**

**Statement.** *the canonical bundle is byte-for-byte identical to the snapshot*
(`:96` in RU-6, `:152` in RU-7b) resolves to `core_atomic_unchanged`
(`cucumber.rs:11393-11405`), a body that decides which claim to check by asking
which world field the `When` happened to fill:

```rust
if let Some(observation) = &w.core_path_observation {
    assert!(observation.…canonical_unchanged);   // RU-7b
} else {
    assert!(core_atomic_observation(w).canonical_unchanged);   // RU-6
}
```

The routing is load-bearing — `core_atomic_fixture` nulls both fields (`:11350`,
`:11352`) and without the branch RU-7b would panic on the missing
`core_atomic_observation`. The three `canonical_unchanged` values it may read are
computed by three different comparators over three different baselines:

| Reader | Comparator | What it can see |
|---|---|---|
| RU-6, both outlines | `cb7_store_snapshot` (`:1375-1388`) — `store.list("")` then `get` each key | only grammar-valid, listed objects |
| RU-7b, `MemStore` | `cb7_store_snapshot` again (`:3127`, `:3138`) | the same |
| RU-7b, `FsStore` | `core_path_raw_snapshot` (`:3149-3193`) — a raw `read_dir` walk | directories, dotfiles and symlink targets too |

A defect leaving an out-of-grammar file in the tree is invisible to the first two
and visible to the third. One English sentence, three propositions.

**And in RU-7b the assertion cannot fail.** All ten rows perform a **read**,
never a write: `core_path_mem_scenario` (`:3128-3137`) issues
`OwnerContentOperation::Read`; `core_path_fs_scenario` (`:3302-3319`) issues
either `Read` or a bare `bundle.store.get(invalid_input)`. `canonical_unchanged`
is `before == after` around an operation with no write path at all. **No defect
in path confinement — none — can make `:152` red.** It is a vacuous positive, and
it is vacuous *because* the sentence was written for a Rule about mutation and
reused by a Rule about reads. In RU-6 the same branch is the unit's principal
assertion and `ev-f0125e0b` shows it failing, correctly, on four rows.

**Narrowed.** Pass A's statement also asserted that RU-7b's baseline is taken
*after* the attack fixture, making `:152` compare a tampered tree to itself. **The
panel killed that reading** and the killing fact is printed here rather than
quietly dropped: the fixture renames real bundle files and replaces them with
symlinks **inside the snapshot's own range** (`cucumber.rs:3256`, `:3268`), so
snapshotting *before* the fixture would make `before == after` false for every
`FsStore` row **against perfectly correct code**, failing six of ten rows
unconditionally. The current ordering is the only one under which the assertion
can ever pass, and the only one that measures the operation rather than the
test's own attack input. That limb is struck; the three-comparator limb and the
vacuity limb are independent of it and stand.

**Closure criterion.** Two step texts, or one comparator. RU-6's meaning must not
be weakened in the process: it is the larger unit and the one where the assertion
carries weight. The cheapest correct form is to delete `:152` from `:148`'s
outline — RU-7b's claim is confinement and `:151` carries it — or to give RU-7b a
row whose `When` is a **write** through an untrusted key, at which point `:152`
becomes load-bearing there too.

**Pass A origin.** `DBND-711`, RU-7. RU-6 reached the same verdict from the
opposite side, blind to RU-7, and declined to number it: *"it is their finding to
number, not mine."* That is the second convergence recorded in `VERDICTS.md` § E.

---

### `DBND-037` — `OPEN`, P3 — row `:162` tests out-of-layout and the `Then` says out-of-root

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. **Split from a Pass A finding** — see §8.2.

**Scenario `:148`, row `:162` / RU-7b.**

**Statement.** `| FsStore | Store key | e/circle/unlisted-object.json | no
filesystem indirection |` is, on its face, a key *inside* the root that is merely
not in the canonical layout. The fixture writes the file at
`active.join(invalid_input)` (`cucumber.rs:3281`) — inside the store root — and
sets `expected_escape_bytes` to its contents (`:3286`); `outside_before !=
outside_after` (`:3332`) then compares an untouched sibling directory. The `Then`
at `:151` asserts rejection *"before any **out-of-root** store access"*. The
property actually proven is a different sentence of the same spec blockquote:

> A signed manifest cannot legitimize an escape or out-of-layout object.

The row is a good row testing a real property under the wrong `Then`.

**Closure criterion.** Split `:151` into an out-of-layout `Then` and an
out-of-root `Then`, and bind row `:162` to the former.

**Pass A origin.** `DBND-712`, RU-7, first limb.

---

### `DBND-038` — `OPEN`, P3 — row `:161`'s escape detector cannot fire under any implementation

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. **Split from a Pass A finding** — see §8.2.

**Scenario `:148`, row `:161` / RU-7b.**

**Statement.** For `| FsStore | Store key | ../../outside | no filesystem
indirection |`, `expected_escape_bytes` is written to
`outside.path().join("outside")` (`cucumber.rs:3289`). `Cb7TempRoot::new`
(`:1425-1442`) names its directories
`aithos-cb7-cucumber-<pid>-core-path-store-<n>` and
`aithos-cb7-cucumber-<pid>-core-path-outside-<n>` under a common base. **No
resolution of the literal key `../../outside` from the store base can name the
second directory.** So `outside_access_observed` is `false` for that row
regardless of the implementation, and `rejected` carries the whole assertion
alone.

**Why this is a separate finding from `DBND-037`.** That one is a defect of the
Gherkin sentence and closes by editing `:151`. This one is a defect of the
fixture's geometry and closes by moving a file. Different owners, different
edits, and a corrector closing one must not be able to believe it closed both.

**Closure criterion.** Place the escape target where `../../outside` would
actually resolve from the store base, so that the detector at `:3332` can fire;
or drop `expected_escape_bytes` for this row and state that `rejected` is the
whole of its claim.

**Pass A origin.** `DBND-712`, RU-7, second limb.

---

### `DBND-039` — `OPEN`, P3 — `#[then(expr = "{string}")]` is an unbounded wildcard over the whole suite

> **Evidential state: on the record alone.** No mutant ran against it and the
> panel did not examine it. It is a latent hazard, not a live one, and is rated
> P3 for that reason.

**Scenario `:131`, line `:134` / RU-7a.**

**Statement.** `d_capability_result` (`cucumber.rs:8450`) matches *any* `Then`
whose entire text is one quoted string, in any of the 18 feature files the runner
loads (`cucumber.rs:20017-20040`, the whole `features/` directory, with
`fail_on_skipped()`).

**Absence claim, with its search.** `grep -rn 'Then "' features/*.feature` →
**exactly one line**, `features/d-bundle.feature:134`. There is no ambiguity
today. Any future `Then "…"` written anywhere in the suite binds silently to this
body and fails with `CORE-OWN-003 observation`, pointing an author at a Rule they
have never read.

**Closure criterion.** Anchor the step — `#[then(expr = "the capability result is
{string}")]` — and give `:134` the corresponding prefix. Note that this finding
touches the same line as the removed `DBND-030` (§7). With that finding gone,
**nothing else in this note asks for `:134` to be rewritten**, so this closure
criterion is now the only one standing over that step and must be executed on
its own rather than expected as a by-product.

**Pass A origin.** `DBND-713`, RU-7.

### `DBND-040` — `OPEN`, P2 — "journalized" is proved by cardinality alone, and a Gamma entry carrying the wrong operation's `kind` is invisible to all fifty-one scenarios

> **Evidential state: confirmed by transcript. Restored in round 3, narrowed,
> after being removed in round 1 — see §7.** `ev-f18d4843` — an owner `edit`
> made to journalize under a `create`'s kind: `aithos_core::gamma::Kind::SectionModify`
> → `Kind::SectionAdd` at `bundle.rs:975`. Feature gate **green, 51/51,
> 299/299**. That is the finding's own first worked example, applied unaltered,
> and no scenario of this feature notices an operation misdescribing itself in
> the signed log.

**Scenario `:63`, the nine mutating rows / RU-5. Out of series by restoration —
see §6.0.**

**Narrowed on restoration, and the narrowing is the refuter's, kept.** The frozen
statement said of `Entry`'s seven fields *"none is read"*. **That is false and
stays refuted:** `Bundle::verify` calls `gamma::verify_links` (`bundle.rs:1772`),
which calls `Entry::check_form` on every entry (`gamma.rs:428`), and `check_form`
reads `kind`, `target`, `payload` and `body_enc` — four of the seven. The
finding's *second* worked example is refuted with it: a circle entry whose
`target` names a different node is rejected at write time in `gamma_append`,
before any count is taken. Neither claim is made here any more.

**What survives, and it is the centre.** The only journal evidence in this unit
is `bundle.gamma_entries()?.len()` before and after (`cucumber.rs:3414-3419`,
`:3477-3480`) and the delta comparison at `:3537-3542`. **Cardinality is all this
feature checks.** `check_form` reading `kind` does not help, because it validates
the entry's *shape* and never ties it to the operation that produced it: an
`edit` that appends a well-formed `SectionAdd` entry is well-formed, the count
delta is still 1, and all nine mutating rows stay green. `ev-f18d4843` is that
defect, applied and run.

**Spec reference**, `spec/07-gamma.md:111`, verbatim to the end of the sentence
— the entry kind is what distinguishes one owner operation from another in the
log, and the log is what a cold verifier replays:

> - **Owner** entries: signed by `content_sign` (§01.1); no `authorized_by`. The

**Relation to `DBND-018`, stated so the two are not confused.** `DBND-018` is
about the mandate fields `authorized_by`/`authorized_via`, which `check_form`
does **not** read; this is about `kind`, which it does read but does not bind to
the operation. Different fields, different mechanisms, different closure
criteria. They are both P-class findings on the same step line (`:68`) and
neither subsumes the other.

**Closure criterion.** `core_owner_scenario` asserts on the appended entry that
`kind` matches the operation under test — `SectionAdd` for `create`,
`SectionModify` for `edit`, `SectionDelete` for `delete` — as a per-row expected
value driven from the `Examples` grid or the vector, not from a constant. Closed
when `ev-f18d4843` turns the nine mutating rows of `:63` red.

**Pass A origin.** `DBND-504`, RU-5. Removed by the panel in round 1 on a
refutation that answered the finding's second worked example and left its first
untested; restored in round 3 when the first was tested.

---

## 7. The adversarial panel — ten examined, eight removed, two restored

**Ten examined. Eight removed. Two restored by measurement.** The panel took the
ten P1/P2 findings carrying no confirmed mutant. A finding whose statement the
panel falsified is not downgraded and not quietly rewritten: it is removed, and
the fact that killed it is printed beside it. **A finding whose refutation is
itself falsified comes back**, and this section keeps its removal on the record
alongside the transcript that undid it.

The history, because the count moved three times and a reader is entitled to all
three positions:

| Round | What happened | Result |
|---|---|---|
| 1 | Panel run on ten findings, one refuter each | 6 removed, 4 published as survivors |
| 2 | Warden rules the four **under-panelled** — the orchestrator's own rule is that a refutation resting on judgement rather than on a checkable fact gets the full panel, and it had not been applied. Second refuter on all four | all 4 fall; 10 removed, 0 survivors |
| 3 | **Three of the ten refutations are put to the gate instead of read.** Two do not survive their own transcript | **8 removed, 2 restored** |

**The two restorations are the most important measurement in this note**, and not
because of the two findings. Every one of the ten removals was decided by reading
code and citing it. Eight of those readings hold. Two did not, and neither would
have been caught by more reading — one died on twenty unread lines of a helper,
the other on a worked example nobody applied. §8.5.2 turns that into a rule.

Each entry below states its round, its killing fact, and — where one exists — the
transcript that overturned it.

| Pass A id | Unit | Sev | Round | Killed on |
|---|---|---|---|---|
| `DBND-302` | RU-3 | P2 | 1 | the input the uncalled function needs is written by no code path in this repository |
| `DBND-504` | RU-5 | P2 | 1 | `check_form` reads the fields the finding said nothing reads, and the worked example is rejected at write time — **overturned in round 3, `ev-f18d4843`; restored narrowed as `DBND-040`** |
| `DBND-505` | RU-5 | P2 | 1 | the two `Then`s compare against **two different sources**, and nothing cross-checks them |
| `DBND-705` | RU-7 | P2 | 1 | the typed method does take caller-supplied bytes, and the fixture submits them |
| `DBND-708` | RU-7 | P2 | 1 | the sibling row tests the case, and does so for a correct reason |
| `DBND-710` | RU-7 | P2 | 1 | the ordering is stated correctly and the conclusion is inverted |
| `DBND-503` + `-714` + `-715` (was `DBND-020`) | RU-5, RU-7 | P2 | **2** | an executing test asserts the absence the finding reports, and the finding's own remedy would break it |
| `DBND-603` (was `DBND-026`) | RU-6 | P2 | **2** | the leaked staging generation is reachable by the comparison and by two further routes on the reopen path — **overturned in round 3, `ev-f7ee3968`; restored whole as `DBND-026`** |
| `DBND-701` (was `DBND-030`) | RU-7 | P2 | **2** | one row's English sentence *is* evaluated, by the very step the finding attacks |
| `DBND-709` (was `DBND-035`) | RU-7 | P2 | **2** | the repository's own confinement matrix declares the six as application points of **one** check |

### `DBND-302` — removed — *"the second keyless public read surface has no consumer"*

*Frozen statement.* `Bundle::public_read_k1c` (`bundle.rs:1296`) is a `pub`
associated function implementing the frozen K1-C draft.2 keyless read, with its
own `row.body_hash` check (`:1322`), and `grep -rn "public_read_k1c" --include=*.rs
rust/ vectors/` returns exactly one line — its own definition. Zero call sites
anywhere. The Rule *the public zone reads without any key* is therefore proved
for one of two keyless read surfaces, and the unproved one is the draft.2
carrier.

*The killing fact.* The function has zero call sites, but **it requires
`indices/public.json`, and no code path in this repository ever writes that
object.** Verified independently by this pass: `grep -rn "indices/public.json"
rust/ --include=*.rs` returns `grants.rs:627`, `:632`, `bundle.rs:1300` (reads),
`remote.rs:333` (a path allow-list) and `lib.rs:222` (the store-key grammar) —
**no write site of any kind**. Every draft.2 edition this repository can produce
carries `public/sections/<sid>.md` and no public index. An uncalled function
whose input does not exist proves nothing about the Rule.

*What survives.* Nothing that this note carries. The observation that
`public_read_k1c` is dead code is true and is not a finding about `d-bundle`'s
scenarios; if the draft.2 carrier is ever wired, the question returns with it.

### `DBND-504` — removed — *"journalized is proved by cardinality alone"*

*Frozen statement.* The only journal evidence in RU-5 is
`bundle.gamma_entries()?.len()` before and after (`cucumber.rs:3414-3419`,
`:3477-3480`) and the delta comparison at `:3537-3542`. `Entry`
(`aithos-core/src/gamma.rs:115-139`) carries `kind`, `target`, `authorized_by`,
`authorized_via`, `payload`, `body_enc`, `signature` — none is read. An owner
`edit` appending an entry with a `create`'s `kind`, or a `target` naming a
different node, keeps the count delta at 1 and all nine mutating rows green.

*The killing fact.* **False.** `Bundle::verify` calls `gamma::verify_links`
(`bundle.rs:1772`), which calls `Entry::check_form` on **every** entry
(`gamma.rs:428`), and `check_form` reads `kind`, `target`, `payload` and
`body_enc`. The finding's own worked example — a circle entry with a `target`
naming a different node — is rejected at write time in `gamma_append`, before the
count is ever taken.

**OVERTURNED IN ROUND 3. Restored, narrowed, as `DBND-040` (§6).**
`ev-f18d4843`: an owner `edit` made to journalize under a `create`'s kind —
`Kind::SectionModify` → `Kind::SectionAdd` at `bundle.rs:975`, **the finding's
own first worked example, applied unaltered** — leaves the feature gate
**green, 51/51**.

*Why the refutation was incomplete.* It answered the finding's **second** worked
example — a `target` naming a different node, rejected at write time in
`gamma_append` — and generalised from it. The first worked example was never
tested. `check_form` does read `kind`, which is what killed the finding's
over-stated *"none of them is read"*; but reading `kind` validates the entry's
shape and never binds it to the operation that produced it, so a well-formed
entry of the wrong kind passes. **The refutation is right about the sub-claim and
wrong about the finding.**

*What is kept refuted.* *"None of them is read"* stays false — `check_form`
reads four of the seven fields — and the `target` worked example stays refuted.
`DBND-040` states neither. The boundary matters and is worth keeping in view:
`check_form` reads `kind`, `target`, `payload`, `body_enc` and **not**
`authorized_by` or `authorized_via`, which is why `DBND-018` was never touched by
this refutation and remains P1 and confirmed.

### `DBND-505` — removed — *"the three `Then`s are redundant; deleting two changes nothing"*

*Frozen statement.* Every substantive check in RU-5 lives inside
`core_owner_scenario`, unwrapped-with-panic in the *first* `Then`
(`cucumber.rs:11511-11512`); the three `Then` bodies add, between them, one
assertion not already made by the helper, and that one is `assert_eq!(0, 0)`.
Deleting feature lines `:68` and `:69` would leave 15/15 green.

*The killing fact.* **The arithmetic is wrong.** The helper compares
`gamma_delta` against the **vector field** `case["journalized"]`
(`cucumber.rs:3529`, `:3538`); the `Then` compares it against a **hardcoded
predicate on the operation name** (`:11536`,
`usize::from(matches!(observation.operation.as_str(), "create" | "edit" |
"delete"))`). Two different sources, and nothing cross-checks them — so the step
the finding called redundant is the only thing in the suite tying *"every
mutation is journalized"* to anything real. Verified by this pass against the
current tree at `cucumber.rs:11533-11541`.

**STAYS REMOVED — and after round 3 it is the best-evidenced removal in this
note.** `ev-ef839413` builds the joint drift the refutation names and shows the
step doing exactly the work the finding said it did not do: production drifted so
that a `list` journalizes, **and** the vector drifted to agree, so the helper's
comparison against `case["journalized"]` passes and only the `Then`'s hardcoded
`matches!` on the operation name can object. It objects. **RED — 51 scenarios,
48 passed, 3 failed; all three failures are the three `list` rows, one per zone,
and all three fail on `And every mutation is journalized without consuming
mandate counters`** — the step the finding called redundant. It caught a joint
regression the helper let through. The counterfactual the finding denied is the
one the transcript demonstrates.

*Two failed attempts, named rather than hidden.* `ev-1be2a7f4` ran the mutant
**as the finding first specified it** — delete the assertion from otherwise
correct code — and came back **green, 51/51, and worthless**: deleting an
assertion in the absence of the regression it would catch tests nothing, because
green is guaranteed by construction. `ev-1d4725c7` is a compile failure on the
first build of the real mutant (`E0425: cannot find value 'entropy'`,
`bundle.rs:464`, plus an `E0308`), recorded because a red transcript that is a
build error is not a result and must not be mistaken for one. The rule these two
earned is §8.5.2, rule 2.

*What survives.* Nothing that this note carries, and the removal is a net gain in
accuracy: the refutation establishes a **positive** fact about the suite that no
Pass A auditor had, namely that `:68`'s journalization half is doubly sourced —
and `ev-ef839413` measures that both sources are needed. The mandate-counter half
of the same step is `DBND-018` and is untouched.

### `DBND-705` — removed — *"the six-way mismatch enumeration is proved by two identical sessions"*

*Frozen statement.* `:136`'s "arbitrary bytes or a mismatched Ethos, actor,
purpose, node, version or recipient are refused" is proved by
`mismatched_session_refused`, computed from a second session differing from the
first in nothing but a process-monotonic integer
(`NEXT_SESSION_ID.fetch_add`, `session.rs:23`, `:113`, `:135`); and *"arbitrary
bytes"* is vacuous by construction, because the typed methods have no byte-taking
parameter.

*The killing fact.* The identical-sessions core is **true**. What is built on it
is not: `session.rs:354`, `append_header_recipient`, takes `ephemeral: [u8; 32]`
and `nonce: [u8; 24]`, and `cucumber.rs:3082` — **inside the fixture this outline
drives** — submits `[0x76; 32]` and `[0x77; 24]` on the success path. Caller-
supplied bytes both exist and are exercised, so the second half of the statement
is false and the finding as written cannot stand.

*What survives, and why it is not re-filed.* The identical-sessions observation
is true and is not lost. Round 1 said it "is a premise of `DBND-030`, which
survived"; **`DBND-030` did not survive** — it is removed below, in round 2 — so
the true half of `DBND-705` now rests in this section and nowhere else.
Re-filing it under a new identifier would give the same evidence two numbers,
which is what §8 exists to prevent, and would reintroduce by the back door a
claim the panel refused.

### `DBND-708` — removed — *"row `:163` never reaches the symlink check its `filesystem_condition` names"*

*Frozen statement.* `| FsStore | Store key | e/circle/link-out/index.json |
intermediate link-out targets outside root |` is rejected by the store-key
grammar before `checked_join` walks a single component:
`FsStore::checked_join` (`lib.rs:553-579`) calls `validate_store_key(key)?` as its
first statement (`:554`), and `validate_store_key` has no arm for a four-segment
`e/circle/<name>/index.json`. The symlink installed at `cucumber.rs:3244-3253` is
dead fixture, and the outline's only row labelled for the intermediate-symlink
case does not test it.

*The killing fact.* **True of that row, and irrelevant.** The sibling row at
`d-bundle.feature:160`, `| FsStore | display path | folder/link-out/section |
link-out is a symlink outside the zone |`, installs its symlink at the
**intermediate** component and drives a key the grammar accepts —
`e/public/folder/link-out/section.md`, admitted by `lib.rs:157-160` — so
`checked_join`'s per-segment walk (`lib.rs:564-577`, verified by this pass) is the
sole defence making that row pass. **The case is tested.** At most one row is
mislabelled, and it passes for a correct reason.

*What survives.* The mislabelling, which is a cosmetic defect of one
`filesystem_condition` cell and is not re-filed: it changes no closure criterion
and no corrector's action. It is recorded here and nowhere else.

### `DBND-710` — removed — *"the shared `Given` constructs nothing, and `:152`'s baseline is post-attack"*

*Frozen statement.* `Given a published "<store>" bundle snapshotted byte for
byte` publishes no bundle and takes no snapshot; and the snapshot `:152` compares
against is taken **after** the attack fixture has been installed
(`cucumber.rs:3300` versus the fixture `match` at `:3232-3298`), so for five rows
it already contains the attacker's artifacts and `:152` asserts that a tampered
tree equals itself.

*The killing fact.* **The ordering is stated correctly and the conclusion is
inverted.** The fixture renames real bundle files and replaces them with symlinks
**inside the snapshot's own range** (`cucumber.rs:3256`, `:3268`). Snapshotting
*before* the fixture would therefore make `before == after` false for every
`FsStore` row **against perfectly correct code**, failing six of ten rows
unconditionally. The current order is the only one under which the assertion can
ever pass, and the only one that measures the operation rather than the test's
own attack input.

*What survives, and where it went.* The first limb — the `Given` announces a
published, snapshotted bundle and constructs nothing — was uncontested, and it is
**not lost**: RU-5 and RU-6 filed the identical defect independently and it is
published as `DBND-023`, which carries the same step function
(`core_atomic_fixture`, `cucumber.rs:11346`) and the same three feature lines.
Removing `DBND-710` costs this note nothing. The struck limb also appeared inside
`DBND-711`'s evidence, and is struck there too — see `DBND-036`.

### `DBND-503` + `DBND-714` + `DBND-715` — removed, round 2 — *"narrow is a load-bearing word with three unrelated senses"*

*Published in round 1 of this note as `DBND-020`, P2, "survived the adversarial
panel". Identifier retired.*

*Frozen statement, in three limbs.* RU-5's *narrow owner capability* has no
executed referent — `Bundle::owner_content_operation` takes `owner: &OwnerKeys`,
the whole private key set, and no `LocalSession` is constructed anywhere in RU-5.
*narrow* therefore means authority scope in RU-5 and API surface in RU-7a and
reach in RU-7b, with no step body relating any two.

*The killing fact.* `rust/crates/aithos-bundle/tests/cb2_bundle_authority_flows.rs:303-308`,
inside an executing, un-`#[ignore]`d test
(`cb2_bundle_delegated_evidence_atomicity_and_api_inventory_preliminary`;
`grep -c '#\[ignore\]'` over that file returns **0**):

```rust
assert!(BUNDLE_SOURCE.contains("&OwnerKeys"));
for absent in [
    "pub fn content_operation(",
    "pub fn open_bundle_session(",
    "pub fn export_keyless(",
] {
    assert!(!BUNDLE_SOURCE.contains(absent) && …, "{absent}");
}
```

**The absence the finding reports is a gated, asserted design decision.** The
repository does not merely happen to lack a session-based owner content surface;
it asserts that it lacks one, and asserts that `&OwnerKeys` is the parameter.
The finding's own remedy — *"route the fifteen rows through `LocalSession` and
its typed capabilities"* — would make that test fail. A finding whose closure
criterion breaks an executing test is not a finding about a proof gap.

*Second fact, same direction.* `spec/04-mandates.md:1861` defines the Owner row
verbatim: *"Local narrow capability; operation is authorized without a mandate,
journalized, and consumes no mandate counter or constraint."* That **is** the
referent the finding said RU-5's word lacked, and it is precisely what the
Rule's three `Then` steps and the vector row assert. Both facts verified in the
tree by this pass.

*What survives, and what the removal costs.* Nothing that this note carries.
The residual observation — that one English word covers two spec senses living
in two different files — is a **vocabulary observation, not a defect**: each
sense has its own normative sentence (`spec/04-mandates.md:1861` and
`spec/01-identity-and-keys.md` § 1.6), and `INVENTORY.md` § 3.1 already recorded
the ambiguity, deliberately un-numbered, as an observation for Pass A to test.
Pass A tested it and it did not hold. It returns to being an I1 observation.

**The removal also prices this note's own merge, and the price is published
rather than absorbed.** §8.1 merged three findings into one on the argument that
they were one defect with one closure action. That argument was sound, and it
had a cost nobody stated at the time: **a merged finding dies as one.** The
panel attacked the strongest limb, `DBND-503`, and the two weaker limbs went
down with it without being separately examined. Had they stayed separate,
`DBND-714` and `DBND-715` would have been P3s ruled on the record alone — and,
on the reasoning above, ruled out anyway. The outcome is the same here; the
general lesson is not, and §8.1 now carries it.

### `DBND-603` — removed, round 2 — *"line 99 claims no local-mutation orphan and the snapshot cannot see one"*

*Published in round 1 of this note as `DBND-026`, P2, "survived the adversarial
panel". Identifier retired.*

*Frozen statement.* `:99` resolves to a map comparison whose snapshots resolve
through `FsStore::canonical_base()`, which returns the generation directory named
by the `.aithos-current` pointer; the staging generations under
`.aithos-generations/`, the pointer, the mirror marker and the transient
`.tmp-*` files are therefore all outside its range, so a leaked staging
generation — the textbook local-mutation orphan — changes none of the bytes the
assertion compares. Predicted: the six `FsStore` rows stay green under a staging
leak.

*The killing fact, and its three routes.* `rust/crates/aithos-bundle/src/lib.rs:527-530`:

```rust
fn canonical_base(&self) -> io::Result<PathBuf> {
    if let Some(transaction) = &self.transaction {
        return Ok(transaction.staging.clone());
    }
```

The **first** branch returns `transaction.staging`, and `begin_transaction`
(`lib.rs:846-851`) sets that to `generations.join(&generation)` — a directory
**under `.aithos-generations/`**. So staging is inside the range of the
comparison, not categorically outside it. The refuter traced two further routes
on the reopen path: `FsStore::recover_transaction` (`lib.rs:906-909`) calls
`rollback_transaction`, then reads the pointer, and its `Err` propagates through
`Bundle::open` into the scenario helper's `?`, which panics the step; and
`reconcile_compatibility_mirror` (`lib.rs:686-692`) collects every key under the
store root and reconciles it against the active generation **before**
`reopened_snapshot` is taken, so a stray in the root mirror is copied into the
canonical view and the comparison sees it.

**OVERTURNED IN ROUND 3. Restored whole, under its own number, as `DBND-026`
(§6).** `ev-f7ee3968` runs the mutant pair the auditor named at the freeze as
this finding's closure test — `RU-6.md` § M1 **and** § M3 together, so the
staging generation leaks permanently and nothing sweeps it — and the feature gate
is **green, 51/51, 299/299**. All three routes are dead:

- **Route one** was already dead before the transcript. Round 2 of this note said
  so against its own removal: `canonical_base()`'s first branch fires only while
  `self.transaction` is `Some`, and all three snapshots are taken with it `None`.
- **Route two** — `recover_transaction` reads the pointer on the reopen path and
  its `Err` panics the step — **is the auditor's own conceded caveat, and the
  auditor had already named its answer**: § M1, which is half of the pair this
  transcript runs. It was never offered as independently sufficient.
- **Route three** — `reconcile_compatibility_mirror` copies a stray back into the
  canonical view — **is structurally blind, and the verification stopped one
  function short of finding out.** It collects through `collect_from`
  (`lib.rs:581`), whose loop at `:602-609` skips every top-level component whose
  name `starts_with(".aithos-")` — which is exactly where a leaked generation
  lives. The refutation was checked as far as *"it calls `collect_from`"* and no
  further. `ev-f7ee3968` adds a blunter fact: under M1+M3 the compiler reports
  `reconcile_compatibility_mirror` **never used** (transcript line 365), M1 having
  removed its only caller.

*Erratum, round 3.* Round 2 of this note wrote that routes two and three were
each *"enough alone"*. **That is wrong**, and wrong in the direction that made
the removal look better supported than it was: route two was never independent,
and route three never carried at all. The sentence is replaced above. The
`warden-3.md` record carried the same error and has been corrected at its three
sites.

*What survives here.* The removal itself does not. What stays in this section is
the record of a refutation that failed and of the twenty unread lines it failed
on — kept because a refutation that died on an unread helper is worth more as a
record than as a silence. The two round-1 refuter corrections — *three maps, not
two*, and *1774 lines, not 1857* — are facts about the code, are correct, and
travel with the restored finding in §6 as well as being listed in §8.4.

### `DBND-701` — removed, round 2 — *"`Then "<observable_result>"` compares strings, not propositions"*

*Published in round 1 of this note as `DBND-030`, P2, "survived the adversarial
panel". Identifier retired.*

*Frozen statement.* `:134` puts the whole assertion in an `Examples` column and
the step compares that cell to a harness literal, so **no row's English sentence
is evaluated as a claim about the system**; the only behavioural conjunct is one
boolean whose definition differs per row and whose meaning the sentence does not
constrain.

*The killing fact.* `rust/crates/aithos-bundle/tests/cucumber.rs:3011`:

```rust
let operation_succeeded = aithos_core::gamma::verify_owner_entry(&entry, &did).is_ok();
```

`did` is parsed from `did.json` read out of the store five lines above
(`:3005-3010`). That boolean is stored as row `:144`'s `operation_succeeded`
(`:3016` sets `observable_result: "the signature verifies against the public
key"` in the same struct literal) and is asserted by `assert!(observation.operation_succeeded)`
at `:8461` — the very step the finding attacks. **Row `:144`'s English sentence
is the signature verifying against the public key, and what executes is the
signature verifying against the public key, read from the DID document in the
store.** The finding's stated centre is false on the plainest row in the table.

*Its second worked example collapses too.* The finding says row `:146`'s *only*
— *only the intended recipient opens the wrapped key* — "is carried by
`mismatched_object_refused`, which is a separate step's assertion". It is
computed three lines below `operation_succeeded`, in the same fixture function,
for the same row: `cucumber.rs:3084-3086` opens with the intended secret,
`:3087-3090` fails to open with `xsk(0x78)`. Same scenario, same row, adjacent
lines. Both facts verified in the tree by this pass.

*What survives, and why it is not re-filed.* The finding's **premise** is true
and unchallenged: `assert_eq!(observation.observable_result, observable)`
(`:8460`) does compare an `Examples` cell to a harness literal, and row `:143`'s
`operation_succeeded` is a byte-equality against a golden vector with no
signature verification, under a sentence promising one. **That residue is not
re-filed under a new identifier.** Re-filing the surviving fragment of a refuted
finding is precisely how a refuted claim returns wearing a different number, and
this note refuses it on the same ground it refused it for `DBND-705`. It is
recorded here, in the section for things that did not survive, for whoever
revisits row `:143`.

*One consequence inside this note.* Round 1's §7 entry for `DBND-705` said its
true half "is a premise of `DBND-030`, which survived". `DBND-030` did not
survive. That sentence is corrected in place.

### `DBND-709` — removed, round 2 — *"`cold-load key` is a label with no distinct code path, and five of six confinement surfaces are untested"*

*Published in round 1 of this note as `DBND-035`, P2, "survived the adversarial
panel". Identifier retired.*

*Frozen statement.* `spec/02-content-tree.md` enumerates six confinement
surfaces — *"before read, write, list, edition load, staging publication, or
recovery"* — and the outline exercises one, every row being a read; and
`cold-load key` executes byte-for-byte what a `Store key` executes.

*The killing fact.* `vectors/cb2-bundle-boundaries.json:363`, the repository's
own confinement matrix:

```json
"check_applies_to": [
  "read", "write", "list", "cold_load", "staging_publication", "recovery"
]
```

The six are declared by the project as the **application points of one check**,
not as six surfaces each owing its own scenario. The finding read a list of
where a single invariant applies as a list of independent coverage obligations.

*Second fact, and it kills the other half.* The same vector classifies a
`cold_load_key` as a store key, and that classification is evaluated by
`store_key_accepted` in three independent places —
`rust/crates/aithos-bundle/tests/cb2_bundle_boundaries.rs:132` (the definition),
`:372` (`Some("store_key" | "cold_load_key" | "recovery_key") => store_key_accepted(value)`),
and `vectors/gen-cb2-bundle-boundaries.py:82`, `:149` (the generator). The
confinement of a cold load therefore **is** `FsStore::get`'s `checked_join` — the
exact call row `:165` makes. The row is not mislabelled; the finding's model of
what a cold load is was. Both facts verified in the tree by this pass, along
with the vector rows at `:242` and `:340` that carry `input_kind:
"cold_load_key"`.

*What survives.* Nothing that this note carries. Note that the round-1 refuter
had already conceded the naming half was weaker than the coverage half; round 2
removes the coverage half as well, which is the half that was carrying the
severity. **One narrow observation is left standing and is not re-filed as a
finding**, because no scenario names it: none of the ten `invalid_input` cells
contains an uppercase byte, a non-ASCII byte, or a segment over the 64-byte
bound of `name_accepted` (`lib.rs:41-47`), so the spec's *"nonconforming names"*
clause is unexercised. It is recorded here and routed nowhere.

## 8. Reconciliation across the seven units

This is the work no Pass A auditor could do: each saw one unit, and the seven
reports were frozen before any of them could see another. Everything in this
section is a change to the frozen record, and each change names what it acted on.

### 8.1 Merges — five findings into two

**A merge is only worth anything if you can say which two it was**, so each is
named with both origins and with the reason the two are one defect and not two
that half-overlap.

**Merge 1 — `DBND-105` (RU-1) + `DBND-301` (RU-3) → `DBND-003`.**
One step function, `edition_verifies` (`cucumber.rs:12697-12701`), carries two
`#[then]` attributes in two different `Rule` blocks, and its whole body is a bare
`verify().expect(…)`. RU-1 found that the sentence bound to it at `:13` asserts
neither its ordinal nor its "offline"; RU-3 found that the sentence bound to it at
`:51` asserts nothing about the value the `When` read. Neither auditor could see
the other's sentence. **They are one defect because the fix is one action** —
the step must be split before either sentence can be given its own assertion, and
a corrector who added `height == 1` to the shared body would break RU-3's use.
RU-1 saw the hazard from inside its unit and wrote the warning into its closure
criterion without knowing what it was warning about. `VERDICTS.md` § E records
the collision as one no single auditor could see.
*Severity:* the merged finding takes P2, the higher of the two.
*Evidential state:* stated per limb, because one limb has `ev-c7f65638` and the
other has nothing.

**Merge 2 — `DBND-503` (RU-5) + `DBND-714` (RU-7) + `DBND-715` (RU-7) →
`DBND-020`.**
`DBND-503` says RU-5's "narrow owner capability" has no executed referent.
`DBND-714` says *narrow* means different things in RU-5 and RU-7. `DBND-715` says
RU-7's own two halves share nothing but that word. **These are one defect at
three magnifications**: a load-bearing term used across two Rules and two
outlines with no definition and no bridge, whose two spec senses live in two
different files. The closure is a single action — fix the vocabulary against the
two spec sentences and give each Rule its own word — and three findings would
have given a corrector three ways to half-close it. RU-5 and RU-7 reported the
two-sense problem independently and blind to each other (`VERDICTS.md` § E,
convergence 3).
*Severity:* P2, from `DBND-503`.
**Outcome: the merged finding was removed by the panel in round 2 (§7).** The
merge was structurally right — one closure action — and it had a cost this note
did not state when it made it: **a merged finding dies as one.** The panel
attacked `DBND-503`, the strongest limb, and `DBND-714` and `DBND-715` went down
with it without being separately examined. Here that changes nothing, because
the same two facts dispose of all three limbs. The general lesson is recorded
because the next merge may not be so lucky: **merge for a shared closure action,
and expect the merged whole to be only as survivable as its strongest limb.**

**Merge 3 — `DBND-508` (RU-5) + `DBND-605` (RU-6) → `DBND-023`.**
Two different step functions — `core_owner_zone`/`core_owner_fixture` for RU-5,
`core_atomic_fixture` for RU-6 and RU-7b — with the **identical** statement, the
**identical** consequence and the **identical** closure criterion: the `Given`
announces a published state, constructs nothing, and a broken arrangement is
therefore reported as an assertion failure inside the first `Then`. RU-7's
`DBND-710` made the same observation a third time as its uncontested first limb.
Three auditors, three units, one shape. Merging is what makes it visible as a
shape rather than as three small complaints.
*Severity:* P3, unchanged.

**A merge deliberately not made, recorded so the choice is visible.**
`DBND-033` and `DBND-034` (old `DBND-706` and `DBND-707`) share one confirming
mutant, `ev-2d2ebd1b`, and both concern `:148`'s inability to fail. They are kept
separate because their closure criteria are different edits by different logic —
one asks that the *kind* of refusal be asserted, the other asks that something be
allowed to succeed — and a single finding carrying both would let a corrector
close half of it and mark it done. **Merging two distinct closure actions is not
a merge, it is a loss of resolution.**

### 8.2 A split — one finding into two

**`DBND-712` (RU-7) → `DBND-037` + `DBND-038`.**
Pass A filed one P3 covering two rows of `:148`. Row `:162`'s defect is that a
correct test sits under the wrong `Then` — a **Gherkin sentence** defect, closed
by splitting `:151`. Row `:161`'s defect is that the escape detector cannot fire
because the fixture puts the target where the key cannot resolve — a **fixture
geometry** defect, closed by moving a file. Different artifacts, different edits,
and the Pass A closure criterion was already two clauses joined by a semicolon. A
corrector closing one must not be able to believe it closed both.

### 8.3 Rulings on the 23 P3 findings, made on the record alone

The 23 P3s had no mutant and no panel. **That was a budget decision recorded in
`VERDICTS.md` § D — the panel budget was spent where it changes what a corrector
does — and this note does not pretend otherwise.** Each P3 block above says *on
the record alone* in those words. The rulings that changed something:

| Ruling | Finding | What changed |
|---|---|---|
| **Folded, as a restatement of a confirmed P1** | `DBND-021` (old `DBND-506`) | its `mandate_counter_delta` limb is `DBND-018`, which is P1 and confirmed by `ev-19a635cf`. Struck from the P3; five fields become four |
| **Folded, as a restatement of a confirmed P2** | `DBND-027` (old `DBND-602`) | its `:121` limb is the whole of `DBND-025`, confirmed by `ev-7caa8332`. Struck; nine `Then`s asserting five bits become nine asserting four |
| **Struck, because it rested on a claim the panel killed** | `DBND-036` (old `DBND-711`) | its post-attack-baseline limb cited `DBND-710`. The panel killed `DBND-710` on the fact that snapshotting before the fixture would fail six of ten rows against correct code. That limb is struck **with the killing fact printed in the finding**, not silently removed. The three-comparator and vacuity limbs are independent and stand |
| **Answered from the specification instead of routed** | `DBND-011` (old `DBND-205`) | Pass A routed a protocol question — is a circle read obliged to re-check the pin? — and declined to settle it from code. `spec/02-content-tree.md` § 2.11 places the circle signature *inside the seal* and imposes no read-side pin obligation for that zone, so the asymmetry with `public_read` is conformant. The finding is narrowed to what remains: the replay case is asserted nowhere |
| **Spec ground strengthened, severity deliberately not raised** | `DBND-012` (old `DBND-303`) | § 2.11's *"A verifier rejects any owner signature whose embedded placement does not match where the object actually sits (fail-closed)"* is stronger than the ground Pass A cited. The severity stays **P3**, because raising it to P2 would put it in the class the panel examined without it ever having faced the panel. Recorded as a scope judgement, and routed |
| **Merged** | three findings | §8.1 |
| **Split** | one finding | §8.2 |
| **Gained a transcript** | `DBND-028` (old `DBND-604`) | §8.5 |
| **Two P3 folds lost their parent** | `DBND-027`, `DBND-039` | round 1 folded a limb of `DBND-027` into `DBND-025` and treated `DBND-039` as a by-product of `DBND-030`. `DBND-030` is removed (§7), so `DBND-039` stands alone; `DBND-025` survives, so `DBND-027`'s fold stands. Both are stated in the blocks themselves |

**Every other P3 is carried unchanged**, and each says in its own block that it
is on the record alone.

### 8.4 Corrections carried, not silently fixed

An audit that quietly repairs what it got wrong is worth less than one that shows
the correction.

| Correction | Origin | Where it is published |
|---|---|---|
| *two maps* → **three maps** are compared: `before == after && before == reopened_snapshot` | the `DBND-603` refuter | in the `DBND-026` block, at the top, before the statement |
| *1857 lines* → **1774 lines**: `cb7_store_snapshot` is at `cucumber.rs:1375`, `core_path_raw_snapshot` at `:3149`, not `:3232` | the `DBND-603` refuter | same block. Re-verified against the current tree by this pass |
| *the six `MemStore` rows go red* → **four of six** | the control `ev-f0125e0b` | in the `DBND-027` block, with the stake it was attached to and the reason it did not fire |
| *19 `pub fn` in `session.rs`* → **18** | this pass, `grep -n "pub fn" session.rs` | in the `DBND-029` block |
| *`core_path_raw_snapshot` at `:3232-3288`* (RU-6) versus *at `:3149-3193`* (RU-7) — the two Pass A reports disagreed | this pass | `:3149` is correct; `:3232` is the start of the fixture `match`. This is the same error as the 1774 correction, and the refuter found it first |
| *seven Pass A auditors* → **six auditors over seven units** | the warden, breach (D) | §2, §6.1 and §15. `frozen.json` says seven and is **not edited**: it is hashed and cited by that hash, and a freeze that can be corrected afterwards is not a freeze |
| *seventeen mutants* → **nineteen distinct code edits, twenty transcripts, eighteen experiments** | the warden, breach (F), counting the ledger's own arrays | §2 and §4.2. No evidence identifier, verdict or counter moves with it |
| *§4.2 headed "the fifteen confirming mutants"* over **fourteen** rows | the warden, breach (F) | §4.2, re-headed and the arithmetic spelled out |
| *`verify_links` at `bundle.rs:1770`* → **`:1772`** | the warden, breach (G) | in the `DBND-018` block. The panel's own `DBND-504` note had it right, so the repository disagreed with itself |
| *"the Pass A reports are committed in this repository"* → **they were not** | the warden, breach (A) | §1. They are now, and §1 lists the paths |
| *"the audited bytes and the current bytes are the same bytes"* → **false for the feature file** | the warden, breach (A) | §1 worktree row, and §5.2 |
| *round 1's feature-file line citations, published against a file 99 lines longer* | the warden, breach (B) | §1 convention block and §5.2 |
| *four findings "survived the adversarial panel"* → **none did** | the warden, then a second refuter on all four | §7, four new entries; the evidential state itself is struck in §6 |
| *"the panel killed everything it examined … a P1 or P2 finding that met a serious adversary did not survive it"* → **wrong, and drawn from citations rather than transcripts** | round 3, `ev-f7ee3968` and `ev-f18d4843` | §3, replaced by a narrower claim; the rule that replaces the mood is §8.5.2 |
| *`DBND-603` removed* → **restored whole as `DBND-026`** | `ev-f7ee3968` | §6 block and §7 entry, with the twenty unread lines of `collect_from` that the refutation died on |
| *`DBND-504` removed* → **restored narrowed as `DBND-040`** | `ev-f18d4843` | §6 block and §7 entry; the refuted sub-claim stays refuted |
| *§7 `DBND-603`, "either alone is enough"* → **neither route carries alone and neither carries together** | round 3 | §7 entry, as a dated erratum. The same error in `warden-3.md` is corrected at its three sites |
| *§15, "Seven independent assessments"* → **six** | the warden's `VALID` errata | §15, below. Last residual of the seven-auditor miscount, in the one section whose job is to be trustworthy about method |

Both `DBND-603` corrections were described by their refuter as not load-bearing,
and they are not: the finding stands on either arithmetic. They are published
because the reader is entitled to know that the number in the frozen report is
not the number in the tree.

### 8.5 What the transcripts gave that nobody asked them for

`ev-f0125e0b` was run as a **control** for RU-6 — to prove that S10's byte
comparison can fail at all. It also answers a question from a different finding.

Its per-row results for the twelve rows of `:91`, read from the journalled
transcript:

| Row | Result under a `MemStore` rollback that commits |
|---|---|
| `MemStore \| cryptography` | **green** |
| `MemStore \| blob preparation` | red at `:95` — `pinned file altered: e/circle/index.json` |
| `MemStore \| index preparation` | **green** |
| `MemStore \| header or wrap` | red at `:95` — same message |
| `MemStore \| Gamma validation` | red at `:95` — same message |
| `MemStore \| before state replacement` | red at `:96` — `canonical_unchanged` |
| all six `FsStore` rows | green, as designed: the mutant is `MemStore`-only |

The two survivors are exactly the two rows `DBND-028` says collapse into one
injection point, and the four that die are the four whose fault fires after the
first write has landed in the overlay. **`DBND-028` therefore moves from *on the
record alone* to *partly confirmed by transcript*** — the indistinguishability is
measured; the stronger *identical call* claim still needs the probe, which is
named in that block as proposed and unrun. And the three rows that die at `:95`
rather than at `:96` are a measurement of `DBND-023`'s stated consequence: a
fixture-side failure surfacing as an assertion failure in the first `Then`.

`VERDICTS.md` records this as *"two rows it believed `MemStore`-backed are not,
and that is its error about its own fixture"*, and instructs Pass B not to repeat
the six. **The instruction is honoured and the fact is now more precise than the
instruction**: both rows are `MemStore`-backed. What they do not reach is a
non-empty overlay, because both faults fire on the same first write.

### 8.5.1 Two more of the orchestrator's own numbers, corrected in the same place

The warden found both, this note publishes both, and they belong here rather
than in a footnote because they are the same kind of thing as the paragraph
above: a number in an authority that does not survive contact with the artifact
it summarises.

- **"Seven independent auditors" — there were six.** `frozen.json`'s `note`
  field and ledger line 20 both say seven. `pass-a/` holds six reports and no
  `RU-4.md`; `RU-3.md` is headed for RU-3 **and** RU-4 and reads them as one
  sitting on `INVENTORY.md` § 1.8's own recommendation. Round 1 of this note
  disclosed the six correctly in §2 and then wrote "seven" in §6.1 and twice in
  §15, so the audit contradicted itself three times inside one document. Six is
  the number. **`frozen.json` is not edited** — it is hashed, it is cited by that
  hash, and the whole evidential value of a freeze is that it cannot be tidied
  afterwards. The correction lives in the ledger and here.
- **"Seventeen mutants" — there were nineteen distinct code edits.** Counting the
  ledger's own arrays: `wave_1` has 11 items, `wave_2` has 7, one of which
  (*RU-5 parameter reachability*) is two separate edits with two transcripts; the
  gate lines between the freeze and the panel are 20 transcripts, of which
  `ev-19a635cf` and `ev-bec6b91e` measure the **same** mutant by two commands.
  That is 19 distinct edits, or 18 experiments under the ledger's grouping. The
  number 17 comes from `wave_1`'s prose — *"ten predictions, ten confirmations"* —
  which silently drops the `bundle.rs:1658` control from the count of the array
  printed directly beneath it. **The control that was dropped from the count is
  `ev-5474b889`, one of only two runs in this whole campaign that demonstrate a
  scenario catching something.** Losing it from a total is a small arithmetic
  slip; losing it from the argument would not have been.

Neither correction changes an evidence identifier, a verdict, a counter or a
finding. Both are published because this note's subject is claims that do not
match what happened, and it does not get to hold its own authorities to a lower
standard than it holds the feature.

### 8.5.2 Two rules this cycle earned

Written as rules, not as anecdote, because both were paid for.

**Rule 1 — a refutation whose decisive fact is a claim about what the code does
at runtime carries a transcript, not a citation.**

Sort the ten refutations by the kind of fact they turn on.

**Six turn purely on existence claims** that a reader settles by looking, and all
six hold: an object with zero writers (`DBND-302`), a signature that does take
bytes (`DBND-705`), a sibling row that drives the case (`DBND-708`), a fixture
that plants its symlinks inside the snapshot's range (`DBND-710`), a `let`
binding that evaluates the sentence (`DBND-701`), a JSON array that declares the
scope (`DBND-709`), an executing test that asserts the absence (`DBND-503`) —
seven statements across six findings, none of which moved.

**Three turn on claims about runtime behaviour** — what a snapshot would contain,
what a step would catch, what a check would reject. All three were put to the
gate in round 3, and **two of them fell**: `DBND-603` and, on its generalising
half, `DBND-504`. `DBND-505` was confirmed.

**One is mixed, and it is the instructive one.** `DBND-504`'s refutation made an
existence claim — `check_form` does read four of the seven fields — which is true
and stands, and then generalised it into a runtime claim about what the suite
would catch, which is false. **The existence half survived and the runtime half
did not, inside a single refutation.** That is the rule in miniature.

**State the rule as what it is: a filter, not a predictor.** It tells you which
refutations to spend a transcript on; it does not tell you they are wrong. One of
the two it selected here — `DBND-505` — was **confirmed** by its transcript, and
confirmed more strongly than the citation had it. A reader who takes only the
`DBND-026` story away will conclude that panels are unreliable, and that is the
wrong lesson: the panel was right eight times out of ten and its two errors were
both in the class this rule identifies in advance.

**Rule 2 — a mutant that removes a check must be paired with an input the check
would have rejected, or green is guaranteed by construction.**

Deleting an assertion from a suite that is passing proves nothing: nothing was
failing, so nothing stops failing. The mutant must first create the defect the
check exists to catch, and only then remove the check. `ev-1be2a7f4` is the
counter-example, run and recorded: the `DBND-505` assertion deleted from correct
code, **green, 51/51, and worthless**. `ev-ef839413` is the same mutant done
correctly — the joint drift first, the check second — and it is **red on three
rows**.

This repository already knew this rule and lost it for a round. §4.2 states it in
one line — *"an audit that only runs mutants it expects to survive is measuring
its own confidence"* — and `RU-6.md` names M3 and M4 as a **pair** precisely
because M3 alone leaves `recover_transaction` standing as a defence. **A blind
Pass A auditor had the discipline; the panel round did not.** Both mutants
specified in round 3 violated the rule on first draft, and one of them would have
produced a false restoration had its green been banked. The rule is written here
so the next round does not have to rediscover it.

## 9. Recorded follow-ups this feature already owes

Seven `QUEUE.yaml` entries name `d-bundle`. They were recorded before this domain
existed, by impact reviews that were independently accepted. This note **states
where each lands and what would discharge it**; it opens none of them as a
`DBND-*` finding, and it settles no scope question reserved to another owner.

| Key | Lands where | State after this round |
|---|---|---|
| `chdr-028` | `publication.rs` (`verify_draft2_candidate` `:469`, `verify_public_only` `:586-591`, `verify_for_cas` `:643-650`, `export_keyless` `:651-694`), `sdk.rs:35` | **owed and not discharged.** `d-bundle` is at position 2 of `order:` and `k-integration` at 16, so this cycle is first of the two and owes it. **No scenario of this feature touches any of those surfaces**: `grep -n 'cold_verify\|import_keyless\|export_keyless' cucumber.rs` puts every hit in the keyless/cold publication family, none in a step reached by `d-bundle.feature`. Its closure criterion is unchanged — `verify_draft2_candidate` calls `verify_pinned_headers` on `context.candidate_store` and `did.json`, with a RED test. Published in full at `docs/audits/features/c-headers.md` §6bis; **cited, not re-embargoed** |
| `chdr-i3-d-bundle` (`CHDR-034`, `CHDR-030`) | `Bundle::publish` (`bundle.rs:1678`), `Header::validate_as_owner` (`aithos-core/src/header.rs:385-401`), `bundle.rs:667`, `:674` | **owed and not discharged.** Both named surfaces are *executed* by RU-1: `publish` by step `:19`, and `verify` including its I3 tier `verify_pinned_headers` (`bundle.rs:302-321`, called at `:1759`) by all four `Then`s. **No scenario of this feature asserts anything about the I3 owner line.** The debt is correctly recorded as owed by this cycle; nothing here discharges it and nothing here contradicts it. Related surface note: this feature's two mutation steps (`alter_pinned_file`, `wrong_predecessor`) write through the `pub store` field (`bundle.rs:284`), the same injection path `c-headers.md` §6bis records for `c3_owner_line_edition.rs:239-246` |
| `chdr-016-grant-path` | `Bundle::grant` (`grants.rs:739`) → `deliver_entry` (`:754`) → `add_line_on` (`:276-305`) | **not carried by this feature's scenarios, and the assignment is routed, not settled.** `QUEUE.yaml` requires whichever of `g-revocation` and `d-bundle` opens first to state who carries it; `d-bundle` is at position 2 and opens first. The evidence this round can contribute is one fact and it is a negative: **no step of any of the 51 scenarios reaches the grant path.** All seven Pass A units searched their own step bodies and none found `Bundle::grant`, `GenericGrantRequest` outside the CB9 delegated helpers (`cucumber.rs:3554+`), or any `grants.rs` entry point. Assigning the debt on that basis is a cycle-level decision for the run report and the orchestrator, not an audit finding, and this note does not make it |
| `bder-006-d-bundle` | `rename_the_folder` (`cucumber.rs:8394`), `publish_edition` (`:8343`), `reads_at_new_path` (`:12748`); and the `wrap` uses at `:98`, `:106`, `:112`, `:138`, `:146` | **the co-owned-steps record is made here, which is what `../orchestrator/STATE.md:32-42` says `d-bundle` owes "either way".** All three named steps are RU-2's, and `publish_edition` additionally carries RU-1's `:19` through a second `#[when]` attribute on one function. `DBND-009` is the finding that names the coupling's consequence. The `wrap` reading is confirmed from inside RU-6: `:98`, `:106` and `:112` enumerate `wrap` among the artifacts a failed mutation must not leave, and `DBND-028` shows the two rows that name header-and-wrap atomicity interrupt a **read**, so those conjuncts are vacuously satisfied. `:138`/`:146`'s `wrap` is `HeaderWrappingCapability`, unrelated to tag-view anchors — RU-7 confirmed that reading. **The scope re-arbitration is reserved to the owner of the `BDER-006` decision and is not made here.** The feature still has no tag-view scenario |
| `b-derivation-round-2-targeted` | names this feature | content is the row above |
| `chdr-i3-targeted` | names this feature | content is `chdr-i3-d-bundle` |
| `chdr-lota-vector-generators` | conditional — binds the first cycle to touch a vector | **triggered, and satisfied.** This feature's scenarios consume `vectors/cb2-bundle-authority-flows.json` (RU-5) and `vectors/cb2-bundle-boundaries.json` (RU-6). Both generators **have** a `--check` mode — `gen-cb2-bundle-authority-flows.py:383-387` — and neither is among the nine that have none. `DOMAIN.md` § *Vector `--check`* names the commands. Nothing in this note edits a vector |

Two further follow-ups name no feature and reach this cycle
(`chdr-lota-proxy-verdicts`, `chdr-lota-source-text-assertions`). Both are
discharged here rather than deferred: the proxy class is measured absent (§10),
and the one source-text assertion in the Gherkin layer is **classified** by
`DBND-032` — the queue entry said the site was "counted, not classified", and
`ev-794d59c3` classifies it.

## 10. Shared-state pass — negative results

Recorded because a verified negative is worth more than an unverified absence,
and because this feature's largest risk was named in advance and did not
materialise.

- **The proxy class does not reach this feature, and it is measured on both
  columns of the largest outline.** This repository carries 360 Gherkin lines
  resolving to 19 cached-verdict step definitions, and `chdr-lota-proxy-verdicts`
  lists nine features exposed to it. `d-bundle` is not among them, and three Pass
  A auditors reproduced the search rather than inheriting it: the eight
  process-lifetime `OnceLock` verdicts are at `cucumber.rs:1119-1129`, their
  `*_result` helpers at `:7287-7356`, and the only `cb7_result` call site is
  `:9592`, inside an `o-connector-classes-vault` fixture. **No step body reached
  by any of the 51 scenarios calls a `*_result` helper or reads a cached verdict.**
  Then it was measured: `d-bundle.feature:87` `self` → `vault` is **red 1/51**
  (`ev-f0658ee9`) and `:73` `list` → `enumerate` is **red 1/51**
  (`ev-de8fa887`). Both columns of the 15-row outline reach code that errs on an
  unknown value. RU-5 reports 15 of 15 rows distinct, RU-7 14 of 14, RU-6 real
  fault injection into a `Store` decorator.
- **`World` instantiation.** `ProtocolWorld` is `#[derive(Debug, Default, World)]`
  (`cucumber.rs:467`), so cucumber-rs constructs a fresh `World` per scenario. No
  observation crosses a scenario boundary; the fifteen rows of `:63` never
  coexist, which is also why `DBND-024`'s comparative check is impossible as the
  code stands.
- **Fifteen scenarios, fifteen distinct fixtures.** Each row of `:63` gets its own
  `Cb7TempRoot::new("core-owner-{zone}-{operation}")` (`:3383`) — no shared
  directory, no cross-row leakage. RU-6's and RU-7b's rows likewise build from
  `core_atomic_bundle` (`:1699`) per scenario.
- **Source-text assertions: exactly one, and it is in RU-7a.** Search, scope the
  Gherkin layer: `grep -rn 'include_str!("../src/'
  rust/crates/aithos-bundle/tests/cucumber.rs` → one line, `:2054`. The other 51
  `_SOURCE.contains(` sites live in five non-Gherkin binaries
  (`cb2_bundle_boundaries.rs`, `cb2_bundle_authority_flows.rs`,
  `cb2_draft2_carriers.rs`, `cb2_bundle_structure_vault.rs`,
  `cb2_bundle_concurrency_final.rs`), none executed by a scenario of this
  feature. The one site is `DBND-032`.
- **Step bodies carrying more than one sentence — the real sharing graph.**
  `INVENTORY.md` § 4.6 gave the textual lower bound, four repeated step texts.
  The measured graph is larger and crosses `Rule` boundaries in three places:
  `edition_verifies` (`:12697`) carries `:13` **and** `:51` — RU-1 and RU-3,
  `DBND-003`; `publish_edition` (`:8343`) carries `:19` **and** `:42` — RU-1 and
  RU-2, `DBND-009`; `core_atomic_fixture` (`:11346`) carries `:92`, `:117` **and**
  `:149` — RU-6 and RU-7b, `DBND-023`; `core_atomic_unchanged` (`:11393`) carries
  `:96` **and** `:152` — RU-6 and RU-7b, `DBND-036`; `a_published_bundle`
  (`:7706`) carries `:23` **and** `:28`, within RU-1; and
  `d_capability_boundary_holds` (`:8477-8490`) carries `:136`, `:137`, `:138`
  **and** `:139` — four sentences, one `Rule`, three assertions. That was
  `DBND-030`'s second limb; the finding is removed (§7) and **the structural
  fact is unrefuted and now carried by no finding**, which is recorded here
  rather than lost.
- **Production surfaces this feature does not traverse.** `Bundle::grant`
  (`grants.rs:739`), the whole of `publication.rs`'s draft.2 verification family,
  `Header::validate_as_owner` (`aithos-core/src/header.rs:385-401`), and
  `Bundle::public_read_k1c` (`bundle.rs:1296`, which no code path can feed — §7).
  The first three are §9's debts. `aithos-wasm` exposes no bundle-edition surface
  reached here.
- **The production code was attacked and held.** Recorded positively, because the
  seven reports are otherwise read as uniformly negative. `validate_store_key`
  (`lib.rs:142-231`) is a genuinely closed grammar, not a traversal blacklist — a
  key must match one of about fifteen exact forms. `FsStore::checked_join`
  (`:553-579`) validates first, then walks every prefix with `symlink_metadata`,
  rejects any symlink component including the final one, and treats `NotFound` as
  acceptable so the walk is not defeated by ordering; `collect_from`
  (`:581-635`) applies the same check to listings and re-validates every
  discovered key. The self zone's structure secrecy is held by four independent
  mechanisms — the closed store-key allow-list, the sid-only `SelfIndex`
  (`bundle.rs:77-90`), sealed folder descriptors (`:1118-1126`), and the
  non-public seal branch of `log_owner_mutation` (`log.rs:211-224`).
  `Bundle::public_read` (`bundle.rs:1264`) takes no key of any kind: keylessness
  is a **type-level** fact, stronger than any `Then` could make it, and it is the
  one claim in this feature proved better than a scenario could prove it.
  `Bundle::verify` takes `&self` and no key, opens no blob. **No finding in this
  note claims that any of this is broken.**

## 11. Implementation plan

Ordered by value. The whole of it is test and fixture work in
`rust/crates/aithos-bundle/tests/cucumber.rs` plus a Gherkin edit. **No finding
in this note requires a production change in `aithos-core` or
`aithos-bundle`.**

**Lot A — the two constants (P1).** `DBND-018` and `DBND-029`. Compute
`mandate_counter_delta` from the appended Gamma entries, or call
`gamma::verify_owner_entry` on each; compute `secret_material_exposed` from an
executed attempt, or delete `:139` and discharge it with a `trybuild` compile-fail
case. Both are two-line assertions against machinery that already exists. RED
demonstration required: `ev-19a635cf` and `ev-ed18d7ef` must each turn a scenario
red.

**Lot B — the three `SEMANTIC_FALSE_POSITIVE` scenarios.** `DBND-001` (`:27`),
`DBND-008` (`:39`), `DBND-014` (`:55`). Each closes when its already-run mutant
turns its scenario red: `ev-d1fc33b5`, `ev-f7261aa9`, `ev-0b4e1076`. These are the
three scenarios a reader would otherwise count as proof.

**Lot C — the shared step bodies.** `DBND-003` first, because splitting
`edition_verifies` is a precondition for giving `:13` and `:51` their own
assertions; then `DBND-023`; then `DBND-036`. Ordering matters between the first
two and nowhere else. *(Round 1 chained `DBND-023` to `DBND-026`'s pre-mutation
raw snapshot; `DBND-026` is removed, §7, and the chain with it.)*

**Lot D — the remaining P2s.** `DBND-002`, `DBND-007`, `DBND-013`, `DBND-019`,
`DBND-025`, `DBND-026`, `DBND-031`, `DBND-032`, `DBND-033`, `DBND-034`,
`DBND-040` — **eleven**: `DBND-020`, `DBND-030` and `DBND-035` were removed by
the panel, and `DBND-026` and `DBND-040` came back from it with transcripts
(§7). **Every one of the eleven carries its own closure criterion and an
already-run mutant that must go red.**

**The corrector's lot is seventeen findings — every P1 and every P2 — and all
seventeen are confirmed by transcript.** That is the whole of the actionable
content of this note, and it is the number to plan against.

**Lot E — the twenty P3s.** They are on the record alone and none blocks a lot
above. Two of them (`DBND-021`, `DBND-027`) are partly discharged as by-products
of lots A–D; their markers stay until an independent review closes them, per the
marker lifecycle `c-headers.feature` uses. `DBND-039` was in that group in round
1 because `DBND-030` would have discharged it; with `DBND-030` removed it is
**no longer a by-product of anything** and must be executed on its own.

**Two mutants that must be re-run and must go red before this note can move to
`REVIEW_ACCEPTED`**: `ev-2d2ebd1b` (one row of `:148`) and `ev-794d59c3` (one row
of `:131`). Both currently leave the gate wholly green.

## 12. Decisions required

Two, and neither may be taken silently by a corrector.

1. **`chdr-016-grant-path` — which of `d-bundle` and `g-revocation` carries it.**
   `QUEUE.yaml` requires the first to open to state it, and `d-bundle` opens
   first. The audit evidence is a negative — no scenario of this feature reaches
   the grant path — which argues for `g-revocation`, but the argument is a scope
   argument and not an audit finding. **Owner: the orchestrator, in the run
   report.**
2. **`DBND-012`'s routing.** The obligation is `spec/02-content-tree.md` § 2.11's
   — *"A verifier rejects any owner signature whose embedded placement does not
   match where the object actually sits (fail-closed)"* — and no `Rule` of
   `d-bundle` names it. The choice is between writing the verifier and amending
   § 2.11 to say the field is informational. **Owner: the protocol owner.** No
   corrector receives it from this note.

*Round 1 listed a third item here in order to exclude it — `DBND-020`'s
vocabulary question — on the ground that a corrector could execute it without a
ruling. `DBND-020` was removed by the panel (§7) and the exclusion is moot: the
two spec sentences that would have grounded the rewording are the same two facts
that killed the finding.*

## 13. Limits of this conclusion

- **Twenty of the thirty-seven findings rest on reading alone.** Every one says so
  in its own block. A P3 in this note is a claim about what the source says, not
  a measurement, and it has not survived an adversary.
- **No finding in this note survived an adversary. Two had their adversary
  overturned, which is a different and weaker thing.** The panel examined ten
  findings; eight removals stand and two were restored because a transcript
  falsified the refutation, not because the finding withstood it. The seventeen
  findings that stand on a transcript were never attacked — the owner ruling
  scoped the panel to findings without a confirmed mutant — and the twenty P3s
  were never attacked either. **A reader should treat the seventeen
  mutant-confirmed findings as the load-bearing content of this note and the
  twenty P3s as unaudited claims about source text.**
- **This note's own count of findings moved three times, and twice it was
  published wrong.** Round 1 gave four survivors that were not survivors — one
  refuter per finding where `c-headers` used three, and the orchestrator's
  escalation rule unapplied. Round 2 corrected that and drew a confident
  inference from it that round 3's transcripts falsified. **The general lesson is
  that a single refuter's failure to refute is not evidence, and neither is a
  single refuter's success**: what settled all three positions in the end was
  running the mutant.
- **Nineteen mutants confirm what a scenario does *not* catch. Three confirm that
  a scenario catches what it should**, and all three had to be argued for:
  `ev-5474b889`, `ev-f0125e0b` and `ev-ef839413`, covering three scenarios out of
  thirteen. A suite is not shown to work by showing that it fails to fail.
- **`DBND-032`'s dead-code limb is not exhaustively established.** The
  unreachability of `binding.class != class` rests on Rust module privacy and on
  nine call sites in one file. No workspace-wide search for `mem::transmute` or a
  `#[cfg(test)]` constructor was made.
- **The non-Unix path was not determined.** `core_path_fs_scenario` has a
  `#[cfg(not(unix))]` twin returning `Err("CORE-OWN-004 symlink scenarios require
  Unix")` (`cucumber.rs:3339-3346`), which `core_path_refused_before_access` turns
  into a panic. Six of `:148`'s ten rows fail outright off Unix. Whether CI is
  Unix-only was not established by this note.
- **`AuditArgsCapability` is a fifth capability class the `Examples` table of
  `:131` does not name** (`session.rs:73`, `:221`, `:369`, `:375`). Whether that
  is a coverage gap or a correct exclusion needs the D9 audit/config derivation
  topology, which `spec/01-identity-and-keys.md` says is "reserved for the CB2
  vectors". **Routed, not settled**, and deliberately not filed as a finding
  since no scenario names it.
- **`PROCESS.md` does not contain the section several instructions cite.** RU-7
  recorded that `features/.agents/PROCESS.md` has eleven `##` headings and no
  occurrence of "Material isolation", "disclosure" or "blocking condition";
  `d-bundle/DOMAIN.md:790-796` and `g4-client-surfaces/DOMAIN.md:577-591` record
  the same gap, and `c-headers.md`'s `CHDR-040` is the finding for it. **The rule
  that produced §15 of this note is written nowhere in `PROCESS.md`.** Reported,
  not fixed: this note does not edit `PROCESS.md`.

## 14. Definition of done

This note moves to `REVIEW_ACCEPTED` when all of the following hold.

1. The feature gate is green with the contract counters unchanged: **1 feature /
   7 rules / 51 scenarios / 299 steps**, under a fresh `evidence_id`.
2. `bash features/.agents/scripts/verify-feature-tags.sh` is green after the
   marker edit.
3. Every closed finding has a **RED demonstration on the audited baseline for the
   named reason**, not merely a green suite afterwards.
4. The workspace gate and `clippy` are green, with `--no-fail-fast`
   (`DOMAIN.md` § *Gate pyramid*; `chdr-lota-clippy-and-fail-fast`).
5. The Gherkin markers of `features/d-bundle.feature` name every finding still
   open against each scenario, and no marker survives its finding's closure by an
   independent review.
6. The two decisions of §12 are recorded by their owners.

**What this note does not establish, stated so that a green gate is not misread.**
Thirty-seven named defects in the *proof* of this feature are open. Closing all of
them would establish that the 51 scenarios prove what they say. It would not
establish that the feature's seven Rules cover the specification: §10 lists
three production surfaces this feature does not traverse at all, and §7's
`DBND-709` entry leaves one unexercised spec clause standing with no finding to
carry it.

## 15. Disclosure-gate trace

The barrier is recorded here because an audit that erased the mechanism
constraining it would not be an honest audit.

| Step | Date | Fact |
|---|---|---|
| 1 | 2026-08-04 | Pass A: **all six** auditors assess blocking condition 9 independently, across all seven units, and all six raise nothing, each recording its reasoning rather than its conclusion. `frozen.json`, field `disclosure`: *"every finding is a gap between a Gherkin sentence and the assertion behind it, and the production code was read and found intact at `d9120d7`"*. (That same field says "seven auditors"; it is wrong on the count and is not edited — §2, §8.5.1. Round 1 of this note repeated "seven" here and at step 5, twice in one table, and the warden caught both) |
| 2 | 2026-08-04 | RU-7 names the one candidate it can see, `DBND-703`, and pre-writes the redaction that would preserve it — identifier, severity, and the neutral title *"a secret-exposure assertion is backed by a constant"* — while recommending against retention |
| 3 | 2026-08-04 | RU-3 names two candidates for Pass B to re-judge, `DBND-302` and `DBND-303`, and states that their neutral titles are already their section headings so the full text could be lifted out without touching the rest |
| 4 | 2026-08-04 | The panel kills `DBND-302` (§7). Its embargo candidacy falls with it |
| 5 | 2026-08-04 | **This pass re-assesses rather than inherits**, on the ground that a barrier only ever inherited has stopped being one. Three candidates examined in full; the reasoning is below. **None retained. Nothing embargoed. Nothing withheld from this file or from any tracked file** |
| 5b | 2026-08-04 | **Round 2: the barrier is re-passed over the four late removals, not inherited from round 1.** `DBND-503`/`-714`/`-715`, `DBND-603`, `DBND-701` and `DBND-709` move from §6 to §7, and a removed finding's statement is published in full beside the fact that killed it. Each was re-read against condition 9 in its new form. None describes an exploitable weakness: three are statements about assertions, and the fourth (`DBND-603`) is a prediction about a staging directory that two independent code paths make visible. **None retained.** The killing facts themselves were also checked: they cite an executing test, a `match` arm, a `let` binding and a JSON array, and none of them is a technique |
| 6 | 2026-08-04 | **The tables were re-checked against the gate separately from the prose**, because `c-headers.md`'s leak was caught twice by the warden in places nobody thought to check — an impact table and a comparison table. The tables re-checked here are §1's two new tables, §4.1, §4.2, §5, §5.1, **§5.2**, §6.0, §7's summary table, §8.3, §8.4, §8.5, §9 and this one. No cell of any of them carries a mechanism a prose block withholds, because no prose block withholds one. The warden independently read **sixteen tables, 191 table lines and all thirteen Gherkin markers** for the same purpose in round 1 and found nothing; §5.2 is the only table added since |
| 7 | 2026-08-04 | The owner's ruling of the same day published three previously embargoed statements in full — `chdr-028`, `spec-cons-12`, and the code edge of `spec-cons-05`. `chdr-028` binds this cycle and is **cited in §9 and not re-embargoed**. The ruling is a publication decision on three named statements and **was not read as relaxing condition 9**; the assessment below was made against the condition as written |

### The three candidates, and why none is retained

Condition 9 bites when a finding's **statement** would describe an exploitable
weakness **for which no fix exists**. Both halves must hold.

- **`DBND-029`** (`:139` is `assert!(!false)`), and the mutant `ev-ed18d7ef` that
  adds a private-key accessor. The property **holds** at `d9120d7`: 18 `pub fn`
  in `session.rs`, none returning key material, verified by this pass and not
  inherited. The statement describes a proof gap, not a weakness. The mutant is a
  patch to a crate an attacker would already need write access to modify, and
  `chdr-lota-mutants-as-patches` requires every mutant to be published as an
  applicable diff. A fix exists and is named in the closure criterion. **Not
  retained.**
- **`DBND-032`** and its `sign_any` mutant `ev-794d59c3`. Same shape, same
  conclusion: the oracle does not exist at the audited revision; the finding
  states that the grep would not see it if it did. **Not retained.**
- **`DBND-012`** (the owner content signature nothing verifies) — **the only
  candidate whose statement touches shipped behaviour rather than a test.**
  Examined on both halves. *Exploitable?* The placement binding a verifier would
  provide is independently held at `d9120d7` by two mechanisms: the manifest's
  flat pins bind path → sha256 inside a signed edition (`bundle.rs:1749-1755`),
  and `public_read` refuses a body whose hash does not match its row
  (`:1280-1284`). A detached artifact re-placed under a different path fails the
  pin. *No fix exists?* A fix exists, is cheap, and is named: write the verifier,
  or drop the field and amend § 2.11. **Neither half holds. Not retained**, and
  the finding is published in full above.

### What this episode establishes

- **The barrier was exercised, not asserted.** **Six** independent assessments
  across seven units, two named candidates carried forward with pre-written
  redactions, one killed by the panel, three re-judged here from the code rather
  than from the earlier judgement. *(Round 2 wrote "seven" here — the last
  residual of the miscount corrected in §8.5.1, and it survived in the one
  section whose job is to be trustworthy about method. Corrected in round 3.)*
- **A retained statement must be closed by absorption, not by identifier.**
  `c-headers` learned this the hard way when retaining `CHDR-007` while
  publishing `CHDR-008`, whose statement was a strict subset. Checked here: no
  finding in this note is a strict subset of another, because §8.1 merged the
  three pairs that were. Re-checked after the four round-2 removals, since
  removing a finding can leave a subset behind: it did not, and the two residues
  that were left standing (`DBND-701`'s row `:143` premise and `DBND-709`'s
  nonconforming-names clause) are recorded in §7 and deliberately **not**
  re-filed.
- **The tables are the exposure surface.** They were re-checked separately, per
  step 6, and that step exists in this note only because `c-headers.md` §15 wrote
  down where its own barrier failed.

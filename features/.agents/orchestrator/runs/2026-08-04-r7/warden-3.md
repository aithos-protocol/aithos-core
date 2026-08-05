# Warden — `d-bundle`, third review after remediation landed, run `2026-08-04-r7`

**VERDICT: `VALID`** — the delivery breach that produced invalidation 2 is
discharged (`HEAD` = `8068426` = `origin/codex/audit-d-bundle`), and all six
breaches from invalidation 1 are repaired in the repository; two prose errata
remain, neither procedural, both listed in § 8 and neither blocking the
transition.

The cycle may transition to `CORRECTION_REQUESTED`.

I ran no gate, no test and no `cargo` command. Everything below is a read of the
repository at `8068426`, of the journalled transcripts, or of the source at
`d9120d7`. Where I say "verified", I recomputed or re-derived the thing rather
than reading a claim about it.

Written as `warden-3.md`. `warden.md` is my invalidation-1 report, committed at
`f6bae5f`; `warden-2.md` was invalidation 2. None is overwritten.

---

## 1. Delivery — the reason for invalidation 2 — DISCHARGED

| Check | Result |
|---|---|
| `git rev-parse HEAD` | `8068426a8bb6c3f7c063cf82e8fba4876e9296a0` |
| `git ls-remote origin codex/audit-d-bundle` | `8068426a8bb6c3f7c063cf82e8fba4876e9296a0` |
| Identical | **yes** |

The commit is on the remote. I ran the `ls-remote` myself rather than take it on
report; it is a read, not a gate, and it is the one command that settles the
question. The owner's manual landing from the bundle is exactly the route I named
as the one that lets the run resume rather than restart, and it worked.

**The container reset a third time between reviews 2 and 3.** This working tree
is a fresh clone. Everything I am reviewing arrived through the repository. That
is the property invalidation 1 was about, and it is now demonstrated rather than
argued: three resets, and the corpus survived all three because it was committed
and pushed.

**One delivery item is still open and I want it named.** The three transcripts you
ran for this review are **not committed**:

```
 M features/.agents/orchestrator/runs/2026-08-04-r7/ledger.jsonl      (+3 lines)
?? features/.agents/orchestrator/runs/2026-08-04-r7/evidence/ev-1ad4fe50.txt
?? features/.agents/orchestrator/runs/2026-08-04-r7/evidence/ev-d8e82fe3.txt
```

They are correct — I verified all three below — but they are in the state that
has already cost this cycle two invalidations. **Commit and push them with the
transition.** I am not invalidating for it: they were produced for this review
and the transition commit is where they belong. I am naming it because the
pattern is now three-for-three.

---

## 2. The nine authorities — no drift, checked loudly as asked

You asked me to say so loudly if any of the nine had drifted from the digests in
ledger line 54. **None has.**

I recomputed all nine sha256 from the committed bytes and compared to the ledger:
`EVIDENCE.md`, `INVENTORY.md`, `VERDICTS.md`, `pass-a/RU-{1,2,3,5,6,7}.md` —
**nine of nine identical.** These files have now survived two container
destructions since they were committed.

Also re-verified at `8068426`:

- `sha256(pass-a/frozen.json)` = `d3f8f33324c48e3c12bfd425b19238b0ba80bc502a662a5255073e980c3a685b`
  — unchanged from the 14:43:40Z freeze. **The freeze has never been edited**,
  across three reviews and three tree states.
- **36 transcripts, 36 ledger identifiers, zero orphans in either direction,
  zero hash mismatches, every `evidence_id` its own hash prefix.**
- Every `ev-` token in `d-bundle.feature` (17) and in the audit (23) resolves to
  this ledger. `STATE.md` carries two that do not — `ev-a1fa00fc` and
  `ev-f818dc4b` — and both resolve to `runs/2026-08-04-r6/ledger.jsonl`, cited as
  carried-forward `c-headers` debts. Correct cross-cycle citation, not orphans.

---

## 3. The three gates

**`ev-1ad4fe50` — `train-status.py`, the *after* to pair with `ev-0169d294`.**
Its `ouverts :` line lists **35 identifiers: `DBND-001`…`DBND-039` minus `020`,
`026`, `030`, `035`.** Not one retired `DBND-1xx`…`DBND-7xx` identifier. Set
against `ev-0169d294`'s 25 retired identifiers including six removed findings,
the pair is a clean before/after of breach 3 in the ledger rather than in prose.
That is what I asked for and it is exactly what came back.

**`ev-d8e82fe3` — the feature gate at the corrected `HEAD`.** `1 feature / 7
rules / 51 scenarios (51 passed) / 299 steps (299 passed)`, **zero `✘` marks**,
compiled fresh in this tree. The 107 marker lines and the whole document rewrite
changed nothing behavioural.

**`ev-14592971` — `verify-feature-tags.sh`**, byte-identical again, as
content-addressing requires whenever it passes. I noted in review 1 that this
gate's one-line output carries almost no discriminating power; that is unchanged
and is not a criticism of this run.

**On `ev-5f523aae`:** agreed, and I would not have counted it. It was observed in
a container that no longer exists and its ledger line was in the commit that had
not landed. `ev-d8e82fe3` is the transcript I judged on.

---

## 4. The six breaches, re-checked at `8068426`

| # | Breach | State | Evidence |
|---|---|---|---|
| 1 | Authorities not in the repository | **CURED** | § 2; § 1 now has an *"authorities this note rests on, and where they are"* block that states round 1 asserted they were committed and they were not |
| 2 | Stale line citations; false worktree claim | **CURED** | § 5 — mechanically verified |
| 3 | `STATE.md` | **CURED** | § 6 — verified three ways |
| 4 | "Seven independent auditors" | **cured at four sites, one residual** | § 8(a) |
| 5 | 17 mutants vs 19 | **CURED** | corrections table; `ev-5474b889` named as the dropped one |
| 6 | `bundle.rs:1770` | **CURED** | `DBND-018` now cites `:1772`; `:1770` survives only inside the corrections table describing the fix |

**§ 1's worktree row is now true and specific**, naming the 99 + 8 = 107 inserted
lines, stating that no scenario, rule, step or `Examples` row changed, and saying
in its own words: *"Round 1 of this note claimed the audited bytes and the current
bytes were the same bytes; that was false for the feature file and is corrected
here."* A metadata table that records its own previous falsehood is the right
shape.

**A correction to my own review 2.** I reported §7 as having *nine* removal
blocks and flagged `DBND-503`'s as missing. **That was my regex, not the
document.** §7 has **ten**; `DBND-503`'s header reads
`### \`DBND-503\` + \`DBND-714\` + \`DBND-715\` — removed, round 2`, and the `+`
segments defeated a pattern that expected the em-dash immediately after the
identifier. §7's table also has ten rows. The document was right and I was wrong,
and it goes here rather than being quietly dropped.

---

## 5. §5.2 — the offset table, verified mechanically

This is the repair I did not propose and the one I now think is better than what
I asked for. I did not assess it on its argument; I re-derived it.

I extracted `d9120d7:features/d-bundle.feature`, parsed all **21 rows** of the
§5.2 table, and for each row checked two things: that the arithmetic holds
(`audited + offset == current`) **and** that every line of the entire declared
span is byte-identical between audited line *N* and current line *N+offset*.

```
audited: 165 lines  sha256 59a6f361598de459fa063e7bff9915427c5e3d70423c20204d26f294b618c8b5
current: 272 lines  sha256 9913e2aaf034dc5ba68b6eb9f13b74d0b536042515cc7953cbb6f0ae17033ceb
table rows parsed: 21
MISMATCHES: 0
```

**Both declared digests are exact. All 21 offsets hold. Every line of every
declared span matches.** The table is correct in full, not in sample.

**Is it regenerable, and does the document say how?** Effectively yes, and the
safety property is the one that matters. The note declares *both* sha256s and
states the invariant that makes the table derivable — insertions only above
scenario headers, constant offset within each block, nothing else changed. So:

- The **anchor** numbering is the frozen file, which can never change. That is
  strictly better than anchoring to `HEAD`, and it is the numbering `INVENTORY.md`,
  all six Pass A reports and `frozen.json` already use — so §5.2 makes the whole
  corpus consistent instead of making the audit consistent with itself only.
- Your worry — *"a declared numbering that drifts silently on the next marker
  edit would be worse than stale numbers, because it would look right"* — is the
  right worry, and it is answered: the note pins the **current** file's sha256.
  The moment a marker edit lands, `9913e2aa…` stops matching and the drift is
  **detectable by one command** instead of silent. That is the difference between
  a stale number and a lying one.

**What it does not do** is instruct anyone to run that check. The material is all
there; the trigger is not. I suggest — not require — one sentence under §5.2
saying that any edit to `d-bundle.feature` invalidates the `Current` column until
the digest is re-pinned and the table re-derived. That is an erratum-grade
improvement, not a condition.

A3 was right and both your options were worse. Re-anchoring to `HEAD` would have
gone stale a second time inside one session, by measurement — the round-2 marker
edit moved the offsets again, from 99 inserted lines to 107. Leaving them stale
would have misrouted the corrector. The third option is the only one that is
stable by construction.

---

## 6. `STATE.md` — verified three ways

`open_findings` now carries **35 identifiers**. I compared three independent
sources:

| Source | Count |
|---|---|
| `STATE.md` `open_findings` | 35 |
| `### \`DBND-…\` — \`OPEN\`` blocks in the audit | 35 |
| `ouverts :` in `ev-1ad4fe50` | 35 |

**All three sets are identical**, and no identifier ≥ `DBND-100` appears in any
of them. The retired numbering is gone from every routing surface.

The body is rewritten and the handling is good: the authoritative section is
placed **first**, and the bootstrapper's original text is kept below it under
*"Everything below it was written by the bootstrapper before the audit ran, and
is kept for the record, not for instruction. Where the two disagree, this section
wins."* That preserves the record without leaving a second, contradictory
description of the tree in force. `last_transition` is a real timestamp.

The new section states six auditors / seven units, nineteen mutants, 35 findings
at 2 P1 / 13 P2 / 20 P3, the empty panel-survivor category, and that a second
warden invalidation stops the run. All of that matches the audit.

---

## 7. The round-2 removals — including the one I could not certify

I verified all four decisive facts in the source in review 2 for three of them,
and re-verified nothing that had already held. Here is the fourth, which is the
one I said was the most likely place for a second error and which you flagged
again.

### `DBND-603` / `DBND-026` — the three legs, checked individually

All three cited sites confirmed in `rust/crates/aithos-bundle/src/lib.rs`:

- **`begin_transaction`** — `let staging = generations.join(&generation);`.
  Staging *is* under `.aithos-generations/`. The refuter's route-one premise is a
  true statement about the code.
- **`recover_transaction` (`:906`)** — `self.rollback_transaction()?;` then
  `Self::ensure_plain_directory(&self.root)?;` then `let active = self.read_pointer()?;`.
  Every one `?`-propagating `io::Result`. Route two is a real path.
- **`reconcile_compatibility_mirror` (`:686`)** — `Self::collect_from(&self.root, &self.root, &mut mirror_keys)?;`
  then reconciliation against `active`. **CORRECTED 2026-08-04, round 3: route
  three is NOT a real path, and this entry is where I got it wrong.** I verified
  that it calls `collect_from` and stopped there without reading `collect_from`.
  `collect_from` (`lib.rs:581`) skips, at `:602-609`, every top-level component
  whose name `starts_with(".aithos-")` — which is exactly where a leaked staging
  generation lives. The mirror reconciliation is structurally blind to it.
  Measured: `ev-f7ee3968`, green 51/51 under the M1+M3 pair, and the compiler
  reports `reconcile_compatibility_mirror` **never used** under that pair, M1
  having removed its only caller.

**A3's disagreement with its own refuter is correct.** `canonical_base()`'s first
branch is guarded by `if let Some(transaction) = &self.transaction` and fires
only while a transaction is active. Stating it as though it unconditionally puts
staging inside the comparison is wrong, and A3 caught it by checking a verdict
that went its way. That is the right instinct and it is the second time this role
has produced its best work by re-checking something nobody asked it to re-check.

**And the block's mutant claim is exactly right, which I doubted and then
checked.** The audit says *"the mutant's whole content is `self.transaction = None`"*.
I went to the frozen source. `RU-6.md` § M3, the proposed mutant for `DBND-603`:

```diff
     fn rollback_transaction(&mut self) -> io::Result<()> {
-        if let Some(transaction) = self.transaction.take() {
-            Self::remove_internal_path(&transaction.staging)?;
-        }
+        self.transaction = None;
         Ok(())
     }
```

Verbatim. My doubt was that this described the `DBND-601` crash-recovery mutant;
it does not. The audit is precise.

### One thing the block overstates, and it is in the frozen material

§7 says: *"**Routes two and three do**, and either alone is enough."*

**Route two alone is not enough, and the finding's own author said so first.**
`RU-6.md` § M3, immediately under the diff:

> **Prediction: `RU-6` stays green** … Note that the reopen inside the scenario
> would still sweep it via `recover_transaction`, so a sceptic can say recovery
> cleaned up. **M4 removes that defence: apply M1 and M3 together.** Under M1+M3
> the staging directory leaks permanently and nothing cleans it, and I predict
> `RU-6` is *still* green. **That pair is the closure test for `DBND-603`.**

The refuter's route two is the auditor's own conceded caveat, and the auditor had
already named the answer to it. So of the two routes offered as independently
sufficient, one is answerable by a mutant that sits in the frozen Pass A material
under the name **M4**. **CORRECTED 2026-08-04, round 3.** What I wrote here was
*"route three carries; route two does not carry alone"*. **Route three does not
carry either**, for the reason recorded in § 7 above: `collect_from` skips every
`.aithos-*` top-level component, so the mirror reconciliation cannot see the
leaked generation. Neither route carries alone and neither carries together.

**This does resurrect the finding, and I ruled that outcome in advance.** The
auditor's own closure test — `RU-6.md` § M1 and § M3 applied together — was run
as `ev-f7ee3968` and the feature gate is **green, 51/51, 299/299**. The removal
does **not** stand. `DBND-603` is restored under its reconciled identifier
`DBND-026`. The conjunction *"either alone is enough"* in the audit's §7 was
wrong, and so was my own correction of it: erratum § 8(b) understated the error
by half.

**A note on the epistemics, which the block states honestly and I want to
endorse rather than fix.** §7 says plainly: *"The prediction was never measured,
and this note does not now measure it: the mutants remain unrun and are not
re-proposed."* A finding whose content was an unrun prediction is being removed
on the strength of code reading. That is the weaker end of this train's evidence
hierarchy, and the audit says so instead of dressing it up. Given that M4 exists,
is named in the frozen material as *"the closure test for `DBND-603`"*, and would
settle it by transcript, I offer it as an optional command in § 9. I am **not**
making it a condition: the finding is removed, running M4 could only resurrect it,
and a warden should not manufacture work by requiring an experiment whose most
likely outcome is confirming what the code already shows.

### The identifier confusion — final state right, third check

`DBND-503`+`714`+`715` → was `DBND-020`; `DBND-603` → was `DBND-026`;
`DBND-701` → was `DBND-030`; `DBND-709` → was `DBND-035`. §7's table prints the
mapping in both directions on every row. `DBND-039` is untouched and `OPEN`.
Arithmetic: 39 − 4 = 35; all four were P2, so 17 − 4 = 13; **2 P1 + 13 P2 + 20
P3 = 35**, which I counted from the block headers rather than from the summary.

---

## 8. The two errata — required, not blocking

Neither is a procedural failure. Neither misstates evidence, cites a missing
transcript, or misroutes the corrector. Both are prose. **Fix them in the
transition commit.**

**(a) §15, *What this episode establishes*, first bullet — line 3158:**

> - **The barrier was exercised, not asserted.** **Seven independent assessments**, two
>   named candidates carried forward…

There were **six**. This is a residual instance of breach 4 in the one section
whose purpose is to be trustworthy about the audit's own method. It is the more
striking because §15's step 1, thirty lines above, now reads *"**all six**
auditors … across all seven units"* and adds the parenthetical *"Round 1 of this
note repeated 'seven' here and at step 5, twice in one table, and the warden
caught both."* The sweep was scoped to the two **table** rows in §15 and missed
the **prose** bullet in the same section. Everywhere else — §2, §6.1, §8.5's
corrections list, §15 step 1 — the count is correct and the discrepancy with
`frozen.json` is explained. One word.

**(b) §7, `DBND-603` block — *"either alone is enough"*.** Per § 7 above: route
two is answerable by M4, which the finding's own author named as its closure
test. **Superseded 2026-08-04, round 3**: neither route carries. The finding is
restored as `DBND-026` on `ev-f7ee3968`, and the audit's §7 entry now records the
removal, the three dead routes and the twenty unread lines of `collect_from` that
route three died on.

**Optional, not an erratum:** one sentence under §5.2 saying that any edit to
`d-bundle.feature` invalidates the `Current` column until the digest is re-pinned
(§ 5).

---

## 9. Commands

**Required with the transition:** none that I need run. Commit and push the three
uncommitted transcripts and the ledger lines (§ 1), and land the two errata.

**Offered, not required** — the one experiment that would move `DBND-603` from a
reading to a transcript, using the closure test its own auditor named and froze:

```
# apply RU-6.md § M4 = M1 + M3 to a clean tree at d9120d7, then:
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

If it comes back **red**, `DBND-603`'s removal is confirmed by measurement
instead of by reading and §7 gains a transcript. If it comes back **green**, the
finding was right and its removal must be reversed — which is precisely why the
audit's honesty about the removal resting on the record matters, and why I am
raising this as an option for the owner rather than deciding it.

---

## 10. What I could not check

1. **Whether `DBND-603` would in fact go red under M4.** Unrun by design; § 9.
2. **Byte-identity of the committed Pass A corpus against the out-of-tree
   originals.** Those were destroyed with `/root/work` after review 1. I
   substituted digest verification against ledger line 54 (§ 2) and full
   re-execution of my structural contamination checks in review 2, both of which
   reproduce exactly. That is corroboration plus a hash chain, which is stronger
   than what I had, but it is not identity to a pre-commit original.
3. **The twenty findings resting on the record alone.** Untested by design and
   labelled as such. They are correctly excluded from the corrector's lot.
4. **Anything requiring a command.** I ran no gate, no test, no `cargo`.

---

## 11. The transition

`VALID`. The cycle may transition to `CORRECTION_REQUESTED`.

**The corrector's lot is the fifteen findings confirmed by transcript.** I
reconciled that number rather than accepting it: a naive text count returns 17,
because `DBND-003` and `DBND-023` carry **per-limb** evidential states — in both,
the *statement* rests on the record alone and only a limb or a consequence is
confirmed. Moving those two to the record-alone column gives **15 confirmed / 20
on the record alone = 35**, which is what the audit and `STATE.md` both say. The
split is correct and the two per-limb blocks are the reason it looks otherwise.

The twenty resting on the record alone are **not assignable until they carry
evidence**, and I endorse that constraint explicitly: it is the rule that keeps
this audit from handing a corrector work whose premise has never been measured.

---

## 12. Notes on substance — these bear on nothing

1. **The disclosure gate re-checked at `8068426`.** 17 tables, 234 table lines,
   read separately from the prose; all 13 marker blocks; `BLOCKED.md` has **zero**
   `d-bundle` entries. Nothing embargoed, nothing withheld, and nothing that
   needed to be: the four new §7 blocks publish statements the panel **falsified**,
   which by construction cannot describe a live weakness. §15 step 6's table list
   is still accurate against the enlarged document.
2. **The append-only discipline held again.** Ten removals, and not one finding
   was downgraded or quietly rewritten — §7's opening says so and the document
   does it. `frozen.json` still untouched at the same hash after three reviews.
3. **§8.1 pricing its own merges** — *"a merged finding dies as one; the panel
   attacked the strongest limb and the two weaker limbs went down unexamined with
   it"* — is a real cost of reconciliation that nobody asked to have priced, and
   it is the kind of thing that only shows up when a merged finding is later
   killed. `DBND-503`+`714`+`715` is exactly that case. Worth carrying into the
   next cycle's merge policy.
4. **On my own role across the three reviews.** Invalidation 1 was on substance
   and produced six real repairs and, indirectly, four correct refutations that
   would otherwise have reached the corrector as live findings. Invalidation 2 was
   on delivery and produced a pushed commit that then survived a third container
   reset. Both remedies were cheap and neither discarded work. I record that the
   orchestrator did not overrule either, and that each of the three times a role
   it briefed corrected the brief — A3 on the line-citation option, A3 on the two
   wrong identifiers, A3 on its own refuter's lead fact — the correction was
   accepted and published rather than absorbed.
5. **The durability point from review 2 stands as a process amendment for the
   owner**, and is now better evidenced: three container resets in one cycle, and
   the only material that survived all three is what reached `origin`.
   `QUEUE.yaml` declares `durability_remote: origin`; nothing verifies it. A cycle
   that cannot reach `origin` should block **before** producing work it cannot
   deliver. I raise it; I do not decide it.

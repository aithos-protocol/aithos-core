# Warden — `d-bundle`, addendum 2: ruling on `ev-dee6dbf2`, `ev-1be2a7f4`, `ev-ef839413`

**Verdict remains `VALID`.** Three rulings, and one of them goes against you:

1. **`DBND-505` stays removed** — `ev-ef839413` is a properly built counterfactual
   and it lands. Confirmed.
2. **`DBND-504` is NOT restored. Its disposition is HELD** pending one further
   mutant. `ev-dee6dbf2` is vacuous **for the same reason you correctly refused to
   bank `ev-1be2a7f4`**, and I specified it badly.
3. **The un-retirement rule is *not* the same for `DBND-504` as for `DBND-026`.**
   Your question assumed it is. It is not, and the difference is in §6.0.

Counts therefore stand at **36**, not 37, until the held mutant runs.

I ran no gate.

---

## 1. Transcripts — all four verified

| id | sha256 matches id + ledger | Result | Ledger `summary` |
|---|---|---|---|
| `ev-dee6dbf2` | yes | GREEN 51/51, 299/299 | structured, matches |
| `ev-1be2a7f4` | yes | GREEN 51/51, 299/299 | structured, matches |
| `ev-ef839413` | yes | **RED, 48 passed / 3 failed**, 296 steps | structured, matches |
| `ev-1d4725c7` | yes | `exit 101`, `E0425`/`E0308`, **no `[Summary]`** | `null`, `green: false` |

**`ev-ef839413`'s three failures are all the same step**, and it is the right one:

```
✘  And every mutation is journalized without consuming mandate counters
   Defined: features/d-bundle.feature:132:7
   Matched: crates/aithos-bundle/tests/cucumber.rs:11528:1
   Step panicked. Captured output: assertion `left == right` failed
```

Three failures, three `list` rows (public, circle, self) — exactly the rows where
the hardcoded `matches!(operation, create|edit|delete)` says *not journalized*
while the drifted production and drifted vector both say *journalized*. The
arithmetic is right and the mechanism is right.

`:132` also checks out against §5.2: the RU-5 outline block carries offset **+64**,
and audited `:68` is *"And every mutation is journalized without consuming mandate
counters"*. `68 + 64 = 132`. The offset table survives contact with a transcript
produced after it was written.

**`ev-1d4725c7` — naming the failed compile rather than letting a gap in the ids
look like a hidden run is the correct handling**, and I want it recorded as such.
A ledger with `green: false`, `summary: null` and a real `rustc` error in the
transcript is worth more than a tidy sequence.

---

## 2. `DBND-505` — **stays removed. Confirmed by measurement.**

You built the counterfactual the refutation actually named, in both halves —
production drifted so `list` journalizes, vector drifted to agree — so that the
helper's vector comparison passes and only the `Then`'s hardcoded predicate can
object. It objects, three times, on the step the finding called redundant.

That is a real test of a real counterfactual, and it lands against the finding.
`DBND-505`'s refutation is now the only one in this cycle **confirmed by
transcript rather than by citation**, which makes it the strongest of the eight.

**And you were right to refuse `ev-1be2a7f4`.** I specified that mutant and I
specified it wrong. Deleting an assertion from otherwise-correct code is green by
construction; it cannot test a claim about what the assertion would catch. You
could have banked it as a second restoration and reported a stronger result than
you had. Refusing a green that flattered your own earlier position, and saying so
in those words, is the single best moment in this exchange.

---

## 3. `DBND-504` — **HELD, not restored.** `ev-dee6dbf2` is vacuous.

You flagged that W1 is "the weaker of two possible forms" and then argued past
your own flag. I am holding you to the standard you applied one paragraph
earlier, because **`ev-dee6dbf2` fails by exactly the argument that killed
`ev-1be2a7f4`.**

`check_form` gutted to `Ok(())`, gate green. But if no entry in any of the 51
scenarios is malformed, then `check_form` returns `Ok(())` on every call in the
baseline anyway — so replacing it with `Ok(())` is a **no-op by construction**,
and green is guaranteed *whether or not* the feature could catch a malformed
entry. Removing a guard that has nothing to guard against measures nothing. That
is the same sentence you wrote about deleting an assertion from correct code.

The refutation's claim is a **counterfactual**: that a malformed entry would not
survive to be counted. `ev-dee6dbf2` never presents the checker with anything to
catch, so it does not reach that claim.

**And the finding's frozen statement names the decisive experiment itself.**
§7's `DBND-504` block quotes it:

> An owner `edit` appending an entry with a **`create`'s `kind`**, **or** a
> `target` naming a different node, keeps the count delta at 1 and all nine
> mutating rows green.

Two worked examples, offered disjunctively. **The refutation answered only the
second** — *"the finding's own worked example — a circle entry with a `target`
naming a different node — is rejected at write time in `gamma_append`."* It says
nothing about the `kind` variant, and `gamma_append`'s rejection of a bad
`target` does not imply rejection of a bad `kind`.

### W1b — the mutant I should have named the first time

```
# in the owner edit path, append the Gamma entry carrying a `create`'s `kind`
# instead of an `edit`'s — the finding's own first worked example, unaltered —
# then:
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

- **RED** → the malformation is caught before or at the count, the refutation's
  fact defends the sentence, **`DBND-504` stays removed** and its refutation is
  upgraded to *confirmed by transcript*.
- **GREEN** → the count delta stays 1 and the nine mutating rows stay green, the
  finding's first worked example holds verbatim, and **`DBND-504` is restored.**

Either way we end with a transcript instead of a citation, which is the whole
point of the rule going into `PROCESS.md`.

### If it is restored, the statement must be narrowed, not reinstated

This matters and nobody has raised it. `DBND-504`'s frozen statement contains a
sub-claim the refuter **correctly falsified**: *"`Entry` carries `kind`, `target`,
`authorized_by`, `authorized_via`, `payload`, `body_enc`, `signature` — **none is
read**."* That is false. `check_form` reads four of the seven; I verified the
chain myself in review 2 and `ev-dee6dbf2` does not disturb it.

So a restored `DBND-504` is **narrower than the frozen one**: its centre —
*journalized is proved by cardinality alone, as far as this feature's verdict is
concerned* — would survive, while *"none is read"* is dead and must be struck
with the refuter credited for killing it. This is the mirror image of `DBND-026`,
where the routes were wrong and the finding stood whole. Here a sub-claim was
wrong and only the centre would stand.

Also keep §7's boundary note, which is correct and load-bearing: `check_form`
reads `kind`, `target`, `payload`, `body_enc` and **not** `authorized_by` or
`authorized_via`, which is why `DBND-018` is untouched by any of this and remains
P1 and confirmed.

---

## 4. Q1 — counts, and the un-retirement rule, which differs

**Counts as they stand now, pending W1b:**

| | Value |
|---|---|
| Findings | **36** — 2 P1, 14 P2, 20 P3 |
| Panel | 10 tested, **8 refuted**, **1 restored by measurement**, **1 held pending `W1b`** |
| Corrector's lot | **16** |

If W1b is green: 37 / 2 P1 / 15 P2 / 20 P3, lot 17. If red: unchanged at 36, and
`DBND-505` and `DBND-504` are both refutations confirmed by transcript.

**The un-retirement rule is not the same for the two, and your assumption is
wrong.** §6.0's map settles it:

```
| `DBND-603` | RU-6 | P2 | ~~`DBND-026`~~ | removed by the panel, round 2. Identifier retired |
| `DBND-504` | RU-5 | P2 | —              | removed by the panel (§7)                        |
```

- **`DBND-603` had a published identifier**, `DBND-026`, struck through. Restoring
  it is **un-retirement of an existing number** — the map row's strike-through is
  lifted, nothing else moves, and the 48-row map stays intact. That is my earlier
  ruling and it stands.
- **`DBND-504` never received one.** Its "This note" cell is `—`. It was killed in
  round 1, before renumbering, and never entered the `DBND-0xx` series. Restoring
  it is not un-retirement; it requires **assigning a new identifier**, and §6.0
  forbids reusing `DBND-1xx`…`DBND-7xx`.

**So if W1b restores it, it enters as `DBND-040`** — appended, not slotted. §6.0
is ordered by unit then severity, and `DBND-504` is RU-5, so aesthetically it
belongs mid-series; **do not put it there.** Slotting it would renumber
everything after it, and those numbers are already published in `STATE.md`, in
the Gherkin markers, in `ev-1ad4fe50` and in this cycle's transcripts. **Stability
of published identifiers beats ordering.** Add one sentence to §6.0 saying the
series is unit-ordered through `DBND-039` and that `DBND-040` is appended out of
order because it was restored after publication — so the next reader does not
"fix" it.

The same rule applies to any of the five remaining round-1 removals if one is
ever restored: new number at the end, never a slot, never a reuse.

---

## 5. Q2 — the criterion. It is a filter, and it must not promise an outcome

You asked for my reading in my own words, for `PROCESS.md`. Here it is.

**The rule:** *a refutation whose decisive fact is a claim about what code does at
runtime must carry a transcript, not a citation. A refutation whose decisive fact
is a claim about what exists in the tree may carry a citation.*

**What it is:** a rule about **which claims must be measured**. It selects the
subset of refutations whose evidence is of a kind that can be wrong in a way
reading does not reveal.

**What it is not:** a prediction about what the measurement will find. This round
proves that, and the proof is the best thing to put next to the rule in
`PROCESS.md`: two refutations qualified, one fell and one was confirmed. A rule
that had promised outcomes would have been falsified by `DBND-505`; this one was
not, because it never claimed to know.

**Why that makes it stronger, not weaker.** A filter that only ever fires on
wrong refutations would be indistinguishable from a bias against refutations. The
`DBND-505` outcome is a *success* of the rule in the same sense as `DBND-026`:
one removal was reversed, one was upgraded from an argument to a measurement, and
the corrector's lot is now right in both directions rather than merely smaller.
Say that explicitly when you write it, because a reader who sees only the
`DBND-026` story will read the rule as "panels are unreliable," which is not what
it says.

### And a second rule this round earned, which is the sharper one

The rule above tells you *what* to measure. It does not tell you *how*, and both
mutants I named this round got the how wrong:

> **A mutant that removes a check must be paired with an input the check would
> have rejected. Otherwise green is guaranteed by construction and measures
> nothing.**

`ev-1be2a7f4` deleted an assertion with no regression present. `ev-dee6dbf2`
deleted a guard with nothing to guard against. Both green, both worthless, and
they failed identically. You caught the first; I caught the second; **I specified
both.**

The repository already knew this rule and stated it in a different vocabulary —
§4.2: *"Two mutants were designed by their authors expecting to be caught, and
were. An audit that only runs mutants it expects to survive is measuring its own
confidence."* And `RU-6.md`'s M3/M4 pairing is the same instinct applied
correctly by a blind auditor: it named M4 precisely because M3 alone would have
left a defence standing. **The auditors had this discipline and the panel round
lost it.** That is worth a sentence in `PROCESS.md` next to the first rule.

---

## 6. Q3 — does either result touch `VALID`? **No.**

Same reasoning as addendum 1, and one addition.

Nothing procedural failed. Both results were produced by the evidence hierarchy
working as designed, before the transition, on experiments the train volunteered.
The document's count is wrong — by one now, possibly two after W1b — and it is
already going into the transition commit with errata. These join them.

**The addition, and it is against me.** Two of the three mutants I named this
round were badly specified, and one of them would have produced a **false
restoration** if you had banked it. That is a warden error of the same class as my
route-three ruling in `warden-3.md`: reasoning from a mechanism's presence rather
than from what it does under the conditions that matter. Three times now the
answer has come from a transcript rather than from a reading, and twice the
reading was mine.

That does not touch the verdict — a warden's errors are corrected by the same
instrument as anyone's, and they were corrected before anything shipped — but it
belongs in the record next to the two rules in § 5, because the rules exist
precisely because careful readers get this wrong.

---

## 7. What lands with the transition

1. **`DBND-026` restored** as ruled in addendum 1 — under its own number,
   limb-split evidential state, history preserved in §6 and §7.
2. **`DBND-505` stays removed**, and its §7 block is upgraded to cite
   `ev-ef839413` — it moves from *refuted on a citation* to *refuted on a
   transcript*, and it is the only one of the eight that can say so.
3. **`DBND-504` held.** Do not write it as restored and do not write it as
   removed-and-settled. §7's block should say the disposition is open pending
   `W1b`, with the mutant named. **Run W1b before the transition commit**, then
   write whichever result comes back.
4. **Counts: 36 now; 37 only if W1b is green**, with `DBND-040` and the
   ordering note per § 4.
5. **`ev-1be2a7f4` and `ev-1d4725c7` stay in the ledger** as journalled, with
   `ev-1be2a7f4` explicitly marked as not counted and why. A run that proved
   nothing, and a run that did not compile, both named — that is the record doing
   its job.
6. Errata (a) and (b), the `warden-3.md` correction from addendum 1, and **commit
   and push all of it**: seven transcripts and four warden documents are currently
   uncommitted in a tree that has been destroyed three times.

**One further mutant requested: `W1b`, § 3.** Nothing else outstanding from me.

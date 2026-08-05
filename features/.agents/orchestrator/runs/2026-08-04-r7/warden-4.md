# Warden — `d-bundle`, addendum: ruling on `ev-f7ee3968`

**The verdict remains `VALID`.** `ev-f7ee3968` is sound, `DBND-026` is restored,
and **my review-3 ruling that route three carries was wrong**. This addendum
rules on the four questions, corrects my own committed report, and names two
further mutants.

I ran no gate. `ev-f7ee3968` was run by the orchestrator on a mutant I named.

---

## 0. First: I was wrong, and here is exactly where

In `warden-3.md` § 7 I wrote: *"Route three carries; route two does not carry
alone."* **Route three does not carry either.** The transcript says so and the
code says why.

I verified that `reconcile_compatibility_mirror` calls
`Self::collect_from(&self.root, &self.root, &mut mirror_keys)?` and concluded it
sees everything under the store root. **I did not read `collect_from`.** Twenty
lines in:

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

`collect_from` **skips every top-level component beginning with `.aithos-`.**
The leaked staging generation lives at `generations.join(&generation)` —
`.aithos-generations/generation-…` (`generations_dir()`, `lib.rs:420`). So
`reconcile_compatibility_mirror` is **structurally blind to the orphan**. It
cannot copy into the canonical view a thing it cannot enumerate.

I confirmed the call and not the filter. That is the exact error class this audit
exists to catch — establishing that a function is reached without establishing
what it does with what it receives — and I made it while adjudicating a finding
about it. It goes at the top of this document rather than in a footnote.

`warden-3.md` is uncommitted. **Correct that sentence before committing it**, or
commit it with a correction note attached; do not let it enter the repository
uncorrected. A tracked warden report carrying a claim that does not match the
code is the thing I invalidated others for twice.

---

## 1. The transcript — verified

| Check | Result |
|---|---|
| `sha256(ev-f7ee3968.txt)` | `f7ee396854345f684c7616f0929b1c11995a631e3efa417c082ca07aca575c51` |
| Ledger `sha256` | identical; `evidence_id` is its own hash prefix |
| Summary | `1 feature / 7 rules / 51 scenarios (51 passed) / 299 steps (299 passed)` |
| `✘` marks | **0** |
| Ledger `summary` | structured counters present, matching the transcript |

**And — the check that decides whether green means anything — the mutant is not
inert.** A green under a mutant that changes nothing on the exercised path proves
nothing, so I checked three things in the source before accepting it:

1. **Rollback fires on this path.** `bundle.rs:433` —
   `self.store.rollback_transaction().map_err(io_err)?;` is on the failure path of
   `owner_content_operation`, the function the RU-6 fault outline drives. M3
   therefore executes with `Some(transaction)` and leaves the staging directory on
   disk. Under M1 nothing later sweeps it. **The orphan is real and permanent.**
2. **The assertion cannot see it.** `cb7_store_snapshot` (`cucumber.rs`) is
   `store.list("")` → `FsStore::list` → `collect_from` → skips `.aithos-*`. The
   snapshot's range excludes precisely the directory where orphans live.
3. **Neither can route three**, per § 0.

So `Then staging remains non-canonical and is cleaned or recoverably resolved
with no local-mutation orphan` **passes while a permanently leaked staging
generation sits on disk with nothing to clean it.** That is not an artefact of a
weak mutant; it is the finding, measured.

The auditor who wrote `RU-6.md` § M3/M4 predicted this outcome, staked the
finding on it, and named the pair as the closure test, blind, before any
transcript existed. It was right.

---

## 2. Ruling on your four questions

### Q1 — the counts. **Confirmed, with one addition.**

- **35 → 36 findings.** `DBND-026` was P2, so **2 P1 / 14 P2 / 20 P3 = 36**.
- **Panel: 10 tested, 9 refuted, 1 restored by measurement.** Confirmed.
- **The addition, and rule on it explicitly because this cycle has already had
  one identifier accident:** it is restored **as `DBND-026`**, not as a new
  number. §6.0 says retired identifiers "will not be reused" — this is not reuse,
  it is **un-retirement of the same finding**, and giving it a fresh number would
  break the 48-row map and orphan `DBND-603` in the Pass A corpus. The published
  series stays `DBND-001`…`DBND-039` with `020`, `030`, `035` removed and `026`
  live. §7's `DBND-603` block moves out of §7 and back into §6; §6.0's row and
  §7's table row both need their disposition flipped, and the *history* stays
  visible in both.
- **The corrector's lot moves 15 → 16.** Agreed, and that is the number I
  validate.

### Q2 — evidential state. **`confirmed by transcript`, with a limb split.**

Correct, and I would go further than you on why: **it is now among the
best-evidenced findings in the note**, not a repaired one. It was predicted
blind, staked on a named mutant by its own author, refuted on judgement, and
restored by the exact experiment its author had specified. Very little in this
repository has been through that.

**But split the limbs**, as the note already does for `DBND-003` and `DBND-023`:

- *"the snapshot cannot see a local-mutation orphan"* — **confirmed by
  transcript**, `ev-f7ee3968`, and the mechanism is nameable: `collect_from`
  skips `.aithos-*`, so the orphan is outside the assertion's range by
  construction.
- *"and a snapshot that can see one exists in the same file"* — **on the record
  alone.** No transcript touches it; it is a reading of an alternative helper.

Your framing — that the history is the most useful thing about it — is right,
and the block should carry the full route: **found blind → predicted with its
mutant → refuted on three judgement routes → route one rejected by A3 → routes
two and three killed by `ev-f7ee3968` → restored.** Including that the warden
ruled route three carried and was wrong.

### Q3 — does this touch `VALID`? **No, and here is the reasoning rather than the assertion.**

It does not, for three reasons:

1. **Nothing procedural failed.** No invariant was breached, no agent ran a gate,
   no claim cites a transcript that does not exist, no artefact misstates its
   evidence. The finding count in the document is wrong by one — and I validated
   that document *with errata to land in the transition commit*. This is a third
   erratum of the same class, caught before the transition rather than after.
2. **The mechanism that caught it is the process working.** A warden named a
   mutant, the orchestrator ran it, and a transcript overturned a judgement three
   roles had accepted — a refuter, an orchestrator, and me. That is the evidence
   hierarchy doing exactly what it is for: measurement beating reading, including
   when the reading is the warden's.
3. **A verdict that flipped here would be the wrong incentive.** You ran an
   experiment that could only cost you — it could confirm a removal or reverse
   one, and reversing one adds work. Invalidating for the result of an experiment
   the train volunteered would teach the train not to volunteer.

**What does change:** the document I validated needs three errata now, not two,
and one of them is a restored finding rather than a word. **The transition is
still authorised**, conditional on the errata landing in the transition commit —
which was already the condition.

### Q4 — does the doubt reach the other nine? **Two of them, and here is the criterion.**

The criterion that separates them, and it is the lesson of `DBND-026`:

> **A refutation is mutant-touchable when its decisive fact is a claim about what
> code *does at runtime*. It is not touchable when its decisive fact is a claim
> about what *exists in the tree*.**

`DBND-026` fell on the wrong side of that line and nobody noticed, because
*"`reconcile_compatibility_mirror` collects every key under the root"* **sounds**
like an existence claim and is actually a runtime claim with a filter in it.

**Not touchable — six.** Their decisive facts are things a mutant cannot change,
and I verified four of them myself:

| Finding | Decisive fact | Why a mutant cannot reach it |
|---|---|---|
| `DBND-302` | five read sites for `indices/public.json`, zero writers | exhaustive grep over the tree; and an uncalled function has nothing to mutate |
| `DBND-705` | `session.rs:354` takes `[u8;32]`/`[u8;24]`; `cucumber.rs:3082` submits `[0x76;32]`/`[0x77;24]` | a signature and two literals |
| `DBND-708` | row `:160` exists and installs its symlink at the intermediate component | a row in the frozen feature file |
| `DBND-020` | `cb2_bundle_authority_flows.rs:303-308` asserts `&OwnerKeys` present and `open_bundle_session(` absent | mutating `bundle.rs` to add the surface turns that test **red**, which confirms the refutation rather than testing it |
| `DBND-035` | `vectors/cb2-bundle-boundaries.json:363` declares the six as application points of one check | a normative vector in the tree |
| `DBND-030` | see below — I suspected this one and the code answered against me |

**`DBND-030` — I checked it hard and it holds.** My suspicion was that the
refutation's fact (`operation_succeeded` at `cucumber.rs:3011`) belonged to a
*different* assertion than the `Then "<observable_result>"` step the finding
attacks — the same "adjacent leg" shape that just bit us. It does not.
`d_capability_result` (`cucumber.rs:8450-8462`) is that step, and its body is:

```rust
assert_eq!(observation.observable_result, observable);
assert!(observation.operation_succeeded);
```

Both assertions are **inside the step the finding attacks**, and on row `:143`
`operation_succeeded` is `verify_owner_entry(&entry, &did).is_ok()`. The
refutation is correctly scoped and the finding's stated centre is false on that
row. **No mutant needed.** I record that I raised the suspicion and the code
resolved it against me, because a warden that only reports the suspicions that
pan out is not reporting.

**Touchable — two, and I want both run.**

**Mutant W1 — `DBND-504`.** This is the closest structural twin to `DBND-026`.
The refutation's fact is *"`verify_links` calls `check_form`, and `check_form`
reads `kind`, `target`, `payload`, `body_enc`."* True — I verified the chain in
review 2. But *"the fields are read"* does not establish *"this feature's gate
goes red if they are wrong,"* and that gap is exactly where `DBND-026` died.

```
# gut the field checks in Entry::check_form (aithos-core/src/gamma.rs:170),
# leaving it Ok(()), then:
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

**Red** → the reading is load-bearing, refutation confirmed by measurement.
**Green** → `check_form` reads the fields and this feature cannot tell, the
refutation's fact does not defend the sentence, and `DBND-504` is restored the
same way `DBND-026` was.

**Mutant W2 — `DBND-505`.** Its refutation ends in an explicit counterfactual:
*"deleting the `Then` would turn a joint vector-and-production regression from
red to green — the opposite of `changes nothing`."* That is a runtime prediction
and it is directly testable.

```
# delete the Then step at cucumber.rs:11536 (the hardcoded
# matches!(operation, create|edit|delete) comparison), then the same gate
```

**Red** → the `Then` is load-bearing, refutation confirmed. **Green** → the
finding's *"deleting it changes nothing"* was right and the refutation falls.

**Optional, tier three — `DBND-710`.** Its decisive fact is also a counterfactual
(*"if the baseline were taken before the fixture, `before == after` would fail
unconditionally on 6 of 10 examples"*), testable by moving the baseline to where
the finding demanded. I do not request it: unlike W1 and W2 it tests the
finding's own proposed remedy rather than the refutation's premise, and a red
there confirms what two readings already agree on. Run it only if you want the
set closed.

**I am asking for two gates, not three.** You offered three; `DBND-030` does not
need one and I would rather say so than spend a gate to look thorough.

---

## 3. What lands with the transition

1. **Restore `DBND-026`** per Q1 and Q2: back into §6 with a limb-split
   evidential state, out of §7 with its history preserved in both places, §6.0's
   disposition flipped, counts to **36 / 2 P1 / 14 P2 / 20 P3**, corrector's lot
   to **16**, `STATE.md` `open_findings` to 36, and a fresh `train-status.py`
   transcript so the routing surface is journalled at its final value.
2. **Erratum (a)** — §15, *"Seven independent assessments"* → six.
3. **Erratum (b)** — §7's *"either alone is enough"*. As you say, moot in the
   sharpest possible way: neither route carries alone and now the pair does not
   either. It should not be silently deleted with the block — §7's `DBND-603`
   entry becomes the record of a refutation that failed, and that is worth more
   than a removed row.
4. **Correct `warden-3.md` § 7** before it is committed, per § 0.
5. **Commit and push everything**, including `ev-1ad4fe50`, `ev-d8e82fe3`,
   `ev-f7ee3968`, the ledger lines, and this addendum. Four transcripts and two
   warden reports are currently uncommitted in a tree that has been destroyed
   three times.
6. **W1 and W2**, and whatever they change.

If W1 or W2 comes back green, the counts move again and I will rule on them the
same way. That is not instability; it is what happens when a train tests its own
conclusions, and it costs two gates.

---

## 4. The thing worth carrying out of this

The panel was introduced to filter refutations that **sound** right. `DBND-026`
shows the panel can produce one: three routes, each a true statement about the
code, none of which carried, accepted by a refuter, an orchestrator and a warden
in series. What caught it was not a fourth reader. It was a mutant that the
finding's own blind author had specified before any of the three ever saw it.

The operational lesson is narrower than "panels are unreliable": **a refutation
whose decisive fact is a runtime claim should carry a transcript, not a
citation.** Six of the ten removals here rest on existence claims and are safe.
Two rest on runtime claims and are getting transcripts. `DBND-026` rested on
three runtime claims and got none, and it was the one that fell.

That is a rule the next cycle can apply mechanically, and I would rather it went
into `PROCESS.md` than into this report.

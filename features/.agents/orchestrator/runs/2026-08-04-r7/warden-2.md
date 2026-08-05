# Warden — `d-bundle`, re-review after remediation, run `2026-08-04-r7`

**VERDICT: `INVALIDATED`** — invalidation 2 of 2, on **delivery, not on
substance**: commit `8068426` is not in the repository, so the two repairs that
govern what the corrector reads — the rewritten audit and `STATE.md` — exist
only in a session, and between my two reviews I watched a session not survive.

**Read this verdict with its reason attached, because the reason is unusual.**
Every component of the remediation I could reach is **correct**, and I verified
it byte for byte rather than accepting it. The four new refutations are **sound
on facts I checked myself in the source**. Nothing about this cycle's substance
needs redoing, and I am not asking for a single agent to be re-run. What I
cannot do is authorise a transition to correction against a document that, at
the only revision that exists outside a container, is byte-identical to the one
I invalidated.

The remedy is a `git push`, not a re-audit. § 7 says what I think the owner
should do with this.

I ran no gate, no test and no `cargo` command. I read git, I read files, and I
fetched — `git fetch origin codex/audit-d-bundle`, a read operation, and the
only reason this re-review was possible at all.

---

## 1. What happened to my material between the two reviews

I have to lead with this, because it is evidence, not a preamble.

When I wrote the first report, `/root/work/aithos-core` was on branch
`codex/audit-d-bundle` at `b19c7fd`, and `/root/work/` held
`passA-d-bundle/RU-{1..7}`, `passB-d-bundle/`, `refute-d-bundle/`,
`i1-d-bundle/`, `mut7/` and `d-bundle-base.tar`.

**All of it is gone.** At the start of this re-review:

| | State now |
|---|---|
| `/root/work/aithos-core` branch | `codex/audit-c-headers-r2` at `9b8d63d` |
| `git cat-file -t d9120d7` | `fatal: Not a valid object name` |
| `git cat-file -t b19c7fd` | `fatal: Not a valid object name` |
| `features/.agents/orchestrator/runs/2026-08-04-r7/` | **does not exist on disk** |
| `docs/audits/features/d-bundle.md` | **absent** |
| `features/d-bundle.feature` | present, **165 lines, zero markers** — the pre-audit file |
| `/root/work/passA-d-bundle`, `passB-d-bundle`, `refute-d-bundle`, `mut7` | **absent** |
| `git reflog` | `clone: from …aithos-core.git` → `a2087f2`, then two c-headers commits |

The checkout was replaced by a fresh clone at `a2087f2` — **the same revision
named in ledger line 11**, where the orchestrator recorded
`disclosure-barrier-durability` after *"l'effacement du clone local qui a ramené
l'arbre à `a2087f2`"* destroyed two of three embargoed statements. It has now
happened a second time and it took everything not on the remote, including my
own report as I wrote it.

**The nine files committed by repair 1 are the only reason any of the Pass A
corpus still exists.** In my first report I wrote that my no-drift finding
*"rests on scratch-directory mtimes and cannot be repeated by anyone else, ever,
once `/root/work` is gone."* `/root/work` was gone about four hours later. The
commit beat the erasure with hours to spare, and that is the single strongest
argument for the repair I invalidated to obtain.

It is also the whole of my verdict. `8068426` is in exactly the position
`/root/work/passA-d-bundle` was in when I wrote that sentence.

**On the orchestrator's environment note.** It told me *"everything up to
`f6bae5f` is on the remote"*. I checked rather than believed it —
`git ls-remote --heads origin` returns
`f6bae5f4d7b4af76caac717d9b2ee4fc264facf8  refs/heads/codex/audit-d-bundle` and
`d9120d7e0d154cee517b983bf7b6cac0cf8e8096  refs/heads/main`. **The claim is true
and exactly true.** `8068426` is on no ref. Disclosing the push failure
unprompted, and framing it as *"I am currently in exactly the state you
invalidated me for, one level up"*, is the correct handling and I record it as
such. It does not change what I can verify.

---

## 2. The six breaches, re-checked against what is actually in the repository

`f6bae5f` is one commit past `b19c7fd`. It changes **twelve files, 6364
insertions, zero deletions** and touches neither the audit, nor `STATE.md`, nor
the feature file — `git diff --stat b19c7fd f6bae5f` on those three paths is
empty.

| # | Breach | Repository at `f6bae5f` | Verified how |
|---|---|---|---|
| 1 | Cited authorities not in the repository | **CURED** | § 3 |
| 2 | Stale line citations; false worktree claim | **NOT cured** — audit byte-identical | `git diff` |
| 3 | `STATE.md` describes a feature that has not started | **evidence cured, file NOT cured** | § 4 |
| 4 | "Seven independent auditors" | **cured in the ledger**, not in the audit text | § 5 |
| 5 | 17 mutants vs 19 | **cured in the ledger**, not in the audit text | § 5 |
| 6 | `bundle.rs:1770` → `:1772` | **cured in the ledger**, not in the audit text | § 5 |

The breach my first verdict *rested on* is fixed, in the repository, and I
checked every digit of it. Three more are fixed in the append-only ledger, which
is the record of truth. Two — and they are the two the corrector reads — are
fixed only in `8068426`.

---

## 3. Repair 1 — fully verified, and it is the important one

**Nine files committed** under `runs/2026-08-04-r7/`: `pass-a/RU-{1,2,3,5,6,7}.md`,
`VERDICTS.md`, `EVIDENCE.md`, `INVENTORY.md`, plus `warden.md` and
`evidence/ev-0169d294.txt`.

**Digests.** Ledger line 54 records a sha256 for each of the nine. I recomputed
all nine from the committed bytes: **nine of nine match, character for
character.** No file was committed under a digest that describes a different
file.

**The freeze was not touched.** `sha256(pass-a/frozen.json)` =
`d3f8f33324c48e3c12bfd425b19238b0ba80bc502a662a5255073e980c3a685b`, byte-equal
to the hash journalled at 14:43:40Z on 2026-08-04 and to the hash cited in § 1
of the audit. **Correctly refused to edit a hashed-and-cited artefact**, and
superseded it by an appended ledger entry instead. That is the right call and
the opposite call would have been a far worse breach than the one it fixed.

**Transcript integrity.** 34 transcripts (33 + `ev-0169d294`); every sha256
matches its ledger line and every `evidence_id` is its own hash prefix. No
regression.

**The committed corpus is the corpus I reviewed.** I cannot byte-compare against
the out-of-tree originals — they no longer exist — so I re-ran, on the committed
files, every contamination check from my first report:

- `ev-` tokens across all six reports: **0, 0, 0, 0, 0, 0.**
- Finding-identifier partitioning: RU-1 → `1xx` only; RU-2 → `2xx`; RU-3 →
  `3xx` **and** `4xx`; RU-5 → `5xx`; RU-6 → `6xx`; RU-7 → `7xx`. **No report
  cites another auditor's finding.**
- Hex tokens other than `d9120d7`: **none, in any file.**
- `RU-3.md` header: *"PASS A — `d-bundle`, RU-3 and RU-4 … as instructed"*.

Every check reproduces exactly. The corpus is what I read.

**Two items move off my "could not check" list.**
`VERDICTS.md` § D exists and reads *"the panel budget was spent where it changes
what a corrector does"* — precisely what the audit cited it as saying. § C is
titled *"Findings that SURVIVED the panel — 4"*, which is the pre-round-2 state
and is consistent. My item 2 is discharged.

**Consequence.** `§ 2`'s pointer `pass-a/RU-3.md` now resolves. `VERDICTS.md`
and `EVIDENCE.md` now resolve. A reader with only the repository can now open
any Pass A report and check any published statement against its frozen source —
which is what the freeze was always supposed to guarantee and could not.

---

## 4. Repair 3 — the evidence half is exactly what I asked for

`ev-0169d294`, 18:28:11Z, role `warden`, `train-status.py`, taken **before** the
fix. Its final line:

```
  ouverts : DBND-101, DBND-102, DBND-201, DBND-202, DBND-301, DBND-302, DBND-401,
            DBND-402, DBND-501, DBND-502, DBND-503, DBND-504, DBND-505, DBND-601,
            DBND-603, DBND-701, DBND-702, DBND-703, DBND-704, DBND-705, DBND-706,
            DBND-707, DBND-708, DBND-709, DBND-710
```

Twenty-five retired identifiers, six of them (`DBND-302`, `504`, `505`, `705`,
`708`, `710`) findings the audit publishes as removed. **The breach is now in
the ledger as a transcript rather than only as my description of it**, which is
what I asked for and why I asked for it. Taking the "before" reading of a defect
you are about to fix, so the defect survives its own repair, is good practice and
I want it noted as such.

The file itself is unrepaired at `f6bae5f`.

---

## 5. Repairs 4, 5, 6 — correct in form, and the form matters

Ledger line 54 appends three `corrections`, each with `supersedes`, and states
*"appended rather than rewritten because the ledger is append-only."* I checked
the wording of each against my findings:

- **Six auditors, not seven** — supersedes *"every entry saying SEVEN auditors,
  and `pass-a/frozen.json` which says the same"*, and explains why the freeze is
  not edited. It also states the thing I said in my first report and want kept:
  *"the isolation was sound … what is wrong is the accounting, not the method."*
- **Nineteen distinct edits over twenty transcripts** — supersedes the two ledger
  entries and the audit. A3's addition is real and better than my finding: the
  dropped mutant is `ev-5474b889`, one of only two runs in the whole campaign
  that show a scenario **catching** something. Dropping *that* one from a
  campaign total is the least defensible one to drop, and the audit's own § 4.2
  makes the point (*"an audit that only runs mutants it expects to survive is
  measuring its own confidence"*). I did not spot which mutant the count had
  lost; A3 did.
- **`bundle.rs:1772`** — supersedes the `DBND-018` citation.

All three are corrections to the *record*. None has reached the audit text.

---

## 6. The round-2 panel — checked hardest, and it holds

This is the substantive change: four findings removed from the corrector's lot.
I verified all four decisive facts in the source at `d9120d7`, which I now have.
`git diff --stat d9120d7 f6bae5f -- rust/ spec/ vectors/` is **empty**, so the
code I read is the audited code.

**1. `DBND-020` (Pass A `DBND-503`) — REFUTED, fact confirmed exactly.**
`rust/crates/aithos-bundle/tests/cb2_bundle_authority_flows.rs`:
```rust
assert!(BUNDLE_SOURCE.contains("&OwnerKeys"));
for absent in ["pub fn content_operation(", "pub fn open_bundle_session(", "pub fn export_keyless("] {
    assert!(!BUNDLE_SOURCE.contains(absent) && …, "{absent}");
}
```
An **executing** test that requires `&OwnerKeys` and forbids a session-opening
surface. The absence the finding complained of is an asserted design decision,
and the finding's own remedy would turn this test red. Sound.

**2. `DBND-030` (Pass A `DBND-701`) — REFUTED, fact confirmed exactly, at the
stated line.** `cucumber.rs:3011`:
```rust
let operation_succeeded = aithos_core::gamma::verify_owner_entry(&entry, &did).is_ok();
```
That row's `operation_succeeded` **is** a real signature verification, under the
sentence *"the signature verifies against the public key"*. The claim's stated
centre — that no row sentence is ever evaluated — is false on that row. Sound.

**3. `DBND-035` (Pass A `DBND-709`) — REFUTED, fact confirmed.**
`vectors/cb2-bundle-boundaries.json:363` is `"check_applies_to": [` followed by
`read`, `write`, `list`, `cold_load`, `staging_publication`, … The repository's
own normative vector declares the six as application points of **one** check and
classifies `cold_load` among them. The finding treated them as six independent
surfaces of which five are untested. Sound.

**4. `DBND-026` (Pass A `DBND-603`) — refuted, but the lead fact does not carry,
and A3 is right.** This is the one the orchestrator asked me to look at hardest,
and it asked correctly. `lib.rs:527-530`:
```rust
fn canonical_base(&self) -> io::Result<PathBuf> {
    if let Some(transaction) = &self.transaction {
        return Ok(transaction.staging.clone());
    }
```
The refuter's fact — *"`canonical_base()`'s first branch returns the staging
directory itself"* — is **literally true and materially incomplete**: the branch
is guarded by `if let Some(transaction) = &self.transaction` and fires **only
while a transaction is active**. If the three snapshots are taken with none
active, control never reaches it. **A3's disagreement is correct, and it caught
it by verifying its own refuter's lead rather than accepting a verdict that went
its way.** That is the right instinct, disclosed unprompted, and it is the
answer to the orchestrator's own question — *"a refutation accepted on the wrong
leg is a second error wearing the costume of a correction."* On the leg, A3 has
it right and the refuter had it wrong.

**What I could not check on this one:** the second and third routes on which A3
says the finding still falls. Those are in § 7 of the rewritten audit, in
`8068426`. So I can certify that the refuter's lead is bad and that A3 saw it; I
**cannot** certify the legs that replaced it. `DBND-026` is the one finding of
the four whose removal I am not in a position to confirm, and it should not be
read as confirmed by this report.

**The identifier confusion — final state verified right, not just the caution.**
The orchestrator disclosed that its brief to A3 named `DBND-035` for what is
`DBND-030` and `DBND-039` for what is `DBND-035`; followed literally it would
have removed an unattacked P3 and left a killed P2 standing. I checked the
outcome two independent ways:

- § 6.0's map gives `DBND-503→020`, `DBND-603→026`, `DBND-701→030`,
  `DBND-709→035`.
- Ledger line 53 records the four in Pass A numbering: `503`, `603`, `701`,
  `709`. **Both records resolve to the same set: {020, 026, 030, 035}**, which is
  the set the orchestrator reported to me. `DBND-039` is untouched.

**And the set is exactly right for an independent reason.** In my first report I
tabulated the evidential-state label on all 39 findings and found **exactly four**
carrying *"survived the adversarial panel"*: `DBND-020`, `DBND-026`, `DBND-030`,
`DBND-035`. The four now refuted are those four and no others. The category is
empty, which is why striking it from the legend is right rather than tidy.

**Arithmetic.** All four were P2 (§ 6.0 severity column). 39 − 4 = **35**;
17 − 4 = **13**. `2 P1 + 13 P2 + 20 P3 = 35`. Consistent.

**On my own judgement, since the result bears on it.** I ruled the four
under-panelled because the orchestrator's stated rule fires on judgement-based
non-refutations and had not been applied. Round 2 refuted all four, three on
facts I have now confirmed exactly. **10 tested, 10 refuted, 0 survived.** The
panel the orchestrator tried to economise on removed 40 % of the P1/P2 class, and
the four it nearly waved through were the four it mattered most on. I record
that it accepted the ruling without overruling it and then produced the result
that made the ruling bite.

---

## 7. Verdict, and what I am and am not saying

**`INVALIDATED`.** Invalidation 2 of 2. The run stops on this feature under
blocking condition 6 and goes to the owner.

**The reason, stated so it cannot be misread.** Not because the remediation is
wrong — every verifiable part of it is right, and I checked rather than
believed. Not because I distrust the report — its one unverifiable claim I could
test (*"everything up to `f6bae5f` is on the remote"*) was exactly true. **The
reason is that `8068426` is not in the repository.** At `f6bae5f`,
`docs/audits/features/d-bundle.md` is byte-identical to the document I
invalidated: 2954 lines, 39 findings, four of them now known false, `seven`
auditors, `seventeen` mutants, `bundle.rs:1770`, a false worktree claim, and
every feature-file line number stale. `STATE.md` is byte-identical: body saying
the feature has not started, `open_findings` carrying 25 retired identifiers
including six — now ten — dead findings.

I cannot authorise a transition to correction against that state, and the
corrector reads exactly those two files.

**Why this is the right destination and not a punishment.** Blocking condition 6
routes to the owner. The owner is precisely who should hear that the train
cannot push, that its container demonstrably resets to `a2087f2`, and that a
complete remediation — a 35-finding audit, four refutation blocks, a line-offset
table, a rewritten state file — currently exists in one place that has already
proved fatal once today. Stopping an automated train from transitioning on an
undeliverable state is what a blocking condition is for. The orchestrator raised
the push failure to me itself, *because* it bears on breach 1; it was right that
it does, and this is where that observation lands.

**What I am explicitly not asking for.** No agent re-run. No re-audit. No
re-panel. No finding re-opened. Nothing about the cycle's substance is in doubt
on anything I could reach.

**What discharges this.** One thing: **get `8068426` into the repository.**
Restore push access, or land it by any route the owner accepts — the `git bundle`
already written to the owner's disk applied to a clone with credentials, a patch
mailed and applied, a fresh push from an environment without the 403. Then a
re-review is short: I read the 35-finding audit, § 5.2's offset table, the
rewritten `STATE.md`, the four § 7 blocks — in particular `DBND-026`'s
replacement legs, the one thing in § 6 I could not certify — and the two
post-remediation gate transcripts. If they are what the report describes, that
review ends in `VALID`, and I would expect it to.

**If the owner judges the 403 an infrastructure fault outside the train's
control and lands `8068426` by hand**, then in my view this verdict has done its
whole job by making that happen, and the run should resume rather than restart.
That is the owner's call, not mine. My job was to refuse to certify what I
cannot see, and to be unmistakably clear that what I could see was good.

---

## 8. Commands I want run

Only after `8068426` is in the repository. I run none of them.

1. `python3 features/.agents/scripts/train-status.py` — the "after" reading, to
   pair with `ev-0169d294`. Expect `ouverts :` to list the 35 published
   identifiers and no retired one.
2. `bash features/.agents/scripts/verify-feature-tags.sh` — journalled against
   the final tree.
3. `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle`
   — expect `1 feature / 7 rules / 51 scenarios / 299 steps`.

I note the orchestrator's claim that the post-remediation feature gate came back
as `ev-5f523aae`, byte-identical to the pre-Pass-B baseline. If so it is a
genuinely strong result — 99 lines of marker rewriting and a document rewrite
producing the same transcript hash as the run before any of it existed — and
under content-addressing it is self-proving. **It is also not in the ledger at
`f6bae5f`**, whose last gate line is `ev-0169d294`. I could not check it and do
not count it.

---

## 9. What I could not check, and why

1. **The entire content of `8068426`**: the 35-finding audit, § 1's rewrite,
   § 5.2's offset table, § 6.1 and § 15's corrections, § 6.0's identifier
   caution, § 7's four new removal blocks, the struck legend entry, and the
   rewritten `STATE.md`. Not on any ref; `git cat-file -t 8068426` fails.
2. **`DBND-026`'s replacement legs** — § 6. Its removal is the one of the four I
   am not in a position to confirm.
3. **`ev-14592971` and `ev-5f523aae` post-remediation** — § 8.
4. **Byte-comparison of the committed Pass A corpus against the out-of-tree
   originals.** The originals were destroyed with `/root/work`. I substituted
   full re-execution of my structural checks (§ 3), which all reproduce, but that
   is corroboration, not identity.
5. **Anything requiring a command.** I ran no gate, no test, no `cargo`.

---

## 10. Notes on substance — these do not bear on the verdict

1. **The append-only discipline held under pressure.** The correct move on
   breach 4 was the tempting-to-get-wrong one: `frozen.json` says *seven* and is
   wrong, and it is hashed and cited by that hash. Editing it would have made
   every citation of the freeze false and destroyed the one artefact whose
   immutability the whole Pass A guarantee rests on. Appending a superseding
   entry and making the audit say six is right. I want that on the record because
   the wrong choice would have looked like a more thorough repair.
2. **A3 disagreeing with a refuter whose verdict went its way** (§ 6) is the
   single best moment in the remediation, and it is the same shape as § 8.5 in
   the original cycle. Twice now this role has produced its best work by
   re-checking something nobody asked it to re-check.
3. **The orchestrator's self-accounting is accurate where I can test it.** Ledger
   line 53 restates my six breaches without softening, including *"I wrote the
   lesson down and then repeated it in the next feature"*, and its round-2 entry
   states the cost of its own shortcut — *"the four I nearly waved through were
   the four it mattered most on"* — rather than the credit for fixing it. The
   push-failure disclosure is of a piece with that.
4. **A structural observation for the next cycle, not a finding.** Three times in
   one day, load-bearing material has been lost or nearly lost by living outside
   the tree: the two embargoed statements at 13:32, the Pass A corpus that repair
   1 rescued with hours to spare, and now `8068426`. The pattern is not
   carelessness; it is that this train's durability boundary is *push*, and push
   is not under its control. `QUEUE.yaml`'s `durability_remote: origin` states the
   intent. Something should verify it — a cycle that cannot reach `origin` should
   block before it produces work it cannot deliver, rather than after. That is a
   process amendment for the owner, and I raise it rather than decide it.

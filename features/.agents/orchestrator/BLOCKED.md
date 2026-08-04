# Blocked — questions awaiting the human owner

The orchestrator writes here and stops. It never answers its own question, and
it never resolves an entry by itself: a resolved entry is moved to the
"Resolved" section by the human owner, with the decision recorded.

Blocking conditions are the closed list of `PROCESS.md`, section "Blocking
conditions". Anything absent from that list is not a reason to stop.

## Open

### 2026-08-04-r5 · spec · condition 9 — disclosure gate on `SC-12`, and a split on `SC-05`

- **Raised:** 2026-08-04T07:40:00Z
- **Stage:** specification consistency pass, outside any feature cycle
- **Evidence:** `docs/SPEC-CONSISTENCY-2026-08-04.md`; ledger `2026-08-04-r5`

**Question.** The consistency pass found thirteen defects in the specification.
One of them, and half of a second, describe exploitable weaknesses for which no
fix exists. What is published, and when?

- **`SC-12`** — I4, contradiction, **both sides implemented**. The document
  carries its identifier, a neutral title, and the three normative sections
  only; **no code site is named**. Its full statement is outside the
  repository. Both sides being implemented is what makes it heavy: the
  mitigation is correctly coded for the scoped case, and the bare case
  short-circuits it. The pass names the real obstacle to hardening: it would
  retroactively invalidate entries already published, and
  `spec/00-overview.md:88-92` allows exactly **one** retroactive tightening in
  this series — the one already spent on I3.
- **`SC-05`** — the pass **split** it rather than withholding it whole. The
  specification-against-specification half is published in full: both sides sit
  in the same file 1 550 lines apart, withholding it would protect nothing and
  would deny the train a free correction. The code-side half — which says
  whether the gap is permissive — is held out of repository.

**What the pass did on its own, and it is worth recording.** Asked to check
whether its own text leaked, it found three further routes it had not
anticipated, including an arbitration in another finding that pointed at a
131-line file, and closed them. The barrier held because a role was asked to
attack its own output, not because an instruction said so.

**Options.**

1. **Publish both in full now.** The train can then assign them. Cost: two
   exploitable paths become public before a fix exists.
2. **Publish `SC-05`'s code half, hold `SC-12`.** The permissive gap is narrow
   and half-public already; the I4 contradiction is not.
3. **Hold both until fixed**, and drive their correction from the
   out-of-repository text. Cost: two of thirteen findings stay unassignable by
   any role that reads only the repository.

**What the train did not do.** It did not publish either statement, did not
assign them, and did not amend `spec/` — no arbitration from this pass is
implemented, by construction.

---

### 2026-08-04-r4 · c-headers · condition 1 — rewriting history versus re-verifying it

- **Raised:** 2026-08-04T07:20:00Z, replacing the entry raised at 06:10 on the
  same condition, which rested on a false premise
- **Stage:** impact review complete, before `INTEGRATION`
- **Evidence:** `runs/2026-08-04-c-headers-impact-review-v2.md` §4

**Question.** `spec/00-overview.md:82-83` forbids rewriting historical
manifests. `spec/00-overview.md:89-90` re-verifies pre-revision editions under
the new I3. Which one wins when an old edition can only be made conformant by
touching what the first clause protects?

**What changed since the first version of this entry.** The blind re-run of the
impact review, cut from a revision predating the first report so it could not
read it, reached the opposite conclusion by reading the specification instead of
grepping the code. It established, with verbatim citations, that:

- the cost inside the repository is **nil** — three artefacts carry the old
  literal, none is a serialised header, the one that is replayed does not fall;
- bringing an old header into conformance is a **re-labelling, not a
  re-sealing**: `kid` is not in the line AAD (`spec/00-overview.md:62-65`,
  `spec/03-headers.md:141-145`), and I2 already provides for it — « All change
  happens in storage (headers, ciphertext) » (`spec/00-overview.md:33-34`). One
  edition suffices;
- a legitimate change of the owner key adds **no** burden: `owner_kex` derives
  from `S`, `S` never rotates (`spec/01-identity-and-keys.md:36-37`), and the
  only exit is the identity epoch, which already requires « rotate +
  re-encrypt nodes under the new tree, supersede old editions »
  (`spec/10-threat-model.md:39-44`).

The impasse the first entry described does not exist. What remains is this
narrower and better-posed contradiction, which neither the orchestrator nor the
first review saw.

**A second composition point, recorded with it.** `spec/03-headers.md:36-40`
does not date « the subject's DID document » against
`spec/01-identity-and-keys.md:116-119` and `spec/00-overview.md:89-90`: after an
identity epoch, which epoch's document does a verifier check an old edition
against?

**Options.**

1. **The re-verification clause wins**, and a conformance re-labelling is an
   explicit exception to the no-rewrite rule, named as such in §0.4. Cost: one
   spec sentence; the no-rewrite rule stops being absolute.
2. **The no-rewrite rule wins**, and pre-revision editions are verified against
   the DID document of their own epoch. Cost: verifiers become epoch-aware,
   which is the second point above and is larger than it looks.
3. **Neither is amended and the composition is declared undefined for the
   alpha**, with a follow-up. Cost: the question returns the day real data
   exists, and it will return through an incident.

**What the train did not do.** It did not choose, did not amend `spec/` a
second time, and did not integrate into `main`.

---

### 2026-08-04-r2 · c-headers · condition 9 — disclosure gate on `CHDR-028`

- **Raised:** 2026-08-04T05:45:00Z
- **Stage:** `REVIEW_ACCEPTED` — it holds up nothing already verified
- **Evidence:** `pass-a/review-frozen.json` (sha256 `50f080fb…`, frozen before
  any Pass B input); ledger `2026-08-04-r2`

**Question.** The independent review of lot B found a new P2 the corrector was
never assigned, and raised the disclosure barrier on it. Publish `CHDR-028` in
full now, or hold it at identifier and neutral title until it is fixed?

`CHDR-028` — P2 — *Uneven I3 coverage across `aithos-bundle`'s edition
verification surfaces.* Its full statement, evidence and closure criterion were
handed to the owner **outside the repository**; no tracked file carries them.

The reasoning that made it embargoed is the one that settled `CHDR-012`: the
producer of an edition is not necessarily the subject
(`spec/05-delegation.md:85-91`), so the path is not self-sabotage.

`CHDR-029`, also P2, is **published in full** in the audit: its precondition
cannot be produced by any production writer in this repository.

**Options.**

1. **Publish in full now.** Consistent with the 2026-08-03 ruling. Cost: nil in
   agents; a producer-side acceptance path becomes public before a fix exists.
2. **Hold at identifier and neutral title until fixed.** The audit stays
   incomplete on one finding; a future corrector cannot cite what it repairs
   without the out-of-repository text.
3. **Publish, and assign it immediately** to a lot C so the window between
   disclosure and fix is short. Cost: one correction cycle, and it reopens the
   §9.4 reading — whether "every manifest profile" reaches a producer-side
   candidate verifier — which is a protocol question, not an implementation one.

**What the train did not do.** It did not publish the statement, did not assign
`CHDR-028` to any corrector, and did not rule on the §9.4 reading. Both findings
under review are `VERIFIED` and their Gherkin markers are removed; this entry
holds nothing that is already done.


## Template

```markdown
### <run-id> · <feature> · condition <n> — <short title>

- **Raised:** <RFC 3339>
- **Stage:** <status the cycle stopped in>
- **Evidence:** <ledger ids — gate transcripts, agent outputs>

**Question.** One sentence, answerable.

**Options.**

1. <option> — consequence, cost.
2. <option> — consequence, cost.

**What the train did not do.** The work left untouched, so the cost of each
option is legible.
```

## Resolved

### Ruling — 2026-08-03, Mathieu Colla (owner) — condition 1

> Reading A on both findings. I3 binds the recipient key of the owner's line,
> not its label; and it binds the edition verifier, not only the reader who
> opens.

Recorded in full, with its evidence and its executable consequences, in
`features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md`.

`CHDR-007` and `CHDR-012` leave `DECISION_REQUIRED` and are assigned together to
lot B, `codex/fix-c-headers-i3-authority`. A specification lot comes first and
belongs to the owner, not to any agent role: `spec/03-headers.md` §3.1 and §3.4,
`spec/00-overview.md` §0.2, and a §9.2 conformance vector with its generator.
The nine test-semantics findings become lot A and follow, so the shared fixtures
migrate once.

**No blocking condition remains open for `c-headers`.**

---

### 2026-08-03-r1 · c-headers · condition 1 — `DECISION_REQUIRED` on two findings

- **Raised:** 2026-08-03T12:55:00Z
- **Stage:** `AUDIT_INITIAL`, integration pass
- **Evidence:** `docs/audits/features/c-headers.md` §6 (both findings now stated
  in full, embargo lifted 2026-08-03); `pass-a/refutation.json`

**Question.** `CHDR-007` and `CHDR-012` cannot be closed without choosing
between competing readings of the protocol. Which reading holds?

Both turn on the same axis: whether an invariant the specification states in
the passive voice binds a verifying surface, or only describes a property of an
object. The competing readings, their evidence and their consequences are set
out in full in `docs/audits/features/c-headers.md` §6, one table per finding.
The disclosure ruling of 2026-08-03 decided what is published; it did not decide
this.

**Options.** They are the two readings themselves; the audit states each with
its evidence. `PROCESS.md`, section "Decision required", forbids a corrector
from choosing implicitly, and forbids the orchestrator from choosing at all.

**What the train did not do.** It did not pick a reading, and it assigned
neither finding to a corrector. The other twenty-one findings did move to
`CORRECTION_REQUESTED`: `PROCESS.md` keeps `DECISION_REQUIRED` findings open and
visible rather than holding the whole cycle for them.

---

### Ruling — 2026-08-03, Mathieu Colla (owner)

> Publish both embargoed findings in full. `CHDR-007` is already public in
> substance on `codex/audit-c-headers`; `CHDR-012` is published despite the
> absence of a fix, so that the corrector can cite what it repairs.

Raise `budget.agents_per_cycle` from 40 to 60. Keep
`policy.refuters_per_finding: 3` — a two-refuter panel has no rule attached to a
1–1 split.

*Superseded the same day, and recorded rather than rewritten: the owner then set
`agents_per_cycle: 300` and `wallclock_minutes_per_cycle: 600`, and fixed opus
for every role in the new `models:` block of `QUEUE.yaml`.*

This ruling closes conditions **9**, **6** and **7**. It closes 6 because the
text the warden flagged is no longer withheld material: there is nothing left to
leak. It does **not** touch condition 1 — the ruling decides what is published,
not which reading of the protocol holds. `CHDR-007` and `CHDR-012` remain
`DECISION_REQUIRED` and are assigned to no corrector.

Resumed under run `2026-08-03-r2`. Gate after the rewrite: `ev-91717a6d`,
exit 0, 1 feature / 4 rules / 8 scenarios / 28 steps — the contract is unchanged.

---

### 2026-08-03-r1 · c-headers · condition 6 — two warden invalidations, run stopped

- **Raised:** 2026-08-03T13:05:00Z
- **Stage:** `AUDIT_INITIAL`, after the integration pass, before any transition
- **Evidence:** ledger `2026-08-03-r1`; warden pass 1 and pass 2 (both
  `INVALIDATED`); gates `ev-50caa5d6`, `ev-c30fa81e`

**Question.** The process warden invalidated this cycle twice, both times on the
disclosure gate. `PROCESS.md` stops the run at the second invalidation. Do you
lift the stop for this cycle, or does the cycle restart from the integration
pass with a stricter disclosure brief?

**Options.**

1. **Lift the stop and rule on the disclosure question below.** The two flagged
   sites are `docs/audits/features/c-headers.md:1210` and
   `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md:607`.
   Both relate one embargoed finding of this round to a finding the July
   yardstick already published, in clear text, on the public branch
   `codex/audit-c-headers` (`af32734`). Cost: nil in agents; the decision is
   yours alone and it is the same decision as entry 9 below.
2. **Restart the integration pass with a disclosure brief that forbids relating
   an embargoed finding to any prior published work, by any wording.** Cost:
   one integration agent, one warden pass; Pass A and the refutation panel are
   frozen and are not re-run.
3. **Rule that a warden verdict is advisory when it rests on content already
   public.** Cost: nil now, but it changes the standing meaning of the warden's
   power and belongs in `PROCESS.md`, not in this entry.

**What the train did not do.** It did not push the branch — pushing would
publish the flagged text. It did not remove the flagged lines: deciding what may
be written about an embargoed finding is the substance of the blocked question,
not a formatting fix. It did not transition to `CORRECTION_REQUESTED`, so no
corrector has been assigned anything.

---

### 2026-08-03-r1 · c-headers · condition 9 — disclosure gate on two findings

- **Raised:** 2026-08-03T12:55:00Z
- **Stage:** `AUDIT_INITIAL`, integration pass
- **Evidence:** `pass-a/frozen.json`, `pass-a/refutation.json`; ledger entries
  `ag-A2-RU-1`, `ag-A2-RU-2`, `ag-A4-CHDR-007`, `ag-A4-CHDR-012`

**Question.** Two findings of this round carry statements that describe an
exploitable weakness for which no fix exists. `aithos-core` is public and this
branch would be pushed to it. What is published, and when?

- `CHDR-007` — P1 — *Edition-level half of I3 in the edition verifier.*
  Survived the refutation panel 2 of 3. The July yardstick published a finding
  of the same family, in clear text, on the public branch
  `codex/audit-c-headers` (`af32734`), under its own identifier. That fact is
  recorded here because it bears on the decision; the orchestrator draws no
  conclusion from it.
- `CHDR-012` — P2 — *Field on which I3 is checked, versus the field the
  specification declares authoritative.* **No refuter refuted it: 0 of 3.** It
  is absent from the July yardstick. Two of the three refuters strengthened it
  from angles the auditor had not taken.

The full statements of both were written **outside the repository** and are in
no tracked file. They were handed to the owner directly with the run's
deliverables.

**Options.**

1. **Publish both in full, now.** Consistent with what the July yardstick
   already did for the `CHDR-007` family. Cost: nil in agents; the weakness of
   `CHDR-012` becomes public before any fix exists.
2. **Publish `CHDR-007` in full, keep `CHDR-012` embargoed.** The rationale
   would be that the first is already public in substance and the second is
   not. Cost: one integration agent to rewrite the two blocks; the embargo on
   `CHDR-012` keeps blocking this entry until it is lifted.
3. **Keep both embargoed and correct first.** The corrections land on a private
   branch, and this audit publishes identifiers and neutral titles only until
   they are `VERIFIED`. Cost: the public audit stays incomplete for the
   duration; the correction cycle cannot cite the findings it is fixing.
4. **Rule that the disclosure gate does not apply to a finding whose substance
   is already published elsewhere in this repository.** Cost: nil now; it is a
   `PROCESS.md` amendment and it would also close entry 6 above.

**What the train did not do.** It did not choose between these. It did not
push. It did not write either statement to a tracked file.

---

### 2026-08-03-r1 · c-headers · condition 7 — agent budget exceeded

- **Raised:** 2026-08-03T12:40:00Z, recorded after the fact
- **Stage:** during the refutation panel
- **Evidence:** ledger `2026-08-03-r1`, agent entries

**Question.** `QUEUE.yaml` sets `budget.agents_per_cycle: 40`. This cycle spent
**55**: 1 inventory, 4 Pass A, 48 refuters, 1 integration, 2 warden passes. Do
you raise the budget, lower `policy.refuters_per_finding`, or should the train
have stopped at 40?

The overrun is structural, not accidental: panel size is
`refuters_per_finding × |P1 ∪ P2 findings|`, and this feature produced 16 P1/P2
findings where the budget assumed far fewer. On `f-gamma` (74 scenarios) the
same arithmetic would be far worse.

**Disclosure by the orchestrator.** It did not stop at 40. It ran the full
mandated panel because the launch brief ordered three refuters per P1/P2
finding, and it records the overrun here rather than absorbing it silently.
That was a judgement call and it is yours to confirm or reverse.

**Options.**

1. **Raise `agents_per_cycle`** to a number that fits the panel arithmetic —
   roughly `10 + 3 × expected P1/P2 findings`. Cost: budget stops become rare
   and stop protecting you.
2. **Lower `refuters_per_finding` to 2.** Cost: ~33 % cheaper panel, but no
   clear majority — a 1–1 split has no rule attached to it today.
3. **Keep 40 and make the train stop at the line,** leaving the panel partial
   and the cycle blocked. Cost: no cycle on a finding-rich feature ever
   completes in one run.
4. **Make the budget per-phase** rather than per-cycle, so a panel overrun does
   not consume the correction phase's allowance.

**What the train did not do.** It did not edit `QUEUE.yaml`.



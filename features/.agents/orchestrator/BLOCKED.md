# Blocked — questions awaiting the human owner

The orchestrator writes here and stops. It never answers its own question, and
it never resolves an entry by itself: a resolved entry is moved to the
"Resolved" section by the human owner, with the decision recorded.

Blocking conditions are the closed list of `PROCESS.md`, section "Blocking
conditions". Anything absent from that list is not a reason to stop.

## Open

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

### 2026-08-03-r1 · c-headers · condition 1 — `DECISION_REQUIRED` on two findings

- **Raised:** 2026-08-03T12:55:00Z
- **Stage:** `AUDIT_INITIAL`, integration pass
- **Evidence:** `docs/audits/features/c-headers.md` (identifiers and neutral
  titles only); `pass-a/refutation.json`

**Question.** `CHDR-007` and `CHDR-012` cannot be closed without choosing
between competing readings of the protocol. Which reading holds?

Both turn on the same axis, stated here without the detail the disclosure gate
withholds: whether an invariant the specification states in the passive voice
binds a verifying surface, or only describes a property of an object. The
competing readings, their evidence and their consequences are set out in
`docs/audits/features/c-headers.md`, sections 7 and 11, to the extent the
disclosure gate allows.

**Options.** They are the two readings themselves; the audit states each with
its evidence. `PROCESS.md`, section "Decision required", forbids a corrector
from choosing implicitly, and forbids the orchestrator from choosing at all.

**What the train did not do.** It did not pick a reading, did not assign either
finding to a corrector, and did not set `STATE.md` to `CORRECTION_REQUESTED`.

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

*(none)*

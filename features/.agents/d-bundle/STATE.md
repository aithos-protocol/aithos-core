---
feature: d-bundle
status: REVIEW_REQUESTED
mode: review
round: 1
base_main: d9120d7
audit_revision: d9120d7e0d154cee517b983bf7b6cac0cf8e8096
candidate_revision: 9807374c58a46e30a0864b24d6be66c516674394
branch: codex/fix-d-bundle-lot-a
assigned_findings: [DBND-001, DBND-002, DBND-003, DBND-007, DBND-008, DBND-013, DBND-014, DBND-018, DBND-019, DBND-025, DBND-026, DBND-029, DBND-031, DBND-032, DBND-033, DBND-034, DBND-040]
open_findings: [DBND-001, DBND-002, DBND-003, DBND-004, DBND-005, DBND-006, DBND-007, DBND-008, DBND-009, DBND-010, DBND-011, DBND-012, DBND-013, DBND-014, DBND-015, DBND-016, DBND-017, DBND-018, DBND-019, DBND-021, DBND-022, DBND-023, DBND-024, DBND-025, DBND-026, DBND-027, DBND-028, DBND-029, DBND-031, DBND-032, DBND-033, DBND-034, DBND-036, DBND-037, DBND-038, DBND-039, DBND-040]
rejection_count: {}
blocked: null
last_transition: 2026-08-05T11:05:00+00:00
---

# Domain state — `d-bundle`

> **Read this section first. Everything below it was written by the bootstrapper
> before the audit ran, and is kept for the record, not for instruction.** Where
> the two disagree, this section wins.

## Round 1 — initial audit COMPLETE, `AUDIT_INITIAL`, 2026-08-04

| Field | Value |
|---|---|
| Status | **`CORRECTION_REQUESTED`**. The warden validated on its third review (`warden-3.md`), after invalidating twice — once on six real breaches, once on delivery when the remediation commit could not be pushed. Two further rounds of measurement followed the `VALID` and changed the note; the warden ruled each outcome in advance of its transcript |
| Audited revision | `d9120d7`, frozen. Feature gate `ev-6a76a789`, green, **1 feature / 7 rules / 51 scenarios / 299 steps** — a count the bootstrapper had reached from the file alone before anything ran |
| Branch | `codex/audit-d-bundle` |
| Public audit | `docs/audits/features/d-bundle.md` |
| Findings | **37 active** — 2 P1, 15 P2, 20 P3 — reconciled from 48 frozen in Pass A. Ten more were **removed by the adversarial panel** and are published in §7 with the fact that killed each |
| Evidential state, per finding | fifteen are **confirmed by a named mutant transcript**; twenty stand **on the record alone** and say so in those words. **No finding survived the adversarial panel** — ten were tested and ten fell, so that category is empty and the audit says so rather than leaving it silently unused |
| Pass A | six auditors, seven review units — `RU-3` held `RU-4` as well, because the pair is the finding surface. Out-of-tree extracts with no `.git`, no run journal, no ledger, no prior verdict. Frozen at `pass-a/frozen.json` **before any mutant ran**; the warden verified the ordering at 6m15s and the hash |
| Mutants | **nineteen distinct edits over twenty transcripts**, all run by the orchestrator, every one named and predicted by an auditor before a transcript existed |
| Warden | **INVALIDATED once**, `runs/2026-08-04-r7/warden.md`. Six breaches, all real, all repaired. **A second invalidation of this feature stops the run** (blocking condition 6) |
| Blocked | no |

### The two P1

They are the same shape twice, found by two auditors who never spoke: a clause
of the contract defended by an assertion that cannot fail. One is
`assert_eq!(0, 0)` on a counter the harness writes as a literal; the other is
`assert!(!false)` on a flag the harness writes as `false` at four sites. Both
are confirmed by mutant.

### What the audit says the feature gets right

The 360-line cached-verdict proxy class that dominates `f-gamma` does **not**
reach this feature. Both columns of the largest `Scenario Outline` were changed
to values that exist nowhere in the repository and both go red (`ev-f0658ee9`,
`ev-de8fa887`). That is a measured negative result and it is recorded as one.

### Next role — the corrector, and its lot is exactly seventeen

`assigned_findings` carries the **2 P1 and 15 P2**, and every one of the
seventeen is **confirmed by transcript**. That was not true of any earlier round
of this train: the c-headers lots mixed measured findings with argued ones.

The **twenty P3** stay in `open_findings` and are **deliberately not assigned**.
They rest on the record alone and say so in the note. A finding without evidence
is not a thing to correct, it is a thing to measure first, and correcting one
would spend a corrector's round on somebody's reading.

---

## Written at bootstrap, before the audit — kept for the record


## Domain state

| Field | Value |
|---|---|
| Status | `READY` — the domain is bootstrapped and no round has been opened. Per `../scripts/train-status.py`, the next role is **I1 then A2 — inventory and Pass A**: freeze the revision, divide into review units, trace history-blind |
| Expected mode | `audit` — initial audit, via `auditor/audit-d-bundle/SKILL.md` |
| Round | 0 |
| Base of the round | **not frozen** (`base_main: d9120d7`). The role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | **not frozen** (`audit_revision: d9120d7e0d154cee517b983bf7b6cac0cf8e8096`) |
| Candidate revision | none (`candidate_revision: null`) |
| Canonical branch | `codex/audit-d-bundle`, the `PROCESS.md` default. The name is free: `QUEUE.yaml` registers no yardstick for this feature and no local or `origin` ref is named for it — see `DOMAIN.md`, § *Branch and evidence*, which also names the unrelated product branch `origin/codex/bundle-publication-performance` so it is not mistaken for one |
| Yardstick | **none.** `../orchestrator/QUEUE.yaml` lists `yardsticks:` for `c-headers` only. There is no prior manual audit branch and no prior public audit note for this feature |
| Canonical tag | `@d-bundle` (`features/d-bundle.feature:1`), alone on the tag line — no `@wip`, no surface marker |
| Contract on disk | 1 feature / 7 rules / 13 authored scenario blocks → 51 expanded scenarios / 299 steps, counted from the file at `7058a96`. A count of the file, never a gate result |
| Public audit | `docs/audits/features/d-bundle.md` — **does not exist yet**. The auditor creates it and adds its row to `docs/audits/features/README.md` |
| Finding identifiers | `DBND-*`. No family is reserved for this feature in `docs/audits/features/README.md`; the prefix, the pattern it follows and the search showing it unused are in `DOMAIN.md`, § *Contract* |
| Findings | none. No audit has run |
| Gherkin markers | none in `features/d-bundle.feature`. No `@audit-*` or `@aid-*` tag and no audit comment anywhere in the file |
| Recorded follow-ups already owed | **seven, plus five repository-wide conditions** — see the section below. This is the largest recorded debt of any feature in the repository at bootstrap time |
| Bootstrapped by | role **B0 — domain bootstrapper**, 2026-08-04, on `main` at `7058a96`, worktree clean. No gate run, no history read |
| Blocked | no |

## What this file is not

Nothing here may be read as evidence about how `d-bundle` behaves. This domain
was bootstrapped **without running a single gate and without reading any
history** — deliberately, so that the auditor's Pass A begins on raw contract,
code and specification. Every path, symbol, count and command in `DOMAIN.md`
and in this file is either a count of a file on disk, stated as such, or a
command to run. None is a result, and none is a verdict on whether a scenario
is well tested.

The recorded follow-ups below are the one exception, and they are not this
role's opinion: they are quotations of `../orchestrator/QUEUE.yaml` and of
accepted public audits, reproduced so no role can miss them. A follow-up names
a **debt**, not a verdict on this feature.

## Inputs

- process: `../PROCESS.md`;
- domain rules and routing, including § *Project stage*: `../../AGENTS.md`;
- domain: `DOMAIN.md`;
- shared skills: `../shared/audit-gherkin-feature/SKILL.md`,
  `../shared/correct-gherkin-feature/SKILL.md`;
- specialised skills: `auditor/audit-d-bundle/SKILL.md`,
  `corrector/correct-d-bundle/SKILL.md`;
- queue, policy, budgets and recorded follow-ups: `../orchestrator/QUEUE.yaml`;
- ledger format and the restricted frontmatter grammar:
  `../orchestrator/LEDGER.md`;
- open blocking conditions and owner rulings repository-wide:
  `../orchestrator/BLOCKED.md`.

## Recorded follow-ups this feature already owes

Seven entries of `../orchestrator/QUEUE.yaml` name `d-bundle`. They were
recorded before this domain existed, by impact reviews that were independently
accepted. Each is quoted **verbatim with its key**. A `TARGETED` follow-up means
a future cycle of this feature owes specific scenarios; it never reopens another
feature by itself.

### 1. `chdr-028` — embargo lifted today, and this cycle is first in line

```yaml
chdr-028: 'EMBARGO LEVE le 2026-08-04 par le proprietaire, publie en entier dans docs/audits/features/c-headers.md section 6bis. verify_draft2_candidate ne controle rien sur I3, donc verify_public_only, verify_for_cas et PublicationUploadPlan::verified acceptent un paquet epinglant un header qui viole I3, la ou cold_verify le refuse. Cloture: verify_draft2_candidate appelle verify_pinned_headers sur candidate_store et did.json, avec un test RED. Owed par le premier cycle de d-bundle ou k-integration qui ouvre. c-headers est COMPLETE et ne le porte pas'
```

`QUEUE.yaml`, key `chdr-028`. **`d-bundle` is at position 2 of the `order:`
list and `k-integration` at position 16, so this cycle is the first of the two
to open and owes it.**

The full statement is published in `docs/audits/features/c-headers.md` §6bis,
under the heading "`CHDR-028` — `OPEN`, P2 — **publié en entier le 2026-08-04
sur décision du propriétaire**". Citing it is permitted and expected; do not
re-embargo it. The surfaces it names are all in this domain:
`verify_draft2_candidate` (`rust/crates/aithos-bundle/src/publication.rs:469`),
`verify_public_only` (`:586-591`), `verify_for_cas` (`:643-650`),
`export_keyless` (`:651-694`), and `PublicationUploadPlan::verified`
(`rust/crates/aithos-bundle/src/sdk.rs:35`). Its stated closure criterion is
that `verify_draft2_candidate` call `verify_pinned_headers`
(`bundle.rs:302-320`) on `context.candidate_store` and `did.json`, with a RED
test showing a package pinning an I3-violating header refused where it is today
accepted.

`CHDR-028` is a `c-headers` identifier and stays one; a finding opened here
about the same subject takes a `DBND-*` identifier of its own and cites it.

The same ruling that lifted this embargo also published `SC-12` and the code
half of `SC-05` in full (`../orchestrator/BLOCKED.md`, § *RÉSOLU 2026-08-04*).
`QUEUE.yaml`'s `spec-cons-12` line still reads "BLOCKING, embargo" and is
**stale**; `BLOCKED.md` governs. This domain does not edit `QUEUE.yaml`; the
discrepancy is reported to the orchestrator.

### 2. `chdr-i3-d-bundle` — the two I3 debts named for this cycle

```yaml
chdr-i3-d-bundle: 'an edition pinning an I3-violating header must be refused by verify, and publish carries no such guard (CHDR-034, CHDR-030)'
```

`QUEUE.yaml`, key `chdr-i3-d-bundle`. Recorded by the `c-headers` round-1
impact review, 2026-08-04, with "No `FULL_AUDIT`".

- **`CHDR-034`** — "L'émetteur signe des éditions que son propre vérificateur
  refuse." `Bundle::publish` (`bundle.rs:1678`) carries no I3 guard;
  `c3_owner_line_edition.rs:239-246` writes a mutilated header through the `pub`
  `store` field (`bundle.rs:284`), publishes successfully, and only `verify`
  refuses. Closure: either `publish` refuses to pin an I3-violating header while
  leaving the test a post-signature injection path, or
  `spec/09-cli-and-conformance.md` §9.4 says explicitly that the I3 obligation
  binds verification and never issuance
  (`docs/audits/features/c-headers.md`, §6bis, `CHDR-034`).
- **`CHDR-030`** — the `owner_kex`-holding tier of I3
  (`Header::validate_as_owner`, `rust/crates/aithos-core/src/header.rs:385-401`)
  has no production caller; the four production surfaces holding `owner_kex`
  call only the keyless tier, two of them in this domain — `bundle.rs:667` and
  `:674` (`docs/audits/features/c-headers.md`, §6bis, `CHDR-030`).

### 3. `chdr-016-grant-path` — owed jointly, and this cycle must say who carries it

```yaml
chdr-016-grant-path: 'RE-ROUTED out of c-headers lot A on 2026-08-04 by the orchestrator; neither closed nor withdrawn. The grant path actually taken in production implements neither step 1 nor step 3 of spec 03.3, and no step of Rule 3 touches it. Production behaviour on the bundle grant surface, out of scope for a test-semantics lot (blocking condition 8). Owed jointly by g-revocation and d-bundle; whichever opens first states which one carries it'
```

`QUEUE.yaml`, key `chdr-016-grant-path`. **`d-bundle` is at position 2 and
`g-revocation` at position 9, so this cycle opens first and must state, in its
run report, which of the two carries it.** The bootstrapper does not decide
that: assigning scope without evidence is not bootstrapping.

The statement is published in full at `docs/audits/features/c-headers.md`,
`CHDR-016`. The surfaces are `Bundle::grant`
(`rust/crates/aithos-bundle/src/grants.rs:739`) → `deliver_entry` (`:754`, body
`:308-341`) → `add_line_on` (`:276-305`), against
`Session::append_header_recipient` (`session.rs:354-366`) as the §3.3-conformant
comparison.

### 4. `bder-006-d-bundle` — the tag-view and wrap scenarios

```yaml
bder-006-d-bundle: tag-view and wrap scenarios owed by the d-bundle cycle
```

`QUEUE.yaml`, key `bder-006-d-bundle`. Opened by the `b-derivation` round-1
impact review and reconfirmed by the accepted round-2 review, which narrowed
its useful scope and verified the file itself
(`../orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:461-471`):

> `d-bundle.feature` porte sept `Rule` (`:8`, `:32`, `:45`, `:53`, `:61`,
> `:89`, `:129`) et **aucun scénario de vue par tag**. Le mot `wrap` y apparaît
> quatre fois, jamais comme pontage d'ancre : `:98`, `:106`, `:112` l'énumèrent
> parmi les artefacts qu'une mutation échouée ne doit pas laisser, et
> `:138`/`:146` en font une ligne du tableau `Examples` de la règle « Local
> capabilities and paths stay narrow ». La dette de la ronde 1, élargie par la
> décision `BDER-006`, est donc **toujours due**, et son périmètre utile est
> maintenant plus étroit qu'annoncé.

The same section records the step coupling opened by round 1 and unchanged by
round 2: `rename_the_folder`, `publish_edition` and `reads_at_new_path` — all
three are `d-bundle` steps (`cucumber.rs:8394`, `:8343`, `:12748`).

**A re-arbitration is pending and is not this cycle's to make.**
`../orchestrator/STATE.md:32-42` records that the premise which widened this
debt was inexact, that what is genuinely unproven anywhere is "the zone-root
view's coverage of the whole zone, and an explicit « an anchor derives nothing
downward » negative", that "re-arbitration belongs to the owner of the
`BDER-006` decision", and that "`d-bundle` still owes the co-owned-steps record
(round-1 impact report §9.5) either way". Route the scope question; do not
settle it.

### 5. `b-derivation-round-2-targeted` — this feature is named

```yaml
b-derivation-round-2-targeted: [a-identity, c-headers, d-bundle, e-mandates, n-structural-mutations]
```

`QUEUE.yaml`, key `b-derivation-round-2-targeted`. The row of the accepted
round-2 impact review that names this feature is quoted in item 4 above.

### 6. `chdr-i3-targeted` — this feature is named

```yaml
chdr-i3-targeted: [a-identity, d-bundle, g-revocation, g4-client-surfaces, k-integration, m-delegated-editions, n-structural-mutations, o-connector-classes-vault]
```

`QUEUE.yaml`, key `chdr-i3-targeted`. Recorded by the `c-headers` round-1
impact review, 2026-08-04, with "No `FULL_AUDIT`". The specific content for this
feature is item 2.

### 7. `chdr-lota-vector-generators` — conditional, and this domain is exposed to it

```yaml
chdr-lota-vector-generators: 'CHDR-038 widened. vectors/ holds 29 gen-*.py, no CI step is Python (.github/workflows/ci.yml, 2 jobs, 8 steps, read whole), and nine have no --check mode at all: gen-f, gen-g, gen-h, gen-h2, gen-i, gen-eplus, gen-fplus, gen-gplus, gen-cb2-max-children. For those nine there is no verification mode to run even by hand. Normative ground spec/09-cli-and-conformance.md:109, the vectors gate each. Owed by the first cycle to touch a vector'
```

`QUEUE.yaml`, key `chdr-lota-vector-generators`. **Conditional**: it binds this
cycle only if this cycle touches a vector. All six generators this domain would
run **have** a `--check` mode and none is among the nine; their commands are in
`DOMAIN.md`, § *Vector `--check`*. Three of the neighbouring vectors listed
there — `h1-merkle`, `h2-gamma-roots`, `i1-concurrency` — are produced by three
of the nine.

### 8. Follow-ups that name no feature but reach this cycle

Recorded by the `c-headers` lot A impact review, 2026-08-04, in
`../orchestrator/runs/2026-08-04-c-headers-lot-a-impact-review.md`. Quoted in
the part that binds a role here.

- `chdr-lota-clippy-and-fail-fast` — *"CHDR-039 and CHDR-042 confirmed
  repository-wide. No DOMAIN.md of the three names clippy while ci.yml:34
  enforces it. … ci.yml:37 runs cargo test --workspace without --no-fail-fast
  too, so CI under-reports the same way. Fix the three files and the template
  the bootstrapper copies for the other sixteen."*
  **Discharged for this domain at bootstrap**: `DOMAIN.md` § *Gate pyramid*
  names `clippy` among the final global gates and carries `--no-fail-fast` on
  every multi-binary invocation. The three older `DOMAIN.md` files are not this
  feature's to fix.

- `chdr-lota-mutation-protocol` — *"the rule that a test-semantics correction
  proves itself by a named mutant is in no normative file. PROCESS.md correction
  step 2 says RED test when possible and is silent on the impossible case;
  shared/correct-gherkin-feature/SKILL.md execution steps 1-3 presuppose a
  defect on a production path. Sixteen features have no agent directory yet and
  will inherit the shared skill. Merge with the CHDR-040 amendment, one edit not
  two."*
  **Discharged for this domain at bootstrap**: the rule is written into
  `corrector/correct-d-bundle/SKILL.md`, § *Proving a test-semantics
  correction*. The shared skill is unchanged and is not this feature's to edit.

- `chdr-lota-mutants-as-patches` — *"a mutant named in prose cannot be re-run
  and cannot be pointed, so neither its kill count nor its direction can be
  checked. Two measured costs: docs/audits/features/b-derivation.md:413 records
  the review replaying M3 at 4/6 against the corrector 3/6 … and this impact
  review proposed MI-4 in prose, pointed it the wrong way, and needed the
  orchestrator to run the complement MI-4b before the finding appeared. Lot A
  published its mutants as exact patches (review-lot-a.md:44-68); nothing
  requires it."*
  **Discharged for this domain at bootstrap**: the corrector skill requires
  every mutant to be published as an applicable unified diff.

- `chdr-lota-source-text-assertions` — *"new class, measured by search not by
  mutant, and worse than CHDR-011 because it cannot fail for a behavioural
  reason at all. AT LEAST 51 assertions of the form CONST.contains(literal)
  where CONST is include_str! of a src/ file, across five test files … The sixth
  site is inside the Gherkin layer and is the worst: cucumber.rs:2053-2058,
  core_capability_api_is_narrow(), decides a scenario verdict about capability
  API narrowness by grepping src/session.rs for pub fn sign(, pub fn open( and
  pub fn wrap(. IMPORTANT SCOPE LIMIT: only two of the 52 were read and judged
  defective … The other 50 are counted, not classified … What is owed is the
  triage, not 50 corrections."*
  **This one reaches into this domain's own trace.**
  `core_capability_api_is_narrow()` is the sole source of
  `cross_class_substitution_refused` in all four narrow-capability observations
  (`cucumber.rs:2105`, `:3020`, `:3048`, `:3099`), which is what
  `d_capability_boundary_holds` (`:8480`) asserts for
  `features/d-bundle.feature:137`. The five counted test files are all
  `aithos-bundle` test binaries; the per-file counts of `_SOURCE.contains(`
  reproduce as 16 + 15 + 3 + 10 + 7 = 51 exactly
  (`cb2_bundle_boundaries.rs`, `cb2_bundle_authority_flows.rs`,
  `cb2_draft2_carriers.rs`, `cb2_bundle_structure_vault.rs`,
  `cb2_bundle_concurrency_final.rs`). The scope limit holds: counted, not
  classified.

- `chdr-lota-proxy-verdicts` — the nine features whose Gherkin lines resolve to
  the shared `OnceLock` verdicts. **`d-bundle` is not in that list.** That is a
  statement about which features the lot A review measured, not a finding about
  this one; `DOMAIN.md`, § *Shared steps*, records the search performed here
  against the code, and the auditor reproduces it rather than trusting either.

- `chdr-lota-global-gate-resolution` — *"ev-f818dc4b: with cb10_acceptance() at
  cucumber.rs:6570 short-circuited to Ok(()), the entire Cucumber suite is green
  at 18 features, 114 rules, 836 scenarios, 3577 steps — counters IDENTICAL to
  ev-a1fa00fc … The global gate cannot distinguish a tree whose CB10 oracle
  checks nothing from the tree lot A shipped. … No accepted verdict is
  invalidated … but the reusability of the global counter as evidence is
  damaged."*
  Bears on how much this cycle's own final global gate proves. Recorded, not
  acted on.

### 9. Repository-wide conditions a role here inherits

- **The three lifted embargoes.** `CHDR-028`, `SC-12` and the code half of
  `SC-05` were published in full on 2026-08-04T13:00Z by owner ruling
  (`../orchestrator/BLOCKED.md`, § *RÉSOLU 2026-08-04*). Citing them is
  permitted and expected. **Do not re-embargo them.** `QUEUE.yaml`'s
  `spec-cons-12` entry still describes `SC-12` as embargoed and is stale.
- `disclosure-barrier-durability` — P2, opened 2026-08-04 against the process
  itself: an out-of-repository embargo has no durability, and two of the three
  held statements had vanished by the time the embargo was lifted. Not this
  feature's, recorded so a role that meets it routes it.
- `chdr-i3-rewrite-vs-reverify` — recorded in `QUEUE.yaml` as `BLOCKING …
  Owner ruling required`. **It has since been ruled**
  (`../orchestrator/BLOCKED.md`, § *Ruling — 2026-08-04 … condition 1, rewrite
  versus re-verify*): "The binding is over profiles, not over time." The
  `QUEUE.yaml` line is stale in the same way as `spec-cons-12`.
- `chdr-i3-epoch-dating` — explicitly **not** settled by that ruling and left as
  a follow-up.
- `spec-cons-11-vault-header-path` — *"UNREACHABLE, I3 - 02.3, 02.1 and 03.1
  place the vault header outside the prefix is_header_file requires, so
  verify_pinned_headers never parses it. A spec-conformant implementation
  produces an edition whose pinned header escapes I3"*. Assigned to no feature,
  and it names `verify_pinned_headers`, a symbol of this domain
  (`rust/crates/aithos-bundle/src/bundle.rs:302`). Route it; do not adopt it
  silently.
- `chdr-lota-order` and `chdr-lota-f-gamma-sizing` — queue placement
  recommendations awaiting the owner. They do not change this feature's
  position, which the queue still gives as 2 of 17.

## Current instruction

Run the **initial audit** of `features/d-bundle.feature` with
`auditor/audit-d-bundle/SKILL.md`, in the mode named by the `mode` field of the
frontmatter above.

Before collecting any evidence, freeze the revision and record it in
`base_main` and `audit_revision`, here and in the run report.

Read `DOMAIN.md` completely and read only the routing fields of this file —
mode, branch, base revision, observed revision, assigned scope, expected output
path — before Pass A is frozen. The recorded follow-ups above are routing
material and may be read at any time: they name debts, not verdicts.

Do not read `git log`, `git show`, `git diff`, `git blame`, commit messages,
`docs/audits/`, `../orchestrator/runs/`, or any other feature's run reports
before the Pass A result is frozen (`../PROCESS.md`, § *Pass A — current code,
history-blind*). This feature has no yardstick branch, so the ordinary Pass B
material is the diff and the impact-review reports of other features that name
it — of which there are four, listed above.

**One exception, and it is deliberate.** `docs/audits/features/c-headers.md`
§6bis carries the full published statements of `CHDR-028`, `CHDR-030` and
`CHDR-034`, which this cycle owes. They are Pass B material by the letter of
`PROCESS.md`. Do not open them inside a Pass A review unit. Discharge them in
the Pass B and integration passes, and state in the run report where each was
read; the summaries quoted above are enough for routing and are the only form
in which a Pass A unit may see them.

Name every gate you want run and stop. The orchestrator runs it, journals it
under an `evidence_id` and returns the transcript
(`../orchestrator/LEDGER.md`). Cite the `evidence_id`, never a command you ran
yourself.

## For the orchestrator — one edit this role did not make

`features/AGENTS.md` § *Mandatory routing* lists a block per bootstrapped
feature (`a-identity`, `b-derivation`, `c-headers`, `g4-client-surfaces`) and
has **no `d-bundle` block**. The block this domain needs is:

```markdown
For `d-bundle.feature`:

- audit or review:
  `.agents/d-bundle/auditor/audit-d-bundle/SKILL.md`;
- correction:
  `.agents/d-bundle/corrector/correct-d-bundle/SKILL.md`.
```

`features/AGENTS.md` is outside this role's write scope, so the edit is flagged
rather than made. Until it lands, `AGENTS.md` routes `d-bundle` nowhere and this
file is the only routing.

# Impact review — `c-headers` correction lot A

| Field | Value |
|---|---|
| Run type | Impact review, orchestrated train, round 2 lot A |
| Role | `review-gherkin-impacts` |
| Date | 2026-08-04 |
| Feature | `c-headers` |
| Branch | `codex/fix-c-headers-lot-a` |
| Observed revision | `dae12ab` |
| Baseline | `04860e2` (round 2 opened) |
| Accepted candidate | `5905bec`; audit republished at `dae12ab` |
| Source review | `features/.agents/c-headers/auditor/runs/2026-08-04-review-lot-a.md` (eight `VERIFIED`, no rejection) |
| Source correction | `features/.agents/c-headers/corrector/runs/2026-08-04-correction-lot-a.md` |
| Public audit | `docs/audits/features/c-headers.md` |
| Worktree | two files modified and uncommitted at entry (`features/.agents/c-headers/STATE.md`, `features/.agents/orchestrator/runs/2026-08-04-r6/ledger.jsonl`); this role changed nothing |
| Gates run by this role | **none, in any pass.** Fourteen transcripts, run by the orchestrator, hashed and journalled on run `2026-08-04-r6`. Every behavioural claim below cites one, and I read each transcript rather than the message relaying it (§11) |
| Verdict | **No `FULL_AUDIT`**, re-examined three times against the transcripts and confirmed. Blocking condition 10 is not engaged. Defence in §7.0 |
| Queue order | **`o-connector-classes-vault` moves ahead of `f-gamma`** — §7.0ter, on a threshold pre-registered in §11 before `MI-8` ran and crossed at 0 of 58. The only `order:` change this review recommends |
| Embargo | **nothing raised under blocking condition 9**, assessed in every pass. The close call was `MI-8`; ruling and the supply-chain angle in §6.4. `CHDR-028` untouched |
| Revision | **fourth pass. FROZEN.** Two of my own judgements were corrected by transcript — `MI-4`'s direction (§3.3, withdrawn) and my classing `MI-7` as optional (§11) — and one pre-registration paid out against my prior (§7.0ter). All three are recorded in place, not absorbed |
| Owed before integration | **nothing.** Every question a transcript could settle has one |

---

## 0. Method, and what this report may assert

Three rules bind this run, written after an earlier impact review in this train
got the specification wrong twice.

**R-1 — the specification is a source, not a citation index.** Axis S7 targets
`spec/` directly. Every normative sentence this report leans on is opened,
read, and quoted verbatim to its end, conditional clauses included. No section
is cited by number from memory.

**R-2 — every absence claim carries its search.** Each "nothing does X" below
states the exact command, the scope it covered, and the layer it looked at.
Where I did not search, I say so in §6 rather than claim.

**R-3 — a protocol claim may not rest on a code search alone.** §7 is grounded
on `spec/09-cli-and-conformance.md` before it is grounded on `rust/**`.

**What this report may not assert.** I ran no gate, no test, no `cargo`
command, in either pass. Every claim below is one of: (a) a **structural** claim
— what a file contains, what a regex matches, what a call graph reaches —
established by reading and by the searches quoted; or (b) a **behavioural**
claim, which I never make on my own authority. For each behavioural question I
named the mutant and the command (§11), the orchestrator ran it, and the claim
now cites the resulting `evidence_id`. Where no transcript exists, the finding
stays marked structural.

**What the second pass changed, stated before the sections that changed.** Nine
transcripts came back. Three confirmed a finding I had marked provisional
(`ev-dd652e01`, `ev-db029aa9`, `ev-0002cc6b`). One **refuted a claim of mine**
(`ev-e7d1ca62`) and I withdraw it in §3.3 rather than absorb it, because a
report that quietly rewrites its own prediction is worth less than one that
records being wrong. One closed a disclosure question I had deliberately left
open (`ev-826f8f15`). The verdicts in §7 were re-examined against all nine; one
sentence of §7.0's defence was too weak and is replaced in §7.0bis. No verdict
moved.

**A note on the shape of the refutation, because it is instructive.** My `MI-4`
mutant measured the opposite of what my claim needed. I had claimed
`check_grant_append` was *only ever asserted negatively*, and then proposed a
mutant that turns it into an unconditional failure — which tests whether the
**success** path is exercised, not whether the **gate** is. The orchestrator
caught the mismatch, ran the complement, and the complement is where the finding
actually lives. That is the `CHDR-019` error committed by me, in miniature, one
day after reading the erratum: I stated a mutant in the grammar of the wrong
direction. It is recorded here rather than tidied away, and §8.1's argument
about mutants-as-patches now has a first-person example.

---

## 1. What lot A changed, stated narrowly

Lot A modified **no production source**. Search, whole diff, all layers:

```text
git diff --stat 04860e2..dae12ab
```

Four non-documentary files changed:

| File | Nature |
|---|---|
| `rust/crates/aithos-bundle/tests/cucumber.rs` | +327/-? — step semantics for eight `c-headers` scenarios |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | +17 — one positive control inside `c1_fail_closed` |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | +144/-? — structural rotation assertions; first consumer of the vector's `missing_owner_must_fail` case |
| `features/c-headers.feature` | one `Given` phrase (`:68`), plus marker rewrites |

Everything else in the range is `docs/audits/features/c-headers.md`, `STATE.md`,
run reports, the r6 evidence journal and `QUEUE.yaml`.

Confirmed by search that `rust/crates/*/src/` is untouched in the range:

```text
git diff --stat 04860e2..dae12ab -- 'rust/crates/*/src/'   → empty
```

**Consequence for this review.** The `review-gherkin-impacts` skill defines
`FULL_AUDIT` as "a shared helper, API, format, or invariant changed". Lot A
changed none of the four. So `FULL_AUDIT` is unreachable *from the diff*. The
impact worth looking for is not "what breaks" but what lot A **revealed**: a
method, and six defect classes. §2 through §7 are that search.

Two further facts the lot establishes and that this report relies on:

- The audit's own `CHDR-019` mutant is wrong on the code, and the audit now
  carries an erratum (`docs/audits/features/c-headers.md:1216`). §5 asks whether
  other audits are wrong the same way.
- The lot's method — a strengthened assertion over correct code is green the
  moment it is written, so the only honest RED is a named mutant — is stated in
  three places (`STATE.md`, the corrector run, the review §0) and in **no**
  normative file. §2.

---

## 2. Contagion search S1 — the mutation protocol

**The question.** Which features have accepted test-semantics corrections
**without** a mutant?

**The searches, their scope, their layer.**

```text
grep -rn -i "mutant\|mutation" features/.agents/           # whole agent tree, all roles
grep -rn -i "mutant\|mutation" docs/audits/features/       # the three public audits
grep -n -i "mutant\|mutation\|RED" features/.agents/shared/correct-gherkin-feature/SKILL.md \
   features/.agents/shared/audit-gherkin-feature/SKILL.md \
   features/.agents/{a-identity,b-derivation,c-headers}/corrector/correct-*/SKILL.md \
   features/.agents/PROCESS.md
```

**Result 1 — the protocol is nowhere normative.**

- `features/.agents/PROCESS.md` (372 lines): one hit, `:152`, *"follow each
  parameter into production calls and state mutations"* — a different sense of
  the word. Its § *Correction* step 2 reads *"demonstrates each defect with a
  RED test **when possible**"* and the file says nothing about what to do when
  it is not possible. That is exactly the case a test-semantics lot is in.
- `features/.agents/shared/correct-gherkin-feature/SKILL.md`: one hit, `:37`,
  *"Prove the absence of partial effects for every rejected mutation"* — again a
  different sense. Its § *Execution* steps 1-3 are *"Reproduce each defect on
  the identified production path / Write a RED test that isolates the incorrect
  semantics / Confirm that the test fails for the intended reason."* All three
  presuppose a defect **on a production path**. A test-semantics lot has none.
- The three per-feature corrector skills (`correct-a-identity`,
  `correct-b-derivation`, `correct-c-headers`) each open with *"Read
  `../../../shared/correct-gherkin-feature/SKILL.md` completely"* and none adds
  a mutation clause.

**This is contagion by construction.** Sixteen of the nineteen features on disk
have no `features/.agents/<feature>/` directory at all (`ls features/.agents/`
→ `PROCESS.md a-identity b-derivation c-headers orchestrator scripts shared`).
Their corrector skills will be generated from the shared one. Every future
test-semantics lot in this train will therefore be instructed to produce a RED
that cannot exist, and will have nothing telling it to name a mutant instead.

**Result 2 — one feature has already accepted assertions with no mutant, and it
is `a-identity`.**

`grep -rn -i "mutant\|mutation" features/.agents/a-identity/ docs/audits/features/a-identity.md`
returns exactly **one** hit, `docs/audits/features/a-identity.md:426`, and it is
prose describing a test case named "post-signature mutation" — not a mutant, not
a measurement. The a-identity cycle contains no mutation evidence of any kind.

That would be unremarkable if its corrections were all production changes.
Round 1 was (`rust/crates/aithos-core/src/did.rs` changed). Round 2 was not,
entirely. `features/.agents/a-identity/corrector/runs/2026-07-29-correction-02.md:67`
states, in its own § *RED*:

> The new no-write assertions on already-refused genesis cases passed in RED.

Assertions that pass in RED are assertions with no measured discriminating
power. They are the exact object lot A was created to stop shipping, and
`a-identity` shipped them and is `COMPLETE`.

**Result 3 — `b-derivation` did use mutants, and stated them in prose rather
than as patches.** `features/.agents/b-derivation/corrector/runs/2026-07-29-correction-01.md`
carries nine hits and a per-scenario kill table. Its audit
(`docs/audits/features/b-derivation.md:95-101`) names five mutants as one-line
descriptions — *"M3 hash monolithique"*, *"M4 31 octets de zone recopiés"* —
never as patches. §5 shows what that cost.

**Verdict of S1.** `TARGETED` on the process artefacts, not on a feature:
`features/.agents/shared/correct-gherkin-feature/SKILL.md`. And `TARGETED` on
`a-identity` for the round-2 no-write assertions. Neither is a `FULL_AUDIT`:
no `a-identity` verdict is shown wrong, only unproven.

---

## 3. Contagion search S2 — vacuous negatives with no positive control

**The class.** `CHDR-025`: a test body whose every assertion is negative. A
mutation that makes *nothing* succeed leaves all of them satisfied, vacuously,
for a reason unrelated to the property each one names. The lot A review insists
the vacuity is **per-body** (`c1_header_seal.rs:92-97`, verbatim: *"It must not
be moved to another test function: the vacuity is per-body"*).

**Search A — all-negative test bodies.** Scope: every `#[test] fn` in
`rust/crates/aithos-core/tests/*.rs` and `rust/crates/aithos-bundle/tests/*.rs`,
`cucumber.rs` excluded (it has no `#[test]` fns). Layer: conformance-vector and
integration tests. Method: brace-matched body extraction, then a body is flagged
when it contains `is_err()`/`unwrap_err`/`Err(` and contains **no**
`is_ok()`/`assert_eq!`/`assert_ne!`/`.expect(`/`Ok(`.

15 bodies flagged. Hand-inspected, **13 are false positives**: each has a
positive control in a sibling function of the same file that drives the *same*
entry point, so a mutation killing the path kills the sibling. Named, so the
next reader does not re-do the work: `cb15_external_delegated_grant.rs:43`
(sibling `:39`), `cb14_delegated_session_chain.rs:68` (sibling `:61`),
`a2_did.rs:79` (sibling positive at `:76`), `f1_gamma.rs:213` (sibling at
`:185`), `f3_gamma.rs:80,108,130` (positive at `:104`),
`gplus_obligations.rs:136` (positives at `:104`, `:111`, `:124`),
`f2_gamma.rs:135,155,175,194` (positives at `:169`, `:191`),
`fplus_constraints.rs`, `a1_genesis.rs`, `g2_rotation.rs:a_smuggled_recipient_is_rejected`.

**Search B — the sharper form, and the one that yields.** A validator is vacuous
in the strong sense when it is exercised **only** negatively anywhere in the
test corpus. Scope: every `pub fn check_*|verify_*|validate_*` declared under
`rust/crates/*/src/**`, cross-referenced against every call site in
`rust/crates/*/tests/**` including `cucumber.rs`, classified by the 560
characters of surrounding context.

**Exactly one validator in the whole corpus is exercised only negatively:**

```text
aithos_core::gamma::check_grant_append   (declared rust/crates/aithos-core/src/gamma.rs:722)
  the single test call site: rust/crates/aithos-core/tests/f2_gamma.rs:150-153
    assert!(matches!(check_grant_append(&f.entries, &f.root),
                     Err(Error::GammaBudgetExhausted(_))));
```

Confirmed by plain grep, whole repository, `--include=*.rs`, `rust/target`
excluded: `grep -rn "check_grant_append" --include=*.rs rust/` returns nine
lines — one declaration, five production call sites in
`aithos-bundle/src/log.rs:701,746,814` and `aithos-core/src/gamma_replay.rs:334`,
one `use`, and the one test assertion above.

**What I claimed, before evidence.** Structurally: no test anywhere asserts that
`check_grant_append` returns `Ok`. Behaviourally I claimed nothing, and I noted
the function is reached indirectly through `log.rs` on grant-append paths that
Bundle scenarios may exercise successfully — which would kill an always-`Err`
mutant without any test naming it. I proposed `MI-4` to settle it.

### 3.3 `MI-4` refuted my framing. `MI-4b` found where the finding actually is

**`ev-e7d1ca62` — `check_grant_append` → unconditional `Err`, workspace:
681/836. 155 scenarios fail.**

**My "vacuous" framing is withdrawn.** A validator exercised only negatively in
a *direct assertion* is not thereby undefended: the success path here is
exercised 155 scenarios over, transitively, through `log.rs:701,746,814`. The
hazard `CHDR-025` names — a mutation that makes nothing succeed leaves the
negatives satisfied — does **not** obtain for `check_grant_append`. Search B's
heuristic found a real structural fact and I attached the wrong consequence to
it.

**And the mutant was pointed the wrong way, which is my error and worth
naming.** My claim was about the *gate*: does anything detect the gate failing
to refuse? An unconditional-`Err` mutant tests the opposite direction. The
orchestrator ran the complement.

**`ev-69cd5f74` — `MI-4b`, `check_grant_append` → unconditional `Ok`, workspace:
835/836.** Two casualties in the entire repository:

| Casualty | Site |
|---|---|
| `✘ Then a third delegation is rejected` | `features/f-gamma.feature:64`, step at `cucumber.rs:13639-13643` |
| `spent_budgets_fail_closed` | `rust/crates/aithos-core/tests/f2_gamma.rs:135-154` |

**That is the whole defence of the `max_children` budget in this repository:
one Cucumber scenario and one unit test.** The finding is real, it is sharper
than the one I wrote, and it is a *budget-enforcement* finding rather than a
vacuity finding. `spec/09-cli-and-conformance.md:46-47` requires a vector for
"every fail-closed case", naming "N+1 action" explicitly; `max_children` is the
same shape one level up and `vectors/f2-gamma-counting.json`'s
`second_child_of_root_must_fail` is one of the four keys §6.3 shows has no
consumer. So the vector case that would be the second line of defence is exactly
the one nothing reads.

**One more defect visible in the surviving scenario.** `cucumber.rs:13639-13643`
decides the rejection by `e.contains("budget")` — a **string match on the error
message**, not the typed variant. That is `CHDR-011`'s class verbatim ("the
rejection is asserted through a string match rather than the typed variant"),
here defending the only budget the repository has. Renaming an error string
would silently disarm it.

**`ev-5b25fc6f`** is the orchestrator's failed first attempt at `MI-4b`, a
compile error, journalled and disclosed. Recorded so the evidence-id sequence
reads continuous.

**Owner.** `f-gamma`, and it now owns a named artefact rather than a suspicion:
`f-gamma.feature:61-64` + `cucumber.rs:13639-13643` + `f2_gamma.rs:135-154`, the
sole defence of `max_children`, one of the two asserting on a message substring.
`TARGETED`.

---

## 4. Contagion search S3 — assertions decided by a routing hint, not by a seal

**The class.** `CHDR-019`: `Then the first grantee cannot open the new version`
was decided by the `kid` routing hint; the seal was never reached.

**The normative ground first (R-3, R-1).** `spec/03-headers.md:33-35`, verbatim
and to the end of the sentence:

> - `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
>   a routing hint only — the seal is what grants. Recipients try lines addressed to
>   their `kid`. No verifier decides anything from `to`.

And `spec/03-headers.md:55-57`, verbatim, including the permissive clause that a
citation by section number would drop:

> `kid` orders the
> attempts and nothing else: a reader that finds no matching line MAY try the remaining
> lines, and a successful unseal — never a label — is what proves the line was its own.

That second sentence is the whole point. **The absence of a `kid` does not
establish that a party cannot open**, because the specification explicitly
permits that party to try the remaining lines. Any assertion of the form "the
revoked has no line, therefore the revoked cannot read" asserts something the
specification says decides nothing.

**The search.** Scope: every `#[then]` step definition in
`rust/crates/aithos-bundle/tests/cucumber.rs` (20 040 lines), brace-matched
body. Layer: Gherkin step definitions. A body is flagged when it asserts and
mentions `.kid`/`kid_of`/`.to`/`.label`/`.name`/`recipients`/`.lines` while
containing **no** `open*(`, `read_at`, `agent_reads`, `decrypt`, `verify*(`,
`is_err`, `is_ok`.

Four hits. One is `c-headers`' own, already corrected by lot A
(`cucumber.rs:12539`, `owner_line_untouched`). Two are index-ordering
assertions in `i-concurrency` where naming *is* the property
(`cucumber.rs:18185`, `:18209`) — not instances.

**The fourth is a clean instance, in `g-revocation`.**

`features/g-revocation.feature:59-63`:

```gherkin
    Scenario: The new version seals to every survivor and never to the revoked
      Given two agents holding lines on circle folder "projets"
      When the owner revokes the first agent with rotation
      Then the folder's header gains a version without the revoked line
      And the survivor opens the new version with its unchanged keypair
```

`rust/crates/aithos-bundle/tests/cucumber.rs:15688-15709`, the whole assertion
half of the step:

```rust
    let header: Header = read_json(w.bundle.as_ref().unwrap(), &file);
    assert!(header.key_versions.contains_key("2"));
    let v2 = &header.key_versions["2"];
    assert!(
        !v2.lines.iter().any(|l| l.kid == kid_of(AGENT)),
        "revoked line present"
    );
```

Three defects, and they are three different classes at once:

1. **Routing hint (`CHDR-019`).** "never to the revoked" — a capability claim —
   is decided by `l.kid == kid_of(AGENT)`. Per `spec/03-headers.md:55-57` the
   revoked MAY try the remaining lines; the step never makes it try one.
   Compare `c-headers` after lot A, which now decides the same claim by an
   attempted open.
2. **"every" with no referent (`CHDR-013`/`-014`).** The fixture,
   `cucumber.rs:15267-15274` (`two_agents_lines`), grants exactly two agents.
   After the revocation the surviving set has cardinality **one**. "seals to
   **every** survivor" is exercised against a single survivor, and no assertion
   reads `v2.lines.len()` or the position of any line. This is `CHDR-013`
   verbatim, one feature over.
3. **`check_rotation` is not called** — the gap `CHDR-024` names for
   `c-headers`, and `g-revocation` is the feature whose Gherkin most plainly
   claims the rotation rule.

**A fourth, adjacent, in the same Rule.** `cucumber.rs:15722-15751`,
`#[then("the wrap is bound to the node and its new key version")]`, asserts
`wrap.node == node.to_string()` and `wrap.key_version == 2` — two **declared
fields**, not a binding. A wrap whose AAD ignored node and key version entirely
would carry both fields and pass. `c-headers`' own domain skill states the rule
being broken here, `features/.agents/c-headers/corrector/correct-c-headers/SKILL.md`:
*"Prove a binding by an AAD mismatch rejecting, not by a key inequality."* This
is `CHDR-026`'s class ("no negative of the wrap by divergent AAD exists
anywhere") landing in `g-revocation`.

**Verdict.** `g-revocation` — `TARGETED`, and the heaviest single target in this
report after §5. It extends `chdr-i3-g-revocation`, which is about *what* the
rotation must re-seal; this is about *how the scenario decides* that it did.

**Hygiene, same file, one line.** `cucumber.rs:15600-15601` is a dead step
definition with an empty body:
`#[when("the head agent forges a heartbeat with its own key for G")] fn _unused_g(_w: &mut ProtocolWorld) {}`.
Search: `grep -rn "forges a heartbeat with its own key" features/*.feature`
returns `f-gamma.feature:102`, whose phrase has **no** trailing `" for G"`, and
whose real handler is `cucumber.rs:13343`. So no scenario reaches the empty
step. Delete it; it is the shape `PROCESS.md` § *Current scope* names first
("empty, generic, or proxy steps") and its presence is a live invitation.

---

## 5. Contagion search S4 — `Then`s that round-trip on themselves, and one global verdict for many cases

This is the section that changes the shape of the queue.

### 5.1 The narrow search first

**The class.** `CHDR-021`: a scenario that "seals a constant under a constant
and reopens it two steps later under the same constant", establishing only
`f_open(f_seal(x)) == x`. Generalised: an assertion whose expected value is
recomputed by the same production function it is testing.

**Search.** Scope: `cucumber.rs` and every `rust/crates/*/tests/*.rs`. Layer:
assertions. Method: every `assert_eq!` whose argument text calls the same
non-trivial function more than once.

Nine hits. Seven are benign and I name them so the work is not repeated:
`h1_merkle.rs:71` recomputes `h_node` but the line immediately above,
`h1_merkle.rs:70`, pins the same value byte-for-byte against the vector, so the
recomputation is an *additional* structural assertion, not the only one;
`cb2_operation_receipts.rs:214`, `cb5_catalog_contracts.rs:108`,
`g3_move.rs:76,183`, `i1_concurrency.rs:105` and `cucumber.rs:18339` compare
distinct objects.

**One is a clean instance**, `rust/crates/aithos-core/tests/cb6_semantic_replay.rs:47-81`,
`cb6_append_and_cold_replay_share_one_prefix_sensitive_front_door`. The "append"
arm and the "cold" arm are two instances of the **same** `GammaReplayState`,
constructed from the same `did` and `certificates`, fed the **same** `entries`
vector in the **same** order:

```rust
    let mut append = GammaReplayState::new(did.clone(), certificates.clone());
    for entry in &entries { append.admit(entry)…; }
    append.finish()…;
    let mut cold = GammaReplayState::new(did, certificates);
    for entry in &entries { cold.admit(entry)…; }
    cold.finish()…;
    assert_eq!(append.head()…, cold.head()…);
    assert_eq!(append.counters(), cold.counters());
```

The test's name claims append-time and cold-time "share one front door". What
it exercises is one code path, twice. It cannot distinguish a shared front door
from two divergent ones, because the second front door is never instantiated.
Neither `head()` nor `counters()` is pinned against the vector it loads
(`vectors/cb2-bundle-version-coexistence.json`) — checked: the vector's
`positive` object has keys `certificate_names, chains, delegated_entry_ids,
expected, gamma_jsonl, gamma_path, gamma_sha256`, and the test reads only
`gamma_jsonl` and `certificate_names`.

Its Gherkin twin is `features/f-gamma.feature:717-728`, Rule *"Append-time and
cold-time share one replay front door"*, `Then the verdict, accepted prefix and
counters are identical` — which brings us to §5.2, because that `Then` does not
do what the vector test does. It does much less.

**Measured, `MI-3`.** With `admit` swallowing the result of `verify_semantics`
and accepting unconditionally — i.e. *every* semantic check in the Gamma replay
front door disabled — **`ev-887c6f1b`: `cb6_semantic_replay` 1 passed / 2
failed.** The two casualties are
`cb6_mixed_profile_chain_fails_at_the_candidate_prefix` and
`cb6_rejection_does_not_advance_prefix_or_counters`. The test named for the
front-door property,
`cb6_append_and_cold_replay_share_one_prefix_sensitive_front_door`, is **the one
that passes**. It survives the total removal of the thing it is named after,
which is the strongest possible confirmation that it measures determinism and
not agreement between two doors. Structural reading and transcript agree.

### 5.2 The finding: 360 Gherkin phrases decided by five process-global verdicts

`rust/crates/aithos-bundle/tests/cucumber.rs:1119-1129` declares eight
process-lifetime `OnceLock` acceptance verdicts:

```rust
static CB4_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
…
static CB5_CATALOG_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB6_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB7_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
static CB10_ACCEPTANCE: OnceLock<Result<(), String>> = OnceLock::new();
```

Five are live (three `CB5_*` carry `#[allow(dead_code)]`). Each is read through
a two-line pair, e.g. `cucumber.rs:7333-7339`:

```rust
fn cb6_result(w: &mut ProtocolWorld) {
    w.cb6_result = Some(CB6_ACCEPTANCE.get_or_init(cb6_acceptance).clone());
}
fn cb6_assert_green(w: &ProtocolWorld) {
    assert_eq!(w.cb6_result, Some(Ok(())));
}
```

**Search.** Scope: every step definition in `cucumber.rs` — all 20 040 lines,
`#[given]`, `#[when]` and `#[then]`. Layer: Gherkin step definitions. Method:
brace-matched body extraction; a step is flagged when its **entire** body is a
call to `cb<n>_result` and/or `cb<n>_assert_green`. Each flagged step's regex
alternatives were then `fullmatch`ed, one by one, against every
`Given/When/Then/And/But` line of every `features/*.feature`.

**Result: 19 step functions, 360 regex alternatives, 360 matched Gherkin
lines.** Body lengths are 2 to 4 lines. Distribution:

| Feature file | Gherkin lines routed to a shared `OnceLock` verdict |
|---|---:|
| `f-gamma.feature` | 199 |
| `o-connector-classes-vault.feature` | 90 |
| `e-mandates.feature` | 19 |
| `e-mandate-sections.feature` | 13 |
| `f-plus-constraints.feature` | 11 |
| `l-delegated-writes.feature` | 9 |
| `g-plus-obligations.feature` | 8 |
| `h2-gamma-roots.feature` | 8 |
| `g-revocation.feature` | 3 |
| **`a-identity`, `b-derivation`, `c-headers`** | **0** |

The largest single one, `cucumber.rs:8538`, `cb4_positive_contract_fixture`,
swallows **65** distinct `Given` phrases and its whole body is:

```rust
fn cb4_positive_contract_fixture(w: &mut ProtocolWorld) {
    cb4_result(w);
    cb6_result(w);
}
```

Its `When` counterpart (`:9305`) and four `Then` counterparts (`:9314`, `:9322`,
`:9330`, `:9338`) each read `cb4_assert_green(w); cb6_assert_green(w);` and
nothing else.

**Why this is the report's centre of gravity.**

1. **It is `PROXY` by `PROCESS.md`'s own definition**, § *Evidence statuses*:
   *"The scenario consumes a shared verdict without executing its own case."*
   Not a related shape — the definition.
2. **The scenario's parameters reach nothing.** The regexes swallow `".*"` and
   the functions take no parameter. So a `Scenario Outline` with six `Examples`
   rows runs six scenarios that differ in no executed byte. `f-gamma.feature:719-728`
   is exactly that: six cases, one verdict, six greens.
3. **The verdict is computed once per process.** `get_or_init` means the first
   scenario to touch `CB4_ACCEPTANCE` computes it and every later scenario reads
   the cached value. `PROCESS.md:203-204` names this hazard by name — the
   integration pass must cover *"mutable global state, `OnceLock`/cache
   behavior"*. Here it is not a leak; it is the design.
4. **It inflates the counter the train treats as binding.** `LEDGER.md:44-51`
   makes printed counters as binding as the exit code, and lot A's own handoff
   records `full cucumber ev-a1fa00fc 18/114/836/3577`. At least 360 of those
   3577 steps are one bit of information repeated. The gate is not wrong; its
   resolution is 360 times coarser than its counter suggests.
5. **It is the same defect lot A just spent a cycle removing from `c-headers`**,
   at roughly forty-five times the scale, and it sits in the two features the
   queue reaches next after `g4-client-surfaces` and `d-bundle`.

**360 is a floor, not a ceiling.** The extraction only flagged steps whose
*entire* body is the shared-verdict call. At least four further sites
(`cucumber.rs:8957`, `:9860`, `:11610`, `:11666`) call `cb5_assert_green`
**inside** a larger body that also does real work; those are not counted and are
not necessarily defective.

### 5.2bis Measured. The finding is no longer structural

I set the refutation condition before seeing any result: *"Green with identical
counters ⇒ §5 is confirmed. Red ⇒ my regex mapping is wrong and §5 must be
withdrawn."* Three transcripts came back.

**Baseline — `ev-3d485476`**, `@f-gamma` at `dae12ab`, unmutated: green,
**1 feature / 12 rules / 204 scenarios / 891 steps**. And **`ev-3f2994ef`**,
`@o-connector-classes-vault`: green, **1 / 4 / 58 / 249**. These are the
denominators §5.2 was missing.

**`MI-1` — `ev-dd652e01`. The `Examples` row `| exhausted action counter |` at
`features/f-gamma.feature:730` replaced by `| a case that exists nowhere |`:
GREEN, 204 scenarios / 891 steps, counters identical to baseline.**

A `Scenario Outline` case naming a case the repository has never heard of runs,
passes, and changes no counter. The parameter reaches nothing. **Confirmed, not
argued.** The regex mapping in §5.2 was right, and cucumber-rs does resolve
those lines to the catch-alls.

**`MI-2` — one enumerated semantic defect accepted.** The `grant_logged` check
in `GammaReplayState::verify_semantics` neutered, so a delegated entry whose
minting grant was never logged is admitted. **`ev-db029aa9`: `@f-gamma` GREEN,
204/204.** **`ev-bc687fb8`: `cb6_semantic_replay` GREEN.** Both surfaces — the
Gherkin gate and the conformance-vector test — are blind to it.

**`MI-3` — the granularity number, and it is the one to quote.** Semantic
verification disabled *entirely* in the replay front door. **`ev-0002cc6b`:
`@f-gamma` 203 passed / 1 failed.** The single casualty is
`✘ Then the beacon is rejected`.

> Turning off every semantic check in the Gamma replay front door costs
> **one scenario out of 204**.

Put the three together and the shape is exact: a specific wrong case is invisible
(`MI-2`), a nonsense case is invisible (`MI-1`), and removing the entire
acceptance surface is visible once (`MI-3`). The 204-scenario feature defends its
acceptance surface, in aggregate, with roughly one scenario.

**What is still structural rather than measured.** The 360 figure itself, the
per-feature distribution in the table above, and the claim about
`o-connector-classes-vault`'s 90 lines. `MI-1` proves the mechanism on one
`f-gamma` outline; it does not individually prove the other 359 lines. I did not
ask for 360 mutants and would not. The mechanism is now demonstrated and the
inventory remains a search result.

**360 is still a floor.** The extraction only flagged steps whose *entire* body
is the shared-verdict call. At least four further sites (`cucumber.rs:8957`,
`:9860`, `:11610`, `:11666`) consume a shared verdict inside a larger body that
also does real work; those are not counted and may be entirely legitimate.

### 5.3 What this does **not** touch

Zero lines of `a-identity.feature`, `b-derivation.feature` and
`c-headers.feature` route to a shared `OnceLock` verdict. That is the fact that
keeps this out of `FULL_AUDIT` territory, and §7.0 argues it.

### 5.4 What it does touch, and this is new after the transcripts

`MI-3` also prices the **global** gate, which no section of my first pass did.

`f-gamma` is 204 of the 836 scenarios in the unfiltered Cucumber suite — 24% of
it. `LEDGER.md:44-51` makes printed counters as binding as the exit code, and
every cycle in this train, including `c-headers` lot A, files an unfiltered
Cucumber run as its final regression evidence (lot A: `ev-a1fa00fc`,
18/114/836/3577). `ev-0002cc6b` shows that a regression removing the entire
semantic-acceptance surface of the Gamma replay front door moves that global
counter by **one scenario**.

The gate is not wrong and no past verdict is invalidated by this — `c-headers`'
eight `VERIFIED` rest on `c-headers` scenarios, conformance vectors and sixteen
named mutants, none of which pass through `CB4_ACCEPTANCE` or `CB6_ACCEPTANCE`.
What is now measured is that **the global Cucumber counter is a weaker backstop
than its magnitude suggests for any future change touching Gamma replay**, and
the train reads that counter as binding. That is a finding against the evidence
model rather than against a feature, it is the same genus as `CHDR-040`, and it
routes with it in §9.

**And `MI-7` instantiates it exactly, harder than the `f-gamma` version does.**
`cb10_acceptance()` (`cucumber.rs:6570`) short-circuited to `Ok(())` — the
closed-matrix drift guard, the owner genesis, the succession, every case the
function verifies, skipped:

- **`ev-c0d4a435`**: `@o-connector-classes-vault` **GREEN, 58/58 scenarios,
  249/249 steps**.
- **`ev-f818dc4b`**: the **entire** Cucumber suite **GREEN — 18 features / 114
  rules / 836 scenarios / 3577 steps, all passed**.

Those counters are **identical to `ev-a1fa00fc`**, the unfiltered run lot A filed
as its own final regression evidence. **The global gate cannot distinguish a tree
in which the CB10 acceptance oracle checks nothing from the tree lot A shipped.**
That is §5.4's claim with a transcript instead of an inference, and it is the
sharpest single fact in this report.

Lot A's verdicts survive it for the same reason as before — `c-headers` routes
zero lines through `CB10_ACCEPTANCE`. What is damaged is the **reusability of the
global counter as evidence**, not any verdict already recorded against it.

### 5.5 What `MI-7` measures, and the two things it does not

`MI-7` is easy to over-read. Three statements, kept apart.

**What it establishes.** The CB10 acceptance oracle has **no witness**. It is the
sole guardian of the case families it verifies, and **nothing in 836 scenarios
cross-checks it**. Weaken it, delete a case from it, or let it silently stop
covering one, and no gate in the repository reports anything. That is a class
distinct from `PROXY` — call it an **unwitnessed oracle** — and it is worse than
a proxy step, because a proxy at least consumes a verdict that is itself checked
by something.

**What it does not establish, first: coverage of all 90 lines.** `MI-7` mutated
`CB10_ACCEPTANCE` alone, and the 90 `o-connector-classes-vault` lines do not all
consume it. Measured, same method and scope as §5.2:

| Verdict consumed | o-vault lines | via |
|---|---:|---|
| `CB10_ACCEPTANCE` | **41** | `cb10_given`, `cb10_when`, `cb10_then` |
| `CB5_CATALOG`+`CB6`+`CB7` | 29 | `o_catalog_overlay_fixture` / `_action` / `_verdict` |
| `CB5_CATALOG` | 20 | `cb5_catalog_then` |

**`MI-7` covers 41 of the 90.** The other 49 route through three verdicts the
mutant never touched; their detection depth is **unmeasured**.

**What it does not establish, second — and this decides the ordering question:
it is not the same kind of measurement as `MI-3`.**

| | Mutated | Layer | Question it answers | Result |
|---|---|---|---|---|
| `MI-3` | `GammaReplayState::admit` / `verify_semantics` | **production** — `aithos-core/src/gamma_replay.rs` | if *production* breaks totally, how many scenarios notice? | **1 of 204** |
| `MI-7` | `cb10_acceptance()` | **test oracle** — `cucumber.rs:6570` | if the *oracle* stops checking, how many scenarios notice? | **0 of 836** |

Different axes; the numbers are not on one scale. `MI-7`'s zero is in part
structurally forced — the only consumers of that oracle are the steps asserting
its cached verdict, so disabling it *must* make them pass. Its value is not the
zero. Its value is the proof that no independent witness exists.

And `MI-7` does **not** show that a production regression in structure/vault goes
undetected. A production break would presumably make `cb10_acceptance()` return
`Err`, flipping all 41 lines at once — coarse, one bit for 41 lines, but not
zero. **The number comparable to `MI-3`'s 1-of-204 did not exist for
`o-connector-classes-vault`** when I wrote that. `MI-8` produced it.

### 5.6 `MI-8` — the production number, and it retires my objection

**The mutant, and it is production.** `AithosVault::exact_config_authority`
(`rust/crates/aithos-bundle/src/vault.rs:75-96`), perimeter check deleted.
`verify_current_grantee` still runs, so the mandate chain is still verified; what
is removed is the requirement that the chain carry `act.x.<connector>.config`.

| Gate | Evidence | Result |
|---|---|---|
| `@o-connector-classes-vault` | `ev-dd761130` | **GREEN, 58/58 scenarios, 249 steps** |
| Full Cucumber suite | `ev-ce3f49ad` | **GREEN, 18 / 114 / 836 / 3577** |
| Full workspace, `--no-fail-fast` | `ev-a82b15ab` | **one** casualty; Cucumber counters in the same run **836/836, untouched** |

**Zero of 58. Zero of 836.** This is the axis I said was missing, measured on the
layer I said it had to be measured on. My objection to reordering was that an
oracle number and a production number are not the same measurement; that
objection is retired by measurement, not by argument, which is the only way I
wanted it retired.

**The single casualty is not what it looks like, and this is the honest half.**
The orchestrator ran the workspace precisely so "nothing anywhere" would not be
an unsearched absence claim. The search returned one hit. I read it. It fails at
`rust/crates/aithos-bundle/tests/cb2_bundle_structure_vault.rs:299`:

```rust
assert!(VAULT_SOURCE.contains("exact act.x.{connector}.config authority is required"));
```

and `VAULT_SOURCE` is `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vault.rs"))`
(`:55`). **The sole detector in the repository is a grep over the source text of
`vault.rs`.** It noticed a string literal vanish from a file. It did not
construct a grantee, did not exercise the authority path, and did not attempt an
unauthorised open.

So the claim is written this way, and this is the form that goes in the queue:

> **No behavioural test anywhere in this repository detects the deletion of the
> vault config perimeter check.** The Gherkin layer detects nothing — 0 of 58,
> 0 of 836. The one workspace casualty is a source-text assertion, not a
> behavioural one.

**The corollary, and it is why this outranks a coverage statistic.** A mutation
that *preserves* the error string while breaking the predicate — inverting the
`!` on `perimeter.iter().any(...)`, or relaxing `action == "config"` — would pass
**every gate in this repository, `cb2_bundle_structure_vault.rs:299` included**.
The only detector is defeated by leaving a string alone.

**A new class, and it is worse than `CHDR-011`.** `CHDR-011`, and
`cucumber.rs:13639` in §3.3, assert on a runtime error *message* rather than a
typed variant. This asserts on the *source file's bytes*. It is a test that
cannot fail for a behavioural reason at all. Call it a **source-text
assertion**.

**And I ran the search rather than listing it as owed, because it is a grep and
not a gate.** Scope: every `rust/crates/*/tests/**.rs`, `rust/target` excluded.
Layer: the test corpus.

```text
grep -rn 'include_str!' --include=*.rs rust/crates/*/tests/ | grep "/src/"
grep -rn "_SOURCE\.contains" --include=*.rs rust/crates/*/tests/ | wc -l
```

**At least 51 source-text assertions, across five files**, over sixteen
`include_str!`-of-`src` constants:

| File | Constants pulled from `src/` |
|---|---|
| `cb2_bundle_structure_vault.rs:50-55` | `bundle.rs`, `revoke.rs`, `state.rs`, `structure.rs`, `vault.rs` |
| `cb2_bundle_boundaries.rs:48-55` | `lib.rs`, `bundle.rs`, `log.rs`, `merge.rs`, `publication.rs`, `session.rs`, `vault.rs` |
| `cb2_bundle_authority_flows.rs:52-54` | `bundle.rs`, `grants.rs`, `log.rs` |
| `cb2_draft2_carriers.rs:70-72` | `manifest.rs`, `publication.rs` |
| `cb2_bundle_concurrency_final.rs:55-56` | `merge.rs`, `manifest.rs` |

51 is a **floor**: the tuple-loop form at
`cb2_bundle_structure_vault.rs:285-296` asserts through a loop variable and is
not counted by the grep.

**And the sixth site is the one that matters most, because it is inside the
Gherkin layer.** `rust/crates/aithos-bundle/tests/cucumber.rs:2053-2058`:

```rust
fn core_capability_api_is_narrow() -> bool {
    let source = include_str!("../src/session.rs");
    !source.contains("pub fn sign(")
        && !source.contains("pub fn open(")
        && !source.contains("pub fn wrap(")
}
```

A **scenario verdict** about the narrowness of the capability API is decided by
grepping `session.rs` for three strings. Rename `sign` to `sign_bytes` and the
claim passes while the API widens; add `pub fn sign(` inside a comment and it
fails while nothing changes. This is the same class as `CHDR-019` at one further
remove: there the assertion was decided by a routing hint instead of a seal, here
it is decided by source text instead of behaviour.

**What I am not claiming.** That all 51 are defects. A source-text assertion is
legitimate as an *inventory* check — "this public API still exists", which is
what `cb2_bundle_structure_vault.rs:286-296` mostly does — and illegitimate as a
*behavioural* one, which is what `:299` and `cucumber.rs:2053` are. I classified
two of the 52 by reading them. The remaining 50 are **counted, not classified**,
and §12 routes that triage rather than presuming its outcome.

**Closure criterion, and it is small.** One behavioural negative: a grantee
holding `act.x.gmail.reply` and no `config` action must fail to open `/x/gmail`
configuration through `open_vault_with_capability`. That is the guard's sole
caller — `grep -rn "exact_config_authority" --include=*.rs rust/` returns two
hits, the private declaration at `vault.rs:75` and the call at `vault.rs:110`.
Cost: nil, alpha.

---

## 6. Contagion searches S5-S7

### 6.1 S5 — degenerate fixtures, "every other" with no referent

**Search.** Scope: all 19 `features/*.feature`. Layer: Gherkin contract text.

```text
grep -rn -iE "every other|all other|the other (line|entry|section|node|member|version)s?|untouched|unchanged|byte-identical|leaves .* intact|no other" features/*.feature
```

26 hits. Each traced to its step definition. Most are legitimate:
`h-merkle.feature:92` (*"no other zone appears in the diff"*) is backed by
`cucumber.rs:17117-17127` which asserts `added.len() == 1` in the preceding step
and then `diff.keys().all(|k| k.starts_with("circle:"))` — a genuine universal
over a non-degenerate set. `i-concurrency`, `l-delegated-writes`,
`n-structural-mutations` and `o-connector-classes-vault` byte-identity claims
compare real stored bytes.

**One clean instance**, `e-mandate-sections.feature:107`:

```gherkin
      Then the helper's chain verifies
      And no other section is covered by the child
```

`cucumber.rs:10096-10117`, `cb3_child_covers_no_other_section`, decides "no
other section" against **one** hard-coded sibling:

```rust
    assert!(!covers_section_op(
        &perimeter,
        &SectionOp { verb: Verb::Read, zone, sid: cb3_section_sid("note2"), folders: &[], tags: &[] },
    ));
```

The universal is exercised at cardinality one, and the child's perimeter is
never asserted to have exactly one entry — `perimeter.first()` is read and the
rest ignored. That is `CHDR-013` and `CHDR-014` together. `TARGETED` on
`e-mandate-sections`.

The second instance is `g-revocation`'s "every survivor", already stated in
§4.

### 6.2 S6 — `Given`s that announce one state and construct another

**Search.** Scope: all 19 feature files, every `Given`/`And` line carrying an
explicit cardinal (`one|two|three|four|single`); each traced to its step
definition in `cucumber.rs`. 40 lines examined.

**Result: no instance found beyond the one lot A already fixed.** The two most
likely candidates were checked and are truthful: `g-revocation.feature:60`
*"two agents holding lines"* → `cucumber.rs:15267-15274` grants exactly two;
`g-revocation.feature:65` *"a zone holder reading folder by pure derivation"* →
`cucumber.rs:15276-15287`, which not only constructs what it says but carries a
**positive control inside the `Given`** (`assert!(w.read_at(…).is_ok())` at
`:15284`). That last one is worth recording as the repository's own
counter-example: the discipline `CHDR-025` demands already exists here, written
by someone, uncodified.

This is a negative result and I report it as one. It is the one class of the six
that did not spread.

### 6.3 S7 — a normative case declared by a vector with no consumer

**The specification first (R-1, R-3).** `spec/09-cli-and-conformance.md:37-53`,
verbatim, and quoted through the clause that scopes it:

> ## 9.2 Test vectors (normative at promotion)
>
> `vectors/` MUST cover, from a fixed `S`: […] Both success and every fail-closed case
> (unauthorized revocation, over-wide sub-mandate, N+1 action, expired heartbeat) get a
> vector. I3 gets its own family (§03.1): a header whose every key version carries the
> owner line → valid; a header one of whose key versions carries no owner line at all →
> the edition is rejected; a header whose line labelled `"owner"` is sealed to a key that
> is not the subject's `owner_kex` → rejected; a header whose owner line is not labelled
> `"owner"` but is sealed to `owner_kex` → valid, proving the label decides nothing in
> either direction. Each case states which verifier tier it binds: keyless (edition
> verification) or `owner_kex`-bearing.

Two clauses matter and both are conditional, which is why they are quoted rather
than cited. *"Each case states which verifier tier it binds"* is scoped by its
sentence position to the **I3 family**, not to every vector — I checked before
generalising it, and `vectors/c3-owner-line.json` does carry a `tier` field on
each of its five cases (`grep -l "tier" vectors/*.json` → that file alone).
So §9.2's tier clause is **satisfied**, and I record that rather than turn a
correct thing into a finding.

The other clause is `spec/09-cli-and-conformance.md:109`, closing §9.4:

> An implementation states which levels it claims; the vectors gate each.

That is the normative sentence CHDR-038 rests on, and it is a protocol
statement, not a code observation: a vector case that no consumer reads gates
nothing, whatever the code does.

**Search.** Scope: every `vectors/*.json` except `ownership.json`. Method: walk
every object; collect every key containing `must_fail`/`must_reject` and every
`negative`/`negatives` key; then check whether the key name occurs anywhere in
the concatenated text of every `.rs` file under `rust/` (`rust/target`
excluded). Layer: conformance vectors against their Rust consumers.

**508 such keys. 503 are consumed. Five are not:**

```text
vectors/f2-gamma-counting.json  /expected/next_action_via_root_must_fail    = "GammaBudgetExhausted"
vectors/f2-gamma-counting.json  /expected/next_action_via_leaf_must_fail    = "GammaBudgetExhausted"
vectors/f2-gamma-counting.json  /expected/second_child_of_root_must_fail    = "GammaBudgetExhausted"
vectors/f2-gamma-counting.json  /expected/action_under_ghost_chain_must_fail= "GammaGrantNotLogged"
vectors/f3-gamma-liveness.json  /forged_beacon_must_fail                    = "InvalidGammaEntry"
```

`g2-rotation.json`'s `missing_owner_must_fail` is **not** in that list — lot A
consumed it (`g2_rotation.rs:152-183`), which is `CHDR-009` closing and is the
proof the search is calibrated.

**The two cases are of different weight and I separate them.**

*The four `f2` keys.* `f2_gamma.rs:16-24` deserialises `expected` as an untyped
`serde_json::Value` and reads it only through
`fn expect_n(f: &Fixture, key: &str)` (`:63-65`), called with the string
literals `"actions_via_root"`, `"actions_via_leaf"`, `"children_of_root"` and
the three window keys. The four `*_must_fail` keys are never named. The
*behaviours* they declare **are** exercised — `f2_gamma.rs:142,147,152,166,186`
assert `Err(Error::GammaBudgetExhausted(_))` and `:200` asserts
`Err(Error::GammaGrantNotLogged(_))` — but against hardcoded Rust literals, not
against the vector. So the vector is not the oracle it claims to be: change the
declared variant in the JSON and no test moves. P3, real, narrow.

*The `f3` key is heavier.* `vectors/f3-gamma-liveness.json` declares
`forged_beacon_must_fail: "InvalidGammaEntry"`. The `F3` struct
(`f3_gamma.rs:16-25`) has no field for it, and serde ignores unknown members.
The only test exercising a forged beacon,
`f3_gamma.rs:108-128 a_forged_beacon_never_counts`, asserts a **different**
variant:

```rust
    assert!(matches!(
        heartbeat_ok(&log, &f.mandate, "2026-08-04T00:00:01Z", &f.doc),
        Err(Error::GammaHeartbeatStale(_))
    ));
```

`GammaHeartbeatStale` is a liveness verdict about the log; `InvalidGammaEntry`
is a rejection of the entry. The vector declares the second and nothing asserts
it. `grep -rn "InvalidGammaEntry" --include=*.rs rust/` returns 20 sites, all in
`log.rs`, `constraints.rs`, `gamma_replay.rs`, `gamma_v2.rs`, `receipts.rs` and
four `cucumber.rs` steps, none reached from a forged heartbeat fixture.

**What I claimed, and what `MI-5` settled.** Structurally, the vector's declared
normative case has no consumer and the nearest test asserts a different variant.
I declined to claim that production fails to reject a forged beacon, because I
had not established it and saying so would have been exactly the overreach
`CHDR-019` committed.

**`ev-826f8f15`** settles it, and settles it in production's favour. The probe
did not exist in the repository, so the orchestrator wrote one — the
`f3_gamma.rs` fixture plus one call to `gamma::verify_owner_entry` on the forged
beacon — ran it, journalled it, and deleted it uncommitted:

```text
MI-5 verify_owner_entry => Err(InvalidGammaEntry("gamma_0000000000000000000000000N: signature does not verify"))
MI-5 vector declares forged_beacon_must_fail = Some(String("InvalidGammaEntry"))
```

**Production rejects the forged beacon with exactly the variant the vector
declares.** So:

- there is **nothing to embargo**, and the forward flag I left in §6.4 closes
  clean;
- the finding is now **precisely** what I hoped it would be and no more: a
  normative case declared by a vector, correctly implemented, and read by no
  consumer anywhere. That is `CHDR-009`'s class exactly, with a transcript
  behind it rather than an analogy — and `CHDR-009` was closed by lot A by
  writing the missing consumer, which is the same fix and the same size;
- the restraint paid. Had I written "a forged beacon may not be rejected", I
  would have published a false security claim into a public repository. The
  cost of not writing it was one sentence and one named command.

**Closure criterion.** `f3_gamma.rs` reads `forged_beacon_must_fail` from the
vector and asserts `verify_owner_entry` returns that variant — the shape
`g2_rotation.rs:152-183` now has for `missing_owner_must_fail`. Cost: nil, alpha.

**And the generators, which is `CHDR-038` at repository scale.** `vectors/`
holds **29** `gen-*.py`. Searches, whole extract:

```text
cat .github/workflows/ci.yml          # 2 jobs, 8 steps, not one of them is Python
grep -rn "gen-\|python" .github/ scripts/    # cargo fmt --check only
```

CI runs `verify-feature-tags.sh`, `cargo fmt`, `cargo clippy`,
`cargo test --workspace`, and a wasm `cargo check`. **No gate in this repository
runs any vector generator.** Worse than CHDR-038 stated: of the 29 generators,
**nine have no `--check` mode at all** — `gen-f.py`, `gen-g.py`, `gen-h.py`,
`gen-h2.py`, `gen-i.py`, `gen-eplus.py`, `gen-fplus.py`, `gen-gplus.py`,
`gen-cb2-max-children.py` (`grep -c -- "--check"` per file → 0). For those nine
there is no verification mode to run even by hand; running them rewrites the
vector. That covers the `f-gamma`, `g-revocation`, `h-merkle`, `h2-gamma-roots`,
`i-concurrency`, `f-plus-constraints` and `g-plus-obligations` families — that
is, most of the queue.

### 6.4 The disclosure gate

I ran the check rather than assuming it did not apply, and I applied it to each
finding in this report separately.

- §5 (the 360 shared-verdict phrases) describes a **test-coverage** weakness, not
  an exploitable one. The underlying `cb4_acceptance`/`cb6_acceptance`/
  `cb10_acceptance` functions do run real checks over real cases; what is lost is
  per-scenario attribution, not a production guard. Nothing here tells an attacker
  anything about a deployed system, and nothing is deployed. `MI-2` and `MI-3`
  confirm the shape of the blindness without changing that assessment: an
  undetected *test* gap, not an undefended production path.
- §6.3's `f3` case is the one I looked hardest at, because "a forged beacon is
  not rejected" *would* be gateable. I declined to write that sentence, having
  not established it, and flagged forward that if `MI-5` showed production
  failing to reject, **that** result would need a condition-9 assessment before
  being written to any tracked file. **`ev-826f8f15` closes the flag:**
  production rejects it as `InvalidGammaEntry`, precisely as the vector declares.
  There was never an exploitable statement here — but that was not knowable when
  I wrote the section, and the order of operations is the point.
- §4's `g-revocation` routing-hint finding restates a class already published in
  full as `CHDR-019` at `docs/audits/features/c-headers.md`. Embargoing a
  restatement of a published finding would make a published finding look
  unpublished.
- §3.3's `MI-4b` result — `max_children` defended by one scenario and one unit
  test — was assessed and is **not** gateable. It names no unguarded production
  path: `ev-69cd5f74` shows the gate *is* implemented and *is* detected, once.
  A statement that a correct guard is thinly tested does not tell an attacker
  how to defeat it.
- **`MI-8` (§5.6) — assessed at length, and my ruling is publish in full.** This
  is the only finding in the review where I think the question was genuinely
  close, so the reasoning is recorded rather than the conclusion alone.

  *For publication.* The guard is **present and correct in shipped code**
  (`vault.rs:75-96`), so there is no weakness to fix and therefore nothing for
  the statement to be "before". The gate's own words protect a finding *"whose
  written statement would describe an exploitable weakness before a fix
  exists"*. Reaching the described state requires editing the victim's source
  and rebuilding, which presupposes the attacker already has more than the bug
  would give them. The repository is public: `vault.rs:75` and
  `cb2_bundle_structure_vault.rs:299` are already readable by anyone, and the
  inference that the latter is a source grep is one step from reading it.
  Nothing is deployed. And the precedent is settled in this train — `M9`,
  `M10`, `MI-3` and `CHDR-032`'s emission path all published in full.

  *The angle the orchestrator's assessment did not name, and which I checked
  before agreeing.* This finding says a specific class of malicious change —
  one that breaks the predicate while preserving the error string — would pass
  every gate in the repository. That is a statement about the **review
  apparatus**, and it is of more use to a supply-chain attacker than to a
  network one. I weighed it and still publish, for three reasons: gates are not
  the merge control, human review is, and this finding does not claim human
  review is absent; the fix is one behavioural negative and is cheaper than the
  embargo process that would delay it; and withholding it leaves the hole open
  for exactly as long as the embargo lasts, which is the worst available
  outcome. An embargo that protects a weakness by preventing its repair is not
  a disclosure control.

  *Conclusion.* Publishable in full, including the corollary about the
  string-preserving mutation, because the corollary is the part that makes the
  fix obviously necessary. I agree with the orchestrator's ruling and reached it
  independently. **No blocking condition 9.**
- `CHDR-028` is under embargo and is not restated here, in any form, including
  by paraphrase of its target.

**Nothing raised under blocking condition 9**, in either pass, and the single
open flag is now closed by transcript. Recorded with its reasoning, because "no
embargoed finding" is a claim like any other.

---

## 7. Per-feature verdicts

### 7.0 Why there is no `FULL_AUDIT`, argued rather than assumed

The round 1 impact review found none. That is a prior, not a precedent, and I
tested it against the strongest candidate this report produced — §5, 360 Gherkin
phrases decided by five process-global verdicts. The verdict is still no, and
here is the argument rather than the conclusion.

**`FULL_AUDIT` means one of two things in this train, and neither is met.**

1. *By the skill's stated criterion* (`review-gherkin-impacts/SKILL.md`):
   "a shared helper, API, format, or invariant changed". Lot A changed none —
   `git diff --stat 04860e2..dae12ab -- 'rust/crates/*/src/'` is empty (§1). Not
   met, and not arguably met.
2. *By the only reading under which it would be worth spending*: an **accepted**
   audit whose verdicts the new evidence puts in doubt. Three audits are
   accepted: `a-identity`, `b-derivation`, `c-headers`. Against each:
   - **None routes a single Gherkin line to a shared `OnceLock` verdict** —
     measured in §5.2's table, where all three read 0.
   - **The erratum search (§8 below) found no second wrong mutant** in any of
     the three.
   - The one genuine problem in an accepted audit — `a-identity`'s AID-001
     evidence, §8.2 — concerns a production surface **in another repository**
     and touches no Gherkin verdict of `a-identity.feature`, whose 30 scenarios
     run here and were verified here.

**And what `FULL_AUDIT` would actually buy.** Every feature carrying §5's
defect — `f-gamma`, `o-connector-classes-vault`, `e-mandates`,
`e-mandate-sections`, `f-plus-constraints`, `l-delegated-writes`,
`g-plus-obligations`, `h2-gamma-roots`, `g-revocation` — is **queued for an
initial audit it has not yet had**. Classifying an unaudited feature
`FULL_AUDIT` schedules nothing that is not already scheduled, and fires blocking
condition 10, which stops the train. That is a real cost paid for no
information.

**So the instrument is `TARGETED`, and the burden it must carry is different.**
A `TARGETED` follow-up for a queued feature is not a suggestion to look; it is a
named artefact the cycle owes. Every entry in §9 names the feature, the file,
the line, and what closes it. Where a `TARGETED` would be a placeholder, I have
written `NONE` instead.

**One reservation, stated before the transcripts and now resolved.** I wrote:
if `MI-1` and `MI-2` come back green, `f-gamma` is not merely owed scenarios —
its Rule and the ~199 lines around it are `SEMANTIC_FALSE_POSITIVE` before its
audit has started. They came back green. §7.0bis is the re-examination.

### 7.0bis Re-examining `f-gamma` with the numbers in hand

The orchestrator asked for this explicitly, and it is the right thing to ask,
because the numbers say something sharper than my first pass did.

**What the numbers say.** `ev-3d485476`: 204 scenarios. `ev-dd652e01`: a nonsense
`Examples` value changes nothing. `ev-db029aa9`: a specific admitted semantic
defect changes nothing. `ev-0002cc6b`: disabling **every** semantic check in the
Gamma replay front door costs **one scenario of 204**. And `ev-69cd5f74`: the
`max_children` budget, across the entire 836-scenario suite, is defended by one
scenario and one unit test.

**The verdict does not move, and here is the sentence the owner will ask for.**

> `FULL_AUDIT` is not a severity label. It is the instrument that fires blocking
> condition 10 and stops the train, and its purpose is to force a re-examination
> of work already accepted. `f-gamma` has no accepted work: it is queue position
> 16 and has never been audited. Declaring `FULL_AUDIT` on it would stop the
> train in order to schedule an audit that is already scheduled, while changing
> nothing about what that audit will find. The evidence is severe; the
> instrument is still wrong.

**And the three things that would have moved it, checked rather than assumed.**

1. *Does any accepted verdict rest on the surface `MI-1`-`MI-3` discredit?*
   No. `c-headers`, `a-identity` and `b-derivation` route **zero** Gherkin lines
   through `CB4_ACCEPTANCE`/`CB6_ACCEPTANCE`/`CB10_ACCEPTANCE` (§5.2). Lot A's
   eight `VERIFIED` rest on `c-headers` scenarios, conformance vectors and
   sixteen named mutants, none of which touch the shared verdicts. Checked, not
   assumed.
2. *Does it invalidate evidence a future cycle will rely on?* Partly, and that
   is a real cost I had not priced in the first pass. §5.4 states it: the global
   Cucumber counter is a weaker backstop than its magnitude suggests for any
   change touching Gamma replay, and `LEDGER.md:44-51` makes that counter
   binding. **That routes to the process, not to `f-gamma`** — it is the same
   genus as `CHDR-040`, and a `FULL_AUDIT` of `f-gamma` would not fix it.
3. *Would stopping the train now save work?* No. The next four features in
   `order:` are `g4-client-surfaces`, `d-bundle`, `h-merkle`, `e-mandates`, and
   none of them can be audited better by first re-auditing something never
   audited.

### 7.0ter `MI-8`: the threshold is crossed, and `o-connector-classes-vault` moves first

**I pre-registered this in §11 before the result existed:** *"0-2 failures of 58
means `o-vault` moves ahead of `f-gamma` in `order:`; more than that means the
`f-gamma`-first recommendation stands."* `ev-dd761130` reports **0 of 58**.

**So `o-connector-classes-vault` goes first, ahead of `f-gamma`.** I am not
looking for a reason it should not. A threshold that only binds when the result
is comfortable is not a threshold, and the entire value of registering it in
advance was to remove my discretion at exactly this moment.

**The evidence also argues it on the merits, which is a check on the
pre-registration rather than a substitute for it.**

| | production-mutant detection | Gherkin scenarios | shared-verdict lines | subject |
|---|---|---:|---:|---|
| `o-connector-classes-vault` | **0 of 58** (`ev-dd761130`) | 58 | 90 | vault config authority — credentials |
| `f-gamma` | 1 of 204 (`ev-0002cc6b`) | 204 | 199 | Gamma replay acceptance |

Three reasons beyond the raw comparison:

1. **Detection is zero on the authority axis, and the subject is authority.** The
   mutant let any mandate-holder open any connector's configuration and no
   scenario objected.
2. **The only detector is a source grep** (§5.6), so the mitigation I would
   otherwise have cited against reordering does not exist. Had `ev-a82b15ab`'s
   casualty been a behavioural test I would have argued the number was
   misleadingly harsh. It is not, and it is not.
3. **It is the cheaper cycle.** 58 scenarios against 204. Doing it first buys the
   larger severity reduction per unit of auditor time, which is what queue order
   is for.

`f-gamma` keeps its own re-positioning recommendation and its re-sizing; it moves
ahead of where it sits now (position 16 of 17), behind `o-connector-classes-vault`.
Placement of both remains the owner's.

**What I got wrong, and it is worth naming as method rather than as
housekeeping.** I called `MI-7` optional; it was not. I then declined to reorder
on `MI-7` — correctly, because it measured the wrong axis — and named the command
that would settle it. Both the refusal and the reversal came from the same rule:
say what result would change the conclusion, then let the result decide. The
refusal was not stubbornness and the reversal is not capitulation; they are the
same discipline applied twice.

---

**Does `MI-7` alone move `o-connector-classes-vault`? No — and this was written
before `MI-8` existed. It is kept because the reasoning is why `MI-8` was run.**

The orchestrator's framing is that `f-gamma` costs one scenario of 204 and
`o-connector-classes-vault` costs none of 836, so the deepest hole is the second
one. The arithmetic is right and the comparison is not, because the two numbers
answer different questions (§5.5): `MI-3` mutated **production**, `MI-7` mutated
a **test oracle**. Reordering the queue on "0 < 1" would be substituting one
measurement for another that looks like it — which is precisely the error
`CHDR-019` made, that `b-derivation.md:136` makes in miniature, and that I made
myself in `MI-4` (§3.3). Three times in one review is enough to notice the
pattern and stop doing it.

So I answer the question as asked, in two parts.

**Is line count the right ordering metric?** No — but neither is detection depth
on its own, and I will not pretend the second is available. What orders a queue
is *where an auditor's time buys the most*, and that has two components: how much
is undefended (severity) and how much must be rebuilt (work). For `f-gamma` I now
have both: **1 of 204** on a production mutant, 199 lines, 204 scenarios, 24% of
the suite, and a surface that bleeds into six other feature files. For
`o-connector-classes-vault` I have the work figure — 90 lines, 58 scenarios — and
**no severity figure at all**, because `MI-7` measured the oracle rather than the
code beneath it.

**So `f-gamma` keeps its recommendation and `o-connector-classes-vault` gets a
conditional one.** If `MI-8` returns a low number — say, a production mutant on
the structure/vault path costing 0-2 of 58 — then `o-connector-classes-vault`
should move ahead of `f-gamma`, because 50 declared cases with an unwitnessed
oracle *and* shallow production detection is a worse position than 199 lines with
detection of 1. I am saying in advance what result would change my recommendation,
so that the recommendation is falsifiable rather than adjustable. Until `MI-8`
runs, the ordering claim I am willing to defend is `f-gamma` only.

> **Resolved.** `MI-8` returned **0 of 58** (`ev-dd761130`). The condition above
> fired and §7.0ter carries the reordering. This paragraph is left standing
> because a pre-registration that is edited away once it pays out is worth
> nothing next time.

**What `MI-7` does change, and it is not the ordering.** It promotes §5.4 from
inference to measurement, it adds the *unwitnessed oracle* class, and it gives
`o-connector-classes-vault` a `TARGETED` artefact far more concrete than the one
it had: not "90 lines look like proxies" but "the oracle behind 41 of them can be
replaced by `Ok(())` with no gate in the repository noticing."

**What I do escalate, since severity has to land somewhere.** Three concrete
recommendations, none of which is a `FULL_AUDIT` and all of which the
orchestrator can act on without stopping:

- **Re-position `f-gamma` in `order:`.** It sits at position 16 of 17. It is now
  the largest measured hole in the repository — 204 scenarios, one of which
  defends the acceptance surface — and it is scheduled last. The queue's ordering
  was set before this was known. Moving it earlier costs nothing and is the one
  action the numbers actually argue for. I recommend it and leave the placement
  to the owner.
- **Re-size its budget.** `QUEUE.yaml:48` sizes `f-gamma` at 74 scenarios; the
  gate reports **204** (`ev-3d485476`). The budget note was written against the
  wrong denominator by a factor of 2.8, and that is before counting that ~199 of
  the 204 will need step definitions written, not merely audited. A cycle sized
  for 74 scenarios of auditing will hit the wallclock limit mid-panel, which is
  precisely the failure `QUEUE.yaml:48-52` was written to prevent.
- **Run `MI-8` before deciding `o-connector-classes-vault`'s position.** It is
  one mutant and it is the difference between an ordering recommendation I can
  defend and one I would be guessing at. Independently of the ordering, the
  *unwitnessed oracle* finding stands on `ev-f818dc4b` alone and needs nothing
  further: `vectors/cb2-bundle-structure-vault.json` declares **50** cases —
  verified by counting the arrays, 26 + 7 + 6 + 4 + 7 — and one test function is
  their only guardian.

**One thing I am explicitly not claiming.** That `f-gamma`'s production code is
wrong. `MI-3` shows the *tests* do not see the acceptance surface being removed;
`MI-5` shows production correctly rejecting a forged beacon with the exact
declared variant, and `MI-4` shows the grant-append success path exercised 155
scenarios over. The evidence is consistently that this feature is **implemented
and unproven**, which is the same diagnosis lot A carried for `c-headers` and the
same remedy: write the assertions, and prove each with a named mutant.

### 7.1 The table

| Feature | Verdict | Artefact owed |
|---|---|---|
| `f-gamma` | **`TARGETED`, heaviest — measured** | 199 Gherkin lines on `OnceLock` verdicts, mechanism proved by `ev-dd652e01`/`ev-db029aa9`; all semantic checks off costs 1 of 204 scenarios (`ev-0002cc6b`); `cb6_semantic_replay.rs:47-81` survives its own property being deleted (`ev-887c6f1b`); `max_children` defended by `f-gamma.feature:64` + `f2_gamma.rs:135-154` alone, the first asserting on a message substring (`ev-69cd5f74`); five unconsumed `must_fail` vector cases, `f3`'s correctly implemented (`ev-826f8f15`); `gen-f.py` has no `--check`. **Re-position and re-size: `QUEUE.yaml:48` says 74 scenarios, the gate reports 204** |
| `o-connector-classes-vault` | **`TARGETED`, deepest — goes first (§7.0ter)** | **Production detection is zero.** `MI-8` deleted the perimeter check in `AithosVault::exact_config_authority` (`vault.rs:75-96`): `ev-dd761130` **58/58 green**, `ev-ce3f49ad` **836/836 green**. The sole workspace casualty (`ev-a82b15ab`) is a **source-text grep**, `cb2_bundle_structure_vault.rs:299`, so **no behavioural test anywhere detects it**, and a mutation preserving the error string would pass every gate. **Unwitnessed oracle:** `cb10_acceptance()` (`cucumber.rs:6570`) stubbed to `Ok(())` leaves the whole suite green — `ev-c0d4a435`, `ev-f818dc4b`, **counters identical to lot A's own `ev-a1fa00fc`** — while being sole guardian of the 50 cases `vectors/cb2-bundle-structure-vault.json` declares (26+7+6+4+7, counted). 90 lines split **41 `CB10` / 29 `CB5`+`CB6`+`CB7` / 20 `CB5`**; `MI-7` covers 41, the other 49 unmeasured |
| `g-revocation` | **`TARGETED`** | `cucumber.rs:15688-15709` decides "never to the revoked" by `kid`, against `spec/03-headers.md:55-57`; "every survivor" at cardinality 1; no `check_rotation`; `cucumber.rs:15722-15751` proves a binding by field equality; 3 lines on `CB10_ACCEPTANCE`; dead step `cucumber.rs:15600` |
| `e-mandates` | **`TARGETED`** (was `NONE` in round 1, different axis) | 19 Gherkin lines on `CB4_ACCEPTANCE` via `cucumber.rs:8538`, `:9614` |
| `e-mandate-sections` | **`TARGETED`** (was `NONE` in round 1, different axis) | `cucumber.rs:10096-10117` — "no other section" at cardinality 1, perimeter length unasserted; 13 lines on `CB4_ACCEPTANCE` |
| `f-plus-constraints` | **`TARGETED`** (extends `chdr-i3-*` light entry) | 11 lines on `CB4_ACCEPTANCE`; `gen-fplus.py` has no `--check` |
| `g-plus-obligations` | **`TARGETED`** (was `NONE` in round 1, different axis) | 8 lines on `CB4_ACCEPTANCE`; `gen-gplus.py` has no `--check` |
| `h2-gamma-roots` | **`TARGETED`** (extends `chdr-i3-*` light entry) | 8 lines on `CB4_ACCEPTANCE`; `gen-h2.py` has no `--check` |
| `l-delegated-writes` | **`TARGETED`** (extends `chdr-i3-*` light entry) | 9 lines on `CB10_ACCEPTANCE` via `cucumber.rs:11703` |
| `h-merkle` | **`TARGETED`** (light, new) | `gen-h.py` has no `--check`; no gate runs it. The S5 search cleared `h-merkle.feature:92` explicitly |
| `i-concurrency` | **`TARGETED`** (light, new) | `gen-i.py` has no `--check`; no gate runs it |
| `a-identity` | **`TARGETED`** | round-2 no-write assertions passed in RED, no mutant (§2); AID-001 evidence unreproducible in this repository, dead link at `docs/audits/features/a-identity.md:301` (§8.2); `DOMAIN.md:80-83` multi-binary regression without `--no-fail-fast` (`CHDR-042` class) |
| `b-derivation` | **`TARGETED`** (light) | mutants named in prose not as patches; the review's own replay diverged from the corrector's on M3, 3/6 vs 4/6, attributed to *"instance de mutant différente"* (`docs/audits/features/b-derivation.md:413`) (§8.1) |
| `d-bundle` | `NONE` on this review's axes | Nothing found. `chdr-i3-d-bundle` and `bder-006-d-bundle` stand unchanged |
| `g4-client-surfaces` | `NONE` on this review's axes | Nothing found. `chdr-i3-g4-cli` stands unchanged |
| `n-structural-mutations` | `NONE` on this review's axes | Nothing found. `chdr-i3-n-structural` stands unchanged |
| `m-delegated-editions` | `NONE` on this review's axes | Nothing found. `chdr-i3-m-delegated` stands unchanged |
| `k-integration` | `NONE` on this review's axes | Nothing found. `chdr-i3-k-integration` stands unchanged |
| `c-headers` | `NONE` (self) | Lot A closed its instances of all six classes |

**On contradicting round 1.** Round 1 (`2026-08-04-c-headers-impact-review-v2.md`
§2.12) classified `e-mandates`, `e-mandate-sections`, `f-gamma` and
`g-plus-obligations` as `NONE`. I do not overturn that: round 1's axis was I3 /
`owner_kex` propagation, and on that axis `NONE` remains right. My axis is
defect-class contagion and it is a different question. Both verdicts stand,
scoped.

---

## 8. The `CHDR-019` erratum search across the other audits

**The question, as posed.** Do other audits in `docs/audits/features/` state
mutants or regressions that are wrong the same way — a mutant asserted in the
grammar of an in-API capability that the code cannot exhibit?

**The searches, their scope, their layer.** Scope: the whole of
`docs/audits/features/` — `README.md`, `a-identity.md`, `b-derivation.md`,
`c-headers.md`. There are no other public audits. Layer: audit prose.

```text
grep -rn -i "mutant\|mutation" docs/audits/features/
grep -rn -iE "régression survivante|reste vert|resterait vert|survivrait|survit à|ne tue" docs/audits/features/*.md
```

96 hits for the first (c-headers 72, b-derivation 22, a-identity 1, README 1);
13 for the second.

### 8.1 Result: no second wrong mutant, and one structural precondition that would hide one

**`a-identity.md` states no mutant at all.** Its single hit, `:426`, is the prose
phrase "post-signature mutation" naming a test case. There is nothing to be
wrong about — which is itself §2's finding, not this one's.

**`b-derivation.md`'s mutants are stated with measured kill counts**, e.g.
`:95-101`:

```text
M1 constant [0x42;32]        : b2_derivation FAIL,  9/815 scénarios BDD échouent
M3 hash monolithique         : b2_derivation FAIL, 71/815 scénarios BDD échouent
M5 étape XOR (unidirection.) : b2_derivation FAIL,  2/815 scénarios BDD échouent
```

That is the opposite of `CHDR-019`'s failure mode: `CHDR-019` attached a
**capability** claim to a mutant and never measured it. These carry numbers.

**But the precondition that made `CHDR-019` possible is present here too, and it
has already cost something measurable.** `b-derivation.md` names its mutants by
prose description — *"M3 hash monolithique"*, *"M4 31 octets de zone recopiés"* —
never as patches. `docs/audits/features/b-derivation.md:409-413` records what
that produced when the review tried to reproduce them:

| Mutant | Correcteur | **Rejoué par la revue** |
|---|---:|---:|
| M3 hash monolithique | 3 / 6 | **4 / 6** (instance de mutant différente) |

Two roles ran "the same" mutant and got different kill counts, and the audit's
own explanation is that they were not in fact the same mutant. That is the
`CHDR-019` structural hazard exactly: **a mutant stated in prose rather than as
an exact patch cannot be re-run, so its claimed kill count cannot be checked and
a wrong one cannot be detected.** The b-derivation review caught it, named
`M5b` as its own contribution and opened `BDER-012` for the residual — the
process worked. Nothing requires it to work next time.

Lot A's review fixed this locally: `2026-08-04-review-lot-a.md:44-68` is a
section titled *"The mutants, as exact patches"*. Nothing in `PROCESS.md` or in
either shared skill requires it. That is the same gap as §2, one layer up, and
it routes to the same artefact.

**And I supplied a first-person example while writing this report.** My `MI-4`
was stated in prose — *"`check_grant_append` → unconditional `Err`"* — and it
tested the opposite direction from the claim it was meant to settle (§3.3). Had
the orchestrator run it and stopped there, `ev-e7d1ca62`'s 155 failures would
have read as *"the gate is well covered"*, which is false; the gate is covered
once, and `ev-69cd5f74` is what shows it. A mutant is only as good as the
direction it is pointed in, and prose does not record direction. This is the
same defect as `b-derivation`'s M3 divergence and the same as `CHDR-019`,
committed by the role writing the section that complains about it. It belongs in
the record.

**One over-reach found, and I decline to call it an error.**
`docs/audits/features/b-derivation.md:136` and `:191-193` state that under M5
*"chaque frère se calcule depuis l'autre par un inconnu, les clés sont
maximalement liées, et le scénario reste vert"*. The second half is measured
(0 mutants killed of 5). The first half is a capability claim whose truth
depends on whether a stranger knows both sibling labels — an out-of-API premise
stated in the grammar of an in-API one, which is precisely what §2 of the lot A
review faults `CHDR-019` for. Unlike `CHDR-019`, the finding does not **rest**
on it: the measured half is independently sufficient, and the audit says so at
`:196` (*"pouvoir réel et mesuré (2 mutants sur 5)"*). I record it as a
rhetorical residual, not as a defect, and the closure criterion is one sentence,
not a re-audit.

### 8.2 What the search found instead, and it is worse than a wrong mutant

While reading `a-identity.md` for mutants I checked its gate evidence, and the
result belongs here because it is the same genus: an audit stating evidence that
cannot be reproduced.

`docs/audits/features/a-identity.md:159-183` records `AID-001`'s RED and GREEN as:

```text
cargo test -p aithos-provider --test cucumber
cargo test -p aithos-provider --test vectors_replay p9_cases_replay_wire_exact… --exact
cargo test -p aithos-provider --lib
python3 verify-p9.py
```

and `:301` links the changed surface as
`[aithos-provider/src/artifacts.rs](../../../rust/crates/aithos-provider/src/artifacts.rs)`.

**Searches, whole repository, tracked files only:**

```text
cat rust/Cargo.toml            # members: aithos-core, aithos-bundle, aithos-cli, aithos-owner, aithos-wasm
ls rust/crates/                # the same five. No aithos-provider.
ls vectors/ | grep -i p9       # empty
git ls-files | grep -i "p9"    # documentation only, no gen-p9.py, no verify-p9.py, no p9-store-reads.json
```

`aithos-provider` is **not in this repository**. It was split out on 2026-07-30
(`docs/CHANTIER-SPLIT-REPO-GATEWAY-SERVICE-2026-07-30.md`), the day **after**
the a-identity cycle closed. So:

- `AID-001` is `VERIFIED` on transcripts that name a test binary this repository
  does not contain;
- `docs/audits/features/a-identity.md:301` is a dead link;
- `PROCESS.md` § *Evidence hierarchy* rule 2 — *"the current executable code
  establishes the behavior that actually exists"* — has no referent for that
  verdict here, at any cost.

**Scope of the damage, stated precisely so it is not inflated.** The
`a-identity.feature` contract itself — 30 scenarios — runs in
`rust/crates/aithos-bundle/tests/cucumber.rs`, which is here, and `AID-002` and
`AID-005` were verified against it. Only the `AID-001` Provider remainder is
stranded. This is a `TARGETED` documentation and provenance repair, not a
reopening: the audit must either re-site `AID-001`'s evidence against surfaces
this repository contains, or state plainly that its proof lives in
`aithos-protocol/aithos-provider` at commit `e6fc5dc`, mark it as
cross-repository, and fix the link. Cost nil, alpha.

---

## 9. Routing the five new findings

| Finding | Scope, as the review states it | Route |
|---|---|---|
| `CHDR-037` — the marker lifecycle has no `IMPLEMENTED` state | `PROCESS.md:232-238`; repository-wide, since every feature's markers obey it | **Process artefact.** `features/.agents/PROCESS.md` § *Gherkin audit-marker lifecycle*. Not a feature's debt. Closes when a marker carries its status inline, or the section says the corrector rewrites the prose of markers it addresses. Merge with the `CHDR-040` amendment rather than making two edits to the same file |
| `CHDR-038` — no gate runs any of the 28 vector generators | Repository-wide by construction | **Repository artefact**, and this review widens it: 29 generators, and **nine have no `--check` mode at all** (§6.3). CI has no Python step (`.github/workflows/ci.yml`, 2 jobs, 8 steps, verified whole). Grounded normatively on `spec/09-cli-and-conformance.md:109`, *"the vectors gate each"*. Closes with a CI step or a feature-flagged `#[test]` running `--check` for the 20 that have one, **plus** an explicit recorded decision or a `--check` mode for the nine that do not. Owed by whichever cycle touches a vector first — currently `d-bundle` and `g-revocation` |
| `CHDR-039` — declared gates omit CI's clippy | Repository-wide | **Confirmed repository-wide by measurement.** `grep -n -i "clippy" features/.agents/*/DOMAIN.md` → **zero hits in all three existing `DOMAIN.md`**, while `.github/workflows/ci.yml:34` runs `cargo clippy --workspace --all-targets … -D warnings`. Fix all three, and fix the template the bootstrapper will copy for the other sixteen |
| `CHDR-040` — the process clauses this train enforces are not in `PROCESS.md` | P2, against the process itself | **Blocking-adjacent, orchestrator-owned; do not route to a feature.** The train's own §2 finding compounds it: the mutation protocol is likewise nowhere normative. I endorse the review's **Form B** — three sentences after the § *Artifacts* table — and add one clause to it: that a role facing a correction with no production defect names a mutant instead of a RED test. That single insertion closes `CHDR-040`, `CHDR-037` and §2 of this report at once |
| `CHDR-042` — the declared regression command hides failures after the first red binary | `c-headers/DOMAIN.md`; the review says the pattern is copied | **Confirmed for `a-identity`, cleared for `b-derivation`.** `a-identity/DOMAIN.md:80-83` carries `cargo test … -p aithos-core --test a1_genesis --test a2_did` — multi-binary, no `--no-fail-fast`. `b-derivation/DOMAIN.md:88` is single-binary and immune. All three carry `--no-fail-fast` correctly on the workspace line, so the flag is understood and simply absent from the tier that needs it. **And one level up:** `.github/workflows/ci.yml:37` runs `cargo test --workspace --manifest-path rust/Cargo.toml` with no `--no-fail-fast` either, so CI under-reports the same way |
| `CHDR-041` | reserved, not opened; condition has not fired | No route. Recorded so the identifier is not reused |

---

## 10. What I could not verify, and why

Nine transcripts closed four of the nine gaps my first pass declared. The four
are struck through with their evidence; the five that remain open are the honest
residue, and two of them are new.

**Closed by transcript.**

- ~~*Every behavioural claim is structural only.*~~ **Closed for §5 and §6.3**
  (`ev-dd652e01`, `ev-db029aa9`, `ev-bc687fb8`, `ev-887c6f1b`, `ev-0002cc6b`,
  `ev-826f8f15`) and **resolved against me for §3** (`ev-e7d1ca62`, `ev-69cd5f74`
  — see §3.3). It remains true for the items still listed below.
- ~~*Whether cucumber-rs resolves those lines to the catch-alls.*~~ **Closed by
  `ev-dd652e01`**: the nonsense `Examples` value ran green with identical
  counters, which is only possible if the catch-all is the resolving step. My
  regex mapping and cucumber-rs agree.
- ~~*The `f-gamma` scenario count against the budget.*~~ **Closed by
  `ev-3d485476`: 204 scenarios, 891 steps.** `QUEUE.yaml:48` says 74. The budget
  note is wrong by a factor of 2.8 and §7.0bis recommends re-sizing.
- ~~*The disclosure assessment of `MI-5`'s outcome.*~~ **Closed by
  `ev-826f8f15`.** Production rejects the forged beacon as `InvalidGammaEntry`,
  exactly as the vector declares. Nothing gateable, and the flag closed in the
  order it should have: raised before the result, assessed on the result.

**Still open, and I did not narrow them by running anything.**

1. **The 360 figure is a floor, and the *lines* remain a search result even where
   the *verdicts* are now measured.** This is the distinction `MI-7` makes it
   easy to blur, so it is stated flatly:
   - **Measured:** that the `CB6` and `CB10` verdicts have the detection depth
     §5.2bis and §5.5 report (`ev-0002cc6b`, `ev-f818dc4b`), and that the
     resolution mechanism is real on one `f-gamma` outline (`ev-dd652e01`).
   - **Not measured:** that any *particular* Gherkin line resolves to the step I
     mapped it to, except the single line `MI-1` mutated. The per-feature
     distribution in §5.2 and the 41/29/20 split in §5.5 are **`fullmatch`
     search results**, not transcripts. `MI-7`'s whole-suite green is consistent
     with the 41, and consistency is not proof.
   - **Not measured at all:** the 49 `o-connector-classes-vault` lines behind
     `CB5_CATALOG`, `CB6` and `CB7`, which `MI-7` never touched.
   Separately, my extraction only flagged steps whose *entire* body is the
   shared-verdict call; at least four further sites (`cucumber.rs:8957`,
   `:9860`, `:11610`, `:11666`) consume a shared verdict inside a larger body
   that also does real work and are unclassified.
2. **Whether the nine generators without `--check` ever produced their vectors
   reproducibly.** I established the absence of a `--check` mode by
   `grep -c -- "--check"` per file. I did not read the nine to see whether some
   other flag serves the purpose under a different name, and I did not run one.
3. **`a-identity`'s AID-001 in its own repository.** I established that
   `aithos-provider` is absent **here**. I have no access to
   `aithos-protocol/aithos-provider` and cannot say whether the correction
   stands there, whether commit `e6fc5dc` is reachable in it, or whether the
   crate still exists. The finding is that this repository's audit cannot be
   checked from this repository — not that the work was not done. No transcript
   can close this from inside this repository.
4. **Whether `PROCESS.md`'s missing sections are missing on purpose.** §9 routes
   `CHDR-040` on the reviewer's evidence and my own confirmation that the
   mutation protocol is likewise absent. Whether the owner intends
   `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md` to remain the
   normative home is an owner question and I did not presume an answer.
5. **Closed for `CB10` on both axes; still open for the other three verdicts.**
   - ~~*Oracle axis, `CB10`.*~~ **Closed by `ev-f818dc4b`** — no witness.
   - ~~*Production axis, `CB10`.*~~ **Closed by `ev-dd761130` / `ev-ce3f49ad` /
     `ev-a82b15ab`** — 0 of 58, 0 of 836, sole casualty a source grep.
   - **Still open, untouched on either axis:** `CB4_ACCEPTANCE`,
     `CB5_CATALOG_ACCEPTANCE` and `CB7_ACCEPTANCE`, which between them carry the
     49 `o-connector-classes-vault` lines `MI-8` and `MI-7` never reached, plus
     `f-gamma`'s `CB4` share and the `e-mandates` / `e-mandate-sections` /
     `f-plus-constraints` / `g-plus-obligations` / `h2-gamma-roots` lines.
   I decline to extrapolate two `CB10` results across five surfaces. `CB10` is
   now the only verdict in the repository whose depth is known.
6. **Whether `ev-69cd5f74`'s two casualties are the *only* defence of
   `max_children`, or merely the only ones the mutant reached.** An unconditional
   `Ok` from `check_grant_append` defeats the gate at one site; a budget could in
   principle also be enforced elsewhere by a path the mutant does not traverse. I
   did not search for a second enforcement site before writing "that is the whole
   defence", and the sentence should be read as "that is the whole defence *this
   mutant reached*". **Still open** — and note that for `MI-8` I did not repeat
   the omission: `grep -rn "exact_config_authority" --include=*.rs rust/` returns
   two hits, the private declaration at `vault.rs:75` and its single caller at
   `vault.rs:110`, so there is no second enforcement site there.
7. **New: the *source-text assertion* class (§5.6) is counted but not
   classified.** I ran the search rather than leaving it owed — **at least 51
   assertions across five files, plus `cucumber.rs:2053-2058` inside the Gherkin
   layer**. I then read exactly **two** of the 52 and judged them defective
   (`cb2_bundle_structure_vault.rs:299` and `cucumber.rs:2053`). The other 50 are
   **counted, not classified**: some are legitimate API-inventory checks and I
   have not separated them. Anyone reading the number as "50 more defects" is
   reading more than I measured.

**Classes I searched and found nothing, reported as negatives rather than
omitted:** `Given`s that announce one state and construct another (§6.2, 40
lines examined, zero instances beyond lot A's own); vector **files** with no
consumer (every `vectors/*.json` is referenced by at least one `.rs` except
`cb2-core-bundle-red-ledger.json`, which `vectors/README.md:74` declares a
bookkeeping ledger rather than an oracle, so it is not an instance).

---

## 11. The commands, what each decided, and the one I still want

All were run by the orchestrator, hashed and journalled on run `2026-08-04-r6`.
This role ran none of them. `MI-4b` is the orchestrator's addition and it is the
one that found the real finding.

**I read the transcripts rather than the summary.** `PROCESS.md` § *Correction
review* rule 5 treats another role's conclusion as *"a claim to verify, not
evidence"*, and that applies to a message relaying results as much as to a
corrector's report. Reading, not running, is within this role. Three checks
worth recording:

- **`MI-1` really mutated.** `diff ev-3d485476.txt ev-dd652e01.txt` is **two
  lines**: `✔ Given identical public facts for "exhausted action counter"` →
  `✔ Given identical public facts for "a case that exists nowhere"`, and a
  compile timing. The step is ticked. Had the transcripts been byte-identical I
  would have suspected the mutation never landed; they are not, and the one line
  that differs is the mutation itself, passing.
- **`MI-4b`'s casualties are exactly two.** `grep -n "test result: FAILED"
  ev-69cd5f74.txt` returns **one** line, and the named failure is
  `spent_budgets_fail_closed`, panicking at `f2_gamma.rs:150:5` on
  `check_grant_append`. Plus `✘ Then a third delegation is rejected` at
  transcript line 1195, in the 836-scenario Cucumber run. No third casualty
  anywhere in 5 145 lines. §3.3's sentence is exact — subject to §10's caveat 6
  that this is what the mutant *reached*.
- **`MI-5`'s probe was deleted.** `ev-826f8f15` shows it compiled as
  `crates/aithos-core/tests/mi5_probe.rs`; `git status` shows no such file. The
  orchestrator's statement that it was journalled and not committed checks out.

| Id | What it did | Evidence | Decided |
|---|---|---|---|
| `MI-0` | `@f-gamma` at `dae12ab`, unmutated | `ev-3d485476` | Baseline **204 scenarios / 891 steps**, 1 feature / 12 rules. Also closed §10's budget gap: `QUEUE.yaml:48` says 74 |
| `MI-6` | `@o-connector-classes-vault`, unmutated | `ev-3f2994ef` | Baseline **58 / 249**, 1 / 4 |
| `MI-1` | `f-gamma.feature:730`, `\| exhausted action counter \|` → `\| a case that exists nowhere \|` | `ev-dd652e01` | **GREEN, counters identical.** §5 confirmed by my own pre-registered refutation condition. The `Examples` parameter reaches nothing |
| `MI-2` | `grant_logged` check neutered in `GammaReplayState::verify_semantics` | `ev-db029aa9`, `ev-bc687fb8` | `@f-gamma` **GREEN 204/204** and `cb6_semantic_replay` **GREEN**. Both surfaces blind to an admitted unlogged-grant delegation |
| `MI-3` | `admit` swallows `verify_semantics`, accepts unconditionally | `ev-887c6f1b`, `ev-0002cc6b` | `cb6_semantic_replay` **1 passed / 2 failed** — and the survivor is the front-door test itself (§5.1). `@f-gamma` **203/204**, sole casualty `Then the beacon is rejected`. **The granularity number: the whole acceptance surface is worth one scenario** |
| `MI-4` | `check_grant_append` → unconditional `Err` | `ev-e7d1ca62` | **Refuted my framing.** 681/836, 155 scenarios fail: the success path is heavily exercised transitively. My mutant was pointed the wrong way (§3.3, §8.1) |
| `MI-4b` | *Orchestrator's addition.* `check_grant_append` → unconditional `Ok` | `ev-69cd5f74` (and `ev-5b25fc6f`, a disclosed compile error on the first attempt) | **835/836.** Two casualties: `f-gamma.feature:64` and `f2_gamma.rs:135-154`. The entire defence of `max_children`. This is where the finding actually lives |
| `MI-5` | Temporary probe, `gamma::verify_owner_entry` on the forged beacon; run, journalled, deleted, not committed | `ev-826f8f15` | Production returns **`Err(InvalidGammaEntry(…))`**, exactly the variant `vectors/f3-gamma-liveness.json` declares. **Nothing to embargo**; the finding is pure coverage, `CHDR-009`'s class, now with a transcript |
| `MI-7` | *I called this optional; it was not.* `cb10_acceptance()` (`cucumber.rs:6570`) short-circuited to `Ok(())` | `ev-c0d4a435`, `ev-f818dc4b` | `@o-connector-classes-vault` **58/58 green**; the **whole suite 836/836 green**, counters identical to lot A's `ev-a1fa00fc`. **Zero detection.** Establishes the *unwitnessed oracle* class and instantiates §5.4 with a transcript. Covers **41 of 90** lines, and measures the **oracle**, not production (§5.5) |

| `MI-8` | **Production.** `AithosVault::exact_config_authority` (`vault.rs:75-96`), perimeter check deleted; `verify_current_grantee` left intact | `ev-dd761130`, `ev-ce3f49ad`, `ev-a82b15ab` | `@o-connector-classes-vault` **0 failures of 58**; whole Cucumber suite **836/836 green**; workspace `--no-fail-fast` finds **one** casualty, and it is a **source-text grep** (`cb2_bundle_structure_vault.rs:299`), so **no behavioural test detects it**. **Threshold crossed — §7.0ter reorders the queue** |

**Nothing further to run.** Every question this review raised that a transcript
could answer has one. What remains open (§10) is open because it needs a *search
or a judgement*, not a gate — except the `CB4`/`CB5_CATALOG`/`CB7` depth figures,
which I am deliberately **not** requesting: `CB10` was worth measuring because it
decided a queue position, and those three decide nothing until the cycles that
own them open. Asking for three more mutants now would be manufacturing evidence
for a decision no one is taking.

**Two judgements of mine that transcripts corrected, kept together so the pattern
is visible.**

- **`MI-4`: pointed the wrong way.** I claimed something about the *gate* and
  proposed a mutant testing the *success path*. `ev-e7d1ca62`'s 155 failures
  would have read as "well covered" had the orchestrator stopped there. Fixed by
  `MI-4b`.
- **`MI-7`: wrongly called optional.** I reasoned that no verdict depended on it
  because `o-connector-classes-vault` was already `TARGETED` — true, and beside
  the point. The verdict did not change; the *evidence-model* finding did, and
  `ev-f818dc4b` is the only transcript here showing the global gate not moving at
  all. It then forced `MI-8`, which reordered the queue.

Both were caught by the orchestrator running something I had under-specified or
under-valued. Neither was caught by me re-reading my own reasoning, which is the
argument for the mutant protocol in §2 stated as evidence rather than as
principle.

---

## 12. Final `QUEUE.yaml` entries

**Final text, after evidence.** Restricted grammar only: no nesting, one level
of inline maps or lists, single-quoted scalars with no embedded apostrophes.
These extend `follow_ups:`; none replaces an existing entry, and each is named so
it cannot be confused with a `chdr-i3-*` key. Validated against
`features/.agents/scripts/train-status.py`'s own parser, merged onto the current
`QUEUE.yaml`: **50 `follow_ups` keys, 19 new, 0 grammar violations.**

**What the transcripts changed from the first draft**, so the diff is legible
rather than silent:

- `chdr-lota-proxy-detail` loses *"Unconfirmed until MI-1"* and gains the
  evidence ids and the granularity number.
- `chdr-lota-f-gamma` is rewritten: the `check_grant_append` clause was
  **wrong in direction** and is replaced by the `MI-4b` finding; the
  `forged_beacon` clause is corrected — production is right, only the consumer
  is missing.
- `chdr-lota-f-gamma-sizing` is **new**: 204 scenarios against a 74-scenario
  budget note, plus the re-positioning recommendation.
- `chdr-lota-global-gate-resolution` is **new**, and `MI-7` rewrote it: the
  headline is now `ev-f818dc4b`, not `ev-0002cc6b`.
- `chdr-lota-unwitnessed-oracle` is **new**, forced by `MI-7`. It is a class,
  not a feature debt, and it carries the search the other four verdicts owe.
- `chdr-lota-o-vault` is **new**, and `MI-8` rewrote it: it now leads with
  0-of-58 on a production mutant and the source-grep character of the sole
  casualty, not with the oracle result.
- `chdr-lota-order` is **new**, forced by `MI-8` crossing the pre-registered
  threshold. It is the only entry in this set that changes `order:`.
- `chdr-lota-source-text-assertions` is **new**, from a search I ran rather than
  requested. It carries an explicit scope limit: 52 counted, 2 classified.
- `chdr-lota-commands-owed` became `chdr-lota-evidence` and now records that
  **nothing further is owed**, plus which measurements I am deliberately not
  asking for and why.
- `chdr-lota-disclosure` is **new**: the condition-9 assessment, including the
  supply-chain angle that decided the close call.

```yaml
  # Recorded by the c-headers lot A impact review, 2026-08-04. No FULL_AUDIT.
  # Second pass, after nine transcripts on run 2026-08-04-r6. Full reasoning:
  # features/.agents/orchestrator/runs/2026-08-04-c-headers-lot-a-impact-review.md
  chdr-lota-proxy-verdicts: [f-gamma, o-connector-classes-vault, e-mandates, e-mandate-sections, f-plus-constraints, l-delegated-writes, g-plus-obligations, h2-gamma-roots, g-revocation]
  chdr-lota-proxy-detail: '360 Gherkin lines resolve to 19 step definitions whose whole body asserts one of five process-global OnceLock verdicts (cucumber.rs:1119-1129, 7295-7346). PROXY by the PROCESS.md definition. Distribution f-gamma 199, o-vault 90, e-mandates 19, e-mandate-sections 13, f-plus 11, l-delegated 9, g-plus 8, h2 8, g-revocation 3; zero in a-identity, b-derivation, c-headers. MECHANISM MEASURED: ev-dd652e01, an Examples row replaced by a case the repository has never heard of, green at identical counters. ev-db029aa9 and ev-bc687fb8, one admitted semantic defect invisible to both surfaces. ev-0002cc6b, all semantic checks off costs 1 scenario of 204. The 360-line inventory itself remains a search result and is a floor'
  chdr-lota-f-gamma: 'heaviest target, and now measured. Beyond the 199 proxy lines: cb6_semantic_replay.rs:47-81 builds one GammaReplayState twice and calls it two front doors, and ev-887c6f1b shows it is the test that SURVIVES deletion of the property it is named for. max_children is defended by exactly two artefacts in the whole repository, f-gamma.feature:64 and f2_gamma.rs:135-154 (ev-69cd5f74, 835/836 under an always-Ok check_grant_append), and the first of the two asserts on the substring budget in the error message rather than the typed variant, which is the CHDR-011 class. Four must_fail keys of f2-gamma-counting.json have no consumer. forged_beacon_must_fail of f3-gamma-liveness.json has no consumer either, but production is CORRECT: ev-826f8f15 returns InvalidGammaEntry, exactly as declared, so this is pure coverage, the CHDR-009 class, and the fix is the consumer that g2_rotation.rs:152-183 now has. gen-f.py has no --check mode'
  chdr-lota-f-gamma-sizing: 'scheduling, not a verdict. QUEUE.yaml sizes f-gamma at 74 scenarios; the gate reports 204 scenarios and 891 steps (ev-3d485476), so the budget note is wrong by a factor of 2.8, and roughly 199 of the 204 need step definitions written rather than merely audited. f-gamma also sits at position 16 of 17 while being the largest measured hole in the repository. Re-size the budget, and consider re-positioning it earlier in order. The owner decides the placement'
  chdr-lota-global-gate-resolution: 'against the evidence model, not against a feature, and the sharpest measured fact in the review. ev-f818dc4b: with cb10_acceptance() at cucumber.rs:6570 short-circuited to Ok(()), the entire Cucumber suite is green at 18 features, 114 rules, 836 scenarios, 3577 steps — counters IDENTICAL to ev-a1fa00fc, the unfiltered run lot A filed as its own final regression evidence. The global gate cannot distinguish a tree whose CB10 oracle checks nothing from the tree lot A shipped. ev-0002cc6b is the softer version on the Gamma side: all semantic checks off moves the global counter by one scenario. LEDGER.md:44-51 makes printed counters as binding as the exit code. No accepted verdict is invalidated — c-headers, a-identity and b-derivation route zero lines through the shared verdicts — but the reusability of the global counter as evidence is damaged. Same genus as CHDR-040, routes with it'
  chdr-lota-unwitnessed-oracle: 'new class, distinct from PROXY and worse: an oracle nothing cross-checks. cb10_acceptance() is the sole guardian of the 50 cases vectors/cb2-bundle-structure-vault.json declares (26 structural authority, 7 structural failure, 6 revocation failure, 4 vault crud, 7 vault access; counted), and ev-f818dc4b shows it can be replaced by Ok(()) with no gate anywhere reporting anything. A proxy step at least consumes a verdict that is itself checked. Search the other four live OnceLock verdicts for the same shape: CB4_ACCEPTANCE, CB5_CATALOG_ACCEPTANCE, CB6_ACCEPTANCE, CB7_ACCEPTANCE at cucumber.rs:1119-1129, none of them tested for a witness'
  chdr-lota-o-vault: 'deepest measured target in the repository, and it goes FIRST. MI-8 deleted the perimeter check in AithosVault::exact_config_authority (rust/crates/aithos-bundle/src/vault.rs:75-96), production not a test helper, leaving verify_current_grantee intact: ev-dd761130 @o-connector-classes-vault GREEN 58 of 58, ev-ce3f49ad whole Cucumber suite GREEN 836 of 836. ev-a82b15ab, workspace with --no-fail-fast, returns ONE casualty and it is a source-text grep at cb2_bundle_structure_vault.rs:299 over include_str! of src/vault.rs. So the correct claim is that NO BEHAVIOURAL TEST ANYWHERE detects it, and a mutation preserving the error string while inverting the predicate would pass every gate including that one. Separately MI-7 (ev-c0d4a435, ev-f818dc4b) shows cb10_acceptance() at cucumber.rs:6570 is an unwitnessed oracle. 90 shared-verdict lines split 41 CB10, 29 CB5+CB6+CB7, 20 CB5; the 49 remain unmeasured on both axes. Closure for the headline defect is one behavioural negative: a grantee holding act.x.gmail.reply and no config action must fail to open /x/gmail configuration through open_vault_with_capability, the guard sole caller at vault.rs:110'
  chdr-lota-order: 'ordering recommendation, owner decides placement. o-connector-classes-vault moves AHEAD of f-gamma, on a threshold the impact review pre-registered before MI-8 ran: 0-2 failures of 58 moves it, more leaves f-gamma first. Result was 0. Merits agree with the pre-registration: detection is zero on the authority axis and the subject IS authority; the only detector is a source grep so the mitigation that would argue against reordering does not exist; and at 58 scenarios against 204 it is the cheaper cycle, so it buys the larger severity reduction per unit of auditor time. f-gamma keeps its own re-positioning out of slot 16 of 17 and its re-sizing, behind o-connector-classes-vault'
  chdr-lota-source-text-assertions: 'new class, measured by search not by mutant, and worse than CHDR-011 because it cannot fail for a behavioural reason at all. AT LEAST 51 assertions of the form CONST.contains(literal) where CONST is include_str! of a src/ file, across five test files: cb2_bundle_structure_vault.rs:50-55, cb2_bundle_boundaries.rs:48-55, cb2_bundle_authority_flows.rs:52-54, cb2_draft2_carriers.rs:70-72, cb2_bundle_concurrency_final.rs:55-56. 51 is a floor, the tuple-loop form at cb2_bundle_structure_vault.rs:285-296 is not counted. The sixth site is inside the Gherkin layer and is the worst: cucumber.rs:2053-2058, core_capability_api_is_narrow(), decides a scenario verdict about capability API narrowness by grepping src/session.rs for pub fn sign(, pub fn open( and pub fn wrap(. IMPORTANT SCOPE LIMIT: only two of the 52 were read and judged defective, cb2_bundle_structure_vault.rs:299 and cucumber.rs:2053. The other 50 are counted, not classified, and some are legitimate API-inventory checks. What is owed is the triage, not 50 corrections'
  chdr-lota-g-revocation: 'cucumber.rs:15688-15709 decides never to the revoked by l.kid == kid_of(AGENT); spec/03-headers.md:55-57 says a reader that finds no matching line MAY try the remaining lines, so kid absence proves nothing. every survivor is exercised at cardinality one, no line count asserted, check_rotation never called. cucumber.rs:15722-15751 proves a wrap binding by field equality, against the rule written in correct-c-headers/SKILL.md. Dead empty step at cucumber.rs:15600, phrase reached by no scenario. Extends chdr-i3-g-revocation, does not replace it'
  chdr-lota-e-mandate-sections: 'cucumber.rs:10096-10117 decides no other section is covered by the child against one hard-coded sibling note2, and reads perimeter.first() without asserting the perimeter length. CHDR-013 and CHDR-014 class. Round 1 classified this feature NONE on the I3 axis; that verdict stands, this is a different axis'
  chdr-lota-vector-generators: 'CHDR-038 widened. vectors/ holds 29 gen-*.py, no CI step is Python (.github/workflows/ci.yml, 2 jobs, 8 steps, read whole), and nine have no --check mode at all: gen-f, gen-g, gen-h, gen-h2, gen-i, gen-eplus, gen-fplus, gen-gplus, gen-cb2-max-children. For those nine there is no verification mode to run even by hand. Normative ground spec/09-cli-and-conformance.md:109, the vectors gate each. Owed by the first cycle to touch a vector'
  chdr-lota-clippy-and-fail-fast: 'CHDR-039 and CHDR-042 confirmed repository-wide. No DOMAIN.md of the three names clippy while ci.yml:34 enforces it. a-identity/DOMAIN.md:80-83 is a multi-binary regression without --no-fail-fast; b-derivation/DOMAIN.md:88 is single-binary and immune. ci.yml:37 runs cargo test --workspace without --no-fail-fast too, so CI under-reports the same way. Fix the three files and the template the bootstrapper copies for the other sixteen'
  chdr-lota-mutation-protocol: 'the rule that a test-semantics correction proves itself by a named mutant is in no normative file. PROCESS.md correction step 2 says RED test when possible and is silent on the impossible case; shared/correct-gherkin-feature/SKILL.md execution steps 1-3 presuppose a defect on a production path. Sixteen features have no agent directory yet and will inherit the shared skill. Merge with the CHDR-040 amendment, one edit not two'
  chdr-lota-mutants-as-patches: 'a mutant named in prose cannot be re-run and cannot be pointed, so neither its kill count nor its direction can be checked. Two measured costs: docs/audits/features/b-derivation.md:413 records the review replaying M3 at 4/6 against the corrector 3/6, explained as a different mutant instance; and this impact review proposed MI-4 in prose, pointed it the wrong way, and needed the orchestrator to run the complement MI-4b before the finding appeared. Lot A published its mutants as exact patches (review-lot-a.md:44-68); nothing requires it. Same artefact as chdr-lota-mutation-protocol'
  chdr-lota-a-identity: 'TARGETED on a COMPLETE feature, not a reopening. Round 2 shipped assertions its own report says passed in RED (correction-02.md:67) with no mutant anywhere in the cycle. AID-001 is VERIFIED on gates naming aithos-provider, a crate absent from rust/Cargo.toml since the 2026-07-30 split, and docs/audits/features/a-identity.md:301 is a dead link into it. The 30 Gherkin scenarios run here and are unaffected. Repair the provenance or re-site the evidence. Cannot be closed from inside this repository'
  chdr-lota-b-derivation: 'TARGETED, light. Its M5 rationale (b-derivation.md:136, 191-193) states an out-of-API capability in the grammar of an in-API one, the CHDR-019 shape. The finding does not rest on it and the measured half is sufficient, so this is a one-sentence correction, not a re-audit'
  chdr-lota-evidence: 'fourteen transcripts on run 2026-08-04-r6 settle every behavioural claim; NOTHING FURTHER IS OWED. Baselines ev-3d485476 f-gamma 204/891 and ev-3f2994ef o-vault 58/249. Proxy confirmations ev-dd652e01, ev-db029aa9, ev-bc687fb8, ev-887c6f1b, ev-0002cc6b. Refutation of this review own MI-4 framing ev-e7d1ca62, complement ev-69cd5f74, disclosed compile error ev-5b25fc6f. Disclosure probe ev-826f8f15, nothing embargoed. MI-7 ev-c0d4a435 and ev-f818dc4b. MI-8 ev-dd761130, ev-ce3f49ad, ev-a82b15ab. The depth figures for CB4_ACCEPTANCE, CB5_CATALOG_ACCEPTANCE and CB7_ACCEPTANCE are deliberately NOT requested: CB10 was worth measuring because it decided a queue position, those three decide nothing until their cycles open'
  chdr-lota-disclosure: 'blocking condition 9 assessed in every pass and raised NOTHING. The close call was MI-8 and the ruling is publish in full, reached independently of the orchestrator and agreeing with it. Grounds: the guard is present and correct in shipped code at vault.rs:75-96 so there is no weakness to fix and nothing for the statement to be before; reaching the described state requires editing the victim source and rebuilding; the repository is public and both cited lines are already readable; nothing is deployed; and M9, M10, MI-3 and the CHDR-032 emission path were all published in full. The angle the review added: this finding is of more use to a supply-chain attacker than a network one, since it says a string-preserving predicate break would pass every gate. Published anyway because gates are not the merge control, the fix is one behavioural negative and is cheaper than the embargo that would delay it, and an embargo that protects a weakness by preventing its repair is not a disclosure control. CHDR-028 remains embargoed and is not restated anywhere in the report'
```

---

## 13. Limits of this conclusion, and next action

**This report is FROZEN at the third pass.** It performed dependency and
defect-class analysis. It changed no code, no audit, no feature file, no
`QUEUE.yaml`, and restarted nothing. **It ran no gate in any pass**; the eleven
transcripts it cites were run by the orchestrator, are the orchestrator's
evidence, and are journalled on run `2026-08-04-r6`. I read each of them rather
than the summary relaying them (§11).

Its `NONE` verdicts are scoped to the six classes searched and to the searches
quoted; they are not general clearances. Its `TARGETED` verdicts each name a
file and a line. Where a transcript corrected me — `MI-4`'s direction, and my
calling `MI-7` optional — the correction is recorded in place rather than
smoothed into agreement.

**One methodological note, since it recurred three times and is the review's own
lesson.** `CHDR-019` stated an out-of-API property in the grammar of an in-API
one. `b-derivation.md:136` does a smaller version of it. I did it in `MI-4`. And
the `f-gamma`-versus-`o-vault` comparison invites a fourth: a production mutant
and a test-oracle mutant produce numbers that look comparable and are not
(§5.5). The report declines that one, which is why `MI-8` exists.

**Next action, in order. Nothing on this list is a command for me.**

1. The orchestrator writes the §12 entries — it, not this role. **19 keys, parsed
   clean against `train-status.py`.**
2. The orchestrator applies **one** `order:` change: `o-connector-classes-vault`
   ahead of `f-gamma`, and `f-gamma` out of slot 16 of 17. Placement of both is
   the owner's; only the relative order is mine, and it is pre-registered.
3. `CHDR-040`, `CHDR-037` and §2's mutation-protocol gap want **one** amendment
   to `features/.agents/PROCESS.md`, and it is the owner's to write.

Nothing here blocks the train. Blocking condition 10 is not engaged, blocking
condition 9 was assessed in every pass and raised nothing, and **no accepted
verdict in this repository is put in doubt by anything measured here** —
`c-headers`, `a-identity` and `b-derivation` route zero Gherkin lines through any
shared `OnceLock` verdict, which was checked rather than assumed.

**The report is frozen.**

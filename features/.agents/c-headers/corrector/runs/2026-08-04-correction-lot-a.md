# Conclusion — correction lot A of `c-headers.feature`: test semantics

| Field | Value |
|---|---|
| Run type | correction, round 2, lot A, native (not `RECONSTRUCTED`) |
| Role | `correct-c-headers` (`corrector/correct-c-headers/SKILL.md`) |
| Date | 2026-08-04 |
| Orchestrator run journal | `../../../orchestrator/runs/2026-08-04-r6/` |
| Correction branch | `codex/fix-c-headers-lot-a` |
| Base `main` | `2f2d55d`; branch head at run start `04860e2` |
| Audited revision | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` |
| Reference gate on the base | `ev-9c60b798` — exit 0, 1 feature / 4 rules / 8 scenarios / 28 steps |
| Scope | `assigned_findings` minus `CHDR-016`: **eight** findings |
| Findings handled | `CHDR-001`, `CHDR-002`, `CHDR-009`, `CHDR-013`, `CHDR-014`, `CHDR-019`, `CHDR-021`, `CHDR-025` → **`IMPLEMENTED`** |
| Findings not handled | `CHDR-016` — removed from lot A by orchestrator decision (below). `CHDR-028` under embargo; `CHDR-029`, `CHDR-030` owned by other features |
| Result | `REVIEW_REQUESTED` |

## What this role did and did not do

**This role executed nothing.** No `cargo` command, no test, no build was run
here. Every result below was produced by the orchestrator on run
`2026-08-04-r6` and reported back with an `evidence_id`; no result is asserted
that does not carry one, and no `evidence_id` appears here that the
orchestrator did not send. Facts established by reading the tree (`git`,
`grep`) are marked as such. One formatter invocation — `rustfmt` on the three
edited Rust files — was run by this role; it is a formatter, not a gate, and
nothing is claimed from it. `cargo fmt --check` was gated independently
(`ev-e3b0c442`).

The Pass A / Pass B barrier of `PROCESS.md` is the auditor's evidence model.
This run does not claim it and is contaminated by mandate: it read the public
audit §6 and §11, `STATE.md`, `DOMAIN.md` and `features/AGENTS.md` before
touching anything.

## The methodological point that governs this lot

Lot A is **test semantics**. Production behaviour was already correct; what was
missing was proof. A strengthened assertion over correct production code is
therefore GREEN the moment it is written, and "a test that fails today" does
not exist for eight of the nine findings originally assigned.

The only honest RED here is a **named mutant**: a small, reversible edit under
which the *old* assertion is green and the *new* one is red. That is the audit's
own §11 "RED attendu" column, and it is what the orchestrator ran — twice per
mutant, once with the lot A changes stashed and once with them restored. The
first half of each pair is the load-bearing one: it is what proves the old
assertion was blind rather than merely weaker.

`CHDR-016` was the single exception — a genuine production defect with a genuine
RED — and it is precisely the one that left the lot.

## Per finding: the mutant, and RED before GREEN

Base greens before any mutation: `ev-27165515` (`c1_header_seal`),
`ev-ea8e1ac8` (`g2_rotation`, 7 tests), `ev-900a26c9` (feature gate,
1/4/8/28).

### `CHDR-001` — the version half of "bound to its node and version"

`replay_line_other_node` now records three attempts: a control open on the
line's own header, the existing graft onto `NODE_OTHER`, and a new graft of the
same v1 line into a **v2 of the same node**, opened at version 2. Subject, node
and kid are identical across that third attempt, so `line_aad`'s `key_version`
component is the sole discriminator.

Mutant: `key_version` removed from `line_aad` (`seal.rs`).

| | `evidence_id` | Result |
|---|---|---|
| without the patch | `ev-be6df11d` | **green 8/8** — the old assertion is blind |
| with the patch | `ev-eb765f2c` | **RED**, 7/8 scenarios, 27/28 steps, `✘ Then opening it there is rejected` — `attempt 2 after the mutation must be rejected, got Ok([119, 119, ...])` |
| base restored | `ev-e473df3b` | green 8/8 |

### `CHDR-002` — the rejection scenarios become differential

`corrupt_line` and `replay_line_other_node` each record a control open before
mutating; `opening_rejected` asserts the first recorded attempt is `Ok(DK)` and
every later one is `Err`, with the cardinal read.

Mutant: `owner_pub_c()` moved from `xsk(0x0A)` to `xsk(0x0B)` — the fixture's
owner line becomes permanently unopenable, a regression with nothing to do with
corruption or replay.

| | `evidence_id` | Result |
|---|---|---|
| without the patch | `ev-bf5be536` | 6/8 — scenarios 1 and 7 fail, and **scenarios 3 and 4 pass**. That is the finding, observed rather than argued |
| with the patch | `ev-6b0b6408` | 3/8 — scenarios 3 and 4 flip red on the positive control; scenario 8 flips too, an extra catch owed to the `CHDR-021` rebuild, whose new `Then` opens v1 as the owner |

### `CHDR-009` — the fail-closed side of three I3 gates

`vectors/g2-rotation.json:17` declared `missing_owner_must_fail` and the `G2`
struct did not deserialise it: the field had **no consumer anywhere in the
repository**, while its sibling `smuggled_must_fail` was honoured. Verified by
`grep` at run time: before this change, `Error::MissingOwnerLine` was asserted
by no test in the repository, in any crate. Three tests now bind three distinct
gates. The orchestrator expanded the campaign to one mutant per gate; each flips
exactly one test and leaves the other six passing.

| Gate | Mutant | without | with |
|---|---|---|---|
| `rotate` (via `check_owner_line`) | helper neutered | `ev-a770a551` green | `ev-c4b5812a` **RED** on `rotate_refuses_a_survivor_set_without_the_owner` |
| `check_rotation` | trailing owner gate deleted | `ev-7eb35ee6` green | `ev-73210dba` **RED**, `expected MissingOwnerLine, got Ok(())` |
| `validate` | keyless parse gate neutered | `ev-7c73867f` green | `ev-27c0c3a9` **RED** on `validate_refuses_a_key_version_without_the_owner_line` |

**On the redundancy question the orchestrator asked.** None of the three should
be dropped, and the campaign is what establishes it: a test no mutant isolates
is the redundant one, and each of these three was isolated. One qualification is
owed, though. Mutant (a) neuters `check_owner_line`, which backs **both**
`build_at` (`header.rs:164`) and `rotate` (`header.rs:234`); it is therefore not
exclusive to `rotate`, and the pre-existing scenario 5 — which asserts only that
the message contains the string `I3` — would also die under it, untested here
since only `g2_rotation` was gated. A gate-exclusive mutant does exist and is
offered: delete the call at `header.rs:234` alone, leaving `build_at`'s intact.
That would isolate `rotate` strictly. Not run; the orchestrator's call.

### `CHDR-013` / `CHDR-014` — cardinal, position, and a non-degenerate referent

Scenario 6's `Given` is split off scenario 4's and carries two pre-existing
recipients; the appended grantee is `g2`, distinct from the `g1` already there;
the `Then` asserts `lines.len() == saved.len() + 1`, whole-prefix byte equality
against the pre-append vector, and that no key version was created.

Mutant: `append_line`'s `push` becomes `insert(0, …)` (`header.rs`).

| | `evidence_id` | Result |
|---|---|---|
| without the patch | `ev-5bb8fe85` | green 8/8 — `find(\|l\| l.to == "owner")` returns the owner line whatever was pushed around it |
| with the patch | `ev-541727ed` | **RED**, 7/8, `✘ And the owner line is byte-identical to before` — `every pre-existing line stays byte-identical AND keeps its position` |

### `CHDR-019` — routing hint replaced by structure and capability

The old assertion queried kid `g1`, absent from v2: `Header::open`'s filter loop
was empty, `open_line` was never entered, and the revoked's secret was never
used. §03.1 declares `to`/`kid` non-authorizing, so it proved neither the
structural claim nor the capability claim. Three assertions replace it: the
structural fact, `check_rotation(2, owner_kid)`, and a loop trying the revoked's
**secret** against every kid actually routable in v2.

Two mutants, one per half.

| Half | Mutant | without | with |
|---|---|---|---|
| capability | `kek` derived from the ephemeral alone (`seal.rs`) | `ev-589e3e89` 7/8 — the failure is the predicted scenario-2 collateral, scenario 7 **passes** | `ev-bda111af` **RED** 6/8, scenario 7 flips as predicted |
| structural | `Header::rotate` appends v1's lines to v2 (`header.rs`) | `ev-c1a72050` green 8/8 — the old assertion is blind | `ev-a72503f2` **RED** 7/8 |

**A first mutant was proposed by this role and was wrong; the diagnosis is
recorded because the audit text carries the same error.** The audit's stated
surviving mutant for `CHDR-019` is "remove the DH secret from the HKDF IKM in
`kek`, and line `g2` becomes openable by anyone who knows g2's public key". Run:
`ev-a87b91f1` without the patch and `ev-41261f7c` with it, **both green 8/8**.
The reason is in the code, not in the assertion. Under that mutant the KEK is
`HKDF(constant, info = KEK_INFO ‖ 0x00 ‖ epk ‖ recipient_pub)` — it still binds
`recipient_pub`, and `open_line` computes that value itself from the secret the
caller supplies (`seal.rs:118`, then `:120`). A caller holding g1's secret
therefore builds `f(epk, g1_pub)` while the line was sealed under
`f(epk, g2_pub)`; the AEAD rejects. The weakness is real at the primitive and
**unreachable through any API a Gherkin step can call**, since nothing in
`header.rs` or `seal.rs` lets a caller pass an arbitrary recipient public key.
The replacement mutant — `kek` from the ephemeral alone — removes the recipient
binding too and is reachable; that is the one that flips. Explanation 1, proved
on the code: the mutant was wrong, the assertion had nothing to catch.

### `CHDR-021` — scenario 8 rebuilt on real derivation

The old scenario sealed a constant under a constant and reopened the same
in-memory object under the same literal two steps later, establishing only
`wrap_open(wrap_seal(k, dk)) == dk`. It now derives a parent folder key and a
child section key from the B2 zone key, seals the child's v1 under the **derived**
key, runs a real rotation to a fresh key, posts the wrap under the derived parent
key with node and version read off the rotated header, and the `Then` re-derives
the parent key from an ancestor before opening, checks `wrap.node`,
`wrap.key_version` and `wrap.via`, and cross-checks the recovered key against
what the rotated header actually seals. `CHILD_NODE` and `PARENT_KEY` are gone.

Mutant: the wrap declares a key version other than the one the rotated header
carries. Two textual forms, because the fixture itself changed — base
`CHILD_NODE, 2,` → `3,`; patched `version` → `version + 1`. Stated rather than
hidden.

| | `evidence_id` | Result |
|---|---|---|
| without the patch | `ev-9e12fac6` | green 8/8 — `Wrap::open` recomputes its AAD from its own fields, so the old `Then` structurally could not see it |
| with the patch | `ev-dc55d4ab` | **RED** 7/8, `wrap posted under the wrong key version` |

### `CHDR-025` — the positive control `c1_fail_closed` lacked

All four of its assertions were negative, with no known-good base in its own
body. A mutation making nothing open at all satisfied every one of them
vacuously. The control now asserts the untouched tuple opens on `dk_hex` first.

Mutant: the header-line purpose literal (`PURPOSE_HEADER_LINE` → `PURPOSE_WRAP`)
in `seal.rs`. Gate: `-- --exact c1_fail_closed`, so the verdict is per-body.

| | `evidence_id` | Result |
|---|---|---|
| without the patch | `ev-73408ae8` | **green** — four negative assertions, all vacuously satisfied |
| with the patch | `ev-11f7e8c7` | **RED** at `c1_header_seal.rs:103` — `positive control: the untouched tuple MUST open under the nominal AAD: SealRejected("line does not open")` |

The finding's **second** conjunct — produce or retract the independent-generation
claim on `vectors/c1-header-seal.json` — was already closed before this lot
began. `vectors/gen-c.py` exists on this branch, added by lot B at `5be3047`;
its `check_c1()` reconstructs the vector byte for byte and names `CHDR-025` in
its docstring. The public audit is stale on that point. Established by reading
the tree, not by execution. One gap is flagged and is not this role's to fix:
`gen-c.py` is not wired into CI — `.github/workflows/ci.yml` runs `cargo fmt`
and the test job, and nothing re-runs `python3 gen-c.py --check`.

## The Gherkin phrase change, argued on its merits

`features/c-headers.feature:68`, and nothing else in the feature file:

> `Given a sealed header for the owner` → `Given a sealed header for the owner and an existing reader`

`CHDR-014` requires the grant scenario's `Given` to carry at least two
pre-existing recipients: on a single-recipient header, "every other line
untouched" degenerates to "the only other line is untouched" — there is no rest
to perturb and no order to permute, and an append that re-seals or reorders the
survivors is indistinguishable from the O(1) push §03.3 mandates. Changing the
fixture without changing the phrase would have left a `Given` announcing one
recipient while establishing two. That is a step whose text does not describe
the state it creates — the exact defect class this audit exists to find. Leaving
it would have been introducing the defect while repairing it. The orchestrator
holds the decision and took it; this role argued for it and applied it.

Constraints honoured, all checkable:

- counts unchanged — **8 scenarios / 28 steps / 4 rules**, confirmed by `grep`
  on the feature file and by the gate summary of `ev-c2945d9b`;
- no other phrase touched — `git diff` on the feature file is one line;
- no accidental rebinding. The four `#[given("a sealed header …")]` literals are
  pairwise distinct and each matches exactly one line of the feature. The
  near neighbour `a sealed header for the owner and a grantee` is already bound
  to `sealed_header_owner_grantee` (scenarios 2 and 3) and was deliberately
  **not** reused: a duplicate literal would have shadowed the two-recipient
  fixture or made the step ambiguous.

The change is visible in the gate transcripts rather than merely asserted:
`ev-c2945d9b` and `ev-a1fa00fc` both print `✔ Given a sealed header for the
owner and an existing reader`.

## Orchestrator decisions, recorded as the orchestrator's

Both of the following are the orchestrator's, not this role's.

1. **`CHDR-016` is out of lot A.** Its statement is about production grant
   behaviour in `aithos-bundle` — `add_line_on` appends at `KV = 1` instead of
   `latest_version()` and never opens the current header — not about a Gherkin
   assertion. Correcting it from this branch would have been blocking condition
   8, scope. Removed from `assigned_findings` in `STATE.md` and recorded in
   `QUEUE.yaml` as `chdr-016-grant-path`, owed jointly by `g-revocation` and
   `d-bundle`. Neither closed nor withdrawn. This role analysed it and wrote no
   test for it; the lot is eight findings.
2. **The three-mutant expansion on `CHDR-009`** was the orchestrator's. This
   role's answer to the redundancy question it raised is in the `CHDR-009`
   section above: keep all three, with one qualification on mutant (a) and an
   exclusive replacement offered.

## Limits of the conclusion

- **No mutant proves the `CHDR-014` degeneracy claim itself** — that a `Given`
  carrying one recipient cannot express "every other line". The claim is about
  the fixture, and its proof is the `saved.len() >= 2` guard, which is an
  assertion about the test's own precondition and not a differential. The
  `insert(0, …)` mutant covers only the ordering half. Journalled by the
  orchestrator; restated here.
- **No mutant covers the cardinal half of `CHDR-013`.** `insert(0, …)` flips the
  prefix assertion, not `lines.len() == saved.len() + 1`. A surnumerary line —
  the case the audit names — is untested. Mutant available on request: make
  `append_line` push its line twice.
- **No mutant covers three of the four `CHDR-021` assertions.** Only the version
  binding was exercised. The node binding, the `via` binding and the
  derivation-cut pair (a) and (b) are unproven by mutation. Two mutants
  available on request: pass `&child_node` as `via` in `post_uplink_wrap`; and
  seal v1 under a constant instead of the derived key in `derived_node_rotated`.
- **`CHDR-009` mutant (a) is not gate-exclusive** — see above. Its RED proves
  `rotate`'s fail-closed side is now typed; it does not prove that test is the
  gate's only defender.
- **The `CHDR-001` evidence pair is not from an identical tree.** `ev-eb765f2c`
  was taken before the Gherkin phrase edit and prints the old phrase; its
  counterpart `ev-be6df11d` does too. Scenario 4's `Given` is untouched by that
  edit and the final tree gate `ev-c2945d9b` is green, so no re-run is needed —
  but the pair predates the tree being reviewed, and that is stated rather than
  smoothed over.
- **`CHDR-002` is P3 by the audit's own reconciliation**, on the ground that no
  production mutant survives the whole `Rule` thanks to this defect. `ev-bf5be536`
  confirms it: scenario 1 dies under the fixture mutant. What is repaired is
  scenario-level proof strength, which is what the finding claims — no more.
- **The audit's stated mutant for `CHDR-019` is wrong**, for the reason proved in
  that section. This role did not edit `docs/audits/`; the correction is reported
  for the reviewer and the audit owner.
- **No `VERIFIED` is claimed.** This role cannot raise it.
- **Backward compatibility was not weighed**, per `features/AGENTS.md`
  § *Project stage*. The repository's own cost was weighed and is nil: no vector,
  no pinned digest and no `vectors/ownership.json` entry is touched — that file
  pins JSON, not test sources. Two now-unused fixtures were removed,
  `CHILD_NODE` and `PARENT_KEY`; `PARENT_KEY = [0x55; 32]` mirrored
  `g2-rotation.json`'s `via_key_hex`, but no assertion ever tied them.

## Affected files and symbols

| File | Symbols |
|---|---|
| `features/c-headers.feature` | line 68 only |
| `rust/crates/aithos-bundle/tests/cucumber.rs` | `ProtocolWorld.saved_lines`, `.uplink_parent`, `.uplink_child`, `.uplink_child_key_before`; `sealed_header_owner_only` (split), `sealed_header_owner_and_reader` (new), `derived_node_rotated`, `corrupt_line`, `replay_line_other_node`, `append_grantee_line`, `post_uplink_wrap`, `grantee_opens` (split), `new_grantee_opens` (new), `opening_rejected`, `owner_line_untouched`, `revoked_cannot_open`, `parent_recovers_via_wrap`; `CHILD_NODE` and `PARENT_KEY` removed |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | `c1_fail_closed` |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | `G2.missing_owner_must_fail`; `check_rotation_refuses_a_new_version_without_the_owner_line`, `rotate_refuses_a_survivor_set_without_the_owner`, `validate_refuses_a_key_version_without_the_owner_line` (new) |

No production source file is modified. `spec/`, `vectors/`, `docs/audits/`,
`PROCESS.md`, `BLOCKED.md` are untouched by this role. `STATE.md` and
`QUEUE.yaml` carry the orchestrator's own decisions and are left uncommitted by
this role, which is forbidden to modify them.

## Final gates at the reviewed tree

| Tier | `evidence_id` | Result |
|---|---|---|
| feature | `ev-c2945d9b` | exit 0 — 1 feature / 4 rules / **8 scenarios / 28 steps** |
| full cucumber | `ev-a1fa00fc` | exit 0 — 18 features / 114 rules / 836 scenarios / 3577 steps |
| workspace | `ev-3013c663` | exit 0 |
| `cargo fmt --check` | `ev-e3b0c442` | exit 0 |
| `cargo clippy --workspace --all-targets -D warnings` | `ev-d6ce5ee9` | exit 0 |

## Status and next action

All eight assigned findings are **`IMPLEMENTED`**. None is `VERIFIED`; only an
independent reviewer may raise them.

Next: `REVIEW_REQUESTED`, via `auditor/audit-c-headers/SKILL.md` in review mode,
on a candidate extract without `.git` and without this report — the same shape
that worked on lot B (`PROCESS.md`, § *Material isolation of Pass A*). Two debts
are named for that reviewer: the audit's stated `CHDR-019` mutant is unreachable
through the public API and its text should be corrected; and the Gherkin markers
at `features/c-headers.feature` still name lot A identifiers, so their removal
is not a simple deletion.

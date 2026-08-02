# Correction review 02 — `b-derivation.feature`

## Run identity

| Field | Value |
|---|---|
| Run type | independent correction review, round 2 |
| Role / skill | auditor — `audit-b-derivation`, `review` mode |
| Date | 2026-08-02 |
| Baseline (immutable) | `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` |
| Candidate (immutable) | `4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41` |
| Branch | `codex/fix-b-derivation-bder-006-008-decisions` |
| Worktree HEAD at review start | `ffdba3e46d23b5f1166460d9491a8e58d5240633` (branch tip: candidate + this round's documentation commits) |
| Assigned findings | `BDER-006`, `BDER-008` — accepted or rejected separately |
| Out of scope | `BDER-007`, `BDER-010`, `BDER-012`, `d-bundle.feature` |
| Output | `features/.agents/b-derivation/auditor/runs/2026-08-02-audit-review-02.md` |

### Execution environment, disclosed

Neither `cargo` nor `rustup` exists on the machine that hosts the repository.
Every gate in this report was run on an **exported copy** of the immutable
revisions, not on the workstation:

```text
git archive --format=tar 4f5921e -- . ':!rust/target' | gzip -9 > rust/target/_to_delete/auditor-cand-4f5921e.tar.gz
git archive --format=tar 513b366 -- . ':!rust/target' | gzip -9 > rust/target/_to_delete/auditor-base-513b366.tar.gz
```

Export integrity, verified on both sides:

```text
04d21ea44e9a9d984034a889f6da2eec3a8f9089491b71b09d107d8edf924c22  auditor-cand-4f5921e.tar.gz
e6aaad9e09d2def4ccd86d7acad5b60079eab1b64c016a8599467055794abe42  auditor-base-513b366.tar.gz
```

`git archive` reconstructs the revision's tree from Git objects, so the copy is
the revision, not the workstation's working tree. Both Pass A code reading and
every gate below were performed against those extracted trees. **The gates did
not run on the owner's workstation.**

A second environment hazard was found and neutralised. The container that runs
these gates is shared with the round-2 corrector's earlier session: a
`/tmp/cand` tree with a 5.5 GB Cargo `target/` directory, timestamped 07:46—07:48,
pre-existed this run. A first gate attempt extracted the candidate over that
tree; because `git archive` restores commit mtimes that are older than the
pre-existing build artefacts, Cargo reported `Finished ... in 0.11s` and
**reused a binary it had not rebuilt from the candidate sources**. That result
was discarded. Every gate reported below was re-run in fresh directories
(`/tmp/aud-cand`, `/tmp/aud-base`) with a private `CARGO_TARGET_DIR`, and the
logs are quoted with their compilation lines so the rebuild is visible. No file
under the corrector's `/tmp` directories was read.

## Pass A — frozen history-blind verdict

### Inputs and contamination status

Read before freezing: `features/.agents/PROCESS.md`;
`features/.agents/b-derivation/auditor/audit-b-derivation/SKILL.md`;
`features/.agents/shared/audit-gherkin-feature/SKILL.md`;
`features/.agents/b-derivation/DOMAIN.md`; the two decision records of
`features/.agents/b-derivation/decisions/` (normative — they define this round's
contract); the routing table of `STATE.md` (lines 1–23, stopping before
`## Current instruction`); and, from the candidate tree only:
`features/b-derivation.feature`, `rust/crates/aithos-bundle/tests/cucumber.rs`,
`rust/crates/aithos-core/src/derive.rs`,
`rust/crates/aithos-core/tests/b2_derivation.rs`,
`rust/crates/aithos-bundle/tests/vectors_ownership.rs`,
`spec/02-content-tree.md`, `vectors/README.md`, `vectors/b2-derivation.json`,
`vectors/ownership.json`, `vectors/gen-{f,g,h,h2,i}.py`,
`.github/workflows/ci.yml`, `features/d-bundle.feature`.

Not read before freezing: `corrector/runs/2026-08-02-correction-02.md`;
`docs/audits/features/b-derivation.md`; the round-1 review and initial audit
bodies; `STATE.md`'s `## Current instruction` section; `git log`, `git show`,
`git diff`, `git blame`, any commit message.

Three contaminations are disclosed, none of them a verdict from another role:

1. **Historical fact inside a normative input.** The `BDER-008` decision record
   names commit `1b7d258` and pre-states the shape of the honest provenance
   claim. Decision records are normative for this round and explicitly allowed
   in Pass A, but this one carries a historical assertion I cannot verify
   history-blind. Treated as an unverified claim, not as evidence — see U2.
2. **Assigned scope pre-describes the corrections.** `STATE.md`'s routing table
   says `BDER-006` (`Rule` retitled) and `BDER-008` (B2 `description` rewritten,
   values frozen). `PROCESS.md` allows reading the assigned scope, so this is
   sanctioned, but it does tell me what changed before Pass A. It does not tell
   me whether either change is correct, which is the entire question.
3. **My own procedural slip.** While diagnosing the stale-build hazard above, I
   ran `diff -rq` between the two extracted trees. That is differential
   evidence, obtained before Pass A was frozen. It exposed the *set* of changed
   paths — `features/b-derivation.feature`, `vectors/b2-derivation.json`,
   `vectors/ownership.json`, and nothing under `rust/` or `spec/` — but no file
   content. I disclose it rather than pretend it did not happen; its effect is
   limited to confirming point 2 and adding one path (`vectors/ownership.json`)
   that scope had not announced. All Pass A judgements below rest on candidate
   content read directly, and none of them depends on the baseline.

### Static tag gate

```text
$ bash features/.agents/scripts/verify-feature-tags.sh      # candidate 4f5921e
/tmp/aud-cand/features/gateway-delegated-client-surfaces.feature: expected first line @gateway-delegated-client-surfaces, got @wip @g4 @wasm @cli
EXIT=1
```

The mandatory static gate of `PROCESS.md` **fails on the candidate**. I did not
work around it and did not repair it. Same command on the baseline:

```text
$ bash features/.agents/scripts/verify-feature-tags.sh      # baseline 513b366
/tmp/aud-base/features/gateway-delegated-client-surfaces.feature: expected first line @gateway-delegated-client-surfaces, got @wip @g4 @wasm @cli
EXIT_BASE=1
```

Identical failure, identical file, identical first line — the offending file is
byte-identical on both revisions. The failure **pre-exists the candidate** and
is caused by `features/gateway-delegated-client-surfaces.feature`, whose first
line is `@wip @g4 @wasm @cli` instead of the canonical
`@gateway-delegated-client-surfaces`. It is outside `b-derivation`'s scope, it
is not a regression of this round, and it does not touch `@b-derivation`, which
the script accepts. It is nonetheless a standing violation of `PROCESS.md`
§"Feature targeting and gate pyramid": a repo-wide gate that no role can pass
today. Recorded here as an observation for the orchestrator; not charged to
this candidate, not closed by this review.

### Feature gate — once, on the immutable candidate

```text
$ cd /tmp/aud-cand/rust && CARGO_TARGET_DIR=/tmp/aud-tc \
  cargo test --manifest-path /tmp/aud-cand/rust/Cargo.toml \
             -p aithos-bundle --test cucumber -- --tags @b-derivation
   Compiling aithos-core v0.1.0-alpha.1 (/tmp/aud-cand/rust/crates/aithos-core)
   Compiling aithos-bundle v0.1.0-alpha.1 (/tmp/aud-cand/rust/crates/aithos-bundle)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 26s
     Running tests/cucumber.rs (/tmp/aud-tc/debug/deps/cucumber-be4ecf3a035cbebd)
Feature: Content-tree derivation
Rule: Derivation is deterministic and per-segment
  Scenario: The same path always yields the same key                 (6 steps ✔)
  Scenario: Sibling nodes get unrelated keys                         (4 steps ✔)
Rule: Holding a folder yields its subtree, nothing else
  Scenario: A folder holder derives every descendant                 (5 steps ✔)
  Scenario: A folder holder cannot reach sideways                    (6 steps ✔)
  Scenario: Renaming never re-keys                                   (6 steps ✔)
Rule: Each tag anchor is a distinct derivation
  Scenario: A folder-local tag view is its own lock                  (3 steps ✔)
[Summary]
1 feature
3 rules
6 scenarios (6 passed)
30 steps (30 passed)
```

Run once, on the immutable candidate, from a verified-clean rebuild (the
`Compiling aithos-core` / `Compiling aithos-bundle` lines prove the candidate
sources were actually recompiled). The `b-derivation` block is counted by name:
`Feature: Content-tree derivation`, 3 rules, 6 scenarios, 30 steps — the counts
`DOMAIN.md` requires since round 1. Per `BDER-011` the exit code proves nothing;
the printed block is the evidence and it is complete.

No unfiltered Cucumber gate, no workspace gate, no `fmt`/`clippy` was run by
this role.

### U1 (`BDER-006`) — does the tag-view `Rule` promise exactly what its scenario proves?

Candidate text (`features/b-derivation.feature:58-65`):

```gherkin
  Rule: Each tag anchor is a distinct derivation

    @audit-partial @bder-006
    # AUDIT BDER-006 — PARTIAL / DECISION_REQUIRED; see the public audit.
    Scenario: A folder-local tag view is its own lock
      Given a zone key and a folder
      When I derive the tag view "toto" at the folder and at the zone root
      Then the two anchors differ from each other and from the folder key
```

Step-by-step trace against the candidate's own code:

- `Given a zone key and a folder` → `cucumber.rs:7541 zone_and_folder`. Calls
  `a_zone_key` (`:7493`), which sets `w.zone_dk = B2Vector::load().zone_dk()` —
  the zone DK comes from `vectors/b2-derivation.json`, not from a local literal.
  Then `w.deep_path = NodePath::folder(Zone::Circle, vec![sid(1)])`.
- `When I derive the tag view "toto" ...` → `cucumber.rs:8079 derive_tag_anchors`.
  The Gherkin string parameter `"toto"` is bound and used: it builds
  `NodePath::tag_view(Circle, [sid(1)], tag)`, `NodePath::tag_view(Circle, [], tag)`
  and `NodePath::folder(Circle, [sid(1)])`, and pushes
  `node_key(&zone, ·)` for each into `w.node_keys`. The parameter is not ignored
  and the third value is a real folder key, not a placeholder.
- `node_key` (`aithos-core/src/derive.rs:46-56`) walks `path.folders` with
  `derive_key(folder_label(sid), key)` and terminates the `TagView` leaf with
  `derive_key(tag_label(tag), key)`, where `tag_label` is
  `"aithos-core/v1/t/" + tag` (`derive.rs:38-40`). This is exactly
  `spec/02-content-tree.md` §2.5 `K(tag anchor) = derive("aithos-core/v1/t/"+tag, K(folder))`.
  So each anchor genuinely **is** a derivation, by the production label, from
  the production folder key — and the zone-root anchor derives from the zone DK
  with an empty spine, which §2.1 explicitly authorises ("tag view anchored at
  ANY folder, zone root included").
- `Then the two anchors differ ...` → `cucumber.rs:12295 anchors_distinct`. It
  first asserts `w.node_keys.len() == 3` ("this Then reads exactly three
  derivations" — the `BDER-009` precondition guard), then collects into a
  `BTreeSet` and asserts `unique.len() == 3`. Pairwise distinctness of the three
  values, no weaker.

Confrontation with the spec at this exact place. §2.5 is the derivation clause
(the formula above). §2.9 is the *anchoring* clause and says something the
derivation layer cannot show: an anchor "grants nothing by derivation downward
— sections enter it by **wrap**"; "a zone-root view spans the whole zone; a
folder-local view spans that subtree only". The candidate title, `Each tag
anchor is a distinct derivation`, makes a claim in the vocabulary of §2.5 only.
It does not say "anchor", "view", "spans", "grants" or "covers"; it makes no
promise about what a holder of an anchor key can read. The scenario proves it:
three production derivations, pairwise distinct. Title and evidence are in the
same register.

Where the §2.9 proof should live, and whether it exists. It does not exist
today. `features/d-bundle.feature` — the feature `DOMAIN.md` names as the owner
of "tag-view rebuild and the wraps that populate an anchor" — has seven `Rule`s
(editions chain, sealed-store round-trip, public zone, self zone, owner
operation parity, local transaction atomicity, narrow capabilities) and **no
tag-view and no `wrap` scenario**. So the behavioural half of §2.9 is currently
unproven anywhere in the executable corpus. That is a real coverage hole, and it
is the hole the `BDER-006` decision record undertakes to close by widening the
`TARGETED` `d-bundle` follow-up. Nothing in the candidate closes it, and nothing
in the candidate was supposed to.

Residual over-promise, stated honestly. `Each` is a universal quantifier
exhibited by a single sample: one tag (`"toto"`), two anchor sites (one
folder-local, one zone-root), one folder key. The scenario does **not** exercise
two *different* tags at the same folder, i.e. the separation carried by the tag
string itself inside the `t/` label; and it does not anchor either
`tag_anchor_*_hex` value of `vectors/b2-derivation.json` byte for byte, so
distinctness rests on inequality between random-looking arrays — the weakest of
the three proof shapes named in the audit skill — rather than on the vector.
Two mitigations are visible in the candidate's own code and are not speculation:
`rust/crates/aithos-core/tests/b2_derivation.rs::b2_deep_chain_and_anchors`
does assert both anchors byte-exactly against the vector, and
`cucumber.rs:208-216 b2_production_labels` includes `tag_label(tag)` in the
21-label battery the sibling scenario replays. This residue is sampling breadth,
which is the subject matter of `BDER-012` (out of scope for this review), not of
`BDER-006`. It is recorded, not charged.

Frozen Pass A verdict, U1: **the retitled `Rule` promises what its scenario
proves, in the register the scenario can prove.** The §02.9 semantics is no
longer promised by the title, and it is provably not proven anywhere in the
corpus, so the title no longer over-promises. Provisionally `VERIFIED`, subject
to Pass B confirming that the baseline title did promise more and that this is
the only change to the file.

### U2 (`BDER-008`) — does the B2 `description` describe a provenance the repository supports?

The candidate's `description` (`vectors/b2-derivation.json:3`) makes six
checkable assertions. I verified each one against the candidate tree, field by
field, without history.

| # | Assertion in `description` | Independent check | Verdict |
|---|---|---|---|
| 1 | "no independent B2 generator exists in this repository" | `ls vectors/` — 38 `gen-*.py` files, none named `gen-b2*`; no file recomputes B2's expected values other than as a cross-check | **corroborated** |
| 2 | "this vector, its Gherkin fixtures and `rust/crates/aithos-core/tests/b2_derivation.rs` were all created in the same commit `1b7d258`" | requires `git log`/`git show` | **undecidable history-blind** — see below |
| 3 | "`folder1_key_hex` is recomputed from `zone_dk_hex` by five Python scripts (`gen-f.py`, `gen-g.py`, `gen-h.py`, `gen-h2.py`, `gen-i.py`)" | each script loads `b2-derivation.json`, recomputes `derive("aithos-core/v1/d/"+folder_sids[0], zone_dk)` in Python `blake3` and asserts equality: `gen-f.py:104-108`, `gen-g.py:150-153`, `gen-h.py:68-73`, `gen-h2.py:95-100`, `gen-i.py:76-81`. Exactly five, exactly those five | **corroborated** |
| 4 | "and `deep_section_key_hex` by `gen-f.py` alone" | `gen-f.py:107,109` walks the full spine plus the `s/` leaf and asserts `deep_section_key_hex`; no other script mentions the field | **corroborated** |
| 5 | "`sibling_section_sid`, `sibling_section_key_hex`, `tag` and both `tag_anchor_*_hex` fields have no external witness and are self-certified by `aithos-core::derive` through `b2_derivation.rs`" | repo-wide search for those field names outside `*.rs` returns nothing; `b2_derivation.rs:41-68` asserts all five against `node_key`, i.e. against the code under test | **corroborated** |
| 6 | "No value changes: the vector stays frozen (README rule 3)" | see the independent recomputation below | **corroborated as to correctness**; the *frozen* half is settled in Pass B |

On assertion 2, I say so plainly rather than stepping over the barrier: the
provenance of a file — which commit created it, alongside what — is a historical
property and cannot be established from the current tree. It is deferred to Pass
B and is verified there. Note that this claim reached me through the `BDER-008`
decision record, which is a normative Pass A input; I treated it as a claim, not
as evidence.

**Independent recomputation of every value.** Rather than trust either the Rust
test or the five partial Python cross-checks, I recomputed all five expected
keys from `zone_dk_hex`, the sids and the tag with an independent Python
`blake3` (1.0.9), applying the §2.5 formulas directly:

```text
folder1_key_hex              MATCH  f26a2d48bba677cf6e313be1c647434719f7847cb441952190348969709e388c
deep_section_key_hex         MATCH  8147bb7a4f8fb4d6608c15206a83c082ada108ed760025fe5421f758f79f987d
sibling_section_key_hex      MATCH  d625b5c0faf2d561433b2b137f473e08f6d8db389552d862dba91262b0b43df2
tag_anchor_folder1_hex       MATCH  a11d049c38e7cc2efbeb5d9ddc7edfe1d7995ccfb6096e5eacbb9d6812242ce6
tag_anchor_zone_root_hex     MATCH  646ee439e02001714ad213dd0d5e63d99eaae3778ab7df4e064ba74294ff73ac
```

All five are arithmetically correct for spec §2.5. This does not make them
*independently generated* — I recomputed them from the committed inputs, which
is a conformance check, not a provenance proof — but it does establish that no
value in the candidate is wrong, and that the two tag anchors, which have no
committed external witness, are nevertheless correct.

**The frozen-vector mechanism.** `vectors/ownership.json` pins a SHA-256 per
vector and `rust/crates/aithos-bundle/tests/vectors_ownership.rs` enforces it —
its own header states the intent: "Le manifeste épingle aussi le SHA-256 de
chaque vecteur : la règle 3 du README (« frozen once green ») devient
mécanique." The candidate's `vectors/ownership.json` carries
`sha256 = ec5be7976b62a9d2810ccfcdc598d9b6bbdcbdaee0b1960db384a875a01abe75` for
`b2-derivation.json`, and `sha256sum vectors/b2-derivation.json` on the
candidate returns exactly that. The pin and the file agree, so the mechanism is
internally coherent on the candidate and the harness will pass.

Two properties of that mechanism must be stated for the record, because they
bound what "frozen" can mean here. First, the pin is over the **whole file**, so
it cannot distinguish a description edit from a value edit; re-pinning was
mechanically required to change the prose, and the pin therefore proves
integrity-after-the-fact, never that only prose moved. That specific
verification is differential and is done in Pass B. Second, the five Python
cross-checks that constitute B2's only external corroboration are **not wired
into CI**: `.github/workflows/ci.yml` runs `cargo fmt`, `cargo clippy` and
`cargo test --workspace`, and nothing invokes `vectors/gen-*.py`. So on any
given CI run, `folder1_key_hex`'s five witnesses are dormant and the effective
authority is `b2_derivation.rs`, i.e. the code under test. The decision record
already anticipates this and defers it to the generator lot; I confirm it
independently and do not charge it to this candidate.

**A defect the `description` cannot repair, found in Pass A.** The false
provenance claim that `BDER-008` exists to remove is present in the repository
in **two** places, and the candidate corrects only one. The other is the header
of the very test file the new `description` names as the self-certification
route:

```rust
// rust/crates/aithos-core/tests/b2_derivation.rs:1-2
//! Conformance vector B2 — content-tree derivation (spec 01.3, 02.5).
//! Expected values generated independently (Python blake3).
```

On the candidate, line 2 still asserts independent generation by Python blake3 —
the exact wording the `BDER-008` decision quotes as the claim to retire
("« generated independently (Python blake3) »"). The repository now says two
contradictory things about the same five values: the vector's `description`
says there is no independent B2 generator and that the tag anchors are
self-certified by `derive.rs` through `b2_derivation.rs`, while
`b2_derivation.rs` itself says its expected values were generated
independently. A reader arriving through the Rust test — the normal route for a
conformance implementer, and the route `DOMAIN.md` lists as the feature's "Core
test" — still receives the false claim, unmitigated.

This is not scope creep into `BDER-007`. `BDER-007` asks for an independent
generator to *exist*; this finding asks only that the repository stop asserting
one exists. It is the same defect, the same sentence, the same finding, in a
second file. `derive.rs`'s own doc comments were checked and make no provenance
claim; `cucumber.rs:141-152` is honest and already states the five-generator /
one-generator / no-witness split correctly.

Frozen Pass A verdict, U2: the rewritten `description` is **accurate on every
assertion I can check without history**, and no value moved or is wrong. But
`BDER-008` — "the repository claims an independent generation that does not
exist" — is **not resolved**, because
`rust/crates/aithos-core/tests/b2_derivation.rs:2` still makes that claim
verbatim. Provisionally **not `VERIFIED`**: correction incomplete.

### Pass A limits

- Provenance is partly historical. Assertion 2 of the `description` (same-commit
  creation of vector, fixtures and test) and the "no value changed" half of
  assertion 6 are both undecidable from the current tree and are settled in Pass
  B, explicitly labelled as such.
- Pass A cannot tell whether the baseline `Rule` title over-promised; it can only
  establish what the candidate title promises and what the candidate proves.
- I read no production Rust outside the derivation path and its two test
  harnesses; this review is not an audit of `aithos-bundle`'s anchoring code.
- One `diff -rq` between the extracted trees preceded this freeze; disclosed
  above with its exact reach.

**Pass A is frozen at this line. Everything below was written after opening the
`513b366..4f5921e` range, the commit messages, the corrector's run report and
the public audit.**

## Pass B — historical and differential review

### Inputs opened after the freeze

`git log`/`git show`/`git diff` over `513b366..4f5921e` and over the artefacts'
birth commits; the candidate's commit message;
`features/.agents/b-derivation/corrector/runs/2026-08-02-correction-02.md`;
`docs/audits/features/b-derivation.md`;
`features/.agents/orchestrator/STATE.md` and the impact review report;
`STATE.md`'s `## Current instruction` section.

### The reviewable range is exactly what state claims

```text
$ git log --oneline 513b366..4f5921e
4f5921e fix(derivation): ronde 2 — BDER-006 (titre de la Rule) et BDER-008 (provenance du vecteur B2)

$ git diff --stat 513b366 4f5921e
 features/b-derivation.feature | 2 +-
 vectors/b2-derivation.json    | 2 +-
 vectors/ownership.json        | 4 ++--
 3 files changed, 4 insertions(+), 4 deletions(-)
```

One commit, three files, four lines, no Rust, no step definition, no spec, no
other feature, no other vector. This matches my pre-freeze `diff -rq`
observation and adds nothing Pass A did not already account for. The branch
carries three later documentation commits (`5274905`, `bb4763f`, `ffdba3e`) plus
my own Pass A freeze (`9c52a7a`); none of them is part of the behavioural
candidate, and none of them touches `features/`, `vectors/` or `rust/`.

### `BDER-006` — the baseline title did over-promise

```diff
-  Rule: Tag views anchor at folders
+  Rule: Each tag anchor is a distinct derivation
```

This is the whole change to `features/b-derivation.feature`. The baseline title,
`Tag views anchor at folders`, is written in the vocabulary of
`spec/02-content-tree.md` §2.9 — it asserts the anchoring relation and its
attachment point, which is precisely the clause that governs "grants nothing by
derivation downward", "sections enter by **wrap**", and "a folder-local view
spans that subtree only". The single scenario under it proves none of that; it
proves three distinct derivations. Pass A established, without seeing this line,
that the candidate title claims only what §2.5 governs and that the scenario
proves exactly that claim. Pass B now supplies the other half: the baseline
title genuinely promised more than the Rule contained, so the retitle removes an
over-promise rather than papering over a behavioural gap.

Differential detectability: none is possible, and none is claimed. A `Rule`
title is not executable; no RED test can distinguish the two revisions. The
corrector states this explicitly and does not manufacture a test. The only
executable check that applies — that the retitle changes no selection and no
count — I reproduced myself on the immutable candidate: 3 rules, 6 scenarios,
30 steps, the same signature `DOMAIN.md` requires since round 1.

The linked obligation is real, and I verified it is carried. The `BDER-006`
decision is "option A **with a mandatory `d-bundle` extension in the same
movement**", and the decision record warns that without the extension "cette
décision dégénère en « A seule » et le §02.9 reste sans preuve ; ce n'est pas ce
qui est décidé ici". Accepting the retitle without confirming that the
obligation survives would do exactly that. It survives:
`features/.agents/orchestrator/STATE.md:15-18` records, under `## Tracked
follow-ups`, "**`d-bundle` targeted follow-up (widened by the BDER-006
decision):** its future cycle must record the co-owned steps (impact report
§9.5) **and add the tag-view/`wrap` scenarios proving the behavioral half of
spec §02.9**". That is the register the `d-bundle` cycle will read.
`b-derivation`'s own `STATE.md` carries the same widening. Pass A's independent
finding that `d-bundle.feature` contains no tag-view and no `wrap` scenario
confirms the debt is still outstanding — which is expected, not a defect of this
candidate.

### `BDER-008` — every claim of the new `description` holds, including the historical one

Pass A corroborated five of the six assertions from the current tree and
deferred one. Pass B settles the deferred assertion and the frozen-value half:

```text
$ git log --diff-filter=A --format='%h %ad %s' --date=short -- vectors/b2-derivation.json
1b7d258 2026-07-09 step B complete: content-tree derivation (B2)
$ git log --diff-filter=A --format='%h %ad %s' --date=short -- rust/crates/aithos-core/tests/b2_derivation.rs
1b7d258 2026-07-09 step B complete: content-tree derivation (B2)
$ git show --stat 1b7d258 | grep cucumber
 rust/crates/aithos-bundle/tests/cucumber.rs        | 160 +++++++++++++++++++++
$ git log --all --diff-filter=A --format='%h %s' -- 'vectors/gen-b*'
(empty)
```

The vector, the conformance test and the 160-line Gherkin step block were all
introduced by the same commit `1b7d258` on 2026-07-09, and no `gen-b2*`
generator has ever existed on **any** branch. Assertion 2 of the `description`
is exact. So is the corollary the finding rests on: the "independence" the
baseline claimed was a property of the author's workstation in July, never of
this repository.

Frozen-vector rule, verified key by key rather than taken on trust:

```text
same key set and order: True
same    vector                     same    folder1_key_hex
same    zone_dk_hex                same    deep_section_key_hex
same    folder_sids                same    sibling_section_key_hex
same    section_sid                same    tag_anchor_folder1_hex
same    sibling_section_sid        same    tag_anchor_zone_root_hex
same    tag
CHANGED description
```

`description` is the only field that moved. Combined with Pass A's independent
Python `blake3` recomputation of all five expected keys — five `MATCH` — the
vector is both unchanged and correct. README rule 3 holds in substance.

The `ownership.json` re-pin is mechanically necessary and correct on both sides:

```text
$ git show 513b366:vectors/b2-derivation.json | sha256sum
73a4740d5d0c4361e91fc54c3def279517701689e653f8f99928b186a007b139   == baseline pin
$ git show 4f5921e:vectors/b2-derivation.json | sha256sum
ec5be7976b62a9d2810ccfcdc598d9b6bbdcbdaee0b1960db384a875a01abe75   == candidate pin
```

Each revision's pin equals that revision's file. `vectors_ownership.rs:182
vectors_match_their_pinned_digests` compares exactly these values, so the
corrector's RED/GREEN claim ("4 passed / 1 failed with the stale pin, 5 passed
as committed") is arithmetically certain from the two digests above without
re-running it: the pin is over whole-file bytes, and the two digests differ. I
did not rerun that test — it is a relevant-regression gate owned by the
corrector, and the digests settle it.

### The corrector's report is a claim; here is what survived verification

Verified true and reproduced independently: the range and file list; the retitle
being the sole feature change; no value change in B2; the digest re-pin and its
necessity; the same-commit birth of the three B2 artefacts; the absence of any
`gen-b2*` in history; the `@b-derivation` block reporting 1 feature / 3 rules /
6 scenarios / 30 steps on the immutable candidate; the pre-existing red status
of `verify-feature-tags.sh`.

Not reproduced, and correctly attributed to the corrector rather than to me: the
global Cucumber run (18 features / 836 scenarios), `cargo test --workspace
--no-fail-fast`, `cargo fmt --check`, and the focused `vectors_ownership`
RED/GREEN. `PROCESS.md` reserves those for the correcting role and forbids this
role from rerunning them. They are reported here as the corrector's evidence,
never as mine.

One claim I checked and downgraded: the corrector's report describes
`b2_derivation.rs:2` as "hors périmètre assigné, laissée en place et signalée".
The disclosure is honest and I credit it. The characterisation is what I
disagree with — see the reconciliation below. Given the decision record's
`Conséquences exécutables` name exactly one action for this round ("réécrire la
`description` du vecteur"), stopping and disclosing was the correct behaviour
for a corrector; it is the reviewer's job, not the corrector's, to decide
whether the finding closes with that residue outstanding.

### Agreement and disagreement with Pass A

| Pass A verdict | Pass B evidence | Reconciled |
|---|---|---|
| U1: the candidate title promises what the scenario proves | baseline title `Tag views anchor at folders` did promise §2.9; one-line diff; counts unchanged; the `d-bundle` obligation is recorded in the orchestrator's tracked follow-ups | **confirmed, and strengthened** — Pass A could not see that the old title over-promised; Pass B supplies it |
| U2: the new `description` is accurate on every history-blind check | assertion 2 (`1b7d258`) confirmed; no `gen-b2*` ever; only `description` changed | **confirmed** |
| U2: `BDER-008` not resolved — `b2_derivation.rs:2` still carries the retracted claim | the corrector found the same line and disclosed it in the public audit and its run report | **unchanged as a fact; re-classified** — see below |

Nothing in Pass B reopened the current-code trace, and nothing in Pass B
upgraded a verdict on historical intent alone.

## Reconciliation, finding by finding

### `BDER-006` — `VERIFIED`

Independent proof reproduced by this role: the traced scenario
(`cucumber.rs:7541`, `:8079`, `:12295`) derives three keys through
`aithos-core::derive::node_key` using the production `t/<tag>` and `d/<sid>`
labels of `spec/02-content-tree.md` §2.5, and asserts, behind an arity guard,
that all three are pairwise distinct. The candidate `Rule` title, `Each tag
anchor is a distinct derivation`, states that and nothing else — no anchoring,
no coverage, no `wrap`, no scope. The baseline title did state more. The gate I
ran once on `4f5921e` shows the Rule still selected and still counted by name,
with the required 3 / 6 / 30 signature.

The `DECISION_REQUIRED` half is closed by the human owner's record of
2026-08-02, not by me; the executable consequence assigned to this round —
retitle only — is implemented exactly, with no scenario, step, tag or comment
touched. The mandatory `d-bundle` counterpart is recorded where the `d-bundle`
cycle will read it, so acceptance does not silently degrade the decision into
"option A alone".

Marker lifecycle: per `PROCESS.md` §"Gherkin audit-marker lifecycle", I remove
`@audit-partial @bder-006` and its adjacent `# AUDIT BDER-006` comment from
`features/b-derivation.feature`. This is the reviewer's gesture; the corrector
correctly left them in place.

Residue recorded, not charged: `Each` is exhibited by one tag and one pair of
anchor sites, and the two `tag_anchor_*_hex` vector values are not asserted by
the Gherkin layer. That is sampling breadth, i.e. `BDER-012` territory, out of
scope for this review, and it is mitigated by
`b2_derivation.rs::b2_deep_chain_and_anchors` and by `tag_label` being present
in the 21-label battery of `cucumber.rs:208-216`.

### `BDER-008` — `VERIFIED`, with `BDER-013` opened on the residue

The finding's own closure criterion, as written in this note before the round
("soit un `gen-b2-derivation.py` est commité et nommé, soit la `description`
énonce la provenance réelle et le fait que `folder1_key_hex` et
`deep_section_key_hex` sont corroborés tandis que les trois autres champs ne le
sont pas. Aucune valeur ne change"), is met exactly, and I verified each half
independently rather than accepting it: five named Python cross-checks located
line by line, one for the deep key, zero external witness for the other five
fields, no `gen-b2*` in any branch, all three artefacts born in `1b7d258`, all
five expected keys recomputed from scratch in Python `blake3`, and `description`
the only field that moved.

Pass A had provisionally withheld `VERIFIED` because
`rust/crates/aithos-core/tests/b2_derivation.rs:2` still reads `//! Expected
values generated independently (Python blake3).` — the exact sentence the
decision retracts. That fact is unchanged and I stand behind it. What changed is
its classification, on two grounds that only Pass B could supply. First, the
residue is disclosed, not concealed: the corrector found the same line and
recorded it in this note and in its run report. Second, this repository has an
established and correct way of handling exactly this shape — round 1's review
accepted `BDER-002` as `VERIFIED` and opened `BDER-012` on its residue rather
than rejecting a correction that met its stated criterion. Rejecting here would
punish a corrector for respecting a decision record that named one action, and
would leave the residue tracked by nothing.

So `BDER-008` closes, and the residue gets its own stable identifier and stays
visible until someone fixes it. It is not folded into `BDER-007`: `BDER-007`
asks for an independent generator to *exist*; `BDER-013` asks only that the
repository stop asserting one already does.

### `BDER-013` — the retracted provenance claim survives in the Rust conformance test (new, `OPEN`, P3)

`rust/crates/aithos-core/tests/b2_derivation.rs:1-2`:

```rust
//! Conformance vector B2 — content-tree derivation (spec 01.3, 02.5).
//! Expected values generated independently (Python blake3).
```

After round 2, the repository states two contradictory things about the same
five expected keys. `vectors/b2-derivation.json`'s `description` says no
independent B2 generator exists and that `sibling_section_*`, `tag` and both
`tag_anchor_*_hex` are self-certified by `aithos-core::derive` **through
`b2_derivation.rs`**; `b2_derivation.rs` itself says its expected values were
generated independently by Python `blake3`. A conformance implementer arriving
through the Rust test — the route `DOMAIN.md` lists as this feature's "Test de
conformité", and the route the new `description` itself names — still receives
the retracted claim.

Expected correction: replace line 2 with the same honest provenance the vector
now carries, or point it at the vector's `description` rather than restating a
provenance. No value, no assertion and no test name changes; the file's five
assertions stay exactly as they are. Closure criterion: no file in the
repository asserts independent generation for the B2 expected values while no
`gen-b2-derivation.py` exists. Naturally bundled with the future B2 generator
lot that closes `BDER-007`, but it does not depend on it — the claim can be
retracted today.

Checked while opening this: `aithos-core/src/derive.rs` makes no provenance
claim, and `cucumber.rs:141-152` is already honest, stating the
five-generators / one-generator / no-witness split correctly. The other vectors
carrying `generated independently` wording (`a1-genesis`, `a2-did`, `e1`,
`f1`, `g1`, `g2`, `g3`, `h1`, `h2`, `i1`, `cb2-max-children-versioning`) all
have a committed `gen-*.py`; B2 is the only family without one. This finding is
specific to B2 and does not generalise.

## Commands and results — exactly what this role ran

```text
1) bash features/.agents/scripts/verify-feature-tags.sh          (candidate 4f5921e)
   -> EXIT=1, gateway-delegated-client-surfaces.feature: first line @wip @g4 @wasm @cli
2) bash features/.agents/scripts/verify-feature-tags.sh          (baseline 513b366)
   -> EXIT=1, identical failure, identical byte-identical file  => pre-existing
3) cargo test --manifest-path rust/Cargo.toml -p aithos-bundle \
        --test cucumber -- --tags @b-derivation                  (candidate 4f5921e, once)
   -> clean rebuild (aithos-core + aithos-bundle recompiled, 1m 26s)
   -> Feature: Content-tree derivation
      1 feature / 3 rules / 6 scenarios (6 passed) / 30 steps (30 passed)
4) python3 + blake3 1.0.9, independent recomputation of the five B2 expected keys
   -> 5 / 5 MATCH
5) sha256 of vectors/b2-derivation.json at both revisions vs their ownership pins
   -> both match
```

Not run by this role, by design: unfiltered Cucumber, `--workspace`,
`cargo fmt`, `cargo clippy`, and the focused `vectors_ownership` RED/GREEN.

## Findings handled and not handled

| Finding | Assigned | Outcome |
|---|---|---|
| `BDER-006` | yes | `VERIFIED`; Gherkin markers removed by this review |
| `BDER-008` | yes | `VERIFIED`; residue split off as `BDER-013` |
| `BDER-013` | new | `OPEN`, P3, opened by this review |
| `BDER-007`, `BDER-010`, `BDER-012` | no | untouched, remain `OPEN` and visible |
| `d-bundle.feature` | no | not audited; its widened `TARGETED` debt confirmed outstanding |

## Affected files and symbols

Changed by the candidate: `features/b-derivation.feature` (line 58, `Rule`
title); `vectors/b2-derivation.json` (`description`); `vectors/ownership.json`
(`updated`, `b2-derivation.json` `sha256`).

Changed by this review: this run report; `features/b-derivation.feature`
(removal of `@audit-partial @bder-006` and its adjacent comment);
`docs/audits/features/b-derivation.md`; `docs/audits/features/README.md`;
`features/.agents/b-derivation/STATE.md`. No Rust, no vector, no other feature.

Symbols and surfaces exercised or inspected: `aithos_core::derive::{derive_key,
folder_label, section_label, tag_label, node_key}`;
`aithos_core::path::{NodePath, Zone, Leaf}`;
`aithos_bundle::tests::cucumber::{zone_and_folder, a_zone_key,
derive_tag_anchors, anchors_distinct, B2Vector, b2_production_labels}`;
`aithos_bundle::vectors_ownership::vectors_match_their_pinned_digests`;
`aithos_core::tests::b2_derivation::{b2_deep_chain_and_anchors,
b2_folder_key_alone_derives_descendants}`; `spec/02-content-tree.md` §2.1, §2.5,
§2.9; `vectors/README.md` rules 1 and 3.

Cross-feature impact candidates for the impact reviewer: `vectors/ownership.json`
is a repo-wide manifest — its `updated` field and one digest moved, and
`vectors_ownership.rs` is a shared harness; `vectors/README.md` rules 1 and 3
are cited by every vector family; `spec/02-content-tree.md` §2.9 is co-owned
with `d-bundle` and `n-structural-mutations`; the `verify-feature-tags.sh`
breakage below is repo-wide by construction.

## Two process points that need an owner

Neither is charged to this candidate; both outlive it.

1. **The mandatory static pre-gate is red for every feature in the repository.**
   `PROCESS.md` §"Feature targeting and gate pyramid" requires
   `features/.agents/scripts/verify-feature-tags.sh` to be run "before any
   audit, correction, or review". It exits 1 on both `513b366` and `4f5921e`
   because `features/gateway-delegated-client-surfaces.feature` starts with
   `@wip @g4 @wasm @cli` instead of `@gateway-delegated-client-surfaces`. Every
   role in every feature is currently obliged to run a gate none of them can
   pass. Either the script must tolerate `@wip` features, or that file must
   carry its canonical tag, or `PROCESS.md` must say what a role does when the
   pre-gate is red for an unrelated file. Expected owner: the process owner
   (Mathieu), via the orchestrator. This review did not work around it, did not
   repair it, and did not let it block a verdict that does not depend on it.

2. **A wording contradiction in this round's routing.** The task routing for
   this review lists the permitted resulting statuses as `REVIEW_ACCEPTED`,
   `CORRECTION_REQUESTED` or `DECISION_REQUIRED`, while
   `audit-b-derivation/SKILL.md` lists `CORRECTION_REQUESTED`,
   `DECISION_REQUIRED` or `IMPACT_REVIEW_REQUESTED`. `PROCESS.md` §"Manual
   lifecycle" settles it: `REVIEW_ACCEPTED → IMPACT_REVIEW_REQUESTED →
   COMPLETE`, so the two are consecutive states of the same transition and the
   skill simply names the next actionable one. `STATE.md` below records
   `REVIEW_ACCEPTED` with the impact review as the next role, which satisfies
   both readings. Flagged because `PROCESS.md` is the law and a reader
   comparing the two lists would find them inconsistent.

## Explicit rulings requested by state

### On the three divergences the corrector declared

1. **`verify-feature-tags.sh` red at the baseline** — **accepted as
   pre-existing, not charged to this candidate.** I ran the script myself on
   both immutable revisions; it exits 1 on each, on the same file, whose first
   line is byte-identical at both. It does not touch `@b-derivation`, which the
   script accepts. It does not block acceptance of two findings that do not
   depend on it. It does need its own owner, and I escalate it above rather
   than absorbing it into this feature's record.
2. **`b2_derivation.rs:2` left untouched** — **the disclosure is accepted, the
   characterisation is not.** "Outside the assigned scope" is right as a
   description of the corrector's mandate — the decision record named exactly
   one action — but wrong as a description of the defect: that line is the same
   retracted claim about the same five values. It is therefore not dropped. It
   becomes `BDER-013`, `OPEN`, tracked in the public audit, and it does not
   prevent `BDER-008` from meeting its written closure criterion.
3. **Gates run on a container export** — **accepted, with the same limitation
   restated for this role.** The device has no Rust toolchain, so neither the
   corrector nor I could run any gate on the workstation. I did not rely on the
   corrector's export: I produced my own with `git archive`, verified its
   SHA-256 on both sides, and rebuilt from scratch. That is as close to the
   immutable revision as this environment allows, and the report says so
   wherever a number appears.

### On README rule 3 and the `ownership.json` re-pin

State asks for an explicit ruling on whether re-pinning the digest is the
intended reading of README rule 3. **It is.**

Rule 3 reads: "Frozen once green. A merged vector never changes; a spec change
that would alter one requires a new vector id and an explicit spec redline."
The subject of that sentence is the vector — the expected values that any
implementation must reproduce byte for byte, which is what rule 3 exists to
protect and what a new vector id would be needed for. No expected value moved;
I verified that key by key against the baseline and, independently, by
recomputing all five keys from the committed inputs. No spec change occurred, so
no new vector id and no redline are owed.

The SHA-256 pin in `ownership.json` is not rule 3 itself; it is the enforcement
mechanism `vectors_ownership.rs` gives it, and it necessarily covers whole-file
bytes, prose included. Reading rule 3 as forbidding the re-pin would mean a
vector's documentation could never be corrected once merged — which would have
made this round's human decision impossible to execute, and would freeze a false
provenance claim into the repository permanently. That cannot be the intent of a
rule whose companion, rule 1, requires the `description` to name the generator
used.

The correct corollary, and the one this review records, is about what the pin
can and cannot prove: because it is a whole-file digest, a green
`vectors_match_their_pinned_digests` proves the file matches its manifest
**after** the change, never that only prose moved. The "no value changed"
guarantee therefore rests on the field-by-field comparison and the independent
recomputation in this report, not on the harness. Any future round that edits a
vector's prose owes the same explicit demonstration.

## Limits of this conclusion

- **The gates did not run on the owner's workstation.** They ran in a container,
  on `git archive` exports of `513b366` and `4f5921e` whose SHA-256 was checked
  on both sides. The exports reconstruct the revisions from Git objects, so they
  are the revisions and not a dirty worktree — but no gate output in this report
  was produced on the machine that holds the repository.
- A first gate attempt silently reused a pre-existing build belonging to the
  corrector's earlier session in the same container. That result was discarded
  and every number here comes from a rebuild whose compilation lines are quoted.
  Anyone reproducing this must use a clean `CARGO_TARGET_DIR`.
- `VERIFIED` here means the two assigned findings meet their written closure
  criteria under independently reproduced evidence. It does **not** mean the
  tag-view Rule is `PROVEN` in the `PROCESS.md` sense: its scenario remains a
  bounded sample (`BDER-012`) whose anchors have no external witness
  (`BDER-007`), and the behavioural half of §2.9 remains unproven anywhere in
  the corpus until the `d-bundle` cycle discharges its widened debt.
- I did not audit `d-bundle.feature`, `aithos-bundle`'s anchoring code, or any
  finding outside `BDER-006` / `BDER-008`. My statement that `d-bundle.feature`
  carries no tag-view or `wrap` scenario is a coverage observation, not an audit
  of that feature.
- Three contaminations are disclosed in Pass A, including one procedural slip of
  my own. None of them supplied a verdict from another role, and no Pass A
  judgement depends on the baseline.
- The global Cucumber, workspace, `fmt` and `clippy` results in this report are
  the corrector's, reported as such and not reproduced by this role.

## Next action

`STATE.md` → `REVIEW_ACCEPTED`; next role: the global impact reviewer, skill
`review-gherkin-impacts`, on the accepted round-2 range `513b366..4f5921e`.
Integration into `main` remains a human decision and is not performed here.

Carried forward, visible and unclosed: `BDER-007` (independent B2 generator
lot), `BDER-010` (informative), `BDER-012` (bounded negatives), `BDER-013` (new,
opened by this review), the `d-bundle` widened `TARGETED` debt, and the
repo-wide `verify-feature-tags.sh` breakage awaiting a process decision.

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

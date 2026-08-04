# Independent review — `c-headers` lot A, candidate `5905bec`

Role: independent reviewer. Material: the `git archive` extract of the candidate
at `/root/work/review-lot-a`, no `.git`. `/root/work/aithos-core` was **not**
opened at any point, before or after the freeze.

Scope: `CHDR-001`, `CHDR-002`, `CHDR-009`, `CHDR-013`, `CHDR-014`, `CHDR-019`,
`CHDR-021`, `CHDR-025`. `CHDR-016` is out of lot A and is not judged here.

Pass A was frozen before any evidence was returned to me and before any history
was disclosed. Pass B material — twenty transcripts on run `2026-08-04-r6`, and
the three commit facts in §9 — was delivered afterwards. Where Pass B moved a
verdict or contradicted a Pass A claim, §8 says so explicitly; nothing has been
silently rewritten.

Contents: §1 the eight findings · §2 the `CHDR-019` audit-mutant debt · §3 the
Gherkin markers · §4 what I could not verify · §5 regressions and new findings ·
§6 `CHDR-040`, a finding against the process · §7 what I attacked and could not
break · §8 Pass A/B reconciliation · §9 disclosed history · §10 verdicts.

**I ran nothing.** Every command below was named by me and executed by the
orchestrator, which hashed and journalled each transcript. Every behavioural
claim cites the `evidence_id` I was given.

---

## 0. Method, and what the evidence can and cannot be

This lot changes no production code. Verified, whole extract, all layers:
`grep -rn "CHDR-" --include="*.rs" rust/crates/*/src/` returns nothing; every
`CHDR-*` attribution lives under `rust/crates/{aithos-core,aithos-bundle}/tests/`.
The corrector's framing — *no production behaviour was wrong; what was missing
was proof* — is therefore true of the diff, and it has a hard consequence: **the
feature gate cannot go RED on this lot**, so a green gate is worth nothing here.
`ev-1335c8f1` (green, 1 feature / 4 rules / 8 scenarios / 28 steps) is recorded
as a precondition, not as support for any verdict.

The only honest RED is a **named mutant** under which the old assertion is green
and the new one red. Twelve were named and all twelve were run. Three of them —
`M3`, `M12`, and the feature-gate arm of `M10` — were designed to make the lot
look *inert*, not to confirm it. Two of those three came back green, which is
what makes the other nine mean something.

### The mutants, as exact patches

All are single edits under `rust/crates/aithos-core/src/`. None touches a test.

| Id | File / symbol | Edit |
|---|---|---|
| `M1` | `seal.rs`, `fn aad` `:28-29` | delete the `0x00` separator and `key_version.to_string()` bytes; `let _ = key_version;` |
| `M2` | `header.rs`, `Header::rotate` `:235-248` | seed the new version with `key_versions[new_version-1].lines.clone()`, then extend with `build_lines(…)` |
| `M3` | `seal.rs`, `fn kek` `:84` | `Hkdf::<Sha256>::new(None, shared)` → `…new(None, &[0u8; 32])`. **The mutant the public audit states for `CHDR-019`, transcribed literally.** |
| `M4` | `header.rs`, `append_line` | push the new line twice |
| `M5` | `header.rs`, `append_line` `:211` | `push(…)` → `insert(0, …)` |
| `M6` | `header.rs`, `append_line`, before the push | flip one hex character of `c` on every pre-existing line whose `to != OWNER_LABEL` |
| `M7` | `header.rs`, `Wrap::seal` `:417` | `let node: &str = "/e/self";` — shadows the parameter, so AAD and stored field move together and `Wrap::open` stays symmetric |
| `M8` | `derive.rs`, `node_key` | `Leaf::Section(sid) => { let _ = sid; key }` |
| `M9` | `header.rs`, `build_lines` `:110` | seal the **owner** line to `XPublicKey::from([0x77u8; 32])`, grantee lines untouched |
| `M10` | `header.rs`, three deletions at once | drop `check_owner_line` from `rotate` `:234`; drop the owner check from `check_rotation` `:357-362`; `validate` → `Ok(())` |
| `M11` | `seal.rs` `:15` | `PURPOSE_HEADER_LINE` → `b"aithos-core/v1/header-lineX"` |
| `M12` | `derive.rs`, `derive_key` `:17` | return `*key_material` — derivation reduced to identity |

Baselines: `ev-14592971` (`verify-feature-tags.sh`, green),
`ev-1335c8f1` (feature gate, green, 1/4/8/28), `ev-1a19fdf4`
(`c1_header_seal` + `g2_rotation` + `g3_move`, green).

---

## 1. The eight findings

### `CHDR-001` — **`VERIFIED`**

**Defect.** Scenario 4 claims "bound to its node **and** version" and varied only
the node: both `Header::build` calls land on version 1 and the open is at version
1, so the `key_version` component of `line_aad` was identical on both sides.

**Correction.** `replay_line_other_node` (`cucumber.rs:8215-8248`) records three
attempts: a control open on the origin header; the node half (same line grafted
into a `NODE_OTHER` header, opened at v1); and the version half (`:8239-8248`) —
the same line inserted as `key_versions["2"]` of the *origin* header, same
`subject_did`, same `node`, same `kid`, opened at v2. In the third attempt
`key_version` is the sole varying input to `line_aad`.

**Mutant demanded.** `M1` — remove `key_version` from `aad()`. Also the audit's
own §11 lot-5 expected RED.

**Evidence: `ev-9ba93af7`.** 7 passed / 1 failed, scenario 4 only, at
`attempt 2 after the mutation must be rejected, got Ok([119; 32])`.

**What it actually shows, and why it discriminates.** `0x77` is `DK`
(`cucumber.rs:262`), so the v2 replay *succeeded* under `M1` — the version
binding was the only thing stopping it. The attempt index is the discriminator:
**attempt 1, the cross-node graft, still failed**, because the node still varies
under `M1`. The pre-correction `Then` was
`assert!(w.opened.last().unwrap().is_err())` over a `When` that recorded only the
cross-node graft — so under `M1` the old scenario is green. One transcript
carries both arms: the failing index proves the new assertion bites, the passing
index proves the old one did not. Verdict settled.

---

### `CHDR-002` — **`VERIFIED`**

**Defect** (post-reconciliation, P3). `opening_rejected` was a bare `is_err()`
with no known-good base **in the scenario's own body**. Scenarios 3 and 4
asserted failure after a mutation without having asserted success before it; a
fixture regression making the owner line permanently unopenable left both green.

**Correction.** `corrupt_line` (`:8199-8213`) opens once before flipping the hex
character and once after. `replay_line_other_node` (`:8224`) records a control
open of the very line it is about to steal, on its own header, before the graft.
`opening_rejected` (`:12506-12528`) asserts `opened.len() >= 2`, then
`opened.first() == Some(DK)` under the message *positive control: the targeted
line must open on its own header BEFORE the mutation*, then `is_err()` on every
later attempt. That is the closure criterion verbatim.

**Mutant demanded.** `M9` — the audit's own hypothesis made executable: the owner
line sealed to a foreign key. `check_owner_line` compares `Recipient` public keys
and runs before `build_lines` (`header.rs:164`, `:169`), so I3 still passes and
the header still builds; only the owner line's seal is dead.

**Evidence: `ev-11dee753`.** 5 failed / 3 passed. Scenarios 3 and 4 red at the
positive control. **Scenario 5 green.**

**What it actually shows.** Scenario 5 staying green is the control I specified
in advance: it proves `M9` did not accidentally disable the I3 gate and that the
two REDs are attributable to the seal, not to the build. Scenarios 1, 7 and 8
also fell — expected collateral of an unopenable owner line, and precisely the
audit's stated reason for downgrading this finding to P3 (no production mutant
survives the whole `Rule`, because `owner_opens` falls). The finding is about
per-scenario proof strength, and the per-scenario evidence is that under `M9` the
*old* scenarios 3 and 4 were green — their last attempt was still `Err`, since
under `M9` nothing opens — while the new ones fail at a message naming the
control. Verdict settled.

---

### `CHDR-009` — **`VERIFIED`**

**Defect** (post-reconciliation). No test in the repository asserted
`Error::MissingOwnerLine` anywhere but the `build` gate; and
`vectors/g2-rotation.json:17` declares `"missing_owner_must_fail":
"MissingOwnerLine"` — a normative case — which the `G2` struct did not even
deserialize.

**Absence claim, with its search.** Whole extract, all layers, not `rust/**`
only. `grep -rn "missing_owner_must_fail" .` → `vectors/gen-g.py:134`,
`vectors/g2-rotation.json:17`, `g2_rotation.rs:{35,152,159,183}`, plus prose in
`docs/`. Exactly one consumer, and it is the new test. `grep -rn
"MissingOwnerLine" .` over `*.rs *.json *.md *.py *.toml *.feature *.sh *.yml`
→ four production sites in `header.rs` (`:92`, `:358`, `:374`, `:394`), the
variant in `error.rs:60`, three vector cases in `vectors/c3-owner-line.json`,
and four assertion sites in `g2_rotation.rs`. No other test in either crate
asserts the variant.

**Correction.** Three tests in `g2_rotation.rs`, one per un-exercised gate, each
asserting the **typed** variant rather than a string:
`check_rotation_refuses_a_new_version_without_the_owner_line` (`:156`, consumes
the vector field and builds a v2 whose kids are a strict subset of v1 so the
smuggling branch is provably silent);
`rotate_refuses_a_survivor_set_without_the_owner` (`:189`, plus
`!header.key_versions.contains_key("2")` — no partial effect);
`validate_refuses_a_key_version_without_the_owner_line` (`:224`, with its own
positive control at `:237`).

**Mutant demanded.** `M10` — all three gates deleted at once.

**Evidence: `ev-dce43f1c`** (`--test g2_rotation`): 4 passed / 3 failed, red on
exactly the three new test names. **`ev-4ed2d6f3`** (feature gate under the same
mutant): **fully green, 8/28**.

**What it actually shows.** `ev-dce43f1c` is the positive arm and its precision
matters: 4 passed means `survivor_set_is_old_minus_revoked`,
`a_smuggled_recipient_is_rejected`, `a_clean_rotation_is_accepted` and
`uplink_wrap_bytes_match_python` were untouched, so `M10` reached exactly the
three gates named and no further. `ev-4ed2d6f3` is the old-assertion arm and it
is uncontested: with all three I3 gates gone, the entire feature is green. That
is the finding, stated as an experiment rather than an argument — the Gherkin
never observed these gates failing. Verdict settled.

**One thing this does not prove, and I do not claim it.** The new
`check_rotation` test calls the gate directly on a hand-built header. The audit
records that in both real callers (`revoke.rs:214`, `vault.rs:404`) the owner
branch is *dominated* by `check_owner_line` inside `rotate`. So the gate is
proven; its reachability from production is not, and I did not trace
`revoke.rs`/`vault.rs` line by line — outside this feature's pilot limits
(`DOMAIN.md` § *Pilot limits*). The closure criterion asked for the gate. The
rest belongs to `CHDR-024`/`CHDR-036`.

---

### `CHDR-013` — **`VERIFIED`**

**Defect.** "Grant is one appended line" was asserted nowhere — neither cardinal
nor position. `owner_line_untouched` did `.find(|l| l.to == "owner")` against one
saved line, blind to a surnumerary line, a duplicate and a reordering.

**Absence claim, with its search.** `grep -rn "lines\.len()" --include="*.rs" .`
over the whole extract → `log.rs:143` (audit-log lines), `i1_concurrency.rs:123`,
`cucumber.rs:1267` (manifest `n`), `cucumber.rs:17569`, `cucumber.rs:19231`,
`gamma.rs:919-922`. The only header line-cardinal assertion in the extract is the
new `cucumber.rs:12552`. The absence claim was true; it is now closed by exactly
one site.

**Spec, quoted to its end.** `spec/03-headers.md:66-72`:

> ```
> 1. Open the node's current DK (own line).
> 2. Seal DK to the recipient's X25519 key → one new line.
> 3. Append it to key_versions[current].lines. Publish the edition.
> ```
> Content untouched, other lines untouched, DK unchanged. This is the frequent, cheap
> operation. (If old versions still hold un-re-encrypted content the recipient should
> read, the issuer adds a line to those versions too — §3.5.)

"Append" and "one new line" are the cardinal and the position; "other lines
untouched" is the prefix equality. The parenthetical is quoted because it is the
one clause permitting a grant to touch another key version — and only by adding
to it, never by rewriting it, so it weakens neither assertion.

**Correction.** `owner_line_untouched` (`:12540-12570`) asserts
`lines.len() == saved.len() + 1` (*a grant appends EXACTLY one line (§03.3)*) and
`&lines[..saved.len()] == &saved[..]` (*every pre-existing line stays
byte-identical AND keeps its position*), both against `w.saved_lines`, the whole
pre-append vector snapshotted at `:7626`.

**Mutants demanded.** `M4` (double push — cardinal) and `M5` (`insert(0, …)` —
position; the audit's §11 lot-4 expected RED).

**Evidence: `ev-a1f966ca`** — 7/1, scenario 6, `a grant appends EXACTLY one line
(§03.3)`. **`ev-1b889900`** — 7/1, scenario 6, `every pre-existing line stays
byte-identical AND keeps its position`.

**What it actually shows.** Both mutants are invisible to the pre-correction
assertion: `find(|l| l.to == "owner")` returns the untouched owner line whatever
is pushed after it (`M4`) and is order-blind (`M5`). In both runs the other seven
scenarios passed, and within scenario 6 `new_grantee_opens` passed — the failure
is isolated to the cardinal and the position, not to the seal. Verdict settled.

**Nit, not charged.** `assert_eq!(header.key_versions.len(), 1, "a grant creates
no key version")` is equivalent to its message only because this fixture starts
at one version; against a multi-version header (`spec/03-headers.md:115-123`) the
assertion would be wrong while the message stayed right. A snapshot of the
pre-append count would be exact. Cost of leaving it: nil — no scenario of this
feature grants on a multi-version header.

---

### `CHDR-014` — **`VERIFIED`**

**Defect.** The `Given` was `sealed_header_owner_only`, sealing to
`&[owner_rec()]`. "Every other line untouched" degenerated to "the only other
line is untouched": with `n = 1` there is no rest to perturb and no order to
permute, so an `O(1)` push and an `O(n)` rebuild-and-reseal are
indistinguishable.

**Correction.** A new `Given`, `sealed_header_owner_and_reader` (`:7615-7628`),
sealing to `[owner_rec(), grantee_rec("g1", 0x21)]` and snapshotting the whole
`lines` vector. The Gherkin phrase at `c-headers.feature:68` is unchanged and now
describes the state actually built. `append_grantee_line` (`:8269-8278`) appends
`g2`, distinct from the `g1` the `Given` carries, and `new_grantee_opens`
(`:12480-12489`) was split out of `grantee_opens`, which previously served both
phrases. The `Then` also asserts `saved.len() >= 2` — *'every other line' needs
at least two pre-existing recipients to have a non-degenerate referent* — so a
regression of the fixture back to one recipient is self-detecting.

**Mutant demanded.** `M6`, chosen specifically because it is a **no-op on the old
fixture**: it perturbs only lines whose `to != OWNER_LABEL`, and the old
single-recipient header had none. It is the mutant the degenerate `Given` was
structurally unable to see, which is this finding's exact claim.

**Evidence: `ev-b3ccaaf3`.** 7/1, scenario 6, at `every pre-existing line stays
byte-identical AND keeps its position`.

**What it actually shows.** The RED lands on the **whole-vector prefix
equality**, not on the owner-line check — the distinction `CHDR-014` names, and
the reason `M6` excludes the owner line on purpose. Even the pre-correction
`find(|l| l.to == "owner")` assertion would have been satisfied under `M6`; under
the pre-correction *fixture* the mutant edits nothing at all. The old scenario
was green by construction, not by luck. Verdict settled.

---

### `CHDR-019` — **`VERIFIED`** on the defect as stated; see §2 for the audit's mutant

**Defect.** `revoked_cannot_open` called `Header::open(DID_C, 2, "g1",
&xsk(0x21))`. `Header::open` filters `kv.lines.iter().filter(|l| l.kid == kid)`
(`header.rs:266`); v2 carried only `owner-kex` and `g2`, so the loop was empty,
`open_line` was never reached, and the revoked's secret was passed and never
used. The rejection came from a field the spec declares non-authorizing, and no
assertion read `key_versions["2"].lines`.

**Spec, quoted to its end.** `spec/03-headers.md:33-35`:

> `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
> a routing hint only — the seal is what grants. Recipients try lines addressed to
> their `kid`. No verifier decides anything from `to`.

and `spec/03-headers.md:56-59`, which the audit does not quote and which decides
the shape of the fix:

> `kid` orders the attempts and nothing else: a reader that finds no matching line MAY try the remaining
> lines, and a successful unseal — never a label — is what proves the line was its own.
> No network, no per-read state.

That `MAY` is why `Header::open`'s `kid` filter is not itself a defect — the spec
permits stopping at the routing hint — and why the corrected `Then`'s loop over
`v2.lines` is the right shape: it does what a reader *may* do, and so reaches the
seal.

**Correction.** `revoked_cannot_open` (`:12590-12617`) replaces one assertion
with three: structural (`v2.lines.iter().all(|l| l.kid != "g1")`); mechanical
(`header.check_rotation(2, &owner_kid_c())`); capability (`header.open(DID_C, 2,
&line.kid, &xsk(0x21))` for every line routable in v2). The `expect` message on
the second reads *survivors ⊆ previous, owner kept*, which is what
`check_rotation` implements (`header.rs:347-356`, a `BTreeSet` containment test)
— **not** what `spec/03-headers.md:109-111` requires ("the new version's lines
MUST equal the previous lines minus the revoked"). The corrector did not
overclaim; the ⊆-versus-equality gap is `CHDR-024`'s recorded out-of-verdict note
and stays open.

**Mutant demanded.** `M2` — `rotate` carries the previous version's lines
forward. The audit's own §11 lot-3 expected RED.

**Evidence: `ev-39f02b30`.** 7/1, scenario 7 only:
`the revoked gets NO line in the new version: ["z6LStLK2kx…", "g1", "g2", "z6LStLK2kx…", "g2"]`.

**What it actually shows.** The printed kid list is five entries — v1's owner,
`g1`, `g2`, followed by v2's owner and `g2` — which confirms `M2` was applied as
named and no more heavily. `survivor_opens` and `owner_opens_new` passed:
`Header::open` tries every `kid`-matching line and returns the first that opens,
so the stale v1 copy is skipped. Scenario 8, which also rotates, passed. And the
old assertion is green under `M2` by construction: the carried-forward `g1` line
is bound to `line_aad(did, node, 1)` and is opened at version 2, so
`Header::open(…, 2, "g1", …)` still returns `Err`. Old green, new red, single
scenario, single assertion. Verdict settled.

**Two things the transcript teaches that I want on the record.**

1. `check_rotation(2)` does **not** catch `M2` — `g1` is present in the previous
   version, so containment holds. The structural assertion, not the mechanical
   one, is load-bearing here. The `check_rotation` call still earns its line: it
   closes `CHDR-024`'s closure criterion as a by-product.
2. The capability loop caught nothing in any of the twelve runs. I could not
   construct a production mutant it kills that nothing else kills, and I say so
   rather than crediting it. The mutant it is obviously aimed at — a v2 line
   carrying a survivor's `kid` but sealed to the revoked's key — is **not
   expressible as a production-code edit in this codebase**: a `Line` stores only
   `to`, `kid`, `epk`, `n`, `c` (`header.rs:43-50`), so `rotate` has no access to
   the revoked's public key and no edit to it can produce that line. The `kek`
   mutants that would open a line to a wrong holder are caught earlier — see §2.
   The loop remains cheap insurance against a state the *bundle* layer can
   produce (`CHDR-032`, duplicate `kid` in a key version, enforced nowhere), and
   I keep it. But it is unproven, and an unproven assertion should be labelled,
   not counted.

---

### `CHDR-021` — **`VERIFIED`**, with a named residual kept alive under this finding

**Defect** — the one carrying scenario 8's `SEMANTIC_FALSE_POSITIVE`.
`post_uplink_wrap` sealed `Wrap::seal(DID_C, NODE_A, &PARENT_KEY, CHILD_NODE, 2,
&DK2, non(9))` and the `Then` reopened the **same in-memory object** with the
**same literal** `PARENT_KEY`. `Wrap::open` recomputes its AAD from its own
`self.node` and `self.key_version`, so the assertion could not detect a wrap
posted under the wrong node or version. `w.header` stayed `None` throughout: no
derived node, no rotation, no content tree.

**Spec, quoted to its end.** `spec/03-headers.md:87-95`, step 2bis:

> Derivation up-link. If the rotated node N is derived from a parent node P that
> the rotator holds, it also publishes an up-link wrap: seal(DK'_N) openable via
> K_P — same primitive as a tag wrap (AAD purpose `tagwrap`, §00.3), bound to
> subject_did ‖ N ‖ new key_version. The wrap restores the parent→child derivation
> path broken by the fresh random DK', so holders of P (or of any ancestor of P)
> keep reading N by derivation without needing a line of their own. If the rotator
> holds exactly N but not P, it instead seals DK'_N individually to the current
> holders of P (public keys read from P's header); the first manager of P that
> later acts posts the definitive wrap.

The final conditional is quoted because the corrected scenario takes the first
branch — the rotator holds P. That is a legitimate reading of the scenario title;
the second branch is exercised by no scenario of this feature and is not charged
to this lot.

**Correction.** `derived_node_rotated` (`:7660-7693`) builds a parent folder and
a child section under it as real `NodePath`s, derives the child key with
`node_key(&zone_dk, &child)` off the B2 zone key, builds the child's header at v1
**under that derived key**, then performs a real `Header::rotate` to `DK2`
dropping `g1`. `post_uplink_wrap` (`:8297-8321`) takes the via key from
`node_key(&zone_dk, &parent)` — derived, not a literal — and reads the wrapped
node and version off the rotated header rather than receiving them as literals
the `Then` will hand back. `parent_recovers_via_wrap` (`:12638-12695`) asserts:
(a) the child key was one derivation from the parent and v1 sealed *that* key;
(b) the rotation moved the child off it, where the new key is obtained by opening
the header — independently of the wrap; (c) `wrap.node == header.node`,
`wrap.key_version == header.latest_version()`, `wrap.via == parent`; (d) the wrap
yields the value from (b) under a key the holder derived.

(d) equated against (b)'s independently obtained value is what stops the
assertion being a round trip on itself: two routes computed separately, then
compared. The scenario now proves the "cut, then restored" pair its title claims.

**Mutants demanded.** `M7` — `Wrap::seal` seals and stores under a constant node,
chosen deliberately to be **symmetric** so the wrap still round-trips and the old
assertion stays green. Plus `M8` for the derivation half.

**Evidence: `ev-c78772c4`** — 7/1, scenario 8, `wrap posted under the wrong
node`. **`ev-16a836a9`** (`M8`) — 7/1, scenario 8, `the child key was reachable
from the parent by one derivation`.

**What it actually shows.** Under `M7` the wrap still opens and still yields the
key it sealed — that is what "symmetric" means — so the pre-correction `Then`
(`wrap.open(…) == DK2`) was green. Only the new binding assertion sees it: exactly
the gap the finding named. `ev-16a836a9` proves the derivation half is
load-bearing rather than decorative — the old scenario computed no `node_key` at
all and was green under `M8`. Verdict settled.

**Residual, kept under this finding rather than given an identifier —
`ev-ec9412a7`.** The audit's `CHDR-021` block names a surviving class: *toute
mutation symétrique de `aad()` et de `derive_key(CTX_WRAP_KEY, ·)`*, including
derivation reduced to identity. I asked for it *after* correction. Under `M12`
the feature gate is **fully green, 8/28**. The class survives the corrected
scenario exactly as it survived the old one.

I argued this both ways and land on *note, not identifier*:

- **For an identifier:** a residual buried in a block about to be marked
  `VERIFIED` risks vanishing from the actionable set, and `PROCESS.md:236-238`
  strips the Gherkin marker on `VERIFIED`, so the feature file will no longer
  point at it.
- **Against, and this decides it:** the audit *already states this class* inside
  the `CHDR-021` block, together with the reason it is contained — the symmetric
  mutations are caught outside Gherkin by the byte pins `g3_move.rs:157-159`
  (`wrap_aad_hex`) and `g2_rotation.rs:112-114` (`wrap.c == cipher_hex`). Minting
  `CHDR-041` would create a second, competing statement of a published finding,
  and the identifier-collision warning at §1 of the audit is already one such
  mess. Worse, it would charge lot A for a property **no scenario of this feature
  claims**, which `DOMAIN.md` § *Pilot limits* forbids.

**Containment: measured, not read — `ev-cbce8aa0`.** At the time I made that
call, "the byte pins catch it" was a reading of three files, not a result, and I
said so and named the command. It has now been run. Under `M12`, with
`--no-fail-fast` across the three binaries:

- `c1_header_seal` — 2 passed / 1 failed, `c2_wrap_roundtrip_and_cross_check`;
- `g2_rotation` — 6 passed / 1 failed, `uplink_wrap_bytes_match_python`;
- `g3_move` — 1 passed / 2 failed, `derivation_below_moved_node_is_stable` and
  `new_path_bindings_and_parent_wrap`.

All three pins I named fell, and the `g3_move` half fell as two tests rather than
the one I predicted. So the class **is** caught — by the pinned vectors, and by
nothing in Gherkin. `ev-ec9412a7` and `ev-cbce8aa0` are a pair and only mean
something together: **green where this feature's scenarios look, red where the
vectors look.** That is the precise shape of the residual, and it is now a
measurement rather than an inference. My stated flip condition — *a green run
there would promote this to a finding of its own* — did not fire.

**The condition I attach to the call.** The `CHDR-021` block's surviving-mutant
paragraph must be **retained verbatim** when the finding is marked `VERIFIED`,
with `ev-ec9412a7` and `ev-cbce8aa0` appended together — the audit itself says at
§13 that no mutation experiment was conducted by that cycle, so this pair is the
train's first measurement of the class. If the paragraph is dropped on closure,
the residual is lost and I withdraw this call in favour of an identifier.

**A near-miss on the way there, recorded because it was nearly my error.** The
first run of that command — `ev-debade53`, written exactly as I named it, without
`--no-fail-fast` — reported a **single** failure, in `c1_header_seal`. Read at
face value that transcript says `g2_rotation` and `g3_move` were green under
`M12`, which would have flipped this call and opened a spurious finding. They
were not green: `cargo test` fails fast **across test binaries**, so neither ever
executed, and their absence from the transcript is not a result. The orchestrator
caught that and ran it again with `--no-fail-fast`; I did not catch it, and I
would have drawn the wrong conclusion from `ev-debade53` alone. The command was
mine, so the defect in it is mine. It is raised as `CHDR-042` in §5, because the
same flag is missing from the regression command `DOMAIN.md` tells every
corrector to run.

**Two tautologies I noticed and am not charging.** `assert_eq!(child.to_string(),
header.node)` (`:12684`) compares two values the `Given` set from the same
`NodePath`. And `wrap.via` never enters `wrap_aad` (`seal.rs:41-43`) — it is
stored and read by no production path. Both cost nothing and document intent.

---

### `CHDR-025` — **`VERIFIED`** on both halves; one residual raised as `CHDR-038`

**Defect.** `c1_fail_closed` had **no positive control in its own body**: the
tuple `(sk, epk, c, n)` comes from the vector, and that it opens under the
nominal AAD was established only in *another* test function. Any symmetric
mutation of `line_aad` changes the AAD on both sides, so the four negative
assertions keep passing for an entirely different reason. Beyond that, only byte
pins defended the `key_version` component — and the first of those rested on a
vector whose generator did not exist in the repository.

**Half 1 — the positive control.** `c1_header_seal.rs:92-107` opens the untouched
tuple under the nominal AAD and asserts it yields `v.dk_hex`, before any of the
four negatives, with a comment stating the audit's own rule: the vacuity is
per-body and cannot be repaired from another test function.

**Mutant demanded.** `M11` — change `PURPOSE_HEADER_LINE`, i.e. a symmetric AAD
mutation. `c1_fail_closed` does not re-seal; it decrypts the **frozen vector
ciphertext**, so nothing opens under `M11`.

**Evidence: `ev-ad4db6a1`** (`-- --exact c1_fail_closed`) — RED at
`c1_header_seal.rs:103`, `positive control: the untouched tuple MUST open under
the nominal AAD: SealRejected("line does not open")`. **`ev-34e698d8`**
(unscoped) — 1 passed / 2 failed.

**What it actually shows.** The `--exact` scoping is the claim, not a
convenience: this is a per-body vacuity finding, so the discriminating run must
exclude `c1_owner_and_grantee_lines`, which would otherwise mask it. With the
pre-correction body — four negatives and nothing else — every one is satisfied
under `M11`, vacuously, and the test is green. `ev-34e698d8` shows the sibling
falling too, in a different function, which is the situation the finding
describes. Verdict settled on half 1.

**Half 2 — the independent-generation claim.** The criterion was *produce or
withdraw* it. It is produced: `vectors/gen-c.py` exists, its docstring states the
second-implementation rule (blake3 + PyNaCl + manual RFC 5869 HKDF, never the
Rust reference), and `check_c1()` (`:167-207`) reconstructs
`c1-header-seal.json`'s owner line, grantee line and C2 wrap and asserts them
byte for byte without rewriting the frozen file. The criterion is met on the
candidate. Pass B establishes it was met by lot B's base `5be3047`, not by lot A
— see §9; that changes the credit, not the verdict.

**Unexpected strengthening, from `ev-2e427d6e`.** See §8: under `M3` the new
positive control also falls. It is not only a differential base for the four
negatives — because it decrypts a *frozen* ciphertext, it is an **asymmetric pin
on the entire seal path**: KEK derivation, AAD construction and AEAD together.
Any mutation to `kek`, `aad` or the cipher breaks it, symmetric or not. That is
strictly more than the closure criterion asked for, and I only learned it from a
mutant I had aimed elsewhere.

---

## 2. The `CHDR-019` audit-mutant debt

**The claim under examination.** `docs/audits/features/c-headers.md`, in the
`CHDR-019` block:

> Régression survivante construite par un réfuteur, retenue : muter `kek`
> (`seal.rs:83-89`) pour que l'IKM HKDF n'intègre plus le secret DH laisse le
> nommage intact, `survivor_opens` et `owner_opens_new` verts, et rend la ligne
> `g2` ouvrable par quiconque connaît la clé publique de g2.

**Verdict: the stated mutant is wrong on the code, and the transcript settles it
without argument.**

**Evidence: `ev-c16f1a9a`** — the feature gate under `M3`, the audit's mutant
transcribed literally: **fully green, 8 scenarios / 28 steps**. Not one scenario
flips, before or after correction.

**The structural reason, in `seal.rs`.** `kek` uses all three of its arguments
(`seal.rs:83-89`):

```rust
fn kek(shared: &[u8; 32], epk: &XPublicKey, recipient: &XPublicKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let info = [KEK_INFO, &[0u8], epk.as_bytes(), recipient.as_bytes()].concat();
```

The mutant removes `shared` from the IKM and leaves `recipient.as_bytes()` in the
`info`. And `open_line` never *receives* a recipient public key — it **derives
one from the secret it was handed** (`seal.rs:117-120`):

```rust
    let epk = XPublicKey::from(*epk);
    let recipient_pub = XPublicKey::from(recipient_secret);
    let shared = recipient_secret.diffie_hellman(&epk).to_bytes();
    let cipher = XChaCha20Poly1305::new((&kek(&shared, &epk, &recipient_pub)).into());
```

So after the mutation the KEK remains a function of the opener's own key pair. A
line sealed to `g2` was sealed under `info(epk, g2_pub)`; a caller supplying
`xsk(0x21)` computes `info(epk, g1_pub)` and derives a different KEK. Through
`open_line` — the only door `Header::open` has (`header.rs:271`) — the line stays
openable by exactly one party, `g2`, precisely as before the mutation.

**The empirical disproof, which is cleaner than the structural one.** If the
audit's sentence were true — *openable by anyone who knows g2's public key* —
then scenario 2, `A non-recipient opens nothing`, would have gone red under `M3`:
`stranger_tries` (`cucumber.rs:8191-8197`) tries `xsk(0x99)` against every kid of
the header. `ev-c16f1a9a` shows it green. A stranger derives its own public key
into the `info` and still fails. The audit's security claim is falsified by an
assertion that predates this lot entirely.

**What is true, and what the audit conflated with it.** At the level of the raw
AEAD the weakness is real: with a constant IKM and a public `info`, anyone can
compute the KEK from `epk` and the recipient's public key and decrypt directly.
That is a sound cryptographic observation. It is not a *testable* one here,
because nothing in this repository reaches the ciphertext except through
`open_line`, and `open_line` is not a decryption oracle taking a KEK — it takes a
secret and rederives the public key from it. The audit states an out-of-API
property in the grammar of an in-API one, and it is the in-API form a corrector
would have had to make an assertion catch.

**Where the audit is nonetheless right, and where it contradicts itself.** The
mutant *does* survive scenario 7 — both before and after the correction — so it
is a surviving regression of that scenario. What it is not is evidence of the
capability the audit attributes to it. And the audit's own §11, lot 3, states the
correct mutant: *"injecter une ligne `g1` en v2 → doit tomber"*. The audit's §6 and its §11
disagree with each other, and **§11 is the one that is right on the code**:
`ev-39f02b30` is that mutant, and it flips scenario 7 alone.

**The correct mutants.**

1. *For the structural half* — the half the correction closes: `M2`, a `rotate`
   carrying the previous version's lines forward. Old assertion green (the copied
   `g1` line is bound to AAD v1 and fails at v2), new structural assertion red.
   Measured: `ev-39f02b30`.
2. *For the capability claim the audit's prose reaches for* — the minimal mutant
   that really opens a line to a wrong holder through the API must drop **both**
   the DH secret from the IKM **and** `recipient.as_bytes()` from the `info`. But
   that one is killed by `stranger_recovers_nothing`, a pre-existing assertion of
   scenario 2. So even the repaired form of the audit's mutant does not
   discriminate the new capability loop from what the feature already had — the
   same conclusion I reached independently in `CHDR-019`, by a different route.

**Correction to my own Pass A, from `ev-2e427d6e`.** I predicted that under `M3`
the only casualty in the repository would be the C1 byte pin. Wrong: **two** tests
fail, `c1_owner_and_grantee_lines` *and* `c1_fail_closed`, 1 passed / 2 failed.
`c1_fail_closed` falls because lot A's new positive control opens the pinned
vector ciphertext under the nominal AAD, and `M3` changes the KEK. This does not
touch the debt verdict — `ev-c16f1a9a` is the feature gate and it is green — but
it changes two subsidiary statements of mine and I flag both rather than absorb
them:

- my sentence "the only thing that falls is the C1 byte pin" is **withdrawn**;
- the audit's mutant *is* caught by the repository after all — at the
  conformance-vector layer, and now **twice** rather than once, the second catch
  being an incidental gift of lot A's `CHDR-025` work. So lot A did add detection
  power against the audit's mutant. Not in scenario 7, where the audit put it,
  but in `c1_fail_closed`. Worth saying plainly, because it is the one place the
  lot exceeded its brief.

**What I am not doing.** I am not editing `docs/audits/features/c-headers.md`.
The block belongs to the auditor and the owner. The correction it needs is
confined to the `CHDR-019` paragraph of the audit's §6: replace the `kek` mutant with the
`rotate`-carries-forward mutant already stated in §11 lot 3, cite `ev-39f02b30`
and `ev-c16f1a9a`, and either drop the "openable by anyone who knows the public
key" sentence or re-site it as an out-of-API observation about `kek`'s
construction, explicitly flagged as unobservable through `open_line`.

---

## 3. The Gherkin marker block

**The debt as handed to me was already discharged, and Pass B names who.**
`features/c-headers.feature:47-55` in this candidate does **not** carry
`DECISION_REQUIRED … neither is assigned to a corrector`. Search, whole
`features/` tree: `grep -rn "DECISION_REQUIRED\|chdr-007\|chdr-012"
features/*.feature` returns one hit, at `a-identity.feature:35`, unrelated.

At Pass A I recorded that I could not tell whether lot B's reviewer or lot A's
corrector had removed those markers, and declined to guess; the second case would
have been a diff outside the assigned scope. Pass B answers it: commit `c547ccd`,
*"review(c-headers): CHDR-007 and CHDR-012 VERIFIED — lot B accepted"*, lot B's
reviewer, which is what `PROCESS.md:307` assigns to that role — and its message
records the gate re-run after removal (`ev-cf4a9d62`, unchanged at 1/4/8/28). Lot
A's corrector, `03283b0`, touches `features/c-headers.feature` on **exactly one
line**: 68, the `Given` rebinding that `CHDR-014` required. **No scope violation.
`STATE.md`'s round-1 block is simply stale.**

**What is wrong with the block as it now stands.** Its prose asserts, of the
candidate, two things that are false on the candidate:

- *"Only the build-time I3 gate is exercised on its fail-closed side"* — false:
  `g2_rotation.rs:156`, `:189`, `:224`, proven live by `ev-dce43f1c`.
- *"the normative case declared by vectors/g2-rotation.json has no consumer"* —
  false: `g2_rotation.rs:35` deserializes `missing_owner_must_fail`, `:159`
  asserts it.

The same holds at `:33-39`, which still says *"both headers are built at version
1 and the open is at version 1, so key_version never varies"* and *"Outside
Gherkin the version binding is defended only by byte pins against vectors, never
by a behavioural differential (CHDR-025)"*. `ev-9ba93af7` and `ev-ad4db6a1`
falsify both. `PROCESS.md:229-231` requires markers to *"describe current,
actionable gaps"*; a marker asserting a closed gap is worse than no marker,
because the next role reads it as current.

**What it should say, given the eight verdicts.** The edit is not a deletion —
every block mixes lot A identifiers with unresolved ones.

`:21-27` (scenario 3) — `CHDR-002` closed, `CHDR-027` open:
```gherkin
    @audit-partial @chdr-027
    # AUDIT CHDR-027 — PARTIAL.
    # Detail: docs/audits/features/c-headers.md
```

`:33-39` (scenario 4) — `CHDR-001`, `CHDR-025`, `CHDR-002` closed, `CHDR-027`
open:
```gherkin
    @audit-partial @chdr-027
    # AUDIT CHDR-027 — PARTIAL.
    # Detail: docs/audits/features/c-headers.md
```

`:47-51` (scenario 5) — `CHDR-009` closed, `CHDR-010` and `CHDR-011` open:
```gherkin
    @audit-partial @chdr-011 @chdr-010
    # AUDIT CHDR-011, CHDR-010 — PARTIAL.
    # The I3 rejection is asserted through a string match on "I3" rather than
    # the typed variant, and the scenario's Given is empty.
    # Detail: docs/audits/features/c-headers.md
```

`:59-66` (scenario 6) — `CHDR-013` and `CHDR-014` closed; `CHDR-016` re-routed,
neither closed nor withdrawn, so its marker must survive **and name its new
owner**; `CHDR-015`, `CHDR-017`, `CHDR-018` open:
```gherkin
    @audit-partial @chdr-016 @chdr-015 @chdr-017 @chdr-018
    # AUDIT CHDR-016 — OPEN, re-routed 2026-08-04 to g-revocation and d-bundle
    # as chdr-016-grant-path (orchestrator QUEUE.yaml); CHDR-015, CHDR-017,
    # CHDR-018 — PARTIAL.
    # The production grant surface is still touched by no step of this Rule.
    # Detail: docs/audits/features/c-headers.md
```

`:75-80` (scenario 7) — `CHDR-019` closed, and `CHDR-024` closed as a by-product:
its closure criterion was *"invoquer `check_rotation(2)` dans le `Then` existant
du scénario 7"*, which `cucumber.rs:12603` now does. If the owner accepts that
by-product closure the whole block goes; if not, it keeps `@chdr-024` alone. It
must **not** keep *"'cannot open' is decided by the kid routing hint"*, which
`ev-39f02b30` shows is false on the candidate.

`:88-94` (scenario 8) — `CHDR-021` carried the scenario's
`SEMANTIC_FALSE_POSITIVE` verdict, so the scenario's verdict changes with it and
the **tag** changes, not merely its identifier list:
```gherkin
    @audit-partial @chdr-020 @chdr-026
    # AUDIT CHDR-020, CHDR-026 — PARTIAL.
    # No negative of the wrap by divergent AAD exists anywhere (CHDR-026); a
    # symmetric mutation of derive_key still survives this scenario (ev-ec9412a7).
    # Detail: docs/audits/features/c-headers.md
```

**Ownership.** `PROCESS.md:307` assigns marker removal on `VERIFIED` to the
reviewer, not the corrector, so lot A leaving these blocks untouched is correct
process and nothing is charged to it. What I do charge — `CHDR-037` in §5 — is
that the lifecycle has no rule for the window between `IMPLEMENTED` and
`VERIFIED`, in which the marker text is required to stay and guaranteed to be
false. **I have not edited the feature file.**

---

## 4. What I could not verify, and why

1. **The reachability of `check_rotation`'s owner branch from production.**
   `ev-dce43f1c` proves the gate; it does not prove that `revoke.rs:214` or
   `vault.rs:404` can reach it, and the audit records that both are dominated by
   `check_owner_line` inside `rotate`. I did not trace either file — outside this
   feature's pilot limits. Belongs to `CHDR-024`/`CHDR-036`.

2. ~~**Whether the `M12` class is contained by the byte pins.**~~ **Settled after
   this section was first written** — `ev-cbce8aa0`, four failures across the
   three vector binaries. Struck rather than deleted, because the sequence
   matters: this was a gap I declared and named a command for, and the command
   closed it. See `CHDR-021`.

3. **Clippy.** CI runs `cargo clippy --workspace --all-targets -- -D warnings`
   (`.github/workflows/ci.yml:34`); `DOMAIN.md` § *Final global gates* does not
   list it, and lot A adds several hundred lines of `--all-targets` code. I did
   not run it — a global gate, and `PROCESS.md:86` forbids me — so I cannot say
   the candidate is clippy-clean. Raised as `CHDR-039`.

4. **The workspace and the unfiltered Cucumber gate.** Not run by me, by rule.
   Whether lot A's edits disturb another feature rests on the corrector's own
   global gates, which are Pass B material I deliberately did not read. What I
   can say is structural and is in §5.

5. **The capability loop of `revoked_cannot_open`.** Twelve mutants, none killed
   by it. I could not construct one it kills that nothing else kills, and I have
   argued in `CHDR-019` why I believe none is expressible against this codebase.
   That is an argument, not a proof of impossibility. If someone produces one,
   the argument falls and the loop is vindicated.

6. **`CHDR-016`.** Not judged, by instruction. Its marker at
   `c-headers.feature:59` is still live and must stay.

7. **Whether the `PROCESS.md` clauses binding this review exist.** They do not,
   in this revision. See `CHDR-040`. I obeyed them anyway — material isolation,
   the disclosure gate, the evidence ledger — because the instruction and
   `STATE.md` both bind me and the proposal text is unambiguous. But I could not
   read them where I was told they were.

8. **Which of my twelve mutants a *human* would consider adversarial enough.**
   Every one was designed by me against a defect statement I had also read. A
   mutant author who had not read the closure criteria might find a gap I did
   not. This is the structural limit of a reviewer who writes his own mutants,
   and no transcript removes it.

---

## 5. Regressions and new findings

**No regression found in lot A.** The lot touches no production code (search in
§0). No step phrase it changed is shared with another feature: each of the twenty
phrases of `c-headers.feature` grepped across `features/*.feature` resolves to
`c-headers.feature` alone. The constants dropped with scenario 8 (`CHILD_NODE`,
`PARENT_KEY`) have no remaining reference (`grep -n "CHILD_NODE\|PARENT_KEY"
cucumber.rs` → one comment line).

**Cross-scenario leakage, checked rather than assumed.** `ProtocolWorld` is
`#[derive(Debug, Default, World)]` (`cucumber.rs:467`), so it is rebuilt per
scenario — but that is a reading, and `ev-1335c8f1` turns it into a measurement.
Scenario 2 pushes two `Err`s into `opened` and precedes scenario 3 in file order;
scenario 3's `opening_rejected` now asserts `opened.first() == Some(DK)`. Had
`opened` carried across, scenario 3 would be red. It is green. The new assertion
is, incidentally, a live detector of the leak the integration pass was meant to
look for.

Identifiers continue from `CHDR-036`, the highest in the public audit.

### `CHDR-037` — `OPEN`, P3 — the marker lifecycle has no `IMPLEMENTED` state

`PROCESS.md:232-234` admits `IMPLEMENTED` as a status warranting a Gherkin
marker; `:236-238` removes the marker only on `VERIFIED`. Between the two, the
marker's prose is required to stay and is guaranteed to describe a state the
candidate no longer has — as at `c-headers.feature:33-39` and `:47-51`, both
falsified above by `ev-9ba93af7`, `ev-ad4db6a1` and `ev-dce43f1c`. A reader
arriving mid-cycle cannot tell a live gap from a closed one.
**Closure criterion.** Either the marker carries the status inline
(`# AUDIT CHDR-009 — IMPLEMENTED, awaiting review`), or `PROCESS.md`
§ *Gherkin audit-marker lifecycle* states that a corrector updates the prose of
the markers it addresses. Cost: nil, alpha.

### `CHDR-038` — `OPEN`, P3 — the restored independent-generation claim is enforced by no gate

**File.** `vectors/gen-c.py`. **Symbol.** `check_c1()` at `:167-207`, invoked
unconditionally by `main()` at `:283-299`; the script also accepts `--check`,
which additionally verifies `c3-owner-line.json` byte for byte rather than
rewriting it.

`check_c1` is the artifact that settles half 2 of `CHDR-025`: it reconstructs
`c1-header-seal.json`'s owner line, grantee line and C2 wrap from a second
implementation (blake3 + PyNaCl + manual RFC 5869 HKDF) and asserts them against
the committed file without writing it. **Nothing runs it.** Searches, whole
extract: `.github/workflows/ci.yml` has five steps and none is Python;
`grep -rn "gen-c\|--check" .github/workflows scripts` → `cargo fmt … --check`
only; `grep -n "gen-\|generator\|python"
rust/crates/aithos-bundle/tests/vectors_ownership.rs` → no match.
`vectors/ownership.json:30-34` pins the vector's sha256 and `vectors_ownership.rs`
enforces the pin, which catches drift of the *file* but not divergence between
the generator and the Rust implementation — the exact thing `check_c1` exists to
detect. The claim `c1_header_seal.rs:2-3` makes is therefore reproducible on
demand and verified by no gate.

This generalises: `vectors/` holds 28 `gen-*.py` generators and I found no gate
that runs any of them.

**Not chargeable to lot A.** Pass B places `gen-c.py` at `5be3047`, lot B's base
(§9). Lot A inherited it. The finding stands on its own.

**Closure criterion.** A CI step, or a `#[test]` behind a feature flag, running
`python3 gen-c.py --check` from `vectors/`; and the same treatment generalised to
the other generators, or an explicit recorded decision that vector generators are
run by hand at authoring time only.

### `CHDR-039` — `OPEN`, P3 — the declared final gates omit the clippy gate CI enforces

`DOMAIN.md` § *Final global gates* lists `cargo test … --test cucumber`,
`cargo test … --workspace --no-fail-fast`, and `cargo fmt … --check`.
`.github/workflows/ci.yml:34` also runs
`cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings`.
Lot A adds several hundred lines of code that `--all-targets` compiles. A
corrector running the declared gates can hand off a candidate CI rejects.
**Closure criterion.** Add the clippy invocation to `DOMAIN.md` § *Final global
gates* — and to the other features' `DOMAIN.md`, since the omission is not
specific to this one.

### `CHDR-040` — `OPEN`, P2 — the process clauses this train enforces are not in `PROCESS.md`

Stated in full as its own section, §6, at the orchestrator's request: it is a
finding against the process rather than against `c-headers`, and it is the most
consequential thing in this report.

### `CHDR-041` — reserved, not opened

Held by the orchestrator for the contingency in `CHDR-021`: if the audit's
surviving-mutant paragraph is dropped on closure, the `M12` residual loses its
home and `CHDR-041` opens for it. The condition has not fired. Recorded here so
the identifier is not reused — this audit already carries one identifier
collision (audit §1) and does not need a second.

### `CHDR-042` — `OPEN`, P3 — the declared regression command hides failures after the first red binary

**File.** `features/.agents/c-headers/DOMAIN.md`, § *Relevant regressions*:

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal --test g2_rotation --test g3_move --test b2_derivation
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb10_structure_vault --test vectors_ownership
```

Neither carries `--no-fail-fast`. `cargo test` fails fast **across test
binaries**, so the first binary to go red aborts the run and the remaining
binaries never execute. Their absence from the transcript reads exactly like a
pass and is not one.

**Measured, on this review, twice.** Same mutant (`M12`), same binaries, one flag
apart:

| Run | Command | Reported |
|---|---|---|
| `ev-debade53` | as `DOMAIN.md` writes it | **1** failure, in `c1_header_seal`; `g2_rotation` and `g3_move` silent |
| `ev-cbce8aa0` | `--no-fail-fast` added | **4** failures across all three binaries |

The blast radius was under-reported by a factor of four, and the two binaries
that vanished were the ones carrying the answer. This is not hypothetical: it
nearly cost this review a wrong verdict (`CHDR-021`, near-miss note).

**Severity, argued down rather than up.** It cannot produce a false green — the
exit code is still non-zero and a corrector cannot mistake the run for a pass.
The harm is narrower: an incomplete failure picture, in a train whose §14
*Definition of terminé* requires the corrector to *"documenter les deux
résultats"* and whose `LEDGER.md:44-51` makes printed counters as binding as the
exit code. A corrector reporting "one regression, in `c1_header_seal`" would be
reporting the truth of its transcript and not the truth of its change. P3.

**Note the inconsistency inside the same document.** `DOMAIN.md` § *Final global
gates* **does** carry `--no-fail-fast` on the workspace gate. The flag is
understood; it is simply absent from the tier where a multi-binary invocation
makes it necessary.

**Closure criterion.** Add `--no-fail-fast` to both regression commands in
`DOMAIN.md` § *Relevant regressions*, and to the equivalent section of the other
features' `DOMAIN.md`, since the pattern is copied. Cost: nil, alpha.

### Disclosure gate — nothing raised, and why I looked

I ran the check rather than assuming it did not apply. One candidate surfaced:
`spec/03-headers.md:39-40` — *"Two lines of one key version MUST NOT carry the
same `kid`"* — which nothing in the extract enforces, and whose consequence is a
second line declaring the subject's `owner_kex` while sealed elsewhere.
Searches: `grep -rniE "duplicate|uniq|dedup"` over `header.rs` and
`aithos-bundle/src/` → `state.rs:83`, `merge.rs:356`, `log.rs:856`,
`bundle.rs:135`, none about header lines; `grep -rniE "same .?kid|kid.*uniq"`
over `*.rs *.md *.py *.json` repo-wide → the spec sentence, two proposal
documents, one unrelated comment.

**It is already published in full**, as `CHDR-032` at
`docs/audits/features/c-headers.md:1432-1455`, including the emission path
`aithos header-seal --recipient <label>:<owner_kid>:<foreign key>` and the note
that the owner-bearing tier would resist but is called nowhere (`CHDR-030`). The
gate protects statements that are *not yet* public. Embargoing a restatement of a
published finding would be theatre and would make a published finding look
unpublished. **Nothing to raise under blocking condition 9** — recorded with its
search, because "no embargoed finding" is a claim like any other.

---

## 6. `CHDR-040` — `OPEN`, P2 — the process clauses this train enforces are not in `PROCESS.md`

This is a finding against the train, not against `c-headers`. It is stated
separately because it is the only thing in this report whose subject is the rule
system every role in the cycle — auditor, refuters, corrector, orchestrator and
me — has been made to obey.

### The claim

Three of this cycle's normative devices are cited as `PROCESS.md` sections and
are not in `PROCESS.md`:

1. § *Material isolation of Pass A* — the rule that produced the extract I was
   handed;
2. the **numbered list of blocking conditions**, 1 through 10;
3. the **disclosure gate**, blocking condition 9 — the rule deciding what a
   public repository does not learn.

### Who cites them as binding

| Citation | Text |
|---|---|
| `features/.agents/c-headers/STATE.md:29` | "…until your behavioural verdict is frozen (`../PROCESS.md`, § *Material isolation of Pass A*)" |
| `features/.agents/c-headers/STATE.md:55` | "…without the corrector's run report until its behavioural verdict is frozen (`PROCESS.md`, § *Material isolation of Pass A*)" |
| `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md:71` | "Matérielle, conformément à AM (`PROCESS.md` § *Material isolation of Pass A*)." |
| `features/.agents/c-headers/corrector/runs/2026-08-04-correction-i3-authority.md:156` | "…i.e. blocking condition 8 of `PROCESS.md`" |
| `features/.agents/c-headers/STATE.md:39` | "**All four blocking conditions are now closed.** Conditions 9, 6 and 7 by the disclosure and budget ruling of 2026-08-03; condition 1 by …" |
| my own reviewer brief | "This is `features/.agents/PROCESS.md`, § *Material isolation of Pass A*" and "This is blocking condition 9" |

### The search, its scope and its layer

Whole extract, all layers, not `features/**` and not `rust/**` only:

```text
grep -rn "Material isolation\|blocking condition" --include=*.md .
```

Hits, in full: `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md:233` (the
section heading) and `:261`, `:392`, `:475`; `docs/RECONNAISSANCE-ORCHESTRATEUR-2026-08-03.md:125`
(a table row proposing it); `features/.agents/orchestrator/LEDGER.md:48`;
`features/.agents/orchestrator/BLOCKED.md:214`; the five `.agents/c-headers`
citations above; and `docs/audits/features/README.md:79`.

`features/.agents/PROCESS.md` is **372 lines** and appears in that output **zero
times**. Its § *Artifacts* table (`:215-222`) does not list the proposal. Its
§ *Review-unit isolation and impartiality* (`:188-211`) — the section the
proposal amends — ends at *"A later orchestrator may spawn fresh agents for the
review units without changing the evidence model"* and contains no material-
isolation rule.

### The exact sentences that are missing

From `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md:233-261`, absent from
`PROCESS.md`:

> ### Material isolation of Pass A
>
> In orchestrated mode, Pass A isolation is material, not declarative. Each Pass A
> review unit runs against an extract of the immutable revision produced by
> `git archive`, with no `.git` directory. The agent does not refrain from reading
> history; it cannot, because no history is present.
>
> The correction review uses the same device. The reviewer receives an extract of
> the candidate revision, without `.git` and without the corrector's run report,
> until its behavioral verdict is frozen. The diff and the corrector's conclusion
> are delivered only for Pass B.
>
> An instruction not to read history is not sufficient for an unattended agent and
> must not be relied upon as the sole barrier.

From the same file, `:460-470`, the numbered conditions — of which `PROCESS.md`
carries no trace, not the list and not the numbering:

> 2. A third rejection of the same finding.
> 3. A red gate not attributable to the current scope.
> 4. Pass A contamination, declared or detected.
> 5. A refutation panel majority against the auditor.
> 6. Two warden invalidations of the same feature.
> 7. An exhausted budget — time, tokens, or disk.
> 8. A diff outside the assigned scope.
> 9. A finding caught by the disclosure gate.
> 10. A `FULL_AUDIT` classification by the impact review.

And `:475-479`, the gate itself:

> `aithos-core` is public, and orchestrated branches are pushed to it. A finding
> whose written statement would describe an exploitable weakness before a fix
> exists must not be written to any tracked file. The agent records the finding
> identifier and a neutral title, and raises blocking condition 9. The human owner
> decides what is published, and when.

The proposal's own closing note, `:487-495`, states that the owner revised it
*"before this proposal was ever applied to `PROCESS.md`"*. So it is not a draft
awaiting review — it is a document the owner has already amended **in place**,
while it sits outside the normative file.

### Why this matters more than a broken cross-reference

`PROCESS.md:110-121` establishes an evidence hierarchy in which *"Git history is
context, not proof"* and a written record of a past gate *"is history"*. The same
discipline applied to the rules themselves gives the problem its shape: a role
told to obey `PROCESS.md` § *Material isolation of Pass A*, who opens
`PROCESS.md` and searches for it, finds nothing — and has no way to distinguish
"the rule was renamed" from "the rule does not exist" from "I am being told to do
something no one wrote down". `PROCESS.md:141-146` even lists what Pass A may not
read, and the proposal is not among the permitted inputs, so a strict reader of
the normative file would decline to read the document that contains the rule
binding it.

The disclosure gate is the sharp end. It is the single rule whose failure is
irreversible: a finding written to a tracked file in a public repository cannot
be unwritten. It is currently defined nowhere in the artifact set that
`PROCESS.md` § *Artifacts* enumerates. I applied it in §5 of this report — I
looked for an embargoable finding, found `CHDR-032`, and declined to embargo a
published statement — and I applied it out of a proposal document, on the
instruction of a brief, not out of the process.

### Minimal normative text

I am not editing `PROCESS.md`; it is the owner's. The smallest change that closes
this, in one of two forms:

**Form A, preferred — apply the three blocks.** Insert § *Material isolation of
Pass A* verbatim after `PROCESS.md:211` (the end of § *Review-unit isolation and
impartiality*, which it amends); insert the numbered blocking conditions and the
§ *Disclosure gate* block verbatim before § *Evidence statuses*; add the proposal
to the § *Artifacts* table as the superseded source.

**Form B, if application is not yet wanted.** One paragraph in `PROCESS.md`,
placed immediately after the § *Artifacts* table:

> **Orchestrated-mode amendments.** In orchestrated mode this process is extended
> by `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md`, which is normative
> for material isolation of Pass A, the adversarial refutation panel, the numbered
> blocking conditions 1-10, and the disclosure gate. Where the two disagree, the
> amendment wins for orchestrated runs. A role that cannot locate a cited section
> in this file looks there before proceeding, and treats its absence from both as
> a blocking condition rather than as permission.

Form B is three sentences and makes every current citation resolve. Neither form
costs anything: nothing is deployed, no edition is published, and no role's past
work is invalidated by writing down what it was already doing.

**Closure criterion.** A reader of `features/.agents/PROCESS.md` who searches it
for "Material isolation", "blocking condition" or "disclosure gate" finds either
the rule or an unambiguous pointer to it.

**One thing I am explicitly not claiming.** That any role misapplied these rules.
Material isolation was applied to me — the extract had no `.git`, and I did not
open `/root/work/aithos-core`. The gate was applied by the audit, visibly, at its
§15. The rules are being followed. They are simply not written where they are
cited, and a rule system that works only because everyone already knows it is one
personnel change from not working at all.

---

## 7. What I attacked and could not break

Eight `VERIFIED` verdicts read like eight successes. They are worth something
only in proportion to what was tried and failed, so here is the failed half of
the work, in the order it was attempted.

**1. `M3` — the audit's own mutant, aimed at the lot.** I ran it hoping to show
that lot A's `CHDR-019` work caught nothing the audit claimed for it. Half
succeeded, half failed: the feature gate is green (`ev-c16f1a9a`, the debt
disproof), but `ev-2e427d6e` showed the lot catching it anyway, in
`c1_fail_closed`, by an assertion added for a different finding. The attack found
a defect in the audit and an unearned strength in the lot.

**2. `M12` — aimed at `CHDR-021`.** Designed to show the rebuilt scenario 8 still
blind to symmetric derivation mutations. It is blind to them (`ev-ec9412a7`,
green), so as an attack on the *scenario* it landed — but as an attack on the
*repository* it failed: `ev-cbce8aa0` shows four vector tests catching the class.
The residual is real and narrow, and narrower than I wanted it to be.

**3. `M10`'s feature-gate arm.** Run to test whether the Gherkin could see the
three I3 gates at all. Green (`ev-4ed2d6f3`). That confirmed the finding rather
than breaking the fix.

**4. The capability loop in `revoked_cannot_open` — five attempts, none
succeeded.** I tried hardest here, because it is the assertion I most suspected
of being decorative:

- `kek` drops the DH secret from the IKM → measured, `ev-c16f1a9a`: the loop does
  not see it. No discrimination.
- `kek` drops the DH secret **and** `recipient` from the `info` → would open
  every line to every holder, but is killed by `stranger_recovers_nothing`,
  scenario 2, which predates the lot. No discrimination.
- `rotate` zips survivors' kids against the *previous* version's recipient keys →
  killed by `survivor_opens`, also pre-existing. No discrimination.
- `Header::open` drops the `kid` filter and tries every line → the revoked's
  secret still fails against all of them. No effect at all.
- `rotate` emits a second line carrying a survivor's `kid` but sealed to the
  revoked's key → **this one the loop would uniquely kill**, and it is not
  expressible: `Line` stores `to`, `kid`, `epk`, `n`, `c` (`header.rs:43-50`) and
  no recipient public key, so `rotate` cannot recover the revoked's key from the
  header it is rotating.

Outcome: I could not fault the loop and I could not credit it. It is labelled
unproven in `CHDR-019` rather than counted as one of the three assertions that
close the finding.

**5. My own attribution inside `CHDR-019`.** I expected `check_rotation(2)` to be
a killer of `M2`. `ev-39f02b30` shows it accepting `M2` — `g1` is present in the
previous version, so containment holds. My guess about which of the three new
assertions was load-bearing was wrong, and the transcript corrected me. Recorded
in `CHDR-019`.

**6. Cross-scenario state leakage.** `DOMAIN.md` flags `open_into` as the funnel
every negative scenario passes through and asks whether `opened` is carried
across scenarios. I went looking. Not only is there no leak — `ev-1335c8f1`
*disproves* one, because scenario 2 leaves two `Err`s in `opened` and precedes
scenario 3, whose new `Then` demands `opened.first() == Ok(DK)`. The corrector's
assertion is an accidental detector for the hazard the integration pass was told
to hunt.

**7. Scope violation — two suspicions, both dissolved.** The `CHDR-002` work
looked at first like unassigned effort until I re-read `assigned_findings` and
found `CHDR-002` is in the lot. The removal of the `CHDR-007`/`CHDR-012` markers
looked like scope creep until Pass B attributed it to `c547ccd`, lot B's
reviewer. Blocking condition 8 is not engaged; I checked twice and was wrong
twice.

**8. Vacuous new assertions.** I checked every new assertion that iterates or
slices, for the failure mode this whole audit is about — passing because it
examined nothing. `opening_rejected`'s `.skip(1)` loop is guarded by
`opened.len() >= 2` immediately above it; the capability loop over `v2.lines` is
non-empty because `check_rotation` requires an owner line; `owner_line_untouched`'s
prefix slice is guarded by `saved.len() >= 2`. All three guarded, deliberately.
Nothing found.

**9. Gherkin-to-fixture honesty.** After `CHDR-014` repointed the `Given`, I
checked whether the phrase still describes the state built: *"a sealed header for
the owner and an existing reader"* against `[owner_rec(), grantee_rec("g1")]`.
Honest. Nothing found.

**10. Assertion messages claiming more than their assertions.** This found two
nits and no faults: `key_versions.len() == 1` labelled *"a grant creates no key
version"* (true only because the fixture starts at one version), and
`check_rotation`'s `expect` saying *"survivors ⊆ previous"* where
`spec/03-headers.md:109-111` says **equal**. The second is the corrector being
*more* careful than the spec-versus-code gap permits, not less — it describes
what `check_rotation` does rather than what §3.4 requires. Both recorded as nits;
neither is chargeable.

**11. `M1`'s redundancy.** I suspected the version-half graft might be caught by
something else, making it ornamental. `ev-9ba93af7` shows attempt 1 failing and
attempt 2 succeeding under `M1`: nothing else in the scenario sees it.

**The one attack I could not mount.** Every mutant here was designed by me,
against defect statements I had also read. A mutant author who had not read the
closure criteria might find a gap I did not. That is the structural limit of a
reviewer who writes his own mutants, and no transcript removes it. It is item 8
of §4 and it is the honest ceiling on all eight verdicts.

---

## 8. Pass A / Pass B reconciliation

Pass A predicted a failing assertion, a scenario number and a message for each of
the twelve mutants before any evidence existed. Eleven matched. One did not, and
it is the only place Pass B changed anything I wrote.

| Mutant | Pass A prediction | Transcript | Agreement |
|---|---|---|---|
| `M1` | scenario 4, `attempt 2 …` | `ev-9ba93af7`, 7/1 | exact |
| `M2` | scenario 7, structural msg | `ev-39f02b30`, 7/1 | exact |
| `M3` | feature gate fully green | `ev-c16f1a9a`, 8/28 green | exact |
| `M3` | `c1_header_seal` red **only** at the Python cross-check | `ev-2e427d6e`, **2 failed** | **diverged** |
| `M4` | scenario 6, cardinal msg | `ev-a1f966ca`, 7/1 | exact |
| `M5` | scenario 6, prefix msg | `ev-1b889900`, 7/1 | exact |
| `M6` | scenario 6, prefix msg, not the owner check | `ev-b3ccaaf3`, 7/1 | exact |
| `M7` | scenario 8, wrong-node msg | `ev-c78772c4`, 7/1 | exact |
| `M8` | scenario 8, derivation msg | `ev-16a836a9`, 7/1 | exact |
| `M9` | scenarios 3 and 4 at the control; **scenario 5 green** | `ev-11dee753`, 5/3 | exact |
| `M10` | exactly 3 new tests red; feature gate green | `ev-dce43f1c` 4/3, `ev-4ed2d6f3` green | exact |
| `M11` | red at the positive control | `ev-ad4db6a1`, `ev-34e698d8` | exact |
| `M12` | feature gate fully green | `ev-ec9412a7`, 8/28 green | exact |
| `M12` | the class caught by C2, `g2_rotation` and the `g3_move` wrap pins | `ev-cbce8aa0`, 4 failed across 3 binaries | exact, and one test wider |

**The divergence, and what it means.** I predicted one failure in
`c1_header_seal` under `M3`; `ev-2e427d6e` shows two. The second is
`c1_fail_closed`, and it falls because lot A's new positive control decrypts the
*frozen vector ciphertext* under the nominal AAD, which `M3` breaks by changing
the KEK.

I had reasoned about that control as a differential base for four negative
assertions — which is how the closure criterion frames it. That framing is
incomplete. Because the ciphertext is a constant read from a pinned file rather
than something the test re-seals, the control is an **asymmetric pin on the whole
seal path**: KEK derivation, AAD construction and AEAD together. It catches
symmetric mutations, which is what `CHDR-025` asked for, and it also catches
mutations that leave both sides consistent with each other but inconsistent with
the frozen bytes — which is more than `CHDR-025` asked for.

Three consequences, all in the corrector's favour, none of which I would have
found without a mutant aimed elsewhere:

1. `CHDR-025`'s correction is stronger than its own closure criterion.
2. My §2 sentence "the only thing that falls is the C1 byte pin" is **withdrawn**.
3. The audit's `CHDR-019` mutant *is* caught by the repository, at the
   conformance-vector layer, and lot A **doubled** that catch. The audit put the
   detection in scenario 7, where it does not exist and cannot; it exists in
   `c1_fail_closed`, where lot A put it by accident. §2 records both.

No verdict moved. The divergence strengthened one finding and corrected one
subsidiary claim of mine.

**A second divergence, of instrumentation rather than prediction.** `ev-debade53`
— the first run of my closing command, written as I named it — reported one
failure where `ev-cbce8aa0` reports four. Nothing about the code differed; the
first command lacked `--no-fail-fast`, `cargo test` aborted after the first red
binary, and two binaries never ran. That is not a divergence between my
prediction and the code but between my *command* and what it appeared to
measure, and read at face value it would have flipped the `CHDR-021` call. The
orchestrator caught it; I did not. The command was mine, so the defect is mine,
and it generalises to a command `DOMAIN.md` puts in front of every corrector —
`CHDR-042`.

It is also the sharpest illustration in this review of the rule the brief gave
me: *a green gate proves nothing on its own*. Here it was not even a green gate,
only a **silence**, and silence read as green is the same error one layer down.

**On a lot that comes back clean.** Eight for eight is a result I am aware looks
like agreement with a plausible diff, so §7 sets out the failed half of the work
in full: eleven attacks, of which the five aimed at the capability loop in
`revoked_cannot_open` all failed, one corrected my own attribution inside
`CHDR-019`, two suspicions of scope violation dissolved on inspection, and the
hunt for a vacuous new assertion found three that are explicitly guarded. Twelve
mutants, three of them (`M3`, `M12`, `M10`'s feature-gate arm) built to show the
lot inert rather than to confirm it; two came back green and are reported as such
— `M12` as a live residual on `CHDR-021`, `M3` as the disproof of the audit's own
claim. One assertion the lot added killed nothing in any run and is labelled
unproven rather than counted. Two of the corrector's assertion messages are
looser than what they assert and are recorded as nits. Five new findings are
raised, one of them P2 against the process itself. The lot is clean; the review
is not a clean bill.

---

## 9. Pass B history, as disclosed

Recorded because `PROCESS.md:118-121` makes history context, not proof — none of
the below establishes behaviour, and no verdict above rests on it.

- **`c547ccd`** — *"review(c-headers): CHDR-007 and CHDR-012 VERIFIED — lot B
  accepted"*. Lot B's reviewer removed the two `DECISION_REQUIRED` markers, kept
  lot A's, and re-ran the gate (`ev-cf4a9d62`, 1/4/8/28 unchanged). Answers §3.
- **`03283b0`** — lot A's correction. Touches `features/c-headers.feature` on one
  line only, `:68`. No diff outside the assigned scope; blocking condition 8 not
  engaged.
- **`5be3047`** — lot B's base, *"spec: apply the I3 authority lot — variant A"*,
  which introduced `vectors/gen-c.py`. Half 2 of `CHDR-025` was therefore closed
  before lot A began. The verdict is unaffected — the criterion is met on the
  candidate — but the **credit** belongs to lot B, and `CHDR-038` remains open
  and is not chargeable to lot A.

---

## 10. Verdicts

| Finding | Verdict | Mutant | Evidence |
|---|---|---|---|
| `CHDR-001` | **`VERIFIED`** | `M1` | `ev-9ba93af7` |
| `CHDR-002` | **`VERIFIED`** | `M9` | `ev-11dee753` |
| `CHDR-009` | **`VERIFIED`** | `M10` | `ev-dce43f1c`, `ev-4ed2d6f3` |
| `CHDR-013` | **`VERIFIED`** | `M4`, `M5` | `ev-a1f966ca`, `ev-1b889900` |
| `CHDR-014` | **`VERIFIED`** | `M6` | `ev-b3ccaaf3` |
| `CHDR-019` | **`VERIFIED`** | `M2` | `ev-39f02b30`; debt on `ev-c16f1a9a` |
| `CHDR-021` | **`VERIFIED`** | `M7`, `M8` | `ev-c78772c4`, `ev-16a836a9`; residual `ev-ec9412a7` |
| `CHDR-025` | **`VERIFIED`** | `M11` | `ev-ad4db6a1`, `ev-34e698d8`; strengthened by `ev-2e427d6e` |

Preconditions: `ev-14592971`, `ev-1335c8f1`, `ev-1a19fdf4`.
None `NOT_VERIFIED`. None `REGRESSION`.

**Conditions attached to acceptance**, none of which blocks it:

1. The `CHDR-021` block of the public audit retains its surviving-mutant
   paragraph verbatim on closure, with `ev-ec9412a7` appended. If it is dropped,
   my call to keep `M12` as a note rather than an identifier is withdrawn.
2. The `CHDR-019` mutant in the audit's §6 is corrected per §2 before the audit is republished.
3. The Gherkin markers are rewritten per §3, not merely stripped — scenario 8's
   scenario-level tag changes with its verdict, and `@chdr-016` survives with its
   re-routing named.

**Nothing outstanding.** The one command I had left open — `M12` against the
three vector binaries, to settle whether the symmetric class is contained — was
run twice (`ev-debade53`, then `ev-cbce8aa0` with `--no-fail-fast`). It came back
red in four places, so my stated flip condition did not fire and the residual
stays a note under `CHDR-021`. I name no further command.

**Findings raised by this review.** `CHDR-037` (P3, marker lifecycle),
`CHDR-038` (P3, `gen-c.py` unwired), `CHDR-039` (P3, clippy absent from the
declared gates), `CHDR-040` (**P2**, §6, the process clauses are not in
`PROCESS.md`), `CHDR-042` (P3, the regression command hides failures after the
first red binary). `CHDR-041` is reserved, not opened.

---

**This report is frozen.** Pass A was frozen before evidence; Pass B is
reconciled in §8 with its one divergence named rather than absorbed; the failed
attacks are in §7 beside the successful ones. Twenty evidence ids from run
`2026-08-04-r6`, every one issued by the orchestrator on a command I named and
none produced by me, plus `ev-cf4a9d62`, which is lot B's and is quoted as
history in §3 and §9, never as support for a verdict. Eight `VERIFIED`, no
`NOT_VERIFIED`, no `REGRESSION`, five new findings, three conditions on
acceptance now carried by the orchestrator.

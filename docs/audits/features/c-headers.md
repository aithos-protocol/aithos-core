# Audit — `c-headers.feature`

## 1. Metadata

| Field | Value |
|---|---|
| Feature | `features/c-headers.feature` (`@c-headers`) |
| Round | 1 — initial audit |
| Date | 2026-07-30 |
| Observed revision | `3803fe806702143d5bb887b5ddc33fd3e0526285` |
| `main` base | `240c6589986af6115530c90a7aa8646c2c44b68f` |
| Branch | `codex/audit-c-headers` |
| Worktree state | clean except the pre-existing untracked `_to_delete/` |
| Scope | the semantic truth of the eight existing scenarios; four `Rule` blocks |
| Finding prefix | `CHDR-*` (`CHDR-001` … `CHDR-016`) |
| Domain | `features/.agents/c-headers/DOMAIN.md` |
| Run report | `features/.agents/c-headers/auditor/runs/2026-07-30-audit-initial.md` |

The observed revision differs from the `main` base only by the commit that
created this feature's agent domain. No `features/` or `rust/` file audited
here differs between the two.

## 2. Method provenance

Four review units, one per Gherkin `Rule`, each executed by a fresh agent
against a source-only extract of the observed revision with **no `.git`
present**, so history-blindness in Pass A was enforced by construction rather
than by discipline. Each unit received the contract, the domain, the routing
fields of the state, and the current code; none received another unit's
verdict, the Git history, or any prior conclusion. No prior conclusion exists
for this feature — this is round 1.

| Unit | Rule | Scenarios |
|---|---|---|
| RU-1 | A line seals the node key to exactly one recipient | 4 |
| RU-2 | The owner line is mandatory (I3) | 1 |
| RU-3 | Grant is one appended line, touching nobody | 1 |
| RU-4 | Rotation cuts the revoked and re-links the parent | 2 |

All four Pass A reports were frozen and written before Pass B began. Pass B
(history, differential evidence) and the shared-state integration pass were
performed last, by the integrating auditor, over the four frozen reports.

**Contamination disclosure.** The integrating auditor had seen the repository's
recent commit log while preparing the branch and the domain, before the units
ran. The four Pass A verdicts are unaffected: the units ran in isolated
contexts with no history available. Pass B reconciliation is by definition
history-aware. Three units disclosed incidental exposure to *other features'*
identifiers (`AID-*`, `BDER-*`) through `docs/audits/features/README.md` and
through source comments in `cucumber.rs:479-485`; no `CHDR-*` conclusion
existed anywhere to leak.

## 3. Verdict

The feature is **green and honest about its cryptography, weak about its
structure, and false about its up-link**.

Every scenario is selected and executed, every `When` reaches real production
code in `aithos-core`, and no scenario is a proxy for another's verdict. The
positive cryptographic path — seal a node key to a recipient, recover it,
rotate to a new version, let the survivor and the owner in — is proved with
exact expected-key comparisons at exact expected versions.

What is not proved is almost everything structural. Six of the eight scenarios
assert a *behavioral* consequence where the contract states a *structural*
fact, and the two are not equivalent:

- "the revoked **gets no line**" is proved as "the revoked cannot open";
- "**every other** line untouched" is proved against a header with one line;
- "bound to its node **and version**" varies only the node;
- "an up-link wrap **restores derivation**" performs no content-tree
  derivation at all.

One scenario is a semantic false positive: `An up-link wrap restores derivation
for the parent holder` contains no derived node, no rotation, and no
content-tree derivation. It seals a constant under a constant and opens it with
the same constant, two steps later, in the same scenario.

Two `Then` functions — `opening_rejected` and `revoked_cannot_open` — are bare
`is_err()` checks on an API that returns a byte-identical error for every
failure cause, so no assertion phrased at that API can attribute a rejection to
the cause its scenario names. `Header::open` returns the same
`Error::SealRejected("no line opens for kid …")` whether the line was
corrupted, replayed onto another node, addressed to someone else, or **absent
entirely**.

### Exact counts

```
1 feature
4 rules
8 scenarios (8 passed)
28 steps (28 passed)
```

## 4. Reproduced evidence

Static gate, run once from the repository root:

```
features/.agents/scripts/verify-feature-tags.sh
→ feature tags ok (18 files)
```

Canonical feature gate, run once on the immutable observed revision:

```
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers
→ 1 feature / 4 rules / 8 scenarios (8 passed) / 28 steps (28 passed)
```

The printed counts match the feature file exactly (4 `Rule` blocks, 8
scenarios, 28 steps) and are the evidence of selection and execution.

**The exit code was not used and is not evidence.** `BDER-011` is open on this
baseline: `rust/crates/aithos-bundle/tests/cucumber.rs:19730` calls
`filter_run`, not `filter_run_and_exit`, under `harness = false`, so the runner
exits `0` even when scenarios fail. The observed `GATE_EXIT=0` is reported here
only for completeness and carries no verdict.

The auditor ran no unfiltered Cucumber, no broad regression, and no workspace
gate during the audit itself, and needed no focused test: no semantic
contradiction arose.

### Adversarial verification of the top findings (disclosed deviation)

After the audit was written, its four highest-severity claims were handed to an
independent verifier instructed to **refute** them. All four survived. Two were
settled by mutation experiment rather than by code reading, which is stronger
evidence than an initial audit normally produces and is recorded here as such.

**`CHDR-003`.** `Header::check_rotation`'s body was replaced by an
unconditional `panic!`, the workspace rebuilt, and the feature gate re-run:

```
1 feature / 4 rules / 8 scenarios (8 passed) / 28 steps (28 passed)
```

Identical counts with a `check_rotation` that cannot execute. No `c-headers`
step reaches it.

**`CHDR-006`.** `line_aad` was rewritten to emit
`purpose ‖ 0 ‖ did ‖ 0 ‖ node`, dropping the `key_version` component while
leaving the shared `aad()` helper — and therefore `wrap_aad` and `blob_aad` —
byte-for-byte untouched. Feature gate: `8 scenarios (8 passed)`. Then, to
establish blast radius, the unfiltered suite:

```
18 features / 114 rules / 836 scenarios (836 passed) / 3577 steps (3577 passed)
```

The whole BDD layer is blind to it. The only failure anywhere in the workspace
was `aithos-core::c1_header_seal::c1_owner_and_grantee_lines`, the byte
cross-check against the Python-generated vector. That run also exposed
`CHDR-016` below.

Both mutations were reverted and the restoration verified by `diff` and
`md5sum` against pre-edit backups; a final clean-tree run reproduced the
baseline gate and `3 passed; 0 failed` for `c1_header_seal`.

**Disclosed deviation from the role boundary.** `PROCESS.md` forbids the
auditor from running unfiltered Cucumber. That unfiltered run happened, as a
refutation attempt by an adversarial verifier measuring the blast radius of a
deliberate mutation. It is reported here as the measurement it is and is
**not** offered as a regression-gate claim; the corrector still owns the global
gates. Reporting it is better than concealing it.

One environment note, disclosed because it affects reproducibility rather than
the verdict: the gate was executed against a `git archive` extract of the
observed revision in a container with a Rust toolchain, because the workspace
requires a sibling `aithos-client` checkout for the `aithos-gateway` member and
no toolchain is available on the mounted-device path. The extract is byte-equal
to the revision by construction.

## 5. Scenario matrix

| # | Scenario | Status | Production path | What the assertion actually compares |
|---|---|---|---|---|
| 1 | Owner and grantee each open their line | `PROVEN` | `Header::build` → `build_lines` → `seal_line`; `Header::open` → `open_line` ×2 | Two independently recovered keys, each `assert_eq!` against `DK`; the `kid` filter proves each recipient opened its own line |
| 2 | A non-recipient opens nothing | `PARTIAL` | `Header::build`; `Header::open` ×2 with `xsk(0x99)` | `!opened.is_empty()` and `all(is_err)` — never ties the attempt count to the header's line count |
| 3 | A corrupted line fails closed | `PARTIAL` | `Header::build`; in-test nibble flip on `lines[0].c`; `Header::open` | `opened.last().is_err()` — bare, no positive control, no cause attribution |
| 4 | A line is bound to its node and version | `PARTIAL` | `Header::build` ×2; line transplant; `Header::open` under a different node's AAD | `opened.last().is_err()`; only the `node` AAD component is varied |
| 5 | A header without an owner line is invalid | `PROVEN` | `Header::build` → `build_at` → `check_owner_line` → `Err(MissingOwnerLine)` | The `When` panics on `Ok`; the `Then` asserts the stringified error contains `"I3"` |
| 6 | Granting a new reader leaves every other line untouched | `PARTIAL` | `Header::append_line` → `seal_line`; `Header::open` | Recovered key `== DK`; full `PartialEq` byte-identity of the owner `Line` against a pre-append snapshot — on a header that has exactly one other line |
| 7 | The revoked gets no line in the new version | `PARTIAL` | `Header::build` → `Header::rotate` → `build_lines`; `Header::open` ×3 | Survivor and owner recover `DK2` at v2 (strong); revoked assertion is a bare `is_err()`; `key_versions["2"].lines` is never read |
| 8 | An up-link wrap restores derivation for the parent holder | `SEMANTIC_FALSE_POSITIVE` | `Wrap::seal` → `wrap_seal` → `derive_key(CTX_WRAP_KEY, …)`; `Wrap::open` | A symmetric AEAD round-trip under the same 32-byte constant used to seal, in the same scenario, with no header, no rotation and no derivation |

Totals: 2 `PROVEN`, 5 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE`.

## 6. Ordered findings

### `CHDR-001` — the up-link wrap scenario proves nothing it claims — `OPEN`, P1

**Status:** `SEMANTIC_FALSE_POSITIVE`. **Scenario 8.** Independently verified by
adversarial refutation — see §4.

The scenario contains no derived node, no rotation, and no content-tree
derivation. `Given a derived node rotated to a fresh random key` is an empty
body (`cucumber.rs:7597-7600`) that writes nothing; `w.header` stays `None` for
the whole scenario. The `When` (`cucumber.rs:8163-8174`) constructs
`Wrap::seal(DID_C, NODE_A, &PARENT_KEY, CHILD_NODE, 2, &DK2, non(9))`. The
`Then` (`cucumber.rs:12395-12404`) opens that same object with the same
`PARENT_KEY` and compares the result with `DK2`. That is an AEAD round-trip
under a constant, asserted two steps after the seal.

Two precisions, because the loose form of this finding is refutable. First,
`derive_key` **is** reached: `wrap_seal` and `wrap_open` both call
`derive_key(CTX_WRAP_KEY, via_key)` (`seal.rs:136-137`, `:150`). What is absent
is *content-tree* derivation — no `node_key` walk, no `folder_label` /
`section_label` step, no parent→child link. Second, `via = NODE_A =
"/e/circle"` **is** textually the path-parent of `CHILD_NODE`; that relation is
simply inert. `via` is stored as a struct field, never enters the AAD
(`wrap_aad` is computed over the *wrapped* node, `header.rs:340`, `:352`), and
is read by neither `Wrap::open` nor the `Then`.

`PARENT_KEY = [0x55; 32]` (`cucumber.rs:265`) is a bare fixture: it is the
output of no `node_key` walk, it is opened from no header line, and it is not
the key of the node the wrap names as its parent — everywhere else in this
feature `NODE_A`'s key is `DK = [0x77; 32]`. `DK2 = [0x66; 32]` is produced by
no rotation in this scenario. Nothing computes `node_key` for `CHILD_NODE`, so
the derivation path the wrap is supposed to *restore* is never established,
never severed, and never re-established.

Spec `03-headers.md:69-78` and `:83-87` state the purpose: holders of the
parent *or of any ancestor of it* keep reading the rotated node by derivation
(`02-content-tree.md` §2.5) without a line of their own. Neither half is
exercised.

**Closure criteria.** The `Given` builds real state: derive `K_P` for the
parent, derive the pre-rotation child key from it via `node_key`, rotate the
child's header to a fresh key, store both. The `Then` recovers `K_P` by
derivation from an ancestor key before opening the wrap, and additionally
asserts that the pre-rotation derived child key no longer opens the new
version — the severed-then-restored pair the scenario name claims.

### `CHDR-002` — "gets no line" is proved as "cannot open" — `OPEN`, P1

**Status:** `PARTIAL`. **Scenario 7.**

No assertion in this scenario reads `key_versions["2"].lines`. The only
revocation-side check is
`assert!(header.open(DID_C, 2, "g1", &xsk(0x21)).is_err())`
(`cucumber.rs:12374-12382`).

The structural claim is unreachable through that API *by construction*. When no
line carries the `kid`, the loop at `header.rs:233` never executes and control
falls to `header.rs:242-245`; when a line exists but `open_line` rejects
(`seal.rs:129`), the loop exhausts and reaches the *same statement*, producing
a byte-identical `Error::SealRejected`. So "no line addressed to the revoked
exists" and "a line exists but does not decrypt" are indistinguishable at this
API, and the scenario asserts the weaker of the two.

Spec `03-headers.md:80` states the structural fact; `:88-89` makes it
mechanically verifiable ("the new version's lines MUST equal the previous lines
minus the revoked").

**Closure criteria.** Add a structural assertion to the existing `Then`:
`assert!(header.key_versions["2"].lines.iter().all(|l| l.kid != "g1"))`, and/or
call `header.check_rotation(2).unwrap()` there (see `CHDR-003`).

### `CHDR-003` — the Rule's contract is owned by a function it never calls — `OPEN`, P2

**Status:** `PARTIAL`. **Scenario 7.** Proven by mutation — see §4.

`Header::check_rotation` (`header.rs:275-305`) implements exactly the
well-formedness the Rule title asserts: every new-version `kid` must exist in
the predecessor version, and the new version must carry an owner line. It is
called by neither `Header::rotate` (`header.rs:192-217`, which calls only
`check_owner_line` at `:201`) nor by any step of this feature. Its exercised
call sites are all elsewhere: `aithos-bundle/src/revoke.rs:199`,
`aithos-bundle/src/vault.rs:400`, `cucumber.rs:15260` (a `g-revocation` step),
`aithos-core/tests/g2_rotation.rs:79,92`.

Consequence: the `c-headers` Rule that names rotation as its subject stays
green when `check_rotation`'s body is replaced by an unconditional `panic!` —
verified, §4. (Stated precisely: *deleting* the function would break
compilation of `revoke.rs:199`, `vault.rs:400`, `cucumber.rs:15260` and
`g2_rotation.rs:79,92`, so the gate would not build. Neutralising its body is
the meaningful test, and the gate does not notice.)

This is not a report that a smuggled-recipient scenario is missing — that would
be out of scope. It is that the scenario which *does* claim the cut never
reaches the function that decides it.

**Closure criteria.** Invoke `check_rotation(2)` inside the existing `Then` of
scenario 7.

### `CHDR-004` — the revoked-cannot-open assertion survives the rotation not happening — `OPEN`, P2

**Status:** `PARTIAL`. **Scenario 7.**

`revoked_cannot_open` (`cucumber.rs:12374-12382`) asserts only `is_err()`. If
the `When` (`cucumber.rs:8147`) were removed or silently no-oped,
`key_versions` would hold no `"2"` and `Header::open` would return
`Error::SealRejected("no key version 2")` at `header.rs:230-232` — and this
`Then` would still pass. It is protected only by its two sibling `Then`s, which
`unwrap()` on version 2.

**Closure criteria.** Establish the version-2 precondition inside the
assertion, so the failure is attributable to "no line for `g1`" rather than to
"no version 2". The structural check of `CHDR-002` satisfies this as a side
effect.

### `CHDR-005` — the Rule's two halves are never joined — `OPEN`, P3

**Status:** `PARTIAL`. **Scenarios 7 and 8.**

Scenario 7 rotates `NODE_A` to `DK2` at v2 and posts no wrap. Scenario 8 posts
a wrap for `CHILD_NODE` at v2 under `DK2` with no rotation. Spec
`03-headers.md:61-79` and `06-revocation.md:28-34` make step 2bis part of the
*same* rung-2 act on the *same* node. The shared constant `DK2`
(`cucumber.rs:264`, used at `:8155` and `:8171`) makes the two scenarios read
as a chain while no executed state links them — and they target different nodes
(`/e/circle` vs `/e/circle/d/…01`).

**Closure criteria.** Rotate the child node in scenario 8's `Given` and wrap
*that* rotation's key, so the wrapped `DK'` is the value the rotation produced.
Subsumed by `CHDR-001`'s correction if that correction rotates a real child.

### `CHDR-006` — the "version" half of the binding scenario is never exercised — `OPEN`, P2

**Status:** `PARTIAL`. **Scenario 4.** Proven by mutation, and **worse than the
scenario-level statement suggests** — see §4.

The line AAD binds `subject_did ‖ node ‖ key_version` (`seal.rs:21-31`,
`:35-37`; spec `03-headers.md:124-126`). In `replay_line_other_node`
(`cucumber.rs:8113-8122`) both `Header::build` calls default to version 1
(`header.rs:116`) and the open is at version 1, so `subject_did` and
`key_version` are identical on both sides and only `node` differs.

A regression that drops `key_version` from `line_aad` leaves not merely the
four scenarios of this Rule green but **the entire Cucumber suite green — all
18 features, 836 scenarios, 3577 steps** (measured, §4). Cross-version line
replay, the exact threat §3.8 is written against, has no behavioral coverage
anywhere in the repository. The single detector is the byte-exactness
cross-check `aithos-core/tests/c1_header_seal.rs:66` against the independently
generated Python vector — and see `CHDR-016`, which is the reason that detector
is the *only* one.

**Closure criteria.** Add a second recorded attempt that transplants the same
v1 owner line into a v2 key version of the *same* node (or opens it at
version 2), and require both recorded attempts to be `Err`. Alternatively split
the scenario in two so the Gherkin stops claiming more than one variation.

### `CHDR-007` — rejection assertions attribute no cause and have no positive control — `OPEN`, P2

**Status:** `PARTIAL`. **Scenarios 3 and 4** (shared `Then`).

`opening_rejected` (`cucumber.rs:12340-12344`) accepts any `Err`. As established
in `CHDR-002`, `Header::open` emits an identical error for a wrong key, a wrong
`subject_did`/`node`/`key_version`, a corrupted byte, and for "no line carries
that kid at all" — the last of which performs no cryptography whatsoever. So
scenarios 3 and 4 prove "opening returned an error", not "opening was rejected
*because of the corruption*" or "*because of the node rebinding*".

Neither scenario establishes that the line opened *before* the mutation, so a
fixture regression that made the owner line permanently unopenable would keep
both green.

The corruption itself is well-formed and does reach the AEAD: the step flips
the first hex character of `lines[0].c` (`cucumber.rs:8103-8111`), which always
changes the value, keeps the string valid hex and the same length, so
`hex::decode` at `header.rs:236` still succeeds and the failure occurs inside
XChaCha20-Poly1305 rather than on the encoding-rejection path. The defect is in
the assertion, not in the mutation.

**Closure criteria.** Make each rejection scenario differential inside itself:
in `corrupt_line`, open once before the flip and once after, and assert
`opened[0] == Ok(DK)` and `opened[1].is_err()`; additionally assert `epk`, `n`,
`to`, `kid` and the sibling line are unchanged so the ciphertext is the only
varied input. In `replay_line_other_node`, record a control open of the stolen
line on its original header before the transplant.

### `CHDR-008` — "tries every line" is a hardcoded kid list — `OPEN`, P3

**Status:** `PARTIAL`. **Scenario 2.**

The `When` (`cucumber.rs:8096-8101`) iterates the literal
`["owner-kex", "g1"]` rather than the header's lines, and the `Then`
(`cucumber.rs:12334-12338`) requires only "non-empty and all `Err`". A line
added to the fixture would be silently untried; a kid literal that stopped
matching any line would produce a vacuous `Err` from `header.rs:242` with zero
decryption performed, and the scenario would still pass. Under the code as it
stands coverage is in fact complete, so this is a proof-strength defect, not a
live behavioral defect.

**Closure criteria.** Derive the kid list from the header, and assert
`opened.len() == lines.len()` alongside `all(is_err)`.

### `CHDR-009` — no scenario reaches the C1/C2 vectors — `OPEN`, P3

**Status:** `PARTIAL`. **Scenarios 1 and 8.**

The Gherkin fixtures and the conformance vectors share no inputs beyond the
node string `/e/circle`: the vector fixes `subject_did = did:aithos:z6Mkopv…`,
`dk_hex = c8c9…`, `esk_hex = 7879…`, `n_hex = 0001…`, while the scenarios use
`did:aithos:test-header`, `0x77…`, `0x41…`, `0x61…`. For the wrap, the vector's
`via_key`, `wrapped_node`, `key_version` and `dk` coincide with the fixtures,
but its `subject_did` and nonce do not — and both feed the AAD, so the
scenario's ciphertext cannot equal the vector's.

`aithos-core/tests/c1_header_seal.rs` does cross-check byte-for-byte, but it
calls `seal_line` / `open_line` / `line_aad` / `wrap_seal` / `wrap_open`
**directly and never constructs a `Header`**. Nothing therefore anchors the
`Header` layer's own line encoding — the `epk`/`n`/`c` hex fields, the AAD built
from `self.node` and the decimal version, the recipient-to-ephemeral zip at
`header.rs:91` — to independently generated bytes.

**Closure criteria.** Add one assertion in `c1_header_seal.rs` that builds a
`Header` from the vector inputs and compares
`key_versions["1"].lines[i].{epk,n,c}` with the vector's hex. This is a
core-test addition, not a Gherkin change, and leaves the scenarios' behavioral
role intact.

### `CHDR-010` — "touching nobody" is exercised against a single-line header — `OPEN`, P2

**Status:** `PARTIAL`. **Scenario 6.**

The `Given` (`cucumber.rs:7569-7573`) seals to `&[owner_rec()]` — one recipient
— so `key_versions["1"].lines` holds exactly one pre-existing entry. "Every
other line untouched" degenerates to "the only other line is untouched", and
the scenario cannot distinguish an `O(1)` push from an `O(n)` rebuild-and-reseal
of the remaining recipients, because with `n = 1` there is no remainder to
disturb and no ordering to perturb.

The `O(1)` property is real and visible in `header.rs:159-177`, which reads no
field of any existing `Line` — but that is code evidence, not scenario
evidence. A fixture that would give `n = 2` already exists and is unused here:
`sealed_header_owner_grantee` (`cucumber.rs:7552-7565`).

The byte-identity assertion itself has real force and is correctly ordered: the
snapshot is taken at `cucumber.rs:7571`, inside the same `Given` the scenario
runs, strictly before `w.header` is populated and therefore strictly before the
`When`; `Line` derives `PartialEq` over all five fields including the full hex
`epk`/`n`/`c`, so a silently rebuilt owner line would necessarily differ.

**Closure criteria.** Point the `Given` at a header with at least two
pre-existing recipients, snapshot the whole `lines` vector, append a *different*
grantee, and assert both prefix equality (which also pins order) and
`lines.len() == saved.len() + 1`. Note this requires changing the Gherkin
`Given` phrase, which is currently shared with scenario 4.

### `CHDR-011` — "DK unchanged" is entailed, not asserted; §3.3 step 1 is never executed — `OPEN`, P3

**Status:** `PARTIAL`. **Scenario 6.**

Neither `Then` re-opens the owner line after the append. The `When`
(`cucumber.rs:8143`) *supplies* `&DK` to `append_line`, and the first `Then`
checks that `DK` comes back — a seal/open self-consistency check on freshly
injected key material. Spec `03-headers.md:52` step 1 ("Open the node's current
DK (own line)") is never exercised by this scenario, although
`aithos-bundle/src/session.rs:354-366` implements exactly that composition.

**Closure criteria.** Add a `Then` re-opening the owner line at version 1 and
asserting `== DK`; optionally have the `When` obtain its `dk` by opening the
owner line first, matching §3.3 steps 1-3.

### `CHDR-012` — the byte-identity assertion is position- and cardinality-blind — `OPEN`, P3

**Status:** `PARTIAL`. **Scenario 6.**

`owner_line_untouched` (`cucumber.rs:12355-12360`) locates the owner line with
`.find(|l| l.to == "owner")` and asserts nothing about `lines.len()` or index.
An `append_line` that inserted at position 0, duplicated a line, or removed an
unrelated line would still satisfy it. Harmless against today's
`kv.lines.push(…)` (`header.rs:173`), but it does not pin the "one appended
line" half of the Rule title.

**Closure criteria.** Folded into `CHDR-010`'s prefix-plus-length assertion.

### `CHDR-013` — three `Given` steps establish nothing — `OPEN`, P3

**Status:** contract fidelity. **Scenarios 1, 5 and 8.**

`dk_and_two_recipients` (`cucumber.rs:7547-7550`), `single_grantee` (`:7575`)
and `derived_node_rotated` (`:7597-7600`) all have empty bodies and unused
World parameters. In each case the precondition the Gherkin names is
re-created inside the `When` from compile-time constants, or — for scenario 8 —
never created at all.

Deleting any of the three would change no outcome. The contract text is
therefore unexecutable: it can be edited without affecting the test, and the
test's real inputs cannot be read from the contract. This is the "empty,
generic, or proxy step" case explicitly in scope per `PROCESS.md`.

For scenario 8 this finding is not merely cosmetic — it is the mechanism of
`CHDR-001`.

**Closure criteria.** Have each `Given` place its named state in the World and
each `When` consume it.

### `CHDR-014` — the I3 rejection is matched by `Display` substring — `OPEN`, P3

**Status:** proof brittleness. **Scenario 5.**

`build_without_owner` (`cucumber.rs:8134`) stores `e.to_string()`, destroying
the typed error at the World boundary; `header_invalid`
(`cucumber.rs:12348-12349`) then asserts `msg.contains("I3")`. The typed variant
`Error::MissingOwnerLine` is public and `Error` derives `PartialEq`
(`error.rs:6`, `:59-60`), so a typed assertion is available and unused.

The assertion is *currently* exact, because `build_at` (`header.rs:124-152`)
has exactly one fallible statement — `check_owner_line` at `:133` — but that is
a property of today's code, not of the test. Any new fallible check in
`build_at`, or a node path containing the literal `I3` (the payload is
`node.to_owned()`, `header.rs:75`), would satisfy the assertion without the
owner check having been the cause; conversely, rewording the message breaks the
scenario while the invariant holds.

The scenario is nonetheless `PROVEN`: it cannot pass unless `Header::build`
genuinely returns `Err`, because the `When` panics on `Ok` (`cucumber.rs:8133`)
*and* the `Then`'s `unwrap()` on a `None` `rejection` is independently
fail-closed.

**Closure criteria.** Store the typed error and assert
`matches!(err, Error::MissingOwnerLine(ref n) if n == NODE_A)`.

### `CHDR-015` — I3 is not enforced at the edition level — `DECISION_REQUIRED`, P2

**Status:** surface finding, outside the eight scenarios' semantic truth.

Spec `03-headers.md:36-37` states two things: every key version must include
the owner line, **and** "an edition whose any header violates this is
invalid". The first is enforced at three points in `aithos-core`
(`Header::build`/`build_at` via `check_owner_line` at `header.rs:133`;
`Header::rotate` at `:201`; `Header::validate` at `:308-315`). The second is
enforced nowhere.

`aithos-bundle/src/state.rs:59-68` (`header_hash_at`) parses `header.json` as
untyped `serde_json::Value` and folds its JCS bytes into the Merkle state tree
without ever constructing a `Header`. `Bundle::verify`
(`aithos-bundle/src/bundle.rs:1652-1758`), the offline edition verifier, checks
the DID document, manifest signatures, the hash chain, pinned digests, stray
files, the gamma chain and the recomputed roots — and calls `Header::validate`
on nothing. The grant, structure, revoke and vault read paths
(`grants.rs:287,456`, `structure.rs:199,284,573,751`,
`revoke.rs:155,289,365,510`, `vault.rs:114,335,358`) deserialize headers
without validating them.

So a header lacking an owner line cannot be *created* through the `aithos-core`
constructors, but one arriving by any other route — a hand-edited
`header.json`, an imported bundle, a future writer, a `serde` round-trip —
would be hashed into the state tree, pinned, signed into a manifest, and pass
`Bundle::verify` unchallenged.

**Why this is a decision and not a correction.** Closing it requires choosing
between competing product semantics: (a) `Bundle::verify` validates every
header in the edition, making I3 an edition-level invariant as the spec text
reads, at the cost of parsing every header on every verify; (b) I3 stays a
construction-time invariant and the spec sentence is narrowed to say so; (c)
validation moves to the read paths only. A corrector must not choose this
implicitly.

**Expected owner.** Human — the protocol owner.

This finding is recorded because the audit was explicitly asked whether an
I3-violating header can enter or survive an edition through a path the scenario
never crosses. It does not qualify scenario 5's `PROVEN` verdict: the Gherkin
claims only build-time rejection, and absence of a scenario is out of scope.

### `CHDR-016` — the one test that guards version binding guards it vacuously — `OPEN`, P2

**Status:** `PARTIAL`. **Not a scenario finding** — a core-test finding
surfaced by the `CHDR-006` mutation, recorded here because it is the reason
`CHDR-006` matters more than it looks.

`aithos-core/tests/c1_header_seal.rs:105-107` is the repository's only explicit
negative test for key-version binding:

```rust
let other_ver = line_aad(&v.subject_did, &v.node, v.key_version + 1);
assert!(open_line(&sk, &epk, &c, &n, &other_ver).is_err());
```

Under the `CHDR-006` mutation — `line_aad` with the `key_version` component
removed — this assertion **still passed**. It passed vacuously: with
`key_version` gone, the recomputed AAD no longer matches the vector's
ciphertext at all, so the open fails for an entirely different reason than the
one the test names. The same reasoning applies to the sibling assertions in
`c1_fail_closed` (`:92-107`), each of which asserts only `is_err()` on a
ciphertext whose baseline openability is established in a *different* test
function.

So version binding in this repository is protected by exactly one thing: the
byte cross-check at `c1_header_seal.rs:66` against independently generated
Python bytes. Nothing behavioral defends it, including the test written to
defend it.

**Closure criteria.** Give `c1_fail_closed` a positive control in its own body
— assert that the unmodified `(sk, epk, c, n, aad)` tuple opens to `dk_hex`
first — so that each subsequent negative assertion is a genuine differential
against a known-good baseline rather than an unanchored `is_err()`.

## 7. Implementation plan

Ordered by value. The whole set is test-and-fixture work in
`rust/crates/aithos-bundle/tests/cucumber.rs`, plus one addition in
`rust/crates/aithos-core/tests/c1_header_seal.rs` and one Gherkin edit. **No
production change in `aithos-core` or `aithos-bundle` is required by any
finding except, at the decision owner's discretion, `CHDR-015`.**

That is itself a result worth stating plainly: this audit found the header
implementation faithful to spec §03 everywhere it looked. What it found weak is
the evidence, not the product.

| Lot | Findings | Change | Expected RED |
|---|---|---|---|
| 1 | `CHDR-001`, `CHDR-005`, `CHDR-013` (scenario 8) | Rebuild scenario 8 on real derivation: derive `K_P`, derive the child key, rotate the child, wrap that rotation's key, recover `K_P` by derivation before opening | Today's scenario passes with `PARENT_KEY` replaced by any constant; after the fix it must fail when the wrap is sealed under a key unrelated to the parent |
| 2 | `CHDR-002`, `CHDR-003`, `CHDR-004` | Add the structural check and `check_rotation(2)` to scenario 7's `Then` | Inject a `g1` line into v2 by hand → must fail; delete the `rotate` call → must fail |
| 3 | `CHDR-007` | Differential rejection assertions with a positive control, in `corrupt_line` and `replay_line_other_node` | Make the owner line unopenable in the `Given` → must fail, where today it passes |
| 4 | `CHDR-006` | Add the cross-version replay attempt to scenario 4 | Drop `key_version` from `line_aad` → must fail, where today it passes |
| 5 | `CHDR-010`, `CHDR-011`, `CHDR-012` | Two-recipient fixture for scenario 6, whole-vector snapshot, prefix + length assertions, post-append owner re-open | Change `push` to `insert(0, …)` → must fail; re-seal the surviving lines on append → must fail |
| 6 | `CHDR-008` | Derive the kid list from the header; assert attempt count | Add a third line to the fixture → must fail |
| 7 | `CHDR-009`, `CHDR-016` | Vector-anchor the `Header` layer in `c1_header_seal.rs`; give `c1_fail_closed` a positive control | Swap the recipient/ephemeral zip order in `build_lines` → must fail; drop `key_version` from `line_aad` → `c1_fail_closed` must fail *on its version case*, not merely somewhere |
| 8 | `CHDR-013`, `CHDR-014` | Populate the three empty `Given`s; assert the typed I3 error | Reword the I3 message → must **not** fail after the fix |

Lot 1 and lot 2 are the security-relevant ones and should land first.

## 8. Decisions required

`CHDR-015` — whether I3 is an edition-level invariant enforced by
`Bundle::verify`, a construction-time invariant with the spec text narrowed to
match, or a read-path check. Owner: the protocol owner. No corrector may
choose this implicitly.

## 9. Definition of done

- Every `OPEN` finding above is either `VERIFIED` by an independent review or
  explicitly carried forward with a recorded reason.
- Each correction lands with a RED test demonstrated to fail on the audited
  baseline for the intended reason, and the corrector documents both results.
- The canonical feature gate reports the expected scenario/step counts after
  the corrections — counts, not exit code, while `BDER-011` is open.
- The corrector runs the relevant regressions named in `DOMAIN.md`
  (`c1_header_seal`, `g2_rotation`, `g3_move`) and one final unfiltered
  Cucumber and workspace gate before handoff.
- `CHDR-015` has a recorded decision before any correction touches
  `Bundle::verify`.
- Gherkin audit markers are removed for every finding accepted as `VERIFIED`.

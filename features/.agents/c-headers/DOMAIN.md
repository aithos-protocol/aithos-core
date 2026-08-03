# Domain — `c-headers.feature`

## Contract

This feature covers the header object, the single place a node key is ever
stored, and it is stored sealed (`features/c-headers.feature:1-6`):

- one sealed line per authorized identity, each line opening for exactly one
  recipient;
- the mandatory owner line (I3);
- grant as one appended line, `O(1)`, touching no other line;
- rotation as a new key version without the revoked;
- the derivation up-link wrap that restores the parent → child route broken by
  a fresh random key.

Four `Rule` blocks, eight scenarios, twenty-eight steps (counted from
`features/c-headers.feature`).

The public audit will be `docs/audits/features/c-headers.md`. It does not exist
yet in this extract; `docs/audits/features/README.md:20` already reserves the
`CHDR-001` identifier family for this feature, so findings take stable
`CHDR-*` identifiers.

## Branch and evidence

- Canonical audit branch: `codex/audit-c-headers-r2`
  (`features/.agents/c-headers/STATE.md` frontmatter, key `branch`).
- The `PROCESS.md` default name `codex/audit-<feature-name>` is **not** free
  here: `features/.agents/orchestrator/QUEUE.yaml:55-56` registers
  `codex/audit-c-headers` as this feature's *yardstick* — prior manual work
  that is a Pass B input and a milestone comparison only, never a Pass A
  input. The train's branch therefore carries the `-r2` suffix so the two are
  never confused.
- Corrections use a dedicated `codex/fix-c-headers-<finding-or-scope>`
  descendant branch of the immutable audited revision
  (`features/AGENTS.md:13-15`, `features/.agents/PROCESS.md:39-41`).
- `base_main`, `audit_revision`, and `candidate_revision` are `null` in
  `STATE.md`: no revision has been frozen for this feature yet. The role that
  freezes one records it there and in its run report.
- No gate has been run for this feature by this role. Every number below is a
  command to run, never a result.

## Protocol invariants

Numbered claims, each with the specification text that governs it and the
symbol that implements it.

1. A line is an X25519 + HKDF-SHA256 + XChaCha20-Poly1305 seal of the node DK
   to one recipient public key, with its own ephemeral stored in the line
   (`spec/03-headers.md:29-32`, `:119-129`) —
   `aithos_core::seal::seal_line` / `open_line`
   (`rust/crates/aithos-core/src/seal.rs:92-132`).
2. The KEK is `HKDF-SHA256(ikm = X25519(esk, recipient_pub), salt = ∅,
   info = "aithos-core/v1/hdr-kek" ‖ 0x00 ‖ epk ‖ recipient_pub)`
   (`spec/03-headers.md:123-125`) — `seal.rs:83-89` (`KEK_INFO`, `fn kek`).
3. The line AAD binds `subject_did ‖ node ‖ key_version` under purpose
   `aithos-core/v1/header-line` (`spec/03-headers.md:32`, `:126-128`,
   `spec/00-overview.md:57-60`) — `aithos_core::seal::line_aad`
   (`seal.rs:35-37`, `fn aad` at `:21-31`). Replay on another node or another
   version therefore fails at the AEAD tag, not at a routing check.
4. `to` / `kid` are routing hints only; the seal is what grants
   (`spec/03-headers.md:33-35`) — `Header::open` filters on `kid` and then
   tries the seal, returning the first line that actually opens
   (`rust/crates/aithos-core/src/header.rs:221-246`).
5. **I3** — every `key_versions[*].lines` MUST include the owner line, and an
   edition carrying a violating header is invalid
   (`spec/03-headers.md:36-37`, `spec/00-overview.md:33-35`) —
   `check_owner_line` (`header.rs:71-77`) on `build` / `build_at` / `rotate`,
   plus parse-time `Header::validate` over every version
   (`header.rs:308-315`). The rejection text is
   `"I3 violated — header without an owner line: {0}"`
   (`rust/crates/aithos-core/src/error.rs:59-60`).
6. Grant = append one line to the current version, content and other lines
   untouched (`spec/03-headers.md:46-58`) — `Header::append_line`
   (`header.rs:159-188`), which pushes onto `kv.lines` and rewrites nothing.
7. Rotation = a new key version whose lines are the survivors plus the owner;
   the revoked simply has no line and cannot derive the fresh random DK'
   (`spec/03-headers.md:64-88`, `spec/06-revocation.md:25-44`) —
   `Header::rotate` (`header.rs:192-217`).
8. Rotation well-formedness is mechanical: the new version's recipient set MUST
   be the previous version's minus the revoked, and a smuggled-in recipient is
   fail-closed (`spec/03-headers.md:93-96`) — `Header::check_rotation`
   (`header.rs:275-305`), which also re-checks the owner line.
9. The up-link wrap seals DK' under a key derived from the parent key with
   context `aithos-core/v1/wrap`, AAD purpose `aithos-core/v1/tagwrap` bound to
   `subject_did ‖ wrapped_node ‖ key_version` (`spec/03-headers.md:72-84`,
   `:130-134`) — `Wrap::seal` / `Wrap::open` (`header.rs:330-357`) over
   `wrap_seal` / `wrap_open` / `wrap_aad` (`seal.rs:136-163`, `:41-43`) and
   `derive_key(CTX_WRAP_KEY, via_key)` (`seal.rs:19`, `:137`).
10. Old key versions are retained while un-re-encrypted blobs reference them;
    reads target the newest lock (`spec/03-headers.md:98-106`) —
    `Header::latest_version` / `open_latest` (`header.rs:250-269`).
11. A header leaks only the set of recipient public keys of its node — no
    scope, verb, or human label (`spec/03-headers.md:108-113`) — the `Line`
    struct carries exactly `to`, `kid`, `epk`, `n`, `c`
    (`header.rs:33-40`).
12. The header hash is folded into its node's Merkle hash, so appending a line
    or rotating bumps the node's proof path (`spec/03-headers.md:115-117`,
    `spec/02-content-tree.md:556`, `:567`, `:574`). This is the seam with
    `h-merkle.feature`; it is **not** claimed by any `c-headers` scenario.
13. A node below the zone root may carry its own header, and derivation is the
    default route while a header line is the granted one — both resolve
    (`spec/02-content-tree.md:123-127`). This is the seam with
    `b-derivation.feature`.

Invariants 12 and 13 are stated here as context for the integration pass, not
as obligations of the eight scenarios.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/c-headers.feature` |
| Steps | `rust/crates/aithos-bundle/tests/cucumber.rs` |
| Header object | `rust/crates/aithos-core/src/header.rs` |
| Seal primitives | `rust/crates/aithos-core/src/seal.rs` |
| Error surface (I3, seal rejection) | `rust/crates/aithos-core/src/error.rs` |
| Wrap key derivation | `rust/crates/aithos-core/src/derive.rs` (`derive_key`) |
| Core test | `rust/crates/aithos-core/tests/c1_header_seal.rs` |
| Rotation / move neighbours | `rust/crates/aithos-core/tests/{g2_rotation,g3_move}.rs` |
| Vector | `vectors/c1-header-seal.json` |
| Vector pin | `vectors/ownership.json` (entry `c1-header-seal.json`, `owner: core`, `sha256: af0f63bd…`), enforced by `rust/crates/aithos-bundle/tests/vectors_ownership.rs` |
| Specification | `spec/03-headers.md` §3.1, §3.2, §3.3, §3.4, §3.5, §3.6, §3.8 ; `spec/00-overview.md` §0.2 (I1, I2, I3), §0.3 ; `spec/02-content-tree.md` §2.5, §2.9, §2.10 ; `spec/06-revocation.md` §6.1, §6.2 ; `spec/09-cli-and-conformance.md` §9.2 |

### Vectors involved

- `vectors/c1-header-seal.json` — C1 (owner line and grantee line, byte-exact
  `epk` / `c`) and C2 (wrap), all ephemerals and nonces fixed as inputs. Read
  only by `rust/crates/aithos-core/tests/c1_header_seal.rs:41-46`. **No
  Gherkin step of `c-headers.feature` reads it**: the step fixtures are
  hand-made constants (`cucumber.rs:258-284`), and the comment at
  `cucumber.rs:258` states the split explicitly — "behavioral; byte-exactness
  lives in C1". An auditor must decide, per scenario, whether that split leaves
  a stated byte-level claim unproven inside the feature.
- `vectors/g2-rotation.json` — the mechanical rotation rule and up-link wrap
  bytes (`rust/crates/aithos-core/tests/g2_rotation.rs`).
- `vectors/g3-move.json` — new-path AAD bindings and the wrap under the new
  parent (`rust/crates/aithos-core/tests/g3_move.rs:145-176`).

## Shared steps, fixtures, and helpers

All in `rust/crates/aithos-bundle/tests/cucumber.rs`.

Line numbers below point at the `fn` line of each step definition.

- Fixtures: `DID_C`, `NODE_A`, `NODE_OTHER`, `CHILD_NODE`, `DK`, `DK2`,
  `PARENT_KEY`, `xsk`, `owner_rec`, `grantee_rec`, `eph`, `non`
  (`:258-285`).
- World fields: `header`, `saved_line`, `opened`, `wrap_obj` (`:486-489`),
  plus the shared `rejection` field consumed by the I3 `Then`.
- Helper: `ProtocolWorld::open_into(version, kid, sk_byte)` (`:7396`) —
  every negative scenario funnels through it, so the integration pass must
  check that `opened` is not carried across scenarios.
- `Given`: `dk_and_two_recipients` (`:7548`, an empty body),
  `sealed_header_owner_grantee` (`:7553`), `sealed_header_owner_only`
  (two `#[given]` phrases on one function, `:7569`), `single_grantee`
  (`:7576`, an empty body), `sealed_header_three` (`:7579`),
  `derived_node_rotated` (`:7598`, an empty body).
- `When`: `seal_into_header` (`:8092`, delegates to the `Given`),
  `stranger_tries` (`:8097`), `corrupt_line` (`:8104`),
  `replay_line_other_node` (`:8114`), `build_without_owner`
  (`:8124`), `append_grantee_line` (`:8139`), `rotate_without_g1`
  (`:8148`), `post_uplink_wrap` (`:8164`).
- `Then`: `owner_opens` (`:12312`), `grantee_opens` (two phrases,
  `:12324`), `stranger_recovers_nothing` (`:12335`),
  `opening_rejected` (two phrases, `:12342`), `header_invalid`
  (`:12347`, asserts the message contains `I3`), `owner_line_untouched`
  (`:12353`), `survivor_opens` (`:12364`), `revoked_cannot_open`
  (`:12375`), `owner_opens_new` (`:12385`),
  `parent_recovers_via_wrap` (`:12396`).

Three `Given` bodies are empty and three step functions are shared by two
Gherkin phrases. Both facts are inputs to the trace, not verdicts.

## Public surfaces that claim the same invariants

Inspect these to check whether they bypass or contradict the header verdict,
not to audit them.

- Bundle grant path: `rust/crates/aithos-bundle/src/grants.rs:276-305`
  (`add_line_on` — append on an existing header, `Header::build` with the owner
  line otherwise) and `:324-345` (tag-view anchor plus `Wrap::seal` bridging),
  `:460`, `:468`; header/wrap file layout `hdr_file` / `wrap_file`
  (`grants.rs:139-155`).
- Bundle read path: `rust/crates/aithos-bundle/src/bundle.rs:552`, `:565`,
  `:630`, `:637` (`validate` before `open`), `:673`
  (`owner_current_section_key_with_kex`, deepest ancestor header at its latest
  version).
- Rotation / revocation: `rust/crates/aithos-bundle/src/revoke.rs:163`,
  `:199` (`check_rotation` — "fail-closed: no smuggled recipient"), `:205`,
  `:290`, `:413` (`Header::build_at`), `:428` (`Wrap::seal`).
- Structural mutations: `rust/crates/aithos-bundle/src/structure.rs:201`,
  `:332-341`, `:757`, `:777` (`build_at`), `:788`, `:864-873`.
- Vault: `rust/crates/aithos-bundle/src/vault.rs:121`, `:336`, `:400`
  (`check_rotation`).
- Delegated session append: `rust/crates/aithos-bundle/src/session.rs:363-365`
  (`validate` then `open_latest` then `append_line`).
- Audit log / vault lines: `rust/crates/aithos-bundle/src/log.rs:425`,
  `:446` (`grant_audit_line`).
- CLI: `rust/crates/aithos-cli/src/cmd/header_seal.rs` (injects one ephemeral
  and one nonce per line at the surface, `:45-56`) and
  `rust/crates/aithos-cli/src/cmd/header_open.rs` (`validate` then `open`,
  `:27-32`).
- `rust/crates/aithos-wasm/src/lib.rs` exposes **no** header or wrap surface
  (no `Header`, `Wrap`, or `seal` symbol in the file).

## Known coupling with other features

- `b-derivation.feature` — the alternative route to a node key. Its own domain
  names `c-headers` explicitly: "node-local DK and up-link wrap (spec 03.4),
  the alternative route to derivation"
  (`features/.agents/b-derivation/DOMAIN.md:122-123`). The wrap key is
  `derive_key("aithos-core/v1/wrap", K_via)` (`seal.rs:137`), so a derivation
  change reaches the wrap.
- `g-revocation.feature` — the same rotation and up-link wrap, at bundle level:
  `:17`, `:62`, `:65-69`, `:72-74`, `:76-79`, `:123-126`, `:133`, `:150`.
  `g2_rotation.rs` and `g3_move.rs` are its Core-side vectors.
- `n-structural-mutations.feature` — move as rotation, survivor lines and the
  destination up-link wrap (`:53`, `:60`, `:66`, `:84`); move-specific header
  construction is `Header::build_at` (`header.rs:124-155`).
- `h-merkle.feature` — the header hash folded into the node hash, one proof
  attesting row, header version and wraps at once (`:5-10`, `:55`).
- `d-bundle.feature` — atomicity of header and wrap writes (`:98`, `:106`,
  `:112`) and the "node-version-and-recipient header line" capability row
  (`:146`), whose step observation is
  `core_header_capability_scenario` (`cucumber.rs:3034-3092`).
- `o-connector-classes-vault.feature` — vault header lines remain opaque and
  non-authorizing (`cucumber.rs:11897`, regex alternative "encrypted normative
  header lines remain opaque and non-authorizing").
- **Recorded follow-up.** `features/.agents/orchestrator/QUEUE.yaml:61-63`
  registers `b-derivation-round-2-targeted` over `c-headers`. The accepted
  round-2 impact review states the obligation precisely
  (`features/.agents/orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`):
  `vectors/c1-header-seal.json` and `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3`
  claim independent generation while no `gen-c1*` generator exists in
  `vectors/`; the classification is `TARGETED` — evidence class to requalify,
  behaviour intact. The same note records that `c-headers` shares **no** step
  with the retitled `b-derivation` Rule and that its four `wrap` occurrences
  (`c-headers.feature:6`, `:55-58`) are the §03.4 up-link wrap, not the §02.9
  tag anchor.

## Gate pyramid

Canonical feature tag: `@c-headers` (`features/c-headers.feature:1`).

Run the static check from the repository root:

```text
features/.agents/scripts/verify-feature-tags.sh
```

Run Cargo commands from the repository root with the workspace manifest.

### Auditor evidence — once per immutable revision

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers
```

The auditor runs no unfiltered Cucumber, broad regression, or workspace gate.
It may run one exact focused test only to resolve a semantic contradiction.

### Focused tests

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal
```

Its three tests are `c1_owner_and_grantee_lines`, `c1_fail_closed`, and
`c2_wrap_roundtrip_and_cross_check`
(`rust/crates/aithos-core/tests/c1_header_seal.rs:76`, `:84`, `:110`). Name the
exact test with `-- --exact <name>` when a single semantic contradiction is at
stake.

### Relevant regressions — corrector, after the final correction

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal --test g2_rotation --test g3_move --test b2_derivation
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb10_structure_vault --test vectors_ownership
```

Why each: `g2_rotation` covers `check_rotation` and the up-link wrap bytes;
`g3_move` covers `build_at` and the wrap under the new parent; `b2_derivation`
covers `derive_key`, on which the wrap key depends; `cb10_structure_vault`
exercises `open_latest` on a rotated header
(`cb10_structure_vault.rs:526-560`); `vectors_ownership` fails if
`vectors/c1-header-seal.json` moves without its `sha256` pin being updated
(`vectors_ownership.rs:182-199`).

### Final global gates — corrector, once before review handoff

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

### Reading the counters

The Cucumber runner scans every feature not tagged `@wip`. The `@c-headers`
gate must select **1 feature / 4 rules / 8 scenarios / 28 steps**, counted from
`features/c-headers.feature` in this extract. A gate that reports a different
count has not run this contract. Neither role runs a gate once per scenario or
review unit.

Reading the printed counts is not a style preference. The exit code of the
`aithos-bundle --test cucumber` runner became trustworthy only after `BDER-011`
was closed and independently accepted on 2026-07-30 (per
`features/.agents/b-derivation/STATE.md:34` and
`features/.agents/a-identity/DOMAIN.md:119-127`); the recorded-gate rules in
`features/.agents/orchestrator/LEDGER.md:44-51` treat exit 0 with zero selected
scenarios, or with failures reported, as red whatever the exit code says.

### Pre-gate status, verified on this revision

`features/.agents/scripts/verify-feature-tags.sh` is mandatory before any
audit, correction, or review (`features/AGENTS.md:24`,
`features/.agents/PROCESS.md:58-59`). On the current revision it is **green**:
`feature tags ok (19 files)`, exit 0.

An earlier note in `features/.agents/b-derivation/STATE.md` describes it as red
repository-wide. That observation belonged to revisions predating the canonical
tag lot, and no longer holds. It is recorded here only so a reader who meets
the old note does not act on it.

The general rule this illustrates: never restate a gate result read from a
document. Run the gate, or cite the ledger entry of a run. A written record of
a past gate is history, and `PROCESS.md` is explicit that history is context,
not proof.

## Pilot limits

Audit only the semantic truth of the eight existing scenarios. Do not design
new general scenarios, and do not extend the audit to revocation, move,
Merkle, or bundle-atomicity features — report the impact instead. Findings take
stable `CHDR-*` identifiers (`docs/audits/features/README.md:20`).

## Open questions this domain could not resolve from the extract

1. Whether `docs/audits/features/c-headers.md` and the yardstick branch
   `codex/audit-c-headers` (`QUEUE.yaml:56`) contain prior manual findings:
   neither exists in this extract, which has no `.git` directory. Treat the
   yardstick strictly as a Pass B input when it becomes readable.
2. Whether `base_main` for this cycle is the same `main` revision as
   `b-derivation`'s last recorded baseline. No revision is observable here.

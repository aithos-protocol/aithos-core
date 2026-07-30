# Domain — `c-headers.feature`

## Contract

This feature covers the header object, the only place a node key is ever
stored, and it is stored sealed:

- one line per authorized identity, each an independent ECIES seal of the
  node key to one recipient public key;
- the owner line is mandatory in every key version (I3);
- grant is one appended line, `O(1)`, touching no other line;
- rotation is a new key version sealed to the survivors only, plus an up-link
  wrap that restores the parent→child derivation path severed by a fresh
  random key.

The public audit is `docs/audits/features/c-headers.md`.
Findings take stable `CHDR-*` identifiers.

## Branch and evidence

- Canonical audit branch: `codex/audit-c-headers`.
- Created from local `main` at
  `240c6589986af6115530c90a7aa8646c2c44b68f`.
- Initial audit evidence revision: recorded by the auditor in `STATE.md`
  before tracing, and never silently rebased afterwards.
- Corrections use a dedicated `codex/fix-c-headers-<finding-or-scope>`
  descendant branch.

## Protocol invariants

1. A line is `X25519 → HKDF-SHA256 → XChaCha20-Poly1305` with its own
   ephemeral, `epk` stored in the line (spec 03.1, 03.8). Appending a line
   never needs another line's ephemeral — that is what makes grant `O(1)`.
2. KEK derivation binds the transcript:
   `HKDF-SHA256(ikm = X25519(esk, recipient_pub), salt = ∅,
   info = "aithos-core/v1/hdr-kek" ‖ 0x00 ‖ epk ‖ recipient_pub)` (spec 03.8).
3. Line AAD is
   `"aithos-core/v1/header-line" ‖ 0x00 ‖ subject_did ‖ 0x00 ‖ node ‖ 0x00 ‖
   key_version`, `key_version` as decimal ASCII. A line is therefore bound to
   its subject, its node **and** its version: replay across any of the three
   must fail closed (spec 03.8).
4. `to` and `kid` are routing hints only. The seal is what grants; a reader
   tries every line matching its `kid` and the AEAD decides (spec 03.1, 03.2).
5. **I3:** every `key_versions[*].lines` MUST include the owner line. A header
   violating this is invalid, and so is the edition containing it
   (spec 03.1).
6. Grant (spec 03.3): open the current key, seal it to the new recipient,
   append. Content untouched, other lines byte-identical, node key unchanged.
7. Rotation (spec 03.4): fresh random `DK'`, `key_version += 1`, one line per
   surviving recipient plus the owner. The revoked gets no line and cannot
   derive `DK'` because it is random, not derived.
8. Rotation well-formedness (spec 03.4): the new version's recipient set must
   equal the previous version's minus the revoked. A smuggled-in recipient —
   one whose `kid` is absent from the prior version — invalidates the
   rotation, fail-closed.
9. Up-link wrap (spec 03.4 step 2bis): `seal(DK'_N)` openable via `K_P`, key
   `derive_key("aithos-core/v1/wrap", K_via)`, AAD purpose
   `"aithos-core/v1/tagwrap"` bound to `subject_did ‖ wrapped_node ‖
   key_version`. It restores derivation for holders of the parent or any
   ancestor without giving them a line. An up-link wrap whose author does not
   hold the parent is rejected.
10. Old key versions are retained while any un-re-encrypted blob references
    them (spec 03.5); eager re-encryption drops them in the same edition.
11. Header hygiene (spec 03.6): a header leaks only the recipient public-key
    set of its node — no scope, verb, or human label.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/c-headers.feature` |
| Steps | `rust/crates/aithos-bundle/tests/cucumber.rs` |
| Header object | `rust/crates/aithos-core/src/header.rs` |
| Seals and AADs | `rust/crates/aithos-core/src/seal.rs` |
| Derivation (wrap key) | `rust/crates/aithos-core/src/derive.rs` |
| Core test | `rust/crates/aithos-core/tests/c1_header_seal.rs` |
| Vectors | `vectors/c1-header-seal.json` (C1 line, C2 wrap) |
| Specification | `spec/03-headers.md` §3.1–3.6, §3.8; `spec/02-content-tree.md` §2.5, §2.9; `spec/06-revocation.md` |

The header fixtures live in the Cucumber World: `DID_C`, `NODE_A`,
`NODE_OTHER`, `CHILD_NODE`, `DK`, `DK2`, `PARENT_KEY`, `owner_rec()`,
`grantee_rec()`, `eph()`, `non()`, `xsk()`, `ProtocolWorld::open_into`,
and the `header` / `saved_line` / `opened` / `wrap_obj` / `rejection` World
fields. Several `Given` and `When` phrases for this feature share those
fixtures; treat the fixture layer as part of the traced surface.

## Gate pyramid

Canonical feature tag: `@c-headers`.

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
It may run an exact focused or mutation test only to resolve a semantic
contradiction.

### Corrector focused and relevant regressions

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test g2_rotation
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test g3_move
```

Repeat only the focused RED/GREEN proof while implementation changes. Run the
canonical feature gate once after the final correction.

### Corrector final integration — once before review handoff

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

### The gate's exit code is not evidence yet

`BDER-011` is open on this baseline: `aithos-bundle/tests/cucumber.rs` calls
`filter_run`, not `filter_run_and_exit`, under `harness = false`, so the runner
exits `0` even when scenarios fail. Until `BDER-011` is closed, the printed
scenario/step block is the **only** evidence the gate produces. Read the
counts; do not report `EXIT=0` as a pass.

The Cucumber runner scans all features not tagged `@wip`. Confirm the executed
`c-headers` counts against the feature file at the observed revision — four
`Rule` blocks, eight scenarios, 28 steps as written today — and record any
divergence rather than the global exit code. Neither role runs a gate once per
scenario or review unit.

## Surfaces and neighboring domains to inspect

- Header consumers in the bundle:
  `rust/crates/aithos-bundle/src/{bundle,grants,revoke,structure,vault,state,session,log}.rs`
  — the real grant, rotation, and validation paths that must not bypass the
  verdict proved by the scenarios;
- CLI header surface: `rust/crates/aithos-cli/src/main.rs`;
- `b-derivation.feature`: the node-key derivation route the up-link wrap
  restores (spec 02.5, 03.4);
- `g-revocation.feature` and `rust/crates/aithos-core/tests/g2_rotation.rs`:
  rotation as revocation rung 2, where `check_rotation` is the well-formedness
  gate;
- `n-structural-mutations.feature` and
  `rust/crates/aithos-core/tests/g3_move.rs`: `Header::build_at`, the moved
  node whose first version is not 1;
- `d-bundle.feature`: tag wraps, which share the `tagwrap` primitive and AAD
  with the up-link wrap;
- `h-merkle.feature`: the header hash folded into the node's Merkle path
  (spec 03.6, 02.10);
- `m-delegated-editions.feature`, `e-mandates.feature`,
  `o-connector-classes-vault.feature`: other features whose Gherkin mentions
  headers or wraps and may share step phrases or World state.

Textual proximity is not a semantic dependency. Inspect these surfaces to
check whether they bypass or contradict the header verdict, not to audit them.

## Pilot limits

Audit only the semantic truth of the eight existing scenarios. Do not design
new general scenarios, and do not extend the audit to revocation, move,
tag-view, or Merkle features. Additional tests needed to prove a requested
correction remain in scope.

Note what this feature's Gherkin does **not** claim, and do not turn it into a
finding: retention of old versions (spec 03.5), `check_rotation`'s
smuggled-recipient rejection, the rejection of an up-link wrap posted by a
non-holder of the parent, and header hygiene (spec 03.6) have no scenario
here. Absence of a scenario is out of scope; a scenario that claims one of
these without proving it is in scope.

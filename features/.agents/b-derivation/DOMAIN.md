# Domain — `b-derivation.feature`

## Contract

This feature covers content-tree key derivation:

- one BLAKE3 `derive_key` per canonical path segment;
- derivation labels built from sids, never from human names;
- one-wayness: a folder key yields its whole subtree, present and future,
  and nothing above or beside it;
- rename re-keys nothing;
- tag views as anchors derived at any folder, zone root included.

The public audit is `docs/audits/features/b-derivation.md`.

## Branch and evidence

- Canonical audit branch: `codex/audit-b-derivation`.
- Initial audit evidence revision: `891c808`.
- Current `main` integration baseline:
  `5c3a61852dee0886fb6fff008a6304e8ea2c71bb`.
- Corrections use a dedicated
  `codex/fix-b-derivation-<finding-or-scope>` descendant branch.

The initial evidence remains tied to `891c808`. The A-Identity impact review
classified `b-derivation` as `NONE`, allowing the audit record to move onto the
new baseline without claiming that the newer revision was the original audit
target.

## Protocol invariants

1. `K(child folder) = derive_key("aithos-core/v1/d/" + sid, K(parent))`
   (spec 02.5).
2. `K(section) = derive_key("aithos-core/v1/s/" + sid, K(folder))`.
3. `K(tag anchor) = derive_key("aithos-core/v1/t/" + tag, K(folder))`.
4. Determinism: the same zone DK and the same canonical path always yield the
   same key, byte-exact against the B2 vectors.
5. Domain separation: the `d` / `s` / `t` markers keep contexts disjoint; an
   identical sid used as a folder and as a section must not collide.
6. Labels use sids, never names, so renaming a node re-keys nothing
   (spec 02.2, 02.9).
7. One-wayness: no derivation from a folder key reaches an ancestor or a
   sibling subtree.
8. Subtree coverage is present *and* future: a folder key derives descendants
   created after the key was granted.
9. A tag anchor is a node key distinct from its folder key and from the same
   tag anchored at another folder. Derivation grants nothing downward from an
   anchor: sections enter a tag view by wrap (spec 02.9).
10. Depth is unlimited and reading at depth *d* costs exactly *d* derivations.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/b-derivation.feature` |
| Steps | `rust/crates/aithos-bundle/tests/cucumber.rs` |
| Derivation | `rust/crates/aithos-core/src/derive.rs` |
| Canonical paths | `rust/crates/aithos-core/src/path.rs` |
| Sids | `rust/crates/aithos-core/src/ids.rs` |
| Core test | `rust/crates/aithos-core/tests/b2_derivation.rs` |
| Vectors | `vectors/b2-derivation.json` |
| Specification | `spec/01-identity-and-keys.md` §1.3, `spec/02-content-tree.md` §2.1, §2.2, §2.5, §2.9 |

## Gate pyramid

Canonical feature tag: `@b-derivation`.

Run the static check from the repository root:

```text
features/.agents/scripts/verify-feature-tags.sh
```

Run Cargo commands from the repository root with the workspace manifest.

### Auditor evidence — once per immutable revision

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @b-derivation
```

The auditor runs no unfiltered Cucumber, broad regression, or workspace gate.
It may run an exact focused or mutation test only to resolve a semantic
contradiction.

### Corrector focused and relevant regressions

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test b2_derivation
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

The Cucumber runner scans all features not tagged `@wip`. Confirm the exact
number of executed `b-derivation` scenarios in its output — three Rules, six
scenarios and 30 steps since the round 1 correction (21 before) — not only the
global exit code. Neither role runs a gate once per scenario or review unit.

Reading the printed counts is not a style preference here. Until `BDER-011` is
closed, this runner exits 0 even when scenarios fail
(`aithos-bundle/tests/cucumber.rs` calls `filter_run`, not
`filter_run_and_exit`, under `harness = false`), so the printed block is the
only evidence the gate produces.

## Surfaces and neighboring domains to inspect

- Bundle consumers of `node_key`: `rust/crates/aithos-bundle/src/{bundle,grants,structure,revoke,log}.rs`;
- Core consumers: `rust/crates/aithos-core/src/{seal,gamma}.rs`;
- CLI derivation surface: `rust/crates/aithos-cli/src/main.rs`;
- `c-headers.feature`: node-local DK and up-link wrap (spec 03.4), the
  alternative route to derivation;
- `n-structural-mutations.feature` and `rust/crates/aithos-core/tests/g3_move.rs`:
  rename and move, where move is a rotation precisely because derivation cannot
  be un-taught;
- `d-bundle.feature`: tag-view rebuild and the wraps that populate an anchor.

Textual proximity is not a semantic dependency. Inspect these surfaces to check
whether they bypass or contradict the derivation verdict, not to audit them.

## Pilot limits

Audit only the semantic truth of the six existing scenarios. Do not design new
general scenarios and do not extend the audit to header, move, or tag-view
features. Findings take stable `BDER-*` identifiers.

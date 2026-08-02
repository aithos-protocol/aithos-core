# Domain — `a-identity.feature`

## Contract

This feature covers Aithos identity genesis:

- determinism from a 32-byte owner master seed;
- separation of root, content, and key-exchange keys;
- independence of the succession secret;
- publication and verification of the DID document;
- transition to a new identity under succession authority.

The public audit is `docs/audits/features/a-identity.md`.

## Protocol invariants

1. The DID is bound to the root key and signed by that key.
2. Root, content, and succession use the expected Ed25519 codec.
3. Key exchange uses the expected X25519 codec.
4. Version, algorithm, and signature fragment are closed to known values.
5. Unknown wire members must not be dropped before verification.
6. An epoch transition must bind the previous document, declaration, and
   successor document actually presented.
7. Previous and successor identities must be distinct.
8. The succession secret must not be derivable from the owner master seed.
9. Cold-custody claims must be testable properties of the surfaces that make
   those claims.
10. A DID document is always root-signed; the succession key signs only an
    `EpochTransition`.
11. A new root creates a new DID. Same-DID succession-signed `did.json`
    replacement is invalid.
12. Provider must verify the complete previous/transition/successor triplet
    through Core before accepting an epoch successor, with no partial write on
    refusal.

The binding decision for invariants 10–12 is recorded in
`decisions/2026-07-29-aid-001-provider-epoch-transition.md`.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/a-identity.feature` |
| Steps | `rust/crates/aithos-bundle/tests/cucumber.rs` |
| Keys | `rust/crates/aithos-core/src/keys.rs` |
| DID and transition | `rust/crates/aithos-core/src/did.rs` |
| Derivation and wire | `rust/crates/aithos-core/src/{derive,wire}.rs` |
| Bundle consumer | `rust/crates/aithos-bundle/src/bundle.rs` |
| Gateway creation | `rust/crates/aithos-gateway/src/core_bridge.rs` |
| CLI custody | `rust/crates/aithos-cli/src/{main,custody}.rs` |
| Provider storage | `rust/crates/aithos-provider/src/artifacts.rs` |
| Core tests | `rust/crates/aithos-core/tests/{a1_genesis,a2_did}.rs` |
| Vectors | `vectors/{a1-genesis,a2-did}.json` |

After the AID-001/002/005 candidate correction, also inspect
`rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs`.

## Gate pyramid

Canonical feature tag: `@a-identity`.

Run the static check from the repository root:

```text
features/.agents/scripts/verify-feature-tags.sh
```

Run Cargo commands from the repository root with the workspace manifest.

### Auditor evidence — once per immutable revision

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @a-identity
```

The auditor runs no unfiltered Cucumber, broad regression, or workspace gate.
It may run one exact focused test only to resolve a semantic contradiction.

### Corrector regressions — after the final correction

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test a1_genesis --test a2_did
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test aid_identity_surfaces
```

### Corrector final integration — once before review handoff

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
```

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

The auditor's feature gate must report exactly the expanded `a-identity`
scenario count. The corrector's final Cucumber gate scans every feature not
tagged `@wip`; record its global scenario and step counts. Neither role runs a
gate once per scenario or review unit.

## Surfaces and neighboring domains to inspect

- Bundle: `did.json` parsing and cold verification;
- WASM/client: public mandate and DID verification;
- Gateway: identity creation and succession source;
- Provider: strict `did.json` deposit, epoch-transition acceptance, and
  successor distribution;
- `f-gamma.feature`: `rotate identity` facts, distinct from epoch transition
  but potentially sharing DID invariants.

## Pilot limits

Audit only the semantic truth of existing scenarios and the tests added to
close AID-001, AID-002, and AID-005. Do not design new general scenarios.
AID-003 and AID-004 remain open until explicitly assigned to a correction
round.

### Gate exit-code caveat (recorded 2026-08-02, per the b-derivation impact review)

Until `BDER-011` was fixed (`78c06ba`, merged `090d11a`, `VERIFIED` `c630753`
on 2026-07-30), the `aithos-bundle --test cucumber` runner called `filter_run`
under `harness = false` and exited 0 even when scenarios failed. Any
`EXIT=0` claim from that gate **predating the fix** is not evidence by itself;
only the printed per-scenario counters are. Claims after the fix carry normal
evidentiary weight. This aligns with
`features/.agents/b-derivation/DOMAIN.md` (gate semantics section).

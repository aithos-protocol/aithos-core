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

## Minimum gates

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
cargo test -p aithos-bundle --test aid_identity_surfaces
cargo test -p aithos-bundle --test cucumber
cargo test --workspace --no-fail-fast
cargo fmt --all -- --check
```

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

The Cucumber runner scans all features not tagged `@wip`. Confirm the exact
number of executed `a-identity` scenarios in its output, not only the global
exit code.

## Surfaces and neighboring domains to inspect

- Bundle: `did.json` parsing and cold verification;
- WASM/client: public mandate and DID verification;
- Gateway: identity creation and succession source;
- Provider: replacement and distribution of `did.json`;
- `f-gamma.feature`: `rotate identity` facts, distinct from epoch transition
  but potentially sharing DID invariants.

## Pilot limits

Audit only the semantic truth of existing scenarios and the tests added to
close AID-001, AID-002, and AID-005. Do not design new general scenarios.
AID-003 and AID-004 remain open until explicitly assigned to a correction
round.

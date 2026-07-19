# Changelog

All notable changes to published Aithos Core packages are documented here.

The project uses Semantic Versioning for package APIs. Wire-format stability is
tracked separately by the explicit `aithos-*-core` profiles carried in signed
artifacts.

## [Unreleased]

- Align the CLI and WASM surfaces with the completed Core + Bundle boundary.
- Add reproducible release automation after the initial registry publications.

## [0.1.0-alpha.1] - 2026-07-19

Initial Rust pre-release:

- Pure deterministic `aithos-core` trust engine.
- Identity, key derivation, mandates, delegation, attenuation, revocation, and
  connector catalog verification.
- Typed operation facts, receipts, sessions, obligations, and carrier verdicts.
- Gamma v1/v2 verification and deterministic semantic replay.
- `aithos-bundle` storage boundary with editions, mutations, vault operations,
  concurrency, merge, publication packages, cold verification, and CAS facts.
- Conformance vectors and BDD acceptance suites through the CB13 final gate.
- Business Source License 1.1 with an Aithos Provider restriction and automatic
  conversion to Apache-2.0.

[Unreleased]: https://github.com/aithos-protocol/aithos-core/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/aithos-protocol/aithos-core/releases/tag/v0.1.0-alpha.1

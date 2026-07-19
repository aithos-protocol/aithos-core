# Contributing to Aithos Core

Issues, protocol review, security analysis, and conformance feedback are
welcome.

## Code contributions

External code contributions are not accepted until Innoestate Holdings
publishes its Contributor License Agreement and signing workflow. This is
necessary because the project is source-available and also offered under
separate commercial terms.

Please open an issue before preparing a non-trivial change. A maintainer may
invite a contribution once the applicable contribution terms are in place.

## Development gates

Changes follow the repository's vector-first TDD/BDD process:

1. define or update the normative contract;
2. add an independently generated conformance vector when byte-level behavior
   changes;
3. add a failing unit or BDD test;
4. implement the smallest protocol change;
5. run formatting, clippy with warnings denied, native tests, and the WASM
   check.

Core logic must remain deterministic: no I/O, clock, entropy source, network,
or ambient state in `aithos-core`.

## Licensing

Feedback and issue discussions do not transfer copyright. Any future code
contribution will require explicit contribution terms. See `LICENSE` and
`COMMERCIAL-LICENSE.md`.

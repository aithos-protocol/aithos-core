# Aithos Core

Reference implementation of **Aithos Core** — a trust layer for AI agents:
identity, scoped mandates, recursive delegation, scoped revocation, and a
tamper-evident action log, enforceable **from files alone**. A server is never
a trust party.

> Status: pre-release. The current manifest issuance profile is
> `aithos-core: 1.0.0-draft.2`; historical draft profiles remain verifiable.
> Rust packages start at `0.1.0-alpha.1`.

## Repository layout

```
spec/          The normative specification (chapters 00–10). Source of truth.
vectors/       Conformance vectors (JSON). The language-neutral contract:
               any implementation, in any language, must reproduce them.
               Expected values are generated independently of the Rust code
               (e.g. Python blake3 + PyNaCl) whenever possible.
docs/          Working documents (execution plan, decisions).
rust/          Reference implementation (Cargo workspace, 6 crates).
docker/        Multi-stage build → static binary in a FROM scratch image.
```

## Workspace layering (normative for contributors)

| Crate | Role | May do I/O? |
|---|---|---|
| `aithos-core` | Pure protocol logic: keys, derivation, paths, headers, mandates, verifier, revocation, gamma, Merkle | **No.** No clock, no RNG, no network, no disk — time `T`, randomness and storage are injected by the caller |
| `aithos-bundle` | Bundle layout (§02.3), editions, `Store` trait (`mem`, `fs`; `s3` later) | Yes — the only crate that touches I/O |
| `aithos-cli` | The `aithos-core` binary (spec §09.1) | Yes (surface) |
| `aithos-wasm` | Thin WASM bindings, packaged as `@aithos/core` (local `wasm-pack` build; publishing is a separate, explicit decision) | No (surface, no logic) |
| `aithos-gateway` | Keyholding runner gateway that enforces mandates and records actions | Yes (service) |
| `aithos-provider` | Provider store, relay, witness, and service binaries | Yes (service) |

The purity rule is what makes every operation deterministic, replayable
against `vectors/`, and compilable to WASM unchanged: **one canonical core**
serves the CLI, the Docker image and the WASM surface — everything in this
repository, and nothing outside it.

## Build & test

```
cargo test  --workspace --manifest-path rust/Cargo.toml   # incl. conformance vectors
cargo check -p aithos-wasm --target wasm32-unknown-unknown --manifest-path rust/Cargo.toml
cargo bench --manifest-path rust/Cargo.toml -p aithos-bundle   # perf targets §09.3
docker build -f docker/Dockerfile -t aithos-core:0.1.0 .       # FROM scratch, ~4 MB
wasm-pack build rust/crates/aithos-wasm --target nodejs --release  # then npm pack (local only)
```

CI runs fmt, clippy (`-D warnings`), the native test suite, and the wasm32
check on every push.

## Where things stand

The Core + Bundle protocol boundary is implemented through the CB13 acceptance
gate, including deterministic authority verdicts, capability checks,
transactional mutations, semantic Gamma v2 replay, publication packages, cold
verification, CAS facts, and concurrency/replay closure.

The CLI and WASM surfaces predate part of that final boundary and remain
unpublished until their public APIs are aligned. Provider and gateway crates
are deployable service components and are intentionally not crates.io
packages.

Implementation follows [`docs/EXECUTION-PLAN.md`](docs/EXECUTION-PLAN.md):
vectors first, TDD/BDD, a living end-to-end scenario, and explicit gates.

## Published package plan

| Artifact | First version | Registry status |
|---|---:|---|
| `aithos-core` | `0.1.0-alpha.1` | publish first to crates.io |
| `aithos-bundle` | `0.1.0-alpha.1` | publish after `aithos-core` |
| `aithos-cli` | — | not published yet |
| `@aithos/core` (WASM) | — | not published yet |
| `aithos-gateway` | — | service component; registry publishing disabled |
| `aithos-provider` | — | service component; registry publishing disabled |

## License

The software is **source-available**, not Open Source. Core/client-side
components use the Business Source License 1.1 with broad production rights
except operating an Aithos Provider for third parties. Provider and gateway
components have a narrower internal-use grant. Separate commercial licenses
are available from Innoestate Holdings.

The specification, conformance vectors, and documentation are available under
CC BY 4.0 so independent implementations can interoperate.

See [`LICENSE`](LICENSE) and [`COMMERCIAL-LICENSE.md`](COMMERCIAL-LICENSE.md).

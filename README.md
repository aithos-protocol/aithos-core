# aithos-core

Reference implementation of **Aithos Core** — a trust layer for AI agents:
identity, scoped mandates, recursive delegation, scoped revocation, and a
tamper-evident action log, enforceable **from files alone**. A server is never
a trust party.

> Status: DRAFT. Wire version `aithos-core: 1.0.0-draft.1`.

## Repository layout

```
spec/          The normative specification (chapters 00–10). Source of truth.
vectors/       Conformance vectors (JSON). The language-neutral contract:
               any implementation, in any language, must reproduce them.
               Expected values are generated independently of the Rust code
               (e.g. Python blake3 + PyNaCl) whenever possible.
docs/          Working documents (execution plan, decisions).
rust/          Reference implementation (cargo workspace, 4 crates).
docker/        Multi-stage build → static binary in a FROM scratch image.
```

## Workspace layering (normative for contributors)

| Crate | Role | May do I/O? |
|---|---|---|
| `aithos-core` | Pure protocol logic: keys, derivation, paths, headers, mandates, verifier, revocation, gamma, Merkle | **No.** No clock, no RNG, no network, no disk — time `T`, randomness and storage are injected by the caller |
| `aithos-bundle` | Bundle layout (§02.3), editions, `Store` trait (`mem`, `fs`; `s3` later) | Yes — the only crate that touches I/O |
| `aithos-cli` | The `aithos-core` binary (spec §09.1) | Yes (surface) |
| `aithos-wasm` | Thin WASM bindings, packaged as `@aithos/core` (local `wasm-pack` build; publishing is a separate, explicit decision) | No (surface, no logic) |

The purity rule is what makes every operation deterministic, replayable
against `vectors/`, and compilable to WASM unchanged: **one canonical core**
serves the CLI, the Docker image and the WASM surface — everything in this
repository, and nothing outside it.

## Build & test

```
cargo test  --workspace --manifest-path rust/Cargo.toml   # incl. conformance vectors
cargo check -p aithos-wasm --target wasm32-unknown-unknown --manifest-path rust/Cargo.toml
docker build -f docker/Dockerfile -t aithos-core .
```

CI runs fmt, clippy (`-D warnings`), the native test suite, and the wasm32
check on every push.

## Where things stand

Implementation proceeds strictly by the step plan in
[`docs/EXECUTION-PLAN.md`](docs/EXECUTION-PLAN.md): vectors first, TDD, a
living end-to-end scenario that grows with each step, and a manual CLI
checkpoint per step. No step starts before the previous one is validated.

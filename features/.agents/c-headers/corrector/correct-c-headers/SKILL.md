---
name: correct-c-headers
description: Correct only the Headers findings explicitly assigned by features/.agents/c-headers/STATE.md. Use this skill after a c-headers audit to change header line sealing, the owner-line rule, grant append, rotation, or the up-link wrap and their tests without broadening scope or self-verifying the correction.
---

# Correct `c-headers.feature`

1. Read `../../../shared/correct-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` and `../../STATE.md` completely.
3. Read the public audit and the auditor's latest conclusion.
4. Address only the findings assigned by state.

## Domain rules

- Put header invariants in `aithos-core` (`src/header.rs`, `src/seal.rs`), not
  in a step definition. A guard that lives only in `cucumber.rs` protects no
  reader.
- Never change an AAD purpose label, the KEK `info` string, or the wrap
  derivation context without a normative decision: `header-line`, `tagwrap`,
  `aithos-core/v1/hdr-kek`, and `aithos-core/v1/wrap`
  (`seal.rs:15-19`) are wire-visible. Changing one silently invalidates every
  existing header, wrap, and stored ciphertext, and breaks
  `vectors/c1-header-seal.json`, `vectors/g2-rotation.json`, and
  `vectors/g3-move.json` at once.
- Keep randomness injected. Ephemerals and nonces are caller-supplied by design
  (`seal.rs:1-5`, `spec/03-headers.md:136-139`); generating them inside a seal
  function would destroy the byte-exact conformance vectors.
- Preserve the byte-exact positive vectors unless a normative decision says
  otherwise, and extend them rather than replace them. Any change to
  `vectors/c1-header-seal.json` requires re-pinning its `sha256` in
  `vectors/ownership.json`, enforced by
  `rust/crates/aithos-bundle/tests/vectors_ownership.rs`.
- Keep every header path fail-closed. I3 is checked at build, at rotate, and at
  parse (`check_owner_line`, `Header::validate`); a correction must not leave a
  reader that opens a header it never validated.
- Prove a binding by an AAD mismatch rejecting, not by a key inequality; prove
  the O(1) grant by byte-identity of the untouched lines, not by a successful
  open.
- Do not fix revocation, move, Merkle, vault, or bundle-atomicity features
  unless the assigned finding requires it; report the impact instead. The
  header API is consumed by `grants.rs`, `revoke.rs`, `structure.rs`,
  `vault.rs`, `session.rs`, `log.rs`, `bundle.rs`, and the two CLI commands —
  a signature change there is a cross-feature change.
- Add only the scenarios or tests needed to prove the assigned findings.

## Gates

- Focused RED/GREEN while the implementation changes:
  `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal`.
- Canonical feature gate once after the final change:
  `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers`.
- Relevant regressions and the final global gates: exactly the commands in
  `../../DOMAIN.md`, section "Gate pyramid". Record the printed counters, not
  only the exit code.

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, RED, GREEN, and changed files.
- Move findings at most to `IMPLEMENTED`.
- Request review from `audit-c-headers`.
- Set `STATE.md` to `REVIEW_REQUESTED`.

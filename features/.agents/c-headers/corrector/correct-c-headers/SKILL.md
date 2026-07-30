---
name: correct-c-headers
description: Correct only the Headers findings explicitly assigned by features/.agents/c-headers/STATE.md. Use this skill after a c-headers audit to change header sealing, line handling, rotation, or up-link wraps and their tests without broadening scope or self-verifying the correction.
---

# Correct `c-headers.feature`

1. Read `../../../shared/correct-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` and `../../STATE.md` completely.
3. Read the public audit and the auditor's latest conclusion.
4. Address only the findings assigned by state.

## Domain rules

- Put header invariants in `aithos-core`, not in a step definition. A step that
  re-implements sealing proves the step, not the product.
- Never change an AAD purpose string, the KEK `info` layout, or the
  `key_version` encoding without a normative decision: any of them silently
  invalidates every existing header line.
- Preserve the byte-exact C1/C2 vectors in `vectors/c1-header-seal.json`,
  extending them rather than replacing them.
- Keep the crypto layer's randomness injected. Never introduce an internal RNG
  into a seal path; ephemerals and nonces stay inputs.
- Keep grant `O(1)`: an append must leave every existing line byte-identical
  and must not need another line's ephemeral.
- Keep I3 fail-closed at build, rotate, and parse time alike.
- Prove a rejection by its stated cause. When a finding is that `is_err()` is
  too weak, assert the specific error or the specific varied AAD component, not
  merely that something failed.
- Prove the absence of partial effects for every rejected build or rotation.
- Do not fix revocation, move, tag-view, or Merkle features unless the assigned
  finding requires it; report the impact instead.

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, RED, GREEN, and changed files.
- Read the printed Cucumber scenario/step counts rather than the exit code
  while `BDER-011` is open, and say so in the report.
- Move findings at most to `IMPLEMENTED`.
- Request review from `audit-c-headers`.
- Set `STATE.md` to `REVIEW_REQUESTED`.

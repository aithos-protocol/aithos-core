---
name: correct-b-derivation
description: Correct only the Derivation findings explicitly assigned by features/.agents/b-derivation/STATE.md. Use this skill after a b-derivation audit to change derivation labels, path walking, or their tests without broadening scope or self-verifying the correction.
---

# Correct `b-derivation.feature`

1. Read `../../../shared/correct-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` and `../../STATE.md` completely.
3. Read the public audit and the auditor's latest conclusion.
4. Address only the findings assigned by state.

## Domain rules

- Put derivation invariants in `aithos-core`, not in a step definition.
- Never change an existing context string or label format without a normative
  decision: a label change silently re-keys every existing bundle.
- Preserve the byte-exact positive vectors in `vectors/b2-derivation.json`
  unless a normative decision says otherwise, and extend them rather than
  replace them.
- Prove containment by the absence of a reachable derivation, not by a key
  inequality.
- Do not fix rename, move, header, or tag-view features unless the assigned
  finding requires it; report the impact instead.

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, RED, GREEN, and changed files.
- Move findings at most to `IMPLEMENTED`.
- Request review from `audit-b-derivation`.
- Set `STATE.md` to `REVIEW_REQUESTED`.

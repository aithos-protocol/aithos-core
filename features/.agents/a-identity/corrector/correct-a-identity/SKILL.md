---
name: correct-a-identity
description: Correct only the Identity findings explicitly assigned by features/.agents/a-identity/STATE.md. Use this skill after an a-identity audit to change DID, succession, or epoch-transition paths and their tests without broadening scope or self-verifying the correction.
---

# Correct `a-identity.feature`

1. Read `../../../shared/correct-gherkin-feature/SKILL.md` completely.
2. Read `../../DOMAIN.md` and `../../STATE.md` completely.
3. Read the public audit and the auditor's latest conclusion.
4. Address only the findings assigned by state.

## Domain rules

- Put shared DID invariants in `aithos-core`, not in a step definition.
- Close wire parsing before reconstructing verified JCS.
- Bind the previous document, transition declaration, and successor
  explicitly.
- Check the public surfaces that consume the Core verdict.
- Preserve byte-exact positive vectors unless a normative decision says
  otherwise.
- Do not address AID-003 or AID-004 without explicit assignment.

## Handoff

- Write the conclusion under `../runs/`.
- Record baseline, candidate commit, RED, GREEN, and changed files.
- Move findings at most to `IMPLEMENTED`.
- Request review from `audit-a-identity`.
- Set `STATE.md` to `REVIEW_REQUESTED`.

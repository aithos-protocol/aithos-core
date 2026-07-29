# État du domaine `a-identity`

| Champ | Valeur |
|---|---|
| Statut | `REVIEW_REQUESTED` |
| Mode attendu | `review` |
| Round | 1 |
| Baseline d'audit | `be2d098eeb79107c861462a6433df9ef45871265` |
| Commit de correction | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Branche du correcteur | `fix/aid-001-002-005-identity-fail-closed` |
| Findings candidats | `AID-001`, `AID-002`, `AID-005` |
| Findings hors correction | `AID-003`, `AID-004` |
| Prochain rôle | `audit-a-identity` |
| Conclusion attendue | `auditor/runs/<date>-audit-review-01.md` |

## Entrées

- audit public : `docs/audits/features/a-identity.md` ;
- audit initial reconstruit :
  `auditor/runs/2026-07-29-audit-initial-reconstructed.md` ;
- correction reconstruite :
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.

## Instruction courante

Reviewer le commit de correction contre la baseline. Ne pas implémenter de
changement. Reproduire les preuves, accepter ou refuser chaque finding
séparément, puis mettre à jour cet état.

Le commit `56436f3` est une entrée immuable. Toute modification ultérieure doit
produire un nouveau commit et un nouveau round.

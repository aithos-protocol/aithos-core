# État du domaine `a-identity`

| Champ | Valeur |
|---|---|
| Statut | `DECISION_REQUIRED` |
| Mode attendu | `décision protocolaire manuelle` |
| Round | 2 |
| Baseline d'audit initiale | `be2d098eeb79107c861462a6433df9ef45871265` |
| Commit revu | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Baseline de correction round 2 | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Branche revue | `fix/aid-001-002-005-identity-fail-closed` |
| Findings vérifiés | `AID-002`, `AID-005` (périmètre du pilote) |
| Finding en décision | `AID-001` |
| Findings hors correction | `AID-003`, `AID-004` |
| Prérequis bloquant | sémantique du remplacement Provider `did.json` |
| Prochain rôle | propriétaire du protocole |
| Conclusion attendue | décision Provider, puis `corrector/runs/<date>-correction-02.md` si correction |

## Entrées

- audit public : `docs/audits/features/a-identity.md` ;
- audit initial reconstruit :
  `auditor/runs/2026-07-29-audit-initial-reconstructed.md` ;
- correction reconstruite :
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.
- review indépendante round 1 :
  `auditor/runs/2026-07-29-audit-review-01.md`.

## Instruction courante

Obtenir d'abord une décision protocolaire explicite pour AID-001 :

- choisir si le remplacement Provider `did.json` reste une succession même-DID
  spécifique ou adopte la transition d'époque du §10.4 ;
- ne lancer une correction round 2 qu'après cette décision, et uniquement pour
  mettre en œuvre la sémantique retenue ;
- conserver AID-002 et AID-005 inchangés, désormais `VÉRIFIÉS` dans le
  périmètre du pilote ;
- ne pas corriger AID-003 ou AID-004 dans ce round.

Le commit `56436f3` reste une entrée immuable et devient la baseline du round 2.
Toute correction doit produire un nouveau commit.

# État du domaine `a-identity`

| Champ | Valeur |
|---|---|
| Statut | `CORRECTION_REQUESTED` |
| Mode attendu | `correction` |
| Round | 2 |
| Baseline d'audit initiale | `be2d098eeb79107c861462a6433df9ef45871265` |
| Commit revu | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Baseline de correction round 2 | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Branche revue | `fix/aid-001-002-005-identity-fail-closed` |
| Findings vérifiés | `AID-002` |
| Findings redemandés | `AID-001`, `AID-005` |
| Findings hors correction | `AID-003`, `AID-004` |
| Prochain rôle | `correct-a-identity` |
| Conclusion attendue | `corrector/runs/<date>-correction-02.md` |

## Entrées

- audit public : `docs/audits/features/a-identity.md` ;
- audit initial reconstruit :
  `auditor/runs/2026-07-29-audit-initial-reconstructed.md` ;
- correction reconstruite :
  `corrector/runs/2026-07-29-correction-01-reconstructed.md`.
- review indépendante round 1 :
  `auditor/runs/2026-07-29-audit-review-01.md`.

## Instruction courante

Corriger uniquement les refus de review AID-001 et AID-005 :

- aligner ou faire arbitrer explicitement le remplacement Provider
  `did.json` afin qu'il ne puisse pas persister un document que le verdict
  Core strict refuse ;
- livrer les preuves AID-005 encore requises ou obtenir un arbitrage explicite
  de leur périmètre ;
- conserver AID-002 inchangé, désormais `VÉRIFIÉ` ;
- ne pas corriger AID-003 ou AID-004 dans ce round.

Le commit `56436f3` reste une entrée immuable et devient la baseline du round 2.
Toute correction doit produire un nouveau commit.

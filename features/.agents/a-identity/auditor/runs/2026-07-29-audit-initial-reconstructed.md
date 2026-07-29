# Conclusion reconstruite — audit initial de `a-identity.feature`

| Champ | Valeur |
|---|---|
| Type | `RECONSTRUIT` |
| Rôle source | auditeur sémantique |
| Date de l'audit | 2026-07-29 |
| Révision observée | `2fee855` avec worktree signalé sale |
| Commit documentaire produit | `be2d098` |
| Audit public | `docs/audits/features/a-identity.md` |
| Résultat | `CORRECTION_REQUESTED` |

## Provenance

Cette conclusion a été reconstruite après l'audit à partir du commit
`be2d098`, des commentaires ajoutés à la feature, de l'audit public et des
résultats consignés. Elle n'a pas été générée nativement par le skill
`audit-a-identity`.

## Conclusion de l'audit

Les neuf scénarios étaient sélectionnés et exécutaient du code Rust réel.
Aucun step n'était vide, `@wip` ou remplacé par un verdict global `OnceLock`.

Le vert n'établissait toutefois pas tout le contrat :

- 6 scénarios `PROUVÉ` ;
- 2 scénarios `PARTIEL` ;
- 1 scénario `FAUX POSITIF`.

## Findings ouverts

| Finding | Verdict | Correction demandée |
|---|---|---|
| `AID-001` | `PARTIEL` | Fermer le schéma DID, valider version, signature et codecs des quatre clés |
| `AID-002` | `FAUX POSITIF` | Vérifier réellement précédent + transition + document successeur |
| `AID-003` | `PARTIEL` | Supprimer les dérivations de succession depuis le master owner |
| `AID-004` | `DÉCISION REQUISE` | Définir et appliquer une custody réellement froide |
| `AID-005` | preuve insuffisante | Ajouter les tests nécessaires pour démontrer AID-001 et AID-002 |

AID-005 est inclus uniquement comme preuve corrective des scénarios existants,
pas comme campagne générale de recherche de tests manquants.

## Preuves consignées

```text
Runner ciblé :
1 feature
6 rules
9 scenarios (9 passed)
30 steps (30 passed)

cargo test -p aithos-core --test a1_genesis --test a2_did
a1_genesis: 4 passed
a2_did:     3 passed
```

Des sondes négatives temporaires ont rapporté :

```text
signed malformed non-root keys accepted: true
signed wrong version/alg/fragment accepted: true
unknown unsigned wire field ignored and accepted: true
transition to malformed DID accepted: true
transition to same DID accepted: true
```

Les sondes temporaires n'ont pas été conservées dans le dépôt ; les tests RED
durables faisaient partie de la correction demandée.

## Artefacts produits

- commentaires et tags d'audit dans `features/a-identity.feature` ;
- audit public `docs/audits/features/a-identity.md` ;
- identifiants stables AID-001 à AID-005 ;
- critères de clôture et tests RED attendus.

## Handoff demandé

Lancer `correct-a-identity` sur AID-001, AID-002 et les preuves AID-005.
Ne pas traiter AID-003/AID-004 sans décision d'architecture. Demander ensuite
une review indépendante à `audit-a-identity`.

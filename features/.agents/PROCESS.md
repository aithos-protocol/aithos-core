# Processus manuel — vérité sémantique des features Gherkin

## Objectif

Déterminer si chaque scénario existant qui passe :

1. est réellement sélectionné et exécuté ;
2. transmet ses paramètres aux steps attendus ;
3. atteint un chemin de production concret ;
4. vérifie exactement le résultat annoncé ;
5. reste cohérent avec le protocole Aithos et ses surfaces réelles.

Un runner vert n'est pas une preuve suffisante.

## Périmètre courant

Inclure :

- step vide, générique ou proxy ;
- paramètre Gherkin ignoré ;
- assertion plus faible que le `Then` ;
- résultat global réutilisé pour plusieurs cas ;
- implémentation réelle mais contraire au scénario ou au protocole ;
- surface de production qui contourne le verdict exercé ;
- test RED indispensable pour rendre un scénario existant honnête.

Exclure :

- recherche générale de fonctionnalités non décrites par les scénarios
  existants ;
- enrichissement produit ou protocolaire sans lien avec un faux vert ;
- refactor opportuniste ;
- correction d'un autre domaine non requise par le finding courant.

## Artefacts

| Artefact | Rôle |
|---|---|
| `features/<feature>.feature` | Contrat et marqueurs d'audit concis |
| `docs/audits/features/<feature>.md` | Audit technique public et findings stables |
| `.agents/<feature>/DOMAIN.md` | Connaissance durable du périmètre |
| `.agents/<feature>/STATE.md` | Étape courante et SHA à examiner |
| `.agents/<feature>/<rôle>/runs/*.md` | Conclusions datées et handoffs |

L'audit public est la source de vérité technique. Les runs expliquent qui a
fait quoi, sur quelle révision et quelle action doit suivre.

## Cycle manuel

```text
AUDIT_INITIAL
  → CORRECTION_REQUESTED
  → REVIEW_REQUESTED
      → CORRECTION_REQUESTED
      → ou DECISION_REQUIRED
           → CORRECTION_REQUESTED
           → ou REVIEW_ACCEPTED
      → ou REVIEW_ACCEPTED
           → IMPACT_REVIEW_REQUESTED
           → COMPLETE
```

### Audit initial

L'auditeur :

1. fige le SHA et l'état du worktree ;
2. compte les scénarios sélectionnés et exécutés ;
3. trace chaque step vers ses appels et assertions ;
4. classe chaque scénario ;
5. ajoute des commentaires seulement aux scénarios problématiques ;
6. écrit ou met à jour l'audit public ;
7. produit une conclusion datée ;
8. positionne `STATE.md` sur `CORRECTION_REQUESTED`.

### Correction

Le correcteur :

1. lit les findings explicitement demandés ;
2. démontre le défaut par un test RED lorsque c'est possible ;
3. implémente le changement minimal ;
4. rejoue les gates ciblés et les régressions pertinentes ;
5. documente le diff et les résultats ;
6. marque au plus les findings `IMPLÉMENTÉ` ;
7. demande une review indépendante ;
8. positionne `STATE.md` sur `REVIEW_REQUESTED`.

### Review

L'auditeur :

1. examine le diff exact `baseline..correction` ;
2. ne tient pas la conclusion du correcteur pour preuve ;
3. vérifie chaque critère de clôture ;
4. rejoue les tests dans un contexte propre ;
5. contrôle les surfaces et les effets partiels ;
6. refuse ou accepte chaque finding séparément ;
7. marque `VÉRIFIÉ` uniquement après preuve indépendante ;
8. consigne les fichiers, symboles, formats et surfaces modifiés.

Une review refusée renvoie au correcteur. Après trois refus pour le même
finding, arrêter l'automatisme et demander une décision humaine.

### Décision requise

Utiliser `DECISION_REQUIRED` lorsqu'un finding ne peut pas être fermé sans
choisir entre plusieurs sémantiques de protocole, de sécurité ou de produit.

Dans cet état :

1. l'auditeur documente les comportements en conflit et leurs preuves ;
2. aucun correcteur ne choisit implicitement la sémantique ;
3. `STATE.md` désigne le propriétaire de la décision comme prochain rôle ;
4. la décision est enregistrée avant tout nouveau round ;
5. l'état passe ensuite à `CORRECTION_REQUESTED` ou `REVIEW_ACCEPTED`.

### Review des impacts

Après acceptation seulement, le reviewer global :

1. lit l'audit, les runs et le diff accepté ;
2. recherche les autres features qui partagent steps, helpers, symboles,
   formats, vecteurs ou sections de spec ;
3. classe chaque impact en `AUCUN`, `CIBLÉ` ou `AUDIT COMPLET` ;
4. produit un rapport global ;
5. ne modifie ni ne relance aucune feature.

La décision de relancer un audit reste manuelle.

## Statuts de preuve

| Statut | Sens |
|---|---|
| `PROUVÉ` | Le scénario exerce et prouve exactement son contrat |
| `PARTIEL` | Une frontière ou un invariant annoncé n'est pas exercé |
| `FAUX POSITIF` | Le scénario passe sans vérifier le résultat annoncé |
| `PROXY` | Le scénario consomme un verdict partagé sans jouer son cas |
| `IMPLÉMENTÉ` | Un correctif candidat existe, review requise |
| `VÉRIFIÉ` | L'auditeur a reproduit et accepté le correctif |
| `DECISION_REQUIRED` | Un propriétaire humain doit trancher avant correction |

## Conclusion obligatoire

Chaque run indique :

- type de run et rôle ;
- date ;
- SHA observé, baseline et éventuel SHA corrigé ;
- état du worktree ;
- périmètre ;
- commandes et résultats exacts ;
- findings traités et non traités ;
- fichiers et symboles affectés ;
- limites de la conclusion ;
- prochaine action et skill attendu.

Une conclusion reconstruite a posteriori porte explicitement le statut
`RECONSTRUIT`. Elle distingue les faits observables des résultats seulement
rapportés par un autre agent.

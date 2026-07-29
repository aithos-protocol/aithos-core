# Audits d'implémentation des features Gherkin

Ce répertoire contient une note vivante par fichier `features/*.feature`.
L'objectif est de distinguer précisément :

1. un scénario réellement exécuté ;
2. un scénario qui appelle bien du code de production ;
3. un scénario qui prouve effectivement tout ce que son texte affirme ;
4. une capacité conforme sur les surfaces qui l'utilisent réellement.

Un runner vert ne suffit pas, à lui seul, à satisfaire les quatre niveaux.

## Convention

- Un fichier stable par feature : `a-identity.md`, `b-derivation.md`, etc.
- Une date d'audit et la révision Git observée sont toujours indiquées.
- L'audit porte sur l'état disque observé. Un worktree sale est signalé ; il
  n'est jamais présenté comme une baseline reproductible propre.
- Chaque écart reçoit un identifiant stable dérivé de la feature :
  `AID-001`, `BDER-001`, `CHDR-001`, etc.
- Les constats ne sont pas supprimés après correction. Leur statut passe de
  `OUVERT` à `IMPLÉMENTÉ`, puis à `VÉRIFIÉ`, avec la preuve de clôture.

## Statuts de couverture

| Statut | Signification |
|---|---|
| `PROUVÉ` | Les entrées du scénario pilotent une API de production et ses assertions vérifient exactement le résultat annoncé. |
| `PARTIEL` | Une partie du contrat est réelle, mais une frontière ou un invariant annoncé n'est pas exercé. |
| `FAUX POSITIF` | Le scénario passe sans vérifier le résultat qu'il affirme. |
| `NON COUVERT` | Aucun scénario sélectionné ne porte l'exigence. |
| `PROXY` | Le scénario réutilise un verdict global sans exécuter son cas propre. |

## Structure obligatoire d'une note

Chaque note contient :

1. **Métadonnées** — feature, date, révision, état du worktree et périmètre.
2. **Verdict** — résultat synthétique et compte exact des scénarios/steps.
3. **Preuves rejouées** — commandes et résultats observés.
4. **Matrice scénario par scénario** — statut et chemin de production.
5. **Écarts ordonnés** — impact, preuve et comportement attendu.
6. **Plan d'implémentation** — changement minimal, tests RED attendus et
   critères de clôture.
7. **Décisions à trancher** — choix de protocole ou de produit qui ne doivent
   pas être décidés silencieusement dans le code.
8. **Définition de terminé** — gates communs nécessaires pour fermer la note.

## Règles de preuve

Un scénario n'est classé `PROUVÉ` que si :

- aucun `@wip` ou filtre ne l'exclut ;
- le runner exécute un nombre non nul et attendu de scénarios ;
- le `When` appelle l'implémentation de production ou une façade publique
  réelle ;
- les paramètres de la ligne Gherkin atteignent cet appel ;
- le `Then` vérifie le résultat propre au scénario, pas un succès global ;
- un refus vérifie l'absence d'effet partiel quand une mutation est en jeu ;
- les frontières annoncées — parsing wire, store frais, reopen, réseau,
  restart — sont réellement franchies ;
- les cas cryptographiques structurants sont renforcés par des vecteurs
  indépendants lorsque la conformité byte-exacte est requise.

Les tests unitaires, vecteurs et Gherkins sont complémentaires. Aucun ne doit
être présenté comme le substitut silencieux d'un autre.

## Index

| Feature | Note | Verdict courant |
|---|---|---|
| `a-identity.feature` | [`a-identity.md`](a-identity.md) | AID-001/002/005 implémentés ; AID-003/004 ouverts |

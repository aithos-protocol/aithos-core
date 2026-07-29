---
name: review-gherkin-impacts
description: Analyser les effets de bord possibles d'un correctif Gherkin déjà accepté par son auditeur spécialisé. Utiliser ce skill après une review VÉRIFIÉE pour croiser le diff avec les autres features, steps, helpers, API, formats, vecteurs et sections de spec, puis produire un rapport manuel sans modifier ni relancer les autres audits.
---

# Reviewer les impacts entre features

## Conditions d'entrée

1. Lire complètement `../../../PROCESS.md`.
2. Exiger une conclusion d'audit portant `REVIEW_ACCEPTED`.
3. Exiger une baseline et un commit corrigé immuables.
4. Arrêter si la correction n'est qu'`IMPLÉMENTÉE`.

## Analyse

1. Examiner le diff accepté.
2. Extraire les fichiers, fonctions, types, steps, formats et vecteurs changés.
3. Rechercher leurs usages dans tous les runners et fichiers `.feature`.
4. Croiser les sections de spec citées par les autres audits.
5. Distinguer un simple voisinage textuel d'une dépendance sémantique.
6. Classer chaque feature :
   - `AUCUN` : aucune dépendance crédible ;
   - `CIBLÉ` : quelques scénarios précis à revoir ;
   - `AUDIT COMPLET` : helper, API, format ou invariant partagé.

## Sortie

Écrire un rapport daté sous `../runs/` avec :

- baseline et commit accepté ;
- audit et review sources ;
- éléments modifiés ;
- recherches effectuées ;
- features potentiellement touchées et preuves ;
- recommandation manuelle.

Ne modifier aucun code, audit ou fichier feature. Ne lancer aucun autre agent.

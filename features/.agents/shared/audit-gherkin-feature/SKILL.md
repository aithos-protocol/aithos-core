---
name: audit-gherkin-feature
description: Auditer la vérité sémantique d'une feature Gherkin existante ou reviewer un correctif issu de cet audit. Utiliser ce skill pour vérifier que des scénarios verts sont réellement sélectionnés, transmettent leurs paramètres, appellent du code de production et prouvent exactement leur contrat, ou pour décider si des findings implémentés peuvent passer à VÉRIFIÉ.
---

# Auditer une feature Gherkin

## Préparation

1. Lire complètement `../../PROCESS.md`.
2. Lire le domaine et l'état de la feature spécialisée.
3. Relever le SHA, la branche, le worktree et le mode demandé.
4. Refuser de confondre un état sale avec une baseline reproductible.
5. Ne pas rechercher les scénarios entièrement manquants.

## Audit initial

1. Inventorier les Rules, Scenarios, Outlines, Examples et tags.
2. Identifier le runner et confirmer qu'il sélectionne la feature.
3. Compter les scénarios et steps réellement exécutés.
4. Résoudre chaque phrase vers sa définition de step.
5. Vérifier que les paramètres atteignent l'appel de production.
6. Suivre le retour jusqu'à l'assertion propre au scénario.
7. Contrôler les frontières annoncées : parsing wire, store, reopen, restart,
   réseau, signature, mutation et absence d'effet partiel.
8. Comparer le résultat au texte Gherkin et aux sections de spec concernées.
9. Renforcer les cas cryptographiques byte-exacts par les vecteurs pertinents.
10. Classer chaque scénario `PROUVÉ`, `PARTIEL`, `FAUX POSITIF` ou `PROXY`.

Ne pas classer `PROUVÉ` parce qu'une fonction existe, qu'un vecteur porte un
nom proche ou que le runner global est vert.

## Review d'un correctif

1. Lire la baseline et le commit candidat depuis l'état de la feature.
2. Examiner le diff exact sans modifier le code de production.
3. Relier chaque changement à un finding et à son critère de clôture.
4. Vérifier que les nouveaux tests auraient détecté l'ancien comportement.
5. Rejouer les gates annoncés dans un contexte propre.
6. Vérifier les chemins publics et les refus sans effet partiel.
7. Rechercher les contournements parallèles dans les surfaces du domaine.
8. Accepter ou refuser chaque finding séparément.
9. Ne jamais promouvoir un finding non traité ou hors périmètre.
10. Utiliser `DECISION_REQUIRED`, plutôt qu'une demande de correction, si le
    changement imposerait de choisir une sémantique de protocole ou de produit.

Traiter la conclusion du correcteur comme un handoff à vérifier, pas comme une
preuve.

## Sorties

- Mettre à jour l'audit public.
- Garder les identifiants de finding stables.
- Ajouter ou mettre à jour les marqueurs Gherkin nécessaires.
- Écrire une conclusion datée dans le dossier `runs` du rôle spécialisé.
- Mettre à jour l'état avec la prochaine action.
- En cas d'acceptation, énumérer les fichiers, symboles, formats, sections de
  spec et surfaces susceptibles d'avoir un effet transverse.
- En cas de décision requise, exposer les comportements concurrents, leurs
  preuves et le propriétaire attendu sans choisir à sa place.

Ne pas implémenter le correctif pendant l'audit ou la review.

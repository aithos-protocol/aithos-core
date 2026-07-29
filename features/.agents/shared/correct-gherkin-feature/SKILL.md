---
name: correct-gherkin-feature
description: Implémenter les corrections demandées par un audit sémantique Gherkin déjà documenté. Utiliser ce skill lorsqu'un STATE de feature demande une correction de findings précis pour rendre des scénarios existants cohérents avec leur contrat et le protocole, avec tests RED, changement minimal, preuves GREEN et handoff obligatoire vers l'auditeur.
---

# Corriger une feature Gherkin auditée

## Préparation

1. Lire complètement `../../PROCESS.md`.
2. Lire le domaine, l'état, l'audit public et le dernier run d'audit.
3. Vérifier que l'état demande explicitement une correction.
4. Figer la baseline avant la première modification.
5. Limiter le travail aux findings assignés.

## Exécution

1. Reproduire chaque défaut sur le chemin de production indiqué.
2. Écrire un test RED qui isole la sémantique défaillante.
3. Vérifier que le test échoue pour la bonne raison.
4. Implémenter le correctif minimal dans la couche qui porte l'invariant.
5. Éviter les vérificateurs parallèles et les rustines propres au test.
6. Rejouer le test ciblé, la feature et les régressions pertinentes.
7. Vérifier l'absence d'effet partiel pour toute mutation refusée.
8. Formater et contrôler le diff.

Ajouter les scénarios ou tests nécessaires à la preuve des findings demandés.
Ne pas ouvrir un chantier de couverture générale ni traiter un finding non
assigné.

## Documentation et handoff

- Documenter les tests RED puis GREEN.
- Énumérer les fichiers, symboles, formats et surfaces modifiés.
- Signaler toute divergence entre l'audit et la réalité rencontrée.
- Mettre à jour un finding au plus vers `IMPLÉMENTÉ`.
- Ne jamais utiliser le statut `VÉRIFIÉ`.
- Écrire une conclusion datée dans le dossier `runs` du correcteur.
- Demander explicitement une review au skill auditeur spécialisé.
- Positionner l'état sur `REVIEW_REQUESTED` avec la baseline et le commit
  candidat immuables.

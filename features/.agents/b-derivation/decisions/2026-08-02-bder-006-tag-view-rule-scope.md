# Décision — BDER-006 : périmètre de la `Rule` « Tag views anchor at folders »

| Champ | Valeur |
|---|---|
| Finding | `BDER-006` (P2, `DECISION_REQUIRED`) |
| Date | 2026-08-02 |
| Décideur | Propriétaire du protocole (Mathieu) |
| Statut résultant | `CORRECTION_REQUESTED` (ronde 2) |

## Décision

**Option A, avec extension obligatoire de `d-bundle` dans le même mouvement.**

1. La `Rule` de `b-derivation.feature` reste une `Rule` de **dérivation pure**.
   Son titre est reformulé pour ne plus promettre la sémantique d'ancrage du
   §02.9 (`spec/02-content-tree.md`).
2. En contrepartie, et pour ne pas laisser le trou identifié par la revue 01
   (`d-bundle.feature` ne contient aujourd'hui aucun scénario de vue de tag ni
   de `wrap`), le suivi ciblé `d-bundle` ouvert par la revue d'impact du
   2026-07-29 est **élargi** : le futur cycle `d-bundle` doit ajouter les
   scénarios tag-view/`wrap` prouvant la moitié comportementale du §02.9
   (ancre vide par défaut, pontage par `wrap`, vue locale limitée à son
   sous-arbre, vue racine couvrant la zone).

## Motifs

- Cohérence des frontières de domaine : la sémantique d'ancrage vit dans
  `aithos-bundle` (`grants.rs:324-343`, `:884-893`, `:965-973` ; `state.rs:156` ;
  `structure.rs:286`), pas dans `aithos-core::derive`. L'option B aurait fait
  traverser deux crates aux steps de `b-derivation`, à rebours du modèle
  d'audit par domaine et du point de contact déjà surveillé avec `d-bundle`.
- Le trou de couverture se bouche au bon endroit : le suivi `TARGETED`
  `d-bundle` existe déjà ; l'extension s'y rattache.
- Le témoin comportemental de l'ancrage attendu par BDER-007 sera apporté par
  cette extension, dans la bonne feature.

## Conséquences exécutables

- Ronde 2 `b-derivation` (correcteur) : reformuler le titre de la `Rule`,
  retirer `@audit-partial @bder-006` après revue acceptée.
- Cycle `d-bundle` : ajouter les scénarios tag-view/`wrap` (obligation liée —
  sans elle, cette décision dégénère en « A seule » et le §02.9 reste sans
  preuve ; ce n'est pas ce qui est décidé ici).

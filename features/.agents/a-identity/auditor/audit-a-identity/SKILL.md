---
name: audit-a-identity
description: Auditer ou reviewer exclusivement features/a-identity.feature avec la connaissance de ses invariants de genèse, DID, succession et transition d'époque. Utiliser ce skill lorsque features/.agents/a-identity/STATE.md désigne l'auditeur pour un audit initial ou la review indépendante d'un correctif Identity.
---

# Auditer `a-identity.feature`

1. Lire complètement `../../../shared/audit-gherkin-feature/SKILL.md`.
2. Lire complètement `../../DOMAIN.md` et `../../STATE.md`.
3. Lire l'audit public et les runs indiqués par l'état.
4. Exécuter uniquement le mode demandé par l'état.

## Mode audit initial

- Cartographier tous les scénarios de `a-identity.feature`.
- Contrôler les invariants listés dans `DOMAIN.md`.
- Documenter les écarts sous des identifiants `AID-*`.
- Écrire la conclusion dans `../runs/`.

## Mode review

- Comparer la baseline et le commit de correction indiqués dans `STATE.md`.
- Reviewer séparément AID-001, AID-002 et AID-005.
- Rejouer les gates applicables de `DOMAIN.md`.
- Vérifier les chemins Bundle, WASM/client, Gateway et Provider pertinents.
- Ne pas fermer ni corriger AID-003 ou AID-004.
- Ne modifier aucun fichier Rust.
- Mettre à jour l'audit public vers `VÉRIFIÉ` uniquement pour les findings
  effectivement reproduits.
- Écrire `../runs/<date>-audit-review-01.md`.
- Positionner l'état sur `CORRECTION_REQUESTED` ou
  `DECISION_REQUIRED` ou `IMPACT_REVIEW_REQUESTED`.

La conclusion du correcteur est informative. Reconstruire le verdict depuis le
diff, le code et les tests.

---
name: correct-a-identity
description: Corriger exclusivement les findings Identity explicitement demandés dans features/.agents/a-identity/STATE.md. Utiliser ce skill pour modifier les chemins DID, succession ou transition d'époque et leurs tests après un audit a-identity, sans élargir le périmètre ni auto-valider la correction.
---

# Corriger `a-identity.feature`

1. Lire complètement `../../../shared/correct-gherkin-feature/SKILL.md`.
2. Lire complètement `../../DOMAIN.md` et `../../STATE.md`.
3. Lire l'audit public et la dernière conclusion de l'auditeur.
4. Traiter uniquement les findings assignés par l'état.

## Règles du domaine

- Porter les invariants DID partagés dans `aithos-core`, pas dans un step.
- Fermer le parsing wire avant toute reconstruction du JCS vérifié.
- Lier explicitement précédent, transition et successeur.
- Vérifier les surfaces qui consomment le verdict Core.
- Préserver les vecteurs positifs byte-exacts sauf décision normative contraire.
- Ne pas traiter AID-003 ou AID-004 sans assignation explicite.

## Handoff

- Écrire la conclusion dans `../runs/`.
- Consigner baseline, commit, RED, GREEN et fichiers modifiés.
- Marquer les findings au plus `IMPLÉMENTÉ`.
- Demander une review à `audit-a-identity`.
- Positionner `STATE.md` sur `REVIEW_REQUESTED`.

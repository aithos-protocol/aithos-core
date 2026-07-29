# Domaine des features Gherkin

Ces instructions s'appliquent à tout travail initié depuis `features/`.

## Routage obligatoire

Avant d'auditer ou de corriger une feature :

1. lire `.agents/PROCESS.md` ;
2. trouver son domaine sous `.agents/<nom-de-feature>/` ;
3. lire `DOMAIN.md` et `STATE.md` ;
4. charger le skill spécialisé indiqué par `STATE.md` ;
5. respecter le rôle demandé sans anticiper l'étape suivante.

Pour `a-identity.feature` :

- audit ou review :
  `.agents/a-identity/auditor/audit-a-identity/SKILL.md` ;
- correction :
  `.agents/a-identity/corrector/correct-a-identity/SKILL.md`.

## Frontières des rôles

- L'auditeur inspecte, classe, documente et review. Il ne corrige pas le code
  de production.
- Le correcteur implémente les findings demandés. Il peut marquer un finding
  `IMPLÉMENTÉ`, jamais `VÉRIFIÉ`.
- Le reviewer d'impacts intervient uniquement après une review acceptée. Il
  signale les autres features potentiellement touchées sans les modifier ni
  les relancer.

L'audit courant porte uniquement sur la vérité sémantique des scénarios
existants qui passent. La recherche de scénarios entièrement manquants est
hors périmètre. Les tests supplémentaires nécessaires pour prouver un
correctif demandé restent dans le périmètre.

Les commentaires des `.feature` pointent vers les audits publics de
`docs/audits/features/`. Les conclusions opérationnelles et les handoffs
restent sous `.agents/`.

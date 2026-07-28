# Handoff — démo Léa provider pilotable en CLI

> **ARCHIVE DE PREUVE.** Cette répétition Provider/CLI précède la démo intégrée
> G4 + Sheets et ne représente plus le chemin critique.

**Date :** 2026-07-21

**Statut :** prêt pour une répétition accompagnée en CLI ; dashboard
reportée à l'étape suivante.

**Runbook opératoire :** `docs/DEMO-LEA-PROVIDER-CLI.md`.

## Livré

- promotion owner d'un historique local vers le provider par le wire
  signé, reprenable et fail-closed (`owner-replicate-history`) ;
- publication effective d'une nouvelle édition lors d'un hot edit de
  briefing ;
- génération d'une configuration provider sans secret
  (`demo-lea-render-config`) : ventes en mode A, journal en mode B,
  credentials référencés dans Vault ;
- trois MCP synthétiques distincts avec discovery owner, authentification
  des appels et réponse Notion à cinq prospects ;
- driver `aithos-demo-lea` pour jouer ou vérifier les beats 1 à 6, les
  gestes owner et auditeur des beats 7 et 8 restant visibles dans la CLI
  principale ;
- e2e local et provider fondés sur le même workflow de réplication que la
  commande opérateur.

## Preuves de clôture

| Contrôle | Résultat |
|---|---:|
| gateway lib + bins | 86/86 |
| e2e démo Léa fs + provider | 2/2 |
| Cucumber gateway | 152 scénarios / 790 steps |
| clippy gateway, tous targets, `-D warnings` | vert |
| rustfmt gateway | vert |
| `cargo check --workspace --locked` | vert |

La configuration générée a aussi été parsée par le vrai chargeur et
contrôlée sans bearer ni token Vault en clair. Aucun tenant de démo n'a
été laissé sur AWS par ce développement.

## Reporté sans bloquer la démo CLI

- dashboard opérateur puis owner/auditeur ;
- connecteurs Notion/Gmail/Calendar réels et leurs contraintes
  TLS/OAuth/session ;
- authentification de l'endpoint agent s'il sort de loopback, Vault de
  production et rotation ;
- quotas/rate limits, GC/rétention, DR/DPA ;
- durcissements E1/E2, D3/D5/D6 et re-dérivation du sidecar ;
- optimisations des gates de performance déjà consignées.

## Prochaine action

Faire la répétition avec Mathieu en suivant le runbook, commande par
commande. Les écarts observés pendant cette répétition deviennent les
seuls correctifs bloquants avant de construire le parcours dashboard.

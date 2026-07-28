# Documentation `aithos-core`

État vérifié le 22 juillet 2026 contre les sources Rust, les features BDD, les
tests ciblés et l'historique Git. Cet index sépare volontairement la norme, les
références vivantes, les chantiers et les archives.

## Sources de vérité

1. `../spec/00-overview.md` à `../spec/10-threat-model.md` : norme protocolaire
   draft ;
2. `../README.md` : carte des crates et état global du dépôt ;
3. `../DESIGN.md` : architecture de référence, désormais implémentée au-delà du
   jalon greenfield ;
4. `../rust/**/tests/features/*.feature` : contrats comportementaux, dont les
   scénarios `@wip` signalent les comportements non qualifiés ;
5. le code et les tests : arbitres en cas de divergence avec un handoff daté.

Les anciens handoffs et prompts ne sont jamais une source de vérité courante,
même lorsqu'ils contiennent un résultat de test valable à leur date.

## Chantiers courants

- [`HANDOFF-GATEWAY-COMPAGNON-DEMO-INTEGREE-2026-07-22.md`](HANDOFF-GATEWAY-COMPAGNON-DEMO-INTEGREE-2026-07-22.md) :
  audience G4, CORS Gateway/Provider et bundle de démo ; implémentation locale
  présente, gate navigateur live et intégration Git encore à clore ;
- [`MANDATES-PRODUCT-GAPS.md`](MANDATES-PRODUCT-GAPS.md) : deux anciens P0
  protocolaires fermés, surface owner/lifecycle/preview encore partielle ;
- [`CHANTIER-REFACTOR-OAUTH-LIBRAIRIES-STANDARD-2026-07-22.md`](CHANTIER-REFACTOR-OAUTH-LIBRAIRIES-STANDARD-2026-07-22.md) :
  refactor futur, non bloquant pour la première démo ;
- [`GMAIL-SEND-EXTENSION-ARCHITECTURE.md`](GMAIL-SEND-EXTENSION-ARCHITECTURE.md) :
  architecture désormais implémentée localement, qualification live Gmail
  encore ouverte.

## Références d'architecture et produit

- [`DEPLOYMENT-CONTAINMENT.md`](DEPLOYMENT-CONTAINMENT.md) ;
- [`GATEWAY-BOOTSTRAP.md`](GATEWAY-BOOTSTRAP.md) ;
- [`HUB-MCP.md`](HUB-MCP.md) ;
- [`INFRA-PROVIDER.md`](INFRA-PROVIDER.md) ;
- [`STANDARDS-COMPAT.md`](STANDARDS-COMPAT.md) ;
- [`EXPLORATION-DESKTOP-GATEWAY.md`](EXPLORATION-DESKTOP-GATEWAY.md), piste
  explicitement non tranchée.

## Runbooks encore utilisables

- [`CLI-GUIDE.md`](CLI-GUIDE.md) ;
- [`CLI-INSTALL-VAULT.md`](CLI-INSTALL-VAULT.md) ;
- [`CLI-DELEGATED-OAUTH.md`](CLI-DELEGATED-OAUTH.md) ;
- [`DEMO-GATEWAY-VAULT.md`](DEMO-GATEWAY-VAULT.md) ;
- [`DEMO-GATEWAY-NOTION-CLI.md`](DEMO-GATEWAY-NOTION-CLI.md), canary live
  Notion qualifié ;
- [`RUNBOOK-CONNECTOR-PROFILES-OAUTH-SAAS.md`](RUNBOOK-CONNECTOR-PROFILES-OAUTH-SAAS.md),
  exploitation locale des profils Notion/Sheets/Gmail ;
- [`DEMO-LEA.md`](DEMO-LEA.md), [`DEMO-LEA-SCENARIO.md`](DEMO-LEA-SCENARIO.md)
  et [`DEMO-LEA-PROVIDER-CLI.md`](DEMO-LEA-PROVIDER-CLI.md), parcours historique
  encore reproductible mais distinct de la démo intégrée G4 + Sheets.

Les runbooks [`DEMO-GATEWAY-GENERIQUE.md`](DEMO-GATEWAY-GENERIQUE.md) et
[`GUIDE-GATEWAY-DEMO-LOCALE.md`](GUIDE-GATEWAY-DEMO-LOCALE.md) sont déjà marqués
DEV/legacy. Ils restent utiles pour le diagnostic, pas pour qualifier la démo
intégrée.

## Décisions et preuves datées conservées

- [`DECISION-COMPUTE-STORE-PROPOSITION-GATE6-2026-07-20.md`](DECISION-COMPUTE-STORE-PROPOSITION-GATE6-2026-07-20.md) ;
- [`REDLINE-A1-DRAFT2-PROPOSITION-GATE5-2026-07-20.md`](REDLINE-A1-DRAFT2-PROPOSITION-GATE5-2026-07-20.md) ;
- [`ADDENDUM-P5-RACINE-REELLE-2026-07-21.md`](ADDENDUM-P5-RACINE-REELLE-2026-07-21.md),
  explicitement une preuve live datée, pas une sonde actuelle ;
- [`CONFORMANCE.md`](CONFORMANCE.md), mesure du jalon K du 11 juillet, antérieure
  à CB13 et aux surfaces WASM/Gateway actuelles.

## Archives — ne pas utiliser comme plans de reprise

Chaque fichier ci-dessous porte désormais une bannière d'archive ou de
supersession en tête. Ils vivent désormais dans [`archive/`](archive/) ; les liens des références
vivantes ont été mis à jour, preuves et historique de décision préservés.

### Construction Core/Bundle

- `EXECUTION-PLAN.md`, `HANDOFF.md` ;
- `HANDOFF-CORE-BUNDLE-PROTOCOL-*.md` ;
- `HANDOFF-CORE-PROTOCOL-*.md` ;
- `PROMPT-CORE-PROTOCOL-*.md` et `PROMPT-REPRISE-CORE-*.md` ;
- `NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md` ;
- `2026-07-12-delegated-writes.md` et `HANDOFF-2026-07-12-pass-L.md`.

### Gateway, OAuth initial et démos antérieures

- `GATEWAY-HANDOFF.md`, `HANDOFF-GATEWAY-HUB.md` ;
- `HANDOFF-GATEWAY-G2-G6*.md`, `HANDOFF-GATEWAY-G3*.md` ;
- `HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md` et
  `SESSION-G1-G7-ENTERPRISE-2026-07-21.md` ;
- `HANDOFF-GATEWAY-G4-PROD-MCP-DELEGATED-SESSIONS-2026-07-22.md` et
  `PROMPT-REPRISE-G4.md` ;
- `HANDOFF-GATEWAY-VAULT*.md` ;
- `GATEWAY-UPSTREAM-OAUTH-VM.md`, `HANDOFF-GATEWAY-OAUTH-AMONT-VM-2026-07-21.md`
  et `HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md` ;
- `HANDOFF-DEMO-GATEWAY-LOCAL-CLI-2026-07-22.md` ;
- `HANDOFF-DEMO-LEA-*.md` ;
- `GAPS-DEMO-E2E.md`.

### Provider

- `HANDOFF-PROVIDER-AWS.md` ;
- tous les `HANDOFF-PROVIDER-P2-*.md`, `HANDOFF-PROVIDER-P3*.md`,
  `HANDOFF-PROVIDER-P5-*.md` et `HANDOFF-PROVIDER-P7*.md` ;
- `PROMPT-REPRISE-M2-2026-07-18.md` ;
- tous les `PROMPT-REPRISE-PROVIDER-*.md`.

### Client, SDK, mandats et démo intégrée

- `SDK-V0-CONTRACT.md` ;
- `HANDOFF-MANDATES-SURFACE-2026-07-15.md` et
  `HANDOFF-MANDATES-M3-2026-07-16.md` ;
- `ETAT-DES-LIEUX-DEMO-GATEWAY-CLIENT-SDK-2026-07-22.md` ;
- `HANDOFF-CLIENT-SDK-G4-INTEGRATION-2026-07-22.md` ;
- `HANDOFF-FINALISATION-CLIENT-SDK-DEMO-INTEGREE-2026-07-22.md` ;
- `STOP-G4-SC1-NON-ROOT-SESSION-2026-07-22.md`, blocage résolu ;
- `HANDOFF-GATEWAY-OAUTH-CONNECTEURS-SAAS-2026-07-22.md`, plan OAC
  partiellement clos ;
- `HANDOFF-GMAIL-SEND-EXTENSION.md`.

## Limites de l'audit

L'archivage ne signifie pas que chaque assertion historique est fausse : il
signifie que le document n'est plus sûr comme état ou ordre d'exécution. Les
features conservent encore des scénarios `@wip`, notamment autour des mandats,
des sessions déléguées, de la durabilité OAuth et de certaines surfaces
Provider. Aucun bandeau de ce nettoyage ne transforme ces scénarios en fonction
qualifiée.

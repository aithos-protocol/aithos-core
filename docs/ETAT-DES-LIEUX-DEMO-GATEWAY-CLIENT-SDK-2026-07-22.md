# État des lieux — démo Gateway, aithos-client et SDK v2

Date : 2026-07-22

Statut : **démo locale prête ; démo navigateur intégrée à terminer**.

## 1. Verdict exécutif

Les briques cryptographiques et protocolaires ne sont plus le principal
problème. `aithos-client`, le SDK v2 et la Gateway possèdent les surfaces G4
nécessaires, et leurs suites locales sont vertes.

Deux niveaux de démonstration doivent être distingués :

- **jouable maintenant** : `/delegation` en local, sans infrastructure, avec
  création d'Ethos, récupération Owner/délégué, mandat exact, écriture déléguée
  et refus voisin ;
- **pas encore bout en bout** : navigateur → Provider → Gateway G4/OAuth → token
  → MCP SaaS → preuve Gamma.

Le second parcours est proche structurellement mais possède encore des blocages
concrets de transport, d'UI, de configuration et d'E2E. Il ne doit pas être
présenté comme validé avant leur fermeture.

## 2. Baselines auditées

| Dépôt | Branche | HEAD observé | État |
| --- | --- | --- | --- |
| `aithos-core` | `codex/publish-aithos-core-busl` | `92abb81` | propre, 55 commits devant `origin/main` |
| `aithos-client` | `codex/client-sdk-v2-parking` | `890acf3` | propre, branche de travail non intégrée |
| `aithos-sdk` | `codex/g1-g7-enterprise-sdk` | `4d28f1c` | propre |
| `aithos-sdk-example` | `codex/g1-g7-enterprise-dashboard` | `914c629` | propre |

La démo doit épingler ces quatre révisions ou leur intégration explicite. Un
build contre `main` ne reproduira pas nécessairement les surfaces observées.

## 3. Ce qui est réellement livré

| Couche | Surface | Statut |
| --- | --- | --- |
| Client Rust/WASM | cold verification, handles opaques, Owner reconnecté, mandats d'action | livré |
| Client G4 | parent `Issue(depth=1)`, publication K1-C, leaf, grant Gamma, PoP et vérification parent + leaf | livré |
| SDK Provider | publication CAS, manifest en dernier, lecture publique | livré |
| SDK MCP | initialize, tools/list, tools/call, ping, JSON/SSE, bearer et refus typé | livré |
| SDK control | status, contextes, preuve paginée et Gamma vérifié | livré sur l'ancienne surface |
| SDK OAuth entrant | discovery RFC 9728/8414, DCR, PKCE, state, code, token et refresh | livré |
| SDK G4 | prepare, prepare-grant, complete, cancel et orchestration du parent | livré |
| Gateway G4 | routes et activation réelle du leaf de session | livré dans le code |
| Dashboard `/` | console Provider/Gateway historique G1/G7 | présente, mais pas une preuve live actuelle |
| Dashboard `/delegation` | parcours offline Owner/délégué | prêt localement |
| Dashboard `/delegation` avancé | publication du parent et signature de cérémonie | partiel |
| Onboarding profils Sheets/Gmail/Notion | SDK et UI d'administration | manquant |

Le SDK utilise le vrai package WASM `@aithos/client` pour la cryptographie. Ses
mocks se limitent aux transports HTTP dans les tests d'intégration.

## 4. Preuves rejouées pendant l'audit

- `aithos-client` : `cargo test --workspace` vert, dont **113 scénarios BDD**
  et les tests Rust/WASM G4, publication, contrôle et navigateur ;
- `aithos-sdk` : `npm test`, **27/27** verts ;
- `aithos-sdk-example` : build Vinext vert et `npm test`, **6/6** verts ;
- les quatre worktrees sont restés propres après les gates.

Observation live en lecture seule le 2026-07-22 :

- `https://store.aithos.fr/healthz` répond `200` avec
  `x-aithos-store: 1.0.0-draft.1` ;
- `https://demo.mcp.aithos.fr/.well-known/oauth-protected-resource` répond
  `200` et annonce la ressource `/mcp` ;
- `https://demo.mcp.aithos.fr/.well-known/oauth-authorization-server` répond
  `200` et annonce DCR, Authorization Code, refresh et PKCE S256 ;
- `/healthz` n'est pas exposé sur cette Gateway publique et répond `404` ; ce
  n'est donc pas une sonde de readiness utilisable pour la démo ;
- Vault écoute localement sur `127.0.0.1:8200`, mais aucune Gateway locale
  n'écoutait sur `127.0.0.1:4890` au moment de l'audit. Le handoff CLI qui la
  décrivait active est devenu obsolète sur ce point.

Ces observations prouvent la présence des endpoints publics, pas un parcours
OAuth ni SaaS réussi.

## 5. Blocages avant une démo navigateur intégrée

### P0-1 — synchroniser la grammaire control client/SDK

La Gateway expose désormais :

- `profile-stage` ;
- `client-secret` ;
- `disconnect` ;
- review, approve, deny et dispatch des approbations Gmail.

Le client et le SDK ne savent signer/transporter que l'ancienne surface
`stage`, OAuth start/status, activate et suppression du draft. Ils ne peuvent
donc pas administrer les profils SaaS actuels depuis le dashboard.

Pour une première démo Sheets read-only, ce blocage peut être borné par un
pré-provisioning opérateur. Il reste P0 pour toute démo d'onboarding ou Gmail.

### P0-2 — corriger les POST qui doivent avoir un corps strictement vide

Le SDK envoie actuellement `{}` pour `oauth/start` et `activate`. La Gateway et
le client exigent un corps de zéro octet pour ces actions. Une enveloppe signée
sur `{}` n'est pas équivalente à une enveloppe signée sur un corps absent ; le
parcours réel est refusé.

Le SDK doit proposer une primitive POST sans corps et ses tests doivent passer
par la vraie grammaire client/Gateway.

### P0-3 — câbler OAuth et G4 comme un seul flux dans l'UI

Le SDK sait déjà effectuer le parcours correct, mais `/delegation` :

- demande manuellement `transactionId` et l'audience ;
- instancie G4 sans conserver la même instance `GatewayOAuthClient` ;
- ne passe pas l'objet `authorization` à `waitForVerifiedParent()` ;
- affiche le callback comme un lien au lieu d'appeler `oauth.exchange()` ;
- ne transmet ensuite aucun access token à `sdk.mcp()` ;
- ne termine ni par un appel autorisé, ni par un refus voisin, ni par une preuve.

L'UI court-circuite ainsi les recroisements client id, redirect URI, PKCE et
state que le SDK possède déjà.

### P0-4 — rendre les transports navigateur réellement accessibles

Depuis `http://localhost:3000`, le dashboard appelle des origines différentes.
Les observations et le code montrent actuellement :

- aucun header CORS sur la discovery OAuth Gateway ;
- `OPTIONS /ceremony/prepare` répond `405` sans autorisation CORS ;
- le Provider n'autorise `*` que pour les lectures publiques anonymes ;
- les publications signées avec `X-Aithos-Auth` ne disposent pas du preflight
  nécessaire ;
- le control plane possède une allowlist d'origines exacte, mais elle ne couvre
  pas automatiquement OAuth, cérémonie, MCP et Provider.

Une vraie solution de démo doit être choisie et testée : origine unique, ou CORS
exact configuré sur chaque surface nécessaire. Désactiver la sécurité du
navigateur n'est pas un gate acceptable.

### P0-5 — fournir une configuration reproductible et un E2E réel

Il manque un bundle de démonstration qui assemble et épingle :

- Gateway, Vault et state OAuth durable ;
- Provider, tenant, contexte et Ethos ;
- origine exacte du dashboard ;
- manifest et profil Sheets read-only pré-approuvés ;
- credentials Google et redirect URI gérés hors dépôt ;
- synchronisation du parent G4 depuis le Provider ;
- noms exacts du connecteur et de la capability ;
- procédure de nettoyage sans afficher de secret.

Le gate final doit partir d'un navigateur réel et traverser la Gateway, le
Provider, Vault et l'amont OAuth. Les transports simulés actuels ne suffisent
pas à cette qualification.

## 6. Écarts non bloquants pour le premier beat, mais à ne pas masquer

- la console `/` utilise encore des noms historiques (`calendar-safe`,
  `calendar.events_read/write`) et un bearer saisi manuellement ;
- le mandat d'action affiché par cette console sert de garde locale et n'est pas
  transporté comme autorité du call MCP ;
- aucune page ne liste ni n'administre les profils connecteurs actuels ;
- la PoP de cérémonie existe, mais la PoP générique de découverte des mandats
  reste explicitement absente ;
- le dashboard n'a pas de test de clics navigateur contre une vraie Gateway ;
- le package client, le SDK et le dashboard restent sur des branches non
  intégrées et non publiées ;
- la qualification production OAC-7 et les points de revue du runtime SaaS
  restent un chantier séparé. Une démo Sheets read-only ne vaut pas validation
  production des profils, du lifecycle, des approbations Gmail ou des courses
  OAuth.

## 7. Parcours de démonstration recommandé

### Beat A — répétition immédiate, sans infrastructure

URL : `http://localhost:3000/delegation`.

1. créer un Ethos local et télécharger la récupération Owner ;
2. se reconnecter comme Owner ;
3. générer une identité déléguée et télécharger séparément sa récupération ;
4. émettre un mandat d'édition exact ;
5. se connecter comme délégué ;
6. écrire dans le périmètre autorisé ;
7. montrer le refus d'une cible voisine ;
8. recharger la page et montrer que les secrets doivent être réimportés.

Ce beat est prêt, reproductible et honnête, mais ne montre ni Gateway ni OAuth.

### Beat B — première démo intégrée cible

Choix recommandé : **G4 + Google Sheets read-only**, connecteur pré-provisionné.

1. démarrer le bundle Gateway/Vault/Provider et vérifier ses probes ;
2. ouvrir le dashboard sur une origine autorisée ;
3. reconnecter l'Owner et créer l'identité déléguée ;
4. démarrer OAuth entrant avec `beginAuthorization()` ;
5. créer et publier le parent G4 lié exactement à l'audience `/mcp` ;
6. faire vérifier le parent par la Gateway ;
7. signer localement leaf, grant et cérémonie avec le même objet OAuth ;
8. échanger le code et garder les tokens uniquement en mémoire du handle prévu ;
9. appeler `tools/list`, puis `<instance>__read_range` ;
10. montrer le refus d'une capability ou d'un périmètre voisin ;
11. charger et vérifier localement la preuve Gamma ;
12. verrouiller les handles et nettoyer les ressources de démonstration.

Sheets read-only est préférable au premier passage : Sheets write ajoute la
preuve d'effet, et Gmail ajoute le workflow complet d'approbation et d'envoi
unique.

## 8. Définition de fini de la démo intégrée

La démo est prête seulement si :

- elle se déroule dans un navigateur normal, sans contournement CORS ;
- aucun `transactionId`, bearer ou callback n'est copié manuellement ;
- le même objet OAuth vérifie le parcours du début à l'échange du code ;
- le parent vient réellement du Provider et est vérifié par la Gateway ;
- le token vient réellement de la Gateway et permet un appel MCP borné ;
- un refus voisin est observé avant l'amont ;
- la preuve Gamma est récupérée et vérifiée par le client ;
- aucun secret, token, mandat privé ou plaintext n'est persisté dans le
  navigateur ou journalisé ;
- un script E2E rejoue le parcours minimal et laisse un diagnostic exploitable
  sans donnée sensible ;
- les quatre SHA utilisés sont consignés dans le runbook de répétition.

## 9. Ordre de travail conseillé

1. fermer P0-2 (corps vide) et ajouter le test croisé ;
2. décider et fermer P0-4 (origine/CORS) ;
3. câbler le flux OAuth/G4/token/MCP de `/delegation` ;
4. produire la configuration locale pré-provisionnée Sheets read-only ;
5. écrire et passer l'E2E navigateur réel ;
6. répéter le beat B et figer le runbook ;
7. seulement ensuite ouvrir l'onboarding UI des profils et Gmail.

La synchronisation complète P0-1 peut avancer en parallèle, mais le
pré-provisioning permet de ne pas la placer sur le chemin critique du premier
beat intégré.

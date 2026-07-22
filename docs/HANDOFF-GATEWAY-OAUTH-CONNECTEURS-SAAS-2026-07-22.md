# HANDOFF — OAuth amont et connecteurs SaaS gouvernés

**Date :** 2026-07-22

**Dépôt :** `code/aithos-core`

**Branche observée :** `codex/publish-aithos-core-busl`

**HEAD d'entrée :** `1c11bb1` (`fix(gateway): expose delegated hub and briefing tools`)

**Statut :** plan de reprise consolidé ; aucune connexion SaaS réelle n'est encore qualifiée.

Ce document reprend et actualise :

- `HANDOFF-GATEWAY-OAUTH-AMONT-VM-2026-07-21.md` ;
- `HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md` ;
- `HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md` ;
- `GMAIL-SEND-EXTENSION-ARCHITECTURE.md` et
  `HANDOFF-GMAIL-SEND-EXTENSION.md`.

Il ne concerne pas l'OAuth **entrant** utilisé par Claude pour ouvrir une session
sur la gateway. Cette verticale est désormais prouvée par G4. Ici, la gateway est
le client OAuth **amont** qui obtient, conserve et utilise une autorisation Notion
ou Google sans livrer de token à l'agent.

## 1. Verdict et cible produit

Le socle générique ne doit pas être réécrit. Sont déjà livrés et testés :

- Authorization Code + PKCE S256 et `state` à usage unique ;
- callback public `/oauth/callback` ;
- client secret, état pending, access token et refresh token dans Vault KV v2 ;
- contrôle des scopes, refresh sérialisé et rotation du refresh token ;
- injection du bearer au dernier moment, après autorisation et log-before-effect ;
- refus fail-closed avant toute requête amont ;
- API owner `/control/v1/connectors/**`, staging d'un connecteur préapprouvé,
  activation à chaud et contrôle du manifeste MCP pinné.

La cible suivante est donc une **bibliothèque de profils de connecteurs** sur un
socle OAuth commun, avec deux chemins d'exécution :

| Famille | Première cible | Exécution | Pourquoi |
|---|---|---|---|
| MCP hébergé avec OAuth natif | Notion | relay MCP gouverné | Valide discovery, enregistrement client et OAuth MCP standard |
| API SaaS sans MCP requis | Google Sheets | extension REST compilée | Surface minimale, schéma et effets contrôlés par Aithos |
| API SaaS à effet sensible | Gmail | extension `send_guarded` | `gmail.send` minimal, approbation et preuve avant envoi |

Un serveur MCP tiers ou auto-hébergé reste possible, mais n'est pas la voie par
défaut : il exige la même approbation de manifeste, le même pin et une décision de
confiance explicite. Ne pas inventer un « MCP Gmail officiel stable » comme
prérequis du plan.

## 2. Écart exact du socle actuel

`UpstreamOAuthConfig` exige aujourd'hui des valeurs statiques : `auth_url`,
`token_url`, `client_id`, une référence de `client_secret`, des scopes, une
redirect URI et les références Vault. `UpstreamOAuthClient` ajoute uniquement les
paramètres OAuth communs à l'URL d'autorisation.

Les manques bloquant les fournisseurs réels sont :

1. aucune discovery de Protected Resource Metadata puis Authorization Server
   Metadata (RFC 9728 et RFC 8414) ;
2. aucun Dynamic Client Registration (RFC 7591), aucun Client ID Metadata
   Document, et pas de client public `token_endpoint_auth_method=none` ;
3. `client_secret` est toujours obligatoire et toujours envoyé au token endpoint ;
4. aucun paramètre d'autorisation typé par profil, notamment
   `access_type=offline`, `include_granted_scopes=true` et le traitement borné de
   `prompt=consent` pour Google ;
5. pas de liaison durable du consentement à l'identité de compte retournée
   (`issuer`, `sub`, compte/workspace) ;
6. pas de cycle public `reauth_required`, déconnexion/révocation et nettoyage
   explicite des références runtime ;
7. pas encore de profil Notion, Google Sheets ou Gmail, ni de canary fournisseur
   réel ;
8. le registre actuel modélise surtout une instance par id de connecteur : le
   multi-compte et l'isolation par principal doivent être contractualisés avant
   l'UI finale.

Ce qui précède est un delta fournisseur, pas une remise en cause de G3, G4, du
hub, de Gamma, des mandats ou de la garde Vault.

## 3. Invariants non négociables

- Les applications OAuth appartiennent au client ou à son organisation ; Aithos
  n'héberge pas silencieusement un client OAuth mutualisé.
- Aucun token, secret, code, verifier ou `state` n'entre dans le navigateur du
  dashboard après le callback, dans Gamma, les logs, les erreurs ou la surface MCP.
- Les coordonnées Vault sont dérivées côté gateway ; le navigateur ne choisit ni
  mount ni path.
- Un consentement est lié à `(contexte, principal, connecteur, compte, issuer)`.
- Les scopes accordés doivent couvrir exactement le profil approuvé ; un
  élargissement demande un nouveau consentement et une nouvelle approbation.
- Autorité, bornes, révocation et log durable précèdent la résolution du token et
  toute sortie réseau.
- L'activation d'un MCP compare la surface live au manifeste approuvé ; une API
  REST n'expose que les outils synthétiques compilés et pinnés par Aithos.
- Une perte OAuth désactive seulement le connecteur concerné et ne provoque jamais
  un appel anonyme.
- Toute action d'écriture est idempotente ou protégée par digest/idempotency key ;
  Gmail Send exige en plus la politique et l'approbation prévues par GSE.

## 4. Plan d'action

### OAC-0 — contrats et inventaire, sans fournisseur réel

**But :** figer les variantes OAuth sans modifier le chemin heureux existant.

- Ajouter aux features Gherkin une matrice `confidential client`, `public client`,
  discovery, DCR/CIMD, paramètres provider, rotation, réauthentification et
  isolation multi-compte.
- Définir un `ConnectorProfile` fermé et versionné : endpoints/discovery autorisés,
  méthode d'authentification client, scopes admissibles, paramètres OAuth typés,
  audience/resource, API ou MCP, classe de risque et manifeste attendu.
- Interdire les maps libres de query params et les URLs découvertes hors issuer
  validé. Pinner l'issuer, les endpoints HTTPS et les algorithmes annoncés.
- Formaliser le modèle de clé Vault, sans migration destructive :
  `connectors/<context>/<principal>/<connector>/<account>/{registration,pending,token}`.
- Produire les doubles réseau : metadata server, DCR, AS, ressource MCP, Google
  OAuth, Sheets et Gmail.

**Gate :** contrats RED observés et commit étroit ; aucune modification du Core ou
de la grammaire des mandats.

### OAC-1 — moderniser le client OAuth générique

**But :** rendre le socle compatible avec les profils Notion et Google.

- Implémenter discovery RFC 9728 → RFC 8414 avec limites de taille, timeout,
  HTTPS, allowlist d'issuer et validation stricte des métadonnées.
- Supporter explicitement `client_secret_post`, si requis
  `client_secret_basic`, et `none`; ne jamais déduire la méthode d'un secret vide.
- Ajouter DCR RFC 7591 et CIMD comme stratégies déclarées. Conserver les
  coordonnées d'enregistrement dans Vault si elles contiennent un secret.
- Ajouter des paramètres d'autorisation **typés et allowlistés** par profil ;
  aucun passthrough arbitraire depuis le dashboard.
- Conserver la rotation actuelle du refresh token, accepter les réponses qui ne
  répètent pas le refresh token, suivre l'expiration éventuelle de celui-ci et
  publier seulement `connected|expired|reauth_required|unavailable`.
- Ajouter une primitive de déconnexion : retrait immédiat de la registry runtime,
  révocation fournisseur lorsque disponible, puis suppression Vault sûre ou
  résidu explicitement signalé si le broker ne sait pas supprimer.
- Lier le token set à l'issuer et à l'identité de compte vérifiée ; refuser qu'un
  callback ou refresh change silencieusement de compte.

**Gate :** faux AS adversarial vert, aucune régression des scénarios OAuth amont et
G7, zéro sentinelle secrète dans réponses, URL finale, logs, stores ou preuves.

### OAC-2 — canary Notion MCP hébergé

**But :** premier fournisseur réel, car il exerce tout le chemin MCP OAuth moderne.

- Endpoint officiel : `https://mcp.notion.com/mcp` en Streamable HTTP.
- Découvrir les métadonnées, utiliser DCR ou CIMD selon le profil approuvé, PKCE
  S256 et refresh ; ne pas demander de bearer statique.
- Enroller la surface live, faire classer et signer le manifeste par l'owner, puis
  n'exposer qu'un petit sous-ensemble `read` initial.
- Prouver : consentement humain, activation à chaud, `tools/list` pinné, appel
  mandaté, refus voisin, refresh, drift et déconnexion.
- Ne pas promettre un usage totalement headless : Notion exige actuellement un
  consentement OAuth utilisateur pour son MCP hébergé.

**Gate live :** deux répétitions depuis un état Vault frais, puis une répétition
après restart gateway/Vault, sans token dans le client agent.

### OAC-3 — profil OAuth Google commun

**But :** mutualiser l'onboarding de Sheets et Gmail sans mutualiser leurs scopes.

- Profil web server Google avec endpoint d'autorisation et token officiels,
  `access_type=offline`, `include_granted_scopes=true` et redirect URI exacte.
- Utiliser `prompt=consent` seulement lors de la création/réparation explicite du
  consentement, pas à chaque connexion.
- Conserver le refresh token existant lorsque Google n'en renvoie pas un nouveau.
- En mode de publication `Testing`, rendre visible dans le runbook que les refresh
  tokens Google expirent après sept jours pour les scopes hors identité de base ;
  le gate durable nécessite une application `Internal` Workspace ou publiée selon
  le contexte client.
- Un profil par capability : aucun token combinant Gmail et Sheets par commodité.
- Vérifier le compte consenti et exposer seulement un label expurgé au dashboard.

**Gate :** faux Google complet, refresh absent/rotaté/révoqué, `invalid_grant`,
scope réduit et compte différent couverts avant tout canary réel.

### OAC-4 — Google Sheets, lecture puis écriture bornée

**But :** premier connecteur API REST synthétique.

- Introduire ou réutiliser le registre d'extensions compilées défini par GSE-0 ;
  ne pas ajouter un second listener MCP.
- Premier outil : lecture d'une plage A1 explicitement bornée sur un spreadsheet
  autorisé. Préférer `drive.file` avec sélection Google Picker pour limiter l'accès
  aux fichiers choisis ; à défaut, documenter explicitement que
  `spreadsheets.readonly` voit tous les spreadsheets du compte.
- Ajouter ensuite un outil `values_update_guarded` : ids/ranges allowlistés,
  dimensions et octets bornés, digest, idempotence, quota, et résultat expurgé.
- Le scope `spreadsheets` n'est accepté que pour le lot write ; il ne peut pas
  être ajouté à un consentement read sans nouvelle approbation.
- Les règles Aithos peuvent limiter une plage même si le scope Google s'applique
  au fichier entier ; les `ProtectedRange` Google restent une défense en profondeur.

**Gate live :** lecture d'une cellule de démo, refus d'une plage voisine, écriture
bornée optionnelle, preuve Gamma et révocation avec zéro appel après retrait.

### OAC-5 — Gmail à effet gouverné

**But :** réaliser le plan `aithos-gmail__send_guarded` sans élargir la boîte mail.

- Réutiliser le profil Google mais demander uniquement
  `https://www.googleapis.com/auth/gmail.send` pour la v1.
- Implémenter `users.messages.send` derrière le pack compilé GSE : normalisation
  MIME, destinataires/domaines allowlistés, quotas, digest, outbox chiffrée,
  approbation, expiration et idempotence.
- Ne pas demander `gmail.readonly`, `gmail.modify` ou `gmail.compose` pour faire
  fonctionner l'envoi. Ces scopes sont plus larges ou restreints et constituent
  un produit séparé avec exigences de vérification/assessment.
- Ne jamais mettre le corps ou les destinataires en clair dans Gamma ; relier le
  `message_id` au digest et à la décision d'approbation.

**Gate live :** refus, demande pending, approbation puis un seul envoi, révocation
entre approbation et dispatch, et aucune fuite du corps/token.

### OAC-6 — onboarding dashboard et multi-compte

**But :** rendre le parcours utilisable sans déplacer la confiance dans le front.

- Consommer les routes `/control/v1/connectors/**` existantes via le client Rust/
  WASM et le SDK ; aucune signature, canonicalisation ou validation d'autorité en
  TypeScript.
- Afficher profil approuvé, scopes exacts, compte/workspace, état public, dernier
  refresh et besoin de réauthentification, sans coordonnées Vault.
- Ouvrir le navigateur système pour le consentement, reprendre par polling borné,
  puis activer seulement après discovery/manifest check.
- Ajouter sélection Google Picker pour le profil `drive.file` si ce choix produit
  est retenu.
- Supporter plusieurs comptes par des ids distincts et empêcher toute collision de
  callback, state, token ou surface d'outils.

**Gate navigateur :** aucun secret/token dans DOM, URL, storage, console ou trace ;
un connecteur cassé n'affecte pas son voisin.

### OAC-7 — exploitation et qualification de production

- Runbooks de création d'application OAuth `Internal`/`External`, redirect URIs,
  scopes, consent screen, rotation et révocation.
- Sauvegarde/restore Vault testée sans exporter de plaintext ; restart gateway et
  Vault dans les deux ordres.
- Quotas, timeouts, retry/backoff et `Retry-After` par provider ; aucune répétition
  automatique d'un effet non idempotent.
- Métriques sans cardinalité de compte ni payload ; alertes sur
  `reauth_required`, refresh failures et manifest drift.
- Gate staging avec comptes jetables, puis revue sécurité/verification fournisseur
  avant toute promotion. La gateway de démonstration locale actuelle n'est pas une
  preuve de production.

## 5. Ordre recommandé et dépendances

```text
OAC-0 contrats
  → OAC-1 socle OAuth moderne
    → OAC-2 Notion MCP live
    → OAC-3 profil Google
      → OAC-4 Sheets read
        → OAC-4 Sheets write borné
      → GSE-0/GSE-1 politique d'effet
        → OAC-5 Gmail send
  → OAC-6 dashboard multi-compte
  → OAC-7 staging et production
```

Notion et le profil Google peuvent être développés après OAC-1 sans partager de
fichiers d'implémentation provider. Les modifications du routeur, de
`upstream_oauth.rs`, de `config.rs` et des features centrales restent sous un seul
ownership à la fois.

## 6. Gates de vérification par lot

```sh
cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

Chaque lot réseau ajoute des tests sur sockets réelles avec doubles locaux. Les
tests CI n'utilisent jamais un token réel. Chaque fournisseur live possède un
runbook séparé, un compte de test, des scopes minimaux et un protocole de retrait.

Critères transverses :

- zéro sortie amont avant autorité + log durable ;
- zéro appel anonyme après erreur OAuth ;
- zéro secret/token dans les sorties et preuves ;
- refus des scopes réduits, élargis ou liés au mauvais compte ;
- refresh concurrent unique et rotation atomique ;
- old-or-new complet au crash, jamais de record partiel ;
- révocation effective avant l'appel suivant ;
- drift MCP ou extension non approuvée = outil absent.

## 7. Décisions produit à prendre avant les effets réels

Les lots OAC-0 à OAC-2 peuvent avancer avec les décisions existantes. Avant
Sheets write et Gmail Send, faire valider :

1. Google Workspace `Internal` pour la démo durable, ou application `External`
   avec processus de publication/verification ;
2. Sheets : `drive.file` + Picker recommandé, ou accès read à tous les Sheets ;
3. Gmail : compte expéditeur, destinataires/domaines de démo, quota et mécanisme
   d'approbation ;
4. politique de rétention de l'outbox chiffrée et destruction des tokens ;
5. identité fonctionnelle utilisée pour lier un compte OAuth à un principal
   Aithos et comportement lors d'un changement de compte.

## 8. État Git observé au handoff

L'audit du 2026-07-22 ne conclut pas à un workspace globalement propre :

| Dépôt | Branche / HEAD | État |
|---|---|---|
| `code/aithos-core` | `codex/publish-aithos-core-busl` / `1c11bb1` | 2 fichiers provider modifiés étrangers, répertoires de transfert et 7 docs non suivis ; branche 48 commits devant `origin/main` |
| `provider` | `feat/p6-p7-tunnel` / `5536840` | propre |
| `code/aithos-client` | `codex/client-sdk-v2-parking` / `e082ca6` | 11 fichiers modifiés |
| `code/aithos-sdk` | `codex/g1-g7-enterprise-sdk` / `648e24b` | 7 chemins modifiés/non suivis |
| `code/aithos-sdk-example` | `codex/g1-g7-enterprise-dashboard` / `b1def67` | 6 chemins modifiés/non suivis |
| `landings` | `main` / `ba1afba` | propre et synchronisé avec son upstream |
| `marketing/landings/agent-native` | `main` / `7516f28` | propre |

Ne pas stasher, restaurer, formater globalement ni absorber ces changements. Avant
chaque lot, attribuer les fichiers concernés et n'indexer que le périmètre du lot.

## 9. Références fournisseur vérifiées

- Notion, connexion MCP et endpoint officiel :
  <https://developers.notion.com/guides/mcp/get-started-with-mcp>
- Notion, intégration d'un client MCP, discovery, DCR, PKCE et refresh :
  <https://developers.notion.com/guides/mcp/build-mcp-client>
- Google, OAuth web server et offline access :
  <https://developers.google.com/identity/protocols/oauth2/web-server>
- Google Sheets, scopes et recommandation `drive.file` :
  <https://developers.google.com/workspace/sheets/api/scopes>
- Gmail, scopes OAuth :
  <https://developers.google.com/workspace/gmail/api/auth/scopes>
- Gmail, `users.messages.send` :
  <https://developers.google.com/workspace/gmail/api/guides/sending>

## 10. Prompt de reprise

> Reprendre `code/aithos-core` sur la branche active sans la changer. Lire
> intégralement `docs/HANDOFF-GATEWAY-OAUTH-CONNECTEURS-SAAS-2026-07-22.md`,
> `docs/HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md`, les features
> `gateway-upstream-oauth.feature` et `gateway-connectors.feature`, puis
> `config.rs`, `upstream_oauth.rs`, `connectors.rs`, `control.rs` et
> `proxy_mcp.rs`. Préserver tout changement étranger. Commencer uniquement par
> OAC-0 : écrire les contrats RED pour discovery, client public/confidentiel,
> DCR/CIMD, paramètres Google typés, liaison de compte, réauthentification et
> isolation multi-compte. Ne contacter aucun fournisseur réel, ne modifier ni le
> Core ni la grammaire des mandats, et ne passer à OAC-1 qu'après revue des
> contrats et commit étroit.

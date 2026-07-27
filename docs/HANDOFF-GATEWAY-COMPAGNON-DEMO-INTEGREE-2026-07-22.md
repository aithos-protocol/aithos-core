# HANDOFF — Lot Gateway compagnon de la démo intégrée

Date : 2026-07-22

Statut réévalué le 22 juillet 2026 : **implémentation locale en cours de
qualification**. Le worktree contient désormais la liaison byte-exacte de
l'audience, le CORS fermé Gateway/Provider, leurs tests adversariaux et le bundle
`demo/integrated`. Les tests ciblés de session déléguée et de publication distante
sont verts. Restent avant clôture : revue/commit attribué de ce worktree, gate du
bundle et E2E navigateur live avec Google Sheets et secrets hors Git.

Ce handoff isole volontairement les changements Core/Gateway restants. Le
Client, le SDK et le dashboard ont avancé sans modifier le code Gateway afin de
ne pas chevaucher le lot OAuth parallèle.

## 1. Baselines après implémentation Client/SDK/dashboard

| Dépôt | Branche | HEAD |
| --- | --- | --- |
| `code/aithos-core` | `codex/publish-aithos-core-busl` | `d0f833e` avant ce document |
| `code/aithos-client` | `codex/client-sdk-v2-parking` | `2e5f549` |
| `code/aithos-sdk` | `codex/g1-g7-enterprise-sdk` | `9a8edd0` |
| `code/aithos-sdk-example` | `codex/g1-g7-enterprise-dashboard` | `9fe95ef` |

Package Client reconstruit :

```text
target/npm-artifacts/aithos-client-0.1.0-alpha.2-dev.tgz
sha256 329b15c778d2c4a0dfb464c2ed84461c8b69f5c47fcbd9aafb8cfe15b2005bf2
wasm   967281259bba49e5709a1eac1543f5fcef10184859a31890b0df6aba90e1661a
```

## 2. Livré hors Gateway

### Client

- `POST /control/v1/connectors/{id}/profile-stage`, JSON fermé et borné ;
- `POST /control/v1/connectors/{id}/disconnect`, corps strictement vide ;
- parité native/browser ;
- `client-secret` existant préservé.

### SDK

- `/authorize` aligné sur le vrai `PendingCeremonyView` public de la Gateway ;
- les bindings privés attendus sont conservés en mémoire puis comparés à
  `/ceremony/prepare` ;
- `profileStage`, `setClientSecret` et `disconnect` ;
- `startOAuth`, `activate` et `disconnect` transportent zéro octet ;
- types publics alignés.

### Dashboard

- un même objet OAuth est injecté dans G4 ;
- discovery, DCR, PKCE, parent G4, cérémonie et exchange sont enchaînés ;
- aucune saisie de transaction, audience, callback ou bearer ;
- bearer uniquement dans le client MCP en mémoire ;
- `tools/list` puis sélection dynamique de `read_range` ;
- appel Sheets réel, tentative voisine `write_range`, puis preuve Gamma locale ;
- logout abandonne OAuth, authorization, G4 et MCP et verrouille les handles.

## 3. P0 — isolation exacte de l'audience Gateway — implémentée localement

Le Client publie déjà `gateway_audience` dans les faits signés de l'opération
de grant (`aithos-client/src/publication.rs`). Aucun changement de wire ou de
grammaire de mandat n'est demandé.

Le défaut de baseline était que
`Runner::eligible_session_parents(delegate_pub, now)` filtrait la chaîne, la
révocation et `Issue`, sans consommer l'audience publiée ni la `resource` OAuth.
Le worktree courant fait désormais circuler la ressource dans la sélection,
extrait le fait `gateway_audience` signé et refuse l'absence, l'ambiguïté ou une
audience voisine. La couture Gateway appelle le front door Core délégué ; elle
ne remplace pas sa décision cryptographique.

Contrat obligatoire :

```text
parent publié avec gateway_audience = https://gateway-a.example/mcp
ceremony préparée sur resource      = https://gateway-b.example/mcp
=> parent absent de eligible_parents
```

Comportement désormais attendu et implémenté localement : la resource exacte
est rattachée au fait d'opération signé et vérifié, puis comparée
byte-exactement. Le navigateur n'est jamais la source d'autorité de cette valeur.

Tests présents à conserver au minimum :

- test positif audience identique ;
- test négatif A/B ;
- test fait absent, altéré ou ambigu => parent inéligible ;
- régression multi-contexte et révocation.

## 4. P0 — CORS exact pour le parcours navigateur — implémenté localement

Le défaut de baseline était limité au control plane et aux GET publics. Le
worktree courant applique maintenant une allowlist exacte aux routes
OAuth/cérémonie/MCP et aux publications Provider signées. Les paragraphes
suivants restent le contrat de non-régression de cette implémentation.

### Gateway OAuth et cérémonie

Surfaces : discovery protected-resource, discovery AS, `register`, `authorize`
JSON, `token`, `ceremony/prepare`, `prepare-grant`, `complete` et `cancel`.

Headers réellement utilisés : `Accept`, `Content-Type`. Les réponses doivent
porter l'ACAO exact, sans credentials, et `Vary: Origin` quand applicable.

### MCP

Headers réellement utilisés :

- `Authorization` ;
- `Content-Type` et `Accept` ;
- `MCP-Protocol-Version` ;
- `MCP-Session-Id` après initialisation.

Exposer au navigateur au minimum `MCP-Session-Id` et, sur les refus OAuth,
`WWW-Authenticate` si le SDK doit le lire.

### Provider signé

La publication utilise notamment :

- `Content-Type: application/octet-stream` ;
- `X-Aithos-Store` ;
- `X-Aithos-Auth` ;
- `If-Head`.

Les méthodes exactes doivent être dérivées du plan de publication réel. Exposer
les headers lus par le SDK, notamment `ETag` et `X-Aithos-Store`.

Pour toutes les surfaces :

- origine issue d'une allowlist configurée, jamais réfléchie aveuglément ;
- aucun `Access-Control-Allow-Credentials` ;
- méthodes/headers minimaux par route ;
- `OPTIONS` sans authentification, mutation, Vault, journal ou appel amont ;
- origine inconnue refusée sans ACAO ;
- aucun wildcard sur token, cérémonie, MCP ou publication signée ;
- comportement historique inchangé si la configuration dashboard est absente.

Une matrice de tests route × méthode × origine × headers est requise.

## 5. Bundle présent ; E2E live encore nécessaire

Le bundle sans secret `demo/integrated` et son preflight sont présents dans le
worktree. Aucun succès réel ne peut cependant être revendiqué avant revue de ce
lot et exécution avec une infrastructure et des credentials réels :

- Gateway production AS et état OAuth durable ;
- Provider, Vault et journal ;
- origine dashboard explicitement autorisée ;
- Ethos/contexte et profil Google Sheets activé ;
- credentials Google hors Git ;
- spreadsheet/range de démonstration read-only ;
- navigateur normal, sans extension ni désactivation de sécurité.

Le test final doit observer : publication Provider, OAuth entrant, cérémonie,
token en mémoire, `tools/list`, `read_range`, refus `write_range` sans hit Google,
preuve Gamma vérifiée et logout. Ajouter un scanner sentinelle sur URL, HTML,
console, localStorage, sessionStorage, IndexedDB et Cache Storage.

## 6. Preuves vertes reçues

- Client : `cargo test --workspace`, vert ;
- SDK : `npm test`, 28 tests verts ;
- Dashboard : `npm test`, 7 tests verts ;
- Dashboard : `npm run lint`, vert ;
- Dashboard : build Vinext/SSR vert.

`tsc --noEmit` reste bloqué uniquement par les types ambiants historiques
Cloudflare (`cloudflare:workers`, `Fetcher`, `D1Database`) ; aucune erreur de la
page intégrée n'a été rapportée avant ces erreurs globales.

## 7. Frontières

- ne pas réintroduire Gmail approvals ou Sheets write dans le beat principal ;
- ne pas modifier le wire Core, A.2, la canonicalisation ou les vecteurs ;
- ne pas renvoyer les credentials Google au dashboard ;
- ne pas journaliser code OAuth, verifier, state, bearer ou refresh token ;
- ne pas accepter une origine ou une audience par défaut permissif.

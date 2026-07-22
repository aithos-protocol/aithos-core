# HANDOFF — Lot Gateway compagnon de la démo intégrée

Date : 2026-07-22

Statut : **READY pour reprise par le développement Gateway en cours**.

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

## 3. P0 — isolation exacte de l'audience Gateway

Le Client publie déjà `gateway_audience` dans les faits signés de l'opération
de grant (`aithos-client/src/publication.rs`). Aucun changement de wire ou de
grammaire de mandat n'est demandé.

Aujourd'hui `Runner::eligible_session_parents(delegate_pub, now)` filtre la
chaîne, la révocation et `Issue`, mais ne consomme ni l'audience publiée ni la
`resource` OAuth. `oauth_ceremony_prepare` possède pourtant
`preparation.resource` avant d'appeler cette méthode.

Contrat obligatoire :

```text
parent publié avec gateway_audience = https://gateway-a.example/mcp
ceremony préparée sur resource      = https://gateway-b.example/mcp
=> parent absent de eligible_parents
```

Attendu : faire circuler la resource exacte jusqu'à la sélection, rattacher le
parent au fait d'opération signé et vérifié, puis comparer byte-exactement les
audiences. Ne pas faire confiance à une audience fournie par le navigateur et
ne pas élargir le mandat.

Ajouter au minimum :

- test positif audience identique ;
- test négatif A/B ;
- test fait absent, altéré ou ambigu => parent inéligible ;
- régression multi-contexte et révocation.

## 4. P0 — CORS exact pour le parcours navigateur

Le control plane a déjà une allowlist exacte. Les routes OAuth/cérémonie/MCP
n'en bénéficient pas et le Provider ne couvre que les GET publics anonymes.

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

## 5. Bundle et E2E encore nécessaires

Le code UI est intégré mais aucun succès réel ne peut être revendiqué avant le
lot Gateway/CORS et une infrastructure reproductible :

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


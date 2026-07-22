# Runbook — profils OAuth SaaS gouvernés

Ce runbook couvre le déploiement du socle livré dans `aithos-gateway` pour un
MCP OAuth, Google Sheets en lecture et Gmail `send_guarded`. Les profils sont
fermés, versionnés et **désactivés par défaut**. Les coordonnées Vault sont
dérivées par la gateway ; elles ne sont jamais acceptées depuis le dashboard.

## 1. Préconditions

- Une gateway multi-contexte avec `dashboard`, `journal` et un broker Vault KV
  v2 configurés.
- Une redirect URI HTTPS exacte pointant sur `/oauth/callback`.
- Un manifeste scellé dans le contexte owner. Le `manifest_pin` du profil doit
  être le digest de ce manifeste.
- Une application OAuth appartenant au client ou à son organisation.
- Pour Google en mode `Testing`, considérer les refresh tokens comme
  temporaires. Un gate durable demande une application Workspace `Internal` ou
  une application publiée selon les règles de l'organisation.

Ne jamais placer un client secret, token, verifier PKCE, `state`, corps de mail
ou coordonnées Vault dans le YAML, le navigateur ou Gamma.

## 2. Modèle de garde

Une instance reçoit un identifiant de compte opaque et six records distincts :

```text
connectors/<contexte>/p-<hash-principal>/<connecteur>/<compte>/
  registration
  client-secret
  pending
  token
  revocation
  outbox
```

`outbox` n'est utilisé que par Gmail. Son contenu pending est confié au broker
Vault, donc chiffré au repos par Vault. Après dispatch, expiration ou échec
ambigu, le contenu de revue est effacé et seul le digest/état demeure.

## 3. Fragments de profils

Les pins et identifiants ci-dessous sont des valeurs à remplacer. Un profil ne
doit passer à `enabled: true` qu'après scellement du manifeste et validation de
la configuration complète.

### Google Sheets lecture bornée

```yaml
connector_profiles:
  - id: google-sheets-read
    version: "1"
    enabled: false
    risk: read
    execution:
      kind: compiled_rest
      adapter: google_sheets_read
      api_base_url: https://sheets.googleapis.com/
      manifest_id: google-sheets-read
      manifest_pin: sha256:<DIGEST_MANIFESTE_SCELLE>
      settings:
        kind: google_sheets_read
        allowed_ranges:
          <SPREADSHEET_ID_DEMO>:
            - "Demo!A1:C20"
        max_response_bytes: 524288
    oauth:
      credential_broker: enterprise
      auth_url: https://accounts.google.com/o/oauth2/v2/auth
      token_url: https://oauth2.googleapis.com/token
      client_id: <CLIENT_ID_GOOGLE>
      scopes:
        - openid
        - email
        - https://www.googleapis.com/auth/spreadsheets.readonly
      redirect_uri: https://<GATEWAY>/oauth/callback
      client_authentication: client_secret_post
      registration:
        strategy: static
      authorization_parameters:
        access_type: offline
        include_granted_scopes: true
        prompt_consent_on_repair: true
      revocation_url: https://oauth2.googleapis.com/revoke
      account_binding:
        issuer: https://accounts.google.com
        source:
          kind: user_info
          endpoint: https://openidconnect.googleapis.com/v1/userinfo
        subject_field: sub
        account_field: email
```

Ce profil utilise `spreadsheets.readonly` et peut donc voir tous les Sheets du
compte au niveau OAuth ; l'adaptateur Aithos refuse néanmoins toute paire
spreadsheet/plage absente de l'allowlist. Le choix `drive.file` exige le parcours
Google Picker, qui reste un gate produit distinct.

Pour activer l'écriture, créer un **profil et un consentement séparés** avec
`risk: guarded_write`, le scope
`https://www.googleapis.com/auth/spreadsheets`, l'adaptateur
`google_sheets_write_guarded` et des settings fermés :

```yaml
execution:
  kind: compiled_rest
  adapter: google_sheets_write_guarded
  api_base_url: https://sheets.googleapis.com/
  manifest_id: google-sheets-write
  manifest_pin: sha256:<DIGEST_MANIFESTE_SCELLE>
  settings:
    kind: google_sheets_write_guarded
    allowed_ranges:
      <SPREADSHEET_ID_DEMO>:
        - "Demo!B2:C5"
    max_cells: 100
    max_request_bytes: 65536
```

`values_update_guarded` exige le digest BLAKE3 hexadécimal du payload canonique
`{spreadsheet_id, range, values}`. Il fait un unique `PUT` `RAW`, sans retry :
rejouer le même payload remplace la même plage par les mêmes valeurs et reste
idempotent. Une plage voisine, un digest différent, une cellule composite ou un
dépassement cellules/octets est refusé avant résolution OAuth.

### Gmail envoi gouverné

```yaml
  - id: gmail-send
    version: "1"
    enabled: false
    risk: guarded_write
    execution:
      kind: compiled_rest
      adapter: gmail_send_guarded
      api_base_url: https://gmail.googleapis.com/
      manifest_id: gmail-send
      manifest_pin: sha256:<DIGEST_MANIFESTE_SCELLE>
      settings:
        kind: gmail_send_guarded
        allowed_recipients:
          - demo@example.org
        allowed_domains: []
        max_recipients: 1
        max_subject_bytes: 200
        max_body_bytes: 65536
        approval_ttl_seconds: 900
    oauth:
      credential_broker: enterprise
      auth_url: https://accounts.google.com/o/oauth2/v2/auth
      token_url: https://oauth2.googleapis.com/token
      client_id: <CLIENT_ID_GOOGLE_GMAIL_DEDIE>
      scopes:
        - openid
        - email
        - https://www.googleapis.com/auth/gmail.send
      redirect_uri: https://<GATEWAY>/oauth/callback
      client_authentication: client_secret_post
      registration:
        strategy: static
      authorization_parameters:
        access_type: offline
        include_granted_scopes: true
        prompt_consent_on_repair: true
      revocation_url: https://oauth2.googleapis.com/revoke
      account_binding:
        issuer: https://accounts.google.com
        source:
          kind: user_info
          endpoint: https://openidconnect.googleapis.com/v1/userinfo
        subject_field: sub
        account_field: email
```

Le profil Gmail ne demande ni `gmail.readonly`, ni `gmail.modify`, ni
`gmail.compose`. Une requête agent crée seulement une approbation. L'envoi est
impossible tant qu'une autorité owner n'a pas approuvé puis dispatché le digest
immuable.

### MCP OAuth avec discovery

```yaml
  - id: notion-read
    version: "1"
    enabled: false
    risk: read
    execution:
      kind: mcp
      endpoint: https://mcp.notion.com/mcp
      manifest_id: notion-read
      manifest_pin: sha256:<DIGEST_MANIFESTE_SCELLE>
    oauth:
      credential_broker: enterprise
      client_id: ""
      auth_url: ""
      token_url: ""
      scopes:
        - <SCOPE_APPROUVE>
      redirect_uri: https://<GATEWAY>/oauth/callback
      endpoints:
        strategy: discovery
        protected_resource: https://mcp.notion.com/mcp
        issuer: <ISSUER_PINNE_APRES_REVUE_METADATA>
      client_authentication: client_secret_post
      registration:
        strategy: dynamic
```

L'issuer et les scopes doivent être obtenus par revue des métadonnées puis
pinnés dans le profil ; ne pas copier une valeur découverte directement dans un
profil actif sans approbation.

## 4. Parcours owner

Toutes les routes exigent l'autorité de configuration existante et utilisent le
principal vérifié pour l'isolation.

1. `POST /control/v1/connectors/{instance}/profile-stage` avec uniquement
   `v`, `id`, `context` et `{profile: {id, version}}`.
2. Pour un client confidentiel statique,
   `PUT /control/v1/connectors/{instance}/client-secret` une seule fois.
3. `POST /control/v1/connectors/{instance}/oauth/start`, puis ouvrir
   `authorization_url` dans le navigateur système.
4. Poll borné de `GET /control/v1/connectors/{instance}/oauth/status`.
5. `POST /control/v1/connectors/{instance}/activate` seulement lorsque l'état
   est `connected` ou `expired`. L'activation revalide le manifeste live MCP ou
   le catalogue compilé local.
6. Déconnexion par
   `POST /control/v1/connectors/{instance}/disconnect`. L'outil disparaît avant
   la révocation fournisseur. Un refus de révocation conserve les records
   nécessaires à un retry et publie uniquement un résidu expurgé.

Pour Gmail, le résultat agent `pending` contient un `approval_id` et un digest :

```text
GET  /control/v1/connectors/{id}/approvals/{approval_id}
POST /control/v1/connectors/{id}/approvals/{approval_id}/approve
POST /control/v1/connectors/{id}/approvals/{approval_id}/deny
POST /control/v1/connectors/{id}/approvals/{approval_id}/dispatch
```

La revue est la seule réponse owner contenant le destinataire, le sujet et le
corps. Elle est `no-store`. Les réponses agent/Gamma ne contiennent que digest,
état, expiration et éventuel `message_id`.

## 5. Qualification

Avant promotion :

```sh
cd rust
CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

Puis exécuter avec des comptes jetables : deux onboardings depuis Vault frais,
un restart gateway puis Vault dans chaque ordre, refresh, changement de compte,
révocation, manifest drift, plage Sheets voisine, Gmail refus/pending/
approve/dispatch unique et révocation entre approbation et dispatch.

Les canaries live ne sont pas remplaçables par les doubles CI. Ne promouvoir
aucun profil tant que les décisions Workspace Internal/External, Picker Sheets,
destinataires Gmail et rétention n'ont pas été validées par l'owner.

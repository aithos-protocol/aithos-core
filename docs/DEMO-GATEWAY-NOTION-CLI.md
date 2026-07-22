# Démo CLI Aithos Gateway avec le MCP hébergé Notion

État qualifié le 22 juillet 2026 sur le MCP officiel
`https://mcp.notion.com/mcp`.

Cette démo prouve le parcours suivant, sans client graphique Aithos :

1. discovery OAuth protégée et authorization server ;
2. Dynamic Client Registration public (`token_endpoint_auth_method=none`) ;
3. consentement humain avec PKCE S256 ;
4. jetons conservés uniquement dans Vault KV v2 ;
5. découverte authentifiée et pin du manifeste MCP ;
6. exposition locale de deux lectures Notion ;
7. appel réel `notion-fetch` ;
8. refus local de `notion-create-pages` ;
9. actes et refus signés dans Gamma, avec xref dans le journal.

## 1. Préconditions

- `vault` disponible localement ;
- Rust/Cargo ;
- un workspace Notion de démonstration ;
- le binaire `aithos-gateway` issu d'un commit contenant `b498a92` ou plus récent.

Notion ne fournit pas de scope OAuth read-only. Le consentement donne donc au MCP
hébergé les capacités permises par le compte. La restriction read-only de cette
démo est appliquée par les mandats et la politique Aithos. Utiliser un workspace
de test.

Si le disque système macOS est presque plein, placer les temporaires de Cargo sur
le volume de travail :

```sh
env TMPDIR=/volume/disposant/espace cargo build -p aithos-gateway --bin aithos-gateway
```

## 2. Vault de démonstration

Choisir un port loopback libre. `-dev-no-store-token` évite toute écriture dans
le token helper utilisateur :

```sh
vault server -dev -dev-no-store-token \
  -dev-listen-address=127.0.0.1:28201 \
  -dev-root-token-id='<JETON_EPHEMERE>'
```

Ce mode est en mémoire et strictement réservé à la démo. Une interruption du
processus efface DCR, PKCE et tokens ; il faut alors recommencer le consentement.

## 3. Configuration OAuth initiale

La configuration peut utiliser des stores FS vides pendant le consentement et
la discovery. Les quatre objets OAuth ont des coordonnées distinctes :

```yaml
listen: 127.0.0.1:4890

credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:28201
    mount: secret
    auth: { kind: token-env, env: AITHOS_NOTION_DEMO_VAULT_TOKEN }

servers:
  - name: notion
    transport: http
    url: https://mcp.notion.com/mcp
    oauth:
      scopes: []
      redirect_uri: http://127.0.0.1:4890/oauth/callback
      endpoints:
        strategy: discovery
        protected_resource: https://mcp.notion.com/mcp
        issuer: https://mcp.notion.com
      client_authentication: none
      registration:
        strategy: dynamic
        vault:
          broker: enterprise
          path: aithos/demo/notion/registration
          field: value
      pending_vault:
        broker: enterprise
        path: aithos/demo/notion/pending
        field: value
      token_vault:
        broker: enterprise
        path: aithos/demo/notion/token
        field: value

contexts:
  - name: notion-demo
    store: { kind: fs, root: /tmp/aithos-notion-demo/context }
    tools: {}

journal:
  store: { kind: fs, root: /tmp/aithos-notion-demo/journal }
```

## 4. Identité et Ethos locaux

```sh
aithos-gateway --identity /tmp/aithos-notion-demo/agent.id keygen

aithos-gateway owner-init-context \
  --master-seed-hex "$MASTER_SEED_HEX" \
  --label notion-demo \
  --store-root /tmp/aithos-notion-demo/context

aithos-gateway owner-init-journal \
  --master-seed-hex "$MASTER_SEED_HEX" \
  --agent-label notion-demo-agent \
  --agent-pub "$AGENT_PUB" \
  --gateway-pub "$GATEWAY_PUB" \
  --store-root /tmp/aithos-notion-demo/journal
```

Les graines passées sur la ligne de commande sont une facilité DEV uniquement.

## 5. Consentement Notion

```sh
env AITHOS_NOTION_DEMO_VAULT_TOKEN='<JETON_EPHEMERE>' \
  aithos-gateway --config /tmp/aithos-notion-demo/gateway.yaml \
  owner-connect-oauth --server notion --wait-secs 900
```

Ouvrir l'URL imprimée, sélectionner le workspace de démonstration, reconnaître
explicitement le callback loopback et continuer. Le succès attendu est :

```text
OAuth connection established for notion.
```

Ne jamais réutiliser une ancienne URL après la perte du Vault : le state PKCE
correspondant n'existe plus.

## 6. Discovery authentifiée et enrôlement

Omettre `--url` est volontaire : la CLI résout alors le serveur nommé depuis la
configuration et attache son jeton OAuth Vault.

```sh
env AITHOS_NOTION_DEMO_VAULT_TOKEN='<JETON_EPHEMERE>' \
  aithos-gateway --config /tmp/aithos-notion-demo/gateway.yaml \
  owner-discover-server \
  --server notion \
  --output /tmp/aithos-notion-demo/notion.json

jq -r '.tools[].name' /tmp/aithos-notion-demo/notion.json
```

Le canary du 22 juillet 2026 a découvert 20 outils. Le choix de démo est :

- `notion-fetch=read:granted` ;
- `notion-search=read:granted` ;
- tous les autres outils explicitement classés `write:denied`.

Chaque nouvelle surface ou dérive de schéma doit être revue ; ne pas générer une
approbation aveugle à partir des seuls noms.

Après `owner-enroll-server`, exposer seulement :

```yaml
tools:
  notion__notion-fetch:
    server: notion
    tool: notion-fetch
    access: read
    granted: true
  notion__notion-search:
    server: notion
    tool: notion-search
    access: read
    granted: true
  notion__notion-create-pages:
    server: notion
    tool: notion-create-pages
    access: write
    granted: false
```

Le troisième mapping sert uniquement à rendre le refus démontrable ; il n'est
pas publié dans `tools/list`.

## 7. Lancement et preuves CLI

```sh
env AITHOS_NOTION_DEMO_VAULT_TOKEN='<JETON_EPHEMERE>' \
  aithos-gateway \
  --config /tmp/aithos-notion-demo/gateway.yaml \
  --identity /tmp/aithos-notion-demo/agent.id \
  run
```

Liste gouvernée :

```sh
curl -sS http://127.0.0.1:4890/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  | jq '.result.tools[].name'
```

Appel réel en lecture :

```sh
curl -sS http://127.0.0.1:4890/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"notion__notion-fetch","arguments":{"id":"self"}}}' \
  | jq '{error,result}'
```

Refus d'écriture, sans appel provider :

```sh
curl -sS http://127.0.0.1:4890/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"notion__notion-create-pages","arguments":{}}}' \
  | jq '.error'
```

Preuves Gamma :

```sh
rg -n 'notion-fetch|notion-create-pages|mandate_denied' \
  /tmp/aithos-notion-demo/context/gamma \
  /tmp/aithos-notion-demo/journal/gamma
```

## 8. Résultat du canary qualifié

- DCR public Notion : réussi ;
- consentement et échange de code : réussis ;
- records Vault présents : `registration`, `pending`, `token` ;
- discovery MCP authentifiée : 20 outils ;
- `tools/list` Gateway : deux lectures Notion, plus les outils journal natifs ;
- `notion-fetch` sur l'identité du workspace : réussi ;
- `notion-search` : réussi ;
- `notion-create-pages` : refus `-32001`, `mandate_denied` ;
- Gamma contexte : action autorisée et refus signés ;
- Gamma journal : xref de l'action et miroir du refus.

# Démo gateway + coffre d'entreprise (HashiCorp Vault KV v2)

**But.** Reproduire à la main, avec un **vrai Vault**, ce que l'E2E durable
(`tests/e2e_vault.rs`) prouve déjà sans Docker : l'entreprise expose des MCP
derrière le gateway ; les tokens MCP vivent dans un coffre standard ; le
gateway les résout **par appel**, après mandat et log-before-relay ; l'agent,
le YAML, les stores et les journaux n'en voient jamais un octet ; toute panne
du coffre ferme la route.

> ⚠️ **Jamais production.** Vault tourne ici en mode `dev` : in-memory,
> auto-unsealed, sans TLS — explicitement interdit en production par
> HashiCorp. Toutes les valeurs de tokens sont **générées pour la démo** ;
> n'inscrire aucune valeur réelle dans ce document, dans Git, dans un
> handoff ou dans un argument CLI.

Prérequis : Docker, `cargo`, `curl`, `openssl` (ou tout générateur d'aléa).
Tout se passe en loopback. Quatre terminaux : T1 (Vault), T2 (MCP démo),
T3 (gateway), T4 (owner/agent).

Racine de travail jetable :

```bash
export DEMO=/tmp/aithos-vault-demo
mkdir -p "$DEMO" && cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
```

## 1. Lancer Vault Community (T1)

```bash
export AITHOS_VAULT_TOKEN=$(openssl rand -hex 16)   # token d'accès au coffre, DÉMO
docker run --rm --name aithos-vault-demo --cap-add=IPC_LOCK \
  -e VAULT_DEV_ROOT_TOKEN_ID="$AITHOS_VAULT_TOKEN" \
  -p 8200:8200 hashicorp/vault
```

Le mode dev monte automatiquement le moteur KV **v2** sur `secret/`.

## 2. Déposer les tokens MCP dans le coffre (T4)

```bash
export AITHOS_VAULT_TOKEN=<la valeur du T1>          # même shell owner
export GITHUB_MCP_TOKEN=$(openssl rand -hex 12)      # démo, non-production
export LINEAR_MCP_TOKEN=$(openssl rand -hex 12)

docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" -e VAULT_ADDR=http://127.0.0.1:8200 \
  aithos-vault-demo vault kv put -mount=secret aithos/mcp/github token="$GITHUB_MCP_TOKEN"
docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" -e VAULT_ADDR=http://127.0.0.1:8200 \
  aithos-vault-demo vault kv put -mount=secret aithos/mcp/linear token="$LINEAR_MCP_TOKEN"
```

## 3. Lancer deux MCP de démo (T2)

`examples/demo_mcp.rs` affiche l'`Authorization` reçue à CHAQUE requête —
c'est l'observation wire-side de la démo. `linear` **exige** son bearer
(401 sinon) ; `github` l'affiche seulement, pour montrer la rotation.

```bash
cargo run -p aithos-gateway --example demo_mcp -- \
  --port 9101 --name github --tools issues.list,issues.create &
cargo run -p aithos-gateway --example demo_mcp -- \
  --port 9102 --name linear --tools tickets.list --bearer "$LINEAR_MCP_TOKEN" &
```

## 4. Provisionner (T4, côté owner)

```bash
export MASTER=$(openssl rand -hex 32)                # graine maîtresse, DÉMO
BIN="cargo run -q -p aithos-gateway --"

$BIN --identity "$DEMO/agent.id" keygen
# noter agent_pub / gateway_pub imprimés :
export AGENT_PUB=<agent_pub>  GATEWAY_PUB=<gateway_pub>

$BIN owner-init-journal --master-seed-hex "$MASTER" --agent-label demo-agent \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" --store-root "$DEMO/journal"

$BIN owner-discover-server --server github --url http://127.0.0.1:9101/mcp \
  --output "$DEMO/github.json"
$BIN owner-discover-server --server linear --url http://127.0.0.1:9102/mcp \
  --output "$DEMO/linear.json"
# (linear exige son bearer : pour la discovery de démo, relancer T2/linear
#  sans --bearer le temps de l'enrollment, ou fournir l'accès owner autrement —
#  la discovery est un geste OWNER, hors du chemin gouverné.)

$BIN owner-init-context --master-seed-hex "$MASTER" --label customer-support \
  --store-root "$DEMO/support"
$BIN owner-enroll-server --master-seed-hex "$MASTER" --label customer-support \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --proposal "$DEMO/github.json" \
  --approve issues.list=read --approve issues.create=write \
  --store-root "$DEMO/support"

$BIN owner-init-context --master-seed-hex "$MASTER" --label operations \
  --store-root "$DEMO/operations"
$BIN owner-enroll-server --master-seed-hex "$MASTER" --label operations \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --proposal "$DEMO/linear.json" \
  --approve tickets.list=read \
  --store-root "$DEMO/operations"
# garder chaque auditor_seed_hex imprimée (montrée UNE fois).
```

## 5. La config ne contient que des références (T3)

```bash
cat > "$DEMO/gateway.yaml" <<'EOF'
listen: 127.0.0.1:4870
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
servers:
  - name: github
    transport: http
    url: http://127.0.0.1:9101/mcp
    credential: { broker: enterprise, path: aithos/mcp/github, field: token }
  - name: linear
    transport: http
    url: http://127.0.0.1:9102/mcp
    credential: { broker: enterprise, path: aithos/mcp/linear, field: token }
contexts:
  - name: customer-support
    store: { kind: fs, root: /tmp/aithos-vault-demo/support }
    tools:
      github__issues_list:   { server: github, tool: issues.list,   access: read }
      github__issues_create: { server: github, tool: issues.create, access: write }
  - name: operations
    store: { kind: fs, root: /tmp/aithos-vault-demo/operations }
    tools:
      linear__tickets_list: { server: linear, tool: tickets.list, access: read }
journal:
  store: { kind: fs, root: /tmp/aithos-vault-demo/journal }
EOF

grep -c "$GITHUB_MCP_TOKEN\|$LINEAR_MCP_TOKEN\|$AITHOS_VAULT_TOKEN" "$DEMO/gateway.yaml" || true
# → 0 : aucune valeur secrète dans le YAML.

AITHOS_VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
  cargo run -q -p aithos-gateway -- \
  --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" run
```

Le token du coffre n'entre QUE par la variable d'environnement du process
gateway — jamais en YAML, jamais en argument CLI.

## 6. Le parcours agent (T4)

```bash
MCP=http://127.0.0.1:4870/mcp
rpc() { curl -s "$MCP" -H 'content-type: application/json' -d "$1"; echo; }

# a. un endpoint unique, seuls les outils grantés sont listés
rpc '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'

# b. appel granté : T2 affiche authorization=Bearer <token sorti du coffre>
rpc '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"github__issues_list","arguments":{}}}'
rpc '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"linear__tickets_list","arguments":{}}}'
#    linear répond 200 : le bearer exigé était le bon.

# c. write connu mais non granté : refusé, ni Vault ni MCP touchés
rpc '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"github__issues_create","arguments":{"title":"x"}}}'

# d. panne du coffre : la route se ferme AVANT l'amont
docker stop aithos-vault-demo          # (T1 s'arrête)
rpc '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"github__issues_list","arguments":{}}}'
# → "credential unavailable", T2/github n'affiche RIEN de nouveau.

# e. rotation sans toucher au YAML ni redémarrer le gateway
#    (relancer T1 avec le MÊME AITHOS_VAULT_TOKEN, puis :)
export GITHUB_MCP_TOKEN_V2=$(openssl rand -hex 12)
docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" -e VAULT_ADDR=http://127.0.0.1:8200 \
  aithos-vault-demo vault kv put -mount=secret aithos/mcp/github token="$GITHUB_MCP_TOKEN_V2"
rpc '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"github__issues_list","arguments":{}}}'
# → T2/github affiche la NOUVELLE valeur ; gateway.yaml n'a pas bougé.
```

## 7. Preuves de non-fuite et audit (T4)

```bash
# aucune valeur secrète dans les stores, la config, les journaux :
grep -r "$GITHUB_MCP_TOKEN\|$GITHUB_MCP_TOKEN_V2\|$LINEAR_MCP_TOKEN\|$AITHOS_VAULT_TOKEN" \
  "$DEMO" && echo "FUITE" || echo "OK: zéro occurrence"

# l'audit par contexte fonctionne toujours :
cargo run -q -p aithos-gateway -- --config "$DEMO/gateway.yaml" \
  --identity "$DEMO/agent.id" audit-export \
  --auditor-seed-hex <auditor_seed_hex de customer-support> --context customer-support
```

Attendu : `OK: zéro occurrence` — les seules apparitions des tokens sont
wire-side (sortie stdout des MCP de démo, qui JOUENT l'amont) et dans le
coffre lui-même.

## 8. Nettoyage

```bash
docker stop aithos-vault-demo 2>/dev/null; rm -rf "$DEMO"
```

## Limites connues (assumées pour cette tranche)

- **TLS non compilé** : `reqwest` est construit sans backend TLS dans ce
  workspace — un Vault ou un MCP en `https://` passe la config (exigée hors
  loopback) mais échoue au premier appel. Pour une instance distante réelle,
  activer une feature TLS (`rustls-tls`) de `reqwest` — une ligne de Cargo,
  hors périmètre démo.
- **Auth Vault = token-env seulement** : AppRole/Kubernetes auth sont des
  adapters ultérieurs derrière le même `auth.kind`, non bloquants ici
  (l'API AppRole est prévue pour les workflows machine).
- **Discovery owner-side sans credential** : `owner-discover-server` parle à
  l'amont avec l'accès de l'OWNER (hors chemin gouverné). Un amont qui exige
  un bearer dès `tools/list` se discovery via un accès owner temporaire.
- **Mode `dev` de Vault** : in-memory, auto-unsealed, root token — outil de
  démo uniquement ; un déploiement réel utilise un Vault scellé/TLS et une
  policy minimale pour le token du gateway (jamais root).

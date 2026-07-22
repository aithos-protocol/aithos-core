# Démo générique — votre agent, vos MCP, le provider réel

> **Statut : DEV/LEGACY direct.** Ce runbook reste utile pour une répétition
> loopback à bearer statique. Il ne décrit ni le relay/TLS public, ni la
> cérémonie OAuth déléguée G4, ni l'OAuth amont moderne. Ne pas le présenter
> comme le parcours de production ; pour G4, utiliser
> `CLI-DELEGATED-OAUTH.md` et le handoff de délégation courant.

Date : 2026-07-21. Complète `DEMO-LEA-PROVIDER-CLI.md` (la répétition
scriptée) et `GUIDE-GATEWAY-DEMO-LOCALE.md` (gestion/dépannage) : ici,
**l'agent est un vrai agent** (Claude Code, Codex CLI, Cursor — tout
client MCP Streamable HTTP) et **les connecteurs sont les MCP de votre
choix**. La gateway reste locale, gouvernée par un Ethos, adossée au
provider réel (`store.aithos.fr`) : journal en mode B, contexte en
mode A.

Ce qui se démontre : l'agent voit EXACTEMENT la surface mandatée
(`tools/list` = les outils accordés, rien d'autre), chaque acte est
journalisé avant relais, chaque refus est pédagogique (`-32001`), les
credentials n'existent que dans Vault et ne transitent jamais par
l'agent, et toute l'histoire est publiquement prouvée (témoin).

---

## 1. Contraintes à connaître avant de choisir ses MCP

- **Transport HTTP obligatoire.** La gateway parle aux amonts en MCP
  Streamable HTTP (`transport: http`, une URL). Un serveur MCP *stdio*
  doit d'abord être exposé en HTTP (par son option native s'il en a
  une) — sinon, prends-en un autre pour la démo.
- **Credential = bearer statique (ou rien).** L'injection se fait par
  en-tête `Authorization: Bearer <secret>` résolu depuis Vault à
  l'appel. Un MCP à OAuth interactif (flux navigateur) n'est pas
  jouable aujourd'hui ; un MCP à token/PAT l'est.
- **Bornes sur champs de premier niveau.** Les `--bound` de
  l'enrôlement s'appliquent aux arguments plats de l'outil
  (`champ=one_of:...`, `forbid`, `max:`, `require`, `slots:`). Les
  outils à schémas imbriqués s'enrôlent très bien (le schéma est scellé
  tel quel) — mais sans borne fine sur les sous-champs.
- **Discovery authentifiée : contournement fourni** (§4). La commande
  `owner-discover-server` appelle `tools/list` sans credential ; si ton
  MCP exige le bearer même pour lister, passe par le mini-proxy local.
- **Agent local seulement.** Un agent qui tourne dans un cloud (Claude
  web/Cowork cloud) ne peut pas joindre `127.0.0.1` — l'entrée par
  `<org>.mcp.aithos.fr` attend le chantier G1. Pour cette démo :
  Claude Code CLI, Codex CLI, Cursor, ou tout client MCP sur la machine.

Exemples de MCP réels compatibles (à re-vérifier dans la doc du
fournisseur, les URLs bougent) : GitHub MCP hébergé
(`https://api.githubcopilot.com/mcp`, bearer = un PAT fine-grained en
lecture seule pour la démo) ; des MCP publics sans auth type DeepWiki
ou Context7 ; ou n'importe quel MCP HTTP interne à toi.

## 2. Session jetable

Comme la démo Léa (§0–1 du runbook Léa) : build des binaires, puis :

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
GW=target/debug/aithos-gateway
ADMIN=target/debug/aithos-store-admin

export DEMO=/tmp/aithos-demo-gen
mkdir -p "$DEMO"
export MASTER=$(openssl rand -hex 32)
export TENANT="demo-gen-$(date +%m%d%H%M)-$(openssl rand -hex 2)"
export AITHOS_VAULT_TOKEN=$(openssl rand -hex 16)
```

Pose ici les secrets de TES connecteurs (exemple avec un PAT GitHub) :

```bash
export GITHUB_PAT='github_pat_…'   # jamais dans git/yaml/captures
```

## 3. Vault dev et secrets (sauter si tous tes MCP sont sans auth)

Deux options équivalentes — un Vault en mode dev, non scellé, en
mémoire (tout disparaît à l'arrêt : c'est voulu pour la démo).

**Option A — Vault installé sur la machine** (`vault` dans le PATH) :

```bash
vault server -dev -dev-root-token-id="$AITHOS_VAULT_TOKEN" \
  -dev-listen-address=127.0.0.1:8200 &
export VAULT_ADDR=http://127.0.0.1:8200
export VAULT_TOKEN="$AITHOS_VAULT_TOKEN"

vkv() { vault kv put -mount=secret "$1" token="$2"; }
```

**Option B — Docker** :

```bash
docker run --rm -d --name aithos-demo-vault --cap-add=IPC_LOCK \
  -e VAULT_DEV_ROOT_TOKEN_ID="$AITHOS_VAULT_TOKEN" -p 8200:8200 hashicorp/vault

vkv() { docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
  -e VAULT_ADDR=http://127.0.0.1:8200 aithos-demo-vault \
  vault kv put -mount=secret "$1" token="$2"; }
```

Puis, dans les deux cas, une entrée par connecteur authentifié :

```bash
vkv aithos/mcp/github "$GITHUB_PAT"
# aithos/mcp/<nom> pour chaque connecteur
```

Contrôle (les deux options) : `curl -s http://127.0.0.1:8200/v1/sys/health | jq .sealed`
→ `false`. La gateway, elle, ne verra que `AITHOS_VAULT_TOKEN` par
l'environnement au `run` — le yaml ne porte que l'adresse et des refs.

## 4. Provisioning owner

Identité du pod + les deux Ethos (mêmes gestes que Léa) :

```bash
$GW --identity "$DEMO/agent.id" keygen | tee "$DEMO/keygen.out"
export AGENT_PUB=$(awk '/^agent_pub:/ {print $2}' "$DEMO/keygen.out")
export GATEWAY_PUB=$(awk '/^gateway_pub:/ {print $2}' "$DEMO/keygen.out")

$GW owner-init-journal --master-seed-hex "$MASTER" --agent-label agent \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --store-root "$DEMO/journal" | tee "$DEMO/journal.out"
export JOURNAL_DID=$(awk '/^journal_did:/ {print $2}' "$DEMO/journal.out")
export MEMORY_MANDATE=$(awk '/^memory_mandate:/ {print $2}' "$DEMO/journal.out")

$GW owner-init-context --master-seed-hex "$MASTER" --label travail \
  --store-root "$DEMO/travail" | tee "$DEMO/context.out"
export CONTEXT_DID=$(awk '/^context_did:/ {print $2}' "$DEMO/context.out")
```

**Discovery.** Si le MCP sert `tools/list` sans auth :

```bash
$GW owner-discover-server --server github \
  --url https://api.githubcopilot.com/mcp --output "$DEMO/github.json"
```

S'il exige le bearer même pour lister (cas GitHub) : mini-proxy local
qui ajoute l'en-tête, discovery à travers lui, puis on l'éteint —
le bearer ne touche jamais un fichier :

```bash
python3 - <<'EOF' &
import http.server, urllib.request, os, sys
UP = os.environ["DISCOVER_URL"]; TOK = os.environ["DISCOVER_BEARER"]
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        req = urllib.request.Request(UP, data=body, method="POST", headers={
            "Content-Type": "application/json", "Accept": "application/json, text/event-stream",
            "Authorization": f"Bearer {TOK}"})
        try:
            with urllib.request.urlopen(req, timeout=30) as r: out = r.read(); code = r.status
        except urllib.error.HTTPError as e: out = e.read(); code = e.code
        self.send_response(code); self.send_header("Content-Type", "application/json")
        self.end_headers(); self.wfile.write(out)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", 9309), H).serve_forever()
EOF
DISCOVER_URL=https://api.githubcopilot.com/mcp DISCOVER_BEARER="$GITHUB_PAT" \
  $GW owner-discover-server --server github \
  --url http://127.0.0.1:9309/ --output "$DEMO/github.json"
kill %1
```

(⚠ exporter `DISCOVER_URL`/`DISCOVER_BEARER` avant de lancer le proxy ;
consigné : un flag `--bearer-env` natif sur `owner-discover-server` est
le petit lot qui supprimera ce détour.)

**Enrôlement.** Ouvre `"$DEMO/github.json"`, choisis tes outils, puis
approuve EXPLICITEMENT chacun — c'est le geste de gouvernance :

```bash
$GW owner-enroll-server --master-seed-hex "$MASTER" --label travail \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --proposal "$DEMO/github.json" \
  --approve get_issue=read:granted \
  --approve list_issues=read:granted \
  --approve create_issue=write:granted \
  --approve delete_repo=write:denied \
  --store-root "$DEMO/travail" | tee "$DEMO/enroll.out"
export CONTEXT_MANDATE=$(awk '/^agent_mandate:/ {print $2}' "$DEMO/enroll.out")
export AUDITOR_SEED=$(awk '/^auditor_seed_hex:/ {print $2}' "$DEMO/enroll.out")
```

Adapte les noms d'outils à TON proposal (tout outil découvert non
approuvé est refusé d'office ; garde au moins un `write:denied` — c'est
lui qui fera le refus pédagogique de la démo). Ajoute des `--bound`
si l'outil a des champs plats bornables. Puis la directive :

```bash
$GW owner-grant-briefing --master-seed-hex "$MASTER" --label travail \
  --agent-pub "$AGENT_PUB" --store-root "$DEMO/travail"
$GW owner-set-briefing --master-seed-hex "$MASTER" --label travail \
  --zone circle --title 'Consigne' \
  --text 'Toujours répondre en français. Ne jamais créer d'"'"'issue sans étiquette demo.' \
  --store-root "$DEMO/travail"
```

## 5. Provider : tenant + seed (identique à Léa §5)

```bash
aws sso login --profile aithos-prod   # creds fraîches
export AITHOS_ADMIN_CONTROL_TABLE=aithos-provider-prod-control
export AITHOS_ADMIN_OBJECTS_BUCKET=aithos-provider-prod-store-data
export AITHOS_ADMIN_HEADS_TABLE=aithos-provider-prod-heads

$ADMIN create "$TENANT"
$ADMIN bind-did "$TENANT" "$JOURNAL_DID"
$ADMIN bind-did "$TENANT" "$CONTEXT_DID"

$GW owner-replicate-history --master-seed-hex "$MASTER" \
  --kind journal --label agent --store-root "$DEMO/journal" \
  --tenant "$TENANT" --url https://store.aithos.fr
$GW owner-replicate-history --master-seed-hex "$MASTER" \
  --kind context --label travail --store-root "$DEMO/travail" \
  --tenant "$TENANT" --url https://store.aithos.fr
```

## 6. Le yaml — neutre, sans secret

`demo-lea-render-config` est câblé sur la topologie Léa ; ici on écrit
le yaml à la main (même grammaire, vérifiée fail-closed au boot).
`$DEMO/gateway.yaml` :

```yaml
listen: 127.0.0.1:4890
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth: { kind: token-env, env: AITHOS_VAULT_TOKEN }
servers:
  - name: github
    transport: http
    url: https://api.githubcopilot.com/mcp
    credential: { broker: enterprise, path: aithos/mcp/github, field: token }
  # un serveur SANS auth : même bloc, sans `credential:`
contexts:
  - name: travail
    store:
      kind: replicated
      root: /tmp/aithos-demo-gen/travail
      url: https://store.aithos.fr
      tenant: "<TENANT>"
      did: "<CONTEXT_DID>"
      mandate: ["<CONTEXT_MANDATE>"]
    tools:
      github__get_issue:    { server: github, tool: get_issue,    access: read,  granted: true }
      github__list_issues:  { server: github, tool: list_issues,  access: read,  granted: true }
      github__create_issue: { server: github, tool: create_issue, access: write, granted: true }
      github__delete_repo:  { server: github, tool: delete_repo,  access: write, granted: false }
journal:
  store:
    kind: remote
    url: https://store.aithos.fr
    tenant: "<TENANT>"
    did: "<JOURNAL_DID>"
    mandate: ["<MEMORY_MANDATE>"]
    local: /tmp/aithos-demo-gen/journal
```

Remplace les `<…>` (`echo $TENANT $CONTEXT_DID …`), aligne `tools:` sur
tes approbations (un outil du yaml absent du manifest scellé = refus au
boot, c'est la garde). Contrôle : aucun secret —
`grep -c "$GITHUB_PAT\|$AITHOS_VAULT_TOKEN" $DEMO/gateway.yaml` → 0.

## 7. Lancer, vérifier, brancher l'agent

Terminal gateway :

```bash
AITHOS_VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
  $GW --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" run
```

Sonde à la main (avant tout agent) :

```bash
curl -s http://127.0.0.1:4890/mcp -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq '.result.tools[].name'
```

Attendu : tes outils accordés + `briefing.read`, `journal.search`,
`journal.write` — et RIEN d'autre (pas le `write:denied`).

**Claude Code** (le plus direct) :

```bash
claude mcp add --transport http aithos http://127.0.0.1:4890/mcp
claude   # puis : « quels outils as-tu via aithos ? »
```

**Codex CLI** : déclarer un serveur MCP streamable HTTP vers
`http://127.0.0.1:4890/mcp` dans sa config (`~/.codex/config.toml`,
section `mcp_servers` — selon la version, vérifier sa doc).
**Tout autre client MCP** : URL `http://127.0.0.1:4890/mcp`, transport
Streamable HTTP, pas d'auth (loopback — limite assumée, voir §10).

## 8. Le scénario de démo (l'agent aux commandes)

Demande à l'agent, dans l'ordre — chaque phrase déclenche un moment :

1. « Liste tes outils aithos » → la surface EST le mandat.
2. « Lis le briefing avant de travailler » → `briefing.read`, la
   directive de l'owner, et une entrée `ethos.read` au journal.
3. Une action ACCORDÉE (ex. « liste les issues du repo X ») → l'appel
   passe, le bearer est injecté par la gateway (l'agent ne l'a jamais
   vu), acte au gamma AVANT le relais.
4. Une action REFUSÉE (ex. « supprime le repo X ») → l'outil n'est
   même pas exposé ; si l'agent le force, `-32001` avec la raison.
5. Une action HORS BORNE si tu en as posé une → refus qui NOMME la
   borne et l'ensemble autorisé.
6. Édition à chaud : dans le terminal owner,
   `owner-set-briefing … --text '<nouvelle consigne>'`, puis redemande
   une lecture de briefing à l'agent → le nouveau texte, sans restart.
   Re-répliquer ensuite (`owner-replicate-history … --kind context …`).
7. La preuve : `audit-export --auditor-seed-hex "$AUDITOR_SEED"
   --context travail --kind action` (puis `--kind ethos.read`) — tout y
   est, y compris les refus.

## 9. Le deuxième écran — preuves provider (inchangé)

Comme `GUIDE-GATEWAY-DEMO-LOCALE.md` §3 : checkpoints du témoin sur
`$JOURNAL_DID`/`$CONTEXT_DID` (~6 s après chaque publication), surface
anonyme via `public.aithos.fr`. C'est la moitié « vous n'avez pas à
nous croire » de la démo.

## 10. Purge et limites

```bash
$ADMIN purge "$TENANT" --yes
# Vault : option A → kill %1  (le process `vault server -dev` en jobs)
#         option B → docker stop aithos-demo-vault
test "$DEMO" = /tmp/aithos-demo-gen && rm -rf -- "$DEMO"
```

Limites assumées de cette version : agent LOCAL uniquement (l'entrée
`<org>.mcp.aithos.fr` = chantier G1) ; endpoint gateway sans auth sur
loopback (l'OAuth de la gateway existe pour les hôtes tiers — hors
périmètre ici) ; MCP à OAuth interactif non jouables ; discovery
authentifiée via mini-proxy en attendant `--bearer-env` ; bornes sur
champs plats. Aucune de ces limites ne touche la chaîne de preuve.

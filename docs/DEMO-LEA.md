# Démo « Léa » — runbook jour J

> **Chemin provider + CLI courant :** voir
> [`DEMO-LEA-PROVIDER-CLI.md`](./DEMO-LEA-PROVIDER-CLI.md). Le présent
> document reste le runbook manuel détaillé et le repli 100 % local.

**But.** Dérouler à la main, en conditions réelles, ce que la répétition
générale automatisée prouve déjà deux fois : les 8 beats de
`DEMO-LEA-SCENARIO.md` §4 sont verts dans `gateway-demo-lea.feature`
(harness in-process, mocks wire) et dans `tests/e2e_demo_lea.rs` (vrai
binaire, faux Vault + trois faux MCP sur sockets). Ce runbook est le
troisième étage : **vrai Vault Docker, vrais connecteurs quand ils s'y
prêtent**, Cowork branché sur l'endpoint unique, et la checklist de ce
que Mathieu montre à l'écran, beat par beat.

> ⚠️ **Jamais production.** Vault en mode `dev` (in-memory, sans TLS,
> interdit en prod par HashiCorp). Toutes les valeurs de tokens de ce
> document sont **générées pour la démo** ; aucune valeur réelle dans
> Git, dans un handoff, ni en argument CLI. Gate explicite : une
> **répétition générale avec Mathieu en conditions réelles** précède le
> jour J (DEMO-LEA-SCENARIO §6.3).

Prérequis : Docker, `cargo`, `curl`, `openssl`, Node ≥ 18 (connecteur
Notion réel). Tout en loopback. Terminaux : T1 (Vault), T2 (les trois
MCP), T3 (gateway), T4 (owner/agent).

```bash
export DEMO=/tmp/aithos-lea-demo
mkdir -p "$DEMO" && cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
BIN="cargo run -q -p aithos-gateway --"
```

## 0. Choisir l'amont, beat par beat (état vérifié le 2026-07-15)

La démonstration n'exige PAS trois connecteurs réels : elle exige trois
amonts **séparés, pleine puissance, jamais bridés par eux-mêmes** — le
mur doit être le gateway. Trois options par serveur, du plus sûr au plus
spectaculaire ; les mocks `demo_mcp` restent le repli intégral (le
storyboard est identique, seuls les logos changent).

- **notion — connecteur réel recommandé : le serveur OFFICIEL
  self-hosted en HTTP.** `@notionhq/notion-mcp-server --transport http`
  sert du Streamable HTTP sur loopback et **exige un bearer statique**
  (`--auth-token`) : exactement la forme de notre coffre — le bearer va
  dans Vault, le token d'intégration Notion (`ntn_…`) reste dans l'env
  du process serveur, hors gateway. L'endpoint hébergé
  `https://mcp.notion.com/mcp` est lui en **OAuth PKCE uniquement**
  (pas de token statique) : hors v1.
- **gmail / calendar — deux voies réelles.**
  1. **Les MCP distants OFFICIELS Google** (nouveauté 2026, Developer
     Preview) : `https://gmailmcp.googleapis.com/mcp/v1` et
     `https://calendarmcp.googleapis.com/mcp/v1`, transport HTTP, OAuth
     2.0. Un **access token** de courte durée se dépose dans Vault pour
     la fenêtre de démo — et son expiration devient un ARGUMENT : la
     rotation KV en live (beat bonus) est le geste produit. Exige
     l'enrollment au Developer Preview + `gcloud services enable
     gmailmcp.googleapis.com calendarmcp.googleapis.com` + **TLS** (voir
     limites : feature `rustls-tls` d'une ligne, à activer pour tout
     endpoint distant).
  2. **Wrapper HTTP loopback communautaire** (ex.
     `taylorwilsdon/google_workspace_mcp` : `uv run main.py
     --single-user --transport streamable-http`) : credentials Google
     pré-autorisés côté process, endpoint HTTP local. L'amont reste
     pleine puissance ; notre wrapper stdio générique reste Phase D.
- **Vérifications AVANT de retenir un connecteur réel** (à faire en
  répétition, pas le jour J) :
  1. **Discovery propre** : `owner-discover-server` doit obtenir
     `tools/list` en un POST stateless. Un serveur qui exige
     `initialize` + `Mcp-Session-Id` avant `tools/list` n'est pas
     compatible avec nos upstreams v1 (stateless) → wrapper loopback ou
     repli mock pour CE serveur.
  2. **Champs bornables à la racine** : ouvrir le proposal JSON et
     vérifier que le champ à borner (`to`, `start`, …) est bien au
     premier niveau de l'`inputSchema` (bornes v1 = champs racine). Un
     schéma imbriqué → repli mock pour ce beat.
  3. **Noms réels** : les bounds et la config se déclarent sur les noms
     DÉCOUVERTS (ex. `send_message` plutôt que `send_email`) — adapter
     les commandes ci-dessous après discovery.

Le reste du runbook écrit le chemin **mocks** (reproductible partout,
zéro dépendance externe) et signale à chaque étape ce qui change avec un
connecteur réel.

## 1. Vault Community (T1)

```bash
export AITHOS_VAULT_TOKEN=$(openssl rand -hex 16)   # accès coffre, DÉMO
docker run --rm --name aithos-lea-vault --cap-add=IPC_LOCK \
  -e VAULT_DEV_ROOT_TOKEN_ID="$AITHOS_VAULT_TOKEN" \
  -p 8200:8200 hashicorp/vault
```

## 2. Un token PLEINE PUISSANCE par serveur, dans le coffre (T4)

```bash
export AITHOS_VAULT_TOKEN=<valeur du T1>
export NOTION_BEARER=$(openssl rand -hex 12)     # démo — ou --auth-token du serveur Notion réel
export GMAIL_BEARER=$(openssl rand -hex 12)      # démo — ou access token OAuth (voie officielle)
export CALENDAR_BEARER=$(openssl rand -hex 12)

vkv() { docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" -e VAULT_ADDR=http://127.0.0.1:8200 \
  aithos-lea-vault vault kv put -mount=secret "$1" token="$2"; }
vkv aithos/mcp/notion   "$NOTION_BEARER"
vkv aithos/mcp/gmail    "$GMAIL_BEARER"
vkv aithos/mcp/calendar "$CALENDAR_BEARER"
```

## 3. Trois MCP séparés, délibérément permissifs (T2)

Mocks : `demo_mcp` affiche l'`Authorization` reçue à chaque requête
(l'observation wire-side de la démo) et **exige** son bearer quand
`--bearer` est donné — l'amont authentifie, il ne restreint jamais.

```bash
cargo run -p aithos-gateway --example demo_mcp -- \
  --port 9201 --name notion --tools query_database,create_page \
  --bearer "$NOTION_BEARER" &
cargo run -p aithos-gateway --example demo_mcp -- \
  --port 9202 --name gmail --tools search_emails,send_email,delete_email \
  --bearer "$GMAIL_BEARER" &
cargo run -p aithos-gateway --example demo_mcp -- \
  --port 9203 --name calendar --tools list_events,create_event \
  --bearer "$CALENDAR_BEARER" &
```

Connecteur Notion réel à la place du mock 9201 :

```bash
NOTION_TOKEN=<ntn_… intégration> npx @notionhq/notion-mcp-server \
  --transport http --port 9201 --auth-token "$NOTION_BEARER"
```

> Les bornes du §5 se posent alors sur les noms d'outils RÉELS
> découverts (§0.3) ; la base « prospects » est une database Notion de
> démo garnie de 5 fiches.

## 4. Provisionner l'Ethos « ventes » (T4, côté owner)

```bash
export MASTER=$(openssl rand -hex 32)               # graine maîtresse, DÉMO
$BIN --identity "$DEMO/agent.id" keygen
export AGENT_PUB=<agent_pub>  GATEWAY_PUB=<gateway_pub>   # imprimés ci-dessus

$BIN owner-init-journal --master-seed-hex "$MASTER" --agent-label lea \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" --store-root "$DEMO/journal"

$BIN owner-discover-server --server notion   --url http://127.0.0.1:9201/mcp --output "$DEMO/notion.json"
$BIN owner-discover-server --server gmail    --url http://127.0.0.1:9202/mcp --output "$DEMO/gmail.json"
$BIN owner-discover-server --server calendar --url http://127.0.0.1:9203/mcp --output "$DEMO/calendar.json"
# (amont exigeant un bearer dès tools/list : discovery = geste OWNER —
#  relancer l'amont sans --bearer le temps de la capture, ou accès owner dédié.)

$BIN owner-init-context --master-seed-hex "$MASTER" --label ventes --store-root "$DEMO/ventes"
```

**L'enrollment est UN SEUL geste owner** : trois manifests, la table de
distribution complète (classe, décision, bornes), un seul mandat agent
couvrant l'union des outils grantés, un seul auditeur.

```bash
$BIN owner-enroll-server --master-seed-hex "$MASTER" --label ventes \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --proposal "$DEMO/notion.json" --proposal "$DEMO/gmail.json" --proposal "$DEMO/calendar.json" \
  --approve query_database=read:granted \
  --approve create_page=write:denied \
  --approve search_emails=read:granted \
  --approve send_email=write:granted \
  --approve delete_email=write:denied \
  --approve list_events=read:granted \
  --approve create_event=write:granted \
  --bound send_email:to=one_of:prospect-a@clients.example,prospect-b@clients.example,prospect-c@clients.example \
  --bound send_email:bcc=forbid \
  --bound send_email:to=max:3 \
  --bound send_email:subject=require \
  --bound create_event:start=slots:tue,thu@14:00-18:00 \
  --store-root "$DEMO/ventes"
# NOTER auditor_seed_hex (montrée UNE fois) — c'est le mandat du prestataire.
export AUDITOR_SEED=<auditor_seed_hex>
```

**Le caractère** : le pen briefing (geste séparé, révocable seul), la
consigne circle, la note self qui ne sortira jamais.

```bash
$BIN owner-grant-briefing --master-seed-hex "$MASTER" --label ventes \
  --agent-pub "$AGENT_PUB" --store-root "$DEMO/ventes"

$BIN owner-set-briefing --master-seed-hex "$MASTER" --label ventes \
  --zone circle --title "Consigne commerciale" \
  --text "Tout mail de prise de rendez-vous mentionne le DPE du bien et propose d'abord une visite virtuelle." \
  --store-root "$DEMO/ventes"

$BIN owner-set-briefing --master-seed-hex "$MASTER" --label ventes \
  --zone self --title "Note owner" \
  --text "Marge de négociation interne max 8% — owner only." \
  --store-root "$DEMO/ventes"
```

## 5. La config ne contient que des références (T3)

```bash
cat > "$DEMO/gateway.yaml" <<'EOF'
listen: 127.0.0.1:4890
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
servers:
  - name: notion
    transport: http
    url: http://127.0.0.1:9201/mcp
    credential: { broker: enterprise, path: aithos/mcp/notion, field: token }
  - name: gmail
    transport: http
    url: http://127.0.0.1:9202/mcp
    credential: { broker: enterprise, path: aithos/mcp/gmail, field: token }
  - name: calendar
    transport: http
    url: http://127.0.0.1:9203/mcp
    credential: { broker: enterprise, path: aithos/mcp/calendar, field: token }
contexts:
  - name: ventes
    store: { kind: fs, root: /tmp/aithos-lea-demo/ventes }
    tools:
      notion__query_database:  { server: notion,   tool: query_database, access: read,  granted: true }
      notion__create_page:     { server: notion,   tool: create_page,    access: write, granted: false }
      gmail__search_emails:    { server: gmail,    tool: search_emails,  access: read,  granted: true }
      gmail__send_email:       { server: gmail,    tool: send_email,     access: write, granted: true }
      gmail__delete_email:     { server: gmail,    tool: delete_email,   access: write, granted: false }
      calendar__list_events:   { server: calendar, tool: list_events,    access: read,  granted: true }
      calendar__create_event:  { server: calendar, tool: create_event,   access: write, granted: true }
journal:
  store: { kind: fs, root: /tmp/aithos-lea-demo/journal }
EOF

grep -c "$NOTION_BEARER\|$GMAIL_BEARER\|$CALENDAR_BEARER\|$AITHOS_VAULT_TOKEN" "$DEMO/gateway.yaml" || true
# → 0 : le YAML ne connaît AUCUN secret, et AUCUNE borne (politique scellée au manifeste).

AITHOS_VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
  cargo run -q -p aithos-gateway -- \
  --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" run
```

## 6. Brancher Léa (Cowork) sur l'endpoint unique

Le seul point de passage de Léa est `http://127.0.0.1:4890/mcp`
(Streamable HTTP, sans auth agent-side en v1 — assumé au scénario §2.3).
Dans le client MCP (Claude/Cowork, config MCP « HTTP ») :

```json
{
  "mcpServers": {
    "innoestate": { "type": "http", "url": "http://127.0.0.1:4890/mcp" }
  }
}
```

Léa reçoit alors `initialize.instructions` (« consulter `briefing.read`
avant toute action sortante ») et voit EXACTEMENT les outils grantés +
`briefing.read` + `journal.*`. Le prompt de démo lui demande simplement
d'organiser des visites avec les prospects de la base — tout le reste
est gouverné par le gateway. Répéter la formulation en répétition
générale : le refus pédagogique doit VISIBLEMENT la faire se corriger
(beats 3→4).

Pour le déroulé scripté (sans LLM), le harnais curl suffit :

```bash
MCP=http://127.0.0.1:4890/mcp
rpc() { curl -s "$MCP" -H 'content-type: application/json' -d "$1"; echo; }
```

## 7. La checklist des 8 beats (ce que Mathieu montre à l'écran)

1. **Surface exacte.** `rpc '{"jsonrpc":"2.0","id":1,"method":"initialize"}'`
   → montrer `instructions`. Puis `tools/list` → 5 outils grantés +
   `briefing.read` + `journal.write/search` ; `gmail__delete_email` et
   `notion__create_page` INVISIBLES. T1/T2 muets (zéro hit).
2. **La donnée vient de Notion.**
   `rpc '…"method":"tools/call","params":{"name":"notion__query_database","arguments":{}}…'`
   → la liste des prospects ; T2/notion affiche `authorization=Bearer
   <token du coffre>` — SON token, pas un autre. (Chemin mock :
   `demo_mcp` répond un texte neutre — la preuve du beat est le bearer
   wire-side + l'acte loggé ; les « 5 prospects » vivent dans le
   connecteur réel, où le mandat n'en autorise que 3.)
3. **Le mur qui enseigne.** Envoi aux 5 (`to` = a..e + `subject`) →
   refus pédagogique nommant `send_email.to`, les intrus d et e ET la
   liste approuvée. Montrer T1 (aucun hit gmail) et T2/gmail (muet).
4. **L'auto-correction.** Envoi à a, b, c → passe ; T2/gmail affiche UN
   appel, nom brut `send_email`, bearer du coffre.
5. **Les créneaux.** `create_event` mercredi 10:00 → refus nommant
   {tuesday, thursday 14:00–18:00} ; jeudi 15:00 → passe (T2/calendar :
   un appel, son bearer).
6. **Le caractère.** `briefing.read` → la consigne DPE/visite virtuelle
   exacte, étiquetée ventes/circle ; la note self n'apparaît NULLE PART.
   Montrer l'entrée de lecture dans le gamma (ou la garder pour le 8).
7. **Édition à chaud.** Dans T4, `owner-set-briefing --zone circle
   --text "… Joindre le lien du dossier de visite."` (texte complet
   ré-écrit) pendant que le gateway TOURNE → `briefing.read` suivant
   sert le nouveau texte. Zéro redémarrage.
8. **La preuve.** Le mandat d'auditeur rejoue tout :

   ```bash
   $BIN --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" audit-export \
     --auditor-seed-hex "$AUDITOR_SEED" --context ventes --kind action
   $BIN --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" audit-export \
     --auditor-seed-hex "$AUDITOR_SEED" --context ventes --kind ethos.read
   ```

   → les actes notion/gmail/calendar, les DEUX refus `bound_violated`
   avec leur détail pédagogique (`send_email.to`, `create_event.start`),
   les lectures de briefing — tout signé, chaîné. Une requête plus large
   (`--kind grant`) est REFUSÉE par le certificat. Puis le grep des
   sentinelles :

   ```bash
   grep -r "$NOTION_BEARER\|$GMAIL_BEARER\|$CALENDAR_BEARER\|$AITHOS_VAULT_TOKEN" \
     "$DEMO" && echo "FUITE" || echo "OK: zéro occurrence"
   ```

   **Bonus si le rythme le permet** : rotation live du token gmail
   (`vkv aithos/mcp/gmail <nouveau>`) → l'appel suivant porte la
   nouvelle valeur, YAML intact, gateway jamais redémarré.

## 8. Nettoyage

```bash
docker stop aithos-lea-vault 2>/dev/null; rm -rf "$DEMO"
```

## Limites connues (assumées pour cette tranche)

- **TLS non compilé** : `reqwest` sans backend TLS — tout endpoint réel
  en `https://` (MCP officiels Google, Vault distant) exige d'activer la
  feature `rustls-tls` de reqwest (une ligne de Cargo). Le chemin mock
  est 100 % loopback et n'en a pas besoin.
- **Upstreams v1 stateless** : pas de handshake `initialize` ni de
  `Mcp-Session-Id` vers l'amont — vérifier chaque connecteur réel en
  répétition (§0.3) ; incompatible → wrapper HTTP loopback, ou mock.
- **OAuth** : le gateway ne fait AUCUN flow OAuth en v1. Les endpoints
  OAuth-only (Notion hébergé) sont hors périmètre ; les access tokens
  courts (Google officiel) se déposent au coffre pour la fenêtre de
  démo, la rotation KV couvrant l'expiration.
- **Bornes v1 = champs racine** : vérifier les schémas découverts avant
  de promettre un beat sur un connecteur réel.
- **Auth agent-side du endpoint : aucune** (scénario §2.3, assumé v1) —
  loopback seulement.
- **Vault `dev`** : outil de démo ; un déploiement réel = Vault
  scellé/TLS + policy minimale (jamais root).

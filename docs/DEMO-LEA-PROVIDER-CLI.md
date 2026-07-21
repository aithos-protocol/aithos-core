# Démo Léa — provider réel, pilotage CLI

Ce runbook est le chemin reproductible recommandé avant la dashboard :
trois amonts MCP séparés et permissifs, Vault dev local, gateway local,
contexte `ventes` en mode A (fs primaire + réplication provider) et
journal en mode B (provider primaire). Toutes les données sont
synthétiques et le tenant est purgé en fin de répétition.

Les huit beats restent ceux de `DEMO-LEA-SCENARIO.md`. La CLI
`aithos-demo-lea` peut les jouer d'un bloc ou un par un (`--beat 1` à
`--beat 6`) ; les beats 7 et 8 restent volontairement des gestes owner
et auditeur visibles.

## 0. Construire une fois

Depuis `rust/` :

```bash
cargo build -p aithos-gateway --bins --examples
cargo build -p aithos-provider --bin aithos-store-admin

GW=target/debug/aithos-gateway
LEA=target/debug/aithos-demo-lea
MCP=target/debug/examples/demo_mcp
ADMIN=target/debug/aithos-store-admin
```

## 1. Session jetable

```bash
export DEMO=/tmp/aithos-lea-demo
mkdir -p "$DEMO"
export MASTER=$(openssl rand -hex 32)
export TENANT="demo-lea-$(date +%m%d%H%M)-$(openssl rand -hex 2)"
export AITHOS_VAULT_TOKEN=$(openssl rand -hex 16)
export NOTION_BEARER=$(openssl rand -hex 16)
export GMAIL_BEARER=$(openssl rand -hex 16)
export CALENDAR_BEARER=$(openssl rand -hex 16)
```

Valeurs de démonstration uniquement. Ne jamais copier ces variables dans
Git, YAML, un handoff ou une capture partagée.

## 2. Vault dev et secrets

Terminal Vault :

```bash
docker run --rm --name aithos-lea-vault --cap-add=IPC_LOCK \
  -e VAULT_DEV_ROOT_TOKEN_ID="$AITHOS_VAULT_TOKEN" \
  -p 8200:8200 hashicorp/vault
```

Terminal owner :

```bash
vkv() {
  docker exec -e VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
    -e VAULT_ADDR=http://127.0.0.1:8200 \
    aithos-lea-vault vault kv put -mount=secret "$1" token="$2"
}
vkv aithos/mcp/notion "$NOTION_BEARER"
vkv aithos/mcp/gmail "$GMAIL_BEARER"
vkv aithos/mcp/calendar "$CALENDAR_BEARER"
```

## 3. Trois MCP permissifs

Chaque processus protège les appels par son bearer, mais laisse
`tools/list` disponible pour le geste de discovery owner. Les bornes ne
viennent jamais de l'amont.

```bash
$MCP --port 9201 --name notion \
  --tools query_database,create_page \
  --bearer "$NOTION_BEARER" --allow-unauthenticated-tools-list \
  --response 'query_database=prospects: a, b, c, d, e'

$MCP --port 9202 --name gmail \
  --tools search_emails,send_email,delete_email \
  --bearer "$GMAIL_BEARER" --allow-unauthenticated-tools-list

$MCP --port 9203 --name calendar \
  --tools list_events,create_event \
  --bearer "$CALENDAR_BEARER" --allow-unauthenticated-tools-list
```

Les trois commandes vivent normalement dans trois terminaux afin que les
hits amont soient visibles pendant la présentation.

## 4. Provisionner les deux Ethos

```bash
$GW --identity "$DEMO/agent.id" keygen | tee "$DEMO/keygen.out"
export AGENT_PUB=$(awk '/^agent_pub:/ {print $2}' "$DEMO/keygen.out")
export GATEWAY_PUB=$(awk '/^gateway_pub:/ {print $2}' "$DEMO/keygen.out")

$GW owner-init-journal \
  --master-seed-hex "$MASTER" --agent-label lea \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --store-root "$DEMO/journal" | tee "$DEMO/journal.out"
export JOURNAL_DID=$(awk '/^journal_did:/ {print $2}' "$DEMO/journal.out")
export MEMORY_MANDATE=$(awk '/^memory_mandate:/ {print $2}' "$DEMO/journal.out")

$GW owner-init-context --master-seed-hex "$MASTER" --label ventes \
  --store-root "$DEMO/ventes" | tee "$DEMO/context.out"
export CONTEXT_DID=$(awk '/^context_did:/ {print $2}' "$DEMO/context.out")

$GW owner-discover-server --server notion \
  --url http://127.0.0.1:9201/mcp --output "$DEMO/notion.json"
$GW owner-discover-server --server gmail \
  --url http://127.0.0.1:9202/mcp --output "$DEMO/gmail.json"
$GW owner-discover-server --server calendar \
  --url http://127.0.0.1:9203/mcp --output "$DEMO/calendar.json"

$GW owner-enroll-server \
  --master-seed-hex "$MASTER" --label ventes \
  --agent-pub "$AGENT_PUB" --gateway-pub "$GATEWAY_PUB" \
  --proposal "$DEMO/notion.json" --proposal "$DEMO/gmail.json" \
  --proposal "$DEMO/calendar.json" \
  --approve query_database=read:granted \
  --approve create_page=write:denied \
  --approve search_emails=read:granted \
  --approve send_email=write:granted \
  --approve delete_email=write:denied \
  --approve list_events=read:granted \
  --approve create_event=write:granted \
  --bound send_email:to=one_of:a,b,c \
  --bound send_email:bcc=forbid \
  --bound send_email:to=max:3 \
  --bound send_email:subject=require \
  --bound create_event:start=slots:tue,thu@14:00-18:00 \
  --store-root "$DEMO/ventes" | tee "$DEMO/enroll.out"

export CONTEXT_MANDATE=$(awk '/^agent_mandate:/ {print $2}' "$DEMO/enroll.out")
export AUDITOR_SEED=$(awk '/^auditor_seed_hex:/ {print $2}' "$DEMO/enroll.out")

$GW owner-grant-briefing --master-seed-hex "$MASTER" --label ventes \
  --agent-pub "$AGENT_PUB" --store-root "$DEMO/ventes"
$GW owner-set-briefing --master-seed-hex "$MASTER" --label ventes \
  --zone circle --title 'Consigne commerciale' \
  --text "Tout mail de prise de rendez-vous mentionne le DPE du bien et propose d'abord une visite virtuelle." \
  --store-root "$DEMO/ventes"
$GW owner-set-briefing --master-seed-hex "$MASTER" --label ventes \
  --zone self --title 'Note owner' \
  --text 'Marge de négociation interne max 8% — owner only.' \
  --store-root "$DEMO/ventes"
```

## 5. Créer le tenant et seeder le provider

Rafraîchir `.aws-env` avant ce bloc. Puis :

```bash
source /Volumes/Math17/aithos/v2/.aws-env
export AITHOS_ADMIN_CONTROL_TABLE=aithos-provider-prod-control
export AITHOS_ADMIN_OBJECTS_BUCKET=aithos-provider-prod-store-data
export AITHOS_ADMIN_HEADS_TABLE=aithos-provider-prod-heads

$ADMIN create "$TENANT"
$ADMIN bind-did "$TENANT" "$JOURNAL_DID"
$ADMIN bind-did "$TENANT" "$CONTEXT_DID"

$GW owner-replicate-history --master-seed-hex "$MASTER" \
  --kind journal --label lea --store-root "$DEMO/journal" \
  --tenant "$TENANT" --url https://store.aithos.fr
$GW owner-replicate-history --master-seed-hex "$MASTER" \
  --kind context --label ventes --store-root "$DEMO/ventes" \
  --tenant "$TENANT" --url https://store.aithos.fr
```

`owner-replicate-history` est reprenable : un second appel saute les
objets identiques et ne publie que les éditions locales plus récentes.
Il refuse si le remote est en avance ou si le DID local ne correspond
pas au couple `kind`/`label`.

## 6. Générer la configuration sans secret

```bash
$GW demo-lea-render-config --output "$DEMO/gateway.yaml" \
  --tenant "$TENANT" \
  --context-root "$DEMO/ventes" --context-did "$CONTEXT_DID" \
  --context-mandate "$CONTEXT_MANDATE" \
  --journal-sidecar "$DEMO/journal" --journal-did "$JOURNAL_DID" \
  --journal-mandate "$MEMORY_MANDATE"

grep -c "$NOTION_BEARER\|$GMAIL_BEARER\|$CALENDAR_BEARER\|$AITHOS_VAULT_TOKEN" \
  "$DEMO/gateway.yaml" || true
# attendu : 0
```

## 7. Lancer le gateway et jouer les beats CLI

Terminal gateway :

```bash
AITHOS_VAULT_TOKEN="$AITHOS_VAULT_TOKEN" \
  $GW --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" run
```

Terminal démo, répétition compacte :

```bash
$LEA
```

Ou pas à pas :

```bash
$LEA --beat 1
$LEA --beat 2
$LEA --beat 3
$LEA --beat 4
$LEA --beat 5
$LEA --beat 6
```

## 8. Beat 7 — édition à chaud

```bash
$GW owner-set-briefing --master-seed-hex "$MASTER" --label ventes \
  --zone circle --title 'Consigne commerciale' \
  --text "Tout mail de prise de rendez-vous mentionne le DPE du bien, propose d'abord une visite virtuelle et joint le lien du dossier de visite." \
  --store-root "$DEMO/ventes"

$LEA --beat 6 --directive-contains 'lien du dossier'

# Reprendre la réplication owner : seules les nouveautés partent.
$GW owner-replicate-history --master-seed-hex "$MASTER" \
  --kind context --label ventes --store-root "$DEMO/ventes" \
  --tenant "$TENANT" --url https://store.aithos.fr
```

## 9. Beat 8 — preuve auditeur

```bash
$GW --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" audit-export \
  --auditor-seed-hex "$AUDITOR_SEED" --context ventes --kind action
$GW --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" audit-export \
  --auditor-seed-hex "$AUDITOR_SEED" --context ventes --kind ethos.read
```

La première sortie doit contenir les actes Notion/Gmail/Calendar et les
deux `bound_violated`. La seconde contient les lectures de briefing.

## 10. Nettoyage borné

```bash
$ADMIN purge "$TENANT" --yes
docker stop aithos-lea-vault
test "$DEMO" = /tmp/aithos-lea-demo && rm -rf -- "$DEMO"
```

Vérifier ensuite `control=0`, `heads=0` et le préfixe tenant S3 vide.

## Après la CLI — explicitement reporté

Ces points ne bloquent pas la démonstration CLI :

- dashboard `app.aithos.fr` : vue opérateur, tenants/quotas, volumes,
  fraîcheur witness, puis vue owner/auditeur avec vérification WASM ;
- remplacement progressif des mocks : Notion réel d'abord, puis
  Gmail/Calendar après validation TLS, OAuth, discovery stateless,
  schémas plats et noms d'outils réels ;
- authentification agent-side si l'endpoint quitte loopback ; Vault
  scellé/TLS et auth AppRole/Kubernetes ;
- quotas tenant, rate limits relay, GC/rétention, DR et DPA ;
- durcissements E1/E2, D3/D5/D6, re-dérivation du sidecar éphémère ;
- optimisation des gates officiels append, sync froid et CDN.

Règle de promotion : aucun de ces reports ne doit conduire à utiliser
des données client, des tokens longs ou un endpoint public non authentifié
pendant la démo CLI.

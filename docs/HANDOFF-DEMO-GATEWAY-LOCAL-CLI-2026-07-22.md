# HANDOFF — exécuter la démo CLI avec gateway locale

> **ARCHIVE D'EXÉCUTION.** Ce document décrit une répétition ponctuelle et un
> état de processus observé le 22 juillet ; il ne garantit pas qu'une Gateway
> écoute encore sur les ports indiqués. Utiliser les runbooks courants et refaire
> les probes avant toute répétition.

Date : 2026-07-22

Statut : **READY pour exécution accompagnée**, sans développement, déploiement,
publication, push ni merge.

## 0. But exact

Jouer depuis un autre contexte Codex la démo déjà disponible :

```text
CLI de pilotage aithos-gateway
        |
        v
gateway locale :4890 ---- Vault local :8200
        |                        |
        +---- MCP GitHub HTTPS --+
        |
        +---- RemoteStore provider prod : store.aithos.fr
```

Cette répétition montre une gateway réellement locale, un secret de connecteur
résolu dans Vault, une surface MCP mandatée, un appel sûr, un refus voisin, un
briefing et la preuve audit. Elle utilise la CLI du binaire
`aithos-gateway` ; elle ne doit pas toucher le chantier étranger
`rust/crates/aithos-cli`.

Clarification utilisateur du 2026-07-22 : le provider accessible est la prod et
c'est volontaire. Utiliser les services prod existants et, si une répétition
fraîche l'exige, un tenant jetable créé puis purgé suivant le runbook est admis.
Cela n'autorise aucun Terraform/apply, changement DNS manuel, image, déploiement,
publication, push ou merge.

## 1. Frontière : ne pas sur-vendre cette répétition

Le processus déjà actif est le parcours CLI historique **direct**. Sa config ne
contient ni stanza `relay` G1 ni stanza `dashboard` G7. Il prouve donc :

- gateway et Vault locaux ;
- vrai MCP GitHub HTTPS pré-approuvé ;
- vrai RemoteStore `store.aithos.fr` ;
- outils mandatés, refus avant amont et preuves.

Il ne prouve pas le tunnel G1, la terminaison TLS publique côté client, les
routes `/control/v1/**`, l'attachement à chaud G7 ni le dashboard navigateur.
Ne jamais présenter ce smoke comme le beat G1+G7 complet. Pour ce dernier,
reprendre le handoff G1+G7 principal et son ordre §8 ; ce document n'autorise
aucun correctif source opportuniste.

## 2. État disque observé — vérité au passage de relais

Worktree : `/Volumes/Math17/aithos/v2/code/aithos-core`

- branche : `codex/publish-aithos-core-busl` ;
- HEAD : `dcef190` (`docs(g1-g7): record client context binding stop`) ;
- branche observée `ahead 29` de `origin/main` ;
- changements préexistants étrangers : Docker, Cargo workspace/lock, crate
  `aithos-cli`, `store_admin.rs`, `cucumber_relay.rs`, répertoires de transfert
  et plusieurs documents non suivis ;
- ne rien restaurer, stasher, formater, indexer ou committer globalement ;
- ne pas intervenir dans `aithos-client`, `aithos-sdk` ou
  `aithos-sdk-example` : un autre contexte les traite.

Un build frais du HEAD a été produit dans une cible isolée, sans toucher
`rust/target` :

```text
/tmp/aithos-cli-gateway-demo-target-20260722/debug/aithos-gateway
  sha256 a94f2d448bf36044fc9c2ca5d81c86638560c16c0fe4ca21868fcb8bd03e2473
/tmp/aithos-cli-gateway-demo-target-20260722/debug/aithos-demo-lea
  sha256 9afbc33605e2ba289f58422604b7c221386c3276ac6497cf15a95d398711ef3c
/tmp/aithos-cli-gateway-demo-target-20260722/debug/examples/demo_mcp
  sha256 18ae0e3220ad5fcfa1ee6e2576f6f065ad3870cd43bb3e96c8b5268d6f955af5
```

Le binaire admin existant
`rust/target/debug/aithos-store-admin` précède la modification étrangère de sa
source et expose `create`, `bind-did`, `bind-gateway`, `unbind-gateway`,
`suspend`, `reactivate` et `purge`. Ne pas le rebâtir depuis le worktree sale
sans réattribuer d'abord le changement source.

## 3. Services déjà actifs — chemin recommandé immédiat

Observation du 2026-07-22 :

- Vault Community 2.0.3 écoute `127.0.0.1:8200`, initialisé et non scellé ;
- `aithos-gateway` écoute `127.0.0.1:4890` ;
- config : `/tmp/aithos-demo-gen/gateway.yaml` ;
- identité : `/tmp/aithos-demo-gen/agent.id`, mode `0600` ;
- contexte : `travail`, store répliqué vers `https://store.aithos.fr` ;
- journal : RemoteStore `https://store.aithos.fr` ;
- connecteur : GitHub MCP `https://api.githubcopilot.com/mcp` ;
- surface observée : `github__get_me`, `journal.write`, `journal.search`,
  `briefing.read` ;
- `github__delete_file` est connu mais non accordé et n'est pas listé.

Ces processus et `/tmp/aithos-demo-gen` sont hérités d'une répétition
précédente. Les préserver : ne pas les tuer, relancer, écraser, purger ou
supprimer sans constater leur propriété. En particulier, ne pas purger le tenant
référencé par cette config.

Attention : `enroll.out` contient une autorité auditeur et a été hérité en mode
`0644`. Ne jamais l'afficher, l'attacher ou le copier dans un handoff. Une
répétition fraîche doit commencer par `umask 077` afin que toutes les sorties
owner restent privées.

## 4. Lectures obligatoires du contexte d'exécution

Lire intégralement, dans cet ordre, avant le premier appel métier :

1. ce handoff ;
2. `docs/DEMO-GATEWAY-GENERIQUE.md` ;
3. `docs/GUIDE-GATEWAY-DEMO-LOCALE.md` ;
4. `docs/DEMO-LEA-PROVIDER-CLI.md` ;
5. `docs/SESSION-G1-G7-ENTERPRISE-2026-07-21.md` pour distinguer les preuves
   déjà livrées du smoke direct ;
6. seulement si un nouveau tenant prod est nécessaire :
   `docs/HANDOFF-PROVIDER-P7B-BASCULE-RELAY-DONE-2026-07-20.md` et les sections
   provider/control pertinentes de `docs/INFRA-PROVIDER.md`.

Les trois premiers documents de démo sont actuellement non suivis mais font
partie de l'état disque reçu. Ne pas les modifier ni les inclure dans un commit.

## 5. Préflight sans effet métier

Depuis `aithos-core/rust` :

```bash
set +x
umask 077

lsof -nP -iTCP:8200 -iTCP:4890 -sTCP:LISTEN

curl -sS --max-time 3 http://127.0.0.1:8200/v1/sys/health \
  | jq '{initialized,sealed,standby,version}'

MCP_URL=http://127.0.0.1:4890/mcp
curl -sS --max-time 10 "$MCP_URL" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
  | jq -e '[.result.tools[].name]'
```

Attendu : Vault `sealed=false` et les quatre outils listés au §3. Si le sandbox
Codex refuse le loopback alors que `lsof` montre les listeners, demander
l'exécution host/loopback ; ne pas conclure que les services sont arrêtés et ne
pas en lancer une seconde copie sur les mêmes ports.

STOP si la surface diffère, si la config ne pointe plus vers le tenant observé,
ou si un autre processus a repris les ports.

## 6. Déroulé de démonstration immédiat

Définir le petit harnais sans journaliser le shell :

```bash
set +x
MCP_URL=http://127.0.0.1:4890/mcp
rpc() {
  curl -sS --max-time 30 "$MCP_URL" \
    -H 'content-type: application/json' \
    -d "$1"
  printf '\n'
}
```

### Beat 1 — surface exacte

```bash
rpc '{"jsonrpc":"2.0","id":10,"method":"initialize"}' | jq .
rpc '{"jsonrpc":"2.0","id":11,"method":"tools/list"}' \
  | jq '[.result.tools[].name]'
```

Montrer que `github__delete_file` est absent et que le bearer GitHub n'apparaît
nulle part dans la réponse.

### Beat 2 — briefing gouverné

Cet appel crée volontairement une entrée `ethos.read` dans le journal prod :

```bash
rpc '{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"briefing.read","arguments":{"context":"travail"}}}' \
  | jq .
```

### Beat 3 — appel sûr mandaté

`get_me` est une lecture GitHub. Il résout le credential dans Vault, appelle le
MCP et journalise l'action avant l'appel amont :

```bash
rpc '{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"github__get_me","arguments":{}}}' \
  | jq .
```

Vérifier une réponse JSON-RPC sans secret. Ne lancer aucun autre outil GitHub.

### Beat 4 — voisin refusé avant Vault et amont

Forcer le nom connu mais non accordé :

```bash
rpc '{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"github__delete_file","arguments":{}}}' \
  | jq .
```

Attendu : refus stable ; aucun effet GitHub. Ne pas remplacer cet exemple par
un outil arbitraire ou un write réel.

### Beat 5 — preuve auditeur locale

Ne jamais imprimer la graine. Utiliser le binaire frais isolé et effacer la
variable dès la commande terminée :

```bash
set +x
DEMO=/tmp/aithos-demo-gen
GW=/tmp/aithos-cli-gateway-demo-target-20260722/debug/aithos-gateway
AUDITOR_SEED="$(awk '/^auditor_seed_hex:/ {print $2}' "$DEMO/enroll.out")"

"$GW" --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" \
  audit-export --auditor-seed-hex "$AUDITOR_SEED" \
  --context travail --kind action

"$GW" --config "$DEMO/gateway.yaml" --identity "$DEMO/agent.id" \
  audit-export --auditor-seed-hex "$AUDITOR_SEED" \
  --context travail --kind ethos.read

unset AUDITOR_SEED
```

Montrer les actes/refus et la lecture du briefing, pas la graine ni les fichiers
owner bruts. Si le format historique n'est pas accepté par le binaire courant,
STOP et documenter l'incompatibilité ; ne pas migrer ou réécrire les stores.

## 7. Contrôles de sortie

La répétition immédiate est réussie si :

- Vault reste non scellé et la gateway reste joignable ;
- `tools/list` contient exactement les quatre noms observés ;
- `briefing.read` rend uniquement la directive `travail` autorisée ;
- `github__get_me` passe ;
- `github__delete_file` est refusé sans effet amont ;
- `audit-export` vérifie les nouvelles entrées dans la portée auditeur ;
- aucun token, secret, seed, header `Authorization`, query OAuth ou chemin Vault
  n'apparaît dans les captures partagées ;
- `git status --short` est identique hors création de preuves explicitement
  demandées hors dépôt.

Ne pas scanner le contenu de Vault. Pour une preuve anti-fuite, chercher des
marqueurs sentinelles connus uniquement dans la config, les stores et les logs
de la répétition fraîche ; ne jamais placer la vraie valeur d'un secret dans une
commande susceptible d'être historisée ou affichée.

## 8. Reprise fraîche uniquement si l'état actif est inutilisable

Suivre `docs/DEMO-GATEWAY-GENERIQUE.md` puis
`docs/DEMO-LEA-PROVIDER-CLI.md` avec ces corrections de sûreté :

1. `umask 077`, `set +x` et un `mktemp -d` dédié ;
2. ports distincts des listeners hérités ;
3. build dans un nouveau `CARGO_TARGET_DIR` sous `/tmp`, avec `--locked` ;
4. credential de connecteur fourni dans l'environnement owner, écrit dans
   Vault, jamais dans YAML, fichier, handoff ou capture ;
5. credentials AWS uniquement dans le terminal admin, jamais sourcés dans le
   processus gateway ; profil/ressources prod du runbook ;
6. tenant au nom jetable unique, DIDs liés avant réplication ;
7. conserver le tenant et le chemin exacts dans des variables de session ;
8. à la fin, `aithos-store-admin purge "$TENANT" --yes`, vérifier le repos
   provider, arrêter seulement les PID créés par cette répétition et supprimer
   seulement le répertoire `mktemp` validé ;
9. ne jamais purger `demo-gen-07211548-0c57` ni supprimer
   `/tmp/aithos-demo-gen` : ils sont hérités.

Une répétition fraîche reste un parcours direct tant qu'aucune stanza `relay`
n'est configurée. L'ajout du relay/TLS/G7 n'est pas une opération de démo
improvisée : reprendre le lot live du handoff principal.

## 9. Conditions de STOP

STOP documenté, sans contournement, si :

- attribution incertaine d'un processus, tenant, secret ou artefact ;
- credentials prod expirés ou origine de credential inconnue ;
- divergence du tenant/DID/mandat ou RemoteStore en avance ;
- appel proposé hors `github__get_me` ou hors connecteur pré-approuvé ;
- besoin de modifier du code, Terraform, DNS, image ou grammaire protocolaire ;
- conflit avec le contexte client/SDK/dashboard parallèle ;
- fuite ou impression d'un secret/seed ;
- preuve auditeur invalide ou surface différente de celle attendue.

## 10. Prompt de reprise pour l'autre contexte

> Exécuter uniquement la démo CLI décrite dans
> `/Volumes/Math17/aithos/v2/code/aithos-core/docs/HANDOFF-DEMO-GATEWAY-LOCAL-CLI-2026-07-22.md`.
> Lire intégralement ce handoff et ses références obligatoires avant tout appel
> métier. État disque et processus = vérité ; préserver tous les changements,
> services et tenants hérités. Ne modifier aucun source et ne toucher ni
> `aithos-client`, ni `aithos-sdk`, ni `aithos-sdk-example`. Commencer par le
> préflight et utiliser la gateway/Vault déjà actifs sur `4890/8200` si leur
> identité et leur surface correspondent. Jouer surface, briefing, seul appel
> sûr `github__get_me`, refus voisin `github__delete_file`, puis preuve auditeur,
> sans jamais afficher de secret ou seed. Ne pas présenter ce parcours direct
> comme le beat G1+G7 complet. Si l'état hérité est inutilisable, une répétition
> fraîche contre le provider prod est admise uniquement avec tenant jetable,
> purge vérifiée et sans déploiement AWS. Toute divergence d'attribution,
> autorité ou périmètre impose STOP documenté.

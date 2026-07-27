# PROMPT DE REPRISE — Piste P / P7 : bascule control-plane (tenants réels)

> **ARCHIVE — ne pas exécuter.** La bascule P7 est close.

Colle ce prompt tel quel dans une nouvelle session. Dépôts :
`code/aithos-core` (+ `provider/` pour le Terraform). **État DISQUE =
vérité.** Rituel opposable : `provider/.claude/skills/rituel-tests/SKILL.md`.

## Contexte en 30 secondes

Le store Aithos (`https://store.aithos.fr`) tourne en prod sur Fargate avec
les backends durables de l'étape 6 (S3 `store-data` versionné + table
DynamoDB `heads`, CAS A.5, write-once ⑧b, cache A.6) — gate déployé JOUÉ et
VERT le 2026-07-20 : `HANDOFF-PROVIDER-P2-GATE6-DEPLOYE-DONE-2026-07-20.md`,
plan Terraform à 0 écart, `deployed-replay-etape6.py` 20/20 contre la prod.

**Le trou qui motive P7** (documenté et gravé, INFRA-PROVIDER §8 note
« RÉALISATION » du 2026-07-20) : le service lit ses tenants UNIQUEMENT
depuis un fichier bootstrap embarqué dans l'image (`control.rs`,
read-model statique). La prod est au repos sur `prod-none.json` (ZÉRO
tenant) : personne ne peut écrire. Il n'existe ni bin admin, ni lecture
DynamoDB des tenants, et `modules/control-plane-min` (table + policies,
fmt/validate verts depuis le 17/07) n'est PAS instancié dans `envs/prod`.
Le gate étape 6 a été joué avec un bootstrap minimal jetable — écart
assumé et gravé, la vraie mécanique est CE lot.

## Mission (P7 — control plane minimal, HANDOFF-PROVIDER-AWS lot P7)

Objectif de fin de lot : **un tenant réel se crée/suspend/purge par une
CLI d'admin (DynamoDB), le service le sert sans redéploiement, suspension
effective < 60 s** — et les bootstraps embarqués ne portent plus aucun
tenant en prod.

Ordre imposé (rituel) :

1. **Gate contrat d'abord — BDD AVANT le code.** Scénarios cucumber écrits
   et observés RED contre le binaire étape 6 avant toute implémentation.
   À couvrir au minimum : tenant actif/inconnu/suspendu via le backend
   control-plane ; `did_not_bound` ; propagation d'une suspension
   (< 60 s, borne testable par injection d'horloge/TTL) ; **fail-closed**
   (table injoignable → `503 unavailable`, JAMAIS un `unknown_tenant`
   inventé — pattern des seams étape 6) ; coexistence des backends
   (memory/bootstrap pour dev/tests, dynamodb en prod).
2. **Points d'arbitrage à trancher AU GATE CONTRAT (jamais unilatéraux)** :
   - schéma de la table (control-plane-min : clé `t` seule aujourd'hui —
     suffit-elle pour `(tenant, dids[], suspended)` + mappings tunnel B.2,
     ou faut-il un range key ?) ;
   - fraîcheur : lecture directe à chaque requête vs cache TTL court —
     la promesse « suspension < 60 s » est la borne, le choix est
     l'arbitrage ;
   - sort du fichier bootstrap : `AITHOS_STORE_BOOTSTRAP` reste REQUIRED
     aujourd'hui (binaire fail-closed) — garde-t-on un bootstrap «
     coquille » en prod ou l'env devient-il optionnel quand le backend
     control est dynamodb ?
   - périmètre relay : les mappings tunnel B.2 basculent-ils aussi (le
     relay a son propre bootstrap) ou restent-ils hors lot ?
   - wire : AUCUN code d'erreur nouveau attendu (`unknown_tenant`,
     `did_not_bound`, `suspended` existent en A.7) — toute dérive = STOP
     et arbitrage.
3. **Implémentation** : backend DynamoDB derrière les MÊMES lookups de
   `control.rs` (`tenant_state`, `did_bound`, `resolve_tunnel`) ; env
   twelve-factor cohérent avec l'existant (`AITHOS_STORE_CONTROL_BACKEND`
   memory|dynamodb + `AITHOS_STORE_CONTROL_TABLE`, défaut memory — une
   ancienne task def boote le nouveau binaire) ; **bin admin**
   (`create` / `bind-did` / `suspend` / `reactivate` / `purge` →
   DynamoDB, creds admin de l'opérateur, jamais dans l'image, purge =
   le runbook GC §8 : versions S3 du préfixe + item heads + item tenant).
4. **Terraform** : `control-plane-min` instancié dans `envs/prod` ;
   task role store + policy LECTURE seule sur LA table (moindre
   privilège, pattern task_data) ; policies admin séparées (jamais la
   task). `fmt` + `validate` verts. AUCUN plan/apply sans Mathieu.
5. **Batterie de non-régression** (avant de déclarer vert local) :
   cucumber complet, `red-replay-p7.py` 15/15, `red-replay-p9.py` 33/33,
   `cargo test --features pod-stub`, core+bundle `--locked`, clippy
   `-D warnings`, fmt. Vecteurs p1..p9 GELÉS — aucun octet ne bouge.
6. **STOP au gate déployé.** Session dédiée avec creds : plan lu
   intégralement, apply, tenant réel créé par le bin admin, rejeu
   `python3 vectors/deployed-replay-etape6.py https://store.aithos.fr
   <tenant-créé>` (20/20 attendu), preuve suspension < 60 s contre la
   prod, purge outillée, gravures éventuelles sur GO.

## Interdits (opposables)

Le commit est le geste de Mathieu. Aucune gravure INFRA-PROVIDER sans GO
explicite. Aucun apply sans plan lu et GO. Les vecteurs gelés ne se
modifient jamais. Fail-closed partout : un backend muet refuse, il
n'invente rien. `.aws-env` ne se charge que sur délégation explicite.

## Se lit avec

`HANDOFF-PROVIDER-P2-GATE6-DEPLOYE-DONE-2026-07-20.md` (état déployé,
consignations §3 — dont : `refuse()` sans `Cache-Control` à porter à CE
gate contrat si tu veux le trancher ; churn Terraform du module relay),
`INFRA-PROVIDER.md` (§8 notes du 20/07, annexes A normatives),
`HANDOFF-PROVIDER-AWS.md` (lot P7 + « gate déploiement non fait » du
17/07), `control.rs` (le read-model à faire basculer),
`modules/control-plane-min` (la table qui attend).

## Environnement (état au 2026-07-20, à re-vérifier en début de session)

- VM Cowork device MORTE (`device_bash` refuse) ; staging des `.feature`
  refusé (HTTP 400) — **demander à Mathieu un tarball frais du dépôt en
  début de session** (`cd /Volumes/Math17/aithos/v2/code/aithos-core &&
  tar czf ../../_transfer/aithos-core-disk-$(date +%Y%m%d-%H%M).tgz
  --exclude rust/target --exclude 'rust/target-*' .`) puis overlay par
  mtime pour ce qui bouge ensuite ; `device_commit_files` écrit tout
  (y compris les `.feature`) — le write-back disque marche.
- AWS depuis le sandbox : `pip install awscli boto3 --break-system-packages`,
  Terraform à télécharger (releases.hashicorp.com OK), docker daemon KO —
  l'image se pousse par l'API ECR (voir le handoff gate 6, digests et
  méthode). Le MCP aws-api (device) était HS (token SSO absent) — les
  creds passent par `.aws-env` rafraîchi par Mathieu (SSO ~1 h,
  demander le refresh au moment des applies), copie sandbox purgée en
  fin de session.
- Python du harnais : `pip install pynacl base58 blake3 --break-system-packages`.
- État AWS de départ : cluster `aithos-provider-prod`, service store
  task def :6 (durable, `prod-none.json`, desired_count 2 — passé par
  `-var`, PAS figé dans envs/prod : un apply sans la var redescend à 1,
  à figer ou re-passer), tables `nonces`/`relay-nonces`/`heads`, bucket
  `store-data`, état TF `provider/envs/prod/terraform.tfstate` sur
  `aithos-landings-tfstate-128066560720`, vars GitHub
  `placeholder/aithos-provider` + `placeholder/aithos-core`.

Commence par : vérifier l'état du disque (les commits gate 6 de Mathieu
sont-ils faits ?), reconstruire le sandbox, rejouer la batterie de
non-régression, PUIS ouvrir le gate contrat P7 (scénarios RED). STOP à
chaque gate.

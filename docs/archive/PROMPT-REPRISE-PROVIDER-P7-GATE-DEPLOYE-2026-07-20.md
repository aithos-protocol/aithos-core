# PROMPT REPRISE — Piste P / P7 : GATE DÉPLOYÉ (2026-07-20)

> **ARCHIVE — ne pas exécuter.** Le gate déployé est terminé.

Date : 2026-07-20 (préparé en session, après-midi). Dépôts :
`code/aithos-core` + `provider/`. État DISQUE = vérité. Mission : jouer le
gate déployé du lot P7 (bascule control-plane DynamoDB) — plan lu
INTÉGRALEMENT, apply sur GO Mathieu uniquement, preuves déployées,
gravures sur GO explicite. Rien d'autre : AUCUN code nouveau (INFRA-PROVIDER
§8, A.7).

Se lit avec `HANDOFF-PROVIDER-P7-VERT-LOCAL-2026-07-20.md` (l'état
d'entrée : lot VERT LOCAL, arbitrages tranchés),
`HANDOFF-PROVIDER-P2-GATE6-DEPLOYE-DONE-2026-07-20.md` (les gestes du
gate précédent : image par API ECR, plans lus, purge),
`PROMPT-REPRISE-PROVIDER-P7-BASCULE-2026-07-20.md` (la mission d'origine),
`provider/.claude/skills/rituel-tests/SKILL.md`.

## 0. Corroboration de préparation (session du 20/07, sandbox cloud)

Sandbox reconstruit depuis le disque (tarball 08:03 + overlay par mtime,
staging fichier à fichier — VM device morte, pas de `device_bash`). Sur
cet état = le disque post-write-back P7 :

| Preuve rejouée en préparation | Résultat |
|---|---|
| `cargo check --locked -p aithos-provider --all-targets` | vert (le code P7 compile, cucumber.rs inclus) |
| build release musl `aithos-store-api` + `aithos-store-admin` | 2 binaires static-pie ; CLI admin conforme (`create\|bind-did\|suspend\|reactivate\|purge --yes`) |
| unités lib | **50/50** (dont les 4 CachedControl) |
| `vectors_replay` vs binaire reconstruit | **5/5** (p1 octet-exact, p1 sémantique, p2, **p7**, **p9** wire-exact — p9 exige les vecteurs postérieurs au tarball, stagés depuis le disque) |
| garde 1 : `dynamodb` + bootstrap à tenants | exit 2, message fail-closed exact |
| garde 2 : `memory` sans bootstrap | exit 2 |
| garde 3 : `dynamodb` + coquille zéro-tenant + table injoignable | boote, healthz 200, wire → **503 `unavailable` + `Cache-Control: no-store`** (jamais un `unknown_tenant` inventé) |
| `terraform fmt -check` + `validate` (envs/prod, TF 1.13.5) | verts |
| behave `control-p7.feature` keyless vs binaire local | **2/2 GREEN, 3 deploy-gate SKIP** (bruyants) |

Réserve reconduite : les `.feature` sous
`rust/crates/aithos-provider/tests/features/store/` refusent toujours le
staging (HTTP 400) — cucumber n'a PAS été rejoué en sandbox (le VERT
LOCAL 134/134 reste la preuve de la session P7 ; le job `test` de
`provider-image.yml` rejouera `cargo test -p aithos-provider` en CI au
push, features du dépôt incluses). Les `.feature` de `provider/e2e/`
stagent, eux, normalement.

## 1. Préalables (Mathieu, avant la session gate)

1. **Commits** — les sessions n'ont touché QUE les chemins du §2 du
   handoff P7 :
   ```sh
   cd code/aithos-core
   git add rust/crates/aithos-provider rust/Cargo.toml rust/Cargo.lock docs
   git commit -m "P7 bascule control-plane: seam ControlStore (memory|dynamodb, cache TTL 30 s, fail-closed 503 no-store), bin aithos-store-admin, gate contrat 15 scénarios RED→GREEN, cucumber 134/134"
   cd ../../provider
   git add infra/terraform/envs/prod infra/terraform/modules/store-api e2e/features
   git commit -m "P7: control-plane-min instancié dans envs/prod; store-api env exclusif bootstrap XOR control + attach reader; e2e control-p7 (keyless + deploy-gate)"
   ```
   (`Cargo.lock` inchangé — l'add est sans effet, sans danger.)
   Un push sur master déclenche `provider-image.yml` : le job `test`
   (non-régression features en CI) est utile ; le job `push` échouera sur
   les vars GitHub `placeholder/*` — attendu, l'image reste le geste de
   la session par l'API ECR.
2. **`.aws-env` rafraîchi** (SSO ~1 h — celui de 11:43 est expiré) au
   moment d'ouvrir la session gate, pas avant.

## 2. Séquence du gate (STOP à chaque plan, apply sur GO)

1. **Sandbox** : reconstruire comme en §0 (ou reprendre la session de
   préparation si encore vivante — binaires musl et Terraform déjà en
   place). Rejouer p7/p9 contre le binaire reconstruit.
2. **Décision au gate — les bootstraps et l'image** : sous `dynamodb` la
   task def ne porte plus AUCUN chemin bootstrap ; l'image peut donc
   (a) garder `prod-none.json`/`prod-replay-20260720.json` embarqués
   (inertes, churn Dockerfile nul) ou (b) les sortir (image = binaire +
   CA bundle seuls). Trancher AVANT l'assemblage, consigner.
3. **Image `:prod`** : assembler la couche unique (binaire musl
   static-pie + `ca-certificates.crt` [+ bootstraps selon ②]) et pousser
   `:prod` + `:<sha>` par l'API ECR (docker daemon absent — précédent
   gate 6, digests au handoff). NE PAS forcer de redéploiement : la
   révision de task def de l'apply fera l'unique rollout.
4. **Plan n°1 — la bascule** :
   ```sh
   terraform plan -var image_tag=prod -var desired_count=2 -out p7.tfplan | tee plan-p7-1.txt
   ```
   Lu INTÉGRALEMENT. Attendu, RIEN d'autre :
   - **+4** : `aws_dynamodb_table` control (single-table pk/sk),
     `aws_iam_policy` admin, `aws_iam_policy` reader (module
     control-plane-min), attach `task_control_reader` (reader SEULE sur
     le task role — l'admin ne s'attache jamais) ;
   - **~** : révision task def store — l'env PERD
     `AITHOS_STORE_BOOTSTRAP` et GAGNE
     `AITHOS_STORE_CONTROL_BACKEND=dynamodb` +
     `AITHOS_STORE_CONTROL_TABLE=<table>` (PAS de TTL : défaut binaire
     30 s) ; service store re-pointé ;
   - churn relay connu (réécritures à l'identique + révision task def
     relay, coupure brève des tunnels — consigné gate 6) ;
   - AUCUN destroy hors anciennes révisions de task def ; ni S3, ni
     heads, ni SG, ni DNS.
   **STOP → GO Mathieu → apply.** `desired_count=2` toujours par `-var`
   (non figé — consigné gate 6).
5. **Wire post-bascule (keyless)** : healthz 200, rollout COMPLETED ;
   `GET /t/nobody-was-ever-here/<did>/did.json` → **404
   `unknown_tenant` + `Cache-Control: no-store`** (la table répond, la
   coquille ne fabrique rien) ; `acme` → `unknown_tenant` (le bootstrap
   ne nourrit plus le wire).
6. **Tenant de preuve (CLI admin, creds OPÉRATEUR de la session — jamais
   dans l'image)** :
   ```sh
   export AITHOS_ADMIN_CONTROL_TABLE=<table>   # + AITHOS_ADMIN_HEADS_TABLE, AITHOS_ADMIN_OBJECTS_BUCKET (purge)
   aithos-store-admin create <tenant>           # grammaire A.1 vérifiée à l'entrée
   aithos-store-admin bind-did <tenant> <did>   # exige le meta
   ```
7. **Preuves déployées** :
   - `python3 vectors/deployed-replay-etape6.py https://store.aithos.fr
     <tenant>` → **20/20** (re-runnable, reprend la tête courante) ;
   - `E2E_BASE_URL=https://store.aithos.fr E2E_CONTROL_TENANT=<tenant>
     E2E_ADMIN_CMD="aithos-store-admin" python3 -m behave e2e/features`
     — suite COMPLÈTE (store-p1, store-acme-p6, relay-p6, control-p7),
     les lots précédents ne régressent pas ; la **suspension < 60 s** se
     mesure du RETOUR de la commande admin au premier flip sur le wire
     (suspension ET réactivation — les négatifs cachés propagent aussi) ;
   - vérif admin : items control exacts (tenant#/meta, tenant#/did#,
     gateway#/meta si B.5 exercé).
8. **Purge outillée** : `aithos-store-admin purge <tenant> --yes` —
   versions S3 du préfixe (+ delete markers) → items heads → lignes
   control EN DERNIER (un purge interrompu laisse le tenant refusant).
   Wire final : `<tenant>` → `unknown_tenant`. **Plan
   `-detailed-exitcode` (mêmes -var) = 0 écart.**
9. **Gravures INFRA-PROVIDER (§7/§8) sur GO explicite uniquement** :
   bascule réalisée (la table = seule source de tenants), cache 30 s
   (borne < 60 s), `no-store` sur tous les refus (tranché gate 6, pinné
   P7), relay HORS LOT (garde `relay.json`, petit lot suivant).
10. **Hygiène** : purger la copie sandbox de `.aws-env` ; handoff
    GATE-DEPLOYE-DONE ; les commits de clôture restent le geste de
    Mathieu.

## 3. Environnement (constaté en préparation)

VM device morte (`device_bash` refuse — staging/commit fichier à fichier
seulement) ; staging refusé (HTTP 400) sur les `.feature` de
`rust/.../tests/features/store/` uniquement ; tarball 13:14 (438 Mo) au-
dessus de la limite de staging — l'assemblage passe par le tarball 08:03
(8,6 Mo) + overlay par mtime, p9 et `deployed-replay-etape6.py` stagés à
part. Sandbox de préparation : rustc 1.95 + cible musl, Terraform 1.13.5,
boto3 + behave installés, docker daemon ABSENT (image par API ECR),
aws-api MCP historiquement inutilisable (token SSO device) — tout AWS par
CLI/boto3 sandbox sur les creds de session.

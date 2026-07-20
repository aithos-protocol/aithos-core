# HANDOFF — Piste P / P7 bascule control-plane : GATE DÉPLOYÉ — FAIT (2026-07-20)

Date : 2026-07-20 (fin d'après-midi). Dépôts : `code/aithos-core` +
`provider/`. État DISQUE = vérité. Statut : **le gate déployé P7 est JOUÉ
et VERT** — la table DynamoDB `aithos-provider-prod-control` est la SEULE
source de tenants en prod, la task def store (`:7`) ne porte plus aucun
bootstrap, l'image `:prod` n'embarque plus que le binaire et le CA bundle,
un tenant réel s'est créé/suspendu/réactivé/purgé par `aithos-store-admin`
avec **suspension propagée en 0,8 s** (borne < 60 s), plan final à
**0 écart**, les 2 gravures INFRA-PROVIDER écrites (GO Mathieu en
session). Le commit reste TON geste.

Se lit avec `HANDOFF-PROVIDER-P7-VERT-LOCAL-2026-07-20.md` (l'état
d'entrée), `PROMPT-REPRISE-PROVIDER-P7-GATE-DEPLOYE-2026-07-20.md` (la
mission jouée), `INFRA-PROVIDER.md` (2 notes gravées ce jour : §7 «
BASCULE RÉALISÉE », §8 « la mécanique CLI est LIVE »).

## 0. Séquence de la session (tout GO'é par Mathieu, dans l'ordre)

1. **Sandbox reconstruit** (tarball 08:03 + overlay par mtime + provider/
   stagé fichier à fichier ; les 3 `.feature` store post-tarball refusent
   toujours le staging HTTP 400 — cucumber non rejoué, le VERT LOCAL
   134/134 + la CI au push restent la preuve). **Batterie d'entrée
   GREEN** : cargo check `--locked`, unités 50/50, binaires musl
   static-pie (api + admin, CLI conforme), vectors_replay 5/5, gardes
   fail-closed 1-2-3 (exit 2 / exit 2 / healthz 200 + wire 503
   `unavailable` + `no-store`), terraform fmt + validate, behave keyless
   2/2 + 3 SKIP bruyants.
2. **Décision ② tranchée (AskUserQuestion)** : les bootstraps SORTENT de
   l'image — `:prod` = binaire + CA bundle seuls. Caveat consigné : un
   retour vers une task def bootstrap (type `:6`) repasse par l'image du
   gate étape 6 épinglée par digest (`sha256:187cee4c…aeec3`), le tag
   `:prod` ne la porte plus. `docker/store-api.Dockerfile` mis à jour.
3. **Image poussée** par l'API ECR (docker daemon absent — méthode gate
   6, couche unique déterministe) : digest
   `sha256:fa01790110843400f8e92733cbf46e0d69038b0f9bd4e9ebbe78a9705b0ea1f1`
   (layer gz 7,6 Mo). AUCUN redéploiement forcé — la révision de task def
   de l'apply a fait l'unique rollout. Le tag `:<sha>` attend ton commit
   (voir §4).
4. **Dérive consignée et GO'ée : DEUX applies au lieu d'un.** Terraform
   refuse le plan unique (« Invalid count argument » : le `count` de
   l'attach reader exige la policy déjà existante — le précédent M2
   n'avait pas le cas, la policy acme préexistait). Zéro code modifié ;
   workaround standard `-target` :
   - **Plan n°0** (`plan-p7-0.txt`, `-target=module.control_plane`) lu
     INTÉGRALEMENT : **+3** (table control `pk`/`sk` PITR, policy admin,
     policy reader), 0 change, 0 destroy. Apply → 3 added.
   - **Plan n°1** (`plan-p7-1.txt`) lu INTÉGRALEMENT : **3 add / 6
     change / 2 destroy** — task def store remplacée (l'env PERD
     `AITHOS_STORE_BOOTSTRAP`, GAGNE `AITHOS_STORE_CONTROL_BACKEND=dynamodb`
     + `AITHOS_STORE_CONTROL_TABLE=aithos-provider-prod-control`, PAS de
     var TTL — défaut binaire 30 s), service re-pointé, attach
     `task_control_reader[0]` créé (reader SEULE — l'admin ne s'attache
     jamais), churn relay connu (réécritures à l'identique + révision
     task def, coupure brève des tunnels), AUCUN destroy hors les 2
     anciennes révisions. NB : `-var terraform_state_bucket_name=
     aithos-landings-tfstate-128066560720` est REQUISE pour un plan
     propre (sinon le plan retire les statements d'état du rôle CI plan
     — constaté et neutralisé en session). Apply → rollout **COMPLETED**,
     task defs store `:7` + relay `:7`, 2/2 tâches.
5. **Wire post-bascule (keyless)** : healthz 200 ;
   `nobody-was-ever-here` → **404 `unknown_tenant` + `Cache-Control:
   no-store`** (la table répond, la coquille n'invente rien) ; `acme` →
   `unknown_tenant` (le bootstrap ne nourrit plus le wire).
6. **Tenant de preuve** : `aithos-store-admin create replay-p7-20260720`
   + `bind-did` (grammaire A.1 vérifiée à l'entrée ; items control
   exacts : `tenant#…/meta {s: false}` + `tenant#…/did#…`). Servi par le
   service SANS redéploiement.
7. **Preuves déployées** : `deployed-replay-etape6.py https://store.aithos.fr
   replay-p7-20260720` → **20/20 GREEN** (genèse par le wire, ⑧b, CAS
   A.5, classes A.6). Suite behave COMPLÈTE : **store-p1 ✓,
   store-acme-p6 ✓, control-p7 5/5 ✓** — la suspension mesurée du RETOUR
   de la commande admin au premier flip : **0,8 s** ; réactivation
   **0,5 s** (borne 60 s, TTL 30 s). Deux consignations §2.
8. **Purge outillée** : `purge replay-p7-20260720 --yes` (5 versions S3 →
   1 item heads → 2 lignes control EN DERNIER) + `purge acme --yes`
   (recréé pour la preuve store-p1, voir §2). Wire final : les deux →
   `unknown_tenant`. **Table control : 0 item** (l'état de repos P7 : la
   table vide EST la coquille). Plan final `-detailed-exitcode` (mêmes
   `-var`) = **0 écart**.
9. **Gravures** (GO explicite) : 2 notes dans `INFRA-PROVIDER.md` — §7
   « control plane : BASCULE RÉALISÉE » (table seule source, cache 30 s,
   fail-closed no-store, relay HORS LOT), §8 « tenant de rejeu : la
   mécanique CLI est LIVE » (purge = runbook GC outillé, no-store sur
   tous les refus pinné).

## 1. Preuves (contre la PROD, 2026-07-20)

| Preuve | Résultat |
|---|---|
| batterie d'entrée sandbox (check, unités, vectors_replay, gardes, fmt/validate, behave keyless) | verte (§0.1) |
| plans lus avant chaque apply | 3/3 (`plan-p7-0`, `plan-p7-1`, final), fichiers livrés |
| rollout post-apply | COMPLETED, store `:7` + relay `:7`, 2/2 tâches, healthz 200 |
| wire keyless post-bascule | `unknown_tenant` + `no-store` ; acme retiré ✓ |
| tenant CLI servi sans redéploiement | ✓ (create + bind-did → wire 200) |
| deployed-replay (tenant CLI) | **20/20 GREEN** |
| behave suite complète | store-p1 ✓, store-acme-p6 ✓, **control-p7 5/5** ✓ (relay-p6 : §2) |
| suspension / réactivation < 60 s | **0,8 s / 0,5 s** (premier flip, wire réel) |
| items control | exacts (meta + did#) ; `gateway#` non exercé (B.5 relay hors lot) |
| purge outillée (×2 tenants) | S3 0 version, heads 0, control 0 item ; wire `unknown_tenant` |
| plan final | **0 écart** (`-detailed-exitcode` = 0) |

## 2. Consigné SANS graver (observations de session)

- **relay-p6 non rejouable depuis CE sandbox** : le proxy egress du
  sandbox cloud intercepte le TLS (issuer « Anthropic Egress Gateway »,
  ALPN `aithos-tunnel/1` strippé) — les 4 sondes d'enregistrement
  errorent (« 400 Bad Request » émis par le proxy) et les 2 sondes
  TCP/joignabilité échouent. Le relay prod est SAIN (rollout COMPLETED,
  logs `eof`/`no_tunnel` normaux). Aux gates M1/M2 ces sondes passaient
  par la VM device — morte aujourd'hui. À rejouer depuis un réseau non
  intercepté (VM device revenue, ou machine Mathieu :
  `python3 e2e/tools/relay-register.py ok`).
- **store-p1 exige le tenant fixture `acme`** : au repos zéro-tenant
  (vrai depuis gate 6), 4 scénarios répondaient `unknown_tenant`. GO
  Mathieu : `acme` recréé par la CLI + genèse wire du did.json fixture
  (matériel public committé), suite verte, puis PURGÉ avec le tenant de
  preuve. Pas une régression du code P7 — mais toute future passe de la
  suite complète au repos devra refaire ce geste (ou arbitrer un
  `E2E_FIXTURE_TENANT` optionnel au prochain gate contrat).
- **Course de précondition behave (2 tâches × cache 30 s)** : les `Given`
  de suspension/réactivation font UN seul GET derrière l'ALB — juste
  après un flip, l'autre tâche peut encore servir l'ancien état (observé
  1 fois : run vert au rejeu après convergence des caches). Flake de
  harnais, pas de violation de borne ; retouche possible (poll du Given)
  à un prochain gate contrat, features gelées d'ici là.
- **`-var terraform_state_bucket_name` obligatoire** pour tout
  plan/apply propre d'`envs/prod` (sinon diff parasite sur
  `aws_iam_role_policy.plan`). Passée en session ; à ajouter au README
  d'envs/prod un jour (micro-choix, pas fait unilatéralement).
- **La dérive deux-applies est un one-shot** : l'attach reader existe
  désormais dans l'état — les prochains plans redeviennent uniques.

## 3. Livré (fichiers, write-back disque)

- `code/aithos-core/docs/INFRA-PROVIDER.md` : 2 notes gravées (§7, §8).
- `code/aithos-core/docs/HANDOFF-PROVIDER-P7-GATE-DEPLOYE-DONE-2026-07-20.md`
  (ce fichier).
- `code/aithos-core/docker/store-api.Dockerfile` : bootstraps sortis de
  l'image (décision ② P7, commentaire + digest de repli).
- `provider/infra/terraform/envs/prod/plan-p7-0.txt`, `plan-p7-1.txt`,
  `plan-p7-final.txt` : les plans lus, tels qu'appliqués.

## 4. Reste pour clore (Mathieu)

1. **Commits** — les deux blocs du §1 du prompt de reprise (toujours pas
   faits à la fin de session), en Y AJOUTANT les fichiers du §3 :
   ```sh
   cd code/aithos-core
   git add rust/crates/aithos-provider rust/Cargo.toml rust/Cargo.lock docs docker
   git commit -m "P7 bascule control-plane: seam ControlStore (memory|dynamodb, cache TTL 30 s, fail-closed 503 no-store), bin aithos-store-admin, gate contrat 15 scénarios RED→GREEN, cucumber 134/134 — GATE DÉPLOYÉ VERT (suspension 0,8 s, purge outillée, 2 gravures)"
   cd ../../provider
   git add infra/terraform/envs/prod infra/terraform/modules/store-api e2e/features
   git commit -m "P7: control-plane-min instancié dans envs/prod; store-api env exclusif bootstrap XOR control + attach reader; e2e control-p7 (5/5 déployé); plans du gate"
   ```
   Le push déclenche le job `test` de provider-image.yml (rejeu cucumber
   CI — la seule passe des features P7 hors disque) ; le job `push`
   échouera sur les vars `placeholder/*` (attendu).
2. **Tag `:<sha>` ECR** (après le commit aithos-core, sha = HEAD) :
   ```sh
   aws ecr put-image --repository-name aithos-provider-prod-store-api \
     --image-tag <sha-court> --image-manifest \
     "$(aws ecr batch-get-image --repository-name aithos-provider-prod-store-api \
        --image-ids imageTag=prod --query 'images[0].imageManifest' --output text)"
   ```
3. **Hygiène creds** : la copie sandbox de `.aws-env` est purgée ; le
   fichier disque expire seul (SSO ~1 h). Rien d'autre à faire.
4. **Prochain lot** (au choix du plan) : bascule relay (petit lot —
   `relay.json` → table control, mappings B.2 + `gateway#`), ou P3/P4
   (client RemoteStore, sync/perf), ou witness P5.

## 5. Environnement (delta session)

VM device MORTE toute la session (`device_bash` refuse) ; staging des
`.feature` de `rust/…/tests/features/store/` toujours refusé (HTTP 400) ;
**nouveau constat** : le proxy egress du sandbox intercepte le TLS sortant
(certificat resigné « Anthropic Egress Gateway ») — sans effet sur le
HTTPS ordinaire (curl/behave store OK) mais fatal aux protocoles ALPN
custom (sondes relay, §2). Sandbox : rustc 1.95 + musl-tools (apt),
Terraform 1.13.5, boto3/behave/pynacl, docker daemon absent (image par
API ECR), aws-api MCP non utilisé — tout AWS par CLI/boto3 sur les creds
de session (`.aws-env` rafraîchi par Mathieu ~15:45, copie sandbox
purgée en fin de session).

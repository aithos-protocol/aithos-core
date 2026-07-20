# HANDOFF — Piste P / Provider P2 : GATE DÉPLOYÉ étape 6 — FAIT (2026-07-20)

Date : 2026-07-20 (après-midi). Dépôts : `code/aithos-core` +
`provider/`. État DISQUE = vérité. Statut : **le gate déployé étape 6 est
JOUÉ et VERT** — backends durables S3 + DynamoDB en prod, preuve wire
20/20 contre `https://store.aithos.fr`, HA 2 tâches, tenant de rejeu
purgé, plan Terraform final à **0 écart**, les 4 gravures ⑦ ACTÉES et
ÉCRITES (GO Mathieu en session). Le commit reste TON geste.

Se lit avec `HANDOFF-PROVIDER-P2-ETAPE6-VERT-LOCAL-2026-07-20.md` (l'état
d'entrée), `DECISION-COMPUTE-STORE-PROPOSITION-GATE6-2026-07-20.md` et
`INFRA-PROVIDER.md` (4 notes gravées ce jour : A.5 ordre des effets, A.6
complément classes, A.7 `immutable_conflict` — registre à 10, §8
réalisation du tenant de rejeu).

## 0. Séquence de la session (tout GO'é par Mathieu, dans l'ordre)

1. **Sandbox reconstruit** (tarball 08:03 + overlay par mtime — les 2
   `.feature` finaux restent non-stageables, arbitrage : corroboration
   par build + rejeux, cucumber 131/131 reste la preuve de la session
   précédente). Binaire debug reconstruit : **p7 15/15, p9 33/33** GREEN.
2. **Trou détecté et arbitré AVANT tout apply** : la mécanique « tenant
   replay via CLI P7 » gravée le matin n'existe pas (pas de bin admin,
   pas de lecture DynamoDB des tenants par le service, `control-plane-min`
   non instancié dans `envs/prod` — vérifié code + AWS). GO Mathieu :
   **bootstrap minimal** — l'image embarque `prod-replay-20260720.json`
   (bindings pré-genèse des 2 DIDs des vecteurs, ZÉRO preload/seed) et
   `prod-none.json` (zéro tenant, état de repos) ; `replay.json` (acme)
   QUITTE l'image prod (décision ② exécutée). Gravé §8.
3. **Binaire musl statique** (release, static-pie) reconstruit depuis
   l'état corroboré ; p7/p9 rejoués contre CE binaire (15/15, 33/33) ;
   garde ② vérifiée localement (durable + preloads → exit 2).
4. **① Plan n°1 lu intégralement** (`plan-etape6-1.txt`) : 9 add
   (bucket versionné+SSE+PAB+ownership, table heads, policy task_data,
   2 révisions de task defs), 6 change, 2 destroy (anciennes révisions).
   Seul changement d'env store : `AITHOS_STORE_BOOTSTRAP →
   /bootstrap/prod-replay-20260720.json` — **SANS env durable**, comme
   attendu. Bruit relay : le `depends_on = [module.store_api]` du module
   relay diffère ses data sources dès que store-api bouge → réécritures
   à l'identique + une révision de task def relay par apply (redéploie le
   relay, coupure brève des tunnels — précédent M2, consigné).
5. **② Image poussée** sur ECR `:prod` par l'API (couche unique :
   binaire + CA bundle + les 2 bootstraps ; digest
   `sha256:187cee4c…aeec3`). ③ **Apply n°1** : task def **:4** déployée,
   healthz 200 ; wire : `acme` → `unknown_tenant` (retiré ✓),
   `replay-20260720` → `not_found` (pré-genèse ✓).
6. **④ Plan n°2 lu** (les 4 env durables seuls) + apply : task def
   **:5**, rollout COMPLETED — le boot prouve le bootstrap sans preloads
   (sinon exit 2 en boucle).
7. **⑤ Preuve déployée** : nouveau driver
   `vectors/deployed-replay-etape6.py` (séquentiel, horloge réelle,
   enveloppes re-signées avec les clés committées, did.json re-signé pour
   le tenant de rejeu — l'URL bundle porte le tenant). **20/20 GREEN**
   (détail §2). Vérif admin : layout S3 `t/replay-20260720/…` exact +
   item heads DynamoDB = la tête servie.
8. **⑥ Plan n°3 lu** (`desired_count 1→2` seul) + apply : 2 cibles
   saines derrière l'ALB, **20/20 rejoué à travers les 2 tâches** — le
   2e run reprend l'état durable du 1er (genèse « already-deposited »,
   gamma chaîné sur la tête courante) : aucune mémoire par tâche ne peut
   simuler ça. Les 2 streams de logs ont servi.
9. **Purge (plan n°4 lu + apply)** : bootstrap → `prod-none.json`
   (task def **:6**), puis suppression admin des 14 versions S3 du
   préfixe + item heads. Wire final : `replay-20260720` ET `acme` →
   `unknown_tenant`. **Plan final `-detailed-exitcode` = 0 écart.**
10. **⑦ Gravures** (GO « tout ») : les 4 notes dans `INFRA-PROVIDER.md`.

## 1. Livré (fichiers, write-back disque)

- `code/aithos-core/docker/store-api.Dockerfile` : `replay.json` sorti de
  l'image prod ; COPY des 2 bootstraps sans preloads (commentaire décision ②).
- `code/aithos-core/rust/crates/aithos-provider/bootstrap/prod-replay-20260720.json`
  + `prod-none.json` (nouveaux).
- `code/aithos-core/vectors/deployed-replay-etape6.py` (nouveau — le
  driver du gate déployé, réutilisable : `python3 deployed-replay-etape6.py
  <url> <tenant>` ; re-runnable, reprend la tête courante).
- `code/aithos-core/docs/INFRA-PROVIDER.md` : 4 notes gravées.
- `provider/infra/terraform/envs/prod/main.tf` : `bootstrap_path =
  "/bootstrap/prod-none.json"` (état de repos) + `durable_backends = true`
  (commentés gate étape 6). NB : `desired_count` est resté une var
  (appliqué avec `-var desired_count=2`) — voir « reste » ci-dessous.
- `provider/infra/terraform/envs/prod/plan-etape6-{1,2,3,4}.txt` : les 4
  plans lus, tels qu'appliqués.

## 2. Preuves (contre la PROD, 2026-07-20)

| Preuve | Résultat |
|---|---|
| p7/p9 vs binaire debug reconstruit | 15/15, 33/33 GREEN |
| p7/p9 vs binaire musl DE L'IMAGE | 15/15, 33/33 GREEN |
| garde ② locale (durable+preloads) | exit 2, message fail-closed exact |
| deployed-replay (1 tâche, :5) | **20/20 GREEN** |
| deployed-replay (2 tâches, :6→ re-run) | **20/20 GREEN**, état repris |
| dont ⑧b | re-dépôt identique accepté ; squat → `400 artifact_invalid`/`immutable_conflict` |
| dont CAS A.5 réel | genèse `If-Head: none`, append, `428 cas_required`, perdant `409` + tête courante relue |
| dont A.6 en-têtes réels | did.json + e/public : `public, max-age=0, must-revalidate` + ETag fort (SHA-256 exact) ; cert : `public, max-age=31536000, immutable` ; heads + segment courant : `no-store` |
| layout admin | S3 `t/<tenant>/<did>/…` exact ; heads item = tête servie ; purge → 0 version restante |
| plans lus avant chaque apply | 4/4, fichiers committés |
| plan final | **0 écart infra↔code** (`-detailed-exitcode` = 0) |
| healthz post-purge | 200, 2 tâches, task def :6 |

## 3. Consigné SANS graver (observations de session)

- **503 `unavailable` des seams objets/têtes** : opérationnel, pattern
  nonces (P1 arbitrage n°3) — pas de ligne A.7 nouvelle.
- **`refuse()` n'émet aucun `Cache-Control`** sur les surfaces d'erreur
  (observé : 404 sans en-tête). RFC 9110 rend un 404 heuristiquement
  cachable — un `no-store` explicite sur les refus est à porter au
  prochain gate contrat (micro-choix, pas fait unilatéralement).
- **Mapping tunnel démo retiré du bootstrap prod** (conséquence ②) :
  `/acme/txt` pour `demo.mcp.aithos.fr` répond désormais
  `mapping_mismatch` jusqu'à la bascule P7. Le cert du relais est
  out-of-band (M2), rien ne casse aujourd'hui.
- **Churn Terraform du module relay** : chaque apply touchant store-api
  redéploie le relay (data sources différées par le `depends_on` module).
  Fix propre possible (depends_on ressource plutôt que module) — à
  arbitrer un jour, pas urgent.
- **Note de build (transitoire, même statut qu'au gate P1)** : l'image a
  été assemblée dans la session (couche unique déterministe, digests dans
  le handoff) — le Dockerfile officiel mis à jour reconstruira à
  l'identique au premier run CI. Vars GitHub des trust policies toujours
  `placeholder/*` (verrouillées par construction).
- **Créer la vraie bascule P7 reste un lot entier** : bin admin
  (create/bind/suspend → DynamoDB), lecture control-plane par le service,
  `control-plane-min` dans `envs/prod`, preuve suspension < 60 s.

## 4. Reste pour clore (Mathieu)

1. **Commits** (les sessions n'ont touché QUE ces chemins) :
   ```sh
   cd code/aithos-core
   git add rust/crates/aithos-provider rust/Cargo.toml rust/Cargo.lock \
           vectors docs docker
   git commit -m "P2 étape 6 DÉPLOYÉE: backends durables S3+DynamoDB en prod — deployed-replay 20/20, ⑧b immutable_conflict sur le wire, cache A.6 réel, HA x2, tenant rejeu purgé, 4 gravures INFRA-PROVIDER"
   cd ../../provider
   git add infra/terraform/modules/store-api infra/terraform/envs/prod
   git commit -m "P2 étape 6: store-data + heads + task role étendu; envs/prod: durable_backends=true, bootstrap prod-none, plans du gate"
   ```
2. **Hygiène creds** : `.aws-env` à vider (les clés de session expirent
   seules à 12:33 ; le sandbox a purgé sa copie).
3. **Optionnel** : figer `desired_count = 2` en dur dans `envs/prod`
   (aujourd'hui : défaut 1 + `-var` à l'apply — un apply sans la var
   redescendrait à 1) ; cucumber 131/131 à rejouer localement si tu veux
   la ceinture ET les bretelles (le disque a les features finaux).
4. **Prochain lot** (au choix du plan) : P7 bascule control-plane, ou
   P3/P4 (client RemoteStore, sync/perf), ou witness P5.

## 5. Environnement (delta session)

VM device toujours morte ; staging `.feature` toujours refusé (HTTP 400)
— contourné : corroboration par binaire + p7/p9 (GO Mathieu), les
features finaux étape 6 n'ont PAS été rejoués en sandbox. `.aws-env`
rafraîchi une fois par Mathieu en session (SSO admin — expiration 12:33
Paris) ; aws-api MCP resté inutilisable (token SSO device absent), tout
AWS est passé par le CLI/boto3 du sandbox avec les creds de session,
purgés en fin de session. Terraform 1.13.5 installé dans le sandbox ;
docker daemon indisponible → image assemblée/poussée par l'API ECR
(python), digests §0.5.

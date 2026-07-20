# HANDOFF — Piste P / P7 bascule control-plane : VERT LOCAL (2026-07-20)

Date : 2026-07-20 (après-midi). Dépôts : `code/aithos-core` + `provider/`.
État DISQUE = vérité. Statut : **le lot P7 est VERT LOCAL** — gate contrat
joué (scénarios observés RED puis GREEN), 4 arbitrages tranchés par
Mathieu en session, seam control implémenté, bin admin écrit, Terraform
instancié (fmt + validate verts), batterie de non-régression complète
GREEN. **STOP au gate déployé** — aucun plan, aucun apply, aucune gravure
INFRA-PROVIDER (le commit reste le geste de Mathieu).

Se lit avec `HANDOFF-PROVIDER-P2-GATE6-DEPLOYE-DONE-2026-07-20.md` (état
d'entrée, prod au repos sur `prod-none.json`), `INFRA-PROVIDER.md` (§8,
A.7 — AUCUN code nouveau), `PROMPT-REPRISE-PROVIDER-P7-BASCULE-2026-07-20.md`
(la mission), `provider/.claude/skills/rituel-tests/SKILL.md`.

## 0. Séquence de la session (rituel respecté)

1. **État disque vérifié** : commits gate 6 FAITS (aithos-core 13:08,
   provider 13:11). Sandbox reconstruit (tarball source 08:03 + overlay
   par mtime, 26 fichiers ; le staging des `.feature` refuse toujours
   HTTP 400 — write-back par `device_commit_files` OK, lui).
2. **Batterie d'entrée GREEN** : p7 15/15, p9 33/33, cucumber 119/119
   (étape 6), 46 unités, core+bundle `--locked`, clippy `-D warnings`,
   fmt. Vecteurs gelés intacts (rejeux octet-exact).
3. **Gate contrat** : `store-control-p7.feature` écrit (15 scénarios) et
   **observé RED** (exit 101, « Step doesn't match any function », les
   119 existants ne régressent pas) ; `control-p7.feature` (behave e2e)
   écrit. Les deux committés sur disque AVANT l'implémentation.
4. **4 arbitrages tranchés par Mathieu** (AskUserQuestion, en session) :
   - **Fraîcheur : cache TTL 30 s** (`AITHOS_STORE_CONTROL_TTL_SECS`,
     défaut 30) — borne < 60 s tenue avec marge ; résultats négatifs
     cachés aussi (création ET suspension propagent dans la borne).
   - **Bootstrap : optionnel sous dynamodb** — la task def ne porte PLUS
     `AITHOS_STORE_BOOTSTRAP` ; garde fail-closed : backend dynamodb +
     bootstrap portant tenant/tunnel/preload/heads → exit 2.
   - **Relay : HORS LOT** — le store bascule (y compris `resolve_tunnel`
     pour `/acme/txt`) ; le relay garde son bootstrap (`relay.json`),
     sa bascule est un petit lot suivant.
   - **Cache-Control : no-store sur TOUS les refus** (consigné gate 6
     tranché) — `refuse()` + `refuse_deposit()` l'émettent, 2 scénarios
     le pinnent.
   Point levé : le « schéma clé `t` seule » du prompt de reprise était
   dépassé — `control-plane-min` porte déjà le single-table `pk`/`sk`
   (`tenant#<t>/meta` `{s: BOOL}`, `tenant#<t>/did#<did>`,
   `gateway#<gw>/meta` `{t,h,s}`) ; schéma validé tel quel.
5. **Implémentation** (mêmes lookups, wire inchangé) :
   - `control.rs` : trait `ControlStore` (tenant_state / did_bound /
     resolve_tunnel, chacun avec `now_ms`), `ControlUnavailable` →
     503 `unavailable` (pattern seams étape 6) ; `ControlPlane`
     (bootstrap) reste le backend dev/tests ET le read-model du relay
     (sync, intact) ; `CachedControl` (fenêtre de fraîcheur stricte,
     jamais de stale au-delà du TTL, horloge de requête injectée — la
     borne se prouve à l'horloge de test, jamais au sleep) ;
     `DynamoDbControl` (GetItem simples, item malformé = unanswerable,
     jamais une absence fantôme).
   - `service.rs`/`envelope.rs`/`acme.rs` : les 3 sites de lookup passent
     par le seam, `Err` → `Refusal::Unavailable` ; un backend muet ne
     fabrique NI `unknown_tenant` NI `did_not_bound`.
   - `store_api.rs` : `AITHOS_STORE_CONTROL_BACKEND` memory|dynamodb
     (défaut memory — une ancienne task def boote le nouveau binaire),
     `AITHOS_STORE_CONTROL_TABLE`, `AITHOS_STORE_CONTROL_TTL_SECS` ;
     bootstrap REQUIRED sous memory, optionnel sous dynamodb + garde.
   - **`aithos-store-admin` (nouveau bin)** : create / bind-did /
     suspend / reactivate / purge `--yes` → DynamoDB sous les creds de
     l'OPÉRATEUR (`AITHOS_ADMIN_CONTROL_TABLE`, jamais dans l'image) ;
     `purge` = le runbook GC §8 outillé : versions S3 du préfixe
     `t/<tenant>/` (+ delete markers) → items heads → lignes control EN
     DERNIER (un purge interrompu laisse le tenant refusant) ; create
     conditionnel (jamais d'écrasement d'un suspended), bind-did exige
     le meta, grammaire tenant A.1 vérifiée à l'entrée.
6. **Terraform** : `control-plane-min` instancié dans `envs/prod` ;
   `store-api` : attach policy READER seule sur le task role (l'admin
   n'attache jamais), env exclusif bootstrap XOR control (sous dynamodb
   la task def ne porte plus de bootstrap du tout). `fmt -check` +
   `validate` verts (Terraform 1.13.5). **AUCUN plan/apply.**
7. **e2e behave** : `control-p7.feature` aligné sur les steps existants +
   `control_steps.py` (nouveau) — 2 scénarios keyless (unknown_tenant,
   no-store) joués GREEN contre le binaire local ; 3 scénarios
   deploy-gate qui SKIPPENT bruyamment sans `E2E_CONTROL_TENANT` /
   `E2E_ADMIN_CMD` (jamais verts par accident) ; la borne < 60 s se
   mesure du RETOUR de la commande admin au premier flip sur le wire.

## 1. Preuves (locales, 2026-07-20)

| Preuve | Résultat |
|---|---|
| Gate contrat observé RED avant implémentation | exit 101, 13 puis 15 steps « doesn't match », 119 existants verts |
| cucumber store (`--features pod-stub`) | **134/134** (15 P7 + 119 étape 6), 815 steps |
| cucumber tunnel / relay | 18/18, 12/12 |
| unités lib (dont 4 nouvelles CachedControl) | 50/50 |
| red-replay-p7 / p9 vs binaire reconstruit | **15/15, 33/33** (vecteurs gelés, octet-exact) |
| core + bundle `--locked` | OK |
| clippy `-D warnings` + fmt | OK |
| garde P7 (dynamodb + bootstrap à tenants) | **exit 2**, message fail-closed exact |
| garde memory sans bootstrap | **exit 2** |
| dynamodb + coquille zéro-tenant, table injoignable | boote, healthz 200, wire → **503 `unavailable` + `Cache-Control: no-store`** (jamais un unknown_tenant inventé) |
| behave control-p7 keyless vs binaire local | 2/2 GREEN, 3 deploy-gate SKIP |
| terraform fmt + validate (envs/prod) | verts |

Réserve consignée : les 2 features étape 6 du sandbox venaient du tarball
de 09:57 (staging `.feature` refusé) — cucumber 134/134 passe avec le
`cucumber.rs` final du disque, corroboration forte ; l'écart exact se
ferme en un `tar czf` des features si souhaité.

## 2. Livré (write-back disque fait, 16 fichiers)

`code/aithos-core` : `src/control.rs`, `src/service.rs`,
`src/envelope.rs`, `src/acme.rs`, `src/bin/store_api.rs`,
`src/bin/store_admin.rs` (nouveau), `Cargo.toml` (bin admin),
`tests/cucumber.rs`, `tests/features/store/store-control-p7.feature`
(nouveau), ce handoff. `Cargo.lock` inchangé.

`provider` : `infra/terraform/envs/prod/main.tf`,
`modules/store-api/{main,variables}.tf`, `e2e/features/control-p7.feature`
(nouveau), `e2e/features/steps/control_steps.py` (nouveau).

## 3. Reste pour clore le lot (gate déployé — session dédiée avec creds)

1. **Commits Mathieu** (les sessions n'ont touché QUE les chemins du §2).
2. Session gate déployé : `.aws-env` rafraîchi (SSO ~1 h), image `:prod`
   reconstruite (binaire musl + les bootstraps sortent-ils de l'image ?
   décision au gate — l'env n'y pointe plus), **plan lu INTÉGRALEMENT**
   (attendu : table control + 2 policies + attach reader + révision task
   def qui RETIRE `AITHOS_STORE_BOOTSTRAP` et ajoute les 2 env control ;
   churn relay connu), apply sur GO, `desired_count=2` re-passé par
   `-var` (toujours pas figé — consigné gate 6).
3. Preuves déployées : tenant réel créé par `aithos-store-admin` ;
   `python3 vectors/deployed-replay-etape6.py https://store.aithos.fr
   <tenant>` **20/20** ; `E2E_BASE_URL=… E2E_CONTROL_TENANT=…
   E2E_ADMIN_CMD=… behave e2e/features` (suite COMPLÈTE, les features
   des lots précédents ne régressent pas) — la suspension < 60 s se
   prouve là ; purge outillée du tenant de preuve.
4. Gravures INFRA-PROVIDER (§7/§8 : bascule réalisée, cache 30 s,
   no-store des refus, relay hors lot) sur GO explicite uniquement.

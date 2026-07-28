# HANDOFF — Piste P / P7b bascule relay : GATE DÉPLOYÉ — FAIT (2026-07-20)

> **ARCHIVE DE PREUVE.** Bascule relay P7b close.

Date : 2026-07-20 (soirée). Dépôts : code/aithos-core + provider/. État DISQUE = vérité.
Statut : le gate P7b est JOUÉ et VERT — **le relay lit ses mappings B.2 dans la table
control** (task def relay `:8`, plus aucun bootstrap nulle part en prod), le join
d'autorité B.5 est adopté par B.2 (redline gravée), le balayeur B.4 ferme les tunnels
suspendus/purgés/re-mappés en < 60 s, un enregistrement LIVE a été accepté depuis le
réseau Mathieu via un mapping enrôlé à la CLI, la table est revenue à ZÉRO item (état
de repos), plan final 0 écart. **Session conduite de bout en bout sur délégation
Mathieu (absent), avec un TÉMOIN DE GATE adversarial en agent — une première ;
verdict : 1 bloquant trouvé et corrigé avant clôture (D1).** Le commit reste TON geste.

Se lit avec HANDOFF-PROVIDER-P7-GATE-DEPLOYE-DONE-2026-07-20.md (l'état d'entrée),
INFRA-PROVIDER.md (2 gravures ce soir : redline B.2 « ordre d'autorité B.5 » + note §7
« bascule relay : RÉALISÉE » qui clôt le « relay HORS LOT » de la note P7).

## 0. Séquence de la session (délégation explicite Mathieu en cours de session)

1. **Arbitrages tranchés par Mathieu (AskUserQuestion, avant le code)** : ① B.2
   étape 4 = join complet motif B.5 (binding → suspension binding → état TENANT →
   match exact — `suspend <tenant>` gate ses tunnels en UNE écriture) ; ② panne du
   backend control : les tunnels ACTIFS survivent, seuls les nouveaux enregistrements
   refusent `unavailable` ; ③ périmètre 7 points GO.
2. **Sandbox reconstruit** (staging fichier à fichier, ~250 fichiers ; les 8 `.feature`
   sous rust/…/tests/features refusent toujours le staging HTTP 400 — contourné par un
   tarball Mathieu, qui purge aussi les `._*` AppleDouble). Batterie d'entrée :
   cargo check --locked workspace EXIT=0.
3. **Gate contrat RED→GREEN** : `relay-control.feature`, 8 scénarios RED (« Step
   doesn't match ») → seam codé → 8/8 GREEN, puis **9/9** (scénario « remapped »
   ajouté sur verdict témoin D3). Le harnais joue la VRAIE composition : vraie
   `RelayDoor`, vraie `CachedControl` 30 s, vrai `reconcile_registry`, horloge
   injectée — jamais un sleep.
4. **Code** : `ControlStore` object-safe consommé par le relay ; `authorize_gateway`
   (control.rs) = l'ordre B.5, UN chemin de code partagé tunnel.rs + acme.rs ;
   `reconcile_registry` + `SessionRegistry::sessions` (passthrough.rs) ; bin/relay.rs
   backend memory|dynamodb + garde fail-closed P7 verbatim + tâche de balayage TTL/2 ;
   CLI `bind-gateway` (miroir d'abord) / `unbind-gateway` (binding d'abord) / `purge`
   étendu gateway-rows (delete CONDITIONNEL `t = :t`, correctif D1) ; fixtures
   (relay.json + les 4 harnais) sèment le tenant (le join l'exige) ; p3 rejoue
   byte-exact, vecteur committé INTOUCHÉ.
5. **Docker/Terraform** : relay.Dockerfile sans bootstrap (repli M2 épinglé
   `sha256:d8f93851…58250`) ; module relay bloc env conditionnel motif store-api
   verbatim (`control_table_name` null → bootstrap, posé → CONTROL_BACKEND +
   CONTROL_TABLE, RIEN d'autre) ; envs/prod passe table + reader policy au relay.
6. **Gate déployé (GO Mathieu explicite avant la séquence)** : image :prod relay
   poussée par l'API ECR (couche unique déterministe, binaire musl static-pie + CA
   bundle, digest `sha256:328e6119…4c8e7e`, layer gz 6,1 Mo) ; apply du plan lu
   (2 add / 1 change / 1 destroy — task def :8 env control, attach reader, service ;
   PLUS `-var desired_count=2` OBLIGATOIRE sinon le plan rétrograde le store 2→1,
   piège re-rencontré et documenté §2) ; rollout COMPLETED 1/1.
7. **Preuves** (§1) puis **rituel témoin** : agent adversarial → verdict D1-D4 ;
   D1 corrigé + rejoué, D3 couvert par le 9e scénario, D2/D4 consignés (gravure §7).
8. **État de repos** : purge acme par la CLI corrigée — 1 binding+miroir balayés,
   meta EN DERNIER, table à 0 item.

## 1. Preuves (contre la PROD, 2026-07-20)

| Preuve | Résultat |
|---|---|
| cargo check --locked workspace | EXIT=0 |
| unités lib + bins | 53 + 1 (grammaire hostname CLI) |
| cucumber complet (features via tarball) | store 146/146 (931 steps), tunnel 26/26, relay **27/27** (passthrough + 9 P7b) |
| replays byte-exact | p3 tunnel 2/2 (join actif, vecteur intouché), sni p5, acme p6 2/2, relay_handshake 4/4 |
| vectors_replay / witness_replay | 5/5 / 3/3 |
| gardes fail-closed bin relay | 4/4 exit 2 (memory sans bootstrap ; dynamodb+bootstrap non vide ; dynamodb sans table ; backend inconnu) + boot nominal sain |
| binaires musl static-pie | relay + admin + api |
| terraform fmt/validate | 0 / valid |
| plan appliqué (lu INTÉGRALEMENT ×2, v1 rejetée pour l'écart desired_count) | 2 add / 1 change / 1 destroy — rien hors lot |
| rollout | COMPLETED, task def relay :8, 1/1 |
| items control post bind-gateway | EXACTS : `tenant#acme/meta {s:false}` + miroir `tenant#acme/gateway#z6MksPy…` + `gateway#z6MksPy…/meta {t,h,s:false}` |
| **enregistrement LIVE** (réseau Mathieu, TLS+ALPN réel) | `relay-register.py` → `{"aithos-tunnel":"1.0.0-draft.1","ok":true}` ✓ CONFORME, loggué côté relay `outcome=ok tenant=acme` |
| **suite déployée `relay-control-p7b`** (réseau Mathieu, retour de session) | **4/4** — enrôlement CLI→relay ok ; suspension refusée en **28,9 s** ; réactivation en **29,5 s** ; désenrôlement → `mapping_mismatch` en **31,3 s** (borne 60 s, poll 2 s) |
| purge (CLI corrigée D1) | gateway+miroir balayés, meta DERNIER, table 0 item |
| plan final `-detailed-exitcode` (mêmes -var) | **0 — No changes** |
| témoin de gate (agent adversarial) | « le vert n'est pas réfuté » après correctif D1 ; comptes reconfirmés indépendamment |

## 2. Consigné SANS graver (observations de session)

- **Suite behave `relay-control-p7b.feature` REJOUÉE au retour de Mathieu : 4/4**
  (mesures au §1). Le premier essai avait levé un `AmbiguousStep` behave (préfixe de
  step partagé avec control_steps.py — corrigé : `the admin CLI flips "{command}" on
  tenant "{tenant}"`). Post-suite, la table a été re-purgée à 0 item depuis le
  sandbox (CLI corrigée D1) — l'état de repos est rétabli, le §4.1 est CLOS.
- **Verdicts témoin consignés** : D2 — GoAway perdable dans une fenêtre de course
  infime (`Notify::notify_waiters` sans permis ; un tunnel dépinglé peut garder son
  mux ouvert ; correctif `CancellationToken` au prochain lot relay). D4a —
  `suspended_of` traite un item `meta` sans attribut `s` comme actif (la CLI seul
  écrivain le pose toujours). D4b — `AITHOS_RELAY_CONTROL_TTL_SECS` non clampé (un
  opérateur > 40 s casserait silencieusement la borne < 60 s). Les deux premiers
  sont AUSSI dans la gravure §7.
- **`-var desired_count=2` OBLIGATOIRE** sur tout plan/apply d'envs/prod (avec
  `terraform_state_bucket_name` et les deux `github_repository_*` =
  `placeholder/aithos-provider` / `placeholder/aithos-core`) — sans lui le plan
  rétrograde le store 2→1. Vu, neutralisé, pas appliqué. Le README d'envs/prod
  mérite ces quatre -var un jour (micro-choix, pas fait unilatéralement).
- **Sondes ALPN toujours injouables depuis le sandbox** (proxy egress TLS) — motif P7
  §2 inchangé ; la preuve live est venue du réseau Mathieu.
- **Staging `.feature` HTTP 400** : contournement pérenne = tarball
  (`tar czf features-rust.tgz code/…/tests/features` depuis v2/) ; penser à purger
  les `._*` AppleDouble après extraction (5 erreurs de parse cucumber sinon).
- **Fenêtre image↔task def** : entre le push :prod et l'apply, un restart spontané de
  l'ancienne task def aurait bouclé (image sans bootstrap + env bootstrap). Fenêtre
  de ~1 min, GO'ée, rien observé — même acceptation qu'au P7 store.
- La copie sandbox de `.aws-env` meurt avec le container ; le fichier disque expire
  seul (SSO ~1 h).

## 3. Livré (fichiers, write-back disque fait)

**code/aithos-core** : src/{control,tunnel,relay,passthrough,acme}.rs,
src/bin/{relay,store_admin}.rs, bootstrap/relay.json (tenant semé),
tests/{tunnel_replay,relay_handshake,cucumber_tunnel,cucumber_relay}.rs,
tests/features/relay/relay-control.feature (NOUVEAU, 9 scénarios),
docker/relay.Dockerfile, docs/INFRA-PROVIDER.md (2 gravures), ce handoff,
PROMPT-REPRISE-PROVIDER-P7B-CLOTURE-2026-07-20.md.

**provider** : infra/terraform/modules/relay/{main,variables}.tf,
modules/control-plane-min/main.tf (item miroir documenté), envs/prod/main.tf,
envs/prod/{plan-p7b.txt,plan-p7b-final.txt} (les plans lus, tels qu'appliqués),
e2e/features/relay-control-p7b.feature (NOUVEAU) + steps/relay_control_steps.py
(NOUVEAU).

## 4. Reste pour clore (Mathieu)

1. ~~Suite déployée~~ **FAIT** (4/4, mesures §1) ; ~~retour table vide~~ **FAIT**
   (re-purge depuis le sandbox, 0 item).
2. **Commits** — si les blocs P7 (handoff P7 §4) ne sont pas encore commités, les
   faire d'abord ; puis :
   ```
   cd code/aithos-core
   git add rust/crates/aithos-provider docs docker
   git commit -m "P7b bascule relay: B.2 étape 4 sur ControlStore (join autorité B.5, helper partagé authorize_gateway), balayeur B.4 <60s (les tunnels survivent aux pannes), CLI bind/unbind-gateway + purge gateway-rows conditionnel (verdict témoin D1), image relay sans bootstrap — gate contrat 9 scénarios RED→GREEN, cucumber 146+26+27, GATE DÉPLOYÉ VERT (enregistrement live ok, table seule source, plan final 0 écart)"

   cd ../../provider
   git add infra/terraform/modules/relay infra/terraform/modules/control-plane-min infra/terraform/envs/prod e2e/features
   git commit -m "P7b: relay sur la table control (env conditionnel motif store-api, reader policy attachée); e2e relay-control-p7b (CLI→relay, borne <60s); plans du gate"
   ```
   Le push aithos-core déclenche provider-image.yml : le job test rejoue TOUTES les
   features (dont les 9 P7b) — la passe CI habituelle ; le job push (store) refera
   :prod+:sha store, sans conséquence.
3. **Tags ECR `:<sha>`** (sha = HEAD aithos-core court), store (reliquat P7) **et
   relay** :
   ```
   aws ecr put-image --repository-name aithos-provider-prod-relay \
     --image-tag <sha-court> --image-manifest \
     "$(aws ecr batch-get-image --repository-name aithos-provider-prod-relay \
        --image-ids imageTag=prod --query 'images[0].imageManifest' --output text)"
   ```
   (idem `aithos-provider-prod-store-api` si le reliquat P7 tient toujours.)

## 5. Prochain lot (au choix du plan)

Witness P5 (le dernier contrat non servi — module TF et vecteur p4 prêts), ou P3/P4
client RemoteStore + gates perf §3.6, ou le lot ops (§8 : quotas tenant B.4,
rétention GC, DR testée) — la route v1 discutée en session : P5 → P3/P4 → ops →
dashboard. Au prochain lot relay, embarquer D2 (CancellationToken) et D4.

## 6. Environnement (delta session)

VM device MORTE (device_bash refuse) ; staging `.feature` HTTP 400 (tarball = le
contournement) ; proxy egress TLS (sondes ALPN impossibles du sandbox) ; terraform
1.13.5 + rustc 1.95 + musl installés dans le container ; docker daemon absent (image
par API ECR, script /tmp/push-relay-image.py de la session — recréable, méthode
gate 6) ; applies joués d'ici sous les creds SSO de Mathieu (export ~17:50 Paris).

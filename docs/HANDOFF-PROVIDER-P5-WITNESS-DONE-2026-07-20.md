# HANDOFF — Piste P / Lot A (P5 witness) : GATE DÉPLOYÉ — FAIT (2026-07-20)

> **ARCHIVE DE PREUVE.** Gate witness clos ; les coordonnées de déploiement sont
> historiques.

Date : 2026-07-20 (nuit). Dépôts : code/aithos-core + provider/. État DISQUE = vérité.
Statut : **le dernier contrat (C3) est SERVI et PROUVÉ en prod.** Gate contrat
12 scénarios RED→GREEN (72 steps), gate déployé VERT — `aithos-witness` tourne sur
Fargate (clé KMS Ed25519 native, stream heads comme déclencheur C.2), le feed public
`witness.aithos.fr` sert des checkpoints **vérifiés indépendamment (PyNaCl)**,
**latence publish→checkpoint : 6,1 s** (2e édition 11,5 s), plan final 0 écart,
tables au repos (control 0, heads 0). Témoin de gate adversarial : « le vert n'est
pas réfuté » — **1 bloquant (D1, racine quotidienne perdable) corrigé, rejoué et
REDÉPLOYÉ avant clôture** ; 7 consignés (D2–D8). Les consignés P7b (D2 GoAway,
D4a/D4b) sont AUSSI embarqués. Le commit reste TON geste (blocs §4).

Se lit avec HANDOFF-PROVIDER-P5-WITNESS-VERT-LOCAL-2026-07-20.md (l'état
intermédiaire et la reconstruction du sandbox), INFRA-PROVIDER.md (2 gravures ce
soir : note §7 « P5 RÉALISÉ » + additif C.3 keys.json/classes de cache).

## 0. Séquence (Mathieu présent par intermittence, GO explicite pour le gate)

1. Arbitrages AskUserQuestion (voir handoff VERT-LOCAL §0.2) : ① stream heads,
   ② S3+CloudFront, ③ desired 1, ④ KMS per C.1.
2. Gate contrat RED→GREEN (11 puis 12 scénarios après le verdict D1).
3. **Creds** : la session SSO expirée a ressoudé deux fois le cache
   (`export-credentials` SANS `sso login` ressert les creds périmés ; puis le
   PONT a servi une copie stagée périmée du fichier — cache par taille).
   Débloqué par le MCP `aws-api` de la machine Mathieu (session locale fraîche) :
   **rôle de session temporaire `aithos-ops-session-p5`** créé (AdministratorAccess,
   trust account-root, 1 h), assumé depuis le sandbox — CONSIGNÉ, supprimé à la
   clôture (§4.4).
4. Tags ECR posés : store+relay `:96531e2` (digests conformes), witness
   `:prod` + `:96531e2-p5`.
5. **Plan lu INTÉGRALEMENT ×2** : v1 REJETÉE — le `depends_on = [module.store_api]`
   au niveau module forçait un REMPLACEMENT de la task def relay à contenu
   identique (1 destroy hors lot) dès que store-api portait un changement (le
   stream heads). Corrigé par une dépendance CIBLÉE (`cluster_name =
   module.store_api.cluster_name`, var nouvelle du module relay). Plan v2 :
   **26 add / 2 change / 0 destroy**, rien hors lot, relay intact.
6. Apply (26/1/0), image witness poussée par l'API ECR (couche unique
   déterministe, digest `sha256:649f554b…5fed1eba` post-D1 ; pré-D1 :
   `sha256:d630018d…`), rollout COMPLETED, clé KMS résolue
   `z6MkkfMTRRPf1Zt2vkbPrJuGLBXux2Wm2jJc74jp3HSj3f2E`.
7. Preuves wire (§1), purge, plan final 0, témoin adversarial, correctif D1,
   redéploiement (consigné : l'image a changé SOUS le tag `:prod` sans
   changement d'infra — `force_new_deployment` est l'acte minimal ; la règle
   « la révision de task def fait le rollout » couvre les applies, pas un
   correctif d'image à tag constant), gravures, write-back.

## 1. Preuves (contre la PROD, 2026-07-20)

| Preuve | Résultat |
|---|---|
| cargo check --locked workspace | EXIT=0 |
| unités lib + bins | 56 + 2 (dont D2 course, D4a strict, D4b clamp) |
| cucumber | store 146/146 (931), tunnel 12/12 (40), relay 27/27 (151), **witness 12/12 (72)** |
| replays byte-exact | vectors 5/5, p3 2/2, p5, p6 2/2, handshake 4/4, **witness p4 3/3** — vecteurs INTOUCHÉS |
| musl static-pie | 4/4 (witness 16,7 Mo) |
| gardes fail-closed bin witness | 5/5 exit 2 + boot nominal sain |
| terraform | fmt OK, validate Success ; plan appliqué **26 add / 2 change / 0 destroy** (lu ×2, v1 rejetée pour le churn relay) |
| rollout | COMPLETED, service witness 1/1 (×2 : initial + redéploiement D1) |
| **deployed-replay-witness.py** (13 checks) | **13/13 GREEN** — genesis 204, publish h1 200, **checkpoint public en 6,1 s**, keys.json auto-signé vérifié, clé au registre, signature PyNaCl, champs C.1 exacts (gamma_head copié), cache feed+keys `public, max-age=60`, publish h2 200, 2e checkpoint 11,5 s, chaîne ≠ équivocation |
| behave witness-p5 | 3/3 (8 steps) — keys.json signé servi, classe de cache, feed inconnu sans fuite |
| purge (CLI admin) | 5 versions S3 + 1 heads + 2 lignes control — table **0 item** (heads 0 aussi) |
| plan final -detailed-exitcode (mêmes 4 -var) | **0 — No changes** |
| témoin de gate (agent adversarial, re-comptes indépendants) | « LE VERT N'EST PAS RÉFUTÉ » ; comptes reconfirmés (11/11 puis 12/12, 146, 27, 12, p4 3/3, wire PyNaCl, AWS état) ; D1 bloquant corrigé + rejoué + redéployé |

## 2. Verdicts témoin consignés (D1 corrigé ; D2–D8 consignés)

- **D1 (BLOQUANT, CORRIGÉ)** : la racine quotidienne pouvait être perdue
  (rollover manqué sur restart ; erreur feed jamais retentée). Correctif :
  `publish_missing_roots` — balayage idempotent, dérivé du feed, à CHAQUE tick
  et au boot ; scénario 12 RED→GREEN ; image redéployée.
- **D2** : la vérification de keys.json est structurellement circulaire — la
  confiance d'amorçage est l'origine TLS ; à la rotation rien ne chaîne vers la
  clé précédente. Gravé comme limite dans l'additif C.3.
- **D3** : `observe()` signe en KMS AVANT le dedup C.2 → à n DIDs, ~1440·n
  signatures jetées/jour par le reconcile au tick. Inverser dedup/signature au
  prochain passage witness (perf/coût, pas une faute).
- **D4** : au rollover, reconcile (nouveau jour = dedup vierge) + heartbeat
  émettent DEUX lignes octet-identiques par DID (Ed25519 déterministe, même
  `now`). Toléré C.2, dédupliqué dans la racine — le heartbeat est devenu
  redondant avec le reconcile par tick : à simplifier un jour.
- **D5** : la policy emitter n'épingle pas `kms:SigningAlgorithm` (le piège
  `_PH_` n'est bloqué que par le code) et la key policy donne kms:* au root du
  compte. À resserrer au prochain lot ops.
- **D6** : l'append-only du feed est une discipline de code (`If-Match`), pas
  une contrainte IAM (PutObject sans condition). Atténué : versioning, pas de
  Delete, bucket privé OAC.
- **D7** : le test de course D2-P7b est probabiliste (pas de barrière forçant
  « cancel avant park ») — le fix est correct par construction, le test
  l'exerce presque toujours.
- **D8** : les lignes de rejeu RESTENT dans le feed après purge du tenant —
  DESIGN (append-only C.3 ; les renier casserait la racine). ⚠ Conséquence
  opérationnelle : re-créer un tenant de rejeu sur le MÊME DID et re-publier
  une hauteur déjà observée fabriquerait une VRAIE équivocation publique
  (histoire du DID réinitialisée par la purge) — les prochains rejeux
  utilisent un DID frais ou assument la paire C.4. Résidu : 4 nonces TTL
  auto-purgés (~15 min).

## 3. Consigné SANS graver (session)

- **Piège terraform** : un `depends_on` MODULE met toutes les data sources du
  module en « known after apply » dès que la dépendance porte UN changement →
  remplacements de task defs à contenu identique. Toujours préférer la
  dépendance ciblée par référence d'output.
- **Piège staging** : le pont peut resservir une copie stagée périmée d'un
  fichier de MÊME taille (mtime avancé, contenu ancien). Contournement : copier
  sous un chemin jamais stagé. Pour les creds : passer par le MCP aws-api
  (assume-role) est plus fiable que le fichier.
- **SSO** : `export-credentials` sans `sso login` préalable ressert le cache
  expiré ; purger `~/.aws/cli/cache` au besoin.
- La borne D4b est TTL ≤ 39 (39 + 19,5 < 60), pas 40 (40 + 20 = 60 tangente).
- Ce que le gate n'a PAS prouvé (axe G du témoin) : rollover/racine réels en
  prod (première racine au prochain minuit UTC — le balayage D1 la garantit),
  rotation de clé, équivocation en prod, reconcile au restart avec heads non
  vide, latence multi-DID.

## 4. Reste pour clore (Mathieu)

1. **Commits** — blocs prêts (après ton push des commits P7/P7b déjà faits) :
   ```
   cd code/aithos-core
   git add rust/crates/aithos-provider rust/Cargo.toml rust/Cargo.lock docker vectors/deployed-replay-witness.py docs
   git commit -m "P5 witness: le contrat C3 servi — service aithos-witness (stream heads NEW_AND_OLD_IMAGES = déclencheur C.2, observation corroborée par re-hash du manifest, pending sweep, heartbeat, racines quotidiennes JAMAIS perdues — verdict témoin D1, keys.json signé), signeur KMS Ed25519 natif (RAW), feed S3 If-Match + classes C.3, gardes fail-closed 5/5 ; consignés P7b embarqués: GoAway level-triggered (CancellationToken), suspended_of strict, clamp TTL relay ≤39 — gate contrat 12 scénarios RED→GREEN (72 steps), witness_replay p4 3/3, GATE DÉPLOYÉ VERT (checkpoint public 6,1 s post-publish, vérif PyNaCl indépendante, plan final 0 écart)"

   cd ../../provider
   git add infra/terraform/modules/witness infra/terraform/modules/store-api infra/terraform/modules/relay infra/terraform/envs/prod e2e/features
   git commit -m "P5: module witness complet (Fargate desired 1, KMS, feed S3+CloudFront OAC witness.aithos.fr, policy observer t/*/manifest.json seul), stream NEW_AND_OLD_IMAGES sur heads, dépendance ciblée cluster_name (fin du churn depends_on module), e2e witness-p5; plans du gate"
   ```
   (`envs/prod/plan-p5.txt` = le plan lu, tel qu'appliqué.)
2. **Push GitHub** (ton geste) — la CI `provider-image.yml` rejouera toutes les
   features (dont les 12 witness) ; le job push store refera :prod+:sha store
   sans conséquence. Le tag witness du HEAD post-commit : à poser après push
   (le sha changera), motif §4.3 P7b.
3. Le fichier `.aws-env-2039` proposé pendant le blocage n'a jamais été créé —
   rien à nettoyer. `.aws-env` : expire seul.
4. ~~Rôle `aithos-ops-session-p5`~~ : **supprimé en clôture de session** (via le
   MCP aws-api : detach AdministratorAccess + delete-role) — vérifie au besoin :
   `aws iam get-role --role-name aithos-ops-session-p5` doit répondre NoSuchEntity.

## 5. Prochain lot (la route v1 : P5 ✓ → P3/P4 → ops → dashboard)

Lot B — client RemoteStore (P3/P4) : brancher
`aithos-gateway/src/store_adapter.rs` (stub) sur le wire A.2 réel (signature
d'enveloppes côté client, CAS A.5, cache A.6/§3.4), gates perf §3.6 mesurés
contre la prod (append p50 < 120 ms — les sondes HTTPS ordinaires passent du
sandbox). Puis lot C ops (quotas tenant relay B.4, quotas store, rétention/GC
30 j, DR testée, docs DPA/metadata, D3/D5/D6 du verdict ci-dessus) et lot D
dashboard (périmètre à proposer).

## 6. Environnement (delta session)

Sandbox reconstruit intégralement (~250 fichiers + extraction git-objects pour
les .feature — voir VERT-LOCAL §2) ; toolchain rustc stable + musl, terraform
1.13.5, behave/pynacl/base58/blake3 ; docker absent (API ECR) ; VM device morte ;
sondes ALPN toujours impossibles du sandbox (HTTPS ordinaire passe — toutes les
preuves witness sont passées d'ici) ; creds : rôle de session via MCP aws-api
(motif consigné §0.3), expirations gérées par re-assume.

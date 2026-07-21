# HANDOFF — Piste P / Lot A (P5 witness) : VERT LOCAL — gate déployé en attente de creds (2026-07-20)

Date : 2026-07-20 (soirée). Dépôts : code/aithos-core + provider/. État DISQUE = vérité.
Statut : le gate CONTRAT du témoin (C3, le dernier contrat non servi) est JOUÉ et
VERT — RED constaté d'abord (11 scénarios « Step doesn't match »), puis 11/11 GREEN
(66 steps) contre le VRAI `WitnessService`. Batterie locale complète VERTE. Terraform
écrit et validé (module witness complet : service Fargate + KMS + feed S3 + CloudFront
us-east-1 + DNS ; stream NEW_AND_OLD_IMAGES sur la table heads). **GO Mathieu obtenu
pour le gate déployé** (AskUserQuestion, en session) — bloqué au moment d'écrire sur
le renouvellement de la session SSO (creds expirés 19:32, l'export a resservi le
cache : il faut `aws sso login` avant `export-credentials`). Le commit reste TON geste.

Se lit avec HANDOFF-PROVIDER-P7B-BASCULE-RELAY-DONE-2026-07-20.md (état d'entrée) et
INFRA-PROVIDER.md annexe C (normative). Le handoff DONE du gate déployé suivra.

## 0. Séquence de la session (Mathieu présent par intermittence)

1. **Préalable §1 vérifié sur disque + prod** : commits P7+P7b FAITS (les deux dépôts,
   18:06, messages conformes) ; push GitHub PAS fait (origin/main au 19/07 — la CI
   `provider-image.yml` n'a pas rejoué les features P7b ; arbitrage Mathieu : il
   pousse, moi je pose les tags ECR `:96531e2`) ; prod à l'état de repos (table
   control 0 item, store :7 2/2, relay :8 1/1, digests conformes, wire sain).
2. **Arbitrages tranchés par Mathieu (AskUserQuestion, avant le code)** :
   ① déclencheur C.2 = **DynamoDB Streams sur la table heads** (le point de
   sérialisation A.5 ; NEW_AND_OLD_IMAGES — l'ancienne image distingue l'avance
   d'édition d'une réécriture gamma-only, qui n'émet RIEN) ;
   ② publication C.3 = **S3 + CloudFront sur witness.aithos.fr** (motif landings,
   OAC, bucket privé) ;
   ③ **desired_count = 1** (un seul écrivain du feed ; le témoin signe des
   observations, jamais de l'autorité — une interruption dégrade la fraîcheur) ;
   ④ clé = **KMS native Ed25519 sign-only per annexe C.1** (le prompt de reprise
   disait « Secrets Manager, motif relay-cert » — CONTREDIT l'annexe gravée ; le
   disque prime, tranché avec Mathieu).
3. **Workspace sandbox reconstruit** (~250 fichiers par lots de 50 ; les `.feature`
   refusent toujours le staging HTTP 400 — le tarball de P7b était PÉRIMÉ
   (antérieur au 9e scénario relay-control), contourné SANS re-tar Mathieu : les
   fichiers manquants extraits BYTE-EXACT des objets git des commits de 18:06
   (marche des trees + zlib). Batterie d'entrée : cargo check --locked EXIT=0.
4. **Gate contrat RED→GREEN** : `witness-service.feature`, 11 scénarios RED
   (« Step doesn't match any function ») → code → 11/11 GREEN (66 steps). Le
   harnais joue la VRAIE composition : vrai `WitnessService`, vrai `MemObjects`,
   feed mémoire aux sémantiques ETag de S3, fixtures = manifests p2 committés
   (m1/m2/m2b — la paire de fork EST la fixture d'équivocation), clé témoin p4,
   horloge injectée partout — jamais un sleep, jamais l'horloge murale.
5. **Code** (voir §3) : le témoin OBSERVE — événement heads → fetch du manifest
   dans le layout → chain hash RECALCULÉ → cohérent ou RIEN (pending sweep,
   jamais un checkpoint inventé, jamais une réparation du store). Idempotence
   C.2 déduite du FEED lui-même (re-lisible au boot, jamais une mémoire de
   process). Deux observations incompatibles = TOUTES DEUX émises (la paire
   publique EST la preuve C.4). keys.json auto-signé (format concret ADDITIF —
   l'annexe C.1 nomme le fichier sans fixer la forme ; gravure proposée §5).
6. **Terraform** : module witness devient un service complet ; stream sur heads ;
   envs/prod instancie (provider aliasé us-east-1 pour le cert CloudFront) ;
   `terraform fmt` OK, `validate` Success (init -backend=false).
7. **Préparé pour le gate déployé** : Dockerfile witness (FROM scratch, motif
   store-api), script push ECR couche unique déterministe (recréé, méthode
   gate 6 — copie sous v2/_transfer/), rejeu signé `deployed-replay-witness.py`
   (publish minimal draft.1 root-signé par le wire → poll du feed public →
   vérification PyNaCl INDÉPENDANTE : registre keys.json auto-signé, signature,
   manifest_hash recalculé, gamma_head copié, 2e édition = chaîne pas fork,
   latence mesurée), feature behave `witness-p5.feature` (surface publique sans
   clé) + steps préfixe « witness » (anti-AmbiguousStep).

## 1. Batterie locale (contre le disque reconstruit, 2026-07-20)

| Preuve | Résultat |
|---|---|
| cargo check --locked workspace | EXIT=0 (avec les nouvelles deps : aws-sdk-kms, aws-sdk-dynamodbstreams — Cargo.lock régénéré, à committer) |
| unités lib + bins | 54 + 1 |
| cucumber | store 146/146 (931 steps), tunnel 12/12 (40), relay 27/27 (151), **witness 11/11 (66) — RED constaté avant** |
| replays byte-exact | vectors_replay 5/5, tunnel p3 2/2, sni p5 1/1, acme p6 2/2, relay_handshake 4/4, **witness p4 3/3** (vecteurs INTOUCHÉS) |
| musl static-pie | 4/4 : store-api, relay, store-admin, **witness (16,7 Mo)** |
| gardes fail-closed bin witness | **5/5 exit 2** (kms sans key id ; signer inconnu ; feed s3 sans bucket ; heads table absente ; seed hex invalide) + boot nominal sain (warns dev, reconcile skipped fail-closed, tick vivant) |
| clippy / fmt | 0 dans le code du lot ; 1 warning lib PRÉ-EXISTANT (pathmap.rs:532, clippy plus récent) + diffs rustfmt PRÉ-EXISTANTS (store_admin.rs:640, passthrough.rs:190, cucumber_relay.rs:1161) — PAS touchés (état committé), consignés |
| terraform | fmt OK (récursif), validate Success |
| behave dry-run | 37 scénarios, 107 steps liés, 0 AmbiguousStep |

## 2. Consigné SANS graver (observations de session)

- **Tarball features périmé** : `features-rust.tgz` (15:11) antérieur au 9e
  scénario P7b (15:51) et sans les features e2e. Contournement NOUVEAU sans
  intervention Mathieu : extraction byte-exacte depuis les objets git loose
  (commit → trees → blobs, zlib) — reproductible tant que les commits sont récents
  (objets non packés).
- **« tunnel 26/26 » du handoff P7b introuvable** : tunnel-register.feature = 12
  scénarios (12/12 ici, fichier du disque == blob committé). Probablement un
  compte agrégé ou une coquille — à éclaircir si ça re-compte.
- **keys.json : format concret défini ici** (`{"aithos-witness-keys":
  "1.0.0-draft.1", keys:[…], witness_key, signature}` auto-signé, vérif =
  version+alg épinglés, clé signataire ∈ keys, signature §01.4). ADDITIF à C.1
  (qui nomme le fichier et son signataire sans forme) — gravure proposée.
- **Classes de cache feed choisies** (C.3 ne fixe que le feed 60 s) :
  `<did>.jsonl` et `keys.json` → `public, max-age=60` ; `roots/<date>.json`
  (adressé par date, jamais réécrit) → `public, max-age=31536000, immutable` ;
  Content-Type `application/x-ndjson` pour les feeds. À graver si validé au gate.
- **Checkpoint.did = la clé de ligne heads** (le point de sérialisation), jamais
  relu du manifest (qui n'embarque pas de champ did).
- **SSO piège** : `export-credentials` SANS `aws sso login` préalable ressert le
  cache expiré (mtime avance, contenu identique) — vérifier
  AWS_CREDENTIAL_EXPIRATION après tout export.
- Le sha court des commits P7/P7b : **96531e2** (aithos-core) / 0da8ffc
  (provider). Tags ECR store+relay `:96531e2` à poser dès creds valides
  (arbitrage : moi ; le push GitHub : Mathieu).

## 3. Livré (fichiers, write-back disque fait)

**code/aithos-core** : rust/crates/aithos-provider/src/witness_service.rs
(NOUVEAU : seam FeedStore Mem+S3, WitnessService — on_event/reconcile/heartbeat/
sweep_pending/publish_daily_root/publish_keys), src/witness.rs (WitnessKeys +
build/verify_keys_doc, ADDITIF), src/bin/witness.rs (NOUVEAU : KmsWitnessSigner
ED25519_SHA_512 RAW via thread signeur dédié, poller DynamoDB Streams LATEST +
boot reconcile + tick pending/rollover, gardes fail-closed), src/lib.rs (module),
Cargo.toml (deps + bin + [[test]] cucumber_witness), rust/Cargo.toml (workspace
deps kms/dynamodbstreams), rust/Cargo.lock, tests/cucumber_witness.rs (NOUVEAU),
tests/features/witness/witness-service.feature (NOUVEAU, 11 scénarios),
docker/witness.Dockerfile (NOUVEAU), vectors/deployed-replay-witness.py
(NOUVEAU — ne touche AUCUN vecteur gelé), ce handoff.

**provider** : infra/terraform/modules/witness/{main,variables,outputs,versions}.tf
(retravaillés : le seam witness_task_role_name retiré — le module crée son rôle)
+ {service,cdn}.tf (NOUVEAUX : ECR, service Fargate desired 1 sans ingress,
policy observer minimale — heads rows+stream, s3 `t/*/manifest.json` SEUL —,
cert ACM us-east-1, CloudFront OAC, bucket policy), modules/store-api/main.tf
(stream NEW_AND_OLD_IMAGES sur heads) + outputs.tf (heads_table_arn,
heads_stream_arn), envs/prod/main.tf (module witness + witness_alias dns) +
providers.tf (alias us_east_1), e2e/features/witness-p5.feature +
steps/witness_steps.py (NOUVEAUX).

**v2/_transfer** : push-witness-image.py (l'outil opérateur, hors dépôts —
recréable, méthode gate 6).

## 4. Reste pour clore le lot A (la session continue si possible)

1. **Creds** : `aws sso login --profile aithos-prod` puis export → .aws-env.
2. Tags ECR `:96531e2` store+relay (API, non destructif).
3. **Gate déployé** (GO Mathieu ACQUIS) : terraform init (backend S3 :
   aithos-landings-tfstate-128066560720 / provider/envs/prod/terraform.tfstate /
   eu-west-3 / use_lockfile) + plan avec LES 4 -var (terraform_state_bucket_name,
   github_repository_infra=placeholder/aithos-provider,
   github_repository_code=placeholder/aithos-core, desired_count=2) → LECTURE
   INTÉGRALE → apply → push witness :prod (script) + tag :96531e2 → attente
   distribution CloudFront + cert us-east-1 → tenant `replay-w-20260720`
   (aithos-store-admin create + bind-did, creds opérateur, env
   AITHOS_ADMIN_CONTROL_TABLE=aithos-provider-prod-control) →
   `deployed-replay-witness.py` (les preuves + latences) + behave
   witness-p5 → purge (AITHOS_ADMIN_OBJECTS_BUCKET/HEADS_TABLE en plus) →
   table 0 item → plan final -detailed-exitcode 0.
4. Témoin de gate adversarial en agent (rituel P7b) ; gravures INFRA-PROVIDER
   (§7 note P5 RÉALISÉ + additifs C.1 keys.json / classes de cache si validés) ;
   handoff DONE ; blocs de commit pour Mathieu.

## 5. Environnement (delta session)

VM device MORTE (device_bash refuse dès le boot) ; staging `.feature` HTTP 400
(contournement git-objects, §2) ; MCP remote-devices a flappé une fois
(reconnecté seul) ; toolchain sandbox : rustc stable + target musl (rustup),
terraform 1.13.5, behave/pynacl/blake3/base58 pip ; docker daemon absent (push
par API ECR) ; GitHub API bloquée du sandbox (vérif CI impossible d'ici — l'état
push lu dans .git/refs).

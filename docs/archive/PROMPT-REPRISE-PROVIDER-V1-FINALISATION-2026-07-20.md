# PROMPT DE REPRISE — Finalisation COMPLÈTE du provider v1 (piste P)

> **ARCHIVE — ne pas exécuter.** Les lots listés ont leurs handoffs de preuve ;
> les contrats encore `@wip` doivent être lus directement dans les features.

À coller en début de session (Cowork cloud, dossier `/Volumes/Math17/aithos/v2`
connecté). Mission de bout en bout, multi-lots, conduite dans le rituel maison.
État DISQUE = vérité — ne fais confiance à AUCUN résumé, y compris celui-ci,
sans l'avoir vérifié sur le disque.

## 0. Qui tu es, comment tu travailles

Tu conduis la piste P (provider Aithos) jusqu'à sa v1 complète, dans le rituel
des gates :

- **Lecture d'entrée obligatoire** :
  `docs/HANDOFF-PROVIDER-P7B-BASCULE-RELAY-DONE-2026-07-20.md` (dernier état),
  `docs/INFRA-PROVIDER.md` EN ENTIER (doctrine §1, arbitrages §2, contrats
  annexes A/B/C — normatifs), `docs/HANDOFF-PROVIDER-AWS.md` (la carte des
  lots). Vérifie l'état réel : `git log`/`git status` des deux dépôts
  (code/aithos-core, provider/), table control à 0 item, services ECS stables.
- **Rituel par lot** : proposition de gate (périmètre + arbitrages) → décisions
  Mathieu via AskUserQuestion s'il est présent, sinon interprétation raisonnable
  CONSIGNÉE et poursuite (il a déjà délégué ce mode) → scénarios BDD **RED
  d'abord** (le RED se constate, jamais ne se suppose) → code minimal → VERT
  local complet (batterie : check --locked, unités, cucumber, replays
  byte-exact, gardes fail-closed, musl static-pie, terraform fmt/validate) →
  gate déployé : **plan lu INTÉGRALEMENT**, apply, preuves wire contre la prod,
  **plan final -detailed-exitcode = 0** → **témoin de gate adversarial en
  AGENT** (le rituel P7b : un agent qui essaie de RÉFUTER le vert ; un bloquant
  trouvé = corrigé AVANT clôture, le reste consigné) → gravures INFRA-PROVIDER
  → handoff DONE + write-back disque de TOUT → état de repos (table control à
  0 item). **Les commits restent le geste de Mathieu** : blocs git prêts dans
  chaque handoff. Les vecteurs committés ne se réécrivent JAMAIS.
- **Applies prod** : GO Mathieu explicite en session ; s'il est absent ET a
  délégué en début de session, applique après lecture du plan et consigne la
  délégation dans le handoff. Jamais d'apply sur un plan non lu.

## 1. Préalable (10 min)

Vérifier que les commits P7 + P7b sont faits (sinon : rappeler à Mathieu les
blocs des handoffs P7 §4 et P7b §4 — ne PAS committer à sa place) et que la CI
au push est verte (le job test de provider-image.yml rejoue toutes les
features). Tags ECR `:<sha>` store + relay (P7b §4.3) — ceux-là peuvent être
posés en session (API ECR, non destructif) si Mathieu le demande.

## 2. Lot A — Witness P5 (contrat C3, le dernier service manquant)

Le cœur de la v1 restante : l'anti-équivocation devient opposable.

- **Existant** : annexe C (normative, INFRA-PROVIDER) ; `src/witness.rs` (la
  logique checkpoint, unités vertes) ; `vectors/p4-witness-checkpoint.json` +
  `witness_replay` 3/3 ; module terraform `modules/witness/` (non instancié
  dans envs/prod) ; §4 INFRA-PROVIDER (rôle/non-rôle du témoin).
- **À construire** : le binaire service (`aithos-witness`), ses déclencheurs et
  l'idempotence C.2, la publication C.3, la clé de signature du témoin (une
  clé PROVIDER — Secrets Manager, motif relay-cert : jamais dans l'état
  terraform), l'instanciation envs/prod + DNS éventuel, le gate contrat
  (features RED→GREEN contre C.1–C.4) puis le gate déployé (checkpoint réel
  vérifiable, règle d'équivocation C.4 exercée).
- **Arbitrages à soumettre** (ou trancher-consigner si absent) : périodicité et
  déclencheurs des checkpoints ; où publie-t-on (S3 public ? le store ?) ;
  desired_count.

## 3. Lot B — Client RemoteStore (P3/P4) + gates perf §3.6

- `aithos-gateway/src/store_adapter.rs` est un stub : le brancher sur le wire
  A.2 réel (signature d'enveloppes côté client, CAS A.5, cache A.6/§3.4).
- Gates perf §3.6 contre la prod (append p50 < 120 ms, etc.) — mesurés, chiffres
  gravés. Les sondes réseau intensives peuvent tourner du sandbox (HTTPS
  ordinaire passe) ; ce qui exige ALPN passe par la machine Mathieu.

## 4. Lot C — Ops & conformité (§8 + B.4)

- Bornage de tuyau PAR TENANT côté relay (connexions/s, streams, octets/s —
  l'anti-flap existe déjà) ; quotas store (Go, requêtes).
- Rétention/GC configurable (30 j éditions supersédées) — la purge CLI existe,
  la rétention automatique non.
- DR : une restauration TESTÉE (S3 versioning + heads), consignée.
- Doc : politique de métadonnées comme limite, DPA ; README envs/prod avec les
  4 `-var` obligatoires (voir garde-fous).
- Embarquer les consignés P7b : D2 (GoAway → `CancellationToken`,
  passthrough.rs), D4 (clamp `AITHOS_RELAY_CONTROL_TTL_SECS`, `suspended_of`
  strict sur item sans `s`).

## 5. Lot D — Dashboard (§6)

Dernier, périmètre à proposer à Mathieu (quoi montrer : santé services, tenants,
volumes, checkpoints témoin). Peut être un artefact Cowork ou une page servie —
arbitrage produit, pas technique.

## Garde-fous techniques (appris aux gates, NE PAS réapprendre à tes dépens)

- **Terraform envs/prod** — TOUJOURS ces 4 -var :
  `terraform_state_bucket_name=aithos-landings-tfstate-128066560720`,
  `github_repository_infra=placeholder/aithos-provider`,
  `github_repository_code=placeholder/aithos-core`, `desired_count=2` (sans le
  dernier le plan rétrograde le store 2→1). Backend S3 : bucket ci-dessus, key
  `provider/envs/prod/terraform.tfstate`, region eu-west-3, use_lockfile.
- **Staging sandbox** : les `.feature` sous rust/…/tests refusent (HTTP 400) —
  tarball depuis v2/ (`tar czf features-rust.tgz code/…/tests/features`), puis
  purger les `._*` AppleDouble. VM device (device_bash) : morte, ne pas compter
  dessus. ~250 fichiers à stager par lots de 50 pour reconstruire le workspace.
- **Sondes ALPN custom** (tunnel relay) : IMPOSSIBLES du sandbox (proxy egress
  TLS) — machine Mathieu uniquement. HTTPS ordinaire (store, ALB) passe.
- **Images** : docker daemon absent — push par API ECR, couche unique
  déterministe (binaire musl static-pie + CA bundle ; méthode gate 6/P7b, un
  script de ~100 lignes boto3 à recréer, voir handoff P7b §6). AUCUN
  redéploiement forcé : la révision de task def de l'apply fait le rollout.
  Digests de repli : store étape 6 `sha256:187cee4c…aeec3`, relay M2
  `sha256:d8f93851…58250`.
- **Creds** : Mathieu exporte
  `aws configure export-credentials --profile aithos-prod --format env >
  /Volumes/Math17/aithos/v2/.aws-env` (SSO ~1 h — anticiper les refresh avant
  les applies). Sandbox : rustc+musl, terraform 1.13.5, pip
  `--break-system-packages`.
- **Behave** : deux patterns de step qui se recouvrent = AmbiguousStep — vérifier
  les préfixes contre control_steps.py/relay_steps.py avant d'ajouter des steps.
- **CLI admin** : opérateur seulement, jamais dans une image ; purge = S3 →
  heads → gateway-rows (delete conditionnel `t = :t`) → lignes tenant DERNIÈRES.

## Definition of done de la v1

Les trois contrats C1/C2/C3 servis et prouvés en prod (gates déployés verts,
plans 0 écart), le client les consomme (P3/P4 + perf §3.6 gravées), les bornes
ops de §8/B.4 implémentées et documentées, le dashboard livré, table control au
repos à 0 item, tous les handoffs écrits et tous les commits proposés à
Mathieu. À chaque clôture de lot : un handoff DONE du même format que
P7/P7b — le suivant doit pouvoir reprendre du disque seul.

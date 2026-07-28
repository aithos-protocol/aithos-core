# PROMPT DE REPRISE — Lot B, partie 2 : GATE P3 (DEMO-LEA remote) puis P4 (sync/batch + cdn-public + perf §3.6)

> **ARCHIVE — ne pas exécuter.** Les deux gates ont été joués.

À coller en début de session (Cowork cloud, dossier /Volumes/Math17/aithos/v2
connecté). État DISQUE = vérité — ne fais confiance à AUCUN résumé, y compris
celui-ci, sans vérification disque.

## 0. Rituel (inchangé — le rituel maison des gates)

Lecture d'entrée : docs/HANDOFF-PROVIDER-P3-JALON-CLIENT-2026-07-21.md (le
dernier état — TOUT y est), docs/INFRA-PROVIDER.md EN ENTIER (annexe A),
docs/HANDOFF-PROVIDER-P5-WITNESS-DONE-2026-07-20.md + ADDENDUM. Vérifier :
git log des deux dépôts (attendu : core `294df36` P5 + `bb31d71` P3-jalon ;
provider `46399b0` P5 — les fichiers POST-bb31d71 du handoff §2 sont sur le
disque, NON committés, ils partent au commit de gate), tables control/heads à
0, services stables (store 2/2, relay 1/1, witness 1/1),
witness.aithos.fr/keys.json 200. Puis : proposition de gate → décisions
Mathieu (AskUserQuestion) ou interprétation raisonnable CONSIGNÉE s'il est
absent (mode délégué) → BDD/test RED constaté → code minimal → VERT local
complet → gate déployé (GO Mathieu ; plans lus INTÉGRALEMENT) → témoin
adversarial en AGENT (un bloquant = corrigé avant clôture) → gravures →
handoff DONE → write-back disque de TOUT → repos (tables 0). Commits = geste
de Mathieu. Vecteurs committés INTOUCHABLES.

## 1. Ce qui est DÉJÀ acquis (ne pas refaire, ne pas re-arbitrer)

- Client `RemoteStore` VERT : contrat 16/16 (cucumber_remote, vrai service
  sur socket), spike e2e journal remote 2/2 (mode B via Bridge complet,
  mode A répliqué/relu) — `e2e_journal_remote.rs` contient la mécanique de
  seed owner (historique d'éditions par slots manifests/<h>.json).
- Arbitrages Mathieu PRIS (2026-07-21) : ① ureq+rustls ; ② seam signeur
  keyholder ; ③ perf officielle = machine Mathieu, sandbox indicatif ;
  ④ deux gates un lot ; ⑤ cdn-public dans ce lot ; + HYBRIDE + micro-redline
  A.1 additive (header/root de zone servables, gateway/** et manifests/*
  restent au sidecar du pod), gravure annexe + BDD sans vecteur p10.
- Batterie verte : check --locked, store 150/150, tunnel 12/12, relay 27/27,
  witness 12/12, replays 5/5+2/2+1+2/2+4/4+3/3, remote 16/16, gateway
  152/152 + 85, e2e_demo_lea (fs) OK.

## 2. Le lot restant

- **Gate P3** : DEMO-LEA rejouée À L'IDENTIQUE avec journal.store = remote
  (mode B) — paramétrer e2e_demo_lea.rs (les deux variantes DANS le même
  fichier : helpers partagés, beats identiques) : service provider réel
  in-process (dev-dep en place), enrôlement tenant+DID, seed owner par le
  wire (motif du spike), gateway BINAIRE avec yaml
  `journal: store: {kind: remote, url, tenant, did, mandate: [<pen>], local: <dir>}`
  (le pen = memory_mandate imprimé par owner-init-journal) ; un contexte
  ventes en mode A (`kind: replicated`) répliqué et RELU depuis le store ;
  assertions journal finales par lecteur REMOTE. Décider en route : la
  surface client `replicate_history` (consigné C6 du handoff).
- **Gate P3 déployé** (OBLIGATOIRE — la redline A.1, le 304, le treillis et
  gamma-appendeur sont des changements SERVICE) : image store par API ECR
  (couche unique, modèle v2/_transfer/push-witness-image.py), plan lu, apply,
  rejeu wire sur tenant de rejeu à DID FRAIS (⚠ D8 : jamais re-publier une
  hauteur déjà observée par le témoin sur un DID passé au témoin), suite
  behave, plan final -detailed-exitcode 0, purge, tables 0.
- **Témoin adversarial** : soumettre EXPLICITEMENT les consignés C1–C6 du
  handoff (surtout C1 gamma-appendeur = confidentialité du log, et C2
  treillis lecture). Un bloquant = corrigé avant clôture.
- **Gravures INFRA-PROVIDER** (après verdict) : micro-redline A.1, 304 (A.6),
  A.3 treillis+gamma selon verdict, convention JCS client, note §3.5 modes
  A/B réalisés (sidecar, réplication asynchrone).
- **P4** : client POST /sync {have_edition} + /batch (le serveur les sert,
  p9 les fige — côté client : intégration + BDD dans cucumber_remote) ;
  module Terraform `cdn-public` (CloudFront de la zone publique du store —
  arbitrage ⑤) ; **gates perf §3.6 MESURÉS** : cache hit p50 < 5 ms, sync
  froid 1 000 sections < 2 s, append mode B p50 < 120 ms depuis l'Europe,
  GET immuable CloudFront p50 < 30 ms — script de bench FOURNI à Mathieu
  (officiel depuis sa machine, motif suites déployées P7b), pré-mesure
  sandbox indicative ; chiffres gravés dans INFRA-PROVIDER §3.6.

## 3. Garde-fous techniques (appris — NE PAS réapprendre)

- **Reconstruction sandbox** : ~250 fichiers par lots de 50
  (device_stage_files) ; les `.feature` refusent le staging (HTTP 400) →
  extraction BYTE-EXACT depuis .git/objects (commit→trees→blobs, zlib) —
  le sens COMMIT (device_commit_files) écrit les .feature sans blocage ;
  vecteurs requis : tous les p*.json + a1-genesis + cb2-draft2-carriers +
  g*/f*/h*/i1/e1/c1/b2/eplus/fplus/gplus. Batterie d'entrée AVANT le
  premier RED (compteurs §1 du handoff).
- **Piège du PONT** : copies stagées PÉRIMÉES resservies (vu sur les
  reflogs) — vérifier le CONTENU du fichier stagé, jamais le mtime ; pour
  un objet git, viser un chemin jamais stagé.
- **VM device MORTE** (device_bash indisponible) ; les commits git = gestes
  de Mathieu (blocs prêts dans le handoff de gate).
- **Creds AWS** : `aws sso login` par Mathieu puis MCP aws-api
  (`--profile aithos-prod`) — la voie fichier .aws-env expire (~1 h) et le
  pont peut la resservir périmée. Pour un apply : au besoin rôle de session
  temporaire (motif P5), DETACH+DELETE à la clôture.
- **Terraform envs/prod** : TOUJOURS les 4 -var
  (terraform_state_bucket_name=aithos-landings-tfstate-128066560720,
  github_repository_infra=placeholder/aithos-provider,
  github_repository_code=placeholder/aithos-core, desired_count=2) ;
  backend S3 key provider/envs/prod/terraform.tfstate eu-west-3
  use_lockfile. JAMAIS de depends_on MODULE (dépendance ciblée par output).
- **Images** : docker absent → push par API ECR, couche unique déterministe ;
  digests de repli au handoff P5 ; cdn-public : certs CloudFront en
  us-east-1 (motif landings/witness).
- **Sondes ALPN** : impossibles du sandbox ; HTTPS ordinaire passe (toutes
  les preuves store/witness passent d'ici) ; behave : préfixes de steps
  uniques (AmbiguousStep).
- **Cucumber/Gherkin** : pas d'échappements \" dans les {string} (vu au
  jalon — corps JSON simples) ; harnais remote = compteurs PAR MÉTHODE.
- Container recyclable entre tours : write-back disque À CHAQUE jalon.
- `AITHOS_REMOTE_DEBUG=1` = tap de debug du client (requêtes refusées).

## 4. Après le lot B

Lot C ops (§8 + B.4 : bornage tuyau par tenant relay, quotas store,
rétention/GC 30 j configurable, DR testée, docs métadonnées/DPA, README
envs/prod ; + consignés P5 : D3 signature-avant-dedup, D5 kms:SigningAlgorithm
+ key policy, D6 append-only par IAM ; + C4 du handoff si non résolu au gate :
re-dérivation du sidecar pour runner éphémère) ; puis Lot D dashboard
(périmètre à proposer). Definition of done v1 :
PROMPT-REPRISE-PROVIDER-V1-FINALISATION-2026-07-20.md.

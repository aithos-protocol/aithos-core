# PROMPT DE REPRISE — Lot B : client RemoteStore (P3/P4) + gates perf §3.6
À coller en début de session (Cowork cloud, dossier /Volumes/Math17/aithos/v2 connecté).
État DISQUE = vérité — ne fais confiance à AUCUN résumé, y compris celui-ci, sans vérification disque.

## 0. Rituel (inchangé — le rituel maison des gates)
Lecture d'entrée : docs/HANDOFF-PROVIDER-P5-WITNESS-DONE-2026-07-20.md (+ son ADDENDUM
du 2026-07-21) — le dernier état ; docs/INFRA-PROVIDER.md EN ENTIER (annexe A = le wire
que le client doit parler : A.2 enveloppe, A.5 CAS, A.6 cache) ; HANDOFF-PROVIDER-AWS.md
(P3/P4). Vérifier : git log des deux dépôts (les commits P5 §4 du handoff DONE faits ?
sinon les rappeler à Mathieu), tables control/heads à 0, services stables (store :7 2/2,
relay :8 1/1, witness 1/1), witness.aithos.fr/keys.json 200.
Puis : proposition de gate (périmètre + arbitrages) → décisions Mathieu (AskUserQuestion)
ou interprétation raisonnable CONSIGNÉE s'il est absent (mode délégué) → BDD RED constaté →
code minimal → VERT local complet → gate déployé (GO Mathieu ; plans lus INTÉGRALEMENT) →
témoin adversarial en agent (un bloquant = corrigé avant clôture) → gravures → handoff DONE
→ write-back disque de TOUT → repos (tables 0). Commits = geste de Mathieu. Vecteurs
committés INTOUCHABLES.

## 1. Le lot
- **P3 — client `RemoteStore`** dans aithos-bundle (feature `remote`) : impl du trait
  `Store` (SYNC : get/put/list, std::io::Result — voir aithos-bundle/src/lib.rs) parlant
  le wire A.2 : enveloppe X-Aithos-Auth signée (JCS, body_b3 BLAKE3, nonce, at), PUT
  d'artefacts, publish manifest + append gamma sous CAS A.5 (If-Head, rejouer le 409 →
  rebase), retries+backoff, cache local immuable (A.6 : immutable vs no-store vs
  revalidate ETag). `aithos-gateway/src/store_adapter.rs` (stub Fs/Mem) gagne la variante
  `remote { url, tenant }` (le refus fail-closed du s3 reste) ; config par contexte/journal
  (config.rs StoreConfig) ; décorateur mode A (réplication asynchrone post-publish).
  **Gate P3 : DEMO-LEA rejouée à l'identique avec journal.store = remote (mode B) ; un
  contexte mode A répliqué et relu depuis le store.**
- **P4 — sync/pack + perf** : POST /sync {have_edition} (client), get_many/batch ;
  gates perf §3.6 MESURÉS : cache hit p50 < 5 ms, sync froid 1 000 sections < 2 s,
  append mode B p50 < 120 ms depuis l'Europe, GET immuable CloudFront p50 < 30 ms.
  Chiffres gravés dans INFRA-PROVIDER.

## 2. Arbitrages à soumettre (ou trancher-consigner si absent)
① Client HTTP du RemoteStore : minimal bloquant (ureq+rustls — motif pod_stub « ce qui
   est signé = ce qui est envoyé », deps minimales pour une lib cliente) vs reqwest.
   Le trait Store est SYNC — pas de runtime dans la lib.
② Identité signataire du client : seam signeur injecté (owner #content / clé gateway
   selon le mode), jamais une clé en dur ; où la config la prend (keyholder existant).
③ Origine des mesures perf : le sandbox est LOIN de eu-west-3 (TLS ~0,64 s à froid,
   RTT estimé ~100 ms — « depuis l'Europe » n'y est pas honorable). Officiel = machine
   Mathieu (script fourni, motif suites déployées P7b) ; sandbox = pré-mesure indicative.
④ Périmètre : P3 seul puis P4, ou les deux dans le lot (recommandé : deux gates, un lot).
⑤ CloudFront du public store (cdn-public) : requis pour « GET immuable < 30 ms » — dans
   ce lot ou consigné au lot ops ?

## 3. Garde-fous techniques (appris aux gates P5/P7b — NE PAS réapprendre)
- **Reconstruction sandbox** : ~250 fichiers par lots de 50 (device_stage_files) ; les
  `.feature` refusent le staging (HTTP 400) → extraction BYTE-EXACT depuis les objets
  git loose (.git/objects : commit→trees→blobs, zlib — méthode P5, marche tant que les
  commits sont récents) ; vecteurs requis par les tests : TOUS les p*.json + a1-genesis
  + cb2-draft2-carriers (1,2 Mo) + les g*/f*/h*/i1/e1/c1/b2/eplus/fplus/gplus.
- **Batterie d'entrée AVANT le premier RED** : check --locked EXIT=0, cucumber store
  146/146 + tunnel 12/12 + relay 27/27 + witness 12/12, replays 5/5+2/2+1+2/2+4/4+3/3.
- **Creds AWS** : .aws-env expire (~1 h). Piège 1 : export-credentials SANS sso login
  ressert le cache périmé. Piège 2 : le PONT peut resservir une copie stagée périmée
  d'un fichier de même taille — vérifier AWS_CREDENTIAL_EXPIRATION du contenu STAGÉ.
  Voie robuste (motif P5 consigné) : MCP aws-api de la machine Mathieu
  (`--profile aithos-prod`) → create-role temporaire (trust account-root, Admin, 1 h)
  → assume-role → creds dans le sandbox ; DETACH+DELETE le rôle à la clôture.
- **Terraform envs/prod** : TOUJOURS les 4 -var (terraform_state_bucket_name=
  aithos-landings-tfstate-128066560720, github_repository_infra=placeholder/
  aithos-provider, github_repository_code=placeholder/aithos-core, desired_count=2) ;
  backend S3 key provider/envs/prod/terraform.tfstate eu-west-3 use_lockfile. JAMAIS
  de depends_on MODULE (churn de task defs à contenu identique — piège vu au P5, corrigé
  par dépendance ciblée cluster_name).
- **Images** : docker absent → push par API ECR, couche unique déterministe
  (v2/_transfer/push-witness-image.py = le modèle) ; digests de repli au handoff P5.
- **Tenant de rejeu** : create/bind par aithos-store-admin (AITHOS_ADMIN_CONTROL_TABLE=
  aithos-provider-prod-control ; purge exige aussi OBJECTS_BUCKET+HEADS_TABLE), purge
  après. ⚠ D8 : re-publier une hauteur déjà observée sur un DID déjà passé au témoin
  = VRAIE équivocation publique — un rejeu store+witness utilise un DID frais, ou des
  hauteurs jamais observées, ou assume la paire C.4.
- **Sondes ALPN** : impossibles du sandbox ; HTTPS ordinaire passe. behave : préfixes
  de steps uniques (AmbiguousStep).
- Container recyclable entre tours : write-back disque À CHAQUE jalon, pas seulement
  à la clôture.

## 4. Après le lot B
Lot C ops (§8 + B.4 : bornage tuyau par tenant relay, quotas store, rétention/GC 30 j
configurable, DR testée, docs métadonnées/DPA, README envs/prod avec les 4 -var ;
+ les consignés du verdict P5 : D3 signature-avant-dedup, D5 kms:SigningAlgorithm non
épinglé + key policy, D6 append-only par IAM) ; puis Lot D dashboard (périmètre à
proposer). Definition of done v1 : voir PROMPT-REPRISE-PROVIDER-V1-FINALISATION.

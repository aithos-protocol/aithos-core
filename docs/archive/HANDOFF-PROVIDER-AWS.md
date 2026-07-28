# HANDOFF — Piste P : le provider Aithos sur AWS

> **ARCHIVE — journal de construction Provider.** Les lots M2/P2/P3/P4/P5/P7
> ont depuis leurs gates `DONE`. Pour l'architecture encore opposable, utiliser
> `INFRA-PROVIDER.md` ; pour l'état, utiliser `README.md` et les tests actuels.

## État express — 2026-07-18 — M2 finalisé « prod stable » : /acme/txt (B.5) + cert relais OUT-OF-BAND + keepalive TCP — vert-local, STOP gate plan

**Session (reprise M2, les 3 dérives du 07-17 tranchées par Mathieu et
APPLIQUÉES).** Rituel tenu : feature + vecteur AVANT le code, rejeu
byte-exact contre le vrai binaire, smoke vrais-binaires, STOP au gate.
Aucun apply, aucun commit, `desired_count` intouché, P2 intouché.

- **API B.5 `/acme/txt` implémentée (feature-first)** — le stopgap « cert
  pod manuel » disparaît. Nouveau vecteur **`p6-acme-txt.json` (37 cas,
  séquence stateful)** : générateur + simulateur indépendants
  (`gen-p.py`/`verify-p.py`), **p1..p5 byte-identiques** ; BDD
  `tests/features/store/store-acme.feature` (**32 scénarios**, harnais
  in-process, effets DNS assertés sur backend mémoire inspectable) ;
  rejeu **byte-exact contre le vrai binaire** (`tests/acme_replay.rs`,
  3 plans : normal + binding suspendu + tenant suspendu). Ordre gravé
  (exception B.5) : forme (key multibase + mandate [] = FORME) →
  host/method/path → body_b3 → skew → nonce → signature sous
  `gateway_pub` → verbe (PUT/DELETE sinon not_covered) → forme du corps
  (`{hostname, value}` fermé, LDH minuscule strict, value 1..255
  base64url) → mapping (resolve → suspended → tenant → hostname) →
  anti-abus (≤ 10 PUT/h/hostname, compté APRÈS autorisation) → effet.
  `mapping_mismatch` rejoint le registre HTTP (403). Code : `acme.rs` +
  `dns.rs` (seam TXT : Route53 UPSERT/DELETE idempotent, mémoire, `off`
  fail-closed 503), purge serveur 10 min (tâche + testée à horloge
  injectée), logs `class=acme` (jamais le hostname ni la value — A.8).
  Bootstrap store (`replay.json`) gagne le mapping tunnel démo,
  drift-guardé contre p6. e2e sans-clé : `store-acme-p6.feature` (behave).
- **`aithos-pod-stub` : mode ACME (`POD_ACME=1`)** — la moitié CLIENT de
  B.5 : ordre LE (ou staging), challenge posé par `PUT /acme/txt` signé
  de la clé gateway (zéro secret nouveau), CSR et **clé privée générées
  localement — la clé ne quitte jamais le pod (A3)**, cache local,
  DELETE de nettoyage (purge serveur en backstop). Client HTTP minimal
  (ce qui est signé = ce qui est envoyé). À exercer en vrai à l'étape 4
  du gate (vraie machine, LE staging d'abord).
- **Keepalive B.3 appliqué (redline gravée dans INFRA-PROVIDER annexe
  B.3)** : `SO_KEEPALIVE` idle court (30 s / 10 s ×3) posé côté relais
  (socket accepté — cible = socket tunnel ; inerte ailleurs) ET côté
  pod-stub (socket sortant) ; module `keepalive.rs`, valeurs relues du
  noyau en test. yamux reste `Config::default()`, **aucun yamux
  épinglé**. Scénario pod-FIGÉ toujours `@wip @draft2`.
- **Cert relais OUT-OF-BAND (Terraform retravaillé)** : `modules/relay-cert`
  = **2 secrets Secrets Manager VIDES + ARNs/names en output, rien
  d'autre** — la clé ne transite JAMAIS l'état. Providers
  `vancluever/acme` + `hashicorp/tls` retirés de `envs/prod`
  (required_providers, bloc provider, vars `acme_email`/`acme_server_url`)
  → **`terraform init/validate/plan` relançables depuis le sandbox (aws
  seul)** — vérifié : `fmt` OK, `init -backend=false` OK, `validate` OK.
  `module.relay_cert` désormais inconditionnel ; task role du store
  reçoit la **policy ACME-TXT** (module dns, TXT + `_acme-challenge.*` +
  zone mcp seulement) + env `AITHOS_STORE_DNS_BACKEND=route53` /
  `AITHOS_STORE_ACME_ZONE_ID` (câblés zone mcp ; sans eux le binaire
  démarre backend `off` → /acme refuse 503, data plane intact).
- **INFRA-PROVIDER gravé (les 2 notes du mandat)** : §7 note
  Lambda-vs-Fargate (store à trancher au gate P2 ; relais Fargate quoi
  qu'il arrive) ; annexe B.3 redline keepalive (M2 = EOF + TCP keepalive ;
  PING applicatif = draft.2 via B.6).
- **Compteurs locaux (tout vert)** : **37 unités** + **66 BDD store**
  (34 + 32 acme) + 12 BDD tunnel + 18 BDD passthrough + rejeux
  p1/p3/p4/p5 byte-exact + **p6 byte-exact contre le vrai binaire (2
  tests, 3 plans)** + 4 handshake + witness verts ; **clippy 0, fmt OK** ;
  vecteurs Python `gen-p.py`+`verify-p.py` verts (P1..P6) ; smoke
  vrais-binaires relais+pod-stub vert (reach servi par le pod, ghost/
  nosni/plain silencieux, register 4 verdicts, logs 0 octet applicatif) ;
  behave dry-run vert (25 scénarios, 66 steps liés).

**Gate déploiement M2 (STOP ici — plan à lire, apply sur parole) :**
1. `terraform plan` depuis le sandbox (aws seul, creds Mathieu) —
   artefact + lecture intégrale. Attendu : 2 secrets vides + attache
   policy ACME-TXT + révisions task defs (store : env B.5 ; relay :
   tunnel name + secrets `*_PEM` + grant exécution scoped) ; **zéro
   infra détruite** (les « 2 destroy » du compteur = les 2 ANCIENNES
   révisions de task def dé-enregistrées, sémantique -/+ d'ECS ; les
   services se mettent à jour in-place, circuit breaker + rollback).
   Fait 2026-07-18 : plan = 6 add / 5 change / 2 destroy — lu au gate.
2. Mathieu : cert `relay.aithos.fr` hors-bande (lego/certbot DNS-01) +
   `aws secretsmanager put-secret-value` (cert + clé).
3. Build image relais M2 (musl statique hand-assemble comme P1) + store
   M2 (le /acme/txt exige la nouvelle image + bootstrap à tunnels) ;
   push ECR ; apply sur délégation.
4. Rejeu depuis une VRAIE machine : `relay-register.py` (TLS+ALPN),
   `relay-reach.py reach` avec `aithos-pod-stub POD_ACME=1` (staging LE
   d'abord) derrière NAT → joignabilité HTTPS + relais aveugle + cert
   auto-provisionné, clé côté pod.

**Dérives restantes (aucune nouvelle)** : nonce 16-car (p1, historique) ;
liste des vecteurs en tête d'annexe B à rafraîchir (p5/p6) — cosmétique,
par gate.

---

## État express — 2026-07-17 — P6/M2 : passthrough SNI + joignabilité HTTPS (vert-local, gate déploiement)

**Session (reprise piste P, cadrage M2 confirmé avec Mathieu).** Rituel
BDD tenu : features + vecteurs AVANT le code, rejeu vert, **STOP au gate
déploiement** (plan + creds Mathieu). Aucun déploiement, aucun `apply`,
rien sur `main` — tout sur `feat/obligations` (code, crate non commité, in
situ) et `feat/p6-p7-tunnel` (provider). La prod P1 (`store.aithos.fr`)
reste intacte. Choix M2 tranchés avec Mathieu : périmètre complet (GoAway,
anti-flap, keepalive), cert relais par **ACME Terraform → Secrets
Manager**, cert pod démo = **stub épinglé + LE manuel** (l'API B.5
/acme/txt reste une tranche ultérieure).

**Livré, prouvé vert en local, rejeux octet-exact** (branche code +
`provider@feat/p6-p7-tunnel`) :

- **Vecteur `p5-tunnel-sni.json` (annexe B.1/B.4)** — extraction SNI/ALPN
  sans terminaison. Générateur Python indépendant (ClientHello TLS 1.3
  honnêtes : navigateur, porte tunnel, casse mixte, fragmenté 2 records,
  sans SNI, non-TLS, tronqué, > 16 KiB) + vérificateur Python from-scratch.
  **p1..p4 restent BYTE-IDENTIQUES** (gelés respectés). Rejoué par
  `verify-p.py` ET **byte-exact contre le vrai code** (`tests/sni_replay.rs`,
  8 cas).
- **`sni.rs`** — peek pur (`peek_client_hello`), bornes SPEC (16 KiB / 10 s,
  jamais des knobs runtime), fail-closed ; décisions
  peeked|no_sni|not_tls|incomplete|too_large 1-pour-1 avec p5.
- **`passthrough.rs`** — porte publique unique `RelayDoor::serve` : peek →
  (porte tunnel = seul TLS terminé, ALPN aithos-tunnel/1) | (hostname à
  tunnel actif = un stream yamux, octets pipés dès le ClientHello, le pod
  termine SON TLS) | (tout le reste = fermeture silencieuse). Registre de
  sessions vivantes, **GoAway au remplacement** (1 hostname = 1 tunnel),
  **anti-flap ≥ 6/min → rate_limited**, cleanup de liveness (pod parti →
  déréférencé), logs `event=flow` expurgés (jamais un octet applicatif,
  jamais le SNI non vérifié).
- **`tls.rs`** — config rustls/ring de la porte tunnel (ALPN épinglé), cert
  chargé depuis PEM (env `*_PEM` façon Secrets Manager, ou chemin).
- **`bin/relay.rs`** — réécrit M2 (porte unique, peek, terminaison
  sélective, passthrough) ; l'ancien binaire M1 est remplacé.
- **`bin/pod_stub.rs`** (dev/e2e, feature `pod-stub`) — la moitié signée du
  wire : compose sur le relais, se déclare serveur yamux, termine SON TLS
  public, répond `/healthz`. C'est le « MCP joignable » de la démo.
- **BDD `relay-passthrough.feature` (18 scénarios) écrite AVANT le code** —
  porte TLS/ALPN, passthrough byte-exact (96 KiB, > 1 fenêtre yamux),
  half-close, SNI insensible à la casse, fermetures silencieuses (SNI
  inconnu / sans SNI / non-TLS / > 16 KiB / hello expiré), GoAway,
  anti-flap, liveness, logs aveugles. Harnais `cucumber_relay.rs` sur
  **vraies sockets + vrai TLS + vrai yamux**. Le scénario « pod FIGÉ
  détecté par PING yamux » est marqué **@wip/@draft2** (voir dérive
  ci-dessous), non compté vert.
- **Rejeu bout-en-bout des VRAIS binaires en local** (relay + pod-stub +
  sondes) : `GET https://demo.mcp.aithos.fr/healthz` **servi par le pod de
  bout en bout** (cert vu = celui du pod, jamais celui du relais) ; ghost
  SNI et HTTP en clair → fermeture sans un octet ; enregistrement ok /
  mapping_mismatch / clock_skew / signature_invalid sur la porte TLS+ALPN ;
  **scan des logs = 0 octet applicatif, 0 SNI ghost** (relais aveugle
  prouvé sur le binaire réel).
- **e2e enrichi** (`provider@feat/p6-p7-tunnel`) : `relay-p6.feature` gagne
  la porte TLS+ALPN, les fermetures B.4 et la joignabilité HTTPS ; sonde
  `relay-register.py` passe en TLS+ALPN (repli `RELAY_TLS=0`) ; nouvelle
  sonde `relay-reach.py` (reach|ghost|nosni|plain) ; steps behave ajoutés
  (dry-run vert). À rejouer contre l'infra au gate.
- **Terraform M2 écrit** (`fmt` OK ; câblage AWS `validate` OK ; le module
  ACME est HCL-valide — `validate` complet = au gate, le provider
  `vancluever/acme` n'est pas récupérable depuis ce sandbox) : module
  `relay-cert` (ACME DNS-01 Route53 → Secrets Manager), module `relay`
  étendu (env `AITHOS_RELAY_TUNNEL_NAME`, secrets `*_PEM`, grant exécution
  scoped aux 2 ARNs), `envs/prod` composé (cert conditionné à un email
  ACME). **Le NLB reste TCP passthrough** — M2 ne le change pas.
- **Compteurs locaux** : 29 tests unitaires + 34 BDD store + 12 BDD tunnel
  + **18 BDD passthrough** + rejeux p1/p3/p4/**p5** byte-exact + 4 handshake
  + witness/vectors verts ; **clippy 0, fmt OK**.

**Gate déploiement M2 (pour Mathieu, quand tu veux) — non fait ici :**
1. `terraform plan` (artefact) avec un `acme_email` réel + tes creds — lire
   intégralement ; le plan crée : cert LE `relay.aithos.fr` (ACME DNS-01),
   2 secrets Secrets Manager (cert + clé), grant exécution scoped, env
   `AITHOS_RELAY_TUNNEL_NAME`/secrets sur la task def relay.
2. Build + push de l'image relay M2 (le Dockerfile est inchangé : `FROM
   scratch`, CA embarquée, bootstrap public ; le cert vient de Secrets
   Manager, jamais de l'image).
3. `apply` sur ta délégation explicite ; puis rejeu **depuis une VRAIE
   machine** (proxy TLS :443) : `relay-register.py ok|mismatch|skew|badsig`
   (TLS+ALPN), `relay-reach.py reach` avec un `aithos-pod-stub` compilé
   derrière NAT → **joignabilité HTTPS prouvée, relais aveugle**.
4. Preuve d'anti-flap et de suspension (< 60 s) : la suspension propagée
   arrive avec **P7 live** (control plane DynamoDB) — cf. mission suivante.

**Dérives à arbitrer au gate (jamais corrigées unilatéralement) :**
- **Keepalive B.3 (PING yamux)** : le crate `yamux 0.13` épinglé n'expose
  pas le PING ; la liveness implémentée détecte un pod *déconnecté*
  (EOF/erreur → déréférencement immédiat), pas un pod *figé* (socket TCP
  vivant, appli muette). Scénario figé laissé **@wip/@draft2** (annexe B.6
  réserve déjà « un canal de contrôle riche : draft.2 »). À trancher :
  épingler une version yamux avec keepalive, backstop TCP-keepalive, ou
  redline B.3.
- **Clé TLS du relais dans le state Terraform** : le module ACME stocke la
  clé (`acme_certificate`/`tls_private_key`) dans le state (bucket chiffré
  + versionné) — caveat **assumé** à l'arbitrage. Alternative si refusé :
  cert obtenu hors-bande, importé dans Secrets Manager (le module relay lit
  les mêmes ARNs).
- **API B.5 `/acme/txt`** non implémentée en M2 : le pod démo prend son cert
  public par LE manuel (toi, en local). L'auto-provisioning par le pod via
  le store est une tranche ultérieure.
- **Nonce du relais partagé test** : la clé passerelle p3 (publique,
  committée) reste le mapping du bootstrap embarqué — comme M1, sans risque
  (mémoire), à isoler avec P7 (control plane réel).

---


## ✅ GATE P1 VALIDÉ — Mathieu, 2026-07-17

Store déployé sur `https://store.aithos.fr`, preuves rejouées **vertes
depuis la machine de Mathieu** (`deployed replay GREEN … P1 gate
evidence`, 2 tests OK ; cas à chaîne fail-closed 403 comme attendu) ; e2e
wire vertes ; plan Terraform lu (39 add / 0 change / 0 destroy) et
appliqué sur délégation explicite, contrôle post-apply à 0 écart ; état
express tenu. **P1 clos.** Prochain : P2 (store réel, `verify_chain`,
ensemble) ou déploiement de P5/P6/P7 après build de leurs binaires.

## État express — 2026-07-17T08:26Z — SESSION AUTONOME : P5 + cœur P6/P7 (vert-local, gate déploiement)

**Session autonome (Mathieu absent, feu vert explicite « continue de bout
en bout en suivant le rituel BDD, idéalement jusqu'à la finalisation »).**
Garde-fous tenus : aucun déploiement (creds absents/purgés ; apply hors
présence = interdit maintenu), rien sur `main` — tout sur des branches de
revue. La prod P1 (`store.aithos.fr`) reste intacte.

Livré, **prouvé vert en local, rejeux octet-exact** (branche code +
`provider@feat/p6-p7-tunnel`, commit `06329ef`) :

- **P6/P7 cœur — enregistrement tunnel (annexe B.2)** : `tunnel.rs`
  (ordre normatif forme→skew→nonce→signature→mapping, fail-closed, nonce
  brûlé avant la signature) + `control.rs` étendu (mapping
  `gateway_pub ↔ tenant ↔ hostname ↔ suspended` — slice P7). BDD
  `tunnel-register.feature` (12 scénarios) écrite AVANT le code ;
  **rejeu `p3` octet-exact** contre le vrai code (6 cas :
  register_ok, mapping_mismatch, signature_invalid, clock_skew,
  nonce_replayed, suspended). Pas d'oracle d'énumération (gateway inconnu
  et hostname faux = même `mapping_mismatch`).
- **P5 — témoin (annexe C)** : `witness.rs` (checkpoint C.1, feed line,
  racine quotidienne à domaines dédiés `mk-leaf`/`mk-node` + mroot
  left-heavy, règle d'équivocation C.4). **Rejeu `p4` octet-exact** :
  checkpoints, feed, racine (`eeb44d4d…`), équivocation. Signeur KMS
  Ed25519 = seam de déploiement (`WitnessSigner`).
- **Terraform lots suivants** (fmt+validate verts, autonomes — `envs/prod`
  intact) : `modules/relay` (NLB TCP :443 passthrough + Fargate + ECR +
  EIPs), `modules/witness` (KMS `ECC_NIST_EDWARDS25519` sign-only + feed
  S3 + policy emitter), `modules/control-plane-min` (table DynamoDB
  tenants + policies admin/reader).
- **Revue adversariale indépendante** : verdict **CONFORME** (ordre B.2
  exact, aucun fail-open, aucun oracle, byte-exact p3/p4, zéro secret
  serveur). Deux durcissements appliqués dans la foulée : l'équivocation
  exige désormais l'appartenance au **registre de clés publié** (une paire
  signée par des clés non publiées n'est plus une « preuve »), et le
  chemin de vérif témoin épingle `alg`/`version`.
- **Compteurs locaux** : 22 tests unitaires + 34 scénarios BDD store + 12
  BDD tunnel + rejeux p1/p3/p4 byte-exact, clippy 0, fmt OK.

**Gate déploiement (pour Mathieu, quand tu veux) — non fait ici** :
- P6 : bin `relay` (routeur SNI + yamux passthrough) + câblage
  `envs/prod` (module relay → `dns.relay_alias` + wildcard
  `*.mcp.aithos.fr`) + apply + preuve « gateway derrière NAT jointe,
  relay aveugle » ;
- P5 : bin `witness` (émetteur checkpoints + heartbeat, signe via KMS) +
  câblage module witness + apply + feed `witness.aithos.fr` ;
- P7 : bin admin (create/bind/suspend → DynamoDB) + bascule des services
  du bootstrap embarqué vers la table control-plane + apply + preuve
  suspension < 60 s.

**P2 (store réel) — délibérément NON entamé en autonomie.** Son cœur est
`verify_chain` (§04.5, l'autorisation des chaînes de mandats) : c'est le
jugement le plus sensible du système (un faux positif = une lecture non
autorisée). Je ne finalise pas une logique d'autorisation sans ton gate.
Les cas à chaîne de `p1` restent donc **fail-closed** (`chain_invalid`),
comme en P1 — jamais acceptés. P2 est le prochain lot à faire ensemble.

## État express — 2026-07-17T07:20Z — P1 DÉPLOYÉ EN PROD, PREUVES VERTES

**Le store tourne sur `https://store.aithos.fr`.** Déploiement exécuté par
Claude sur instruction explicite de Mathieu (2026-07-17, creds de session
SSO temporaires fournis via `.aws-env` — délégation de l'apply actée, plan
lu intégralement avant chaque apply, cf. ci-dessous). Validation finale
« ensemble » = dernier pas du gate.

- **Apply n°1** : plan lu — 39 à créer, 0 à modifier, **0 à détruire**,
  rien de la landing. VPC 2 AZ, zone `mcp.aithos.fr` déléguée + policy
  ACME, ACM `store.aithos.fr`, ALB TLS, ECR, DynamoDB nonces (TTL), logs
  30 j, cluster/task/service Fargate, rôles plan/deploy (trust GitHub en
  `placeholder/*` — verrouillés par construction, à re-planer avec les
  vrais noms de dépôts à la mise sur GitHub).
- **Deux vrais bugs trouvés PAR le rituel de gate** (le rejeu signé
  refusait 503 `unavailable` — fail-closed exact : nonce store
  injoignable, jamais une acceptation) :
  ① `AWS_REGION` absent de la task def — le SDK Rust ne résout pas la
  région sur Fargate (pas de chaîne task-metadata) → DynamoDB inatteignable.
  Corrigé dans `modules/store-api` (apply n°2 : 1 add/1 change/1 destroy =
  révision de task def, lu avant apply). Commit provider `1307906`.
  ② Image `FROM scratch` sans racines TLS (« no native root CA certificates
  found ») → bundle CA ajouté au chemin standard dans
  `docker/store-api.Dockerfile` (stage alpine `ca-certificates`) et dans
  l'image poussée.
- **Preuves du gate, contre la prod** : rejeu signé
  `deployed replay GREEN against https://store.aithos.fr` (hello PUT
  accepté ; skew 301 s ; nonce rejoué ; signature corrompue — codes A.7
  exacts) ; e2e behave 10/10 ; byte-exact local 4/4 + fail-closed sur les
  cas à chaîne ; `terraform plan -detailed-exitcode` final = **0 écart**
  infra↔code.
- **Note de build (transitoire)** : l'image poussée a été assemblée dans la
  session (binaire musl statique + CA bundle + fixture, `FROM scratch`) —
  le CDN alpine n'est pas joignable depuis le sandbox. Le Dockerfile
  officiel corrigé reconstruira à l'identique avec la toolchain épinglée
  au premier run CI (GitHub).
- **Hygiène credentials** : clés de session temporaires (SSO, expirent
  seules) ; purgées du sandbox après usage ; la copie `.aws-env.session`
  déplacée dans `provider/_to_delete/` — **supprimer ce dossier et vider
  `.aws-env`**. Aucun credential dans un dépôt ni un log.
- **Reste pour clore le gate P1** : test ensemble (Mathieu) ; décisions
  des points d'arbitrage 1–6 ; commit du dépôt code (crate + docs, geste
  de gate) ; plus tard mise sur GitHub + re-plan des trust policies.

## État express — 2026-07-17 (amendement : PROD DIRECTE, arbitrage Mathieu)

**Arbitrage Mathieu (2026-07-17)** : plateforme unique déployée directement
sur les **noms apex gravés par A6** — `store.aithos.fr`, `*.mcp.aithos.fr`
(un seul compte AWS, pas d'utilisateurs, démo à venir). L'environnement
`dev` n'existe plus pour l'instant ; il reviendra en sous-domaine au
premier besoin réel, avec les mêmes modules. Exécuté (commit `2cde0f7` du
dépôt provider) :

- `envs/dev` → `envs/prod` (clé d'état `provider/envs/prod/terraform.tfstate`,
  préfixe de ressources `aithos-provider-prod-*`, tags `Environment=prod`) ;
- tag d'image `:prod` (workflow `provider-image.yml` + task definition) ;
- e2e, skill `rituel-tests`, runbook et CI alignés sur
  `https://store.aithos.fr` ;
- fixture de rejeu renommée `bootstrap/replay.json` (ex-`dev.json`).
- **Nouveau point d'arbitrage n°6 (P2 au plus tard)** : le tenant de rejeu
  `acme` embarqué dans l'image de prod repose sur les clés des vecteurs
  committés — donc PUBLIQUES. Requis pour prouver le gate P1, sans risque
  tant que le store est en mémoire (rien ne persiste), mais dès que P2
  apporte S3 il faut le retirer de la prod ou l'isoler (tenant de rejeu
  dédié + purge, ou retour d'un env dev). À trancher au gate P2.

Le guide de gate ci-dessous se lit avec ces valeurs : URL
`https://store.aithos.fr`, cluster `aithos-provider-prod`, service
`aithos-provider-prod-store-api`, ECR `aithos-provider-prod-store-api:prod`.

## État express — 2026-07-16T14:20Z (amendement : dépôt provider dédié)

**Restructuration ACTÉE PAR MATHIEU (2026-07-16, post-livraison P1)** — le
Terraform provider vit désormais dans un **dépôt git dédié `provider/`**
(initialisé, branche `main`, commit `5d9e9cd`), extrait du dépôt landing :

- `provider/infra/terraform/{modules,envs}` — DÉPLACÉ depuis
  `infra/terraform/` du dépôt landing (qui retrouve son périmètre
  d'origine : landings + `bootstrap/` du bucket d'état, partagé, intouché) ;
- `provider/e2e/` — **suite e2e Gherkin (behave), sans clé**, un fichier de
  feature par lot : `store-p1.feature` (10 scénarios, verts contre le
  binaire réel) ; les lots suivants ajoutent les leurs (P2/P5/P6/P7) et un
  gate exige toute la suite verte contre le dev déployé. La moitié SIGNÉE
  du wire reste dans le dépôt code (`vectors_replay.rs`) — une seule
  implémentation de crypto ;
- `provider/.claude/skills/rituel-tests/SKILL.md` — le rituel gravé et
  versionné : BDD avant code, échelle de preuve (simulation → binaire réel
  → endpoint déployé), e2e par lot, gates/interdits ;
- `provider/.github/workflows/` — `provider-terraform.yml` (plan-only) et
  `provider-e2e.yml` (behave, sans credential) ; le README du runbook et
  la variable `github_repository_infra` pointent maintenant ce dépôt.
  Mise sur GitHub + pipeline de prod : plus tard, par Mathieu.
- Résidu : un dossier `provider/_to_delete/` (fichiers temporaires git que
  le pont distant ne peut pas supprimer) — à supprimer à la main.

L'arborescence cible ci-dessous est amendée d'autant : côté Terraform,
lire `provider/infra/terraform` partout où le gate P0 écrivait
`../../infra/terraform`.

## État express — 2026-07-16T13:49Z

**P0 : LIVRÉ — GATE VALIDÉ (Mathieu, 2026-07-16). P1 : LIVRÉ — EN ATTENTE
DU GATE (revue Mathieu + apply Terraform manuel + rejeu déployé). STOP.**

- **Rituel tenu : BDD avant le code.** `rust/crates/aithos-provider/tests/
  features/store-hello.feature` (34 scénarios : l'ordre A.2 cas par cas,
  version wire, limites A.8, discipline de logs) écrite avant la première
  ligne de code, verte contre la vraie surface axum (177 steps).
- **Crate `aithos-provider` créé** (workspace inchangé par ailleurs, core
  pur intact — seul `rust/Cargo.toml` gagne le membre + 6 deps workspace) :
  `envelope.rs` (ordre A.2 #2–#10, now injecté, fail-closed), `pathmap.rs`
  (grammaire A.1 #0 + exceptions anonymes A2), `nonces.rs` (réservation
  (key, nonce) insert-if-absent : DynamoDB TTL en prod, mémoire en dev/test,
  fenêtre ≥ 600 s bornes incluses), `control.rs` (read-model tenants par
  bootstrap vérifié — P7 le remplace), `objects.rs` (mémoire — S3 en P2),
  `redact.rs` (registre A.8 fermé, une seule ligne de log par requête,
  test-sentinelle anti-fuite), `service.rs` + `bin/store_api.rs` (axum
  toujours chaud, `/healthz`, arrêt gracieux). Binaire statique musl 14 Mo
  vérifié (image `FROM scratch`, `docker/store-api.Dockerfile`).
- **Vecteurs p1 rejoués contre le binaire réel** (`tests/vectors_replay.rs`,
  process enfant + vraie socket) : les 4 cas P1 **verts octet-exact**
  (`accept_put_owner_root` — did.json du vecteur préchargé —,
  `reject_clock_skew_301s`, `reject_nonce_replayed` — nonce brûlé AVANT le
  refus #9 —, `reject_signature_invalid`) ; les 5 cas à chaîne assertés
  **fail-closed 4xx, jamais acceptés** (P2 les rend exacts). Mode gate :
  `AITHOS_REPLAY_URL=https://store.dev.aithos.fr cargo test -p
  aithos-provider --test vectors_replay` re-signe les mêmes sémantiques
  (deltas committés, horloge réelle) contre le endpoint déployé.
- **Terraform prolongé, existant intouché** : `modules/dns` (zone déléguée
  `mcp.dev.aithos.fr` + `store.` ; `relay./witness./app.` + wildcard =
  entrées optionnelles pour P6/P5 ; policy ACME B.5 créée — TXT
  `_acme-challenge.*` uniquement, attachée en P6), `modules/store-api`
  (ALB TLS + Fargate + ECR + DynamoDB nonces TTL + logs 30 j A.8 ; task
  role = UN droit : `dynamodb:PutItem` sur la table nonces),
  `envs/dev` (backend bootstrap existant, clé `provider/envs/dev/…`, VPC
  2 AZ sans NAT, runbook `README.md`). `fmt` + `validate` verts ; **aucun
  plan/apply lancé ici, `.aws-env` jamais lu**.
- **CI zéro credential longue durée, deux rôles OIDC épinglés** :
  `provider-terraform.yml` (dépôt infra : fmt + validate + **plan en
  artefact sur PR ; AUCUN job d'apply n'existe**) sur le rôle `plan`
  (lecture seule + état scoped) ; `provider-image.yml` (dépôt code : tests
  + build + push ECR `:dev`/`:sha` + redeploy) sur le rôle `deploy`
  (push ECR + UpdateService, rien d'autre).
- **Revue adversariale passée** (ordre A.2, registre A.7 exact 20/20,
  aucune voie fail-open vers Owner/2xx, aucun secret côté serveur, logs
  étanches) — verdict CONFORME ; une borne de fenêtre nonce (backend
  mémoire) corrigée dans la foulée.
- **Dérives/écarts à arbitrer au gate (jamais corrigés unilatéralement —
  la correction va dans INFRA-PROVIDER.md)** : voir « Points d'arbitrage
  du gate P1 » ci-dessous.
- **Reste pour clore le gate P1 (Mathieu)** : ⓪ installer les deux
  workflows (protégés en écriture distante, livrés en staging) :
  `mv _transfer/provider-image.yml .github/workflows/` (dépôt code) et
  `mv _transfer/provider-terraform.yml .github/workflows/` (dépôt infra) ;
  ① init/plan/**lecture**/apply `envs/dev` (runbook
  `infra/terraform/envs/dev/README.md`) ; ② config GitHub (environnements
  `provider-plan`/`provider-deploy`, secrets ARN, variables) ; ③ premier
  run `provider-image.yml` ; ④ rejeu déployé (commande ci-dessus) ;
  ⑤ revue + décision des points d'arbitrage.
- **Prochain pas après gate** : P6 (relay) + P7 (control plane) en priorité
  — chemin critique démo — pendant que P2–P5 suivent.

### Points d'arbitrage du gate P1 (dérives constatées, à trancher dans INFRA-PROVIDER.md)

1. **Nonce 16–64 car. (A.2) vs vecteurs gelés** : `p0-n-rej-sig-05` et
   `p0-n-rej-key-09` font 15 caractères. Appliquer la borne basse rendrait
   `reject_signature_invalid` non rejouable (il répondrait
   `envelope_invalid`). Choix P1 : borne haute appliquée (≤ 64), borne
   basse NON appliquée, documenté dans `envelope.rs`. À trancher : redline
   A.2 (borne basse = exigence client, SHOULD) ou régénération de vecteurs
   sous un nouvel id.
2. **`did_not_bound` révélé à l'étape #7** (après forme/skew/nonce, avant
   signature #8) : lecture fidèle de la note anti-énumération A.7
   (« seulement sous enveloppe valide ») mais la table A.2 le place en #1
   et « valide » peut se lire « signature comprise ». Oracle marginal :
   le bit d'enrôlement est déjà public par design (GET anonyme `did.json`
   → 200/404). À préciser dans l'annexe.
3. **Codes transitoires hors registre A.7** : `501 not_implemented`
   (routes A.3 valides non servies en P1 : heads/batch/gamma/sync/list,
   et PUT des classes vérifiées A.4 : manifest/did.json/certs/segment) et
   `503 unavailable` (table nonces injoignable → refus fail-closed, jamais
   une acceptation). Le premier disparaît avec P2 ; le second est
   opérationnel, pas wire — à documenter ou intégrer au registre.
4. **Ordre des contrôles pré-grammaire** : négociation de version (426) et
   borne de corps 32 MiB (413) évalués avant #0 — le corps doit être borné
   avant d'être haché ; l'annexe ne fixe pas cet ordre. À confirmer.
5. **Blocage résiduel P1 (assumé, levé en P2)** : objets en mémoire par
   tâche (`desired_count = 1`), tenants par bootstrap embarqué (matériel
   public, ancré sur `vectors/p1` par test anti-dérive). Les nonces sont
   déjà DynamoDB.

---

## Historique — État express 2026-07-16T12:19Z (gate P0)

**P0 : LIVRÉ — GATE VALIDÉ (Mathieu, 2026-07-16, lancement P1 acté).
P1 : À LANCER dans un contexte dédié (prompt remis à Mathieu).**

- **Annexes normatives gravées** dans `INFRA-PROVIDER.md` : A = wire
  `aithos-store 1.0.0-draft.1` (C1 — enveloppe `X-Aithos-Auth` exacte, ordre
  de vérification 0–10, path-map, vérification d'artefacts, CAS `If-Head` sur
  les deux têtes, registre d'erreurs, limites + discipline de logs) ; B =
  tunnel `aithos-tunnel` (C2 — enregistrement signé, yamux, SNI, bornage,
  API ACME déléguée) ; C = checkpoint `aithos-witness` (C3 — format, feed,
  racine quotidienne à domaines dédiés, règle d'équivocation). Précisions
  additives assumées vs croquis §3.2 : champ `host` dans l'enveloppe
  (anti-rejeu inter-plans, G7 réutilise l'enveloppe), `POST /gamma` (append
  d'une entrée, chemin chaud mode B) à côté du PUT de segment (réplique mode
  A), route `/heads`, `If-Head: none` pour la genèse. **Arbitrage KMS
  dissous au gate** (vérifié en ligne le 2026-07-16, info post-cutoff) : AWS
  KMS signe EdDSA/Ed25519 nativement depuis nov. 2025 — C.1 corrigée en clé
  KMS native sign-only (`ECC_NIST_EDWARDS25519`, `ED25519_SHA_512`,
  `MessageType: RAW`, jamais le mode `_PH_`) ; wire et vecteurs inchangés.
- **Vecteurs P1–P4 committés** (`vectors/p1…p4-*.json` + générateur
  indépendant `vectors/gen-p.py`, ancré sur A1+G1) : 24 cas dont TOUS les
  rejets exigés — skew (301 s, + borne 300 s acceptée), nonce rejoué, chaîne
  révoquée (forward-only), CAS mismatch (manifest + gamma), plus fenêtre
  expirée, `not_covered`, signatures corrompues, key≠feuille,
  `cas_required`, `prev_hash_mismatch`, mapping tunnel, suspension,
  équivocation témoin (prouvée / heartbeat / heights ≠). Rejoués verts par
  `vectors/verify-p.py` (simulation indépendante de l'ordre des annexes —
  le rejeu contre le vrai service reste le gate P2).
- **Arborescence cible proposée** : § « Arborescence cible » ci-dessous.
- **Interdits honorés** : zéro secret, zéro ressource AWS touchée (aucun
  `plan`/`apply`), `.aws-env` jamais lu, aucun commit/merge git, core intact.
- **Prochain pas** : P1 dans un contexte dédié (modules Terraform `dns` +
  env `dev` sur le backend bootstrap, squelette axum, hello signé rejouant
  les cas d'enveloppe de `p1` — apply Terraform par Mathieu uniquement),
  puis P6/P7 en priorité (chemin critique démo) pendant que P2–P5 suivent.

> **Statut : PRÊT À LANCER — 2026-07-16.** Plan d'action exécutable de la piste
> provider (`store.aithos.fr`, `witness.aithos.fr`, relay `*.mcp.aithos.fr`,
> control plane minimal). Se lit avec [`INFRA-PROVIDER.md`](../INFRA-PROVIDER.md)
> (doctrine, wire v0, contrats C1–C3) et en parallèle de
> [`HANDOFF-GATEWAY-HUB.md`](HANDOFF-GATEWAY-HUB.md) (piste G). Les deux pistes ne
> se parlent **que** par les contrats — toute dérive se corrige d'abord dans
> INFRA-PROVIDER.md, jamais par un accord implicite entre lots.

## Contexte en 30 secondes

Le protocole est enforceable depuis les fichiers seuls ; le provider n'est jamais
une partie de confiance. On construit : un `Store` distant signé (wire v0, C1), un
témoin (C3), un relay passthrough SNI (C2) et le minimum de control plane pour
lier `gateway_pub ↔ tenant ↔ hostname`. Le chemin critique de la **démo BYO** ne
passe que par **P1 + P6 + P7** — le store (P2–P4) n'est pas requis pour la démo et
avance en parallèle.

## Interdits (opposables à chaque lot)

- Aucun secret client côté serveur : pas de clé privée, pas de token, pas de
  plaintext. Le serveur vérifie des signatures et déplace des octets.
- Jamais de lecture de payloads au relay : passthrough TCP/SNI strict (A3).
- Logs applicatifs expurgés (discipline `credentials.rs`) : jamais un chemin de
  section, un corps, une enveloppe complète ; rétention 30 j.
- Le `covers()` serveur est de l'anti-abus : tout refus est un `403` propre,
  jamais une décision d'autorité ; le serveur ne « corrige » jamais un artefact.
- Fail-closed partout : artefact non vérifiable → rejeté ; CAS mismatch → `409` +
  tête courante ; jamais d'écrasement silencieux.
- Pas de dépendance du core au réseau : `aithos-core` reste pur ; tout le réseau
  vit dans `aithos-bundle` (client) et dans le service (serveur).
- Terraform uniquement (pas de ressource console) ; déploiement par le rôle OIDC
  GitHub existant ; aucun credential AWS longue durée.

## Lots

### P0 — Spec wire gravée + vecteurs *(S, bloquant, commun aux deux pistes)*
Geler le wire v0 (INFRA-PROVIDER §3.2–3.4, C1), le format checkpoint (C3) et le
protocole tunnel (C2) en annexes normatives ; produire des **vecteurs de requêtes
signées** langage-neutres (JSON : enveloppe, clés de test, réponses attendues —
même esprit que `vectors/`), y compris cas de rejet (skew, nonce rejoué, chaîne
révoquée, CAS mismatch).
**Gate : revue Mathieu + vecteurs committés.**

### P1 — Socle Terraform + squelette service *(S)*
Modules `dns` (wildcard `*.mcp.aithos.fr`, `store.`, `witness.`, délégation
ACME), état/backend (pattern bootstrap existant), env `dev` ; squelette Rust axum
sur Fargate + ALB + healthcheck ; CI de déploiement OIDC (pattern landings).
**Gate : un endpoint `dev` répond à une requête signée « hello » vérifiée.**

### P2 — Store service v1 *(L)*
GET/LIST/batch + PUT d'artefacts signés : enveloppe (Ed25519 + anti-rejeu DDB),
`verify_chain` avec cache par `(mandate_id, tête_de_révocation)`, path-map
lecture/append/publish (§3.3), layout S3 `/t/<tenant>/<did>/…`, CAS `If-Head` sur
`manifest.json` + segment gamma (DynamoDB pour les têtes, S3 pour les corps).
Features Cucumber côté service (`store-auth.feature`, `store-cas.feature`,
`store-paths.feature`) + les vecteurs P0 rejoués contre le vrai service.
**Gate : features vertes + un owner pousse/relit un bundle complet via CLI.**

### P3 — Client `RemoteStore` *(M, côté repo `aithos-core`)*
Impl `Store` dans `aithos-bundle` (HTTP, enveloppe signée, `put_if`, retries +
backoff, cache local immuable) ; `store_adapter` accepte `kind: remote { url,
tenant }` (le refus fail-closed du s3 reste) ; config par contexte/journal ;
décorateur mode A (réplication asynchrone post-publish).
**Gate : DEMO-LEA rejouée à l'identique avec `journal.store = remote` (mode B) ;
un contexte en mode A répliqué et relu depuis le store.**

### P4 — Sync/pack + perf *(M)*
`POST /sync {have_edition}` → pack des chemins changés (descente de racines) ;
`get_many` ; CloudFront sur l'immuable public ; bench et gates de perf
(INFRA-PROVIDER §3.6).
**Gate : cibles tenues sur un Ethos de 1 000 sections (p50 cache < 5 ms, sync
froid < 2 s, append < 120 ms).**

### P5 — Témoin *(S)*
Contre-signature à chaque publish/réplique + heartbeat quotidien ; clé KMS
sign-only ; feed `witness.aithos.fr/<did>.jsonl` + racine quotidienne ;
vérificateur de checkpoint dans la CLI.
**Gate : une équivocation simulée (deux manifests concurrents même height) est
détectée par le vérificateur à partir des seuls feeds.**

### P6 — Relay passthrough *(M, indépendant de P2–P5)*
NLB :443 → routeur SNI (Rust, tokio) → registre de tunnels ; enregistrement
sortant signé par la clé de gateway, vérifié contre P7 ; multiplexage (yamux ou
h2), keepalive 30 s, reconnexion backoff ; bornage de tuyau (rate/conn par tenant,
coupure) ; support ACME DNS-01 délégué (API interne posant les TXT).
**Gate : une gateway dev derrière NAT est jointe via
`https://demo.mcp.aithos.fr/mcp` de bout en bout, relay aveugle (vérifié : aucun
octet applicatif loggable).**

### P7 — Control plane minimal *(S)*
Table tenants (`tenant ↔ did(s) ↔ gateway_pub ↔ hostname ↔ quotas`) ; CLI d'admin
interne (create/bind/suspend) ; aucun secret émis — l'enrôlement lie des clés
publiques.
**Gate : enrôler un tenant complet en une commande ; la suspension coupe tunnel et
store en < 60 s.**

## Ordonnancement

```
P0 ──► P1 ──► P2 ──► P3 ──► P4        (chaîne store)
        │       └───► P5              (témoin, après P2)
        ├────────► P6 ─┐
        └────────► P7 ─┴─► gate G1/G9 (démo BYO, avec la piste G)
```

Chemin critique démo : **P0 → P1 → P6/P7** (le store n'y est pas). P2–P5 avancent
en parallèle sans bloquer personne.

## Arborescence cible (proposée au gate P0, exécutée en P1+)

**Côté Rust — un crate `aithos-provider` dans le workspace** (côté serveur ;
le client `RemoteStore` de P3 vit dans `aithos-bundle` derrière une feature
`remote`, comme prévu par son en-tête « `s3` will live behind a feature » —
le core reste pur, le réseau vit dans `aithos-bundle` et le service) :

```
rust/crates/aithos-provider/
  Cargo.toml               # bins: aithos-store-api, aithos-relay, aithos-witness
  bootstrap/dev.json       # (P1) tenants dev = fixture p1, public, test anti-dérive
  src/
    lib.rs
    envelope.rs            # X-Aithos-Auth : parse + ordre A.2 (réutilise aithos-core)
    pathmap.rs             # grammaire A.1 + covers() A.3 (anti-abus)
    artifacts.rs           # vérifications A.4 par classe de chemin (P2)
    cas.rs                 # têtes DynamoDB + If-Head (A.5) (P2)
    objects.rs             # layout S3 /t/<tenant>/<did>/… (P1 : seam + mémoire)
    nonces.rs              # anti-rejeu (DynamoDB TTL + mémoire dev/test)
    control.rs             # read-model tenants (P1 : bootstrap ; P7 : réel)
    redact.rs              # discipline de logs A.8 (façon credentials.rs)
    service.rs             # (P1, additif) surface axum partagée bin/tests
    time.rs                # (P1, additif) RFC 3339 Zulu strict, epoch ms
    witness.rs             # checkpoint C3 : build/sign/feed/racine (P5)
    tunnel.rs              # registre C2 : enregistrement B.2, GoAway, bornage (P6)
    bin/store_api.rs       # axum toujours chaud (P1 : squelette + hello signé)
    bin/relay.rs           # routeur SNI + yamux (P6)
    bin/witness.rs         # émetteur checkpoints + heartbeat quotidien (P5)
  tests/
    cucumber.rs            # (P1) harnais BDD de store-hello.feature
    features/              # BDD avant le code (rituel) : store-hello.feature (P1),
                           # store-auth.feature, store-cas.feature,
                           # store-paths.feature (P2),
                           # tunnel-register.feature, witness-checkpoint.feature
    vectors_replay.rs      # rejoue vectors/p1…p4 contre le service réel
                           # (P1 : cas p1 octet-exact + mode endpoint déployé ;
                           #  gate P2 : le reste)
```

Un seul crate, trois binaires : les trois plans partagent enveloppe, redaction
et read-model control plane sans se partager d'état ; le chemin critique démo
(P6/P7) compile sans le store. `aithos-gateway/store_adapter.rs` gagne en P3 la
variante `remote { url, tenant }` (le refus fail-closed du `s3` inconnu reste).

**Côté Terraform — `../../infra/terraform`, prolongé en modules** (patterns
existants conservés : bootstrap d'état S3 versionné/chiffré, rôle OIDC GitHub
à environnement épinglé, jamais de `.aws-env`, plan avant tout apply, aucun
apply sans validation humaine) :

```
infra/terraform/
  bootstrap/               # existant (bucket d'état) — inchangé
  *.tf                     # existant (landing ai-first) — inchangé, migration hors périmètre
  modules/
    dns/                   # zones + wildcard *.mcp.aithos.fr, store., witness., app.,
                           # relay. ; délégation ACME (la zone pose les TXT via B.5)
    cdn-public/            # S3+CloudFront : app, feed témoin, zone publique du store
    store-api/             # ALB + Fargate (axum toujours chaud) + ECR + S3 données
                           # + DynamoDB (têtes A.5, nonces A.2#6)
    relay/                 # NLB :443 + service SNI/tunnels + EIPs
    witness/               # clé Ed25519 scellée KMS (C.1) + bucket feed + schedule heartbeat
    control-plane-min/     # table tenants (tenant↔did(s)↔gateway_pub↔hostname↔quotas)
                           # + rôle CLI admin interne (P7)
  envs/
    dev/                   # composition des modules, backend key envs/dev
    prod/                  # idem, créé au premier besoin réel
```

CI : nouveau rôle OIDC dédié provider (pattern `github-actions.tf` landings :
environnement GitHub épinglé, zéro credential longue durée) ; PR = `fmt` +
`validate` + `plan` en artefact ; `apply` **manuel uniquement** après lecture
du plan (environnement protégé). Région `eu-west-3` (+ `us-east-1` pour les
certs CloudFront, comme les landings) ; DR = A5 (versioning même région).

## Ce que cette piste ne fait pas

Pas de console self-service, pas de facturation automatique, pas d'AS OAuth (piste
G), pas de dashboard (app statique = piste G, lot G7 pour la surface de preuve ;
l'hébergement CloudFront est trivial et vit dans `cdn-public`). Pas de
cross-région (A5).

## Définition de « fini » (piste entière)

Un tenant enrôlé en une commande ; un bundle poussé, servi, synchronisé et
contre-signé ; une gateway derrière NAT jointe sur son hostname avec un relay
prouvé aveugle ; les cibles de perf tenues ; les vecteurs P0 rejouables par un
tiers sans lire le code du service.

# PROMPT DE REPRISE — Piste P / provider Aithos — clôture M2 vers prod stable

> **ARCHIVE — ne pas exécuter.** M2 et les lots Provider suivants sont clos.

> À coller dans un nouveau contexte. Reprend la piste P (provider Aithos sur
> AWS) au point exact du 2026-07-18 : le cœur M2 est écrit et vert en local,
> trois arbitrages sont tranchés, et il reste à finir M2 « orienté prod
> stable » puis à passer le gate déploiement. Se lit avec
> `code/aithos-core/docs/INFRA-PROVIDER.md` (annexes A/B/C) et
> `HANDOFF-PROVIDER-AWS.md`.

---

Tu prends la suite de la piste P : le provider Aithos sur AWS. Tu suis le rituel BDD (features Gherkin AVANT le code, puis rejeu des vecteurs contre le binaire réel) et tu STOP à chaque gate pour revue humaine (Mathieu).

DOCTRINE (non négociable) : le provider déplace des octets et vérifie des signatures ; il ne détient jamais de secret CLIENT, ne décide jamais. Fail-closed partout (forme, skew ±300 s, nonce, signature, mapping, version → refus + fermeture). covers() serveur = anti-abus, jamais l'autorité. Logs expurgés façon credentials.rs. aithos-core reste pur (crate aithos-provider séparé). Terraform seulement, aucun apply sans creds humaines explicites (plan d'abord, en artefact). Pas de merge main sans gate humain. Le relais NE termine JAMAIS le TLS public (A3) : il lit le SNI sans terminer, le pod termine le TLS du navigateur en gardant sa clé.

## Décisions gravées avec Mathieu (2026-07-18) — à respecter

1. **Orientation prod stable directe** (pas d'env dev jetable ; le dev = local, déjà prouvé sur localhost). Le relais reste **Fargate/NLB** — c'est l'unique composant always-on impératif (processus vivant tenant les tunnels TCP persistants sans terminer le TLS ; Lambda ne peut pas). Le store/témoin/public peuvent aller serverless plus tard.
2. **Compute du store : Lambda vs Fargate = à trancher au gate P2** (quand S3 arrive et qu'on touche la couche stockage). Rust a des cold starts modestes (~15-30 ms) ; Lambda gagne à volume bas/pics (dev/early), Fargate à haut volume soutenu. Le relais reste Fargate quoi qu'il arrive. Graver cette note dans INFRA-PROVIDER §7.
3. **HA multi-tâches = tranche ultérieure, NE PAS bumper `desired_count` du relais** : le registre hostname→session yamux est en mémoire par tâche → 2 tâches casseraient le routage (navigateur sur tâche B, tunnel sur tâche A). La vraie HA relais exige un **registre partagé + saut relais-à-relais** (patron ngrok/Cloudflare Tunnel) — à concevoir. HA store exige P2 (S3, sortir de la mémoire par tâche). Donc M2 = relais **prod-grade en fonctionnalités**, pas encore multi-tâches.

### Les 3 dérives tranchées (à appliquer dans M2)

- **Keepalive B.3** → **redline draft.2 + backstop TCP keepalive maintenant.** Ajouter `SO_KEEPALIVE` (idle court) au socket du tunnel pod côté relais ET côté `aithos-pod-stub`. Rédiger la redline dans INFRA-PROVIDER annexe B : « M2 = détection de déconnexion (EOF) + TCP keepalive ; PING actif applicatif (pod FIGÉ, TCP vivant mais muet) = draft.2 via le canal de contrôle riche que B.6 réserve. » Le scénario « pod figé » reste `@wip/@draft2` (déjà en place dans `relay-passthrough.feature`). NE PAS épingler un vieux yamux (couplerait la gateway G1 à un mux legacy).
- **Clé TLS du relais** → **OUT-OF-BAND, clé jamais dans le state.** Retravailler `modules/relay-cert` : SUPPRIMER les ressources `acme_registration`/`acme_certificate`/`tls_private_key`/`tls_cert_request` ET les `aws_secretsmanager_secret_version`. Ne garder que les 2 `aws_secretsmanager_secret` VIDES (cert + clé) + leurs ARN en output. Mathieu obtient le cert `relay.aithos.fr` hors-bande (lego/certbot DNS-01) et fait `aws secretsmanager put-secret-value` — Terraform ne gère jamais la valeur. Retirer le provider `vancluever/acme` et `hashicorp/tls` de `envs/prod` (required_providers + bloc provider acme + vars `acme_email`/`acme_server_url`). **Conséquence bonus : sans le provider acme (bloqué en 403 depuis le sandbox cloud), `terraform init/plan/apply` redevient lançable depuis le sandbox** (aws seul).
- **API B.5 `/acme/txt`** → **implémenter dans M2** (feature-first). Le store gagne `PUT`/`DELETE /acme/txt` : enveloppe A.2 avec `key = gateway_pub` et `mandate: []`, **autorité = mapping control plane** du signataire (exception gravée B.5, pas une chaîne de mandats). Pose/retire `TXT _acme-challenge.<hostname>` (TTL 60 s), le hostname DOIT appartenir au tenant du gateway_pub. Erreurs registre A.7 + `mapping_mismatch`. Anti-abus ≤ 10 PUT/h/hostname. Attacher la policy Route53 ACME-TXT (déjà écrite dans `modules/dns`, `acme_txt_policy_arn`) au **task role du store**. Résultat : le pod (client) auto-provisionne SON cert public `<org>.mcp.aithos.fr`, la clé restant côté client (A3) — ce qui **supprime** le stopgap « cert pod manuel ». Le `aithos-pod-stub` gagne un mode qui obtient son cert via `/acme/txt` pour la démo.

## DÉJÀ FAIT et VERT EN LOCAL (ne pas refaire)

P0/P1 livrés + validés, `store.aithos.fr` en prod. P6/M1 relais **déployé** (`relay.aithos.fr`, NLB TCP :443, poignée B.2 byte-exact, 2 tâches services live : store td:2, relay td:1). P5 témoin + P7 control-plane-min : modules écrits, **non composés** dans envs/prod (rien de live).

**P6/M2 — cœur écrit et prouvé vert en local (branche code `feat/obligations` = crate `rust/crates/aithos-provider` NON commité, in situ ; `provider@feat/p6-p7-tunnel`) :**

- **`sni.rs`** — peek pur du ClientHello (SNI+ALPN) sans terminer, bornes SPEC 16 KiB / 10 s (constantes, pas des knobs), décisions peeked|no_sni|not_tls|incomplete|too_large. **Vecteur `p5-tunnel-sni.json`** (générateur + vérificateur Python indépendants, `gen-p.py`/`verify-p.py`) rejoué **byte-exact** contre le vrai code (`tests/sni_replay.rs`, 8 cas). **p1..p4 restent byte-identiques (gelés respectés).**
- **`passthrough.rs`** — porte publique unique `RelayDoor::serve` : peek → porte tunnel (seul TLS terminé, ALPN aithos-tunnel/1) | hostname à tunnel actif (stream yamux, octets pipés dès le ClientHello, le pod termine son TLS) | sinon fermeture silencieuse. Registre de sessions vivantes, GoAway au remplacement, anti-flap ≥ 6/min → rate_limited, liveness (pod déconnecté → déréférencé), logs `event=flow` expurgés.
- **`tls.rs`** — config rustls/ring de la porte tunnel (ALPN épinglé), PEM depuis env `*_PEM` (Secrets Manager) ou chemin.
- **`bin/relay.rs`** — réécrit M2 (porte unique, peek, terminaison sélective, passthrough). **`bin/pod_stub.rs`** (dev/e2e, feature `pod-stub`) — la moitié signée du wire : compose sur le relais, serveur yamux, termine son TLS public, répond `/healthz`.
- **BDD `tests/features/relay/relay-passthrough.feature`** (18 scénarios écrits AVANT le code) — harnais `tests/cucumber_relay.rs` sur vraies sockets + vrai TLS + vrai yamux. Le scénario pod-figé est `@wip/@draft2`.
- **Rejeu bout-en-bout des VRAIS binaires en local** : `GET https://demo.mcp.aithos.fr/healthz` servi par le pod de bout en bout (cert = celui du pod, jamais du relais) ; ghost SNI + HTTP clair → fermeture sans un octet ; register ok/mismatch/skew/badsig sur porte TLS+ALPN ; **logs = 0 octet applicatif, 0 SNI ghost**.
- **e2e** (`provider`) : `relay-p6.feature` enrichi (porte TLS+ALPN, fermetures B.4, joignabilité HTTPS `@pod`) ; `relay-register.py` en TLS+ALPN (repli `RELAY_TLS=0`) ; `relay-reach.py` (reach|ghost|nosni|plain) ; steps behave (dry-run vert).
- **Terraform M2** : `modules/relay` étendu (env `AITHOS_RELAY_TUNNEL_NAME`, secrets `*_PEM`, grant exécution scoped) ; `modules/relay-cert` **(à retravailler out-of-band, cf. décision)** ; `envs/prod` composé. NLB M1 réutilisé tel quel. fmt OK ; câblage AWS `validate` OK.
- **Compteurs** : 29 unités + 34 BDD store + 12 BDD tunnel + 18 BDD passthrough + rejeux p1/p3/p4/p5 byte-exact + handshakes + witness/vectors verts ; **clippy 0, fmt OK**.

## TA MISSION — finir M2 orienté prod stable, puis STOP au gate

1. **Appliquer les 3 décisions tranchées ci-dessus** (out-of-band cert ; TCP keepalive + redline B.3 ; `/acme/txt` feature-first). Pour `/acme/txt` : écrire la/les feature(s) et les vecteurs AVANT le code, rejouer vert.
2. **Rejouer tout vert en local** : `cargo test -p aithos-provider --features pod-stub` (unités + BDD + rejeux byte-exact), smoke bout-en-bout relais+pod-stub sur localhost, behave dry-run.
3. **STOP gate déploiement.** Comme le provider acme disparaît (décision out-of-band), tu peux lancer `terraform plan` DEPUIS le sandbox (aws seul, backend S3 bucket `aithos-landings-tfstate-128066560720`, région eu-west-3). Produire le plan en artefact, le LIRE intégralement à Mathieu (attendu : 2 secrets vides + grant exécution scoped + révision task def relais + attache policy ACME-TXT au task role store ; 0 destroy). **Aucun apply sans la parole explicite de Mathieu.**
4. Après validation : Mathieu obtient le cert relais hors-bande + `put-secret-value` ; build image relais M2 (hand-assemble musl statique comme au P1 — le CDN alpine n'est pas joignable du sandbox) + push ECR + apply sur délégation ; **rejeu depuis une VRAIE machine** (proxy TLS :443) : `relay-register.py` TLS+ALPN + `relay-reach.py reach` avec `aithos-pod-stub` (cert via `/acme/txt`) derrière NAT → joignabilité HTTPS + relais aveugle prouvés.
5. **NE PAS toucher P2 en autonomie** (verify_chain = jugement d'autorité, gate humain). **NE PAS bumper `desired_count`** (cf. décision 3). Graver la note Lambda-vs-Fargate (§7) et la redline keepalive (annexe B) dans INFRA-PROVIDER — par gate.

## OÙ

- Code : `code/aithos-core` branche `feat/obligations`. Le crate `rust/crates/aithos-provider` (bins `aithos-store-api`, `aithos-relay`, `aithos-pod-stub`) est **sur disque, NON commité** (geste de gate) — travailler in situ, ne pas cloner à froid sans commit. Vecteurs `vectors/gen-p.py`+`verify-p.py`+`p1..p5-*.json`. `docker/relay.Dockerfile`.
- Provider : `provider` branche `feat/p6-p7-tunnel` — `infra/terraform/{modules,envs/prod}`, `e2e/` (behave), `.claude/skills/rituel-tests`, CI plan-only.
- ⚠️ cargo n'est PAS dans la VM device (device_bash sans réseau/cargo) : stager le crate vers le sandbox cloud pour compiler/tester, puis réécrire in situ. Écriture retour : `cp` (le mount bloque `unlink`, donc `tar x` échoue sur les fichiers existants — extraire dans un temp puis `cp -R temp/. dest/`).

## TESTER

- crypto+BDD : `cd code/aithos-core/rust && cargo test -p aithos-provider --features pod-stub`
- rejeu vecteurs Python : `cd code/aithos-core/vectors && python3 gen-p.py && python3 verify-p.py`
- smoke M2 local (vrais binaires) : lancer `aithos-relay` (cert auto-signé CA:FALSE via openssl, `AITHOS_RELAY_NONCE_BACKEND=memory`, bootstrap `bootstrap/relay.json`) + `aithos-pod-stub` (POD_RELAY_CA=cert relais) → `relay-reach.py reach` + `relay-register.py`.
- wire e2e : `E2E_BASE_URL=https://store.aithos.fr behave provider/e2e/features`
- sonde relais depuis une VRAIE machine (pas le sandbox) : `python3 provider/e2e/tools/relay-register.py [ok|mismatch|skew|badsig]` (TLS+ALPN) et `relay-reach.py reach`.

## GOTCHAS

- task def Fargate DOIT porter `AWS_REGION` ; image `FROM scratch` DOIT embarquer le bundle CA. Cert auto-signé de test = **CA:FALSE** (rustls refuse une CA en end-entity). yamux 0.13 : NE PAS `set_max_connection_receive_window` bas (assert `≥ max_num_streams × 256 KiB` → panic) — laisser `Config::default()`. Le relais ne parle pas HTTP (ligne JSON signée + LF ; TLS+ALPN aithos-tunnel/1 depuis M2). provider `vancluever/acme` = 403 depuis le sandbox (résolu par la décision out-of-band). Creds Mathieu dans `/Volumes/Math17/aithos/v2/.aws-env` (SSO, expire) — exporter pour terraform, purger après, jamais dans un log/dépôt. 4 EIP publiques allouées pour 2 attendues côté relais (~7 $/mois de reliquat dev→prod à vérifier/libérer).

## NORMATIF

INFRA-PROVIDER.md — Annexe A aithos-store (enveloppe A.2, ordre 0–10, erreurs A.7, logs A.8, CAS A.5), B aithos-tunnel (enregistrement B.2, passthrough B.3, SNI B.4, ACME B.5), C aithos-witness. JCS (RFC 8785), Ed25519 sur JCS-avec-signature.value="", clés multibase z6Mk…, BLAKE3, RFC 3339 Zulu. Contenu core : spec §02 (arbre : zones=dossiers racine, sections/dossiers par `sid` ULID, tags par wrap), §07 (gamma segments mensuels, 2 couches), §08 (connecteurs = manifeste d'actions signé, coffre `/x/<id>`).

PREMIÈRE ACTION : lire l'état (ce prompt + INFRA-PROVIDER annexes A/B + le crate in situ), confirmer le cadrage, puis écrire la feature + les vecteurs de `/acme/txt` AVANT le code ; appliquer en parallèle le cert out-of-band (Terraform) et le TCP keepalive. Rejeu vert, puis STOP gate (plan depuis le sandbox + creds Mathieu). Ne déploie rien sans plan lu + parole explicite de Mathieu.

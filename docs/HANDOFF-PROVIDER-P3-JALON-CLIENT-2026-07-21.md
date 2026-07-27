# HANDOFF — Lot B (P3/P4), partie 1 : client RemoteStore JALON VERT LOCAL + micro-redline A.1 (2026-07-21)

> **ARCHIVE — jalon intermédiaire.** P3/P4 sont ensuite passés au gate déployé.

Date : 2026-07-21. Dépôts : code/aithos-core + provider/. État DISQUE = vérité.
Statut : **le client P3 existe et parle le vrai wire — 16 scénarios de contrat
RED→GREEN contre le VRAI service sur socket, spike e2e mode B ET mode A 2/2
(le journal écrit et relu À TRAVERS le wire A.2)**. Le gate P3 (DEMO-LEA à
l'identique) n'est PAS clos : ni rejoué, ni gravé, ni passé au témoin, ni
déployé. Les changements SERVICE de ce jalon (micro-redline A.1, 304,
treillis) imposent un GATE DÉPLOYÉ avant gravure « servi en prod ».

Se lit avec HANDOFF-PROVIDER-P5-WITNESS-DONE-2026-07-20.md (+ ADDENDUM du
2026-07-21), INFRA-PROVIDER.md (annexe A = le wire), et le prompt de reprise
PROMPT-REPRISE-PROVIDER-P3-GATE-P4-2026-07-21.md.

## 0. Séquence (Mathieu présent, arbitrages AskUserQuestion)

1. Batterie d'entrée reconstruite et close (dont witness 12/12 après les
   commits P5 §4 de Mathieu, faits en session). AWS vérifié : tables
   control/heads 0, store 2/2, relay 1/1, witness 1/1, keys.json 200,
   racine 2026-07-20 réelle confirmée.
2. **Arbitrages Mathieu ①–⑤** : ① ureq+rustls (client bloquant minimal,
   pas de runtime dans la lib) ; ② seam signeur INJECTÉ (keyholder, jamais
   une clé en config) ; ③ mesures perf officielles = machine Mathieu
   (sandbox = pré-mesure indicative) ; ④ deux gates (P3 puis P4), un lot ;
   ⑤ cdn-public DANS ce lot (P4).
3. Feature-first : `tests/features/remote/store-remote-client.feature`
   (16 scénarios, RED constaté 16/16 failed) → `aithos-bundle/src/remote.rs`
   → VERT 16/16.
4. Confrontation mode B constatée sur le vrai service : layout natif du
   bundle vs grammaire A.1 + pretty-print vs JCS. **Arbitrage Mathieu :
   HYBRIDE + micro-redline additive, gravure annexe + BDD (pas de p10).**
5. Micro-redline servie (BDD RED→GREEN), canonicalisation JCS client,
   sidecar hybride, spike e2e 2/2 VERT. Write-back disque À CHAQUE jalon.
6. **Commits faits par Mathieu en session** : aithos-core `294df36` (P5
   witness) + `bb31d71` (P3 jalon client) ; provider `46399b0` (P5 infra).
   Les fichiers modifiés APRÈS bb31d71 (liste §4) partent au commit de gate.

## 1. Preuves (2026-07-21, sandbox ; wire public pour l'état prod)

| Preuve | Résultat |
|---|---|
| cargo check --locked workspace | EXIT=0 |
| cucumber | store **150/150** (974) = 146 + 2 revalidation-304 + 2 redline-A.1 ; tunnel 12/12 ; relay 27/27 ; witness 12/12 |
| replays byte-exact | vectors 5/5, p3 2/2, p5 1, p6 2/2, handshake 4/4, witness p4 3/3 — vecteurs INTOUCHÉS |
| **contrat client P3** (cucumber_remote, vrai service sur socket) | **16/16** (142 steps) : enveloppe A.2 acceptée par le service, body_b3, nonces frais, not_covered typé, list paginé, publish CAS genesis+successeur (têtes byte-exactes des vecteurs p7), 409→tête adoptée (rebase), POST /gamma + conflit, retries+backoff bornés (jamais un 4xx), cache immutable 1 hit, no-store 2 hits, If-None-Match→304 |
| **spike e2e journal remote** (gateway, vrai service in-process) | **2/2** : mode B = Bridge complet sur RemoteStore (owner-init local → réplication owner par le wire, historique d'éditions rejoué → journal_write sous le pen mémoire → relu par lecteur owner indépendant) ; mode A = fs primaire + réplique asynchrone post-append convergée |
| non-régression gateway | cucumber 152/152, lib 85/85, e2e_demo_lea (fs) OK |
| état AWS (MCP aws-api, session Mathieu) | control **0 item**, heads **0 item**, store 2/2, relay 1/1, witness 1/1 ACTIVE |
| wire public | store.aithos.fr/healthz 200 ; witness.aithos.fr/keys.json 200 ; roots/2026-07-20.json 200 (l'addendum confirmé) |

## 2. Ce qui a été construit (tout sur le disque)

- **`aithos-bundle/src/remote.rs`** (feature `remote` = ureq+rustls+base64,
  committé bb31d71 puis étendu) : `RemoteStore` sync (trait Store), enveloppe
  X-Aithos-Auth (JCS, body_b3, nonce/at/entropie/horloge INJECTÉS, nonce
  frais par tentative), `EnvelopeSigner` (seam ; `KeySigner::owner|mandated`),
  publish CAS A.5 (`If-Head` de la tête suivie, accept ET 409 adoptent la
  tête servie — le 409 EST l'entrée du rebase), POST /gamma par diff de
  segment (mode B ; réplique PUT sinon), `heads()` adopte (« none » = connu
  vide), retries 502-504/transport avec backoff borné (jamais un verdict 4xx),
  cache A.6 tenu DEPUIS les en-têtes du wire (immutable / must-revalidate+ETag
  → If-None-Match/304 / no-store), **canonicalisation JCS des classes signées
  au dépôt** (manifest.json, did.json, certs/* — la signature couvre le JCS,
  le pretty local est une commodité), erreurs typées `RemoteError`
  (+`reason` du registre A.7), taps d'acceptation (enveloppes, requêtes,
  If-Head, statuts, backoffs ; debug `AITHOS_REMOTE_DEBUG=1`).
- **Service (post-bb31d71, à committer au gate)** :
  - micro-redline A.1 : `e/<zone>/header.json` (zone ∈ {circle,self,x}) et
    `e/<zone>/root.enc` (circle,self) SERVABLES (ObjectPath::ZoneHeader/
    ZoneRoot, classe private-revalidate + ETag fort, contrôle léger A.4) ;
    `gateway/**` et `manifests/tree-*` épinglés HORS grammaire par BDD ;
  - If-None-Match → **304** sur les classes revalidate (2 scénarios) ;
  - **treillis §04.2 côté couverture** : la LECTURE d'un objet de zone est
    servie par TOUT verbe de la zone (« append crée et lit ») ;
  - **gamma lisible par l'appendeur** : GET segment couvert aussi par
    l'ensemble POST-/gamma (write-verbs ∪ act) — voir consignés §3.
- **Gateway** : `StoreConfig::Remote { url, tenant, did, mandate, local }`
  (mode B ; `local` = SIDECAR fs des clés runner/dérivées, Mem sinon) et
  `StoreConfig::Replicated { root, url, … }` (mode A) ; validation config
  fail-closed ; `GatewayStore::from_config_with_identity` (seam keyholder →
  signeur agent ; `from_config` REFUSE remote sans identité — les chemins
  owner CLI restent fs) ; routage hybride mode B (grammaire → wire,
  `gateway/**`+`manifests/*` → sidecar, list fusionné) ; décorateur mode A
  (fs primaire, réplication asynchrone post-publish ET post-append,
  `replicate_now()`/`join_replication()`, échec de sweep = bruit opérationnel
  jamais une erreur du primaire) ; `core_bridge::Runtime::open` câblé.
- **Harnais** : `cucumber_remote.rs` (service réel sur socket + proxy à
  pannes TCP + compteurs par méthode/chemin) ; `e2e_journal_remote.rs`
  (spike modes B et A — la mécanique de rejeu de l'historique d'éditions
  via les slots `manifests/<h>.json` y est écrite).

## 3. Consignés (pour le témoin adversarial du gate P3 — AUCUN gravé)

- **C1 (à confronter)** : la lecture gamma élargie à l'ensemble des
  appendeurs (write-verbs ∪ act) — question de CONFIDENTIALITÉ du log
  (un pen scopé lit tout le squelette clair du gamma). Défense : anti-abus
  jamais autorité, les corps scellés restent scellés, le store voit déjà ce
  squelette ; la ligne read.gamma reste la voie des tiers. À trancher/graver.
- **C2 (à confronter)** : le treillis lecture (tout verbe de zone sert la
  lecture de la zone) élargit ce qu'un pen dir-scopé peut LIRE côté serveur
  (le sélecteur n'exclut plus) ; le périmètre réel reste enforcé par le core.
- **C3** : la canonicalisation JCS au dépôt côté client — « ce qui est signé
  = ce qui est envoyé » est préservé (la signature couvre le JCS), mais les
  octets locaux ≠ octets wire ; à graver comme convention client A.1.
- **C4** : le sidecar mode B — `gateway/state.json` (ids de mandats du
  runner) et `manifests/*` (slots serveur / caches dérivables) ne quittent
  jamais le pod ; un runner éphémère sans `local:` doit savoir re-dériver
  (state.json N'EST PAS re-dérivé aujourd'hui — le mode B éphémère complet
  exige ce point ou un `local` persistant ; consigné, pas résolu).
- **C5** : clippy neuf (rustc 1.95) : 2 warnings PRÉEXISTANTS hors lot
  (relay.rs const-assert, store_admin.rs to_string) — non touchés.
- **C6** : le rejeu d'historique d'éditions (owner_replicate du spike)
  vit dans le TEST ; en faire une surface client (`replicate_history`) est
  un choix à faire au gate DEMO-LEA.

## 4. Reste pour clore le gate P3 (puis P4) — le prochain contexte

1. **DEMO-LEA à l'identique** : paramétrer `e2e_demo_lea.rs` (fs + remote
   dans le même fichier ; la mécanique est prouvée par le spike) — journal
   mode B (seed owner par le wire, beats par la gateway BINAIRE via yaml
   `journal: store: {kind: remote, url, tenant, did, mandate: [pen], local}`),
   un contexte (ventes) mode A répliqué et relu. Assertions finales du
   journal par lecteur REMOTE.
2. **Gate déployé** (les changements service l'exigent) : image store
   reconstruite (API ECR, couche unique — modèle push-witness-image.py),
   plan lu INTÉGRALEMENT (4 -var), apply, rejeu wire (tenant de rejeu DID
   FRAIS — piège D8), plan final 0, purge, tables 0.
3. **Gravures INFRA-PROVIDER** : micro-redline A.1 (header/root + exclusions
   runner), 304/If-None-Match (A.6), treillis lecture + gamma-appendeur
   (A.3, selon verdict témoin C1/C2), convention JCS client, note §3.5
   mode B hybride/sidecar + mode A réalisés.
4. **Témoin adversarial en AGENT** (un bloquant = corrigé avant clôture) ;
   handoff DONE ; blocs de commit pour Mathieu ; write-back ; repos.
5. **P4** : client /sync + /batch (le serveur les sert déjà — p9) ; module
   cdn-public (arbitrage ⑤) ; script de bench pour la machine Mathieu
   (officiel, arbitrage ③) + pré-mesure sandbox ; chiffres §3.6 gravés.

## 5. Environnement (delta session)

Sandbox reconstruit (~250 fichiers ; .feature par extraction git-objects —
méthode P5, VALIDÉE aussi dans le sens commit : device_commit_files ÉCRIT
les .feature sans blocage) ; VM device morte (pas de device_bash) ; le PONT
ressert des copies périmées (reflogs !) — vérifier le CONTENU, pas le mtime ;
MCP aws-api opérationnel après `aws sso login` Mathieu (profil aithos-prod) ;
toolchain : rustc 1.95 stable, behave/pynacl/base58/blake3 pip ; le spike a
laissé `AITHOS_REMOTE_DEBUG` (tap de debug client, inerte sans env).

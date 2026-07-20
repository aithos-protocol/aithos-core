# INFRA-PROVIDER — Architecture de production : le provider Aithos

> **Statut : DÉCIDÉ v1 — 2026-07-16 (arbitrages Mathieu).**
> Ce document fixe la doctrine d'hébergement, le design du provider (store, témoin,
> relay, dashboard) et les **contrats d'interface C1–C3** entre les deux plans
> d'action parallèles : [`HANDOFF-PROVIDER-AWS.md`](HANDOFF-PROVIDER-AWS.md) et
> [`HANDOFF-GATEWAY-HUB.md`](HANDOFF-GATEWAY-HUB.md). Il prolonge
> `GATEWAY-HANDOFF.md` §3bis.1, `DESIGN.md` §7 et `STANDARDS-COMPAT.md` (C1)
> **sans modifier le core**. L'analyse de couverture produit est dans
> [`GAPS-DEMO-E2E.md`](GAPS-DEMO-E2E.md).
> **Annexes normatives A–C gravées le 2026-07-16 (lot P0, en attente du gate)** :
> wire `aithos-store` (C1, annexe A), tunnel `aithos-tunnel` (C2, annexe B),
> checkpoint `aithos-witness` (C3, annexe C) ; vecteurs `vectors/p1…p4-*.json`.

## 1. Doctrine

**La ligne de fracture.** Peut tourner chez Aithos : tout ce qui **déplace des
octets et vérifie des signatures/preuves**. Ne tourne jamais chez Aithos : tout ce
qui **ouvre un scellé, tient une clé privée, ou résout un secret**. Conséquence
directe de I1 et de « a server is never a trust party » (§00) — promue ici au rang
de règle d'hébergement opposable à tout service futur.

**Les trois plans.**

| Plan | Où | Contenu |
|---|---|---|
| Autorité & clés | Client | master seed, succession, seeds runner (`agent.id`), frappe/révocation de mandats, déchiffrement, coffre de credentials |
| Exécution | Client + fournisseurs de ressources | pod agent + gateway (enforcement, log-before-relay, injection de tokens), LLM |
| Disponibilité & preuve | **Aithos (AWS)** | RemoteStore (ciphertext), distribution certs/DID/révocations, sérialisation CAS, témoin, relay hub, control plane, distribution logicielle signée |

**Jamais chez Aithos** : le keyholder, le `CredentialBroker`, un AS OAuth partagé,
un coffre « managé », tout composant qui déscelle une DK ou signe une entrée gamma
pour le compte d'un client. La neutralité est le produit : Aithos opère le greffe
(modèle Certificate Transparency / Sigstore) — les fonctions qui gagnent à être
tenues par un tiers précisément parce qu'elles n'exigent aucune confiance.

**Le résidu de confiance honnête.** Aithos reste l'éditeur du binaire gateway.
Réponse : chaîne d'approvisionnement, pas hébergement — builds reproductibles
(l'image `FROM scratch` statique s'y prête), hashes publiés et **notarisés par le
témoin** (§4), spec ouverte + vecteurs, audit externe à terme.

**Bornage de tuyau, jamais bornage d'action.** Aithos ne peut pas borner une
action : les bornes sont scellées vers owner+gateway (illisibles pour nous par
construction), les arguments sont des données métier, et la gateway revérifie de
toute façon. Aithos borne le tuyau : rate limits, quotas de connexions, filtrage
IP, coupure d'un tenant — sans rien lire. Une infrastructure qui ne peut pas
décider ne peut pas mal décider ni être contrainte de décider.

## 2. Arbitrages gravés

| # | Décision (2026-07-16) | Détail |
|---|---|---|
| A1 | Pistes **P et G en parallèle** après ce doc | contrats C1–C3 ci-dessous ; la démo BYO ne dépend pas du store |
| A2 | Accès au store **sous mandat par défaut** | requête signée + `verify_chain` + `covers()` ; exceptions : zone `public`, DID doc ; certs/révocations : toggle de visibilité par Ethos |
| A3 | Relay **passthrough TCP/SNI dès v1** | Aithos ne voit ni payloads MCP ni tokens ; certs par org via ACME DNS-01 délégué |
| A4 | **Témoin dès la v1** du store | contre-signature des têtes + checkpoints publics |
| A5 | DR : backup versionné **même région** (eu-west-3) en v1 | pas de cross-région avant formulation contractuelle de la résidence EU |
| A6 | Nommage : hub `<org>.mcp.aithos.fr` ; store `store.aithos.fr` ; témoin `witness.aithos.fr` ; app `app.aithos.fr` | le sous-domaine par org est **requis** par A3 : le routage par chemin est impossible sans terminer TLS |

## 3. Le RemoteStore (plan de données)

### 3.1 Rôle et non-rôle

Un backend `Store` de plus (§3bis.1) : miroir + sérialisation + gate anti-abus.
Le `covers()` serveur est un **contrôle de disponibilité et de métadonnées, jamais
l'enforcement** — un serveur compromis ne fait pas lire un blob sans ligne de
header. Le serveur est volontairement bête : toute l'intelligence (merge disjoint,
forks, vérification) reste côté client, où la spec la met (§02.6).

### 3.2 Wire v0 — `aithos-store: "1.0.0-draft.1"`

> Le détail normatif — routes complètes, forme exacte de l'enveloppe, ordre des
> vérifications, registre d'erreurs, CAS — est **gravé en annexe A (contrat
> C1)** ; en cas d'écart de formulation avec le croquis ci-dessous, l'annexe
> prime.

Base : `https://store.aithos.fr/t/<tenant>/<did>/…` (le tenant route et facture,
le DID porte l'autorité ; les deux ne se confondent jamais).

| Verbe | Route | Sémantique |
|---|---|---|
| GET | `/t/…/<path>` | lecture d'un objet du bundle (chemins spec §02.3) |
| GET | `/t/…?list=<prefix>` | listing par préfixe |
| POST | `/t/…/batch` | `get_many` : N chemins, une réponse multipart |
| PUT | `/t/…/<path>` | dépôt d'un artefact **déjà signé** ; le serveur vérifie avant d'accepter |
| PUT conditionnel | idem + `If-Head: <sha256>` | **CAS** sur les deux têtes chaudes : `manifest.json` (hash du manifest courant) et segment gamma courant (hash de la dernière entrée). Mismatch → `409` + tête courante ; le client rebase/merge et republie |
| POST | `/t/…/sync` | `{ have_edition: N }` → pack des chemins changés depuis N (descente de racines, §02.10) en un aller-retour |

**Enveloppe signée** (toutes routes, sauf exceptions A2) : en-tête
`X-Aithos-Auth` = JCS signé Ed25519 de
`{ method, path, body_b3, at (RFC 3339), nonce, mandate: [ids] }` par la clé de
l'appelant (owner `#root`/`#content`, gateway, auditeur, grantee). Les certs
référencés vivent dans le bundle ; au premier contact ils peuvent être joints.
Anti-rejeu : fenêtre ±300 s + nonce LRU (DynamoDB TTL). Vérification serveur :
signature d'enveloppe → `verify_chain(T=now)` (cache par `(mandate_id,
tête_de_révocation)`) → path-map §3.3. Le chemin chaud se réduit à une
vérification Ed25519 + un lookup.

### 3.3 Path-map `covers()` (v1, lecture + append + publish)

| Périmètre du mandat | Chemins servis |
|---|---|
| — (anonyme) | `e/public/**`, `did.json` ; `certs/**` + entrées `revoke` si toggle visibilité = public |
| `read.gamma[#…]` | `gamma/**`, `certs/**` (filtrage fin des kinds côté client/export ; le serveur filtre par sélecteurs grossiers) |
| `read.<zone>#dir/tag/id=…` | `manifest.json`, index de zone, `e/<zone>/hdr/**` et `e/<zone>/blobs/**` du sous-arbre couvert |
| verbes d'écriture délégués (pass L) | PUT des blobs/headers du périmètre + append gamma — le serveur vérifie l'entrée **comme un verifier** (signature feuille, `authorized_via`, `prev`) |
| owner / délégué avec `authorized_by` | PUT `manifest.json` (publish, CAS obligatoire) |
| `act.x.<id>.config` | `x/<id>/**` |

Défaut : refus. Le serveur ne « comprend » jamais un contenu — il vérifie des
signatures et des périmètres sur des chemins.

### 3.4 Cache et immuabilité

Objets immuables (blobs par `(sid, key_version)`, entrées gamma, certs, éditions
passées) : `Cache-Control: immutable`, servis via CloudFront quand publics, cache
local client sans invalidation. Objets mutables : `manifest.json`, tête gamma,
`hdr/*.json` (mutent par révision de header) — minuscules, jamais cachés. Règle
produit : **on synchronise puis on navigue localement** — jamais un aller-retour
par clic ; le diff d'éditions fait le reste en O(changé × log n).

### 3.5 Les deux modes (même backend, même wire)

| | Mode A — local-primary + réplique | Mode B — provider-primary |
|---|---|---|
| Primaire | fs dans le pod | le provider |
| Provider | réplique asynchrone post-publish + témoin + lectures tierces (auditeurs, dashboard) | vérité + sérialisation CAS ; débloque multi-runners et containers éphémères |
| Panne provider | l'agent continue (log-before-relay local) | **fail-closed** : actes refusés |
| Usage type | Ethos de contexte d'entreprise (souveraineté) | journal / Ethos de travail de l'agent (§3bis.3), démos SaaS |

Déclaré par contexte et par journal dans la config gateway (`store: { kind:
remote, url, tenant }` ; le mode A est un décorateur de réplication au-dessus du
même client).

### 3.6 Cibles de performance (gates, façon §09.3)

| Mesure | Cible |
|---|---|
| Navigation sur cache local (hit) | p50 < 5 ms |
| Sync à froid, 1 000 sections | < 2 s |
| Append d'acte (mode B, depuis l'Europe) | p50 < 120 ms |
| GET objet immuable (CloudFront hit) | p50 < 30 ms |
| Disponibilité store v1 | 99,9 % |

## 4. Le témoin — **contrat C3**

> Format, feed, racine quotidienne et règle d'équivocation **gravés en
> annexe C** ; le croquis ci-dessous reste la vue d'ensemble.

**Format checkpoint** (`aithos-witness: "1.0.0-draft.1"`), JSON signé :

```jsonc
{ "aithos-witness": "1.0.0-draft.1",
  "did": "did:aithos:z6Mk…",
  "edition_height": 42,
  "manifest_hash": "sha256:…",
  "gamma_head": "sha256:…",
  "observed_at": "2026-07-16T12:00:00Z",
  "witness_key": "z6Mk…",
  "signature": { "alg": "ed25519", "value": "<hex>" } }
```

Déclencheurs : à chaque publish accepté (mode B) ou réplique reçue (mode A) +
heartbeat quotidien par DID. Publication : feed public append-only
`https://witness.aithos.fr/<did>.jsonl` + racine quotidienne agrégée. Clé :
KMS sign-only, rotation annuelle — elle signe des **observations, jamais de
l'autorité** (aucun client ne dépend d'elle pour agir). Usages : anti-équivocation
(deux checkpoints incompatibles = preuve), borne de fraîcheur pour la révocation,
notarisation des hashes de binaires (supply chain). Gossip 2-of-N avec des témoins
tiers : plus tard, le format le permet (rien à changer côté clients).

## 5. Le hub public (plan de joignabilité) — **contrat C2**

> Le protocole tunnel — enregistrement signé, multiplexage, keepalive, routage
> SNI, API ACME déléguée — est **gravé en annexe B**.

**Nommage** : `<org>.mcp.aithos.fr` (A6). Wildcard `*.mcp.aithos.fr` → relay.
Onboarder un client = une écriture control plane, zéro ticket DNS.

**Relay** : NLB TCP :443 → routeur SNI → tunnel du pod. Le pod ouvre une
connexion **sortante** TLS persistante multiplexée vers `relay.aithos.fr` et
s'enregistre : `{ tenant, hostname, gateway_pub, at, nonce }` signé par la clé de
gateway ; le relay vérifie contre le mapping control plane
(`gateway_pub ↔ tenant ↔ hostname`, posé à l'enrôlement — **zéro secret
nouveau**). Les flux entrants dont le SNI correspond sont pipés dans le mux.
Keepalive 30 s, reconnexion backoff. Aithos voit : SNI, volumes, timing. Rien
d'autre (A3).

**Certificats** : ACME DNS-01 délégué — la zone est à nous, le pod obtient et
renouvelle le cert de son hostname ; **la clé TLS privée reste chez le client**.

**OAuth = projection du mandat** (chantier C1 de STANDARDS-COMPAT, exécuté côté
gateway — voir `HANDOFF-GATEWAY-HUB.md` G3–G5). L'AS est servi **par la gateway à
travers le tunnel**, jamais chez Aithos : un AS chez nous pourrait fabriquer des
sessions (le token remplacerait la preuve de possession de clé). Session = la
personne frappe un **sous-mandat de session** vers `gateway_pub` ; le token pointe
ce sous-mandat ; actes signés sous la chaîne owner → personne → session ; budgets
décomptés par la règle de sous-arbre ; non-répudiation complète.

**Deux flux à ne jamais confondre** : *hub* (l'agent externe parle MCP à la
gateway ; le manifest est descellé à la gateway ; rien n'est déchiffré chez
l'utilisateur) vs *lecture d'Ethos* (un humain/outil tire ciphertext + headers du
RemoteStore et déchiffre localement).

**Sortie libre** : un client peut exposer `mcp.acme.com` avec ses certs et se
passer du relay — rien ne casse. Le relay est une commodité facturable, pas une
captivité.

## 6. Le dashboard

App statique `app.aithos.fr` (S3+CloudFront). Données par **requêtes signées
depuis le navigateur** : RemoteStore (historique, preuves) et gateway via son
hostname public (live, surface owner/auditeur mandat-gated — G7). Vérification
Merkle/signatures et déchiffrement **en wasm dans le navigateur** ; clés chargées
localement (fichier v1, `RemoteVault` plus tard). Écritures d'owner : signées côté
navigateur, poussées comme artefacts. L'OAuth ne sert que pour les hosts tiers
(Claude, ChatGPT) — notre app parle requêtes signées nativement. Slogan : *la
dashboard, c'est chez vous, affiché par nous.*

## 7. Terraform et environnements

Prolonge `infra/terraform` (bootstrap d'état, rôle OIDC GitHub — patterns
existants). Modules : `dns` (wildcards + délégation ACME), `cdn-public`
(landings + app + witness feed + zone publique du store), `store-api`
(ALB + Fargate + S3 + DynamoDB), `relay` (NLB + service SNI/tunnels), `witness`
(clé KMS + feed), `control-plane-min` (table tenants + CLI d'admin interne).
Environnements `dev`/`prod` ; région `eu-west-3` (+ `us-east-1` pour les certs
CloudFront, comme les landings). Service : Rust axum **toujours chaud** sur
Fargate — pas de Lambda sur le chemin chaud (cold starts = la latence qu'on
refuse) ; réutilise `aithos-bundle` tel quel. Budget MVP : dizaines d'€/mois hors
egress ; l'immuable caché écrase l'egress répété ; le poste qui scale = le relay.

> **Note gravée 2026-07-18 (arbitrage Mathieu, gate M2) — compute du store :
> Lambda vs Fargate, à trancher au gate P2.** Le **relais reste Fargate/NLB
> quoi qu'il arrive** — c'est l'unique composant always-on impératif : un
> processus vivant tient les tunnels TCP persistants sans terminer le TLS,
> ce que Lambda ne peut pas faire. Pour le **store**, la phrase ci-dessus se
> nuance : les cold starts Rust sont modestes (~15–30 ms) ; **Lambda gagne à
> volume bas / en pics** (dev/early), **Fargate à haut volume soutenu**. La
> décision se prend **au gate P2**, quand S3 arrive et qu'on touche la
> couche stockage (sortir l'objet de la mémoire par tâche est de toute façon
> le préalable, HA comprise). Le témoin et le plan public peuvent aller
> serverless plus tard sans toucher les wires.

## 8. Métadonnées, conformité, exploitation

- **Politique de métadonnées (à documenter comme limite, façon chiffrement au
  repos)** : le store voit qui demande quels chemins, quand ; le squelette clair
  du gamma. Logs applicatifs sous discipline de rédaction type `credentials.rs` :
  jamais un chemin de section ni un corps dans un log ; rétention 30 j ; DPA.
- **Multi-tenant** : préfixes S3 + conditions IAM par tenant ; quotas (Go,
  requêtes, egress relay) comptés au tenant.
- **Anti-rejeu** : skew toléré ±300 s ; nonces TTL.
- **DR (A5)** : versioning S3 + backup même région ; restauration testée.
- **GC / crypto-erasure** : la supersession d'éditions (§06, rung 4) devient une
  opération outillée côté store (rétention des éditions supersédées : 30 j puis
  purge, configurable par tenant) — le droit à l'effacement s'exerce par rotation
  + purge, à documenter.
- **Versionnage wire** : `aithos-store` / `aithos-witness` en `1.0.0-draft.1`,
  même convention que le core ; toute rupture = bump + période de double-service.

## 9. Hors périmètre (renvois)

Console self-service et facturation (control plane v2) ; `RemoteVault` (garde
distante des clés utilisateur — `GATEWAY-BOOTSTRAP.md` §4bis) ; gateway hébergée
sous enclave (voie écartée v1) ; fédération XAA/ID-JAG et AS externes
(`STANDARDS-COMPAT.md` C2) ; hardening pack connecteurs (GitHub App et tokens
courts — opt-in client, plus tard).

---

## Annexe A (normative) — wire `aithos-store: "1.0.0-draft.1"` — contrat C1

> Gravée 2026-07-16, lot P0 (`HANDOFF-PROVIDER-AWS.md`). Elle précise §3.2–3.4 ;
> en cas d'écart, l'annexe prime. Toute rupture = bump (`draft.2`, …) + période
> de double-service (§8). Vecteurs : `vectors/p1-store-envelope.json`,
> `vectors/p2-store-cas.json` (générateur indépendant `vectors/gen-p.py`).
> La surface de preuve gateway (piste G, lot G7) réutilise **la même enveloppe**
> A.2 — seule l'autorité (`host`) change.

### A.1 Conventions

- **Version.** Toute réponse porte `X-Aithos-Store: 1.0.0-draft.1`. Le client
  PEUT envoyer le même en-tête ; version majeure inconnue → `426` +
  `version_unsupported`.
- **Encodages** (règles de `vectors/README.md`) : octets bruts en hex
  minuscule ; têtes de chaînage préfixées `sha256:<hex>` ; `body_b3` = hex
  BLAKE3 nu ; clés publiques en multibase base58btc/multicodec (`z6Mk…`) ;
  instants RFC 3339 Zulu ; tout JSON signé = RFC 8785 (JCS).
- **Signature.** Convention partagée avec manifest et DID doc (§01.4) : la
  signature couvre le JCS du document avec `signature.value = ""` ; `value` =
  signature Ed25519 en hex.
- **Routes données.** `/t/<tenant>/<did>/<chemin>` ; `tenant` =
  `[a-z0-9][a-z0-9-]{2,31}` ; `<did>` littéral (jamais percent-encodé) ;
  `<chemin>` appartient **exactement** à la grammaire de layout §02.3
  (`manifest.json`, `did.json`, `e/public/**`, `e/<zone>/index.json`,
  `e/<zone>/blobs/<sid>.enc`, `e/<zone>/hdr/<node>.json`, `x/<id>/…`,
  `certs/<mandate_id>.json`, `gamma/<YYYY-MM>.jsonl`). Tout chemin hors
  grammaire → `path_invalid`, avant même l'enveloppe (fail-closed, zéro
  interprétation).
- **Layout draft.2 servable (redline gate 5, 2026-07-20).** La grammaire
  admet ADDITIVEMENT les chemins du layout porteur K1-B/K1-C, sous-ensemble
  exact de la grammaire fermée du bundle (`validate_store_key`) :
  `manifests/<h>.json` (`<h>` = entier décimal ≥ 1, sans zéro de tête — le
  slot d'édition écrit par le publish A.5, jamais par un PUT client) ;
  `changesets/<64hex>.json` et `evidence/<64hex>.json` (64 hex minuscules =
  le suffixe du digest K1-C §02.6.3) ; les alias K1-C `public/sections/<sid>.md`,
  `circle/blobs/<sid>.json` (même grammaire `<sid>` que `e/<zone>/blobs/`),
  et les trois clés littérales `indices/public.json`, `roots/public.json`,
  `vault/catalog-pins.json`. Rien d'autre : les clés internes du bundle
  (`manifests/tree-…`, `manifests/index-…`, suffixe `-alt`, `gateway/**`,
  `gamma/gamma.jsonl`) restent HORS grammaire wire — `path_invalid`.

### A.2 L'enveloppe signée `X-Aithos-Auth`

Obligatoire sur toutes les routes **sauf** les GET anonymes A2 :
`e/public/**`, `did.json`, et `certs/**` + entrées `revoke` du gamma si le
toggle tenant `certs_public` est vrai (P7 ; défaut : faux).

```
X-Aithos-Auth: base64url-sans-padding( JCS(enveloppe) )
```

```jsonc
{ "v": 1,
  "host": "store.aithos.fr",              // autorité, minuscule, sans port par défaut
  "method": "PUT",                        // verbe HTTP, majuscules
  "path": "/t/acme/did:aithos:z6Mk…/manifest.json",  // request-target exact, query incluse
  "body_b3": "<hex BLAKE3(corps brut)>",  // "" si la requête n'a pas de corps
  "at": "2026-07-16T12:00:00Z",           // RFC 3339 Z
  "nonce": "<opaque ; guidance CLIENT : 16–64 car., ≥ 96 bits d'entropie ; le serveur n'impose que la borne haute ≤ 64 (anti-abus) — redline gate 4, 2026-07-20>",
  "mandate": ["mandate_<racine>", "…", "mandate_<feuille>"],  // [] pour l'owner
  "key": "#root" | "#content" | "z6Mk…",  // fragment DID (owner) | pubkey feuille (mandaté)
  "signature": { "alg": "ed25519", "value": "<hex>" } }
```

`host` est une précision additive au croquis §3.2 : la même enveloppe sert la
surface de preuve gateway (G7), et lier l'autorité interdit le rejeu
inter-plans. `mandate` liste la chaîne complète, racine d'abord, feuille en
dernier — l'ordre d'`authorized_via` (§07.2).

**Ordre de vérification (normatif, fail-closed : la première erreur répond,
rien d'autre n'est évalué) :**

| # | Contrôle | Erreur (A.7) |
|---|---|---|
| 0 | chemin dans la grammaire A.1 | `path_invalid` |
| 1 | tenant connu et non suspendu ; DID lié au tenant (P7) | `unknown_tenant` / `suspended` / `did_not_bound` |
| 2 | en-tête présent ; base64url, JSON et forme valides ; `v == 1` ; champ inconnu ⇒ rejet | `envelope_missing` / `envelope_invalid` |
| 3 | `host`, `method`, `path` == requête reçue, octet à octet | `envelope_invalid` |
| 4 | `body_b3` == BLAKE3(corps brut) — ou `""` et absence de corps | `envelope_invalid` |
| 5 | `\|now_serveur − at\| ≤ 300 s` | `clock_skew` |
| 6 | `(key, nonce)` jamais vu — fenêtre ≥ 600 s, réservation (insert-if-absent, DynamoDB TTL) **avant** tout effet de bord | `nonce_replayed` |
| 7 | résolution de clé — `#root`/`#content` : DID doc stocké du `<did>` du chemin (exception genèse A.4) ; multibase : `mandate` non vide **et** `feuille.grantee.pubkey == key` | `chain_invalid` |
| 8 | signature d'enveloppe sous la clé résolue | `signature_invalid` |
| 9 | mandaté : `verify_chain` §04.5 étapes 1–6 — signatures lien à lien, `subject == <did>`, fenêtres évaluées à `at`, atténuation §05.3, révocation évaluée à `now_serveur` sur le gamma stocké | `chain_invalid` / `chain_revoked` |
| 10 | path-map A.3 (`covers()` anti-abus, §3.3) | `not_covered` |

Le serveur n'évalue **aucune contrainte de comptage** (budgets, `max_actions`,
obligations, heartbeat) : les compter ici serait de l'autorité (§3.1) — c'est
l'affaire des verifiers et de la gateway. Cache de chaîne autorisé par
`(mandate_feuille_id, epoch_de_révocation)` ; l'epoch du DID avance à chaque
entrée `revoke` acceptée.

**Certs joints (premier contact).** En-tête optionnel `X-Aithos-Certs:
base64url-sans-padding(JSON [mandats], ordre racine→feuille)`, ≤ 64 KiB. Le
serveur vérifie la chaîne puis PEUT matérialiser les `certs/<id>.json` absents
(artefacts world-readable, §04.9) ; il ne les modifie jamais.

### A.3 Routes et path-map

| Verbe | Route | Sémantique |
|---|---|---|
| GET | `/t/<t>/<did>/<chemin>` | lecture d'un objet |
| GET | `/t/<t>/<did>?list=<préfixe>[&after=<chemin>][&limit=<n≤1000>]` | listing paginé : `{"paths": […], "truncated": bool}` — filtré au périmètre couvert (grossier) |
| GET | `/t/<t>/<did>/heads` | `{"height": N, "manifest": "sha256:…"\|null, "gamma": "sha256:…"\|null, "segment": "<YYYY-MM>"\|null}` — les têtes chaudes |
| POST | `/t/<t>/<did>/batch` | corps `{"paths": […]}` (≤ 256) → `multipart/mixed`, une part par chemin, ordre de la requête ; par part : `Content-Location` + `X-Aithos-Status: 200\|403\|404` (corps seulement si 200) |
| PUT | `/t/<t>/<did>/<chemin>` | dépôt d'un artefact **déjà signé** ; vérification A.4 avant acceptation |
| PUT | `…/manifest.json` + `If-Head` | **publish, CAS obligatoire** (A.5) |
| POST | `/t/<t>/<did>/gamma` + `If-Head` | **append d'UNE entrée** (corps = JCS de l'entrée), CAS obligatoire — le chemin chaud du mode B |
| PUT | `…/gamma/<YYYY-MM>.jsonl` + `If-Head` | réplique de segment (mode A) : le contenu stocké doit être un **préfixe octet à octet** du nouveau, chaque entrée ajoutée vérifiée A.4 |
| POST | `/t/<t>/<did>/sync` | corps `{"have_edition": N}` → pack `multipart/mixed` des chemins changés depuis N (descente de racines §02.10), `manifest.json` en première part ; édition N purgée → `410` + `edition_gone` (resync complet) |

Path-map (`covers()` serveur — reprend §3.3, grammaire §04.2) :

| Périmètre de la chaîne | Chemins servis |
|---|---|
| — (anonyme, A2) | GET `e/public/**`, `did.json` ; + `certs/**` et entrées `revoke` si `certs_public` ; + GET `public/sections/**`, `indices/public.json`, `roots/public.json` (alias K1-C de la zone publique — même statut que `e/public/**` ; redline gate 5, 2026-07-20) |
| toute chaîne valide du DID | GET `/heads`, `manifest.json`, `did.json`, `certs/**` ; + GET `manifests/<h>.json`, `changesets/**`, `evidence/**`, `vault/catalog-pins.json` (matériel de preuve public par construction K1-B — nécessaire au cold verify sans capacité privée ; redline gate 5, 2026-07-20) |
| `read.gamma[#…]` | GET `gamma/**` (filtrage fin des kinds côté client/export ; le serveur filtre par sélecteurs grossiers `since`/`until` → segments) |
| `read.<zone>[#sel]` | GET index de zone, `e/<zone>/hdr/**`, `e/<zone>/blobs/**` du sous-arbre couvert (`dir=` nodal §04.2 ; sans index chargé le serveur sert le fichier si le sélecteur ne peut exclure le chemin — anti-abus, jamais l'autorité) ; + GET `circle/blobs/<sid>.json` pour `read.circle` (l'alias K1-C du blob de zone — mêmes règles de sélecteur que `e/circle/blobs/**` ; redline gate 5, 2026-07-20) |
| verbe d'écriture (`edit\|append\|write\|delete`) sur la zone (pass L) | PUT `e/<zone>/blobs/**`, `e/<zone>/hdr/**`, index de zone ; POST `/gamma` (A.4) ; + PUT `circle/blobs/<sid>.json` (zone `circle`) et `public/sections/**` (zone `public` — la ligne d'écriture publique qui manquait ; redline gate 5, 2026-07-20) |
| owner, ou délégué avec `authorized_by` (§02.6) | PUT `manifest.json` (CAS), `did.json`, `certs/**`, `gamma/<YYYY-MM>.jsonl` (réplique) ; + PUT `changesets/<64hex>.json`, `evidence/<64hex>.json`, `indices/public.json`, `roots/public.json`, `vault/catalog-pins.json` (sidecars et dérivés d'une publication draft.2, déposés AVANT le publish qui les épingle ; redline gate 5, 2026-07-20) |
| `act.x.<id>.*` | GET/PUT `x/<id>/**` ; POST `/gamma` pour ses entrées `action`/`inference` |

Défaut : refus (`not_covered`). L'owner (`#root`/`#content`) couvre tout sur
son DID. Le serveur ne « comprend » jamais un contenu — il vérifie des
signatures et des périmètres sur des chemins.

**`manifests/<h>.json` n'a pas de ligne d'écriture (redline gate 5,
2026-07-20)** — le slot est écrit par le serveur lors d'un publish accepté
(A.5) ; tout PUT client sur `manifests/**` répond `not_covered` (le chemin
est dans la grammaire A.1 — ce n'est pas `path_invalid` — mais aucune
chaîne, owner compris, ne le couvre en écriture).

### A.4 Vérification des artefacts au dépôt (« le serveur vérifie avant d'accepter »)

Anti-abus fail-closed, jamais l'autorité : un artefact rejeté répond
`artifact_invalid` (+ un `reason` court, registre fermé) ; le serveur ne
corrige, ne complète, ne réécrit **jamais**.

- **`did.json`** : parse `aithos-did-core`, `id == <did>` du chemin `==`
  multibase(`keys.root`), clés bien formées, auto-signature `#root` (§01.4).
  **Genèse** : premier `did.json` d'un DID accepté si l'enveloppe `#root`
  vérifie sous la clé racine **du document déposé** et que le control plane
  liste ce DID pour le tenant (l'enrôlement P7 précède toujours — pas de
  chicken-and-egg). Rotation d'identité = document d'époque §01.4, vérifié
  sous la clé `succession` du document précédent.
- **`manifest.json`** : parse, version connue, signature racine ou déléguée
  (`authorized_via` §02.6 — chaîne vérifiée comme A.2#9), `edition.height ==
  height_stocké + 1`, `edition.prev_hash == chain_hash(manifest stocké)`,
  CAS A.5. `merges`/`resolves_fork` sont acceptés tels quels : le store
  **n'arbitre jamais un fork** — le CAS sérialise, les perdants rebasent
  (§02.6 reste client-side), le témoin observe.
- **`certs/<id>.json`** : parse mandat, `id` == nom de fichier, `subject ==
  <did>`, signature du lien vérifiée (racine : `#root` ; sous-mandat : clé du
  grantee parent), chaîne parente résoluble et valide au moment du dépôt.
- **POST `/gamma`** (une entrée) : parse strict (kind du registre §07.9.2,
  `prevs` seulement sur `merge`), `prev == tête stockée` (== `If-Head`),
  signature d'entrée valide — owner (`#content`, ou `#root` pour les kinds
  structurels §07.2) ou déléguée (clé feuille d'`authorized_via`, chaîne
  vérifiée comme A.2#9 à `entry.at`). Kinds à cible claire (`action`,
  `inference`, `grant`, `revoke`, `rotate`, `heartbeat`, `merge`) : la chaîne
  doit couvrir l'opération que l'entrée affiche (`act.x.<c>.<a>` pour
  `action`, autorité §06.4 pour `revoke`, …). Kinds à corps scellé
  (`section.*`, `ethos.read`) : chaîne valide + au moins un verbe d'écriture
  (resp. `log_reads`/`read.*`) sur la zone du subject — le placement réel est
  invérifiable ici par construction (§07.3) et reste au client. Accepté ⇒
  append au segment UTC du mois d'`entry.at` + avance transactionnelle de la
  tête (A.5).
- **`e/<zone>/blobs/<sid>.enc`, `hdr/*.json`, index, `e/public/**`, `x/**`** :
  contrôle de forme léger (JSON parsable là où c'est du JSON, tailles A.8) ;
  aucun contrôle de contenu — l'intégrité opposable arrive au publish
  (roots §02.10) et à la vérification cliente. Le ciphertext est opaque par
  design.
- **`changesets/<64hex>.json`, `evidence/<64hex>.json` (redline gate 5,
  2026-07-20)** : contrôle de forme léger (JSON parsable, tailles A.8)
  **+ adressage par contenu** : le nom de fichier doit égaler le digest
  K1-C recalculé sur les octets déposés — `C("aithos-core/v1/changeset"` |
  `"aithos-core/v1/evidence", JCS(objet))`, §02.6.3 — sinon
  `artifact_invalid` + `reason: "id_mismatch"` (registre A.7 inchangé).
  Aucune vérification sémantique du contenu : la cohérence
  changeset/evidence/manifest est l'affaire du verifier (K1-B), jamais du
  store — anti-abus, pas l'autorité.
- **Alias K1-C (redline gate 5, 2026-07-20)** (`public/sections/*.md`,
  `circle/blobs/*.json`, `indices/public.json`, `roots/public.json`,
  `vault/catalog-pins.json`) : même contrôle léger que leurs équivalents
  `e/**` (JSON parsable là où c'est du JSON ; le `.md` public et le porteur
  de blob restent opaques).

Tout PUT/append accepté déclenche le hook témoin (annexe C) selon le mode.

### A.5 CAS — les deux têtes chaudes

Table des têtes (DynamoDB), clé `(tenant, did)` : `{height,
manifest_chain_hash, gamma_head, gamma_segment}` ; écritures conditionnelles
(transaction avec le dépôt S3 de l'objet).

- Grammaire : `If-Head: sha256:<64 hex>` ou `If-Head: none` (« la ressource
  n'existe pas encore »).
- **`manifest.json`** : tête = `sha256:` + `chain_hash` du manifest courant
  (SHA-256 du JCS avec `signature.value=""` — la valeur même que le
  successeur épingle en `prev_hash`). Genèse : `If-Head: none` +
  `height == 1` + `prev_hash == ""`.
- **gamma** : tête = `sha256:` + SHA-256 du JCS de la dernière entrée — la
  valeur que la nouvelle entrée porte en `prev`. Log vide : `If-Head: none`
  + `prev == ""`.
- PUT `manifest.json` ou POST `/gamma` **sans** `If-Head` → `428` +
  `cas_required` (jamais d'écrasement silencieux).
- Mismatch → `409` + `{"error": "cas_mismatch", "head": "sha256:…"}`
  (+ `"height"` pour le manifest) : le client rebase/merge (§02.6) et
  republie. La réplique de segment (PUT, mode A) suit la même règle sur la
  tête du segment stocké.
- **Réponses d'accept (redline gate 4, 2026-07-20).** Un publish accepté
  répond `200` + `{"head": "sha256:…", "height": N}` — les valeurs mêmes
  que `/heads` servira ; un append gamma accepté répond `200` +
  `{"head": "sha256:…"}` ; un dépôt de `certs/<id>.json` accepté répond
  `204` sans corps. Jamais un écho d'artefact, jamais un chemin.
- **Grammaire `If-Head` fermée (redline gate 4, 2026-07-20).** Une valeur
  hors grammaire (`none` | `sha256:<64 hex minuscule>`) ne peut égaler
  aucune tête stockée : elle reçoit la réponse du mismatch — `409` + tête
  courante — jamais un troisième registre d'erreur.
- **Scan de révocation et segments (redline gate 4, 2026-07-20).** La
  révocation du A.2 #9 s'évalue sur le log pointé par
  `did_doc.revocations` **∪ tous les segments mensuels où un append a été
  accepté** — un `revoke` déposé via POST `/gamma` mord immédiatement,
  sans réécriture de pointeur. La liste des mois appendés est un détail
  de backend de la table des têtes (attribut d'implémentation à côté du
  tuple A.5), jamais exposée sur le wire.

### A.6 Cache et immuabilité (précise §3.4)

- `Cache-Control: public, max-age=31536000, immutable` : `certs/<id>.json`
  (I2), segments gamma des mois **révolus**, éditions passées archivées ;
  + `manifests/<h>.json`, `changesets/<hash>.json`, `evidence/<hash>.json`
  (adressés par hauteur/contenu, jamais réécrits — le write-once ⑧b de
  l'étape 6 rend la classe opposable ; redline gate 5, 2026-07-20).
- `Cache-Control: no-store` : `manifest.json`, `/heads`, segment gamma
  courant, `hdr/*.json`, index de zone ; + `indices/public.json`,
  `roots/public.json`, `vault/catalog-pins.json` (avancent à chaque
  publication ; redline gate 5, 2026-07-20).
- **Alias K1-C (redline gate 5, 2026-07-20)** : `public/sections/<sid>.md` :
  `Cache-Control: public, max-age=0, must-revalidate` + ETag fort (SHA-256
  des octets) — le sid est stable, le contenu peut être réédité.
  `circle/blobs/<sid>.json` : même classe que `e/<zone>/blobs/<sid>.enc`
  (`private, max-age=0, must-revalidate` + ETag fort).
- `e/<zone>/blobs/<sid>.enc` : `Cache-Control: private, max-age=0,
  must-revalidate` + ETag fort (SHA-256 des octets) — le chemin ne porte pas
  `key_version` (§02.3), une ré-encryption (rung 3) réécrit l'objet ; le
  client cache par `(chemin, key_version de l'index)` et revalide à l'ETag.
  L'immuable logique de §3.4 (« blob par `(sid, key_version)` ») vit côté
  client.
- CloudFront ne fronte que le public : `e/public/**`, `did.json`,
  `certs/**` si `certs_public`, et le feed témoin (annexe C) ; +
  `public/sections/**`, `indices/public.json`, `roots/public.json`
  (redline gate 5, 2026-07-20).

### A.7 Registre d'erreurs

Corps d'erreur : `{"error": "<code>", "at": "<now serveur>"}` +
`head`/`height` sur `cas_mismatch`, `reason` court sur `artifact_invalid`.
Jamais un chemin, un extrait de corps ou une enveloppe dans une réponse
d'erreur ni dans un log (A.8).

**Registre fermé des `reason` d'`artifact_invalid` (redline gate 5,
2026-07-20)** : `form`, `signature`, `chain`, `prev_hash_mismatch`,
`id_mismatch`, `subject_mismatch`, `entry_signature`, `prev_mismatch`,
`prefix_mismatch` (réplique de segment qui ne préserve pas le préfixe
stocké octet à octet — une réplique ne réécrit jamais l'histoire).
Rien d'autre, jamais de texte libre ; tout reason nouveau = redline.

| HTTP | `error` | Quand |
|---|---|---|
| 400 | `path_invalid` | chemin hors grammaire A.1 |
| 400 | `envelope_invalid` | forme, champ inconnu, host/method/path/body_b3 discordants |
| 400 | `artifact_invalid` | vérification A.4 échouée |
| 401 | `envelope_missing` | enveloppe absente sur route non anonyme |
| 401 | `clock_skew` | \|now − at\| > 300 s |
| 401 | `nonce_replayed` | `(key, nonce)` déjà vu |
| 401 | `signature_invalid` | signature d'enveloppe fausse |
| 403 | `chain_invalid` | chaîne malformée, expirée, non attenuée, subject ≠ did, key ≠ feuille |
| 403 | `chain_revoked` | un id de la chaîne est révoqué à `now_serveur` |
| 403 | `not_covered` | path-map : refus par défaut — un `403` propre, jamais une décision d'autorité |
| 403 | `did_not_bound` / `suspended` | control plane P7 |
| 404 | `unknown_tenant` | tenant inconnu (pas d'énumération de DIDs : un DID non lié répond `did_not_bound` seulement sous enveloppe valide) |
| 404 | `not_found` | objet absent **dans** un périmètre couvert |
| 409 | `cas_mismatch` | A.5 — la réponse porte la tête courante |
| 410 | `edition_gone` | `sync` depuis une édition purgée (§8 GC) |
| 413 | `payload_too_large` | limites A.8 |
| 426 | `version_unsupported` | version majeure inconnue |
| 428 | `cas_required` | `If-Head` absent où il est obligatoire |
| 429 | `rate_limited` | quotas tenant (§1 : bornage de tuyau) |

### A.8 Limites et discipline de logs

Anti-abus, configurables par tenant (défauts) : objet ≤ 32 MiB ; entrée gamma
≤ 64 KiB ; enveloppe ≤ 8 KiB ; `X-Aithos-Certs` ≤ 64 KiB ; batch ≤ 256
chemins / 32 MiB de réponse ; listing ≤ 1000 chemins/page.

Logs applicatifs (discipline `credentials.rs`, opposable) : champs **autorisés**
= `at`, `tenant`, `did`, classe de route (énum fermée : `read`, `list`,
`heads`, `batch`, `put_artifact`, `publish`, `gamma_append`, `gamma_replica`,
`sync`, `acme`), verbe, statut HTTP, code d'erreur, tailles, durée. Champs
**interdits** = le chemin complet, la query, tout corps, toute enveloppe, tout
en-tête de valeur. Rétention 30 j (§8).

---

## Annexe B (normative) — tunnel `aithos-tunnel: "1.0.0-draft.1"` — contrat C2

> Gravée 2026-07-16, lot P0. Consommée par P6 (relay) et G1 (client gateway).
> Vecteurs : `vectors/p3-tunnel-register.json`. Rappel A3 : le relay est un
> passthrough TCP/SNI strict — il ne termine jamais le TLS public, ne lit
> jamais un octet applicatif.

### B.1 Topologie

Entrant public : NLB TCP `:443` → routeur SNI (lecture du ClientHello **sans
terminaison**) → stream mux du tunnel du pod. Sortant pod :
une connexion TLS persistante vers `relay.aithos.fr:443` (cert serveur =
relay ; le pod authentifie par enregistrement signé, jamais par mTLS — zéro
secret nouveau, la clé de gateway existe déjà).

### B.2 Enregistrement

Après la poignée TLS (ALPN `aithos-tunnel/1`), le pod envoie **une ligne**
(JCS + `\n`, ≤ 4 KiB) :

```jsonc
{ "aithos-tunnel": "1.0.0-draft.1",
  "tenant": "acme",
  "hostname": "acme.mcp.aithos.fr",
  "gateway_pub": "z6Mk…",                  // clé Ed25519 de la gateway, multibase
  "at": "2026-07-16T12:00:00Z",
  "nonce": "<opaque, 16–64 car.>",
  "signature": { "alg": "ed25519", "value": "<hex>" } }
```

Signature : convention A.1 (JCS, `value=""`). Vérifications relay, dans
l'ordre, fail-closed : forme (champ inconnu ⇒ rejet) → skew ±300 s → nonce
jamais vu (fenêtre ≥ 600 s) → signature sous `gateway_pub` → mapping control
plane `gateway_pub ↔ tenant ↔ hostname` exact et non suspendu (P7). Réponse :
une ligne `{"aithos-tunnel": "1.0.0-draft.1", "ok": true}` puis passage en
mux ; ou `{"ok": false, "error": "<code>"}` puis fermeture. Codes (registre
A.7 restreint) : `envelope_invalid`, `clock_skew`, `nonce_replayed`,
`signature_invalid`, `mapping_mismatch`, `suspended`, `rate_limited`.

Un hostname = **un tunnel actif** : un enregistrement valide pour un hostname
déjà servi **remplace** l'ancien (le pod redémarré ne attend pas un timeout) ;
l'ancien mux reçoit GoAway et ferme. Anti-flap : ≥ 6 enregistrements/min sur
un hostname → `rate_limited`.

### B.3 Multiplexage

**yamux** sur la connexion TLS ; le pod tient le rôle **serveur** yamux
(accepte les streams), le relay le rôle client. Fenêtre initiale 256 KiB.

> **Keepalive — redline 2026-07-18 (arbitrage Mathieu, gate M2), remplace le
> « ping yamux toutes les 30 s » du draft initial :** « M2 = détection de
> déconnexion (EOF) + TCP keepalive ; PING actif applicatif (pod FIGÉ, TCP
> vivant mais muet) = draft.2 via le canal de contrôle riche que B.6
> réserve. » Concrètement en M2 : `SO_KEEPALIVE` (idle court, ~30 s /
> sondes 10 s ×3) posé **des deux côtés du socket tunnel** — relais et
> pod — détecte un pair mort sans FIN (crash, NAT expiré) et garde la
> traduction NAT du tunnel sortant vivante ; le mux reste
> `Config::default()` (aucun knob yamux, G1 ne se couple à aucun mux
> legacy). Le cas « pod figé » reste explicitement hors M2 (scénario
> `@wip @draft2` de `relay-passthrough.feature`).

Reconnexion pod : backoff exponentiel, base 1 s, ×2, plafond 60 s,
jitter ±20 %.

Un flux entrant accepté (B.4) = un stream yamux ouvert par le relay ; les
octets TCP y sont pipés **depuis le premier octet (ClientHello inclus)** —
aucun préambule, aucune trame de contrôle applicative dans le stream : le pod
re-lit le SNI lui-même et termine le TLS public (la clé TLS privée reste chez
le client, A3/§5). Half-close propagé dans les deux sens ; reset de stream =
RST de la connexion TCP correspondante.

### B.4 Routage SNI et bornage de tuyau

ClientHello lu en ≤ 10 s et ≤ 16 KiB, sinon fermeture sèche. SNI absent,
non-TLS, ou hostname sans tunnel actif → fermeture TCP silencieuse (pas de
bannière : rien à énumérer). Correspondance exacte, insensible à la casse,
sur le registre des tunnels.

Bornage (§1 : « borner le tuyau, jamais l'action » ; valeurs = config ops par
tenant, hors wire) : connexions/s, streams simultanés, octets/s.
**Suspension** (P7) : purge du registre + fermeture des tunnels + refus
d'enregistrement, propagée en < 60 s.

Le relay voit — liste exhaustive, opposable : SNI, IPs/ports, volumes,
timings, événements de tunnel. Logs sous la discipline A.8 (champs autorisés :
`at`, tenant, hostname, IP source, octets in/out, durée, événement) ; jamais
un octet applicatif — le gate P6 le **prouve** (aucun chemin de code ne
loggue le contenu d'un stream).

### B.5 ACME DNS-01 délégué

La zone est à nous, le cert et sa clé restent chez le client (A3). API
minimale, servie par le service store (`https://store.aithos.fr`), enveloppe
A.2 avec `key = gateway_pub` et `mandate: []` — **exception graved** : sur
les routes `/acme/*`, l'autorité n'est pas une chaîne de mandats mais le
mapping control plane du `gateway_pub` signataire (même modèle que B.2).

| Verbe | Route | Corps | Sémantique |
|---|---|---|---|
| PUT | `/acme/txt` | `{"hostname": "acme.mcp.aithos.fr", "value": "<digest ACME, ≤ 255 car.>"}` | pose `TXT _acme-challenge.<hostname>` (TTL 60 s) ; le hostname DOIT appartenir au tenant du `gateway_pub` |
| DELETE | `/acme/txt` | idem | retire l'enregistrement ; de toute façon purgé après 10 min |

Erreurs : registre A.7 + `mapping_mismatch`. Anti-abus : ≤ 10 PUT/h par
hostname.

### B.6 Ce que le contrat ne fixe pas

Le choix NLB/EIP, le nombre de nœuds relay, les valeurs de quotas : ops.
La version yamux du crate : implémentation (le protocole yamux est stable).
Un canal de contrôle riche (drain, métriques poussées) : `draft.2` si besoin —
le format d'enregistrement porte la version pour ça.

---

## Annexe C (normative) — checkpoint `aithos-witness: "1.0.0-draft.1"` — contrat C3

> Gravée 2026-07-16, lot P0. Consommée par P5 (témoin) et par tout vérificateur
> tiers (CLI, dashboard). Vecteurs : `vectors/p4-witness-checkpoint.json`.

### C.1 Le checkpoint

```jsonc
{ "aithos-witness": "1.0.0-draft.1",
  "did": "did:aithos:z6Mk…",
  "edition_height": 42,
  "manifest_hash": "sha256:…",     // = sha256: + chain_hash du manifest observé
                                   //   (JCS, signature.value="" — §02.6, la valeur
                                   //   même qu'un successeur épingle en prev_hash)
  "gamma_head": "sha256:…",        // copié du manifest observé
  "observed_at": "2026-07-16T12:00:00Z",
  "witness_key": "z6Mk…",          // Ed25519 multibase
  "signature": { "alg": "ed25519", "value": "<hex>" } }
```

Signature : convention A.1. `alg` : **seul `ed25519` est au registre en
`draft.1`**. Garde de la clé (P5, hors wire ; corrigé au gate P0, vérifié en
ligne le 2026-07-16) : **clé KMS native, vrai sign-only** — AWS KMS signe
EdDSA/Ed25519 depuis novembre 2025 : key spec `ECC_NIST_EDWARDS25519`,
algorithme `ED25519_SHA_512` (Ed25519 « pur » FIPS 186-5 §7.6 / RFC 8032,
signatures 64 octets interopérables avec tout vérificateur du stack),
`MessageType: RAW` (borne 4096 octets — nos JCS signés font ~400). La clé
naît dans KMS et n'en sort jamais ; chaque signature est IAM-gated et tracée
CloudTrail ; `GetPublicKey` exporte les 32 octets publics, encodés multibase
en `witness_key`. Piège documenté : ne jamais employer `ED25519_PH_SHA_512`
(mode préhashé, non interopérable avec une vérification Ed25519 pure).
Rotation annuelle : nouvelle clé publiée à côté de l'ancienne, les checkpoints
portent `witness_key` — un vérificateur accepte toute clé du registre publié
des clés témoin (`witness.aithos.fr/keys.json`, signé par la clé sortante).

Le témoin signe des **observations, jamais de l'autorité** (§4) : aucun
client ne dépend de lui pour agir ; il ne « valide » pas un manifest, il
atteste l'avoir vu.

### C.2 Déclencheurs et idempotence

Un checkpoint est émis : à chaque publish accepté (mode B), à chaque réplique
de manifest acceptée (mode A), et en heartbeat quotidien par DID (re-signature
de la tête courante, `observed_at` frais). Idempotence : re-voir le même
`(did, edition_height, manifest_hash)` dans la même journée UTC n'émet pas de
nouvelle ligne hors heartbeat. Plusieurs lignes de même height et même
`manifest_hash` = fraîcheur, jamais une faute.

### C.3 Publication

- **Feed par DID** : `https://witness.aithos.fr/<did>.jsonl`, append-only,
  public, une ligne = **exactement les octets JCS signés** du checkpoint
  (rejouables tel quel). `Cache-Control: public, max-age=60`.
- **Racine quotidienne agrégée** : `https://witness.aithos.fr/roots/<YYYY-MM-DD>.json` :

```jsonc
{ "aithos-witness-root": "1.0.0-draft.1",
  "date": "2026-07-16",
  "root": "<hex>",                 // mroot sur les checkpoints du jour UTC
  "n": 1234,
  "witness_key": "z6Mk…",
  "signature": { "alg": "ed25519", "value": "<hex>" } }
```

Conventions de hachage : celles de §02.10 avec **domaines dédiés** (aucun
splicing inter-protocoles possible) — `H_leaf(p) =
BLAKE3("aithos-witness/v1/mk-leaf" ‖ 0x00 ‖ p)`, `H_node` idem avec
`mk-node`, `mroot` = arbre binaire équilibré left-heavy (§02.10), feuilles =
`H_leaf(octets JCS de la ligne)` sur **toutes** les lignes émises du jour UTC
(tous DIDs), triées par ordre d'octets du JCS, dédupliquées à l'identique ;
`n` = leur compte. Un feed qui renie une ligne casse la racine du jour.

### C.4 Vérification et équivocation

Un checkpoint se vérifie seul : signature sous une `witness_key` du registre
publié. Règle d'équivocation (anti-fork, §10.6) : **deux checkpoints valides,
même `did`, même `edition_height`, `manifest_hash` différents** = preuve
portable d'équivocation (du store ou de l'owner du DID) — la paire signée
suffit, aucun accès au store requis. Usage fraîcheur : un vérificateur PEUT
exiger un checkpoint plus récent que sa tolérance (`freshness` §04.4) comme
borne anti-rollback de l'état de révocation (§06.5). Gossip 2-of-N avec des
témoins tiers : le format le permet déjà (plusieurs `witness_key` sur le même
DID), rien à changer côté clients.

### C.5 Réservé

Feed de notarisation d'artefacts logiciels (hashes de binaires gateway, §1
« résidu de confiance ») : format dédié en `draft.2` — le checkpoint C3 reste
lié à un DID, on ne le surcharge pas.

# HANDOFF — Piste G : la gateway devient un hub public (OAuth, sessions, démo BYO)

> **ÉTAT EXPRESS (2026-07-17, 13ᵉ session gw — G3 CLOS, l'AS OAuth `gateway_as`, gate réel passé).**
> Profil VM hybride confirmé (egress 000, unlink DENIED sur montage, pas de
> toolchain VM), protocole cloud+janitor §5 à la lettre, HEAD d'entrée
> `22a67c4`, baseline revalidée À L'IDENTIQUE avant tout travail. **Contrat
> d'abord** (`4eb1b39`, seul) : `gateway-oauth.feature` (33 scénarios @wip),
> les 7 décisions Mathieu gravées en tête de Feature. **Décisions
> (AskUserQuestion, 2026-07-17)** : (1) token→chaîne pré-G4 = la **chaîne
> agent du contexte** (précédent G6) via une couture INJECTABLE
> (`Runner::agent_authority_ceiling`) — G4/G5 y branchent le `not_after` du
> sous-mandat de session sans toucher l'AS ; (2) token = **JWT EdDSA fait
> main** (JWS compact ed25519, zéro dépendance — le lockfile est à la piste
> P) ; (3) **clé d'adapter** = secret gateway ordinaire, fichier **0600** né
> au 1er `run` depuis l'`EntropySource` injecté (défaut `as.key`), JAMAIS
> dans le keyholder, jamais un objet protocole ; (4) consentement = page
> **DEV** 1-clic Approve ; (5) stanza **`as:` opt-in** (absente =
> byte-identique), shape multi-context requise, `issuer` explicite (http
> loopback only, https ailleurs — règle des brokers Vault) ; (6) TTLs access
> **3600 s** / refresh **7 j**, plafonnés par le `not_after` de la chaîne ;
> (7) **DCR** ouverte aux clients publics PKCE + allowlist intégrée (callback
> `claude.ai` exact + `localhost`/`127.0.0.1:*` tout port, RFC 8252),
> extensible par `redirect_allowlist`. **G3 clos** (`9610fe1`) : découverte
> RFC 9728/8414, DCR RFC 7591, PKCE **S256**, `/authorize` + consentement
> DEV, `/token` audience-borné RFC 8707, refresh rotation one-shot + coupure
> de famille au rejeu, codes one-shot ; gate bearer sur `/mcp` (Origin 403 →
> bearer 401+`WWW-Authenticate` → corps → JSON-RPC) — le token n'accorde que
> l'ENTRÉE, la chaîne de mandats revérifie chaque acte (une **révocation
> devance tout token non expiré**) ; émission journalisée (I5,
> `act.x.gateway.oauth_issue`, nomme le client, zéro octet de token) ;
> **gate RÉEL** : vrai binaire `run` avec `as:` en loopback cloud, client
> OAuth générique scripté (20 checks verts) **obtient un token et appelle
> `tools/list`**, MCP Inspector CLI liste à travers l'endpoint OAuth-protégé
> avec le bearer (refusé sans), clé d'adapter 0600 absente des stores/logs.
> Reste (non simulé, dit au gate) : le flow OAuth **navigateur** complet
> (Inspector UI / Claude custom connector réel) — répétition avec Mathieu.
> Suite : gateway **82 unit / 4 CLI / 152 scénarios-790 steps / 6 e2e
> (e2e_demo_lea compris) / 7 owner / 5 équivalence** ; core+bundle+cli
> inchangés (**100** + **229/906**) ; clippy `-D warnings` + fmt clean.
> `keyholder.rs`/`credentials.rs` intouchés, zéro retouche core, démo Léa
> byte-identique. Prochain : **G4 (cérémonie)** puis **G5 (multi-principal)**
> — la couture `agent_authority_ceiling` les attend ; G7/G8.a-c-d
> parallélisables → `docs/HANDOFF-GATEWAY-G3-DONE-2026-07-17.md`.

> **ÉTAT EXPRESS (2026-07-16 nuit, 12ᵉ session gw — G2 + G6 CLOS, gates réels passés).**
> Profil VM hybride confirmé (egress 000, unlink DENIED sur montage, pas de
> toolchain VM), protocole cloud+janitor §5 à la lettre, HEAD d'entrée
> `6fdfe3c`, baseline revalidée À L'IDENTIQUE avant tout travail. **Contrats
> d'abord** (`3b451ae`, seuls) : `gateway-streamable.feature` (13 scénarios) +
> `gateway-ethos-read.feature` (17), sonde de parse détag/re-tag des deux
> côtés. Décisions Mathieu consignées (AskUserQuestion, 2026-07-16) : session
> id STATELESS (émis à l'initialize depuis l'EntropySource injecté, écho,
> jamais exigé — l'autorité reste à la chaîne, G5 apportera les chaînes de
> session), GET/DELETE → 405, batch refusé -32600 (aligné 2025-06-18), Origin
> validé fail-closed MAINTENANT (MUST anti DNS-rebinding) ; G6 = surface
> DÉRIVÉE des mandats, jamais un toggle — recalculée par appel (couvert ∩
> lignes ∩ contenu), public = frontière de lisibilité servie SANS grant à
> toute session connectée, self jamais par défaut (le grant explicite est
> gravé au contrat mais reste @wip : la résolution self déléguée est un lot
> core séparé, vectors-first), `ethos.context` = briefing + corps public +
> index scellé sans corps. **G2 clos** (`d17d77b`) : coquille transport axum —
> notification → 202 corps vide (le bug -32601 sur `notifications/initialized`
> est mort), id-less non-notification → 400 fail-closed (jamais d'acte
> silencieux), `ping` → `result:{}`, `Mcp-Session-Id` opaque émis/écho, 405
> GET/DELETE, batch → -32600, Origin non-local → 403 avant tout JSON-RPC ;
> gate RÉEL : MCP Inspector ET Claude Code se connectent, listent, appellent —
> zéro erreur de protocole des deux côtés, audit-export montre les 2 actes.
> **G6 clos** (`1350e20`) : outils natifs `ethos.read/list/context` servis par
> le hub (jamais relayés), surface dérivée du SCAN des certificats — toute
> chaîne valide vers la clé agent, quel que soit le geste émetteur (owner CLI,
> sous-mandat de délégué, G8.c demain) — grant à chaud allume, révocation à
> chaud éteint (refus nommant le mandat révoqué), chaque corps circle ouvert =
> une entrée `ethos.read` sous la chaîne qui a lu (lecture injournalisable =
> appel refusé, précédent C2), étagères `briefing/` EXCLUES des outils de
> données (leur surface dédiée reste briefing.read — chemin chaud démo Léa
> byte-identique), préfixe `ethos` réservé partout (RESERVED_PREFIXES 2→3,
> is_reserved_server, hub validate_server), gestes `owner-grant-ethos-read`
> (self refusé pédagogiquement, ligne circle à l'agent ET à l'auditeur — le
> précédent K) et `owner-add-section` (GAPS beat 2), surfaces harnais
> `owner_revoke_mandate_id` / `owner_issue_ethos_read_subchain` (pré-M3/G8.c) ;
> gate RÉEL : zones remplies par CLI, session sans `read.circle` AVEUGLE
> (liste sans ligne circle, refus nommant le périmètre), grant à chaud puis
> Claude Code lit la mémoire scellée et s'en sert (« 550 000 € »), lectures au
> gamma, zéro contenu dans les logs. Suite : gateway **63 unit / 4 CLI /
> 119 scénarios-627 steps / 6 e2e (e2e_demo_lea compris) / 7 owner /
> 5 équivalence** ; core+bundle+cli inchangés (**100** + **229/906**) ; clippy
> `-D warnings` + fmt clean. Restent `@wip` : 1 ethos-read (self-serves, lot
> core), 14 gateway-mandates (M3/M4/M6), 8 e-mandate-sections (M4). Prochain :
> G7 (surface de preuve) / G8.a-c-d (sessions dédiées) / G3 (OAuth, chemin
> critique) → `docs/HANDOFF-GATEWAY-G2-G6-DONE-2026-07-16.md`.

> **ÉTAT EXPRESS (2026-07-16 soir, 11ᵉ session gw — piste G lancée, G8.b CLOS).**
> Profil VM hybride confirmé (sondes egress 000 + unlink interdit sur montage),
> protocole cloud+janitor §5 à la lettre, HEAD d'entrée `67a6c34`, baseline
> revalidée À L'IDENTIQUE en cloud avant tout travail. **G8.b (= M5) clos,
> vectors-first** : vecteur **E+** committé seul (`4e59385` —
> `vectors/eplus-attenuation.json` + `gen-eplus.py`, 71 cas parent/enfant/verdict
> + une chaîne signée owner→agent→helper, génération Python indépendante, octets
> croisés contre les builders Rust) ; puis `constraints_attenuate` (`d87a5ed`) :
> validation typée des DEUX côtés de chaque lien, containment par famille
> fail-closed sur tout le vocabulaire §04.4, clés inconnues rejetées dans les
> deux sens (M0.c, pas de copy-through), câblé dans `verify_chain_revocable`
> (subsume windows+obligations) — la gateway passe par la même porte à chaque
> authorize/append, le verrou des sous-mandats de session (G3–G5) est fermé.
> **Décisions Mathieu consignées (AskUserQuestion, 2026-07-16)** : (1) drop
> toléré UNIQUEMENT pour les familles conjointes en sous-arbre à l'append
> (`max_actions`, `max_actions_per`, `rate_limit`, `max_children`, `budgets`,
> `heartbeat` — exigé par le contrat F vert), refusé partout ailleurs ;
> (2) `purpose`/`session_bind`/`attestation_key` héritent à identité stricte ;
> (3) tranche contrat committée seule avant l'impl. Les 26 scénarios de la
> matrice détaggés (`6fdfe3c`) : bundle **229 scénarios / 906 steps**, core+cli
> **100** (97+3 E+), gateway inchangée **90/481, 62 unit, 4 CLI, 6 e2e
> (e2e_demo_lea compris), 7 owner, 5 équivalence** ; clippy `-D warnings` + fmt
> clean (core, bundle, gateway). Un enfant qui élargit est refusé en nommant la
> famille élargie (refus pédagogique). Restent G8.a (id=, M4), G8.c (émission
> multi, M3), G8.d (composition) — contrats déjà committés. Prochain :
> **G2** (tolérance clients MCP réels) et **G6** (`ethos.read/list/context`),
> loopback, zéro AWS → `docs/HANDOFF-GATEWAY-G2-G6-2026-07-16.md` (brief de
> reprise : UNE session pour les deux lots — même crate, jamais deux sessions
> parallèles dessus).

> **Statut : PRÊT À LANCER — 2026-07-16.** Plan d'action exécutable côté
> `aithos-gateway` (+ retouches core encadrées, lot G8). Se lit avec
> [`INFRA-PROVIDER.md`](INFRA-PROVIDER.md) (contrats C1–C3) et en parallèle de
> [`HANDOFF-PROVIDER-AWS.md`](HANDOFF-PROVIDER-AWS.md). Cible produit : le
> scénario de [`GAPS-DEMO-E2E.md`](GAPS-DEMO-E2E.md) — une assistante commerciale
> branche son Claude Cowork sur `<org>.mcp.aithos.fr` avec son mandat et agit sur
> les connecteurs de l'entreprise, bornée, journalisée, prouvable.

## Contexte en 30 secondes

La gateway d'aujourd'hui est un sidecar pod-internal : HTTP clair sur loopback,
une identité runner, aucun OAuth (choix explicite — « auth du endpoint agent »
était hors v1 ; le chantier C1 `gateway_as` de STANDARDS-COMPAT était dormant
« tant qu'aucun consommateur externe n'est en face »). Le consommateur externe
arrive : l'endpoint devient public à travers le tunnel, l'auth devient
obligatoire, et la gateway doit servir **N sessions aux mandats différents**. La
moitié resource-server est déjà verte (hub, grants, bornes, drift,
log-before-relay) ; les primitives de la cérémonie existent dans le core
(`build_sub`, `verify_chain`, signatures détachées JCS, crate `aithos-wasm`).

## Interdits (opposables à chaque lot)

- **Le keyholder et le `CredentialBroker` ne bougent pas d'un octet.** Aucune
  graine ne sort, aucun secret upstream ne traverse la frontière agent.
- La clé de signature des tokens OAuth = **clé d'adapter**, secret gateway
  ordinaire, jamais un objet protocole (C1). Un token n'est jamais une autorité :
  la chaîne de mandats est revérifiée à chaque acte.
- Jamais de réécriture d'appel ; refus pédagogiques ; fail-closed partout ;
  aucun secret ni chemin de section dans les logs/erreurs.
- Le core reste pur ; toute retouche core (G8) suit le rituel vectors-first + BDD
  et passe par ses propres features.
- Rien de la piste P n'est bloquant sauf via les contrats : G1 dépend de C2
  (tunnel) ; tout le reste tourne en loopback/dev sans AWS.

## Lots

### G1 — Tunnel client + TLS *(M ; contrat C2)*
Stanza config `relay: { endpoint, tenant, hostname }` ; connexion sortante TLS
persistante multiplexée, enregistrement signé par la clé de gateway ; obtention et
renouvellement du cert ACME DNS-01 délégué ; listener TLS (reload à chaud) ; mode
sans relay conservé (exposition directe `mcp.acme.com` — la sortie reste libre).
**Gate : gateway derrière NAT jointe via `https://demo.mcp.aithos.fr/mcp` (avec
P6) ; en local, listener TLS direct vert.**

### G2 — Tolérance clients MCP réels *(S)* — ✅ CLOS (2026-07-16, `d17d77b`, gate Inspector + Claude Code passé)
Streamable HTTP face à de vrais hosts : `notifications/initialized` (ne jamais
répondre à une notification), `ping`, gestion de session (`Mcp-Session-Id`), GET
SSE optionnel, erreurs JSON-RPC propres sur `resources/*`/`prompts/*` (capacités
non annoncées). Test contre MCP Inspector + Claude Code.
**Gate : un client MCP réel se connecte, liste, appelle — zéro erreur de
protocole dans les logs des deux côtés.**

### G3 — L'AS OAuth (`gateway_as`) *(L)*
Metadata RFC 9728 (protected resource → AS) + RFC 8414 ; **DCR (RFC 7591) et
CIMD** (exigences Claude vérifiées 2026-07-16 : DCR/CIMD supportés, bearer
statique refusé, consentement toujours requis, callback
`https://claude.ai/api/mcp/auth_callback` + localhost pour les CLI) ;
`/authorize` + PKCE ; `/token` + refresh (durée de vie du refresh ≤ `not_after`
du sous-mandat de session — au-delà, on refait la cérémonie) ; tokens signés clé
d'adapter, audience = le hub (RFC 8707), jamais de passthrough ; validation sur
`/mcp` → résolution en chaîne de session.
**Gate : `gateway-oauth.feature` verte (découverte, DCR, PKCE, refresh, rejets) ;
un client OAuth générique obtient un token et appelle `tools/list`.**

### G4 — La cérémonie *(M)*
Page servie par la gateway (à travers le tunnel) : import du **pack d'invitation**
ou d'un couple {mandat, clé} ; wasm (`aithos-wasm` étendu) : vérification locale
du mandat, signature du challenge, **frappe du sous-mandat de session** vers
`gateway_pub` (TTL courte, scopes ⊆ mandat, `issue` non re-délégué) ; POST du
sous-mandat à l'AS qui lie token ↔ session. Flow CLI équivalent pour les devs.
Deux modes de livraison du mandat, décision GAPS §4 : **pack d'invitation**
(mandat + keypair générés côté owner, envoyés par mail — DÉMO uniquement, marqué
DEV, custody voyagée assumée) et **pubkey-first** (elle génère sa clé dans la
page, renvoie sa pubkey, l'owner frappe — le flux de prod, non-répudiation
pleine).
**Gate : depuis un navigateur vierge, pack → session active en < 2 min ; le
sous-mandat apparaît dans les certs ; sa révocation coupe la session.**

### G5 — Multi-principal *(L, le cœur)*
Une chaîne de mandats **par session** dans le hub : `tools/list` filtré par la
session (deux mandats différents ⇒ deux surfaces), `authorize` sur la chaîne de
session, actes gamma signés sous owner → personne → session (`authorized_via`
complet), comptage par la règle de sous-arbre (les budgets de la personne se
décomptent), refus pédagogiques par session, isolation stricte inter-sessions.
**Gate : 2 sessions simultanées aux mandats différents sur le même hub : surfaces,
comptages et refus distincts ; les entrées gamma nomment la bonne chaîne.**

### G6 — Outils Ethos natifs *(M — le trou découvert par la démo)* — ✅ CLOS (2026-07-16, `1350e20`, gate réel passé ; self-serves @wip → lot core résolution self déléguée)
Aujourd'hui les seuls outils natifs sont `journal.*` et `briefing.read` : **aucun
moyen MCP de lire les sections d'un Ethos**. Ajouter, mandat-gated par session :
`ethos.read` (section/dossier par zone couverte), `ethos.list` (arborescence du
périmètre couvert), et un `ethos.context` (pack briefing + sections d'accueil).
Physique : la gateway déscelle avec **ses propres lignes** (elle opère le
contexte) ; autorité : la **chaîne de session** doit couvrir `read.<zone>#…` ;
chaque lecture = entrée gamma `ethos.read` sous la chaîne de session (le kind
existe, étape F+). Préfixe `ethos` réservé dans la config comme `journal`/
`briefing`. Écritures déléguées (`ethos.write` sur périmètres couverts, pass L
côté core) : incluses si le scénario le demande, sinon lot suivant.
**Gate : features `gateway-ethos-read` vertes ; « Claude lit l'Ethos et s'en
sert » passe en conditions réelles ; une session sans `read.circle` ne voit ni ne
lit rien de circle.**

### G7 — Surface de preuve *(S/M)*
Endpoints HTTPS owner/auditeur sur le hostname du hub, **requêtes signées**
(mêmes enveloppes que le store, pas d'OAuth pour nos surfaces) : lecture gamma
scopée par le mandat d'auditeur, certs, état des contextes. Mini page statique
« preuve » (hébergée `app.aithos.fr`, lot trivial côté P/`cdn-public`) : charge
mandat + clé, appelle la surface, **vérifie en wasm** (chaîne, hash-chain,
signatures) et affiche le journal — refus compris.
**Gate : la beat finale de la démo (« on ouvre l'Ethos, on constate le gamma »)
passe dans un navigateur, vérification locale incluse.**

### G8 — Mandats P0 : parité owner/mandat *(M/L, core + gateway, rituel complet)*
Le vieux caillou de la version précédente, tracé dans
`MANDATES-PRODUCT-GAPS.md` : (a) sélecteur `id=` de section (condition des writes
propres sur `self`) ; (b) **atténuation complète de toutes les contraintes** dans
`verify_chain` (aujourd'hui fenêtres + obligations seulement — un sous-mandat ne
doit jamais élargir `max_actions`, `action_params`, budgets…) ; (c) émission de
**plusieurs mandats restreints** depuis un même Ethos, surface CLI/config
propre ; (d) composition borne-manifeste ∧ restriction-mandat (l'intersection
s'applique). Vectors-first, features `e-mandates` étendues.
**Gate : vecteurs promus + features vertes ; un owner émet 3 mandats disjoints
sur le même Ethos (zones ≠, connecteurs ≠, budgets ≠) et les 3 tournent en même
temps via G5.**

### G9 — Démo BYO générale *(S)*
Dérouler le scénario de `GAPS-DEMO-E2E.md` §1 en conditions réelles (Claude
Cowork réel, connecteurs réels ou mocks `demo_mcp`, relay dev) ; runbook jour J
(façon `DEMO-LEA.md`) ; répétition avec Mathieu.
**Gate : run complet sans intervention, deux fois de suite.**

## Ordonnancement

```
G1 ──► G2 ──► G3 ──► G4 ──► G5 ──► G9
              (G6, G7, G8 en parallèle dès le départ — sans dépendance à G1–G5,
               testables en loopback ; G5 consomme G8.c pour le gate multi-mandats)
```

Parallélisme réel : **G6/G7/G8 n'attendent rien** (loopback). G1 n'attend que P6
pour son gate final (le code se développe contre un relay factice). Le chemin
critique démo : G1→G5 + G6 + G7, avec P6/P7 en face.

## Conciliation (état final de la gateway)

| Zone | Avant | Après |
|---|---|---|
| `config.rs` | `listen`, contexts/servers/journal | + `relay:`, `tls:`, `as:` (issuer, callbacks autorisés), outils `ethos.*` réservés |
| Endpoint | `/mcp` (+ `/v1/chat/completions`) loopback HTTP | + TLS, + `/.well-known/*`, `/authorize`, `/token`, `/register`, surface preuve ; joignable via tunnel |
| Principals | 1 chaîne runner | N chaînes de session + chaîne runner |
| Outils natifs | `journal.*`, `briefing.read` | + `ethos.read/list/context` |
| Keyholder / Broker | — | **inchangés** |
| Docs | — | MàJ `HUB-MCP.md`, `DEPLOYMENT-CONTAINMENT.md` (topologie tunnel), variante BYO de `DEMO-LEA.md` |

## Définition de « fini » (piste entière)

Une personne externe, munie d'un mail d'invitation, branche son Claude Cowork sur
`<org>.mcp.aithos.fr`, agit dans les bornes, se fait refuser pédagogiquement hors
bornes, et l'entreprise montre la preuve complète dans un navigateur — sans
qu'Aithos ait vu un payload, un token ou une clé, et sans qu'un octet du
keyholder ou du broker ait changé.

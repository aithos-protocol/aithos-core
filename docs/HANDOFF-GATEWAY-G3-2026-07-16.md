# Handoff — Piste G, reprise : lot G3 (l'AS OAuth `gateway_as`)

**Date :** 2026-07-16 (préparé en 12ᵉ session gw, G2+G6 clos)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD d'entrée :** `22a67c4` (docs: G2+G6 session close). Une session piste P
est ACTIVE en parallèle sur `crates/aithos-provider` : si des commits se sont
posés au-dessus de `22a67c4`, vérifier qu'ils sont P (fichiers disjoints du
gateway) et continuer — précédent §6 de GATEWAY-HANDOFF. Ne JAMAIS toucher
`rust/Cargo.toml`, `rust/Cargo.lock`, `crates/aithos-provider/`,
`vectors/README.md` s'ils sont sales : c'est leur chantier.
**Références, dans l'ordre :** `docs/HANDOFF-GATEWAY-HUB.md` (état express 12ᵉ
en tête + le plan G1–G9, LE document faisant foi — G3 y est spécifié),
`docs/HANDOFF-GATEWAY-G2-G6-DONE-2026-07-16.md` (l'état frais : ce que G2 a
préparé au transport), **`docs/STANDARDS-COMPAT.md` chantier C1 (`gateway_as`)
INTÉGRALEMENT** — c'est LA définition du chantier, jamais encore lue par une
session gateway —, `docs/INFRA-PROVIDER.md` §5 (OAuth = projection du mandat ;
l'AS servi PAR la gateway, jamais chez Aithos — un AS chez le provider
pourrait fabriquer des sessions) et §8 (logistique Claude vérifiée :
DCR/CIMD supportés, bearer statique refusé, consentement toujours requis,
callback `https://claude.ai/api/mcp/auth_callback` + localhost pour les CLI),
`docs/GAPS-DEMO-E2E.md` beats 5b/5c, `docs/GATEWAY-HANDOFF.md` §5 **À LA
LETTRE** (protocole d'environnement), spec/04-mandates.md §4.7
(`session_bind`, `max_sessions` — les clés de session) et spec/05 (sous-
mandats), puis le code dans cet ordre : `proxy_mcp.rs` (la coquille transport
G2 — `handle_multi`, `MCP_SESSION_HEADER`, `origin_is_local` : l'AS ride le
même listener), `main.rs` (chemin `run`, le précédent `app.merge(router_llm)`
— l'AS sera un routeur mergé pareil), `config.rs` (la stanza `as:` atterrit
là ; `deny_unknown_fields`, formes exclusives), `core_bridge.rs`
(`agent_read_chains`/`walk_agent_cert_chains` : le précédent de résolution
chaîne — la résolution token→chaîne s'en inspire ; `authorize` ; l'autorité
INJECTABLE posée par G6), `keyholder.rs` (LECTURE SEULE — intouchable),
`tests/features/gateway-streamable.feature` + les steps wire de `cucumber.rs`
(serveur axum éphémère + reqwest — le pattern EXACT pour tester des endpoints
OAuth), `tests/e2e_hub.rs` (harnais binaire réel).
**Mission :** G3 seul, dans UNE session gateway (même crate — jamais deux
sessions parallèles sur `aithos-gateway` ; la session P sur `aithos-provider`
ne compte pas). Contrat Gherkin d'abord, committé seul (précédents
H0/M1/G2-G6), puis impl par tranches. G3 conditionne G4 (cérémonie) et G5
(multi-principal) — il pose l'AS, pas les sessions multiples.

---

## 0. Ce qui est fait (ne pas refaire)

- **G2 clos** (`d17d77b`) : coquille transport axum — notifications → 202
  vide, id-less → 400, `ping`, `Mcp-Session-Id` stateless (émis/écho, jamais
  exigé), 405 GET/DELETE, batch → -32600, **Origin fail-closed** (403 avant
  tout JSON-RPC). Gate réel : Inspector + Claude Code, zéro erreur de
  protocole. G3 hérite de ce socle : les endpoints AS s'ajoutent au MÊME
  listener, la validation token s'insère dans cette coquille.
- **G6 clos** (`1350e20`) : `ethos.read/list/context`, surface dérivée du
  scan des certificats (toute chaîne valide vers la clé agent), grant/
  révocation à chaud, lectures journalisées. **L'autorité est restée
  INJECTABLE exprès** : la bascule « chaîne de session » de G5 passera par
  les mêmes coutures. Gate réel : Claude Code lit la mémoire scellée.
- **G8.b (= M5) clos** (sessions antérieures) : `constraints_attenuate` câblé
  dans `verify_chain_revocable` — le verrou des sous-mandats de session est
  fermé. Rien à rebrancher : la gateway passe déjà par cette porte.
- Détails, décisions consignées et périmètres exacts : les en-têtes de
  `gateway-streamable.feature` et `gateway-ethos-read.feature`, et le DONE
  handoff.

### La baseline figée (2026-07-16 nuit, tout vert, à revalider À L'IDENTIQUE)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | **63 unit, 4 CLI, 119 scénarios / 627 steps, 6 e2e** (dont `e2e_demo_lea`), **7 owner, 5 équivalence** |
| aithos-core + bundle + cli | **100 tests** + Cucumber bundle **229 / 906** |
| Hygiène | clippy `-D warnings` + `cargo fmt --check` clean (core, bundle, gateway) |
| `@wip` restants | 1 ethos-read (self-serves, lot core), 14 gateway-mandates (M3/M4/M6), 8 e-mandate-sections (M4) |

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-core -p aithos-bundle -p aithos-cli
cargo clippy -p aithos-gateway -p aithos-core -p aithos-bundle --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway -p aithos-core -p aithos-bundle
```

## 1. Périmètre G3 — l'AS OAuth `gateway_as` *(L)*

Le consommateur externe arrive (Claude custom connector) : l'endpoint devient
authentifié. L'AS est SERVI PAR LA GATEWAY (jamais chez Aithos — INFRA §5 :
un AS provider pourrait fabriquer des sessions ; ici le token pointe une
autorité que seule la cérémonie frappe). Contenu, par le plan HUB :

- **Découverte** : RFC 9728 (protected resource metadata sur le hub → pointe
  l'AS) + RFC 8414 (`/.well-known/oauth-authorization-server`). Un `/mcp`
  non authentifié (quand `as:` est actif) répond 401 + `WWW-Authenticate`
  pointant la metadata — c'est ainsi que Claude découvre l'AS.
- **DCR (RFC 7591) + CIMD** : enregistrement dynamique des clients publics ;
  exigences Claude vérifiées 2026-07-16 (DCR/CIMD supportés, bearer statique
  refusé, consentement toujours requis, callback
  `https://claude.ai/api/mcp/auth_callback` + localhost pour les CLI).
- **`/authorize` + PKCE** (S256 obligatoire) ; **`/token` + refresh** — durée
  de vie du refresh ≤ `not_after` du sous-mandat de session ; au-delà, on
  refait la cérémonie (jamais un refresh qui survit à son autorité).
- **Tokens signés clé d'adapter** : un secret gateway ORDINAIRE, jamais un
  objet protocole (C1) ; audience = le hub (RFC 8707) ; jamais de
  passthrough. **Un token n'est jamais une autorité** : la chaîne de mandats
  est revérifiée à chaque acte — le token n'est qu'un pointeur de session.
- **Validation sur `/mcp`** : token → résolution en chaîne (v1 pré-G4 : voir
  point non tranché n°1) → le pipeline actuel (authorize, bounds,
  log-before-relay) INCHANGÉ derrière.
- **Architecture suggérée** (à confirmer en lisant C1) : nouveau module
  (ex. `gateway_as.rs`), routeur axum mergé sur le même listener (précédent
  `router_llm` dans `main.rs`), stanza config `as:` (issuer, callbacks
  autorisés, TTLs) validée fail-closed, `deny_unknown_fields`. Entropie :
  codes, tokens, ids UNIQUEMENT depuis l'`EntropySource` injecté (précédent
  `session_entropy` de G2). Aucun secret ni token dans logs/erreurs
  (discipline `credentials.rs`).
- Contrat : `tests/features/gateway-oauth.feature`, committé seul d'abord —
  découverte, DCR, PKCE heureux, refresh, ET les rejets (redirect_uri hors
  liste, code rejoué, PKCE faux, token expiré/forgé, audience fausse,
  refresh au-delà du `not_after`).

## 2. Points non tranchés → AskUserQuestion AU CONTRAT, jamais en silence

1. **Liaison token→chaîne pré-G4** : la cérémonie (G4) n'existe pas encore —
   à quoi le token se lie-t-il ? Options : la chaîne agent du contexte
   (comme G6 pré-G5) via un consentement DEV, ou un sous-mandat de session
   dev-stub frappé par un geste CLI owner (plus proche de la cible, exerce
   déjà `session_bind`). Dans les deux cas : GARDER LA RÉSOLUTION INJECTABLE
   (une fonction chaîne-en-paramètre) — G4/G5 la remplaceront sans toucher
   au reste.
2. **Format du token** : maison (JCS + ed25519 sous la clé d'adapter — zéro
   dépendance nouvelle, le token est opaque pour le client de toute façon)
   vs JWT standard (interop maximale, une dépendance de plus). Lire C1
   d'abord — il tranche peut-être déjà.
3. **Custody de la clé d'adapter** : `keyholder.rs` est INTOUCHABLE — la clé
   vit ailleurs (suggestion : fichier 0600 à côté de l'identité, généré par
   un geste owner ou au premier `run` avec `as:`). À trancher.
4. **Consentement pré-G4** : page minimale marquée DEV (auto-consent
   explicite) vs formulaire réel. Claude EXIGE un écran de consentement —
   le minimum honnête qui passe son flow.
5. **La stanza `as:` est OPT-IN** : absente = comportement actuel
   byte-identique (loopback ouvert, demo Léa intacte). À graver au contrat —
   c'est la condition de préservation du chemin chaud.
6. **TTLs** : access token court (minutes ?), refresh ≤ `not_after` du
   sous-mandat — valeurs par défaut à consigner.
7. **DCR** : registration ouverte aux clients publics PKCE ? Allowlist
   `redirect_uri` (callback Claude + `http://localhost:*` CLI) — forme
   exacte à consigner.

## 3. Rituel (inchangé, non négociable)

Contrat Gherkin AVANT le code, committé seul → impl par tranche → détag
progressif — suite complète verte à CHAQUE détag → e2e → docs. Un compteur
qui bouge sans détag = STOP. Cucumber gateway SÉQUENTIEL
(`max_concurrent_scenarios(1)`). STOP à chaque gate pour validation Mathieu ;
toute décision non tranchée = AskUserQuestion (mode absent si le pont
flappe : defaults @wip committés, zéro impl des points non tranchés ; le
pont a flappé deux fois en 12ᵉ — reposer UNE fois à la reconnexion, pas de
boucle).

**Interdits absolus** : `keyholder.rs` et `credentials.rs` ne bougent pas
d'un octet ; la clé de signature des tokens = clé d'adapter, secret gateway
ordinaire, JAMAIS un objet protocole ; un token ne remplace JAMAIS la
vérification de chaîne ; `as:` absent = comportement actuel byte-identique ;
jamais de réécriture d'appel ; refus pédagogiques ; fail-closed partout ;
aucun secret, token, code ou chemin de section dans logs/erreurs/panics ;
entropie injectée uniquement ; toute retouche core (normalement AUCUNE) =
rituel vectors-first + BDD ; pas de merge `main` sans gate humain ; le
chemin chaud de la démo Léa ne bouge pas (8 beats + `e2e_demo_lea` verts à
chaque commit).

## 4. Protocole d'environnement (12ᵉ session : hybride confirmé)

Sondes D'ABORD (egress + unlink SUR LE MONTAGE — pas /tmp) : en 12ᵉ, egress
000, unlink DENIED, pas de toolchain VM → **protocole cloud+janitor
GATEWAY-HANDOFF §5 à la lettre**. Ce qui a marché tel quel :

- Tar du working tree depuis le montage (`tar czf _transfer/….tgz
  --exclude='rust/target*' --exclude=.git --exclude='_*' --exclude=ui-mockup
  --exclude=.DS_Store --exclude=cargo-linux .`) ; `device_stage_files` ;
  build/test cloud (rustc 1.95.0 préinstallé, `CARGO_INCREMENTAL=0`, suite
  gateway ~2 min, bundle ~4 min). ATTENTION : le tar embarque l'état P sale
  (Cargo.toml + aithos-provider) — il compile ; ne pas s'en étonner, ne pas
  y toucher.
- Retours : `device_commit_files` fichier par fichier + **sha256 croisé dans
  les deux sens à chaque transfert** ; staging git sélectif, fichiers nommés
  un à un (c'est ce qui a protégé les fichiers P en 12ᵉ).
- Git VM : `mv .git/*.lock _gitjunk/` avant CHAQUE commande écrivante,
  JAMAIS `git status` (lectures : `git --no-optional-locks log/diff`).
  Warnings `tmp_obj`/`HEAD.lock` cosmétiques.
- **Le conteneur cloud a le réseau, npx ET le CLI `claude`** : les tests
  contre clients réels se font LÀ (12ᵉ : Inspector CLI + `claude mcp add` +
  `claude -p` depuis `/home/claude` — la config MCP est scopée par projet).
  Pour G3 : le « client OAuth générique » du gate = flow PKCE scripté
  (curl/python) contre le vrai binaire ; Inspector a aussi un flow OAuth.
  Un test Claude interactif complet (navigateur) attendra une répétition
  avec Mathieu — le dire au gate plutôt que le simuler.
- Scories assumées (ignorer, suppression impossible depuis la VM) :
  `_transfer/` (tars des sessions), `_gitjunk/`, `_to_delete/`.

En fin de session : suites complètes + clippy + fmt, synchro sha-croisée,
**état express en tête de `docs/HANDOFF-GATEWAY-HUB.md`** (+ bloc dans
GATEWAY-HANDOFF.md tracké, §6), et un handoff de reprise comme celui-ci
(untracked, HEAD d'entrée exact).

## 5. Gates

G3 : `gateway-oauth.feature` verte (découverte, DCR, PKCE, refresh, rejets) ;
**un client OAuth générique obtient un token et appelle `tools/list`** à
travers le vrai binaire en loopback ; `as:` absent laisse tout l'existant
byte-identique (la suite complète EST ce gate-là). La démo Léa reste
prioritaire et intacte. Après G3 : G4 (cérémonie) puis G5 (multi-principal)
— G7 et G8.a/c/d restent parallélisables en sessions dédiées.

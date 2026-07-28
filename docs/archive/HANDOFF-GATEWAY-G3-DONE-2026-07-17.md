# Handoff — Piste G : lot G3 CLOS (l'AS OAuth `gateway_as`, 2026-07-17)

> **ARCHIVE DE PREUVE.** Gate G3 clos ; les baselines et prochaines étapes sont
> historiques.

**Branche :** `feat/obligations` (jamais switché)
**HEAD d'entrée :** `22a67c4` → **chaîne de la session :** `4eb1b39`
(contrat @wip seul) → `9610fe1` (impl G3, 33 scénarios détaggés). Le
commit docs (état express HUB + ce handoff) suit si Mathieu l'autorise ;
sinon disque untracked, comme les précédents.
**Références :** `docs/HANDOFF-GATEWAY-HUB.md` (état express 13ᵉ en tête,
LE document faisant foi), `docs/HANDOFF-GATEWAY-G3-2026-07-16.md` (le
brief d'entrée), le contrat `tests/features/gateway-oauth.feature` (les 7
décisions gravées en tête de Feature).

---

## 1. Ce qui est fait — G3 (`gateway_as`), gate réel passé

L'authorization server OAuth 2.1 minimal (chantier C1 de STANDARDS-COMPAT),
**servi PAR la gateway sur le même listener que `/mcp`** (précédent coquille
G2), **opt-in par la stanza `as:`** — absente, la gateway est
byte-identique (loopback ouvert, démo Léa intacte : la suite complète EST
ce gate-là, elle reste verte).

- **`oauth.rs` (neuf, zéro dépendance)** : le lockfile est à la piste P, donc
  tout est maison — JWT EdDSA compact (JWS `header.payload.signature`,
  base64url sans padding), PKCE **S256** (challenge = base64url(sha256) via
  le `sha256_hex` du core, décodé), découverte **RFC 9728** (protected
  resource metadata) + **RFC 8414** (AS metadata), **DCR RFC 7591**
  (clients publics PKCE, `token_endpoint_auth_method: none`, jamais de
  secret émis), `/authorize` + page de consentement **DEV** (bouton
  Approve, marquée DEV), codes d'autorisation **one-shot**, refresh
  **rotation one-shot** avec **coupure de famille** au rejeu, **RFC 8707**
  (resource exigé à `/authorize` ET `/token`, `aud` dans le token).
- **Clé d'adapter** : secret gateway ORDINAIRE, fichier **0600** né au
  premier `run` avec `as:` actif depuis l'`EntropySource` injecté (défaut
  `as.key`, chemin configurable). **JAMAIS dans le keyholder, jamais un
  objet protocole** — vérifié : elle n'apparaît dans aucun store ni log.
- **Liaison token→chaîne INJECTABLE** (`Runner::agent_authority_ceiling`) :
  pré-G4, le `not_after` de la chaîne agent du contexte plafonne chaque
  token ; un refresh ne survit jamais à cette autorité (au-delà, on refait
  le flow). G4/G5 remplacent le plafond par le `not_after` du sous-mandat
  de session **par la même couture**, sans toucher à l'AS.
- **Gate bearer sur `/mcp`** : ordre `Origin(403) → bearer(401 +
  WWW-Authenticate pointant la resource metadata) → forme du corps →
  JSON-RPC`. Un token valide n'accorde que l'ENTRÉE ; la chaîne de mandats
  est revérifiée à chaque acte (une **révocation devance tout token non
  expiré** — prouvé au scénario ET au pipeline `record_act`).
- **Émission journalisée** (I5) : une entrée `act.x.gateway.oauth_issue`
  par frappe, nommant le `client_id` en clair, `args_hash` seul, **aucun
  octet de token/code/secret** dans les logs, erreurs ou gamma.
- **TTLs** (décidés 2026-07-17) : access **3600 s**, refresh **7 j**, les
  deux plafonnés par le `not_after` de la chaîne liée.
- **DCR allowlist** : intégrée (`https://claude.ai/api/mcp/auth_callback`
  exact + `http://localhost:*`/`127.0.0.1:*` tout port, RFC 8252),
  extensible par `redirect_allowlist:` ; tout le reste refusé
  pédagogiquement.

### Gate client OAuth générique (conteneur cloud, vrai binaire, loopback)

Vrai binaire `run` avec `as:` actif, ethos provisionné (1 contexte
`ventes` + journal). Script PKCE générique (python/requests) : **20 checks
verts** — découverte 9728/8414, DCR public, PKCE→consentement DEV→code,
échange audience-borné (`aud == …/mcp`, `expires_in=3600`), **`tools/list`
servi AVEC le token** (`crm.read` + tools natifs), forgé→401 invalid_token,
code rejoué→invalid_grant, refresh rotation + rejeu→coupure de famille.
**MCP Inspector CLI** (vrai client) liste les outils à travers l'endpoint
OAuth-protégé avec le bearer ; **refusé sans token** (« Failed to connect…
Error POSTing to endpoint »). Clé d'adapter 0600 confirmée, 2 entrées
`oauth.issue` au gamma du journal (nommant le client, zéro token).

**Reste pour une répétition avec Mathieu (non simulé, dit au gate)** : le
flow OAuth **navigateur** complet — Inspector UI ou Claude custom connector
réel avec le callback `claude.ai` — demande un vrai navigateur et un compte
Pro/Max ; à faire ensemble le jour de la démo (précédent G2, où Claude Code
avait servi de client réel côté CLI).

## 2. Baseline de sortie (tout vert, à revalider À L'IDENTIQUE en reprise)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | **82 unit** (63 + 5 config `as:` + 14 oauth-core), 4 CLI, **152 scénarios / 790 steps** (dont 33 G3), **6 e2e** (dont `e2e_demo_lea`), **7 owner**, **5 équivalence** |
| aithos-core + bundle + cli | **100 tests** + Cucumber bundle **229 / 906** (inchangés — zéro retouche core) |
| Hygiène | clippy `-D warnings` + `cargo fmt --check` clean (core, bundle, gateway) |

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-core -p aithos-bundle -p aithos-cli
cargo clippy -p aithos-gateway -p aithos-core -p aithos-bundle --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway -p aithos-core -p aithos-bundle
```

Restent `@wip` (inchangés) : 1 ethos-read (self-serves, lot core), 14
gateway-mandates (M3/M4/M6), 8 e-mandate-sections (M4).

## 3. Décisions gravées cette session (AskUserQuestion, 2026-07-17)

1. **Token→chaîne pré-G4** : la chaîne agent du contexte (précédent G6),
   résolution INJECTABLE (`agent_authority_ceiling`).
2. **Format token** : JWT EdDSA fait main (zéro dépendance, aligné C1).
3. **Custody clé d'adapter** : fichier 0600 né au 1er run (défaut `as.key`).
4. **Consentement** : page DEV 1-clic Approve.
5. **Stanza `as:`** : opt-in, shape multi-context requise, `issuer` explicite
   (http loopback only, https ailleurs).
6. **TTLs** : access 3600 s / refresh 7 j, plafonnés par `not_after`.
7. **DCR** : ouverte aux clients publics PKCE + allowlist intégrée
   (Claude callback + loopback tout port), extensible.

Toutes consignées en tête de `gateway-oauth.feature`.

## 4. Interdits tenus (rappel)

`keyholder.rs` et `credentials.rs` **intouchés** (zéro octet). La clé de
signature = clé d'adapter, secret gateway ordinaire, jamais un objet
protocole. Un token ne remplace jamais la vérification de chaîne. `as:`
absent = comportement actuel byte-identique. Préfixes réservés inchangés.
Aucune réécriture d'appel. Refus pédagogiques, fail-closed partout. Aucun
secret/token/code dans logs/erreurs/panics. Entropie injectée uniquement.
**Zéro retouche core.** Cucumber gateway séquentiel. Le chemin chaud de la
démo Léa ne bouge pas (8 beats + `e2e_demo_lea` verts). Pas de merge `main`
sans gate humain.

## 5. Protocole d'environnement (13ᵉ session : hybride confirmé)

Sondes refaites : **egress 000, unlink DENIED sur le montage** (débris
`_to_delete/probe-unlink-20260716-s13.txt` non créé — unlink refusé), pas
de toolchain VM → **cloud+janitor GATEWAY-HANDOFF §5 à la lettre**. Tar du
working tree → `_transfer/aithos-core-src-20260716-g3.tgz` (sha256
`8dad1625…`, croisé dans les deux sens), build/test cloud rustc 1.95.0
(`CARGO_INCREMENTAL=0`, gateway ~2 min à froid). Retours
`device_commit_files` fichier par fichier, **sha256 croisés un à un** (les
9 fichiers gateway confirmés identiques disque↔cloud). Staging git
sélectif, fichiers nommés un à un (P protégé). Janitor `mv .git/*.lock
_gitjunk/` avant chaque commande git écrivante, jamais de `git status`
(lectures `--no-optional-locks`), warnings `tmp_obj`/`HEAD.lock`
cosmétiques. Le **pont desktop a flappé plusieurs fois** (précédent
3ᵉ/9ᵉ/12ᵉ) — travail cloud continué, transferts passés entre deux flaps,
AskUserQuestion reposé UNE fois à la reconnexion. Gate réel : le conteneur
cloud a le réseau, `npx` (Inspector) et le CLI `claude`.

**Constat d'arbre (ne pas toucher, décision Mathieu)** : la piste P laisse
`rust/Cargo.toml`, `rust/Cargo.lock`, `vectors/README.md` sales sur le
disque (session AWS parallèle). Non touchés, disjoints du gateway — aucun
commit de cette session ne les a stagés. Le tar d'entrée embarque leur état
sale ; il compile, ne pas s'en étonner.

## 6. Prochains lots

- **G4 (la cérémonie)** — page servie par la gateway : import du pack
  d'invitation / pubkey-first, wasm (vérif mandat, frappe du sous-mandat de
  session vers `gateway_pub`), POST à l'AS qui lie token ↔ session.
  **G3 lui a posé la couture** : `agent_authority_ceiling` devient le
  `not_after` du sous-mandat de session, `oauth.rs` inchangé.
- **G5 (multi-principal)** — une chaîne de session par token ; le ceiling
  et le binding token→chaîne se spécialisent par session via la même
  couture injectable.
- **G7 (surface de preuve)**, **G8.a/c/d**, **lot core « résolution self
  déléguée »** — parallélisables, contrats déjà committés @wip.
- La démo Léa reste prioritaire et intacte.

## 7. Rituel de reprise (inchangé, non négociable)

Lire `docs/HANDOFF-GATEWAY-HUB.md` (état express) puis ce document ; sondes
egress+unlink AVANT tout ; baseline revalidée À L'IDENTIQUE avant la
première modif ; contrats committés seuls avant l'impl ; détag progressif,
suite complète verte à chaque détag ; UNE session par crate ; STOP à chaque
gate pour Mathieu ; keyholder/credentials intouchables ; fail-closed
partout ; aucun secret ni chemin de section dans logs/erreurs ; jamais de
réécriture d'appel ; cucumber gateway SÉQUENTIEL ; le chemin chaud de la
démo Léa ne bouge pas.

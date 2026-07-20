# Handoff — Piste G, reprise : lots G2 + G6 (tolérance MCP réels, lecture d'Ethos)

**Date :** 2026-07-16 (préparé en 11ᵉ session gw, G8.b clos)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD d'entrée :** `6fdfe3c` (test: detag the 26 attenuation-matrix scenarios)
**Références, dans l'ordre :** `docs/HANDOFF-GATEWAY-HUB.md` (état express en
tête + le plan G1–G9, LE document faisant foi), `docs/GAPS-DEMO-E2E.md`
(beats 5a et 6a, décisions §4), `docs/HUB-MCP.md` (§3 décision 4 — périmètre
`tools/*` v1 ; §5 préfixes réservés), `docs/INFRA-PROVIDER.md` §5 (les deux
flux à ne jamais confondre), `docs/GATEWAY-HANDOFF.md` §5 **À LA LETTRE**
(protocole d'environnement), spec/07-gamma.md §7.9 (`ethos.read`) et
spec/04-mandates.md §4.2–4.3 (verbes, zones, lignes), puis le code dans cet
ordre : `proxy_mcp.rs` (`process_multi`, le routeur — G2 vit là),
`core_bridge.rs` (chercher `briefing_read`, `journal_search`,
`read_hub_manifest` : les TROIS précédents exacts de G6 — descellement par
ligne, autorité par chaîne, `log_read_as_agent` par lecture), `config.rs`
(`RESERVED_PREFIXES`), `hub.rs` (`validate_server`), `main.rs` (chemin `run`),
`tests/features/*.feature` + `tests/cucumber.rs` gateway (style des steps),
`tests/e2e_hub.rs` (harnais réseau).
**Mission :** G2 puis G6, dans UNE seule session gateway — les deux lots
touchent les mêmes fichiers (`proxy_mcp.rs`, `config.rs`, cucumber gateway) :
**ne jamais lancer deux sessions parallèles sur ce crate**. Contrats Gherkin
d'abord, committés seuls (précédents H0/M1), puis impl par tranche.

---

## 0. Ce qui est fait (ne pas refaire)

- **G8.b (= M5) CLOS, vectors-first** — le verrou de sécurité des sous-mandats
  de session (dépendance dure : G8.b AVANT le gate de G5) est fermé :
  - `4e59385` : vecteur **E+** seul (`vectors/eplus-attenuation.json` +
    `gen-eplus.py`) — 71 cas parent/enfant/verdict + une chaîne signée
    owner→agent→helper, génération Python indépendante, octets croisés contre
    les builders Rust.
  - `d87a5ed` : `constraints_attenuate` dans `aithos-core/src/constraints.rs`,
    câblé dans `verify_chain_revocable` (subsume windows+obligations). La
    gateway passe par la même porte à chaque authorize/append — rien d'autre à
    brancher pour G3–G5.
  - `6fdfe3c` : les 26 scénarios de la matrice détaggés (steps + DSL dans le
    cucumber bundle).
- **Décisions Mathieu consignées (2026-07-16, AskUserQuestion)** : (1) drop
  d'une contrainte héritée toléré UNIQUEMENT pour les familles conjointes en
  sous-arbre à l'append (`max_actions`, `max_actions_per`, `rate_limit`,
  `max_children`, `budgets`, `heartbeat`), refusé partout ailleurs ; (2)
  `purpose`/`session_bind`/`attestation_key` à identité stricte ; (3) clés
  inconnues à un lien de délégation = refus dans les deux sens (M0.c). Un
  vecteur promu ne change JAMAIS — toute retouche = nouveau vecteur + redline.
- M0 tranché, M1/M2 clos (voir `docs/HANDOFF-MANDATES-M3-2026-07-16.md`).
  Restent dans G8 : a (`id=`, contrats M4 : 8 `@wip` e-mandate-sections),
  c (émission multi, contrats M3 : 12 `@wip` gateway-mandates),
  d (composition) — parallélisables, sessions dédiées.

### La baseline figée (2026-07-16 soir, tout vert, à revalider À L'IDENTIQUE)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | 62 unit, 4 CLI, **90 scénarios / 481 steps**, **6 e2e** (dont `e2e_demo_lea`), **7 owner**, **5 équivalence** |
| aithos-core + bundle + cli | **100 tests** (97 + 3 E+) + Cucumber bundle **229 scénarios / 906 steps** |
| Hygiène | clippy `-D warnings` + `cargo fmt --check` clean (core, bundle, gateway) |

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-core -p aithos-bundle -p aithos-cli
cargo clippy -p aithos-gateway -p aithos-core -p aithos-bundle --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway -p aithos-core -p aithos-bundle
```

## 1. Périmètre G2 — tolérance clients MCP réels *(S)*

Streamable HTTP face à de vrais hosts, dans `proxy_mcp.rs` (`process_multi` +
le routeur axum) :

- **Notifications : ne JAMAIS répondre.** Constat de session : aujourd'hui
  `process_multi` répond `-32601` à `notifications/initialized` — un client
  réel strict peut s'en offusquer, et JSON-RPC interdit toute réponse à une
  notification (pas d'`id`). Détection = absence d'`id` (et/ou préfixe
  `notifications/`) → HTTP 202 corps vide côté transport, zéro JSON-RPC.
- **`ping`** → `{"jsonrpc":"2.0","id":…,"result":{}}`.
- **`Mcp-Session-Id`** : émettre un id opaque à l'`initialize` (en-tête de
  réponse), le tolérer/echo sur les requêtes suivantes. ATTENTION entropie :
  la gateway n'a QUE l'`EntropySource` injecté — pas de rand sauvage. Le
  comportement exact (rejet d'un id inconnu ? stateless assumé ?) : vérifier
  la spec MCP 2025-03-26 et le comportement d'Inspector/Claude AVANT d'écrire
  le contrat, et consigner la décision dans la feature.
- **GET /mcp** (SSE optionnel) : réponse propre (405, ou flux si trivial) —
  décision au contrat après lecture de la spec ; on n'offre pas ce qu'on ne
  tient pas.
- **`resources/*` / `prompts/*`** : `-32601` propre (déjà le cas), capacités
  jamais annoncées dans `initialize` (déjà le cas — `{"tools": {}}`).
- **Test contre MCP Inspector** (`npx @modelcontextprotocol/inspector`) puis
  Claude Code : À FAIRE DANS LE CONTENEUR CLOUD (le seul endroit avec réseau
  + binaire), gateway en loopback. Le gate : un client réel se connecte,
  liste, appelle — zéro erreur de protocole dans les logs des deux côtés.
- Contrat : nouvelle feature gateway (ex. `gateway-streamable.feature`),
  committée seule d'abord.

## 2. Périmètre G6 — outils natifs `ethos.read` / `ethos.list` / `ethos.context` *(M)*

Le trou découvert par la démo (GAPS beat 6a) : aucun moyen MCP de lire les
sections d'un Ethos. Modèle EXACT à suivre : le triptyque briefing (lot K) —
`briefing_available`/`briefing_read` dans `core_bridge.rs`, dispatch dans
`proxy_mcp.rs`, descripteurs conditionnels, refus §3bis.8.

- **Physique** : la gateway desselle avec SES lignes ou celles livrées à la
  chaîne du contexte (précédents : `read_hub_manifest` = ligne gateway,
  `briefing_read` = chaîne agent + `read_section_as_agent`). **Autorité** : la
  chaîne couvrant `read.<zone>#…` — v1 pré-G5 c'est la chaîne AGENT du
  contexte (comme briefing) ; la bascule « chaîne de session » arrive avec G5
  (G6 doit juste garder l'autorité INJECTABLE — une chaîne en paramètre, pas
  un champ figé). À consigner explicitement dans le contrat.
- **Trace** : chaque lecture = une entrée gamma `ethos.read` sous la chaîne
  qui a lu (`log_read_as_agent` existe, kind F+ existant) ; une lecture
  injournalisable fait échouer TOUTE la lecture (précédent `journal_search`).
- `ethos.read` : section par zone/chemin couverts. `ethos.list` :
  l'arborescence du périmètre couvert SEULEMENT (index clairs public/circle
  via `zone_rows` ; `self` : structure scellée — ne lister que ce que la
  chaîne couvre ET que les lignes ouvrent, sinon rien). `ethos.context` :
  pack briefing + sections d'accueil (composition de l'existant).
- **Préfixe `ethos` réservé** partout : `RESERVED_PREFIXES` (config.rs, 2→3),
  `is_reserved_server`/`validate_server` (config.rs + hub.rs) — comme
  `journal`/`briefing`. Tests miroirs des tests de réservation existants.
- **Refus pédagogiques** : une session sans `read.circle` ne voit NI ne lit
  rien de circle (list vide de circle, read refusé nommant le périmètre
  manquant) ; `self` jamais servi par défaut (décision GAPS §4.2).
- Écritures déléguées (`ethos.write`) : SEULEMENT si le scénario le demande —
  sinon lot suivant. Ne pas les entamer.
- Contrat : `tests/features/gateway-ethos-read.feature`, committé seul.
- Gate : features vertes ; « Claude lit l'Ethos et s'en sert » en conditions
  réelles ; une session sans `read.circle` ne voit ni ne lit rien de circle.

## 3. Rituel (inchangé, non négociable)

Contrats Gherkin AVANT le code, committés seuls → impl par tranche → détag
progressif — suite complète verte à CHAQUE détag → e2e → docs. Un compteur
qui bouge sans détag = STOP. Cucumber gateway SÉQUENTIEL
(`max_concurrent_scenarios(1)` — ne pas retirer). STOP à chaque gate pour
validation Mathieu ; toute décision non tranchée par les docs = AskUserQuestion
(mode absent si le pont flappe : defaults @wip committés, zéro impl des points
non tranchés).

**Interdits absolus** : `keyholder.rs` et `credentials.rs` ne bougent pas d'un
octet ; jamais de réécriture d'appel ; refus pédagogiques ; fail-closed
partout ; aucun secret ni chemin de section dans logs/erreurs/panics ; un
token ne remplace jamais la vérification de chaîne ; toute retouche core =
rituel vectors-first + BDD dans ses propres features (G2/G6 n'en exigent
normalement AUCUNE) ; pas de merge `main` sans gate humain.

## 4. Protocole d'environnement (session 11 : profil hybride confirmé)

Sondes D'ABORD (egress + unlink SUR LE MONTAGE — pas /tmp) : en 11ᵉ session,
egress 000, unlink interdit, pas de cargo VM → **protocole cloud+janitor
GATEWAY-HANDOFF §5 à la lettre**. Ce qui a marché tel quel :

- Tar du WORKING TREE depuis le montage (`tar czf _transfer/….tgz
  --exclude='rust/target*' --exclude=.git --exclude='_*' --exclude=ui-mockup
  --exclude=.DS_Store --exclude=cargo-linux .`) — inclut les docs untracked,
  c'est voulu ; `device_stage_files` sur le tar ; build/test cloud
  (`CARGO_INCREMENTAL=0`, rustc 1.95 préinstallé, suite gateway ~2 min,
  bundle ~4 min).
- Retours : `device_commit_files` fichier par fichier + **sha256 croisé dans
  les deux sens à chaque transfert**.
- Git sur la VM : `mv .git/*.lock _gitjunk/` avant CHAQUE commande écrivante,
  JAMAIS `git status` (lectures : `git --no-optional-locks log/ls-files`).
  Warnings `tmp_obj`/`HEAD.lock` cosmétiques. Staging sélectif, fichiers
  nommés un à un.
- Scories assumées : `_transfer/aithos-core-src-20260716.tgz` (tar 11ᵉ),
  `_gitjunk/`, `_to_delete/` — suppression impossible depuis la VM, ignorer.
- Docs untracked (décision de commit à Mathieu) : HANDOFF-GATEWAY-HUB,
  GAPS-DEMO-E2E, INFRA-PROVIDER, HANDOFF-PROVIDER-AWS, MANDATES-PRODUCT-GAPS,
  HUB-MCP, les handoffs datés — dont celui-ci.

En fin de session : suites complètes + clippy + fmt, synchro sha-croisée,
**état express en tête de `docs/HANDOFF-GATEWAY-HUB.md`**, et un handoff de
reprise comme celui-ci (untracked, HEAD d'entrée exact).

## 5. Gates

G2 : un client MCP réel (Inspector puis Claude Code) se connecte, liste,
appelle — zéro erreur de protocole des deux côtés. G6 : features
`gateway-ethos-read` vertes ; lecture d'Ethos en conditions réelles ; une
session sans `read.circle` aveugle sur circle. La démo Léa reste prioritaire
et intacte : son chemin chaud ne doit pas bouger (les 8 beats + e2e_demo_lea
restent verts à chaque commit).

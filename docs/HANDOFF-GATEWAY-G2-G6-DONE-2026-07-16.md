# Handoff — Piste G : lots G2 + G6 CLOS (12ᵉ session gw, 2026-07-16 nuit)

> **ARCHIVE DE PREUVE.** Lots clos, conservés pour la traçabilité ; les prochains
> travaux indiqués ici ne sont plus courants.

**Branche :** `feat/obligations` (jamais switché)
**HEAD d'entrée :** `6fdfe3c` → **HEAD de sortie :** le commit docs qui suit
`1350e20` (chaîne de la session : `3b451ae` contrats seuls → `d17d77b` G2 →
`1350e20` G6 → docs).
**Références :** `docs/HANDOFF-GATEWAY-HUB.md` (état express 12ᵉ en tête, LE
document faisant foi), `docs/HANDOFF-GATEWAY-G2-G6-2026-07-16.md` (le brief
d'entrée de cette session), les deux contrats
`tests/features/gateway-streamable.feature` et
`tests/features/gateway-ethos-read.feature` (les décisions y sont gravées en
tête de Feature).

---

## 1. Ce qui est fait

### G2 — tolérance clients MCP réels (`d17d77b`)

- **Décisions Mathieu (AskUserQuestion, consignées dans la feature)** :
  (1) sessions STATELESS — `Mcp-Session-Id` opaque émis à l'`initialize`
  (hex de 16 octets de l'`EntropySource` injecté, jamais de rand sauvage),
  écho de l'id présenté, jamais exigé, jamais stocké ; l'autorité reste à la
  chaîne de mandats (les chaînes de session arrivent avec G5/OAuth).
  (2) GET et DELETE sur `/mcp` → 405 (pas de SSE offert, pas de terminaison
  client). (3) corps tableau (batch JSON-RPC) → UNE erreur -32600 propre
  (« batching is not supported ») — aligné sur la révision 2025-06-18 qui a
  retiré le batching. (4) Origin validé fail-closed MAINTENANT (MUST de spec,
  anti DNS-rebinding) : absent ou loopback passe, tout le reste → 403 avant
  tout traitement JSON-RPC.
- **Impl** (`proxy_mcp.rs`) : la coquille transport vit dans `handle_multi`
  (Origin → forme du corps → règle de notification → dispatch) ;
  `process_multi` reste le cœur transport-free et gagne le bras `ping` →
  `result:{}`. Une notification (pas d'`id`) → HTTP 202 corps vide, zéro
  JSON-RPC — le bug -32601 sur `notifications/initialized` est mort. Un
  message sans `id` qui n'est PAS une `notifications/*` → 400 fail-closed
  (un acte sans canal de réponse ne doit jamais s'exécuter en silence).
  `McpRouter` porte `session_entropy: Mutex<Box<dyn EntropySource + Send>>`
  (+1 champ sur les 11 sites de construction).
- **Steps** : wire-level (serveur axum éphémère + reqwest, le pattern maison
  vault) — le transport EST la chose testée.
- **Gate réel passé (conteneur cloud, loopback)** : MCP Inspector CLI
  (connexion, tools/list, tools/call) puis Claude Code (`claude mcp add` +
  appel réel) — zéro erreur de protocole des deux côtés, `audit-export`
  montre exactement les 2 actes (un par client), chacun couvert par le
  mandat agent.

### G6 — outils natifs ethos.read / ethos.list / ethos.context (`1350e20`)

- **Décisions Mathieu (AskUserQuestion + clarification, consignées dans la
  feature)** : la surface est DÉRIVÉE, jamais un toggle — recalculée à
  chaque `initialize`/`tools/list` depuis TOUTE chaîne valide vers la clé
  agent trouvée dans le store du contexte (owner CLI aujourd'hui, sous-mandat
  d'un délégué, surface G8.c demain), intersectée avec ce que les lignes
  ouvrent et ce que les zones contiennent. Une révocation retombe au
  prochain appel, à chaud. Zone par zone : **public** = frontière de
  lisibilité (§02.1) — du contenu public informe TOUTE session connectée
  (sa connexion présuppose un mandat) et se lit sans clé, zéro coût gamma ;
  **circle** = seulement sous une chaîne couvrant `read.circle` dont les
  lignes ouvrent, chaque corps ouvert = une entrée `ethos.read` sous la
  chaîne qui a lu, une lecture injournalisable fait échouer l'appel
  (précédent C2) ; **self** = jamais par défaut (GAPS §4.2), le grant
  explicite est LE principe gravé mais reste `@wip` (voir §3).
- **Impl** (`core_bridge.rs`) : `walk_agent_cert_chains` (marche brute des
  certs, reconstruction par `parent`) → `agent_read_chains` (filtre
  `verify_chain_revocable`) ; `zone_all_rows` (index clair résolu, étagères
  `briefing/` EXCLUES — voir plus bas) ; `covered_circle_rows` (`verify_op`
  par ligne — les chaînes dir=/tag= scopées marchent par construction) ;
  `ethos_surface`/`ethos_list`/`ethos_read_section`/`ethos_context_pack` sur
  le Bridge, leurs homologues multi-contextes sur le Runner. Le refus d'une
  lecture couvrable par une chaîne RÉVOQUÉE nomme le mandat révoqué
  (`revoked_covering_read`, sonde froide sur le chemin du refus seulement).
  Dispatch dans `proxy_mcp.rs` AVANT `resolve` (précédent journal/briefing),
  refus §3bis.8 : journal toujours + contexte quand l'appel le nomme (seule
  surface native au contexte identifiable). Descripteurs recalculés par
  appel, les descriptions NOMMENT les zones servies (« Readable now —
  ventes: public, circle ») ; l'`initialize` compose briefing + phrase ethos
  sans casser les assertions K (substring).
- **La règle qui a préservé le chemin chaud** : les étagères `briefing/`
  sont exclues de `zone_all_rows` — les directives gardent leur surface
  dédiée (`briefing.read`, lot K) et le monde démo Léa (dont le seul contenu
  circle est sa directive) reste MUET côté outils de données : son assertion
  de liste EXACTE et ses 8 beats sont byte-identiques.
- **Gestes owner** : `owner-grant-ethos-read --zones public,circle` (mint +
  ligne circle à l'agent ET à l'auditeur — l'implication K assumée ; `self`
  refusé en nommant le lot core manquant ; PAS de champ d'état — le scan
  découvre) ; `owner-add-section --zone --path --text` (GAPS beat 2, remplir
  les zones par le vrai binaire) ; côté harnais : `owner_revoke_mandate_id`
  (une entrée `revoke`, subsumée par M3 plus tard) et
  `owner_issue_ethos_read_subchain` (owner → délégué `read.circle` +
  `issue#depth=1` → sous-mandat vers l'agent via `Mandate::build_sub` — le
  VRAI chemin de sous-délégation, exercé par le scénario délégué).
- **Réservation** : `RESERVED_PREFIXES` 2→3 (`ethos`), `is_reserved_server`,
  hub `validate_server` — tests miroirs des réservations `briefing`.
- **Gate réel passé (conteneur cloud, vrai binaire)** : zones remplies par
  CLI ; SANS grant : surface « ventes: public », `ethos.list` sans ligne
  circle, `ethos.read` circle refusé en nommant `read.circle` ; grant À
  CHAUD (gateway en marche) : surface « public, circle » au tools/list
  suivant, lecture scellée servie ; **Claude Code lit la mémoire circle et
  s'en sert** (réponse « 550 000 € », information n'existant que dans la
  section scellée) ; entrées `ethos.read` au gamma ; zéro contenu métier
  dans les logs gateway.

### La baseline de sortie (tout vert, à revalider À L'IDENTIQUE en reprise)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | **63 unit** (62+1 miroir ethos), 4 CLI, **119 scénarios / 627 steps**, **6 e2e** (dont `e2e_demo_lea`), **7 owner**, **5 équivalence** |
| aithos-core + bundle + cli | **100 tests** + Cucumber bundle **229 / 906** (inchangés — zéro retouche core) |
| Hygiène | clippy `-D warnings` + `cargo fmt --check` clean (core, bundle, gateway) |

Restent `@wip` : **1** gateway-ethos-read (self-serves), **14**
gateway-mandates (M3/M4/M6), **8** e-mandate-sections (M4).

## 2. Décisions gravées cette session (rappel une-ligne)

1. G2 : session id stateless émis+écho jamais exigé ; 405 GET/DELETE ;
   batch → -32600 ; Origin fail-closed maintenant. (Toutes dans l'en-tête de
   `gateway-streamable.feature`.)
2. G6 : surface auto-dérivée des mandats — public sans grant dès qu'il a du
   contenu, self sur grant explicite seulement, `owner-grant-ethos-read`
   n'est qu'une voie d'émission v1 (jamais un toggle) ; pack = briefing +
   corps public + index scellé. (Toutes dans l'en-tête de
   `gateway-ethos-read.feature`.)

## 3. Le trou honnête laissé ouvert (et où il est consigné)

**Résolution self déléguée côté bundle.** `read_section_as_agent` passe par
`resolve_clear` (index clair) ; la structure de `self` est scellée et
`self_resolve` est owner-only — un mandat `read.self` explicite ne peut donc
pas être SERVI aujourd'hui. Le principe (« choix, pas limite », GAPS §4.2)
est gravé dans le contrat : scénario « An explicitly granted self read
serves and journalizes like circle » `@wip`, et le geste v1 refuse
`--zones self` en nommant exactement ce manque. Le lot core qui l'ouvrira
suit le rituel complet vectors-first + BDD (ce n'était PAS le droit de cette
session : zéro retouche core, promesse tenue).

## 4. Protocole d'environnement (12ᵉ session : hybride confirmé)

Sondes d'abord : egress 000, unlink DENIED sur le montage (débris
`_to_delete/probe-unlink-20260716-s12.txt`), pas de toolchain VM →
cloud+janitor GATEWAY-HANDOFF §5 à la lettre. Tar
`_transfer/aithos-core-src-20260716-g2g6.tgz` (sha256 `75531ad7…`), build
cloud rustc 1.95.0, retours `device_commit_files` fichier par fichier,
sha256 croisés à CHAQUE transfert dans les deux sens, janitor des locks
avant chaque commande git écrivante, jamais de `git status`, warnings
tmp_obj cosmétiques. Le pont desktop a flappé deux fois (AskUserQuestion
coupés en vol) — précédent 3ᵉ/9ᵉ : travail cloud continué, question reposée
UNE fois à la reconnexion, mode absent sinon. Gates réels : le conteneur
cloud a le réseau (npx Inspector, `claude` CLI présents) — c'est LE bon
endroit pour les tests contre clients réels.

**Constat d'arbre (ne pas toucher, décision Mathieu)** : une session piste P
était ACTIVE en parallèle sur le même working tree pendant cette session
(lot P1 : `rust/crates/aithos-provider/` créé, `rust/Cargo.toml` +
`rust/Cargo.lock` modifiés à 13:52 UTC — après mon tar d'entrée —, plus la
retouche `vectors/README.md` des annexes P0). Crates disjoints, précédent §6
assumé : P∥G est le plan, mes commits n'ont jamais stagé leurs fichiers
(staging sélectif), aucun commit P ne s'est intercalé dans la chaîne
`3b451ae → d17d77b → 1350e20 → 22a67c4`. La session P committera les siens ;
au merge global, fichiers disjoints, rien ne se conflicte. Scories
habituelles intactes : `_transfer/`, `_gitjunk/`, `_to_delete/`.

## 5. Prochains lots (rien d'entamé ici)

- **G3 (OAuth `gateway_as`)** — le chemin critique démo ; DCR/CIMD, PKCE,
  refresh ≤ `not_after` du sous-mandat de session. G2 lui a préparé le
  transport (session id, Origin, refus propres).
- **G7 (surface de preuve)** — S/M, loopback, parallélisable.
- **G8.a / G8.c / G8.d** — sessions dédiées (contrats déjà committés @wip) ;
  G8.c donnera la voie d'émission produit que le scan G6 ramassera sans une
  ligne de code.
- **Lot core « résolution self déléguée »** — vectors-first, débloque le
  scénario self-serves @wip et le `--zones self` du geste.
- La démo Léa reste prioritaire et intacte ; le beat de révocation live
  (GAPS §4.3) a désormais son précédent technique (drop à chaud prouvé par
  scénario ET en réel).

## 6. Rituel de reprise (inchangé, non négociable)

Lire `docs/HANDOFF-GATEWAY-HUB.md` (état express en tête) puis ce document ;
sondes egress+unlink AVANT tout ; baseline revalidée À L'IDENTIQUE avant la
première modif ; contrats committés seuls avant l'impl ; détag progressif,
suite complète verte à chaque détag ; UNE session par crate ; STOP à chaque
gate pour Mathieu ; keyholder/credentials intouchables ; fail-closed
partout ; aucun secret ni chemin de section dans logs/erreurs ; jamais de
réécriture d'appel ; cucumber gateway SÉQUENTIEL ; le chemin chaud de la
démo Léa ne bouge pas.

# Handoff — démo Léa : lots K et D CLOS, gate répétition générale

**Date :** 2026-07-15 nuit (9ᵉ session gw, fin — profil cloud+janitor)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD de sortie :** le commit docs qui suit `0db670e`
**Références, dans l'ordre :** `docs/DEMO-LEA-SCENARIO.md` (LE document
de référence, validé — rien ne se code hors de lui),
`docs/GATEWAY-HANDOFF.md` (état express 9ᵉ session + protocole
d'environnement §5), les quatre contrats
`tests/features/gateway-{grants,bounds,briefing,demo-lea}.feature`
(**zéro `@wip`**), `docs/DEMO-LEA.md` (le runbook jour J), puis ce
document.
**Prochaine étape :** la **répétition générale avec Mathieu en
conditions réelles** (gate explicite, DEMO-LEA-SCENARIO §6.3). Rien ne
se code avant ce gate hors ajustements qu'il révèle.

## 1. État exact à la reprise

### Fait et vert (cette session, commits Mac canoniques)

| **Commit** | **Contenu** |
| :-: | :-: |
| `b2f5b69` | **Lot K** : briefing conditionnel servi et journalisé (8 scénarios détaggés) |
| `0db670e` | **Lot D** : les 8 beats détaggés + e2e réseau `tests/e2e_demo_lea.rs` (vrai binaire) |
| (docs) | Runbook `DEMO-LEA.md`, état express + §6 GATEWAY-HANDOFF, ce handoff |

**Suite au vert (vérifiée avant ce handoff)** : 62 unit, 4 CLI,
**88 scénarios / 473 steps** Cucumber (gateway-briefing 8/8 et
gateway-demo-lea 8/8 détaggés), **6 e2e réseau** (dont
`e2e_demo_lea`), 5 owner-side, clippy `-D warnings` et
`cargo fmt --check -p aithos-gateway` clean.

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

Cucumber reste **séquentiel** (`max_concurrent_scenarios(Some(1))`) —
ne pas retirer (starvation tokio sous mondes à sockets, cf. 8ᵉ session).

## 2. Les coutures tranchées cette session (à relire par Mathieu)

1. **Pen briefing = geste owner séparé.** `owner-grant-briefing` (CLI +
   `owner_grant_briefing`) : UN mandat de lecture dédié
   (`Verb::Read` sur `public:briefing/` et `circle:briefing/`) vers la
   pubkey agent + la **ligne circle** (§04.3). Séparé exprès : un pen
   par usage, révocable seul, il **survit au re-enrollment** (le
   re-enroll remplace les mandats OUTILS, jamais le canal de consignes ;
   `owner_reenroll_server` re-patch `state.briefing_mandate`). Exige un
   contexte déjà équipé (state présent), fail-closed sinon.
2. **La ligne circle va aussi à l'AUDITEUR.** Une requête gamma ne sert
   que les entrées scellées que le requérant peut physiquement ouvrir
   (`log_query_as_agent` : hint map sur ses lignes). L'auditeur mandaté
   sur les lectures reçoit donc la ligne `circle:briefing/` — assumé et
   documenté : l'auditeur du contexte peut lire les consignes dont il
   audite les lectures.
3. **Public = frontière de lisibilité, pas de trace de lecture.** La
   zone public n'a ni clé de zone ni header racine (§02.1) : aucune
   ligne physique possible, donc pas d'entrée `ethos.read` scellable
   pour une lecture publique. v1 : les lectures CIRCLE sont
   journalisées (le contrat et la démo vivent en circle) ; une consigne
   public est servie claire, hash-pinnée, sans entrée de lecture —
   documenté dans `Bridge::briefing_read`.
4. **Une section par zone** (`briefing/directives`), création au
   premier `owner-set-briefing`, rewrite ensuite — **circle-only en
   rewrite** (le core est « circle only this pass ») ; public et self
   sont write-once en v1. `--zone self` accepté (notes owner) et
   structurellement hors de portée du runtime.
5. **Surface conditionnelle recalculée PAR APPEL** (`initialize` et
   `tools/list` sondent `briefing_available()` — index clair seulement,
   zéro entrée) : le beat 7 est exact sans redémarrage ; tout état
   d'erreur se lit « rien à dire » (surface muette), la lecture réelle
   échoue fort.
6. **Auditeur de contexte élargi** : `equip` mint désormais
   `read.gamma#kind=action` **et** `read.gamma#kind=ethos.read` (deux
   entrées ; chaque requête nomme UN kind, plus large = refusé —
   vérifié sur les scénarios/e2e existants, kind=grant et unscoped
   toujours refusés). Le mono `onboard` reste action-only
   (gateway-audit contract). Beat 8 = deux exports scopés
   (`--kind action` puis `--kind ethos.read`).
7. **Refus `bound_violated` porteurs du détail.**
   `Runner::record_bound_refusal` → payload clair
   `{tool, reason, detail}` où detail = le message pédagogique (champ,
   valeurs fautives, règle approuvée — la politique SCELLÉE de l'owner,
   structurellement sans secret). Les AUTRES refus gardent le code nu
   (leurs messages ne sont pas garantis leak-free). Le beat 8 rejoue la
   leçon exacte.
8. **Enrollment batch.** `owner_enroll_servers` (et
   `owner-enroll-server --proposal` répétable) : N manifests scellés
   chacun sous son `/x/<server>`, **UN mandat agent couvrant l'union
   des outils grantés** (« un seul mandat agent », scénario §3),
   validation all-or-nothing (doublon de serveur, déjà enrollé, manifest
   invalide → rien n'est pinné). CLI : approvals ventilées par nom
   d'outil découvert, nom ambigu entre serveurs → refus ; `--replace`
   reste mono-serveur. Le chemin mono-manifest délègue au batch
   (comportement et entropie identiques — owner_surface inchangé).

## 3. Le gate, et comment le préparer

**Répétition générale avec Mathieu en conditions réelles** — dérouler
`docs/DEMO-LEA.md` de bout en bout. Avant de promettre un connecteur
réel, la checklist §0 du runbook :

- **Notion** : self-hosted officiel `@notionhq/notion-mcp-server
  --transport http --auth-token <bearer>` = prêt pour le coffre (le
  bearer dans Vault, le `NOTION_TOKEN` dans l'env du process serveur).
  L'endpoint hébergé `mcp.notion.com` est OAuth-PKCE-only : hors v1.
- **Gmail/Calendar** : les MCP distants OFFICIELS Google (2026,
  Developer Preview, `gmailmcp.googleapis.com/mcp/v1` /
  `calendarmcp.googleapis.com/mcp/v1`, HTTP+OAuth) — access token court
  déposé au coffre pour la fenêtre de démo (l'expiration devient
  l'argument rotation) ; sinon wrapper HTTP loopback communautaire
  single-user. Le wrapper stdio générique reste Phase D.
- **Trois vérifs par connecteur réel** (en répétition, jamais le
  jour J) : discovery stateless OK (pas d'`initialize`/`Mcp-Session-Id`
  exigé), champs bornables à la RACINE du schéma découvert, bounds et
  config posés sur les noms RÉELS.
- **TLS** : endpoints `https://` réels → activer la feature
  `rustls-tls` de reqwest (une ligne de Cargo) — le chemin mock est
  100 % loopback sans TLS.

## 4. Protocole d'environnement (inchangé, à la lettre)

GATEWAY-HANDOFF §5, profil cloud+janitor : `git archive HEAD` sur la VM
→ tar dans `_transfer/` → build/test dans le conteneur cloud
(`CARGO_INCREMENTAL=0`, target dédié) → retours **fichier par fichier**
via device_commit_files avec sha256 croisés → commits janitorisés sur le
Mac (`mv` des `.git/*.lock` vers `_gitjunk/` avant chaque commande git
écrivante, jamais de `git status` intercalé ; warnings tmp_obj
cosmétiques). Commits par tranche. Le pont desktop peut flapper (3ᵉ et
9ᵉ sessions) : committer tranche par tranche, continuer le travail
cloud pendant les coupures. Scories intactes : `_gitjunk/`,
`_to_delete/`, `_transfer/` (+ tars), `docs/EXPLORATION-DESKTOP-GATEWAY.md`,
`docs/HANDOFF-GATEWAY-VAULT-FINALIZATION-2026-07-15.md` untracked.

## 5. Gates et limites

- Ne pas merger `main`, ne pas déployer, aucune donnée/token réels dans
  le repo. Les valeurs de démo sont générées.
- **Après le gate seulement** : V4 LLM / writes réels côté Ethos /
  `resources/*` / AppRole / TLS / wrapper stdio — la liste longue est au
  §5 de `HANDOFF-GATEWAY-VAULT-DONE-2026-07-15.md` ; s'y ajoutent les
  hors-v1 du scénario §7 (regex/suffixes, règles croisées, champs
  imbriqués, second agent).
- En fin de session : suite complète + clippy + fmt, synchro
  sha-croisée, paragraphe §6 GATEWAY-HANDOFF + état express, et un
  handoff comme celui-ci.

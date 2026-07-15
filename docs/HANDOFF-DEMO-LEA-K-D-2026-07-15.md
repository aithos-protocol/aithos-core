# Handoff — démo Léa : lots K (briefing) et D (répétition générale)

**Date :** 2026-07-15 (8ᵉ session gw, fin — profil cloud+janitor)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD de sortie :** le commit docs qui suit `56d2a14`
**Références, dans l'ordre :** `docs/DEMO-LEA-SCENARIO.md` (LE document de
référence, validé par Mathieu — rien ne se code hors de lui),
`docs/GATEWAY-HANDOFF.md` (état express + protocole d'environnement §5),
les quatre contrats `tests/features/gateway-{grants,bounds,briefing,demo-lea}.feature`,
puis ce document.
**Prochain agent :** implémenter le **Lot K** puis le **Lot D**, BDD-first,
détag progressif, puis le runbook jour J. Gates Mathieu : répétition en
conditions réelles avant la démo.

---

## 1. État exact à la reprise

### Fait et vert (cette session, commits Mac canoniques)

| Commit | Contenu |
|---|---|
| `6ba28d6` | `docs/DEMO-LEA-SCENARIO.md` — scénario de référence validé |
| `190d6b4` | Les 4 contrats `@wip` committés seuls (V0) |
| `0e59e91` | **Lot W** : décision d'octroi ≠ classe de risque (writes grantables, défauts sûrs, révocation politique, mismatch config/manifeste fail-closed, CLI `TOOL=read\|write[:granted\|denied]`) |
| `56d2a14` | **Lot P** : bornes d'arguments (`one_of`/`time_slots`/`forbid`/`require`/`max_items`) scellées au manifeste hors pin hash, check après authorize avant log, refus `bound_violated` pédagogique, zéro hit coffre/amont, forme par schéma pinné, CLI `--bound TOOL:FIELD=RULE` |

**Suite au vert (vérifiée avant ce handoff)** : 61 unit, 4 CLI,
**72 scénarios / 355 steps** Cucumber (`gateway-grants` 6/6 et
`gateway-bounds` 12/12 détaggés), 5 e2e réseau, 5 owner-side, clippy
`-D warnings` et `cargo fmt --check -p aithos-gateway` clean.

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

### Encore `@wip` (le travail de la prochaine session)

- `gateway-briefing.feature` — 8 scénarios (Lot K) ;
- `gateway-demo-lea.feature` — 8 beats (Lot D).

### Leçon d'environnement neuve (committée avec le Lot P)

Le main Cucumber tourne désormais en **séquentiel**
(`max_concurrent_scenarios(Some(1))`) : les mondes lancent de vrais
serveurs sockets et du crypto owner-side bloquant dans les steps ; en
concurrent, les workers tokio s'affament et les réponses wire ratent le
budget broker de 5 s (timeouts vault flaky, deux scénarios victimes
déterministes). Ne pas retirer ce réglage sans traiter la cause.

## 2. Lot K — briefing (design figé, à implémenter)

Contrat : `gateway-briefing.feature`. Décisions déjà prises
(`DEMO-LEA-SCENARIO.md` §5) : outil natif `briefing.read`, zones
**public + circle** servies, **`self` jamais**, surface conditionnelle
(consignes présentes → outil listé + `initialize.instructions` ; tout
vide et rien d'inscriptible → surface muette), lecture journalisée dans
le gamma du contexte, édition owner servie à la lecture suivante, préfixe
`briefing` réservé partout.

Pointeurs d'implémentation (à vérifier dans le code, pas de terrain
miné connu) :

- **Réservations** : `config.rs` `RESERVED_PREFIX`/`validate_tools` +
  `is_reserved_server` ; `hub.rs` `validate_server` — ajouter `briefing`
  à côté de `journal`/`gateway`, avec les scénarios config du contrat.
- **Côté core (`core_bridge`, seule porte)** : la mécanique sections
  existe — regarder `journal_write`/`journal_search` (lot C2, pattern
  complet d'un outil natif : dispatch dans `proxy_mcp::journal_dispatch`,
  bridge, lecture journalisée via `log_read_as_agent`-équivalent),
  `owner_read_journal_note`, `record_section_add/rewrite` (couture pass-L
  « écritures de CONTEXTE » notée comme non exercée — c'est peut-être
  exactement la couture des consignes owner), `ensure_folder`/`publish`,
  `deliver_zone_line`. Le gateway lit les manifests par sa ligne vault :
  la lecture des sections de consignes demandera une ligne/clé adaptée
  par zone — trancher la couture minimale, documenter.
- **Outillage owner** : une commande `owner-set-briefing --label <ctx>
  --zone public|circle --title <t> --text <t>` (création + rewrite) qui
  réutilise les writes owner du core. Elle sert aussi au beat 7 (édition
  à chaud) et au runbook.
- **Runtime** : dispatch de `briefing.read` AVANT resolve dans
  `process_multi` (pattern `JOURNAL_WRITE`), refus §3bis.8 journal seul ;
  `initialize` : ajouter `instructions` au résultat statique QUAND au
  moins une zone grantée est non vide (état calculé à l'open, rafraîchi
  par lecture — trancher : recalcul par appel est le plus simple et rend
  le beat 7 exact) ; `tools/list` : ajouter le descripteur conditionnel
  à côté des outils journal.
- **Multi-contexte** : `briefing.read {context?}` optionnel — sans
  argument, toutes les consignes grantées étiquetées par contexte et par
  zone (le contrat l'exige : « labeled by context », « names the zone »).

## 3. Lot D — répétition générale (après K)

Contrat : `gateway-demo-lea.feature` (Background à table + 8 beats). Le
harness = fusion des mondes existants : `provision_bounds_world`
(coffre + wire MCP + bornes, généraliser à N serveurs) + consignes du
Lot K + le write DENIED supplémentaire par serveur (`delete_email`,
`create_page`). Étapes :

1. Détag des 8 beats, steps composites (le Background provisionne tout
   le monde Innoestate en une fonction).
2. **e2e réseau** `tests/e2e_demo_lea.rs` sur le modèle d'`e2e_vault.rs` :
   vrai binaire, trois faux MCP + faux Vault sur sockets, le storyboard
   complet dont l'édition de consigne à chaud (via la commande owner du
   Lot K) et le balayage de sentinelles.
3. **Runbook** `docs/DEMO-LEA.md` (pendant de `DEMO-GATEWAY-VAULT.md`) :
   vrai Vault Docker, vrais connecteurs — **vérifier par recherche web
   l'état 2026** du MCP Notion (HTTP direct + token ≈ prêt pour notre
   coffre) et des MCP Gmail/Calendar (probablement stdio+OAuth → wrapper
   HTTP loopback à documenter, notre wrapper stdio générique restant
   Phase D) ; Cowork branché sur l'endpoint unique ; la checklist des 8
   beats avec ce que Mathieu montre à l'écran. Rappels runbook : aucune
   valeur réelle de token dans Git/handoffs, TLS reqwest non compilé
   (loopback only, feature `rustls-tls` = une ligne pour le réel).

## 4. Protocole d'environnement (inchangé, à la lettre)

GATEWAY-HANDOFF §5, profil cloud+janitor : `git archive HEAD` sur la VM →
tar dans `_transfer/` → build/test dans le conteneur cloud
(`CARGO_INCREMENTAL=0`, target dédié) → retours **fichier par fichier**
via device_commit_files avec sha256 croisés → commits janitorisés sur le
Mac (mv des `.git/*.lock` vers `_gitjunk/` avant chaque commande git
écrivante, jamais de `git status` intercalé ; warnings `tmp_obj`
cosmétiques). Commits par tranche : chaque commit Mac porte l'état exact
de sa tranche. Scories intactes : `_gitjunk/`, `_to_delete/`,
`_transfer/` (+ tars), `docs/EXPLORATION-DESKTOP-GATEWAY.md`, l'input
`HANDOFF-GATEWAY-VAULT-FINALIZATION-2026-07-15.md` untracked.

## 5. Gates et limites

- Ne pas merger `main`, ne pas déployer, aucune donnée/token réels dans
  le repo. Les valeurs de démo sont générées.
- Après D : **répétition générale avec Mathieu** en conditions réelles
  (gate explicite), puis seulement V4 LLM / writes réels côté Ethos /
  `resources/*` — voir `HANDOFF-GATEWAY-VAULT-DONE-2026-07-15.md` §5
  pour la liste longue.
- En fin de session : suite complète + clippy + fmt, synchro sha-croisée,
  paragraphe §6 GATEWAY-HANDOFF + état express, et un handoff comme
  celui-ci.

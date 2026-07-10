# Aithos Gateway — Handoff (reprise en contexte neuf)

**But.** Reprendre le chantier du gateway (runner conteneurisé) sans rien reperdre.
Complète `GATEWAY-BOOTSTRAP.md` (le pourquoi/quoi) avec l'état exact du code et
les leçons d'environnement. Session initiale : 2026-07-10.

**Branche : `feat/gateway`** (depuis feat/f-plus). Crate : `rust/crates/aithos-gateway/`.

---

## 1. État : MVP audit VERT (première brique vendable)

`tests/features/gateway-audit.feature` — **5 scénarios / 29 steps verts**, plus
8 tests unitaires (config, policy) et 3 tests de surface CLI (`cli_surface.rs`,
binaire réel). Le parcours vendu fonctionne de bout en bout en lib :

on plugge un agent (config YAML + onboard), les lectures passent et sont
tracées, les écritures et l'inconnu sont refusés fail-closed et les REFUS sont
tracés, le kind est imposé par l'opération (jamais par l'appelant), la chaîne
se vérifie offline, et un auditeur tiers exporte exactement sa tranche
(`read.gamma#kind=action`) — requête plus large refusée par le certificat.

```
cargo test  -p aithos-gateway --manifest-path rust/Cargo.toml   # tout le crate
cargo clippy -p aithos-gateway --all-targets -- -D warnings      # clean
```

## 2. Décisions prises (avec Mathieu, 2026-07-10)

- **Transport MCP v1 : Streamable HTTP** (JSON-RPC sur POST `/mcp`, stateless).
  stdio amont = plus tard, via wrapper, sans toucher le flux.
- **Ethos v1 : disque local** (FsStore), le cloud DOIT rester possible →
  `StoreConfig` parse déjà `s3` mais `store_adapter` le refuse (fail-closed).
- **Config entreprise : YAML whitelist** `tools: {outil: read|write}`,
  `deny_unknown_fields`, default-deny pour tout outil absent.
- **proxy_llm v1 (post-MVP) : OpenAI-compatible** d'abord (stub en place).

## 3. Architecture posée (et pourquoi)

- **`core_bridge` = SEULE porte vers aithos-core/bundle** (avec son annexe
  `store_adapter`). Tout le reste (policy, config, proxy) parle en noms
  d'outils et verdicts. Les évolutions d'API du core s'absorbent là. Le bridge
  ré-exporte l'entropie (`EntropySource`, `OsEntropy`, `SeqEntropy`) pour que
  binaire et tests n'importent jamais le bundle.
- **Trois mandats à l'onboarding** (grants loggés, jamais silencieux) :
  agent (`act.x.mcp.<action>` par outil read), **gateway lui-même**
  (`act.x.gateway.*` — un refus n'est PAS un acte de l'agent, c'est un acte de
  gouvernance du gateway, sous sa propre clé), auditeur
  (`read.gamma#kind=action`).
- **Double mur d'enforcement** : `authorize` (verify_chain + action_covered)
  pour refuser proprement AVANT de relayer ; `log_action` re-vérifie tout à
  l'append (chaîne, révocations, budgets) — le bundle refuse lui-même de
  logger un acte non couvert. **Log-before-relay** : pas d'entrée gamma → pas
  d'appel amont.
- **Contrainte de grammaire absorbée côté gateway** : les actions d'act se
  découpent au DERNIER point (`act.x.<connector>.<action>`), donc les noms
  d'outils MCP pointés (`user.read`) s'aplatissent (`user_read`) dans le
  mandat ; le nom brut reste dans le payload clair (`tool`) ; les collisions
  post-aplatissement sont rejetées à la config.
- **Keyholder** : seeds agent + gateway, zeroize, jamais sérialisés vers la
  console (testé en surface). Persistés entre onboard et run via le store
  (`gateway/keys.json`) — **v1 disque local uniquement ; passer par KMS/keystore
  scellé avant tout store cloud** (le refus S3 de v1 verrouille ça).

## 4. Reste à faire (itérations suivantes, cf. GATEWAY-BOOTSTRAP §7)

- Sceller les args des actes (`sealed_args` §07.9.3 — l'API `log_action` les
  accepte déjà, il manque `grant_audit_line` à l'onboard + le flag).
- `tools/list` filtré par le mandat (démo plus propre ; enforcement au call
  déjà là). Passthrough SSE/streaming du Streamable HTTP complet.
- `proxy_llm` OpenAI-compat (kind `inference`, budgets tokens, creds vault),
  `proxy_web` (fenêtres + domains), ethos S3, container/pod (deployment doc),
  test d'intégration HTTP réel (axum+reqwest bout en bout ; la lib est
  couverte, la surface run l'est par démarrage manuel).

## 5. Env sandbox — leçons du 2026-07-10 (s'ajoutent au HANDOFF §5 du core)

- **CARGO_HOME dédié sur le volume** : `rust/target-linux/cargo-home` (les
  caches /tmp appartiennent à `nobody` après recyclage VM et /tmp est plein).
  Seedé depuis `/tmp/cargo2/registry` (cache+index lisibles).
- **CARGO_TARGET_DIR SÉPARÉ PAR SESSION** : `rust/target-linux-gw` pour le
  gateway. NE PAS partager `target-linux` avec la session core : les flocks
  cargo sont inopérants sur le montage FUSE → deux cargo concurrents se
  corrompent mutuellement (rmeta/dep-info mutilés, E0463 en cascade).
- **`timeout 40` tue cargo en pleine écriture** : après chaque kill, purger
  les artefacts du crate en cours (`rm deps/<crate>-*`), sinon la corruption
  empoisonne les builds suivants. Les GROS crates (tokio ~34 s) se buildent
  seuls : `cargo build -j 1 -p tokio` dans une tranche pleine.
- SIGBUS rustc sporadiques (mmap sur FUSE) : retry, puis purge ciblée.

## 6. ⚠ Git : sessions parallèles, un seul working tree

La session core (G/move-as-rotation) et la session gateway partagent le même
working tree. Le passage à `feat/gateway` a déplacé HEAD sous la session core :
ses commits `e721fce` (spec+feature move) et `f613a93` (vector G3) ont atterri
SUR `feat/gateway`, intercalés entre les commits gateway. Fichiers disjoints,
rien ne se conflicte, mais à réordonner à la fin (cherry-pick vers feat/f-plus
ou merge global — décision Mathieu). Éviter à l'avenir : `git worktree add`
par session, ou une seule session git-active à la fois.

# Handoff — surface mandats, reprise : Lot M3 (émission/révocation multi)

**Date :** 2026-07-16 (préparé en 10ᵉ session gw, M0+M1+M2 clos)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD d'entrée :** `67a6c34` (docs: mandates surface session close)
**Références, dans l'ordre :** `docs/HANDOFF-MANDATES-SURFACE-2026-07-15.md`
(LE plan intégral M0→M6 — untracked, inchangé, toujours faisant foi),
`docs/MANDATES-PRODUCT-GAPS.md` (le cahier des écarts — untracked),
`spec/04-mandates.md` / `spec/05-delegation.md` / `spec/08-connectors.md`,
`docs/GATEWAY-HANDOFF.md` (état express 10ᵉ + protocole d'environnement
§5 À LA LETTRE), puis ce document.
**Mission de la prochaine session gw :** Lot M3 seul, à fond (1 à 1½
session). M4 (`id=`) et M5 (atténuation) restent des SESSIONS CORE
DÉDIÉES, parallélisables, avec leurs contrats déjà committés.

---

## 0. Ce qui est fait (ne pas refaire)

- **M0 tranché** (2026-07-16, AskUserQuestion, les six recos Mathieu) :
  (a) mandats restreints = **roots owner multiples**, containment vérifié
  à l'ÉMISSION contre politique Ethos ∩ manifeste approuvé ;
  (b) **un seul mandat actif par (Ethos, keypair)** — M3 refuse le doublon ;
  (c) clés de contraintes inconnues en sous-délégation = **refus
  fail-closed** (gravé dans les contrats M5) ;
  (d) N mandats émis, **UN runner actif par contexte** jusqu'au
  RemoteStore — limite documentée, les actes de délégués s'exercent au
  STORE (protocole), jamais par un second runner ;
  (e) chantier APRÈS le gate répétition démo Léa (prioritaire, intact) ;
  (f) nommage `owner-issue-mandate` / `owner-revoke-mandate` /
  `owner-preview-mandate`.
- **M1 clos** (`aa02353`, contrats SEULS, sondes de parse validées) :
  `tests/features/gateway-mandates.feature` (16 scénarios),
  `features/e-mandate-sections.feature` (8), matrice d'atténuation dans
  `features/f-plus-constraints.feature` (26). **Ne jamais modifier un
  contrat committé sans décision Mathieu consignée.**
- **M2 clos** (`f8cbc88`) : moteur de politique effective PUR dans
  `core_bridge.rs` (§2 ci-dessous), CLI `owner-preview-mandate`,
  5 tests d'équivalence, 2 scénarios preview détaggés, 2 owner_surface
  neufs. **Le chemin chaud n'est PAS rebranché** — l'équivalence est la
  preuve ; le rebranchement éventuel reste un lot ultérieur explicite.

### La baseline figée (2026-07-16, tout vert, à revalider À L'IDENTIQUE)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | 62 unit, 4 CLI, **90 scénarios / 481 steps**, **6 e2e**, **7 owner**, **5 équivalence** (`tests/policy_equivalence.rs`) |
| aithos-core + bundle + cli | **97 tests** + Cucumber bundle **203 scénarios / 826 steps** |
| Hygiène | clippy `-D warnings` + `cargo fmt --check` clean (gateway) |

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-core -p aithos-bundle -p aithos-cli
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

Restent `@wip` : **14** dans gateway-mandates (M3 en détagge 12 ; la
section `id=` attend M4, « Remaining uses … gamma » attend M6), **8**
dans e-mandate-sections (M4), **26** dans f-plus-constraints (M5).

## 1. Périmètre M3 (plan §4, inchangé) + repères de code

`core_bridge::owner_issue_mandate(master, label, grantee_pub, outils⊆,
zones/dirs⊆, contraintes, window)` :

- **Validation par le moteur M2** — la même porte, fail-closed : outil
  demandé absent ou non granté au manifeste → refus nommant l'outil et
  la décision ; borne du manifeste héritée telle quelle, jamais
  éditable, tentative d'élargissement → refus nommant le champ et la
  valeur ; contrainte seulement resserrante.
- Mint d'un **root** vers la pubkey destinataire (le privé `mint()` /
  `mint_entries()` de core_bridge prend déjà un `grantee_pub`
  arbitraire), certificat persisté `certs/<id>.json`, **grant
  journalisé** (l'émission n'est jamais silencieuse).
- **Registre additif owner-side** `gateway/issued.json` (id, label du
  mandat, grantee_pub, émis à, statut/raison) — **le `state.json` du
  runner ne bouge PAS** (zéro impact runtime, c'est l'invariant de
  non-régression du lot).
- Cardinalité (b) : refus si un mandat **actif** (non révoqué, fenêtre
  ouverte à T) existe déjà vers cette keypair sur cet Ethos — le refus
  nomme le mandat en place ; après `owner_revoke_mandate`, l'émission
  passe.
- `owner_revoke_mandate(master, label, mandate_id, reason)` :
  `log_revoke_owner` ciblé (mécanique existante — voir
  `owner_reenroll_server`, core_bridge, qui révoque déjà politiquement)
  + trace au registre.
- Attribution : deux clés délégués, deux actes — deux signatures
  distinctes vérifiables offline, `authorized_via` disjoints. Les actes
  de délégués s'écrivent au store par le chemin protocole (bundle
  `log_action` avec la chaîne du délégué), pas par le runner (décision d).
- Invariants : la ligne vault `/x/<server>` ne se livre JAMAIS sur un
  `act.*` (custody gateway) ; les **7** tests owner_surface existants
  inchangés ; sous-ensembles de zones/dossiers via la couture pass-L
  (`GrantSpec`/`deliver_zone_line`) pour le scénario « narrows the
  folders ».
- CLI : `owner-issue-mandate`, `owner-revoke-mandate` ; étendre
  `owner-preview-mandate` à un mandat émis du registre (par id) —
  extension ADDITIVE de `preview_load` (aujourd'hui il lit
  `state.agent_mandate` ; le statut `revoked` par révocations est déjà
  câblé dans `preview_status`).

## 2. Le moteur M2, tel qu'il est (à réutiliser, pas à réécrire)

Dans `core_bridge.rs`, section « effective policy (M2) » :

- `EFFECTIVE_POLICY_VERSION = "aithos-effective-policy-v1"`.
- Privés : `preview_load` (état + cert + `did.json` + révocations du
  gamma + manifestes scellés par `owner_read_hub_manifest`),
  `preview_status` (revoked > not_yet_valid > expired > active/invalid,
  comparaisons de chaînes RFC 3339 Z — le style du core),
  `effective_call_verdict` (resolve → `verify_chain_revocable` →
  `covers_act` → bornes, messages IDENTIQUES au runtime, préfixe
  « exposed tool `<t>`: » compris), `describe_effective_policy`
  (read-model : granted/covered/served + bornes héritées).
- Publics : `owner_preview_mandate(...)`, `owner_preview_call(...)`.
- Nuance assumée et documentée : le verdict pur conjoint les
  révocations ; le pré-check runtime (`authorize`) les délègue au mur
  d'append — aucun état runtime atteignable ne diverge, et
  `tests/policy_equivalence.rs` l'exige LITTÉRALEMENT (code + message)
  sur toute la matrice grants/bounds. Toute retouche du moteur doit
  garder ces 5 tests verts tels quels.

## 3. Rituel du lot (inchangé, non négociable)

Décisions → (contrats déjà là) → impl par tranche → détag progressif —
suite complète verte à CHAQUE détag → e2e → docs. Un compteur existant
qui bouge sans détag = STOP. Cucumber gateway SÉQUENTIEL
(`max_concurrent_scenarios(1)` — ne pas retirer). Commits par tranche,
protocole cloud+janitor GATEWAY-HANDOFF §5 à la lettre : sondes
egress/unlink d'abord (débris dans `_to_delete/`), `git archive HEAD` →
tar dans `_transfer/` → sha256 croisé → build/test cloud
(`CARGO_INCREMENTAL=0`, target dédié) → retours device_commit_files
fichier par fichier sha256-croisés → janitor des locks avant chaque
commande git écrivante (`mv .git/*.lock _gitjunk/`), warnings `tmp_obj`
cosmétiques, JAMAIS de `git status`. Pas de merge `main`, pas de
déploiement, aucune donnée/token réels. Scories intactes (`_gitjunk/`,
`_to_delete/`, `_transfer/`, docs untracked — dont le plan SURFACE et
le cahier GAPS, décision de commit à Mathieu).

En fin de session : suites complètes + clippy + fmt, synchro
sha-croisée, état express + §6 GATEWAY-HANDOFF, et un handoff de
reprise comme celui-ci (untracked, HEAD d'entrée exact).

## 4. Gate

La répétition générale de la démo Léa (`docs/DEMO-LEA.md`,
`docs/DEMO-LEA-SCENARIO.md`) reste **prioritaire et indépendante** — ce
chantier ne la retarde jamais. Le chemin chaud de la démo n'a pas bougé
en 10ᵉ session (M2 est additif) et ne doit pas bouger en M3 (le
`state.json` du runner est intouchable dans ce lot).

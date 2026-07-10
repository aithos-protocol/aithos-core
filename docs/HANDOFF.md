# Aithos Core — Handoff (reprise en contexte neuf)

**But.** Reprendre l'implémentation de référence d'aithos-core sans rien reperdre.
Résume où on en est, comment on travaille, et la prochaine étape exacte.

**Branche : `feat/obligations`** (créée depuis le HEAD complet `feat/gateway` ;
F+, G **et G+** entièrement closes ici ; le crate `aithos-gateway` ride sur la
branche mais n'est JAMAIS touché — sessions parallèles ; merge = décision
Mathieu). Repo : `code/aithos-core/`.

---

## 1. Ce qu'est aithos-core

Couche de confiance pour agents IA, **enforceable depuis les fichiers seuls, sans
serveur de confiance** : identité, mandats scopés, délégation récursive, révocation
scopée, log d'actions inviolable. Primitives : BLAKE3 (dérivation), XChaCha20-Poly1305
(AEAD), Ed25519 (signatures), X25519+HKDF-SHA256 (scellés ECIES). La spec normative
vit dans `spec/00..10` — **source de vérité**.

Profil cible : **absentee owner** — l'owner émet un mandat large puis ne repasse
presque jamais ; toute la maintenance est récursive.

## 2. Méthode de travail (à respecter à la lettre)

Rituel par étape du plan (`docs/EXECUTION-PLAN.md`) :

1. **Feature d'abord (BDD).** `.feature` Gherkin (anglais) co-écrit AVANT le code,
   scénarios `@wip` (le runner les saute → suite verte). C'est le contrat.
2. **Vecteurs d'abord (TDD).** Valeurs attendues générées **indépendamment** en Python
   — le générateur est committé (`vectors/gen-f.py`, auto-validé contre B2/E1).
3. **Impl** jusqu'à vecteurs + scénarios verts ; dé-tagger `@wip` au fur et à mesure.
4. **Checkpoint manuel CLI** (non bloquant).
5. **Commit** anglais clair. Validation utilisateur → étape suivante.

Règles gravées : pureté du core (zéro I/O/horloge/RNG — temps `T`, aléa, storage
injectés) ; DDD-lite (types = noms spec) ; fail-closed (un variant d'erreur par
rejet) ; wire figé étape 0 (multibase base58btc, JCS RFC 8785, AAD §00.3).

## 3. Décisions de design (rappels + celles du recul gamma 2026-07-10)

- **Une seule clé de contenu owner** (`content_sign`) ; audience dans le payload
  signé, jamais dans la clé. self = jamais signé (§02.11).
- **Décision B** : mandat de tête = dead-man heartbeat par défaut (30 j).
- **Clé de succession** froide, non dérivée de S.
- **Une clé, N périmètres** ; sids stables = labels de dérivation.
- **Gamma (recul validé)** : enveloppe **2 couches** — header de comptage CLAIR
  (id/prev/at/kind/via/sig, nécessaire au comptage offline par tout verifier
  jusqu'à H) + **corps scellé** `{target,payload}` pour les `section.*` sur zones
  à clé (public reste clair). Hint de reconnaissance dérivable par les seuls
  détenteurs de la clé du nœud (`gamma-hint`). **La frontière de lecture est la
  LISIBILITÉ, pas l'auteur** : un grant de sous-arbre ouvre les corps de son
  périmètre, owner = tout (S), étranger = squelette. Log **segmenté par mois**
  (`gamma/<YYYY-MM>.jsonl`), append aveugle (gamma_head suffit). `read.gamma`
  = moitié certificat (policy sur kind/action/date, physique sur dir/id/tag).
  À H : racines committées → preuves O(log n), option de sceller kind/via (champ
  `v` prêt). Crypto-erasure par destruction de clé.
- **Coverage NODALE (gravée 2026-07-10, passe move)** : un `dir` de périmètre
  nomme son dossier par le **sid terminal** (le préfixe = adresse à l'émission,
  gardée pour l'audit, jamais une contrainte). Couverture ssi la chaîne de la
  cible **passe par ce sid** — chaîne COURANTE pour une op, listes émises pour
  le containment §05.3 (délégations pré-move stables). Équivalent strict au
  préfixe-segments tant que rien n'a bougé (sids uniques) ; ne diverge qu'après
  move. Les `gamma-selector` dirs restent POSITIONNELS (coordonnées historiques
  du log). Spec : §04.2 amendée + note §02.9. `dir_covers()` dans mandate.rs.
- **Move = rotation (§02.9, décision validée)** : couper l'ancien parent est LE
  but (sinon move serait gratuit comme rename — garder son accès serait un wrap
  O(1), trivial mais sur-grant silencieux, refusé). Ligne directe sur M =
  survivante (policy ET crypto) ; wrap via le NOUVEAU parent ; ré-encryption
  eager du sous-arbre (posture rotate_folder). Header de M reconstruit au
  NOUVEAU chemin (AAD) à v+1, l'ancien fichier reste en archéologie. Couper
  quelqu'un = `revoke`, jamais move. Option future : `move --keep-old-access`.
- **Obligations §4.12 — 4 décisions gravées (Mathieu, 2026-07-10, commit
  08bc9e8)** : (1) **UN wire** — counter_sign/binding se désugarent en
  obligation réservée `co_sign` (attestor = clé content owner, verdict
  approve, Δ_cosign 5m) ; le reçu signe TOUJOURS le payload complet
  `{obligation, mandate_id, action, args_hash, verdict, presented_digest?,
  at}` (« implicite » = la déclaration, jamais les octets signés — §4.6
  amendée) ; (2) **mandate_id = authorized_by de l'entrée** (la feuille) — un
  reçu ne se transfère jamais entre sous-mandats frères, même obligation
  ancêtre et args identiques ; (3) **atténuation JCS-stricte** — une
  obligation héritée doit être byte-identique chez l'enfant, resserrer =
  AJOUTER (la conjonction est le resserrement), counter_sign/binding ⊇ en
  sets de noms d'action ; (4) **refus = Err pur** (GammaObligationUnsatisfied,
  rien d'appendé — précédent check_budgets) ; le « refusal log » est un devoir
  du gateway, hors protocole. Astuce d'impl qui PORTE l'anti-rejeu : le
  vérificateur reconstruit le payload depuis les coordonnées de l'ENTRÉE —
  toute divergence (mandat, action, args, digest) tue la signature, zéro
  comparaison à la main.

## 4. État du code (10 étapes closes sur 13)

Workspace cargo 4 crates : `aithos-core` (pur), `aithos-bundle` (I/O, Store),
`aithos-cli`, `aithos-wasm`.

| Étape | Statut | Livré |
|---|---|---|
| 0 Conventions | ✅ | wire.rs, jcs.rs, cucumber harness |
| A Identité | ✅ | keys.rs, did.rs — A1, A2 |
| B Dérivation | ✅ | derive.rs, path.rs — B2 |
| C Scellés | ✅ | seal.rs, header.rs — C1, C2 |
| D Bundle | ✅ | bundle.rs, manifest.rs, entropy.rs, FsStore |
| E Mandats | ✅ | mandate.rs, grants.rs — E1 |
| **F Gamma** | ✅ | gamma.rs (chaîne, enveloppe, compteur, heartbeat, ancre), log.rs (segments, appends autorisés, query owner/agent), PerimeterEntry::{Act,Gamma}, covers_act/covers_gamma_query, manifest.gamma_head, SectionSpec/ActionSpec (dette params réglée) — F1, F2, F3 |
| **F+ Contraintes avancées** | ✅ | constraints.rs (Window arithmétique half-open, BudgetProfile OU, reçus d'attestation, action_params), kinds inference/ethos.read + classes registry, atténuation fenêtres dans verify_chain, vault d'audit (`e/x/header.json`) + args scellés §7.9.3, log_inference/log_read_as_agent, audit owner (audit_action_args, audit_log_against) — vecteur F+ |
| **G Révocation** | ✅ | revocation.rs (set actif, forward-only, autorité issuer/ancêtre/watchdog), PerimeterEntry::Revoke, verify_chain_revocable (état injecté, §04.5 step 4), revoke.rs bundle (rotate_folder : versions header, survivants, up-link 2bis, ré-encryption, lecture version-aware), log_revoke owner/délégué, CLI `revoke [--rotate]` — G1, G2. **Move-as-rotation (§02.9) SOLDÉ (2026-07-10)** : `move_folder` (re-parentage sid stable, DK' au nouveau chemin scellée à l'ancien line set, wrap via NOUVEAU parent, ré-encryption), coverage nodale `dir_covers` (§04.2), écritures circle version-aware (`section_add` via `owner_current_section_key` — trou réel : écrire à v1 sous un dossier tourné/déplacé rendait le contenu à l'ancien parent), marche de clé agent descendante (wraps parent→enfant + zroot), `Header::build_at`, 3 scénarios, vecteur G3, CLI `move` + test surface |
| **G+ Obligations** | ✅ | **SOLDÉ (2026-07-10 soir)** : constraints.rs (Obligation/AppliesTo, parse_obligations + désugarage counter_sign/binding → instance `co_sign`, verify_obligation_receipt — payload RECONSTRUIT des coordonnées de l'entrée, le rejeu cross-mandat/action/args meurt dans la signature —, check_obligations, obligations_attenuate JCS-strict), branché check_action_append (gamma.rs, à côté de check_budgets, clé content injectée du DID doc) + verify_chain (mandate.rs, add-only par maillon), log_action_with_checks (log.rs, additif — ActionSpec intact, le gateway build inchangé), GammaObligationUnsatisfied fail-closed. CLI grant-act --obligations-json/--counter-sign, action --check-json, **approve** (--approver-seed-hex ou --owner-seed-hex co_sign, --presented WYSIWYS, --key-only). Vecteur G+ Python indépendant (gen-gplus.py auto-validé contre le reçu F+) : JCS byte-identique, 6 reçus valides, 8 négatifs, atténuation. 26 scénarios, 2 tests de surface |
| H Merkle | ⬜ | racines zones + **racines gamma** (segments + trie mandate_id→count) |
| I Concurrence | ⬜ | merge disjoint, fork, entrées merge |
| K Intégration | ⬜ | scénario K, Docker, npm |

**Tests : 149 scénarios / 522 steps cucumber (zéro skip) + vecteurs
A→F+/G1/G2/G3/G+ + 15 tests de surface CLI, tous verts ; `clippy --workspace
--all-targets -- -D warnings` clean ; `cargo fmt` passé.**

Deux trous de discipline découverts et soldés (2026-07-10) : (1) le scénario
« A revoked chain is refused at verification time » était silencieusement
SKIPPÉ (step revoke définie en `#[when]` seulement, utilisée en Given) — le
« +1 skip move @wip » de l'ancien handoff, c'était LUI ; (2) l'ancien scénario
move passait À VIDE (steps stubs `{}`, pas de tag @wip) et son Given était
faux au regard de la sémantique clarifiée (ligne directe = survivant).

CLI : `grant-act` (--max-actions, --heartbeat-every/grace, **--budgets-json,
--windows-json, --obligations-json, --counter-sign**), `action` (--cert
répétable, **--budget-ref --model --tokens --receipt-json --args-json
--check-json**), **`approve`** (reçu d'obligation signé ; --approver-seed-hex
device OU --owner-seed-hex co_sign ; --presented WYSIWYS ; --key-only sort la
pub multibase à épingler), **`inference`**, `heartbeat`, `log-show`,
`log-verify`, `log-query` (--kind accepte les classes, ex. ethos.write),
**`move <folder> --under <parent>`** (publie l'édition). `grant` logge son
entrée gamma.
Décision de wire F+ à connaître : instants en RFC 3339 Z, jamais d'epoch ns —
RFC 8785 sérialise les nombres en doubles IEEE 754, un epoch ns (>2^53)
perdrait de la précision dans les octets signés.
Checkpoint manuel déroulé : 3 actions max_actions=3 → 4ᵉ `GammaBudgetExhausted` ;
heartbeat 10s/5s → silence 16s → `GammaHeartbeatStale` → beacon → reprise. ✔
Checkpoint G+ déroulé (bloc 12 du guide) : publish sans reçu → refus fail-closed ;
approve WYSIWYS → logged ; rejeu autres args → « args_hash does not match » ;
reçu 12 min / max_age 5m → « stale (720s > 300s) » ; counter_sign : send refusé,
reply passe, co_sign owner → logged ; log-verify OK. ✔

## 5. Comment builder / tester

```
cargo test  --workspace --manifest-path rust/Cargo.toml
cargo clippy --workspace --manifest-path rust/Cargo.toml --all-targets -- -D warnings
python3 vectors/gen-f.py   # régénère F1-F3 (auto-check B2/E1 d'abord)
```

**Env sandbox (2026-07-10 soir, session move — la VM se dégrade à chaque
recyclage, lire AVANT de builder)** :
- toolchain : `/tmp/rustup/toolchains/stable-aarch64-unknown-linux-gnu/bin` EN
  DIRECT dans PATH (rustup shim cassé). `CARGO_INCREMENTAL=0`.
- **Disque VM PLEIN** (100%, résidus `nobody` indélébiles, pas de sudo) →
  `CARGO_HOME=<repo>/rust/cargo-linux` (sur le volume Mac, gitignoré) ET
  `CARGO_TARGET_DIR=<repo>/rust/target-linux`. `rust/target/` = artefacts macOS
  de Mathieu, NE PAS toucher.
- **Piège mortel : les kills à ~40s laissent des artefacts DÉCHIRÉS sur le
  montage** (fingerprint ok, .rmeta absent/corrompu → E0463/E0460, StableCrateId
  collisions, ICE). Recette qui converge : builds `-j 1`, `sync` après chaque
  tranche, capturer la dernière ligne `Compiling <crate>` et `cargo clean -p
  <crate>` au début de la tranche suivante ; itérer cible par cible (d'abord
  `-p aithos-bundle --test cucumber`, le workspace complet à la fin). Si le
  registry part en vrille (collisions serde) : `rm -rf cargo-linux/registry/src`
  (les tarballs re-extraient, réseau crates.io OK).
- Suppressions bloquées sur le montage → outil allow_cowork_file_delete au
  premier "Operation not permitted". `.git/*.lock` à rm avant commit.
- `rust/target` a été PURGÉ du tracking git (7938 fichiers macOS committés par
  erreur depuis B ; l'historique garde le poids — réécriture = décision à part).
  `.gitignore` couvre `rust/target*/` et `rust/cargo-linux/`.

## 6. Prochaine étape : H — Merkle

**G+ est ENTIÈREMENT close (2026-07-10 soir, cette session).** Rituel intégral
déroulé : 4 points de décision validés par Mathieu → spec amendée (08bc9e8) →
feature committée AVANT le code (fe508c6, 26 scénarios) → vecteur Python
indépendant auto-validé contre le reçu F+ (71e93d5) → impl core (9f572db) →
CLI + 2 tests de surface (6b39413) → checkpoint manuel bloc 12 ✔. Voir tableau
§4 et décisions §3 (les 4 décisions gravées). `verify_op` intact, zéro
régression de vecteur, le token-receipt F+ garde son squelette (verify_receipt
mesure, verify_obligation_receipt gate — mêmes octets Ed25519+JCS). Gateway
jamais touché.

**G est ENTIÈREMENT close** (move-as-rotation soldé 2026-07-10, voir tableau §4
et décisions §3 — coverage nodale + move). Les 3 scénarios move sont verts, le
vecteur G3 croise le générateur Python indépendant, la CLI a `move`.

**Décision G gravée (rappel)** : révocation = UNE entrée gamma `revoke` (pas de
doc autonome §6.4). `verify_chain_revocable(chain, doc, at, revs)` = §04.5
étape 4, état injecté (pureté core). `verify_chain` inchangée appelle avec revs
vide → zéro régression. Rotation version-aware : la clé de section se résout
par le header de la folder à `row.key_version` (up-link wrap pour les dérivants).

**H — Merkle** (spec §02.10 + racines gamma) : `H_leaf/H_node` domain-separated,
racines par zone + racines gamma (segments + trie mandate_id→count), preuves
O(log n), diff par descente. + le durcissement offline « no mislabeled effects »
rangé en défense en profondeur (additif, sans breaking wire — voir plan §H).

## 7. Points ouverts / dette assumée (non bloquants)

- Lecture gamma côté agent sur zone `self` : la walk des descripteurs avec clés
  d'agent n'est pas écrite (owner-only pour l'instant) — les scénarios couvrent circle.
- Wildcard `act.x.<c>.*` : le refus des actions classe `binding` attend les
  manifests connecteurs (§08.1) — TODO noté dans covers_act.
- **Post-move (passe 2026-07-10, assumé)** : (a) tag-views ancrées sur un dossier
  déplacé — l'ancre suit le nœud en dérivation mais les wraps de vue ne sont pas
  re-postés : fail-closed (« no key path »), passe dédiée si besoin ; (b) header
  de GRANT profond sous un ancêtre tourné/déplacé garde sa lignée v1 (classe de
  limite héritée de G : rotate_folder ne re-scelle pas les headers descendants) —
  la marche read/write est cohérente des deux côtés, mais un re-seal descendant
  serait la version incident-grade ; (c) moves = Circle only (self : structure
  scellée, autre passe) ; (d) `sections_under`/`resolve_*` restent positionnels
  (adresses courantes — c'est correct : l'index EST l'adresse du moment).
- Index/caches de query optimisés : post-F (le scan segmenté suffit) ; preuves de
  complétude pour mirrors : H.
- ~~Tests CLI : aucun~~ **SOLDÉ (2026-07-10)** : `rust/crates/aithos-cli/tests/
  cli_surface.rs` — 12 tests `assert_cmd`+`predicates`+`tempfile` sur le binaire
  réel, dans `cargo test --workspace`. Parcours critiques (init/éditions/tamper,
  mandat+lecture agent, budget épuisé, heartbeat 1s/1s suspend-reprend, profils
  de budget, fenêtres absolues, args scellés+audit+opacité disque) + les deux
  invariants de surface : kinds canoniques par verbe (et `--kind` rejeté par
  clap), aucun seed dans stdout/stderr/certs/log, inputs invalides fail-closed.
  Le checkpoint manuel (docs/CLI-GUIDE.md) reste, non bloquant.
- **Décision d'archi de déploiement (2026-07-10)** : la clé de l'agent est détenue
  par la CLI/le container, jamais par le LLM (qui produit des intentions, pas des
  signatures). Contrainte hors protocole (le core reste agnostique). Durcissement
  offline « no mislabeled effects » rangé en défense en profondeur pour H (additif,
  sans breaking wire) — voir plan.
- Merge entries / éditions concurrentes : I. Manifest à pins plats jusqu'à H.
- Artefact de récupération combiné (S ‖ succession) : couche présentation, plus tard.
- **G+ (dette assumée, non bloquante)** : (a) `log-show` (vue résumée) n'affiche
  pas `checks[]` — les reçus sont dans le payload, visibles au fichier brut et
  attestés par le test de surface ; (b) quorum M-of-N réservé dans la spec
  (`attestor` porte déjà le set ; un futur `quorum: k` transforme le OR en
  k-of-n) ; (c) Modèle 2 d'approbation (service attestant pour l'humain) =
  fallback futur, pas le MVP ; (d) le désugarage counter_sign matche le nom
  d'action sur TOUT connecteur du périmètre (le raccourci §4.4 ne porte pas de
  connecteur — une obligation déclarée `applies_to` fait le ciblage fin).
- Env sandbox G+ : `pip install pynacl base58 --break-system-packages` requis
  dans une VM fraîche pour les générateurs de vecteurs.

# Aithos Core — Handoff (reprise en contexte neuf)

**But.** Reprendre l'implémentation de référence d'aithos-core sans rien reperdre.
Résume où on en est, comment on travaille, et la prochaine étape exacte.

**Branche : `feat/f-gamma`** (étape F complète dessus, PAS encore mergée sur
master — décision Mathieu). Repo : `code/aithos-core/`.

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

## 4. État du code (7 étapes closes sur 12)

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
| F+ Contraintes avancées | ⏳ **prochaine** | spec d'abord (§04.4+§07.4) : active_windows, budgets[], container tier X — voir plan (note coordination levée : F est fini) |
| G Révocation | ⬜ | révocations = entrées gamma |
| H Merkle | ⬜ | racines zones + **racines gamma** (segments + trie mandate_id→count) |
| I Concurrence | ⬜ | merge disjoint, fork, entrées merge |
| K Intégration | ⬜ | scénario K, Docker, npm |

**Tests : 68 scénarios / 235 steps cucumber + vecteurs A1/A2/B2/C1/E1/F1/F2/F3,
tous verts ; `clippy --all-targets -- -D warnings` clean ; `cargo fmt` passé.**

CLI (nouveaux verbes F) : `grant-act` (mandat d'action, --max-actions,
--heartbeat-every/grace), `action` (--cert répétable, chaîne leaf-last),
`heartbeat`, `log-show`, `log-verify`, `log-query` (--kind/--action/--since/
--until/--folder/--tag/--mandate). `grant` logge désormais son entrée gamma.
Checkpoint manuel déroulé : 3 actions max_actions=3 → 4ᵉ `GammaBudgetExhausted` ;
heartbeat 10s/5s → silence 16s → `GammaHeartbeatStale` → beacon → reprise. ✔

## 5. Comment builder / tester

```
cargo test  --workspace --manifest-path rust/Cargo.toml
cargo clippy --workspace --manifest-path rust/Cargo.toml --all-targets -- -D warnings
python3 vectors/gen-f.py   # régénère F1-F3 (auto-check B2/E1 d'abord)
```

**Env sandbox (2026-07-10, après recyclage VM)** : les caches `/tmp` d'une session
précédente peuvent appartenir à `nobody` (illisibles). Setup qui marche :
- toolchain : `/tmp/rustup/toolchains/stable-aarch64-unknown-linux-gnu/bin` appelée
  EN DIRECT (rustup shim cassé sans settings) — exporter ce bin dans PATH.
- `CARGO_HOME=/tmp/cargo2` (copie de /tmp/cargo, sans bin/), `CARGO_INCREMENTAL=0`.
- `CARGO_TARGET_DIR=<repo>/rust/target-linux` (sur le volume Mac — la VM n'a pas
  la place ; `rust/target/` = artefacts macOS de Mathieu, NE PAS toucher).
- Suppressions de fichiers bloquées par défaut sur le montage Cowork → si
  "Operation not permitted" sur rm/unlink (cargo, git), demander le déblocage
  (outil allow_file_delete). `.git/*.lock` à rm avant commit.
- Process background tués entre appels shell → builds par tranches (`timeout 40`),
  les artefacts s'accumulent.
- `rust/target` a été PURGÉ du tracking git (7938 fichiers macOS committés par
  erreur depuis B ; l'historique garde le poids — réécriture = décision à part).
  `.gitignore` couvre `rust/target*/`.

## 6. Prochaine étape : F+ (spec d'abord), puis G

**F+** (voir `docs/EXECUTION-PLAN.md` § F+, discuté/écrit par Mathieu 2026-07-10) :
enrichissement purement additif du vocabulaire `constraints` — (1) `active_windows`
arithmétiques absolues (remplace active_hours, zéro TZ/DST), (2) `budgets:[profile]`
en OU logique (modèle/tokens/fenêtres par profil, comptage par `budget_ref`),
(3) enforcement modèle via container (tier X) + hook `attestation` (pont tier V).
Rituel : **graver la spec §04.4+§07.4 d'abord**, puis feature/vecteur/impl.
Le moteur de comptage de F (somme sur `authorized_via`, fenêtres glissantes,
filtre payload) se réutilise tel quel.

**G** ensuite : révocations = entrées gamma (kind `revoke` déjà dans l'enum),
rotation + re-scellement + up-link (§03.4), cascade, watchdog, move-as-rotation.

## 7. Points ouverts / dette assumée (non bloquants)

- Lecture gamma côté agent sur zone `self` : la walk des descripteurs avec clés
  d'agent n'est pas écrite (owner-only pour l'instant) — les scénarios couvrent circle.
- Wildcard `act.x.<c>.*` : le refus des actions classe `binding` attend les
  manifests connecteurs (§08.1) — TODO noté dans covers_act.
- Index/caches de query optimisés : post-F (le scan segmenté suffit) ; preuves de
  complétude pour mirrors : H.
- Merge entries / éditions concurrentes : I. Manifest à pins plats jusqu'à H.
- Artefact de récupération combiné (S ‖ succession) : couche présentation, plus tard.

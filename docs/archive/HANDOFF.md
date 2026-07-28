# Aithos Core — Handoff (reprise en contexte neuf)

> **ARCHIVE — handoff du 11 juillet 2026.** Le plan 0→K qu'il clôt reste une
> preuve historique ; les branches, compteurs de tests et prochaines étapes de
> ce document sont dépassés par CB13 et les chantiers Provider/Gateway. Ne pas
> l'utiliser comme prompt de reprise.

**But.** Reprendre l'implémentation de référence d'aithos-core sans rien reperdre.
Résume où on en est, comment on travaille, et la prochaine étape exacte.

> **ÉTAT EXPRESS (2026-07-11 soir, session K)** : **LE PLAN EST COMPLET —
> 14 étapes closes et committées, 0→K.** La session K a déroulé le rituel
> intégral (baseline verte, 4 décisions validées, bornage aithos-core-only,
> feature avant code) et livré : k-integration.feature (fil-rouge + bundle
> vécu, 191 scénarios / 766 steps zéro skip), **un trou réel soldé**
> (rotate_folder post-move — §6), bench §09.3 (9/10 cibles, l'écart
> documenté), image Docker FROM scratch **4,11 Mo** buildée+smokée, npm
> pack LOCAL @aithos/core@0.1.0 importé byte-exact, CONFORMANCE.md,
> checkpoint bloc 16 ✔. Prochaine étape : **décisions post-plan de
> Mathieu** (§6). ⚠️ Environnement inchangé : builds dans le conteneur
> CLOUD, git janitor sur le montage — tout est au §5, à lire AVANT d'agir.

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
- **Racines gamma §7.10 — 4 décisions gravées (Mathieu, 2026-07-11, round
  H2)** : (1) **wire additif + n par segment** — `gamma_roots: {"<YYYY-MM>":
  {root, n}}` + `gamma_counts_root` à côté de gamma_head (posture H1, hashes
  pré-H2 intouchés) ; le `n` committé borne l'énumération ; (2) **trie
  compteur complet** — `mandate_id → {entries, actions, children, budgets:
  {ref → {actions, tokens}}}`, zéros et maps vides OMIS ; `entries` (total
  toutes-kinds sous authorized_via) ajouté au round : c'est lui qui porte la
  complétude « toutes les entrées de M », pas seulement les actions ; tokens
  post-override (reçu atteste > déclaré) ; (3) **verifier-side cette
  passe** — publish committe, verify recompute et compare, check_budgets/
  check_action_append gardent le tally brut (l'appendeur détient le log) ;
  test croisé trie == tallys dans la suite ; (4) **proof wire v1 réutilisé
  tel quel** — segments en ordre de chaîne H_leaf(octets exacts de la
  ligne), feuilles du trie H_leaf(mandate_id ‖ 0x00 ‖ JCS) triées par id,
  absence = 2 feuilles ADJACENTES encadrant l'id (adjacence vérifiable des
  deux preuves seules : suffixe commun au-dessus de la divergence,
  cross-replay des siblings à la divergence, all-left/all-right en dessous),
  rims = all-right (première feuille) / all-left (dernière), trie vide =
  racine 32×0x00. Complétude : compte prouvé k + k inclusions distinctes.
  Sceller kind/via (bump `v`) : DIFFÉRÉ, conforme à la forward note.
- **Concurrence §2.6/§7.6 — 4 décisions gravées (Mathieu, round I) +
  décisions d'impl validées par la passe du 2026-07-11** : les 4 gravées =
  (1) merge manifest : `prev_hash` pointe le parent au plus petit hash
  d'édition + champ ADDITIF `merges: [a, b]` triés ; (2) index partagés =
  3-way par sid (base = ancêtre commun, rows changées de leur branche,
  ajouts unis, deletes tiennent, même sid changé des deux côtés = fork) ;
  (3) entrée gamma merge = `prev` (tip du parent min) + ADDITIF `prevs:
  [head_a, head_b]`, seule kind autorisée ; segment fusionné = préfixe
  commun, sous-chaîne LOW, sous-chaîne HIGH, entrée merge — octets jamais
  réécrits, monotonie `at` relâchée à la jointure, racines §7.10
  recommittées ; (4) portée = I1 + I2 complets (refus verifier du fork non
  résolu + résolution nearest common manager, autorité dir_covers, délégué
  borné, owner dernier recours). Les décisions D'IMPL prises en session
  (signalées, non re-validées une à une) : id d'entrée merge DÉTERMINISTE
  blake3(`aithos-core/v1/merge-entry-id` ‖ hash_lo ‖ hash_hi)→ULID ;
  snapshots d'index par édition `manifests/index-<zone>-<h>.json` (base du
  3-way ; merge exige un ancêtre publié post-I, sinon refus « missing
  snapshots ») ; slots `-alt` déterministes (HIGH/perdant) ; frontière =
  labels sans descendant changé, racine de zone induite, tag-views = nœuds
  propres (conservateur) ; fork genesis accepté par verify_links ;
  résolution d'un couple mergeable autorisée (l'autorité gate) ; manifest
  délégué accepté UNIQUEMENT porteur de `resolves_fork`. Convention pinnée
  par le vecteur : payload de l'entrée merge = `{"merges": [a, b]}` miroir
  du manifest ; prev = tip du parent au plus petit hash ; prevs =
  [tip_lo, tip_hi].

## 4. État du code (14 étapes closes et committées — 0→K : PLAN COMPLET)

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
| **H1 Merkle contenu** | ✅ | **SOLDÉ (2026-07-11)** : merkle.rs pur (h_leaf/h_node BLAKE3 domain-separated, mroot left-heavy, mroot_path, proof wire v1 node/wrap, verify_proof depuis les octets DÉCLARÉS — le splice meurt sur le domaine, deux sens), state.rs bundle (arbre recalculable des fichiers seuls : zones hiérarchiques avec headers foldés + tag views ancrées + wraps mroot'és, self plat, vault par chemin de storage ; prove_section/prove_self ; tree_diff par descente), manifest.roots À CÔTÉ des pins plats (serde default : chain hashes pré-H intouchés), sidecar d'arbre pinné par édition, verify recompare les racines. Vecteur H1 Python (gen-h.py auto-validé contre B2). 14 scénarios (dont move-as-rotation traqué par l'arbre). CLI prove / edition-diff |
| **H2 Racines gamma** | ✅ | **SOLDÉ (2026-07-11)** : spec §7.10 gravée (5d7ab89) ; gamma.rs section roots pure (segment_root en ordre de chaîne sur les octets EXACTS des lignes, counts_tally → GammaCounters {entries, actions, children, budgets} avec omission des zéros, counts_root, prove/verify count + entry + absence par adjacence triée + verify_complete_actions), manifest.gamma_roots {mois → {root, n}} + gamma_counts_root ADDITIFS (posture H1, serde default), log.rs gamma_state (publish committe, verify recompute et compare — GammaRootMismatch), 3 variants d'erreur fail-closed. Vecteur H2 Python (gen-h2.py, TRIPLE ancrage : B2 blake3, H1 conventions mroot byte-identiques, F2 = le segment 2026-07 EST les entrées committées de F2 et les tallies reproduisent ses expected). 12 scénarios. CLI log-prove (compte + absence) + test de surface |
| **I Concurrence** | ✅ | **SOLDÉE (2026-07-11, passe de vérification)** : core Entry.prevs additif (wire pré-I intact), check_form fail-closed (prevs = merge only, exactement 2, distincts, prev == prevs[0], payload.merges [lo, hi] croissants, pas de target), verify_links en marche de graphe (fork par extension d'un hash consommé — genesis inclus —, merge consomme exactement les DEUX tips ouverts, monotonie `at` relâchée à la jointure seulement, fin = un seul tip, doublon refusé), EditionFork/MergeRejected/ForkResolutionRejected ; bundle merge.rs (3-way par sid contre les snapshots d'index par édition, merge_segment_lines préfixe+LOW+HIGH, frontier() sur tree_diff, edition_merge à entrée d'id déterministe signée #content — sautée si le log n'a pas forké —, fork_check, resolve_fork autorité dir_covers AVANT toute écriture, verify_merge_edition/verify_resolution_edition sur sidecars authentifiés par les pins signés) ; manifest merges/resolves_fork/authorized_via additifs + ManifestSigner délégué ; publish_artifacts factorisé + snapshots d'index ; slots -alt ; section_rewrite/section_delete (circle ; delete = row seule, blob = crypto-erasure) ; MemStore: Clone ; 12 scénarios dé-taggés, vecteur i1_concurrency.rs byte-exact, CLI edition-merge + surface, bloc 15 ✔ — commits e1c22ce / 50d863e / 9d4ac58 / 96fd28c |
| **K Intégration** | ✅ | **SOLDÉE (2026-07-11, session K)** : k-integration.feature — fil-rouge absentee-owner (genèse → 4 zones → reader/gmail/social contraints → co-sign, args scellés+audit, budget épuisé, approbation → stale/beacon → délégation reader→helper + cut → move → revoke gmail+rotation (reader survit) → racines + preuve d'inclusion + count proof → fork 2 copies → merge → replay froid) + 3 scénarios bundle vécu (9 classes de tamper refusées, vue sans-clé/périmètres/un-taught, watchdog tue-sans-lire) ; glue ~45 defs + builder k_build_lived + copies fraîches ; fix revoke.rs (clé courante via ligne owner) ; bench criterion §09.3 ; Dockerfile pinné 1.96-alpine ; wasm RNG-free (chacha alloc-only, ulid no_std), wasm-opt off ; docker/npm-smoke.mjs ; docs/CONFORMANCE.md |

**Tests : 191 scénarios / 766 steps cucumber (zéro skip) + vecteurs
A→I1 + 17 tests de surface CLI, tous verts ; `clippy --workspace
--all-targets -- -D warnings` clean ; `cargo fmt` passé ; le check
`wasm32-unknown-unknown` re-passe (il était cassé au HEAD — voir §6) ;
gateway vérifié vert au même arbre (11 scénarios / 57 steps + e2e réseau —
la session parallèle a CLOS sa Phase B pendant la session K, interleaving
git propre, jamais touché ici).**

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
Checkpoint H2 déroulé (bloc 14 du guide) : publish → gamma_roots {2026-07 :
{root, n:3}} + counts_root au manifest ; log-prove --mandate →
{"entries":2,"actions":2} vérifié offline ; --absent id fantôme → prouvé ;
--absent id compté → refus fail-closed ; tamper de la dernière entrée →
edition-verify refuse (pin plat en première ligne ; la racine recalculée
diverge aussi — le scénario « caught by root recomputation alone » couvre le
cas mirror sans pins). ✔
Checkpoint I déroulé (bloc 15 du guide, 2026-07-11) : deux copies
divergentes → edition-merge → « merge edition published (height 4,
parents lo + hi) », prev_hash == merges[0], les DEUX écritures lisibles,
edition-verify + log-verify OK ; grant du même dossier des deux côtés →
« EditionFork: same-node conflict on circle:d/… », refus fail-closed. ✔

## 5. Comment builder / tester

```
cargo test  --workspace --manifest-path rust/Cargo.toml
cargo clippy --workspace --manifest-path rust/Cargo.toml --all-targets -- -D warnings
python3 vectors/gen-f.py   # régénère F1-F3 (auto-check B2/E1 d'abord)
```

**⚠️ MÉTHODE COURANTE (2026-07-11, passe de vérif I — l'environnement a
CHANGÉ, les recettes VM d'avant sont OBSOLÈTES)** :
- **La VM locale ne peut plus builder** : recyclée saine (disque OK) MAIS
  (a) egress réseau coupé — proxy deny-all, 403/timeout sur TOUT
  (static.rust-lang.org, crates.io, pypi, github) → aucune toolchain
  installable ; (b) **unlink/rmdir interdits sur le montage ET hors
  montage impossible à contourner** — l'outil `allow_cowork_file_delete`
  N'EXISTE PLUS. rename/mv (même par-dessus un fichier existant, même un
  dossier), hardlink, écriture-truncate : tous OK. Suppression voulue =
  `mv` vers `_to_delete/`.
- **Builds + tests : dans le conteneur CLOUD de la session** (x86_64,
  réseau crates.io/rustup OK, disque ~30G, pas de limite 45 s). Recette de
  la passe I, reproductible : rustup pinné **1.96.1** (parité clippy avec
  la suite — stable a pu bouger) minimal + clippy + rustfmt ;
  `git archive HEAD` sur la VM → tar → device_stage_files → arbre
  baseline ; tar des fichiers sales → arbre de travail ;
  `CARGO_INCREMENTAL=0`, un CARGO_TARGET_DIR PAR arbre (le partage
  provoque des collisions de cache inter-workspaces — vécu) ; suite
  complète ~2 min à froid. Retour : tar des fichiers modifiés →
  SendUserFile + device_commit_files → `cp` par-dessus (jamais tar -x
  direct : GNU tar UNLINK par défaut ; extraire dans un dossier neuf puis
  cp). **Croiser un sha256 global des fichiers des deux côtés avant de
  committer.** Différence assumée : validation x86_64 (code pur
  serde/blake3/dalek — sorties identiques par définition) ; les caches
  aarch64 `rust/cargo-linux` + `rust/target-linux` restent sur le volume,
  dormants, si l'egress VM revient un jour.
- **Git : sur le montage (la VM), sous PROTOCOLE JANITOR** — git marche
  SAUF ses unlinks de rollback : tout `git status` (et tout commit à vide)
  laisse un `index.lock`/`HEAD.lock` orphelin qui bloque l'op suivante
  (`File exists`). Protocole : `git config gc.auto 0` (une fois) ; avant
  CHAQUE commande git qui écrit, `mv .git/index.lock .git/HEAD.lock →
  _gitjunk/` s'ils existent ; JAMAIS de `git status` intercalé dans une
  séquence add/commit ; jamais `commit -a` ; jamais `git stash` (mise à
  l'écart par copie de fichiers à la place). add/commit qui RÉUSSISSENT ne
  laissent rien (le lock est renommé, pas unlinkés). Vérifié sur les 5
  commits de la passe I. Pour committer en tranches un fichier scindé :
  cp la version intermédiaire → `git add` → cp la version finale (l'index
  garde le snapshot) → `git commit` (sans -a).
- `rust/target/` = artefacts macOS de Mathieu, NE PAS toucher.
  `rust/target*` et `rust/cargo-linux/` gitignorés ; `rust/target` a été
  PURGÉ du tracking (l'historique garde le poids — réécriture = décision à
  part).
- Python (générateurs de vecteurs) : dans le conteneur cloud aussi
  (`pip install blake3 --break-system-packages` ; pynacl/base58 selon le
  générateur). La VM n'a plus pypi.
- Archéologie VM (si l'egress revient) : toolchain dans /tmp/rustup PATH
  direct, tranches -j1 ≤ 40 s + sync + clean de la crate déchirée, couple
  cucumber+gateway en -j2 sans clean, jamais `-p cucumber` standalone,
  nohup meurt en fin d'appel — voir l'historique git de ce fichier
  (commits ≤ 2460ee3) pour le détail.

## 6. Étape K close — le plan est complet ; prochaine étape : décisions post-plan

**K est close (2026-07-11, session K).** Rituel intégral : baseline verte au
HEAD 26b68fb (187/658, 17 CLI, A→I1, clippy/fmt — sha256 croisés) → lecture
spec/09 + plan §K → **4 décisions validées par Mathieu** : (1) scénario K =
UN fil-rouge unique ; (2) table §09.3 adoptée telle quelle (criterion, 1M en
synthétique in-memory, résultats consignés, pas de gate CI) ; (3) npm =
packaging SEUL à 0.1.0, pack local, AUCUNE publication, nom @aithos/core du
plan ; (4) image = build+smoke dans le cloud, rien livré. + **bornage
gravé** (à la demande de Mathieu, commit 634f74b) : tout reste dans
aithos-core, aucun élément d'un projet Aithos antérieur, README/plan/wasm
metadata amendés. Feature v2 (4 scénarios : fil-rouge + tampers + sans-clé +
watchdog) validée et committée @wip AVANT le code (be6326e).

**Ce que l'impl a trouvé (et la valeur du scénario K)** :
- **Un trou réel, soldé** : `rotate_folder` calculait la « vieille » clé du
  dossier par DÉRIVATION pure depuis la racine de zone — faux pour un
  dossier DÉPLACÉ (sa clé v2 est fraîche, scellée au header, plus
  dérivable) → `SealRejected` à toute rotation post-move. Même classe que
  le trou `section_add` soldé en G. Fix minimal dans revoke.rs : la clé
  courante s'ouvre par la LIGNE OWNER du header (open_latest), capturée
  avant la rotation. G2 intact (ligne owner d'un dossier jamais déplacé =
  la même clé que la dérivation). Exercé par K1 ET par le checkpoint CLI.
- **Le check wasm32 était cassé au HEAD** (personne ne le lançait : la CI
  n'a jamais tourné — pas de remote). Cause : getrandom entraîné dans le
  graphe wasm par les default-features de chacha20poly1305 (getrandom 0.2)
  et ulid (std→rand→getrandom 0.3). Fix fidèle au « core pur, zéro RNG » :
  `chacha20poly1305 default-features=false features=["alloc"]`, `ulid
  default-features=false` (no_std ; nos usages = from(u128)/Display/
  FromStr). Personne (gateway inclus) ne tirait ces RNG. Le check re-passe.
- **Retouches de phrasé de la feature pendant l'impl** (~10 lignes,
  mécaniques) : les steps réutilisés de G+/I portent des TEMPS FIGÉS
  (GP_AT=07-04, I_*=07-10) antérieurs au tip du bundle vécu (monotonie
  `at`), des chemins pré-move (« projets/note1 »), ou des assertions
  incompatibles (beacon_rejected exige zéro beacon valide ; le no-leak
  exigeait tout scellé alors que PUBLIC est clair par design → variante
  keyed-zone). Timeline K : kd(n)=jour 9+n, tout en juillet (un segment,
  pas de fork trans-mois), sondes post-vécu ≤ 30/07 (NA30).
- Le vault d'audit (§7.9.3) se pose au GRANT (grant_audit_line) — la glue
  gmail le fait, comme le Given F+.

**Livrables K** : bench criterion (`benches/perf.rs`, dev-dep workspace) —
9/10 cibles avec 6×–230× de marge, **l'écart : gamma_full_verify 10k =
538 ms vs < 200 ms** sur le conteneur 2 vCPU (≈ 50 µs/verify Ed25519 ; un
laptop M-series est au voisinage de la cible) — options documentées dans
CONFORMANCE (batch-verify dalek ~2-3× additif, ou scan parallèle), décision
Mathieu ; image **4,11 Mo** (< 15 Mo) buildée ET smokée dans le cloud
(dockerd FONCTIONNE dans le conteneur de session ; pour builder DANS docker
il faut injecter le CA du proxy egress — Dockerfile.cloud temporaire, non
committé ; le Dockerfile canonique pinné rust:1.96-alpine reste
auto-suffisant sur toute machine normale) ; npm : wasm-pack --target nodejs
(wasm-opt OFF par metadata — le binaryen des distros est trop vieux pour le
wasm de rustc 1.96), nom patché @aithos/core au pack (wasm-pack ne scope
pas librement), tarball local installé dans un projet neuf, import node 22,
genesis_pubkeys **byte-exact vs A1** (docker/npm-smoke.mjs) ;
docs/CONFORMANCE.md (niveaux §09.4 revendiqués, table §09.2 avec 4 dettes
de promotion explicites, table §09.3 mesurée, packaging) ; CLI-GUIDE bloc
16 ✔ (~0,37 s de travail effectif).

**Prochaine étape — décisions post-plan (Mathieu)** :
1. gamma-verify 10k : batch-verify / parallèle / caveat machine ?
2. merge `feat/obligations` ↔ `feat/gateway` (les deux chantiers sont
   verts sur la même branche physique — le merge de BRANCHES est sa
   décision historique) ;
3. passes différées : wasm « Core reader » (§09.4), vecteurs de promotion
   (4 lignes §09.2), re-seal descendant post-move, tag-views sur dossier
   déplacé, publication npm/registry le jour venu ;
4. promotion de la spec (les DRAFT → normatif) quand il en décidera.

## 6bis. Étape précédente : G+ — Obligations

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

**H — Merkle : ENTIÈREMENT close** (H1 contenu + H2 racines gamma, toutes deux
2026-07-11, voir tableau §4). Reste de l'idée H : le durcissement offline « no
mislabeled effects » demeure rangé en défense en profondeur, différé (additif,
sans breaking wire — voir plan §H).

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
- ~~Merge entries / éditions concurrentes : I~~ **SOLDÉ (2026-07-11)** — voir tableau §4.
- Artefact de récupération combiné (S ‖ succession) : couche présentation, plus tard.
- **G+ (dette assumée, non bloquante)** : (a) `log-show` (vue résumée) n'affiche
  pas `checks[]` — les reçus sont dans le payload, visibles au fichier brut et
  attestés par le test de surface ; (b) quorum M-of-N réservé dans la spec
  (`attestor` porte déjà le set ; un futur `quorum: k` transforme le OR en
  k-of-n) ; (c) Modèle 2 d'approbation (service attestant pour l'humain) =
  fallback futur, pas le MVP ; (d) le désugarage counter_sign matche le nom
  d'action sur TOUT connecteur du périmètre (le raccourci §4.4 ne porte pas de
  connecteur — une obligation déclarée `applies_to` fait le ciblage fin).
- Env sandbox G+ : `pip install pynacl base58 blake3 --break-system-packages`
  requis dans une VM fraîche pour les générateurs de vecteurs.
- **H1 (dette assumée, non bloquante)** : (a) headers self NON foldés dans
  les feuilles self (un vérificateur externe ne peut pas mapper hdr files ↔
  rows sans les descripteurs scellés) — les pins plats couvrent ; passe
  dédiée si un jour les grants self se multiplient ; (b) le sidecar d'arbre
  (`manifests/tree-<h>.json`) est un cache pinné, pas du wire signé — la
  vérité reste les racines du manifest ; (c) `prove` CLI couvre les sections
  (public/circle) et les blobs self — pas encore les folders/tag views en
  cible directe (leur header est attesté transitivement par la preuve d'une
  section dessous) ; (d) le diff rapporte les labels sid-based, la
  résolution en chemins d'affichage est côté outil.
- **H2 (dette assumée, non bloquante)** : (a) `verify_complete_actions`
  couvre les ACTIONS (le mètre §7.4) ; « tout kind sous M » a son compte
  committé (`entries`) mais pas encore de verifier composé — un client le
  compose des mêmes briques (compte prouvé + inclusions) ; (b) fenêtres
  ROULANTES (max_actions_per, rate_limit) volontairement hors trie — un
  compteur statique ne porte pas une fenêtre glissante ; les scans de
  segments restent bornés par root+n (documenté §7.10) ; (c) CLI log-prove
  = compte + absence ; les preuves d'inclusion d'ENTRÉE et les réponses de
  complétude sont des APIs core (prove_entry, verify_complete_actions) sans
  surface CLI dédiée ; (d) sceller kind/via = bump `v`, différé (décision
  du round) ; (e) ~~cohabitation racines de segment / entrées merge~~
  réglée en I : segment fusionné = préfixe + LOW + HIGH + entrée merge,
  octets jamais réécrits, racines recommittées sur le layout fusionné.
- **K (dette assumée, non bloquante)** : (a) 4 lignes de §09.2 couvertes
  par scénarios mais sans vecteurs JSON gelés (signature policy §02.11,
  tag re-label, kex-mismatch, révocation non autorisée / sous-mandat trop
  large) — listées dans CONFORMANCE, dues « at promotion » ; (b)
  gamma_full_verify 10k au-dessus de la cible sur 2 vCPU (décision batch /
  parallèle / caveat en attente) ; (c) npm = stub de surface (genesis
  seule) — l'extension « Core reader » wasm est une passe dédiée ; (d)
  wasm-opt désactivé par metadata (réactivable avec un binaryen récent) ;
  (e) rotation post-move : l'up-link wrap est re-posté via la RACINE de
  zone (comportement historique) — pour un dossier déplacé, le wrap via le
  NOUVEAU parent posté par le move devient stale à la rotation suivante :
  fail-closed (jamais d'over-grant), passe dédiée si des holders de
  sous-arbre parent apparaissent ; (f) le checkpoint CLI ne couvre pas la
  délégation agent→helper (pas de commande `delegate` — la feature K1 la
  couvre côté bibliothèque).
- **I (dette assumée, non bloquante)** : (a) `section_delete` ne supprime
  pas le blob (Store sans delete ; effacement cryptographique §06) — la
  row disparaît, le blob reste pinné orphelin ; (b) publication déléguée
  GÉNÉRALE d'un manifest = passe future (un délégué n'est accepté que
  porteur de `resolves_fork`) ; (c) l'id de l'entrée merge du VECTEUR est
  un id fixture — l'id déterministe blake3→ULID est exercé par le
  scénario « two mergers produce identical bytes », pas par le vecteur ;
  (d) tag-views = nœuds propres dans la frontière (conservateur : deux
  branches touchant la même vue forkent même si les sections diffèrent) ;
  (e) merge/resolve = zones à index (public/circle/self) ; le vault
  (chemins de storage, pins plats) suit l'union fail-closed des fichiers
  non-index ; (f) les caches VM aarch64 (`rust/cargo-linux`,
  `rust/target-linux*`) dorment sur le volume — inutilisés depuis le
  passage aux builds cloud (§5), à purger via `_to_delete/` si la place
  manque un jour.

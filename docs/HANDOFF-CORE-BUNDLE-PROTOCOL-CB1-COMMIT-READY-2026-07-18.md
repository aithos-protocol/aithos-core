# Handoff — protocole Core + Bundle, CB1 prêt au commit et non commité

**Date :** 2026-07-18

**Dépôt :** `/Volumes/Math17/aithos/v2/code/aithos-core`

**Branche observée :** `feat/obligations` — ne pas la changer

**HEAD observé :**

`7349cf62f98c39ee03bfef1ed3ca0616a76485dc`

`feat(provider): piste P — crate aithos-provider P1→P6/M2, vecteurs p1..p6, gate 2026-07-18`

**Statut :** CB0 terminé ; CB1 validé, corrigé après contre-audit et prêt au
commit ; index vide ; aucun commit CB1 ; aucun travail CB2 commencé.

**Prochaine frontière :** autorisation explicite du staging nominatif et du commit
contractuel CB1. Le présent handoff n'accorde pas cette autorisation.

**Aucun push, merge, changement de branche ou déploiement n'est demandé.**

---

## 0. Autorité et portée

Lire entièrement, dans cet ordre :

1. le présent handoff ;
2. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-CB1-VALIDATED-2026-07-18.md` ;
3. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md` ;
4. `docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md` ;
5. `docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md` ;
6. `docs/NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md` ;
7. `README.md` et les rituels BDD, vectors-first et pure-core du dépôt.

Le handoff CB1 validé reste la source opposable du contrat. Le présent document
actualise uniquement son état après le gate de reprise et les deux clarifications
autorisées par Mathieu.

Les décisions D1–D9, T1–T3, G-A–G-E, le confinement des chemins, le comptage
logique dédupliqué et le versioning indépendant des nouveaux compteurs restent
acquis. Ne pas les rouvrir ou les réinterpréter.

Ce document ne vaut pas :

- autorisation de `git add` ou `git commit` ;
- autorisation de créer un handoff `CB1-DONE` ;
- autorisation de commencer CB2 ;
- attribution de `vectors/README.md`, Cargo, Provider ou d'une surface aval ;
- autorisation de push, merge, switch ou déploiement.

Le présent handoff et son prompt de reprise sont hors du futur commit CB1.

---

## 1. Clarifications finales autorisées

Le gate read-only a relevé deux formulations ambiguës. Mathieu a autorisé leur
clarification le 2026-07-18.

### 1.1 Secret upstream et transaction vault

`features/o-connector-classes-vault.feature` exprime désormais :

```text
local vault update after an out-of-protocol upstream secret replacement
```

Le remplacement du secret upstream reste hors protocole. Seule la mise à jour
locale du vault Aithos, avec Gamma, preuves config, roots et publication, appartient
à la transaction Core + Bundle. Aucun effet connecteur réel n'est introduit.

### 1.2 Schémas de compteurs historiques

`features/h2-gamma-roots.feature` précise désormais que :

- les artefacts historiques restent byte-identiques et vérifiables sous leur
  version historique ;
- seul du nouveau matériau de compteur injecté sous un schéma ancien ou non
  versionné, ou sous une version de schéma inconnue, échoue fermé ;
- `max_actions`, les kinds Gamma et les count roots historiques ne sont jamais
  réinterprétés.

Une contre-relecture sémantique indépendante a confirmé que ces deux écarts sont
levés, sans nouveau wire, effet, code, implémentation de step Cucumber ou vecteur.

---

## 2. État Git et ownership à préserver

### 2.1 Branche, HEAD et index

- Branche : `feat/obligations`.
- HEAD : `7349cf62f98c39ee03bfef1ed3ca0616a76485dc`.
- Ce HEAD est le commit Provider concurrent légitime déjà décrit dans le handoff
  précédent.
- Son parent direct est
  `cda4f058708a5a43c5b21870bf0e1bce925d74e1`.
- L'intersection entre les 54 fichiers du commit Provider et les 19 cibles CB1 est
  vide.
- L'index est vide.
- `git diff --check` et `git diff --cached --check` sont propres.

Ne pas revenir au HEAD historique et ne pas modifier la branche.

### 2.2 Worktree étranger

Le dépôt partagé contient toujours des fichiers non suivis appartenant à Mathieu ou
à d'autres pistes, notamment :

- `_gitjunk/**`
- `_to_delete/**`
- `_transfer/**`
- de nombreux documents sous `docs/**`

Après création du présent handoff et de son prompt, 106 fichiers non suivis hors
CB1 sont présents :

- 52 sous `_gitjunk/**` ;
- 12 sous `_to_delete/**` ;
- 20 sous `_transfer/**` ;
- 22 sous `docs/**`, dont le présent handoff et son prompt.

Ne rien nettoyer, déplacer, restaurer, supprimer ou capturer. Ne jamais utiliser
`git add .`, `git add -A` ou le staging d'un répertoire.

Restent hors ownership Core + Bundle :

- `rust/crates/aithos-provider/**`
- `rust/Cargo.toml`
- `rust/Cargo.lock`
- `vectors/README.md`
- les vecteurs et documents Provider
- Gateway, CLI/WASM, client, RemoteStore et SDK réseau

---

## 3. Fichiers exacts du futur commit CB1

### Specs — 5

- `spec/01-identity-and-keys.md`
- `spec/02-content-tree.md`
- `spec/04-mandates.md`
- `spec/05-delegation.md`
- `spec/08-connectors.md`

### Features suivies modifiées — 11

- `features/d-bundle.feature`
- `features/e-mandate-sections.feature`
- `features/e-mandates.feature`
- `features/f-gamma.feature`
- `features/f-plus-constraints.feature`
- `features/g-plus-obligations.feature`
- `features/g-revocation.feature`
- `features/h2-gamma-roots.feature`
- `features/i-concurrency.feature`
- `features/k-integration.feature`
- `features/l-delegated-writes.feature`

### Features nouvelles — 3

- `features/m-delegated-editions.feature`
- `features/n-structural-mutations.feature`
- `features/o-connector-classes-vault.feature`

Total : exactement 19 fichiers.

État statistique avant staging :

- 16 fichiers suivis : `1252 insertions(+), 69 deletions(-)` ;
- nouvelles features : 101 + 100 + 176 = 377 lignes ;
- projection arithmétique du futur diff indexé :
  19 fichiers, 1629 insertions et 69 suppressions, à confirmer après staging.

Aucun changement de mode ou de type de fichier.

Exclusions explicites :

- ce handoff et son prompt ;
- tous les autres documents non suivis ;
- `_gitjunk`, `_to_delete`, `_transfer` ;
- tout Rust, Cargo, Provider, vecteur, Gateway, CLI/WASM, client ou SDK.

---

## 4. Preuves finales du gate CB1

### 4.1 Inventaire Gherkin

Référence : HEAD `7349cf62f98c39ee03bfef1ed3ca0616a76485dc`.

- Baseline : 219 déclarations / 9 `@wip`.
- Features suivies : 51 nouvelles déclarations / 51 nouveaux `@wip`.
- Nouvelles features `m/n/o` :
  - `m` : 9/9 ;
  - `n` : 7/7 ;
  - `o` : 15/15 ;
  - total : 31 déclarations / 31 `@wip`.
- Delta CB1 : 82 déclarations / 82 `@wip`.
- État final :
  - 301 déclarations ;
  - 256 `Scenario` ;
  - 45 `Scenario Outline` ;
  - 91 `@wip`.

Chaque nouvelle déclaration possède un `@wip` sur la ligne immédiatement
précédente.

La comparaison au HEAD confirme :

- aucun scénario historique supprimé ou modifié ;
- aucun step historique supprimé ou modifié ;
- aucun tag historique supprimé ou retaggé ;
- les rares suppressions dans le diff des features ne concernent que du texte
  descriptif redliné.

### 4.2 Confinement

- Les seules cibles CB1 sont les 19 fichiers de la section 3.
- Aucun statut ou diff CB1 sous `rust/**` ou `vectors/**`.
- Aucun Rust, aucune implémentation de step Cucumber, aucun vecteur, oracle,
  fixture ou wire post-CB1.
- Aucun fichier Provider, Gateway, CLI/WASM, client ou SDK.
- Aucun chevauchement avec le commit Provider courant.

### 4.3 Validation Bundle

Commande rejouée depuis `rust/` :

```bash
CARGO_TARGET_DIR=/Volumes/Math17/aithos/v2/.codex-targets/core-bundle-cb0-20260718 \
CARGO_INCREMENTAL=0 \
cargo test -p aithos-bundle --locked
```

Résultat :

- exit 0 ;
- 14 features ;
- 65 rules ;
- 229 scénarios verts ;
- 906 steps verts ;
- test unitaire `MemStore` vert ;
- 4/4 tests I1 concurrence verts ;
- doc-tests verts.

---

## 5. Ce qui reste à faire sur le protocole

Le protocole Core + Bundle n'est pas terminé avec CB1. CB1 fixe uniquement le
contrat de complétude.

| Lot | Travail restant et gate |
|---|---|
| Commit CB1 | Stager nominativement les 19 fichiers, présenter le diff indexé, créer le commit contractuel isolé, relever le hash, puis STOP. |
| CB2 | Autorisation séparée. Oracles indépendants → vecteurs → tests rouges observés pour la bonne raison → commit vecteurs/tests distinct. Aucun Rust d'implémentation. |
| CB3 | Forme canonique, `id=`, containment D1, lattice D2 et validation T3 dans Core, sans changer les octets historiques. |
| CB4 | Opération canonique et verdict Core pur unique : possession, chaîne, révocation, périmètre, contraintes, preuves et compteurs. Aucun helper partiel ne produit `Allow`. |
| CB5 | Contraintes complètes : T1/T2/G-E, matrice d'applicabilité, compteurs mutation/total versionnés, catalogue et classes connecteurs. Aucun nouveau wire sans vecteurs. |
| CB6 | Rejeu Gamma sémantique complet contre le préfixe historique ; mêmes verdicts, compteurs et état append-time/cold-time. |
| CB7 | Transaction Bundle G-B/G-C : snapshot/overlay, verdict Core, write-set déterministe, point logique de linéarisation et récupération `FsStore` sans état mixte. |
| CB8 | Parité owner et grants génériques, lignes exactes par zone/SID/vault, aucune divergence entre certificat et clés livrées. |
| CB9 | Mutations déléguées complètes `public/circle/self`, lectures, Gamma, authorship et refus atomiques. |
| CB10 | Structure, révocation/rotation et vault isolé `/x/<connector>` avec `.config` exact ; aucun effet connecteur réel. |
| CB11 | Changesets dérivés et éditions normales owner/grantee single-actor/single-chain, sans changement parasite. |
| CB12 | Paquet de publication local et façade keyless Bundle→Core ; export vers store vierge, retrait des capacités privées et cold verify réel. |
| CB13 | Concurrence, forks/merges/résolutions et gate final Core + Bundle : aucun `@wip` pertinent, parité append/replay et fresh-store complète. |

Chaque lot respecte :

```text
contrat validé
→ vecteur indépendant si bytes signés
→ test rouge pour la raison attendue
→ TDD dans le crate propriétaire
→ retrait des seuls @wip réellement verts
→ intégration locale réelle
→ fmt/clippy/tests/workspace/WASM
→ commit étroit
→ gate avant le lot suivant
```

Le Provider protocolaire ne reprend qu'après CB13 vert, contrats et vecteurs
committés, paquet/façade keyless stabilisés, reason codes et faits CAS définis, cold
verify depuis un store vierge réussi et ownership Provider réattribué.

Après CB13, la reprise Provider reste mécanique :

```text
façade keyless Bundle
→ mapping du verdict
→ stockage opaque
→ CAS durable
→ witness/head canonique
→ vrai HTTP avec restart
→ téléchargement dans un nouveau store
→ cold verify
```

Les mutations `aithos-client` restent postérieures au gate protocole. Le SDK réseau
reste hors de ce dépôt.

---

## 6. Gate immédiat du commit CB1

### Sans autorisation explicite

1. vérifier read-only branche, HEAD, statut, index et ownership ;
2. vérifier les 19 cibles et les compteurs ;
3. présenter le gate ;
4. demander l'autorisation ;
5. STOP sans `git add`.

### Avec autorisation explicite limitée au commit CB1

1. relever à nouveau branche, HEAD et index dans le worktree partagé ;
2. stager chaque fichier de la section 3 par son nom exact ;
3. exécuter :

```bash
git diff --cached --check
git diff --cached --name-status
git diff --cached --stat
```

4. exiger exactement les 19 fichiers ;
5. inspecter et présenter le diff indexé complet ;
6. committer avec le message proposé :

```text
test(protocol): add core-bundle CB1 completeness contracts @wip
```

7. relever le hash, le subject, la liste des 19 fichiers et les compteurs ;
8. présenter le worktree résiduel appartenant aux autres pistes ;
9. STOP avant tout document `CB1-DONE` et avant CB2.

Phrase d'autorisation suffisante :

```text
J'autorise explicitement le staging des 19 fichiers contractuels CB1 nommés dans
le handoff, la présentation de leur diff indexé, puis leur commit isolé. Aucun
autre fichier et aucun travail CB2 ne sont autorisés.
```

---

## 7. Blocages et conditions d'arrêt

STOP et demander une décision si :

- la branche n'est plus `feat/obligations` ;
- HEAD a avancé et chevauche une des 19 cibles ;
- l'index n'est plus vide et son contenu n'est pas attribué ;
- une cible contient un changement étranger au présent handoff ;
- le nombre de fichiers, déclarations ou `@wip` diffère ;
- le diff indexé contient autre chose que les 19 fichiers ;
- un test exige de modifier Rust, Cargo, Provider, vecteurs ou une surface aval ;
- une action CB2 exige de modifier `vectors/README.md`, Cargo/lock ou un fichier
  chevauché sans attribution.

Interdictions permanentes :

- aucun push, merge, switch, déploiement, clean, reset, restore ou déplacement de
  travaux existants ;
- aucun nouveau champ signé, kind Gamma, compteur ou migration sans Gherkin validé
  et vecteur indépendant ;
- aucune règle Core copiée dans Bundle, Provider, Gateway ou une surface ;
- aucun faux CAS Provider ou faux E2E réseau ;
- aucune clé, DK, credential ou donnée protégée dans un artefact public ;
- aucun démarrage CB2 dans le gate de commit CB1.

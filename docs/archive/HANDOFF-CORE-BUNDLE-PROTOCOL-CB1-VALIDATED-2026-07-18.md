# Handoff — protocole Core + Bundle, CB1 validé et non commité

> **ARCHIVE — étape CB1 dépassée.** Conservé comme preuve de validation ; l'état
> courant est post-CB13.

**Date :** 2026-07-18

**Dépôt :** `/Volumes/Math17/aithos/v2/code/aithos-core`

**Branche observée :** `feat/obligations` — ne pas la changer

**HEAD courant observé :**

`7349cf62f98c39ee03bfef1ed3ca0616a76485dc`

`feat(provider): piste P — crate aithos-provider P1→P6/M2, vecteurs p1..p6, gate 2026-07-18`

**Statut :** CB0 terminé ; contrats CB1 validés humainement et intégrés au
worktree ; index vide ; aucun commit CB1 ; aucun vecteur ou code post-CB1 commencé.

**Prochaine frontière :** autorisation explicite du staging et du commit
contractuel CB1. CB2 reste interdit avant ce commit et une nouvelle autorisation.

---

## 0. Autorité et portée de ce handoff

Ce document complète, sans les modifier :

1. `docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md`
2. `docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md`
3. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md`
4. `docs/NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md`

Le plan d'action reste l'autorité de séquencement CB0→CB13. Le présent handoff
remplace seulement son état historique CB0/CB1 par l'état réellement observé et
validé ci-dessous.

Il ne vaut pas :

- autorisation de stage ou commit ;
- autorisation de commencer CB2 ;
- attribution de `vectors/README.md`, Cargo ou d'un fichier Provider ;
- autorisation de push, merge, changement de branche ou déploiement.

Les deux documents de passation créés avec ce handoff ne font pas partie du futur
commit contractuel CB1.

---

## 1. Validation humaine opposable

Après présentation de CB0, de la matrice Lot 0, du diff CB1, des contradictions
d'implémentation et des recommandations, Mathieu a validé le 2026-07-18 :

> Je valide G-A à G-E, le confinement des chemins, le comptage logique dédupliqué
> des publications et le principe de versioning des compteurs, selon la proposition
> consolidée. J'autorise leur intégration dans les redlines CB1, mais pas encore le
> travail post-CB1.

Les décisions D1–D9 et T1–T3 du handoff Lot 1 restent également opposables. Elles
ne doivent pas être rouvertes ou réinterprétées par la session suivante.

La validation a été intégrée dans les specs comme décision datée, et non comme
proposition encore « pending ».

---

## 2. État réel du worktree à préserver

### 2.1 Branche et HEAD

- Branche initiale et courante : `feat/obligations`.
- HEAD au début de CB0 :
  `cda4f058708a5a43c5b21870bf0e1bce925d74e1`.
- Pendant l'audit read-only, une piste Provider concurrente a créé le commit
  `7349cf62f98c39ee03bfef1ed3ca0616a76485dc`.
- La session Core + Bundle n'a ni créé, ni stagé, ni amendé ce commit.
- Le plan d'action conserve `cda4f058…` comme HEAD historique ; ne pas tenter d'y
  revenir.

### 2.2 Ownership

Le dépôt contient de nombreux éléments non suivis appartenant à Mathieu ou à
d'autres pistes :

- `_gitjunk/**`
- `_to_delete/**`
- `_transfer/**`
- handoffs et prompts historiques sous `docs/**`
- travaux Provider déjà intégrés au HEAD ou encore présents dans leur arbre

Ne rien nettoyer, déplacer, restaurer, supprimer ou incorporer par défaut.

Sont toujours hors ownership Core + Bundle sans attribution explicite :

- `rust/crates/aithos-provider/**`
- `rust/Cargo.toml`
- `rust/Cargo.lock`
- `vectors/README.md`
- vecteurs et documents appartenant à la piste Provider
- Gateway, CLI, WASM, client, RemoteStore et SDK réseau

### 2.3 Index

L'index Git est vide à la clôture de cette session. Aucun fichier CB1 ou document de
passation n'est stagé.

---

## 3. CB0 terminé

CB0 a établi :

- branche, HEAD, worktree et ownership ;
- inventaire des contrats et de l'implémentation ;
- matrice Lot 0 actualisée dans le rapport de revue, sans modifier le handoff
  existant non suivi ;
- baseline Core, Bundle, workspace et WASM ;
- contradictions d'implémentation réelles.

Le premier `cargo test --workspace --locked` a échoué uniquement par `ENOSPC` sur
le volume temporaire système. Le même run a réussi avec :

`CARGO_TARGET_DIR=/Volumes/Math17/aithos/v2/.codex-targets/core-bundle-cb0-20260718`

Baseline observée :

| Gate | Résultat |
|---|---|
| `cargo test -p aithos-core --locked` | vert |
| `cargo test -p aithos-bundle --locked` | vert |
| Cucumber Bundle | 14 features, 65 rules, 229 scénarios, 906 steps — verts |
| tests I1 concurrence | 4/4 verts |
| `cargo test --workspace --locked` | vert avec target isolé |
| `cargo fmt --all --check` | vert |
| Clippy Core + Bundle `-D warnings` | vert |
| Clippy workspace `-D warnings` | vert |
| check `aithos-wasm` `wasm32-unknown-unknown` | vert |

---

## 4. CB1 validé

### 4.1 Inventaire

- 219 → 301 déclarations Gherkin.
- 9 → 91 tags `@wip`.
- 82 nouvelles déclarations :
  - 51 dans des features suivies ;
  - 31 dans `m`, `n`, `o`.
- Chaque nouvelle déclaration possède un `@wip` local immédiat.
- 256 `Scenario` et 45 `Scenario Outline` au total.
- Aucun scénario, step ou tag historique supprimé ou retaggé.
- Aucun nouveau kind Gamma publication.
- Aucun champ, hash, signature, algorithme ou version wire implicite.
- Aucun Rust, aucune implémentation de step Cucumber, aucun vecteur, Provider,
  Gateway, CLI/WASM ou client modifié.

Le runner actif reste volontairement à 229 scénarios / 906 steps, puisque les 82
nouveaux contrats restent `@wip`.

### 4.2 Décisions consolidées

#### G-A — `.config`

Dans la version courante, `act.x.<id>.config` est une capacité vault exacte,
indivisible, extérieure au catalogue métier `read/act/binding`.

- Elle couvre read/create/edit/delete de la config de ce connecteur.
- Aucun wildcard ne la couvre.
- Elle n'hérite d'aucun `co_sign` implicite.
- Toutes les contraintes et obligations applicables de la chaîne se conjoignent.
- Un split read/write futur exige un contrat versionné, des règles de migration et
  des vecteurs indépendants ; il ne réinterprète jamais les mandats actuels.
- Les colonnes `Cfg-R`/`Cfg-M` de la matrice sont analytiques, pas deux droits v1.

#### G-B — transaction Store

Une transaction possède un point logique de linéarisation après verdict Core.

- Avant ce point, rejet ou panne laisse l'état canonique byte-for-byte inchangé.
- `MemStore` remplace un état complet.
- `FsStore` prépare hors du bundle et utilise un mécanisme de linéarisation local
  au Store.
- Génération, marker ou référence internes restent hors namespace, layout,
  manifeste, pins et wire signés.
- Reopen/récupération observe l'ancien ou le nouvel état complet, jamais un mélange.
- Un crash ou accusé perdu au point de linéarisation peut exiger de découvrir
  l'issue depuis le manifest/head canonique.
- L'unique exception d'orphelin reste le préchargement D3 explicite de blobs opaques
  content-addressed non référencés, hors transaction locale.

#### G-C — capacités cryptographiques

Les API stables reçoivent des capacités opaques, étroites, typées par but et liées
au contexte.

- Pas de `sign(bytes)`, decrypt-bytes ou wrap-bytes générique public.
- Le sujet, domaine, Ethos, acteur et, selon le cas, node, version et destinataire
  sont liés avant la cryptographie.
- Une capacité d'une classe d'artefact ne se substitue pas à une autre.
- Aucun seed ou secret brut n'est exigé ou retourné lorsque la capacité suffit.
- Une session locale porte un Ethos, un acteur et, pour un grantee, une chaîne.

#### G-D — façade keyless

- Bundle vérifie layout, versions, hashes, références, atteignabilité et forme des
  preuves.
- Bundle transmet des artefacts publics typés à un verdict Core pur.
- Append-time et cold-time appellent la même sémantique.
- Le Provider futur appelle cette façade puis ne fait que stockage opaque,
  transport et CAS ; il ne réimplémente aucune règle.

#### G-E — extension inconnue

- Une extension inconnue peut être parsée et préservée uniquement sur un root qui
  est aussi feuille.
- Elle interdit la sous-délégation.
- Le wire actuel ne porte aucune enveloppe d'applicabilité comprise par Core :
  toute consommation refuse avec une décision typée « extension non comprise ».
- La déclaration du grantee ou une enveloppe future inconnue ne change pas ce refus.
- Le refus ne produit ni Gamma, ni état canonique, ni compteur.
- Une version future ne peut permettre la non-applicabilité qu'avec enveloppe
  signée versionnée, lois d'applicabilité/atténuation/enforcement et vecteurs
  indépendants ; les bytes existants ne sont jamais réinterprétés.

#### Confinement des chemins

- Display paths : grammaire humaine de la spec.
- Store keys : layout canonique exact.
- Rejet absolu, dot, segment vide, traversal et objet hors layout.
- `FsStore` ancre sa racine canonique et refuse toute indirection filesystem qui
  sortirait de cette racine, y compris au cold load et en récupération.
- Un manifeste signé ne légitime jamais un escape ou objet hors layout.

#### Publication et compteurs

- Les compteurs comptent des consommations logiques, pas des artefacts.
- Une publication/merge/résolution grantee consomme une unité d'autorité publisher.
- Les opérations contenues, sémantiquement distinctes, restent comptées chacune.
- Une référence d'édition et une preuve Gamma de la même mutation comptent une fois.
- Un `kind:"merge"` et son enveloppe de publication comptent une décision publisher.
- Une résolution n'introduit aucun kind Gamma distinct.
- Aucun kind Gamma publication n'est inventé.

#### Versioning des nouveaux compteurs

- Les compteurs mutation et total sont des sémantiques validées mais n'existent pas
  dans le wire actuel.
- Leur schéma signé, leaf encoding, roots, replay et migration attendent CB2 et des
  vecteurs indépendants.
- Les artefacts historiques restent byte-identiques sous leur version historique.
- `max_actions`, kinds Gamma et count roots historiques ne sont pas réinterprétés.
- Schéma ancien enrichi, non versionné ou version inconnue : refus fermé.

---

## 5. Fichiers exacts du futur commit contractuel CB1

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

Total : 19 fichiers.

Ne pas ajouter à ce commit :

- le présent handoff ;
- son prompt de reprise ;
- un autre document ;
- Cargo, lock, Provider ou vecteur ;
- `_gitjunk`, `_to_delete`, `_transfer` ou une scorie non suivie.

Diff suivi observé avant ajout des trois nouvelles features :

`16 files changed, 1252 insertions(+), 69 deletions(-)`

Les trois nouvelles features comptent 377 lignes au total.

---

## 6. Preuves de validation CB1

Contrôles finaux après intégration de la validation humaine :

```text
git diff --check
→ propre

nouveaux contrats suivis
→ 51 déclarations / 51 @wip

features m/n/o
→ 31 déclarations / 31 @wip

total
→ 301 déclarations / 91 @wip

cargo test -p aithos-bundle --locked
→ vert

Cucumber
→ 14 features / 65 rules / 229 scénarios / 906 steps verts
```

Trois audits read-only indépendants ont conclu :

- aucun écart bloquant dans les specs ;
- aucun écart bloquant dans le Gherkin ;
- aucune portée non approuvée ou décision manquante ;
- aucun nouveau wire implicite ;
- index vide.

---

## 7. Écarts d'implémentation toujours ouverts

CB1 est un contrat, pas une implémentation. Les principaux gaps restent :

- `aithos-core/src/mandate.rs`
  - lattice D2 contraire ;
  - `id=` absent ;
  - `verify_op` incomplet.
- `aithos-core/src/constraints.rs`
  - validation root et atténuation incomplètes ;
  - `max_children` encore supprimable.
- `aithos-core/src/gamma.rs`
  - pas de rejeu sémantique complet de toutes les consommations.
- `aithos-bundle/src/lib.rs`
  - Store limité à `get/put/list`, sans transaction opposable.
- `aithos-bundle/src/bundle.rs` et `grants.rs`
  - mutations owner/grantee encore largement `circle`-only ;
  - publication normale grantee refusée ;
  - écritures possibles avant Gamma.
- `aithos-bundle/src/log.rs`
  - `gamma_verify` reste partiel.
- Vault
  - racine commune historique, sans isolation complète `/x/<connector>`.
- Façade keyless
  - assemblage et verdict sémantique froid unique absents.

Ne contourner aucun gap dans Provider, Gateway ou une surface.

---

## 8. Prochaine action — gate de commit CB1

La validation du contrat est acquise. Le staging et le commit ne sont pas encore
autorisés dans ce thread.

La prochaine session doit d'abord recevoir une phrase explicite telle que :

> J'autorise le staging des 19 fichiers contractuels CB1 listés dans le handoff,
> leur diff indexé, puis le commit contractuel isolé. Aucun autre fichier et aucun
> travail CB2 ne sont autorisés.

Sans cette phrase ou une autorisation équivalente :

1. vérifier read-only branche, HEAD, statut et index ;
2. comparer les 19 fichiers à ce handoff ;
3. présenter le plan de staging ;
4. STOP sans `git add`.

Avec autorisation :

1. stager les 19 fichiers par noms exacts ;
2. vérifier `git diff --cached --check` ;
3. vérifier que le diff indexé contient exactement ces 19 fichiers ;
4. présenter le diff indexé ;
5. committer avec un message étroit, proposé :
   `test(protocol): add core-bundle CB1 completeness contracts @wip` ;
6. relever le hash et présenter les données nécessaires à un futur handoff
   `CB1-DONE` ;
7. demander une autorisation séparée avant de créer ou modifier ce nouveau
   document ;
8. STOP avant CB2.

Aucun push n'est demandé.

---

## 9. Gate CB2 ultérieur

CB2 dépend de deux autorisations distinctes :

1. CB1 validé **et commité** ;
2. lancement explicite de CB2 et attribution des fichiers vectoriels chevauchés.

CB2 doit respecter :

```text
oracle indépendant
→ vecteur
→ test observé rouge pour la raison attendue
→ commit vecteurs/tests séparé
→ seulement ensuite CB3/TDD Rust
```

Faits d'ownership bloquants :

- `vectors/README.md` appartient encore à la piste Provider ;
- `rust/Cargo.toml` et `rust/Cargo.lock` restent hors scope ;
- si le registre exige `vectors/README.md`, STOP et demander une attribution ;
- aucun oracle n'appelle la fonction Rust testée ;
- aucun octet historique ne change sans vecteur de non-régression.

---

## 10. Séquence restante

```text
commit CB1 isolé
→ STOP
→ CB2 oracles/vecteurs/tests rouges
→ CB3 forme canonique et id=
→ CB4 opération canonique et verdict pur
→ CB5 contraintes, compteurs et catalogue
→ CB6 rejeu Gamma
→ CB7 transaction Bundle
→ CB8 parité owner et grants
→ CB9 mutations déléguées
→ CB10 structure, révocation et vault
→ CB11 changesets et éditions
→ CB12 paquet et cold verify local
→ CB13 concurrence et gate final Core + Bundle
→ reprise Provider dans sa piste
```

Chaque tranche passe son propre gate et produit un commit étroit. Le Provider ne
reprend le protocole qu'après CB13.

---

## 11. Interdictions permanentes

- Ne pas changer de branche.
- Ne pas clean/reset/restore/checkout les travaux existants.
- Ne pas push/merge/déployer.
- Ne pas stage ou commit un fichier non explicitement attribué.
- Ne pas modifier Provider, Gateway, CLI/WASM, client ou SDK réseau.
- Ne pas toucher Cargo/lock sans attribution.
- Ne pas toucher `vectors/README.md` sans attribution.
- Ne pas produire de vecteur avant le commit CB1 et le gate CB2.
- Ne pas produire de Rust avant vecteur indépendant et test rouge.
- Ne pas retirer un `@wip` avant test réel durablement vert.
- Ne pas simuler le CAS Provider dans Bundle.
- Ne pas copier une règle Core dans Bundle ou dans une surface aval.
- Ne pas confondre export/import local et E2E Provider.

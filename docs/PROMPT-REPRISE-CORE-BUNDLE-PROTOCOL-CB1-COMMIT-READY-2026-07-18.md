# Prompt de reprise — commit contractuel CB1 Core + Bundle

Copier-coller le bloc suivant dans un nouveau contexte.

---

Tu reprends le gate du commit contractuel CB1 Core + Bundle dans :

`/Volumes/Math17/aithos/v2/code/aithos-core`

## Source principale opposable

Lis entièrement avant toute action :

`docs/HANDOFF-CORE-BUNDLE-PROTOCOL-CB1-COMMIT-READY-2026-07-18.md`

Lis ensuite entièrement, dans l'ordre indiqué par ce handoff, les sources qu'il
référence.

## État attendu

- branche : `feat/obligations` — ne pas la changer ;
- HEAD observé :
  `7349cf62f98c39ee03bfef1ed3ca0616a76485dc` ;
- ce HEAD est un commit Provider concurrent légitime ;
- CB0 est terminé ;
- CB1 est validé, corrigé et prêt au commit, mais non commité ;
- l'index attendu est vide ;
- 82 nouvelles déclarations sont toutes `@wip` ;
- aucun vecteur, oracle, Rust ou code post-CB1 n'est autorisé.

Si la branche courante n'est pas `feat/obligations`, STOP sans la changer. Si HEAD
ou ownership diffère, ne reviens pas à l'ancien état : relève le changement,
contrôle les 19 cibles et signale tout chevauchement avant staging.

## Mission de cette reprise

Cette reprise est uniquement le gate du commit contractuel CB1.

### 1. Inspection read-only

Exécute :

```bash
git branch --show-current
git rev-parse HEAD
git status --short --branch --untracked-files=all
git diff --check
git diff --name-only
git diff --name-only --cached
```

Confirme que l'index est vide et que le commit Provider courant n'a pas chevauché
les cibles CB1.

### 2. Cibles exactes

Les seules cibles CB1 sont :

```text
spec/01-identity-and-keys.md
spec/02-content-tree.md
spec/04-mandates.md
spec/05-delegation.md
spec/08-connectors.md
features/d-bundle.feature
features/e-mandate-sections.feature
features/e-mandates.feature
features/f-gamma.feature
features/f-plus-constraints.feature
features/g-plus-obligations.feature
features/g-revocation.feature
features/h2-gamma-roots.feature
features/i-concurrency.feature
features/k-integration.feature
features/l-delegated-writes.feature
features/m-delegated-editions.feature
features/n-structural-mutations.feature
features/o-connector-classes-vault.feature
```

Exige exactement ces 19 fichiers, sans manquante ni extra.

### 3. Contrôles contractuels

Confirme :

- 51 nouvelles déclarations / 51 `@wip` dans les features suivies ;
- 31 nouvelles déclarations / 31 `@wip` dans `m/n/o` ;
- total 301 déclarations / 91 `@wip` ;
- 256 `Scenario` et 45 `Scenario Outline` ;
- chaque nouvelle déclaration a un `@wip` sur la ligne immédiatement précédente ;
- aucun scénario, step ou tag historique n'a été supprimé, modifié ou retaggé ;
- aucun Rust, aucune implémentation de step Cucumber, aucun vecteur, oracle,
  fixture, wire ou fichier d'une autre piste n'est inclus ;
- les deux clarifications finales du handoff sont présentes et ne réintroduisent
  aucun effet upstream ou rejet des artefacts historiques.

### 4. Validation pertinente

Depuis `rust/`, avec le target isolé :

```bash
CARGO_TARGET_DIR=/Volumes/Math17/aithos/v2/.codex-targets/core-bundle-cb0-20260718 \
CARGO_INCREMENTAL=0 \
cargo test -p aithos-bundle --locked
```

Résultat attendu :

- 14 features ;
- 65 rules ;
- 229 scénarios verts ;
- 906 steps verts ;
- 4/4 tests I1 verts.

## Autorisation de commit

Le présent prompt n'autorise pas à lui seul `git add` ou `git commit`.

Sans phrase explicite de Mathieu autorisant le staging et le commit CB1 :

1. présente branche, HEAD, statut, contrôles et liste exacte des 19 fichiers ;
2. demande l'autorisation ;
3. STOP sans `git add`.

Avec une autorisation explicite limitée au commit CB1 :

1. relève à nouveau branche, HEAD, index et ownership ;
2. stage chaque fichier par son nom exact, jamais un répertoire, `git add .` ou
   `git add -A` ;
3. vérifie :

```bash
git diff --cached --check
git diff --cached --name-status
git diff --cached --stat
```

4. exige exactement les 19 fichiers ci-dessus ;
5. exclus explicitement :
   - les handoffs et prompts ;
   - tous les autres docs non suivis ;
   - `_gitjunk`, `_to_delete`, `_transfer` ;
   - `rust/crates/aithos-provider/**` ;
   - `rust/Cargo.toml` et `rust/Cargo.lock` ;
   - `vectors/README.md` et tout vecteur ;
   - Gateway, CLI/WASM, client et toute surface ;
6. inspecte et présente le diff indexé complet avant commit ;
7. crée le commit étroit :

```text
test(protocol): add core-bundle CB1 completeness contracts @wip
```

8. relève le hash, le subject, les 19 fichiers et les compteurs ;
9. présente le worktree résiduel des autres pistes ;
10. STOP avant tout handoff `CB1-DONE` et avant CB2.

## Ce qui reste après le commit

Ne commence rien de cette liste dans cette reprise :

```text
CB2  oracles indépendants, vecteurs et tests rouges
CB3  forme canonique, id= et lattice
CB4  opération canonique et verdict Core pur
CB5  contraintes, compteurs et catalogue connecteurs
CB6  rejeu Gamma sémantique
CB7  transaction Bundle
CB8  parité owner et grants
CB9  mutations déléguées public/circle/self
CB10 structure, révocation, rotation et vault
CB11 changesets et éditions déléguées
CB12 paquet de publication et cold verify local
CB13 concurrence et gate final Core + Bundle
```

CB2 exige une nouvelle autorisation. Tout fichier vectoriel chevauché exige une
attribution ; si le registre doit changer, STOP jusqu'à attribution explicite de
`vectors/README.md`.

Son ordre futur reste :

```text
oracle indépendant
→ vecteur
→ test observé rouge pour la raison attendue
→ commit vecteurs/tests distinct
→ seulement ensuite TDD Rust
```

## Interdictions absolues

- aucun push, merge, changement de branche ou déploiement ;
- aucun clean, reset, restore, checkout ou déplacement de travaux existants ;
- aucun Rust, aucune implémentation de step Cucumber, aucun retrait de `@wip`,
  aucun vecteur, oracle ou fixture ;
- aucun Cargo/lock, Provider, Gateway, CLI/WASM, client, RemoteStore ou SDK ;
- aucun nouveau champ signé, kind Gamma, compteur ou migration wire ;
- aucune modification de `vectors/README.md` sans attribution ;
- aucune logique Core recopiée dans Bundle ou une surface ;
- aucun faux CAS ou faux E2E Provider.

## Conditions d'arrêt

STOP et demande une décision si :

- la branche n'est plus `feat/obligations` ;
- HEAD a avancé et chevauche une cible ;
- l'index n'est plus vide et son contenu n'est pas attribué ;
- une cible contient un changement non décrit par le handoff ;
- le nombre de fichiers, déclarations ou `@wip` diffère ;
- la validation exige une modification hors scope ;
- le diff indexé contient autre chose que les 19 fichiers ;
- le commit mélangerait contrat et implémentation.

---

Pour autoriser aussi le commit dans la tâche de reprise, ajouter séparément au
message de lancement :

```text
J'autorise explicitement le staging des 19 fichiers contractuels CB1 nommés dans
le handoff, la présentation de leur diff indexé, puis leur commit isolé. Aucun
autre fichier et aucun travail CB2 ne sont autorisés.
```

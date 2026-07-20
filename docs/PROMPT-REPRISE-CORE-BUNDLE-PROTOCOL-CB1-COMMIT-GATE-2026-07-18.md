# Prompt de reprise — Core + Bundle, gate de commit CB1

Copier le bloc ci-dessous dans une nouvelle tâche Codex.

```text
Tu reprends le gate de commit CB1 Core + Bundle dans :

/Volumes/Math17/aithos/v2/code/aithos-core

SOURCE PRINCIPALE OPPOSABLE

Lis entièrement avant toute action :

docs/HANDOFF-CORE-BUNDLE-PROTOCOL-CB1-VALIDATED-2026-07-18.md

Lis ensuite entièrement, dans cet ordre :

1. docs/HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md
2. docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md
3. docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md
4. docs/NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md
5. README.md et les rituels BDD/vectors-first/pure-core rendus obligatoires

ÉTAT ATTENDU

- branche : feat/obligations — ne pas la changer ;
- HEAD observé au handoff :
  7349cf62f98c39ee03bfef1ed3ca0616a76485dc ;
- ce HEAD est un commit Provider concurrent légitime, non créé par la piste
  Core + Bundle ;
- CB0 est terminé ;
- CB1 est validé humainement et intégré dans le worktree ;
- l'index attendu est vide ;
- 82 nouvelles déclarations sont toutes @wip ;
- aucun vecteur ou code post-CB1 n'est autorisé dans cette reprise.

Si la branche courante n'est pas `feat/obligations`, STOP sans la changer et
demande une décision. Si HEAD ou ownership diffère, n'essaie pas de revenir à
l'ancien état : relève le changement, vérifie les 19 cibles et signale tout
chevauchement avant staging.

DÉCISIONS OPPOSABLES

D1–D9 et T1–T3 du handoff Lot 1 sont acquises.

Le gate humain du 2026-07-18 a également validé :

- G-A : act.x.<id>.config est une capacité vault exacte et indivisible en v1,
  couvrant CRUD config, hors catalogue read/act/binding, hors wildcard et sans
  co_sign implicite ; tout split futur est versionné et non rétroactif ;
- G-B : transaction à point logique de linéarisation ; ancien ou nouvel état
  complet après crash, jamais un mélange ; mécanisme FsStore strictement local au
  Store et hors layout/wire ;
- G-C : capacités crypto opaques, étroites, typées par purpose et liées au contexte,
  sans sign/decrypt/wrap oracle générique ;
- G-D : Bundle assemble et valide le layout, Core rend le verdict pur, Provider
  futur ne fait que stockage opaque/transport/CAS ;
- G-E : une extension inconnue root-feuille est préservée mais toute consommation
  refuse en v1, sans Gamma/état/compteur ; une version future ne réinterprète jamais
  les bytes actuels ;
- confinement séparé display paths / Store keys, avec refus des escapes filesystem ;
- compteurs sur consommations logiques : pas de double comptage manifest/Gamma,
  kind:merge corrélé à son enveloppe, aucun kind publication ou résolution inventé ;
- compteurs mutation/total sémantiquement validés mais wire réservé à CB2, versionné
  par vecteurs indépendants et jamais simulé par max_actions ou les roots existants.

MISSION DE CETTE REPRISE

Cette reprise est uniquement le gate du futur commit contractuel CB1.

1. Inspecte read-only :

   git branch --show-current
   git rev-parse HEAD
   git status --short --branch --untracked-files=all
   git diff --check
   git diff --name-only
   git diff --name-only --cached

2. Vérifie que les seules cibles du futur commit CB1 sont exactement :

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

3. Confirme :

   - 51 nouvelles déclarations / 51 @wip dans les fichiers suivis ;
   - 31 nouvelles déclarations / 31 @wip dans m/n/o ;
   - total 301 déclarations / 91 @wip ;
   - chaque nouvelle déclaration a un @wip local immédiat ;
   - aucun scénario, step ou tag historique n'a été supprimé ou retaggé ;
   - aucun Rust, step, vecteur, wire ou fichier d'une autre piste n'est inclus.

4. Rejoue seulement la validation pertinente depuis `rust/`, avec target isolé :

   CARGO_TARGET_DIR=/Volumes/Math17/aithos/v2/.codex-targets/core-bundle-cb0-20260718 \
   CARGO_INCREMENTAL=0 \
   cargo test -p aithos-bundle --locked

   Le résultat attendu est notamment :

   - 14 features ;
   - 65 rules ;
   - 229 scénarios verts ;
   - 906 steps verts ;
   - 4 tests I1 verts.

AUTORISATION DE COMMIT

Le présent prompt n'autorise pas à lui seul git add ou git commit.

Sans phrase explicite de Mathieu autorisant le staging et le commit CB1 :

- présente branche, HEAD, statut, contrôles et liste exacte des 19 fichiers ;
- demande l'autorisation ;
- STOP sans git add.

Avec une autorisation explicite limitée au commit CB1 :

1. stage chaque fichier par son nom exact, jamais un répertoire ou `git add .` ;
2. vérifie :

   git diff --cached --check
   git diff --cached --name-status
   git diff --cached --stat

3. exige exactement les 19 fichiers ci-dessus ;
4. exclus explicitement :

   - ce handoff et ce prompt ;
   - tous les autres docs non suivis ;
   - _gitjunk, _to_delete et _transfer ;
   - rust/crates/aithos-provider/** ;
   - rust/Cargo.toml et rust/Cargo.lock ;
   - vectors/README.md et tout vecteur ;
   - Gateway, CLI/WASM, client et toute surface.

5. présente le diff indexé avant commit ;
6. crée un commit étroit proposé :

   test(protocol): add core-bundle CB1 completeness contracts @wip

7. relève le hash, le subject et les compteurs ;
8. présente dans ton rapport les données nécessaires à un futur handoff CB1-DONE :

   - hash du commit ;
   - liste des 19 fichiers ;
   - 82 nouvelles déclarations toutes @wip ;
   - commandes et résultats ;
   - worktree résiduel appartenant aux autres pistes ;
   - blocage explicite avant CB2 ;

9. ne crée ou modifie aucun document sans autorisation séparée ;
10. STOP.

INTERDICTION POST-COMMIT

Ne commence pas CB2 dans cette reprise, même si le commit réussit.

CB2 exige une nouvelle autorisation et l'arbitrage de l'ownership de
vectors/README.md. Son ordre futur sera :

oracle indépendant
→ vecteur
→ test observé rouge pour la raison attendue
→ commit vecteurs/tests distinct
→ seulement ensuite TDD Rust

INTERDICTIONS ABSOLUES

- aucun push, merge, changement de branche ou déploiement ;
- aucun clean, reset, restore, checkout ou déplacement des travaux existants ;
- aucun code Rust, aucune implémentation de step Cucumber ou retrait de @wip ;
- aucun vecteur, oracle ou fixture ;
- aucun nouveau champ signé ou migration wire ;
- aucune modification Provider, Gateway, CLI/WASM, client, RemoteStore ou SDK ;
- aucune modification Cargo/lock ;
- aucune modification de vectors/README.md sans attribution ;
- aucune logique Core recopiée dans Bundle ou une surface ;
- aucune simulation du CAS Provider ou faux E2E Provider.

CONDITIONS D'ARRÊT

STOP et demande une décision si :

- la branche courante n'est pas feat/obligations ;
- un des 19 fichiers contient un changement non décrit par le handoff ;
- l'index n'est pas vide au départ et son contenu n'est pas attribué ;
- HEAD a avancé et chevauche une cible ;
- une validation exige de modifier Rust, Cargo, Provider ou vecteurs ;
- le nombre de fichiers, déclarations ou @wip diffère ;
- le diff indexé contient autre chose que les 19 fichiers ;
- le commit nécessiterait de mélanger contrat et implémentation.
```

Pour autoriser le commit dans la tâche de reprise, ajouter séparément au message de
lancement :

```text
J'autorise explicitement le staging des 19 fichiers contractuels CB1 nommés dans le
handoff, la présentation de leur diff indexé, puis leur commit isolé. Aucun autre
fichier et aucun travail CB2 ne sont autorisés.
```

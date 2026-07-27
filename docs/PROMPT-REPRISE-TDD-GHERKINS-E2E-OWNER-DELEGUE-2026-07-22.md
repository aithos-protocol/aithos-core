# Prompt de reprise — développement TDD Gherkins E2E owner/délégué

Copier-coller le prompt ci-dessous dans une nouvelle tâche Codex ouverte à la
racine `/Volumes/Math17/aithos/v2`.

---

Tu reprends le chantier Aithos de couverture fonctionnelle Gherkin réellement
E2E pour owner et délégué.

Le workspace contient plusieurs dépôts liés :

- `code/aithos-core`
- `code/aithos-client`
- `code/aithos-sdk`
- le provider dans `code/aithos-core/rust/crates/aithos-provider`

La gateway est explicitement **hors périmètre** de ce chantier. Ne développe
pas de surface générique `ethos.write` dans la gateway et ne cherche pas à
activer ses sessions déléguées. Le provider reste en périmètre, car il est une
frontière indispensable au véritable E2E client/SDK.

## Sources de vérité à lire entièrement avant toute modification

1. `code/aithos-core/docs/TODO-AUDIT-GHERKINS-E2E-2026-07-22.md`
2. `code/aithos-core/docs/PLAN-TDD-GHERKINS-E2E-OWNER-DELEGUE-2026-07-22.md`

Lis les deux fichiers jusqu'à EOF. Le second document contient la matrice
canonique, les scénarios précis, le développement associé, les gates GREEN,
l'ordre des lots et la définition de done. Ne résume pas leur lecture à partir
de ce prompt : utilise réellement leur contenu comme backlog.

## Objectif final

Prouver sur la même donnée et le même package la chaîne suivante :

`intention -> clé/capacité -> mandat -> Core/Bundle -> plan client -> SDK -> provider HTTP/CAS -> vrai restart -> téléchargement -> store vierge -> cold verify -> lecture`

La couverture finale doit comprendre :

- les 15 cellules owner list/read/create/edit/delete sur public/circle/self ;
- les 18 cellules déléguées de la matrice L ;
- les 19 mutations verticales owner/délégué à publier réellement ;
- autorité, possession, expiry, revocation et refus latéraux ;
- structure, atomicité, Gamma, changeset, evidence et authorship ;
- CAS, interruption, idempotence, restart, store vierge et artefacts hostiles ;
- parité client natif/WASM ;
- transport et orchestration SDK sans plan ou fetch factice ;
- non-fuite des clés, DK et contenus circle/self.

Le même `operation_ref`, le même acteur, les mêmes mandate ids, le même
manifest head et le même package digest doivent être suivis à travers toutes
les frontières.

## Discipline de travail obligatoire

Travaille strictement en TDD, lot par lot, dans l'ordre du plan :

1. écrire ou réécrire le Gherkin et ses steps ;
2. exécuter le test et observer un RED pertinent ;
3. seulement ensuite développer le minimum nécessaire ;
4. repasser le scénario et toutes les régressions du lot au GREEN ;
5. consigner les commandes, compteurs et résultats ;
6. passer au lot suivant uniquement lorsque le gate du lot courant est réel.

Ne marque jamais un lot terminé parce qu'un test CB autonome est vert.

Interdictions :

- aucun nouveau step release ne doit appeler `cbN_result()` ou
  `cbN_assert_green()` ;
- aucun résultat global mis en cache ne doit remplacer les paramètres d'une
  ligne `Examples` ;
- aucun `@wip` ne doit transformer un scénario requis en suite verte ;
- aucun plan JavaScript littéral, fetch mock ou package d'une autre fixture ne
  doit satisfaire un scénario E2E ;
- aucune publication grantee ne doit être signée ou republiée par l'owner ;
- aucun reopen/cold verify ne doit réutiliser le store ou les capacités du
  producteur ;
- aucune évolution du wire ne doit être inventée uniquement pour faciliter un
  test.

Les mocks restent acceptables dans les tests unitaires existants. Ils ne
comptent simplement pas dans le gate E2E.

## Worktree et sécurité des modifications

Le worktree contient déjà de nombreuses modifications utilisateur, notamment
dans les docs, la gateway et le provider. Elles ne t'appartiennent pas.

- Inspecte `git status` dans chaque dépôt avant d'éditer.
- Préserve toutes les modifications existantes et évite les fichiers hors
  périmètre.
- N'utilise ni reset, ni checkout destructif, ni nettoyage global.
- Ne crée pas de commit et ne pousse rien sans demande explicite.
- Les deux documents de backlog sont des sources de vérité : mets à jour leurs
  cases et ajoute un journal d'exécution, mais ne supprime pas les exigences.

## Point de départ vérifié le 2026-07-22

Les suites sélectionnées étaient vertes avant développement :

- Core/Bundle Cucumber : 815 scénarios, 3 505 steps ;
- CB9 delegated content : 3/3 ;
- CB10 structure/vault : 4/4 ;
- CB12 publication package : 5/5 ;
- client natif : 94 scénarios, 459 steps ;
- client WASM : 19 scénarios, 117 steps ;
- provider : 151 scénarios, 992 steps ;
- gateway : 296 scénarios, mais hors périmètre de ce chantier.

Ces verts ne prouvent pas tous un vrai E2E : certains Core sont des proxies et
les parcours provider délégué/cold roundtrip sont filtrés. Conserve néanmoins
ces compteurs comme baseline de non-régression.

## Ordre d'exécution attendu

### Étape 0 — harnais anti-faux-verts

Commence impérativement par HARN-001 à HARN-005 du plan :

- runner Cucumber SDK ;
- manifest des scénarios release ;
- preuve qu'une ligne Outline atteint réellement la production ;
- trace publique d'un package unique ;
- refus des fake plans dans le gate E2E ;
- superviseur du provider ;
- backend provider durable de test permettant un vrai changement de PID.

Avant de développer toute mutation circle/self, prouve que ce harnais détecte
volontairement un proxy CB, un fake plan et un pseudo-restart.

### Étapes suivantes

Suis ensuite exactement le tableau « Ordre RED -> DEV -> GREEN recommandé » du
plan :

1. Core owner ;
2. Core délégué ;
3. structure et révocation ;
4. édition et cold package unique ;
5. client natif multi-zone ;
6. WASM ;
7. SDK publication/CAS/download ;
8. provider delegated CAS et restart ;
9. gates E2E 19 mutations ;
10. replay, contraintes, obligations, compteurs et vault.

Ne saute pas directement au scénario transverse en conservant des proxies dans
les couches basses.

## Développements structurants déjà identifiés

Le plan RED pourra préciser ou réduire ces changements, mais il faudra
probablement :

- remplacer `PublicMutationIntent` par un `MutationIntent` fermé multi-zone ;
- généraliser `PublicationPlan::build_owner` et `build_grantee` ;
- étendre les keyholders purpose-bound public/circle/self ;
- ajouter les bindings WASM owner/grantee mutation sans exporter les clés ;
- faire produire un unique `KeylessPublicationPackage` par le vrai acteur ;
- remplacer les steps CB proxy par des worlds indépendants et paramétrés ;
- ajouter le runner Gherkin et les features à `aithos-sdk` ;
- ajouter au SDK heads/list/batch/sync ou `downloadSnapshot` ;
- distinguer `published_unverified` de `published_and_verified` ;
- activer le publish manifest délégué sous CAS dans le provider ;
- ajouter un backend provider durable réservé au test local ;
- lancer un vrai binaire `aithos-store-api`, le tuer et le redémarrer ;
- télécharger dans un nouveau store local vide avant cold verify.

Ne réalise un élément que lorsqu'un scénario RED du lot courant le réclame.

## Validation attendue à chaque lot

À chaque compte rendu, donne :

- les IDs de scénarios traités ;
- le RED observé avant le développement ;
- les fichiers de production modifiés et pourquoi ;
- les commandes exactes lancées ;
- les compteurs scénarios/steps après GREEN ;
- les scénarios restant dans le lot ;
- les risques ou décisions de protocole éventuels.

Maintiens un fichier de suivi :

`code/aithos-core/docs/JOURNAL-TDD-GHERKINS-E2E-OWNER-DELEGUE-2026-07-22.md`

Crée-le au démarrage avec une section par lot. N'y inscris aucun secret ni
bytes privés.

## Commandes de baseline utiles

Adapte les target dirs au volume disponible : évite `/tmp` si le volume système
est plein. Ne lance la gateway que pour vérifier une régression transitive
exceptionnelle ; elle n'est pas un gate de ce chantier.

Depuis `code/aithos-core/rust` :

```sh
CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cucumber
CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cb9_delegated_content
CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cb10_structure_vault
CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cb12_publication_package
CARGO_INCREMENTAL=0 cargo test -p aithos-provider --test cucumber
```

Depuis `code/aithos-client` :

```sh
CARGO_INCREMENTAL=0 cargo test -p aithos-client --test cucumber --test phase_e_cucumber --test phase_f_cucumber -p aithos-client-wasm --test cucumber
```

Depuis `code/aithos-sdk` :

```sh
npm test
npm run test:gherkin
```

`test:gherkin` n'existe pas encore : sa première absence fait partie du RED du
lot 0.

## Critères de décision

Tu peux corriger sans demander lorsqu'il s'agit d'implémenter le comportement
déjà fermé par les Gherkins et les specs existantes.

Arrête-toi et demande une décision seulement si :

- public/circle/self exigent une évolution incompatible du wire ;
- deux sources normatives donnent des verdicts contradictoires ;
- le choix du backend durable de test modifie l'architecture de production ;
- une API publique doit être supprimée sans chemin de compatibilité ;
- une autorité devrait être élargie au-delà des matrices signées existantes.

Dans ce cas, produis d'abord un RED minimal et expose les deux options avec leur
impact. Ne contourne pas le désaccord par un mock ou un `@wip`.

## Condition d'arrêt

Ne t'arrête pas après un audit supplémentaire ou après la création du runner.
Poursuis le développement lot par lot tant qu'un prochain changement sûr et
dans le périmètre permet d'avancer.

Le chantier est achevé uniquement lorsque la définition de done globale du
plan est satisfaite, notamment :

- 15 cellules owner et 18 cellules grantee scenario-driven dans Core ;
- 19 mutations verticales réelles via le SDK ;
- client natif et WASM identiques avec inputs injectés ;
- vrai provider HTTP redémarré avec PID différent ;
- store vierge distinct et vérification keyless avant capacité ;
- aucun proxy CB, fake plan ou `@wip` dans le périmètre release ;
- CAS, altérations, interruptions et non-fuite tous GREEN.

Commence maintenant par :

1. lire intégralement les deux documents ;
2. inspecter les statuts Git sans modifier ;
3. créer le journal TDD ;
4. exécuter les baselines proportionnées ;
5. écrire le premier RED HARN-001/HARN-005 ;
6. développer le lot 0 jusqu'à son gate GREEN ;
7. enchaîner avec le lot 1.

---

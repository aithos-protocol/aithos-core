# Audit d'implémentation — `b-derivation.feature`

## Métadonnées

| Champ | Valeur |
|---|---|
| Feature auditée | `features/b-derivation.feature` |
| Date | 2026-07-29 |
| Révision Git observée | `891c808` (branche `codex/gherkin-agent-pilot`) |
| État observé | Worktree propre — baseline reproductible |
| Runner principal | `aithos-bundle --test cucumber` |
| Implémentation principale | `aithos-core::{derive,path,ids}` |
| Surfaces contrôlées | Core, Bundle, CLI ; vecteurs `vectors/` et leurs générateurs |
| Méthode | Deux passes, Pass A aveugle à l'historique en trois unités de revue isolées (une par `Rule`), puis passe d'intégration, revue challenger adverse, et Pass B différentielle |
| Statut de la note | **OUVERTE — ronde 1 acceptée et intégrée (2026-08-02) ; `BDER-011` `VERIFIED` (2026-07-30) ; décisions `BDER-006` et `BDER-008` arbitrées le 2026-08-02 ; ronde 2 corrigée et en attente de revue indépendante (`REVIEW_REQUESTED`, candidat `4f5921e`)** |

## Verdict

Les six scénarios sont sélectionnés et exécutent du vrai code Rust de
production. Aucun step n'est `@wip`, mocké, ni remplacé par un verdict
`OnceLock` partagé — la passe d'intégration a explicitement écarté le motif
`PROXY` pour les six.

Le vert ne prouve pourtant presque rien du contrat annoncé :

- 0 scénario est `PROUVÉ` ;
- 2 scénarios sont `PARTIEL` ;
- 4 scénarios sont `FAUX POSITIF` au regard du résultat qu'ils affirment.

**Le code de production n'est pas en cause.** `aithos-core::derive` est
conforme à la spec 01.3 / 02.5, aucune surface de production ne contourne
`node_key`, et les trois fonctions de label n'ont aucun autre site d'appel que
`node_key` lui-même. Tous les écarts ci-dessous portent sur ce que les
scénarios *ne prouvent pas*.

### Mesure de pouvoir discriminant

La revue challenger a muté `node_key` dans une copie jetable du workspace et
rejoué les gates. Résultat, par scénario, sur cinq mutants :

| Scénario | Mutants tués / 5 |
|---|---:|
| A folder holder derives every descendant | **5** |
| Sibling nodes get unrelated keys | 2 |
| A folder holder cannot reach sideways | 2 |
| A folder-local tag view is its own lock | 2 |
| The same path always yields the same key | **0** |
| Renaming never re-keys | **0** |
| `tests/b2_derivation.rs` (hors Gherkin) | **5** |

Le mutant décisif — M5, une étape `parent XOR blake3(label)` au lieu de
`blake3::derive_key`, refactor « KDF moins chère » parfaitement plausible —
détruit intégralement l'unidirectionnalité : le détenteur du dossier 1 récupère
la clé de zone par un XOR et atteint exactement la clé de section du frère.
**813 des 815 scénarios BDD restent verts sous ce mutant.** Seuls quatre tests
unitaires byte-exacts et deux scénarios BDD le voient.

### Mitigation à lire avec chaque écart

Aucun invariant de protocole n'est laissé sans garde. Les dérivations sont
figées byte-exact par `tests/b2_derivation.rs`, `f1_gamma.rs:64` et
`g3_move.rs:114` contre `vectors/{b2-derivation,f1-gamma-chain,g3-move}.json`,
et l'invariant de renommage est prouvé de bout en bout par
`features/d-bundle.feature:38-41`, qui appelle le vrai `Bundle::rename_folder`
et relit la section à son nouveau chemin. Cette note ne dit pas que la
dérivation est non testée ; elle dit que **la couche Gherkin n'y contribue
presque rien**, alors que c'est elle qui porte le contrat lisible.

## Preuves rejouées

### Gherkin

```text
18 features
112 rules
815 scenarios (815 passed)
3505 steps (3505 passed)
```

Bloc `b-derivation` observé explicitement dans la sortie : 1 feature, 3 rules,
6 scénarios, 21 steps, tous verts. Le compte a été contrôlé nommément, pour
qu'une sélection vide ne puisse pas passer pour un succès.

### Vecteurs

```text
cargo test -p aithos-core --test b2_derivation
b2_deep_chain_and_anchors            ... ok
b2_folder_key_alone_derives_descendants ... ok
2 passed
```

### Sondes négatives hors dépôt

Mutants appliqués à `node_key` dans une copie jetable, jamais dans le dépôt :

```text
M1 constant [0x42;32]        : b2_derivation FAIL,  9/815 scénarios BDD échouent
M2 ignore le chemin          : b2_derivation FAIL,  7/815 scénarios BDD échouent
M3 hash monolithique         : b2_derivation FAIL, 71/815 scénarios BDD échouent
M4 31 octets de zone recopiés: b2_derivation FAIL, 71/815 scénarios BDD échouent
M5 étape XOR (unidirection.  : b2_derivation FAIL,  2/815 scénarios BDD échouent
   totalement détruite)
```

Sous M5, `cargo test --workspace --no-fail-fast` ne fait tomber que quatre
tests unitaires — tous des tests de vecteurs — et deux scénarios BDD.

Une énumération exhaustive de 13 332 dérivations étiquetées atteignables depuis
la clé du dossier 1 (épines de dossiers jusqu'à la profondeur 3 sur les sids
0..9, terminées ou non par `s/<sid>` ou `t/toto`) donne 0 atteinte de la clé de
section du frère sur l'implémentation réelle. Le scénario correspondant en
explore 3.

Les sondes et leurs artefacts ne font pas partie du dépôt.

## Cartographie des preuves

| Objet | Source principale | Rôle dans l'audit |
|---|---|---|
| Contrat Gherkin | [`features/b-derivation.feature`](../../../features/b-derivation.feature) | Texte normatif testé |
| Steps | [`aithos-bundle/tests/cucumber.rs`](../../../rust/crates/aithos-bundle/tests/cucumber.rs) | Entrées, appels et assertions réellement exécutés |
| Dérivation | [`aithos-core/src/derive.rs`](../../../rust/crates/aithos-core/src/derive.rs) | `node_key`, `folder_label`, `section_label`, `tag_label` |
| Chemins canoniques | [`aithos-core/src/path.rs`](../../../rust/crates/aithos-core/src/path.rs) | `NodePath`, `Leaf`, parsing |
| Vecteur indépendant | [`b2-derivation.json`](../../../vectors/b2-derivation.json) | Preuve positive byte-exacte, hors Gherkin |
| Test de conformité | [`aithos-core/tests/b2_derivation.rs`](../../../rust/crates/aithos-core/tests/b2_derivation.rs) | Seule preuve byte-exacte de la chaîne |
| Consommateurs Bundle | [`grants.rs`](../../../rust/crates/aithos-bundle/src/grants.rs), [`structure.rs`](../../../rust/crates/aithos-bundle/src/structure.rs), [`revoke.rs`](../../../rust/crates/aithos-bundle/src/revoke.rs), [`log.rs`](../../../rust/crates/aithos-bundle/src/log.rs), [`bundle.rs`](../../../rust/crates/aithos-bundle/src/bundle.rs) | Surfaces qui consomment le verdict de dérivation |
| Renommage réel | [`bundle.rs:1534`](../../../rust/crates/aithos-bundle/src/bundle.rs), [`structure.rs:446`](../../../rust/crates/aithos-bundle/src/structure.rs) | Surface que le scénario de renommage n'appelle jamais |
| Verbe CLI | [`aithos-cli/src/main.rs:1414`](../../../rust/crates/aithos-cli/src/main.rs) | `node-key` — vérification manuelle de déterminisme |

## Matrice scénario par scénario

| # | Scénario | Statut | Observation |
|---:|---|---|---|
| 1 | The same path always yields the same key | `FAUX POSITIF` | Le `When` appelle `node_key` deux fois sur **la même valeur** `NodePath` clonée ; le `Then` compare la sortie d'une fonction pure à elle-même. Zéro mutant tué sur cinq. Ni `NodePath::parse`, ni le vecteur B2, ni aucune valeur attendue indépendante n'intervient. |
| 2 | Sibling nodes get unrelated keys | `FAUX POSITIF` | « Unrelated » est encodé en `assert_ne!` sur deux `[u8;32]`. Sous M5, `f1 XOR f2 = blake3(l1) XOR blake3(l2)` : chaque frère se calcule depuis l'autre par un inconnu, les clés sont *maximalement* liées, et le scénario reste vert. L'assertion prouve la sensibilité au sid, pas l'indépendance. |
| 3 | A folder holder derives every descendant | `PARTIEL` | Le meilleur scénario de la feature — le seul à tuer les cinq mutants, y compris M5, parce que son `Then` croise `derive_key(section_label, folder_key)` avec `node_key(zone, chemin_complet)`. L'écart est « every » : une section, une profondeur, une forme. |
| 4 | A folder holder cannot reach sideways | `FAUX POSITIF` | Le `Given` a un corps vide. Le `Then` est un négatif universel prouvé par trois `assert_ne!` ponctuels, qui restent verts si l'on remplace la clé détenue par `[0x00;32]` — l'assertion est aveugle à ce que le `When` a produit. Sous M5, la phrase du `Then` est **fausse** et le scénario est vert. C'est l'écart le plus grave de la feature : l'unidirectionnalité est la propriété de sécurité que cette `Rule` existe pour défendre. |
| 5 | Renaming never re-keys | `FAUX POSITIF` | Le `When` ne renomme rien : il ré-appelle `node_key` sur un `deep_path` inchangé. `Bundle::rename_folder` et `structural_rename_folder` ne sont jamais atteints. Le raisonnement « les noms ne sont pas des entrées, donc renommer est un no-op, donc renommer ne re-keye pas » suppose sa conclusion, et ne résiste même pas au changement qu'il prétend garder : un champ nom optionnel dans `NodePath` laisserait le scénario vert. |
| 6 | A folder-local tag view is its own lock | `PARTIEL` | La phrase du `Then` est littéralement et entièrement prouvée : le `BTreeSet` de trois clés impose les trois inégalités annoncées, et l'assertion attrape une ancre insensible à l'épine du dossier. Ce qui n'est pas exercé, c'est la sémantique d'ancrage §02.9 elle-même — l'ancre ne donne rien par dérivation descendante, les sections y entrent par `wrap`, une vue locale ne couvre que son sous-arbre. |

## Écarts et implémentations requises

### BDER-001 — Ancrer le déterminisme à une valeur attendue indépendante

**Priorité : P1 — `VERIFIED` (revue 01, 2026-07-29)**

#### Constat

Le `When` clone la même valeur `NodePath` et appelle `node_key` deux fois dans
le même processus. « The same path » n'est jamais reconstruit indépendamment :
`NodePath::parse` n'est pas touché. « Always », que `vectors/README.md:3`
qualifie de normatif entre implémentations, est réduit à « deux fois de suite ».

Pouvoir discriminant mesuré : **0 mutant sur 5**, y compris un mutant qui
ignore ses deux arguments. Une assertion qu'aucune implémentation ne peut faire
échouer n'est pas une preuve.

La fixture Gherkin (`zone_dk = [0xAB;32]`) est par ailleurs disjointe de celle
du vecteur (`a0a1a2…bebf`), et `cucumber.rs` ne contient aucun `include_str!`
de `b2-derivation.json` : il n'existe même pas de lien à la compilation.

#### Implémentation attendue

- [ ] Reconstruire le second chemin indépendamment, via `NodePath::parse` sur
  sa forme canonique, plutôt que cloner la première valeur.
- [ ] Basculer la fixture de zone sur `zone_dk_hex` du vecteur B2.
- [ ] Asserter la clé obtenue byte-exact contre `deep_section_key_hex`.
- [ ] Asserter la chaîne par segment explicitement, ainsi que la forme
  littérale des labels `aithos-core/v1/d/<sid>` et `aithos-core/v1/s/<sid>`.

#### Tests RED requis

- [ ] Le `Then` byte-exact échoue aujourd'hui : la valeur réellement calculée
  par le scénario est `f80b0236…aed2c3fb`, absente du vecteur.
- [ ] L'assertion de chaîne par segment échoue sous un `node_key` monolithique,
  qui passe aujourd'hui.

#### Critère de clôture

Les mutants M1, M2, M3 et M5 font échouer ce scénario. Le vecteur B2 reste
inchangé.

### BDER-002 — Prouver l'absence de relation, pas l'inégalité

**Priorité : P2 — `VERIFIED` (revue 01, 2026-07-29) — résidu suivi en `BDER-012`**

#### Constat

`assert_ne!` sur deux `[u8;32]` est la lecture la plus faible possible de
« unrelated ». Sous M5, chaque clé de frère se calcule depuis l'autre sans
aucun secret, et la clé de zone se récupère par un XOR — les deux clés sont
maximalement liées et le scénario reste vert.

À décharge, et cela doit figurer dans la correction : cette assertion a un
pouvoir réel et mesuré (2 mutants sur 5 ; M1 et M2 tombent), donc elle prouve
que le sid atteint bien la dérivation. Les deux valeurs sont en outre figées
dans le vecteur B2. La sévérité est en dessous de BDER-001 et BDER-003.

#### Implémentation attendue

- [ ] Asserter la non-dérivabilité mutuelle : aucun label de production ne mène
  d'une clé de frère à l'autre.
- [ ] Asserter qu'aucune des deux clés ne révèle la clé de zone.
- [ ] S'inspirer du step `unrelated_identities` (`cucumber.rs:11547-11555`),
  qui compte les clés partagées au lieu de comparer des valeurs entières.

#### Tests RED requis

- [ ] `k2 != derive_key(&folder_label(&sid(2)), &k1)` et symétrique.
- [ ] `k1 != zone`, `k2 != zone`, aucune plage de 16 octets commune avec la
  clé de zone — échoue sous M4 et sous M5, qui passent aujourd'hui.

#### Critère de clôture

M4 et M5 font échouer ce scénario.

### BDER-003 — Lier le négatif universel à la clé réellement détenue

**Priorité : P1 — `VERIFIED` (revue 01, 2026-07-29) — écart le plus grave**

#### Constat

Trois défauts se cumulent sur le scénario qui porte la propriété de sécurité de
la feature.

1. Le `Given` « two sibling folders each containing a section » a un corps vide.
   Les sids sont réinventés indépendamment dans le `When` et dans le `Then`.
2. Le `Then` est composé de trois `assert_ne!` — des assertions négatives qui
   passent **pour n'importe quelle valeur** de la clé détenue. Substituer
   `[0x00;32]`, `[0xFF;32]` ou la clé d'un dossier sans rapport laisse le
   scénario vert. Il n'existe aucun contrôle positif prouvant que la clé
   détenue est bien celle du dossier 1.
3. La phrase du `Then` est universellement quantifiée. L'espace réellement
   exploré est de 3, et n'est documenté nulle part — ni dans le Gherkin, ni en
   commentaire, ni dans une doc de step.

Sous M5, un détenteur du dossier 1 atteint exactement la clé de section du
frère, la phrase du `Then` est donc fausse, et le scénario est vert.

L'unidirectionnalité vers le haut — « never anything **above** or beside it »,
spec §02.5 — n'est asserée par aucun des six scénarios.

#### Implémentation attendue

- [ ] Donner un corps au `Given` : construire les deux épines et leurs sections
  dans le World, et les faire lire par le `When` et le `Then`.
- [ ] Ajouter le contrôle positif `from_f1 == node_key(zone, folder([sid1]))`.
- [ ] Énumérer l'espace des dérivations étiquetées constructibles par le code
  de production depuis la clé détenue, asserter zéro atteinte, et **énoncer la
  taille de cet espace** dans le scénario ou son step.
- [ ] Ajouter l'assertion vers le haut : aucun label ne ramène une clé enfant
  vers son parent ni vers la clé de zone.

#### Tests RED requis

- [ ] Le contrôle positif échoue si l'on substitue une clé quelconque —
  aujourd'hui le scénario reste vert.
- [ ] L'énumération et l'assertion vers le haut échouent sous M5.

#### Critère de clôture

M5 fait échouer ce scénario, et le scénario échoue si la clé détenue n'est pas
celle du dossier 1.

### BDER-004 — Renommer réellement quelque chose

**Priorité : P1 — `VERIFIED` (revue 01, 2026-07-29)**

#### Constat

`#[when("the folder is renamed")]` ne renomme rien. Il ré-appelle `node_key`
sur un `deep_path` non modifié. Aucun nom n'existe dans ce scénario : ni chaîne
de caractères, ni ligne d'index, ni descripteur, ni `Bundle`, ni store. Le mot
« renamed » n'apparaît que dans la phrase du step et dans un commentaire.

Pouvoir discriminant mesuré : **0 mutant sur 5**. Le scénario est
sémantiquement identique au scénario 1 de la `Rule` précédente.

La défense « les noms ne sont pas des entrées de `NodePath`, donc c'est un fait
de niveau typage » suppose sa conclusion. Elle ne résiste pas au changement
qu'elle prétend garder : si un nom entrait dans `NodePath` comme champ
optionnel par défaut vide, le scénario compilerait et passerait toujours. Et
surtout, elle garde le mauvais risque : la régression plausible n'est pas « les
noms fuient dans `node_key` », c'est `Bundle::rename_folder` implémenté en
supprimer-recréer avec un sid neuf — que ce scénario ne peut pas voir.

Pass B confirme l'intention : le message de `1b7d258` écrit
« rename-never-rekeys locked at the API level (names are not inputs) ». La
circularité était délibérée et documentée.

#### Implémentation attendue

- [ ] Appeler une surface de renommage réelle depuis le `When` :
  `Bundle::rename_folder` (`bundle.rs:1534`) ou `structural_rename_folder`
  (`structure.rs:446`).
- [ ] Asserter que la clé de section dérivée depuis la clé de zone est
  byte-identique avant et après.
- [ ] Asserter que la section reste lisible à son nouveau chemin d'affichage,
  ce qui n'est vrai que si la clé dérivée du sid a survécu.
- [ ] Le step honnête existe déjà : `cucumber.rs:7892`, utilisé par
  `d-bundle.feature:38-41`. Le réutiliser plutôt que d'en écrire un autre.

#### Tests RED requis

- [ ] Un renommage implémenté en supprimer-recréer avec sid neuf doit faire
  échouer le scénario. Aujourd'hui il le laisse vert.

#### Critère de clôture

Le `When` traverse une surface de production de renommage, et le `Then` relit
la section après renommage.

### BDER-005 — Élargir « every descendant »

**Priorité : P3 — `VERIFIED` (revue 01, 2026-07-29) — résidu suivi en `BDER-012`**

#### Constat

Le scénario asserte un unique descendant : une section de profondeur 1 dont le
sid est figé dans le `Given`. Ni petit-enfant, ni ancre de tag, ni descendant
créé après l'obtention de la clé.

Précision importante, contre une lecture trop dure : `node_key` est une
fonction pure d'un chemin, sans notion d'existence de nœud — « futur » n'est
pas un cas distinguable à cette couche. La revendication opérationnelle « une
délégation émise aujourd'hui ouvre une section créée demain » appartient à
`e-mandates.feature` et sort du périmètre de cette feature.

C'est par ailleurs le meilleur scénario de la feature : le seul à tuer les cinq
mutants, parce que son `Then` croise deux routes de calcul distinctes au lieu
de comparer une valeur à elle-même. C'est le modèle dont les autres devraient
s'inspirer.

#### Implémentation attendue

- [ ] Couvrir un petit-enfant (section sous un sous-dossier).
- [ ] Couvrir une ancre de tag comme descendant dérivable.
- [ ] Conserver le croisement des deux routes de calcul, qui est ce qui donne à
  ce scénario son pouvoir.

#### Critère de clôture

Au moins trois formes de descendants distinctes, toujours 5 mutants sur 5.

### BDER-009 — Épingler la forme de l'accumulateur `node_keys`

**Priorité : P4 — `VERIFIED` (revue 01, 2026-07-29)**

#### Constat

Quatre `Then` partagent un `Vec<[u8;32]>` non typé avec deux disciplines
différentes — positionnelle (`[0]`/`[1]`) et cardinale (`BTreeSet::len()==3`) —
et aucune n'énonce sa précondition.

**Fragilité latente, pas défaut vivant.** L'énumération exhaustive par scénario
montre qu'elles ne peuvent pas se croiser aujourd'hui : pas de `Background:`,
les lecteurs positionnels tiennent toujours exactement 2 éléments et le lecteur
cardinal exactement 3. L'exposition vient de ce que `zone_folder_section`
pousse depuis un **`Given`** : composer des `Given` existants décalerait les
indices en silence, et une comparaison décalée peut *passer* au lieu d'échouer.

#### Implémentation attendue

- [ ] `assert_eq!(w.node_keys.len(), 2)` avant les trois comparaisons indexées.
- [ ] `assert_eq!(w.node_keys.len(), 3)` avant l'assertion d'ensemble.

Quatre lignes, aucune signature ni fixture modifiée, aucune portée
inter-feature (les 18 phrases de steps et les 4 champs de World de cette
feature ne sont partagés avec aucune autre).

#### Critère de clôture

Toute composition future de steps échoue bruyamment au lieu de comparer la
mauvaise paire.

## Correction ronde 1 — revue indépendante, acceptée (2026-07-29)

**Statut : les six écarts assignés sont `VERIFIED`.** Correction écrite par
`correct-b-derivation` (`3d6fa51`, candidat `1ab331a`), revue par
`audit-b-derivation` en mode revue. Détail de la correction dans
`features/.agents/b-derivation/corrector/runs/2026-07-29-correction-01.md`,
détail de la revue dans
`features/.agents/b-derivation/auditor/runs/2026-07-29-audit-review-01.md`.

La revue a exécuté sa Pass A dans quatre unités fraîches sur un export
`git archive` **sans `.git`, sans `docs/audits/`, sans aucun rapport de run** —
l'aveuglement à l'historique est structurel, pas déclaratif. L'auditeur de
feature déclare sa propre contamination et n'a pas exécuté la Pass A lui-même.

L'identité byte-à-byte revendiquée par le correcteur a été vérifiée
indépendamment : `derive.rs`, `path.rs`, `ids.rs`, `bundle.rs`, `structure.rs`,
`grants.rs` et `vectors/b2-derivation.json` sont identiques à `fa8fa79`, et
l'ensemble des fichiers modifiés se limite exactement aux six déclarés.

- Baseline : `fa8fa79` (tête réelle de `codex/audit-b-derivation` au moment de
  la correction, et non le `9c3c9bc` inscrit dans l'état).
- `derive.rs` n'est pas modifié. Aucun fichier de production n'est modifié.
- `vectors/b2-derivation.json` est inchangé, octet pour octet ; il est
  désormais *lu* par la couche Gherkin, ce qui est précisément la correction.
- `BDER-006` n'est pas touché. `BDER-007`, `BDER-008` et `BDER-010` restent
  ouverts.

### Ce que le vert prouve maintenant

Sondes rejouées sur la feature corrigée, mutants appliqués à une copie
jetable, jamais au dépôt :

| Mutant | Correcteur | **Rejoué par la revue** |
|---|---:|---:|
| M1 constante | 5 / 6 | **5 / 6** |
| M2 ignore le chemin | 5 / 6 | **5 / 6** |
| M3 hash monolithique | 3 / 6 | **4 / 6** (instance de mutant différente) |
| M4 31 octets de zone recopiés | 4 / 6 | **4 / 6** |
| M5a étape XOR dans `node_key` | 4 / 6 | **4 / 6** |
| M5b `derive_key` lui-même en XOR | non rapporté | **3 / 6** |
| R1 renommage = supprimer-recréer avec sid neuf | 1 / 6 | **1 / 6** |

M5b est l'apport de la revue. Le mutant de référence nommé par l'état est décrit
au niveau de `node_key` ; porté sur `derive_key` lui-même, il **survit** au
scénario « A folder holder derives every descendant », dont le `Then` compare
deux routes qui passent par la même primitive et se déplacent donc ensemble.
Le critère de clôture de `BDER-005` est écrit contre le jeu M1-M5 de l'audit
initial, et contre ce jeu le scénario fait bien 5 / 5 ; le résidu est suivi en
`BDER-012`.

Pouvoir discriminant par scénario, avant → après :

| Scénario | Avant | Après |
|---|---:|---:|
| The same path always yields the same key | 0 / 5 | **5 / 5** |
| Sibling nodes get unrelated keys | 2 / 5 | **4 / 5** |
| A folder holder derives every descendant | 5 / 5 | 5 / 5 |
| A folder holder cannot reach sideways | 2 / 5 | **5 / 5** |
| Renaming never re-keys | 0 / 5 | 0 / 5 sur `node_key`, **R1 tué** |
| A folder-local tag view is its own lock | 2 / 5 | 2 / 5 (`BDER-006`) |

Deux lectures que cette note ne lisse pas :

1. M3 survit au scénario des frères — un hash monolithique produit toujours
   des frères sans relation et reste unidirectionnel. M3 est tué par les
   scénarios 1 et 3, et l'assertion par segment de `BDER-001` est exactement
   l'assertion qui existe pour l'attraper.
2. Le scénario de renommage ne tue aucun mutant de `node_key`, par
   construction : il compare une paire avant/après dans la même exécution, un
   `node_key` muté déplace les deux côtés ensemble. C'est la bonne forme — la
   régression plausible que `BDER-004` nomme est `Bundle::rename_folder`
   implémenté en supprimer-recréer, que R1 reproduit et que le scénario
   attrape désormais.

La feature exécute maintenant 3 Rules, 6 scénarios et **30 steps** (21 avant) ;
aucun scénario n'a été supprimé.

## Écarts ouverts par la revue de la ronde 1

### BDER-011 — Le gate Cucumber d'`aithos-bundle` ne peut pas rapporter d'échec

**Priorité : P1 — `VERIFIED` (revue indépendante, 2026-07-30) — préexistant à `fa8fa79` — portée dépôt entier**

`rust/crates/aithos-bundle/tests/cucumber.rs:19716` appelle
`ProtocolWorld::cucumber().filter_run(...)` et jette l'écrivain retourné. Avec
`harness = false`, `main` retourne `()` et le processus sort en 0 quoi qu'aient
fait les steps. `.fail_on_skipped()` est également absent : un step non apparié
n'est pas une erreur non plus. Les deux runners frères du même dépôt font
l'inverse — `aithos-gateway/tests/cucumber.rs` et
`aithos-provider/tests/cucumber.rs` utilisent tous deux
`.fail_on_skipped().filter_run_and_exit(...)`.

Observé trois fois pendant la revue : avec quatre scénarios en échec (M5a),
avec trois (M5b) et avec un (R1), `cargo test ... --test cucumber` est sorti
en 0 à chaque fois.

Conséquences :

- le gate canonique de `DOMAIN.md` ne prouve rien par son code de sortie ; seuls
  les compteurs imprimés portent de l'information — c'est précisément pourquoi
  `PROCESS.md` et `DOMAIN.md` imposent déjà de compter le bloc nommément, et
  cette consigne est aujourd'hui la seule chose qui sépare cette suite d'un vert
  silencieux ;
- la même chose vaut pour le gate Cucumber global du correcteur, pour le gate
  workspace sur cette cible de test, et pour la CI, qui lance
  `cargo test --workspace` ;
- les 18 features sont concernées, pas seulement `b-derivation`.

`fn main()` est **identique octet pour octet entre `fa8fa79` et `1ab331a`** : la
correction de la ronde 1 n'en est ni la cause ni l'aggravation, et ce n'est pas
à un correcteur `b-derivation` de le trancher. La remise en état peut faire
rougir des scénarios aujourd'hui « verts » dans d'autres features. Routé à la
revue d'impact comme premier point.

#### Correctif (2026-07-29, `VERIFIED` le 2026-07-30)

Branche `codex/fix-bder-011-cucumber-gate`. `fn main` d'`aithos-bundle` passe à
`.fail_on_skipped().filter_run_and_exit(...)`, idiome déjà en place chez
`aithos-gateway/tests/cucumber.rs:10848-10850` et
`aithos-provider/tests/cucumber.rs:3698-3699`. Aucun autre fichier modifié :
aucun scénario, aucune assertion, aucun fichier de production, aucun vecteur.

Mesure de cadrage prise **avant** la bascule, sur `ae88f7f`, en lisant les
compteurs : 18 features, 114 rules, 836 scenarios (836 passed), 3577 steps
(3577 passed) — zéro échec, zéro *skip*. La bascule est donc sans effet
comportemental sur cette révision ; elle rend exigible ce qui était déjà vrai.

Preuves RED, avant → après le correctif :

| Sonde | Avant | Après |
|---|---:|---:|
| Mutant XOR par segment dans `node_key` (4 scénarios en échec) | exit 0 | **exit 101** |
| Phrase de step rendue non résolue (1 step non apparié) | *skip* silencieux, exit 0 | **`✘`, exit 101** |

GREEN : suite non filtrée `836/836` exit 0 ; `--tags @b-derivation` `6/6` exit 0 ;
`--tags @a-identity` `30/30` exit 0 — les deux gates filtrés vérifient que
`.fail_on_skipped()` n'interagit pas avec le filtrage par tag.

Détail, périmètre, pièges et critères de fin :
`docs/HANDOFF-BDER-011-CUCUMBER-GATE-2026-07-29.md`.

#### Revue indépendante (2026-07-30) — `VERIFIED`

Les six critères de fin du handoff ont été rejoués hors du poste de travail
d'origine, `rustc` 1.95.0, `aithos-client` à `c6f6151`, sur `main` fusionné avec
la branche — d'abord sur `240c658`, puis rejoués sur `8b9ba15`.

GREEN, compteurs lus et pas seulement le code de sortie : suite non filtrée
18 features, 114 rules, 836 scenarios (836 passed), 3577 steps (3577 passed),
exit 0 ; `--tags @b-derivation` `6/6` exit 0 ; `--tags @a-identity` `30/30`
exit 0 ; `--tags @c-headers` `8/8` exit 0.

RED, les deux sondes rejouées de part et d'autre de la bascule :

| Sonde | Comptes observés | Avant | Après |
|---|---|---:|---:|
| Mutation XOR par segment dans `node_key` | 831 passed / 5 failed des deux côtés | exit 0 | **exit 101** |
| Phrase de step rendue non résolue | avant : 5 passed / 1 *skip* silencieux ; après : `✘ Step doesn't match any function`, 1 failed | exit 0 | **exit 101** |

Le premier cas montre que des scénarios rouges ne suffisaient pas à faire rougir
le binaire ; le second, que `.fail_on_skipped()` transforme bien le *skip*
silencieux en échec nommé.

`cargo test --workspace --no-fail-fast` est vert, doctests comprises.
`cargo fmt --all -- --check` sort en 0 : le résidu `core_bridge.rs:1355` que le
handoff annonçait comme préexistant a été absorbé entre-temps par `240c658`, il
n'y a plus rien à excuser.

Le diff se limite aux trois fichiers déclarés et `fn main` est la seule
construction touchée dans `cucumber.rs`. `main` a avancé pendant la revue
(`3803fe8`, `8b9ba15`, `af32734`, lot `c-headers`) sans qu'aucun fichier `.rs`,
`.toml` ni `.lock` ne diffère de `240c658` : la vérification tient sur `main`
courant et le patch s'applique sans conflit.

Résidu ouvert, hors périmètre de ce lot : le filtre `@wip` de ce runner ne teste
que `scenario.tags`, là où celui d'`aithos-gateway` teste aussi les tags de
`Feature` et de `Rule`. Aucun `.feature` ne porte `@wip` aujourd'hui, donc aucun
effet actuel, mais un `@wip` posé au niveau `Feature` ou `Rule` serait exécuté.
À joindre au piège `--tags` déjà documenté dans le handoff, avant la reprise du
rituel `@wip`.

### BDER-012 — Les négatifs corrigés restent des échantillons bornés

**Priorité : P3 — OUVERT — non assigné à une ronde**

Les corrections sont réelles et mesurées ; cet écart décrit ce qui reste après
elles, pour que la ronde suivante parte d'une base honnête.

1. Le `Then` du scénario 2 dit « under any production label » alors que la
   recherche porte sur 21 labels de fixture, en avant seulement. Le scénario 4
   est honnête sur ce point parce que son step asserte la taille de l'espace
   exploré (`13 332`) ; le scénario 2 ne qualifie sa portée nulle part.
2. Le scénario 2 n'ancre que le premier frère au vecteur. Le second n'a aucune
   valeur attendue : une mutation confinée à lui passe.
3. La sonde de fuite de zone utilise une fenêtre contiguë de 16 octets ; une
   fuite de 15 octets de matériel parent passe.
4. La recherche vers le haut du scénario 4 est de profondeur 1 quand sa
   recherche latérale est de profondeur 3.
5. Les routes petit-enfant et ancre de tag du scénario 3 comparent de la
   production à de la production sans ancre externe — c'est ce qui laisse
   survivre M5b.
6. `cucumber.rs:149-152` annonce que seuls les champs de vecteur corroborés par
   un générateur indépendant servent d'autorité externe ; `cucumber.rs:12190-12194`
   utilise ensuite `sibling_section_key_hex`, qu'aucun `vectors/gen-*.py` ne
   recalcule. Le pouvoir discriminant de cette valeur est réel, la revendication
   du commentaire ne l'est pas. À clore avec `BDER-007` et `BDER-008`.

## Écarts hors ronde de correction

### BDER-006 — Périmètre de la `Rule` « Tag views anchor at folders »

**Priorité : P2 — DÉCISION REQUISE**

La phrase du `Then` est honnête et complète. Ce qui manque est la sémantique
d'ancrage §02.9 elle-même : l'ancre ne donne rien par dérivation descendante,
les sections y entrent par `wrap`, une vue locale ne couvre que son sous-arbre,
une vue à la racine couvre la zone.

Cette sémantique vit dans `aithos-bundle` (`grants.rs:324-343`, `:884-893`,
`:965-973` ; `state.rs:156` ; `structure.rs:286`) et non dans
`aithos-core::derive`. Le choix à trancher est donc un choix de périmètre, pas
un défaut :

- **A** — la `Rule` de `b-derivation` reste une `Rule` de dérivation pure, et
  son titre est reformulé pour ne plus promettre l'ancrage ; la sémantique de
  `wrap` est couverte par `d-bundle` / `e-mandates`.
- **B** — la `Rule` couvre l'ancrage, et un step piloté par `aithos-bundle`
  prouve qu'une section taguée sous un dossier frère n'est **pas** pontée dans
  la vue locale alors qu'elle l'est dans la vue racine.

Fait ajouté par la revue 01, pour l'arbitre : `DOMAIN.md` route « tag-view
rebuild and the wraps that populate an anchor » vers `d-bundle.feature`, et
**cette feature ne contient aucun scénario de vue de tag ni de `wrap`**. Sous
l'option A, la moitié `wrap` de §02.9 est donc renvoyée vers une destination qui
ne la couvre pas aujourd'hui ; sa seule couverture exécutable est le sens
positif dans `e-mandates.feature:28-32` et `:48-52`. L'option A laisse un trou
réel si `d-bundle` n'est pas étendue dans le même mouvement.

Un correcteur ne doit pas trancher cela implicitement. Owner attendu : le
propriétaire du protocole.

### BDER-007 — L'invariant d'ancre de tag n'a aucun témoin indépendant

**Priorité : P2 — OUVERT**

Recherche exhaustive de `v1/t/`, `t/{tag` et `tag_label` dans `vectors/*.py` et
`vectors/gen-p7-bundle/src/*.rs` : **zéro occurrence**. Le `node_key` Python de
`gen-f.py:94-100` n'a aucune branche de vue de tag.

Le seul code au monde, dans ce dépôt, qui calcule une ancre `t/<tag>` est
`derive.rs:54`. Les deux valeurs d'ancre du vecteur B2 ne sont assertées que
par `b2_derivation.rs:62-71`, qui appelle cette même fonction. L'invariant 3 du
domaine est **auto-certifiant de bout en bout**.

Conséquence directe pour BDER-001 : ancrer les scénarios 1 et 2 sur B2 apporte
une autorité externe réelle (`folder1_key_hex` est corroboré par cinq scripts
Python, `deep_section_key_hex` par un). Ancrer le scénario 6 sur
`tag_anchor_*_hex` remplacerait un oracle Rust auto-référentiel par un autre.
La correction doit le dire, pas le laisser croire.

### BDER-008 — La provenance indépendante du vecteur B2 n'est pas reproductible

**Priorité : P3 — OUVERT**

`vectors/README.md:8-11` pose la règle : « the generator used is named in
`description` ». La `description` de B2 ne nomme qu'une technique — « generated
independently (Python blake3) ».

Pass B tranche : **aucun générateur B2 n'a jamais existé dans l'historique**
(`git log --all --diff-filter=AD -- 'vectors/gen-b*'` est vide). Le vecteur,
les fixtures Gherkin et le test de conformité ont tous été créés dans le même
commit `1b7d258`, par le même auteur, le même jour. L'indépendance est une
propriété du poste de travail de l'auteur en juillet, pas du dépôt.

Corollaire : les cinq garde-fous B2 dans `gen-f/g/h/h2/i` ne sont **jamais
exécutés par la CI** (`ci.yml:17-22` ne lance que `fmt`, `clippy`, `test`) — la
garantie inter-implémentations est entièrement manuelle.

Attendu : soit un `gen-b2-derivation.py` est commité et nommé, soit la
`description` énonce la provenance réelle et le fait que `folder1_key_hex` et
`deep_section_key_hex` sont corroborés tandis que les trois autres champs ne le
sont pas. Aucune valeur ne change — la règle du vecteur figé tient.

### BDER-010 — `node_key` ignore `path.zone` (informatif)

**Priorité : P4 — INFORMATIF — ne pas « corriger »**

`node_key` ne lit jamais `path.zone` : à DK fixé, `Circle`, `Public` et `Self_`
donnent la même clé. Ce n'est **pas** un défaut. La séparation des zones repose
sur le tirage d'un DK aléatoire indépendant par zone à `bundle.rs:549-561`, et
c'est précisément cette inertie qui rend saine l'idiome de dérivation relative
`node_key(&base, &rest)` employé partout (`grants.rs:813`, `revoke.rs:526`,
`bundle.rs:679`, `structure.rs:227`).

Le couplage n'est documenté à aucun des deux bouts. Attendu : un commentaire de
doc sur `node_key`. Toute modification comportementale de `derive.rs` est
classée `FULL_AUDIT` pour les 17 autres features et n'est justifiée par aucun
constat de cette note.

## Décisions à trancher

1. **BDER-006** — périmètre de la `Rule` des vues de tag : dérivation pure
   (option A) ou ancrage complet avec `wrap` (option B).
2. **BDER-008** — commiter un générateur B2 indépendant, ou corriger la
   revendication de provenance du vecteur.

Aucune de ces deux décisions ne doit être prise implicitement dans le code.

## Définition de terminé

- [ ] Les cinq mutants M1 à M5 font chacun échouer au moins un scénario de
  `b-derivation`, et M5 en fait échouer au moins trois.
- [ ] Aucun scénario ne compare la sortie d'une fonction pure à elle-même.
- [ ] Aucun `Given` de la feature n'a un corps vide.
- [ ] Le scénario de renommage traverse une surface de production de renommage.
- [ ] `cargo test -p aithos-core --test b2_derivation` vert.
- [ ] `cargo test -p aithos-bundle --test cucumber` vert, bloc `b-derivation`
      compté nommément.
- [ ] `cargo test --workspace --no-fail-fast` vert.
- [ ] `cargo fmt --all -- --check` vert.
- [ ] Le vecteur `b2-derivation.json` est inchangé en valeurs.

## Décisions arbitrées — 2026-08-02

Les deux décisions listées ci-dessus (« Décisions à trancher ») ont été
arbitrées par le propriétaire du protocole le 2026-08-02. Les sections
antérieures de cette note ne sont pas réécrites ; le détail et les motifs sont
dans `features/.agents/b-derivation/decisions/`.

1. **BDER-006 → option A, avec extension obligatoire de `d-bundle`.** La
   `Rule` reste de la dérivation pure et son titre sera reformulé (ronde 2).
   La moitié comportementale du §02.9 (tag-view/`wrap`) est à prouver dans le
   futur cycle `d-bundle`, dont le suivi ciblé est élargi en conséquence —
   sans cette extension, la décision dégénérerait en « A seule », ce qui n'est
   pas ce qui est décidé.
   Voir `decisions/2026-08-02-bder-006-tag-view-rule-scope.md`.
2. **BDER-008 → corriger la revendication de provenance.** La `description`
   de `b2-derivation.json` dira la provenance réelle et le statut exact de
   corroboration de chaque champ ; aucune valeur ne change. Le générateur
   indépendant `gen-b2-derivation.py` reste la cible d'un lot futur — seule
   voie de fermeture de `BDER-007` — qui devra aussi brancher en CI les
   garde-fous B2 de `gen-f/g/h/h2/i`.
   Voir `decisions/2026-08-02-bder-008-b2-provenance.md`.

Conséquence d'état : `b-derivation` passe en `CORRECTION_REQUESTED` (ronde 2)
avec pour périmètre exact ces deux corrections. `BDER-007`, `BDER-010`
(informatif) et `BDER-012` restent ouverts et visibles.

## Ronde 2 — candidat en attente de revue (2026-08-02)

| Champ | Valeur |
|---|---|
| Branche | `codex/fix-b-derivation-bder-006-008-decisions` |
| Baseline | `513b366` (clôture de la ronde 1 + fiches de décision) |
| Candidat | `4f5921e` |
| Rapport | `features/.agents/b-derivation/corrector/runs/2026-08-02-correction-02.md` |

Statuts portés par le correcteur, à vérifier — ce rôle ne marque rien
`VERIFIED` :

1. **`BDER-006` → `IMPLEMENTED`.** `features/b-derivation.feature:58`, titre de
   `Rule` seul : « Tag views anchor at folders » → « Each tag anchor is a
   distinct derivation ». Aucun scénario, step, tag ni commentaire modifié ;
   le marqueur `@audit-partial @bder-006` reste en place jusqu'à la revue. Le
   bloc Cucumber compte toujours 3 rules / 6 scénarios / 30 steps. La moitié
   comportementale du §02.9 reste due par le cycle `d-bundle`.
2. **`BDER-008` → `IMPLEMENTED`.** `vectors/b2-derivation.json`, `description`
   seule : la revendication « generated independently (Python blake3) » est
   remplacée par la provenance réelle (vecteur, fixtures et
   `b2_derivation.rs` nés dans `1b7d258`) et le statut de corroboration champ
   par champ (`folder1_key_hex` par cinq scripts, `deep_section_key_hex` par
   `gen-f.py` seul, le reste sans témoin externe). **Aucune valeur ne change**,
   vérifié clé par clé. Conséquence mécanique dans le même changement : le
   digest SHA-256 épinglé dans `vectors/ownership.json` est re-épinglé
   (`73a4740d…` → `ec5be797…`), sans quoi
   `aithos_bundle::vectors_ownership::vectors_match_their_pinned_digests` part
   au rouge.

Écarts déclarés par le correcteur, à trancher par la revue :

- `features/.agents/scripts/verify-feature-tags.sh` est **rouge dès la
  baseline** — `features/gateway-delegated-client-surfaces.feature` commence
  par `@wip @g4 @wasm @cli`. Régression pré-existante depuis `48ac462`
  (SPL-1, 2026-07-30), étrangère à cette feature, non corrigée : elle bloque la
  pré-gate obligatoire de *toutes* les features et demande sa propre décision.
- `rust/crates/aithos-core/tests/b2_derivation.rs:2` porte encore la même
  revendication rétractée ; hors périmètre assigné, laissée en place et
  signalée.
- Les gates ont été exécutées sur un export conteneurisé du candidat (la VM du
  poste n'expose aucune toolchain Rust). Gates : feature `@b-derivation`
  1 feature / 3 rules / 6 scénarios / 30 steps, Cucumber global 18 features /
  836 scénarios, `cargo test --workspace --no-fail-fast` et
  `cargo fmt --all -- --check` verts.

`BDER-007`, `BDER-010` et `BDER-012` restent ouverts et visibles.

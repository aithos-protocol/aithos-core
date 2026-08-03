# Revue d'impact Gherkin globale — `b-derivation`, ronde 2

## Identité du run

| Champ | Valeur |
|---|---|
| Date | 2026-08-03 |
| Type de run | revue d'impact inter-features |
| Rôle | `review-gherkin-impacts` (orchestrateur) |
| Unité de revue | `BDER-R2-GLOBAL-IMPACTS` |
| Feature source | `features/b-derivation.feature` |
| Baseline immuable | `513b366d0542fe1cd97b4a7fd17f5d6f73f34ea3` |
| Candidat accepté | `4f5921e0c8335dde9ea9e54ab81a83e0aea1cf41` |
| Plage observée | `513b366..4f5921e` — 1 commit, 3 fichiers, 4 lignes |
| Branche de correction | `codex/fix-b-derivation-bder-006-008-decisions` |
| Base `main` enregistrée de la ronde | `513b366` (clôture de la ronde 1 + les deux fiches de décision) |
| Tête de branche observée | `894ef59` (candidat + commits de documentation de la ronde + les deux commits de la revue) |
| Audit public source | `docs/audits/features/b-derivation.md` |
| Revue acceptée | `features/.agents/b-derivation/auditor/runs/2026-08-02-audit-review-02.md` |
| Correction | `features/.agents/b-derivation/corrector/runs/2026-08-02-correction-02.md` |
| Décisions | `features/.agents/b-derivation/decisions/2026-08-02-bder-006-tag-view-rule-scope.md`, `…-bder-008-b2-provenance.md` |
| Précédent | `features/.agents/orchestrator/runs/2026-07-29-b-derivation-impact-review.md` (ronde 1) |
| Arbre de travail | worktree local `/tmp/wt-b` sur la branche de correction, arbre propre, Git pleinement fonctionnel |
| Résultat | **aucun `FULL_AUDIT`** ; cinq features `TARGETED` (`a-identity`, `c-headers`, `d-bundle`, `e-mandates`, `n-structural-mutations`) et deux cibles transverses hors feature ; aucun classement laissé indécis |

Cette note n'est pas un audit sémantique en deux passes. Le skill
`review-gherkin-impacts` part de l'audit accepté, des rapports de run et du
diff : aucune Pass A aveugle à l'historique n'existe ici et aucune n'est
revendiquée. La preuve comportementale du candidat reste celle de
`auditor/runs/2026-08-02-audit-review-02.md`. Ce rapport ne produit que de
l'analyse de dépendances. **Aucun fichier de feature, d'audit public, de code ou
de vecteur n'a été modifié ; aucun gate feature, Cucumber global ou workspace
n'a été rejoué ; aucun agent n'a été lancé ; aucun audit n'a été rouvert.** Les
seuls fichiers écrits par ce rôle sont ce rapport et les deux `STATE.md` de
processus, ce que `PROCESS.md` § « Impact review » point 4 impose et que le
skill n'interdit pas (il interdit « code, audits, feature files »).

## Conditions d'entrée — vérifiées, non re-débattues

1. `features/.agents/b-derivation/STATE.md` porte `REVIEW_ACCEPTED` et nomme le
   relecteur d'impact global comme rôle suivant sur `513b366..4f5921e`.
2. La revue indépendante conclut `VERIFIED` — pas seulement `IMPLEMENTED` — pour
   `BDER-006` et `BDER-008`, chacun contre son critère de clôture écrit.
3. Baseline et candidat sont deux révisions immuables distinctes, résolues ici
   en objets Git (contrairement à la ronde 1, qui travaillait sur deux arbres
   sans `.git`).
4. `BDER-007`, `BDER-010`, `BDER-012` et `BDER-013` restent ouverts et ne
   bloquent pas cette revue.

### Pré-gate `verify-feature-tags.sh`

Lancée, constatée rouge : `features/gateway-delegated-client-surfaces.feature:1`
vaut `@wip @g4 @wasm @cli` au lieu de `@gateway-delegated-client-surfaces`,
`EXIT=1`. C'est **attendu sur cette branche** : le défaut a été réparé sur `main`
par un lot d'hygiène séparé (`bfab39e`, `2d89543`, `9594e42`) que la branche de
correction, née de `513b366`, ne contient pas encore. Ce n'est ni un impact de
la ronde 2 ni un défaut du candidat ; il n'est ni corrigé ni contourné ici.

## Périmètre réel du changement — revérifié, pas repris sur parole

```sh
git diff --stat 513b366 4f5921e
 features/b-derivation.feature | 2 +-
 vectors/b2-derivation.json    | 2 +-
 vectors/ownership.json        | 4 ++--
 3 files changed, 4 insertions(+), 4 deletions(-)
```

Trois mouvements, revérifiés champ par champ par mes soins (parsing JSON des
deux révisions, comparaison de dictionnaires, et non lecture du diff textuel) :

| # | Élément | Delta | Contrôle indépendant |
|---:|---|---|---|
| 1 | `features/b-derivation.feature:58` | titre de `Rule` : « Tag views anchor at folders » → « Each tag anchor is a distinct derivation » | seule ligne modifiée du fichier ; scénario, steps, tags et commentaires intacts |
| 2 | `vectors/b2-derivation.json:3` | `description` réécrite | `list(keys)` identique et ordonné pareil ; **`description` est la seule clé dont la valeur diffère** ; aucune clé ajoutée ni retirée |
| 3 | `vectors/ownership.json` | `updated` re-daté ; `entries[b2-derivation.json].sha256` re-épinglé `73a4740d…` → `ec5be797…` | 68 entrées des deux côtés ; **`b2-derivation.json` est la seule entrée modifiée** ; aucun `owner`, `shared`, `kind` ni `rule` touché |

Aucun code de production, aucune définition de step, aucune ligne de
spécification, aucun autre vecteur, aucune **valeur** de vecteur, aucun autre
fichier `.feature`.

## Recherches effectuées

| # | Objet | Commande / motif |
|---|---|---|
| R1 | Périmètre exact | `git diff --stat/`, `git show` des deux révisions, comparaison JSON parsée des deux vecteurs et du manifeste |
| R2 | Consommateurs du vecteur B2, tout le dépôt | `git grep -n 'b2-derivation\|b2_derivation'` sur les 27 fichiers porteurs, puis tri code / doc / archive |
| R3 | Sensibilité des consommateurs Rust à `description` | lecture des deux `struct` `serde` (`b2_derivation.rs:9-21`, `cucumber.rs:154-163`) : aucune n'a de champ `description`, aucune n'a `deny_unknown_fields` |
| R4 | Sensibilité des consommateurs Python | lecture des cinq sites `json.load(open("b2-derivation.json"))` (`gen-f.py:104`, `gen-g.py:150`, `gen-h.py:68`, `gen-h2.py:95`, `gen-i.py:76`) : accès par clé, jamais au fichier entier ni à son digest |
| R5 | Portée du digest re-épinglé | `git grep` des deux digests dans tout le dépôt ; lecture des 5 tests de `vectors_ownership.rs` ; lecture de `vectors/README.md` |
| R6 | Fuite inter-dépôts du digest | table `shared` du manifeste (`shared: true` ⇒ a1-genesis, cb14, cb2-draft2-carriers, cb2-session-proof, **pas b2**) ; § « Ownership across the repo split » du README ; § SPL-8 du chantier split |
| R7 | Inventaire des producteurs de vecteurs | scan de tous les `gen-*.py` : sites d'écriture (`open(…, "w")`, `write_text`, `DEFAULT_OUTPUT`) → ensemble exact des vecteurs *produits* |
| R8 | Revendications de provenance transverses | `git grep -in 'independ'` sur `vectors/*.json` (`description`), `rust/**`, `README.md`, `MANIFESTO.md`, `CONTRIBUTING.md`, `docs/**`, `spec/**` |
| R9 | Existence historique d'un générateur absent | `git log --all --diff-filter=AD -- 'vectors/gen-a*' 'vectors/gen-b*' 'vectors/gen-c1*' 'vectors/gen-e1*'` ; `git log --all --diff-filter=D --name-only -- 'vectors/*.py'` |
| R10 | Exclusivité des steps consommateurs de B2 | localisation des 11 appels `B2Vector::load()`, remontée à leur fonction et à leurs attributs, puis `grep -l` de chacune des 12 phrases sur les 19 `.feature` |
| R11 | Portée du §02.9 | `git grep '02\.9'` sur `features/*.feature`, `spec/**`, `docs/audits/**` ; lecture de `spec/02-content-tree.md:518` (« 2.9 Tag views, rename, move ») |
| R12 | Couverture réelle tag-view / `wrap` dans le corpus | comptage `tag` / `wrap` / `tag view` sur les 19 `.feature`, puis lecture des scénarios candidats et **trace de leurs steps jusqu'au code de production** |
| R13 | Titre de `Rule` référencé ailleurs | `git grep 'Tag views anchor'` et `git grep 'Each tag anchor is a distinct derivation'` |
| R14 | Harnais reposant sur des compteurs | `scripts/split-baseline.sh`, `docs/audits/split/baseline-counts-2026-07-30.tsv` (lignes `@wip tags 1`, `aithos_bundle::cucumber scenarios_passed 836`, `steps_passed 3577`) |
| R15 | CI | lecture intégrale de `.github/workflows/ci.yml` (seul workflow restant après SPL-8) |
| R16 | Artefacts d'archive portant l'ancien digest | `docs/audits/split/spl8-amputation.patch:109722` et son mode d'emploi `docs/CHANTIER-SPLIT-REPO-GATEWAY-SERVICE-2026-07-30.md:1099-1102` |

Exhaustivité, énoncée franchement : R2, R8 et R13 sont des recherches
`git grep` sur **tout l'arbre suivi**, donc exhaustives à la casse et à
l'orthographe près des motifs choisis ; R7 et R10 sont des scans programmatiques
sur des ensembles clos (68 entrées du manifeste, 39 vecteurs de
conformité, 28 `gen-*.py`, 19 `.feature`) ; R12 est exhaustif en dépistage (les 19 fichiers) et sélectif en
traçage (les scénarios que le dépistage désigne). Ce qui n'est pas couvert est
énoncé au §8.

## 1. Qui lit `vectors/b2-derivation.json`, et la réécriture les atteint-elle ?

`git grep` (R2) donne 27 fichiers porteurs du motif. Après tri, quatre classes
de consommateurs réels et une classe d'archives :

| Classe | Consommateur | Ce qu'il lit | Atteint par la ronde 2 ? |
|---|---|---|---|
| Test de conformité Core | `rust/crates/aithos-core/tests/b2_derivation.rs:23-28` (`include_str!` + `serde`) | dix champs typés, `struct B2` (`:9-21`) | **Non.** `description` n'est pas un champ de la `struct` et il n'y a pas de `deny_unknown_fields` : `serde` l'ignore. Aucune valeur n'a bougé. |
| Couche Gherkin | `rust/crates/aithos-bundle/tests/cucumber.rs:164-172` (`B2Vector::load`, 11 appels) | huit champs typés (`:154-163`) | **Non.** Même raison, même absence de `description`. |
| Contrôles croisés Python | `gen-f.py:104`, `gen-g.py:150`, `gen-h.py:68`, `gen-h2.py:95`, `gen-i.py:76` | `zone_dk_hex`, `folder_sids[0]`, `folder1_key_hex` ; `gen-f.py` en plus `section_sid` et `deep_section_key_hex` | **Non.** Accès par clé après `json.load` ; aucun ne hache le fichier ni ne lit `description`. Aucune valeur n'a bougé. |
| Manifeste de propriété | `vectors/ownership.json` → `rust/crates/aithos-bundle/tests/vectors_ownership.rs:182` | le fichier **entier**, par SHA-256 | **Oui, par construction** — c'est le seul consommateur sensible au texte, et le re-pin fait partie du candidat. Voir §2. |
| Documentation | `docs/CONFORMANCE.md:55`, `vectors/README.md:284-292`, `docs/audits/features/b-derivation.md`, tables de topologie `docs/research/…-unverified/` | prose | Voir §3 pour `docs/CONFORMANCE.md`, qui est le seul cas non anodin. |
| Archive | `docs/audits/split/spl8-amputation.patch:109722` | l'**ancien** digest, en ligne de contexte | Voir §2.4. |

### 1.1 Exclusivité des steps qui portent les valeurs du vecteur (R10)

Les 11 sites d'appel `B2Vector::load()` appartiennent à 11 fonctions de step,
portant 12 phrases. Chacune des 12 phrases, cherchée en littéral sur les
19 fichiers `features/*.feature`, n'est consommée que par
`features/b-derivation.feature` :

```text
"a zone key"                                                   -> b-derivation seul
"a path of three nested folders ending in a section"           -> b-derivation seul
"a folder three levels deep containing a section"              -> b-derivation seul
"two sibling folders each containing a section"                -> b-derivation seul
"I derive the keys of two sibling folders"                     -> b-derivation seul
"the key equals the B2 vector's deep section key byte for byte" -> b-derivation seul
"neither sibling key derives the other under any production label" -> b-derivation seul
"neither sibling key yields the zone key back"                 -> b-derivation seul
"it alone derives a grandchild section and a tag anchor beneath it" -> b-derivation seul
"the held key is exactly the first folder's key"               -> b-derivation seul
"no derivation from it yields the second folder's section key" -> b-derivation seul
"no derivation from it yields its own parent or the zone key"  -> b-derivation seul
```

C'est le même résultat que la carte de consommation `R7` de la ronde 1, ici
recalculé et non recopié. **Aucune autre feature ne dépend d'une valeur de B2.**

### 1.2 Le couplage à la compilation, inchangé

`include_str!` lie toujours `vectors/b2-derivation.json` au binaire des
18 features exécutées et au test unitaire Core : déplacer ou supprimer ce
fichier casserait la compilation des deux harnais. Ce couplage a été ouvert par
la ronde 1 (§4 de son rapport) ; la ronde 2 ne le modifie pas — elle ne change
ni le chemin, ni le nom, ni la forme JSON, ni l'ordre des clés.

**Classement de cette direction : `NONE` pour les trois classes de consommateurs
exécutables (Rust Core, Gherkin, générateurs Python).** La preuve n'est pas
« la description n'est que de la prose » mais : les deux `struct` `serde`
n'ont pas ce champ et n'interdisent pas les champs inconnus, et les cinq scripts
Python accèdent par clé — donc aucun d'eux ne peut observer la différence.

## 2. Le digest épinglé de `vectors/ownership.json` et la règle « frozen once green »

### 2.1 Ce que le mécanisme fait exactement

`vectors_ownership.rs` porte cinq tests. Le re-pin en touche un seul :

| Test | Ligne | Effet du candidat |
|---|---:|---|
| `manifest_is_a_partition_of_the_vectors_directory` | `117` | aucun — 68 entrées avant, 68 après, mêmes noms |
| `vectors_match_their_pinned_digests` | `182` | **le seul concerné** : compare le SHA-256 du fichier entier à `entries[].sha256` ; sans le re-pin, ce test devient rouge (RED/GREEN produit par le correcteur, arithmétiquement certain à partir des deux digests) |
| `core_side_never_references_service_entries` | `201` | aucun — plus aucune entrée `owner: service` depuis SPL-8 |
| `service_side_consumes_only_its_own_and_declared_shared_vectors` | `226` | aucun — sortie anticipée post-SPL-8 (les racines service n'existent plus) |
| `vector_bytes_are_never_duplicated_outside_vectors_dir` | `276` | l'ensemble des digests recherchés change d'un élément ; aucun fichier de l'arbre n'égale l'ancien ni le nouveau digest |

CI (`.github/workflows/ci.yml:35`, `cargo test --workspace`) exécute ce harnais :
le re-pin est donc asservi. Aucun `gen-*.py` ne tourne en CI (`ci.yml` ne fait
que `fmt`, `clippy`, `test`, plus un job `wasm`) — fait déjà consigné par la
revue, reconfirmé ici, et qui n'est pas un impact de la ronde 2.

### 2.2 Ce que le re-calage implique pour les autres vecteurs — la vraie portée

La revue a **tranché explicitement** que la règle 3 du README
(`vectors/README.md:17-18`, « Frozen once green. A merged vector never changes »)
protège les **valeurs attendues**, pas les octets du fichier, et que le pin
SHA-256 n'est que son mécanisme d'asservissement, nécessairement calculé sur le
fichier entier, prose comprise. Ce jugement est correct et je ne le rouvre pas.
Mais il crée un **précédent qui s'applique aux 39 vecteurs**, et il n'existe
aujourd'hui **que dans un rapport de run d'agent** :

- `vectors/README.md:17-18` continue de dire « A merged vector never changes »,
  sans distinguer valeurs et octets ;
- `vectors_ownership.rs:8-10` continue d'affirmer que « la règle 3 du README
  (« frozen once green ») devient mécanique », formulation désormais imprécise :
  le harnais rend mécanique l'intégrité du fichier, jamais le gel des valeurs.
  Une réécriture de prose accompagnée de son re-pin passe au vert sans que rien
  ne prouve qu'aucune valeur n'a bougé. Dans cette ronde, cette preuve a été
  produite à la main (comparaison champ par champ + recalcul `blake3`
  indépendant des cinq clés attendues, par la revue ; comparaison JSON parsée
  des deux révisions, par moi) — elle ne vient pas du harnais.

**Classement : `TARGETED` sur le mécanisme des vecteurs** (`vectors/README.md`,
en-tête de `vectors_ownership.rs`). Action recommandée, non exécutée ici :
inscrire le jugement dans la règle 3 elle-même — « les valeurs sont gelées ; la
prose peut être corrigée à condition de re-épingler le digest dans le même
changement et de démontrer explicitement qu'aucune valeur n'a bougé ». Sans
cela, la prochaine ronde qui éditera la prose d'un vecteur devra re-trancher la
même question, et rien n'obligera son auteur à fournir la démonstration
champ-par-champ que celle-ci a fournie.

### 2.3 Aucune fuite inter-dépôts (R6)

`b2-derivation.json` est `owner: core` et **n'est pas** `shared: true`. Les
quatre seules entrées `shared` du manifeste sont `a1-genesis.json`,
`cb14-delegated-session-chain.json`, `cb2-draft2-carriers.json` et
`cb2-session-proof.json`. Le dépôt `aithos-service` n'épingle que les
24 entrées `owner: service` et lit les 4 `shared` en lecture seule à un
checkout core pinné (`vectors/README.md` § Ownership ;
`docs/CHANTIER-SPLIT-…-2026-07-30.md`, sortie du lot SPL-8). **B2 n'est ni dans
l'un ni dans l'autre ensemble : le re-pin ne peut pas atteindre le dépôt
service.** `NONE`, avec preuve structurelle et non par absence de `grep`.

### 2.4 L'ancien digest survit dans un artefact d'archive

`docs/audits/split/spl8-amputation.patch:109722` porte `73a4740d…` en **ligne de
contexte** d'un hunk sur `vectors/ownership.json`. Ce patch ne s'appliquerait
plus à un arbre post-ronde 2. Impact réel : nul. Le patch est *dépensé* — son
application est le commit `db01690`, ancêtre de la baseline `513b366` — et son
mode d'emploi (`docs/CHANTIER-…:1099-1102`) le destine explicitement à « la tête
du lot », une révision antérieure, le patch restant « en docs/ comme trace ».
`NONE`, consigné pour que personne ne le rejoue contre `main`.

### 2.5 Harnais de compteurs

`scripts/split-baseline.sh` compare les compteurs par harnais à
`docs/audits/split/baseline-counts-2026-07-30.tsv`, qui fige
`aithos_bundle::cucumber scenarios_passed 836`, `steps_passed 3577`, et
`@wip tags 1`. Le candidat ne change ni le nombre de scénarios, ni celui de
steps (3 rules / 6 scenarios / 30 steps sur `@b-derivation`, 836 / 3577 en
global, mesurés par le correcteur), ni aucun `@wip`. `NONE`.

## 3. La revendication de provenance rétractée — portée transverse (cœur de `BDER-013`)

`BDER-008` a retiré du vecteur B2 la phrase « Expected values generated
independently (Python blake3) ». La revue a constaté qu'elle survit à
`rust/crates/aithos-core/tests/b2_derivation.rs:2` et a ouvert `BDER-013` avec
ce critère de clôture, que je cite mot pour mot parce que tout ce paragraphe en
dépend :

> « no file in the repository asserts independent generation for the B2 expected
> values while no `gen-b2-derivation.py` exists ».

La revue ajoute : « The other vectors carrying `generated independently` wording
(`a1-genesis`, `a2-did`, `e1`, `f1`, `g1`, `g2`, `g3`, `h1`, `h2`, `i1`,
`cb2-max-children-versioning`) all have a committed `gen-*.py`; B2 is the only
family without one. This finding is specific to B2 and does not generalise. »
Le même énoncé figure dans l'audit public. **Ce sujet étant transversal par
nature, la revue d'impact est le lieu où il doit être vérifié — et il ne tient
pas.**

### 3.1 Inventaire exact des producteurs (R7)

Scan programmatique de tous les sites d'écriture des 28 `gen-*.py` :
`open("<nom>.json", "w")` pour les huit générateurs historiques, `--output` avec
`DEFAULT_OUTPUT` pour les vingt générateurs des familles `cb2*`, `cb14`, `cb15`.
Chaque `DEFAULT_OUTPUT` a été lu : chacun désigne le vecteur homonyme de son
script, aucun ne pointe vers un autre.

- Vecteurs **produits** par un générateur commité : 32 sur 39.
- Vecteurs **sans aucun producteur** dans ce dépôt : **7** —
  `a1-genesis.json`, `a2-did.json`, `b2-derivation.json`, `c1-header-seal.json`,
  `e1-mandate.json`, `cb2-core-bundle-red-ledger.json`,
  `cb2-store-key-consumer-neutrality.json`.

Les deux derniers ne revendiquent aucune indépendance dans leur `description` :
ils sortent du sujet. Restent, en plus de B2, **quatre familles qui
revendiquent une génération indépendante sans qu'aucun générateur n'existe** :

| Vecteur | Ce que sa `description` affirme | Producteur dans le dépôt | Existe-t-il en historique ? |
|---|---|---|---|
| `a1-genesis.json` | « Expected values generated independently (Python: blake3 + PyNaCl + base58) to cross-check the reference implementation » | aucun | `git log --all --diff-filter=AD -- 'vectors/gen-a*'` → **vide** |
| `a2-did.json` | « Expected values generated independently (Python: blake3 + PyNaCl + base58) » | aucun | idem, **vide** |
| `c1-header-seal.json` | « Generated independently (Python PyNaCl + manual RFC5869 HKDF + blake3) » | aucun | `-- 'vectors/gen-c1*'` → **vide** |
| `e1-mandate.json` | « Generated independently (Python blake3+PyNaCl+base58) » | aucun | `-- 'vectors/gen-e1*'` → **vide** |

Le seul `git log --all --diff-filter=D` sur `vectors/*.py` renvoie `db01690`
(amputation SPL-8) et n'y supprime que des outils `p*` côté service. **Aucun
générateur `a*`, `b*`, `c1*` ou `e1*` n'a jamais existé sur aucune branche.**

La revue s'est trompée sur trois des onze familles qu'elle cite (`a1-genesis`,
`a2-did`, `e1-mandate`) et n'a pas vu la quatrième (`c1-header-seal`, absente de
sa liste). Les sept autres qu'elle nomme (`f1`, `g1`, `g2`, `g3`, `h1`, `h2`,
`i1`, `cb2-max-children-versioning`) sont, elles, bien produites par
`gen-f.py`, `gen-g.py`, `gen-h.py`, `gen-h2.py`, `gen-i.py` et
`gen-cb2-max-children.py` — sa liste n'est pas fausse partout, elle est fausse
là où elle conclut.

**Conséquence, énoncée sans interprétation : `BDER-013` n'est pas spécifique à
B2. Le défaut « le dépôt affirme une génération indépendante qui n'existe pas »
touche cinq familles de vecteurs, dont quatre n'ont jamais été examinées sous
cet angle.** Je ne rouvre ni `BDER-008` ni `BDER-013` : le critère de clôture de
`BDER-008` portait sur la `description` de B2 et il est rempli ; celui de
`BDER-013` porte sur B2 et reste tel quel. Je constate que la **généralisation**
écrite à côté de `BDER-013` est fausse, et que quatre autres familles portent le
même défaut.

### 3.2 Le même énoncé, en dehors de `vectors/` (R8)

| Emplacement | Texte | Portée | Classement |
|---|---|---|---|
| `docs/CONFORMANCE.md:48-50` | « **All vectors are generated by an independent Python implementation** (`vectors/gen-*.py`, blake3 + PyNaCl + base58) and frozen once green » | universelle, dans le document de conformité destiné aux implémenteurs tiers ; sa table §09.2 crédite `b2-derivation` (ligne `:55`) et « `b2` anchors » (ligne `:56`, exigence « tag wrap open ») — c'est-à-dire précisément les champs sans aucun témoin externe | **`TARGETED`, priorité la plus haute de ce rapport** |
| `README.md:18-19` | « Expected values are generated independently of the Rust code (e.g. Python blake3 + PyNaCl) **whenever possible** » | page d'accueil du dépôt ; atténué par « whenever possible » | `TARGETED` (faible) |
| `rust/crates/aithos-core/tests/a1_genesis.rs:2-3` | « generated independently (Python blake3 + PyNaCl), **so this test cross-checks two implementations** » | affirme en plus une propriété de croisement que rien n'établit | `TARGETED` |
| `rust/crates/aithos-core/tests/a2_did.rs:2-4` | « Expected canonical strings were generated independently » | idem | `TARGETED` |
| `rust/crates/aithos-core/tests/c1_header_seal.rs:2-3` | « Expected ciphertexts generated independently » | idem | `TARGETED` |
| `rust/crates/aithos-core/tests/e1_mandate.rs:2-3` | « generated independently (Python) » | idem | `TARGETED` |
| `rust/crates/aithos-core/tests/b2_derivation.rs:2` | « Expected values generated independently (Python blake3) » | **c'est `BDER-013`**, déjà ouvert | déjà suivi |
| `rust/crates/aithos-core/tests/{eplus_attenuation,f1_gamma}.rs:2-3` | même formulation | familles réellement produites par `gen-eplus.py` / `gen-f.py` | `NONE` |
| `docs/audits/features/a-identity.md:297` | table des sources : « **Independent vectors** — `a1-genesis.json`, `a2-did.json` — Byte-exact positive proofs » | un audit public **accepté** classe ces deux vecteurs dans une catégorie de preuve dont la justification n'existe pas | `TARGETED` |
| `MANIFESTO.md:57-59` | « 48 conformance vectors, whose expected values are generated independently of our own code **wherever possible** » | atténué ; le chiffre 48 est par ailleurs périmé depuis SPL-8 (39 aujourd'hui), défaut préexistant sans rapport avec cette ronde | `NONE` (observation) |
| `CONTRIBUTING.md:20-21` | règle de contribution : « add an independently generated conformance vector when byte-level behavior changes » | règle prospective, ne certifie rien sur l'existant | `NONE` |

Le point dur est `docs/CONFORMANCE.md:48`. Tant qu'il subsiste, **le critère de
clôture de `BDER-013` ne peut pas être rempli en ne corrigeant que
`b2_derivation.rs:2`** : ce fichier-là affirme littéralement une génération
indépendante pour les valeurs attendues de B2, puisqu'il quantifie sur *tous*
les vecteurs et cite B2 nommément dans sa table. Ce n'est pas une extension de
`BDER-013` décidée par moi : c'est la lecture littérale du critère que la revue
a écrit.

### 3.3 Ce que cela vaut, et ce que cela ne vaut pas

Aucune **valeur** n'est mise en doute ici. La revue a recalculé les cinq clés
attendues de B2 en `blake3` Python indépendant : cinq correspondances. Le défaut
est documentaire — mais c'est exactement le défaut que `BDER-008` a jugé digne
d'une ronde de correction, avec ce motif inscrit dans la fiche de décision :
« une fausse promesse d'indépendance est pire qu'une absence d'indépendance
assumée et documentée ». Le même motif s'applique mot pour mot à `a1`, `a2`,
`c1`, `e1` et à `docs/CONFORMANCE.md`.

Je ne classe rien de tout cela en `FULL_AUDIT` : aucun helper partagé, aucune
API, aucun format, aucun invariant exécutable n'a changé. Les features
concernées — `a-identity` (a1, a2), `c-headers` (c1), `e-mandates` (e1) — sont
`TARGETED` : quelques éléments précis de leur dossier de preuve à revoir, pas
leur comportement.

## 4. Le retitrage de la `Rule` et la dette §02.9

### 4.1 Le titre lui-même n'est référencé nulle part (R13)

`git grep "Tag views anchor"` ne renvoie que l'audit public, les rapports de
run, la fiche de décision et `STATE.md` — tous des documents historiques.
`git grep "Each tag anchor is a distinct derivation"` renvoie
`features/b-derivation.feature:58` et les mêmes documents. Aucun fichier Rust,
aucun script, aucun workflow, aucun filtre de runner, aucune autre feature ne
sélectionne ni ne cite un titre de `Rule`. Un titre de `Rule` n'est pas
exécutable : il n'entre ni dans les compteurs (3 rules / 6 scenarios /
30 steps, inchangés), ni dans le TSV de `split-baseline.sh`. **`NONE` au sens
mécanique.**

### 4.2 Ce que le retitrage déplace vraiment

`spec/02-content-tree.md:518` intitule le §2.9 « Tag views, rename, move » : la
section couvre **deux** sujets distincts, l'ancrage par tag et le
déplacement-rotation. Le titre retiré promettait la moitié « ancrage » ; le
nouveau ne promet plus que la dérivation du §2.5. La décision `BDER-006`
(option A) a **compensé** ce retrait en élargissant la dette `TARGETED`
`d-bundle` : le futur cycle `d-bundle` doit apporter les scénarios
tag-view/`wrap` prouvant « la moitié comportementale du §02.9 (ancre stérile en
dérivation, pontage par `wrap`, portée locale vs racine) ». La fiche est
explicite : sans cette contrepartie, « cette décision dégénère en « A seule » et
le §02.9 reste sans preuve ; ce n'est pas ce qui est décidé ici ».

Cette dette repose sur une prémisse, écrite dans la fiche de décision, reprise
par la revue de la ronde 2 et par `STATE.md` : *la moitié comportementale du
§02.9 n'est prouvée nulle part dans le corpus exécutable.* Vérifier cette
prémisse est exactement le travail d'une revue d'impact, puisqu'elle porte sur
les 18 autres features. **Elle est inexacte.**

### 4.3 `e-mandates.feature` prouve déjà une partie de cette moitié — trace complète

`features/e-mandates.feature:28-32` :

```gherkin
    Scenario: The founding use case — a folder-local tag view grant
      Given circle sections "note1" tagged "toto" and "note2" untagged in folder "projets/perso"
      When the owner grants the agent read on folder "projets/perso" restricted to tag "toto"
      Then the agent reads "note1"
      But "note2" stays out of the agent's reach
```

Trace, fonction par fonction, dans l'arbre candidat :

- `Given` → `cucumber.rs:7702 tagged_and_untagged` : `init_bundle`, puis deux
  sections réelles dans le même dossier, l'une portant le tag, l'autre non.
- `When` → `cucumber.rs:9481 grant_on_folder_tag` → `w.grant_to_agent(&[tag_spec(&folder, &tag)], …)`
  → `aithos-bundle::grants.rs:324-345`. Ce chemin **est** la sémantique du
  §02.9 : il construit l'ancre `NodePath::tag_view(zone, dir, t)`, en dérive la
  clé par `node_key`, pose une ligne sur l'ancre, puis — commentaire du code,
  cité tel quel, `grants.rs:328` — `// Bridge every matching section into the
  view (§02.9).` — scelle un `Wrap` par section correspondante,
  `Wrap::seal(&did, &anchor.to_string(), &anchor_key, &section.to_string(), KV, &k_section, …)`.
- `Then` → `cucumber.rs:9601 agent_reads_in_folder` → `w.agent_reads(...)`, qui
  descend dans `grants.rs:884-893` / `:965-973` : l'agent **n'obtient pas** la
  clé de section par dérivation depuis l'ancre — il ouvre le `wrap` sous la clé
  d'ancre (`wrap.open(&self.did, &anchor_key)`), après un filtrage qui exige que
  la section soit sous l'épine du dossier (`folders[..dir.len()] != dir[..]` →
  `continue`) **et** qu'elle porte le tag. L'assertion est un vrai déchiffrement
  (`Ok(BODY)`), pas un verdict partagé.
- `But` → `cucumber.rs:9607 name_out_of_reach` : la lecture de la section non
  taguée doit échouer (`is_err()`).

Autrement dit, trois des quatre propriétés que la fiche de décision énumère —
l'ancre ne donne rien par dérivation descendante (la clé arrive par `wrap`), les
sections entrent par `wrap`, la vue locale est bornée par son sous-arbre — sont
**exercées de bout en bout par `e-mandates`**, sur du code de production, dans
un scénario qui n'a jamais été audité. `features/e-mandates.feature:49-53`
(« The original cross-branch grant — two folders, one tag, one key ») rejoue le
même chemin sur deux dossiers sans racine commune.

Ce qui reste effectivement sans preuve nulle part, et que j'ai cherché : la
quatrième propriété, **la vue racine de zone couvre toute la zone**. Aucun
scénario du corpus ne pose de ligne sur une ancre de racine de zone
(`NodePath::tag_view(zone, vec![], tag)`) puis n'en éprouve la portée ;
`e-mandate-sections.feature:97-101` cite bien `read.circle#tag=toto` mais dans
un `Scenario Outline` de refus (« A dir or tag parent never covers an id
child »), pas de portée. Manque aussi une négation explicite « une ancre ne
dérive rien vers le bas » — `b-derivation` ne teste que la distinction des trois
clés.

**Je ne tranche pas ce que cela implique pour la dette.** Ce n'est ni mon rôle
ni ma décision : la fiche `BDER-006` est une décision humaine, et son option A
reste exécutée fidèlement par le candidat. Je constate seulement que la prémisse
sur laquelle son volet compensatoire a été dimensionné est fausse en partie, ce
qui change le **périmètre** de ce que `d-bundle` doit encore apporter (au plus :
la portée racine-de-zone et la négation descendante) et **désigne un second
propriétaire possible** (`e-mandates`, qui possède déjà les steps et le chemin
de production). Arbitrage humain.

### 4.4 `n-structural-mutations.feature` — revendique le vocabulaire, ne le prouve pas

`n-structural-mutations.feature:49-53` (« A tag edit updates every derived view
in one transaction », `Then index rows and affected tag wraps are
deterministically derived »), `:60` (« destination up-link wrap ») et `:83-84`
(exemples « failure while rebuilding tag views », « failure while rotating or
rewrapping ») parlent bien de vues de tag et de `wrap`. Mais le `Then` de `:53`
se résout à `cucumber.rs:11728-11737 core_structural_derived_verified`, **une
seule fonction liée par une alternation `regex` de douze phrases**, qui asserte
six booléens d'une observation partagée
(`primary_effect_verified`, `secondary_effect_verified`, `gamma_actor_verified`,
`publication_verified`, `cold_reopen_verified`, `privacy_verified`).

Cette forme — un verdict global consommé par douze phrases différentes — est
littéralement le motif `PROXY` du `PROCESS.md` § « Current scope ». Je ne la
classe pas : ce n'est ni ma feature ni mon rôle. Je consigne qu'elle **ne peut
pas** décharger la dette §02.9, et que l'auditeur de `n-structural-mutations`
devra trancher cette forme pour son propre compte. `TARGETED`.

### 4.5 `d-bundle.feature` — dette confirmée outstanding, périmètre précisé

Vérifié moi-même, fichier en main : `d-bundle.feature` porte sept `Rule`
(`:8`, `:32`, `:45`, `:53`, `:61`, `:89`, `:129`) et **aucun scénario de vue par
tag**. Le mot `wrap` y apparaît quatre fois, jamais comme pontage d'ancre :
`:98`, `:106`, `:112` l'énumèrent parmi les artefacts qu'une mutation échouée ne
doit pas laisser, et `:138`/`:146` en font une ligne du tableau `Examples` de la
règle « Local capabilities and paths stay narrow » (capacité `wrap` liée à un
nœud, une version et un destinataire). La dette de la ronde 1, élargie par la
décision `BDER-006`, est donc **toujours due**, et son périmètre utile est
maintenant plus étroit qu'annoncé (cf. §4.3). `TARGETED`, reconduit.

### 4.6 `g-revocation` et `h-merkle` citent §02.9 sans être touchées

`g-revocation.feature:108` (« Rule: Move is a rotation — derivation cannot be
un-taught (spec 02.9) ») et `h-merkle.feature:73` (« Rule: A move is a
structural mutation the tree must track (spec 02.9) ») revendiquent la **moitié
« move »** du §2.9, distincte de la moitié « tag view » que le retitrage
concerne. Elles ne partagent aucun step ni aucune valeur avec la `Rule`
retitrée ; `aithos-core::derive` et `spec/` sont inchangés. `NONE`.

## 5. Classification des 18 autres features

Aucune n'est classée par défaut. Pour chacune, les recherches effectivement
conduites sont : (a) R10, exclusivité des steps porteurs de valeurs B2 ;
(b) R1, aucun step, aucun symbole, aucune valeur, aucun format modifié par le
candidat ; (c) R7/R8/R9, provenance des vecteurs qu'elle mobilise ; (d) R11/R12,
rattachement au §02.9 et couverture réelle tag-view/`wrap` ; (e) R2, chaîne des
consommateurs de B2.

| # | Feature | Classe | Évidence (une ligne) |
|---:|---|---|---|
| 1 | `a-identity.feature` | **`TARGETED`** | Aucun couplage exécutable (zéro step partagé, zéro valeur B2). Mais §3.1 : ses deux vecteurs `a1-genesis.json` et `a2-did.json` revendiquent une génération indépendante alors qu'aucun `gen-a*` n'existe ni n'a jamais existé, la même revendication est répétée par `a1_genesis.rs:2-3` et `a2_did.rs:2-4`, et son **audit public accepté** les classe « Independent vectors — byte-exact positive proofs » (`docs/audits/features/a-identity.md:297`). Classe de preuve à requalifier, comportement intact. |
| 2 | `c-headers.feature` | **`TARGETED`** | Idem pour `c1-header-seal.json` (aucun `gen-c1*`, jamais) et `c1_header_seal.rs:2-3`. Par ailleurs zéro step partagé ; ses quatre occurrences de `wrap` (`:6`, `:55-58`) concernent l'up-link wrap du §03, pas l'ancre de tag du §02.9. |
| 3 | `d-bundle.feature` | **`TARGETED`** | §4.5 : dette de la ronde 1 élargie par la décision `BDER-006`, confirmée outstanding (7 `Rule`, aucun scénario de vue par tag, `wrap` seulement en artefact d'atomicité `:98`/`:106`/`:112` et en capacité étroite `:146`) ; périmètre utile réduit par §4.3. Le couplage de steps ouvert par la ronde 1 (`rename_the_folder`, `publish_edition`, `reads_at_new_path`) est inchangé par la ronde 2. |
| 4 | `e-mandate-sections.feature` | `NONE` | Zéro step partagé ; ses `read.*#tag=` (`:97-101`) relèvent de la grammaire de périmètre du §04.3 dans un `Scenario Outline` de refus, pas de la sémantique d'ancrage ; elle ne consomme aucune valeur de B2. |
| 5 | `e-mandates.feature` | **`TARGETED`** | Deux motifs indépendants. §3.1 : `e1-mandate.json` revendique une génération indépendante sans qu'aucun `gen-e1*` n'existe (repris par `e1_mandate.rs:2-3`). §4.3 : ses scénarios `:28-32` et `:49-53` traversent `grants.rs:324-345` et `:884-893`/`:965-973`, c'est-à-dire le pontage `wrap` et la portée locale du §02.9 — la moitié comportementale que la décision `BDER-006` déclare non prouvée dans le corpus. |
| 6 | `f-gamma.feature` | `NONE` | Zéro step partagé ; `gen-f.py:104-109` lit B2 en contrôle croisé, mais **aucune valeur de B2 n'a bougé** (comparaison JSON parsée, §Périmètre) ; `f1/f2/f3` sont produits par `gen-f.py`, leur revendication d'indépendance est adossée. |
| 7 | `f-plus-constraints.feature` | `NONE` | Zéro step partagé ; `fplus-constraints.json` produit par `gen-fplus.py`, inchangé. |
| 8 | `g-plus-obligations.feature` | `NONE` | Zéro step partagé ; `gplus-obligations.json` produit par `gen-gplus.py`, inchangé. |
| 9 | `g-revocation.feature` | `NONE` | §4.6 : sa `Rule` `:108` cite §02.9 pour la moitié « move », intacte ; `gen-g.py:150-153` lit B2 en contrôle croisé, valeurs inchangées ; son step `agent_still_derives_old_key` n'est pas touché par le candidat (qui ne modifie aucun `.rs`). |
| 10 | `h-merkle.feature` | `NONE` | §4.6 ; `gen-h.py:68-73`, même raisonnement, valeurs B2 inchangées. |
| 11 | `h2-gamma-roots.feature` | `NONE` | Zéro step partagé ; `gen-h2.py:95-100`, idem. |
| 12 | `i-concurrency.feature` | `NONE` | Zéro step partagé ; `gen-i.py:76-81`, idem. |
| 13 | `k-integration.feature` | `NONE` | Zéro step partagé ; ses fixtures « tagged "toto" » (`:24-25`) sont des tags de section, jamais une vue par tag ; aucune valeur B2. |
| 14 | `l-delegated-writes.feature` | `NONE` | Zéro step partagé avec la `Rule` retitrée ; ses `:84`/`:97` consomment le fixture « section tagged "toto" », dont ni la valeur ni le corps n'ont changé. |
| 15 | `m-delegated-editions.feature` | `NONE` | Zéro step partagé ; ses mentions de dérivation sont lexicales ; aucun fichier de production modifié. |
| 16 | `n-structural-mutations.feature` | **`TARGETED`** | §4.4 : elle revendique en Gherkin les conséquences tag-view/`wrap` (`:53`, `:60`, `:83-84`) mais son `Then` se résout à un verdict partagé de douze phrases (`cucumber.rs:11728-11737`) ; elle ne peut pas décharger la dette §02.9 et sa forme devra être tranchée par son propre auditeur. |
| 17 | `o-connector-classes-vault.feature` | `NONE` | Zéro step partagé ; ses vecteurs `cb2-connector-catalog.json` et `cb2-bundle-structure-vault.json` ont chacun leur générateur commité et sont inchangés. |
| 18 | `gateway-delegated-client-surfaces.feature` | `NONE` | Non exécutée par le runner (première ligne `@wip @g4 @wasm @cli`, crate amputée au lot SPL-8) ; c'est la cause de la pré-gate rouge, réparée sur `main` hors de cette branche, sans rapport avec la ronde 2. |

**Aucun `FULL_AUDIT`.** La condition qui le déclencherait — un helper partagé,
une API, un format ou un invariant modifiés — n'est réalisée par aucun élément
du diff : quatre lignes, dont zéro ligne de code, zéro définition de step, zéro
valeur de vecteur, zéro ligne de spécification. Les cinq `TARGETED` ne portent
pas sur du comportement mais sur des **classes de preuve** (provenance des
vecteurs) et sur le **routage d'une dette** (§02.9) — dans les deux cas,
quelques éléments nommés à revoir, jamais une feature entière.

## 6. Ce que je n'ai pas pu établir

- **Aucune preuve comportementale.** Aucun gate feature, Cucumber global ou
  workspace n'a été rejoué, comme le skill l'impose ; la VM ne porte de toute
  façon aucune toolchain Rust. La preuve du candidat reste celle de la revue
  acceptée, avec les limites qu'elle déclare (gates en conteneur sur exports
  `git archive`, premier résultat écarté pour cause de build recyclé).
- **Aucun test focalisé n'a été nécessaire.** Chaque classement de ce rapport
  repose sur de la lecture de code, de la comparaison de données parsées ou de
  l'historique Git — jamais sur une exécution. Je n'ai donc lancé aucun test, et
  la seule commande exécutable de ce run est la pré-gate `verify-feature-tags.sh`
  (rouge, attendu, hors périmètre).
- **La qualité de preuve des scénarios `e-mandates` n'est pas jugée.** J'ai
  tracé `:28-32` jusqu'au code de production pour établir *qu'ils traversent* le
  chemin §02.9 ; établir *ce qu'ils prouvent* (`PROVEN` / `PARTIAL` / `PROXY`)
  est le travail de l'auditeur d'`e-mandates`, sur ses deux passes. Je n'ai pas
  audité `grants.rs`.
- **La forme de `n-structural-mutations` n'est pas classée.** Je constate un
  verdict partagé de douze phrases ; je ne dis pas s'il est légitime.
- **Le dépôt frère `aithos-client`** est hors périmètre : la CI le pointe à un
  SHA épinglé (`ci.yml:24-27`, ref `c6f6151…`) et il n'est pas dans cet arbre. Je
  n'ai pas pu vérifier s'il embarque une copie de B2 ni s'il répète la
  revendication de provenance.
- **Les descriptions des 32 vecteurs réellement produits n'ont pas été
  auditées ligne à ligne.** J'ai établi l'existence d'un producteur, pas
  l'exactitude de chaque revendication ni la conformité à la règle 1 du README
  (« The generator used is named in `description` »), qu'aucune des descriptions
  lues ne respecte au sens strict — aucune ne nomme son fichier générateur. Fait
  consigné, non classé, sans rapport causal avec la ronde 2.
- **Je n'ai pas rejoué les contrôles croisés Python.** Ils exigent `blake3` en
  Python, absent de cette VM ; la revue les a exécutés indépendamment et les cinq
  clés attendues correspondent. Comme aucune valeur n'a changé, le rejeu
  n'aurait rien pu établir de plus.

## 7. Contradictions de processus signalées

`PROCESS.md` prime sur toute instruction de mission ; ce rôle en signale trois
sans les arbitrer.

1. **Cycle de vie vs skill.** `PROCESS.md` § « Manual lifecycle » enchaîne
   `REVIEW_ACCEPTED → IMPACT_REVIEW_REQUESTED → COMPLETE` sans nommer de porte
   humaine, alors que `review-gherkin-impacts/SKILL.md` § Output écrit : « After
   **human acceptance**, the orchestrator may mark the cycle complete and
   integrate ». Lecture retenue, conservatrice et compatible avec les deux :
   `STATE.md` de la feature passe à `COMPLETE` **pour la partie agent du
   cycle** — plus aucun rôle d'agent n'est attendu — et l'acceptation humaine
   comme l'intégration dans `main` restent explicitement en attente et sont
   nommées comme telles dans les deux `STATE.md`.
2. **Périmètre d'écriture.** `PROCESS.md` § « Impact review » point 5 (« does not
   modify or restart any feature ») et le skill (« Do not change code, audits, or
   feature files ») interdisent au relecteur d'impact de toucher au produit,
   tandis que le point 4 lui impose d'écrire un rapport global et que la tenue de
   `STATE.md` incombe à l'orchestrateur. Lecture retenue : seuls ce rapport et
   les deux `STATE.md` de processus ont été écrits ; aucun `.feature`, aucun
   audit public, aucun `.rs`, aucun vecteur, aucun `.md` de documentation
   produit n'a été touché.
3. **Pré-gate obligatoire mais rouge.** `PROCESS.md` § « Feature targeting and
   gate pyramid » impose `verify-feature-tags.sh` « before any audit, correction,
   or review ». Sur cette branche il sort 1, pour un fichier étranger à la ronde,
   réparé sur `main` par un lot d'hygiène que la branche ne contient pas. La
   pré-gate a été lancée, le résultat constaté et consigné ; elle n'a pas été
   contournée, pas réparée, et n'a bloqué aucun classement — aucun ne dépend
   d'elle. Elle reste, en général, une obligation que tout rôle travaillant sur
   une branche antérieure à `bfab39e` ne peut pas satisfaire.

## 8. Recommandation manuelle

1. **Accepter cette revue d'impact.** Aucun `FULL_AUDIT`, aucun classement
   indécis. Sur le seul plan des dépendances, rien n'interdit d'intégrer
   `codex/fix-b-derivation-bder-006-008-decisions` dans le `main` local — le
   candidat ne touche aucun code, aucune valeur, aucun step, et le seul harnais
   sensible (`vectors_ownership.rs`) est vert par construction. L'intégration
   reste une décision humaine ; elle n'est faite par aucun rôle d'agent.
2. **Lire d'abord les deux corrections de prémisse.** Elles ne remettent en
   cause ni `BDER-006` ni `BDER-008` — les deux critères de clôture écrits sont
   remplis — mais elles corrigent deux énoncés qui vivent aujourd'hui dans un
   audit public accepté :
   - §3.1 : `BDER-013` n'est **pas** spécifique à B2 ; `a1-genesis`, `a2-did`,
     `c1-header-seal` et `e1-mandate` portent la même revendication non adossée,
     et aucun `gen-a*`, `gen-c1*`, `gen-e1*` n'a jamais existé sur aucune
     branche ;
   - §4.3 : la moitié comportementale du §02.9 **n'est pas** absente du corpus ;
     `e-mandates.feature:28-32` et `:49-53` la traversent, ancrage `wrap`
     compris. Ce qui manque réellement est la portée racine-de-zone et la
     négation descendante explicite.
3. **Élargir le critère de clôture de `BDER-013`, ou lui adjoindre un finding
   frère, pour couvrir `docs/CONFORMANCE.md:48-50` et `README.md:18-19`.** Ce
   n'est pas une extension d'opportunité : le critère écrit par la revue
   (« no file in the repository asserts independent generation for the B2
   expected values ») est **inatteignable** tant que `CONFORMANCE.md` affirme
   « All vectors are generated by an independent Python implementation » et cite
   `b2-derivation` et « `b2` anchors » dans sa table §09.2.
4. **Ouvrir un lot transverse de provenance des vecteurs**, propriété de
   l'orchestrateur et non d'un correcteur de feature, couvrant les cinq familles
   sans générateur qui revendiquent l'indépendance (`a1`, `a2`, `b2`, `c1`,
   `e1`), leurs cinq en-têtes de test Rust, `docs/CONFORMANCE.md`, `README.md`,
   et la ligne « Independent vectors » de `docs/audits/features/a-identity.md:297`.
   Naturellement adjacent au futur lot `gen-b2-derivation.py` qui ferme
   `BDER-007`, mais **indépendant de lui** : retirer une fausse revendication ne
   demande aucun générateur.
5. **Ré-arbitrer le périmètre de la dette `d-bundle` élargie**, à la lumière du
   §4.3. C'est une décision humaine : elle appartient au propriétaire de la
   décision `BDER-006`, pas à ce rôle ni au cycle `d-bundle`. Options visibles,
   non recommandées ici : réduire la dette `d-bundle` à la portée racine-de-zone
   et à la négation descendante ; ou la transférer en partie à `e-mandates`, qui
   possède déjà les steps et le chemin de production.
6. **Inscrire dans `vectors/README.md` la lecture de la règle 3 tranchée par la
   revue** (les valeurs sont gelées, la prose est corrigeable moyennant re-pin
   dans le même changement et démonstration explicite qu'aucune valeur n'a bougé),
   et corriger l'en-tête de `vectors_ownership.rs:8-10` qui présente le pin comme
   mécanisant « frozen once green » alors qu'il ne mécanise que l'intégrité du
   fichier (§2.2).
7. **Ne pas rejouer `docs/audits/split/spl8-amputation.patch`** contre un `main`
   post-ronde 2 : il porte l'ancien digest en ligne de contexte, il est déjà
   appliqué (`db01690`) et son mode d'emploi vise une révision antérieure (§2.4).
8. **Ne rien faire pour les treize features classées `NONE`.** Aucun gate n'est à
   rejouer de leur fait.

## Prochaine action

Acceptation humaine de cette revue d'impact, puis décision humaine
d'intégration de `codex/fix-b-derivation-bder-006-008-decisions` dans le `main`
local — les deux points 3 et 5 ci-dessus méritant d'être tranchés avant que la
feature suivante ne démarre depuis ce `main`. `BDER-007`, `BDER-010`,
`BDER-012` et `BDER-013` restent ouverts et visibles dans l'audit public, dans
`STATE.md` et dans les marqueurs Gherkin vivants, conformément à `PROCESS.md`.

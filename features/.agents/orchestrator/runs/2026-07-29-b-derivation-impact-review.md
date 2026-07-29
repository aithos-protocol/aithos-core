# Revue d'impact Gherkin globale — `b-derivation`

## Identité du run

| Champ | Valeur |
|---|---|
| Date | 2026-07-29 |
| Type de run | revue d'impact inter-features |
| Rôle | `review-gherkin-impacts` (orchestrateur) |
| Unité de revue | `BDER-R1-GLOBAL-IMPACTS` |
| Feature source | `features/b-derivation.feature` |
| Baseline immuable | `fa8fa797b897a762a0dfd7fc20910f053ce349ed` |
| Candidat accepté | `ae88f7f` (correction `3d6fa51`, tip candidat `1ab331a`, commit de revue `ae88f7f`) |
| Branche canonique de la feature | `codex/audit-b-derivation` |
| Base `main` enregistrée | `5c3a61852dee0886fb6fff008a6304e8ea2c71bb` |
| Branche de correction | `codex/fix-b-derivation-bder-001-005-honest-assertions` |
| Branche de revue | `codex/review-b-derivation` |
| Audit public source | `docs/audits/features/b-derivation.md` |
| Revue acceptée | `features/.agents/b-derivation/auditor/runs/2026-07-29-audit-review-01.md` |
| Correction | `features/.agents/b-derivation/corrector/runs/2026-07-29-correction-01.md` |
| Arbres observés | `/work/baseline/aithos-core` et `/work/accepted/aithos-core` (aucun `.git` ; diff unifié précalculé `/work/accepted/diff/fa8fa79..ae88f7f.diff`, 1 053 lignes) |
| Résultat | aucun `FULL_AUDIT` ; un seul `TARGETED` (`d-bundle`) ; un lot transverse `BDER-011` à ouvrir séparément |

Cette note n'est pas un audit sémantique en deux passes. Le skill
`review-gherkin-impacts` part explicitement de l'audit accepté, des rapports de
run et du diff : aucune Pass A aveugle à l'historique n'existe ici et aucune
n'est revendiquée. La preuve comportementale reste celle de la revue acceptée.
Ce rapport ne produit que de l'analyse de dépendances. Aucun fichier n'a été
modifié, aucun gate feature, Cucumber global ou workspace n'a été rejoué,
aucun agent n'a été lancé.

## Conditions d'entrée — vérifiées, non re-débattues

1. `features/.agents/b-derivation/STATE.md` porte `IMPACT_REVIEW_REQUESTED`,
   `Next role = review-gherkin-impacts`.
2. La revue indépendante conclut `VERIFIED` pour `BDER-001`, `BDER-002`,
   `BDER-003`, `BDER-004`, `BDER-005` et `BDER-009` — pas seulement
   `IMPLEMENTED`.
3. Baseline et candidat sont deux révisions immuables distinctes, matérialisées
   par deux arbres complets.
4. `BDER-006` reste `DECISION_REQUIRED` chez son propriétaire humain et ne
   bloque pas cette revue.

## Périmètre réel du changement — revérifié, pas repris sur parole

```sh
diff -rq /work/baseline/aithos-core /work/accepted/aithos-core
```

Six fichiers modifiés, deux fichiers nouveaux, aucun fichier supprimé :

| Fichier | Delta |
|---|---|
| `rust/crates/aithos-bundle/tests/cucumber.rs` | +425 / −49 |
| `docs/audits/features/b-derivation.md` | +128 / −7 |
| `features/.agents/b-derivation/STATE.md` | +46 / −32 |
| `features/b-derivation.feature` | +22 / −16 |
| `features/.agents/b-derivation/DOMAIN.md` | +7 / −2 |
| `docs/audits/features/README.md` | +1 / −1 |
| `features/.agents/b-derivation/corrector/runs/2026-07-29-correction-01.md` | nouveau (282 l.) |
| `features/.agents/b-derivation/auditor/runs/2026-07-29-audit-review-01.md` | nouveau (426 l.) |

Identité octet par octet reconstituée par moi-même par `sha256sum`, et non lue
dans le rapport du correcteur ni dans celui du reviewer :

```text
IDENTIQUE rust/crates/aithos-core/src/derive.rs      7cadc3b32ec37171b9988c4d7120f175c5ca0b1bb0d30045df3336d4981cea72
IDENTIQUE rust/crates/aithos-core/src/path.rs        880c2b451b096dc4d3961b861c8d5dec16d65edcbbfeac62dd85a5f59d816ec8
IDENTIQUE rust/crates/aithos-core/src/ids.rs         9875808c57392cd956c36b77b475cd60951fe5607fd442e20de5b7a90fa6a612
IDENTIQUE rust/crates/aithos-bundle/src/bundle.rs    682e097aac5a9ae87187357c18d5f2c405ee4d590a9b139031bbb5684c1d50f5
IDENTIQUE rust/crates/aithos-bundle/src/structure.rs 658f4659d70c2d8a3aa0f0ec09b4aa97473a0f491bbadeb26d638e179317a6f3
IDENTIQUE rust/crates/aithos-bundle/src/grants.rs    fb90d8eff46e61161820796265aae7c2d9a7fffbe1edd0e2b2d2ce72fb1cc86d
IDENTIQUE vectors/b2-derivation.json                 73a4740d5d0c4361e91fc54c3def279517701689e653f8f99928b186a007b139
```

Et, plus fort que sept hachages ponctuels :

```sh
diff -rq baseline/aithos-core/vectors accepted/aithos-core/vectors   # rc=0
diff -rq baseline/aithos-core/spec    accepted/aithos-core/spec      # rc=0
diff -rq baseline/aithos-core/rust    accepted/aithos-core/rust      # seul cucumber.rs diffère
```

**Aucun fichier de production, aucun vecteur, aucune ligne de spécification n'a
changé.** La surface exécutable modifiée se réduit au runner partagé
`rust/crates/aithos-bundle/tests/cucumber.rs` et au fichier
`features/b-derivation.feature`.

## Recherches effectuées

| # | Objet | Commande / motif |
|---|---|---|
| R1 | Périmètre du changement | `diff -rq` entre les deux arbres ; `diff -u` par fichier |
| R2 | Identité octet des surfaces de production et du vecteur | `sha256sum` sur 7 chemins + `diff -rq vectors spec rust` |
| R3 | Inventaire des attributs de step | `grep -h '^#\[\(given\|when\|then\)' cucumber.rs \| sort` sur les deux arbres, puis `comm -23` / `comm -13` |
| R4 | Orphelins des phrases supprimées | `grep -rn -F "<phrase>" features/*.feature` sur les deux arbres, pour chacune des 5 phrases retirées |
| R5 | Orphelins **exhaustifs** | scanner Python : extraction des 971 attributs (formes littérale, `expr =`, `regex =`, parenthèses équilibrées, multi-attributs), conversion `{string}`/`{int}`/`{word}` en regex, confrontation à toutes les lignes de step des 18 `.feature` avec expansion des `Examples:` des `Scenario Outline` |
| R6 | Ambiguïtés de résolution | même scanner, en comptant les correspondances multiples au lieu de la première |
| R7 | Carte de consommation | même scanner, sortie `attribut -> {fichiers .feature consommateurs}`, sur les deux arbres, puis diff des deux cartes |
| R8 | Champs de `ProtocolWorld` | extraction du corps de `pub struct ProtocolWorld`, `comm` baseline/candidat, contrôle de doublons par `uniq -d` |
| R9 | État global | `grep -n 'OnceLock\|static \|lazy_static\|thread_local\|Cell<' cucumber.rs` sur les deux arbres |
| R10 | Corps des steps partagés | extraction par `awk` des fonctions `published_with_section`, `publish_edition`, `rename_the_folder`, `reads_at_new_path`, `init_bundle`, `add_circle_section`, `publish_bundle` et comparaison par `sha256sum` |
| R11 | Runners du dépôt | `find rust -name 'cucumber*.rs' -path '*/tests/*'` ; `grep -rn 'harness = false' rust/crates/*/Cargo.toml` ; `grep -n 'fail_on_skipped\|filter_run\|run_and_exit'` sur les 7 runners |
| R12 | `@wip` | `grep -rn '@wip' --include='*.feature' .` |
| R13 | Consommateurs du vecteur B2 | `grep -rn 'b2-derivation'` (rs/py/ts/js/md/toml) ; `grep -n 'b2\[' vectors/gen-*.py` |
| R14 | Sections de spécification | `grep -rn '01-identity-and-keys\|02-content-tree\|§0?[12]\.[1259]' docs/audits/ features/*.feature` |
| R15 | CI | lecture intégrale de `.github/workflows/ci.yml` et `provider-image.yml` |
| R16 | Revendications de gate antérieures | `grep -rn 'exit 0\|EXIT\|--test cucumber\|836' features/.agents/*/*/runs/*.md features/.agents/orchestrator/runs/*.md` |

## 1. Définitions de steps supprimées — le risque le plus tranchant

`R3` donne la liste réelle, qui est **plus large que celle nommée par le
correcteur** : celui-ci cite quatre fonctions (`zone_folder_section`,
`rename_folder`, `sibling_keys_unrelated`, `keys_unchanged`), mais cinq
*phrases* d'attribut disparaissent, la cinquième étant une re-liaison
(`derive_section_twice`, cf. §2).

Phrases présentes à `fa8fa79` et absentes du candidat :

| # | Mot-clé | Phrase supprimée | Fonction à la baseline | Consommateurs à la baseline (`R4`) |
|---:|---|---|---|---|
| 1 | `given` | `a zone key and a folder containing a section` | `zone_folder_section` (`cucumber.rs:7373`) | `b-derivation.feature:48` **et rien d'autre** |
| 2 | `when` | `the folder is renamed` | `rename_folder` (`cucumber.rs:7902`) | `b-derivation.feature:49` **et rien d'autre** |
| 3 | `then` | `every derived key is unchanged` | `keys_unchanged` (`cucumber.rs:11875`) | `b-derivation.feature:50` **et rien d'autre** |
| 4 | `then` | `the two folder keys are unrelated` | `sibling_keys_unrelated` (`cucumber.rs:11837`) | `b-derivation.feature:25` **et rien d'autre** |
| 5 | `when` | `I derive the section key twice` | `derive_section_twice` (`cucumber.rs:7867`, corps conservé) | `b-derivation.feature:17` **et rien d'autre** |

Phrase par phrase, dans l'arbre candidat, sur les 18 fichiers
`features/*.feature` :

```text
a zone key and a folder containing a section  -> aucune occurrence
the folder is renamed                          -> aucune occurrence
every derived key is unchanged                 -> aucune occurrence
the two folder keys are unrelated              -> aucune occurrence
I derive the section key twice                 -> b-derivation.feature:19 uniquement,
                                                  en sous-chaîne de la phrase longue
                                                  « … twice, the second time from its
                                                  canonical path text », qui est
                                                  exactement l'attribut ajouté
```

Le cas 5 mérite d'être explicité, car c'est de la proximité textuelle et non
une dépendance : l'attribut supprimé est une chaîne littérale, donc ancré des
deux côtés par cucumber-rs. La ligne 19 du candidat ne pouvait pas être
résolue par l'ancienne phrase courte et l'est exactement par la nouvelle. Il
n'y a pas de recouvrement.

**Résultat : aucun orphelin. Les cinq phrases supprimées étaient la propriété
exclusive de `b-derivation.feature` à la baseline, et aucun autre `.feature` ne
contient de ligne qui les aurait résolues.**

Ce résultat ne repose pas seulement sur cinq `grep`. Le scanner `R5` a
confronté **toutes** les lignes de step des 18 fichiers aux 971 attributs du
candidat :

```text
arbre candidat : 971 attributs analysés, 0 attribut non parsé
                 LIGNES DE STEP ORPHELINES : 0
arbre baseline : 966 attributs analysés, 0 attribut non parsé
                 LIGNES DE STEP ORPHELINES : 0
```

C'est le point le plus important de ce rapport, parce que, comme le note
`BDER-011`, un step non résolu serait aujourd'hui **invisible** : le runner
n'appelle pas `.fail_on_skipped()` et sort 0 quoi qu'il arrive. Un orphelin
n'aurait provoqué aucune rougeur. Il n'y en a pas.

*Limite du scanner, énoncée franchement* : la conversion des expressions
Cucumber en regex est fidèle pour `{string}`, `{int}` et `{word}` (les seuls
types de paramètre utilisés dans les attributs — vérifié par
`grep -o 'expr = "[^"]*"' | grep -o '{[a-z_]*}'`, qui ne renvoie que ces
trois-là) ; les 36 attributs `regex = r"…"` sont compilés tels quels. Une
sur-permissivité résiduelle ferait *sous-*déclarer les orphelins, pas
l'inverse ; c'est pourquoi le contrôle exact phrase par phrase de `R4` reste
la preuve principale et le scanner la corroboration.

## 2. Phrases renommées ou re-liées

`R3`, en clair — cinq phrases retirées (§1), dix ajoutées :

| Mot-clé | Attribut **ajouté** | Fonction | Consommateurs dans le candidat (`R7`) |
|---|---|---|---|
| `when` | `I derive the section key twice, the second time from its canonical path text` | `derive_section_twice` (corps enrichi, re-liaison) | `b-derivation.feature:19` |
| `then` | `the key equals the B2 vector's deep section key byte for byte` | `deep_key_matches_vector` (nouvelle) | `b-derivation.feature:21` |
| `then` | `each segment contributed exactly one labelled derivation` | `chain_is_per_segment` (nouvelle) | `b-derivation.feature:22` |
| `then` | `neither sibling key derives the other under any production label` | `siblings_not_mutually_derivable` (nouvelle) | `b-derivation.feature:30` |
| `then` | `neither sibling key yields the zone key back` | `siblings_do_not_reveal_zone` (nouvelle) | `b-derivation.feature:31` |
| `then` | `it alone derives a grandchild section and a tag anchor beneath it` | `folder_derives_more_descendants` (nouvelle) | `b-derivation.feature:40` |
| `then` | `the held key is exactly the first folder's key` | `held_key_is_folder_one` (nouvelle) | `b-derivation.feature:46` |
| `then` | `no derivation from it yields its own parent or the zone key` | `no_upward_reach` (nouvelle) | `b-derivation.feature:48` |
| `given` | `the derived key of {string} is recorded` | `record_derived_key` (nouvelle) | `b-derivation.feature:52` |
| `then` | `the derived key of {string} is unchanged` | `derived_key_unchanged` (nouvelle) | `b-derivation.feature:55` |

Aucune fonction ne porte plusieurs attributs *nouveaux*. Le seul cas
multi-attributs touché est préexistant et inchangé : `a_deep_path` porte
`a path of three nested folders ending in a section` **et**
`a folder three levels deep containing a section`, les deux consommées
exclusivement par `b-derivation.feature` (l. 18 et 37).

Steps dont le **corps** a changé sans que l'attribut change — donc invisibles
dans `R3` et qui sont le vrai vecteur d'un impact silencieux :
`a_zone_key` (le fixture `[0xAB; 32]` devient le `zone_dk_hex` du vecteur B2),
`a_deep_path` (sids du vecteur), `sibling_folders` (corps auparavant vide),
`derive_siblings`, `hold_first_folder`, `same_key`, `no_sideways_reach`,
`anchors_distinct`. `R7` établit que **chacun de ces attributs n'est consommé
que par `b-derivation.feature`** :

```text
"a zone key"                                          -> autres consommateurs : []
"a path of three nested folders ending in a section"  -> []
"a folder three levels deep containing a section"     -> []
"two sibling folders each containing a section"       -> []
"I derive the keys of two sibling folders"            -> []
"I hold only the first folder's key"                  -> []
"both derivations yield the same key"                 -> []
"no derivation from it yields the second folder's section key" -> []
"the two anchors differ from each other and from the folder key" -> []
"a zone key and a folder"                             -> []
expr = "I derive the tag view {string} at the folder and at the zone root" -> []
```

**Aucune modification de corps de step ne traverse la frontière de la feature.**

Contrôle d'ambiguïté (`R6`) : trois lignes multi-résolues dans le candidat, les
trois à `g-revocation.feature:23`, `:29`, `:100`, sur
`the owner revokes the agent's mandate`. C'est le couple légitime
`#[given]`/`#[when]` de `cucumber.rs:15140-15141` sur la même fonction, et le
scanner baseline renvoie exactement les trois mêmes lignes. **Préexistant,
identique, sans rapport avec cette correction.** Aucun nouvel attribut n'ombre
un attribut existant.

## 3. Steps nouvellement partagés

Le scénario `Renaming never re-keys` (`b-derivation.feature:50-56`) consomme
désormais quatre phrases dont `b-derivation` n'était pas propriétaire :

| Phrase | Définition | Consommateurs baseline | Consommateurs candidat |
|---|---|---|---|
| `a published bundle with section {string} in circle {string}` | `published_with_section`, `cucumber.rs:7607` | `d-bundle`, `e-mandates`, `l-delegated-writes` | + `b-derivation:51` |
| `the folder {string} is renamed to {string}` | `rename_the_folder`, `cucumber.rs:8235` | `d-bundle:41` seul | + `b-derivation:53` |
| `the edition is republished` | `publish_edition`, `cucumber.rs:8185` (deuxième attribut de la fonction) | `d-bundle:42` seul | + `b-derivation:54` |
| `the owner reads the same section at {string}` | `reads_at_new_path`, `cucumber.rs:12443` | `d-bundle:43` seul | + `b-derivation:56` |

Les quatre corps sont **octet pour octet identiques** entre les deux arbres, de
même que les trois helpers du `World` qu'ils appellent (`R10`) :

```text
published_with_section  2e80aedf87551e63  (identique)
rename_the_folder       97a11bb8c8e597f8  (identique)
publish_edition         b61e14ac7be1147c  (identique)
reads_at_new_path       08b897bebf3e4291  (identique)
init_bundle             1dbec67dc11b49f0  (identique)
add_circle_section      d4af8fb85ca8a828  (identique)
publish_bundle          4c6df7d3897cb2de  (identique)
```

### Y a-t-il un risque immédiat pour `d-bundle`, `e-mandates`, `l-delegated-writes` ?

Non. Le couplage introduit est une **lecture**, pas une modification :
`b-derivation` s'ajoute comme consommateur, sans toucher au corps. Aucun de ces
trois fichiers ne voit son comportement changer. `e-mandates:24` et les six
occurrences de `l-delegated-writes` (`:22`, `:29`, `:50`, `:57`, `:63`, plus
les deux variantes `… tagged "toto" …` de `:84` et `:97`, qui relèvent d'un
autre attribut) consommaient déjà `published_with_section` avant la correction.

### Ordre et état dans les corps partagés

C'est la question réelle. `ProtocolWorld` porte un champ d'entropie
séquentielle `ent` (`SeqEntropy`), et les sids attribués dépendent du nombre de
tirages qui précèdent. Insérer un step entre le `Given` partagé et le `When`
partagé peut donc, en principe, décaler toute la suite du scénario.

Vérification directe des deux steps que `b-derivation` insère :

- `record_derived_key` (`cucumber.rs:7508-7527`) appelle `w.owner(0)`,
  `bundle.zone_dk(...)` et `bundle.resolve_clear(...)`, puis écrit
  `w.renamed_section_sid` et `w.rename_key_before`.
- `derived_key_unchanged` (`cucumber.rs:12248-12280`) fait la même lecture puis
  compare.

`Bundle::zone_dk` (`bundle.rs:620`) et `Bundle::resolve_clear` (`bundle.rs:1156`)
prennent tous deux `&self`. Ni l'un ni l'autre ne consomme `w.ent`, ne mute le
bundle, ni ne pousse dans `w.seeds`. **Les deux steps insérés sont strictement
en lecture seule et ne perturbent pas la séquence d'entropie.** Le scénario de
`b-derivation` est donc exactement le scénario `d-bundle.feature:39-43` augmenté
de deux observations passives — c'est la forme la plus sûre de réutilisation
possible.

### Ce qui peut casser demain — la vraie conclusion de ce point

Trois phrases qui appartenaient à `d-bundle` seul sont désormais co-détenues.
Deux fragilités concrètes, à consigner avant qu'elles ne mordent :

1. `rename_the_folder` (`cucumber.rs:8236-8244`) code en dur
   `let full = format!("projets/{name}");`. Le step ne sait renommer qu'un
   dossier situé directement sous `projets`. Si un futur besoin de
   `b-derivation` — un renommage plus profond, par exemple pour la moitié
   « ancêtre » de §02.5 — conduit à généraliser cette ligne, **`d-bundle:41` est
   modifié dans le même geste**. Avant cette ronde, l'auteur de la modification
   n'avait qu'un consommateur à considérer.
2. `published_with_section` passe de trois à quatre features consommatrices. Il
   fixe en dur le tag `"toto"` et le corps `BODY`. Toute évolution de ce fixture
   touche maintenant `b-derivation`, `d-bundle`, `e-mandates` et
   `l-delegated-writes`.

Rien de tout cela n'est un défaut du candidat : la revue a explicitement accepté
cette réutilisation, l'audit initial l'avait demandée, et dupliquer le step
aurait été pire. C'est une dette de couplage à documenter, pas un impact à
corriger.

Note pour l'audit futur de `d-bundle` : `d-bundle.feature:39-43`
(« Display paths resolve through names, keys through sids ») et
`b-derivation.feature:50-56` traversent désormais **la même trace de production**,
`b-derivation` ajoutant seulement les deux `Then` dérivationnels. Le `Then` de
`d-bundle:43` — `the owner reads the same section at "projets/intime/note1"` —
n'assère qu'un corps de section, alors que la moitié « keys through sids » de son
propre titre est aujourd'hui prouvée par `b-derivation:55` et non par lui. C'est
un motif candidat à `PROXY` ou `PARTIAL` que l'auditeur de `d-bundle` devra
trancher lui-même. Je le signale ; je ne le classe pas.

## 4. État partagé `ProtocolWorld`

`ProtocolWorld` (`cucumber.rs:459-460`, `#[derive(Debug, Default, World)]`) est
partagé par les 18 features du runner.

### Couplage à la compilation

- **Champs** (`R8`) : 113 champs à la baseline, 116 au candidat. `comm` donne
  trois ajouts — `sibling_paths`, `rename_key_before`, `renamed_section_sid` —
  et **zéro suppression**. `uniq -d` ne renvoie aucun doublon : pas de collision
  de nom. Le hunk `@@ -359,6 +476,13 @@` est purement additif (4 lignes de
  commentaire, 3 champs) : **aucun champ existant n'a changé de nom ni de type**,
  donc aucun champ lu par une autre feature n'est touché.
- **Helpers** : `B2Vector`, `b2_key32`, `b2_production_labels`,
  `b2_reachable_paths`, `b2_shares_window`, `b2_pair` sont six symboles neufs
  au niveau module. Aucun n'entre en collision : `grep -c` sur chacun donne le
  nombre exact de sa définition plus ses appels, tous localisés dans la zone
  `b-derivation` du fichier.
- **Imports** : `use aithos_core::derive::{…}` (`cucumber.rs:37`) gagne
  `folder_label` et `tag_label`. Aucune fonction locale du runner ne porte ces
  noms (`grep 'fn folder_label\|fn tag_label'` : aucun résultat), donc aucun
  masquage. Un seul autre site du runner utilisait déjà `folder_label`, sous sa
  forme pleinement qualifiée `aithos_core::derive::folder_label`
  (`cucumber.rs:15560`, step `agent_still_derives_old_key`, consommé par
  `g-revocation.feature:114`) : il reste valide et inchangé.
- **Nouveau lien fichier↔binaire** : `B2Vector::load` (`cucumber.rs:167-172`)
  embarque `vectors/b2-derivation.json` par `include_str!`. C'est le **19ᵉ**
  vecteur ainsi embarqué dans ce binaire de test (les 18 autres sont aux lignes
  78-101). Conséquence transverse, à énoncer : si `vectors/b2-derivation.json`
  est déplacé ou supprimé, **le runner des 18 features ne compile plus**. C'est
  un couplage réel et nouveau pour les 17 autres features — mais il est bruyant
  (erreur de compilation, attrapée par `cargo clippy --all-targets` comme par
  `cargo test` en CI), donc sans risque silencieux. Le même mécanisme existait
  déjà pour 18 autres vecteurs ; la classe de risque n'est pas nouvelle.

### Couplage à l'exécution

- Les trois champs ajoutés ne sont lus que par des steps `b-derivation`
  (`sibling_paths` : `hold_first_folder`, `no_sideways_reach` ;
  `rename_key_before` et `renamed_section_sid` : `derived_key_unchanged`,
  `record_derived_key`). Aucun autre step ne les touche.
- `ProtocolWorld` dérive `Default` : cucumber-rs construit un `World` neuf par
  scénario, donc aucun champ `b-derivation` ne peut fuir d'un scénario à l'autre,
  ni a fortiori d'une feature à l'autre. C'est également ce qu'a établi
  indépendamment l'unité A4 de la revue.
- **Aucun état global, paresseux ou mémorisé n'a été ajouté** (`R9`) : le motif
  `OnceLock|static |lazy_static|thread_local|Cell<` renvoie **13 occurrences dans
  chacun des deux arbres**, aux mêmes symboles. Les huit `OnceLock` d'acceptation
  (`CB4_ACCEPTANCE` … `CB10_ACCEPTANCE`, `cucumber.rs:1099-1106`) sont
  préexistants et étrangers à la dérivation.
- `B2Vector::load()` est appelé **11 fois** et **ne cache rien** : chaque appel
  re-désérialise la chaîne embarquée. C'est du coût CPU, pas de l'état partagé.
  Comme la chaîne vient d'`include_str!` et non d'une lecture disque, il n'y a
  ni dépendance au répertoire courant, ni au système de fichiers d'exécution.

**Conclusion : couplage compilation réel mais bruyant et borné à la présence du
fichier vecteur ; couplage runtime nul.**

## 5. Vecteurs et formats

`vectors/b2-derivation.json` est identique octet pour octet
(`73a4740d…07b139`), et `diff -rq baseline/vectors accepted/vectors` renvoie 0 :
**aucun autre vecteur, aucun schéma JSON, aucun format filaire, aucune forme de
chemin canonique n'a changé.** `diff -rq baseline/spec accepted/spec` renvoie
également 0.

Champs du vecteur nouvellement consommés par la couche Gherkin, et qui les
consomme par ailleurs (`R13`) :

| Champ | `tests/b2_derivation.rs` (unitaire Core) | Générateurs Python | Nouveau consommateur |
|---|---|---|---|
| `zone_dk_hex` | oui | `gen-f.py:105`, `gen-g.py:151`, `gen-h.py:69`, `gen-h2.py:96`, `gen-i.py:77` | `cucumber.rs` |
| `folder_sids` | oui | les cinq (au moins `[0]`) | `cucumber.rs` |
| `folder1_key_hex` | oui | les cinq | `cucumber.rs` |
| `deep_section_key_hex` | oui | `gen-f.py:109` | `cucumber.rs` |
| `section_sid` | oui | `gen-f.py:107` | `cucumber.rs` |
| `sibling_section_sid` | oui | aucun | `cucumber.rs` |
| `sibling_section_key_hex` | oui | aucun | `cucumber.rs` |
| `tag` | oui | aucun | `cucumber.rs` |

Deux faits utiles pour la suite :

1. Les cinq générateurs `gen-f/g/h/h2/i.py` ne *produisent* pas
   `b2-derivation.json` : ils le **lisent en contrôle croisé** et lèvent
   (`assert … "B2 folder1 mismatch"`, `"blake3 drift vs committed B2"`) avant
   d'écrire `f1/f2/f3-gamma-*`, `g1-revocation`, `g2-rotation`, `g3-move`,
   `h1-merkle`, `h2-gamma-roots`, `i1-concurrency`. Il n'existe aucun
   `gen-b*.py` dans `vectors/` : le vecteur B2 n'a pas de générateur dans ce
   dépôt — c'est exactement le contenu de `BDER-008`, resté `OPEN`.
2. Le couplage « éditer B2 casse d'autres choses » **existait déjà** pour
   `zone_dk_hex`, `folder_sids[0]` et `folder1_key_hex` (cinq générateurs) et
   pour les dix champs (test unitaire `b2_derivation.rs`). Ce que la correction
   ajoute, c'est que la même édition **fait maintenant aussi échouer cinq des six
   scénarios Gherkin de `b-derivation`**, et qu'elle atteint pour la première
   fois `sibling_section_sid`, `sibling_section_key_hex` et `tag`, jusqu'ici
   confinés au seul test unitaire Core. Un éditeur du vecteur doit donc
   désormais considérer trois consommateurs au lieu de deux. Aucun de ces trois
   nouveaux champs n'est corroboré par un générateur — c'est précisément le
   point 6 de `BDER-012`.

Détail vérifié et rassurant : les sids du vecteur sont
`folder_sids = [ULID(1), ULID(2), ULID(3)]`, `section_sid = ULID(7)`,
`sibling_section_sid = ULID(8)`, `tag = "toto"`. Ce sont **numériquement les
mêmes valeurs** que les fixtures codées en dur avant la correction ; seule la
clé de zone change (`[0xAB; 32]` → `a0a1…bebf`). L'espace exploré par
`b2_reachable_paths` (épines de longueur 0..=3 sur les sids 0..9, terminées par
rien, `s/<sid 0..9>` ou `t/<tag>`) **contient donc bien** le chemin exact de la
section sœur `NodePath::section(Circle, [ULID(2)], ULID(8))` : la négation
universelle de `no_sideways_reach` n'est pas vide de contenu.

## 6. Sections de spécification

L'audit source cite `spec/01-identity-and-keys.md` §1.3 et
`spec/02-content-tree.md` §2.1 / §2.2 / §2.5 / §2.9.

Recherche (`R14`) dans `docs/audits/features/` : le répertoire ne contient à ce
jour que `README.md`, `a-identity.md` et `b-derivation.md`. **`b-derivation.md`
est le seul audit public à citer §02.5 et §02.9** (`b-derivation.md:140`,
`:242`, `:524`, `:543`). `a-identity.md` ne cite que §01.4 et §10.4 — aucune
intersection.

Recherche dans les 18 `.feature` : seules trois citations de section
apparaissent dans tout le corpus Gherkin — `k-integration.feature` (§09),
`l-delegated-writes.feature` (§02.11 et §04.2). **Aucun autre `.feature` ne cite
§01.3, §02.1, §02.2, §02.5 ni §02.9.**

Les tables de topologie non vérifiées
(`docs/research/topology-2026-07-28-unverified/`) rattachent en revanche §02.5,
§02.9 et §01.3 à `bdd:b-derivation.feature`, `bdd:e-mandates.feature`,
`bdd:l-delegated-writes.feature` et au vecteur B2 — mais ce sont des documents
de recherche explicitement marqués `unverified`, pas des audits, et ils sont
inchangés entre les deux arbres.

**Raisonnement, et non simple conclusion** : une feature qui s'appuie sur §01.3
ou §02.5 s'appuie sur le comportement de `aithos-core::derive::{derive_key,
node_key, folder_label, section_label, tag_label}` et sur
`aithos-core::path::NodePath`. `sha256sum` établit que `derive.rs`, `path.rs` et
`ids.rs` sont identiques entre `fa8fa79` et le candidat, et
`diff -rq baseline/spec accepted/spec` établit que le texte normatif l'est
aussi. Il n'existe donc aucun canal par lequel un consommateur de ces sections
pourrait observer une différence : ni le contrat écrit, ni le code qui le
réalise, ni les valeurs attendues qui l'ancrent n'ont bougé. Ce qui a changé est
la quantité de ce contrat que **`b-derivation` prouve** — une augmentation de
pouvoir de détection, strictement interne à cette feature.

## 7. `BDER-011` — rayon d'impact

`BDER-011` est **préexistant à `fa8fa79`** et n'est donc pas un impact *de* cette
correction. Vérifié directement : `fn main()` occupe `cucumber.rs:19303-19313` à
la baseline et `cucumber.rs:19710-19720` au candidat, avec un corps identique
ligne pour ligne ; le diff `fa8fa79..ae88f7f` ne contient aucun hunk touchant
`fn main`. C'est en revanche un défaut de harnais **partagé**, et la revue
d'impact est le bon endroit pour en cadrer le périmètre.

### 7.1 Runners concernés et runners indemnes

```sh
find rust -name 'cucumber*.rs' -path '*/tests/*'
grep -n 'fail_on_skipped\|filter_run\|run_and_exit' <les 7 runners>
```

| Runner | Ligne | Forme | Verdict |
|---|---|---|---|
| `rust/crates/aithos-bundle/tests/cucumber.rs` | `19716` | `ProtocolWorld::cucumber().filter_run(...)`, sans `.fail_on_skipped()` | **AFFECTÉ** |
| `rust/crates/aithos-gateway/tests/cucumber.rs` | `10849-10850` | `.fail_on_skipped().filter_run_and_exit(...)` | indemne |
| `rust/crates/aithos-provider/tests/cucumber.rs` | `3698-3699` | `.fail_on_skipped().filter_run_and_exit(...)` | indemne |
| `rust/crates/aithos-provider/tests/cucumber_relay.rs` | `1333-1334` | `.fail_on_skipped().filter_run_and_exit(...)` | indemne |
| `rust/crates/aithos-provider/tests/cucumber_remote.rs` | `1113-1114` | `.fail_on_skipped().run_and_exit(...)` | indemne |
| `rust/crates/aithos-provider/tests/cucumber_tunnel.rs` | `314-315` | `.fail_on_skipped().run_and_exit(...)` | indemne |
| `rust/crates/aithos-provider/tests/cucumber_witness.rs` | `464-465` | `.fail_on_skipped().run_and_exit(...)` | indemne |

**Un seul runner sur sept est atteint, et c'est celui qui porte la totalité du
corpus Gherkin auditable de ce pilote.** Les six autres sont corrects sur les
deux points : sortie par code d'erreur *et* échec sur step non résolu.

Les huit cibles `harness = false` du dépôt sont recensées
(`aithos-bundle/Cargo.toml:46` et `:51`, `aithos-gateway/Cargo.toml:64`,
`aithos-provider/Cargo.toml:85,89,93,97,103`) ; `aithos-bundle/Cargo.toml:51`
est le bench `perf`, pas un runner Cucumber.

### 7.2 Features tournant sur le runner affecté

`aithos-bundle/tests/cucumber.rs:19711` pointe
`concat!(env!("CARGO_MANIFEST_DIR"), "/../../../features")`, c'est-à-dire le
répertoire racine `features/`. Les **18** fichiers y résident, tous porteurs de
leur tag canonique unique :

| Feature | Tag | Rules | Scénarios |
|---|---|---:|---:|
| `a-identity.feature` | `@a-identity` | 8 | 13 |
| `b-derivation.feature` | `@b-derivation` | 3 | 6 |
| `c-headers.feature` | `@c-headers` | 4 | 8 |
| `d-bundle.feature` | `@d-bundle` | 7 | 13 |
| `e-mandate-sections.feature` | `@e-mandate-sections` | 4 | 14 |
| `e-mandates.feature` | `@e-mandates` | 7 | 15 |
| `f-gamma.feature` | `@f-gamma` | 12 | 74 |
| `f-plus-constraints.feature` | `@f-plus-constraints` | 13 | 56 |
| `g-plus-obligations.feature` | `@g-plus-obligations` | 10 | 34 |
| `g-revocation.feature` | `@g-revocation` | 9 | 21 |
| `h-merkle.feature` | `@h-merkle` | 4 | 14 |
| `h2-gamma-roots.feature` | `@h2-gamma-roots` | 6 | 19 |
| `i-concurrency.feature` | `@i-concurrency` | 5 | 16 |
| `k-integration.feature` | `@k-integration` | 3 | 7 |
| `l-delegated-writes.feature` | `@l-delegated-writes` | 7 | 18 |
| `m-delegated-editions.feature` | `@m-delegated-editions` | 4 | 20 |
| `n-structural-mutations.feature` | `@n-structural-mutations` | 4 | 7 |
| `o-connector-classes-vault.feature` | `@o-connector-classes-vault` | 4 | 24 |

(Les comptages `Scenario` incluent les `Scenario Outline`, dont l'expansion par
`Examples:` explique l'écart avec les 836 scénarios exécutés.)

### 7.3 Conclusions, rapports et CI qui reposent sur un code de sortie

Il faut distinguer deux natures de preuve, parce que la revue elle-même le fait :
**les compteurs imprimés portent de l'information ; le code de sortie n'en porte
aucune.** Une revendication qui cite `836 scenarios (836 passed)` reste une
preuve. Une revendication qui cite `EXIT=0` n'en est pas une.

| Emplacement | Revendication exacte | Statut |
|---|---|---|
| `.github/workflows/ci.yml:23` | `cargo test --workspace --manifest-path rust/Cargo.toml` | **Non-preuve pour la cible `aithos-bundle --test cucumber`.** La CI ne lit qu'un code de sortie et personne ne relit sa sortie standard. **La CI n'a jamais pu détecter une régression Gherkin sur les 18 features.** Elle détecte en revanche les erreurs de compilation, via cette étape et via `ci.yml:21` `cargo clippy --workspace --all-targets -- -D warnings`. |
| `.github/workflows/provider-image.yml:36` | `cargo test -p aithos-provider` | Non affecté — les cinq runners Provider utilisent `*_and_exit`. |
| `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-01.md:177-183` | ```cargo test -p aithos-bundle --test cucumber``` / ```EXIT=0``` / `18 features / 114 rules / 836 scenarios (836 passed) / 3568 steps (3568 passed)` | **Revendication mixte.** Le `EXIT=0` explicitement cité est sans valeur ; les compteurs qui le suivent restent de la preuve. La ligne `EXIT=0` doit être retirée ou annotée. |
| `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-02.md:169` | `Bundle Cucumber, full \| 836 scenarios passed, 3,568 steps passed` | Preuve conservée (compteurs, pas de code de sortie). |
| `features/.agents/a-identity/corrector/runs/2026-07-29-correction-01-reconstructed.md:77` | `bundle cucumber 836 scenarios, 0 failures` | Preuve affaiblie : rapport `RECONSTRUCTED`, la source du « 0 failures » n'est pas tracée jusqu'à un bloc imprimé. |
| `features/.agents/b-derivation/corrector/runs/2026-07-29-correction-01.md` (section GREEN) | `cargo test --workspace --no-fail-fast` / `exit 0 — 98 test binaries, 632 unit and integration tests passed, 0 failed` | Le `exit 0` du workspace **ne dit rien** de la cible `cucumber` d'`aithos-bundle`. Les blocs Cucumber cités séparément (`836 scenarios (836 passed) / 3577 steps (3577 passed)` et `6 scenarios / 30 steps`) restent, eux, de la preuve. |
| `features/.agents/a-identity/DOMAIN.md:88-99` | prescrit `cargo test -p aithos-bundle --test cucumber` en gate d'intégration finale | **Ce `DOMAIN.md` ne porte aucun avertissement `BDER-011`.** Il demande bien d'« enregistrer les compteurs globaux », ce qui protège partiellement, mais il ne dit pas que le code de sortie est vide de sens. À aligner sur `b-derivation/DOMAIN.md:108-115`, qui le dit désormais explicitement. |
| `features/.agents/b-derivation/DOMAIN.md:105-115` | corrigé par cette ronde : « lire les compteurs imprimés ; le bloc imprimé est la seule preuve que ce gate produise » | À jour. |
| `features/.agents/orchestrator/runs/2026-07-29-a-identity-impact-review.md:207-209` | « les gates comportementaux n'ont pas été rejoués par le reviewer d'impact » | Non affecté : cette note ne revendique aucun code de sortie. |

Effet spécifique de l'absence de `.fail_on_skipped()` : un step non résolu n'est
ni une erreur ni un échec, il est *skipped*. Deux conséquences distinctes qu'il
faut ne pas confondre — (a) le code de sortie reste 0, ce qui est vérifié
empiriquement par la revue (trois observations, sous M5a, M5b et R1) ; (b) le
bloc imprimé mentionne-t-il les steps *skipped* dans ses compteurs ? Le writer
par défaut de `cucumber` 0.21 les distingue en principe des `passed`, ce qui
rendrait un orphelin visible à un lecteur qui vérifie `passed == total` — mais
**je n'ai pas exécuté le runner et je ne présente pas ce point comme vérifié.**
C'est précisément pour cela que le contrôle exhaustif d'orphelins du §1 a été
fait statiquement plutôt que déduit d'un run vert.

### 7.4 `@wip`

```sh
grep -rn '@wip' --include='*.feature' .
```

**Aucune occurrence dans les 18 fichiers `features/*.feature`.** La fermeture de
filtre `!scenario.tags.iter().any(|t| t == "wip")` (`cucumber.rs:19716-19718`)
n'exclut donc aujourd'hui rien du tout sur le runner affecté, ce qui confirme
l'observation de l'unité A4.

Le tag est en revanche massivement utilisé ailleurs — 36 occurrences dans
`rust/crates/aithos-gateway/tests/features/` (dont quatre features entièrement
`@wip` : `gateway-rustls-release`, `gateway-delegated-session-runtime`,
`gateway-delegated-client-surfaces`, `gateway-oauth-durable`,
`gateway-delegated-session-ceremony`) et dans
`rust/crates/aithos-provider/tests/features/` (`store-cold-roundtrip` presque
entièrement, `store-publication`, `relay-passthrough`). Ces fichiers tournent
sur des runners indemnes. **Il faut donc s'attendre à ce que la fermeture du
runner `aithos-bundle` interagisse un jour avec `@wip` si le rituel de
`docs/EXECUTION-PLAN.md` réintroduit ce tag côté `features/` — mais ce n'est
pas le cas aujourd'hui.**

### 7.5 Observation hors dépôt, clairement étiquetée

Le dépôt frère `aithos-client` (dépendance de chemin, **hors des révisions
auditées ici, à une révision non épinglée par cette revue**) porte deux runners
supplémentaires présentant le même défaut :
`crates/aithos-client/tests/cucumber.rs:2650` et
`crates/aithos-client-wasm/tests/cucumber.rs:1462`, tous deux
`…cucumber().filter_run(` sans `fail_on_skipped`. Je consigne le fait pour le
cadrage du lot de remédiation ; je ne le compte pas dans le périmètre de cette
revue d'impact et je ne l'ai pas vérifié contre une révision figée.

## 8. Classification des 17 autres features

Aucune n'est classée `NONE` par défaut. Pour chacune, les recherches
effectivement conduites sont : (a) sa présence dans la carte de consommation
`R7` face aux attributs supprimés, ajoutés et modifiés-en-corps ; (b) le
scan d'orphelins exhaustif `R5` ; (c) le scan d'ambiguïté `R6` ; (d) `R2`
(aucun symbole de production, aucun vecteur, aucune spec modifiés) ; (e) `R13`
pour la chaîne de vecteurs ; (f) `R14` pour les sections de spécification.

| # | Feature | Classe | Évidence (une ligne) |
|---:|---|---|---|
| 1 | `a-identity.feature` | `NONE` | `R7` : partage zéro attribut avec `b-derivation` ; `R14` : son audit public ne cite que §01.4 et §10.4 ; seul lien = le runner partagé, dont `fn main` est inchangé — mais voir la recommandation `BDER-011` sur `a-identity/DOMAIN.md:88-99`. |
| 2 | `c-headers.feature` | `NONE` | `R7` : zéro attribut partagé ; ses 3 mentions textuelles de « deriv » sont de la proximité lexicale, sans step commun ni symbole commun (`R2` : `derive.rs` identique). |
| 3 | `d-bundle.feature` | **`TARGETED`** | `R7` : quatre attributs passent de mono- à bi-consommateur, dont trois (`the folder {string} is renamed to {string}` `:8235`, `the edition is republished` `:8185`, `the owner reads the same section at {string}` `:12443`) lui appartenaient seul ; corps identiques (`R10`), donc aucun impact présent, mais `d-bundle:39-43` et `b-derivation:50-56` traversent désormais la même trace de production — à revoir lors de son propre audit (§3). |
| 4 | `e-mandate-sections.feature` | `NONE` | `R7` : zéro attribut partagé, y compris sur `published_with_section` ; zéro mention de dérivation dans le fichier ; `R5` : aucun orphelin. |
| 5 | `e-mandates.feature` | `NONE` | `R7` : partage `a published bundle with section {string} in circle {string}` (`:24`), qu'elle consommait **déjà** à la baseline ; corps `2e80aedf87551e63` identique dans les deux arbres — l'ajout d'un consommateur n'est pas un changement pour elle. |
| 6 | `f-gamma.feature` | `NONE` | `R7` : zéro attribut partagé ; `R13` : `gen-f.py:104-109` lit B2 en contrôle croisé pour produire `f1/f2/f3-gamma-*.json`, mais B2 est identique octet pour octet (`R2`), donc ces trois vecteurs ne peuvent pas avoir dérivé. |
| 7 | `f-plus-constraints.feature` | `NONE` | `R7` : zéro attribut partagé ; zéro mention de dérivation ; son vecteur `fplus-constraints.json` est inchangé (`diff -rq vectors` rc=0). |
| 8 | `g-plus-obligations.feature` | `NONE` | `R7` : zéro attribut partagé ; zéro mention de dérivation ; `gplus-obligations.json` inchangé. |
| 9 | `g-revocation.feature` | `NONE` | `R7` : zéro attribut partagé ; son step `agent_still_derives_old_key` (`cucumber.rs:15543-15569`, consommé en `:114`) est le seul autre step du runner à appeler `derive_key`/`node_key`/`folder_label` — corps **non touché par le diff**, et l'ajout de `folder_label` à l'import `:37` ne le masque pas puisqu'il utilise la forme pleinement qualifiée `:15560` ; `R6` : ses trois multi-résolutions sont préexistantes et identiques. |
| 10 | `h-merkle.feature` | `NONE` | `R7` : zéro attribut partagé ; `R13` : `gen-h.py:68-73` lit B2 en contrôle croisé avant d'écrire `h1-merkle.json`, B2 inchangé. |
| 11 | `h2-gamma-roots.feature` | `NONE` | `R7` : zéro attribut partagé ; `R13` : `gen-h2.py:95-100`, même raisonnement, B2 inchangé. |
| 12 | `i-concurrency.feature` | `NONE` | `R7` : zéro attribut partagé ; `R13` : `gen-i.py:76-81`, même raisonnement, B2 inchangé. |
| 13 | `k-integration.feature` | `NONE` | `R7` : zéro attribut partagé ; `k-integration.feature:67` utilise `the owner moves folder …`, step distinct de `the folder … is renamed to …` et non touché par le diff ; sa seule citation de spec est §09. |
| 14 | `l-delegated-writes.feature` | `NONE` | `R7` : partage `a published bundle with section {string} in circle {string}` sur cinq lignes (`:22`, `:29`, `:50`, `:57`, `:63`), déjà consommé à la baseline, corps identique ; ses `:84`/`:97` relèvent d'un attribut `… tagged {string} …` distinct et intact ; ses citations de spec sont §02.11 et §04.2, hors périmètre de l'audit source. |
| 15 | `m-delegated-editions.feature` | `NONE` | `R7` : zéro attribut partagé, malgré 14 mentions textuelles de « deriv » — proximité lexicale pure, aucun step ni symbole commun (`R2` : aucun fichier de production modifié). |
| 16 | `n-structural-mutations.feature` | `NONE` | `R7` : zéro attribut partagé ; `structure.rs` identique (`658f4659…`) ; ses 7 mentions de « deriv » ne se traduisent par aucune définition de step commune. |
| 17 | `o-connector-classes-vault.feature` | `NONE` | `R7` : zéro attribut partagé ; zéro mention de dérivation ; `cb2-connector-catalog.json` et `cb2-bundle-structure-vault.json` inchangés. |

**Aucun `FULL_AUDIT`.** La condition qui le déclencherait — un helper partagé,
une API, un format ou un invariant modifié — n'est réalisée par aucun élément du
diff : le seul helper partagé touché est l'import de module `cucumber.rs:37`
(additif, sans masquage), et les trois champs de `World` ajoutés ne sont lus par
personne d'autre.

## 9. Recommandation manuelle

1. **Accepter cette revue d'impact et intégrer `codex/audit-b-derivation` dans
   le `main` local**, base enregistrée `5c3a618`. Aucune autre feature ne
   requiert d'action avant intégration. Les findings non résolus — `BDER-006`
   (décision), `BDER-007`, `BDER-008`, `BDER-010`, `BDER-011`, `BDER-012` —
   survivent à l'intégration dans l'audit public, dans `STATE.md` et dans les
   marqueurs Gherkin vivants, conformément à `PROCESS.md`.
2. **Ouvrir `BDER-011` en lot transverse dédié, avant toute ronde suivante.**
   Périmètre minimal et suffisant : `rust/crates/aithos-bundle/tests/cucumber.rs:19716`
   → `.fail_on_skipped().filter_run_and_exit(features, …)`, aligné sur
   `aithos-gateway/tests/cucumber.rs:10849-10850`. Ce lot **doit être conduit par
   un rôle correcteur/exécution**, pas par un correcteur de feature, parce qu'il
   est susceptible de faire rougir des scénarios aujourd'hui « verts » dans
   plusieurs des 18 features — c'est le but. Gates recommandés **pour ce rôle**,
   pas pour moi : `cargo test -p aithos-bundle --test cucumber` non filtré, puis
   `cargo test --workspace --no-fail-fast`. Prévoir que `.fail_on_skipped()`
   puisse révéler des steps non résolus que le scan statique du §1 ne verrait pas
   (paramètres personnalisés, `Scenario Outline` dont l'expansion diffère de la
   mienne).
3. **Aligner `features/.agents/a-identity/DOMAIN.md:88-99`** sur
   `features/.agents/b-derivation/DOMAIN.md:108-115` : y inscrire que le code de
   sortie du gate `aithos-bundle --test cucumber` ne prouve rien tant que
   `BDER-011` est ouvert. C'est une modification de document que je ne fais pas
   ici, ce rôle étant en lecture seule.
4. **Annoter `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-01.md:177-183`**
   pour marquer que la ligne `EXIT=0` n'y est pas une preuve, les compteurs qui
   la suivent restant valides. Ne pas réécrire le rapport : y ajouter la note.
5. **`d-bundle` — suivi ciblé, sans redémarrage.** Consigner dans son futur
   `DOMAIN.md`/audit deux faits : (a) `rename_the_folder` (`cucumber.rs:8235`),
   `publish_edition` (`:8185`) et `reads_at_new_path` (`:12443`) sont désormais
   co-détenus avec `b-derivation` — toute évolution, en particulier du
   `format!("projets/{name}")` codé en dur, touche les deux ; (b)
   `d-bundle.feature:39-43` est maintenant un sous-ensemble strict de
   `b-derivation.feature:50-56`, dont la moitié « keys through sids » de son
   propre titre est prouvée ailleurs. **Ce n'est pas une demande de réaudit** :
   la décision de rouvrir un audit reste manuelle.
6. **Consigner la dépendance nouvelle au vecteur B2.** Ajouter à `vectors/README.md`
   (ou au futur `DOMAIN.md` de `b-derivation`) que `b2-derivation.json` a
   désormais **trois** classes de consommateurs — `tests/b2_derivation.rs`, cinq
   générateurs Python en contrôle croisé, et la couche Gherkin — et que
   `sibling_section_sid`, `sibling_section_key_hex` et `tag` n'ont aucun témoin
   générateur (`BDER-008`, `BDER-012` point 6).
7. **Ne rien faire pour les seize autres features.** Aucun gate n'est à rejouer
   de leur fait par cette correction.

## 10. Limites de cette conclusion

- **Ce rapport n'est pas une preuve comportementale.** Aucun gate feature,
  Cucumber global ou workspace n'a été rejoué, comme le skill l'impose. La preuve
  comportementale du candidat reste celle de
  `auditor/runs/2026-07-29-audit-review-01.md`, avec les limites que ce rapport
  déclare lui-même (sept mutants, container non indépendant du matériel, lecture
  non exhaustive des writers de `cucumber` 0.21).
- **Aucun Git dans les deux arbres.** Toutes les révisions citées
  (`fa8fa79`, `3d6fa51`, `1ab331a`, `ae88f7f`, `5c3a618`, `9c3c9bc`, `891c808`)
  sont reprises de `STATE.md` et des rapports de run ; je ne les ai pas résolues
  en objets Git. Ce que j'ai vérifié directement, ce sont **deux arbres de
  fichiers** et le diff précalculé entre eux, dont j'ai confirmé qu'il coïncide
  exactement avec un `diff -rq` que j'ai recalculé moi-même.
- **Le scan d'orphelins est statique.** Il modélise la résolution de cucumber-rs ;
  il ne l'exécute pas. Les 36 attributs `regex = …` sont compilés tels quels par
  Python, dont le moteur d'expressions régulières n'est pas celui de la crate
  `regex` de Rust ; aucune des alternations concernées ne touche un attribut
  `b-derivation`, mais une divergence de moteur reste théoriquement possible sur
  les features Gamma / contraintes / connecteurs. Le contrôle décisif du §1 —
  les cinq phrases supprimées, en littéral exact — ne dépend d'aucune de ces
  approximations.
- **Le lien entre steps *skipped* et compteurs imprimés n'est pas vérifié.** Je
  n'affirme pas qu'un orphelin serait visible dans le bloc imprimé ; je m'appuie
  uniquement sur la preuve statique.
- **`aithos-client` est hors périmètre.** Les deux runners défectueux qui y sont
  signalés (§7.5) le sont à titre de cadrage, à une révision non épinglée par
  cette revue.
- **La revue d'impact ne rouvre aucune décision.** `BDER-006` reste
  `DECISION_REQUIRED` chez son propriétaire humain ; le fait relevé par la revue
  — `d-bundle.feature` ne contient aucun scénario de vue par tag ni de `wrap`,
  vérifié ici par `grep` sur les 18 fichiers — est reporté sans être arbitré.
- **`BDER-012` n'est pas réévalué.** Ce rôle ne juge pas la force résiduelle des
  assertions corrigées ; il n'a examiné leur contenu que pour établir quels
  champs de vecteur et quels champs de `World` sont désormais consommés.

## Prochaine action

Acceptation humaine de cette revue d'impact, puis, par l'orchestrateur :
passage de `features/.agents/b-derivation/STATE.md` à `COMPLETE`, mise à jour de
`features/.agents/orchestrator/STATE.md`, et intégration de
`codex/audit-b-derivation` dans le `main` local depuis la base `5c3a618`. La
feature suivante ne démarre que depuis ce `main` mis à jour.

Le lot `BDER-011` doit être ouvert **avant** que la ronde suivante ne revendique
un gate vert comme preuve. Il n'appartient à aucune feature ; il appartient à
l'orchestrateur.

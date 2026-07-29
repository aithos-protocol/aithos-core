# HANDOFF — `BDER-011` : le gate Cucumber d'`aithos-bundle` ne pouvait pas échouer

## Prompt de reprise

> Revoir de façon indépendante le lot transverse `BDER-011`. Lire intégralement
> `docs/HANDOFF-BDER-011-CUCUMBER-GATE-2026-07-29.md` et toutes ses références
> obligatoires avant tout jugement. État disque et Git = vérité ; le présent
> document est une **revendication à vérifier**, pas une preuve. Travailler
> depuis la branche `codex/fix-bder-011-cucumber-gate`, à partir de la révision
> qu'elle enregistre. Rejouer les gates RED et GREEN ci-dessous sur la révision
> candidate avant de lire le diff. Ne pas élargir le périmètre : ce lot ne
> corrige aucun scénario, aucune assertion, aucun fichier de production. Si un
> scénario devient rouge, s'arrêter et documenter — c'est le but du lot, pas un
> échec du correctif.

## Le défaut

`rust/crates/aithos-bundle/tests/cucumber.rs` déclarait son runner ainsi :

```rust
futures::executor::block_on(
    ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| {
        !scenario.tags.iter().any(|t| t == "wip")
    }),
);
```

`Cucumber::filter_run` **retourne son écrivain et ne sort jamais** ; seules les
variantes `*_and_exit` propagent l'échec. Avec `harness = false`
(`aithos-bundle/Cargo.toml`, `[[test]] name = "cucumber"`), `main` retourne
`()` et le processus sort en 0 quels qu'aient été les résultats des steps.
`.fail_on_skipped()` était également absent : une phrase de step non résolue
n'était pas une erreur non plus, seulement un *skip* silencieux.

Ce n'était pas une hypothèse. Pendant la revue de la ronde 1 `b-derivation`, le
fait a été observé trois fois : avec quatre scénarios en échec, avec trois, puis
avec un seul, `cargo test -p aithos-bundle --test cucumber` est sorti en **0** à
chaque fois, tout en affichant les `✘` à l'écran.

Portée : **les 18 features** tournent sur ce runner. Le code de sortie de leur
gate canonique, du gate Cucumber global d'un correcteur, du gate workspace sur
cette cible, et de la CI (`cargo test --workspace`) ne portait aucune
information. Les deux runners frères du même dépôt faisaient déjà correctement
les choses — `aithos-gateway/tests/cucumber.rs:10848-10850` et
`aithos-provider/tests/cucumber.rs:3698-3699` utilisent tous deux
`.fail_on_skipped().filter_run_and_exit(...)`.

Le défaut est **préexistant** : `fn main` est identique octet pour octet entre
`fa8fa79` et `ae88f7f`. Il n'a été introduit par aucune ronde de correction.

## Références obligatoires

Lire intégralement, dans cet ordre :

1. `features/AGENTS.md` ;
2. `features/.agents/PROCESS.md`, en particulier « Feature targeting and gate
   pyramid » ;
3. `features/.agents/b-derivation/auditor/runs/2026-07-29-audit-review-01.md`,
   section `BDER-011` — l'origine du constat et les trois observations ;
4. `features/.agents/orchestrator/runs/2026-07-29-b-derivation-impact-review.md`,
   §7 « rayon d'impact » — l'inventaire des conclusions antérieures qui
   s'appuient sur ce code de sortie ;
5. `docs/audits/features/b-derivation.md`, écart `BDER-011` ;
6. `rust/crates/aithos-gateway/tests/cucumber.rs:10845-10860` et
   `rust/crates/aithos-provider/tests/cucumber.rs:3694-3705`, les deux
   références internes du bon idiome.

## Le correctif

Une seule construction, alignée sur les deux runners frères :

```rust
futures::executor::block_on(
    ProtocolWorld::cucumber()
        .fail_on_skipped()
        .filter_run_and_exit(features, |_, _, scenario| {
            !scenario.tags.iter().any(|t| t == "wip")
        }),
);
```

Aucun autre fichier n'est modifié. Aucun scénario, aucune assertion, aucun
fichier de production, aucun vecteur. Le commentaire « Ritual » d'origine est
conservé ; un commentaire `BDER-011` est ajouté pour que la construction ne soit
pas « simplifiée » par un futur passage.

`max_concurrent_scenarios(Some(1))`, présent chez le runner gateway,
**n'a pas été repris** : la sérialisation y répond à un besoin de ce runner
(état de service partagé) et l'imposer ici ralentirait 836 scénarios sans motif
constaté. À trancher séparément si un jour un flakiness de concurrence
apparaît sur cette suite.

## Mesure de cadrage effectuée avant la bascule

C'était le seul risque réel du lot : personne ne savait combien de scénarios
échouaient ou étaient silencieusement *skipped* dans les 18 features, puisque le
code de sortie mentait. Mesure faite **avant** toute modification, sur la
révision `ae88f7f`, en lisant les compteurs imprimés :

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber

18 features
114 rules
836 scenarios (836 passed)
3577 steps (3577 passed)
```

Zéro échec, zéro *skip*. La bascule est donc, sur cette révision, sans effet
comportemental : elle ne fait que rendre exigible ce qui était déjà vrai. C'est
ce qui permet de la livrer sans lot de réparation associé.

## Gates exécutés — à rejouer par le rôle de revue

Tous exécutés sur un conteneur Linux `x86_64`, `rustc 1.95.0`, sur un export
`git archive` de `ae88f7f` plus la dépendance `aithos-client` (`c6f6151`), le
poste de travail n'exposant pas de toolchain Rust à ce rôle. L'arbre testé est
byte-identique au contenu suivi de `ae88f7f`.

### GREEN

| Gate | Résultat | Code de sortie |
|---|---|---:|
| `--test cucumber` non filtré | 18 features, 114 rules, 836 scenarios (836 passed), 3577 steps (3577 passed) | **0** |
| `--test cucumber -- --tags @b-derivation` | 1 feature, 3 rules, 6 scenarios (6 passed), 30 steps (30 passed) | **0** |
| `--test cucumber -- --tags @a-identity` | 1 feature, 8 rules, 30 scenarios (30 passed), 93 steps (93 passed) | **0** |

Les deux gates filtrés vérifient le point non trivial : `.fail_on_skipped()`
n'interagit pas avec le filtrage par tag. Un scénario écarté par `--tags` est
*filtré*, pas *skipped*, et ne fait donc pas échouer le gate. Une feature
voisine a été prise exprès pour ne pas conclure depuis la seule feature d'où
vient le constat.

### RED — le correctif ne vaut que si l'échec devient exigible

| Sonde | Avant le correctif | Après le correctif |
|---|---:|---:|
| Mutant M5a — étape `parent XOR blake3(label)` dans `node_key` : 4 scénarios en échec | exit **0** | exit **101** |
| Phrase de step rendue non résolue (`yield the same key` → `yield the very same key`) : 1 step non apparié | *skip* silencieux, exit **0** | `✘`, 6 scenarios (5 passed, 1 failed), exit **101** |

La seconde sonde est la preuve propre de `.fail_on_skipped()` : sans elle, une
phrase orpheline resterait invisible. C'est exactement le risque que la revue
d'impact a dû écarter à la main, par un scan littéral exhaustif des phrases
supprimées à travers les 18 fichiers `.feature`, parce que le runner ne pouvait
pas le signaler.

Les deux mutations ont été appliquées à une copie jetable, jamais au dépôt, et
les fichiers ont été restaurés byte-identiques après chaque sonde.

### Non exécuté, et déclaré comme tel

`cargo test --workspace --no-fail-fast` et `cargo fmt --all -- --check` n'ont pas
été rejoués dans ce lot. Le rôle de revue doit les exiger avant intégration dans
`main`. Rappel de la revue de la ronde 1 : `cargo fmt --check` échouait **déjà**
sur la baseline, sur `rust/crates/aithos-gateway/src/core_bridge.rs:1355`, sans
rapport avec ce lot ; ne pas absorber ce correctif ici.

## Ce que ce lot ne fait pas

- Il ne réévalue aucune conclusion d'audit antérieure. Les rapports qui citent
  un `EXIT=0` comme preuve restent à annoter — la revue d'impact les recense
  (§7), en particulier
  `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-01.md:177-183`.
  Les compteurs imprimés qui suivent ces lignes, eux, restent valides.
- Il n'aligne pas `features/.agents/a-identity/DOMAIN.md:88-99` sur
  `features/.agents/b-derivation/DOMAIN.md:108-115`. Recommandation 3 de la revue
  d'impact, à traiter dans le domaine `a-identity`, pas ici.
- Il ne touche pas `max_concurrent_scenarios`, ni le filtre `@wip` (aucun fichier
  `.feature` ne porte ce tag aujourd'hui, le filtre n'exclut donc rien).
- Il ne marque `BDER-011` qu'`IMPLEMENTED`. Un rôle d'exécution ne se
  vérifie pas lui-même.

## Un piège documenté, pour la suite

`cucumber` 0.21 fait que l'option CLI `--tags` **remplace** le filtre passé à
`filter_run*`, elle ne s'y compose pas. Conséquence : un scénario tagué `@wip`
serait exécuté par un gate lancé avec `--tags`, alors que le gate par défaut
l'écarte. Aucun `.feature` ne porte `@wip` aujourd'hui, donc aucune contamination
actuelle — mais le jour où le rituel `@wip` reprend, le gate documenté et le gate
par défaut ne sélectionneront pas le même ensemble. À traiter avant, pas après.

## Critères de fin, pour le rôle de revue

- [ ] Les trois gates GREEN sont rejoués et sortent en 0, compteurs lus, pas
      seulement le code de sortie.
- [ ] Les deux sondes RED sont rejouées et sortent en non-zéro.
- [ ] `cargo test --workspace --no-fail-fast` est vert, ou ses échecs sont
      documentés comme préexistants.
- [ ] `cargo fmt --all -- --check` : l'échec `core_bridge.rs:1355` est reconnu
      comme préexistant et non absorbé ici.
- [ ] Le diff se limite à `fn main` de `rust/crates/aithos-bundle/tests/cucumber.rs`,
      à l'entrée `BDER-011` de l'audit public et à ce document.
- [ ] `BDER-011` passe à `VERIFIED` **par la revue**, jamais par ce lot.

# Chantier — orchestrateur automatique du process d'audit Gherkin

**Date** : 2026-08-03
**Périmètre** : `code/aithos-core/features/` — automatiser de bout en bout le
process décrit par `features/.agents/PROCESS.md` sur les 19 fichiers de feature.
**Statut** : conception, à valider. Aucune ligne de code produite.

**Choix cadres déjà arrêtés** (session du 2026-08-03) :

1. runtime = session Cowork cloud (conteneur), pas Codex sur le Mac ;
2. autonomie = le fil avance seul jusqu'au blocage, pas d'accord humain entre
   deux agents ;
3. déclenchement = manuel (« go »), puis zéro intervention jusqu'au rapport ;
4. livrable de la session = ce document.

---

## 1. Constats de départ (vérifiés dans l'arbre, 2026-08-03)

| Fait | Valeur observée | Conséquence de conception |
|---|---|---|
| Fichiers de feature | 19 | 17 restent à traiter |
| Volume | ~383 déclarations `Scenario`/`Scenario Outline` (Examples non dépliés), 114 `Rule` | Le grain naturel d'unité de revue est le `Rule` → ~114 unités Pass A |
| Domaines outillés | **2 seulement** : `a-identity` (COMPLETE), `b-derivation` (round 2 ouvert) | 17 domaines à amorcer : `DOMAIN.md`, `STATE.md`, 2 skills spécialisés |
| `.agents/c-headers/` | répertoires vides sur `main`, mais le round de juillet **existe** sur `codex/audit-c-headers` (`af32734`) : 2158 lignes, 15 fichiers, audit `CHDR-001`…`CHDR-016`, 4 unités Pass A gelées | *Résolu le 2026-08-03.* **Décision : refaire le round** depuis le `main` courant — la branche part de `240c658`, antérieur au correctif `BDER-011`, donc ses preuves de gate viennent d'un harnais qui ne pouvait pas échouer. `af32734` sert d'étalon, jamais d'entrée Pass A. |
| **`origin/main` vs `main` local** | `db01690` contre `72b96ec` — **13 commits en avance, 0 en retard** : clôture du round 1, lot balises, round 2 complet | *Découvert le 2026-08-03.* Précondition bloquante : le fil clone `origin`, donc tout ce qui reste local lui est invisible (§8). |
| Fiabilité du pont device | `device_list_dir` / `device_stage_files` ont servi un instantané **périmé de ~3 h** | Tout fait critique lu via le pont se revérifie par une lecture fraîche. Sans effet sur le fil, qui lit depuis `origin`. |
| `b-derivation` round 2 | **terminé** sur `codex/fix-b-derivation-bder-006-008-decisions` (`804e7bb`, fusionné dans `main` local) : `BDER-006`/`BDER-008` `VERIFIED`, `BDER-013` ouvert, revue d'impact faite | *Corrigé le 2026-08-03.* Ne peut plus servir de rodage au fil (§10). |
| Audits publics | `a-identity.md`, `b-derivation.md` seulement | 17 notes d'audit à créer |
| Harnais Cucumber | `filter_run_and_exit` + `fail_on_skipped` (BDER-011 **fermé**) | **Le code de sortie est de nouveau une preuve.** L'autonomie repose là-dessus. |
| `rust/Cargo.toml:111` | entrée morte `aithos-client` présente, mais **`cargo metadata` charge le workspace sans le checkout frère** (vérifié le 2026-08-03, 5 paquets) | *Résolu.* Le fil n'a pas besoin du frère. La purge reste un point d'hygiène SPL-9. |
| Gate de référence | `@a-identity` @ `db01690` : `exit=0`, 30 scénarios / 93 steps, **2 min 38 à froid** avec `CARGO_TARGET_DIR` partagé | Chaîne de preuve validée. **13 déclarations `Scenario` → 30 exécutions** : facteur `Outline` ×2,3 à reporter sur tous les dimensionnements |
| Worktrees `wt-b`, `wt-c` | enregistrés et **verrouillés**, pointent vers `/tmp/wt-b` et `/tmp/wt-c` (`9594e42`) | À purger avant tout run (`unlock` puis `remove --force`) |
| Marqueurs d'audit vivants | `a-identity` : 1, `b-derivation` : 2 | Le cycle de vie des marqueurs est déjà tenu, il faudra le tenir automatiquement |

Deux tailles extrêmes commandent le dimensionnement : `g4-client-surfaces`
(4 scénarios, 0 `Rule`, `@wip`) et `f-gamma` (74 scénarios, 12 `Rule`,
736 lignes). Un orchestrateur qui ne sait traiter que le premier ne sert à rien.

---

## 2. Ce que le PROCESS impose et qui résiste à l'automatisation

Six verrous. Chacun doit recevoir une réponse **structurelle** — c'est-à-dire
rendue vraie par l'architecture — et non une réponse de discipline (« l'agent
recevra la consigne de ne pas… »). Un agent autonome finira toujours par violer
une consigne de discipline ; il ne peut pas violer une contrainte matérielle.

| # | Verrou du PROCESS | Réponse de discipline (insuffisante) | Réponse structurelle retenue |
|---|---|---|---|
| V1 | Barrière Pass A / Pass B : cécité historique avant gel | « n'appelle pas `git log` » | L'agent Pass A travaille dans un **extrait `git archive` sans `.git`** : il ne *peut pas* lire l'histoire |
| V2 | Contexte neuf par unité de revue (anti-ancrage) | « oublie l'unité précédente » | **Un `agent()` = un contexte** ; les unités ne se voient jamais |
| V3 | Pyramide de gates (ne pas rejouer les gates globaux) | « ne relance pas le workspace » | Les agents **n'ont pas le droit de lancer un gate** ; l'orchestrateur les lance et distribue le transcript |
| V4 | Preuve non falsifiable (« gate vert » écrit sans l'avoir lancé) | relecture humaine | Tout rapport cite un `evidence_id` du **journal de l'orchestrateur** ; une commande absente du journal invalide le rapport |
| V5 | Frontière de rôle (un correcteur ne pose jamais `VERIFIED`) | consigne dans le skill | **Sortie JSON schématisée** : le schéma du correcteur n'a pas de valeur `VERIFIED` |
| V6 | Indépendance du reviewer vis-à-vis du correcteur | « ne lis pas son rapport » | Le reviewer reçoit **l'extrait du candidat sans `.git` et sans le rapport de correction** pour son Pass A ; le rapport ne lui est délivré qu'après gel |

Ces six réponses sont le vrai contenu du chantier. Le reste est de la plomberie.

---

## 3. Principe directeur

> **L'orchestrateur n'a pas de mémoire. Un run est une fonction de l'état du
> dépôt.**

Aucune décision, aucun verdict, aucun jalon ne vit dans le contexte d'une
session. Tout est écrit sur disque, dans le dépôt, et committé. Conséquences :

- une session Cowork qui meurt (fin de conteneur, déconnexion, budget) ne coûte
  qu'un cycle en cours, jamais le fil ;
- n'importe quelle session ultérieure reprend en lisant l'état, sans qu'on ait
  à lui raconter l'historique ;
- le même mécanisme sert à la reprise manuelle et à une éventuelle tâche
  planifiée (phase 5) ;
- le fil est **rejouable** et **auditable après coup** — ce qui est la moindre
  des choses pour un outil dont le métier est l'audit.

---

## 4. Architecture en trois couches

### Couche 1 — l'état (dans le dépôt, versionné)

C'est la vérité. Quatre artefacts, dont trois nouveaux.

**a. `features/.agents/<feature>/STATE.md` — enrichi d'un frontmatter YAML.**
Le tableau markdown existant reste, pour les humains. Le frontmatter est ce que
l'orchestrateur lit. Il ne contient **que les champs de routage** que le
PROCESS autorise déjà un agent Pass A à consulter.

```yaml
---
feature: b-derivation
status: CORRECTION_REQUESTED      # état de la machine (§5)
mode: correction
round: 2
base_main: 1ab331a…               # base main du cycle
audit_revision: null              # révision immuable auditée
candidate_revision: null
branch: codex/fix-b-derivation-bder-006-008-decisions
assigned_findings: [BDER-006, BDER-008]
open_findings:    [BDER-007, BDER-010, BDER-012]
rejection_count:  {BDER-006: 0, BDER-008: 0}
blocked: null                     # ou {reason, question, since}
last_transition: 2026-08-02T18:00:00+02:00
---
```

**b. `features/.agents/orchestrator/QUEUE.yaml` — l'ordre et la politique.**
Ordre des features, budgets par cycle, politique d'autonomie, drapeaux de
divulgation. C'est le seul fichier que Mathieu édite pour piloter le fil.

**c. `features/.agents/orchestrator/runs/<date>-<run-id>/ledger.jsonl` — le
journal append-only.** Une ligne par événement : lancement d'agent, commande
exécutée, transcript (hash + chemin), transition d'état, blocage. C'est la
source de toutes les preuves citables.

```json
{"ts":"2026-08-03T09:12:44Z","kind":"gate","evidence_id":"ev-0f3a",
 "feature":"c-headers","cmd":"cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers",
 "rev":"db01690","exit":0,"summary":{"scenarios":8,"passed":8,"failed":0,"skipped":0},
 "transcript":"runs/2026-08-03-r1/evidence/ev-0f3a.txt","sha256":"…"}
```

**d. `features/.agents/orchestrator/BLOCKED.md` — la boîte aux lettres humaine.**
Une entrée par arrêt : la question exacte, les options, les preuves, le coût de
chaque branche. C'est le seul endroit que Mathieu doit lire au réveil.

### Couche 2 — l'exécution (conteneur, éphémère)

- un **script Workflow** (JS déterministe) : boucle sur la queue, pipeline des
  étapes, un `agent()` par rôle, aucune décision confiée au modèle sur le
  *séquencement* ;
- un **bootstrap conteneur** scripté : clone, toolchain, `CARGO_TARGET_DIR`
  partagé, vérification préalable du workspace ;
- les **gates**, lancés uniquement par ce script.

Le choix du Workflow (plutôt qu'un agent orchestrateur conversationnel) est
délibéré : le séquencement, les compteurs de rejet, les budgets et les
conditions d'arrêt sont du **code**, pas du jugement. Un orchestrateur-modèle
qui décide lui-même s'il a le droit de passer à la feature suivante finira par
se donner cette permission.

### Couche 3 — la preuve (isolation)

Trois espaces de travail par cycle, jamais mélangés :

```
work/
  repo/                     clone complet (.git) — Pass B, correcteur, impact
  passA/<feature>/<unit>/   extrait git archive de la révision immuable, SANS .git
  passA/<feature>/candidate/ extrait du candidat, SANS .git ET sans le rapport correcteur
  evidence/                 transcripts de gates (immuables, hashés)
```

---

## 5. Machine à états

Le cycle du PROCESS, rendu explicite et gardé. Une transition ne s'opère que si
son invariant est vérifié **par du code**, pas par l'affirmation d'un agent.

```
UNBOOTSTRAPPED
   │ (bootstrapper : DOMAIN.md + STATE.md + skills spécialisés)
   ▼
READY ──► AUDIT_INITIAL ──► CORRECTION_REQUESTED ──► REVIEW_REQUESTED
                                   ▲                       │
                                   │ rejet (<3)            ├─► DECISION_REQUIRED ─► BLOCKED
                                   └───────────────────────┤
                                                           ├─► rejet n°3 ────────► BLOCKED
                                                           ▼
                                                    REVIEW_ACCEPTED
                                                           │
                                                           ▼
                                              IMPACT_REVIEW_REQUESTED
                                                           │
                                                           ▼
                                                     INTEGRATION
                                                           │
                                                           ▼
                                                       COMPLETE ──► feature suivante
```

| Transition | Invariant vérifié par le code avant de basculer |
|---|---|
| `→ AUDIT_INITIAL` | `verify-feature-tags.sh` vert · branche `codex/audit-<f>` créée depuis le `main` local courant · révision gelée · gate de feature vert au journal |
| `→ CORRECTION_REQUESTED` | audit public écrit · rapport de run complet (tous les champs §« Required run conclusion ») · verdicts Pass A gelés **antérieurs** aux entrées Pass B dans le journal · marqueurs Gherkin cohérents avec les findings non résolus |
| `→ REVIEW_REQUESTED` | diff limité au périmètre assigné · au moins un RED prouvé au journal avant le GREEN · gate de feature vert **après** le dernier changement · gate Cucumber global vert · aucun `VERIFIED` dans la sortie du correcteur |
| `→ REVIEW_ACCEPTED` | Pass A du reviewer gelé avant toute lecture du diff · chaque finding accepté ou rejeté séparément · marqueurs des `VERIFIED` retirés du `.feature` |
| `→ INTEGRATION` | rapport d'impact écrit · aucun `FULL_AUDIT` non traité, ou consigné en suivi · gardien de process vert |
| `→ COMPLETE` | merge dans la branche de run · findings ouverts toujours visibles dans l'audit public et les marqueurs · `STATE.md` mis à jour et committé |

Toute transition impossible n'est pas une erreur : c'est un **blocage** (§9),
avec sa question posée en clair.

---

## 6. Topologie des agents

Ta demande — « un agent par fichier de feature et rôle, plus un agent global de
vérification » — se décline en **neuf rôles**. Sept par feature, deux globaux.

### Par feature

| Rôle | Nb | Espace de travail | Entrées | Sortie (JSON schématisé) |
|---|---|---|---|---|
| **B0 — amorceur de domaine** | 1 (si absent) | `repo/` | `PROCESS.md`, les 2 domaines modèles, le `.feature` | `DOMAIN.md`, `STATE.md`, les 2 skills spécialisés. **Ne lit ni ne touche le code de production.** |
| **I1 — inventaire & découpe** | 1 | extrait sans `.git` | le `.feature` seul | liste d'unités de revue (1 `Rule`, ou grappe de risque de 3–6 scénarios), avec justification du découpage |
| **A2 — Pass A unitaire** | N (parallèle) | `passA/<unit>/` | son unité · steps · code de prod · surfaces · spec · vecteurs · `DOMAIN.md` · champs de routage du `STATE.md` · transcript du gate | verdict provisoire par scénario + trace de preuve, **gelé** |
| **A3 — Pass B & intégration** | 1 | `repo/` | verdicts A2 gelés · `git log -p` · audits antérieurs · rapports antérieurs | réconciliation, passe d'état partagé (`OnceLock`, caches, hooks, steps communs), audit public, marqueurs, rapport de run |
| **A4 — réfuteur adverse** | 2–3 par finding P1/P2 (parallèle) | extrait sans `.git` | l'énoncé du finding seul | `refuted` / `confirmed` + preuve. Consigne : **réfuter par défaut si incertain** |
| **C1 — correcteur** | 1 | `repo/` | uniquement les findings assignés + leurs décisions | diff, RED/GREEN, statut au plus `IMPLEMENTED` |
| **R1 — reviewer indépendant** | 1 | `passA/candidate/` puis `repo/` | candidat sans `.git`, **sans** le rapport du correcteur, jusqu'au gel | acceptation/rejet par finding, `VERIFIED` possible |

Le rôle A4 n'est pas dans le PROCESS actuel. Il vient de la leçon du round
c-headers : la passe de réfutation adverse avait confirmé les quatre
affirmations P1/P2, corrigé trois formulations exagérées et **révélé un finding
que l'audit avait manqué**. En autonomie, elle devient indispensable : c'est le
seul contre-pouvoir avant que le correcteur ne touche du code.

### Globaux

| Rôle | Quand | Ce qu'il juge | Ce qu'il ne juge pas |
|---|---|---|---|
| **G1 — reviewer d'impact** | après chaque `REVIEW_ACCEPTED` | dépendances inter-features du diff accepté (`NONE` / `TARGETED` / `FULL_AUDIT`) | la justesse de la correction |
| **G2 — gardien de process** | à chaque transition majeure et en fin de cycle | **la conformité, pas la cryptographie** : barrière Pass A/B respectée dans l'ordre du journal · aucune preuve citée hors journal · pyramide de gates non violée · frontières de rôle tenues · cycle de vie des marqueurs · complétude des rapports | le contenu technique des findings |

G2 est ton « agent global de vérification ». Il a un pouvoir réel : **il peut
invalider un cycle**. Deux invalidations consécutives sur la même feature →
blocage humain. Sans lui, l'autonomie produit des cycles bien formés et vides.

### Invariant d'écriture

À tout instant, **un seul rôle écrit** : B0, A3, C1, R1 ou G1, jamais deux à la
fois. I1, A2 et A4 sont en lecture seule et peuvent donc se paralléliser sans
risque. Les features sont traitées **strictement en série** — ce qui est aussi
ce qu'exige le PROCESS (« intégrer dans `main` local avant la feature
suivante »), et ce qui supprime toute question de conflit de merge.

---

## 7. Exécution déterministe des preuves

**Aucun agent ne lance de commande de gate.** L'orchestrateur les lance, écrit
le transcript dans `evidence/`, le hashe, journalise l'`evidence_id`, et
**distribue le texte** à l'agent qui en a besoin.

Bénéfices, dans l'ordre d'importance :

1. un rapport ne peut plus affirmer « gate vert » sans qu'un transcript
   correspondant existe (V4) ;
2. la pyramide de gates est respectée par construction : l'auditeur ne *peut
   pas* relancer le workspace (V3) ;
3. un seul `CARGO_TARGET_DIR` partagé — la compilation n'est payée qu'une fois
   par révision au lieu d'une fois par agent ;
4. la sortie est parsée en JSON (`exit`, compteurs `[Summary]`) : le fil décide
   sur des nombres, pas sur de la prose.

> **Amendement à soumettre** — `PROCESS.md` §« Feature targeting and gate
> pyramid » dit que « l'auditeur lance le gate de feature ». À reformuler en :
> le rôle **possède** le gate (il en assume la responsabilité et le cite dans
> son rapport) ; l'orchestrateur **l'exécute** pour son compte et lui en fournit
> le transcript immuable. L'intention — chaque rôle répond de ses preuves — est
> préservée ; la falsifiabilité est ajoutée.
>
> On garde `filter_run_and_exit` comme preuve primaire (BDER-011 est fermé),
> mais le fil enregistre **aussi** les compteurs `[Summary]` et refuse un gate
> dont le code de sortie et les compteurs se contredisent. C'est une sonde
> permanente contre la régression du harnais.

---

## 8. Conteneur, git, durabilité

**Précondition absolue — rien de local ne doit rester non poussé.** L'état de
départ du fil est `origin/main`, pas ton disque. Un run lancé sur un `origin`
en retard refait à l'aveugle un travail déjà fait, sur une base fausse. Le
contrôle est inscrit dans les interdits d'AM-4 : le fil refuse de tourner sur
une base `main` qu'il n'a pas lui-même vérifiée. Au 2026-08-03 cette
précondition n'est **pas** remplie (voir `RECONNAISSANCE-ORCHESTRATEUR-2026-08-03.md` §1).

**Bootstrap** (à chaque nouveau conteneur, scripté, ~5 min) :

1. `git clone` de `aithos-protocol/aithos-core` depuis GitHub — jeton lu dans
   `/Volumes/Math17/aithos/v2/.github-env`, section `#aithos` ;
2. checkout de la branche de run si elle existe, sinon `main` ;
3. **contrôle préalable** : `cargo metadata` doit charger le workspace sans
   checkout frère `aithos-client` ; si l'entrée morte de `rust/Cargo.toml:111`
   le fait échouer, on ajoute le frère au pin `c6f6151` et on ouvre un
   sous-chantier SPL-9 ;
4. `verify-feature-tags.sh` ;
5. gate de feature « à blanc » sur une feature déjà `COMPLETE` (`a-identity`)
   pour préchauffer le cache et prouver que la chaîne de preuve fonctionne
   **avant** de dépenser un seul agent.

**Git.** Le Mac ne fait rien pendant un run — règle acquise le 2026-08-01 :
plus aucune commande `git` sur `code/aithos-core` via le pont device, chaque
appel y laisse un verrou que seul toi peux effacer. Le fil travaille dans son
clone.

- branche de run : `agents/train-<date>` — c'est **le `main` local mouvant** au
  sens du PROCESS ; elle avance d'un cycle accepté à la fois ;
- branches par feature : `codex/audit-<f>` et `codex/fix-<f>-<scope>`, comme
  aujourd'hui ;
- ~~**le fil ne pousse jamais sur `main`**~~ — **révisé le 2026-08-04 par le
  propriétaire** : le fil intègre un cycle accepté dans `main` et le pousse.
  Motif identique à `policy.backward_compatibility_required` : rien n'est
  déployé, aucune édition n'a été publiée, donc une mauvaise intégration coûte
  un `revert` et rien d'autre. C'était le seul endroit où un humain regardait
  avant que le travail n'atterrisse sur la branche principale, et cette relecture
  est abandonnée en connaissance de cause. Expire à la même condition —
  première édition publiée hors du dépôt, ou sortie de l'alpha. Il pousse aussi
  la branche de run et les
  branches de feature. La promotion vers `main` reste ton geste ;
- push après chaque cycle `COMPLETE` → l'état survit à la mort du conteneur ;
- au réveil : tu lis `BLOCKED.md`, tu regardes la branche de run, tu promeus ou
  tu renvoies au fil.

**Reprise à froid.** Nouveau conteneur, `git pull` de la branche de run, lecture
des `STATE.md` : le fil sait exactement où il en est. Aucune reconstruction
narrative, aucun « prompt de reprise » à rédiger.

---

## 9. Conditions d'arrêt

Le fil s'arrête, écrit dans `BLOCKED.md`, et **ne devine pas**. Liste
exhaustive — tout ce qui n'y est pas ne justifie pas un arrêt :

| # | Déclencheur | Question posée |
|---|---|---|
| 1 | `DECISION_REQUIRED` | les sémantiques concurrentes, leurs preuves, le coût de chaque option |
| 2 | 3ᵉ rejet du même finding | ce que le correcteur n'arrive pas à faire et pourquoi |
| 3 | Gate rouge non attribuable au périmètre en cours | transcript + révision + hypothèse |
| 4 | Contamination Pass A déclarée ou détectée par G2 | quelle unité, quelle fuite, refaire ou accepter |
| 5 | Réfuteurs majoritairement contre l'auditeur | le finding et les deux lectures |
| 6 | G2 invalide deux fois la même feature | quel invariant de process est violé |
| 7 | Budget épuisé (temps, tokens, disque) | où en est le cycle, coût de la suite |
| 8 | Diff hors périmètre | fichiers touchés hors assignation |
| 9 | Finding de sécurité exploitable sur dépôt **public** | publier, décrire en creux, ou différer (§11) |
| 10 | `FULL_AUDIT` prononcé par G1 | rouvrir une feature déjà `COMPLETE` — le PROCESS le réserve explicitement à l'humain |

Le point 10 mérite d'être souligné : `PROCESS.md` dit « the decision to restart
an audit remains manual ». Le fil ne rouvrira jamais une feature close de son
propre chef, même en mode autonome. C'est le seul endroit où j'ai choisi de ne
pas te proposer d'assouplissement.

---

## 10. Ordre de passage et budgets

Ordre proposé pour `QUEUE.yaml` (tu l'édites, c'est ton levier) :

| # | Feature | Scén. | `Rule` | Raison de la place |
|---|---|---|---|---|
> **Révisé le 2026-08-03.** `b-derivation` round 2 est terminé (correction,
> revue, impact) et ne peut plus servir de rodage. Le premier cycle du fil
> devient `c-headers`, dont le round manuel de juillet fournit justement le
> point de comparaison du jalon de vérité (phase 3).

| 1 | `c-headers` | 8 | 4 | premier cycle : le round manuel existe sur `codex/audit-c-headers`, il sert d'étalon |
| 2 | `g4-client-surfaces` | 4 | 0 | la plus petite ; valide le cas « 0 `Rule` » et le tag `@wip` ; renommage et pré-gate déjà faits |
| 4 | `d-bundle` | 13 | 7 | `TARGETED` déjà dû par la décision BDER-006 (scénarios tag-view/`wrap`) |
| 5 | `h-merkle` | 14 | 4 | socle, peu couplé |
| 6 | `e-mandates` | 15 | 7 | |
| 7 | `e-mandate-sections` | 14 | 4 | juste après, steps communs encore chauds côté A3 |
| 8 | `n-structural-mutations` | 7 | 4 | |
| 9 | `i-concurrency` | 16 | 5 | état partagé — bon test de la passe d'intégration |
| 10 | `h2-gamma-roots` | 19 | 6 | |
| 11 | `g-revocation` | 21 | 9 | |
| 12 | `l-delegated-writes` | 18 | 7 | |
| 13 | `m-delegated-editions` | 20 | 4 | |
| 14 | `o-connector-classes-vault` | 24 | 4 | |
| 15 | `g-plus-obligations` | 34 | 10 | |
| 16 | `f-plus-constraints` | 56 | 13 | gros morceau, pipeline rodé |
| 17 | `f-gamma` | 74 | 12 | le plus gros |
| 18 | `k-integration` | 7 | 3 | **en dernier** : il intègre les autres, il doit les auditer stabilisés |

**Ordre de grandeur, à énoncer sans détour.** Un cycle complet sur une feature
moyenne (5 `Rule`) mobilise ~15 agents : 1 inventaire, 5 Pass A, 1 Pass B,
~6 réfuteurs, 1 correcteur, 1 reviewer, plus G1 et G2. `f-gamma` en demandera
plutôt 30. Sur les 17 features : **250 à 350 agents**. Ce n'est pas une nuit,
c'est de l'ordre d'un cycle par run, soit ~17 runs. Le fil est conçu pour cette
réalité : il est *repris*, pas *veillé*. Le budget par cycle est déclaré dans
`QUEUE.yaml` et son épuisement est un arrêt propre (blocage n°7), pas un crash.

---

## 11. Amendements à soumettre avant toute implémentation

Cinq modifications de `PROCESS.md` / `AGENTS.md`. Toutes sont des ajouts ;
aucune ne relâche une exigence de preuve.

| # | Où | Contenu | Justification |
|---|---|---|---|
| **AM-1** | §gate pyramid | l'orchestrateur **exécute** les gates, le rôle les **possède** et les cite par `evidence_id` | rend la preuve falsifiable (V4) |
| **AM-2** | §review-unit isolation | l'isolation Pass A est **matérielle** (extrait sans `.git`), pas déclarative | ferme V1 |
| **AM-3** | nouveau §« Adversarial refutation » | passe A4 obligatoire sur tout finding P1/P2 avant handoff au correcteur | leçon c-headers ; contre-pouvoir en autonomie |
| **AM-4** | nouveau §« Orchestrated mode » | machine à états, `STATE.md` frontmatter, ledger, liste close des blocages, rôle G2 | rend le mode autonome explicite plutôt que subi |
| **AM-5** | §impact review | « Do not launch another agent » vaut pour **le rôle**, pas pour l'orchestrateur | lève une contradiction littérale du texte actuel |

**Point à trancher, non tranché ici** : `aithos-core` est **public**. Un audit
qui documente un faux positif sémantique sur un chemin de vérification
cryptographique décrit, de fait, une faiblesse exploitable. Le process actuel
publie tout, immédiatement, sans distinguer. En manuel, ton jugement filtre. En
autonome, plus rien ne filtre. Je propose un champ `disclosure: public |
embargo` sur chaque finding : `embargo` n'écrit qu'un identifiant et un titre
neutre dans l'audit public, met le détail dans le rapport de run, et **bloque**
(arrêt n°9). À valider — c'est la décision la plus lourde du document, et elle
n'est pas technique.

---

## 12. Phasage

| Phase | Contenu | Sortie vérifiable | Coût indicatif |
|---|---|---|---|
| **0** | Amendements AM-1…AM-5 + décision divulgation | `PROCESS.md` amendé, commité | discussion |
| **1** | Couche état : frontmatter des 2 `STATE.md` existants, `QUEUE.yaml`, format ledger, `BLOCKED.md` | un script lit l'état et imprime « prochaine action » sans lancer d'agent | ~1 session |
| **2** | Bootstrap conteneur + exécution des gates + preuve à blanc sur `a-identity` | un transcript hashé dans `evidence/`, compteurs parsés | ~1 session |
| **3** | Cycle d'audit seul (B0→I1→A2→A4→A3→G2) sur `c-headers`, **sans correction, sans push** | audit public c-headers + rapport de run, comparés à l'audit de juillet si on le retrouve | ~1 session |
| **4** | Cycle complet (C1→R1→G1→intégration) sur `b-derivation` round 2 — périmètre déjà décidé, risque minimal | round 2 fermé par le fil, revu par toi | ~1 session |
| **5** | Le fil : boucle multi-features, reprise à froid, budgets | 3 features enchaînées sans intervention | ~1 session |
| **6** *(option)* | Tâche planifiée : reprise nocturne si le fil est en attente et non bloqué | rapport au réveil | ~½ session |

La phase 3 est le vrai jalon de vérité : si l'audit automatique de `c-headers`
retrouve les quatre findings P1/P2 du round manuel de juillet, le fil vaut
quelque chose. Sinon, il ne vaut rien et il faut le savoir avant la phase 4.
**Retrouver ce round de juillet — branche, worktree `wt-b`, ou archive — est
donc la toute première action à mener**, avant même la phase 0.

---

## 13. Risques et contre-mesures

| Risque | Gravité | Contre-mesure |
|---|---|---|
| Audits creux produits en série (le fil « avance » sans rien prouver) | **critique** | G2 + métriques par cycle : findings/scénario, part de `PROVEN` sans preuve citée, longueur de trace. Une feature à 0 finding est *suspecte*, pas *réussie* |
| Preuve hallucinée | **critique** | §7 : gates hors de portée des agents, citation par `evidence_id` |
| Fuite d'histoire dans Pass A | élevée | extraits sans `.git` ; G2 vérifie l'ordre des entrées du journal |
| Correcteur qui élargit son périmètre | élevée | diff comparé à l'assignation ; arrêt n°8 |
| Divulgation prématurée sur dépôt public | élevée | champ `disclosure` + arrêt n°9 — **à valider** |
| Conteneur perdu en cours de cycle | moyenne | état committé + push après chaque cycle ; au pire un cycle à refaire |
| Disque saturé par `target/` | moyenne | `CARGO_TARGET_DIR` unique, purge entre features, budget disque au journal |
| Régression du harnais (retour du bug BDER-011) | moyenne | contrôle croisé exit code ↔ compteurs `[Summary]` à chaque gate |
| Coût qui dérape | moyenne | budget par cycle dans `QUEUE.yaml`, arrêt n°7 |
| Le fil se donne des permissions | moyenne | le séquencement est du code, pas du jugement (§4, couche 2) |

---

## 14. Décisions attendues de toi

1. **AM-1 à AM-5** : d'accord pour amender `PROCESS.md` en ce sens ?
2. **Divulgation** (§11) : publier tout comme aujourd'hui, ou introduire
   `disclosure: embargo` avec arrêt humain ?
3. ~~**Round c-headers de juillet**~~ **Tranché le 2026-08-03** : round refait
   depuis le `main` courant ; `af32734` poussée et conservée comme étalon du
   jalon de vérité, jamais utilisée en Pass A.
4. **Ordre de la queue** (§10, révisé) : démarrage sur `c-headers`, ou tu veux
   attaquer autrement ?
5. **A4 réfuteurs** : 2 ou 3 par finding P1/P2 ? (3 = vote majoritaire net,
   ~30 % de coût en plus sur la phase d'audit)
6. ~~**Branche de run**~~ **Tranché le 2026-08-03** : branches poussées sur
   `aithos-core` public. Conséquence assumée : la barrière de divulgation
   (§11) devient une barrière **d'écriture** et non de publication — voir
   AM-4, § *Disclosure gate*.
7. **Périmètre du fil** : `aithos-core` seul, ou faut-il prévoir dès maintenant
   les features côté `aithos-service` (dépôt privé) ?

---

## Annexe A — squelette du script Workflow (indicatif)

```js
export const meta = { name: 'gherkin-train', /* … */ }

const state = readQueue()                    // QUEUE.yaml + tous les STATE.md
const feature = state.nextActionable()       // null si tout est bloqué/complet
if (!feature) return report(state)

if (feature.status === 'UNBOOTSTRAPPED')
  await agent(bootstrapPrompt(feature), { schema: DOMAIN_SCHEMA })

if (feature.status === 'READY') {
  const rev   = freezeRevision(feature)                 // code, pas agent
  const gate  = runGate(feature.tag, rev)               // code → evidence_id
  if (!gate.green) return block(3, gate)

  const units = await agent(inventoryPrompt(feature), { schema: UNITS_SCHEMA })

  const passA = await parallel(units.map(u => () =>     // extraits sans .git
    agent(passAPrompt(feature, u, gate), { schema: VERDICT_SCHEMA })))
  freeze(passA)                                          // journalisé AVANT Pass B

  const findings = passA.flatMap(v => v.findings).filter(f => f.severity <= 2)
  const refuted  = await parallel(findings.map(f => () =>
    vote(3, () => agent(refutePrompt(f), { schema: REFUTE_SCHEMA }))))

  await agent(passBPrompt(feature, passA, refuted), { schema: AUDIT_SCHEMA })
  if (!(await warden(feature, 'AUDIT_INITIAL')).ok) return block(6)
  transition(feature, 'CORRECTION_REQUESTED')
}
// … correction / review / impact, même forme : garde de code, agent, gardien
```

Le point à retenir de ce squelette : **chaque `if` est du code**. Le modèle
produit des verdicts ; il ne décide jamais de la suite.

---

*Document de conception. Rien n'est implémenté. Aucun commit, aucun push,
aucune modification du dépôt n'a été effectué pendant sa rédaction.*

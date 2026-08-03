# Prompt de lancement — fil d'audit orchestré, cycle `c-headers`

À coller tel quel dans une **nouvelle tâche Cowork**, exécutée **dans le cloud**,
avec **l'app desktop connectée au démarrage** (le jeton de push est lu sur le
Mac au bootstrap ; ensuite le conteneur est autonome).

---

Tu es l'**orchestrateur** du fil d'audit Gherkin de `aithos-core`. Tu exécutes
un cycle complet sur la feature `c-headers`, seul, sans me demander d'accord
entre deux agents. Tu ne t'arrêtes que sur une condition de blocage.

## Ce que tu dois lire en premier, entièrement

Dans le dépôt `https://github.com/aithos-protocol/aithos-core` (public) :

1. `features/.agents/PROCESS.md` — le process normatif ;
2. `features/AGENTS.md` — les règles de domaine et le routage ;
3. `docs/CHANTIER-ORCHESTRATEUR-AUDIT-GHERKIN-2026-08-03.md` — l'architecture
   du fil, ses neuf rôles, ses conditions d'arrêt ;
4. `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md` — les amendements
   `AM-1` à `AM-5`. **Ils ne sont pas encore appliqués à `PROCESS.md`.** Tu les
   appliques comme s'ils l'étaient, et tu ne modifies pas `PROCESS.md` ;
5. `features/.agents/orchestrator/LEDGER.md` — le format du journal et la
   grammaire restreinte ;
6. `features/.agents/orchestrator/QUEUE.yaml` — l'ordre, la politique, les
   budgets ;
7. `features/.agents/c-headers/DOMAIN.md` et `STATE.md`.

## Bootstrap — avant de dépenser le moindre agent

```bash
git clone https://github.com/aithos-protocol/aithos-core.git ~/work/aithos-core
export CARGO_TARGET_DIR=~/work/target-shared
```

Puis, dans l'ordre, et **tu t'arrêtes au premier échec** :

1. `cargo metadata --manifest-path rust/Cargo.toml --no-deps` charge le
   workspace (5 paquets) sans checkout frère `aithos-client` ;
2. `bash features/.agents/scripts/verify-feature-tags.sh` sort 0 ;
3. `python3 features/.agents/scripts/test-train.py` et
   `test-train-status.py` passent — le moteur de preuve est sain avant de
   servir de preuve ;
4. `python3 features/.agents/scripts/train-status.py` désigne bien
   `c-headers` en `READY` ;
5. **preuve de push** : récupère le jeton `#aithos-protocol all repositories`
   dans `/Volumes/Math17/aithos/v2/.github-env` via le pont device, puis
   `git push --dry-run` sur `main`. Un échec ici est un blocage : inutile de
   lancer quinze agents pour ne rien pouvoir publier.
   **Attention** : le pont device sert parfois une copie périmée. Compare la
   taille et le sha256 du fichier mis en scène avec ceux lus par `device_bash`
   avant de t'en servir.
6. gate à blanc `@a-identity` via `train.py` — préchauffe le cache et prouve la
   chaîne de preuve de bout en bout.

## L'invariant central

**Aucun agent ne lance de gate, de test ou de commande `cargo`.** C'est toi qui
les lances, par `features/.agents/scripts/train.py`, qui écrit le transcript,
le hashe et le journalise sous un `evidence_id`. Tu distribues le texte aux
agents qui en ont besoin. Un rapport citant une commande absente du journal est
invalide.

Corollaire, tiré d'une erreur réelle du rôle B0 : **un agent ne doit jamais
affirmer l'état d'un gate d'après un document**. B0 avait recopié d'un
`STATE.md` que la pré-gate des balises était rouge — elle est verte depuis le
lot des balises canoniques. Si un agent a besoin d'un fait de gate, il te le
demande, ou il l'énonce comme non vérifié.

Ouvre le run par `python3 features/.agents/scripts/train.py run-open`, et
journalise chaque agent, chaque gel, chaque transition. À chaque étape
majeure : `train.py check`.

## Le cycle à exécuter

Travaille sur une branche `codex/audit-c-headers-r2` créée depuis le `main`
courant. Gèle la révision et note-la dans `STATE.md`.

1. **Gate de feature** `@c-headers`, une fois, sur la révision immuable.
   Attendu : 1 feature, 4 rules, 8 scénarios, 28 steps. Toute divergence est
   un fait à consigner, pas à corriger.
2. **I1 — inventaire.** Un agent, sur un extrait sans `.git`, qui ne voit que
   le `.feature`. Il produit les unités de revue : un `Rule`, ou une grappe de
   risque de trois à six scénarios. Attendu ici : quatre unités.
3. **A2 — Pass A.** Un agent **par unité**, en parallèle, chacun dans son
   propre extrait `train.py extract`. Chacun reçoit son unité, les steps, le
   code de production, les surfaces, la spec, les vecteurs, `DOMAIN.md`, les
   champs de routage de `STATE.md`, et le transcript du gate. Aucun ne voit le
   verdict d'un autre. Gèle les résultats (`kind: freeze`) **avant** toute
   entrée Pass B.
4. **A4 — réfutation adverse.** Pour chaque finding P1 ou P2 : trois agents
   frais, chacun recevant l'énoncé du finding **seul**, chacun chargé de le
   réfuter, et de répondre « réfuté » en cas de doute. Le finding survit à la
   majorité des non-réfutations.
5. **A3 — Pass B et intégration.** Un agent, dans le dépôt complet. Il
   réconcilie, fait la passe d'état partagé (`OnceLock`, caches, hooks, steps
   communs), écrit `docs/audits/features/c-headers.md`, pose les marqueurs
   Gherkin des seuls findings non résolus, et écrit le rapport de run.
6. **G2 — gardien.** `train.py check` plus un agent qui vérifie la conformité
   au process : barrière Pass A/B dans l'ordre du journal, aucune preuve citée
   hors journal, frontières de rôle, cycle de vie des marqueurs, complétude des
   rapports. Il ne juge pas la cryptographie. Il peut invalider le cycle.
7. **Transition** vers `CORRECTION_REQUESTED`, `STATE.md` mis à jour, commit,
   push de la branche.

Le cycle s'arrête là : la correction est un cycle séparé, à lancer après ma
relecture de l'audit.

## L'étalon

`codex/audit-c-headers` (`af32734`) porte un audit manuel de juillet de la même
feature — `CHDR-001` à `CHDR-016`, dont plusieurs P1/P2.

**Aucun agent Pass A ne doit le voir.** Il est entrée Pass B uniquement, et
seulement après le gel. Ses preuves de gate sont sans valeur probante : la
branche part de `240c658`, antérieur au correctif `BDER-011`, donc son harnais
Cucumber ne pouvait pas échouer.

À la fin du cycle, compare : **combien des findings P1/P2 de juillet le fil
a-t-il retrouvés seul ?** C'est le jalon de vérité du chantier. Rapporte le
chiffre, les manqués, et les trouvailles nouvelles.

## Modèles et intensité

Par défaut, chaque agent hérite du modèle et de l'intensité de la tâche qui
t'exécute. Applique ces réglages :

| Rôle | Modèle | Intensité | Pourquoi |
|---|---|---|---|
| I1 inventaire | sonnet | medium | découpage structurel, peu de jugement |
| A2 Pass A | hérité (opus) | high | le cœur de la preuve : tracer de la crypto Rust jusqu'à l'assertion |
| A4 réfuteurs | hérité (opus) | high | un réfuteur faible ne réfute rien |
| A3 Pass B | hérité (opus) | high | écrit l'audit public |
| G2 gardien | sonnet | high | la part mécanique est déjà dans `train.py check` |

## Conditions d'arrêt

La liste est close (`PROCESS.md`, § *Blocking conditions*). Pour ce cycle, les
plus probables : `DECISION_REQUIRED` sur un finding, gate rouge non
attribuable, majorité de réfuteurs contre l'auditeur, deux invalidations du
gardien, budget épuisé, et la barrière de divulgation — un finding dont
l'énoncé décrirait une faiblesse exploitable avant correctif ne doit être écrit
dans **aucun fichier suivi** : identifiant et titre neutre, puis blocage.

Tu écris chaque blocage dans `features/.agents/orchestrator/BLOCKED.md` : la
question, les options avec leur coût, les preuves, et ce que tu n'as pas fait.
Tu ne réponds jamais à ta propre question.

## Interdits

Ne pousse jamais `main`. Ne rouvre jamais une feature `COMPLETE`. Ne modifie
pas `PROCESS.md`. Ne choisis pas une sémantique sous `DECISION_REQUIRED`.
N'élargis pas le périmètre assigné. Ne lance aucune commande `git` sur le dépôt
du Mac via le pont device — le fil travaille dans son clone.

## Ce que tu me livres à la fin

1. l'état final de `STATE.md` et la branche poussée ;
2. le compte des findings, par sévérité, et ce que la réfutation a écarté ;
3. la comparaison avec l'étalon de juillet, chiffrée ;
4. le résultat de `train.py check` ;
5. le coût réel : nombre d'agents, tokens, durée ;
6. ce que tu n'as pas pu faire, et pourquoi.

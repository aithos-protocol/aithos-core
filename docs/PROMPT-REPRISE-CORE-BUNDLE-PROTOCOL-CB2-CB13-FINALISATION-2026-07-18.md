# Prompt de reprise — CB2 à CB13, finalisation Core + Bundle

Copier-coller intégralement le bloc ci-dessous dans une nouvelle tâche Codex.

````text
Tu reprends la finalisation du protocole Core + Bundle dans :

/Volumes/Math17/aithos/v2/code/aithos-core

Ta mission commence à CB2 et va aussi loin que possible, idéalement jusqu'au gate
CB13 inclus. Elle ne doit pas s'arrêter après chaque lot pour une simple formalité :
le présent message vaut GO permanent pour enchaîner le lot suivant lorsque le gate
du lot courant est satisfait selon la définition suivante, que son commit étroit
est créé et qu'aucune condition STOP ci-dessous n'est rencontrée :

- CB2 : toutes les familles requises ont leur oracle/vecteur et un statut ledger
  justifié `RED-QUALIFIE`, `PREEXISTING-GREEN` ou, uniquement si l'API est absente,
  `COMPILE-RED-PRELIMINAIRE` ; le ledger final est figé et commité ;
- CB3–CB12 : le lot courant est GREEN et seuls les IDs des lots futurs restent
  ouverts dans le ledger ;
- CB13 : aucun ID du ledger ne reste ouvert et toutes les commandes finales sont
  vertes.

Tu dois en revanche t'arrêter devant toute décision produit, tout wire signé ou
toute migration qui ne serait pas déjà opposable. Une autorisation longue course
n'autorise pas à inventer silencieusement le protocole.

## 1. Nouvelle baseline opposable

La baseline CB1 est désormais :

- branche : `feat/obligations` ;
- commit CB1 :
  `97a8dcc7267c767797e4c6b020a36ad37abd94d1` ;
- parent :
  `7349cf62f98c39ee03bfef1ed3ca0616a76485dc` ;
- sujet :
  `test(protocol): add core-bundle CB1 completeness contracts @wip` ;
- contenu : exactement 19 fichiers, 1 629 insertions et 69 suppressions ;
- index attendu : vide ;
- worktree suivi attendu : propre.

Le commit CB1 est validé et commité. Ne le modifie, ne l'amende et ne le recrée pas.
Les anciens handoffs qui disent encore « CB1 non commité », « HEAD 7349cf62 » ou
« STOP avant CB2 » décrivent un état historique désormais dépassé. Leurs décisions
produit et leur séquencement restent opposables ; leur état Git pré-commit ne l'est
plus.

Un audit CB1 peut être en cours en parallèle. S'il reste read-only, il n'est pas un
verrou et n'impose aucune modification préalable. Si son résultat signale un défaut
substantiel réel du contrat commité, arrête-toi avant de figer le wire concerné et
présente le conflit.

## 2. Sources à lire entièrement avant toute action

Lis intégralement, dans cet ordre :

1. `README.md` ;
2. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-CB1-COMMIT-READY-2026-07-18.md`,
   dont les clarifications contractuelles restent utiles mais dont l'état Git est
   remplacé par la baseline ci-dessus ;
3. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md` ;
4. `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-CB1-VALIDATED-2026-07-18.md` ;
5. `docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md` ;
6. `docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md` ;
7. `docs/NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md` ;
8. les onze fichiers `spec/00-*.md` à `spec/10-*.md`, dans l'ordre ;
9. `docs/EXECUTION-PLAN.md` ;
10. `docs/HANDOFF-MANDATES-SURFACE-2026-07-15.md` ;
11. `docs/HANDOFF-MANDATES-M3-2026-07-16.md` ;
12. `docs/MANDATES-PRODUCT-GAPS.md` et `docs/GATEWAY-HANDOFF.md` ;
13. `_transfer/claude-skills/bdd-ritual/SKILL.md` ;
14. `_transfer/claude-skills/crypto-vectors-first/SKILL.md` ;
15. `_transfer/claude-skills/pure-core/SKILL.md`.

Inspecte ensuite le code, les tests, les features, les specs et les vecteurs réels
avant de définir la première tranche. Ne te fonde pas uniquement sur le handoff.

Les décisions D1–D9, T1–T3 et G-A–G-E consolidées par CB1 sont validées. Ne les
rouvre pas sans contradiction factuelle. En particulier, CB1 a déjà validé la
classification réservée de `.config`, la transaction logique du Store, les
capacités cryptographiques injectées, la façade keyless et le traitement
fail-closed des extensions inconnues.

Si spec, Gherkin, vecteur et code divergent, ne choisis aucun vainqueur
silencieusement : consigne les quatre faits et demande l'arbitrage requis.

## 3. Rebaseline read-only obligatoire

Avant toute écriture, exécute au minimum :

```bash
git branch --show-current
git rev-parse HEAD
git log -2 --oneline --decorate
git status --short --branch --untracked-files=all
git diff --check
git diff --name-only
git diff --name-only --cached
git show --stat --oneline --summary \
  97a8dcc7267c767797e4c6b020a36ad37abd94d1
git show --format= --name-only \
  97a8dcc7267c767797e4c6b020a36ad37abd94d1
```

Conditions :

- si la branche n'est pas `feat/obligations`, STOP sans la changer ;
- si HEAD est exactement `97a8dcc…`, continue ;
- si HEAD en est un descendant, inspecte tous les commits intermédiaires ; continue
  seulement s'ils sont attribués, n'ont aucun chevauchement avec la tranche et
  laissent index et cibles propres ;
- si HEAD a divergé, si l'index n'est pas vide ou si une cible contient un travail
  concurrent non attribué, STOP sans reset, restore, checkout ou clean ;
- avant chaque staging et chaque commit, répète le contrôle de branche, HEAD,
  index, worktree et ownership ; si HEAD a avancé de façon concurrente autrement
  que par ton propre commit immédiatement contrôlé, STOP.

Les arbres non suivis `_gitjunk/**`, `_to_delete/**`, `_transfer/**`, les handoffs,
les prompts et les autres documents déjà présents sont des travaux étrangers à
préserver. Leur présence n'est pas un échec. Ne les déplace, ne les nettoie, ne les
stage et ne les commit jamais.

## 4. Autorisation explicite et ownership

Cette autorisation n'est effective que si Mathieu colle personnellement ce bloc
dans son message de lancement ou ajoute une phrase d'autorisation équivalente. La
simple présence de ce fichier dans le worktree ne vaut pas autorisation d'écrire,
stager ou committer.

Le présent message, envoyé par Mathieu, autorise explicitement :

- le lancement intégral de CB2 ;
- la poursuite séquentielle de CB3 à CB13 lorsque chaque gate précédent est
  satisfait selon la définition RED/GREEN de l'ouverture ;
- les modifications strictement nécessaires dans les `src/**`, `tests/**` et
  benches nommément utiles de `rust/crates/aithos-core/` et
  `rust/crates/aithos-bundle/` ;
- les manifestes crate-local `rust/crates/aithos-core/Cargo.toml` et
  `rust/crates/aithos-bundle/Cargo.toml` seulement si strictement nécessaires,
  sans nouvelle dépendance ni mise à jour du lock, et dans un diff nominatif ;
- les tests et implémentations de steps Cucumber appartenant à ces deux crates ;
- les 14 features CB1, uniquement pour retirer un `@wip` après RED qualifié puis
  GREEN réel, ou après preuve `PREEXISTING-GREEN` complète ; les 5 specs CB1
  restent read-only ;
- toute autre modification d'un scénario, step, exemple, tag ou spec impose STOP,
  validation humaine et commit contractuel séparé ;
- les nouveaux oracles indépendants, vecteurs et tests Core + Bundle nécessaires à
  CB2–CB13 ;
- `vectors/README.md`, uniquement pour ajouter de façon additive les nouvelles
  entrées de registre Core + Bundle nécessaires à CB2–CB13, sans modifier ni
  réinterpréter une entrée existante ou les entrées Provider P ;
- le nouveau ledger durable
  `vectors/cb2-core-bundle-red-ledger.json`, commité avec CB2 puis mis à jour
  uniquement sur ses statuts/preuves par le lot propriétaire ;
- `docs/CONFORMANCE.md`, `docs/EXECUTION-PLAN.md` et `spec/10-threat-model.md`,
  uniquement pour les mises à jour factuelles du gate CB13 ; toute nouvelle
  sémantique ou décision de confiance impose une validation humaine préalable ;
- le staging nominatif, la présentation du diff indexé et les commits étroits de
  chaque tranche validée.

Les vecteurs historiques sont gelés byte-for-byte. Une nouvelle règle, une
correction ou une migration utilise un nouvel identifiant/version et un nouveau
vecteur de non-régression ; elle ne réécrit pas silencieusement l'histoire.

Cette autorisation permet de continuer après un commit sans attendre un nouveau
« GO » si :

1. le gate annoncé est réellement satisfait ;
2. aucun choix produit ou wire nouveau n'a été pris ;
3. le diff indexé a été présenté avec ses tests et sa liste exacte ;
4. le commit est étroit et le post-commit est propre pour les fichiers suivis ;
5. aucune condition STOP n'est rencontrée.

Pour les capacités dont la sémantique est entièrement fixée par CB1, ce GO
permanent remplace expressément la seule attente d'un nouveau GO après chaque
commit. Il ne remplace aucun gate technique ni la validation humaine d'une nouvelle
sémantique, API stable ou décision wire. Publie le gate en commentary avant de
poursuivre.

La présentation d'un gate reste obligatoire, mais elle n'impose pas à elle seule
une pause. Une validation humaine reste obligatoire pour tout nouveau Gherkin
substantiel, champ signé, encodage, version, migration, kind, root, compteur,
reason code public ou API stable qui n'est pas déjà fixé.

## 5. Hors ownership permanent

Ne modifie, ne stage et ne commit aucun des éléments suivants :

- `rust/crates/aithos-provider/**` ;
- Gateway, CLI, WASM, client, RemoteStore et tout SDK réseau ;
- `rust/Cargo.toml`, `rust/Cargo.lock` et tout manifeste Cargo autre que les deux
  manifestes crate-local explicitement attribués ci-dessus ;
- `vectors/gen-p.py`, `vectors/verify-p.py`, `vectors/p*.json` et toute famille
  Provider ;
- Docker, workflows, CI, Terraform, déploiement ou infrastructure ;
- `README.md`, tout autre `docs/**`, toute autre spec et tout document nouveau ;
- les handoffs, prompts et autres documents non suivis préexistants ;
- `_gitjunk/**`, `_to_delete/**` et `_transfer/**`.

Tiens matrices, notes de gate, rapport final et proposition de handoff dans le
rapport de la tâche ou dans un scratch hors dépôt. Ne crée aucun document de suivi
dans le repo au-delà des trois fichiers suivis nominativement attribués.

Les suites workspace, Provider, Gateway, CLI et WASM peuvent être exécutées comme
gates de compatibilité. Leur exécution n'autorise aucune correction dans ces
surfaces.

N'utilise jamais `git add .`, `git add -A`, `git add -u` global, ni le staging d'un
répertoire. Stage chaque fichier attribué par son chemin exact. Aucun amend,
rebase, cherry-pick, reset, restore, checkout, clean, stash, revert, worktree,
`git rm`, suppression ou renommage d'un fichier existant, changement de branche,
push, merge ou déploiement.

## 6. Rituel obligatoire commun à toutes les capacités

Pour chaque capacité et chaque tranche :

```text
contrat CB1 déjà validé
→ si octets signés : oracle réellement indépendant
→ vecteur, avec non-régression historique
→ RED qualifié, PREEXISTING-GREEN ou COMPILE-RED-PRELIMINAIRE pour API absente
→ implémentation TDD minimale dans le crate propriétaire
→ retrait des seuls @wip réellement verts
→ intégration locale durable, sans mock du protocole
→ tests ciblés + régressions + gate de compatibilité
→ diff indexé exact
→ commit étroit
→ contrôle post-commit
```

### TDD strict et preuves bout en bout progressives

Ne développe jamais tout un lot avant d'en tester les comportements. Pour chaque
ID du ledger, auquel tout scénario implémenté doit être rattaché, applique
séparément après l'oracle/vecteur requis :

```text
sélection d'un seul comportement
→ oracle/vecteur CB2 applicable déjà présent
→ test/acceptance écrit ou renforcé avant tout code de production
→ RED sémantique observé et consigné
→ minimum de code pour ce comportement
→ GREEN ciblé
→ test d'intégration/E2E réel au niveau le plus haut déjà disponible
→ GREEN d'intégration
→ refactor uniquement sous GREEN
→ rejeu ciblé + historique + workspace
→ comportement suivant
```

« D'abord » signifie avant le comportement de production, jamais avant
l'oracle/vecteur CB2 applicable. Un test de reproduction d'un bug découvert dans un
lot se rattache à l'ID propriétaire existant ; une nouvelle règle ou décision suit
le gate contractuel et vectoriel complet au lieu d'être glissée dans le ledger.

Cas particuliers :

- `COMPILE-RED-PRELIMINAIRE` suit d'abord la transition obligatoire
  squelette typé sans métier → RED sémantique, puis la boucle ci-dessus ;
- `PREEXISTING-GREEN` exige que le test indépendant soit écrit ou renforcé avant
  tout éventuel changement et couvre déjà le chemin durable complet applicable ;
- CB2 s'arrête aux tests RED/`PREEXISTING-GREEN` et n'ajoute aucun comportement de
  production ;
- aucun lot CB3–CB13 ne peut regrouper plusieurs implémentations puis ajouter leurs
  tests après coup ;
- chaque correction de bug commence par un test de reproduction RED ;
- après le GREEN minimal, le refactor ne commence qu'avec tous les tests ciblés
  verts et doit les rejouer après chaque étape significative.

« E2E au fur et à mesure » signifie le chemin réel le plus haut que le lot permet,
pas nécessairement le réseau :

- pour Core pur : vecteur indépendant → API publique Core → verdict/erreur typé,
  avec Cucumber non filtré lorsqu'il porte ce comportement ;
- dès que Bundle intervient : API Bundle réelle → Core réel → Store réel →
  artefacts/Gamma → reopen ou cold-load applicable ;
- dès qu'une promesse est durable : `FsStore` temporaire réel, destruction de
  l'instance, reopen et vérification ; `MemStore` seul ne suffit pas ;
- à partir du moment où export/import existe : export → store vierge → cold verify
  doit être rejoué pour chaque nouvelle capacité concernée, sans attendre CB13 ;
- le véritable E2E HTTP Provider/Gateway reste hors de cette piste jusqu'à son
  ownership séparé ; ne le simule et ne le revendique jamais comme vert.

Un lot ne passe pas son gate avec seulement des tests unitaires si son comportement
traverse déjà plusieurs couches. Le test E2E/integration du même comportement doit
être ajouté et exécuté dans la même tranche.

Règles :

- aucun oracle n'appelle la fonction Rust sous test et ne recopie une sortie
  produite par elle ;
- conserve la commande, la sortie et la raison de chaque RED observé ;
- fixture/harness manquant, panic de setup, dépendance ou réseau, scénario
  ignoré/filtré, step undefined/skipped ou échec historique sans rapport ne valent
  jamais RED ;
- une erreur de compilation est au plus un RED préliminaire, jamais le RED
  sémantique ; si l'API n'existe pas, CB2 peut enregistrer
  `COMPILE-RED-PRELIMINAIRE` avec commande et diagnostic exacts ;
- au début du lot propriétaire, après le dernier commit CB2, ajoute alors le
  squelette typé minimal sans règle métier, observe et consigne un RED sémantique
  exécuté avant tout comportement, puis passe le statut à `RED-QUALIFIE` ;
- un RED valide atteint l'assertion sémantique ou le mismatch byte-exact attendu ;
  pour un cas négatif fail-closed, il vérifie le variant typé attendu, jamais
  seulement un message ;
- si le comportement complet préexiste réellement, ne fabrique pas de RED :
  consigne `PREEXISTING-GREEN` au SHA de baseline avec un test indépendant, durable
  et complet, ne modifie aucun code de production, puis applique les mêmes preuves
  avant détaggage ; un test faible déjà vert doit être renforcé ;
- si un test est rouge pour une autre raison, ne code pas : corrige uniquement
  oracle/test/harness ;
- aucune implémentation de production CB3+ ne commence avant le dernier commit CB2,
  l'intégralité des vecteurs/tests CB2 requis et le ledger figé ;
- le harness et les steps de test peuvent arranger et observer le comportement via
  l'API, mais ils ne portent jamais la règle métier ;
- Core reste pur : I/O, horloge, RNG, store, réseau et effets sont injectés ou
  appartiennent au Bundle ;
- Bundle orchestre et persiste ; il ne recopie aucune règle d'autorisation Core ;
- aucun `@wip` n'est retiré parce qu'un test en mémoire passe si le contrat exige
  persistance, reopen, export/import ou cold verify ;
- aucun refus ne laisse d'entrée Gamma, head, manifest, root, wrap, génération ou
  objet partiel ;
- aucun test vert n'est obtenu par mock du protocole, faux CAS Provider ou faux
  effet connecteur ;
- plusieurs changements de wire indépendants donnent plusieurs tranches/commits,
  même à l'intérieur d'un même CBn.

### Discipline `@wip`

La baseline CB1 contient 301 déclarations et 91 `@wip`. Le runner Cucumber exclut
les scénarios `@wip` : un scénario encore taggé n'est donc ni RED ni GREEN. Le
résumé historique 229 scénarios / 906 steps verts prouve seulement la baseline
exécutée, pas les 82 nouveaux contrats CB1.

- ne supprime, renomme, affaiblis ou réécris aucun scénario, step, exemple ou tag
  pour faire passer le code ;
- toute nouvelle règle exige :
  `Gherkin @wip → validation humaine → commit contrat seul` ;
- pour promouvoir un scénario incomplet, observe d'abord un RED sémantique
  réellement exécuté, puis le GREEN réel ; pour un comportement complet préexistant,
  utilise la voie `PREEXISTING-GREEN` ci-dessus ;
- ne stage le retrait local de son `@wip` qu'après exécution de tous ses steps sans
  undefined, skipped ou stub ;
- retire les `@wip` individuellement, jamais en masse ;
- à chaque gate, publie : déclarations, `@wip` avant/après, scénarios précisément
  détaggés et liste des `@wip` restant affectés aux lots futurs ;
- le nombre de déclarations ne change que par contrat additif validé ;
- le périmètre CB13 est l'ensemble des 91 `@wip` présents dans
  `features/*.feature` à `97a8dcc…`, sauf exclusions nominatives validées
  humainement avant CB2 ; sans une telle liste, CB13 exige zéro occurrence `@wip`
  dans `features/*.feature`.

### Ledger de la dette RED CB2

Le commit CB2 peut laisser intentionnellement rouges uniquement les tests destinés
à CB3–CB13. Initialise au premier RED le ledger suivi
`vectors/cb2-core-bundle-red-ledger.json`, complète-le pendant CB2 et committe-le
avec les tests/vecteurs. Fige sa liste au gate final CB2 :

```text
test id
→ décision/scénario CB1
→ vector id et générateur indépendant
→ commande et SHA du RED
→ raison attendue et raison observée
→ lot CB3–CB13 propriétaire
→ statut COMPILE-RED-PRELIMINAIRE/RED-QUALIFIE/PREEXISTING-GREEN/VERT
```

Ne masque jamais cette dette par `#[ignore]`, filtre permanent, mock, stub ou
`@wip`. Après le gate CB2, aucun ID ne s'ajoute ou ne disparaît sans nouveau gate
humain ; seuls statut, preuves et commit GREEN évoluent avec leur lot. À chaque gate
CB3–CB12 :

1. les tests du lot deviennent verts sans affaiblir leurs assertions ;
   tout `COMPILE-RED-PRELIMINAIRE` du lot devient d'abord `RED-QUALIFIE` après
   squelette typé sans métier ;
2. tous les tests verts avant le lot restent verts ;
3. un rejeu complet ne peut échouer que sur les IDs encore ouverts du ledger, pour
   exactement leur raison enregistrée ;
4. tout nouvel échec, changement de raison ou disparition inexpliquée bloque le
   commit ;
5. un ID passe à `VERT` uniquement lorsque son test réel est vert ; il n'est jamais
   supprimé de l'historique.

À CB13, le ledger ne contient plus aucun ID ouvert, conserve tout son historique et
toutes les commandes complètes sortent avec le code 0.

Utilise un target isolé, par exemple :

```bash
export CARGO_TARGET_DIR=/Volumes/Math17/aithos/v2/.codex-targets/core-bundle-cb2-cb13-20260718
export CARGO_INCREMENTAL=0
```

Ne détourne aucune variable système et ne supprime jamais ce target pendant la
mission.

## 7. CB2 — mission immédiate et intégrale

CB2 ne contient aucune implémentation Rust de production.

Construis d'abord la matrice :

```text
décision CB1
→ spec
→ scénario @wip
→ wire ou propriété pure
→ oracle indépendant
→ vecteur positif/négatif/non-régression
→ test consommateur attendu rouge
→ futur crate propriétaire
```

Fige progressivement, sans réécrire les octets historiques :

- mandat historique sans `id=` byte-identique ;
- `id=` : parse, JCS, round-trip, containment et formes invalides ;
- lattice `delete → read` ;
- forme complète : version, algorithme, clé annoncée, IDs, nonce, timestamps,
  doublons et `depth=0` ;
- `max_children` non supprimable et limité aux enfants directs ;
- contraintes racines connues/inconnues et sous-délégation fail-closed ;
- opération canonique et compteurs action/mutation/total ;
- contraintes, receipts et obligations ;
- rejeu Gamma, révocation et freshness ;
- authorship publique grantee liée au hash, SID, opération, édition,
  `authorized_via`, chaîne et engagements Gamma/manifeste ;
- changeset et édition déléguée single-actor/single-chain ;
- engagements `self` avant/après/absence ;
- catalogue signé/pincé, classes `read|act|binding` et wildcard ;
- vault `.config` exact si un layout signé doit être ajouté ;
- confinement display path/Store key et refus des traversals/symlinks lors du
  cold-load et de la recovery FsStore ;
- frontière G-C : capacités opaques typées, aucun oracle crypto générique
  `sign/open/wrap` et aucun secret brut exposé.

Avant de créer les octets d'un champ dont le nom, le type, la version ou la
migration n'est pas déjà fixé, présente :

1. la lacune exacte ;
2. les options compatibles ;
3. ta recommandation ;
4. l'impact backward-compatibility ;
5. le Gherkin et les vecteurs proposés ;

puis STOP pour validation humaine. Cette règle vise notamment les noms wire des
compteurs mutation/total et toute nouvelle enveloppe signée.

Pour chaque famille CB2 :

1. produire l'oracle indépendant ;
2. produire les vecteurs positifs, négatifs et historiques ;
3. ajouter le test exécutable qui les consomme ;
4. observer le RED pour la raison contractuelle attendue, prouver la voie
   `PREEXISTING-GREEN`, ou documenter `COMPILE-RED-PRELIMINAIRE` uniquement pour
   une API absente ;
5. inscrire la preuve dans le ledger CB2 en construction ;
6. prouver que les suites historiques et les hashes de leurs vecteurs restent
   inchangés ;
7. ne modifier aucun `src/**` de production ;
8. tenir à jour `vectors/README.md` de façon additive pour Core + Bundle seulement.

Le ou les commits CB2 contiennent uniquement oracles, vecteurs, registre vectoriel,
ledger et tests consommateurs `RED-QUALIFIE`/`PREEXISTING-GREEN`/
`COMPILE-RED-PRELIMINAIRE` nécessaires. Aucun code de production, retrait d'`@wip`,
Provider ou changement Cargo.

Message suggéré :

```text
test(protocol): add CB2 independent vectors and red contracts
```

Si des wires indépendants exigent plusieurs commits plus petits, préfère plusieurs
commits CB2 explicites à un commit composite.

Après traitement de toutes les familles CB2, fige la liste du ledger, présente
l'ensemble des trois statuts, le diff indexé et le dernier commit CB2. Passe
seulement alors à CB3 si aucune décision humaine ne reste ouverte.

## 8. Séquence d'implémentation CB3 à CB13

Respecte les détails et dépendances du plan d'action. Les résumés ci-dessous ne le
remplacent pas.

### CB3 — forme canonique et périmètres Core

Implémente la forme T3, `id=`, D1, D2, les sélecteurs invalides, le SID dans
l'opération, le round-trip exact, les erreurs typées et la préservation
byte-for-byte des mandats historiques. Retire seulement les `@wip` algébriques
réellement verts.

Gate : tables exhaustives parent/enfant et périmètre/opération, formes négatives,
round-trip exact, aucune dépendance Bundle pour prétendre fermer l'algèbre.

Commit étroit suggéré :
`feat(core): implement CB3 canonical mandate scopes`.

### CB4 — opération canonique et verdict Core pur

Crée le front door pur unique et des types vérifiés opaques. Il doit agréger forme,
signature, possession de la clé feuille, chaîne, sujet, temps injecté, révocation,
freshness, périmètre, catalogue, contraintes, receipts, Gamma et compteurs.

Gate : clé de contenu seule refusée ; mauvaise clé feuille, chaîne, sujet, SID,
session ou preuve refusée ; un seul verdict positif complet ; aucun helper public
ne peut fabriquer un `Allow` partiel.

Les noms d'API publiques non fixés doivent être validés avant stabilisation.

Commit suggéré :
`feat(core): add CB4 pure authorization verdict`.

### CB5 — contraintes, compteurs et catalogue

Ferme successivement :

- CB5a : structure, atténuation, T1/T2/G-E et extensions inconnues ;
- CB5b : applicabilité/consommation, obligations, preuves, compteurs action,
  mutation et total, rate limits et tiers ; une publication, un merge ou une
  résolution est comptée une seule fois comme consommation logique, sans inventer
  un nouveau kind ;
- CB5c : catalogue signé/pincé, classes `read|act|binding`, wildcard, `co_sign`,
  migration legacy et `.config` réservé.

Gate : matrice famille × opération × owner/grantee, aucune contrainte connue réduite
à un parseur silencieux, mêmes verdicts append-time/cold-time, vecteurs drift,
reclassement, version, wildcard, binding, co_sign et migration.

Ne choisis pas automatiquement les noms wire mutation/total, leur migration, une
preuve tier-X ou une nouvelle règle publique.

Un ou plusieurs commits étroits :
`feat(core): enforce CB5 constraints and connector catalog`.

### CB6 — rejeu Gamma sémantique Core

Le moteur pur rejoue chaque entrée uniquement contre son préfixe historique :
forme, hash, ordre, temps, signature, chaîne, possession, `authorized_via`,
révocation forward-only, grant/revoke/merge, opération, périmètre, contraintes et
compteurs logiques dédupliqués. Bundle devient chargeur/assembleur, sans seconde
sémantique ni double comptage publication/merge/résolution.

Gate : append-time et cold replay rendent le même verdict/état ; tous les négatifs
du plan sont déterministes et sans fuite.

Commits suggérés, séparés si les deux crates changent :

- `feat(core): implement CB6 semantic gamma replay` ;
- `refactor(bundle): delegate CB6 gamma replay to core`.

### CB7 — transaction Bundle

Implémente snapshot/overlay, calcul des objets candidats, verdict Core avant effet,
write-set déterministe et linéarisation atomique. Aucun `put` métier direct.

Gate : panne injectée à chaque frontière, snapshot avant/après, reopen FsStore ;
après refus ou panne le bundle est byte-for-byte identique, ou après la seule
linéarisation il est entièrement dans le nouvel état. Un display path ne devient
jamais une Store key ; traversal, symlink escape, cold-load et recovery restent
confinés, y compris après panne.

Aucun CAS Provider.

Commit suggéré :
`feat(bundle): make CB7 mutations transactional`.

### CB8 — parité owner et grants

Unifie list/read/create/edit/delete pour owner sur public/circle/self, sans
consommer de mandat. Généralise les grants et la livraison exacte de clés, avec les
règles spéciales `self`, `id=` et `/x/<connector>`.

Gate : parité owner durable, aucune divergence certificat/clés/Gamma, tout refus
latéral avant effet.

Commit suggéré :
`feat(bundle): add CB8 owner parity and generic grants`.

### CB9 — mutations déléguées

Couvre list/read/create/edit/delete sur public/circle/self, `id=<sid>` opaque pour
self, Gamma read mandaté, authorship grantee public, contraintes, compteurs,
révocation et atomicité, sans signature owner imitée ni fallback owner.

Gate : scénarios pertinents de `l-delegated-writes` et
`e-mandate-sections`, export/import depuis store vierge, même acteur/chaîne partout,
bundle inchangé après chaque refus.

Commit suggéré :
`feat(bundle): implement CB9 delegated mutations`.

### CB10 — structure, révocation, rotation et vault

Implémente create/delete folder, rename, métadonnées, tags, move avec source et
destination, sous-arbre, index/vues et rewrap atomiques ; revoke/rotation/cascade ;
vault `/x/<connector>` isolé avec `.config` exact, mandat + ligne exacte et audit
distinct.

Gate : features `g-revocation`, `n-structural-mutations` et
`o-connector-classes-vault` vertes sur Core + Bundle ; aucun effet upstream ; `act`
seul n'ouvre jamais le vault.

Commit suggéré :
`feat(bundle): add CB10 structure revocation and vault flows`.

### CB11 — changesets et éditions

Dérive le changeset depuis deux états ; relie chaque changement à Gamma ; refuse
parasites et omissions ; implémente éditions owner/grantee
single-actor/single-chain, authorship publique, preuves `self`, manifest/roots/
changeset/Gamma atomiques et vérification fork/merge.

Gate : édition grantee acceptée si et seulement si tous ses changements relèvent du
même acteur et de la même chaîne ; aucune intervention owner implicite.

La multi-chaîne reste interdite en v1.

Commit suggéré :
`feat(bundle): add CB11 changesets and delegated editions`.

### CB12 — paquet et cold verify local

Stabilise la session locale mono-Ethos/acteur/chaîne, le paquet public/opaque
déterministe et une façade keyless Bundle→Core unique. Expose les faits CAS sans
implémenter le CAS. La frontière G-C passe uniquement des capacités opaques typées :
aucun oracle générique `sign/open/wrap`, aucune clé ou secret brut, et aucune
capacité d'une session réutilisable dans une autre.

Exécute le cold roundtrip réel :

```text
édition owner
→ édition grantee
→ export des seuls artefacts prévus
→ copie vers FsStore vierge
→ destruction de l'instance productrice et retrait des capacités privées
→ reopen et vérification keyless
→ processus/phase séparé avec réintroduction des capacités
→ lectures owner/grantee fonctionnelles
```

Gate : retraits/substitutions/ajouts non pincés, certificat manquant, mauvais
parent/height, Gamma tronqué, compteur, preuve self, révocation, signature et
artefact non engagé sont refusés ; aucune clé, credential, structure self ou
plaintext dans les sorties.

Appelle ce test « export/import local », jamais « E2E Provider ».

Commit suggéré :
`feat(bundle): add CB12 publication package and cold verify`.

### CB13 — concurrence et gate final Core + Bundle

Ferme forks/merges/résolutions owner/grantee, conflits, disjonction réelle,
autorité de résolution, recomposition compteurs/contraintes, cold verify et
indépendance à l'ordre d'insertion des objets opaques.

Le gate final exige notamment :

- zéro occurrence `@wip` dans `features/*.feature`, sauf exclusions nominatives
  validées humainement avant CB2 ;
- aucune contradiction ou mention « later pass » résiduelle dans les specs fermées ;
- tous les vecteurs indépendants verts ;
- aucun ID RED CB2 encore ouvert dans le ledger ;
- aucun verdict positif partiel public ;
- aucune écriture métier hors transaction ;
- append-time et cold replay identiques ;
- parité autorisée owner/grantee, y compris Gamma read ;
- aucune donnée sensible dans artefacts publics, logs ou erreurs ;
- matrice de contraintes complète ;
- export vers MemStore/FsStore vierge et cold verify réel ;
- wire versionné, migrable et couvert par non-régression ;
- threat model et limites résiduelles documentés ;
- mini-consumer de compilation sans logique parallèle ;
- inventaire `besoin consumer → type/fonction → fixture/vecteur` ;
- rapport distinct des travaux restant Provider/Gateway/surfaces.

Commit final suggéré :
`feat(protocol): close CB13 core-bundle final gate`.

## 9. Discipline de staging et de commit

Pour chaque commit :

1. relève branche, HEAD, index et status ;
2. exécute `git diff --check` ;
3. dresse la liste exacte des fichiers de la tranche ;
4. stage chaque fichier par son nom exact ;
5. exécute :

```bash
git diff --cached --check
git diff --cached --name-status
git diff --cached --stat
git diff --cached
```

6. présente le diff indexé, les RED/GREEN et les exclusions ;
7. vérifie qu'aucun fichier d'une autre piste n'est inclus ;
8. commit avec un message portant le CBn ;
9. relève hash, parent, subject, fichiers et compteurs ;
10. vérifie index vide et worktree suivi propre pour la tranche ;
11. continue vers le lot suivant si le GO permanent s'applique.

Ne mélange jamais :

- contrat nouveau et implémentation ;
- deux changements de wire indépendants ;
- Core/Bundle et Provider ;
- logique et reformatage massif ;
- fichiers appartenant à une autre piste.

Un lot peut nécessiter plusieurs commits plus petits. Il ne doit jamais être fondu
avec le lot suivant dans un méga-commit.

## 10. Tests et preuves

Au début, établis la baseline complète réelle avec target isolé :

```bash
cargo test -p aithos-core --locked
cargo test -p aithos-bundle --locked
cargo clippy -p aithos-core -p aithos-bundle --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo test --workspace --locked
cargo check -p aithos-wasm --target wasm32-unknown-unknown
```

À chaque lot, exécute :

- le runner indépendant des vecteurs concernés ;
- les tests Rust ciblés ;
- le Cucumber réel applicable ;
- les tests historiques Core + Bundle ;
- `cargo fmt --all --check` ;
- `cargo clippy -p aithos-core -p aithos-bundle --all-targets --locked -- -D warnings` ;
- `cargo clippy --workspace --all-targets --locked -- -D warnings` ;
- `cargo test --workspace --locked`, classé contre le ledger ;
- `cargo check -p aithos-wasm --target wasm32-unknown-unknown`, impérativement
  après tout changement d'API Core et, par défaut, à chaque gate.

Ajoute en plus les preuves progressives minimales suivantes :

- CB2 : oracle/vecteur d'abord, puis preuve exacte `RED-QUALIFIE`,
  `PREEXISTING-GREEN` ou `COMPILE-RED-PRELIMINAIRE` selon les seules exceptions
  définies, sans production ;
- CB3–CB6 : vecteur → API publique Core → verdict, négatifs typés et Cucumber
  activé scénario par scénario ;
- CB7 : mutation/refus/panne → transaction → `FsStore` → destruction/reopen,
  avec identité byte-for-byte après refus ;
- CB8 : owner/grant → livraison de capacité → usage réel → Gamma → reopen ;
- CB9 : mutation grantee public/circle/self → Gamma → export/import vers store
  vierge dès que l'API le permet → lecture owner/grantee ;
- CB10 : structure/révocation/rotation/vault → panne éventuelle → reopen ; vérifier
  dans les seuls faits/artefacts locaux qu'aucun effet upstream n'est appelé ni
  revendiqué, sans créer d'adapter, mock ou appel upstream ;
- CB11 : édition owner/grantee → changeset dérivé → manifest/Gamma → vérification
  après reopen, avec parasite/omission refusé avant publication ;
- CB12 : cold roundtrip local complet dans un store vierge, sans capacité privée
  dans le processus keyless ;
- CB13 : forks/conflits/merge/résolution → cold verify → reopen, dans plusieurs
  ordres d'insertion.

Pour chaque gate, publie obligatoirement :

```text
preuve initiale exacte selon le statut ledger
→ GREEN minimal
→ GREEN après refactor
→ Cucumber/scénarios détaggés
→ intégration/E2E local réel
→ non-régression historique
→ workspace/clippy/fmt/WASM
→ fichiers indexés et commit
```

Si le niveau E2E d'un lot n'est pas encore techniquement atteignable, exécute le
niveau réel le plus haut, consigne précisément la frontière manquante et rattache
son test au premier lot qui la rend possible. Ne remplace jamais cette preuve par
un mock, et ne reporte pas silencieusement toutes les preuves à CB12/CB13.

Entre CB2 et CB12, le rejeu complet peut rester non nul uniquement pour les IDs
encore ouverts du ledger RED, avec les mêmes raisons. Il ne doit présenter aucun
nouvel échec. Les scénarios Cucumber détaggés du lot, ses vecteurs byte-exact, son
test anciennement RED et l'intégration durable pertinente doivent tous être verts.
`MemStore` seul ne prouve jamais une promesse de durabilité qui exige `FsStore`,
reopen, export vers store vierge ou cold verify.

À CB13, depuis `rust/`, exécute au minimum :

```bash
cargo test -p aithos-core --locked
cargo test -p aithos-bundle --locked
cargo clippy -p aithos-core -p aithos-bundle --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo test --workspace --locked
cargo check -p aithos-wasm --target wasm32-unknown-unknown
```

Ajoute le runner des vecteurs indépendants, le Cucumber réel et les tests
export/import/cold verify. Ne masque aucun échec avec un mock, un ignore, un
`@wip` retiré prématurément ou une modification aval.

Si une suite workspace échoue à cause d'une piste étrangère déjà défaillante,
prouve la baseline avant/après et ne la corrige pas. Le gate Core + Bundle ciblé
peut être documenté, mais le gate CB13 global ne doit pas être déclaré entièrement
vert tant que la commande requise échoue.

## 11. Conditions STOP

STOP immédiatement, préserve tout et demande une décision si :

- la branche n'est plus `feat/obligations` ;
- HEAD avance de façon concurrente autrement que par ton propre commit contrôlé, ou
  un changement concurrent touche une cible ;
- l'index contient un fichier non attribué ;
- une dépendance exige `rust/Cargo.toml`, `rust/Cargo.lock`, une nouvelle dépendance
  ou un manifeste Cargo non attribué ;
- CB2 exige de modifier un vecteur historique ou une entrée Provider ;
- un nouveau choix produit, champ signé, encoding, kind, root, compteur, version,
  migration, reason code ou API stable n'est pas déjà couvert par CB1 ;
- spec, Gherkin, vecteur et code divergent ;
- l'oracle dépend du Rust testé ou le RED échoue pour une mauvaise raison ;
- un lot ne peut devenir vert qu'en modifiant Provider, Gateway, CLI/WASM, client,
  RemoteStore ou SDK ;
- Core aurait besoin d'I/O, d'une horloge ou d'un RNG ambiant ;
- Bundle devrait recopier une règle Core ;
- la vérification keyless exigerait clé de contenu, credential, plaintext ou
  structure self ;
- une capacité ne peut pas être rejouée à froid ;
- l'owner doit intervenir alors que le mandat et ses obligations suffisent ;
- une panne peut laisser manifest, Gamma, root, head ou état partiellement visible ;
- le travail commence à simuler CAS/HTTP Provider, Gateway ou effet connecteur ;
- le gate du lot précédent n'est pas satisfait et commité selon la définition
  RED/GREEN de l'ouverture.

Un test difficile, une tâche longue ou un contexte compacté ne sont pas des raisons
de STOP. Continue à partir des commits et preuves déjà produits. Si un vrai STOP
survient, fournis un handoff précis : dernier commit sain, fichiers modifiés/indexés,
RED/GREEN, décision manquante et commande exacte de reprise.

## 12. Sens exact de « finalisation bout en bout »

Cette mission doit fermer de bout en bout le protocole local Core + Bundle :

```text
opération owner/grantee
→ verdict Core pur
→ transaction Bundle
→ artefacts publics/opaques
→ export vers store local vierge
→ cold verify keyless
→ réouverture fonctionnelle séparée
→ forks/merges/résolutions
```

CB13 est le gate « prêt pour reprise Provider ». Il n'est pas le gate produit
global.

N'implémente pas dans cette mission :

- HTTP ou backend durable Provider ;
- CAS serveur, witness/head intégré ou restart réseau ;
- runtime Gateway, MCP/OAuth, tool-host ou custody de credentials ;
- appel réel d'un connecteur ;
- E2E Provider/Gateway ;
- adaptation CLI/WASM/client/SDK.

Après CB13, produis dans ton rapport final :

1. les hashes CB2→CB13 et la matrice de couverture finale ;
2. les preuves du gate local Core + Bundle ;
3. la liste exacte des API, reason codes, artefacts et faits CAS remis au Provider ;
4. les écarts restant Provider/Gateway/surfaces ;
5. un prompt de reprise proposé pour le vrai E2E suivant :

```text
Bundle grantee
→ HTTP Provider
→ CAS durable
→ arrêt/restart
→ téléchargement dans un nouveau store
→ cold verify
→ lectures owner/grantee
```

Ne lance pas ce chantier aval sans attribution séparée de ses fichiers. Ne déclare
pas le protocole produit global terminé avant ces vrais E2E.

## 13. Résultat attendu

Le résultat n'est pas seulement « des tests verts ». Il faut :

- des contrats CB1 conservés ;
- des wires figés par oracle indépendant et non-régression ;
- un Core pur qui rend le verdict complet unique ;
- un Bundle transactionnel qui produit les mêmes faits append-time et cold-time ;
- les opérations owner/grantee réellement durables sur toutes les zones autorisées ;
- des éditions déléguées, contraintes, Gamma, vault, publication et concurrence
  vérifiables à froid ;
- zéro occurrence `@wip` dans `features/*.feature`, sauf exclusions nominatives
  validées avant CB2 ;
- une suite CB13 entièrement verte ; sinon statut `BLOCKED` documenté, CB13 non
  fermé et aucune reprise Provider autorisée ;
- des commits étroits, auditables et sans fichier étranger ;
- un handoff exact pour le vrai E2E Provider/Gateway suivant.

Commence maintenant par la rebaseline read-only puis CB2. Ne modifie ni n'amende le
commit CB1 ; les futurs retraits d'`@wip` appartiennent exclusivement aux commits
CB3–CB13 qui les rendent réellement verts.
````

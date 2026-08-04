# Audit d'implémentation — `c-headers.feature`

## 1. Métadonnées

| Champ | Valeur |
|---|---|
| Feature auditée | `features/c-headers.feature` (`@c-headers`) |
| Ronde | 1 — audit initial, mode orchestré |
| Date | 2026-08-03 |
| Révision observée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` (`a2087f2`) |
| Base `main` enregistrée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` |
| Branche | `codex/audit-c-headers-r2` |
| Run orchestré | `2026-08-03-r1` (`features/.agents/orchestrator/runs/2026-08-03-r1/`) |
| État du worktree | propre pour tout le périmètre audité ; `features/.agents/c-headers/STATE.md` modifié par l'orchestrateur (gel de révision) et `features/.agents/orchestrator/runs/2026-08-03-r1/` non suivi au moment de l'audit |
| Périmètre | la vérité sémantique des huit scénarios existants ; quatre blocs `Rule` |
| Préfixe de findings | `CHDR-*` (`docs/audits/features/README.md:20`) |
| Domaine | `features/.agents/c-headers/DOMAIN.md` |
| Rapport de run | `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md` |
| Étalon Pass B | branche `origin/codex/audit-c-headers` (`af32734`), audit manuel de juillet 2026 |

### Mise à jour du 2026-08-04 — clôture du lot A

| Champ | Valeur |
|---|---|
| Ronde | 2 — revue de correction, lot A (**sémantique de test**, aucun code de production touché) |
| Date | 2026-08-04 |
| Révision candidate revue | `5905bec` (correction `03283b0`) ; branche `codex/fix-c-headers-lot-a`, tête `10de842` au moment de cette mise à jour |
| Run orchestré | `2026-08-04-r6` (`features/.agents/orchestrator/runs/2026-08-04-r6/ledger.jsonl`) |
| Revue indépendante | `features/.agents/c-headers/auditor/runs/2026-08-04-review-lot-a.md` — **gelée**, isolation matérielle du Pass A, douze mutants nommés et exécutés |
| Rapport de correction | `features/.agents/c-headers/corrector/runs/2026-08-04-correction-lot-a.md` |
| Findings clos | `CHDR-001`, `CHDR-002`, `CHDR-009`, `CHDR-013`, `CHDR-014`, `CHDR-019`, `CHDR-021`, `CHDR-025` → **`VERIFIED`** |
| Findings ouverts par cette revue | `CHDR-037`, `CHDR-038`, `CHDR-039`, `CHDR-040` (**P2**), `CHDR-042` — §6ter. `CHDR-041` est **réservé et non ouvert** |
| Correction de la présente note | le mutant énoncé par le bloc `CHDR-019` de §6 était **faux sur le code**. Il est retiré et remplacé (§6, `CHDR-019`) |

**Ce que cette clôture n'établit pas.** Huit verdicts propres ne sont pas une
preuve que la feature est correcte : ce sont huit défauts nommés **dans ses
preuves** qui sont fermés. Restent ouverts, entre autres, `CHDR-016` (re-routé
hors de `c-headers`, ni clos ni retiré), `CHDR-028` (embargo levé le 2026-08-04), les
findings P3 de §6 et §6bis, les cinq findings de §6ter, et les suites
enregistrées par l'orchestrateur dans `QUEUE.yaml`. Le détail est en §3.

**Ce que la revue a mesuré et qui ne va pas dans le sens du lot.** Un mutant
sur douze — la réduction de `derive_key` à l'identité — laisse le gate de
feature **entièrement vert** après correction (`ev-ec9412a7`) ; il est retenu
comme résidu nommé sous `CHDR-021`. Une assertion ajoutée par le lot n'a été
tuée par aucun des douze mutants et est étiquetée **non prouvée**, non comptée
(§6, `CHDR-019`).

### Avertissement — collision d'identifiants avec l'étalon de juillet

La branche publique `origin/codex/audit-c-headers` porte un
`docs/audits/features/c-headers.md` daté du 2026-07-30 qui attribue déjà les
identifiants `CHDR-001` … `CHDR-016` à **d'autres énoncés** que ceux de la
présente note. Les deux documents sont publics et revendiquent la même famille
d'identifiants stables réservée par `docs/audits/features/README.md:20`.

Cette note n'a pas autorité pour renuméroter l'un ou l'autre jeu. Tant que la
collision n'est pas tranchée par le propriétaire humain, **tout renvoi à un
`CHDR-*` doit nommer sa source** : « `CHDR-nnn` (ronde 1, run `2026-08-03-r1`) »
ou « `CHDR-nnn` (étalon de juillet, `af32734`) ». La §8 donne la table de
correspondance complète entre les deux jeux. La collision n'est pas une
condition de blocage au sens de `PROCESS.md` § *Blocking conditions* — cette
liste est close — mais elle est signalée au propriétaire par la même voie que
la barrière de divulgation.

## 2. Provenance de la méthode

Mode orchestré. L'isolation du Pass A est **matérielle** : chaque unité de revue
a tourné contre un extrait `git archive` de `a2087f2` **sans répertoire `.git`**
(`ledger.jsonl`, entrées `role: extract`, `sha256:
589fcc39c257f05a7a639845c79c5d7f9886e585841a3c2f459f8503b02bba0c`). Aucun agent
de Pass A n'a exécuté de gate : l'orchestrateur seul exécute les gates, écrit
les transcripts et enregistre un `evidence_id`.

| Unité | `Rule` | Scénarios | Contamination Pass A |
|---|---|---|---|
| RU-1 | A line seals the node key to exactly one recipient | 1 à 4 | aucune |
| RU-2 | The owner line is mandatory (I3) | 5 | aucune |
| RU-3 | Grant is one appended line, touching nobody | 6 | aucune |
| RU-4 | Rotation cuts the revoked and re-links the parent | 7 et 8 | aucune |

Les quatre unités ont été gelées dans
`features/.agents/orchestrator/runs/2026-08-03-r1/pass-a/frozen.json` avant
l'ouverture du Pass B. Un panel de réfutation adverse a ensuite instruit les
**seize** findings P1/P2 gelés, à trois réfuteurs indépendants chacun, chaque
réfuteur ne recevant que l'énoncé du finding (`pass-a/refutation.json`,
`ledger.jsonl`, entrées `role: refutation`). Le Pass B, la passe d'état partagé
et la réconciliation ont été conduits en dernier, par l'auditeur intégrateur,
sur le dépôt complet.

**Divulgation de contamination.** L'auditeur intégrateur lit l'histoire, l'étalon
de juillet et les verdicts gelés : c'est la définition du Pass B. Aucune de ces
entrées n'a été visible d'une unité de Pass A. La ligne `counts` de
`frozen.json` est **erronée** (elle annonce P2=14 / P3=9) ; le décompte réel du
gel est P1=1, P2=15, P3=8, total 24. L'erreur est de comptage, pas de contenu :
la liste `findings` du même fichier est correcte et fait foi.

## 3. Verdict

La cryptographie de header est fidèle à `spec/03-headers.md`. **Aucun finding de
cette note ne demande une correction de `aithos-core`.** Ce qui est faible, ce
n'est pas le produit : c'est la preuve.

Trois constats structurent la ronde.

1. **Six des huit scénarios énoncent un fait structurel et prouvent une
   conséquence comportementale.** « le révoqué *n'a pas de ligne* » est prouvé
   comme « le révoqué n'ouvre pas » ; « *toute autre* ligne intacte » est prouvé
   sur un header qui n'a qu'une autre ligne ; « liée à son nœud *et à sa
   version* » ne fait varier que le nœud ; « le wrap *restaure la dérivation* »
   n'exécute aucune dérivation.
2. **Le scénario 8 ne prouve pas son énoncé.** Il scelle une constante sous une
   constante et la rouvre deux pas plus loin sous la même constante, sans header,
   sans rotation et sans dérivation. Le Pass A l'avait classé `PROXY` ; la
   réconciliation le requalifie en `SEMANTIC_FALSE_POSITIVE` (§5, §6
   `CHDR-021`).
3. **La liaison de version du sceau de ligne n'a aucun défenseur
   comportemental dans tout le dépôt.** Elle n'est tenue que par des épinglages
   d'octets contre des vecteurs, dont l'un n'a pas de générateur dans le dépôt
   (§6 `CHDR-025`). C'est le seul finding nouveau de sévérité P2 de la ronde, et
   il vient de la passe d'état partagé.

Deux findings appellent une décision humaine avant toute correction :
`CHDR-007` (P1) et `CHDR-012` (P2). Les deux ont été retenus par la barrière de
divulgation pendant le cycle, puis publiés en entier sur décision du
propriétaire le 2026-08-03 (§6, préambule ; trace complète en §15). Ils restent
`DECISION_REQUIRED` et ne sont assignés à aucun correcteur.

### Mise à jour du 2026-08-04 — ce que le lot A ferme, ce qui reste ouvert

Les trois constats ci-dessus décrivent `a2087f2` et sont conservés tels quels :
c'est ce que la correction répare. Après la clôture du lot A, sur la révision
candidate `5905bec` :

1. **Le constat 1 est fermé sur les quatre exemples qu'il cite.** « le révoqué
   n'a pas de ligne » est désormais lu dans `key_versions["2"].lines`
   (`CHDR-019`, `ev-39f02b30`) ; « toute autre ligne intacte » s'exerce sur deux
   destinataires préexistants avec cardinal et préfixe assertés (`CHDR-013`,
   `CHDR-014`, `ev-a1f966ca`, `ev-1b889900`, `ev-b3ccaaf3`) ; « liée à son nœud
   *et à sa version* » fait varier la version (`CHDR-001`, `ev-9ba93af7`) ; le
   scénario 8 dérive, tourne et enveloppe réellement (`CHDR-021`,
   `ev-c78772c4`, `ev-16a836a9`).
2. **Le constat 2 tombe.** `CHDR-021`, qui portait le verdict
   `SEMANTIC_FALSE_POSITIVE` du scénario 8, est `VERIFIED`. Le scénario 8 repasse
   `PARTIAL` — pas `PROVEN` : `CHDR-020` et `CHDR-026` restent ouverts sur lui
   (§5).
3. **Le constat 3 est fermé, et sa seconde moitié l'était avant le lot.**
   `c1_fail_closed` a désormais un contrôle positif dans son propre corps
   (`CHDR-025`, `ev-ad4db6a1`), et `vectors/gen-c.py` — le générateur
   indépendant manquant — existe depuis `5be3047`, base du lot B. Le crédit de
   cette moitié appartient au lot B, pas au lot A. Le générateur n'est en
   revanche exécuté par **aucun gate** : `CHDR-038`, §6ter.

**Le paragraphe `DECISION_REQUIRED` ci-dessus est périmé et est corrigé ici.**
Le propriétaire a tranché la sémantique le 2026-08-03
(`features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md`,
lecture A sur les deux), le lot B a été implémenté puis accepté, et `CHDR-007`
comme `CHDR-012` sont `VERIFIED` depuis le 2026-08-04. Ils ne sont plus
`DECISION_REQUIRED` et ne sont plus non assignés. La condition de blocage 1 est
fermée par cette décision ; les blocs de §6 le disent déjà, la présente §3 ne
le disait pas.

**Ce qui reste ouvert après le lot A**, énuméré plutôt que résumé :

- `CHDR-016` — **re-routé** hors de `c-headers` le 2026-08-04 par
  l'orchestrateur, vers `g-revocation` et `d-bundle`, enregistré dans
  `features/.agents/orchestrator/QUEUE.yaml` sous `chdr-016-grant-path`. **Ni
  clos ni retiré** : son marqueur Gherkin survit et nomme son nouveau
  propriétaire.
- `CHDR-028` — **publié en entier le 2026-08-04** sur décision du propriétaire ; l'embargo est levé (§6bis).
- `CHDR-007` et `CHDR-012` sont clos, mais laissent huit findings résiduels
  distincts : `CHDR-029` à `CHDR-036` (§6bis).
- Les P3 de §6 non touchés par le lot : `CHDR-004`, `-005`, `-006`, `-010`,
  `-011`, `-015`, `-017`, `-018`, `-020`, `-024`, `-026`, `-027`.
- Les cinq findings ouverts par la revue du lot A : `CHDR-037`, `CHDR-038`,
  `CHDR-039`, `CHDR-040` (P2, contre le process lui-même), `CHDR-042` (§6ter).
- Les suites enregistrées par l'orchestrateur dans
  `features/.agents/orchestrator/QUEUE.yaml`, qui ne sont pas des findings de
  cette note et ne sont pas repris ici.

**Quatre clôtures en sous-produit, constatées et non prises.** Le lot A remplit,
sans que ce soit son mandat, le critère de clôture énoncé de quatre findings qui
ne lui étaient pas assignés :

| Finding | Critère énoncé | Ce que le lot A a fait |
|---|---|---|
| `CHDR-017` | « une assertion structurelle (préfixe intact, cardinal +1 — voir `CHDR-013`) » | faite (`cucumber.rs:12552-12570`) |
| `CHDR-018` | « deux fonctions distinctes, ou un paramètre Gherkin lié » | `new_grantee_opens` (`:12482`) scindé de `grantee_opens` |
| `CHDR-024` | « invoquer `check_rotation(2)` dans le `Then` existant du scénario 7 » | fait (`:12603`) |
| `CHDR-027` | « un contrôle positif interne dans chacun des scénarios 3 et 4 » | fait (`:12506-12528`) |

**Aucun des quatre n'est marqué `VERIFIED` par cette note**, et leurs marqueurs
Gherkin restent en place. Le motif est un principe, pas une prudence : un
critère de clôture rempli n'est pas un verdict. Ces quatre findings n'étaient
dans le périmètre d'aucune revue indépendante, aucun mutant n'a été conçu contre
eux, et l'auditeur qui les a écrits ne peut pas les clore lui-même
(`PROCESS.md:307`, `features/AGENTS.md` § *Role boundaries*). Leur marqueur dit
désormais **exactement cela** — critère rempli en sous-produit, clôture non
prononcée — plutôt que de continuer à décrire un trou refermé, qui serait
précisément le défaut que `CHDR-037` nomme. Ils sont remis à l'orchestrateur
comme candidats à un lot de revue court.

### Compteurs exacts

Cités par `evidence_id`, jamais recopiés d'un document.

```
ev-50caa5d6 — 1 feature / 4 rules / 8 scenarios (8 passed) / 28 steps (28 passed)
```

## 4. Preuves reproduites

Le rôle auditeur n'exécute aucun gate en mode orchestré
(`PROCESS.md` § *Orchestrated gate execution*, amendement AM). La propriété du
gate ne bouge pas : seule son exécution bouge.

| `evidence_id` | Commande | Rev | Exit | Compteurs |
|---|---|---|---|---|
| `ev-50caa5d6` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers` | `a2087f2` | 0 | 1 feature / 4 rules / 8 scénarios (8 passés) / 28 steps (28 passés) |
| `ev-d6840262` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @a-identity` | `a2087f2` | 0 | 1 feature / 8 rules / 30 scénarios (30 passés) / 93 steps (93 passés) — gate de préchauffage, hors périmètre |

Transcripts : `features/.agents/orchestrator/runs/2026-08-03-r1/evidence/`.

Les compteurs de `ev-50caa5d6` correspondent exactement au fichier de contrat
(4 `Rule`, 8 `Scenario`, 28 pas) : c'est la preuve de sélection et d'exécution.

**Le code de sortie est ici probant, et il ne l'était pas en juillet.**
`cucumber.rs:19736-19745` appelle désormais `fail_on_skipped()` puis
`filter_run_and_exit`. C'est le correctif `BDER-011`, accepté le 2026-07-30. Sur
la branche étalon de juillet, `main()` appelait `filter_run`, qui rend son writer
et ne quitte jamais : sous `harness = false` le binaire sortait `0` avec des
scénarios en échec. **Aucun chiffre de gate provenant de la branche étalon n'est
cité dans cette note, et aucun ne peut l'être.**

Aucune exécution autre que les deux ci-dessus n'est revendiquée. Toute
affirmation de comportement dans cette note repose sur la lecture du code
courant à `a2087f2`, jamais sur une exécution non journalisée.

## 5. Matrice des scénarios

| # | Scénario | Pass A | Réconcilié | Chemin de production | Ce que l'assertion compare réellement |
|---|---|---|---|---|---|
| 1 | Owner and grantee each open their line | `PROVEN` | `PROVEN` | `Header::build` → `build_at` → `build_lines` → `seal_line` ; `Header::open` → `open_line` ×2 | deux clés recouvrées indépendamment, chacune `assert_eq!` contre `DK` ; le filtre `kid` prouve que chaque destinataire a ouvert sa propre ligne |
| 2 | A non-recipient opens nothing | `PROVEN` | `PROVEN` | `Header::build` ; `Header::open` ×2 sous `xsk(0x99)` | `!opened.is_empty()` puis `all(is_err)` ; le nombre de tentatives n'est jamais lié au nombre de lignes |
| 3 | A corrupted line fails closed | `PARTIAL` | `PARTIAL` | `Header::build` ; bascule d'un caractère hex sur `lines[0].c` ; `Header::open` | `opened.last().is_err()` — sans contrôle positif interne ; la scène n'établit jamais que la ligne s'ouvrait avant la corruption |
| 4 | A line is bound to its node and version | `PARTIAL` | `PARTIAL` | `Header::build` ×2 ; greffe de ligne ; `Header::open` sous l'AAD d'un autre nœud | `opened.last().is_err()` ; seule la composante `node` de `line_aad` varie, `key_version` reste 1 des deux côtés |
| 5 | A header without an owner line is invalid | `PARTIAL` | `PARTIAL` | `Header::build` → `build_at` → `check_owner_line` → `Err(MissingOwnerLine)` | le `When` panique sur `Ok` ; le `Then` assère que l'erreur *stringifiée* contient `"I3"` ; un seul des quatre portails I3 du code est exercé côté fail-closed |
| 6 | Granting a new reader leaves every other line untouched | `PARTIAL` | `PARTIAL` | `Header::append_line` → `seal_line` ; `Header::open` | clé recouvrée `== DK` ; identité d'octets `PartialEq` de la ligne owner contre un instantané pré-append — sur un header dont l'ensemble « toute autre ligne » a le cardinal 1 |
| 7 | The revoked gets no line in the new version | `PARTIAL` | `PARTIAL` | `Header::build` → `Header::rotate` → `build_lines` ; `Header::open` ×3 | survivant et owner recouvrent `DK2` en v2 (fort) ; le rejet du révoqué est produit par le filtre `kid`, jamais par le sceau ; `key_versions["2"].lines` n'est lu par aucune assertion |
| 8 | An up-link wrap restores derivation for the parent holder | `PROXY` | **`SEMANTIC_FALSE_POSITIVE`** | `Wrap::seal` → `wrap_seal` → `derive_key(CTX_WRAP_KEY, …)` ; `Wrap::open` | un aller-retour AEAD symétrique sous la constante même qui a servi à sceller, dans le même scénario, sans header, sans rotation et sans dérivation |

Totaux réconciliés : **2 `PROVEN`, 5 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE`**.

### Mise à jour du 2026-08-04 — la matrice après le lot A

Cette matrice décrit `a2087f2` et n'est pas réécrite : elle est l'état contre
lequel la correction se mesure. Un seul **statut** bouge, et il bouge parce que
le finding qui le portait est clos :

| # | Statut à `a2087f2` | Statut après le lot A | Motif |
|---|---|---|---|
| 8 | `SEMANTIC_FALSE_POSITIVE` | **`PARTIAL`** | `CHDR-021`, qui portait ce verdict (§6), est `VERIFIED` ; `CHDR-020` et `CHDR-026` restent ouverts sur ce scénario |
| 3, 4, 5, 6, 7 | `PARTIAL` | `PARTIAL` | des findings ouverts subsistent sur chacun : `CHDR-027` ; `CHDR-027` ; `CHDR-010`, `CHDR-011` ; `CHDR-015`, `CHDR-016`, `CHDR-017`, `CHDR-018` ; `CHDR-024` |
| 1, 2 | `PROVEN` | `PROVEN` | inchangés, non touchés par le lot |

Totaux après le lot A : **2 `PROVEN`, 6 `PARTIAL`, 0
`SEMANTIC_FALSE_POSITIVE`**. Aucun scénario ne passe à `PROVEN` : fermer le
défaut de preuve nommé par un finding ne prouve pas les autres phrases du même
scénario, et cette note ne requalifie pas un scénario sur la foi d'une clôture
partielle.

**Un seul mouvement de la colonne « ce que l'assertion compare réellement »
mérite d'être cité, parce qu'il contredit la ligne 7 ci-dessus.** Le rejet du
révoqué n'est plus produit par le filtre `kid` : le `Then` du scénario 7 lit
maintenant `key_versions["2"].lines` et essaie le secret du révoqué contre
chaque `kid` réellement routable en v2 (`cucumber.rs:12590-12617`). Le
contre-exemple mesuré est `ev-39f02b30`.

### Pourquoi le scénario 8 n'est pas `PROXY`

`PROXY` désigne un scénario qui « consomme un verdict partagé sans exécuter son
propre cas ». Le scénario 8 exécute bien son propre cas : `post_uplink_wrap`
(`cucumber.rs:8164-8174`) construit un `Wrap` réel et `parent_recovers_via_wrap`
(`:12396-12404`) l'ouvre réellement. Ce qu'il ne fait pas, c'est prouver ce que
sa phrase énonce — la définition exacte de `SEMANTIC_FALSE_POSITIVE`. Les trois
composantes de la phrase sont absentes du code exécuté :

- « a parent holder » — `PARENT_KEY` (`cucumber.rs:265`) n'est la sortie d'aucun
  `node_key`, n'est ouverte d'aucune ligne de header, et n'est la clé d'aucun
  nœud du scénario ;
- « the new node key » — `DK2` (`:264`) n'est produite par aucune rotation ici ;
  `w.header` reste `None` pendant tout le scénario ;
- « restores derivation » — aucun `node_key`, aucun `folder_label`, aucun lien
  parent→enfant n'est calculé.

Ce qui est établi est exactement `wrap_open(wrap_seal(k, dk)) == dk`.
`Wrap::open` (`header.rs:351-353`) recalcule son AAD depuis ses **propres**
champs `self.node` et `self.key_version` : l'assertion ne peut donc pas détecter
un wrap posté sous le mauvais nœud ni sous la mauvaise version. Et `via`
(`header.rs:344`) n'entre pas dans `wrap_aad` (`seal.rs:41-43`) : il est stocké,
lu par personne.

## 6. Findings

Statut du panel noté `n/3 réfutations`. Un finding survit sur une majorité de
non-réfutations. Un finding réfuté par une majorité **revient à l'auditeur comme
question ouverte** (`PROCESS.md` § *Adversarial refutation*) : la ligne
« réconciliation » de chaque bloc dit ce que le Pass B en a fait, sur preuve de
code courant.

### Barrière de divulgation — levée le 2026-08-03

`aithos-core` est public et cette branche y sera poussée. Le Pass A avait marqué
quatre findings `disclosure: embargo` : `CHDR-003`, `CHDR-007`, `CHDR-008` et
`CHDR-012`. **Aucun ne l'est plus.**

- `CHDR-003` et `CHDR-008` sont **retirés** par la réconciliation (§7) ; leur
  embargo tombe avec eux.
- `CHDR-007` et `CHDR-012` sont **publiés en entier sur décision du propriétaire
  humain**, enregistrée le 2026-08-03 :

> « Publier les deux en entier. `CHDR-007` est déjà public en substance sur
> `codex/audit-c-headers` ; `CHDR-012` est publié malgré l'absence de correctif,
> au motif que le correcteur doit pouvoir citer ce qu'il répare. »
>
> — Mathieu Colla, propriétaire du protocole, 2026-08-03. Run de reprise
> `2026-08-03-r2`.

La condition de blocage 9 est donc **résolue**. La barrière a réellement joué
pendant ce cycle et la trace en est conservée en §15 : ce n'est pas une
formalité rétroactive.

**Ce que la décision ne tranche pas.** La levée de l'embargo est une décision de
publication, non de sémantique. `CHDR-007` et `CHDR-012` restent tous deux
`DECISION_REQUIRED` : la question normative qu'ils posent — un invariant que la
spécification énonce à la voix passive lie-t-il une surface vérifiante, ou
décrit-il seulement une propriété d'objet ? — n'est pas tranchée et ne doit pas
l'être par un correcteur. **Ces deux findings ne sont assignés à aucun
correcteur** (§11 lot 0, §12, §15).

---

### `CHDR-007` — **`VERIFIED`** le 2026-08-04, P1 — 1/3 réfutations (survit)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-i3-authority.md`), sur la révision candidate
> `9dc5889`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel : c'est ce que la correction répare. Preuve différentielle —
> RED `ev-47ec8aac` (baseline `5be3047` : `Bundle::verify` renvoie `Ok(())` sur
> une édition dont le header épinglé a perdu sa ligne owner, et sur une autre
> dont la ligne étiquetée `"owner"` déclare la clé d'un étranger) → GREEN
> `ev-b925a0cf` (candidat, 3/3). Gate de feature `ev-2b8ccdc0` (1/4/8/28), gate
> workspace `ev-8bfeccca` (836 scénarios / 3577 pas). La lecture retenue est
> celle de la décision du 2026-08-03 : `Header::validate` est appelé sur chaque
> header épinglé par l'édition, dans `Bundle::verify` **et** dans
> `publication::cold_verify`. Trois findings résiduels distincts en sont issus :
> `CHDR-028`, `CHDR-034`, `CHDR-036`.

**La moitié « édition » de I3 n'est imposée par aucun vérificateur d'édition.**
**Scénario 5 / RU-2 — finding de surface publique.**

`spec/00-overview.md:33-34` et `spec/03-headers.md:36-37` énoncent I3 en **deux**
propositions :

> **I3 — Owner line.** Every header MUST contain a line for the owner. A header
> without one is invalid, **and so is the edition carrying it.**
>
> **I3:** every `key_versions[*].lines` MUST include the owner line. **An edition
> whose any header violates this is invalid.**

La première moitié est imposée en quatre points de `aithos-core`
(`check_owner_line` sur `build`/`build_at` à `header.rs:133`, sur `rotate` à
`:201`, la branche owner de `check_rotation` à `:298-303`, et `validate` à
`:308-315`). **La seconde ne l'est nulle part.**

`Bundle::verify` (`bundle.rs:1654-1769`), le vérificateur d'édition hors ligne,
contrôle le document DID (`:1656`), la chaîne et les signatures de manifestes,
la hauteur et `prev_hash`, les fusions et résolutions de fork, les digests
SHA-256 des fichiers épinglés, l'absence de fichier non épinglé, les liens gamma
et `gamma_head`, et les racines Merkle d'état et gamma. **Il n'appelle jamais
`Header::validate`** ; recherche exhaustive sur son corps entier
(`bundle.rs:1654-1769`) : aucune occurrence de `Header` ni de `validate`.

Le seul contact de la vérification avec les headers est indirect :
`header_hash_at` (`state.rs:57-62`, « `BLAKE3(JCS(header.json))` if the node was
ever granted, else zeros ») et `vault_build` (`state.rs:240-248`) les
désérialisent en `serde_json::Value` **opaque** pour en calculer le digest JCS.
Un header dépourvu de ligne owner y produit un hash parfaitement valide, qui est
plié dans la racine Merkle d'état, épinglé au manifeste, et signé.

**Portée élargie par un réfuteur, vérifiée :** `publication::cold_verify`
(`publication.rs:836-939`) est un **second** vérificateur d'édition, tout aussi
muet sur I3.

**Conséquence rattachée par le même réfuteur.**
`spec/10-threat-model.md:19` inscrit « Owner un-lockable-out » à la table des
menaces et n'y cite qu'une seule contre-mesure : « owner line mandatory in every
header (I3) ». Producteur possible identifié : un délégué signant une édition
ordinaire — branche `m.version == CORE_DRAFT2_VERSION` de `verify`,
`bundle.rs:1664` — qui publie une rotation dont la nouvelle `key_version` omet la
ligne owner et ré-encrypte sous une DK' aléatoire. Un header sans ligne owner ne
peut pas être *créé* par les constructeurs de `aithos-core`, mais un header
arrivant par une autre route — `header.json` édité à la main, bundle importé,
écrivain futur, aller-retour `serde` — serait haché dans l'arbre d'état, épinglé,
signé dans un manifeste, et passerait `verify` sans opposition.

### Les deux lectures concurrentes — exposées, non arbitrées

| | Lecture A — I3 est un invariant d'édition | Lecture B — I3 est un invariant de construction |
|---|---|---|
| Fondement | `spec/00-overview.md:33-34` et `spec/03-headers.md:36-37` disent « and so is the edition carrying it » / « An edition whose any header violates this is invalid » : la phrase vise l'édition, donc le vérificateur d'édition | la spécification énonce I3 à la **voix passive** et ne l'impose explicitement à aucun vérificateur ; aucun vecteur de `spec/09-cli-and-conformance.md` §9.2 ne gate le cas |
| Conséquence | `Bundle::verify` et `publication::cold_verify` doivent valider chaque header de l'édition | l'architecture actuelle — fail-closed à l'écriture (`header.rs:133`, `:201`) plus validation au parse (`header.rs:308-315`, appelée en `bundle.rs:630`, `:637`, `log.rs:425`, `session.rs:363`, `aithos-cli/src/cmd/header_open.rs:28`) — est **conforme** |
| Coût | parser chaque header à chaque `verify` | la phrase de spec doit être resserrée pour dire ce que le code fait |
| Porté par | l'auditeur et deux réfuteurs sur trois | le réfuteur dissident |

Une troisième lecture est ouverte et n'a été portée par personne : déplacer la
validation sur les seuls chemins de lecture. **Aucun correcteur ne peut choisir
implicitement.** `DECISION_REQUIRED`, propriétaire attendu : le propriétaire du
protocole.

**Réconciliation.** Maintenu à P1. Le Pass B confirme la lecture du Pass A sur le
code courant et ne l'élargit pas ; il y absorbe `CHDR-008` (§7), dont l'énoncé
— la couverture inégale de `Header::validate` sur les chemins de lecture — est un
sous-ensemble strict de la même question normative.

**Rapport à l'étalon de juillet.** L'étalon publie déjà ce constat en clair sur
la branche publique `codex/audit-c-headers` (`af32734`), sous
`CHDR-015 — I3 is not enforced at the edition level — DECISION_REQUIRED, P2`.
Cette ronde le retrouve indépendamment, le relève à P1, et ajoute deux éléments
que juillet n'avait pas : le second vérificateur `publication::cold_verify`, et
le rattachement explicite à `spec/10-threat-model.md:19`.

**Référence de spec.** `spec/00-overview.md:33-34` ; `spec/03-headers.md:36-37` ;
`spec/10-threat-model.md:19` ; `spec/09-cli-and-conformance.md` §9.2.

**Critère de clôture.** Une décision enregistrée du propriétaire du protocole,
**antérieure** à toute correction, désignant laquelle des trois lectures fait
foi ; puis, selon cette décision, soit l'appel de `Header::validate` sur chaque
header de l'édition dans `Bundle::verify` **et** `publication::cold_verify`, soit
le resserrement de la phrase de spec, soit la validation sur les chemins de
lecture — et, dans les trois cas, un test qui échoue sur la baseline auditée pour
la raison nommée.

---

### `CHDR-012` — **`VERIFIED`** le 2026-08-04, P2 — **0/3 réfutations**

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-i3-authority.md`), sur la révision candidate
> `9dc5889`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel. Preuve différentielle — RED `ev-15f8f483` (baseline
> `5be3047` : `Recipient::owner` produit `kid: "owner-kex"` là où §03.1 exige
> `z6LSeYCJg2G3i6zEiYd2bvnacfR8EnQoUUv3315nBbJL85sS` ; `validate()` **accepte**
> `owner_label_foreign_key` et **rejette** `unlabelled_owner_line`, soit l'écart
> reproduit dans les deux directions) → GREEN `ev-9f82e070` (candidat, 6/6),
> `ev-b925a0cf` (3/3), `ev-f4579eab` (g2_rotation 4/4), `ev-b19b0db3`
> (c1_header_seal 3/3, inchangé), `ev-6608a56c` (pins de vecteurs).
> `check_owner_line` compare désormais `r.pubkey` à `owner_kex` **et** le `kid`
> dérivé ; `validate` et `check_rotation` reçoivent le kid owner attendu ;
> aucun contrôle I3 de production ne lit plus `to`. Fait établi par exécution
> (`ev-15f8f483`) : la ligne construite et la ligne attendue ne diffèrent que
> par `kid` — `epk`, `n` et `c` identiques —, donc la variante A ne redérive
> aucun chiffré et n'invalide aucun vecteur épinglé à l'octet. Cinq findings
> résiduels distincts en sont issus : `CHDR-029`, `CHDR-030`, `CHDR-031`,
> `CHDR-032`, `CHDR-035`.

**I3 est vérifié sur un champ que la spécification déclare non autorisant, et
non sur celui qu'elle déclare définitoire.**
**Scénario 5 / RU-2 — finding de surface publique.**

C'est le finding le plus solide du cycle : **aucun des trois réfuteurs ne l'a
entamé**, deux l'ont renforcé depuis des angles que le Pass A n'avait pas pris,
et il est **absent de l'étalon manuel de juillet**.

#### Le constat

Les quatre points de contrôle I3 de `aithos-core` comparent tous un **label** :

| Point de contrôle | Ligne | Test |
|---|---|---|
| `check_owner_line`, appelé par `build_at` (`header.rs:133`) et `rotate` (`:201`) | `header.rs:71-77` | `recipients.iter().any(\|r\| r.to == OWNER_LABEL)` |
| branche owner de `check_rotation` | `header.rs:298-303` | `new.lines.iter().any(\|l\| l.to == OWNER_LABEL)` |
| `validate` (parse-time) | `header.rs:308-315` | `kv.lines.iter().any(\|l\| l.to == OWNER_LABEL)` |

Or `spec/03-headers.md:33-35` déclare précisément ce champ non autorisant :

> `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`);
> it is **a routing hint only — the seal is what grants**.

Le commentaire de `header.rs:31-32` reprend la phrase mot pour mot. Les trois
champs de `Recipient` (`header.rs:16-18`) sont `pub`, donc le constructeur
`Recipient::owner` (`header.rs:22-28`) — le seul endroit où `to` et le `kid`
`"owner-kex"` sont posés ensemble — n'est en rien contraignant : n'importe quel
appelant peut construire un `Recipient { to: "owner", kid: …, pubkey: … }` à la
main.

#### Angle spec — l'écart est à la lettre, pas seulement à l'intention

`spec/01-identity-and-keys.md:23` définit :

> **owner_kex** is **the recipient key** of the owner's line in every header (I3).

La spécification définit donc la ligne owner **par sa clé destinataire**, pas par
son label. Le code vérifie l'inverse. Et la comparaison correcte est
**disponible et non faite** : à `build_at` et à `rotate`, `check_owner_line`
reçoit des `Recipient` qui portent un `pubkey: XPublicKey` (`header.rs:18`), et
`OwnerKeys::owner_kex_pub()` (`keys.rs:51-53`) rend exactement la valeur à
laquelle le comparer.

#### Angle modèle de menace — la garde correspondante n'existe pas

`spec/05-delegation.md:85-91` autorise explicitement un révocateur « owner **or
ancestor** » à re-sceller les lignes des survivants, **ligne owner comprise** :

> it rotates the node key and republishes the header omitting the revoked
> child's line but keeping every other line — including lines it did not create
> (those it re-seals under the new DK using its own access).

La règle de garde qui devrait borner ce pouvoir — un vérificateur rejette une
rotation de header dont le signataire n'est pas un émetteur autorisé — **n'est
pas implémentée**, ce que le dépôt constate déjà lui-même :
`docs/proposals/header-rotation-authority.md:37-48` relève que `check_rotation`
« ne vérifie que deux choses : aucun destinataire clandestin, la ligne owner est
présente. **Aucun contrôle d'autorité** », statut *Proposé — non adopté*.
Conséquence directe : un rotateur émettant `{ to: "owner", kid: <son propre kid,
déjà présent en v1> }` passe `check_rotation` — la garde anti-clandestin ne voit
rien puisque le `kid` existait, et la garde I3 ne voit rien puisque le label
dit `"owner"`.

#### Angle code — la seule liaison réelle est constructive, jamais vérificative

Le seul endroit du dépôt qui relie une ligne owner à la clé publiée dans le
document DID est `Bundle::owner_kex_recipient` (`grants.rs:171-174`) :

```rust
pub(crate) fn owner_kex_recipient(&self) -> Result<Recipient> {
    let doc = self.did_doc()?;
    let bytes = wire::multibase_to_x25519_pub(&doc.keys.kex)?;
    Ok(Recipient::owner(bytes.into()))
}
```

Il est **côté écrivain**. Aucune contrepartie vérificative n'existe, et il n'en
existe structurellement pas : `validate(&self)` et `check_rotation(&self, v)`
prennent le seul `Header` en paramètre et n'ont **aucun accès** au document DID.

#### Surface publique concernée

`aithos-cli/src/cmd/header_seal.rs:30-56` accepte des destinataires au format
libre `label:kid:x25519_pub_hex`, construit
`Recipient { to: label, kid, pubkey }` sans aucune contrainte sur `label`, et
les passe tels quels à `Header::build` (`:56`). En regard,
`aithos-cli/src/cmd/header_open.rs:27-32` valide puis ouvre — et **accepte** donc
le fichier ainsi produit, puisque `validate` ne regarde que le label.

#### Atténuations, relevées et pesées

1. `header_seal.rs:1-2` se déclare « DEV surface over test keys » : ce n'est pas
   une surface de production.
2. Une ligne owner falsifiée serait remplacée par la vraie à la rotation
   suivante : `revoke.rs:180`, `structure.rs:259` et `vault.rs:375` remplacent
   toute ligne dont `line.to == "owner"` par `owner_kex_recipient()`, c'est-à-dire
   par la clé du document DID. Le mensonge est donc auto-réparant à la première
   rotation — mais rien ne garantit qu'une rotation survienne, et ces trois sites
   **font confiance au même label** pour décider quelle ligne remplacer.

Ces atténuations réduisent l'exploitabilité ; elles ne touchent pas le constat,
qui est un écart entre la lettre de la spécification et le champ testé.

### Les deux lectures concurrentes — exposées, non arbitrées

| | Lecture A — la ligne owner est définie par sa clé | Lecture B — la ligne owner est définie par son label |
|---|---|---|
| Fondement | `spec/01-identity-and-keys.md:23` : `owner_kex` **est** « the recipient key of the owner's line » ; `spec/03-headers.md:33-35` retire toute autorité à `to` | I3 est un invariant **structurel** de l'objet header ; `to` est le champ que la structure expose, et lier I3 au document DID ferait sortir `Header` de `aithos-core`, qui ne connaît pas les DID |
| Conséquence | `check_owner_line` doit comparer `r.pubkey` à `owner_kex_pub()` ; `validate` et `check_rotation` doivent recevoir la clé attendue en paramètre | le code courant est correct, et c'est la couche appelante (`grants.rs:171-174` et ses homologues) qui porte la liaison |
| Coût | changement de signature de trois fonctions publiques de `aithos-core` ; `validate` cesse d'être `(&self)` | la spécification doit dire que `to` est *aussi* le champ définitoire de I3, ce qui contredit `spec/03-headers.md:33-35` |
| Porté par | l'auditeur et les trois réfuteurs | personne ne l'a défendue ; elle est reconstruite ici pour que la décision soit posée équitablement |

**Aucun correcteur ne peut choisir implicitement.** `DECISION_REQUIRED`,
propriétaire attendu : le propriétaire du protocole.

**Réconciliation.** Maintenu à P2, intact. C'est le seul finding de la ronde à
sortir du panel sans une seule réfutation. Le Pass B n'y a rien retiré et a
vérifié indépendamment chacune des références ci-dessus sur `a2087f2`.

**Référence de spec.** `spec/01-identity-and-keys.md:23` ;
`spec/03-headers.md:33-35`, `:36-37`, `:93-96` ; `spec/05-delegation.md:85-91` ;
`docs/proposals/header-rotation-authority.md:37-48`.

**Critère de clôture.** Une décision enregistrée du propriétaire du protocole
désignant le champ définitoire de I3 ; puis, si la lecture A est retenue,
comparer `r.pubkey` à `owner_kex_pub()` dans `check_owner_line` et donner à
`validate` / `check_rotation` la clé owner attendue — avec un test RED qui
construit un header portant `{ to: "owner", pubkey: <clé arbitraire> }`, passe
sur la baseline auditée, et échoue après correction.

---

### `CHDR-025` — **`VERIFIED`** le 2026-08-04, P2 — nouveau, issu de la passe d'état partagé

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel : c'est ce que la correction répare.
>
> **Moitié 1 — le contrôle positif.** `c1_header_seal.rs:92-107` ouvre le tuple
> non modifié sous l'AAD nominale et assère `dk_hex` **avant** les quatre
> négatifs. Mutant : `M11`, `PURPOSE_HEADER_LINE` changé — une mutation
> *symétrique* de l'AAD, exactement la classe que le finding nomme.
> `c1_fail_closed` ne re-scelle rien : il déchiffre le chiffré **figé** du
> vecteur, donc rien n'ouvre sous `M11`. Preuve — **`ev-ad4db6a1`**
> (`-- --exact c1_fail_closed`) : RED à `c1_header_seal.rs:103`, *positive
> control: the untouched tuple MUST open under the nominal AAD:
> SealRejected("line does not open")*. **`ev-34e698d8`** (non porté) : 1 passé /
> 2 échoués. Le portage `--exact` **est** la revendication et non une commodité :
> la vacuité est par corps, donc la mesure discriminante doit exclure
> `c1_owner_and_grantee_lines`, qui la masquerait. Sous le corps
> pré-correction — quatre négatifs et rien d'autre — chacun est satisfait
> vacuement sous `M11` et le test est vert.
>
> **Moitié 2 — la revendication de génération indépendante.** Le critère était
> *produire ou retirer*. Elle est produite : `vectors/gen-c.py` existe, sa
> docstring énonce la règle de seconde implémentation (blake3 + PyNaCl + HKDF
> RFC 5869 manuel, jamais la référence Rust), et `check_c1()` (`:167-207`)
> reconstruit la ligne owner, la ligne grantee et le wrap C2 de
> `c1-header-seal.json` octet à octet sans réécrire le fichier gelé. **Le crédit
> n'appartient pas au lot A** : le fichier est arrivé par `5be3047`, base du lot
> B (revue §9). Le verdict n'en dépend pas — le critère est rempli sur le
> candidat — mais l'attribution est rectifiée ici parce que la note l'aurait
> sinon portée au lot A. Le générateur n'est exécuté par aucun gate :
> **`CHDR-038`**, §6ter, non imputable au lot A.
>
> **Renforcement inattendu, mesuré, plus fort que le critère de clôture.**
> **`ev-2e427d6e`** — sous `M3`, le mutant `kek` que la présente note énonçait à
> tort sous `CHDR-019` — montre que le nouveau contrôle positif tombe **aussi**.
> Parce qu'il déchiffre un chiffré figé et non un chiffré qu'il vient de
> produire, ce contrôle n'est pas seulement une base différentielle pour quatre
> négatifs : c'est un **épinglage asymétrique de tout le chemin de sceau** —
> dérivation de KEK, construction d'AAD et AEAD ensemble. Toute mutation de
> `kek`, de `aad` ou du chiffre le casse, symétrique ou non. C'est strictement
> plus que ce que le critère demandait, et la revue ne l'a appris que d'un
> mutant visé ailleurs.

**La liaison `key_version` du sceau de ligne n'a aucun défenseur comportemental
dans le dépôt.**
**Scénario 4 et test de conformance C1.**

`c1_fail_closed` (`rust/crates/aithos-core/tests/c1_header_seal.rs:82-107`) est
le seul test négatif explicite de liaison de version du dépôt :

```rust
let other_ver = line_aad(&v.subject_did, &v.node, v.key_version + 1);
assert!(open_line(&sk, &epk, &c, &n, &other_ver).is_err());
```

Il n'a **aucun contrôle positif dans son propre corps**. Le triplet
`(sk, epk, c, n)` provient du vecteur ; que ce triplet s'ouvre sous l'AAD
nominale n'est établi que dans une *autre* fonction de test,
`c1_owner_and_grantee_lines` (`:76-80`). Toute mutation de `line_aad` change
l'AAD des deux côtés à la fois : l'assertion continue de passer, mais pour une
raison entièrement différente de celle que son commentaire nomme. Les trois
assertions sœurs de `c1_fail_closed` (`:92`, `:97`, `:101`) ont le même défaut.

Il ne reste alors, dans tout le dépôt, que des **épinglages d'octets** pour
défendre la composante `key_version` de `line_aad` (`seal.rs:29`, `:35-37`) :

- `c1_header_seal.rs:66-70` — `hex::encode(&c) == line.c_hex` contre
  `vectors/c1-header-seal.json` ;
- `g3_move.rs:149-152` — `hex::encode(line_aad(…)) == v.line_aad_hex` contre
  `vectors/g3-move.json`.

Et le premier de ces deux épinglages repose sur un vecteur dont **le générateur
n'existe pas dans le dépôt** : `c1_header_seal.rs:2-3` déclare « generated
independently (Python PyNaCl + manual RFC 5869 HKDF) », mais `vectors/` ne
contient aucun `gen-c1*` alors qu'il contient vingt-huit autres générateurs
`gen-*.py`. C'est exactement l'obligation `TARGETED` déjà enregistrée par la
revue d'impact acceptée de `b-derivation` ronde 2
(`features/.agents/orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`)
et reportée dans `features/.agents/c-headers/STATE.md`. Cette note en établit la
conséquence : ce n'est pas une simple classe de preuve à requalifier, c'est le
dernier verrou d'un invariant de sécurité de §3.8.

Côté Gherkin, le scénario 4 dit « bound to its node **and version** » et ne fait
varier que le nœud (`CHDR-001`). Les deux findings se composent : la moitié
« version » n'est ni exercée par le contrat, ni défendue comportementalement
ailleurs.

**Portée de la revendication.** Ce finding est établi par lecture du code
courant et par l'absence, vérifiée, de tout autre site. Il ne repose sur aucune
exécution : aucune expérience de mutation n'a été conduite par ce rôle, et la
mesure de rayon d'explosion publiée par l'étalon de juillet est écartée (§8).

**Référence de spec.** `spec/03-headers.md:32`, `:124-128` ;
`spec/00-overview.md:57-60`.

**Critère de clôture.** Donner à `c1_fail_closed` un contrôle positif dans son
propre corps — asserter d'abord que le tuple non modifié ouvre sur `dk_hex` —
de sorte que chacune des quatre assertions négatives soit un différentiel contre
une base connue bonne ; **et** produire ou retirer la revendication de
génération indépendante de `vectors/c1-header-seal.json`.

---

### `CHDR-001` — **`VERIFIED`** le 2026-08-04, P2 — 1/3 réfutations (survit)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel.
>
> **Correction.** `replay_line_other_node` (`cucumber.rs:8215-8248`) enregistre
> trois tentatives : une ouverture de contrôle sur le header d'origine ; la
> moitié « nœud » (greffe dans un header `NODE_OTHER`, ouverte en v1) ; et la
> moitié « version » (`:8239-8248`) — la même ligne insérée comme
> `key_versions["2"]` du header **d'origine**, mêmes `subject_did`, `node` et
> `kid`, ouverte en v2. Dans cette troisième tentative, `key_version` est la
> seule entrée variable de `line_aad`.
>
> **Mutant.** `M1` — suppression du séparateur `0x00` et des octets de
> `key_version` dans `aad()` (`seal.rs:28-29`). C'est le RED attendu du lot 5
> de §11.
>
> **Preuve — `ev-9ba93af7`** : 7 passés / 1 échoué, scénario 4 seul, à
> *attempt 2 after the mutation must be rejected, got Ok([119; 32])*.
>
> **Pourquoi cette preuve discrimine.** `0x77` est `DK` (`cucumber.rs:262`) :
> sous `M1` le rejeu en v2 **réussit**, donc la liaison de version était bien la
> seule chose qui l'arrêtait. L'indice de tentative est le discriminant —
> **la tentative 1, la greffe inter-nœuds, échoue toujours** sous `M1`, le nœud
> variant encore. Un seul transcript porte les deux bras : l'indice qui tombe
> prouve que la nouvelle assertion mord, l'indice qui passe prouve que
> l'ancienne ne mordait pas. La revue relève par ailleurs que rien d'autre dans
> le scénario ne voit `M1` : la moitié « version » n'est pas ornementale
> (revue §7, point 11).

**Le scénario « A line is bound to its node and version » n'exerce que la
liaison au nœud.**
**Scénario 4.**

`replay_line_other_node` (`cucumber.rs:8114-8122`) ne fait varier que le nœud :
`NODE_A` `/e/circle` → `NODE_OTHER` `/e/self`. Les deux `Header::build`
retombent sur la version 1 (`header.rs:114-116`) et l'ouverture se fait en
version 1 (`:8120`, `open_into(1, …)`). Les composantes `subject_did` et
`key_version` de `line_aad(subject_did, node, key_version)` (`seal.rs:35-37`,
`aad` `:21-31`), recalculées par `Header::open` (`header.rs:228`), sont
identiques des deux côtés. Le scénario prouve strictement moins que sa phrase.

**Correction imposée par le panel** (deux réfuteurs sur trois l'exigent) : ne pas
relayer ce constat en « la liaison à la version n'est testée nulle part ». Elle
l'est hors Gherkin — `c1_header_seal.rs:105-107` et l'épinglage d'octets
`g3_move.rs:149-152`. Le défaut est de **portée du scénario**, pas de couverture
du corpus.

**Réconciliation.** Maintenu, avec la correction du panel intégrée à l'énoncé
ci-dessus. Le Pass B ajoute une qualification que le panel n'avait pas :
`c1_header_seal.rs:105-107` est un défenseur **vacant** (`CHDR-025`). La
formulation exacte retenue est donc : *la liaison à la version n'est exercée par
aucun scénario, et hors Gherkin elle n'est défendue que par des épinglages
d'octets, jamais par un différentiel comportemental.* Le réfuteur dissident tient
que la conséquence de sécurité est nulle ; `CHDR-025` montre pourquoi elle ne
l'est pas.

**Référence de spec.** `spec/03-headers.md:32`, `:124-128`.

**Critère de clôture.** Une seconde tentative enregistrée qui greffe la même
ligne v1 dans une version 2 du **même** nœud (ou l'ouvre en version 2), les deux
tentatives devant être `Err` ; ou une scission du scénario en deux, pour que le
Gherkin cesse de revendiquer deux variations.

---

### `CHDR-009` — **`VERIFIED`** le 2026-08-04, P2 — 2/3 réfutations (réfuté, reformulé)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel.
>
> **Correction.** Trois tests dans `g2_rotation.rs`, un par portail non exercé,
> chacun assérant la **variante typée** et non une chaîne :
> `check_rotation_refuses_a_new_version_without_the_owner_line` (`:156`, qui
> consomme enfin le champ de vecteur et construit une v2 dont les kids sont un
> sous-ensemble strict de v1, de sorte que la branche « smuggling » est
> prouvablement muette) ; `rotate_refuses_a_survivor_set_without_the_owner`
> (`:189`, plus `!header.key_versions.contains_key("2")` — aucun effet partiel) ;
> `validate_refuses_a_key_version_without_the_owner_line` (`:224`, avec son
> propre contrôle positif à `:237`).
>
> **Mutant.** `M10` — les trois portails supprimés d'un coup : `check_owner_line`
> retiré de `rotate` (`header.rs:234`), la garde owner retirée de
> `check_rotation` (`:357-362`), `validate` réduit à `Ok(())`.
>
> **Preuves. `ev-dce43f1c`** (`--test g2_rotation`) : 4 passés / 3 échoués, RED
> sur exactement les trois nouveaux noms. **`ev-4ed2d6f3`** (gate de feature sous
> le même mutant) : **entièrement vert, 8 scénarios / 28 pas**.
>
> **Ce que le couple montre.** `ev-dce43f1c` est le bras positif et sa précision
> compte : 4 passés signifie que `survivor_set_is_old_minus_revoked`,
> `a_smuggled_recipient_is_rejected`, `a_clean_rotation_is_accepted` et
> `uplink_wrap_bytes_match_python` sont intacts — `M10` a atteint exactement les
> trois portails visés et rien de plus. `ev-4ed2d6f3` est le bras de l'ancienne
> assertion et il n'est pas contestable : les trois portails I3 supprimés, la
> feature entière est verte. C'est le finding énoncé comme expérience et non
> comme argument — le Gherkin n'a jamais observé ces portails échouer.
>
> **Ce que la clôture ne prouve pas, et qui n'est pas revendiqué.** Le nouveau
> test de `check_rotation` appelle le portail directement sur un header
> construit à la main. Chez ses deux appelants réels (`revoke.rs:214`,
> `vault.rs:404`) la branche owner reste **dominée** par `check_owner_line` dans
> `rotate`, comme cette note l'établit plus bas. Le portail est prouvé ; son
> atteignabilité depuis la production ne l'est pas, et la revue n'a pas tracé
> ces deux fichiers — hors des limites de pilote de cette feature. Le reste
> appartient à `CHDR-024` et `CHDR-036`.

**Trois des quatre portails I3 du code n'ont aucun versant fail-closed testé, et
un cas spécifié par vecteur n'est pas implémenté.**
**Scénario 5.**

*Énoncé gelé au Pass A* : quatre portails I3 existent — `check_owner_line` sur
`build`/`build_at` (`header.rs:133`), sur `rotate` (`:201`), la branche owner de
`check_rotation` (`:298-303`), et `validate` (`:308-315`) ; le scénario n'exerce
que le premier.

*Réfutation majoritaire, vérifiée sur le code courant et acceptée* : les portails
2, 3 et 4 **sont exécutés**. `rotate` l'est par `cucumber.rs:8148` et `:15249` ;
la branche owner de `check_rotation` l'est par `g2_rotation.rs:92`
(`a_clean_rotation_is_accepted`) ;
`validate` l'est à chaque lecture de zone, vault, session et log
(`bundle.rs:630`, `:637`, `session.rs:363`, `log.rs:425`,
`aithos-cli/src/cmd/header_open.rs:28` — cinq sites). Ce qui manque est leur
**versant fail-closed**.

**Formulation exacte retenue après réconciliation :**

> Aucun test du dépôt n'assère `Error::MissingOwnerLine` ailleurs qu'au portail
> `build`. De plus `vectors/g2-rotation.json:17` déclare
> `"missing_owner_must_fail": "MissingOwnerLine"` — un cas normatif — que la
> struct `G2` de `rust/crates/aithos-core/tests/g2_rotation.rs:9-16` ne
> désérialise même pas : le champ n'a aucun consommateur dans le dépôt. Le cas
> est spécifié par le vecteur et n'est implémenté nulle part.

Vérifié sur le code courant : `vectors/g2-rotation.json:17` porte bien la clé ;
la struct `G2` déclare `old_kids`, `revoked_kid`, `expected_survivor_kids`,
`smuggled_new_kid`, `uplink` — et rien d'autre. Le champ frère
`smuggled_must_fail` (`:16`) est, lui, honoré par
`a_smuggled_recipient_is_rejected` (`g2_rotation.rs:68-80`) : l'asymétrie est
interne au même vecteur.

Second acquis du panel, retenu : la branche owner de `check_rotation` est
**dominée** par `check_owner_line` dans `rotate` chez ses deux appelants
(`revoke.rs:198-199`, `vault.rs:392-400`), `build_lines` recopiant `r.to`
verbatim (`header.rs:94-100`) — branche morte pour ces chemins, et même variante
d'erreur.

**Réconciliation.** Réfuté dans sa formulation gelée, **maintenu à P2 dans la
formulation ci-dessus**. La trouvaille du vecteur non désérialisé appartient au
panel et est portée au crédit de la ronde. Le finding change d'énoncé, pas de
sévérité : un cas normatif sans consommateur est plus grave qu'un portail non
exercé.

**Référence de spec.** `spec/03-headers.md:36-37` ; `spec/00-overview.md:33-35` ;
`spec/09-cli-and-conformance.md` §9.2.

**Critère de clôture.** Faire consommer `missing_owner_must_fail` par
`g2_rotation.rs` — désérialiser le champ, construire une v2 sans ligne owner et
asserter `Err(Error::MissingOwnerLine(_))` sur `check_rotation` — et ajouter une
assertion typée équivalente sur `rotate` et sur `validate`.

---

### `CHDR-013` — **`VERIFIED`** le 2026-08-04, P2 — 1/3 réfutations (survit)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel.
>
> **L'affirmation d'absence, avec sa recherche.** Le finding disait qu'aucune
> assertion de cardinal de lignes de header n'existait dans le dépôt. La revue
> l'a vérifiée plutôt que reprise : `grep -rn "lines\.len()" --include="*.rs" .`
> sur **l'extrait entier**, toutes couches, non `rust/**` seul → `log.rs:143`
> (lignes de journal d'audit), `i1_concurrency.rs:123`, `cucumber.rs:1267`
> (manifeste `n`), `cucumber.rs:17569`, `cucumber.rs:19231`, `gamma.rs:919-922`.
> La seule assertion de cardinal de lignes de header de l'extrait est la
> nouvelle, `cucumber.rs:12552`. L'affirmation d'absence était vraie ; elle est
> close par exactement un site.
>
> **Spec, citée jusqu'à sa fin.** `spec/03-headers.md:66-72` :
>
> > ```
> > 1. Open the node's current DK (own line).
> > 2. Seal DK to the recipient's X25519 key → one new line.
> > 3. Append it to key_versions[current].lines. Publish the edition.
> > ```
> > Content untouched, other lines untouched, DK unchanged. This is the frequent, cheap
> > operation. (If old versions still hold un-re-encrypted content the recipient should
> > read, the issuer adds a line to those versions too — §3.5.)
>
> « Append » et « one new line » donnent le cardinal et la position ; « other
> lines untouched » donne l'égalité de préfixe. La parenthèse est citée parce
> qu'elle est la seule clause autorisant un grant à toucher une autre
> `key_version` — et seulement en y **ajoutant**, jamais en la réécrivant : elle
> n'affaiblit donc ni l'un ni l'autre.
>
> **Correction.** `owner_line_untouched` (`cucumber.rs:12540-12570`) assère
> `lines.len() == saved.len() + 1` (*a grant appends EXACTLY one line (§03.3)*)
> et `&lines[..saved.len()] == &saved[..]` (*every pre-existing line stays
> byte-identical AND keeps its position*), les deux contre `w.saved_lines`, le
> vecteur pré-append entier instantané à `:7626`.
>
> **Mutants.** `M4` (double `push` — le cardinal) et `M5` (`insert(0, …)` — la
> position ; RED attendu du lot 4 de §11).
>
> **Preuves. `ev-a1f966ca`** — 7/1, scénario 6, *a grant appends EXACTLY one
> line (§03.3)*. **`ev-1b889900`** — 7/1, scénario 6, *every pre-existing line
> stays byte-identical AND keeps its position*.
>
> **Ce que les deux montrent.** Les deux mutants sont invisibles à l'assertion
> pré-correction : `find(|l| l.to == "owner")` renvoie la ligne owner intacte
> quoi qu'on pousse après elle (`M4`) et est aveugle à l'ordre (`M5`). Dans les
> deux runs les sept autres scénarios passent, et à l'intérieur du scénario 6
> `new_grantee_opens` passe : l'échec est isolé au cardinal et à la position,
> pas au sceau.
>
> **Un nit relevé et non imputé.** `assert_eq!(header.key_versions.len(), 1, "a
> grant creates no key version")` n'équivaut à son message que parce que ce
> fixture part d'une seule version ; contre un header multi-versions
> (`spec/03-headers.md:115-123`) l'assertion serait fausse là où le message
> resterait juste. Un instantané du compte pré-append serait exact. Coût de
> le laisser : nul — aucun scénario de cette feature ne grant sur un header
> multi-versions.

**« Grant is one appended line » n'est asserté nulle part : ni cardinal, ni
position.**
**Scénario 6.**

`owner_line_untouched` (`cucumber.rs:12353-12361`) fait
`.find(|l| l.to == "owner")` puis `assert_eq!` contre l'instantané `saved_line`
(posé en `:7571` comme `lines[0].clone()`). Il ne lit ni `kv.lines.len()`, ni
l'index, ni l'ensemble des lignes. `append_line` fait un `push`
(`header.rs:180-186`) : la ligne owner d'origine reste à l'index 0 et `find` la
renvoie quoi que le mutant ait poussé ensuite. `Header::validate`
(`header.rs:308-315`) n'exige qu'**au moins une** ligne owner. Aucune assertion
sur un cardinal de lignes de header n'existe dans le dépôt. Le trou couvre aussi
une ligne surnuméraire vers une clé tierce, pas seulement un doublon.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident objecte que le cardinal
figure au titre de la `Rule` (`features/c-headers.feature:38`) et non dans une
phrase du scénario, et que le grief appellerait donc un scénario supplémentaire
sous la `Rule` plutôt qu'une correction du scénario existant. La réconciliation
tranche : `PROCESS.md` § *Evidence hierarchy* point 1 fait du **scénario et de
ses exigences normatives citées** le contrat ; le titre de `Rule` est une
exigence normative citée, et `spec/03-headers.md:46-58` énonce la même chose.
Le grief vise le bon artefact. La remédiation, elle, peut légitimement prendre
la forme d'un scénario supplémentaire : le critère de clôture laisse les deux
ouvertes.

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Une assertion de cardinal et de préfixe :
`lines.len() == saved.len() + 1` et égalité du préfixe contre l'instantané du
vecteur complet — ce qui épingle aussi l'ordre. Recouvre `CHDR-014`.

---

### `CHDR-014` — **`VERIFIED`** le 2026-08-04, P2 — 2/3 réfutations (réfuté, reformulé, maintenu)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`. L'énoncé ci-dessous décrit le code **audité** (`a2087f2`) et est
> conservé tel quel.
>
> **Correction.** Un nouveau `Given`, `sealed_header_owner_and_reader`
> (`cucumber.rs:7613-7628`), scellant à `[owner_rec(), grantee_rec("g1", 0x21)]`
> et instantaniant le vecteur `lines` entier. `append_grantee_line`
> (`:8269-8278`) appende `g2`, distinct du `g1` que le `Given` porte, et
> `new_grantee_opens` (`:12482-12489`) est scindé de `grantee_opens`, qui
> servait jusque-là deux phrases. Le `Then` assère en outre `saved.len() >= 2` —
> *'every other line' needs at least two pre-existing recipients to have a
> non-degenerate referent* — de sorte qu'une régression du fixture vers un seul
> destinataire se détecte elle-même. La phrase Gherkin de
> `c-headers.feature:68` a été rebindée en conséquence : c'est **la seule ligne
> du fichier de contrat** que le lot A touche.
>
> **Mutant.** `M6` — bascule d'un caractère hex de `c` sur chaque ligne
> préexistante dont `to != OWNER_LABEL`, choisi précisément parce qu'il est un
> **no-op sur l'ancien fixture** : l'ancien header à un seul destinataire n'en
> avait aucune. C'est le mutant que le `Given` dégénéré était structurellement
> incapable de voir, ce qui est exactement la revendication du finding.
>
> **Preuve — `ev-b3ccaaf3`** : 7/1, scénario 6, à *every pre-existing line stays
> byte-identical AND keeps its position*.
>
> **Ce que la preuve montre.** Le RED tombe sur l'**égalité de préfixe du
> vecteur entier**, pas sur le contrôle de la ligne owner — la distinction que
> `CHDR-014` nomme, et la raison pour laquelle `M6` exclut délibérément la ligne
> owner. Même l'assertion pré-correction `find(|l| l.to == "owner")` aurait été
> satisfaite sous `M6` ; sous le *fixture* pré-correction, le mutant n'édite
> rien du tout. L'ancien scénario était vert par construction, pas par chance.
>
> **Une limite que le correcteur a déclarée et que la revue n'a pas levée.**
> Aucun mutant ne prouve la thèse de dégénérescence elle-même — qu'un `Given` à
> un destinataire ne *peut pas* exprimer « toute autre ligne ». Cette thèse
> porte sur le fixture, et sa preuve est la garde `saved.len() >= 2`, qui est
> une assertion sur la précondition du test et non un différentiel.

**« Toute autre ligne intacte » est exercé sur un header dont « toute autre
ligne » a le cardinal 1.**
**Scénario 6.**

Le `Given` du scénario 6 est `sealed_header_owner_only` (`cucumber.rs:7569-7573`)
qui scelle à `&[owner_rec()]` — un seul destinataire. `key_versions["1"].lines`
tient donc exactement une entrée avant l'append. « Every other line untouched »
dégénère en « la seule autre ligne est intacte », et le scénario ne peut pas
distinguer un `push` `O(1)` d'un rebuild-and-reseal `O(n)` des destinataires
restants : avec `n = 1` il n'y a ni reste à perturber, ni ordre à permuter.

Le mutant qui re-scelle les lignes non-owner à l'append compile et passe :
`KeyVersion.lines` est `pub` (`header.rs:42-45`) et l'invariant n'est porté que
par un commentaire (`header.rs:157-158`).

*Clause du Pass A que le panel a réfutée, et qui est retirée de l'énoncé* : « le
fixture multi-destinataires n'est câblé qu'à la `Rule` de rotation ». C'est
faux. Il y en a **deux** : `sealed_header_owner_grantee` (`cucumber.rs:7553`,
owner + g1, câblé à `c-headers.feature:17` et `:22`, donc à la `Rule` « A line
seals… ») et `sealed_header_three` (`:7579`, câblé à `:49`). La remédiation en
est simplifiée, pas le finding.

**Réconciliation.** Réfuté 2/3, **maintenu à P2 après retrait de la clause
fausse**. Les deux réfutations attaquent des propositions annexes — le câblage
des fixtures, et la couverture fonctionnelle ailleurs (`cb10_structure_vault.rs`
`:307`/`:334`/`:355`, `cb9_delegated_content.rs:439`). Aucune n'atteint la
proposition centrale, qui est vérifiée sur le code courant : *ce* scénario
exerce l'invariant sur un ensemble de cardinal 1. La couverture ailleurs n'est
jamais byte-identique et plafonne à **une** ligne de grantee préexistante, ce
que le troisième réfuteur concède. `PROCESS.md` § *Evidence hierarchy* point 1
donne le contrat au scénario : une couverture ailleurs ne fait pas qu'un
scénario prouve sa phrase.

Le Pass B renforce ce maintien d'un fait différentiel : l'étalon manuel de
juillet porte le même grief (`CHDR-010` de juillet, P2) et l'a fait **survivre à
sa propre passe adverse** sur un code byte-identique (§8).

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Pointer le `Given` sur un header à au moins deux
destinataires préexistants — `sealed_header_owner_grantee` existe déjà —,
instantanier le vecteur `lines` entier, appeler un grantee **différent**, et
asserter égalité de préfixe et cardinal. Recouvre `CHDR-013`.

---

### `CHDR-016` — `OPEN`, P2 — 1/3 réfutations (survit)

**Le chemin de grant de production n'implémente ni l'étape 1 ni l'étape 3 de
§3.3, et aucun pas de cette `Rule` ne le touche.**
**RU-3 — finding de surface publique.**

`Bundle::grant` (`grants.rs:739`) → `deliver_entry` (`:754`, corps `:308-341`) →
`add_line_on` (`:276-305`) :

- (a) calcule la DK par dérivation pure `node_key(&zone_dk, &node)`
  (`grants.rs:321`) sans jamais ouvrir le header existant — l'étape 1 de §3.3
  (« Open the node's current DK (own line) ») n'est pas exécutée ;
- (b) appelle `header.append_line(&did, KV, dk, …)` (`grants.rs:289`) avec
  `KV: u64 = 1` (`bundle.rs:25`) au lieu de `latest_version()`.

`rotate_folder` (`revoke.rs:142-240`) conserve v1 (`insert` de la clé « 2 »,
`header.rs:202-210`), scelle une DK' issue de `ent.e32()` (`revoke.rs:195`) et
bumpe chaque section à `key_version = 2`. Un `Bundle::grant` ultérieur sur ce
dossier dépose donc la ligne du nouveau lecteur dans `key_versions["1"]`,
scellant la clé dérivée pré-rotation ; le grant renvoie `Ok` et publie. Côté
lecture, `agent_section_key` demande v2 (`grants.rs:1037-1044`), l'ouverture
échoue faute de ligne, le repli `agent_node_key`/`try_header` ouvre à `KV`
(`grants.rs:827-830`) et rend une clé périmée, et `open_blob_v` à la version 2
refuse (`bundle.rs:505-518`). Fail-shut : le bénéficiaire reçoit moins que
prévu, le révoqué ne gagne rien.

Les deux surfaces conformes à §3.3 — `Session::append_header_recipient`
(`session.rs:354-366` : `validate`, puis `open_latest`, puis `append_line`) et
`deliver_connector_line` (`grants.rs:454-461`, `latest_version()`) — ne sont
touchées par aucun pas de cette `Rule`.

Deux précisions du panel, retenues : `deliver_exact_section` (`grants.rs:414`)
passe par `owner_current_section_key` et livre donc la bonne DK' — seule
l'étiquette de version de la ligne y est fausse ; et pour `move_folder` le
nouveau header est bâti par `build_at(new_v)` sans clé « 1 », donc le grant
échoue bruyamment — l'affirmation vaut pour la rotation de révocation.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident classe le finding hors
périmètre au motif que `bundle.rs:25` porte le commentaire « single key version
**until step G** (revocation rotates) », donc dette assumée de
`g-revocation`/`d-bundle`. Le Pass B écarte cette réfutation **sur preuve de code
courant** : l'étape G a livré. `revoke.rs` existe, `rotate_folder`
(`revoke.rs:142-240`) tourne, `Header::build_at` existe pour le déplacement
(`header.rs:124-155`), et `grants.rs:1054-1070` lit déjà les wraps de rotation.
La condition suspensive du commentaire est échue et `KV = 1` est resté. Une
dette dont la date d'échéance est passée n'est plus une dette assumée.

Le finding reste néanmoins **à cheval sur deux périmètres** : le défaut vit dans
`aithos-bundle`, pas dans `aithos-core`. Il est consigné ici parce que
`PROCESS.md` § *Current scope* inclut explicitement « production surfaces that
bypass the exercised verdict », et il est **signalé comme impact** à
`g-revocation` et `d-bundle` (§9).

**Référence de spec.** `spec/03-headers.md:46-58` (§3.3, étapes 1 et 3) ;
`spec/03-headers.md:98-106` (§3.5, les lectures visent le verrou le plus récent).

**Critère de clôture.** Un pas de cette `Rule` qui traverse une surface de grant
de production conforme à §3.3, plus la correction de `add_line_on` pour qu'il
ouvre le header courant et append à `latest_version()`. La seconde moitié
appartient au cycle `g-revocation`/`d-bundle`.

---

### `CHDR-019` — **`VERIFIED`** le 2026-08-04, P2 — 1/3 réfutations (survit)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1 et §2), sur la révision
> candidate `5905bec`, **sur le défaut tel qu'énoncé**. Le *mutant* que ce bloc
> énonçait était faux ; voir l'erratum ci-dessous, qui est la partie la plus
> importante de cette mise à jour.
>
> **Spec, citée jusqu'à sa fin, et une phrase que la version initiale de cette
> note ne citait pas.** `spec/03-headers.md:33-35` :
>
> > `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
> > a routing hint only — the seal is what grants. Recipients try lines addressed to
> > their `kid`. No verifier decides anything from `to`.
>
> et `spec/03-headers.md:56-59`, qui décide la **forme** du correctif :
>
> > `kid` orders the attempts and nothing else: a reader that finds no matching line MAY try the remaining
> > lines, and a successful unseal — never a label — is what proves the line was its own.
> > No network, no per-read state.
>
> Ce `MAY` est la raison pour laquelle le filtre `kid` de `Header::open` n'est
> pas lui-même un défaut — la spec permet de s'arrêter à l'indice de routage —
> et la raison pour laquelle la boucle du `Then` corrigé sur `v2.lines` est la
> bonne forme : elle fait ce qu'un lecteur *peut* faire, et atteint donc le
> sceau.
>
> **Correction.** `revoked_cannot_open` (`cucumber.rs:12590-12617`) remplace une
> assertion par trois : structurelle
> (`v2.lines.iter().all(|l| l.kid != "g1")`) ; mécanique
> (`header.check_rotation(2, &owner_kid_c())`) ; de capacité
> (`header.open(DID_C, 2, &line.kid, &xsk(0x21))` pour chaque ligne routable en
> v2). Le message de la deuxième dit *survivors ⊆ previous, owner kept*, ce que
> `check_rotation` implémente réellement (`header.rs:347-356`, un test
> d'inclusion de `BTreeSet`) — et **non** ce que `spec/03-headers.md:109-111`
> exige (« the new version's lines MUST equal the previous lines minus the
> revoked »). Le correcteur n'a pas sur-revendiqué ; l'écart inclusion-vs-égalité
> reste la note hors verdict de `CHDR-024` et reste ouvert.
>
> **Mutant.** `M2` — `rotate` recopie les lignes de la version précédente en v2.
> C'est le RED attendu du **lot 3 de §11**, et c'est celui qui est juste sur le
> code.
>
> **Preuve — `ev-39f02b30`** : 7/1, scénario 7 seul, à *the revoked gets NO line
> in the new version: ["z6LStLK2kx…", "g1", "g2", "z6LStLK2kx…", "g2"]*.
>
> **Ce que la preuve montre.** La liste de kids imprimée compte cinq entrées —
> owner v1, `g1`, `g2`, puis owner v2 et `g2` — ce qui confirme que `M2` a été
> appliqué tel que nommé, ni plus lourdement. `survivor_opens` et
> `owner_opens_new` passent : `Header::open` essaie chaque ligne de `kid`
> correspondant et rend la première qui ouvre, donc la copie v1 périmée est
> ignorée. Le scénario 8, qui tourne lui aussi, passe. Et l'ancienne assertion
> est verte sous `M2` par construction : la ligne `g1` recopiée est liée à
> `line_aad(did, node, 1)` et est ouverte en version 2, donc
> `Header::open(…, 2, "g1", …)` rend toujours `Err`. Ancien vert, nouveau rouge,
> un seul scénario, une seule assertion.
>
> **Deux choses que le transcript enseigne et qui sont portées au dossier.**
>
> 1. `check_rotation(2)` ne **rattrape pas** `M2` : `g1` est présent dans la
>    version précédente, donc l'inclusion tient. C'est l'assertion structurelle,
>    et non la mécanique, qui porte ici. L'appel à `check_rotation` gagne
>    néanmoins sa ligne : il remplit, en sous-produit, le critère de clôture de
>    `CHDR-024`. La revue avait prédit le contraire et le transcript l'a
>    corrigée (revue §7, point 5).
> 2. **La boucle de capacité n'a rien tué dans aucun des douze runs.** La revue
>    n'a pas pu construire de mutant de production qu'elle tue et que rien
>    d'autre ne tue, et le dit plutôt que de la créditer. Le mutant qu'elle vise
>    manifestement — une ligne v2 portant le `kid` d'un survivant mais scellée à
>    la clé du révoqué — **n'est pas exprimable comme une édition de code de
>    production dans ce dépôt** : une `Line` ne stocke que `to`, `kid`, `epk`,
>    `n`, `c` (`header.rs:43-50`), donc `rotate` n'a aucun accès à la clé
>    publique du révoqué. La boucle reste une assurance bon marché contre un
>    état que la couche *bundle* peut produire (`CHDR-032`, `kid` dupliqué dans
>    une `key_version`, imposé nulle part) et elle est conservée. Mais elle est
>    **non prouvée**, et une assertion non prouvée doit être étiquetée, pas
>    comptée : elle n'est pas l'une des assertions qui closent ce finding.

**« Le premier grantee ne peut pas ouvrir la nouvelle version » est décidé par
l'indice de routage, jamais par le sceau.**
**Scénario 7.**

`revoked_cannot_open` (`cucumber.rs:12375-12383`) appelle
`Header::open(DID_C, 2, "g1", &xsk(0x21))`. `Header::open` filtre
`kv.lines.iter().filter(|l| l.kid == kid)` (`header.rs:233`) ; la v2 construite
par le `When` (`cucumber.rs:8148-8161`) ne porte que les kids `owner-kex` et
`g2`. La boucle est donc **vide** et le contrôle tombe en `header.rs:242-245`
sans jamais appeler `open_line` (`seal.rs:110-132`). Le secret `xsk(0x21)` est
passé et n'est jamais utilisé. Le rejet est produit par un champ que
`spec/03-headers.md:33-35` déclare non-autorisant (« `to`/`kid` are routing
hints only — the seal is what grants »), commentaire repris en
`header.rs:31-32`.

Aucune assertion du scénario ne lit `key_versions["2"].lines` : le fait
structurel que la phrase énonce n'est jamais observé, et le fait cryptographique
n'est jamais exercé. Le scénario ne prouve ni l'un ni l'autre.

Le dépôt dispose de l'idiome fort à quatre règles de là : `stranger_tries`
(`cucumber.rs:8097-8102`) essaie tous les kids avec la même clé.

#### Erratum du 2026-08-04 — le mutant que ce bloc énonçait était faux

**Cette section remplace le mutant énoncé par la version du 2026-08-03. Le
texte retiré était :**

> ~~Régression survivante construite par un réfuteur, retenue : muter `kek`
> (`seal.rs:83-89`) pour que l'IKM HKDF n'intègre plus le secret DH laisse le
> nommage intact, `survivor_opens` et `owner_opens_new` verts, et rend la ligne
> `g2` ouvrable par quiconque connaît la clé publique de g2.~~

C'est **une erreur de cette note**, pas une subtilité, pas une imprécision de
formulation. Elle a été construite par un réfuteur, **retenue par la
réconciliation**, publiée dans ce document, et lue par le correcteur du lot A à
qui son mandat imposait de lire §6. Le correcteur l'a proposée comme mutant, l'a
vue revenir verte (`ev-a87b91f1` sans le correctif, `ev-41261f7c` avec, les deux
8/8), en a diagnostiqué la cause dans le code et l'a signalée
(`corrector/runs/2026-08-04-correction-lot-a.md`). La revue indépendante l'a
ensuite transcrite **littéralement** et l'a fait exécuter, et le transcript
tranche sans argument.

**Ce que le parcours de cette erreur dit du dispositif.** Trois réfuteurs
adverses ont instruit `CHDR-019` sans la relever — l'un d'eux l'a écrite. La
réconciliation Pass B l'a retenue. Elle a été publiée. Ce qui l'a arrêtée n'est
aucune relecture : c'est la première **exécution** d'un mutant nommé. §13 de
cette note énonçait que le cycle qui l'a écrite n'avait conduit aucune
expérience de mutation ; c'est exactement le coût de cette limite, payé, et
chiffrable — une revendication de sécurité fausse, publiée pendant un jour dans
un dépôt public.

**Mesure. `ev-c16f1a9a`** — le gate de feature sous `M3`, c'est-à-dire
`Hkdf::<Sha256>::new(None, shared)` → `…new(None, &[0u8; 32])` à `seal.rs:84`,
le mutant de cette note transcrit littéralement : **entièrement vert, 8
scénarios / 28 pas**. Pas un scénario ne bascule, ni avant ni après la
correction.

**La raison structurelle, dans `seal.rs`.** `kek` utilise ses trois arguments
(`seal.rs:83-89`) :

```rust
fn kek(shared: &[u8; 32], epk: &XPublicKey, recipient: &XPublicKey) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, shared);
    let info = [KEK_INFO, &[0u8], epk.as_bytes(), recipient.as_bytes()].concat();
```

Le mutant retire `shared` de l'IKM et **laisse `recipient.as_bytes()` dans
l'`info`** (`:85`). Or `open_line` ne *reçoit* jamais de clé publique de
destinataire : il la **dérive du secret qu'on lui remet** (`seal.rs:117-120`) :

```rust
    let epk = XPublicKey::from(*epk);
    let recipient_pub = XPublicKey::from(recipient_secret);
    let shared = recipient_secret.diffie_hellman(&epk).to_bytes();
    let cipher = XChaCha20Poly1305::new((&kek(&shared, &epk, &recipient_pub)).into());
```

Après la mutation, la KEK reste donc une fonction de la paire de clés de celui
qui ouvre. Une ligne scellée à `g2` l'a été sous `info(epk, g2_pub)` ; un
appelant fournissant `xsk(0x21)` calcule `info(epk, g1_pub)` et dérive une KEK
différente. À travers `open_line` — la seule porte dont `Header::open` dispose
(`header.rs:271`) — la ligne reste ouvrable par exactement une partie, `g2`,
précisément comme avant la mutation.

**La réfutation empirique, plus propre que la structurelle.** Si la phrase de
cette note était vraie — *ouvrable par quiconque connaît la clé publique de
g2* — alors le scénario 2, « A non-recipient opens nothing », serait passé au
rouge sous `M3` : `stranger_tries` (`cucumber.rs:8191-8197`) essaie `xsk(0x99)`
contre chaque kid du header. `ev-c16f1a9a` le montre vert. Un étranger dérive
sa propre clé publique dans l'`info` et échoue quand même. **La revendication de
sécurité de cette note est falsifiée par une assertion antérieure au lot.**

**Ce qui est vrai, et ce que cette note y a confondu.** Au niveau de l'AEAD brut
la faiblesse est réelle : avec un IKM constant et une `info` publique, n'importe
qui peut calculer la KEK à partir de `epk` et de la clé publique du destinataire,
puis déchiffrer directement. C'est une observation cryptographique saine. Elle
n'est pas *testable* ici, parce que rien dans ce dépôt n'atteint le chiffré
autrement que par `open_line`, et que `open_line` n'est pas un oracle de
déchiffrement prenant une KEK : il prend un secret et en redérive la clé
publique. Cette note énonce une propriété **hors API** dans la grammaire d'une
propriété **dans l'API** — et c'est la forme « dans l'API » qu'un correcteur
aurait eu à faire attraper par une assertion. Elle n'existe pas.

**Le mutant qui, lui, réalise la capacité visée, et pourquoi il ne discrimine
rien.** Il faudrait retirer **à la fois** le secret DH de l'IKM **et**
`recipient.as_bytes()` de l'`info`. Celui-là ouvre bien chaque ligne à chaque
détenteur — et il est tué par `stranger_recovers_nothing`, une assertion
**préexistante** du scénario 2. Même réparé, le mutant de cette note ne
distingue donc pas la nouvelle boucle de capacité de ce que la feature avait
déjà.

**Le mutant correct est celui que §11 lot 3 énonçait déjà, et §6 se contredisait
avec §11.** Le lot 3 du plan d'implémentation dit *« injecter une ligne `g1` en
v2 → doit tomber »* — c'est-à-dire `M2`, une `rotate` recopiant les lignes de la
version précédente. §6 disait autre chose. **La contradiction est tranchée en
faveur de §11 : c'est §11 qui est juste sur le code**, et `ev-39f02b30` est la
mesure de son RED, sur le scénario 7 seul. La seconde phrase du texte retiré —
« Autre mutant survivant : une `rotate` recopiant les lignes v1 en v2 » — était
juste et est promue au rang de mutant principal dans le statut de clôture
ci-dessus.

**Où cette note situait la détection, et où elle est réellement.** La note
plaçait la détection de `M3` dans le scénario 7. **Elle ne peut pas y être** :
`ev-c16f1a9a` montre le gate entier vert. Elle existe ailleurs, et le lot A l'a
doublée sans le viser. **`ev-2e427d6e`** — `c1_header_seal` sous `M3` — donne
**1 passé / 2 échoués** : `c1_owner_and_grantee_lines`, l'épinglage d'octets
attendu, **et `c1_fail_closed`**. Ce second échec est dû au contrôle positif que
le lot A a ajouté pour `CHDR-025` : il déchiffre un chiffré **figé** du vecteur
sous l'AAD nominale, et `M3` change la KEK. Il épingle donc tout le chemin de
sceau, pas seulement quatre négatifs. Le mutant de cette note *est* rattrapé par
le dépôt — à la couche des vecteurs de conformance, et désormais **deux** fois
plutôt qu'une. C'est le seul endroit où le lot A dépasse son mandat, et il vaut
d'être dit en clair : ce n'est pas dans le scénario 7 que cette note l'avait mis,
c'est dans `c1_fail_closed`.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident soutient que le titre du
scénario reprend `spec/03-headers.md:87` (« The revoked … gets no line in the new
version ») et que §3.2 définit « ouvrir » comme router puis desceller, l'absence
de ligne routable étant donc un échec d'ouverture au sens du contrat. La
réconciliation écarte cet argument sur la lettre du pas exécuté : la phrase du
`Then` est « the first grantee **cannot open** the new version », un énoncé de
capacité, pas de structure. Prouver la structure demanderait de lire `lines` ;
prouver la capacité demanderait d'atteindre le sceau. Le scénario ne fait ni
l'un ni l'autre.

**Portée élargie, signalée** : le même motif — kid du révoqué passé à
`open_latest` — se retrouve en `cucumber.rs:5013` et
`cb10_structure_vault.rs:548-553`.

**Référence de spec.** `spec/03-headers.md:33-35`, `:80`, `:87-89`, `:93-96` ;
`spec/06-revocation.md:25-44`.

**Critère de clôture.** Une assertion structurelle dans le `Then` existant —
`assert!(header.key_versions["2"].lines.iter().all(|l| l.kid != "g1"))` — et un
appel à `header.check_rotation(2)` au même endroit (voir `CHDR-024`).

---

### `CHDR-021` — **`VERIFIED`** le 2026-08-04, P2 — 1/3 réfutations (survit) — portait le verdict du scénario 8

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`, **avec un résidu nommé conservé sous ce finding** — voir plus bas,
> le paragraphe des mutants survivants, qui est conservé et non supprimé. Ce
> finding portant le verdict `SEMANTIC_FALSE_POSITIVE` du scénario 8, ce verdict
> tombe avec lui : le scénario 8 repasse `PARTIAL` (§5).
>
> **Spec, citée jusqu'à sa fin.** `spec/03-headers.md:87-95`, étape 2bis :
>
> > Derivation up-link. If the rotated node N is derived from a parent node P that
> > the rotator holds, it also publishes an up-link wrap: seal(DK'_N) openable via
> > K_P — same primitive as a tag wrap (AAD purpose `tagwrap`, §00.3), bound to
> > subject_did ‖ N ‖ new key_version. The wrap restores the parent→child derivation
> > path broken by the fresh random DK', so holders of P (or of any ancestor of P)
> > keep reading N by derivation without needing a line of their own. If the rotator
> > holds exactly N but not P, it instead seals DK'_N individually to the current
> > holders of P (public keys read from P's header); the first manager of P that
> > later acts posts the definitive wrap.
>
> La conditionnelle finale est citée parce que le scénario corrigé prend la
> **première** branche — le rotateur détient P. C'est une lecture légitime du
> titre du scénario ; la seconde branche n'est exercée par aucun scénario de
> cette feature et n'est pas imputée à ce lot.
>
> **Correction.** `derived_node_rotated` (`cucumber.rs:7660-7693`) construit un
> dossier parent et une section enfant comme de vrais `NodePath`, dérive la clé
> enfant par `node_key(&zone_dk, &child)` depuis la clé de zone B2, bâtit le
> header de l'enfant en v1 **sous cette clé dérivée**, puis exécute une vraie
> `Header::rotate` vers `DK2` en laissant tomber `g1`. `post_uplink_wrap`
> (`:8297-8321`) prend la clé « via » de `node_key(&zone_dk, &parent)` — dérivée,
> non littérale — et lit le nœud enveloppé et la version **sur le header
> tourné** plutôt que de les recevoir comme littéraux que le `Then` lui rendra.
> `parent_recovers_via_wrap` (`:12638-12695`) assère : (a) la clé enfant était à
> une dérivation du parent et la v1 scellait *cette* clé ; (b) la rotation a
> déplacé l'enfant hors d'elle, la nouvelle clé étant obtenue **en ouvrant le
> header**, indépendamment du wrap ; (c) `wrap.node == header.node`,
> `wrap.key_version == header.latest_version()`, `wrap.via == parent` ; (d) le
> wrap rend la valeur de (b) sous une clé que le détenteur a dérivée.
> (d) comparé à la valeur obtenue indépendamment en (b) est ce qui empêche
> l'assertion d'être un aller-retour sur elle-même : deux routes calculées
> séparément, puis comparées.
>
> **Mutants.** `M7` — `Wrap::seal` scelle et stocke sous un nœud constant
> (`header.rs:417`, `let node: &str = "/e/self";` shadowant le paramètre), choisi
> délibérément **symétrique** pour que le wrap continue de faire son aller-retour
> et que l'ancienne assertion reste verte. Plus `M8` — `node_key` ignorant
> `Leaf::Section(sid)` — pour la moitié dérivation.
>
> **Preuves. `ev-c78772c4`** (`M7`) — 7/1, scénario 8, *wrap posted under the
> wrong node*. **`ev-16a836a9`** (`M8`) — 7/1, scénario 8, *the child key was
> reachable from the parent by one derivation*.
>
> **Ce que les preuves montrent.** Sous `M7` le wrap s'ouvre toujours et rend
> toujours la clé qu'il a scellée — c'est ce que « symétrique » veut dire — donc
> le `Then` pré-correction (`wrap.open(…) == DK2`) était vert. Seule la nouvelle
> assertion de liaison le voit : exactement l'écart que le finding nomme.
> `ev-16a836a9` prouve que la moitié dérivation porte au lieu d'être décorative
> — l'ancien scénario ne calculait aucun `node_key` et était vert sous `M8`.
>
> **Deux tautologies relevées et non imputées.**
> `assert_eq!(child.to_string(), header.node)` (`:12684`) compare deux valeurs
> que le `Given` a posées depuis le même `NodePath`. Et `wrap.via` n'entre pas
> dans `wrap_aad` (`seal.rs:41-43`) : il est stocké et lu par aucun chemin de
> production. Les deux ne coûtent rien et documentent l'intention.
>
> **Ce que le lot n'a pas couvert, déclaré par le correcteur et non levé par la
> revue.** Trois des quatre assertions ne sont prouvées par aucun mutant : la
> liaison de nœud l'est (`M7`), mais la liaison `via`, et la paire « coupée puis
> rétablie » (a)/(b), ne le sont pas.

**Le `Then` du wrap est un aller-retour sur lui-même et ne discrimine aucune
route.**
**Scénario 8.**

`post_uplink_wrap` (`cucumber.rs:8164-8175`) pose
`Wrap::seal(DID_C, NODE_A, &PARENT_KEY, CHILD_NODE, 2, &DK2, non(9))` dans
`w.wrap_obj` ; `parent_recovers_via_wrap` (`:12396-12404`) rouvre le **même objet
en mémoire** avec le **même littéral** `PARENT_KEY` et compare à `DK2`.
`Wrap::open` (`header.rs:351-357`) recalcule l'AAD depuis ses **propres** champs
`self.node` et `self.key_version` : l'assertion ne peut donc pas détecter un wrap
posté sous le mauvais nœud ni sous la mauvaise version.

Ce qui est établi est `wrap_open(wrap_seal(k, dk)) == dk`. Ne sont établis ni
(a) qu'un détenteur du parent atteignait l'enfant par dérivation avant la
rotation, ni (b) qu'il ne l'atteint plus après, ni (c) que la récupération passe
par le wrap plutôt que par une autre route — il n'en existe aucune autre,
`w.header` restant `None` pendant tout le scénario.

Mutants qui survivent, précisés par un réfuteur : toute mutation **symétrique**
de `aad()` (purpose `tagwrap` → autre, suppression des séparateurs `0x00`,
omission de `subject_did` / `wrapped_node` / `key_version`) et de
`derive_key(CTX_WRAP_KEY, ·)` (constante quelconque, dérivation réduite à
l'identité). Seul un mutant unilatéral meurt. Hors Gherkin, ces mutations
symétriques sont rattrapées par les épinglages d'octets `g3_move.rs:157-159`
(`wrap_aad_hex`) et `g2_rotation.rs:112-114` (`wrap.c == cipher_hex`) — ce qui
confirme que le scénario n'y contribue rien.

**Ce paragraphe est conservé mot pour mot à la clôture de ce finding.** Il
énonce une classe de mutants que la correction ne ferme pas, et c'est la
condition que la revue indépendante attache à son verdict `VERIFIED` : si le
paragraphe disparaît, le résidu perd son domicile et la revue retire son
arbitrage en faveur d'un identifiant propre (`CHDR-041`, réservé, §6ter).

**Mesuré le 2026-08-04, et pour la première fois de ce train : `ev-ec9412a7` et
`ev-cbce8aa0`, une paire.** §13 de cette note dit qu'aucune expérience de
mutation n'avait été conduite par le cycle qui l'a écrite ; ce qui précède était
donc un raisonnement sur du code lu. Ce ne l'est plus. Le mutant est `M12`,
`derive_key` (`derive.rs:17`) réduit à `return *key_material` — la
« dérivation réduite à l'identité » nommée ci-dessus.

- **`ev-ec9412a7`** — le gate de feature sous `M12` : **entièrement vert, 8
  scénarios / 28 pas**, après correction. La classe survit au scénario 8
  reconstruit exactement comme elle survivait à l'ancien.
- **`ev-cbce8aa0`** — les trois binaires de vecteurs sous le même `M12`, avec
  `--no-fail-fast` : `c1_header_seal` 2 passés / 1 échoué
  (`c2_wrap_roundtrip_and_cross_check`) ; `g2_rotation` 6 / 1
  (`uplink_wrap_bytes_match_python`) ; `g3_move` 1 / 2
  (`derivation_below_moved_node_is_stable` et
  `new_path_bindings_and_parent_wrap`). Quatre échecs — un de plus que les trois
  épinglages nommés ci-dessus, la moitié `g3_move` tombant en deux tests.

Les deux ne signifient quelque chose qu'**ensemble**, et c'est la forme exacte
du résidu : **vert là où regardent les scénarios de cette feature, rouge là où
regardent les vecteurs.** La classe est donc contenue — par les vecteurs
épinglés, et par rien dans le Gherkin. La condition de bascule que la revue
s'était fixée — *un run vert de ce côté-là promeut ceci en finding propre* — ne
s'est pas déclenchée.

**Un quasi-accident, consigné parce qu'il a failli être une erreur de la revue.**
Le premier run de cette commande — **`ev-debade53`**, écrite telle que la revue
l'avait nommée, sans `--no-fail-fast` — rapportait **un seul** échec, dans
`c1_header_seal`. Lu au premier degré, ce transcript dit que `g2_rotation` et
`g3_move` étaient verts sous `M12`, ce qui aurait fait basculer l'arbitrage et
ouvert un finding fantôme. Ils n'étaient pas verts : `cargo test` s'arrête au
premier binaire rouge, **entre binaires**, et ni l'un ni l'autre n'a jamais été
exécuté. Leur absence du transcript n'est pas un résultat. Le rayon d'explosion
était sous-rapporté d'un facteur quatre. Le même drapeau manque à la commande de
régression que `DOMAIN.md` met devant chaque correcteur : **`CHDR-042`**, §6ter.

**Réconciliation.** Maintenu à P2, et **c'est ce finding qui porte le verdict
`SEMANTIC_FALSE_POSITIVE` du scénario 8** (§5). Le réfuteur dissident montre que
les cas négatifs du wrap et la restauration effective de la dérivation sont
couverts ailleurs — `c1_header_seal.rs:122`, `g-revocation.feature:65-69` et
`:76-79`, `g3_move.rs:157-159`. Argument de couverture, non de contrat : il ne
rend pas ce scénario honnête. Le même réfuteur formule une réserve que la
réconciliation retient et promeut en finding propre : il n'existe nulle part de
négatif **du wrap** par AAD divergente (`CHDR-026`).

**Référence de spec.** `spec/03-headers.md:69-84`, `:130-134` ;
`spec/02-content-tree.md` §2.5.

**Critère de clôture.** Le `Given` construit un état réel : dériver `K_P` pour le
parent, dériver la clé enfant pré-rotation par `node_key`, faire tourner une
vraie rotation du header enfant, ranger les deux. Le `Then` recouvre `K_P` par
dérivation depuis un ancêtre **avant** d'ouvrir le wrap, et assère en outre que
la clé enfant dérivée pré-rotation n'ouvre plus la nouvelle version — la paire
« coupée puis rétablie » que le nom du scénario revendique.

---

### `CHDR-002` — **`VERIFIED`** le 2026-08-04, P3 — 3/3 réfutations (réfuté, reformulé, déclassé)

> **Statut de clôture.** `VERIFIED` par la revue indépendante du 2026-08-04
> (`auditor/runs/2026-08-04-review-lot-a.md` §1), sur la révision candidate
> `5905bec`, **dans sa formulation post-réconciliation** — la moitié « contrôle
> positif », la seule maintenue. La moitié « attribution de cause » avait été
> retirée par le panel et n'est pas rouverte.
>
> **Correction.** `corrupt_line` (`cucumber.rs:8199-8213`) ouvre une fois avant
> de basculer le caractère hex et une fois après. `replay_line_other_node`
> (`:8224`) enregistre une ouverture de contrôle de la ligne même qu'il
> s'apprête à voler, sur son propre header, avant la greffe. `opening_rejected`
> (`:12506-12528`) assère `opened.len() >= 2`, puis `opened.first() == Some(DK)`
> sous le message *positive control: the targeted line must open on its own
> header BEFORE the mutation*, puis `is_err()` sur chaque tentative ultérieure.
> C'est le critère de clôture mot pour mot.
>
> **Mutant.** `M9` — l'hypothèse même de cette note rendue exécutable : la ligne
> owner scellée à une clé étrangère (`header.rs:110`, la ligne **owner** scellée
> à `XPublicKey::from([0x77u8; 32])`, lignes grantee intactes).
> `check_owner_line` compare des clés publiques de `Recipient` et tourne avant
> `build_lines` (`header.rs:164`, `:169`), donc I3 passe encore et le header se
> construit encore ; seul le sceau de la ligne owner est mort.
>
> **Preuve — `ev-11dee753`** : 5 échoués / 3 passés. Scénarios 3 et 4 rouges au
> contrôle positif. **Scénario 5 vert.**
>
> **Ce que la preuve montre.** Le scénario 5 restant vert est le témoin que la
> revue avait spécifié *à l'avance* : il prouve que `M9` n'a pas accidentellement
> désarmé le portail I3, et que les deux RED sont imputables au **sceau**, pas à
> la construction. Les scénarios 1, 7 et 8 tombent aussi — dommage collatéral
> attendu d'une ligne owner inouvrable, et précisément le motif pour lequel cette
> note avait déclassé ce finding en P3 : aucun mutant de production ne survit à
> la `Rule` entière, `owner_opens` tombant. Le finding porte sur la force de
> preuve **par scénario**, et la preuve par scénario est que sous `M9` les
> *anciens* scénarios 3 et 4 étaient verts — leur dernière tentative restait
> `Err`, puisque sous `M9` rien n'ouvre — là où les nouveaux échouent sur un
> message qui nomme le contrôle.
>
> **Un effet de bord, mesuré, en faveur de la correction.** La revue a vérifié
> plutôt que supposé qu'aucun état ne fuit entre scénarios : le scénario 2 pousse
> deux `Err` dans `opened` et précède le scénario 3, dont le `Then` exige
> désormais `opened.first() == Some(DK)`. Si `opened` traversait la frontière, le
> scénario 3 serait rouge. Il est vert (`ev-1335c8f1`). L'assertion ajoutée ici
> est, incidemment, un détecteur vivant de la fuite que la passe d'état partagé
> cherchait (§10).

**Les deux scénarios de rejet n'ont aucun contrôle positif interne.**
**Scénarios 3 et 4 (`Then` partagé).**

*Énoncé gelé au Pass A* : `opening_rejected` (`cucumber.rs:12342-12345`) sert deux
phrases et n'assère que `opened.last().unwrap().is_err()` ; les deux scénarios
prouveraient « une erreur est survenue », pas « le sceau a rejeté ».

*Réfutation unanime, vérifiée sur le code courant et acceptée* : les cinq sorties
d'erreur de `Header::open` (`header.rs:232`, `:234`, `:235`, `:237`, `:242`) sont
**toutes** la variante `Error::SealRejected` — asserter la variante serait une
tautologie ; et `open_into` fait `.map_err(|e| e.to_string())`
(`cucumber.rs:7402`), donc il n'existe plus de variante à examiner au moment du
`Then`. Surtout, dans les deux montages toutes les branches concurrentes sont
inatteignables : `corrupt_line` (`:8104-8112`) bascule un caractère hex en
préservant longueur et validité hex, la version 1 existe, `epk`/`n` sont intacts,
le kid correspond — seule la branche `:242` (échec du tag AEAD) est atteignable ;
`replay_line_other_node` (`:8114-8122`) insère une ligne bien formée de même kid
dans un header valide — idem. `is_err()` **est** donc « le sceau a rejeté ».

**Contre-preuve instruite au Pass B, qu'aucun réfuteur n'avait examinée.**
L'étalon de juillet formule un grief distinct sur le même `Then` : l'absence de
**contrôle positif**. Vérifié sur le code courant, il tient :

> Ni le scénario 3 ni le scénario 4 n'établissent que la ligne visée s'ouvrait
> *avant* la mutation. `sealed_header_owner_grantee` (`cucumber.rs:7553-7566`)
> et `sealed_header_owner_only` (`:7569-7573`) construisent le header et rien ne
> l'ouvre jusqu'au `When`, qui mute puis ouvre une seule fois. Une régression de
> fixture rendant la ligne owner définitivement inouvrable — un `owner_rec()`
> pointant sur une mauvaise clé publique, par exemple — laisserait les deux
> scénarios **verts**, l'assertion étant satisfaite pour une raison qui n'a rien
> à voir avec la corruption ni avec le rejeu.

**Formulation exacte retenue après réconciliation :**

> `opening_rejected` est un `is_err()` nu sans base connue bonne dans le corps du
> scénario. Les scénarios 3 et 4 ne sont pas différentiels : ils asserent l'échec
> après mutation sans avoir asserté le succès avant. Le seul contrôle positif de
> la `Rule` vit dans un **autre** scénario, `owner_opens` (`cucumber.rs:12312`,
> scénario 1).

**Réconciliation.** Réfuté 3/3 dans sa formulation gelée — la moitié
« attribution de cause » est **retirée**, le panel a raison. La moitié « contrôle
positif », distincte et non réfutée, est **maintenue et déclassée en P3** : sous
le code courant aucun mutant de production ne survit à la `Rule` entière grâce à
ce défaut, puisque `owner_opens` (scénario 1) tomberait. C'est un défaut de force
de preuve à l'échelle du scénario, pas un défaut vivant. Il se compose avec
`CHDR-027`.

**Référence de spec.** `spec/03-headers.md:29-32`, `:119-129`.

**Critère de clôture.** Rendre chaque scénario de rejet différentiel dans son
propre corps : dans `corrupt_line`, ouvrir une fois avant la bascule et une fois
après, asserter `opened[0] == Ok(DK)` puis `opened[1].is_err()` ; dans
`replay_line_other_node`, enregistrer une ouverture de contrôle de la ligne volée
sur son header d'origine avant la greffe.

---

### `CHDR-015` — `OPEN`, P3 — 3/3 réfutations (réfuté, reformulé, déclassé)

**La `Rule` du grant teste la primitive, pas la capacité de grant.**
**Scénario 6.**

*Énoncé gelé au Pass A* : l'étape 1 de §3.3 (« Open the node's current DK (own
line) ») n'est pas exercée ; le `When` passe la constante `DK` ; `append_line`
accepte un `dk` arbitraire sans vérifier qu'il est celui que scellent les lignes
présentes.

*Réfutation unanime, vérifiée sur le code courant et acceptée* : (i) mauvaise
attribution de couche — `append_line` (`header.rs:159-188`) ne détient aucun
secret X25519 et ne peut structurellement pas ouvrir une ligne ;
`session.rs:352-353` documente la frontière (« the DK and owner KEX secret never
cross this API boundary ») ; (ii) l'étape 1 **est** implémentée et exercée à sa
couche — `session.rs:364-365` (`open_latest` puis `append_line`),
`grants.rs:459-460`, `bundle.rs:631`/`:638`, et `e-mandates.feature:23-25`
traverse `Bundle::grant` puis l'agent déchiffre le contenu réel
(`cucumber.rs:9588`) ; (iii) la prémisse « la valeur que l'appelant lui a
scellée » est fausse — `assert_eq!(dk, DK)` (`cucumber.rs:12331`) compare à la
**constante de module** `DK = [0x77; 32]` (`:263`), un oracle de terrain, et
conjuguée au `Then` byte-identique elle épingle owner et grantee sur la même
valeur.

**Formulation exacte retenue après réconciliation :**

> La `Rule` « Grant is one appended line » n'exerce que la primitive
> `Header::append_line`. Aucun de ses pas ne traverse une capacité de grant de
> production — ni `Session::append_header_recipient` (`session.rs:354-366`), ni
> `Bundle::grant` (`grants.rs:739`). C'est une observation de **couverture** de
> la `Rule`, pas un défaut d'assertion du scénario.

**Réconciliation.** Réfuté 3/3, **déclassé en P3 et reformulé**. Le résidu concédé
par deux réfuteurs est exactement la formulation ci-dessus. Ce résidu recoupe
`CHDR-016`, qui porte la conséquence de sécurité ; `CHDR-015` n'en garde que la
part de couverture.

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Fusion avec `CHDR-016` : un pas de la `Rule` qui traverse
une surface de grant conforme à §3.3 clôt les deux.

---

### `CHDR-020` — `OPEN`, P3 — 2/3 réfutations (réfuté, reformulé, déclassé)

**Le `Given` du scénario 8 est un corps vide : l'état composite qu'il nomme
n'est jamais construit.**
**Scénario 8.**

*Énoncé gelé au Pass A* : `derived_node_rotated` (`cucumber.rs:7598-7601`) a un
corps vide (un commentaire) ; les trois faits qu'il pose sont fictifs —
`CHILD_NODE` n'est relié à `PARENT_KEY` par aucune dérivation, aucune `rotate`
n'a lieu, `DK2` est une constante.

*Réfutations majoritaires, vérifiées sur le code courant* : (i) le `When`
reconstruit l'état énoncé sous forme d'arguments de `Wrap::seal` (via, via_key,
node, key_version = 2, dk) et écrit `w.wrap_obj` ; le `Given` vide est un idiome
récurrent du fichier, dont le jumeau `dk_and_two_recipients` (`:7548`) dans la
même feature ; (ii) les constantes ne sont pas arbitraires — vérifié :
`PARENT_KEY = [0x55; 32]` (`cucumber.rs:265`) est `vectors/g2-rotation.json:19`
(`via_key_hex = 5555…`), `DK2 = [0x66; 32]` (`:264`) est `:20`
(`new_dk_hex = 6666…`), `CHILD_NODE` est `:22`, la version 2 est `:23` ; et le
lien parent→enfant **est** cryptographique via `derive_key(CTX_WRAP_KEY, via_key)`
(`seal.rs:19`, `:137`), `CHILD_NODE` entrant dans `wrap_aad` (`seal.rs:39-42`).
Exiger une dérivation parent→DK' contredirait d'ailleurs
`spec/03-headers.md:66` (« Generate DK' (fresh random) … not derived from
anything he holds »).

**Nuance ajoutée par le Pass B.** L'alignement sur le vecteur G2 est partiel et
ne fait pas du scénario un contrôle de conformité : `vectors/g2-rotation.json:21`
fixe `nonce_hex = 7777…` là où le scénario passe `non(9) = [0x69; 24]`, et
`:24` fixe un `subject_did` différent de `DID_C`. Les deux entrent dans le calcul,
donc le chiffré du scénario ne peut pas égaler celui du vecteur, et aucune
assertion ne les compare.

**Formulation exacte retenue après réconciliation :**

> `derived_node_rotated` a un corps vide et ne place aucun état dans le `World`.
> Le texte du contrat est donc inexécutable : il peut être réécrit sans changer
> le résultat, et les entrées réelles du test ne sont pas lisibles depuis le
> contrat. Fidélité de contrat, au même titre que `CHDR-004` et `CHDR-010`.

**Réconciliation.** Réfuté 2/3, **déclassé en P3 et reformulé** en finding de
fidélité de contrat. La sévérité au niveau du scénario n'est pas perdue : elle
est portée par `CHDR-021`, qui survit, et qui établit le verdict
`SEMANTIC_FALSE_POSITIVE`. Les deux findings visent le même scénario et le
présent résultat les rend cohérents plutôt qu'opposés — le `Given` vide est le
**mécanisme**, le `Then` en aller-retour est la **conséquence**. Résidu concédé
par les réfuteurs et retenu : rien ne vérifie que `via` est ancêtre de `node`
(voir §9, impact `g-revocation`).

**Référence de spec.** `spec/03-headers.md:64-88`.

**Critère de clôture.** Que le `Given` place l'état qu'il nomme dans le `World` et
que le `When` le consomme. Subsumé par le critère de `CHDR-021`.

---

### `CHDR-004` — `OPEN`, P3

**`Given` vide : tout l'arrangement du scénario 1 vit dans son `When`.**
**Scénario 1.** Non soumis au panel (P3).

`dk_and_two_recipients` (`cucumber.rs:7548-7551`) a un corps vide. Le `When`
`seal_into_header` (`:8092-8095`) délègue à `sealed_header_owner_grantee`
(`:7553`), qui est aussi le `Given` des scénarios 2 et 3 : **le `When` du
scénario 1 et le `Given` des scénarios 2 et 3 sont le même code**. La séparation
`Given`/`When` du scénario 1 est fictive. Voir `CHDR-027` pour la conséquence
d'état partagé.

**Critère de clôture.** Que le `Given` pose `DK` et les deux `Recipient` dans le
`World` et que le `When` les consomme.

---

### `CHDR-005` — `OPEN`, P3

**« every line » n'est contraint par aucune assertion du scénario 2.**
**Scénario 2.** Non soumis au panel (P3).

`stranger_recovers_nothing` (`cucumber.rs:12335-12339`) vérifie
`!opened.is_empty()` puis `all(is_err)` : le nombre de tentatives n'est pas fixé.
Le mot « every » n'est porté que par la boucle du `When` sur le littéral
`["owner-kex", "g1"]` (`:8098`), pas par les lignes du header. Une ligne ajoutée
au fixture ne serait pas essayée ; un kid littéral cessant de correspondre
produirait un `Err` vide depuis `header.rs:242` sans aucun déchiffrement. Sous le
code actuel la couverture est en fait complète : défaut de force de preuve, pas
défaut vivant.

**Critère de clôture.** Dériver la liste de kids du header, et asserter
`opened.len() == lines.len()` à côté de `all(is_err)`.

---

### `CHDR-006` — `OPEN`, P3

**Aucun scénario n'épingle la bijection destinataire → ligne, et le constructeur
tronque silencieusement.**
**Scénario 1.** Non soumis au panel (P3).

`build_lines` (`header.rs:83-102`) zippe `recipients` avec
`ephemerals.iter().zip(nonces)` : un `zip` tronque à la plus courte des trois
séquences, sans erreur. Aucun scénario n'assère que le nombre de lignes égale le
nombre de destinataires.

**Recoupe `CHDR-023`, requalifié hors périmètre (§7).** La différence est que
`CHDR-023` visait `Header::rotate` — dont les deux appelants construisent des
cardinalités égales par construction — tandis que `CHDR-006` vise une assertion
que le scénario 1 pourrait porter et ne porte pas. `CHDR-006` reste donc un
finding, à P3, dans le périmètre de la vérité sémantique du scénario 1.

**Critère de clôture.** Une assertion de cardinal dans le `Then` du scénario 1.

---

### `CHDR-010` — `OPEN`, P3

**`Given` vide : les paramètres de la phrase ne sont jamais liés.**
**Scénario 5.** Non soumis au panel (P3).

`single_grantee` (`cucumber.rs:7576`) a un corps vide. Ni « a node key » ni « a
single grantee recipient » ne deviennent un état ; le `When`
`build_without_owner` (`:8124-8137`) recrée tout depuis des constantes de
compilation.

**Critère de clôture.** Identique à `CHDR-004`.

---

### `CHDR-011` — `OPEN`, P3

**Le `Then` n'assère qu'une sous-chaîne du message d'erreur.**
**Scénario 5.** Non soumis au panel (P3).

`build_without_owner` (`cucumber.rs:8134`) range `e.to_string()`, détruisant
l'erreur typée à la frontière du `World` ; `header_invalid` (`:12347-12351`)
assère ensuite `msg.contains("I3")`. La variante typée `Error::MissingOwnerLine`
est publique et `Error` dérive `PartialEq`. Ni la variante ni le nœud transporté
(`/e/circle`) ne sont vérifiés : la discrimination tient par coïncidence
lexicale entre `error.rs:59` et `:71`. Un chemin de nœud contenant le littéral
`I3` satisferait l'assertion sans que le contrôle owner soit la cause — la charge
utile est `node.to_owned()` (`header.rs:75`).

Le scénario reste néanmoins fail-closed : le `When` panique sur `Ok`
(`cucumber.rs:8133`) et l'`unwrap()` du `Then` sur un `rejection` à `None`
échoue indépendamment.

**Note d'état partagé.** `rejection` (`cucumber.rs:463`) est un champ du `World`
partagé par tout le fichier de 19 700 lignes : il est écrit en `:7796` et
`:8134`, lu en `:12348` et `:12513`. Le `World` étant réinstancié par scénario
(§7), aucune valeur ne traverse un scénario aujourd'hui. Le jour où un `Given`
de `c-headers` écrirait `rejection`, l'assertion par sous-chaîne cesserait d'être
discriminante sans que rien ne le signale.

**Critère de clôture.** Ranger l'erreur typée et asserter
`matches!(err, Error::MissingOwnerLine(ref n) if n == NODE_A)`.

---

### `CHDR-017` — `OPEN`, P3

**La revendication `O(1)` du récit de la `Feature` n'est ni mesurée ni assertée.**
**Scénario 6.** Non soumis au panel (P3).

`features/c-headers.feature:5` et `spec/03-headers.md` §3.3 revendiquent `O(1)`.
La seule trace est structurelle : `append_line` fait un `push` (`header.rs:180`)
et ne lit aucun champ d'aucune ligne existante (`:159-188`). C'est une preuve de
code, pas une preuve de scénario.

**Critère de clôture.** Soit une assertion structurelle (préfixe intact,
cardinal +1 — voir `CHDR-013`), soit le retrait de la revendication du récit.

---

### `CHDR-018` — `OPEN`, P3

**Le `Then` est une fonction partagée et câblée en dur, incapable de distinguer
une ligne appendue d'une ligne construite.**
**Scénarios 1 et 6.** Non soumis au panel (P3).

`grantee_opens` (`cucumber.rs:12324-12333`) porte deux phrases `#[then]` de deux
`Rule` différentes — « the grantee opens the header and recovers the node key »
(`features/c-headers.feature:14`) et « the new grantee opens the node key »
(`:43`) — et code en dur version 1, kid `g1`, secret `xsk(0x21)`, attendu `DK`.
Le mot « new » de la seconde phrase n'a aucun correspondant dans le code : la
fonction ne peut pas distinguer la ligne appendue par le scénario 6 de la ligne
construite par le scénario 1.

**Critère de clôture.** Deux fonctions distinctes, ou un paramètre Gherkin lié.

---

### `CHDR-024` — `OPEN`, P3

**Aucun pas de RU-4 n'appelle `check_rotation` : la bonne forme mécanique de
§3.4 est hors de portée de la `Rule` qui la nomme.**
**Scénarios 7 et 8.** Non soumis au panel (P3).

`Header::check_rotation` (`header.rs:275-305`) implémente exactement la bonne
forme que le titre de la `Rule` revendique. `Header::rotate` (`:192-217`) ne
l'appelle pas — il n'appelle que `check_owner_line` (`:201`). Ses appelants
vérifiés, exhaustivement, sur le code courant : `revoke.rs:199`, `vault.rs:400`,
`cucumber.rs:15260` (un pas de `g-revocation`), `g2_rotation.rs:79` et `:92`.
Aucun n'appartient à `c-headers`.

**Hors verdict, consigné pour l'intégration.** `check_rotation` est lui-même plus
faible que `spec/03-headers.md:93-96`, qui exige une **égalité** « previous minus
revoked » là où `header.rs:288-297` ne teste qu'une **inclusion** : une rotation
qui *supprime* un survivant sans autorité passe. Déjà connu du dépôt
(`docs/proposals/header-rotation-authority.md:37-48`, statut *Proposé — non
adopté*). Ce point n'est énoncé par aucun scénario de `c-headers` : il est
signalé, non audité.

**Recoupe `CHDR-009`** (portails I3) et l'étalon de juillet (§8).

**Critère de clôture.** Invoquer `check_rotation(2)` dans le `Then` existant du
scénario 7. Recouvre la moitié structurelle de `CHDR-019`.

---

### `CHDR-026` — `OPEN`, P3 — nouveau, issu de la passe d'état partagé

**Le wrap n'a aucun négatif par AAD divergente, nulle part.**
**Scénario 8.**

Le sceau de ligne dispose de négatifs sur les deux axes de son AAD —
`c1_header_seal.rs:100-102` (autre nœud) et `:105-107` (autre version) — même si
`CHDR-025` établit qu'ils sont vacants. Le wrap n'en a **aucun**. Recensement
exhaustif des sites qui exercent `wrap_open` ou `Wrap::open` dans le dépôt :

| Site | Ce qu'il assère |
|---|---|
| `c1_header_seal.rs:117-119` | aller-retour sous la bonne clé |
| `c1_header_seal.rs:122` | échec sous une **clé via** nulle — seul négatif du wrap |
| `g2_rotation.rs:112-116` | octets contre le vecteur, puis aller-retour |
| `g3_move.rs:157-176` | `wrap_aad` épinglé, puis aller-retour sous la nouvelle clé parent |
| `cucumber.rs:12401` | le `Then` du scénario 8 |
| `grants.rs:1054`, `:1063` | chemin de lecture de production |

`wrap_aad` est épinglé octet à octet (`g3_move.rs:157-159`), mais aucun test
n'établit qu'un `Wrap` posté sous un autre nœud ou une autre version est refusé.
`Wrap::open` recalculant son AAD depuis ses propres champs (`header.rs:351-353`),
un `Wrap` dont `node` ou `key_version` aurait été réécrit par un attaquant
échouerait — mais rien ne le prouve, et l'asymétrie avec le sceau de ligne est
non intentionnelle.

**Référence de spec.** `spec/03-headers.md:72-84`, `:130-134`.

**Critère de clôture.** Deux assertions dans `c1_header_seal.rs::c2_wrap_…` :
rouvrir le chiffré du vecteur sous `wrap_aad(did, autre_nœud, version)` puis
sous `wrap_aad(did, nœud, version + 1)`, les deux devant être `Err`, après un
contrôle positif dans le même corps (voir `CHDR-025`).

---

### `CHDR-027` — `OPEN`, P3 — nouveau, issu de la passe d'état partagé

**Toute la `Rule` RU-1 repose sur un unique constructeur de fixture, et son seul
contrôle positif vit dans un autre scénario que ceux qui en dépendent.**
**Scénarios 1 à 4.**

Trois des quatre scénarios de RU-1 partagent le même constructeur :
`sealed_header_owner_grantee` (`cucumber.rs:7553-7566`) est le `Given` des
scénarios 2 et 3 **et** le corps du `When` du scénario 1 (`:8092-8095`, via
`CHDR-004`). Le quatrième, le scénario 4, utilise
`sealed_header_owner_only` (`:7569-7573`), qui porte deux phrases `#[given]` —
`features/c-headers.feature:27` (scénario 4) et `:41` (scénario 6, une autre
`Rule`) — et écrit **deux** champs du `World`, `saved_line` et `header` ; le
scénario 4 reçoit donc un instantané `saved_line` qu'il ne lit jamais.

La `Rule` entière ne comporte ainsi que deux formes d'appel à `Header::build`, et
un unique contrôle positif : `owner_opens` (`:12312-12322`), dans le scénario 1.
Les scénarios 3 et 4 asserent un rejet sans jamais avoir établi une base connue
bonne dans leur propre corps (`CHDR-002`) : leur pouvoir de détection est donc
emprunté à un scénario voisin. Ce n'est pas un `PROXY` au sens du tableau des
statuts — aucun verdict partagé n'est consommé — mais c'est un couplage que
l'isolation par unité de revue ne pouvait pas voir, et c'est précisément ce que
le point 5 de `PROCESS.md` § *Review-unit isolation* demande d'instruire.

**Critère de clôture.** Un contrôle positif interne dans chacun des scénarios 3
et 4 (voir `CHDR-002`), ce qui rompt la dépendance.

---

## 6bis. Findings issus de la revue de correction du 2026-08-04

Neuf findings nouveaux, relevés par la revue indépendante de la correction
`CHDR-007` / `CHDR-012` sur la révision candidate `9dc5889`
(`auditor/runs/2026-08-04-review-i3-authority.md`). **Aucun n'empêche la
clôture** des deux findings assignés : chacun a été arbitré explicitement comme
*distinct* et non comme un défaut de la correction — le motif est donné dans
chaque bloc. Aucun n'est assigné à un correcteur par cette revue.

Les identifiants reprennent à `CHDR-028` : `CHDR-026` et `CHDR-027` étaient déjà
attribués par la passe d'état partagé du cycle précédent.

### `CHDR-028` — `OPEN`, P2 — **publié en entier le 2026-08-04 sur décision du propriétaire**

**Titre : couverture inégale de I3 entre les surfaces de vérification d'édition
de `aithos-bundle`.**

> **Levée de l'embargo.** Ce finding a été retenu à l'identifiant et au titre
> neutre du 2026-08-04T05:45Z au 2026-08-04T13:00Z, sous la condition de
> blocage 9. Le propriétaire a tranché : publication intégrale. L'énoncé
> ci-dessous est celui qui lui avait été transmis hors dépôt, restitué sans
> retrait. Le fichier hors dépôt n'existe plus.

**Énoncé.** Le lot B a doté deux vérificateurs d'édition du contrôle I3 :
`Bundle::verify` (`rust/crates/aithos-bundle/src/bundle.rs:1759`) et
`publication::cold_verify` (`rust/crates/aithos-bundle/src/publication.rs:897`),
tous deux via `verify_pinned_headers` (`bundle.rs:302-320`).

Un **troisième** vérificateur public reste muet :
`KeylessPublicationPackage::verify_public_only`
(`rust/crates/aithos-bundle/src/publication.rs:586-591`) et son enveloppe
`verify_for_cas` (`:643-650`), qui délèguent à `verify_draft2_candidate`
(`:469`). Cette fonction contrôle la forme du manifeste, la signature d'acteur,
la topologie, l'égalité `manifest.files == expected_files` et les porteurs
K1-C — et **rien sur I3**.

Le paquet contient pourtant les octets nécessaires :
`objects = context.candidate_store.clone()` (`:660`) est le store complet
post-mutation, donc les `e/…/hdr/*.json` et `did.json`, et `manifest.files` les
épingle (`:204`). C'est exactement ce que `import_keyless` (`:729`) puis
`cold_verify` relisent pour, eux, rejeter. `export_keyless` (`:651-694`)
s'auto-valide par le même appel muet (`:691`).

Cette surface est consommée **comme un verdict d'acceptation** :
`rust/crates/aithos-bundle/src/sdk.rs:36`, `PublicationUploadPlan::verified`,
documenté « Verify the complete package locally and derive the provider
operation order ». Un paquet épinglant un header qui viole I3 obtient donc un
plan d'upload « vérifié localement », part chez le fournisseur, et n'est refusé
qu'ensuite par un tiers qui aurait la bonne idée d'appeler `cold_verify`.

`spec/09-cli-and-conformance.md:99-101` (§9.4) exige le rejet « without holding
any key, and **on every `aithos-core` manifest profile** ».
`verify_public_only` est précisément le vérificateur du profil draft.2 côté
producteur.

**Pourquoi l'embargo avait été levé sur cette base.** Le producteur d'une
édition n'est pas nécessairement le sujet : `spec/05-delegation.md:85-91`
autorise un délégué ou un ancêtre à re-sceller les lignes, ligne owner comprise.
C'est le raisonnement même qui avait écarté la défense « auto-sabotage » pour
`CHDR-012`. L'énoncé décrit donc un chemin non corrigé par lequel une édition
non conforme obtient un verdict d'acceptation sur une API publique — ce qui est
exactement ce que la barrière retient par défaut, et exactement ce que le
propriétaire a choisi de publier pour que le finding devienne assignable.

**Critère de clôture.** `verify_draft2_candidate` appelle
`verify_pinned_headers` sur `context.candidate_store` et `did.json`, et un test
RED démontre qu'un paquet dont un header viole I3 est refusé par
`verify_public_only`, `verify_for_cas` et `PublicationUploadPlan::verified`, là
où il est aujourd'hui accepté.

**Assignation.** Aucune. `c-headers` est `COMPLETE` et n'est jamais rouverte :
la surface visée appartient à `aithos-bundle`. Le finding est porté par
`QUEUE.yaml` sous `chdr-028`, à charge du premier cycle de `d-bundle` ou de
`k-integration` qui l'ouvre.

**Pourquoi il n'empêche pas `CHDR-007` d'être `VERIFIED`.** Le critère de
clôture de `CHDR-007` nomme deux fonctions — `Bundle::verify` et
`publication::cold_verify` — et la décision du 2026-08-03 n'en nomme qu'une. Les
deux sont faites, prouvées par `ev-47ec8aac` → `ev-b925a0cf`. La surface visée
ici a une reachability, un contrat et un moment de vie distincts ; elle mérite
son propre critère de clôture, pas une réouverture. L'arbitrage est
**discutable** et il est explicitement remis au propriétaire : `spec/09-cli-and-conformance.md`
§9.4 dit « on every `aithos-core` manifest profile », ce qui peut se lire comme
englobant cette surface.

### `CHDR-029` — `OPEN`, P2

**La clé publique d'un destinataire survivant est reconstruite depuis `to`,
l'étiquette de routage, et non depuis `kid`, le champ qui nomme sa clé.**

`spec/03-headers.md:34-38` est explicite dans les deux sens : « `to` […] is a
routing hint only — the seal is what grants. […] **No verifier decides anything
from `to`** » et « **`kid` names the line's recipient key** ».

Quatre sites de production violent cette répartition. Dans la même boucle qui
reconnaît correctement la ligne owner par son `kid`, la branche `else` fait :

```rust
// revoke.rs:188
let ed = wire::multibase_to_ed25519_pub(&line.to)?;
...
survivors.push(Recipient { to: line.to.clone(), kid: line.kid.clone(), pubkey: ed2x(&vk) });
```

- `rust/crates/aithos-bundle/src/revoke.rs:188` (`rotate_folder`)
- `rust/crates/aithos-bundle/src/revoke.rs:396` (`move_folder`)
- `rust/crates/aithos-bundle/src/structure.rs:266` (`structural_recipients`)
- `rust/crates/aithos-bundle/src/vault.rs:381` (`rotate_vault_connector`)

**Conséquence.** Sur un header portant une ligne où `to != kid`, la rotation
scelle DK' sous la clé désignée par `to` tout en recopiant `kid` verbatim : la
nouvelle ligne **ment sur son destinataire**. Le détenteur du `kid` déclaré perd
l'accès, celui de l'étiquette l'acquiert, et `Header::check_rotation`
(`header.rs:347-356`) ne peut rien voir puisqu'il ne compare que des ensembles
de `kid`. Le graphe d'accès que `spec/03-headers.md` §3.6 promet de ne jamais
sur-déclarer devient faux.

**Précondition, et ce qui la borne.** Aucun écrivain de production ne produit
`to != kid` : `grants.rs:161-166` et `log.rs:441-445` posent `to = kid =
multibase Ed25519`, `Recipient::owner` pose `to = "owner"`, traité par la
branche `kid == owner_kid`. La divergence n'entre que par un `header.json`
édité à la main, un bundle importé, ou `aithos header-seal`
(`header_seal.rs:53-57`, format libre `label:kid:pubkey`). Et rien ne la rejette
au passage : ni `Header::validate`, ni `check_rotation`, ni
`bundle::verify_pinned_headers` ne contrôlent `to == kid`.

**Antériorité.** Ces quatre lignes sont **inchangées** par la correction
`9dc5889` (`git diff 5be3047..9dc5889` ne les touche pas ; seules les branches
`line.to == "owner"` voisines ont été retirées). Le finding préexiste donc à la
correction et n'en est pas une régression.

**Pourquoi il n'empêche pas `CHDR-012` d'être `VERIFIED`.** Le critère de
clôture de `CHDR-012` nomme trois points de contrôle — `check_owner_line`,
`validate`, `check_rotation` — et les trois ont migré. Ces quatre sites ne sont
pas des *contrôles* I3 : ce sont des résolutions de clé de grantee, sur un
champ que la même spec déclare non autorisant. C'est la même faute de catégorie,
sur un autre objet, et elle a son propre critère de clôture.

**Référence de spec.** `spec/03-headers.md:34-38`, `:93-96` ; §3.6.

**Critère de clôture.** Les quatre sites résolvent la clé du survivant depuis
`line.kid` (décodage `multibase_to_ed25519_pub` du `kid`, avec repli fail-closed
si le `kid` ne décode pas), **ou** un contrôle `to == kid` pour toute ligne non
owner est ajouté à `Header::validate` et donc au vérificateur d'édition. Dans
les deux cas : un test RED construisant un header portant `{ to: A, kid: B }`,
le faisant tourner, et constatant que la nouvelle version scelle sous B — test
qui passe sur `9dc5889` et échoue après correction.

### `CHDR-030` — `OPEN`, P3

**Le palier `owner_kex`-porteur de I3, rendu obligatoire par la spec amendée,
est implémenté mais n'a aucun appelant de production.**

`spec/03-headers.md:40-42` impose **deux** paliers : « Every verifier MUST
check, without any key, that some line of every key version declares
`owner_kex` as its `kid` ; **a verifier holding `owner_kex` MUST additionally
check that that line opens under it, and MUST reject the header when it does
not.** »

`Header::validate_as_owner` (`rust/crates/aithos-core/src/header.rs:385-401`)
implémente exactement le second palier, et il est **ajouté par la correction**
(absent de `5be3047`). Son seul appelant du dépôt est
`rust/crates/aithos-core/tests/c3_owner_line.rs:171`. Recherche exhaustive sur
`crates/*/src/` : zéro.

Les quatre surfaces de production qui détiennent pourtant `owner_kex`
n'appellent que le palier keyless : `bundle.rs:667`
(`zone_dk_with_owner_kex`), `bundle.rs:674` (`vault_dk`), `log.rs:427`
(`audit_key_owner_with_kex`), `session.rs:363` (`append_header_recipient`).
L'`open_owner` qui suit chacune fait bien échouer le chemin si la ligne ne
s'ouvre pas — mais **pour la seule version ouverte**, alors que §3.1 exige le
contrôle sur toutes. Un header multi-versions dont une version ancienne porte
une ligne déclarant `owner_kex` sans s'ouvrir sous elle passe.
`vault.rs:334` (`read_vault_config_owner`) n'appelle même pas `validate`, à
rebours de son homologue `log.rs:427`.

Le cas est celui que le vecteur nomme `owner_label_foreign_seal` et que le
vecteur lui-même qualifie de « documented boundary of spec 03.1 » : aucun
vérificateur keyless ne l'attrape, et c'est précisément pourquoi la spec ajoute
le second palier.

**Référence de spec.** `spec/03-headers.md:40-42` ; `vectors/c3-owner-line.json`,
cas `owner_label_foreign_seal`, `tier: "owner_kex"`.

**Critère de clôture.** Les surfaces détenant `owner_kex` appellent
`validate_as_owner` au lieu de `validate`, ou une raison écrite est consignée
pour chacune qui ne le fait pas. Un test RED : un header à deux versions dont la
version 1 porte une ligne déclarant `owner_kex` mais scellée ailleurs, accepté
aujourd'hui par `zone_dk_with_owner_kex`, rejeté après.

### `CHDR-031` — `OPEN`, P3

**Effet partiel : `Bundle::move_folder` écrit l'index avant la garde I3, sur une
API publique non transactionnelle.**

`rust/crates/aithos-bundle/src/revoke.rs:324` (`pub fn move_folder`) n'est
enveloppé dans aucune `self.transaction(...)`. Il écrit
`e/circle/index.json` en `:422`, **puis** appelle `Header::build_at` en `:431`,
dont la première instruction est la garde `check_owner_line`
(`header.rs:164`). Si le header source ne porte pas de ligne dont le `kid` vaut
`owner_kid`, `survivors` ne contient pas la ligne owner (`:393-394` jamais
atteint), `build_at` renvoie `MissingOwnerLine`, et le store reste avec le
dossier reparenté, sans header au nouveau chemin et sans wrap up-link.

Le même ordonnancement existe en `structure.rs:777` → `:781`, mais y est
**couvert** : `structural_operation` (`structure.rs:1102-1109`) enveloppe tout
dans `self.transaction`, qui rollback sur `Err` (`bundle.rs:421-437`).
`revoke.rs::rotate_folder` (`:142`) est indemne : `check_owner_line` (`:203`) et
`check_rotation` (`:214`) précèdent la première écriture (`:215`).

**Critère de clôture.** Soit `move_folder` est enveloppé dans une transaction
comme `structural_operation`, soit la garde I3 est évaluée avant la première
écriture. Un test RED : `move_folder` sur un header sans ligne owner, puis
constat que `e/circle/index.json` a changé alors que l'appel a échoué.

### `CHDR-032` — `OPEN`, P3

**L'unicité des `kid` dans une `key_version` n'est imposée nulle part.**

`spec/03-headers.md:38` : « Two lines of one key version **MUST NOT** carry the
same `kid`. » Aucun contrôle du dépôt ne l'applique :
`Header::validate` (`header.rs:371-378`) cherche une occurrence et s'arrête,
`check_rotation` (`:334-364`) raisonne sur un `BTreeSet` qui absorbe les
doublons, `append_line` (`:190-219`) et `build_lines` (`:96-120`) n'inspectent
rien, et `bundle::verify_pinned_headers` (`bundle.rs:302-320`) hérite de
`validate`.

**Conséquence.** Une seconde ligne déclarant `owner_kex` mais scellée à un tiers
est acceptée par tout vérificateur keyless, y compris le vérificateur d'édition
que `CHDR-007` vient d'installer. `aithos header-seal` peut l'émettre
directement : `--recipient <label>:<owner_kid>:<clé étrangère>`
(`header_seal.rs:53-57`), le `--owner-kex-hex` obligatoire ne contraignant que
la ligne owner que la commande construit elle-même. Le palier porteur y
résisterait — `Header::open` (`:266`) essaie **toutes** les lignes du `kid` —,
mais `CHDR-030` établit qu'il n'est appelé nulle part.

**Critère de clôture.** `Header::validate` rejette toute `key_version` portant
deux lignes de même `kid`, et le cas rejoint la famille C3 du §9.2 avec son
vecteur. Test RED : un header à deux lignes `owner_kex`, accepté par
`Bundle::verify` sur `9dc5889`, rejeté après.

### `CHDR-033` — `OPEN`, P3

**Le bump majeur de version du crate exigé par la décision n'a pas eu lieu.**

La décision du 2026-08-03 écrit : « Cinq signatures publiques changent — **bump
majeur de version du crate**. » Les cinq ont bien changé (`build`, `build_at`,
`rotate`, `validate`, `check_rotation` — confirmé par
`git diff 5be3047..9dc5889 -- rust/crates/aithos-core/src/header.rs` et par le
rapport du correcteur). La version, elle, n'a pas bougé : `rust/Cargo.toml:12`
porte toujours `version = "0.1.0-alpha.1"`, et la section `[Unreleased]` de
`CHANGELOG.md:10-13` ne mentionne pas la rupture d'API de `aithos-core::header`.

S'y ajoute une rupture de **format au repos** que le correcteur signale
lui-même : un header écrit par un binaire antérieur porte `kid: "owner-kex"` et
échoue désormais `Bundle::verify`. Aucun artefact de ce type n'existe dans
l'arbre, mais l'obligation est rétroactive et arbitrée comme telle ; elle
appartient au journal de version autant qu'à la revue d'impact.

**Critère de clôture.** Version du workspace incrémentée selon la politique
SemVer du dépôt, et une entrée `CHANGELOG.md` nommant les cinq signatures et la
non-lisibilité des headers antérieurs.

### `CHDR-034` — `OPEN`, P3

**L'émetteur signe des éditions que son propre vérificateur refuse.**

`Bundle::publish` (`rust/crates/aithos-bundle/src/bundle.rs:1678`) ne comporte
aucune garde I3. Le test `c3_owner_line_edition.rs:239-246` en fait la
démonstration : il écrit un header mutilé via `bundle.store.put` — le champ
`store` est `pub` (`bundle.rs:284`), ce qui court-circuite `validate_store_key`
et toute autre invariante —, appelle `publish`, **qui réussit**, et n'obtient
l'erreur qu'à `verify`.

C'est un choix de conception assumé par le correcteur, et argumenté : placer le
contrôle dans `state_tree()` aurait rendu `publish` fail-closed et le RED
`ev-47ec8aac` inatteignable. Le constat n'en est pas moins que
`spec/09-cli-and-conformance.md:102-104` définit le **Core issuer** comme « the
above (reader) + … », donc comme portant aussi l'obligation du lecteur.

**Critère de clôture.** Soit `publish` refuse d'épingler un header violant I3 —
en laissant au test un chemin d'injection post-signature —, soit
`spec/09-cli-and-conformance.md` §9.4 dit explicitement que l'obligation I3 ne
lie que la vérification et jamais l'émission.

### `CHDR-035` — `OPEN`, P3

**Les deux seules surfaces CLI de §03 ne sont invoquées par aucun test.**

`aithos header-seal` et `aithos header-open` sont les surfaces sur lesquelles la
décision du 2026-08-03 impose une contrainte nominative — « elle ne doit pas
pouvoir produire silencieusement un header que `verify` rejetterait ».
`rust/crates/aithos-cli/tests/cli_surface.rs` ne les invoque jamais ; le constat
était déjà consigné par `docs/research/topology-2026-07-28-unverified/lot-A-00-01-03-10.md:239`.

La correction a bien durci les deux commandes — `--owner-kex-hex` obligatoire et
ligne owner construite par le programme (`header_seal.rs:19-20`, `:42-44`),
`--owner-kid` obligatoire et `validate` avant toute ouverture
(`header_open.rs:17`, `:35`). **Rien ne le prouve par exécution.**

Écart résiduel à la lettre de la décision, à consigner ici : celle-ci offrait
deux branches — lire le document DID, **ou** exiger un drapeau explicite pour le
cas non lié. La correction a pris une troisième voie, supprimer le cas non
gouverné. Elle ferme la production d'un header *sans* ligne owner ; elle ne
ferme pas la production d'un header dont l'`owner_kex` n'est pas celui du sujet,
et aucun nommage ne le signale. Le propriétaire s'était réservé ce point comme
« le seul point de cette décision que le propriétaire pourrait vouloir
reprendre » : il lui revient.

**Critère de clôture.** `cli_surface.rs` exerce `header-seal` et `header-open`,
dont au moins un cas négatif : un header produit avec un `--owner-kex-hex`
étranger, épinglé dans une édition, rejeté par `Bundle::verify`.

### `CHDR-036` — `OPEN`, P3

**La couverture de `Header::validate` sur les chemins de lecture reste inégale —
résidu de `CHDR-008`, absorbé par `CHDR-007` mais non traité par la lecture
retenue.**

`spec/03-headers.md:40` dit « **Every** verifier MUST check, without any key,
that some line of every key version declares `owner_kex` as its `kid` ».
`spec/09-cli-and-conformance.md` §9.4 rattache l'obligation à la vérification
d'édition, qui est faite. Entre les deux textes subsiste un écart de portée que
la décision n'a pas tranché : elle a explicitement écarté la troisième lecture
— « la validation sur les chemins de lecture » —, et le correcteur le consigne
dans ses limites.

`validate` est appelé en `bundle.rs:318`, `:667`, `:674` ; `log.rs:427` ;
`session.rs:363` ; `cli/header_open.rs:35`. Il ne l'est pas en `grants.rs:834`,
`:1044`, `:1204` ; `structure.rs:192`, `:279`, `:752` ; `revoke.rs:155`, `:303`,
`:383`, `:526` ; `vault.rs:334` ; `log.rs:399`.

**Ce n'est pas un défaut de la correction** : la lecture retenue par le
propriétaire n'exigeait pas ces sites. C'est un écart entre la spec amendée et
le code, consigné pour que la prochaine décision le voie.

**Critère de clôture.** Soit `spec/03-headers.md:40` restreint « every verifier »
au vérificateur d'édition, en cohérence avec §9.4 ; soit les chemins de lecture
listés appellent `validate`.

## 6ter. Findings issus de la revue de correction du lot A (2026-08-04)

Cinq findings nouveaux, relevés par la revue indépendante du lot A sur la
révision candidate `5905bec`
(`auditor/runs/2026-08-04-review-lot-a.md` §5 et §6). **Aucun n'empêche la
clôture** des huit findings du lot : quatre visent le train — le process, les
gates déclarés, le cycle de vie des marqueurs — et le cinquième vise un
artefact hérité du lot B. Aucun n'est assigné à un correcteur par cette revue.

Les identifiants reprennent à `CHDR-037`, `CHDR-036` étant le plus haut de §6bis.
**`CHDR-041` est réservé et n'est pas ouvert** — le motif est donné dans son
bloc, et il est enregistré ici pour que l'identifiant ne soit jamais réutilisé.

### `CHDR-037` — `OPEN`, P3 — le cycle de vie des marqueurs n'a pas d'état `IMPLEMENTED`

`PROCESS.md:232-234` admet `IMPLEMENTED` parmi les statuts justifiant un
marqueur Gherkin ; `:236-238` ne retire le marqueur qu'à `VERIFIED`. Entre les
deux, la prose du marqueur est **tenue de rester** et est **garantie** de
décrire un état que le candidat n'a plus. Le cas s'est produit sur cette feature
et il est mesuré, pas supposé : à la révision candidate,
`c-headers.feature:33-39` disait encore *« both headers are built at version 1
and the open is at version 1, so key_version never varies »* et *« Outside
Gherkin the version binding is defended only by byte pins against vectors, never
by a behavioural differential »*, tous deux falsifiés par `ev-9ba93af7` et
`ev-ad4db6a1` ; et `:47-51` disait *« Only the build-time I3 gate is exercised on
its fail-closed side; the normative case declared by vectors/g2-rotation.json has
no consumer »*, falsifié par `ev-dce43f1c`. Un lecteur arrivant en cours de cycle
ne peut pas distinguer un trou vivant d'un trou fermé, alors que
`PROCESS.md:229-231` exige que les marqueurs *« describe current, actionable
gaps »*.

**Ce n'est pas imputable au lot A.** `PROCESS.md:307` assigne le retrait des
marqueurs au **reviewer**, pas au correcteur : que le lot A ait laissé ces blocs
intacts est le process appliqué correctement.

**Critère de clôture.** Soit le marqueur porte le statut en ligne
(`# AUDIT CHDR-009 — IMPLEMENTED, awaiting review`), soit `PROCESS.md`
§ *Gherkin audit-marker lifecycle* énonce qu'un correcteur met à jour la prose
des marqueurs qu'il adresse. Coût : nul, alpha.

### `CHDR-038` — `OPEN`, P3 — la revendication de génération indépendante restaurée n'est imposée par aucun gate

**Fichier.** `vectors/gen-c.py`. **Symbole.** `check_c1()` (`:167-207`), appelé
sans condition par `main()` (`:283-299`) ; le script accepte en outre `--check`,
qui vérifie de surcroît `c3-owner-line.json` octet à octet au lieu de le
réécrire.

`check_c1` est l'artefact qui règle la moitié 2 de `CHDR-025` : il reconstruit la
ligne owner, la ligne grantee et le wrap C2 de `c1-header-seal.json` depuis une
seconde implémentation (blake3 + PyNaCl + HKDF RFC 5869 manuel) et les assère
contre le fichier committé sans l'écrire. **Rien ne l'exécute.**

**L'affirmation d'absence, avec sa recherche, sa portée et sa couche.** Dépôt
entier, toutes couches, vérifié par le présent rôle et non repris de la revue :
`grep -rn "gen-c\.py\|gen_c\.py"` sur l'arbre suivi → `vectors/gen-c.py:20` (sa
propre docstring d'usage), `vectors/ownership.json:270` (une entrée de manifeste
de propriété, qui épingle le fichier et ne l'exécute pas), et des journaux
d'orchestrateur et rapports de run sous `features/.agents/`. Aucun site
d'exécution. `.github/workflows/ci.yml` compte cinq `run:` —
`verify-feature-tags.sh` (`:22`), `cargo fmt --check` (`:33`), `cargo clippy`
(`:35`), `cargo test --workspace` (`:37`), `cargo check -p aithos-wasm` (`:59`)
— dont aucun n'est Python. `grep -rn "\-\-check" .github/workflows scripts` →
`cargo fmt … --check` seul. `vectors/ownership.json` épingle le sha256 du
vecteur et `vectors_ownership.rs` impose l'épinglage, ce qui attrape une dérive
du **fichier** mais pas une divergence entre le générateur et l'implémentation
Rust — exactement ce que `check_c1` existe pour détecter. La revendication de
`c1_header_seal.rs:2-3` est donc reproductible à la demande et vérifiée par
aucun gate.

**Cela se généralise** : `vectors/` contient **29** générateurs `gen-*.py`
(`ls vectors/gen-*.py | wc -l`) et aucun gate n'en exécute un seul.

**Non imputable au lot A.** `gen-c.py` est arrivé par `5be3047`, base du lot B.
Le lot A en a hérité. Le finding tient sur ses propres pieds.

**Critère de clôture.** Une étape CI, ou un `#[test]` derrière un drapeau de
feature, exécutant `python3 gen-c.py --check` depuis `vectors/` ; et le même
traitement généralisé aux autres générateurs, ou une décision enregistrée
énonçant explicitement que les générateurs de vecteurs ne sont exécutés qu'à la
main au moment de l'écriture.

### `CHDR-039` — `OPEN`, P3 — les gates finaux déclarés omettent le gate clippy que la CI impose

`features/.agents/c-headers/DOMAIN.md` § *Final global gates* (`:290-296`) liste
`cargo test … --test cucumber`, `cargo test … --workspace --no-fail-fast` et
`cargo fmt … --check`. `.github/workflows/ci.yml:35` exécute en outre
`cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings`.
Le lot A ajoute plusieurs centaines de lignes que `--all-targets` compile. Un
correcteur exécutant les gates déclarés peut donc passer la main sur un candidat
que la CI refuse.

**Ce que cette note ne peut pas dire.** Que le candidat est clippy-propre. La
revue n'a pas exécuté clippy — c'est un gate global, et `PROCESS.md:86` le lui
interdit. Le correcteur, lui, l'a exécuté et déclare `ev-d6ce5ee9`, exit 0 ; ce
fait est une revendication du correcteur, non une vérification indépendante, et
il est cité comme tel.

**Critère de clôture.** Ajouter l'invocation clippy à `DOMAIN.md`
§ *Final global gates* — et aux `DOMAIN.md` des autres features, l'omission ne
leur étant pas spécifique.

### `CHDR-040` — `OPEN`, **P2** — les clauses de process que ce train applique ne sont pas dans `PROCESS.md`

**Ce finding vise le train, pas `c-headers`.** Il est consigné dans l'audit
public de cette feature parce que c'est le seul endroit tracé où la revue du lot
A pouvait le porter, et il est **remis au propriétaire** : la présente note ne
modifie pas `PROCESS.md`, qui ne lui appartient pas.

**La revendication.** Trois des dispositifs normatifs de ce cycle sont cités
comme des sections de `PROCESS.md` et n'y sont pas :

1. § *Material isolation of Pass A* — la règle qui a produit l'extrait remis au
   reviewer ;
2. la **liste numérotée des conditions de blocage**, 1 à 10 ;
3. la **barrière de divulgation**, condition de blocage 9 — la règle qui décide
   ce qu'un dépôt public n'apprend pas.

**Qui les cite comme liantes.** Numérotation du dépôt à `10de842`, vérifiée par
le présent rôle ; la revue citait la numérotation de l'extrait de `5905bec`, que
deux commits d'orchestrateur ont depuis décalée.

| Site de citation | Texte |
|---|---|
| `features/.agents/c-headers/STATE.md:29` | « …withheld from the reviewer until its behavioural verdict was frozen (`../PROCESS.md`, § *Material isolation of Pass A*) » |
| `features/.agents/c-headers/STATE.md:93` | « …**without the corrector's run report** until its behavioural verdict is frozen (`PROCESS.md`, § *Material isolation of Pass A*) » |
| `features/.agents/c-headers/STATE.md:77` | « **All four blocking conditions are now closed.** Conditions 9, 6 and 7 by the disclosure and budget ruling of 2026-08-03 ; condition 1 by … » |
| `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md:71` | « Matérielle, conformément à AM (`PROCESS.md` § *Material isolation of Pass A*). » |
| `features/.agents/c-headers/corrector/runs/2026-08-04-correction-i3-authority.md:156` | « …i.e. blocking condition 8 of `PROCESS.md` » |
| `features/.agents/c-headers/corrector/runs/2026-08-04-correction-lot-a.md:330` | « …the same shape that worked on lot B (`PROCESS.md`, § *Material isolation of Pass A*) » |
| `features/.agents/c-headers/corrector/runs/2026-08-04-correction-lot-a.md:248` | « Correcting it from this branch would have been blocking condition 8, scope. » |
| les briefs remis aux rôles **de ce cycle-ci**, y compris à celui qui écrit cette mise à jour | « This is `features/.agents/PROCESS.md`, § *Material isolation of Pass A* » et « This is blocking condition 9 » |

Huit sites, dont les instructions données aux rôles du cycle en cours.

**La recherche, sa portée et sa couche.** Dépôt suivi entier à `10de842`, toutes
couches, ni `features/**` ni `rust/**` seuls, exécutée par le présent rôle :

```text
grep -rn "Material isolation\|blocking condition" --include=*.md .
```

Occurrences, en entier, hors le présent document et hors le rapport de revue qui
énonce le finding :
`docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md:108`, `:233` (le titre de
section), `:261`, `:392`, `:475` ;
`docs/RECONNAISSANCE-ORCHESTRATEUR-2026-08-03.md:125` (une ligne de tableau qui
le *propose*) ; `features/.agents/orchestrator/LEDGER.md:48` ;
`features/.agents/orchestrator/BLOCKED.md:214` ;
`features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md:71` et
`:295` ; `features/.agents/c-headers/corrector/runs/2026-08-04-correction-i3-authority.md:156` ;
`features/.agents/c-headers/corrector/runs/2026-08-04-correction-lot-a.md:248` et
`:330` ; `features/.agents/c-headers/STATE.md:29`, `:77`, `:93` ; et
`docs/audits/features/README.md:79`.

**`features/.agents/PROCESS.md` fait 371 lignes et apparaît zéro fois dans cette
sortie.** Sa table § *Artifacts* (`:215-222`) ne liste pas la proposition. Sa
§ *Review-unit isolation and impartiality* (`:188-211`) — la section que la
proposition amende — s'achève sur *« A later orchestrator may spawn fresh agents
for the review units without changing the evidence model »* et ne contient
aucune règle d'isolation matérielle.

**Trois écarts avec la recherche de la revue, déclarés plutôt que lissés.** La
revue comptait 372 lignes et six sites de citation ; le présent rôle en compte
371 et huit. Les trois écarts s'expliquent et aucun ne déplace le finding :
(i) la revue tournait sur un extrait `git archive` de `5905bec`, dont `STATE.md`
a été réécrit depuis par `c1f8380` et `10de842` — d'où les numéros de ligne
différents ; (ii) le rapport de correction du lot A était **soustrait** au
reviewer par l'isolation matérielle elle-même, d'où ses deux sites manquants —
la règle non écrite a caché deux des citations de la règle non écrite ;
(iii) `audit-initial.md:295` et `PROPOSITION…:108` citent « blocking condition »
sans citer une section absente et n'appuient pas la revendication ; ils sont
listés pour que la recherche soit reproductible, pas pour la gonfler.

**Le texte manquant, cité.** De
`docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md:233-261` :

> ### Material isolation of Pass A
>
> In orchestrated mode, Pass A isolation is material, not declarative. Each Pass A
> review unit runs against an extract of the immutable revision produced by
> `git archive`, with no `.git` directory. The agent does not refrain from reading
> history; it cannot, because no history is present.
>
> The correction review uses the same device. The reviewer receives an extract of
> the candidate revision, without `.git` and without the corrector's run report,
> until its behavioral verdict is frozen. The diff and the corrector's conclusion
> are delivered only for Pass B.
>
> An instruction not to read history is not sufficient for an unattended agent and
> must not be relied upon as the sole barrier.

Du même fichier, `:460-470`, les conditions numérotées, dont `PROCESS.md` ne
porte aucune trace — ni la liste, ni la numérotation :

> 2. A third rejection of the same finding.
> 3. A red gate not attributable to the current scope.
> 4. Pass A contamination, declared or detected.
> 5. A refutation panel majority against the auditor.
> 6. Two warden invalidations of the same feature.
> 7. An exhausted budget — time, tokens, or disk.
> 8. A diff outside the assigned scope.
> 9. A finding caught by the disclosure gate.
> 10. A `FULL_AUDIT` classification by the impact review.

Et `:475-479`, la barrière elle-même :

> `aithos-core` is public, and orchestrated branches are pushed to it. A finding
> whose written statement would describe an exploitable weakness before a fix
> exists must not be written to any tracked file. The agent records the finding
> identifier and a neutral title, and raises blocking condition 9. The human owner
> decides what is published, and when.

La note de clôture de la proposition (`:487-495`) énonce que le propriétaire l'a
révisée *« before this proposal was ever applied to `PROCESS.md` »*. Ce n'est
donc pas un brouillon en attente de revue : c'est un document que le
propriétaire a déjà amendé **en place**, pendant qu'il siège hors du fichier
normatif.

**Pourquoi c'est plus qu'un renvoi cassé.** `PROCESS.md:110-121` établit une
hiérarchie de preuve où *« Git history is context, not proof »* et où la trace
écrite d'un gate passé *« is history »*. La même discipline appliquée aux règles
elles-mêmes donne au problème sa forme : un rôle à qui l'on dit d'obéir à
`PROCESS.md` § *Material isolation of Pass A*, qui ouvre `PROCESS.md` et l'y
cherche, ne trouve rien — et n'a aucun moyen de distinguer « la règle a été
renommée » de « la règle n'existe pas » de « on me demande quelque chose que
personne n'a écrit ». `PROCESS.md:141-146` énumère même ce qu'un Pass A peut
lire, et la proposition n'y figure pas : un lecteur strict du fichier normatif
refuserait donc de lire le document qui contient la règle qui le lie.

La barrière de divulgation est le bout tranchant. C'est la seule règle dont la
défaillance est **irréversible** : un finding écrit dans un fichier suivi d'un
dépôt public ne peut pas être dé-écrit. Elle n'est aujourd'hui définie nulle part
dans le jeu d'artefacts que `PROCESS.md` § *Artifacts* énumère. La revue du lot A
l'a appliquée — elle a cherché un finding embargeable, a trouvé `CHDR-032`, et a
refusé d'embargoter un énoncé déjà publié — et elle l'a appliquée depuis un
document de proposition, sur l'instruction d'un brief, non depuis le process.

**Texte normatif minimal**, tel que la revue le propose. Deux formes ; le choix
appartient au propriétaire.

**Forme A, préférée — appliquer les trois blocs.** Insérer § *Material isolation
of Pass A* verbatim après `PROCESS.md:211` (fin de § *Review-unit isolation and
impartiality*, qu'elle amende) ; insérer les conditions de blocage numérotées et
le bloc § *Disclosure gate* verbatim avant § *Evidence statuses* ; ajouter la
proposition à la table § *Artifacts* comme source supersédée.

**Forme B, si l'application n'est pas encore souhaitée.** Un paragraphe dans
`PROCESS.md`, placé immédiatement après la table § *Artifacts* :

> **Orchestrated-mode amendments.** In orchestrated mode this process is extended
> by `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md`, which is normative
> for material isolation of Pass A, the adversarial refutation panel, the numbered
> blocking conditions 1-10, and the disclosure gate. Where the two disagree, the
> amendment wins for orchestrated runs. A role that cannot locate a cited section
> in this file looks there before proceeding, and treats its absence from both as
> a blocking condition rather than as permission.

La forme B fait trois phrases et rend résoluble chacune des six citations.
Aucune des deux ne coûte quoi que ce soit : rien n'est déployé, aucune édition
n'est publiée, et le travail passé d'aucun rôle n'est invalidé par le fait
d'écrire ce qu'il faisait déjà (`features/AGENTS.md` § *Project stage*).

**Ce qui n'est explicitement pas revendiqué.** Qu'un rôle ait mal appliqué ces
règles. L'isolation matérielle a bien été appliquée au reviewer — l'extrait
était sans `.git`, et le dépôt complet n'a pas été ouvert. La barrière a
visiblement joué (§15). Les règles sont suivies. Elles ne sont simplement pas
écrites là où elles sont citées, et un système de règles qui ne tient que parce
que tout le monde les connaît déjà est à un changement de personnel de ne plus
tenir.

**Critère de clôture.** Un lecteur de `features/.agents/PROCESS.md` qui y
cherche « Material isolation », « blocking condition » ou « disclosure gate » y
trouve soit la règle, soit un renvoi non ambigu vers elle.

### `CHDR-041` — **réservé, non ouvert**

Tenu par l'orchestrateur pour la contingence énoncée dans `CHDR-021` : si le
paragraphe des mutants survivants de ce bloc était supprimé à la clôture, le
résidu `M12` perdrait son domicile et `CHDR-041` s'ouvrirait pour l'accueillir.
**La condition ne s'est pas déclenchée** : le paragraphe est conservé mot pour
mot, avec `ev-ec9412a7` et `ev-cbce8aa0` en paire.

L'identifiant est consigné ici pour qu'il ne soit **jamais réutilisé**. Cet audit
porte déjà une collision d'identifiants (§1) et n'en a pas besoin d'une seconde.

### `CHDR-042` — `OPEN`, P3 — la commande de régression déclarée masque les échecs après le premier binaire rouge

**Fichier.** `features/.agents/c-headers/DOMAIN.md`, § *Relevant regressions*
(`:275-280`) :

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-core --test c1_header_seal --test g2_rotation --test g3_move --test b2_derivation
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb10_structure_vault --test vectors_ownership
```

Ni l'une ni l'autre ne porte `--no-fail-fast`. `cargo test` s'arrête au premier
échec **entre binaires de test** : le premier binaire rouge avorte le run et les
binaires suivants ne s'exécutent jamais. Leur absence du transcript se lit
exactement comme un succès et n'en est pas un.

**Mesuré, sur cette revue, deux fois.** Même mutant (`M12`), mêmes binaires, un
drapeau d'écart :

| Run | Commande | Rapporté |
|---|---|---|
| `ev-debade53` | telle que `DOMAIN.md` l'écrit | **1** échec, dans `c1_header_seal` ; `g2_rotation` et `g3_move` silencieux |
| `ev-cbce8aa0` | `--no-fail-fast` ajouté | **4** échecs répartis sur les trois binaires |

Le rayon d'explosion était sous-rapporté d'un facteur quatre, et les deux
binaires disparus étaient ceux qui portaient la réponse. Ce n'est pas
hypothétique : cela a failli coûter un verdict faux à la revue du lot A
(`CHDR-021`, note de quasi-accident).

**Sévérité, argumentée à la baisse plutôt qu'à la hausse.** La commande ne peut
pas produire un faux vert — le code de sortie reste non nul et un correcteur ne
peut pas prendre le run pour un succès. Le dommage est plus étroit : une image
incomplète de l'échec, dans un train dont §14 *Définition de terminé* exige du
correcteur qu'il « documente les deux résultats » et dont
`features/.agents/orchestrator/LEDGER.md:44-51` rend les compteurs imprimés
aussi liants que le code de sortie. Un correcteur rapportant « une régression,
dans `c1_header_seal` » rapporterait la vérité de son transcript et pas celle de
son changement. P3.

**L'incohérence est interne au même document.** `DOMAIN.md` § *Final global
gates* (`:294`) porte bien `--no-fail-fast` sur le gate workspace. Le drapeau est
compris ; il manque simplement à l'étage où une invocation multi-binaires le rend
nécessaire.

**Critère de clôture.** Ajouter `--no-fail-fast` aux deux commandes de
`DOMAIN.md` § *Relevant regressions*, et à la section équivalente des `DOMAIN.md`
des autres features, le motif étant recopié.

### Barrière de divulgation — vérifiée pour le lot A, rien à retenir

La revue du lot A a exécuté le contrôle plutôt que de supposer qu'il ne
s'appliquait pas, et le présent rôle a vérifié ce jugement plutôt que d'en
hériter.

**Un candidat a été trouvé** : `spec/03-headers.md:39-40` — *« Two lines of one
key version MUST NOT carry the same `kid` »* — que rien dans le dépôt n'impose,
et dont la conséquence est une seconde ligne déclarant l'`owner_kex` du sujet
tout en scellant ailleurs. Recherches de la revue :
`grep -rniE "duplicate|uniq|dedup"` sur `header.rs` et `aithos-bundle/src/` →
`state.rs:83`, `merge.rs:356`, `log.rs:856`, `bundle.rs:135`, aucune portant sur
les lignes de header ; `grep -rniE "same .?kid|kid.*uniq"` sur
`*.rs *.md *.py *.json`, dépôt entier → la phrase de spec, deux documents de
proposition, un commentaire sans rapport.

**Il est déjà publié en entier**, comme `CHDR-032` de §6bis — chemin d'émission
inclus (`aithos header-seal --recipient <label>:<owner_kid>:<clé étrangère>`) et
mention que le palier porteur résisterait mais n'est appelé nulle part
(`CHDR-030`). Vérifié dans le présent document, pas repris sur parole. La
barrière protège les énoncés qui ne sont **pas encore** publics ; embargoter la
reformulation d'un finding publié serait du théâtre et ferait passer un finding
publié pour un finding retenu.

**Rien à retenir au titre de la condition de blocage 9 pour ce lot.** Les cinq
findings ci-dessus visent le process, les gates déclarés et le câblage d'un
générateur de vecteurs : aucun ne décrit un chemin d'exploitation. L'erratum
`CHDR-019` décrit la construction d'un **mutant** — du code délibérément
modifié — et non le code du dépôt, qui lie bien le secret DH ; ce n'est pas une
faiblesse exploitable de `aithos-core`. `CHDR-028` était encore sous embargo au
moment de cette passe et n'a pas été touché ; l'embargo a été levé le même jour
par le propriétaire, voir la ligne 9 du journal de divulgation. Consigné avec ses recherches, parce que « aucun finding embargoté »
est une revendication comme une autre.

## 7. Findings retirés ou requalifiés

| Id | Panel | Décision de réconciliation | Motif, sur preuve de code courant |
|---|---|---|---|
| `CHDR-003` | 2/3 réfuté | **retiré** — embargo levé avec le finding | voir ci-dessous |
| `CHDR-008` | 2/3 réfuté | **retiré** en tant que finding autonome — absorbé par `CHDR-007`, publié en entier en §6 | voir ci-dessous |
| `CHDR-022` | 1/3 réfuté (survivait) | **requalifié en impact** `g-revocation` (§9) — n'est plus un finding `c-headers` | voir ci-dessous |
| `CHDR-023` | 3/3 réfuté | **requalifié hors périmètre** — durcissement défensif | voir ci-dessous |

### `CHDR-003` — retiré, embargo levé

*Titre neutre du gel : « Actual reach of the node binding proved by the replay
scenario ».* L'embargo tombant avec le finding, l'énoncé est publié.

*Énoncé gelé* : `Header::open` (`header.rs:228`) construit l'AAD depuis
`self.node`, le champ auto-déclaré du fichier désérialisé, et non l'emplacement
de stockage ; la liaison prouvée par le scénario 4 serait « ligne ↔ champ node »
et non « ligne ↔ nœud d'appartenance ».

*Réfutation confirmée par le Pass B sur trois preuves de code courant, chacune
revérifiée indépendamment :*

1. **Le verrou est structurel et ailleurs.** `hdr_file`
   (`grants.rs:139-146`) place tout header à
   `e/<zone>/hdr/<blake3(node)[..12]>.json`. Déplacer un header sous un autre
   nœud sans que le lecteur s'en aperçoive exigerait une collision BLAKE3 sur
   96 bits.
2. **L'AAD des blobs ne vient jamais de `header.node`.** `open_blob_v`
   (`bundle.rs:504-518`) calcule `blob_aad(&self.did, &node.to_string(),
   version)` depuis le `NodePath` **résolu par l'appelant**, comme `seal_blob`
   (`:492`). Un header déplacé rend donc la DK de son nœud d'origine, laquelle
   n'ouvre aucun blob du nœud cible : fail-shut.
3. **L'ancrage Merkle indexe le hash du header par son chemin.**
   `vault_build` (`state.rs:240-248`) et `header_hash_at` (`:58-62`) associent
   `BLAKE3(JCS(header.json))` à `path` ; `manifest.files` épingle
   path → sha256 ; le tout est signé.

Un contrôle explicite chemin ↔ champ existe par ailleurs, en `vault.rs:114-119`
(`header.node != Self::config_node(connector)` → `Error::SealRejected`), ce qui
confirme que l'idiome est connu du dépôt et employé là où il est nécessaire.

Le réfuteur dissident objectait qu'aucun contrôle équivalent n'existe aux autres
sites de lecture et que `read_vault_config_owner` (`vault.rs:335`) ne recoupe pas
non plus. C'est exact, mais sans conséquence : les points 1 à 3 rendent la
substitution soit impossible, soit inoffensive. **Aucune preuve de code courant
ne soutient une conséquence de sécurité. Le finding est retiré.**

### `CHDR-008` — retiré en tant que finding autonome

*Titre neutre du gel : « Coverage of parse-time I3 validation across header read
paths ».*

Ce finding portait sur la **couverture** de `Header::validate` sur les chemins de
lecture. Sa base factuelle est vérifiée et n'est pas contestée : `Header` dérive
`Deserialize` sans hook (`header.rs:47`), cinq sites seulement appellent
`validate()` — `bundle.rs:630`, `:637`, `log.rs:425`, `session.rs:363`,
`aithos-cli/src/cmd/header_open.rs:28` — tandis que `Header` est désérialisé sur
bien plus de sites, dont `grants.rs:287`, `:456`, `:827`, `:1037`, `:1197`,
`structure.rs:199`, `:751`, `revoke.rs:289`, `:365`, `:510` et `bundle.rs:670`.
`append_line` (`header.rs:159-188`) ne refait pas `check_owner_line`.

Les deux réfutations acceptées : (i) I3 est une propriété de **disponibilité**,
non de confidentialité (`spec/10-threat-model.md:19`), et aucun site non validant
ne produit de résultat faux — `Header::open` échoue fail-shut ; (ii) cinq des
sept chemins qui **mutent** un header portent un contrôle I3 équivalent via
`rotate` ou `build_at`, si bien que le trou réel se réduit à `add_line_on`
(`grants.rs:287-291`), et l'asymétrie propriétaire/délégué invoquée par le Pass A
est fausse en général — `bundle.rs:670` est un chemin propriétaire non validant.

Le Pass B constate que cet énoncé est **un sous-ensemble strict** de `CHDR-007`,
dont il partage la question normative et la décision attendue. Le conserver comme
finding autonome dédoublerait la même décision humaine et lui donnerait deux
critères de clôture concurrents. Il est donc **retiré et absorbé par
`CHDR-007`**, dont il devient une pièce de dossier. Sa matière est publiée
ci-dessus, l'embargo ayant été levé sur les deux identifiants le 2026-08-03
(§6, préambule).

Consigné pour le propriétaire de la décision : le réfuteur dissident (angle
périmètre) n'a pas pu réfuter et a signalé que
`features/.agents/c-headers/auditor/audit-c-headers/SKILL.md:52` et `:76`
**commandent nommément** cette analyse. Le retrait est un choix de structure du
dossier, pas un abandon de l'analyse.

### `CHDR-022` — requalifié en impact `g-revocation`

*Énoncé gelé* : le `via` modélisé par le scénario 8 ne correspond pas à celui que
la surface de rotation réelle poste.

Le Pass B **conteste l'énoncé sur la topologie du scénario lui-même**.
`NodePath::zone_root(Zone::Circle).to_string()` vaut `/e/circle`
(`path.rs:59-65`, `:135-147`, `Zone::as_str` `:20-26`) — c'est exactement
`NODE_A`. Et `CHILD_NODE = /e/circle/d/000…01` est de profondeur 1 : son parent
direct **est** la racine de zone. Le scénario modélise donc précisément ce que
`rotate_folder` poste (`revoke.rs:204-214` : `via = NodePath::zone_root(zone)`,
clé `zone_dk`). À sa propre profondeur, le scénario ne diverge pas de la
production.

Ce que le finding établit réellement, et qui est vrai sur le code courant, est un
défaut de **disponibilité en production à profondeur ≥ 2** : `rotate_folder`
poste toujours l'up-link sous la racine de zone, et `agent_section_key` ne tente
ce wrap que si `depth == 0` (`grants.rs:1061-1070`, commentaire « only the
zone-root key itself opens them » ; `structure.rs:216` répète le garde). Un
détenteur du parent intermédiaire entre la boucle à `depth = 1`, cherche
`wrap_file([a], [a,b])` qui n'existe pas, ne peut pas prendre la branche racine,
retombe sur `node_key` (`grants.rs:1071-1078`) et obtient une clé périmée — alors
que `spec/03-headers.md:76-80` promet que « holders of P (or of any ancestor of
P) keep reading N by derivation ». S'y ajoute que `agent_section_key` s'arrête au
**premier** header ouvrable et retourne (`grants.rs:1080-1082`) sans réessayer un
ancêtre plus haut.

Ce défaut vit entièrement dans `aithos-bundle`, ne se manifeste à aucune
profondeur atteinte par un scénario de `c-headers`, et relève de
`g-revocation`. Les limites du pilote (`DOMAIN.md` § *Pilot limits*) sont
explicites : ce qui touche `g-revocation` est **un impact à signaler, pas un
finding à auditer**. Il est donc requalifié et reporté en §9. La dette est déjà
consignée dans `docs/archive/HANDOFF.md:449`.

### `CHDR-023` — requalifié hors périmètre

*Énoncé gelé* : `Header::rotate` valide `check_owner_line` sur la liste de
destinataires puis délègue à `build_lines` dont le `zip` tronque silencieusement
(`header.rs:89-102`) — fail-open possible sur I3 ; et `key_versions.insert`
(`header.rs:202`) n'exige ni monotonie ni absence de la clé.

*Réfutation unanime, confirmée sur le code courant* : les deux cas sont
inatteignables. Les deux seuls appelants construisent éphémères et nonces par
`survivors.iter().map(…)` (`revoke.rs:196-197`, `vault.rs:389-390`) —
cardinalités égales par construction — et calculent `new_v = latest_version() + 1`
(`revoke.rs:156-157`, `vault.rs:387-388`), strictement croissant. Chacun appelle
`check_rotation` immédiatement après (`revoke.rs:199`, `vault.rs:400`), qui
revérifie la ligne owner **sur les lignes produites** (`header.rs:298-303`) ; et
`validate` rejoue I3 à chaque parse.

De surcroît, aucun scénario de `c-headers` n'énonce cette propriété :
`PROCESS.md` § *Current scope* exclut « general searches for behavior not
described by an existing scenario ». **Requalifié en durcissement défensif hors
périmètre.** Consigné, non audité, sans critère de clôture.

## 8. Comparaison à l'étalon manuel de juillet

L'étalon est `docs/audits/features/c-headers.md` de la branche publique
`origin/codex/audit-c-headers` (`af32734`), daté du 2026-07-30, révision observée
`3803fe8`, seize findings `CHDR-001`…`CHDR-016` numérotés dans un **espace de
noms distinct** de celui de la présente note (§1).

### 8.1 Le code audité est identique

Diff `3803fe8..a2087f2` sur le périmètre :

| Fichier | Diff |
|---|---|
| `features/c-headers.feature` | identique |
| `rust/crates/aithos-core/src/header.rs` | identique |
| `rust/crates/aithos-core/src/seal.rs` | identique |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | identique |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | identique |
| `rust/crates/aithos-core/tests/g3_move.rs` | identique |
| `vectors/c1-header-seal.json` | identique |
| `vectors/g2-rotation.json` | identique |
| `rust/crates/aithos-bundle/tests/cucumber.rs` | 16 insertions, 3 suppressions — **uniquement `main()`**, le correctif `BDER-011` |

Aucune définition de pas, aucun fixture, aucun champ du `World`, aucun helper
n'a bougé. La comparaison n'a donc **aucune excuse de dérive** : un finding de
juillet non retrouvé cette ronde est un manqué, pas une observation périmée.

### 8.2 Ce que les preuves de gate de juillet valent

Rien. La branche étalon part de `240c658`, antérieur au correctif `BDER-011` :
son `main()` appelait `filter_run`, qui sous `harness = false` sort `0` même avec
des scénarios en échec. L'étalon le dit lui-même. **Aucun chiffre de gate de
juillet n'est cité dans cette note.**

Une revendication d'exécution de juillet est en outre **contredite par le code
courant**, et le fait est consigné parce qu'il touche `CHDR-025`. L'étalon
rapporte qu'une mutation retirant `key_version` de `line_aad` laissait
« 18 features / 836 scenarios / 3577 steps » verts, la seule défaillance de tout
le workspace étant `c1_owner_and_grantee_lines`. Or `g3_move.rs:149-152` assère
`hex::encode(line_aad(&v.subject_did, &v.new_node, v.key_version)) ==
v.line_aad_hex`, et ce fichier est **identique** entre les deux révisions ; son
dernier commit, `97d7187`, est un ancêtre de `240c658`. Cette assertion aurait dû
tomber elle aussi. Ce rôle n'exécute aucune commande et ne peut donc pas
trancher par mesure : le fait est consigné comme une contradiction entre une
revendication d'exécution non reproduite et la lecture du code courant, et la
revendication est écartée. `CHDR-025` ne s'appuie que sur la lecture.

### 8.3 Table de correspondance — findings P1/P2 de juillet

Neuf findings de juillet sont P1 ou P2.

| Juillet (`af32734`) | Sév. juillet | Retrouvé seul cette ronde ? | Identifiant 2026-08-03 | Sév. | Écart |
|---|---|---|---|---|---|
| `CHDR-001` — le scénario du wrap ne prouve rien de ce qu'il revendique (sc. 8, `SEMANTIC_FALSE_POSITIVE`) | P1 | **oui** | `CHDR-021` (+ `CHDR-020`, `CHDR-026`) | P2 | sévérité abaissée P1 → P2 ; verdict de scénario identique après réconciliation |
| `CHDR-002` — « gets no line » prouvé comme « cannot open » (sc. 7) | P1 | **oui** | `CHDR-019` | P2 | sévérité abaissée P1 → P2 |
| `CHDR-003` — `check_rotation` n'est appelé par aucun pas de la `Rule` | P2 | **oui** | `CHDR-024` | P3 | sévérité abaissée P2 → P3 |
| `CHDR-004` — l'assertion « revoked cannot open » survit à la rotation qui n'a pas lieu | P2 | **non** | — | — | **manqué** (§8.4) |
| `CHDR-006` — la moitié « version » du scénario de liaison n'est jamais exercée (sc. 4) | P2 | **oui** | `CHDR-001` | P2 | identique |
| `CHDR-007` — les assertions de rejet n'attribuent aucune cause et n'ont aucun contrôle positif (sc. 3 et 4) | P2 | **partiellement** | `CHDR-002` | P3 | la moitié « cause » est réfutée 3/3 et retirée ; la moitié « contrôle positif » n'a été retrouvée qu'au Pass B, **en lisant l'étalon** — pas seule |
| `CHDR-010` — « touching nobody » exercé sur un header à une ligne (sc. 6) | P2 | **oui** | `CHDR-014` | P2 | identique ; réfuté 2/3 par le panel, rétabli en réconciliation |
| `CHDR-015` — I3 n'est pas imposé au niveau de l'édition (`DECISION_REQUIRED`) | P2 | **oui** | `CHDR-007` | P1 | sévérité **relevée** P2 → P1 ; cette ronde ajoute le second vérificateur `publication::cold_verify` (`publication.rs:836-939`) et le rattachement à `spec/10-threat-model.md:19`, absents de l'étalon. Un embargo avait été posé sur ce constat déjà publié par l'étalon ; il a été levé par décision du propriétaire le 2026-08-03 (§6, §15) |
| `CHDR-016` — le seul test qui garde la liaison de version la garde vacuement | P2 | **non** | `CHDR-025` (Pass B) | P2 | **manqué au Pass A** ; retrouvé au Pass B, indépendamment renforcé par l'absence de générateur `gen-c1*` |

### 8.4 Manqués — chiffres bruts

**Deux findings P1/P2 de juillet ont échappé au Pass A de cette ronde.**

- `CHDR-004` de juillet — **non retrouvé, à aucun stade.** L'assertion
  `revoked_cannot_open` (`cucumber.rs:12375-12383`) n'assère que `is_err()`. Si
  le `When` (`:8148`) était supprimé ou neutralisé, `key_versions` ne porterait
  aucune clé « 2 » et `Header::open` renverrait
  `Error::SealRejected("no key version 2")` en `header.rs:229-232` — **et ce
  `Then` passerait encore**. Il n'est protégé que par ses deux `Then` frères, qui
  font `unwrap()` sur la version 2. `CHDR-019` de cette ronde décrit la branche
  `header.rs:242-245` (boucle de kids vide) et **pas** la branche `:229-232`
  (version absente). Le manqué est réel et distinct. Vérifié sur le code courant.
  Il est absorbé par le critère de clôture de `CHDR-019`, qui exige une assertion
  structurelle établissant la précondition de version 2 — mais il n'a pas été
  trouvé par ce cycle.
- `CHDR-016` de juillet — **manqué au Pass A**, y compris par les seize
  réfuteurs. Pire : le panel a **utilisé** `c1_header_seal.rs:105-107` comme
  preuve de code courant pour imposer une correction de `CHDR-001`, c'est-à-dire
  s'est appuyé sur le test même que juillet avait montré vacant. Le Pass B a
  retrouvé le fait et l'a promu en `CHDR-025`, avec une preuve supplémentaire que
  juillet n'avait pas : l'absence de générateur `gen-c1*` dans `vectors/`.

Deux findings P3 de juillet sont également sans équivalent cette ronde et sont
consignés sans être promus : `CHDR-005` (les deux moitiés de la `Rule` de rotation
ne sont jamais jointes — les scénarios 7 et 8 visent des nœuds différents) et
`CHDR-009` (aucun scénario n'atteint les vecteurs C1/C2 ; `c1_header_seal.rs` ne
construit jamais de `Header`).

### 8.5 Nouveaux — ce que juillet n'avait pas

| Cette ronde | Sév. | Nature |
|---|---|---|
| `CHDR-012` | P2 | absent de l'étalon ; **0/3 réfutation** — le seul finding de la ronde à sortir du panel intact ; `DECISION_REQUIRED` |
| `CHDR-016` | P2 | le chemin de grant de production (`Bundle::grant` → `add_line_on`) appende à `KV = 1` après rotation ; absent de l'étalon |
| `CHDR-013` | P2 | cardinal et position des lignes après append — juillet le portait à P3 (`CHDR-012` de juillet), cette ronde à P2 |
| `CHDR-009` | P2 | le cas `missing_owner_must_fail` de `vectors/g2-rotation.json:17` n'a aucun consommateur — **trouvaille du panel de réfutation**, absente de l'étalon |
| `CHDR-022` (requalifié) | — | la divergence de `via` de `rotate_folder` à profondeur ≥ 2 ; absente de l'étalon ; reportée en impact `g-revocation` |
| `CHDR-026` | P3 | aucun négatif du wrap par AAD divergente ; absent de l'étalon |
| `CHDR-027` | P3 | couplage de fixture de RU-1 et localisation du seul contrôle positif ; absent de l'étalon |

### 8.6 Chiffres bruts

| Mesure | Juillet (`af32734`) | Cette ronde (`a2087f2`) |
|---|---|---|
| Findings publiés | 16 | 27 identifiants, dont 23 findings actifs |
| P1 | 2 | 1 |
| P2 | 7 | 9 |
| P3 | 7 | 13 |
| `DECISION_REQUIRED` | 1 | 2 |
| Retirés / requalifiés | 0 | 4 |
| Findings P1/P2 de juillet retrouvés seuls au Pass A | — | **6 sur 9** |
| Findings P1/P2 de juillet retrouvés au Pass B seulement | — | 1 sur 9 (`CHDR-016` de juillet) |
| Findings P1/P2 de juillet retrouvés partiellement | — | 1 sur 9 (`CHDR-007` de juillet) |
| Findings P1/P2 de juillet non retrouvés | — | **1 sur 9** (`CHDR-004` de juillet) |
| Verdicts de scénario identiques | — | 7 sur 8 (le scénario 5 : `PROVEN` en juillet, `PARTIAL` ici) |

**Lecture honnête.** Sur un code strictement identique, un pipeline orchestré de
quarante-huit agents de réfutation plus quatre unités de Pass A a retrouvé seul
six des neuf findings P1/P2 d'un audit manuel, en a manqué un entièrement et un
autre au Pass A, et en a produit quatre nouveaux de rang P2 dont un que
personne n'a pu réfuter. Le pipeline gagne en volume, en traçabilité et en
résistance aux formulations excessives — le panel a corrigé quatre énoncés
surdimensionnés et en a retiré deux. Il perd en tenue : les deux manqués sont
tous deux des assertions *vacantes* — un `is_err()` qui passerait sans que le
`When` ait eu lieu, un négatif qui passe sous n'importe quelle mutation de son
AAD. C'est un angle mort de méthode, pas de chance.

## 9. Impacts signalés, non audités

Le pilote borne l'audit à la vérité sémantique des huit scénarios de
`c-headers`. Ce qui suit est **signalé**, jamais audité, et n'ouvre aucune
feature.

| Cible | Impact | Origine |
|---|---|---|
| `g-revocation` | l'up-link de `rotate_folder` est posté sous la racine de zone et n'est lu qu'à `depth == 0` : à profondeur ≥ 2 un détenteur d'ancêtre perd la dérivation que `spec/03-headers.md:76-80` lui promet | `CHDR-022`, requalifié |
| `g-revocation` | `agent_section_key` s'arrête au premier header ouvrable (`grants.rs:1080-1082`) sans réessayer un ancêtre plus haut | `CHDR-022` |
| `g-revocation`, `d-bundle` | `KV = 1` (`bundle.rs:25`) survit à la livraison de l'étape G ; `add_line_on` appende à la version 1 après rotation | `CHDR-016` |
| `g-revocation` | `check_rotation` teste une inclusion là où `spec/03-headers.md:93-96` exige une égalité ; une rotation qui supprime un survivant passe (`docs/proposals/header-rotation-authority.md:37-48`) | `CHDR-024` |
| `h-merkle` | le hash du header est plié dans le hash de nœud (`state.rs:57-62`, `:240-248`) via un `serde_json::Value` opaque, sans que `Header::validate` soit jamais appelé sur ce chemin : un header violant I3 y produit un digest valide, épinglé puis signé | `CHDR-007` |
| transverse | `vectors/c1-header-seal.json` revendique une génération indépendante sans générateur dans le dépôt — obligation `TARGETED` déjà enregistrée | `CHDR-025` |
| transverse | le motif « kid du révoqué passé à `open_latest` » se retrouve en `cucumber.rs:5013` et `cb10_structure_vault.rs:548-553` | `CHDR-019` |

**Mise à jour du 2026-08-04.** Deux lignes de ce tableau ont bougé.

- La ligne `CHDR-016` est **exécutée en tant que routage** : l'orchestrateur a
  sorti `CHDR-016` du lot A le 2026-08-04 — son énoncé porte sur le comportement
  de grant de production dans `aithos-bundle`, pas sur une assertion Gherkin, et
  le corriger depuis une branche de sémantique de test aurait engagé la condition
  de blocage 8 — et l'a enregistré dans
  `features/.agents/orchestrator/QUEUE.yaml` sous `chdr-016-grant-path`, dû
  conjointement par `g-revocation` et `d-bundle`. **Ni clos ni retiré.** Son
  marqueur Gherkin reste vivant et nomme désormais son nouveau propriétaire.
- La ligne « générateur absent » de `CHDR-025` est **caduque** : `vectors/gen-c.py`
  existe depuis `5be3047`. Elle est remplacée par un impact plus étroit et
  vérifié — aucun gate n'exécute ce générateur, ni aucun des 29 de `vectors/`
  (`CHDR-038`, §6ter).

## 10. Passe d'état partagé — résultats négatifs

Consignés parce qu'un résultat négatif vérifié vaut mieux qu'une absence de
vérification.

- **Instanciation du `World`.** `ProtocolWorld` (`cucumber.rs:459-461`) dérive
  `Debug, Default, World`. Le harnais construit un `World` neuf par scénario :
  `opened`, `header`, `saved_line`, `rejection` et `wrap_obj` ne traversent
  **aucune** frontière de scénario. Vérifié.
- **`ProtocolWorld::open_into`** (`:7396-7404`). Trois sites d'appel dans tout le
  fichier, tous dans `c-headers` : `:8099` (dans la boucle du scénario 2),
  `:8110`, `:8120`. `opened` s'accumule au sein d'un scénario et `opening_rejected`
  lit `.last()` ; avec au plus une poussée par scénario de rejet, aucun risque de
  lecture d'un résultat étranger. Vérifié.
- **`OnceLock`, caches, `static`, hooks.** Les huit `OnceLock` du fichier
  (`:1100-1110`) sont des caches d'acceptation `CB4`/`CB5`/`CB6`/`CB7`/`CB10`, lus
  exclusivement en `:7269-7330`. **Aucun pas de `c-headers` ne les touche**, et
  aucun autre `static`, `lazy` ou hook n'est sur un chemin de header. Le gate
  filtré par `--tags @c-headers` n'en initialise donc aucun, et son résultat ne
  dépend pas de l'ordre des features. Vérifié.
- **Runner.** `main()` (`:19724-19746`) : `fail_on_skipped()` puis
  `filter_run_and_exit`, filtre `@wip` aux trois niveaux (feature, rule,
  scénario). Aucun scénario de `c-headers` n'est tagué : les huit sont
  sélectionnés, ce que confirment les compteurs de `ev-50caa5d6`. Vérifié.
- **Surfaces publiques de `DOMAIN.md`.** Toutes inspectées. `aithos-wasm`
  n'expose **aucune** surface de header ou de wrap — zéro occurrence de `Header`,
  `Wrap` ou `seal` dans `rust/crates/aithos-wasm/src/lib.rs`. Vérifié. Trois
  surfaces contournent le verdict exercé et portent chacune un finding :
  `Bundle::grant` (`CHDR-016`), les deux vérificateurs d'édition
  `Bundle::verify` (`bundle.rs:1654-1769`) et `publication::cold_verify`
  (`publication.rs:836-939`), muets sur I3 (`CHDR-007`), et la surface CLI de
  scellement `aithos-cli/src/cmd/header_seal.rs:30-56`, qui accepte un `to`
  libre (`CHDR-012`). Les surfaces
  conformes — `Session::append_header_recipient` (`session.rs:354-366`),
  `deliver_connector_line` (`grants.rs:454-461`), `header_open`
  (`aithos-cli/src/cmd/header_open.rs:27-32`) — ne sont traversées par aucun pas
  de la feature.
- **Pas partagés par plusieurs phrases ou plusieurs `Rule`.** Trois fonctions
  portent deux phrases : `sealed_header_owner_only` (`:7569`, deux `#[given]`,
  deux `Rule`), `grantee_opens` (`:12324`, deux `#[then]`, deux `Rule`),
  `opening_rejected` (`:12342`, deux `#[then]`, une `Rule`). Les conséquences
  sont portées par `CHDR-018`, `CHDR-002` et `CHDR-027`.

## 11. Plan d'implémentation

Ordonné par valeur. L'ensemble est du travail de test et de fixture dans
`rust/crates/aithos-bundle/tests/cucumber.rs`, plus deux additions dans
`rust/crates/aithos-core/tests/` et une édition Gherkin. **Aucun finding de
cette note n'exige une modification de production dans `aithos-core`.** Deux
findings exigent une décision humaine préalable, et l'un d'eux
(`CHDR-007`) pourrait, selon la décision, entraîner une modification de
production dans `aithos-bundle` — `Bundle::verify` et `publication::cold_verify`
— tandis qu'une décision sur `CHDR-012` pourrait toucher trois signatures
publiques de `aithos-core::header`. Aucune de ces deux corrections n'est
assignable avant décision.

| Lot | Findings | Changement | RED attendu |
|---|---|---|---|
| 0 | `CHDR-007`, `CHDR-012` | **rien** avant décision humaine | — |
| 1 | `CHDR-025`, `CHDR-026` | contrôle positif dans le corps de `c1_fail_closed` ; deux négatifs de wrap par AAD divergente ; statuer sur la provenance de `vectors/c1-header-seal.json` | retirer `key_version` de `line_aad` → `c1_fail_closed` doit tomber **sur son cas de version**, pas ailleurs |
| 2 | `CHDR-021`, `CHDR-020` | reconstruire le scénario 8 sur une dérivation réelle : dériver `K_P`, dériver la clé enfant, faire tourner une vraie rotation, envelopper la DK' de cette rotation, recouvrer `K_P` par dérivation avant d'ouvrir le wrap | le scénario actuel passe avec `PARENT_KEY` remplacé par n'importe quelle constante ; après correction il doit tomber |
| 3 | `CHDR-019`, `CHDR-024` | assertion structurelle sur `key_versions["2"].lines` et appel à `check_rotation(2)` dans le `Then` du scénario 7 | injecter une ligne `g1` en v2 → doit tomber ; supprimer l'appel à `rotate` → doit tomber |
| 4 | `CHDR-013`, `CHDR-014`, `CHDR-017` | fixture à deux destinataires pour le scénario 6, instantané du vecteur entier, assertions de préfixe et de cardinal | remplacer `push` par `insert(0, …)` → doit tomber ; re-sceller les lignes survivantes à l'append → doit tomber |
| 5 | `CHDR-001` | tentative de rejeu inter-versions dans le scénario 4 | retirer `key_version` de `line_aad` → le scénario doit tomber, là où il passe aujourd'hui |
| 6 | `CHDR-002`, `CHDR-027` | assertions de rejet différentielles avec contrôle positif interne, dans `corrupt_line` et `replay_line_other_node` | rendre la ligne owner inouvrable dans le `Given` → doit tomber, là où cela passe aujourd'hui |
| 7 | `CHDR-009` | faire consommer `missing_owner_must_fail` par `g2_rotation.rs` ; assertions typées sur `rotate` et `validate` | le champ du vecteur n'a aucun consommateur → le nouveau test doit exister et passer |
| 8 | `CHDR-016`, `CHDR-015` | un pas de RU-3 qui traverse une surface de grant conforme à §3.3 | grant après rotation → doit tomber sur la version de ligne |
| 9 | `CHDR-004`, `CHDR-005`, `CHDR-006`, `CHDR-010`, `CHDR-011`, `CHDR-018` | peupler les `Given` vides ; erreur I3 typée ; cardinal des tentatives ; `Then` distincts | reformuler le message I3 → ne doit **plus** faire tomber après correction |

Les lots 1 et 2 sont ceux qui portent la sécurité et doivent atterrir en
premier.

### Mise à jour du 2026-08-04 — état des lots

| Lot | État | Détail |
|---|---|---|
| 0 | **fait** | décision du propriétaire du 2026-08-03, puis lot B ; `CHDR-007` et `CHDR-012` `VERIFIED` le 2026-08-04 |
| 1 | **fait pour `CHDR-025`** | contrôle positif dans `c1_fail_closed` ; la moitié « provenance du vecteur » l'était déjà par `5be3047`. `CHDR-026` reste ouvert |
| 2 | **fait pour `CHDR-021`** | scénario 8 reconstruit sur une dérivation réelle. `CHDR-020` reste ouvert |
| 3 | **fait pour `CHDR-019`** | assertion structurelle et appel à `check_rotation(2)`. `CHDR-024` n'est pas clos par cette note : sa clôture en sous-produit est **proposée** et appartient au propriétaire |
| 4 | **fait pour `CHDR-013` et `CHDR-014`** | fixture à deux destinataires, cardinal et préfixe. `CHDR-017` reste ouvert |
| 5 | **fait** | `CHDR-001` |
| 6 | **fait pour `CHDR-002`** | contrôles positifs internes. `CHDR-027` reste ouvert |
| 7 | **fait** | `CHDR-009` |
| 8 | **re-routé** | `CHDR-016` sorti du lot A vers `g-revocation`/`d-bundle` (§9). `CHDR-015` reste ouvert |
| 9 | non commencé | `CHDR-004`, `-005`, `-006`, `-010`, `-011`, `-018` |

**La colonne « RED attendu » de ce tableau vaut mieux que la prose de §6, et
c'est consigné plutôt que tu.** Sur `CHDR-019`, le lot 3 énonçait le bon mutant —
*« injecter une ligne `g1` en v2 → doit tomber »* — pendant que §6 en énonçait un
faux. Les deux sections de cette même note se contredisaient. La contradiction
est tranchée dans l'erratum du bloc `CHDR-019` de §6, **en faveur de ce
tableau** : `ev-39f02b30` mesure ce RED, 7/1, scénario 7 seul.

## 12. Décisions requises

> **Périmé depuis le 2026-08-03, corrigé le 2026-08-04.** Les deux décisions
> demandées par cette section **ont été prises**, le 2026-08-03, par le
> propriétaire du protocole, en lecture A sur les deux
> (`features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md`).
> `CHDR-007` et `CHDR-012` ont ensuite été assignés au lot B, corrigés, et
> déclarés `VERIFIED` le 2026-08-04. Ils ne sont plus `DECISION_REQUIRED` et ne
> sont plus non assignés. La condition de blocage 1 est fermée.
>
> La section est conservée parce qu'elle énonce la question telle qu'elle se
> posait, et parce que la décision se lit mieux contre elle. **Une seule
> question ouverte demeure dans cette section : la collision d'identifiants
> `CHDR-*` de §1, qui n'est toujours pas tranchée.** À quoi cette mise à jour
> ajoute une décision de propriétaire, d'une autre nature — `CHDR-040` (§6ter),
> qui demande où sont écrites les règles que ce train applique. Elle n'est pas
> `DECISION_REQUIRED` au sens du process : c'est un finding, avec un critère de
> clôture et deux formes de texte normatif au choix.

Deux findings sont `DECISION_REQUIRED`. Aucun correcteur ne peut choisir
implicitement, et **ni l'un ni l'autre n'est assigné à un correcteur**.

Les deux posent, sous deux formes, **une seule et même question de lecture du
protocole** : un invariant que la spécification énonce à la voix passive lie-t-il
une surface vérifiante, ou décrit-il seulement une propriété d'objet ?

1. **`CHDR-007`** — P1, 1/3 réfutations. « An edition whose any header violates
   this is invalid » (`spec/03-headers.md:37`) est-il une obligation pesant sur
   `Bundle::verify` et `publication::cold_verify`, ou l'énoncé d'une propriété
   que l'architecture « fail-closed à l'écriture + validation au parse » satisfait
   déjà ? Les deux lectures, leurs fondements, leurs conséquences et leurs coûts
   sont tabulés dans le bloc `CHDR-007` de §6. Propriétaire : le propriétaire du
   protocole.
2. **`CHDR-012`** — P2, **0/3 réfutations**. La ligne owner est-elle définie par
   sa clé destinataire — `spec/01-identity-and-keys.md:23`, « owner_kex is the
   recipient key of the owner's line in every header (I3) » — ou par son label
   `to`, que `spec/03-headers.md:33-35` déclare pourtant « a routing hint only » ?
   Les deux lectures sont tabulées dans le bloc `CHDR-012` de §6. Propriétaire :
   le propriétaire du protocole. Ce finding n'a subi aucune réfutation et est
   absent de l'étalon de juillet.

**Ce qui a déjà été décidé, et ne préjuge de rien.** Le propriétaire a tranché le
2026-08-03 la seule question de *publication* : les deux findings sont publiés en
entier (§6, préambule). Cette décision lève la condition de blocage 9 ; elle ne
touche pas la condition 1, qui reste ouverte sur la sémantique.

Une troisième question, qui n'est pas un finding, est portée au même propriétaire
en §1 : la collision d'identifiants `CHDR-*` entre cette note et l'étalon publié.

## 13. Limites de la conclusion

- **Aucune commande n'a été exécutée par le rôle qui écrit cette note.** Les
  seules preuves d'exécution citées sont `ev-50caa5d6` et `ev-d6840262`, produits
  par l'orchestrateur. Toute autre affirmation de comportement est une lecture de
  code courant à `a2087f2`, et est énoncée comme telle.
- **Aucune expérience de mutation n'a été conduite par ce cycle.** Les mutants
  décrits dans les blocs de findings sont des raisonnements sur le code lu,
  proposés comme RED attendus du plan d'implémentation, **non** comme des
  résultats mesurés. Les mesures de mutation publiées par l'étalon de juillet
  sont écartées et l'une d'elles est contredite par le code courant (§8.2).
- **Aucun statut `VERIFIED` n'est posé.** L'auditeur ne clôt rien.
- **Le périmètre est la vérité sémantique des huit scénarios existants.** Aucun
  scénario nouveau n'est conçu. Ce qui touche `g-revocation`, `d-bundle`,
  `n-structural-mutations` ou `h-merkle` est signalé en §9 et n'est pas audité.
- **La conclusion publique est désormais complète.** Aucun finding n'est retenu :
  `CHDR-007` et `CHDR-012` ont été publiés en entier sur décision du propriétaire
  du 2026-08-03. Deux findings restent néanmoins `DECISION_REQUIRED` sur leur
  **sémantique**, ce qui est une limite différente : cette note expose les
  lectures concurrentes, elle n'en retient aucune.
- **La ligne `counts` du gel est erronée** (§2) ; le décompte réel est établi
  dans cette note et dans le rapport de run.
- **Les identifiants `CHDR-*` sont ambigus** tant que la collision de §1 n'est
  pas tranchée.

### Mise à jour du 2026-08-04 — trois de ces limites ont bougé

- **« Aucune expérience de mutation n'a été conduite par ce cycle »** reste vrai
  du cycle qui a écrit cette note, et **n'est plus vrai du dossier**. La revue du
  lot A a nommé douze mutants et les a tous fait exécuter par l'orchestrateur
  (`2026-08-04-r6`). Trois d'entre eux — `M3`, `M12`, et le bras « gate de
  feature » de `M10` — étaient conçus pour montrer le lot **inerte**, non pour le
  confirmer ; deux sont revenus verts et sont rapportés comme tels. C'est cette
  campagne qui a permis de découvrir que le mutant énoncé par le bloc `CHDR-019`
  était faux.
- **« Aucun statut `VERIFIED` n'est posé »** n'est plus vrai : dix findings le
  sont — `CHDR-007` et `CHDR-012` le 2026-08-04 par la revue du lot B, et les
  huit du lot A par la revue du 2026-08-04. Aucun n'a été posé par l'auditeur qui
  les avait écrits : dans les deux cas un reviewer indépendant, matériellement
  isolé, a tranché sur transcript.
- **« Toute affirmation de comportement est une lecture de code courant »** ne
  vaut plus pour les blocs de clôture : chacun cite un `evidence_id` du ledger de
  `2026-08-04-r6`. Le rôle qui écrit cette mise à jour, lui, **n'a exécuté aucune
  commande** — ni gate, ni test, ni `cargo`. Les faits établis par lecture de
  l'arbre (`grep`, `ls`, `git log`) sont signalés comme tels aux endroits où ils
  servent.

**Deux limites nouvelles, propres à cette mise à jour.**

- **Le plafond de la campagne de mutation est structurel.** Les douze mutants ont
  été conçus par le reviewer contre des énoncés de défaut qu'il avait aussi lus.
  Un auteur de mutants n'ayant pas lu les critères de clôture pourrait trouver un
  trou qu'il n'a pas trouvé. Aucun transcript ne lève cette limite, et elle est le
  plafond honnête des huit verdicts.
- **Une assertion ajoutée par le lot n'est prouvée par rien.** La boucle de
  capacité de `revoked_cannot_open` n'a été tuée par aucun des douze mutants ;
  elle est étiquetée non prouvée dans le bloc `CHDR-019` et n'est pas comptée
  parmi les assertions qui closent le finding.

## 14. Définition de terminé

- Chaque finding `OPEN` ci-dessus est soit `VERIFIED` par une revue indépendante,
  soit explicitement reporté avec un motif enregistré.
- `CHDR-007` et `CHDR-012` ont une décision **de sémantique** enregistrée avant
  qu'une correction ne les touche. La décision de publication du 2026-08-03 ne
  vaut pas décision de sémantique.
- ~~La barrière de divulgation est levée ou confirmée par le propriétaire
  humain~~ — **fait le 2026-08-03** ; la note publique a été mise à jour en
  conséquence (§6, §15).
- La collision d'identifiants avec l'étalon de juillet est tranchée.
- Chaque correction atterrit avec un test RED démontré défaillant sur la baseline
  auditée **pour la bonne raison**, et le correcteur documente les deux
  résultats.
- Le gate canonique rapporte les compteurs attendus après correction — exit code
  **et** compteurs, la règle permanente issue de `BDER-011`.
- Le correcteur exécute les régressions nommées par `DOMAIN.md`
  (`c1_header_seal`, `g2_rotation`, `g3_move`, `b2_derivation`,
  `cb10_structure_vault`, `vectors_ownership`) puis un gate Cucumber global et un
  gate workspace avant passation.
- Les marqueurs Gherkin sont retirés pour chaque finding accepté `VERIFIED`.

### Mise à jour du 2026-08-04 — ce qui est fait, ce qui ne l'est pas

- **Fait.** Les dix findings `VERIFIED` (deux du lot B, huit du lot A) portent
  chacun leur preuve différentielle et leurs `evidence_id`. Les marqueurs Gherkin
  des dix sont retirés — **par réécriture, non par suppression** : chaque bloc
  mêlait des identifiants clos et des identifiants ouverts, et le verdict de
  scénario du scénario 8 change avec `CHDR-021` au lieu de disparaître.
  `@chdr-016` survit et nomme son re-routage.
- **Fait.** Le correcteur a exécuté les régressions nommées par `DOMAIN.md`, puis
  un gate Cucumber global et un gate workspace, et documente les deux résultats
  (`ev-c2945d9b`, `ev-a1fa00fc`, `ev-3013c663`, `ev-e3b0c442`, `ev-d6ce5ee9`).
  Une réserve : ces commandes de régression n'emportent pas `--no-fail-fast` et
  peuvent donc sous-rapporter un échec — `CHDR-042`, §6ter.
- **Pas fait.** La collision d'identifiants de §1 n'est pas tranchée.
- **Pas fait.** Les findings restés `OPEN` de §6, §6bis et §6ter ne sont ni
  `VERIFIED` ni reportés avec un motif enregistré. `CHDR-016` est le seul dont le
  report porte un motif enregistré : le re-routage du 2026-08-04.
- **Une règle à ajouter, tirée de ce cycle.** Une clôture doit dire ce que le
  mutant a mesuré **et** ce qu'aucun mutant n'a mesuré. Deux assertions de ce lot
  ne sont prouvées par rien (§13), et elles sont étiquetées plutôt que comptées.

## 15. Trace de la barrière de divulgation

La barrière a réellement joué pendant ce cycle. Elle est consignée ici parce
qu'un audit qui effacerait le mécanisme l'ayant contraint ne serait pas un audit
honnête.

| Étape | Date | Fait |
|---|---|---|
| 1 | 2026-08-03 | Le Pass A marque quatre findings `disclosure: embargo` — `CHDR-003`, `CHDR-007`, `CHDR-008`, `CHDR-012` — et lève la condition de blocage 9 (`pass-a/frozen.json`, champ `note`) |
| 2 | 2026-08-03 | L'auditeur intégrateur écrit la première version de cette note : `CHDR-007` et `CHDR-012` par identifiant et titre neutre seuls ; `CHDR-003` et `CHDR-008`, retirés par la réconciliation, publiés en clair |
| 3 | 2026-08-03 | Le **gardien de process invalide le cycle** : une ligne d'impact `h-merkle` de §9, rattachée à `CHDR-007`, décrivait le mécanisme au lieu de s'en tenir à l'identifiant. Invalidation n° 1 |
| 4 | 2026-08-03 | Correction : la ligne fautive et quatre autres occurrences du même genre sont rédigées. Le gardien invalide **une seconde fois** ; la condition de blocage 6 — deux invalidations de la même feature — s'ouvre et arrête le run |
| 5 | 2026-08-03 | Le propriétaire humain tranche la publication : « Publier les deux en entier. `CHDR-007` est déjà public en substance sur `codex/audit-c-headers` ; `CHDR-012` est publié malgré l'absence de correctif, au motif que le correcteur doit pouvoir citer ce qu'il répare. » — Mathieu Colla. Condition 9 **résolue** ; condition 6 tombe avec elle, la fuite reprochée n'en étant plus une |
| 6 | 2026-08-03 | Run de reprise `2026-08-03-r2` : `CHDR-007` et `CHDR-012` sont restitués en entier dans cette note, avec le même niveau de citation que les findings jamais retenus |
| 7 | 2026-08-04 | Revue du lot A : la barrière est **repassée**, pas héritée. Un candidat trouvé (`spec/03-headers.md:39-40`, unicité des `kid`), constaté déjà publié en entier comme `CHDR-032`, donc non retenu. Aucun des cinq findings de §6ter ne décrit un chemin d'exploitation. `CHDR-028` était encore sous embargo à ce moment et n'a pas été touché. Recherches consignées en §6ter |
| 8 | 2026-08-04 | Le rôle qui écrit la présente mise à jour **re-vérifie** ce jugement au lieu d'en hériter, en relisant le bloc `CHDR-032` de §6bis dans le document publié. Confirmé : le chemin d'émission y figure déjà en clair. Un embargo sur la reformulation d'un énoncé publié ferait passer un finding publié pour un finding retenu |
| 9 | 2026-08-04 | **Le propriétaire lève l'embargo sur `CHDR-028`** : publication intégrale. L'énoncé, ses preuves et son critère de clôture sont restitués en §6bis au même niveau de citation que les findings jamais retenus. Condition 9 **résolue** pour ce finding. Il n'est assigné à personne ici : `c-headers` est `COMPLETE` et la surface visée appartient à `aithos-bundle` ; il est porté par `QUEUE.yaml` sous `chdr-028` |
| 10 | 2026-08-04 | **Ce que la levée a coûté à faillir.** Le fichier hors dépôt qui portait l'énoncé, `/root/work/EMBARGO-CHDR-028.md`, avait été détruit entre-temps par le même effacement du clone local qui a ramené l'arbre de travail à `a2087f2`. Le texte n'a survécu que parce que l'orchestrateur l'avait relu en début de session et le portait donc dans son contexte. Un embargo hors dépôt est une rétention **sans durabilité** : le dépôt est sauvegardé, poussé et répliqué, le fichier hors dépôt ne l'est pas. Deux énoncés voisins retenus par la même barrière, `SC-12` et le bord code de `SC-05`, n'ont **pas** eu cette chance et doivent être re-dérivés depuis le code. Consigné comme un défaut de la barrière elle-même, pas comme un incident |

Ce que l'épisode établit, et qui vaut au-delà de cette feature :

- **La barrière est un gate d'écriture, pas de publication.** `QUEUE.yaml:21-24`
  le dit : les branches orchestrées sont poussées au dépôt public, donc la
  rétention doit avoir lieu au moment où un agent écrit, pas au moment où un
  humain relit. Le gardien a fait exactement ce pour quoi il existe.
- **Une rétention partielle est instable.** Retenir `CHDR-007` tout en publiant
  `CHDR-008`, dont l'énoncé en est un sous-ensemble, a produit une incohérence
  interne que la seconde correction a dû résoudre en retenant les deux. Un
  périmètre d'embargo doit être fermé par absorption, pas par identifiant.
- **Un embargo posé sur une information déjà publique coûte sans protéger.**
  `CHDR-007` figurait déjà en clair sur `codex/audit-c-headers` ; la rétention
  n'a rien protégé et a seulement rendu cette note moins utile à son lecteur.
  C'est le motif que le propriétaire a retenu en premier.
- **La décision de publier n'est pas la décision de trancher.** `CHDR-007` et
  `CHDR-012` sont désormais lisibles en entier et restent `DECISION_REQUIRED` :
  la condition de blocage 1 est ouverte, et aucun correcteur ne les reçoit.
  *(Périmé depuis le 2026-08-03 : la décision de sémantique a été prise, les deux
  findings ont été corrigés et sont `VERIFIED`. Conservé parce que c'est ce que
  l'épisode a établi le jour où il s'est produit ; corrigé en §3.)*
- **La règle qui a produit tout ce tableau n'est écrite nulle part.** Ajouté le
  2026-08-04. La barrière de divulgation — condition de blocage 9 — est citée
  par huit sites et ne figure pas dans `features/.agents/PROCESS.md`. Elle vit
  dans un document de proposition non appliqué. C'est `CHDR-040` (§6ter), et
  c'est la seule règle de ce train dont la défaillance est irréversible.

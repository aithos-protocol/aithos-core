# Audit initial — `c-headers.feature`, run orchestré `2026-08-03-r1`

## 1. Identité du run

| Champ | Valeur |
|---|---|
| Type de run | audit initial, ronde 1, mode orchestré |
| Rôle | auditeur intégrateur — **A3, Pass B, passe d'état partagé et intégration** |
| Date | 2026-08-03 |
| Révision observée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` (`a2087f2`) |
| Base `main` enregistrée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` |
| Baseline / candidat | sans objet — audit initial, aucune correction n'existe (`candidate_revision: null`) |
| Branche | `codex/audit-c-headers-r2`, créée depuis `main` à `a2087f2` |
| Run orchestré | `2026-08-03-r1` — `features/.agents/orchestrator/runs/2026-08-03-r1/` |
| Périmètre | la vérité sémantique des huit scénarios existants de `features/c-headers.feature` ; quatre blocs `Rule` |
| Unités de revue | `RU-1`, `RU-2`, `RU-3`, `RU-4` (une par `Rule`) |
| Audit public écrit | `docs/audits/features/c-headers.md` |
| Étalon Pass B | `origin/codex/audit-c-headers` (`af32734`), audit manuel du 2026-07-30 |
| Statut du rapport | **direct** — non `RECONSTRUCTED` |

### État du worktree

Observé au moment de l'audit, `git status --short` :

```
 M features/.agents/c-headers/STATE.md
?? features/.agents/orchestrator/runs/2026-08-03-r1/
```

`STATE.md` a été modifié par l'orchestrateur pour geler `base_main` et
`audit_revision` ; le répertoire de run n'était pas encore suivi. **Aucun
fichier du périmètre audité** — `features/c-headers.feature`, `rust/`,
`vectors/`, `spec/` — n'était modifié. Les écritures de ce rôle sont listées en
§9.

### Limites de rôle respectées

- Aucune commande `cargo`, aucun test, aucun build n'a été lancé par ce rôle.
  Les seules preuves d'exécution citées sont `ev-50caa5d6` et `ev-d6840262`.
- Aucun statut `VERIFIED` n'est posé.
- Aucun code de production, de step ou de test n'a été modifié.
- `STATE.md`, `PROCESS.md`, `QUEUE.yaml`, `LEDGER.md` et `BLOCKED.md` n'ont pas
  été touchés.
- Aucune autre feature n'a été ouverte, fermée ou rouverte.
- `PROCESS.md` a été appliqué **tel qu'amendé** par les amendements AM-1 à AM-5
  de `docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md`, sans que ce fichier
  ni `PROCESS.md` ne soient modifiés.

## 2. Entrées

| Entrée | Chemin |
|---|---|
| Verdicts Pass A gelés et verdicts du panel, en clair | `/root/work/passA-raw/A3-input.md` — **hors dépôt**, délibérément |
| Gel rédigé | `features/.agents/orchestrator/runs/2026-08-03-r1/pass-a/frozen.json` |
| Verdicts du panel | `features/.agents/orchestrator/runs/2026-08-03-r1/pass-a/refutation.json` |
| Journal | `features/.agents/orchestrator/runs/2026-08-03-r1/ledger.jsonl` |
| Transcripts | `features/.agents/orchestrator/runs/2026-08-03-r1/evidence/` |
| Process | `features/.agents/PROCESS.md` + AM-1..AM-5 |
| Domaine, état | `features/.agents/c-headers/DOMAIN.md`, `STATE.md` |
| Politique orchestrateur | `features/.agents/orchestrator/{LEDGER.md,QUEUE.yaml}` |
| Étalon de juillet | `git show origin/codex/audit-c-headers:docs/audits/features/c-headers.md` |
| Code, spec, vecteurs, histoire git | dépôt complet avec `.git` à `a2087f2` |

## 3. Pass A — entrées, isolation, verdicts gelés, contamination

Ce rôle **n'a pas exécuté le Pass A** ; il en reçoit le gel. Le rappel qui suit
est là pour que le rapport se suffise à lui-même.

### Isolation

Matérielle, conformément à AM (`PROCESS.md` § *Material isolation of Pass A*).
Chaque unité a tourné contre un extrait `git archive` de `a2087f2` **sans
répertoire `.git`** — `ledger.jsonl`, entrées `role: extract`,
`sha256: 589fcc39c257f05a7a639845c79c5d7f9886e585841a3c2f459f8503b02bba0c`,
workspaces `passA/c-headers/RU-{1,2,3,4}`. L'agent ne s'abstient pas de lire
l'histoire : il ne le peut pas.

### Verdicts provisoires gelés

| Unité | `Rule` | Scénario | Verdict Pass A |
|---|---|---|---|
| RU-1 | A line seals the node key to exactly one recipient | 1 Owner and grantee each open their line | `PROVEN` |
| RU-1 | | 2 A non-recipient opens nothing | `PROVEN` |
| RU-1 | | 3 A corrupted line fails closed | `PARTIAL` |
| RU-1 | | 4 A line is bound to its node and version | `PARTIAL` |
| RU-2 | The owner line is mandatory (I3) | 5 A header without an owner line is invalid | `PARTIAL` |
| RU-3 | Grant is one appended line, touching nobody | 6 Granting a new reader leaves every other line untouched | `PARTIAL` |
| RU-4 | Rotation cuts the revoked and re-links the parent | 7 The revoked gets no line in the new version | `PARTIAL` |
| RU-4 | | 8 An up-link wrap restores derivation for the parent holder | `PROXY` |

### Statut de contamination du Pass A

**Aucune contamination**, pour les quatre unités
(`frozen.json`, champ `contamination: "none"` par unité ; `ledger.jsonl`,
`history_visible: false` sur chaque entrée d'agent de Pass A et de réfutation).
La condition de blocage 4 n'est pas ouverte.

Contamination de **ce** rôle : totale et assumée. Ce rôle lit l'histoire git,
l'étalon de juillet, les verdicts gelés et les verdicts du panel. C'est la
définition du Pass B (`PROCESS.md` § *Pass B*). Le gel a précédé chaque entrée
de Pass B dans le journal.

### Erreur du gel, corrigée ici

La ligne `counts` de `pass-a/frozen.json` annonce `P2: 14, P3: 9`. C'est faux.
Le décompte réel du gel, obtenu en comptant la liste `findings` du même fichier,
est **P1 = 1, P2 = 15, P3 = 8, total 24, embargo 4**. L'erreur est de comptage,
pas de contenu ; la liste `findings` est correcte et fait foi. Corrigé dans
l'audit public §2 et ici.

## 4. Panel de réfutation adverse

`PROCESS.md` § *Adversarial refutation* (AM). Avant qu'un finding ne soit remis
à un correcteur, tout finding P1 ou P2 reçoit un panel indépendant : des agents
frais recevant **l'énoncé du finding seul**, chacun chargé de le réfuter et de
répondre `refuted` en cas d'incertitude. Un finding survit sur une majorité de
non-réfutations.

| Mesure | Valeur |
|---|---|
| Findings soumis (P1/P2) | 16 |
| Réfuteurs par finding | 3 (`QUEUE.yaml:16-17`, `refuters_per_finding: 3`) |
| Agents de réfutation lancés | **48** |
| Findings survivants | 8 |
| Findings réfutés à la majorité | 8 |
| Findings P3 non soumis | 8 (le panel ne couvre que P1/P2) |

Journal : `ledger.jsonl`, entrées `role: refutation`, `panel_size: 3`,
`history_visible: false`, `inputs: ["finding statement only"]`.

### Résultats

| Finding | Réfutations | Survit | Devenir après réconciliation |
|---|---|---|---|
| `CHDR-001` | 1/3 | oui | maintenu P2, énoncé corrigé par le panel puis requalifié par le Pass B |
| `CHDR-002` | 3/3 | non | reformulé, déclassé P2 → P3 |
| `CHDR-003` | 2/3 | non | **retiré** ; embargo levé avec lui |
| `CHDR-007` | 1/3 | oui | maintenu P1, `DECISION_REQUIRED` ; publié en entier, embargo levé le 2026-08-03 (§10) |
| `CHDR-008` | 2/3 | non | **retiré** en tant que finding autonome, absorbé par `CHDR-007` |
| `CHDR-009` | 2/3 | non | reformulé, maintenu P2 |
| `CHDR-012` | **0/3** | oui | maintenu P2, `DECISION_REQUIRED` ; publié en entier, embargo levé le 2026-08-03 (§10) |
| `CHDR-013` | 1/3 | oui | maintenu P2 |
| `CHDR-014` | 2/3 | non | reformulé (clause fausse retirée), **maintenu P2** |
| `CHDR-015` | 3/3 | non | reformulé, déclassé P2 → P3 |
| `CHDR-016` | 1/3 | oui | maintenu P2 ; la réfutation « hors périmètre » écartée sur preuve de code courant |
| `CHDR-019` | 1/3 | oui | maintenu P2 |
| `CHDR-020` | 2/3 | non | reformulé, déclassé P2 → P3 |
| `CHDR-021` | 1/3 | oui | maintenu P2 ; porte le verdict du scénario 8 |
| `CHDR-022` | 1/3 | oui | **requalifié en impact `g-revocation`** — n'est plus un finding `c-headers` |
| `CHDR-023` | 3/3 | non | **requalifié hors périmètre** — durcissement défensif |

### Ce que le panel a apporté et ce qu'il a coûté

Apporté : quatre énoncés surdimensionnés corrigés (`CHDR-001`, `CHDR-009`,
`CHDR-014`, `CHDR-020`) ; deux findings retirés sur preuve
(`CHDR-003`, `CHDR-023`) ; **une trouvaille propre** que l'audit avait manquée —
`vectors/g2-rotation.json:17` déclare `"missing_owner_must_fail":
"MissingOwnerLine"` que la struct `G2` de `g2_rotation.rs:9-16` ne désérialise
pas, cas normatif sans consommateur, désormais le cœur de `CHDR-009`.

Coûté : le panel a **utilisé comme preuve** `c1_header_seal.rs:105-107` pour
imposer une correction de `CHDR-001`, alors que cette assertion est vacante
(§6, `CHDR-025`). Aucun des quarante-huit réfuteurs ne l'a vu. Un panel qui ne
reçoit que l'énoncé d'un finding cite volontiers un test sans vérifier que ce
test détecte quoi que ce soit.

Condition de blocage 5 (« majorité de panel contre l'auditeur ») : ouverte par
huit findings, **levable** — les huit sont tranchés sur preuve de code courant
en §6.

## 5. Pass B — entrées et preuves différentielles

### 5.1 Le code audité est identique à celui de l'étalon de juillet

`git diff 3803fe8 a2087f2` sur le périmètre :

| Fichier | Résultat |
|---|---|
| `features/c-headers.feature` | identique |
| `rust/crates/aithos-core/src/header.rs` | identique |
| `rust/crates/aithos-core/src/seal.rs` | identique |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | identique |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | identique |
| `rust/crates/aithos-core/tests/g3_move.rs` | identique |
| `vectors/c1-header-seal.json` | identique |
| `vectors/g2-rotation.json` | identique |
| `rust/crates/aithos-bundle/tests/cucumber.rs` | 1 fichier, +16 −3, **uniquement dans `main()`** |

Le seul écart de `cucumber.rs` est le correctif `BDER-011` :
`filter_run` → `fail_on_skipped()` + `filter_run_and_exit`, avec filtre `@wip`
aux trois niveaux. **Aucune définition de pas, aucun fixture, aucun champ du
`World`, aucun helper n'a bougé.**

Conséquence méthodologique : la comparaison à l'étalon n'a aucune excuse de
dérive. Un finding P1/P2 de juillet non retrouvé est un manqué.

### 5.2 Histoire des artefacts du périmètre

| Fichier | Commits qui l'ont touché |
|---|---|
| `features/c-headers.feature` | `168d824` (contrat @wip), `04f0eca` (étape C), `f1ab74a` (ajout du seul tag `@c-headers`) |
| `rust/crates/aithos-core/src/header.rs` | `04f0eca`, `4638a57` (étape G, révocation), `97d7187` (étape G close, move-as-rotation, `build_at`) |
| `rust/crates/aithos-core/src/seal.rs` | `04f0eca`, `626d57b` (étape D) |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | **`04f0eca` seulement** — jamais retouché depuis sa création |
| `rust/crates/aithos-core/tests/g3_move.rs` | `97d7187` |

Le fait décisif est la troisième ligne : `c1_fail_closed` n'a jamais été
retouché depuis l'étape C. Le défaut que juillet y avait relevé est donc encore
là, verbatim, et l'est resté à travers deux étapes majeures du protocole.

`f1ab74a` et `97d7187` sont tous deux des ancêtres de `240c658`, la base `main`
de l'étalon : l'histoire ne fournit aucune raison de croire que l'outillage de
test de juillet différait du courant.

### 5.3 Une revendication d'exécution de juillet contredite par le code courant

L'étalon rapporte (§4 de sa note) qu'une mutation retirant `key_version` de
`line_aad` laissait « 18 features / 114 rules / 836 scenarios / 3577 steps »
verts, et que la seule défaillance de tout le workspace était
`c1_owner_and_grantee_lines`.

Or `g3_move.rs:149-152` assère
`hex::encode(line_aad(&v.subject_did, &v.new_node, v.key_version)) ==
v.line_aad_hex` ; ce fichier est identique entre les deux révisions et son
dernier commit `97d7187` est un ancêtre de `240c658`. Cette assertion aurait dû
tomber sous la même mutation.

Ce rôle n'exécute aucune commande et ne peut donc pas trancher par mesure. Le
fait est consigné comme une **contradiction entre une revendication d'exécution
non reproduite et la lecture du code courant**, et la revendication est écartée.
`CHDR-025` est construit sans elle, sur la seule lecture du code.

### 5.4 Preuves de gate de l'étalon : sans valeur probante

La branche étalon part de `240c658`, antérieur au correctif `BDER-011` : son
`main()` appelait `filter_run`, qui rend son writer et ne quitte jamais ; sous
`harness = false` le binaire sortait `0` avec des scénarios en échec. Son
harnais Cucumber **ne pouvait pas échouer**. Aucun chiffre de gate provenant de
cette branche n'est cité, ni dans l'audit public, ni ici.

### 5.5 Accord et désaccord avec chaque verdict Pass A gelé

`PROCESS.md` : un verdict ne peut être renforcé ou renversé que par une preuve
de **code courant** nouvellement identifiée. L'intention historique seule ne
suffit jamais.

| Scénario | Pass A | Pass B | Motif, sur preuve de code courant |
|---|---|---|---|
| 1 | `PROVEN` | **accord** | `Header::build` → `build_at` → `build_lines` → `seal_line` ; deux `assert_eq!` contre `DK` avec filtre `kid` distinct. Aucune preuve nouvelle ne dégrade le verdict. `CHDR-004`, `CHDR-006`, `CHDR-027` sont des P3 de fidélité de contrat, pas des défauts d'assertion |
| 2 | `PROVEN` | **accord** | `!opened.is_empty()` + `all(is_err)` ; sous le code courant les deux kids littéraux couvrent exactement les deux lignes du fixture. `CHDR-005` est un défaut de force de preuve, pas un défaut vivant |
| 3 | `PARTIAL` | **accord sur le statut, désaccord sur le motif** | le motif gelé (attribution de cause) est réfuté 3/3 et retiré ; le motif retenu est l'absence de contrôle positif interne, vérifiée en `cucumber.rs:7553-7566`, `:8104-8112`, `:12342-12345` |
| 4 | `PARTIAL` | **accord, renforcé** | `replay_line_other_node` (`:8114-8122`) : les deux `build` retombent à la version 1 (`header.rs:114-116`) et l'ouverture se fait en version 1 ; preuve nouvelle : le défenseur hors Gherkin cité par le panel est vacant (`CHDR-025`) |
| 5 | `PARTIAL` | **accord** | un seul des quatre portails I3 est exercé côté fail-closed ; `vectors/g2-rotation.json:17` n'a aucun consommateur ; `header_invalid` (`:12347-12351`) n'assère qu'une sous-chaîne. L'étalon de juillet classait ce scénario `PROVEN` ; ce Pass B ne suit pas, sur la preuve du vecteur non consommé |
| 6 | `PARTIAL` | **accord** | `sealed_header_owner_only` (`:7569-7573`) scelle un seul destinataire ; `owner_line_untouched` (`:12353-12361`) ne lit ni cardinal ni index ; `Bundle::grant` appende à `KV = 1` |
| 7 | `PARTIAL` | **accord** | `revoked_cannot_open` (`:12375-12383`) : le filtre `kid` (`header.rs:233`) rend la boucle vide, `open_line` n'est jamais appelé, le secret est inutilisé ; `key_versions["2"].lines` n'est lu nulle part |
| 8 | `PROXY` | **désaccord — requalifié `SEMANTIC_FALSE_POSITIVE`** | motivé ci-dessous |

### 5.6 Le désaccord sur le scénario 8

`PROXY` désigne « a scenario that consumes a shared verdict without executing
its own case ». Le scénario 8 exécute son propre cas : `post_uplink_wrap`
(`:8164-8175`) construit un `Wrap` réel, `parent_recovers_via_wrap`
(`:12396-12404`) l'ouvre réellement. Aucun verdict partagé n'est consommé.

Ce qu'il fait, c'est passer sans prouver ce qu'il énonce — la définition de
`SEMANTIC_FALSE_POSITIVE`. Preuves de code courant :

- `derived_node_rotated` (`:7598-7601`) a un corps vide ; `w.header` reste
  `None` pendant tout le scénario ;
- `PARENT_KEY` (`:265`) n'est la sortie d'aucun `node_key`, n'est ouverte
  d'aucune ligne, n'est la clé d'aucun nœud ; `DK2` (`:264`) n'est produite par
  aucune rotation ici ;
- `Wrap::open` (`header.rs:351-353`) recalcule l'AAD depuis ses **propres**
  champs `self.node` et `self.key_version` : le `Then` ne peut pas détecter un
  wrap posté sous le mauvais nœud ni sous la mauvaise version ;
- `via` (`header.rs:344`) n'entre pas dans `wrap_aad` (`seal.rs:41-43`) : il est
  stocké, lu par personne.

Ce qui est établi est exactement `wrap_open(wrap_seal(k, dk)) == dk`.

**Cohérence des deux verdicts opposés du panel sur ce scénario.** `CHDR-020`
(réfuté 2/3) et `CHDR-021` (survit) visent tous deux le scénario 8. Le Pass B
les rend complémentaires plutôt qu'opposés : le `Given` vide est le
**mécanisme**, le `Then` en aller-retour est la **conséquence**. `CHDR-020` est
déclassé en P3 comme finding de fidélité de contrat ; `CHDR-021` est maintenu à
P2 et porte le verdict de scénario. Le verdict `PROXY` du gel est remplacé, non
pas parce que le panel l'a demandé — il n'a jamais été saisi du verdict de
scénario — mais parce que la définition du tableau des statuts ne s'y applique
pas.

## 6. Réconciliation des huit findings réfutés

`PROCESS.md` § *Adversarial refutation* : « A finding refuted by a majority
returns to the auditor as an open question, not as a closed case. A disagreement
the auditor cannot settle with current-code evidence is a blocking condition. »

**Les huit sont tranchés sur preuve de code courant. Aucun ne reste
indécidable.** Détail complet dans l'audit public §6 et §7 ; synthèse ici.

| Finding | Décision | Preuve de code courant décisive |
|---|---|---|
| `CHDR-002` | **reformulé, déclassé P3** | moitié « cause » retirée : les cinq sorties de `Header::open` (`header.rs:232`, `:234`, `:235`, `:237`, `:242`) sont toutes `SealRejected` et `open_into` (`:7402`) stringifie ; moitié « contrôle positif » retenue : ni `:7553-7566` ni `:7569-7573` n'ouvrent avant le `When`, donc une ligne owner rendue inouvrable laisserait les scénarios 3 et 4 verts |
| `CHDR-003` | **retiré ; embargo levé** | `hdr_file` = `e/<zone>/hdr/blake3(node)[..12].json` (`grants.rs:139-146`) ; `open_blob_v` calcule `blob_aad` depuis le `NodePath` de l'appelant, jamais depuis `header.node` (`bundle.rs:504-518`, cf. `:492`) ; `vault_build`/`header_hash_at` indexent le hash par chemin (`state.rs:240-248`, `:58-62`). Aucune conséquence de sécurité ne subsiste |
| `CHDR-008` | **retiré comme finding autonome, absorbé par `CHDR-007`** | base factuelle confirmée : cinq sites `.validate()` (`bundle.rs:630`, `:637`, `log.rs:425`, `session.rs:363`, `aithos-cli/src/cmd/header_open.rs:28`) contre une douzaine de sites de désérialisation de `Header` (`grants.rs:287`, `:456`, `:827`, `:1037`, `:1197`, `structure.rs:199`, `:751`, `revoke.rs:289`, `:365`, `:510`, `bundle.rs:670`) ; `append_line` (`header.rs:159-188`) ne refait pas `check_owner_line` ; le trou réel se réduit à `add_line_on` (`grants.rs:287-291`). L'énoncé est un sous-ensemble strict de `CHDR-007` et dédoublerait la même décision humaine |
| `CHDR-009` | **reformulé, maintenu P2** | réfutation acceptée : les portails 2-4 sont exécutés (`:8148`, `:15249`, `g2_rotation.rs:92`, cinq sites de `validate`). Énoncé retenu : `vectors/g2-rotation.json:17` déclare `missing_owner_must_fail` que la struct `G2` (`g2_rotation.rs:9-16`) ne désérialise pas — vérifié champ par champ ; le champ frère `smuggled_must_fail` (`:16`) est, lui, consommé (`:68-80`) |
| `CHDR-014` | **reformulé, maintenu P2** | clause fausse retirée : il existe bien deux fixtures multi-destinataires (`:7553` câblé à `c-headers.feature:17` et `:22` ; `:7579` câblé à `:49`). Noyau confirmé : le `Given` du scénario 6 est `sealed_header_owner_only` (`:7569-7573`), un seul destinataire, donc « toute autre ligne » a le cardinal 1 ; `KeyVersion.lines` est `pub` et l'invariant n'est qu'un commentaire (`header.rs:157-158`) |
| `CHDR-015` | **reformulé, déclassé P3** | réfutation acceptée : `append_line` ne détient aucun secret X25519 (`header.rs:159-188`), la frontière est documentée (`session.rs:352-353`), l'étape 1 est exercée à sa couche (`session.rs:364-365`, `grants.rs:459-460`, `bundle.rs:631`/`:638`), et `assert_eq!(dk, DK)` compare à la constante de module `:263`. Résidu retenu : observation de couverture de la `Rule` |
| `CHDR-020` | **reformulé, déclassé P3** | réfutation acceptée et vérifiée : `PARENT_KEY`/`DK2`/`CHILD_NODE`/version 2 sont exactement `vectors/g2-rotation.json:19-23`. Nuance ajoutée : `:21` (`nonce_hex = 7777…`) et `:24` (`subject_did`) divergent du scénario, donc l'alignement est partiel et n'en fait pas un contrôle de conformité. Retenu : `Given` vide, contrat inexécutable |
| `CHDR-023` | **requalifié hors périmètre** | les deux cas sont inatteignables : `revoke.rs:196-197` et `vault.rs:389-390` construisent des cardinalités égales par `survivors.iter().map(…)` ; `revoke.rs:156-157` et `vault.rs:387-388` calculent `latest_version() + 1` ; `check_rotation` suit immédiatement (`revoke.rs:199`, `vault.rs:400`). Et aucun scénario n'énonce la propriété → `PROCESS.md` § *Current scope*, exclusion |

### Les deux points de vigilance nommés

**`CHDR-014` recoupe `CHDR-010` de juillet, qui avait survécu à la passe adverse
de juillet.** Instruit sérieusement, comme demandé. Le code est byte-identique
entre les deux révisions (§5.1) : le finding de juillet porte donc sur
exactement le code courant. Les deux réfutations de cette ronde attaquent des
propositions **annexes** — le câblage des fixtures (faux, corrigé) et la
couverture ailleurs (`cb10_structure_vault.rs:307`/`:334`/`:355`,
`cb9_delegated_content.rs:439`, jamais byte-identique, plafonnée à une ligne de
grantee préexistante, ce que le troisième réfuteur concède). Aucune n'atteint la
proposition centrale, laquelle est vérifiée : le `Given` scelle à un seul
destinataire. `PROCESS.md` § *Evidence hierarchy* point 1 donne le contrat au
scénario ; une couverture ailleurs ne fait pas qu'un scénario prouve sa phrase.
**Maintenu à P2.** Deux passes adverses indépendantes, à un mois d'écart, sur le
même code, aboutissent donc à des verdicts opposés — et c'est la passe de
juillet qui avait raison.

**`CHDR-020` (réfuté) et `CHDR-021` (survivant) visent le même scénario 8 ; le
verdict `PROXY` doit être reconsidéré à la lumière des deux.** Fait : §5.6. Le
verdict devient `SEMANTIC_FALSE_POSITIVE`, `CHDR-021` le porte, `CHDR-020` est
déclassé en P3 comme mécanisme.

### Écart supplémentaire tranché, non demandé

`CHDR-022` **survivait** au panel (1/3) et est néanmoins requalifié en impact.
`NodePath::zone_root(Zone::Circle)` rend `/e/circle` (`path.rs:59-65`,
`:135-147`, `:20-26`) — c'est exactement `NODE_A` — et `CHILD_NODE` est de
profondeur 1, donc son parent direct **est** la racine de zone. À sa propre
profondeur, le scénario modélise précisément ce que poste `rotate_folder`
(`revoke.rs:204-214`). Le défaut réel — `depth == 0` requis pour lire un wrap de
rotation (`grants.rs:1061-1070`), arrêt au premier header ouvrable
(`:1080-1082`) — ne se manifeste qu'à profondeur ≥ 2 et vit entièrement dans
`aithos-bundle`. `DOMAIN.md` § *Pilot limits* est explicite : ce qui touche
`g-revocation` est un impact à signaler, pas un finding à auditer. Requalifié.

### Écart inverse, tranché contre le panel

`CHDR-016` : la réfutation « hors périmètre, dette assumée de `g-revocation` »
s'appuie sur le commentaire de `bundle.rs:25` — « single key version **until step
G** (revocation rotates) ». Le Pass B l'écarte sur preuve de code courant :
**l'étape G a livré**. `revoke.rs` existe, `rotate_folder` (`:142-240`) tourne,
`Header::build_at` existe (`header.rs:124-155`), `grants.rs:1054-1070` lit déjà
les wraps de rotation. La condition suspensive du commentaire est échue et
`KV = 1` est resté. Une dette dont l'échéance est passée n'est plus une dette
assumée. **Maintenu P2**, avec impact signalé à `g-revocation` et `d-bundle`.

## 7. Passe d'état partagé

Obligatoire, `PROCESS.md` § *Review-unit isolation*, point 5. Aucune unité de
Pass A ne l'a faite : c'est l'apport propre de ce rôle.

### 7.1 Fonctions de step partagées

| Fonction | Ligne | Phrases | `Rule` | Conséquence |
|---|---|---|---|---|
| `sealed_header_owner_only` | `:7569` | 2 `#[given]` — `c-headers.feature:27`, `:41` | RU-1 **et** RU-3 | écrit deux champs du `World`, `saved_line` et `header` ; le scénario 4 reçoit un instantané `saved_line` qu'il ne lit jamais → `CHDR-027` |
| `grantee_opens` | `:12324` | 2 `#[then]` — `:14`, `:43` | RU-1 **et** RU-3 | version 1, kid `g1`, secret `xsk(0x21)`, attendu `DK` en dur ; le mot « new » de la seconde phrase n'a aucun correspondant → `CHDR-018` |
| `opening_rejected` | `:12342` | 2 `#[then]` — `:24`, `:29` | RU-1 | `assert!(w.opened.last().unwrap().is_err())` sans contrôle positif → `CHDR-002` |
| `sealed_header_owner_grantee` | `:7553` | `#[given]` de `:17` et `:22`, **et corps du `When`** `seal_into_header` (`:8092`) de `:12` | RU-1 | trois des quatre scénarios de RU-1 partagent un unique constructeur → `CHDR-004`, `CHDR-027` |

### 7.2 Champs du `World` écrits par un scénario et lus par un autre

`ProtocolWorld` (`cucumber.rs:459-461`) dérive `Debug, Default, World`. Le
harnais construit un `World` **neuf par scénario** : `header` (`:486`),
`saved_line` (`:487`), `opened` (`:488`), `wrap_obj` (`:489`) et `rejection`
(`:463`) ne traversent **aucune** frontière de scénario. Vérifié.

`rejection` est néanmoins un champ partagé par tout le fichier de 19 700
lignes : écrit en `:7796` et `:8134`, lu en `:12348` et `:12513`. Aujourd'hui
sans risque, puisqu'un seul écrivain est atteignable par scénario. Combiné à
`CHDR-011` (assertion par sous-chaîne `msg.contains("I3")`), il devient
discriminant-par-accident le jour où un `Given` de `c-headers` écrirait
`rejection`. Consigné dans l'audit public sous `CHDR-011`.

`ProtocolWorld::open_into(version, kid, sk_byte)` (`:7396-7404`) : **trois** sites
d'appel dans tout le fichier, tous dans `c-headers` — `:8099` (boucle du
scénario 2), `:8110`, `:8120`. `opened` s'accumule au sein d'un scénario et
`opening_rejected` lit `.last()` ; avec au plus une poussée par scénario de
rejet, aucune lecture d'un résultat étranger n'est possible. Le helper stringifie
l'erreur (`.map_err(|e| e.to_string())`), ce qui détruit la variante typée à la
frontière — élément retenu par la réfutation unanime de `CHDR-002`.

### 7.3 Ordre d'exécution, instanciation, `fail_on_skipped`, `filter_run_and_exit`

`main()` (`:19724-19746`) :

```
ProtocolWorld::cucumber()
    .fail_on_skipped()
    .filter_run_and_exit(features, |feature, rule, scenario| {
        !feature.tags.iter().any(|t| t == "wip")
            && rule.is_none_or(|r| !r.tags.iter().any(|t| t == "wip"))
            && !scenario.tags.iter().any(|t| t == "wip")
    })
```

- `fail_on_skipped()` : une phrase de step non résolue devient une erreur, non
  un saut silencieux ;
- `filter_run_and_exit` : le code de sortie propage l'échec — c'est le correctif
  `BDER-011`, et c'est ce qui rend `exit 0` probant pour `ev-50caa5d6` là où il
  ne l'était pas en juillet ;
- le filtre ne retire que `@wip`, aux trois niveaux. Aucun scénario de
  `c-headers` n'est tagué `@wip` : les huit sont sélectionnés, ce que confirment
  les compteurs de `ev-50caa5d6` (8 scénarios, 28 steps).

Les marqueurs d'audit posés par ce rôle (§9) sont des tags `@audit-*` et
`@chdr-*` : le filtre ne les regarde pas. Le contrat reste à 4 `Rule`,
8 `Scenario`, 28 pas — compté sur le fichier après édition.

### 7.4 `OnceLock`, caches, `static`, `lazy`, hooks

Recensement exhaustif du dépôt sur les chemins de header : **huit `OnceLock`**,
tous dans `cucumber.rs:1100-1110` — `CB4_ACCEPTANCE`,
`CB5_CONSTRAINTS_ACCEPTANCE`, `CB5_COUNTS_ACCEPTANCE`, `CB5_RECEIPTS_ACCEPTANCE`,
`CB5_CATALOG_ACCEPTANCE`, `CB6_ACCEPTANCE`, `CB7_ACCEPTANCE`, `CB10_ACCEPTANCE`.
Lus exclusivement en `:7269-7330`, via `get_or_init`, tous dans des pas
`cb*_result`.

**Aucun pas de `c-headers` ne les touche.** Aucun `lazy_static`, `once_cell`,
`static mut` ou `thread_local` n'existe dans `aithos-core/src/`,
`aithos-bundle/src/` ni ailleurs dans `cucumber.rs`. Aucun hook `Before`/`After`
n'est déclaré. Le gate filtré par `--tags @c-headers` n'initialise donc aucun
cache global, et son résultat ne dépend pas de l'ordre des features. Résultat
négatif, vérifié.

### 7.5 Surfaces publiques de `DOMAIN.md` — l'une contourne-t-elle le verdict ?

| Surface | Contourne le verdict ? | Constat |
|---|---|---|
| `Bundle::grant` → `deliver_entry` → `add_line_on` (`grants.rs:739`, `:754`, `:276-305`) | **oui** | DK par dérivation pure `node_key(&zone_dk, &node)` (`:321`), append à `KV = 1` (`:289`, `bundle.rs:25`) → `CHDR-016` |
| `Bundle::verify` (`bundle.rs:1654-1769`) et `publication::cold_verify` (`publication.rs:836-939`) | **oui** | les deux vérificateurs d'édition ; aucune occurrence de `Header` ni de `validate` dans le corps de `verify`, vérifié exhaustivement → `CHDR-007` |
| `aithos-cli` `header_seal` (`:30-56`) | **oui** | accepte `label:kid:x25519_pub_hex` libre et construit `Recipient { to: label, … }` sans contrainte sur `label` → `CHDR-012` |
| `Session::append_header_recipient` (`session.rs:354-366`) | non | conforme à §3.3 : `validate`, `open_latest`, `append_line`. **Touchée par aucun pas de la `Rule`** → `CHDR-015` |
| `deliver_connector_line` (`grants.rs:454-461`) | non | conforme : `latest_version()`. Touchée par aucun pas |
| `aithos-cli` `header_open` (`:27-32`) | non | `validate` puis `open` |
| `Bundle` read path (`bundle.rs:630`, `:637`, `:673`) | non | `validate` avant `open` |
| `revoke.rs` / `vault.rs` rotation (`:199`, `:400`) | non | `check_rotation` fail-closed après `rotate` |
| `log.rs:425`, `:446` | non | `validate` sur le fichier relu |
| `structure.rs` (`:201`, `:332-341`, `:757`, `:777`, `:788`) | non pour le verdict de cette feature | `build_at`, `open_latest` |
| `aithos-wasm` (`src/lib.rs`) | **sans objet** | **zéro** occurrence de `Header`, `Wrap` ou `seal` : aucune surface header n'est exposée. Vérifié |

### 7.6 Nouveaux findings issus de la passe d'état partagé

Numérotés à partir de `CHDR-025`, comme demandé.

- **`CHDR-025` — P2.** La liaison `key_version` du sceau de ligne n'a aucun
  défenseur comportemental dans le dépôt. `c1_fail_closed`
  (`c1_header_seal.rs:82-107`) n'a **aucun contrôle positif dans son propre
  corps** : le triplet `(sk, epk, c, n)` vient du vecteur et sa lisibilité
  nominale n'est établie que dans une autre fonction de test (`:76-80`). Toute
  mutation de `line_aad` change l'AAD des deux côtés : l'assertion passe pour une
  raison différente de celle qu'elle nomme. Il ne reste alors que des épinglages
  d'octets — `c1_header_seal.rs:66-70` et `g3_move.rs:149-152` — et le premier
  repose sur un vecteur **dont le générateur n'existe pas** : `vectors/` contient
  vingt-huit `gen-*.py` et **aucun `gen-c1*`**, alors que
  `c1_header_seal.rs:2-3` revendique une génération indépendante. C'est
  exactement l'obligation `TARGETED` déjà enregistrée par la revue d'impact
  `b-derivation` ronde 2 ; cette note en établit la conséquence de sécurité.
  Recoupe `CHDR-016` de juillet et le renforce.
- **`CHDR-026` — P3.** Aucun négatif du wrap par AAD divergente n'existe dans le
  dépôt. Recensement exhaustif des sites `wrap_open`/`Wrap::open` :
  `c1_header_seal.rs:117-119` (aller-retour), `:122` (clé via nulle — **seul**
  négatif du wrap), `g2_rotation.rs:112-116`, `g3_move.rs:157-176`,
  `cucumber.rs:12401`, `grants.rs:1054`/`:1063`. `wrap_aad` est épinglé octet à
  octet mais jamais mis en défaut sur le nœud ni sur la version, alors que le
  sceau de ligne dispose des deux axes. Asymétrie non intentionnelle.
- **`CHDR-027` — P3.** Couplage de fixture de RU-1. Trois des quatre scénarios
  partagent `sealed_header_owner_grantee` (`:7553`) ; le quatrième utilise
  `sealed_header_owner_only` (`:7569`), lui-même partagé avec RU-3. La `Rule` ne
  comporte que deux formes d'appel à `Header::build` et **un seul contrôle
  positif**, `owner_opens` (`:12312`), situé dans le scénario 1. Les scénarios 3
  et 4 empruntent donc leur pouvoir de détection à un scénario voisin. Ce n'est
  pas un `PROXY` — aucun verdict n'est consommé — mais c'est précisément le type
  de couplage que l'isolation par unité ne peut pas voir.

## 8. Commandes exactes et résultats

**Aucune commande n'a été exécutée par ce rôle.** En mode orchestré, l'orchestrateur
seul exécute les gates, écrit les transcripts, les hache et enregistre un
`evidence_id` (`PROCESS.md` § *Orchestrated gate execution*, AM). La propriété
du gate ne bouge pas : ce rôle en répond, il ne l'exécute pas.

| `evidence_id` | Commande exacte | Rev | Exit | Compteurs `[Summary]` | Transcript |
|---|---|---|---|---|---|
| `ev-50caa5d6` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers` | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` | 0 | 1 feature / 4 rules / 8 scénarios (8 passés) / 28 steps (28 passés) | `runs/2026-08-03-r1/evidence/ev-50caa5d6.txt`, sha256 `50caa5d6…b077be` |
| `ev-c30fa81e` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers` | `a2087f2-plus-markers` | 0 | 1 feature / 4 rules / 8 scénarios (8 passés) / 28 steps (28 passés) | `runs/2026-08-03-r1/evidence/ev-c30fa81e.txt`, sha256 `c30fa81e…f9060a` |
| `ev-d6840262` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @a-identity` | `a2087f2` | 0 | 1 feature / 8 rules / 30 scénarios (30 passés) / 93 steps (93 passés) | `runs/2026-08-03-r1/evidence/ev-d6840262.txt`, sha256 `d6840262…7fe70f` — gate de préchauffage, hors périmètre |

Les trois entrées figurent au journal (`ledger.jsonl`, `kind: "gate"`,
`green: true`, exit **et** compteurs enregistrés — la sonde permanente contre une
régression de classe `BDER-011`).

Les compteurs de `ev-50caa5d6` correspondent exactement au fichier de contrat
(4 `Rule`, 8 `Scenario`, 28 pas) : c'est la preuve de sélection et d'exécution
sur la révision auditée, avant toute écriture de ce rôle.

**Les marqueurs d'audit n'ont pas altéré la sélection.** `ev-c30fa81e` a été
exécuté par l'orchestrateur sur l'arbre portant les marqueurs posés en §12 et
rapporte exactement les mêmes compteurs que `ev-50caa5d6` : 1 feature / 4 rules
/ 8 scénarios (8 passés) / 28 steps (28 passés), exit 0. C'est à ce titre, et à
ce seul titre, que cette preuve est citée ici — les tags posés ne sont pas
`@wip` et ne modifient aucune phrase de scénario.

### Anomalie de gate `ev-bc752da6` — documentée, non probante

Une preuve de gate intermédiaire du journal, `ev-bc752da6`
(`rev: a2087f2+markers`, exit 0), rapporte **7 features / 28 rules / 56
scénarios (56 passés) / 196 steps (196 passés)** — soit exactement sept fois les
compteurs attendus. Cause établie par l'orchestrateur et consignée au journal
(`ledger.jsonl`, `agent_id: note-ev-bc752da6`, `status: anomaly-explained`) :
les extraits de Pass A créés par `train.py extract` vivent sous `features/`, et
le runner Cucumber scanne ce répertoire récursivement ; les six copies de
`c-headers.feature` portées par les extraits ont donc été sélectionnées en plus
de l'originale. **Défaut d'orchestration, non de harnais.** Les extraits ont été
purgés et le gate rejoué, ce qui a produit `ev-c30fa81e`.

`ev-bc752da6` n'est cité ici que pour documenter l'anomalie et sa cause ; il
n'est utilisé comme preuve de rien. `ev-50caa5d6` n'est pas concerné : il
précède la création du premier extrait. Ce rôle n'a exécuté aucun de ces gates
et ne se prononce pas au-delà de ce que le journal enregistre.

**Aucune autre affirmation d'exécution n'est faite.** Tout le reste de ce
rapport est une lecture du code courant à `a2087f2`, énoncée comme telle.
Aucune expérience de mutation n'a été conduite : les mutants décrits sont des
raisonnements sur le code lu, proposés comme RED attendus du plan
d'implémentation, non comme des résultats mesurés.

## 9. Findings traités et non traités

### Décompte final

| Sévérité | Actifs | Identifiants |
|---|---|---|
| P1 | 1 | `CHDR-007` |
| P2 | 9 | `CHDR-001`, `CHDR-009`, `CHDR-012`, `CHDR-013`, `CHDR-014`, `CHDR-016`, `CHDR-019`, `CHDR-021`, `CHDR-025` |
| P3 | 13 | `CHDR-002`, `CHDR-004`, `CHDR-005`, `CHDR-006`, `CHDR-010`, `CHDR-011`, `CHDR-015`, `CHDR-017`, `CHDR-018`, `CHDR-020`, `CHDR-024`, `CHDR-026`, `CHDR-027` |
| **Total actif** | **23** | |
| Retirés | 2 | `CHDR-003`, `CHDR-008` |
| Requalifiés hors périmètre | 2 | `CHDR-022` (impact `g-revocation`), `CHDR-023` (durcissement défensif) |
| **Total d'identifiants** | **27** | `CHDR-001` … `CHDR-027` |

Mouvements par rapport au gel (P1=1, P2=15, P3=8, total 24) : trois findings
déclassés P2 → P3 (`CHDR-002`, `CHDR-015`, `CHDR-020`), quatre sortis (deux
retirés, deux requalifiés), trois créés au Pass B / passe d'état partagé
(`CHDR-025` P2, `CHDR-026` P3, `CHDR-027` P3).

### `DECISION_REQUIRED`

`CHDR-007` (P1) et `CHDR-012` (P2). Deux sémantiques de protocole concurrentes
chacun ; les deux lectures et leurs preuves sont documentées sous barrière de
divulgation. **Aucun correcteur ne choisit implicitement.** Propriétaire attendu
pour les deux : le propriétaire du protocole.

### Non traités, délibérément

- Tout ce qui touche `g-revocation`, `d-bundle`, `n-structural-mutations`,
  `h-merkle` : **impact signalé**, jamais audité. Table complète dans l'audit
  public §9.
- La faiblesse de `check_rotation` elle-même — inclusion au lieu d'égalité,
  `header.rs:288-297` contre `spec/03-headers.md:93-96` — est consignée sous
  `CHDR-024` **hors verdict** : aucun scénario de `c-headers` ne l'énonce. Déjà
  connue du dépôt (`docs/proposals/header-rotation-authority.md:37-48`, statut
  *Proposé — non adopté*).
- Aucun scénario nouveau n'est conçu.

## 10. Barrière de divulgation — état

`aithos-core` est public et cette branche y sera poussée. `QUEUE.yaml:21-24`
est explicite : « Branches are pushed to the public repository; the disclosure
gate is therefore a **write** gate, not a publication gate. »

| Finding | Embargo au gel | État final | Motif |
|---|---|---|---|
| `CHDR-003` | oui | **levé** | finding retiré (§6) ; l'embargo tombe avec lui ; énoncé complet publié dans l'audit public §7 |
| `CHDR-007` | oui | **levé — décision du propriétaire, 2026-08-03** | publié en entier dans l'audit public §6 ; reste P1, `DECISION_REQUIRED` |
| `CHDR-008` | oui | **levé** | finding retiré comme entité autonome ; sa matière est publiée sous `CHDR-007`, dont l'embargo est levé lui aussi |
| `CHDR-012` | oui | **levé — décision du propriétaire, 2026-08-03** | publié en entier dans l'audit public §6 ; reste P2, `DECISION_REQUIRED`, 0/3 réfutation |

### Chronologie — la barrière a réellement contraint ce run

1. Le Pass A marque quatre findings `disclosure: embargo` et lève la condition 9
   (`pass-a/frozen.json`, champ `note`).
2. Ce rôle écrit la première version de l'audit avec `CHDR-007` et `CHDR-012`
   réduits à leur identifiant et à un titre neutre.
3. **Le gardien de process (G2) invalide le cycle** : une ligne d'impact
   `h-merkle` de l'audit §9, rattachée à `CHDR-007`, décrivait le mécanisme.
   Invalidation n° 1.
4. Ce rôle corrige la ligne fautive et quatre autres occurrences du même genre.
   **Le gardien invalide une seconde fois** ; la condition de blocage 6 — deux
   invalidations sur la même feature — s'ouvre et arrête le run.
5. Le propriétaire humain tranche la publication.
6. Run de reprise `2026-08-03-r2` : les deux findings sont restitués en entier.

### Décision du propriétaire — citée textuellement

> « Publier les deux en entier. `CHDR-007` est déjà public en substance sur
> `codex/audit-c-headers` ; `CHDR-012` est publié malgré l'absence de correctif,
> au motif que le correcteur doit pouvoir citer ce qu'il répare. »
>
> — Mathieu Colla, propriétaire du protocole, 2026-08-03.

**Condition de blocage 9 : résolue.** La condition 6 tombe avec elle : la fuite
reprochée n'en est plus une, puisqu'il n'y a plus rien à retenir. Aucun fichier
suivi écrit par ce rôle ne contient plus de formule de rétention.

### Ce que la décision ne tranche pas

Elle porte sur la **publication**, pas sur la **sémantique**. `CHDR-007` et
`CHDR-012` restent `DECISION_REQUIRED` ; la condition de blocage 1 reste ouverte ;
aucun des deux n'est assigné à un correcteur (§15). L'audit public expose les
lectures concurrentes de chacun sans en retenir aucune.

### Le fait qui a pesé pour `CHDR-007`, consigné

L'étalon manuel de juillet publiait déjà ce constat en clair sur la branche
publique `codex/audit-c-headers` (`af32734`), sous
`CHDR-015 — I3 is not enforced at the edition level — DECISION_REQUIRED, P2` :
il nomme le mécanisme, les fonctions concernées et les chemins de lecture non
validants. L'embargo demandé par le Pass A portait donc sur une information déjà
publiée — son effet protecteur était nul, et son seul effet réel était de rendre
l'audit de cette ronde incomplet pour un lecteur de bonne foi. C'est le motif que
le propriétaire a retenu en premier. Le même raisonnement **ne s'appliquait pas**
à `CHDR-012`, absent de l'étalon et sans précédent public : il est publié sur un
motif distinct, énoncé par le propriétaire — permettre au correcteur de citer ce
qu'il répare.

### Enseignements de méthode, pour les cycles suivants

- **La barrière est un gate d'écriture, pas de publication** (`QUEUE.yaml:21-24`).
  Le gardien a fait exactement ce pour quoi il existe, deux fois.
- **Une rétention partielle est instable.** Retenir `CHDR-007` tout en publiant
  `CHDR-008`, dont l'énoncé en est un sous-ensemble strict, a produit une
  incohérence interne qu'il a fallu résoudre en retenant les deux. Un périmètre
  d'embargo doit être fermé par absorption, pas par identifiant.
- **Un embargo sur une information déjà publique coûte sans protéger.**

## 11. Comparaison à l'étalon de juillet

Table complète, tables de correspondance et chiffres bruts : audit public §8.
Résumé ici, car c'est le jalon de vérité du chantier.

### Ce que juillet avait

Seize findings `CHDR-001`…`CHDR-016` — **espace de noms distinct** de celui de
cette ronde (§13) — dont neuf P1/P2 : `CHDR-001` (P1), `CHDR-002` (P1),
`CHDR-003`, `CHDR-004`, `CHDR-006`, `CHDR-007`, `CHDR-010`, `CHDR-015`,
`CHDR-016` (P2).

### Résultat de la comparaison

| Mesure | Valeur |
|---|---|
| Findings P1/P2 de juillet retrouvés **seuls** par le Pass A de cette ronde | **6 sur 9** |
| Retrouvés au Pass B seulement, en lisant l'étalon | 1 sur 9 — `CHDR-016` de juillet → `CHDR-025` |
| Retrouvés partiellement | 1 sur 9 — `CHDR-007` de juillet → `CHDR-002`, dont seule la moitié « cause » avait été vue, et elle est fausse |
| **Non retrouvés du tout** | **1 sur 9** — `CHDR-004` de juillet |
| Verdicts de scénario identiques | 7 sur 8 (le scénario 5 : `PROVEN` en juillet, `PARTIAL` ici) |
| Findings nouveaux de rang P2, absents de juillet | 4 — `CHDR-012`, `CHDR-016`, `CHDR-009` (formulation panel), `CHDR-013` (P3 en juillet) |

### Les deux manqués, sans atténuation

**`CHDR-004` de juillet — jamais retrouvé, à aucun stade de cette ronde.**
`revoked_cannot_open` (`cucumber.rs:12375-12383`) n'assère que `is_err()`. Si le
`When` (`:8148`) était supprimé ou neutralisé, `key_versions` ne porterait aucune
clé « 2 » et `Header::open` renverrait `Error::SealRejected("no key version 2")`
en `header.rs:229-232` — **et ce `Then` passerait encore**. Il n'est protégé que
par ses deux `Then` frères, qui font `unwrap()` sur la version 2. `CHDR-019` de
cette ronde décrit la branche `:242-245` (boucle de kids vide) et **pas** la
branche `:229-232` (version absente). Vérifié sur le code courant. Le manqué est
réel et distinct ; il est absorbé par le critère de clôture de `CHDR-019`, mais
ce cycle ne l'a pas trouvé.

**`CHDR-016` de juillet — manqué au Pass A, y compris par les 48 réfuteurs.**
Pire que manqué : le panel a **utilisé** `c1_header_seal.rs:105-107` comme preuve
de code courant pour imposer une correction à `CHDR-001`, s'appuyant sur le test
même que juillet avait montré vacant. Le Pass B l'a retrouvé et promu en
`CHDR-025`, avec une preuve que juillet n'avait pas : l'absence de générateur
`gen-c1*` dans `vectors/`.

Deux findings P3 de juillet sont également sans équivalent et sont consignés sans
être promus : `CHDR-005` (les deux moitiés de la `Rule` de rotation jamais
jointes) et `CHDR-009` (aucun scénario n'atteint les vecteurs C1/C2).

### Lecture

Sur un code strictement identique, un pipeline orchestré — quatre unités de
Pass A matériellement isolées plus 48 agents de réfutation — a retrouvé seul six
des neuf findings P1/P2 d'un audit manuel, en a manqué un entièrement et un
autre au Pass A, et en a produit quatre nouveaux de rang P2 dont un que personne
n'a pu réfuter.

Le pipeline gagne en volume, en traçabilité et en résistance aux formulations
excessives : le panel a corrigé quatre énoncés surdimensionnés et en a retiré
deux qui n'auraient pas dû atteindre un correcteur. Il perd en tenue, et le
mode de perte est instructif : **les deux manqués sont tous deux des assertions
vacantes** — un `is_err()` qui passerait sans que son `When` ait eu lieu, un
négatif qui passe sous n'importe quelle mutation de son AAD. Un agent qui trace
« step → production → assertion » voit ce que l'assertion compare ; il ne voit
pas ce qu'elle continuerait de comparer si son antécédent disparaissait. C'est
un angle mort de méthode, pas de chance, et il est reproductible : les deux
manqués sont de la même classe.

## 12. Fichiers et symboles affectés

### Écritures de ce rôle

| Fichier | Nature |
|---|---|
| `docs/audits/features/c-headers.md` | **créé** — audit public, 14 sections |
| `docs/audits/features/README.md` | ligne d'index de `c-headers` ajoutée |
| `features/c-headers.feature` | marqueurs d'audit sur les six scénarios portant un finding non résolu ; +43 lignes, **0 suppression** ; aucune phrase de scénario modifiée, aucune renumérotation |
| `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md` | **créé** — ce rapport |

### Trajet des passages soumis à la barrière, puis restitués

Six passages ont été rédigés après les invalidations du gardien, puis
**intégralement restitués** au run `2026-08-03-r2` après la décision du
propriétaire du 2026-08-03 (§10). Aucun n'est plus retenu.

| Fichier | Emplacement | Rédigé puis restitué |
|---|---|---|
| `docs/audits/features/c-headers.md` | §9, ligne d'impact `h-merkle` → `CHDR-007` | **la fuite signalée par le gardien** : chemin de code (`state.rs:57-62`, `:240-248`) et absence d'appel à `Header::validate` |
| `docs/audits/features/c-headers.md` | §7, bloc `CHDR-008` | la base factuelle en `fichier:ligne` : cinq sites `validate()` contre une douzaine de sites de désérialisation |
| `docs/audits/features/c-headers.md` | §10, surfaces publiques | la désignation nominale des trois surfaces rattachées à `CHDR-007` et `CHDR-012` |
| `docs/audits/features/c-headers.md` | §11, chapeau du plan | la portée de production d'une décision sur `CHDR-007` et sur `CHDR-012` |
| `docs/audits/features/c-headers.md` | §8.3, correspondance `CHDR-015` de juillet → `CHDR-007` | les deux apports de cette ronde absents de l'étalon |
| ce rapport | §6 (ligne `CHDR-008`), §7.5 (deux lignes), §10 (fait à peser) | mêmes restitutions, plus la caractérisation du contenu du texte public de juillet |

À quoi s'ajoute la restitution principale : les blocs complets de `CHDR-007` et
`CHDR-012` dans l'audit public §6, avec mécanisme, preuves `fichier:ligne`,
références de spec, surfaces, conséquence, apports des réfuteurs, tables des
lectures concurrentes et critères de clôture.

Aucun verdict, aucune sévérité, aucun statut n'est modifié par ces corrections :
seule la rédaction change.

Marqueurs posés, conformément à `PROCESS.md` § *Gherkin audit-marker lifecycle*
(scénarios 1 et 2 laissés nus, verdict `PROVEN`) :

| Scénario | Tags |
|---|---|
| 3 A corrupted line fails closed | `@audit-partial @chdr-002 @chdr-027` |
| 4 A line is bound to its node and version | `@audit-partial @chdr-001 @chdr-025 @chdr-002 @chdr-027` |
| 5 A header without an owner line is invalid | `@audit-partial @chdr-009 @chdr-011 @chdr-010 @chdr-007 @chdr-012` |
| 6 Granting a new reader leaves every other line untouched | `@audit-partial @chdr-013 @chdr-014 @chdr-016 @chdr-015 @chdr-017 @chdr-018` |
| 7 The revoked gets no line in the new version | `@audit-partial @chdr-019 @chdr-024` |
| 8 An up-link wrap restores derivation for the parent holder | `@audit-semantic-false-positive @chdr-021 @chdr-020 @chdr-026` |

Après édition, le fichier compte toujours **4 `Rule`, 8 `Scenario`, 28 pas** —
compté sur le fichier, pas exécuté.

### Non touchés, délibérément

`features/.agents/c-headers/STATE.md` (l'orchestrateur écrit la transition),
`features/.agents/PROCESS.md`, `features/.agents/orchestrator/QUEUE.yaml`,
`LEDGER.md`, `BLOCKED.md`,
`docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md`, et tout code de
production, de step ou de test.

### Symboles au cœur des findings

`aithos_core::header` — `Header::{build, build_at, append_line, rotate, open,
open_latest, latest_version, check_rotation, validate}`, `check_owner_line`,
`build_lines`, `Recipient`, `Line`, `KeyVersion`, `Wrap::{seal, open}`,
`OWNER_LABEL`.
`aithos_core::seal` — `aad`, `line_aad`, `wrap_aad`, `blob_aad`, `kek`,
`seal_line`, `open_line`, `wrap_seal`, `wrap_open`, `CTX_WRAP_KEY`.
`aithos_bundle` — `hdr_file`, `wrap_file`, `add_line_on`, `deliver_entry`,
`deliver_exact_section`, `deliver_connector_line`, `agent_section_key`,
`agent_node_key`, `rotate_folder`, `move_folder`, `open_blob_v`,
`header_hash_at`, `vault_build`, `Session::append_header_recipient`, `KV`.
Tests et vecteurs — `c1_header_seal.rs::{c1_owner_and_grantee_lines,
c1_fail_closed, c2_wrap_roundtrip_and_cross_check}`, `g2_rotation.rs::G2`,
`g3_move.rs::new_path_bindings_and_parent_wrap`, `vectors/c1-header-seal.json`,
`vectors/g2-rotation.json`, `vectors/g3-move.json`.
Steps — `cucumber.rs` : `ProtocolWorld::open_into`, `dk_and_two_recipients`,
`sealed_header_owner_grantee`, `sealed_header_owner_only`, `single_grantee`,
`sealed_header_three`, `derived_node_rotated`, `seal_into_header`,
`stranger_tries`, `corrupt_line`, `replay_line_other_node`,
`build_without_owner`, `append_grantee_line`, `rotate_without_g1`,
`post_uplink_wrap`, `owner_opens`, `grantee_opens`, `stranger_recovers_nothing`,
`opening_rejected`, `header_invalid`, `owner_line_untouched`, `survivor_opens`,
`revoked_cannot_open`, `owner_opens_new`, `parent_recovers_via_wrap`, `main`.

## 13. Limites de la conclusion

- **Aucune commande n'a été exécutée par ce rôle.** Seuls `ev-50caa5d6` et
  `ev-d6840262` sont cités. Tout le reste est lecture de code courant à
  `a2087f2`.
- **Aucune expérience de mutation.** Les mutants décrits sont des raisonnements
  sur le code lu, proposés comme RED attendus, non comme résultats mesurés. Les
  mesures de mutation publiées par l'étalon sont écartées, et l'une d'elles est
  contredite par le code courant (§5.3).
- **Aucun `VERIFIED`.** L'auditeur ne clôt rien.
- **Périmètre borné** aux huit scénarios existants. Les impacts sont signalés,
  jamais audités. Aucune autre feature n'est ouverte, fermée ou rouverte.
- **La conclusion publique est complète.** Aucun finding n'est retenu : la
  barrière a été levée par décision du propriétaire le 2026-08-03 (§10). La
  limite qui subsiste est différente et porte sur la **sémantique** : `CHDR-007`
  et `CHDR-012` restent `DECISION_REQUIRED`, l'audit expose leurs lectures
  concurrentes sans en retenir aucune.
- **Ce rôle n'a pas exécuté le Pass A** et n'en atteste pas l'isolation autrement
  que par les traces du journal. Il atteste que le gel précède chaque entrée de
  Pass B dans `ledger.jsonl`.
- **Collision d'identifiants.** La branche publique `codex/audit-c-headers`
  (`af32734`) attribue déjà `CHDR-001`…`CHDR-016` à d'autres énoncés. Deux
  documents publics revendiquent la même famille d'identifiants stables réservée
  par `docs/audits/features/README.md:20`. Tant que la collision n'est pas
  tranchée, tout renvoi à un `CHDR-*` doit nommer sa source. Ce n'est **pas** une
  condition de blocage au sens de `PROCESS.md` § *Blocking conditions* — cette
  liste est close — mais c'est une question portée au propriétaire par la même
  voie que la barrière de divulgation.
- **La ligne `counts` du gel est erronée** ; le décompte réel est rétabli en §3.

## 14. Conditions de blocage

| # | Condition | État | Détail |
|---|---|---|---|
| 1 | `DECISION_REQUIRED` sur un finding | **OUVERTE — la seule** | `CHDR-007` (P1) et `CHDR-012` (P2). La question est unique sous deux formes : un invariant que la spec énonce à la voix passive lie-t-il une surface vérifiante, ou décrit-il seulement une propriété d'objet ? **Non tranchée, et à ne pas trancher par un correcteur.** Les deux findings ne sont assignés à personne |
| 4 | contamination du Pass A | fermée | `contamination: "none"` pour les quatre unités ; `history_visible: false` au journal |
| 5 | majorité de panel contre l'auditeur | **résolue** | 8 findings réfutés à la majorité ; les 8 sont tranchés sur preuve de code courant (§6) ; aucun désaccord ne reste indécidable |
| 6 | deux invalidations du gardien sur la même feature | **résolue** | deux invalidations ont bien eu lieu (§10) et avaient arrêté le run ; elles tombent avec la condition 9 — la fuite reprochée n'en est plus une, puisqu'il n'y a plus rien à retenir |
| 7 | budget épuisé | **résolue** | `agents_per_cycle` relevé à 60 dans `QUEUE.yaml` par l'orchestrateur ; ce rôle n'a pas touché le fichier |
| 9 | finding pris par la barrière de divulgation | **résolue** | décision du propriétaire du 2026-08-03, citée textuellement en §10 : `CHDR-007` et `CHDR-012` publiés en entier ; `CHDR-003` et `CHDR-008` déjà levés par retrait. Plus aucune formule de rétention dans aucun fichier suivi |

Les conditions 2, 3, 8 et 10 ne sont pas ouvertes par ce run.

**Une seule condition reste ouverte : la 1.** Elle porte sur la sémantique du
protocole et appartient au propriétaire humain. La décision du 2026-08-03 est une
décision de publication et ne la préjuge en rien.

**Aucune condition de blocage supplémentaire n'est inventée.** La liste de
`PROCESS.md` est close ; la collision d'identifiants (§13) en est absente et ne
justifie donc pas un arrêt, mais elle est portée au propriétaire.

## 15. Action suivante et skill attendu

L'orchestrateur écrit la transition ; ce rôle ne touche pas `STATE.md`.

**Transition attendue :** `AUDIT_INITIAL` → `CORRECTION_REQUESTED`. Les
conditions de blocage 5, 6, 7 et 9 sont résolues (§14). La condition 1 reste
ouverte mais **ne bloque pas la transition** : elle borne le périmètre
assignable, elle ne l'annule pas. Les vingt et un findings actifs autres que
`CHDR-007` et `CHDR-012` sont assignables dès maintenant.

**Répartition des décideurs, explicite :**

| Findings | Décideur | Motif |
|---|---|---|
| `CHDR-007` (P1), `CHDR-012` (P2) | **le propriétaire humain du protocole** | `DECISION_REQUIRED` : deux lectures normatives concurrentes chacun, exposées sans arbitrage dans l'audit public §6 et §12. **Assignés à aucun correcteur** |
| les 21 autres findings actifs | **le correcteur** | aucune sémantique concurrente ; critères de clôture écrits ; lots ordonnés à l'audit public §11 |

Invariants de la transition `→ CORRECTION_REQUESTED`
(`PROCESS.md` § *Guarded transitions*, AM), état à la clôture de ce run :

| Invariant | État |
|---|---|
| audit public écrit | fait — `docs/audits/features/c-headers.md` |
| rapport de run complet | fait — ce fichier, tous les champs de § *Required run conclusion* |
| le gel Pass A précède toute entrée Pass B au journal | vérifié dans `ledger.jsonl` |
| les marqueurs Gherkin correspondent aux findings non résolus | fait — six scénarios marqués, deux laissés nus |
| panel de réfutation enregistré pour tout finding P1/P2 | fait — 16 findings, 48 agents, `pass-a/refutation.json` |

**Skill attendu ensuite :** le correcteur, via
`features/.agents/shared/correct-gherkin-feature/SKILL.md`, sur une branche
`codex/fix-c-headers-<finding-or-scope>` descendante de `a2087f2`, avec un
périmètre assigné explicitement — et **jamais** `CHDR-007` ni `CHDR-012` avant
décision enregistrée.

Ordre de valeur recommandé pour l'assignation, repris de l'audit public §11 :
lot 1 (`CHDR-025`, `CHDR-026`), puis lot 2 (`CHDR-021`, `CHDR-020`), puis lot 3
(`CHDR-019`, `CHDR-024`). Les lots 1 et 2 portent la sécurité.

Une seule question demeure pour `BLOCKED.md`, que l'orchestrateur écrit et que ce
rôle ne résout pas : **la sémantique de `CHDR-007` et celle de `CHDR-012`** — un
invariant que la spécification énonce à la voix passive lie-t-il une surface
vérifiante, ou décrit-il seulement une propriété d'objet ? Les deux lectures de
chaque finding, avec leurs fondements, conséquences, coûts et porteurs, sont
tabulées dans l'audit public §6.

La question de la barrière de divulgation sur ces deux findings est **close** :
tranchée par le propriétaire le 2026-08-03 (§10).

Et, hors liste close, portée au propriétaire par la même voie : la collision
d'identifiants `CHDR-*` avec l'étalon publié.

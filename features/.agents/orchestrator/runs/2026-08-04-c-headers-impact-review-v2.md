# Revue d'impact Gherkin globale — `c-headers`, cycle I3-authority (v2)

## Identité du run

| Champ | Valeur |
|---|---|
| Date | 2026-08-04 |
| Type de run | revue d'impact inter-features |
| Rôle | `review-gherkin-impacts` (G1, orchestrateur) |
| Unité de revue | `CHDR-I3-GLOBAL-IMPACTS-V2` |
| Feature source | `features/c-headers.feature` |
| Racine de travail | `features/.agents/orchestrator/runs/2026-08-04-r4/passA/c-headers/IMPACT-BLIND/` — extrait `git archive`, **sans `.git`** |
| Entrées de diff | `ACCEPTED-DIFF-spec.diff`, `ACCEPTED-DIFF-correction.diff`, `ACCEPTED-COMMITS.txt` (10 commits, `a0af985..c547ccd`) |
| Audit public source | `docs/audits/features/c-headers.md` (2 024 lignes, §6bis comprise) |
| Décision | `features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md` |
| Revendications non probantes | `features/.agents/c-headers/corrector/runs/2026-08-04-correction-i3-authority.md`, `.../auditor/runs/2026-08-04-review-i3-authority.md` |
| Précédent de forme | `features/.agents/orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md` |
| Résultat | **aucun `FULL_AUDIT`** ; onze features `TARGETED`, sept `NONE` ; les deux `COMPLETE` intactes ; un signalement d'embargo remonté à l'orchestrateur |

**Ce que ce rôle n'a pas fait.** Aucune commande `cargo`, `git`, aucun test, aucun
build. Aucun fichier de feature, de code, de vecteur, de spec, de `STATE.md` ni
`QUEUE.yaml` modifié. Aucune feature rouverte, relancée ou réassignée. Aucun
finding audité. Le seul fichier écrit par ce rôle est ce rapport. Toute
affirmation d'exécution ci-dessous est une **citation** d'un rapport d'agent
antérieur, marquée comme revendication.

---

## 0. Discipline de méthode appliquée

**R-1.** Toute affirmation sur le protocole est fondée sur `spec/`, citée
verbatim en bloc avec `fichier:ligne`, phrase entière jusqu'à son terme. Le code
n'est jamais cité à l'appui d'un énoncé normatif.

**R-2.** Chaque affirmation d'absence porte, dans la même phrase, la recherche
exacte, son périmètre et **la couche** pour laquelle elle vaut.

**R-3.** Une recherche dans `rust/**` établit un fait sur le code. Elle
n'établit rien sur ce que le protocole prévoit. Les deux sont tenues séparées
partout ci-dessous.

---

## 1. Le périmètre réel du cycle — revérifié fichier par fichier

Trois mouvements, à traiter séparément.

### 1.1 Lot de spécification (`5be3047`)

`ACCEPTED-DIFF-spec.diff` touche six fichiers de `spec/` — `00-overview.md`,
`03-headers.md`, `05-delegation.md`, `06-revocation.md`,
`09-cli-and-conformance.md`, `10-threat-model.md` — plus `vectors/c3-owner-line.json`
(nouveau), `vectors/gen-c.py` (nouveau), `vectors/ownership.json`, et
`docs/PROPOSITION-SPEC-I3-AUTHORITY-2026-08-03.md`.

`vectors/ownership.json` : deux entrées **réellement nouvelles**
(`c3-owner-line.json`, `gen-c.py`) ; cinq entrées (`README.md`,
`gplus-obligations.json`, `h1-merkle.json`, `h2-gamma-roots.json`,
`i1-concurrency.json`) **déplacées dans le fichier avec un `sha256` identique
caractère pour caractère** entre la ligne retirée et la ligne ajoutée (vérifié
sur les quatre digests : `f74f6c2b…`, `590efdd7…`, `c497b9b2…`, `556de81a…`).
`rust/crates/aithos-bundle/tests/vectors_ownership.rs:17` lit le manifeste dans
`BTreeMap`/`BTreeSet` : l'ordre physique n'est pas observable. **Aucun impact
sur `g-plus-obligations`, `h-merkle`, `h2-gamma-roots`, `i-concurrency` par ce
canal** — recherche : comparaison des digests retirés/ajoutés dans le diff, plus
lecture des types du harnais ; périmètre : `vectors/ownership.json` et
`vectors_ownership.rs` ; couche : **corpus de données + code de test**.

### 1.2 Changement de format filaire

`spec/03-headers.md:20` porte désormais `"kid": "z6LSOwnerKex…"` là où la
révision antérieure portait le littéral `"owner-kex"`. Le chiffré n'est pas
re-dérivé : voir §4.2 ci-dessous pour la démonstration par la spec.

### 1.3 Correction de code (`9dc5889`)

Fichiers de production touchés, relevés par `awk '/^diff --git/'` sur
`ACCEPTED-DIFF-correction.diff` :
`aithos-bundle/src/{bundle,grants,log,publication,revoke,session,structure,vault}.rs`,
`aithos-cli/src/cmd/{header_open,header_seal}.rs`, `aithos-core/src/header.rs`.

**`rust/crates/aithos-bundle/src/state.rs` n'y figure pas.** Recherche : liste
exhaustive des en-têtes `diff --git` du fichier de diff accepté ; périmètre : le
diff de correction accepté ; couche : **le code**. C'est le point 3 des
« Conséquences exécutables » de la décision
(`decisions/2026-08-03-chdr-007-012-i3-authority.md:110-111`) qui n'a pas été pris
par la voie qu'elle décrivait ; le correcteur a posé une passe séparée
`verify_pinned_headers` (`bundle.rs:302-320`). Ce n'est pas un défaut à juger
ici — la correction est acceptée — mais c'est un fait d'impact pour `h-merkle`
(§2.5).

---

## 2. Classification par feature

Rappel d'état, qui gouverne la lecture du tableau : **seules `a-identity` et
`b-derivation` sont `COMPLETE`.** Les seize autres n'ont jamais été auditées et
figurent dans `features/.agents/orchestrator/QUEUE.yaml`, bloc `order`. Pour
elles, `TARGETED` ne signifie pas « réauditer » : il signifie *« leur cycle à
venir doit des scénarios nommés »*, ce que `QUEUE.yaml` § `follow_ups` enregistre
déjà pour la ronde `b-derivation`. `FULL_AUDIT` n'aurait de sens que pour une
feature déjà close — c'est-à-dire pour les deux seules `COMPLETE`, traitées au §3.

| Feature | Classe | Preuve principale |
|---|---|---|
| `a-identity` | `NONE` | §3 |
| `b-derivation` | `NONE` | §3 |
| `d-bundle` | `TARGETED` | §2.1 |
| `g-revocation` | `TARGETED` | §2.2 |
| `n-structural-mutations` | `TARGETED` | §2.3 |
| `o-connector-classes-vault` | `TARGETED` | §2.4 |
| `h-merkle` | `TARGETED` | §2.5 |
| `k-integration` | `TARGETED` | §2.6 |
| `i-concurrency` | `TARGETED` | §2.7 |
| `m-delegated-editions` | `TARGETED` | §2.8 |
| `l-delegated-writes` | `TARGETED` | §2.9 |
| `f-plus-constraints` | `TARGETED` | §2.10 |
| `h2-gamma-roots` | `TARGETED` | §2.11 |
| `e-mandates` | `NONE` | §2.12 |
| `e-mandate-sections` | `NONE` | §2.12 |
| `f-gamma` | `NONE` | §2.12 |
| `g-plus-obligations` | `NONE` | §2.12 |
| `g4-client-surfaces` | `NONE` | §2.13 |

### Méthode de dépistage

Trois passes, toutes sur l'arbre entier de l'extrait :

- **P1 — pas partagés.** Extraction programmatique des 19 fichiers `.feature` et
  des attributs `#[given/when/then]` de
  `rust/crates/aithos-bundle/tests/cucumber.rs` (19 779 lignes), littéraux et
  `regex` compris ; croisement de chaque corps de pas avec les symboles changés.
- **P2 — symboles.** `grep` des cinq signatures migrées (`build`, `build_at`,
  `rotate`, `validate`, `check_rotation`), des API nouvelles (`owner_kid`,
  `open_owner`, `open_owner_latest`, `validate_as_owner`, `owner_kex_pub`,
  `verify_pinned_headers`, `is_header_file`) et du littéral `owner-kex` sur
  `rust/crates/*/src` et `rust/crates/*/tests`.
- **P3 — spec.** Lecture intégrale des 11 fichiers de `spec/` (4 348 lignes) ;
  `grep` des motifs `owner_kex`, `owner line`, `I3`, `kid`, et des motifs de
  migration (§4).

Exhaustivité, énoncée franchement : P1 est exhaustif en dépistage sur les 19
`.feature` et sur `cucumber.rs`, et **sélectif d'un seul niveau d'appel** —
un pas qui atteint une API changée par deux fonctions intermédiaires non nommées
peut m'avoir échappé ; les cas repris ci-dessous ont tous été retracés à la main
jusqu'au site de production. P2 et P3 sont exhaustifs à la casse et à
l'orthographe près des motifs. Les binaires de test hors `cucumber.rs`
(`cb2_*`, `cb1x_*`) n'ont été ouverts que là où un pas y renvoie.

---

### 2.1 `d-bundle` — `TARGETED`

Trois canaux distincts, chacun avec sa preuve.

**(a) Un pas partagé, réécrit par la correction.**
`features/d-bundle.feature:145` porte la ligne d'`Examples` :

```
| wrap       | node-version-and-recipient header line  | line for another node or recipient       | only the intended recipient opens the wrapped key |
```

`cucumber.rs:3100` la dispatche vers `core_header_capability_scenario`
(`cucumber.rs:3041`), dont l'appel `Header::build` a reçu l'argument
`&owner.owner_kex_pub()` par la correction (hunk `@@ -3047,6 +3054,7 @@` du diff
accepté). Cette fonction est en outre **le seul appelant du dépôt** de
`LocalSession::append_header_recipient` (`session.rs:354`) — recherche :
`grep -rn "append_header_recipient" rust/crates/` ; périmètre : les cinq crates ;
couche : **le code** —, méthode dont la correction a changé la validation
(`session.rs:363`, `validate()` → `validate(&header_owner_kid(...))` et
`open_latest(..., "owner-kex", ...)` → `open_owner_latest(...)`).

**(b) La passe I3 est posée dans le vérificateur d'édition.**
`features/d-bundle.feature:13`, `:25`, `:30` (`edition 1 verifies offline`,
`edition verification is rejected`) atteignent `Bundle::verify` via
`edition_verifies` / `edition_rejected` ; `Bundle::verify` appelle désormais
`verify_pinned_headers` en `bundle.rs:1759`. La spec le rend obligatoire :

> `spec/09-cli-and-conformance.md:97-101`
> ```
> - **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
>   verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed, and
>   MUST reject an edition pinning a header that violates I3 (§03.1) — without holding
>   any key, and on every `aithos-core` manifest profile.
> ```

**(c) L'asymétrie émetteur/vérificateur.** `CHDR-034` (§6bis) porte que
`Bundle::publish` (`bundle.rs:1678`) n'a aucune garde I3. `d-bundle` est la
feature qui possède `publish` et `verify` ; son `Rule: Editions chain and verify
offline` est le lieu où la question se pose. Le critère de clôture de
`CHDR-034` propose explicitement une branche de spec — que
`spec/09-cli-and-conformance.md` §9.4 dise que I3 ne lie que la vérification.
Elle ne le dit pas aujourd'hui ; ce qu'il dit du **Core issuer** est :

> `spec/09-cli-and-conformance.md:102-104`
> ```
> - **Core issuer**: the above + mint/delegate/revoke + header rotation with the
>   authority checks of §05.5. MUST refuse to sign an over-wide sub-mandate (pre-flight
>   §05.3) and an unauthorized header rotation.
> ```
>
> « the above » renvoie au **Core reader** du bullet précédent, obligation I3 comprise.

**Scénarios dus au cycle `d-bundle`** : un cas où une édition épinglant un header
sans ligne owner est refusée par `verify` ; un cas d'asymétrie `publish`/`verify` ;
la ligne `wrap` de l'outline de capacité re-tracée jusqu'au `kid` réel.

---

### 2.2 `g-revocation` — `TARGETED`, le plus chargé

**(a) Le seul pas partagé hors `c-headers` que la correction réécrit.**
`features/g-revocation.feature:71` :

```
When the new version claims a line for a key absent from the old version
```

implémenté par `smuggle_recipient` (`cucumber.rs:15255`), réécrit par la
correction (hunk `@@ -15244,12 +15274,14 @@`) : `Recipient::owner(owner_pub)` et
`check_rotation(2, &aithos_core::header::owner_kid(&owner_pub))`. Le `Then`
associé, `header verification is rejected` (`cucumber.rs:15481`), ne lit qu'un
`is_err()` : il ne distingue pas le refus pour destinataire clandestin du refus
pour ligne owner manquante. C'est le point exact où la nouvelle sémantique entre
sans être observée.

**(b) Une obligation normative nouvelle sur la rotation, sans scénario.**

> `spec/03-headers.md:108-113`
> ```
> The wrap
> re-establishes that path in one entry and touches no other line. Verification is
> mechanical: the new version's lines MUST equal the previous lines minus the revoked
> (plus, in the exactly-N case, recipients ⊆ P's header), the new version MUST carry the
> owner line as defined in §3.1 — the revoker re-seals DK' to the subject's `owner_kex`
> read from the DID document, never to whatever key the previous owner line used — and an
> up-link wrap whose author does not hold P is rejected.
> ```

> `spec/05-delegation.md:86-92`
> ```
>   owner's other grants on the same node: it rotates the node key and republishes the
>   header **omitting the revoked child's line but keeping every other line** — including
>   lines it did not create (those it re-seals under the new DK using its own access).
>   The owner line is re-sealed to the subject's `owner_kex` read from the DID document
>   (§03.1), never to the recipient key the previous owner line happened to carry: a
>   rotation that reproduces a wrong owner line propagates it, and I3 makes the whole
>   edition invalid.
> ```

> `spec/06-revocation.md:30-36`
> ```
>   2. if mode ≥ rotate:
>        for each node N in M's perimeter that the revoker has authority over:
>          DK' ← random; version++
>          header[N].new = { lines: reseal DK' to all survivors
>                                   + the owner line, sealed to owner_kex (§03.1) }  # not M
>          post the derivation up-link wrap for N (§03.4 step 2bis)
>          if mode ≥ reencrypt: rewrite N's blobs under keys derived from DK'
> ```

Ces trois passages posent la même règle sous trois angles : le révocateur, qui
peut être un **ancêtre** et non le sujet, doit lire `owner_kex` dans le document
DID plutôt que recopier la ligne précédente. `revoke.rs` implémente la lecture
(`:170` `owner_kex_pub()`, `:203` `rotate(..., &owner_kex, ...)`, `:214`
`check_rotation(new_v, &owner_kid)`). **Aucun scénario de
`features/g-revocation.feature` n'énonce cette obligation** — recherche :
`grep -in "owner" features/g-revocation.feature`, 3 occurrences de « header »,
aucune de « owner line », « owner_kex » ni « DID document » ; périmètre : le
fichier `.feature` entier ; couche : **le corpus Gherkin**.

**(c) Le vecteur G2 reste sur l'ancien littéral.**
`vectors/g2-rotation.json:6` et `:12` portent toujours `"owner-kex"` dans
`old_kids` et `expected_survivor_kids` ; `vectors/gen-g.py:103` le produit ; et
`rust/crates/aithos-core/tests/g2_rotation.rs:21` le fige en
`const G2_OWNER_KID: &str = "owner-kex";`, avec la justification écrite de ne pas
le rouvrir (`g2_rotation.rs:9-20`). Le vecteur devient donc une **fixture de
forme** délibérément décorrélée du fil, alors que §9.2 exige :

> `spec/09-cli-and-conformance.md:43-44`
> ```
> a revocation rotation (old line absent, survivor line opens new DK); a gamma entry
> sign/verify and a `max_actions` count; an edition prev_hash and a fork resolution.
> ```

**(d) Impacts déjà routés par l'audit.** `docs/audits/features/c-headers.md:1834-1837`
adresse trois impacts à `g-revocation` (`CHDR-022` requalifié, `CHDR-016`,
`CHDR-024`). `CHDR-029` (§6bis) y ajoute `revoke.rs:188` (`rotate_folder`) et
`revoke.rs:396` (`move_folder`), inchangés par la correction, où la clé du
survivant est reconstruite depuis `to`.

**Scénarios dus au cycle `g-revocation`** : la ligne owner re-scellée à
l'`owner_kex` du document DID et non à celle de la version précédente ; un `Then`
qui distingue le motif de refus dans le scénario du destinataire clandestin ;
la question de la régénération de G2 sur un `owner_kex` réel, avec le nouvel id
de vecteur et la redline que `vectors/README.md` impose.

---

### 2.3 `n-structural-mutations` — `TARGETED`

Dix pas de `features/n-structural-mutations.feature` atteignent
`structure.rs::structural_*` (P1). Deux changements les traversent :

- `structure.rs:259-266` : `structural_recipients` reconnaît désormais la ligne
  owner par `line.kid == owner_kid` (avant : `line.to == "owner"`), avec
  `owner_kex` lu du document DID en `:259`.
- `structure.rs:783` : `Header::build_at` reçoit `&self.owner_kex_pub()?`.

S'y ajoute `CHDR-031` (§6bis) : `Bundle::move_folder` (`revoke.rs:324`) écrit
`e/circle/index.json` en `:422` **avant** la garde I3 de `build_at` en `:431`, sur
une API publique non transactionnelle — là où `structural_operation`
(`structure.rs:1102-1109`) enveloppe tout dans `self.transaction`. La feature
porte précisément le contrat contraire :

`features/n-structural-mutations.feature` — pas `it is refused before canonical
effect` et `the mutation commits`.

**Scénarios dus** : effet partiel de `move_folder` sur un header sans ligne
owner ; résolution de la clé du survivant depuis `kid` et non `to`
(`CHDR-029`, site `structure.rs:266`).

---

### 2.4 `o-connector-classes-vault` — `TARGETED`

Les pas de cette feature passent par `cb10_when` / `cb10_then`
(`cucumber.rs:11927-11931`), qui consomment le verdict de
`rust/crates/aithos-bundle/tests/cb10_structure_vault.rs` (`vault_config_operation`,
`:648`). La correction a réécrit les deux chemins vault :

- `vault.rs:336` `read_vault_config_owner` → `open_owner_latest` ;
- `vault.rs:357-404` `rotate_vault_connector` : `owner_kex_pub()` (`:372`),
  sélection de la ligne owner par `line.kid == owner_kid` (`:375-377`),
  `rotate(..., &owner_kex, ...)` (`:395-399`), `check_rotation(new_version,
  &owner_kid)` (`:404`).

La ligne d'`Examples` `recipient revocation and rotation`
(`features/o-connector-classes-vault.feature:228`) est celle qui exerce cette
rotation. La spec fait du vault le lieu où I3 porte le plus :

> `spec/08-connectors.md:196-199`
> ```
> - **Double barrier.** "Who holds the vault" = whoever has a valid header line on the
>   exact `/x/<id>` node. The owner always does (I3). A non-owner access succeeds only
>   with both a valid chain covering exact `act.x.<id>.config` and that exact line.
> ```

Ce « always » repose désormais sur la clé destinataire, plus sur l'étiquette.
`CHDR-030` relève par ailleurs que `vault.rs:334` (`read_vault_config_owner`)
n'appelle même pas `validate`, à rebours de son homologue `log.rs:427` ;
`CHDR-029` y ajoute `vault.rs:381`.

**Scénarios dus** : la rotation vault maintient la ligne owner définie par la clé ;
la lecture owner du vault valide le header avant d'ouvrir.

---

### 2.5 `h-merkle` — `TARGETED`

Deux canaux.

**(a) L'impact déjà signalé par l'audit, non refermé par la correction.**
`docs/audits/features/c-headers.md:1838` :

```
| `h-merkle` | le hash du header est plié dans le hash de nœud (`state.rs:57-62`, `:240-248`) via un `serde_json::Value` opaque, sans que `Header::validate` soit jamais appelé sur ce chemin : un header violant I3 y produit un digest valide, épinglé puis signé | `CHDR-007` |
```

`state.rs` **n'est pas dans le diff de correction** (recherche : liste des
en-têtes `diff --git` de `ACCEPTED-DIFF-correction.diff` ; périmètre : le diff
accepté ; couche : **le code**). L'impact tient donc tel quel, et `CHDR-034` en
est la formulation à l'autre bout de la même chaîne (`publish` signe ce que
`verify` refuse). Le contrat de la feature l'expose directement :

`features/h-merkle.feature:8-9` — « The header is folded into its node's hash, so
one proof attests row, header version and … » ; `:55` — « And a fresh proof
carries the new header hash and verifies ».

**(b) Un pas de la feature traverse `move_folder`.** `h_move_republish`, pas
`the owner moves the folder under "projets" and republishes`, appelle
`Bundle::move_folder` — donc l'ordonnancement de `CHDR-031`.

**Scénarios dus** : un header violant I3 ne doit pas produire une racine d'état
signable ; le chemin `state_tree` et le chemin `verify_pinned_headers` doivent
donner le même verdict sur la même édition.

---

### 2.6 `k-integration` — `TARGETED`

`features/k-integration.feature` réunit treize pas touchant du code changé (P1),
dont :

- `a cold verifier given only the files accepts the final edition and the full log`
  → `k_cold_replay` → `publication::cold_verify`, qui a reçu la passe I3
  (`publication.rs:882-898`) ;
- `the owner revokes the gmail agent's mandate with rotation and re-encryption`
  → `revoke.rs`, donc §2.2 ;
- `edition 1 verifies offline`, `edition verification is rejected`,
  `edition verification is refused` → `Bundle::verify`.

Surtout, c'est la feature qui revendique un **niveau de conformité** de bout en
bout (`Feature: Integration — one bundle lives the whole protocol (plan §K, spec
§09)`), et §9.4 vient d'ajouter une obligation à ce niveau :

> `spec/09-cli-and-conformance.md:106-108`
> ```
> An implementation states which levels it claims; the vectors gate each.
> ```

**Scénarios dus** : le vérificateur froid refuse une édition épinglant un header
non conforme à I3, sans détenir de clé.

---

### 2.7 `i-concurrency` — `TARGETED` (léger)

Huit pas atteignent `Bundle::verify` (`i_then_verifies`,
`i_then_resolution_verifies`, `i_then_both_present`, `i_then_log_verifies`,
`i_fresh_replay_rebuilds_semantic_counts`, …). Une édition de fusion ou de
résolution épingle les headers des **deux** branches ; la passe I3 s'y applique
sans que §02.6 ne dise ce qui arrive si l'une des branches en porte un
non conforme. Le texte de §02.6 sur la fusion et le fork est **inchangé** par ce
lot — recherche : `awk '/^diff --git/'` sur `ACCEPTED-DIFF-spec.diff`,
`spec/02-content-tree.md` absent de la liste ; périmètre : le diff de spec
accepté ; couche : **la spécification**.

**Scénarios dus** : une édition de fusion dont une branche épingle un header sans
ligne owner ; le verdict attendu, et par quelle règle.

---

### 2.8 `m-delegated-editions` — `TARGETED` (léger)

Le pas `a grantee publishes an authorized self mutation by exact SID`
(`features/m-delegated-editions.feature`) atteint `core_self_edition_scenario`
(`cucumber.rs:9337` → `:2865`), qui appelle `cold_verify` en `:2873` — donc la
passe I3 nouvelle. Le pas régex `m_carrier_action` (`cucumber.rs:9359-9362`)
couvre `Bundle validates the candidate against its expected parent` et une
vingtaine d'autres phrases de cette feature, et retombe sur
`core_cold_roundtrip_scenario` (`:2775`, `cold_verify` en `:2833` et `:2842`).

Le motif de fond est celui que la décision elle-même invoque
(`decisions/2026-08-03-chdr-007-012-i3-authority.md:49-54`) : le producteur d'une
édition n'est pas nécessairement le sujet. `spec/05-delegation.md:89-92` est du
texte normatif neuf adressé exactement à ce producteur délégué.
`features/m-delegated-editions.feature:81` exige que « the changeset explains
content, index, root, header, wrap, Gamma, vault and rotation consequences » : la
conséquence « ligne owner » y entre désormais.

**Scénarios dus** : une édition déléguée dont la rotation reproduit une mauvaise
ligne owner est refusée ; le changeset l'explique.

---

### 2.9 `l-delegated-writes` — `TARGETED` (léger)

`an agent with exact authority for self SID "…"` (`cucumber.rs:11567`) atteint
`core_self_edition_scenario` et donc `cold_verify`. `core_fence_read` et
`l_super_revokes_helper` touchent respectivement des chemins de lecture de header
et `revoke`. `CHDR-036` liste par ailleurs `grants.rs:834`, `:1044`, `:1204`
parmi les chemins de lecture qui n'appellent pas `validate` — chemins de grantee
délégué.

**Scénarios dus** : un écrivain délégué ne publie pas une édition que le
vérificateur froid refuse.

---

### 2.10 `f-plus-constraints` — `TARGETED` (léger)

`the audit reports every logged action compliant` → `audit_compliant` →
`rotate_folder` ; `the sealed body is swapped for another one` → `swap_sealed_body`
→ `audit_key_owner_with_kex` (`log.rs:426-428`), dont la correction a changé la
validation. Le canal est celui de la clé d'audit du vault, pas des contraintes
elles-mêmes.

**Scénarios dus** : aucun scénario nouveau n'est requis par la spec ; le cycle
doit seulement constater que la clé d'audit passe par un header validé.

---

### 2.11 `h2-gamma-roots` — `TARGETED` (léger)

`edition verification is refused` et `the content roots and the flat file pins
still verify` atteignent `Bundle::verify`. Aucun texte de `spec/07-gamma.md`
n'est touché par le lot — recherche : liste des `diff --git` de
`ACCEPTED-DIFF-spec.diff`, `spec/07-gamma.md` absent ; périmètre : le diff de
spec accepté ; couche : **la spécification**. L'impact se réduit à la
cohabitation de la nouvelle passe avec les racines gamma dans le même `verify`.

---

### 2.12 `e-mandates`, `e-mandate-sections`, `f-gamma`, `g-plus-obligations` — `NONE`

- **Symboles.** Aucun de leurs pas n'atteint une API changée autrement que par
  `verification_rejected` (`cucumber.rs:12008-12011`), qui est
  `DidDocument::verify()` et non `Bundle::verify` — lecture directe du corps du
  pas. Les trois pas `cb4_*` de `f-gamma` touchent `structural_*` par le nom du
  motif, non par le chemin vault/header.
- **Spec.** Le lot ne touche ni `spec/04-mandates.md` ni `spec/07-gamma.md`
  (recherche : les en-têtes `diff --git` de `ACCEPTED-DIFF-spec.diff` ;
  périmètre : le diff de spec accepté ; couche : **la spécification**). La seule
  mention d'`owner_kex` dans `spec/04-mandates.md` est `:92`, inchangée :
  > ```
  > `kex_pubkey` MUST equal the Ed25519→X25519 conversion of `pubkey` under the normative
  > map (§01.2); a mismatch invalidates the mandate. Header lines seal to `kex_pubkey` —
  > nothing is left implicit, yet the grantee still owns exactly one keypair (the owner's
  > `owner_kex` is already explicit; this symmetrizes the grantees).
  > ```
  Elle **anticipait** déjà la lecture A ; le lot ne la modifie pas.
- **Corpus.** Aucun de leurs vecteurs (`e1-mandate.json`, `eplus-attenuation.json`,
  `f1..f3`, `fplus-constraints.json`, `gplus-obligations.json`) n'est touché par
  l'un des deux diffs, ni par valeur ni par digest.

### 2.13 `g4-client-surfaces` — `NONE`

`features/g4-client-surfaces.feature` porte quatre scénarios, tous sur
`delegate_pubkey`, `verify_mandate_chain`, `build_session_submandate`,
`sign_ceremony_challenge` et la custody de clé. **`rust/crates/aithos-wasm/src/lib.rs`
ne contient aucune occurrence de `Header` ni de `header`** — recherche :
`grep -rn "header\|Header" rust/crates/aithos-wasm/src/` ; périmètre : la seule
source de ce crate ; couche : **le code**. Le crate n'importe que
`verify_chain`, `verify_chain_revocable`, `Mandate`, `MandateSpec`,
`PerimeterEntry` (`lib.rs:10`).

Les deux commandes CLI durcies par la correction, `aithos header-seal` et
`aithos header-open`, ne relèvent pas de cette feature : ses scénarios CLI
portent sur la cérémonie de session déléguée. La dette de couverture
correspondante (`CHDR-035`) est une dette **`c-headers`** sans propriétaire de
feature, et elle est proposée comme suivi transverse au §6.

---

## 3. Les deux features `COMPLETE`

`PROCESS.md` § *Prohibitions* (dans la version amendée soumise,
`docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md:485-489`) :

> ```
> The orchestrator may not reopen a feature already `COMPLETE`, may not push
> `main`, may not choose a semantics under `DECISION_REQUIRED`, may not widen a
> corrector's assigned scope, and may not run a cycle on a repository whose
> recorded `main` base it did not itself verify.
> ```

et la condition 10 de la liste close (`:474`) fait d'un `FULL_AUDIT` une
condition de blocage, jamais une réouverture. La décision le redit elle-même
(`decisions/2026-08-03-chdr-007-012-i3-authority.md:140-142`).

### 3.1 `a-identity` — aucun verdict invalidé

- **L'audit accepté ne porte aucun verdict sur les headers.**
  `grep -ci "header" docs/audits/features/a-identity.md` → **0**. Recherche :
  `grep` insensible à la casse sur les 566 lignes du fichier ; périmètre :
  l'audit public accepté de `a-identity` ; couche : **la documentation d'audit**.
- **Aucun de ses pas n'est touché.** Les trois pas de `a-identity.feature`
  qui appellent une méthode `verify()` — `doc_contains_four_keys`,
  `doc_signature_verifies`, `verification_rejected` — portent tous sur
  `DidDocument`, lu directement en `cucumber.rs:12010`
  (`w.did_doc.as_ref().unwrap().verify()`). Aucun n'entre dans un hunk de
  `cucumber.rs` du diff de correction (hunks aux lignes ~268, 3054, 7564-8186,
  12344, 12417, 15274).
- **Le durcissement lui donne raison plutôt que tort.** `a-identity.feature:75`
  porte la ligne d'`Examples` `| a kex key in the Ed25519 codec |`, classée
  `PROVEN` par l'audit accepté (`docs/audits/features/a-identity.md:317`,
  scénario 8, « malformed/wrong-codec keys … correctly re-signed »). Or `keys.kex`
  du document DID est exactement l'entrée que §03.1 rend load-bearing :

  > `spec/03-headers.md:36-40`
  > ```
  > - `kid` names the line's recipient **key**: the grantee's multibase Ed25519 pubkey,
  >   whose X25519 counterpart is obtained by the normative map of §01.2, or — for the
  >   owner line — the subject's `owner_kex` in multibase (`z6LS…`), byte-identical to
  >   `keys.kex` of the subject's DID document (§01.4). Two lines of one key version
  >   MUST NOT carry the same `kid`.
  > ```

  Le verdict `PROVEN` de `a-identity` sur le codec de `keys.kex` devient plus
  porteur qu'il ne l'était. Il n'est pas contredit.

**Verdict : `NONE`. Rien n'est dû. Aucun `FULL_AUDIT`, aucune réouverture.**

### 3.2 `b-derivation` — aucun verdict invalidé

- **L'audit accepté ne porte aucun verdict sur les headers.**
  `grep -in "header" docs/audits/features/b-derivation.md` → **2 occurrences**,
  `:527` et `:547`, toutes deux des références au *gate* et à la *branche*
  `c-headers`, jamais à l'objet header. Recherche : `grep` insensible à la casse
  sur les 977 lignes ; périmètre : l'audit public accepté de `b-derivation` ;
  couche : **la documentation d'audit**.
- **Aucun de ses pas n'atteint une API changée** — P1 renvoie l'ensemble vide
  pour `b-derivation.feature`.
- **Un seul chemin de production traversé, sémantiquement inchangé.** Les deux
  derniers scénarios (`Renaming never re-keys`) atteignent
  `Bundle::owner_current_section_key_with_kex` (`bundle.rs:694`), dont l'unique
  modification est `open_latest(&self.did, "owner-kex", owner_kex)` →
  `open_owner_latest(&self.did, owner_kex)` (`bundle.rs:710`). Sur un bundle
  produit par le même arbre, les deux résolvent la même ligne.
- **Sa dette ouverte est indépendante.** `QUEUE.yaml` § `follow_ups` porte
  `b-derivation-round-2-targeted: [a-identity, c-headers, d-bundle, e-mandates,
  n-structural-mutations]`. Elle n'est ni levée ni aggravée par ce cycle. En
  revanche la dette `TARGETED` du générateur `gen-c1*` inexistante, ouverte par
  la revue d'impact `b-derivation` du 2026-08-03, est refermée par
  `vectors/gen-c.py` — c'est la revendication du lot de spécification
  (`decisions/…:90-92`), à vérifier par qui de droit, pas ici.

**Verdict : `NONE`. Rien n'est dû. Aucun `FULL_AUDIT`, aucune réouverture.**

---

## 4. L'obligation rétroactive, instruite de bout en bout

C'est la partie que le propriétaire lira en premier. Elle est construite
uniquement sur `spec/`, citée avant toute conclusion.

### 4.1 Ce que la spécification pose

> `spec/00-overview.md:85-92`
> ```
> A profile gates the introduction of signed constructs; it never gates a verification
> rule. The I3 obligation of §0.2 introduces no signed construct and changes no signed
> byte: it binds every `aithos-core` profile, historical ones included. A rule that
> bound only the newest profile would be escaped by publishing under an older one, and
> would bind nothing. Editions published before specification revision
> `2026-08-03-i3-authority` are therefore re-verified under it; this is the one
> retroactive tightening of this series, and it is stated here rather than hidden in a
> profile.
> ```

> `spec/00-overview.md:35-40` (I3, dans son entier)
> ```
> 3. **I3 — Owner line.** Every `key_versions[*].lines` of every header MUST contain
>    the owner line: the line whose recipient key is the subject's `owner_kex`, as
>    published in the DID document (§01.1, §01.4, §03.1). A header without one is
>    invalid. An edition verifier MUST parse every header the edition pins and MUST
>    reject the edition if any key version of any of them has no owner line. The
>    routing label `to` never establishes the owner line and never satisfies I3.
> ```

> `spec/03-headers.md:45-50` (l'obligation, jusqu'à son terme — la seconde
> phrase borne la première par le palier de vérificateur)
> ```
> - **I3:** every `key_versions[*].lines` MUST include the owner line. A header
>   violating this is invalid. An edition verifier MUST reject an edition that pins
>   such a header (§0.2, §9.4). Every verifier MUST check, without any key, that some
>   line of every key version declares `owner_kex` as its `kid`; a verifier holding
>   `owner_kex` MUST additionally check that that line opens under it, and MUST reject
>   the header when it does not.
> ```

Le critère keyless est donc, mot pour mot, *« some line of every key version
declares `owner_kex` as its `kid` »*. Un header antérieur portant
`kid: "owner-kex"` ne le déclare pas. La limite nommée par le correcteur
(`corrector/runs/2026-08-04-correction-i3-authority.md:240`) est fondée.

### 4.2 Combien d'artefacts du dépôt portent l'ancien format, et lesquels sont rejoués

Recherche : `grep -rn 'owner-kex'` sur **tout** l'extrait, puis tri manuel des
occurrences en écartant (i) les deux fichiers `ACCEPTED-DIFF-*`, (ii) la chaîne
de contexte de dérivation `aithos-core/v1/owner-kex` — qui est un autre objet,
posé par `spec/01-identity-and-keys.md:13` et repris en
`rust/crates/aithos-core/src/derive.rs:12` et `vectors/gen-c.py:116` —,
(iii) la prose d'audit, de décision, de proposition et de rapport de run,
(iv) `docs/audits/split/spl8-amputation.patch`, patch d'archive non exécutable.

Restent **trois artefacts vivants**, tous du même objet, aucun n'étant un header
sérialisé :

| Artefact | Ligne | Nature | Rejoué par un test ? |
|---|---|---|---|
| `vectors/g2-rotation.json` | `:6` (`old_kids`), `:12` (`expected_survivor_kids`) | listes d'étiquettes, pas de lignes scellées | **Oui** — `rust/crates/aithos-core/tests/g2_rotation.rs:33-37` (`include_str!`), consommé en `:86` et `:105` |
| `vectors/gen-g.py` | `:103` (`old_kids = ["owner-kex", …]`) | générateur du précédent | non exécuté par le harnais Rust |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | `:21` (`const G2_OWNER_KID`) | constante du test | c'est le test lui-même |

**Aucun header sérialisé au format antérieur n'existe dans le dépôt.** Recherche,
en trois passes : `find . -name "header.json"` → 0 résultat ;
`grep -rln 'key_versions' vectors/ docker/ scripts/ ui-mockup/` → deux fichiers
seulement, `vectors/c3-owner-line.json` et `vectors/gen-c.py`, tous deux créés
par ce lot et au format nouveau ; `grep -rln '"kid"' vectors/ rust/` → les deux
mêmes. Périmètre : l'arbre entier de l'extrait. Couche : **le corpus de
données**. Le §6bis de l'audit dit la même chose
(`docs/audits/features/c-headers.md:1471-1473`).

Le seul artefact rejoué, `g2-rotation.json`, **ne tombe pas** : la correction a
introduit `G2_OWNER_KID` (`g2_rotation.rs:21`) et le passe explicitement à
`check_rotation(2, G2_OWNER_KID)` (`:97`, `:114`), avec la justification écrite
de ne pas rouvrir le vecteur (`:9-20`). Le coût est déplacé, pas payé : le
vecteur de conformité G2 n'exhibe plus une ligne owner conforme au fil. C'est la
dette `g-revocation` du §2.2(c).

**Conclusion de cette sous-partie, à la couche du dépôt : le dépôt ne contient
aucune donnée que l'obligation rétroactive rendrait invérifiable.** Le coût
rétroactif est intégralement hors dépôt.

### 4.3 Ce que la spécification prévoit pour un porteur de données existantes

**Elle ne prévoit rien de spécifique, et c'est un fait de la couche
spécification, pas une déduction du code.** Recherche :
`grep -rn -iE "migrat|legacy|grandfath|republi|re-publi|supersed|backward|retroactiv|transition|pre-existing|existing depl"`
sur `spec/` en entier (11 fichiers, 4 348 lignes). Les seules clauses de
migration trouvées concernent :

- le **plan des mandats** — `spec/00-overview.md:103-111`,
  `spec/04-mandates.md:32` (§4.1.1), `:45-51`, `:68`, `:290`,
  `spec/05-delegation.md:66` ;
- le **catalogue de connecteurs** — `spec/08-connectors.md:181-182`, `:245`.

**Aucune ne porte sur les headers ni sur I3.** Périmètre : les 11 fichiers de
`spec/`. Couche : **la spécification**.

Ce que la spécification donne néanmoins, et qui suffit à instruire le cas :

**(a) Le header est le plan mutable, par construction.**

> `spec/00-overview.md:33-34`
> ```
> 2. **I2 — Credentials are immutable.** A grantee's keypair and mandate are never
>    modified after issuance. All change happens in storage (headers, ciphertext).
> ```

**(b) La réparation ne re-dérive aucun chiffré.** L'AAD de la ligne ne contient
pas `kid` :

> `spec/00-overview.md:62-65`
> ```
> AAD convention, NUL-separated after the purpose label
> `"aithos-core/v1/<purpose>"`: `subject_did ‖ node_path ‖ key_version` for content
> purposes (`blob`, `tagwrap`, `vault`, `gamma-payload`), `subject_did ‖ header_path ‖
> key_version` for `header-line`. Purposes never overlap.
> ```

> `spec/03-headers.md:141-145`
> ```
>        ss   = X25519(esk, recipient_pub)
>        kek  = HKDF-SHA256( ikm = ss, salt = ∅,
>                 info = "aithos-core/v1/hdr-kek" ‖ 0x00 ‖ epk ‖ recipient_pub )
>        c    = XChaCha20-Poly1305( kek, n₂₄,
>                 aad = "aithos-core/v1/header-line" ‖ 0x00 ‖ subject_did
>                       ‖ 0x00 ‖ node ‖ 0x00 ‖ key_version, DK )
> ```

`kid` n'entre ni dans `info` ni dans l'`aad`. **Pour un porteur dont la ligne
owner était déjà scellée à son `owner_kex` réel, la mise en conformité est une
ré-étiquette de `kid`, pas un re-scellement** — le seul octet qui change est le
champ `kid`, et le chiffré `c` reste valide. C'est ce que le lot de
spécification a lui-même établi pour le corpus
(`docs/PROPOSITION-SPEC-I3-AUTHORITY-2026-08-03.md:98-102`, correction du
2026-08-03).

**(c) Mais cette ré-étiquette n'est pas gratuite au niveau de l'édition.**

> `spec/03-headers.md:132-134`
> ```
> The header's hash is folded into its node's Merkle hash (§02.10): appending a line or
> rotating bumps the node's proof path to the signed state root, so a reader proves it
> holds the **current** header without fetching any other header.
> ```

Changer un octet de `header.json` change son `BLAKE3(JCS(...))`, donc la racine
d'état, donc le manifeste : **une nouvelle édition est nécessaire.** Et la
réécriture de l'histoire est fermée :

> `spec/00-overview.md:80-83`
> ```
> Version order is causal, never inferred from physical JSONL order: draft1/v1 may
> lead to draft1/v1 or draft2/v2, while draft2/v2 never leads back. Missing, mixed on
> one introducing edge, or unknown profiles fail closed. Historical manifests and
> entries are never rewritten or assigned synthetic references.
> ```

Noter la portée exacte, jusqu'au terme : la phrase interdit de réécrire des
**manifestes historiques** et des **entrées**. Elle ne dit rien des headers, qui
sont précisément le plan que I2 déclare mutable.

**Conclusion pour le porteur de données existantes, à la couche
spécification :** la spécification lui impose la conformité rétroactive
(`00-overview.md:89-92`), ne lui accorde aucune clause de grâce, aucun profil
d'échappement, aucune procédure de migration nommée, et lui laisse deux faits
utilisables — le header est mutable (I2), et `kid` n'entre pas dans la
cryptographie (§0.3, §3.8). La conformité de l'**état courant** s'obtient donc
par une ré-étiquette publiée en une édition, sans re-dérivation ni
re-chiffrement. Les **éditions passées**, elles, restent telles qu'elles sont :
`00-overview.md:82-83` interdit de réécrire leurs manifestes, et
`00-overview.md:89-90` dit qu'elles sont re-vérifiées sous la nouvelle révision.
**La spécification pose ces deux phrases sans dire laquelle l'emporte pour une
édition historique dont le header portait l'ancien `kid`.** C'est la question
que le propriétaire doit trancher, et elle n'est tranchée nulle part dans les
4 348 lignes de `spec/` — recherche : lecture intégrale des 11 fichiers, plus le
`grep` de migration ci-dessus ; périmètre : `spec/` ; couche : **la
spécification**.

### 4.4 Ce que la spécification prévoit pour un changement légitime de la clé du propriétaire

La réponse est nette, et elle rend la question moins coûteuse qu'il n'y paraît.

> `spec/01-identity-and-keys.md:11-14`
> ```
> root_sign    = ed25519_seed( derive("aithos-core/v1/root-sign", S) )     — identity root
> content_sign = ed25519_seed( derive("aithos-core/v1/content-sign", S) )  — the owner's pen
> owner_kex    = x25519_sk(    derive("aithos-core/v1/owner-kex", S) )
> ```

> `spec/01-identity-and-keys.md:128-135` (tableau, l'inventaire déclaré complet)
> ```
> | Holder | Material | Mutable? |
> |---|---|---|
> | Owner | `S` (+ derived signing/kex keys, recomputed) | never (root re-key = new identity epoch, §10.4) |
> | Owner (cold) | succession keypair, independent of `S` | never; used once per identity epoch (§1.1) |
> | Device | one X25519 device key + wrap of `S` | replaceable per device |
> | Grantee | one Ed25519 keypair | **never** (I2) |
> | Node | DK per (path, key_version) | rotated by authorized revocations |
> ```

> `spec/01-identity-and-keys.md:31-39` (la clé de succession, phrase entière)
> ```
> **Succession key.** At genesis the owner also generates a **succession keypair**
> (Ed25519), independent of `S` (not derived from it), public half pinned in the DID
> document (§1.4), private half kept cold — paper, HSM, or threshold custody, never on
> a device that runs agents. It is the **sole authority** for one act: declaring a new
> master key, i.e. signing the identity-epoch transition (§10.4) that publishes a
> successor DID document when `S` is compromised or lost. Since `S` itself never
> rotates, the succession key is the only exit; in the absentee-owner profile it is
> also the last-resort cut for a compromised head mandate (§04.8, §10.8). It signs
> nothing else, ever.
> ```

> `spec/10-threat-model.md:39-44`
> ```
> Holds a wrap of `S` ⇒ total compromise (as in any root-holding design). Response: a
> **new identity epoch** — generate `S′`, publish a new DID doc signed by the cold
> **succession key** (§01.1), the sole authority for an epoch transition; re-issue
> mandates, rotate + re-encrypt nodes under the new tree, supersede old editions. Heavy and deliberate; `S` is a single
> object precisely so it can be placed in threshold/MPC custody to raise this bar. Old
> ciphertext the attacker copied stays his (physical limit).
> ```

> `spec/10-threat-model.md:111-112`
> ```
> 3. **Declaring a new master key** after seed compromise or loss — reserved to the
>    cold succession key (§01.1), the single exit from a non-rotating `S`.
> ```

**Conclusion, à la couche spécification.** `owner_kex` est dérivée de `S`
(§01.1:13) ; `S` ne tourne jamais (§01.5, ligne `Owner` du tableau ; §01.1:36-37
« Since `S` itself never rotates ») ; le seul changement légitime de la clé du
propriétaire est donc **l'époque d'identité**, et §10.4 exige déjà d'elle
« rotate + re-encrypt nodes under the new tree, supersede old editions » —
c'est-à-dire de **re-sceller chaque header sous le nouvel arbre**. L'I3 amendée
n'ajoute donc **aucune charge** au changement légitime de clé du propriétaire :
la seule procédure que la spécification prévoit réécrit déjà toutes les lignes
owner.

**Un point de composition reste ouvert, et il est de spec, pas de code.**
§03.1:36-40 lie le contrôle à « the subject's DID document » **sans le dater**.
§01.4:116-119 dit :

> ```
> A verifier accepts a successor DID document only if the transition verifies under
> the **previous** document's `succession` key. Any other signer — including
> `#root` itself — is rejected: a stolen `S` can never steal the identity's future. It is signed by root_sign and versioned by the same
> edition chain as the bundle. Grantee keys never appear in it.
> ```

et §00.4:89-90 rend I3 applicable aux éditions antérieures à la révision. Le
composé des trois — quel document DID lie une édition publiée sous l'époque
précédente — n'est énoncé nulle part dans `spec/`. Recherche : lecture intégrale
des 11 fichiers, plus `grep -rn 'owner_kex|owner line|I3|kid'` ; périmètre :
`spec/` ; couche : **la spécification**. Je le signale ; je ne le tranche pas.

### 4.5 Ce qui reste au journal de version

`CHDR-033` (§6bis) porte la conséquence de release : `rust/Cargo.toml:12` reste
`version = "0.1.0-alpha.1"` et `CHANGELOG.md:10-13` ne nomme ni la rupture d'API
de `aithos-core::header` ni la rupture de format au repos. Ce n'est pas un
impact de feature — c'est un suivi transverse, proposé au §6.

---

## 5. Les neuf findings de §6bis — débordement, sans audit

Je ne juge aucun de ces findings. Je dis seulement s'ils débordent de
`c-headers` et vers quelle feature.

| Finding | Déborde ? | Vers |
|---|---|---|
| `CHDR-028` | **sous embargo — voir ci-dessous** | non déterminé par ce rôle |
| `CHDR-029` | oui | `g-revocation` (`revoke.rs:188`, `:396`), `n-structural-mutations` (`structure.rs:266`), `o-connector-classes-vault` (`vault.rs:381`) |
| `CHDR-030` | oui | `d-bundle` (`bundle.rs:667`, `:674`), `o-connector-classes-vault` (`vault.rs:334`), `f-plus-constraints` (`log.rs:427`), `m-delegated-editions` / `l-delegated-writes` (`session.rs:363`) |
| `CHDR-031` | oui | `n-structural-mutations` (contrat « refused before canonical effect »), `h-merkle` (le pas `move … and republishes`), `d-bundle` (contrat transactionnel) |
| `CHDR-032` | marginalement | reste `c-headers` (`Header::validate`) ; touche `d-bundle` par `verify_pinned_headers` et la CLI `header-seal` |
| `CHDR-033` | non — hors feature | suivi transverse de release (`Cargo.toml`, `CHANGELOG.md`) |
| `CHDR-034` | oui | `d-bundle` (`Bundle::publish`), `h-merkle` (`state_tree`) |
| `CHDR-035` | non — sans propriétaire de feature | `g4-client-surfaces` **ne le porte pas** (§2.13) ; dette CLI `c-headers`, proposée en suivi transverse |
| `CHDR-036` | oui, par ses sites | `g-revocation` (`revoke.rs`), `n-structural-mutations` (`structure.rs`), `o-connector-classes-vault` (`vault.rs`), `l-delegated-writes` / `m-delegated-editions` (`grants.rs`) |

### `CHDR-028` — arrêt et signalement

Le finding est sous embargo. L'audit n'en porte qu'un identifiant et un titre
neutre (`docs/audits/features/c-headers.md:1277-1300`), et `BLOCKED.md` § *Open*
enregistre la condition 9 comme ouverte et appartenant au propriétaire. **Je
n'écris rien de plus sur son énoncé, ses preuves ou son critère de clôture.**

**Signalement, conformément au brief.** En instruisant le §4 — la portée réelle
de l'obligation rétroactive sur un porteur de données existantes — mon analyse
m'a conduit à une observation de code sur **l'étendue de la passe de
vérification d'édition** qui pourrait coïncider avec le mécanisme de
`CHDR-028`. Je me suis arrêté : je ne l'écris pas ici et je ne l'ai reportée
dans aucun classement du §2. Elle vous est remise hors rapport. C'est une
condition de blocage 9 et elle vous appartient — je ne la résous pas, et je ne
déduis rien de son existence pour la classification, qui tient sans elle.

**Conséquence de méthode :** toute classification qui dépendrait d'une
énumération des surfaces de vérification d'édition de `aithos-bundle` est
différée. Je n'ai pas conduit cette énumération, délibérément.

---

## 6. Suivis proposés pour `QUEUE.yaml`

**Je ne modifie pas `QUEUE.yaml`. Ce qui suit est une proposition de contenu.**
Le bloc `follow_ups` existant est conservé intégralement ; les clés ci-dessous
s'y ajoutent.

```yaml
follow_ups:
  # — existant, inchangé —
  b-derivation-round-2-targeted: [a-identity, c-headers, d-bundle, e-mandates, n-structural-mutations]
  bder-006-d-bundle: tag-view and wrap scenarios owed by the d-bundle cycle

  # — proposé par la revue d'impact c-headers I3-authority du 2026-08-04 —
  chdr-i3-authority-targeted: [d-bundle, g-revocation, n-structural-mutations, o-connector-classes-vault, h-merkle, k-integration, i-concurrency, m-delegated-editions, l-delegated-writes, f-plus-constraints, h2-gamma-roots]
  chdr-i3-g-revocation: the rotation scenarios owe the owner line re-sealed to the DID document's owner_kex and never reproduced from the previous line (spec 03.4:110-113, 05:89-92, 06:33-34); the smuggled-recipient Then must distinguish its refusal motive; vectors/g2-rotation.json still freezes the pre-lot owner kid in old_kids and expected_survivor_kids
  chdr-i3-d-bundle: the edition cycle owes a rejected edition pinning an I3-violating header, and a statement on the publish/verify asymmetry (CHDR-034)
  chdr-i3-h-merkle: state.rs folds header.json into the node hash without Header::validate and is untouched by the correction; the h-merkle cycle owes the case where an I3-violating header must not produce a signable state root
  chdr-i3-n-structural: move_folder writes the zone index before the I3 guard on a non-transactional public API (CHDR-031); the structural cycle owes the partial-effect scenario
  chdr-i3-o-vault: vault rotation and owner-side vault read owe the key-defined owner line (spec 08:196-199); read_vault_config_owner calls no validate (CHDR-030)
  chdr-i3-k-integration: the integration cycle owes the cold verifier refusing an I3-violating edition, keyless, as spec 09.4 now requires of a Core reader
  chdr-i3-concurrency: merge and fork editions pin both branches' headers; spec 02.6 is untouched by the lot and says nothing about an I3-violating branch

  # — suivis transverses, sans propriétaire de feature —
  chdr-033-release: workspace version bump and a CHANGELOG entry naming the five changed aithos-core::header signatures and the at-rest break for headers written by an earlier binary
  chdr-035-cli-coverage: rust/crates/aithos-cli/tests/cli_surface.rs exercises neither `aithos header-seal` nor `aithos header-open`; the debt is c-headers', not g4-client-surfaces'
  chdr-i3-spec-open-question: spec/ does not say which DID document binds an edition published under a previous identity epoch, while 00-overview.md:89-90 makes I3 retroactive and 01-identity-and-keys.md:116-119 governs successor acceptance — owner question, not an agent's
```

`order` : **inchangé**. Aucun `FULL_AUDIT` n'est prononcé, donc la condition de
blocage 10 n'est pas levée pour la simple raison qu'elle n'est pas soulevée.

---

## 7. Conditions de blocage

| Condition | État |
|---|---|
| 10 — `FULL_AUDIT` par la revue d'impact | **non soulevée.** Aucune feature n'est classée `FULL_AUDIT` |
| 9 — barrière de divulgation | **déjà ouverte sur `CHDR-028`** (`BLOCKED.md` § *Open*, entrée `2026-08-04-r2`). Ce rapport ne la ferme pas et y ajoute le signalement du §5, remis hors rapport |

---

## 8. Limites de la conclusion

1. **Aucune exécution.** Aucun `cargo`, aucun `git`, aucun test. Toute
   affirmation de comportement ci-dessus est une lecture de source ou une
   citation d'un rapport antérieur explicitement marquée comme revendication.
   Un pas classé `TARGETED` sur la foi d'un chemin d'appel lu n'a pas été
   observé s'exécuter.
2. **Pas d'histoire git.** L'extrait est sans `.git`. Les antériorités
   revendiquées par l'audit (« ces quatre lignes sont inchangées par `9dc5889` »)
   n'ont pas été revérifiées : je n'en ai pas les moyens et je ne les reprends
   pas à mon compte.
3. **Le dépistage de pas est exhaustif, le traçage est d'un niveau.** Un pas
   atteignant une API changée par deux fonctions intermédiaires non nommées peut
   avoir échappé à P1. Les cas retenus ont tous été retracés à la main.
4. **Les binaires de test hors `cucumber.rs`** (`cb2_*`, `cb1x_*`,
   `cb10_structure_vault.rs`, `cb12_publication_package.rs`,
   `cb13_concurrency_final.rs`) n'ont été ouverts que là où un pas Gherkin y
   renvoie. Une dépendance portée uniquement par l'un d'eux, sans pas Gherkin,
   n'est pas couverte.
5. **`CHDR-028` n'est pas instruit** et une piste d'analyse a été volontairement
   abandonnée (§5). Ce rapport est complet **sous cette réserve**, et le
   propriétaire de l'embargo doit relire le §4 en sachant qu'une observation lui
   a été retirée.
6. **Je n'ai pas jugé la justesse de la correction.** Elle est acceptée ; ce
   rapport ne traite que de ce que le reste du corpus en subit.

## 9. Action suivante attendue

Décision humaine sur les suivis du §6 et sur la question ouverte du §4.4, puis
reprise du train à `g4-client-surfaces` selon `QUEUE.yaml` § `order`, inchangé.

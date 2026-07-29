# Audit d'implémentation — `a-identity.feature`

## Métadonnées

| Champ | Valeur |
|---|---|
| Feature auditée | `features/a-identity.feature` |
| Date | 2026-07-29 |
| Révision Git observée | `2fee855` (audit initial) ; `be2d098` + correctif AID-001/002/005 (cette révision) |
| État observé | Correctif appliqué sur la branche `fix/aid-001-002-005-identity-fail-closed`, baseline `be2d098` propre pour les gates rejoués |
| Runner principal | `aithos-bundle --test cucumber` |
| Implémentation principale | `aithos-core::{keys,did,derive,wire}` |
| Surfaces contrôlées | Core, Bundle, CLI, WASM, Gateway et Provider lorsque l'exigence Identity les concerne |
| Statut de la note | **PARTIELLEMENT CLÔTURÉE** — AID-001, AID-002 et AID-005 `IMPLÉMENTÉ` ; AID-003 et AID-004 restent `OUVERT` |

## Verdict

### Après correctif (2026-07-29)

La feature compte désormais **30 scénarios** — les 9 d'origine plus 21 cas
négatifs — tous sélectionnés et exécutant du vrai code de production :

- AID-001 `IMPLÉMENTÉ` — `DidDocument::verify` valide la version, les
  métadonnées de signature et les quatre clés sous leur codec propre ; le
  schéma wire est fermé (`deny_unknown_fields`) sur `DidDocument`, `DidKeys`,
  `SignatureBlock` et `EpochTransition`.
- AID-002 `IMPLÉMENTÉ` — `EpochTransition::verify(prev)` est remplacée par
  `verify_declaration(prev)` (déclaration seule, nommée comme telle) et
  `verify_succession(prev, next)` (triplet complet). Le `Then` Gherkin
  transmet et vérifie réellement `next_doc` : le faux positif est levé.
- AID-005 `IMPLÉMENTÉ` — chaque ligne Gherkin construit son défaut propre,
  appelle l'API de production correspondante et vérifie son verdict propre.
- AID-003 et AID-004 restent `OUVERT` : hors périmètre du correctif, ils
  exigent une décision d'architecture (source d'entropie de succession pour
  les créations Gateway, définition normative de la garde froide). Leurs
  marqueurs `@audit-partial @aid-003 @aid-004` restent en place dans la
  feature.

### Audit initial (conservé)

Les neuf scénarios étaient sélectionnés et exécutaient du vrai code Rust de
production. Aucun step de cette feature n'était vide, `@wip`, mocké ou remplacé
par un verdict `OnceLock`.

La feature ne constituait toutefois pas une preuve complète de son propre
contrat :

- 6 scénarios `PROUVÉ` au niveau précis qu'ils exercent ;
- 2 scénarios `PARTIEL` ;
- 1 scénario `FAUX POSITIF` au regard du résultat annoncé ;
- trois écarts d'implémentation affectant le fail-closed DID, la transition
  d'époque et l'indépendance/custody de la succession.

## Preuves rejouées

### Après correctif — RED puis GREEN

Les gates ci-dessous ont été rejoués sur un environnement Linux
(`rustc 1.95.0`), avec `aithos-client` monté en dépendance sœur à sa révision
`c6f6151`.

**Baseline `be2d098`, avant correctif :**

```text
cargo test --workspace --no-fail-fast
  627 tests unitaires et d'intégration passés, 0 échec
  aithos-bundle cucumber : 815 scenarios (815 passed), 3505 steps
```

**Preuve RED — les nouveaux cas échouent AVANT le correctif.** Le
`did.rs` d'origine a été remis en place derrière deux shims
(`verify_declaration` / `verify_succession` délégant à l'ancien `verify`),
afin que les nouveaux tests compilent contre l'ancienne sémantique :

```text
cargo test -p aithos-core --test a2_did
  test result: FAILED. 3 passed; 3 failed
  échecs : aid001_signed_but_semantically_invalid_documents_are_rejected
           aid001_unknown_wire_members_are_refused_not_dropped
           aid002_transition_binds_the_presented_successor_document

cargo test -p aithos-bundle --test cucumber
  836 scenarios (818 passed, 18 failed)
```

18 des 21 scénarios ajoutés échouent contre l'ancienne sémantique. Les
3 restants — signature d'une succession étrangère, `prev_did` ne désignant pas
le document fourni, signature root prétendant `#succession` — étaient déjà
refusés par le code d'origine mais n'étaient pas exprimés en Gherkin ; ils sont
ajoutés comme tests de non-régression, pas comme correctifs.

**GREEN — après correctif :**

```text
cargo test --workspace --no-fail-fast   → EXIT=0
  632 tests unitaires et d'intégration passés, 0 échec   (+5)
  aithos-bundle cucumber : 836 scenarios (836 passed), 3568 steps   (+21 / +63)

Tous les autres runners sont inchangés au scénario près :
  aithos-core cucumber      : 299 scenarios / 1422 steps
  aithos-provider cucumber  : 151 scenarios /  992 steps
  autres runners            :  27 / 21 / 12 / 12 / 6 scénarios, inchangés
```

Détail des +5 tests : 3 dans `a2_did.rs` (AID-001 sémantique, AID-001 wire
fermé, AID-002 triplet) et 2 dans le nouveau
`aithos-bundle/tests/aid_identity_surfaces.rs` (rejeu sur `Bundle::open` /
`Bundle::verify` et sur `verify_chain`, l'appel exact que délègue l'export
WASM public).

Détail des +21 scénarios Gherkin : 7 documents DID correctement re-signés mais
sémantiquement invalides, 3 membres wire inconnus, 10 transitions ne liant pas
leur successeur, 1 transition root prétendant `#succession`.

**Gates transverses :**

```text
cargo fmt --all -- --check
  1 seul écart, PRÉEXISTANT et hors périmètre :
  aithos-gateway/src/core_bridge.rs:1355 (walk_cert_chains_censused)
  → volontairement NON reformaté pour garder le diff du correctif fermé.

cargo clippy --workspace --all-targets
  aucun avertissement nouveau attribuable aux fichiers modifiés.
```

### Audit initial (conservé)

#### Gherkin

Résultat ciblé observé :

```text
1 feature
6 rules
9 scenarios (9 passed)
30 steps (30 passed)
```

Le filtre a été effectué sur les neuf noms de scénarios. Le compte final a été
contrôlé explicitement afin qu'une sélection vide ne puisse pas être prise pour
un succès.

#### Tests de conformité

```text
cargo test -p aithos-core --test a1_genesis --test a2_did

a1_genesis: 4 passed
a2_did:     3 passed
```

Les vecteurs A1/A2 figent les dérivations, encodages et JCS positifs contre des
valeurs générées indépendamment. Après correctif, `a2_did` compte 6 tests et le
JCS positif A2 reste byte-identique.

### Sondes négatives hors dépôt (audit initial)

Une sonde temporaire utilisant uniquement les API publiques d'alors avait
confirmé les acceptations suivantes :

```text
signed malformed non-root keys accepted: true
signed wrong version/alg/fragment accepted: true
unknown unsigned wire field ignored and accepted: true
transition to malformed DID accepted: true
transition to same DID accepted: true
```

Ces cinq acceptations sont désormais des refus, chacun couvert par un test
versionné dans le dépôt. La sonde temporaire n'en faisait pas partie ; elle
n'est plus nécessaire.

## Cartographie des preuves

| Objet | Source principale | Rôle dans l'audit |
|---|---|---|
| Contrat Gherkin | [`features/a-identity.feature`](../../../features/a-identity.feature) | Texte normatif testé |
| Steps | [`aithos-bundle/tests/cucumber.rs`](../../../rust/crates/aithos-bundle/tests/cucumber.rs) | Entrées, appels et assertions réellement exécutés |
| Genesis | [`aithos-core/src/keys.rs`](../../../rust/crates/aithos-core/src/keys.rs) | `MasterSeed`, `OwnerKeys`, succession |
| DID et transition | [`aithos-core/src/did.rs`](../../../rust/crates/aithos-core/src/did.rs) | Construction et vérification |
| Vecteurs indépendants | [`a1-genesis.json`](../../../vectors/a1-genesis.json), [`a2-did.json`](../../../vectors/a2-did.json) | Preuves positives byte-exactes |
| Ouverture Bundle | [`aithos-bundle/src/bundle.rs`](../../../rust/crates/aithos-bundle/src/bundle.rs) | Consommation réelle de `did.json` |
| Création Gateway | [`aithos-gateway/src/core_bridge.rs`](../../../rust/crates/aithos-gateway/src/core_bridge.rs) | Dérivation/cérémonie effective de succession |
| Custody CLI | [`aithos-cli/src/main.rs`](../../../rust/crates/aithos-cli/src/main.rs), [`custody.rs`](../../../rust/crates/aithos-cli/src/custody.rs) | Stockage des secrets owner et succession |
| Dépôt Provider | [`aithos-provider/src/artifacts.rs`](../../../rust/crates/aithos-provider/src/artifacts.rs) | Sémantique actuelle de remplacement `did.json` |

## Matrice scénario par scénario

Statuts **après correctif**. La colonne « avant » rappelle l'audit initial.

| # | Scénario | Avant | Après | Observation |
|---:|---|---|---|---|
| 1 | Same seed → same identity | `PROUVÉ` | `PROUVÉ` | Deux appels réels à `OwnerKeys::genesis`; A1 fixe aussi les trois sorties publiques. |
| 2 | Different seeds → no shared public key | `PROUVÉ` | `PROUVÉ` | Deux seeds distincts alimentent réellement la dérivation ; la propriété cryptographique générale reste fondée sur BLAKE3, pas sur l'exhaustivité de deux fixtures. |
| 3 | Three keys pairwise distinct | `PROUVÉ` | `PROUVÉ` | Les trois clés réelles sont comparées et proviennent de trois contextes de dérivation distincts. |
| 4 | Seed exactly 32 bytes | `PROUVÉ` | `PROUVÉ` | `MasterSeed::from_slice` impose `[u8; 32]`; le test A1 couvre 31 et 33 octets. |
| 5 | Succession independent and cold | `PARTIEL` | `PARTIEL` | Inchangé — AID-003/AID-004 hors périmètre du correctif ; les marqueurs restent. |
| 6 | DID lists four public keys | `PROUVÉ` | `PROUVÉ` | `DidDocument::build` est appelé ; A2 fixe le document JCS positif byte-exact. |
| 7 | A DID document altered after signing fails closed | `PARTIEL` | `PROUVÉ` | Renommé pour dire exactement ce qu'il prouve : une altération post-signature. Les autres familles de défaut ont leurs propres scénarios (8, 9). |
| 8 | Correctly signed but semantically invalid DID document (Outline ×7) | — | `PROUVÉ` | **Nouveau.** Clé content non-multibase, content en codec X25519, kex en codec Ed25519, succession malformée, version, algorithme et fragment non supportés — chacun correctement re-signé sous sa propre clé root. |
| 9 | Unknown member on the DID wire (Outline ×3) | — | `PROUVÉ` | **Nouveau.** Membre inconnu au niveau racine, dans `keys` et dans `signature` : refusé à la désérialisation, jamais supprimé silencieusement. |
| 10 | Succession-signed epoch transition accepts successor | `FAUX POSITIF` | `PROUVÉ` | Le `Then` reçoit désormais `next_doc` et le vérifie via `verify_succession`. |
| 11 | Anything else, including root, is rejected | `PROUVÉ` | `PROUVÉ` | Inchangé. |
| 12 | A transition that does not bind its successor (Outline ×10) | — | `PROUVÉ` | **Nouveau.** Autre document successeur présenté, successeur altéré, successeur re-signé mais malformé, `next_did == prev_did`, `next_did` malformé, `next_did` non-`did:aithos`, succession étrangère, `prev_did` étranger, version et algorithme non supportés. |
| 13 | Root signing while claiming the succession fragment | — | `PROUVÉ` | **Nouveau** en Gherkin (le cas existait déjà côté vecteur A2). |

## Écarts et implémentations requises

### AID-001 — Vérification DID stricte et fermée

**Priorité : P1 — IMPLÉMENTÉ (2026-07-29)**

#### Constat (avant correctif)

`DidDocument::verify` valide aujourd'hui :

- le décodage de `keys.root` ;
- la liaison `id == did:aithos:<root>` ;
- la signature sous cette racine.

Il ne valide pas :

- `keys.content` comme clé Ed25519 ;
- `keys.kex` comme clé X25519 ;
- `keys.succession` comme clé Ed25519 ;
- `aithos-did-core == DID_VERSION` ;
- `signature.alg == "ed25519"` ;
- `signature.key == "#root"` ;
- l'absence de champs JSON inconnus.

Les consommateurs réels désérialisent le JSON vers `DidDocument`, puis appellent
ce même vérificateur. Sans `deny_unknown_fields`, un champ ajouté au wire peut
être supprimé avant la reconstruction du JCS vérifié.

#### Implémentation livrée

- [x] Schéma fermé (`deny_unknown_fields`) sur `DidDocument`, `DidKeys`,
  `SignatureBlock` et `EpochTransition`.
- [x] `aithos-did-core == DID_VERSION`, `signature.alg == "ed25519"` et
  `signature.key == "#root"` validés explicitement, AVANT la signature.
- [x] Les quatre clés décodées sous leur codec propre : `root`, `content` et
  `succession` en Ed25519 (avec construction de `VerifyingKey`, donc contrôle
  du point), `kex` en X25519. Une clé au bon format mais au mauvais codec est
  refusée.
- [x] Liaison `id ↔ root` et vérification Ed25519 conservées à l'identique.
- [x] Une erreur `InvalidDidDocument` distincte par famille de défaut.
- [x] Surfaces vérifiées : `Bundle::open`, `Bundle::verify`,
  `mandate::verify_chain` (l'appel exact que délègue l'export WASM public),
  `aithos-gateway/core_bridge` et `control`, `aithos-provider/control`,
  `aithos-client` — toutes appellent `DidDocument::verify` sans parser
  permissif parallèle.

Deux constantes publiques ont été ajoutées pour que le fragment et
l'algorithme cessent d'être des littéraux dispersés : `SIGNATURE_ALG`,
`ROOT_FRAGMENT`, `SUCCESSION_FRAGMENT`.

#### Tests RED livrés

- [x] `content` non-multibase et `content` en codec X25519.
- [x] `kex` en codec Ed25519 et `kex` tronquée.
- [x] `succession` malformée.
- [x] Mauvaise version, mauvais algorithme, mauvais fragment — chacun
  correctement re-signé afin d'isoler le contrôle sémantique. Un contrôle
  positif re-signe le document intact et vérifie qu'il passe, pour qu'aucun
  refus ne puisse être imputé au ré-encodage.
- [x] Membre inconnu au niveau racine, dans `keys` et dans `signature`.
- [x] Rejeu de tous ces cas via `Bundle::open`, `Bundle::verify` et
  `verify_chain` (`aithos-bundle/tests/aid_identity_surfaces.rs`).

#### Clôture

Tous les cas négatifs sont refusés par le même verdict Core. A2 positif reste
byte-identique (`a2_did_document_matches_and_verifies` inchangé). Aucune
surface ne réinterprète ni ne supprime silencieusement un champ avant
vérification.

**Réserve, à trancher hors de ce correctif :** le remplacement `did.json` du
Provider (`artifacts::deposit_did`) reste une vérification distincte, signée
sous `#succession` du document stocké, et n'appelle donc pas `verify()` sur le
document entrant. C'est l'arbitrage nommé de la décision 2 ci-dessous, laissé
inchangé : le durcir ici aurait cassé le fixture P9 `did_rotation_ok` sans
décision préalable.

### AID-002 — Lier la transition au document successeur réel

**Priorité : P1 — IMPLÉMENTÉ (2026-07-29)**

#### Constat (avant correctif)

`EpochTransition::verify(&prev_doc)` ne reçoit pas le document successeur. Il
peut accepter une transition correctement signée dont `next_did` est malformé,
identique à `prev_did`, absent du store ou sans rapport avec le document ensuite
présenté.

Le step Gherkin construit bien `next_doc`, mais n'en transmet que l'identifiant.
Son assertion « successor DID document is accepted » ne vérifie donc aucun
document successeur.

`f-gamma.feature` refuse séparément deux DID identiques dans les faits d'une
opération `rotate identity`, mais ne vérifie ni la signature d'époque ni le
document successeur. Le Provider possède par ailleurs une logique de
remplacement `did.json` sous le même DID, explicitement distincte de
l'artefact d'époque §10.4.

#### Implémentation livrée

`EpochTransition::verify(prev)` est **supprimée**. Deux API la remplacent, dont
les noms disent exactement ce qu'elles prouvent :

- `verify_declaration(&prev_doc)` — la déclaration seule. Documentée comme
  insuffisante pour accepter un successeur, et pointant vers l'autre API.
- `verify_succession(&prev_doc, &next_doc)` — le verdict complet §10.4, le seul
  qui autorise à remplacer `prev_doc` par `next_doc`.

- [x] API recevant `prev_doc`, `next_doc` et la transition.
- [x] Les DEUX documents passent le validateur strict AID-001.
- [x] `prev_did == prev_doc.id`.
- [x] `next_did == next_doc.id`.
- [x] `prev_doc.id != next_doc.id`, et `next_did != prev_did` dès la
  déclaration.
- [x] `next_did` doit être un identifiant `did:aithos:` dont la partie racine
  décode en clé Ed25519 — un `next_did` malformé est refusé avant même qu'un
  document successeur soit présenté.
- [x] Version, algorithme et fragment de la transition validés.
- [x] Signature vérifiée sous la succession du document précédent.
- [x] Aucune API nommée comme acceptant un successeur ne peut ne vérifier que
  la déclaration : le seul appelant qui l'accepte s'appelle
  `verify_declaration`.

Aucun code de production ne consommait `EpochTransition::verify` : les seuls
appelants étaient `a2_did.rs` et le runner Cucumber, tous deux migrés. Le
changement d'API ne casse donc aucune surface.

#### Tests RED livrés

- [x] `next_did` malformé et `next_did` non-`did:aithos`.
- [x] Même DID avant/après.
- [x] Transition valide mais autre document successeur présenté — un troisième
  identifiant, sans rapport avec les deux autres.
- [x] Document successeur altéré après signature.
- [x] Document successeur correctement re-signé mais sémantiquement malformé —
  il doit passer le MÊME verdict strict que le prédécesseur.
- [x] Transition signée par root avec `#root`.
- [x] Transition signée par root mais annonçant `#succession`.
- [x] Transition signée par la succession d'un autre DID.
- [x] `prev_did` désignant un document autre que celui fourni.
- [x] Version et algorithme non supportés, chacun correctement re-signé.
- [x] Cas positif complet : le JCS A2 de la transition reste byte-identique et
  `verify_succession(doc1, doc2)` l'accepte.

#### Clôture

Le `Then` Gherkin transmet et valide réellement `next_doc`. La différence du
Provider est explicitement nommée (voir la réserve AID-001 et la décision 2) et
ne prétend pas implémenter §10.4.

**Reste ouvert :** l'extension des vecteurs A2 avec des cas négatifs générés
INDÉPENDAMMENT (Python). Les négatifs livrés sont dérivés en test à partir des
positifs A2 figés — c'est une preuve réelle du verdict, mais pas une preuve
croisée d'implémentation. Voir AID-005.

### AID-003 — Supprimer toute dérivation de succession depuis le master owner

**Priorité : P1 — OUVERT** (hors périmètre du correctif 2026-07-29 : change le
comportement de création déterministe d'Ethos et exige une décision sur la
source d'entropie de succession)

#### Constat

Le Core pur est correctement conçu : `succession_from_entropy` ne reçoit pas
`MasterSeed`. L'onboarding principal Gateway tire également deux valeurs
d'entropie successives.

En revanche, `owner_init_journal` et `owner_init_context` appellent
`derived_succession(master, kind, label)`. La succession est donc
reconstructible depuis le même master que les clés owner. La compromission de
ce master compromet alors aussi l'autorité censée permettre d'en sortir.

#### Implémentation attendue

- [ ] Supprimer `derived_succession(master, ...)`.
- [ ] Exiger une entropie ou une capacité de succession indépendante lors de
  la création d'un journal ou contexte.
- [ ] Ne jamais fournir le master owner à un composant chargé de fabriquer la
  succession.
- [ ] Définir le format de référence publique/custody nécessaire pour les
  créations déterministes de plusieurs Ethos, sans réintroduire une dérivation
  depuis `S`.
- [ ] Ajouter une garde d'architecture empêchant la réapparition d'une
  dérivation de succession depuis un secret owner.

#### Tests RED requis

- [ ] Même master owner + deux entropies de succession → mêmes clés owner,
  successions différentes.
- [ ] Changement du master owner sans réutiliser la custody de succession →
  aucune autorité de succession implicite.
- [ ] Création Gateway de journal et contexte exigeant explicitement la source
  de succession indépendante.

#### Critère de clôture

Aucun chemin de production ne peut recalculer la clé privée de succession à
partir de `S`, d'un master d'entreprise ou d'une clé dérivée owner.

### AID-004 — Définir et appliquer la garde froide

**Priorité : P1 — DÉCISION REQUISE** (hors périmètre du correctif
2026-07-29)

#### Constat

Le mode CLI managé stocke `master_seed_hex` et `succession_seed_hex` dans le
même objet `KeyMaterial` et le même backend. Cela ne correspond pas à
« papier, HSM ou threshold custody, jamais sur un appareil qui exécute des
agents ».

Le Core retourne par ailleurs un `SigningKey` Ed25519 générique. Le type
n'empêche pas un appelant de signer autre chose qu'une transition d'époque.

#### Implémentation attendue après décision

- [ ] Définir opérationnellement ce que « cold » impose aux surfaces
  supportées : backend distinct obligatoire, HSM, capacité externe, export
  one-shot, ou politique explicitement hors logiciel.
- [ ] Séparer le matériel de succession de `KeyMaterial` si la séparation de
  backend est normative.
- [ ] Préférer une capacité typée `SuccessionSigner` ne signant qu'un
  `EpochTransition` à l'exposition stable d'un `SigningKey` générique.
- [ ] Refuser une configuration qui prétend être cold tout en utilisant la
  custody active du master, si cette séparation est normative.
- [ ] Clarifier « only S is ever backed up » : `S` est l'unique sauvegarde des
  clés owner dérivées, mais la succession reste un secret indépendant à
  conserver dans une custody distincte.

#### Tests requis

- [ ] Le profil managé ne sérialise jamais master et succession dans le même
  objet ou backend lorsque le mode cold est revendiqué.
- [ ] La surface stable ne propose aucune signature arbitraire avec la
  succession.
- [ ] Une succession indisponible n'est jamais régénérée depuis le master.

#### Critère de clôture

La règle « independent and cold » possède une définition testable, appliquée
par les surfaces, et pas seulement un commentaire dans `keys.rs`.

### AID-005 — Renforcer le contrat Gherkin et les vecteurs

**Priorité : P2 — IMPLÉMENTÉ EN MAJEURE PARTIE (2026-07-29)**

#### Implémentation livrée

- [x] Le scénario de signature altérée est conservé et renommé
  « A DID document altered after signing fails closed » — il dit désormais
  exactement ce qu'il prouve.
- [x] Scenario Outline de documents DID signés mais sémantiquement invalides
  (7 lignes) couvrant AID-001.
- [x] Scenario Outline de membres wire inconnus (3 lignes).
- [x] Le `Then` de transition vérifie le triplet
  précédent/transition/successeur.
- [x] Scenario Outline des négatifs AID-002 (10 lignes) plus le cas root
  prétendant `#succession`.
- [ ] **Reste ouvert** — scénario de cérémonie de succession utilisant l'API
  réelle de création d'identité plutôt que deux constantes choisies dans le
  step. Dépend d'AID-003 : tant que `owner_init_journal` et
  `owner_init_context` dérivent la succession du master, exercer « la vraie
  surface de création » figerait précisément le comportement à corriger.
- [ ] **Reste ouvert** — extension d'A2 avec des cas négatifs générés
  indépendamment en Python. Les négatifs livrés sont dérivés en test des
  positifs A2 figés ; ils prouvent le verdict, pas l'indépendance
  d'implémentation.
- [ ] **Reste ouvert** — gate ciblé échouant si le nombre exécuté n'est pas
  exactement celui attendu. Le runner Cucumber du dépôt n'expose pas de
  filtre ciblé ni d'assertion de compte ; l'ajouter touche l'outillage de
  test commun à 18 features et sortait du périmètre « aucune régression ».
  Le compte est pour l'instant contrôlé à la main : 30 scénarios pour
  `a-identity.feature`, 836 pour le runner `aithos-bundle`.

#### Clôture partielle

Chaque ligne Gherkin ajoutée construit son défaut propre, appelle l'API de
production correspondante et vérifie son verdict propre. Aucun step n'est
`@wip`, vide, mocké ni adossé à un verdict `OnceLock`. Les tests A1/A2
autonomes restent des preuves complémentaires et A2 positif est inchangé.

## Décisions à trancher

1. **API de transition.** ~~Remplacer `EpochTransition::verify(prev)` par une
   API complète, ou conserver une primitive de signature clairement nommée et
   ajouter un validateur de succession distinct ?~~ **TRANCHÉ (2026-07-29) :**
   les deux. `verify(prev)` est supprimée ; `verify_declaration(prev)` porte la
   primitive sous un nom qui ne promet rien sur le successeur, et
   `verify_succession(prev, next)` porte le verdict complet. Aucun code de
   production ne consommait l'ancienne API.
2. **Provider.** Le remplacement d'un `did.json` sous le même DID reste-t-il
   une opération Provider spécifique, ou doit-il adopter la transition
   d'époque vers un nouveau DID ?
3. **Custody froide.** La séparation est-elle imposée techniquement par Aithos,
   ou seulement documentée comme responsabilité de l'opérateur ?
4. **Capacité typée.** La clé privée de succession doit-elle être inaccessible
   au Core applicatif derrière une interface de signature bornée ?

Ces décisions doivent être prises dans la spec ou une décision d'architecture
avant que l'implémentation ne les fige implicitement.

## Définition de terminé

La note pourra passer à **VÉRIFIÉE** lorsque :

- [ ] AID-001 à AID-005 sont clôturés ou explicitement arbitrés hors périmètre
  — **AID-001, AID-002 et AID-005 (majeure partie) faits ; AID-003 et AID-004
  restent ouverts** ;
- [x] aucun scénario `a-identity.feature` n'est `@wip`, proxy ou vide ;
- [ ] le runner ciblé exécute exactement le nombre attendu de scénarios
  — compte contrôlé à la main (30), pas encore par un gate ;
- [x] les tests A1/A2 positifs restent byte-exacts ;
- [x] les nouveaux négatifs échouent avant correction puis passent après
  — 18 des 21 scénarios ajoutés et 3 des 6 tests `a2_did` sont RED contre
  l'ancienne sémantique ; les autres sont des non-régressions assumées ;
- [x] Core, Bundle, WASM (via `verify_chain`), Gateway et Provider partagent le
  même verdict pour les objets DID — à la réserve nommée du remplacement
  `did.json` Provider (décision 2) ;
- [x] `cargo test -p aithos-core --test a1_genesis --test a2_did` passe
  (4 + 6 tests) ;
- [x] le runner Cucumber passe (836 scénarios, 3568 steps) ;
- [x] les gates workspace et Clippy passent ; le formatage passe hors d'un
  écart préexistant et hors périmètre dans `aithos-gateway` ;
- [x] la révision Git de clôture et les résultats exacts sont inscrits ici.

## Historique

| Date | État | Note |
|---|---|---|
| 2026-07-29 | `PARTIELLEMENT CLÔTURÉE` | Correctif AID-001, AID-002 et AID-005 sur `fix/aid-001-002-005-identity-fail-closed`. `DidDocument::verify` durcie et schéma wire fermé ; `EpochTransition::verify` remplacée par `verify_declaration` + `verify_succession` ; feature portée de 9 à 30 scénarios ; nouveau test de rejeu de surfaces. Baseline 627 tests / 815 scénarios → 632 tests / 836 scénarios, 0 échec, aucune régression. AID-003 et AID-004 restent ouverts et leurs marqueurs restent dans la feature. |
| 2026-07-29 | `ANNOTÉE` | Marqueurs inline ajoutés sans exclusion : `@audit-partial` sur AID-001/AID-003/AID-004 et `@audit-false-positive` sur AID-002 ; rejeu ciblé inchangé, 9 scénarios et 30 steps passés. |
| 2026-07-29 | `OUVERTE` | Audit initial : neuf scénarios verts, trois écarts d'implémentation principaux et deux renforcements de preuve requis. |

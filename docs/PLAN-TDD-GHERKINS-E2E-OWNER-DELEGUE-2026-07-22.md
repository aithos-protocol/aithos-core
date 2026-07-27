# Plan TDD — couverture Gherkin E2E owner et délégué

Date : 2026-07-22

Document source : `docs/TODO-AUDIT-GHERKINS-E2E-2026-07-22.md`.

Prompt de reprise associé :
`docs/PROMPT-REPRISE-TDD-GHERKINS-E2E-OWNER-DELEGUE-2026-07-22.md`.

## Objectif

Construire par TDD une preuve fonctionnelle complète, scenario par scenario,
des opérations owner et délégué sur Core/Bundle, client natif, client WASM et
SDK. Le provider est inclus comme dépendance de la frontière E2E : sans un vrai
transport HTTP, un CAS et un redémarrage sur stockage durable, le SDK ne peut
pas être qualifié E2E.

La chaîne à prouver est :

`intention -> clé/capacité -> mandat -> Core/Bundle -> plan client -> SDK -> provider HTTP/CAS -> restart -> téléchargement -> store vierge -> cold verify -> lecture`

Le même `operation_ref`, le même acteur, le même manifest head et le même
package doivent être suivis d'un bout à l'autre.

## Ce que « couverture complète » signifie ici

La couverture est exhaustive sur les dimensions sémantiques fermées du
protocole, pas sur toutes les chaînes de caractères ou tous les instants
possibles :

| Dimension | Valeurs obligatoires |
|---|---|
| acteur | owner, leaf grantee |
| zone | public, circle, self |
| contenu | list, read, create, edit, delete |
| structure | create folder, rename, move, delete empty, delete subtree, tag edit |
| autorité | zone, dir, tag, exact id, combinaison interdite, hors périmètre |
| possession | bonne clé + chaîne, clé seule, chaîne seule, mauvaise clé, ligne cryptographique absente |
| temps | valide, avant fenêtre, expiré, révoqué juste avant effet |
| publication | genesis, successor, CAS correct, CAS stale, replay idempotent |
| stockage | MemStore, FsStore, provider durable redémarré, store local vierge |
| preuve | owner authorship, grantee authorship, Gamma, changeset, evidence, manifest |
| surface | Core/Bundle, client natif, WASM, SDK Node, HTTP provider |
| panne | avant effet, pendant dérivation, avant manifest, CAS perdu, objet manquant/substitué |

Un scénario n'est GREEN que si ses propres paramètres atteignent l'API de
production. Les helpers globaux `cbN_result()` ne sont pas autorisés dans les
nouveaux steps.

## Matrice fonctionnelle canonique

### Opérations owner

Toutes les cellules suivantes doivent être acceptées par Core, le client et le
roundtrip SDK :

| ID | zone | opération | publication attendue |
|---|---|---|---|
| OWN-PUB-LIST | public | list | aucune mutation |
| OWN-PUB-READ | public | read | aucune mutation |
| OWN-PUB-CREATE | public | create | édition owner |
| OWN-PUB-EDIT | public | edit | édition owner |
| OWN-PUB-DELETE | public | delete | édition owner |
| OWN-CIR-LIST | circle | list | aucune mutation |
| OWN-CIR-READ | circle | read | aucune mutation |
| OWN-CIR-CREATE | circle | create | édition owner chiffrée |
| OWN-CIR-EDIT | circle | edit | édition owner chiffrée |
| OWN-CIR-DELETE | circle | delete | édition owner chiffrée |
| OWN-SELF-LIST | self | list | aucune mutation |
| OWN-SELF-READ | self | read | aucune mutation |
| OWN-SELF-CREATE | self | create | édition owner opaque |
| OWN-SELF-EDIT | self | edit | édition owner opaque |
| OWN-SELF-DELETE | self | delete | édition owner opaque |

### Opérations déléguées

Cette table reprend exactement la matrice L actuellement verte par proxy. Elle
doit être exécutée par chaque ligne d'exemple :

| ID | zone | opération | autorité | verdict |
|---|---|---|---|---|
| DEL-PUB-LIST | public | list | `read.public#dir=projects` | accepté |
| DEL-PUB-READ | public | read | `read.public#id=note` | accepté |
| DEL-PUB-CREATE | public | create | `append.public#dir=projects` | accepté |
| DEL-PUB-EDIT | public | edit | `edit.public#id=note` | accepté |
| DEL-PUB-DELETE | public | delete | `delete.public#id=note` | accepté |
| DEL-CIR-LIST | circle | list | `read.circle#dir=projects` | accepté |
| DEL-CIR-READ | circle | read | `read.circle#id=note` | accepté |
| DEL-CIR-CREATE | circle | create | `append.circle#dir=projects` | accepté |
| DEL-CIR-EDIT | circle | edit | `edit.circle#id=note` | accepté |
| DEL-CIR-DELETE | circle | delete | `delete.circle#id=note` | accepté |
| DEL-SELF-LIST | self | list | `read.self#dir=sealed` | accepté |
| DEL-SELF-READ | self | read | `read.self#id=opaque-note` | accepté |
| DEL-SELF-CREATE-ZONE | self | create | `append.self` | accepté |
| DEL-SELF-CREATE-ID | self | create | `append.self#id=preallocated` | accepté |
| DEL-SELF-EDIT | self | edit | `edit.self#id=opaque-note` | accepté |
| DEL-SELF-DELETE | self | delete | `delete.self#id=opaque-note` | accepté |
| DEL-SELF-EDIT-DIR | self | edit | `edit.self#dir=sealed` | refusé |
| DEL-SELF-DELETE-TAG | self | delete | `delete.self#tag=private` | refusé |

### Matrice E2E de mutations à publier

Les opérations list/read sont vérifiées dans un lot de lecture séparé. Le gate
de publication doit exécuter au minimum ces 19 mutations complètes :

| acteur | zone | mutations |
|---|---|---|
| owner | public | create, edit, delete |
| owner | circle | create, edit, delete |
| owner | self | create, edit, delete |
| grantee | public | create-dir, edit-id, delete-id |
| grantee | circle | create-dir, edit-id, delete-id |
| grantee | self | create-zone, create-preallocated-id, edit-id, delete-id |

## Lot 0 — harnais qui empêche les faux verts

### Gherkins à créer

Fichier SDK : `aithos-sdk/features/harness-conformance.feature`.

#### HARN-001 — Every release scenario is selected exactly once

- Given le catalogue Core, client, WASM, SDK et provider ;
- When le manifest de tests est construit ;
- Then chaque scénario release appartient à exactement un runner ;
- And aucun `@wip`, `@superseded` ou scénario sans steps ne compte comme GREEN.

#### HARN-002 — Every outline row reaches production

Scenario Outline avec trois lignes sentinelles différentes. Chaque ligne doit
produire un `operation_ref` ou un refus différent et observable ; un résultat
global mis en cache fait échouer le test.

#### HARN-003 — One trace follows one package across every process

Le harnais enregistre `did`, actor key, mandate ids, `operation_ref`, expected
head, new head, package digest et downloaded head. Toute substitution de
fixture ou de package échoue.

#### HARN-004 — A provider restart is a real process restart

Le PID doit changer, le port peut changer, les objets et heads doivent survivre
par un backend durable de test. Conserver seulement un `Arc<MemObjects>` entre
deux routers ne satisfait pas ce scénario.

#### HARN-005 — No fake plan can satisfy an SDK E2E scenario

Le plan doit provenir de `@aithos/client`, passer `verify` ou `verifyAgainst`,
et ses artefacts doivent être ceux envoyés. Les objets JavaScript littéraux
utilisés dans `test/provider-client.test.js` restent de bons tests unitaires,
mais ne peuvent pas alimenter le runner E2E.

### Développement associé

- [ ] Ajouter `@cucumber/cucumber` et `test:gherkin` à `aithos-sdk/package.json`.
- [ ] Créer `aithos-sdk/test/gherkin/world.js`, `steps/*.js` et un manifest
  partagé des scénarios release.
- [ ] Ajouter une commande racine, par exemple
  `scripts/test-owner-delegate-e2e.sh`, qui compile Core, le package client
  WASM/npm, le provider et lance ensuite le runner SDK.
- [ ] Ajouter un backend provider durable réservé aux tests, derrière les
  traits existants : `FsObjects`, `FsHeads` et `FsNonces`, ou un unique backend
  SQLite transactionnel. Il doit permettre un redémarrage de binaire sans AWS.
- [ ] Ajouter un superviseur de test qui démarre `aithos-store-api` sur un port
  éphémère, attend `/healthz`, l'arrête proprement et le redémarre sur le même
  répertoire.
- [ ] Produire un bootstrap public de test depuis le vrai genesis client au
  lieu de maintenir un DID parallèle codé en dur.
- [ ] Faire échouer la CI si les compteurs de scénarios sélectionnés changent
  sans mise à jour explicite du manifest.

### Gate GREEN

Le harnais doit échouer volontairement si un step est remplacé par
`cb9_assert_green()`, si un plan fake est injecté, ou si le provider n'a pas
réellement changé de PID.

## Lot 1 — Core/Bundle : opérations owner réelles

### Gherkins à réécrire

Fichier existant : `features/d-bundle.feature`.

#### CORE-OWN-001 — The owner performs every content operation

Scenario Outline de 15 lignes, une par cellule `OWN-*` de la matrice. Pour
chaque ligne : construire un Bundle propre, exécuter l'opération réelle, puis
vérifier le corps/liste ou l'édition produite. Pour les 9 mutations, vérifier
Gamma et fresh reopen.

#### CORE-OWN-002 — Owner mutations commit atomically

Rejouer les 12 frontières MemStore/FsStore de la ligne 90 et les deux succès
MemStore/FsStore de la ligne 115. Le paramètre `boundary` doit sélectionner un
vrai point d'injection, et chaque refus doit comparer toutes les clés
canoniques avant/après.

#### CORE-OWN-003 — Narrow owner capabilities cannot cross purpose

Rejouer les quatre lignes sign/open/wrap de la ligne 130 avec de vraies
capabilities distinctes et les mauvais objets annoncés.

#### CORE-OWN-004 — Store roots cannot be escaped

Rejouer chaque cas de la ligne 147, y compris les symlinks FsStore, dans des
répertoires temporaires séparés.

### Développement associé

- [x] Remplacer les steps CB7/CB8/CB12 par un `ProtocolWorld` propre à chaque
  scénario et par des appels `Bundle` réels.
- [x] Exposer les points de panne uniquement sous une interface de test typée,
  sans branche permissive dans le code de production.
- [ ] Si un cas owner échoue, corriger `bundle.rs`, `publication.rs` ou le
  store concerné ; aucune nouvelle fonctionnalité owner n'est théoriquement
  nécessaire, car les tests CB indiquent que les primitives existent déjà.

### Gate GREEN

15 cellules owner + toutes les frontières atomiques passent sans aucun appel
à `cbN_result` dans leurs steps.

## Lot 2 — Core/Bundle : autorité et opérations déléguées

### Gherkins à réécrire

#### CORE-DEL-001 — One pure operation enforces every zone rule

Fichier : `features/l-delegated-writes.feature:103`. Exécuter les 18 lignes
`DEL-*` de la matrice. Une acceptation doit vérifier l'effet exact, l'acteur
Gamma et le reopen. Un refus doit vérifier bundle, Gamma et manifest inchangés.

#### CORE-DEL-002 — Possession and authority are independent

Fichiers : `features/l-delegated-writes.feature:132` et
`features/e-mandates.feature:135`.

Lignes obligatoires :

- bonne ligne + bonne clé + chaîne valide -> lisible ;
- clé/ligne valide sans chaîne -> non autorisé ;
- chaîne valide sans ligne -> autorisé mais indéchiffrable ;
- ligne sœur + chaîne valide -> indéchiffrable ;
- chaîne sans clé -> refus ;
- mauvaise clé -> refus ;
- chaîne révoquée -> refus par replay réel.

#### CORE-DEL-003 — Exact id grants mutate only one section

Fichier : `features/e-mandate-sections.feature`. Remplacer les proxies des
quatre scénarios lignes 13–31 et exécuter les matrices id/zone déjà présentes
lignes 68–132.

#### CORE-DEL-004 — Current authority is checked immediately before effect

Fichier : `features/l-delegated-writes.feature:167`.

- session ouverte puis expiration ;
- session ouverte puis revocation ;
- clé toujours capable d'ouvrir une ancienne ligne mais autorité inactive ;
- chaque cas laisse le snapshot inchangé.

#### CORE-DEL-005 — Any delegated refusal rolls back everything

Fichier : `features/l-delegated-writes.feature:179`. Injecter un échec de
validation Gamma après préparation cryptographique mais avant linéarisation et
vérifier l'absence de blob, header, wrap, Gamma, evidence et manifest orphelin.

### Développement associé

- [ ] Réécrire les steps `cb9_given/when/then` pour construire la zone,
  l'opération, le mandat et le verdict depuis les paramètres.
- [x] Utiliser `Bundle::grantee_content_operation` et les APIs de session
  réelles ; ne pas réimplémenter `covers_section_op` dans le test.
- [ ] Compléter les APIs Bundle seulement si les RED révèlent un trou sur
  circle/self, preallocated self SID ou rollback.
- [ ] Rendre observable dans le résultat de test l'`operation_ref` réellement
  créé et l'acteur, sans exposer de secret.

### Gate GREEN

Les 18 lignes déléguées, les 7 combinaisons possession/autorité et les deux
changements temporels passent indépendamment.

## Lot 3 — Core/Bundle : structure, révocation et cryptographie

### Gherkins à réécrire

#### CORE-STR-001 — Structural authority matrix

Fichier : `features/n-structural-mutations.feature:7`. Exécuter les 26 lignes
d'autorité réelles : read/list, create child, rename, delete empty, move et
delete subtree.

#### CORE-STR-002 — Derived structural consequences are atomic

Scénarios réels pour tag edit, move, subtree delete et self opaque, lignes
41–88. Vérifier index, tag views, roots, rotations, wraps, changeset et Gamma.

#### CORE-STR-003 — Seven injected structural failures are effect-free

Exécuter les 7 lignes de la ligne 70 avec snapshot byte-for-byte et reopen.

#### CORE-REV-001 — Revocation is one durable cryptographic cut

Fichier : `features/g-revocation.feature:129–160`.

- cold reopen du cut complet ;
- toutes les frontières de panne de la matrice existante ;
- entrée antérieure toujours attribuable ;
- mutation postérieure au cut refusée ;
- survivor lines et rotations exactes.

### Développement associé

- [x] Remplacer les steps `cb10_*` par les vraies opérations structurelles et
  de révocation paramétrées.
- [ ] Factoriser une transaction Bundle unique pour contenu dérivé, rotation,
  Gamma et publication si les RED montrent plusieurs commits observables.
- [x] Conserver le SID stable lors des moves et ne jamais publier la structure
  self dans les preuves.

### Gate GREEN

26 cas d'autorité + 7 pannes + tag/move/subtree/self + revocation cold reopen
passent avec des bundles indépendants.

## Lot 4 — Core/Bundle : édition normale et cold verification

### Gherkins à réécrire

#### CORE-ED-001 — One actor signs one normal edition

Fichier : `features/m-delegated-editions.feature:7`.

Quatre lignes : owner, leaf avec une chaîne complète, deux chaînes partielles,
chaîne sans preuve de clé. Vérifier la clé réellement signataire et l'absence
d'owner dans une édition grantee ordinaire.

#### CORE-ED-002 — Draft2 carriers are derived from the same candidate

Exécuter individuellement les scénarios lignes 36–138 :

- les 7 formes manifest draft1/draft2/unknown ;
- les 2 références changeset/evidence ;
- changeset fermé et trié ;
- acyclicité carriers/publication ;
- evidence non autorisante ;
- les 5 kinds d'evidence ;
- les 5 changements omis/inventés ;
- join operation/Gamma/authority.

#### CORE-ED-003 — Zone proofs survive cold replay

Scénarios lignes 148–171 : authorship public, réouverture sans capacité,
transition self opaque et présentation Gamma opposable.

#### CORE-ED-004 — Carrier defects fail before publication

Exécuter les 10 défauts ligne 180 et les 4 exports incomplets MemStore/FsStore
ligne 199. Chaque cas part de son propre candidat et vérifie zéro reachability.

#### CORE-COLD-001 — Owner and grantee history roundtrip in fresh stores

Fichier : `features/k-integration.feature:158–187`.

- MemStore et FsStore ;
- producteur détruit avant verify ;
- capacité grantee réintroduite puis retirée ;
- les 5 défauts d'artefacts existants.

### Développement associé

- [x] Remplacer `m_carrier_*` et `k_cold_round_trip_*` par un builder de
  scénario qui produit un unique `KeylessPublicationPackage`.
- [x] Faire assembler l'édition par le signataire réel via
  `assemble_draft2_candidate`/`export_keyless`, puis réutiliser exactement ce
  package dans `import_keyless` et `cold_verify`.
- [x] Compléter la vérification cold des manifests `authorized_by`, authorship
  et self transitions si un RED révèle encore un owner fallback.
- [ ] Exposer une structure de trace publique contenant actor, authority ids,
  operation refs, package digest et heads.

### Gate GREEN

Toutes les lignes de M et K passent avec un package unique par scénario. Aucun
step ne combine plusieurs résultats CB indépendants.

## Lot 5 — Core : replay, contraintes, obligations et vault

Ce lot clôt les faux verts P1 qui ne sont pas tous nécessaires au premier CRUD
multi-zone, mais qui sont nécessaires à la couverture fonctionnelle complète
d'un délégué.

### Gherkins à réécrire

#### CORE-SEM-001 — Append and cold replay share the same verdict

Réécrire les groupes proxy de `f-gamma.feature` lignes 161–594 et 620–736.
Chaque kind, faits et défaut doit construire sa propre entrée Gamma.

#### CORE-CONSTRAINT-001 — Every delegated constraint applies to the right operation

Réécrire `f-plus-constraints.feature` : U1 lignes 163–192, contraintes
versionnées lignes 337–405 et matrice d'applicabilité lignes 409–453.

Statut : **GREEN scenario-driven**. Les U1 action/inference, l'isolation v1,
les versions `max_children`, les extensions inconnues, les 23 cellules
d'applicabilité et la parité append/cold appellent leurs validateurs Core
propres sans résultat CB global.

#### CORE-OBLIGATION-001 — Every required receipt is bound to one occurrence

Réécrire `g-plus-obligations.feature` lignes 166–255 : R2, matcher, co-sign,
tier-X et parité fresh-store.

Statut : **GREEN scenario-driven**. R2 fermé, 25 refus typés, neuf matchers,
six consommations ciblées, co-sign, quatre tier-X et replay fresh-store ont
chacun leur observation issue des validateurs publics.

#### CORE-COUNT-001 — Delegated counters replay identically

Réécrire `h2-gamma-roots.feature` lignes 89–163, avec action, mutation,
publication, grant et total-consumption réellement distincts.

Statut : **GREEN scenario-driven**. Les 11 consommations, les profils
historiques, les 49 défauts compteurs/mandats et les trois formes de
publication sont validés séparément par `verify_delegated_counts` et
`verify_delegated_count_mandates`.

#### CORE-VAULT-001 — Connector catalog and vault authority are exact

Réécrire toute `o-connector-classes-vault.feature`, y compris les 4 CRUD
config, les combinaisons autorité/ligne, les rotations locales et les pannes.

### Développement associé

- [ ] Remplacer les proxies CB4/CB5/CB6/CB10 restants par des fixtures minimales
  paramétrées utilisant les mêmes validateurs publics que l'append/replay.
- [x] Supprimer les proxies des contraintes versionnées/applicabilité et des
  receipts U1/R2/matchers/obligations ciblées.
- [ ] Unifier le front door sémantique append/cold si deux chemins de verdict
  subsistent.
- [ ] Corriger les compteurs, receipts, catalog pins ou vault transactions
  uniquement à partir des RED précis ; ne pas élargir le wire pour faciliter
  les tests.

### Gate GREEN

Chaque ligne d'exemple obtient son propre operation ref et son propre verdict ;
un défaut d'une famille ne peut pas être masqué par une autre fixture verte.

## Lot 6 — client natif : mutations owner et délégué multi-zone

### Gherkins à créer ou étendre

Conserver `features/e-public-mutation.feature` comme première tranche verte,
puis créer `features/h-zone-mutation.feature`.

#### CLIENT-OWN-001 — Owner builds a verified plan for every zone mutation

Scenario Outline de 9 lignes : create/edit/delete × public/circle/self.

Assertions obligatoires :

- expected head et new head exacts ;
- plan vérifié contre le baseline ;
- public sous carriers publics ;
- circle sous blobs/headers/wraps chiffrés ;
- self seulement sous identifiants/commitments opaques ;
- Gamma, changeset, evidence et manifest dans le même plan ;
- baseline immuable avant application.

#### CLIENT-DEL-001 — Grantee builds every authorized mutation plan

Scenario Outline de 10 lignes selon la matrice E2E déléguée : public 3,
circle 3, self 4.

Assertions obligatoires : leaf signataire, `authorized_via` exact, revalidation
juste avant assembly, aucune signature owner, plan cold-verifiable.

#### CLIENT-DEL-002 — Invalid delegated authority produces no plan

Lignes : hors perimeter, zone sans droit, dir/tag self write, chaîne expirée,
chaîne révoquée, chaîne réordonnée, autre DID, mauvaise leaf, ligne de contenu
absente, stale expected head et entropy reuse.

#### CLIENT-RW-001 — The produced snapshot is readable by the right actor

Statut : **GREEN** sur public/circle/self, cible exacte, sibling et lock.

Après `verifyAgainst` :

- owner lit public/circle/self ;
- grantee lit public/circle/self couvert ;
- grantee ne lit pas sibling circle/self ;
- retirer/locker la capacité coupe les lectures privées sans changer le
  verdict keyless.

### Développement associé

- [x] Remplacer `PublicMutationIntent` par un `MutationIntent` fermé portant
  zone, kind, target et timestamp. Préserver un alias public temporaire si la
  compatibilité l'exige.
- [x] Généraliser `PublicationPlan::build_owner` et `build_grantee` dans
  `crates/aithos-client/src/publication.rs`.
- [x] Remplacer `perform_public_mutation` par des opérations de keyholder
  purpose-bound public/circle/self ; aucune capacité de signature arbitraire.
- [x] Réutiliser les mutations Bundle pour produire blobs, headers, wraps,
  roots et preuves, au lieu de reconstruire une seconde sémantique côté client.
- [ ] Étendre `mandate.rs`, `provider.rs`, `session.rs` et les erreurs typées aux
  targets circle/self exacts.
- [ ] Garder les méthodes public existantes comme wrappers si nécessaire, puis
  les déprécier une fois le SDK migré.

### Gate GREEN

9 plans owner + 10 plans grantee + tous les refus passent dans le runner natif,
avec des artefacts réellement vérifiés par Bundle.

## Lot 7 — client WASM : parité et confinement

### Gherkins à créer

Fichier : `features/x-browser-mutation.feature`, runner
`aithos-client-wasm/tests/cucumber.rs`.

#### WASM-MUT-001 — Browser and native produce the same plan

Scenario Outline sur les 19 mutations E2E. À entropy et time identiques : même
DID, même expected/new head, mêmes paths, mêmes bytes, même package digest.

#### WASM-MUT-002 — Browser grantee authority is imported as two opaque halves

Bonne clé + chaîne accepte ; clé seule, chaîne seule, mauvaise leaf et autre
DID refusent sans plan ni handle résiduel.

#### WASM-MUT-003 — No secret or plaintext crosses JavaScript

Inspecter résultats, erreurs, diagnostics, network observer, localStorage,
sessionStorage, IndexedDB et Cache Storage pour owner et grantee circle/self.

#### WASM-MUT-004 — Lock and reload cut mutation authority

Après lock ou reset, toute nouvelle mutation privée est `session_locked` ou
`missing_authority`; un handle alias ne réanime rien.

### Développement associé

- [x] Ajouter des méthodes WASM génériques `owner_mutation` et
  `grantee_mutation` avec intent JSON fermé ou arguments typés.
- [x] Étendre les registres de handles pour les plans multi-zone sans stocker
  de plaintext additionnel.
- [x] Exposer seulement metadata/artifacts du plan ; garder clés, DK et
  capabilities dans Rust.
- [x] Mettre à jour le package npm `@aithos/client` et ses types TypeScript.

WASM-MUT-004 est **GREEN** : le bootstrap owner possède un lock avec
tombstone partagé par tous ses alias, le lock de clé grantee coupe le planner,
et un reset/reload invalide les anciens handles même après nouveau cold verify.

### Gate GREEN

Les 19 plans natifs/WASM sont byte-identical avec inputs injectés et aucun
secret n'est observable côté JavaScript.

## Lot 8 — SDK : vrais Gherkins de transport et orchestration

### Gherkins à créer

#### SDK-PUB-001 — A real client plan uploads in canonical order

Fichier : `aithos-sdk/features/provider-publication.feature`.

Scenario Outline owner/grantee × public/circle/self. Utiliser un vrai plan
client ; vérifier chaque body envoyé, chaque enveloppe, manifest dernier,
absence de `manifests/<height>.json` en PUT client et résultat typé.

#### SDK-PUB-002 — Every artifact uses the authenticated actor

Owner envelope sans mandat ; grantee envelope avec leaf et chaîne exactes. La
JavaScript layer ne peut remplacer ni key ni `authorized_by`.

#### SDK-CAS-001 — CAS is stable, idempotent and typed

- matching head -> published ;
- same new head déjà commis -> already_committed ;
- stale other head -> `CasConflictError` avec head courant ;
- aucun retry automatique d'une édition conflictuelle ;
- manifest sans `If-Head` -> refus ;
- transport coupé avant manifest -> aucun nouveau head.

#### SDK-DOWNLOAD-001 — SDK downloads a complete verifiable snapshot

Fichier : `aithos-sdk/features/provider-download.feature`.

- lire `/heads` sous l'autorité adaptée ;
- utiliser list/batch ou sync pour obtenir tous les artefacts nécessaires ;
- construire `Artifact[]` sans confiance préalable ;
- appeler `AithosClient.coldVerify` ;
- refuser missing, duplicate, unexpected ou substituted artifact ;
- ne jamais demander hors perimeter pour une autorité grantee.

#### SDK-ROUNDTRIP-001 — Publish, restart, download and verify

Fichier : `aithos-sdk/features/owner-delegate-roundtrip.feature`. Ce scénario
orchestre les 19 lignes de mutation du gate E2E, puis les lectures associées.

#### SDK-ERROR-001 — Transport errors never become authority verdicts

DNS/refused socket/timeout/503 restent `transport` ou `provider_unavailable` ;
403 mandate reste `provider_forbidden/not_covered` ; 409 reste CAS ;
`artifact_invalid` conserve reason sans bytes sensibles.

### Développement associé

- [x] Ajouter les `.feature`, le runner Cucumber JS et les steps SDK.
- [x] Remplacer, dans les scénarios E2E, les plans littéraux/fetch mocks par
  le vrai package `@aithos/client` et le vrai `store_api`.
- [x] Ajouter à `ProviderClient` des méthodes typées `heads`, `list`, `batch`,
  `sync` et/ou `downloadSnapshot`, avec enveloppe owner/grantee.
- [ ] Ajouter `AithosSdk.publish`, `AithosSdk.downloadAndVerify` et
  `AithosSdk.publishRestartAndVerify` ou un orchestrateur équivalent ; éviter
  de cacher les étapes dans le test.
- [x] Vérifier le plan localement avant le premier fetch et vérifier le
  snapshot téléchargé avant de le rendre utilisable.
- [x] Rendre le résultat `published_unverified` impossible à confondre avec un
  roundtrip vérifié ; introduire un statut distinct `published_and_verified`.
- [x] Étendre `src/index.d.ts` et les erreurs sans exposer de clés ni de
  fonction de signature générique.

### Gate GREEN

Le runner SDK prouve un transport réel pour owner et grantee sur les trois
zones, y compris CAS et téléchargement froid. Les tests unitaires mocks restent
en complément mais ne comptent pas dans ce gate.

## Lot 9 — provider : support indispensable au vrai E2E

### Gherkins à activer

#### PROVIDER-DEL-001 — Delegated author publishes under CAS

Activer `store-publication.feature:123` avec le package délégué produit dans le
scénario, pas avec un manifest parallèle.

#### PROVIDER-COLD-001 à 008

Activer les huit scénarios de `store-cold-roundtrip.feature` : import atomique,
store non vide, absence de secret, restart, lectures owner/grantee, objet
substitué, objet manquant et mauvais tip.

#### PROVIDER-CAS-001 — Concurrent actors race on one head

Lignes : owner/owner, owner/grantee, grantee/grantee. Une seule publication
gagne ; le perdant obtient le head gagnant et aucun artefact mutable partiel.

### Développement associé

- [x] Achever la branche delegated manifest de `artifacts.rs`/`service.rs` et
  vérifier `authorized_by` sous le lock CAS.
- [x] Brancher le backend durable de test du lot 0 au binaire `store_api`.
- [x] Implémenter le download/import utilisé par les scénarios cold, sans faire
  confiance au provider pour la sémantique.
- [ ] Rendre les artefacts immuables idempotents et le manifest/head
  transactionnels lors des courses.

### Gate GREEN

Le binaire est réellement redémarré et le SDK reconstruit un store local vierge
à partir de ses réponses HTTP.

## Lot 10 — gates E2E transverses définitifs

### E2E-MUT-001 — Every owner and grantee mutation survives the full chain

Scenario Outline de 19 lignes. Pour chaque ligne :

1. créer ou charger le même genesis ;
2. ouvrir l'autorité owner ou grantee ;
3. produire le plan ;
4. vérifier le plan localement ;
5. publier via SDK/provider réel ;
6. tuer le provider ;
7. redémarrer sur le même backend ;
8. télécharger dans un nouveau répertoire local vide ;
9. détruire le producteur et cold-verify keyless ;
10. réintroduire la capacité attendue et lire l'effet exact.

### E2E-READ-001 — Every actor sees exactly its perimeter

Six lignes : owner et grantee × public/circle/self. Vérifier list + read,
sibling absent, public keyless, circle/self privés, self sans structure publique.

### E2E-AUTH-001 — Authority can change between planning and commit

Lignes : expiration, revocation, head stale, mandate remplacé, key locked. La
revalidation finale refuse avant manifest et ne laisse aucun effet visible.

### E2E-FAIL-001 — Every interruption is recoverable or safely refused

Lignes : avant premier artifact, après artifact immuable, avant manifest,
pendant CAS, réponse perdue après commit, objet manquant au download, objet
substitué, provider coupé au milieu du sync.

### E2E-SEC-001 — No layer leaks authority or protected content

Scanner argv, env de test contrôlé, logs capturés, HTTP, erreurs, package,
stockage browser et artefacts publics pour sentinelles owner seed, grantee key,
DK, body circle et body self.

### Gate GREEN final

Les mêmes identifiants publics sont égaux à chaque frontière. Aucune assertion
ne peut être satisfaite par une fixture distincte, un mock transport ou un
verdict global déjà calculé.

## Ordre RED -> DEV -> GREEN recommandé

| Étape | RED à committer en premier | Développement autorisé après observation du RED | GREEN requis |
|---|---|---|---|
| 0 | HARN-001 à 005 | runner, manifest, backend durable, superviseur | le harnais détecte les faux verts |
| 1 | CORE-OWN-001 à 004 | steps réels, corrections Bundle ciblées | owner 3 zones réel |
| 2 | CORE-DEL-001 à 005 | steps CB9 réels, trous Bundle éventuels | délégué 18 cellules réel |
| 3 | CORE-STR/REV | transactions structure/revocation | structure et cut atomiques |
| 4 | CORE-ED/COLD | package unique, vérification delegated | même package froid |
| 5 | CLIENT-OWN/DEL/RW | MutationIntent multi-zone, keyholders | plans natifs 3 zones |
| 6 | WASM-MUT | bindings/handles multi-zone | parité native/WASM |
| 7 | SDK-PUB/CAS/DOWNLOAD | transport et orchestration typés | SDK sans plan fake |
| 8 | PROVIDER-DEL/COLD/CAS | delegated CAS et backend restart | provider réel |
| 9 | E2E-MUT/READ/AUTH/FAIL/SEC | corrections verticales seulement | gate complet 19 mutations |
| 10 | CORE-SEM/CONSTRAINT/OBLIGATION/COUNT/VAULT | suppression des derniers proxies | couverture fonctionnelle totale |

Chaque étape doit garder tous les lots précédents verts. Un lot ne peut pas
être déclaré terminé parce que son test CB sous-jacent passe ; seul son runner
Gherkin scenario-driven fait foi.

## Traçabilité avec l'audit précédent

| TODO audit | Couverture dans ce plan |
|---|---|
| GH-E2E-001, même verticale multi-zone | E2E-MUT-001, E2E-READ-001 |
| GH-E2E-002, même package délégué | CORE-ED-002 à 004, CORE-COLD-001, HARN-003 |
| GH-E2E-003, client circle/self | CLIENT-OWN-001, CLIENT-DEL-001/002, CLIENT-RW-001 |
| GH-E2E-004, publish provider délégué | SDK-PUB-001/002, PROVIDER-DEL-001 |
| GH-E2E-005, cold roundtrip provider | SDK-DOWNLOAD-001, PROVIDER-COLD-001 à 008 |
| GH-E2E-006, Gherkins SDK | Lot 0, lot 8, lot 10 |
| GH-E2E-007, gateway générique | hors gate Core/client/SDK ; décision séparée conservée |
| CORE-GH-001 | CORE-OWN-001 à 004 |
| CORE-GH-002 | CORE-DEL-002/003/004 |
| CORE-GH-003 | CORE-SEM-001 |
| CORE-GH-004 | CORE-CONSTRAINT-001, CORE-COUNT-001 |
| CORE-GH-005 | CORE-OBLIGATION-001 |
| CORE-GH-006 | CORE-REV-001 |
| CORE-GH-007 | CORE-COLD-001 |
| CORE-GH-008 | CORE-DEL-001 à 005 |
| CORE-GH-009 | CORE-ED-001 à 004 |
| CORE-GH-010 | CORE-STR-001 à 003 |
| CORE-GH-011 | CORE-VAULT-001 |

## Définition de done globale

- [ ] Tous les Gherkins release sont sélectionnés, aucun n'est `@wip`.
- [x] Aucun pickle release ne contient de placeholder `<...>` non substitué.
- [ ] Aucun step release n'appelle `cbN_result` ou `cbN_assert_green`.
- [x] Les 15 cellules owner et 18 cellules grantee sont scenario-driven dans
  Core ; les 19 mutations owner/grantee et les 9 mutations d'autorité owner
  reconnectée sont verticales dans le SDK.
- [x] Owner et grantee publient sous leur propre capacité, sans substitution.
- [x] Public/circle/self ont chacun leur représentation et leurs refus exacts.
- [x] Les plans natif et WASM sont identiques à inputs injectés identiques.
- [x] Le SDK utilise de vrais handles client et un vrai provider HTTP.
- [x] Le PID provider change pendant le roundtrip et les données survivent.
- [x] Le store de vérification est vide et distinct du producteur.
- [x] La vérification keyless précède toute réintroduction de capacité privée.
- [ ] CAS, interruptions et altérations sont fail-closed sans effet partiel.
- [ ] Les tests de non-fuite passent sur résultats, erreurs, réseau et stockage.
- [ ] La CI publie les compteurs sélectionnés/exclus et la trace des identifiants
  publics du package, sans aucune donnée secrète.

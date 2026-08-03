# TODO — rendre chaque Gherkin réellement exécutable de bout en bout

Date de l'audit : 2026-07-22

Plan TDD détaillé associé :
`docs/PLAN-TDD-GHERKINS-E2E-OWNER-DELEGUE-2026-07-22.md`.

Périmètre inspecté : `aithos-core`, `aithos-bundle`, `aithos-client`,
`aithos-client-wasm`, `aithos-gateway`, `aithos-provider` et `aithos-sdk`.

## Verdict

Le catalogue sélectionné par les runners est vert, mais tous les Gherkins ne
prouvent pas encore leur propre parcours réel.

- 1 375 scénarios effectivement sélectionnés passent : Core/Bundle 815,
  client/WASM 113, gateway 296 et provider 151.
- Une partie des Gherkins Core est verte par **proxy** : les steps appellent un
  test d'acceptation CB global, mis en cache, sans utiliser les paramètres de la
  ligne `Examples` courante.
- Plusieurs contrats gateway/provider sont `@wip` et sont explicitement
  exclus des runners.
- `aithos-sdk` ne contient aucun fichier `.feature` et n'a donc aucune preuve
  Gherkin propre.
- Le grand parcours recherché n'existe pas encore : mutation signée par le
  délégué en `public`/`circle`/`self`, assemblage de la même édition déléguée,
  publication HTTP sous CAS, redémarrage du provider, téléchargement dans un
  store vierge, vérification froide et relecture avec les capacités exactes.

Le souvenir initial est donc juste : les primitives Core existent en grande
partie, mais le parcours vertical délégué complet n'est pas opérationnel.

## Critère utilisé

Un scénario est classé **réel au niveau annoncé** seulement si :

1. ses paramètres alimentent réellement l'appel au système testé ;
2. le `When` appelle l'implémentation de production ou une façade publique
   réelle, et non un verdict global préparé ailleurs ;
3. le `Then` vérifie le résultat produit par ce scénario ;
4. un refus vérifie aussi l'absence d'effet partiel ;
5. lorsqu'il promet un reopen, un restart, HTTP ou un store frais, cette
   frontière est réellement franchie.

Un test unitaire ou d'acceptation sous-jacent peut être excellent et vert sans
que le Gherkin qui l'invoque soit lui-même une preuve E2E de chaque exemple.

## Exécutions de contrôle

| Runner | Résultat | Remarque |
|---|---:|---|
| `aithos-bundle --test cucumber` | 815 scénarios, 3 505 steps, tous verts | Inclut les proxies décrits plus bas. |
| `aithos-bundle --test cb9_delegated_content` | 3/3 tests verts | CRUD délégué Core/Bundle, sans verticale provider complète. |
| `aithos-bundle --test cb10_structure_vault` | 4/4 tests verts | Implémentation structure/vault réelle, mais plusieurs Gherkins n'en pilotent pas les cas individuellement. |
| `aithos-bundle --test cb12_publication_package` | 5/5 tests verts | Édition déléguée et cold roundtrip propriétaire prouvés séparément. |
| runners natifs client A-D/G + phase E + phase F | 94 scénarios, 459 steps, tous verts | Les 26 scénarios de mutation phase E sont réels, mais limités à `public`. |
| runner WASM | 19 scénarios, 117 steps, tous verts | Lecture/session ; pas de mutation déléguée browser. |
| runner gateway hors sandbox | 296 scénarios, 1 406 steps, tous verts | Les features/scénarios `@wip` sont filtrés. |
| runner provider | 151 scénarios, 992 steps, tous verts | Le publish délégué et tout `store-cold-roundtrip` sont filtrés. |

Le premier essai gateway dans le sandbox a échoué avant les assertions, car
les faux serveurs ne pouvaient pas ouvrir leurs sockets locales. Le même binaire
hors sandbox passe 296/296 ; cet incident n'est pas un rouge fonctionnel.

## P0 — verticale d'écriture déléguée manquante

### GH-E2E-001 — créer une seule preuve verticale multi-zone

- [ ] Construire un scénario réel pour chaque opération acceptée/refusée de
  `features/l-delegated-writes.feature`, avec les valeurs de chaque ligne
  `Examples` réellement injectées.
- [ ] Couvrir `public`, `circle` et `self`, y compris les sélecteurs `dir`,
  `id`, zone entière et les refus structurels propres à `self`.
- [ ] Utiliser la clé feuille et la chaîne de mandats exactes pour signer la
  mutation et l'édition ; ne jamais faire publier ensuite l'owner à la place du
  délégué.
- [ ] Prouver que Gamma, authorship, changeset, evidence et manifest décrivent
  le même `operation_ref` et le même acteur.
- [ ] Envoyer ce package au provider via HTTP avec `If-Head`, arrêter puis
  redémarrer le provider, télécharger vers un store réellement vierge et
  appeler la vérification froide.
- [ ] Réintroduire ensuite séparément les capacités owner/grantee et vérifier
  les lectures couvertes ainsi que les refus latéraux.
- [ ] Ajouter les variantes objet manquant, objet substitué, head obsolète,
  chaîne expirée/révoquée et panne avant commit, avec zéro effet partiel.

### GH-E2E-002 — ne plus assembler deux preuves séparées pour simuler la verticale

`cb12_bundle_assembles_the_exact_signed_draft2_candidate` prouve un candidat
signé par un délégué. `cb12_owner_package_survives_fresh_mem_and_fs_cold_verification`
prouve un package owner dans un store frais. Leur succès conjoint ne prouve pas
qu'un **même package délégué** a parcouru toutes les couches.

- [ ] Produire le package frais directement depuis la mutation du délégué.
- [ ] Conserver le même manifest head et le même digest de package jusqu'à la
  réouverture.
- [ ] Détruire le producteur et toutes les capacités privées avant la
  vérification froide.

### GH-E2E-003 — étendre le client au-delà de `public`

Le type public `PublicMutationIntent` refuse actuellement toute zone autre que
`public` (`aithos-client/crates/aithos-client/src/publication.rs`, autour de la
ligne 54). Les Gherkins phase E testent réellement create/edit/delete public,
et testent explicitement que `circle` et `self` rendent `ZoneNotAllowed`.

- [ ] Remplacer l'intention exclusivement publique par une intention de
  mutation typée par zone, sans affaiblir les règles `self`.
- [ ] Ajouter les Gherkins positifs create/edit/delete `circle` et `self`.
- [ ] Faire transporter au plan les blobs, headers/wraps, index/roots et
  preuves opaques réellement produits pour la zone.
- [ ] Vérifier le plan dans un nouveau snapshot, pas seulement sa forme.
- [ ] Ajouter le parcours WASM équivalent sans exporter la clé vers JavaScript.

### GH-E2E-004 — activer le publish délégué du provider

- [ ] Activer
  `rust/crates/aithos-provider/tests/features/store/store-publication.feature:123`,
  **A delegated author with authorized_by may publish under the CAS**.
- [ ] Faire charger un package réellement exporté par Bundle, et non une
  recomposition locale de fixtures indépendantes.
- [ ] Vérifier la chaîne `authorized_by`, la clé feuille, la signature du
  manifest, le predecessor et le CAS dans la même requête.
- [ ] Vérifier les courses CAS owner/délégué et délégué/délégué.

### GH-E2E-005 — activer tout le cold roundtrip provider

Les huit scénarios de
`rust/crates/aithos-provider/tests/features/store/store-cold-roundtrip.feature`
sont tous `@wip` :

- [ ] A keyless package installs into a fresh empty store in one transaction.
- [ ] `import_keyless` refuses a store that is not already empty.
- [ ] The exported package carries no private material.
- [ ] A published edition survives a restart and cold-verifies in a virgin store.
- [ ] The owner and the grantee each read their covered objects from the provider.
- [ ] A substituted object fails cold verification.
- [ ] A missing pinned object fails cold verification.
- [ ] A store whose manifest is not the edition tip fails cold verification.

### GH-E2E-006 — donner une preuve Gherkin propre au SDK

`aithos-sdk` ne contient aucun `.feature`.

- [ ] Ajouter un runner et un contrat SDK qui consomme le plan réel du client,
  publie tous les artefacts dans l'ordre imposé, commet le manifest en dernier
  sous CAS, puis récupère les artefacts nécessaires au cold verify.
- [ ] Tester owner, délégué public, délégué circle et délégué self.
- [ ] Tester les erreurs transport/CAS sans transformer une erreur réseau en
  verdict d'autorité.

### GH-E2E-007 — décider et tester la surface gateway d'écriture générique

`gateway-journal.feature` est réel et vert, mais il prouve une surface métier
spécialisée `journal.write`, câblée sur `Zone::Circle`. Le bridge possède aussi
des helpers section add/modify/delete principalement circle. Il n'existe pas de
feature positive générique `ethos.write` couvrant les trois zones.

- [ ] Décider si la gateway doit exposer l'écriture Ethos générique ou rester
  limitée à des outils métier spécialisés.
- [ ] Si elle l'expose, ajouter un Gherkin public/circle/self qui passe par la
  session déléguée durable et rejoint le package publié.
- [ ] Sinon, documenter explicitement que la verticale d'écriture est
  client/SDK/provider et ne pas présenter `journal.write` comme sa preuve.

## P1 — Gherkins Core verts par proxy à remplacer

Le mécanisme en cause est visible dans
`rust/crates/aithos-bundle/tests/cucumber.rs` : `cb4_result` à `cb10_result`
stockent un unique résultat via `OnceLock::get_or_init`. Des familles entières
de steps appellent ensuite seulement `cbN_result()` ou `cbN_assert_green()`.
Ainsi, plusieurs lignes `Examples` différentes obtiennent le même verdict sans
que leurs paramètres soient passés à l'implémentation.

Pour chaque groupe ci-dessous :

- [ ] remplacer le proxy par un état de scénario propre ;
- [ ] construire l'entrée depuis les paramètres Gherkin ;
- [ ] appeler l'API de production ciblée ;
- [ ] conserver et vérifier le résultat de cette exécution précise ;
- [ ] garder les tests CB autonomes comme tests complémentaires, pas comme
  substituts du scénario.

### CORE-GH-001 — Bundle et transactions

Dans `features/d-bundle.feature` :

- [ ] ligne 62, **The local owner performs every content operation without a mandate** — toutes les lignes zone/opération ;
- [ ] ligne 90, **Failure before the logical commit point preserves the old bundle byte for byte** — toutes les frontières MemStore/FsStore ;
- [ ] ligne 115, **A successful local transaction publishes content and Gamma together** — MemStore et FsStore ;
- [ ] ligne 130, **A bundle operation uses only its narrow opaque cryptographic capability** — toutes les classes de capacité ;
- [ ] ligne 147, **An untrusted path or Store key can never escape its selected root** — toutes les entrées/pathologies filesystem.

### CORE-GH-002 — mandats précis et verdict append/cold

- [ ] `features/e-mandate-sections.feature:13`, section circle précise ;
- [ ] `features/e-mandate-sections.feature:19`, section self précise ;
- [ ] `features/e-mandate-sections.feature:25`, verbes d'écriture par `id` ;
- [ ] `features/e-mandate-sections.feature:31`, écriture self par `id` ;
- [ ] `features/e-mandates.feature:135`, ligne d'exemple `revoked mandate chain`, actuellement appuyée sur un verdict CB6 global ;
- [ ] `features/e-mandates.feature:148`, verdict identique avant append et après export.

### CORE-GH-003 — Gamma et opérations typées

Dans `features/f-gamma.feature` :

- [ ] règle ligne 161, **Gamma replays every protocol consumption semantically** ;
- [ ] règle ligne 208, **One typed operation occurrence has one cross-view commitment**, jusqu'au scénario de refus ligne 577 ;
- [ ] scénarios SC1 positifs lignes 586 et 594 ; le Scenario Outline négatif ligne 602 pilote, lui, des défauts distincts ;
- [ ] règle ligne 620, **Gamma v2 is a monotone operation-evidence profile** ;
- [ ] règle ligne 716, **Append-time and cold-time share one replay front door**.

### CORE-GH-004 — contraintes et compteurs

- [ ] `features/f-plus-constraints.feature`, règle U1 lignes 163–192 ;
- [ ] même fichier, règles de contraintes versionnées et `max_children`, lignes 337–405 ;
- [ ] même fichier, matrice d'applicabilité et parité append/cold, lignes 409–453 ;
- [ ] `features/h2-gamma-roots.feature`, compteurs délégués et replay sémantique, lignes 89–163.

### CORE-GH-005 — obligations et receipts

Dans `features/g-plus-obligations.feature`, remplacer les proxies des règles
lignes 166–255 : R2, matcher draft3, consommations ciblées, co-signature de
publication, vérité tier-X et égalité append/fresh-store.

### CORE-GH-006 — révocation atomique

Dans `features/g-revocation.feature` :

- [ ] ligne 129, cold reopen après incident cut ;
- [ ] ligne 137, toutes les pannes injectées pendant la révocation ;
- [ ] ligne 156, autorité historique jugée à l'instant de chaque entrée.

### CORE-GH-007 — intégration en store frais

Dans `features/k-integration.feature` :

- [ ] ligne 158, MemStore et FsStore avec la même histoire owner+délégué ;
- [ ] ligne 171, réintroduction séparée de la capacité privée ;
- [ ] ligne 177, les cinq classes d'artefact manquant/substitué.

Ces scénarios appellent aujourd'hui CB9 et CB12 séparément ; leurs paramètres
`store` et `artifact defect` ne construisent pas le parcours annoncé.

### CORE-GH-008 — écritures déléguées multi-zone

Dans `features/l-delegated-writes.feature`, remplacer tous les scénarios de la
ligne 103 à la ligne 179 :

- [ ] les 18 lignes de parité zone/opération ;
- [ ] les 4 combinaisons possession/autorité ;
- [ ] l'authorship public ;
- [ ] les 3 transitions self ;
- [ ] expiry et revocation après ouverture de session ;
- [ ] le rollback total d'un refus.

Les scénarios historiques lignes 13–95 manipulent bien le Bundle scénario par
scénario ; ils ne remplacent toutefois pas les nouveaux cas circle/self ni le
package provider.

### CORE-GH-009 — éditions déléguées

- [ ] Remplacer **tous** les scénarios et toutes les lignes `Examples` de
  `features/m-delegated-editions.feature`.

Les steps `m_carrier_fixture`, `m_carrier_action` et `m_carrier_verdict`
additionnent les verdicts CB4/CB6/CB9/CB12. Ils ne construisent pas le candidat
décrit par chaque scénario. C'est le principal faux vert du chantier.

### CORE-GH-010 — mutations structurelles

- [ ] Remplacer **tous** les scénarios et toutes les lignes `Examples` de
  `features/n-structural-mutations.feature`.

Les 26 cas d'autorité, les 7 pannes et les scénarios tag/move/subtree/self
partagent actuellement le même `cb10_result` mis en cache.

### CORE-GH-011 — catalogues et vault connecteur

- [ ] Remplacer **tous** les scénarios et toutes les lignes `Examples` de
  `features/o-connector-classes-vault.feature`.

Les sections utilisent CB5 catalog, CB6/CB7 overlay ou CB10 vault comme verdict
global ; les actions, classes, pannes et opérations CRUD des exemples ne sont
pas jouées individuellement.

## P1 — contrats explicitement exclus par les runners

### Gateway

Le runner gateway filtre les tags `@wip` au niveau feature, rule et scenario
(`rust/crates/aithos-gateway/tests/cucumber.rs`, autour des lignes 10838–10842).

- [ ] Activer les 4 scénarios de `g4-client-surfaces.feature`.
- [ ] Activer tous les scénarios et exemples de `gateway-delegated-session-ceremony.feature`.
- [ ] Activer tous les scénarios et exemples de `gateway-delegated-session-runtime.feature`.
- [ ] Activer le self read de `gateway-ethos-read.feature:133`.
- [ ] Activer les scénarios `@wip` de `gateway-mandates.feature` aux lignes
  24, 32, 40, 47, 55, 64, 71, 80, 86, 96, 104, 114, 137 et 144.
- [ ] Activer le remote proof SDK de `gateway-control.feature:83`.
- [ ] Hors verticale d'écriture : activer toute
  `gateway-oauth-durable.feature` et toute `gateway-rustls-release.feature`, ou
  les déplacer hors du catalogue de release si elles ne sont plus contractuelles.

### Provider

Le runner provider filtre tout scénario `@wip`
(`rust/crates/aithos-provider/tests/cucumber.rs`, autour des lignes 3654–3661).

- [ ] Activer le publish délégué, ligne 123.
- [ ] Activer les huit scénarios cold roundtrip listés dans GH-E2E-005.
- [ ] Hors verticale d'écriture : activer ou retirer du contrat de release le
  witness publish de `store-publication.feature:389`.

### Client

Il n'y a pas de scénario `@wip` actif. Les runners sélectionnent 9 features et
passent tous. `a-offline-authority.feature` est taggée `@superseded` et exclue
intentionnellement ; ses cas sont annoncés comme redistribués dans A/B/C/D/W.

- [ ] Ajouter un contrôle de catalogue qui vérifie automatiquement que chaque
  scénario superseded possède un remplaçant nommé, ou retirer ce fichier du
  catalogue exécutable afin qu'il ne ressemble pas à une preuve active.

## Garde-fous CI proposés

- [ ] Publier, pour chaque runner, le nombre de features/scénarios sélectionnés
  et le nombre exclu par tag.
- [ ] Faire échouer le gate de release si un `@wip` est présent dans le
  périmètre release, même si le runner de développement continue de le filtrer.
- [ ] Interdire dans les steps Gherkin les appels directs aux helpers globaux
  `cbN_result`/`cbN_assert_green`, sauf dans un scénario explicitement nommé
  comme agrégat de smoke tests.
- [ ] Vérifier qu'au moins un paramètre de chaque `Scenario Outline` atteint
  l'appel de production et l'assertion finale.
- [ ] Séparer les libellés : `component-real`, `cross-component-real`,
  `proxy-smoke`, `wip` et `superseded`.
- [ ] Ajouter un gate unique `delegated-write-cold-roundtrip` qui ne peut pas
  être satisfait par la somme de plusieurs fixtures ou packages différents.

## Ordre recommandé

1. Activer le publish délégué provider et le cold roundtrip avec le package
   public déjà réellement produit par le client.
2. Étendre l'intention client à `circle`, puis à `self` avec SID opaque.
3. Remplacer les proxies L et M par le vrai package vertical ; brancher N pour
   les mutations structurelles.
4. Ajouter la façade et les Gherkins SDK.
5. Brancher, si souhaité, la surface gateway générique et ses sessions
   déléguées durables.
6. Nettoyer les autres proxies Core et les `@wip` hors verticale.

## Condition de clôture du chantier

Le chantier n'est clos que lorsqu'une exécution CI peut montrer, pour chacun
des trois espaces, les mêmes identifiants de mutation, d'édition et de package
à travers la chaîne suivante :

`clé déléguée + mandat -> client/WASM -> Bundle/Core -> SDK -> provider HTTP/CAS -> restart -> store vierge -> cold verify -> lecture autorisée/refus latéral`

Un vert obtenu par un autre acteur, un autre package, une autre fixture ou un
verdict CB mis en cache ne satisfait pas cette condition.

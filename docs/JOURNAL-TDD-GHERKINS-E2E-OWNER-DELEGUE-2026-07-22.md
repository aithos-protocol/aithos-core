# Journal TDD — Gherkins E2E owner et délégué

Date de démarrage : 2026-07-22

Sources de vérité :

- `docs/TODO-AUDIT-GHERKINS-E2E-2026-07-22.md`
- `docs/PLAN-TDD-GHERKINS-E2E-OWNER-DELEGUE-2026-07-22.md`

Ce journal ne contient que des identifiants publics, commandes, compteurs et
verdicts. Aucune clé privée, DK ou donnée protégée ne doit y être inscrite.

## État initial du worktree

- `aithos-core` : worktree déjà fortement modifié, notamment documentation,
  gateway, provider et `Cargo.lock` ; ces changements sont préservés.
- `aithos-client` : changements préexistants dans `publication.rs`,
  `mandate.rs` et `session_parent_planning.rs` ; ils sont préservés.
- `aithos-sdk` : `README.md` modifié et `docs/README.md` non suivi avant le
  chantier ; ils sont préservés.
- Gateway : explicitement hors périmètre du gate.

## Lot 0 — HARN-001 à HARN-005

### RED

- `npm run test:gherkin` : échec attendu, script absent (`Missing script:
  test:gherkin`).
- Après ajout du contrat, HARN-004 a échoué explicitement avec
  `durable provider supervisor is not implemented`.
- Le sandbox a ensuite refusé l'ouverture loopback (`listen EPERM`) ; la même
  commande autorisée hors sandbox a exposé deux vrais défauts successifs :
  bootstrap genesis absent (`chain_invalid`) puis redépôt byte-identique de
  `did.json` traité à tort comme une succession (`artifact_invalid/signature`).

### Développement

- Ajout du runner Cucumber JS, du catalogue/manifest release verrouillé et des
  steps HARN-001 à HARN-005 dans `aithos-sdk`.
- Ajout de la garde `assertRealClientPublicationPlan` contre les plans JS
  littéraux dans les scénarios E2E.
- Ajout des backends `FsObjects`, `FsHeads`, `FsNonces` derrière les traits
  provider existants et du choix explicite `filesystem` réservé au local E2E.
- Ajout d'un superviseur lançant réellement `aithos-store-api`, publiant un
  plan genesis client réel, arrêtant le PID puis redémarrant sur le même
  répertoire.
- Le redépôt strictement byte-identique de `did.json` est désormais idempotent
  avant toute interprétation comme succession.
- Ajout de `scripts/test-owner-delegate-e2e.sh`.

### GREEN

- `npm run test:gherkin` (hors sandbox pour loopback) : **7 scénarios, 23
  steps**, tous verts.
- Manifest : 1 177 pickles au total, 1 158 release sélectionnés, 19 exclus
  (11 `@wip`, 8 `@superseded`), 0 non assigné, 0 assigné plusieurs fois,
  0 scénario vide.
- Compteurs par runner : Core 815 ; client natif 64 + phase E 26 + phase F 4 ;
  WASM 19 ; SDK 7 ; provider store 151, remote 21, relay 27, tunnel 12,
  witness 12.
- `cargo test -p aithos-provider fs_backend::tests::objects_heads_and_nonces_survive_reconstruction --lib` : 1/1.
- `npm test` : 28/28.
- Le scénario restart compare deux PID distincts, relit après restart les
  octets publics exacts et le head durable écrit par la première publication.

### Baselines de non-régression après lot 0

- `CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cucumber` : 815
  scénarios, 3 505 steps.
- `CARGO_INCREMENTAL=0 cargo test -p aithos-client --test cucumber --test
  phase_e_cucumber --test phase_f_cucumber -p aithos-client-wasm --test
  cucumber` : natif 94 scénarios / 459 steps, WASM 19 / 117.
- `CARGO_INCREMENTAL=0 cargo test -p aithos-provider --test cucumber` : 151
  scénarios / 992 steps.

## Lot 1 — CORE-OWN-001 à CORE-OWN-004

### CORE-OWN-001 — RED

- Les steps proxy CB8 ont été remplacés par des steps paramétrés, avec un
  `When` volontairement non câblé.
- `CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cucumber` : RED
  pertinent, **15/15 lignes** public/circle/self × list/read/create/edit/delete
  échouent avec `CORE-OWN-001 RED: scenario-specific Bundle operation is not
  wired`; les autres scénarios continuent leur exécution.

### CORE-OWN-001 — développement et GREEN

- Chaque ligne construit désormais son propre `FsStore`, son identité owner,
  son contenu initial et appelle réellement `Bundle::owner_content_operation`
  avec la zone et l'opération de la ligne.
- Les résultats list/read sont vérifiés exactement ; create/edit/delete
  vérifient l'effet après destruction et `Bundle::open` frais.
- Le delta Gamma est 0 pour list/read et 1 pour chaque mutation ; aucun mandat
  ni compteur de mandat n'est utilisé.
- Même commande : **815 scénarios, 3 505 steps**, tous verts.

### CORE-OWN-002 — RED, développement et GREEN

- RED initial : les 12 lignes de panne ont échoué explicitement avec
  `typed failure injection is not wired`.
- RED intermédiaire : après câblage du Store fautif, 10/12 lignes étaient
  vertes ; les deux lignes `header or wrap` ont correctement révélé que le
  chemin sélectionné ne déclenchait pas ce point.
- Un `CoreAtomicFaultStore` typé injecte désormais exactement une panne sur
  le premier passage pertinent : écriture post-cryptographie, blob, index,
  ouverture header/wrap, Gamma ou commit/marker.
- Chaque ligne exécute une vraie création owner dans un Bundle publié, compare
  toutes les clés canoniques avant/après, détruit puis rouvre MemStore ou
  FsStore et vérifie l'édition ancienne.
- Les deux succès exécutent un vrai edit circle, vérifient les changements
  blob, index, Gamma et manifest, puis un reopen complet.
- GREEN ciblé : **12/12 pannes, 96/96 steps** et **2/2 succès, 12/12 steps**.

### CORE-OWN-003 — RED, développement et GREEN

- RED : les quatre lignes sign manifest, sign Gamma, open body et wrap header
  ont échoué avec `scenario capability is not wired`.
- Les capacités Gamma et Header disposent maintenant d'opérations typées :
  signature d'un `EntrySpec` owner et ajout d'une ligne recipient sans exposer
  DK ou KEX.
- La clé Gamma owner est la vraie clé `content_sign`; sa signature est vérifiée
  par `verify_owner_entry`.
- `BodyOpeningCapability` est désormais lié à une zone et un display path ;
  une lecture sibling et une autre session sont refusées.
- La ligne manifest assemble le candidat draft.2 verrouillé ; la ligne wrap
  prouve que seul le recipient attendu ouvre le DK.
- GREEN ciblé : **4/4 scénarios, 32/32 steps**.

### CORE-OWN-004 — RED, correction du feature et GREEN

- RED : 10/10 lignes ont d'abord échoué explicitement.
- Ce RED a révélé que les placeholders Gherkin contenant des espaces restaient
  littéraux. Ils ont été renommés `invalid_input`, `input_kind` et
  `filesystem_condition`, sans changer les dix exemples.
- Les quatre lignes MemStore passent par une vraie lecture Bundle et comparent
  le snapshot complet.
- Les six lignes FsStore utilisent des répertoires isolés et, selon la ligne,
  un fichier interdit réellement présent ou un symlink intermédiaire/final
  vers un objet extérieur réellement lisible. Le snapshot brut inclut fichiers,
  répertoires et cibles de symlink avant/après, sans suivre les liens.
- GREEN ciblé : **10/10 scénarios, 40/40 steps**.

### Gate lot 1

- `CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cucumber` : vert,
  sans warning du harness owner et sans appel CB8 global.
- Tests adjacents : `cb12_publication_package` 5/5,
  `cb2_bundle_boundaries` 6/6, `cb7_transaction_contracts` 6/6.

## Lot 2 — CORE-DEL-001 à CORE-DEL-005

### CORE-DEL-001 — RED, développement et GREEN

- RED : les **18/18 lignes** ont échoué avec `scenario-specific grantee
  operation is not wired`.
- Chaque ligne construit un Bundle MemStore propre avec une cible public,
  circle et self, émet la seule autorité annoncée, puis appelle réellement
  `Bundle::grantee_content_operation`.
- Les 16 acceptations vérifient l'effet exact après export dans un Store vide,
  le delta Gamma égal à 1, `authorized_via`, `authorized_by`, la clé acteur et
  `gamma_verify` après destruction du producteur.
- Les deux refus self dir/tag comparent toutes les clés canoniques et un delta
  Gamma nul. Le cas tag utilise un certificat signé sans inventer une ligne
  self que l'API interdit précisément.
- RED intermédiaire : 14/18 ; l'oracle self confondait le descriptor de dossier
  avec le SID de section, et le self préalloué exact n'a volontairement aucun
  display path. L'oracle final sélectionne le SID section et vérifie le
  préalloué par son inclusion opaque dans `SelfIndex`.
- GREEN ciblé : **18/18 scénarios, 72/72 steps**.

### CORE-DEL-002 — RED, développement et GREEN

- RED réel : 4/4 lignes content fence et 5/5 lignes possession/chaîne ont
  échoué avec les marqueurs CORE-DEL-002 dédiés.
- Le feature délégué avait lui aussi un placeholder avec espace non substitué
  (`<key material>`), corrigé en `<key_material>`.
- Les scénarios livrent réellement la ligne exacte ou sœur, construisent une
  chaîne exacte sans livraison pour le cas « no line », appellent la lecture
  grantee et distinguent autorité pure et déchiffrement effectif.
- Les cas sans chaîne, sans preuve et mauvaise clé passent par la même API
  Bundle. Le cas révoqué ajoute une vraie entrée `revoke` owner avant la
  tentative et est refusé par replay courant.
- GREEN : content fences **4/4, 12/12 steps** ; possession/chaîne **5/5,
  15/15 steps**.

### CORE-DEL-003 — RED, développement et GREEN

- Les quatre scénarios exact-id ne passent plus par `cb9_result` : chacun
  construit son Bundle, ses deux sections et son certificat exact.
- Les lectures circle/self ouvrent réellement la cible et refusent le SID
  frère ; les edits circle/self survivent à un `Bundle::open` frais.
- Le cas circle edit tente aussi une création sœur avec le même certificat :
  elle est refusée et le snapshot complet reste identique.
- Les matrices id/zone continuent d'exécuter les validateurs Core exacts pour
  les neuf opérations, huit parents dir/tag, trois parents whole-zone et les
  formes dupliquées.

### CORE-DEL-004 — RED, développement et GREEN

- Le placeholder `<authority change>`, qui restait littéral, a été corrigé en
  `<authority_change>`.
- Les deux lignes utilisent un vrai certificat edit et prouvent d'abord que la
  même ligne permet une mutation avant le changement d'autorité.
- Le cas expiry avance l'horloge après `not_after`; le cas revoke ajoute une
  vraie entrée Gamma owner. Dans les deux cas l'edit suivant est refusé,
  toutes les clés canoniques sont identiques et un Store frais relit l'ancien
  contenu.

### CORE-DEL-005 — RED, développement et GREEN

- Le scénario injecte exactement une panne à l'écriture Gamma, après les
  préparations blob/index de l'edit délégué mais avant la linéarisation.
- La transaction réelle est refusée puis rollbackée ; le snapshot entier est
  byte-identical, aucun artefact candidat n'est reachable et un Bundle frais
  vérifie Gamma et relit le contenu antérieur.
- Le wording Gherkin nomme maintenant la frontière réellement injectée
  (`late Gamma validation`) au lieu de prétendre injecter une contrainte.

### Gate lot 2

- `CARGO_INCREMENTAL=0 cargo test -p aithos-bundle --test cucumber` :
  **815 scénarios, 3 505 steps**, tous verts.
- CORE-DEL-001 à 005 sont scenario-driven pour les matrices et frontières du
  lot ; les scénarios authorship/cold adjacents restent affectés au lot 4.

## Lot 3 — CORE-STR-001 à CORE-STR-003 et CORE-REV-001

### CORE-STR-001 — RED, correction production et GREEN

- Les 26 lignes construisent chacune un Bundle circle indépendant, l'unique
  combinaison d'autorités annoncée et appellent réellement
  `Bundle::structural_operation` pour list, create child, rename, delete
  empty, move et delete subtree.
- Chaque acceptation vérifie l'effet exact, le delta Gamma et un reopen frais ;
  chaque refus vérifie toutes les clés byte-for-byte et un delta Gamma nul.
- RED initial pertinent : les deux deletes empty autorisés et le delete
  subtree complet étaient refusés. `structural_delete_folder` supprimait le
  header avant de récupérer la clé acteur nécessaire à l'évidence Gamma.
- Correction production : la clé prouvée est capturée après les gates
  d'autorité mais avant toute suppression d'index/header/blob.
- GREEN : **26/26 lignes** ; gate Core complet **815/815 scénarios,
  3 505/3 505 steps**.

### CORE-STR-002 — RED, développement et GREEN fonctionnel

- Les lectures de sous-arbre, edits de tags, moves, suppressions récursives et
  mutations self opaques utilisent chacun un Bundle indépendant et les vraies
  APIs structure/content déléguées.
- Les scénarios vérifient les effets primaires et dérivés : vue tag, SID stable,
  version header/wrap, refus de l'ancien chemin, suppression des blobs/index/
  headers, acteur Gamma et absence de path/plaintext self dans les octets
  publics.
- Chaque état accepté est rouvert depuis un Store frais. Une publication owner
  séparée prouve actuellement la cohérence du Store produit, mais ne satisfait
  pas encore l'exigence plus forte d'une édition draft.2 signée par le même
  délégué dans la transaction structurelle. Ce point reste explicitement
  ouvert dans le lot 4 et empêche de déclarer le gate lot 3 définitivement clos.

### CORE-STR-003 — RED, développement et GREEN

- Les sept frontières partent de sept Bundles propres : hors périmètre,
  descendant propre, collision, traversal, panne tag, panne rotation/rewrap et
  panne Gamma/manifest.
- Les quatre refus sémantiques et les trois pannes typées comparent le Store
  complet avant/après, vérifient l'absence d'artefact partiel et rouvrent l'état
  antérieur depuis un Store frais.

### CORE-REV-001 — RED, développement et GREEN

- Le cut réel révoque une chaîne, conserve une chaîne survivante, tourne la
  version cryptographique, retire la ligne révoquée, conserve la lecture du
  survivant, ajoute Gamma et produit une édition vérifiable après reopen.
- Six frontières transactionnelles sont injectées séparément : target/verdict,
  écriture header, écriture wrap, blob, Gamma et manifest. Chaque échec est
  byte-identical et l'ancien destinataire relit l'état précédent après reopen.
- Le replay crée une mutation déléguée avant le cut, refuse une mutation après
  le cut sans aucun octet nouveau, puis reconstruit depuis un Store frais la
  révocation courante et l'historique antérieur toujours attribuable.
- Gate Core complet : **815/815 scénarios, 3 505/3 505 steps** ; test adjacent
  `cb10_structure_vault` : **4/4**.

## Audit des placeholders Gherkin

- Le scan de tous les catalogs release a trouvé 31 noms de colonnes contenant
  des espaces et donc laissés littéraux par le parser (`<family verb>`,
  `<receipt state>`, `<artifact defect>`, etc.). Tous les headers et usages ont
  été rendus canoniques avec `_`; le `<digest_suffix>` absent a reçu une vraie
  colonne d'exemple.
- Le manifeste release stocke désormais les placeholders encore présents dans
  chaque pickle et expose `unresolvedPlaceholders`; HARN-001 exige zéro.
- Le seul angle-bracket littéral restant côté client (`<sid>`) a été renommé
  `{sid}` dans le feature et son step exact.
- Gate SDK hors sandbox loopback : **7/7 scénarios, 23/23 steps** ; manifeste
  inchangé à 1 177 pickles et `unresolvedPlaceholders: 0`.

## Lot 4 — CORE-ED-001 à CORE-ED-004 et CORE-COLD-001

### CORE-ED-001 — scenario-driven et GREEN

- Les quatre lignes actor/authority ne passent plus par CB12 global. L'owner
  produit et vérifie une vraie édition Bundle signée `#root`; le leaf assemble
  le draft.2 avec sa propre capacité manifest et sa chaîne exacte.
- Deux références d'autorité partielles et une session dont la clé ne correspond
  pas à l'acteur traversent le même verdict et sont refusées. La clé du manifest
  accepté est comparée à la clé acteur et l'édition leaf ne contient aucune
  signature owner.

### CORE-ED-002 — scenario-driven et GREEN

- Les 7 formes draft.1/draft.2/unknown exécutent individuellement
  `Manifest::verify_form`; les deux références carrier sont dérivées par
  `assemble_draft2`, reliées à leur sidecar canonique et comparées au pin
  `files` exact.
- Les cinq kinds d'evidence sélectionnent chacun leur item dans le candidat
  complet puis repassent le verdict K1-C.
- Les cinq défauts de changeset et les dix défauts carrier/evidence sélectionnent
  chacun un candidat mono-défaut distinct et appellent
  `verify_draft2_candidate_value`; aucun résultat agrégé des 37 cas n'est lu.

### CORE-ED-004 / CORE-COLD-001 — package réel et GREEN

- Un builder de scénario construit un package keyless complet depuis zéro :
  DID, manifest parent, opération/faits, changeset dérivé, authorship signé,
  evidence et draft.2.
- Le mode history construit d'abord l'édition owner, puis l'utilise comme vrai
  parent de hauteur 3 pour une édition leaf contenant un certificat signé et
  un manifest signé par le leaf. Le package final contient donc les deux
  signatures et une seule chaîne de manifests.
- Le même package est importé dans MemStore ou FsStore frais après destruction
  des sessions. Les quatre défauts d'export M et les cinq défauts K retirent ou
  substituent réellement certificat, Gamma, parent ou authorship avant le cold
  verdict.
- Gate Core : **815/815 scénarios, 3 505/3 505 steps**. Le nombre d'appels
  proxy `cbN_result`/`cbN_assert_green` restants dans le runner est passé de 117
  à **96**.

### Compléments CORE-ED-003 / CORE-COLD-001

- Le scénario self construit désormais une édition leaf de hauteur 3 sur le
  vrai parent owner, change un objet `e/self/blobs/<sid>.enc`, ne transporte
  aucun authorship public, cold-vérifie le package et scanne les octets publics
  pour l'absence de nom/path/title. Le SID opaque reste le seul identifiant de
  la transition.
- La réintroduction de capacité vérifie d'abord un Bundle publié sans clé
  privée, clone ce Store immuable, attache ensuite la vraie chaîne et la vraie
  clé leaf pour ouvrir la ligne circle, refuse une mauvaise clé, puis prouve que
  le Store keyless original vérifie toujours après retrait de la copie capable.

## Lot 5 — CLIENT-OWN-001, CLIENT-DEL-001/002 et CLIENT-RW-001

### CLIENT-OWN-001 — RED, développement et GREEN

- `MutationIntent` est fermé sur create/edit/delete et porte désormais la zone.
  Les wrappers publics historiques restent compatibles.
- Le bootstrap client initialise les racines Bundle `circle` et `self`; les
  mutations owner passent par les vraies opérations Bundle puis assemblent le
  Store obtenu, Gamma, changeset, evidence et manifest dans un seul package
  draft.2.
- Le Scenario Outline `h-zone-mutation.feature` exécute les 9 lignes
  public/circle/self. Chaque plan est producteur-vérifié, cold-vérifié puis lu
  par une vraie `OwnerSession`; delete est observé comme absent et le package
  self ne contient ni path ni plaintext.

### CLIENT-DEL-001 — RED, développement et GREEN

- Le RED initial était l'absence de racines protégées, puis l'absence de
  publication des lignes de clés. `prepare_generic_grant` expose désormais la
  phase Core sans manifest legacy, afin que le client publie certificat,
  headers/wraps et Gamma dans la même édition draft.2 owner.
- `PublicationPlan::build_grantee` accepte `MutationIntent`; circle et self
  appellent `Bundle::grantee_content_operation`, qui revalide chaîne,
  révocations, perimeter et possession juste avant mutation.
- Les 10 lignes exigées sont vertes : public 3, circle 3 et self 4, incluant
  create self de zone et create self sur SID préalloué. Le manifest est signé
  par le leaf, `authorized_via` égale la chaîne présentée et aucune signature
  owner n'est utilisée pour l'édition grantee.
- Le défaut Core découvert sur deux éditions successives partageant le même
  sidecar d'evidence vide a été corrigé : une clé content-addressée identique
  est dédupliquée uniquement si les octets/digests sont identiques; une
  collision divergente reste refusée. Un test CB12 verrouille ce cas.
- Gate runner natif Phase E : **50/50 scénarios, 261/261 steps**. Tests Rust
  dédiés owner et grantee multi-zone : **3/3**.

### Restant du lot

- CLIENT-DEL-002 couvre maintenant directement le nouvel intent protégé :
  sibling circle, autorité directory sur self, chaîne expirée, mauvaise leaf et
  entropy de mutation réutilisée sont refusés sans `PublicationPlan`.
- Le RED sibling/self-dir a révélé que le préflight client quittait trop tôt
  pour circle/self et laissait Bundle remonter un `Protocol(InvalidMandate)`.
  Le préflight résout désormais le SID/folders/tags circle et le SID opaque
  self, appelle `covers_section_op`, et rend le verdict stable
  `OperationNotCovered` avant assembly.
- CLIENT-RW-001 est GREEN : un vrai grant `read` exact est publié après deux
  sections public/circle/self, une nouvelle `GranteeSession` lit le corps
  cible, refuse le sibling `OperationNotCovered`, puis refuse toute lecture
  après lock avec `SessionLocked`.
- Le RED circle a découvert que `read_section_as_agent` repassait par l'ancien
  `verify_op` sans SID : un mandat `EthosId` valide était donc rejeté. La
  lecture Core appelle maintenant `check_grantee_section` et son
  `SectionOp` exact avant acquisition de clé.
- La lecture self utilise le SID opaque résolu owner au grant; le client ne
  publie ni path ni structure self et le grantee ouvre uniquement la ligne
  exacte livrée.
- L'ancien `perform_public_mutation` transversal a été supprimé. Deux marqueurs
  scellés distincts bornent désormais les usages internes :
  `MutationSigningOperation` pour une mutation déjà validée et
  `ProviderSigningOperation` pour une enveloppe provider. Les seeds et la clé
  de signature ne sortent jamais du keyholder.

## Lot 6 — WASM-MUT-001 à WASM-MUT-004

### WASM-MUT-001/002/003/004 — GREEN

- `BrowserRuntime` expose les opérations génériques `owner_mutation`,
  `grantee_mutation` et `mutation_grant`; les clés restent derrière
  `BootstrapHandle`/`KeyHandle` et les packages derrière `PlanHandle`.
- Le nouveau Scenario Outline `i-wasm-mutation.feature` reconstruit un
  baseline indépendant pour chacune des **19 mutations**. Les inputs natif et
  navigateur utilisent exactement les mêmes temps et entropies; chaque ligne
  compare tous les artefacts et le package digest.
- Les 9 owner et 10 grantee, y compris le grant circle/self et le SID self
  préalloué, sont byte-identical. Les résultats self sont scannés pour
  l'absence du path et du plaintext.
- Gate WASM Cucumber : **41/41 scénarios, 202/202 steps**. Test séquentiel
  supplémentaire des 9 mutations owner : **1/1**.

- WASM-MUT-004 ajoute trois scénarios dédiés aux nouveaux planners : tous les
  alias d'un bootstrap verrouillé reçoivent `session_locked`, une clé grantee
  verrouillée reçoit `keyholder_locked`, et un runtime rechargé refuse l'ancien
  bootstrap par handle invalide même après re-téléchargement/cold verify des
  artefacts. Aucun `PlanHandle` n'est créé dans ces refus.
- `BootstrapHandle.lock()` est exposé dans Rust/WASM, le wrapper JS et les
  types npm. Le runtime détruit la capacité et garde un tombstone jusqu'au
  reset pour que les alias ne puissent pas la réanimer.
- Le build navigateur release et `wrapper-security.test.mjs` sont verts.

## Lot 7 — SDK-PUB/CAS/DOWNLOAD/ROUNDTRIP/ERROR

### SDK-PUB-001/002 — RED, corrections de frontière et GREEN

- Le premier transport réel a révélé que l'enveloppe utilisait le head de
  manifest pour `gamma/*.jsonl`. `PublicationPlan` conserve maintenant le
  prédécesseur Gamma distinct et le provider envelope choisit le CAS selon la
  route.
- Les certificats Bundle étaient stockés en JSON indenté alors que le provider
  exige leur forme JCS signée. Les trois chemins de création de mandat écrivent
  désormais les bytes JCS canoniques.
- Un package froid contient tout le Store, mais un upload delegated ne doit
  pas réécrire les objets inchangés hors perimeter. `upload_order` est
  maintenant le delta byte-exact du baseline, suivi de `manifest.json`.
- La matrice transport owner/grantee × public/circle/self est verte :
  **6 scénarios, 36 steps**, vrai `aithos-store-api`, bodies exacts, acteur et
  chaîne exacts, manifest dernier.

### SDK-CAS-001 — GREEN

- Replay du plan commis : `already_committed`, y compris lorsque le premier
  conflit observable est le CAS Gamma; le SDK relit alors `/heads` et compare
  le head de manifest.
- Concurrence stale : `CasConflictError` expose le head manifest gagnant et
  aucun retry automatique de l'édition perdante n'est effectué.
- Coupure transport avant le manifest successor : erreur `transport`, head de
  manifest resté sur la genèse.
- Gate ciblé : **3 scénarios, 15 steps**.

### SDK-DOWNLOAD-001 / SDK-ROUNDTRIP-001 — GREEN

- Le client expose des enveloppes de lecture fermées et purpose-bound pour
  `/heads`, GET objet et `/batch`; aucune méthode ou cible arbitraire à signer
  n'est exposée.
- `ProviderClient.downloadSnapshot` lit le tip, le files map et le slot courant
  `manifests/<height>.json`, batch les objets, construit un tableau neuf et
  appelle `AithosClient.coldVerify` avant de rendre le snapshot.
- Un vrai restart change le PID tout en conservant objets, nonces et heads.
  Le vérificateur est une nouvelle instance WASM; la capacité privée est
  réintroduite seulement après le verdict keyless.
- Deux cas hostiles modifient ou retirent réellement un objet dans le backend
  temporaire et sont refusés comme `artifact_invalid` / `artifact_missing`.
- Le gate vertical complet est vert : **19 scénarios, 133 steps**, les 19
  mutations owner/grantee public/circle/self sont publiées, redémarrées,
  téléchargées, cold-vérifiées et relues.
- Le RED self préalloué a fermé CLIENT-RW-001 : la lecture grantee porte
  désormais sa zone réelle; self se résout uniquement par SID opaque et la
  chaîne exacte.

### Compteur SDK

- Runner SDK complet après ajout : **38 scénarios, 226 steps** avant mise à
  jour du manifest; 37/38 étaient verts, HARN-001 a correctement refusé le
  changement de compteur.
- Manifest explicite finalement mis à jour : total 1254, selected 1235,
  client Phase E 50, WASM 41, SDK 38. Relance complète GREEN :
  **38/38 scénarios, 226/226 steps**.

## Lot 8 — PROVIDER-DEL/COLD/CAS

- La publication delegated réelle et le cold restart sont prouvés par le
  binaire, mais la matrice provider Cucumber native `PROVIDER-COLD-001..008`
  et les trois courses concurrentes restent à activer dans son runner propre.
- Le path-map provider autorise maintenant un mandat exact-id à atteindre
  l'index de sa zone (chemin sans SID), sans ouvrir l'index d'une autre zone;
  Core demeure l'autorité qui vérifie le delta signé.

## Lot 9 — E2E-MUT/READ/AUTH/FAIL/SEC

- E2E-MUT-001 est vert sur les 19 lignes via le runner SDK.
- E2E-FAIL couvre déjà coupure avant manifest, objet manquant et substitué.
- Restent les six lignes list/read de perimeter, expiry/revocation entre plan
  et commit, réponse perdue après commit, coupure sync et le scan non-fuite
  transverse complet.

## Lot 10 — CORE-SEM/CONSTRAINT/OBLIGATION/COUNT/VAULT

### Déproxyfication Core en cours

- Gate Core complet après les changements : **815/815 scénarios, 3505/3505
  steps**.
- `CORE-CONSTRAINT-001` n'utilise plus le résultat global CB5 pour les 23
  cellules de la matrice D7. Core expose une décision fermée et typée
  `constraint_requirement`; chaque ligne compare séparément applicabilité et
  preuve cold requise. La parité append/cold et le refus d'une extension
  inconnue sont exécutés directement.
- Les quatre combinaisons draft.1/draft.2 de `max_children`, ses cinq cas
  d'atténuation, le comptage direct enfant/petit-enfant, la migration
  homogène, les trois formes root/leaf et le traitement owner sont désormais
  scenario-driven. Les tests vérifient les liens Gamma et l'immutabilité des
  certificats historiques.
- U1 action/inference valide chaque table fermée, la signature, le binding à
  l'operation ref et les 31 refus. Un receipt v1 historique passe uniquement
  par `verify_receipt` et est refusé par `verify_u1_receipt`.
- R2 valide les deux tables fermées et les 25 défauts typés. Les neuf matchers
  draft.3, les 24 défauts matcher/chaîne, les six opérations avec receipt, la
  co-signature, les quatre preuves tier-X et la parité fresh-store appellent
  directement `verify_r2_receipt`, `verify_obligation`,
  `verify_obligation_chain` ou `verify_u1_receipt`.
- Les 11 lignes de compteurs conceptuels appellent séparément
  `verify_delegated_counts` et contrôlent kind, domaine, acteur et deltas.
- Les quatre scénarios compteurs agrégés ont aussi été déproxyfiés : profil
  historique fermé, 36 défauts `InvalidDelegatedCounts`, 13 défauts
  `InvalidMandate`, et normal/merge/résolution comptent chacun deux mutations
  plus exactement une unité publisher.
- Les deux scénarios H2 de frontière « roots ≠ autorité » et de parité
  append/cold utilisent maintenant un refus Bundle self hors périmètre avec
  reopen réel, puis deux `GammaReplayState` reconstruits depuis des bytes
  rechargés; heads, compteurs et D7 doivent être identiques.
- Le scénario append/reopen de révocation et autorité reconstruit un vrai
  Bundle, exporte dans un store neuf et n'utilise plus CB6/CB9.

### Dette explicite restante

- Il reste des appels proxy dans les groupes `CORE-SEM-001`, le
  catalogue/overlay et le vault. En particulier,
  `cb6_semantic_verdict` lit encore des drapeaux de vecteur au lieu de faire
  admettre une vraie entrée par `GammaReplayState`; ce bloc n'est donc pas
  compté comme couverture réelle. Le relevé statique courant compte **41**
  appels directs `cbN_result`/`cbN_assert_green` dans les steps release
  (contre 67 au début de cette tranche), et **44** en incluant les wrappers
  spécialisés `cbN_*_result`.
- La définition de done « aucun `cbN_result`/`cbN_assert_green` » reste
  volontairement non cochée jusqu'à suppression de ces derniers appels.

## Lot 11 — SDK owner authority reconnectée et refresh délégué (2026-07-23)

### Autorité owner après reload — GREEN

- `MemoryOwnerAuthority` peut désormais construire les mêmes neuf mutations
  `create/edit/delete × public/circle/self` que le bootstrap, sans réutiliser
  le handle de genèse.
- Les bindings runtime, WASM, JavaScript et TypeScript exposent
  `OwnerAuthorityHandle.publish(snapshot, mutation)`.
- `OwnerAuthorityHandle.grantMutation` émet et publie aussi les mandats
  génériques public/circle/self depuis l'autorité reconnectée; les scénarios
  délégués n'utilisent plus le bootstrap pour créer leur chaîne.
- `OwnerAuthorityHandle.providerReadEnvelope` signe les lectures fermées
  `/heads`, objet et `/batch`; `ProviderClient.downloadSnapshot` accepte cette
  autorité sans owner bootstrap.
- Les scénarios SDK recréent réellement un second runtime, cold-vérifient le
  baseline, réimportent l'autorité depuis la recovery owner, publient, tuent et
  redémarrent le provider, puis téléchargent dans un troisième runtime.
- La clé owner reste une capacité séparée, utilisée uniquement après cold
  verification pour ouvrir le plaintext `circle/self`; l'autorité de
  publication ne devient pas une API de déchiffrement.

### Refresh délégué dans le périmètre — GREEN

- Un téléchargement complet naïf signé par un mandat étroit a été observé RED :
  le provider refusait correctement les objets inchangés hors périmètre.
- `downloadSnapshot` accepte maintenant un `baselineArtifacts` déjà vérifié.
  Il réutilise seulement les bytes dont le digest est inchangé entre les deux
  manifests, télécharge sous la signature du délégué uniquement le delta, puis
  cold-vérifie l'ensemble dans le runtime neuf.
- Les dix lignes déléguées du roundtrip utilisent désormais le grantee et sa
  chaîne pour le download post-restart, et non l'owner. Les refus latéraux
  public/circle/self restent couverts côté client Phase E.

### Compteurs et gates

- SDK Node : **34/34 tests**.
- SDK Gherkin : **47/47 scénarios, 289/289 steps**.
- Manifest release : **1263 total, 1244 sélectionnés, SDK 47**.
- Client Phase E : **50/50 scénarios, 261/261 steps**.
- WASM Gherkin : **41/41 scénarios, 202/202 steps**.
- Tests natifs ciblés : autorité owner reconnectée multi-zone et enveloppes
  provider owner reconnecté GREEN.

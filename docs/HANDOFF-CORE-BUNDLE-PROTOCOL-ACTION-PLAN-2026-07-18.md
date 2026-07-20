# Handoff — finalisation protocolaire `aithos-core` + `aithos-bundle`

**Date :** 2026-07-18

**Dépôt :** `/Volumes/Math17/aithos/v2/code/aithos-core`

**Branche observée :** `feat/obligations` — ne pas la changer

**HEAD observé :**

`cda4f058708a5a43c5b21870bf0e1bce925d74e1`

**Statut :** plan d'action détaillé ; le runtime provider reste gelé sur les contrats
protocolaires jusqu'au gate final Core + Bundle.

**Périmètre de code autorisé :**

- `rust/crates/aithos-core/**`
- `rust/crates/aithos-bundle/**`
- specs, features et vecteurs strictement nécessaires à ces deux crates

**Aucun push, merge, changement de branche ou déploiement n'est demandé.**

---

## 0. Objet du handoff

Terminer intégralement le protocole utile au produit dans les crates qui en sont
propriétaires, avant :

- l'extension d'`aithos-client` aux mutations ;
- l'intégration protocolaire du provider cloud ;
- la construction du SDK réseau hors de ce dépôt.

À la sortie de cette tranche, toute opération autorisable par un mandat doit être :

- exprimable et sérialisable ;
- atténuable et révocable ;
- évaluée par un verdict pur unique ;
- journalisée et rejouable ;
- transformable en édition publiable ;
- vérifiable sans clé privée ni plaintext depuis un store local vierge.

Une mutation Ethos, structurelle ou vault est en plus exécutée atomiquement dans un
bundle. Une action connecteur externe est autorisable, consommable, journalisable
et transformable en plan/preuve par Core + Bundle ; son effet upstream réel reste
le gate Gateway ultérieur.

Cela couvre `public`, `circle`, `self`, Gamma, délégation, révocation,
contraintes, éditions, structure, connecteurs et vault.

Cette tranche prépare exactement le paquet et le verdict dont le provider aura
besoin. Elle ne prétend pas réaliser l'aller-retour HTTP du provider : ce dernier
reste un gate séparé.

---

## 1. Autorité documentaire

Lire entièrement, dans cet ordre, avant toute modification :

1. `docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md`
2. `docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md`
3. le présent handoff
4. `docs/NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md`
5. `README.md`
6. `spec/00-*.md` à `spec/10-*.md`
7. le plan d'exécution et les handoffs mandats cités par le handoff principal
8. les rituels BDD, vectors-first et pure-core du dépôt
9. les features et le code réel visés ci-dessous

Le handoff Lot 1 rend D1–D9 opposables. Il prévaut sur les formulations historiques
contraires qu'il identifie. Il ne fige pas à lui seul de nouveaux octets signés.

Le présent handoff acte une décision de séquencement plus récente de Mathieu. Il
remplace uniquement, dans les handoffs précédents :

- l'ordre qui lançait l'intégration provider avant la fermeture des connecteurs,
  du vault et du cold verify Core + Bundle ;
- l'obligation de créer dans le même gate les features provider/gateway alors que
  leurs arbres appartiennent à d'autres pistes.

Les contrats downstream ne sont ni supprimés ni déclarés complets : ils sont
différés à leurs handoffs dédiés. CB1 peut donc fermer le contrat **Core + Bundle**
sans prétendre fermer le Lot 1 global ou le protocole produit.

Si une spec, un scénario, un vecteur et le code divergent, ne choisir aucun des
quatre silencieusement. Consigner la contradiction et appliquer le gate humain
prévu par le rituel.

---

## 2. Décision de séquencement

Le chemin critique devient :

```text
contrats Gherkin validés
→ vecteurs indépendants
→ forme et périmètre core
→ verdict pur unique
→ contraintes et rejeu Gamma
→ transaction bundle
→ parité des mutations et grants
→ structure, révocation et vault
→ changesets et éditions déléguées
→ export/import et cold verify
→ gate Core + Bundle
→ reprise protocolaire du provider
```

Le provider peut continuer uniquement les travaux indépendants listés dans la note
qui lui est destinée. Toute route, erreur, enveloppe ou transaction qui dépend du
contrat de publication reste fail-closed.

---

## 3. Séparation stricte des responsabilités

### `aithos-core`

Propriétaire de la logique protocolaire pure :

- formes canoniques, JCS, hashes et signatures ;
- algèbre d'opérations et de périmètres ;
- chaînes, atténuation, proof of possession et révocation ;
- contraintes et consommation ;
- classes connecteurs et catalogue approuvé ;
- construction/vérification sémantique de Gamma ;
- preuves publiques génériques ;
- verdict d'autorisation et verdict de publication.

Interdits :

- I/O ;
- réseau ;
- accès implicite à l'horloge ;
- RNG implicite ;
- chemins de fichiers ;
- stockage ;
- secret manager ou runtime connecteur.

Tout temps, état, nonce, hasard, receipt ou fait externe est fourni explicitement
en entrée.

### `aithos-bundle`

Propriétaire de :

- layout des bundles ;
- stores locaux et transaction logique ;
- chiffrement, headers, wraps et lignes ;
- mutations de contenu et de structure ;
- manifestes, éditions et changesets concrets ;
- import/export d'artefacts ;
- assemblage du paquet de publication ;
- vérification froide du layout puis délégation au verdict core.

Le bundle ne réimplémente jamais `covers`, les contraintes, la révocation, le
lattice, une classe connecteur ou une règle Gamma.

### Surfaces aval

| Consumer futur | Reçoit ou appelle | Ne doit jamais dupliquer |
|---|---|---|
| Provider | paquet public/opaque, verdict keyless et faits CAS | chaîne, périmètre, contraintes, Gamma, changeset ou autorité |
| Gateway | opération typée, catalogue approuvé, verdict et plan bundle | classification, wildcard, receipt, JCS ou règle vault |
| CLI/WASM | builders, DTO et résultats typés | JSON signé, hashes, signatures ou logique métier |
| Client offline | session locale, mutation, export, import et cold verify | moteur de mandat, contraintes ou édition |
| SDK externe | bytes canoniques et résultat CAS provider | protocole ; il ne fait que transport, auth et retry |

---

## 4. Hors périmètre explicite

Les éléments suivants restent hors scope et ne doivent pas être modifiés dans cette
tranche :

- fichiers `rust/crates/aithos-provider/**` ;
- routes HTTP, auth de transport, backend durable, CAS serveur, witness,
  tunnel, Docker, Terraform ou CI provider ;
- runtime, hub, proxy, OAuth, broker, credentials ou appel connecteur
  d'`aithos-gateway` ;
- commande CLI ou surface WASM nouvelle ;
- mutation dans `aithos-client` ;
- `RemoteStore`, client HTTP ou SDK réseau ;
- orchestration multi-Ethos ou multi-chaînes.

Les tests workspace et WASM sont des gates de compatibilité, pas une autorisation à
y copier une règle.

Ne modifier aucun scénario aval existant et ne retagger aucun scénario vert. Les
nouveaux contrats provider, gateway, CLI/WASM et client sont différés ; lorsqu'ils
seront créés dans leur piste, ils commenceront `@wip`. Un export vers un store local
vierge n'est pas nommé « E2E provider ».

---

## 5. État réel à reprendre

### Core

Points de départ connus :

- `rust/crates/aithos-core/src/mandate.rs`
  - sépare encore opérations Ethos et `ActOp` (`mandate.rs:431-465`) ;
  - `id=` n'existe pas dans le périmètre Ethos (`mandate.rs:68`) ;
  - `verify_op` compose seulement une partie du verdict attendu
    (`mandate.rs:785`) ;
  - le wildcard action n'exprime pas encore la séparation complète
    `read/act/binding`.
- `rust/crates/aithos-core/src/constraints.rs`
  - plusieurs contraintes sont parsées et/ou atténuées sans matrice d'exécution
    exhaustive ;
  - `max_children` appartient encore aux contraintes supprimables
    (`constraints.rs:814`) ;
  - la validation typée complète d'un root n'est pas systématique.
- `rust/crates/aithos-core/src/gamma.rs`
  - possède des primitives de liens, signatures et checks ciblés ;
  - les checks d'action et de grant restent séparés
    (`gamma.rs:635`, `gamma.rs:710`) ;
  - ne fournit pas encore le rejeu froid sémantique complet de toutes les
    consommations.
- `rust/crates/aithos-core/src/revocation.rs`
  - doit être intégré au même verdict append-time/cold-time.

### Bundle

Points de départ connus :

- `rust/crates/aithos-bundle/src/lib.rs`
  - `Store` n'offre actuellement que `get`, `put` et `list` (`lib.rs:24`) ;
  - aucune transaction générale n'est exprimée.
- `rust/crates/aithos-bundle/src/bundle.rs`
  - rewrite/delete owner restent limités à `circle`
    (`bundle.rs:565-577`, `bundle.rs:617-628`) ;
  - `publish` reste owner-only (`bundle.rs:1155`) ;
  - la vérification refuse une édition déléguée normale
    (`bundle.rs:1160-1180`) ;
  - les artefacts d'édition utiles au provider ne forment pas encore une façade
    publique stable (`bundle.rs:124`).
- `rust/crates/aithos-bundle/src/grants.rs`
  - les écritures grantee restent essentiellement codées pour `circle` ;
  - des blobs et index peuvent être écrits avant la journalisation Gamma
    (`grants.rs:618-655`, `grants.rs:674-709`, `grants.rs:727-747`).
- `rust/crates/aithos-bundle/src/log.rs`
  - recalcule encore certaines décisions de périmètre/action ;
  - `gamma_verify` ne réalise pas le rejeu protocolaire intégral (`log.rs:616`).
- `rust/crates/aithos-bundle/src/manifest.rs`
  - sait vérifier une signature delegate, mais délègue encore la chaîne et
    l'autorité au caller (`manifest.rs:173`).
- `rust/crates/aithos-bundle/src/state.rs`
  - expose des primitives de Merkle et de diff ;
  - le diff n'est pas encore le changeset typé et autorisé exigé.
- `rust/crates/aithos-bundle/src/merge.rs` et `revoke.rs`
  - conservent des limitations owner/circle et devront rejoindre le moteur commun.

Ne contourner aucun de ces gaps dans le provider ou une surface.

---

## 6. Invariants non négociables

1. L'owner agit avec sa capacité locale, sans mandat.
2. Un grantee agit uniquement avec sa clé privée — ou une capacité cryptographique
   équivalente — **et** une chaîne valide.
3. Pouvoir déchiffrer ou signer ne constitue jamais une autorisation.
4. Toute opération refusée l'est avant effet canonique.
5. Append-time et cold replay appellent la même règle pure.
6. Une ligne de clé sans mandat n'autorise rien ; un mandat sans ligne ne permet
   pas de déchiffrer.
7. Une mutation owner est journalisée mais ne consomme aucun mandat.
8. Une mutation grantee consomme révocation, contraintes et compteurs.
9. Une édition déléguée v1 a un seul acteur et une seule chaîne.
10. Une édition n'est publiable que si chaque changement est expliqué par Gamma et
    couvert par le même acteur/chaîne.
11. Le provider ne reçoit ni clé de contenu, ni secret local, ni plaintext
    `circle`, `self` ou vault.
12. Le wildcard ne couvre jamais une opération `binding` ni `.config`.
13. Un droit d'action connecteur ne livre jamais un credential.
14. Une erreur publique ne révèle aucune donnée scellée.
15. Aucun helper public ne doit permettre de fabriquer un verdict positif partiel.
16. Une session locale porte un seul Ethos, un seul acteur et, pour un grantee, une
    seule chaîne ; plusieurs Ethos ou mandats sont orchestrés par plusieurs
    sessions isolées, sans autorité globale ambiante.
17. `aithos-client` reste strictement offline et le SDK réseau reste hors dépôt.
18. Le lattice signé reste :
    - create utilise `append` ou `write` ;
    - edit utilise `edit`, `append` ou `write` ;
    - delete utilise `delete` ou `write` et implique `read` ;
    - `write` couvre le CRUD complet ;
    - aucun nouveau verbe wire `create` n'est introduit silencieusement.

---

## 7. Micro-gates à fermer avant stabilisation des API

Ces points n'autorisent pas un choix silencieux par l'agent.

### G-A — Classification de `.config`

Décision D9 acquise :

- layout exact `/x/<connector>` ;
- capacité exacte `act.x.<connector>.config` ;
- barrière double : mandat valide et ligne de clé exacte ;
- exclusion de tout wildcard ;
- aucun droit d'action ordinaire ne livre le credential.

Relation non encore explicitement ratifiée avec les classes D8.

**Recommandation :** traiter `.config` comme une capacité vault réservée du
protocole, extérieure au catalogue métier `read/act/binding`. Elle reste exacte,
versionnée et non couverte par wildcard. Elle n'hérite pas automatiquement d'un
`co_sign` de classe `binding` ; une obligation doit être explicite.

Fermer ce point au contrat `o-connector-classes-vault.feature`, avant vecteur ou
code.

### G-B — Transaction du `Store`

Le trait actuel ne peut pas rendre l'atomicité opposable.

**Recommandation :**

- mutation calculée dans un snapshot/overlay ;
- `prepare → validate core → commit` ;
- write-set déterministe ;
- aucun helper métier n'écrit directement ;
- `MemStore` remplace son état canonique atomiquement ;
- `FsStore` prépare physiquement hors du répertoire du bundle puis utilise un
  commit atomique et récupérable ;
- tout refus ou panne injectée laisse le répertoire du bundle byte-for-byte
  identique, staging externe compris comme scratch à nettoyer/récupérer ;
- aucun orphelin n'est permis pour une mutation locale échouée ;
- l'exception D3 sur les blobs opaques content-addressed non référencés vaut
  uniquement pour le préchargement explicite d'une publication, hors transaction
  locale du bundle, et jamais pour un état canonique partiel.

Valider cette forme avant CB6. Ne pas confondre cette transaction locale avec le CAS
serveur du provider.

### G-C — Capacités cryptographiques locales

Les futures surfaces ne doivent pas dépendre de seeds privés exportés.

**Recommandation :** les API bundle prennent des capacités opaques et étroites
(`sign`, `open`, `wrap` selon besoin), avec implémentation locale actuelle. Elles ne
prennent pas une clé privée brute quand une interface de capacité suffit.

Le wire n'est pas affecté. Valider la forme Rust avant de déclarer l'API stable.

### G-D — Façade keyless

Répartition recommandée :

- core reçoit des artefacts publics déjà typés et produit le verdict sémantique
  pur ;
- bundle décode, vérifie layout/hashes/atteignabilité, puis appelle core ;
- le provider futur appelle une façade bundle unique et effectue ensuite seulement
  stockage opaque, transaction CAS et transport.

Les noms Rust montrés dans les documents sont conceptuels jusqu'aux vecteurs.

### G-E — Consommation d'une extension inconnue sur un root-feuille

T2 acte déjà que l'extension est structurellement préservée/tolérée et interdit sa
sous-délégation. Il ne faut pas confondre cette validité de certificat avec une
preuve d'exécution.

**Recommandation :** si le Core ne sait ni évaluer l'extension ni prouver qu'elle
est non applicable à l'opération, la consommation refuse avec un verdict typé
`extension non comprise`, visible dans l'audit. Aucune surface ne peut produire un
`Allow` ou déclarer l'extension exécutée.

Valider cette articulation au contrat de contraintes avant CB5.

---

## 8. Rituel obligatoire pour chaque capacité

Pour chaque tranche ci-dessous :

1. écrire ou compléter le Gherkin `@wip` ;
2. obtenir la validation humaine ;
3. committer le contrat seul ;
4. produire un oracle/vecteur indépendant pour tout wire ou octet signé ;
5. ajouter un test qui échoue pour la raison attendue ;
6. implémenter le minimum en TDD dans le crate propriétaire ;
7. retirer uniquement les `@wip` réellement verts ;
8. exécuter le vrai test d'intégration local sans mock du protocole ;
9. exécuter `fmt`, `clippy`, tests ciblés, workspace et WASM ;
10. présenter le gate ;
11. faire un commit étroit.

Interdictions :

- implémenter avant le contrat validé ;
- générer l'oracle avec le Rust testé ;
- retirer un `@wip` sur un test en mémoire si l'artefact doit être durable ;
- grouper plusieurs changements de wire indépendants ;
- corriger une surface aval en y recopiant une logique.

---

## 9. Registre contractuel à maintenir

Créer et tenir à jour une matrice traçable :

```text
spec
→ scénario Gherkin
→ vecteur/oracle
→ fonction core
→ fonction bundle
→ test append-time
→ test cold-time
→ artefact public
→ consumer futur
```

Chaque ligne porte un statut :

- absente ;
- partielle ;
- complète ;
- contradictoire.

Une capacité reste partielle si elle :

- n'existe que dans une spec ;
- est encore `@wip` ;
- fonctionne uniquement en mémoire ;
- n'a pas d'édition chiffrée publiable ;
- impose l'owner malgré un mandat suffisant ;
- ne passe pas export → store vierge → cold verify.

---

## 10. Plan d'action détaillé

### CB0 — Reprise, ownership et baseline

**But :** commencer sans détruire ni capturer les travaux de Mathieu ou du provider.

Actions :

- relever branche, HEAD, `git status --short` et diff ciblé ;
- attribuer chaque fichier déjà modifié/non suivi ;
- ne toucher aucun fichier qui chevauche une autre piste ;
- utiliser un `CARGO_TARGET_DIR` isolé ;
- rejouer la baseline Core, Bundle, workspace et WASM pertinente ;
- enregistrer les échecs préexistants sans les « réparer » hors tranche ;
- mettre à jour la matrice Lot 0 existante, sans la recréer ni rouvrir D1–D9/T1–T3 ;
- figer les octets des mandats historiques sans `id=`, des manifestes et des
  entrées Gamma avant extension additive.

Ownership immédiat connu :

- `rust/crates/aithos-provider/**`, Cargo workspace/lock, vecteurs P et documents
  provider appartiennent à la piste provider en cours ;
- `vectors/README.md` porte déjà une modification de cette piste : ne pas l'éditer
  avant attribution ou séparation explicite, même si de nouveaux vecteurs Core
  doivent être ajoutés à côté ;
- la piste Core + Bundle ne les stage ni ne les commit ;
- si une dépendance exige de toucher `rust/Cargo.toml` ou `rust/Cargo.lock`, arrêter
  et demander une attribution explicite.

Gate de sortie :

- aucun fichier chevauché ;
- baseline consignée ;
- matrice Lot 0 mise à jour sans réouverture des décisions ;
- aucun code protocolaire modifié.

---

### CB1 — Contrats Gherkin Core + Bundle

**Dépendance :** CB0.

Compléter d'abord les features existantes :

- `features/d-bundle.feature`
  - parité owner list/read/create/edit/delete et droits
    `read/edit/append/delete/write` sur `public/circle/self` ;
  - publication, Gamma et rollback.
- `features/e-mandates.feature`
  - lattice D2, forme complète et cas négatifs T3.
- `features/e-mandate-sections.feature`
  - `id=` exact et containment D1.
- `features/f-plus-constraints.feature`
  - atténuation T1/T2 et matrice d'applicabilité D7.
- `features/f-gamma.feature`
  - kinds, append, autorité et rejeu sémantique.
- `features/g-plus-obligations.feature`
  - receipts, obligations et preuves tier X.
- `features/g-revocation.feature`
  - révocation, rotation et atomicité.
- `features/h2-gamma-roots.feature`
  - roots, proofs, compteurs et égalité append-time/cold-time.
- `features/l-delegated-writes.feature`
  - parité grantee `public/circle/self`.
- `features/i-concurrency.feature`
  - conflits, forks et merges locaux ; aucun CAS provider simulé.
- `features/k-integration.feature`
  - E2E offline réel, export/import et store vierge ; jamais renommé E2E
    provider.

Créer si absentes :

- `features/m-delegated-editions.feature`
  - édition normale single-actor/single-chain et cold verify.
- `features/n-structural-mutations.feature`
  - folders, rename, move, tags, subtree delete et rewrap.
- `features/o-connector-classes-vault.feature`
  - catalogue/classes, wildcard, `/x/<connector>` et `.config`.

Appliquer dans le même commit contractuel les redlines minimales déjà actées :

- `spec/05-delegation.md` : `dir`/`tag` ne couvrent jamais `id=` ;
- specs portant le lattice : `delete` implique `read`, et create/edit/delete sont
  mappés sur les verbes existants sans ajouter un verbe wire ;
- spec/texte/vecteur historique sur `max_children` : enfants directs seulement,
  contrainte non supprimable ;
- toute autre contradiction D1–D9/T1–T3 explicitement recensée par le handoff Lot 1.

Produire aussi la matrice normative :

```text
famille de contrainte
× read / mutation / action / grant / revoke / publication
× owner / grantee
× append-time / cold-time
→ applicable / non applicable / preuve publique / preuve d'exécution
```

Gate de sortie :

- tous les scénarios nouveaux sont `@wip` ;
- D1–D9 sont transcrites sans nouveau wire inventé ;
- G-A, G-B, G-C, G-D et G-E sont explicitement validés ;
- commit du contrat isolé ;
- s'arrêter au gate/commit CB1 avant tout vecteur ou code ;
- aucune implémentation.

---

### CB2 — Oracles, vecteurs indépendants et tests rouges

**Dépendance :** contrat CB1 validé et committé.

Créer progressivement, avec générateurs indépendants du Rust :

- mandat historique sans `id=` byte-identique ;
- `id=` : parse, JCS, round-trip, containment et formes invalides ;
- lattice `delete → read` ;
- forme complète :
  version, algorithme, clé annoncée, IDs, nonce, timestamps, doublons,
  `depth=0` ;
- `max_children` non supprimable et limité aux enfants directs ;
- root connu/inconnu et sous-délégation interdite si inconnue ;
- opération canonique et compteurs action/mutation/total ;
- contraintes, receipts et obligations ;
- rejeu Gamma, révocation et freshness ;
- authorship publique grantee liée au hash, SID, opération, édition et
  `authorized_via`, puis engagée par Gamma/manifeste ;
- changeset et édition déléguée single-chain ;
- engagements `self` avant/après/absence ;
- catalogue signé/pincé, classes et wildcard ;
- vault `.config` exact si son layout signé change.

Fichiers probables :

- `vectors/gen-*.py`
- `vectors/*.json`
- `vectors/README.md`
- nouveaux tests dans `rust/crates/aithos-core/tests/`
- tests vectoriels dédiés dans `rust/crates/aithos-bundle/tests/`

Gates :

- chaque test est observé rouge avant code ;
- l'échec correspond à la règle attendue ;
- aucun oracle n'appelle la fonction Rust sous test ;
- aucun octet historique ne change sans vecteur de non-régression.

---

### CB3 — Forme canonique et périmètres dans Core

**Dépendance :** vecteurs mandat rouges de CB2.

Fichiers principaux :

- `rust/crates/aithos-core/src/mandate.rs`
- `rust/crates/aithos-core/src/ids.rs`
- `rust/crates/aithos-core/src/error.rs`
- `rust/crates/aithos-core/src/lib.rs`

Travail :

- ajouter `id` au périmètre Ethos ;
- parser et sérialiser canoniquement ;
- refuser `id&dir`, `id&tag` et les sélecteurs dupliqués ;
- porter le SID dans l'opération section-précise ;
- implémenter :
  - zone entière → `id` ;
  - même `id` → même `id` ;
  - jamais `dir/tag → id` ;
- rendre `delete` couvrant `read` ;
- valider toute la forme T3 avant confiance dans la signature ;
- séparer validation structurelle du root et validation d'un lien ;
- préserver byte-for-byte les mandats historiques sans `id=`.

Tests :

- table parent/enfant exhaustive ;
- table périmètre/opération exhaustive ;
- round-trip exact ;
- formes négatives et erreurs typées.

Gate :

- retrait progressif des seuls `@wip` purement algébriques devenus verts ;
- aucun comportement Bundle nécessaire pour prétendre fermer ces règles.

---

### CB4 — Opération canonique et verdict pur unique

**Dépendance :** CB3.

Créer un module Core dédié, nom à décider après contrat, qui exprime notamment :

- lecture et mutation de section ;
- création, edit, delete, rename et move structurels ;
- cible source et destination ;
- action connecteur et classe approuvée ;
- vault `.config` réservé ;
- lecture/append Gamma ;
- grant, revoke, rotation, merge et publication ;
- acteur, sujet, session, SID et preuves injectées.

Le front door pur agrège :

- forme et signature ;
- preuve de possession de la clé feuille ;
- chaîne et sujet uniques ;
- temps injecté ;
- état de révocation et freshness ;
- périmètre ;
- catalogue/classe connecteur ;
- contraintes et receipts ;
- état Gamma et compteurs.

Introduire des types opaques vérifiés, par exemple une chaîne vérifiée ou une
autorisation consommable, afin qu'un caller ne puisse pas confondre un
`Vec<Mandate>` parsé avec une preuve d'autorité.

Les noms publics et leur wire ne sont pas figés par ce handoff.

Gate :

- clé de contenu seule refusée ;
- mauvaise clé feuille, sujet, SID, session ou preuve rejetés ;
- un seul verdict positif complet ;
- les helpers partiels restent internes ou ne peuvent produire un `Allow`.

---

### CB5 — Contraintes complètes dans Core

**Dépendance :** CB4.

#### CB5a — Structure et atténuation

- valider toutes les contraintes connues dès le mandat root ;
- tolérer une extension inconnue sur un root-feuille en la conservant ;
- refuser sa sous-délégation tant que son atténuation n'est pas prouvable ;
- appliquer la résolution G-E validée à sa consommation et à sa visibilité dans le
  verdict/audit ;
- retirer `max_children` des familles supprimables ;
- exiger sa répétition avec valeur enfant inférieure ou égale ;
- compter uniquement les enfants directs ;
- tester présence, absence, type et valeur de chaque famille.

#### CB5b — Applicabilité et consommation

Implémenter la matrice contractuelle pour :

- fenêtres de validité et d'activité ;
- freshness et heartbeat ;
- sessions et `session_bind` ;
- `first_party_only` ;
- purpose ;
- obligations, receipts et `co_sign` ;
- budgets, spend et attestations ;
- `max_actions` et ses compteurs, réservés exclusivement aux actions connecteurs ;
- contrainte et compteur distincts pour les mutations Ethos ;
- contrainte et compteur distincts pour le total des consommations déléguées ;
- rate limits ;
- paramètres d'action ;
- domain, transparency et notifications selon leur tier.

Règles :

- toute contrainte connue et applicable est évaluée ;
- toute preuve publique exigée mais absente refuse fermé ;
- toute preuve d'exécution tier X non vérifiable keyless est explicitement
  représentée et ne devient jamais un `Allow` silencieux ;
- owner journalisé, mais sans consommation de mandat.

Les noms wire des limites mutation/total et leur migration ne sont figés qu'après
Gherkin validé puis vecteurs indépendants. Ils ne réutilisent pas silencieusement
`max_actions`.

Gate :

- matrice automatisée famille × opération × owner/grantee ;
- mêmes résultats append-time et cold-time ;
- aucune famille connue réduite à un parseur silencieux.

#### CB5c — Classes et catalogue connecteurs

- manifeste de connecteur signé et approbation owner séparée ;
- catalogue content-addressed et version exacte pincée dans l'autorisation ;
- une classe canonique unique par action : `read`, `act` ou `binding` ;
- wildcard limité aux actions `read` et `act` effectivement présentes dans le
  catalogue approuvé ;
- `binding` toujours exact, avec receipt owner réservé `co_sign` ;
- refus d'une action absente, reclassée ou provenant d'une autre version ;
- migration legacy versionnée : un ancien `write` peut se projeter vers `act`
  uniquement selon le contrat de migration ; il ne couvre jamais `binding` et une
  réinscription est requise pour les nouveaux droits ;
- types publics vérifiables permettant au Bundle d'engager manifeste du connecteur,
  approbation owner, hash et version sans reclasser l'action ;
- aucune découverte de connecteur ni aucun appel MCP dans Core.

Gate :

- classes et wildcard sont déterminés uniquement par le catalogue approuvé injecté ;
- les mêmes faits produisent la même décision append-time et cold-time ;
- vecteurs positifs/négatifs couvrent drift, reclassement, version, wildcard,
  `binding`, `co_sign` et migration ;
- `.config` reste sur le chemin réservé décidé par G-A.

---

### CB6 — Rejeu Gamma sémantique dans Core

**Dépendance :** CB4 et CB5.

Faire du rejeu un moteur pur qui, pour chaque entrée et uniquement contre son
préfixe historique :

1. vérifie forme, kind, hash, ordre et temps ;
2. vérifie la signature owner ou grantee ;
3. résout la chaîne dans les certificats injectés ;
4. vérifie proof of possession et `authorized_via` ;
5. applique la révocation forward-only à la date de l'entrée ;
6. vérifie l'autorité de grant/revoke/merge ;
7. reconstruit l'opération canonique ;
8. évalue périmètre et contraintes ;
9. consomme les compteurs ;
10. accepte alors seulement l'entrée dans l'état rejoué.

Le `gamma_verify` Bundle devient un chargeur/assembleur vers ce moteur. Il ne garde
pas une seconde sémantique.

Négatifs obligatoires :

- entrée historique N+1 injectée ;
- receipt rejoué ;
- heartbeat/freshness périmé ;
- consommation après révocation ;
- grant non journalisé ;
- dépassement `max_children` ;
- mutation hors SID ;
- signature valide sous mauvaise chaîne ;
- Gamma structurellement valide mais sémantiquement invalide.

Gate :

- append-time et rejeu froid rendent le même verdict et le même état ;
- les erreurs sont déterministes et sans fuite.

---

### CB7 — Fondation transactionnelle du Bundle

**Dépendance :** CB6 et validation G-B/G-C.

Fichiers probables :

- `rust/crates/aithos-bundle/src/lib.rs`
- nouveau `transaction.rs` ou `write_set.rs`
- nouveau `mutation.rs`
- `bundle.rs`
- `log.rs`
- tests d'atomicité dédiés

Architecture :

- ouvrir un snapshot/overlay ;
- calculer blobs, index, headers, wraps et entrée Gamma candidate ;
- obtenir le verdict Core avant effet ;
- construire un write-set déterministe ;
- committer l'état canonique en une transaction logique ;
- interdire les `put` directs dans les helpers métier.

Tests :

- store injectant une panne à chaque frontière ;
- snapshot avant/après ;
- panne pendant crypto, index, header, wrap, Gamma et commit ;
- reopen réel du `FsStore` après panne ;
- staging situé hors bundle, inatteignable et nettoyé/récupéré ;
- aucune entrée de refus dans Gamma.

Gate :

- tout refus ou panne injectée laisse le bundle byte-for-byte identique ;
- aucune génération, aucun head et aucun objet de mutation échouée ne subsiste dans
  le bundle ;
- aucun nouvel état partiel n'est observable après reopen.

---

### CB8 — Parité owner et grants génériques

**Dépendance :** CB7.

#### Lectures et mutations owner

Passer par un moteur commun pour `public`, `circle` et `self` :

- list/read ;
- create sous autorité `append|write` ;
- modification sous autorité `edit|append|write` ;
- suppression sous autorité `delete|write`, avec lecture implicite ;
- mêmes opérations Gamma ;
- lecture Gamma via l'opération canonique lorsqu'une surface la demande ;
- authorship owner pour `public` ;
- chiffrement pour `circle` ;
- structure scellée pour `self` ;
- aucun mandat consommé.

#### Grants et livraison de clés

Généraliser :

- périmètres Ethos/gamma/action/issue/revoke/config combinés ;
- livraison exacte zone/dir/tag/dir&tag/id ;
- pour `self`, aucune ligne ou autorité structurelle `dir/tag/dir&tag` : seules la
  zone entière et une ligne SID opaque exacte sont permises ;
- ligne de section exacte pour `id=`;
- aucune clé pour action/gamma/issue/revoke ;
- `/x/<connector>` uniquement pour `.config`;
- certificat, headers, wraps et grant Gamma dans la même transaction ;
- délégation et `max_children` évalués avant effet.

Gate :

- aucune divergence entre certificat et clés livrées ;
- parité owner durable dans les trois zones ;
- refus latéral avant tout effet.

---

### CB9 — Mutations déléguées complètes

**Dépendance :** CB3 à CB8.

Fichiers probables :

- `grants.rs`
- `bundle.rs`
- `log.rs`
- `state.rs`
- moteur commun de mutation
- tests Cucumber et vectoriels

Couvrir :

- list/read sur `public`, `circle` et `self`, avec refus latéral identique aux
  mutations ;
- create sous `append|write`, modification sous `edit|append|write`, suppression
  sous `delete|write` ;
- zone, dir, tag, dir&tag et id selon les règles de chaque zone ;
- pour une mutation `self`, zone entière ou `id=<sid>` opaque uniquement :
  `dir`, `tag` et `dir&tag` ne la couvrent jamais ;
- création `self` par zone entière ou SID préalloué exact ;
- lecture Gamma mandatée, avec le même contrôle chaîne/révocation/contraintes et le
  même rejeu froid ;
- authorship grantee signée pour `public` ;
- aucune imitation de signature owner ;
- révocation, contraintes et compteurs via le verdict Core ;
- Gamma et nouvel état dans la même transaction ;
- owner absent, sauf receipt `co_sign` explicitement applicable ;
- révocation ou expiration survenue après ouverture d'une session.

Gate :

- scénarios pertinents de `l-delegated-writes` et
  `e-mandate-sections` verts ;
- lecture, mutation, édition future et rejeu portent le même acteur/chaîne ;
- list/read et Gamma read sont validés après export/import depuis un store vierge ;
- bundle inchangé après chaque refus.

---

### CB10 — Structure, révocation et vault

**Dépendance :** CB9.

#### Structure

- create/delete folder ;
- rename ;
- changement de titre, nom et tags ;
- move avec autorité sur source et destination ;
- suppression couvrant tout le sous-arbre ;
- index et vues tags dérivés atomiquement ;
- rewrap/rotation appartenant à la transaction logique.

#### Révocation et rotation

- revoke → rotation → rewrap/ré-encryption → Gamma ;
- cascade et réadoption ;
- move-as-rotation ;
- vérification froide de l'autorité et des survivants ;
- rotations de racine de confiance owner-only.

#### Vault

- layout isolé `/x/<connector>` ;
- DK, header, lignes et rotation indépendants ;
- `.config` exact et hors wildcard ;
- mandat et ligne exacte tous deux nécessaires ;
- capacité d'audit cryptographiquement distincte ;
- aucun appel connecteur, upstream ou secret manager.

Gate :

- features `g-revocation`, `n-structural-mutations` et
  `o-connector-classes-vault` vertes sur leur périmètre Core + Bundle ;
- aucun effet connecteur réel ;
- un droit `act` seul ne permet pas d'ouvrir le vault.

---

### CB11 — Changesets et éditions déléguées

**Dépendance :** toutes les mutations et CB6.

Fichiers probables :

- `rust/crates/aithos-bundle/src/manifest.rs`
- `rust/crates/aithos-bundle/src/state.rs`
- `rust/crates/aithos-bundle/src/merge.rs`
- `rust/crates/aithos-bundle/src/bundle.rs`
- nouveaux `changeset.rs` et/ou `edition.rs`

Travail :

- dériver un changeset typé depuis deux états, jamais l'accepter comme simple
  affirmation ;
- relier chaque changement atteignable à une opération Gamma autorisée ;
- détecter modification parasite, omission et changement inexpliqué ;
- supporter une édition normale owner ou grantee ;
- imposer single-actor/single-chain en v1 ;
- référencer les certificats/hashes nécessaires au cold verify ;
- lier authorship publique au hash du contenu, SID, opération, édition,
  `authorized_via` et chaîne ;
- engager cette preuve d'authorship par Gamma et par le manifeste ;
- produire les preuves `self` d'inclusion, remplacement, retrait et absence sans
  exposer la structure ;
- engager manifest, roots, changeset et Gamma head dans la même transaction ;
- revérifier fork/merge/résolution contre tous les changements.

Gate :

- une édition déléguée normale est acceptée si et seulement si tous ses changements
  appartiennent au même acteur et à la même chaîne ;
- aucune intervention owner implicite ;
- modification parasite refusée avant publication.

---

### CB12 — Paquet de publication et cold verify local

**Dépendance :** CB11.

Stabiliser d'abord une session locale explicitement mono-Ethos/mono-acteur et, pour
un grantee, mono-chaîne. Elle reçoit les capacités opaques validées par G-C et expose
lecture, mutation, préparation de publication, import et vérification. Elle ne
contient ni autorité globale, ni réseau, ni orchestration de plusieurs sessions.

Le Bundle expose un paquet déterministe séparant :

- enveloppe publique vérifiable ;
- manifestes et changesets ;
- certificats et preuves publiques ;
- catalogue/approbations publics utiles ;
- manifeste, hash, version et approbation owner du catalogue connecteur pincé ;
- delta et preuves Gamma ;
- roots et parent/height attendus ;
- inventaire d'objets opaques content-addressed ;
- faits nécessaires au futur CAS, sans implémenter ce CAS.

La façade keyless :

1. décode et valide versions/formes ;
2. vérifie hashes, pins, layout et atteignabilité ;
3. assemble les artefacts publics typés ;
4. appelle le verdict pur Core ;
5. retourne un résultat typé : sujet, édition, parent attendu, heads/roots,
   objets atteignables et reason code ;
6. ne retourne jamais de clé ou donnée sensible.

#### Cold roundtrip obligatoire

1. créer une édition owner puis une édition grantee ;
2. exporter uniquement les artefacts prévus ;
3. les copier vers un `FsStore` vierge ;
4. détruire l'instance productrice et retirer toutes les capacités privées du
   processus de vérification ; conserver seulement, hors de ce processus, les
   capacités nécessaires au test fonctionnel final ;
5. rouvrir depuis le store vierge ;
6. vérifier keyless ;
7. dans une phase/processus séparé, réintroduire les capacités owner/grantee et
   rouvrir les contenus afin de confirmer la conservation fonctionnelle.

Négatifs :

- retrait, substitution ou ajout non pincé ;
- certificat manquant ;
- mauvais parent ou hauteur ;
- Gamma tronqué ;
- compteur dépassé ;
- preuve `self` falsifiée ;
- révocation stale ;
- mauvaise signature ;
- artefact canonique non engagé.

Gate :

- le futur provider peut accepter/refuser en appelant une seule façade puis son
  CAS ;
- aucune règle ne reste à réimplémenter côté serveur ;
- ce test est nommé export/import local, pas roundtrip provider.

---

### CB13 — Concurrence, consolidation et gate final

**Dépendance :** CB12.

Concurrence :

- forks/merges/résolutions owner et grantee ;
- conflit sans modification du head canonique ;
- merge automatique seulement pour changements réellement disjoints ;
- même nœud soumis à une résolution par autorité couvrante ;
- recomposition correcte des contraintes et compteurs ;
- cold verify après merge et résolution ;
- résultat indépendant de l'ordre d'insertion des objets opaques.

Gate final Core + Bundle :

- aucun `@wip` Core + Bundle du périmètre retenu ;
- aucune mention « later pass » ou contradiction résiduelle dans les specs du
  périmètre fermé ;
- tous les vecteurs indépendants verts ;
- aucun helper public ne rend un verdict partiel positif ;
- aucune écriture métier hors moteur transactionnel ;
- append-time et cold replay identiques ;
- parité list/read/create/edit/delete et Gamma read owner/grantee sur les zones où
  le protocole les autorise ;
- aucune clé/plaintext dans les artefacts publics, logs ou erreurs ;
- matrice de contraintes entièrement opposable ;
- export → `MemStore`/`FsStore` vierge → cold verify réel ;
- version wire stabilisée, additive/migrable et couverte par non-régression ;
- threat model, limites résiduelles et propriétés non vérifiables keyless
  explicitement documentés ;
- mini-consumer de compilation prouvant l'usage des API sans logique parallèle ;
- inventaire exact :
  `besoin consumer → type/fonction → fixture/vecteur` ;
- rapport séparé des travaux restant provider/gateway/surfaces.

Commandes minimales depuis `rust/`, avec target isolé :

```bash
cargo test -p aithos-core --locked
cargo test -p aithos-bundle --locked
cargo clippy -p aithos-core -p aithos-bundle --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo test --workspace --locked
cargo check -p aithos-wasm --target wasm32-unknown-unknown
```

Ajouter le runner des vecteurs indépendants et le Cucumber réel applicable. Ne pas
masquer un échec par un mock ou un `@wip` retiré prématurément.

CB13 est le gate de reprise du Provider, pas le gate protocole produit global. Le
gate global reste ouvert jusqu'aux vrais E2E Provider/Gateway et à l'adaptation
mince des surfaces prévues par leurs handoffs.

---

## 11. Lots de commits recommandés

Chaque ligne est un commit distinct après son gate :

1. contrats Gherkin seulement ;
2. oracles/vecteurs seulement ;
3. forme et `id=` Core ;
4. opération/verdict Core ;
5. contraintes Core ;
6. rejeu Gamma Core ;
7. transaction Bundle ;
8. parité owner/grants ;
9. mutations déléguées ;
10. structure/révocation/vault ;
11. changesets/éditions ;
12. paquet/cold verify ;
13. concurrence et consolidation.

Un commit ne mélange pas :

- contrat et implémentation ;
- deux changements de wire indépendants ;
- Core/Bundle et provider ;
- logique et reformatage massif ;
- fichiers appartenant à une autre piste.

Stage uniquement les fichiers de la tranche. Présenter le diff indexé avant chaque
commit. Ne jamais pousser sans demande explicite.

---

## 12. Conditions d'arrêt

Arrêter immédiatement et demander une décision si :

- un fichier à modifier contient déjà un travail non attribué ;
- le changement exige `rust/Cargo.toml` ou `rust/Cargo.lock` actuellement détenu
  par la piste provider ;
- le registre vectoriel exige une modification de `vectors/README.md` avant que
  son chevauchement provider soit arbitré ;
- G-A, G-B, G-C, G-D ou G-E n'est pas explicitement validé au moment où il devient
  bloquant ;
- un nouveau champ signé n'a pas de Gherkin validé et de vecteur indépendant ;
- une règle devrait être copiée dans Bundle pour contourner une API Core absente ;
- une vérification keyless requerrait une clé de contenu ou du plaintext ;
- une capacité ne peut pas être rejouée à froid ;
- un test ne peut devenir vert qu'en modifiant provider, gateway, CLI/WASM ou
  client ;
- l'owner doit intervenir alors que le mandat et ses obligations suffisent ;
- une panne peut rendre un manifest visible sans Gamma correspondant.

---

## 13. Définition de « prêt pour reprise Provider »

La publication protocolaire du provider ne reprend que si :

1. CB13 est vert ;
2. les contrats et vecteurs concernés sont committés ;
3. le paquet public/opaque est stable ;
4. la façade keyless est documentée et testée ;
5. les reason codes publics sont stables ;
6. parent, hauteur, heads et faits CAS sont explicités ;
7. le store vierge vérifie froidement owner et grantee ;
8. les objets sensibles sont absents des sorties ;
9. le provider n'a aucune règle à réimplémenter ;
10. l'ownership de la tranche provider suivante est attribué.

La reprise provider suit alors :

```text
appel de la façade keyless
→ mapping mécanique du verdict
→ stockage opaque
→ transaction CAS durable
→ witness/head canonique
→ vrai HTTP avec arrêt/restart
→ téléchargement dans un nouveau store
→ cold verify
```

Ce dernier aller-retour est le gate provider ultérieur. Le présent chantier doit
le rendre possible, pas le simuler.

---

## 14. Résultat attendu

Le livrable final n'est pas seulement une suite verte. C'est un noyau protocolaire
pur et un bundle transactionnel capables de produire, publier conceptuellement et
revérifier à froid les mêmes opérations owner/grantee sur toutes les zones
autorisées.

À ce stade seulement :

- le provider peut devenir un vérificateur keyless et un store opaque ;
- le gateway peut orchestrer sans inventer une classe ou une autorité ;
- CLI/WASM/client peuvent rester des surfaces minces ;
- le SDK externe peut rester un pur composant réseau.

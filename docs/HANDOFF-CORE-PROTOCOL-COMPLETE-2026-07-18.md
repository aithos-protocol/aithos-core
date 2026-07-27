# Handoff — finalisation intégrale du protocole Aithos Core

> **ARCHIVE — plan de finalisation exécuté.** Les surfaces annoncées manquantes
> ont été traitées dans CB1–CB13 ; consulter le code, `spec/` et le gate `522dfcd`.

**Date de préparation :** 2026-07-18  
**Destinataire :** nouvelle session/agent chargé de terminer `aithos-core` avant
l'extension fonctionnelle d'`aithos-client` et la création du SDK réseau.  
**Dépôt :** `/Volumes/Math17/aithos/v2/code/aithos-core`  
**Branche observée :** `feat/obligations` — ne pas la changer sans demande explicite.  
**HEAD observé :** `cda4f058708a5a43c5b21870bf0e1bce925d74e1`
(`feat(bundle): expose owner kex read capability`).  
**Aucun push demandé.**

Ce document complète les handoffs historiques. Il porte une ambition plus large :
fermer **tout le périmètre protocolaire nécessaire au produit**, pas seulement un lot
gateway ou une démonstration.

---

## 1. Mission opposable

Avant d'ajouter les mutations à `aithos-client`, puis de construire le SDK réseau,
`aithos-core` doit devenir une fondation protocolaire complète et cohérente :

1. chaque droit exprimable par un mandat est défini sans ambiguïté ;
2. chaque droit est sérialisable, vérifiable, atténuable et révocable ;
3. chaque droit est réellement exécutable par un owner ou un délégué autorisé ;
4. toute mutation produit une preuve gamma et une édition publiable ;
5. un provider sans clé peut accepter/refuser une publication à partir des seuls
   artefacts publics, sans jamais voir de donnée en clair ;
6. `public`, `circle`, `self`, gamma et les connecteurs sont couverts ;
7. les contraintes de mandat sont effectivement appliquées sur toutes les opérations
   auxquelles elles se rapportent, pas seulement parsées ;
8. les API Rust de référence sont suffisamment complètes pour que le client offline,
   le binding WASM et le futur SDK n'aient jamais à réimplémenter le protocole.

Une fonctionnalité présente uniquement dans la spec, un scénario `@wip`, un helper
de test ou une mutation locale impossible à publier **ne compte pas comme terminée**.

---

## 2. Décisions produit et modèle de confiance

Ces décisions sont acquises et ne doivent pas être réinterprétées :

- Le provider ne voit et ne stocke que des artefacts chiffrés ou publics. Il peut
  voir les certificats et preuves publiques nécessaires à la vérification keyless,
  mais jamais les clés privées, secrets locaux ou données client en clair.
- Le dashboard est hébergé par Aithos, mais vérification, autorisation,
  déchiffrement et mutations sensibles ont lieu localement dans le navigateur.
- L'owner agit grâce à sa capacité locale. Il n'a besoin d'aucun mandat.
- Un délégué n'agit qu'avec **sa clé privée ET une chaîne de mandats valide**. La
  possession cryptographique d'une clé de contenu ne suffit jamais à l'autoriser.
- Un mandat peut et doit permettre à un délégué de créer, modifier et supprimer
  lorsque son périmètre et ses contraintes le permettent.
- `aithos-client` reste un moteur Rust strictement offline : aucun HTTP, DNS, socket
  ou appel provider.
- Le futur SDK réseau appellera le provider et orchestrera `aithos-client`; il ne
  réimplémentera pas les règles de mandat ou le wire Aithos.
- Une application peut ouvrir plusieurs sessions isolées, pour plusieurs Ethos et
  plusieurs chaînes de mandats. Le client n'a pas à fusionner les mandats en une
  autorité globale.
- La première surface navigateur suppose le JavaScript livré honnête et le poste non
  compromis, mais aucune règle protocolaire fail-closed ne peut être reportée.

---

## 3. État du worktree à préserver

Au moment de ce handoff, le dépôt est volontairement sale et contient des travaux
existants appartenant à Mathieu :

- modifications suivies dans `rust/Cargo.lock`, `rust/Cargo.toml`,
  `vectors/README.md`;
- provider, Dockerfiles, workflows, vecteurs et nombreux documents non suivis ;
- répertoires de transfert et de récupération `_transfer/`, `_gitjunk/`,
  `_to_delete/`.

Ne rien nettoyer, déplacer, supprimer, restaurer ou incorporer par défaut. Ne pas
faire de `reset`, `checkout --`, `clean`, changement de branche ou commit global.
Avant chaque tranche :

1. relever `git status --short --branch`;
2. attribuer les changements existants à leur chantier ;
3. limiter strictement le staging aux fichiers de la tranche ;
4. signaler tout chevauchement avant d'éditer.

Le présent document et son prompt ont été ajoutés comme fichiers de handoff. Leur
commit éventuel reste une décision de la session d'exécution.

---

## 4. Sources de vérité à lire entièrement avant toute action

Dans cet ordre :

1. `README.md`
2. `spec/00-overview.md` à `spec/10-*.md`, sans limiter la lecture aux mandats
3. `docs/EXECUTION-PLAN.md`
4. `docs/HANDOFF-MANDATES-SURFACE-2026-07-15.md`
5. `docs/HANDOFF-MANDATES-M3-2026-07-16.md`
6. `docs/MANDATES-PRODUCT-GAPS.md`
7. `docs/GATEWAY-HANDOFF.md`
8. toutes les features racine relatives à mandates, délégation, révocation,
   contraintes, gamma, écritures et intégration
9. les features de `rust/crates/aithos-gateway/tests/features/`
10. les rituels dans `_transfer/claude-skills/`, notamment :
    - `bdd-ritual/SKILL.md`
    - `crypto-vectors-first/SKILL.md`
    - `pure-core/SKILL.md`

Inspecter ensuite le code réel de :

- `aithos-core`: mandate, constraints, revocation, gamma, headers, manifests ;
- `aithos-bundle`: grants, mutations, log, editions, merge, révocation ;
- `aithos-provider`: enveloppes, CAS, witness, store/tunnel ;
- `aithos-gateway`: hub, policy, core bridge, MCP, credentials ;
- `aithos-cli` et `aithos-wasm`: surfaces publiques.

Ne pas faire confiance aux compteurs ou statuts historiques des anciens handoffs :
relever les tests, tags `@wip`, API et écarts dans l'état réel du worktree.

---

## 5. État fonctionnel observé le 2026-07-18

### 5.1 Modèle de mandat prévu

La spec permet :

- zones : `public`, `circle`, `self`;
- verbes : `read`, `edit`, `append`, `delete`, `write`;
- sélecteurs : zone entière, `dir`, `tag`, `dir&tag`, `id`;
- connecteurs : `act.x.<connector>.<action|*>`;
- journal : `read.gamma` filtré ;
- structure d'autorité : `issue#depth=n`, `revoke[.<zone>#selector]`;
- contraintes temporelles, quantitatives, budgétaires, argumentaires,
  sessionnelles, d'approbation et de transparence.

### 5.2 Ce qui est réellement exécutable

- Lecture `public` keyless.
- Lecture owner sur `public`, `circle`, `self`.
- Lecture déléguée `circle` sur zone/dossier/tag/dossier+tag.
- Mutation déléguée locale de sections `circle` :
  - création avec `append`;
  - réécriture avec `edit` ou `append`;
  - suppression avec `delete` ou `write`;
  - preuve gamma signée par le grantee.
- Sous-délégation, journalisation du grant et révocation déléguée.
- Requêtes gamma déléguées, avec ouverture physique surtout câblée sur `circle`.
- Actions de connecteur via le gateway :
  - manifeste MCP découvert et approuvé ;
  - périmètre `act.x...` vérifié ;
  - bornes owner appliquées ;
  - gamma écrit avant l'effet externe ;
  - relay fail-closed.
- Obligations/co-signatures, fenêtres, budgets, compteurs, rate limits et heartbeat
  sur le chemin d'append des actions.

### 5.3 Écarts bloquants confirmés

1. **Publication déléguée normale impossible.** `Bundle::publish` demande les clés
   owner. `Bundle::verify` n'accepte une signature déléguée que pour une résolution
   de fork. Une mutation déléguée locale ne peut donc pas devenir une édition normale
   canonique récupérable ensuite depuis le provider.
2. **`id=` absent du modèle Rust Ethos.** Il existe pour gamma, pas pour
   `PerimeterEntry::Ethos` ni `Op`. Les scénarios dédiés sont `@wip`.
3. **Écritures déléguées limitées à `circle`.** Pas de parité `public`/`self`.
4. **Pas de mutations dans `aithos-client`.** La surface actuelle est lecture seule,
   ce qui est correct tant que le protocole ci-dessous n'est pas fermé.
5. **Pas de parité générale owner/délégué.** Inventorier toutes les mutations owner
   (sections, dossiers, moves, tags, rotations, vault, etc.) et décider lesquelles
   sont délégables. Toute exclusion doit être normative, pas accidentelle.
6. **Contraintes partiellement exécutées.** Plusieurs familles sont validées ou
   atténuées mais ne sont pas appliquées à toutes les opérations concernées.
7. **Les mutations de contenu ne passent pas par le même moteur de consommation que
   les actions.** Les compteurs, fenêtres actives et obligations ne sont pas
   uniformément appliqués aux écritures.
8. **Classes connecteurs incomplètes.** La spec dit `read/act/binding`; le gateway
   expose surtout `read/write`.
9. **Wildcard binding non fermé.** `covers_act` laisse actuellement `*` couvrir toute
   action; le manifeste gateway ne fournit pas encore la classe protocolaire qui
   permettrait d'exclure automatiquement `binding`.
10. **Vault trop grossier.** La spec cible `/x/<connector>` et
    `act.x.<connector>.config`; le bundle livre actuellement surtout une ligne sur la
    racine `/x`, utilisée pour l'audit des arguments, pas une gestion complète et
    isolée de la config par connecteur.
11. **Surface d'émission générique incomplète.** Le helper `Bundle::grant` construit
    surtout des entrées Ethos et `issue`. Les mandats combinant zones, connecteurs,
    gamma, revoke et contraintes sont souvent assemblés par des chemins spécialisés
    ou de test.
12. **Gamma délégué `self` incomplet.** Le query path indique explicitement que les
    corps `self` restent owner-side.

---

## 6. Contradictions à résoudre au gate, avant le code

Ces points changent le contrat signé. Ne pas les inventer silencieusement.

### D1 — Containment de `id=`

- La spec de délégation dit qu'un parent `dir=p` peut couvrir un enfant `id=` situé
  sous `p`.
- Le Gherkin M0 plus récent dit que le core pur ne peut pas résoudre cette relation :
  seuls la zone entière ou le même `id=` couvrent l'enfant.

**Décision existante la plus récente :** zone entière ou `id` identique uniquement.
La considérer opposable, sauf nouvelle validation explicite de Mathieu. Si elle est
confirmée, aligner la spec de délégation.

### D2 — `delete` implique-t-il `read` ?

La spec dit que toute mutation implique `read`. Le lattice Rust ne fait pas couvrir
`Read` par `Delete`.

**Recommandation :** aligner le code sur la spec (`delete` couvre `read`) à moins
qu'un droit explicite de suppression aveugle soit voulu et documenté.

### D3 — Forme d'une édition publiée par un délégué

Définir explicitement :

- qui signe le manifeste ;
- comment la chaîne `authorized_via` est embarquée ou référencée ;
- comment le verifier calcule le changeset entre les éditions ;
- comment chaque fichier/index/noeud modifié est relié à une opération autorisée ;
- comment gamma, fichiers, index, roots et manifest sont atomiques ;
- comment une édition déléguée concurrente devient fork/merge/résolution ;
- comment le provider effectue un CAS et un contrôle keyless ;
- quelle autorité peut publier des mutations couvrant plusieurs périmètres ou
  plusieurs mandats.

Le provider ne doit pas avoir besoin des clés de contenu pour décider si l'enveloppe
publique et ses preuves sont acceptables.

### D4 — Authorship de `public`

Le contenu public owner est actuellement signé par la clé de contenu owner. Un
délégué ne peut ni ne doit imiter cette signature.

Définir une authorship déléguée publique vérifiable : signature grantee +
`authorized_via` + preuve gamma/manifest, avec une présentation claire distinguant
contenu owner et contenu délégué.

### D5 — Vérification keyless des mutations `self`

La structure `self` est scellée. Pour une édition déléguée, définir comment le
provider/verifier keyless vérifie que le changement reste dans le périmètre :

- `id=` ou zone entière ;
- commitment public de changeset ;
- preuve signée liée au noeud sans révéler son nom ou contenu ;
- traitement des créations, dont le SID n'existait pas avant.

Ne pas résoudre ce point en exposant la structure `self`.

### D6 — Opérations structurelles

Décider si les droits `append/edit/delete/write` couvrent uniquement les sections ou
aussi :

- création/suppression/renommage/déplacement de dossiers ;
- changement de tags/titres/noms ;
- rotation de clés ;
- gestion des vues de tags ;
- configuration de connecteurs.

**Principe demandé :** obtenir une parité complète sur le périmètre produit. Toute
opération owner non délégable doit avoir une raison de sécurité normative explicite.

### D7 — Contraintes sur mutations

Définir quelles contraintes comptent :

- uniquement les actions connecteurs ;
- ou aussi chaque mutation Ethos ;
- et sous quel nom/action gamma elles sont comptabilisées.

La documentation d'`aithos-client` anticipe déjà que budgets et consommation restent
attachés aux actions/mutations journalisées. Il faut une règle unique dans le core.

### D8 — Classes et wildcard connecteurs

Confirmer la migration vers `read/act/binding` et la règle :

- `*` couvre `read` et `act`;
- `*` ne couvre jamais `binding`;
- `binding` doit être nommé et co-signé selon les obligations.

Définir où vit la classe signée afin que core, gateway et audit rendent le même
verdict.

### D9 — Vault par connecteur

Confirmer :

- un noeud et une ligne distincts `/x/<connector>`;
- `.config` donne read+write uniquement sur ce connecteur ;
- un simple `act.x.<connector>.<action>` ne livre jamais le credential ;
- le tool-host peut agir sans donner la ligne vault au grantee ;
- rotation et révocation d'un connector vault sont indépendantes.

---

## 7. Définition de « protocole fini »

Le chantier n'est clos que si la matrice suivante est verte par tests de
conformance et E2E réels.

### 7.1 Zones

Pour `public`, `circle`, `self`, vérifier selon les règles propres à chaque zone :

- owner : list/read/create/edit/delete ;
- grantee : list/read/create/edit/delete lorsque le mandat le couvre ;
- refus latéral avant usage de clé ;
- zone entière, `dir`, `tag`, `dir&tag`, `id` ;
- création future sous un périmètre autorisé ;
- tags/titres/noms et opérations structurelles selon D6 ;
- expiration, révocation et contraintes après ouverture de session ;
- données et métadonnées interdites absentes des refus ;
- édition publiée, rechargée depuis un autre store, puis revérifiée à froid.

### 7.2 Autorité

- root owner sans mandat ;
- grantee : clé + chaîne, preuve de possession ;
- chaîne multi-niveaux ;
- atténuation complète de chaque famille ;
- `issue`, `max_children`, profondeur ;
- révocation issuer/ancestor/watchdog, cascade et réadoption ;
- plusieurs mandats/keys sans confusion de sujet ou de session ;
- aucune extension silencieuse lors d'un nouveau connecteur ou droit.

### 7.3 Publication et provider

- owner et grantee produisent des éditions normales ;
- upload envelope strictement opaque ;
- provider keyless vérifie forme, signatures, chaîne, anti-replay et CAS ;
- tentative hors périmètre refusée avant commit ;
- gamma et manifest ne divergent jamais ;
- conflits concurrents déterministes ;
- merge/résolution vérifiés ;
- nouveau téléchargement froid reproduit exactement l'état accepté ;
- aucune clé/plaintext dans provider, logs, erreurs, URL ou headers.

### 7.4 Connecteurs

- action exacte et wildcard ;
- classes `read`, `act`, `binding`;
- binding nommé + approbation obligatoire ;
- bornes et `action_params` sur les vrais arguments ;
- manifeste approuvé et drift fail-closed ;
- log avant effet ;
- refus journalisé selon la politique ;
- vault isolé par connecteur ;
- config déléguée uniquement avec `.config`;
- rotation/révocation de credential ;
- action via tool-host sans remise de credential au grantee.

### 7.5 Contraintes

Pour chaque contrainte normative :

- forme valide/invalide ;
- atténuation parent/enfant ;
- héritage/conjonction ;
- application réelle au bon moment ;
- refus fail-closed ;
- compteur/audit vérifiable offline ;
- comportement identique entre core, bundle, gateway et future surface client.

Les familles incluent au minimum : validity window, `max_actions`,
`max_children`, `max_sessions`, `max_actions_per`, `rate_limit`,
`active_windows`, `budgets`, `log_reads`, `obligations`, `counter_sign`,
`binding`, `domains`, `action_params`, `disclose_agency`, `notify`, `purpose`,
`session_bind`, `heartbeat`, `freshness`, `spend_cap`, `first_party_only`.

---

## 8. Plan d'exécution recommandé

Les numéros sont séquentiels. Ne paralléliser que les travaux qui ne modifient pas le
même contrat signé.

### Lot 0 — Rebaseline et matrice de couverture

- Relever les statuts réels, tests, `@wip`, API publiques et changements sales.
- Construire une matrice `spec → feature → vector → core → bundle → provider →
  gateway → CLI/WASM`.
- Marquer chaque case : absent, partiel, complet, contradictoire.
- Rejouer les suites existantes sans modifier le code.
- Présenter les décisions D1–D9 à Mathieu.

**Gate :** validation explicite de Mathieu sur les décisions et la matrice. Aucun
changement de wire avant ce gate.

### Lot 1 — Contrats Gherkin de complétude

- Corriger les contradictions de spec validées au Lot 0.
- Écrire les features manquantes en `@wip` :
  - édition normale déléguée ;
  - publication provider CAS ;
  - parité des zones ;
  - opérations structurelles ;
  - contraintes sur mutations ;
  - classes connecteurs et vault isolé ;
  - rechargement froid après publication.
- Faire un commit de contrat isolé, sans implémentation.

**Gate :** Mathieu valide le nouveau contrat Gherkin. Un contrat substantiel non
validé bloque le lot suivant.

### Lot 2 — Vecteurs et wire

- Figer avec oracle indépendant les octets actuels sans `id`.
- Ajouter les vecteurs `id=`.
- Ajouter les vecteurs de manifeste/changeset/édition déléguée.
- Ajouter les vecteurs d'enveloppe provider et de conflits CAS.
- Ajouter les vecteurs de classes connecteurs/vault si le wire change.
- Obtenir des tests rouges avant implémentation.

**Gate :** l'oracle ne réutilise pas l'implémentation Rust testée.

### Lot 3 — Perimeter et autorisation pure complets

- Implémenter `id=` dans parser, sérialisation, `Op`, containment et atténuation.
- Corriger le lattice (`delete`) selon D2.
- Introduire une opération canonique couvrant contenu, structure, gamma, action et
  config.
- Produire un verdict pur unique : chaîne + périmètre + contraintes + classe
  d'action + temps + état de révocation.
- Garder `aithos-core` sans I/O, horloge ou RNG implicites.

**Gate :** tout consommateur utilise ce verdict, aucune logique protocolaire dupliquée.

### Lot 4 — Mutations déléguées complètes dans le bundle

- Étendre les mutations autorisées à toutes les zones et opérations validées.
- Délivrer les lignes exactes : zone, dossier, tag view, section.
- Gérer authorship owner/grantee.
- Appliquer les contraintes et compteurs aux mutations.
- Rendre chaque mutation atomique avec sa preuve gamma au niveau transaction logique.
- Tester les échecs avant modification durable.

**Gate :** une erreur d'autorisation, clé, contrainte ou gamma laisse le bundle
byte-for-byte dans son état précédent.

### Lot 5 — Éditions déléguées et conflits

- Concevoir/implémenter le manifeste délégué validé au Lot 0.
- Calculer et vérifier les changesets.
- Autoriser la publication uniquement si chaque changement est couvert.
- Généraliser merge/fork/résolution à plusieurs writers.
- Vérifier les éditions sans clés de contenu.

**Gate :** mutation déléguée → publication → copie dans store vierge → cold verify →
lecture owner/grantee, sans owner dans la boucle.

### Lot 6 — Provider opaque complet

- Définir l'enveloppe d'upload et le CAS.
- Valider signatures, chaîne, anti-replay, hauteur et parent.
- Refuser une édition non autorisée ou concurrente sans état partiel.
- Ne jamais demander de clé ni de plaintext.
- E2E sur un vrai transport local, sans mock du protocole.

**Gate :** sweep de non-fuite provider et reprise après conflit.

### Lot 7 — Contraintes intégrales

- Brancher chaque famille sur son point d'exécution.
- Harmoniser action connecteur, mutation, lecture présentée, session et publication.
- Compléter `action_params`; distinguer contraintes V et X sans trou silencieux.
- Vérifier compteurs de sous-arbre et attestations.

**Gate :** matrice par famille, parent/enfant et owner/grantee.

### Lot 8 — Connecteurs et vault

- Migrer vers `read/act/binding`.
- Fermer le wildcard binding.
- Isoler `/x/<connector>`.
- Implémenter `.config`, lecture/écriture/rotation.
- Garantir la custody tool-host.
- Unifier politique effective et preview.

**Gate :** appel réel de connecteur de test, log-before-effect, binding co-signé,
credential absent du grantee et du provider.

### Lot 9 — Surfaces de référence

- Exposer toute la capacité via API Rust stables.
- Compléter CLI et WASM minces sans logique protocolaire.
- Fournir à `aithos-client` des primitives d'orchestration, pas du réseau.
- Documenter les erreurs typées et read/write models stables.
- Ne pas commencer le SDK réseau dans ce dépôt.

**Gate :** aucun consommateur n'a besoin d'accéder à des structures internes ou de
réimplémenter un `covers`, un hash, une signature ou un changeset.

### Lot 10 — Gate de sortie protocole

- Specs cohérentes et sans mention « later pass » sur le périmètre retenu.
- Aucun `@wip` relatif au périmètre complet.
- Tous vecteurs indépendants verts.
- fmt, clippy `-D warnings`, tests workspace, WASM et E2E verts.
- Threat model et limites résiduelles documentés.
- Version de wire stabilisée/migrable.
- Handoff explicite vers `aithos-client`, puis vers le SDK.

---

## 9. Rituel de développement obligatoire

Pour chaque lot :

1. **Gherkin d'abord**, scénario `@wip`.
2. Validation humaine du contrat si nouveau choix produit.
3. Commit du contrat seul.
4. Vecteur/oracle indépendant avant le code lorsqu'un octet signé change.
5. Test rouge prouvant l'absence de la capacité.
6. TDD minimal dans le crate propriétaire de la règle.
7. Retrait progressif des `@wip`, un scénario cohérent à la fois.
8. E2E réel sans mock du protocole.
9. Suites complètes du workspace.
10. `cargo fmt --check`, clippy `-D warnings`, WASM check.
11. Commit étroit de la tranche.
12. État express et gate avant le lot suivant.

Ne jamais :

- copier la logique de protocole dans gateway, provider, WASM, client ou SDK ;
- accepter une mutation parce que la clé permet cryptographiquement de l'écrire ;
- faire confiance au provider ;
- signer comme owner un contenu produit par un grantee ;
- dégrader `self` en rendant sa structure publique ;
- adapter un test détaggé pour faire passer une implémentation sans décision ;
- pousser, merger, déployer ou utiliser des secrets réels sans demande explicite.

---

## 10. Commandes de gate à confirmer contre le workspace réel

À exécuter depuis `rust/`, avec target temporaire si nécessaire pour éviter les
artefacts concurrents :

```bash
CARGO_INCREMENTAL=0 cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo check -p aithos-wasm --target wasm32-unknown-unknown
```

Ajouter les suites Cucumber, E2E provider/gateway et scripts de vecteurs réellement
présents. Ne pas reprendre des compteurs historiques : afficher les nouveaux comptes
à chaque gate.

---

## 11. Conditions d'arrêt et escalade

S'arrêter au gate et demander Mathieu si :

- une décision D1–D9 n'est pas déjà explicitement validée ;
- deux sources de vérité se contredisent ;
- une solution exigerait que le provider voie une clé ou du clair ;
- une mutation ne peut être vérifiée keyless ;
- une capacité owner devrait rester interdite aux délégués sans règle normative ;
- un contrat Gherkin détaggé devrait changer ;
- un changement wire n'a pas de vecteur indépendant/migration ;
- le travail recouvre des modifications sales non attribuables ;
- le périmètre exige une action externe, un push ou un déploiement non demandé.

Ne pas déclarer le protocole fini en remplaçant une décision de sécurité par une
limitation documentaire. Une limitation n'est acceptable que si Mathieu la choisit
explicitement comme hors périmètre de la version.

---

## 12. Résultat attendu du prochain agent

La première session de reprise ne doit pas essayer de tout coder. Elle doit :

1. relire les sources ;
2. rebaseliner l'état réel ;
3. produire la matrice exhaustive ;
4. confirmer ou corriger les écarts ci-dessus ;
5. préparer les contrats Gherkin du premier lot ;
6. s'arrêter au gate des décisions D1–D9 si elles ne sont pas toutes opposables.

Après validation, elle pourra engager le développement lot par lot jusqu'au gate de
sortie complet.

# Handoff — Aithos Core, Lot 1 : contrats de complétude

**Date :** 2026-07-18

**Statut :** gate produit Lot 0 validé ; D1–D9 actées ; relation
`.config`/classe D8 explicitement non ratifiée

**Dépôt :** `/Volumes/Math17/aithos/v2/code/aithos-core`

**Branche observée :** `feat/obligations` — ne pas la changer

**HEAD observé au gate :**

`cda4f058708a5a43c5b21870bf0e1bce925d74e1`

**Prochaine tranche :** Lot 1 seulement — specs et Gherkin `@wip`

**Aucun push, merge ou déploiement demandé.**

Ce handoff complète
`docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md`. Il remplace uniquement :

- son gate ouvert sur D1–D9 ;
- son instruction de recommencer le Lot 0 ;
- son obligation de s'arrêter faute de décisions opposables.

Toute sa mission, ses sources obligatoires, sa séparation des responsabilités, sa
définition de « protocole fini », ses lots 2–10 et son rituel restent applicables.

---

## 0. Validation humaine et portée de l'accord

Après présentation de la matrice Lot 0, des contradictions et des recommandations,
Mathieu a répondu le 2026-07-18 :

> Ok top.
>
> On va suivre tes recommandations.
>
> Tu peux l'acter et me créer un handsoff et un prompt à lancer dans un autre
> contexte STP ?

Cette réponse ferme le gate produit D1–D9 et valide aussi les deux recommandations
transverses qui accompagnaient le gate :

1. `max_children` devient non supprimable en délégation ;
2. une contrainte racine inconnue reste tolérée sur une chaîne feuille, mais interdit
   toute sous-délégation tant que le core ne sait pas prouver son atténuation.

Les décisions ci-dessous sont donc opposables. Elles autorisent la rédaction du
contrat Gherkin. Elles **n'autorisent pas** à inventer directement un wire, modifier
des octets signés ou commencer l'implémentation.

Le seul sous-point découvert après ce gate qui n'était pas contenu dans les
recommandations acceptées est la relation entre la capacité vault `.config` et les
classes d'actions D8. D9 fixe déjà son exactitude et son exclusion du wildcard ; sa
classification éventuelle reste un gate ciblé, explicité ci-dessous.

---

## 1. Décisions D1–D9 actées

### D1 — Containment de `id=`

`id=<sid>` désigne exactement une section.

- `id=` ne se combine pas avec `dir=` ou `tag=` dans une même entrée.
- Le périmètre de zone entière couvre un enfant `id=`.
- `id=x` couvre uniquement `id=x`.
- `dir=` et `tag=` ne couvrent jamais un enfant `id=`, même si un appelant pourrait
  résoudre extérieurement la position de ce SID.
- Une opération Ethos section-précise fournit son SID au verdict pur du core.
- Cette règle vaut pour `public`, `circle` et `self`, sous réserve des règles de
  confidentialité propres à `self`.

La phrase contraire de `spec/05-delegation.md`, selon laquelle `dir=` couvre un
`id=` situé dessous, doit être redlinée au Lot 1.

### D2 — `delete` implique `read`

Toute mutation implique `read`. Il n'existe pas de suppression aveugle implicite.

- `edit` couvre `read`.
- `append` couvre `edit` et `read`.
- `delete` couvre `read`.
- `write` couvre `append`, `edit`, `delete` et `read`.

Les specs, Gherkin, vecteurs et implémentations doivent rendre ce lattice identique.

### D3 — Édition normale publiée par un délégué

La v1 retient une édition déléguée à **acteur unique et chaîne unique**.

- Une édition normale peut être signée par l'owner ou par le grantee feuille.
- L'owner utilise sa capacité locale et ne présente pas de mandat.
- Le grantee signe avec sa clé privée et présente une chaîne valide.
- Chaque changement de l'édition déléguée doit être couvert par cette même chaîne.
- Si plusieurs chaînes sont nécessaires, elles produisent des éditions séparées.
  Un format agrégé multi-chaînes exige un futur contrat et de nouveaux vecteurs.
- Le grantee ne signe jamais comme owner. L'owner n'est ni acteur ni signataire de
  l'édition déléguée ; il n'intervient que comme attestor si une obligation
  `co_sign`/approbation explicitement applicable l'exige.

L'enveloppe publique doit engager, sans en figer encore les noms de champs :

- l'édition de base et le parent attendu ;
- le manifeste, les roots et le changeset public typé ;
- le delta Gamma, ses preuves et le nouveau head ;
- la correspondance entre chaque changement, son opération et son autorisation ;
- les hashes des certificats et les certificats ou références content-addressées
  nécessaires à une vérification froide ;
- la signature de l'acteur.

Les objets chiffrés peuvent être transférés ou préchargés de façon opaque, mais une
unique opération CAS rend visibles atomiquement le nouveau manifeste et le nouveau
head Gamma. Un échec ne peut exposer un manifeste sans Gamma correspondant, ni
l'inverse. Des blobs content-addressed opaques, préchargés mais non référencés après
un conflit, peuvent rester orphelins et être collectés ; ils ne constituent jamais
une édition canonique ou partiellement joignable.

Le provider vérifie keyless : forme, acteur, preuve de possession, chaîne,
révocations, fenêtres, contraintes de tier V, receipts/attestations publiques exigées
pour le tier X, changeset, Gamma, roots, parent et CAS. Il ne prétend pas vérifier la
vérité d'une contrainte tier X qui nécessiterait le plaintext. Sans preuve publique
acceptable pour un tier X requis, il refuse fermé. Il n'obtient aucune clé de contenu
ni donnée en clair.

Une concurrence sur le même parent produit un conflit explicite et déterministe.
Fork, merge et résolution restent soumis à l'autorité couvrant tous leurs
changements ; une signature déléguée n'est plus réservée à la seule résolution.

### D4 — Authorship déléguée de `public`

- Un contenu public owner conserve son authorship owner.
- Un grantee ne produit ni n'imite une signature owner.
- Une mutation publique déléguée porte une authorship grantee signée et liée au
  hash du contenu, au SID, à l'opération, à l'édition et à `authorized_via`.
- Cette preuve est engagée par Gamma et par le manifeste.
- Une vérification froide distingue sans ambiguïté contenu owner et contenu
  délégué, sans clé privée.

La présentation produit peut afficher l'identité ou la clé de l'acteur et sa chaîne
d'autorisation ; elle ne doit jamais présenter un contenu délégué comme directement
signé par l'owner.

### D5 — Mutations `self` vérifiables keyless

Le provider ne découvre jamais la structure scellée de `self`.

- Les écritures `self` sont autorisables à la zone entière ou par `id=<sid>`.
- Un edit ou delete par `id=` porte sur le même SID opaque avant et après.
- Une création est autorisée par un `append`/`write` de zone entière, ou par un SID
  préalloué explicitement autorisé avant la création.
- La création prouve l'absence antérieure du SID ; edit et delete prouvent
  respectivement remplacement ou retrait.
- L'édition engage des commitments opaques avant/après, les roots d'index/header,
  l'opération, Gamma, la chaîne et le manifeste.
- Une affirmation signée sans preuve liée à l'état précédent ne suffit pas.

Ces preuves peuvent exposer un SID et des hashes opaques. Elles ne peuvent exposer
nom, chemin, titre, tags, contenu, relations de dossiers ou clé.

### D6 — Opérations structurelles

Les verbes s'appliquent aux sections et aux dossiers :

- `read` : lister et lire ce que le périmètre permet de présenter ;
- `edit` : modifier un objet existant, dont corps, titre, nom ou tags ;
- `append` : créer ou modifier, sans supprimer ;
- `delete` : supprimer un objet existant, avec lecture implicite ;
- `write` : CRUD complet.

Règles composées :

- un renommage sans changement de parent est un edit ;
- un déplacement exige autorité de modification sur le nœud source et
  `append`/`write` sur la destination ;
- la suppression d'un dossier exige la couverture du dossier et de tout le
  sous-arbre affecté ;
- vues de tags, index et réindexations sont des conséquences atomiques et
  déterministes de l'opération, jamais des mutations silencieuses ;
- tout rewrap ou rotation rendu nécessaire par un move, une révocation ou un
  changement de frontière cryptographique appartient à la transaction logique.

Les rotations de racine de confiance, de succession ou de récupération restent
owner-only. `issue`, `revoke` et la configuration de connecteurs utilisent leurs
droits dédiés ; `write.<zone>` ne les couvre jamais implicitement.

### D7 — Contraintes sur mutations

Une mutation Ethos déléguée est une consommation protocolaire.

- Un type d'opération canonique et pur, ci-après `ConsumptionOp`, décrit les
  mutations, actions et autres consommations autorisées.
- Le même verdict core est appelé avant tout effet et lors du rejeu froid.
- Temps, état Gamma, révocations, session, receipts et compteurs sont injectés.
- Aucune surface ne recalcule localement le périmètre ou les contraintes.
- Une contrainte applicable qui ne peut pas être évaluée échoue fermé ; aucune
  famille connue ne peut rester un simple parseur silencieux.
- Les mutations owner sont journalisées, mais ne consomment aucun mandat.

Pour préserver la sémantique existante :

- `max_actions` et ses compteurs dérivés restent réservés aux actions connecteurs ;
- un compteur et une limite explicites sont ajoutés pour les mutations Ethos ;
- un compteur et une limite explicites sont ajoutés pour le total des consommations
  déléguées ;
- leurs noms wire, encodages et règles exactes de migration sont figés seulement
  après Gherkin validé puis vecteur indépendant au Lot 2.

Le Lot 1 doit fournir une matrice normative par contrainte et type d'opération :
fenêtres, freshness, heartbeat, sessions, first-party, purpose, obligations,
budgets, spend, rate limits, paramètres, transparence et notifications. Cette
matrice doit distinguer « applicable », « non applicable par définition » et
« preuve requise » ; elle ne peut contenir de comportement implicite.

### D8 — Classes connecteurs et wildcard

Chaque action de connecteur possède exactement une classe canonique :
`read`, `act` ou `binding`.

- La classe provient d'un manifeste de connecteur signé, versionné,
  content-addressé et explicitement approuvé par l'owner ; le signataire du
  manifeste et la preuve d'approbation ne sont pas confondus.
- Le digest/version et la preuve publique minimale nécessaires au verdict sont
  pincés par l'autorisation/chaîne puis prouvés par l'édition, et sont vérifiables
  keyless.
- Une classe ne peut pas être déduite localement par le gateway.
- `act.x.<connector>.*` couvre les actions `read` et `act` du manifeste engagé.
- Le wildcard ne couvre jamais `binding`.
- Une action `binding` doit être nommée exactement et présenter le reçu owner réservé
  `co_sign`. Les autres obligations applicables se conjoignent et ne le remplacent
  pas.
- Un changement de classe ou de catalogue produit une nouvelle version approuvée ;
  aucun drift runtime n'est accepté.
- Une action ajoutée ou reclassée après l'émission d'un mandat reste refusée sans
  nouvelle autorisation pinçant le nouveau catalogue.
- L'atténuation et l'exécution utilisent la même classe prouvée et le même verdict
  pur du core.

Migration :

- le legacy `read` peut devenir `read` ;
- un legacy `write` peut être mappé transitoirement vers `act` dans une version de
  migration explicite ;
- aucun legacy `write` ne vaut jamais preuve de `binding` ;
- les connecteurs sont réenrôlés pour le contrat canonique.

### D9 — Vault isolé par connecteur

Le chemin logique est `/x/<connector>` ; sa forme bundle canonique est
`x/<connector>`.

- Chaque connecteur possède DK, header, lignes, versions et rotation indépendants.
- Aucun grant générique de la racine `/x` n'est livré par défaut.
- `act.x.<connector>.config` est le droit réservé exact sur le vault de ce
  connecteur ; aucun wildcard ne le couvre.
- `.config` autorise le CRUD de la configuration de ce connecteur seulement.
- Le Lot 1 ne lui attribue pas automatiquement une classe D8 ni `co_sign`. Si sa
  relation au catalogue `read|act|binding` doit être normée pour écrire un scénario,
  s'arrêter et obtenir une validation explicite de Mathieu.
- `.config` suit les contraintes et obligations effectivement présentes dans son
  mandat.
- Un accès réussi exige simultanément la chaîne couvrant `.config` et une ligne
  valide sur `/x/<connector>`. La chaîne sans ligne ne déchiffre rien ; la ligne
  sans autorisation n'autorise rien.
- Un droit `act.x.<connector>.<action>` ordinaire n'ouvre jamais la config et ne
  livre aucune ligne vault au grantee.
- La capacité d'audit des arguments est cryptographiquement distincte de la capacité
  d'ouvrir la config. La topologie exacte des clés/sous-nœuds reste au Lot 2 après
  contrat et vecteur ; le Lot 1 ne l'invente pas.
- Un tool-host peut détenir sa propre ligne, résoudre le credential au dernier
  moment et agir après verdict + log-before-effect, sans transmettre le secret au
  grantee.
- Un secret manager externe peut servir de backend de custody ; il ne devient
  jamais la source d'autorité protocolaire.
- Rotation, révocation, recipients et epochs sont indépendants par connecteur.
- Une mutation `.config` reste atomique avec Gamma, publiable et vérifiable à froid.

Provider, logs, erreurs et preuves publiques ne contiennent jamais le credential, la
config privée, une clé privée ou un DK en clair. Les headers peuvent contenir les
lignes/wraps chiffrés normatifs, qui restent inutilisables par le provider.

---

## 2. Décisions transverses actées

### T1 — `max_children` non supprimable

`max_children` signifie le nombre maximal d'enfants directs d'un mandat.

- Il n'est pas chain-conjoined.
- Il sort de la liste des contraintes supprimables.
- Si le parent le porte, l'enfant doit le répéter avec une valeur inférieure ou
  égale ; son absence élargit le droit et invalide le lien.
- Chaque grant est journalisé avant utilisation du mandat enfant.
- Le rejeu froid recompte les enfants du mandat minting exact.
- Un éventuel plafond de descendants de tout le sous-arbre sera une autre
  contrainte, avec un autre contrat.

Le commentaire Rust et le vecteur E+ affirmant que chaque ancêtre continuerait à
lier le sous-arbre sont contradictoires avec l'exécution réelle. Ils devront être
redlinés/régénérés selon le rituel vectors-first, pas modifiés au Lot 1.

### T2 — Contraintes racines connues et inconnues

- `constraints` est toujours un objet JSON.
- Toute clé connue est validée selon sa forme typée, y compris sur le mandat racine.
- Une clé inconnue sur un mandat racine feuille est préservée et tolérée
  conformément à la règle de forward compatibility de la spec 04.
- Une chaîne portant une telle clé ne peut pas être sous-déléguée tant que le core
  ne connaît pas sa loi d'atténuation.
- Toute clé inconnue sur un lien parent/enfant échoue fermé.
- Toute contrainte connue mais mal formée échoue sur racine comme sur lien.
- Le core doit distinguer validation structurelle de racine et
  validation/atténuation d'un lien.

La tolérance d'une extension inconnue sur une chaîne feuille ne permet pas aux
surfaces de prétendre qu'elles l'ont exécutée. L'extension doit rester visible dans
le verdict/audit comme non comprise.

### T3 — Corrections de conformance sans nouveau choix produit

Le Lot 0 a également confirmé les défauts suivants, à contractualiser sans leur
inventer de sémantique divergente :

- `issue#depth=0` est invalide ;
- les sélecteurs Ethos dupliqués sont invalides ;
- version, algorithme de signature, clé annoncée, ids, nonce et timestamps d'un
  mandat doivent être validés avant confiance ;
- révocation, constraints consumption et preuve de possession font partie du
  verdict froid, pas d'une composition facultative par l'appelant.

---

## 3. Résultat du Lot 0 — ne pas refaire

### 3.1 Baseline

Les gates suivants ont été rejoués sans modifier le code. Les suites ouvrant des
sockets loopback ont été confirmées dans l'exécution autorisée hors restrictions de
sandbox :

- `cargo test --workspace --locked` : vert ;
- `cargo fmt --all --check` : vert ;
- `cargo clippy --workspace --all-targets --locked -- -D warnings` : vert ;
- check `aithos-wasm` : vert ;
- vecteurs provider vérifiés par l'oracle Python indépendant : P1 `10`, P2
  `5 + 5 gamma`, P3 `6`, P4 checkpoints/root/equivocation, P5 `8` ;
- `aithos-core` : `78/78` ;
- CLI : `17/17` ;
- gateway : `82` unitaires, `152` scénarios Cucumber / `790` étapes et E2E réels ;
- provider : store `34`, relay `18`, tunnel `12`.

Un recomptage Cucumber bundle redondant a ensuite été empêché avant exécution par
la saturation de `/tmp`. Ce n'était pas un échec du protocole. Le prochain agent
doit néanmoins afficher ses propres compteurs au gate et utiliser un target isolé.

Un autre rejeu relay dans la sandbox restreinte a échoué au setup avec
`Operation not permitted` sur les `18` scénarios. Ce résultat environnemental ne
remplace pas le run hors sandbox vert et ne doit pas être présenté comme une
régression protocolaire ; tout nouveau gate réseau doit toutefois distinguer
explicitement setup sandbox et assertions métier.

### 3.2 Dette Gherkin

Inventaire observé :

- features core/bundle : `219` déclarations, dont `9 @wip` ;
- features gateway : `163` déclarations, dont `15 @wip` ;
- features provider : `65` déclarations, dont `1 @wip` ;
- dette protocolaire core + gateway : `24 @wip`.

Un scénario `@wip` n'est ni exécuté ni complet.

### 3.3 Matrice de reprise

Légende : `A` absent, `P` partiel, `C` complet sur le périmètre actuel,
`X` contradictoire.

| Capacité | Spec | Gherkin | Vector | Core | Bundle | Provider | Gateway | CLI/WASM | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Owner par capacité locale | C | C | P | C | C | A | P | P | P |
| Forme/signature d'un mandat | C | P | P | P | P | P | P | P | P |
| Zones `public/circle/self` | C | C | P | C | C | P | P | P | P |
| Scope `dir/tag/dir&tag` | C | C | P | C | C | P | P | P | P |
| Scope Ethos `id=` | X | P/@wip | A | A | A | A | A | A | X |
| Lattice des verbes | C | P | A | X | P | A | P | P | X |
| Sous-délégation `issue/depth` | C | P | P | P | C | P | P | P | P |
| `max_children` | C | P | X | X | P | A | P | P | X |
| Atténuation des contraintes | C | P/X | P | P | P | P | P | P | P |
| Révocation et cascade | C | C | P | P | P | P | P | P | P |
| Lecture déléguée `circle` | C | C | P | C | C | P | C | P | P |
| Lecture déléguée `public/self` | C | P | A | P | P | P | A/P | P | P |
| Mutations déléguées `circle` | C | C | A | P | P | A | P | P | P |
| Mutations déléguées `public/self` | C | P | A | P | A | A | A | A | A |
| Opérations structurelles | P | P | A | A | P | A | A | A | P |
| Gamma local et proofs | C | C | C | C | C | P | C | P | P |
| Consommation actions | C | C | P | P | C | A | P | P | P |
| Consommation mutations | P | P | A | A | A | A | A | A | A |
| Authorship public déléguée | P | A | A | A | A | A | A | A | A |
| Édition normale déléguée | P | A | A | P | X | A | A | A | X |
| Atomicité mutation + Gamma | C | P | A | P | A | A | A | A | A |
| Changeset keyless `self` | P | A | A | A | A | A | A | A | A |
| CAS/keyless provider | C | P | P | P | P | A/P | A | A | P |
| Cold roundtrip provider | C | A | A | P | P | A | A | A | A |
| Classes `read/act/binding` | C | P/@wip | A | A | P | A | X | A | X |
| Wildcard hors binding | C | P/@wip | A | X | P | A | X | A | X |
| Vault `/x/<connector>` + `.config` | C/DRAFT | P | A | P | X | P | X | A | X |
| CLI mince sans règle | C | P | P | C | P | A | P | P | P |
| WASM de référence | C | P | P | C | P | A | P | A | A |

Cette matrice n'est pas une déclaration de complétude. Le Lot 1 ne changera que la
colonne Gherkin vers « contractuel `@wip` » et les contradictions de spec validées.

### 3.4 Preuves code critiques

- `id=` manque au modèle Ethos :
  `rust/crates/aithos-core/src/mandate.rs:66`.
- Le lattice ne fait pas couvrir `Read` par `Delete` :
  `rust/crates/aithos-core/src/mandate.rs:53`.
- La validation racine saute la validation typée des contraintes :
  `rust/crates/aithos-core/src/mandate.rs:700`.
- `max_children` est déclaré supprimable alors que le check ne reçoit que le
  mandat minting :
  `rust/crates/aithos-core/src/constraints.rs:812` et
  `rust/crates/aithos-core/src/gamma.rs:710`.
- Le moteur de consommation complet vise les actions :
  `rust/crates/aithos-core/src/gamma.rs:635`.
- Le rejeu froid Gamma ne rejoue pas toutes les consommations :
  `rust/crates/aithos-bundle/src/log.rs:616`.
- Le `Store` n'expose que `get/put/list`, sans transaction, et les chemins de
  mutation écrivent l'état avant leur append Gamma :
  `rust/crates/aithos-bundle/src/lib.rs:21`,
  `rust/crates/aithos-bundle/src/bundle.rs:462`,
  `rust/crates/aithos-bundle/src/bundle.rs:585`,
  `rust/crates/aithos-bundle/src/grants.rs:626` et
  `rust/crates/aithos-bundle/src/log.rs:522`.
- Les helpers de mutation owner edit/delete sont limités à `circle` :
  `rust/crates/aithos-bundle/src/bundle.rs:566` et
  `rust/crates/aithos-bundle/src/bundle.rs:618`.
- `Bundle::publish` prend les clés owner et `verify` refuse une édition déléguée
  normale :
  `rust/crates/aithos-bundle/src/bundle.rs:1155` et
  `rust/crates/aithos-bundle/src/bundle.rs:1179`.
- Le vault actuel ouvre une racine commune `e/x/header.json` :
  `rust/crates/aithos-bundle/src/bundle.rs:337`.
- Le provider P1 renvoie encore `501 not_implemented` pour manifest/certs/Gamma :
  `rust/crates/aithos-provider/src/service.rs:258`.
- Le gateway ne porte que `Read|Write` :
  `rust/crates/aithos-gateway/src/config.rs:71`.
- `covers_act` n'a pas de classe binding :
  `rust/crates/aithos-core/src/mandate.rs:448`.
- WASM n'expose actuellement que la surface genesis :
  `rust/crates/aithos-wasm/src/lib.rs`.

---

## 4. Worktree et ownership à préserver

État observé avant la création de ce handoff :

```text
## feat/obligations
 M rust/Cargo.lock
 M rust/Cargo.toml
 M vectors/README.md
?? .github/workflows/provider-image.yml
?? docker/relay.Dockerfile
?? docker/store-api.Dockerfile
?? docs/... (plusieurs travaux et handoffs)
?? rust/crates/aithos-provider/
?? vectors/gen-p.py
?? vectors/p1-store-envelope.json ... p5-tunnel-sni.json
?? vectors/verify-p.py
?? _gitjunk/
?? _to_delete/
?? _transfer/
```

Tous ces changements appartiennent à Mathieu. Ne rien nettoyer, restaurer, déplacer,
ajouter au stage ou incorporer implicitement.

Point de chevauchement déjà certain :
`rust/crates/aithos-provider/**` est entièrement non suivi et appartient à la piste
provider P. L'ajout de
`rust/crates/aithos-provider/tests/features/store/store-publication.feature` ou
`store-cold-roundtrip.feature` exige une autorisation explicite de Mathieu sur ces
fichiers précis. Sans elle :

1. s'arrêter avant toute écriture du Lot 1 ;
2. ne pas déplacer artificiellement le contrat provider dans une feature bundle ;
3. présenter les fichiers proposés et demander l'attribution de cette tranche.

Le contrat exhaustif Lot 1 est destiné à un gate et un commit uniques. Si Mathieu
choisit explicitement de scinder L1a hors provider et L1b provider, chaque sous-lot
doit avoir son propre gate et son propre commit, et L1a ne peut pas être déclaré
« Lot 1 complet ».

Si une spec ou feature racine visée est devenue modifiée depuis ce relevé, s'arrêter
avant édition et demander une décision.

---

## 5. Mission stricte du Lot 1

Le Lot 1 produit le contrat, pas l'implémentation.

### Autorisé

- redlines minimales des specs contradictoires selon D1–D9/T1–T3 ;
- nouveaux scénarios Gherkin, tous marqués `@wip` ;
- nouveaux fichiers `.feature` nécessaires ;
- commentaires de Feature référençant les décisions correspondantes ;
- projection Gherkin dans les features gateway, sans changement de runtime ;
- mise à jour de la matrice dans le rapport de revue puis dans le handoff DONE,
  hors commit de contrat.

### Interdit

- code Rust, step Cucumber ou helper ;
- vecteur, oracle ou fixture cryptographique ;
- champ JSON, algorithme, signature, hash, version ou migration wire ;
- changement Cargo, CLI, WASM, gateway runtime, provider runtime ou client ;
- retrait d'un `@wip` ;
- retag ou affaiblissement d'un scénario vert ;
- commit avant validation humaine du diff ;
- push, merge, déploiement, switch, reset, clean ou restore.

### Répartition contractuelle recommandée

- `features/d-bundle.feature` :
  parité owner create/edit/delete/write sur `public/circle/self`, publication,
  Gamma et rollback strictement atomique.
- `features/e-mandates.feature` :
  D2 et T3 sur le lattice et la forme canonique des mandats.
- `features/e-mandate-sections.feature` :
  D1, formes invalides, containment et parité `id=`.
- `features/f-plus-constraints.feature` :
  D7, T1, T2, consommation des mutations et rejeu.
- `features/g-revocation.feature` :
  scénarios additifs `@wip` d'atomicité
  revoke → rotation → rewrap/ré-encryption → Gamma, sans retagger les scénarios
  verts existants.
- `features/l-delegated-writes.feature` :
  parité create/edit/delete/write `public/circle/self`, D4, D5, atomicité locale.
- nouveau `features/m-delegated-editions.feature` :
  D3, édition normale à chaîne unique, changeset et vérification offline depuis un
  store local vierge reconstruit à partir d'un ensemble d'artefacts.
- nouveau `features/n-structural-mutations.feature` :
  D6, dossiers, rename, move, tags, sous-arbre, rewrap et refus atomiques.
- nouveau `features/o-connector-classes-vault.feature` :
  D8/D9, classes, wildcard, binding, `.config`, custody et isolation.
- `features/i-concurrency.feature` :
  fork/merge/résolution après un conflit, sans simuler le CAS provider.
- `rust/crates/aithos-gateway/tests/features/gateway-mandates.feature` :
  projection `read/act/binding`, wildcard hors binding, `.config` exact et absence
  de ligne vault pour un simple droit d'action ; scénarios `@wip` seulement.
- après attribution explicite de l'arbre provider :
  - `rust/crates/aithos-provider/tests/features/store/store-publication.feature`
    possède expected-head/CAS, un seul gagnant et les refus keyless ;
  - `rust/crates/aithos-provider/tests/features/store/store-cold-roundtrip.feature`
    possède vrai binaire service + HTTP + backend durable + restart + nouveau
    process/store client vide.

`features/k-integration.feature` reste un E2E offline. Un clone de `MemStore` ne doit
jamais être renommé « aller-retour provider ».

### Propriétés observables obligatoires

Le Gherkin doit verrouiller :

- clé grantee + chaîne valide, jamais clé seule ;
- owner absent comme acteur/signataire de l'édition déléguée, sauf receipt ou
  co-signature explicitement exigé par une obligation ;
- parité `public/circle/self` sous leurs règles de confidentialité ;
- état, blobs, index, headers, wraps et Gamma atomiques ;
- bundle local byte-for-byte inchangé après tout refus ou panne injectée ; une
  éventuelle télémétrie de refus vit hors bundle et hors preuve Gamma ;
- provider : tuple canonique visible `(manifest head, gamma head, height, parent)`
  inchangé après refus/conflit, aucun artefact partiel joignable comme édition ;
  seuls des blobs opaques non référencés et collectables peuvent subsister ;
- édition grantee normale à chaîne unique et sans modification parasite ;
- signature/authorship owner et grantee distinctes ;
- changeset keyless sans structure `self` ;
- contraintes tier V et révocations rejouées à froid ; receipts/attestations
  publiques du tier X vérifiées sans prétendre connaître le plaintext ;
- wildcard refusé pour binding ;
- `.config` exact et isolé ;
- credential absent de tout grantee qui ne porte qu'un droit d'action, et toujours
  absent du provider ; un grantee `.config` ne l'ouvre qu'avec sa ligne exacte ;
- CAS concurrent avec un seul gagnant et aucun état partiel ;
- enveloppe de transport et autorisation de publication vérifiées indépendamment :
  requête valide + publication invalide est refusée, et inversement ;
- vrai binaire provider, HTTP, backend durable conservé à travers arrêt/restart,
  nouveau process client, store local vierge et cold verify, sans état injecté par
  le harnais.

Le contrat décrit des propriétés et des verdicts. Les noms de champs et octets
signés restent au Lot 2.

---

## 6. Rituel et gates du Lot 1

1. Lire intégralement les sources obligatoires du handoff principal.
2. Relever `git status --short --branch --untracked-files=all`.
3. Comparer chaque cible à l'état Lot 0 ; s'arrêter sur chevauchement.
4. Écrire uniquement specs et scénarios `@wip`.
5. Vérifier qu'aucun scénario vert n'a été affaibli.
6. Afficher :
   - redlines spec avant/après et décision source ;
   - liste exacte des fichiers ;
   - inventaire scénarios et `@wip` avant/après ;
   - matrice Gherkin mise à jour dans le rapport, sans l'ajouter au commit contrat ;
   - diff complet de contrat.
7. Présenter ce diff à Mathieu puis **STOP**.
8. Après validation explicite seulement :
   - rejouer syntaxe/features vertes et baseline pertinente ;
   - stage nominatif des seuls fichiers validés ;
   - inspecter le diff staged ;
   - commit de contrat isolé ;
   - produire un handoff `...-DONE-<date>.md`.
9. Ne pas commencer le Lot 2 dans la même tranche.

Message de commit suggéré après validation :

```text
test(protocol): add D1-D9 completeness contracts @wip
```

### Gate humain pré-commit

Le Lot 1 ne passe que si Mathieu confirme que :

- chaque phrase est testable ;
- D1–D9/T1–T3 sont transcrites sans changement ;
- aucune donnée `self`, clé ou plaintext n'est requise du provider ;
- tout refus laisse l'état métier/canonique inchangé selon les invariants distincts
  bundle/provider ci-dessus ;
- le cold roundtrip utilise un vrai binaire, HTTP et un backend durable de test
  conservé après restart ;
- aucun détail wire prématuré n'a été inventé ;
- le diff contient uniquement les fichiers de contrat autorisés.

---

## 7. Après le Lot 1

L'ordre reste :

1. Lot 2 : oracle et vecteurs indépendants, puis tests rouges ;
2. core pur : formes, `id=`, lattice, opération/verdict canonique, contraintes ;
3. bundle : mutations complètes et atomicité ;
4. éditions déléguées, changesets, conflits ;
5. provider opaque, CAS et cold roundtrip ;
6. connecteurs/vault puis gateway mince ;
7. CLI et WASM de référence ;
8. gate protocole complet ;
9. seulement ensuite, mutations dans `aithos-client`, toujours offline ;
10. enfin le SDK réseau hors de ce dépôt.

Le harnais réseau des E2E provider n'est pas un SDK et ne doit pas devenir une
bibliothèque cliente réutilisable dans ce dépôt.

Les Lots 1–10 ne modifient pas `aithos-client`. Le Lot 9 stabilise uniquement les API
core/bundle destinées à son futur moteur de mutations. Les mutations client ne
commencent qu'après le gate protocole du Lot 10.

---

## 8. Conditions d'arrêt

S'arrêter et demander Mathieu si :

- une cible est déjà modifiée ou non suivie par un autre chantier ;
- l'autorisation d'écrire les features provider manque ;
- deux décisions actées se révèlent contradictoires entre elles ;
- un scénario n'est testable qu'en inventant une preuve/wire ou en donnant une règle
  à un crate qui n'en est pas propriétaire ;
- un scénario vert devrait changer ;
- une décision exigerait un nouveau détail wire non validé ;
- une preuve keyless exigerait une clé, du plaintext ou la structure `self` ;
- un test prétend faire un roundtrip provider avec un mock ou un clone mémoire ;
- le Lot 1 nécessiterait un step, du Rust, un vecteur ou un changement Cargo ;
- un commit inclurait un fichier étranger à la tranche.

Le prochain contexte commence au Lot 1. Il ne refait pas le Lot 0 et ne rouvre pas
D1–D9.

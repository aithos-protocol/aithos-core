# Audit d'implémentation — `a-identity.feature`

## Métadonnées

| Champ | Valeur |
|---|---|
| Feature auditée | `features/a-identity.feature` |
| Date | 2026-07-29 |
| Révision Git observée | `2fee855` |
| État observé | Worktree sale ; les sources Identity ciblées ne portent pas de modification suivie, mais cette note décrit l'état disque et non une baseline Git propre |
| Runner principal | `aithos-bundle --test cucumber` |
| Implémentation principale | `aithos-core::{keys,did,derive,wire}` |
| Surfaces contrôlées | Core, Bundle, CLI, WASM, Gateway et Provider lorsque l'exigence Identity les concerne |
| Statut de la note | **OUVERTE — corrections requises** |

## Verdict

Les neuf scénarios sont sélectionnés et exécutent du vrai code Rust de
production. Aucun step de cette feature n'est vide, `@wip`, mocké ou remplacé
par un verdict `OnceLock`.

La feature ne constitue toutefois pas encore une preuve complète de son propre
contrat :

- 6 scénarios sont `PROUVÉ` au niveau précis qu'ils exercent ;
- 2 scénarios sont `PARTIEL` ;
- 1 scénario est un `FAUX POSITIF` au regard du résultat annoncé ;
- trois écarts d'implémentation affectent le fail-closed DID, la transition
  d'époque et l'indépendance/custody de la succession.

## Preuves rejouées

### Gherkin

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

### Tests de conformité

```text
cargo test -p aithos-core --test a1_genesis --test a2_did

a1_genesis: 4 passed
a2_did:     3 passed
```

Les vecteurs A1/A2 figent les dérivations, encodages et JCS positifs contre des
valeurs générées indépendamment.

### Sondes négatives hors dépôt

Une sonde temporaire utilisant uniquement les API publiques actuelles a
confirmé les acceptations suivantes :

```text
signed malformed non-root keys accepted: true
signed wrong version/alg/fragment accepted: true
unknown unsigned wire field ignored and accepted: true
transition to malformed DID accepted: true
transition to same DID accepted: true
```

La sonde et ses artefacts temporaires ne font pas partie du dépôt.

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

| # | Scénario | Statut | Observation |
|---:|---|---|---|
| 1 | Same seed → same identity | `PROUVÉ` | Deux appels réels à `OwnerKeys::genesis`; A1 fixe aussi les trois sorties publiques. |
| 2 | Different seeds → no shared public key | `PROUVÉ` | Deux seeds distincts alimentent réellement la dérivation ; la propriété cryptographique générale reste fondée sur BLAKE3, pas sur l'exhaustivité de deux fixtures. |
| 3 | Three keys pairwise distinct | `PROUVÉ` | Les trois clés réelles sont comparées et proviennent de trois contextes de dérivation distincts. |
| 4 | Seed exactly 32 bytes | `PROUVÉ` | `MasterSeed::from_slice` impose `[u8; 32]`; le test A1 couvre 31 et 33 octets. |
| 5 | Succession independent and cold | `PARTIEL` | Le Core reçoit une entropie séparée, mais le step la choisit lui-même ; la garde froide n'est pas exercée et certaines surfaces dérivent ou co-stockent la succession. |
| 6 | DID lists four public keys | `PROUVÉ` | `DidDocument::build` est appelé ; A2 fixe le document JCS positif byte-exact. |
| 7 | Tampered DID fails closed | `PARTIEL` | Une modification signée par l'ancienne signature est rejetée, mais les autres clés et la forme wire ne sont pas validées strictement. |
| 8 | Succession-signed epoch transition accepts successor | `FAUX POSITIF` | Le document successeur n'est jamais fourni au vérificateur ; seul son texte `id` est copié dans la transition. |
| 9 | Anything else, including root, is rejected | `PROUVÉ` | Le Gherkin couvre `#root`; A2 couvre aussi une signature root prétendant être `#succession`. |

## Écarts et implémentations requises

### AID-001 — Vérification DID stricte et fermée

**Priorité : P1 — OUVERT**

#### Constat

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

#### Implémentation attendue

- [ ] Fermer le schéma de `DidDocument`, `DidKeys` et `SignatureBlock`.
- [ ] Valider explicitement la version et les métadonnées de signature.
- [ ] Décoder et typer les quatre clés avec le codec attendu.
- [ ] Conserver la liaison `id ↔ root` et la vérification Ed25519 existantes.
- [ ] Faire remonter une erreur `InvalidDidDocument` précise pour chaque
  famille de défaut.
- [ ] Vérifier que les surfaces Bundle, WASM, Catalog, Gateway et Provider
  consomment toutes ce verdict strict sans parser permissif parallèle.

#### Tests RED requis

- [ ] Document correctement signé dont `content` est malformée.
- [ ] Document correctement signé dont `kex` utilise le mauvais codec.
- [ ] Document correctement signé dont `succession` est malformée.
- [ ] Mauvaise version, mauvais algorithme et mauvais fragment, chacun
  correctement re-signé afin d'isoler le contrôle sémantique.
- [ ] Champ top-level, champ `keys` et champ `signature` inconnus ajoutés au
  JSON wire.
- [ ] Rejeu des mêmes cas via `Bundle::open` et la surface WASM publique.

#### Critère de clôture

Tous les cas négatifs sont refusés par le même verdict Core ; A2 positif reste
byte-identique ; aucune surface ne réinterprète ou ne supprime silencieusement
un champ avant vérification.

### AID-002 — Lier la transition au document successeur réel

**Priorité : P1 — OUVERT**

#### Constat

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

#### Implémentation attendue

- [ ] Introduire une API non ambiguë qui reçoit `prev_doc`, `next_doc` et la
  transition.
- [ ] Vérifier les deux documents DID avec le validateur strict AID-001.
- [ ] Exiger `prev_did == prev_doc.id`.
- [ ] Exiger `next_did == next_doc.id`.
- [ ] Exiger `prev_doc.id != next_doc.id`.
- [ ] Valider version, algorithme et fragment de la transition.
- [ ] Vérifier la signature sous la succession du document précédent.
- [ ] Éviter qu'une API nommée comme acceptant un successeur puisse ne vérifier
  que la déclaration.

#### Tests RED requis

- [ ] `next_did` malformé.
- [ ] Même DID avant/après.
- [ ] Transition valide mais autre document successeur présenté.
- [ ] Document successeur invalide ou signature root invalide.
- [ ] Transition signée par root avec `#root`.
- [ ] Transition signée par root mais annonçant `#succession`.
- [ ] Transition signée par la succession d'un autre DID.
- [ ] Cas positif complet avec les vecteurs A2 étendus.

#### Critère de clôture

Le `Then` Gherkin transmet et valide réellement `next_doc`; le Provider et les
faits d'opération utilisent la même définition protocolaire, ou leur différence
est explicitement nommée et ne prétend pas implémenter §10.4.

### AID-003 — Supprimer toute dérivation de succession depuis le master owner

**Priorité : P1 — OUVERT**

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

**Priorité : P1 — DÉCISION REQUISE**

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

**Priorité : P2 — OUVERT**

#### Implémentation attendue

- [ ] Conserver le scénario de signature altérée, mais le nommer précisément.
- [ ] Ajouter un Scenario Outline de documents DID signés mais
  sémantiquement invalides couvrant AID-001.
- [ ] Remplacer le `Then` de transition par une vérification du triplet
  précédent/transition/successeur.
- [ ] Ajouter les négatifs AID-002.
- [ ] Ajouter un scénario de cérémonie de succession utilisant l'API réelle
  de création d'identité, plutôt que deux constantes choisies dans le step.
- [ ] Étendre A2 avec des cas négatifs générés indépendamment.
- [ ] Faire échouer le gate ciblé si le nombre exécuté n'est pas exactement
  celui attendu.

#### Critère de clôture

Chaque ligne Gherkin construit son défaut propre, appelle l'API de production
correspondante et vérifie son verdict propre. Les tests A1/A2 autonomes restent
des preuves complémentaires.

## Décisions à trancher

1. **API de transition.** Remplacer `EpochTransition::verify(prev)` par une API
   complète, ou conserver une primitive de signature clairement nommée et
   ajouter un validateur de succession distinct ?
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

- [ ] AID-001 à AID-005 sont clôturés ou explicitement arbitrés hors périmètre ;
- [ ] aucun scénario `a-identity.feature` n'est `@wip`, proxy ou vide ;
- [ ] le runner ciblé exécute exactement le nombre attendu de scénarios ;
- [ ] les tests A1/A2 positifs restent byte-exacts ;
- [ ] les nouveaux négatifs échouent avant correction puis passent après ;
- [ ] Core, Bundle, CLI, WASM, Gateway et Provider partagent le même verdict
  pour les objets DID et transitions qu'ils exposent ;
- [ ] `cargo test -p aithos-core --test a1_genesis --test a2_did` passe ;
- [ ] le runner Cucumber ciblé passe ;
- [ ] les gates workspace, Clippy et formatage passent ;
- [ ] la révision Git de clôture et les résultats exacts sont inscrits ici.

## Historique

| Date | État | Note |
|---|---|---|
| 2026-07-29 | `ANNOTÉE` | Marqueurs inline ajoutés sans exclusion : `@audit-partial` sur AID-001/AID-003/AID-004 et `@audit-false-positive` sur AID-002 ; rejeu ciblé inchangé, 9 scénarios et 30 steps passés. |
| 2026-07-29 | `OUVERTE` | Audit initial : neuf scénarios verts, trois écarts d'implémentation principaux et deux renforcements de preuve requis. |

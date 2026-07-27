# Plan d’action — intégrer `aithos-client` dans la Gateway pour les outils Ethos

Date : 2026-07-25  
Statut : **exécution en cours — gates 1 à 9 validées, release/activation en attente**  
Périmètre : Gateway locale de démonstration, outils MCP `ethos.*` uniquement  
Priorité : absence de régression sur les connecteurs non-Ethos et absence de corruption d’un Ethos existant

> État au 2026-07-25 : le canari d’écriture déléguée `circle` utilise désormais
> un `VerifiedWorkingSet` borné à l’opération. Il ne télécharge ni une zone
> complète ni les payloads `self`. Create/edit/delete ont été vérifiés contre
> un Provider réel in-process. Le backend reste `legacy` par défaut et
> `client-provider` exige une activation explicite. L’écriture publique
> déléguée reste hors de ce premier canari.

## 1. Décision proposée

La Gateway conserve son rôle de façade MCP, de serveur OAuth, de gestionnaire
de sessions et de routeur de connecteurs. Elle délègue progressivement la
logique de lecture et de mutation des Ethos au moteur Rust `aithos-client`.

La couche réseau du SDK JavaScript n’est pas embarquée dans le binaire Rust.
Les invariants de transport de `aithos-sdk` sont repris par un transport Rust
borné :

- lecture des heads et des artefacts Provider ;
- enveloppes signées `X-Aithos-Auth` ;
- publication du delta uniquement ;
- Gamma et manifest avec leurs heads distincts ;
- manifest en dernier ;
- CAS obligatoire ;
- détection d’un commit déjà effectué ;
- refus explicite en cas d’état incertain.

L’intégration se fait derrière une frontière `EthosBackend`, avec l’ancien
backend conservé comme référence et mécanisme de rollback.

```text
Cowork
  |
  v
Gateway MCP / OAuth / sessions
  |
  +-- outil exactement ethos.* --> EthosBackend
  |                                 |
  |                                 +-- legacy
  |                                 |
  |                                 `-- aithos-client
  |                                      |
  |                                      `-- transport Provider Rust
  |
  `-- tout autre outil -----------> routage actuel inchangé
```

## 2. Motivation

Le parcours Ethos actuel de la Gateway réimplémente une partie de la logique
déjà disponible dans `aithos-client` et `aithos-sdk`.

Le problème live observé sur l’écriture déléguée `circle` illustre le risque de
cette duplication :

- la session autorise la mutation ;
- le writer délégué est correctement construit ;
- une prélecture est cependant effectuée avec le lecteur technique permanent ;
- ce lecteur n’est pas couvert pour `circle` ;
- le Provider refuse la prélecture avec `403 not_covered` avant toute mutation.

`aithos-client` possède déjà les primitives nécessaires :

- vérification froide d’un snapshot non fiable ;
- vérification des mandats, de l’atténuation, des fenêtres et des révocations ;
- lecture locale de `public`, `circle` et `self` ;
- plans owner et délégués pour create/edit/delete ;
- chiffrement, scellement et signatures ;
- `PublicationPlan` immuable ;
- heads manifest et Gamma distincts ;
- calcul du delta à publier ;
- enveloppes Provider signées ;
- parité native/WASM.

Le but n’est pas de contourner les règles de la Gateway. Le but est qu’une
seule implémentation cryptographique et protocolaire construise les opérations
Ethos, tandis que la Gateway reste responsable de l’expérience MCP et de la
session.

## 3. Objectif mesurable

À la fin du chantier :

1. les six outils MCP suivants utilisent `aithos-client` pour un contexte
   explicitement activé :
   - `ethos.context` ;
   - `ethos.list` ;
   - `ethos.read` ;
   - `ethos.create` ;
   - `ethos.edit` ;
   - `ethos.delete` ;
2. toute opération non-Ethos suit le chemin actuel sans changement observable ;
3. les clés et les contenus `circle` restent dans la Gateway locale ;
4. le Provider ne reçoit que des artefacts et enveloppes protocolaires ;
5. une mutation n’envoie que les artefacts modifiés puis engage le nouveau
   manifest par CAS ;
6. une interruption ne déclenche jamais une seconde écriture automatique par
   l’ancien backend ;
7. le backend legacy peut être réactivé sans migration de données ;
8. un E2E réel prouve une mutation déléguée sur un Ethos neuf et un appel
   non-Ethos sur la même Gateway.

## 4. Non-objectifs

Ce chantier ne doit pas :

- remplacer ou refondre OAuth ;
- modifier le protocole MCP public ;
- renommer les outils ou changer leurs schémas ;
- modifier la découverte ou l’activation des connecteurs externes ;
- modifier le Hub, ses pins ou ses noms exposés ;
- modifier les connecteurs GitHub, Gmail, Sheets ou autres upstreams ;
- héberger les clés Ethos côté Provider ;
- ouvrir `self` à Cowork si le profil produit courant l’interdit ;
- supprimer immédiatement le backend legacy ;
- publier de nouvelle version npm ou crates.io sans décision séparée ;
- migrer ou réécrire un Ethos existant ;
- utiliser un Ethos de démonstration contenant des données utiles pour les
  premiers tests d’écriture.

## 5. Invariants non négociables

### 5.1 Connecteurs non-Ethos

Pour toute requête dont le nom d’outil n’est pas l’un des six noms réservés
`ethos.*` :

- même résolution du contexte ;
- même résolution du connecteur ;
- même contrôle de mandat ;
- même journalisation avant relayage ;
- mêmes arguments envoyés à l’upstream ;
- même bearer ou OAuth amont ;
- mêmes timeouts ;
- même réponse JSON-RPC ;
- même code et même classe d’erreur ;
- aucun appel à `aithos-client`.

Une différence détectée sur cet invariant bloque le chantier.

### 5.2 OAuth et cérémonie G4

Les routes et contrats suivants restent inchangés :

- discovery protected resource ;
- discovery authorization server ;
- DCR ;
- `/authorize` ;
- `/token` et refresh ;
- `/ceremony/prepare` ;
- `/ceremony/prepare-grant` ;
- `/ceremony/complete` ;
- `/ceremony/cancel`.

Le nouveau backend consomme uniquement une autorité de session déjà vérifiée.
Il ne réinterprète pas le token OAuth comme un mandat.

### 5.3 Confidentialité

- aucune seed n’entre dans un log ;
- aucun bearer n’entre dans un log ;
- aucun contenu de section n’entre dans un log ;
- aucun header `X-Aithos-Auth` complet n’entre dans un log ;
- aucun artefact `*.enc` n’entre dans un log ;
- les contenus `circle` sont déchiffrés uniquement dans la Gateway locale ;
- les buffers clairs ne sont pas conservés dans un cache partagé ;
- le keyholder est verrouillable et sa durée de vie est bornée à la requête ou
  à la session explicitement définie.

### 5.4 Autorité

Une signature valide ne suffit jamais. Chaque opération doit vérifier :

- le DID attendu ;
- la possession de la clé correspondant au bénéficiaire final ;
- la chaîne complète ;
- l’atténuation ;
- la fenêtre temporelle ;
- les révocations ;
- le périmètre et le verbe ;
- le `purpose` ;
- le `session_bind` ;
- la zone et, lorsqu’il existe, le sélecteur de section ;
- la correspondance entre l’acteur du plan et l’acteur de l’enveloppe Provider.

### 5.5 Publication

- le snapshot de départ est vérifié avant toute décision ;
- le plan est vérifié à froid avant transport ;
- le plan est vérifié contre le head courant ;
- seuls les chemins présents dans `upload_order` sont envoyés ;
- `manifests/<height>.json` n’est jamais écrit directement par le client ;
- `manifest.json` est le dernier artefact publié ;
- le head Gamma attendu est utilisé pour Gamma ;
- le head manifest attendu est utilisé pour le manifest ;
- un `409` n’est jamais transformé en succès sans relire et vérifier le head ;
- une erreur après le début d’une publication interdit un fallback automatique
  vers le backend legacy.

### 5.6 Fail closed

Une donnée manquante, une chaîne ambiguë, une clé indisponible, une réponse
Provider non bornée, un plan invalide ou un état de commit incertain produisent
un refus. Aucun chemin de compatibilité ne doit élargir l’autorité.

## 6. Architecture cible

### 6.1 Frontière `EthosBackend`

Créer une interface interne asynchrone, limitée aux besoins MCP :

```rust
trait EthosBackend {
    async fn context(...);
    async fn list(...);
    async fn read(...);
    async fn create(...);
    async fn edit(...);
    async fn delete(...);
}
```

Les types d’entrée doivent être fermés et sémantiques. L’interface ne doit pas
exposer :

- un `sign(bytes)` générique ;
- un chemin Provider arbitraire ;
- une méthode HTTP arbitraire ;
- une enveloppe libre ;
- un accès direct aux seeds.

### 6.2 Implémentations

`LegacyEthosBackend` :

- appelle le comportement existant ;
- sert de référence pendant la transition ;
- reste le défaut jusqu’à la fin des gates.

`ClientEthosBackend` :

- matérialise l’autorité de session ;
- télécharge un snapshot borné ;
- appelle `aithos-client` ;
- produit ou vérifie le `PublicationPlan` ;
- utilise le transport Provider Rust ;
- traduit les erreurs vers le registre MCP actuel.

`ShadowEthosBackend` :

- autorisé uniquement pour les lectures et les dry-runs ;
- appelle legacy et client ;
- retourne la réponse legacy ;
- compare uniquement des résultats normalisés ou des digests non secrets ;
- n’effectue jamais deux publications.

### 6.3 Activation

Configuration proposée :

```yaml
ethos_backend:
  default: legacy
  contexts:
    sales-canary: client
```

Valeurs fermées :

- `legacy` ;
- `shadow-read` ;
- `client`.

Règles :

- une configuration absente équivaut à `legacy` ;
- une valeur inconnue refuse le démarrage ;
- `shadow-read` refuse toute mutation ;
- l’override est par contexte, jamais par nom de connecteur externe ;
- le backend choisi est journalisé sans DID, contenu ou autorité.

## 7. Modèle d’autorité du backend Client

### 7.1 Entrée de session

La Gateway remet au backend un objet déjà borné :

```text
SessionEthosAuthority
  context
  did
  leaf_id
  chain
  session_pub
  session_bind
  gateway/resource purpose
  verification instant
  permitted zones and verbs
```

Le backend recroise ces données avec le snapshot et avec `aithos-client`.

### 7.2 Keyholder

Le keyholder doit signer avec la clé correspondant au bénéficiaire présenté au
Provider. Le premier spike doit prouver, sans mutation distante :

1. quelle clé de la Gateway signe actuellement les enveloppes déléguées ;
2. que la feuille de la chaîne présentée au Provider nomme exactement cette
   clé ;
3. que la capacité KEX attendue permet d’ouvrir la zone `circle` ;
4. que `session_bind` est conservé sans élargissement ;
5. qu’un `MemoryGranteeKeyholder` ou un adaptateur plus générique peut être
   construit sans exporter la seed hors du keyholder de la Gateway.

Si l’API `aithos-client` exige une modification, elle doit être limitée à une
abstraction de keyholder typée. Une API générique de signature est interdite.

### 7.3 Durée de vie

Pour la première version :

- keyholder dérivé par appel ;
- aucun cache de contenu clair ;
- aucun cache global de chaîne active ;
- verrouillage à la fin de l’appel ;
- entropie injectée distincte pour chaque tentative.

Une optimisation de cache ne peut être étudiée qu’après les E2E et une revue
de sécurité distincte.

## 8. Transport Provider Rust

Le transport est une couche sans autorité métier. Il ne décide jamais si une
opération est permise.

### 8.1 Lecture

Séquence minimale :

1. lire `/heads` avec une enveloppe correspondant à l’autorité de session ;
2. déterminer les artefacts nécessaires depuis le manifest ;
3. utiliser batch lorsque le contrat et les limites le permettent ;
4. borner le nombre d’artefacts et la taille totale ;
5. remettre les octets non fiables à `aithos-client` ;
6. ne retourner du contenu qu’après vérification froide et autorisation.

Il est interdit d’utiliser le lecteur technique permanent pour une prélecture
de contenu `circle` effectuée au nom d’une session déléguée.

### 8.2 Écriture

Séquence :

1. obtenir et vérifier le snapshot courant ;
2. construire le plan avec `aithos-client` ;
3. appeler `verify_against` sur le snapshot de départ ;
4. vérifier l’`upload_order` ;
5. générer une enveloppe fraîche par artefact ;
6. uploader le delta dans l’ordre ;
7. utiliser le head Gamma pour un segment Gamma ;
8. publier `manifest.json` en dernier avec le head manifest ;
9. relire les heads ;
10. télécharger et vérifier à froid le snapshot résultant ;
11. retourner le nouveau head seulement après vérification.

### 8.3 Reprise et idempotence

Avant `manifest.json`, un retry d’artefact n’est accepté que si :

- l’artefact est immuable et les octets stockés sont identiques ; ou
- le contrat Provider déclare explicitement le dépôt idempotent.

Après une erreur sur `manifest.json` :

1. ne pas republier immédiatement ;
2. relire les heads ;
3. si le manifest courant est le `new_head` du plan, déclarer
   `already_committed` ;
4. sinon retourner un conflit ou un état incertain ;
5. ne jamais exécuter le backend legacy pour « essayer autrement ».

### 8.4 Divergence Gamma

Gamma et manifest possèdent des heads distincts. Le transport doit distinguer :

- plan encore publiable ;
- Gamma déjà avancé par une autre opération ;
- Gamma du plan déjà accepté mais manifest non engagé ;
- manifest déjà engagé ;
- conflit exigeant un rebase.

La logique de référence de `aithos-sdk` doit être transformée en tests
contractuels byte-exacts avant son portage Rust.

## 9. Stratégie TDD et phases

Chaque phase suit :

1. RED : test démontrant le manque ou protégeant un comportement existant ;
2. GREEN : changement minimal ;
3. REFACTOR : uniquement sous tests verts ;
4. gate d’arrêt ;
5. commit isolé recommandé.

### Phase 0 — geler la baseline

But : disposer d’une preuve de non-régression avant la première modification.

Travaux :

- enregistrer le HEAD des dépôts `aithos-core` et `aithos-client` ;
- enregistrer les hashes des binaires de démo utilisés ;
- capturer la configuration publique sans secret ;
- lancer les tests Gateway ciblés existants ;
- ajouter des tests de caractérisation du routage non-Ethos ;
- capturer les réponses normalisées de `initialize` et `tools/list` ;
- tester un upstream statique fictif ;
- tester un upstream dynamique fictif ;
- tester un connecteur avec OAuth amont simulé ;
- tester journalisation, refus et réponse de l’upstream ;
- effectuer un smoke manuel GitHub en lecture sur la Gateway actuelle.

Livrables :

- fixtures JSON-RPC sans secret ;
- matrice des outils exposés ;
- rapport baseline avec commandes et résultats.

Gate :

- aucun développement si la baseline n’est pas reproductible ;
- aucun développement si les tests non-Ethos sont déjà instables.

### Phase 1 — spike de compatibilité sans routage

But : prouver que la Gateway peut lier `aithos-client` sans changer son
comportement.

Travaux :

- ajouter une dépendance locale épinglée vers le crate Rust `aithos-client` ;
- confirmer qu’elle réutilise les mêmes instances de `aithos-core` et
  `aithos-bundle` ;
- refuser toute duplication de versions protocolaires ;
- compiler la Gateway sans instancier le nouveau backend ;
- construire en test un snapshot Client à partir d’une fixture Gateway ;
- prouver la compatibilité du keyholder de session ;
- documenter tout écart d’API.

Gate :

- stop si deux versions de `aithos-core` ou `aithos-bundle` apparaissent ;
- stop si l’intégration exige une opération de signature arbitraire ;
- stop si une seed doit sortir en clair d’un composant qui ne la détenait pas ;
- stop si le coût binaire ou mémoire est jugé incompatible avec la démo sans
  investigation séparée.

### Phase 2 — extraire la frontière backend, comportement legacy identique

But : introduire le seam sans changer une seule réponse.

RED :

- tests byte-identiques avant/après sur les six outils Ethos legacy ;
- tests byte-identiques sur les outils non-Ethos ;
- test configuration absente égale `legacy` ;
- test valeur inconnue refuse le démarrage.

GREEN :

- créer `EthosBackend` ;
- encapsuler les appels existants dans `LegacyEthosBackend` ;
- brancher uniquement les six constantes réservées ;
- conserver la génération actuelle de `tools/list`.

Gate :

- suite Gateway complète verte ;
- diff de réponse nul sur les fixtures ;
- aucun changement dans `hub.rs`, `connectors.rs` ou les routes OAuth, sauf
  justification et gate séparés.

### Phase 3 — transport Provider Rust contractuel

But : reproduire les invariants utiles d’`aithos-sdk` sans logique MCP.

RED :

- enveloppe read heads ;
- enveloppe read objet ;
- enveloppe batch ;
- enveloppe PUT artefact ;
- distinction expected manifest head / expected Gamma head ;
- manifest en dernier ;
- corps et hash byte-exacts ;
- nonce nouveau ;
- `409` concurrent ;
- `already_committed` vérifié par relecture ;
- interruption avant manifest ;
- refus d’un plan contenant `manifests/**` ;
- réponse Provider surdimensionnée ;
- timeout ;
- absence de fuite dans les erreurs.

GREEN :

- transport testable derrière une interface HTTP injectée ;
- aucune dépendance au routeur MCP ;
- tests contre un Provider in-process réel.

Gate :

- parité avec les fixtures du SDK ;
- aucune publication live à cette phase.

### Phase 4 — lectures Client en mode shadow

But : valider `context`, `list` et `read` sans changer ce que reçoit Cowork.

RED :

- public lisible ;
- circle lisible avec mandat couvrant ;
- sibling refusé ;
- mandat expiré refusé ;
- mandat révoqué refusé ;
- leaf/key mismatch refusé ;
- `self` refusé selon la politique de session Cowork ;
- snapshot altéré refusé ;
- prélecture effectuée avec l’autorité de session ;
- aucun appel via le reader technique permanent.

GREEN :

- implémenter les trois lectures dans `ClientEthosBackend` ;
- ajouter `shadow-read` ;
- comparer :
  - zones ;
  - nombre d’éléments ;
  - paths ;
  - digests de contenu ;
  - codes de refus ;
- ne jamais comparer ou logger le contenu lui-même.

Gate :

- zéro divergence inexpliquée sur les fixtures ;
- zéro divergence sur un nouvel Ethos de canari ;
- aucun changement de latence non borné.

### Phase 5 — mutations Client en dry-run

But : construire de vrais plans sans écrire.

RED :

- create public couvert ;
- edit public couvert ;
- delete public couvert ;
- create circle couvert ;
- edit circle couvert ;
- delete circle couvert ;
- absence de droit ;
- mauvais sélecteur ;
- no-op ;
- stale head ;
- entropie réutilisée ;
- chain reorder ;
- subject mismatch ;
- expected digest incorrect ;
- plan dont le dernier upload n’est pas `manifest.json`.

GREEN :

- transformer les arguments MCP en `MutationIntent` fermé ;
- construire le `PublicationPlan` ;
- vérifier le plan à froid ;
- exposer uniquement un résultat interne de dry-run ;
- comparer les paths et heads attendus avec les invariants Gateway existants.

Gate :

- aucune publication réseau ;
- aucune double exécution ;
- revue manuelle des deltas d’artefacts pour public et circle.

### Phase 6 — E2E Provider isolé

But : prouver une vraie publication sans toucher à un Ethos existant.

Préconditions :

- nouveau tenant ou namespace de test lorsque possible ;
- nouveau DID ;
- nouveau mandat court ;
- sections synthétiques sans valeur ;
- heads initiaux enregistrés ;
- procédure de lecture et vérification post-commit prête ;
- aucune suppression automatique de données.

Scénarios :

1. owner crée l’Ethos de test ;
2. délégataire lit public ;
3. délégataire lit circle ;
4. délégataire crée une section circle ;
5. relire et vérifier à froid ;
6. délégataire édite la même section ;
7. relire et vérifier à froid ;
8. délégataire supprime la section ;
9. relire et vérifier à froid ;
10. droit voisin refusé ;
11. mandat expiré ou révoqué refusé ;
12. deux writers sur le même head produisent un succès et un conflit ;
13. interruption simulée avant manifest ;
14. reprise sans double commit ;
15. public délégué uniquement si le contrat produit le permet explicitement.

Gate :

- chaque head accepté possède un snapshot froidement vérifiable ;
- aucun artefact inattendu ;
- aucun write sur un DID hors canari ;
- pas de bascule live si Gamma et manifest restent divergents.

### Phase 7 — canari Gateway local

But : activer le backend Client pour un contexte neuf uniquement.

Configuration :

```yaml
ethos_backend:
  default: legacy
  contexts:
    cowork-client-canary: client
```

Scénarios :

- OAuth Cowork complet ;
- `tools/list` contient les mêmes outils ;
- `ethos.context` ;
- `ethos.list` ;
- `ethos.read` ;
- create/edit/delete circle ;
- refus public ou succès public conforme à la politique décidée ;
- appel GitHub en lecture dans la même Gateway ;
- activation d’un connecteur dynamique ;
- refresh OAuth ;
- redémarrage Gateway puis nouvelle session.

Gate :

- les appels non-Ethos sont byte-identiques aux fixtures ;
- l’appel GitHub live reste fonctionnel ;
- aucune hausse anormale des erreurs ou timeouts ;
- rollback testé avant la bascule démo.

### Phase 8 — déploiement démo réversible

Travaux :

- construire un nouveau binaire release dans un target externe ;
- désactiver l’incrémental si l’espace disque l’exige ;
- calculer et enregistrer le SHA-256 ;
- installer sous un nouveau nom immuable ;
- conserver le binaire précédent ;
- sauvegarder la configuration publique et le mapping de binary path ;
- activer uniquement le contexte de canari ;
- jouer le smoke non-Ethos ;
- jouer le smoke Ethos ;
- étendre ensuite aux nouveaux contextes de démo, jamais automatiquement aux
  anciens.

Gate :

- aucune suppression de l’ancien binaire ;
- aucun remplacement in-place ;
- aucune activation globale avant observation du canari.

### Phase 9 — convergence ultérieure

Cette phase n’est pas nécessaire pour la première correction.

Après une période d’observation :

- supprimer les duplications Ethos devenues inutiles ;
- conserver les tests de caractérisation ;
- décider si `legacy` reste un rollback durable ;
- aligner la documentation et les versions publiées ;
- envisager une crate de transport partagée plutôt qu’un port maintenu à la
  main.

La suppression du backend legacy exige une décision séparée.

## 10. Matrice minimale de tests anti-régression

| Domaine | Test | Attendu |
| --- | --- | --- |
| MCP | `initialize` | byte-identique hors instruction Ethos attendue |
| MCP | `tools/list` sans Ethos | byte-identique |
| MCP | `tools/list` avec Ethos | mêmes noms et schémas |
| Connecteur statique | call read | même upstream, mêmes arguments |
| Connecteur dynamique | activation + call | comportement inchangé |
| OAuth amont | bearer/refresh | comportement inchangé |
| Hub | pin et exposed name | comportement inchangé |
| Journal | log avant relay | comportement inchangé |
| Briefing | présence conditionnelle | comportement inchangé |
| Refus | outil inconnu | même code et message normalisé |
| Ethos read | public | même contenu vérifié |
| Ethos read | circle couvert | succès |
| Ethos read | circle non couvert | refus |
| Ethos write | create/edit/delete | plan valide + Provider accepté |
| Ethos write | stale head | conflit, aucun fallback |
| Ethos write | partial upload | head manifest inchangé ou commit prouvé |
| Sécurité | logs | aucun secret ou contenu |
| Live | GitHub read | succès avant et après |
| Live | Cowork Ethos canari | succès et refus voisin |

## 11. Observabilité

Métriques autorisées :

- backend sélectionné ;
- outil Ethos appelé ;
- contexte pseudonymisé par digest ;
- zone ;
- classe d’opération ;
- résultat success/refusal/conflict/unavailable ;
- nombre d’artefacts ;
- nombre d’octets total ;
- durée fetch/verify/plan/publish/reverify ;
- ancien et nouveau head tronqués ou digestés ;
- divergence shadow oui/non.

Interdictions :

- DID complet dans les logs de niveau normal si non nécessaire ;
- paths sensibles `self` ;
- titre, tags ou body ;
- ciphertext ;
- bearer ;
- seed ;
- recovery ;
- enveloppe complète ;
- mandat complet ;
- réponse MCP contenant du contenu.

Les traces détaillées restent opt-in, bornées et redacted.

## 12. Rollback

### 12.1 Avant toute publication

Un échec peut retourner une erreur MCP. Aucun état distant n’a changé.

### 12.2 Après le début d’une publication

Le système :

1. interdit le fallback legacy ;
2. relit les heads ;
3. compare avec `expected_head` et `new_head` ;
4. vérifie les artefacts déjà déposés si nécessaire ;
5. retourne `already_committed`, `conflict` ou `state_uncertain` ;
6. exige une décision opérateur avant nouvelle tentative en cas d’incertitude.

### 12.3 Rollback de déploiement

1. repasser le contexte de canari à `legacy` ;
2. arrêter proprement le nouveau binaire ;
3. relancer le binaire précédent avec sa configuration précédente ;
4. vérifier discovery et `tools/list` ;
5. tester un connecteur non-Ethos ;
6. relire les heads de l’Ethos canari ;
7. ne supprimer aucun artefact Provider.

Le rollback ne réécrit pas l’historique. Les éditions déjà committées restent
valides.

## 13. Risques et réponses

| Risque | Impact | Réponse |
| --- | --- | --- |
| Régression du routeur générique | tous les connecteurs | seam limité aux six constantes + tests byte-identiques |
| Double publication | corruption logique/conflit | jamais de dual-write, jamais de fallback après début d’upload |
| Mauvaise clé leaf | `chain_invalid` ou élargissement | spike keyholder + recroisement leaf/key |
| Mauvais reader de préflight | `not_covered` | toutes les lectures au nom de la session utilisent la même autorité |
| Divergence Gamma/manifest | publication bloquée | heads distincts, relecture, rebase explicite |
| Deux versions Core | décisions différentes | gate Cargo, dépendances uniques et épinglées |
| Client dev non publié | build non reproductible | pin de révision + hash + CI locale |
| Contenu clair dans les logs | fuite | redaction tests + métriques sans contenu |
| Hausse de latence | blocage MCP | mesures par phase + bornes fetch/verify |
| Manque d’espace disque | build interrompu | target externe, incrémental off, aucun nettoyage destructif automatique |
| Différence de politique `self` | élargissement d’autorité | politique Gateway prioritaire, test de refus Cowork |
| Public write non stabilisé | comportement ambigu | gate produit explicite avant activation |
| État live différent des fixtures | surprise démo | canari DID neuf + E2E Provider réel |

## 14. Fichiers et dépendances anticipés

Modifications probables dans `aithos-core` :

- `rust/Cargo.toml` ou workspace de dépendances ;
- `rust/Cargo.lock` ;
- `rust/crates/aithos-gateway/Cargo.toml` ;
- `rust/crates/aithos-gateway/src/config.rs` ;
- `rust/crates/aithos-gateway/src/proxy_mcp.rs`, câblage minimal ;
- `rust/crates/aithos-gateway/src/lib.rs` ;
- nouveau `rust/crates/aithos-gateway/src/ethos_backend.rs` ;
- nouveau `rust/crates/aithos-gateway/src/ethos_client_backend.rs` ;
- nouveau `rust/crates/aithos-gateway/src/provider_transport.rs` ;
- nouveaux tests contractuels et E2E.

Modifications possibles dans `aithos-client`, uniquement si nécessaires :

- généralisation bornée du keyholder délégataire ;
- export d’un type de plan ou d’une primitive déjà existante mais non publique ;
- aucune API de signature générique ;
- nouveaux tests de compatibilité Gateway.

Fichiers qui ne doivent pas être modifiés dans les premières phases :

- logique métier de `connectors.rs` ;
- logique métier de `hub.rs` ;
- implémentations OAuth ;
- profils de connecteurs externes ;
- assets de cérémonie ;
- Provider déployé, sauf découverte d’un défaut de contrat prouvé par un test
  indépendant.

Toute nécessité de toucher ces zones déclenche une revue et un sous-plan.

## 15. Discipline Git et builds

- travailler sur une branche dédiée ;
- commits courts par phase ;
- ne pas mélanger documentation, refactor générique et comportement Ethos ;
- ne pas reformater des fichiers sans rapport ;
- conserver les changements utilisateur existants ;
- ne jamais utiliser `git reset --hard` ou une suppression de worktree ;
- utiliser un `CARGO_TARGET_DIR` externe et nommé ;
- conserver `CARGO_INCREMENTAL=0` pour les builds release de démo si nécessaire ;
- enregistrer le hash de chaque binaire installé ;
- ne jamais remplacer un binaire en cours d’exécution.

Ordre recommandé des commits :

1. tests de caractérisation non-Ethos ;
2. dépendance et spike de compatibilité ;
3. seam `EthosBackend` avec legacy uniquement ;
4. transport Provider contractuel ;
5. lectures Client shadow ;
6. mutations dry-run ;
7. E2E Provider isolé ;
8. activation canari ;
9. documentation et runbook de rollback.

## 16. Conditions d’arrêt immédiat

Le développement s’arrête si :

- un test non-Ethos change de résultat ;
- l’intégration requiert de modifier le protocole OAuth ;
- l’intégration requiert d’exposer une seed ;
- une opération Client exige plus de droits que le mandat ;
- le plan touche un chemin inattendu ;
- un snapshot post-publication ne passe pas la vérification froide ;
- Gamma et manifest restent dans un état inexpliqué ;
- un conflit CAS est masqué ;
- un fallback pourrait exécuter deux fois la mutation ;
- un E2E viserait un Ethos existant contenant des données utiles ;
- le Provider doit être assoupli sans test contractuel indépendant ;
- l’espace disque impose un nettoyage risqué ;
- le rollback n’est pas démontré avant l’activation démo.

Dans ces cas, produire un rapport de blocage et demander une décision explicite.

## 17. Définition de terminé

Le chantier est terminé pour la démo uniquement lorsque :

- [ ] les tests de caractérisation non-Ethos sont verts ;
- [ ] la Gateway lie une révision épinglée d’`aithos-client` ;
- [ ] une seule version de Core/Bundle est présente ;
- [ ] `LegacyEthosBackend` est byte-identique au comportement initial ;
- [ ] `ClientEthosBackend` lit public et circle avec la session correcte ;
- [ ] les six outils Ethos possèdent leurs tests de succès et de refus ;
- [ ] create/edit/delete produisent des plans froidement vérifiés ;
- [ ] le transport publie le delta et le manifest en dernier ;
- [ ] les conflits et reprises sont testés ;
- [ ] un nouvel Ethos canari passe le E2E Provider réel ;
- [ ] un connecteur GitHub ou équivalent fonctionne avant et après ;
- [ ] OAuth et refresh fonctionnent avant et après ;
- [ ] aucun secret ou contenu n’apparaît dans les logs ;
- [ ] le binaire précédent est conservé ;
- [ ] le rollback a été joué ;
- [ ] la configuration par défaut reste `legacy` ;
- [ ] l’activation démo est bornée aux contextes explicitement choisis.

## 18. Décisions à prendre avant GREEN

### D1 — transport

Recommandation : transport Rust interne, contractuellement aligné sur
`aithos-sdk`. Ne pas embarquer Node dans la Gateway.

### D2 — dépendance `aithos-client`

Recommandation : path local pendant le développement, révision Git immuable ou
release exacte avant toute CI distante/release.

### D3 — keyholder

Recommandation : adaptateur typé sur les opérations Client. Refuser une API
`sign(arbitrary_bytes)`.

### D4 — activation

Recommandation : flag par contexte, défaut `legacy`, canari sur DID neuf.

### D5 — public délégué

Recommandation : ne l’activer qu’après un E2E séparé prouvant le carrier public,
la couverture Provider et la politique produit. `circle` reste le premier
parcours d’écriture délégué canari.

### D6 — `self`

Recommandation : conserver le refus Cowork actuel, même si les primitives
Client sous-jacentes savent construire d’autres profils.

### D7 — dual-run

Recommandation : shadow uniquement pour les lectures et plans non publiés.
Jamais deux writers.

### D8 — working set mandaté projeté (décision validée après gate 8)

Le Provider refuse correctement `e/self/**` à un mandat limité à circle, alors
que le `VerifiedSnapshot` actuel exige le bundle intégral épinglé par le
manifeste. Ne jamais contourner ce conflit en élargissant la couverture du
Provider.

Décision appliquée : ajouter dans `aithos-client` un type distinct
`VerifiedWorkingSet`, qui :

- vérifie la signature, la chaîne et le CAS du manifeste intégral ;
- exige toutes les preuves globales nécessaires ;
- exige uniquement les artefacts nécessaires à l’opération annoncée ;
- exige le blob cible pour edit/delete et aucun blob de zone pour create ;
- vérifie la queue Gamma et la ligne Header destinée exactement au grantee ;
- refuse un Header destiné à un bénéficiaire voisin ;
- refuse tout payload `self` dans le working set `circle` ;
- refuse toute lecture ou mutation hors de la projection ;
- ne peut jamais être converti implicitement en `VerifiedSnapshot`.

Le vérificateur intégral `VerifiedSnapshot` n’a pas été assoupli. Les deux
types conservent donc des contrats distincts : bundle intégral pour le premier,
ensemble minimal explicitement borné pour le second.

## 19. Première tranche exécutable

La première tranche de développement doit s’arrêter avant toute écriture live.

Elle contient uniquement :

1. baseline et tests non-Ethos ;
2. spike de dépendance/keyholder ;
3. seam `EthosBackend` ;
4. backend legacy inchangé ;
5. transport Provider sous mocks/in-process ;
6. lectures Client en shadow ;
7. plans de mutation dry-run.

Un go/no-go explicite est requis avant la phase 6 et les premières publications
sur le Provider réel.

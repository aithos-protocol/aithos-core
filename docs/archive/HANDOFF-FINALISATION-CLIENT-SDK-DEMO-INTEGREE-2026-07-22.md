# HANDOFF — Finaliser Client, SDK et dashboard pour la démo intégrée

> **ARCHIVE — phases Client/SDK/dashboard livrées.** Ce plan a été exécuté puis
> remplacé, pour son reliquat Core/Gateway, par
> `HANDOFF-GATEWAY-COMPAGNON-DEMO-INTEGREE-2026-07-22.md`. Le gate navigateur
> live reste nécessaire avant de qualifier la démo intégrée.

Date : 2026-07-22

Statut historique : **exécuté pour Client/SDK/dashboard ; reliquat transféré**.

## 0. Décision produit opposable

Le chemin critique de la démo est désormais :

```text
Dashboard
   → OAuth entrant Gateway + PKCE
   → publication et vérification du parent G4
   → cérémonie et token en mémoire
   → MCP Google Sheets read-only
   → refus voisin
   → preuve Gamma vérifiée
```

Les approbations Gmail sont **entièrement hors périmètre**. Ne pas ajouter au
client, au SDK ou au dashboard les routes review/approve/deny/dispatch et ne pas
créer d'écran correspondant.

Si le beat principal est terminé et qu'il reste du temps, un lot optionnel
pourra exposer un envoi d'email direct par un outil MCP dédié utilisant l'API
Gmail. Ce lot est décrit au §12 ; il ne doit jamais retarder ni fragiliser la
démo Sheets.

## 1. Objectif de fini

Livrer un parcours navigateur sans saisie ou copie manuelle de
`transactionId`, callback OAuth ou bearer :

1. l'Owner reconnecte localement son Ethos ;
2. le SDK démarre OAuth entrant par discovery, DCR et PKCE ;
3. le client émet et publie le parent G4 pour l'audience découverte ;
4. la Gateway retrouve et vérifie ce parent ;
5. le délégué signe localement leaf, grant et preuve de cérémonie ;
6. le SDK échange le code en vérifiant le state ;
7. le bearer reste uniquement en mémoire et initialise MCP ;
8. `tools/list` expose le connecteur Sheets pré-provisionné ;
9. un `read_range` réel réussit ;
10. une capability ou un périmètre voisin est refusé avant l'amont ;
11. la preuve Gamma est chargée puis vérifiée localement ;
12. logout verrouille les handles et oublie tokens et autorités.

Le parcours doit fonctionner dans un navigateur normal. Une extension qui
désactive CORS, un navigateur lancé avec `--disable-web-security`, un bearer
collé manuellement ou un succès simulé invalide le gate.

## 2. Dépôts, baselines et état reçu

| Dépôt | Branche | HEAD de référence | État reçu |
| --- | --- | --- | --- |
| `code/aithos-core` | `codex/publish-aithos-core-busl` | `2322b91` | propre ; documentation et Gateway actuelles |
| `code/aithos-client` | `codex/client-sdk-v2-parking` | `890acf3` | propre ; G4/client WASM livré |
| `code/aithos-sdk` | `codex/g1-g7-enterprise-sdk` | `4d28f1c` | propre ; OAuth/G4/MCP livré |
| `code/aithos-sdk-example` | `codex/g1-g7-enterprise-dashboard` | `914c629` | propre ; `/` et `/delegation` livrés |

Les quatre SHA sont une baseline coordonnée, pas quatre releases publiées.
Commencer par vérifier qu'ils n'ont pas bougé. Si un dépôt a avancé, attribuer
les changements, relire le diff et consigner le nouveau SHA avant toute écriture.

Le présent document remplace le plan d'action historique de
`HANDOFF-CLIENT-SDK-G4-INTEGRATION-2026-07-22.md`, dont les écarts G4 ont depuis
été implémentés. L'état des lieux factuel reste dans
`ETAT-DES-LIEUX-DEMO-GATEWAY-CLIENT-SDK-2026-07-22.md`.

## 3. Ce qui existe déjà et doit être préservé

### Client

- cold verification complète ;
- handles Owner, délégué et signer de cérémonie opaques ;
- parent G4 fermé avec `Issue(depth=1)`, actions exactes et `max_sessions` ;
- publication K1-C, manifest en dernier et vérification CAS locale ;
- vérification parent + leaf, grant Gamma et preuve de cérémonie ;
- zéro réseau et aucune persistance dans le moteur client.

### SDK

- `ProviderClient.publish()` et lecture publique ;
- `GatewayOAuthClient` : discovery, DCR, PKCE, state, code, token et refresh ;
- `GatewayCeremonyClient` et `G4SessionParentOrchestrator` ;
- MCP Streamable HTTP JSON/SSE, bearer et refus `-32001` typé ;
- contrôle Gateway, pagination et preuve Gamma vérifiée ;
- utilisation du vrai package `@aithos/client`, sans crypto TypeScript.

### Dashboard

- `/` conserve la console Provider/Gateway historique ;
- `/delegation` joue le parcours offline Owner/délégué ;
- aucune seed, récupération, mandat ou token n'est persisté dans le storage ;
- le build SSR/Vinext est vert.

Ne pas remplacer ces surfaces. Les changements sont additifs ou corrigent un
défaut démontré.

## 4. Problèmes à fermer

### FCS-1 — grammaire control client incomplète

Le client ne signe pas encore les routes récentes nécessaires au lifecycle
minimal des profils :

- `POST /control/v1/connectors/{id}/profile-stage` avec JSON fermé ;
- `PUT /control/v1/connectors/{id}/client-secret` avec JSON fermé et borné ;
- `POST /control/v1/connectors/{id}/disconnect` avec corps de zéro octet.

Ajouter uniquement ces routes. Exclure explicitement toutes les routes
`/approvals/**`.

### FCS-2 — faux corps vide dans le SDK

`startOAuth()` et `activate()` envoient actuellement `{}`. La Gateway et le
client exigent zéro octet. Ajouter au transport SDK une opération signée sans
corps ; ne pas traiter `{}`, `null`, une ligne vide ou un JSON vide comme
équivalent.

Appliquer la même primitive à `disconnect()` et aux autres routes déclarées
`ControlBody::Empty`. Le digest et l'enveloppe doivent porter exactement le
corps vide attendu par A.2.

### FCS-3 — flux OAuth/G4 fragmenté dans le dashboard

La page avancée demande encore manuellement la transaction et l'audience, ne
conserve pas le même objet OAuth dans l'orchestrateur et s'arrête sur un lien de
callback.

Elle doit utiliser :

```text
const oauth = sdk.oauth(locator)
authorization = oauth.beginAuthorization(...)
const g4 = sdk.g4(locator, { oauth })
g4.waitForVerifiedParent({ authorization, ... })
g4.startCeremony(...)
tokens = oauth.exchange(authorization, redirectTo)
mcp = sdk.mcp(locator, { bearer: tokens.accessToken, ... })
```

L'objet `authorization`, le verifier, le state, le bearer et le refresh token
restent dans des références mémoire non rendues. Ils ne vont ni dans React
state affichable, ni dans une URL de navigation, ni dans un storage, ni dans les
logs. Le `redirectTo` est donné directement à `exchange()` ; le dashboard ne
navigue pas vers une route callback qui perdrait les secrets PKCE en mémoire.

### FCS-4 — CORS incomplet

Le dashboard local parle à plusieurs origines. Aujourd'hui :

- le control plane possède une allowlist exacte ;
- OAuth, cérémonie et MCP ne bénéficient pas de cette politique ;
- le Provider autorise les lectures publiques, pas les publications signées et
  leur preflight.

Le lot compagnon Core doit fournir une politique exacte et fermée :

- origine issue d'une allowlist explicite, jamais réfléchie aveuglément ;
- aucun `Access-Control-Allow-Credentials` ;
- méthodes et headers strictement nécessaires par route ;
- `OPTIONS` sans authentification, sans mutation et sans accès Vault/amont ;
- `Vary: Origin` quand la réponse dépend de l'origine ;
- aucun wildcard sur les routes qui retournent token, contenu de cérémonie ou
  résultat signé ;
- configuration de démo incluant explicitement l'origine locale retenue ;
- comportement inchangé si aucune origine dashboard n'est configurée.

Surfaces minimales : discovery OAuth, DCR, authorize JSON, token, cérémonie,
MCP et publications Provider signées avec `X-Aithos-Auth`, `If-Head` et les
headers réellement employés par le SDK.

### FCS-5 — absence de bundle et d'E2E réels

Il manque une configuration reproductible combinant Gateway, Vault, Provider,
state OAuth durable, contexte, Ethos, profil Sheets, origine dashboard et
credentials Google hors dépôt.

Les tests actuels utilisent le vrai WASM mais des transports HTTP simulés. Le
gate final doit être un navigateur réel traversant tous les transports.

## 5. Frontières de sécurité

- Aucun changement de wire Core, de grammaire de mandat, de canonicalisation ou
  de vecteur normatif.
- Aucune signature, construction de mandat ou vérification cryptographique en
  JavaScript/TypeScript.
- Aucune seed, clé privée, récupération, code OAuth, verifier, access token ou
  refresh token dans localStorage, sessionStorage, IndexedDB, Cache Storage,
  URL, HTML, console ou artefact de test.
- Les credentials Google, Vault tokens et autorités réelles n'entrent jamais
  dans Git.
- Les refus restent fail-closed et ne produisent aucun appel amont.
- Le Provider et le backend dashboard ne deviennent pas des proxies de secrets
  ou d'autorité.
- Les APIs et marqueurs de capability existants ne sont pas renommés sans
  migration et tests de compatibilité.
- Gmail approvals, Gmail direct et Sheets write ne sont pas introduits dans le
  beat principal.

## 6. Rituel de développement et contrats

Chaque phase suit ce rituel :

1. écrire ou compléter le contrat Gherkin/test avant le code ;
2. faire constater le RED pour la cause attendue ;
3. committer le contrat seul dans le dépôt propriétaire ;
4. implémenter le minimum ;
5. rejouer le test ciblé puis le gate complet du dépôt ;
6. faire une revue indépendante sécurité/tests ;
7. committer l'implémentation sans inclure de fichiers générés ;
8. actualiser les SHA et preuves dans le handoff de sortie.

Contrats croisés obligatoires :

- vecteur exact d'une route JSON et d'une route à corps zéro ;
- matrice CORS par méthode, route, origine et headers ;
- séquence OAuth/G4 avec mêmes bindings du début à l'échange ;
- scanner storage/réseau avec sentinelles de secret ;
- appel Sheets autorisé et refus voisin prouvé sans hit amont.

## 7. Découpage d'implémentation

### Phase 0 — préflight et baseline

- vérifier les branches, HEAD, worktrees et versions Node/Rust ;
- construire le tarball client local et vérifier son hash dans SDK/dashboard ;
- rejouer tous les gates de baseline du §10 ;
- consigner les endpoints et noms exacts du connecteur de démo sans secret ;
- ne pas commencer si un dépôt contient des changements non attribués.

### Phase 1 — client : parité control bornée

Propriétaire : `aithos-client` uniquement.

- graver les trois routes FCS-1 dans les tests natifs et WASM ;
- garantir la distinction byte-exacte entre corps absent et `{}` ;
- tester autorité, contexte, connecteur exact, expiry et refus voisin ;
- mettre à jour types NPM et smoke du package ;
- reconstruire `target/npm-package` seulement après gate vert.

Sortie : nouveau SHA client et hash du package communiqués au SDK.

### Phase 2 — SDK : transport et lifecycle

Propriétaire : `aithos-sdk` uniquement.

- consommer le package client livré par Phase 1 ;
- ajouter la requête signée bodyless ;
- ajouter `profileStage`, `setClientSecret` et `disconnect` avec types fermés ;
- corriger `startOAuth` et `activate` ;
- préserver `stageConnector` pour compatibilité, sans l'utiliser dans le beat ;
- ne créer aucune méthode d'approbation Gmail ;
- tester les octets envoyés et le refus d'un ancien package client.

Sortie : surface control alignée et tests SDK verts.

### Phase 3 — Core compagnon : CORS exact

Propriétaire : `aithos-core` uniquement, fichiers Gateway/Provider et tests
associés. Cette phase peut avancer en parallèle des Phases 1 et 2 après accord
sur la matrice CORS.

- écrire les scénarios Cucumber/HTTP de la matrice FCS-4 ;
- étendre la configuration existante au lieu d'introduire une seconde source
  d'origines ;
- servir les preflights exacts sur les surfaces nécessaires ;
- vérifier origine refusée, `null`, wildcard, headers supplémentaires, méthode
  voisine et absence totale de mutation ;
- ne pas élargir les routes administratives Provider étrangères à la démo.

Sortie : dashboard local utilisable sans affaiblissement global de CORS.

### Phase 4 — dashboard : un seul parcours OAuth/G4/MCP

Propriétaire : `aithos-sdk-example` uniquement.

- consommer le SDK de Phase 2 ;
- remplacer les champs manuels transaction/audience/bearer du chemin normal par
  les valeurs de l'objet OAuth ;
- conserver éventuellement un panneau diagnostic non secret, désactivé par
  défaut, mais aucun fallback manuel de succès ;
- publier le parent avec l'audience discovery byte-exacte ;
- effectuer cérémonie, échange, initialize et `tools/list` ;
- sélectionner le tool Sheets exact depuis la configuration de démo, jamais un
  nom historique codé en dur ;
- afficher résultat borné, refus voisin et preuve Gamma vérifiée ;
- verrouiller et effacer toutes les références mémoire au logout ou erreur
  terminale ;
- préserver intégralement le parcours offline déjà vert.

Sortie : parcours complet présent dans l'UI, encore validé contre doubles HTTP.

### Phase 5 — bundle de démo

Propriétaire : lead intégration ; aucun secret committé.

- fournir une config exemple sans valeur sensible et un script de préflight ;
- documenter l'injection hors dépôt de Vault et Google ;
- pré-provisionner un seul profil Sheets read-only et une seule capability ;
- utiliser un tenant, un tableur et un compte jetables ;
- ajouter probes Gateway/Vault/Provider et vérification des origins ;
- documenter restart, cleanup et comportement si un service manque ;
- ne pas dépendre des anciens processus `/tmp` ni supposer le port 4890 actif.

Sortie : environnement recréable depuis zéro par le runbook.

### Phase 6 — E2E navigateur réel

- lancer le dashboard avec un navigateur standard ;
- capturer uniquement méthodes, origines, statuts et tailles, jamais les corps
  sensibles ;
- jouer le parcours complet du §1 ;
- prouver qu'aucune sentinelle n'entre dans storage, URL, logs ou requêtes
  étrangères ;
- vérifier qu'un voisin refusé n'atteint pas Google ;
- redémarrer la Gateway puis prouver le comportement attendu de refresh ou la
  reconnexion explicitement requise ;
- produire un compte rendu avec les quatre SHA, versions et résultats.

Sortie : gate intégré vert et runbook de répétition court.

## 8. Travail en agents indépendants

La reprise peut utiliser des agents séparés, avec ownership non chevauchant :

| Agent | Ownership | Peut commencer |
| --- | --- | --- |
| A — Client | `aithos-client` | immédiatement après Phase 0 |
| B — CORS | fichiers Gateway/Provider de `aithos-core` | après accord sur la matrice CORS |
| C — SDK | `aithos-sdk` | tests/contrats immédiatement ; implémentation finale après package Client |
| D — Dashboard/intégration | `aithos-sdk-example` et runbook | UI sur doubles immédiatement ; gate réel après SDK + CORS |

Les agents ne committent jamais les fichiers d'un autre ownership. Les seuls
artefacts de synchronisation sont : SHA, hash du package client, types publics,
matrice CORS et fixture de corps exact. Le lead intègre séquentiellement Client
→ SDK → Dashboard, puis rejoue le gate cross-repo.

## 9. Définition de fini par dépôt

### `aithos-client`

- routes minimales alignées, approbations exclues ;
- bodyless byte-exact testé natif et WASM ;
- package browser sans réseau, storage ni secret ;
- tous les tests, fmt, clippy, build browser et smoke verts.

### `aithos-sdk`

- aucun `{}` envoyé sur une route bodyless ;
- méthodes lifecycle typées et bornées ;
- une seule instance OAuth relie authorization, cérémonie et exchange ;
- MCP consomme le bearer en mémoire ;
- 100 % des tests existants et nouveaux verts.

### `aithos-sdk-example`

- parcours offline préservé ;
- aucun champ manuel nécessaire au parcours intégré ;
- aucun token ou secret rendu/persisté ;
- tool Sheets réel, refus voisin et preuve visible ;
- build, lint, tests SSR/storage et E2E verts.

### `aithos-core`

- CORS exact sur les seules surfaces requises ;
- aucune mutation au preflight ;
- origine absente/refusée inchangée ;
- suites Gateway/Provider ciblées, Cucumber, fmt et clippy vertes ;
- aucune prétention OAC-7 production ajoutée par ce gate de démo.

## 10. Gates à exécuter

### Client

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-client
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo check -p aithos-client-wasm --target wasm32-unknown-unknown
./scripts/build-browser.sh
./scripts/check-native.sh
./scripts/smoke-npm.sh
./scripts/check-secrets.sh
```

### SDK

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-sdk
npm test
npm run check:architecture
```

### Dashboard

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-sdk-example
npm run lint
npm test
```

### Core compagnon

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
cargo fmt --check -p aithos-gateway -p aithos-provider
cargo clippy -p aithos-gateway -p aithos-provider --all-targets -- -D warnings
cargo test -p aithos-gateway
cargo test -p aithos-provider
```

Ajouter ensuite le gate navigateur du runbook. Un test unitaire avec un fetch
mocké ne peut pas remplacer ce dernier.

## 11. Conditions d'arrêt

STOP et demander revue si :

- une modification du wire/Core ou de la grammaire de mandat paraît nécessaire ;
- la solution requiert de persister PKCE, state ou token côté navigateur ;
- CORS exige `*`, une réflexion d'origine ou des credentials navigateur ;
- un backend proxy est proposé pour recevoir une autorité ou un token ;
- le parcours ne peut réussir qu'avec une transaction ou un bearer manuel ;
- le profil Sheets nécessite un scope ou une capability non approuvés ;
- le test live risque un effet sur des données non jetables ;
- un chantier Gmail ou OAC production entre sur le chemin critique ;
- les branches reçues contiennent des changements non attribués.

## 12. Lot optionnel après gate — email direct par MCP

Ce lot ne commence qu'après le gate Sheets vert et son commit de clôture.

Décision produit : exposer un outil MCP distinct qui envoie directement un mail
via l'API Gmail, sans workflow d'approbation Aithos. Ne pas réutiliser
`send_guarded` avec une approbation automatique et ne pas conserver des routes
owner mortes dans l'UI.

Avant code, graver un contrat séparé précisant au minimum :

- nom de capability distinct, par exemple `send_email` ;
- destinataires ou domaines explicitement autorisés par le mandat/profil ;
- limites de destinataires, sujet et corps ;
- texte brut initial, sans pièce jointe ni HTML ;
- identifiant d'opération/digest pour diagnostiquer les retries ;
- comportement fail-closed lorsqu'une réponse Gmail est ambiguë, afin de ne pas
  doubler l'envoi ;
- `message_id` comme résultat public borné et événement Gamma attribué ;
- compte et destinataire jetables pour le gate ;
- aucune conservation du contenu dans les logs ou le Provider.

Ce lot appartient principalement à `aithos-core`/Gateway. Le SDK générique sait
déjà appeler un outil MCP ; son éventuelle évolution se limite aux types ou à
l'UX, pas à une seconde implémentation Gmail.

## 13. Prompt de reprise

> Reprendre la finalisation de la démo depuis
> `docs/HANDOFF-FINALISATION-CLIENT-SDK-DEMO-INTEGREE-2026-07-22.md` dans
> `aithos-core`. Vérifier les quatre branches/HEAD du §2 et rejouer la Phase 0
> avant toute écriture. La cible est exclusivement le beat navigateur G4 +
> OAuth + Google Sheets read-only + refus voisin + preuve Gamma. Les
> approbations Gmail sont hors périmètre et aucune route `/approvals/**` ne doit
> être ajoutée au client, au SDK ou au dashboard. Utiliser les ownerships du §8
> pour paralléliser sans chevauchement. Commencer par les contrats RED : corps
> HTTP strictement vide, grammaire control minimale et matrice CORS exacte.
> Intégrer ensuite Client → SDK → Dashboard, produire le bundle de démo et
> terminer par un E2E dans un navigateur normal. Ne jamais persister ou afficher
> seed, verifier PKCE, state, bearer ou refresh token. STOP sur tout changement
> de wire/Core, wildcard CORS, proxy de secrets ou donnée Google non jetable.
> L'envoi direct Gmail du §12 est optionnel et interdit avant clôture du gate
> Sheets.

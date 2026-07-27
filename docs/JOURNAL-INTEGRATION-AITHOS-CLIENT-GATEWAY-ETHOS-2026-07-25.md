# Journal d’exécution — `aithos-client` dans la Gateway Ethos

Plan de référence :
[`PLAN-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md`](./PLAN-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md)

Date de lancement : 2026-07-25  
Règle d’avancement : passage automatique d’une gate à l’autre tant que les
invariants restent verts.  
Règle live : aucune écriture Provider réelle avant le go/no-go de la gate 8.

## Baseline

### Dépôts

| Dépôt | Branche | HEAD | Changements suivis | Non suivis |
| --- | --- | --- | ---: | ---: |
| `aithos-core` | `codex/gateway-demo-companion` | `59549d915131c4b46236f7866a39f85d9ad1bd7a` | 130 | 14 |
| `aithos-client` | `codex/gateway-demo-companion` | `77b53d3a38524817cd9faf495abce471d467cbbd` | 32 | 8 |
| `aithos-sdk` | `codex/g1-g7-enterprise-sdk` | `6d4b37bb75167469388cdec65a83fef8f93c0e01` | 10 | 5 |

Empreintes de l’état de travail au lancement :

| Dépôt | SHA-256 diff suivi | SHA-256 status porcelain |
| --- | --- | --- |
| `aithos-core` | `a657c2c1b7caad2bcf35b51c012630f8f853dafb8eb5c9a8db7e9cb324423995` | `2d767f2d78d8b93fb7f2e5b38c4dc8a6fddec85ea45a07bcd27c30b52125e981` |
| `aithos-client` | `0b3b081c15b3f5f327c6d8b11b52f5628351d63025d06791a3a816e0bcc54a04` | `c75a178ec745d96278c65054b414a3f5f1161a87d28d1861a2bc61f5db778ec8` |
| `aithos-sdk` | `51c1ece84e75e111226d5366d774549602012e93260f6bfd656ab9d3ba20e01a` | `4db5c4cc451b7443fa729f4235d35fd9769629151881c425b53a6824a552d130` |

Ces empreintes ne remplacent pas Git et n’autorisent aucun reset. Elles servent
uniquement à distinguer la baseline préexistante des changements du chantier.

### Runtime

- Gateway en écoute : `127.0.0.1:14890`.
- PID observé : `162`.
- Binaire :
  `/Volumes/Math17/aithos-runtime/demo/bin/aithos-gateway-delegated-write-eec42245`.
- SHA-256 :
  `eec422453afc1356f2f2ca814c45f986ccf9323d2c46906a1e085f73af139883`.
- Le binaire reste en place et ne sera jamais remplacé in-place.

### Outils et espace

- Rust : `rustc 1.95.0`.
- Cargo : `cargo 1.95.0`.
- Node : `v23.9.0`.
- Volume `/Volumes/Math17` : environ 171 GiB disponibles au lancement.
- Volume système : environ 8,5 GiB disponibles au lancement.
- Tous les targets lourds du chantier doivent rester sous
  `/Volumes/Math17/aithos/v2`.

## Gates

| Gate | État | Preuve / remarque |
| --- | --- | --- |
| 1 — baseline | validée | `aithos-gateway --lib` : 174/174 verts hors restriction réseau du sandbox |
| 2 — non-Ethos | validée | E2E session 1/1 + BDD 299/299 scénarios, 1422/1422 étapes |
| 3 — Cargo/keyholder | validée | une seule instance core/bundle ; capability client bornée et testée |
| 4 — seam legacy | validée | 176/176 unitaires, E2E Ethos 2/2, non-Ethos 1/1, BDD 299/299 |
| 5 — transport Provider | validée | wire fermé, borné et capturé byte-exactement en loopback |
| 6 — lectures shadow | validée | shadow non bloquant + client lecture 66/66 scénarios |
| 7 — mutations dry-run | validée | plans public/circle + enveloppes, zéro réseau |
| 8 — E2E Provider isolé | validée | publication réelle en mémoire + cold verify + lecture sémantique |
| 9 — canari Gateway | validée | working set borné + create/edit/delete réels + relecture `/heads` |
| 10 — déploiement/rollback | prête pour activation | candidat immuable installé ; ancien runtime toujours actif |

## Événements

### Lancement

- Le chantier démarre sans nettoyage, reset, stash ou commit des changements
  préexistants.
- Les tests et builds utilisent un target dédié sur le volume externe.
- Les connecteurs non-Ethos constituent l’invariant d’arrêt prioritaire.

### Gate 1 — baseline

- Compilation et exécution initiales dans le sandbox : 163 tests verts et
  11 refus `Operation not permitted` lors de l’ouverture de sockets locales.
- Relance byte-identique hors restriction réseau du sandbox :
  `cargo test -p aithos-gateway --lib`.
- Résultat : **174 tests passés, 0 échec**.
- Les deux tests de transport MCP HTTP/SSE, les tests de credentials Vault et
  le test d’admission distante précédemment bloqués par le sandbox sont verts.
- Conclusion : la baseline applicative est saine ; les refus initiaux étaient
  exclusivement environnementaux.

### Gate 2 — caractérisation non-Ethos

- `e2e_delegated_session` : **1/1 test vert**. La preuve couvre notamment :
  surface filtrée par session, refus avant upstream, journalisation avant
  effet, relais unique de l’outil autorisé et coupure à chaud après révocation.
- Suite BDD Gateway complète : **18 features, 76 rules, 299 scénarios et
  1422 étapes, tous verts**.
- Cette suite couvre les connecteurs statiques et dynamiques, les credentials
  Vault, l’OAuth upstream, l’activation à chaud, la dérive de manifeste et
  l’isolation d’une panne à son seul connecteur.
- Invariant figé : les futurs changements Ethos ne doivent modifier ni la
  résolution, ni les corps relayés, ni les compteurs d’appel de ces routes.

### Gate 3 — Cargo et custody

- `aithos-client 0.1.0-alpha.2-dev` est une dépendance locale explicite de la
  Gateway.
- `cargo tree -i` prouve une instance unique de `aithos-core
  0.1.0-alpha.1` et `aithos-bundle 0.1.0-alpha.1`, partagée par Gateway,
  Provider et client.
- La Gateway ne publie aucun seed : `Keyholder::with_ethos_client_grantee`
  crée une capability temporaire, limitée à une closure et aux opérations
  scellées de `aithos-client`.
- Test de custody : la clé publique de cette capability est exactement celle
  de l’agent Gateway ; le `Debug` reste `Keyholder(<sealed>)`.
- Suite `aithos-client --lib` : **4/4 tests verts**.

### Gate 4 — seam legacy

- Ajout d’un `EthosBackend` attaché au routeur MCP.
- La sélection déléguée est une allowlist exacte des six outils natifs ; la
  sélection legacy reste volontairement limitée aux trois lectures pour
  conserver ses refus historiques.
- Tous les routeurs existants utilisent explicitement `Legacy`, qui constitue
  le rollback immédiat.
- Preuves après refactor :
  - Gateway unitaires : **176/176** ;
  - E2E Ethos délégué : **2/2** ;
  - E2E connecteur délégué non-Ethos : **1/1** ;
  - BDD complète : **299/299 scénarios, 1422/1422 étapes**.

### Gate 5 — transport Provider signé

- Transport Rust fermé : HTTPS obligatoire hors loopback exact, credentials
  URL/query/fragment/path de base refusés, redirections désactivées, timeouts
  et réponse maximale bornés.
- Seuls `GET`, `POST` et `PUT` sur `/t/**` issus d’un plan signé sont admis.
- `X-Aithos-Store`, `X-Aithos-Auth`, `If-Head` et le corps opaque sont
  transmis sans réinterprétation.
- Test de capture loopback : méthode, enveloppe, CAS et bytes sont exacts.
- Aucun backend de production n’utilise encore ce transport à cette gate.

### Gate 6 — lectures shadow

- Mode `ClientShadow` ajouté uniquement au harness E2E Ethos. Il tente une
  vérification/lecture indépendante puis sert toujours les bytes legacy.
- Une erreur ou divergence shadow est un événement structuré fixe et ne peut
  ni changer la réponse, ni toucher un connecteur voisin.
- Le fixture Gateway historique conserve des chemins publics pré-K1C dans
  son manifeste : `aithos-client` le refuse comme snapshot incomplet, ce qui
  confirme que le fallback non bloquant est nécessaire pendant la migration.
- Correction TDD bornée dans `aithos-client` : un grantee demandant `self`
  reçoit à nouveau `ZoneNotAllowed` avant résolution de chemin. L’invariant
  owner-only ne change pas.
- Preuves client : **66/66 scénarios, 314/314 étapes** ; lecture ciblée :
  **7/7 tests**. E2E Gateway shadow : **2/2**.

### Gate 7 — mutations dry-run

- Sous la vraie custody Gateway, `aithos-client` construit un grant circle,
  une création déléguée, cold-vérifie le résultat et génère une enveloppe
  Provider pour chaque delta, sans aucune requête réseau.
- Preuves ciblées :
  - dry-run Gateway/custody : **1/1** ;
  - mutation publique déléguée : **1/1** ;
  - mutations de zones : **2/2** ;
  - enveloppes Provider : **1/1**.

### Gate 8 — E2E Provider isolé

- Un Provider réel en mémoire est démarré sur loopback ; aucun hôte externe ni
  Ethos de démo n’est contacté.
- Le test publie successivement un genesis, un mandat circle, puis une
  mutation déléguée produite par `aithos-client`, via le transport fermé de la
  Gateway.
- Le résultat est retéléchargé sous autorité propriétaire, cold-vérifié puis
  ouvert : la section circle contient exactement le texte signé.
- Premier RED observé : le genesis Client envoyait un changeset avant
  `did.json`, ce qui rendait la clé racine inconnue du Provider
  (`403 chain_invalid`).
- Correction TDD : `did.json` est maintenant le premier objet du
  `GenesisPlan::upload_order`, `manifest.json` reste le dernier. Le test
  unitaire `genesis_planning` est passé de rouge à vert.
- Preuves :
  - E2E historique Gateway/Provider : **1/1** ;
  - E2E Client/transport Gateway/Provider : **1/1** ;
  - ordre du genesis Client : **1/1**.

### Gate 9 — working set mandaté et canari Gateway

- Le conflit constaté à la gate 8 a été résolu sans élargir la couverture
  Provider et sans affaiblir `VerifiedSnapshot`.
- `aithos-client` expose un `VerifiedWorkingSet` distinct et borné :
  - create `circle` ne télécharge aucun blob de zone ;
  - edit/delete téléchargent exactement le blob cible ;
  - les preuves communes, la queue Gamma et la ligne Header du grantee sont
    vérifiées ;
  - une ligne Header destinée à un bénéficiaire voisin est refusée ;
  - tout payload `self` est refusé dans ce working set.
- La Gateway dérive une capability Client temporaire depuis la clé de feuille
  de la session. Aucune seed, opération de signature générique ou cache de
  contenu clair n’est exposé.
- `ClientProvider` intercepte uniquement les mutations simples `circle` sur
  un contexte Provider-primary. Les zones `public`/`self`, les requêtes riches
  non encore couvertes et les stores locaux suivent le backend legacy.
- Le transport publie le delta dans l’ordre, engage `manifest.json` en dernier,
  puis relit `/heads`. Après erreur ou conflit, il ne déclare un succès que si
  le nouveau head attendu est réellement visible ; aucun fallback writer
  legacy n’est exécuté.
- Le mutex du runner est libéré avant les appels réseau Provider afin de ne pas
  bloquer les autres connecteurs pendant une mutation Ethos.
- Activation fermée :
  - variable absente ou `legacy` : comportement historique ;
  - `shadow` : lecture comparative non bloquante ;
  - `client-provider` : canari mutation `circle` ;
  - valeur inconnue : refus de démarrage.
- Preuves fonctionnelles :
  - working sets Client : **5/5** ;
  - scénarios Client phase E : **49/49, 255/255 étapes** ;
  - E2E Gateway/Client/Provider : **2/2**, dont create/edit/delete sous session
    Gateway avec lecture propriétaire du résultat ;
  - E2E historique Provider : **1/1** ;
  - E2E Ethos local/shadow : **2/2** ;
  - E2E session non-Ethos avec `ClientProvider` actif : **1/1**.
- Preuves de non-régression après formatage :
  - Core carriers : **3/3** ;
  - Bundle publication/concurrence : **8/8** ;
  - Gateway unitaires : **181/181** ;
  - BDD Gateway complète : **18 features, 76 rules, 299/299 scénarios,
    1422/1422 étapes**.
- Limite volontaire : l’écriture publique déléguée reste sur le refus
  historique. Elle exige le E2E produit séparé prévu par D5.

### Gate 10 — préparation release

- Release terminée en **3 min 48 s**.
- Candidat construit :
  `/Volumes/Math17/aithos/v2/.cargo-target-ethos-client-gateway/release/aithos-gateway`.
- SHA-256 :
  `cc596b68905cf86bb60c6f0c1944e961c2b7b2ae3b64b4039b5d242c28393ed5`.
- Candidat installé sans remplacement sous :
  `/Volumes/Math17/aithos-runtime/demo/bin/aithos-gateway-ethos-client-cc596b68`.
- Le hash du fichier installé est identique au candidat.
- Le processus de démonstration en écoute n’a pas été arrêté, remplacé ou
  modifié : PID `162`, port `127.0.0.1:14890`, toujours sur
  `aithos-gateway-delegated-write-eec42245`.
- La compilation release utilise
  `/Volumes/Math17/aithos/v2/.cargo-target-ethos-client-gateway` avec
  `CARGO_INCREMENTAL=0`.
- Le nouveau binaire démarre en `legacy` en l’absence d’activation explicite.
- Activation canari prévue :
  `AITHOS_ETHOS_BACKEND=client-provider`.
- Rollback prévu : relancer le binaire précédent, ou lancer le nouveau sans
  cette variable ; dans les deux cas le backend Ethos redevient `legacy`.

### Tentative d’activation et rollback

- L’ancien PID `162` a été arrêté proprement.
- Une première tentative détachée du candidat a été nettoyée par
  l’environnement d’exécution Codex avant l’ouverture du port ; aucun message
  applicatif ni mutation Provider n’a été produit.
- Le candidat a ensuite été lancé dans une session persistante. Son
  initialisation n’a ouvert le port qu’après une attente silencieuse d’environ
  une minute.
- Par prudence, la tentative a été interrompue dès l’annonce du listener et le
  rollback vers le binaire précédent a été exécuté.
- L’ancien binaire a présenté le même délai d’initialisation, ce qui localise
  l’attente dans la restauration du runtime partagé plutôt que dans le nouveau
  backend.
- État final vérifié :
  - PID `16323` ;
  - ancien binaire `aithos-gateway-delegated-write-eec42245` ;
  - écoute `127.0.0.1:14890` ;
  - discovery locale et publique identiques, resource
    `https://demo.mcp.aithos.fr/mcp`.
- Aucun test manuel Cowork n’a donc encore été effectué avec le candidat
  `client-provider`. Cette validation reste le prochain jalon.

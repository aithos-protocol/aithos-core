# Session G1 + G7 entreprise — 2026-07-21

Statut : `G1c COMPLETE`, prêt pour `G7a`.

Ce journal est la trace d'intégration locale de la verticale définie par
`HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md`. Il ne publie,
ne pousse, ne fusionne et ne déploie rien.

## Garde-fous actifs

- Un seul lead intégrateur intervient dans `aithos-gateway`.
- Aucun sous-agent n'a lu, modifié ou testé `aithos-gateway` en parallèle du
  lead pendant A0.
- L'ordre du §8 est bloquant : A0, A1, G1a, G1b, G1c, G7a, G7b, SDK,
  dashboard, beat navigateur x2, témoin adversarial, clôture.
- Les contrats Gherkin doivent être observés RED puis recevoir un commit étroit
  avant toute implémentation associée.
- OAuth amont, JCS, signatures et autorité restent dans les briques Rust
  existantes ; aucune réimplémentation TypeScript n'est autorisée.
- La portée exclut les MCP arbitraires, GSE/Gmail send, toute nouvelle grammaire
  protocolaire, AWS/prod, publication, push et merge.
- Tout conflit de propriété non résoluble ou décision hors §4 impose un STOP
  documenté.

## Références obligatoires lues intégralement avant action

- `docs/HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md`
- `docs/INFRA-PROVIDER.md`
- `docs/HANDOFF-GATEWAY-HUB.md`
- `docs/HANDOFF-PROVIDER-P7B-BASCULE-RELAY-DONE-2026-07-20.md`
- `docs/HANDOFF-GATEWAY-OAUTH-AMONT-VM-2026-07-21.md`
- `docs/GATEWAY-UPSTREAM-OAUTH-VM.md`
- `docs/HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md`
- `docs/SDK-V0-CONTRACT.md`
- `docs/HANDOFF-CLIENT-SDK-DEMO-V2-2026-07-21.md`
- `docs/README.md`, `docs/EXECUTION-PLAN.md`,
  `docs/RELEASE-BOUNDARY.md` dans `aithos-client`
- Les références de code provider exigées : `pod_stub`, `tunnel`,
  `passthrough`, `acme`, `keepalive`, vecteur P3, générateur et README des
  vecteurs.
- Les références de code gateway exigées : `config`, `main`, `proxy`, `oauth`,
  `upstream_oauth`, `credentials`, `store_adapter`, `core_bridge` et les 15
  features Gherkin existantes.

Aucun `AGENTS.md` n'est présent dans les quatre worktrees audités.

## Vérité disque et attribution A0

### `aithos-core`

- Branche au début : `codex/publish-aithos-core-busl`.
- HEAD au début : `1f48cb4` (`gateway: add upstream OAuth custody for modern MCP`).
- Un seul worktree Git détecté.
- Index initial vide.
- Les changements CLI/custody préexistants sont étrangers à G1/G7 et restent
  volontairement non indexés :
  - `docker/Dockerfile`
  - `rust/Cargo.lock`
  - `rust/Cargo.toml`
  - `rust/crates/aithos-cli/Cargo.toml`
  - `rust/crates/aithos-cli/src/main.rs`
  - `rust/crates/aithos-cli/tests/cli_surface.rs`
  - `rust/crates/aithos-cli/src/custody.rs` (non suivi)
  - `docs/CLI-INSTALL-VAULT.md` (non suivi)
- Les répertoires et documents non suivis hérités suivants restent étrangers et
  non indexés : `_gitjunk/`, `_to_delete/`, `_transfer/`, les guides de démo
  gateway, les documents Gmail/GSE et les handoffs G1/G7/OAuth reçus.
- Le répertoire `rust/crates/aithos-gateway` était propre au début de A0.
- Les prérequis client/SDK hérités, déjà présents sur disque et séparables des
  changements étrangers, ont été garés sans modification fonctionnelle dans
  quatre commits étroits :
  - `c073d65 chore(core): park inherited K1-C client prerequisites`
  - `a15c0f6 chore(sdk): park inherited verified upload plan`
  - `53f5877 chore(provider): park inherited anonymous SDK reader`
  - `e7e280f docs(sdk): preserve inherited v0 demo contract`
- HEAD après parking : `e7e280faca5be03bb3a5724176348b2bd5a19032`.
- Un ancien `.git/objects/maintenance.lock`, daté du 2026-07-11, est présent ;
  aucun détenteur n'a été observé par `lsof`. Il n'a pas été supprimé.

### `aithos-client`

- État initial : branche `main`, HEAD `19dcb43`, 13 fichiers suivis modifiés et
  14 chemins non suivis, mélange de travaux hérités.
- Avec l'autorisation explicite d'intervenir dans tous les dépôts V2, les
  ensembles attribuables ont été séparés et garés sur
  `codex/client-sdk-v2-parking` :
  - `8b6afce chore(browser): park grantee identity and recovery`
  - `04ac636 docs: park client SDK demo handoff`
  - `8b61abb test(client): park phase E contracts`
  - `e05b504 wip(client): park genesis publication mandates and provider envelope`
  - `169eac4 wip(browser): park publication bindings`
- Les deux commits `wip` et les features `@wip` sont explicitement des travaux
  incomplets, pas une preuve GREEN.
- HEAD A0 : `169eac4449c60e130c487a6fd38b486f54b7059e` ; worktree propre.

### `aithos-sdk`

- Le dossier n'était pas un dépôt autonome et apparaissait comme contenu non
  suivi du dépôt parent `/Volumes/Math17/aithos` ; ce parent n'est pas une base
  d'intégration sûre pour G1/G7.
- Après autorisation explicite, un dépôt local autonome sans remote a été créé.
- Commits de conservation :
  - `d06ee96 chore(sdk): establish local repository hygiene`
  - `69c9804 chore(sdk): preserve inherited v2 baseline`
- Branche A0 : `codex/g1-g7-enterprise-sdk`.
- HEAD A0 : `69c9804bdf966b282c1595a3bc4ac004ff4dbb34` ; worktree propre.
- Le package reste privé et n'a aucune configuration de publication.

### `aithos-sdk-example`

- Le dépôt existait avec une branche `main` non née : 30 fichiers source non
  suivis, aucun commit, aucun remote.
- Les 30 fichiers ont été conservés tels quels dans
  `fbcefa7 chore(dashboard): preserve inherited SDK console baseline`.
- Branche A0 : `codex/g1-g7-enterprise-dashboard`.
- HEAD A0 : `fbcefa76222c0cc772d9d8fdc9846f29f89c00f2` ; worktree propre.
- `node_modules`, `dist` et `.wrangler` étaient déjà ignorés et n'ont pas été
  indexés ni supprimés.
- `.openai/hosting.json` a déclenché la lecture des règles Sites. La portée
  utilisateur demeure strictement locale : aucune opération d'hébergement ou
  de déploiement n'est autorisée.

## Baseline vérifiée

### Core/provider/gateway

- `cargo test -p aithos-bundle --test cb12_publication_package` : 5/5.
- Test ciblé provider `public_remote_store` : 1/1. Le premier essai a été bloqué
  par le sandbox sur l'ouverture d'un socket ; le même test hors sandbox a
  réussi.
- `cargo fmt --check` ciblé sur les fichiers de prérequis : réussi.
- Clippy ciblé `aithos-core`, `aithos-bundle`, puis provider lib et test
  `public_remote_store` avec `-D warnings` : réussi.
- Clippy provider `--all-targets` signale deux avertissements préexistants dans
  `src/bin/relay.rs` et `src/bin/store_admin.rs`; aucun n'a été masqué ou corrigé
  pendant A0.
- Suite complète `cargo test -p aithos-gateway` : réussie hors sandbox, avec 99
  tests unitaires, 159 scénarios/818 étapes Cucumber et toutes les suites E2E et
  documentaires du crate.
- Le premier essai gateway dans une cible Cargo neuve a saturé le volume. Seule
  cette cible temporaire créée par la session a été supprimée après validation ;
  aucun artefact hérité n'a été effacé.

### Client

- Tests natifs ciblés : 7/7.
- Test browser publication ciblé : 1/1.
- `cargo clippy -p aithos-client --all-targets -- -D warnings` : réussi.
- Suite complète `cargo test -p aithos-client` : réussie ; 38 scénarios Gherkin
  réussis et 26 scénarios `@wip` ignorés.
- Clippy WASM tous targets : réussi.
- Suite complète `cargo test -p aithos-client-wasm` : réussie, dont 19 scénarios
  Cucumber/117 étapes.
- Rustfmt ciblé et vérification syntaxique Node : réussis.
- Le build browser destructif de `target/npm-package`, consommé par le SDK, n'a
  pas été lancé pendant l'audit.

### SDK et dashboard

- SDK : `node --test`, 3/3 ; vérification syntaxique Node réussie.
- Dashboard : test source `tests/sdk-console-source.test.mjs`, 1/1.
- Le test `rendered-html.test.mjs` du dashboard est un squelette hérité obsolète
  et ne constitue pas une preuve du dashboard G1/G7.

## Risques adversariaux ouverts à l'issue de A0

Ces constats sont des entrées obligatoires pour les lots ultérieurs ; aucun
n'est présenté comme résolu :

- Le beat est impossible avec l'API actuelle : le client ne sait émettre que
  `PublicMandateIntent::edit_section`; le SDK n'expose ni client gateway, ni
  descripteurs, ni OAuth/MCP.
- Après `BrowserRuntime::reset`, les identifiants de handles repartent à zéro et
  un ancien handle JavaScript peut aliaser une nouvelle autorité.
- Les clés privées bootstrap owner/succession n'ont pas de primitive locale de
  verrouillage/destruction hors reset global.
- L'en-tête provider `If-Head` n'est pas dans l'enveloppe JCS signée.
- Les 26 scénarios phase E restent `@wip` et exclus.
- La réutilisation de `PublicationEntropy` n'est pas refusée ; la création
  déléguée et la précision de certaines erreurs d'autorité restent incomplètes.
- Le plan SDK peut dupliquer le manifeste, écraser silencieusement des chemins
  dupliqués, et `plan.verify()` crée un handle de snapshot ignoré/non libéré.
- Les tests SDK utilisent un provider factice, sans relecture froide de preuve
  finale ; les URL `http` sont actuellement acceptées.
- Certaines graines de récupération deviennent des chaînes JavaScript
  immuables ; le contrat de consommation/zeroization des buffers n'est pas
  public et les longueurs invalides ne sont pas toutes nettoyées.
- Les contrôles d'architecture TypeScript et les métadonnées npm restent trop
  faibles pour servir de preuve finale.

Constat négatif vérifié : le SDK TypeScript existant ne réimplémente aujourd'hui
ni OAuth, ni JCS, ni signature, ni autorité ; il délègue au WASM. Aucun token
codé en dur n'a été trouvé pendant l'audit.

## Verdict A0

- Propriété des changements connue ou conservée comme étrangère.
- Quatre dépôts disposent désormais d'un point de reprise local explicite.
- Aucun push, merge, déploiement ou publication effectué.
- Espace disque observé en fin d'audit : environ 1,8 Gio ; les lots suivants
  doivent réutiliser les cibles existantes et surveiller le volume.
- A1 peut commencer. Aucune implémentation G1/G7 ne doit précéder son contrat
  RED commité.

## A1 — contrats gelés

- `965508d test(g1-g7): freeze enterprise gateway contracts` a ajouté les
  contrats relay, control et connecteurs : 73 scénarios. Le mode temporaire
  `fail_on_skipped` a produit le RED attendu avant implémentation.
- `d369265 test(g1): classify relay contract slices` a fixé les tranches
  ordonnées `@g1a`, `@g1b`, `@g1c`.
- `a4164ab test(g1): bind registration outline examples` a rendu les exemples
  de refus B.2 exécutables sans modifier le contrat.
- Le contrat SDK a été gelé séparément dans `aithos-sdk` par
  `2aba987 test(sdk): freeze gateway control surface` et observé RED sur
  l'absence de `GatewayControlClient`. Aucun lot SDK n'a été implémenté avant
  G7.

## G1a — tunnel sortant

- Commits étroits :
  - `6f15fe0 feat(gateway): add outbound relay client`
  - `94fcffe fix(gateway): enable keepalive retry support`
- Le client Rust utilise TLS avec racines système, SNI et ALPN
  `aithos-tunnel/1`, la ligne B.2 JCS/Ed25519 via le bridge Rust borné, yamux en
  mode serveur, keepalive et reconnexion exponentielle jitterée.
- Les refus provider arrivent avant yamux ; remplacement, GoAway, fraîcheur du
  nonce et isolation concurrente ont été vérifiés sur le provider réel
  in-process.
- Gate : 13 scénarios/52 étapes `@g1a`, tests provider P3/tunnel, suite gateway,
  clippy et fmt réussis.

## G1b — TLS publique et ACME client

- Commit étroit : `c129d43 feat(gateway): terminate public TLS and manage ACME`.
- TLS publique est terminée après yamux dans le gateway ; le relay ne voit que
  les enregistrements chiffrés. Les PEM explicites et le cache ACME exigent des
  modes privés, refusent les symlinks et valident clé, SAN, chaîne et validité.
- Le cache publie une génération complète par pointeur atomique. Un renouvellement
  échoué conserve seulement un certificat encore valide.
- ACME DNS-01 utilise `instant-acme`, la B.5 existante et le bridge Rust borné ;
  aucun OAuth, JCS, signature ou modèle d'autorité n'a été réimplémenté en
  TypeScript.
- Gate : 4 scénarios/18 étapes `@g1b`, vecteurs P6 PUT/DELETE, provider
  `acme_replay`, 112 tests unitaires gateway, tests d'opacité réels, clippy, fmt
  et scan anti-fuite réussis.

## G1c — routeur unique direct + relay

- RED ciblé observé avant code : 3 scénarios `@g1c` et leurs 14 étapes étaient
  indéfinis ; les contrats étaient déjà commités par A1.
- Le binaire conserve une seule identité `Arc<Keyholder>` sans exposer de
  signer générique, construit une seule application Axum et clone ce même
  `Router` pour l'écoute directe et le `RelayApplicationListener`.
- Le listener relay reçoit, via une file bornée, des flux yamux dont la TLS
  publique a déjà été terminée côté gateway. Le nombre de traitements de flux
  simultanés est borné à 64 ; surcharge et fermeture échouent fermées.
- L'écoute directe est liée avant le superviseur relay. Une panne PEM, ACME,
  racines TLS, dial, reconnexion ou relay ne termine pas le serveur direct.
  L'ACME initial s'exécute hors de ce chemin et les renouvellements réussis
  remplacent à chaud la configuration rustls.
- La preuve d'intégration `public_tls_relay` traverse le vrai vérificateur
  provider, B.2, TLS tunnel, yamux, TLS publique et le listener Axum. Elle sert
  `/mcp`, `/oauth/callback` et `/control/v1/status`, exécute le registre OAuth
  amont existant, garde verifier et jetons dans le Vault de test, appelle le MCP
  protégé avec le bearer résolu et vérifie la capture chiffrée sans sentinel.
- Gate G1 complet :
  - `@g1a or @g1b or @g1c` : 20 scénarios/84 étapes réussis ;
  - `public_tls_relay` : 2/2 ; `relay_client` : 3/3 ;
    `acme_txt_client` : 1/1 ;
  - lib gateway : 113/113 ;
  - `cargo check --all-targets`, clippy `--all-targets -D warnings`, fmt et
    `git diff --check` réussis ;
  - scan des nouveaux logs production : uniquement quatre messages constants,
    aucun token, secret, verifier, clé ou détail de dial.
- Une cible Cargo G1c neuve a rempli `/tmp` avant liaison Cucumber. Seul le
  répertoire `/tmp/aithos-core-gateway-g1c-target`, créé par cette session, a
  été supprimé ; les tests ont ensuite réutilisé `rust/target`. Aucun artefact
  ou changement étranger n'a été supprimé.
- Aucun autre agent n'est intervenu dans `aithos-gateway` pendant G1a, G1b ou
  G1c. Aucun push, merge, déploiement ou publication n'a été effectué.

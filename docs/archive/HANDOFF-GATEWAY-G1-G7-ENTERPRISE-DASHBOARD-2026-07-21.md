# HANDOFF — G1 + G7 : dashboard entreprise, relay sortant et OAuth MCP amont

> **ARCHIVE — plan partiellement absorbé.** Le relay/OAuth amont et les profils
> SaaS ont depuis leur implémentation et leur runbook dédiés. Les sujets UI encore
> ouverts ne doivent pas être repris depuis cette baseline.

Date : 2026-07-21

Statut : **plan historique partiellement livré**. Les contrats gateway G1/G7 ont
été figés dans `965508d` puis le registre de connecteurs et l'activation à chaud
ont été livrés dans `af42a6f`. La tranche client/SDK/dashboard reste un chantier
séparé ; ne pas relancer ce document depuis l'étape 1. Utiliser le handoff
client/SDK du 2026-07-22 pour cette reprise.

Branche `aithos-core` observée : `codex/publish-aithos-core-busl`.

HEAD d'entrée observé : `1f48cb447b1d27ebbcdac4fb063cb3712c7c3b1c`
(`gateway: add upstream OAuth custody for modern MCP`).

Ce document remplace le raccourci « G1 + G7 suffisent » par la verticale exacte
qui doit être livrée. Le G7 historique ne couvrait qu'une surface de preuve en
lecture. La démonstration demandée exige aussi une surface owner bornée, un
registre runtime de connecteurs, l'orchestration du client OAuth amont déjà
implémenté et la tranche SDK/dashboard qui les consomme.

## 0. Verdict produit et beat final

La démo entreprise visée est :

1. l'utilisateur ouvre le dashboard servi depuis `app.aithos.fr` ;
2. le dashboard constate qu'une gateway cliente est joignable ;
3. l'utilisateur choisit un connecteur MCP **déjà approuvé** par son entreprise ;
4. il attache son instance (endpoint et coordonnées OAuth non secrètes) ;
5. le client secret traverse un TLS de bout en bout vers la gateway et est écrit
   immédiatement dans le Vault du client ;
6. la gateway lance Authorization Code + PKCE ;
7. le navigateur consent chez le fournisseur ;
8. le callback public revient à la gateway à travers le relay ;
9. les tokens sont échangés et conservés par la gateway dans le Vault ;
10. la gateway découvre `tools/list` avec le bearer, compare la surface live au
    manifest approuvé et refuse tout drift ;
11. le connecteur devient actif sans redémarrage ;
12. le dashboard montre son état, exécute au moins un appel sûr déjà mandaté,
    montre un refus voisin, puis vérifie la preuve Gamma sans secret.

La démo de développement doit pouvoir tourner avec gateway + Vault **sur le
poste local**, derrière le même tunnel sortant G1. La VM n'est pas un prérequis :
elle remplace ensuite le poste comme lieu d'exécution, sans changer le protocole,
le hostname ni le dashboard.

« Ajouter un connecteur » signifie dans cette verticale : **attacher une instance
d'un connecteur dont le manifest/catalogue et les capabilities ont déjà été
approuvés**. L'introduction d'un MCP arbitraire et la frappe d'une nouvelle
politique owner depuis le dashboard constituent un lot ultérieur. Ne jamais les
simuler ou les présenter comme livrés.

## 1. État réel à l'entrée

### `aithos-core`

- Le relay provider est déployé et son passthrough TCP/SNI a été prouvé.
- `aithos-provider/src/bin/pod_stub.rs` est la référence exécutable de la moitié
  cliente B.2/B.3/B.5 : TLS sortant, enregistrement signé, yamux serveur, TLS
  public terminé côté client et ACME DNS-01 délégué.
- G1 n'est pas implémenté dans `aithos-gateway`.
- G7 n'est pas implémenté.
- G2/G3/G6 sont clos : MCP Streamable HTTP, AS OAuth entrant de la gateway et
  outils Ethos en lecture existent.
- Le client OAuth **amont** générique est livré au commit d'entrée : PKCE S256,
  state, callback `/oauth/callback`, refresh, tokens dans Vault et refus
  fail-closed. La commande actuelle est `owner-connect-oauth`.
- La configuration des serveurs est encore statique dans le YAML. Le dashboard
  ne dispose d'aucune API pour attacher ou activer un serveur.
- Le worktree est sale et contient plusieurs chantiers étrangers. État disque =
  vérité ; ne rien restaurer, stasher, formater ou committer globalement.

### `aithos-client`

Chemin : `/Volumes/Math17/aithos/v2/code/aithos-client`.

HEAD observé : `19dcb436ec4efbd153b4018bd81146658303e9e4` sur `main`.

Le chantier client/SDK v2 est actif et le worktree est sale. Lire intégralement
`docs/HANDOFF-CLIENT-SDK-DEMO-V2-2026-07-21.md`. Réutiliser son enveloppe signée
offline et ses handles opaques ; ne jamais reconstruire une signature, un mandat
ou une règle de protocole en TypeScript.

### SDK et dashboard

- SDK v2 local : `/Volumes/Math17/aithos/v2/code/aithos-sdk`.
- Console réelle en cours : `/Volumes/Math17/aithos/v2/code/aithos-sdk-example`.
  Ce dépôt n'avait encore aucun commit à l'observation ; tous ses fichiers sont
  donc à attribuer avant écriture.
- Le prototype visuel `v2/apps/dashboard/living-product` est une référence UX,
  pas un oracle de protocole et pas la cible réseau.
- La console réelle affiche aujourd'hui `Gateway: Reporté`.

## 2. Références obligatoires

Le lead lit intégralement, dans cet ordre, avant tout contrat ou code :

1. ce handoff ;
2. `docs/INFRA-PROVIDER.md`, au minimum doctrine, sections 3, 5, 6, annexe A et
   annexe B — si le document a changé, relire intégralement les sections liées ;
3. `docs/HANDOFF-GATEWAY-HUB.md` ;
4. `docs/HANDOFF-PROVIDER-P7B-BASCULE-RELAY-DONE-2026-07-20.md` ;
5. `docs/HANDOFF-GATEWAY-OAUTH-AMONT-VM-2026-07-21.md` et
   `docs/GATEWAY-UPSTREAM-OAUTH-VM.md` ;
6. `docs/HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md` ;
7. `docs/SDK-V0-CONTRACT.md` ;
8. côté client, `docs/HANDOFF-CLIENT-SDK-DEMO-V2-2026-07-21.md`,
   `README.md`, `docs/EXECUTION-PLAN.md` et `docs/RELEASE-BOUNDARY.md` ;
9. le code de référence `aithos-provider/src/bin/pod_stub.rs`, puis
   `tunnel.rs`, `passthrough.rs`, `acme.rs`, `keepalive.rs` et les vecteurs p3 ;
10. côté gateway : `config.rs`, `main.rs`, `proxy_mcp.rs`, `oauth.rs`,
    `upstream_oauth.rs`, `credentials.rs`, `store_adapter.rs`, `core_bridge.rs`
    et les features associées.

Une modification normative de C2/A.2, du Core, d'un vecteur ou de la grammaire
de mandat est une extension de périmètre : STOP. Le présent lot doit consommer
les contrats existants.

## 3. Architecture opposable

```text
app.aithos.fr (fichiers statiques)
        |
        | fetch HTTPS cross-origin, requête owner/auditeur signée
        v
<org>.mcp.aithos.fr:443
        |
        | NLB + relay TCP/SNI opaque ; TLS public non terminé chez Aithos
        v
tunnel TLS sortant ouvert par la gateway (yamux)
        |
        | TLS public terminé dans l'infra cliente
        v
aithos-gateway ----- Vault KV v2 client
        |
        +----------- MCP OAuth amont

RemoteStore Aithos <----- ciphertext, certs et preuves signées -----> navigateur
```

Règles :

- le backend du dashboard ne proxifie jamais un appel owner vers la gateway ;
- la page téléchargée parle directement au hostname de la gateway ;
- le relay Aithos ne voit aucun octet HTTP, token, secret ou payload MCP ;
- la clé TLS publique reste chez le client ;
- la clé owner/config/auditeur reste dans un handle local opaque ;
- Vault et les MCP amont ne sont jamais joignables depuis le navigateur ;
- le dashboard ne reçoit jamais access token, refresh token, client secret
  stocké, Vault token ou référence interne inutile ;
- les pages d'historique et de preuve utilisent le RemoteStore quand la gateway
  est hors ligne ; l'API live ne devient pas une seconde vérité.

## 4. Décisions prises pour permettre l'autonomie

Ces décisions ne doivent pas être redemandées à Mathieu pendant son absence.

1. **Gate principal local derrière relay.** Gateway et Vault tournent localement ;
   un relay/control plane in-process sert les tests. La VM reste un runbook et un
   gate manuel ultérieur.
2. **Relay opt-in.** Sans stanza `relay`, le comportement actuel reste
   byte-identique. Le listener loopback direct reste disponible.
3. **Un seul routeur applicatif.** Direct, relay et tests servent le même
   `axum::Router`. Aucun fork de logique HTTP.
4. **Surface owner signée, pas de cookie ni OAuth Aithos.** Les routes `/control`
   consomment l'enveloppe A.2 signée par l'autorité présentée. Le G3 OAuth entrant
   protège les consommateurs MCP ; il ne donne aucune autorité owner au dashboard.
5. **CORS exact.** Seuls les origins explicitement configurés, par défaut
   `https://app.aithos.fr`, sont admis. Jamais `*`, jamais réflexion aveugle de
   `Origin`, jamais credentials navigateur implicites.
6. **Connecteurs approuvés seulement.** Le runtime ne peut attacher qu'un serveur
   dont le manifest approuvé est déjà scellé sous `/x/<server>` dans le contexte.
7. **Registry local.** Les bindings runtime sans secret vivent dans un record
   versionné `gateway/connectors.json` du sidecar client. `gateway/**` ne part
   jamais au RemoteStore. Écriture atomique et validation complète avant swap.
8. **Activation à chaud.** Le router dérive la surface du manifest approuvé et du
   registry actif. Aucun YAML réécrit, aucun restart requis.
9. **Secret éphémère navigateur.** Le formulaire peut transmettre un client
   secret directement à la gateway sous TLS public de bout en bout. Le champ est
   `autocomplete=off`, jamais persisté, jamais relu, immédiatement zeroized côté
   Rust après écriture Vault. Une version future pourra remplacer ce seam par un
   signer/companion local sans changer l'API de haut niveau.
10. **OAuth amont existant, une seule implémentation.** G7 orchestre
    `UpstreamOAuthRegistry`; il ne réimplémente ni PKCE, ni refresh, ni callback.
11. **Discovery après connexion.** Pour un MCP OAuth, `tools/list` live est appelé
    uniquement après obtention du bearer ; la surface doit correspondre au pin
    approuvé avant activation.
12. **Pas de déploiement implicite.** Tests in-process et navigateur local sont la
    définition automatisable. Bind prod, DNS, ACME Let's Encrypt prod, push,
    Terraform et publication du dashboard exigent une autorisation explicite.

## 5. Contrat G1 — tunnel client et TLS public

### 5.1 Configuration stricte

Shape indicative, à graver par Gherkin avant l'implémentation :

```yaml
relay:
  endpoint: https://relay.aithos.fr:443
  tunnel_name: relay.aithos.fr
  tenant: acme
  hostname: acme.mcp.aithos.fr
  cert:
    kind: acme-dns01
    directory: https://acme-v02.api.letsencrypt.org/directory
    store_url: https://store.aithos.fr
    cache_dir: /var/lib/aithos/tls
  reconnect:
    base_ms: 1000
    max_ms: 60000
    jitter_percent: 20
dashboard:
  allowed_origins:
    - https://app.aithos.fr
```

`deny_unknown_fields` partout. HTTPS obligatoire hors loopback. Tenant, hostname,
SNI et `gateway_pub` doivent être cohérents. Cert/key cache en 0600, répertoire
non world-readable. Le mode fichier PEM explicite est admis pour test/entreprise ;
la clé privée ne peut jamais venir d'une valeur YAML inline.

### 5.2 Comportement

- utiliser la clé gateway existante pour signer la ligne B.2 ;
- nonce et instant frais, entropie injectée en tests ;
- TLS vers le relay avec SNI et ALPN `aithos-tunnel/1` ;
- activer TCP keepalive suivant B.3 ;
- lire une seule réponse bornée ; refus = aucun mux ;
- devenir yamux serveur ; chaque stream entrant commence au ClientHello public ;
- terminer le TLS public côté gateway ;
- servir le même router axum que le listener direct ;
- reconnecter sur EOF/GoAway avec backoff exponentiel plafonné et jitter ;
- remplacement du tunnel et shutdown propres, aucun tunnel zombie ;
- readiness distincte : process vivant, Vault joignable, tunnel connecté ;
- métriques/logs bornés aux événements, hostname/tenant, durées et volumes ; aucun
  octet HTTP, header, query string, token ou secret.

### 5.3 ACME

Extraire ou réutiliser la moitié cliente du `pod_stub`, sans faire dépendre le
gateway du crate provider et de ses dépendances AWS. Deux choix admis :

- petit module wire pur partagé sans dépendance provider/runtime ; ou
- client gateway indépendant vérifié byte-for-byte contre le vecteur p3.

Interdit : `aithos-gateway -> aithos-provider` en dépendance de production.

Le compte ACME et la clé de certificat naissent côté client. Le TXT DNS-01 est
posé par l'API B.5 signée sous `gateway_pub`. Cache réutilisé si le certificat
est encore valide ; renouvellement avant expiration ; échec de renouvellement
ne remplace jamais un certificat encore valide par un état cassé.

### 5.4 Scénarios Gherkin minimaux G1

Contrat dédié `gateway-relay.feature`, committé seul et RED avant code :

- stanza absente = chemin direct inchangé ;
- champ inconnu, HTTP public, hostname invalide, cert/key permissive : refus boot ;
- ligne B.2 byte-exacte au vecteur p3 ;
- mapping valide : tunnel et HTTPS public atteignent `/mcp`, `/oauth/callback`
  et `/control/v1/status` ;
- mapping inconnu/suspendu, signature fausse, nonce rejoué, clock skew : aucun
  stream applicatif ;
- le relay ne voit/loggue aucun marqueur sentinelle HTTP ;
- deux connexions publiques simultanées sont isolées ;
- remplacement reçoit GoAway ; reconnexion récupère sans restart ;
- relay indisponible : listener direct continue, relay readiness rouge ;
- ACME clé locale, cache 0600, renouvellement atomique ;
- callback OAuth amont réel de test traverse le tunnel et écrit seulement Vault.

## 6. Contrat G7 — preuve et contrôle entreprise

G7 est découpé en G7-A (auth), G7-P (preuve) et G7-C (connecteurs/OAuth). Les
trois sont nécessaires au beat final.

### 6.1 G7-A — enveloppe, Origin et anti-rejeu

Toutes les routes `/control/v1/**` exigent :

- HTTPS hors loopback ;
- `Origin` exact dans l'allowlist ;
- `X-Aithos-Auth` A.2 canonical signé couvrant méthode, path exact, `body_b3`,
  instant, nonce et chaîne de mandats ;
- fenêtre ±300 s et nonce one-shot ;
- résolution DID/cert/revocation fraîche ;
- autorité exacte owner, auditeur ou `act.x.<connector>.config` selon la route ;
- taille de body et temps de traitement bornés.

CORS : preflight `OPTIONS`, origin exact, méthodes/headers minimaux,
`Vary: Origin`, `Access-Control-Max-Age` borné. Les réponses sensibles portent
`Cache-Control: no-store`. Aucun cookie, aucune session implicite, aucun bearer
dashboard persistant.

Les contrôles Origin/CORS sont une barrière navigateur, jamais l'autorité : une
requête curl avec un Origin falsifié reste refusée sans signature/mandat valide.

### 6.2 G7-P — surface de preuve

Routes minimales, formes fermées et paginées :

```text
GET /control/v1/status
GET /control/v1/contexts
GET /control/v1/contexts/<name>/certs?cursor=&limit=
GET /control/v1/contexts/<name>/gamma?kind=&cursor=&limit=
GET /control/v1/contexts/<name>/heads
```

La gateway renvoie des artefacts signés/ciphertext et un état opérationnel
minimal. Elle ne déchiffre pas pour le dashboard. Le navigateur vérifie chaîne,
hash-chain, signatures et portée auditeur en `aithos-client`/WASM. Une requête
auditeur ne voit que sa tranche. Quand le RemoteStore possède la même preuve, le
dashboard peut la lire directement et comparer ; la surface live n'est pas une
seconde vérité.

`/status` ne révèle jamais : chemins locaux, env vars, Vault refs, URLs avec
query, détails d'erreur amont, tokens, secrets, corps MCP ou arguments d'action.

### 6.3 G7-C — registry, attachement et OAuth amont

Routes minimales :

```text
GET    /control/v1/connectors
POST   /control/v1/connectors/<id>/stage
PUT    /control/v1/connectors/<id>/client-secret
POST   /control/v1/connectors/<id>/oauth/start
GET    /control/v1/connectors/<id>/oauth/status
POST   /control/v1/connectors/<id>/activate
DELETE /control/v1/connectors/<id>/draft
```

Le contrat JSON doit être versionné, fermé et borné. Ne pas exposer une route
générique « écrire le YAML » ou « écrire n'importe quel chemin Vault ».

#### Stage

L'entrée publique contient uniquement : id canonique, contexte, endpoint HTTPS,
transport, coordonnées OAuth publiques, scopes attendus, redirect URI exact,
identifiant du manifest/pin approuvé et identifiants logiques des records Vault.
Le serveur résout lui-même les paths autorisés à partir de l'id ; le navigateur
ne choisit aucun mount ou chemin arbitraire.

Avant de créer le draft, la gateway :

1. retrouve le manifest approuvé déjà scellé dans le contexte ;
2. vérifie signature, pin, id et appartenance au contexte ;
3. vérifie l'autorité de config exacte ;
4. journalise l'intention de gouvernance sans donnée secrète ;
5. écrit un draft non actif dans le registry local.

#### Client secret

Body fermé `{ "client_secret": "..." }`, limite stricte. La valeur :

- ne passe jamais dans serde `Debug`, erreur, trace, Gamma ou réponse ;
- est écrite dans le record Vault dérivé par la gateway ;
- est zeroized immédiatement ;
- n'est jamais relue par le dashboard ;
- ne rend pas le connecteur actif.

Une référence de secret est non secrète mais n'a pas à être retournée. Un échec
Vault laisse le draft déconnecté et ne modifie pas le router.

#### OAuth start/status

`oauth/start` appelle l'implémentation existante et retourne seulement l'URL de
consentement et l'expiration de la tentative, avec `no-store`. Le PKCE verifier,
state record et tokens restent dans Vault. `/oauth/callback` demeure l'unique
callback et ne redirige vers le dashboard qu'avec un résultat générique ; aucun
code, state ou token dans l'URL finale.

Les états publics sont fermés : `draft`, `secret_missing`, `disconnected`,
`pending`, `connected`, `expired`, `drifted`, `unavailable`. Aucun détail Vault
ou fournisseur dans la réponse.

#### Activate

Activation est une transaction logique :

1. vérifier à nouveau autorité, revocations, draft et manifest ;
2. obtenir un bearer valide via `UpstreamOAuthRegistry` ;
3. appeler `tools/list` une fois ;
4. comparer noms, schémas, classes et digest au manifest approuvé ;
5. en cas de drift, enregistrer `drifted`, refuser et ne rien exposer ;
6. construire la vue runtime depuis le manifest approuvé, jamais depuis une
   confiance accordée à la réponse live ;
7. persister atomiquement `gateway/connectors.json` ;
8. swapper la registry en mémoire ;
9. seulement alors rendre les outils visibles sur `tools/list`.

La lecture `tools/list` normale ne consulte ni Vault ni réseau. Un restart relit
le registry, revalide pins et état OAuth, et échoue fermé par connecteur sans
empêcher les connecteurs sains de démarrer.

La suppression d'un draft peut nettoyer son record secret/token si le broker
offre une suppression sûre. Sinon, elle désactive toute référence runtime et le
handoff DONE documente le résidu Vault ; ne pas inventer une suppression par
écrasement silencieux.

### 6.4 Scénarios Gherkin minimaux G7

Contrats dédiés, committés RED avant code :

- CORS exact et preflight depuis `app.aithos.fr` ; wildcard et origin voisin
  refusés ;
- signature absente/fausse, body modifié, nonce rejoué, clock skew, mandat expiré
  ou révoqué : zéro effet ;
- auditeur borné ne lit que son Gamma ; owner/config n'obtient aucun plaintext
  secret ;
- status ne contient aucun marqueur sentinelle ;
- connector id/path/URL/scopes/champs inconnus rejetés avant Vault ;
- connecteur non approuvé ou mauvais pin rejeté ;
- client secret écrit une fois dans Vault, absent de toutes les sorties/stores
  hors Vault ;
- OAuth start place pending state seulement dans Vault ;
- callback heureux rend `connected`, callback error/replay/mauvais state refuse ;
- refresh heureux ; refresh cassé = `unavailable`, zéro requête upstream ;
- activation découvre avec bearer puis accepte un catalogue identique ;
- outil ajouté, retiré, schema modifié ou digest différent = drift, zéro outil
  exposé ;
- persistance crash-safe : ancien registry ou nouveau complet, jamais un JSON
  partiel ;
- hot activation sans restart ; `tools/list` ne lit pas Vault ;
- deux connecteurs isolés : panne OAuth de A n'éteint pas B ;
- un appel mandaté passe log-before-relay ; le voisin est refusé avant Vault et
  avant upstream ;
- toutes les réponses d'erreur sont stables et redacted.

## 7. Client, SDK et dashboard

### 7.1 Frontière client/SDK

`aithos-client`/WASM produit l'enveloppe signée et vérifie les artefacts reçus.
Le SDK ne fait que : fetch, CORS, timeout/retry borné, pagination, erreurs typées
et orchestration UI.

Ajouter une façade `GatewayControlClient` seulement après que l'API offline du
client existe. Interdit en TypeScript : Ed25519/JCS maison, body hash maison,
parsing de mandat comme autorité, acceptation d'un Gamma non vérifié.

Erreurs publiques minimales : `gateway_offline`, `origin_denied`,
`authority_denied`, `connector_not_approved`, `secret_unavailable`,
`oauth_pending`, `oauth_denied`, `oauth_unavailable`, `manifest_drift`,
`activation_failed`, `upstream_denied`.

### 7.2 Dashboard réel

Cible : la console `/Volumes/Math17/aithos/v2/code/aithos-sdk-example`, après
attribution de son worktree. Réutiliser le SDK v2 local et les handles opaques.
Le prototype `apps/dashboard/living-product` peut inspirer le composant visuel.

La tranche UI minimale contient :

- hostname gateway et indicateur online/offline ;
- sélection d'une identité/mandat de config local, jamais uploadé chez Aithos ;
- liste des connecteurs approuvés et de leurs états ;
- formulaire d'attachement d'instance ;
- champ secret éphémère avec effacement immédiat ;
- bouton « Connecter avec OAuth » ouvrant le consentement ;
- reprise après callback et polling borné du statut ;
- comparaison « approuvé / live » et refus de drift lisible ;
- activation ;
- un appel autorisé, un appel voisin refusé ;
- panneau de preuve vérifiée localement ;
- offline UX honnête quand la gateway est absente.

Aucune donnée secrète dans localStorage, sessionStorage, IndexedDB, URL,
analytics, console, crash report ou service worker. CSP stricte, pas de script
tiers dans la page de gestion des clés/secrets. Les réponses live ne sont pas
cachées.

### 7.3 E2E navigateur

Le gate reproductible lance :

- faux relay/control plane conforme C2 ;
- gateway réelle avec tunnel G1 ;
- Vault KV v2 réel ou faux HTTP contractuel déjà utilisé par les tests gateway ;
- faux AS OAuth et faux MCP protégés sur sockets réelles ;
- dashboard réel servi en HTTPS/local de test ;
- navigateur Playwright/équivalent réel.

Le navigateur réalise le beat §0 sans injection directe dans la mémoire des
processus. Assertions réseau : aucun secret hors route TLS gateway→Vault/AS,
aucun token dans DOM, storage, URL, console ou trace ; relay sans payload ; un
refus voisin produit zéro hit Vault/upstream.

## 8. Ordre de développement et commits

Ne pas commencer par l'UI. Ordre strict :

1. **A0 — audit et coordination** : worktrees, branches, agents actifs, baseline,
   ownership des changements ; état écrit dans le journal de session.
2. **A1 — contrats seuls** : features G1/G7 + types publics SDK, parse RED observé,
   commit étroit.
3. **G1a — tunnel local** : dial, B.2, yamux, backoff, fake relay, tests.
4. **G1b — TLS/ACME** : terminaison public TLS, B.5, cache/renewal, tests.
5. **G1c — intégration router** : direct + relay, callback OAuth à travers tunnel,
   suites gateway.
6. **G7a — auth/CORS + preuve** : middleware A.2, anti-rejeu, routes read-only,
   vérification adversariale.
7. **G7b — registry + OAuth control** : stage, secret, start/status, activate,
   hot swap, persistance.
8. **SDK** : wrapper transport après API offline signée du client.
9. **Dashboard** : verticale réelle, aucun mock de succès.
10. **E2E navigateur** : beat complet deux fois de suite depuis des états frais.
11. **Témoin adversarial indépendant** : confidentialité, autorité, drift, crash,
    isolation, logs, doctrine provider.
12. **Docs et handoff DONE** : runbook local, runbook VM, résultats exacts, limites.

Commits recommandés : contrats ; G1 tunnel ; G1 TLS/intégration ; G7 auth/preuve ;
G7 registry/OAuth ; client/SDK ; dashboard/E2E ; docs. Un commit ne mélange jamais
deux dépôts. Ne pas pousser, publier ou merger sans autorisation explicite.

## 9. Rituel multi-agents pendant l'absence de Mathieu

Un seul **lead intégrateur** tient le plan et les décisions ci-dessus.

Règles d'ownership :

- jamais deux agents simultanés sur `aithos-gateway` ; `config.rs`, `main.rs`,
  `proxy_mcp.rs`, `cucumber.rs` et les Cargo files appartiennent exclusivement au
  lead pendant leurs fenêtres d'intégration ;
- un agent G1 peut travailler sur de nouveaux modules/tests après gel du contrat ;
- un agent G7 peut travailler ensuite sur de nouveaux modules/tests, pas en
  parallèle dans le même crate ;
- un agent client/SDK n'intervient qu'après clôture ou parking propre du chantier
  client actif ;
- un agent dashboard n'écrit que dans le dépôt dashboard attribué ;
- le témoin adversarial est en lecture/test uniquement jusqu'à son verdict ; il
  ne corrige pas lui-même le défaut qu'il découvre ;
- tout transfert entre agents cite commit, fichiers possédés, tests passés,
  changements étrangers observés et risques restants ;
- avant chaque commit, le lead inspecte `git diff`, `git diff --cached`, status et
  attribution fichier par fichier ; jamais `git add .` ;
- aucun agent ne stash, reset, checkout, formatte globalement ou commet le travail
  d'un autre ;
- un conflit d'ownership provoque attente/coordination, pas une résolution
  silencieuse.

Le lead peut prendre seul les décisions déjà fixées dans §4. Il doit STOPPER et
laisser un handoff bloqué, sans bricolage, pour : changement de protocole/vecteur,
nouvelle autorité, OAuth/token hébergé chez Aithos, support d'un MCP arbitraire,
envoi Gmail/GSE, migration de secrets, destruction de données, mutation AWS prod,
publication ou merge.

À chaque gate : contrat RED prouvé → impl minimale → GREEN ciblé → adversarial →
suites voisines → fmt/clippy → commit étroit → message au lead. Un compteur qui
bouge sans scénario détaggé ou expliqué = STOP.

## 10. Gates techniques

### Gateway/provider local

À adapter seulement si la toolchain l'exige, sans réduire la couverture :

```sh
cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-provider --test cucumber_tunnel
CARGO_INCREMENTAL=0 cargo test -p aithos-provider --test cucumber_relay
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway
```

Ajouter les E2E dédiés G1/G7 et les exécuter deux fois avec répertoires et nonces
frais. `git diff --check` sur chaque dépôt. Si le fmt global échoue sur des fichiers
étrangers, consigner les chemins exacts et vérifier le périmètre modifié ; ne pas
les reformater.

### Client/SDK/dashboard

Respecter les gates du handoff client. Ajouter au minimum : tests d'architecture
SDK, tests unitaires `GatewayControlClient`, build TypeScript, lint, E2E navigateur,
scan des storage/console/traces pour sentinelles secrètes.

### Gate live

Le gate live n'est pas implicite. Avec autorisation et credentials frais :

- créer/binder un tenant et hostname jetables ;
- connecter une gateway locale au relay déployé ;
- obtenir un certificat staging puis production si autorisé ;
- jouer le dashboard contre un vrai MCP OAuth non destructif ;
- ne jamais appeler une capability destructive ;
- purger le mapping jetable et prouver l'état de repos ;
- aucun token/secret dans captures, sorties ou handoff.

## 11. Définition de fini

Le lot est fini seulement si :

- une gateway sans port entrant ouvre G1 et sert le même router derrière le relay ;
- le TLS public termine chez le client et le relay ne voit aucun payload ;
- le dashboard utilise des requêtes signées, CORS exact et aucune autorité SaaS
  implicite ;
- preuve historique disponible sans gateway, état live honnête avec gateway ;
- un connecteur approuvé est stagé, son secret écrit uniquement dans Vault,
  son OAuth effectué et son catalogue live revalidé ;
- le connecteur devient actif à chaud et survit à un restart ;
- un appel mandaté atteint le MCP après log, un voisin refuse avant Vault/upstream ;
- le dashboard vérifie localement la preuve ;
- le beat complet passe deux fois dans un navigateur réel avec gateway locale ;
- aucune seed, clé, credential, token, code OAuth ou payload ne fuit ;
- les suites, clippy, fmt ciblé et témoin adversarial sont verts ;
- chaque dépôt a des commits étroits attribuables, sans travail étranger ;
- les limites restent dites : connecteur pré-approuvé, pas d'arbitrary MCP, pas
  d'envoi Gmail, pas de déploiement/publish implicite.

## 12. Hors périmètre explicite

- GSE/Gmail send et toute action externe destructive ;
- découverte OAuth RFC 8414/9728 ou DCR amont si les URLs/client id ne sont pas
  explicitement configurés ;
- marketplace public de connecteurs arbitraires ;
- frappe d'une nouvelle politique owner complète depuis le dashboard ;
- multi-principal G4/G5 au-delà de ce qui est requis par les mandats existants ;
- backend Aithos détenteur de session owner, clé, secret ou token client ;
- haute disponibilité Vault/gateway, autoscaling et DR VM ;
- billing/self-service tenant ;
- déploiement AWS, DNS, publication dashboard, push ou merge.

La VM devient le lot d'exploitation suivant : installer les mêmes binaires,
copier uniquement la configuration non secrète et les identités prévues, brancher
Vault, ouvrir l'egress 443 et rejouer exactement le même gate via le même hostname.

## 13. Prompt de reprise

> Reprendre la verticale entreprise G1 + G7 depuis
> `/Volumes/Math17/aithos/v2/code/aithos-core`. Lire intégralement
> `docs/HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md` et toutes ses
> références obligatoires avant toute action. État disque = vérité : auditer les
> worktrees `aithos-core`, `aithos-client`, `aithos-sdk` et
> `aithos-sdk-example`, attribuer chaque changement et préserver tout travail
> étranger. Tenir un lead intégrateur unique ; ne jamais faire travailler deux
> agents simultanément dans `aithos-gateway`. Exécuter les lots dans l'ordre §8,
> contrats Gherkin RED et commit étroit avant chaque implémentation. La cible est
> le beat §0 complet deux fois dans un navigateur réel : gateway + Vault locaux,
> tunnel sortant G1, TLS terminé côté client, dashboard réel, attachement d'un
> connecteur pré-approuvé, secret uniquement dans Vault, OAuth MCP amont,
> validation anti-drift, activation à chaud, appel mandaté, refus voisin et preuve
> vérifiée localement. Ne pas réimplémenter OAuth, JCS, signatures ou autorité en
> TypeScript. Ne pas étendre aux MCP arbitraires, GSE/Gmail send, nouvelle
> grammaire protocolaire, déploiement AWS/prod, publication, push ou merge. Faire
> intervenir un témoin adversarial indépendant avant clôture. Suites complètes,
> clippy, fmt ciblé, scan anti-fuite et handoff DONE obligatoires ; toute décision
> hors §4 ou conflit de propriété non résoluble = STOP documenté, jamais un
> contournement silencieux.

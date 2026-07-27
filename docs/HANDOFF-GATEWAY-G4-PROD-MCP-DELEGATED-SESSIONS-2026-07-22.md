# HANDOFF — G4/G5 et passage en production : authentification MCP par mandat délégué et session éphémère

> **ARCHIVE DE PLAN.** G4 a livré la session déléguée et le front door Core SC1.
> Les gates de production et la démo intégrée sont suivis dans les documents
> courants indexés par `README.md`.

Date : 2026-07-22

Statut : **plan historique ayant conduit à G4**. La gateway, le Core, le WASM et
la CLI déléguée ont depuis été livrés dans les commits suivant `c10fbc4`, jusqu'à
`1c11bb1`. Le parcours live de démonstration est prouvé ; la custody navigateur
et l'intégration client/SDK restent à reprendre dans le handoff client/SDK du
2026-07-22. Les sections « manque » et l'ordre P0–P11 décrivent l'état d'entrée,
pas l'état courant. Aucun déploiement implicite n'est autorisé.

Branche observée : `codex/publish-aithos-core-busl`.

HEAD observé à la rédaction : `c10fbc4` (`docs(demo): hand off local gateway CLI run`).

Ce document fixe les décisions produit et le plan de livraison pour remplacer le
consentement OAuth DEV par une authentification réelle fondée sur un mandat
délégué, tout en restant compatible avec les clients MCP distants standards tels
que Claude Cowork. Il inclut les conditions nécessaires avant de qualifier puis
déployer une gateway de production.

Il n'autorise à lui seul ni déploiement AWS, ni binding d'un hostname de
production, ni publication de package/image/dashboard, ni push, ni merge.

---

## 0. Résultat exact attendu

Une personne possède localement une clé Ed25519 de délégué et un mandat Aithos
émis vers sa clé publique. Elle connecte Claude Cowork à
`https://<entreprise>.mcp.aithos.fr/mcp`.

La gateway sert le flow OAuth MCP standard, mais remplace le bouton DEV par une
cérémonie cryptographique :

```text
owner
  │ émet
  ▼
mandat personne (grantee = clé publique du délégué)
  │ la personne signe une délégation de session
  ▼
sous-mandat de session (grantee = gateway_pub,
                         session_bind = clé publique éphémère)
  │
  ├── OAuth code/refresh/access token liés à un sid opaque
  │
  └── chaque opération est vérifiée sous :
      chaîne owner → personne → gateway
      + certificat SC1
      + preuve de session SC1
      + périmètre/contraintes/obligations
```

Le Bearer OAuth n'est jamais l'autorité. Il est une projection périssable qui
sélectionne une session. La chaîne, les révocations, la fenêtre, le tool, ses
arguments et ses obligations sont revérifiés avant chaque effet.

Le lot est fini lorsqu'un vrai Claude Cowork :

1. découvre la gateway par RFC 9728/RFC 8414 ;
2. s'enregistre par DCR et lance PKCE ;
3. passe la cérémonie avec une vraie clé de délégué ;
4. obtient une surface dérivée de cette seule session ;
5. exécute un appel sûr mandaté ;
6. se fait refuser un voisin hors périmètre avant Vault/amont ;
7. est coupé à l'appel suivant après révocation ;
8. laisse une preuve Gamma attribuée à la bonne chaîne et à la bonne session.

## 1. Ce qui a été réellement prouvé le 2026-07-22

### 1.1 Démo directe locale

Un runtime persistant de développement a été créé hors dépôt sous :

```text
/Volumes/Math17/aithos-runtime/demo
```

État prouvé par l'opérateur :

- Vault natif local sur `127.0.0.1:18200`, stockage fichier, initialisé et non
  scellé ;
- gateway locale sur `127.0.0.1:14890` ;
- tenant provider `demo`, actif dans le control plane de production ;
- DIDs journal et contexte liés au tenant et historiques répliqués ;
- connecteur GitHub réel, credential uniquement dans Vault ;
- 44 outils découverts et décidés explicitement : `get_me` accordé, 43 voisins
  refusés ;
- surface locale exacte : `briefing.read`, `github__get_me`,
  `journal.search`, `journal.write` ;
- briefing gouverné lu ;
- `github__get_me` réussi ;
- `github__delete_file` refusé par mandat avec `-32001`, sans effet amont ;
- preuves `action` et `ethos.read` exportées, sans fuite de secret.

### 1.2 Relay, TLS public et OAuth entrant

Le mapping suivant existe pour le tenant de développement :

```text
demo.mcp.aithos.fr → tenant demo → gateway_pub observée
```

Le parcours public a été prouvé :

- tunnel sortant relay actif ;
- certificat Let's Encrypt exact pour `demo.mcp.aithos.fr`, SAN exact ;
- `/.well-known/oauth-authorization-server` public : HTTP 200 ;
- `/.well-known/oauth-protected-resource` public : ressource exacte `/mcp` ;
- `/mcp` sans token : HTTP 401 + `WWW-Authenticate` RFC 9728 ;
- DCR/PKCE/callback Claude réellement franchis ; l'opérateur a connecté Cowork
  à l'environnement DEV ;
- le certificat et la clé TLS restent côté gateway ; le relay ne termine pas le
  TLS public.

Ne pas sur-vendre : aucune preuve partagée dans cette session ne montre encore
un appel métier public effectué depuis Cowork puis son nouvel audit. La connexion
OAuth réelle est prouvée par l'opérateur ; le gate métier Cowork reste à figer
dans le lot présent.

### 1.3 Incident Rustls à conserver comme preuve de blocage

Le binaire hérité :

```text
/Volumes/Math17/aithos-runtime/demo/bin/aithos-gateway
sha256 a94f2d448bf36044fc9c2ca5d81c86638560c16c0fe4ca21868fcb8bd03e2473
```

a paniqué au démarrage du plan ACME/relay : Rustls ne pouvait choisir entre
`ring` et `aws-lc-rs` après un build aux features unifiées.

Un build ciblé, depuis une archive propre de `c10fbc4`, a produit un candidat
`ring` seul :

```text
/Volumes/Math17/aithos-runtime/demo-build-ring-20260722/target/debug/aithos-gateway
sha256 71e725f756f0940c23b660b17aad8fcc56ca58c9f0d08da97a6e059c180db1fa
```

Ce candidat a établi le certificat et le tunnel, mais reste un binaire `dev`
non publiable. La correction de production n'est pas « toujours construire le
package seul » : le process doit sélectionner explicitement son
`CryptoProvider` avant toute construction TLS, et les builds workspace comme
ciblés doivent être testés.

## 2. Ce qui manque et interdit aujourd'hui la production

Les points suivants sont des bloqueurs, pas des améliorations optionnelles :

1. `/authorize` affiche un bouton DEV dont le POST ne vérifie aucune signature
   de délégué ;
2. clients DCR, codes et familles refresh sont en mémoire dans `AuthServer` et
   disparaissent au restart ;
3. aucun token n'est lié à une chaîne de session distincte ;
4. la gateway utilise encore la chaîne runner pré-G4 comme ceiling global ;
5. `aithos-wasm` n'expose aujourd'hui que `genesis_pubkeys` ;
6. SC1/W1.1 existe dans le Core mais n'est pas consommé par le hot path MCP ;
7. le multi-principal par token/session n'est pas câblé ;
8. le build public peut paniquer selon l'unification des features Rustls ;
9. les binaires observés sont des builds debug sans provenance de release
   publiée ;
10. le Vault local, ses tokens périodiques et ses fichiers d'initialisation sont
    une installation de démonstration, pas un runbook d'exploitation ;
11. `demo`, son hostname et son credential GitHub restent des objets de
    développement ; ils ne deviennent pas la production par renommage ;
12. supervision, sauvegarde/restauration, rotation, rollout et rollback ne sont
    pas encore qualifiés.

## 3. Périmètre et hors périmètre

### Dans le lot

- profil MCP distant compatible OAuth 2.1/DCR/PKCE/Bearer ;
- enrôlement `pubkey-first` d'un délégué ;
- cérémonie cryptographique servie par la gateway ;
- sous-mandat court vers `gateway_pub` ;
- `session_bind` et SC1/W1.1 consommés sans changer leur wire ;
- liaison OAuth `sid` → session → chaîne ;
- multi-principal et surface par session ;
- persistance Vault de l'état AS et des clés de session temporaires ;
- redémarrage, refresh, révocation et audit ;
- flow CLI équivalent ;
- build release reproductible et runbook de production ;
- gate réel Cowork sur un outil de lecture non destructif.

### Hors périmètre

- enveloppe signée par Cowork à chaque requête ;
- profil Aithos-native et DPoP propriétaire ;
- délégation d'une clé privée à Anthropic ;
- marketplace de connecteurs arbitraires ;
- Gmail send ou toute action externe destructive dans le gate ;
- nouvelle grammaire de mandat ou réinterprétation d'un profil historique ;
- backend Aithos détenteur d'une clé de personne, clé de session, token OAuth ou
  credential connecteur ;
- frappe d'un mandat owner complet depuis le dashboard ;
- publication/déploiement/push/merge implicite.

## 4. Lectures obligatoires avant contrat ou code

Lire intégralement, dans cet ordre :

1. ce handoff ;
2. `docs/HANDOFF-GATEWAY-G3-DONE-2026-07-17.md` ;
3. `docs/HANDOFF-GATEWAY-HUB.md` ;
4. `docs/PROMPT-REPRISE-G4.md`, en considérant les arbitrages du §5 ci-dessous
   comme remplaçant ses points alors non tranchés ;
5. `docs/GAPS-DEMO-E2E.md` ;
6. `docs/STANDARDS-COMPAT.md`, notamment §5.1 et C1 ;
7. `docs/INFRA-PROVIDER.md`, doctrine, §5, annexe A.2 et annexe B ;
8. `spec/04-mandates.md`, §4.5, §4.6, §4.7 et §4.12 ;
9. `spec/05-delegation.md` intégralement ;
10. `vectors/cb2-session-proof.json` et son générateur indépendant ;
11. le code `aithos-core` qui expose `verify_session` et
    `verify_max_sessions` ;
12. `crates/aithos-wasm/` intégralement ;
13. côté gateway : `oauth.rs`, `proxy_mcp.rs`, `core_bridge.rs`, `config.rs`,
    `main.rs`, `control.rs`, `keyholder.rs`, `credentials.rs`, puis les features
    G1/G2/G3/G6/G7 ;
14. `docs/HANDOFF-GATEWAY-G1-G7-ENTERPRISE-DASHBOARD-2026-07-21.md` pour les
    frontières dashboard/control, sans reprendre son état d'entrée désormais
    dépassé ;
15. la documentation officielle Claude sur les connecteurs MCP distants :
    `https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp` ;
16. la spécification d'autorisation MCP :
    `https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization`.

Une retouche Core, Bundle, vecteur, profile SC1/W1.1 ou wire provider constitue
une extension de périmètre : STOP, contrat/vecteur indépendant et validation
humaine avant code.

## 5. Décisions gravées le 2026-07-22

Ces décisions ont été validées par Mathieu et ne doivent pas être redemandées
pendant l'implémentation.

### 5.1 Compatibilité et attribution

1. La première production sert **MCP compatible uniquement** : OAuth 2.1, DCR,
   PKCE et Bearer.
2. Le Bearer sélectionne une session ; il ne remplace jamais l'autorité Aithos.
3. Cowork ne signe pas chaque requête. La formulation opposable est :

   ```text
   OAuth Claude → sid → sous-mandat de session → chaîne déléguée → action Gamma
   ```

4. La gateway produit les preuves opérationnelles sous sa clé de feuille et la
   clé de session, puis journalise la chaîne complète. Ne jamais présenter ce
   profil comme une preuve cryptographique émise par Cowork.
5. Le profil Aithos-native est différé et ne doit pas contaminer ce lot.

### 5.2 Enrôlement et custody de la personne

1. **`pubkey-first` est le seul flux de production.**
2. Le pack dont l'entreprise génère et transporte la keypair reste DEV et doit
   être impossible à activer dans le profil production.
3. La clé Ed25519 du délégué naît côté utilisateur et n'est jamais transmise à
   la gateway, au provider, à Claude ni à Aithos.
4. Première custody web admise : keystore Aithos chiffré, protégé par phrase
   secrète, chargé dans WASM uniquement pendant la cérémonie, zeroized ensuite.
5. Aucune seed ou clé en clair dans DOM sérialisé, URL, logs, analytics,
   localStorage, sessionStorage, IndexedDB, service worker ou crash report.
6. L'interface de signature reste injectable pour un keystore natif ou matériel
   ultérieur.

### 5.3 Durées et concurrence

Profil de production par défaut :

```text
authorization code : 2 minutes, one-shot
access token       : 15 minutes
session/refresh    : 8 heures maximum
sessions actives  : 3 maximum par mandat délégué
```

Toutes les durées sont plafonnées par les fenêtres de la chaîne et de SC1. Le
refresh ne prolonge jamais la session ; après huit heures, nouvelle cérémonie.
Le parent peut imposer plus court ou moins de sessions, jamais l'inverse.

### 5.4 Route et UX

1. `/authorize` reste l'endpoint OAuth standard et valide d'abord client,
   redirect URI, PKCE et `resource`.
2. Une requête valide affiche la cérémonie à la place du bouton DEV.
3. La finalisation utilise une route interne versionnée, indicative
   `POST /ceremony/complete`, liée à la transaction `/authorize` en cours.
4. La page affiche avant signature : hostname/entreprise, client, resource,
   Ethos/contexte, chaîne, périmètre, contraintes, obligations, TTL,
   `gateway_pub` et clé publique de session.
5. Annuler ne frappe rien, ne mint aucun code et détruit le pending state.

### 5.5 Persistance

1. L'état OAuth en mémoire est interdit en production.
2. Le Vault de l'entreprise conserve, sous namespaces fermés et CAS : clients
   DCR, pending ceremonies, sessions, familles refresh, anti-rejeu et clés de
   session temporaires.
3. Les refresh tokens sont conservés sous forme de hash lorsque la valeur brute
   n'est plus nécessaire.
4. La clé d'adapter OAuth a une custody durable Vault/keystore entreprise ; elle
   n'est pas un objet protocole et n'entre pas dans `keyholder.rs`.
5. Restart : les sessions non expirées et non révoquées continuent ; un état
   incomplet ou non déchiffrable échoue fermé.

### 5.6 Révocation et obligations

1. Révocation du parent, de la feuille de session ou de la session locale : dès
   l'appel suivant, Bearer et refresh refusés avant Vault connecteur/amont.
2. Les obligations du parent sont conservées et peuvent être renforcées.
3. Une action `binding` ou portant `co_sign` continue d'exiger son reçu lié à
   l'opération ; la cérémonie n'est pas une approbation anticipée de toutes les
   actions futures.
4. L'UX de contre-signature par action est un lot distinct.

## 6. Conciliation normative SC1 avec un client MCP Bearer

La spécification W1.1 impose, pour une opération sous `session_bind`, deux
preuves indépendantes sur le même `operation_ref` : la preuve de la feuille et
la preuve de la clé de session.

Une clé de session détenue uniquement par la page ne convient pas à Cowork : une
fois OAuth terminé, les requêtes partent de l'infrastructure Anthropic et la page
ne peut plus signer chaque opération.

Décision de couture pour le profil MCP :

1. la gateway génère une keypair de session Ed25519 lors de la transaction de
   cérémonie ;
2. elle conserve la seed uniquement dans le Vault entreprise, avec expiration
   maximale de huit heures ;
3. elle envoie à la page seulement `session_pub`, un nonce et le digest fermé de
   la transaction ;
4. le délégué frappe et signe un sous-mandat dont :
   - `issued_by` = clé publique du délégué ;
   - `parent` = son mandat actif ;
   - `grantee.pubkey` = `gateway_pub` ;
   - `session_bind` = `session_pub` ;
   - fenêtre ⊆ parent et ≤ 8 h ;
   - périmètre/contraintes/obligations seulement atténués ;
   - aucun droit `issue` transmis ;
5. la signature du sous-mandat prouve la décision du délégué ; une preuve de
   cérémonie distincte lie aussi client OAuth, redirect, resource, PKCE,
   `gateway_pub`, `session_pub`, nonce et digest WYSIWYS ;
6. la gateway vérifie la chaîne et l'atténuation avec le Core, publie le
   certificat de feuille selon le store existant, puis signe SC1 sous la clé de
   feuille `gateway_pub` ;
7. à chaque action, la gateway produit la preuve native de feuille sous
   `gateway_pub` et la preuve de session sous la clé éphémère, toutes deux sur le
   même `operation_ref`, puis appelle `verify_session` avant l'effet.

Cette couture est compatible avec la règle d'atténuation actuellement
implémentée dans `aithos-core/src/constraints.rs` : le vérifieur parcourt toutes
les contraintes du parent et autorise le child à ajouter une contrainte connue
plus restrictive. Le parent personne peut donc ne pas porter `session_bind` et
le sous-mandat de session peut l'ajouter. Si le parent porte déjà
`session_bind`, la valeur du child doit être strictement identique ; la supprimer
ou la changer est un élargissement refusé. Ce comportement doit être figé par un
test de régression avant de construire la cérémonie dessus.

Ainsi, la clé longue durée de la personne reste froide entre cérémonies, la clé
de session est réellement courte, Cowork reste standard, et le Core ne reçoit
aucune exception au double-proof.

`max_sessions = 3` se vérifie avec `verify_max_sessions` sur l'ensemble injecté
des clés de session actives déjà vérifiées. La gateway doit définir par contrat
la notion d'active (non expirée, non révoquée, session locale active) sans créer
un nouveau wire SC1. Les doublons et états ambigus échouent fermés.

## 7. Flow de production détaillé

### 7.1 Enrôlement pubkey-first, hors OAuth

1. L'entreprise crée une invitation opaque à usage borné, sans clé privée.
2. Le navigateur charge l'application de cérémonie depuis le hostname de la
   gateway et vérifie son origine/TLS.
3. Le signer local génère la keypair de personne.
4. L'utilisateur sauvegarde un keystore chiffré et confirme sa récupération.
5. Seule la clé publique et l'identifiant d'invitation remontent.
6. L'owner vérifie l'identité hors protocole puis émet un mandat vers cette
   pubkey, avec au minimum le périmètre métier, `issue#depth=1` pour les sessions
   et la borne de sessions décidée.
7. Le mandat est publié dans les certs de l'Ethos et l'invitation devient prête.
8. Le navigateur récupère la chaîne publique et la vérifie localement.

Le mécanisme d'identification RH/SSO qui précède l'émission du mandat n'est pas
l'autorité Aithos. Il aide l'owner à choisir une pubkey ; le mandat signé reste
l'octroi effectif.

### 7.2 Cérémonie OAuth

1. Cowork reçoit 401 et découvre le protected resource puis l'AS.
2. DCR enregistre un client public et son callback autorisé.
3. `/authorize` valide client, redirect, `resource`, state et PKCE S256.
4. La gateway crée un pending record one-shot, une keypair de session et un
   challenge canonique fermé ; la seed part immédiatement au Vault.
5. La page charge la chaîne publique, demande le keystore chiffré et le déverrouille
   localement.
6. WASM vérifie parent, fenêtre, révocation fraîche, `issue`, atténuation demandée
   et digest de présentation.
7. WASM construit puis signe le sous-mandat de session et la preuve de cérémonie.
8. `/ceremony/complete` réserve le nonce avant tout effet, revérifie tous les
   octets côté gateway et refuse toute divergence.
9. La gateway vérifie `max_sessions`, persiste/publie la feuille, produit SC1,
   crée `sid` et lie le code OAuth one-shot à ce `sid`.
10. Le callback Claude reçoit seulement code + state.
11. `/token` échange code + PKCE + resource et émet access/refresh bornés à la
    session.
12. La clé de personne est zeroized côté page ; aucun secret de session ne sort
    du Vault.

### 7.3 Appel MCP

Ordre fail-closed, avant toute résolution de credential ou requête amont :

1. Origin/transport et forme HTTP ;
2. signature/audience/expiry du Bearer ;
3. résolution durable de `sid` ;
4. état session actif et client/resource cohérents ;
5. recharge chaîne/certs/révocations ;
6. fenêtre parent/feuille/SC1 ;
7. surface de la session et `verify_op` ;
8. bornes d'arguments et obligations ;
9. allocation du `operation_ref` ;
10. preuve native gateway + preuve session SC1 ;
11. `verify_session` ;
12. log-before-relay sous la chaîne complète ;
13. seulement alors Vault connecteur puis upstream.

Un refus avant l'étape 12 est journalisé sous l'identité de gouvernance prévue,
sans prétendre que l'agent a réalisé l'acte. Aucun refus d'autorité ne touche le
credential broker ou l'amont.

### 7.4 Refresh et redémarrage

- refresh rotation one-shot avec coupure de famille au rejeu ;
- nouveau token `exp ≤ min(now+15m, session.not_after, chain ceiling)` ;
- refresh après huit heures, révocation ou absence de state : `invalid_grant` ;
- restart : recharger adapter key, DCR, sessions et refresh depuis Vault ;
- session dont la clé temporaire manque : désactiver seulement cette session,
  jamais démarrer en mode dégradé permissif ;
- un connecteur/session cassé n'éteint pas les sessions saines.

### 7.5 Révocation et déconnexion

- déconnexion utilisateur : désactiver `sid`, couper famille refresh, détruire
  la clé de session en Vault ;
- révocation protocolaire de la feuille/du parent : publier l'acte autorisé,
  invalider les sessions dérivées et les clés temporaires ;
- appel suivant : refus avant Vault/amont ;
- la preuve historique reste vérifiable avec les certificats et SC1 publics.

## 8. État durable de l'AS

Introduire une abstraction étroite de stockage, injectée en tests. Le nom et la
forme finale sont fixés par contrat avant code ; l'implémentation production est
Vault, l'implémentation test peut être mémoire.

Records logiques minimaux :

```text
adapter-key/<kid>
dcr-client/<client_id>
pending/<transaction_id>
session/<sid>
session-key/<sid>
refresh-family/<family_id>
nonce/<purpose>/<digest>
```

Un record session peut porter uniquement les références nécessaires : version,
tenant/contexte, subject DID, client id, resource, chaîne ids/digests, leaf id,
SC1 digest, session pub, timestamps, statut et version CAS. La seed est un record
secret séparé. Aucun token brut, code, seed ou secret ne doit apparaître dans un
record non secret, un log ou une réponse.

Exigences :

- namespaces dérivés par la gateway, jamais choisis par le navigateur ;
- formes fermées, tailles bornées, `deny_unknown_fields` ;
- CAS pour création/rotation/consommation ;
- réservation de nonce/code avant effet ;
- nettoyage idempotent après expiration ;
- horloge et entropie injectées ;
- politiques Vault minimales et séparées des credentials connecteurs ;
- indisponibilité Vault = session/auth indisponible, jamais bypass.

## 9. Surface WASM et CLI

`aithos-wasm` reste un binding fin au-dessus du Core. Il ne réimplémente ni JCS,
ni Ed25519, ni mandat, ni atténuation en JavaScript.

Surface indicative à figer par tests de surface :

```text
delegate_pubkey
verify_mandate_chain
build_session_submandate
sign_ceremony_challenge
```

Les fonctions reçoivent les octets/JSON fermés et un signer injecté ou un
keystore déverrouillé. Elles ne génèrent pas silencieusement de randomness si la
discipline WASM exige l'entropie du caller. Elles ne retournent jamais la seed.

Le flow CLI équivalent doit :

- fonctionner sans navigateur avec un signer local injecté ;
- accepter clé/keystore par stdin, file descriptor ou interface custody, jamais
  par argument de process ;
- afficher uniquement URL, pubkeys, ids et verdicts non secrets ;
- exécuter le même verify/build/sign que WASM ;
- permettre les gates génériques OAuth sans créer un second protocole.

Les flags `--*-seed-hex` existants restent DEV et ne constituent pas la surface
production.

## 10. Contrats RED obligatoires avant implémentation

Créer ou étendre des features dédiées, committées seules et observées RED.

### 10.1 Cérémonie

- page DEV absente en profil production ;
- parent valide + issue depth 1 → feuille strictement atténuée ;
- parent invalide, expiré, révoqué, mauvaise subject/pubkey → refus ;
- absence d'issue ou profondeur épuisée → refus ;
- scope, fenêtre, contrainte, obligation ou `issue` élargi → refus nommant la
  famille ;
- `gateway_pub`, `session_pub`, OAuth client/resource/PKCE/redirect/nonce ou
  digest WYSIWYS modifié → refus ;
- challenge rejoué, pending expiré, POST répété → aucun code/session ;
- annulation → aucun cert, code ou token ;
- clé personne absente/mauvais keystore → aucun POST autoritaire ;
- TTL > 8 h → refus ;
- parent sans `session_bind` + child qui ajoute le `session_bind` de la session
  → atténuation acceptée ;
- parent avec `session_bind` + child identique → accepté ; valeur changée ou
  contrainte supprimée → refus ;
- quatrième session active → refus ; trois premières isolées.

### 10.2 Session et SC1

- SC1 exact et les deux preuves sur le même `operation_ref` passent ;
- preuve feuille ou session absente/fausse/croisée → `InvalidSession` ;
- mauvais digest SC1, key, interval, mandate id ou subject → refus ;
- `sid` absent, inconnu, d'un autre client/resource/contexte → 401 ;
- session expirée/révoquée/désactivée → zéro Vault/upstream ;
- deux sessions aux mandats différents obtiennent des `tools/list` différents ;
- comptages et obligations suivent la bonne chaîne ;
- Gamma porte `authorized_via` exact et le fait session exact.

### 10.3 OAuth durable

- DCR/session/refresh survivent à un restart réel ;
- code one-shot, mauvais PKCE, mauvaise resource/audience → refus ;
- refresh rotation et rejeu coupent la famille ;
- corruption/CAS conflict/indisponibilité Vault → fail-closed ;
- aucun token/code/seed dans logs, errors, gamma, DOM, storage, URL finale ;
- nettoyage d'expiré n'efface jamais une session voisine.

### 10.4 Rustls/release

- sélection explicite `ring` installée avant toute construction TLS ;
- build package ciblé et build workspace ne paniquent pas ;
- test ACME/relay réel avec le binaire release ;
- graphe des features documenté ;
- aucun artefact debug accepté par le gate production.

## 11. Ordre de développement et commits

Ordre strict, un seul lead sur `aithos-gateway` :

1. **P0 — audit/attribution** : état disque/process, changements étrangers,
   baseline exacte, espace disque et toolchains ; aucun format global.
2. **P1 — contrats seuls** : Gherkin cérémonie/session/durabilité + surface WASM
   et CLI ; commit RED étroit.
3. **P2 — correction bootstrap TLS** : sélection explicite du CryptoProvider,
   tests workspace/ciblé ; commit isolé.
4. **P3 — state store AS** : traits, impl mémoire test, impl Vault production,
   migration de DCR/refresh sans changer le flow heureux ; restart tests.
5. **P4 — Core bridge SC1** : consommer `verify_session` et
   `verify_max_sessions`, aucune réécriture du Core.
6. **P5 — cérémonie gateway** : pending/challenge, leaf vers gateway_pub, SC1,
   liaison code→sid, révocation.
7. **P6 — WASM pubkey-first** : verify/build/sign, keystore chiffré et page
   WYSIWYS ; scan navigateur.
8. **P7 — multi-principal G5** : token→session, surface/authorize/log par chaîne,
   isolation et comptages.
9. **P8 — CLI équivalent** : même primitives et flow OAuth scriptable.
10. **P9 — E2E** : navigateur générique puis vrai Cowork, restart et révocation.
11. **P10 — release/ops** : build propre, image/binaire signé, SBOM, runbooks,
    canary et rollback.
12. **P11 — handoff DONE** : commits, hashes, résultats exacts, limites, aucun
    secret.

Commits recommandés : contrats ; rustls ; state store ; bridge SC1 ; cérémonie ;
WASM/page ; multi-principal ; CLI/E2E ; ops/docs. Ne jamais mélanger plusieurs
dépôts dans un commit.

## 12. Interdits d'implémentation

- `keyholder.rs` et `credentials.rs` ne changent pas sans STOP et nouveau
  cadrage ;
- aucune clé de personne dans la gateway ;
- aucune clé de session ou token chez le provider Aithos ;
- aucune autorité basée seulement sur un JWT ;
- aucune signature/JCS/atténuation maison en TypeScript ;
- aucune exception SC1 pour Cowork ;
- aucun appel upstream avant autorisation, preuves et log-before-relay ;
- aucun secret dans YAML, argv, screenshots, handoff ou fixture ;
- aucune migration destructive de Vault ;
- aucune réinterprétation des artefacts historiques ;
- aucun `git add .`, stash/reset/checkout destructif ou format global ;
- aucune action métier destructive pendant les gates.

## 13. Qualification production

### 13.1 Build

- source propre, commit/tag exact, lockfile attribué ;
- `cargo build --release --locked` ciblé et workspace ;
- suites complètes, clippy `-D warnings`, fmt ciblé ;
- SHA-256, provenance, SBOM et signature de l'artefact ;
- reproduction indépendante du hash ou justification des écarts ;
- aucun binaire sous `target/debug` ou répertoire de démo.

### 13.2 Vault entreprise

- Vault non-dev, TLS ou listener loopback strict, stockage durable ;
- procédure d'initialisation/unseal et séparation des recovery materials ;
- root token révoqué/retiré de l'exploitation courante ;
- AppRole/cert/auth machine ou mécanisme renouvelable de production ;
- politiques séparées : credentials connecteurs, AS state, session keys ;
- renouvellement de leases supervisé ;
- backup/restauration et exercice de restauration ;
- audit device sans payload secret ;
- rotation adapter key/session namespaces documentée.

### 13.3 Runtime gateway

- compte OS dédié, répertoires 0700 et fichiers secrets 0600 ;
- service manager avec restart/backoff et limites ;
- egress minimal vers relay/store/witness/ACME/connecteurs approuvés ;
- aucun port entrant requis hors politique entreprise ;
- readiness séparée process/Vault/tunnel/cert/AS state ;
- métriques sans query, bearer, body ou path Vault ;
- alertes certificat, tunnel, Vault lease, refresh failures et refus anormaux ;
- horloge NTP surveillée ;
- logs bornés, rotation et rétention.

### 13.4 Environnements

Ne pas promouvoir `demo` en place.

```text
dev     : tenant/hostname actuels, données de démonstration
staging : tenant et hostname jetables dédiés, ACME staging d'abord
prod    : nouveau tenant entreprise, nouveau hostname, policies et Vault prod
```

Toute création/binding/purge de tenant, mutation DNS/control plane, certificat
Let's Encrypt production, déploiement ou publication exige une autorisation
explicite et un runbook avec rollback.

### 13.5 Canary

Le premier connecteur production expose un seul outil read-only. Le canary :

1. enrôle une pubkey de test ;
2. émet un mandat minimal ;
3. connecte un client OAuth générique ;
4. redémarre la gateway et confirme la session ;
5. appelle une lecture ;
6. confirme un refus voisin avant Vault/amont ;
7. révoque et confirme la coupure ;
8. vérifie Gamma hors ligne ;
9. scanne logs/metrics/traces ;
10. seulement ensuite autorise Cowork et des périmètres supplémentaires.

## 14. Gates de sortie

### Gate A — protocole/local

- tous les contrats RED puis GREEN ;
- vecteur SC1 historique inchangé et consommé ;
- aucune retouche Core, ou rituel vectors-first séparé si défaut prouvé ;
- deux sessions simultanées, surfaces et comptages isolés ;
- restart et refresh rotation verts ;
- révocation coupe le prochain acte ;
- quatrième session refusée ;
- zéro hit broker/upstream sur tous les refus d'autorité.

### Gate B — navigateur générique

- navigateur vierge, pubkey-first, mandat réel, cérémonie < 2 minutes hors délai
  d'approbation owner ;
- WYSIWYS exact ;
- aucune donnée sensible dans DOM/storage/console/network capture ;
- code et challenge replay refusés ;
- session visible dans certs/preuves sans seed.

### Gate C — Cowork réel

- ajout du custom connector public ;
- DCR + PKCE + cérémonie ;
- `briefing.read` puis un seul outil read-only mandaté ;
- voisin hors surface et appel forcé refusé ;
- audit exact ;
- restart puis nouvel appel ou refresh réussi ;
- révocation live puis refus au prochain appel.

### Gate D — release/staging

- binaire release signé, Rustls stable dans les deux modes de build ;
- Vault production-like avec backup/restore ;
- tunnel/cert staging, reconnect et renouvellement ;
- canary deux fois depuis état frais ;
- témoin adversarial indépendant ;
- rollback exécuté, pas seulement documenté.

### Gate E — production

Gate humain explicite après revue des preuves A–D. Alors seulement : tenant et
hostname prod, ACME prod, release publiée, service activé. Aucun élargissement de
mandat lors du déploiement.

## 15. Conditions de STOP

STOP documenté, sans contournement, si :

- le Core/SC1 ne permet pas la couture décrite sans changer un wire ;
- la gateway ne peut pas produire les deux preuves sur le même `operation_ref` ;
- Cowork exige un comportement hors OAuth/MCP documenté ;
- une session ne peut pas survivre au restart sans exposer sa clé ;
- une révocation ne devance pas un token encore valide ;
- une contrainte est élargie ou non vérifiable ;
- un secret apparaît dans sortie, log, trace, URL ou stockage navigateur ;
- Vault indisponible conduit à un mode permissif ;
- attribution d'un changement, process, tenant ou artefact incertaine ;
- le travail entre en conflit avec `aithos-client`, SDK, provider ou CLI étranger ;
- le gate requiert une action externe destructive ;
- déploiement/publish/push/merge n'a pas été explicitement autorisé.

## 16. Définition de fini

Le lot « G4/G5 production MCP » est fini seulement si :

- le bouton DEV n'existe pas en profil production ;
- une vraie clé de délégué autorise une session courte sans quitter son custody ;
- la feuille de session est strictement atténuée et liée à `gateway_pub` et
  `session_bind` ;
- chaque opération passe le verifier SC1 double-proof ;
- OAuth sélectionne une session durable, jamais une autorité implicite ;
- les surfaces et audits sont distincts par session ;
- les TTL 2m/15m/8h et la borne 3 sont appliqués ;
- restart, refresh, révocation et anti-rejeu sont verts ;
- vrai Cowork passe appel sûr, refus, audit et coupure ;
- release, Vault, supervision, backup/restore et rollback sont qualifiés ;
- aucun secret, seed, token, code ou payload n'a fui ;
- le tenant `demo` reste identifié DEV ;
- un gate humain autorise séparément le déploiement production.

## 17. Prompt de reprise

> Reprendre le lot G4/G5 et la qualification production depuis
> `/Volumes/Math17/aithos/v2/code/aithos-core` en suivant intégralement
> `docs/HANDOFF-GATEWAY-G4-PROD-MCP-DELEGATED-SESSIONS-2026-07-22.md`.
> Lire toutes les références obligatoires dans l'ordre du §4 avant tout contrat
> ou code. État disque et processus = vérité ; préserver tous les changements,
> services, tenants, credentials et artefacts hérités. Commencer par P0, attribuer
> chaque fichier et revalider la baseline. Une seule session de développement
> possède `aithos-gateway` à la fois. Écrire et committer d'abord les contrats RED
> cérémonie/session/durabilité et les surfaces WASM/CLI, puis suivre strictement
> l'ordre §11. Consommer SC1/W1.1, `verify_session` et `verify_max_sessions` sans
> réinventer ni modifier leur wire. Profil cible : MCP OAuth compatible,
> pubkey-first, clé personne locale, feuille vers `gateway_pub`, clé de session
> gateway éphémère conservée au Vault, access 15 min, session/refresh 8 h,
> maximum 3 sessions. Le Bearer sélectionne `sid` ; chaîne, révocation, tool,
> arguments, obligations et double-proof sont revérifiés avant chaque effet.
> `keyholder.rs`/`credentials.rs` restent intouchables ; aucun secret dans argv,
> YAML, logs, DOM ou handoff. Corriger explicitement le bootstrap Rustls et
> qualifier les builds release ciblé/workspace. Gates : navigateur générique,
> vrai Cowork, restart, refresh replay, multi-session, refus avant Vault/amont,
> révocation live, audit, anti-fuite, staging, backup/restore et rollback. Aucun
> AWS/DNS/control-plane prod, publication, push ou merge sans autorisation
> explicite. Toute divergence normative, fuite, élargissement d'autorité ou
> conflit d'attribution impose STOP documenté.

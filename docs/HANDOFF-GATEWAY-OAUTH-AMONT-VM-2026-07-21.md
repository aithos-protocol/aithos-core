# HANDOFF + PROMPT DE REPRISE — Lot « OAuth amont » de la gateway (sur VM)

> **REMPLACÉ.** Le socle décrit ici est livré et documenté dans
> `HANDOFF-GATEWAY-UPSTREAM-OAUTH-DONE-2026-07-21.md`. La suite générique
> discovery/DCR/connecteurs SaaS est désormais pilotée par
> `HANDOFF-GATEWAY-OAUTH-CONNECTEURS-SAAS-2026-07-22.md`. Conserver ce fichier
> uniquement comme état d'entrée historique ; ne pas exécuter son prompt de
> reprise.

Date : 2026-07-21. Dépôt : `code/aithos-core` (crate `aithos-gateway`).
À coller en début de session. État DISQUE = vérité — ne fais confiance à
AUCUN résumé sans vérification. Même rituel que les gates provider
(BDD RED → code minimal → VERT → témoin adversarial → gravure/handoff →
commits = geste de Mathieu ; worktree partagé avec l'agent aithos-client
→ commits ISOLÉS).

## 0. But du lot, en une phrase

Donner à la gateway un **client OAuth 2.1 amont** : pouvoir s'authentifier
auprès d'un MCP (ou d'une API) protégé par OAuth — flux authorization
code + PKCE, tokens rangés dans le **Vault du client**, rafraîchis
automatiquement, injectés par appel. C'est le morceau qui débloque les
MCP hébergés modernes (Notion hébergé, Gmail, etc.) et qui sous-tend le
futur « connecter un connecteur » du dashboard.

## 1. Décisions déjà prises (ne pas re-arbitrer)

- **OAuth par utilisateur (3-legged)**, pas domain-wide delegation — le
  plus démonstratif (l'écran de consentement Google est visible en démo)
  et le plus universel (marche aussi Notion/GitHub).
- **L'app OAuth appartient au CLIENT** (son projet Google Cloud / son
  service account), jamais à Aithos. Le `client_id` est de la config ; le
  `client_secret` est une **référence Vault**, jamais en clair.
- **Mode « Testing »** côté Google pour la démo : Mathieu s'ajoute comme
  *test user* → **vérification CASA contournée**, `gmail.send` marche
  immédiatement (refresh token expire à 7 j — sans importance en démo).
- **Cible d'exécution : une VM publique.** Conséquence CENTRALE (voir §2) :
  le `redirect_uri` n'est PAS en loopback mais sur le **hostname HTTPS
  public de la VM** → type de client OAuth = **« Web application »**
  (pas « Desktop »). Le consentement s'affiche quand même dans le
  navigateur de l'opérateur — la valeur démo est préservée.
- **Doctrine, non négociable** : le token (access + refresh) vit dans le
  **Vault du client**, JAMAIS chez Aithos, JAMAIS dans la config, JAMAIS
  dans un log (discipline `credentials.rs`). Aithos ne tient rien.
- Le **même endpoint callback** servira plus tard le dashboard — le coder
  générique dès maintenant (pas de chemin jetable).

## 2. Le lot technique

Le crate `aithos-gateway`. La brique d'auth amont existante est
`UpstreamAuth` dans `src/proxy_mcp.rs` (`None | InlineBearer | Brokered`)
+ le seam `CredentialBroker` de `src/credentials.rs` (Vault-kv2). On
AJOUTE une variante OAuth, sans casser l'existant.

### 2.1 Config (`src/config.rs`)
Une nouvelle forme d'auth serveur, ex. sous `servers[].credential` ou un
bloc `oauth`. Champs : `auth_url`, `token_url` (ou découverte par
metadata, cf. 2.5), `client_id`, `client_secret` (réf Vault),
`scopes: [...]`, `redirect_uri` (le callback public de la VM),
`token_vault_path` (où ranger access/refresh/expiry). Validation
fail-closed comme le reste (`from_yaml`).

### 2.2 Le module client OAuth (`src/upstream_oauth.rs`, nouveau)
- Flux **authorization code + PKCE** (RFC 7636).
- `build_consent_url()` → l'URL que l'opérateur ouvre.
- `exchange_code(code)` → POST token endpoint → `{access, refresh,
  expires_in}` → **écrit dans Vault** (via le broker).
- `access_token()` → lit Vault ; si expiré, **refresh** (POST token
  endpoint avec `grant_type=refresh_token`) → réécrit Vault → rend le
  nouvel access. Fail-closed : pas de token / refresh échoué → erreur,
  JAMAIS d'appel amont non authentifié.

### 2.3 Le callback (route sur le serveur axum de la gateway)
La gateway sert déjà `/mcp` (axum, `src/proxy_mcp.rs`/`main.rs`). AJOUTER
une route `/oauth/callback` sur le MÊME serveur : elle reçoit `?code=...`,
appelle `exchange_code`, affiche une page « connecté, vous pouvez fermer ».
Sur la VM, caddy route déjà 443 → la gateway, donc
`https://<vm-host>/oauth/callback` atteint cette route.

### 2.4 La commande owner (`src/main.rs`)
`owner-connect-oauth --server <nom> [--store-root/--config ...]` :
imprime l'URL de consentement (l'opérateur l'ouvre dans son navigateur),
attend le callback, échange, confirme « <server> connecté ». Un geste
owner, une fois par connecteur.

### 2.5 Runtime (`src/proxy_mcp.rs`, `HttpUpstream`)
Nouvelle branche `UpstreamAuth::OAuth { … }` dans `authorize()` (la
fonction que le fix SSE d'aujourd'hui a introduite) : résout l'access
token (avec refresh transparent) et pose `Authorization: Bearer <access>`.
Comme le brokered : résolution au dernier moment, le secret ne survit pas
à la requête. Compose avec le décodage SSE + session déjà livré.

### 2.6 Découverte (optionnel mais recommandé — la voie MCP standard)
La norme MCP OAuth (2025) : le serveur MCP publie sa *protected resource
metadata* (RFC 9728) → l'AS metadata (RFC 8414) → registration dynamique
(RFC 7591). L'implémenter rend la gateway compatible avec TOUT MCP OAuth
conforme (Notion hébergé en tête) sans config manuelle des URLs. Si trop
long pour le premier gate : câbler `auth_url`/`token_url` en config
d'abord, la découverte en incrément.

### 2.7 ⚠ Fork à confirmer AVANT de coder la cible Gmail
« Gmail via MCP » a deux formes — trancher avec Mathieu :
- **(A) MCP hébergé protégé par OAuth** (ex. Notion `mcp.notion.com`) : le
  endpoint MCP LUI-MÊME exige OAuth → **c'est exactement ce lot**. Notion
  hébergé est la cible OAuth la plus propre pour prouver le mécanisme.
- **(B) MCP Gmail auto-hébergé** qui tient lui-même les creds Google : la
  gateway lui parle en **bearer statique** (comme Notion/GitHub
  aujourd'hui) et l'OAuth Google se passe DANS le serveur MCP, pas dans la
  gateway → ce lot n'est PAS requis pour ça.
Le produit vise (A) (« connecter un connecteur en OAuth »). Recommandation :
**prouver le lot sur un MCP OAuth conforme (Notion hébergé)**, puis traiter
Gmail selon la forme retenue. Ne pas présumer qu'un « Gmail MCP hébergé
OAuth » officiel existe sans le vérifier.

## 3. Garde-fous (doctrine + appris)

- Tokens dans le **Vault du client** uniquement ; discipline `credentials.rs`
  (aucun token dans un log, une erreur, une sortie CLI). Test anti-fuite.
- `client_secret` = réf Vault, jamais en clair dans le yaml (grep = 0,
  comme la démo).
- Fail-closed partout : pas de token → refus (jamais d'appel amont nu).
- **Worktree partagé** avec l'agent aithos-client : committer CE lot seul
  (`upstream_oauth.rs`, deltas ciblés `config.rs`/`proxy_mcp.rs`/`main.rs`),
  message dédié — ne rien mélanger à la verticale SDK.
- **PRÉREQUIS — vérifier d'abord le fix SSE livré aujourd'hui** (voir §5) :
  il est sur le disque mais N'A PAS été passé à la suite complète ni
  committé. Le confirmer VERT avant d'empiler.

## 4. Preuves attendues

**En process (le gate, RED→VERT) :** un faux AS in-process (endpoints
authorize + token) + un faux resource server qui vérifie le bearer.
Prouver : le consentement produit des tokens rangés en Vault ; le runtime
injecte l'access ; un access expiré déclenche le refresh ; un refresh
échoué refuse fail-closed ; aucun token dans les logs. + suite gateway
non régressée (cucumber 152, e2e_demo_lea, lib), fmt/clippy `-D warnings`.

**Live (chez Mathieu / sur la VM — le sandbox ne peut PAS) :** le vrai
consentement Google dans le navigateur, le token réel en Vault, un appel
réel (`get_me` / envoi) qui passe par la gateway sous mandat, journalisé,
témoin. C'est la clôture — elle t'appartient.

## 5. Prérequis à exécuter AVANT le lot

### 5.1 Confirmer le fix SSE d'aujourd'hui (livré, non vérifié en suite)
Fichiers déjà sur le disque : `crates/aithos-gateway/src/proxy_mcp.rs`
(décodage SSE + `Mcp-Session-Id` + poignée initialize) et `rust/Cargo.toml`
(`reqwest` features `["json","rustls-tls"]`). Prouvé en direct aujourd'hui
(discovery GitHub 44 outils, `get_me` réel, refus `-32001`), mais pas passé
à la batterie. Faire :
```
cd rust
cargo test -p aithos-gateway --lib upstream_transport   # les tests SSE ajoutés
cargo test -p aithos-gateway --test e2e_demo_lea         # non-régression JSON
cargo test -p aithos-gateway --test cucumber             # 152/152
cargo clippy -p aithos-gateway --all-targets -- -D warnings
```
Puis commit isolé : « gateway: compatibilité transport streamable HTTP —
décodage SSE, Mcp-Session-Id, poignée initialize, rustls-tls ». C'est la
fondation du lot OAuth (l'OAuth réutilise `authorize()` introduit ici).

### 5.2 Setup Google Cloud (Mathieu, ~15 min, une fois) — cible VM
1. Google Cloud Console → nouveau projet (gratuit).
2. **Activer l'API** visée (Gmail API pour l'envoi de mail).
3. **OAuth consent screen** : type « External », statut **Testing**,
   ajouter Mathieu comme **test user**. Scopes : le minimum (`gmail.send`
   pour envoyer ; `gmail.readonly` pour lire).
4. **Credentials → Create OAuth client ID → type « Web application »**.
   Redirect URI autorisé : **`https://<hostname-de-la-VM>/oauth/callback`**
   (le hostname public que caddy sert — cf. runbook VM). Récupérer
   `client_id` + `client_secret`.
5. Le `client_secret` ira dans le **Vault de la VM**
   (`vault kv put secret/aithos/oauth/<server> client_secret=...`), jamais
   dans le yaml.

> Note VM vs local : en local (gateway sur le poste), le client OAuth
> serait de type « Desktop » avec redirect loopback
> (`http://127.0.0.1:PORT/oauth/callback`). Sur VM, c'est « Web
> application » + le callback HTTPS public. Le module `upstream_oauth`
> doit rendre le `redirect_uri` CONFIGURABLE pour couvrir les deux.

## 6. État acquis au 2026-07-21 (contexte de reprise)

- Gateway locale prouvée de bout en bout contre un vrai MCP moderne
  (GitHub via Vault) : `tools/list` = la surface du mandat, `get_me` réel,
  `delete_file` refusé `-32001`. Ethos journal (mode B) + contexte (mode A)
  seedés sur `store.aithos.fr` par `owner-replicate-history`, observés par
  le témoin. Le fix SSE (§5.1) a rendu ça possible.
- Provider P3/P4 déployé et au repos (store 2/2, relay 1/1, witness 1/1 ;
  `store/public/witness .fr` = 200). Lot B clos (handoff
  `HANDOFF-PROVIDER-P3P4-DONE-2026-07-21.md`).
- Runbooks démo : `DEMO-GATEWAY-GENERIQUE.md` (ton agent, tes MCP) et
  `GUIDE-GATEWAY-DEMO-LOCALE.md` (gestion/dépannage).
- Après CE lot : l'entrée distante « via aithos.fr » — soit **VM publique**
  (DNS + caddy TLS, retenu pour la démo, conforme « sortie libre » §5), soit
  **G1 relais** (produit ; le relais est déployé, `pod_stub` = référence
  client). Puis le **dashboard-OAuth** (le flux validé : le token atterrit
  à la gateway, jamais chez Aithos ; dashboard statique = chef d'orchestre,
  gateway = coffre).

## 7. Le prompt de reprise (à coller tel quel)

> Reprends le lot « OAuth amont » de la gateway `aithos-gateway`, cible VM
> publique, en suivant ce handoff. Ordre : (0) rituel d'entrée, état DISQUE
> = vérité. (1) PRÉREQUIS §5.1 — vérifier/committer le fix SSE d'aujourd'hui
> (suite gateway VERTE + clippy) ; ne rien empiler tant que ce n'est pas
> vert. (2) BDD/tests RED d'abord : faux AS + resource server in-process,
> les cas du §4. (3) code minimal : module `upstream_oauth.rs` (auth code +
> PKCE, tokens en Vault, refresh, fail-closed), route `/oauth/callback`,
> commande `owner-connect-oauth`, branche `UpstreamAuth::OAuth` dans
> `authorize()`. `redirect_uri` CONFIGURABLE (VM = Web app + callback HTTPS
> public ; local = Desktop + loopback). (4) VERT complet + non-régression
> (cucumber 152, e2e_demo_lea) + fmt/clippy `-D warnings` + test anti-fuite
> de token. (5) témoin adversarial (confidentialité du token, fail-closed
> sur refresh, doctrine « Aithos ne tient rien »). (6) handoff DONE + blocs
> de commit ISOLÉS (worktree partagé avec aithos-client) + write-back. La
> preuve LIVE (consentement Google réel, envoi réel) reste le geste de
> Mathieu sur la VM — fournir le runbook. AVANT de coder la cible Gmail,
> trancher le fork §2.7 avec Mathieu (MCP OAuth hébergé type Notion vs MCP
> Gmail auto-hébergé). Doctrine absolue : le token vit dans le Vault du
> client, jamais chez Aithos, jamais en config, jamais en log.

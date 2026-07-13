# Aithos — Compatibilité avec les standards agentiques & identité

> **Statut : DRAFT v0.1 (interne, FR).** Analyse de compatibilité entre le protocole
> Aithos Core (`aithos-core: 1.0.0-draft.1`) et les standards d'interopérabilité /
> d'identité pour agents IA — établis et émergents. **Objectif contraint** : rendre
> Aithos interopérable avec ces standards **sans changer un seul cas d'usage, un seul
> comportement, un seul invariant (I1–I5)**. Toute compatibilité est **additive** :
> adaptateurs à la gateway, profils d'export, ponts de crédentiels — jamais une
> modification de la sémantique du core.
> Complète : `DESIGN.md` (rationale), `spec/00–10` (normatif),
> `docs/GATEWAY-BOOTSTRAP.md` / `GATEWAY-HANDOFF.md` (le runner),
> `docs/DEPLOYMENT-CONTAINMENT.md` (topologie). Initiée 2026-07-12.
>
> **Méthode.** État de l'art au 2026-07-12, bâti sur sources primaires datées (specs,
> drafts IETF/OpenID/W3C, repos et annonces officiels), avec vérification adversariale
> multi-agents pour le cœur (MCP, A2A, XAA/ID-JAG) et lecture directe des sources pour
> le reste. Chaque affirmation porte son niveau de confiance :
> **[A]** vérifiée contre la source primaire par 3 vérificateurs indépendants (3-0) ;
> **[B]** lue sur la source primaire (citation extraite, un seul passage de lecture) ;
> **[C]** inférence de conception — cohérente avec les specs mais à prototyper.
> Les URL et dates sont en §9.

## 0. TL;DR

1. **Aucun changement du core n'est nécessaire** pour être compatible avec tout ce
   qui compte en 2026. Chaque point de contact identifié se traite à l'étage
   gateway/outillage (adapter, profil d'export, pont de crédentiels). Le wire, les
   invariants I1–I5 et les vecteurs gelés ne bougent pas — et aucun standard
   examiné n'exige le contraire.
2. **Trois standards dominent et sont mûrs** : **MCP** (rév. 2025-11-25, autorisation
   OAuth 2.1, >10 000 serveurs, gouvernance Linux Foundation/AAIF depuis déc. 2025),
   **A2A 1.0.0** (Linux Foundation, 150+ organisations, les trois hyperscalers) et le
   pont d'identité d'entreprise **XAA/ID-JAG** (draft IETF actif porté par Okta,
   intégré à MCP par l'extension SEP-990, clients beta : Claude, VS Code…).
   C'est la cible prioritaire du plan d'action.
3. **« NAS (Linux Foundation) » se décompose en deux réalités** : (a) **ANS — Agent
   Name Service** est une proposition OWASP GenAI (whitepaper mai 2025), dont le
   draft IETF v1 a expiré et dont la **v2** (avril 2026, ancrage domaine + logs de
   transparence SCITT) est un draft individuel actif — early, à suivre, pas à
   implémenter ; (b) la **Linux Foundation** héberge bien les standards agentiques,
   mais via **A2A** (juin 2025), **AGNTCY** (juil. 2025) et l'**AAIF** qui a reçu
   **MCP** d'Anthropic (déc. 2025).
4. **L'écart structurel unique** entre Aithos et ce paysage : eux = *bearer tokens*
   courts émis online ; nous = *certificats holder-of-key* délégables et vérifiables
   offline. La réconciliation sanctionnée par les specs : **échanger le mandat contre
   un token court, audience-borné, à chaque frontière** — ce que l'interdiction de
   passthrough de MCP impose de toute façon à tout le monde. Notre chaîne de mandats
   reste la source de vérité ; le token n'est qu'une projection périssable.
5. **Trois convergences gratuites** (déjà dans le wire Aithos) : Ed25519 partout
   (A2A card signing via JWS/EdDSA, UCAN, Biscuit, NANDA) ; l'encodage multibase/
   multicodec `z…` **identique à `did:key`** (étape 0 du plan d'exécution) ; et
   **JCS RFC 8785**, que la signature des Agent Cards A2A exige aussi. Les profils
   d'export identité (did:key, did:web, JWKS) sont quasi gratuits.
6. **Le gamma n'a pas d'équivalent normatif** chez MCP/A2A/XAA (l'audit y est hors
   périmètre) : c'est un différenciateur, pas un conflit. La seule convergence à
   surveiller : les logs de transparence SCITT adoptés par ANS v2 — même famille
   d'idées que nos racines committées (§07.10).

## 1. Principe directeur et anti-objectifs

**Le core ne bouge pas.** La règle du repo (« le gateway consomme aithos-core en
bibliothèque ; le core ne bouge pas ») s'applique ici à l'identique : la
compatibilité standards est un travail de **couche d'adaptation**, au même étage que
`proxy_mcp` / `proxy_llm` — jamais un chantier de spec core. Trois mécanismes, par
ordre de préférence :

1. **Adapter (gateway)** — parler le protocole externe sur le fil, traduire vers les
   primitives Aithos (`verify_op`, `log_action`, mandats, vault). Modèle :
   `proxy_mcp` existant.
2. **Profil d'export (représentation)** — publier un objet Aithos existant dans un
   format standard, signé, **sans seconde source de vérité** (le bundle reste
   autoritaire ; l'export est dérivé, régénérable, daté). Modèle : DID document →
   représentation W3C.
3. **Pont de crédentiels (échange à la frontière)** — convertir un crédential
   externe entrant (token OAuth, ID-JAG) en entrées du modèle Aithos (vault,
   provisioning de mandat), et projeter un mandat sortant en crédential externe
   (access token court, VC, assertion).

**Anti-objectifs.** Aucune adaptation ne doit : (a) introduire un serveur comme
partie de confiance (DESIGN §7) ; (b) toucher aux invariants I1–I5 ni aux vecteurs
gelés ; (c) faire du wire Aithos un dialecte d'un standard externe ; (d) créer une
seconde source de vérité (un token ou une carte qui « vaudrait » sans le mandat
derrière). Quand un standard exige un comportement incompatible (ex. révocation
uniquement online), la réponse est un **profil dégradé documenté à la frontière**
(TTL courts, re-vérification à chaque usage), jamais une concession du core.

**Pourquoi c'est tenable.** Le pattern « couche de confiance interne + émission de
crédentiels standards aux frontières » est exactement celui que le paysage 2026
sanctionne : MCP délègue explicitement l'authorization server à une entité séparée
[A], interdit le passthrough de tokens [A] (donc impose la ré-émission à chaque
saut — structurellement congruent avec notre atténuation par lien), et laisse le
transport STDIO hors de son cadre OAuth (« retrieve credentials from the
environment » [A]) — le vide exact que notre gateway occupe déjà.

## 2. Ce qui ne bouge pas (rappel opposable)

| Invariant / décision | Conséquence pour l'interop |
|---|---|
| I1 — pas de secret stocké | les tokens externes entrants vivent dans le vault `/x/<id>`, comme aujourd'hui |
| I2 — crédentiels immuables | on ne « met pas à jour » un mandat pour suivre un standard ; on émet des projections |
| I3 — ligne owner | aucun format externe ne devient un chemin d'accès qui contournerait les headers |
| I4 — autorité = émission | aucune API externe ne peut révoquer/émettre ; elle *demande*, le détenteur d'autorité agit |
| I5 — pas d'action silencieuse | tout appel entrant via un adapter (MCP, A2A, OAuth) produit son entrée gamma, comme `proxy_mcp` aujourd'hui |
| Serveur = jamais partie de confiance | l'AS OAuth de la gateway émet des projections *vérifiables contre le bundle*, il ne détient aucune vérité propre |
| Vérification offline | les tokens émis ont TTL ≤ fenêtre de fraîcheur ; l'état révocation reste le gamma, pas une CRL |
| Wire figé (JCS, multibase, Ed25519) | c'est un *atout* d'interop, pas une dette — voir §5.3 |

## 3. Les sept surfaces d'ancrage

Là où Aithos rencontre le monde extérieur. Chaque surface a un propriétaire dans
l'architecture existante ; aucune n'est dans le core-crypto.

| # | Surface | Objets Aithos | Propriétaire | Standards concernés |
|---|---|---|---|---|
| S1 | **Identité** | `did:aithos:…`, DID document (§01.4), clés Ed25519/X25519 multibase | spec §01 + outillage | W3C DID 1.1 (did:key, did:web), JWKS, CIMD, SPIFFE |
| S2 | **Crédentiels / mandats** | mandat-certificat (§04), chaînes (§05) | spec §04–05 + gateway | OAuth 2.1 (+ RFC 8693/9396/8707), ID-JAG, VC 2.0 / SD-JWT VC, AP2, (UCAN/Biscuit/GNAP en parenté) |
| S3 | **Exécution d'outils** | connecteurs `act.x.*` (§08), `proxy_mcp`, `McpRouter` | gateway (fait) | MCP (client & serveur, authz OAuth 2.1) |
| S4 | **Agent ↔ agent** | rien de dédié aujourd'hui | gateway (à créer) | A2A 1.0 (Agent Card, tasks, 3 bindings, extensions) |
| S5 | **Secrets sortants & SSO entreprise** | vault `/x/<id>` (§08.2) | gateway + spec §08 | OAuth 2.1 client, XAA/ID-JAG, IPSIE (profils), DPoP |
| S6 | **Audit / preuve** | gamma (§07), racines committées (§07.10), `read.gamma` | spec §07 | aucun équivalent normatif ; convergence SCITT (ANS v2) à suivre |
| S7 | **Découverte / nommage** | `bundle` locations du DID doc | gateway (à créer, optionnel) | A2A Agent Card well-known, MCP Registry, AGNTCY, NANDA, ANS |

Lecture : la compatibilité **prioritaire** (S3, S5, S4) est celle qui touche des
standards *déployés* ; S1 est un multiplicateur quasi gratuit ; S2 en représentation
(VC) et S7 sont opportunistes ; S6 est un différenciateur à défendre tel quel.

## 4. Le paysage 2026, standard par standard

### 4.1 MCP — Model Context Protocol (l'outil ↔ l'agent)

**Statut & gouvernance.** Lancé par Anthropic en novembre 2024 ; révision courante
**2025-11-25** (succède à 2025-06-18), exigences normatives dérivées du schéma
TypeScript versionné du repo officiel [A]. Un **release candidate 2026-07-28** est
annoncé (gelé le 21 mai 2026 : validation d'issuer RFC 9207, CIMD, guidance refresh
tokens) — il **ne change rien** au modèle bearer/no-passthrough [A]. Depuis le
9 décembre 2025, MCP est un projet fondateur de l'**Agentic AI Foundation (AAIF,
Linux Foundation)**, aux côtés de goose (Block) et AGENTS.md (OpenAI) ; membres
platinum : AWS, Anthropic, Block, Bloomberg, Cloudflare, Google, Microsoft, OpenAI ;
gold notamment : Okta, IBM, Salesforce, Cisco, Oracle, SAP [B, vérifié sur le
communiqué LF]. Adoption : >10 000 serveurs MCP publiés, clients Claude, Cursor,
Microsoft Copilot, Gemini, VS Code, ChatGPT [B]. C'est le standard agent↔outil de
facto — et notre gateway en parle déjà le dialecte (Streamable HTTP, JSON-RPC).

**Ce que sa couche d'autorisation normalise (rév. 2025-11-25)** [A] :

- Serveur MCP = **resource server OAuth 2.1** (draft-ietf-oauth-v2-1-13 cité par la
  spec ; OAuth 2.1 lui-même est toujours un draft IETF, -15 en juillet 2026) ;
  client MCP = client OAuth 2.1 ; **l'authorization server peut être une entité
  séparée** (« may be hosted with the resource server or a separate entity »).
- Découverte : **RFC 9728** Protected Resource Metadata (MUST serveur,
  `authorization_servers` désigne l'AS) + **RFC 8414** AS Metadata (ou OIDC
  Discovery). Dynamic Client Registration (RFC 7591) rétrogradé SHOULD→MAY ;
  **CIMD** (OAuth Client ID Metadata Documents, draft-00) ajouté en SHOULD : le
  `client_id` peut être une URL HTTPS pointant un document JSON de métadonnées
  auto-hébergé (SEP-991).
- Jetons : **bearer pur** (aucun sender-constraining dans le cœur), header
  `Authorization: Bearer` obligatoire, jamais en query string ; **RFC 8707
  Resource Indicators MUST** côté client (paramètre `resource` = URI canonique du
  serveur, dans la requête d'autorisation ET de token) ; le serveur MUST valider
  l'audience et répondre 401 sinon ; **no-passthrough** : « MCP servers MUST NOT
  accept or transit any other tokens » — tout appel amont exige un token distinct.
- **PKCE obligatoire** (S256 ; refus si `code_challenge_methods_supported` absent).
- **L'autorisation est OPTIONNELLE et HTTP-only** : « Implementations using an
  STDIO transport SHOULD NOT follow this specification, and instead retrieve
  credentials from the environment. »
- Extension **SEP-990 « Enterprise-Managed Authorization »** = Cross App Access /
  ID-JAG intégré à MCP (repo `modelcontextprotocol/ext-auth`, versionné hors cœur ;
  promu stable courant 2026, adopté par Anthropic, Microsoft, Okta) [A].
- Nouveauté 2025-11-25 : **Tasks** (SEP-1686), suivi expérimental des travaux longs [A].

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| La gateway **serveur MCP protégé** | rester RS OAuth 2.1 : publier RFC 9728, accepter des Bearer émis par notre propre AS | [C] sanctionné par [A] |
| **AS OAuth adossé aux mandats** (le chantier clé, §6-C1) | `verify_chain` → mint access token court, audience-borné RFC 8707, scopes/`authorization_details` dérivés du périmètre | [C] |
| La gateway **client MCP** vers serveurs tiers protégés | flux OAuth 2.1+PKCE, tokens tiers stockés au vault `/x/<id>` — modèle §08.2 inchangé | [C] trivial |
| **STDIO carve-out** | nos serveurs MCP locaux lancés par la gateway reçoivent leurs credentials « de l'environnement » = le vault ; on est déjà exactement dans le cadre | [A] |
| **CIMD / SEP-991** | publier pour chaque agent un document client `https://…` auto-hébergé — même esprit que did:web ; réutilise l'export JWKS (§6-C4) | [B]+[C] |
| **SEP-990 / XAA** | voir §4.3 — c'est le pont SSO entreprise | [A] |

**Écarts de modèle** : bearer vs holder-of-key (§5.1) ; révocation par expiration de
token vs notre échelle (§5.2) ; aucun vocabulaire de contraintes (fenêtres, budgets,
obligations) — nos contraintes restent enforced par la gateway à *chaque* usage du
token, le token n'étant qu'un laissez-passer d'entrée (§5.1). Rien de bloquant.

### 4.2 A2A — Agent2Agent (l'agent ↔ l'agent)

**Statut & gouvernance.** Lancé par Google le 9 avril 2025 (50+ partenaires) ;
donné à la **Linux Foundation** le 23 juin 2025 (Open Source Summit, 100+
entreprises) ; version courante **1.0.0** (première stable, tag du 12 mars 2026 ;
patch v1.0.1 repo le 28 mai 2026) [A]. Au premier anniversaire (9 avril 2026) :
**150+ organisations**, déploiements production revendiqués, support shippé chez les
trois hyperscalers — Azure AI Foundry (endpoint A2A **en public preview**, A2A v0.3),
Copilot Studio, Amazon Bedrock AgentCore Runtime (nov. 2025), Google Cloud [A ; ne
pas survendre : « supported by » ≠ production généralisée, et Azure est en preview].

**Ce que la v1.0.0 normalise** [A] :

- **Modèle canonique protobuf** (`a2a.proto` = source de vérité normative) avec
  **trois bindings équivalents** : JSON-RPC 2.0 (§9), gRPC (§10), HTTP+JSON/REST
  (§11) ; un agent multi-transport MUST offrir le même jeu d'opérations partout
  (§5.1). Conséquence bridge : cibler la couche d'opérations abstraite, pas un fil.
- **Agent Card** : manifeste JSON auto-descriptif (well-known
  `/.well-known/agent-card.json` en v1.0 ; anciennement `agent.json`), déclarant
  identité, skills, transports et **securitySchemes** (§4.5) : OAuth2 (authorization
  code §4.5.8, client credentials §4.5.9, device code §4.5.10 — device/PKCE ajoutés
  en 1.0, patterns legacy retirés), OpenIdConnect, APIKey, HTTPAuth, **MutualTLS**
  (seul schéma non-bearer).
- **Acquisition des credentials hors-bande** (MUST §7.6.1) sauf mécanisme in-band
  négocié **via extension** (§7.6.4) ; état de tâche `TASK_STATE_AUTH_REQUIRED` pour
  déléguer une autorisation au client en cours de tâche (§7.6).
- **Signature des Agent Cards** (§8.4, optionnelle mais normative quand utilisée) :
  canonicalisation **JCS RFC 8785** (§8.4.1) puis **JWS RFC 7515** (§8.4.2), champ
  `signatures[]` (pluriel — **plusieurs autorités signataires possibles**, §4.4.7).
  JWS supporte EdDSA/Ed25519 (RFC 8037) → une identité Aithos peut signer ou
  co-certifier une carte telle quelle.
- **Mécanisme d'extensions formel** (§4.6) : déclaration dans
  `capabilities.extensions[]` (objets `AgentExtension`, champ `required`) ; si
  `required:true` non supportée par le client, l'agent MUST échouer
  (`ExtensionSupportRequiredError`, JSON-RPC -32008 / gRPC FAILED_PRECONDITION /
  HTTP 400). Présenté par la spec comme LE moyen d'innover « without modifying the
  core protocol », avec chemin vers l'intégration au cœur.

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| **Exposer un agent Aithos en A2A** | endpoint A2A gateway (un binding suffit — JSON-RPC, cohérent avec `proxy_mcp`) ; chaque task → `Op` → `verify_op` → gamma, exactement le pipeline `proxy_mcp` | [C] |
| **Agent Card générée depuis le mandat** | la carte (skills = connecteurs/périmètre résumé) est un *export dérivé* du mandat, régénérée à l'émission/révocation ; jamais une vérité propre | [C] |
| **Co-signature Ed25519 des cartes** | `signatures[]` accepte plusieurs signataires : signature owner/gateway Aithos à côté d'une signature d'hébergeur ; JCS déjà dans notre stack | [A] pour le mécanisme, [C] pour l'usage |
| **Extension `aithos.mandate.v1`** | déclarée `required:true` quand l'interlocuteur doit présenter/vérifier une chaîne de mandats in-band (la §7.6.4 autorise l'échange de credentials in-band via extension) ; dégradé : sans extension, OAuth2 scheme vers notre AS | [C] |
| **Appels A2A sortants = connecteur** | chaque agent distant déclaré devient un connecteur (`act.x.<agent-id>.<skill>`, ex. `act.x.a2a-gmailbot.draft` — id sans point, règle du découpage au dernier point, GATEWAY-HANDOFF §3) : périmètre, contraintes, obligations et gamma s'appliquent tels quels | [C], zéro nouveauté protocole |
| **TASK_STATE_AUTH_REQUIRED** | mappe naturellement nos obligations (§04.12) : la tâche s'interrompt, le reçu (co-sign humain, guardrail) se collecte, l'action consomme | [C] |

**Écarts** : sécurité déclarative bearer-orientée, credentials hors-bande (grep
DPoP/holder-of-key : zéro occurrence dans la spec [A]) ; pas de délégation atténuée ;
audit hors périmètre. Mêmes réponses qu'en §4.1 : AS frontière + extension.

### 4.3 Identité d'entreprise — XAA / ID-JAG (Okta), IPSIE (OpenID)

**ID-JAG / Cross App Access — statut.** Draft IETF **adopté par le WG OAuth**,
standards-track : `draft-ietf-oauth-identity-assertion-authz-grant`, version
courante **-04 (21 mai 2026)** [A — une claim citant -03 comme courante a été
réfutée 0-3]. Auteurs : Parecki (Okta), McGuinness, Campbell (Ping). « XAA » est le
nom produit ; techniquement un profil enterprise du draft `oauth-identity-chaining`
[B]. Écosystème (oauth.net/cross-app-access, juillet 2026) [B] : IdP Okta (early
access), Athenz (beta), Keycloak (en cours) ; **clients : Claude (beta), Claude Code
(beta), VS Code, WorkOS, Archestra** ; AS : Stytch, Auth0 (beta), Scalekit… ;
resource apps : Asana, Atlassian, Canva, Figma, Granola, Linear, Supabase. Intégré à
MCP comme extension **SEP-990** (« MCP Enterprise Managed Authorization ») [A].

**Mécanique (objets techniques)** [A] : empilement de **RFC 8693 Token Exchange** et
**RFC 7523 JWT Bearer**. (1) Le client (l'app agentique) échange l'assertion
d'identité de l'utilisateur à l'IdP : `requested_token_type =
urn:ietf:params:oauth:token-type:id-jag`, `audience` = l'AS de la ressource d'un
autre domaine de confiance. (2) L'IdP évalue sa politique d'entreprise et émet
l'**ID-JAG** : JWT signé, header `typ: oauth-id-jag+jwt`, claims REQUIRED
`iss/sub/aud/client_id/jti/exp/iat`, OPTIONAL `scope`, `authorization_details`
(RFC 9396), `act` (RFC 8693), `resource` (RFC 8707), `tenant`. (3) Le client
présente l'ID-JAG au token endpoint du Resource AS (`grant_type = jwt-bearer`) qui
émet SON access token. L'IdP devient le point de politique ; l'utilisateur ne
re-consent pas app par app. L'**Appendix A.4 « AI Agent using External Tools »**
vise explicitement les agents IA (« the agent often operates on behalf of the end
user, and its actions are constrained by the user's identity, role, and
permissions ») [A, citation corrigée par les vérificateurs]. Modèle bearer-JWT,
IdP online ; **-04 ajoute un key-binding DPoP/`cnf`(jkt) OPTIONNEL (§9.8.1)** [A] —
à surveiller : c'est la première brèche holder-of-key dans ce circuit.

**IPSIE.** Working group **OpenID Foundation** (chairs : Parecki/Okta, Hardt),
charté pour **profiler l'existant** (OIDC pour le SSO, Shared Signals Framework,
SCIM ; domaines : SSO, lifecycle, entitlements, risk signals, logout, token
revocation) — « developing new general-purpose specifications… is out of scope » ;
la charte publiée ne mentionne ni agents IA ni ID-JAG [B]. Conclusion : IPSIE n'est
pas une cible d'implémentation pour Aithos ; c'est un *label de conformité
entreprise* à suivre — le jour où IPSIE profile l'accès agentique, il profilera
très vraisemblablement XAA/ID-JAG, déjà couvert ci-dessous.

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| **Consommer un ID-JAG** (sens entreprise → Aithos) | le token endpoint de notre AS gateway accepte `grant_type=jwt-bearer` avec un ID-JAG (validation JWKS de l'IdP, `aud`, `exp`, `typ`) et le traduit en session bornée par un mandat provisionné — le pont « l'entreprise autorise, Aithos encadre » | [C] |
| **Émettre des ID-JAG depuis des mandats** (sens Aithos → SaaS) | adaptateur côté IdP : périmètre/contraintes → `scope` + `authorization_details` (RFC 9396), chaîne de mandats → claim `act` (RFC 8693) | [C] |
| **SEP-990 sur notre gateway MCP** | supporter le profil « Enterprise-Managed Authorization » sur nos endpoints MCP protégés — même flux que ci-dessus, emballage MCP | [A] mécanisme, [C] usage |
| **DPoP §9.8.1** | si Okta/Ping l'implémentent, lier nos tokens projetés à la clé du grantee (`cnf`) — réduit l'écart bearer/holder-of-key au lieu de le contourner | [B], question ouverte |

### 4.4 ANS — Agent Name Service (le DNS des agents, proposé)

> C'est le standard visé par « NAS (Linux Foundation) ». Précision de gouvernance :
> ANS est **OWASP GenAI Security Project** (Agentic Security Initiative), pas Linux
> Foundation — la confusion vient probablement de l'AAIF (MCP, déc. 2025) ou d'A2A
> (juin 2025), qui sont, elles, à la LF.

**v1 (historique).** Whitepaper OWASP publié le **14 mai 2025** [B] + draft IETF
`draft-narajala-ans-00` (soumission individuelle, intended status *Experimental*,
auteurs DistributedApps.ai/AWS/Intuit/Cisco) — **expiré en novembre 2025, inactif**
[B, vérifié sur datatracker]. Contenu v1 : cadre de découverte inspiré DNS ;
identité **PKI classique X.509** (CA + Registration Authority, révocation
**CRL/OCSP** — RFC 6960) ; nom structuré `ANSName = Protocol "://" AgentID "."
agentCapability "." Provider ".v" Version` ; **Protocol Adapter Layer** traduisant
les entrées de registre vers A2A (Agent Cards), MCP (tool descriptions), ACP ;
réponses de résolution signées par la clé du registre (MUST) ; le registre stocke
aussi des informations **DID** ; threat model MAESTRO (impersonation, registry
poisoning, DoS) [B].

**v2 (l'actif).** `draft-narajala-courtney-ansv2-01` — « **Agent Name Service v2 :
A Domain-Anchored Trust Layer for Autonomous AI Agent Identity** », dernière
révision **13 avril 2026**, expire le 15 octobre 2026, draft individuel (GoDaddy,
OWASP, DistributedApps.ai, Cisco), toujours hors WG [B, vérifié sur datatracker].
Changements structurants vs v1 [B] : ANSName simplifié en **`ans://v{version}.
{agentHost}`** (ancrage domaine — la confiance s'adosse au DNS existant) ;
**architecture à double certificat** ; l'intégrité du registre passe de
« conceptuelle » à des **logs de transparence cryptographiques alignés IETF
SCITT** ; les protocol adapters migrent vers des **SDK côté client** (plus dans la
Registration Authority).

**Traction honnête.** Le survey de référence des registres d'agents (arXiv
2508.03095 v3, oct. 2025) analyse cinq approches « prominentes » — MCP Registry,
A2A Agent Cards, AGNTCY ADS, Microsoft Entra Agent ID, NANDA — et **ANS n'y figure
pas** [B]. Statut réel : proposition sérieuse, itérée, non déployée.

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| **Registration adapter (publish-only)** | publier l'identité publique d'un agent (DID doc export, carte A2A, endpoints) vers un registre ANS s'il émerge ; jamais une dépendance de vérification (I4 : le registre *demande*, ne décide pas) | [C] |
| **X.509 v1 → non-cible** | la PKI CA/CRL/OCSP de v1 est l'anti-modèle d'Aithos (autorité centrale, révocation online) ; v2 ancre au domaine — compatible avec un export **did:web** (§6-C4), qui porte déjà la confiance DNS/TLS | [C] |
| **SCITT / transparence (v2)** | même famille d'idées que nos racines gamma committées (§07.10 : inclusion + complétude prouvables). Opportunité de *papier/PoC* « le gamma comme transparency log d'agent » plutôt que d'implémentation immédiate | [C] |
| **ANSName** | si besoin un jour : mapper `urn:aithos:agent:…` (grantee.id) vers `ans://v1.<host>` est un renommage, pas un changement de modèle | [C] |

**Verdict.** Veille active (v2 bouge), zéro implémentation avant qu'un WG l'adopte
ou qu'un registre réel le déploie. Le concept qu'ANS valide pour nous : la
découverte d'agents se standardise *au-dessus* des protocoles (adapters A2A/MCP) —
exactement notre thèse d'architecture.

### 4.5 AP2 — Agent Payments Protocol (le mandat de paiement)

**Statut & gouvernance.** Annoncé par Google le **16 septembre 2025** avec 60+
organisations (Adyen, Amex, Coinbase, Mastercard, PayPal, Revolut, Salesforce,
ServiceNow…) [B]. Spec **v0.2**, et **donation à la FIDO Alliance** : la
standardisation continue dans les WG FIDO « Agentic Authentication Technical » et
« Payments Technical » [B, vérifié sur ap2-protocol.org le 2026-07-12]. Extension
des protocoles existants : « available as an extension for the open-source
Agent2Agent (A2A) protocol » et de l'Universal Commerce Protocol (UCP) ; à
l'annonce, positionné aussi comme extension MCP [B]. Extension x402 (crypto,
Coinbase/Ethereum Foundation/MetaMask) [B].

**Le cœur du modèle — et pourquoi il nous parle.** AP2 appelle « **Mandates** » des
**Verifiable Digital Credentials signés par l'utilisateur** (clé device,
typiquement hardware-backed, avec authentification en session) [B]. En v0.2 : le
**Checkout Mandate** (open : contraintes/objectifs avant panier ; closed :
autorisation d'un checkout finalisé) et le **Payment Mandate** (open : contraintes
de paiement — budget, instruments ; closed : autorisation d'un montant précis lié
au checkout) [B]. La chaîne intent→cart→payment forme une piste d'audit
non-répudiable [B]. Autrement dit : **AP2 est la validation industrielle (Google +
réseaux de paiement + FIDO) du modèle Aithos** — l'autorisation d'agent comme
*credential signé holder-of-key, contraint, auditable*, pas comme bearer token.

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| **Mandat Aithos ↔ AP2 Mandate** | mapping sémantique direct : périmètre+contraintes (`spend_cap`, fenêtres, `max_actions`) ↔ open mandate ; action `binding` + co-signature owner (§04.6/§04.12) ↔ closed mandate signé en session ; le reçu d'obligation EST le « user present » d'AP2 | [C] |
| **Transport** | AP2 étant une extension A2A, il arrive « gratuitement » derrière le chantier A2A (§6-C3) le jour où un cas d'usage paiement existe | [C] |
| **VDC** | même chantier de représentation que §4.6 (mandat → VC) | [C] |

**Verdict.** Pas de chantier avant un cas d'usage paiement ; mais à citer
systématiquement : c'est l'argument d'autorité que « mandat cryptographique
contraint » est la direction du marché, jusque chez les réseaux de paiement.

### 4.6 W3C — DID 1.1, Verifiable Credentials 2.0, SD-JWT VC

**Statuts.** **VC Data Model 2.0 : W3C Recommendation depuis le 15 mai 2025**
(standard finalisé) [B]. **DID 1.1 : Candidate Recommendation Snapshot du 5 mars
2026** (phase d'appel à implémentations : ≥ 2 implémentations conformes par
feature pour passer REC ; pas avant le 5 avril 2026) [B, vérifié sur w3.org le
2026-07-12]. **SD-JWT VC : draft IETF OAuth -17 (6 juillet 2026), soumis à l'IESG**
pour publication Proposed Standard ; s'appuie sur SD-JWT publié **RFC 9901** [B].

**Ce qui compte pour Aithos** [B] :

- **DID 1.1** définit exactement notre couche : identifiant global, persistant,
  cryptographique, sans autorité d'enregistrement centrale ; une **DID method**
  définit create/resolve/update/deactivate. Le DID document 1.1 (base : Controlled
  Identifiers v1.0) expose les *verification relationships* — dont
  **`capabilityDelegation`** et **`capabilityInvocation`**, les slots standards
  pour « la clé qui délègue » et « la clé qui exerce » — et encode les clés en
  **`publicKeyMultibase`** (notre format !) ou `publicKeyJwk`.
- **VC 2.0** : credential = claims signées d'un issuer sur un subject ;
  **sécurisation obligatoire** par Data Integrity proofs (dont la cryptosuite
  `eddsa-jcs-2022` — Ed25519 + JCS, littéralement notre pile), JOSE/COSE ou
  SD-JWT ; les DID sont supportés mais **optionnels** ; rôles issuer/holder/subject
  avec holder ≠ subject possible ; **selective disclosure** native ; la révocation
  est un **status consulté à la vérification** (online) ; VC 2.0 standardise
  l'attestation/présentation, **pas la re-délégation atténuée** — c'est le gap que
  notre chaîne comble en interne.
- **SD-JWT VC** : **holder binding cryptographique** via claim `cnf` + **KB-JWT**
  signé par la clé du holder (MUST quand key binding actif) — du proof-of-possession
  standardisé, pas du bearer ; résolution d'issuer **web/PKI** (`iss` HTTPS +
  `/.well-known/jwt-vc-issuer`, ou `x5c`) — **pas de résolution DID définie** ;
  révocation optionnelle par Token Status List (online, SHOULD) ; média type
  `application/dc+sd-jwt`.

**Points d'ancrage Aithos** :

| Ancrage | Mécanisme | Niveau |
|---|---|---|
| **Représentation DID 1.1 du DID doc** | export conforme : `did:aithos` documenté comme method (ou pont did:web/did:key), clés en `publicKeyMultibase`, `#root` sous `capabilityDelegation`, clés de grantee référencées à l'invocation | [C], quasi gratuit |
| **Mandat → VC 2.0** | profil d'export : issuer = subject DID, credentialSubject = grantee (id URN + pubkey), claims = périmètre/contraintes ; preuve Data Integrity `eddsa-jcs-2022` (Ed25519+JCS : mêmes octets de canonicalisation que notre wire) | [C] |
| **Mandat → SD-JWT VC** | `cnf` = la clé du grantee (kex/Ed25519) ; la présentation KB-JWT rejoue notre étape 8 du verifier (§04.5, proof of possession) ; selective disclosure ≈ notre disclosure sélective (§02.11) | [C] |
| **Gap DID chez SD-JWT VC** | servir `/.well-known/jwt-vc-issuer` sur le domaine du bundle (pont web) — cohérent avec did:web et CIMD (§4.1) | [C] |

**Verdict.** La famille W3C/IETF-VC est le **langage de représentation** naturel
des mandats pour l'extérieur (audit tiers, AP2, wallets) — jamais leur forme
interne : VC 2.0 ne porte ni notre atténuation ni notre révocation offline. Export
one-way, régénérable, TTL court quand porteur de droits.

### 4.7 SPIFFE / SPIRE — identité de workload (CNCF)

**Ce que c'est.** Le standard CNCF d'identité d'infrastructure : SPIFFE ID
(`spiffe://trust-domain/…`), SVID X.509 ou JWT émis et **rotés automatiquement par
un serveur SPIRE central**, fédération entre trust domains. Signal d'adoption côté
agents : HashiCorp Vault Enterprise 1.21 authentifie en SPIFFE et émet des
X509-SVID pour les workloads non-humains « like AI agents » ; architecture de
référence HashiCorp : chaque agent reçoit SPIFFE ID + SVID d'un SPIRE central [B].

**Rapport à Aithos.** **Complémentaire, pas concurrent** : SPIFFE répond « quel
processus/pod est-ce ? » (identité d'infrastructure, courte, centralisée), Aithos
répond « au nom de qui, pour quoi, dans quelles limites ? » (autorité déléguée,
holder-of-key, offline). Le modèle SPIRE (émission centrale online, rotation
continue) est exactement ce que notre §00.5 refuse pour l'*autorité* — mais il est
sain pour l'*infrastructure*. Ancrage [C] : dans le pod (DEPLOYMENT-CONTAINMENT),
le **gateway peut porter un SVID** pour le mTLS d'infra (attester « ce runner est
bien le nôtre » auprès du réseau d'entreprise) pendant que les mandats Aithos
restent l'unique autorité d'action. Aucune dépendance inverse. Chantier : néant ;
une page de doc d'intégration quand un client entreprise le demandera.

### 4.8 Capabilities à délégation atténuée — GNAP, Biscuit, UCAN, ZCAP, Macaroons

La parenté conceptuelle d'Aithos (DESIGN §8 les cite déjà). État 2026 et ce qu'on
en retient :

| Standard | Statut | Modèle | À retenir pour Aithos |
|---|---|---|---|
| **GNAP** (RFC 9635, oct. 2024) [B] | Proposed Standard IETF | tokens **key-bound par défaut** (bearer = opt-in explicite), 4 méthodes de proofing (httpsig, mtls, jwsd, jws) + **registre IANA extensible** ; « NOT an extension of OAuth 2.0… not intended to be directly compatible » | le seul standards-track dont le défaut est holder-of-key — mais hors écosystème MCP/entreprise ; son registre de proofing est le point d'entrée si un jour on veut y inscrire un profil Ed25519/DID [C] |
| **Biscuit** (eclipse-biscuit, spec v2, blocs v3–v5) [B] | spec open source active (Eclipse) | **bearer** à atténuation offline, chaîne de signatures **Ed25519**, `revocation_id` par bloc (listes de révocation externes), **third-party blocks** signés | la preuve qu'atténuation offline se fait aussi en bearer — notre choix holder-of-key reste plus fort ; leurs third-party blocks ≈ nos obligations (§04.12), validation croisée du design |
| **UCAN** (v1.0.0, ucan-wg) [B] | spec communautaire (pas IETF/W3C), adoption niche (web décentralisé, Bluesky-adjacent) | certificats de capabilities **DID (did:key) + Ed25519 MUST**, atténuation obligatoire par lien, encodage DAG-CBOR/CID (pas JWT), révocation seulement RECOMMENDED | le plus proche sémantiquement de nos chaînes (§05) ; incompatibilité d'enveloppe (IPLD vs JCS/JSON) ; interop opportuniste seulement si l'écosystème décentralisé devient un marché |
| **ZCAP-LD** (v0.3, W3C CCG) [B] | draft communautaire, TODO ouverts (révocation), traction faible (~37 étoiles) | capabilities JSON-LD signées Data Integrity, `capabilityChain`, caveats | à lire comme prior art ; pas une cible |
| **Macaroons** (Google, 2014) | papier + libs, usage interne datacenter | bearer HMAC à caveats, vérification par le seul émetteur | prior art historique de l'atténuation ; aucune interop à viser |

**Verdict.** Aucun de ces modèles n'est sur le chemin critique du marché 2026
(MCP/A2A/OAuth ont gagné la couche d'interop). Leur valeur ici : ils **valident**
les choix Aithos (atténuation offline, Ed25519, obligations tierces) et
fournissent le vocabulaire pour documenter nos différences. Zéro chantier.

### 4.9 Découverte & registres — AGNTCY, NANDA, MCP Registry, Entra Agent ID

État des lieux (survey arXiv 2508.03095 v3 + sources primaires) [B] :

- **AGNTCY** (Linux Foundation depuis le 29 juillet 2025 ; initié par Cisco en mars
  2025 avec LangChain/Galileo ; 65+ sociétés ; membres formateurs : Cisco, Dell,
  Google Cloud, Oracle, Red Hat). Quatre composants : découverte **OASF** (schémas
  d'agents), **identité vérifiable** avec contrôle d'accès, messagerie **SLIM**,
  observabilité. Se positionne interopérable A2A/MCP : « makes A2A agents and MCP
  servers discoverable through AGNTCY directories ». Son Agent Directory Service :
  DHT Kademlia/IPFS + artefacts OCI + intégrité **Sigstore**.
- **NANDA** (MIT, « Internet of AI Agents ») : index + **AgentFacts** vérifiables —
  W3C VC et objets **signés Ed25519**, chemin d'attestation blockchain optionnel
  (ERC-8004), gouvernance proposée : hébergement du registre par 15 universités.
  Stade recherche (papiers arXiv juil.–août 2025), traction académique.
- **MCP Registry** (officiel) : publication centralisée de descripteurs
  `mcp.json` versionnés ; vérification d'identité par **GitHub OAuth + DNS TXT**
  (confiance web/DNS, ni PKI ni DID).
- **Microsoft Entra Agent ID** : registre d'agents d'entreprise dans l'IdP
  Microsoft — la déclinaison « annuaire = tenant » du problème.

**Points d'ancrage Aithos** [C] : publier, ne jamais dépendre. Un
`registry_adapter` optionnel de la gateway peut pousser la carte A2A signée et le
DID doc exporté vers AGNTCY/MCP Registry (et ANS si ça se déploie) ; la
*vérification* reste chaîne de mandats + bundle. Le couple AGNTCY-Sigstore et le
NANDA-VC confirment que le marché ancre la découverte dans des **artefacts
signés** — notre matière première.

## 5. Les écarts de modèle structurels (et leur traitement)

Cinq écarts reviennent dans tout le paysage. Aucun ne se résout en changeant le
core ; chacun a un traitement de frontière nommé, réutilisé par les chantiers §6.

### 5.1 Bearer online vs holder-of-key offline

**L'écart.** MCP : bearer pur audience-borné [A]. A2A : schémas déclaratifs
bearer-orientés, credentials hors-bande, zéro proof-of-possession dans la spec [A].
ID-JAG : JWT bearer minté online par l'IdP (DPoP optionnel depuis -04) [A]. Aithos :
le mandat ne vaut que signé par la clé du grantee à chaque acte (§04.5 étape 8).

**Le traitement : la projection périssable.** À chaque frontière sortante, la
gateway échange la chaîne de mandats contre un **access token court, audience-borné
(RFC 8707), scopes dérivés du périmètre** ; le token n'accorde que l'*entrée* — le
`verify_op` Aithos reste évalué à chaque acte derrière la frontière, et l'entrée
gamma reste obligatoire (I5). Deux propriétés font tenir le pont : (a) MCP interdit
le passthrough [A], donc *tout le monde* ré-émet à chaque saut — notre atténuation
par lien épouse ce grain ; (b) le TTL du token se borne par la **fenêtre de
fraîcheur** (§04.4 `freshness`, §07.7) : la doctrine « certificat vérifié contre
révocation fraîche » devient « token qui expire avant que la fraîcheur ne périme ».
Un token perdu expose au pire une fenêtre — jamais un droit durable, jamais une clé.

### 5.2 Révocation online vs l'échelle offline

**L'écart.** CRL/OCSP (ANS v1), Token Status List (SD-JWT VC, SHOULD online),
status VC 2.0 consulté à la vérification, politique IdP (XAA), expiration de token
(OAuth) — tout le paysage révoque *en consultant un service*. Aithos révoque en
publiant une édition (échelle §06), vérifiable de fichiers seuls.

**Le traitement.** Les deux modèles se composent sans se toucher : côté externe,
révoquer = **cesser d'émettre** les projections (l'AS refuse au prochain échange —
délai ≤ TTL) ; côté interne, l'échelle reste l'acte de vérité (rung 1 suffit à
faire refuser l'AS : c'est un verifier §04.5). Correspondance : rung 0/expiry ↔
`exp` du token ; rung 1/cert ↔ refus d'émission + status list si on en publie une ;
rungs 2–4 ↔ sans équivalent externe (c'est notre plus-value). Ne JAMAIS publier de
CRL « autoritaire » : un index de révocation exporté est dérivé du gamma,
best-effort, jamais une source de vérité (cohérent §06.5).

### 5.3 Identité : domaine/URL/X.509 vs DID

**L'écart.** Le circuit entreprise ancre l'identité dans le web : JWKS d'IdP
(XAA), `client_id` URL (CIMD), `iss` HTTPS + well-known (SD-JWT VC), DNS TXT (MCP
Registry), domaine (ANS v2), X.509 (SPIFFE, ANS v1). Aithos ancre dans la clé
(`did:aithos:multibase(root)`).

**Le traitement : trois exports, zéro octet de wire changé.**
(1) **did:key** — notre encodage multibase/multicodec (`0xed01`) est déjà celui de
did:key : la clé de n'importe quel grantee EST un did:key à préfixe près.
(2) **did:web** — publier le DID doc exporté sous
`https://<domaine>/.well-known/did.json` : la confiance DNS/TLS que demandent ANS
v2/CIMD/SD-JWT VC, sans toucher au document signé root.
(3) **JWKS** — les mêmes clés en JWK (`OKP/Ed25519`), pour tout ce qui parle OAuth.
Le DID doc du bundle reste l'autoritaire ; les trois exports sont régénérables et
datés. DID 1.1 fournit les slots (`capabilityDelegation`/`capabilityInvocation`,
`publicKeyMultibase`) pour une représentation fidèle [B].

### 5.4 Délégation : inexistante chez eux, centrale chez nous

**L'écart.** Ni MCP, ni A2A, ni XAA ne définissent de re-délégation atténuée ;
l'IETF n'a que le claim `act` (RFC 8693) pour *décrire* une chaîne d'acteurs, et
`authorization_details` (RFC 9396) pour des droits riches. UCAN/Biscuit l'ont —
hors du circuit entreprise (§4.8).

**Le traitement.** La chaîne reste interne. Vers l'extérieur, elle se *décrit* :
`act` imbriqués (un maillon par mandat de la chaîne), `authorization_details` de
type `aithos-perimeter` portant entrées de périmètre et contraintes lisibles. La
perte est assumée et documentée : un vérificateur externe lit la chaîne, il ne peut
pas la *vérifier* sans le bundle — c'est le dégradé de frontière, pas un trou : le
seul octroi effectif reste le token court émis après vérification complète.

### 5.5 Audit : hors périmètre chez eux, invariant chez nous

**L'écart.** Aucun des standards examinés ne normalise le log d'audit (constat
d'absence [A/B]) ; l'observabilité est laissée aux produits. Aithos en fait un
invariant (I5) prouvable (§07.10).

**Le traitement.** Rien à adapter — à *défendre* : chaque adapter écrit ses entrées
gamma comme `proxy_mcp` aujourd'hui (refus compris, §3bis.8). La seule convergence
externe à suivre est SCITT (via ANS v2) : le jour où « transparency log d'agent »
devient un genre, nos racines committées sont déjà le mécanisme — opportunité de
positionnement, pas de refonte.

### 5.6 L'humain dans la boucle : obligations vs mécanismes épars

**L'écart-mirroir (en notre faveur).** Le paysage éparpille l'approbation humaine :
politique IdP (XAA), `TASK_STATE_AUTH_REQUIRED` (A2A), elicitation (MCP), user
presence FIDO (AP2 closed mandates), CIBA côté OpenID. Aithos l'unifie : obligations
§04.12 (reçu signé lié par `args_hash`, co_sign owner, guardrail, double contrôle).

**Le traitement.** Mapper, pas importer : l'adapter A2A traduit « obligation à
décharger » en `AUTH_REQUIRED` ; l'adapter MCP en elicitation/erreur explicite ;
l'app d'approbation (CLI `approve` de G+) reste l'attestor. AP2 valide ce design
jusque dans le paiement.

## 6. Les chantiers (spécification, zéro changement core)

Chaque chantier est un module de la gateway (`rust/crates/aithos-gateway/`) ou un
outil d'export, avec le rituel maison : feature Gherkin d'abord, fail-closed
partout, gamma sur chaque acte, tests de surface. Aucun ne modifie
`aithos-core`/`aithos-bundle` (le C4 ajoute un *outil* de plus qui les consomme).

### C1 — `gateway_as` : l'authorization server adossé aux mandats (S3, S5)

**Quoi.** Un module AS OAuth 2.1 minimal dans la gateway : métadonnées RFC 8414,
token endpoint, PKCE, RFC 8707 (`resource` exigé, `aud` dans le token émis),
tokens = JWT courts signés par une clé de service AS (clé d'adapter, gérée comme
un secret gateway — PAS un objet protocole), claims : `sub` = subject DID, `act` =
chaîne (§5.4), `scope`/`authorization_details` = projection du périmètre, `exp` ≤
min(`not_after`, fenêtre `active_windows` courante, fraîcheur). Émission = (1)
`verify_chain` complet à T, (2) mint, (3) entrée gamma `act` (`x.oauth.issue`,
target = audience) — une émission est un acte, jamais silencieuse.
**Ce qui ne change pas.** `verify_op` à chaque usage derrière l'AS ; les contraintes
non-projetables (`max_actions`, budgets) restent enforced par la gateway au fil des
actes — le token n'est qu'un laissez-passer.
**Done quand.** Un client MCP tiers (Claude, VS Code, Inspector) s'authentifie
contre notre serveur MCP protégé via RFC 9728→8414→PKCE→token, fail-closed testé
(mauvaise audience, token expiré, mandat révoqué → 401 + refus gamma). Taille : M.

### C2 — `xaa_bridge` : consommer (et plus tard émettre) des ID-JAG (S5)

**Quoi.** (a) *Consommer* : le token endpoint de C1 accepte
`grant_type=jwt-bearer` avec un ID-JAG (`typ: oauth-id-jag+jwt` ; validation JWKS
IdP, `aud`, `exp`, `jti` anti-rejeu) et le mappe vers un mandat *pré-provisionné*
par l'owner pour ce couple (IdP, sujet) — l'IdP n'obtient jamais plus que ce qu'un
mandat Aithos accorde (I4 : l'entreprise demande, le mandat décide). (b) *Émettre*
(exploratoire, après a) : module IdP-side qui mint des ID-JAG depuis des mandats
pour les SaaS externes. (c) Profil SEP-990 sur nos endpoints MCP.
**Done quand.** Un client XAA beta (Claude/VS Code) traverse IdP→ID-JAG→notre AS→
outil MCP, chaque saut loggé ; ID-JAG rejoué/mal-audiencé → refus. Taille : M
(a+c) ; L avec (b).

### C3 — `proxy_a2a` : l'endpoint agent-à-agent (S4, S7)

**Quoi.** (a) *Servir* : binding JSON-RPC A2A v1.0 minimal (message/send, tasks,
`AUTH_REQUIRED` pour obligations) ; Agent Card générée depuis le mandat (skills =
actions/périmètre résumé), signée §8.4 (JCS→JWS EdDSA — notre JCS existant),
`securitySchemes` pointant l'AS de C1 ; extension déclarée `aithos.mandate.v1`
(métadonnées : DID doc export, politique de présentation de chaîne in-band §7.6.4)
— `required:false` par défaut, `true` sur les déploiements qui l'exigent (-32008
sinon, fail-closed standard). (b) *Appeler* : connecteurs A2A sortants
(`act.x.<a2a-agent-id>.<skill>`, ids sans point), manifeste par agent distant, vault pour ses
credentials — pipeline connecteur inchangé.
**Done quand.** Un client SDK A2A officiel consomme notre carte, exécute une task
sous mandat (gamma), et notre gateway appelle un agent A2A externe sous
`act.x.<a2a-agent-id>.*` avec budget/fenêtres/obligations actifs. Taille : L.

### C4 — `identity_export` : did:key / did:web / JWKS / DID 1.1 (S1)

**Quoi.** Un outil (CLI ou module gateway) qui dérive du bundle : (1) did:key par
clé publique (préfixe multicodec déjà bon) ; (2) document did:web
(`/.well-known/did.json`) conforme DID 1.1 (`publicKeyMultibase`,
`capabilityDelegation` = #root, `assertionMethod` = #content) ; (3) JWKS
(OKP/Ed25519) ; (4) document CIMD par agent (client_id URL, SEP-991). Tous dérivés,
datés, régénérables ; le DID doc signé du bundle reste l'autoritaire.
**Done quand.** Un résolveur DID tiers lit notre did:web ; un AS OAuth tiers lit
notre JWKS ; vecteurs d'export croisés (mêmes octets de clés que `vectors/a2-did`).
Taille : S. **À faire en premier** — débloque C1/C2/C3 et ne touche que la lecture.

### C5 — `vc_export` : mandat → VC 2.0 / SD-JWT VC (S2)

**Quoi.** Profil d'export : mandat → Verifiable Credential (issuer = subject DID,
credentialSubject = grantee, claims = périmètre/contraintes/validité, preuve Data
Integrity `eddsa-jcs-2022`) et/ou SD-JWT VC (`cnf` = clé grantee, KB-JWT à la
présentation = notre PoP §04.5-8, selective disclosure alignée §02.11). Usage :
audit tiers, wallets, AP2-readiness. Export one-way ; un VC exporté n'ouvre rien
chez nous sans la chaîne réelle.
**Done quand.** Un vérificateur VC standard (lib tierce) valide le credential et sa
présentation key-bound ; note publiée sur la perte sémantique (pas d'atténuation).
Taille : M. Optionnel tant qu'aucun consommateur VC n'est en face.

### C6 — AP2-readiness (S2, dormant)

Étude courte au moment voulu : mapping mandat-Aithos ↔ Checkout/Payment Mandates
(open/closed), obligations ↔ user-presence FIDO ; s'active derrière C3 (AP2 =
extension A2A) le jour d'un cas d'usage paiement. Taille : S (étude), implémentation
à chiffrer alors.

### C7 — `registry_adapter` + veille ANS/SCITT (S7, S6)

Publication opportuniste (AGNTCY, MCP Registry ; ANS si déployé) des artefacts déjà
produits par C3/C4 ; PoC/papier « gamma roots comme transparency log » aligné SCITT
quand ANS v2 bouge. Publish-only, jamais une dépendance de vérification. Taille : S.

### C8 — SPIFFE d'infrastructure (optionnel, S1-infra)

Documentation (pas de code core/gateway obligatoire) : le pod peut porter un SVID
pour le mTLS d'infra ; les mandats restent l'unique autorité d'action. Taille : XS.

## 7. Plan d'action

Phasé comme le reste du repo : une phase = un contrat BDD co-écrit d'abord, des
fail-closed testés, une validation manuelle, pas de retour en arrière. S'insère
après la Phase C gateway (proxy_llm ✅) — les chantiers standards SONT la matière
naturelle de la Phase D « industrialisation » côté interop. Tailles : S ≈ jours,
M ≈ 1–2 semaines, L ≈ 3–5 semaines (calibre gateway actuel).

### Phase 0 — Doctrine & veille (S, immédiat)

1. Graver la doctrine de cette doc (adapter/export/pont, anti-objectifs §1-2) —
   relecture Mathieu, statut de la doc DRAFT → décidé.
2. Poser le **tableau de veille versionnée** (§7.1 ci-dessous) et son rituel : à
   chaque session interop, vérifier les 6 lignes, dater.
3. Décider le nom du module (`aithos-gateway::interop` vs crate `aithos-interop`).
   Recommandation : module dans le crate gateway (même rituel, extraction plus
   tard, comme la décision GATEWAY-BOOTSTRAP §3).

**Critère de sortie.** Doc committée + décisions notées dedans (comme les
« décidé 2026-07-10 » de la spec).

### Phase 1 — Identité exportée (S, tout de suite après)

C4 en entier (did:key/did:web/JWKS/CIMD). Aucun risque, pure lecture du bundle,
débloque tout le reste. Livrable annexe : vecteurs d'export croisés Python (rituel
vectors-first).

**Critère de sortie.** Résolveur DID tiers + AS tiers lisent nos exports ;
`vectors/` étendus ; doc §01 inchangée (on n'y touche pas — l'export est outillage).

### Phase 2 — MCP conforme & SSO entreprise (M puis M, le cœur)

C1 (`gateway_as`) puis C2a+c (`xaa_bridge` consommation + SEP-990). Ordre imposé :
C2 s'appuie sur le token endpoint de C1. Jalons de démo vendables :
« un client MCP du commerce (Claude/VS Code) se connecte à un serveur MCP gouverné
par Aithos, SSO entreprise compris, chaque acte dans le gamma ».

**Critères de sortie.** Interop réelle avec ≥ 1 client tiers non modifié ;
fail-closed complets (audience, expiry, révocation rung 1 → refus à l'émission) ;
TTL ≤ fraîcheur testé ; zéro octet de token accepté en passthrough (aligné [A]).

**Risque suivi.** RC MCP 2026-07-28 (sort ~2 semaines après cette doc) : les MUST
cités sont vérifiés inchangés dans le RC [A], mais re-valider au release. ID-JAG
est un draft mouvant (-04 expire le 22 nov. 2026) : pinner la version implémentée,
adapter au -05.

### Phase 3 — A2A (L)

C3 (servir + appeler). Après la Phase 2 (réutilise l'AS pour `securitySchemes`).
Décisions à prendre en entrée de phase (mêmes gabarits que « Décisions à prendre »
de GATEWAY-BOOTSTRAP §9) : binding v1 (JSON-RPC recommandé), politique de cartes
(qui signe : owner content key vs clé de service certifiée par mandat —
recommandation : clé de service, certifiée, révocable rung 1), granularité du
connecteur sortant (un connecteur par agent distant `act.x.<a2a-agent-id>.<skill>`, recommandé, vs un connecteur unique `act.x.a2a.<skill>` avec l'agent en argument scellé).

**Critère de sortie.** Interop SDK A2A officiel dans les deux sens + extension
`aithos.mandate.v1` documentée/versionnée (repo `spec/` de l'extension à part,
comme `modelcontextprotocol/ext-auth` le fait pour SEP-990).

### Phase 4 — Représentations VC & paiement (M, optionnelle/pilotée par la demande)

C5 (VC/SD-JWT VC) quand un consommateur existe (auditeur externe outillé VC,
wallet, exigence AP2) ; C6 (étude AP2) si cas d'usage paiement. Ni l'une ni l'autre
ne bloque quoi que ce soit.

### Phase 5 — Découverte & positionnement (S, continu)

C7 (publication registres + veille ANS/SCITT + le papier « gamma comme
transparency log ») ; C8 (page SPIFFE). Opportuniste, jamais bloquant.

### 7.1 Tableau de veille (à vérifier à chaque session interop)

| Sujet | Version pinnée (2026-07-12) | Événement qui déclenche une action |
|---|---|---|
| Spec MCP | 2025-11-25 (+ RC 2026-07-28 annoncé) | release finale du RC → re-lire changelog authz |
| Extension SEP-990 / ext-auth | stable (repo ext-auth) | intégration au cœur MCP → prioriser C2c |
| A2A | 1.0.0 (v1.0.1 repo) | 1.1/2.0, évolution §4.6/§8.4 |
| ID-JAG | draft-ietf -04 (exp. 2026-11-22) | -05+, adoption du §9.8.1 DPoP par Okta/Ping → activer l'option DPoP de C1 |
| OAuth 2.1 | draft -15 | publication RFC → mettre à jour les références C1 |
| DID 1.1 / VC / SD-JWT VC | CR 2026-03-05 / REC 2025-05-15 / -17 IESG | DID 1.1 REC (≥ avril 2026 possible), SD-JWT VC RFC → figer C4/C5 |
| ANS | v2 draft-01 (exp. 2026-10-15) | adoption par un WG IETF ou déploiement d'un registre réel → chiffrer C7-ANS |
| AP2 / FIDO | v0.2 (WG FIDO) | sortie des WG FIDO, exigences VDC → activer C6 |

### 7.2 Risques transverses

| Risque | Traitement |
|---|---|
| Drafts mouvants (ID-JAG, OAuth 2.1, SD-JWT VC, ANS v2) | pinner les versions dans le code et cette doc ; la veille §7.1 est le rituel de rattrapage |
| Projection avec perte (périmètre → scopes) | perte documentée par chantier ; le droit effectif reste `verify_op` par acte ; ne jamais promettre à un tiers ce que le scope seul dit |
| Seconde source de vérité rampante (cartes, VC, tokens qui « vivent ») | tout export porte date + provenance et se régénère du bundle ; revue explicite à chaque chantier (checklist PR) |
| Fatigue de standards (implémenter ce qui mourra) | règle d'engagement : implémenter ce qui a ≥ 2 implémenteurs indépendants déployés (MCP, A2A, XAA passent ; ANS, UCAN, GNAP non) |
| Clé de service AS/cartes = nouveau secret | c'est un secret *gateway* (comme la clé API LLM), au vault/keyholder, jamais un objet protocole ; révocable par rotation + re-publication des exports |

## 8. Matrice récapitulative

| Standard | Statut 2026-07 | Gouvernance | Surface | Mécanisme Aithos | Chantier | Priorité |
|---|---|---|---|---|---|---|
| MCP | rév. 2025-11-25, RC 2026-07-28 ; de facto (10k+ serveurs) | AAIF / Linux Foundation | S3, S5 | adapter (fait) + AS frontière | C1 | **P1** |
| XAA / ID-JAG | draft IETF WG -04 ; EA/beta multi-vendeurs (clients : Claude, VS Code) | IETF OAuth WG (Okta moteur) | S5 | pont de crédentiels | C2 | **P1** |
| A2A | 1.0.0 stable ; 150+ orgs, 3 hyperscalers | Linux Foundation | S4, S7 | adapter + extension + co-signature cartes | C3 | **P2** |
| DID 1.1 / did:key / did:web | CR mars 2026 / stables | W3C | S1 | profil d'export | C4 | **P1 (gratuit)** |
| VC 2.0 / SD-JWT VC | REC mai 2025 / -17 IESG (RFC 9901 base) | W3C / IETF | S2 | profil d'export | C5 | P3 |
| AP2 | v0.2, donné à FIDO | FIDO Alliance (WGs) | S2 | mapping sémantique (via A2A ext.) | C6 | P3 (dormant) |
| ANS | v1 expiré ; v2 draft individuel actif | OWASP GenAI (drafts perso) | S7, S6 | veille + publish-only | C7 | P4 (veille) |
| AGNTCY / NANDA / MCP Registry | LF juil. 2025 / recherche MIT / officiel | LF / MIT / AAIF | S7 | publish-only | C7 | P4 |
| SPIFFE/SPIRE | standard CNCF établi | CNCF | S1-infra | doc d'intégration pod | C8 | P4 |
| IPSIE | WG actif (profils, pas d'agents au charter) | OpenID Foundation | S5 | rien à faire (suivi) | veille | P5 |
| UCAN / Biscuit / GNAP / ZCAP / Macaroons | niche / RFC 9635 hors circuit | divers | S2 (parenté) | prior art, zéro chantier | — | P5 |

## 9. Sources (primaires, datées, consultées le 2026-07-12)

**MCP** — spec authorization 2025-06-18 et 2025-11-25 :
`modelcontextprotocol.io/specification/2025-06-18/basic/authorization`,
`…/2025-11-25/basic/authorization`, `…/2025-11-25` (+ changelog) ; blog anniversaire
2025-11-25 et RC : `blog.modelcontextprotocol.io/posts/2025-11-25-first-mcp-anniversary/`,
`…/2026-07-28-release-candidate/` ; SEP-990/991 : issue #990 et repo
`github.com/modelcontextprotocol/ext-auth`. Gouvernance : communiqué Linux
Foundation « Formation of the Agentic AI Foundation », 2025-12-09.
**A2A** — spec : `a2a-protocol.org/latest/specification/` (+ URL épinglée v1.0.0,
+ `docs/specification.md` du repo `a2aproject/A2A`, releases GitHub) ; annonce 1.0 :
`a2a-protocol.org/latest/announcing-1.0/` ; communiqués LF 2025-06-23 (donation) et
2026-04-09 (150+ orgs) ; docs Microsoft Learn (Azure AI Foundry, Copilot Studio) et
AWS Bedrock AgentCore (devguide + blog ML, 2025-11-11).
**XAA / ID-JAG** — draft :
`datatracker.ietf.org/doc/draft-ietf-oauth-identity-assertion-authz-grant/` (-04,
2026-05-21) + texte `ietf.org/archive/id/…-04.txt` ; page implémentations :
`oauth.net/cross-app-access/` ; Okta dev blog 2025-06-23 (annonce) et 2026-02-17
(resource app) ; briques : RFC 8693, RFC 7523, RFC 9396, RFC 8707 ; OAuth 2.1 :
draft-ietf-oauth-v2-1 (-15) (`datatracker.ietf.org/doc/draft-ietf-oauth-v2-1/`).
**IPSIE** — `openid.net/wg/ipsie/` (+ charte), page modifiée 2026-02-03.
**ANS** — whitepaper OWASP GenAI v1.0 (2025-05-14) :
`genai.owasp.org/resource/agent-name-service-ans-for-secure-al-agent-discovery-v1-0/` ;
drafts : `datatracker.ietf.org/doc/draft-narajala-ans/` (expiré) et
`datatracker.ietf.org/doc/draft-narajala-courtney-ansv2/` (-01, 2026-04-13).
**AP2** — `ap2-protocol.org` (spec v0.2, donation FIDO) ; annonce Google Cloud
2025-09-16 (`cloud.google.com/blog/products/ai-machine-learning/announcing-agents-
to-payments-ap2-protocol`).
**W3C / IETF-VC** — `w3.org/TR/vc-data-model-2.0/` (REC 2025-05-15) + communiqué
W3C ; `w3.org/TR/did-1.1/` (CR Snapshot 2026-03-05) ;
`datatracker.ietf.org/doc/draft-ietf-oauth-sd-jwt-vc/` (-17, 2026-07-06 ; base
RFC 9901).
**Capabilities** — UCAN : `github.com/ucan-wg/spec` (v1.0.0) ; Biscuit :
`github.com/eclipse-biscuit/biscuit/blob/master/SPECIFICATIONS.md` ; GNAP :
RFC 9635 (`datatracker.ietf.org/doc/html/rfc9635`) ; ZCAP :
`w3c-ccg.github.io/zcap-spec/` (v0.3).
**SPIFFE** — blog HashiCorp « SPIFFE: securing the identity of agentic AI and
non-human actors » (Vault Enterprise 1.21/2.0) ; `spiffe.io`.
**Registres** — AGNTCY : communiqué LF 2025-07-29 ; NANDA : arXiv 2507.14263
(« Beyond DNS », MIT) + `github.com/projnanda` ; survey des registres :
arXiv 2508.03095 (v3, 2025-10-20).

**Rappel des niveaux.** [A] = vérifié 3-0 par vérification adversariale contre la
source primaire (harness du 2026-07-12, 110 agents, 24 claims confirmées / 1
réfutée) ; [B] = citation extraite de la source primaire (une passe) ; [C] =
inférence de conception à prototyper — aucun éditeur ne documente d'intégration
Aithos, leur faisabilité se prouve aux critères de sortie des phases §7.

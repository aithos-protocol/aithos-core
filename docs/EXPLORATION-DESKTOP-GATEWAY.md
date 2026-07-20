# Aithos — Desktop gateway : encapsuler une app agentique perso (piste d'exploration)

> **Statut : PISTE D'EXPLORATION (non tranchée).** Note de vision/produit (interne, FR).
> **Hors protocole** : décrit une forme de déploiement *desktop* de la couche au-dessus
> d'aithos-core (gateway + coffre chez l'utilisateur). Le core ne bouge pas — il fournit
> déjà les briques (`verify_chain`, `log_action`, `read.gamma`, connecteurs, `Vault`,
> `McpRouter`). Complète `DEPLOYMENT-CONTAINMENT.md` (topologie pod/serveur) et
> `GATEWAY-BOOTSTRAP.md` (§4bis `RemoteVault`). Initiée 2026-07-11.

## 1. L'idée en une phrase

Une **application desktop** qui laisse l'utilisateur employer n'importe quel agent
personnel (Claude via l'Agent SDK, un moteur OpenAI-compatible, un modèle local), dont
**tous les accès aux outils — MCP, web — passent automatiquement par notre gateway
locale**, avec le même périmètre gouverné et le même log inviolable (gamma), sans
reconfigurer chaque app. Le pendant desktop de l'audit externe : le coffre et
l'enforcement chez l'utilisateur, pas chez le LLM.

## 2. Deux formes, et une seule tient debout

L'intuition « se brancher derrière l'app perso » cache deux produits très différents.

| Forme | Principe | Surface gouvernée | Verdict |
|---|---|---|---|
| **1. Le shim** | L'utilisateur garde son app (Cowork perso, ChatGPT desktop, Cursor…) ; notre app se glisse entre elle et le monde | Seulement ce que l'app **route volontairement en local** (≈ le seam MCP) | Partiel — jamais du containment |
| **2. L'hôte** | Notre desktop app **embarque le moteur** (Agent SDK / boucle OpenAI-compat) et présente une UX façon Cowork | **Tout l'egress d'outils**, par construction | La version qui tient |

Le **produit Cowork grand public** (hébergé chez Anthropic) n'est encapsulable dans
*aucune* des deux : c'est un service cloud, pas un binaire. « Encapsuler Cowork » se
traduit légalement par « embarquer l'Agent SDK sur lequel Cowork est bâti » (Forme 2).

**Filiation repo** : la Forme 2 est déjà anticipée — `GATEWAY-BOOTSTRAP` §4bis décrit
`RemoteVault` (« le coffre est une API qu'on maîtrise : app desktop/mobile de
l'utilisateur ; sign/unseal délégués ; l'octet de clé ne quitte jamais le poste ;
zero-knowledge fort »), et la décision 3bis pose « container sur serveur, coffre chez
l'utilisateur ». Cette note relocalise aussi le *runtime* sur le poste.

## 3. La carte des coutures d'interception

Sur desktop il n'y a **pas** de network namespace (pas d'egress lockdown L3/L4 comme
dans le pod). Les seules coutures propres, sans MITM ni CA racine installé :

```
  App agentique                Notre desktop app                Monde
  ─────────────                ─────────────────                ─────
  moteur LLM  ───────── inférence directe (NON intercepté) ───► provider cloud
                                                                 (Anthropic / OpenAI)
  client MCP  ──► [ GATEWAY locale ] ──► verify_op + gamma ────► vrais MCP (localhost / net)
                        │  coffre /x/ (Vault), clés jamais chez le LLM
  outil web   ──► [ proxy_web (à construire) ] ── act.x.web.* ─► web autorisé
  outil exec  ──► (piège : réseau libre = tout rouvert) ──────► ⚠ à egress-locker
```

- **Seam MCP = fort.** Les apps déclarent leurs serveurs MCP *localement* (fichier de
  config). On pointe l'app vers la **gateway locale** au lieu des vrais MCP ; elle
  fan-out sous mandat. Zéro MITM : l'app route parce que l'utilisateur l'a configurée.
- **Seam inférence = exclu** (et c'est voulu). Anthropic en direct : non intercepté,
  non MITM-able. On audite des *actes*, pas des *pensées*.
- **Seam web = fragile en Forme 1, propre en Forme 2.** Forcer le HTTPS d'une app
  *étrangère* exige proxy système + CA racine (invasif, MITM du provider = no-go, et le
  cert-pinning le défait). En Forme 2, `proxy_web` capture l'egress de *nos* outils par
  construction — pas d'astuce OS.

## 4. Le hub MCP agrégé : une config → N apps

Le cœur vendable de « les mêmes accès automatiquement ». La gateway expose **un endpoint
MCP agrégé** (le `McpRouter` fait déjà l'agrégation `tools/list` multi-contexte). On
déclare connexions + mandats **une fois** ; toute app qui pointe son client MCP vers la
gateway hérite du **même périmètre tracé**. Une config, N apps, un seul gamma.

Ce n'est pas hypothétique : le pont Cowork→MCP local de la session courante
(`aithos` servi au poste, atteint par un agent cloud) est déjà ce mécanisme en action.

## 5. L'asymétrie qui joue en notre faveur côté OpenAI

| Moteur | Gateway dans le chemin d'inférence ? | Budget tokens (F+) enforçable par la gateway ? |
|---|---|---|
| **Anthropic direct** (Agent SDK) | Non — TLS direct, non MITM-able | **Non** — métrage tokens vit côté Anthropic (facturation / OTEL) |
| **OpenAI-compatible** | **Oui** — swap `base_url` → gateway | **Oui** — `proxy_llm` déjà là : credential wire-side, modèle imposé, `usage` réel, une entrée inference/appel |

« Cowork **ou** équivalent OpenAI » n'est donc pas symétrique : le monde OpenAI-compat
donne *en plus* la gouvernance de l'inférence (et le retour du métrage tokens dans le
gamma) que l'Anthropic-direct refuse. Choix de moteur = choix de ce qu'on peut gouverner.

Récupérer le métrage tokens **malgré** l'inférence Anthropic directe reste possible sans
MITM : ingérer l'export **OTEL de coût** du SDK et en dériver des entrées `inference`
dans le gamma (le mécanisme `record_inference` existe déjà, il lui manque un *feed*).
Décision d'archi ouverte.

## 6. Ce qui est déjà codé vs. à construire

| Brique | État (branche `feat/obligations`) |
|---|---|
| `proxy_mcp` (map outil → Op, verify, log-before-relay, relais) | **Fait, vert** |
| `McpRouter` agrégé multi-contexte (`tools/list`, routage `tools/call`) | **Fait** |
| `proxy_llm` OpenAI-compat (credential wire-side, modèle imposé, métrage) | **Fait** |
| `Vault` (`LocalVault` MVP ; `RemoteVault` = coffre desktop) | Trait posé ; `RemoteVault` à écrire |
| Provisioning (clé d'agent née dans l'hôte, N mandats de N Ethos) | Décidé (3bis.2) ; remplace l'auto-mint MVP |
| `proxy_web` (egress filtrant `act.x.web.*` + fenêtres + domaines + budgets) | **Stub — à construire** |
| **UX desktop** (multi-cerveaux, choix moteur, config MCP unique) | **À construire** (produit, pas protocole) |
| Packaging desktop du binaire gateway (statique, comme l'image `FROM scratch`) | À faire |

**Aucune feature protocole manquante** (cohérent avec `DEPLOYMENT-CONTAINMENT` §8) : le
travail restant est le runner desktop, `proxy_web`, `RemoteVault`, et l'UX.

## 7. Limites honnêtes (à ne jamais survendre)

- **Rerouter ≠ contenir — encore plus vrai sur desktop.** Sans namespace, une app peut
  ignorer la gateway et parler direct aux MCP si l'utilisateur les a aussi déclarés en
  direct. Le containment réel n'existe qu'en **Forme 2** (on héberge l'agent).
- **Ce qui s'exécute dans le cloud du provider échappe.** Connecteurs *intégrés*
  (OAuth côté serveur), web tool exécuté server-side : jamais sur le poste, donc jamais
  gouvernés. Seuls les outils **exécutés localement** passent par la couture.
- **Le piège de l'outil exec/shell** (repris de `DEPLOYMENT-CONTAINMENT` §6) : un outil
  « exécuter du code » avec réseau libre rouvre tout. En Forme 2 il doit être
  lui-même egress-locké vers la gateway.
- **Le canal Anthropic est un trou allowlisté** : épingler domaine/IP, vérifier qu'il
  n'est pas détournable en tunnel.

## 8. Le plus court chemin faisable (MVP Forme 2)

1. **Hôte desktop** embarquant l'Agent SDK (auth abo ou clé) + une boucle
   OpenAI-compat ; l'utilisateur choisit le cerveau.
2. **Gateway + coffre en local** (`LocalVault` d'abord). L'app pointe *son propre*
   client MCP vers la gateway → `proxy_mcp` déjà là.
3. **Moteur OpenAI** : `base_url` → gateway → `proxy_llm` déjà là → tokens métrés.
4. **`proxy_web`** pour l'egress local des outils (le chantier neuf).
5. **Provisioning** : clé d'agent née dans l'app, mandats émis depuis les outils de
   l'owner (jamais de clé d'owner dans le runtime desktop).

## 9. Questions ouvertes à trancher

- **Forme 1 (shim) vaut-elle une v0 « observabilité »** (hub MCP seul, assumé non-
  containment) pour un time-to-demo court, ou on va direct en Forme 2 ?
- **Métrage tokens Anthropic-direct** : feed OTEL→gamma, ou on assume que le budget
  tokens vit côté Anthropic et le gamma ne gouverne que les actes ?
- **Coffre** : `LocalVault` (clé en mémoire de l'app) suffit-il au MVP, ou `RemoteVault`
  (mobile séparé) dès le départ pour la thèse zero-knowledge ?
- **Cohabitation** avec les hubs MCP natifs des providers (ex. « MCP tunnels ») :
  se substituer, ou s'insérer en amont ?

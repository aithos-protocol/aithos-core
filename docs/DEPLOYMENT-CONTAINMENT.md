# Aithos — Architecture de déploiement & containment (doc produit)

> Document de vision/produit (interne, FR). **Hors protocole** : décrit la couche
> *au-dessus* d'aithos-core (le runner conteneurisé, le gateway, le réseau). Le
> protocole reste agnostique — il fournit les briques (`verify_chain`,
> `log_action`, `read.gamma`, `active_windows`, connecteurs), le runner les
> applique. Initié 2026-07-10 (discussion design global).

## 1. Objectif produit — première brique : l'audit externe

Permettre à une entreprise de **plugger un agent existant** avec Aithos en quelques
minutes, sans changer son fonctionnement, pour :

- le **cadrer** (enforcement : lecture seule, fenêtres, budgets, modèle) si besoin,
- **tracer** toutes ses actions dans un log inviolable (le gamma) hébergé par
  l'entreprise, pas par un tiers,
- ouvrir un **audit tiers scopé** : créer un mandat `read.gamma` pour un auditeur
  externe qui voit ce qu'on l'autorise à voir, et rien d'autre.

Positionnement : « déployez votre agent dans notre runner — il tourne pareil, mais
tout est cadré, tracé, et il ne peut rien faire hors de son mandat. »

## 2. Ce qui relève du protocole vs de la couche déploiement

| Garantie | Fournie par | Comment |
|---|---|---|
| Politique (qui peut quoi, quand, combien) | **Protocole** (core) | mandats + `verify_chain` |
| Log inviolable, auditable, scopé | **Protocole** (core) | gamma + `read.gamma` |
| Le gateway impose modèle/kind/périmètre | **Runner** (gateway) | tier X, la clé vit ici |
| **Le gateway est le seul chemin de sortie** | **Runner** (réseau) | egress lockdown |
| Résistance à l'évasion d'exécution | **Runner** (isolation) | Docker → microVM selon menace |

Règle d'or : **Aithos garantit que le canal géré est incontournablement fidèle ;
l'isolation garantit qu'il n'existe pas d'autre canal.** Les deux ensemble = le
containment. Ni l'un ni l'autre seul ne suffit — ne jamais survendre le reroutage
seul comme de la sécurité.

## 3. Architecture de référence : le runner (pod sidecar)

```
Runner (unité de déploiement livrée par Aithos, tourne sur un host de l'entreprise) :
  ├── Container AGENT
  │     • réseau INTERNE au pod, aucune route directe vers l'extérieur
  │     • non-privilégié : pas de CAP_NET_ADMIN, user non-root
  │     • ne détient AUCUNE clé Aithos
  │     • le runtime LLM + ses outils ; parle au gateway via localhost
  └── Container GATEWAY
        • deux pattes : interne (vers l'agent) + externe (vers le monde autorisé)
        • détient les clés (agent + ethos), signe le gamma, impose le mandat
        • NAT/proxy filtrant : décide chaque sortie (LLM API, MCP, web, ethos cloud)
        • egress externe vers : provider LLM, MCP interne, ethos (cloud, chiffré)
```

L'entreprise ne fournit que le **host** (nœud K8s, VM, Docker host) et son accès
internet existant. Nous fournissons le pod avec sa topologie réseau interne. On ne
dépend **pas** de la NetworkPolicy de l'entreprise.

## 4. Egress : incontournable + finement filtré (deux couches)

- **L3/L4 — lockdown namespace (incontournabilité).** L'agent n'a physiquement pas
  de route sauf vers le gateway (config du network namespace du pod, écrite par
  nous). Même s'il ignore tout proxy, il ne sort pas ailleurs. Tient **si** l'agent
  est non-privilégié (condition §3).
- **L7 — forward proxy (filtrage fin).** Le trafic HTTP(S) de l'agent passe par le
  gateway comme proxy : visibilité par domaine/URL, filtrage applicatif, log. Seul,
  contournable (un client peut l'ignorer) — d'où la couche L3/L4 dessous.

## 5. L'accès réseau EST un connecteur gouverné (le beau bouclage protocole)

L'accès internet de l'agent n'est pas un cas spécial : c'est une **action
connecteur** `act.x.web.fetch` (ou `net.*`), soumise au mandat comme n'importe
quelle action. Tout le vocabulaire F+ s'applique tel quel :

- **`active_windows`** → « internet le lundi 9h-18h » (fenêtre arithmétique absolue).
- **`domains`** → whitelist des destinations autorisées.
- **`budgets` / `max_actions`** → plafond de requêtes.
- **gamma `kind:"act"`** → chaque accès tracé, `target` = le domaine.

Le gateway lit le mandat, applique fenêtre + domaines + budget, autorise/bloque, et
logge. Ouvrir/fermer internet par fenêtres = le modèle de mandat appliqué au réseau,
**rien à inventer côté protocole**.

## 6. Niveaux d'isolation selon le modèle de menace

| Modèle de menace | Isolation suffisante | Complexité |
|---|---|---|
| Agent honnête mais faillible (déraille, hallucine) | Docker + egress lockdown + sidecar | faible (standard cloud-native) |
| Agent potentiellement hostile (prompt injection active) | microVM / gVisor / Kata (isolation kernel-level) | plus élevée |

Le POC valide le premier niveau. Le second n'est requis que si l'agent doit être
supposé activement malveillant (le namespace Docker seul peut être percé par un
exploit d'évasion kernel).

**Piège transversal** : un outil « exécuter du code / shell » rouvre tout (curl
depuis le sandbox de code). Cet outil, s'il existe, doit être **lui-même
egress-locked** (pas de réseau libre). Règle : aucun outil ne donne d'exécution
arbitraire avec réseau non médié.

## 7. Limites honnêtes (à ne jamais survendre)

- Le reroutage seul (changer les env vars) = **observabilité**, pas containment.
- L'audit est aussi complet que l'isolation est forte : sans egress lockdown,
  l'audit ne couvre que les flux qui passent par le gateway.
- On ne récupère pas le contexte interne du LLM (cache `/k/`) si l'agent est une
  boîte noire externe qu'on ne fait que proxifier — seulement si le runner l'héberge.
- L'ethos en cloud : illisible sans clé/mandat (ciphertext au repos), mais le cloud
  voit les *métadonnées d'accès* (tailles, timing) — limite standard du chiffrement
  au repos, à documenter le moment venu.

## 8. Ce que ça implique pour la roadmap

Le **runner** (pod agent + gateway sidecar + egress + médiation MCP/LLM/web) est le
prochain gros chantier après aithos-core, et vit **au-dessus** du protocole. Le core
lui donne déjà tout : identité, mandats, gamma, `read.gamma`, `active_windows`,
connecteurs. Il n'y a **pas** de feature protocole manquante pour ce MVP — le travail
restant est le runner et l'UX d'onboarding (config en X minutes).

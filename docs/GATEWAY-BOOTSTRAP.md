# Aithos Gateway — amorçage de construction (doc produit)

> Doc de démarrage (interne, FR) du **runner conteneurisé** : le composant qui
> plugge un agent existant avec Aithos. **Au-dessus** d'aithos-core, qu'il
> consomme comme bibliothèque. Voir `DEPLOYMENT-CONTAINMENT.md` pour le threat
> model et la topologie réseau ; ce doc-ci est le « par où on commence à coder ».
> Initié 2026-07-10.

## 1. But

Un binaire (le **gateway**) qui s'interpose entre un agent et ses dépendances
externes (LLM API, MCP interne, web), applique le **mandat** de l'agent, **logge**
chaque acte dans le gamma, et **détient les clés** que l'agent ne voit jamais.
Empaqueté en container, déployé en sidecar dans l'infra de l'entreprise. Résultat
vendable (première brique) : **l'audit externe** d'un agent qui continue de tourner
comme avant.

## 2. Positionnement : le gateway consomme aithos-core en bibliothèque

Le gateway est écrit en **Rust** et dépend des crates du workspace — pas d'appel CLI
en sous-processus, pas de FFI. Il réutilise directement le moteur de confiance :

| Besoin gateway | Fourni par aithos-core | Fonction (lib) |
|---|---|---|
| « Cet acte est-il autorisé, maintenant ? » | `aithos-core::mandate` | `verify_op(chain, did_doc, at, op)` |
| Chaîne de mandats valide à T (fenêtres, budgets, révocation) | `aithos-core::mandate` | `verify_chain(...)` |
| Tracer une action / une inférence | `aithos-bundle` | log d'entrée `act` / `inference` |
| Compter les budgets (max_actions, tokens, fenêtres) | `aithos-bundle` gamma | déjà dans le compteur F/F+ |
| Auditer (lecture scopée du log) | `aithos-bundle::log` | `LogFilter` / `read.gamma` |
| Accès à l'ethos chiffré (local ou cloud) | `aithos-bundle` | trait `Store` (fs, s3 à venir) |
| Sceller/ouvrir, dériver, signer | `aithos-core` | `seal`, `derive`, `header`, `keys` |

Le core ne bouge pas : c'est le gateway qui orchestre ces briques face au trafic
réel. **Aucune feature protocole manquante pour le MVP** (voir §8 du deployment doc).

## 3. Où ça vit

Nouveau crate **`aithos-gateway`** dans le workspace cargo existant (`rust/crates/`),
dépendant de `aithos-core` + `aithos-bundle`. Avantage : réutilise le build, les
deps, le CI, le rituel de test. Extraction en repo séparé plus tard, quand ça grossit
— sans coût, c'est déjà un crate autonome.

## 4. Composants à construire

```
aithos-gateway/
  policy/        moteur de politique : charge le(s) mandat(s), appelle verify_op,
                 mappe {requête entrante} → {Op aithos}, décide allow/deny
  keyholder/     détient la keypair de l'agent + les clés d'ethos ; signe le gamma ;
                 JAMAIS exposé à l'agent (process/mémoire séparés)
  proxy_mcp/     parle MCP ; expose les mêmes outils ; filtre par le mandat ; logge
  proxy_llm/     parle l'API provider (OpenAI-compat d'abord) ; impose model/budget ;
                 logge inference (méta seulement, jamais le prompt → cache /k/)
  proxy_web/     egress HTTP filtrant : act.x.web.* + active_windows + domains + budget
  store_adapter/ Store vers l'ethos (fs local d'abord, cloud chiffré ensuite)
  config/        onboarding : identité agent, mandat initial, endpoints, whitelists
```

Les trois proxies partagent le **même moteur de politique** : chaque requête devient
un `Op` (verbe + cible), passé à `verify_op` ; si ok, exécutée puis loggée ; sinon
rejetée fail-closed et loggée comme refus.

## 5. Flux d'une requête (exemple : appel d'un outil MCP)

```
agent → proxy_mcp : call tool "user.update(...)"
  1. policy: mappe → Op { act.x.mcp.update }
  2. verify_op(chain, did, now, op)   → refusé (lecture seule) → 403 + gamma "act(refusé)"
     (ou autorisé si outil read → continue)
  3. keyholder signe l'entrée gamma "act" (kind imposé, pas choisi par l'agent)
  4. proxy relaie vers le vrai MCP (localhost:4124), renvoie la réponse
  5. (option) consolidation : écrire un condensé dans l'ethos (zone scopée)
```

Le kind et le périmètre sont **imposés par le gateway** (l'agent n'a pas la clé,
ne fabrique aucune entrée) — c'est la décision d'archi « clé chez le container ».

## 6. Architecture container

Reprend `DEPLOYMENT-CONTAINMENT.md` : pod à deux containers (**agent** isolé sans
route directe + **gateway** en NAT filtrant qui tient les clés), egress lockdown
L3/L4 (namespace) + L7 (proxy), agent non-privilégié. On livre le pod ; l'entreprise
fournit le host. L'image gateway = binaire Rust statique `FROM scratch` (comme le
Dockerfile aithos-core existant).

## 7. MVP « audit externe » — le plus petit périmètre

Livrer d'abord le strict nécessaire pour vendre l'audit :

1. **`proxy_mcp` en lecture seule + log.** Mappe outils → `Op`, applique le mandat
   (read/write), logge chaque appel. C'est le cœur de la démo (« il ne peut plus
   écrire, et tout est tracé »).
2. **`keyholder` + `store_adapter` local.** Clés dans le gateway, ethos/gamma sur
   disque local d'abord (cloud ensuite).
3. **Onboarding `config`.** Une commande : découvre les outils MCP, génère
   identité + mandat lecture-seule, imprime les 2 endpoints à mettre côté agent.
4. **Export audit.** Un mandat `read.gamma` scopé pour un auditeur + une commande
   qui sort le log filtré (réutilise `LogFilter`).

Non-MVP (itérations suivantes) : `proxy_llm`, `proxy_web` + fenêtres, ethos cloud,
consolidation automatique, microVM.

## 8. Rituel de construction (identique à aithos-core)

- **Feature d'abord (BDD).** `features/*.feature` du gateway co-écrites avant le
  code (parcours : plug d'un agent, refus d'écriture, audit tiers).
- **Tests de surface.** Le proxy se teste en intégration (agent factice + MCP
  factice) ; `assert_cmd` pour la CLI d'onboarding (cf. `cli_surface.rs`).
- **Fail-closed partout.** Toute ambiguïté de politique → refus + log, jamais
  « laisser passer ».
- **Le LLM ne signe jamais, le gateway impose le kind** — testé explicitement.

## 9. Décisions à prendre avant de coder

- **`proxy_llm` v1** : cible-t-on l'API OpenAI-compatible d'abord (le plus répandu)
  ou Anthropic natif ?
- **Protocole MCP** : transport à supporter en premier (stdio / SSE / HTTP) ?
- **Ethos** : disque local en v1 puis cloud, ou cloud (S3) direct dès le MVP ?
- **Mapping outil→Op** : la config est fournie par l'entreprise (elle sait ce
  qu'elle veut cadrer) — quel format déclaratif (YAML de whitelist read/write) ?

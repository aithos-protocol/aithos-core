# Aithos — Hub MCP gouverné : n'importe quel MCP, sous mandat (design v1)

> **Statut : DESIGN TRANCHÉ v1 (décisions Mathieu, 2026-07-12 et
> compléments validés le 2026-07-13).** Note interne, FR.
> **Hors protocole** : tout est gateway (cohérent avec `EXPLORATION-DESKTOP-GATEWAY` §6 —
> « aucune feature protocole manquante »). Complète `GATEWAY-HANDOFF.md` (état du code)
> et `EXPLORATION-DESKTOP-GATEWAY.md` §4 (le hub agrégé comme cœur vendable).
> Rituel inchangé : décisions → feature Gherkin → impl.

## 1. L'idée en une phrase

Le gateway expose **un endpoint MCP unique** derrière lequel on peut brancher
**n'importe quel serveur MCP tiers** ; tout ce qui traverse est **borné par le
protocole** : couvert par un mandat, loggé avant relais dans le gamma de l'Ethos
octroyant, refusé fail-closed sinon — et le serveur amont lui-même est tenu par un
**manifeste approuvé et pinné** par l'owner, jamais cru sur parole à runtime.

## 2. Constat de départ : le McpRouter EST déjà le hub, mais fermé

La chaîne verte actuelle (Phase B close + proxy_llm) fait déjà le geste :
un `/mcp` unique, N contextes = N Ethos + N upstreams + N tool maps, routage par nom,
double mur (`authorize` puis re-vérif à l'append), **log-before-relay ×2** (acte
contexte + xref journal), refus routés §3bis.8, default-deny. Ce qui le rend fermé —
et qui est le périmètre exact de ce chantier :

| Manque | Aujourd'hui |
|---|---|
| Découverte des outils amont | tool maps YAML écrites à la main |
| Schémas réels | `tools/list` agrégé = noms seuls, inputSchema objet ouvert |
| Serveur partagé entre Ethos | 1 contexte = 1 upstream, soudés |
| Défense contre l'amont | le `tools/list` amont n'est jamais ingéré… donc jamais vérifié non plus |
| Credentials vers l'amont | rien (la cible §3bis.4 vault `/x/` existe pour `llm:`) |
| Méthodes au-delà de tools/* | `-32601` (assumé) |

## 3. Décisions tranchées (Mathieu, 2026-07-12)

1. **Topologie : le serveur est découplé des contextes.** Un serveur MCP est déclaré
   **une fois** (connexion + manifeste pinné) et peut porter des outils grantés par
   **plusieurs Ethos / plusieurs mandats**. C'est le grant (l'entrée de tool map d'un
   contexte, pointant un outil d'un serveur) qui accroche l'outil à l'Ethos dont le
   mandat le couvre et dont le gamma reçoit l'acte. Le serveur ne « appartient » à
   personne ; le mandat, si.
   - Corollaire v1 : **un nom d'outil exposé → exactement un contexte couvrant**
     (même règle d'ambiguïté que les collisions actuelles, rejet à la config).
     Le même outil granté par deux Ethos rendrait le routage — et la preuve —
     ambigus ; si le besoin émerge, ce sera une décision séparée (désambiguïsation
     à l'appel, jamais silencieuse).
2. **Exposition : `tools/list` ne sert QUE les outils couverts** par un mandat, avec
   les schémas pinnés. L'agent ne voit jamais la description d'un outil non gouverné
   (zéro surface de prompt-injection gratuite). Nuance interne : les outils connus
   mais non grantés (les `write` d'aujourd'hui) **restent dans la map interne** pour
   que les refus les nomment précisément — ils ne sont juste jamais listés.
3. **Pin intégral strict.** Le hash d'approbation couvre nom + inputSchema +
   description, par outil. Tout drift amont à runtime = **refus fail-closed** +
   entrée de gouvernance (clé du gateway). Absorber un changement = re-enroll
   explicite (diff montré à l'owner, re-grant). Un changement de description est un
   événement d'audit, pas un bruit.
4. **Périmètre v1 : `tools/*` seulement.** `initialize` (statique), `tools/list`
   (couverts + schémas pinnés), `tools/call` (routé). Le reste `-32601`.
   `resources/*` et `prompts/*` viendront avec leur propre mapping de verbes
   (ex. `resources/read` = classe read), jamais en passthrough.

## 4. Le cœur du design : le manifeste approuvé (l'owner signe à la place du tiers)

La spec §08.1 veut un **manifeste signé** par connecteur (actions + classes de risque
`read`/`act`/`binding`). Un MCP tiers ne signe rien — mais l'owner peut **figer sa
parole au moment du grant** :

- **Enroll** (owner-side, hors chemin chaud) : `discover` capture le `tools/list`
  amont → **manifeste proposé** (par outil : nom aplati, inputSchema, description,
  hash, classe de risque proposée `read` par défaut refusable) → l'owner classe et
  approuve → le manifeste approuvé est scellé dans l'Ethos octroyant → le mandat est
  minté vers la pubkey de l'agent en couvrant les actions **de ce manifeste**.
- **Run** : le hub ne fait **jamais** confiance à l'amont. La vérité runtime =
  manifeste approuvé + mandat. Le `tools/list` agrégé est reconstruit depuis les
  manifestes pinnés (l'amont n'est même pas consulté) ; à chaque `tools/call`, si la
  réponse d'un `tools/list` de contrôle ou la forme de l'outil a dérivé du pin →
  refus + gouvernance.
- **Evolve** : re-discover → diff → nouvelle approbation → **nouveau mandat, même
  keypair** (I2 : widening natif §04.1), révocation politique de l'ancien, le tout
  loggé (`grant` + `revoke`).

Ça retourne la faiblesse actuelle (YAML statique) en thèse produit : la staticité
devient un **pin gouverné**, et le rug-pull/tool-poisoning amont devient un refus
tracé au lieu d'un vecteur d'attaque.

**Stockage du manifeste approuvé — décidé (Mathieu, 2026-07-13)** : le vault `/x/<server>`
de chaque Ethos octroyant (§08.2 : « config de connecteur », gardien = gateway, ligne
grantée). Chaque Ethos pinne **sa** vue du serveur (au minimum le sous-ensemble
d'outils qu'il grante) : duplication assumée, souveraineté par Ethos — un contexte
peut re-enroller sans toucher les autres.

## 5. Namespacing (la contrainte d'aplatissement, prolongée)

Les actions d'act se coupent au dernier point : `act.x.<connector>.<action>`. Avec des
serveurs arbitraires, `connector` = **l'id du serveur enrollé**, `action` = le nom
d'outil amont aplati (points → underscores, règle actuelle). Deux serveurs qui
exposent tous deux `search` ne collisionnent donc plus par nature :

```
outil amont "search" du serveur github   → périmètre act.x.github.search
outil amont "search" du serveur linear   → périmètre act.x.linear.search
nom exposé à l'agent (proposition)       → github__search / linear__search
```

- **Nom exposé** : `<server>__<tool_aplati>` (double underscore, charset MCP-safe —
  les points sont fragiles chez certains clients). Le nom amont **brut** reste dans
  le payload clair (`tool`), comme aujourd'hui ; le relais amont renvoie le nom brut.
- Collisions post-aplatissement (`a` + `b__c` vs serveur `a__b` + outil `c`) : rejet
  à la config, règle inchangée. **Décidé 2026-07-13 :** `__` reste autorisé dans les
  ids de serveurs ; la collision est détectée sur le nom exposé calculé, jamais
  évitée par une interdiction de charset.
- Préfixes réservés : `journal` (outils natifs du lot C2) — un serveur enrollé ne
  peut pas s'appeler `journal` (ni, plus tard, `gateway`).

## 6. Forme de config (v3, esquisse — fail-closed comme toujours)

```yaml
listen: 127.0.0.1:4870
servers:                      # ressources de première classe, partagées
  - name: github
    transport: http           # stdio: wrapper, chantier séparé
    url: https://mcp.github.example/mcp
contexts:
  - name: company-brand
    store: { kind: fs, root: /var/lib/aithos/brand }
    tools:
      github__create_issue: { server: github, tool: create_issue, access: read }
      github__merge_pr:     { server: github, tool: merge_pr,     access: write }  # connu, refusé, jamais listé
journal:
  store: { kind: fs, root: /var/lib/aithos/journal }
```

Validations : formes mono/multi/hub exclusives ; `server` inconnu → rejet ; un même
`(server, tool)` granté par deux contextes → rejet (décision 3.1) ; collisions
d'aplatissement → rejet ; noms réservés → rejet.

## 7. Flux runtime `tools/call` (delta sur l'existant, rien de retiré)

1. `resolve(nom_exposé)` → (contexte, serveur, outil amont) — default-deny.
2. **Vérif pin** : l'outil appelé est conforme au manifeste approuvé de l'Ethos
   (forme + hash). Drift → refus `manifest_drift` (gouvernance).
3. `authorize` sur le mandat du contexte à T (inchangé).
4. **Log-before-relay ×2** (inchangé) : acte dans le gamma du contexte
   (`act.x.<server>.<tool>`, args hashés), xref journal.
5. Relais vers le serveur (nom amont brut restauré dans `params.name`).
6. Refus : §3bis.8 inchangé (journal toujours, contexte si gouvernance grantée).

## 8. Ce qui existe vs à construire

| Brique | État |
|---|---|
| Routage, double mur, log ×2, refus routés, journal | **Fait, vert** (Phase B) |
| `proxy_llm` (métrage inférence, budgets F+) | **Fait, vert** (Phase C) |
| Config v3 `servers:` + tools référencées | **Fait, vert** (H1) |
| `discover` / manifeste proposé / approbation / pin | À construire (owner-side) |
| Vérif pin à runtime + refus drift | À construire |
| `tools/list` = couverts + schémas pinnés | À construire (remplace noms-seuls) |
| Nom exposé `<server>__<tool>` + restauration du nom brut | À construire |
| Credentials amont (bearer/OAuth) | Couture v1 = config (comme `llm:`), cible vault §3bis.4 |
| Wrapper stdio, SSE/streaming, `resources/*` | Hors v1 (Phase D) |

## 9. Lots proposés (Gherkin-first, dans l'ordre)

- **H0 — contrat.** `gateway-hub.feature` : scénarios écrits AVANT le code, committés
  seuls. Couvre : enroll heureux, tools/list = couverts seulement + schémas pinnés,
  call routé vers le bon Ethos (2 Ethos, 1 serveur partagé), refus write connu
  (nommé, jamais listé), drift de manifeste → refus + gouvernance, re-enroll =
  nouveau mandat + revoke politique, noms réservés/collisions rejetés à la config.
- **H1 — config v3 : ✅ CLOS (2026-07-13).** `servers:` de première classe
  (`name`/`transport:http`/`url`), références de tools
  `{server, tool, access}`, formes mono/multi/hub exclusives, serveur inconnu,
  `(server,tool)` inter-contextes, réservations et collisions d'aplatissement
  intra/inter-serveurs rejetés ; `deny_unknown_fields` imbriqué. Les configs v1/v2
  restent compatibles ; le runtime hub refuse explicitement jusqu'à H3.
- **H2 — enroll owner-side** : `discover` (capture), manifeste proposé, approbation,
  pin dans l'Ethos, grant (réutilise `owner-grant-context`).
- **H3 — runtime** : pin check, `tools/list` reconstruit des manifestes, nom exposé.
- **H4 — e2e réseau** : 2 faux MCP dont 1 partagé par 2 Ethos ; drift simulé sur le
  fil ; audit-export par contexte montre les actes du serveur partagé chez le bon.

## 10. Compléments décidés (Mathieu, 2026-07-13)

- **Stockage** : vault `/x/<server>` par Ethos octroyant (§4), duplication souveraine
  assumée.
- **Contrôle de drift** : cohérence locale gratuite à chaque call ; `tools/list`
  amont à l'ouverture de session et sur erreur amont. Pas de round-trip ajouté à
  chaque call heureux.
- **Classes de risque v1** : `read`/`write`, avec représentation extensible à
  l'enroll ; `read`/`act`/`binding` attend les manifests connecteurs §08.1.
- **Credentials amont v1** : bearer en config, couture temporaire assumée comme
  `llm.api_key`; cible inchangée = vault `/x/`.
- **Ambiguïté inter-serveurs** : détection post-aplatissement ; `__` n'est pas
  interdit dans les ids de serveurs.

# Handoff — finalisation gateway : coffre de credentials + hub MCP de démonstration

> **ARCHIVE — plan exécuté.** Le coffre et le hub ont depuis des preuves `DONE`
> et des extensions OAuth plus récentes.

**Date :** 2026-07-15  
**Branche constatée :** `feat/obligations`  
**HEAD constaté :** `e9d2a8d` (`docs: finalize governed hub handoff`)  
**Statut :** handoff de cadrage, aucun code modifié dans cette session  
**Prochain agent :** lire d'abord `AGENTS.md`, `docs/GATEWAY-HANDOFF.md`,
`docs/HUB-MCP.md`, `docs/GATEWAY-BOOTSTRAP.md`, puis ce document.

---

## 1. Mission

Finaliser une **démo crédible de la gateway Aithos** dans laquelle :

1. l'entreprise expose plusieurs serveurs MCP derrière un endpoint gateway unique ;
2. l'owner découvre, approuve et accorde explicitement les outils ;
3. le mandat Aithos cadre chaque appel et le gamma le journalise avant effet ;
4. l'agent ne reçoit, ne lit et ne journalise **jamais** les tokens des serveurs MCP ;
5. les tokens proviennent d'un coffre standard d'entreprise, utilisable aussi bien
   localement pour la démo que sur une instance distante de l'entreprise ;
6. aucune indisponibilité ou erreur du coffre ne provoque un appel MCP sans credential
   ou hors mandat : tout échoue **fail-closed**.

Le lot prioritaire est le coffre de credentials MCP. Ne pas le mélanger avec
`RemoteStore/S3`, qui concerne la persistance des Ethos et n'est pas requis pour cette
démo.

---

## 2. Décision produit : HashiCorp Vault KV v2 comme référence de démo

### Choix principal

Implémenter un broker **HashiCorp Vault KV v2**.

Pourquoi :

- Vault est une référence courante dans les infrastructures d'entreprise ;
- Vault Community est installable gratuitement ;
- l'image Docker officielle permet une démo locale reproductible sans compte cloud ;
- le même protocole HTTP vise ensuite un Vault Community/Enterprise/HCP distant ;
- KV v2 stocke et versionne des secrets arbitraires et expose une API HTTP stable.

Pour la démo automatique, utiliser Vault en mode `dev` **uniquement**. Le mode dev est
in-memory, auto-unsealed, sans TLS par défaut et explicitement interdit en production.

Sources officielles :

- <https://developer.hashicorp.com/vault/docs/get-started/developer-qs>
- <https://developer.hashicorp.com/vault/docs/concepts/dev-server>
- <https://developer.hashicorp.com/vault/docs/secrets/kv/kv-v2>
- <https://developer.hashicorp.com/vault/docs/get-vault>

### Alternative SaaS gratuite, non bloquante

Infisical Cloud annonce un plan `Free` à 0 USD/mois, utilisable en cloud ou self-hosted,
avec API/CLI/SDK et jusqu'à cinq identités. C'est une bonne seconde intégration si une
démo sans Docker devient obligatoire, mais **ne pas construire deux adapters dans le
premier lot**.

- <https://infisical.com/pricing>
- <https://infisical.com/docs/documentation/getting-started/concepts/deployment-models>

L'abstraction doit rendre cet adapter ultérieur possible sans modifier le routeur MCP.

---

## 3. Terminologie : trois objets à ne pas confondre

### `CredentialBroker` — chantier immédiat

Résout un token MCP depuis une référence non secrète et l'injecte uniquement dans
l'appel HTTP amont. C'est lui qui répond au besoin de la démo.

### `KeyVault` / `Keyholder` — chantier distinct

Garde les seeds de signature et les clés de déchiffrement Aithos. Aujourd'hui
`Keyholder` garde deux seeds, les persiste en `0600`, puis `core_bridge` emprunte encore
les octets via `agent_seed()` / `gateway_seed()`. Le trait `Vault { sign, unseal,
session_key }` est documenté mais pas implémenté.

Ne pas refactorer la garde des clés en même temps que le broker de tokens, sauf couture
minimale indispensable. Ce serait une PR séparée après la démo.

### `Store` — hors sujet ici

Stocke bundle/gamma (`FsStore`, plus tard éventuellement distant). Un Store n'est pas un
coffre de credentials. Aucun `RemoteStore`/S3 n'est requis pour la démo Vault.

---

## 4. État actuel vérifié

### Déjà vert

- Hub MCP HTTP gouverné H0→H4 et H2b fermé.
- Config v3 `servers:` et contexts multi-Ethos.
- Discovery stricte de `tools/list`.
- Approbation explicite `read|write` par l'owner.
- Manifestes pinnés/scellés sous `/x/<server>/manifest.enc`.
- `tools/list` agent reconstruit hors ligne depuis les pins couverts.
- Routage `<server>__<tool>` vers le nom MCP amont brut.
- Vérification du pin et du mandat avant appel.
- `log-before-relay` : gamma contexte + xref journal.
- Contrôle de drift au démarrage et après erreur amont.
- Bearer observé uniquement sur le fil amont dans l'E2E H4.
- Re-enrollment : nouveau pin/mandats et révocation politique des anciens.

Dernière vérification complète de la session précédente :

- 36 tests unitaires ;
- 4 tests CLI ;
- 41 scénarios Cucumber / 202 steps ;
- 4 E2E réseau ;
- 5 tests owner-side ;
- Clippy `-D warnings` vert.

Relancer avant toute modification :

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-core/rust
CARGO_TARGET_DIR=/tmp/aithos-core-gateway-vault-target \
  CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
CARGO_TARGET_DIR=/tmp/aithos-core-gateway-vault-target \
  CARGO_INCREMENTAL=0 cargo clippy -p aithos-gateway --all-targets -- -D warnings
```

### Dette exacte qui bloque la démo coffre

`ServerConfig` contient encore :

```rust
pub bearer_token: Option<String>
```

`main.rs` clone cette chaîne dans `HttpUpstream`, puis `proxy_mcp.rs` applique
`bearer_auth(token)`. Le token reste invisible à l'agent mais existe en clair dans
`gateway.yaml` et durablement dans la mémoire du processus.

Même dette côté LLM : `LlmConfig.api_key: String`. Le premier lot peut viser MCP ;
l'adaptation LLM doit ensuite réutiliser exactement le même broker.

---

## 5. Architecture cible minimale

### Interface runtime

Introduire une abstraction object-safe et async, par exemple :

```rust
pub trait CredentialBroker: Send + Sync {
    fn resolve<'a>(
        &'a self,
        reference: &'a CredentialRef,
    ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>>;
}
```

Noms indicatifs : les adapter au style du repo. Propriétés obligatoires :

- le routeur et la policy ne connaissent que `CredentialRef` ;
- aucune valeur secrète ne doit circuler comme un `String` nu ; son wrapper ne doit implémenter ni `Debug` ni `Serialize` ;
- `SecretValue` zeroize son buffer au `Drop` ;
- erreurs expurgées : jamais de valeur, header ou réponse brute du coffre ;
- aucun secret dans le manifeste approuvé ni dans le gamma ;
- le secret est résolu au plus tard possible, juste avant l'appel amont ;
- rotation dans Vault sans changement de config ; privilégier résolution par appel
  pour la première version, sans cache secret.

Note honnête : `reqwest`/HTTP peut copier le header dans ses buffers ; on garantit que
le token ne franchit jamais la frontière gateway→agent et qu'il n'est jamais persisté,
pas que le processus gateway n'en possède aucun octet. Pour qu'il ne traverse même pas
la mémoire du gateway, il faudrait un proxy d'egress injecteur de credentials séparé,
hors périmètre de cette démo.

### Configuration proposée

La config ne contient que l'adresse du coffre et des références non secrètes :

```yaml
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN

servers:
  - name: github
    transport: http
    url: http://127.0.0.1:9101/mcp
    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
```

Le token MCP est créé dans Vault avec KV v2 :

```bash
vault kv put -mount=secret aithos/mcp/github token="$GITHUB_MCP_TOKEN"
```

Le token d'accès **au coffre** est lui-même fourni au gateway via une variable
d'environnement dans la démo. Ne jamais le mettre dans YAML, les arguments CLI, le
gamma ou un fichier du repo.

Pour un usage entreprise, documenter ensuite AppRole/Kubernetes auth. L'API AppRole est
faite pour les workflows machine ; ne pas rendre AppRole bloquant pour le premier E2E.

### Compatibilité

Ajouter `credential:` de manière additive. Conserver temporairement
`bearer_token:` pour les anciens scénarios/configs, mais :

- refuser `credential` + `bearer_token` simultanés ;
- marquer l'inline legacy/unsafe dans la doc ;
- la démo et tous les nouveaux tests doivent utiliser `credential` ;
- ne pas supprimer la couture legacy avant que le parcours Vault soit vert.

---

## 6. Séquence de PR obligatoire — BDD d'abord

### V0 — contrat rouge, commit seul

Créer `rust/crates/aithos-gateway/tests/features/gateway-vault.feature` avec `@wip`,
sans implémentation. Scénarios minimaux :

1. un outil granté récupère son bearer dans Vault KV v2 et l'amont le voit ;
2. l'agent ne voit le bearer dans aucune réponse MCP ;
3. token absent du YAML, des stores Ethos, gammas, journal et stderr ;
4. outil inconnu ou non granté : zéro requête au coffre et zéro requête MCP amont ;
5. Vault indisponible : refus avant MCP amont + journalisation du refus ;
6. path/field absent ou réponse Vault malformée : refus fail-closed ;
7. deux serveurs utilisent deux références différentes sans confusion ;
8. rotation de la valeur KV : l'appel suivant utilise la nouvelle valeur sans
   modifier la config ;
9. `tools/list` ne consulte ni Vault ni l'amont ;
10. `credential` + `bearer_token` est rejeté à la config.

Écrire également un test unitaire de redaction : `Debug` et chaque erreur du broker ne
contiennent ni le token MCP ni le token Vault.

### V1 — config + abstraction

- `CredentialBroker`, `CredentialRef`, `SecretValue`.
- Parsing `credential_brokers` et `server.credential` avec
  `deny_unknown_fields`.
- Validation : broker/path/field/env non vides, broker référencé existant, exactement
  une source de credential.
- HTTP(S) seulement ; autoriser HTTP uniquement pour loopback/dev explicite.
- Aucun accès réseau dans `tools/list` ou la phase de policy.

### V2 — adapter Vault KV v2 + câblage runtime

- Client HTTP Vault KV v2 : `GET /v1/<mount>/data/<path>`.
- Auth démo via `X-Vault-Token` lu depuis la variable nommée dans la config.
- Lecture stricte de `data.data.<field>` ; types inattendus rejetés.
- Timeout borné ; statuts non-2xx expurgés ; aucune reprise fail-open.
- `HttpUpstream` demande le secret au broker uniquement après : resolve → pin →
  authorize → log-before-relay.
- Le bearer est appliqué sur le seul fil MCP amont.
- Une erreur broker produit un code de refus stable, jamais le contenu de l'erreur
  distante.

### V3 — E2E durable + vraie démo

Deux niveaux :

1. **CI durable sans Docker** : faux serveur Vault HTTP local + deux faux MCP, vraies
   sockets, vrai binaire. Vérifier compteurs de hits et absence des secrets partout.
2. **Runbook manuel avec vrai Vault** : Docker officiel `hashicorp/vault`, KV v2,
   deux secrets MCP, deux faux MCP ou deux MCP de démo, puis appel via le gateway.

Créer un runbook `docs/DEMO-GATEWAY-VAULT.md`. Ne jamais inscrire de valeur de token
réelle dans ce document ou dans Git. Les valeurs de démo doivent être générées ou
fournies par environnement et explicitement marquées non-production.

### V4 — réutilisation par `proxy_llm`

Remplacer le chemin sûr `llm.api_key` par une `CredentialRef` utilisant le même broker.
Conserver temporairement l'inline legacy comme pour MCP. Ajouter le scénario réseau :
credential Vault visible uniquement chez le faux provider LLM.

---

## 7. Ce que signifie « toutes les ressources MCP »

Ne pas masquer cette ambiguïté dans le handoff.

### Sens générique : tous les outils accordés par l'entreprise

Le hub sait déjà agréger N serveurs et N contextes, mais il ne sert actuellement que
les pins `read`. Une entrée classée `write` signifie encore « connue mais non grantée ».

Après la démo Vault, séparer explicitement :

- **classe de risque** de l'outil : `read|write` ;
- **décision d'octroi** : granté ou non ;
- contraintes du mandat : fenêtres, budgets, périmètre, obligations.

Puis permettre à l'owner de granter un outil classé `write` sans rendre tous les writes
accessibles. Les writes grantés doivent apparaître dans `tools/list`, passer par le même
double mur et être journalisés avant relais. Les writes connus mais non grantés restent
cachés et refusés précisément.

Cette évolution exige un nouveau contrat BDD séparé ; ne pas la glisser dans V1/V2.

### Sens normatif MCP : `resources/list`, `resources/read`, templates

Le hub v1 ferme actuellement tout sauf `initialize`, `tools/list`, `tools/call`.
L'agrégation des primitives MCP `resources/*` n'existe pas. La traiter après le lot
credentials, avec :

- namespace serveur sans collision ;
- manifestes/pins pour URIs et templates ;
- mandat de lecture ;
- aucune récupération de credential pendant un simple listing local ;
- lecture journalisée avant appel amont ;
- refus fail-closed et drift.

Pour la première démo, « ressources » peut être montré par les **outils MCP grantés**.
Ne pas annoncer le support de la primitive MCP Resources tant que ses tests ne sont pas
verts.

---

## 8. Invariants de sécurité et tests de non-fuite

À vérifier dans chaque PR :

- l'agent ne peut jamais choisir le header d'authentification ;
- supprimer/rejeter `Authorization`, `X-Vault-Token` et équivalents provenant des
  arguments agent avant relais ;
- secret jamais dans `Debug`, `Display`, panic, stderr, JSON-RPC, gamma, journal,
  manifestes, config sérialisée ou store ;
- aucune valeur secrète dans les arguments de processus ;
- outil refusé → aucun hit Vault et aucun hit MCP ;
- coffre refusé/timeout/malformé → aucun hit MCP ;
- log contexte + xref journal réussissent avant la résolution du secret et le relais ;
- aucun secret n'est envoyé au LLM ;
- les appels au Vault distant exigent TLS, sauf mode démo loopback explicitement
  borné ;
- token Vault de démo = policy minimale en dehors du mode root-dev ;
- ne jamais présenter le mode Vault dev comme déployable en production.

Le container agent doit rester séparé du filesystem, des variables d'environnement et
de la mémoire du gateway. Sans cette isolation, « token invisible sur MCP » est de
l'observabilité, pas du containment.

---

## 9. Acceptation de la démo

La tranche est terminée quand une commande/runbook reproductible démontre :

1. Vault Community démarre localement gratuitement ;
2. GitHub/Linear fictifs ont chacun un token distinct dans KV v2 ;
3. le YAML gateway ne contient que des références ;
4. l'agent liste les outils grantés des deux serveurs derrière un endpoint unique ;
5. un appel granté passe, est loggé dans le bon Ethos + journal, et l'amont voit son
   bearer ;
6. un appel non granté est refusé et ne touche ni Vault ni l'amont ;
7. une panne Vault bloque l'appel ;
8. une rotation KV est prise en compte sans ré-enrollment ni modification YAML ;
9. une recherche récursive des valeurs sentinelles dans tous les artefacts gateway ne
   trouve rien hors observation wire-side du faux Vault/faux MCP ;
10. tests, fmt et clippy sont verts.

Commandes finales :

```bash
cargo fmt --check --manifest-path rust/Cargo.toml
cargo test -p aithos-gateway --manifest-path rust/Cargo.toml
cargo clippy -p aithos-gateway --all-targets --manifest-path rust/Cargo.toml -- -D warnings
```

Puis écrire un nouveau handoff : fichiers, commits, résultats exacts, limites restantes,
et mode de reproduction de la démo.

---

## 10. Limites et gates

- Ne pas déployer en production.
- Ne pas merger `main`/`master` sans gate humain.
- Ne pas toucher à des tokens ou données utilisateur réels.
- Ne jamais stocker un token dans le repo, un handoff ou un argument CLI visible.
- Ne pas commencer `RemoteStore/S3`, desktop UX ou `proxy_web` dans cette tranche.
- Ne pas confondre l'intégration Vault de credentials avec la future garde distante des
  clés Aithos.
- Les scories non suivies `_gitjunk/`, `_to_delete/`, `_transfer/` et
  `docs/EXPLORATION-DESKTOP-GATEWAY.md` appartiennent à l'état existant : ne pas les
  supprimer ou les inclure par accident.

---

## 11. Ordre de reprise recommandé

1. Vérifier branche/HEAD et arbre sale sans modifier les scories.
2. Relancer la suite gateway existante.
3. Commit V0 : feature Vault `@wip` uniquement.
4. V1 config/abstraction jusqu'aux tests unitaires verts.
5. V2 adapter KV v2 + détag progressif des scénarios.
6. V3 E2E réseau + runbook vrai Vault.
7. Faire valider la démo par Mathieu.
8. Ensuite seulement : V4 LLM, octroi réel des tools write, puis MCP `resources/*`.

Le résultat recherché n'est pas « Vault sait garder un token ». Le résultat produit est :
**l'entreprise donne à l'agent des capacités MCP gouvernées ; seul le gateway échange
avec le coffre et présente les credentials aux serveurs, après autorisation et audit.**

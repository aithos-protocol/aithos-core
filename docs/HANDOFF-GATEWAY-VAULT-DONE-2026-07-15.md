# Handoff — coffre de credentials MCP : tranche V0→V3 CLOSE

**Date :** 2026-07-15 (7ᵉ session gw, profil cloud+janitor)
**Branche :** `feat/obligations` (jamais switchée)
**HEAD d'arrivée :** `e9d2a8d` — **HEAD de sortie :** le commit de ce handoff,
au-dessus de `916ecb3`
**Répond à :** `docs/HANDOFF-GATEWAY-VAULT-FINALIZATION-2026-07-15.md`
(l'input, resté untracked — décision Mathieu), `docs/GATEWAY-HANDOFF.md`,
`docs/HUB-MCP.md` §8/§10.
**Statut :** V0→V3 verts et committés. **V4 (LLM) non commencé** — gate :
validation de la démo par Mathieu (§11.7 de l'input).

---

## 1. Ce qui est livré

Le hub gouverné ne porte plus ses tokens MCP en YAML : chaque serveur déclare
une **référence non secrète** `{broker, path, field}` et le gateway résout le
bearer dans un **HashiCorp Vault KV v2** — par appel, au plus tard possible,
**après** mandat et log-before-relay. L'agent, la config, les stores, les
gammas, le journal et la sortie du process n'exposent jamais un octet de
secret ; toute défaillance du coffre ferme la route **fail-closed** avant tout
contact amont.

### Les quatre commits (sur `feat/obligations`, staging sélectif)

| Tranche | Commit | Contenu |
|---|---|---|
| V0 — contrat rouge, commit seul | `9dd81fc` | `tests/features/gateway-vault.feature` : 10 blocs @wip écrits avant le code |
| V1 — config + abstraction | `ea224d3` | module `src/credentials.rs` (`CredentialRef`/`SecretValue`/`CredentialBroker`), `credential_brokers:` + `servers[].credential`, variante `CredentialUnavailable`, scénario config détaggé |
| V2 — adapter KV v2 + câblage | `34dfd22` | `VaultKv2Broker`, mode brokered de `HttpUpstream`, refus `credential_unavailable` au routeur, harness cucumber sockets réels, les 9 scénarios runtime détaggés |
| V3 — e2e réseau + démo | `916ecb3` | `tests/e2e_vault.rs` (CI durable sans Docker), `examples/demo_mcp.rs`, `docs/DEMO-GATEWAY-VAULT.md` (runbook vrai Vault) |

Chaque commit intermédiaire est cohérent (le contenu committé à V1 compile et
passe sa suite de l'époque ; le fmt final est porté par V2).

### Résultats exacts (fin de session, cloud rustc 1.95.0)

- **55** tests unitaires lib (36 → +12 config, +7 credentials dont les tests
  de **redaction adversariale** du broker) ;
- **4** tests CLI (`cli_surface`) ;
- **53 scénarios / 269 steps** Cucumber (41/202 → +12 : les 10 blocs vault,
  l'outline comptant double) — zéro `@wip` vault ;
- **5 e2e réseau** : `e2e_http`, `e2e_multi`, `e2e_llm`, `e2e_hub` +
  **`e2e_vault`** (nouveau) ;
- **5** tests owner-side (`owner_surface`) ;
- `cargo clippy -p aithos-gateway --all-targets -- -D warnings` : vert ;
- `cargo fmt --check -p aithos-gateway` : clean. NB : la commande du §9 de
  l'input (`cargo fmt --check --manifest-path rust/Cargo.toml` depuis la
  racine) répond « Failed to find targets » sur ce workspace virtuel — passer
  par `-p aithos-gateway` (ou `cd rust && cargo fmt --check`).

La baseline d'arrivée avait été revalidée à l'identique de l'input
(36/4/41/202/4 e2e/5 owner, clippy vert) avant toute modification.

---

## 2. Architecture posée (décisions d'implémentation)

- **`CredentialRef`** — la moitié visible : `broker` (nom d'entrée
  `credential_brokers`), `path` (segments KV), `field`. Désérialisée
  `deny_unknown_fields`, validée charset strict (pas de `.`/`..`/segments
  vides ; field plat).
- **`SecretValue`** — ni `Debug`, ni `Display`, ni `Serialize`, ni `Clone` ;
  lecture unique `expose()` au fil ; buffer zeroizé au `Drop`. Toute struct
  qui l'embarquerait perd ces derives — c'est l'enforcement. (Nos propres
  tests ont dû contourner `unwrap_err()` : la propriété bloque la stdlib.)
- **`CredentialBroker`** — trait object-safe async (`Pin<Box<dyn Future>>`),
  la forme exacte proposée par l'input. Infisical/SM cloud = adapters futurs
  derrière le même seam, sans toucher le routeur.
- **`VaultKv2Broker`** — un `GET <addr>/v1/<mount>/data/<path>` par
  résolution, `X-Vault-Token` lu de l'env **à chaque appel** (zeroizé après
  usage), timeout borné 5 s, lecture stricte de `data.data.<field>` (string
  non vide sinon refus). **Zéro cache secret** → la rotation KV est effective
  à l'appel suivant, sans changement de config ni redémarrage.
- **Erreurs expurgées structurellement** : les messages sont construits de
  statuts et de causes fixes (« vault answered status 500 », « vault is
  unreachable », « no usable field \`x\` ») — jamais du corps de réponse, d'un
  header ou d'une valeur. Prouvé par tests unitaires contre un faux Vault qui
  farcit ses réponses de sentinelles.
- **Ordre runtime** (décision structurante) : resolve → pin → authorize →
  **log acte + xref** → *résolution du secret dans `HttpUpstream::forward()`*
  → relais. Une panne coffre après log produit : **acte d'intention loggé**
  (le log dit « j'allais relayer ») **puis refus `credential_unavailable`**
  appariés dans le gamma du contexte ET le journal — et **zéro octet vers
  l'amont** (la résolution précède l'envoi HTTP dans `forward()`). Le routeur
  saute la sonde de drift dans ce cas (elle ne ferait que réveiller le coffre
  pour une route déjà fermée).
- **`HttpUpstream::for_server(server, &brokers)`** — le constructeur partagé
  binaire/harnesses : `credential` → brokered, sinon `bearer_token` legacy,
  sinon rien. `credentials::build_brokers(&cfg)` construit la map une fois au
  démarrage.
- **Config** : `credential_brokers` exige la forme hub ; map non vide ; noms
  au charset serveur ; `vault-kv2`/`token-env` seuls kinds (fail-closed) ;
  adresse http(s) avec **http borné au loopback** (IPv4 127/8 parsée, jamais
  un préfixe de nom d'hôte — `http://127.evil.example` est refusé —, plus
  `localhost` et `[::1]`) ; `credential` + `bearer_token` simultanés rejetés
  (« exactly one credential source per server ») ; broker inconnu rejeté ;
  broker déclaré non référencé toléré. La couture `bearer_token` reste
  parseable, marquée LEGACY/UNSAFE en doc de code — **ne pas la retirer avant
  validation de la démo réelle**.
- **Contrôle de session-open** (`verify_hub_upstreams`) : les `tools/list` de
  contrôle passent par le même `forward()` → ils portent le bearer résolu et
  exigent un coffre disponible au démarrage — fail-closed dès l'ouverture.
- **`tools/list` agent** : reconstruit des pins, ne touche ni coffre ni
  amont (compteurs à zéro observés en cucumber ET en e2e).

## 3. Preuves (ce que les tests observent réellement)

- **Cucumber** : le harness vault n'utilise PAS de fakes in-process pour le
  chemin chaud — vrai `VaultKv2Broker` (reqwest), vrai `HttpUpstream`, vrai
  routeur, contre un faux Vault KV v2 axum et des faux MCP **sur sockets
  réels** qui enregistrent l'`Authorization`. La **sonde d'ordre** : à chaque
  hit du faux Vault, le handler compte les actes déjà dans le gamma du
  contexte (`acts_at_hit == [1]` : l'acte précède la résolution).
- **e2e_vault** (vrai binaire) : provisioning owner sans aucun hit coffre ;
  bearers par serveur sous noms bruts ; arguments agent en forme de headers
  (`Authorization`, `X-Vault-Token`) **sans effet** sur le fil ; write connu
  refusé à zéro hit ; panne coffre (abort du faux Vault) → refus expurgé,
  zéro contact amont, refus journalisé ; redémarrage du coffre + rotation →
  nouvelle valeur au relais suivant, YAML intact (comparaison byte-à-byte) ;
  `audit-export` par contexte intact ; balayage récursif du tempdir entier
  (stores, config, proposals, identité, **stderr du child**) : aucune des
  quatre sentinelles. Gammas exacts : support 4 actes x.github (list,
  forged-args, intention-panne, post-rotation) + refus `mandate_denied` puis
  `credential_unavailable` ; operations 1 acte ; journal 5 xrefs + 2 refus.
- Le token du coffre n'atteint jamais un MCP ; les tokens MCP n'atteignent
  jamais le coffre ; le token du coffre n'entre que par l'ENV du process.

## 4. Environnement et protocole de cette session

Profil **cloud+janitor** (GATEWAY-HANDOFF §5, à la lettre) : `git archive
HEAD` sur la VM → `_transfer/aithos-src-20260715.tgz` (490 Ko ; **scorie
neuve à ignorer**, avec les tars des sessions précédentes) → build/test dans
le conteneur cloud (rustc/cargo 1.95.0, `CARGO_INCREMENTAL=0`, target dédié
`/tmp/aithos-core-gateway-vault-target`, suite ~2 min à froid) → retour
fichier-par-fichier via device_commit_files avec **sha256 croisés sur chacun
des 16 payloads par tranche** (les commits Mac reproduisent l'état exact de
chaque tranche, pas l'état final aplati) → commits janitorisés (mv des
`.git/*.lock` vers `_gitjunk/` avant chaque commande écrivante ; un
`index.lock` du 13/07 traînait à l'arrivée, janitorisé pareil). Warnings
`unable to unlink .git/objects/*/tmp_obj_*` : cosmétiques, confirmés une
énième fois. Un premier essai de tar complet (465 Mo, target inclus par
erreur de pattern) a timeout le pont — le `git archive` des sessions
précédentes reste LA méthode ; le fichier a été écrasé par l'archive propre.

## 5. Limites restantes (assumées, dans l'ordre suggéré)

1. **V4 — LLM** : `LlmConfig.api_key` est toujours la couture inline.
   Réutiliser exactement le même broker (`CredentialRef` + `for_server`-like
   dans `HttpLlmUpstream`), scénario réseau « credential visible uniquement
   chez le faux provider ». **Après validation démo par Mathieu.**
2. **TLS** : `reqwest` est compilé sans backend TLS (workspace
   `default-features = false`). Un Vault/MCP `https://` passe la config mais
   échoue au premier appel (`vault transport failed`). Une feature
   `rustls-tls` = une ligne de Cargo pour l'entreprise réelle — hors démo.
3. **Auth coffre** : `token-env` seul. AppRole/Kubernetes = adapters derrière
   `auth.kind`, non bloquants (décision input : ne pas bloquer le premier
   E2E).
4. **Discovery owner-side sans credential** : `owner-discover-server` parle
   avec l'accès de l'owner. Un amont qui gate `tools/list` se discovery par
   accès owner temporaire (documenté au runbook).
5. **Couture `bearer_token` legacy** : encore parseable (exclusivité
   enforced). La retirer = petite PR après la démo Vault réelle validée.
6. **Puis** : octroi réel des tools `write` (nouveau contrat BDD séparé,
   §7 de l'input), agrégation MCP `resources/*` (après credentials), et le
   réordonnancement des branches — décisions Mathieu.

## 6. Reproduire

```bash
# suite complète (cloud ou local) :
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway
cargo clippy -p aithos-gateway --all-targets -- -D warnings
cargo fmt --check -p aithos-gateway

# e2e coffre seul :
cargo test -p aithos-gateway --test e2e_vault

# démo manuelle avec un VRAI Vault (Docker) :
#   suivre docs/DEMO-GATEWAY-VAULT.md (T1 Vault dev → T2 demo_mcp →
#   T3 gateway → T4 parcours agent, panne, rotation, non-fuite).
```

## 7. Scories et état non committé (inchangés, ne pas toucher)

`_gitjunk/` (locks janitorisés, quelques entrées ajoutées cette session),
`_to_delete/`, `_transfer/` (+ `aithos-src-20260715.tgz` de cette session),
`docs/EXPLORATION-DESKTOP-GATEWAY.md`, `.DS_Store`, et l'input
`docs/HANDOFF-GATEWAY-VAULT-FINALIZATION-2026-07-15.md` — untracked, à
Mathieu de décider s'il se committe.

**Le résultat produit** : l'entreprise donne à l'agent des capacités MCP
gouvernées ; **seul le gateway échange avec le coffre** et présente les
credentials aux serveurs, après autorisation et audit — et un coffre qui
tombe ne laisse jamais sortir un appel.

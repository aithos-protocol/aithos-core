# Reprise de contexte — `aithos-client` dans la Gateway Ethos

Date : 2026-07-25  
Origine : session Codex Desktop `019f950a-4c1c-7cf3-964d-db8b7dec0968`
(« Diagnostiquer connexion Cowork », cwd `/Volumes/Math17/aithos/v2`,
24/07 18:51 → 25/07 09:31), interrompue par une indisponibilité du service.

Documents de référence :

- [`PLAN-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md`](./PLAN-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md)
- [`JOURNAL-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md`](./JOURNAL-INTEGRATION-AITHOS-CLIENT-GATEWAY-ETHOS-2026-07-25.md)
  (à jour jusqu'à 08:42 ; la suite est consignée ici)

## 1. État au moment de l'interruption

- Gates 1 → 9 validées, gate 10 préparée. Le journal fait foi pour les preuves.
- Candidat immuable installé :
  `/Volumes/Math17/aithos-runtime/demo/bin/aithos-gateway-ethos-client-cc596b68`
  (SHA-256 `cc596b68905cf86bb60c6f0c1944e961c2b7b2ae3b64b4039b5d242c28393ed5`).
- Rollback disponible : `aithos-gateway-delegated-write-eec42245`.
- Activation canari : `AITHOS_ETHOS_BACKEND=client-provider`.

### Événements 08:45 → 09:31 (non consignés jusqu'ici)

1. Token AWS expiré ; `.aws-env` régénéré. La Gateway candidate a bien hérité
   des nouveaux credentials ; `aithos-app` a dû être relancée séparément
   (`pnpm dev:demo` ne relit `.aws-env` qu'au démarrage).
2. IndexedDB vidé, Ethos et mandat recréés depuis la dashboard
   (contexte `sales-036a6f74d2813521`, mandat `mandate_2AH8Z6A3F17YBASCX10PCDF1A9`,
   connecteur `github-demo` lié, périmètre de session mixte `act.x.github-demo.*`
   + `write.circle`).
3. Le préflight `Ethos.context` de Cowork emprunte **toujours l'ancien lecteur**
   (chemin legacy) : longues attentes sur des timeouts Provider, aucune boucle
   d'écriture, aucune section créée.
4. Appel direct `Ethos.create` (sans préflight) : le nouveau chemin est bien
   atteint et refuse proprement, avant toute publication :
   `aithos gateway: core bridge failed: aithos-client mutation planning refused: protocol verification failed`.
5. Deux hypothèses testées et **écartées** (scénarios verts) :
   binding connecteur `e/x` présent ; feuille de session mixte GitHub +
   `write.circle`.
6. Diagnostic opt-in ajouté dans
   `crates/aithos-gateway/src/core_bridge.rs` (~l.1975) :
   `AITHOS_ETHOS_DIAGNOSTICS=protocol` affiche la cause Core sur le stderr de la
   Gateway sans modifier le message rendu à Cowork.
   Binaire construit à 09:21 dans
   `.cargo-target-ethos-client-gateway/release/aithos-gateway`
   (SHA-256 `4f59df25718723a238eae98b111deaa104b9f3b7c933ff3940da32cce55a946b`),
   **non installé** dans `aithos-runtime/demo/bin`.

## 2. Cause identifiée par analyse statique

`ClientError::Protocol` (rendu « protocol verification failed ») enveloppe
exclusivement une erreur `aithos_core::Error`. Dans le chemin working set, le
premier point d'appel Core après acceptation du working set est
`bundle.grantee_content_operation(...)`
(`aithos-client/src/publication.rs:2287`, `.map_err(protocol)`).

Asymétrie entre le chemin propriétaire et le chemin délégué :

| Chemin | Appel | Comportement si le dossier n'existe pas |
| --- | --- | --- |
| Propriétaire — `section_add` | `bundle.rs:766` → `ensure_folder(...)` | le dossier est **créé** |
| Délégué — `grantee_section_add` | `grants.rs:1222` → `resolve_folder(...)` | `Error::InvalidPath("no folder <seg>")` |

`resolve_folder` (`grants.rs:178`) ne crée rien : il résout un chemin d'affichage
en SIDs à partir de `e/<zone>/index.json` et échoue si un segment est absent.

L'appel réel utilisait `folder: sales` sur un Ethos **recréé de zéro**, dont la
zone `circle` ne contient encore aucun dossier. L'E2E vert
(`e2e_ethos_client_provider.rs`, `gateway_session_working_set_creates_circle_content_on_the_real_provider`)
crée à la **racine** de `circle` (`folder = ""`, l.687) : le cas « dossier
nommé » n'est donc jamais exercé.

La description de l'outil MCP porte déjà cette contrainte
(`proxy_mcp.rs:2555` : « The folder must already exist »), mais le scénario de
démo demandait explicitement `folder: sales`.

Cohérence avec toutes les observations : refus avant publication, working set
accepté, les deux autres hypothèses vertes, échec déterministe sur Ethos neuf.

**Confiance : élevée, mais non encore confirmée à l'exécution.**

## 3. Confirmation — sans rebuild ni redémarrage

Relancer exactement le même appel Cowork en **omettant `folder`** (création à la
racine de `circle`), sur le même Ethos et le même mandat :

- succès → cause confirmée ;
- même refus → cause à chercher plus loin ; installer alors le binaire de
  diagnostic (§4) pour obtenir la cause Core exacte en un seul essai.

Variante équivalente : créer d'abord une section `circle/sales/...` depuis la
dashboard (chemin propriétaire, qui crée le dossier), puis rejouer la création
déléguée avec `folder: sales`.

## 4. Diagnostic opt-in, si nécessaire

```zsh
install -m 700 \
  /Volumes/Math17/aithos/v2/.cargo-target-ethos-client-gateway/release/aithos-gateway \
  /Volumes/Math17/aithos-runtime/demo/bin/aithos-gateway-ethos-client-diag-4f59df25

gateway_runtime=/Volumes/Math17/aithos-runtime/demo
gateway_vault_token="$(
  jq -er '.auth.client_token | select(type == "string" and length > 0)' \
    "$gateway_runtime/private/gateway-vault-auth-g4.json"
)"

AITHOS_VAULT_TOKEN="$gateway_vault_token" \
AITHOS_ETHOS_BACKEND=client-provider \
AITHOS_ETHOS_DIAGNOSTICS=protocol \
"$gateway_runtime/bin/aithos-gateway-ethos-client-diag-4f59df25" \
  --config "$gateway_runtime/gateway/gateway-public-sdk-demo-869f08b2.yaml" \
  --identity "$gateway_runtime/gateway/agent.id" \
  run
```

Ligne attendue sur le terminal Gateway :
`aithos_gateway_ethos_protocol_diagnostic: mutation planning: <cause Core>`.
Le message rendu à Cowork reste inchangé.

## 5. Ce qu'il reste à faire

1. **Confirmer la cause** (§3), sans rebuild.
2. **Décider** du traitement du dossier en écriture déléguée :
   - a. laisser la contrainte, et créer les dossiers côté propriétaire
     (dashboard) — aucun changement de protocole ;
   - b. autoriser la création de dossier dans le chemin délégué, avec le même
     contrôle `check_delegated_write` sur la chaîne de dossiers résultante —
     changement d'invariant, exige une décision explicite et un E2E dédié.
3. **Couvrir le trou de test** dans
   `e2e_ethos_client_provider.rs` : création déléguée dans un dossier nommé,
   existant et inexistant, plus un dossier imbriqué.
4. **Chemin de lecture** : `Ethos.context/list/read` sont toujours en legacy et
   provoquent les longues attentes du préflight Cowork. À basculer ou à borner
   avant la démo (non couvert par la gate 9).
5. **Gate 9c / gate 10** : rejouer les non-régressions puis le canari Cowork,
   et consigner le résultat dans le journal.
6. **Limite volontaire maintenue** : écriture `public` déléguée toujours refusée
   (exige l'E2E produit prévu par D5).

## 6. Récupération du contexte

Les transcripts Codex sont conservés localement sous
`~/.codex/sessions/AAAA/MM/JJ/rollout-*.jsonl` (un objet JSON par ligne :
`session_meta`, `response_item`, `event_msg`, `compacted`), avec un index de
threads dans `~/.codex/session_index.jsonl`. Un contexte perdu peut donc être
reconstruit intégralement à partir de ces fichiers.

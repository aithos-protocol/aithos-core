# HANDOFF — Piste P / Provider : reprise P2 (intégration protocolaire de publication)

> **ARCHIVE — plan P2 exécuté.** Les étapes suivantes possèdent leurs preuves
> `DONE`; ne pas reprendre cette baseline.

**Date :** 2026-07-19
**Dépôts :** `code/aithos-core` (branche `feat/obligations`) et `provider` (branche `feat/p6-p7-tunnel`)
**Statut :** M2 déployé et validé en production ; CB13 core/bundle vert ; **P2 débloqué, prêt à reprendre.**

> Ce handoff se lit avec `INFRA-PROVIDER.md` (annexe A, normative),
> `NOTE-PROVIDER-CORE-BUNDLE-PROTOCOL-GATE-2026-07-18.md`,
> `HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md` (§13) et
> `HANDOFF-PROVIDER-AWS.md`. Il ouvre la tranche que la NOTE gelait.

---

## 0. En une phrase

Maintenant que CB13 (core + bundle) est vert, le provider reprend
l'intégration protocolaire : **brancher la façade keyless de
`aithos-bundle`, mapper mécaniquement ses verdicts, rendre le commit CAS
atomique sur un backend durable, brancher le témoin, prouver l'E2E à
froid.** Le provider **ne réimplémente aucune règle** core/bundle — il
déplace des octets, sérialise un CAS, transporte. L'autorité vit dans la
façade.

---

## 1. Pré-requis de reprise — les 10 conditions §13, vérifiées

Rejoué moi-même dans le sandbox le 2026-07-19 (arbre à `feat/obligations`
HEAD `522dfcd`) :

1. **CB13 vert** — 216 tests core, 59 tests + **815 scénarios cucumber**
   bundle, **447 tests workspace**, clippy `-D warnings` 0, `fmt` OK, WASM
   OK, **0 `@wip`** core/bundle. ✓
2. Contrats Gherkin + vecteurs `cb2-*` committés (reflog CB1→CB13). ✓
3. Paquet public/opaque stable : `KeylessPublicationPackage`. ✓
4. Façade keyless documentée + testée (`publication.rs`, tests `cb12`). ✓
5. Reason codes publics stables : `aithos_core::Error` / `aithos_bundle`
   variantes fermées. ✓
6. Parent / hauteur / heads / faits CAS explicités : `PublicationCasFacts`. ✓
7. Store vierge vérifie à froid owner + grantee : `cold_verify`
   (tests `cb12`). ✓
8. Aucun objet sensible dans les sorties (keyless, zéro plaintext). ✓
9. Le provider n'a **aucune règle** à réimplémenter (façade unique). ✓
10. **Ownership de la tranche provider attribué** — Mathieu attribue au
    **provider track** la piste « provider publication » le 2026-07-19.
    Les 2 features réservées ci-dessous appartiennent donc au provider ;
    elles ne sont créées **que** ici. ✓

---

## 2. État déployé (M2, prod) — ne pas casser

- **Store `store.aithos.fr`** (ECS td:3, image `:prod` musl static) : wire
  A.2/A.8, `/acme/txt` (B.5) live, backend DNS Route53, purge 10 min.
- **Relais `relay.aithos.fr`** (ECS td:2) : passthrough SNI aveugle, porte
  tunnel TLS+ALPN, cert **ACM exportable** (clé jamais dans Terraform).
- **Fail-closed intact** (le squelette P1 refuse tout ce qu'il ne sait pas
  encore vérifier — c'est la barrière que P2 lève, gate par gate) :
  - `envelope.rs` #9 : une requête **mandatée** → `chain_invalid` ;
  - `service.rs` : `manifest.json`/`certs`/`gamma`/`heads`/`batch`/`sync`
    → `501 not_implemented`.
- Terraform : `envs/prod` (aws seul), plan lu, apply humain. `desired_count`
  relais = 1 (NE PAS bumper — registre yamux en mémoire par tâche).

---

## 3. Doctrine (rappel opposable)

Le provider déplace des octets et vérifie des **preuves publiques déjà
typées** ; il ne détient jamais de secret client, ne voit jamais de
plaintext `circle`/`self`/vault, ne décide jamais. `covers()` serveur =
**anti-abus, jamais l'autorité** (§3.1). Fail-closed partout. Logs
expurgés A.8 (registre fermé). Terraform plan-first, apply humain. Pas de
merge `main` sans gate. `aithos-core`/`aithos-bundle` restent purs (zéro
I/O) ; le provider les consomme.

---

## 4. Les points de branchement exacts (anchors)

### 4.1 Façade keyless — `aithos-bundle/src/publication.rs`

```rust
pub struct KeylessPublicationPackage { … }
impl KeylessPublicationPackage {
    pub fn verify_public_only(&self) -> Result<VerifiedK1cCarriers>;
    pub fn verify_for_cas(&self)    -> Result<VerifiedPublication>;   // ← l'entrée provider
}
pub struct VerifiedPublication { pub carriers: VerifiedK1cCarriers, pub cas: PublicationCasFacts }
pub struct PublicationCasFacts {
    pub subject: String, pub manifest_profile: String, pub mode: PublicationMode,
    pub new_height: u64, pub expected_predecessors: Vec<String>,
    pub resolution_winner: Option<String>,
    pub source_gamma_head: String, pub new_manifest_head: String, pub new_gamma_head: String,
    pub roots: BTreeMap<String,String>, pub gamma_roots: …, pub gamma_counts_root: String,
    pub reachable_objects: Vec<String>, pub package_digest: String,
}
pub fn import_keyless<S: Store>(store: &mut S, package: &KeylessPublicationPackage) -> Result<()>;
pub fn cold_verify<S: Store>(…) -> …;          // store vierge → vérif à froid
pub fn cold_verify_for_cas<S: Store>(…) -> …;
pub fn export_keyless(…) -> …;
pub fn package_with_objects(…) -> …;
```

**Le provider appelle `verify_for_cas()` une seule fois, puis mappe
mécaniquement** : succès → persister `reachable_objects` (opaque) +
comparer/avancer atomiquement les têtes (`expected_predecessors` →
`new_manifest_head`/`new_gamma_head`, `new_height`) ; rejet → une variante
d'erreur fermée → un code A.7. **Il ne dérive aucun verdict sémantique de
ces champs.**

### 4.2 Verdict d'opération core — `aithos-core/src/operation.rs`

Pour l'**autorisation de requête** (A.2 #7–#10, le `#9` mandaté que P1
défère) :

```rust
pub fn verify_operation_facts(input: OperationFactsInput<'_>) -> Result<VerifiedOperationFacts>;
pub fn verify_operation_projection(…) -> …;
```

Complété par `mandate`, `gamma_replay` (rejeu sémantique CB6),
`revocation`, `constraints`, `catalog`, `carriers`. **Le store appelle ces
fonctions ; il ne recopie aucune de leurs branches.**

### 4.3 Points de défer P1 à remplacer (dépôt code)

- `crates/aithos-provider/src/envelope.rs` — l'arm
  `if !owner_fragment { return Err(Refusal::ChainInvalid) }` (#9) →
  résolution feuille (#7), signature (#8), `verify_chain` core (#9),
  `covers()` anti-abus (#10). Les cas P1-deferred de `p1-store-envelope`
  (`accept_get_mandated`, `reject_window_expired`, `reject_not_covered`,
  `reject_chain_revoked`, `reject_key_leaf_mismatch`) deviennent verts,
  **byte-exact**.
- `crates/aithos-provider/src/service.rs` — `_ => NotImplemented`
  (Heads/Batch/Gamma/Sync/List) et `store_object` (`Manifest|Cert|
  GammaSegment → NotImplemented`) → A.3/A.4/A.5 via la façade + le seam CAS.
- `crates/aithos-provider/src/objects.rs` — seam `ObjectStore` (mémoire) →
  ajouter un backend S3 ; **ajouter un seam CAS** (têtes DynamoDB) pour la
  transaction A.5. Le CAS provider ≠ la transaction bundle locale (G-B).

---

## 5. Le rituel (non négociable — `.claude/skills/rituel-tests`)

1. **Gherkin AVANT le code**, committé seul : dépôt code
   `crates/aithos-provider/tests/features/store/` + e2e `provider/e2e`
   (behave, sans clé). Un scénario par cas du contrat, refus nommant son
   code A.7.
2. **Vecteurs indépendants AVANT le code** (générateur Python séparé),
   **construits sur les paquets keyless exportés par `aithos-bundle`** —
   jamais de crypto réinventée côté provider. Observés rouges d'abord.
3. Unités + BDD contre la vraie surface axum ; **rejeu byte-exact contre
   le vrai binaire** (process enfant, vraie socket).
4. Terraform `fmt`/`validate` + **plan lu, apply humain**.
5. E2e contre la plateforme déployée après apply.
6. **Gate = STOP.** État express horodaté en tête de `HANDOFF-PROVIDER-AWS`,
   preuves listées, dérives par gate humain. Pas de merge sans gate.

---

## 6. Ordre de reprise (plan CB §13 + INFRA-PROVIDER annexe A)

1. **Features** `store-publication.feature` + `store-cold-roundtrip.feature`
   (`@wip`), attribuées au provider (cond. 10). Redlines A.2–A.5 minimales
   si écart constaté (par gate, jamais silencieux).
2. **Vecteurs p7+** (publication CAS, cold roundtrip) sur les paquets
   keyless — rouges d'abord.
3. **Autorisation mandatée** : brancher `verify_chain`/`verify_operation_facts`
   core au `#9` (résolution feuille, signature, chaîne, `covers()`
   anti-abus). Rejeu **byte-exact** contre `p1` (les 5 cas P1-deferred
   passent verts).
4. **A.4/A.5** : PUT `manifest.json`/`gamma`/`certs` + **CAS des deux têtes**
   (manifest `chain_hash`, gamma head) en **transaction atomique opaque**.
   Publish/edition → `verify_for_cas()` ; entrée `/gamma` → vérif d'entrée
   core. Le store **n'arbitre jamais un fork** (le CAS sérialise, le
   témoin observe).
5. **Heads / batch / sync** (A.3).
6. **Backend durable** : S3 (objets opaques content-addressed) + DynamoDB
   (têtes CAS) derrière les seams. `MemObjects` → S3 ; nouveau seam CAS.
   *(Décision Lambda-vs-Fargate du store à trancher ICI, gate P2 — cf.
   INFRA-PROVIDER §7 note gravée. Le relais reste Fargate quoi qu'il
   arrive.)*
7. **Témoin** sur le head canonique (annexe C ; `witness.rs` déjà écrit,
   non composé). Clé KMS Ed25519 sign-only.
8. **Vrai E2E** : bundle grantee → HTTP provider → arrêt/restart →
   téléchargement dans un store vierge → **cold verify** → lectures
   owner/grantee. Aucun mock du protocole.

**Chaque étape = son gate STOP.**

---

## 7. Où / normatif

- **Code** : `code/aithos-core` (`feat/obligations`), crate
  `rust/crates/aithos-provider` (committé `7349cf6`). Vecteurs `vectors/`
  (`gen-p.py`/`verify-p.py`, `p1..p6`). Bundle façade
  `rust/crates/aithos-bundle/src/publication.rs`.
- **Provider infra** : `provider` (`feat/p6-p7-tunnel`, committé `65293f4`)
  — `infra/terraform`, `e2e` (behave), CI plan-only.
- **Normatif** : INFRA-PROVIDER **annexe A** — A.2 (enveloppe, ordre 0–10),
  A.3 (routes + path-map), A.4 (vérif d'artefacts au dépôt), A.5 (CAS deux
  têtes), A.7 (registre d'erreurs), A.8 (limites + logs). JCS RFC 8785,
  Ed25519 sur JCS-`value=""`, multibase `z6Mk…`, BLAKE3, RFC 3339 Zulu.
- ⚠️ **Sandbox** : `cargo` absent de la VM device — stager le crate vers le
  sandbox cloud pour compiler/tester, réécrire in situ par `cp` (le mount
  bloque `unlink`). `cargo-zigbuild` + `zig` pour le musl statique ; crane
  pour les images ; l'egress du sandbox **intercepte le TLS brut** (toute
  sonde TLS/ALPN réelle = vraie machine).

---

## 8. Interdits (opposables à chaque gate)

- **Réimplémenter** `verify_chain`/`covers`/contraintes/gamma/changeset/
  révocation dans le provider : **DÉLÉGUER** à core/bundle.
- Toucher `aithos-client`, `aithos-gateway`, CLI/WASM (pistes distinctes).
- **Bumper `desired_count`** du relais (registre en mémoire par tâche →
  HA = tranche ultérieure : registre partagé + saut relais-à-relais).
- `apply` sans plan lu + parole explicite de Mathieu ; merge `main` sans
  gate.
- Modifier un vecteur gelé (`p1..p6`, `cb2-*`, `a1..i1`) : nouveau id +
  redline d'annexe par gate.
- Créer les 2 features hors du provider track (ownership attribué).
- Brancher le témoin sur un feed de publication tant que le head canonique
  n'est pas figé par ce chantier.

---

## 9. Repères de départ

M2 prod live (store td:3, relais td:2) ; CB13 vert ; le squelette store P1
refuse fail-closed tout ce qui dépasse P1. Le premier gate P2 est **les 2
features + leurs vecteurs rouges**, avant la première ligne de code.

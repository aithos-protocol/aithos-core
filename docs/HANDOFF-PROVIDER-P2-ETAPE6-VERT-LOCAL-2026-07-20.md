# HANDOFF — Piste P / Provider P2 : étape 6 (backend durable S3+DynamoDB, cache A.6, write-once ⑧b) — VERT LOCAL, STOP au gate déployé

Date : 2026-07-20. Dépôt : code/aithos-core (+ provider/ pour le Terraform). État DISQUE = vérité.
Statut : code étape 6 implémenté et vert en sandbox — **STOP au gate déployé** (creds, build image, apply, preuve AWS réelle : rien de tout ça n'a été touché). Le commit reste le geste de Mathieu.
Se lit avec HANDOFF-PROVIDER-P2-GATE5-DONE-2026-07-20.md, DECISION-COMPUTE-STORE-PROPOSITION-GATE6-2026-07-20.md (ACTÉE) et INFRA-PROVIDER.md (2 notes gravées le même jour).

## 0. Séquence de la session (tout GO'é par Mathieu, dans l'ordre)

1. **Sandbox reconstruit depuis le disque** (tarball 08:03 + overlay par mtime + mini-tarball features fourni par Mathieu — le refus de staging `.feature` persiste, la VM device est toujours morte) ; batterie gate 4+5 rejouée : p7 15/15, p9 33/33, cucumber 119/119, 53 binaires core+bundle, clippy 0, fmt clean. **L'état disque des gates 4+5 est corroboré.**
2. **Deux décisions du gate P2 actées et GRAVÉES** (INFRA-PROVIDER §7 et §8, notes du 2026-07-20) :
   - **① Compute du store : Fargate.** L'argument qui tranche seul : A.8 grave le PUT direct ≤ 32 MiB, et aucune entrée Lambda n'accepte > 6 Mo en requête (le streaming 07/2025 ne couvre que la réponse ; vérifié en ligne). Le wire prime.
   - **② Tenant de rejeu `acme` : retrait de l'image prod + tenant jetable** (`replay-<date>` via la CLI P7, purgé après preuve). Gravé §8 ; **opposable dans le binaire** (voir ci-dessous).
3. **Gate contrat étape 6, rituel tenu** : 12 scénarios BDD écrits AVANT le code (cache A.6, write-once ⑧b, fail-closed des seams) — **RED observé 9/12** contre le binaire gate 5 (les 3 verts = les 2 re-dépôts idempotents que le squelette acceptait déjà en écrasant + un no-store déjà correct ; le contrat les fige).
4. **Implémentation** : 12/12 verts, toute la batterie verte, vecteurs p1..p9 byte-identiques (aucun octet de vecteur touché).

## 1. Livré (code — crates/aithos-provider)

- **`objects.rs` (réécrit)** : le seam parle `Result` — un backend qui ne répond pas → `Err(StoreUnavailable)` → **503 `unavailable`** (le précédent nonces), jamais une absence inventée. Nouvelle méthode **`put_once`** (⑧b) : `Stored | Identical | Conflict`. Backend **`S3Objects`** : layout `t/<tenant>/<did>/<chemin>`, `list` → ListObjectsV2 paginé (trié, promesse du seam indépendante du backend), `put_once` → PUT conditionnel `If-None-Match: *` + relecture/comparaison sur 412 (write-once multi-instance sans jamais prétendre au CAS A.5). Alias `StoreFuture<'a, T>`.
- **`heads.rs` (réécrit)** : même passage à `Result` (`HeadsUnavailable`). Backend **`DynamoDbHeads`** : table `(t, d)`, attributs `h/m/g/gs/months` (months = liste des mois appendés, détail d'implémentation A.5 jamais sur le wire), CAS = un PutItem conditionnel (expression attribute names partout — zéro mot réservé), **lectures fortement cohérentes** (une lecture stale fabriquerait des conflits CAS fantômes), le perdant relit la vérité courante. Alias `HeadsFuture<'a, T>`.
- **`artifacts.rs`** : tous les appels seams propagent `Unavailable` (fail-closed). `deposit_cert` et `deposit_sidecar` passent par **`put_once`** → `ArtifactReason::ImmutableConflict` (reason `immutable_conflict`, **micro-redline A.7 portée au gate** — même patron que `prefix_mismatch` au gate 5).
- **`envelope.rs`** : résolution de clé (#7), certs de chaîne (#9) et scan de révocation propagent `Unavailable` — un backend muet ne « continue » jamais comme si le segment était absent.
- **`service.rs`** : **classes de cache A.6 par chemin** (`cache_class()` — la classe appartient au CHEMIN, calculée à l'instant de service, jamais au backend) : immutable (certs, manifests/<h>, changesets, evidence, segments gamma des mois révolus), no-store (manifest.json, /heads, segment courant, hdr, index, indices/roots/vault), public must-revalidate + **ETag fort** (public/sections alias), private must-revalidate + ETag fort (blobs, alias circle). **Complément A.6 (à graver au gate)** : did.json + e/public/** → public must-revalidate + ETag ; x/** → private must-revalidate + ETag. Toutes les surfaces d'erreur/collection restent no-store.
- **`bin/store_api.rs`** : env `AITHOS_STORE_OBJECTS_BACKEND` (memory défaut | s3 + BUCKET) et `AITHOS_STORE_HEADS_BACKEND` (memory défaut | dynamodb + TABLE) — une ancienne task def boote le nouveau binaire tel quel. **Garde décision ②** : backend durable + preloads/seeds dans le bootstrap → **refus de boot** (fail-closed ; le matériel de rejeu ne persiste jamais).
- **Cargo** : + `aws-sdk-s3` (workspace + crate) ; Cargo.lock mis à jour.

## 2. Livré (contrat + Terraform)

- `store-reads.feature` : +7 scénarios (@cache @gate6 ×5, @a6-completion ×1, @fail-closed ×1). `store-publication.feature` : +5 (@write-once ×4, @fail-closed ×1). Harnais : steps ETag/cache/squat + **wrappers Flaky** (injection de panne des seams — la preuve BDD du fail-closed 503).
- **Leçon d'implémentation documentée dans le contrat** : l'adressage par contenu des sidecars étant calculé sur les octets déposés, le conflit ⑧b « honnête » y est inatteignable (autre corps ⇒ autre digest ⇒ `id_mismatch`) et un cert re-sérialisé meurt en `form` (JCS canonique) — le bras ⑧b atteignable = un objet différent DÉJÀ stocké sous le nom immuable (scénarios « squat »).
- **Terraform `provider/infra/terraform/modules/store-api`** (fmt + validate verts, harnais local jetable non livré) : bucket **`<prefix>-store-data`** (versionné A5, SSE, public access block, ownership enforced), table **`<prefix>-heads`** (t/d), policy task role **moindre privilège** (Get/PutObject + ListBucket sur LE bucket ; Get/PutItem sur LA table ; **aucun Delete** — le GC §8 sera un runbook audité à part), env durable **derrière `durable_backends = false` par défaut** : le flip au gate déployé exige un bootstrap sans preloads (décision ② — sinon le binaire refuse de booter). `desired_count` reste 1 ; passer à 2 AU GATE seulement.

## 3. Preuves (sandbox, 2026-07-20)

| Preuve | Résultat |
|---|---|
| RED avant code (rituel) | 9/12 RED contre le binaire gate 5 |
| cucumber après code | **131/131 scénarios (843 steps)** — 119 gate 5 + 12 étape 6 |
| red-replay-p7.py / p9.py (vrai binaire étape 6) | **15/15** / **33/33 GREEN** (non-régression gates 4+5) |
| vectors_replay | 5/5 (p1 byte-exact, p2, p7, p9 wire-exact) |
| pod-stub complet | relay 18/18, tunnel 12/12, 46 unités (+1 : put_once) |
| aithos-core + aithos-bundle --locked | 53 binaires verts |
| clippy --all-targets -D warnings / fmt --check | 0 / clean |
| gen/verify p1..p9 | verts, **p9 byte-identique au disque** (sha256) |
| terraform fmt + validate (module étendu) | clean / Success |

## 4. Arbitrages portés au gate étape 6 (rien gravé unilatéralement)

1. **`immutable_conflict`** = reason nouveau d'`artifact_invalid` (⑧b) — micro-redline A.7 à graver (le registre passerait à 10).
2. **Complément A.6** : classes de did.json (`public, max-age=0, must-revalidate` + ETag), e/public/** (idem) et x/** (private) — non nommées par l'annexe, implémentées par cohérence, à graver.
3. **503 `unavailable` étendu aux seams objets/têtes** (déjà le comportement nonces, P1 arbitrage n°3 « opérationnel, pas wire ») — à documenter A.7 ou assumer opérationnel.
4. **Ordre des effets du dépôt** (implémenté, à confirmer) : vérifier → CAS (DynamoDB, point de sérialisation — un perdant n'écrit RIEN) → écrire S3. Un crash entre CAS et écriture S3 laisse tête > objet : le prochain lecteur/appendeur voit l'incohérence (CAS mismatch côté client), le serveur ne répare JAMAIS (doctrine) — réparation = runbook ops. La « transaction avec le dépôt S3 » d'A.5 se lit comme cette discipline, pas comme une transaction cross-service (qui n'existe pas).
5. **Pas de vecteur p10** : les classes de cache et le ⑧b sont prouvés par BDD in-process + le seront par l'e2e du gate déployé contre la prod ; les octets wire des gates 4+5 restent figés par p7/p9.

## 5. Reste pour clore (Mathieu)

1. **Passe locale** (sandbox = vert, ta machine = autorité) :
```bash
cd code/aithos-core/rust
cargo test -p aithos-provider --features pod-stub --locked
cargo build -p aithos-provider --bin aithos-store-api
cd ../vectors
python3 red-replay-p7.py ../rust/target/debug/aithos-store-api   # 15/15
python3 red-replay-p9.py ../rust/target/debug/aithos-store-api   # 33/33
```
2. **Commit** (couvre gates 4+5 si pas déjà faits + étape 6 ; la session n'a touché QUE ces chemins) :
```bash
cd code/aithos-core
git add rust/crates/aithos-provider rust/Cargo.toml rust/Cargo.lock docs
git commit -m "P2 étape 6 (vert local): backends S3+DynamoDB derrière les seams, cache A.6, write-once ⑧b, fail-closed 503 — cucumber 131/131, p7 15/15, p9 33/33"
cd ../../provider
git add infra/terraform/modules/store-api
git commit -m "P2 étape 6: bucket store-data versionné + table heads + task role moindre privilège + durable_backends (flip au gate déployé)"
```
3. **Gate déployé étape 6** (session dédiée, creds + délégation apply) : ① `terraform plan` lu intégralement (attendu : +bucket +table +policy + révision task def SANS env durable) ; ② build/push image étape 6 ; ③ apply ; ④ bootstrap **sans preloads** + `durable_backends = true` + re-plan/apply ; ⑤ tenant `replay-<date>` (CLI P7), rejeu p7/p9 contre la prod, **e2e cache A.6** (en-têtes réels), purge du tenant ; ⑥ `desired_count = 2` + preuve HA ; ⑦ graver les arbitrages §4.

## 6. Environnement (delta session)

VM device toujours morte ; `.feature` toujours refusé en staging (mini-tarball fourni par Mathieu = le contournement qui marche) ; `device_commit_files` écrit tout le reste sans problème. Tarball disque périmé compensé par overlay mtime — demander un tarball frais en début de prochaine session reste le plus simple.

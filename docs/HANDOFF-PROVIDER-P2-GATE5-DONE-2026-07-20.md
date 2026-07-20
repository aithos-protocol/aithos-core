# HANDOFF — Piste P / Provider P2 : gate 5 (A.3 surface de lecture + redline A.1 draft.2 + écritures restantes) LIVRÉ

Date : 2026-07-20. Dépôt : `code/aithos-core` (état DISQUE = vérité, même
règle qu'au gate 4). Statut : **étape 5 implémentée et verte — STOP au
gate** : preuves listées, arbitrages actés en session (GO Mathieu sur le
cadrage, la redline ④ et les six arbitrages du gate contrat 5), commit +
passe locale de Mathieu = clôture formelle.

Se lit avec `HANDOFF-PROVIDER-P2-GATE4-DONE-2026-07-20.md`,
`REDLINE-A1-DRAFT2-PROPOSITION-GATE5-2026-07-20.md` (ACTÉE, gravée) et
`INFRA-PROVIDER.md` annexe A (normatif, redlines gate 5 gravées le même
jour).

## 0. Séquence de la session (tout GO'é par Mathieu, dans l'ordre)

1. **Gate 4 clos côté sandbox** : features disque validées par preuve
   (rejeu complet sur l'état disque), 16 `@wip` levés dans
   `store-publication.feature` (écrit-back), `cucumber.rs` re-fmt
   (écrit-back). Reste ta passe locale + commit gate 4 si pas déjà faits.
2. **Redline ④ ACTÉE et GRAVÉE** dans `INFRA-PROVIDER.md` (A.1 chemins
   draft.2 servables ; A.3 lignes de couverture + note `manifests/<h>`
   sans ligne d'écriture ; A.4 sidecars adressés par contenu + alias
   light-form ; A.6 classes de cache + CloudFront). Les 4 défauts
   confirmés tels quels ; la symétrie `e/public/**` canonique en écriture
   N'EST PAS dans la redline (alias seulement).
3. **Gate contrat 5** : `store-reads.feature` (19 scénarios) +
   10 scénarios did.json/réplique dans `store-publication.feature` +
   vecteur `p9-store-reads.json` (31 cas / 33 pas wire, gen/verify/driver)
   — observé **30/33 RED** contre le binaire gate 4 (les 3 GREEN = refus
   fail-closed que le squelette avait déjà ; le contrat les fige).
   Six arbitrages portés au gate, **GO Mathieu**.
4. **Implémentation** : 33/33 GREEN + toute la batterie (ci-dessous).

## 1. Livré (code)

- `src/pathmap.rs` : 8 variantes `ObjectPath` nouvelles (redline gate 5 :
  `ManifestSlot`, `Changeset`, `Evidence`, `PublicSectionAlias`,
  `CircleBlobAlias`, `IndicesPublic`, `RootsPublic`, `VaultCatalogPins`) ;
  grammaire fermée (stems bundle-internes `tree-`/`index-`/`-alt`/
  `gateway/**`/`gamma/gamma.jsonl` → `path_invalid`) ; lignes de
  couverture A.3 de la redline (anonyme += alias publics ; toute chaîne
  valide += manifests/changesets/evidence/vault ; `read.circle` +=
  alias blob ; écriture zone += alias circle + `public/sections/**` ;
  owner/authorized_by += sidecars et dérivés ; `ManifestSlot` = AUCUNE
  ligne d'écriture) ; `parse_list_query` (grammaire `?list=` fermée) ;
  test de composition : chaque chemin redline accepté par le wire DOIT
  passer `aithos_bundle::validate_store_key` (subset prouvé, jamais une
  recopie).
- `src/service.rs` : dispatch complet — `GET /heads` (tuple A.5, null où
  vide), `GET ?list=` (filtrage grossier par `covers()`, pagination
  after/limit, `413` au-delà de 1000), `POST /batch` (multipart/mixed,
  parts en ordre de requête, `Content-Location` + `X-Aithos-Status`
  200|403|404, corps sur 200 seul, ≤256 chemins, ≤32 MiB), `POST /sync`
  (règle gelée : `manifest.json` en 1re part + diff lexicographique des
  files maps épinglées tenue→courante, filtré par couverture, `410` si le
  slot tenu manque), PUT `did.json`/réplique/sidecars branchés, PUT
  `manifests/**` → `not_covered` (owner compris). Le `501
  not_implemented` a DISPARU du chemin de données (plus aucun arm).
- `src/artifacts.rs` : `deposit_did` (genèse : `id == did` + `verify()`
  core sous la clé du document DÉPOSÉ ; remplacement : signature
  `#succession` vérifiée sous la clé succession STOCKÉE — lecture intérim
  actée), `deposit_replica` (préfixe octet-exact → `prefix_mismatch`,
  chaque entrée ajoutée vérifiée comme un append via le helper partagé
  `verify_gamma_entry_text`, mois de l'entrée == segment, CAS tête de
  segment, idempotent si zéro entrée ajoutée), `deposit_sidecar`
  (JSON parsable + digest K1-C `C(domain,octets)` recalculé == nom →
  sinon `id_mismatch`). Reason nouveau UNIQUE : `prefix_mismatch` (A.7).
- `src/envelope.rs` : exception genèse A.2 #7 — `#root` sur PUT
  `did.json` d'un DID lié SANS document stocké se résout contre le
  document déposé (jamais une porte ouverte : l'enrôlement précède).
- `src/control.rs` : `did_json` optionnel au bootstrap (bind-only =
  état pré-genèse).
- `src/objects.rs` : `list()` sur le seam (lecture pure, tri
  lexicographique ; S3 → ListObjectsV2 à l'étape 6 ; toujours AUCUNE
  écriture conditionnelle sur ce seam).
- `Cargo.toml` provider : + `sha2` (workspace) ; `Cargo.lock` mis à jour.

## 2. Livré (contrat + preuves)

- `tests/features/store/store-reads.feature` : 19 scénarios, tous levés.
- `store-publication.feature` : +10 scénarios did.json/réplique, levés.
  Restent `@wip` : `@cas @publish @delegated` (incompatibilité
  vecteur/texte héritée du gate 4, décision pendante), `@witness @gate7`,
  tout `store-cold-roundtrip` (gate 8).
- `vectors/gen-p9.py` / `verify-p9.py` (50 checks indépendants) /
  `red-replay-p9.py` (driver per-case, pattern ⑦) /
  `p9-store-reads.json`. p1..p8 INTOUCHÉS (sha256 re-vérifiés).
- `tests/vectors_replay.rs` : + `p9_cases_replay_wire_exact` (multipart
  et corps byte-exacts contre le vrai binaire).
- `tests/cucumber.rs` : steps étape 5 (heads/list/batch/sync/redline/
  did/réplique), parseur multipart byte-exact, états p8_cold seedés des
  octets gelés.

## 3. Preuves (sandbox, 2026-07-20)

| Preuve | Résultat |
|---|---|
| `red-replay-p9.py` (avant code) | 30/33 RED (3 GREEN = fail-closed déjà correct) |
| `red-replay-p9.py` (après) | **33/33 GREEN** |
| `red-replay-p7.py` | 15/15 GREEN (non-régression gate 4) |
| `cargo test --test vectors_replay` | 5/5 (p1 byte-exact, p2, p7, p9 wire-exact, deployed skip) |
| `cargo test --test cucumber` | **119/119 scénarios (727 steps)** |
| `cargo test --features pod-stub` | 11 binaires ok (relay 18/18, tunnel 12/12 inclus) |
| `cargo test -p aithos-core -p aithos-bundle --locked` | 53 binaires verts |
| clippy `-D warnings` / `cargo fmt --check` | 0 / clean |
| `gen-p.py && verify-p.py` | p1..p6 byte-identiques |
| `gen-p7/p8` + `verify-p7` | byte-identiques (sha256 pinnés re-vérifiés) |
| `gen-p9 && verify-p9` | 31 cas, 50 checks indépendants verts |

## 4. Arbitrages du gate 5 (actés en session, GO Mathieu)

1. Remplacement `did.json` sous la clé `succession` stockée (même `id`) —
   lecture INTÉRIM d'A.4 ; l'artefact d'époque §10.4 (`next_did` =
   nouvelle identité) reste OUVERT, à trancher au chantier identité.
2. Corps `/batch`/`/sync` malformé → `envelope_invalid` (forme de requête
   fermée ; registre A.7 inchangé).
3. Règle du pack `/sync` = `manifest.json` + diff des files maps
   épinglées ; slot tenu purgé → `410`. `have_edition` 0 ou > courant →
   `410` (fail-closed).
4. `prefix_mismatch` = le SEUL reason `artifact_invalid` nouveau —
   micro-redline A.7 **GRAVÉE** (2026-07-20, sur le « Ok top » de
   Mathieu) : le registre fermé des neuf reasons est maintenant dans
   l'annexe A.7 elle-même.
5. Listing : toute chaîne valide liste ; filtrage grossier par zone ;
   `limit` > 1000 → `413` jamais clampé ; le listing NOMME le cert
   d'enrôlement (objet stocké comme un autre).
6. Cache A.6 : PAS affirmé par p9 (étape 6, avec le vrai backend).

Constatés en cours d'implémentation (documentés, pas des décisions) :
le driver p9 a évolué deux fois avant le vert (parseur multipart
byte-exact ; cert d'enrôlement dans le listing attendu) — évolutions de
HARNAIS/générateur, aucun octet p1..p8 touché ; réplique idempotente
(zéro entrée ajoutée) → accept avec la tête courante, rien d'écrit.

## 5. Reste pour clore (Mathieu) — le commit est TON geste (VM morte)

1. Passe locale : la batterie du §3 (au minimum
   `cargo test -p aithos-provider --features pod-stub` +
   `red-replay-p7/p9` contre ton binaire) :
   ```sh
   cd code/aithos-core/rust
   cargo test -p aithos-provider --features pod-stub --locked
   cargo build -p aithos-provider --bin aithos-store-api
   cd ../vectors
   python3 red-replay-p7.py ../rust/target/debug/aithos-store-api   # 15/15
   python3 red-replay-p9.py ../rust/target/debug/aithos-store-api   # 33/33
   ```
2. Commit (couvre gate 4 — features/fmt — ET gate 5 ; la session n'a
   touché QUE ces chemins) :
   ```sh
   cd code/aithos-core
   git add rust/crates/aithos-provider rust/Cargo.lock vectors docs
   git commit -m "P2 gates 4+5: features gate 4 validées + surface de lecture A.3, redline A.1 draft.2, did.json/replica/sidecars — p7 15/15, p9 33/33, cucumber 119/119"
   ```
3. Prochaine étape (6) : backend durable S3 + DynamoDB derrière les
   seams (`ObjectStore.list` → ListObjectsV2 ; CAS → écriture
   conditionnelle ; write-once ⑧b ; classes de cache A.6 ; décision
   Lambda-vs-Fargate à trancher AVANT le code, INFRA-PROVIDER §7).

## 6. Environnement (delta session)

VM Cowork device toujours morte ; `.feature` toujours refusé en staging
MAIS `device_commit_files` les écrit très bien (write-back utilisé toute
la session). Le tarball disque complet
(`_transfer/aithos-core-disk-20260720.tgz`) est le bon véhicule
disque→sandbox : demander le refresh à Mathieu en début de session.

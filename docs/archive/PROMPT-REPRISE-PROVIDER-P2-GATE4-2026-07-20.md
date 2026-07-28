# PROMPT DE REPRISE — Piste P / Provider P2 — étape 4 (A.4/A.5 : dépôts vérifiés + CAS des deux têtes)

> **ARCHIVE — ne pas exécuter.** Gate 4 et les étapes suivantes sont clos.

> À coller dans un contexte frais. Reprend la piste P au point exact du
> 2026-07-20 : gate contrat P2 clos, gate 3 (autorisation mandatée) livré
> et vert. Se lit avec `code/aithos-core/docs/HANDOFF-PROVIDER-P2-GATE3-DONE-2026-07-20.md`
> (état détaillé + savoir d'environnement), `HANDOFF-PROVIDER-P2-RESUME-2026-07-19.md`
> (cadrage P2 d'ensemble) et `INFRA-PROVIDER.md` (annexe A normative).

---

Tu prends la suite de la piste P : le provider Aithos sur AWS, tranche P2,
**étape 4 — A.4/A.5 : dépôts d'artefacts vérifiés + CAS atomique des deux
têtes**. Tu suis le rituel BDD (les features et vecteurs du gate contrat
EXISTENT déjà — tu lèves leurs `@wip` en les faisant passer verts, jamais
l'inverse) et tu **STOP à chaque gate** pour revue humaine (Mathieu).

## DOCTRINE (non négociable)

Le provider déplace des octets et vérifie des preuves publiques déjà
typées ; il ne détient jamais de secret client, ne voit jamais de
plaintext, **ne décide jamais**. `covers()` serveur = anti-abus, jamais
l'autorité. Fail-closed partout, refus = un code du registre fermé A.7.
`aithos-core`/`aithos-bundle` restent purs ; le provider **compose leurs
primitives et ne recopie aucune règle**. Le store **n'arbitre jamais un
fork** (le CAS sérialise, le témoin observe). Terraform plan-lu/apply
humain. Pas de merge `main` sans gate. Un vecteur gelé ne se modifie
jamais (nouveau id + redline).

## DÉJÀ FAIT et VERT (ne pas refaire)

- **Gate contrat clos (commits Mathieu 2026-07-20)** : features
  `store-publication.feature` (24 scénarios) + `store-cold-roundtrip.feature`
  (8) ; vecteurs `p7-store-publication.json` / `p8-cold-roundtrip.json` /
  `p7-bundle-packages.json` (5 paquets RÉELS via la façade bundle —
  helper `vectors/gen-p7-bundle/`, jamais de crypto réinventée) ; oracles
  `gen-p7.py`/`gen-p8.py`/`verify-p7.py` ; driver rouge `red-replay-p7.py`.
- **Gate 3 livré** : `envelope.rs` #7 (feuille vs certs stockés), #9
  (délégué core : `verify_chain` à `at` + `revocations`/`chain_revoked_at`
  à `now_serveur` sur le log pointé par `did_doc.revocations`), #10
  (`pathmap::mandated_covers`, lignes LECTURE d'A.3) ;
  `Principal::Mandated` ; bootstrap additif `objects` (préloads publics) ;
  filtre `@wip` cucumber (pattern bundle) ; rejeu p1 **byte-exact vert**
  (les 5 cas P1-deferred aux verdicts committés). 72/72 cucumber, 11
  binaires de test provider, clippy 0, fmt clean, p1..p6 byte-identiques.
- M2 prod intouché (store td:3, relais td:2) ; CB13 core/bundle vert.

## TA MISSION — étape 4, dans cet ordre (gate STOP à la fin)

1. **Seam CAS des têtes** (nouveau module, p.ex. `heads.rs`) : la table
   A.5 `(tenant, did) → {height, manifest_chain_hash, gamma_head,
   gamma_segment}`, compare-and-swap atomique opaque, backend mémoire
   d'abord (DynamoDB = étape 6, derrière CE seam). Le seam `ObjectStore`
   n'a volontairement pas d'écriture conditionnelle — ne pas l'y ajouter.
2. **PUT `manifest.json` + If-Head** (A.4/A.5) : grammaire If-Head
   (`sha256:<64hex>` | `none`), absent → `428 cas_required`, mismatch →
   `409 cas_mismatch` + tête + height ; puis vérif de dépôt en COMPOSANT
   core/bundle : parse + `Manifest::verify_form` (draft.1 ET draft.2 —
   vérifié : la forme draft.1 de `Manifest::build` == les manifests p2
   gelés, pas de conflit), signature racine (`verify_signature`-équivalent
   sous le did.json stocké) ou déléguée (`authorized_via` : chaîne =
   certs stockés + `verify_chain` core + `verify_delegate_signature`),
   `height == stocké+1`, `prev_hash == manifest_chain_hash de la TABLE`
   (jamais re-hasher les octets stockés). Accepté ⇒ persister l'objet +
   `manifests/<h>.json` + avancer la tête EN TRANSACTION.
3. **POST `/gamma` + If-Head** (une entrée) : CAS sur la tête gamma ;
   vérif d'entrée DÉLÉGUÉE à core (parse strict kind/prevs, signature
   d'entrée owner `#content`/`#root` ou déléguée, chaîne couvrant
   l'opération affichée — `verify_operation_facts` et la machinerie
   mandate/gamma_replay là où core l'expose). Accepté ⇒ append au segment
   UTC du mois d'`entry.at` + avance de tête transactionnelle. ⚠ le scan
   de révocation du #9 lit aujourd'hui `did_doc.revocations` — quand les
   appends arrivent, fusionner les segments dans ce scan (et trancher la
   tension pointeur-vs-segments, arbitrage n°4 du handoff).
4. **PUT `certs/<id>.json`** : id == nom de fichier, subject == `<did>`,
   signature du lien vérifiée (racine `#root` ; sous-mandat : clé du
   grantee parent), chaîne parente résoluble au dépôt.
5. **PUT blobs/hdr/index mandatés** (pass L, lignes ÉCRITURE d'A.3) :
   étendre `mandated_covers` aux verbes d'écriture ; lever le `501` de
   l'arm PUT Mandated de `service.rs`.
6. **Rejeu p7 vert** : `python3 red-replay-p7.py` passe de 15/15 RED à
   15/15 GREEN (il devient le driver de non-régression) ; étendre
   `vectors_replay.rs` pour rejouer p7 byte-exact contre le binaire
   (seeding via le bootstrap `objects` — le pattern gate 3). p2 gelé doit
   AUSSI passer (mêmes routes). Lever les `@wip` des scénarios
   `@cas`/`@artifacts` en implémentant leurs steps. `@witness @gate7` et
   tout `store-cold-roundtrip` RESTENT `@wip` (gates 7–8).
7. **Arbitrages à porter au gate** (jamais résolus en douce) : ① merge vs
   lettre A.4 (`prev == tête stockée` — le vecteur fige le cas compatible ;
   redline candidate `stocké ∈ merges`) ; ② nonce 16-car. (l'appliquer
   casse p1 → redline ou nouveaux ids) ; ③ pointeur `revocations` vs
   segments mensuels ; ④ la redline A.1 draft.2 (manifests/, changesets/,
   evidence/, alias K1-C) requise pour SERVIR les paquets — gate 5/8, mais
   à graver dès qu'un GET la touche.

**Chaque écart au contrat = redline A.2–A.5 minimale proposée au gate,
jamais un accommodement silencieux. STOP au gate 4 : preuves listées,
dérives documentées, parole de Mathieu avant l'étape 5.**

## OÙ

- Code : `code/aithos-core` branche `feat/obligations`, crate
  `rust/crates/aithos-provider` (le #9 est dans `src/envelope.rs`, le
  path-map dans `src/pathmap.rs`, la surface dans `src/service.rs`, le
  seam objets dans `src/objects.rs`). Façade bundle :
  `rust/crates/aithos-bundle/src/publication.rs`. Vecteurs : `vectors/`
  (p7/p8 + `gen-p7-bundle/`). Recette des paquets :
  `aithos-bundle/tests/cb12_publication_package.rs`.
- Provider infra : `provider` branche `feat/p6-p7-tunnel` (rien à toucher
  avant l'étape 6 ; e2e behave à l'étape 8).

## ENVIRONNEMENT (⚠ savoir durement acquis — lire avant d'agir)

- La VM Cowork device (`device_bash`) peut être MORTE (« failed to
  start ») : dans ce cas AUCUN git/shell device — fichiers via
  `device_commit_files` (uuids SendUserFile), **commits git à la main de
  Mathieu**, redémarrage de l'app desktop possible pour la ranimer.
- Le pont REFUSE l'extension `.feature` en staging (HTTP 400) : lire les
  features via un TARBALL (`_transfer/aithos-core-src.tgz`, arbre complet
  au 2026-07-18) ; l'écriture `device_commit_files` les accepte.
- Sandbox cloud : extraire le tgz PUIS overlay des fichiers plus récents
  (mtimes des `device_list_dir`). `cargo test -p aithos-bundle` exige TOUS
  les vecteurs `cb2-*` (include_bytes). Les features bundle du tgz sont
  antérieures à CB13 (229 scénarios au lieu de 815) : la passe locale de
  Mathieu fait autorité sur ce chiffre.
- `pip install --break-system-packages blake3 pynacl base58
  gherkin-official`. Musl/`zig`/`crane` : étape 6 seulement. Creds AWS
  `.aws-env` (SSO — vérifier `AWS_CREDENTIAL_EXPIRATION`) : étape 6
  seulement, purger après usage, jamais dans un log/dépôt.

## TESTER

```sh
cd code/aithos-core/rust
cargo test -p aithos-provider --features pod-stub      # 72/72 + 11 binaires verts AVANT ton code
cargo test -p aithos-core -p aithos-bundle --locked    # la façade ne régresse jamais
cargo clippy -p aithos-provider --all-targets --features pod-stub -- -D warnings
cd ../vectors
python3 gen-p.py && python3 verify-p.py                # p1..p6 byte-identiques
python3 gen-p7.py && python3 gen-p8.py && python3 verify-p7.py
cargo build -p aithos-provider --bin aithos-store-api  # (depuis rust/)
python3 red-replay-p7.py ../rust/target/debug/aithos-store-api   # 15/15 RED aujourd'hui → 15/15 GREEN = ton gate
```

## NORMATIF

INFRA-PROVIDER **annexe A** : A.2 (ordre 0–10, fail-closed), A.3 (routes +
path-map), A.4 (vérification d'artefacts au dépôt — déléguée), A.5 (CAS
des deux têtes : manifest `chain_hash` = SHA-256 du JCS avec
`signature.value=""` ; gamma = SHA-256 du JCS de la dernière entrée), A.7
(registre fermé — `cas_mismatch` porte `head`+`height`,
`artifact_invalid` porte un `reason` court fermé), A.8 (limites + logs).
Le serveur n'évalue AUCUNE contrainte de comptage (budgets, max_actions —
l'autorité des verifiers, §3.1). JCS RFC 8785, Ed25519 sur
JCS-`value=""`, multibase `z6Mk…`, BLAKE3, RFC 3339 Zulu.

## PREMIÈRE ACTION

Lire l'état (ce prompt + `HANDOFF-PROVIDER-P2-GATE3-DONE-2026-07-20.md` +
annexe A + `p7-store-publication.json` + `envelope.rs`/`service.rs`/
`objects.rs` in situ), vérifier que la suite est verte AVANT ta première
ligne (`cargo test -p aithos-provider --features pod-stub` et le rouge
15/15 de `red-replay-p7.py`), **confirmer le cadrage à Mathieu**, puis
implémenter dans l'ordre 1→6 ci-dessus. STOP au gate 4. Ne réimplémente
aucune règle core/bundle : compose leurs primitives.

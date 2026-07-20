# HANDOFF — Piste P / Provider P2 : gate contrat CLOS + gate 3 (autorisation mandatée) LIVRÉ

**Date :** 2026-07-20
**Dépôts :** `code/aithos-core` (branche `feat/obligations`) et `provider` (branche `feat/p6-p7-tunnel`, intouché cette session)
**Statut :** gate contrat (features + vecteurs rouges) **clos par Mathieu** (committé 2026-07-20). Gate 3 (étape 3 — A.2 #7–#10) **implémenté, vert dans le sandbox, posé dans l'arbre** — commit + passe locale de Mathieu = clôture formelle. Étapes 4–8 restantes.

> Se lit avec `HANDOFF-PROVIDER-P2-RESUME-2026-07-19.md` (le cadrage
> d'ensemble P2, toujours valable), `INFRA-PROVIDER.md` (annexe A) et
> `PROMPT-REPRISE-PROVIDER-P2-GATE4-2026-07-20.md` (le prompt de reprise).

---

## 1. Gate contrat — CLOS (commits Mathieu 2026-07-20)

- **Features** (`rust/crates/aithos-provider/tests/features/store/`) :
  `store-publication.feature` (24 scénarios) + `store-cold-roundtrip.feature`
  (8 scénarios). Un cas par ligne du contrat, chaque refus nomme son code A.7.
- **Vecteurs** (`vectors/`) : `p7-store-publication.json` (8 manifest + 2
  certs + 5 gamma), `p8-cold-roundtrip.json` (paquet froid + 3 sabotages +
  plan de lectures), `p7-bundle-packages.json` (intermédiaire committé,
  sha256 `00793c25…`) — **5 paquets réels** émis par le helper
  `vectors/gen-p7-bundle/` (crate autonome hors workspace, lock committé)
  qui n'appelle QUE la façade bundle : `assemble_draft2_candidate` →
  `export_keyless` → `verify_for_cas`. Zéro crypto réinventée.
- **Oracles Python** : `gen-p7.py`, `gen-p8.py` (générateurs),
  `verify-p7.py` (seconde implémentation — digests de paquets recalculés
  from scratch, chaîne déléguée vérifiée sous la racine du DID littéral),
  `red-replay-p7.py` (driver de preuve rouge contre le vrai binaire).
- **Ancres byte-exactes prouvées** : clés a1 rederivées ; `did.json`
  reconstruit par `DidDocument::build` == `p1.did_json_jcs` octet à octet ;
  cas délégué == le candidat `cb2-draft2-carriers` gelé ré-exporté keyless ;
  émission Rust déterministe (2 runs identiques) ; `p1..p6` byte-identiques.
- **Rouge observé avant tout code** : cucumber 98 scénarios → 66 verts
  (P1/M2 intact) + 32 rouges (= les 32 nouveaux) ; rejeu wire **15/15
  rouges** contre le binaire, chaque refus = la barrière P1 exacte.

## 2. Gate 3 — autorisation mandatée : LIVRÉ (dans l'arbre, commit à faire)

### Ce qui a changé (10 fichiers)

- **`src/envelope.rs`** — le `#9` n'est plus une barrière :
  - `#7` : chaîne chargée depuis les **certs stockés** (`certs/<id>.json`,
    ids validés par `pathmap::mandate_id_is_valid` avant de devenir clés de
    stockage), `feuille.grantee.pubkey == key` sinon `chain_invalid` ;
  - `#9` : **délégué à core, deux appels composés, zéro règle recopiée** —
    `aithos_core::mandate::verify_chain(chain, did_doc, at)` (liens,
    subject, fenêtres à `at`, atténuation §05.3) puis
    `revocation::revocations()` + `chain_revoked_at(chain, revs,
    now_serveur)` sur le **log stocké pointé par `did_doc.revocations`**.
    Asymétrie fail-closed : log absent = état vide prouvable (accepte) ;
    log imparsable = refus `chain_invalid`. Anti-rollback du log = domaine
    du témoin (annexe C), pas de ce check.
  - `#10` : `pathmap::mandated_covers` sur le périmètre de la FEUILLE
    (l'atténuation était l'affaire du #9 core). `Principal::Mandated` ajouté
    (sans payload — la chaîne s'y ajoutera à l'étape 4 quand A.4 en aura
    besoin).
- **`src/pathmap.rs`** — `mandated_covers()` : lignes **lecture** d'A.3,
  littérales. Chaîne valide → `/heads`, `manifest.json`, `did.json`,
  `certs/**`, `e/public/**` ; `read.gamma` → segments filtrés au mois par
  `since`/`until` grossiers (comparaison lexicographique RFC 3339 Z, zéro
  arithmétique d'horloge) ; `read.<zone>` → index/hdr/blobs avec la règle
  gravée « un sélecteur qui ne peut pas exclure côté serveur SERT » (seul
  `id=` exclut, par sid) ; `act.x.<id>` → `x/<id>/**`. **Lignes écriture =
  refus par défaut jusqu'au gate 4** (le 403 propre d'A.3).
- **`src/service.rs`** — arm PUT à trois principals : Owner écrit (P1),
  Mandated → `501 not_implemented` (couverture sans capacité = barrière
  honnête, inatteignable tant que #10 refuse les écritures), Anonymous →
  `not_covered` (défensif).
- **`src/control.rs`** — bootstrap additif : chaque DID peut porter
  `objects: [{key, utf8}]` (artefacts PUBLICS de fixture — certs, segments
  gamma, blobs. Opaques au chargement, vérifiés à l'usage ; P7 remplace).
  `PreloadedDoc` devient `(tenant, did, chemin, bytes)`.
- **`src/bin/store_api.rs`** — consomme le tuple à 4.
- **`tests/cucumber.rs`** — fixtures enrichies (mandate_jcs, gamma states,
  clé `revocations`), steps de seeding (`seed_chain_state`), le When
  mauvaise-feuille, et le **filtre `@wip`** (pattern exact du harnais
  bundle : `filter_run_and_exit(…, |_,_,sc| !sc.tags.iter().any(|t| t ==
  "wip"))`).
- **`tests/vectors_replay.rs`** — bootstrap du binaire enfant enrichi
  (cert + gamma post_revoke en SURENSEMBLE — correct pour tous les cas car
  la révocation est forward-only, évaluée au `server_now` de chaque cas —
  + le blob couvert) ; les 5 assertions deferred **flippées byte-exact**.
- **3 évolutions de features** (contrat raffiné en landant les steps) :
  1. tags `@wip` par scénario (héritage feature→scenario non fiable dans
     cucumber-rs ; le pattern bundle est per-scénario) : 6 vivants
     (`@authorization`), 26 gelés ;
  2. Given d'état de chaîne ajouté à 3 scénarios — dont UN de
     `store-hello.feature` (« mandated corrupted signature → 401 ») écrit
     dans le monde P1 où la vérif de feuille #7 était différée : l'ordre
     normatif 7→8 exige l'état de chaîne pour même atteindre #8 ;
  3. scénario « owner couvre tout » réécrit sur `e/self/**` (le périmètre
     que NI l'anonyme NI le mandat p1 n'atteignent — meilleur contrat que
     GET `manifest.json` sur un store qui n'en a pas).

### Preuves (sandbox cloud, 2026-07-20)

| Preuve | Résultat |
|---|---|
| `cargo test -p aithos-provider --test cucumber` | **72/72 scénarios** (398 steps) — 6 @authorization verts, P1/M2 intact |
| `cargo test -p aithos-provider --test vectors_replay` | **2/2** — les 5 p1-deferred **byte-exact** aux verdicts committés (accept 2xx, `chain_invalid`, `not_covered`, `chain_revoked`, paire nonce intacte) |
| `cargo test -p aithos-provider --features pod-stub` | 11 binaires de test, tous verts |
| `cargo test -p aithos-core -p aithos-bundle --locked` | exit 0, 52 binaires (⚠ features bundle du sandbox datées du tgz de la veille : 229 scénarios joués — la passe locale Mathieu fait autorité sur les 815) |
| `cargo clippy -p aithos-provider --all-targets --features pod-stub -- -D warnings` | 0 |
| `cargo fmt --check` | clean |
| `python3 gen-p.py && verify-p.py` ; `gen-p7/p8 && verify-p7` | verts ; `p1..p6` byte-identiques au dépôt |

### Commit à faire (Mathieu, après sa passe locale)

```sh
cd code/aithos-core
git add rust/crates/aithos-provider
git commit -m "P2 gate 3: autorisation mandatée — #7/#9/#10 branchés sur core, rejeu p1 byte-exact vert"
```

## 3. Arbitrages ouverts (constatés, jamais résolus en douce)

1. **A.1/A.3 vs layout draft.2 (redline, gate 5/8).** Les paquets réels
   épinglent `manifests/<h>.json`, `changesets/<hash>.json`,
   `evidence/<hash>.json`, les alias K1-C `public/sections/<sid>.md`,
   `circle/blobs/<sid>.json`, `indices/`, `roots/`, `vault/` — absents de
   la grammaire wire A.1. Servir un paquet draft.2 (GET, batch, sync, cold
   roundtrip p8) exige la redline. Nommé dans les descriptions p7/p8.
2. **Merge vs lettre d'A.4 (gate 4).** `prev_hash` d'un merge = premier
   parent trié (topologie bundle §02.6) ; si le store a sérialisé l'AUTRE
   jumeau, « `prev_hash == chain_hash(stocké)` » refuse un merge légitime.
   Le vecteur p7 fige le cas compatible (state = min des deux têtes) ;
   redline candidate : accepter `stocké ∈ merges`.
3. **Nonce 16 car. (A.2).** L'appliquer casserait le rejeu p1 byte-exact
   (nonces plus courts dans les rejects committés). Résolution = redline
   INFRA-PROVIDER (borne basse guidance client, serveur ≤64 anti-abus) OU
   nouveaux ids de vecteurs. Main de Mathieu.
4. **`revocations: gamma/gamma.jsonl` (did p1 gelé) vs segments mensuels
   A.1 (gate 4).** Le #9 lit aujourd'hui le pointeur du did.json. Quand
   POST `/gamma` appendra aux segments `gamma/<YYYY-MM>.jsonl`, le scan de
   révocation devra fusionner les segments — et la tension
   pointeur-vs-layout devra être tranchée (redline A.1 ou convention DID).

## 4. Prochaines étapes (l'ordre P2 inchangé)

- **Étape 4 (gate suivant)** : A.4/A.5 — PUT `manifest`/`certs` + POST
  `/gamma` + **CAS atomique des deux têtes** (nouveau seam têtes ; le seam
  `ObjectStore` n'a volontairement PAS d'écriture conditionnelle).
  Vérifs de dépôt = **composer** les primitives core/bundle (manifest :
  `Manifest::verify_form` + signature racine/déléguée via
  `verify_delegate_signature`/`verify_chain` + height/prev vs la TABLE des
  têtes ; gamma : vérif d'entrée core + chaîne couvrant l'op affichée via
  `verify_operation_facts` ; certs : id/subject/signature du lien).
  `verify_for_cas()` plein reste le chemin producteur/cold (p8) — confirmé
  par le fait que p2 (gelé) attend l'accept de manifests draft.1 sans
  carriers, et que la forme draft.1 de `Manifest::build` == la forme des
  manifests Python p2 (conflit p2/A.4 dissous, vérifié). Les 15 cas p7
  passent verts ; `red-replay-p7.py` devient le driver de non-régression.
- Étape 5 : heads/batch/sync (A.3) + redline A.1 draft.2.
- Étape 6 : S3 + DynamoDB derrière les seams ; **trancher Lambda-vs-Fargate
  du store** (INFRA-PROVIDER §7 note gravée). Relais Fargate intouché.
- Étape 7 : témoin sur head canonique (annexe C, KMS Ed25519 sign-only).
- Étape 8 : vrai E2E (p8) — bundle grantee → HTTP → restart → store vierge
  → cold verify → lectures owner/grantee. `store-p2.feature` behave (e2e
  wire) s'écrit à ce gate (décision Mathieu au cadrage).

## 5. Environnement — le savoir opérationnel de cette session (⚠ nouveau)

- **`device_bash` (VM Cowork) est resté MORT toute la session** (« failed
  to start ») : aucun git/shell device. Fichiers posés via
  `device_commit_files` (uuids SendUserFile) ; **les commits git sont à la
  main de Mathieu**. Un redémarrage de l'app desktop peut ranimer la VM.
- **Le pont refuse l'extension `.feature` en STAGE (HTTP 400)** — lecture
  impossible fichier à fichier. Contournements prouvés : un TARBALL les
  fait passer (`_transfer/aithos-core-src.tgz`) ; l'ÉCRITURE
  (`device_commit_files`) accepte `.feature`.
- **Recette sandbox** : extraire le tgz (arbre complet du dépôt au
  2026-07-18) puis **overlay** des fichiers plus récents un à un (mtimes
  des listings). Les tests bundle/core `include_bytes!` les vecteurs
  `cb2-*` — TOUS nécessaires pour `cargo test -p aithos-bundle`. Les
  features bundle (racine `features/`) du tgz sont ANTÉRIEURES à CB13
  (229 scénarios au lieu de 815) et non-rafraîchissables (blocage
  `.feature`) : la passe locale fait autorité.
- **Layout K1-C draft.2 (appris du code)** : les corps des mutations
  vivent aux ALIAS `public/sections/<sid>.md` (authorship UNIQUEMENT pour
  zone `public`, chemin dérivé du sid par core) et
  `circle/blobs/<sid>.json` ; chaque op contenue doit épingler
  `history_heads == predecessors` de l'édition EXACTEMENT ; une édition
  exige ≥1 op contenue (« no contained actor ») ; un merge valide = une op
  FRAÎCHE projetée à l'édition de fusion. La recette complète des paquets :
  `aithos-bundle/tests/cb12_publication_package.rs` (owner_cold_fixture) +
  `vectors/gen-p7-bundle/src/main.rs`.
- Cloud : cargo 1.95, python 3.11 (`pip install --break-system-packages
  blake3 pynacl base58 gherkin-official`). `zig`/`crane` absents (utiles
  seulement au déploiement, étape 6+). AWS non touché cette session —
  creds `.aws-env` (SSO, vérifier `AWS_CREDENTIAL_EXPIRATION`) seulement à
  partir de l'étape 6.

## 6. Interdits (rappel, inchangés)

Réimplémenter une règle core/bundle (DÉLÉGUER) ; toucher
`aithos-client`/`aithos-gateway`/CLI/WASM ; bumper `desired_count` relais ;
apply sans plan lu + parole de Mathieu ; merge `main` sans gate ; modifier
un vecteur gelé (`p1..p6`, `cb2-*`, et désormais `p7`/`p8`/
`p7-bundle-packages` une fois committés) — nouveau id + redline par gate ;
brancher le témoin sur un feed de publication avant que le head canonique
soit figé (étape 7).

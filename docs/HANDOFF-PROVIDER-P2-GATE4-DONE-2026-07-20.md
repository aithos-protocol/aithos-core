# HANDOFF — Piste P / Provider P2 : gate 4 (A.4/A.5 — dépôts vérifiés + CAS des deux têtes) LIVRÉ

**Date :** 2026-07-20
**Dépôts :** `code/aithos-core` (branche de travail ; HEAD constaté sur
`codex/publish-aithos-core-busl`, gate 3 non committé au démarrage de la
session — état DISQUE = vérité, confirmé par Mathieu au cadrage) ;
`provider` intouché.
**Statut :** étape 4 **implémentée et verte dans le sandbox** — STOP au
gate : preuves listées ci-dessous, arbitrages ①–⑦ à trancher, commit +
passe locale de Mathieu = clôture formelle.

> Se lit avec `HANDOFF-PROVIDER-P2-GATE3-DONE-2026-07-20.md` (état gate 3),
> `PROMPT-REPRISE-PROVIDER-P2-GATE4-2026-07-20.md` (la mission) et
> `INFRA-PROVIDER.md` annexe A (normatif).

---

## 1. Livré (étape 4, dans l'ordre de la mission)

### 1.1 `src/heads.rs` (NOUVEAU) — le seam CAS des têtes (A.5)

- `HeadsRecord` = le tuple A.5 : `{height, manifest_chain_hash (hex nu,
  la valeur que le successeur épingle en `prev_hash`), gamma_head
  (`sha256:…`, la valeur que l'entrée suivante porte en `prev`),
  gamma_segment}` + `gamma_segments` (voir arbitrage ③).
- `trait HeadsTable { read, cas }` — **compare-and-swap atomique
  opaque** : l'appelant présente le record exact qu'il a lu et celui
  qu'il veut ; la table échange ou rend la vérité courante. Backend
  mémoire (`MemHeads`) ; DynamoDB à l'étape 6 DERRIÈRE ce seam.
- Le seam `ObjectStore` n'a **pas** gagné d'écriture conditionnelle
  (voulu) : rien d'autre ne peut se prendre pour le CAS.

### 1.2 `src/artifacts.rs` (NOUVEAU) — vérification de dépôt A.4, composée

- `deposit_manifest` — PUT `manifest.json` + `If-Head` : ordre exact de
  l'oracle p7 gelé : 428 `cas_required` → 409 `cas_mismatch` (+ tête +
  height) → forme (parse + canonicité JCS + `Manifest::verify_form`,
  draft.1 ET draft.2) → signature racine (`Manifest::verify_signature`
  sous le did.json stocké) ou déléguée (certs stockés +
  `verify_chain_composed` à `edition.created_at` +
  `Manifest::verify_delegate_signature` ; le déposant DOIT être l'acteur
  de l'édition : chaîne d'enveloppe == `authorized_via`, §02.6.1) →
  `height == stocké+1` et `prev_hash == manifest_chain_hash de la TABLE`
  (jamais un re-hash d'octets stockés) → **CAS d'abord** (le perdant
  n'écrit RIEN), puis persistance byte-preserved à `manifest.json` +
  `manifests/<h>.json`.
- `deposit_gamma` — POST `/gamma` + `If-Head` (une entrée) : CAS sur la
  tête gamma → `Entry::check_form` (parse strict core : kinds du
  registre, `prevs` seulement sur merge) → signature/autorité d'entrée
  **déléguée à core** (`verify_owner_entry` `#content` /
  `verify_delegated_entry` — chaîne == `authorized_via`, vérifiée à
  `entry.at`, feuille signataire, couverture de l'action affichée) →
  `prev == tête stockée` → append au segment UTC du mois d'`entry.at` +
  avance de tête (CAS d'abord, sérialisé par un verrou in-process
  par `(tenant, did)` — voir §3.4).
- `deposit_cert` — PUT `certs/<id>.json` : canonicité JCS, `id` == nom
  de fichier, `subject == <did>`, chaîne parente résolue AU DÉPÔT depuis
  les certs stockés (racine→feuille, borne anti-abus 16) et vérifiée par
  `verify_chain_composed` à `now_serveur`.
- Registre fermé des `reason` d'`artifact_invalid` (A.7) : `form`,
  `signature`, `chain`, `prev_hash_mismatch`, `id_mismatch`,
  `subject_mismatch`, `entry_signature`, `prev_mismatch` — exactement
  ceux de l'oracle `verify-p7.py`.

### 1.3 `#9`/`#10` (envelope.rs) et path-map écriture (pathmap.rs)

- `Principal::Mandated(Vec<Mandate>)` porte désormais la chaîne vérifiée
  (l'égalité acteur/chaîne d'A.4 en a besoin).
- Le scan de révocation du `#9` lit le log pointé par
  `did_doc.revocations` **∪ les segments mensuels appendés** (la liste
  `gamma_segments` de la table) — un revoke accepté par POST `/gamma`
  mord immédiatement, sans réécriture de pointeur (arbitrage ③).
- `mandated_covers` : lignes ÉCRITURE d'A.3, littérales — verbe
  d'écriture (`edit|append|write|delete`) sur la zone → PUT
  blobs/hdr/index + POST `/gamma` ; `act.x.<id>.*` → PUT `x/<id>/**` +
  POST `/gamma` ; « owner, ou délégué avec authorized_by » → PUT
  `manifest.json`/`did.json`/`certs/**`/réplique de segment (couverture
  pour toute chaîne valide, l'autorité reste au dépôt A.4) ; écrire
  `e/public/**` n'a PAS de ligne A.3 → refus (lié à la redline ④). Le
  501 de l'arm PUT Mandated de `service.rs` est levé.

### 1.4 Surface (service.rs) + bootstrap (control.rs, store_api.rs)

- Dispatch : PUT `manifest.json` → publish (CAS) ; PUT `certs/<id>` →
  dépôt ; POST `/gamma` → append ; PUT blobs/hdr/index → light-form A.4
  (owner ET mandaté couvert) ; `did.json` et réplique de segment restent
  `501` (voir §4, hors périmètre étape 4) ; heads/batch/sync/list = gate 5.
- Réponses : accept publish = `200 {"head","height"}` ; accept append =
  `200 {"head"}` ; accept cert = `204` ; `cas_mismatch` porte
  `head` (+`height` manifest) ; `artifact_invalid` porte `reason`
  (arbitrage ⑥ : le corps de réponse d'accept n'est pas spécifié par
  l'annexe — redline candidate).
- Bootstrap additif `heads` par DID (seed de la table A.5 — fixtures de
  rejeu SEULEMENT ; l'enrôlement P7 ne seed jamais, la genèse passe par
  le CAS). `load_bootstrap` retourne `(plane, preloads, head_seeds)`.

### 1.5 Rejeux — les preuves du fil

- `red-replay-p7.py` **évolué** (arbitrage ⑦ — évolution de driver,
  jamais un vecteur modifié) : chaque cas p7 rejoue contre un serveur
  ENFANT FRAIS seedé de l'état gelé du cas (`state_heads` → bootstrap
  `heads`, `state_objects` → objets) ; le did.json du sujet cb2 est
  synthétisé des seeds COMMITTÉS (did:key-style, la racine EST le DID
  littéral) ; les instants d'enveloppe = `edition.created_at` du
  manifeste (la fenêtre du mandat cb2 est un fait gelé du vecteur).
  Vérifie aussi `head`/`height`/`reason` octets à octets.
- `tests/vectors_replay.rs` : + `p2_cases_replay_wire_exact` (p2 GELÉ,
  mêmes routes — le `state_head` nu de p2 est normalisé vers le tuple
  A.5 via la table `manifests` gelée du vecteur) et
  `p7_cases_replay_wire_exact` (les 15 cas). p1 byte-exact inchangé.
- Cucumber : 16 scénarios `@cas`/`@artifacts` levés de `@wip`, steps
  implémentés sur les cas p7 GELÉS (aucun octet re-dérivé). ⚠ features :
  voir §2.

## 2. ⚠ Features — reconstruction en attente de la vérité disque

Le pont refuse toujours `.feature` en staging (HTTP 400 re-confirmé), la
VM Cowork est restée morte TOUTE la session (même après redémarrage de
l'app), et les 3 features à jour du disque (post-évolutions gate 3,
mtimes 19/07 20:35) sont POSTÉRIEURES au dernier commit (gate contrat,
19/07 20:11). Le tarball demandé à Mathieu
(`_transfer/features-p2-20260720.tgz`) n'était pas encore là à la
rédaction.

Fait en attendant : les versions COMMITTÉES ont été extraites des objets
git loose (commit `c2681b25`, arbre byte-exact) et les 3 évolutions
documentées du gate 3 ré-appliquées :
- tags par scénario (6 `@authorization` vivants ; `@witness @gate7`,
  `@cas @publish @delegated` et tout cold-roundtrip restent `@wip`) ;
- Given d'état de chaîne : `store-hello` « mandated corrupted
  signature » + `store-publication` « accept_get_mandated » (le blob
  couvert) ;
- « owner couvre tout » réécrit sur `e/self/**`.

Résultat : **88/88 scénarios** (66 P1/M2 + 6 authorization + 16 levés).
**À VALIDER contre les fichiers disque de Mathieu avant tout write-back
des features** — le scénario délégué (`@cas @publish @delegated`) du
texte committé (genesis + If-Head none + height 1) est INCOMPATIBLE avec
le paquet gelé `delegated_cb2` (height 2 sur prédécesseur `9998…`) : la
version disque l'a probablement réécrit ; la mienne le garde `@wip` en
attendant. Aucun write-back de `.feature` n'a été fait.

## 3. Preuves (sandbox cloud, 2026-07-20)

| Preuve | Résultat |
|---|---|
| `python3 red-replay-p7.py …/aithos-store-api` | **15/15 RED avant le code** (re-prouvé sur ce binaire) → **15/15 GREEN après** (head/height/reason byte-exacts) |
| `cargo test -p aithos-provider --test vectors_replay` | 4/4 — p1 byte-exact, **p2 gelé wire-exact**, **p7 wire-exact**, deployed skip |
| `cargo test -p aithos-provider --test cucumber` | **88/88 scénarios** (511 steps) — sur features reconstruites (§2) |
| `cargo test -p aithos-provider --features pod-stub` | 11 binaires ok + cucumber_relay 18/18 + cucumber_tunnel 12/12 |
| `cargo test -p aithos-core -p aithos-bundle --locked` | exit 0, 52 binaires (features bundle datées du tgz : passe locale = autorité) |
| `cargo clippy -p aithos-provider --all-targets --features pod-stub -- -D warnings` | 0 |
| `cargo fmt --check` | clean |
| `python3 gen-p.py && verify-p.py` | p1..p6 byte-identiques |
| `python3 gen-p7.py && gen-p8.py && verify-p7.py` | verts, byte-identiques au dépôt |

### 3.4 Note transactionnelle (mémoire)

L'ordre de commit est : vérifier → **CAS** (le point de sérialisation ;
un perdant n'écrit rien) → écrire les objets. Un verrou async in-process
par `(tenant, did)` (`DepositLocks`, service) empêche l'entrelacement
lecture-append-écriture du segment gamma dans UN processus ; multi-
instance = l'écriture conditionnelle DynamoDB + la transaction S3 de
l'étape 6, derrière les seams (rien à réécrire au-dessus).

## 4. Arbitrages — **ACTÉS par Mathieu au gate (2026-07-20)**

> Parole donnée en session (« GO » sur les recommandations). Décisions :
>
> - **④ redline A.1/A.3 draft.2 : OUI sur le principe** — le texte exact
>   (chemins entrant dans la grammaire, lignes de couverture A.3, classes
>   de cache A.6) est la PREMIÈRE ACTION de l'étape 5, validé par Mathieu
>   avant toute ligne de code.
> - **⑥ + ⑧a : GRAVÉS** dans INFRA-PROVIDER.md A.5 (redlines
>   2026-07-20) — réponses d'accept `200 {"head"[,"height"]}` / `204`
>   certs ; `If-Head` hors grammaire → la réponse du mismatch (409).
> - **⑦ : BÉNI** — `red-replay-p7.py` per-case (serveur frais + état
>   gelé par cas) est LE driver de non-régression.
> - **③ : GRAVÉ** dans A.5 (redline 2026-07-20) — scan #9 = pointeur ∪
>   segments appendés ; la liste des mois reste un détail de backend,
>   jamais sur le wire. Le schéma DynamoDB (étape 6) se dessine dessus.
> - **⑧b : write-once à l'étape 6** — re-dépôt identique idempotent,
>   octets différents sous le même id refusés ; comportement actuel
>   inchangé d'ici là.
> - **① : DIFFÉRÉ** — refus du jumeau divergent maintenu (la lettre +
>   l'oracle) ; redline candidate `stocké ∈ merges` re-examinée au
>   premier cas réel, gate 8 au plus tard.
> - **② : GRAVÉ** dans A.2 (redline 2026-07-20) — la borne basse 16
>   devient guidance client ; le serveur n'impose que ≤ 64 (c'est le
>   comportement du code depuis P1, plus aucune dérive).
> - **⑤ + ⑧d : PRINCIPE ACTÉ** — core exportera son vérificateur
>   d'autorité Value-level (et la voie `#root` structurel) au prochain
>   gate qui ouvre core ; l'interim A.4-littéral du provider fait foi
>   d'ici là, marqué provisoire.

Le détail d'origine de chaque arbitrage (contexte + options) reste
ci-dessous pour mémoire.

### Détail d'origine (tel que porté au gate)

1. **Merge vs lettre d'A.4** (hérité) — le vecteur fige le cas
   compatible (`prev == tête stockée`) ; redline candidate
   `stocké ∈ merges`. Le code refuse le cas divergent
   (`prev_hash_mismatch`), comme l'oracle.
2. **Nonce 16 car.** (hérité) — inchangé, borne haute 64 seulement.
3. **Pointeur `revocations` vs segments mensuels** — tranché
   OPÉRATIONNELLEMENT par l'union (le scan #9 lit pointeur ∪ segments
   appendés) ; la table porte pour ça `gamma_segments` (liste des mois
   appendés), un champ d'implémentation EN PLUS du tuple A.5 — redline
   A.1/A.5 ou convention DID à graver.
4. **Redline A.1 draft.2** (hérité) — `manifests/<h>.json` est ÉCRIT par
   le publish (recette bundle export_keyless) mais n'est PAS servable
   (hors grammaire A.1) ; à graver dès qu'un GET la touche (gate 5/8).
5. **NOUVEAU — chaînes draft.3 (K1-C) : composition impossible en
   l'état.** `mandate::verify_chain` (typé) refuse le profil draft.3
   (`validate_form` : draft.1|draft.2) et sa grammaire de périmètre
   (dirs UUID) ne parse pas ; le vérificateur Value-level de core
   (`carriers::validate_authority_documents`) n'est **pas exporté**.
   Interim implémenté (`artifacts::verify_chain_composed`) : typé
   partout où possible ; pour une chaîne homogène draft.3, les contrôles
   A.4-LITTÉRAUX de l'oracle gelé + fenêtres à `at` (contiguïté,
   subject, racine ancrée au DID littéral, signatures sur les octets
   STOCKÉS, feuille == signataire). Kex/atténuation draft.3 restent aux
   verifiers K1-C (client). Le `#10` d'un leaf draft.3 sert les lignes
   « toute chaîne valide » seulement (périmètre imparsable ≠ faute :
   refus par défaut sur les lignes à périmètre). **Redline candidate :
   core exporte son vérificateur Value-level ; l'interim disparaît.**
6. **NOUVEAU — corps de réponse d'accept.** L'annexe ne spécifie pas la
   réponse d'un publish/append accepté ; implémenté `200
   {"head"[,"height"]}` (les valeurs que `/heads` servira) et `204` pour
   les certs. Redline A.3/A.5 minimale à graver.
7. **NOUVEAU — driver `red-replay-p7.py` évolué** (per-case seeding,
   serveur frais par cas, instants par cas, did.json cb2 synthétisé des
   seeds committés) : la version gate-contrat séquentielle ne POUVAIT
   pas représenter les `state_heads` par cas (prouvé : cas 5/6/7/8
   inatteignables séquentiellement). Vecteurs intouchés
   (`gen-p7`/`verify-p7` re-générés byte-identiques).
8. **Mineurs, constatés :** If-Head malformé → `cas_mismatch` (aucune
   3e grammaire) ; re-dépôt d'un cert vérifié = écrasement accepté
   (l'immuabilité A.6 arrive avec le backend réel) ; PUT `did.json` et
   réplique de segment restent `501` (A.4 les définit mais la mission
   étape 4 ne les liste pas — features disque à vérifier) ; entrées
   gamma structurelles signées `#root` → refusées (`verify_owner_entry`
   core = `#content` seul ; « ou #root » d'A.4 sans primitive core —
   même famille que ⑤).

## 5. Fichiers touchés (write-back en cours de gate)

`rust/crates/aithos-provider/` : `src/heads.rs` (nouveau),
`src/artifacts.rs` (nouveau), `src/envelope.rs`, `src/pathmap.rs`,
`src/service.rs`, `src/control.rs`, `src/lib.rs`, `src/bin/store_api.rs`,
`src/bin/relay.rs` (1 ligne), `Cargo.toml` (dep aithos-bundle),
`tests/cucumber.rs`, `tests/vectors_replay.rs` ; `rust/Cargo.lock`
(1 ligne) ; `vectors/red-replay-p7.py`. Features : RETENUES (§2).
`aithos-core`/`aithos-bundle` : **zéro ligne touchée**.

## 6. Environnement (delta session)

- VM Cowork morte toute la session, y compris après redémarrage de
  l'app. `.feature` toujours refusé en staging. **Nouveau contournement
  prouvé : les objets git loose (`.git/objects/…`, sans extension) se
  stagent — arbre committé reconstruit octet à octet** (commit → trees
  → blobs, zlib). Ne donne que l'état COMMITTÉ.
- Terminal interdit de frappe en computer-use (tier « click ») — pas de
  tar à distance par ce chemin.
- L'overlay tgz+mtimes : un fichier stagé peut DISPARAÎTRE de
  `/mnt/user-data/uploads` après des staging ultérieurs — re-vérifier
  la présence avant `cp` (pathmap.rs perdu puis re-stagé cette session).

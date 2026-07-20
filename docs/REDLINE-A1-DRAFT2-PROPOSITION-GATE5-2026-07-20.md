# REDLINE A.1/A.3/A.4/A.6 — servir le layout draft.2 (K1-B/K1-C) — PROPOSITION

> Piste P / Provider P2 — première action de l'étape 5 (arbitrage ④ acté au
> gate 4, 2026-07-20 : « OUI sur le principe ; le texte exact est la PREMIÈRE
> ACTION de l'étape 5, validé par Mathieu avant toute ligne de code »).
> Statut : **ACTÉE — GO Mathieu 2026-07-20, gravée dans `INFRA-PROVIDER.md`
> le même jour** (A.1 routes données, A.3 path-map + note manifests sans
> ligne d'écriture, A.4 sidecars + alias, A.6 cache + CloudFront). Les
> quatre défauts du §5 sont confirmés tels quels ; la symétrie `e/public/**`
> canonique (§5.4) n'est PAS dans cette redline. Ce document reste la trace
> du raisonnement.

## 0. Sources de vérité (aucune invention)

- **Grammaire canonique bundle** : `aithos_bundle::validate_store_key`
  (`rust/crates/aithos-bundle/src/lib.rs`) — la grammaire fermée des objets,
  y compris le bloc commenté « Frozen K1-C draft.2 carrier layout ».
- **Chemins réellement épinglés** par les paquets gelés (`p7-bundle-packages.json`,
  `p8-cold-roundtrip.json`) : `manifests/<h>.json`, `changesets/<64hex>.json`,
  `evidence/<64hex>.json`, `public/sections/<sid>.md`, `circle/blobs/<sid>.json`,
  `indices/public.json`, `roots/public.json`, `vault/catalog-pins.json`
  (+ les chemins déjà dans A.1 : `manifest.json`, `did.json`, `certs/…`,
  `gamma/<YYYY-MM>.jsonl`).
- **Contrat post-redline déjà gelé** : le `read_plan` de `p8-cold-roundtrip.json` —
  le grantee `read.circle` (mandat p1 committé) LIT `circle/blobs/<sid>.json`
  et `public/sections/<sid>.md` (accept), et reste refusé `not_covered` sur
  `e/self/blobs/…` ; l'owner lit `manifest.json`/`did.json`. Ce plan est un
  vecteur gelé : le texte ci-dessous le satisfait à la lettre.
- Spec : §2.6.2 (K1-B, trois membres signés `operation_ref`/`changeset_ref`/
  `evidence_ref`), §2.6.3 (K1-C, sidecars `changesets/<digest>.json` et
  `evidence/<digest>.json`, digest = 64 hex minuscules après `sha256:`).

---

## 1. Texte exact — A.1, puce « Routes données » (ADDITIF)

À insérer à la fin de la puce **Routes données**, après « Tout chemin hors
grammaire → `path_invalid` … » :

> **Layout draft.2 servable (redline gate 5, 2026-07-20).** La grammaire
> admet ADDITIVEMENT les chemins du layout porteur K1-B/K1-C, sous-ensemble
> exact de la grammaire fermée du bundle (`validate_store_key`) :
> `manifests/<h>.json` (`<h>` = entier décimal ≥ 1, sans zéro de tête — le
> slot d'édition écrit par le publish A.5, jamais par un PUT client) ;
> `changesets/<64hex>.json` et `evidence/<64hex>.json` (64 hex minuscules =
> le suffixe du digest K1-C §02.6.3) ; les alias K1-C `public/sections/<sid>.md`,
> `circle/blobs/<sid>.json` (même grammaire `<sid>` que `e/<zone>/blobs/`),
> et les trois clés littérales `indices/public.json`, `roots/public.json`,
> `vault/catalog-pins.json`. Rien d'autre : les clés internes du bundle
> (`manifests/tree-…`, `manifests/index-…`, suffixe `-alt`, `gateway/**`,
> `gamma/gamma.jsonl`) restent HORS grammaire wire — `path_invalid`.

## 2. Texte exact — A.3, path-map (lignes ADDITIVES)

Modifications du tableau path-map (chaque ligne existante reçoit un ajout
en fin de cellule « Chemins servis » ; aucune ligne existante ne perd rien) :

| Périmètre de la chaîne | AJOUT à « Chemins servis » |
|---|---|
| — (anonyme, A2) | + GET `public/sections/**`, `indices/public.json`, `roots/public.json` (alias K1-C de la zone publique — même statut que `e/public/**`) |
| toute chaîne valide du DID | + GET `manifests/<h>.json`, `changesets/**`, `evidence/**`, `vault/catalog-pins.json` (matériel de preuve public par construction K1-B — nécessaire au cold verify sans capacité privée) |
| `read.<zone>[#sel]` | + GET `circle/blobs/<sid>.json` pour `read.circle` (l'alias K1-C du blob de zone — mêmes règles de sélecteur que `e/circle/blobs/**`) |
| verbe d'écriture sur la zone (pass L) | + PUT `circle/blobs/<sid>.json` (zone `circle`) ; + PUT `public/sections/**` (zone `public` — la ligne d'écriture publique qui MANQUAIT, cause du refus constaté au gate 4) |
| owner, ou délégué avec `authorized_by` | + PUT `changesets/<64hex>.json`, `evidence/<64hex>.json`, `indices/public.json`, `roots/public.json`, `vault/catalog-pins.json` (les sidecars et dérivés d'une publication draft.2 — déposés AVANT le publish qui les épingle) |

> Note normative sous le tableau : **`manifests/<h>.json` n'a pas de ligne
> d'écriture** — le slot est écrit par le serveur lors d'un publish accepté
> (A.5) ; tout PUT client sur `manifests/**` répond `not_covered` (le chemin
> est dans la grammaire A.1 — ce n'est pas `path_invalid` — mais aucune
> chaîne, owner compris, ne le couvre en écriture).

## 3. Texte exact — A.4 (puce ADDITIVE, après la puce POST `/gamma`)

> - **`changesets/<64hex>.json`, `evidence/<64hex>.json`** : contrôle de
>   forme léger (JSON parsable, tailles A.8) **+ adressage par contenu** :
>   le nom de fichier doit égaler le digest K1-C recalculé sur les octets
>   déposés — `C("aithos-core/v1/changeset"|"aithos-core/v1/evidence",
>   JCS(objet))`, §02.6.3 — sinon `artifact_invalid` + `reason:
>   "id_mismatch"` (registre A.7 inchangé : le reason existe déjà pour les
>   certs). Aucune vérification sémantique du contenu : la cohérence
>   changeset/evidence/manifest est l'affaire du verifier (K1-B), jamais du
>   store — anti-abus, pas l'autorité.
> - **Alias K1-C** (`public/sections/*.md`, `circle/blobs/*.json`,
>   `indices/public.json`, `roots/public.json`, `vault/catalog-pins.json`) :
>   même contrôle léger que leurs équivalents `e/**` (JSON parsable là où
>   c'est du JSON ; le `.md` public et le porteur de blob restent opaques).

## 4. Texte exact — A.6 (puces MODIFIÉES, additif)

- Ligne `immutable` — ajouter : `manifests/<h>.json`, `changesets/<hash>.json`,
  `evidence/<hash>.json` (adressés par hauteur/contenu, jamais réécrits —
  le write-once ⑧b de l'étape 6 rend la classe opposable).
- Ligne `no-store` — ajouter : `indices/public.json`, `roots/public.json`,
  `vault/catalog-pins.json` (avancent à chaque publication).
- Nouvelle puce : « `public/sections/<sid>.md` : `Cache-Control: public,
  max-age=0, must-revalidate` + ETag fort (SHA-256 des octets) — le sid est
  stable, le contenu peut être réédité. `circle/blobs/<sid>.json` : même
  classe que `e/<zone>/blobs/<sid>.enc` (`private, max-age=0,
  must-revalidate` + ETag fort). »
- Puce CloudFront — ajouter au public fronté : `public/sections/**`,
  `indices/public.json`, `roots/public.json`.

---

## 5. Les quatre décisions que ce texte tranche PAR DÉFAUT (à confirmer)

1. **Anonymat des alias publics.** `public/sections/**`, `indices/public.json`,
   `roots/public.json` alignés sur `e/public/**` (GET anonyme A2). Alternative
   plus fermée : les mettre sous « toute chaîne valide ». Défaut choisi :
   l'alias d'une zone publique EST public — un même octet ne change pas de
   statut selon le chemin qui le nomme.
2. **`vault/catalog-pins.json` sous « toute chaîne valide »** (pas anonyme,
   pas `act.x.*`) : c'est de la preuve de catalogue (evidence K1-B), pas un
   objet de connecteur `x/<id>/**`. Alternative : l'aligner sur `act.x.*`.
3. **Vérif du digest K1-C au dépôt des sidecars** (§3) : le serveur recalcule
   `C(domain, JCS(octets))` — mécanique, borné A.8, zéro autorité. Alternative
   minimale : forme seule (64 hex + JSON parsable), le digest au verifier.
   Défaut choisi : l'adressage par contenu est la définition même du chemin —
   comme `id == nom de fichier` pour les certs. (Si core/bundle exporte un
   helper de digest K1-C, le provider le compose — sinon SHA-256
   domain-separated local, formule §04.5.1, sans rien recopier d'autre.)
4. **La ligne d'écriture publique** (« verbe d'écriture sur `public` → PUT
   `public/sections/**` ») : ouvre AUSSI l'écriture mandatée de la zone
   publique via alias — le refus constaté au gate 4 (« écrire `e/public/**`
   n'a pas de ligne A.3 ») était lié à cette redline. Cohérence : la même
   ligne devrait couvrir PUT `e/public/**` (canonique). Le texte du §2 ne
   l'ajoute PAS (alias seulement) pour rester minimal vis-à-vis des vecteurs
   gelés — dire si tu veux la symétrie canonique dans la même redline.

## 6. Hors périmètre explicite (ne PAS graver)

- `manifests/tree-<h>.json`, `manifests/index-<zone>-<h>.json`, suffixe
  `-alt` : machinerie de merge locale au bundle — jamais sur le wire.
- `gateway/state.json`, `gateway/keys.json` : clés du plan gateway, pas du
  store public.
- `gamma/gamma.jsonl` : pointeur historique (did p1 gelé) lu par le scan #9
  côté serveur (∪ segments, redline ③ gravée) — pas une route wire.
- Le tableau des routes A.3 (GET/`heads`/`batch`/`sync`/`list`) est
  inchangé : cette redline n'ouvre que des CHEMINS, la surface de lecture
  est l'objet de l'étape 5 elle-même.
- A.7 : aucun code nouveau, aucun reason nouveau (`id_mismatch` réutilisé).

## 7. Après le GO

1. Graver §1–§4 dans `INFRA-PROVIDER.md` (marqués « redline gate 5,
   2026-07-20 »), avec les amendements éventuels du §5.
2. Étape 5 (code) : étendre `pathmap.rs` (grammaire + lignes de couverture)
   en COMPOSANT `aithos_bundle::validate_store_key` là où c'est possible
   (allowlist wire par-dessus, jamais une recopie de la grammaire), puis la
   surface `heads`/`batch`/`sync`/`list`, BDD d'abord (features/vecteurs du
   gate contrat, `@wip` levés en passant vert).

# Note de conception — l'exhaustivité du manifeste et le coût qu'elle porte

*28 juillet 2026. **Note de conception, pas une redline.** Rien ici n'est à implémenter aujourd'hui : le sujet ne débloque aucune ligne du scénario pilote et le danger réel — la dérive quadratique — est déjà corrigé. Ce document existe pour que l'analyse ne soit pas à refaire le jour où un bundle dépassera quelques milliers de sections.*

---

## 1. Le constat de départ

Une publication écrit un manifeste qui contient `files` : un dictionnaire **chemin → SHA-256**, une entrée par objet du bundle. Ce dictionnaire doit être sérialisé, re-canonicalisé en JCS, puis signé — **à chaque édition, quelle que soit la taille du changement**.

| Sections | Manifeste | Publication complète |
|---:|---:|---:|
| 1 000 | 121 Ko | ~30 ms |
| 5 000 | 594 Ko | ~167 ms |
| 10 000 | 1,2 Mo | ~416 ms |

Créer un dossier vide sur un bundle de 5 000 sections coûte exactement le même prix que tout réécrire.

Sur un bundle réel de **2 sections**, `files` compte 20 entrées dont **9 sont des sidecars** (`manifests/**`), et deux paires y sont rigoureusement identiques — les copies d'index des zones jamais touchées.

---

## 2. Ce que `files` garantit réellement — et qui n'est pas ce qu'on croit

Première lecture, fausse : « c'est une redondance avec les racines de Merkle ». Une section `circle` porte déjà son `blob_sha` dans sa ligne d'index, laquelle est hachée dans la racine signée. Donc double protection, donc gaspillage.

Cette lecture rate la seconde boucle de `Bundle::verify()` :

```rust
// No unpinned strays besides the manifest itself.
for path in self.store.list("")? {
    if … && !latest.files.contains_key(&path) {
        return Err(err(format!("unpinned file: {path}")));
    }
}
```

**`files` ne garantit pas seulement l'intégrité : il ferme l'ensemble.** Le manifeste signé déclare la liste **complète et exhaustive** des objets du bundle. Rien ne peut y être glissé.

Les racines de Merkle **ne peuvent pas** offrir cette propriété : elles couvrent l'arbre de contenu *atteignable*, pas l'ensemble des fichiers. Un objet parasite qu'aucun index ne référence — header orphelin, segment de journal surnuméraire — ne modifie aucune racine.

### Les deux autres usages, qui confirment la nature de l'objet

| Où | Usage |
|---|---|
| `provider::service.rs:1197` | calcul du pack de `/sync` : diff des cartes entre l'édition détenue et la tête |
| `bundle::publication.rs:299` | référence de l'état antérieur pour dériver un changeset — savoir ce qui existait avant, donc détecter les suppressions |

### Et l'histoire l'explique

§2.10 : *« Roots ride the manifest **beside** the flat file pins (additive, decided 2026-07-11) »*, et *« Empty on pre-H editions »*.

Les épinglages plats **précèdent** les racines. C'était le mécanisme d'intégrité d'origine. Les racines ont été ajoutées **à côté**, jamais à la place, parce qu'elles ne couvraient pas l'exhaustivité. Ce n'est ni un vestige ni une négligence : c'est une propriété distincte conservée faute d'un meilleur porteur.

**Corollaire important : vider `files`, ou le restreindre aux blobs `self`, serait un recul de sécurité, pas une simplification.**

---

## 3. La forme proposée — s'engager sur l'ensemble au lieu de l'énumérer

La propriété voulue est « voici l'ensemble exact des objets, et rien d'autre ». Elle ne demande pas de **lister** N fichiers, seulement de **s'engager** sur cet ensemble. C'est ce que le protocole fait déjà partout ailleurs.

```jsonc
"files_root":  "<hex>",   // mroot des couples (chemin ‖ 0x00 ‖ empreinte) triés par chemin
"files_count": 20         // le cardinal de l'ensemble
```

Les trois propriétés sont conservées, une quatrième est gagnée :

- **exhaustivité** — un objet parasite change la racine, un objet manquant aussi ; et le cardinal ne laisse nulle part où se cacher, exactement l'argument de §7.10 : *« committing `n` gives enumeration nowhere to hide »* ;
- **intégrité** — un octet modifié change la racine ;
- **référence d'état antérieur** — la dérivation de changeset compare deux racines et descend, au lieu de diffier deux cartes ;
- **nouveau : preuve d'inclusion en O(log n) par fichier**, absente aujourd'hui.

Le manifeste redevient **O(1)** : plus de sérialisation O(n), plus de canonicalisation JCS O(n), plus de signature sur 594 Ko. La vérification complète reste O(n) — il faut relire les objets pour recalculer la racine — mais elle n'est payée qu'à la vérification, pas à chaque publication.

Tout reste dans le vocabulaire du design : mêmes domaines `H_leaf`/`H_node`, même `mroot` gauche-lourd, même forme de preuve v1 que §2.10 et §7.10.

### Ce que ça coûte

Changement d'octets **signés**, donc bump de profil de manifeste selon le mécanisme monotone de §0.4 : `files_root` additif, `files` conservé et optionnel, un nouveau profil qui exige la forme racine. Plus :

- régénération des vecteurs qui épinglent des octets de manifeste ;
- `provider::service.rs` — `/sync` passe à la descente de racines, ce que `INFRA-PROVIDER.md` prescrit **déjà** (*« pack des chemins changés depuis N (descente de racines, §02.10) »*) et que le code ne fait pas ;
- `client::publication.rs` — dérivation de changeset et cold verify.

---

## 4. Les sidecars sont un problème séparé

Une racine ne les rend pas supprimables : les retirer la changerait, comme aujourd'hui.

Or les sidecars — `manifests/tree-<h>.json`, `manifests/index-<zone>-<h>.json` — sont des **caches**. Leur perte dégrade la fusion 3-way et `/sync` ; elle ne touche jamais l'intégrité. Ils n'ont donc pas leur place dans le même ensemble fermé que le bundle canonique.

**Proposition : une seconde catégorie engagée séparément**, soumise à une politique de rétention explicite (les K dernières éditions), et absente de l'ensemble fermé du bundle. Un vérifieur qui ne les trouve pas peut toujours vérifier l'intégrité ; il perd seulement la capacité de fusionner ou de synchroniser contre une édition ancienne.

C'est cette séparation qui débloque le lot `0.4`, aujourd'hui impossible : on ne peut pas purger ce que le manifeste épingle.

---

## 5. Une question ouverte : pourquoi les lignes `self` ne portent pas de `blob_sha`

§2.10 note *« §2.8 rows carry no blob hash by design »*. La spec ne dit nulle part **pourquoi**.

Conséquence : une section `circle` est protégée deux fois — par son `blob_sha` haché dans la racine, et par l'épinglage plat. Une section `self` ne l'est qu'une fois, par l'épinglage plat seul. C'est *pour `self`* que la carte est indispensable à l'intégrité.

Or, en cherchant ce qu'un `blob_sha` révélerait de `self` :

- ni noms, ni titres, ni tags, ni structure — tout cela reste dans le chiffré, et §2.8 n'est pas menacé ;
- il révélerait **quelles** sections `self` ont changé entre deux éditions… mais **`files` le révèle déjà**, puisqu'il épingle `e/self/blobs/<sid>.enc` avec son empreinte.

L'information que l'omission semble protéger est donc **déjà publique par l'autre chemin**.

**Si ce raisonnement tient**, ajouter `blob_sha` aux lignes `self` alignerait les trois zones et ramènerait `files` à son seul rôle d'exhaustivité — ce qui rend la forme racine du §3 d'autant plus naturelle.

**À faire avant de trancher** : retrouver la décision d'origine. « By design » suggère une intention ; l'absence de trace n'est pas une preuve qu'il n'y en avait pas.

---

## 6. Ce qui a été fait, et ce qui ne l'est pas

**Fait** — la dérive quadratique est corrigée (`0.1a`) : le coût d'une édition ne grandit plus avec le nombre d'éditions passées. C'était le vrai danger, parce qu'il n'a pas de borne et qu'un pilote actif l'atteint même avec peu de contenu. Mesuré ×4,27 avant, ×0,92 après, 815 scénarios verts.

**Fait** — la double sérialisation du manifeste est supprimée (`0.1c`).

**Non fait, et volontairement** — tout ce que décrit cette note. Le coût résiduel est linéaire en taille du bundle, ce qui est normal, et invisible à l'échelle d'un pilote (~15 ms à 500 sections).

**Ce qui remplace l'implémentation, pour l'instant** : un seuil asserté dans les benchs, qui transforme la dette en alarme. Le jour où une publication dépasse le seuil, cette note est là.

---

## Traçabilité

| Élément | Source |
|---|---|
| Mesures de coût | `MESURE-COUT-EDITION.md`, `examples/scale_probe*.rs`, `examples/scale_profile.rs` |
| Boucle « no unpinned strays » | `bundle.rs::verify` |
| `/sync` par diff de carte | `provider::service.rs:1197` |
| `/sync` prescrit par descente de racines | `docs/INFRA-PROVIDER.md` A.3 |
| Pins antérieurs aux racines | `spec/02-content-tree.md` §2.10, décision du 2026-07-11 |
| `self` sans `blob_sha` « by design » | `spec/02-content-tree.md` §2.10, §2.8 |
| Mécanisme de profil monotone | `spec/00-overview.md` §0.4 |

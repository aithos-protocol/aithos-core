# Mesure — le coût réel d'une édition de section

*28 juillet 2026. Mesures exécutées sur `aithos-bundle` compilé en `--release`, store mémoire (donc sans I/O disque ni réseau : ce sont des planchers).*

---

## 1. Réponse courte

**Le problème est entièrement dans `aithos-bundle`.** `aithos-core` est sain, le client et le SDK sont innocents, le provider est victime et amplificateur. Une petite partie — la portée de la carte `files` du manifeste — est normative et demande une redline étroite.

| Composant | Verdict |
|---|---|
| **`aithos-core`** | ✅ sain. `merkle.rs` fournit `h_leaf`, `h_node`, `mroot`, `mroot_path`, `run_proof` — tout ce qu'il faut pour une mise à jour O(log n). `run_proof` **fait déjà** le repli incrémental. Rien à corriger, juste à utiliser. |
| **`aithos-bundle`** | ❌ **la totalité du problème**, dans trois fonctions (§3). |
| **`aithos-client`** | ✅ innocent. Zéro occurrence de `state_tree`, `zone_build` ou `mroot` : il consomme ce que le bundle produit et l'assemble en package. Il subit. |
| **`aithos-sdk`** | ✅ innocent. `ProviderClient.publish()` téléverse les artefacts fournis, manifeste en dernier. |
| **`aithos-provider`** | ✅ innocent, ❗ **amplificateur** : le CAS porte sur `manifest.json`, qui pèse 1,2 Mo à 10 000 sections. Chaque édition re-téléverse un manifeste O(n) **plus** un index O(n) **plus** un sidecar d'arbre O(n). |
| **`spec/02`** | ⚠️ une décision normative à réviser : la portée de `files` (§4). |

---

## 2. Les chiffres

### A. Coût d'une édition d'une seule section, selon la taille du bundle

| Sections | Édition + publication | manifest.json | index.json | tree-N.json |
|---:|---:|---:|---:|---:|
| 100 | 2,9 ms | 15 Ko | 27 Ko | 14 Ko |
| 1 000 | 23,8 ms | 121 Ko | 269 Ko | 137 Ko |
| 5 000 | 133,6 ms | 594 Ko | 1,3 Mo | 684 Ko |
| 10 000 | **365,8 ms** | **1,2 Mo** | **2,7 Mo** | **1,4 Mo** |

**C'est super-linéaire** : ×10 sections → ×15 temps. La cible de §9.3 est *« state-root update on one edit (1M sections) → < 1 ms »*. À 10 000 sections on est à 366 ms, soit trois ordres de grandeur au-dessus de la cible pour un bundle cent fois plus petit.

### B. Dérive sur des éditions successives — bundle fixe de 1 000 sections

| Édition | Durée | manifest | Objets | Poids du store |
|---:|---:|---:|---:|---:|
| 2 | 23,6 ms | 121 Ko | 1 025 | 2,5 Mo |
| 11 | 27,5 ms | 126 Ko | 1 070 | 7,1 Mo |
| 21 | 33,9 ms | 131 Ko | 1 120 | 12,4 Mo |
| 41 | **45,2 ms** | 141 Ko | 1 220 | **23,0 Mo** |

Le contenu n'a pas bougé : c'est toujours 1 000 petites sections. Mais **le temps a doublé en 40 éditions** et **le store a été multiplié par 9**. Chaque édition ajoute ~0,5 Mo et 5 objets, indépendamment de ce qui a changé.

À 10 000 sections, la même dérive donnerait des éditions à plusieurs secondes et un store de plusieurs centaines de mégaoctets pour quelques mégaoctets de contenu réel.

---

## 3. Les trois causes, toutes dans `aithos-bundle`

### Cause 1 — `state.rs::state_tree()` reconstruit tout, à chaque publication

```rust
pub fn state_tree(&self) -> Result<StateTree> {
    for zone in [Zone::Public, Zone::Circle] { let zb = self.zone_build(zone)?; … }
    let (self_leaves, self_root) = self.self_build()?;
    let (vault_leaves, vault_root) = self.vault_build()?;
}
```

- `zone_build()` relit `e/<zone>/index.json` **en entier**, collecte tous les tags, puis descend récursivement tout l'arbre en recalculant **chaque** hash de nœud.
- `self_build()` relit l'index self complet et re-hache **chaque** ligne.
- `vault_build()` fait `store.list("e/x/")` puis **télécharge et hache chaque objet du coffre**. C'est le pire : de l'I/O sur des données qui n'ont pas changé.

Aucune maintenance incrémentale — alors que la fonction de diff existe déjà, juste à côté : `state.rs::tree_diff(old, new)`.

### Cause 2 — `bundle.rs::all_pinned_files()` relit et hache le bundle entier

```rust
fn all_pinned_files(&self, exclude_latest: u64) -> Result<BTreeMap<String, String>> {
    for path in self.store.list("")? {            // TOUS les fichiers
        files.insert(path.clone(), sha256_hex(&self.get(&path)?));   // lecture + SHA-256 intégral
    }
}
```

À chaque publication, chaque octet du bundle est relu et haché — y compris **tous les manifestes passés, tous les sidecars d'arbre et tous les snapshots d'index**. C'est ce qui produit la dérive du tableau B : plus il y a d'éditions, plus il y a de fichiers à relire, donc plus l'édition suivante coûte cher. **Le coût est quadratique en nombre d'éditions.**

### Cause 3 — `publish_artifacts()` écrit quatre fichiers O(n) par édition

```rust
let tree = self.state_tree()?;
self.put_json(&format!("manifests/tree-{height}.json"), &tree)?;      // O(n)
for zone in ["public", "circle", "self"] {
    let bytes = self.get(&format!("e/{zone}/index.json"))?;
    self.write_object(&format!("manifests/index-{zone}-{height}.json"), &bytes)?;   // O(n) ×3
}
```

Chaque édition **archive une copie complète** de l'arbre d'état et des trois index. Ce sont les 0,5 Mo par édition du tableau B. L'intention est légitime — le sidecar sert aux diffs par descente de racines, les snapshots d'index servent de base au merge 3-way — mais c'est stocké sans rétention ni delta.

### Cause 4 (aggravante) — l'index est monolithique

`ZoneIndex { folders: Vec<FolderRow>, sections: Vec<SectionRow> }` → un seul `e/circle/index.json` contenant *tous* les dossiers et *toutes* les sections. Éditer une section réécrit et re-téléverse le fichier entier.

La spec prévoit déjà la sortie, §2.3 : *« Sharding of large indexes is permitted (deterministic, by `sha256(sid)`) but omitted here for clarity; it does not affect keys or headers. »* C'est un des `FUTUR` du topo — permis, jamais construit.

---

## 4. La part normative — la portée de `files`

§2.10 grave la décision du 2026-07-11 :

> *« Roots ride the manifest **beside** the flat file pins (additive) : the flat pins keep covering byte-rollback of sealed `self` blobs (§2.8 rows carry no blob hash by design). »*

La raison est réelle : une ligne d'index `self` est `{sid, key_version, gamma_ref}` — **aucun hash de blob**. Sans les pins plats, un blob self pourrait être remplacé par une version antérieure sans que rien ne le détecte.

**Mais cette raison ne justifie de pinner que les blobs `self`.** Les sections `public` et `circle` portent déjà `blob_sha` dans leur ligne d'index, et cette ligne est haché dans la feuille Merkle — donc doublement couvertes. Les certificats, les headers, les segments gamma et les manifestes passés ont leurs propres mécanismes d'intégrité.

**Redline proposée** : restreindre `files` à exactement ce que les racines Merkle ne couvrent pas — en pratique les blobs `self` (et, si besoin, les objets de coffre tant que `vault_build` ne les couvre pas correctement). Le manifeste passe alors de O(tous les fichiers) à O(sections self), et surtout `all_pinned_files` cesse de relire le bundle entier.

---

## 5. Le plan de correction

Par ordre de rendement décroissant.

| # | Correction | Où | Gain attendu |
|---|---|---|---|
| 1 | Restreindre `files` aux objets non couverts par les racines | spec §2.10 + `bundle.rs::all_pinned_files` | supprime la lecture intégrale du bundle à chaque publication — **c'est le gain principal** et la cause de la dérive quadratique |
| 2 | Maintien incrémental du `StateTree` : recalculer uniquement le chemin des nœuds modifiés (`run_proof` fait déjà le repli) | `bundle::state.rs` | O(n) → O(log n) sur le calcul de racine ; c'est la cible de §9.3 |
| 3 | `vault_build` : ne pas relire tout le coffre — pinner les hashes et ne recalculer que le delta | `bundle::state.rs` | supprime l'I/O inutile la plus coûteuse |
| 4 | Rétention des sidecars `manifests/tree-*` et `manifests/index-*` : garder les K dernières éditions, pas toutes | `bundle.rs::publish_artifacts` | stoppe la croissance linéaire du store |
| 5 | Shardage déterministe des index par `sha256(sid)` | `bundle.rs` (layout déjà permis par §2.3) | l'édition ne réécrit et ne téléverse qu'un shard, pas la zone |
| 6 | Asserter les seuils de §9.3 dans les benchs Criterion existants | `benches/perf.rs` | transforme la garantie en gate CI mesuré |

**Les corrections 1 à 4 ne touchent aucun octet de wire existant** hors la portée de `files` (correction 1, redline étroite). La correction 5 change le layout de stockage, donc les chemins — additive, mais à décider avant que l'app se construise dessus.

---

## 6. Ce que ça veut dire pour l'app

Trois conséquences directes sur la conception, à prendre en compte dès le départ :

1. **Faire les corrections 1 à 3 avant de construire l'app.** Sinon chaque édition dans le navigateur re-téléverse un manifeste et un index complets, et l'expérience se dégrade visiblement dès quelques milliers de sections. Ce n'est pas un problème qu'on optimise après : il structure le débit du provider et la boucle CAS.
2. **La correction 5 (shardage) change les chemins de stockage.** À trancher avant, pas après.
3. **Instrumenter dès le premier jour.** Les dix benchs de §9.3 existent déjà, annotés ligne par ligne ; il leur manque uniquement des seuils assertés. L'app doit pouvoir afficher le coût réel d'une édition — c'est aussi une bonne démo.

---

## Annexe — reproduction

Deux sondes écrites pour cette mesure, dans la copie locale du dépôt :

- `rust/crates/aithos-bundle/examples/scale_probe.rs` — coût d'une édition selon la taille du bundle
- `rust/crates/aithos-bundle/examples/scale_probe2.rs` — dérive sur N éditions successives
- `rust/crates/aithos-bundle/examples/scale_profile.rs` — décomposition manuelle des phases d'une publication

Le garde-fou exécuté par `cargo test` est déterministe :

- `rust/crates/aithos-bundle/tests/publication_history_regressions.rs` vérifie que
  le nombre de lectures de l'historique immuable reste constant, que les pins
  reportés égalent un scan complet sur un Store append-only, et qu'un objet
  historique écrasé n'est jamais légitimé par une publication ultérieure.

```
cargo run --release -p aithos-bundle --example scale_probe  1000 5000 10000
cargo run --release -p aithos-bundle --example scale_probe2 1000 40
```

# Lot B — Inventaire de capacités : `spec/02-content-tree.md`

> **RECHERCHE NON VÉRIFIÉE — NE PAS UTILISER COMME PREUVE DE CONFORMITÉ.**
> Voir [`README.md`](README.md) pour la provenance, les défauts connus et les
> règles d'utilisation de cet inventaire.

Périmètre : arbre de contenu, zones, dossiers, sections, dérivation, tags, blobs,
éditions, règle de fork, racines Merkle, politique de signature, opérations K1-B / K1-C.

Conventions de colonnes :

- **Force** — `MUST` / `MUST NOT` / `SHOULD` / `MAY` / `FUTUR`.
- **Clé** — `aucune` (calculable/vérifiable par un tiers sans secret → hébergeable
  chez Aithos), `signature` (clé privée Ed25519 requise), `descellement`
  (DK, clé kex ou clé de contenu requise).
- **Implémenté** — `crate::fichier` ou `NON` (ou `PARTIEL` avec précision).
- **Prouvé** — `vecteur:<fichier>` / `test:<fichier>` / `bdd:<feature>` /
  `PROXY` / `@wip` / `RIEN`.
- **Exposé** — `cli` / `wasm` / `client` / `sdk` / `provider-wire` / `gateway` / `AUCUNE`.

Chemins de référence : `core::` = `rust/crates/aithos-core`, `bundle::` =
`rust/crates/aithos-bundle`, `cli::` = `rust/crates/aithos-cli`,
`provider::` = `rust/crates/aithos-provider`, `gateway::` = `rust/crates/aithos-gateway`,
`client::` = `/root/aithos-client/crates/aithos-client`.

---

## Tableau des capacités

| # | § | Capacité | Force | Clé | Implémenté | Prouvé | Exposé |
|---|---|---|---|---|---|---|---|
| 1 | 02.1 | Un chemin canonique sépare ses domaines par les marqueurs `d`/`t`/`s`. | MUST | aucune | `core::path.rs` (`NodePath::parse`/`Display`) | test:core/src/path.rs#parse_display_roundtrip | cli |
| 2 | 02.1 | Une zone est le dossier racine de son arbre. | MUST | aucune | `core::path.rs` (`NodePath::zone_root`) | test:core/src/path.rs | cli |
| 3 | 02.1 | Tout dossier contient librement sous-dossiers et sections, en récursion sans limite de profondeur. | MUST | aucune | `core::path.rs` (`folders: Vec<Sid>`, aucun plafond) | bdd:b-derivation.feature | cli |
| 4 | 02.1 | Un nom humain n'apparaît jamais dans un chemin canonique. | MUST NOT | aucune | `core::path.rs` (seuls `Sid` et tag) | test:core/src/path.rs | cli |
| 5 | 02.1 | Une vue par tag s'ancre sur N'IMPORTE quel dossier, racine de zone incluse. | MUST | aucune | `core::path.rs` (`Leaf::TagView`), `core::derive.rs` (`tag_label`) | vecteur:b2-derivation.json | cli |
| 6 | 02.1 | `/e/public` est en clair : dossiers nommés réels sur disque, aucune cryptographie. | MUST | aucune | `bundle::bundle.rs` (`section_add`, `e/public/<path>.md`) | bdd:d-bundle.feature | cli, client, gateway |
| 7 | 02.1 | `circle` et `self` sont des zones chiffrées, chacune enracinée dans un nœud à DK + header propres. | MUST | descellement | `bundle::bundle.rs` (`init`, `zone_dk`), `core::header.rs` | vecteur:c1-header-seal.json | cli, client |
| 8 | 02.1 | `self` a la même forme que `circle` mais ses chemins sont opaques. | MUST | aucune | `bundle::bundle.rs` (`SelfIndex` plat) | bdd:d-bundle.feature | cli |
| 9 | 02.1 | Un nœud de coffre `/x/<connector>` porte DK + header. | MUST | descellement | `bundle::vault.rs` | vecteur:cb2-bundle-structure-vault.json | cli, gateway |
| 10 | 02.1 | Le parseur rejette zone inconnue, marqueur inconnu, marqueur pendant, segment après terminal, tag invalide, slash initial manquant. | MUST NOT | aucune | `core::path.rs` (`NodePath::parse`) | test:core/src/path.rs#rejects_malformed_paths | cli |
| 11 | 02.2 | Chaque dossier et chaque section porte un `sid` ULID globalement unique, assigné à la création. | MUST | aucune | `core::ids.rs` (`Sid`), `bundle::bundle.rs` (`new_sid`) | vecteur:b2-derivation.json | cli, client |
| 12 | 02.2 | Le `sid` n'est JAMAIS modifié. | MUST NOT | aucune | `bundle::bundle.rs`, `bundle::revoke.rs` (`move_folder` conserve le sid) | vecteur:g3-move.json | cli |
| 13 | 02.2 | Le `sid` est le label de dérivation et le nom de fichier du blob. | MUST | aucune | `core::derive.rs` (`folder_label`/`section_label`), `bundle::bundle.rs` (`blobs/<sid>.enc`) | vecteur:b2-derivation.json | cli |
| 14 | 02.2 | Un nom humain respecte `[a-z0-9_-]{1,64}`. | MUST | aucune | `core::ids.rs` (`validate_name`), `bundle::lib.rs` (`name_accepted`) | vecteur:cb2-bundle-boundaries.json | cli, client |
| 15 | 02.2 | Un nom est unique parmi ses frères. | MUST | aucune | PARTIEL — `bundle::structure.rs` (renommage/déplacement/métadonnées délégués) contrôle ; `bundle::bundle.rs` (`section_add`, `ensure_folder`, `rename_folder`, voie propriétaire) NE contrôle PAS | bdd:n-structural-mutations.feature (voie déléguée seulement) | cli |
| 16 | 02.2 | Un nom est pure métadonnée : clair dans l'index pour `public`/`circle`, scellé pour `self`. | MUST | aucune / descellement (self) | `bundle::bundle.rs` (`SectionRow.name`, `SelfSection`, descripteur scellé) | bdd:d-bundle.feature | cli |
| 17 | 02.2 | Une section est `{sid, name, title, tags[], body}` ; un dossier est `{sid, name, children}`. | MUST | aucune | `bundle::bundle.rs` (`SectionRow`, `FolderRow`, `SelfSection`, descripteur `kind:"folder"`) | vecteur:cb2-bundle-structure-vault.json | cli |
| 18 | 02.2 | Aucun concept `ns` distinct ne survit : `gmail:0042` est du sucre pour dossier `gmail/` + section `0042`. | MUST NOT | aucune | `bundle::bundle.rs` (aucun champ/notion `ns`) | RIEN | AUCUNE |
| 19 | 02.2 | Un chemin d'affichage se résout noms→sids par l'index (`public`/`circle`) ou par les descripteurs scellés (`self`). | MUST | aucune / descellement (self) | `bundle::bundle.rs` (`resolve_clear`, `self_desc_location`) | bdd:d-bundle.feature | cli, client, gateway |
| 20 | 02.2 | Muter l'ensemble de tags d'une section est une édition de la SECTION, jamais de la vue par tag. | MUST | aucune | `bundle::structure.rs` (`structural_edit_metadata` → `structural_section_gate(Verb::Edit)`) | bdd:n-structural-mutations.feature | gateway |
| 21 | 02.2 | Un grant `tag=` couvre les sections portant actuellement le tag, PAS le ré-étiquetage. | MUST NOT | aucune | `bundle::grants.rs` (`check_delegated_write` avec les tags courants) | vecteur:cb2-bundle-authority-flows.json | gateway |
| 22 | 02.2 | Ajouter ou retirer un tag exige un périmètre d'édition `id=`, `dir=` ou zone sur la section elle-même. | MUST | aucune | `bundle::structure.rs` (`structural_section_gate`) | bdd:n-structural-mutations.feature | gateway |
| 23 | 02.2 | Une passe de réparation créant un wrap de tag manquant valide d'abord l'auteur de la mutation de tag (périmètre couvrant au `at` de l'entrée, per gamma) et échoue fermé. | MUST | aucune | NON (aucune passe de réparation n'existe ; la création de wrap n'a lieu qu'en ligne, sous contrôle d'autorité) | RIEN | AUCUNE |
| 24 | 02.3 | La disposition du bundle est exactement la carte canonique de fichiers de §2.3. | MUST | aucune | `bundle::lib.rs` (`validate_store_key`, grammaire fermée) | vecteur:cb2-bundle-boundaries.json | cli, provider-wire |
| 25 | 02.3 | `manifest.json` est signé et chaîné linéairement. | MUST | signature (émission) / aucune (vérification) | `bundle::manifest.rs` | vecteur:i1-concurrency.json + bdd:d-bundle.feature | cli, client |
| 26 | 02.3 | L'index `circle` est un arbre en clair : `folders [{sid,name,parent_sid}]` + `sections [{sid,name,folder_sid,title,tags,blob_sha,key_version,gamma_ref}]`. | MUST | aucune | PARTIEL — `bundle::bundle.rs` (`ZoneIndex`) : `gamma_ref` ABSENT du code | bdd:d-bundle.feature (sans `gamma_ref`) | cli |
| 27 | 02.3 | L'index `self` est une liste plate opaque `[{sid,key_version,gamma_ref}]` — rien d'autre. | MUST | aucune | PARTIEL — `bundle::bundle.rs` (`SelfRow` = `sid`,`key_version`,`access?`) : `gamma_ref` absent, `access` (locateur opaque scellé) en plus | bdd:d-bundle.feature | cli |
| 28 | 02.3 | Les blobs `self` contiennent sections ET descripteurs de dossier scellés, indistinguables. | MUST | aucune (observation) / descellement (lecture) | `bundle::bundle.rs` (`e/self/blobs/<sid>.enc` pour les deux) | bdd:d-bundle.feature | cli |
| 29 | 02.3 | Les headers sont adressés par sid (`e/<zone>/hdr/<node>.json`). | MUST | aucune | `bundle::grants.rs` (`hdr_file`) | vecteur:c1-header-seal.json | cli |
| 30 | 02.3 | Le shardage déterministe des gros index par `sha256(sid)` est permis. | MAY | aucune | NON | RIEN | AUCUNE |
| 31 | 02.3 (CB1) | Un chemin d'affichage non fiable est relatif à sa zone logique déjà sélectionnée et applique la grammaire de nom de §2.2. | MUST | aucune | `bundle::lib.rs` (`validate_display_path`) | vecteur:cb2-bundle-boundaries.json + bdd:d-bundle.feature | cli, client, gateway |
| 32 | 02.3 (CB1) | Un chemin d'affichage rejette préfixe absolu, segment vide ou point, traversée, nom non conforme, et toute résolution qui sortirait de la zone AVANT accès au store. | MUST NOT | aucune | `bundle::lib.rs` (`relative_segments`, `validate_display_path`) | bdd:d-bundle.feature (`../circle/secret`, `/absolute/section`, `folder/./section`, `folder//section`) | cli, gateway |
| 33 | 02.3 (CB1) | Une clé de store est relative, confinée et obéit à la disposition canonique exacte de §2.3 (pas à la grammaire de nom). | MUST | aucune | `bundle::lib.rs` (`validate_store_key`) | vecteur:cb2-bundle-boundaries.json | provider-wire |
| 34 | 02.3 (CB1) | Les chemins logiques canoniques gardent leur préfixe `/e/…` ou `/x/…` et ne sont pas des entrées de chemin d'affichage. | MUST | aucune | `core::path.rs` vs `bundle::lib.rs` (surfaces disjointes) | test:core/src/path.rs | cli |
| 35 | 02.3 (CB1) | `FsStore` ancre sa racine canonique ouverte et refuse tout lien symbolique / jonction / point d'analyse dont la résolution sortirait de cette racine, avant lecture, écriture, listage, chargement d'édition, publication de staging ou récupération. | MUST | aucune | `bundle::lib.rs` (`checked_join`, `ensure_plain_directory`, `collect_from`, `read_generation_marker`) | bdd:d-bundle.feature (`folder/link-out/section`, `../../outside`) | cli |
| 36 | 02.3 (CB1) | Un manifeste signé ne peut légitimer ni une évasion ni un objet hors disposition. | MUST NOT | aucune | `bundle::lib.rs` (`validate_store_key` appliqué sur `get`/`put`/`list`) | vecteur:cb2-bundle-boundaries.json | provider-wire |
| 37 | 02.4 | Le plaintext d'un blob de section `circle` est le JCS de `{md, sig?}`. | MUST | descellement | `bundle::bundle.rs` (`section_add`, `section_rewrite`) | vecteur:cb2-bundle-authority-flows.json | cli |
| 38 | 02.4 | Le plaintext d'une section `self` est `{kind:"section", name, title, tags, md}`. | MUST | descellement | `bundle::bundle.rs` (`SelfSection`) | bdd:d-bundle.feature | cli |
| 39 | 02.4 | Le plaintext d'un descripteur de dossier `self` est `{kind:"folder", name, children:[sids]}`. | MUST | descellement | `bundle::bundle.rs` (`self_add_child`, `write_desc`) | bdd:d-bundle.feature | cli |
| 40 | 02.4 | Le chiffré est `XChaCha20-Poly1305(K_node, nonce, plaintext)`. | MUST | descellement | `core::seal.rs` (`blob_seal`/`blob_open`) | vecteur:g3-move.json | cli |
| 41 | 02.4 | L'AAD du blob a le purpose `blob` et lie `subject_did ‖ chemin sid canonique ‖ key_version`. | MUST | descellement | `core::seal.rs` (`blob_aad`, `PURPOSE_BLOB`) | vecteur:g3-move.json (`blob_aad_hex`) | cli |
| 42 | 02.4 | Une section `public` est du markdown brut ; son intégrité tient au `sha256` de l'index. | MUST | aucune | `bundle::bundle.rs` (`public_read` compare `row.blob_sha`, `public_read_k1c` compare `row.body_hash`) | bdd:d-bundle.feature | cli, client, gateway, provider-wire |
| 43 | 02.5 | `K(zone root)` = DK courante de `/e/<zone>` tirée de son header. | MUST | descellement | `bundle::bundle.rs` (`zone_dk`, `zone_dk_with_owner_kex`) | vecteur:c1-header-seal.json | cli |
| 44 | 02.5 | `K(child folder) = derive("aithos-core/v1/d/"+sid, K(parent))`. | MUST | descellement | `core::derive.rs` (`folder_label`, `node_key`) | vecteur:b2-derivation.json | cli |
| 45 | 02.5 | `K(tag anchor) = derive("aithos-core/v1/t/"+tag, K(folder))`. | MUST | descellement | `core::derive.rs` (`tag_label`) | vecteur:b2-derivation.json (`tag_anchor_folder1_hex`, `tag_anchor_zone_root_hex`) | cli |
| 46 | 02.5 | `K(section) = derive("aithos-core/v1/s/"+sid, K(folder))`. | MUST | descellement | `core::derive.rs` (`section_label`) | vecteur:b2-derivation.json | cli |
| 47 | 02.5 | Une seule dérivation BLAKE3 `derive_key` par segment de chemin ; la profondeur est architecturalement illimitée. | MUST | descellement | `core::derive.rs` (`node_key`) | vecteur:b2-derivation.json (chaîne zone→d1→d2→d3→section) | cli |
| 48 | 02.5 | La dérivation est à sens unique : détenir la clé d'un dossier donne tout son sous-arbre, jamais rien au-dessus ni à côté. | MUST | descellement | `core::derive.rs` (`blake3::derive_key`) | bdd:b-derivation.feature (« A folder holder cannot reach sideways ») | cli |
| 49 | 02.5 | Tout nœud sous la racine de zone PEUT aussi porter son propre header (DK aléatoire + wrap montant) ; les deux routes résolvent. | MAY (porter) / MUST (résoudre) | descellement | `core::header.rs`, `bundle::grants.rs` (résolution par wrap), `bundle::revoke.rs` | vecteur:g3-move.json (wrap montant) | cli |
| 50 | 02.5 | `key_version` dans l'index indique sous quelle génération de DK le blob a été écrit ; le header porte toutes les versions vivantes pour que le lecteur résolve n'importe laquelle. | MUST | descellement | `core::header.rs` (`key_versions`), `bundle::bundle.rs` (`open_blob_v`) | vecteur:g2-rotation.json | cli |
| 51 | 02.6 | Le manifeste porte `edition: {height, prev_hash, created_at}`. | MUST | aucune | `bundle::manifest.rs` (`Edition`) | vecteur:i1-concurrency.json | cli, client |
| 52 | 02.6 | Le manifeste porte les racines d'état `roots: {public, circle, self, vault}` et le sommet du log `gamma_head`. | MUST | aucune | `bundle::manifest.rs` (`roots`, `gamma_head`) | vecteur:h1-merkle.json + bdd:h-merkle.feature | cli, client |
| 53 | 02.6 | Le manifeste est signé par la racine propriétaire, ou par un délégué avec `authorized_via`. | MUST | signature | `bundle::manifest.rs` (`ManifestSigner`, `verify_delegate_signature`) | bdd:m-delegated-editions.feature | cli, client |
| 54 | 02.6 | `prev_hash` = SHA-256 du JCS du manifeste précédent avec `signature=""`. | MUST | aucune | `bundle::manifest.rs` (`unsigned_jcs`, `chain_hash`) | bdd:d-bundle.feature (« A broken chain fails closed ») | cli |
| 55 | 02.6 | Les éditions forment une chaîne linéaire : la hauteur croît strictement et chacune épingle son prédécesseur. | MUST | aucune | `bundle::bundle.rs` (`verify`, boucle `1..=height`) | bdd:d-bundle.feature | cli |
| 56 | 02.6 | Une édition n'est valide que si elle prolonge la plus longue chaîne vue par le vérifieur ET que son `prev_hash` correspond. | MUST | aucune | PARTIEL — `bundle::bundle.rs` (`verify`) contrôle `prev_hash` et la hauteur ; la règle « plus longue chaîne vue » n'est pas implémentée | bdd:d-bundle.feature (chaînage seul) | cli |
| 57 | 02.6 | Deux éditions concurrentes de même hauteur touchant des ensembles de nœuds disjoints ne sont pas un conflit et PEUVENT être fusionnées en `height+1`. | MAY | signature (publier) / aucune (vérifier) | `bundle::merge.rs` (`edition_merge_as`, `fork_check`) | vecteur:i1-concurrency.json + bdd:i-concurrency.feature | cli (`edition-merge`) |
| 58 | 02.6 | Le publieur de la fusion est un propriétaire OU un grantee feuille dont la chaîne unique couvre chaque opération typée et chaque changement des deux parents. | MUST | signature | `core::concurrency.rs` (`verify_disjoint_merge`, `MergeAuthority`), `bundle::merge.rs` (`write_covers_labels`) | vecteur:cb2-bundle-concurrency-final.json + bdd:i-concurrency.feature | cli |
| 59 | 02.6 | L'édition de fusion applique les deux changesets et liste ses parents par hash d'édition croissant dans `merges: [hash_a, hash_b]`. | MUST | aucune | `bundle::manifest.rs` (`merges`), `bundle::merge.rs` | vecteur:i1-concurrency.json | cli |
| 60 | 02.6 | Tout vérifieur calcule le même résultat de fusion (octet pour octet). | MUST | aucune | `bundle::merge.rs` (fusion déterministe + JCS) | bdd:i-concurrency.feature (« Two mergers produce byte-identical merge manifests ») | cli |
| 61 | 02.6 | La fusion ne contourne jamais l'autorité. | MUST NOT | aucune | `core::concurrency.rs` (`verify_authority`), `bundle::merge.rs` | bdd:i-concurrency.feature (« A merge author must cover every derived change ») | cli |
| 62 | 02.6 | Un fork proprement dit (conflit même nœud, ou fusions irréconciliables) est résolu par le gestionnaire commun le plus proche, dont le périmètre couvre chaque nœud touché par les deux branches. | MUST | signature | `bundle::merge.rs` (`resolve_fork`, `verify_resolution_edition`) | bdd:i-concurrency.feature (« The nearest common manager resolves the fork ») | AUCUNE (pas de commande CLI) |
| 63 | 02.6 | Un délégué qualifie seulement dans sa propre autorité ; la racine propriétaire qualifie toujours et reste le dernier recours. | MUST | signature | `bundle::merge.rs` (`verify_resolution_edition` + `write_covers_labels`) | bdd:i-concurrency.feature (« A delegate cannot resolve a fork outside its perimeter », « The owner resolves as last resort ») | AUCUNE |
| 64 | 02.6 | L'édition résolvante nomme le `prev_hash` gagnant dans `resolves_fork` et son contenu prolonge la branche gagnante. | MUST | aucune | `bundle::manifest.rs` (`resolves_fork`), `bundle::merge.rs` | vecteur:cb2-bundle-concurrency-final.json | AUCUNE |
| 65 | 02.6 | Les vérifieurs acceptent la résolution sous le même contrôle d'autorité qu'une écriture sur ces nœuds. | MUST | aucune | `bundle::merge.rs` (`verify_resolution_edition`) | bdd:i-concurrency.feature | cli (`edition-verify`) |
| 66 | 02.6 | Un vérifieur confronté à un fork non résolu REFUSE de tenir l'une des branches pour canonique pour les écritures déléguées et fait remonter le conflit. | MUST | aucune | `bundle::merge.rs` (`fork_check` → `Error::EditionFork`), `core::concurrency.rs` | bdd:i-concurrency.feature (« An unresolved same-node fork is refused by the verifier ») | cli |
| 67 | 02.6 (passe I) | `prev_hash` d'une fusion épingle le parent de PLUS FAIBLE hash d'édition ; `merges` est ascendant, additif, absent des éditions pré-I dont les hashes de chaîne restent intacts. | MUST | aucune | `bundle::manifest.rs` (`skip_serializing_if`), `bundle::merge.rs` (`verify_merge_edition`) | vecteur:i1-concurrency.json | cli |
| 68 | 02.6 | Un vérifieur conscient des fusions exige deux parents de même hauteur partageant le même grand-parent. | MUST | aucune | `bundle::merge.rs` (`verify_merge_edition`) | bdd:i-concurrency.feature | cli |
| 69 | 02.6 | Il exige des changesets disjoints. | MUST | aucune | `bundle::merge.rs` (`frontier` + intersection), `bundle::state.rs` (`tree_diff`) | vecteur:i1-concurrency.json | cli |
| 70 | 02.6 | Il exige un état fusionné qu'il reproduit octet pour octet (racines de contenu §2.10 et racines gamma §7.10 recommittées). | MUST | aucune | `bundle::merge.rs` + `bundle::bundle.rs` (`verify` recalcule `roots`/`gamma_roots`) | bdd:i-concurrency.feature (« The merged segment recommits its root and count ») | cli |
| 71 | 02.6 | Le changeset d'un parent est son diff par descente de racine §2.10 contre l'ancêtre commun. | MUST | aucune | `bundle::state.rs` (`tree_diff`), `bundle::merge.rs` (`frontier`) | vecteur:h1-merkle.json + bdd:h-merkle.feature | cli (`edition-diff`) |
| 72 | 02.6 | Deux changesets sont disjoints ssi leurs ensembles d'étiquettes de nœuds touchées ne s'intersectent pas. | MUST | aucune | `bundle::merge.rs` (`fork_check`) | vecteur:i1-concurrency.json | cli |
| 73 | 02.6 | Un fichier d'index partagé ne casse pas la disjonction : les lignes fusionnent en 3-way par sid (base = ancêtre commun ; ligne changée prise sur sa branche ; ajouts unionés ; suppressions tiennent, aucune résurrection ; tri existant + JCS → octets identiques pour tout fusionneur). | MUST | aucune | `bundle::merge.rs` (`merge_zone_index`, `merge_self_index`, fusion de lignes par sid) | vecteur:i1-concurrency.json + bdd:i-concurrency.feature (« Two adds in the same folder merge three-way by sid », « A deletion does not resurrect through the merge ») | cli |
| 74 | 02.6 | Le MÊME sid changé sur les deux branches EST un conflit même-nœud — un fork, jamais fusionné. | MUST NOT | aucune | `bundle::merge.rs`, `core::concurrency.rs` (`verify_disjoint_merge`) | bdd:i-concurrency.feature (« The same section modified on both branches is a fork ») | cli |
| 75 | 02.6 | Les écritures déléguées de la branche perdante sont exposées, jamais rejouées en silence. | MUST NOT | aucune | `bundle::merge.rs` (`verify_resolution_edition`, parent alt conservé) | bdd:i-concurrency.feature | cli |
| 76 | 02.6.1 | Une édition v1 a exactement UN acteur : le propriétaire sans mandat, ou le grantee feuille présentant exactement une chaîne valide. | MUST | signature | `core::carriers.rs` (`verify_normal_edition_actor`) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature | client |
| 77 | 02.6.1 | Tout changement de contenu, de structure, de header, de Gamma, de racine et de manifeste d'une édition grantee est expliqué par ce même acteur et cette même chaîne. | MUST | aucune | `core::carriers.rs` (`derive_changeset`, `validate_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 78 | 02.6.1 | Deux chaînes produisent des éditions séparées ; v1 n'a pas d'édition multi-chaînes agrégée. | MUST NOT | aucune | `core::carriers.rs` (`verify_normal_edition_actor`) | bdd:m-delegated-editions.feature | client |
| 79 | 02.6.1 | Pour une fusion ou une résolution, l'acteur/chaîne unique est le publieur et l'autorité de la nouvelle édition ; les acteurs historiques de chaque parent sont conservés, ni réécrits ni usurpés. | MUST | signature | `core::concurrency.rs` (`verify_fork_resolution`), `bundle::merge.rs` | vecteur:cb2-bundle-concurrency-final.json | AUCUNE |
| 80 | 02.6.1 | Le vérifieur DÉRIVE le changeset typé depuis l'état parent épinglé et l'état candidat ; il ne fait jamais confiance à une liste asserée par l'appelant. | MUST NOT | aucune | `core::carriers.rs` (`derive_changeset`) | bdd:m-delegated-editions.feature (« A caller cannot omit or invent a change ») | client |
| 81 | 02.6.1 | Le vérifieur contrôle parent et hauteur attendus, chaque objet changé et retiré, l'opération canonique et l'entrée Gamma correspondantes, les hashes de chaîne et de certificat nécessaires au froid, les racines et le head Gamma recalculés, et la signature de l'acteur. | MUST | aucune | `core::carriers.rs` (`verify_k1c_carriers`) + `bundle::bundle.rs` (`verify`) | vecteur:cb2-draft2-carriers.json | client |
| 82 | 02.6.1 | Un changement inexpliqué, un changement omis, une consommation Gamma en trop, ou un acteur différent dans le nouveau delta invalident l'édition même si tous les hashes et liens sont structurellement valides. | MUST | aucune | `core::carriers.rs` | vecteur:cb2-draft2-carriers.json (32 négatifs `InvalidOperation`) | client |
| 83 | 02.6.1 | Un grantee signe en son nom propre, jamais comme le propriétaire. | MUST NOT | signature | `bundle::manifest.rs` (`verify_delegate_signature` exige `signature.key == leaf.grantee.pubkey`) | bdd:m-delegated-editions.feature (« A normal edition is signed in the actor's own capacity ») | client, gateway |
| 84 | 02.6.1 | Le propriétaire est absent d'une publication déléguée normale, sauf obligation explicitement applicable exigeant un reçu `co_sign` ; ce reçu atteste l'opération et ne fait pas du propriétaire l'acteur de l'édition. | MUST | signature (co_sign) | `core::receipts.rs`, `core::constraints.rs` | bdd:m-delegated-editions.feature + bdd:g-plus-obligations.feature | gateway |
| 85 | 02.6.2 | Tout manifeste `aithos-core: "1.0.0-draft.2"` porte les trois membres signés supplémentaires `operation_ref`, `changeset_ref`, `evidence_ref`. | MUST | aucune | `bundle::manifest.rs` (`build_draft2`, `verify_form`) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature | client, provider-wire |
| 86 | 02.6.2 | `operation_ref` est la référence W1 exacte de cette occurrence de publication normale, de fusion ou de résolution. | MUST | aucune | `core::carriers.rs` (`validate_publication`) | vecteur:cb2-operation-projection.json | client |
| 87 | 02.6.2 | `changeset_ref` adresse par contenu l'UNIQUE changeset clos dérivé pour le candidat relativement à ses états parents applicables. | MUST | aucune | `core::carriers.rs` (`verify_carrier_link`), `bundle::publication.rs` | vecteur:cb2-draft2-carriers.json | client |
| 88 | 02.6.2 | `evidence_ref` adresse par contenu l'ensemble de preuves publiques clos permettant de rejouer les occurrences et l'autorité du publieur sans capacité privée. | MUST | aucune | `core::carriers.rs` (`validate_evidence`) | vecteur:cb2-draft2-carriers.json | client |
| 89 | 02.6.2 | La signature du manifeste couvre les trois références. | MUST | signature | `bundle::manifest.rs` (`unsigned_jcs` inclut les trois champs) | vecteur:cb2-draft2-carriers.json | client |
| 90 | 02.6.2 | Un manifeste draft2 auquel il manque une référence, ou qui en porte une `null`, inconnue ou malformée, échoue fermé. | MUST | aucune | `bundle::manifest.rs` (`verify_form`, `carrier_count == 3`) | bdd:m-delegated-editions.feature (« Manifest profiles fix the K1-B carrier presence ») | client |
| 91 | 02.6.2 | Un manifeste draft1 INTERDIT les trois références et reste octet-identique en vérification historique. | MUST NOT | aucune | `bundle::manifest.rs` (`verify_form`, branche `CORE_VERSION`) | vecteur:cb2-bundle-version-coexistence.json | client |
| 92 | 02.6.2 | Le changeset lie les références parentes applicables, les références d'opérations contenues en ordre causal déterministe, leurs transitions logiques avant/après, et toute conséquence déterministe de store nécessaire à expliquer les octets candidats. | MUST | aucune | `core::carriers.rs` (`derive_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 93 | 02.6.2 | Le changeset inclut les références d'opérations contenues mais EXCLUT l'`operation_ref` de la publication, le hash du manifeste candidat et tout dérivé transitif. | MUST NOT | aucune | `core::carriers.rs` (`validate_changeset`, `validate_publication`) | bdd:m-delegated-editions.feature (« The publication reference and changeset are acyclic ») | client |
| 94 | 02.6.2 | Une transition manquante, un octet inexpliqué, une opération en trop ou une conséquence discordante invalident la publication. | MUST | aucune | `core::carriers.rs` (`validate_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 95 | 02.6.2 | L'ensemble de preuves ne porte QUE du matériel de preuve public : paternité déléguée, certificat SC1 et preuve de session, reçus R2/U1, preuve de catalogue approuvé, présentation de lecture explicitement signée. | MUST | aucune | `core::carriers.rs` (`EvidenceItem`, cinq variantes closes) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature | client |
| 96 | 02.6.2 | Une preuve n'accorde jamais d'autorité par elle-même ; chaque item est recoupé avec son `operation_ref` exact, les faits reconstruits, la chaîne d'autorité, le manifeste candidat et le changeset dérivé. | MUST NOT / MUST | aucune | `core::carriers.rs` (`validate_evidence`) | bdd:m-delegated-editions.feature (« The evidence carrier proves but never authorizes ») | client |
| 97 | 02.6.2 | Contenu privé, identifiants, DK, clés privées et plaintext protégé sont INTERDITS dans l'ensemble de preuves. | MUST NOT | aucune | `core::carriers.rs` (tables closes, `exact_object`) | vecteur:cb2-draft2-carriers.json | client, provider-wire |
| 98 | 02.6.3 | `changeset_ref` et `evidence_ref` sont exacts, non nuls, à deux membres (`aithos-…-core: "1.0.0-draft.1"` + `digest: "sha256:<64 hex minuscules>"`), sans membre supplémentaire. | MUST | aucune | `core::carriers.rs` (`CarrierRef`, `exact_object`) | vecteur:cb2-draft2-carriers.json | client |
| 99 | 02.6.3 | `changeset_ref.digest = C("aithos-core/v1/changeset", JCS(changeset))` et `evidence_ref.digest = C("aithos-core/v1/evidence", JCS(evidence set))`. | MUST | aucune | `core::carriers.rs` (`CHANGESET_DOMAIN`/`EVIDENCE_DOMAIN`), `bundle::publication.rs` | vecteur:cb2-draft2-carriers.json (`domains`) | client, provider-wire |
| 100 | 02.6.3 | Les clés canoniques des sidecars sont `changesets/<64hex>.json` et `evidence/<64hex>.json`. | MUST | aucune | `core::carriers.rs` (`verify_carrier_link` → `directory`), `bundle::lib.rs` (`validate_store_key`) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature | provider-wire |
| 101 | 02.6.3 | La map `files` du manifeste épingle aussi la chaîne d'octets JCS de chaque sidecar sous cette clé avec son SHA-256 nu historique ; la référence à domaine séparé et l'épinglage de fichier sont des contrôles indépendants. | MUST | aucune | `core::carriers.rs` (`verify_carrier_link(files, sidecars)`) | vecteur:cb2-draft2-carriers.json | client |
| 102 | 02.6.3 | Le changeset dérivé a exactement cinq membres de premier niveau. | MUST | aucune | `core::carriers.rs` (`Changeset`, `exact_object`) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature (« A derived changeset has one closed commitment-only table ») | client |
| 103 | 02.6.3 | `height` et `predecessors` égalent les faits de publication. | MUST | aucune | `core::carriers.rs` (`validate_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 104 | 02.6.3 | `operations` égale `contained_operations` dans son ordre causal déjà fixé, sans occurrence dupliquée, et exclut l'occurrence de publication. | MUST | aucune | `core::carriers.rs` | vecteur:cb2-draft2-carriers.json | client |
| 105 | 02.6.3 | Chaque changement a exactement les quatre membres `key_commitment`, `before`, `after`, `operation_ref`. | MUST | aucune | `core::carriers.rs` (`StateChange`) | vecteur:cb2-draft2-carriers.json | client |
| 106 | 02.6.3 | `before` et `after` sélectionnent chacun une variante exacte (`{"state":"absent"}` ou `{"state":"present","byte_commitment":…}`) et diffèrent. | MUST | aucune | `core::carriers.rs` (`StateValue`) | vecteur:cb2-draft2-carriers.json | client |
| 107 | 02.6.3 | `key_commitment` et `byte_commitment` réutilisent les engagements K1.1-B `state-key` et `state-bytes` sur la clé de store canonique exacte et les octets stockés exacts. | MUST | aucune | `core::operation.rs` (`STATE_KEY_DOMAIN`, engagements d'état) | vecteur:cb2-operation-facts-mutation.json | client |
| 108 | 02.6.3 | Les changements se trient par `(key_commitment, operation_ref.occurrence)` en ASCII ascendant exact ; un engagement de clé n'apparaît qu'une fois. | MUST | aucune | `core::carriers.rs` (`validate_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 109 | 02.6.3 | Chaque `operation_ref` d'un changement est membre exact de `operations`. | MUST | aucune | `core::carriers.rs` | vecteur:cb2-draft2-carriers.json | client |
| 110 | 02.6.3 | Quand plusieurs opérations contenues écrivent la même clé de store, le rejeu applique l'ordre de `operations` et l'unique changement nomme le DERNIER écrivain des octets finaux ; les conséquences dérivées (index, racine, wrap, header, Gamma, coffre, rotation) nomment leur dernier écrivain causal au lieu d'allouer une occurrence. | MUST | aucune | `core::carriers.rs` (`derive_changeset`, attribution last-writer) | vecteur:cb2-draft2-carriers.json (5 clés changées, 5 opérations contenues) | client |
| 111 | 02.6.3 | Le changeset est non vide, sauf à la genèse normale. | MUST | aucune | `core::carriers.rs` (`validate_changeset`) | vecteur:cb2-draft2-carriers.json | client |
| 112 | 02.6.3 | Le changeset exclut son propre sidecar, le sidecar de preuve et le manifeste candidat (acyclicité). | MUST NOT | aucune | `core::carriers.rs` | bdd:m-delegated-editions.feature (« Carrier objects are acyclic consequences rather than changeset rows ») | client |
| 113 | 02.6.3 | L'ensemble de preuves publiques a exactement trois membres (`aithos-evidence-core`, `items`, `delegated_counts`). | MUST | aucune | `core::carriers.rs` (`EvidenceSet`) | vecteur:cb2-draft2-carriers.json | client |
| 114 | 02.6.3 | `items` est trié par octets JCS de chaque item complet, sans valeur JCS dupliquée. | MUST | aucune | `core::carriers.rs` (`item_jcs` + tri) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature | client |
| 115 | 02.6.3 | Chaque item sélectionne exactement l'une des cinq tables closes (`authorship`, `session`, `receipt`, `catalog`, `presentation`). | MUST | aucune | `core::carriers.rs` (`EvidenceItem`) | bdd:m-delegated-editions.feature (« Every evidence item selects one exact nested proof table ») | client |
| 116 | 02.6.3 | Le document imbriqué sélectionné valide sous son propre profil exact. | MUST | aucune | `core::carriers.rs` (`verify_authorship`, `verify_session_item`, `verify_receipt_item`, `verify_catalog_item`, `verify_presentation_item`) | vecteur:cb2-draft2-carriers.json | client |
| 117 | 02.6.3 | Une paire de session porte la même référence d'opération et les mêmes clés. | MUST | aucune | `core::carriers.rs` (`verify_session_item`) | vecteur:cb2-session-proof.json | client |
| 118 | 02.6.3 | Un reçu est inclus une fois par contrôle lié à l'opération requis. | MUST | aucune | `core::carriers.rs` (`verify_receipt_item`), `core::receipts.rs` | vecteur:cb2-operation-receipts.json | client |
| 119 | 02.6.3 | Un item de catalogue ne sert plusieurs occurrences d'action que si chaque `catalog_ref` K1.2 sélectionne ses digests complets de catalogue et d'approbation. | MUST | aucune | `core::carriers.rs` (`verify_catalog_item`), `core::catalog.rs` | vecteur:cb2-connector-catalog.json | client |
| 120 | 02.6.3 | `delegated_counts` est la référence D7 exacte de §7.10.1, toujours présente, avec la racine vide quand aucune occurrence déléguée n'existe. | MUST | aucune | `core::carriers.rs` (`validate_delegated_counts`), `core::delegated_counts.rs` | vecteur:cb2-delegated-counts.json | client |
| 121 | 02.6.3 | Un item de preuve inutilisé, non corrélé ou dupliqué invalide l'édition. | MUST | aucune | `core::carriers.rs` (`validate_evidence`) | vecteur:cb2-draft2-carriers.json | client |
| 122 | 02.6.3 | La paternité publique déléguée a exactement les dix membres montrés. | MUST | aucune | `core::carriers.rs` (`verify_authorship` + `exact_object`) | vecteur:cb2-draft2-carriers.json | client |
| 123 | 02.6.3 | La paternité publique n'est émise QUE pour une mutation grantee d'une section `public`. | MUST | aucune | `core::carriers.rs` (`inventory.authorship`), `bundle::grants.rs` (`sign_public_authorship` sur `public` seul) | bdd:m-delegated-editions.feature (« Public delegated authorship travels with the edition ») | gateway |
| 124 | 02.6.3 | `content_hash` est le SHA-256 des octets exacts du corps public stocké. | MUST | aucune | `core::carriers.rs` (`sha256_prefixed(body)`) | vecteur:cb2-draft2-carriers.json | client |
| 125 | 02.6.3 | `sid`, sujet, référence d'opération, hauteur/prédécesseurs d'édition, `authorized_via` et clé égalent les faits d'opération/publication reconstruits et l'autorité W1. | MUST | aucune | `core::carriers.rs` (`verify_authorship`) | vecteur:cb2-draft2-carriers.json | client |
| 126 | 02.6.3 | La clé du grantee signe le JCS de l'objet avec le `sig` de premier niveau omis. | MUST | signature | `core::carriers.rs` (`verify_omitted_signature`), `bundle::grants.rs` (`sign_public_authorship`) | vecteur:cb2-draft2-carriers.json | gateway |
| 127 | 02.6.3 | L'objet de paternité ne contient ni manifeste candidat ni digest de carrier (le manifeste peut donc l'engager sans cycle). | MUST NOT | aucune | `core::carriers.rs` (liste de membres exacte) | bdd:m-delegated-editions.feature (« Public grantee authorship has one acyclic signed table ») | client |
| 128 | 02.6.3 | Les signatures publiques du propriétaire restent sa preuve de contenu historique ; la paternité `circle` reste dans le blob scellé et Gamma ; `self` n'a AUCUN document de paternité public. | MUST | aucune / descellement (circle) | `bundle::bundle.rs` (`SectionRow.sig` public, `{md,sig}` circle, `SelfSection` non signée), `bundle::grants.rs` | bdd:m-delegated-editions.feature (« Self delegated changes reveal opaque state relations only ») | cli, gateway |
| 129 | 02.6.3 | Une requête Gamma opposable utilise la présentation signée exacte à neuf membres. | MUST | signature | `core::carriers.rs` (`verify_presentation_item`) | vecteur:cb2-draft2-carriers.json + bdd:m-delegated-editions.feature (« An opposable Gamma presentation has one signed result table ») | cli (`log-query`), gateway |
| 130 | 02.6.3 | `entries` contient les objets d'entrée Gamma complets sélectionnés dans leur ordre causal/segment vérifié, sans id dupliqué ; vide est valide. | MUST | aucune | `core::carriers.rs` (`verify_presentation_item`) | vecteur:cb2-draft2-carriers.json | cli |
| 131 | 02.6.3 | Bundle ré-exécute la requête canonique sur l'historique épinglé par `source_head` et exige exactement les mêmes entrées. | MUST | aucune | `core::carriers.rs` (comparaison `gamma_result`), `bundle::log.rs` | vecteur:cb2-operation-facts-read.json | cli |
| 132 | 02.6.3 | Sujet, source head, digest de requête et `at` égalent les faits K1.2-R-B et la projection W1. | MUST | aucune | `core::carriers.rs`, `core::operation.rs` | vecteur:cb2-operation-facts-read.json | cli |
| 133 | 02.6.3 | Cette présentation n'alloue AUCUN kind Gamma ni seconde occurrence. | MUST NOT | aucune | `core::carriers.rs`, `core::gamma_v2.rs` | bdd:f-gamma.feature + bdd:m-delegated-editions.feature | cli |
| 134 | 02.6.3 | Un manifeste draft2 porte les trois références exactes au premier niveau et sa signature existante les couvre avec seulement `signature.value` vidé. | MUST | signature | `bundle::manifest.rs` (`unsigned_jcs`) | vecteur:cb2-draft2-carriers.json | client |
| 135 | 02.6.3 | L'`operation_ref` de publication est reconstruit seulement après que le changeset achevé fixe le `changeset_ref` ; il ne contient ni le digest de preuve ni le manifeste candidat. | MUST | aucune | `core::carriers.rs` (`validate_publication`) | bdd:m-delegated-editions.feature (« The publication reference and changeset are acyclic ») | client |
| 136 | 02.6.3 | Forme de carrier malformée, décalage digest/chemin/épinglage, échec d'ordre ou de doublon, changement omis ou inventé, item inexpliqué, décalage de signature ou de vue croisée de paternité/présentation retournent `Error::InvalidOperation(String)`. | MUST | aucune | `core::carriers.rs` (`invalid()` → `Error::InvalidOperation`) | vecteur:cb2-draft2-carriers.json (32 négatifs) | client |
| 137 | 02.6.3 | Forme de manifeste signé malformée ou présence/profil incorrect retournent l'historique `Error::InvalidDidDocument(String)`. | MUST | aucune | `bundle::manifest.rs` (`verify_form`) | vecteur:cb2-draft2-carriers.json (5 négatifs) | client |
| 138 | 02.6.3 | Aucun carrier n'est accepté comme autorité et aucun échec n'émet ni ne publie de manifeste candidat. | MUST NOT | aucune | `bundle::publication.rs`, `core::carriers.rs` | bdd:m-delegated-editions.feature (« K1-C carrier defects fail closed before publication ») | client |
| 139 | 02.7 | Le manifeste épingle `gamma_head` = SHA-256 de la dernière entrée gamma. | MUST | aucune | `bundle::manifest.rs` (`gamma_head`), `core::gamma.rs` (`head`) | vecteur:f1-gamma-chain.json | cli, client |
| 140 | 02.7 | Une édition et son head gamma bougent ensemble. | MUST | aucune | `bundle::bundle.rs` (`publish_artifacts`, `verify`) | bdd:d-bundle.feature + bdd:f-gamma.feature | cli |
| 141 | 02.7 | Un vérifieur contrôle que le `gamma_ref` de CHAQUE section se résout dans le log. | MUST | aucune | NON — aucun champ `gamma_ref` n'existe dans `ZoneIndex`/`SelfIndex` ni ailleurs dans le code | RIEN | AUCUNE |
| 142 | 02.7 | Un vérifieur contrôle que le head correspond. | MUST | aucune | `bundle::bundle.rs` (`verify` : `gamma::head(entries) == latest.gamma_head`) | bdd:d-bundle.feature | cli (`edition-verify`) |
| 143 | 02.8 | Dans `self`, noms, titres, tags et liens parent/enfant vivent À L'INTÉRIEUR du chiffré. | MUST | descellement (lecture) / aucune (constat) | `bundle::bundle.rs` (`SelfIndex`/`SelfRow` sans métadonnée, `SelfSection` scellée) | bdd:d-bundle.feature (« Self is a flat sea of opaque blobs ») | cli |
| 144 | 02.8 | Chaque dossier `self` a un petit descripteur scellé sous sa propre clé listant `{name, children:[sids]}`. | MUST | descellement | `bundle::bundle.rs` (`write_desc`/`read_desc`, `self_add_child`) | bdd:d-bundle.feature | cli |
| 145 | 02.8 | Un lecteur autorisé reconstruit exactement le sous-arbre qu'il peut ouvrir, de haut en bas depuis le nœud le plus profond qu'il détient, et rien d'autre. | MUST | descellement | `bundle::bundle.rs` (`self_collect_sections`), `bundle::grants.rs` (`find_self_with_zone_key`) | vecteur:cb2-bundle-authority-flows.json | cli, gateway |
| 146 | 02.8 | Headers et cibles gamma utilisent des sid-paths, si bien qu'octroyer ou éditer un nœud `self` ne fuit aucune structure. | MUST | aucune | `bundle::grants.rs` (`hdr_file(Zone::Self_)`), `bundle::log.rs` | vecteur:cb2-bundle-authority-flows.json | gateway |
| 147 | 02.8 | Sur `self`, les périmètres `dir=` et `tag=` sont opposables en LECTURE mais NON vérifiables pour l'ÉCRITURE ; les périmètres d'écriture `self` utilisent `id=` ou un grant de zone. | MUST | aucune | `bundle::structure.rs` (refus des revendications `dir`/`tag` d'affichage sur `self`), `bundle::grants.rs` (« self tag delivery requires an exact id ») | bdd:n-structural-mutations.feature (« A self structural mutation uses zone authority or exact opaque SIDs ») | gateway |
| 148 | 02.8 | La vérification sans clé d'une mutation `self` n'utilise que des preuves d'état opaques : création = absence antérieure + inclusion ultérieure d'un SID préalloué autorisé (ou autorité zone append/write) ; édition = remplacement du même SID ; suppression = son retrait. | MUST | aucune | `core::operation.rs` (K1.2-M-B) | vecteur:cb2-operation-facts-mutation.json (13 cas domaine/verbe) | client |
| 149 | 02.8 | Cette preuve lie engagements antérieur et suivant, opération, entrée Gamma, chaîne, racines et édition SANS exposer nom, chemin, titre, tag, corps, relation de dossier ni clé. | MUST NOT | aucune | `core::operation.rs` | vecteur:cb2-operation-facts-mutation.json (15 négatifs `InvalidStateFact`) + bdd:f-gamma.feature | client |
| 150 | 02.8 | Une assertion signée sans preuve rattachée à l'état antérieur est insuffisante. | MUST | aucune | `core::operation.rs` (`verify_state_fact`) | vecteur:cb2-operation-facts-mutation.json | client |
| 151 | 02.8 | K1.1-B représente un état logique présent par l'engagement exact d'ensemble d'objets protégés de §4.5.1, et l'absence par la variante close `{"state":"absent"}`. | MUST | aucune | `core::operation.rs`, `core::carriers.rs` (`StateValue`) | vecteur:cb2-operation-facts-mutation.json | client |
| 152 | 02.8 | Le document de fait d'état ne contient que des engagements à domaine séparé sur des clés de store canoniques et des octets stockés exacts ; ni cible en clair, ni clé de store, ni nouveau sidecar public. | MUST NOT | aucune | `core::operation.rs` (`STATE_KEY_DOMAIN`) | vecteur:cb2-operation-facts-mutation.json | client |
| 153 | 02.8 | La table de preuve propre à la famille lie la cible opaque exactement à cet ensemble d'objets engagé AVANT toute production d'engagement d'opération. | MUST | aucune | `core::operation.rs` | vecteur:cb2-operation-facts-mutation.json | client |
| 154 | 02.8 | K1.2-M-B engage le SID cible et les tableaux de SID parents applicables DANS le document de faits d'opération protégé, la projection publique ne portant que `facts_ref`. | MUST | aucune | `core::operation.rs` (`facts_ref`, `validate_selected_facts`) | vecteur:cb2-operation-facts-mutation.json + vecteur:cb2-operation-projection.json | client |
| 155 | 02.8 | Ces tableaux ne rendent JAMAIS `dir=` ni `tag=` vérifiables comme autorité d'écriture dans `self`. | MUST NOT | aucune | `core::operation.rs`, `bundle::structure.rs` | bdd:n-structural-mutations.feature | client |
| 156 | 02.8 | Pour une suppression ou un déplacement de dossier `self`, la preuve opaque couvre l'ensemble exact des engagements affectés et l'autorité de chacun (suppression : couverture + retrait de chaque descendant ; déplacement : édition source, append/write destination, et chaque conséquence de rotation, rewrap et re-chiffrement), sans exposer relations, noms ni contenus. | FUTUR | aucune | NON | RIEN — spec : « Its additive signed encoding is reserved for independent CB2 vectors. » | AUCUNE |
| 157 | 02.9 | Une vue par tag `…/t/<tag>` est un nœud d'ancre dérivé de son dossier et n'octroie RIEN par dérivation descendante. | MUST | descellement | `core::derive.rs` (`tag_label`), `bundle::grants.rs` | bdd:b-derivation.feature (« A folder-local tag view is its own lock ») | cli |
| 158 | 02.9 | Les sections entrent dans une vue par WRAP : le gestionnaire du dossier scelle `wrap(K_section)` sous la clé d'ancre quand une section sous ce dossier porte le tag. | MUST | descellement | `bundle::grants.rs` (`deliver_entry` avec `GrantSelector::Tag`), `bundle::structure.rs` (`structural_sync_metadata_tag_wraps`) | vecteur:h1-merkle.json (wraps) + bdd:n-structural-mutations.feature | gateway |
| 159 | 02.9 | La création du wrap passe par un contrôle de paternité fail-closed (§2.2). | MUST | aucune | PARTIEL — `bundle::structure.rs` (`structural_section_gate` avant synchronisation) ; aucun contrôle rétroactif de l'entrée Gamma de mutation de tag | bdd:n-structural-mutations.feature | gateway |
| 160 | 02.9 | Une vue racine de zone couvre toute la zone ; une vue locale à un dossier ne couvre que ce sous-arbre. | MUST | descellement | `bundle::grants.rs` (`sections_under` borné par la chaîne d'ancre) | vecteur:b2-derivation.json + bdd:b-derivation.feature | gateway |
| 161 | 02.9 | Une seule ligne de header sur l'ancre est le grant O(1) « lire ce qui est taggé X sous ce dossier, maintenant et à l'avenir ». | MUST | descellement | `bundle::grants.rs` (`GrantSelector::Tag`, `deliver_zone_line`) | vecteur:cb2-bundle-authority-flows.json | cli (`grant`), gateway |
| 162 | 02.9 | Le renommage est gratuit : il édite une ligne d'index ou un descripteur, ne re-clé rien, ne déplace aucun octet. | MUST | aucune (index) / descellement (descripteur self) | `bundle::bundle.rs` (`rename_folder`), `bundle::structure.rs` | bdd:b-derivation.feature (« Renaming never re-keys ») | cli |
| 163 | 02.9 | Le déplacement EST une rotation : nouvelle DK' de M (§03.4) + wrap montant posté sous le NOUVEAU parent, survivants re-scellés. | MUST | descellement | `bundle::revoke.rs` (`move_folder`), `bundle::structure.rs` (`structural_move_folder`) | vecteur:g3-move.json + bdd:n-structural-mutations.feature | cli (`move`) |
| 164 | 02.9 | Les détenteurs de l'ancien parent sont coupés cryptographiquement ; les détenteurs du nouveau parent dérivent par le wrap. | MUST | descellement | `bundle::revoke.rs` (`move_folder`) | vecteur:g3-move.json (`moved_new_dk_hex`, `wrap_cipher_hex`) | cli |
| 165 | 02.9 | Le coût est proportionnel aux headers octroyés de M (+ re-chiffrement de son sous-arbre si grade incident). | MUST | descellement | `bundle::revoke.rs` (re-scellage des survivants + re-chiffrement du sous-arbre) | vecteur:g3-move.json | cli |
| 166 | 02.9 | La variante paresseuse de re-chiffrement est tolérée comme hygiène (§06.8). | MAY | descellement | NON (l'implémentation est systématiquement eager) | RIEN | AUCUNE |
| 167 | 02.9 | Un mandat accordé SUR M lui-même (ligne de header directe, re-scellée comme survivant) garde clé ET couverture à la nouvelle adresse de M. | MUST | descellement | `bundle::revoke.rs` (`survivors` re-scellés au nouveau nœud) | vecteur:g3-move.json (`containment`) | cli |
| 168 | 02.9 | Un mandat sur l'ANCIEN parent perd le sous-arbre de M aussi au moment de la vérification (politique et physique concordent). | MUST | aucune | `core::path.rs` (`covers`, confinement nodal §04.2), `core::mandate.rs` | vecteur:g3-move.json (`containment`) | cli |
| 169 | 02.9 | Le déplacement re-parente la ligne d'index de M ; les sids étant stables, chaque label de dérivation sous M est inchangé et seule la clé de M est fraîche. | MUST | aucune / descellement | `bundle::revoke.rs` (`move_folder`) | vecteur:g3-move.json | cli |
| 170 | 02.9 | Le header rotaté de M (et chaque corps re-chiffré) lie le NOUVEAU chemin canonique de M. | MUST | descellement | `bundle::revoke.rs` (`Header::build_at(new_node)`, `put_blob_v(new_section)`) | vecteur:g3-move.json (`line_aad_hex`, `blob_aad_hex`) | cli |
| 171 | 02.9 | L'ancien fichier de header reste en place, trace immuable des versions scellées à l'ancienne adresse. | MUST | aucune | `bundle::revoke.rs` (aucune suppression de `hdr_file(old_node)`) | bdd:h-merkle.feature (« A moved folder proves at its new address and dies at the old one ») | cli |
| 172 | 02.9 | Un déplacement ne traverse JAMAIS les zones. | MUST NOT | aucune | `bundle::revoke.rs` / `bundle::structure.rs` (`let zone = Zone::Circle` en dur) | vecteur:cb2-bundle-structure-vault.json | cli |
| 173 | 02.9 | Un déplacement ne cible JAMAIS M lui-même ni un descendant (aucun cycle). | MUST NOT | aucune | `bundle::revoke.rs` / `bundle::structure.rs` (contrôle de préfixe de chaîne) | bdd:n-structural-mutations.feature (« move into the node's own descendant ») | cli |
| 174 | 02.9 | Un déplacement n'atterrit JAMAIS à côté d'un frère de même nom. | MUST NOT | aucune | `bundle::revoke.rs` / `bundle::structure.rs` (collision de nom) | bdd:n-structural-mutations.feature (« destination sibling name collision ») | cli |
| 175 | 02.9 | Le déplacement coupe par conséquence, pas par intention : couper quelqu'un relève de la révocation (§06). | MUST | descellement | `bundle::revoke.rs` (les survivants sont exactement l'ensemble de lignes précédent) | vecteur:g1-revocation.json / vecteur:g3-move.json | cli |
| 176 | 02.10 | Le manifeste de chaque édition épingle une racine d'état par zone, plus le coffre, à côté de `gamma_head`. | MUST | aucune | `bundle::manifest.rs` (`roots`), `bundle::state.rs` (`state_tree`) | vecteur:h1-merkle.json + bdd:h-merkle.feature (« The manifest pins four state roots beside the flat pins ») | cli, client |
| 177 | 02.10 | `H_leaf(p) = BLAKE3("aithos-core/v1/mk-leaf" ‖ 0x00 ‖ p)`. | MUST | aucune | `core::merkle.rs` (`h_leaf`) | vecteur:h1-merkle.json | cli, wasm |
| 178 | 02.10 | `H_node(l, r) = BLAKE3("aithos-core/v1/mk-node" ‖ 0x00 ‖ l ‖ r)`. | MUST | aucune | `core::merkle.rs` (`h_node`) | vecteur:h1-merkle.json | cli |
| 179 | 02.10 | `mroot(list)` est l'arbre binaire équilibré `H_node` sur la liste triée ; `32×0x00` si vide. | MUST | aucune | `core::merkle.rs` (`mroot`, `EMPTY_ROOT`) | vecteur:h1-merkle.json + bdd:h-merkle.feature (« An empty flat zone pins the empty root ») | cli |
| 180 | 02.10 | Récursion `mroot` : `mroot([x]) = x`, sinon `H_node(mroot(left), mroot(right))` avec `left` = les ⌈n/2⌉ premiers (split left-heavy). Aucune duplication, aucune promotion. | MUST | aucune | `core::merkle.rs` (`div_ceil`) | vecteur:h1-merkle.json + test:core/src/merkle.rs#left_heavy_odd_split | cli |
| 181 | 02.10 | `header_hash(N) = BLAKE3(JCS(header.json))` si N a déjà été octroyé, sinon `32×0x00`. | MUST | aucune | `bundle::state.rs` (`header_hash_at`) | vecteur:h1-merkle.json | cli |
| 182 | 02.10 | Nœud section : `H_leaf(JCS(index_row) ‖ header_hash)`. | MUST | aucune | `bundle::state.rs` (`folder_node`, branche sections) | vecteur:h1-merkle.json | cli |
| 183 | 02.10 | Nœud vue par tag : `H_leaf("t/"+tag ‖ header_hash ‖ mroot(wraps, par sid de section))`. | MUST | aucune | `bundle::state.rs` | vecteur:h1-merkle.json | cli |
| 184 | 02.10 | Nœud dossier : `H_leaf(JCS(folder_row) ‖ header_hash ‖ mroot(hashes des enfants))`. | MUST | aucune | `bundle::state.rs` (`folder_node`) | vecteur:h1-merkle.json | cli |
| 185 | 02.10 | Racine de zone : le hash du nœud du dossier racine, avec le label littéral `"z/"+zone` à la place de la ligne d'index. | MUST | aucune | `bundle::state.rs` (préfixe `z/<zone>`) | vecteur:h1-merkle.json | cli |
| 186 | 02.10 | Les enfants d'un dossier se trient par `(kind, key)` avec l'ordre `"d" < "s" < "t"` ; `key` = sid pour d/s, la chaîne du tag pour t. | MUST | aucune | `bundle::state.rs` (clés de tri `d\0`, `s\0`, `t\0`) | vecteur:h1-merkle.json | cli |
| 187 | 02.10 | Wraps de vue par tag : `mroot` sur `H_leaf(section_sid ‖ 0x00 ‖ BLAKE3(JCS(wrap)))`, triés par sid de section. | MUST | aucune | `bundle::state.rs` | vecteur:h1-merkle.json | cli |
| 188 | 02.10 | `self` et le coffre sont PLATS : feuilles `H_leaf(JCS(index_row) ‖ header_hash)` triées par sid (coffre : par label de nœud), `root = mroot(leaves)` directement, sans charge utile de dossier. | MUST | aucune | PARTIEL — `bundle::state.rs` : `self_build` replie `32×0x00` au lieu du vrai `header_hash` (dette assumée en commentaire) ; `vault_build` utilise `path ‖ 0x00 ‖ object_hash` au lieu de `JCS(index_row) ‖ header_hash` | vecteur:h1-merkle.json (forme plate) ; le pli du header `self` n'est prouvé nulle part | cli |
| 189 | 02.10 | Une preuve `self` ne révèle que des hashes frères, jamais la structure. | MUST NOT | aucune | `bundle::state.rs` (`prove_self`) | bdd:h-merkle.feature (« A self proof reveals sibling hashes only, never structure ») | cli |
| 190 | 02.10 | Les racines chevauchent le manifeste À CÔTÉ des épinglages de fichiers plats (additif) ; les épinglages plats continuent de couvrir le rollback d'octets des blobs `self` scellés. | MUST | aucune | `bundle::manifest.rs` (`roots` + `files`), `bundle::bundle.rs` (`all_pinned_files`) | bdd:h-merkle.feature + bdd:d-bundle.feature (« A tampered file fails the edition ») | cli |
| 191 | 02.10 | Fil de preuve v1 : le vérifieur part des octets RÉCLAMÉS, applique les étapes ordonnées `{"node":{"side","hash"}}` puis `{"wrap":{"pre","post"}}`. | MUST | aucune | `core::merkle.rs` (`ProofStep`, `run_proof`, `verify_proof`) | vecteur:h1-merkle.json | cli (`prove`) |
| 192 | 02.10 | La preuve vérifie SSI le `cur` final égale la racine de zone épinglée. | MUST | aucune | `core::merkle.rs` (`verify_proof`) | bdd:h-merkle.feature (« A tampered index row fails its proof ») | cli |
| 193 | 02.10 | La séparation de domaine est la défense anti-splicing : un hash de nœud fourni là où une feuille est attendue (ou l'inverse) change la chaîne de domaine et la racine meurt. | MUST | aucune | `core::merkle.rs` (`LEAF_DOMAIN`/`NODE_DOMAIN`) | bdd:h-merkle.feature (« A leaf can never be spliced as an interior node », « An interior node can never pose as a leaf ») + test:core/src/merkle.rs | cli |
| 194 | 02.10 | Un lecteur vérifie n'importe quelle ligne, header ou sous-arbre contre le manifeste signé en O(log n), sans jamais chercher d'index ; n'importe quel miroir peut servir ces preuves sans être digne de confiance. | MUST | aucune | `bundle::state.rs` (`prove_section`, `prove_self`), `core::merkle.rs` | vecteur:h1-merkle.json + bdd:h-merkle.feature | cli, provider-wire |
| 195 | 02.10 | Un octroi ou une rotation remonte naturellement le chemin du nœud jusqu'à la racine. | MUST | aucune | `bundle::state.rs` (header replié dans le hash du nœud) | bdd:h-merkle.feature (« A grant bumps the granted node's path to the root ») | cli |
| 196 | 02.10 | Deux éditions se diffèrent par descente de racine (sync en O(changed × log n)). | MUST | aucune | `bundle::state.rs` (`tree_diff`) | bdd:h-merkle.feature (« A one-section change diffs to exactly its path », « Identical editions diff empty ») | cli (`edition-diff`) |
| 197 | 02.10 | Une preuve Merkle prouve l'INCLUSION dans une édition signée, jamais la fraîcheur ; la péremption reste bornée par `freshness` (§04.4) et les chaînes édition + gamma. | MUST NOT | aucune | `core::constraints.rs` (famille `freshness`) | vecteur:fplus-constraints.json | cli, gateway |
| 198 | 02.11 | Le propriétaire signe son contenu avec une plume unique, `content_sign` ; l'audience vit dans la charge signée, jamais dans la clé. | MUST | signature | `core::keys.rs` (`content_sign`), `bundle::bundle.rs` (`owner_content_sig`) | vecteur:a1-genesis.json | cli |
| 199 | 02.11 | Une signature de contenu propriétaire couvre TOUJOURS le JCS de `{zone, path, sid, body_hash}`. | MUST | signature | `bundle::bundle.rs` (`owner_content_sig`) | RIEN (aucun vecteur ni test ne fige ces octets) | cli |
| 200 | 02.11 | Un vérifieur rejette toute signature propriétaire dont le placement embarqué ne correspond pas à l'endroit où l'objet se trouve réellement (fail-closed). | MUST | aucune | NON — `bundle::bundle.rs` (`verify`) ignore `row.sig` ; aucune fonction de vérification de signature de contenu propriétaire n'existe | RIEN | AUCUNE |
| 201 | 02.11 | `public` : la signature voyage dans la ligne d'index. | MUST | signature (émission) | `bundle::bundle.rs` (`SectionRow.sig` pour `Zone::Public`) | bdd:d-bundle.feature | cli, provider-wire |
| 202 | 02.11 | `public` : la signature PEUT voyager en sidecar avec le markdown brut. | MAY | aucune | NON | RIEN | AUCUNE |
| 203 | 02.11 | `circle` : la signature fait partie du plaintext du blob scellé ; seuls les lecteurs de la section peuvent la vérifier. | MUST | descellement | `bundle::bundle.rs` (`{ "md": body, "sig": sig }`) | RIEN (le champ est écrit, jamais revérifié) | cli |
| 204 | 02.11 | `self` : le contenu n'est JAMAIS signé — déniable par défaut. | MUST NOT | aucune | `bundle::bundle.rs` (`SelfSection` sans champ `sig`, commentaire §02.11 explicite) | bdd:d-bundle.feature | cli |
| 205 | 02.11 | Pour `self`, l'intégrité vient de l'AEAD + l'ancrage gamma, et l'attribution de paternité de l'entrée gamma signée par le propriétaire sur le sid opaque. | MUST | aucune (vérification) / signature (émission) | `core::seal.rs`, `bundle::log.rs` (`log_owner_mutation`) | vecteur:f1-gamma-chain.json | cli |
| 206 | 02.11 | Divulgation sélective : révéler UNE clé de section permet à quiconque de vérifier chiffré → entrée gamma signée → chaîne d'édition, prouvant paternité et date pour cette section seule. | MAY | descellement | NON | RIEN | AUCUNE |
| 207 | 02.11 | Un contenu produit par un agent n'est JAMAIS signé avec des clés propriétaire, dans aucune zone ; l'agent signe son entrée gamma avec sa propre paire de clés sous sa chaîne. | MUST NOT | signature | `core::gamma.rs` (entrée déléguée), `bundle::grants.rs` (`row.sig = None` sur toute écriture grantee) | vecteur:cb2-bundle-authority-flows.json + bdd:m-delegated-editions.feature | gateway |
| 208 | 02.11 | Pour `public`, une mutation produite par un grantee porte la signature du grantee liée au hash de contenu, au SID, à l'opération canonique, à l'édition et à l'`authorized_via` feuille ; Gamma et le manifeste engagent cette preuve. | MUST | signature | `bundle::grants.rs` (`sign_public_authorship`), `core::carriers.rs` (`verify_authorship`) | vecteur:cb2-draft2-carriers.json | gateway |
| 209 | 02.11 | La vérification à froid distingue paternité propriétaire et paternité déléguée sans clé privée. | MUST | aucune | `bundle::grants.rs` (`verify_public_authorship`) | bdd:m-delegated-editions.feature (« Public delegated authorship travels with the edition ») | client, provider-wire |
| 210 | 02.11 | La présentation produit PEUT montrer le grantee et sa chaîne d'autorisation, mais ne DOIT PAS étiqueter ce contenu comme directement signé par le propriétaire. | MUST NOT | aucune | `bundle::grants.rs` (`row.sig` et `row.authorship` mutuellement exclusifs, contrôlés dans `verify_public_authorship`) | bdd:m-delegated-editions.feature | client |
| 211 | 02.12 (G-B) | Une mutation est calculée contre un instantané immuable dans un overlay, soumise au verdict pur de Core, réduite à un write-set déterministe, et seulement ensuite committée. | MUST | aucune | `bundle::bundle.rs` (`transaction`), `bundle::lib.rs` (`MemStore.overlay`, `FsStore` staging) | vecteur:cb2-bundle-boundaries.json + bdd:d-bundle.feature | cli, client |
| 212 | 02.12 | Les helpers métier n'écrivent JAMAIS d'objet canonique directement. | MUST NOT | aucune | `bundle::bundle.rs` (`write_object` interne + `transaction`) | vecteur:cb2-bundle-boundaries.json | cli |
| 213 | 02.12 | Chaque transaction a UN point de linéarisation logique, après validation Core. | MUST | aucune | `bundle::lib.rs` (`commit_transaction`) | bdd:d-bundle.feature (« one deterministic write-set advances content, roots, manifest and Gamma ») | cli |
| 214 | 02.12 | Un rejet ou un échec avant ce point laisse le bundle canonique octet pour octet inchangé : ni manifeste ni head Gamma avancés, ni index/header/wrap/blob partiel, ni orphelin. | MUST | aucune | `bundle::lib.rs` (`rollback_transaction`), `bundle::bundle.rs` (`transaction`) | bdd:d-bundle.feature (Scenario Outline, 12 frontières MemStore/FsStore) + vecteur:cb2-bundle-boundaries.json | cli |
| 215 | 02.12 (K1-B) | Une action de connecteur ou une inférence met en scène son Gamma et ses preuves dans ce même overlai non canonique, obtient l'autorisation pré-effet, réalise l'effet externe, ajoute la preuve d'usage post-effet applicable, et seulement alors atteint le point de linéarisation local. | MUST | aucune | `bundle::bundle.rs` (`transaction`), `bundle::log.rs`, `core::constraints.rs` | bdd:o-connector-classes-vault.feature — PROXY (`cb5_catalog_result`/`cb6_result`/`cb7_result` via `OnceLock` dans `bundle/tests/cucumber.rs`) | gateway |
| 216 | 02.12 | Le contrôle pré-effet est la permission d'exécuter, pas une admission dans l'historique accepté. | MUST | aucune | `core::operation.rs`, `core::constraints.rs` | PROXY (`cb7_result`) | gateway |
| 217 | 02.12 | L'acceptation à l'append final et le rejeu à froid reçoivent les mêmes faits publics achevés et utilisent la même sémantique Core. | MUST | aucune | `core::gamma_replay.rs`, `core::carriers.rs` | vecteur:cb2-gamma-v2-replay.json | client |
| 218 | 02.12 | Si l'exécution est refusée ou si l'effet externe échoue, l'overlay est jeté et le bundle canonique reste octet-identique. | MUST | aucune | `bundle::bundle.rs` (`transaction`, branche `Err`) | bdd:d-bundle.feature + PROXY (`cb7_result`) | cli, gateway |
| 219 | 02.12 | Si le processus perd son état après l'effet externe mais avant le commit local, le retry exige une réconciliation côté connecteur ; AUCUN objet canonique `pending` ni seconde occurrence inférée n'est créé. | MUST NOT | aucune | `bundle::lib.rs` (`validate_store_key` n'admet aucun objet `pending`) | PROXY (`cb7_result`, scénario « the runtime recovers without accepted history for that occurrence ») | gateway |
| 220 | 02.12 | `MemStore` committe en remplaçant atomiquement son état canonique. | MUST | aucune | `bundle::lib.rs` (`MemStore` overlay → `objects`) | bdd:d-bundle.feature (exemples `MemStore`) | cli |
| 221 | 02.12 | `FsStore` prépare dans un staging récupérable physiquement HORS du répertoire du bundle canonique et utilise un mécanisme de linéarisation récupérable local au Store. | MUST | aucune | PARTIEL — `bundle::lib.rs` : staging sous `<root>/.aithos-generations` avec pointeur `<root>/.aithos-current`, donc hors du NAMESPACE canonique mais à l'intérieur du répertoire racine | bdd:d-bundle.feature (exemples `FsStore`) + vecteur:cb2-bundle-boundaries.json | cli |
| 222 | 02.12 | Toute métadonnée interne de génération, marqueur de commit ou référence est hors du namespace canonique, de la disposition §2.3, du manifeste, des épinglages et du fil signé. | MUST NOT | aucune | `bundle::lib.rs` (préfixe `.aithos-` filtré du listage et rejeté par `validate_store_key`) | vecteur:cb2-bundle-boundaries.json | cli |
| 223 | 02.12 | Lecteurs, réouverture et récupération observent soit l'ancien état complet, soit le nouvel état complet, jamais un mélange. | MUST | aucune | `bundle::lib.rs` (`canonical_base`, `read_pointer`, `recover_transaction`) | bdd:d-bundle.feature (« no reader or reopen observes an individual file replacement or partial edition ») | cli |
| 224 | 02.12 | Le contrat n'exige AUCUN appel système multi-fichiers non portable. | MUST NOT | aucune | `bundle::lib.rs` (un seul `rename` de pointeur + `sync_all`) | vecteur:cb2-bundle-boundaries.json | cli |
| 225 | 02.12 | Un crash ou un acquittement perdu à la frontière de linéarisation peut exiger de découvrir l'issue committée depuis le manifeste/head canonique ; le scratch est nettoyé ou résolu de façon récupérable. | MUST | aucune | `bundle::lib.rs` (`recover_transaction`, marqueurs de génération) | bdd:d-bundle.feature (« a crash or lost acknowledgement at that point resolves… ») | cli |
| 226 | 02.12 | La SEULE exception d'orphelin est le préchargement explicite D3 de blobs de publication opaques adressés par contenu, hors transaction locale ; ces blobs ne sont jamais un état canonique atteignable. | MUST NOT | aucune | `bundle::publication.rs`, `bundle::remote.rs` | vecteur:p7-store-publication.json + vecteur:p7-bundle-packages.json | client, provider-wire |
| 227 | 02.12 (G-D) | Bundle est la SEULE frontière d'assemblage publique : elle décode et valide disposition, version, hashes, références, atteignabilité et forme des preuves, puis passe des artefacts publics typés au vérifieur sémantique pur de Core. | MUST | aucune | `bundle::*` (I/O) vs `core::*` (`#![forbid(unsafe_code)]`, pur) | bdd:m-delegated-editions.feature (« Layout verification feeds one pure Core semantic verdict ») | client |
| 228 | 02.12 | L'append-time et le cold-time alimentent le même vérifieur avec les mêmes faits et obtiennent le même verdict. | MUST | aucune | `core::carriers.rs`, `core::gamma_replay.rs` | vecteur:p8-cold-roundtrip.json + vecteur:cb2-gamma-v2-replay.json | client |
| 229 | 02.12 | Exporter une édition dans un `MemStore` ou un `FsStore` neuf et la rouvrir SANS capacité privée propriétaire ni grantee DOIT suffire à vérifier l'historique propriétaire et délégué. | MUST | aucune | `bundle::publication.rs` (`export_keyless`/`import_keyless`), `bundle::bundle.rs` (`verify`) | vecteur:p8-cold-roundtrip.json + vecteur:cb2-bundle-boundaries.json (`keyless_export`) | client (`cold_verify`), provider-wire |
| 230 | 02.12 | Un futur provider peut appeler cette unique façade Bundle puis ne faire que du stockage opaque, du transport et son propre CAS. | FUTUR | aucune | `provider::service.rs` (le crate existe ; la spec le qualifie de « future provider ») | vecteur:p1-store-envelope.json + vecteur:p2-store-cas.json | provider-wire |
| 231 | 02.12 | Le provider ne reçoit AUCUNE clé de contenu ni plaintext protégé et ne DOIT PAS copier ni réimplémenter la sémantique de périmètre, mandat, contrainte, révocation, Gamma, changeset ou paternité. | MUST NOT | aucune | `provider::service.rs` (`store_object` : contrôles de forme légers seulement ; `501 not_implemented` sur manifeste / did.json / certs / gamma ; blobs traités en octets opaques) | vecteur:p1-store-envelope.json + vecteur:p9-store-reads.json | provider-wire |

---

## Comptages

- **231 capacités normatives** inventoriées.
- **Force** : `MUST` 185 · `MUST NOT` 37 · `MAY` 5 · `FUTUR` 2 · 2 lignes mixtes
  (`MAY (porter) / MUST (résoudre)`, `MUST NOT / MUST`). Aucun `SHOULD` dans ce chapitre.
- **Clé (primaire)** : `aucune` 181 · `descellement` 31 · `signature` 19.
- **Implémenté** : 216 oui · 7 partiels · 8 `NON`.
- **Prouvé** : 11 `RIEN` · 4 `PROXY` · aucun `@wip`.

## Points saillants

### A. MUST non implémentés (ou implémentés à trous)

1. **§02.7 — `gamma_ref` par section : totalement absent.** La spec écrit
   « a verifier checks that every section's `gamma_ref` resolves in the log »
   et fait figurer `gamma_ref` dans les deux schémas d'index (`e/circle/index.json`
   et `e/self/index.json`). Le champ n'existe NULLE PART dans le code, ni dans
   les vecteurs, ni dans les features. Seul le lien `manifest.gamma_head` ↔ tip
   du log est vérifié (`bundle::bundle.rs::verify`). Ligne 141.
2. **§02.11 — Aucun vérifieur de signature de contenu propriétaire.**
   « A verifier rejects any owner signature whose embedded placement does not
   match where the object actually sits (fail-closed) » : `owner_content_sig`
   PRODUIT la signature sur `{zone, path, sid, body_hash}`, mais aucune fonction
   ne la revérifie — `Bundle::verify()` ignore `row.sig`, et le `sig` du blob
   `circle` n'est jamais réouvert pour contrôle. Le fail-closed sur placement
   n'existe donc pas. Lignes 199, 200, 203.
3. **§02.6 — Règle de la « plus longue chaîne vue » non implémentée.**
   `verify()` contrôle `prev_hash`, la hauteur strictement croissante et le tip,
   mais aucune comparaison entre chaînes candidates concurrentes de longueurs
   différentes. Seul le cas « même hauteur, même grand-parent » est traité
   (fusion/fork). Ligne 56.
4. **§02.2 — Unicité des noms parmi les frères : non appliquée sur la voie propriétaire.**
   `bundle::structure.rs` (chemins délégués : rename, move, edit-metadata) contrôle
   les collisions ; `bundle::bundle.rs` (`section_add`, `ensure_folder`,
   `rename_folder`) ne contrôle rien. Un propriétaire peut donc créer deux sections
   de même nom dans le même dossier, ce que `public_read` (qui résout par `name`)
   résoudrait alors arbitrairement. Ligne 15.
5. **§02.2 — Passe de réparation des wraps de tag : inexistante.** Le MUST
   « when a repair pass creates a missing tag wrap … it MUST first validate the
   author of that tag mutation … and fail closed » n'a aucun équivalent : il n'y
   a pas de passe de réparation, seulement la synchronisation en ligne sous
   contrôle d'autorité au moment de la mutation. Lignes 23 et 159.
6. **§02.10 — Feuilles plates non conformes.** `bundle::state.rs::self_build`
   replie `32×0x00` au lieu de `header_hash(N)` (commentaire du code :
   « Headers of self nodes are NOT folded in H1 … assumed debt »), et
   `vault_build` construit ses feuilles comme `path ‖ 0x00 ‖ object_hash`
   au lieu de `JCS(index_row) ‖ header_hash`. Un octroi sur un nœud `self` ne
   remonte donc PAS la racine `self`. Ligne 188.
7. **§02.3 — Schémas d'index divergents.** Au-delà de `gamma_ref`, `SelfRow`
   porte un champ supplémentaire `access` (locateur opaque scellé) là où la spec
   dit « nothing else ». Lignes 26 et 27.

### B. Implémentés mais non prouvés

8. **§02.11 — Le préimage de signature de contenu propriétaire n'est figé par
   aucun vecteur.** `{zone, path, sid, body_hash}` en JCS n'apparaît ni dans
   `vectors/` ni dans un test unitaire : une réimplémentation tierce n'a aucun
   oracle byte-for-byte pour cette signature. Lignes 199, 203.
9. **§02.2 — Absence du concept `ns` : conformité par absence, non testée.**
   Aucun test ne verrouille l'interdiction de réintroduire un identifiant
   namespacé. Ligne 18.
10. **§02.12 — Les frontières K1-B « effet externe » sont couvertes en PROXY.**
    Les scénarios `o-connector-classes-vault.feature` (staging pré-effet /
    post-effet, non-création d'un `pending` canonique, réconciliation côté
    connecteur) passent par les helpers `cb5_catalog_result` / `cb6_result` /
    `cb7_result` avec `OnceLock` dans `bundle/tests/cucumber.rs` : le verdict
    provient d'un oracle de données, pas d'un chemin d'exécution réel du bundle.
    Lignes 215, 216, 218, 219.
11. **§02.12 — Staging `FsStore` « physically outside the canonical bundle
    directory ».** L'implémentation place le staging et les pointeurs sous
    `<root>/.aithos-generations` et `<root>/.aithos-current`, soit à l'INTÉRIEUR
    du répertoire racine ouvert. La propriété tenue est « hors du namespace
    canonique » (filtrage `.aithos-` + `validate_store_key`), pas « hors du
    répertoire ». Ligne 221.

### C. FUTUR — citation exacte du statut donné par la spec

12. **§02.8 — Preuve opaque de suppression/déplacement de dossier `self`.**
    Spec, §2.8 dernier paragraphe : « For a `self` folder delete or move, that
    opaque proof covers the exact set of affected commitments and the authority
    for each of them. […] **Its additive signed encoding is reserved for
    independent CB2 vectors.** » → non construit, aucun vecteur, aucune feature.
    Ligne 156.
13. **§02.12 — Provider comme simple stockage opaque.** Spec, §2.12 dernier
    paragraphe : « **A future provider may call this one Bundle façade** and then
    perform only opaque storage, transport, and its own CAS. It receives no
    content key or protected plaintext and MUST NOT copy or reimplement
    perimeter, mandate, constraint, revocation, Gamma, changeset, or authorship
    semantics. » Le crate `aithos-provider` existe et respecte l'opacité
    (`501 not_implemented` sur les classes vérifiables), mais le statut normatif
    reste projeté. Lignes 230, 231.
14. **§02.3 — Shardage des index.** « **Sharding of large indexes is permitted**
    (deterministic, by `sha256(sid)`) but omitted here for clarity; it does not
    affect keys or headers. » → `MAY`, non implémenté. Ligne 30.
15. **§02.9 — Variante paresseuse de re-chiffrement au déplacement.**
    « Cost ∝ M's granted headers (+ re-encryption of M's subtree if
    incident-grade); **the lazy variant is tolerated as hygiene (§06.8)**. » →
    `MAY`, non implémenté (`move_folder` re-chiffre systématiquement). Ligne 166.
16. **§02.11 — Divulgation sélective et sidecar de signature publique.**
    « **Selective disclosure (the official inverse).** The owner can convert
    deniability into proof, per section, at will […] » et « the signature ships
    in the index row and **MAY** travel as a sidecar with the raw markdown ».
    Aucune des deux n'existe dans le code. Lignes 202, 206.

### D. Lecture « Clé » — ce qui peut tourner en ligne

- **`aucune` (clé primaire) : 181 capacités sur 231 (≈ 78 %).** Toute la vérification
  (chaîne d'éditions, fusion/fork, racines Merkle, carriers K1-B/K1-C, faits
  d'opération opaques `self`, confinement de chemins, atomicité) est
  calculable par un tiers sans le moindre secret — c'est exactement le
  périmètre hébergeable chez Aithos, et c'est ce que `provider::service.rs`
  et `p8-cold-roundtrip.json` matérialisent déjà.
- **`descellement` (clé primaire) : 31 capacités.** Concentrées en §2.4 (blobs), §2.5
  (dérivation), §2.8 (descripteurs `self`), §2.9 (wraps de tag, rotation de
  déplacement). Elles doivent rester côté client — aucune n'est atteignable
  depuis le provider.
- **`signature` (clé primaire) : 19 capacités.** Émission des manifestes, paternité publique
  déléguée, présentations Gamma opposables, `co_sign` propriétaire. Elles
  exigent la clé privée Ed25519 de l'acteur ; leur CONTRÔLE, lui, est toujours
  en `aucune`.

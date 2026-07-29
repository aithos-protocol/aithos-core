# Constat d'audit — ligne d'index `self` orpheline lors d'une création déléguée

| Champ | Valeur |
|---|---|
| Statut | **Défaut confirmé — correctif non implémenté** |
| Vérifié le | 29 juillet 2026 |
| Révision `aithos-core` | `be2d098eeb79107c861462a6433df9ef45871265` |
| Composant | `aithos-bundle` |
| Chemin principal | `grants.rs::grantee_create_self` |

## Résultat

Une création déléguée dans la zone `self` peut écrire une ligne dans
`e/self/index.json` sans rattacher la section à un descripteur. La section
devient alors présente dans l'index plat mais absente du parcours de l'arbre.

Ce constat décrit l'implémentation actuelle. Il est indépendant de la
proposition d'autoriser `dir=` ou `tag=` pour les écritures `self`.

## Chemin qui produit le défaut

1. `grantee_create_self` n'accepte actuellement qu'une cible ancrée à la
   racine.
2. La fonction ajoute un `SelfRow` à l'index et persiste
   `e/self/index.json`.
3. Elle tente ensuite de dériver la clé de zone afin de mettre à jour le
   descripteur.
4. Cette mise à jour se trouve derrière un `if let Ok(...)`.
5. Un échec de dérivation est donc ignoré après la persistance de la ligne.

L'opération englobante est transactionnelle, mais ce chemin silencieux ne
renvoie aucune erreur : la transaction peut donc se terminer normalement avec
l'index mis à jour et le descripteur inchangé.

Le succès de la commande ne prouve pas l'invariant « une ligne d'index
correspond à exactement un emplacement dans l'arbre ».

## Impact

- divergence entre l'index et les descripteurs ;
- section invisible pour un lecteur qui parcourt uniquement l'arbre ;
- état ambigu pour les suppressions, déplacements et preuves de couverture ;
- impossibilité d'utiliser la structure physique comme garde-fou d'un scope
  `dir=` tant que l'invariant n'est pas garanti.

## Correctif minimal attendu

1. Résoudre et ouvrir le descripteur cible avant toute mutation persistée.
2. Préparer l'index et le descripteur comme une seule mutation logique.
3. Échouer fermé si le descripteur ne peut pas être ouvert ou mis à jour.
4. Refuser à la vérification toute ligne d'index sans rattachement unique.
5. Ajouter un test négatif qui force l'échec de dérivation et vérifie qu'aucune
   ligne n'a été écrite.
6. Ajouter un round-trip à froid qui parcourt l'arbre et retrouve exactement
   les mêmes SIDs que l'index.

## Critère de fermeture

Le défaut est fermé uniquement lorsque les tests passent sur le chemin de
production réel et démontrent :

- l'atomicité logique index/descripteur ;
- l'absence d'orphelin après un échec injecté ;
- l'unicité du rattachement après relecture à froid.

## Évolution associée mais distincte

La possibilité d'écrire sous `self` avec des scopes `dir=` ou `tag=` est une
évolution protocolaire séparée :
[`../../proposals/scoped-self-writes.md`](../../proposals/scoped-self-writes.md).

# Proposition — sharding déterministe des index de zone

| Champ | Valeur |
|---|---|
| Statut | **Exploratoire — non adopté** |
| Date | 28 juillet 2026 |
| Portée | format de stockage, manifests, provider et compatibilité |
| Décideur | à désigner |
| Implémentation autorisée | non |

## Problème

Les index monolithiques sont réécrits lors des éditions. Leur coût peut devenir
dominant à grande échelle. La spécification autorise le principe d'un sharding
déterministe par `sha256(sid)`, mais ne définit pas encore un format wire
complet, une négociation de version ou une procédure de migration.

## Forme étudiée

Une option consiste à remplacer :

```text
e/<zone>/index.json
```

par :

```text
e/<zone>/index/<shard>.json
```

où `shard` serait dérivé d'un préfixe hexadécimal de `sha256(sid)`.

Cette forme est une hypothèse de conception. Le nombre de shards, leur
découverte et leur engagement cryptographique restent à définir.

## Pourquoi ce n'est pas encore une décision

Le chemin `e/<zone>/index.json` appartient actuellement à une grammaire fermée
et apparaît dans le bundle, les path maps du provider, les documents
d'infrastructure, les tests et les vecteurs. Le modifier est une évolution de
layout et potentiellement de wire, pas un simple paramètre interne.

Une table de shards actifs dans le manifeste serait elle-même un nouvel élément
signé. Sa forme, son caractère obligatoire et le comportement des anciens
lecteurs ne sont pas spécifiés.

Le « shard unique » n'est pas automatiquement compatible avec l'ancien layout :
le contenu logique peut être identique alors que les chemins, le manifeste et
les objets engagés diffèrent.

## Questions à résoudre

1. Le layout actuel reste-t-il lisible, et pendant combien de versions ?
2. Comment un lecteur distingue-t-il un index monolithique d'un index shardé ?
3. Où la liste des shards est-elle engagée et comment détecter une omission ?
4. Le nombre de shards est-il fixe, versionné ou adaptable ?
5. Comment re-sharder sans perdre l'explication d'autorité d'une édition ?
6. Quels effets sur les racines, manifests, CAS, preuves et baselines ?
7. Quelle migration pour les bundles déjà publiés et les caches locaux ?
8. Quels providers et clients doivent accepter simultanément les deux layouts ?

## Gates avant adoption

- mesure reproductible du coût actuel et du gain attendu ;
- redline normative du layout et de sa négociation de version ;
- matrice de compatibilité lecteur/émetteur/provider ;
- stratégie de migration et de rollback ;
- vecteurs positifs et négatifs couvrant omission, mauvais placement et
  re-shardage ;
- test interopérable multi-dépôts ;
- revue sécurité de l'engagement de la liste des shards.

## Non-objectifs

Le sharding ne résout pas à lui seul le recalcul du `StateTree`, la rétention
des sidecars ou la croissance générale de l'historique de publication. Ces
optimisations doivent être mesurées et décidées séparément.

## Références

- `spec/02-content-tree.md`, section sur les index et le sharding ;
- `docs/INFRA-PROVIDER.md`, layout actuellement documenté ;
- mesures de coût de publication à rattacher ici lorsqu'elles auront été
  intégrées sur la branche courante.

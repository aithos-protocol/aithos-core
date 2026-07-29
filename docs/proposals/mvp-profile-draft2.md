# Proposition — profil de mandat `draft.2` pour le MVP

| Champ | Valeur |
|---|---|
| Statut | **Proposé — non adopté** |
| Date | 28 juillet 2026 |
| Portée | déclaration de conformité du MVP |
| Décideur | à désigner |
| Implémentation autorisée | non |

## Question à trancher

Le MVP doit-il revendiquer uniquement le profil
`aithos-mandate-core: 1.0.0-draft.2`, sans revendiquer `draft.3` ni le niveau
*Agent host* ?

Cette proposition ne modifie pas la spécification et ne constitue pas une
déclaration de conformité. Elle formalise un choix de périmètre à faire valider.

## Motivation

La spécification réserve encore une partie des tables d'octets associées à
`draft.3` à une validation humaine. Une implémentation ne doit pas inventer ces
octets. Le profil `draft.2` est donc une cible plus prudente tant que :

- les tables `draft.3` ne sont pas approuvées et figées par des vecteurs ;
- les chemins d'émission opération/évidence et catalogue ne sont pas audités de
  bout en bout ;
- le niveau *Agent host* reste hors du périmètre déclaré.

Le code qui accepte ou vérifie un format ne prouve pas à lui seul que le format
est produit par les chemins réels ni qu'un niveau de conformité est atteint.

## Conséquences si la proposition est adoptée

- la déclaration de conformité nomme explicitement `draft.2` ;
- `draft.3` et *Agent host* restent hors périmètre, sans être présentés comme
  implémentés ou implicitement couverts ;
- les fonctionnalités réservées à `draft.3` peuvent rester présentes comme
  code expérimental ou vérificateurs isolés, mais ne comptent pas comme preuve
  du MVP ;
- toute réintégration d'*Agent host* rouvre simultanément le choix du profil.

## Conditions d'adoption

1. Nommer le décideur et enregistrer son approbation.
2. Auditer les appels de production des fonctions opération/évidence et
   catalogue, au lieu de se fier à leur seule présence.
3. Définir précisément les niveaux de conformité revendiqués.
4. Ajouter des gates qui relient chaque revendication à un vecteur ou test
   sémantique précis.
5. Vérifier que les surfaces client, provider et gateway n'annoncent pas
   `draft.3` par défaut.

## Réouverture

La décision devra être réexaminée lorsque les tables `draft.3` auront été
validées, ou avant toute revendication du niveau *Agent host*.

## Références

- `spec/00-overview.md`, section consacrée aux profils de mandat ;
- `spec/09-conformance.md`, déclaration des niveaux revendiqués ;
- `vectors/README.md`, statut des tables et vecteurs ;
- audits Gherkin vivants sous `docs/audits/features/`.

# Inventaire topologique du 28 juillet 2026 — recherche non vérifiée

## Statut

Ce répertoire conserve quatre inventaires manuels couvrant les spécifications
00 à 10. Ils constituent du **matériau de recherche**, pas une matrice de
conformité, un plan d'implémentation ou une source de vérité.

Une capacité ne doit être déclarée implémentée ou prouvée qu'après vérification
de son exigence, du chemin de production réellement exécuté et de l'assertion
qui établit sa sémantique. Les audits vivants de `docs/audits/` priment sur ces
inventaires.

## Provenance disponible

Les documents d'origine ne consignaient pas les révisions Git utilisées pendant
leur rédaction. Les révisions ci-dessous sont celles présentes lors de leur
mise en quarantaine le 29 juillet 2026 ; elles ne prouvent pas que chaque ligne
a été évaluée contre ces versions exactes.

| Dépôt | Révision lors de la mise en quarantaine |
|---|---|
| `aithos-core` | `be2d098eeb79107c861462a6433df9ef45871265` |
| `aithos-client` | `c6f615123ca3dc83708ba029b898375409551719` |
| `aithos-sdk` | `6117daa0984b70b7f7821fd2d4400aec75467036` |
| `provider` | `5536840cbab01186533daeded9961cf27b35e805` |

Les chemins absolus `/root/aithos-*` présents dans les inventaires sont des
traces de l'environnement de rédaction. Ils ne sont ni portables ni normatifs.

## Contenu conservé

- `lot-A-00-01-03-10.md` : 174 entrées ;
- `lot-B-02.md` : 231 entrées ;
- `lot-C-04.md` : 430 entrées ;
- `lot-D-05-09.md` : 305 entrées.

Les identifiants forment une séquence complète de 1 140 entrées. Cette
continuité structurelle ne valide ni les catégories, ni les preuves citées, ni
les verdicts d'implémentation.

## Défauts connus

- les lignes A111 et A166 contenaient des caractères `|` non échappés ; leur
  syntaxe de table a été réparée lors de la mise en quarantaine, sans valider
  leur contenu ;
- l'ancien CSV dérivé divergeait silencieusement des tables sur au moins douze
  entrées et a été supprimé ;
- les agrégats publiés ne se réconciliaient pas : 49 capacités manquaient dans
  la classification des besoins en clés, 22 dans celle des preuves et 12 dans
  celle des forces normatives ;
- plusieurs références de tests appartiennent à des dépôts voisins sans
  révision enregistrée ;
- un nom de vecteur ou de feature a parfois été utilisé comme preuve pour de
  nombreuses exigences, sans lien vers une assertion précise ;
- certains verdicts « implémenté » décrivent en réalité un comportement partiel
  ou non conforme ;
- les conclusions sur l'identité et la succession sont contredites par
  `docs/audits/features/a-identity.md`.

## Règles d'utilisation

1. Ne jamais produire de pourcentage de conformité à partir de ces fichiers.
2. Ne jamais transformer une référence de fichier en preuve sans identifier
   l'assertion exécutée et le chemin de production atteint.
3. Valider les entrées pertinentes au fil des audits Gherkin, puis les reporter
   dans une future source canonique normalisée.
4. Si une matrice est reconstruite, choisir une seule source structurée et
   générer toutes ses vues avec un validateur de schéma, de liens et de totaux.

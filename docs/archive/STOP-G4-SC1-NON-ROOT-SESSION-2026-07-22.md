# STOP G4 — SC1 ne consomme pas une feuille de sous-mandat

> **RÉSOLU — archive de diagnostic.** Le front door Core
> `verify_delegated_session(DelegatedSessionEvidence)` a été ajouté vectors-first,
> puis branché par la Gateway dans le commit `e90fc41`. Le blocage décrit ci-dessous
> n'existe plus ; le texte est conservé pour expliquer pourquoi l'extension était
> nécessaire.

Date : 2026-07-22

Statut historique : STOP observé avant P7, levé le 22 juillet 2026.

## Constat reproductible

La cérémonie G4 doit émettre un sous-mandat court signé par le délégué. Le
constructeur Core `Mandate::build_sub` fixe nécessairement `parent` à l'id du
mandat délégué (`aithos-core/src/mandate.rs`, lignes 954–983).

Le vérifieur SC1 public `verify_session` commence par `session_mandate`. Cette
fonction refuse toute mandate dont `parent` est présent avec le verdict
`SC1 leaf chain is required for a non-root mandate`
(`aithos-core/src/operation.rs`, lignes 1903–1924).

`SessionEvidence` ne porte qu'une mandate, le certificat SC1, la projection et
les deux preuves. Il ne porte ni chaîne, ni DID, ni état de révocation. La
gateway ne peut donc pas fournir le prérequis que le verdict réclame au même
front door Core.

Conséquence : une feuille G4 valide est toujours non-root et sera toujours
refusée par `verify_session` avant la vérification des deux preuves. Retirer
`parent`, fabriquer une mandate root, ou accepter une vérification gateway
parallèle changerait l'autorité ou contournerait le Core ; ces options sont
interdites par le handoff.

## Portée atteinte

P0 à P6 sont implémentés. P7 ne peut pas brancher `sid → chaîne → SC1` sur le
hot path sans retouche Core. Aucun fichier Core, Bundle, vecteur ou wire n'a été
modifié après ce constat.

## Extension recommandée, soumise à validation humaine

Ajouter vectors-first un front door Core distinct pour une session déléguée
non-root. Il recevrait la chaîne complète et ses données de vérification
(DID, instant et révocations), vérifierait la chaîne/atténuation, sélectionnerait
exactement sa feuille, puis réutiliserait sans changement le certificat SC1,
la projection W1.1 et les deux preuves sur le même `operation_ref`.

Contraintes de compatibilité :

- le wire SC1/W1.1 historique et `verify_session` restent inchangés ;
- le vecteur historique reste vert byte-exact ;
- de nouveaux vecteurs indépendants couvrent feuille non-root, chaîne tronquée,
  parent révoqué, feuille substituée et preuves croisées ;
- aucune exception ou vérification cryptographique parallèle dans la gateway ;
- reprise de P7 seulement après validation humaine explicite de cette extension.

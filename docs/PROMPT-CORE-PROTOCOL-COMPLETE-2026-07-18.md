# Prompt de reprise — finaliser intégralement Aithos Core

> **ARCHIVE — ne pas exécuter.** Le plan auquel ce prompt renvoie a été clos par
> CB13.

Copier le bloc ci-dessous dans une nouvelle tâche Codex.

```text
Tu reprends la finalisation intégrale du protocole dans :

/Volumes/Math17/aithos/v2/code/aithos-core

Ta source de reprise principale est :

docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md

Lis-la entièrement avant toute action, puis lis intégralement toutes les sources
qu'elle rend obligatoires : README, specs 00–10, execution plan, handoffs mandats,
features pertinentes, rituels BDD/vectors-first/pure-core et code réel des crates
core, bundle, provider, gateway, CLI et WASM.

Mission opposable :

Aithos Core doit être parfaitement terminé sur tout le périmètre protocolaire utile
au produit AVANT d'étendre aithos-client aux mutations et de construire le SDK
réseau. Toutes les fonctionnalités autorisables par un mandat doivent être réellement
expressibles, sérialisables, atténuables, révocables, exécutables, journalisées et
publiables sur public/circle/self, gamma et connecteurs. Le futur client et le SDK ne
doivent jamais réimplémenter une règle du protocole.

Décisions déjà acquises :

- provider et dashboard serveur ne manipulent que des artefacts chiffrés/publics ;
- aucune clé de déchiffrement ni donnée client en clair ne quitte le client ;
- owner = capacité locale sans mandat ;
- grantee = clé privée ET chaîne de mandats valide ;
- la possibilité cryptographique seule n'autorise jamais une opération ;
- un mandat doit pouvoir autoriser create/edit/delete/write ;
- aithos-client reste strictement offline et le SDK réseau reste hors de ce dépôt ;
- plusieurs Ethos/mandats sont orchestrés par plusieurs sessions isolées.

État critique déjà observé, à revérifier :

- les écritures déléguées locales existent seulement pour des sections circle ;
- une édition déléguée normale est refusée : seuls les fork resolutions acceptent
  actuellement une signature déléguée ;
- id= n'est pas implémenté pour les périmètres Ethos ;
- public/self n'ont pas la parité déléguée ;
- plusieurs contraintes sont parsées/atténuées mais pas exécutées partout ;
- les mutations Ethos ne passent pas par tout le moteur de consommation des actions ;
- le gateway n'a pas encore les classes protocolaires read/act/binding complètes ;
- le wildcard peut encore couvrir binding ;
- le vault n'est pas isolé et géré complètement par /x/<connector> + .config ;
- aithos-client est volontairement lecture seule pour le moment.

Commence uniquement par le Lot 0 du handoff :

1. vérifie le worktree réel et préserve absolument tous les changements existants ;
2. ne change pas de branche, ne nettoie rien, ne pousse rien ;
3. rejoue les tests de baseline pertinents sans modifier le code ;
4. construis une matrice exhaustive :
   spec → Gherkin → vector → core → bundle → provider → gateway → CLI/WASM ;
5. classe chaque capacité absent/partiel/complet/contradictoire ;
6. examine les décisions D1–D9 du handoff ;
7. présente à Mathieu les contradictions et recommandations qui nécessitent sa
   validation ;
8. n'implémente aucun changement de wire ou nouveau contrat substantiel avant ce
   gate.

Après validation de Mathieu, applique strictement le rituel :

Gherkin @wip d'abord → validation → commit contrat isolé → oracle/vecteur indépendant
→ test rouge → TDD minimal → retrait progressif des @wip → vrai E2E sans mock du
protocole → fmt/clippy/tests/WASM → gate → commit étroit.

Exigence de fin :

Une capacité ne compte pas comme terminée si elle n'existe que dans la spec, reste
@wip, marche seulement en mémoire, ne peut pas produire une édition chiffrée
publiable, exige l'owner dans la boucle, ou ne peut pas être revérifiée à froid après
un aller-retour par le provider. Le provider doit accepter/refuser keyless, sans clé
ni plaintext. Core reste pur ; bundle possède l'I/O ; provider/gateway/WASM/clients
orchestrent sans dupliquer le protocole.

Le dépôt est sale et contient des travaux en cours appartenant à Mathieu. Stage et
commit uniquement les fichiers de ta tranche. Si une modification existante
chevauche ton travail, arrête-toi et demande une décision.

Ne te contente pas d'un plan abstrait : inspecte et cite le code réel. Mais pour cette
première session, arrête-toi au gate produit/contrat si les décisions D1–D9 ne sont
pas toutes explicitement opposables.
```

# Mandats — écarts à fermer pour la surface produit

> **État réévalué le 2026-07-22 contre le code et les contrats.** Cette note ne
> modifie pas la spécification. Deux écarts protocolaires décrits le 15 juillet
> sont désormais fermés ; la surface produit générale reste en partie ouverte.

| Écart initial | État vérifié | Reste réellement à faire |
|---|---|---|
| Sélecteur de section `id=` | **fermé dans le Core et le client** via `PerimeterEntry::EthosId`, parsing/couverture et cibles `Sid` | détaguer et compléter les parcours produit Gateway encore `@wip` |
| Atténuation des contraintes | **fermée dans le Core** via `constraints_attenuate` et `constraints_attenuate_for_profile` | achever les contrats Gateway d'émission/preview encore `@wip` |
| Plusieurs mandats restreints | **partiel** : émission de parent de session et preview owner existent | surface owner générale, lifecycle multi-mandats et UI stable |
| Borne Ethos + restriction mandat | **partiel** : le preview et le verdict runtime partagent une logique testée | snapshot/supersession produit et couverture exhaustive des états/usages |

## Principe produit à préserver

Un mandat est une **vue soustractive et immuable** d'un Ethos :

```text
droits effectifs = politique Ethos/connecteurs
                ∩ périmètre du mandat
                ∩ périmètres de ses parents
                ∩ contraintes applicables
```

Un mandat peut sélectionner moins de contenu ou d'outils et ajouter des contraintes ;
il ne peut jamais élargir la politique de l'Ethos ni un mandat parent. Les credentials
des connecteurs restent dans le vault/gateway, sauf grant explicite de
`act.x.<connector>.config`.

## P0 — requis avant de brancher l'interface sur le core réel

### 1. Sélecteur de section `id=` pour les zones Ethos — fermé au niveau protocole

La spec autorise `read.self#id=<sid>` et les variantes avec verbes. Le Core porte
désormais une variante fermée `PerimeterEntry::EthosId`, son parsing, sa
sérialisation et sa couverture d'opération. Le client transporte aussi une cible
`Sid` dans les intentions de mandat.

Reste côté produit :

- fermer les scénarios Gateway encore marqués `@wip`, dont le scénario historique
  qui attendait ce sélecteur ;
- qualifier la livraison des lignes de header et les parcours UI sur les zones
  réellement exposées.

### 2. Atténuation des contraintes — fermée dans le vérifieur Core

`verify_chain` appelle maintenant le moteur typé d'atténuation. Les caps,
allow-lists, budgets, paramètres d'action, heartbeat/freshness et profils fermés
sont traités fail-closed par le même Core.

Reste côté intégration :

- détaguer les scénarios Gateway d'émission qui prouvent les mêmes refus ;
- conserver le preview owner et le runtime sur cette fonction commune, sans
  réimplémentation UI.

### 3. Émettre plusieurs mandats restreints depuis un Ethos déjà équipé

`owner_enroll_servers` transforme actuellement tous les outils `granted` des
manifestes en un unique `agent_mandate`, sans contraintes supplémentaires. Il manque
la surface owner permettant de réutiliser l'Ethos comme plafond d'autorité.

À faire :

- ajouter une commande/API owner qui reçoit un destinataire, un sous-ensemble de
  zones/dossiers/sections, un sous-ensemble d'outils et des contraintes ;
- valider chaque outil demandé contre le manifeste approuvé et ses bornes dures ;
- permettre plusieurs mandats actifs par Ethos et/ou par même keypair ;
- ne jamais livrer la ligne vault du credential pour un simple droit `act.*` ;
- journaliser émission, remplacement et révocation dans gamma ;
- conserver `owner_enroll_servers` comme raccourci d'équipement initial, pas comme
  unique chemin de création de mandat.

### 4. Formaliser la composition « borne Ethos + restriction mandat »

Le gateway possède déjà des bornes dures par outil dans le manifeste scellé
(`one_of`, `time_slots`, `forbid`, `require`, `max_items`). La spec place les fenêtres,
quotas, budgets et obligations dans le mandat. Le contrat effectif doit être unique et
explicite.

À faire :

- graver que les bornes du manifeste sont héritées, non modifiables par le mandat ;
- appliquer les restrictions du mandat en conjonction, après autorisation de l'outil
  et avant l'effet externe ;
- exposer une fonction pure de calcul/description de la politique effective, utilisée
  par le runtime et par le preview UI ;
- garantir qu'un changement de manifeste produit un nouveau snapshot/mandat et
  révoque ou supersède l'ancien, sans élargissement silencieux.

## P1 — cohérence et durcissement

- Faire respecter la règle `act.x.<connector>.*` : le wildcard ne couvre jamais une
  action `binding`, au lieu de laisser cette distinction uniquement au manifeste ou
  au host.
- Faire converger les classes gateway v1 `read/write` vers les classes protocolaires
  `read/act/binding` sans casser les manifests existants.
- Fournir un read-model stable pour le produit : mandat actif/expiré/révoqué,
  périmètre effectif, contraintes héritées, usages restants et raison précise d'un
  refus.

## Critères d'acceptation produit

1. Le preset **Full Ethos** énumère les droits et outils présents au moment de
   l'émission ; un futur connecteur n'est jamais ajouté automatiquement.
2. Le mode **Restricted** ne permet que de décocher un droit ou de resserrer une
   contrainte héritée.
3. Le preview présenté à l'owner et la décision du gateway sont calculés par la même
   logique.
4. Toute extension produit un nouveau mandat signé ; l'ancien reste vérifiable dans
   l'audit et peut être révoqué/supersédé explicitement.

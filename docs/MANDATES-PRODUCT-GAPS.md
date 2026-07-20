# Mandats — écarts à fermer pour la surface produit

> État au 2026-07-15. Cette note ne modifie pas la spécification : elle liste les
> coutures manquantes entre `spec/04-mandates.md`, `spec/05-delegation.md`,
> `spec/08-connectors.md` et l'interface de création de mandats.

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

### 1. Implémenter le sélecteur de section `id=` pour les zones Ethos

La spec autorise `read.self#id=<sid>` et les variantes avec verbes, mais
`PerimeterEntry::Ethos`, son parseur et `Op` ne portent actuellement que `dir` et
`tag`.

À faire :

- ajouter `id: Option<Sid>` à la représentation Ethos et au wire canonique ;
- parser, sérialiser et vérifier son containment (`id` ne se compose avec rien) ;
- transmettre le sid de section à `covers_op` ;
- livrer la header line de la section lors du grant ;
- couvrir lecture et écriture par section, notamment `self`, en BDD + tests core,
  bundle et CLI ;
- ajouter un vecteur de conformance afin de figer les octets signés.

### 2. Compléter l'atténuation de toutes les contraintes

`verify_chain` vérifie aujourd'hui l'atténuation des fenêtres absolues et des
obligations. La règle normative est plus large : un enfant doit aussi resserrer les
caps numériques, budgets, domaines, paramètres d'action, heartbeat, freshness,
`first_party_only`, `counter_sign` et `binding`.

À faire :

- introduire une validation/normalisation typée des contraintes connues ;
- implémenter `constraints_attenuate(parent, child)` fail-closed ;
- définir explicitement le traitement des clés inconnues lors d'une sous-délégation ;
- tester au minimum chaque famille : cap inférieur accepté, cap supérieur refusé,
  allow-list incluse acceptée, suppression d'une contrainte héritée refusée ;
- faire appeler le même contrôle par la vérification offline et la gateway.

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

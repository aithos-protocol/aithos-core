# Démo SDK navigateur — écarts, déploiement et rollback

État vérifié le 22 juillet 2026. Ce document complète
`ETAT-DES-LIEUX-DEMO-GATEWAY-CLIENT-SDK-2026-07-22.md` après correction des
gates navigateur. Aucun changement de production n'a été appliqué pendant
cet audit.

## Résultat de l'audit

Le parcours cible reste G4 + Google Sheets read-only : discovery OAuth, DCR,
PKCE, publication du parent, cérémonie, échange du code, `tools/list`, lecture
d'une plage, refus de l'écriture voisine et vérification Gamma.

Les éléments manquants et désormais traités sur les branches de préparation
sont :

1. CORS exact sur les routes Gateway utilisées par le SDK : discovery,
   registration, authorization, token, cérémonie et MCP ;
2. preflight et réponse CORS exacts pour les publications signées du Store ;
3. liaison du parent G4 au `resource` OAuth exact dans le mandat signé, puis
   filtrage de ce parent par la Gateway ;
4. `/healthz` sur la Gateway multi-contextes pour le smoke test de rollout ;
5. configuration Terraform de l'allowlist Store et maintien explicite des
   deux tâches durables déjà actives en production.

Le relay TLS reste un passthrough opaque : il ne termine pas le HTTP public et
n'a donc aucun CORS à ajouter. Le witness et CloudFront ne participent pas aux
requêtes mutantes de ce parcours. L'ALB Store transmet déjà `OPTIONS` au
service. Aucun changement de security group, DNS, certificat, relay, witness
ou CDN n'est requis.

## Politique navigateur retenue

Gateway, dans la configuration du pod de démonstration :

```yaml
dashboard:
  allowed_origins:
    - https://app.aithos.fr
    - http://localhost:3000
```

Store, dans la task ECS :

```text
AITHOS_STORE_BROWSER_ORIGINS=https://app.aithos.fr,http://localhost:3000
```

Il s'agit d'origines exactes, sans credentials CORS. Une origine inconnue,
dupliquée ou mal formée est refusée avant tout effet. Les lectures publiques
anonymes restent accessibles avec `Access-Control-Allow-Origin: *`. Une
publication navigateur garde toutes ses barrières normales : enveloppe
`X-Aithos-Auth`, signature, nonce, fenêtre temporelle et CAS `If-Head`.

## État de production à préserver

Snapshot observé avant rollout :

| Service | Task definition | Image actuellement exécutée |
| --- | --- | --- |
| Store | `aithos-provider-prod-store-api:7` | digest `sha256:cec2c66708d33fc9fbabcc2e3f2c64d0c55ca6469d035fb6453defc1874b16c2` |
| Relay | `aithos-provider-prod-relay:8` | digest `sha256:328e6119688bcb2bc15c081926dea346ef475155c30d010225c204a1bf4c8e7e` |
| Witness | `aithos-provider-prod-witness:1` | digest `sha256:649f554b88e1a6483b95df51c51056a1ab93d506c44b0ade827d2db85fed1eba` |

Le Store est à `desired/running = 2/2`. Le binding public actif est le tenant
`demo`, hostname `demo.mcp.aithos.fr`, non suspendu. La branche refactorisée
descend de `origin/main` au commit `30efdb8`; ce commit est la baseline source
à conserver pour un rollback Gateway. Le binaire réellement en cours
d'exécution doit néanmoins être archivé et hashé avant remplacement : le
binding public ne prouve pas son commit de build.

Le plan Terraform effectué contre le state et les ressources réelles, épinglé
sur le digest Store courant, ne prévoit que :

- une nouvelle révision de task definition Store, avec le digest immuable et
  `AITHOS_STORE_BROWSER_ORIGINS` ;
- la mise à jour en place du service Store vers cette révision.

Il ne prévoit aucun changement relay, witness, réseau, DNS, certificat ou
stockage. Terraform l'affiche comme `1 add, 1 change, 1 destroy` parce qu'une
task definition ECS immuable est remplacée ; aucune donnée applicative n'est
détruite.

## Ordre de déploiement recommandé

### 1. Geler le rollback

Avant de construire quoi que ce soit :

- ajouter au manifeste ECR du Store courant un tag de rollback daté ;
- conserver les ARN des trois task definitions ci-dessus ;
- copier le binaire, la configuration non secrète et le hash du binaire
  Gateway actuellement actif ;
- vérifier que les tokens, clés privées, recovery files et credentials Google
  ne sont dans aucun fichier de build ou log.

### 2. Construire sans tag mutable

Construire le Store et la Gateway depuis des commits propres et revus. Pousser
le Store avec un tag de commit, relever le digest ECR, puis refaire le plan
Terraform avec ce digest. Le plan final doit garder exactement le même
périmètre que le plan d'audit.

Construire le package navigateur `aithos-client`, puis faire consommer cette
version au SDK et au dashboard. Le parent émis doit contenir la contrainte
signée `purpose = <resource OAuth exact>` ; mélanger l'ancien package client
avec la nouvelle Gateway conduit volontairement à zéro parent éligible.

### 3. Déployer le Store, puis la Gateway

Appliquer le plan Store et attendre `rolloutState=COMPLETED`, `running=2` et
`healthz=200`. Tester ensuite :

- lecture publique depuis une origine quelconque ;
- preflight PUT signé depuis `http://localhost:3000` ;
- refus sans header CORS depuis une origine non autorisée ;
- écriture signée de test sur un objet de démonstration et CAS voisin refusé.

Déployer ensuite la Gateway côte à côte avec l'ancien binaire et la
configuration d'origines ci-dessus. Ne basculer le process public qu'après les
smokes locaux. Vérifier `/healthz`, les deux documents discovery, un preflight
cérémonie et un preflight MCP avant le parcours complet.

### 4. Gate navigateur réel

Depuis `http://localhost:3000/delegation`, sans extension de désactivation
CORS et sans copier manuellement transaction, callback ou bearer :

1. importer l'Owner et créer le délégué ;
2. démarrer OAuth et publier le parent G4 ;
3. réimporter la récupération du délégué et terminer la cérémonie ;
4. échanger le code, initialiser MCP et lister les outils ;
5. appeler dynamiquement l'outil `read_range` exposé ;
6. montrer le refus du `write_range` voisin ;
7. charger la preuve Gamma et verrouiller les handles.

Le redirect URI du client OAuth Aithos enregistré par DCR pour ce beat est
exactement `http://localhost:3000/delegation`. Le redirect URI du client Google
amont reste `https://demo.mcp.aithos.fr/oauth/callback`. Le profil Sheets doit
être pré-provisionné, read-only et lié aux mêmes noms de
connecteur/capability que le parent.

## Rollback

Le rollback rapide du Provider consiste à remettre immédiatement le service
Store sur `aithos-provider-prod-store-api:7`. Les backends S3 et DynamoDB ne
changent pas, et les deux tâches doivent revenir à `2/2`. Revenir ensuite sur
le commit Terraform précédent et appliquer pour résorber le drift créé par ce
rollback opérationnel.

Le rollback Gateway consiste à rebascule vers le binaire et la configuration
archivés avant rollout. À défaut d'un artifact vérifié, reconstruire la
baseline `origin/main` `30efdb8`, sans modifier les volumes Vault/OAuth ni les
certificats. Le parent G4 enrichi d'une contrainte `purpose` reste un artifact
plus strict ; l'ancienne Gateway peut l'ignorer, mais il ne faut jamais
réémettre un parent moins contraint pour rendre le rollback possible.

Après rollback, vérifier Store `2/2`, `/healthz`, discovery, tunnel public et
une lecture MCP connue. Conserver les logs des nouvelles tasks et du nouveau
process pour l'analyse, sans y copier de credentials.

## Gate restant avant autorisation de prod

Le code et les plans ferment les écarts structurels, mais quatre preuves
restent volontairement à produire avec les credentials et artifacts finaux :

- image Store refactorisée construite, poussée et épinglée par son digest ;
- binaire/config Gateway refactorisés installés côte à côte ;
- plan Terraform final recalculé avec ce nouveau digest ;
- parcours navigateur Sheets read-only complet sur les endpoints publics.

Ces quatre points sont des opérations de déploiement et de qualification, pas
des changements d'architecture supplémentaires.

# Chantier futur — refactor OAuth sur des bibliothèques standard

Date : 2026-07-22

Statut : **chantier à venir, non bloquant pour la première démo**.

## 1. Décision proposée

La cible n'est pas de remplacer toute la sécurité Aithos par une bibliothèque
générique. La cible est une architecture hybride :

- confier la mécanique OAuth/OIDC standard à des bibliothèques Rust reconnues ;
- conserver dans Aithos les règles métier et de sécurité qui lui sont propres :
  profils connecteurs, politiques, custody Vault, preuves Gamma, restrictions de
  routes, approbations, durable state et cérémonie de délégation G4.

Les deux candidats principaux sont :

- `oauth2` pour Authorization Code, PKCE, échange de code, refresh et erreurs
  protocolaires ;
- `openidconnect` pour OIDC, ID Token, JWKS, validation de l'issuer, du nonce et
  des claims lorsque le fournisseur expose réellement OIDC.

Les versions, features Cargo, MSRV et licences seront figées pendant le spike de
dépendances. Ce document ne grave volontairement pas un numéro de version qui
pourrait devenir obsolète avant le démarrage.

## 2. Pourquoi ce chantier

Le code actuel n'est pas une implémentation naïve : il couvre déjà discovery,
DCR/CIMD, PKCE, state, callback, token, refresh, plusieurs méthodes
d'authentification client, stockage Vault et des limites fail-closed. Le risque
vient surtout du fait qu'Aithos porte lui-même une partie importante de la
machine protocolaire :

- surface à maintenir et à auditer plus grande ;
- interopérabilité à prouver fournisseur par fournisseur ;
- traitement des réponses et extensions OAuth à faire évoluer localement ;
- risque de courses ou de cas RFC rares dans le cycle de vie du state et des
  tokens ;
- coût de revue élevé pour chaque nouvelle variante OAuth/OIDC.

Le bénéfice attendu est donc une réduction du code protocolaire maison et une
meilleure interopérabilité, sans déplacer les secrets ni affaiblir les
invariants Aithos.

## 3. Frontière de responsabilité cible

| Responsabilité | Cible |
| --- | --- |
| Construction Authorization Code + PKCE | `oauth2` |
| Échange de code et refresh | `oauth2` |
| Parsing des erreurs OAuth standard | `oauth2` |
| Validation OIDC, ID Token, JWKS, nonce et claims | `openidconnect` |
| Discovery RFC 8414/9728 | adaptateur Aithos autour des types de bibliothèque, avec contrôles d'origine conservés |
| DCR RFC 7591 et CIMD | adaptateur Aithos tant que la couverture des bibliothèques n'est pas suffisante |
| Choix et validation du profil connecteur | Aithos |
| State durable, one-shot et anti-rejeu | Aithos |
| Custody des credentials et tokens dans Vault | Aithos |
| Approbations, restrictions de capabilities et preuve Gamma | Aithos |
| OAuth entrant et cérémonie G4 | lot séparé, migré seulement après l'amont |

Une bibliothèque ne devient jamais une source d'autorité métier. Elle produit
un résultat protocolaire que l'adaptateur Aithos revalide et lie au connecteur,
au compte, au tenant et au state durable attendus.

## 4. Invariants non négociables

La migration doit conserver au minimum :

1. PKCE S256 obligatoire pour les clients publics et chaque profil qui le
   réclame ;
2. `state` imprévisible, durable, lié à une seule tentative et consommé
   atomiquement avant tout effet réutilisable ;
3. redirect URI exacte, jamais dérivée d'une donnée reçue au callback ;
4. issuer, endpoints et protected resource liés aux origines autorisées ;
5. aucune valeur de token, secret, code, verifier ou réponse sensible dans les
   logs et les erreurs publiques ;
6. aucun token durable hors Vault ;
7. rotation du refresh token atomique, avec conservation de l'ancien état si la
   nouvelle écriture échoue ;
8. réponses réseau bornées avant parsing ;
9. timeouts, refus TLS hors loopback et politique de redirection HTTP explicite ;
10. séparation stricte entre identité OAuth/OIDC et autorité Aithos.

## 5. Découpage proposé

### OLR-0 — ADR, menace et dépendances

- cartographier les chemins OAuth entrant et amont ;
- figer MSRV, licences, features Cargo et politique de mises à jour ;
- écrire les vecteurs de compatibilité et les cas négatifs avant migration ;
- décider explicitement quelles parties de DCR/CIMD restent dans Aithos.

Sortie : ADR acceptée, graphe de dépendances audité et corpus de régression.

### OLR-1 — seam interne et tests de parité

- introduire une interface interne d'échange OAuth sans changer les routes
  publiques ni le schéma Vault ;
- encapsuler l'implémentation actuelle derrière cette interface ;
- rejouer les mêmes vecteurs sur l'ancien et le nouveau moteur ;
- ajouter des tests de concurrence sur state, callback et refresh.

Sortie : aucun changement fonctionnel, possibilité de bascule par profil en
test.

### OLR-2 — Authorization Code, PKCE et refresh via `oauth2`

- migrer d'abord un profil read-only de démonstration ;
- conserver les contrôles d'origine et les limites de réponse Aithos autour du
  client HTTP ;
- comparer byte-for-byte les paramètres utiles et les verdicts publics ;
- étendre ensuite aux méthodes d'authentification client nécessaires.

Sortie : un profil réel passe la suite locale et un live gate sans fallback.

### OLR-3 — OIDC via `openidconnect`

- activer uniquement pour les profils qui déclarent OIDC ;
- vérifier issuer, audience, signature, nonce, expiration et claims requis ;
- lier l'identité validée au compte connecteur sans en faire une autorité Aithos ;
- conserver UserInfo comme source bornée et explicitement optionnelle.

Sortie : compte OIDC réel validé, erreurs et claims hostiles couverts.

### OLR-4 — discovery, DCR et CIMD

- réutiliser les types de bibliothèque lorsqu'ils couvrent le contrat ;
- garder un adaptateur strict pour les extensions non couvertes ;
- accepter les champs RFC légitimes non utilisés sans accepter de changement
  d'issuer, d'endpoint ou de méthode d'authentification ;
- vérifier les fournisseurs qui réémettent des métadonnées supplémentaires.

Sortie : matrice d'interopérabilité documentée par fournisseur.

### OLR-5 — déploiement progressif

- bascule profil par profil, d'abord en environnement de démo ;
- métriques sans données sensibles sur les verdicts et les fallbacks ;
- rollback de configuration sans migration destructive du Vault ;
- suppression de l'ancien moteur seulement après les gates live et une fenêtre
  d'observation convenue.

### OLR-6 — OAuth entrant G3/G4

Le serveur OAuth entrant et la cérémonie G4 forment un chantier distinct du
client OAuth amont. Leur migration éventuelle ne commence qu'après stabilisation
du lot amont. Les bibliothèques clientes `oauth2` et `openidconnect` ne
remplaceraient pas à elles seules un Authorization Server ; un composant serveur
adapté devrait être évalué séparément, avec les mêmes exigences de state durable
et de délégation Aithos.

## 6. Gates de sortie

- tests unitaires et propriétés sur PKCE, state, callback, rotation et redaction ;
- corpus de réponses fournisseur hostiles et réponses RFC enrichies ;
- tests de concurrence prouvant le one-shot avant le token endpoint ;
- tests d'intégration avec Vault réel et serveurs OAuth de test ;
- au moins un live gate read-only par famille de fournisseur ciblée ;
- aucun changement de route publique, schéma Vault ou preuve Gamma non prévu par
  une migration explicite ;
- aucune dépendance à API instable ou feature par défaut inutile.

## 7. Charge indicative

Ordre de grandeur : **10 à 20 jours d'ingénierie**, soit environ **2 à 4 semaines
calendaires** avec revue et qualifications live. Le bas de la fourchette suppose
un premier périmètre limité à Authorization Code + PKCE + refresh pour un profil
read-only. DCR/CIMD multi-fournisseurs, OIDC complet et migration du serveur
entrant peuvent étendre le chantier au-delà.

Ce chantier ne doit pas retarder la première démo. La démo doit utiliser le code
actuel derrière une configuration bornée, puis fournir les observations réelles
qui alimenteront les vecteurs OLR-0.

# HANDOFF — Client, SDK et dashboard : intégration G4 sans régression

> **ARCHIVE — supplanté.** Les écarts CSD-1 à CSD-6 décrits ici ont été traités
> dans les lots Client/SDK/dashboard suivants. Le reliquat Gateway est suivi par
> `HANDOFF-GATEWAY-COMPAGNON-DEMO-INTEGREE-2026-07-22.md`.

**Date :** 2026-07-22

**Dépôts concernés :**

- `code/aithos-client` ;
- `code/aithos-sdk` ;
- `code/aithos-sdk-example`.

**Statut :** changements locaux utiles et testés, à attribuer puis intégrer dans
un contexte séparé. Ce handoff n'autorise aucune modification de `aithos-core`.

## 1. Verdict

Le développement observé est cohérent avec la direction G4 : il apporte la
reconnexion owner, l'identité publique, des primitives de mandat et une démo de
délégation locale. Il est principalement **additionnel**, mais il n'est pas encore
une intégration G4 de bout en bout.

Trois écarts empêchent de le qualifier tel quel :

1. le mandat d'action actuel ne porte que `Act` et ne peut donc pas être parent
   d'une session G4, qui exige aussi `Issue` avec une profondeur de délégation
   bornée ;
2. le SDK décrit honnêtement comme manquants la publication déterministe de la
   délégation, les événements Gamma vérifiés et la preuve de possession ;
3. le dashboard remplace la console provider/gateway existante par une démo locale
   et persiste des seeds de récupération bruts dans `localStorage`.

La bonne reprise consiste à préserver ces apports, ajouter une API G4 distincte,
et réintégrer la démo comme route séparée sans élargir ni casser le chemin actuel.

## 2. Baselines et changements à préserver

### `aithos-client`

- branche : `codex/client-sdk-v2-parking` ;
- HEAD observé : `e082ca6` ;
- 11 fichiers suivis modifiés : README, client Rust, WASM, tests navigateur,
  wrapper web, déclarations NPM et smoke test ;
- intention reconnue : reconnexion owner, export d'identité publique et émission
  de mandat d'action depuis les couches Rust/WASM/browser.

### `aithos-sdk`

- branche : `codex/g1-g7-enterprise-sdk` ;
- HEAD observé : `648e24b` ;
- 4 fichiers suivis modifiés, `docs/` non suivi et deux nouveaux tests ;
- intention reconnue : exposer le flux de délégation et documenter les
  capabilities réellement disponibles ou encore absentes.

### `aithos-sdk-example`

- branche : `codex/g1-g7-enterprise-dashboard` ;
- HEAD observé : `b1def67` ;
- 5 fichiers suivis modifiés et `app/demo-files.ts` non suivi ;
- intention reconnue : démonstrateur browser local de création/reconnexion et de
  fichiers de délégation.

Ne pas restaurer, stasher, reformater globalement ou réécrire ces changements.
Commencer par les attribuer, les relire et faire des commits étroits par dépôt.

## 3. Vérifications déjà vertes au 2026-07-22

Dans `aithos-client` :

- `cargo test --workspace` ;
- `cargo clippy --workspace --all-targets -- -D warnings` ;
- `cargo fmt --check` ;
- `./scripts/check-native.sh` ;
- `./scripts/build-browser.sh` ;
- `./scripts/smoke-npm.sh` ;
- `./scripts/check-secrets.sh`.

Dans `aithos-sdk`, `npm test` passe avec 17 tests.

Dans `aithos-sdk-example`, `npm test` passe avec 4 tests et `npm run build` est
vert.

Ces résultats prouvent la cohérence locale, pas encore la cérémonie G4 contre une
gateway réelle.

## 4. Frontières de compatibilité

- Ne modifier ni le wire Core, ni les Bundles, ni CB2/SC1/W1.1, ni la grammaire
  des mandats.
- Réutiliser les primitives génériques exposées par `aithos-wasm` : génération de
  clé déléguée, vérification de chaîne, sous-mandat de session et signature du
  challenge de cérémonie.
- Ne jamais réimplémenter en JavaScript la canonicalisation, la signature ou la
  vérification d'autorité.
- Ne jamais envoyer au backend une clé owner, une clé de délégué, une seed de
  récupération ou une clé de session.
- Ne pas élargir l'API existante de mandat d'action pour la rendre implicitement
  délégable. Ajouter une intention/API distincte de parent de session G4.
- Conserver la console provider/gateway et ses parcours G1/G7. La démo locale et
  la cérémonie deviennent des routes ou panneaux additionnels.
- Les capacités du SDK restent `missing` tant que le flux réel correspondant
  n'est pas implémenté et prouvé.

## 5. Plan d'action

### CSD-0 — attribution et commits de baseline

- Relire les trois diffs depuis les HEAD ci-dessus.
- Confirmer l'absence de secret réel et de fichier généré suivi.
- Corriger uniquement les défauts démontrés ci-dessous, puis faire un commit
  étroit dans chaque dépôt ; ne pas mélanger les historiques Git.
- Rejouer tous les tests du §3 avant la suite.

### CSD-1 — durcir les apports client actuels

- Dans le wrapper browser, garantir la zéroïsation des copies de seed même si
  l'import de la clé owner échoue avant l'entrée dans le bloc protégé.
- Ajouter un test de ce chemin d'erreur et des exports d'identité publique.
- Documenter clairement que l'émission actuelle crée un mandat d'action direct,
  pas un parent de session G4.
- Préserver les APIs existantes et leurs valeurs par défaut.

### CSD-2 — ajouter une intention de parent de session G4 distincte

- Exposer une API fermée qui demande explicitement : actions exactes,
  `Issue(depth=1)`, fenêtre temporelle, `max_sessions`, audience gateway et
  contraintes utiles.
- Refuser toute action ou audience implicite et toute profondeur supérieure.
- Retourner seulement le mandat signé, la clé publique et un plan de publication ;
  la clé privée reste dans le keyholder client.
- Tester que le mandat produit est accepté comme parent par
  `eligible_session_parents` et que le mandat d'action historique ne l'est pas.
- Si ce test exige un changement dans `aithos-core`, STOP : ouvrir un lot Core
  séparé et demander validation au lieu de contourner le contrat.

### CSD-3 — publication et discovery déterministes

- Définir où le certificat/mandat est publié dans les contrats existants
  provider/Gamma afin que la gateway le découvre et le vérifie.
- Exiger attribution, événement Gamma vérifié et preuve de possession ; ne pas
  déclarer le flux terminé sur la seule présence d'un fichier téléchargé.
- Fournir reprise idempotente, statut explicite et erreur exploitable sans contenu
  secret.

### CSD-4 — client de cérémonie G4

- Consommer les routes et primitives G4 existantes sans protocole propriétaire.
- Signer localement le challenge, construire le sous-mandat de session court et
  vérifier les éléments publics avant soumission.
- Utiliser un keystore chiffré ou un import explicite à chaque session ; aucune
  seed brute dans `localStorage`, `sessionStorage`, IndexedDB, URL ou logs.
- Couvrir expiration, replay, mauvaise gateway, mauvais challenge, révocation et
  restart.

### CSD-5 — orchestration SDK honnête

- Exposer des types et étapes explicites : reconnecter, préparer le parent,
  publier, attendre la vérification, lancer la cérémonie et suivre son statut.
- Mettre à jour les marqueurs de capability uniquement lorsque l'implémentation et
  les tests réels existent.
- Ne pas simuler un succès de publication, de Gamma ou de preuve de possession.
- Mettre à jour les références de baseline au commit réellement intégré.

### CSD-6 — dashboard additionnel

- Restaurer/préserver la page de console provider/gateway G1/G7.
- Placer la démo de délégation et la cérémonie sur une route/panneau séparé.
- Remplacer la persistance des `OwnerRecovery` et `GranteeRecovery` bruts par un
  keystore chiffré explicite ou un import à chaque session.
- Garder les fonctions existantes d'onboarding, de statut connecteur et de
  contrôle gateway accessibles et testées.
- Ne pas commencer simultanément OAC-6 du handoff OAuth : ce lot possède le
  dashboard jusqu'à stabilisation de ses interfaces SDK.

### CSD-7 — preuve end-to-end

Sur une gateway de démo réelle et avec des identités jetables :

1. créer/reconnecter une clé locale ;
2. émettre puis publier un parent de session owner correct ;
3. vérifier sa visibilité dans `eligible_session_parents` ;
4. franchir discovery, DCR, PKCE et la cérémonie ;
5. obtenir `tools/list` borné et réussir un appel de lecture autorisé ;
6. refuser un voisin avant tout effet amont ;
7. retrouver les preuves attribuées ;
8. redémarrer la gateway, puis prouver refresh et révocation.

Le credential de la gateway de démonstration reste dans Vault. Aucun token réel
n'entre dans les fixtures, le dépôt ou la sortie du test.

## 6. Gates de sortie

### Client

- tous les contrôles du §3 restent verts ;
- test d'échec d'import avec zéroïsation ;
- vecteurs du parent G4 et refus du mandat direct ;
- aucun secret détecté dans le package NPM ou le bundle browser.

### SDK

- suite actuelle et nouveaux tests d'orchestration verts ;
- aucune capability annoncée sans primitive réelle ;
- compatibilité des exports existants vérifiée.

### Dashboard

- build et tests verts ;
- console G1/G7 et nouvelle route toutes deux accessibles ;
- test/scanner prouvant l'absence de seed/token dans les storages et le HTML ;
- scénario E2E CSD-7 documenté et reproductible.

## 7. Parallélisation avec OAuth amont

OAC-0 à OAC-5 de
`HANDOFF-GATEWAY-OAUTH-CONNECTEURS-SAAS-2026-07-22.md` peuvent être réalisés en
parallèle : ils possèdent `aithos-core`, tandis que ce plan possède les trois
dépôts client.

La seule dépendance à séquencer est OAC-6, car elle modifie le dashboard et
consomme le SDK. Elle attend CSD-5/CSD-6 ou démarre ensuite depuis leurs commits.
Une évolution nécessaire du wire ou du Core est un STOP et un lot coordonné, pas
une modification opportuniste depuis le contexte client.

## 8. Conditions d'arrêt

STOP et demander revue si l'un des points suivants apparaît :

- modification nécessaire de `aithos-core`, du wire ou d'un profil de mandat ;
- clé ou seed brute persistée, envoyée ou loggée ;
- remplacement d'un parcours dashboard actuellement fonctionnel ;
- capacité SDK annoncée sans preuve réelle ;
- besoin de rendre un mandat historique plus permissif ;
- conflit d'ownership avec OAC-6.

## 9. Prompt de reprise

> Reprendre séparément `code/aithos-client`, `code/aithos-sdk` et
> `code/aithos-sdk-example` sur leurs branches actives, sans toucher à
> `aithos-core`. Lire intégralement
> `docs/HANDOFF-CLIENT-SDK-G4-INTEGRATION-2026-07-22.md` depuis `aithos-core`,
> attribuer les changements locaux listés au §2 et rejouer les gates du §3.
> Commencer par CSD-0 puis CSD-1 : corriger la zéroïsation sur échec d'import,
> conserver l'API de mandat d'action comme non délégable, et faire des commits
> étroits par dépôt. Ensuite ajouter une API distincte de parent de session G4,
> sans modifier le wire/Core. Préserver la console G1/G7 ; déplacer la démo locale
> sur une route séparée et ne persister aucune seed brute. Ne commencer OAC-6
> qu'après stabilisation du SDK/dashboard. STOP si une modification Core devient
> nécessaire.

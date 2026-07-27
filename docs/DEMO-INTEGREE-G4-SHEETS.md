# Démo navigateur intégrée — Provider, OAuth entrant, G4 et Google Sheets

Ce runbook couvre le dernier parcours de démonstration. Il ne contient aucun
secret et ne remplace pas le consentement humain Google.

## Frontières OAuth

- OAuth Google amont : pré-provisionne le profil `google-sheets-read`; ses
  client secret, PKCE, access token et refresh token restent dans Vault.
- OAuth entrant Gateway : commence depuis `/delegation`, découvre la resource
  `/mcp`, publie un parent G4 lié byte-exactement à cette audience, joue la
  cérémonie puis garde le bearer Aithos uniquement en mémoire.

## Préparation

1. Copier `demo/integrated/gateway.example.yaml` hors du dépôt et remplacer
   toutes les variables `${...}`. Ne jamais y injecter le secret Google.
2. Démarrer Vault, puis injecter séparément :
   - `AITHOS_DEMO_VAULT_TOKEN` pour le broker de connecteur ;
   - `AITHOS_DEMO_AS_VAULT_TOKEN` pour l'état OAuth entrant ;
   - le client secret Google dans le record Vault dérivé du connecteur.
3. Démarrer le Provider avec `AITHOS_STORE_ALLOWED_ORIGINS` égal à l'origine
   exacte du dashboard. Une liste séparée par des virgules est acceptée ; HTTP
   est limité au loopback.
4. Pré-provisionner l'Ethos `operations`, le manifeste compilé Sheets scellé,
   son pin, le mandat Agent du store distant, puis activer une instance nommée
   `sheets-safe` pour le seul compte Google jetable.
5. Utiliser uniquement le scope `spreadsheets.readonly`, un tableur jetable et
   la plage exacte déclarée dans `allowed_ranges`.

Le parent G4 créé par le Client porte la `resource` OAuth byte-exacte dans la
contrainte normative signée `purpose`. Elle est héritée sans changement par le
sous-mandat ; la Gateway exige son égalité exacte avec la `resource` OAuth à
`prepare`, `prepare-grant` et `complete`.

## Gate transport avant navigateur

Exporter les coordonnées publiques, puis lancer :

```sh
export AITHOS_DEMO_GATEWAY_URL=http://127.0.0.1:4870
export AITHOS_DEMO_PROVIDER_URL=http://127.0.0.1:4880
export AITHOS_DEMO_DASHBOARD_ORIGIN=http://127.0.0.1:3000
export AITHOS_DEMO_TENANT=demo
export AITHOS_DEMO_DID='did:aithos:z...'
./demo/integrated/preflight.sh
```

Le script vérifie les probes, les preflights OAuth/MCP, la lecture publique
Provider, la publication signée et le refus d'une origine voisine. Il ne fait
aucune mutation.

## Parcours navigateur

1. Démarrer `aithos-sdk-example`, ouvrir `/delegation` dans un navigateur
   standard et créer/reconnecter l'Owner ainsi que le délégué.
2. Dans les paramètres avancés, renseigner Provider, tenant et Gateway ; garder
   `operations`, `sheets-safe` et `read_range`.
3. Cliquer « Démarrer OAuth et publier le parent G4 ».
4. Fournir le fichier privé du délégué uniquement au moment de la cérémonie,
   puis lancer « Signer, échanger et lire Sheets ».
5. Observer successivement : parent vérifié, token en mémoire, `tools/list`,
   `read_range`, refus `write_range` en `-32001`, preuve Gamma vérifiée.
6. Se déconnecter et vérifier que le parcours exige une nouvelle cérémonie.

## Sentinelles et sortie

Avant le parcours, choisir trois chaînes sentinelles différentes pour le code
OAuth, le verifier et un faux bearer. Après le parcours, aucune ne doit être
présente dans l'URL, le HTML, la console, `localStorage`, `sessionStorage`,
IndexedDB ou Cache Storage. Les captures réseau ne doivent conserver que
méthode, origine, statut et taille, jamais les corps ni les headers secrets.

Un succès de démo n'est déclaré que si le refus `write_range` n'entraîne aucun
appel Google, que la preuve Gamma est vérifiée localement et que le logout
efface les handles OAuth, G4 et MCP.

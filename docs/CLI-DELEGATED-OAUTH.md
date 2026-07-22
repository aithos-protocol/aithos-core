# Cérémonie OAuth déléguée en CLI

La commande native exécute le même parcours cryptographique que la page WASM :
découverte OAuth, DCR, PKCE S256, vérification de la chaîne, construction de la
feuille de session, signature du grant Gamma, signature WYSIWYS, échange du code
et stockage privé des tokens.

La clé du délégué n'est jamais acceptée dans les arguments du processus. La
première intégration de signer lit exactement 32 octets Ed25519 encodés en
hexadécimal sur stdin. Une commande de custody peut donc les fournir par pipe
sans les afficher :

```bash
commande-custody-qui-ecrit-la-cle-sur-stdout | \
  aithos oauth authorize-delegated \
    --gateway https://entreprise.mcp.aithos.fr/mcp \
    --signer-stdin \
    --token-output ./aithos-oauth.json \
    --approve
```

`--approve` est obligatoire pour les gates scriptés. Immédiatement avant la
signature, la CLI affiche uniquement la présentation publique vérifiée :
gateway, client, resource, chaîne, contexte, périmètre, contraintes, fenêtre,
clés publiques et digest WYSIWYS. Aucun seed, code OAuth, vérificateur PKCE ou
token n'est écrit sur stdout ou stderr.

Si plusieurs mandats parents sont éligibles, sélectionner exactement celui
voulu :

```bash
commande-custody-qui-ecrit-la-cle-sur-stdout | \
  aithos oauth authorize-delegated \
    --gateway https://entreprise.mcp.aithos.fr \
    --signer-stdin \
    --context finance \
    --parent-id mandate_01J... \
    --token-output ./aithos-oauth.json \
    --approve
```

Le fichier `--token-output` est créé en mode exclusif et avec les permissions
`0600` sur Unix. La CLI refuse de remplacer un fichier existant. Il contient des
secrets OAuth et doit rester sous custody ; ne pas le committer, le journaliser
ou le transmettre à un modèle.

HTTP est accepté uniquement pour une gateway loopback. Toute gateway distante
doit utiliser HTTPS, et les endpoints annoncés par la découverte doivent rester
sur la même origine.

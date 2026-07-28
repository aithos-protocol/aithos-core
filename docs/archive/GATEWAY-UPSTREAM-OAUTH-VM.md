# Gateway — connexion OAuth amont sur VM

> **ARCHIVE OPÉRATIONNELLE.** Ce runbook décrit le socle OAuth amont initial.
> Pour les profils actuels Notion/Sheets/Gmail, utiliser
> `RUNBOOK-CONNECTOR-PROFILES-OAUTH-SAAS.md`.

Ce runbook couvre le premier gate du client OAuth amont : authorization code
+ PKCE, callback HTTPS public, secrets et tokens dans le Vault du client,
refresh automatique et refus fail-closed avant tout appel MCP.

Il ne choisit pas la cible Gmail du fork produit. Le premier témoin live peut
être n'importe quel upstream OAuth pour lequel le client possède une
application et les URLs d'autorisation/token. La découverte RFC 9728/8414 et
la registration dynamique restent un incrément ultérieur ; ce gate utilise
des URLs et un `client_id` explicites.

## 1. Préparer l'application OAuth

Créer l'application chez le fournisseur au nom du client. Pour une gateway
sur VM, déclarer une application Web avec exactement :

```text
https://<gateway-host>/oauth/callback
```

Demander uniquement les scopes nécessaires à l'upstream testé. Ne placer ni
client secret ni token dans le YAML, l'environnement du navigateur, un
argument CLI ou un fichier de runbook.

## 2. Écrire le client secret dans Vault

Le chemin du secret client et celui de l'état OAuth doivent être distincts :
le second est un record dédié que la gateway remplace lors du consentement et
des refresh.

```sh
vault kv put secret/aithos/oauth/protected-client \
  client_secret='<secret fourni par le fournisseur>'
```

La gateway écrira elle-même le champ `state` au chemin
`secret/aithos/oauth/protected-token`. Ce champ contient successivement le
PKCE/state en attente, puis le token set et son expiration. Il ne doit être lu
ou affiché par aucun runbook.

## 3. Configurer le serveur protégé

```yaml
listen: 127.0.0.1:4890

credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN

servers:
  - name: protected
    transport: http
    url: https://mcp.example/mcp
    oauth:
      auth_url: https://identity.example/authorize
      token_url: https://identity.example/token
      client_id: <identifiant-public>
      client_secret:
        broker: enterprise
        path: aithos/oauth/protected-client
        field: client_secret
      scopes:
        - resource.read
      redirect_uri: https://<gateway-host>/oauth/callback
      token_vault:
        broker: enterprise
        path: aithos/oauth/protected-token
        field: state

# contexts: et journal: restent ceux du hub déjà enrollé.
```

`oauth`, `credential` et `bearer_token` sont mutuellement exclusifs pour un
même serveur. HTTP en clair est accepté uniquement sur loopback ; les URLs
publiques OAuth et MCP doivent être HTTPS.

## 4. Démarrer le consentement

Option recommandée : arrêter momentanément la gateway, puis laisser la
commande owner servir le callback sur le même listener pendant cinq minutes.
Caddy continue de router le hostname public vers `127.0.0.1:4890`.

```sh
export AITHOS_VAULT_TOKEN='<token Vault de la VM>'

./aithos-gateway \
  --config /etc/aithos/gateway.yaml \
  owner-connect-oauth --server protected --wait-secs 300
```

La commande imprime uniquement l'URL publique de consentement. L'ouvrir dans
le navigateur de l'opérateur, consentir, puis vérifier le message générique
« OAuth connection established ». Aucun code ni token ne doit apparaître dans
la page ou la sortie CLI.

Mode en deux temps, utile si le callback doit être servi par le process
normal :

```sh
./aithos-gateway --config /etc/aithos/gateway.yaml \
  owner-connect-oauth --server protected --wait-secs 0

./aithos-gateway --config /etc/aithos/gateway.yaml \
  --identity /var/lib/aithos/agent.id run
```

Ouvrir ensuite l'URL imprimée. Après ce mode, redémarrer une fois la gateway :
le démarrage connecté rejoue alors la vérification du manifest MCP pinné.

## 5. Preuve live

Après connexion, démarrer la gateway et appeler un outil déjà couvert par le
mandat du contexte. Vérifier :

1. `tools/list` reste la surface locale pinnée et ne consulte pas Vault ;
2. l'appel autorisé produit l'acte Gamma avant la sortie ;
3. l'upstream reçoit un bearer, mais aucune réponse agent ne le contient ;
4. une expiration provoque exactement un refresh puis l'appel ;
5. révoquer/corrompre le refresh token dans Vault provoque un refus
   `upstream_oauth_unavailable` et zéro requête vers l'upstream ;
6. une recherche des valeurs sentinelles dans config, stores, Gamma, journal
   et sorties retourne zéro occurrence.

Le consentement réel et le témoin sur VM restent le geste de Mathieu. Les
tests CI emploient exclusivement un faux AS, un faux resource server et un
Vault en mémoire/loopback ; ils n'appellent aucun fournisseur externe.

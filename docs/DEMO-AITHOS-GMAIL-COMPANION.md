# Démo — connecteur compagnon Aithos Gmail

## Décision

La démo conserve deux connecteurs indépendants :

- `gmail` relaie le MCP Gmail officiel de Google ;
- `aithos-gmail` exécute dans la Gateway l'adaptateur REST compilé
  `gmail_send_guarded`.

Ils peuvent utiliser le même compte Google et le même client OAuth, mais
conservent des tokens séparés. `aithos-gmail` ne demande que `openid`, `email`
et `gmail.send`.

## Plan d'action

1. Sceller le manifeste compilé `aithos-gmail`.
2. Renseigner le pin, le client OAuth et l'unique destinataire autorisé dans la
   configuration de démo.
3. Démarrer Gateway et Vault.
4. Depuis le dashboard, choisir « Install Aithos Gmail », fournir le client
   secret et terminer le consentement Google.
5. Publier le binding Ethos puis activer le connecteur.
6. Faire demander `aithos-gmail__send_guarded` par l'agent.
7. Revoir, approuver puis dispatcher la demande depuis la surface Owner.
8. Qualifier un unique envoi réel vers la boîte de démonstration autorisée.

## Préparation du manifeste

La proposition locale ne contacte ni Google ni un autre upstream :

```sh
aithos-gateway owner-propose-compiled \
  --server aithos-gmail \
  --adapter gmail_send_guarded \
  --output /tmp/aithos-gmail.proposal.json
```

L'Owner l'enrôle ensuite avec une décision explicite :

```sh
aithos-gateway owner-enroll-server \
  --master-seed-hex <MASTER_SEED> \
  --label operations \
  --agent-pub <AGENT_PUB> \
  --gateway-pub <GATEWAY_PUB> \
  --proposal /tmp/aithos-gmail.proposal.json \
  --approve send_guarded=write:granted \
  --store-root <ETHOS_STORE>
```

La commande imprime `manifest_pin`. Reporter cette valeur dans
`AITHOS_DEMO_GMAIL_MANIFEST_PIN`.

## Variables publiques de configuration

Le template `demo/integrated/gateway.example.yaml` attend :

```text
AITHOS_DEMO_GMAIL_MANIFEST_PIN
AITHOS_DEMO_GMAIL_ALLOWED_RECIPIENT
AITHOS_DEMO_GOOGLE_CLIENT_ID
```

Le client secret Google ne doit jamais être injecté dans le YAML. Le dashboard
le transmet uniquement à la route Owner de la Gateway, qui le stocke dans
Vault avant de démarrer OAuth.

## Gate de démonstration

- un seul destinataire exact autorisé ;
- un seul destinataire par message ;
- texte brut uniquement ;
- aucune pièce jointe ;
- approbation Owner obligatoire ;
- aucun appel Gmail avant `approve` puis `dispatch` ;
- corps absent de Gamma et de la liste des approbations ;
- test réel exclusivement avec un compte et une boîte jetables.

L'API Owner expose :

```text
GET  /control/v1/connectors/aithos-gmail/approvals
GET  /control/v1/connectors/aithos-gmail/approvals/{approval}
POST /control/v1/connectors/aithos-gmail/approvals/{approval}/approve
POST /control/v1/connectors/aithos-gmail/approvals/{approval}/deny
POST /control/v1/connectors/aithos-gmail/approvals/{approval}/dispatch
```

La liste ne contient que les métadonnées et digests. Le contenu en clair n'est
retourné que par la revue Owner d'une demande précise.

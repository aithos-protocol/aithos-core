# Extension Aithos Gmail Send — architecture cible

**Statut :** design actif d'une extension REST optionnelle, préparé le 2026-07-21.

**Frontière :** ce document ne demande aucune logique spécifique dans le client
MCP générique. Gmail sert ici une API REST derrière une extension compilée et
déclarée ; les MCP OAuth restent consommés uniquement par leurs contrats
standards.

**Décision attendue :** valider le lot de développement décrit dans
`HANDOFF-GMAIL-SEND-EXTENSION.md` avant toute implémentation.
**Périmètre :** gateway uniquement ; aucune évolution du protocole Aithos Core.

## 1. Décision produit

La démo doit montrer qu'un agent peut demander l'envoi d'un e-mail depuis le
compte Google Workspace d'un salarié, tout en restant techniquement incapable
de contourner les garde-fous Aithos.

La solution n'est **pas** de modifier le serveur MCP Gmail de Google. Son
serveur officiel est un upstream tiers : il expose les opérations Gmail qu'il
supporte, notamment la création de brouillons, et reste inchangé.

La solution est d'ajouter au gateway une extension optionnelle,
`aithos-gmail`, qui :

1. enrichit la même réponse `tools/list` que les outils du MCP Google relayé ;
2. expose un outil distinctif, `aithos-gmail__send_guarded` ;
3. appelle directement Gmail REST API `users.messages.send` après décision
   Aithos ;
4. détient le token OAuth Google dans le coffre, jamais côté agent ;
5. applique mandat, politique, approbation humaine et journal Gamma avant
   l'effet externe.

L'outil affiché à l'agent porte donc explicitement la marque Aithos et ne peut
pas être confondu avec une capacité annoncée par Google.

```text
Agent (Claude, Codex, …)
          │  MCP Streamable HTTP
          ▼
     Aithos Gateway /mcp
     ├─ outils MCP Google relayés et pinnés
     ├─ aithos-gmail__send_guarded     ← extension Aithos
     ├─ mandat + bornes + politique
     ├─ Gamma : demande / refus / approbation / exécution
     └─ coffre OAuth Google
          │  Gmail API, scope gmail.send seulement
          ▼
       boîte Gmail de l'utilisateur
```

## 2. Surface MCP unifiée

Quand le serveur Google Gmail est enrollé sous l'identifiant `google-gmail` et
que l'extension est activée, le gateway sert par exemple :

```text
google-gmail__search_threads
google-gmail__get_thread
google-gmail__create_draft
aithos-gmail__send_guarded
```

Il ne s'agit pas d'un proxy qui injecte un faux outil dans la réponse du
serveur Google. C'est la surface agrégée du **gateway**, déjà reconstruite
depuis les manifests pinnés par le hub. Cette distinction préserve les garanties
existantes : l'upstream Google ne peut ni masquer ni modifier l'outil Aithos,
et inversement.

L'extension est un serveur synthétique réservé :

| Élément | Valeur v1 |
|---|---|
| ID interne / connecteur de mandat | `aithos-gmail` |
| Nom MCP exposé | `aithos-gmail__send_guarded` |
| Opération Aithos | `act.x.aithos-gmail.send_guarded` |
| Classe de risque | `write` / effet externe |
| Scope Google | `https://www.googleapis.com/auth/gmail.send` |
| Préfixe réservé | `aithos-gmail` |

L'extension ne doit pas demander `mail.google.com`, `gmail.readonly` ni
`gmail.compose` pour le chemin d'envoi de la démo. Le client OAuth de
production dédié à cette extension demandera uniquement `gmail.send`.

## 3. Contrat du geste d'envoi

### 3.1 Outil agent-facing

`aithos-gmail__send_guarded` prend au minimum :

```json
{
  "to": ["demo-recipient@example.test"],
  "subject": "Objet",
  "text_body": "Corps en texte brut",
  "cc": [],
  "bcc": []
}
```

Les pièces jointes, HTML libre, alias d'expéditeur, envoi groupé, planification
et réponses en thread sont hors v1. Ils sont des extensions de schéma et de
politique ultérieures, pas des échappatoires implicites.

### 3.2 Résultat déterministe

Avant tout appel à Google, le gateway normalise le contenu et calcule un
`payload_digest`. La décision est l'une des suivantes :

| Verdict | Effet |
|---|---|
| `denied` | aucune sortie réseau Google ; le refus est journalisé |
| `approval_required` | une demande chiffrée et liée au digest est créée ; rien n'est envoyé |
| `allowed` | le gateway envoie immédiatement puis journalise le résultat |
| `approved` | l'approbateur a validé le digest exact ; le gateway, pas l'agent, envoie |

Le chemin à approbation est central pour la démo : l'agent demande l'envoi,
l'approbateur voit le contenu exact et le contexte de politique, puis le
gateway exécute l'envoi. Il n'existe pas de jeton d'approbation réutilisable par
l'agent ; modifier un caractère crée un nouveau digest et une nouvelle
demande.

### 3.3 Politique minimale de démonstration

Le profil `demo-gmail-guarded` sera activé automatiquement **uniquement dans
la configuration de démonstration**. La production reste opt-in explicite.

```text
default: deny
autoriser: mandat act.x.aithos-gmail.send_guarded valide
destinataires: liste de démonstration ou domaines explicitement autorisés
volume: maximum 5 messages / 24 h / identité
cc + bcc: interdits
pièces jointes: interdites
horaires: fenêtre configurée
approbation: obligatoire pour toute adresse externe
audit: demande + verdict + digest + identités, jamais corps en clair dans Gamma
```

Les bornes déjà couvertes par le gateway (`one_of`, `max_items`, `forbid`,
`require`, etc.) doivent être réutilisées plutôt que contournées par une
politique propre à Gmail. Les règles spécifiques (domaines, volume, DLP,
approbation) sont exposées comme contraintes nommées et vérifiées
fail-closed.

## 4. Custodie OAuth et appel Gmail API

### 4.1 Onboarding

1. L'utilisateur se connecte à Google via le flux OAuth géré par Aithos.
2. Il consent au scope minimal `gmail.send`.
3. Le refresh token est écrit dans le coffre sous une référence non secrète,
   liée à l'identité Aithos et au connecteur `aithos-gmail`.
4. Le gateway ne conserve que cette référence ; l'agent ne reçoit ni client
   secret, ni refresh token, ni access token.

Pour la démo interne, le client OAuth doit être séparé du client expérimental
actuellement large et demander seulement ce scope. Le contrôle Workspace peut
alors être configuré autour d'une application interne connue.

### 4.2 Au moment de l'envoi

Après mandat, bornes, décision de politique et écriture Gamma :

1. le gateway lit le refresh token dans Vault ;
2. il obtient un access token éphémère auprès de Google ;
3. il construit un message MIME texte ;
4. il appelle `POST /gmail/v1/users/me/messages/send` ;
5. il expurge les secrets et journalise seulement les métadonnées et
   empreintes nécessaires à la preuve.

Un `OAuthTokenProvider` est distinct de `CredentialBroker` : le second est une
abstraction de secret générique déjà employée pour les bearers MCP, tandis que
le premier prend en charge le refresh OAuth, l'expiration et le scope attendu.
Le refresh token reste résolu par le même coffre et bénéficie de la même
discipline de redaction/zeroization.

## 5. Architecture d'extensions réutilisable

Le Gmail Send pack est le premier consommateur d'une petite surface stable,
interne au gateway. Il ne faut pas démarrer par le chargement dynamique de
bibliothèques arbitraires : cela créerait une nouvelle frontière d'exécution
non auditée. V1 est compilée et activable par configuration ; le contrat permet
ensuite un sidecar isolé ou un paquet signé.

### 5.1 Extension pack v1

Chaque pack implémente conceptuellement :

```text
ExtensionPack
  id()                 → id réservé et version
  manifest()           → outils, schémas, classes de risque, scopes requis
  validate_config()    → configuration stricte, sans secret
  tool_descriptors()   → contribution à tools/list
  invoke()             → exécution après l'autorisation du gateway
  health()             → état non secret du connecteur
```

Le `McpRouter` demeure le seul composant agent-facing. Il résout un nom exposé
vers soit un upstream MCP pinné, soit un pack local. Dans les deux cas la même
séquence reste obligatoire : résolution → pin/manifest → mandat → bornes et
politique → log-before-effect → exécution → résultat.

`core_bridge` reste l'unique porte vers `aithos-core` et `aithos-bundle`.
Les packs n'importent jamais ces crates directement.

### 5.2 Configuration cible

L'esquisse suivante est illustrative ; le schéma final doit rester
`deny_unknown_fields` et faire l'objet de contrats Gherkin avant code.

```yaml
extensions:
  - id: aithos-gmail
    enabled: true
    profile: demo-gmail-guarded
    oauth:
      provider: google-workspace
      credential:
        broker: vault
        path: oauth/google-workspace/mathieu/gmail-send
        field: refresh_token
    policy:
      approval: external_only
      allowed_recipient_domains: ["example.test"]
      max_messages_per_24h: 5
```

Les secrets ne figurent jamais dans ce YAML. Le pack est désactivé par défaut
hors profil de démo. Les noms de packs réservés ne peuvent pas être usurpés par
un `servers:` externe.

### 5.3 Détachabilité future

Le même contrat autorise trois modes sans changer le client MCP :

| Mode | Usage | Frontière de sécurité |
|---|---|---|
| pack compilé | démo et premier produit | dans le processus gateway |
| crate optionnelle | pack livré séparément mais compilé dans le binaire | mêmes garanties, version verrouillée |
| sidecar privé signé | intégrations lourdes / SDK fournisseur | le gateway garde mandat, politique, coffre et journal ; le sidecar ne reçoit qu'une capacité éphémère bornée |

Un pack ne doit jamais être exposé directement sur Internet comme MCP sans le
gateway : ce serait un bypass des mandats et de l'audit Aithos.

## 6. Modèle de preuve et sécurité

Pour chaque demande, le Gamma doit permettre de reconstituer :

```text
agent + identité Aithos
→ mandat couvrant act.x.aithos-gmail.send_guarded
→ arguments normalisés et payload_digest
→ politique + bornes évaluées
→ décision d'approbation éventuelle
→ identité de l'approbateur et digest validé
→ appel Gmail exécuté / refusé
→ message_id Gmail (si envoyé)
```

Le corps de mail, les adresses complètes et les tokens ne sont pas écrits en
clair dans Gamma. Le système conserve les informations nécessaires à la
preuve, avec pseudonymisation/hachage et rétention configurée ; le contenu de
revue est conservé chiffré dans l'outbox pour la seule durée nécessaire à
l'approbation.

Les contrôles indispensables sont :

- default-deny et outil caché si aucun mandat ne le couvre ;
- contrôle des domaines, alias et volumes avant l'effet ;
- protection contre la réutilisation : idempotency key + digest ;
- séparation demandeur / approbateur configurable ;
- aucune approbation après expiration ou changement de contenu ;
- expurgation stricte des erreurs OAuth et Gmail ;
- révocation du mandat ou du consentement OAuth effective avant toute nouvelle
  exécution.

## 7. Non-objectifs v1

- remplacer le MCP Gmail officiel ;
- donner à un agent un token Google ou un client secret ;
- contourner le consentement Workspace / les règles Admin ;
- ingérer la boîte mail entière pour l'envoi ;
- permettre la prospection de masse ;
- charger à runtime du code de plug-in non signé.

## 8. Extensions suivantes

Après Gmail Send, le même modèle doit pouvoir accueillir :

| Pack | Effet gouverné | Scope minimal à décider |
|---|---|---|
| `aithos-google-sheets` | mise à jour d'une plage autorisée | `spreadsheets` |
| `aithos-google-calendar` | création/modification d'événement | Calendar ciblé |
| `aithos-slack` | message sur canaux autorisés | Slack OAuth ciblé |

Chaque pack doit déclarer son manifest, sa politique, son onboarding OAuth et
ses scénarios de refus. Une bibliothèque d'extensions Aithos devient ainsi une
collection de **capacités à effet réel mais prouvées**, et non une liste de
connecteurs permissifs.

## 9. Références externes

- Gmail API : le scope `gmail.send` permet l'envoi au nom de l'utilisateur ;
  il est classé sensible.
  <https://developers.google.com/workspace/gmail/api/auth/scopes>
- Le MCP Gmail officiel est en Developer Preview et sa surface documentée
  crée des brouillons, sans outil d'envoi direct.
  <https://developers.google.com/workspace/gmail/api/guides/configure-mcp-server>
- Pour une application interne Workspace, Google prévoit une exemption de
  vérification OAuth publique, sous réserve de son usage interne et des règles
  administrateur.
  <https://support.google.com/cloud/answer/13464323>

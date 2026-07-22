# Handoff — Extension Aithos Gmail Send

**Date :** 2026-07-21

**État :** plan actif mais subordonné au socle OAuth générique OAC-0/OAC-3 ;
aucun code de l'extension n'est encore écrit.

**Document d'architecture :** `GMAIL-SEND-EXTENSION-ARCHITECTURE.md`.

**Dépôt :** `code/aithos-core`. L'instruction historique imposant la branche
`feat/obligations` est caduque : conserver la branche active attribuée au lot et
ne jamais la changer silencieusement.

**Contrainte de travail :** le worktree est déjà sale ; ne pas réécrire ni
stager les modifications étrangères.

## Objectif du premier incrément

Ajouter au gateway un pack facultatif `aithos-gmail` qui injecte
`aithos-gmail__send_guarded` dans sa surface MCP agrégée et envoie un e-mail
Gmail uniquement après :

```text
outil couvert par mandat
→ bornes Aithos satisfaites
→ politique Gmail satisfaite
→ log Gamma avant effet
→ approbation humaine si requise
→ Gmail API users.messages.send
```

L'agent ne reçoit jamais de secret Google. La démo doit pouvoir montrer un
refus lisible, une demande d'approbation, puis un envoi effectivement exécuté
par le gateway et prouvé dans le journal.

## Avant de coder

1. Lire intégralement :
   - `docs/GMAIL-SEND-EXTENSION-ARCHITECTURE.md` ;
   - `docs/HUB-MCP.md` ;
   - `docs/GATEWAY-HANDOFF.md` ;
   - `rust/crates/aithos-gateway/src/proxy_mcp.rs` ;
   - `rust/crates/aithos-gateway/src/credentials.rs`.
2. Relever l'état Git sans toucher aux modifications existantes.
3. Confirmer les décisions de produit encore ouvertes ci-dessous. Ne pas
   implémenter un comportement implicite à leur place.
4. Écrire les contrats Gherkin avant le code. Le projet suit explicitement le
   rituel « décisions → feature Gherkin → implémentation ».

## Décisions déjà prises

- Une seule surface agent-facing : `POST /mcp` du gateway.
- Les outils Google MCP relayés gardent leur namespace, par exemple
  `google-gmail__create_draft`.
- L'envoi Aithos est un outil **différent** :
  `aithos-gmail__send_guarded`.
- Le pack est une extension synthétique, pas une falsification de la réponse
  `tools/list` de Google.
- La politique et le mandat restent dans le gateway ; l'extension ne peut pas
  les contourner.
- V1 est un pack compilé et activé par configuration, pas un plug-in dynamique
  arbitraire.
- Gmail v1 demande seulement le scope `gmail.send` et utilise Gmail REST API.
- Le profil de démo l'active automatiquement ; hors démo, l'extension est
  inactive tant qu'elle n'est pas explicitement configurée.
- Les corps et secrets ne vont jamais dans Gamma en clair.

## Décisions à demander à Mathieu avant le lot d'effet réel

1. **Approbation v1 :** webhook/Slack, petite UI gateway, ou CLI owner ?
   Recommandation : endpoint/UI minimal gateway pour la démo, puis adaptateurs
   Slack/ServiceNow.
2. **Règle de démo :** approbation obligatoire pour tous les envois, ou
   seulement hors allowlist ? Recommandation : obligatoire pour tous, afin de
   rendre la preuve immédiatement visible.
3. **Compte expéditeur :** seul `mathieu@aithos.fr` en démo ?
   Recommandation : oui, mono-utilisateur au premier lot.
4. **Destinataires :** liste fermée de boîtes de démonstration ?
   Recommandation : oui, aucune adresse libre tant que les contrôles DLP,
   domaines et anti-abus ne sont pas testés.
5. **Rétention de l'outbox chiffrée :** durée et emplacement. Recommandation :
   effacement à l'issue de l'envoi/refus, avec durée maximale configurée.

## Plan de développement

### GSE-0 — contrat et modèle d'extension

**But :** créer la couture sans Gmail et sans effet externe.

- Définir un manifeste de pack : id, version, outils, schémas, classe de
  risque, contraintes et besoins OAuth déclaratifs.
- Introduire un registre d'extensions dans le gateway, détenu par le routeur.
- Réserver les ids de packs contre les serveurs externes et les collisions de
  noms exposés.
- Ajouter la configuration `extensions:` avec validation stricte et
  default-deny.
- Faire contribuer les packs activés à `tools/list`, en utilisant exactement
  la même dérivation de surface et le même contrôle de mandat que les outils
  hub.

**Contrats à écrire :**

- pack absent : outil non listé et appel refusé ;
- pack activé sans mandat : outil non listé ;
- pack activé et mandat couvrant : outil listé avec schéma pinné ;
- collision d'id ou de nom : config refusée ;
- révocation : outil retiré de la surface sans redémarrage ;
- un refus de pack reste journalisé par l'identité gateway.

**Critère de sortie :** extension factice sans I/O, scénarios verts et aucune
régression du hub.

### GSE-1 — politique de sortie et outbox d'approbation

**But :** transformer un effet `write` en opération contrôlée, sans Google.

- Définir `SendRequest` canonique (destinataires, sujet, texte, cc/bcc) et
  validation fail-closed.
- Normaliser et hasher la charge avant toute décision.
- Ajouter une politique `gmail_guarded` : allowlist, domaines, volume,
  interdictions cc/bcc/attachments et approbation obligatoire/configurable.
- Ajouter une outbox chiffrée courte durée, l'idempotence et le cycle
  `pending → approved|denied|expired → dispatched|failed`.
- Exposer le mécanisme d'approbation uniquement à l'owner/approbateur ; il ne
  doit pas être une capacité de l'agent.
- Journaliser demande, verdict, identité d'approbateur et digests, sans corps.

**Critères de sortie :**

- l'agent ne peut pas modifier une charge approuvée ;
- l'agent ne peut pas réutiliser une approbation ;
- appel hors domaine ou au-delà du quota refusé avant I/O ;
- chaque transition est traçable et testée avec une horloge injectée.

### GSE-2 — OAuth Google et adaptateur Gmail API

**But :** brancher l'effet réel, sans élargir la surface.

- Créer un `OAuthTokenProvider` séparé de `CredentialBroker` ; le refresh
  token est lu dans Vault et l'access token est éphémère.
- Ajouter l'onboarding OAuth et l'association identité Aithos → référence
  Vault, sans jamais afficher ou sérialiser les tokens.
- Implémenter un client Gmail REST limité à `users.messages.send` et un MIME
  texte minimal.
- Traiter erreurs, timeouts, idempotence et redaction ; ne jamais copier les
  corps Google dans les erreurs ou logs.
- Connecter le dispatch de l'outbox au client Gmail seulement après décision et
  log-before-effect.

**Critères de sortie :**

- faux serveur OAuth/Gmail en tests réseau ;
- appel Gmail absent pour tout refus ou approbation en attente ;
- appel Gmail unique pour une approbation valide ;
- token absent du traffic agent, des logs et des exports ;
- `message_id` de résultat relié au digest dans l'audit.

### GSE-3 — profil de démonstration et répétition

**But :** une démo reproductible à Mathieu.

- Ajouter `demo-gmail-guarded` : mono-expéditeur, liste fermée,
  approbation obligatoire, quota cinq/jour.
- Ajouter un faux Gmail local pour la CI et un runbook avec le vrai compte
  Workspace séparé.
- Répéter trois scénarios : refus, approbation puis envoi, révocation entre
  demande et approbation.
- Documenter le bootstrap Google Cloud : client OAuth interne distinct,
  scope `gmail.send`, redirect URI de la gateway, secret/refresh token Vault.

**Gate final :** un agent MCP réel voit l'outil Aithos, demande un envoi ; un
approbateur valide ; le message arrive dans la boîte de démo ; l'auditeur peut
vérifier le mandat, le digest, la décision et l'exécution sans lire le corps.

## Structure de code proposée

Le découpage exact est à décider au début de GSE-0 ; direction recommandée :

```text
rust/crates/aithos-gateway/src/
  extensions.rs             # trait, manifest, registre et config commune
  extensions/
    mod.rs
    gmail.rs                # pack, sans secret
  gmail/
    policy.rs               # règles spécifiques et digest
    approval.rs             # outbox et machine d'états
    oauth.rs                # token provider + redaction
    api.rs                  # client Gmail REST limité
  proxy_mcp.rs              # résolution pack + upstream sous le même mur
```

Ne pas faire importer les packs par `aithos-core` ou `aithos-bundle` :
`core_bridge` est l'unique couture vers ces crates. Ne pas exposer un nouveau
listener MCP pour Gmail : il contournerait `McpRouter`.

## Vérifications obligatoires par lot

```text
cargo fmt --check --manifest-path rust/Cargo.toml
cargo test -p aithos-gateway --manifest-path rust/Cargo.toml
cargo clippy -p aithos-gateway --all-targets -- -D warnings
```

Ajouter des tests Cucumber et un test e2e réseau pour tout effet nouveau. Les
tests qui parlent à Google utilisent des doubles locaux ; aucun test CI ne
requiert un token réel.

## Hors périmètre initial

- lecture de boîte Gmail, réponse à un thread, HTML riche, pièces jointes ;
- envoi en masse ou prospection ;
- Calendar/Sheets, qui réutiliseront le contrat de pack après GSE-0 ;
- plug-ins binaires chargés dynamiquement ;
- bypass direct de la gateway par un client Claude/Codex.

## Prompt de reprise / lancement

> Reprendre `code/aithos-core` sur la branche active sans la changer. Lire
> `docs/HANDOFF-GMAIL-SEND-EXTENSION.md` et
> `docs/GMAIL-SEND-EXTENSION-ARCHITECTURE.md` intégralement. Commencer par
> GSE-0 seulement : ne pas appeler Google, ne pas créer de client OAuth et ne
> pas implémenter un envoi réel. Préserver les modifications étrangères du
> worktree. Écrire et faire valider les contrats Gherkin pour le registre de
> packs, la surface MCP dérivée, les collisions, le default-deny et la
> révocation à chaud ; puis implémenter le plus petit modèle d'extension et
> vérifier fmt, tests et clippy. Rapporter les décisions bloquantes avant tout
> passage à GSE-1.

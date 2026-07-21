# Handoff — OAuth amont gateway, gate local vert

**Date :** 2026-07-21
**Branche :** `codex/publish-aithos-core-busl`
**État :** code et contrats locaux verts ; preuve live VM à faire par Mathieu.

## Livré

- configuration stricte `servers[].oauth`, exclusive de `credential` et
  `bearer_token` ; URLs publiques, `client_id`, scopes et références Vault
  seulement ;
- client authorization code + PKCE S256, state CSRF et pending flow expirant
  après dix minutes ;
- client secret, pending verifier, access token, refresh token et expiration
  détenus dans le Vault du client ; support d'écriture d'un record KV v2
  dédié, avec erreurs expurgées ;
- callback `GET /oauth/callback` sur le listener gateway, réponse HTML
  générique sans code/token ;
- commande `owner-connect-oauth --server <id> [--wait-secs N]` ;
- injection bearer au dernier moment dans `HttpUpstream`, refresh sérialisé,
  contrôle des scopes requis et refus avant toute requête upstream ;
- démarrage possible avec un OAuth non connecté pour servir le callback, mais
  serveur marqué en drift/refusé jusqu'à connexion et redémarrage vérifié ;
- runbook VM : `docs/GATEWAY-UPSTREAM-OAUTH-VM.md`.

## Contrats et preuves locales

`tests/features/gateway-upstream-oauth.feature` ajoute 7 scénarios : config
sans secret et modes exclusifs, URL consent exacte, PKCE dans Vault, callback
et stockage, access bearer wire-only, refresh/rotation, refresh refusé avec
zéro requête resource et zéro fuite malgré un corps AS adversarial.

Gates passés après implémentation :

```text
cargo fmt -p aithos-gateway -- --check                         VERT
cargo test -p aithos-gateway                                  VERT
  lib                                                         99/99
  Cucumber                                                    159/159, 818/818
  e2e_demo_lea                                                2/2
  autres e2e/owner/policy                                     VERTS
cargo clippy -p aithos-gateway --all-targets -- -D warnings   VERT
```

Le `cargo fmt --all -- --check` global reste rouge uniquement sur trois
fichiers étrangers au lot, dont l'état préexistait à cette reprise
(`aithos-bundle/src/bundle.rs`,
`aithos-provider/src/bin/store_admin.rs`,
`aithos-provider/tests/cucumber_relay.rs`). Ils n'ont pas été reformattés afin
de préserver le worktree partagé.

## À faire chez Mathieu

1. choisir un upstream OAuth témoin et créer l'application appartenant au
   client ;
2. configurer le callback HTTPS public de la VM et les deux chemins Vault
   distincts ;
3. jouer `owner-connect-oauth`, consentir dans le navigateur, puis démarrer la
   gateway ;
4. prouver un appel MCP sous mandat, un refresh, puis un refresh cassé avec
   zéro sortie upstream ;
5. capturer le témoin anti-fuite sur config/stores/Gamma/journal/sorties.

## Bornes conservées

- aucune cible Gmail particulière n'a été codée ; le fork MCP hébergé versus
  serveur Gmail auto-hébergé reste à trancher avant cette cible ;
- aucune découverte RFC 9728/8414 ni registration dynamique RFC 7591 : URLs et
  client id explicites pour ce premier gate ;
- aucun token réel, appel Google ou secret externe n'a été utilisé en CI ;
- aucun commit de clôture n'a été créé : le worktree contient toujours des
  modifications étrangères et le geste de commit reste à Mathieu.

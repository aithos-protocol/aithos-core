# OLR-6 — bornage (pas d'implémentation sur cette branche)

Date : 2026-07-22
Statut : **borné, non démarré**

## Pourquoi ce lot est séparé

L'OAuth **entrant** (AS `gateway_as` G3 + cérémonie G4) n'est pas couvert par
les crates clientes `oauth2` / `openidconnect`. Remplacer un Authorization
Server exige un composant serveur dédié, le state durable AS, et la délégation
Aithos.

## Ce que cette branche ne fait PAS

- ne modifie pas le cœur de `oauth.rs` / AS entrant
- ne change pas les routes publiques `/authorize`, `/token`, DCR entrant
- ne touche pas le provider / relai `aithos.fr`

## Préparation pour une future branche

1. Évaluer un AS Rust (ou conserver l'AS maison avec surface RFC réduite)
2. Conserver les invariants : state one-shot, PKCE S256, redaction, Gamma
3. Brancher `feature/olr-oauth-libs-inbound` **après** stabilisation amont OLR-5
4. Corpus séparé : `gateway-oauth.feature` / `gateway-oauth-durable.feature`

## Gate d'ouverture OLR-6

- OLR-3→5 verts en local
- au moins un live gate amont read-only
- ADR serveur entrant acceptée explicitement

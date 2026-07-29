# OLR-4 — matrice discovery / DCR / CIMD

Date : 2026-07-22
Branche d'intégration : `codex/integrate-olr-oauth-libs`

## Décision (rappel ADR)

| Pièce | Cible |
| --- | --- |
| Discovery RFC 8414/9728 | adaptateur Aithos (contrôles d'origine) |
| DCR RFC 7591 / CIMD | adaptateur Aithos |
| `jwks_uri` AS metadata | validé même origine, comparé au pin du profil et utilisé via `ResolvedOAuthEndpoints` |
| Champs RFC non utilisés | acceptés (serde ignore) sans changer issuer/endpoints/auth |

## Matrice interop (locale)

| Fournisseur / harness | Discovery | DCR | JWKS | Notes |
| --- | --- | --- | --- | --- |
| Fake AS cucumber `gateway-upstream-oauth` | oui | oui | n/a | ancre permanente |
| Fake OIDC `olr_oidc_e2e` | oui + `jwks_uri` + champ vendor ignoré | n/a | oui | OLR-3/4 |
| Notion MCP hébergé | live runbook existant | public PKCE | selon IdP | hors AWS provider |

## Non-régression

- issuer / endpoint / auth method pins inchangés
- `jwks_uri` absent, hors origine ou différent du pin profil → fail-closed
- metadata hostile (taille, timeout, origine, issuer) → fail-closed
- registration defects → fail-closed avant consent

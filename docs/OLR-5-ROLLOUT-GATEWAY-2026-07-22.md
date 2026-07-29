# OLR-5 — déploiement progressif (gateway only)

Date : 2026-07-22

## Périmètre

Rollout du moteur amont `oauth2` / OIDC **sur la gateway uniquement**.
Aucun changement Terraform / Fargate / `aithos.fr`.

La qualification historique du prototype était verte au commit `608b392`.
Avant toute bascule, la suite locale complète doit être rejouée sur le commit
courant de `codex/integrate-olr-oauth-libs` et ses résultats archivés. Le live
gate Notion/Gmail reste une condition de bascule, pas une condition de
compilation.

## Qualification d'intégration locale — 2026-07-29

- `cargo test -p aithos-gateway --features olr-oauth-libs` : vert ;
- Cucumber : **299 scénarios / 1 422 étapes**, tous verts ;
- `oac_protocol` : **16/16**, dont refresh CAS inter-instance et panne du
  commit Vault final ;
- OAuth2 E2E : **4/4** ; OIDC E2E : **4/4**, dont issuer, audience,
  expiration, signature, nonce et taille JWKS hostiles ;
- `cargo clippy -p aithos-gateway --all-targets --features olr-oauth-libs
  -- -D warnings` : vert ;
- compilation `--all-targets` sans la feature, format ciblé des fichiers
  modifiés et `git diff --check` : verts.

Cette qualification est locale et ne remplace pas les gates live ci-dessous.

## Étapes

1. **Démo** — `protocol_engine: oauth2` sur un profil read-only
2. Observer `upstream_oauth` dans `GET /control/v1/status` (compteurs sans secrets)
3. Étendre profil par profil
4. **Rollback** — remettre `protocol_engine: native` (ou unset env) ; Vault intact
5. Retirer le moteur native seulement après fenêtre d'observation convenue

Le protocole de mise en production, les preuves attendues et le rollback exact
sont détaillés dans
`docs/RUNBOOK-OLR-PROD-ROLLOUT-ROLLBACK-2026-07-22.md`.

## Bascule

```yaml
oauth:
  protocol_engine: oauth2   # défaut: native
```

ou :

```bash
export AITHOS_UPSTREAM_OAUTH_ENGINE=oauth2
```

OIDC (OLR-3) :

```yaml
account_binding:
  issuer: https://issuer.example
  source:
    kind: id_token
    jwks_uri: https://issuer.example/jwks
    audience: my-client-id
  subject_field: sub
  account_field: email
```

## Rollback

Sans migration Vault : changer la config / l'env et redémarrer la gateway.

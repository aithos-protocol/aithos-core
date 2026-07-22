# OLR-5 — déploiement progressif (gateway only)

Date : 2026-07-22

## Périmètre

Rollout du moteur amont `oauth2` / OIDC **sur la gateway uniquement**.
Aucun changement Terraform / Fargate / `aithos.fr`.

La suite locale de sortie est verte au commit `608b392` : package Gateway
complet avec `--features olr-oauth-libs`, 296 scénarios Cucumber / 1406 étapes,
`clippy -D warnings`, format et build release. Le live gate Notion/Gmail reste
une condition de bascule, pas une condition de compilation.

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

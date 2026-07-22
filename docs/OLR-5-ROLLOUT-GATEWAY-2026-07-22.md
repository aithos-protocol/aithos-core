# OLR-5 — déploiement progressif (gateway only)

Date : 2026-07-22

## Périmètre

Rollout du moteur amont `oauth2` / OIDC **sur la gateway uniquement**.
Aucun changement Terraform / Fargate / `aithos.fr`.

## Étapes

1. **Démo** — `protocol_engine: oauth2` sur un profil read-only
2. Observer `upstream_oauth` dans `GET /control/v1/status` (compteurs sans secrets)
3. Étendre profil par profil
4. **Rollback** — remettre `protocol_engine: native` (ou unset env) ; Vault intact
5. Retirer le moteur native seulement après fenêtre d'observation convenue

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

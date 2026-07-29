# Runbook — suite locale OLR OAuth libs (amont)

Date : 2026-07-22
Branche d'intégration : `codex/integrate-olr-oauth-libs`

## Ce qui est livré

- Seam `oauth_protocol` + moteurs `native` (défaut) et `oauth2`
- Bascule config : `servers[].oauth.protocol_engine: oauth2`
- Bascule process : `AITHOS_UPSTREAM_OAUTH_ENGINE=oauth2`
- E2E loopback sans Vault réel ni provider `aithos.fr`

## Lancer la suite

```bash
cd /chemin/vers/aithos-core
git switch codex/integrate-olr-oauth-libs
chmod +x scripts/run-olr-oauth-local.sh
./scripts/run-olr-oauth-local.sh
```

Ou à la main :

```bash
cd rust
cargo test -p aithos-gateway --features olr-oauth-libs oauth_protocol --lib
cargo test -p aithos-gateway --features olr-oauth-libs \
  --test olr_oauth2_e2e -- --nocapture
cargo test -p aithos-gateway --features olr-oauth-libs \
  --test oac_protocol -- --nocapture
cargo test -p aithos-gateway --features olr-oauth-libs \
  --test olr_oidc_e2e -- --nocapture
```

Cucumber historique (moteur native, optionnel) :

```bash
OLR_RUN_CUCUMBER=1 ./scripts/run-olr-oauth-local.sh
# ou :
cargo test -p aithos-gateway --test cucumber -- \
  --input tests/features/gateway-upstream-oauth.feature
```

## Scénarios E2E couverts (`olr_oauth2_e2e`)

1. Consent PKCE → callback `oauth2` → Vault connected → refresh → bearer resource
2. Client public + refresh `invalid_grant` fail-closed / `reauth_required`
3. Override env `AITHOS_UPSTREAM_OAUTH_ENGINE=oauth2` malgré config `native`
4. PKCE S256 RFC 7636 appendix B
5. OIDC : issuer, audience, signature, nonce, expiration et taille JWKS hostiles
6. Refresh multi-instance : une seule rotation par lease CAS Vault
7. Commit Vault perdu après rotation : aucun bearer et reprise fail-closed

Le callback consomme le pending par CAS avant l'échange. Vault KV v2 est
supporté nativement ; un broker OAuth alternatif doit implémenter
`CredentialBroker::compare_and_store` ou le callback refuse fail-closed.

## Activer oauth2 sur un hub local

```yaml
servers:
  - name: protected
    oauth:
      protocol_engine: oauth2   # défaut: native
      auth_url: ...
      token_url: ...
      # reste inchangé (Vault, scopes, redirect_uri…)
```

## Hors scope de cette suite

- Provider / relai `aithos.fr`
- OAuth entrant G3/G4 (OLR-6)
- Live Google / OIDC réel (gate OLR-5)

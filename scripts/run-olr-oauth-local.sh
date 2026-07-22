#!/usr/bin/env bash
# Local OLR suite (OLR-0 → OLR-5), isolated clone.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/rust"

echo "== unit: oauth_protocol / oauth_oidc / oauth_rollout =="
cargo test -p aithos-gateway --features olr-oauth-libs oauth_protocol --lib
cargo test -p aithos-gateway --features olr-oauth-libs oauth_oidc --lib
cargo test -p aithos-gateway --features olr-oauth-libs oauth_rollout --lib

echo "== e2e: oauth2 engine =="
cargo test -p aithos-gateway --features olr-oauth-libs --test olr_oauth2_e2e

echo "== e2e: oidc + discovery jwks =="
cargo test -p aithos-gateway --features olr-oauth-libs --test olr_oidc_e2e

echo "== optional cucumber upstream =="
if [[ "${OLR_RUN_CUCUMBER:-0}" == "1" ]]; then
  cargo test -p aithos-gateway --test cucumber -- \
    --input tests/features/gateway-upstream-oauth.feature
fi

echo "OK — OLR local suite green (OLR-6 bounded only, not implemented)"

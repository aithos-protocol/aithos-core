#!/bin/sh
set -eu

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing command: $1" >&2
    exit 1
  }
}

required() {
  eval "value=\${$1-}"
  test -n "$value" || {
    echo "missing environment variable: $1" >&2
    exit 1
  }
}

header_value() {
  tr -d '\r' <"$1" | awk -F': *' -v wanted="$2" '
    tolower($1) == tolower(wanted) { print $2 }
  ' | tail -n 1
}

expect_status() {
  actual=$(awk 'toupper($1) ~ /^HTTP\// { code=$2 } END { print code }' "$1")
  test "$actual" = "$2" || {
    echo "unexpected HTTP status: got ${actual:-none}, wanted $2" >&2
    exit 1
  }
}

expect_origin() {
  actual=$(header_value "$1" access-control-allow-origin)
  test "$actual" = "$2" || {
    echo "unexpected Access-Control-Allow-Origin: got ${actual:-none}, wanted $2" >&2
    exit 1
  }
  test -z "$(header_value "$1" access-control-allow-credentials)" || {
    echo "Access-Control-Allow-Credentials must be absent" >&2
    exit 1
  }
}

need curl
need awk
need tr
need tail
required AITHOS_DEMO_GATEWAY_URL
required AITHOS_DEMO_PROVIDER_URL
required AITHOS_DEMO_DASHBOARD_ORIGIN
required AITHOS_DEMO_TENANT
required AITHOS_DEMO_DID

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

curl -fsS -o /dev/null "${AITHOS_DEMO_PROVIDER_URL%/}/healthz"
curl -fsS -o /dev/null \
  "${AITHOS_DEMO_GATEWAY_URL%/}/.well-known/oauth-protected-resource"

curl -sS -D "$tmp/oauth" -o /dev/null -X OPTIONS \
  "${AITHOS_DEMO_GATEWAY_URL%/}/ceremony/prepare" \
  -H "Origin: $AITHOS_DEMO_DASHBOARD_ORIGIN" \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: Accept, Content-Type'
expect_status "$tmp/oauth" 204
expect_origin "$tmp/oauth" "$AITHOS_DEMO_DASHBOARD_ORIGIN"

curl -sS -D "$tmp/mcp" -o /dev/null -X OPTIONS \
  "${AITHOS_DEMO_GATEWAY_URL%/}/mcp" \
  -H "Origin: $AITHOS_DEMO_DASHBOARD_ORIGIN" \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: Accept, Authorization, Content-Type, MCP-Protocol-Version, MCP-Session-Id'
expect_status "$tmp/mcp" 204
expect_origin "$tmp/mcp" "$AITHOS_DEMO_DASHBOARD_ORIGIN"

object="${AITHOS_DEMO_PROVIDER_URL%/}/t/$AITHOS_DEMO_TENANT/$AITHOS_DEMO_DID/did.json"
curl -sS -D "$tmp/public" -o /dev/null -X OPTIONS "$object" \
  -H "Origin: $AITHOS_DEMO_DASHBOARD_ORIGIN" \
  -H 'Access-Control-Request-Method: GET' \
  -H 'Access-Control-Request-Headers: X-Aithos-Store'
expect_status "$tmp/public" 204
expect_origin "$tmp/public" '*'

manifest="${AITHOS_DEMO_PROVIDER_URL%/}/t/$AITHOS_DEMO_TENANT/$AITHOS_DEMO_DID/manifest.json"
curl -sS -D "$tmp/publication" -o /dev/null -X OPTIONS "$manifest" \
  -H "Origin: $AITHOS_DEMO_DASHBOARD_ORIGIN" \
  -H 'Access-Control-Request-Method: PUT' \
  -H 'Access-Control-Request-Headers: Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store'
expect_status "$tmp/publication" 204
expect_origin "$tmp/publication" "$AITHOS_DEMO_DASHBOARD_ORIGIN"

curl -sS -D "$tmp/neighbor" -o /dev/null -X OPTIONS "$manifest" \
  -H 'Origin: https://neighbor.invalid' \
  -H 'Access-Control-Request-Method: PUT' \
  -H 'Access-Control-Request-Headers: Content-Type, If-Head, X-Aithos-Auth, X-Aithos-Store'
expect_status "$tmp/neighbor" 403
test -z "$(header_value "$tmp/neighbor" access-control-allow-origin)" || {
  echo "neighbor origin was reflected" >&2
  exit 1
}

echo "integrated demo transport preflight: OK"

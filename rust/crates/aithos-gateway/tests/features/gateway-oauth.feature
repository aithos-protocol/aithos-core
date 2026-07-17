Feature: OAuth authorization server on the hub — gateway_as (lot G3)
  The external consumer arrives (Claude custom connector): the endpoint
  becomes authenticated. The AS is SERVED BY THE GATEWAY, never by
  Aithos (INFRA §5 — a provider-side AS could fabricate sessions), and
  it rides the SAME listener as /mcp (the G2 shell precedent). This
  contract pins the STANDARDS-COMPAT C1 chantier against MCP authz
  2025-11-25: RFC 9728 protected resource metadata, RFC 8414 AS
  metadata, RFC 7591 dynamic registration, PKCE S256 only, RFC 8707
  resource indicators (resource required at /authorize AND /token,
  audience in the token), OAuth 2.1 refresh rotation. CIMD (SEP-991)
  is DEFERRED to its own slice: a URL-shaped or unknown client_id is
  refused naming dynamic registration as the supported path.
  Decisions Mathieu (AskUserQuestion, 2026-07-17): (1) pre-G4 the
  token binds to the CONTEXT AGENT CHAINS (the G6 precedent — scan of
  the certificates) through an INJECTABLE resolution seam: G4/G5 swap
  in session sub-mandate chains without touching the AS; (2) access
  tokens are hand-rolled EdDSA JWTs (compact JWS, ed25519 under the
  ADAPTER KEY — an ordinary gateway secret, NEVER a protocol object;
  zero new dependencies, the lockfile belongs to the P track); (3) the
  adapter key is born at the first `run` with `as:` active — a 0600
  file beside the identity (path configurable, default `as.key`), from
  the injected EntropySource, rotated by replacing the file; (4) the
  consent is a DEV-marked one-click Approve page naming client_id and
  resource — the honest pre-G4 minimum, replaced by the G4 ceremony;
  (5) the `as:` stanza is OPT-IN — absent, the gateway is
  byte-identical (the whole existing suite is that gate) — requires
  the multi-context shape, and requires an explicit `issuer` (http on
  loopback only, https elsewhere — the Vault broker rule); (6) default
  TTLs: access 3600 s, refresh 7 days (comfort over churn), both
  configurable and STRUCTURALLY capped by the bound chain's
  `not_after` — a refresh never survives its authority, past it the
  flow restarts; (7) DCR is OPEN to PUBLIC PKCE clients only (token
  endpoint auth `none`, no secret ever issued), redirect_uris against
  a BUILT-IN allowlist — https://claude.ai/api/mcp/auth_callback exact
  plus http://localhost:*/http://127.0.0.1:* any port (RFC 8252) —
  extensible via `redirect_allowlist`, everything else refused
  pedagogically. Invariants riding every scenario: a token is NEVER an
  authority — the mandate chain is re-verified at every act, so a
  revocation outruns any unexpired token; issuance is an act, never
  silent (one journal governance entry per minting); authorization
  codes are one-shot and PKCE-bound; refresh tokens rotate one-shot
  and a reuse cuts the whole family; no token, code or key byte ever
  lands in a log, an error body or a gamma payload; on /mcp the order
  stays Origin (403) → bearer (401 + WWW-Authenticate pointing the
  resource metadata) → body shape → JSON-RPC, and behind a valid
  token the existing pipeline (authorize, bounds, log-before-relay)
  is UNTOUCHED.

  Rule: The as: stanza is opt-in — absent, the gateway is byte-identical

    Scenario: Without as:, the AS endpoints do not exist and /mcp stays open
      Given a provisioned multi-context gateway
      When the agent issues a GET to "/.well-known/oauth-authorization-server"
      Then the HTTP status is 404
      When the agent issues a GET to "/.well-known/oauth-protected-resource"
      Then the HTTP status is 404
      When the agent calls "tools/list" over HTTP presenting no session id
      Then the call is served

    Scenario: The as: stanza needs the multi-context shape
      When a gateway config declares an as: stanza on the mono shape
      Then the config is rejected naming the multi-context requirement

    Scenario: An as: issuer off loopback requires TLS
      When a gateway config declares the as: issuer "http://as.example.com"
      Then the config is rejected naming the TLS requirement

    Scenario: Unknown fields in the as: stanza are rejected
      When a gateway config declares an as: stanza with an unknown field
      Then the config is rejected naming the unknown field

  Rule: Discovery — the 401 teaches, the metadata documents answer

    Scenario: An unauthenticated /mcp answers 401 pointing the resource metadata
      Given a gateway served with an active authorization server
      When the agent posts "tools/list" without a bearer token
      Then the HTTP status is 401
      And the WWW-Authenticate header points the protected resource metadata
      And no request reaches any upstream
      And no act is recorded in any gamma

    Scenario: A non-local Origin outranks the missing token
      Given a gateway served with an active authorization server
      When the agent posts "tools/list" with the Origin header "https://evil.example" and no bearer token
      Then the HTTP status is 403

    Scenario: The protected resource metadata names the resource and its AS
      Given a gateway served with an active authorization server
      When the agent issues a GET to "/.well-known/oauth-protected-resource"
      Then the HTTP status is 200
      And the metadata names the /mcp endpoint as the resource
      And the metadata lists the issuer as the only authorization server

    Scenario: The AS metadata pins its endpoints, S256 and public clients
      Given a gateway served with an active authorization server
      When the agent issues a GET to "/.well-known/oauth-authorization-server"
      Then the HTTP status is 200
      And the metadata names the issuer and the authorize, token and registration endpoints
      And the only code challenge method is "S256"
      And the only token endpoint auth method is "none"
      And the grant types are exactly authorization code and refresh token

  Rule: Dynamic registration — public PKCE clients, bounded by the allowlist

    Scenario: The Claude callback registers as a public client
      Given a gateway served with an active authorization server
      When a client registers with the redirect uri "https://claude.ai/api/mcp/auth_callback"
      Then the registration answers 201 with a client_id and no client_secret

    Scenario: A loopback redirect registers whatever its port
      Given a gateway served with an active authorization server
      When a client registers with the redirect uri "http://localhost:43217/callback"
      And a client registers with the redirect uri "http://127.0.0.1:8976/cb"
      Then both registrations answer 201

    Scenario: A redirect off the allowlist is refused naming the accepted forms
      Given a gateway served with an active authorization server
      When a client registers with the redirect uri "https://attacker.example/cb"
      Then the registration is refused with the error "invalid_redirect_uri"
      And the refusal names the built-in allowlist
      And no client is registered

    Scenario: A confidential registration is refused — public PKCE clients only
      Given a gateway served with an active authorization server
      When a client registers asking for the token endpoint auth method "client_secret_basic"
      Then the registration is refused with the error "invalid_client_metadata"
      And the refusal names public PKCE clients

    Scenario: The stanza extends the allowlist for a custom callback
      Given a gateway served with an authorization server also allowing "https://ci.example/cb"
      When a client registers with the redirect uri "https://ci.example/cb"
      Then the registration answers 201 with a client_id and no client_secret

  Rule: /authorize — PKCE S256 under the DEV consent

    Scenario: The consent page is honest about being DEV and names the request
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page with an S256 challenge and the resource
      Then the HTTP status is 200
      And the page is marked DEV and names the client_id and the resource
      And no authorization code is issued yet

    Scenario: Approving the DEV consent issues a one-shot code and echoes the state
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page with an S256 challenge and the resource
      And the user approves the consent
      Then the redirect goes to the registered redirect uri with a code and the presented state

    Scenario: The plain PKCE method is refused naming S256
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page with a "plain" challenge
      Then the redirect carries the error "invalid_request" naming S256
      And no authorization code is issued

    Scenario: A missing code challenge is refused naming PKCE
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page without a code challenge
      Then the redirect carries the error "invalid_request" naming PKCE
      And no authorization code is issued

    Scenario: A missing resource is refused naming RFC 8707
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page without a resource
      Then the redirect carries the error "invalid_target" naming the resource requirement
      And no authorization code is issued

    Scenario: An unknown client never gets a redirect
      Given a gateway served with an active authorization server
      When the authorize page is opened for the unregistered client "https://cimd.example/client.json"
      Then the HTTP status is 400
      And the answer names dynamic registration as the supported path
      And no authorization code is issued

    Scenario: A redirect uri differing from the registration never redirects
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      When the client opens the authorize page with the redirect uri "http://127.0.0.1:9410/other"
      Then the HTTP status is 400
      And no authorization code is issued

  Rule: /token — the proof, the audience, the rotation, the authority ceiling

    Scenario: The exchange mints the audience-bound pair and goes on the record
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      And an approved authorization code
      When the client exchanges the code with its verifier and the resource
      Then the answer carries an access token, a refresh token and the default lifetimes
      And the access token audience is the /mcp resource
      And the issuance is journalized as a governance act naming the client
      And no token byte appears in any gamma payload

    Scenario: A wrong verifier kills the code
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      And an approved authorization code
      When the client exchanges the code with a wrong verifier
      Then the exchange is refused with the error "invalid_grant"
      When the client exchanges the code with its verifier and the resource
      Then the exchange is refused with the error "invalid_grant"

    Scenario: A replayed code is refused
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      And an approved authorization code
      When the client exchanges the code with its verifier and the resource
      And the client exchanges the code with its verifier and the resource
      Then the exchange is refused with the error "invalid_grant"

    Scenario: A resource mismatch at the token endpoint is refused
      Given a gateway served with an active authorization server
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      And an approved authorization code
      When the client exchanges the code naming the resource "https://elsewhere.example/mcp"
      Then the exchange is refused with the error "invalid_target"

    Scenario: The refresh rotates one-shot and a reuse cuts the family
      Given a gateway served with an active authorization server
      And a minted token pair
      When the client refreshes with the refresh token
      Then a fresh access token and a fresh refresh token come back
      When the client refreshes again with the consumed refresh token
      Then the exchange is refused with the error "invalid_grant"
      And the successor refresh token is dead too

    Scenario: A refresh never survives its authority
      Given a gateway served with an active authorization server
      And a minted token pair
      When the clock advances past the agent chain's not_after
      And the client refreshes with the refresh token
      Then the exchange is refused naming the expired authority

    Scenario: Token lifetimes are capped by the chain's not_after
      Given a gateway served with an active authorization server whose clock sits 30 minutes before the chain expiry
      And a registered public client with the redirect uri "http://127.0.0.1:9410/cb"
      And an approved authorization code
      When the client exchanges the code with its verifier and the resource
      Then the access token expires with the chain, not after it

  Rule: The token on /mcp is a pointer, never an authority

    Scenario: A valid bearer rides the untouched pipeline — act logged then relayed
      Given a gateway served with an active authorization server
      And a minted token pair
      When the agent posts a tools call for "brand.read" with the access token
      Then the call is served
      And the act is recorded in the "company-brand" gamma

    Scenario: A forged token is refused before anything moves
      Given a gateway served with an active authorization server
      When the agent posts "tools/list" with the bearer token "forged.token.value"
      Then the HTTP status is 401
      And the WWW-Authenticate header names an invalid token
      And no request reaches any upstream
      And no act is recorded in any gamma

    Scenario: An expired token is refused
      Given a gateway served with an active authorization server
      And a minted token pair
      When the clock advances past the access token lifetime
      And the agent posts "tools/list" with the access token
      Then the HTTP status is 401
      And the WWW-Authenticate header names an invalid token

    Scenario: A right-signature wrong-audience token is refused
      Given a gateway served with an active authorization server
      When the agent posts "tools/list" with a token signed by the adapter key for another audience
      Then the HTTP status is 401
      And no request reaches any upstream

    Scenario: A revocation cuts a live token at the very next call
      Given a gateway served with an active authorization server
      And a minted token pair
      And the agent's covering mandate is revoked
      When the agent posts a tools call for "brand.read" with the access token
      Then the call is refused naming the revoked authority
      And the refusal is journalized

    Scenario: No token or code ever lands in a log or an error
      Given a gateway served with an active authorization server
      And a full authorization flow has run
      Then no gamma payload anywhere carries a token or code byte
      And no error body of the flow echoed a token or code

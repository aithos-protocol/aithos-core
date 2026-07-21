Feature: OAuth 2.1 custody for protected upstreams
  The gateway is an OAuth client on behalf of its owner. Authorization codes,
  access tokens, refresh tokens and the client secret stay behind the gateway;
  the agent sees only the governed MCP surface and its ordinary tool results.

  Rule: Configuration chooses exactly one upstream credential mode

    Scenario: An OAuth server declares only Vault references and a public callback
      When a hub server declares OAuth authorization code with PKCE and Vault custody
      Then the OAuth configuration is accepted without any secret value

    Scenario: OAuth cannot be combined with a bearer credential
      When a hub server declares OAuth and a static bearer together
      Then the configuration is rejected naming the competing credential modes

  Rule: Consent and callback establish Vault custody

    Scenario: The owner starts a consent with PKCE and state
      Given a protected upstream with a fake OAuth authorization server
      When the owner builds the consent URL
      Then the URL carries S256 PKCE, state, the configured scopes and redirect URI
      And the pending verifier lives only in the Vault record

    Scenario: The public callback exchanges the code and stores the token set
      Given a protected upstream with a fake OAuth authorization server
      And the owner has started consent
      When the OAuth callback receives the approved code and matching state
      Then the Vault record contains the access token, refresh token and expiry
      And the callback response contains no token byte

  Rule: Runtime authorization is late, refreshable and fail-closed

    Scenario: A valid access token is injected only on the protected upstream wire
      Given a protected upstream with a completed OAuth consent
      When the gateway calls the protected resource
      Then the resource sees exactly the Vault access token
      And no token byte appears in the gateway result or error text

    Scenario: An expired access token refreshes before the upstream call
      Given a protected upstream with an expired OAuth access token
      When the gateway calls the protected resource
      Then the token endpoint receives one refresh grant
      And the resource sees the rotated access token
      And the rotated token set replaces the expired Vault record

    Scenario: A failed refresh never sends an unauthenticated upstream request
      Given a protected upstream with an expired OAuth access token
      And the fake OAuth server refuses refresh
      When the gateway calls the protected resource
      Then the call is refused as OAuth unavailable
      And the protected resource receives zero requests
      And the refusal contains no access token, refresh token or client secret

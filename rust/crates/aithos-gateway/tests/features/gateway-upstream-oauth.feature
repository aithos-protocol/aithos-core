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

  @oac0 @red
  Rule: A closed profile selects one explicit OAuth client strategy

    Scenario Outline: Confidential and public clients use only their declared token authentication
      Given a connector profile declaring token endpoint authentication <client_authentication>
      And the profile has <secret_custody>
      When the fake authorization server receives an authorization code grant
      Then the token request authenticates the client using only <wire_authentication>
      And client authentication is never inferred from an empty secret

      Examples:
        | client_authentication | secret_custody                  | wire_authentication       |
        | client_secret_post    | a client secret in Vault        | form client_secret        |
        | client_secret_basic   | a client secret in Vault        | HTTP Basic authorization  |
        | none                  | no client-secret reference      | public client_id only     |

    Scenario: A public client never resolves or sends a client secret
      Given a public connector profile using PKCE and token endpoint authentication none
      When consent, callback and refresh complete against the fake authorization server
      Then the client-secret broker receives zero requests
      And every token request omits client_secret and HTTP Basic authorization

    Scenario Outline: Free-form profile input is rejected before discovery
      Given a connector profile containing <profile_defect>
      When the owner stages the connector profile
      Then the profile is rejected as invalid closed configuration
      And metadata, registration, Vault and protected resource receive zero requests

      Examples:
        | profile_defect                                      |
        | an unknown profile field                            |
        | an unsupported token authentication method          |
        | arbitrary authorization query parameters            |
        | an unapproved scope                                  |
        | an unpinned profile version                          |
        | a provider endpoint outside the approved issuer      |

  @oac0 @red
  Rule: Discovery is bounded and pins one authorization server

    Scenario: Protected resource discovery resolves and validates authorization metadata
      Given a profile allowing protected resource and authorization server discovery
      And fake RFC 9728 and RFC 8414 metadata servers for one approved issuer
      When the connector resolves its OAuth endpoints
      Then protected resource metadata is fetched before authorization server metadata
      And the resolved issuer, authorization endpoint and token endpoint are pinned
      And only HTTPS endpoints and advertised S256 are accepted

    Scenario Outline: Adversarial metadata fails closed before registration or consent
      Given fake discovery metadata with <metadata_defect>
      When the connector resolves its OAuth endpoints
      Then discovery is refused with a stable redacted error
      And registration, Vault and protected resource receive zero requests

      Examples:
        | metadata_defect                                      |
        | a response larger than the metadata limit            |
        | a response exceeding the discovery timeout           |
        | a non-HTTPS endpoint off loopback                     |
        | an issuer different from the approved issuer          |
        | an endpoint on a different origin                     |
        | no advertised S256 code challenge support             |
        | a redirect to an unapproved origin                     |
        | an unknown authorization server in resource metadata  |

  @oac0 @red
  Rule: Client registration is declared and kept in gateway custody

    Scenario Outline: The profile selects exactly one client registration strategy
      Given a connector profile declaring <registration_strategy>
      When the connector resolves its OAuth client registration
      Then the gateway obtains the pinned client_id using only <registration_source>
      And no registration credential appears in public status or consent output

      Examples:
        | registration_strategy | registration_source                   |
        | static                | approved public configuration         |
        | dynamic               | the fake RFC 7591 registration server |
        | metadata_document     | the approved client metadata document |

    Scenario: Dynamic registration credentials are durably isolated
      Given a profile using dynamic client registration
      And the fake registration server returns a client secret and expiry
      When registration completes
      Then the complete registration record is stored in its derived Vault location
      And only a redacted registration state is returned to the owner

    Scenario Outline: An invalid registration response is rejected before consent
      Given the fake registration server returns <registration_defect>
      When registration completes
      Then registration is refused as unavailable
      And pending consent, token Vault and protected resource receive zero requests

      Examples:
        | registration_defect                         |
        | a missing client_id                         |
        | a mismatched token authentication method    |
        | a redirect URI different from the callback  |
        | an expired client secret                    |
        | a response larger than the registration limit|

  @oac0 @red
  Rule: Authorization parameters are typed profile data

    Scenario: A Google offline profile emits only its approved typed parameters
      Given a generic OAuth profile with offline access and incremental authorization enabled
      When the owner starts initial consent
      Then the authorization URL includes access_type offline
      And the authorization URL includes include_granted_scopes true
      And the authorization URL omits prompt
      And no untyped parameter reaches the authorization URL

    Scenario Outline: Consent prompt is bounded to explicit lifecycle intent
      Given a generic OAuth profile that supports explicit consent repair
      When the owner starts consent for <consent_intent>
      Then the authorization URL <prompt_outcome>

      Examples:
        | consent_intent       | prompt_outcome                 |
        | initial connection   | omits prompt                   |
        | routine reconnection | omits prompt                   |
        | explicit repair      | includes prompt consent        |

  @oac0 @red
  Rule: Token custody is bound to issuer scopes and account identity

    Scenario: Refresh without a replacement refresh token preserves the current token
      Given a connected profile whose fake authorization server omits refresh_token on refresh
      When the expired access token is refreshed
      Then the new access token is stored with the previous refresh token
      And the protected resource receives only the new access token

    Scenario Outline: Identity or authority drift requires a new consent
      Given a token set bound to one issuer, subject and account label
      When <identity_drift> is observed during callback or refresh
      Then public OAuth state becomes "reauth_required"
      And the old runtime credential is disabled before any protected resource request
      And no account identifier or token appears in the refusal

      Examples:
        | identity_drift                              |
        | the issuer changes                          |
        | the subject changes                         |
        | the verified account changes                |
        | granted scopes are reduced                  |
        | granted scopes exceed the approved profile  |

    Scenario Outline: Public OAuth state has a finite redacted vocabulary
      Given OAuth custody is internally <custody_condition>
      When the owner reads connector status
      Then the public OAuth state is <public_state>
      And status exposes no issuer subject account id or Vault coordinate

      Examples:
        | custody_condition                   | public_state      |
        | current and identity-bound          | connected         |
        | past access-token expiry            | expired           |
        | revoked or invalid_grant            | reauth_required   |
        | malformed or unreachable            | unavailable       |

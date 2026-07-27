@a1 @g7 @g7-connectors
Feature: Pre-approved connector attachment and hot activation
  Control routes may attach only an instance of an already sealed enterprise
  connector. Secrets and OAuth tokens stay in Vault; the runtime trusts the
  approved manifest rather than the live upstream catalogue.

  Rule: Staging accepts only a closed approved descriptor

    Scenario Outline: Invalid connector input is rejected before Vault
      Given a signed config request containing <input_defect>
      When the owner stages the connector instance
      Then staging is refused with a stable redacted error
      And Vault, the local registry and upstream receive zero requests

      Examples:
        | input_defect                         |
        | an invalid connector id              |
        | a browser-selected Vault path         |
        | a non-HTTPS non-loopback endpoint     |
        | scopes outside the approved set       |
        | an unknown JSON field                 |
        | an unsupported transport              |
        | a redirect URI different from callback|

    Scenario Outline: Missing enterprise approval refuses the draft
      Given the requested connector has <approval_defect>
      When a correctly mandated config authority stages it
      Then the connector is refused as not approved
      And no draft, secret record or upstream request is created

      Examples:
        | approval_defect              |
        | no sealed manifest           |
        | a mismatched manifest pin    |
        | a manifest for another id    |
        | a manifest in another context|

    Scenario: A valid approved descriptor creates an inactive durable draft
      Given a sealed approved manifest and the exact connector config mandate
      When the instance descriptor is staged
      Then a versioned non-secret draft is atomically persisted in "gateway/connectors.json"
      And the connector is not visible in runtime tools
      And no gateway registry record is sent to RemoteStore

  Rule: A browser client secret has one TLS destination and one durable home

    Scenario: The client secret is written once into the derived Vault record
      Given an approved inactive connector draft
      When the browser sends a bounded client secret over gateway public TLS
      Then the gateway writes that secret exactly once to its derived Vault record
      And the Rust secret buffer is immediately zeroized
      And the connector remains inactive
      And the secret sentinel is absent from responses, registry, proof, logs and upstream

    Scenario: Vault failure leaves no connected or active state
      Given an approved inactive connector draft and an unavailable Vault
      When the browser submits its client secret
      Then the request fails as secret unavailable
      And the draft remains disconnected and the runtime router is unchanged

    Scenario: Draft deletion never invents secret deletion by overwrite
      Given an inactive draft whose broker cannot safely delete its Vault records
      When a valid config authority deletes the draft
      Then every runtime reference is disabled
      And the residual Vault record is reported only as a non-secret cleanup limitation

  Rule: Bearer TOFU is published into the Ethos before activation

    Scenario: A new bearer connector activates on the running gateway only after its binding is published
      Given a new bearer MCP connector with two live tools and no Ethos binding
      When the owner stages the bearer connector and stores its credential
      And the gateway prepares the complete TOFU binding
      Then preparation returns both live tools without exposing either in runtime
      And activation before Owner publication is refused closed
      When the owner publishes the prepared binding into the current Ethos
      And the same running gateway activates the bearer connector
      Then the complete TOFU catalogue becomes visible without restarting
      And no bearer credential appears in the binding, registry or public responses

  Rule: OAuth control delegates to the existing upstream OAuth registry

    @gmail-dynamic
    Scenario: The official Gmail MCP can be staged for OAuth without a static gateway template
      Given a new official Gmail MCP OAuth descriptor
      When the owner stages the dynamic Gmail connector twice
      Then an inactive Gmail OAuth draft is persisted without client secret material

    @gmail-dynamic
    Scenario: The official Gmail MCP starts OAuth with its pinned Google endpoints
      Given a new official Gmail MCP OAuth descriptor
      When the owner stages the dynamic Gmail connector and stores its client secret
      And the browser starts the dynamic Gmail OAuth flow
      Then the consent URL uses only the approved Google authorization endpoint

    Scenario: OAuth start stores pending custody only in Vault
      Given an approved draft with its client secret in Vault
      When the browser starts upstream OAuth
      Then the existing upstream OAuth registry returns only consent URL and expiry
      And PKCE verifier and state live only in Vault
      And the response is no-store and contains no secret or token

    Scenario: A valid callback connects without exposing OAuth material
      Given a pending upstream OAuth attempt in Vault
      When the callback carries the approved code and matching one-shot state
      Then the existing upstream OAuth registry stores the token set in Vault
      And public status becomes "connected"
      And the callback redirect carries only a generic outcome

    Scenario Outline: Invalid callback never connects
      Given a pending upstream OAuth attempt in Vault
      When the callback carries <callback_defect>
      Then OAuth remains fail-closed with a public non-connected state
      And no code, state, verifier or token appears in any public output

      Examples:
        | callback_defect        |
        | provider denial        |
        | a replayed callback    |
        | the wrong state        |
        | an expired attempt     |

    Scenario: Expired access refreshes once before discovery
      Given a connected connector with an expired access token and valid refresh token
      When activation requests authenticated discovery
      Then the existing upstream OAuth registry performs one refresh
      And discovery receives only the rotated bearer

    Scenario: A revoked refresh grant requires reauthorization without an upstream call
      Given a connected connector whose refresh is refused
      When activation requests authenticated discovery
      Then public status becomes "reauth_required"
      And the protected MCP receives zero requests

  Rule: Activation compares live discovery and swaps runtime atomically

    Scenario: An identical live catalogue activates without restart
      Given a connected approved connector whose live tools match every approved pin
      When a valid config authority activates it
      Then discovery runs once with the Vault bearer
      And a complete registry record is atomically persisted
      And the approved tools become visible without restarting the gateway

    Scenario Outline: Any live catalogue drift exposes zero tools
      Given a connected approved connector with <catalogue_drift>
      When a valid config authority activates it
      Then activation is refused as manifest drift
      And the connector exposes zero tools and the previous runtime remains intact

      Examples:
        | catalogue_drift           |
        | an added tool             |
        | a removed tool            |
        | a modified input schema   |
        | a modified digest         |

    Scenario: A crash during persistence yields old or complete new registry only
      Given one active registry and one validated replacement
      When the process crashes at each atomic persistence boundary
      Then restart reads either the old registry or the complete replacement
      And no partial JSON or half-active connector is accepted

    Scenario: Restart revalidates connectors independently
      Given two persisted active connectors with sealed approved pins
      And only one still has valid OAuth custody
      When the gateway restarts
      Then the healthy connector returns active
      And the unhealthy connector fails closed without disabling its neighbor

    Scenario: Normal tools listing is memory-only
      Given one hot-activated connector
      When an agent calls tools/list repeatedly
      Then the approved runtime view is returned from memory
      And Vault and every upstream receive zero list requests

    Scenario: OAuth failure is isolated per connector
      Given active connectors A and B
      When connector A becomes OAuth unavailable
      Then connector B remains listed and callable
      And connector A sends zero unauthenticated upstream requests

  Rule: Runtime acts remain governed after hot activation

    Scenario: A mandated safe call is logged before relay
      Given a hot-activated connector and a current mandate for one approved safe capability
      When the agent calls that exact capability
      Then the act is durably logged before the upstream receives it
      And only that connector receives the original bounded arguments

    Scenario: A neighboring capability is refused before custody or upstream
      Given a hot-activated connector and a mandate for one approved safe capability
      When the agent calls a neighboring unmandated capability
      Then authority is denied before Vault and upstream
      And a redacted governance refusal is logged

    Scenario: Public connector errors are closed stable and redacted
      Given every internal connector failure contains distinct secret sentinels
      When each failure crosses the control API
      Then its public code belongs to the documented finite error set
      And no sentinel appears in body, headers, URL, registry, proof or logs

  @oac0 @red
  Rule: Connector profiles are sealed versioned capability declarations

    Scenario: An approved profile version materializes one closed connector contract
      Given a sealed connector profile with one version, OAuth strategy, scope set, risk class and execution kind
      And the profile pins one approved MCP manifest or compiled extension manifest
      When the owner stages an instance of that exact profile version
      Then the durable draft references the sealed profile without copying free-form provider data
      And the instance records the same generic profile resolver path as every provider canary
      And the connector remains absent until explicitly activated

    Scenario Outline: Profile drift invalidates only the affected connector
      Given an active connector instantiated from a sealed profile
      When its profile has <profile_drift>
      Then that connector is disabled as profile drift
      And neighboring connectors remain listed and callable
      And no OAuth credential or upstream request is resolved

      Examples:
        | profile_drift                         |
        | a different version                   |
        | a changed scope set                   |
        | a changed risk class                  |
        | a changed execution kind              |
        | a changed approved manifest pin       |

    Scenario: Existing configuration has no implicit profile surface
      Given a valid legacy gateway configuration with static bearer, hub and upstream OAuth servers
      And no connector profile is enabled
      When the gateway starts after profile support is installed
      Then its configuration remains valid with identical tools and credential behavior
      And no profile discovery, registration or extension request occurs

  @oac0 @red
  Rule: Vault coordinates derive from context principal connector and account

    Scenario: One account receives four non-aliasing derived custody records
      Given an approved connector instance for one context, principal, connector and account
      When its registration, pending consent, token and revocation custody are prepared
      Then the gateway derives every Vault coordinate without browser input
      And the records share only the prefix "connectors/<context>/<principal>/<connector>/<account>"
      And registration, pending, token and revocation records do not alias

    Scenario Outline: Unsafe custody identity is rejected before Vault
      Given connector custody containing <custody_defect>
      When the owner stages the connector instance
      Then staging is refused with a stable redacted error
      And Vault, registry, discovery and upstream receive zero requests

      Examples:
        | custody_defect                         |
        | an empty account id                    |
        | a traversal segment in the account id  |
        | a browser-selected Vault coordinate    |
        | a principal from another context       |

  @oac0 @red
  Rule: Multi-account consent and runtime are isolated

    Scenario: Two accounts of one connector complete consent independently
      Given two approved accounts of one connector for the same principal
      When both owners start consent and callbacks arrive in reverse order
      Then each one-shot state resolves only its own pending record
      And each token set is bound to its own issuer subject and account
      And each account activates only its own namespaced tool surface

    Scenario Outline: Cross-account OAuth material never crosses custody boundaries
      Given pending or connected accounts A and B for one connector
      When account A presents <cross_account_material> from account B
      Then account A remains non-connected or keeps its previous complete token set
      And account B remains unchanged
      And token endpoint, protected resource and unrelated Vault records receive zero requests

      Examples:
        | cross_account_material        |
        | callback state                |
        | issuer and subject assertion  |

    Scenario: An opaque code from another account cannot mutate the bound account
      Given pending accounts A and B for one connector
      When account A exchanges an opaque authorization code issued for account B with account A state
      Then at most one bounded token exchange occurs
      And neither account token record is replaced
      And the protected resource receives zero requests

    Scenario: One account requiring reauthorization leaves its neighbor active
      Given two active accounts of one connector for the same principal
      When account A enters "reauth_required"
      Then account A is removed from the runtime router before its next call
      And account B remains listed and callable
      And account A sends zero unauthenticated upstream requests

  @oac0 @red
  Rule: Disconnect removes authority before provider and Vault cleanup

    Scenario: Provider revocation and Vault cleanup follow immediate runtime removal
      Given an active connected connector with a declared revocation endpoint
      When the owner disconnects that account
      Then its runtime tools and credential reference are removed first
      And the fake provider receives one bounded revocation request
      And its registration, pending, token and revocation records are safely deleted
      And the connector reports a public non-connected state

    Scenario: Cleanup limitations never re-enable a disconnected connector
      Given an active connected connector whose broker cannot safely delete records
      When the owner disconnects that account
      Then its runtime tools and credential reference are removed first
      And the residual custody is reported only as a redacted cleanup limitation
      And restart does not re-register or reactivate the connector
      And the protected resource receives zero requests

    Scenario: Revocation failure is visible but remains fail-closed
      Given an active connected connector whose fake provider refuses revocation
      When the owner disconnects that account
      Then its runtime tools and credential reference are removed first
      And public status reports a redacted revocation residue
      And no later call retries the effect or reaches the protected resource

@a1 @g7 @g7-connectors
Feature: Pre-approved connector attachment and hot activation
  Control routes may attach only an instance of an already sealed enterprise
  connector. Secrets and OAuth tokens stay in Vault; the runtime trusts the
  approved manifest rather than the live upstream catalogue.

  Rule: Staging accepts only a closed approved descriptor

    Scenario Outline: Invalid connector input is rejected before Vault
      Given a signed config request containing <input defect>
      When the owner stages the connector instance
      Then staging is refused with a stable redacted error
      And Vault, the local registry and upstream receive zero requests

      Examples:
        | input defect                         |
        | an invalid connector id              |
        | a browser-selected Vault path         |
        | a non-HTTPS non-loopback endpoint     |
        | scopes outside the approved set       |
        | an unknown JSON field                 |
        | an unsupported transport              |
        | a redirect URI different from callback|

    Scenario Outline: Missing enterprise approval refuses the draft
      Given the requested connector has <approval defect>
      When a correctly mandated config authority stages it
      Then the connector is refused as not approved
      And no draft, secret record or upstream request is created

      Examples:
        | approval defect              |
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

  Rule: OAuth control delegates to the existing upstream OAuth registry

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
      When the callback carries <callback defect>
      Then OAuth remains fail-closed with a public non-connected state
      And no code, state, verifier or token appears in any public output

      Examples:
        | callback defect        |
        | provider denial        |
        | a replayed callback    |
        | the wrong state        |
        | an expired attempt     |

    Scenario: Expired access refreshes once before discovery
      Given a connected connector with an expired access token and valid refresh token
      When activation requests authenticated discovery
      Then the existing upstream OAuth registry performs one refresh
      And discovery receives only the rotated bearer

    Scenario: Broken refresh makes the connector unavailable without an upstream call
      Given a connected connector whose refresh is refused
      When activation requests authenticated discovery
      Then public status becomes "unavailable"
      And the protected MCP receives zero requests

  Rule: Activation compares live discovery and swaps runtime atomically

    Scenario: An identical live catalogue activates without restart
      Given a connected approved connector whose live tools match every approved pin
      When a valid config authority activates it
      Then discovery runs once with the Vault bearer
      And a complete registry record is atomically persisted
      And the approved tools become visible without restarting the gateway

    Scenario Outline: Any live catalogue drift exposes zero tools
      Given a connected approved connector with <catalogue drift>
      When a valid config authority activates it
      Then activation is refused as manifest drift
      And the connector exposes zero tools and the previous runtime remains intact

      Examples:
        | catalogue drift           |
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

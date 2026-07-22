@wip @g4 @oauth @vault
Feature: OAuth session state is durable, CAS-protected and secret-safe
  Production DCR clients, pending ceremonies, sessions, refresh families,
  nonces and temporary session keys live in closed Vault namespaces.

  Scenario Outline: Durable state survives a real gateway restart
    Given production OAuth has persisted a <record>
    When the gateway process restarts against the same Vault
    Then that unexpired record remains usable exactly once where applicable

    Examples:
      | record          |
      | DCR client      |
      | active session  |
      | refresh family  |

  Scenario Outline: OAuth bindings are checked at exchange
    Given a one-shot authorization code bound to PKCE resource client and sid
    When token exchange presents <fault>
    Then token exchange returns a redacted OAuth refusal
    And the code cannot be exchanged again

    Examples:
      | fault              |
      | the wrong verifier |
      | the wrong resource |
      | the wrong audience |
      | an unknown session |

  Scenario: Refresh rotation detects reuse across restart
    Given a refresh family has rotated once
    When the consumed refresh token is presented after a gateway restart
    Then the complete family is revoked
    And no later token in that family can be used

  Scenario Outline: State failures are never bypassed
    Given production OAuth state is <fault>
    When a client attempts authorization token refresh or MCP entry
    Then the operation fails closed without credential or upstream I/O

    Examples:
      | fault             |
      | unavailable       |
      | corrupt           |
      | at a CAS conflict |

  Scenario: Expired cleanup is namespace-local
    Given one expired session and one neighboring live session
    When OAuth state cleanup runs twice
    Then only the expired session and its temporary key are absent
    And the neighboring session remains active

  Scenario: Public surfaces contain no custody material
    Given codes tokens session seeds and person key material exist
    When logs errors Gamma DOM storage and final URLs are inspected
    Then none contains a raw code token seed or private key


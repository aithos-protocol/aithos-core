@wip @g4 @ceremony
Feature: A delegate creates a bounded MCP session without exporting their key
  The production authorization flow replaces the development consent button
  with a one-shot, WYSIWYS ceremony. The person key remains in local custody
  and the gateway receives only a signed, attenuated session mandate.

  Scenario: Production authorization renders a ceremony instead of DEV consent
    Given a production authorization server with a registered public client
    When a valid S256 authorization request is opened
    Then the page contains the closed ceremony digest and no DEV approval control

  Scenario: A valid parent delegates one strictly attenuated session
    Given a live person mandate with issue depth 1 and room for another session
    When its holder signs the displayed session submandate and ceremony challenge
    Then the leaf grantee is gateway_pub and session_bind is session_pub
    And the leaf transmits no issue authority
    And one authorization code is bound to the new session

  Scenario Outline: Invalid parent authority creates no session
    Given a parent mandate that is <fault>
    When its holder attempts to complete the session ceremony
    Then the ceremony is refused before any certificate code or token exists

    Examples:
      | fault                         |
      | malformed                     |
      | expired                       |
      | revoked                       |
      | held by another subject key   |
      | missing issue authority       |
      | at exhausted issue depth      |

  Scenario Outline: Attenuation cannot widen a parent
    Given a valid person mandate and a proposed session widening its <family>
    When its holder attempts to complete the session ceremony
    Then the ceremony refusal names <family>
    And no certificate code or token exists

    Examples:
      | family       |
      | scope        |
      | window       |
      | constraint   |
      | obligation   |
      | issue        |

  Scenario Outline: Every displayed binding is signed byte-exactly
    Given a valid signed session ceremony
    When <binding> differs from the displayed ceremony
    Then completion is refused before any certificate code or token exists

    Examples:
      | binding          |
      | gateway_pub      |
      | session_pub      |
      | OAuth client     |
      | OAuth resource   |
      | PKCE challenge   |
      | redirect URI     |
      | nonce            |
      | WYSIWYS digest   |

  Scenario Outline: A ceremony transaction is one-shot and short-lived
    Given a valid pending session ceremony
    When the transaction is <fault>
    Then no additional certificate code or session is created

    Examples:
      | fault                 |
      | replayed              |
      | completed twice       |
      | completed after 2 min |
      | cancelled             |

  Scenario: Session lifetime is capped at eight hours
    Given a valid person mandate with a longer remaining window
    When a session longer than eight hours is requested
    Then the ceremony is refused before any certificate code or token exists

  Scenario Outline: Adding session_bind follows Core attenuation
    Given a valid parent whose session_bind is <parent_binding>
    When the child proposes session_bind <child_binding>
    Then attenuation is <verdict>

    Examples:
      | parent_binding | child_binding | verdict  |
      | absent         | session key A | accepted |
      | session key A  | session key A | accepted |
      | session key A  | session key B | refused  |
      | session key A  | absent        | refused  |

  Scenario: At most three active sessions exist for one person
    Given three isolated active sessions for one person mandate
    When its holder completes a fourth session ceremony
    Then the fourth ceremony is refused before a code exists
    And the first three sessions remain active and isolated


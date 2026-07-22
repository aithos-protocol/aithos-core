@wip @g4 @g5 @session @sc1
Feature: Every MCP operation is authorized by its durable delegated session
  A bearer token selects a session; it is never authority by itself. Before
  relay the gateway verifies the fresh mandate chain and both SC1 proofs over
  the same operation_ref, then records the decision under that exact chain.

  Scenario: Exact SC1 double proof authorizes one operation
    Given a live session leaf bound to its session key
    When the gateway signs the leaf and session proofs over one operation_ref
    Then Core verify_session accepts the exact operation
    And the act is logged before the upstream is called

  Scenario Outline: Broken SC1 never reaches custody or upstream
    Given a live delegated session
    When its operation has <fault>
    Then Core reports InvalidSession
    And neither the credential broker nor the upstream is called

    Examples:
      | fault                         |
      | no leaf proof                 |
      | a false leaf proof            |
      | no session proof              |
      | a false session proof         |
      | crossed operation references  |
      | a different SC1 digest        |
      | a different session key       |
      | a different validity interval |
      | a different mandate id        |
      | a different subject           |

  Scenario Outline: Bearer session binding fails closed
    Given a valid audience-bound bearer
    When its sid is <fault>
    Then MCP answers 401 invalid_token
    And neither the credential broker nor the upstream is called

    Examples:
      | fault                         |
      | absent                        |
      | unknown                       |
      | bound to another client       |
      | bound to another resource     |
      | bound to another context      |
      | expired                       |
      | revoked                       |
      | disabled                      |

  Scenario: Concurrent principals see only their own tools
    Given two active sessions backed by different attenuated mandate chains
    When each session calls tools/list
    Then each answer contains only the tools in that session perimeter
    And each count and obligation follows that session chain

  Scenario: Gamma attributes a session operation exactly
    Given a live delegated session
    When it completes an authorized MCP operation
    Then Gamma authorized_via is the complete session chain
    And Gamma records the exact session fact without a secret

  Scenario: Revocation cuts the next operation
    Given a bearer minted before its parent or leaf is revoked
    When the bearer calls a tool after revocation
    Then the call is refused before credential resolution or upstream I/O


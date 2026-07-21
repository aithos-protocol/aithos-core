@a1 @g7 @g7-control
Feature: Signed enterprise control and proof surface
  The browser talks directly to the customer gateway. Origin is an exact browser
  barrier, while an A.2 envelope and current mandate are the actual authority.

  @g7a
  Rule: CORS is exact and never substitutes for authority

    Scenario: The configured dashboard origin receives a minimal preflight
      Given the only configured dashboard origin is "https://app.aithos.fr"
      When that origin preflights a signed control request
      Then the gateway admits only the required method and headers
      And the response varies on Origin with a bounded max age
      And no browser credentials or wildcard are admitted

    Scenario Outline: A non-exact origin is refused without reflection
      Given exact dashboard Origin matching is configured
      When "<origin>" requests a control route
      Then CORS is refused before any control effect
      And no Access-Control-Allow-Origin wildcard or reflected origin is returned

      Examples:
        | origin                         |
        | https://neighbor.app.aithos.fr  |
        | https://app.aithos.fr.evil.tld  |
        | null                            |

    Scenario: A forged curl Origin without authority remains denied
      Given a non-browser request claims Origin "https://app.aithos.fr"
      But it carries no valid A.2 envelope
      When it requests gateway control status
      Then authority is denied with zero protected read

  @g7a
  Rule: Every control effect is signed, fresh and narrowly mandated

    Scenario Outline: Invalid signed authority produces zero effect
      Given an otherwise valid control request with "<authority_defect>"
      When the gateway verifies the control envelope and mandate chain
      Then the request is refused before proof, Vault, registry and upstream access
      And the stable error contains no authority material

      Examples:
        | authority_defect       |
        | a missing signature    |
        | a false signature      |
        | a modified body        |
        | a replayed nonce       |
        | excessive clock skew   |
        | an expired mandate     |
        | a revoked mandate      |
        | the neighboring right  |

    Scenario: The signature covers the exact method path and body digest
      Given one valid owner control envelope
      When its method, exact path or body is changed independently
      Then every changed request is denied before route execution

  Rule: Proof responses preserve ciphertext and auditor scope

    @g7a
    Scenario: A bounded auditor reads only its Gamma slice
      Given two contexts and an auditor mandated for only one Gamma slice
      When the auditor pages certificates, Gamma and heads through control
      Then only the mandated context and slice are returned
      And the artifacts remain signed or ciphertext for local client verification

    @g7b
    Scenario: Owner and config authority never receive plaintext secrets
      Given a connected connector with secret and tokens in Vault
      When owner and connector config authorities read every permitted control route
      Then no response contains a client secret, token, Vault reference or MCP payload
      And every sensitive response is no-store

    @g7a
    Scenario: Status is operationally useful and redacted
      Given local path, environment, query, token and MCP argument sentinels exist behind the gateway
      When a valid owner requests "/control/v1/status"
      Then status reports bounded process, Vault and relay readiness
      And no sentinel or upstream error detail appears in the response or logs

    @sdk
    Scenario: Live proof agrees with the independently verified remote proof
      Given the same signed proof exists in the gateway sidecar and RemoteStore
      When the browser fetches both copies
      Then aithos-client verifies each copy locally
      And their verified heads and hash chains are identical

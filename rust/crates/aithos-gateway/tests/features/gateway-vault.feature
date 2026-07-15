Feature: Enterprise credential vault — upstream MCP tokens brokered per call
  The governed hub stops carrying upstream credentials in its YAML: a
  server declares a non-secret reference (broker, path, field) and the
  gateway resolves the real token from an enterprise vault — HashiCorp
  Vault KV v2 first — at the last possible moment, after the mandate
  said yes and the act is already in the gamma. The agent never sees a
  credential; neither do the config, the Ethos stores, the gammas, the
  journal or the gateway's own output. A vault that is down, a missing
  path or field, or a malformed vault answer closes the route: no call
  ever reaches an upstream without the credential it was granted under.

  Rule: A granted call carries its vault-resolved bearer — wire-side only

    Scenario: A granted tool fetches its bearer from Vault KV v2 and the upstream sees it
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      When the agent calls "github__issues_list" through the hub
      Then the call succeeds and the upstream saw exactly one bearer "github-mcp-sentinel"
      And the vault was consulted after the act was logged

    Scenario: The agent sees the bearer in no MCP response
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      When the agent initializes, lists the tools, calls the covered tool and calls an unknown tool
      Then no agent-facing response contains "github-mcp-sentinel"
      And no agent-facing response contains the vault access token

    Scenario: The credential lives in no config, store, gamma, journal or gateway output
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      When the agent calls "github__issues_list" through the hub
      Then the gateway config text contains the reference but never "github-mcp-sentinel"
      And no file of any Ethos store contains "github-mcp-sentinel"
      And no gamma or journal entry contains "github-mcp-sentinel"
      And no agent-facing or logged text contains "github-mcp-sentinel"

  Rule: Refused calls never wake the vault; a failing vault never lets a call out

    Scenario: An unknown or ungranted tool triggers zero vault and zero upstream requests
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      And server "github" has known but ungranted tool "issues.create"
      When the agent calls "github__issues_create" and then a completely unknown tool
      Then both calls are refused
      And the vault received zero requests
      And the upstream received zero requests

    Scenario: A vault outage refuses the call before any upstream contact
      Given server "github" is enrolled with covered tool "issues.list" referencing a vault that is down
      When the agent calls "github__issues_list" through the hub
      Then the call is refused as credential unavailable
      And the upstream received zero requests
      And the journal gains one refusal entry naming the credential failure

    Scenario Outline: A missing or malformed vault answer fails closed
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing vault path "<path>" field "<field>"
      When the agent calls "github__issues_list" through the hub
      Then the call is refused as credential unavailable
      And the upstream received zero requests
      And the refusal text names neither the vault answer nor any secret value

      Examples:
        | path              | field |
        | aithos/mcp/absent | token |
        | aithos/mcp/github | tok   |

    Scenario: A malformed vault payload fails closed even under the right path and field
      Given a vault answers path "aithos/mcp/github" with a payload that is not a KV v2 secret
      And server "github" is enrolled with covered tool "issues.list" referencing that vault path
      When the agent calls "github__issues_list" through the hub
      Then the call is refused as credential unavailable
      And the upstream received zero requests

  Rule: References stay per-server and follow the vault, not the config

    Scenario: Two servers resolve two distinct references without confusion
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And the vault also stores "linear-mcp-sentinel" under path "aithos/mcp/linear" field "token"
      And servers "github" and "linear" are enrolled with covered tools referencing their own secrets
      When the agent calls one covered tool of each server through the hub
      Then the "github" upstream saw only bearer "github-mcp-sentinel"
      And the "linear" upstream saw only bearer "linear-mcp-sentinel"

    Scenario: A rotated KV value is used on the next call without any config change
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      And the agent calls "github__issues_list" through the hub
      When the vault value rotates to "github-mcp-rotated"
      And the agent calls "github__issues_list" through the hub
      Then the upstream saw bearer "github-mcp-sentinel" then bearer "github-mcp-rotated"
      And the gateway config was never modified

    Scenario: tools/list consults neither the vault nor the upstream
      Given a vault stores "github-mcp-sentinel" under path "aithos/mcp/github" field "token"
      And server "github" is enrolled with covered tool "issues.list" referencing that vault secret
      When the agent lists the tools
      Then the list includes "github__issues_list"
      And the vault received zero requests
      And the upstream received zero requests

  Rule: One credential source per server, decided at configuration time

    Scenario: Declaring credential and bearer_token together is rejected at config time
      When a hub config gives one server both a vault credential reference and an inline bearer_token
      Then the config is rejected naming the double credential source

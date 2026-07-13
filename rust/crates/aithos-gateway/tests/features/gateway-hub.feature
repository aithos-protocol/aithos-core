Feature: Governed MCP hub — arbitrary servers under mandate
  Phase H turns the existing multi-context router into a governed MCP
  hub. An upstream server is declared once and may serve tools granted
  by several Ethos, but every exposed tool resolves to exactly one
  covering context. The owner approves and pins each tool's name,
  description and input schema before the agent can see or call it.

  Runtime truth is the approved manifest plus the context mandate,
  never a fresh claim from the upstream. `tools/list` is reconstructed
  from covered pins only; a known but ungranted tool stays addressable
  internally so its refusal is precise, without advertising it to the
  agent. Any pin drift closes the route and becomes a governance event.
  Phase H v1 remains deliberately limited to `initialize`,
  `tools/list` and `tools/call`.

  Rule: Enrollment turns upstream discovery into owner-approved pins

    Scenario: The owner enrolls a discovered server and grants approved tools
      Given MCP server "github" advertises tools "issues.list" and "issues.create"
      When the owner discovers "github" and approves each tool's risk class
      Then the approved manifest pins each tool's name, description and input schema
      And the agent receives a mandate covering the approved exposed actions
      And the granting context gamma records the grant

  Rule: The agent sees only covered tools and the exact approved schemas

    @wip
    Scenario: tools/list is built from covered pins without consulting the upstream
      Given server "github" is enrolled with covered tool "issues.list"
      And server "github" has known but ungranted tool "issues.create"
      When the upstream is unavailable and the agent lists the tools
      Then the list includes "github__issues_list" with its pinned description and input schema
      And the list does not include "github__issues_create"
      And no request reaches the upstream

    @wip
    Scenario: Methods outside the tools surface remain closed in hub v1
      Given server "github" is enrolled with covered tool "issues.list"
      When the agent requests MCP resources through the hub
      Then the gateway answers method not found
      And no request reaches the upstream

  Rule: A shared server routes each covered tool through exactly one Ethos

    @wip
    Scenario: Two Ethos grant different tools from one shared server
      Given server "github" is shared by contexts "customer-support" and "engineering"
      And "customer-support" covers exposed tool "github__issues_list"
      And "engineering" covers exposed tool "github__pulls_list"
      When the agent calls both covered tools through the hub
      Then both calls reach the same "github" upstream under their raw tool names
      And "github__issues_list" is logged in the "customer-support" gamma only
      And "github__pulls_list" is logged in the "engineering" gamma only
      And the journal holds one cross-reference per act, joinable both ways

    @wip
    Scenario: A known but ungranted write is hidden and refused precisely
      Given server "github" is enrolled with covered tool "issues.list"
      And server "github" has known but ungranted tool "issues.create"
      When the agent calls "github__issues_create" through the hub
      Then the call never reaches the upstream
      And the refusal names "github__issues_create"
      And the gamma of the context that knows the tool gains one governance refusal
      And the journal gains one refusal entry

  Rule: The approved pin defeats upstream tool poisoning

    @wip
    Scenario: A description drift closes the route and is governed
      Given server "github" is enrolled with covered tool "issues.list"
      And the upstream now advertises a different description for "issues.list"
      And the gateway's runtime drift control observes that change
      When the agent calls "github__issues_list" through the hub
      Then the call is refused as manifest drift before the tool is relayed
      And the granting context gamma gains one governance refusal
      And the journal gains one refusal entry

    @wip
    Scenario: Re-enrollment replaces the pin under a new mandate
      Given server "github" is enrolled with covered tool "issues.list"
      And discovery finds an owner-accepted schema change for "issues.list"
      When the owner re-enrolls the tool for the same agent key
      Then a new mandate covers the newly pinned manifest
      And the old mandate is politically revoked
      And the granting context gamma records the new grant and the revocation
      And tools/list serves only the newly pinned schema

  Rule: Hub names and routes are unambiguous at configuration time

    Scenario Outline: Reserved server names are rejected
      When a hub config declares server "<server>"
      Then the config is rejected naming the reserved server name

      Examples:
        | server  |
        | journal |
        | gateway |

    Scenario: One upstream tool cannot be granted by two contexts
      Given server "github" advertises tool "issues.list"
      When contexts "customer-support" and "engineering" both grant that upstream tool
      Then the config is rejected as an ambiguous context route

    Scenario: Flattened exposed-name collisions are rejected
      Given server "github" grants raw tool "issues.list"
      When that server also grants raw tool "issues_list"
      Then the config is rejected naming the exposed-name collision

    Scenario: An exposed name reachable from two servers is rejected
      Given server "a" grants raw tool "b__c"
      When a hub config also declares server "a__b" granting raw tool "c"
      Then the config is rejected naming the exposed-name collision

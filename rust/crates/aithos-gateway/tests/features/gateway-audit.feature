Feature: Gateway audit MVP — an agent is plugged, contained and audited
  The gateway sits between an existing agent and its MCP tools. It holds
  the keys the agent never sees, enforces the mandate on every call,
  writes one gamma entry per act with the kind imposed by the operation,
  and lets a third-party auditor read exactly their slice of the log.
  First sellable brick: external audit of an agent that keeps running
  as before. (GATEWAY-BOOTSTRAP §7)

  Background:
    Given a company MCP server exposing tools "user.read" and "user.update"
    And a gateway onboarded with a read-only mandate for those tools

  Rule: Plugging in changes the route, not the behaviour of reads

    Scenario: A read tool call passes through and is logged
      When the agent calls tool "user.read" through the gateway
      Then the call reaches the MCP server and the agent gets the answer
      And the gamma log gains one act entry whose kind names the read operation

  Rule: The mandate is enforced fail-closed on every call

    Scenario: A write tool call is refused and the refusal is logged
      When the agent calls tool "user.update" through the gateway
      Then the call never reaches the MCP server
      And the agent receives a policy refusal
      And the gamma log gains one refusal entry

    Scenario: A tool absent from the mapping is denied by default
      When the agent calls tool "user.delete" through the gateway
      Then the call never reaches the MCP server
      And the refusal is logged

  Rule: The agent never holds a key and never chooses a kind

    Scenario: Entries are signed by the gateway with the imposed kind
      When the agent calls tool "user.read" claiming kind "heartbeat"
      Then the claimed kind is ignored
      And the logged entry bears the kind imposed by the operation mapping
      And its signature verifies against the gateway-held agent key

  Rule: A third-party auditor sees exactly their scope

    Scenario: An auditor exports the scoped log and verifies it offline
      Given an auditor granted read.gamma scoped to act entries
      And the agent has made one allowed and one refused call
      When the auditor exports the audit log from the gateway
      Then the export contains the act entries and verifies offline
      And entries outside the auditor's scope are not readable

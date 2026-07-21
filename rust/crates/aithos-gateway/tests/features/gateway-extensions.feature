Feature: Compiled extension packs under the gateway mandate wall
  GSE-0 adds only the reusable seam for optional, compiled-in packs. An
  extension is not an extra MCP listener and it is never trusted as a new
  authority: one configured context remains the proof destination, its live
  mandate decides visibility and calls, and the gateway records refusals
  under its own identity. The pack manifest pins its id, version, tools,
  schemas, risk classes, declarative constraints and OAuth needs, but GSE-0
  creates no OAuth client and performs no external I/O.

  The `aithos-gmail` pack is contract-only in this lot. Its manifest reserves
  `aithos-gmail__send_guarded`; invoking it reaches a deterministic
  not-implemented refusal, never Google.

  Rule: Extensions are opt-in and default-deny

    Scenario: An absent pack is hidden and its tool call is refused
      Given the compiled pack "aithos-gmail" declares tool "send_guarded"
      And the gateway configuration omits extensions
      When the agent lists tools and calls "aithos-gmail__send_guarded"
      Then "aithos-gmail__send_guarded" is not listed
      And the call is refused as an unmapped tool
      And the journal records the refusal under the gateway identity

    Scenario: An activated pack without a covering mandate stays hidden
      Given extension "aithos-gmail" is enabled for context "outbound"
      And context "outbound" has no mandate for "act.x.aithos-gmail.send_guarded"
      When the agent lists tools and calls "aithos-gmail__send_guarded"
      Then "aithos-gmail__send_guarded" is not listed
      And the call is refused by the context mandate
      And no external request is made

  Rule: The visible extension surface is pinned and mandate-derived

    Scenario: An activated pack with a covering mandate contributes its exact descriptor
      Given extension "aithos-gmail" is enabled for context "outbound"
      And context "outbound" covers "act.x.aithos-gmail.send_guarded"
      When the agent lists tools
      Then the list includes "aithos-gmail__send_guarded" with the extension's pinned schema
      And the descriptor comes from manifest version "aithos-extension-pack-v1"

    Scenario: Revoking the covering mandate removes the tool without restart
      Given extension "aithos-gmail" is enabled for context "outbound"
      And context "outbound" covers "act.x.aithos-gmail.send_guarded"
      When the owner revokes that covering mandate while the gateway stays running
      Then the next tool list does not include "aithos-gmail__send_guarded"
      And the gateway process was not restarted

  Rule: Pack and exposed-tool namespaces cannot be usurped

    Scenario: An external server cannot reuse an enabled extension id
      When a gateway config enables extension "aithos-gmail" and declares external server "aithos-gmail"
      Then the config is rejected naming the extension id collision

    Scenario: An external tool cannot reuse an extension's exposed name
      When a gateway config maps external tool "aithos-gmail__send_guarded" and enables that extension
      Then the config is rejected naming the exposed-name collision

  Rule: GSE-0 proves the invocation seam without implementing Gmail

    Scenario: The contract-only Gmail pack refuses without I/O and the refusal is governed
      Given extension "aithos-gmail" is enabled for context "outbound"
      And context "outbound" covers "act.x.aithos-gmail.send_guarded"
      When the agent calls "aithos-gmail__send_guarded" with an empty payload
      Then the pack refuses as not implemented in GSE-0
      And no external request is made
      And the context and journal record a gateway-identity refusal

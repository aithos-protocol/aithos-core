Feature: Journal tools — the agent consolidates its own memory
  Lot C2 of Phase C: the gateway serves aithos-native tools on its own
  `/mcp` endpoint — `journal.write` and `journal.search` — so the agent
  consolidates memory into ITS journal, under mandate, fully traced, and
  reads it back. Served by the gateway itself, never relayed upstream.

  v1 mechanics (contract committed @wip — defaults proposed 2026-07-12,
  pending Mathieu's validation of the five C2 decision points):
  - D1 scope: write AND search (consolidation without recall is worthless).
  - D2 target: a note is ONE act entry in the journal gamma (connector
    `journal`, clear payload, fresh id per write) — the same
    no-protocol-change move as the xref mirror (§3bis.5). The core has no
    agent-side section write yet (`section_add` is owner-keys-only), so
    the `circle:memory/` section target is the recorded v2 migration,
    not this lot.
  - D3 pen: a DEDICATED journal pen (`act.x.journal.*`) minted at
    `owner-init-journal` towards the agent key — one pen per usage, like
    the inference pen; the xref pen stays untouched. No pen (journals
    provisioned before this lot) → every journal tool refuses
    fail-closed, the LLM-tap precedent.
  - D4 exposure: dotted native names; the `journal` prefix is reserved
    against context tool maps (config rejected), mirroring HUB-MCP §5;
    `tools/list` serves the native tools with their real schemas.
  - D5 search: the gateway scans its own journal store (readability
    frontier — it holds the files), and the search is ITSELF journalized
    before results flow back (log-before-effect). Notes only: xref
    mirrors, inferences and refusals are not memory.

  Rule: The owner grants the journal pen at provisioning

    @wip
    Scenario: Provisioning mints a dedicated journal pen towards the agent key
      Given an enterprise master seed
      When the owner creates a journal for the agent's public key
      Then the agent holds a journal pen separate from the xref pen
      And the journal gamma records that the journal pen was received

  Rule: journal.write consolidates memory, never relayed

    @wip
    Scenario: A note lands as one journal entry and reaches no upstream
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note titled "standup" with text "shipped the brand fetch" and tag "daily"
      Then the call never reaches any upstream
      And the journal gains one note entry carrying that note in clear
      And the answer names the recorded note entry
      And no context gamma gains any entry

    @wip
    Scenario: A note with an unknown argument field is refused fail-closed
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note carrying an unknown argument field
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gains no note entry

    @wip
    Scenario: A note without text is refused fail-closed
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note with an empty text
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gains no note entry

    @wip
    Scenario: A journal provisioned without the pen refuses writes
      Given a runner whose journal predates the journal pen
      When the agent writes a note titled "standup" with text "shipped the brand fetch" and tag "daily"
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gains no note entry

  Rule: journal.search reads the memory back, and the read is itself traced

    @wip
    Scenario: A search returns the matching notes
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      And the journal holds a note titled "alpha" with text "brand palette approved"
      And the journal holds a note titled "beta" with text "figma tokens exported"
      When the agent searches the journal for "palette"
      Then the answer carries the "alpha" note only
      And the journal gains one search entry naming that query

    @wip
    Scenario: A tag filter narrows the recall
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      And the journal holds a note titled "alpha" tagged "brand"
      And the journal holds a note titled "beta" tagged "figma"
      When the agent searches the journal for tag "figma"
      Then the answer carries the "beta" note only

    @wip
    Scenario: A journal provisioned without the pen refuses searches too
      Given a runner whose journal predates the journal pen
      When the agent searches the journal for "anything"
      Then the journal gains one refusal entry
      And the journal gains no search entry

  Rule: The native names are reserved and listed with their real schemas

    @wip
    Scenario: tools/list serves the native journal tools with their schemas
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent lists the tools
      Then the list includes "journal.write" and "journal.search" with their argument schemas
      And the context tools keep their open schemas

    @wip
    Scenario: A context tool under the reserved prefix is rejected at config time
      When a config maps the context tool "journal.export"
      Then the config is rejected naming the reserved prefix

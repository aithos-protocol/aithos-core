Feature: Journal tools — the agent consolidates its own memory
  Lot C2 of Phase C: the gateway serves aithos-native tools on its own
  `/mcp` endpoint — `journal.write` and `journal.search` — so the agent
  consolidates memory into ITS journal, under mandate, fully traced, and
  reads it back. Served by the gateway itself, never relayed upstream.

  v2 contract (all five decision points validated by Mathieu,
  2026-07-12, on top of pass L — delegated writes):
  - A note is ONE SECTION in the journal's `circle:memory/` folder
    (fresh unique technical name per write; the human label rides in
    title and tags, clear in the zone index; the body is sealed at
    rest). The gamma trace is a delegated `section.add` with a sealed
    body — target and content teach the keyless nothing.
  - The pen is a DEDICATED memory mandate (`append` on circle
    `memory/`, spec 04.2 lattice) minted at `owner-init-journal`
    towards the agent key, next to — never inside — the xref pen: one
    pen per usage, independently revocable. The append verb creates and
    reads (every mutation verb implies read) but never rewrites nor
    deletes: v1 memory is append-only. No pen (journals provisioned
    before this lot) → every journal tool refuses fail-closed, the
    LLM-tap precedent.
  - `journal.search` matches the CLEAR index only (name, title, tags —
    the readability frontier: the gateway holds the files), newest
    first; the sealed bodies are opened for the returned hits ONLY, and
    every opened body is one journalized `ethos.read`. A search that
    matches nothing opens nothing and logs nothing.
  - Native names are dotted (`journal.write`, `journal.search`); the
    `journal` prefix is reserved against context tool maps (config
    rejected — mirrors HUB-MCP §5); `tools/list` serves the native
    tools with their real argument schemas.

  Rule: The owner grants the memory pen at provisioning

    Scenario: Provisioning mints a dedicated memory pen towards the agent key
      Given an enterprise master seed
      When the owner creates a journal for the agent's public key
      Then the agent holds a memory pen separate from the xref pen
      And the journal gamma records that the memory pen was received

  Rule: journal.write consolidates memory as sealed sections, never relayed

    Scenario: A note lands as one sealed section and reaches no upstream
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note titled "standup" with text "shipped the brand fetch" and tag "daily"
      Then the call never reaches any upstream
      And the owner reads back one memory note titled "standup" with text "shipped the brand fetch"
      And the journal gamma logs one delegated "section.add" with a sealed body
      And the answer names the recorded note
      And no context gamma gains any entry

    Scenario: A note with an unknown argument field is refused fail-closed
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note carrying an unknown argument field
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gamma logs no "section.add"

    Scenario: A note without text is refused fail-closed
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent writes a note with an empty text
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gamma logs no "section.add"

    Scenario: A journal provisioned without the pen refuses writes
      Given a runner whose journal predates the memory pen
      When the agent writes a note titled "standup" with text "shipped the brand fetch" and tag "daily"
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And the journal gamma logs no "section.add"

  Rule: journal.search recalls from the clear index; every opened body is a logged read

    Scenario: A title match returns the note, and only that body is opened
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      And the journal holds a note titled "brand palette approved" with text "use the ochre set"
      And the journal holds a note titled "figma tokens exported" with text "v2 tokens shipped"
      When the agent searches the journal for "palette"
      Then the answer carries the note titled "brand palette approved" only
      And its text "use the ochre set" comes back with it
      And the journal gamma logs exactly one "ethos.read"

    Scenario: A tag filter narrows the recall
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      And the journal holds a note titled "alpha" tagged "brand"
      And the journal holds a note titled "beta" tagged "figma"
      When the agent searches the journal for tag "figma"
      Then the answer carries the note titled "beta" only
      And the journal gamma logs exactly one "ethos.read"

    Scenario: A search that matches nothing opens nothing and logs nothing
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      And the journal holds a note titled "alpha" tagged "brand"
      When the agent searches the journal for "nothing-of-the-sort"
      Then the answer carries no note
      And the journal gamma logs no "ethos.read"

    Scenario: A journal provisioned without the pen refuses searches too
      Given a runner whose journal predates the memory pen
      When the agent searches the journal for "anything"
      Then the journal gains one refusal entry
      And the journal gamma logs no "ethos.read"

  Rule: The native names are reserved and listed with their real schemas

    Scenario: tools/list serves the native journal tools with their schemas
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent lists the tools
      Then the list includes "journal.write" and "journal.search" with their argument schemas
      And the context tools keep their open schemas

    Scenario: A context tool under the reserved prefix is rejected at config time
      When a config maps the context tool "journal.export"
      Then the config is rejected naming the reserved prefix

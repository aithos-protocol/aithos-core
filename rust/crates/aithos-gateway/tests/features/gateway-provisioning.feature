Feature: Provisioned runner — an agent is born, equipped and routed
  Phase B of the gateway (v2 target, decisions of 2026-07-10): the
  container no longer mints anything. The agent's key is born inside the
  runner and never leaves it; the owner mints mandates towards that
  public key from their own tooling; the runner serves several Ethos at
  once, each act lands in the gamma of the context that covers it, and
  the agent's own journal keeps the cross-referenced story of its life.

  Rule: The agent's key is born in the runner and only the pubkey travels

    Scenario: Birth produces a public identity and no exportable secret
      When a runner generates its agent identity
      Then it publishes the agent public key
      And the provision artifacts contain no seed material

  Rule: The owner equips the agent from their side

    Scenario: The owner creates the agent's journal and grants its pen
      Given an enterprise master seed
      When the owner creates a journal for the agent's public key
      Then the journal is an isolated Ethos owned by the enterprise
      And the agent holds a mandate to write its journal
      And the journal gamma records that a mandate was received

    Scenario: The owner grants a context to the agent's public key
      Given a context Ethos "company-brand" with tools "brand.read" and "brand.update"
      When the owner grants the agent read access to that context
      Then the context gamma records the grant
      And the granted certificate names the agent public key

  Rule: Acts land in the gamma of the context that covers them

    @wip
    Scenario: Two contexts, each call routed to its own Ethos
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent calls tool "brand.read" through the gateway
      And the agent calls tool "figma.read" through the gateway
      Then the act on "brand.read" is logged in the "company-brand" gamma only
      And the act on "figma.read" is logged in the "ui-designer" gamma only
      And the journal holds one cross-reference per act, joinable both ways

  Rule: Refusals follow the decided routing — journal always

    @wip
    Scenario: A refused context tool is logged at the context and in the journal
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent calls tool "brand.update" through the gateway
      Then the call never reaches any upstream
      And the "company-brand" gamma gains one refusal entry
      And the journal gains one refusal entry

    @wip
    Scenario: A tool unknown to every context is refused into the journal only
      Given a runner provisioned with contexts "company-brand" and "ui-designer"
      When the agent calls tool "admin.export" through the gateway
      Then the call never reaches any upstream
      And the journal gains one refusal entry
      And no context gamma gains any entry

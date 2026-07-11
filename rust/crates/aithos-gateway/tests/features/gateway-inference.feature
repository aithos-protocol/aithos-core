Feature: Metered inference — the gateway fronts the LLM provider
  Phase C of the gateway (the agent that lives): the agent never talks
  to the LLM provider directly. The gateway holds the provider
  credentials, imposes the model, reads the REAL usage from the
  provider's own answer, and logs one `inference` entry per call in the
  agent's journal — metadata only, never the prompt (that stays in the
  agent's cache). Token budgets (F+) are carried by a dedicated
  inference mandate granted at provisioning: no pen, no LLM; an
  exhausted budget closes the tap fail-closed.

  Rule: The owner grants the inference pen with a token budget

    @wip
    Scenario: Provisioning mints a budgeted inference mandate towards the agent key
      Given an enterprise master seed
      When the owner creates a journal with a token budget of 1000
      Then the agent holds an inference mandate carrying that token budget
      And the journal gamma records that the inference mandate was received

  Rule: The gateway imposes the model and holds the credentials

    @wip
    Scenario: A chat completion is relayed under the imposed model
      Given a runner with an inference pen budgeted at 1000 tokens
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then the provider is called with the configured model only
      And the provider's answer comes back to the agent

    @wip
    Scenario: The provider credentials never surface agent-side
      Given a runner with an inference pen budgeted at 1000 tokens
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then no agent-visible surface contains the provider credentials
      And no journal entry contains the provider credentials

  Rule: Every call is metered from the provider's own usage, metadata only

    @wip
    Scenario: One inference entry per call, real usage, never the prompt
      Given a runner with an inference pen budgeted at 1000 tokens
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then the journal gains one inference entry with the provider's reported usage
      And no journal entry contains the prompt or the completion text

    @wip
    Scenario: A provider answer without usage is withheld
      Given a runner with an inference pen budgeted at 1000 tokens
      And the provider omits usage from its answers
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then the completion is withheld from the agent
      And the journal gains one refusal entry
      And the journal gains no inference entry

  Rule: Token budgets close the tap

    @wip
    Scenario: An exhausted budget refuses before the provider is reached
      Given a runner with an inference pen budgeted at 1000 tokens
      And the budget is already spent
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then the provider is never called
      And the completion is withheld from the agent
      And the journal gains one refusal entry

    @wip
    Scenario: A call that overruns the remaining budget is withheld
      Given a runner with an inference pen budgeted at 1000 tokens
      And the provider reports a usage larger than the remaining budget
      When the agent asks for a chat completion with model "gpt-agent-picked"
      Then the completion is withheld from the agent
      And the journal gains one refusal entry
      And the journal gains no inference entry

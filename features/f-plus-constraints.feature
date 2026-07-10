Feature: Advanced agentic constraints — windows, budgets, inference, kinds, sealed args
  A purely additive enrichment of the mandate constraint vocabulary
  (spec 04.4 + 07.4): time becomes interval arithmetic on absolute windows
  (no timezone, no DST, no calendar in the verifier), spend becomes budget
  profiles composed with OR, every LLM call leaves a light metered entry,
  kinds form a normative registry, and action arguments are predicated and
  sealed for verifiable audit. Model/token truthfulness is staged: declared
  values count at tier V, reality is the container's duty (tier X) until a
  provider attestation bridges it back. Everything below reads files and an
  injected T; the counting engine is step F's, unchanged.

  Rule: An active window is arithmetic — anchor, duration, nothing else

    Scenario: An action inside a one-shot window verifies
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      When the agent appends an action at day 3 15:30
      Then the action verifies

    Scenario: An action after the window closes is refused
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      When the agent appends an action at day 3 18:30
      Then the action is refused as out of window

    Scenario: Window bounds are half-open — start inclusive, end exclusive
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      Then an action exactly at day 3 14:00:00 verifies
      And an action at day 3 17:59:59 verifies
      But an action exactly at day 3 18:00:00 is refused

    Scenario: An action between two occurrences of a periodic window is refused
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours
      When the agent appends an action at day 4 15:00
      Then the action is refused as out of window

    Scenario: The k-th occurrence of a periodic window is a plain offset
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours
      Then an action at day 15 15:00 verifies
      And an action at day 15 19:00 is refused

    Scenario: The until bound closes a periodic window for good
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours until day 20
      Then an action at day 15 15:00 verifies
      But an action at day 22 15:00 is refused

    Scenario: The count bound caps the number of occurrences
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours, 2 occurrences
      Then an action at day 8 15:00 verifies
      But an action at day 15 15:00 is refused

    Scenario: Several windows compose as a union
      Given an agent granted gmail actions active on day 3 morning and day 5 evening
      Then an action in the day 3 morning window verifies
      And an action in the day 5 evening window verifies
      But an action on day 4 noon is refused

    Scenario: Absolute windows and rolling limits are distinct, conjoint mechanisms
      Given an agent granted gmail actions every day at 14:00 for 4 hours with max_actions_per 2 per 24 hours
      When the agent appends two actions inside the day 3 window
      Then a third action inside the day 3 window is refused by the rolling limit
      And an action inside the day 4 window verifies

  Rule: Windows attenuate like every other dimension — a child only tightens

    Scenario: A sub-mandate windowed inside its parent verifies
      Given an agent granted gmail actions active from day 1 to day 20 with issue depth 1
      When the agent delegates the perimeter active from day 3 14:00 for 4 hours
      Then the helper's action inside that window verifies

    Scenario: A sub-mandate reaching outside its parent's windows is rejected
      Given an agent granted gmail actions active from day 1 to day 20 with issue depth 1
      When the agent delegates the perimeter active from day 15 to day 40
      Then the helper's chain is rejected

    Scenario: The mandate validity window and active_windows conjoin
      Given an agent granted gmail actions for 7 days, active daily 14:00 for 4 hours
      Then an action at day 3 15:00 verifies
      But an action at day 9 15:00 is refused even though 15:00 is in phase

  Rule: Budget profiles compose with OR — each profile is a conjunction

    Scenario: The founding grant — Haiku on Thursday afternoon OR Gemma anytime
      Given a mandate with two budget profiles:
        | id     | models        | token_budget | window                            | max_actions |
        | haiku  | claude-haiku  | 10000        | day 1 14:00 for 4 hours, weekly   | 1           |
        | gemma  | gemma         | 25000        | always                            |             |
      When the agent acts citing profile "haiku" with model "claude-haiku" and 8000 tokens at day 1 15:00
      Then the action verifies
      And the log shows 8000 tokens consumed on profile "haiku"

    Scenario: An action must cite a budget profile when the mandate carries budgets
      Given a mandate with two budget profiles
      When the agent acts without citing any budget_ref
      Then the action is refused

    Scenario: A token budget refuses the action that would overflow it
      Given a profile "haiku" with a 10000 token budget
      And 8000 tokens already consumed on "haiku"
      When the agent acts citing "haiku" with 3000 declared tokens
      Then the action is refused as over budget
      But an action citing "haiku" with 2000 declared tokens verifies

    Scenario: Exhausting one profile switches the OR to the next
      Given the founding two-profile mandate
      And profile "haiku" has spent its single action
      When the agent acts citing profile "gemma" with model "gemma" and 5000 tokens
      Then the action verifies

    Scenario: A model outside the profile's list is refused
      Given a profile "haiku" allowing model "claude-haiku"
      When the agent acts citing "haiku" with model "gpt-oss"
      Then the action is refused as model not allowed

    Scenario: A profile is only satisfiable inside its own windows
      Given the founding two-profile mandate
      When the agent acts citing profile "haiku" at day 2 09:00
      Then the action is refused as out of window
      But the same action citing profile "gemma" verifies

    Scenario: Citing an unknown profile fails closed
      Given a mandate with two budget profiles
      When the agent acts citing budget_ref "grok-unlimited"
      Then the action is refused

    Scenario: A delegate's spend drains the same profile — budgets are subtree counts
      Given the founding two-profile mandate with issue depth 1
      And the agent delegates the gemma perimeter to a helper
      When the helper acts citing "gemma" with 20000 tokens
      Then an agent action citing "gemma" with 6000 tokens is refused as over budget

    Scenario: Token consumption is tallied from the log alone
      Given a profile "gemma" with a 25000 token budget
      And logged actions of 5000, 7000 and 9000 tokens on "gemma"
      Then any verifier counts 21000 tokens consumed on "gemma"
      And the remaining budget admits at most 4000 declared tokens

  Rule: Declared values count at tier V — attestation bridges reality back

    Scenario: A profile requiring attestation refuses a bare declaration
      Given a profile "haiku" that requires attestation
      When the agent acts citing "haiku" with a declared usage and no receipt
      Then the action is refused

    Scenario: A provider-signed usage receipt satisfies the attestation hook
      Given a profile "haiku" that requires attestation
      And a provider attestation key pinned in the mandate
      When the agent acts citing "haiku" carrying a receipt signed by the provider
      Then the action verifies
      And the receipt's usage overrides the declared tokens in the tally

    Scenario: A receipt signed by the wrong key is refused
      Given a profile "haiku" that requires attestation
      When the agent acts carrying a receipt signed by the agent itself
      Then the action is refused

    Scenario: A receipt bound to a different action does not transfer
      Given a profile "haiku" that requires attestation
      And a valid receipt for an earlier action's args_hash
      When the agent replays that receipt on a new action
      Then the action is refused

  Rule: Every inference is metered, never transcribed

    Scenario: An inference entry carries counters, never content
      Given the founding two-profile mandate
      When the container logs an inference on "gemma" of 1200 tokens in and 300 out citing "gemma"
      Then the entry is of kind "inference"
      And it reveals provider, model, token counts and budget_ref
      But no prompt or response text exists anywhere in the log files

    Scenario: Inference tokens drain the cited budget profile
      Given a profile "gemma" with a 25000 token budget
      And logged inferences of 12000 and 9000 total tokens citing "gemma"
      When the container logs an inference of 5000 total tokens citing "gemma"
      Then the inference is refused as over budget
      But an inference of 3000 total tokens citing "gemma" verifies

    Scenario: Actions and inferences drain the same profile together
      Given a profile "gemma" with a 25000 token budget
      And a logged action of 20000 declared tokens citing "gemma"
      When the container logs an inference of 6000 total tokens citing "gemma"
      Then the inference is refused as over budget

    Scenario: An inference without budget_ref under a budgets mandate is refused
      Given the founding two-profile mandate
      When the container logs an inference citing no budget_ref
      Then the inference is refused

  Rule: Kinds are a normative registry, not ad hoc strings

    Scenario: An entry with an unregistered kind fails closed
      Given an initialised bundle
      When an entry of kind "banana.peel" is forced onto the log
      Then log verification is rejected

    Scenario: The ethos.write class groups every content mutation in queries
      Given a bundle whose log records section additions, a modification and actions
      When the owner queries the kind class "ethos.write"
      Then every section entry comes back
      But no action or heartbeat entry does

    Scenario: Reads are not journalized by default
      Given an agent granted read on circle folder "projets"
      When the agent reads a section under "projets"
      Then no gamma entry is appended

    Scenario: A log_reads mandate journalizes reads as ethos.read
      Given an agent granted read on circle folder "projets" with log_reads
      When the agent reads a section under "projets" and logs its read
      Then an "ethos.read" entry signed by the agent chains onto the log
      And its sealed body names the section it read

  Rule: Action arguments are predicated — and sealed for verifiable audit

    Scenario: Sealed args open for the owner, stay a hash for a stranger
      Given an agent granted gmail "reply" with sealed-args audit
      When the agent acts with arguments naming recipient "alice@example.com"
      Then the entry carries a clear args_hash and a sealed args body
      And the owner reopens the arguments and finds the recipient
      But a stranger sees only the hash

    Scenario: The sealed args must match their clear hash
      Given a logged action with sealed args
      When the sealed body is swapped for another one
      Then the audit rejects the entry as inconsistent

    Scenario: A recipient outside the allow-list is refused by the container
      Given a mandate whose action_params allow replies only to "alice@example.com"
      When the agent asks the container to reply to "mallory@evil.example"
      Then the container refuses before anything is logged

    Scenario: The owner audits predicate compliance after the fact
      Given a mandate whose action_params allow replies only to "alice@example.com"
      And a logged reply whose sealed args name "alice@example.com"
      When the owner audits the log against the mandate predicates
      Then the audit reports every logged action compliant

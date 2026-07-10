Feature: Advanced agentic constraints — absolute windows, budget profiles, attestation
  A purely additive enrichment of the mandate constraint vocabulary
  (spec 04.4 + 07.4): time becomes interval arithmetic on absolute windows
  (no timezone, no DST, no calendar in the verifier), spend becomes budget
  profiles composed with OR, and model/token truthfulness is staged —
  declared values count at tier V, reality is the container's duty (tier X)
  until a provider attestation bridges it back. Everything below reads
  files and an injected T; the counting engine is step F's, unchanged.

  Rule: An active window is arithmetic — anchor, duration, nothing else

    @wip
    Scenario: An action inside a one-shot window verifies
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      When the agent appends an action at day 3 15:30
      Then the action verifies

    @wip
    Scenario: An action after the window closes is refused
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      When the agent appends an action at day 3 18:30
      Then the action is refused as out of window

    @wip
    Scenario: Window bounds are half-open — start inclusive, end exclusive
      Given an agent granted gmail actions active from day 3 14:00 for 4 hours
      Then an action exactly at day 3 14:00:00 verifies
      And an action at day 3 17:59:59 verifies
      But an action exactly at day 3 18:00:00 is refused

    @wip
    Scenario: An action between two occurrences of a periodic window is refused
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours
      When the agent appends an action at day 4 15:00
      Then the action is refused as out of window

    @wip
    Scenario: The k-th occurrence of a periodic window is a plain offset
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours
      Then an action at day 15 15:00 verifies
      And an action at day 15 19:00 is refused

    @wip
    Scenario: The until bound closes a periodic window for good
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours until day 20
      Then an action at day 15 15:00 verifies
      But an action at day 22 15:00 is refused

    @wip
    Scenario: The count bound caps the number of occurrences
      Given an agent granted gmail actions every 7 days from day 1 14:00 for 4 hours, 2 occurrences
      Then an action at day 8 15:00 verifies
      But an action at day 15 15:00 is refused

    @wip
    Scenario: Several windows compose as a union
      Given an agent granted gmail actions active on day 3 morning and day 5 evening
      Then an action in the day 3 morning window verifies
      And an action in the day 5 evening window verifies
      But an action on day 4 noon is refused

    @wip
    Scenario: Absolute windows and rolling limits are distinct, conjoint mechanisms
      Given an agent granted gmail actions every day at 14:00 for 4 hours with max_actions_per 2 per 24 hours
      When the agent appends two actions inside the day 3 window
      Then a third action inside the day 3 window is refused by the rolling limit
      And an action inside the day 4 window verifies

  Rule: Windows attenuate like every other dimension — a child only tightens

    @wip
    Scenario: A sub-mandate windowed inside its parent verifies
      Given an agent granted gmail actions active from day 1 to day 20 with issue depth 1
      When the agent delegates the perimeter active from day 3 14:00 for 4 hours
      Then the helper's action inside that window verifies

    @wip
    Scenario: A sub-mandate reaching outside its parent's windows is rejected
      Given an agent granted gmail actions active from day 1 to day 20 with issue depth 1
      When the agent delegates the perimeter active from day 15 to day 40
      Then the helper's chain is rejected

    @wip
    Scenario: The mandate validity window and active_windows conjoin
      Given an agent granted gmail actions for 7 days, active daily 14:00 for 4 hours
      Then an action at day 3 15:00 verifies
      But an action at day 9 15:00 is refused even though 15:00 is in phase

  Rule: Budget profiles compose with OR — each profile is a conjunction

    @wip
    Scenario: The founding grant — Haiku on Thursday afternoon OR Gemma anytime
      Given a mandate with two budget profiles:
        | id     | models        | token_budget | window                            | max_actions |
        | haiku  | claude-haiku  | 10000        | day 1 14:00 for 4 hours, weekly   | 1           |
        | gemma  | gemma         | 25000        | always                            |             |
      When the agent acts citing profile "haiku" with model "claude-haiku" and 8000 tokens at day 1 15:00
      Then the action verifies
      And the log shows 8000 tokens consumed on profile "haiku"

    @wip
    Scenario: An action must cite a budget profile when the mandate carries budgets
      Given a mandate with two budget profiles
      When the agent acts without citing any budget_ref
      Then the action is refused

    @wip
    Scenario: A token budget refuses the action that would overflow it
      Given a profile "haiku" with a 10000 token budget
      And 8000 tokens already consumed on "haiku"
      When the agent acts citing "haiku" with 3000 declared tokens
      Then the action is refused as over budget
      But an action citing "haiku" with 2000 declared tokens verifies

    @wip
    Scenario: Exhausting one profile switches the OR to the next
      Given the founding two-profile mandate
      And profile "haiku" has spent its single action
      When the agent acts citing profile "gemma" with model "gemma" and 5000 tokens
      Then the action verifies

    @wip
    Scenario: A model outside the profile's list is refused
      Given a profile "haiku" allowing model "claude-haiku"
      When the agent acts citing "haiku" with model "gpt-oss"
      Then the action is refused as model not allowed

    @wip
    Scenario: A profile is only satisfiable inside its own windows
      Given the founding two-profile mandate
      When the agent acts citing profile "haiku" at day 2 09:00
      Then the action is refused as out of window
      But the same action citing profile "gemma" verifies

    @wip
    Scenario: Citing an unknown profile fails closed
      Given a mandate with two budget profiles
      When the agent acts citing budget_ref "grok-unlimited"
      Then the action is refused

    @wip
    Scenario: A delegate's spend drains the same profile — budgets are subtree counts
      Given the founding two-profile mandate with issue depth 1
      And the agent delegates the gemma perimeter to a helper
      When the helper acts citing "gemma" with 20000 tokens
      Then an agent action citing "gemma" with 6000 tokens is refused as over budget

    @wip
    Scenario: Token consumption is tallied from the log alone
      Given a profile "gemma" with a 25000 token budget
      And logged actions of 5000, 7000 and 9000 tokens on "gemma"
      Then any verifier counts 21000 tokens consumed on "gemma"
      And the remaining budget admits at most 4000 declared tokens

  Rule: Declared values count at tier V — attestation bridges reality back

    @wip
    Scenario: A profile requiring attestation refuses a bare declaration
      Given a profile "haiku" that requires attestation
      When the agent acts citing "haiku" with a declared usage and no receipt
      Then the action is refused

    @wip
    Scenario: A provider-signed usage receipt satisfies the attestation hook
      Given a profile "haiku" that requires attestation
      And a provider attestation key pinned in the mandate
      When the agent acts citing "haiku" carrying a receipt signed by the provider
      Then the action verifies
      And the receipt's usage overrides the declared tokens in the tally

    @wip
    Scenario: A receipt signed by the wrong key is refused
      Given a profile "haiku" that requires attestation
      When the agent acts carrying a receipt signed by the agent itself
      Then the action is refused

    @wip
    Scenario: A receipt bound to a different action does not transfer
      Given a profile "haiku" that requires attestation
      And a valid receipt for an earlier action's args_hash
      When the agent replays that receipt on a new action
      Then the action is refused

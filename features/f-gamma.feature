Feature: The gamma log
  Every mutation and every agentic action leaves one hash-chained, signed
  entry in the gamma log. It is the history spine and the counter the
  serverless design otherwise lacks: budgets, liveness and freshness are
  enforced by tallying files — a server is never a trust party. (spec 07)

  Rule: The log is a write-once, hash-chained record

    @wip
    Scenario: Entries chain and the manifest pins the head
      Given an initialised bundle
      When the owner appends a section addition and a heartbeat
      Then the log verifies offline
      And the manifest pins the log head

    @wip
    Scenario: The chain crosses monthly segments transparently
      Given a bundle with entries logged in two different months
      Then the log lives in two pinned segment files
      And the whole chain verifies across the boundary

    @wip
    Scenario: A tampered past entry breaks the chain
      Given a bundle with a three-entry log
      When one byte of the middle entry is altered
      Then log verification is rejected

    @wip
    Scenario: An entry claiming a wrong predecessor is rejected
      Given a bundle with a three-entry log
      When an entry is appended whose prev is not the current head
      Then log verification is rejected

  Rule: Every entry names its authority — owner key or mandate chain

    @wip
    Scenario: An owner entry is signed by the content key alone
      Given an initialised bundle
      When the owner appends a section addition
      Then the entry verifies with no mandate attached

    @wip
    Scenario: A delegated action carries its full mandate chain
      Given an agent granted action rights on connector "x.gmail"
      When the agent appends an action entry
      Then the entry verifies against the chain at its own timestamp

    @wip
    Scenario: An action timestamped outside its mandate window is rejected
      Given an agent granted action rights for 7 days
      When the agent appends an action entry timestamped at day 8
      Then log verification is rejected

  Rule: The log is the agentic meter — budgets are tallies over files

    @wip
    Scenario: The action after the budget is rejected
      Given an agent granted action rights with max_actions 3
      When the agent appends three action entries
      Then a fourth action entry is rejected

    @wip
    Scenario: A delegate's actions drain every ancestor's budget
      Given an agent granted action rights with max_actions 3 and issue depth 1
      And the agent delegates its perimeter to a helper
      When the agent appends one action and the helper appends two
      Then a further action by either key is rejected

    @wip
    Scenario: Minting a child is itself a counted, logged act
      Given an agent granted issue rights with max_children 2
      When the agent delegates twice, each grant logged
      Then a third delegation is rejected

    @wip
    Scenario: The windowed rate limit counts inside its window only
      Given an agent granted action rights with max_actions_per 2 per 24 hours
      When the agent appends two actions on day 1
      Then a third action on day 1 is rejected
      But an action on day 2 verifies

    @wip
    Scenario: A per-action-kind budget counts only its kind
      Given an agent granted gmail actions with rate_limit 2 "reply" per 72 hours
      When the agent appends two "reply" actions on day 1
      Then a third "reply" on day 2 is rejected
      But a "label" action on day 2 verifies

    @wip
    Scenario: An unlogged grant is a silent action — its chain is dead
      Given an agent that minted a sub-mandate without logging the grant
      When the helper presents an action under that chain
      Then the action is rejected

  Rule: Heartbeat bounds autonomy to owner presence

    @wip
    Scenario: A fresh beacon keeps the head mandate alive
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 20 verifies

    @wip
    Scenario: Owner silence beyond every plus grace suspends the mandate
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 34 is rejected

    @wip
    Scenario: The owner's return resumes a suspended mandate
      Given a head mandate suspended by owner silence
      When the owner beacons again
      Then the next action verifies

    @wip
    Scenario: An agent can never beacon for itself
      Given a head mandate with heartbeat every 30 days grace 72 hours
      When the head agent forges a heartbeat with its own key
      Then the beacon is rejected

  Rule: Off-log artifacts anchor to a recent head — backdating is bounded

    @wip
    Scenario: A request anchored to the current head verifies
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to the current log head
      Then the request verifies

    @wip
    Scenario: A stale anchor is rejected
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to a head 48 hours old
      Then the request is rejected

  Rule: The log reveals the act, never the content — reading is owner-first

    @wip
    Scenario: A mutation body is sealed under the target's own key
      Given a published bundle with a circle section
      When the owner logs a modification of that section
      Then a reader of that section opens the entry body
      But the entry alone reveals only kind, time and author — not the target

    @wip
    Scenario: A stranger holding the files sees the counting skeleton only
      Given a bundle whose log records mutations and actions
      When someone with no key reads the log files
      Then the chain and the budgets still verify
      But no target, tag or content is revealed

    @wip
    Scenario: A subtree grant opens exactly its entries and nothing more
      Given logged mutations on sections under "projets" and under "sante"
      When the owner grants the agent read on circle folder "projets"
      Then the agent opens the bodies of the "projets" entries by their hints
      But the "sante" entry bodies stay sealed to it

    @wip
    Scenario: Appending never requires reading
      Given an agent granted action rights and no read grant
      When the agent appends an action entry knowing only the pinned log head
      Then the entry chains and verifies

Feature: The gamma log
  Every mutation and every agentic action leaves one hash-chained, signed
  entry in the gamma log. It is the history spine and the counter the
  serverless design otherwise lacks: budgets, liveness and freshness are
  enforced by tallying files — a server is never a trust party. (spec 07)

  Rule: The log is a write-once, hash-chained record

    Scenario: Entries chain and the manifest pins the head
      Given an initialised bundle
      When the owner appends a section addition and a heartbeat
      Then the log verifies offline
      And the manifest pins the log head

    Scenario: The chain crosses monthly segments transparently
      Given a bundle with entries logged in two different months
      Then the log lives in two pinned segment files
      And the whole chain verifies across the boundary

    Scenario: A tampered past entry breaks the chain
      Given a bundle with a three-entry log
      When one byte of the middle entry is altered
      Then log verification is rejected

    Scenario: An entry claiming a wrong predecessor is rejected
      Given a bundle with a three-entry log
      When an entry is appended whose prev is not the current head
      Then log verification is rejected

  Rule: Every entry names its authority — owner key or mandate chain

    Scenario: An owner entry is signed by the content key alone
      Given an initialised bundle
      When the owner appends a section addition
      Then the entry verifies with no mandate attached

    Scenario: A delegated action carries its full mandate chain
      Given an agent granted action rights on connector "x.gmail"
      When the agent appends an action entry
      Then the entry verifies against the chain at its own timestamp

    Scenario: An action timestamped outside its mandate window is rejected
      Given an agent granted action rights for 7 days
      When the agent appends an action entry timestamped at day 8
      Then log verification is rejected

  Rule: The log is the agentic meter — budgets are tallies over files

    Scenario: The action after the budget is rejected
      Given an agent granted action rights with max_actions 3
      When the agent appends three action entries
      Then a fourth action entry is rejected

    Scenario: A delegate's actions drain every ancestor's budget
      Given an agent granted action rights with max_actions 3 and issue depth 1
      And the agent delegates its perimeter to a helper
      When the agent appends one action and the helper appends two
      Then a further action by either key is rejected

    Scenario: Minting a child is itself a counted, logged act
      Given an agent granted issue rights with max_children 2
      When the agent delegates twice, each grant logged
      Then a third delegation is rejected

    Scenario: The windowed rate limit counts inside its window only
      Given an agent granted action rights with max_actions_per 2 per 24 hours
      When the agent appends two actions on day 1
      Then a third action on day 1 is rejected
      But an action on day 2 verifies

    Scenario: A per-action-kind budget counts only its kind
      Given an agent granted gmail actions with rate_limit 2 "reply" per 72 hours
      When the agent appends two "reply" actions on day 1
      Then a third "reply" on day 2 is rejected
      But a "label" action on day 2 verifies

    Scenario: An unlogged grant is a silent action — its chain is dead
      Given an agent that minted a sub-mandate without logging the grant
      When the helper presents an action under that chain
      Then the action is rejected

  Rule: Heartbeat bounds autonomy to owner presence

    Scenario: A fresh beacon keeps the head mandate alive
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 20 verifies

    Scenario: Owner silence beyond every plus grace suspends the mandate
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 34 is rejected

    Scenario: The owner's return resumes a suspended mandate
      Given a head mandate suspended by owner silence
      When the owner beacons again
      Then the next action verifies

    Scenario: An agent can never beacon for itself
      Given a head mandate with heartbeat every 30 days grace 72 hours
      When the head agent forges a heartbeat with its own key
      Then the beacon is rejected

  Rule: Off-log artifacts anchor to a recent head — backdating is bounded

    Scenario: A request anchored to the current head verifies
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to the current log head
      Then the request verifies

    Scenario: A stale anchor is rejected
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to a head 48 hours old
      Then the request is rejected

  Rule: The log reveals the act, never the content — reading is owner-first

    Scenario: A mutation body is sealed under the target's own key
      Given a published bundle with a circle section
      When the owner logs a modification of that section
      Then a reader of that section opens the entry body
      But the entry alone reveals only kind, time and author — not the target

    Scenario: A stranger holding the files sees the counting skeleton only
      Given a bundle whose log records mutations and actions
      When someone with no key reads the log files
      Then the chain and the budgets still verify
      But no target, tag or content is revealed

    Scenario: A subtree grant opens exactly its entries and nothing more
      Given logged mutations on sections under "projets" and under "sante"
      When the owner grants the agent read on circle folder "projets"
      Then the agent opens the bodies of the "projets" entries by their hints
      But the "sante" entry bodies stay sealed to it

    Scenario: Appending never requires reading
      Given an agent granted action rights and no read grant
      When the agent appends an action entry knowing only the pinned log head
      Then the entry chains and verifies

  Rule: Auditing and searching gamma — the owner by default, others by mandate

    Scenario: The owner searches its log by filter
      Given a bundle whose log records mutations and actions over two months
      When the owner queries actions of kind "action" on "x.gmail" from day 10 to day 40
      Then exactly the matching entries come back
      And the owner opens every sealed body among them

    Scenario: An audit mandate opens the whole log
      Given logged mutations by the owner and by an agent under "projets"
      When the owner grants an auditor read.gamma with the zone keys
      Then the auditor opens every entry body, including acts it never made

    Scenario: A scoped audit mandate is honored dimension by dimension
      Given an auditor granted read.gamma on action "reply" from day 1 to day 30
      When the auditor queries replies of day 20
      Then the matching entries come back
      But a query for day 40 is refused
      And a query for action "send" is refused

  Rule: Gamma replays every protocol consumption semantically

    @wip
    Scenario Outline: Each canonical operation is authorized before its entry joins history
      Given a candidate Gamma entry for "<operation class>" by "<actor>"
      When Core replays it against the exact historical prefix
      Then form, time, signer, actor authority and operation coverage are verified
      And applicable revocation, constraints, receipts and counters are consumed
      And only then does the entry join replay state

      Examples:
        | operation class      | actor   |
        | Ethos create         | owner   |
        | Ethos edit           | grantee |
        | Ethos delete         | grantee |
        | connector action     | grantee |
        | metered inference    | grantee |
        | journalized read     | grantee |
        | sub-grant            | grantee |
        | scoped revocation    | grantee |
        | disjoint merge kind:merge | grantee |

    @wip
    Scenario Outline: A structurally valid entry with invalid semantics is refused
      Given a hash-linked and correctly encoded candidate Gamma history
      When replay encounters "<semantic defect>"
      Then semantic replay is refused at that entry
      And no later entry or counter is accepted

      Examples:
        | semantic defect                                  |
        | historical action N plus 1 beyond its limit      |
        | receipt replayed under another consumption       |
        | stale heartbeat or freshness state               |
        | consumption at or after effective revocation     |
        | sub-grant absent from Gamma                      |
        | direct-child grant beyond max_children           |
        | mutation outside the authorized opaque SID       |
        | valid signature presented under the wrong chain  |
        | owner entry signed by a different owner key      |
        | delegated entry signed without leaf possession   |

    @wip
    Scenario: A log link and signature never substitute for semantic verification
      Given a Gamma chain whose hashes, order and signatures all verify
      But one delegated mutation is outside its mandate perimeter
      When the bundle performs cold verification
      Then the edition is rejected as semantically invalid
      And no structural-only helper reports the history authorized

  Rule: One typed operation occurrence has one cross-view commitment

    @wip
    Scenario: Append-time and public evidence identify the same operation occurrence
      Given one fresh typed operation occurrence
      When its append-time, Gamma, authorship and edition views are projected
      Then every applicable view yields the same operation commitment
      And changing any applicable authority fact changes that commitment
      And no private operation argument appears in public commitment material

    @wip
    Scenario: Identical effects remain distinct occurrences
      Given two typed operation occurrences with identical effects and distinct occurrence anchors
      When their operation commitments are derived
      Then the commitments differ

    @wip
    Scenario: Cross-view evidence does not create another logical occurrence
      Given the applicable Gamma, authorship and edition views of one typed operation occurrence
      When semantic replay correlates their operation commitments
      Then all evidence refers to exactly one logical occurrence
      And no additional occurrence is inferred from the number of evidence views

    @wip
    Scenario: Historical evidence is never retrofitted with an operation commitment
      Given historical protocol evidence predating operation commitments
      When it is verified under its declared historical protocol version
      Then its bytes and hashes remain unchanged
      And no operation commitment is synthesized
      And no existing args_hash, Gamma identifier or edition hash is reinterpreted as that commitment
      And commitment material is refused under a historical or unknown protocol version, or without a version

  Rule: Append-time and cold-time share one replay front door

    @wip
    Scenario Outline: The same candidate and prefix produce the same typed verdict
      Given identical public facts for "<case>"
      When the candidate is checked before append and after export to a fresh store
      Then the verdict, accepted prefix and counters are identical

      Examples:
        | case                                  |
        | valid owner mutation                  |
        | valid delegated mutation              |
        | valid connector action                |
        | revoked delegated mutation            |
        | exhausted action counter              |
        | missing public obligation receipt     |

    @wip
    Scenario: Active revocations are derived only from verified historical entries
      Given a hash-linked Gamma file containing a forged revocation entry
      When cold replay reconstructs active revocations
      Then the forged entry is rejected before it can revoke or authorize anything

Feature: Mandates and the offline verifier
  A mandate is a pure certificate: it grants a keypair a perimeter, under
  constraints, for a window — verifiable from files alone, at an injected
  time T. The key material travels separately, as header lines. Delegation
  attenuates: policy (covers) and physics (derivation) must both agree.
  (spec 04, 05)

  Rule: A mandate is a pure certificate, verifiable offline

    @wip
    Scenario: A grant verifies inside its window and dies at expiry
      Given an owner and an agent keypair
      When the owner grants the agent read on circle folder "projets" for 7 days
      Then the mandate verifies at day 1
      And the mandate is rejected at day 8

    @wip
    Scenario: The kex binding is checked, not trusted
      Given a mandate whose kex_pubkey does not match its signing key
      Then mandate verification is rejected

  Rule: A grant delivers exactly its perimeter — certificate AND key

    @wip
    Scenario: A folder grant opens the subtree
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent read on circle folder "projets"
      Then the agent reads "projets/perso/note1" with its own keypair

    @wip
    Scenario: The founding use case — a folder-local tag view grant
      Given circle sections "note1" tagged "toto" and "note2" untagged in folder "projets/perso"
      When the owner grants the agent read on folder "projets/perso" restricted to tag "toto"
      Then the agent reads "note1"
      But "note2" stays out of the agent's reach

    @wip
    Scenario: An agent never reads outside its perimeter
      Given circle sections in sibling folders "projets/perso" and "projets/pro"
      When the owner grants the agent read on circle folder "projets/perso"
      Then the agent cannot read the section under "projets/pro"

  Rule: Delegation attenuates, offline, without the owner

    @wip
    Scenario: A delegate re-grants a narrower perimeter on its own
      Given an agent granted read on circle folder "projets" with issue depth 1
      When the agent delegates read on folder "projets/perso" to a helper
      Then the helper's chain verifies
      And the helper reads the section under "projets/perso"

    @wip
    Scenario: An over-wide sub-mandate is rejected
      Given an agent granted read on circle folder "projets/perso" with issue depth 1
      When the agent delegates read on folder "archives" to a helper
      Then the helper's chain is rejected

    @wip
    Scenario: A sub-mandate cannot outlive its parent
      Given an agent granted read on circle folder "projets" for 7 days with issue depth 1
      When the agent delegates the same perimeter to a helper for 30 days
      Then the helper's chain is rejected

    @wip
    Scenario: Exhausted depth cannot delegate further
      Given a helper at the end of a depth-1 chain
      When the helper tries to delegate to a fourth key
      Then the new chain is rejected

Feature: Identity genesis
  The owner's whole identity derives from a single 32-byte master seed S.
  Everything is recomputed on demand; only S is ever backed up. (spec 01.1)

  Rule: Genesis is deterministic

    Scenario: The same seed always yields the same identity
      Given a master seed
      When I derive the owner keys twice
      Then both derivations yield the same public identity

    Scenario: Different seeds yield unrelated identities
      Given two different master seeds
      When I derive the owner keys from each seed
      Then the two identities share no public key

  Rule: Keys are domain-separated

    Scenario: One identity's keys are pairwise distinct
      Given a master seed
      When I derive the owner keys
      Then the three public keys are pairwise distinct

  Rule: Genesis fails closed

    Scenario: A seed must be exactly 32 bytes
      Given a 31-byte seed candidate
      When I try to derive the owner keys
      Then genesis is rejected with "invalid seed length"

  Rule: The succession key is independent and cold

    @wip
    Scenario: The succession key is not derived from the master seed
      Given a master seed
      When I generate a succession keypair twice for the same seed
      Then the two succession keys differ
      And the owner keys are identical both times

  Rule: The DID document publishes the identity

    @wip
    Scenario: The DID document lists the four public keys
      Given a master seed and a succession keypair
      When I build the DID document
      Then it contains the root, content, kex and succession public keys
      And its identifier is derived from the root public key
      And its signature verifies under the root key

    @wip
    Scenario: A tampered DID document fails closed
      Given a signed DID document
      When one byte of it is altered
      Then verification is rejected

  Rule: Only the succession key can declare a new master key

    @wip
    Scenario: An epoch transition signed by the succession key is accepted
      Given an identity and its successor identity
      When the transition is signed by the succession key
      Then the successor DID document is accepted

    @wip
    Scenario: An epoch transition signed by anything else is rejected
      Given an identity and its successor identity
      When the transition is signed by the root key itself
      Then the transition is rejected

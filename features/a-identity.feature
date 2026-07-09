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
      Then the five public keys are pairwise distinct

  Rule: Genesis fails closed

    Scenario: A seed must be exactly 32 bytes
      Given a 31-byte seed candidate
      When I try to derive the owner keys
      Then genesis is rejected with "invalid seed length"

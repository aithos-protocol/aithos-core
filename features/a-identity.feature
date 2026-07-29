Feature: Identity genesis
  The owner's whole identity derives from a single 32-byte master seed S.
  Everything is recomputed on demand; only S is ever backed up. (spec 01.1)

  # Audit markers do not skip scenarios: they make a known semantic gap
  # visible while the current behavior keeps running as a regression test.
  # Tracking: docs/audits/features/a-identity.md

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

    @audit-partial @aid-003 @aid-004
    # AUDIT AID-003/AID-004 — PARTIAL
    # The step injects two fixed, independently chosen entropy values.
    # It proves neither that production genesis keeps succession independent
    # from the owner master nor that its private key remains in cold custody.
    # Required: exercise the real creation surfaces with an independent
    # succession source and verify the custody boundary.
    # Detail: docs/audits/features/a-identity.md#aid-003
    Scenario: The succession key is not derived from the master seed
      Given a master seed
      When I generate a succession keypair twice for the same seed
      Then the two succession keys differ
      And the owner keys are identical both times

  Rule: The DID document publishes the identity

    Scenario: The DID document lists the four public keys
      Given a master seed and a succession keypair
      When I build the DID document
      Then it contains the root, content, kex and succession public keys
      And its identifier is derived from the root public key
      And its signature verifies under the root key

    @audit-partial @aid-001
    # AUDIT AID-001 — PARTIAL
    # This scenario only changes one signed field while retaining the old
    # signature. It does not cover correctly signed but semantically malformed
    # DID keys, metadata, versions, or unknown fields on the JSON wire.
    # Required: reject every malformed signed DID through the shared Core
    # verifier and through the public parsing surfaces.
    # Detail: docs/audits/features/a-identity.md#aid-001
    Scenario: A tampered DID document fails closed
      Given a signed DID document
      When one byte of it is altered
      Then verification is rejected

  Rule: Only the succession key can declare a new master key

    @audit-false-positive @aid-002
    # AUDIT AID-002 — SEMANTIC FALSE POSITIVE
    # The current step verifies EpochTransition against only the previous DID
    # document. The successor document is reduced to next.id, then discarded.
    # Required: verify previous document + transition + successor document,
    # including both DID bindings, distinct identities, and both signatures.
    # Detail: docs/audits/features/a-identity.md#aid-002
    Scenario: An epoch transition signed by the succession key is accepted
      Given an identity and its successor identity
      When the transition is signed by the succession key
      Then the successor DID document is accepted

    Scenario: An epoch transition signed by anything else is rejected
      Given an identity and its successor identity
      When the transition is signed by the root key itself
      Then the transition is rejected

Feature: Identity genesis
  The owner's whole identity derives from a single 32-byte master seed S.
  Everything is recomputed on demand; only S is ever backed up. (spec 01.1)

  # Audit markers do not skip scenarios: they make a known semantic gap
  # visible while the current behavior keeps running as a regression test.
  # Review round 1: AID-002 and AID-005 are verified within the pilot scope.
  # AID-001 requires a Provider protocol decision before any correction round
  # 2; AID-003 and AID-004 remain open below.
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

    Scenario: A DID document altered after signing fails closed
      Given a signed DID document
      When one byte of it is altered after signing
      Then verification is rejected

  Rule: A correct root signature is necessary, never sufficient

    # Each document below is REBUILT and correctly re-signed under its own
    # root key, so only the semantic control under test can explain the
    # rejection. (AID-001)
    Scenario Outline: A correctly signed but semantically invalid DID document is rejected
      Given a signed DID document
      When it is rebuilt and re-signed with <defect>
      Then verification is rejected

      Examples:
        | defect                                  |
        | a content key that is not multibase     |
        | a content key in the X25519 codec       |
        | a kex key in the Ed25519 codec          |
        | a malformed succession key              |
        | an unsupported document version         |
        | an unsupported signature algorithm      |
        | a signature fragment other than #root   |

    # The verified JCS is rebuilt from the typed value: a member that is
    # dropped on the way in would be a signed-then-erased field. (AID-001)
    Scenario Outline: An unknown member on the DID wire is refused, not dropped
      Given a signed DID document
      When <member> is added to its JSON wire
      Then the document does not parse as a DID document

      Examples:
        | member                     |
        | an unknown top-level member |
        | an unknown keys member      |
        | an unknown signature member |

  Rule: Only the succession key can declare a new master key

    Scenario: An epoch transition signed by the succession key is accepted
      Given an identity and its successor identity
      When the transition is signed by the succession key
      Then the successor DID document is accepted

    Scenario: An epoch transition signed by anything else is rejected
      Given an identity and its successor identity
      When the transition is signed by the root key itself
      Then the transition is rejected

  Rule: A transition binds the successor document it names

    # A declaration that names a successor proves nothing about the document
    # actually presented: the whole triple is verified. (AID-002)
    Scenario Outline: A transition that does not bind its successor is rejected
      Given an identity and its successor identity
      When the transition is signed by the succession key but <defect>
      Then the transition is rejected

      Examples:
        | defect                                             |
        | another successor document is presented            |
        | the successor document is altered after signing    |
        | the successor document is re-signed while malformed |
        | it declares the previous identity as its successor |
        | it declares a malformed next_did                   |
        | it declares a next_did that is not a did:aithos    |
        | it is signed by another identity's succession key  |
        | it names a previous identity it was not signed for |
        | it declares an unsupported version                 |
        | it declares an unsupported signature algorithm     |

    Scenario: A transition signed by the root key while claiming the succession fragment is rejected
      Given an identity and its successor identity
      When the transition is signed by the root key claiming to be the succession key
      Then the transition is rejected

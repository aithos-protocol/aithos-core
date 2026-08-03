@c-headers
Feature: Headers — sealed node keys
  The header is the only place a node key is ever stored, and it is stored
  sealed: one line per authorized identity, always including the owner (I3).
  Grant = append a line, O(1). Rotation = new key version without the
  revoked, plus an up-link wrap restoring parent derivation. (spec 03)

  Rule: A line seals the node key to exactly one recipient

    Scenario: Owner and grantee each open their line
      Given a node key and two recipients, the owner and a grantee
      When the node key is sealed into a header
      Then the owner opens the header and recovers the node key
      And the grantee opens the header and recovers the node key

    Scenario: A non-recipient opens nothing
      Given a sealed header for the owner and a grantee
      When a third keypair tries every line
      Then it recovers nothing

    @audit-partial @chdr-002 @chdr-027
    # AUDIT CHDR-002 — PARTIAL; CHDR-027 — PARTIAL.
    # No positive control inside the scenario: nothing establishes that the
    # owner line opened BEFORE the corruption, so a fixture regression that
    # made it permanently unopenable would keep this scenario green. The only
    # positive control of the Rule lives in another scenario.
    # Detail: docs/audits/features/c-headers.md
    Scenario: A corrupted line fails closed
      Given a sealed header for the owner and a grantee
      When one byte of a line's ciphertext is corrupted
      Then opening that line is rejected

    @audit-partial @chdr-001 @chdr-025 @chdr-002 @chdr-027
    # AUDIT CHDR-001 — PARTIAL; CHDR-025 — PARTIAL; CHDR-002, CHDR-027.
    # Only the node half of the binding is exercised: both headers are built
    # at version 1 and the open is at version 1, so key_version never varies.
    # Outside Gherkin the version binding is defended only by byte pins
    # against vectors, never by a behavioural differential (CHDR-025).
    # Detail: docs/audits/features/c-headers.md
    Scenario: A line is bound to its node and version
      Given a sealed header for the owner on one node
      When its owner line is replayed on a different node's header
      Then opening it there is rejected

  Rule: The owner line is mandatory (I3)

    @audit-partial @chdr-009 @chdr-011 @chdr-010 @chdr-007 @chdr-012
    # AUDIT CHDR-009 — PARTIAL; CHDR-011, CHDR-010 — PARTIAL.
    # Only the build-time I3 gate is exercised on its fail-closed side; the
    # normative case declared by vectors/g2-rotation.json has no consumer.
    # CHDR-007 and CHDR-012 — DECISION_REQUIRED. Both concern I3 on surfaces
    # this scenario never crosses: the edition verifiers, and the field on
    # which the owner line is identified. A human owner decides the protocol
    # reading before any correction; neither is assigned to a corrector.
    # Detail: docs/audits/features/c-headers.md
    Scenario: A header without an owner line is invalid
      Given a node key and a single grantee recipient
      When a header is built without the owner line
      Then the header is rejected as invalid

  Rule: Grant is one appended line, touching nobody

    @audit-partial @chdr-013 @chdr-014 @chdr-016 @chdr-015 @chdr-017 @chdr-018
    # AUDIT CHDR-013, CHDR-014, CHDR-016 — PARTIAL; CHDR-015, CHDR-017,
    # CHDR-018 — PARTIAL.
    # "every other line" is exercised on a header holding exactly one other
    # line; neither the line count nor the position is asserted after the
    # append; and the production grant surface is touched by no step of this
    # Rule.
    # Detail: docs/audits/features/c-headers.md
    Scenario: Granting a new reader leaves every other line untouched
      Given a sealed header for the owner
      When a line for a new grantee is appended
      Then the new grantee opens the node key
      And the owner line is byte-identical to before

  Rule: Rotation cuts the revoked and re-links the parent

    @audit-partial @chdr-019 @chdr-024
    # AUDIT CHDR-019 — PARTIAL; CHDR-024 — PARTIAL.
    # "cannot open" is decided by the kid routing hint, which the spec
    # declares non-authorizing: the seal is never reached. No assertion reads
    # key_versions["2"].lines, and no step of this Rule calls check_rotation.
    # Detail: docs/audits/features/c-headers.md
    Scenario: The revoked gets no line in the new version
      Given a sealed header for the owner and two grantees
      When the node is rotated without the first grantee
      Then the surviving grantee opens the new node key
      And the first grantee cannot open the new version
      And the owner opens the new version too

    @audit-semantic-false-positive @chdr-021 @chdr-020 @chdr-026
    # AUDIT CHDR-021 — SEMANTIC_FALSE_POSITIVE; CHDR-020, CHDR-026 — PARTIAL.
    # The scenario contains no derived node, no rotation and no content-tree
    # derivation: it seals a constant under a constant and reopens it two
    # steps later under the same constant. What is established is only
    # wrap_open(wrap_seal(k, dk)) == dk.
    # Detail: docs/audits/features/c-headers.md
    Scenario: An up-link wrap restores derivation for the parent holder
      Given a derived node rotated to a fresh random key
      When the rotator posts the up-link wrap under the parent key
      Then a parent holder recovers the new node key through the wrap

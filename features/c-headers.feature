@c-headers
Feature: Headers — sealed node keys
  The header is the only place a node key is ever stored, and it is stored
  sealed: one line per authorized identity, always including the owner (I3).
  Grant = append a line, O(1). Rotation = new key version without the
  revoked, plus an up-link wrap restoring parent derivation. (spec 03)

  # Audit round 1 (2026-07-30): docs/audits/features/c-headers.md
  # Markers below name unresolved findings only. Remove each one when its
  # finding is independently VERIFIED.

  Rule: A line seals the node key to exactly one recipient

    @chdr-009 @chdr-013
    # PROVEN. CHDR-009: no scenario reaches vectors/c1-header-seal.json, so the
    # Header layer's wire bytes are never anchored to independent bytes.
    # CHDR-013: the Given establishes no state; the When re-creates it.
    Scenario: Owner and grantee each open their line
      Given a node key and two recipients, the owner and a grantee
      When the node key is sealed into a header
      Then the owner opens the header and recovers the node key
      And the grantee opens the header and recovers the node key

    @chdr-008
    # PARTIAL. CHDR-008: "every line" is a hardcoded kid list, and the Then
    # never ties the attempt count to the header's line count.
    Scenario: A non-recipient opens nothing
      Given a sealed header for the owner and a grantee
      When a third keypair tries every line
      Then it recovers nothing

    @chdr-007
    # PARTIAL. CHDR-007: bare is_err() with no positive control. Header::open
    # returns the same error for corruption, wrong key, wrong AAD and for no
    # matching line at all, so the stated cause is not attributed.
    Scenario: A corrupted line fails closed
      Given a sealed header for the owner and a grantee
      When one byte of a line's ciphertext is corrupted
      Then opening that line is rejected

    @chdr-006 @chdr-007
    # PARTIAL. CHDR-006: only the node component of the AAD is varied; the
    # "and version" half of this scenario is never exercised.
    Scenario: A line is bound to its node and version
      Given a sealed header for the owner on one node
      When its owner line is replayed on a different node's header
      Then opening it there is rejected

  Rule: The owner line is mandatory (I3)

    @chdr-013 @chdr-014
    # PROVEN. CHDR-014: the rejection is matched by Display substring, not by
    # error type. CHDR-013: the Given establishes no state.
    # See also CHDR-015 (DECISION_REQUIRED): I3 is not enforced at edition level.
    Scenario: A header without an owner line is invalid
      Given a node key and a single grantee recipient
      When a header is built without the owner line
      Then the header is rejected as invalid

  Rule: Grant is one appended line, touching nobody

    @chdr-010 @chdr-011 @chdr-012
    # PARTIAL. CHDR-010: the fixture header has exactly one other line, so
    # "every other line" and O(1) are not exercised. CHDR-011: "DK unchanged"
    # is entailed, not asserted. CHDR-012: the byte-identity check is position
    # and cardinality blind.
    Scenario: Granting a new reader leaves every other line untouched
      Given a sealed header for the owner
      When a line for a new grantee is appended
      Then the new grantee opens the node key
      And the owner line is byte-identical to before

  Rule: Rotation cuts the revoked and re-links the parent

    @chdr-002 @chdr-003 @chdr-004 @chdr-005
    # PARTIAL. CHDR-002: "gets no line" is proved only as "cannot open";
    # key_versions["2"].lines is never inspected. CHDR-003: check_rotation owns
    # this Rule's contract and is never called. CHDR-004: the revoked assertion
    # also passes if the rotation never happened.
    Scenario: The revoked gets no line in the new version
      Given a sealed header for the owner and two grantees
      When the node is rotated without the first grantee
      Then the surviving grantee opens the new node key
      And the first grantee cannot open the new version
      And the owner opens the new version too

    @chdr-001 @chdr-005 @chdr-013
    # SEMANTIC_FALSE_POSITIVE. CHDR-001: no derived node, no rotation, no
    # parent and no derivation. The scenario seals a constant under a constant
    # and opens it with the same constant. CHDR-005: this wrap and the rotation
    # above are never joined by executed state.
    Scenario: An up-link wrap restores derivation for the parent holder
      Given a derived node rotated to a fresh random key
      When the rotator posts the up-link wrap under the parent key
      Then a parent holder recovers the new node key through the wrap

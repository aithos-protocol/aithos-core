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

    @audit-partial @chdr-027
    # AUDIT CHDR-027 — PARTIAL.
    # Three of this Rule's four scenarios are built by one fixture
    # constructor. CHDR-027's stated closure criterion — an internal positive
    # control in scenarios 3 and 4 — was met by lot A as a by-product, but no
    # independent review has closed CHDR-027, so the marker stays.
    # Detail: docs/audits/features/c-headers.md
    Scenario: A corrupted line fails closed
      Given a sealed header for the owner and a grantee
      When one byte of a line's ciphertext is corrupted
      Then opening that line is rejected

    @audit-partial @chdr-027
    # AUDIT CHDR-027 — PARTIAL.
    # This scenario borrowed its detection power from a neighbour. CHDR-027's
    # stated closure criterion — an internal positive control here and in
    # scenario 3 — was met by lot A as a by-product, but no independent review
    # has closed CHDR-027, so the marker stays.
    # Detail: docs/audits/features/c-headers.md
    Scenario: A line is bound to its node and version
      Given a sealed header for the owner on one node
      When its owner line is replayed on a different node's header
      Then opening it there is rejected

  Rule: The owner line is mandatory (I3)

    @audit-partial @chdr-011 @chdr-010
    # AUDIT CHDR-011, CHDR-010 — PARTIAL.
    # The I3 rejection is asserted through a string match on "I3" rather than
    # the typed variant, and the scenario's Given is empty.
    # Detail: docs/audits/features/c-headers.md
    Scenario: A header without an owner line is invalid
      Given a node key and a single grantee recipient
      When a header is built without the owner line
      Then the header is rejected as invalid

  Rule: Grant is one appended line, touching nobody

    @audit-partial @chdr-016 @chdr-015 @chdr-017 @chdr-018
    # AUDIT CHDR-016 — OPEN, re-routed 2026-08-04 to g-revocation and d-bundle
    # as chdr-016-grant-path (orchestrator QUEUE.yaml); CHDR-015, CHDR-017,
    # CHDR-018 — PARTIAL.
    # The production grant surface is still touched by no step of this Rule
    # (CHDR-015). The closure criteria of CHDR-017 (structural assertion) and
    # CHDR-018 (two distinct Then functions) were met by lot A as by-products,
    # but no independent review has closed either, so both markers stay.
    # Detail: docs/audits/features/c-headers.md
    Scenario: Granting a new reader leaves every other line untouched
      Given a sealed header for the owner and an existing reader
      When a line for a new grantee is appended
      Then the new grantee opens the node key
      And the owner line is byte-identical to before

  Rule: Rotation cuts the revoked and re-links the parent

    @audit-partial @chdr-024
    # AUDIT CHDR-024 — PARTIAL.
    # The Then now calls check_rotation(2), which was CHDR-024's stated
    # closure criterion, met by lot A as a by-product; no independent review
    # has closed CHDR-024, so the marker stays. The live gap is in the gate
    # itself: check_rotation tests an inclusion where spec/03-headers.md:109-111
    # requires an equality, so a rotation dropping a survivor passes.
    # Detail: docs/audits/features/c-headers.md
    Scenario: The revoked gets no line in the new version
      Given a sealed header for the owner and two grantees
      When the node is rotated without the first grantee
      Then the surviving grantee opens the new node key
      And the first grantee cannot open the new version
      And the owner opens the new version too

    @audit-partial @chdr-020 @chdr-026
    # AUDIT CHDR-020, CHDR-026 — PARTIAL.
    # No negative of the wrap by divergent AAD exists anywhere (CHDR-026); a
    # symmetric mutation of derive_key still survives this scenario
    # (ev-ec9412a7), caught only by the pinned vectors (ev-cbce8aa0).
    # Detail: docs/audits/features/c-headers.md
    Scenario: An up-link wrap restores derivation for the parent holder
      Given a derived node rotated to a fresh random key
      When the rotator posts the up-link wrap under the parent key
      Then a parent holder recovers the new node key through the wrap

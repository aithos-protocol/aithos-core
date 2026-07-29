@b-derivation
Feature: Content-tree derivation
  One BLAKE3 derivation per path segment; holding a folder's key yields its
  whole subtree — present and future — and nothing else. Derivation labels
  use sids, never names. (spec 01.3, 02.5)

  # Audit markers do not skip scenarios: they make a known semantic gap
  # visible while the current behavior keeps running as a regression test.
  # Tracking: docs/audits/features/b-derivation.md

  Rule: Derivation is deterministic and per-segment

    @audit-false-positive @bder-001
    # AUDIT BDER-001 — SEMANTIC FALSE POSITIVE
    # The When calls node_key twice on the same cloned NodePath, so the Then
    # compares a pure function's output to itself. It kills 0 of 5 mutants,
    # including one that ignores both arguments. NodePath::parse is never
    # exercised and the B2 vector is never consulted.
    # Required: rebuild the second path independently and assert byte-exact
    # against vectors/b2-derivation.json.
    # Detail: docs/audits/features/b-derivation.md#bder-001
    Scenario: The same path always yields the same key
      Given a zone key
      And a path of three nested folders ending in a section
      When I derive the section key twice
      Then both derivations yield the same key

    @audit-false-positive @bder-002
    # AUDIT BDER-002 — SEMANTIC FALSE POSITIVE
    # "Unrelated" is encoded as byte inequality. Under a per-segment XOR
    # implementation each sibling key is computable from the other by anyone
    # and the zone key is recoverable, yet this scenario stays green.
    # Required: assert mutual non-derivability and non-leakage of the parent.
    # Detail: docs/audits/features/b-derivation.md#bder-002
    Scenario: Sibling nodes get unrelated keys
      Given a zone key
      When I derive the keys of two sibling folders
      Then the two folder keys are unrelated

  Rule: Holding a folder yields its subtree, nothing else

    @audit-partial @bder-005
    # AUDIT BDER-005 — PARTIAL
    # The strongest scenario of this feature: it crosses two distinct
    # computation routes and kills 5 of 5 mutants. The gap is "every":
    # one section, one depth, one shape, no grandchild, no tag anchor.
    # Detail: docs/audits/features/b-derivation.md#bder-005
    Scenario: A folder holder derives every descendant
      Given a zone key
      And a folder three levels deep containing a section
      When I derive the folder's key from the zone key
      Then the folder key alone derives the section beneath it

    @audit-false-positive @bder-003
    # AUDIT BDER-003 — SEMANTIC FALSE POSITIVE — most serious gap here
    # The Given has an empty body. The Then proves a universal negative with
    # three point inequalities that stay green even when the held key is
    # replaced by garbage, so the assertion is blind to what the When
    # produced. Under a per-segment XOR implementation the Then sentence is
    # false and the scenario is still green. Upward one-wayness, which the
    # Rule title promises with "nothing else", is asserted nowhere.
    # Required: positive control on the held key, enumerated derivation
    # space with its size stated, and an upward assertion.
    # Detail: docs/audits/features/b-derivation.md#bder-003
    Scenario: A folder holder cannot reach sideways
      Given a zone key
      And two sibling folders each containing a section
      When I hold only the first folder's key
      Then no derivation from it yields the second folder's section key

    @audit-false-positive @bder-004
    # AUDIT BDER-004 — SEMANTIC FALSE POSITIVE
    # The When renames nothing: it re-derives an unchanged path. No name, no
    # index row, no descriptor, no bundle. It kills 0 of 5 mutants and guards
    # the wrong risk — the plausible regression is rename implemented as
    # delete-and-recreate with a fresh sid, which this cannot see.
    # An honest rename step already exists at cucumber.rs:7892.
    # Detail: docs/audits/features/b-derivation.md#bder-004
    Scenario: Renaming never re-keys
      Given a zone key and a folder containing a section
      When the folder is renamed
      Then every derived key is unchanged

  Rule: Tag views anchor at folders

    @audit-partial @bder-006
    # AUDIT BDER-006 — PARTIAL — DECISION REQUIRED on this Rule's scope
    # The Then sentence is literally and fully proven. What is unexercised is
    # the anchoring semantics of spec 02.9: an anchor grants nothing downward
    # by derivation, sections enter by wrap, a folder-local view spans only
    # its subtree. That semantics lives in aithos-bundle, not in derive.
    # Detail: docs/audits/features/b-derivation.md#bder-006
    Scenario: A folder-local tag view is its own lock
      Given a zone key and a folder
      When I derive the tag view "toto" at the folder and at the zone root
      Then the two anchors differ from each other and from the folder key

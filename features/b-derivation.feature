@b-derivation
Feature: Content-tree derivation
  One BLAKE3 derivation per path segment; holding a folder's key yields its
  whole subtree — present and future — and nothing else. Derivation labels
  use sids, never names. (spec 01.3, 02.5)

  # Unresolved audit markers remain executable.
  # Detail: docs/audits/features/b-derivation.md
  # BDER-011 — repo-wide, not specific to this feature: the aithos-bundle
  # Cucumber runner exits 0 even when scenarios fail. Read the printed
  # scenario and step counts; the exit code of this feature's gate proves
  # nothing until BDER-011 is closed.

  Rule: Derivation is deterministic and per-segment

    Scenario: The same path always yields the same key
      Given a zone key
      And a path of three nested folders ending in a section
      When I derive the section key twice, the second time from its canonical path text
      Then both derivations yield the same key
      And the key equals the B2 vector's deep section key byte for byte
      And each segment contributed exactly one labelled derivation

    @audit-partial @bder-012
    # AUDIT BDER-012 — PARTIAL; "any production label" is proved over 21
    # sampled labels, and only the first sibling is anchored to the vector.
    Scenario: Sibling nodes get unrelated keys
      Given a zone key
      When I derive the keys of two sibling folders
      Then neither sibling key derives the other under any production label
      And neither sibling key yields the zone key back

  Rule: Holding a folder yields its subtree, nothing else

    Scenario: A folder holder derives every descendant
      Given a zone key
      And a folder three levels deep containing a section
      When I derive the folder's key from the zone key
      Then the folder key alone derives the section beneath it
      And it alone derives a grandchild section and a tag anchor beneath it

    Scenario: A folder holder cannot reach sideways
      Given a zone key
      And two sibling folders each containing a section
      When I hold only the first folder's key
      Then the held key is exactly the first folder's key
      And no derivation from it yields the second folder's section key
      And no derivation from it yields its own parent or the zone key

    Scenario: Renaming never re-keys
      Given a published bundle with section "note1" in circle "projets/perso"
      And the derived key of "projets/perso/note1" is recorded
      When the folder "perso" is renamed to "intime"
      And the edition is republished
      Then the derived key of "projets/intime/note1" is unchanged
      And the owner reads the same section at "projets/intime/note1"

  Rule: Tag views anchor at folders

    @audit-partial @bder-006
    # AUDIT BDER-006 — PARTIAL / DECISION_REQUIRED; see the public audit.
    Scenario: A folder-local tag view is its own lock
      Given a zone key and a folder
      When I derive the tag view "toto" at the folder and at the zone root
      Then the two anchors differ from each other and from the folder key

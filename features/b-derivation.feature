@b-derivation
Feature: Content-tree derivation
  One BLAKE3 derivation per path segment; holding a folder's key yields its
  whole subtree — present and future — and nothing else. Derivation labels
  use sids, never names. (spec 01.3, 02.5)

  Rule: Derivation is deterministic and per-segment

    Scenario: The same path always yields the same key
      Given a zone key
      And a path of three nested folders ending in a section
      When I derive the section key twice
      Then both derivations yield the same key

    Scenario: Sibling nodes get unrelated keys
      Given a zone key
      When I derive the keys of two sibling folders
      Then the two folder keys are unrelated

  Rule: Holding a folder yields its subtree, nothing else

    Scenario: A folder holder derives every descendant
      Given a zone key
      And a folder three levels deep containing a section
      When I derive the folder's key from the zone key
      Then the folder key alone derives the section beneath it

    Scenario: A folder holder cannot reach sideways
      Given a zone key
      And two sibling folders each containing a section
      When I hold only the first folder's key
      Then no derivation from it yields the second folder's section key

    Scenario: Renaming never re-keys
      Given a zone key and a folder containing a section
      When the folder is renamed
      Then every derived key is unchanged

  Rule: Tag views anchor at folders

    Scenario: A folder-local tag view is its own lock
      Given a zone key and a folder
      When I derive the tag view "toto" at the folder and at the zone root
      Then the two anchors differ from each other and from the folder key

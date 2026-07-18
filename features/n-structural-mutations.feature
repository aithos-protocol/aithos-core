Feature: Structural mutations
  Folder and metadata operations use the existing read, edit, append, delete and write
  lattice. Their indexes, tag views, rotations, wraps, Gamma and editions commit atomically.

  Rule: Existing verbs cover sections and folders without a new wire verb

    Scenario Outline: Structural operations require their exact composed authority
      Given a grantee with "<authority>"
      When it attempts "<operation>"
      Then the operation is "<verdict>"

      Examples:
        | operation                         | authority                                      | verdict  |
        | list and read a folder            | read on that folder                            | accepted |
        | list and read a folder            | edit on that folder                            | accepted |
        | list and read a folder            | append on that folder                          | accepted |
        | list and read a folder            | delete on that folder                          | accepted |
        | list and read a folder            | write on that folder                           | accepted |
        | create a child folder             | read on the destination folder                 | refused  |
        | create a child folder             | edit on the destination folder                 | refused  |
        | create a child folder             | delete on the destination folder               | refused  |
        | create a child folder             | append on the destination folder               | accepted |
        | create a child folder             | write on the destination folder                | accepted |
        | rename a folder                   | read on that folder                            | refused  |
        | rename a folder                   | delete on that folder                          | refused  |
        | rename a folder                   | edit on that folder                            | accepted |
        | rename a folder                   | append on that folder                          | accepted |
        | rename a folder                   | write on that folder                           | accepted |
        | delete an empty folder            | read on that folder                            | refused  |
        | delete an empty folder            | edit on that folder                            | refused  |
        | delete an empty folder            | append on that folder                          | refused  |
        | delete an empty folder            | delete on that folder                          | accepted |
        | delete an empty folder            | write on that folder                           | accepted |
        | move a folder                     | edit on source and append on destination       | accepted |
        | move a folder                     | append on source and write on destination      | accepted |
        | move a folder                     | delete on source and append on destination     | refused  |
        | move a folder                     | edit on source only                            | refused  |
        | delete a non-empty folder         | delete covering folder and complete subtree    | accepted |
        | delete a non-empty folder         | delete on folder but not one descendant        | refused  |

    Scenario: Structural read means list and present only covered children
      Given a grantee with read on one nested folder
      When it lists the folder and reads one contained section
      Then only covered children are presented
      And a sibling subtree remains absent and unreadable

  Rule: Derived structure is never an unjournalized side mutation

    Scenario: A tag edit updates every derived view in one transaction
      Given a public or circle section whose authorized edit changes its tags
      When the mutation commits
      Then index rows and affected tag wraps are deterministically derived
      And the authorizing Gamma entry, roots and manifest commit together

    Scenario: A move rotates at the changed cryptographic boundary
      Given an authorized move with source and destination authority
      When the node is reparented
      Then its stable SID follows the node
      And required rotation, survivor lines and destination up-link wrap join the transaction
      And the old parent derives no future node key

    Scenario: A subtree delete accounts for every removed descendant
      Given a grantee delete perimeter covering a folder and its complete subtree
      When the folder is deleted
      Then the derived changeset includes every removed row, blob, header and tag consequence
      And one actor chain covers every non-derived removal

  Rule: Structural refusals are effect-free

    Scenario Outline: Invalid structure or injected failure preserves the bundle byte for byte
      Given a published bundle snapshotted before a structural mutation
      When the mutation encounters "<failure>"
      Then it is refused before canonical effect
      And reopen observes the byte-identical old bundle and Gamma head

      Examples:
        | failure                                      |
        | destination outside the grantee perimeter    |
        | move into the node's own descendant          |
        | destination sibling name collision           |
        | display path traversal outside the zone      |
        | failure while rebuilding tag views           |
        | failure while rotating or rewrapping         |
        | failure before Gamma and manifest linearization |

  Rule: Self structure remains sealed

    Scenario: A self structural mutation uses zone authority or exact opaque SIDs
      Given a grantee mutation in self
      When keyless verification derives its state transition
      Then dir and tag claims never authorize the mutation
      And proofs reveal only allowed opaque SIDs and commitments
      And no folder relationship or display metadata escapes

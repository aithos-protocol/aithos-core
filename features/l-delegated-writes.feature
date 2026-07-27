Feature: Delegated writes — the mandate is a pen, not just a key
  A mandate whose perimeter carries a mutation verb writes sections in the
  keyed zones itself: same tree, same blobs, same log — no owner in the
  loop. The node key is symmetric (whoever can open can seal), so the
  CERTIFICATE is what separates a reader from a writer: the §04.2 verb
  lattice, enforced by the verifier at every append. Agent-authored circle
  content is unsigned in the blob (§02.11) — the delegated gamma entry,
  signed by the grantee key under its chain, IS the authorship evidence.
  (spec 02.11, 04.2, 04.3, 05.3, 07.2)

  Rule: A write grant writes — within its perimeter, and the log records it

    Scenario: An append grant creates a section in the granted folder
      Given a published bundle with a circle folder "projets/perso"
      When the owner grants the agent append on circle folder "projets/perso"
      And the agent adds section "memo" with body "written by the pen" under "projets/perso"
      Then the owner reads "projets/perso/memo" as "written by the pen"
      And the log's last entry is a delegated "section.add" under the agent's mandate

    Scenario: An edit grant rewrites an existing section in place
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent edit on circle folder "projets/perso"
      And the agent rewrites "projets/perso/note1" to "corrected by the agent"
      Then the owner reads "projets/perso/note1" as "corrected by the agent"
      And the log's last entry is a delegated "section.modify" under the agent's mandate

    Scenario: A write grant deletes, and the tree forgets the node
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent write on circle folder "projets/perso"
      And the agent deletes "projets/perso/note1"
      Then the section "projets/perso/note1" is gone from the tree
      And the log's last entry is a delegated "section.delete" under the agent's mandate

    Scenario: The agent reads back its own write with the same keypair
      Given a published bundle with a circle folder "projets/perso"
      When the owner grants the agent append on circle folder "projets/perso"
      And the agent adds section "memo" with body "written by the pen" under "projets/perso"
      Then the agent reads "projets/perso/memo" as "written by the pen"

    Scenario: A delegated write body stays sealed to the keyless
      Given a published bundle with a circle folder "projets/perso"
      When the owner grants the agent append on circle folder "projets/perso"
      And the agent adds section "memo" with body "written by the pen" under "projets/perso"
      Then someone with no key learns neither target nor content from the last entry

  Rule: The verb lattice is enforced — read is never a pen

    Scenario: A read grant cannot rewrite
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent read on circle folder "projets/perso"
      And the agent tries to rewrite "projets/perso/note1" to "vandalism"
      Then the write is rejected as outside the perimeter
      And the log gains no entry

    Scenario: An edit grant cannot create
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent edit on circle folder "projets/perso"
      And the agent tries to add section "memo" with body "too wide" under "projets/perso"
      Then the write is rejected as outside the perimeter

    Scenario: An append grant cannot delete
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent append on circle folder "projets/perso"
      And the agent tries to delete "projets/perso/note1"
      Then the write is rejected as outside the perimeter

    Scenario: A writer stays inside its granted folder
      Given circle sections in sibling folders "projets/perso" and "projets/pro"
      When the owner grants the agent write on circle folder "projets/perso"
      And the agent tries to rewrite the section under "projets/pro" to "overreach"
      Then the write is rejected as outside the perimeter

    Scenario: An expired write mandate writes nothing
      Given a published bundle with a circle folder "projets/perso"
      When the owner grants the agent append on circle folder "projets/perso" for 7 days
      And the agent tries to add section "late" with body "too late" under "projets/perso" at day 8
      Then the write is rejected as outside the window
      And the log gains no entry

  Rule: One mandate carries the whole delegated surface at once

    Scenario: One certificate reads, writes, acts, audits its log, delegates and revokes
      Given a published bundle with section "note1" tagged "toto" in circle "projets/perso"
      When the owner grants the agent one mandate carrying write on "projets/perso", gmail reply, gamma read on actions, issue depth 1 and revoke, max_actions 2, for 30 days
      Then the agent reads "projets/perso/note1" with its own keypair
      And the agent adds section "memo" with body "one pen does it all" under "projets/perso" and it verifies
      And the agent appends a gmail "reply" action and it verifies
      And the agent queries the log for its own action and finds it
      When the agent delegates read on folder "projets/perso" to a helper until day 30
      Then the helper reads "projets/perso/note1" through its delegated line
      When the agent revokes the helper from its own issue authority
      Then the helper's chain is rejected as revoked at day 13
      And a second gmail "reply" action verifies and a third is rejected as budget spent

    Scenario: The super-mandate dies whole at expiry — the owner key does not
      Given a published bundle with section "note1" tagged "toto" in circle "projets/perso"
      When the owner grants the agent one mandate carrying write on "projets/perso", gmail reply, gamma read on actions, issue depth 1 and revoke, max_actions 2, for 30 days
      Then at day 31 the same mandate can neither read, nor write, nor act, nor delegate
      But the owner still writes at day 31

  Rule: Grantee content operations have zone-specific parity

    Scenario Outline: One pure operation enforces the grantee rules of each zone
      Given a published bundle and a grantee with "<authority>"
      When the grantee performs "<operation>" in "<zone>"
      Then the operation is "<verdict>"
      And an accepted operation is journalized and cold-verifiable under the same chain

      Examples:
        | zone   | operation | authority                         | verdict  |
        | public | list      | read.public#dir=projects           | accepted |
        | public | read      | read.public#id=note                | accepted |
        | public | create    | append.public#dir=projects         | accepted |
        | public | edit      | edit.public#id=note                | accepted |
        | public | delete    | delete.public#id=note              | accepted |
        | circle | list      | read.circle#dir=projects           | accepted |
        | circle | read      | read.circle#id=note                | accepted |
        | circle | create    | append.circle#dir=projects         | accepted |
        | circle | edit      | edit.circle#id=note                | accepted |
        | circle | delete    | delete.circle#id=note              | accepted |
        | self   | list      | read.self#dir=sealed               | accepted |
        | self   | read      | read.self#id=opaque-note           | accepted |
        | self   | create    | append.self                        | accepted |
        | self   | create    | append.self#id=preallocated        | accepted |
        | self   | edit      | edit.self#id=opaque-note           | accepted |
        | self   | delete    | delete.self#id=opaque-note         | accepted |
        | self   | edit      | edit.self#dir=sealed               | refused  |
        | self   | delete    | delete.self#tag=private            | refused  |

  Rule: Authority and decryption remain independent fences

    Scenario Outline: A grantee needs both its chain and the exact content line
      Given a grantee holds "<key_material>" and presents "<authority>"
      When it attempts to read the exact protected section
      Then the result is "<verdict>"

      Examples:
        | key_material                    | authority                | verdict                   |
        | exact valid section line        | valid covering chain      | readable and authorized   |
        | exact valid section line        | no mandate chain          | refused as unauthorized   |
        | no section line                 | valid covering chain      | authorized but unreadable |
        | sibling section line            | valid covering chain      | unreadable                |

  Rule: Delegated authorship and self proofs survive cold verification

    Scenario: A public grantee edit never imitates owner authorship
      Given an agent with edit authority on one public section
      When the agent publishes a normal delegated edit
      Then its authorship signature binds content hash, SID, operation, edition and authorized_via
      And Gamma and the manifest commit that signature
      And fresh-store verification labels the grantee, never the owner, as author

    Scenario Outline: A self mutation proves only an opaque state transition
      Given an agent with exact authority for self SID "opaque-note"
      When it performs "<operation>" and publishes
      Then the edition proves "<state_relation>" for that SID
      And reveals no name, path, title, tags, body, folder relation or key

      Examples:
        | operation | state_relation                    |
        | create    | prior absence and new inclusion   |
        | edit      | same-SID replacement              |
        | delete    | prior inclusion and new absence   |

  Rule: A session never freezes authority past the operation

    Scenario Outline: Expiry or revocation after session open is checked before effect
      Given a grantee opened a local bundle session while its chain was valid
      And the mandate becomes "<authority_change>" before the candidate mutation
      When the grantee attempts to commit that mutation
      Then the current pure verdict refuses it
      And the bundle, manifest and Gamma head remain byte-for-byte unchanged

      Examples:
        | authority_change |
        | expired          |
        | revoked          |

    Scenario: Any delegated refusal rolls back content and Gamma together
      Given a published bundle snapshotted before a delegated edit
      And late Gamma validation fails after cryptographic preparation
      When the bundle transaction is reopened
      Then every canonical byte equals the snapshot
      And no failed authorship proof, blob or Gamma entry remains reachable

Feature: Revocation — the full ladder, with no server in any trust role
  Cutting an agent is a ladder, not a switch: expiry and cert revocation cut
  delegated protocol consumption (every honoring verifier refuses), rotation cuts FUTURE reads
  (a fresh key the revoked cannot derive), re-encryption cuts EXISTING
  content, and the cost follows the reach of the revoked key — never the
  number of keys. The revoker is the owner or an authorized ancestor,
  present and holding; survivors do nothing and notice nothing. (spec 06)

  Rule: The founding cut — revoke the step-E agent, nobody else notices

    Scenario: One atomic revocation, three verdicts
      Given two agents granted read on circle folder "projets" and a zone holder
      When the owner revokes the first agent with rotation
      Then the revoked agent reads nothing written after the cut
      And the surviving agent reads new content without lifting a finger
      But the zone holder still reads the folder through the up-link wrap

  Rule: Every revocation is a signed, anchored gamma entry

    Scenario: The revocation entry chains onto the log
      Given an agent granted action rights
      When the owner revokes the agent's mandate
      Then a "revoke" entry signed by the owner chains onto the log
      And the log verifies offline

    Scenario: A revoked chain is refused at verification time
      Given an agent granted action rights
      And the owner revokes the agent's mandate
      When the agent presents its chain after the revocation instant
      Then the chain is rejected as revoked

    Scenario: Revocation is forward-only — the past stays attributable
      Given an agent that acted before being revoked
      Then the action logged before revoked_at still verifies at its own timestamp
      But an action timestamped after revoked_at is rejected

  Rule: Authority to revoke is ancestry, checkable from certificates alone

    Scenario: The issuer revokes its own delegate
      Given an agent with issue depth 1 that delegated to a helper
      When the agent revokes the helper's mandate
      Then the helper's chain is rejected as revoked

    Scenario: A stranger's revocation entry is rejected
      Given two unrelated agents granted action rights
      When the first agent forges a revocation of the second's mandate
      Then the revocation entry is rejected
      And the second agent's chain still verifies

    Scenario: A watchdog cuts actions while holding no key at all
      Given a watchdog granted only the revoke right over circle "projets"
      When the watchdog revokes the projets agent's mandate
      Then the projets agent's chain is rejected as revoked
      But the watchdog itself cannot open a single body

  Rule: Rotation — a fresh key the revoked cannot derive (rung 2)

    Scenario: The new version seals to every survivor and never to the revoked
      Given two agents holding lines on circle folder "projets"
      When the owner revokes the first agent with rotation
      Then the folder's header gains a version without the revoked line
      And the survivor opens the new version with its unchanged keypair

    Scenario: The up-link wrap restores derivation for ancestors
      Given a zone holder reading folder "projets" by pure derivation
      When the owner rotates "projets" out of a revoked agent
      Then the zone holder keeps reading through the up-link wrap
      And the wrap is bound to the node and its new key version

    Scenario: A rotation that smuggles in a new recipient is rejected
      Given a rotated header version for folder "projets"
      When the new version claims a line for a key absent from the old version
      Then header verification is rejected

    Scenario: An up-link wrap authored by a non-holder is rejected
      Given a rotated folder "projets" under the circle zone
      When someone without the parent key posts an up-link wrap
      Then the wrap is rejected

  Rule: Expiry lingers, rotation cleans — and re-encryption erases (rungs 0 and 3)

    Scenario: An expired agent still holds yesterday's key until hygiene passes
      Given an agent whose mandate expired yesterday
      Then the agent's actions are rejected by every verifier
      But its key still opens content written under the old version
      When a manager rotates the node in passing
      Then the old key opens nothing written since

    Scenario: Re-encryption moves existing content beyond the revoked key
      Given an agent that exfiltrated nothing but held folder "projets"
      When the owner revokes it with rotation and re-encryption
      Then the folder's existing bodies are rewritten under the new key
      And the revoked key opens neither the new bodies nor the new lines

  Rule: Cascade and re-adoption — delegation trees fail closed, recover cheap

    Scenario: Revoking a parent cascades to its delegates
      Given an agent with issue depth 1 that delegated to a helper
      When the owner revokes the agent's mandate
      Then the helper's chain is rejected as revoked

    Scenario: The owner re-adopts a cascaded delegate at one line
      Given a helper cut by its parent's revocation
      When the owner grants the helper a fresh mandate on the same folder
      Then the helper reads again with the same keypair

  Rule: Move is a rotation — derivation cannot be un-taught (spec 02.9)

    Scenario: Moving a folder cuts the old parent's derivation
      Given an agent granted read on circle folder "archives"
      And a section "archives/old/note1" the agent reads by derivation
      When the owner moves folder "archives/old" under "projets"
      Then the agent still derives the folder's old key — it cannot be un-taught
      But the agent's read of "projets/old/note1" is rejected as outside its perimeter
      And the folder carries a fresh key version at its new path

    Scenario: A directly granted line survives the move
      Given an agent granted read on circle folder "archives/old"
      When the owner moves folder "archives/old" under "projets"
      Then the agent reads new content at "projets/old" with its unchanged keypair

    Scenario: The new parent's holder reads the moved folder through the up-link wrap
      Given an agent granted read on circle folder "projets"
      When the owner moves folder "archives/old" under "projets"
      Then the agent reads "projets/old/note1" through the wrap posted under "projets"

  Rule: Revocation, cryptographic cut, Gamma and publication are one transaction

    Scenario: A complete incident cut verifies after a cold reopen
      Given a published encrypted subtree shared with one grantee and one survivor
      When an authorized manager revokes the grantee
      And the transaction rotates, rewraps survivors, re-encrypts protected content and appends Gamma
      Then one edition commits the revocation and every derived cryptographic change
      And the revoked line opens no new key or rewritten body
      And a fresh keyless store verifies the authority, cut and resulting roots

    Scenario Outline: Failure during a revocation leaves no partial cut
      Given a published bundle snapshotted byte for byte before revocation
      And an injected failure at "<boundary>"
      When an authorized manager attempts revoke, rotation and publication
      Then the canonical bundle remains byte-for-byte identical to the snapshot
      And reopening observes the old recipients, old Gamma head and old edition
      And no revocation entry or rotated material from the failed attempt is reachable

      Examples:
        | boundary                    |
        | revocation verdict          |
        | fresh node key generation   |
        | survivor rewrap             |
        | body re-encryption           |
        | Gamma append                |
        | before manifest and roots linearization |

  Rule: Revocation replay is forward-only

    Scenario: Historical authority is judged at each entry timestamp
      Given a valid delegated mutation before its mandate revocation
      And an otherwise identical mutation at or after revoked_at
      When a fresh store replays the complete Gamma history
      Then the earlier mutation remains valid
      But the later mutation is rejected
      And current revocation state is derived only from verified prior entries

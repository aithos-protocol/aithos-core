Feature: Integration — one bundle lives the whole protocol (plan §K, spec §09)
  The living end-to-end scenario: a single bundle carries genesis, tree
  building, constrained agentic grants with obligations, budgeted and
  counter-signed actions, owner liveness, attenuated delegation, recursive
  maintenance (a helper cut by its issuer, a folder moved as a rotation, an
  agent revoked with re-encryption), Merkle content proofs, committed gamma
  roots, and a concurrent fork merged deterministically — then a cold
  verifier, holding nothing but the files, replays the accumulated history
  and re-derives every commitment offline. Features A through I prove each
  mechanism in isolation; this feature proves the seams between them, on
  one artifact, in one unbroken timeline — and then that the lived artifact
  still defends itself: every tamper class refused, nothing leaked to a
  keyless reader, an incident contained by a watchdog while the owner is
  away. No server, no network, no trust party anywhere.

  Rule: The full lifecycle holds on one bundle and replays cold

    Scenario: An absentee owner's bundle lives a full agentic life and a cold verifier replays it
      # — genesis and a tree with content in every zone —
      Given a fresh identity
      When I initialise its bundle
      Then edition 1 verifies offline
      When I create circle folder "projets/perso" with a section "note1" tagged "toto"
      And I create circle folder "projets/archive" with a section "old2024" tagged "done"
      And I add a public section "readme" in folder "docs"
      And I add a self folder "sante" with a section "journal"
      And I publish the edition
      Then edition 2 verifies and pins edition 1 as its predecessor

      # — the owner equips its agents, beacons once, then leaves —
      When the owner grants a reader agent read on circle folder "projets/perso" with issue depth 1
      And the owner grants a gmail agent read on "projets/perso" plus gmail send and reply, max_actions 3, counter_sign on send
      And the owner grants a social agent publish requiring human approval within 5 minutes, heartbeat every 7 days grace 3 days
      And the owner beacons at K day 0
      And I publish the edition
      Then the reader agent reads the section under "projets/perso"

      # — budgeted, counter-signed, approved, audited: the agents act —
      When the owner co-signs the prepared send and the gmail agent appends it at K day 1
      Then the action appends with the co_sign receipt recorded in its checks
      When the gmail agent replies with arguments naming recipient "alice@example.com" at K day 2
      Then the entry carries a clear args_hash and a sealed args body
      And the owner reopens the arguments and finds the recipient
      When the gmail agent appends one more reply at K day 2
      Then a fourth gmail action entry is rejected as budget spent
      When the social agent publishes without any receipt
      Then the action is refused as obligation unsatisfied
      And the log gains no entry
      When the approver signs the prepared publish and the social agent appends it at K day 2
      Then the action appends with the receipt recorded in its checks

      # — owner liveness: silence suspends, the beacon resumes —
      When the social agent presents an approved publish at K day 12
      Then the action is refused as heartbeat-stale
      When the owner beacons again at K day 20
      And the approver signs the prepared publish and the social agent appends it at K day 20
      Then the action verifies

      # — recursive maintenance: delegation and cut, no owner in sight —
      When the reader agent delegates read on folder "projets/perso" to a helper
      Then the helper reads "projets/perso/note1" through its delegated line
      When the reader agent revokes the helper's mandate
      Then the helper's chain is rejected as revoked from the cut

      # — move is a rotation: the direct line survives at the new address —
      When the owner moves folder "projets/perso" under "projets/archive"
      Then the moved folder carries a fresh key version at its new address
      And the reader agent reads new content at "projets/archive/perso" with its unchanged keypair

      # — revocation with rotation: the reader survives, the revoked key dies —
      When the owner revokes the gmail agent's mandate with rotation and re-encryption
      Then the revoked gmail key opens neither the new bodies nor the new lines
      And the reader agent reads new content without lifting a finger
      And the gmail agent's actions logged before revoked_at still verify at their own timestamps

      # — commitments: content roots, gamma roots, proofs offline —
      When the owner publishes an edition
      Then the manifest pins a root for public, circle, self and the vault
      And the manifest commits a gamma root and entry count for each segment and a counts root
      When a verifier asks for the moved section's inclusion proof
      Then the proof verifies against the circle root of the signed manifest
      And the moved section proves against the new root through its new address
      When a verifier asks for the social mandate's count proof
      Then the count proof verifies offline against the committed counts root

      # — two copies diverge while the owner is away; either copy merges —
      Given two copies of the lived bundle
      And each copy adds a circle section under its own folder
      When either copy publishes the merge edition
      Then the merge manifest pins the lowest-hash parent and lists both parents ascending
      And both sections are present and the edition verifies
      And the merged log verifies from genesis through the join

      # — the cold replay: files in, every claim re-derived offline —
      Then a cold verifier given only the files accepts the final edition and the full log
      And an independent recomputation from the store yields the same four roots
      And an independent recomputation from the store yields the same gamma roots and counts root
      And every logged action re-verifies against its mandate chain at its own timestamp
      And the revoked chains stay refused in the replay

  Rule: The lived artifact defends itself

    Scenario: After the full life, every tamper class is still refused
      # each attack below lands on a fresh copy of the lived bundle
      Given a bundle that lived the full K walkthrough
      When one byte of a pinned file is altered
      Then edition verification is rejected
      Given a fresh copy of the lived bundle
      When the newest manifest claims a wrong predecessor hash
      Then edition verification is rejected
      Given a fresh copy of the lived bundle
      When one byte of an entry inside the merged segment is altered
      Then log verification is rejected
      Given a fresh copy of the lived bundle
      When a mirror rewrites a clear counter field inside the last log entry
      Then edition verification is refused
      Given a fresh copy of the lived bundle
      When the mirror forges a lived section proof that presents an interior hash as a leaf
      Then the proof is refused
      Given a fresh copy of the lived bundle
      When the mirror claims absence of a mandate id that was counted
      Then the absence proof is refused
      Given a fresh copy of the lived bundle
      When the social agent presents an approval receipt bound to other args
      Then the action is refused as obligation unsatisfied
      Given a fresh copy of the lived bundle
      When the social agent forges a fresh heartbeat with its own key
      Then the forged beacon fails log verification
      Given a fresh copy of the lived bundle
      When the social agent presents a request anchored to a stale head
      Then the request is rejected

    Scenario: The lived bundle shows a stranger nothing and each key only its perimeter
      Given a bundle that lived the full K walkthrough
      When a stranger with no key reads "docs/readme" from public
      Then the section body is readable in clear
      When someone with no key reads the log files
      Then no keyed-zone target, tag or content is revealed
      When I inspect every file of the self zone as a stranger
      Then no folder name, section name, title or tag appears anywhere
      And the sealed reply arguments open for no key but the owner's
      And the revoked gmail key reads nothing written after the cut
      And the gmail key still derives the folder's old key — it cannot be un-taught
      And the section under "projets/archive" stays out of the reader's reach

    Scenario: A watchdog contains an incident while the owner is away
      Given a bundle that lived the full K walkthrough
      And the owner grants a night agent gmail send with a watchdog appointed
      When the night agent acts once
      Then the action verifies
      When the watchdog revokes the night agent's mandate
      Then the night agent's chain is rejected as revoked from its cut
      And the action logged before revoked_at still verifies at its own timestamp
      And the watchdog opens no body anywhere in the bundle

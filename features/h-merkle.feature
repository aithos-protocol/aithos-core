Feature: Merkle state roots — verifiable partial reads (spec 02.10, pass H1)
  Each edition's manifest pins one state root per zone, plus the vault, next
  to gamma_head — ADDED beside the flat file pins (decided 2026-07-11). A
  reader verifies any single row, header, or subtree against the signed
  manifest in O(log n) without fetching an index; a mirror serves such
  proofs without being trusted. Hashing is BLAKE3, domain-separated: a leaf
  can never be spliced as an interior node, nor the reverse. The header is
  folded into its node's hash, so one proof attests row, header version and
  wraps at once. public/circle mirror the folder tree; self and the vault
  stay flat — their proofs reveal sibling hashes only, never structure.
  Honest limit: a proof shows inclusion in a signed edition, never
  freshness.

  Rule: Every edition pins a deterministic root per zone, plus the vault

    @wip
    Scenario: The manifest pins four state roots beside the flat pins
      Given a bundle with content in every zone
      When the owner publishes an edition
      Then the manifest pins a root for public, circle, self and the vault
      And the flat file pins are still present and verify

    @wip
    Scenario: Two verifiers reproduce identical roots from the files alone
      Given a bundle with content in every zone
      When the owner publishes an edition
      Then an independent recomputation from the store yields the same four roots

    @wip
    Scenario: An empty zone pins the empty root
      Given a bundle whose public zone holds nothing
      When the owner publishes an edition
      Then the public root is thirty-two zero bytes

    @wip
    Scenario: An edit bumps only its own zone's root
      Given a published edition with content in every zone
      When the owner adds a circle section and republishes
      Then the circle root changes
      But the public root and the self root are unchanged

  Rule: An inclusion proof verifies a row against the signed root in O(log n)

    @wip
    Scenario: A circle section proves inclusion against the zone root
      Given a published edition with a circle section under a folder
      When a verifier asks for the section's inclusion proof
      Then the proof verifies against the circle root of the signed manifest

    @wip
    Scenario: A tampered index row fails its proof
      Given a published edition with a circle section under a folder
      When the mirror alters the section's title inside the proven row
      Then the proof is refused

    @wip
    Scenario: A grant bumps the granted node's path to the root
      Given a published edition with a circle section under a folder
      When the owner grants an agent the folder and republishes
      Then the old edition's proof for the folder no longer verifies against the new root
      And a fresh proof carries the new header hash and verifies

    @wip
    Scenario: A leaf can never be spliced as an interior node
      Given a published edition with a circle section under a folder
      When the mirror forges a proof that treats a leaf hash as an interior node
      Then the proof is refused

    @wip
    Scenario: An interior node can never pose as a leaf
      Given a published edition with a circle section under a folder
      When the mirror forges a proof that presents an interior hash as a leaf
      Then the proof is refused

    @wip
    Scenario: A self proof reveals sibling hashes only, never structure
      Given a published edition with three self blobs
      When a verifier asks for one self blob's inclusion proof
      Then the proof verifies against the self root
      And the proof carries no name, no path and no sibling row

  Rule: A move is a structural mutation the tree must track (spec 02.9)

    @wip
    Scenario: A moved folder proves at its new address and dies at the old one
      Given a published edition with a circle folder "archives/old" holding a section
      When the owner moves the folder under "projets" and republishes
      Then the section proves against the new root through its new address
      And the old edition's proof for the section no longer verifies against the new root

    @wip
    Scenario: A move diffs as both parents' paths
      Given a published edition with a circle folder "archives/old" holding a section
      When the owner moves the folder under "projets" and republishes
      Then the edition diff descends into both the old and the new parent

  Rule: Two editions diff by root descent

    @wip
    Scenario: A one-section change diffs to exactly its path
      Given a published edition with content in every zone
      When the owner modifies one circle section and republishes
      Then the edition diff descends to exactly that section
      And no other zone appears in the diff

    @wip
    Scenario: Identical editions diff empty
      Given a published edition with content in every zone
      When the owner republishes without any change
      Then the edition diff is empty

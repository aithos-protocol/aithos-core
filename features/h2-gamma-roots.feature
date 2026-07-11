Feature: Committed gamma roots — proofs over the log (spec 07.10, pass H2)
  Each edition's manifest commits, beside gamma_head and additively like the
  content roots: one root and entry count per non-empty monthly segment, and
  a counts trie mapping every counted mandate to its meters — entries,
  actions, children, budgets (actions and tokens per cited profile,
  attested tokens beating declarations). Hashing and proofs reuse the 02.10
  wire byte-for-byte: same domains, left-heavy mroot, v1 node steps.
  Segments hash in chain order — the log's order is the truth, nothing is
  sorted; trie leaves sort by mandate id. Every TOTAL cap becomes one
  O(log n) proof; rolling windows stay segment scans, enumeration-complete
  under root+n. Sorted adjacency proves absence. Withholding breaks the
  counts, forging breaks the roots. Appending is untouched: an author still
  needs only gamma_head, and the appender-side checks keep their raw
  tallies — same bytes, same result.

  Rule: Every edition commits the gamma roots, additively

    @wip
    Scenario: The manifest commits one root and count per non-empty segment plus the counts root
      Given a bundle whose log spans two months of delegated actions
      When the owner publishes an edition
      Then the manifest commits a gamma root and entry count for each of the two segments
      And the manifest commits a gamma counts root
      And the content roots and the flat file pins still verify

    @wip
    Scenario: Two verifiers reproduce identical gamma roots from the files alone
      Given a bundle whose log spans two months of delegated actions
      When the owner publishes an edition
      Then an independent recomputation from the store yields the same gamma roots and counts root

    @wip
    Scenario: An empty log commits no segment roots and the empty counts root
      Given a bundle whose log is empty
      When the owner publishes an edition
      Then the manifest commits no gamma segment roots
      And the gamma counts root is thirty-two zero bytes

    @wip
    Scenario: A tampered segment file is caught by root recomputation alone
      Given a published edition whose log counts a delegated action
      When a mirror rewrites a clear counter field inside the last log entry
      Then edition verification is refused

  Rule: An entry proves inclusion against its segment root

    @wip
    Scenario: An action entry proves inclusion against its segment root
      Given a published edition whose log counts a delegated action
      When a verifier asks for the action entry's inclusion proof
      Then the gamma proof verifies offline against the committed segment root

    @wip
    Scenario: A tampered entry fails its inclusion proof
      Given a published edition whose log counts a delegated action
      When the mirror alters the entry's action name inside the proven bytes
      Then the gamma proof is refused

  Rule: The counts trie makes every total cap provable

    @wip
    Scenario: A mandate's count leaf proves its meters in one proof
      Given a published edition whose log counts actions, a sub-grant and budget tokens under a mandate
      When a verifier asks for the mandate's count proof
      Then the count proof verifies offline against the committed counts root
      And the proven counters equal the raw tallies of the chain

    @wip
    Scenario: A descendant's action counts into every ancestor's leaf
      Given a published edition whose log holds an action by a sub-delegate
      When a verifier proves the counts of the root mandate and of the leaf mandate
      Then both count leaves carry that action

    @wip
    Scenario: A mandate never counted proves absent by adjacency
      Given a published edition whose log counts two mandates apart in id order
      When a verifier asks whether an id between them was ever counted
      Then the mirror proves absence with two adjacent leaves bracketing the id

    @wip
    Scenario: A forged absence over a counted mandate is refused
      Given a published edition whose log counts three mandates
      When the mirror claims absence of the middle one with the outer two leaves
      Then the absence proof is refused

  Rule: Completeness closes the withhold

    @wip
    Scenario: Withholding one action from a mandate-filtered answer is detected
      Given a published edition whose log counts three actions under a mandate
      When a mirror answers the mandate's action query with only two proven entries
      Then the answer is refused against the proven count of three

    @wip
    Scenario: A segment enumeration cannot omit an entry
      Given a published edition whose log spans one month
      When a mirror serves the segment with one entry withheld
      Then the recomputed segment root dies against the committed root and count

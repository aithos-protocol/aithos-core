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
  tallies. Historical bytes stay identical; no mutation/total counter wire
  is implied before the CB2 vectors.

  Rule: Every edition commits the gamma roots, additively

    Scenario: The manifest commits one root and count per non-empty segment plus the counts root
      Given a bundle whose log spans two months of delegated actions
      When the owner publishes an edition
      Then the manifest commits a gamma root and entry count for each of the two segments
      And the manifest commits a gamma counts root
      And the content roots and the flat file pins still verify

    Scenario: Two verifiers reproduce identical gamma roots from the files alone
      Given a bundle whose log spans two months of delegated actions
      When the owner publishes an edition
      Then an independent recomputation from the store yields the same gamma roots and counts root

    Scenario: An empty log commits no segment roots and the empty counts root
      Given a bundle whose log is empty
      When the owner publishes an edition
      Then the manifest commits no gamma segment roots
      And the gamma counts root is thirty-two zero bytes

    Scenario: A tampered segment file is caught by root recomputation alone
      Given a published edition whose log counts a delegated action
      When a mirror rewrites a clear counter field inside the last log entry
      Then edition verification is refused

  Rule: An entry proves inclusion against its segment root

    Scenario: An action entry proves inclusion against its segment root
      Given a published edition whose log counts a delegated action
      When a verifier asks for the action entry's inclusion proof
      Then the gamma proof verifies offline against the committed segment root

    Scenario: A tampered entry fails its inclusion proof
      Given a published edition whose log counts a delegated action
      When the mirror alters the entry's action name inside the proven bytes
      Then the gamma proof is refused

  Rule: The counts trie makes every total cap provable

    Scenario: A mandate's count leaf proves its meters in one proof
      Given a published edition whose log counts actions, a sub-grant and budget tokens under a mandate
      When a verifier asks for the mandate's count proof
      Then the count proof verifies offline against the committed counts root
      And the proven counters equal the raw tallies of the chain

    Scenario: A descendant's action counts into every ancestor's leaf
      Given a published edition whose log holds an action by a sub-delegate
      When a verifier proves the counts of the root mandate and of the leaf mandate
      Then both count leaves carry that action

    Scenario: A mandate never counted proves absent by adjacency
      Given a published edition whose log counts two mandates apart in id order
      When a verifier asks whether an id between them was ever counted
      Then the mirror proves absence with two adjacent leaves bracketing the id

    Scenario: A forged absence over a counted mandate is refused
      Given a published edition whose log counts three mandates
      When the mirror claims absence of the middle one with the outer two leaves
      Then the absence proof is refused

  Rule: Completeness closes the withhold

    Scenario: Withholding one action from a mandate-filtered answer is detected
      Given a published edition whose log counts three actions under a mandate
      When a mirror answers the mandate's action query with only two proven entries
      Then the answer is refused against the proven count of three

    Scenario: A segment enumeration cannot omit an entry
      Given a published edition whose log spans one month
      When a mirror serves the segment with one entry withheld
      Then the recomputed segment root dies against the committed root and count

  Rule: Counter domains stay distinct under the versioned delegated-counts trie

    @wip
    Scenario: D7-CB2 fixes the two signed limits and separate counter root
      Given a homogeneous draft3 mandate carrying max_mutations and max_consumptions
      When its accepted occurrences are committed for cold replay
      Then max_mutations counts only delegated Ethos mutation occurrences
      And max_consumptions counts every delegated canonical occurrence once
      And delegated_counts has exactly aithos-delegated-counts-core and root
      And its leaves have only non-zero mutations and consumptions
      But historical gamma_counts_root and entries bytes are unchanged

    @wip
    Scenario Outline: Each delegated consumption affects only its conceptual meters
      Given a mandate history containing one "<consumption>"
      When the verifier rebuilds action, Ethos-mutation and total-consumption tallies
      Then the action tally changes by "<action delta>"
      And the mutation tally changes by "<mutation delta>"
      And the total delegated-consumption tally changes by "<total delta>"

      Examples:
        | consumption                | action delta | mutation delta | total delta |
        | connector action           | 1            | 0              | 1           |
        | metered inference          | 0            | 0              | 1           |
        | delegated Ethos mutation   | 0            | 1              | 1           |
        | journalized delegated read | 0            | 0              | 1           |
        | delegated config mutation  | 0            | 0              | 1           |
        | direct sub-grant           | 0            | 0              | 1           |
        | scoped revocation          | 0            | 0              | 1           |
        | normal grantee publication | 0            | 0              | 1           |
        | merge publication plus its kind:merge entry | 0          | 0              | 1           |
        | delegated fork resolution  | 0            | 0              | 1           |
        | owner Ethos mutation       | 0            | 0              | 0           |

    @wip
    Scenario: New delegated counters do not rewrite historical committed bytes
      Given a historical edition and Gamma vector predating mutation and total meters
      When a verifier replays it under its historical protocol version
      Then the historical edition remains byte-identical and verifiable
      And new meter material is accepted only under the delegated-counts profile
      And old Gamma kinds, max_actions and count roots are never reinterpreted
      And new meter material under an old or unversioned schema, or under an unknown counter-schema version, fails closed

    @wip
    Scenario: Invalid delegated counter evidence has one typed refusal
      Given delegated-counts material with an invalid shape, proof, tally or occurrence correlation
      When Core validates it at append time or during cold replay
      Then it is refused as InvalidDelegatedCounts
      And a malformed max_mutations or max_consumptions certificate is refused as InvalidMandate

    @wip
    Scenario Outline: Publication authority counts once across all of its evidence
      Given a grantee "<operation>" contains two semantically distinct already-counted mutations
      And its publisher authority is evidenced by "<edition evidence>"
      And the same publisher decision has "<Gamma evidence>"
      When semantic replay rebuilds the total delegated-consumption tally
      Then the two mutations and the publication contribute exactly three
      And the edition and Gamma evidence correlate to the same single publisher unit
      And any Gamma evidence and edition reference for the same contained mutation count it once
      And no manifest, root or derived write-set consequence adds another consumption
      And the closed Gamma kind registry gains no implicit publication entry

      Examples:
        | operation          | edition evidence                    | Gamma evidence                         |
        | normal publication | signed manifest and changeset       | no Gamma publication entry             |
        | disjoint merge     | signed merge manifest and changeset | the existing kind:merge entry           |
        | fork resolution    | signed resolving manifest           | no distinct Gamma resolution entry       |

  Rule: Roots prove committed bytes but never authorize them

    @wip
    Scenario: A valid root over an unauthorized mutation is still refused
      Given an edition whose Gamma roots and inclusion proofs recompute exactly
      But one proven mutation is outside its actor's SID perimeter
      When the fresh-store verifier performs semantic replay
      Then the edition is rejected despite the valid roots

    @wip
    Scenario: Append-time and cold-time rebuild identical semantic counts
      Given one accepted mixed history of reads, actions, inferences, mutations, config mutations, grants, revocations, publications and merges
      When counters are computed before the next append and from a fresh-store replay
      Then every conceptual tally and limit verdict is identical
      And the roots commit that replay state without replacing semantic checks

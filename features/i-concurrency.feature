Feature: Concurrency — disjoint merge, fork, resolution (spec 02.6 + 07.6, pass I)
  Editions form a linear chain until two authors sign competing heights.
  Disjoint changesets (root-descent diffs against the common ancestor whose
  node sets do not intersect) are NOT a conflict: any party whose owner
  capability or single grantee chain covers every derived change publishes
  the merge edition — prev_hash pins the parent with the lowest edition
  hash, merges lists both ascending, additive wire — and every merger
  produces byte-identical results. Shared index files merge 3-way by sid: changed
  rows from their branch, added rows unioned, deletions hold, the same sid
  changed on both sides is a fork. The log re-joins at a signed merge entry
  (prev = min parent's tip, additive prevs = both tips, the only
  two-predecessor kind); the merged segment lays out sub-chain A then B
  then the merge entry, entries byte-identical, at-monotonicity relaxed at
  the join, gamma roots recommitted. A fork proper is refused by every
  verifier until the nearest common manager — a delegate only inside its
  own authority, the owner as last resort — signs the resolving edition
  (resolves_fork), whose content extends the winning branch.

  Rule: Disjoint editions merge deterministically, arbiter-free

    Scenario: Two disjoint writes merge into one edition that verifies
      Given two copies of a published bundle
      And each copy adds a circle section under a different folder
      When either party publishes the merge edition
      Then the merge manifest pins the lowest-hash parent and lists both parents ascending
      And both sections are present and the edition verifies

    Scenario: Two mergers produce byte-identical merge manifests
      Given two copies of a published bundle
      And each copy adds a circle section under a different folder
      When each party computes the merge edition independently
      Then the two merged manifests hash identically

    Scenario: Two adds in the same folder merge three-way by sid
      Given two copies of a published bundle
      And each copy adds a differently-named section under the same folder
      When either party publishes the merge edition
      Then the folder's index carries both rows in sid order
      And the edition verifies

    Scenario: A deletion does not resurrect through the merge
      Given two copies of a published bundle holding a circle section
      And one copy deletes that section while the other adds a sibling
      When either party publishes the merge edition
      Then the deleted section stays absent from the merged index
      And the sibling is present

    Scenario: The same section modified on both branches is a fork, not a merge
      Given two copies of a published bundle holding a circle section
      And each copy modifies that same section differently
      When a party attempts the merge edition
      Then the merge is refused as a same-node conflict

  Rule: The merged log re-joins at a signed merge entry

    Scenario: The merge entry carries both tips and the chain verifies through the join
      Given two copies of a published bundle whose agents each logged an action
      When either party publishes the merge edition
      Then the merge entry cites both sub-chain tips in prevs
      And the merged log verifies from genesis through the join

    Scenario: The merged segment recommits its root and count over the deterministic layout
      Given two copies of a published bundle whose agents each logged an action
      When either party publishes the merge edition
      Then the merged segment lays out the lowest-hash parent's entries first
      And the manifest's gamma segment root and count match an independent recomputation

    Scenario: Budgets tally across both sub-chains after the merge
      Given two copies of a published bundle whose agent may act three times in total
      And each copy logs two actions under that mandate
      When either party publishes the merge edition
      Then a fifth action after the merge is refused as budget spent

  Rule: A fork is refused until the nearest common manager resolves it

    Scenario: An unresolved same-node fork is refused by the verifier
      Given two competing editions modifying the same section
      When a verifier is shown both branches
      Then neither branch is canonical and the conflict is surfaced

    Scenario: The nearest common manager resolves the fork
      Given two competing editions modifying the same section under a delegate's folder
      When the covering delegate publishes the resolving edition naming the winner
      Then the resolving edition verifies and extends the winning branch
      And the losing branch's write is surfaced, not replayed

    Scenario: A delegate cannot resolve a fork outside its perimeter
      Given two competing editions touching a folder outside the delegate's grant
      When the delegate attempts the resolving edition
      Then the resolution is refused for lack of authority

    Scenario: The owner resolves as last resort
      Given two competing editions modifying the same section
      When the owner publishes the resolving edition naming the winner
      Then the resolving edition verifies and extends the winning branch

  Rule: Local merge publication still has one fully covering actor

    @wip
    Scenario Outline: A merge author must cover every derived change
      Given two local branches with disjoint changes
      And the publishing actor has "<authority>"
      When that actor attempts the deterministic merge edition
      Then publication is "<verdict>"
      And an accepted grantee merge uses one actor and one mandate chain

      Examples:
        | authority                              | verdict  |
        | one chain covering both changed nodes  | accepted |
        | one chain covering only the first node | refused  |
        | two separate partial chains             | refused  |
        | owner local capability                  | accepted |

    @wip
    Scenario: A local merge never simulates provider CAS
      Given two exported local bundle branches with the same parent
      When an authorized actor merges them into a fresh local store
      Then conflict and authority are decided entirely by Core and Bundle
      And no HTTP, provider backend, remote store or server CAS participates

  Rule: Refused forks and accepted merges preserve replay state

    @wip
    Scenario: A refused resolution changes no local canonical byte
      Given a forked local bundle snapshotted before resolution
      When a grantee outside one touched perimeter attempts to resolve it
      Then the resolution is refused
      And the manifest, roots, Gamma tips and branch artifacts remain byte-for-byte unchanged

    @wip
    Scenario: A fresh store recomposes all counters across an accepted merge
      Given two disjoint branches carrying delegated actions, mutations and grants
      When one authorized actor publishes their deterministic local merge
      Then fresh-store replay rebuilds the same action, mutation, total and direct-child tallies
      And no branch consumption is omitted or counted twice

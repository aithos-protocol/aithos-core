Feature: Section-precise mandates — the id= selector
  spec 04.2 grants one section: read.self#id=<sid> and the verb
  variants. A sid is global — id composes with nothing: not with dir,
  not with tag. Pure containment cannot resolve a section's folder,
  so a dir= parent never covers an id= child (M0 reco, 2026-07-16):
  only the identical id= or the whole-zone entry does. The wire stays
  additive — a mandate without id serializes byte-for-byte as today
  (conformance vector, frozen before code). A section grant delivers
  exactly one header line: the section's own node.

  Rule: id= grants exactly one section — certificate AND key

    @wip
    Scenario: A section grant reads that section and nothing else
      Given a published bundle with circle sections "note1" and "note2" in folder "projets"
      When the owner grants the agent read on section "note1" by id
      Then the agent reads "note1" with its own keypair
      But "note2" stays out of the agent's reach

    @wip
    Scenario: A self section opens by id without opening the zone
      Given self sections "consignes" and "marges"
      When the owner grants the agent read on self section "consignes" by id
      Then the agent reads "consignes" with its own keypair
      But "marges" stays out of the agent's reach

    @wip
    Scenario: An id= grant carries the write verbs too
      Given a published bundle with circle section "brouillon" in folder "projets"
      When the owner grants the agent edit on section "brouillon" by id
      Then the agent rewrites "brouillon" with its own keypair
      But the agent cannot create a sibling section in "projets"

    @wip
    Scenario: A self write is enforceable by id — the sealed structure stays sealed
      Given self sections "consignes" and "marges"
      When the owner grants the agent edit on self section "consignes" by id
      Then the agent rewrites "consignes" with its own keypair
      But "marges" stays out of the agent's reach

  Rule: id composes with nothing

    Scenario: A perimeter entry mixing id with dir or tag is rejected
      When a mandate carries a perimeter entry combining id= with dir= or tag=
      Then the mandate is rejected at parse

    Scenario: A dir= parent does not cover an id= child
      Given an agent granted read on circle folder "projets" with issue depth 1
      When the agent delegates read on a section of "projets" by id
      Then the helper's chain is rejected

    @wip
    Scenario: A whole-zone parent covers an id= child
      Given an agent granted read on circle with issue depth 1
      When the agent delegates read on circle section "note1" by id
      Then the helper's chain verifies
      And the helper reads "note1" but nothing else

    Scenario: An id= parent covers the identical id= and nothing wider
      Given an agent granted read on circle section "note1" by id with issue depth 1
      When the agent delegates read on section "note1" by id
      Then the helper's chain verifies
      But delegating section "note2" by id is rejected
      And delegating the whole folder of "note1" is rejected

  Rule: The op carries the section — covers_op confronts it

    Scenario: A read op outside the granted section is not covered
      Given an agent granted read on section "note1" by id
      When the agent attempts a read op on section "note2"
      Then the op is not covered

    Scenario Outline: An exact id operation stays exact in every zone
      Given an agent granted "<verb>" on "<zone>" section "note1" by id
      When the agent attempts "<operation>" on the same SID
      Then the operation is covered
      But the identical operation on sibling SID "note2" is not covered

      Examples:
        | zone   | verb   | operation |
        | public | read   | read      |
        | public | edit   | edit      |
        | public | delete | delete    |
        | circle | read   | read      |
        | circle | edit   | edit      |
        | circle | delete | delete    |
        | self   | read   | read      |
        | self   | edit   | edit      |
        | self   | delete | delete    |

    Scenario Outline: A dir or tag parent never covers an id child
      Given an agent granted "<selector>" on a zone with issue depth 1
      When the agent delegates the apparently related section by id
      Then the helper's chain is rejected without resolving the SID position

      Examples:
        | selector                         |
        | read.public#dir=projects         |
        | read.circle#dir=projects         |
        | read.self#dir=sealed             |
        | read.public#tag=toto             |
        | read.circle#tag=toto             |
        | read.self#tag=private            |
        | read.circle#dir=projects&tag=toto |
        | read.self#dir=sealed&tag=private |

    Scenario Outline: A whole-zone parent covers an id child in every zone
      Given an agent granted read on all of "<zone>" with issue depth 1
      When the agent delegates one section of that zone by id
      Then the helper's chain verifies
      And no other section is covered by the child

      Examples:
        | zone   |
        | public |
        | circle |
        | self   |

    Scenario Outline: A duplicated selector dimension is invalid form
      When a mandate carries one perimeter entry with "<duplicate_selector>"
      Then the mandate is rejected before signature verification

      Examples:
        | duplicate_selector   |
        | dir=a&dir=b          |
        | tag=a&tag=b          |
        | id=one&id=two        |

  Rule: A self creation is zone-wide or bound to a preallocated opaque SID

    @wip
    Scenario Outline: Self create authority reveals no structure
      Given an agent granted "<authority>" in self
      When the agent creates an opaque self section with "<candidate SID>"
      Then the create verdict is "<verdict>"
      And its proof reveals no name, path, title, tags, body or folder relation

      Examples:
        | authority                    | candidate SID          | verdict |
        | append.self                  | fresh opaque SID       | allowed |
        | write.self                   | fresh opaque SID       | allowed |
        | append.self#id=preallocated  | preallocated SID       | allowed |
        | append.self#id=preallocated  | different fresh SID    | refused |
        | append.self#dir=sealed       | SID apparently below it | refused |

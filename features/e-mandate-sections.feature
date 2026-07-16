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

    @wip
    Scenario: A perimeter entry mixing id with dir or tag is rejected
      When a mandate carries a perimeter entry combining id= with dir= or tag=
      Then the mandate is rejected at parse

    @wip
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

    @wip
    Scenario: An id= parent covers the identical id= and nothing wider
      Given an agent granted read on circle section "note1" by id with issue depth 1
      When the agent delegates read on section "note1" by id
      Then the helper's chain verifies
      But delegating section "note2" by id is rejected
      And delegating the whole folder of "note1" is rejected

  Rule: The op carries the section — covers_op confronts it

    @wip
    Scenario: A read op outside the granted section is not covered
      Given an agent granted read on section "note1" by id
      When the agent attempts a read op on section "note2"
      Then the op is not covered

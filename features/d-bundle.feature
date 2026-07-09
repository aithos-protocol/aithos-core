Feature: Bundle and editions
  The bundle is the subject's entire state as files: indexes, sealed blobs,
  headers, DID document, and a signed manifest. Editions form a linear,
  hash-pinned chain; every check reads files — a server is never a trust
  party. (spec 02.3, 02.6)

  Rule: Editions chain and verify offline

    @wip
    Scenario: Initialising a bundle publishes a verifiable first edition
      Given a fresh identity
      When I initialise its bundle
      Then edition 1 verifies offline
      And the manifest pins the DID document

    @wip
    Scenario: Every publication extends the chain
      Given an initialised bundle
      When I create circle folder "projets/perso" with a section "note1" tagged "toto"
      And I publish the edition
      Then edition 2 verifies and pins edition 1 as its predecessor

    @wip
    Scenario: A tampered file fails the edition
      Given a published bundle
      When one byte of a pinned file is altered
      Then edition verification is rejected

    @wip
    Scenario: A broken chain fails closed
      Given a bundle with two editions
      When the newest manifest claims a wrong predecessor hash
      Then edition verification is rejected

  Rule: Content round-trips through the sealed store

    @wip
    Scenario: The owner reads back what was written
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner reads "projets/perso/note1" from circle
      Then the section body comes back intact

    @wip
    Scenario: Display paths resolve through names, keys through sids
      Given a published bundle with section "note1" in circle "projets/perso"
      When the folder "perso" is renamed to "intime"
      And the edition is republished
      Then the owner reads the same section at "projets/intime/note1"

  Rule: The self zone leaks no structure

    @wip
    Scenario: Self is a flat sea of opaque blobs
      Given a bundle with a self folder "enfance/cicatrices" containing section "1234"
      When I inspect every file of the self zone as a stranger
      Then no folder name, section name, title or tag appears anywhere
      And the owner still reconstructs the full tree from sealed descriptors

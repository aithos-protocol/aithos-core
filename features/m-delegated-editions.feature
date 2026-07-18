Feature: Normal delegated editions
  A normal edition has one actor, one authority context and a derived changeset.
  Public and opaque artifacts suffice for cold verification without an owner fallback.

  Rule: Edition v1 has exactly one actor and at most one mandate chain

    @wip
    Scenario Outline: A normal edition is signed in the actor's own capacity
      Given a candidate normal edition by "<actor>"
      And every derived change is covered by "<authority>"
      When Bundle validates the candidate against its expected parent
      Then publication is "<verdict>"
      And no actor is represented as another actor

      Examples:
        | actor         | authority                                | verdict  |
        | owner         | narrow local owner capability            | accepted |
        | leaf grantee  | one valid chain covering every change    | accepted |
        | leaf grantee  | two partial chains covering different changes | refused |
        | leaf grantee  | a valid chain plus no key proof           | refused  |

    @wip
    Scenario: The owner is absent from an ordinary grantee edition
      Given a grantee has one chain covering every candidate change
      And no applicable obligation requires owner approval
      When the grantee publishes the normal edition
      Then the grantee alone signs as actor
      And no owner signature, key or online participation is required

    @wip
    Scenario: Explicit co_sign attests without changing the edition actor
      Given a grantee publication explicitly requires an owner co_sign obligation
      When the owner provides a fresh bound approval receipt
      Then the grantee remains the sole actor and edition signer
      And the owner appears only as the receipt attestor

  Rule: The changeset is derived and every byte transition is explained

    @wip
    Scenario Outline: A caller cannot omit or invent a change
      Given a parent edition and a candidate state with "<defect>"
      When Bundle derives the typed changeset by comparing both states
      Then the edition is refused
      And no caller-asserted changeset can override the derived result

      Examples:
        | defect                                      |
        | a changed blob omitted from the claim       |
        | a deleted index row omitted from the claim  |
        | a claimed change absent from candidate state |
        | a Gamma entry unrelated to any state change |
        | a changed node outside the one actor chain  |

    @wip
    Scenario: Every delegated change is joined to operation, Gamma and authority
      Given one grantee candidate changes content, an index row and its derived root path
      When the candidate is validated
      Then the content operation is covered by the leaf chain
      And Gamma explains the authored change
      And deterministic index and root updates are recognized as consequences
      And any unexplained parasite change is refused

  Rule: Zone-specific proof survives a fresh-store cold replay

    @wip
    Scenario: Public delegated authorship travels with the edition
      Given a grantee publishes a public content mutation
      Then its signature binds content hash, SID, operation, edition and authorized_via
      And Gamma and the manifest commit that proof
      When the edition is reopened without private capabilities
      Then the verifier distinguishes grantee authorship from owner authorship

    @wip
    Scenario: Self delegated changes reveal opaque state relations only
      Given a grantee publishes an authorized self mutation by exact SID
      When a keyless verifier checks the parent and candidate editions
      Then it proves inclusion, replacement or absence for the same opaque SID
      But it learns no name, path, title, tags, content, folder relation or key

    @wip
    Scenario Outline: A fresh local store rejects incomplete delegated evidence
      Given a grantee edition exported into a fresh empty "<store>" store
      And all private capabilities are absent
      When "<defect>" is present
      Then cold verification is refused

      Examples:
        | store    | defect                              |
        | MemStore | leaf certificate is missing         |
        | MemStore | public authorship proof is missing  |
        | FsStore  | expected parent is wrong             |
        | FsStore  | Gamma delta is truncated             |

  Rule: Bundle is the sole local keyless assembly façade

    @wip
    Scenario: Layout verification feeds one pure Core semantic verdict
      Given a complete exported delegated edition
      When Bundle checks layout, version, hashes, references and reachability
      Then it supplies typed public artifacts to one pure Core verifier
      And no public helper returns Allow from layout, link or hash checks alone

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
    Scenario Outline: Manifest profiles fix the K1-B carrier presence
      Given a candidate manifest under "<profile>"
      And its K1-B carrier state is "<carrier state>"
      When Bundle validates signed manifest form before semantic replay
      Then the manifest is "<verdict>"

      Examples:
        | profile | carrier state                                        | verdict  |
        | draft.1 | operation_ref, changeset_ref and evidence_ref absent | accepted |
        | draft.1 | any K1-B carrier present                             | refused  |
        | draft.2 | all three exact top-level carriers present           | accepted |
        | draft.2 | operation_ref missing or null                        | refused  |
        | draft.2 | changeset_ref missing or null                        | refused  |
        | draft.2 | evidence_ref missing or null                         | refused  |
        | unknown | all three carriers present                           | refused  |

    @wip
    Scenario Outline: Draft2 carrier references have one digest and one canonical sidecar key
      Given a complete derived "<carrier>" document D
      When Bundle addresses and pins D for a draft2 manifest
      Then its reference has exactly "<profile member>" and digest
      And digest is domain-separated SHA-256 of "<domain>", NUL and RFC8785-JCS of D
      And its Store key is "<directory>/<digest suffix>.json"
      And files pins those exact JCS bytes with the historical bare SHA-256

      Examples:
        | carrier   | profile member         | domain                     | directory  |
        | changeset | aithos-changeset-core  | aithos-core/v1/changeset    | changesets |
        | evidence  | aithos-evidence-core   | aithos-core/v1/evidence     | evidence   |

    @wip
    Scenario: A derived changeset has one closed commitment-only table
      Given parent and candidate states with contained operation occurrences
      When Bundle derives their K1-C changeset
      Then it has exactly aithos-changeset-core, height, predecessors, operations and changes
      And height and predecessors equal the publication facts
      And operations equal contained_operations in causal order without the publication occurrence
      And every change has exactly key_commitment, before, after and operation_ref
      And absent state has only state while present state adds byte_commitment
      And every change names one contained operation and before differs from after
      And changes sort by key commitment then occurrence with no duplicate key

    @wip
    Scenario: Carrier objects are acyclic consequences rather than changeset rows
      Given a complete K1-C changeset and evidence set for one candidate manifest
      When Bundle checks every changed canonical Store object
      Then the changeset explains content, index, root, header, wrap, Gamma, vault and rotation consequences
      But it excludes its own sidecar, the evidence sidecar and the candidate manifest
      And the manifest references and files pins explain those three carrier objects
      And no carrier digest depends transitively on the candidate manifest

    @wip
    Scenario: The publication reference and changeset are acyclic
      Given a draft2 candidate with contained operation occurrences
      When Bundle derives its closed changeset and publication operation
      Then the changeset carries the contained operation references in causal order
      But excludes the publication operation_ref and candidate manifest hash
      And publication facts commit the completed changeset
      And every verifier reconstructs the same dependency direction

    @wip
    Scenario: The evidence carrier proves but never authorizes
      Given a complete draft2 evidence set for delegated occurrences
      When a fresh-store verifier replays authorship, session, receipts and catalog evidence
      Then every item is correlated through its exact operation_ref
      And authority is still derived only from owner capability or the mandate chain
      And no private content, credential, DK, private key or protected plaintext is present

    @wip
    Scenario Outline: Every evidence item selects one exact nested proof table
      Given a K1-C evidence item of kind "<kind>"
      When Core validates the selected item
      Then its exact members are "<members>"
      And the nested documents validate under their own profile
      And an unused, duplicate, uncorrelated or authority-bearing item is refused

      Examples:
        | kind         | members                |
        | authorship   | kind,document          |
        | session      | kind,certificate,proof |
        | receipt      | kind,document          |
        | catalog      | kind,catalog,approval  |
        | presentation | kind,document          |

    @wip
    Scenario: The evidence set is closed, sorted and carries D7 without granting authority
      Given all public proof material needed by the contained operations
      When Bundle constructs the K1-C evidence set
      Then it has exactly aithos-evidence-core, items and delegated_counts
      And items sort by complete RFC8785-JCS bytes with no duplicate
      And delegated_counts is always the exact D7 reference, including the empty root
      And every required proof appears once while unrelated proof is refused
      And authority is still derived only from owner capability or one mandate chain

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
    Scenario: Public grantee authorship has one acyclic signed table
      Given a grantee publishes a public section mutation
      When its K1-C authorship document is encoded
      Then it has exactly aithos-authorship-core, subject, zone, sid, content_hash, operation_ref, edition, authorized_via, key and sig
      And zone is public and content_hash covers the exact stored public body bytes
      And edition has exactly height and predecessors matching publication facts
      And authorized_via and key equal the reconstructed W1 authority
      And the grantee key signs RFC8785-JCS with top-level sig omitted
      And no candidate manifest or carrier digest enters the signature

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
    Scenario: An opposable Gamma presentation has one signed result table
      Given a canonical read.gamma query whose result is made opposable
      When its K1-C presentation is encoded
      Then it has exactly aithos-gamma-presentation-core, subject, operation_ref, source_head, request_digest, entries, at, key and sig
      And entries are the complete selected Gamma objects in verified order without duplicate id
      And Bundle re-executes the query against source_head and obtains those exact entries
      And the verified presenter key signs RFC8785-JCS with top-level sig omitted
      And no Gamma entry, Gamma kind or second occurrence is created

    @wip
    Scenario Outline: K1-C carrier defects fail closed before publication
      Given a draft2 candidate with "<defect>"
      When Bundle validates carriers and asks Core for one semantic verdict
      Then publication is refused
      And no candidate manifest, carrier sidecar or Gamma delta becomes reachable

      Examples:
        | defect                                      |
        | malformed or mismatched carrier reference  |
        | sidecar key or files pin mismatch           |
        | unsorted or duplicate changes               |
        | omitted or invented Store consequence       |
        | operation absent from contained operations  |
        | unsorted or duplicate evidence item         |
        | authorship signed by a different actor      |
        | presentation result different from query    |
        | evidence item presented as authority        |
        | private key or protected plaintext in evidence |

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

Feature: Connector classes and isolated vault capabilities
  Connector business actions use an owner-approved signed catalog.
  Each connector vault is isolated and needs both exact authority and an exact key line.

  Rule: An approved catalog is the sole source of action class

    @wip
    Scenario: Catalog signer and owner approval are distinct proofs
      Given a signed, versioned and content-addressed connector catalog
      When the owner approves its exact digest and version
      Then a mandate and edition pin both catalog and approval evidence
      And a keyless verifier never treats catalog signature alone as owner approval

    @wip
    Scenario Outline: Every catalog action has exactly one canonical class
      Given a signed connector catalog whose action has "<class assignment>"
      When the owner and a keyless verifier validate its form
      Then the catalog is "<verdict>"

      Examples:
        | class assignment          | verdict  |
        | exactly one read class    | accepted |
        | exactly one act class     | accepted |
        | exactly one binding class | accepted |
        | no class                  | refused  |
        | two classes               | refused  |
        | one class outside registry | refused  |
        | duplicate identical class  | refused  |

    @wip
    Scenario Outline: Wildcard and exact action rights follow the pinned class
      Given an approved catalog classes action "<action>" as "<class>"
      And a mandate carries "<authority>"
      And the request carries "<receipt>"
      When the grantee attempts that exact action
      Then the verdict is "<verdict>"
      And no runtime component may reclassify it

      Examples:
        | action   | class   | authority           | receipt             | verdict  |
        | list     | read    | act.x.mail.*        | none required       | accepted |
        | send     | act     | act.x.mail.*        | none required       | accepted |
        | purchase | binding | act.x.mail.*        | valid owner co_sign | refused  |
        | purchase | binding | act.x.mail.purchase | no owner co_sign    | refused  |
        | purchase | binding | act.x.mail.purchase | valid owner co_sign | accepted |

    @wip
    Scenario Outline: Catalog drift never widens an existing mandate
      Given a mandate pins one approved connector catalog
      When runtime presents "<catalog change>"
      Then the action is refused until new owner-approved authority is issued

      Examples:
        | catalog change                         |
        | an action added after issuance         |
        | an act action reclassified as binding  |
        | another manifest with the same version |
        | an unapproved newer version            |

    @wip
    Scenario: Legacy migration never grants binding authority
      Given an explicitly versioned legacy connector migration
      When legacy read and write rights are projected
      Then read may map to read and write may map only to act
      And no legacy right proves a binding action
      And canonical rights require re-enrolment

    @wip
    Scenario Outline: Delegation and execution use the same pinned catalog class
      Given a parent mandate carries "<parent authority>" under one approved catalog
      When it delegates "<child authority>" under "<child catalog>"
      Then the child chain is "<verdict>"

      Examples:
        | parent authority   | child authority      | child catalog         | verdict  |
        | act.x.mail.*       | act.x.mail.list      | identical pinned one  | accepted |
        | act.x.mail.*       | act.x.mail.send      | identical pinned one  | accepted |
        | act.x.mail.*       | act.x.mail.purchase  | identical pinned one  | refused  |
        | act.x.mail.purchase | act.x.mail.purchase | identical pinned one  | accepted with inherited co_sign |
        | act.x.mail.*       | act.x.mail.list      | different catalog     | refused  |

  Rule: Config is a reserved exact capability outside business classes

    @wip
    Scenario: Config inherits no silent binding classification or co_sign
      Given the validated G-A classification
      When a mandate carries exact act.x.mail.config
      Then config remains outside the read, act and binding business catalog
      And all applicable constraints and obligations explicitly present in the whole presented chain apply
      And no wildcard or inferred binding co_sign covers it

    @wip
    Scenario Outline: Exact config authority covers CRUD for one connector only
      Given a grantee has exact act.x.mail.config and the exact /x/mail line
      When it performs config "<operation>" for mail
      Then the vault operation is authorized under its applicable constraints
      And Gamma, roots and publication commit any mutation atomically
      And config authority grants no external mail action
      And this protocol version exposes no narrower config read or write authority
      And a finer split requires a later version and never reinterprets this mandate

      Examples:
        | operation |
        | read      |
        | create    |
        | edit      |
        | delete    |

  Rule: Every connector has an independent vault node

    @wip
    Scenario Outline: Vault access requires exact authority and exact line together
      Given a grantee presents "<authority>" and holds "<line>"
      When it attempts to open mail config at /x/mail
      Then the result is "<verdict>"

      Examples:
        | authority                 | line                 | verdict                   |
        | act.x.mail.config         | exact /x/mail line   | authorized and readable   |
        | act.x.mail.config         | no vault line        | authorized but unreadable |
        | act.x.mail.config         | generic /x root line | unreadable                |
        | act.x.mail.config         | /x/calendar line     | unreadable                |
        | no config authority       | exact /x/mail line   | refused as unauthorized   |
        | act.x.mail.*              | exact /x/mail line   | refused as unauthorized   |
        | act.x.calendar.config     | /x/calendar line     | cannot open /x/mail        |

    @wip
    Scenario: An ordinary action never receives a credential
      Given an agent may perform act.x.mail.send through a tool host
      And the tool host opens /x/mail only owner-locally or with its own exact config authority and line
      When Core authorizes and Gamma commits the action
      Then the tool host resolves the credential at the last moment
      And the agent receives no config plaintext, DK or vault line

    @wip
    Scenario: An external secret manager is custody, never authority
      Given /x/mail material is held by an external secret manager
      When a caller has no owner-local context and lacks exact config authority or line
      Then the secret manager result cannot authorize or open the vault
      And Core remains the source of the protocol verdict

    @wip
    Scenario: Audit and config capabilities are cryptographically distinct
      Given one holder may audit sealed action arguments
      And another holder may open /x/mail config
      When each capability is exercised
      Then neither capability opens the other's sealed material

    @wip
    Scenario Outline: Rotation and config mutation are atomic and connector-local
      Given mail and calendar have independent vault nodes
      When "<operation>" is attempted for mail
      Then only /x/mail recipients, versions and roots may change
      And Gamma, config evidence and publication commit atomically
      And fresh-store keyless verification receives no credential

      Examples:
        | operation                          |
        | valid config edit                  |
        | recipient revocation and rotation  |
        | local vault update after an out-of-protocol upstream secret replacement |

    @wip
    Scenario: Failed vault mutation leaves no partial credential state
      Given a published bundle snapshotted before a mail config mutation
      And an injected failure before local commit
      When the authorized mutation is attempted
      Then the canonical bundle remains byte-for-byte identical
      And no Gamma entry, header generation or config blob from the attempt is reachable

    @wip
    Scenario: Public vault evidence and refusals reveal no secret
      Given a published mail config mutation and one refused vault attempt
      When a keyless verifier inspects manifests, proofs, Gamma clear fields, logs and errors
      Then it finds no credential, config plaintext, private key or DK
      And encrypted normative header lines remain opaque and non-authorizing

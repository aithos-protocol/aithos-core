Feature: Bundle and editions
  The bundle is the subject's entire state as files: indexes, sealed blobs,
  headers, DID document, and a signed manifest. Editions form a linear,
  hash-pinned chain; every check reads files — a server is never a trust
  party. (spec 02.3, 02.6)

  Rule: Editions chain and verify offline

    Scenario: Initialising a bundle publishes a verifiable first edition
      Given a fresh identity
      When I initialise its bundle
      Then edition 1 verifies offline
      And the manifest pins the DID document

    Scenario: Every publication extends the chain
      Given an initialised bundle
      When I create circle folder "projets/perso" with a section "note1" tagged "toto"
      And I publish the edition
      Then edition 2 verifies and pins edition 1 as its predecessor

    Scenario: A tampered file fails the edition
      Given a published bundle
      When one byte of a pinned file is altered
      Then edition verification is rejected

    Scenario: A broken chain fails closed
      Given a bundle with two editions
      When the newest manifest claims a wrong predecessor hash
      Then edition verification is rejected

  Rule: Content round-trips through the sealed store

    Scenario: The owner reads back what was written
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner reads "projets/perso/note1" from circle
      Then the section body comes back intact

    Scenario: Display paths resolve through names, keys through sids
      Given a published bundle with section "note1" in circle "projets/perso"
      When the folder "perso" is renamed to "intime"
      And the edition is republished
      Then the owner reads the same section at "projets/intime/note1"

  Rule: The public zone reads without any key

    Scenario: A stranger reads public content with no key at all
      Given a published bundle with a public section "bio" in folder "profil"
      When a stranger with no key reads "profil/bio" from public
      Then the section body is readable in clear
      And its integrity checks against the signed edition

  Rule: The self zone leaks no structure

    Scenario: Self is a flat sea of opaque blobs
      Given a bundle with a self folder "enfance/cicatrices" containing section "blessure"
      When I inspect every file of the self zone as a stranger
      Then no folder name, section name, title or tag appears anywhere
      And the owner still reconstructs the full tree from sealed descriptors

  Rule: Owner operations have durable parity across all three zones

    @wip
    Scenario Outline: The local owner performs every content operation without a mandate
      Given an owner-local bundle session for zone "<zone>"
      And a published existing folder and section in that zone
      When the owner performs "<operation>" through the common bundle operation
      Then the operation succeeds from the narrow owner capability without a mandate
      And every mutation is journalized without consuming mandate counters
      And the resulting edition reopens and verifies from a fresh local store

      Examples:
        | zone   | operation |
        | public | list      |
        | public | read      |
        | public | create    |
        | public | edit      |
        | public | delete    |
        | circle | list      |
        | circle | read      |
        | circle | create    |
        | circle | edit      |
        | circle | delete    |
        | self   | list      |
        | self   | read      |
        | self   | create    |
        | self   | edit      |
        | self   | delete    |

  Rule: A local mutation commits state and Gamma as one transaction

    Scenario Outline: Failure before the logical commit point preserves the old bundle byte for byte
      Given a published "<store>" bundle snapshotted byte for byte
      And an injected failure at "<boundary>"
      When the owner attempts a valid mutation and publication
      Then the mutation is refused before canonical effect
      And the canonical bundle is byte-for-byte identical to the snapshot
      And re-reading or reopening the "<store>" observes the old manifest and Gamma head
      And no failed-mutation blob, index, header, wrap or Gamma entry exists in the canonical bundle
      And staging remains non-canonical and is cleaned or recoverably resolved with no local-mutation orphan

      Examples:
        | store    | boundary          |
        | MemStore | cryptography      |
        | MemStore | blob preparation  |
        | MemStore | index preparation |
        | MemStore | header or wrap    |
        | MemStore | Gamma validation  |
        | MemStore | before state replacement |
        | FsStore  | cryptography      |
        | FsStore  | blob preparation  |
        | FsStore  | index preparation |
        | FsStore  | header or wrap    |
        | FsStore  | Gamma validation  |
        | FsStore  | before commit marker or reference |

    Scenario Outline: A successful local transaction publishes content and Gamma together
      Given a published "<store>" bundle snapshotted byte for byte
      When the owner commits a valid circle edit
      Then one deterministic write-set advances content, roots, manifest and Gamma
      And normal completion exposes the complete new state at one logical commit point
      And a crash or lost acknowledgement at that point resolves to the complete old or complete new state from the canonical manifest and Gamma head
      And no reader or reopen observes an individual file replacement or partial edition

      Examples:
        | store    |
        | MemStore |
        | FsStore  |

  Rule: Local capabilities and paths stay narrow

    @wip
    Scenario Outline: A bundle operation uses only its narrow opaque cryptographic capability
      Given one Ethos-and-actor session backed by a purpose-bound opaque "<capability>" capability
      When Bundle submits the typed "<protocol object>" that needs "<capability>"
      Then "<observable result>"
      And using that capability for "<mismatched object>" is refused
      And arbitrary bytes or a mismatched Ethos, actor, purpose, node, version or recipient are refused
      And a capability for another protocol artifact class cannot substitute
      And no universal sign, open or wrap capability is exposed
      And no seed or private key is accepted or returned by the bundle operation

      Examples:
        | capability | protocol object                         | mismatched object                       | observable result                                |
        | sign       | domain-tagged edition manifest          | Gamma entry                             | the signature verifies against the public key    |
        | sign       | domain-tagged Gamma entry               | edition manifest                        | the signature verifies against the public key    |
        | open       | node-and-version-bound sealed body      | body from a sibling node or version      | the expected plaintext is recovered only locally |
        | wrap       | node-version-and-recipient header line  | line for another node or recipient       | only the intended recipient opens the wrapped key |

    Scenario Outline: An untrusted path or Store key can never escape its selected root
      Given a published "<store>" bundle snapshotted byte for byte
      When a caller supplies "<invalid input>" as a "<input kind>" under "<filesystem condition>"
      Then the operation is rejected before any out-of-root store access
      And the canonical bundle is byte-for-byte identical to the snapshot

      Examples:
        | store    | input kind   | invalid input                  | filesystem condition                         |
        | MemStore | display path | ../circle/secret               | no filesystem indirection                    |
        | MemStore | display path | /absolute/section              | no filesystem indirection                    |
        | MemStore | display path | folder/./section               | no filesystem indirection                    |
        | MemStore | display path | folder//section                | no filesystem indirection                    |
        | FsStore  | display path | folder/link-out/section        | link-out is a symlink outside the zone       |
        | FsStore  | Store key    | ../../outside                  | no filesystem indirection                    |
        | FsStore  | Store key    | e/circle/unlisted-object.json  | no filesystem indirection                    |
        | FsStore  | Store key    | e/circle/link-out/index.json   | intermediate link-out targets outside root   |
        | FsStore  | Store key    | e/circle/index.json            | final index component links outside root      |
        | FsStore  | cold-load key | manifest.json                 | signed manifest component links outside root  |

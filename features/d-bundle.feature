@d-bundle
Feature: Bundle and editions
  The bundle is the subject's entire state as files: indexes, sealed blobs,
  headers, DID document, and a signed manifest. Editions form a linear,
  hash-pinned chain; every check reads files — a server is never a trust
  party. (spec 02.3, 02.6)

  Rule: Editions chain and verify offline

    @audit-partial @dbnd-003 @dbnd-005
    # AUDIT DBND-003, DBND-005 — PARTIAL.
    # Then edition 1 verifies offline is a bare verify().expect() sharing one
    # step body with the public zone's integrity Then (DBND-003): neither the
    # ordinal nor offline is asserted. And the manifest pins the DID document is
    # subsumed by the step before it (DBND-005).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: Initialising a bundle publishes a verifiable first edition
      Given a fresh identity
      When I initialise its bundle
      Then edition 1 verifies offline
      And the manifest pins the DID document

    @audit-partial @dbnd-006
    # AUDIT DBND-006 — PARTIAL.
    # The strongest scenario in the feature: ev-5474b889 kills it. But Every is
    # demonstrated on one publication and linearity on zero forks (DBND-006).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: Every publication extends the chain
      Given an initialised bundle
      When I create circle folder "projets/perso" with a section "note1" tagged "toto"
      And I publish the edition
      Then edition 2 verifies and pins edition 1 as its predecessor

    @audit-partial @dbnd-002 @dbnd-004
    # AUDIT DBND-002, DBND-004 — PARTIAL.
    # The tampered object is re-derived twice, so deleting the whole flat-pin loop
    # leaves this green (DBND-002, ev-de2706a8); the sealed-blob rollback the pins
    # uniquely cover is never tampered. No positive control (DBND-004).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: A tampered file fails the edition
      Given a published bundle
      When one byte of a pinned file is altered
      Then edition verification is rejected

    @audit-partial @dbnd-001 @dbnd-004
    # AUDIT DBND-001, DBND-004 — SEMANTIC_FALSE_POSITIVE.
    # The rejection is produced by an unpinned-stray check, not by the chain link:
    # green with the predecessor-hash comparison deleted (DBND-001, ev-d1fc33b5).
    # fails closed is not distinguished from any other rejection. No positive
    # control (DBND-004).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: A broken chain fails closed
      Given a bundle with two editions
      When the newest manifest claims a wrong predecessor hash
      Then edition verification is rejected

  Rule: Content round-trips through the sealed store

    @audit-partial @dbnd-007 @dbnd-009 @dbnd-011
    # AUDIT DBND-007, DBND-009, DBND-011 — PARTIAL.
    # The Rule's word sealed is asserted by neither scenario: green against a
    # cleartext store (DBND-007, ev-23aeba39). The publication in the Given is
    # observed by no assertion (DBND-009). No negative control (DBND-011).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: The owner reads back what was written
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner reads "projets/perso/note1" from circle
      Then the section body comes back intact

    @audit-partial @dbnd-008 @dbnd-007 @dbnd-009 @dbnd-010 @dbnd-011
    # AUDIT DBND-008, DBND-007, DBND-009, DBND-010, DBND-011 — SEMANTIC_FALSE_POSITIVE.
    # A rename that renames nothing passes: green with the old name surviving as an
    # alias (DBND-008, ev-f7261aa9). No sid and no blob_sha is ever compared. Also
    # DBND-007 (sealed), DBND-009 (republication unobserved), DBND-010 (the step's
    # parent is hard-coded), DBND-011 (no negative control).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: Display paths resolve through names, keys through sids
      Given a published bundle with section "note1" in circle "projets/perso"
      When the folder "perso" is renamed to "intime"
      And the edition is republished
      Then the owner reads the same section at "projets/intime/note1"

  Rule: The public zone reads without any key

    @audit-partial @dbnd-003 @dbnd-012
    # AUDIT DBND-003, DBND-012 — PARTIAL.
    # Keylessness is proved at the type level and is the one claim here proved
    # better than a Then could prove it. But its integrity checks against the
    # signed edition is a whole-bundle verify() that never touches the value read
    # (DBND-003, ev-c7f65638). The owner content signature spec 02.11 promises is
    # verified by nothing in the tree (DBND-012, routed).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: A stranger reads public content with no key at all
      Given a published bundle with a public section "bio" in folder "profil"
      When a stranger with no key reads "profil/bio" from public
      Then the section body is readable in clear
      And its integrity checks against the signed edition

  Rule: The self zone leaks no structure

    @audit-partial @dbnd-014 @dbnd-013 @dbnd-015 @dbnd-016 @dbnd-017
    # AUDIT DBND-014, DBND-013, DBND-015, DBND-016, DBND-017 — SEMANTIC_FALSE_POSITIVE.
    # The scenario passes having inspected nothing: green with the e/self prefix
    # hidden from listing (DBND-014, ev-0b4e1076). anywhere searches one of four
    # normative layers and misses the signed Gamma log (DBND-013, ev-f1718be8).
    # Also DBND-015 (needles hard-coded), DBND-016 (flat sea reaches no
    # assertion), DBND-017 (no self body is read anywhere in this feature).
    # Detail: docs/audits/features/d-bundle.md
    Scenario: Self is a flat sea of opaque blobs
      Given a bundle with a self folder "enfance/cicatrices" containing section "blessure"
      When I inspect every file of the self zone as a stranger
      Then no folder name, section name, title or tag appears anywhere
      And the owner still reconstructs the full tree from sealed descriptors

  Rule: Owner operations have durable parity across all three zones

    @audit-partial @dbnd-018 @dbnd-019 @dbnd-021 @dbnd-022 @dbnd-023 @dbnd-024 @dbnd-040
    # AUDIT DBND-018, DBND-019, DBND-021, DBND-022, DBND-023, DBND-024, DBND-040 — PARTIAL.
    # P1 DBND-018: without consuming mandate counters is assert_eq!(0, 0) and all
    # fifteen rows stay green when every owner entry declares a mandate chain
    # (ev-19a635cf). DBND-040: journalized is proved by cardinality alone -- an
    # edit that logs under a create's Kind stays green (ev-f18d4843). Three rows
    # satisfy the capability clause on keyless paths (DBND-019, ev-b6a36f72).
    # Also DBND-021 (vector fields with no consumer), DBND-022 (no resulting
    # edition on six rows), DBND-023 (the Givens construct nothing), DBND-024
    # (parity is never checked comparatively).
    # Withdrawn from this scenario 2026-08-04: DBND-020, refuted by the panel.
    # Detail: docs/audits/features/d-bundle.md
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

    @audit-partial @dbnd-023 @dbnd-026 @dbnd-027 @dbnd-028 @dbnd-036
    # AUDIT DBND-023, DBND-026, DBND-027, DBND-028, DBND-036 — PARTIAL.
    # The best-proved block in the feature: real fault injection, a three-way byte
    # comparison, and a control that kills four rows (ev-f0125e0b). DBND-026: the
    # byte comparison stops at canonical_base(), so a permanently leaked staging
    # generation is invisible to all 51 scenarios (ev-f7ee3968, the M1+M3 pair the
    # auditor named as its own closure test). Also DBND-023 (the Given constructs
    # nothing), DBND-027 (four Thens, one boolean), DBND-028 (six boundary names,
    # at most four injection points -- two rows measurably indistinguishable under
    # ev-f0125e0b), DBND-036 (one sentence, three comparators).
    # Detail: docs/audits/features/d-bundle.md
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

    @audit-partial @dbnd-025 @dbnd-023 @dbnd-027
    # AUDIT DBND-025, DBND-023, DBND-027 — PARTIAL.
    # Line 121 asserts crash recovery and nothing induces a crash: green with the
    # whole FsStore recovery path gutted (DBND-025, ev-7caa8332). Line 119 is a
    # real positive control and is credited. Also DBND-023, DBND-027.
    # Detail: docs/audits/features/d-bundle.md
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

    @audit-partial @dbnd-029 @dbnd-031 @dbnd-032 @dbnd-039
    # AUDIT DBND-029, DBND-031, DBND-032, DBND-039 — PARTIAL.
    # P1 DBND-029: no seed or private key is accepted or returned is assert!(!false)
    # and a public manifest_private_key() accessor leaves the gate green
    # (ev-ed18d7ef). The mismatched_object column reaches no code (DBND-031,
    # ev-3fa9d172 with control ev-1eefbb66); the two Then lines about cross-class
    # substitution are decided by a grep of session.rs that sign_any defeats
    # (DBND-032, ev-794d59c3). DBND-039: the {string} Then is an unbounded
    # wildcard over the whole suite.
    # Withdrawn from this scenario 2026-08-04: DBND-020 and DBND-030, both refuted
    # by the panel. DBND-030's removal leaves DBND-039 as the only finding asking
    # for the observable_result Then to be rewritten.
    # Detail: docs/audits/features/d-bundle.md
    Scenario Outline: A bundle operation uses only its narrow opaque cryptographic capability
      Given one Ethos-and-actor session backed by a purpose-bound opaque "<capability>" capability
      When Bundle submits the typed "<protocol_object>" that needs "<capability>"
      Then "<observable_result>"
      And using that capability for "<mismatched_object>" is refused
      And arbitrary bytes or a mismatched Ethos, actor, purpose, node, version or recipient are refused
      And a capability for another protocol artifact class cannot substitute
      And no universal sign, open or wrap capability is exposed
      And no seed or private key is accepted or returned by the bundle operation

      Examples:
        | capability | protocol_object                         | mismatched_object                       | observable_result                                |
        | sign       | domain-tagged edition manifest          | Gamma entry                             | the signature verifies against the public key    |
        | sign       | domain-tagged Gamma entry               | edition manifest                        | the signature verifies against the public key    |
        | open       | node-and-version-bound sealed body      | body from a sibling node or version      | the expected plaintext is recovered only locally |
        | wrap       | node-version-and-recipient header line  | line for another node or recipient       | only the intended recipient opens the wrapped key |

    @audit-partial @dbnd-033 @dbnd-034 @dbnd-023 @dbnd-036 @dbnd-037 @dbnd-038
    # AUDIT DBND-033, DBND-034, DBND-023, DBND-036, DBND-037, DBND-038 — PARTIAL.
    # The six FsStore rows are genuinely discriminating against a real per-segment
    # symlink walk and are credited. The four MemStore rows are not: all ten stay
    # green with validate_display_path reduced to Ok(()) (DBND-033, DBND-034,
    # ev-2d2ebd1b), and no row supplies a valid input. Also DBND-023, DBND-036,
    # DBND-037 (row e/circle/unlisted-object.json is out-of-layout, not
    # out-of-root), DBND-038 (the ../../outside row's escape detector cannot fire).
    # Withdrawn from this scenario 2026-08-04: DBND-035, refuted by the panel --
    # the repository's own matrix declares the six confinement surfaces as
    # application points of one check, not six coverage obligations.
    # Detail: docs/audits/features/d-bundle.md
    Scenario Outline: An untrusted path or Store key can never escape its selected root
      Given a published "<store>" bundle snapshotted byte for byte
      When a caller supplies "<invalid_input>" as a "<input_kind>" under "<filesystem_condition>"
      Then the operation is rejected before any out-of-root store access
      And the canonical bundle is byte-for-byte identical to the snapshot

      Examples:
        | store    | input_kind   | invalid_input                  | filesystem_condition                         |
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

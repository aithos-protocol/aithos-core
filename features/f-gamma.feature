Feature: The gamma log
  Every mutation and every agentic action leaves one hash-chained, signed
  entry in the gamma log. It is the history spine and the counter the
  serverless design otherwise lacks: budgets, liveness and freshness are
  enforced by tallying files — a server is never a trust party. (spec 07)

  Rule: The log is a write-once, hash-chained record

    Scenario: Entries chain and the manifest pins the head
      Given an initialised bundle
      When the owner appends a section addition and a heartbeat
      Then the log verifies offline
      And the manifest pins the log head

    Scenario: The chain crosses monthly segments transparently
      Given a bundle with entries logged in two different months
      Then the log lives in two pinned segment files
      And the whole chain verifies across the boundary

    Scenario: A tampered past entry breaks the chain
      Given a bundle with a three-entry log
      When one byte of the middle entry is altered
      Then log verification is rejected

    Scenario: An entry claiming a wrong predecessor is rejected
      Given a bundle with a three-entry log
      When an entry is appended whose prev is not the current head
      Then log verification is rejected

  Rule: Every entry names its authority — owner key or mandate chain

    Scenario: An owner entry is signed by the content key alone
      Given an initialised bundle
      When the owner appends a section addition
      Then the entry verifies with no mandate attached

    Scenario: A delegated action carries its full mandate chain
      Given an agent granted action rights on connector "x.gmail"
      When the agent appends an action entry
      Then the entry verifies against the chain at its own timestamp

    Scenario: An action timestamped outside its mandate window is rejected
      Given an agent granted action rights for 7 days
      When the agent appends an action entry timestamped at day 8
      Then log verification is rejected

  Rule: The log is the agentic meter — budgets are tallies over files

    Scenario: The action after the budget is rejected
      Given an agent granted action rights with max_actions 3
      When the agent appends three action entries
      Then a fourth action entry is rejected

    Scenario: A delegate's actions drain every ancestor's budget
      Given an agent granted action rights with max_actions 3 and issue depth 1
      And the agent delegates its perimeter to a helper
      When the agent appends one action and the helper appends two
      Then a further action by either key is rejected

    Scenario: Minting a child is itself a counted, logged act
      Given an agent granted issue rights with max_children 2
      When the agent delegates twice, each grant logged
      Then a third delegation is rejected

    Scenario: The windowed rate limit counts inside its window only
      Given an agent granted action rights with max_actions_per 2 per 24 hours
      When the agent appends two actions on day 1
      Then a third action on day 1 is rejected
      But an action on day 2 verifies

    Scenario: A per-action-kind budget counts only its kind
      Given an agent granted gmail actions with rate_limit 2 "reply" per 72 hours
      When the agent appends two "reply" actions on day 1
      Then a third "reply" on day 2 is rejected
      But a "label" action on day 2 verifies

    Scenario: An unlogged grant is a silent action — its chain is dead
      Given an agent that minted a sub-mandate without logging the grant
      When the helper presents an action under that chain
      Then the action is rejected

  Rule: Heartbeat bounds autonomy to owner presence

    Scenario: A fresh beacon keeps the head mandate alive
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 20 verifies

    Scenario: Owner silence beyond every plus grace suspends the mandate
      Given a head mandate with heartbeat every 30 days grace 72 hours
      And an owner beacon at day 0
      Then an action at day 34 is rejected

    Scenario: The owner's return resumes a suspended mandate
      Given a head mandate suspended by owner silence
      When the owner beacons again
      Then the next action verifies

    Scenario: An agent can never beacon for itself
      Given a head mandate with heartbeat every 30 days grace 72 hours
      When the head agent forges a heartbeat with its own key
      Then the beacon is rejected

  Rule: Off-log artifacts anchor to a recent head — backdating is bounded

    Scenario: A request anchored to the current head verifies
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to the current log head
      Then the request verifies

    Scenario: A stale anchor is rejected
      Given an agent under a mandate with freshness 24 hours
      When the agent presents a request anchored to a head 48 hours old
      Then the request is rejected

  Rule: The log reveals the act, never the content — reading is owner-first

    Scenario: A mutation body is sealed under the target's own key
      Given a published bundle with a circle section
      When the owner logs a modification of that section
      Then a reader of that section opens the entry body
      But the entry alone reveals only kind, time and author — not the target

    Scenario: A stranger holding the files sees the counting skeleton only
      Given a bundle whose log records mutations and actions
      When someone with no key reads the log files
      Then the chain and the budgets still verify
      But no target, tag or content is revealed

    Scenario: A subtree grant opens exactly its entries and nothing more
      Given logged mutations on sections under "projets" and under "sante"
      When the owner grants the agent read on circle folder "projets"
      Then the agent opens the bodies of the "projets" entries by their hints
      But the "sante" entry bodies stay sealed to it

    Scenario: Appending never requires reading
      Given an agent granted action rights and no read grant
      When the agent appends an action entry knowing only the pinned log head
      Then the entry chains and verifies

  Rule: Auditing and searching gamma — the owner by default, others by mandate

    Scenario: The owner searches its log by filter
      Given a bundle whose log records mutations and actions over two months
      When the owner queries actions of kind "action" on "x.gmail" from day 10 to day 40
      Then exactly the matching entries come back
      And the owner opens every sealed body among them

    Scenario: An audit mandate opens the whole log
      Given logged mutations by the owner and by an agent under "projets"
      When the owner grants an auditor read.gamma with the zone keys
      Then the auditor opens every entry body, including acts it never made

    Scenario: A scoped audit mandate is honored dimension by dimension
      Given an auditor granted read.gamma on action "reply" from day 1 to day 30
      When the auditor queries replies of day 20
      Then the matching entries come back
      But a query for day 40 is refused
      And a query for action "send" is refused

  Rule: Gamma replays every protocol consumption semantically

    @wip
    Scenario Outline: Each canonical operation is authorized before its entry joins history
      Given a candidate Gamma entry for "<operation class>" by "<actor>"
      When Core replays it against the exact historical prefix
      Then form, time, signer, actor authority and operation coverage are verified
      And applicable revocation, constraints, receipts and counters are consumed
      And only then does the entry join replay state

      Examples:
        | operation class      | actor   |
        | Ethos create         | owner   |
        | Ethos edit           | grantee |
        | Ethos delete         | grantee |
        | connector action     | grantee |
        | metered inference    | grantee |
        | journalized read     | grantee |
        | sub-grant            | grantee |
        | scoped revocation    | grantee |
        | disjoint merge kind:merge | grantee |

    @wip
    Scenario Outline: A structurally valid entry with invalid semantics is refused
      Given a hash-linked and correctly encoded candidate Gamma history
      When replay encounters "<semantic defect>"
      Then semantic replay is refused at that entry
      And no later entry or counter is accepted

      Examples:
        | semantic defect                                  |
        | historical action N plus 1 beyond its limit      |
        | receipt replayed under another consumption       |
        | stale heartbeat or freshness state               |
        | consumption at or after effective revocation     |
        | sub-grant absent from Gamma                      |
        | direct-child grant beyond max_children           |
        | mutation outside the authorized opaque SID       |
        | valid signature presented under the wrong chain  |
        | owner entry signed by a different owner key      |
        | delegated entry signed without leaf possession   |

    @wip
    Scenario: A log link and signature never substitute for semantic verification
      Given a Gamma chain whose hashes, order and signatures all verify
      But one delegated mutation is outside its mandate perimeter
      When the bundle performs cold verification
      Then the edition is rejected as semantically invalid
      And no structural-only helper reports the history authorized

  Rule: One typed operation occurrence has one cross-view commitment

    @wip
    Scenario Outline: K1-B selects one closed operation-facts family
      Given a W1 projection for operation kind "<kind>"
      When its operation member is encoded
      Then operation has exactly kind and facts_ref
      And facts_ref has exactly aithos-operation-facts-core and digest
      And the facts profile is "1.0.0-draft.1"
      And its selected closed facts family is "<family>"

      Examples:
        | kind        | family                                  |
        | read        | Ethos read, signed Gamma presentation or vault-config read |
        | mutation    | Ethos, structure or vault-config mutation |
        | action      | pre-effect connector action             |
        | inference   | pre-effect connector inference          |
        | grant       | mandate grant                           |
        | revoke      | mandate revocation                      |
        | rotate      | key or protected-state rotation         |
        | publication | normal, merge or resolution publication |

    @wip
    Scenario Outline: The K1-B operation wrapper is fail-closed
      Given a candidate W1 operation wrapper with "<defect>"
      When Core validates its closed form before commitment comparison
      Then the wrapper is refused
      And no operation commitment or operation_ref is emitted

      Examples:
        | defect                                     |
        | missing kind                               |
        | unknown kind                               |
        | missing facts_ref                          |
        | null facts_ref                             |
        | extra operation member                     |
        | extra facts_ref member                     |
        | unknown operation-facts profile            |
        | malformed or non-lowercase facts digest    |
        | facts family different from operation kind |

    @wip
    Scenario: K1.1-B fixes the operation-facts envelope and digest preimage
      Given a complete operation-facts document F for one registered kind
      When Core derives its facts reference
      Then F has exactly aithos-operation-facts-core, kind and facts
      And its profile equals facts_ref and its kind equals operation.kind
      And facts_ref.digest is lowercase SHA-256 of "aithos-core/v1/operation-facts", NUL and RFC8785-JCS of F
      And null, an extra member or a different selected family is refused

    @wip
    Scenario Outline: K1.1-B state presence has one exact closed shape
      Given a logical operation state is "<state>"
      When its K1.1-B state fact is projected
      Then its exact top-level members are "<members>"
      And state_ref is "<reference>"
      And null or any extra member is refused

      Examples:
        | state   | members         | reference                                   |
        | absent  | state           | forbidden                                   |
        | present | state,state_ref | exact profile and lowercase SHA-256 digest |

    @wip
    Scenario: K1.1-B commits the exact protected current object set
      Given one present logical state with affected canonical store objects
      When its state-fact document S is encoded
      Then S has exactly aithos-state-fact-core and a non-empty objects array
      And every object has exactly key_commitment and byte_commitment
      And the commitments use the state-key and state-bytes domains over the exact UTF-8 key and stored bytes
      And objects are sorted by lowercase key_commitment with no duplicate key
      And state_ref.digest is lowercase SHA-256 of "aithos-core/v1/state-fact", NUL and RFC8785-JCS of S

    @wip
    Scenario Outline: K1.1-B state facts fail closed without disclosing protected coordinates
      Given a candidate state-fact document with "<defect>"
      When Core validates it before operation commitment comparison
      Then the state fact is refused
      And no operation commitment or operation_ref is emitted
      And no clear store key, path, SID, vault record name, target or protected content is accepted in the state fact

      Examples:
        | defect                                  |
        | unknown state-fact profile              |
        | empty objects array                     |
        | unsorted objects array                  |
        | duplicate key commitment                |
        | malformed or non-lowercase commitment   |
        | missing affected object                 |
        | unrelated extra object                  |
        | extra object member                     |
        | state digest mismatch                   |

    @wip
    Scenario Outline: K1.2-R-B selects one exact closed read variant
      Given a read facts object in domain "<domain>"
      When Core validates its selected member table
      Then its exact members are "<members>"
      And null, a missing member or an extra member is refused

      Examples:
        | domain       | members                                        |
        | ethos        | domain,zone,sid,source_edition                  |
        | gamma        | domain,source_head,request_digest               |
        | vault-config | domain,connector,record_key,source_edition      |

    @wip
    Scenario: K1.2-R-B binds an exact source without a circular carrier digest
      Given one signed source manifest and one canonical read.gamma query string Q
      When their read facts are committed
      Then source_edition is sha256-prefixed existing manifest chain hash
      And source_head is the exact non-empty Gamma head being presented
      And request_digest is domain-separated SHA-256 of the exact UTF-8 bytes of Q
      And Q uses canonical selector order dir,id,tag,kind,action,since,until
      And no signature, operation_ref or presentation carrier digest enters request_digest

    @wip
    Scenario Outline: K1.2-R-B creates an occurrence only for opposable read evidence
      Given an authorized "<read>" with no signed read evidence
      When the local read completes
      Then no operation-facts document or persisted operation_ref is produced
      But journalized or explicitly presented read evidence uses one read occurrence
      And every native view carries that same operation_ref

      Examples:
        | read                    |
        | Ethos read              |
        | Gamma query             |
        | vault-config read       |

    @wip
    Scenario Outline: K1.2-R-B read facts fail closed without disclosing protected coordinates
      Given a candidate read facts object with "<defect>"
      When Core validates it before operation commitment
      Then the read facts are refused as InvalidOperationFacts
      And no operation commitment or operation_ref is emitted

      Examples:
        | defect                                      |
        | unknown read domain                         |
        | unknown zone or non-canonical SID           |
        | malformed or mismatched source edition      |
        | empty or malformed source head              |
        | non-canonical Gamma query encoding           |
        | mismatched Gamma request digest              |
        | mismatched vault record-key commitment      |
        | clear display path or vault record name     |

    @wip
    Scenario Outline: K1.2-M-B selects one exact closed mutation variant
      Given a mutation facts object in domain "<domain>" with verb "<verb>"
      When Core validates its selected member table
      Then its exact members are "<members>"
      And null, a missing member or an extra member is refused

      Examples:
        | domain       | verb          | members                                                                  |
        | ethos        | any registered | domain,verb,zone,sid,dir,before,after                                    |
        | structure    | create        | domain,verb,zone,node_kind,sid,destination,before,after                   |
        | structure    | rename/delete | domain,verb,zone,node_kind,sid,source,before,after                        |
        | structure    | move          | domain,verb,zone,node_kind,sid,source,destination,before,after            |
        | vault-config | any registered | domain,verb,connector,record_key,before,after                             |

    @wip
    Scenario Outline: K1.2-M-B fixes every mutation state transition
      Given a closed mutation facts object for "<family verb>"
      When Core validates its before and after states
      Then before is "<before>"
      And after is "<after>"
      And a present-to-present transition has different state reference digests

      Examples:
        | family verb                    | before  | after   |
        | every create                   | absent  | present |
        | every delete                   | present | absent  |
        | ethos edit or redact           | present | present |
        | structure rename or move       | present | present |
        | vault-config edit              | present | present |

    @wip
    Scenario: K1.2-M-B structural coordinates are exact and non-null
      Given a structural mutation with canonical target SID and parent SID arrays
      When its source and destination applicability is checked
      Then create carries destination only
      And rename and delete carry source only
      And move carries source and destination
      And each array is root-to-leaf, duplicate-free and excludes the target SID
      And cross-zone, descendant destination and unknown node-kind candidates are refused

    @wip
    Scenario: K1.2-M-B vault and self facts disclose no protected coordinate
      Given one vault-config mutation and one self mutation
      When their public W1 projections and protected facts are separated
      Then the vault facts carry the exact state-key record commitment and no record name
      And the self public projection carries only facts_ref
      And self dir, source, destination and tag claims grant no write authority
      And an opaque proof binds every claimed target and state transition

    @wip
    Scenario Outline: K1.2-M-B mutation facts fail closed before commitment
      Given a candidate mutation facts object with "<defect>"
      When Core validates its closed family
      Then the mutation facts are refused
      And no operation commitment or operation_ref is emitted

      Examples:
        | defect                                      |
        | unknown domain                              |
        | unknown verb for the selected domain        |
        | unknown zone or node kind                   |
        | null source or destination                  |
        | source or destination on the wrong variant  |
        | duplicate or non-canonical SID coordinate   |
        | invalid before and after transition         |
        | equal state digests for a mutation          |
        | mismatched vault record-key commitment      |
        | clear display path or vault record name     |

    @wip
    Scenario Outline: K1.2-AI-B selects one exact closed pre-effect family
      Given a "<kind>" facts object
      When Core validates its selected member table
      Then its exact members are "<members>"
      And null, a missing member or an extra member is refused

      Examples:
        | kind      | members                                                   |
        | action    | connector,action,catalog_ref,args_hash,budget,purpose      |
        | inference | provider,model,request_digest,budget,purpose                |

    @wip
    Scenario: K1.2-AI-B binds action arguments and one approved catalog reference
      Given exact connector action arguments and one approved catalog reference
      When the action facts are committed before effect
      Then args_hash is the historical SHA-256 of RFC8785-JCS arguments
      And catalog_ref has exactly catalog_version, catalog_digest and approval_digest
      And the exact action and catalog digest bind the derived class without duplicating it
      And neither a catalog signature nor owner approval is accepted as the other proof

    @wip
    Scenario: K1.2-AI-B binds exact private inference request bytes without args_hash
      Given exact private provider request-body bytes fixed before an inference
      When the inference facts are committed
      Then request_digest is domain-separated SHA-256 of those exact bytes
      And provider and model are independently bound as exact non-empty identifiers
      And transport credentials, request plaintext and args_hash are absent

    @wip
    Scenario Outline: K1.2-AI-B makes budget and purpose applicability explicit
      Given effective mandates where "<fact>" is "<applicability>"
      When action or inference facts are projected
      Then the selected variant is "<variant>"
      And omission, null and a volunteered citation are refused

      Examples:
        | fact    | applicability | variant                          |
        | budget  | absent        | state=not-applicable             |
        | budget  | present       | state=cited plus exact budget_ref |
        | purpose | absent        | state=not-applicable             |
        | purpose | present       | state=cited plus exact purpose_ref |

    @wip
    Scenario Outline: K1.2-AI-B facts fail closed before any external effect
      Given candidate action or inference facts with "<defect>"
      When Core validates them before operation commitment
      Then the facts are refused as InvalidOperationFacts
      And no operation commitment, operation_ref or external effect is emitted

      Examples:
        | defect                                      |
        | malformed or mismatched catalog reference   |
        | mismatched action arguments                  |
        | mismatched inference request bytes           |
        | action carrying request_digest               |
        | inference carrying args_hash                 |
        | wrong budget applicability variant           |
        | wrong purpose applicability variant          |
        | tokens or a usage receipt before effect      |

    @wip
    Scenario Outline: K1.2-GRRP-B selects exact grant and revoke facts
      Given a "<kind>" operation targeting one complete signed mandate
      When Core validates its facts before commitment
      Then its exact members are "<members>"
      And the certificate digest includes the complete canonical signature value

      Examples:
        | kind   | members                                |
        | grant  | mandate_id,certificate_digest          |
        | revoke | mandate_id,certificate_digest,reason   |

    @wip
    Scenario Outline: K1.2-GRRP-B represents a revocation reason without optional wire
      Given the native revoke entry carries "<native reason>"
      When its closed reason fact is projected
      Then the variant is "<variant>"
      And null, empty text or a cross-view mismatch is refused

      Examples:
        | native reason | variant                    |
        | absent        | state=absent               |
        | device_lost   | state=present,text exact   |

    @wip
    Scenario Outline: K1.2-GRRP-B selects one standalone rotation domain
      Given a standalone rotation in "<domain>"
      When Core validates its closed target and state transition
      Then its exact target members are "<target members>"
      And before and after are present with different state digests

      Examples:
        | domain      | target members                                      |
        | ethos-zone  | domain,zone,mode,before,after                        |
        | ethos-node  | domain,zone,sid,mode,before,after                    |
        | vault       | domain,connector,mode,before,after                   |
        | identity    | domain,previous_did,next_did,transition_digest,before,after |

    @wip
    Scenario Outline: A derived rotation never creates a second occurrence
      Given a rotation is a deterministic consequence of "<parent operation>"
      When the parent state and changeset are committed
      Then the rotation is covered by that same operation occurrence
      And no rotate operation_ref, Gamma consumption or counter unit is added

      Examples:
        | parent operation      |
        | revoke                |
        | structural move       |
        | vault mutation        |

    @wip
    Scenario Outline: K1.2-GRRP-B selects one exact publication table
      Given a publication in mode "<mode>"
      When Core validates its facts
      Then predecessors have "<cardinality>"
      And exact members are "<members>"

      Examples:
        | mode       | cardinality                     | members                                                       |
        | normal     | zero at genesis otherwise one   | mode,height,predecessors,changeset_ref,contained_operations    |
        | merge      | exactly two sorted distinct     | mode,height,predecessors,changeset_ref,contained_operations    |
        | resolution | exactly two sorted distinct     | mode,height,predecessors,winner,changeset_ref,contained_operations |

    @wip
    Scenario: Publication facts are acyclic and derived
      Given a derived changeset with contained operation references in causal order
      When the publication operation is committed
      Then changeset_ref uses the closed changeset profile and domain
      And contained_operations equals the derived causal order without duplicates
      But neither the publication operation_ref nor candidate manifest hash is included

    @wip
    Scenario Outline: K1.2-GRRP-B structural facts fail closed before effect
      Given candidate grant, revoke, rotate or publication facts with "<defect>"
      When Core validates the selected family
      Then the facts are refused as InvalidOperationFacts
      And no operation commitment, counter or canonical effect is emitted

      Examples:
        | defect                                      |
        | mandate id and certificate mismatch         |
        | revoke reason mismatch                       |
        | unknown rotation domain or mode              |
        | equal rotation state digests                 |
        | derived rotation represented twice           |
        | wrong predecessor cardinality or order       |
        | resolution winner outside predecessors       |
        | omitted or duplicate contained operation     |
        | publication self-reference                   |

    @wip
    Scenario: Append-time and public evidence identify the same operation occurrence
      Given one fresh typed operation occurrence
      When its append-time, Gamma, authorship and edition views are projected
      Then every applicable view yields the same operation commitment
      And changing any applicable authority fact changes that commitment
      And no private operation argument appears in public commitment material

    @wip
    Scenario: Identical effects remain distinct occurrences
      Given two typed operation occurrences with identical effects and distinct occurrence anchors
      When their operation commitments are derived
      Then the commitments differ

    @wip
    Scenario: Cross-view evidence does not create another logical occurrence
      Given the applicable Gamma, authorship and edition views of one typed operation occurrence
      When semantic replay correlates their operation commitments
      Then all evidence refers to exactly one logical occurrence
      And no additional occurrence is inferred from the number of evidence views

    @wip
    Scenario: Historical evidence is never retrofitted with an operation commitment
      Given historical protocol evidence predating operation commitments
      When it is verified under its declared historical protocol version
      Then its bytes and hashes remain unchanged
      And no operation commitment is synthesized
      And no existing args_hash, Gamma identifier or edition hash is reinterpreted as that commitment
      And commitment material is refused under a historical or unknown protocol version, or without a version

  Rule: Gamma v2 is a monotone operation-evidence profile

    @wip
    Scenario Outline: Gamma v2 reference presence is fixed by the closed kind registry
      Given a manifest with aithos-core "1.0.0-draft.2"
      And a structurally valid Gamma v2 entry of kind "<kind>"
      When Core checks its top-level operation reference
      Then operation_ref is "<presence>"
      And when required it is the exact closed reference of the underlying occurrence
      And the opposite presence is refused

      Examples:
        | kind           | presence  |
        | section.add    | required  |
        | section.modify | required  |
        | section.delete | required  |
        | section.redact | required  |
        | ethos.read     | required  |
        | action         | required  |
        | inference      | required  |
        | grant          | required  |
        | revoke         | required  |
        | rotate         | required  |
        | merge          | required  |
        | heartbeat      | forbidden |

    @wip
    Scenario Outline: Manifest and Gamma versions are monotone on every causal edge
      Given a parent manifest "<parent manifest>" whose Gamma predecessor is "<parent gamma>"
      When a child manifest "<child manifest>" introduces a Gamma "<child gamma>" entry
      Then the profile transition is "<verdict>"

      Examples:
        | parent manifest | parent gamma | child manifest | child gamma | verdict  |
        | draft.1         | v1           | draft.1        | v1          | accepted |
        | draft.1         | v1           | draft.2        | v2          | accepted |
        | draft.2         | v2           | draft.2        | v2          | accepted |
        | draft.2         | v2           | draft.1        | v1          | refused  |
        | draft.1         | v1           | draft.1        | v2          | refused  |
        | draft.2         | v2           | draft.2        | v1          | refused  |
        | unknown         | v1           | draft.2        | v2          | refused  |
        | draft.1         | unknown      | draft.2        | v2          | refused  |

    @wip
    Scenario: A mixed-profile fork migrates at its draft2 merge
      Given disjoint competing branches under draft.1 with Gamma v1 and draft.2 with Gamma v2
      When the branches are joined by their deterministic merge
      Then the merge manifest declares draft.2
      And the new kind:merge entry is Gamma v2 with its operation_ref
      And monotonicity is checked against both manifest parents and both Gamma predecessors
      And every retained v1 and v2 parent byte remains unchanged
      And physical segment order never reinterprets a causal edge
      And no publication or resolution Gamma kind is introduced

    @wip
    Scenario: Gamma append is evidence rather than another operation
      Given one typed operation occurrence with an allocated operation_ref
      When its required Gamma evidence is appended
      Then the entry carries that exact operation_ref
      And the append allocates no additional operation occurrence
      And the Gamma id is never reinterpreted as the occurrence

    @wip
    Scenario: A local read.gamma query leaves no protocol artifact
      Given an auditor authorized to query Gamma under read.gamma
      When the auditor performs a local query without producing a signed presentation
      Then the perimeter is checked at operation time
      But no Gamma entry or persisted operation_ref is produced
      And the query is neither cold-replayable nor countable
      And log_reads does not reinterpret the query as ethos.read

    @wip
    Scenario: A signed read.gamma presentation is evidence without a new Gamma kind
      Given an authorized Gamma query whose result is made opposable
      When signed presentation evidence is produced
      Then it represents one canonical read or presentation occurrence
      And the signed evidence carries that occurrence's operation_ref
      But no gamma.read entry or automatic Gamma append is created

    @wip
    Scenario Outline: Gamma occurrence reuse distinguishes replay from a new operation
      Given an accepted operation-bearing Gamma v2 entry with occurrence "O" and commitment "C"
      When a second Gamma candidate has "<occurrence>" and "<commitment>" for "<effect>"
      Then the candidate is "<verdict>"
      And the same verdict applies when the candidate is first compared while joining branches

      Examples:
        | occurrence | commitment | effect      | verdict                              |
        | O          | C          | same effect | refused as replay before tally       |
        | O          | different  | any effect  | refused as equivocation before tally |
        | different  | different  | same effect | accepted as a distinct occurrence    |

    @wip
    Scenario: H2 roots tally raw Gamma lines and never deduplicate operation references
      Given a verified history with a Gamma v1 prefix, valid operation-bearing v2 entries and a v2 heartbeat
      And non-Gamma evidence shares an operation_ref with one accepted Gamma entry
      When segment roots and the counts trie are recomputed
      Then every exact Gamma line contributes once to its segment root and n
      And the existing kind and mandate fields alone feed the existing counters
      And the non-Gamma evidence contributes no H2 line or count
      And two distinct occurrences with identical effects remain two raw entries
      And replay or equivocation invalidates the edition instead of being deduplicated
      And no mutation or total-consumption counter is inferred

  Rule: Append-time and cold-time share one replay front door

    @wip
    Scenario Outline: The same candidate and prefix produce the same typed verdict
      Given identical public facts for "<case>"
      When the candidate is checked before append and after export to a fresh store
      Then the verdict, accepted prefix and counters are identical

      Examples:
        | case                                  |
        | valid owner mutation                  |
        | valid delegated mutation              |
        | valid connector action                |
        | revoked delegated mutation            |
        | exhausted action counter              |
        | missing public obligation receipt     |

    @wip
    Scenario: Active revocations are derived only from verified historical entries
      Given a hash-linked Gamma file containing a forged revocation entry
      When cold replay reconstructs active revocations
      Then the forged entry is rejected before it can revoke or authorize anything

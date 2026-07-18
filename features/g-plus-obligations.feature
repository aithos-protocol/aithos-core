Feature: Obligations — the general gate (spec 04.12)
  An obligation attaches a discharge requirement to a permit: an in-scope
  action may consume — append its entry — only if it carries a valid signed
  receipt from a pinned attestor whose verdict satisfies the predicate. One
  primitive, N enforcement types: guardrail pass, human approval (Model 1,
  the approver's device-held key), dual control, and the owner
  counter-signature — all the same wire shape, verified offline at
  gamma-append (tier V) beside the budget checks, recorded in the entry's
  checks[]. The attestor holds the logic; the protocol holds a signature.
  The receipt binds {obligation, mandate_id, action, args_hash, verdict,
  presented_digest?, at} — mandate_id is the entry's authorized_by, so a
  receipt never transfers across mandates, actions, or args.
  That shape is historical v1. A W1 operation uses R2 v2, whose exact
  operation_ref replaces the duplicated mandate/action/args tuple and binds one
  receipt to one occurrence without changing historical bytes.

  Rule: A guardrail obligation gates in-scope actions on a pass verdict

    Scenario: An action carrying a valid guardrail pass receipt appends
      Given an agent granted social publish under a guardrail obligation
      When the agent publishes with a receipt whose verdict is "pass"
      Then the action appends with the receipt recorded in its checks

    Scenario: A guardrail block verdict refuses the append
      Given an agent granted social publish under a guardrail obligation
      When the agent publishes with a receipt whose verdict is "block"
      Then the action is refused as obligation unsatisfied
      And the log gains no entry

    Scenario: An action outside the obligation's scope needs no receipt
      Given an agent granted gmail send and social publish under a publish-only guardrail obligation
      When the agent sends a mail without any receipt
      Then the action appends with no checks recorded

    Scenario: A wildcard obligation gates every action of the connector
      Given an agent granted social actions under a guardrail obligation on every social action
      When the agent deletes a post with a receipt whose verdict is "pass"
      Then the action appends with the receipt recorded in its checks
      But deleting a post without any receipt is refused as obligation unsatisfied

    Scenario: No max_age means no time limit — an aged receipt still discharges
      Given an agent granted social publish under a guardrail obligation with no max_age
      When the agent publishes with a pass receipt signed 2 days earlier
      Then the action appends with the receipt recorded in its checks

  Rule: Human approval (Model 1) — a pinned device key signs what was shown

    Scenario: An approved action inside max_age appends
      Given an agent granted social publish requiring human approval within 5 minutes
      When the approver signs the prepared publish 2 minutes before the entry
      Then the action appends with the receipt recorded in its checks

    Scenario: A reject verdict refuses the append
      Given an agent granted social publish requiring human approval within 5 minutes
      When the approver signs the prepared publish with verdict "reject"
      Then the action is refused as obligation unsatisfied

    Scenario: A missing receipt refuses the append
      Given an agent granted social publish requiring human approval within 5 minutes
      When the agent publishes without any receipt
      Then the action is refused as obligation unsatisfied
      And the log gains no entry

    Scenario: A stale receipt is refused — approval does not age
      Given an agent granted social publish requiring human approval within 5 minutes
      When the approver signs the prepared publish 6 minutes before the entry
      Then the action is refused as obligation unsatisfied

    Scenario: A receipt bound to different args is refused
      Given an agent granted social publish requiring human approval within 5 minutes
      When the agent presents an approval receipt bound to other args
      Then the action is refused as obligation unsatisfied

    Scenario: A tampered presented_digest breaks the receipt signature
      Given an agent granted social publish requiring human approval within 5 minutes
      And the approver signed a receipt over what was shown on the device
      When the agent swaps the receipt's presented_digest before appending
      Then the action is refused as obligation unsatisfied

    Scenario: A receipt signed by a non-pinned key is refused
      Given an agent granted social publish requiring human approval within 5 minutes
      When the agent presents an approval receipt signed by a stranger key
      Then the action is refused as obligation unsatisfied

    Scenario: Any key of the pinned attestor set satisfies
      Given an agent granted social publish requiring approval by one of two approvers
      When the second approver signs the prepared publish
      Then the action appends with the receipt recorded in its checks

    Scenario: Freshness is symmetric — a receipt slightly ahead of the entry clock verifies
      Given an agent granted social publish requiring human approval within 5 minutes
      When the approver signs the prepared publish 2 minutes after the entry's clock
      Then the action appends with the receipt recorded in its checks

    Scenario: presented_digest is optional — an approval without it still binds
      Given an agent granted social publish requiring human approval within 5 minutes
      When the approver signs the prepared publish without a presented digest
      Then the action appends with the receipt recorded in its checks

    Scenario: A receipt citing another obligation does not discharge this one
      Given an agent granted social publish requiring human approval within 5 minutes
      When the agent presents an approval receipt citing a different obligation id
      Then the action is refused as obligation unsatisfied

    Scenario: A receipt for another action does not transfer
      Given an agent granted social actions requiring human approval on publish
      When the approver's receipt for a delete is presented on a publish with identical args
      Then the action is refused as obligation unsatisfied

  Rule: counter_sign is the owner instance — one wire shape, no special case

    Scenario: A binding action carrying the owner's co_sign receipt appends
      Given an agent granted gmail send with counter_sign on send
      When the owner co-signs the prepared send and the agent appends it
      Then the action appends with the co_sign receipt recorded in its checks

    Scenario: A binding action without the owner's co-signature is refused
      Given an agent granted gmail send with counter_sign on send
      When the agent sends a mail without any receipt
      Then the action is refused as obligation unsatisfied

    Scenario: A receipt is leaf-bound — it never transfers to a sibling mandate
      Given two sibling sub-mandates that may publish under an ancestor approval obligation
      When the first sibling's approval receipt is presented by the second sibling with identical args
      Then the action is refused as obligation unsatisfied

  Rule: Dual control — a second agent's key as attestor

    Scenario: A four-eyes action appends only with the second agent's receipt
      Given an agent granted social publish under dual control with a second agent
      When the second agent signs the prepared publish
      Then the action appends with the receipt recorded in its checks

  Rule: Obligations conjoin — every covering obligation must discharge

    Scenario: Two obligations on one mandate both gate the action
      Given an agent granted social publish under both a guardrail and a human approval obligation
      When the agent publishes with both receipts
      Then the action appends with both receipts recorded in its checks
      But publishing with only the guardrail receipt is refused as obligation unsatisfied

  Rule: Delegation may add obligations, never drop or alter them

    Scenario: A sub-mandate adding an obligation tightens the gate
      Given a head mandate requiring human approval on publish
      And a sub-mandate that adds a guardrail obligation on publish
      When the sub-agent publishes with both receipts
      Then the action appends with both receipts recorded in its checks

    Scenario: An added obligation alone does not discharge the inherited one
      Given a head mandate requiring human approval on publish
      And a sub-mandate that adds a guardrail obligation on publish
      When the sub-agent publishes with only the guardrail receipt
      Then the action is refused as obligation unsatisfied

    Scenario: A sub-mandate dropping its parent's obligation is refused
      Given a head mandate requiring human approval on publish
      When a sub-mandate is minted with no obligations
      Then the chain is refused at verification time

    Scenario: A sub-mandate altering an inherited obligation is refused
      Given a head mandate requiring human approval on publish
      When a sub-mandate is minted with the same obligation loosened to 1 hour
      Then the chain is refused at verification time

  Rule: R2 binds one obligation discharge to one exact W1 occurrence

    Scenario Outline: R2 has two exact closed tables for optional WYSIWYS evidence
      Given an effective pinned obligation for one W1 operation
      When its R2 receipt has "<presentation state>"
      Then its exact members are "<members>"
      And family is "obligation" and v is the JSON number 2
      And sig verifies over RFC8785-JCS with sig omitted

      Examples:
        | presentation state        | members                                                                  |
        | no presented digest       | v,family,operation_ref,obligation,verdict,at,sig                          |
        | a strict presented digest | v,family,operation_ref,obligation,verdict,presented_digest,at,sig         |

    Scenario: R2 uses operation_ref instead of duplicating historical v1 facts
      Given one canonical operation whose authority, native facts and time are fixed
      When a pinned attestor signs its R2 obligation receipt
      Then operation_ref binds the leaf mandate, operation arguments and occurrence
      And the receipt carries no mandate_id, action or args_hash duplicate
      But a missing, stale, replayed, mismatched, duplicate or non-closed receipt is GammaObligationUnsatisfied

  Rule: Draft3 adds one closed matcher for non-action obligations

    Scenario Outline: The matcher selects one exact reconstructed operation tuple
      Given a homogeneous draft3 chain with applies_to_operation "<matcher>"
      When the grantee presents canonical operation "<operation>"
      Then matcher applicability is "<verdict>"
      And no caller-supplied fact or wildcard participates

      Examples:
        | matcher                                | operation                  | verdict        |
        | read ethos                             | public content read        | applicable     |
        | mutation ethos edit                    | public content edit        | applicable     |
        | mutation structure move                | structural move            | applicable     |
        | inference                              | inference                  | applicable     |
        | grant                                  | sub-grant                  | applicable     |
        | revoke                                 | revocation                 | applicable     |
        | rotate vault                           | connector vault rotation   | applicable     |
        | publication normal                     | normal publication         | applicable     |
        | mutation ethos edit                    | public content delete      | non-applicable |

    Scenario: The draft3 matcher cannot reinterpret historical authority
      Given byte-identical draft1 and draft2 obligation mandates
      When applies_to_operation is presented through a sidecar or mixed-version chain
      Then the matcher is refused as InvalidMandate
      And draft3 requires exactly one selector per obligation
      And migration reissues the complete homogeneous chain

  Rule: An explicitly targeted obligation gates every delegated consumption class

    @wip
    Scenario Outline: A delegated operation commits only with its bound receipt
      Given a mandate with an obligation explicitly targeting "<operation>"
      When the grantee presents "<receipt state>" for that canonical operation
      Then the operation is "<verdict>"
      And any accepted receipt is bound to the leaf mandate, operation arguments and time

      Examples:
        | operation             | receipt state                    | verdict  |
        | public content edit   | valid pinned-attestor receipt    | accepted |
        | public content edit   | no receipt                       | refused  |
        | structural move       | receipt for different arguments | refused  |
        | normal publication    | valid owner co_sign receipt      | accepted |
        | normal publication    | stale owner co_sign receipt      | refused  |
        | connector action      | replayed sibling receipt         | refused  |

    @wip
    Scenario: A co-signed delegated publication still has one grantee actor
      Given a grantee publication explicitly requiring owner co_sign
      When the owner supplies the bound approval receipt
      Then the grantee remains the sole edition actor and signer
      And the owner appears only as the receipt attestor
      And every change remains covered by the grantee's single chain

  Rule: Executor facts need public evidence for keyless acceptance

    @wip
    Scenario Outline: A required tier-X truth cannot be asserted by the grantee
      Given a delegated publication whose operation requires "<executor fact>"
      When the public edition carries "<public evidence>"
      Then keyless cold verification is "<verdict>"

      Examples:
        | executor fact       | public evidence                      | verdict  |
        | action_params       | approved bound attestation           | accepted |
        | action_params       | grantee assertion only               | refused  |
        | spend_cap           | no acceptable public attestation     | refused  |
        | disclose_agency     | approved bound attestation           | accepted |

    @wip
    Scenario: Receipt evaluation is identical before append and after export
      Given a delegated operation with a complete ordered receipt set
      When it is evaluated before effect and replayed from a fresh keyless store
      Then both verdicts accept the same receipts and reject the same replays
      And sealed operation data is never exposed to the keyless verifier

Feature: Restricted mandates — the equipped Ethos becomes a ceiling of authority
  owner-enroll-server equips ONE agent key with everything the
  manifests grant. The product surface needs the rest of the story:
  issuing several RESTRICTED mandates from the same equipped Ethos.
  Each issued mandate is a fresh root certificate signed by the
  owner, validated at issuance against the ceiling — Ethos policy
  intersected with the approved manifests (decision M0.a,
  2026-07-16). A mandate is a subtractive, immutable view: a subset
  of the granted tools, a subset of the zones and folders, the
  manifest bounds inherited untouched, constraints only ever
  tightening. One keypair carries at most ONE active mandate per
  Ethos (decision M0.b): zero chain ambiguity at runtime. Several
  mandates may live on one context — verifiable and revocable
  offline; ONE runner stays active per context until RemoteStore
  (decision M0.d, a documented limit — delegate acts below are
  protocol-level, exercised at the store). The owner surface is
  owner-issue-mandate, owner-revoke-mandate and owner-preview-mandate
  (decision M0.f); the preview JSON and the runtime verdict come out
  of the SAME pure function — the preview IS the decision.

  Rule: Issuance only ever narrows — tools, zones, folders

    @wip
    Scenario: A restricted mandate covers a subset of the granted tools
      Given server "gmail" is enrolled with "search_emails" as a granted read and "send_email" as a granted write
      When the owner issues a mandate labeled "prospection" to a fresh delegate key restricted to tool "search_emails"
      Then the issued certificate covers "gmail__search_emails" and not "gmail__send_email"
      And the issuance is journalized as a grant in the context gamma
      And the delegate's chain verifies offline

    @wip
    Scenario: A tool the manifest denies is refused by name at issuance
      Given server "gmail" is enrolled with "search_emails" as a granted read and "send_email" as a denied write
      When the owner issues a mandate to a fresh delegate key restricted to tool "send_email"
      Then the issuance is refused naming "send_email" and its denied manifest decision
      And no certificate is written
      And no grant is journalized

    @wip
    Scenario: A restricted mandate narrows the folders it can read
      Given the "ventes" context circle zone holds folders "prospects" and "interne"
      When the owner issues a mandate to a fresh delegate key restricted to circle folder "prospects" and tool "search_emails"
      Then the delegate reads under "prospects" with its own key
      But nothing under "interne" ever reaches the delegate

    @wip
    Scenario: The full-Ethos preset is a snapshot, never a subscription
      Given server "gmail" is enrolled with "search_emails" as a granted read
      And the owner issues a full-Ethos mandate labeled "majordome" to a fresh delegate key
      When server "calendar" is enrolled afterwards with "create_event" as a granted write
      Then the "majordome" certificate still covers exactly the rights of its issuance day
      And a "calendar__create_event" act under "majordome" is rejected offline

    @wip
    Scenario: A mandate restricted to one precise section awaits the id= selector
      Given the "ventes" context circle zone holds the directive "Toujours vouvoyer les prospects." and a folder "dossiers"
      When the owner issues a mandate to a fresh delegate key restricted to the directive section only
      Then the issued certificate carries an id= selector naming that section
      And the delegate reads that section and nothing else of the zone

  Rule: Manifest bounds are inherited — a mandate tightens, never edits

    @wip
    Scenario: Widening an inherited bound is refused at issuance
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example"
      When the owner issues a mandate restricted to tool "send_email" widening the bound on "to" with "mallory@evil.example"
      Then the issuance is refused naming field "to" and the widening value
      And no certificate is written

    @wip
    Scenario: An added constraint conjoins with the inherited bound
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example" and "prospect-b@clients.example"
      When the owner issues a mandate restricted to tool "send_email" constrained to recipient "prospect-a@clients.example"
      Then the preview of the issued mandate shows the manifest bound and the tighter constraint conjoined
      And its effective recipients are exactly "prospect-a@clients.example"

  Rule: One keypair, one active mandate per Ethos

    @wip
    Scenario: A second active mandate to an equipped keypair is refused
      Given a mandate labeled "prospection" issued to a delegate key on the "ventes" context
      When the owner issues another mandate to the same delegate key on the same context
      Then the issuance is refused naming the active mandate "prospection"

    @wip
    Scenario: Revoking the active mandate frees the keypair
      Given a mandate labeled "prospection" issued to a delegate key on the "ventes" context
      When the owner revokes mandate "prospection" citing reason "mission over"
      And the owner issues a mandate labeled "reporting" to the same delegate key
      Then the issuance passes
      And "reporting" is the only active mandate of that key

  Rule: Several delegates on one context — attribution per key, revocation per mandate

    @wip
    Scenario: Two delegates act under their own keys and their own mandates
      Given mandates "prospection" and "reporting" issued to two distinct delegate keys on the "ventes" context
      When each delegate appends one act under its own key
      Then each act is signed by its own delegate key
      And each act cites its own mandate in authorized_via — the two chains are disjoint
      And both acts verify offline from the files alone

    @wip
    Scenario: A targeted revocation stops one delegate and spares the other
      Given mandates "prospection" and "reporting" issued to two distinct delegate keys on the "ventes" context
      When the owner revokes mandate "prospection" citing reason "compromised laptop"
      Then the revocation is journalized in the context gamma with its reason
      And offline verification now rejects any new act under "prospection"
      But an act under "reporting" still verifies offline

  Rule: The vault line never travels with an act right

    @wip
    Scenario: An issued act mandate leaves the credential in gateway custody
      Given server "gmail" is enrolled with a vault credential and "send_email" as a granted write
      When the owner issues a mandate to a fresh delegate key restricted to tool "send_email"
      Then no vault header line of "gmail" is delivered to the delegate
      And the certificate carries the act right and no config right

  Rule: The preview is the decision — one function, two callers

    @wip
    Scenario: owner-preview-mandate prints the effective policy as stable JSON
      Given server "gmail" is enrolled with "search_emails" as a granted read and "send_email" as a granted write with a one_of bound on "to"
      When the owner previews the agent mandate
      Then the preview JSON names exactly the granted tools, each with its inherited bounds
      And the preview names the validity window and the status "active"

    @wip
    Scenario: The preview verdict and the runtime verdict are the same function's answer
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example"
      When the owner previews a call of "gmail__send_email" to "mallory@evil.example"
      Then the preview verdict is a refusal naming field "to" and the approved set
      And the running gateway refuses the same call with the same verdict

  Rule: The read-model tells the truth about a mandate's lifecycle

    @wip
    Scenario: The preview reports active, expired and revoked with the exact reason
      Given a mandate labeled "prospection" issued for 7 days on the "ventes" context
      Then the preview at day 1 reports status "active"
      And the preview at day 8 reports status "expired"
      And after owner revocation the preview reports status "revoked" with its reason

    @wip
    Scenario: Remaining uses are counted from the gamma alone
      Given a mandate labeled "prospection" carrying max_actions 3
      And its delegate has appended two acts
      When the owner previews mandate "prospection"
      Then the preview reports one remaining use, counted from the log

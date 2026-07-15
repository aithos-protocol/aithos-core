Feature: Granting writes — risk class and grant decision are two owner gestures
  Hub v1 served only read-class tools: "write" meant known-but-refused.
  The demo needs Léa to SEND, so an approval now carries two facts per
  tool: its risk class (read or write — what kind of power it is) and
  the grant decision (granted or denied — whether THIS agent may use
  it). Defaults keep the historic safe semantics: an approval naming
  only a class grants reads and denies writes. A granted write joins
  tools/list with its pinned schema, is covered by the mandate and its
  acts are logged like any other; an ungranted tool of either class
  stays hidden and precisely refused. Flipping a decision is a
  re-enrollment: new mandate, political revocation of the old one.

  Rule: A write is granted explicitly, never by default

    @wip
    Scenario: An explicitly granted write is listed, relayed and logged
      Given server "gmail" advertises tools "search_emails" and "send_email"
      And the owner enrolls "gmail" approving "search_emails" as a granted read and "send_email" as a granted write
      When the agent lists the tools and calls "gmail__send_email" through the hub
      Then the list includes "gmail__send_email" with its pinned description and input schema
      And the call reaches the upstream under raw name "send_email"
      And the act is logged in the granting context gamma with one journal cross-reference

    @wip
    Scenario: An approval naming only classes keeps the safe defaults
      Given server "gmail" advertises tools "search_emails" and "send_email"
      When the owner enrolls "gmail" approving only classes "search_emails=read" and "send_email=write"
      Then "gmail__search_emails" is listed and served
      And "gmail__send_email" is hidden and precisely refused with zero upstream contact

    @wip
    Scenario: A read can be known but denied too
      Given server "gmail" advertises tools "search_emails" and "send_email"
      And the owner enrolls "gmail" approving "search_emails" as a denied read and "send_email" as a granted write
      When the agent lists the tools and calls "gmail__search_emails" through the hub
      Then the list does not include "gmail__search_emails"
      And the refusal names "gmail__search_emails"
      And no request reaches the upstream

  Rule: The decision is on the record and enforced end to end

    @wip
    Scenario: The grant log records both the class and the decision
      Given the owner enrolls "gmail" approving "send_email" as a granted write
      Then the granting context gamma grant entry names "send_email" as a granted "write"
      And the sealed manifest records the decision next to the risk class

    @wip
    Scenario: Config and manifest must agree on the grant decision
      Given the owner enrolls "gmail" approving "send_email" as a denied write
      When a runtime config declares "gmail__send_email" as granted
      Then the gateway refuses to open, naming the grant mismatch

    @wip
    Scenario: Revoking a write grant is a re-enrollment under a new mandate
      Given server "gmail" is enrolled with "send_email" as a granted write
      And the agent has called "gmail__send_email" through the hub once
      When the owner re-enrolls "gmail" with "send_email" denied for the same agent key
      Then a new mandate excludes the write and the old mandate is politically revoked
      And the next call to "gmail__send_email" is refused and never reaches the upstream
      And tools/list no longer includes "gmail__send_email"

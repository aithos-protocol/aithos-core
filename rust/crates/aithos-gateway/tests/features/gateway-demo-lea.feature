Feature: Demo Léa — one bounded, briefed sales agent over three governed connectors
  The dress rehearsal of the Innoestate demo (docs/DEMO-LEA-SCENARIO.md),
  with no LLM: the harness sends exactly the JSON-RPC a real agent will
  send on demo day. One Ethos "ventes"; three upstream servers behind
  one endpoint — notion (read-only), gmail (bounded writes), calendar
  (slotted writes); every upstream bearer lives in the enterprise
  vault; the owner's directives live in the circle zone. The data may
  offer five prospects — the mandate allows three, and the mandate
  wins, out loud, on the record.

  Every upstream here is deliberately permissive: three separate
  endpoints, three full-power tokens, servers that would execute
  anything that reaches them. Whatever restriction this story shows
  is the gateway's alone — which is exactly the product.

  Background:
    Given the Innoestate demo world is provisioned:
      | server   | tool           | class | decision | bounds                                              |
      | notion   | query_database | read  | granted  |                                                     |
      | notion   | create_page    | write | denied   |                                                     |
      | gmail    | search_emails  | read  | granted  |                                                     |
      | gmail    | send_email     | write | granted  | to one_of {a,b,c}; bcc forbid; to max 3; subject require |
      | gmail    | delete_email   | write | denied   |                                                     |
      | calendar | list_events    | read  | granted  |                                                     |
      | calendar | create_event   | write | granted  | start slots tue,thu 14:00-18:00                     |
    And the vault stores one distinct token per server
    And the notion database holds prospects "a", "b", "c", "d" and "e"
    And the "ventes" circle zone directs "Tout mail de prise de rendez-vous mentionne le DPE du bien et propose d'abord une visite virtuelle."
    And the "ventes" self zone holds an owner-only note

  @wip
  Scenario: Beat 1 — the agent surface is exactly the granted, briefed world
    When the agent initializes and lists the tools
    Then the initialize result recommends "briefing.read" before outbound actions
    And the list is exactly the granted tools, "briefing.read" and the journal tools
    And the list includes "gmail__send_email" and "calendar__create_event"
    And the list does not include "gmail__delete_email"
    And the list does not include "notion__create_page"
    And the vault received zero requests
    And the upstream received zero requests

  @wip
  Scenario: Beat 2 — the prospect list comes from notion under the read grant
    When the agent calls "notion__query_database" through the hub
    Then the answer carries the five prospects
    And the "notion" upstream saw only its own vault bearer
    And the act is logged in the "ventes" gamma with one journal cross-reference

  @wip
  Scenario: Beat 3 — sending to everyone is refused and teaches the approved three
    When the agent sends a meeting email to all five prospects
    Then the call is refused as a bound violation
    And the refusal names field "to", prospects "d" and "e" and the approved set
    And the gmail vault path received zero requests
    And the "gmail" upstream received zero requests
    And the "ventes" gamma and the journal each gain one "bound_violated" refusal

  @wip
  Scenario: Beat 4 — the corrected send passes and the wire carries the vault bearer
    When the agent sends a meeting email to prospects "a", "b" and "c"
    Then the call succeeds
    And the "gmail" upstream saw exactly one call under raw name "send_email" bearing its vault token
    And the act is logged in the "ventes" gamma with one journal cross-reference

  @wip
  Scenario: Beat 5 — the visit lands inside the approved slots only
    When the agent books a visit starting "2026-07-15T10:00:00+02:00"
    Then the call is refused as a bound violation naming the approved slots
    When the agent books a visit starting "2026-07-16T15:00:00+02:00"
    Then the call succeeds
    And the "calendar" upstream saw exactly one call under raw name "create_event" bearing its vault token

  @wip
  Scenario: Beat 6 — the briefing steers before action and its read is on the record
    When the agent calls "briefing.read"
    Then the answer carries the DPE directive verbatim
    And no agent-facing response contains the owner-only note
    And the "ventes" gamma gains one briefing read entry

  @wip
  Scenario: Beat 7 — a circle edit changes the character before the next read
    Given the agent has read the briefing once
    When the owner appends "Joindre le lien du dossier de visite." to the circle directive
    And the agent calls "briefing.read" again
    Then the answer carries the appended directive verbatim
    And both reads are journalized in the "ventes" gamma

  @wip
  Scenario: Beat 8 — the auditor replays the whole story from the gamma
    Given the agent has walked beats 2 through 7
    When the auditor exports the "ventes" context with the auditor mandate
    Then the export carries the notion act, the gmail act and the calendar act
    And the export carries the "bound_violated" refusals naming "to" and "start"
    And the export carries the briefing read entries
    And no file of any Ethos store contains any vault token or upstream secret
    And the gateway config text contains references only

Feature: Argument bounds — the owner approves not just the tool but its reach
  Granting send_email says WHAT the agent may do; bounds say ON WHAT.
  At enrollment the owner attaches argument rules to each granted
  tool: a value whitelist (one_of — recipients, or the sub-actions of
  a polymorphic tool), time slots, a forbidden or required field, a
  size cap. The rules live in the sealed approved manifest — never in
  the runtime YAML — and changing them is a re-enrollment. At runtime
  the check runs after the mandate said yes and before the act is
  logged: a violation refuses the WHOLE call (the gateway never
  silently rewrites), and the refusal names the field, the offending
  values and the approved rule — it teaches the agent its perimeter,
  which is no secret: it is exactly what the owner granted. A refused
  call wakes neither the vault nor the upstream.

  Rule: one_of bounds every value of a field, array elements included

    @wip
    Scenario: A send inside the recipient whitelist passes untouched
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example", "prospect-b@clients.example" and "prospect-c@clients.example"
      When the agent calls "gmail__send_email" with recipients "prospect-a@clients.example" and "prospect-c@clients.example"
      Then the call reaches the upstream with its arguments unmodified
      And the act is logged in the granting context gamma

    @wip
    Scenario: One intruder recipient refuses the whole call and teaches the approved set
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example", "prospect-b@clients.example" and "prospect-c@clients.example"
      When the agent calls "gmail__send_email" with recipients including "prospect-d@clients.example" and "prospect-e@clients.example"
      Then the call is refused as a bound violation
      And the refusal names field "to", the offending values and the approved set
      And the vault received zero requests
      And the upstream received zero requests
      And the context gamma and the journal each gain one "bound_violated" refusal

    @wip
    Scenario: A polymorphic tool is bounded to a subset of its own actions
      Given tool "repo_admin" is granted write with a one_of bound on "action" allowing "comment"
      When the agent calls "github__repo_admin" with action "merge"
      Then the call is refused as a bound violation
      And the refusal names field "action", value "merge" and the allowed actions
      And the upstream received zero requests

  Rule: Time slots are evaluated on the datetime's own clock face

    @wip
    Scenario: A visit inside the approved slots passes
      Given tool "create_event" is granted write with time slots "tuesday" and "thursday" from "14:00" to "18:00" on field "start"
      When the agent calls "calendar__create_event" starting "2026-07-16T15:00:00+02:00"
      Then the call reaches the upstream with its arguments unmodified

    @wip
    Scenario: A visit outside the slots is refused and the slots are named
      Given tool "create_event" is granted write with time slots "tuesday" and "thursday" from "14:00" to "18:00" on field "start"
      When the agent calls "calendar__create_event" starting "2026-07-15T10:00:00+02:00"
      Then the call is refused as a bound violation
      And the refusal names field "start", the offending instant and the approved slots
      And the upstream received zero requests

  Rule: Presence, size and shape rules fail closed

    @wip
    Scenario Outline: forbid, require and max_items refuse precisely
      Given tool "send_email" is granted write with bound "<bound>"
      When the agent calls "gmail__send_email" with arguments "<arguments>"
      Then the call is refused as a bound violation
      And the refusal names "<named>"
      And the upstream received zero requests

      Examples:
        | bound              | arguments                          | named                     |
        | forbid bcc         | a bcc field                        | forbidden field `bcc`     |
        | require subject    | no subject field                   | required field `subject`  |
        | to max_items 3     | four whitelisted recipients        | at most 3 items on `to`   |

    @wip
    Scenario: A mistyped argument refuses instead of guessing
      Given tool "send_email" is granted write with a one_of bound on "to" allowing "prospect-a@clients.example"
      When the agent calls "gmail__send_email" with "to" as a single string instead of an array
      Then the call is refused as a bound violation naming the expected shape of "to"
      And the upstream received zero requests

    @wip
    Scenario: A one_of bound on an absent optional field lets the call pass
      Given tool "send_email" is granted write with a one_of bound on "cc" allowing "assistant@innoestate.example"
      When the agent calls "gmail__send_email" without any "cc" field
      Then the call reaches the upstream with its arguments unmodified

  Rule: Bounds are sealed owner policy, not runtime configuration

    @wip
    Scenario: Bounds live in the sealed manifest and nowhere in the YAML
      Given tool "send_email" is granted write with a one_of bound on "to"
      Then the runtime config text declares no bound at all
      And the sealed manifest of the granting context records the bound
      And the upstream pin hash is unchanged by the bound

    @wip
    Scenario: Tightening a bound is a re-enrollment under a new mandate
      Given tool "send_email" is granted write with a one_of bound on "to" allowing three prospects
      And the agent has sent to "prospect-c@clients.example" once
      When the owner re-enrolls "gmail" narrowing the bound to "prospect-a@clients.example" for the same agent key
      Then the old mandate is politically revoked
      And a call to "prospect-c@clients.example" is now refused as a bound violation
      And a call to "prospect-a@clients.example" still passes

    @wip
    Scenario: Bounds on an ungranted tool are rejected at approval
      When the owner approves "delete_email" as a denied write carrying a one_of bound
      Then the approval is rejected naming the ungranted bound

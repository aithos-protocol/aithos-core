Feature: Context briefing — the owner's directives served before action
  An Ethos carries layered directives: the public and circle zones
  hold what the owner wants every mandated agent to know before it
  acts. The hub serves them itself, as the native tool briefing.read,
  and tells the agent to look FIRST — through the initialize
  instructions and the tool description. The surface is conditional:
  directives exist somewhere granted → the tool is listed and the
  instructions point at it; every granted zone is empty and nothing
  is agent-writable → the surface stays completely mute. The self
  zone never reaches the agent. Every read is journalized, and an
  owner edit is served on the very next read — the character changes
  without touching the model, its prompt or its provider.

  Rule: The surface appears only when there is something to say

    @wip
    Scenario: Circle directives expose briefing.read and the initialize instructions
      Given the "ventes" context circle zone holds the directive "Tout mail de prise de rendez-vous mentionne le DPE du bien."
      When the agent initializes and lists the tools
      Then the initialize result carries instructions recommending "briefing.read" before outbound actions
      And the list includes "briefing.read" with a description that says to consult it before acting

    @wip
    Scenario: An empty briefing keeps the surface mute
      Given no granted zone of any context holds a directive
      And the agent has no write right on any briefing zone
      When the agent initializes and lists the tools
      Then the initialize result carries no instructions
      And the list does not include "briefing.read"

  Rule: Reads are exact, layered and journalized

    @wip
    Scenario: briefing.read serves the owner's exact text, labeled by context
      Given the "ventes" context public zone holds the directive "Toujours vouvoyer les prospects."
      And the "ventes" context circle zone holds the directive "Tout mail de prise de rendez-vous mentionne le DPE du bien."
      When the agent calls "briefing.read"
      Then the answer carries both directives verbatim under the "ventes" label
      And the answer names the zone of each directive

    @wip
    Scenario: The self zone never reaches the agent
      Given the "ventes" context self zone holds the note "Marge de négociation max 8%."
      And the "ventes" context circle zone holds the directive "Toujours vouvoyer les prospects."
      When the agent calls "briefing.read"
      Then the answer carries the circle directive
      And no agent-facing response contains "Marge de négociation max 8%."

    @wip
    Scenario: Every briefing read is journalized as a read entry
      Given the "ventes" context circle zone holds a directive
      When the agent calls "briefing.read" twice
      Then the "ventes" context gamma gains exactly two read entries for the briefing
      And each entry is covered by the agent's read mandate

    @wip
    Scenario: An owner edit is served on the very next read without restart
      Given the "ventes" context circle zone holds the directive "Proposer d'abord une visite virtuelle."
      And the agent has read the briefing once
      When the owner rewrites the directive to "Proposer d'abord une visite virtuelle et joindre le dossier de visite."
      And the agent calls "briefing.read" again
      Then the answer carries the rewritten directive verbatim
      And the previous wording appears nowhere in the answer

  Rule: The briefing name belongs to the platform

    @wip
    Scenario: The briefing prefix is reserved in every tool map and server name
      When a hub config declares a server or a tool named under the "briefing" prefix
      Then the config is rejected naming the reserved prefix

    @wip
    Scenario: Unknown briefing arguments fail closed
      Given the "ventes" context circle zone holds a directive
      When the agent calls "briefing.read" with an unknown argument field
      Then the call is refused naming the unknown field
      And the refusal is journalized

Feature: Native Ethos reading — the mandate lights the surface
  The gap the demo found (GAPS beat 6a): no MCP way to read the
  sections of an Ethos. Three native tools close it — ethos.read (one
  covered section body), ethos.list (the covered skeleton), and
  ethos.context (the starting pack). Decisions Mathieu (AskUserQuestion
  + clarification, 2026-07-16): the surface is DERIVED, never toggled —
  the gateway recomputes it per call from EVERY valid chain to the
  agent key found in the context store, whatever gesture minted it
  (owner CLI today, the multi-mandate surface G8.c tomorrow, a
  delegate's sub-mandate), intersected with what the delivered lines
  physically open and with what the zones actually hold; a revocation
  drops the surface on the next call, hot, no restart. Zone by zone:
  public is the readability frontier (§02.1) — clear by design,
  world-readable, so ANY connected session (its connection presupposes
  a mandate) is informed of non-empty public content and reads it
  keyless, with zero gamma cost; circle serves only under a chain
  covering read.circle (or a #dir subtree of it) whose lines open the
  bodies, and EVERY opened body is one ethos.read entry under the
  chain that read (an unjournalizable read fails the whole call — the
  C2 precedent); self is NEVER served by default (GAPS §4.2) — an
  explicit read.self grant is the graved principle, but the bundle has
  no delegated self resolution yet (self_resolve is owner-only), so
  the self-serving scenarios stay @wip pending their own core lot
  (vectors-first) and the v1 gesture refuses self naming exactly that.
  owner-grant-ethos-read is the MINIMAL v1 emission path — a mandate
  mint plus zone lines (circle line to the agent AND to the context
  auditor, the lot-K implication assumed), never a tool toggle; the
  authority stays INJECTABLE (a chain parameter — pre-G5 the chains to
  the agent key; G5 will inject session chains). ethos.context
  composes the briefing pack (its lot-K record preserved: circle
  directives read on the record), the public bodies (clear), and the
  covered sealed-zone index (titles and paths, no body, no entry).
  Refusals are pedagogical — they name the missing perimeter, never a
  section body — and route per §3bis.8: the journal always, the
  context too when the call names one. The ethos prefix joins journal
  and briefing in every reservation (tool maps, mono and multi, and
  hub server names). Delegated writes (ethos.write) are NOT this lot.

  Rule: The surface derives from mandates, lines and content — never from a toggle

    Scenario: Non-empty public content alone lights the surface for any connected session
      Given the "ventes" context public zone holds the section "presentation" with text "Innoestate, conseil immobilier."
      And no mandate covers any sealed zone
      When the agent initializes and lists the tools
      Then the list includes "ethos.read" and "ethos.list" and "ethos.context"
      And the ethos tool descriptions name public access on "ventes"
      And the ethos tool descriptions name no other zone

    Scenario: An empty Ethos keeps the ethos surface mute
      Given no granted zone of any context holds a directive
      And every zone of every context is empty
      When the agent initializes and lists the tools
      Then the list does not include "ethos.read"
      And the list does not include "ethos.list"
      And the list does not include "ethos.context"
      And the initialize result carries no instructions

    Scenario: A read.circle mandate lights circle on the very next call
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And the agent lists the tools once
      When the owner grants ethos read on zones "circle" for the "ventes" context
      And the agent lists the tools again
      Then the ethos tool descriptions name circle access on "ventes"
      And no restart happened

    Scenario: Revoking the read mandate drops the circle surface hot
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And the owner granted ethos read on zones "circle" for the "ventes" context
      When the owner revokes the ethos-read mandate of the "ventes" context
      And the agent lists the tools again
      Then the ethos tool descriptions no longer name circle
      And a subsequent circle read is refused naming the revoked chain

    Scenario: A sub-mandate minted by a delegate lights the surface exactly the same
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And a delegate holding an issue mandate mints a read.circle sub-mandate to the agent key
      When the agent lists the tools
      Then the ethos tool descriptions name circle access on "ventes"
      And a circle read under that chain names the full chain in its entry

  Rule: ethos.list serves the covered skeleton only, and costs nothing

    Scenario: The listed tree carries public and covered circle rows, never self
      Given the "ventes" context public zone holds the section "presentation" with text "Innoestate, conseil immobilier."
      And the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And the "ventes" context self zone holds the note "Marge de négociation max 8%."
      And the owner granted ethos read on zones "circle" for the "ventes" context
      When the agent calls "ethos.list"
      Then the tree names the public section "presentation"
      And the tree names the circle section "memoire/prospects" with its title and no body
      And no self row, sid or title appears in any agent-facing response
      And no gamma entry was written by the listing

    Scenario: Without read.circle the circle skeleton stays invisible
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And no mandate covers the circle zone
      When the agent calls "ethos.list"
      Then the tree carries no circle row
      And the circle section title appears in no agent-facing response

  Rule: ethos.read opens covered bodies on the record

    Scenario: A covered circle read serves the exact text and journalizes one read
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And the owner granted ethos read on zones "circle" for the "ventes" context
      When the agent calls "ethos.read" on zone "circle" path "memoire/prospects" of context "ventes"
      Then the answer carries "Liste des prospects qualifiés." verbatim
      And the "ventes" context gamma gains exactly one ethos.read entry
      And that entry names the granting chain in authorized_via

    Scenario: A public read is keyless and unjournalized — the readability frontier
      Given the "ventes" context public zone holds the section "presentation" with text "Innoestate, conseil immobilier."
      When the agent calls "ethos.read" on zone "public" path "presentation" of context "ventes"
      Then the answer carries "Innoestate, conseil immobilier." verbatim
      And no gamma entry was written by the read

    Scenario: An uncovered circle read is refused naming the missing perimeter
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And no mandate covers the circle zone
      When the agent calls "ethos.read" on zone "circle" path "memoire/prospects" of context "ventes"
      Then the call is refused naming the missing "read.circle" perimeter
      And no section text leaks in the refusal
      And the refusal is journalized
      And the "ventes" context gamma records the refusal too

    Scenario: A self read is refused by default even when self content exists
      Given the "ventes" context self zone holds the note "Marge de négociation max 8%."
      When the agent calls "ethos.read" on zone "self" path "notes/marge" of context "ventes"
      Then the call is refused naming the missing "read.self" perimeter
      And no agent-facing response contains "Marge de négociation max 8%."

    @wip
    Scenario: An explicitly granted self read serves and journalizes like circle — pending the core self-resolution lot
      Given the "ventes" context self zone holds the note "Marge de négociation max 8%."
      And the owner granted ethos read on zones "self" for the "ventes" context
      When the agent calls "ethos.read" on zone "self" path "notes/marge" of context "ventes"
      Then the answer carries "Marge de négociation max 8%." verbatim
      And the "ventes" context gamma gains exactly one ethos.read entry

    Scenario: The v1 gesture refuses self while delegated self resolution is absent
      Given an equipped "ventes" context
      When the owner grants ethos read on zones "self" for the "ventes" context
      Then the gesture is refused naming the pending delegated self resolution
      And no certificate is written

  Rule: ethos.context serves the map, not the vault

    Scenario: The starting pack composes briefing, public bodies and the sealed index
      Given the "ventes" context circle zone holds the directive "Tout mail mentionne le DPE du bien."
      And the "ventes" context public zone holds the section "presentation" with text "Innoestate, conseil immobilier."
      And the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And the owner granted ethos read on zones "circle" for the "ventes" context
      When the agent calls "ethos.context"
      Then the pack carries the directive verbatim
      And the pack carries "Innoestate, conseil immobilier." verbatim
      And the pack names the circle section "memoire/prospects" without its body
      And the only new gamma entries are the briefing directive reads

    Scenario: The pack never exceeds the covered perimeter
      Given the "ventes" context circle zone holds the section "memoire/prospects" with text "Liste des prospects qualifiés."
      And no mandate covers the circle zone
      When the agent calls "ethos.context"
      Then the pack carries no circle row
      And the circle section title appears in no agent-facing response

  Rule: The ethos name belongs to the platform

    Scenario: The ethos prefix is reserved in every tool map and server name
      When a hub config declares a server or a tool named under the "ethos" prefix
      Then the config is rejected naming the reserved prefix

    Scenario: Unknown ethos arguments fail closed
      Given the "ventes" context public zone holds the section "presentation" with text "Innoestate, conseil immobilier."
      When the agent calls "ethos.read" with an unknown argument field
      Then the call is refused naming the unknown field
      And the refusal is journalized

Feature: Tunnel registration — the signed line at the relay door
  Lot P6 (relay, contrat C2): the relay verifies the pod's registration
  line in the exact normative order of INFRA-PROVIDER annexe B.2 —
  fail-closed, the first failing check answers and closes the connection.
  The relay authenticates by a signed line, never by mTLS (zero new
  secret: the gateway key already exists), and it NEVER reads an
  application byte (A3). The control-plane mapping it checks against is the
  minimal slice of P7 (gateway_pub ↔ tenant ↔ hostname ↔ suspended).

  Background:
    Given the control plane binds gateway "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1" to tenant "acme" and hostname "demo.mcp.aithos.fr"

  Rule: Step 0 — form: closed field set, known version

    Scenario: A registration with an unknown field is rejected
      When a registration arrives carrying an extra field "debug"
      Then the registration is refused with "envelope_invalid"

    Scenario: A registration whose bytes are not canonical JCS is rejected
      When a registration arrives re-encoded with spaces between JSON tokens
      Then the registration is refused with "envelope_invalid"

    Scenario: An unknown wire version is rejected
      When a registration arrives claiming version "2.0.0-draft.1"
      Then the registration is refused with "envelope_invalid"

  Rule: Step 1 — the clock window is ±300 s, inclusive

    Scenario: The 300 s boundary is accepted
      Given the relay clock reads "2026-07-16T12:05:00Z"
      When a well-formed registration signed at "2026-07-16T12:00:00Z" arrives
      Then the registration is accepted

    Scenario: 301 s of skew answers clock_skew
      Given the relay clock reads "2026-07-16T12:05:01Z"
      When a well-formed registration signed at "2026-07-16T12:00:00Z" arrives
      Then the registration is refused with "clock_skew"

  Rule: Step 2 — a nonce burns on first sight, before the signature

    Scenario: The same registration presented twice answers nonce_replayed
      When a valid registration is presented twice
      Then the second registration is refused with "nonce_replayed"

  Rule: Step 3 — the signature verifies under gateway_pub

    Scenario: A corrupted signature answers signature_invalid
      When a registration arrives with its signature corrupted
      Then the registration is refused with "signature_invalid"

  Rule: Step 4 — the control-plane mapping is exact and not suspended

    Scenario: A hostname not bound to this gateway answers mapping_mismatch
      When a valid registration claims hostname "other.mcp.aithos.fr"
      Then the registration is refused with "mapping_mismatch"

    Scenario: A gateway the control plane never enrolled answers mapping_mismatch
      When a valid registration is signed by an unmapped gateway key
      Then the registration is refused with "mapping_mismatch"

    Scenario: A suspended tenant is refused registration
      Given the control-plane binding is suspended
      When a valid registration arrives
      Then the registration is refused with "suspended"

  Rule: The happy path opens the mux

    Scenario: A valid registration is accepted and answered ok
      When a valid registration arrives
      Then the registration is accepted
      And the relay answer is the single line {"aithos-tunnel":"1.0.0-draft.1","ok":true}

  Rule: Anti-flap and bounds (B.2/B.4)

    Scenario: A registration line over 4 KiB is refused before parsing
      When a 5000-byte registration line arrives
      Then the registration is refused with "envelope_invalid"

Feature: Relay passthrough — the SNI-routed blind pipe
  Lot P6 (relay, contrat C2), jalon M2: the public side of annexe B. One
  door behind the NLB (:443). For every inbound TCP connection the relay
  reads the ClientHello WITHOUT terminating TLS — bounded to 16 KiB and to
  the hello deadline (B.4) — then routes by SNI:
  - the relay's own tunnel name → the ONLY TLS the relay terminates (its
    own certificate, never a client's), ALPN "aithos-tunnel/1" required,
    then the B.2 registration line, then the yamux mux (pod = yamux
    server, relay = yamux client, initial window 256 KiB — B.3);
  - a hostname with an active tunnel (exact match, case-insensitive) →
    one yamux stream, the TCP bytes piped from the FIRST byte
    (ClientHello included), no preamble, half-close propagated — the pod
    re-reads the SNI and terminates the public TLS itself (the private
    key stays client-side, A3);
  - everything else (no SNI, not TLS, unknown hostname, oversized hello,
    expired deadline) → silent close: not one byte emitted, nothing to
    enumerate.
  Deadlines and intervals are injected by the harness (spec constants in
  the binary — no runtime knob can loosen them); the committed p5 vector
  replays the SNI extraction byte for byte.

  Background:
    Given the control plane binds gateway "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1" to tenant "acme" and hostname "demo.mcp.aithos.fr"
    And a relay listens with a test TLS certificate for tunnel name "relay.test.aithos.fr"

  Rule: The tunnel door is the only TLS the relay terminates — its own name, ALPN aithos-tunnel/1 (B.1/B.2)

    Scenario: A pod dialing the tunnel name with ALPN aithos-tunnel/1 reaches registration
      When a pod opens TLS to the tunnel name offering ALPN "aithos-tunnel/1" and registers
      Then the registration is accepted and the tunnel is active for "demo.mcp.aithos.fr"

    Scenario: A pod handshake offering no ALPN is refused at the TLS layer
      When a pod opens TLS to the tunnel name offering no ALPN
      Then the TLS handshake fails and no registration line is ever read

    Scenario: A pod handshake offering a foreign ALPN is refused at the TLS layer
      When a pod opens TLS to the tunnel name offering ALPN "h2"
      Then the TLS handshake fails and no registration line is ever read

    Scenario: A refused registration answers its code and the tunnel never activates
      When a pod opens TLS to the tunnel name and registers for hostname "other.mcp.aithos.fr"
      Then the registration is refused with "mapping_mismatch"
      And "other.mcp.aithos.fr" has no active tunnel

  Rule: The public pipe starts at the first byte and is never terminated (B.3)

    Scenario: A public client completes its TLS handshake with the pod, not the relay
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When a public client connects with SNI "demo.mcp.aithos.fr"
      Then the certificate the client sees is the pod's, never the relay's

    Scenario: Bytes cross the pipe byte-exact in both directions
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When a public client sends a request through its TLS session with the pod
      Then the pod receives exactly the bytes the client sent
      And the client receives exactly the bytes the pod answered

    Scenario: Half-close propagates through the pipe
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When the public client closes its write side after the request
      Then the pod observes end-of-stream after the request bytes
      And the client still receives the pod's answer

  Rule: SNI routing is exact, case-insensitive, fail-closed (B.4)

    Scenario: A hostname with no active tunnel closes silently
      When a public client connects with SNI "ghost.mcp.aithos.fr"
      Then the connection closes without one byte emitted

    Scenario: A ClientHello without SNI closes silently
      When a public client connects without SNI
      Then the connection closes without one byte emitted

    Scenario: A non-TLS client closes silently
      When a client sends plain HTTP bytes to the public door
      Then the connection closes without one byte emitted

    Scenario: SNI matching is case-insensitive
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When a public client connects with SNI "DeMo.McP.AiThOs.Fr"
      Then the certificate the client sees is the pod's, never the relay's

    Scenario: A hello exceeding 16 KiB closes before routing
      When a client floods more than 16 KiB without completing a ClientHello
      Then the connection closes without one byte emitted

    Scenario: A hello that never completes is closed at the deadline
      When a client sends half a ClientHello and stalls past the hello deadline
      Then the connection closes without one byte emitted

  Rule: One hostname = one active tunnel — a fresh accept replaces the old (B.2)

    Scenario: A replacement registration sends GoAway to the old mux and reroutes
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When a second pod registers and serves "demo.mcp.aithos.fr"
      Then the first pod's mux is closed by GoAway
      And a new public client is served by the second pod

    Scenario: The sixth registration inside a minute on one hostname is rate-limited
      When the same pod registers 6 times within a minute for "demo.mcp.aithos.fr"
      Then the sixth registration is refused with "rate_limited"
      And the fifth registration was accepted

  Rule: Liveness — a gone pod is detected and unregistered (B.3)

    Scenario: A pod that drops its tunnel connection is unregistered
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When the pod drops its tunnel connection
      Then "demo.mcp.aithos.fr" has no active tunnel

    # DÉRIVE B.3 (gate) : la détection d'un pod FIGÉ (socket TCP vivant, appli
    # muette) exige un PING yamux — le crate yamux 0.13 épinglé ne l'expose
    # pas. À trancher au gate : épingler une version avec keepalive, ou un
    # canal de contrôle draft.2 (annexe B.6 le réserve déjà). Non vert ici.
    @wip @draft2
    Scenario: A frozen pod is detected by yamux keepalive within two intervals
      Given a pod is registered and serving "demo.mcp.aithos.fr" with a short keepalive
      When the pod freezes without closing its socket
      Then the tunnel closes and "demo.mcp.aithos.fr" has no active tunnel

  Rule: The relay is blind — logs carry the closed register only (B.4/A.8)

    Scenario: A refused flow never echoes the claimed SNI
      When a public client connects with SNI "ghost.mcp.aithos.fr"
      Then the flow refusal is logged with a reason class only
      And no log line contains "ghost"

    Scenario: No application byte is loggable
      Given a pod is registered and serving "demo.mcp.aithos.fr"
      When a public client sends a distinctive payload through the pipe
      Then no log line contains the payload bytes

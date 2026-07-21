@a1 @g1
Feature: Outbound gateway relay with client-side public TLS
  A gateway with no inbound port can opt into the Aithos relay while keeping
  its public TLS key, HTTP router and every application byte on the client side.

  Rule: Relay configuration is opt-in, closed and fail-closed

    @g1a
    Scenario: Omitting relay preserves the direct listener byte for byte
      Given a gateway configuration with no relay stanza
      When the gateway starts with its direct loopback listener
      Then no relay connection is attempted
      And the direct router behaves exactly as the non-relay baseline

    @g1a
    Scenario Outline: Unsafe relay configuration refuses boot before any dial
      Given a gateway relay configuration containing <defect>
      When the gateway validates its complete configuration
      Then boot is refused naming <verdict>
      And no relay, ACME or application socket is opened

      Examples:
        | defect                                      | verdict                         |
        | an unknown field                            | the unknown field               |
        | a public HTTP relay endpoint                | the HTTPS requirement           |
        | an invalid public hostname                  | the hostname requirement        |
        | a tenant, hostname and SNI mismatch         | the mapping mismatch            |
        | a private key value inline in YAML          | the forbidden inline key        |

    @g1b
    Scenario: A world-readable certificate cache refuses boot
      Given a gateway relay certificate cache readable by group or world
      When the gateway validates its complete configuration
      Then boot is refused naming the private filesystem mode
      And no relay, ACME or application socket is opened

  Rule: Registration and multiplexing consume the existing C2 contract

    @g1a
    Scenario: The gateway registration line is byte-exact to provider vector p3
      Given the gateway key and the injected instant and nonce from vector p3
      When the gateway builds and signs its B.2 registration line
      Then every registration byte equals the p3 expected wire
      And the TLS dial uses the configured SNI and ALPN "aithos-tunnel/1"

    @g1a
    Scenario Outline: A refused registration never opens an application stream
      Given the relay observes <defect>
      When the gateway attempts to establish its outbound tunnel
      Then the relay refuses the registration before yamux
      And zero application streams and zero HTTP bytes cross the mapping

      Examples:
        | defect              |
        | an unknown mapping  |
        | a suspended mapping |
        | a false signature   |
        | a replayed nonce    |
        | excessive clock skew|

    @g1c
    Scenario: One valid mapping serves the single application router through public TLS
      Given a valid outbound relay mapping and a client-owned public certificate
      When public HTTPS requests cross the tunnel
      Then public TLS terminates inside the gateway process
      And the same axum router serves "/mcp", "/oauth/callback" and "/control/v1/status"

    @g1b
    Scenario: The opaque relay never observes or logs an HTTP sentinel
      Given simultaneous public requests carrying distinct secret sentinels
      When their TLS records cross the relay
      Then the relay capture and bounded event logs contain no sentinel
      And each sentinel is visible only to its isolated gateway stream

    @g1b
    Scenario: Simultaneous public connections remain isolated
      Given two public TLS clients connected through one registered tunnel
      When both clients exchange requests concurrently
      Then each yamux stream receives only its own response
      And closing one stream does not close the other

    @g1a
    Scenario: Tunnel replacement and reconnect leave no zombie
      Given one healthy registered tunnel and a replacement connection
      When the relay accepts the replacement and sends GoAway to the old tunnel
      Then the old tunnel shuts down cleanly
      And capped jittered reconnect restores service without restarting the gateway

    @g1c
    Scenario: Relay outage does not take down the direct listener
      Given a running direct listener and an unavailable configured relay
      When relay reconnect attempts exhaust one bounded backoff interval
      Then process and direct readiness stay green
      And relay readiness is red without leaking dial details

  Rule: ACME material is born, cached and renewed only on the client

    @g1b
    Scenario: Public certificate renewal is local and atomic
      Given ACME DNS-01 delegated under the registered gateway public key
      When the gateway obtains and later renews its public certificate
      Then the account key and certificate private key never leave the client
      And the cache directory and key material have private filesystem modes
      And renewal swaps a complete valid certificate atomically
      But a failed renewal leaves the still-valid certificate active

    @g1c
    Scenario: An upstream OAuth callback crosses the relay into Vault custody
      Given a real test authorization server and protected MCP behind the gateway
      And the upstream OAuth flow was started through the relayed application router
      When the public callback returns through the relay
      Then the existing upstream OAuth registry exchanges the code
      And verifier, state and tokens exist only in Vault
      And no callback response, relay capture or gateway event log contains a token

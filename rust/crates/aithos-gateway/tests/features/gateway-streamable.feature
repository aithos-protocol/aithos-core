Feature: Streamable HTTP tolerance — real MCP clients on the hub endpoint
  The multi-context router already speaks JSON-RPC over POST /mcp, but
  a real host (MCP Inspector, Claude) exercises the transport edges the
  acceptance harness never did. This contract pins them against the
  MCP spec 2025-03-26 (verified 2026-07-16), decisions Mathieu
  (AskUserQuestion, 2026-07-16): (1) sessions are STATELESS — the
  gateway emits an opaque Mcp-Session-Id at initialize (injected
  entropy, visible ASCII) and echoes whatever id the client presents,
  but never requires one and never stores one; authority NEVER rides
  the header — it stays with the mandate chain (per-session chains
  arrive with G5, via OAuth). (2) GET and DELETE on /mcp answer 405 —
  no SSE stream and no client-side session termination are offered,
  both spec-legal. (3) A batched (array) body is refused with one
  clean -32600 error: real clients never batch and the 2025-06-18
  revision removed batching from the protocol. (4) The Origin header
  is validated fail-closed NOW (spec security MUST, anti
  DNS-rebinding): absent or loopback passes, anything else is 403
  before any JSON-RPC processing. A JSON-RPC notification (no id) is
  never answered — HTTP 202, empty body, zero JSON-RPC (answering
  notifications/initialized with -32601 is the observed bug this lot
  kills); an id-less message that is NOT a notification is refused 400
  fail-closed (never a silent act). ping answers its empty result
  promptly. resources/* and prompts/* stay -32601 with capabilities
  that never announce them.

  Rule: A notification is never answered

    Scenario: notifications/initialized is acknowledged with 202 and an empty body
      Given a provisioned multi-context gateway
      When the agent posts the notification "notifications/initialized"
      Then the HTTP status is 202
      And the HTTP body is empty
      And no request reaches any upstream

    Scenario: Every notifications/* method is acknowledged the same way
      Given a provisioned multi-context gateway
      When the agent posts the notification "notifications/cancelled"
      Then the HTTP status is 202
      And the HTTP body is empty
      And no request reaches any upstream

    Scenario: An id-less tools/call is refused, never silently executed
      Given a provisioned multi-context gateway
      When the agent posts a "tools/call" for "brand.read" without an id
      Then the HTTP status is 400
      And the response is a JSON-RPC error with a null id naming the missing id
      And no request reaches any upstream
      And no act is recorded in any gamma

  Rule: ping answers promptly and touches nothing

    Scenario: ping answers an empty result
      Given a provisioned multi-context gateway
      When the agent calls "ping"
      Then the answer is exactly the empty JSON-RPC result
      And no request reaches any upstream

  Rule: Sessions are stateless — emitted, echoed, never required

    Scenario: initialize returns an opaque session id from injected entropy
      Given a provisioned multi-context gateway
      When the agent initializes over HTTP
      Then the response carries an Mcp-Session-Id header of visible ASCII
      And two initializations yield two different session ids

    Scenario: The session id the client presents is echoed back
      Given a provisioned multi-context gateway
      When the agent initializes over HTTP and calls "tools/list" presenting the returned session id
      Then the call is served
      And the response echoes the same Mcp-Session-Id header

    Scenario: A request with an unknown or absent session id is served all the same
      Given a provisioned multi-context gateway
      When the agent calls "tools/list" over HTTP presenting the session id "stale-or-foreign"
      And the agent calls "tools/list" over HTTP presenting no session id
      Then both calls are served
      And neither response is an error

  Rule: What the endpoint does not offer, it refuses cleanly

    Scenario: GET /mcp answers 405 — no SSE stream is offered
      Given a provisioned multi-context gateway
      When the agent issues a GET to the MCP endpoint
      Then the HTTP status is 405

    Scenario: DELETE /mcp answers 405 — sessions are not client-terminable
      Given a provisioned multi-context gateway
      When the agent issues a DELETE to the MCP endpoint
      Then the HTTP status is 405

    Scenario: A batched body is refused with one clean invalid-request error
      Given a provisioned multi-context gateway
      When the agent posts a JSON array batching two requests
      Then the response is a JSON-RPC error with a null id and code -32600
      And the error message names batching as unsupported
      And no request reaches any upstream

  Rule: Origin is validated fail-closed before any JSON-RPC

    Scenario: A non-local Origin is refused before processing
      Given a provisioned multi-context gateway
      When the agent posts "tools/list" with the Origin header "https://evil.example"
      Then the HTTP status is 403
      And no request reaches any upstream
      And no act is recorded in any gamma

    Scenario: An absent or loopback Origin passes
      Given a provisioned multi-context gateway
      When the agent posts "tools/list" with the Origin header "http://127.0.0.1:4870"
      And the agent posts "tools/list" without an Origin header
      Then both calls are served

  Rule: Undeclared capabilities stay closed

    Scenario: resources and prompts answer method-not-found and are never announced
      Given a provisioned multi-context gateway
      When the agent initializes and requests MCP resources through the hub
      Then the initialize capabilities announce tools and nothing else
      And the resources request is refused with method-not-found

Feature: Store ACME — the delegated DNS-01 surface (annexe B.5)
  Lot P6 (jalon M2): the zone is ours, the cert and its key stay at the
  client (A3). The store serves PUT/DELETE /acme/txt under the A.2
  envelope with the GRAVED B.5 exception: key = gateway_pub (multibase)
  and mandate: [] — the authority is the control-plane mapping of the
  signing gateway key (the B.2 model), never a mandate chain. The route
  poses/retires TXT `_acme-challenge.<hostname>` (TTL 60 s); the hostname
  MUST belong to the signer's binding. Errors: registre A.7 +
  mapping_mismatch. Anti-abus: ≤ 10 PUT/h per hostname. Fail-closed: the
  first failing check answers, nothing else is evaluated; the server
  never corrects a request and never touches DNS before the full order
  passed. Records are purged server-side after 10 minutes regardless.

  Background:
    Given the tenant "acme" is enrolled and bound to the vector DID
    And the control plane binds gateway key "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1" to tenant "acme" and hostname "demo.mcp.aithos.fr"
    And the service authority is "store.aithos.fr"

  Rule: The route serves PUT and DELETE, nothing else, envelope always

    Scenario: A gateway-signed PUT poses the challenge TXT with TTL 60
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-put-ok"
      Then the request is accepted with status 204
      And the DNS backend holds TXT "_acme-challenge.demo.mcp.aithos.fr" with value "tok-p6-put-ok" and TTL 60

    Scenario: A gateway-signed DELETE retires the challenge TXT
      Given the bound gateway posed TXT value "tok-p6-del-1" for hostname "demo.mcp.aithos.fr"
      When the bound gateway DELETEs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-del-1"
      Then the request is accepted with status 204
      And the DNS backend holds no TXT for "_acme-challenge.demo.mcp.aithos.fr"

    Scenario: Deleting an absent record is idempotent
      When the bound gateway DELETEs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-absent"
      Then the request is accepted with status 204

    Scenario: A fresh PUT replaces the previous value (one live challenge per hostname)
      Given the bound gateway posed TXT value "tok-p6-old" for hostname "demo.mcp.aithos.fr"
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-new"
      Then the request is accepted with status 204
      And the DNS backend holds TXT "_acme-challenge.demo.mcp.aithos.fr" with value "tok-p6-new" and TTL 60

    Scenario: A verb the route does not define answers not_covered
      When the bound gateway GETs "/acme/txt" with a valid envelope
      Then the response is 403 "not_covered"

    Scenario: An unsigned PUT answers envelope_missing
      When an unsigned PUT arrives at "/acme/txt" with a well-formed body
      Then the response is 401 "envelope_missing"

    Scenario: The acme route carries no query
      When an unsigned PUT arrives at path "/acme/txt?debug=1"
      Then the response is 400 "path_invalid"

    Scenario: An unknown acme route stays outside the grammar
      When an unsigned PUT arrives at path "/acme/cert"
      Then the response is 400 "path_invalid"

    Scenario: An unknown wire major version answers version_unsupported
      When the bound gateway PUTs "/acme/txt" claiming wire version "2.0.0"
      Then the response is 426 "version_unsupported"

  Rule: The B.5 envelope form is closed — key = gateway_pub, mandate = []

    Scenario: A non-empty mandate list on /acme/txt is a form fault
      When the bound gateway PUTs "/acme/txt" carrying mandate list "mandate_0000000000000000000000P0M1"
      Then the response is 400 "envelope_invalid"

    Scenario: An owner fragment key on /acme/txt is a form fault
      When an owner-root-signed PUT arrives at "/acme/txt" with a well-formed body
      Then the response is 400 "envelope_invalid"

    Scenario: An unknown body field rejects the request
      When the bound gateway PUTs "/acme/txt" with a body carrying an extra field "ttl"
      Then the response is 400 "envelope_invalid"

    Scenario: A value over 255 characters rejects the request
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with a 256-character value
      Then the response is 400 "envelope_invalid"

    Scenario: A value outside the base64url alphabet rejects the request
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "no spaces allowed"
      Then the response is 400 "envelope_invalid"

    Scenario: A hostname outside the lowercase DNS grammar rejects the request
      When the bound gateway PUTs "/acme/txt" for hostname "DeMo.McP.AiThOs.Fr" with value "tok-p6-case"
      Then the response is 400 "envelope_invalid"

    Scenario: A tampered body is rejected by body_b3
      When the bound gateway signs a PUT "/acme/txt" body but sends different bytes
      Then the response is 400 "envelope_invalid"

    Scenario: An envelope signed for another authority is rejected
      When the bound gateway PUTs "/acme/txt" whose envelope names host "gateway.example.org"
      Then the response is 400 "envelope_invalid"

  Rule: Skew, nonce and signature are the shared A.2 machinery

    Scenario: The 300 s boundary is accepted
      Given the server clock reads "2026-07-18T12:05:00Z"
      When the bound gateway PUTs "/acme/txt" signed at "2026-07-18T12:00:00Z"
      Then the request is accepted with status 204

    Scenario: 301 s of skew answers clock_skew
      Given the server clock reads "2026-07-18T12:05:01Z"
      When the bound gateway PUTs "/acme/txt" signed at "2026-07-18T12:00:00Z"
      Then the response is 401 "clock_skew"

    Scenario: The same (key, nonce) presented twice answers nonce_replayed
      When the bound gateway PUTs "/acme/txt" twice with nonce "bdd-acme-nonce-0001"
      Then the second response is 401 "nonce_replayed"

    Scenario: A nonce burns even when the mapping later refuses
      Given a gateway PUT for a foreign hostname with nonce "bdd-acme-burn-0001" was refused with "mapping_mismatch"
      When the same gateway PUT is presented again
      Then the response is 401 "nonce_replayed"

    Scenario: A corrupted signature answers signature_invalid
      When the bound gateway PUTs "/acme/txt" with its signature corrupted
      Then the response is 401 "signature_invalid"

  Rule: The authority is the control-plane mapping — the graved B.5 exception

    Scenario: A gateway key enrolled for no tunnel answers mapping_mismatch
      When an unenrolled gateway key PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr"
      Then the response is 403 "mapping_mismatch"

    Scenario: A hostname outside the signer's binding answers mapping_mismatch
      When the bound gateway PUTs "/acme/txt" for hostname "other.mcp.aithos.fr" with value "tok-p6-foreign"
      Then the response is 403 "mapping_mismatch"
      And the DNS backend holds no TXT for "_acme-challenge.other.mcp.aithos.fr"

    Scenario: A suspended binding answers suspended
      Given the binding of the gateway key is suspended
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-susp"
      Then the response is 403 "suspended"

    Scenario: A suspended tenant answers suspended
      Given the tenant "acme" is suspended
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-susp2"
      Then the response is 403 "suspended"

  Rule: Anti-abus — at most 10 PUT per rolling hour per hostname

    Scenario: The eleventh PUT inside the hour answers rate_limited
      Given the bound gateway posed 10 challenge values within the hour
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-eleventh"
      Then the response is 429 "rate_limited"

    Scenario: The budget frees once the hour rolls
      Given the bound gateway posed 10 challenge values within the hour
      And the server clock advances by 3601 seconds
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-fresh-hour"
      Then the request is accepted with status 204

    Scenario: DELETE spends no PUT budget
      Given the bound gateway posed 10 challenge values within the hour
      When the bound gateway DELETEs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-any"
      Then the request is accepted with status 204

  Rule: Hygiene — records die on their own, logs stay redacted

    Scenario: A record older than 10 minutes is purged server-side
      Given the bound gateway posed TXT value "tok-p6-stale" for hostname "demo.mcp.aithos.fr"
      When the acme purge runs 601 seconds later
      Then the DNS backend holds no TXT for "_acme-challenge.demo.mcp.aithos.fr"

    Scenario: A fresh record survives the purge
      Given the bound gateway posed TXT value "tok-p6-fresh" for hostname "demo.mcp.aithos.fr"
      When the acme purge runs 599 seconds later
      Then the DNS backend holds TXT "_acme-challenge.demo.mcp.aithos.fr" with value "tok-p6-fresh" and TTL 60

    Scenario: The acme log line carries the closed register only
      When the bound gateway PUTs "/acme/txt" for hostname "demo.mcp.aithos.fr" with value "tok-p6-sentinel-value"
      Then the request log for class "acme" contains no "demo.mcp.aithos.fr" and no "tok-p6-sentinel-value" and no envelope material

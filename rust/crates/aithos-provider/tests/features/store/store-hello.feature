Feature: Store hello — the signed envelope at the door
  Lot P1 (HANDOFF-PROVIDER-AWS): the aithos-store-api skeleton verifies the
  X-Aithos-Auth envelope in the exact normative order of INFRA-PROVIDER
  annexe A.2 (contrat C1) — fail-closed: the FIRST failing check answers,
  nothing else is evaluated. P1 scope: owner keys resolve against the stored
  did.json; mandated chains fail closed pending the P2 chain machinery; the
  server holds no secret, corrects nothing, and logs under the A.8 register.

  Background:
    Given the tenant "acme" is enrolled and bound to the vector DID
    And the vector did.json is stored for that DID
    And the service authority is "store.aithos.fr"

  Rule: Step 0 — the path grammar gates everything, before the envelope

    Scenario: A path outside the A.1 grammar answers path_invalid
      When an unsigned GET arrives for path "/t/acme/not-a-did/whatever"
      Then the response is 400 "path_invalid"

    Scenario: A data path with a traversal segment answers path_invalid
      When an owner-signed PUT arrives for relative path "e/public/../../secrets.md" with body "# nope"
      Then the response is 400 "path_invalid"

    Scenario: A malformed tenant answers path_invalid before anything else
      When an unsigned GET arrives for path "/t/A!/did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr/did.json"
      Then the response is 400 "path_invalid"

  Rule: Step 1 — the tenant routes, the DID carries the authority

    Scenario: An unknown tenant answers unknown_tenant
      When an owner-signed GET arrives for tenant "ghost" and relative path "manifest.json"
      Then the response is 404 "unknown_tenant"

    Scenario: A DID not bound to the tenant is only named under a valid envelope
      When an owner-signed GET for an unbound DID arrives with a valid envelope
      Then the response is 403 "did_not_bound"

  Rule: Step 2 — form: presence, base64url, canonical JCS, closed field set

    Scenario: A non-anonymous route without an envelope answers envelope_missing
      When an unsigned GET arrives for relative path "manifest.json"
      Then the response is 401 "envelope_missing"

    Scenario: A header that is not base64url answers envelope_invalid
      When a GET arrives with header value "not!base64url//"
      Then the response is 400 "envelope_invalid"

    Scenario: An envelope with an unknown field is rejected
      When an owner-signed GET arrives whose envelope carries an extra field "debug"
      Then the response is 400 "envelope_invalid"

    Scenario: An envelope whose bytes are not canonical JCS is rejected
      When an owner-signed GET arrives re-encoded with spaces between JSON tokens
      Then the response is 400 "envelope_invalid"

    Scenario: A version other than 1 is rejected
      When an owner-signed GET arrives whose envelope carries v 2
      Then the response is 400 "envelope_invalid"

  Rule: Step 3 — host, method and path bind the request, byte for byte

    Scenario: An envelope signed for another authority is rejected
      When an owner-signed GET arrives whose envelope names host "gateway.example.org"
      Then the response is 400 "envelope_invalid"

    Scenario: An envelope signed for another method is rejected
      When a GET request carries an envelope signed for method "PUT"
      Then the response is 400 "envelope_invalid"

    Scenario: An envelope signed for another path is rejected
      When a GET for relative path "manifest.json" carries an envelope signed for relative path "did.json"
      Then the response is 400 "envelope_invalid"

  Rule: Step 4 — body_b3 seals the body

    Scenario: A tampered body is rejected
      When an owner-signed PUT for relative path "e/public/hello.md" signs body "# hello" but sends body "# tampered"
      Then the response is 400 "envelope_invalid"

    Scenario: A body where the envelope promised none is rejected
      When an owner-signed GET arrives carrying an unexpected body "# extra"
      Then the response is 400 "envelope_invalid"

  Rule: Step 5 — the clock window is ±300 s, inclusive

    Scenario: The 300 s boundary is accepted
      Given the server clock reads "2026-07-16T12:05:00Z"
      When an owner-signed GET for relative path "did.json" is signed at "2026-07-16T12:00:00Z"
      Then the request is accepted

    Scenario: 301 s of skew answers clock_skew
      Given the server clock reads "2026-07-16T12:05:01Z"
      When an owner-signed GET for relative path "did.json" is signed at "2026-07-16T12:00:00Z"
      Then the response is 401 "clock_skew"

  Rule: Step 6 — a nonce burns on first sight, before any effect

    Scenario: The same (key, nonce) presented twice answers nonce_replayed
      When an owner-signed GET with nonce "bdd-nonce-replay-0001" is presented twice
      Then the second response is 401 "nonce_replayed"

    Scenario: A nonce burns even when the request later fails
      Given a mandated GET with nonce "bdd-nonce-burned-0001" was refused after the nonce check
      When the same mandated GET is presented again
      Then the response is 401 "nonce_replayed"

  Rule: Step 7 — key resolution is fail-closed

    Scenario: A multibase key with an empty mandate chain is rejected
      When a GET arrives signed by a raw key with an empty mandate list
      Then the response is 403 "chain_invalid"

    Scenario: An owner fragment on a DID with no stored did.json is rejected
      Given the did.json of the bound DID is absent from the store
      When an owner-signed GET arrives for relative path "manifest.json"
      Then the response is 403 "chain_invalid"

  Rule: Step 8 — the envelope signature verifies under the resolved key

    Scenario: A corrupted owner signature answers signature_invalid
      When an owner-signed GET arrives with its signature corrupted
      Then the response is 401 "signature_invalid"

    Scenario: A mandated envelope with a corrupted signature answers signature_invalid
      # Gate 3 (P2): #7 resolves the leaf against the STORED certs before
      # #8 (A.2 order) — the chain state must exist for the signature
      # check to even be reached.
      Given the gamma log carries the mandate grant and its bound action
      When a mandated GET arrives with its signature corrupted
      Then the response is 401 "signature_invalid"

  Rule: Step 9 — P1 has no chain machinery: mandated requests fail closed

    Scenario: A well-formed mandated request is refused pending P2, never accepted
      When a correctly signed mandated GET arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
      Then the response is 403 "chain_invalid"

  Rule: Step 10 — the owner covers everything on the DID; anonymous covers A2 only

    Scenario: The signed hello — an owner PUT into the public zone is accepted
      When an owner-signed PUT arrives for relative path "e/public/hello.md" with body "# hello"
      Then the request is accepted
      And the stored object at "e/public/hello.md" equals "# hello"

    Scenario: The owner reads its own bundle back under envelope
      Given an owner-signed PUT stored "# hello" at relative path "e/public/hello.md"
      When an owner-signed GET arrives for relative path "e/public/hello.md"
      Then the request is accepted with body "# hello"

    Scenario: The public zone is served without an envelope
      Given an owner-signed PUT stored "# hello" at relative path "e/public/hello.md"
      When an unsigned GET arrives for relative path "e/public/hello.md"
      Then the request is accepted with body "# hello"

    Scenario: The did.json is served without an envelope
      When an unsigned GET arrives for relative path "did.json"
      Then the request is accepted

    Scenario: An anonymous read outside the public perimeter answers envelope_missing
      When an unsigned GET arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
      Then the response is 401 "envelope_missing"

    Scenario: An absent object inside a covered perimeter answers not_found
      When an owner-signed GET arrives for relative path "e/public/absent.md"
      Then the response is 404 "not_found"

  Rule: The wire is versioned

    Scenario: Every response carries the store version header
      When an unsigned GET arrives for relative path "did.json"
      Then the response carries header "X-Aithos-Store" equal to "1.0.0-draft.1"

    Scenario: An unknown major version answers version_unsupported
      When an unsigned GET arrives for relative path "did.json" with header "X-Aithos-Store" equal to "2.0.0"
      Then the response is 426 "version_unsupported"

  Rule: Anti-abus A.8 — limits answer payload_too_large

    Scenario: An envelope over 8 KiB is refused before parsing
      When a GET arrives with a 9000-byte header value
      Then the response is 413 "payload_too_large"

  Rule: Discipline de logs A.8 — the register is closed

    Scenario: A refusal log names the route class, never the path or the envelope
      When an owner-signed PUT arrives for relative path "e/public/logged-secret-name.md" with body "# sentinel-body"
      Then the request log for class "put_artifact" contains no "logged-secret-name" and no "sentinel-body" and no envelope material

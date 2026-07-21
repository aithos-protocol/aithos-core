@remote-client
Feature: RemoteStore client (P3) — the aithos-bundle client speaks wire A.2 against the real service
  # INFRA-PROVIDER annexe A, consumed from the CLIENT side (HANDOFF-PROVIDER-AWS
  # P3): the `RemoteStore` of aithos-bundle (feature `remote`, sync `Store`
  # trait — get/put/list, std::io::Result) builds the signed X-Aithos-Auth
  # envelope (JCS, body_b3 BLAKE3, fresh nonce, at), deposits artifacts,
  # publishes the manifest and appends gamma under the A.5 CAS (If-Head,
  # a 409 surfaces the current head and the client rebases), retries with
  # backoff on transport faults, and keeps the A.6 local cache classes
  # (immutable / no-store / revalidate-ETag).
  #
  # Written BEFORE the code (rituel BDD). The counterparty is the REAL
  # aithos-provider service, in-process, on a real localhost socket — the
  # same AppState the store cucumber harness proves; nothing is mocked on
  # the wire. Fixtures are the committed p1/p7 vectors (owner keys of the
  # vector DID, real bundle publication packages) — never re-invented
  # crypto. The signer is INJECTED (arbitrage ②: a seam, never a key
  # baked into the lib); the clock and the nonce entropy are injected
  # (the core purity rule extends to the client).
  #
  # Arbitrage ① (2026-07-21, Mathieu): ureq + rustls — minimal blocking
  # client for a sync lib; no async runtime in aithos-bundle.

  Background:
    Given the real store service listens on a local socket
    And the tenant "acme" is enrolled and bound to the vector DID
    And the vector did.json is stored for that DID
    And an owner content signer from the p1 vectors is injected
    And a RemoteStore client points at the service for tenant "acme" and the vector DID

  # ============================================================
  # A.2 — the signed envelope: what is signed IS what is sent.
  # The proof is the real service ACCEPTING (its verify order 0-10 ran).
  # ============================================================

  @envelope
  Scenario: A client get() sends a signed envelope the real service accepts
    Given the artifact "e/public/hello.md" is stored with body "bonjour"
    When the client calls get on "e/public/hello.md"
    Then the call returns the exact stored bytes
    And the request carried an X-Aithos-Auth envelope naming key "#content"

  @envelope
  Scenario: A client put() hashes the exact body into body_b3
    When the client calls put on "e/circle/blobs/01000000000000000000000000.enc" with 4096 opaque bytes
    Then the deposit is accepted by the real service
    And the envelope body_b3 equals the BLAKE3 of the sent body

  @envelope
  Scenario: Two consecutive calls never reuse a nonce
    Given the artifact "e/public/hello.md" is stored with body "bonjour"
    When the client calls get on "e/public/hello.md" twice
    Then both calls succeed
    And the two envelopes carry distinct nonces

  @envelope @fail-closed
  Scenario: A server rejection surfaces as a typed error, never a silent success
    When the client calls get on "gamma/2026-07.jsonl" with a signer the chain does not cover
    Then the call fails with a not_covered store error
    And no bytes are returned

  # ============================================================
  # list — the ?list=prefix surface through the sync trait
  # ============================================================

  @list
  Scenario: list() walks the listing pages of a covered prefix
    Given 3 artifacts are stored under "e/public/"
    When the client calls list on "e/public/"
    Then the listing returns the 3 paths in wire order

  # ============================================================
  # A.5 — publish under CAS: If-Head from the tracked head, 409 -> rebase
  # ============================================================

  @cas
  Scenario: A genesis publish sends If-Head none and adopts the returned head
    Given a p7 genesis publication package is loaded
    When the client publishes the genesis manifest
    Then the publish is accepted with height 1
    And the client's tracked manifest head equals the head the service returned

  @cas
  Scenario: The next publish pins the tracked head in If-Head
    Given a p7 genesis publication package is loaded
    And the client published the genesis manifest
    When the client publishes the successor manifest from p7
    Then the publish is accepted with height 2
    And the If-Head sent equals the head returned by the genesis publish

  @cas @conflict
  Scenario: A concurrent publish surfaces the 409 head for the rebase
    Given a p7 genesis publication package is loaded
    And the client published the genesis manifest
    And another writer already advanced the manifest head on the service
    When the client publishes the successor manifest from p7
    Then the publish fails with a cas_mismatch carrying the current head
    And the client's tracked manifest head now equals the served head

  @cas @gamma
  Scenario: An appended gamma entry rides POST /gamma under the gamma head
    Given the client knows the stored gamma head is empty
    When the client appends a signed gamma entry from the p1 fixtures
    Then the append is accepted
    And the tracked gamma head equals the head the service returned

  @cas @gamma @conflict
  Scenario: A gamma append conflict surfaces the served head so the caller re-signs
    Given the client knows the stored gamma head is empty
    And another writer already appended to the gamma on the service
    When the client appends a signed gamma entry from the p1 fixtures
    Then the append fails with a cas_mismatch carrying the current head

  # ============================================================
  # Transport — retries with backoff; fail-closed, never fail-open
  # ============================================================

  @retry
  Scenario: A transient transport fault is retried with backoff and then succeeds
    Given the artifact "e/public/hello.md" is stored with body "bonjour"
    And the service drops the next 2 connections
    When the client calls get on "e/public/hello.md"
    Then the call returns the exact stored bytes
    And the client waited a backoff between attempts

  @retry @fail-closed
  Scenario: Retries are bounded — a dead service is an error, never a hang
    Given the service stops listening
    When the client calls get on "e/public/hello.md"
    Then the call fails with a transport store error after the bounded retries

  @retry @fail-closed
  Scenario: A 4xx verdict is never retried
    When the client calls get on "gamma/2026-07.jsonl" with a signer the chain does not cover
    Then the service saw exactly 1 request for that path

  # ============================================================
  # A.6 — the local cache classes
  # ============================================================

  @cache
  Scenario: An immutable object is served from the local cache on the second read
    Given the p1 mandate cert is stored by the enrollment
    When the client calls get on the p1 cert path twice
    Then both calls return the exact stored bytes
    And the service saw exactly 1 request for that path

  @cache
  Scenario: A no-store object is fetched from the wire on every read
    Given a p7 genesis publication package is loaded
    And the client published the genesis manifest
    When the client calls get on "manifest.json" twice
    Then the service saw exactly 2 requests for that path

  @cache @etag
  Scenario: A revalidate-class object rides If-None-Match and serves the cache on 304
    Given the artifact "e/circle/blobs/01000000000000000000000000.enc" is stored with 4096 opaque bytes
    When the client calls get on that blob twice
    Then both calls return the exact stored bytes
    And the second request carried If-None-Match and was answered 304

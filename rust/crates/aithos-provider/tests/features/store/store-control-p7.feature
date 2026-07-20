Feature: Store control plane — the tenant read-model as a real store (P7)
  Lot P7 (HANDOFF-PROVIDER-AWS): the image-baked bootstrap stops carrying
  tenants in prod. The SAME lookups of control.rs (tenant_state, did_bound,
  resolve_tunnel) resolve against a control-plane backend (DynamoDB in prod,
  a mutable in-memory double here, behind the SAME freshness cache the
  deployed composition uses) written by an admin plane — create, bind-did,
  suspend, reactivate, purge — with NO service redeploy. The wire does not
  move: every refusal below already exists in the A.7 register
  (unknown_tenant, did_not_bound, suspended, unavailable) — a new code at
  this seam would be a spec drift, arbitrated before it ships.
  Fail-closed everywhere: a control backend that cannot answer refuses 503
  unavailable; it never invents an unknown_tenant (pattern of the étape-6
  seams). Freshness (arbitrage gate contrat 2026-07-20): a short TTL cache,
  bound 30 s — every control-plane write (creation, suspension,
  reactivation, purge) is served at the latest once the bound elapses, well
  inside the < 60 s product promise. The scenarios pin the bound with the
  injected test clock, never with a sleep.

  Background:
    Given the control backend is a mutable control store
    And the service authority is "store.aithos.fr"

  Rule: The A.7 answers do not move behind the control-plane seam

    Scenario: A tenant created in the control store is served without redeploy
      Given an owner-signed GET for control tenant "fresh" answered 404 "unknown_tenant"
      When the admin plane creates tenant "fresh" bound to the vector DID
      And the vector did.json is stored for tenant "fresh"
      And the control freshness bound has elapsed
      Then an owner-signed GET for control tenant "fresh" and relative path "did.json" answers 200
      And the service was never restarted

    Scenario: A tenant absent from the control store answers unknown_tenant
      When an owner-signed GET arrives for control tenant "ghost" and relative path "manifest.json"
      Then the response is 404 "unknown_tenant"

    Scenario: A tenant suspended in the control store answers suspended
      Given the control store holds tenant "acme" bound to the vector DID
      And the admin plane suspends tenant "acme"
      When an owner-signed GET arrives for control tenant "acme" and relative path "manifest.json"
      Then the response is 403 "suspended"

    Scenario: A DID not bound in the control store answers did_not_bound only under a valid envelope
      Given the control store holds tenant "acme" bound to the vector DID
      When an owner-signed GET for a DID the control store never bound arrives with a valid envelope
      Then the response is 403 "did_not_bound"

    Scenario: An unbound DID without a valid envelope is never named
      Given the control store holds tenant "acme" bound to the vector DID
      When an unsigned GET arrives for an unbound DID under tenant "acme"
      Then the response is 401 "envelope_missing"

  Rule: Freshness — a control-plane write cuts within the bound, both ways

    Scenario: A suspension written to the control store is served within the freshness bound
      Given the control store holds tenant "acme" bound to the vector DID
      And an owner-signed GET for control tenant "acme" answered 200
      When the admin plane suspends tenant "acme"
      And the control freshness bound has elapsed
      Then an owner-signed GET for control tenant "acme" and relative path "did.json" answers 403 "suspended"

    Scenario: A reactivation propagates within the same bound
      Given the control store holds tenant "acme" bound to the vector DID, suspended
      And an owner-signed GET for control tenant "acme" answered 403 "suspended"
      When the admin plane reactivates tenant "acme"
      And the control freshness bound has elapsed
      Then an owner-signed GET for control tenant "acme" and relative path "did.json" answers 200

    Scenario: A purge removes the tenant from the wire within the bound
      Given the control store holds tenant "acme" bound to the vector DID
      And an owner-signed GET for control tenant "acme" answered 200
      When the admin plane purges tenant "acme"
      And the control freshness bound has elapsed
      Then an owner-signed GET for control tenant "acme" and relative path "did.json" answers 404 "unknown_tenant"

  Rule: Fail-closed — a mute control backend refuses, it invents nothing

    Scenario: A control-store outage answers unavailable, never unknown_tenant
      Given the control store holds tenant "acme" bound to the vector DID
      And the control store stops answering
      When an owner-signed GET arrives for control tenant "acme" and relative path "manifest.json"
      Then the response is 503 "unavailable"

    Scenario: An outage never fabricates a DID-binding refusal
      Given the control store holds tenant "acme" bound to the vector DID
      And the control store stops answering after the tenant gate
      When an owner-signed GET arrives for control tenant "acme" and relative path "manifest.json"
      Then the response is 503 "unavailable"

    Scenario: The outage of the control store leaves anonymous surfaces untouched
      Given the control store stops answering
      When an unsigned GET arrives for path "/healthz"
      Then the request is accepted

  Rule: A refusal never caches — no intermediary may outlive the control plane
    Arbitrage gate contrat P7 (2026-07-20, consigné au gate 6): refuse()
    emits an explicit Cache-Control: no-store on every error surface — a
    heuristically-cached unknown_tenant or suspended would defeat the
    < 60 s propagation bound through any intermediary (RFC 9110 §9.3.2).

    Scenario: A tenant refusal carries Cache-Control no-store
      When an owner-signed GET arrives for control tenant "ghost" and relative path "manifest.json"
      Then the response is 404 "unknown_tenant"
      And the response carries header "Cache-Control" equal to "no-store"

    Scenario: An envelope refusal carries Cache-Control no-store
      Given the control store holds tenant "acme" bound to the vector DID
      When an unsigned GET arrives for an unbound DID under tenant "acme"
      Then the response is 401 "envelope_missing"
      And the response carries header "Cache-Control" equal to "no-store"

  Rule: Coexistence — the bootstrap backend remains the dev/test shape

    Scenario: The bootstrap-backed control plane keeps serving the committed vectors
      Given the control backend is the p1 replay bootstrap
      And the vector did.json is stored for that DID
      When an owner-signed GET arrives for relative path "did.json"
      Then the request is accepted

    Scenario: The bootstrap backend still refuses what it never enrolled
      Given the control backend is the p1 replay bootstrap
      When an owner-signed GET arrives for control tenant "ghost" and relative path "manifest.json"
      Then the response is 404 "unknown_tenant"

@wip @cold
Feature: Cold roundtrip — a virgin store ingests and cold-verifies a published edition
  # INFRA-PROVIDER annexe A + the aithos-bundle keyless façade
  # (publication.rs): import_keyless / cold_verify / cold_verify_for_cas.
  #
  # This is the P2 real-E2E contract, written BEFORE the code (rituel BDD).
  # The whole feature is @wip until gate 8:
  #   bundle grantee -> HTTP provider -> stop/restart -> download into a virgin
  #   store -> cold verify -> owner/grantee reads. No protocol mock, no content
  #   key, no plaintext. The provider moves opaque bytes and serializes a CAS;
  #   every semantic verdict is the façade's (delegated to aithos-core).
  #
  # Fixtures are the committed p8 vectors: a REAL aithos-bundle export_keyless
  # package (owner edition + a grantee-readable sub-tree), its pinned objects and
  # its typed CAS facts — never re-invented crypto, never a signer or opener.

  Background:
    Given the tenant "acme" is enrolled and bound to the vector DID
    And the service authority is "store.aithos.fr"
    And a bundle-exported publication package is available from p8

  # ============================================================
  # Façade contract — import_keyless installs into ONE fresh store at one
  # logical commit point, and refuses anything else.
  # ============================================================

  @import
  Scenario: A keyless package installs into a fresh empty store in one transaction
    Given a fresh empty store
    When the package is imported into the store
    Then the request is accepted
    And every pinned object of the package is present in the store

  @import @fail-closed
  Scenario: import_keyless refuses a store that is not already empty
    Given a store that already holds one object
    When the package is imported into the store
    Then import is refused because the store is not empty

  @import @keyless
  Scenario: The exported package carries no private material
    When the package is inspected for forbidden shapes
    # reject_private_shape: no seed, private_key, secret_key, owner_keys, dk,
    # credential, plaintext, capability anywhere in the package
    Then the package contains no private material

  # ============================================================
  # The real wire roundtrip — publish over HTTP, survive a restart, download
  # into a SECOND virgin store, cold-verify owner and grantee from bytes alone.
  # ============================================================

  @e2e
  Scenario: A published edition survives a restart and cold-verifies in a virgin store
    Given a fresh empty store served over HTTP by the provider
    And the package edition is published to the provider under the CAS
    When the provider is stopped and restarted
    And a second virgin store downloads every pinned object from the provider
    Then the second store cold-verifies the owner edition
    And the second store cold-verifies the grantee sub-tree
    And the cold CAS facts equal the published manifest and gamma heads

  @e2e @reads
  Scenario: The owner and the grantee each read their covered objects from the provider
    Given a fresh empty store served over HTTP by the provider
    And the package edition is published to the provider under the CAS
    When the owner reads the manifest and did.json
    And the grantee reads the covered sub-tree under its mandate
    Then every read is accepted
    And no read returns an object outside the reader's perimeter

  # ============================================================
  # Cold verification is fail-closed on any tampering — the download is trusted
  # only because the bytes reproduce the pinned hashes.
  # ============================================================

  @e2e @fail-closed
  Scenario: A substituted object fails cold verification
    Given a second virgin store downloaded from the provider
    When one downloaded object is substituted for different bytes
    Then cold verification fails closed

  @e2e @fail-closed
  Scenario: A missing pinned object fails cold verification
    Given a second virgin store downloaded from the provider
    When one pinned object is dropped from the download
    Then cold verification fails closed

  @e2e @fail-closed
  Scenario: A store whose manifest is not the edition tip fails cold verification
    Given a second virgin store downloaded from the provider
    When the manifest.json no longer equals the edition history tip
    Then cold verification fails closed

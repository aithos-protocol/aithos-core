@reads
Feature: Reads — heads, listing, batch, sync and the draft.2 servable layout
  # INFRA-PROVIDER annexe A: A.3 (routes /heads, ?list=, /batch, /sync + the
  # path-map coverage lines), A.1 + redline gate 5 2026-07-20 (the draft.2
  # servable layout: manifests/<h>.json, changesets/, evidence/, K1-C aliases),
  # A.6 (cache classes — asserted at étape 6 with the real backend, not here),
  # A.7 (closed error registry), A.8 (limits: batch ≤ 256, listing ≤ 1000).
  #
  # This is the P2 read-surface contract, written BEFORE the code (rituel BDD).
  # Every scenario is @wip until its gate lands it; each reject NAMES its closed
  # A.7 code. The read plan frozen in p8-cold-roundtrip.json (grantee read.circle
  # reads the K1-C aliases, stays not_covered on e/self/**) is satisfied to the
  # letter. Fixtures are the committed vectors: p1 (owner, mandate, gamma) and
  # the REAL aithos-bundle packages of p7-bundle-packages.json — never
  # re-invented crypto. The p9-store-reads vector freezes the wire bytes.
  #
  # Gate map: gate 5 (this étape) lands every scenario of this feature.

  Background:
    Given the tenant "acme" is enrolled and bound to the vector DID
    And the vector did.json is stored for that DID
    And the service authority is "store.aithos.fr"

  # ============================================================
  # A.3 — GET /heads: the two hot heads, served to any valid chain.
  # ============================================================

  @heads
  Scenario: The owner reads the heads of a published store (p9 heads_ok)
    Given the store holds the p8_cold edition at height 2
    When an owner-signed GET arrives for "/heads"
    Then the request is accepted
    And the heads body carries height 2, the p8_cold manifest head, and a null gamma head

  @heads
  Scenario: A mandated chain reads the heads (p9 heads_mandated)
    Given the store holds the p8_cold edition at height 2
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for "/heads"
    # "toute chaîne valide du DID" serves /heads (A.3 path-map)
    Then the request is accepted

  @heads
  Scenario: An anonymous heads read is refused (p9 heads_anonymous)
    Given the store holds the p8_cold edition at height 2
    When an anonymous GET arrives for "/heads"
    # /heads is not in the anonymous A2 set
    Then the response is 401 "envelope_missing"

  # ============================================================
  # A.3 — GET ?list=: paginated listing, filtered to the covered
  # perimeter (coarse — zone-level, never a selector decision).
  # ============================================================

  @list
  Scenario: The owner lists every stored path under a prefix (p9 list_owner)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an owner-signed list arrives for prefix ""
    Then the request is accepted
    And the listing carries every stored path in lexicographic order, not truncated

  @list
  Scenario: A mandated listing is filtered to the covered perimeter (p9 list_mandated_filtered)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated list arrives for prefix "e/"
    # read.circle: e/circle/** stays, e/self/** is filtered out — coarse
    # perimeter filtering, a 200 with fewer paths, never an error
    Then the request is accepted
    And the listing carries no path outside the covered perimeter

  @list
  Scenario: Listing paginates with after and limit (p9 list_paginated)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an owner-signed list arrives for prefix "" with limit 2
    Then the request is accepted
    And the listing carries 2 paths and is truncated
    When an owner-signed list arrives for prefix "" with limit 2 after the last returned path
    Then the request is accepted
    And the listing continues exactly after the previous page

  @list
  Scenario: A listing limit above 1000 is refused (p9 list_limit_overflow)
    Given the store holds the p8_cold edition at height 2
    When an owner-signed list arrives for prefix "" with limit 1001
    # A.8: listing ≤ 1000 paths/page — fail-closed, never clamped silently
    Then the response is 413 "payload_too_large"

  # ============================================================
  # A.3 — POST /batch: one multipart/mixed response, one part per
  # requested path, request order, per-part X-Aithos-Status.
  # ============================================================

  @batch
  Scenario: A mixed batch answers per-part statuses in request order (p9 batch_mixed)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated batch arrives for a covered path, a missing covered path and an uncovered path
    Then the request is accepted
    And the batch parts answer 200, 404 and 403 in request order
    And only the 200 part carries a body

  @batch
  Scenario: A batch over 256 paths is refused whole (p9 batch_overflow)
    Given the store holds the p8_cold edition at height 2
    When an owner-signed batch arrives with 257 paths
    # A.8: batch ≤ 256 paths — the whole request fails, no partial answer
    Then the response is 413 "payload_too_large"

  @batch
  Scenario: A batch body that is not the closed JSON form is refused (p9 batch_bad_body)
    Given the store holds the p8_cold edition at height 2
    When an owner-signed batch arrives with a non-JSON body
    Then the response is 400 "envelope_invalid"

  # ============================================================
  # A.3 — POST /sync: the changed-paths pack since a held edition.
  # manifest.json first part; a purged edition answers 410.
  # ============================================================

  @sync
  Scenario: Sync from the previous edition packs exactly the delta (p9 sync_delta)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an owner-signed sync arrives with have_edition 1
    Then the request is accepted
    And the pack opens with manifest.json and carries exactly the paths changed since edition 1

  @sync
  Scenario: Sync from the current edition packs only the manifest (p9 sync_current)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an owner-signed sync arrives with have_edition 2
    # nothing changed: the pack still opens with manifest.json — the client
    # re-checks the tip, the server never answers an empty 200
    Then the request is accepted
    And the pack carries manifest.json alone

  @sync
  Scenario: Sync from a purged edition is refused (p9 sync_gone)
    Given the store holds the p8_cold edition at height 2 without the edition 1 slot
    When an owner-signed sync arrives with have_edition 1
    Then the response is 410 "edition_gone"

  # ============================================================
  # A.1 redline gate 5 — the draft.2 servable layout. The frozen p8
  # read plan is the post-redline contract: these scenarios land it.
  # ============================================================

  @redline-paths
  Scenario: An anonymous reader gets a public K1-C section alias (p9 get_alias_public_anonymous)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an anonymous GET arrives for the p8_cold public section alias path
    # public/sections/** is the K1-C alias of the public zone: anonymous A2
    Then the request is accepted

  @redline-paths
  Scenario: A read.circle chain gets the circle blob alias (p9 get_alias_circle_mandated)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for the p8_cold circle blob alias path
    # the frozen p8 read plan: read.circle covers circle/blobs/<sid>.json
    Then the request is accepted

  @redline-paths
  Scenario: A valid chain reads the publication sidecars and edition slots (p9 get_sidecars_chain)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for the p8_cold changeset sidecar path
    Then the request is accepted
    When a correctly signed mandated GET arrives for "manifests/1.json"
    # "toute chaîne valide du DID": proof material, cold-verify without capability
    Then the request is accepted

  @redline-paths
  Scenario: The mandated reader stays denied outside its perimeter (p9 get_alias_denied)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for relative path "e/self/blobs/01000000000000000000000000.enc"
    # the frozen p8 read plan denial: unchanged by the redline
    Then the response is 403 "not_covered"

  @redline-paths
  Scenario: A bundle-internal key stays outside the wire grammar (p9 get_internal_key)
    Given the store holds the p8_cold edition at height 2 with its reachable objects
    When an owner-signed GET arrives for relative path "manifests/tree-2.json"
    # tree-/index-/-alt/gateway/gamma.jsonl: bundle-local, never on the wire
    Then the response is 400 "path_invalid"

  @redline-paths
  Scenario: No client writes an edition slot, owner included (p9 put_manifest_slot_denied)
    Given the store holds the p8_cold edition at height 2
    When an owner-signed PUT arrives for relative path "manifests/3.json" with a JSON body
    # in the A.1 grammar (not path_invalid) but no chain covers it in write:
    # the slot is written by the server on an accepted publish only
    Then the response is 403 "not_covered"

  @redline-paths @sidecars
  Scenario: A publication sidecar deposits under its own digest (p9 put_changeset_ok)
    Given the store holds the p8_cold edition at height 1 before its draft.2 publication
    When the owner deposits the p8_cold changeset sidecar at its digest path
    Then the request is accepted

  @redline-paths @sidecars
  Scenario: A sidecar whose name is not its content digest is refused (p9 put_changeset_id_mismatch)
    Given the store holds the p8_cold edition at height 1 before its draft.2 publication
    When the owner deposits the p8_cold changeset sidecar at a wrong digest path
    # content-addressing is the path's definition (redline gate 5): the K1-C
    # digest is recomputed on the deposited bytes — anti-abuse, no semantics
    Then the response is 400 "artifact_invalid" with reason "id_mismatch"

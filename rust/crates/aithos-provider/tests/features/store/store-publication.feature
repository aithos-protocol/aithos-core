@publication
Feature: Publication — mandated authorization, deposit verification and the two-head CAS
  # INFRA-PROVIDER annexe A: A.2 #7-#10 (mandated authorization), A.4 (deposit
  # verification, delegated to core/bundle), A.5 (CAS on the two hot heads:
  # manifest chain_hash and gamma head), A.7 (closed error registry).
  #
  # This is the P2 publication contract, written BEFORE the code (rituel BDD).
  # Every scenario is @wip until its gate lands it; each reject NAMES its closed
  # A.7 code. The store verifies like a verifier and serializes a CAS — it never
  # derives a semantic verdict from the façade's typed facts, and it NEVER
  # arbitrates a fork (§3.1 doctrine: covers() is anti-abuse, not authority).
  #
  # Gate map (each step = its STOP):
  #   gate 3  authorization  -> the five p1-deferred cases turn green, byte-exact
  #   gate 4  cas, artifacts -> PUT manifest/cert + POST /gamma via the façade
  #   gate 7  witness        -> the observation hook fires on an accepted publish
  #
  # Fixtures are the committed vectors: p1 (owner #root, the mandate, its gamma)
  # for authorization, and p7 (REAL aithos-bundle export_keyless packages — never
  # re-invented crypto) for the draft.2 publications and their CAS facts.

  Background:
    Given the tenant "acme" is enrolled and bound to the vector DID
    And the vector did.json is stored for that DID
    And the service authority is "store.aithos.fr"

  # ============================================================
  # A.2 #7-#10 — mandated request authorization
  # The barrier P1 defers at envelope.rs #9 (chain_invalid); P2 branches
  # verify_chain / verify_operation_facts (leaf #7, signature #8, chain #9,
  # covers() anti-abus #10). These five are the p1-deferred cases, replayed
  # byte-exact against p1 at the gate.
  # ============================================================

  @authorization @p1-deferred
  Scenario: A mandated GET inside the read perimeter is accepted (accept_get_mandated)
    Given the server clock reads "2026-07-16T12:10:30Z"
    And the gamma log carries the mandate grant and its bound action
    And the covered circle blob of the p1 vector is stored
    When a correctly signed mandated GET arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
    Then the request is accepted

  @authorization @p1-deferred
  Scenario: A mandated request signed after the mandate window is refused (reject_window_expired)
    Given the server clock reads "2026-09-01T00:00:00Z"
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
    # the mandate not_after is 2026-08-01: the window fails at `at` (A.2 #9)
    Then the response is 403 "chain_invalid"

  @authorization @p1-deferred
  Scenario: A mandated request whose envelope key is not the chain leaf is refused (reject_key_leaf_mismatch)
    Given the server clock reads "2026-07-16T12:13:00Z"
    And the gamma log carries the mandate grant and its bound action
    When a mandated GET signed by a key that is not the chain leaf arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
    # feuille.grantee.pubkey != key (A.2 #7)
    Then the response is 403 "chain_invalid"

  @authorization @p1-deferred
  Scenario: A mandated request under a revoked chain is refused (reject_chain_revoked)
    Given the server clock reads "2026-07-16T13:05:00Z"
    And the gamma log carries the mandate grant, its bound action and an owner revoke at "2026-07-16T13:00:00Z"
    When a correctly signed mandated GET arrives for relative path "e/circle/blobs/01000000000000000000000000.enc"
    # revocation is evaluated at now_serveur on the stored gamma (A.2 #9)
    Then the response is 403 "chain_revoked"

  @authorization @p1-deferred
  Scenario: A mandated request outside its perimeter is refused (reject_not_covered)
    Given the server clock reads "2026-07-16T12:12:00Z"
    And the gamma log carries the mandate grant and its bound action
    When a correctly signed mandated GET arrives for relative path "e/self/blobs/01000000000000000000000000.enc"
    # perimeter reads circle only; e/self/** is default-denied (A.3 covers())
    Then the response is 403 "not_covered"

  @authorization
  Scenario: The owner fragment covers every path on its own DID
    # e/self/** is exactly the perimeter NEITHER the anonymous reader nor
    # the p1 mandate reaches — only the owner's total coverage serves it.
    Given the server clock reads "2026-07-16T12:00:00Z"
    And an owner-signed PUT stored "sealed-self-bytes" at relative path "e/self/blobs/01000000000000000000000000.enc"
    When an owner-signed GET arrives for relative path "e/self/blobs/01000000000000000000000000.enc"
    Then the request is accepted

  # ============================================================
  # A.5 — CAS on manifest.json (publish). Head = sha256: + chain_hash of the
  # current manifest (JCS, signature.value=""). If-Head: sha256:<64hex> | none.
  # Mandatory on publish; mismatch answers 409 + current head + height; missing
  # answers 428. Real draft.2 manifests from p7 (aithos-bundle export_keyless).
  # ============================================================

  @cas @publish
  Scenario: Genesis publish advances the store to height 1 (p7 genesis_publish)
    Given the store holds no manifest head
    And the bundle-exported genesis publication package is loaded from p7
    When the owner publishes the loaded manifest with If-Head "none"
    Then the request is accepted
    And the stored manifest head becomes the package new_manifest_head at height 1

  @cas @publish
  Scenario: A publish over the matching head advances the edition (p7 publish_ok)
    Given the store holds the p7 genesis edition at height 1
    And the bundle-exported height 2 publication package is loaded from p7
    When the owner publishes the loaded manifest with If-Head the stored manifest head
    Then the request is accepted
    And the stored manifest head becomes the package new_manifest_head at height 2

  @cas @publish
  Scenario: A publish without If-Head is refused, never silently overwritten (p7 publish_cas_required)
    Given the store holds the p7 genesis edition at height 1
    And the bundle-exported height 2 publication package is loaded from p7
    When the owner publishes the loaded manifest with no If-Head
    Then the response is 428 "cas_required"

  @cas @publish
  Scenario: A publish over a stale head loses the race and gets the current head back (p7 publish_cas_stale)
    Given the store holds the p7 height 2 edition at height 2
    And the bundle-exported stale height 2 publication package is loaded from p7
    When the owner publishes the loaded manifest with If-Head the p7 genesis head
    # the loser rebases (§02.6); the store never arbitrates
    Then the response is 409 "cas_mismatch" carrying the stored manifest head at height 2

  @wip @cas @publish @delegated
  Scenario: A delegated author with authorized_by may publish under the CAS
    Given the store holds no manifest head
    And the bundle-exported delegated genesis publication package is loaded from p7
    When a delegated author publishes the loaded manifest with If-Head "none"
    Then the request is accepted
    And the stored manifest head becomes the package new_manifest_head at height 1

  # ============================================================
  # A.5 — CAS on the gamma head (POST /gamma, one entry). Head = sha256: of the
  # last entry JCS — the value the next entry carries as prev. Same 409/428 rule.
  # ============================================================

  @cas @gamma
  Scenario: The genesis gamma append advances the empty log (p7 append_genesis)
    Given the gamma log is empty
    And the committed p7 genesis gamma entry is loaded
    When a grantee appends the loaded gamma entry with If-Head "none"
    Then the request is accepted
    And the stored gamma head becomes the loaded entry head

  @cas @gamma
  Scenario: A gamma append over the matching head advances the log (p7 append_ok)
    Given the store holds the p7 gamma head after the grant entry
    And the committed p7 bound-action gamma entry is loaded
    When a grantee appends the loaded gamma entry with If-Head the stored gamma head
    Then the request is accepted
    And the stored gamma head becomes the loaded entry head

  @cas @gamma
  Scenario: A gamma append without If-Head is refused (p7 append_cas_required)
    Given the store holds the p7 gamma head after the grant entry
    And the committed p7 bound-action gamma entry is loaded
    When a grantee appends the loaded gamma entry with no If-Head
    Then the response is 428 "cas_required"

  @cas @gamma
  Scenario: A gamma append over a stale head is refused with the current head (p7 append_cas_stale)
    Given the store holds the p7 gamma head after the bound action
    And the committed p7 concurrent gamma entry is loaded
    When a grantee appends the loaded gamma entry with If-Head the p7 grant head
    Then the response is 409 "cas_mismatch" carrying the stored gamma head

  # ============================================================
  # A.4 — deposit verification ("the server verifies before accepting").
  # Delegated to core/bundle; the store corrects nothing, completes nothing,
  # rewrites nothing. A rejected artifact answers artifact_invalid + short reason.
  # ============================================================

  @artifacts @manifest
  Scenario: A manifest that does not chain the stored head is refused (p7 publish_prev_hash_mismatch)
    Given the store holds the p7 genesis edition at height 1
    And a draft.2 manifest whose prev_hash does not name the stored head is loaded from p7
    When the owner publishes the loaded manifest with If-Head the stored manifest head
    Then the response is 400 "artifact_invalid"

  @artifacts @manifest
  Scenario: A manifest whose signature does not verify is refused
    Given the store holds no manifest head
    And a draft.2 genesis manifest with a corrupted signature is loaded from p7
    When the owner publishes the loaded manifest with If-Head "none"
    Then the response is 400 "artifact_invalid"

  @artifacts @cert
  Scenario: A well-formed mandate certificate is accepted at deposit
    Given the bundle-exported mandate certificate is loaded from p7
    When a delegated author deposits the loaded certificate
    Then the request is accepted

  @artifacts @cert
  Scenario: A certificate whose subject is not the path DID is refused
    Given a mandate certificate whose subject is a foreign DID is loaded from p7
    When a delegated author deposits the loaded certificate
    Then the response is 400 "artifact_invalid"

  @artifacts @gamma
  Scenario: A gamma entry whose signature does not verify is refused (append_bad_entry_signature)
    Given the store holds the p7 gamma head after the grant entry
    And a bound-action gamma entry with a corrupted signature is loaded from p7
    When a grantee appends the loaded gamma entry with If-Head the stored gamma head
    # CAS passes, the entry itself fails A.4 verification
    Then the response is 400 "artifact_invalid"

  @artifacts @opaque
  Scenario: An opaque blob is stored without any content inspection (§3.1)
    Given the server clock reads "2026-07-16T11:20:00Z"
    When an owner-signed PUT arrives for relative path "e/circle/blobs/01000000000000000000000000.enc" with an opaque ciphertext body
    Then the request is accepted

  @artifacts @light-form
  Scenario: A zone index that is not JSON where JSON is required is refused
    Given the server clock reads "2026-07-16T11:20:00Z"
    When an owner-signed PUT arrives for relative path "e/circle/index.json" with a non-JSON body
    Then the response is 400 "artifact_invalid"

  @artifacts @no-fork
  Scenario: A merge manifest naming two predecessors is accepted without the store choosing a winner
    Given the store holds two competing editions at height 2
    And the bundle-exported merge publication package for height 3 is loaded from p7
    When the owner publishes the loaded manifest with If-Head the stored manifest head
    # merges/resolves_fork are accepted as-is: the CAS serializes, the witness
    # observes, the losers rebase client-side. The store never arbitrates a fork.
    Then the request is accepted
    And the stored manifest head becomes the package new_manifest_head at height 3

  # ============================================================
  # A.4 — did.json deposit (étape 5). Genesis: accepted when the #root
  # envelope verifies under the root key OF THE DEPOSITED DOCUMENT and the
  # control plane lists the DID for the tenant (the enrollment always
  # precedes — no chicken-and-egg). Replacement: verified under the stored
  # document's succession key (interim reading of §01.4 — the §10.4
  # epoch-artifact question is a named arbitrage, never resolved silently).
  # ============================================================

  @artifacts @did @genesis
  Scenario: A genesis did.json verifies under its own deposited root key (p9 did_genesis_ok)
    Given the tenant binds the p9 genesis DID with no stored document
    And the p9 genesis did.json is loaded
    When the genesis owner deposits the loaded did.json
    Then the request is accepted

  @artifacts @did @genesis
  Scenario: A genesis did.json whose id is not the path DID is refused (p9 did_genesis_id_mismatch)
    Given the tenant binds the p9 genesis DID with no stored document
    And a genesis did.json whose id names a foreign DID is loaded
    When the genesis owner deposits the loaded did.json
    Then the response is 400 "artifact_invalid" with reason "id_mismatch"

  @artifacts @did @genesis
  Scenario: A genesis envelope not signed by the deposited root key is refused (p9 did_genesis_wrong_signer)
    Given the tenant binds the p9 genesis DID with no stored document
    And the p9 genesis did.json is loaded
    When a foreign key deposits the loaded did.json
    # the genesis exception resolves #root against the DEPOSITED document:
    # a signature under any other key fails A.2 #8
    Then the response is 401 "signature_invalid"

  @artifacts @did @rotation
  Scenario: A did.json replacement verifies under the stored succession key (p9 did_rotation_ok)
    Given the store holds the vector did.json for the vector DID
    And a successor did.json signed under the stored succession key is loaded
    When the owner deposits the loaded did.json
    Then the request is accepted

  @artifacts @did @rotation
  Scenario: A did.json replacement signed by #root is refused (p9 did_rotation_root_signer)
    Given the store holds the vector did.json for the vector DID
    And a successor did.json signed under the stored root key is loaded
    When the owner deposits the loaded did.json
    # a stolen root can never steal the identity's future (§01.4): only the
    # previous document's succession key authorizes a replacement
    Then the response is 400 "artifact_invalid" with reason "signature"

  # ============================================================
  # A.4/A.5 — gamma segment replica (PUT, mode A). The stored segment must
  # be a byte-exact PREFIX of the new content; every added entry is
  # verified A.4; the segment head follows the same CAS rule.
  # ============================================================

  @cas @replica
  Scenario: A replica extending the stored segment by one verified entry advances the head (p9 replica_extend_ok)
    Given the store holds the p7 gamma segment after the grant entry
    And a replica appending the committed bound-action entry is loaded
    When the owner replicates the loaded segment with If-Head the stored gamma head
    Then the request is accepted
    And the stored gamma head becomes the appended entry head

  @cas @replica
  Scenario: A replica without If-Head is refused (p9 replica_cas_required)
    Given the store holds the p7 gamma segment after the grant entry
    And a replica appending the committed bound-action entry is loaded
    When the owner replicates the loaded segment with no If-Head
    Then the response is 428 "cas_required"

  @cas @replica
  Scenario: A replica over a stale segment head is refused with the current head (p9 replica_cas_stale)
    Given the store holds the p7 gamma segment after the bound action
    And a replica appending the committed concurrent entry over the grant head is loaded
    When the owner replicates the loaded segment with If-Head the p7 grant head
    Then the response is 409 "cas_mismatch" carrying the stored gamma head

  @cas @replica
  Scenario: A replica that does not preserve the stored prefix is refused (p9 replica_not_prefix)
    Given the store holds the p7 gamma segment after the grant entry
    And a replica rewriting the stored first entry is loaded
    When the owner replicates the loaded segment with If-Head the stored gamma head
    # the stored content must be a byte-exact prefix of the new content:
    # a replica never rewrites history (reason named by the p9 vector)
    Then the response is 400 "artifact_invalid" with reason "prefix_mismatch"

  @cas @replica
  Scenario: A replica whose added entry does not verify is refused (p9 replica_bad_added_entry)
    Given the store holds the p7 gamma segment after the grant entry
    And a replica appending a bound-action entry with a corrupted signature is loaded
    When the owner replicates the loaded segment with If-Head the stored gamma head
    Then the response is 400 "artifact_invalid" with reason "entry_signature"

  # ============================================================
  # Annexe C — witness hook. Kept behind its own gate: the observation fires on
  # an accepted publish/replica, but it is NEVER wired onto a publication feed
  # until this chantier freezes the canonical head (interdit). Contract only here.
  # ============================================================

  @wip @witness @gate7
  Scenario: An accepted publish queues one witness observation of the new head
    Given the store holds no manifest head
    And the bundle-exported genesis publication package is loaded from p7
    When the owner publishes the loaded manifest with If-Head "none"
    Then the request is accepted
    And one witness observation is queued for the new head at edition height 1

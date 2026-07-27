Feature: Durable hot enrollment of an Ethos
  A Gateway administrator admits a newly published Ethos from an already
  authorized context. Provider coordinates and local paths remain pinned in
  Gateway configuration, and the Gateway process never needs to restart.

  Scenario: An authorized owner admits an equipped Ethos without a restart
    Given a Gateway with "authority" authorized to enroll contexts
    And a published equipped Ethos named "support"
    When the authorized owner enrolls the "support" Ethos
    Then the enrollment is created and active
    And "support" is immediately visible through the signed control surface
    And the Gateway process identity did not change

  Scenario: Strict replay is idempotent
    Given a Gateway with "authority" authorized to enroll contexts
    And a published equipped Ethos named "support"
    When the authorized owner enrolls the "support" Ethos twice with fresh nonces
    Then the second enrollment reports the existing active context
    And the durable catalogue contains one "support" entry

  Scenario: A catalogue entry survives a Gateway restart
    Given a Gateway with "authority" authorized to enroll contexts
    And a published equipped Ethos named "support"
    And the authorized owner enrolled the "support" Ethos
    When the Gateway restarts over the same catalogue
    Then "support" is active before connector restoration

  Scenario: A name or DID collision is refused without mutation
    Given a Gateway with "authority" authorized to enroll contexts
    And a published equipped Ethos named "support"
    And the authorized owner enrolled the "support" Ethos
    When the authorized owner enrolls another DID as "support"
    Then the enrollment is refused as a context conflict
    And the original "support" context remains active

  Scenario: An owner outside the admission allowlist cannot enroll
    Given a Gateway with "authority" authorized to enroll contexts
    And a loaded context "neighbor" outside the enrollment allowlist
    And a published equipped Ethos named "support"
    When the "neighbor" owner enrolls the "support" Ethos
    Then the enrollment is refused as forbidden
    And no catalogue entry is written

  Scenario: A rotated Gateway identity invalidates a stale enrollment
    Given a Gateway with "authority" authorized to enroll contexts
    And a published equipped Ethos named "support"
    And the browser discovered the previous Gateway identity digest
    When the authorized owner enrolls after the Gateway identity rotates
    Then the enrollment is refused as a context conflict
    And no catalogue entry is written

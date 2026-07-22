@wip @g4 @rustls @release
Feature: Production TLS has one deterministic crypto provider
  The gateway selects rustls ring before constructing any client, listener,
  relay or ACME object, in targeted and workspace release builds alike.

  Scenario Outline: A release binary installs ring before TLS construction
    Given a clean <build> release binary
    When the gateway constructs its first TLS object
    Then rustls uses the explicit ring CryptoProvider without panic

    Examples:
      | build             |
      | gateway package   |
      | complete workspace|

  Scenario: Relay and ACME use the actual release binary
    Given the production release artifact and a staging certificate issuer
    When a real relay TLS connection and renewal are exercised
    Then the release binary reconnects without a CryptoProvider ambiguity

  Scenario: Debug artifacts cannot satisfy the production gate
    Given a successful debug build and no release provenance
    When the production artifact gate runs
    Then the artifact is refused as non-release


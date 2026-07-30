@wip @g4 @wasm @cli
Feature: Browser and CLI ceremonies use the same Core primitives
  Both clients verify, build and sign the closed delegated-session protocol.
  Neither surface invents JCS or attenuation rules or exports seed material.

  Scenario: The WASM API exposes the closed delegated-session surface
    Given caller-supplied entropy and an unlocked local signer
    When delegate_pubkey verify_mandate_chain build_session_submandate and sign_ceremony_challenge are called
    Then every mandate and signature is produced or verified by aithos-core
    And no function returns person or session seed material

  Scenario: Browser custody is pubkey-first
    Given an encrypted person keystore in browser-local custody
    When a WYSIWYS ceremony is signed
    Then only public keys signed protocol objects and the ceremony proof are posted
    And plaintext key material is zeroized without entering URL DOM storage or logs

  Scenario: CLI ceremony reads custody outside argv
    Given a local signer supplied through stdin a file descriptor or custody interface
    When the scripted delegated-session flow completes
    Then the CLI prints only URLs public keys ids and redacted verdicts
    And it executes the same verify build and sign primitives as WASM

  Scenario: Production session commands reject DEV seed arguments
    Given a production delegated-session command
    When private seed material is supplied in process arguments
    Then argument parsing refuses it before any protocol or network effect


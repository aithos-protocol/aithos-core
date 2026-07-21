# Lot A / P5 — le témoin (2026-07-20) : le dernier contrat non servi (C3)
# devient un service. Arbitrages Mathieu (session 2026-07-20, AskUserQuestion) :
#   ① déclencheur C.2 = DynamoDB Streams sur la table heads (le point de
#     sérialisation A.5) — un publish accepté avance la ligne, l'événement
#     porte l'observation ; un append gamma seul n'avance pas l'édition et
#     n'émet RIEN ;
#   ② publication C.3 = S3 + CloudFront sur witness.aithos.fr (feed
#     append-only par DID + racine quotidienne + keys.json) ;
#   ③ desired_count = 1 (un seul écrivain : le témoin signe des
#     observations, jamais de l'autorité — une interruption dégrade la
#     fraîcheur, jamais le produit) ;
#   ④ clé = KMS native Ed25519 sign-only, per annexe C.1 (le module
#     Terraform witness existe déjà ainsi ; le seam WitnessSigner reçoit
#     l'impl KMS au bin, la LocalWitnessSigner prouve le format ici).
#
# Le témoin OBSERVE : il lit la ligne heads (l'événement), va chercher le
# manifest observé dans le layout objets, recalcule son chain hash, et ne
# signe QUE ce qu'il a vu — un manifest absent ou discordant n'émet rien
# (jamais un checkpoint inventé). L'idempotence C.2 se déduit du feed
# lui-même (re-lisible au boot), jamais d'une mémoire de process. La vraie
# table, le vrai stream, le vrai KMS et le vrai bucket sont prouvés au
# gate déployé.

Feature: Witness service (annexe C — contrat C3, service P5)

  Background:
    Given a witness service over an in-memory feed signing with the p4 witness key
    And the store layout holds the p2 manifest chain for tenant "acme"

  Scenario: a publish accepted becomes a signed checkpoint in the DID feed
    When the heads stream delivers the publish of height 1 at "2026-07-16T11:05:00Z"
    Then the DID feed has exactly 1 line
    And the last feed line is a checkpoint of height 1 for the replay DID
    And the checkpoint's manifest_hash is the observed manifest's chain hash
    And the checkpoint's gamma_head is copied from the observed manifest
    And the last feed line verifies under the published key registry

  Scenario: a gamma-only advance emits no checkpoint
    When the heads stream delivers a gamma-only advance at "2026-07-16T11:06:00Z"
    Then the DID feed stays empty

  Scenario: re-observing the same edition in the same UTC day is idempotent
    When the heads stream delivers the publish of height 1 at "2026-07-16T11:05:00Z"
    And the heads stream delivers the publish of height 1 at "2026-07-16T18:00:00Z"
    Then the DID feed has exactly 1 line

  Scenario: the daily heartbeat re-signs the current head with a fresh observed_at
    When the heads stream delivers the publish of height 1 at "2026-07-16T11:05:00Z"
    And the daily heartbeat runs at "2026-07-17T00:05:00Z"
    Then the DID feed has exactly 2 lines
    And both feed lines are checkpoints of height 1 with the same manifest_hash
    And the last checkpoint's observed_at is "2026-07-17T00:05:00Z"
    And the two feed lines are freshness, never an equivocation

  Scenario: the heartbeat sweep covers every DID the heads table knows
    Given the store layout also holds the second replay DID at height 1
    When the daily heartbeat runs at "2026-07-17T00:05:00Z"
    Then each replay DID feed has exactly 1 line

  Scenario: a manifest absent from the layout emits nothing — the witness never invents
    When the heads stream delivers a publish whose manifest is missing at "2026-07-16T11:05:00Z"
    Then the DID feed stays empty
    And the observation is left pending for the next sweep

  Scenario: a manifest that does not match the announced head emits nothing
    When the heads stream delivers a publish whose stored manifest mismatches at "2026-07-16T11:05:00Z"
    Then the DID feed stays empty
    And the observation is left pending for the next sweep

  Scenario: a pending observation is emitted once the layout heals
    When the heads stream delivers a publish whose manifest is missing at "2026-07-16T11:05:00Z"
    And the missing manifest is deposited in the store layout
    And the pending sweep runs at "2026-07-16T11:07:00Z"
    Then the DID feed has exactly 1 line

  Scenario: two incompatible observations become a portable equivocation proof
    When the heads stream delivers the publish of height 2 at "2026-07-16T11:10:00Z"
    And the heads stream delivers the conflicting publish of height 2 at "2026-07-16T11:20:00Z"
    Then the DID feed has exactly 2 lines
    And the verifier finds an equivocation from the feed lines alone

  Scenario: the daily root covers every line of the UTC day, sorted and deduplicated
    Given the store layout also holds the second replay DID at height 1
    When the heads stream delivers the publish of height 1 at "2026-07-16T11:05:00Z"
    And the daily heartbeat runs at "2026-07-16T12:00:00Z"
    And the day rolls over at "2026-07-17T00:05:00Z"
    Then the daily root for "2026-07-16" is published
    And the daily root's n equals the day's distinct feed lines
    And the daily root verifies under the published key registry
    And rebuilding the daily root from the day's feed lines is byte-identical

  Scenario: a missed rollover is healed by the root sweep — roots are never lost
    When the heads stream delivers the publish of height 1 at "2026-07-16T11:05:00Z"
    And the root sweep runs at "2026-07-18T09:00:00Z"
    Then the daily root for "2026-07-16" is published
    And the daily root verifies under the published key registry

  Scenario: the published key registry is served signed as keys.json
    Then keys.json is published and lists the witness key
    And keys.json verifies under its own witness key

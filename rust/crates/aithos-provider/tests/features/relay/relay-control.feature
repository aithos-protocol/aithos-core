# Lot P7b — bascule relay (2026-07-20) : le relay lit ses mappings B.2 dans
# le control plane P7 derrière la couture ControlStore (memory|dynamodb,
# cache TTL 30 s), au lieu du bootstrap embarqué. Arbitrages Mathieu
# (session 2026-07-20) :
#   ① l'étape 4 de B.2 adopte l'ordre d'autorité gravé de B.5 : binding →
#     suspension du binding → état du TENANT (inconnu = mapping_mismatch,
#     suspendu = suspended) → correspondance exacte tenant/hostname ;
#   ② panne du backend control : les tunnels ACTIFS survivent (le balayeur
#     ne ferme jamais sur une erreur), les NOUVEAUX enregistrements
#     refusent `unavailable` — fail-closed sans décapiter le trafic ;
#   ③ B.4 « fermeture des tunnels < 60 s » : un balayeur de réconciliation
#     re-résout chaque tunnel actif contre la couture (période ≤ TTL/2) et
#     GoAway ce qui est suspendu / purgé / re-mappé — borne TTL + période
#     < 60 s, prouvée à l'horloge injectée, jamais au sleep.
#
# Le double scripté du harnais joue la table : mêmes réponses, mêmes pannes
# (motif Scripted de control.rs / gate contrat P7). La vraie table est
# prouvée au gate déployé.

Feature: Relay control-plane seam (annexe B.2 step 4 on the P7 control store)

  Background:
    Given a scripted control store binds gateway "z6MksPykuQeYh4zgthFRFBExrgo1dwFWWenY2TEJ9SvT9jn1" to tenant "acme" and hostname "demo.mcp.aithos.fr"
    And the tenant "acme" is active in the control store
    And a relay listens on the control store through a 30 s freshness cache with tunnel name "relay.test.aithos.fr"

  Scenario: a registration is served through the control seam
    Given a control-seam pod is registered and serving "demo.mcp.aithos.fr"
    Then the seam registry has an active tunnel for "demo.mcp.aithos.fr"
    And a public client is still served through the tunnel

  Scenario: a suspended tenant refuses registration through the B.5 authority join
    Given the tenant "acme" is suspended in the control store
    When a control-seam pod registers for hostname "demo.mcp.aithos.fr"
    Then the seam registration is refused with "suspended"

  Scenario: an orphan binding onto an unknown tenant answers mapping_mismatch
    Given the tenant "acme" is removed from the control store
    When a control-seam pod registers for hostname "demo.mcp.aithos.fr"
    Then the seam registration is refused with "mapping_mismatch"

  Scenario: a control store that cannot answer refuses new registrations fail-closed
    Given the control store stops answering
    When a control-seam pod registers for hostname "demo.mcp.aithos.fr"
    Then the seam registration is refused with "unavailable"

  Scenario: an active tunnel survives a control outage
    Given a control-seam pod is registered and serving "demo.mcp.aithos.fr"
    And the control store stops answering
    When the reconciliation tick runs past the freshness bound
    Then the tunnel stays active for "demo.mcp.aithos.fr"
    And a public client is still served through the tunnel

  Scenario: tenant suspension closes the active tunnel within the freshness bound
    Given a control-seam pod is registered and serving "demo.mcp.aithos.fr"
    And the tenant "acme" is suspended in the control store
    When the reconciliation tick runs past the freshness bound
    Then the seam pod's mux is closed by GoAway
    And the seam registry has no active tunnel for "demo.mcp.aithos.fr"

  Scenario: a purged enrollment closes the active tunnel and refuses re-registration
    Given a control-seam pod is registered and serving "demo.mcp.aithos.fr"
    And the gateway binding is removed from the control store
    When the reconciliation tick runs past the freshness bound
    Then the seam registry has no active tunnel for "demo.mcp.aithos.fr"
    When a control-seam pod registers for hostname "demo.mcp.aithos.fr"
    Then the seam registration is refused with "mapping_mismatch"

  Scenario: a binding remapped to another hostname closes the pinned tunnel
    Given a control-seam pod is registered and serving "demo.mcp.aithos.fr"
    And the gateway binding is remapped to hostname "moved.mcp.aithos.fr" in the control store
    When the reconciliation tick runs past the freshness bound
    Then the seam pod's mux is closed by GoAway
    And the seam registry has no active tunnel for "demo.mcp.aithos.fr"

  Scenario: the freshness cache bounds suspension propagation, never extends it past the TTL
    When a control-seam pod registers at instant 0 for hostname "demo.mcp.aithos.fr"
    And the tenant "acme" is suspended in the control store
    And a control-seam pod registers at instant 10 for hostname "demo.mcp.aithos.fr"
    Then the seam registration is accepted within the freshness bound
    When a control-seam pod registers at instant 31 for hostname "demo.mcp.aithos.fr"
    Then the seam registration is refused with "suspended"

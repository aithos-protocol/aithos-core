Feature: Mandates and the offline verifier
  A mandate is a pure certificate: it grants a keypair a perimeter, under
  constraints, for a window — verifiable from files alone, at an injected
  time T. The key material travels separately, as header lines. Delegation
  attenuates: policy (covers) and physics (derivation) must both agree.
  (spec 04, 05)

  Rule: A mandate is a pure certificate, verifiable offline

    Scenario: A grant verifies inside its window and dies at expiry
      Given an owner and an agent keypair
      When the owner grants the agent read on circle folder "projets" for 7 days
      Then the mandate verifies at day 1
      And the mandate is rejected at day 8

    Scenario: The kex binding is checked, not trusted
      Given a mandate whose kex_pubkey does not match its signing key
      Then mandate verification is rejected

  Rule: A grant delivers exactly its perimeter — certificate AND key

    Scenario: A folder grant opens the subtree
      Given a published bundle with section "note1" in circle "projets/perso"
      When the owner grants the agent read on circle folder "projets"
      Then the agent reads "projets/perso/note1" with its own keypair

    Scenario: The founding use case — a folder-local tag view grant
      Given circle sections "note1" tagged "toto" and "note2" untagged in folder "projets/perso"
      When the owner grants the agent read on folder "projets/perso" restricted to tag "toto"
      Then the agent reads "note1"
      But "note2" stays out of the agent's reach

    Scenario: An agent never reads outside its perimeter
      Given circle sections in sibling folders "projets/perso" and "projets/pro"
      When the owner grants the agent read on circle folder "projets/perso"
      Then the agent reads the section under "projets/perso"
      But the agent cannot read the section under "projets/pro"

  Rule: One keypair, one mandate, many perimeters

    Scenario: A single mandate grants several folders with no common root
      Given circle sections in folders "projets/perso" and "sante/dossiers"
      When the owner grants the agent read on folders "projets/perso" and "sante/dossiers" in one mandate
      Then the agent reads the section under "projets/perso" with its single keypair
      And the agent reads the section under "sante/dossiers" with the same keypair
      But a section under "archives" stays out of the agent's reach

    Scenario: The original cross-branch grant — two folders, one tag, one key
      Given tagged "toto" and untagged sections in both "projets/perso" and "sante/dossiers"
      When the owner grants read on both folders restricted to tag "toto" in one mandate
      Then the agent reads the tagged section of each folder with one keypair
      But every untagged section stays out of the agent's reach

  Rule: Delegation attenuates, offline, without the owner

    Scenario: A delegate re-grants a narrower perimeter on its own
      Given an agent granted read on circle folder "projets" with issue depth 1
      When the agent delegates read on folder "projets/perso" to a helper
      Then the helper's chain verifies
      And the helper reads the section under "projets/perso"

    Scenario: An over-wide sub-mandate is rejected
      Given an agent granted read on circle folder "projets/perso" with issue depth 1
      When the agent delegates read on folder "archives" to a helper
      Then the helper's chain is rejected

    Scenario: A sub-mandate cannot outlive its parent
      Given an agent granted read on circle folder "projets" for 7 days with issue depth 1
      When the agent delegates the same perimeter to a helper for 30 days
      Then the helper's chain is rejected

    Scenario: Exhausted depth cannot delegate further
      Given a helper at the end of a depth-1 chain
      When the helper tries to delegate to a fourth key
      Then the new chain is rejected

  Rule: The signed verb lattice maps operations without a create wire verb

    Scenario Outline: Existing verbs decide each section operation exactly
      Given an agent granted "<grant>" on one section perimeter
      When Core authorizes the canonical "<operation>" operation on that section
      Then the verdict is "<verdict>"
      And the signed perimeter contains no create verb

      Examples:
        | grant  | operation | verdict |
        | read   | create    | refused |
        | edit   | create    | refused |
        | delete | create    | refused |
        | append | create    | allowed |
        | write  | create    | allowed |
        | read   | edit      | refused |
        | delete | edit      | refused |
        | edit   | edit      | allowed |
        | append | edit      | allowed |
        | write  | edit      | allowed |
        | read   | delete    | refused |
        | edit   | delete    | refused |
        | append | delete    | refused |
        | delete | delete    | allowed |
        | write  | delete    | allowed |
        | edit   | read      | allowed |
        | append | read      | allowed |
        | delete | read      | allowed |
        | write  | read      | allowed |

  Rule: Mandate form is closed before signature trust

    Scenario Outline: A signed mandate with invalid form is rejected before its signature can authorize
      Given a mandate whose signature bytes are otherwise valid
      When its "<field>" has "<invalid_form>"
      Then mandate form validation is refused
      And no authorization helper returns a partial Allow

      Examples:
        | field                 | invalid_form                         |
        | protocol version      | unsupported version                   |
        | signature algorithm   | algorithm other than ed25519          |
        | announced signer key  | key different from the issuer         |
        | mandate id            | malformed mandate identifier          |
        | subject id            | malformed or chain-changing subject   |
        | parent and issued_by  | inconsistent issuer relationship      |
        | grantee public key    | malformed multibase key               |
        | kex public key        | mismatch with Ed25519 conversion      |
        | nonce                 | empty string                          |
        | not_before            | non-RFC-3339-Z timestamp              |
        | not_after             | earlier than not_before               |
        | issued_at             | non-RFC-3339-Z timestamp              |
        | selector              | duplicate dir, tag or id dimension    |
        | selector              | id combined with dir or tag           |
        | issue depth           | issue#depth=0                         |

  Rule: Grantee authority always joins possession and chain

    Scenario Outline: Neither a key nor a certificate chain authorizes alone
      Given a grantee operation with "<possession>" and "<chain>"
      When the pure verifier evaluates the same target and time
      Then the verdict is "<verdict>"

      Examples:
        | possession        | chain                  | verdict |
        | valid key proof   | valid mandate chain    | allowed |
        | valid key proof   | no mandate chain       | refused |
        | no key proof      | valid mandate chain    | refused |
        | wrong key proof   | valid mandate chain    | refused |
        | valid key proof   | revoked mandate chain  | refused |

    Scenario: Append-time and cold-time consume one mandate verdict
      Given a form-valid grantee operation, historical Gamma prefix and injected time
      When it is evaluated before append and replayed from the exported edition
      Then both paths return the same typed authorization verdict
      And revocation, constraints and proof of possession are present in both paths

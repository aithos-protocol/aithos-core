# Conformance vectors

The language-neutral contract of aithos-core: any implementation, in any
language, must reproduce these vectors byte for byte. They are normative at
promotion (spec §09.2).

## Rules

1. **Independent generation.** Whenever possible, expected values are
   produced by a second, unrelated implementation (e.g. Python `blake3` +
   `PyNaCl` + `base58`) so the Rust reference is cross-checked, never
   self-certifying. The generator used is named in `description`.
2. **One file per vector family**, named `<id>-<slug>.json` (`a1-genesis`,
   `b2-derivation`, …), ids matching docs/EXECUTION-PLAN.md steps — or a
   piste handoff's lots for prefixed families (`p*` = provider wire, lot P0
   of docs/HANDOFF-PROVIDER-AWS.md, normative annexes of INFRA-PROVIDER.md).
3. **Frozen once green.** A merged vector never changes; a spec change that
   would alter one requires a new vector id and an explicit spec redline.

## Schema

```jsonc
{
  "vector": "A1",                  // id, matches the plan step
  "description": "…",              // what it proves + how it was generated
  "<input fields>": "…",           // hex for raw bytes, multibase for wire keys
  "<expected fields>": "…"
}
```

Negative (fail-closed) cases carry `"must_fail": "<error variant>"` instead
of expected values.

## Encodings

- Raw byte inputs/outputs: lowercase hex, `_hex` suffix.
- Wire-format public keys: multibase base58btc over multicodec
  (`z6Mk…` ed25519-pub 0xed01, `z6LS…` x25519-pub 0xec01), `_multibase` suffix.
- JSON that gets signed/hashed: RFC 8785 (JCS) canonical form.

## CB2 qualification tranche (partial)

These CB2 families are executable qualification oracles, not frozen promoted
vectors yet. Their classified results are recorded one behavior per entry in
`cb2-core-bundle-red-ledger.json` (`frozen: false`, `partial: true`).

- `CB2-MANDATE-CONTRACTS`: historical E1/F1 byte stability, `id=` codec and
  containment, form validation, constraints, and homogeneous-version links.
  Generator: `gen-cb2-mandate-contracts.py`
  (`c51b53600d8a01b388d568d32c89217a80885447999ae929a08e44e21b933a48`);
  vector: `cb2-mandate-contracts.json`
  (`771eef3b92314a5cc6a37882a35cc81cbbb2b4e0d4976c4d555a10ba05cf1e3e`);
  Rust consumer: `aithos-core/tests/cb2_mandate_contracts.rs`.
- `CB2-MC1`: versioned `max_children`, direct-child accounting, homogeneous
  chains, and migration by complete reissuance. Generator:
  `gen-cb2-max-children.py`
  (`7f4c51ce86fced13811409a11e8a169a1ae29efac0069eeb20ffd28428a0085d`);
  vector: `cb2-max-children-versioning.json`
  (`b0f49be51b9ed2097234ad161f11a1b0af546e6ec4f8a99e1cc43c83eef5b1ec`);
  Rust consumer: `aithos-core/tests/cb2_max_children_versioning.rs`.
- `CB2-BUNDLE-VERSION-COEXISTENCE-1`: draft.1/draft.2 coexistence through a
  real `FsStore`, cold reopen, and mixed-version rejection. Generator:
  `gen-cb2-bundle-version-coexistence.py`
  (`06fceb108b697f0d0ee9c95a78d237c2bd55dc544db6b2da9f9d6ddf26fdc530`);
  vector: `cb2-bundle-version-coexistence.json`
  (`61b53d0765c278a56adfa1b35bee99f5144d8ceeb78430be59aa50a8a519ba3c`);
  Rust consumer:
  `aithos-bundle/tests/cb2_bundle_version_coexistence.rs`.
- `CB2-K1-OPERATION-FACTS-MUTATION-1`: K1.1-B operation/state commitment
  bytes and all 13 K1.2-M-B mutation domain/verb cases, plus 23 operation-facts
  and 15 state-fact fail-closed candidates using the approved
  `InvalidOperationFacts` / `InvalidStateFact` taxonomy. The store-key strings
  are opaque fixture inputs and define no Bundle path. Generator:
  `gen-cb2-operation-facts-mutation.py`
  (`6e1f460aa959389dcd44d2763f66a8839d7a8b8703813d728cd8191958e90b27`);
  vector: `cb2-operation-facts-mutation.json`
  (`dc3a57de91ff50895b680111da4da981e1518ad2dcd638d862e63be9eb29b83d`);
  Rust consumer:
  `aithos-core/tests/cb2_operation_facts_mutation.rs`
  (`09e233412bc4add875acc2839f48156f5a10394d18e4691fd20984b6a3d81098`).
- `CB2-K1-OPERATION-FACTS-READ-1`: K1.2-R-B Ethos, signed Gamma
  presentation, and vault-config read facts; existing manifest chain-hash
  preimage, canonical `read.gamma` request digest, vault state-key commitment,
  6 positive cases, and 21 fail-closed `InvalidOperationFacts` candidates.
  Generator: `gen-cb2-operation-facts-read.py`
  (`45c05f254c9cd1d682ee58d75f04911367155fcf01db8bc2c671e7b2dcafa3ba`);
  vector: `cb2-operation-facts-read.json`
  (`b8b4014b390fc3ebc65897f8b59d084da78c97137e9ad7f7703dbe5b0e0194db`);
  Rust consumer: `aithos-core/tests/cb2_operation_facts_read.rs`
  (`79f27ce5ea5511859edd5c9c24fd73e90d4f701468752f61b3f90cae72b51747`).
- `CB2-K1-OPERATION-FACTS-ACTION-INFERENCE-1`: K1.2-AI-B closed action
  and inference pre-effect facts; historical action `args_hash`, exact private
  inference-request and purpose commitments, distinct catalog/approval content
  addresses, all budget/purpose applicability combinations, 8 positive cases,
  and 35 fail-closed `InvalidOperationFacts` candidates. Catalog and approval
  documents are explicitly syntactic fixtures only; their signed tables remain
  separately gated. Generator:
  `gen-cb2-operation-facts-action-inference.py`
  (`481bbb75475813a6161b377e4bca28ddb385a146d9edf6f4cbc0ce2d98fdf901`);
  vector: `cb2-operation-facts-action-inference.json`
  (`4e1f319439ec4cc5faef2012a2a8aa198ba8d62fced2f330d2bbd13580c78b71`);
  Rust consumer:
  `aithos-core/tests/cb2_operation_facts_action_inference.rs`
  (`aab91fdc7cf66efe8e84bd0c6913dc58e599cc4d6799e7b55dde55655abb745e`).
- `CB2-K1-OPERATION-FACTS-STRUCTURAL-1`: K1.2-GRRP-B grant, revoke,
  standalone rotate, and normal/merge/resolution publication facts; complete
  signed-certificate and succession-transition content addresses, closed
  revocation reason variants, protected before/after state references,
  derived-rotation non-duplication, changeset-reference domain, causal
  operation references, 11 positive cases, and 43 fail-closed
  `InvalidOperationFacts` candidates. The changeset document is explicitly a
  syntactic fixture only. Generator: `gen-cb2-operation-facts-structural.py`
  (`244395a552b59116b77a75636ea20d0c2fc55847a97a7e9a735f157deb91f2f0`);
  vector: `cb2-operation-facts-structural.json`
  (`1be1d239055e5a66f0c64d3ca2c2b00cd102bac44a7cd8c2147d16dce8d275e3`);
  Rust consumer: `aithos-core/tests/cb2_operation_facts_structural.rs`
  (`c6ec4a78b33571f4b6ea58e73f0b1417f0dcf3fe9f674cfc1279148039077967`).

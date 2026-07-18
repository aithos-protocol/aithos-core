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

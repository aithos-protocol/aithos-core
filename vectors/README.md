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
- `CB2-D7-DELEGATED-COUNTS-1`: mandate draft3 `max_mutations` and
  `max_consumptions`, canonical occurrence correlation and cross-view
  deduplication, subtree accounting, closed non-zero leaves, the separate
  `delegated_counts` BLAKE3/Merkle root and v1 proof, 36 fail-closed
  `InvalidDelegatedCounts` candidates, and 13 draft3/attenuation
  `InvalidMandate` candidates. The oracle reproduces the frozen H2
  `gamma_counts_root` independently and never reinterprets it. Generator:
  `gen-cb2-delegated-counts.py`
  (`285606cc3aa299091f058957414ce148ae0cfcbc0af4844bfdeb74d300311997`);
  vector: `cb2-delegated-counts.json`
  (`c1edd459b00ff72f2693e54370a60d2c8b981c18ee10d213a4b26897ed2618f1`);
  Rust consumer: `aithos-core/tests/cb2_delegated_counts.rs`
  (`c16647a46331a303706c4a7bef731d71b2ffa74fb6717a08e405746d41b37f72`).
- `CB2-W1-OPERATION-PROJECTION-1`: complete owner/grantee W1/A1/K1
  projections without SC1, full signed-certificate content addressing,
  zero/one/two history heads, operation commitment and closed reference bytes,
  distinct occurrence anchors, cross-view correlation/equivocation, 32
  projection negatives and 6 reference negatives with the approved
  `InvalidOperation` / `InvalidOperationFacts` boundary. SC1 certificate and
  proof bytes remain explicitly outside this family. Generator:
  `gen-cb2-operation-projection.py`
  (`79e2872527d360e9856ef60eed90190179566381c934906bc553e0e52ad239ed`);
  vector: `cb2-operation-projection.json`
  (`99bc175e0b4f07dece0afa828fff22be9be75d4b66f8b00fee3e7d427021e14c`);
  Rust consumer: `aithos-core/tests/cb2_operation_projection.rs`
  (`06ddf1066172e8b129515425eceefc9460f3d70c713d2a09fc0e74067e553972`).
- `CB2-SC1-SESSION-PROOF-1`: real signed draft2 `session_bind` mandate,
  complete leaf-signed SC1 certificate and digest, session-bound W1 projection,
  exact session proof, and explicit long-term/session double possession. The
  native leaf proof is a test-only diagnostic fixture, not a promoted carrier;
  `max_sessions` lifecycle remains separately gated. All 29 mono-defect
  candidates require `InvalidSession`. Generator: `gen-cb2-session-proof.py`
  (`37f8fde6cd69e8ed05d9bb0b2414e676694e2c3a614ff2ad06eaa134259784a6`);
  vector: `cb2-session-proof.json`
  (`17553dd95f515e8045e17e8e46816b1d7e2007d4985eec0048fcac34197bef74`);
  Rust consumer: `aithos-core/tests/cb2_session_proof.rs`
  (`03df1111aa1945e27684bf29a55e8ca7772617f5ab350baaa738dd302eda1b8e`).
- `CB2-R2-U1-OPERATION-RECEIPTS-1`: exact operation-bound R2 obligation
  receipts, U1 action/inference usage receipts, Ed25519 preimages with `sig`
  omitted, actual-usage totals, and the closed homogeneous-draft3 non-action
  obligation matcher. The oracle preserves historical v1 receipts, covers both
  R2 optional-presentation tables and all three family literals, and rejects 25
  R2, 31 U1, and 24 matcher/chain defects with the established exact variants.
  Generator: `gen-cb2-operation-receipts.py`
  (`4247b6485ffba02b860d079fdf60da84bb6d431f9aed5ebbb257b95147c034b3`);
  vector: `cb2-operation-receipts.json`
  (`2ce3d53bda43dc28ce599a8f7ec97d0050c3bf61b8f9ade4b51e8a74336ff22c`);
  Rust consumer: `aithos-core/tests/cb2_operation_receipts.rs`
  (`4279a897f6adb5ac7189f9a6f087adc8e6acc82f13af1e533cb2d59c632af300`).
- `CB2-CAT1-CONNECTOR-CATALOG-1`: exact signed connector catalogue and
  distinct owner-content approval tables, complete signed-document content
  addresses, homogeneous draft3 `catalog_pins`, K1.2 `catalog_ref` binding,
  and closed `read`/`act`/`binding` authorization decisions. The independent
  cryptography oracle rejects 27 catalogue, 22 approval, 19 chain and 8
  action-facts defects with the approved exact variants while preserving five
  historical vector hashes. Generator: `gen-cb2-connector-catalog.py`
  (`4bd896da63a10ced75b263feadb711a7895125de8dec65786b51a3de3bec3ab0`);
  vector: `cb2-connector-catalog.json`
  (`f73b35d29602217983c6401f06fbb49c73032a955d0ac14356d3a988181fe43c`);
  Rust consumer: `aithos-core/tests/cb2_connector_catalog.rs`
  (`56ec7a150fccf9da9bdcaf365b9ea9cc33c116908e64c592195da437bb6e4a22`).
- `CB2-GAMMA-V2-SEMANTIC-REPLAY-1`: signed Gamma v2 top-level
  `operation_ref` presence for the 12-kind closed registry, exact W1
  correlation, monotone manifest/Gamma causal edges, mixed-profile merge,
  replay/equivocation admission, unchanged raw H2 line accounting and the
  semantic replay decision inventory. The independent cryptography/blake3
  oracle preserves five historical vector hashes and rejects 35 entry plus 2
  correlation defects with exact variants. Generator:
  `gen-cb2-gamma-v2-replay.py`
  (`558fa9320caf570363365222457bdf1acd9b059a0719445ac4c899ebfcbd3207`);
  vector: `cb2-gamma-v2-replay.json`
  (`a3cc536ea452940af061ce421c238e08f0894923562b8c8193dbb8d8b853cd06`);
  Rust consumer: `aithos-core/tests/cb2_gamma_v2_replay.rs`
  (`fa2593db199d6cbb2c68bfecd170f95f06826c7bfadd9744ce9aa48de6434e15`).
- `CB2-K1-C-DRAFT2-CARRIERS-1`: one coherent signed draft2 candidate with
  exact W1/K1.2 facts, five contained operations, last-writer changeset
  attribution over five changed Store keys, all five K1-C evidence variants,
  a six-consumption/two-mutation D7 leaf, canonical changeset/evidence
  references and sidecar paths, ordinary file pins, and the acyclic
  publication occurrence. The independent cryptography/blake3 oracle preserves
  eleven historical vector hashes and rejects 32 carrier/correlation defects as
  `InvalidOperation` plus 5 signed-manifest form defects as
  `InvalidDidDocument`. Generator: `gen-cb2-draft2-carriers.py`
  (`d0fbc85c29b2fe06362eec9ee7084f69746589697d44da9570ec94e3e8c8c6eb`);
  vector: `cb2-draft2-carriers.json`
  (`2e75e9af30ba0207bd01a6f347cac1a263f816a7ae0fb3d583f75beabef2badc`);
  Rust consumer: `aithos-bundle/tests/cb2_draft2_carriers.rs`
  (`9c392d0acd16fcc4eb5699cb0677f88f36b48b98239f99327430e59ef8f1e3dd`).
- `CB2-BUNDLE-BOUNDARIES-1`: pure-data G-B/G-C/G-D oracle covering twelve
  MemStore/FsStore failure boundaries, four deterministic recovery states, one
  logical linearization point, five accepted and fifteen refused confined
  paths, six purpose/context-bound opaque capability classes with eighteen
  substitution negatives, and eight fresh keyless cold-load decisions. It
  preserves seven historical vector hashes and promotes neither transaction
  metadata nor a capability encoding to signed wire. Generator:
  `gen-cb2-bundle-boundaries.py`
  (`4d2145459152bd9685c30a986acbc2d90869126f2d151e93777e3d8c8331aa4b`);
  vector: `cb2-bundle-boundaries.json`
  (`73149da64fbdc73bcfd81f8a3d11c83e9421e43f2053ad649bf0db0f585ee187`);
  Rust consumer: `aithos-bundle/tests/cb2_bundle_boundaries.rs`
  (`9ef5ae1c521b6902e799cbf00d1a6d9e2837db5d6a9f777e4ac0d4af8cd84666`).
- `CB2-BUNDLE-AUTHORITY-FLOWS-1`: pure-data CB8/CB9 oracle covering fifteen
  owner operations, eighteen zone-specific grantee decisions, nine exact grant
  delivery rows, four independent authority/decryption fences, public grantee
  authorship, three opaque self state relations, current-authority rechecks,
  six atomic refusals and three Gamma-read decisions. It preserves eight
  historical vector hashes and adds no signed wire. Generator:
  `gen-cb2-bundle-authority-flows.py`
  (`037f01cb6cd348f4afc52122105e90403eb71fdfb7de5290fd1513feae4ce392`);
  vector: `cb2-bundle-authority-flows.json`
  (`30545958c170fda12e53817d3c5b7adb295432a4352e1abcbc7749fdd5c7eca0`);
  Rust consumer: `aithos-bundle/tests/cb2_bundle_authority_flows.rs`
  (`cc48562545ef028b8042860de4fdb842f3dfe1e1f9a5658862c9e41a12344cdb`).

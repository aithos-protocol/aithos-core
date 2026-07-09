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
   `b2-derivation`, …), ids matching docs/EXECUTION-PLAN.md steps.
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

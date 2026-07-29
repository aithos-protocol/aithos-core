# Protocol decision — AID-001 Provider succession semantics

| Field | Value |
|---|---|
| Date | 2026-07-29 |
| Status | `DECIDED` |
| Finding | `AID-001` |
| Decision owner | human protocol owner |
| Selected model | §01.4 / §10.4 identity-epoch transition |
| Rejected model | succession-signed same-DID `did.json` replacement |

## Decision

1. A `DidDocument` is always signed by its own `#root` key and must pass the
   strict Core `DidDocument::verify` verdict.
2. The succession private key signs only `EpochTransition`. It never signs a
   DID document or a routine same-DID update.
3. The root key is bound into the DID identifier. A new root therefore creates
   a distinct `next_did`; root replacement under the previous DID is invalid.
4. A successor is accepted only when Core verifies the complete triplet:
   previous document, succession-signed transition, and root-signed successor
   document.
5. Provider must not persist or expose any part of a refused transition.

## Same-DID updates

This decision does not turn every metadata or location update into a new
identity epoch. If same-DID document updates are supported, they:

- remain signed by `#root`;
- preserve `id`, root, and the pinned succession authority;
- pass strict Core verification;
- follow the protocol's edition/CAS rules.

If those update rules cannot be proved from the existing protocol and storage
surface, Provider must reject a byte-different same-DID replacement
fail-closed rather than inventing another succession protocol.

## Required round 2 outcome

- Remove acceptance of succession-signed same-DID `did.json` replacement from
  `artifacts::deposit_did`.
- Replace or retire P9 `did_rotation_ok` and its root-signer inverse so the
  vectors express the decided semantics.
- Provide a real Provider path for the §10.4 triplet when a canonical existing
  artifact/storage path can be used.
- Reuse Core `DidDocument::verify` and
  `EpochTransition::verify_succession`; do not create a parallel verifier.
- Prove that malformed, mismatched, same-DID, wrong-signer, and partial-write
  cases fail closed.
- Keep AID-002 and AID-005 unchanged and do not address AID-003/AID-004.

If the repository does not define enough wire/storage semantics to expose the
epoch artifact without inventing a new public protocol, the corrector must
still remove the invalid same-DID path and report the missing transport/storage
decision explicitly in its handoff.

## Specification clarification

The shorthand in the threat model that describes publishing a successor DID
document “signed by the cold succession key” means that the succession key
authorizes publication through the epoch transition. The successor DID
document itself remains signed by its own root key.

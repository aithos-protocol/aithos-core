# 0 — Overview

> **Status: DRAFT.** Aithos Core, wire version `aithos-core: "1.0.0-draft.1"`.
> Rationale in `../DESIGN.md`. Everything in this series is enforceable from files
> alone; a server is never a trust party.

## 0.1 Terminology

| Term | Meaning |
|---|---|
| **Subject / owner** | The human the ethos describes; holder of the master seed. |
| **Ethos** | The subject's signed, partly encrypted profile: three zones of markdown sections. |
| **Zone** | `public` (plaintext), `circle`, `self` (encrypted). |
| **Section** | One markdown unit: `{id, title, tags, body}`. Id MAY be namespaced (`gmail:0042`). |
| **Node** | A protectable point of the content tree: a zone, a namespace, a tag view, a section. |
| **Node key (DK)** | The current symmetric key of a node. Random, published only via its header. |
| **Header** | Per granted node: the DK sealed one line per authorized identity (§03). |
| **Mandate** | A signed certificate granting scopes to a grantee keypair (§04). |
| **Chain** | A mandate plus its ancestors up to the owner's root (§05). |
| **Gamma** | The subject's hash-chained log of mutations and actions (§07). |
| **Edition** | One signed state of the bundle; editions form a linear chain (§02). |
| **Bundle** | The set of files: manifest, zone indexes, blobs, headers, certificates, gamma. |

## 0.2 The five normative invariants

1. **I1 — No stored secrets.** At rest, the bundle contains ciphertext, clear
   metadata, headers, certificates, and the log. No plaintext secret, no unsealed key.
2. **I2 — Credentials are immutable.** A grantee's keypair and mandate are never
   modified after issuance. All change happens in storage (headers, ciphertext).
3. **I3 — Owner line.** Every header MUST contain a line for the owner. A header
   without one is invalid, and so is the edition carrying it.
4. **I4 — Authority follows issuance.** Only the issuer of a mandate (or an ancestor
   in its chain, transitively up to the owner) may revoke it or remove its lines.
   Verifiable from certificates alone. A `revoke` perimeter entry (§04.2, §06.7)
   delegates the *certificate* half of this authority — never the key half — within
   attenuation.
5. **I5 — No silent actions.** Every mutation and every connector action performed
   under a mandate MUST be recorded as a gamma entry naming the mandate. An action
   without its entry is invalid; verifiers treat the entry count as the mandate's
   consumption meter.

## 0.3 Cryptographic profile

| Use | Algorithm |
|---|---|
| Derivation | BLAKE3 `derive_key(context, key)`; labels ASCII, prefix `aithos-core/v1/` |
| AEAD | XChaCha20-Poly1305 IETF, 24-byte CSPRNG nonces, purpose-bound AAD |
| Header lines & sealed payloads | X25519-HKDF-SHA256-AEAD (multi-line ECIES, one ephemeral per header revision) |
| Signatures | Ed25519 |
| Hashing / chaining | SHA-256 (edition & gamma chains), BLAKE3 elsewhere |
| Canonicalization | RFC 8785 JCS before signing or hashing JSON |

AAD convention, NUL-separated after the purpose label
`"aithos-core/v1/<purpose>"`: `subject_did ‖ node_path ‖ key_version` for content
purposes (`blob`, `tagwrap`, `vault`, `gamma-payload`), `subject_did ‖ header_path ‖
key_version` for `header-line`. Purposes never overlap.

## 0.4 Version discriminators

`aithos-core` in the manifest; `aithos-mandate-core` in certificates; `aithos-gamma-core`
in log files. All `"1.0.0-draft.1"` in this series. Nothing is inherited from
`Aithos-protocol` unless restated here; identity (DID document format) is restated in
§01 with minimal carry.

## 0.5 Transverse principles

**Revocation is the revoker's atomic act.** Cutting a grantee is performed by the
revoker — the owner or an ancestor of the revoked mandate. Attenuation guarantees the
revoker's perimeter covers the revoked one, so it already holds every key of every
node to rotate (no escalation), and it is present by definition (it signs the entry).
No server, no custodian, no key reserve, no pre-generated material is ever required
(§06.2). Corollary invariant: the *policy* cut — publishing a revocation entry — is
delegable to an actor holding no content key at all (§06.7); the *cryptographic* cut —
rotating a DK — is by mathematical nature the act of a key holder. No automaton can
turn a lock it cannot open.

**Absentee owner.** The owner is a root of authority, not an availability dependency.
The target profile is an owner who issues a broad mandate and then almost never
returns. Every maintenance duty (rotation, wrap repair, lazy re-encryption) is
therefore recursive: it falls on the manager of the node concerned — whoever holds a
perimeter there and issues grants on it — never on the owner. What remains
structurally owner-only is reduced to a minimum and listed in §10.9.

## 0.6 Reading order

01 (keys) → 02 (content) → 03 (headers) → 04 (mandates) → 05 (delegation) →
06 (revocation) → 07 (gamma) → 08 (connectors) → 09 (CLI & conformance) →
10 (threat model).

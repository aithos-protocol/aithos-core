# 0 — Overview

> **Status: DRAFT.** Aithos Core, specification revision `2026-08-03-i3-authority`.
> Manifest publication profiles: `aithos-core: "1.0.0-draft.1"` (historical
> verification) and `"1.0.0-draft.2"` (current issuance) — §0.4.
> Rationale in `../DESIGN.md`. Everything in this series is enforceable from files
> alone; a server is never a trust party.

## 0.1 Terminology

| Term | Meaning |
|---|---|
| **Subject / owner** | The human the ethos describes; holder of the master seed. |
| **Ethos** | The subject's signed, partly encrypted profile: three zones of markdown sections. |
| **Zone** | `public` (plaintext), `circle`, `self` (encrypted) — each the root folder of its tree. |
| **Folder** | A tree node of unlimited depth: `{sid, name, children}`; contains folders and sections freely (§02). |
| **Section** | One markdown unit: `{sid, name, title, tags, body}`, living in a folder. `gmail:0042` is sugar for folder `gmail/`, section `0042`. |
| **Node** | A protectable point of the content tree: a zone (root folder), any folder, a tag view (zone-root or folder-local), a section. |
| **Node key (DK)** | The current symmetric key of a node. Random, published only via its header. |
| **Header** | Per granted node: the DK sealed one line per authorized identity (§03). |
| **Mandate** | A signed certificate granting scopes to a grantee keypair (§04). |
| **Chain** | A mandate plus its ancestors up to the owner's root (§05). |
| **Head mandate** | A broad, long-lived mandate issued directly by the owner to run unattended; carries a dead-man heartbeat by default (§04.8). |
| **Gamma** | The subject's hash-chained log of mutations and actions (§07). |
| **Edition** | One signed state of the bundle; editions form a linear chain (§02). |
| **State root** | Per-zone Merkle root over node hashes, pinned by each signed manifest; O(log n) inclusion proofs (§02.10). |
| **Bundle** | The set of files: manifest, zone indexes, blobs, headers, certificates, gamma. |

## 0.2 The five normative invariants

1. **I1 — No stored secrets.** At rest, the bundle contains ciphertext, clear
   metadata, headers, certificates, and the log. No plaintext secret, no unsealed key.
2. **I2 — Credentials are immutable.** A grantee's keypair and mandate are never
   modified after issuance. All change happens in storage (headers, ciphertext).
3. **I3 — Owner line.** Every `key_versions[*].lines` of every header MUST contain
   the owner line: the line whose recipient key is the subject's `owner_kex`, as
   published in the DID document (§01.1, §01.4, §03.1). A header without one is
   invalid. An edition verifier MUST parse every header the edition pins and MUST
   reject the edition if any key version of any of them has no owner line. The
   routing label `to` never establishes the owner line and never satisfies I3.
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

`aithos-core` in the manifest; `aithos-mandate-core` in certificates; numeric `v`
on each Gamma entry.

The manifest/Gamma publication plane has two monotone profiles:

- manifest `"1.0.0-draft.1"` introduces only historical Gamma v1 entries. Their
  signed bytes and verification semantics remain unchanged;
- manifest `"1.0.0-draft.2"` introduces only Gamma v2 entries and the K1-B
  operation, changeset, and evidence references (§02.6.2, §07.1.1). It MAY retain
  byte-identical draft1/v1 ancestry.

Version order is causal, never inferred from physical JSONL order: draft1/v1 may
lead to draft1/v1 or draft2/v2, while draft2/v2 never leads back. Missing, mixed on
one introducing edge, or unknown profiles fail closed. Historical manifests and
entries are never rewritten or assigned synthetic references.

A profile gates the introduction of signed constructs; it never gates a verification
rule. The I3 obligation of §0.2 introduces no signed construct and changes no signed
byte: it binds every `aithos-core` profile, historical ones included. A rule that
bound only the newest profile would be escaped by publishing under an older one, and
would bind nothing.

That binding is over profiles, not over time. A verifier applies I3 to the edition it
is presented with — the head of the chain it is asked to verify — whatever profile
that edition declares. It does not walk the chain re-verifying superseded editions
under a rule that postdates them. Two reasons, and the second is the load-bearing
one. First, what I3 protects is that the owner can reach the current state; a
superseded edition is not the state anyone reads from. Second, an edition already
published cannot be brought into conformance: the remedy is to rewrite its header,
which changes a signed byte, and this section forbids rewriting historical manifests
and entries. A rule no one can satisfy is not a tightening, it is a trap. The
obligation therefore reaches every profile and every future edition, and stops at the
boundary this specification itself draws around the past.

The mandate plane separately supports two currently issuable semantic profiles:

- `"1.0.0-draft.1"` is the historical verification profile. Its signed bytes and
  attenuation semantics stay frozen by the historical vectors, including the E+
  `max_children` drop case.
- `"1.0.0-draft.2"` is the current issuance profile. It introduces the T1
  `max_children` rule specified in §04.4 and §05.3 without reinterpreting a
  `draft.1` certificate.

Every certificate in one delegation chain MUST declare the same
`aithos-mandate-core` version; a mixed-version chain is invalid. Migration is
reissuance of a complete homogeneous chain under `draft.2`, following normal
issuance and Gamma rules. It never consists of rewriting the discriminator or
any other signed byte of a `draft.1` certificate. Distinct homogeneous `draft.1`
and `draft.2` chains MAY coexist in the same bundle. Mandate `draft.3` is reserved
for the independently versioned catalog and obligation-matcher additions approved
at K1-B; it is not issuable until those closed member tables and migration vectors
are fixed (§04, §08).

Nothing is inherited from `Aithos-protocol` unless restated here; identity (DID
document format) is restated in §01 with minimal carry.

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
structurally owner-only is reduced to a minimum and listed in §10.8.

## 0.6 Reading order

01 (keys) → 02 (content) → 03 (headers) → 04 (mandates) → 05 (delegation) →
06 (revocation) → 07 (gamma) → 08 (connectors) → 09 (CLI & conformance) →
10 (threat model).

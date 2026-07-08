# Aithos Core — Design

> **Status: DRAFT (greenfield).** This repository specifies the Aithos protocol core,
> rebuilt from a blank page around one goal: **a perfectly executed trust layer for
> agents** — identity, scoped mandates, delegation, revocation, and a tamper-evident
> action log — with cryptography that needs **no server** to be enforced. It
> supersedes, when promoted, the `Aithos-protocol` spec series (v0.x and the v2 draft).
> Scope of this edition: ethos zones (public/circle/self) with markdown sections and
> tags, the gamma log, and MCP connector authorization. No generic data store, no
> assets (they graft onto the same primitives later).

## 1. The five requirements

The design answers five requirements, set deliberately and in this order:

1. **One root.** The owner holds a single master secret; from it he can create keys on
   any perimeter and issue mandates.
2. **Recursive delegation.** Any key holder can create keys and mandates on *its own*
   perimeter, alone, offline.
3. **Scoped revocation.** A holder can cut keys *he* created without affecting any key
   he did not create — including keys whose perimeters contain or overlap his.
4. **Immutable credentials.** Ciphertext may be recomputed without limit; **delivered
   key material is never touched**. A grantee's keys are engraved for life.
5. **Autonomy.** Everything is manageable on files, by CLI, with no server or third
   party in any trust role.

Two physical limits are accepted upfront, because no cryptography escapes them: what a
party already read is theirs forever, and without an online gate, revocation takes
effect when re-encryption is published, not at the instant of decision.

## 2. The two planes

Every prior Aithos design entangled two questions: *what* a grant covers and *who*
holds it. The core separates them into two independent planes.

**The certificate plane (who).** A mandate is a signed certificate: issuer, grantee
public key, perimeter, verbs, constraints, validity, parent link. Certificates chain:
the owner's root signs first-level mandates; a mandate with delegation rights signs
sub-mandates. Authority is the chain: you may only revoke (or re-issue) inside your own
issuance subtree, and anyone can verify it from the public certificates alone.

**The content plane (what).** Content keys form a derivation tree. Each protected node
(a zone, a namespace, a tag view, a single section) has a current **node key**; all
keys below derive from it one-way (`K_child = derive(K_node, label)`). Whoever can
open a node reads everything under it — present and future — by pure local
computation.

The bridge between the planes is the **header**: per granted node, a small stored
object containing the node key sealed once per authorized identity — one line per
direct grantee, plus always one line for the owner. Grant = append a line. Revoke =
rotate the node key, republish the header without the line, re-encrypt. Nobody else
moves.

## 3. Why flat headers (and not fancier cryptography)

We evaluated hierarchical KDF trees with epoch regression and keybag refreshes (the v2
draft), server-assisted split keys (OPRF), broadcast encryption (NNL subset-cover),
proxy re-encryption, HIBE and ABE. The flat header wins at Aithos scale for reasons
worth recording:

- A node has dozens of grantees, not millions: one ECIES line each is smaller and
  simpler than any subset-cover, and **revocation shrinks the header** instead of
  growing it. The revoked leaves zero trace in future state.
- Grantee material reduces to **one keypair** — requirement 4 in its purest form. No
  trousseaux of tree positions, no epoch keys to refresh, no keybag to update. A
  grantee's entire state is `{keypair, mandate}`; that pair is also what makes an
  agent portable across machines.
- Adding a reader costs one line and touches nobody. Removing one costs one rotation
  plus re-encryption of exactly the revoked key's reach — the price tag follows the
  blast radius, which rewards least-privilege grants.
- Everything is X25519 + XChaCha20-Poly1305 + Ed25519 + BLAKE3. No pairings, no novel
  assumptions, auditable by any competent reviewer.

Server-assisted profiles (split-key σ) and subset-cover headers remain documented as
optional future modes; neither is needed for the core guarantees.

## 4. Revocation as a ladder

Revocation is not one act but a ladder, each rung buying more with more work:

| Rung | Act | Cuts | Cost |
|---|---|---|---|
| Expiry | nothing — dates are in the certificate | everything, at `not_after` | zero |
| Line removal + rotation | new node key, header without his line | all **future** content | one header |
| Re-encryption | bodies rewritten under the new key | all **existing** content | one pass over the node's bytes |
| Supersession | old editions replaced, GC'd | the past, except what he exfiltrated | storage churn |

Sub-delegation cascades: revoking a mandate invalidates its descendants (their chains
break); survivors the owner wants to keep are re-adopted with one header line each.

## 5. The gamma log as the agentic substrate

Every mutation and **every connector action** under a mandate writes one entry in the
subject's hash-chained gamma log — the *no silent actions* invariant. This makes the
log double as the enforcement substrate for agentic constraints that need counting:
`max_actions: 50` is verifiable by anyone by counting clear-skeleton entries bearing
the mandate id. Constraint enforcement is thus split three ways, explicitly: dates,
counts and chains are **verifier-enforced** (offline, from files); rate windows and
counter-signatures are **checked at verification and at execution**; anything about
the outside world (domains, spend) is **tool-host-enforced** and audited in gamma.

## 6. Agents, connectors, and the vault

A connector (gmail, linkedin…) contributes action scopes (`x.gmail.reply`) checked by
whatever executes the action against the presented mandate chain. Connector secrets
(OAuth tokens, settings) live in the **vault**: an ordinary protected node `/x/<id>`
with its header — the "guardian" of the vault is simply whoever holds a mandate line
on it. Because vault, bundle, and certificates are files, and a grantee's state is one
keypair plus one mandate, **an agent is portable by construction**: copy its keypair,
its mandate, and point it at any mirror of the bundle.

## 7. What the server is not

The protocol assumes no server. Editions form an owner-rooted, signed, linear chain
(height + prev-hash); any mirror can host the files; forks are detectable by anyone
and resolved only by an owner-signed checkpoint. Verifiers — every reader, every CLI —
check signatures, verbs, perimeters, header well-formedness (owner line present,
exclusive authority respected) and reject invalid editions. A provider, when one
exists, is a mirror with a serialization convenience and an optional early-rejection
gate; it holds no key material and no trust role. That decision is recorded here so
the platform work that follows cannot silently reacquire one.

## 8. Map

| Spec | Contents |
|---|---|
| `spec/00-overview.md` | Normative summary, terminology, versioning |
| `spec/01-identity-and-keys.md` | Master seed, signing roots, grantee keypairs, devices, recovery |
| `spec/02-content-tree.md` | Zones, sections, tags, derivation, blobs, editions, fork rule |
| `spec/03-headers.md` | Header format, lines, add/rotate, retention |
| `spec/04-mandates.md` | Certificate format, scopes, the agentic constraint vocabulary, verification |
| `spec/05-delegation.md` | Chains, attenuation, cascade, re-adoption |
| `spec/06-revocation.md` | The ladder, procedures, costs |
| `spec/07-gamma.md` | Chain, entry format, encryption, action accounting |
| `spec/08-connectors.md` | Action scopes, the vault, agent portability |
| `spec/09-cli-and-conformance.md` | CLI surface, test vectors, performance targets |
| `spec/10-threat-model.md` | Properties, attackers, honest limits |

Prior art acknowledged throughout: SPKI/SDSI and object capabilities (certificate
plane), Cryptree/WNFS/Tahoe-LAFS (content plane), age (header envelopes), NNL (the
scale-out header mode we deliberately did not need), UCAN/Biscuit/Macaroons
(attenuated delegation), Key Regression and our own v2 draft (the epoch road not
taken, and why).

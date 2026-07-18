# 8 — Connectors and the vault

> **Status: DRAFT.** How agents act on external services (mail, social, chat), how
> their secrets are held, and why an agent is portable. No generic data store is
> defined in this edition — the vault is the single, minimal exception, dedicated to
> connector configuration.

## 8.1 Connectors

A connector has a lowercase id (`gmail`, `linkedin`, `x`) and a **signed,
versioned, content-addressed manifest** (the Action document). Every action appears
exactly once with exactly one canonical risk class: `read`, `act`, or `binding`.
The manifest signer attests the catalog; a separate, explicit owner approval selects
the exact digest/version trusted by a mandate. The authorization chain and edition
pin the public catalog and approval evidence needed for cold verification. A
gateway, executor, or runtime never invents or locally reclassifies an action.

Actions map to perimeter entries `act.x.<id>.<action>` (§04.2).
`act.x.<id>.*` covers only actions classed `read` or `act` by the exact approved
catalog. A `binding` action is always named exactly and carries the reserved owner
`co_sign` receipt in addition to every other applicable obligation. A catalog
addition, version drift, or reclassification remains unauthorized until a new
owner-approved catalog is pinned by new authority.

> **K1-B catalog migration — human-validated on 2026-07-18.**
>
> New W1 connector authority pins catalog digest, catalog version, and the distinct
> owner-approval evidence inside a homogeneous mandate `draft.3` chain. A catalog
> sidecar or later approval cannot extend a `draft.1` or `draft.2` mandate and
> cannot silently replace the pin of a draft3 chain. Catalog change means fresh
> draft3 certificate ids, issuer-ordered reissuance, and normal Gamma v2 `grant`
> evidence. Historical action and mandate bytes keep their historical semantics.
> The exact draft3 catalog member and catalog/approval document tables remain
> reserved; no draft3 certificate may be emitted before their vectors are approved.

Whoever executes the action — an agent runtime or a tool host — verifies the
presented chain and catalog proof, obtains the single pure Core verdict, honors
tier-X constraints and required public evidence, writes the mandatory `action`
Gamma entry, and only then performs the external effect (§07.4). A legacy `read`
may migrate to `read`, and legacy `write` may map to `act` only under an explicit
versioned migration contract. Legacy authority never proves `binding`; canonical
rights require re-enrolment.

## 8.2 The vault

Connector secrets (OAuth tokens, account handles, settings) live in a **vault node**
`/x/<id>`: an ordinary protected node (DK + header, §03) whose blobs are small config
records keyed by clear names (`oauth`, `profile`, `prefs`). It reuses the content-tree
machinery (derivation, blobs, rotation) — it is the one place this edition stores
structured secret data, and it is scoped tightly to connector config, not a general
database.

- **Isolation.** Each `/x/<id>` has its own DK, header, recipient lines, versions,
  and rotation. No generic `/x` line or root grant is delivered. Authority for one
  connector never opens a sibling connector.
- **Double barrier.** "Who holds the vault" = whoever has a valid header line on the
  exact `/x/<id>` node. The owner always does (I3). A non-owner access succeeds only
  with both a valid chain covering exact `act.x.<id>.config` and that exact line.
  The chain alone decrypts nothing; the line alone authorizes nothing; no wildcard
  covers `.config`. That capability authorizes config CRUD for this vault only; it
  authorizes no external connector action.
- **Guardian model.** A self-hosted agent that manages its own credentials receives
  the exact vault line. An agent that acts through a tool host does **not**: the tool
  host may resolve the credential only in an owner-local context or with its own
  exact `.config` authority and line, after the separate Core verdict for the
  external action. An ordinary action verdict never substitutes for the custodian's
  config authority, and the tool host never transmits the secret to the grantee. An
  ordinary `act.x.<id>.<action>` right delivers no credential or vault line.
- **Custody backend.** An external secret manager MAY hold or unwrap vault material
  as a custody backend. It is never the source of protocol authority: the exact
  chain/capability and header line remain the two fences.
- **Audit separation.** A capability that opens sealed action arguments for audit
  is cryptographically distinct from the capability that opens connector config.
  The exact sub-key topology is reserved for independent vectors; implementations
  must not collapse the two in the meantime.
- **Rotation.** A leaked credential is answered by rotating the upstream secret
  (outside any protocol) **and** rotating the vault node (revocation ladder §06) to cut
  a compromised holder from future config reads. Rotation, recipients, and epochs
  are independent per connector.
- **Atomicity and cold proof.** Config CRUD, header/line changes, rotation, Gamma,
  roots, and publication form one local transaction (§02.12). Refusal leaves the
  bundle unchanged; a fresh-store verifier checks the resulting public/opaque
  evidence without receiving the credential, config plaintext, DK, or private key.
- **No secret in public evidence.** Provider-facing artifacts, Gamma clear fields,
  logs, errors, receipts, proofs, and manifests contain no credential, config
  plaintext, private key, or DK. Normative headers MAY carry encrypted recipient
  lines/wraps; their ciphertext is not authority and remains unusable without the
  exact private opening capability.
- **No `/x` in structural ethos grants.** A grant on `/e` or a whole-ethos grant never
  covers `/x`; vault access is always an explicit `act.x.<id>.config` perimeter entry.

> **CB1 decision G-A — validated at the human protocol gate on 2026-07-18.**
> `.config` is a reserved vault capability outside the connector's business
> `read`/`act`/`binding` catalog. In the current `aithos-mandate-core` version, the
> one exact `act.x.<id>.config` authority covers read/create/edit/delete as an
> indivisible authorization capability. A finer read/write split requires a later
> explicitly versioned capability contract, migration rules, and independent
> vectors; it never reinterprets an existing mandate. Current bytes cannot express
> separate read-only or mutation-only config grants. `.config` is excluded from every
> wildcard and does not inherit `co_sign` merely from the `binding` class; all
> constraints and obligations explicitly present anywhere in the presented chain
> and applicable under §4.13 still conjoin. This classification changes no D9
> isolation or double-barrier rule.

## 8.3 Agent portability

An agent's entire state is `{ grantee keypair, mandate chain, (optional) vault lines }`
— all files, all small, none server-bound. To move an agent to another machine: copy
those files and point it at any mirror of the subject's bundle. Because credentials are
immutable (I2) and content keys are recomputed from headers on demand, the relocated
agent works immediately, with no re-provisioning and no owner action. This is the
concrete payoff of the two-plane design: **the trust an agent carries is portable
because it is a handful of signed files, not server state.**

## 8.4 What is deliberately out of scope

Generic NoSQL collections and binary assets (both specified for the earlier v2 draft)
are **not** part of this edition. They graft onto the identical primitives — a
collection is a node `/c/<name>`, an asset a leaf `/a/<id>`, each with a header — and
will be added once the trust core is proven. Keeping them out now preserves the focus:
a perfectly executed agentic trust layer over the ethos.

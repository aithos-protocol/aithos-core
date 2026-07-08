# 8 — Connectors and the vault

> **Status: DRAFT.** How agents act on external services (mail, social, chat), how
> their secrets are held, and why an agent is portable. No generic data store is
> defined in this edition — the vault is the single, minimal exception, dedicated to
> connector configuration.

## 8.1 Connectors

A connector has a lowercase id (`gmail`, `linkedin`, `x`) and a **signed manifest**
(the Action document: the actions it exposes, each with a risk class
`read`/`act`/`binding`). Actions map to perimeter entries `act.x.<id>.<action>`
(§04.2). Whoever executes the action — an agent runtime or a tool host — verifies the
presented mandate chain, checks the action is covered (via the verb/`covers` algebra,
so a root `*`-style grant never implies `binding` actions implicitly), honors
tier-X/C constraints, performs the call, and writes the mandatory `action` gamma entry
(§07.4).

## 8.2 The vault

Connector secrets (OAuth tokens, account handles, settings) live in a **vault node**
`/x/<id>`: an ordinary protected node (DK + header, §03) whose blobs are small config
records keyed by clear names (`oauth`, `profile`, `prefs`). It reuses the content-tree
machinery (derivation, blobs, rotation) — it is the one place this edition stores
structured secret data, and it is scoped tightly to connector config, not a general
database.

- **Guardian model.** "Who holds the vault" = whoever has a header line on `/x/<id>`.
  The owner always does (I3). A self-hosted agent that manages its own credentials
  receives a vault line (perimeter `act.x.<id>.config`, a reserved action implying
  read+write on the vault); an agent that only acts through a tool host does **not** —
  the tool host holds the vault line and the agent merely holds `act.*` action scopes.
- **Rotation.** A leaked credential is answered by rotating the upstream secret
  (outside any protocol) **and** rotating the vault node (revocation ladder §06) to cut
  a compromised holder from future config reads.
- **No `/x` in structural ethos grants.** A grant on `/e` or a whole-ethos grant never
  covers `/x`; vault access is always an explicit `act.x.<id>.config` perimeter entry.

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

# Aithos SDK v0 — demo contract

> **ARCHIVE — contrat v0 supplanté.** Le SDK v2 vit dans `code/aithos-sdk` et sa
> surface courante est documentée dans son `README.md`. Ne pas implémenter de
> compatibilité à partir de ce contrat sans décision explicite de migration.

Status: implementation contract for the first SDK and dashboard demo.

## Layering

The SDK is a network and application façade over `aithos-client`; it is not a
second protocol implementation:

```text
Dashboard / CLI
      |
      v
Aithos SDK            transport, cache, sync, CAS/retry, MCP
      |
      v
aithos-client/WASM    sessions, mandates, authorization, mutations, artifacts
      |
      v
aithos-core + aithos-bundle
```

`aithos-client` remains strictly offline. Every signed envelope, mandate,
operation, changeset, evidence document and manifest used by the SDK is
produced or accepted by that client. JavaScript and the SDK transport layer do
not reproduce protocol rules.

## Demo outcome

The SDK must let an application:

1. create an Ethos and retain its owner keys on the client side;
2. publish and read the Ethos public zone;
3. describe which enterprise connectors the Ethos may request, without ever
   exposing connector credentials;
4. issue a scoped mandate to a delegate;
5. let that delegate edit the public zone and invoke an authorized action
   through the Aithos MCP hosted by the enterprise gateway.

Circle and self zones are explicitly out of scope for v0. Their wire formats
remain supported by the protocol, but the SDK does not expose them yet.

## Identity and location

An Ethos is addressed by an `EthosLocator`:

- `provider_url`: the public provider authority;
- `tenant`: the provider tenant;
- `did`: the Ethos DID.

Owner and delegate private keys are never fields of `EthosLocator`, connector
descriptors, mandates, logs, or network responses. Signing is supplied through
an injected signer interface so browser, native, hardware-backed, and delegated
custody can evolve independently.

## Public zone

Public reads are anonymous. The provider must serve the public carriers without
an Aithos authorization envelope. Browser-facing deployments must also allow
the SDK origin through CORS.

The canonical v0 publication carrier is the existing K1-C draft.2 pipeline:

- `public/sections/<sid>.md` for section bodies;
- `indices/public.json` for the public index;
- `roots/public.json` for the public root;
- signed operation, facts, evidence, changeset, and publication artifacts;
- `manifest.json` as the final compare-and-swap commit.

The legacy `e/public/**` representation remains readable for compatibility but
is not a second SDK write format.

Owners may create, edit, and delete public sections. Delegates may perform only
the verbs and public perimeter granted by their valid mandate chain. Publication
is assembled and verified locally before upload; artifacts are uploaded first
and the manifest is committed last. A CAS conflict is returned to the caller as
a typed rebase-required error and is never silently overwritten.

## Two MCP planes

The SDK treats the two MCP surfaces as distinct even when they share Aithos
types:

- **Public Ethos/provider surface** — globally reachable anonymous public reads
  and mandated public writes. It does not require an enterprise gateway.
- **Enterprise action MCP** — hosted by the enterprise gateway. It verifies the
  action mandate, resolves connector policy, uses Vault/OAuth credentials, calls
  the upstream connector, and records the result. Connector secrets never cross
  this boundary into the SDK or Ethos.

## Connector descriptors and action mandates

A connector descriptor is metadata only: stable connector id, label, available
capability names, and optional non-secret presentation data. It contains no
access token, refresh token, client secret, Vault reference, or upstream API
credential.

An action mandate binds at least:

- subject Ethos DID;
- delegate public key;
- gateway audience;
- connector and capability allow-list;
- validity interval;
- optional resource constraints and usage limits;
- revocation-compatible mandate id and authority chain.

The gateway denies capabilities not named by the mandate even when the
underlying enterprise connector offers them.

## Typed failures

The SDK exposes stable error categories for invalid input, unsupported scope,
unauthorized operation, expired or revoked mandate, transport failure, provider
verdict, CAS conflict/rebase required, invalid publication artifact, and gateway
action denial. Raw provider and gateway details may be retained as diagnostic
causes but are not the public control-flow contract.

## Release gates

The demo SDK is ready only when automated tests prove:

- anonymous public read and refusal of non-public paths;
- owner public publication;
- delegated public publication within scope and rejection outside scope;
- local publication verification before the first upload;
- manifest-last CAS publication and typed conflict handling;
- connector descriptors contain no secret material;
- an allowed action traverses the enterprise gateway and a neighboring denied
  capability does not;
- the same high-level flow works from the CLI and the minimal dashboard.

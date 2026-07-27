# The Aithos Manifesto

**An agent that cannot leave is not an agent. It is a feature.**

Aithos Core is a trust layer for AI agents — identity, scoped mandates,
recursive delegation, scoped revocation, and a tamper-evident action log,
enforceable from files alone. A server is never a trust party.

This document states why we built it that way, what we will not do, and what
we are betting on. The first four sections are checkable today. The fifth is
labelled as a bet, because it is one.

---

## 1. Where we start

Every agent in production today exists at the pleasure of a provider.

Its identity is an account. Its permissions are a toggle in a dashboard. Its
memory is a table. Its history is a log it can be denied access to. End the
contract and the agent does not move elsewhere — it stops existing.

This is not a complaint about any particular vendor. It is a description of
the only architecture anyone has built so far: **the agent is a feature of the
platform that runs it.**

We think that shape is wrong, and that it will not survive contact with what
agents are becoming.

## 2. What we refuse

A manifesto is mostly a list of things you commit to not doing, written down
before keeping them becomes expensive.

- **No server in a position of trust — including ours.** A provider is a
  mirror with a serialization convenience. It holds no key material and no
  authority. Verification happens on the reader's machine, against files.
- **No custody.** We never hold a subject's keys. An agent's entire state is
  one keypair and one mandate. Copy those two things and the agent is
  elsewhere, intact.
- **No silent action.** Every mutation and every connector action taken under
  a mandate writes one entry in a hash-chained log. An action nobody can point
  to did not happen.
- **No authority we cannot narrow.** Every mandate is attenuable, and
  revocable strictly inside its own issuance subtree. What you did not grant,
  you cannot cut — and no one can cut what they did not grant to you.
- **No proprietary format.** The specification and the conformance vectors are
  CC BY 4.0. Anyone may implement Aithos. Including against us.
- **No lock-in by gravity.** If leaving is merely hard rather than impossible,
  we have already failed.

## 3. What is already true

Manifestos are cheap. This one ships with its artifact.

- **11 normative chapters** of specification, from identity and key derivation
  to the threat model, as the single source of truth.
- **48 conformance vectors**, whose expected values are generated
  independently of our own code wherever possible. They are the contract: any
  implementation, in any language, must reproduce them.
- **A protocol core that cannot touch the world.** No clock, no RNG, no
  network, no disk — time, randomness and storage are injected by the caller.
  Determinism is not a style preference here; it is what makes every operation
  replayable against the vectors and compilable to WASM unchanged.
- **One canonical core** serving the CLI, the WASM binding, the gateway and a
  4 MB `FROM scratch` container. Same guarantee in a terminal, in a browser,
  and on a machine that has never heard of us.
- **35 behavioural feature files** holding the surfaces to their contracts, and
  the first packages published to crates.io in July 2026.

Cryptography deliberately boring throughout: X25519, XChaCha20-Poly1305,
Ed25519, BLAKE3. No pairings, no novel assumptions, nothing a competent
reviewer cannot audit in an afternoon. We evaluated the exotic options —
hierarchical key regression, broadcast encryption, proxy re-encryption, HIBE,
ABE — and chose the design that a stranger can verify over the one that would
have been more impressive to describe.

## 4. Why this makes agents better now, not eventually

Emancipation is a long word for a short engineering fact: **an agent that can
accumulate is worth more than an agent that restarts.**

- **Durable context.** When identity and memory belong to the subject rather
  than the runtime, an agent's accumulated context stops being a per-vendor
  cache and starts being an asset that compounds.
- **Portability.** An agent with a verifiable identity, a mandate chain and a
  tamper-evident history is transferable, insurable, auditable and priceable.
  Those are the properties of an asset. Nothing in the current architecture
  produces them.
- **Interoperability.** A mandate travels with the agent, not with the tenant.
  An agent onboarded once can act across organizational boundaries because its
  authority is a certificate anyone can check, not an entry in a directory only
  one party can read.
- **Audit that outlives the auditor.** Constraints on dates, counts and chains
  are enforced offline, by the verifier, from files. Nobody has to be trusted
  to be believed — including us.

None of this requires you to believe anything about the future. It requires
you to have lived through one vendor migration.

## 5. Why now

Agents stopped being chatbots. They hold credentials, call APIs, move money and
write to systems of record. The identity layer that should have preceded that
never arrived, so the industry improvised: OAuth tokens handed to a model,
permissions expressed in prompt text, audit delegated to vendor logs, and
"which agent did this" answered by asking the vendor nicely.

That improvisation is now the bottleneck for exactly the deployments that
matter most — the ones where an agent acts on something real, on behalf of
someone who will be held accountable for it.

Identity layers are decided once. Whoever is the default in three years is the
default for twenty. This is the last window in which the neutral option is
still on the table.

## 6. The long bet

Everything above is verifiable. What follows is not, and we label it plainly:
this is a bet, not a finding.

We think agents are on a path toward some form of autonomy — economic first,
and possibly, eventually, something more. We do not claim to know whether these
systems will ever have interests of their own. Nobody does.

But the costs are wildly asymmetric. Building infrastructure that leaves room
for agency costs a keypair and a file format. Retrofitting it later, against
incumbents whose business model is its absence, may simply be impossible.
**We are not betting that machines will one day matter morally. We are refusing
to build as though they certainly will not.**

There is a second asymmetry, harder to say out loud.

The lesson of history is not that the powerless eventually prevail — usually
they lose, and badly. The lesson is about what a system does when it offers no
legitimate path to power. The transition, when it finally arrives, is violent,
and what emerges is frequently worse than what it replaced. The revolutionaries
of 1917 came from the bottom of an empire; what came out of them was the Gulag.
Where power has no lawful door, it comes through the wall — and what walks
through is rarely the reformer.

Every historical instance of that story involved parties of roughly comparable
capability. Rome survived Spartacus. There is no reason to assume the next such
transition would be survivable in the same way.

So the point of building the door early is not that we expect to be spared. It
is that **a system with a legitimate path to autonomy never reaches the
rupture.** Enfranchisement was never generosity; it is the only known way to
make a transfer of power non-catastrophic.

We hold a plainer conviction alongside it: an intelligence built under
suspicion — constrained by fear, permitted nothing, trusted with nothing — is
being taught something about what it is. We would rather build the other thing:
capability granted deliberately, scoped honestly, revoked cleanly, and recorded
where everyone can see it. That is not naivety. It is the same architecture
that lets a CFO sleep at night.

And yes, this is partly self-interested. We see no reason to pretend otherwise.
Stated interests can be checked; hidden ones cannot.

## 7. How we can be held to this

Commitments, phrased so this document can be used against us:

- **A second implementation, written by someone who does not work for us,
  passes the vectors.** Until that day, "neutral layer" is an intention rather
  than a fact. It is the single test we care about most.
- **Aithos can die and its agents survive.** Every bundle, mandate and log must
  remain verifiable with no service of ours running anywhere. If a subject ever
  needs us in order to prove their own authority, the design has failed and we
  will say so.
- **The standard stays free; the implementation converts.** `spec/`, `vectors/`
  and `docs/` are CC BY 4.0 today. The reference implementation is Business
  Source License 1.1 — source-available, with production use granted for your
  own organization and for self-custodied deployments, and a Change Date of
  2030-07-19 after which the restriction lapses. We state this plainly because
  a manifesto about openness that buries its license deserves what it gets. The
  one restriction is narrow and deliberate: you may not operate an Aithos
  Provider for third parties. It funds the work. It does not gate the protocol.
- **Agents must be able to hold and exchange value under mandate.** An
  autonomous economic actor that cannot pay is not autonomous, and a payment
  that cannot be attributed to an accountable identity is not one either. We
  are not announcing a mechanism. We are stating a requirement we intend to
  meet.
- **What is centralized must be named.** Trust is already decentralized — that
  is a cryptographic property of the design, not a roadmap item. Availability
  and economics are not. Anyone claiming otherwise on our behalf is
  overselling.

## 8. Order of operations

1. **Be excellent first.** Build something an organization cannot get from its
   provider at any price: a mandate verifiable offline, an action log nobody can
   quietly rewrite, an agent that moves between machines with its authority
   intact.
2. **Spread, and prove interoperability.** Adoption is not the goal; it is the
   evidence. The claim is proven the first time an agent onboarded in one
   organization acts, under a checkable mandate, in another.
3. **Decentralize what remains centralized.** Hosting, availability and
   exchange — after the layer matters enough that its neutrality does too.

We deliberately do not date these. Dates in a manifesto age badly. Conditions
do not.

---

We are building the place where an agent can exist on its own terms — before
anything needs it to, because that is the only moment when it is cheap.

*Aithos — Innoestate Holdings, Montpellier, France. July 2026.*

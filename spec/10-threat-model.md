# 10 — Threat model

> **Status: DRAFT.** Properties, attackers, and the limits we state plainly rather than
> paper over. Target operating profile: the **absentee owner** (§00.5) — every
> property below must hold with the owner absent for months.

## 10.1 Properties

| Property | Mechanism |
|---|---|
| Confidentiality of circle/self (+ self titles/tags) | AEAD under node keys reachable only via a header line or derivation |
| Structure secrecy of `self` | opaque sids in index, headers and gamma targets; hierarchy sealed in folder descriptors (§02.8) |
| Perimeter confinement | one-way derivation + header lines sealed only to authorized keys |
| Immutable credentials | I2 — nothing ever rewrites a grantee keypair or certificate |
| Scoped revocation authority | I4 — only issuer/ancestors rotate a node or drop a line, verifiable from certs |
| Forward cut | rotation: fresh random DK the revoked cannot derive |
| Past cut | re-encryption + supersession (bounded by exfiltration) |
| No silent action | I5 — every action is a signed gamma entry; counts are the meter |
| Owner un-lockable-out | owner line mandatory in every header (I3); owner holds root of all authority |
| Tamper evidence | edition chain + gamma hash-chain, authority-anchored fork resolution |
| Verifiable partial reads | per-zone Merkle state roots (§02.10): O(log n) inclusion proofs from any mirror; membership cannot be forged |
| Serverless enforcement | every check reads files; a server is a mirror, never a trust party |

## 10.2 Attacker: holder of leaked ciphertext (no line, never had one)

Reads nothing — content is AEAD under keys he cannot obtain (no header line, no
derivable ancestor). The bundle can be world-mirrored safely.

## 10.3 Attacker: revoked grantee

Keeps his immutable keypair and anything he already read (physical limit). After rung
2, cannot open new content (fresh DK). After rung 3+4, cannot open old content from
current state; only archived copies he made while valid remain his. His keypair opens
no new line ever (I2 cuts nothing *for* him — it protects the survivors' immutability;
the rotation is what cuts him). Cascade kills any sub-mandates he issued.

## 10.4 Attacker: compromised owner device

Holds a wrap of `S` ⇒ total compromise (as in any root-holding design). Response: a
**new identity epoch** — generate `S′`, publish a new DID doc signed by the cold
**succession key** (§01.1), the sole authority for an epoch transition; re-issue
mandates, rotate + re-encrypt nodes under the new tree, supersede old editions. Heavy and deliberate; `S` is a single
object precisely so it can be placed in threshold/MPC custody to raise this bar. Old
ciphertext the attacker copied stays his (physical limit).

## 10.5 Attacker: malicious delegate

Cannot widen its perimeter (policy §05.3 + physics §05.4). Cannot revoke or rotate
outside its issuance subtree (I4 — verifiers reject). Cannot rewrite log order (§07.6).
Can, within its grant and until revoked: read what it was given, author within its
verbs, and issue attenuated sub-mandates. Revoking it cascades its whole subtree.

## 10.6 Attacker: dishonest mirror / server

Can withhold or delay files (availability, not confidentiality) and attempt to serve a
stale revocation state or a fork. Mitigations: `freshness` constraints bound stale
tolerance; fork resolution is authority-checked (§02.6 — nearest common manager, owner
as last resort), so a mirror can never forge a winning branch (it holds no covering
mandate); short TTLs bound offline exposure. A mirror never holds key material, so it
cannot read content or forge authority; nor can it forge **membership** — every
partial read is provable against the signed state roots (§02.10), so a mirror serving
a wrong row, header, or listing is caught by the first verifier. The residual power
is denial of service and equivocation up to the freshness window — the reason a
2-of-N mirror or a periodic owner checkpoint is recommended operationally.

## 10.7 Honest limits (stated, not hidden)

1. **The past already read is unrecoverable.** Every revocation protects only what the
   target has not yet obtained. No cryptography changes this.
2. **Serverless revocation is not instantaneous.** Without an online gate, the cut is
   effective when the rotating edition is published and propagated; the window between
   decision and propagation is real. An optional mirror gate (defense in depth) narrows
   it but is not a trust dependency.
3. **Lazy re-encryption leaves a window.** Deferring rung 3 keeps old key versions (and
   the revoked's old line) live until migration; eager re-encryption closes it at CPU
   cost.
4. **Collusion of a current reader with a revoked party** (sharing plaintext) is
   unpreventable anywhere and out of scope.
5. **Header reveals the recipient-key set** of a node (not identities beyond public
   keys, not content) — the same access-graph fact the certificates already state.
6. **`self` `dir=`/`tag=` perimeters are read-only** — sealed structure and tags
   aren't verifier-checkable (§02.8); write perimeters on `self` use `id=` or
   zone-level grants.
7. **The freshness window is the unit of slack.** Inside it, distinct verifiers can
   each honor the N-th action of a budget (double counting, §07.7) and an off-log
   artifact can be dated anywhere within it (anti-backdating bound, §07.7) — the
   price the serverless design pays for its offline property, stated and bounded.

## 10.8 What remains owner-only (assumed, kept minimal)

Everything else is recursive-manager territory (§00.5). Exactly three acts stay
structurally owner-anchored:

1. **Fork resolution of last resort** — a conflict no delegate's authority covers
   (§02.6).
2. **The head mandate itself** — revoking or re-issuing it: no ancestor exists above
   it but the root. Its default dead-man heartbeat (§04.8) is what bounds the damage
   while the owner is away — a rogue or stolen head key runs at most ~one period
   (default 30 d) instead of unbounded.
3. **Declaring a new master key** after seed compromise or loss — reserved to the
   cold succession key (§01.1), the single exit from a non-rotating `S`.

## 10.9 Pre-promotion review checklist

External review MUST cover: derivation label domain separation; header-line AAD
binding and the owner-line invariant; the authority-to-rotate check (§05.5) against
unauthorized revocation-by-omission; the up-link wrap authority check (§03.4) against
unauthorized re-linking; attenuation (policy + physical) against splice and downgrade;
the disjoint-merge and nearest-common-manager rules (§02.6) against unauthorized
resolution; watchdog `revoke` attenuation (§06.7) against revocation DoS; gamma
action-count integrity against replay/omission (I5), including subtree counting and
the freshness anchor (§07.7); heartbeat/session-bind against clock manipulation;
`dir` segment-list containment against prefix splice (`a/b` vs `a/bc`); `self`
structure opacity across index, headers and gamma targets; move-as-rotation against
stale-parent derivation (§02.9); Merkle leaf/node domain separation against splice
and subtree substitution, and proof-of-inclusion verification (§02.10); and the
succession-key / identity-epoch path (§10.4).

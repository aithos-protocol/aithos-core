# 5 — Delegation

> **Status: DRAFT.** How any key holder creates keys and mandates on its own perimeter,
> alone and offline, and how authority to revoke follows issuance (requirements 2, 3).

## 5.1 The `issue` right

A mandate carrying `issue#depth=n` (`n ≥ 1`; bare `issue` ⇒ `depth=1`) lets its grantee
mint sub-mandates that attenuate it. Without `issue`, a mandate is a leaf.

## 5.2 Minting a sub-mandate (offline)

The delegate:

```
1. Writes a §04 certificate with parent = its own mandate id, subject unchanged,
   issued_by = its own grantee pubkey, actor keys = the child's.
2. Signs it with its own Ed25519 key.
3. Appends header lines for the child on the nodes it grants — sealed from DKs the
   delegate can itself open (it has its own lines). It can seal a child line for any
   node it holds, or for a descendant node it derives; it cannot seal for a node it
   does not hold (physical attenuation).
```

No owner involvement, no server. The child presents the full chain `[…, parent, child]`
in every request and gamma entry.

## 5.3 Attenuation invariants (verifier, per link child→parent)

1. **Perimeter containment.** Every child entry is covered by a parent entry under the
   verb lattice and selector algebra (a parent `tag=test` covers only child `tag=test`;
   a parent `circle` no-selector covers any child circle selector; `ns=p` covers
   `id=p:x`).
2. **Window containment.** `parent.nb ≤ child.nb ≤ child.na ≤ parent.na`.
3. **Constraint monotonicity.** Child constraints are ≥ strict: numeric caps ≤ parent,
   `domains ⊆`, `counter_sign/binding ⊇`, `heartbeat` at least as tight, `freshness`
   ≤ parent, `first_party_only` not weakened.
4. **Depth.** child `issue#depth=m` ⇒ `m ≤ n−1`; chain length in links ≤ root depth.
5. **Signature & identity.** child.signature verifies under parent.grantee.pubkey;
   `child.issued_by == parent.grantee.pubkey`; `child.grantee.pubkey ≠ child.issued_by`.

Any failure invalidates the child and its descendants.

## 5.4 Cryptographic attenuation (the second fence)

Because the child's header lines are sealed by the delegate **from DKs it can open**,
one-way derivation makes it physically impossible to grant a node the delegate does not
hold. A forged wider perimeter string fails the §5.3 check *and* carries no usable key.
Two independent fences — policy and physics — must both be crossed to over-reach.

## 5.5 Scoped revocation & authority (requirement 3)

**Only the issuer of a mandate, or a transitive ancestor up to the owner, may revoke it
or remove its header lines (I4).** This is verifiable from the certificate chain alone:
the would-be revoker's mandate id must appear on the target's chain (or be the owner
root). Consequences:

- A delegate revokes **its own** children without touching siblings, cousins, or the
  owner's other grants on the same node: it rotates the node key and republishes the
  header **omitting the revoked child's line but keeping every other line** — including
  lines it did not create (those it re-seals under the new DK using its own access).
- To re-seal others' lines under a rotated DK, the revoker must itself hold the node
  (it does — it had `issue` there). It cannot read those grantees' *content* choices,
  only re-wrap the same DK to the same public keys: a mechanical, blind operation.
- The **owner** can revoke anything (root is ancestor of all).

A verifier rejects a header rotation whose signer is not an authorized issuer for the
lines it changed, or that drops a line the signer had no authority over (that would be
an unauthorized revocation by omission).

## 5.6 Cascade and re-adoption

Revoking a mandate breaks every descendant chain (a dead link fails §5.3/§5.5).
Survivors the owner (or a still-valid ancestor) wants to keep are **re-adopted**: a
fresh direct mandate + one header line each (§03.3) — cheap, explicit, and it makes the
new trust path auditable. Compromise of a branch thus collapses the branch; rebuilding
is a deliberate act, not an automatic survival.

## 5.7 Worked example

```
owner → assistant   perimeter: read.circle, edit.circle#tag=test, issue#depth=1, 30d
assistant → helper  perimeter: read.circle#tag=test, act.x.gmail.draft, 24h
  # helper's circle/t/test line is sealed by the assistant from its own circle DK.
  # revoking assistant kills helper (chain breaks); revoking helper alone rotates
  #   /e/circle/t/test, drops helper's line, keeps the assistant's and owner's.
```

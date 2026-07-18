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

Before applying the invariants below, the verifier requires exact equality of the
parent and child `aithos-mandate-core` discriminators. A mixed-version link is
invalid and invalidates its descendants; no constraint rule from either version is
projected onto the other.

1. **Perimeter containment.** Every child entry is covered by a parent entry under
   the verb lattice and selector algebra (§04.2). A parent zone with no selector
   covers any child selector in that zone. `id=x` covers only `id=x`; `dir=` and
   `tag=` NEVER cover an `id=` child, even if an external resolver says that SID
   currently lies below the folder or carries the tag. `dir=p` covers narrower
   `dir=p/q` and `dir=p/q&tag=t`; `tag=t` covers `tag=t` and — policy level —
   `dir=X&tag=t`. Containment of `dir` is nodal (§04.2), never a string prefix.
   Key level: a `dir&tag` grant is minted by a holder of the folder (or an
   ancestor) — a holder of only the zone-root tag view holds section wraps, not the
   folder, and so cannot mint the folder-local anchor (§5.2 physical attenuation).
2. **Window containment.** `parent.nb ≤ child.nb ≤ child.na ≤ parent.na`.
3. **Constraint monotonicity.** Child constraints are ≥ strict: numeric caps ≤ parent,
   `domains ⊆`, `counter_sign/binding ⊇`, `obligations ⊇` (a sub-mandate may add
   an obligation, never drop one, §4.12 — an inherited obligation MUST be
   JCS-identical to the parent's, decided 2026-07-10; tightening is expressed by
   *adding* a stricter obligation, the conjunction is the tightening), `heartbeat`
   at least as tight, `freshness` ≤ parent, `first_party_only` not weakened.
   Under `draft.2`, `max_children` is specifically non-droppable: if the parent
   carries it, every child repeats a value ≤ the parent's, including a chain leaf;
   it counts that child's direct children only, not the ancestor's whole subtree.
   Under historical `draft.1`, the E+ attenuation matrix remains authoritative:
   dropping `max_children` is valid and leaves the child without its own width cap.
   Every known constraint is type-valid on both documents. An unknown constraint
   may be carried only by a directly owner-issued chain leaf; it can neither cross
   nor parent a delegation link because no attenuation law is known (§04.4).
4. **Depth.** child `issue#depth=m` ⇒ `m ≤ n−1`; chain length in links ≤ root depth.
5. **Signature & identity.** child.signature verifies under parent.grantee.pubkey;
   `child.issued_by == parent.grantee.pubkey`; `child.grantee.pubkey ≠ child.issued_by`.

Any failure invalidates the child and its descendants.

Changing versions is not an attenuation link. To migrate, the authorities reissue
the complete chain under `draft.2` in issuer order and log the new grants. They do
not edit a certificate, substitute its discriminator before verification, or mix
old and new certificates in one chain (§04.1.1).

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
- Re-sealing the survivors' lines under the rotated DK is done by the revoker itself
  (owner or ancestor), which holds the node — attenuation guarantees it. The operation
  is mechanical (re-wrapping the same DK' to the same public keys, learning nothing
  new about the survivors), but its author is never a keyless third party: it is, by
  the chain, an authorized holder. No automaton without the key can rotate a node
  (§00.5).
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

Re-adoption does not heal the survivor's **own** subtree: its sub-mandates name its
old mandate id as `parent`, which is now revoked, so their chains stay broken. The
re-adopted mandate is a fresh direct mandate; the survivor MUST re-issue its own
sub-mandates under the new id if it wants to keep them. Expect no automatic subtree
survival.

## 5.7 Worked example

```
owner → assistant   perimeter: read.circle, edit.circle#tag=test, issue#depth=1, 30d
assistant → helper  perimeter: read.circle#tag=test, act.x.gmail.draft, 24h
  # helper's circle/t/test line is sealed by the assistant from its own circle DK.
  # revoking assistant kills helper (chain breaks); revoking helper alone rotates
  #   /e/circle/t/test, drops helper's line, keeps the assistant's and owner's.
```

Width warning: `issue#depth=n` without `max_children` (§04.4) authorizes unlimited
width at every level — `depth` bounds only the chain's length. Pair `issue` with
`max_children` on any sensitive perimeter.

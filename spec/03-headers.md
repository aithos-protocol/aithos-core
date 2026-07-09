# 3 — Headers

> **Status: DRAFT.** The bridge between the certificate plane (who) and the content
> plane (what). The header is the only place a node key is ever stored, and it is
> stored sealed.

## 3.1 Object

One header per granted node — a zone root `/e/<zone>`, any folder
`/e/<zone>/d/<sid>/…`, any tag view `…/t/<tag>` (zone-root or folder-local), any
section `…/s/<sid>`, or a vault `/x/<id>` — at `.../header.json`. A node that was
never individually granted has no header (derivation is its only route):

```jsonc
{ "object": "header", "v": 1,
  "node": "/e/circle",
  "key_versions": {
    "3": {                                   // current DK generation
      "lines": [
        { "to": "owner",            "kid": "owner-kex",     "n": "…", "c": "…" },
        { "to": "z6MkGrantee…",     "kid": "z6MkGrantee…",  "n": "…", "c": "…" },
        { "to": "z6MkAssistant…",   "kid": "z6MkAssistant…","n": "…", "c": "…" }
      ]
    },
    "2": { "lines": [ … ] }                  // retained per §3.5 for old ciphertext
  } }
```

- A **line** is an X25519-HKDF-SHA256-AEAD seal of the node's DK to one recipient
  public key. AAD purpose `header-line`, bound to `subject_did ‖ node ‖ key_version`.
- `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
  a routing hint only — the seal is what grants. Recipients try lines addressed to
  their `kid`.
- **I3:** every `key_versions[*].lines` MUST include the owner line. An edition whose
  any header violates this is invalid.

## 3.2 Reading

To open node N: pick the `key_version` matching the target blob's index entry, find a
line whose `kid` is mine (or `owner`), unseal → DK → derive down (§02.5) → decrypt.
The owner always resolves via `owner-kex`; a grantee via its keypair. No network, no
per-read state.

## 3.3 Grant = append a line (O(1), touches nobody)

To grant an existing node to a new recipient, an authorized issuer (owner, or a
delegate with issuing right on the node, §05):

```
1. Open the node's current DK (own line).
2. Seal DK to the recipient's X25519 key → one new line.
3. Append it to key_versions[current].lines. Publish the edition.
```

Content untouched, other lines untouched, DK unchanged. This is the frequent, cheap
operation. (If old versions still hold un-re-encrypted content the recipient should
read, the issuer adds a line to those versions too — §3.5.)

## 3.4 Rotate = new key version (the costly half of revocation)

To rotate node N (revocation rung 2, §06):

```
1. Generate DK' (fresh random). key_version += 1.
2. Build key_versions[new].lines = a sealed line for every SURVIVING recipient
   (everyone currently authorized minus the revoked) + the owner line. The rotator
   is the revoker itself (owner or authorized ancestor, §05.5): it holds the current
   DK (its own line or derivation) and knows every survivor's public key (the
   existing lines).
2bis. Derivation up-link. If the rotated node N is derived from a parent node P that
   the rotator holds, it also publishes an up-link wrap: seal(DK'_N) openable via
   K_P — same primitive as a tag wrap (AAD purpose `tagwrap`, §00.3), bound to
   subject_did ‖ N ‖ new key_version. The wrap restores the parent→child derivation
   path broken by the fresh random DK', so holders of P (or of any ancestor of P)
   keep reading N by derivation without needing a line of their own. If the rotator
   holds exactly N but not P, it instead seals DK'_N individually to the current
   holders of P (public keys read from P's header); the first manager of P that
   later acts posts the definitive wrap.
3. Re-encrypt the node's content under keys derived from DK' (rung 3), rewriting
   blobs and their index key_version; OR defer (lazy) leaving old versions live.
4. Publish. Surviving grantees' keypairs are unchanged (I2); they open the new lines
   automatically on next read.
```

The revoked recipient gets no line in the new version and cannot derive DK' (fresh
random, not derived from anything he holds).

Why 2bis exists: without the up-link, a fresh random DK' would sever derivation from
every ancestor node — a holder of the zone who read N by pure derivation (§02.5)
would silently lose N without having any header line to fall back on. The wrap
re-establishes that path in one entry and touches no other line. Verification is
mechanical: the new version's lines MUST equal the previous lines minus the revoked
(plus, in the exactly-N case, recipients ⊆ P's header), and an up-link wrap whose
author does not hold P is rejected.

## 3.5 Retention of old versions

Old `key_version` entries are retained while any un-re-encrypted blob still references
them. Under **eager** re-encryption (rung 3 complete), old versions are dropped in the
same edition — the revoked keeps nothing openable from current state. Under **lazy**
re-encryption, old versions (still containing the revoked's old line) persist until
their last blob migrates; a party holding an archived copy can read those specific old
blobs meanwhile. This is the documented cost of deferring, and the reason eager is the
default for incident-class revocations.

## 3.6 Header hygiene

Lines are unnamed beyond the routing `kid`; no scope, verb, or human label is stored
in the header (that lives in the public certificate). A header therefore leaks only
the set of recipient public keys of that node — the same access-graph fact the
certificates already state, and never more.

The header's hash is folded into its node's Merkle hash (§02.10): appending a line or
rotating bumps the node's proof path to the signed state root, so a reader proves it
holds the **current** header without fetching any other header.

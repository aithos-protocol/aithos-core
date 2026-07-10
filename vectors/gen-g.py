#!/usr/bin/env python3
"""Independent generator for the G conformance vectors (revocation, spec 06).

  g1-revocation.json  the revoke gamma entry (JCS + owner signature),
                      authority verdicts, forward-only verdicts
  g2-rotation.json    the mechanical rotation rule (survivor line sets) and
                      the up-link wrap bytes (spec 03.4 step 2bis)
  g3-move.json        move-as-rotation (spec 02.9): nodal dir containment
                      verdicts (04.2), derivation stability below the moved
                      node, new-path AAD bindings, up-link wrap via the NEW
                      parent. Auto-validated against B2 before writing.

Second-implementation rule: blake3 + PyNaCl + hashlib + base58, never the
Rust reference. Usage: python3 gen-g.py   (from vectors/)
"""

import hashlib
import json

import base58
import blake3
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_encrypt
from nacl.signing import SigningKey

SEED = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
AGENT_SK = bytes.fromhex("a1" * 32)


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def derive(context: str, key: bytes) -> bytes:
    return blake3.blake3(key, derive_key_context=context).digest()


def multibase_ed(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xed\x01" + pub).decode()


def sign_doc(doc: dict, sk: SigningKey) -> dict:
    doc = dict(doc)
    doc["signature"] = dict(doc["signature"], value="")
    sig = sk.sign(jcs(doc).encode()).signature
    doc["signature"] = dict(doc["signature"], value=sig.hex())
    return doc


content_sk = SigningKey(derive("aithos-core/v1/content-sign", SEED))
agent_sk = SigningKey(AGENT_SK)
root_sk = SigningKey(derive("aithos-core/v1/root-sign", SEED))
DID = "did:aithos:" + multibase_ed(bytes(root_sk.verify_key))

REVOKED_ID = "mandate_0000000000000000000000000M"
REVOKED_AT = "2026-07-10T12:00:00Z"


def gen_g1():
    entry = sign_doc(
        {
            "v": 1,
            "id": "gamma_0000000000000000000000000R",
            "prev": "",
            "at": REVOKED_AT,
            "kind": "revoke",
            "target": REVOKED_ID,
            "payload": {"reason": "device_lost"},
            "signature": {"alg": "ed25519", "key": "#content", "value": ""},
        },
        content_sk,
    )
    return {
        "vector": "G1",
        "description": "Owner-signed revoke gamma entry (spec 06.4, one artifact): "
                       "canonical JCS + content signature; authority and forward-only "
                       "verdicts. Generated independently (Python blake3+PyNaCl).",
        "seed_hex": SEED.hex(),
        "agent_sk_hex": AGENT_SK.hex(),
        "revoked_mandate_id": REVOKED_ID,
        "entry_jcs": jcs(entry),
        "entry_hash": "sha256:" + hashlib.sha256(jcs(entry).encode()).hexdigest(),
        "authority": {
            # verdicts the Rust side must reproduce with check_revoke_authority
            "owner_entry": "valid",
            "issuer_leaf_matches_issued_by": "valid",
            "leaf_id_in_revoked_ancestry": "valid",
            "unrelated_sibling": "GammaRevocationRejected",
            "watchdog_covering_perimeter": "valid",
            "watchdog_outside_perimeter": "GammaRevocationRejected",
        },
        "forward_only": {
            "revoked_at": REVOKED_AT,
            "2026-07-10T11:59:59Z": "valid",
            "2026-07-10T12:00:00Z": "MandateRevoked",
            "2026-07-11T00:00:00Z": "MandateRevoked",
        },
    }


def gen_g2():
    # Rotation rule (spec 03.4): new lines MUST equal old minus the revoked
    # (owner always present). Kids are routing identities.
    old_kids = ["owner-kex", "zAGENT1", "zAGENT2"]
    revoked = "zAGENT1"
    survivors = [k for k in old_kids if k != revoked]

    # Up-link wrap bytes (03.4 step 2bis == tag-wrap primitive, spec 00.3):
    # wrap_key = blake3_derive("aithos-core/v1/wrap", via_key)
    # aad      = "aithos-core/v1/tagwrap" 0 did 0 node 0 version
    via_key = bytes.fromhex("55" * 32)
    dk_new = bytes.fromhex("66" * 32)
    nonce = bytes.fromhex("77" * 24)
    node = "/e/circle/d/00000000000000000000000001"
    version = 2
    wrap_key = derive("aithos-core/v1/wrap", via_key)
    aad = (
        b"aithos-core/v1/tagwrap" + b"\x00" + DID.encode() + b"\x00"
        + node.encode() + b"\x00" + str(version).encode()
    )
    cipher = crypto_aead_xchacha20poly1305_ietf_encrypt(dk_new, aad, nonce, wrap_key)

    return {
        "vector": "G2",
        "description": "Mechanical rotation rule (spec 03.4): survivor line sets, "
                       "smuggled-recipient rejection, and the up-link wrap bytes "
                       "(step 2bis, tag-wrap primitive). Generated independently "
                       "(Python blake3+PyNaCl).",
        "seed_hex": SEED.hex(),
        "old_kids": old_kids,
        "revoked_kid": revoked,
        "expected_survivor_kids": survivors,
        "smuggled_new_kid": "zINTRUS",
        "smuggled_must_fail": "GammaRevocationRejected",
        "missing_owner_must_fail": "MissingOwnerLine",
        "uplink": {
            "via_key_hex": via_key.hex(),
            "new_dk_hex": dk_new.hex(),
            "nonce_hex": nonce.hex(),
            "node": node,
            "key_version": version,
            "subject_did": DID,
            "cipher_hex": cipher.hex(),
        },
    }


def gen_g3():
    # Auto-validation: reproduce a committed B2 value before generating
    # anything (the Python primitives must match what Rust already passes).
    b2 = json.load(open("b2-derivation.json"))
    zone_b2 = bytes.fromhex(b2["zone_dk_hex"])
    got = derive(f"aithos-core/v1/d/{b2['folder_sids'][0]}", zone_b2)
    assert got.hex() == b2["folder1_key_hex"], "B2 cross-check failed"

    # Cast (ULID sids, Crockford base32 — I,L,O,U excluded):
    #   A = old parent "archives", M = the moved folder, P = new parent
    #   "projets", S = a subfolder below M, X = a section below M.
    A = "0000000000000000000000000A"
    M = "0000000000000000000000000M"
    P = "0000000000000000000000000P"
    S = "0000000000000000000000000S"
    X = "0000000000000000000000000X"

    # Nodal dir containment (04.2): a dir names its folder by TERMINAL sid;
    # empty dir = the zone root. Same rule for op coverage (current chain)
    # and entry-vs-entry containment (recorded chains).
    containment = [
        {"dir": [A], "chain": [A, M], "covers": True},       # static prefix
        {"dir": [A, M], "chain": [P, M], "covers": True},    # direct grant survives the move
        {"dir": [A], "chain": [P, M], "covers": False},      # old parent is cut
        {"dir": [P], "chain": [P, M], "covers": True},       # new parent gains the subtree
        {"dir": [A, M], "chain": [P, M, S], "covers": True}, # whole subtree follows
        {"dir": [], "chain": [P, M], "covers": True},        # zone root covers the zone
        {"dir": [M], "chain": [P], "covers": False},         # sibling, never
        {"dir": [A, M], "chain": [A], "covers": False},      # child never covers parent
    ]

    # Derivation stability (02.5/02.9): sids are the labels, so below M
    # nothing changes; only M's own key is fresh (injected DK').
    zone_dk = bytes.fromhex("42" * 32)
    dk_a = derive(f"aithos-core/v1/d/{A}", zone_dk)
    dk_m_v1 = derive(f"aithos-core/v1/d/{M}", dk_a)     # old, un-teachable
    dk_p = derive(f"aithos-core/v1/d/{P}", zone_dk)     # new parent (never rotated)
    dk_m_v2 = bytes.fromhex("d2" * 32)                  # fresh DK' (injected)
    section_key_v2 = derive(f"aithos-core/v1/s/{X}", dk_m_v2)

    # New-path bindings (03.8): every seal binds M's NEW canonical path at
    # the new version. The old header file keeps the old-path versions.
    old_node = f"/e/circle/d/{A}/d/{M}"
    new_node = f"/e/circle/d/{P}/d/{M}"
    parent_node = f"/e/circle/d/{P}"
    new_section = f"{new_node}/s/{X}"
    version = 2

    def aad(purpose: str, node: str) -> bytes:
        return (
            purpose.encode() + b"\x00" + DID.encode() + b"\x00"
            + node.encode() + b"\x00" + str(version).encode()
        )

    line_aad = aad("aithos-core/v1/header-line", new_node)
    blob_aad = aad("aithos-core/v1/blob", new_section)
    wrap_aad = aad("aithos-core/v1/tagwrap", new_node)

    # Up-link wrap via the NEW parent (02.9): DK' sealed under a key derived
    # from the new parent's key — same primitive as G2, different via.
    nonce = bytes.fromhex("37" * 24)
    wrap_key = derive("aithos-core/v1/wrap", dk_p)
    wrap_cipher = crypto_aead_xchacha20poly1305_ietf_encrypt(
        dk_m_v2, wrap_aad, nonce, wrap_key
    )

    return {
        "vector": "G3",
        "description": "Move-as-rotation (spec 02.9): nodal dir containment "
                       "(04.2), stable derivation labels below the moved node, "
                       "new-path AAD bindings at the new version, up-link wrap "
                       "via the NEW parent. Generated independently (Python "
                       "blake3+PyNaCl), auto-validated against B2.",
        "subject_did": DID,
        "sids": {"old_parent": A, "moved": M, "new_parent": P,
                 "subfolder": S, "section": X},
        "containment": containment,
        "zone_dk_hex": zone_dk.hex(),
        "old_parent_key_hex": dk_a.hex(),
        "moved_old_key_hex": dk_m_v1.hex(),
        "new_parent_key_hex": dk_p.hex(),
        "moved_new_dk_hex": dk_m_v2.hex(),
        "section_key_v2_hex": section_key_v2.hex(),
        "old_node": old_node,
        "new_node": new_node,
        "parent_node": parent_node,
        "new_section_node": new_section,
        "key_version": version,
        "line_aad_hex": line_aad.hex(),
        "blob_aad_hex": blob_aad.hex(),
        "wrap_aad_hex": wrap_aad.hex(),
        "wrap_nonce_hex": nonce.hex(),
        "wrap_cipher_hex": wrap_cipher.hex(),
    }


if __name__ == "__main__":
    for name, gen in [("g1-revocation", gen_g1), ("g2-rotation", gen_g2),
                      ("g3-move", gen_g3)]:
        with open(f"{name}.json", "w") as f:
            json.dump(gen(), f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"wrote {name}.json")

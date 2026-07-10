#!/usr/bin/env python3
"""Independent generator for the G conformance vectors (revocation, spec 06).

  g1-revocation.json  the revoke gamma entry (JCS + owner signature),
                      authority verdicts, forward-only verdicts
  g2-rotation.json    the mechanical rotation rule (survivor line sets) and
                      the up-link wrap bytes (spec 03.4 step 2bis)

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


if __name__ == "__main__":
    for name, gen in [("g1-revocation", gen_g1), ("g2-rotation", gen_g2)]:
        with open(f"{name}.json", "w") as f:
            json.dump(gen(), f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"wrote {name}.json")

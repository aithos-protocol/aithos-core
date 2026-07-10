#!/usr/bin/env python3
"""Independent generator for the F conformance vectors (gamma log, spec 07).

Second implementation rule (vectors/README): every expected value here is
computed with Python blake3 + PyNaCl + hashlib + base58, never by the Rust
reference. The script first re-derives frozen B2 and E1 values as a sanity
check of the shared conventions (JCS, derivation, signing), then emits:

  f1-gamma-chain.json     chain + envelope + owner/delegated signatures
  f2-gamma-counting.json  subtree/windowed/per-action counting fixtures
  f3-gamma-liveness.json  heartbeat and freshness-anchor verdicts

Usage: python3 gen-f.py   (from vectors/)
"""

import hashlib
import json
from datetime import datetime, timedelta, timezone

import base58
import blake3
from nacl.bindings import (
    crypto_aead_xchacha20poly1305_ietf_encrypt,
    crypto_sign_ed25519_pk_to_curve25519,
)
from nacl.signing import SigningKey

# ---------------------------------------------------------------- wire/jcs

CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"


def ulid(n: int) -> str:
    out = []
    for _ in range(26):
        out.append(CROCKFORD[n & 31])
        n >>= 5
    return "".join(reversed(out))


def jcs(obj) -> str:
    # RFC 8785 for the subset used on the wire (ASCII strings, integers).
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def multibase_ed(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xed\x01" + pub).decode()


def multibase_x(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xec\x01" + pub).decode()


def derive(context: str, key: bytes) -> bytes:
    return blake3.blake3(key, derive_key_context=context).digest()


def sha256_hex(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def sign_doc(doc: dict, sk: SigningKey) -> dict:
    doc = dict(doc)
    doc["signature"] = dict(doc["signature"], value="")
    sig = sk.sign(jcs(doc).encode()).signature
    doc["signature"] = dict(doc["signature"], value=sig.hex())
    return doc


def aad(purpose: bytes, did: str, node: str, key_version: int) -> bytes:
    return purpose + b"\x00" + did.encode() + b"\x00" + node.encode() + b"\x00" + str(key_version).encode()


# ------------------------------------------------------------- conventions

CTX_ROOT = "aithos-core/v1/root-sign"
CTX_CONTENT = "aithos-core/v1/content-sign"
CTX_HINT = "aithos-core/v1/gamma-hint"
PURPOSE_BODY = b"aithos-core/v1/gamma-body"
MANDATE_VERSION = "1.0.0-draft.1"

SEED = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
AGENT_SK = bytes.fromhex("a1" * 32)
HELPER_SK = bytes.fromhex("b2" * 32)
ZONE_DK = bytes.fromhex("a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf")

root_sk = SigningKey(derive(CTX_ROOT, SEED))
content_sk = SigningKey(derive(CTX_CONTENT, SEED))
agent_sk = SigningKey(AGENT_SK)
helper_sk = SigningKey(HELPER_SK)
DID = "did:aithos:" + multibase_ed(bytes(root_sk.verify_key))


def node_key(zone_dk: bytes, folder_sids, section_sid=None) -> bytes:
    k = zone_dk
    for sid in folder_sids:
        k = derive(f"aithos-core/v1/d/{sid}", k)
    if section_sid is not None:
        k = derive(f"aithos-core/v1/s/{section_sid}", k)
    return k


def sanity():
    b2 = json.load(open("b2-derivation.json"))
    zk = bytes.fromhex(b2["zone_dk_hex"])
    f1 = node_key(zk, b2["folder_sids"][:1])
    deep = node_key(zk, b2["folder_sids"], b2["section_sid"])
    assert f1.hex() == b2["folder1_key_hex"], "B2 folder1 mismatch"
    assert deep.hex() == b2["deep_section_key_hex"], "B2 deep mismatch"

    e1 = json.load(open("e1-mandate.json"))
    m = json.loads(e1["mandate_jcs"])
    assert jcs(m) == e1["mandate_jcs"], "E1 JCS mismatch"
    resigned = sign_doc(m, SigningKey(derive(CTX_ROOT, bytes.fromhex(e1["seed_hex"]))))
    assert resigned["signature"]["value"] == e1["signature_hex"], "E1 signature mismatch"
    print("sanity: B2 + E1 reproduced byte-for-byte")


# --------------------------------------------------------------- mandates


def grantee_block(gid: str, label: str, sk: SigningKey) -> dict:
    pub = bytes(sk.verify_key)
    return {
        "id": gid,
        "label": label,
        "pubkey": multibase_ed(pub),
        "kex_pubkey": multibase_x(crypto_sign_ed25519_pk_to_curve25519(pub)),
    }


def build_root_mandate(mid, sk_grantee, perimeter, constraints, nb, na, nonce):
    m = {
        "aithos-mandate-core": MANDATE_VERSION,
        "id": mid,
        "subject": DID,
        "parent": None,
        "issued_by": f"{DID}#root",
        "grantee": grantee_block("urn:aithos:agent:agent", "agent", sk_grantee),
        "perimeter": perimeter,
        "constraints": constraints,
        "not_before": nb,
        "not_after": na,
        "issued_at": nb,
        "nonce": nonce,
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return sign_doc(m, root_sk)


def build_sub_mandate(parent, mid, sk_parent, sk_grantee, perimeter, constraints, nb, na, nonce):
    m = {
        "aithos-mandate-core": MANDATE_VERSION,
        "id": mid,
        "subject": DID,
        "parent": parent["id"],
        "issued_by": parent["grantee"]["pubkey"],
        "grantee": grantee_block("urn:aithos:agent:helper", "helper", sk_grantee),
        "perimeter": perimeter,
        "constraints": constraints,
        "not_before": nb,
        "not_after": na,
        "issued_at": nb,
        "nonce": nonce,
        "signature": {"alg": "ed25519", "key": parent["grantee"]["pubkey"], "value": ""},
    }
    return sign_doc(m, sk_parent)


# ----------------------------------------------------------------- gamma


def entry_hash(entry: dict) -> str:
    return "sha256:" + sha256_hex(jcs(entry).encode())


def sign_entry(entry: dict, sk: SigningKey) -> dict:
    return sign_doc(entry, sk)


def owner_entry(eid, prev, at, kind, *, target=None, payload=None, body_enc=None):
    e = {"v": 1, "id": eid, "prev": prev, "at": at, "kind": kind,
         "signature": {"alg": "ed25519", "key": "#content", "value": ""}}
    if target is not None:
        e["target"] = target
    if payload is not None:
        e["payload"] = payload
    if body_enc is not None:
        e["body_enc"] = body_enc
    return sign_entry(e, content_sk)


def delegated_entry(eid, prev, at, kind, sk, via, *, target=None, payload=None, body_enc=None):
    e = {"v": 1, "id": eid, "prev": prev, "at": at, "kind": kind,
         "authorized_by": via[-1], "authorized_via": via,
         "signature": {"alg": "ed25519", "key": multibase_ed(bytes(sk.verify_key)), "value": ""}}
    if target is not None:
        e["target"] = target
    if payload is not None:
        e["payload"] = payload
    if body_enc is not None:
        e["body_enc"] = body_enc
    return sign_entry(e, sk)


def seal_body(nkey: bytes, target: str, payload: dict, nonce: bytes, key_version=1) -> dict:
    plain = jcs({"payload": payload, "target": target}).encode()
    c = crypto_aead_xchacha20poly1305_ietf_encrypt(
        plain, aad(PURPOSE_BODY, DID, target, key_version), nonce, nkey)
    return {"hint": derive(CTX_HINT, nkey).hex(), "n": nonce.hex(), "c": c.hex()}


# ------------------------------------------------------------------- F1


def gen_f1():
    folder_sid, section_sid = ulid(1), ulid(7)
    target = f"/e/circle/d/{folder_sid}/s/{section_sid}"
    nkey = node_key(ZONE_DK, [folder_sid], section_sid)
    nonce = bytes.fromhex("0f" * 24)

    mandate = build_root_mandate(
        "mandate_" + ulid(64), agent_sk, ["act.x.gmail.*"], {"max_actions": 3},
        "2026-07-01T00:00:00Z", "2026-08-01T00:00:00Z", "00" * 16)

    e1 = owner_entry("gamma_" + ulid(1), "", "2026-07-01T00:00:00Z", "section.add",
                     body_enc=seal_body(nkey, target, {"note": "hello"}, nonce))
    e2 = owner_entry("gamma_" + ulid(2), entry_hash(e1), "2026-07-01T00:05:00Z",
                     "heartbeat", payload={"seq": 1})
    e3 = delegated_entry("gamma_" + ulid(3), entry_hash(e2), "2026-07-02T00:00:00Z",
                         "action", agent_sk, [mandate["id"]], target="x.gmail",
                         payload={"action": "reply", "args_hash": "sha256:" + sha256_hex(b"args")})

    return {
        "vector": "F1",
        "description": "Gamma chain, two-layer envelope, owner (content key) and "
                       "delegated (grantee key) entry signatures (spec 07.1-07.3). "
                       "Sealed body under the target node key, purpose gamma-body; "
                       "hint via gamma-hint. Generated independently "
                       "(Python blake3+PyNaCl+hashlib+base58).",
        "seed_hex": SEED.hex(),
        "agent_sk_hex": AGENT_SK.hex(),
        "zone_dk_hex": ZONE_DK.hex(),
        "folder_sid": folder_sid,
        "section_sid": section_sid,
        "target": target,
        "body_nonce_hex": nonce.hex(),
        "key_version": 1,
        "node_key_hex": nkey.hex(),
        "hint_hex": derive(CTX_HINT, nkey).hex(),
        "mandate_jcs": jcs(mandate),
        "entry1_jcs": jcs(e1),
        "entry1_hash": entry_hash(e1),
        "entry2_jcs": jcs(e2),
        "entry2_hash": entry_hash(e2),
        "entry3_jcs": jcs(e3),
        "entry3_hash": entry_hash(e3),
        "gamma_head": entry_hash(e3),
    }


# ------------------------------------------------------------------- F2


def gen_f2():
    nb, na = "2026-07-01T00:00:00Z", "2026-08-01T00:00:00Z"
    root = build_root_mandate(
        "mandate_" + ulid(64), agent_sk, ["act.x.gmail.*", "issue#depth=1"],
        {"max_actions": 3, "max_children": 1,
         "max_actions_per": {"window": "24h", "n": 2},
         "rate_limit": {"action": "reply", "window": "72h", "n": 2}},
        nb, na, "00" * 16)
    leaf = build_sub_mandate(
        root, "mandate_" + ulid(65), agent_sk, helper_sk, ["act.x.gmail.reply"],
        {"max_actions": 2}, nb, na, "01" * 16)
    ghost = build_sub_mandate(
        root, "mandate_" + ulid(66), agent_sk, helper_sk, ["act.x.gmail.reply"],
        {}, nb, na, "02" * 16)

    def act(eid, prev, at, sk, via, action):
        return delegated_entry(eid, prev, at, "action", sk, via, target="x.gmail",
                               payload={"action": action,
                                        "args_hash": "sha256:" + sha256_hex(at.encode())})

    e1 = delegated_entry("gamma_" + ulid(10), "", "2026-07-02T00:00:00Z", "grant",
                         agent_sk, [root["id"]], target=leaf["id"], payload={})
    e2 = act("gamma_" + ulid(11), entry_hash(e1), "2026-07-02T01:00:00Z",
             agent_sk, [root["id"]], "reply")
    e3 = act("gamma_" + ulid(12), entry_hash(e2), "2026-07-02T02:00:00Z",
             helper_sk, [root["id"], leaf["id"]], "reply")
    e4 = act("gamma_" + ulid(13), entry_hash(e3), "2026-07-03T05:00:00Z",
             helper_sk, [root["id"], leaf["id"]], "label")
    entries = [e1, e2, e3, e4]

    return {
        "vector": "F2",
        "description": "Gamma counting (spec 07.4): subtree max_actions via "
                       "authorized_via, max_children via logged grant entries, "
                       "rolling-window max_actions_per, per-action rate_limit, "
                       "and the unlogged-grant fail-closed rule. Counts computed "
                       "independently in Python over the fixture entries.",
        "seed_hex": SEED.hex(),
        "agent_sk_hex": AGENT_SK.hex(),
        "helper_sk_hex": HELPER_SK.hex(),
        "root_mandate_jcs": jcs(root),
        "leaf_mandate_jcs": jcs(leaf),
        "ghost_mandate_jcs": jcs(ghost),
        "entries_jcs": [jcs(e) for e in entries],
        "gamma_head": entry_hash(entries[-1]),
        "expected": {
            "actions_via_root": 3,
            "actions_via_leaf": 2,
            "children_of_root": 1,
            "root_window_24h_at_2026-07-02T23:59:59Z": 2,
            "root_window_24h_at_2026-07-03T23:59:59Z": 1,
            "root_replies_window_72h_at_2026-07-03T00:00:00Z": 2,
            "next_action_via_root_must_fail": "GammaBudgetExhausted",
            "next_action_via_leaf_must_fail": "GammaBudgetExhausted",
            "second_child_of_root_must_fail": "GammaBudgetExhausted",
            "action_under_ghost_chain_must_fail": "GammaGrantNotLogged",
        },
    }


# ------------------------------------------------------------------- F3


def iso(dt):
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


def gen_f3():
    t0 = datetime(2026, 7, 1, tzinfo=timezone.utc)
    beacon1 = owner_entry("gamma_" + ulid(20), "", iso(t0), "heartbeat", payload={"seq": 1})
    beacon2 = owner_entry("gamma_" + ulid(21), entry_hash(beacon1),
                          iso(t0 + timedelta(days=40)), "heartbeat", payload={"seq": 2})
    limit = t0 + timedelta(days=30, hours=72)
    return {
        "vector": "F3",
        "description": "Heartbeat window (spec 07.5, 04.8) and freshness anchor "
                       "(spec 07.7). every=30d grace=72h from beacon1; anchor "
                       "tolerance freshness=24h. Instants and verdicts computed "
                       "independently with Python datetime.",
        "seed_hex": SEED.hex(),
        "agent_sk_hex": AGENT_SK.hex(),
        "beacon1_jcs": jcs(beacon1),
        "beacon2_jcs": jcs(beacon2),
        "heartbeat": {"every": "30d", "grace": "72h"},
        "suspend_after": iso(limit),
        "verdicts_after_beacon1": {
            iso(t0 + timedelta(days=20)): "valid",
            iso(limit): "valid",
            iso(limit + timedelta(seconds=1)): "GammaHeartbeatStale",
            iso(t0 + timedelta(days=34)): "GammaHeartbeatStale",
        },
        "verdict_after_beacon2": {iso(t0 + timedelta(days=41)): "valid"},
        "forged_beacon_signer": "grantee key",
        "forged_beacon_must_fail": "InvalidGammaEntry",
        "freshness": "24h",
        "anchor_at": iso(t0),
        "anchor_verdicts": {
            iso(t0 + timedelta(hours=12)): "valid",
            iso(t0 + timedelta(hours=24)): "valid",
            iso(t0 + timedelta(hours=48)): "GammaStaleAnchor",
        },
    }


if __name__ == "__main__":
    sanity()
    for name, gen in [("f1-gamma-chain", gen_f1), ("f2-gamma-counting", gen_f2),
                      ("f3-gamma-liveness", gen_f3)]:
        with open(f"{name}.json", "w") as f:
            json.dump(gen(), f, indent=2, ensure_ascii=False)
            f.write("\n")
        print(f"wrote {name}.json")

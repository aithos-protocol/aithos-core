#!/usr/bin/env python3
"""Independent generator for the P conformance vectors (piste P, lot P0 —
INFRA-PROVIDER.md annexes A/B/C, HANDOFF-PROVIDER-AWS.md):

  P1  p1-store-envelope.json    signed request envelope X-Aithos-Auth (annexe A.2)
                                accept cases + rejects: clock skew, replayed
                                nonce, revoked chain, expired window, not
                                covered, bad signature, key/leaf mismatch
  P2  p2-store-cas.json         the two hot heads (annexe A.5): manifest publish
                                CAS + single-entry gamma append CAS, mismatch
                                and cas_required negatives
  P3  p3-tunnel-register.json   relay registration line (annexe B.2) + rejects
  P4  p4-witness-checkpoint.json checkpoint, feed lines, daily root, the
                                equivocation rule (annexe C)

Second-implementation rule: every expected value computed with Python
blake3 + PyNaCl + base58 + hashlib, never by the Rust service (which does
not exist yet — these vectors are its contract, replayed against it at the
P2 gate). Anchored on committed vectors before emitting:
  - A1: owner keys re-derived from the committed seed must match byte for
    byte (blake3 derive + Ed25519 + multibase drift check);
  - G1: the committed owner-signed revoke entry must verify under the
    re-derived content key (signature-convention drift check).

Deterministic by construction: fixed seeds, fixed instants, fixed nonces;
`server_now` is an input of every case, never wall-clock.

Usage: python3 gen-p.py   (from vectors/)
"""

import base64
import hashlib
import json

import base58
import blake3
import nacl.bindings
import nacl.signing

CORE = "1.0.0-draft.1"
DID_PATH_TENANT = "acme"
STORE_HOST = "store.aithos.fr"

W_LEAF = b"aithos-witness/v1/mk-leaf\x00"
W_NODE = b"aithos-witness/v1/mk-node\x00"


# ------------------------------------------------------------ primitives

def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def b3_hex(data: bytes) -> str:
    return blake3.blake3(data).hexdigest()


def derive(context: str, key: bytes) -> bytes:
    return blake3.blake3(key, derive_key_context=context).digest()


def mb_ed(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xed\x01" + pub).decode()


def mb_x(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xec\x01" + pub).decode()


def ed2x_pub(pub: bytes) -> bytes:
    return nacl.bindings.crypto_sign_ed25519_pk_to_curve25519(pub)


def sign_doc(doc: dict, sk: nacl.signing.SigningKey) -> dict:
    """Shared signing convention (spec 01.4): the signature covers the JCS
    of the document with signature.value = ""; value is hex Ed25519."""
    unsigned = json.loads(jcs(doc))
    unsigned["signature"]["value"] = ""
    sig = sk.sign(jcs(unsigned).encode()).signature
    signed = json.loads(jcs(doc))
    signed["signature"]["value"] = sig.hex()
    return signed


def verify_doc(doc: dict, pub: bytes) -> bool:
    unsigned = json.loads(jcs(doc))
    sig = bytes.fromhex(unsigned["signature"]["value"])
    unsigned["signature"]["value"] = ""
    try:
        nacl.signing.VerifyKey(pub).verify(jcs(unsigned).encode(), sig)
        return True
    except Exception:
        return False


def b64url(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).decode().rstrip("=")


def entry_head(entry_jcs: str) -> str:
    return "sha256:" + sha256_hex(entry_jcs.encode())


def manifest_chain_hash(manifest: dict) -> str:
    m = json.loads(jcs(manifest))
    m["signature"]["value"] = ""
    return sha256_hex(jcs(m).encode())


def h_leaf(p: bytes) -> bytes:
    return blake3.blake3(W_LEAF + p).digest()


def h_node(l: bytes, r: bytes) -> bytes:
    return blake3.blake3(W_NODE + l + r).digest()


def mroot(hashes: list) -> bytes:
    if not hashes:
        return b"\x00" * 32
    if len(hashes) == 1:
        return hashes[0]
    mid = (len(hashes) + 1) // 2
    return h_node(mroot(hashes[:mid]), mroot(hashes[mid:]))


# ------------------------------------------------------------ identities

SEED = bytes.fromhex(
    "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")

root_sk = nacl.signing.SigningKey(derive("aithos-core/v1/root-sign", SEED))
content_sk = nacl.signing.SigningKey(derive("aithos-core/v1/content-sign", SEED))
kex_sk = derive("aithos-core/v1/owner-kex", SEED)
kex_pub = nacl.bindings.crypto_scalarmult_base(kex_sk)

ROOT_PUB = root_sk.verify_key.encode()
CONTENT_PUB = content_sk.verify_key.encode()
DID = "did:aithos:" + mb_ed(ROOT_PUB)

# succession: independent of S by design (spec 01.1)
succ_sk = nacl.signing.SigningKey(b"\xaa" * 32)
# the P0 store agent (grantee), the gateway and the witness
agent_sk = nacl.signing.SigningKey(b"\x42" * 32)
AGENT_PUB = agent_sk.verify_key.encode()
gateway_sk = nacl.signing.SigningKey(b"\x51" * 32)
GATEWAY_PUB = gateway_sk.verify_key.encode()
witness_sk = nacl.signing.SigningKey(b"\x77" * 32)
WITNESS_PUB = witness_sk.verify_key.encode()

MANDATE_ID = "mandate_" + "0000000000000000000000P0M1"  # 26-char ULID field


# ----------------------------------------------------------- self-checks

def self_check_a1():
    a1 = json.load(open("a1-genesis.json"))
    assert a1["seed_hex"] == SEED.hex(), "A1 seed drift"
    assert ROOT_PUB.hex() == a1["root_sign_pub_hex"], "A1 root pub drift"
    assert CONTENT_PUB.hex() == a1["content_sign_pub_hex"], "A1 content pub drift"
    assert kex_pub.hex() == a1["owner_kex_pub_hex"], "A1 kex pub drift"
    assert mb_ed(ROOT_PUB) == a1["root_sign_pub_multibase"], "A1 multibase drift"
    assert DID == a1["did"], "A1 did drift"


def self_check_g1():
    g1 = json.load(open("g1-revocation.json"))
    entry = json.loads(g1["entry_jcs"])
    assert verify_doc(entry, CONTENT_PUB), \
        "G1 committed revoke entry does not verify: signing-convention drift"
    assert entry_head(g1["entry_jcs"]) == g1["entry_hash"], "G1 head drift"


# ------------------------------------------------------------- fixtures

def build_did_doc():
    doc = {
        "aithos-did-core": CORE,
        "id": DID,
        "keys": {"root": mb_ed(ROOT_PUB), "content": mb_ed(CONTENT_PUB),
                 "kex": mb_x(kex_pub),
                 "succession": mb_ed(succ_sk.verify_key.encode())},
        "revocations": "gamma/gamma.jsonl",
        "bundle": [f"https://{STORE_HOST}/t/{DID_PATH_TENANT}/{DID}"],
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return sign_doc(doc, root_sk)


def build_mandate():
    doc = {
        "aithos-mandate-core": CORE,
        "id": MANDATE_ID,
        "subject": DID,
        "parent": None,
        "issued_by": DID + "#root",
        "grantee": {"id": "urn:aithos:agent:p0-store-vectors",
                    "label": "P0 store agent",
                    "pubkey": mb_ed(AGENT_PUB),
                    "kex_pubkey": mb_x(ed2x_pub(AGENT_PUB))},
        "perimeter": ["read.circle", "append.circle", "act.x.gmail.reply"],
        "constraints": {},
        "not_before": "2026-07-01T00:00:00Z",
        "not_after": "2026-08-01T00:00:00Z",
        "issued_at": "2026-07-01T00:00:00Z",
        "nonce": "p0vectors0001",
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return sign_doc(doc, root_sk)


def envelope(method, path, body: bytes, at, nonce, mandate, key, sk,
             host=STORE_HOST):
    env = {"v": 1, "host": host, "method": method, "path": path,
           "body_b3": b3_hex(body) if body else "",
           "at": at, "nonce": nonce, "mandate": mandate, "key": key,
           "signature": {"alg": "ed25519", "value": ""}}
    return sign_doc(env, sk)


def header_of(env: dict) -> str:
    return b64url(jcs(env).encode())


def corrupt_sig(doc: dict) -> dict:
    bad = json.loads(jcs(doc))
    v = bad["signature"]["value"]
    bad["signature"]["value"] = v[:-2] + ("00" if v[-2:] != "00" else "ff")
    return bad


# --------------------------------------------------------------- P1

def build_p1(did_doc, mandate):
    base = f"/t/{DID_PATH_TENANT}/{DID}"
    blob = base + "/e/circle/blobs/01000000000000000000000000.enc"
    self_blob = base + "/e/self/blobs/01000000000000000000000000.enc"
    agent_mb = mb_ed(AGENT_PUB)

    # gamma states the cases reference (chain of custody for chain_revoked)
    g_grant = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P0G1", "prev": "",
        "at": "2026-07-16T12:00:00Z", "kind": "grant", "target": MANDATE_ID,
        "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
        content_sk)
    h1 = entry_head(jcs(g_grant))
    g_action = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P0A2", "prev": h1,
        "at": "2026-07-16T12:05:00Z", "kind": "action", "target": "x.gmail",
        "authorized_by": MANDATE_ID, "authorized_via": [MANDATE_ID],
        "payload": {"action": "reply", "args_hash": "sha256:" + "cd" * 32},
        "signature": {"alg": "ed25519", "key": agent_mb, "value": ""}},
        agent_sk)
    h2 = entry_head(jcs(g_action))
    g_revoke = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P0R3", "prev": h2,
        "at": "2026-07-16T13:00:00Z", "kind": "revoke", "target": MANDATE_ID,
        "payload": {"reason": "p0-vector"},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
        content_sk)

    def case(name, env, server_now, gamma_state, expect, note,
             nonce_seen=False, body=b""):
        return {"name": name, "envelope": env,
                "x_aithos_auth": header_of(env),
                "request_body_utf8": body.decode(),
                "server_now": server_now,
                "gamma_state": gamma_state, "nonce_seen_before": nonce_seen,
                "expect": expect, "note": note}

    put_body = b"# hello\n"
    cases = [
        case("accept_get_mandated",
             envelope("GET", blob, b"", "2026-07-16T12:10:00Z",
                      "p0-n-accept-get-01", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T12:10:30Z", "post_grant", {"status": "accept"},
             "read.circle covers e/circle/blobs/** (path-map A.3)"),
        case("accept_put_owner_root",
             envelope("PUT", base + "/e/public/hello.md", put_body,
                      "2026-07-16T11:20:00Z", "p0-n-accept-put-02",
                      [], "#root", root_sk),
             "2026-07-16T11:20:05Z", "empty", {"status": "accept"},
             "owner #root covers everything on the DID; body_b3 = BLAKE3(body)",
             body=put_body),
        case("accept_skew_boundary_300s",
             envelope("GET", blob, b"", "2026-07-16T12:00:00Z",
                      "p0-n-accept-skew-03", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T12:05:00Z", "post_grant", {"status": "accept"},
             "|now - at| = 300 s exactly: within tolerance (annexe A.2 #5)"),
        case("reject_clock_skew_301s",
             envelope("GET", blob, b"", "2026-07-16T12:00:00Z",
                      "p0-n-rej-skew-04", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T12:05:01Z", "post_grant",
             {"status": 401, "error": "clock_skew"},
             "301 s > 300 s tolerance"),
        case("reject_nonce_replayed",
             envelope("GET", blob, b"", "2026-07-16T12:10:00Z",
                      "p0-n-accept-get-01", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T12:10:40Z", "post_grant",
             {"status": 401, "error": "nonce_replayed"},
             "same (key, nonce) as accept_get_mandated presented again",
             nonce_seen=True),
        case("reject_signature_invalid",
             corrupt_sig(envelope("GET", blob, b"", "2026-07-16T12:11:00Z",
                                  "p0-n-rej-sig-05", [MANDATE_ID], agent_mb,
                                  agent_sk)),
             "2026-07-16T12:11:00Z", "post_grant",
             {"status": 401, "error": "signature_invalid"},
             "envelope signature corrupted (last byte)"),
        case("reject_window_expired",
             envelope("GET", blob, b"", "2026-09-01T00:00:00Z",
                      "p0-n-rej-window-06", [MANDATE_ID], agent_mb, agent_sk),
             "2026-09-01T00:00:00Z", "post_grant",
             {"status": 403, "error": "chain_invalid"},
             "mandate not_after = 2026-08-01: window fails at `at` (A.2 #9)"),
        case("reject_not_covered",
             envelope("GET", self_blob, b"", "2026-07-16T12:12:00Z",
                      "p0-n-rej-cover-07", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T12:12:00Z", "post_grant",
             {"status": 403, "error": "not_covered"},
             "perimeter reads circle only; e/self/** default-denied (A.3)"),
        case("reject_chain_revoked",
             envelope("GET", blob, b"", "2026-07-16T13:05:00Z",
                      "p0-n-rej-revoked-08", [MANDATE_ID], agent_mb, agent_sk),
             "2026-07-16T13:05:00Z", "post_revoke",
             {"status": 403, "error": "chain_revoked"},
             "revoke entry at 13:00Z < at: forward-only cut (spec 06.4)"),
        case("reject_key_leaf_mismatch",
             envelope("GET", blob, b"", "2026-07-16T12:13:00Z",
                      "p0-n-rej-key-09", [MANDATE_ID], mb_ed(GATEWAY_PUB),
                      gateway_sk),
             "2026-07-16T12:13:00Z", "post_grant",
             {"status": 403, "error": "chain_invalid"},
             "envelope key is not the chain leaf grantee.pubkey (A.2 #7)"),
    ]
    return {
        "vector": "P1",
        "description": "Signed store request envelope X-Aithos-Auth "
                       "(INFRA-PROVIDER annexe A.2, contrat C1, lot P0). "
                       "Envelope = JCS-signed {v, host, method, path, "
                       "body_b3, at, nonce, mandate, key, signature}; header "
                       "value = base64url(JCS) without padding; signature "
                       "covers JCS with signature.value=\"\" (spec 01.4 "
                       "convention). Accept cases + fail-closed rejects: "
                       "clock skew (±300 s), replayed nonce, revoked chain "
                       "(forward-only), expired window, not covered "
                       "(path-map default deny), corrupted signature, "
                       "key/leaf mismatch. server_now is an input: replay "
                       "needs no wall clock. Generated independently "
                       "(Python blake3 + PyNaCl + base58), anchored on "
                       "committed A1 + G1.",
        "tenant": DID_PATH_TENANT,
        "did": DID,
        "did_json_jcs": jcs(did_doc),
        "mandate_jcs": jcs(mandate),
        "agent_sk_hex": agent_sk._seed.hex(),
        "gamma_states": {
            "empty": [],
            "post_grant": [jcs(g_grant), jcs(g_action)],
            "post_revoke": [jcs(g_grant), jcs(g_action), jcs(g_revoke)],
        },
        "cases": cases,
    }, g_grant, g_action


# --------------------------------------------------------------- P2

def build_p2(did_doc, g_grant, g_action):
    did_json_bytes = jcs(did_doc).encode()
    hello1 = b"# hello\n"

    def manifest(height, prev_hash, files, created_at):
        doc = {"aithos-core": CORE,
               "edition": {"height": height, "prev_hash": prev_hash,
                           "created_at": created_at},
               "files": files, "gamma_head": "",
               "signature": {"alg": "ed25519", "key": "#root", "value": ""}}
        return sign_doc(doc, root_sk)

    m1 = manifest(1, "", {"did.json": sha256_hex(did_json_bytes)},
                  "2026-07-16T11:00:00Z")
    c1 = manifest_chain_hash(m1)
    m2 = manifest(2, c1, {"did.json": sha256_hex(did_json_bytes),
                          "e/public/hello.md": sha256_hex(hello1)},
                  "2026-07-16T11:30:00Z")
    c2 = manifest_chain_hash(m2)
    # the equivocation twin (also consumed by P4): same height, same parent,
    # different content — the owner double-signs, the store CAS would refuse
    # the second, a second replica could still accept it: the witness proves it
    m2b = manifest(2, c1, {"did.json": sha256_hex(did_json_bytes),
                           "e/public/hello.md": sha256_hex(b"# hello (fork)\n")},
                   "2026-07-16T11:30:00Z")
    c2b = manifest_chain_hash(m2b)
    m2x = manifest(2, "11" * 32, {"did.json": sha256_hex(did_json_bytes)},
                   "2026-07-16T11:30:00Z")

    h1 = entry_head(jcs(g_grant))
    h2 = entry_head(jcs(g_action))
    agent_mb = mb_ed(AGENT_PUB)
    g_action2 = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P0A4", "prev": h1,
        "at": "2026-07-16T12:06:00Z", "kind": "action", "target": "x.gmail",
        "authorized_by": MANDATE_ID, "authorized_via": [MANDATE_ID],
        "payload": {"action": "reply", "args_hash": "sha256:" + "ee" * 32},
        "signature": {"alg": "ed25519", "key": agent_mb, "value": ""}},
        agent_sk)

    manifest_cases = [
        {"name": "genesis_publish",
         "state_head": None, "if_head": "none", "body_jcs": jcs(m1),
         "expect": {"status": "accept", "new_head": "sha256:" + c1,
                    "new_height": 1},
         "note": "If-Head: none + height 1 + prev_hash \"\" (annexe A.5)"},
        {"name": "publish_ok",
         "state_head": "sha256:" + c1, "if_head": "sha256:" + c1,
         "body_jcs": jcs(m2),
         "expect": {"status": "accept", "new_head": "sha256:" + c2,
                    "new_height": 2},
         "note": "If-Head equals the stored chain hash; prev_hash matches"},
        {"name": "publish_cas_stale",
         "state_head": "sha256:" + c2, "if_head": "sha256:" + c1,
         "body_jcs": jcs(m2b),
         "expect": {"status": 409, "error": "cas_mismatch",
                    "head": "sha256:" + c2, "height": 2},
         "note": "the loser gets the current head back and rebases (02.6); "
                 "the store never arbitrates"},
        {"name": "publish_cas_required",
         "state_head": "sha256:" + c2, "if_head": None, "body_jcs": jcs(m2),
         "expect": {"status": 428, "error": "cas_required"},
         "note": "CAS is mandatory on manifest.json: no silent overwrite"},
        {"name": "publish_prev_hash_mismatch",
         "state_head": "sha256:" + c1, "if_head": "sha256:" + c1,
         "body_jcs": jcs(m2x),
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "prev_hash_mismatch"},
         "note": "If-Head right but the artifact does not chain (A.4)"},
    ]
    gamma_cases = [
        {"name": "append_genesis",
         "state_head": None, "if_head": "none", "entry_jcs": jcs(g_grant),
         "expect": {"status": "accept", "new_head": h1},
         "note": "empty log: If-Head none + prev \"\""},
        {"name": "append_ok",
         "state_head": h1, "if_head": h1, "entry_jcs": jcs(g_action),
         "expect": {"status": "accept", "new_head": h2},
         "note": "prev == If-Head == stored head; grantee-signed action, "
                 "chain covers act.x.gmail.reply"},
        {"name": "append_cas_stale",
         "state_head": h2, "if_head": h1, "entry_jcs": jcs(g_action2),
         "expect": {"status": 409, "error": "cas_mismatch", "head": h2},
         "note": "concurrent writer lost the race: 409 + current head, "
                 "client re-chains and retries"},
        {"name": "append_cas_required",
         "state_head": h2, "if_head": None, "entry_jcs": jcs(g_action2),
         "expect": {"status": 428, "error": "cas_required"},
         "note": "CAS is mandatory on gamma append"},
        {"name": "append_bad_entry_signature",
         "state_head": h1, "if_head": h1,
         "entry_jcs": jcs(corrupt_sig(g_action)),
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "entry_signature"},
         "note": "CAS passes, the entry itself fails verification (A.4): "
                 "the server verifies like a verifier, never repairs"},
    ]
    return {
        "vector": "P2",
        "description": "The two hot heads under CAS (INFRA-PROVIDER annexe "
                       "A.5, contrat C1, lot P0). manifest.json: head = "
                       "sha256:<chain hash> (JCS with signature.value=\"\" — "
                       "the very value a successor pins as prev_hash); gamma: "
                       "head = sha256:<sha256 of last entry JCS> (the value "
                       "the next entry carries as prev). If-Head grammar: "
                       "sha256:<64 hex> | none. Mandatory on publish and "
                       "append; mismatch answers 409 + current head; missing "
                       "answers 428. Real #root-signed manifests, real "
                       "grantee-signed entries. m2b is the equivocation twin "
                       "consumed by P4. Generated independently (Python "
                       "blake3 + PyNaCl), anchored on committed A1 + G1.",
        "tenant": DID_PATH_TENANT,
        "did": DID,
        "manifests": {"m1_jcs": jcs(m1), "m1_chain_hash": c1,
                      "m2_jcs": jcs(m2), "m2_chain_hash": c2,
                      "m2b_jcs": jcs(m2b), "m2b_chain_hash": c2b,
                      "m2x_prev_mismatch_jcs": jcs(m2x)},
        "manifest_cases": manifest_cases,
        "gamma_cases": gamma_cases,
    }, m2, m2b, c2, c2b, c1


# --------------------------------------------------------------- P3

def build_p3():
    mapping = {"tenant": DID_PATH_TENANT,
               "hostname": "demo.mcp.aithos.fr",
               "gateway_pub": mb_ed(GATEWAY_PUB)}

    def reg(hostname, at, nonce, sk=gateway_sk, tenant=DID_PATH_TENANT):
        doc = {"aithos-tunnel": CORE, "tenant": tenant, "hostname": hostname,
               "gateway_pub": mb_ed(sk.verify_key.encode()), "at": at,
               "nonce": nonce,
               "signature": {"alg": "ed25519", "value": ""}}
        return sign_doc(doc, sk)

    ok = reg("demo.mcp.aithos.fr", "2026-07-16T12:00:00Z", "p0-t-ok-01")
    cases = [
        {"name": "register_ok", "registration": ok,
         "line": jcs(ok) + "\n", "server_now": "2026-07-16T12:00:10Z",
         "suspended": False, "nonce_seen_before": False,
         "expect": {"ok": True},
         "note": "signature under gateway_pub + exact control-plane mapping "
                 "(annexe B.2)"},
        {"name": "reject_mapping_mismatch",
         "registration": reg("other.mcp.aithos.fr", "2026-07-16T12:01:00Z",
                             "p0-t-map-02"),
         "server_now": "2026-07-16T12:01:00Z", "suspended": False,
         "nonce_seen_before": False,
         "expect": {"ok": False, "error": "mapping_mismatch"},
         "note": "hostname not bound to this gateway_pub/tenant"},
        {"name": "reject_signature_invalid", "registration": corrupt_sig(ok),
         "server_now": "2026-07-16T12:00:10Z", "suspended": False,
         "nonce_seen_before": False,
         "expect": {"ok": False, "error": "signature_invalid"},
         "note": "corrupted registration signature"},
        {"name": "reject_clock_skew",
         "registration": reg("demo.mcp.aithos.fr", "2026-07-16T12:00:00Z",
                             "p0-t-skew-03"),
         "server_now": "2026-07-16T12:05:01Z", "suspended": False,
         "nonce_seen_before": False,
         "expect": {"ok": False, "error": "clock_skew"},
         "note": "301 s > 300 s tolerance"},
        {"name": "reject_nonce_replayed", "registration": ok,
         "server_now": "2026-07-16T12:00:20Z", "suspended": False,
         "nonce_seen_before": True,
         "expect": {"ok": False, "error": "nonce_replayed"},
         "note": "same registration line presented again"},
        {"name": "reject_suspended", "registration": ok,
         "server_now": "2026-07-16T12:00:10Z", "suspended": True,
         "nonce_seen_before": False,
         "expect": {"ok": False, "error": "suspended"},
         "note": "control-plane suspension refuses registration (< 60 s "
                 "propagation, P7 gate)"},
    ]
    return {
        "vector": "P3",
        "description": "Relay tunnel registration line (INFRA-PROVIDER "
                       "annexe B.2, contrat C2, lot P0). One JCS line + LF "
                       "after TLS (ALPN aithos-tunnel/1), signed by the "
                       "gateway key (spec 01.4 convention); the relay checks "
                       "form, ±300 s skew, nonce, signature, then the "
                       "control-plane mapping gateway_pub-tenant-hostname. "
                       "Response is one JSON line; failure closes the "
                       "connection. The relay never terminates public TLS "
                       "and never reads a payload byte (A3). Generated "
                       "independently (Python PyNaCl).",
        "gateway_sk_hex": gateway_sk._seed.hex(),
        "control_plane_mapping": mapping,
        "cases": cases,
    }


# --------------------------------------------------------------- P4

def build_p4(c1, m2, m2b, c2, c2b):
    def checkpoint(height, mh, observed_at):
        doc = {"aithos-witness": CORE, "did": DID,
               "edition_height": height, "manifest_hash": "sha256:" + mh,
               "gamma_head": "", "observed_at": observed_at,
               "witness_key": mb_ed(WITNESS_PUB),
               "signature": {"alg": "ed25519", "value": ""}}
        return sign_doc(doc, witness_sk)

    ck1 = checkpoint(1, c1, "2026-07-16T11:05:00Z")
    ck2 = checkpoint(2, c2, "2026-07-16T11:35:00Z")
    ck2b = checkpoint(2, c2b, "2026-07-16T11:40:00Z")
    ck_hb = checkpoint(2, c2, "2026-07-17T00:00:05Z")

    day_lines = sorted([jcs(ck1), jcs(ck2), jcs(ck2b)])
    root = mroot([h_leaf(l.encode()) for l in day_lines])
    root_doc = sign_doc({
        "aithos-witness-root": CORE, "date": "2026-07-16",
        "root": root.hex(), "n": len(day_lines),
        "witness_key": mb_ed(WITNESS_PUB),
        "signature": {"alg": "ed25519", "value": ""}}, witness_sk)

    return {
        "vector": "P4",
        "description": "Witness checkpoint (INFRA-PROVIDER annexe C, "
                       "contrat C3, lot P0). Checkpoint = JCS-signed "
                       "observation; manifest_hash = sha256: + the observed "
                       "manifest's chain hash (JCS, signature.value=\"\") — "
                       "the very value a successor pins as prev_hash; "
                       "gamma_head copied from the manifest. Feed line = the "
                       "exact signed JCS bytes. Daily root = left-heavy "
                       "mroot over H_leaf(line) with dedicated domains "
                       "aithos-witness/v1/mk-leaf|mk-node, lines sorted by "
                       "JCS byte order, all DIDs of the UTC day. "
                       "Equivocation rule: two valid checkpoints, same did, "
                       "same edition_height, different manifest_hash = "
                       "portable proof; same manifest_hash re-observed "
                       "(heartbeat) is freshness, never a fault. Manifests "
                       "are P2's m1/m2/m2b. Generated independently (Python "
                       "blake3 + PyNaCl).",
        "witness_sk_hex": witness_sk._seed.hex(),
        "witness_key": mb_ed(WITNESS_PUB),
        "checkpoints": {
            "ck1_jcs": jcs(ck1), "ck2_jcs": jcs(ck2),
            "ck2b_jcs": jcs(ck2b), "ck_heartbeat_jcs": jcs(ck_hb)},
        "feed": {"path": DID + ".jsonl",
                 "lines_2026-07-16": [jcs(ck1), jcs(ck2), jcs(ck2b)],
                 "line_2026-07-17": jcs(ck_hb)},
        "daily_root": {"doc": root_doc, "leaf_order": day_lines},
        "equivocation_cases": [
            {"name": "equivocation_proven", "pair": [jcs(ck2), jcs(ck2b)],
             "expect": {"equivocation": True},
             "note": "same did, same height 2, different manifest_hash: the "
                     "signed pair alone is the proof (annexe C.4)"},
            {"name": "heartbeat_not_equivocation",
             "pair": [jcs(ck2), jcs(ck_hb)],
             "expect": {"equivocation": False},
             "note": "same manifest_hash re-observed later = freshness"},
            {"name": "different_heights_not_equivocation",
             "pair": [jcs(ck1), jcs(ck2)],
             "expect": {"equivocation": False},
             "note": "heights differ: a chain, not a fork"},
        ],
    }


# --------------------------------------------------------------- P5

def _det(label: str, n: int) -> bytes:
    """Deterministic filler bytes (never a secret in use): blake3 XOF over
    a public label — the vectors reproduce byte for byte."""
    return blake3.blake3(b"aithos-p5/" + label.encode()).digest(length=n)


def _ext(ext_type: int, body: bytes) -> bytes:
    return ext_type.to_bytes(2, "big") + len(body).to_bytes(2, "big") + body


def _client_hello(name: str, sni, alpn, oversize=False) -> bytes:
    """A structurally honest TLS 1.3 ClientHello handshake message
    (RFC 8446 §4.1.2) with the extensions a real browser sends: SNI, ALPN,
    supported_versions, groups, signature_algorithms, key_share. All
    variable material is deterministic (`_det`)."""
    ext = b""
    if sni is not None:
        host = sni.encode("ascii")
        entry = b"\x00" + len(host).to_bytes(2, "big") + host
        ext += _ext(0x0000, len(entry).to_bytes(2, "big") + entry)
    # supported_versions: TLS 1.3, TLS 1.2
    ext += _ext(0x002B, b"\x04\x03\x04\x03\x03")
    # supported_groups: x25519, secp256r1
    ext += _ext(0x000A, b"\x00\x04\x00\x1d\x00\x17")
    # signature_algorithms: ed25519, ecdsa_secp256r1_sha256, rsa_pss_rsae_sha256
    ext += _ext(0x000D, b"\x00\x06\x08\x07\x04\x03\x08\x04")
    # key_share: one x25519 share (deterministic filler, never used)
    share = _det(f"keyshare-{name}", 32)
    ks = b"\x00\x1d" + len(share).to_bytes(2, "big") + share
    ext += _ext(0x0033, len(ks).to_bytes(2, "big") + ks)
    if alpn:
        protos = b"".join(len(p).to_bytes(1, "big") + p.encode() for p in alpn)
        ext += _ext(0x0010, len(protos).to_bytes(2, "big") + protos)
    if oversize:
        # RFC 7685 padding extension: a complete, valid hello whose total
        # crosses the 16 KiB peek bound of annexe B.4 — the relay must
        # close on the bound, not parse to the end.
        ext += _ext(0x0015, b"\x00" * (17 * 1024))
    body = (b"\x03\x03" + _det(f"random-{name}", 32)          # legacy_version, random
            + b"\x20" + _det(f"session-{name}", 32)            # legacy_session_id
            + b"\x00\x06\x13\x01\x13\x02\x13\x03"              # TLS 1.3 suites
            + b"\x01\x00"                                      # legacy_compression
            + len(ext).to_bytes(2, "big") + ext)
    return b"\x01" + len(body).to_bytes(3, "big") + body


def _records(handshake: bytes, split=None) -> bytes:
    """Wrap a handshake message in TLS records (type 0x16). `split`
    fragments it across two records at that byte offset — reassembly is the
    parser's duty, not the client's courtesy."""
    def rec(payload: bytes) -> bytes:
        return b"\x16\x03\x01" + len(payload).to_bytes(2, "big") + payload
    if split is None:
        return rec(handshake)
    return rec(handshake[:split]) + rec(handshake[split:])


def build_p5():
    def hello(name, sni, alpn, split=None, oversize=False):
        return _records(_client_hello(name, sni, alpn, oversize=oversize),
                        split=split)

    demo = hello("peek_demo_hostname", "demo.mcp.aithos.fr", ["h2", "http/1.1"])
    frag = hello("peek_fragmented_two_records", "demo.mcp.aithos.fr",
                 ["h2", "http/1.1"], split=77)
    cases = [
        {"name": "peek_demo_hostname",
         "hello_hex": demo.hex(),
         "expect": {"decision": "peeked", "sni": "demo.mcp.aithos.fr",
                    "alpn": ["h2", "http/1.1"]},
         "note": "a browser-shaped ClientHello for an org hostname: SNI and "
                 "ALPN extracted without terminating (B.1/B.4)"},
        {"name": "peek_mixed_case_is_lowercased",
         "hello_hex": hello("peek_mixed_case_is_lowercased",
                            "DeMo.McP.AiThOs.Fr", ["h2"]).hex(),
         "expect": {"decision": "peeked", "sni": "demo.mcp.aithos.fr",
                    "alpn": ["h2"]},
         "note": "B.4: matching is exact and case-insensitive — the "
                 "extractor normalizes to lowercase"},
        {"name": "peek_tunnel_door",
         "hello_hex": hello("peek_tunnel_door", "relay.aithos.fr",
                            ["aithos-tunnel/1"]).hex(),
         "expect": {"decision": "peeked", "sni": "relay.aithos.fr",
                    "alpn": ["aithos-tunnel/1"]},
         "note": "the pod's outbound hello: the relay's own name + the "
                 "tunnel ALPN — the only TLS the relay terminates (B.1/B.2)"},
        {"name": "peek_fragmented_two_records",
         "hello_hex": frag.hex(),
         "expect": {"decision": "peeked", "sni": "demo.mcp.aithos.fr",
                    "alpn": ["h2", "http/1.1"]},
         "note": "the same hello split across two TLS records: reassembly "
                 "is mandatory, fragmentation is not a bypass"},
        {"name": "no_sni_closes",
         "hello_hex": hello("no_sni_closes", None, ["h2"]).hex(),
         "expect": {"decision": "no_sni"},
         "note": "a valid hello without server_name: silent close (B.4 — "
                 "no banner, nothing to enumerate)"},
        {"name": "not_tls_closes",
         "hello_hex": b"GET / HTTP/1.1\r\nHost: demo.mcp.aithos.fr\r\n\r\n".hex(),
         "expect": {"decision": "not_tls"},
         "note": "plain HTTP on the public door: silent close (B.4)"},
        {"name": "truncated_is_incomplete",
         "hello_hex": demo[:40].hex(),
         "expect": {"decision": "incomplete"},
         "note": "a partial hello wants more bytes; stalled past the hello "
                 "deadline the relay closes dry (B.4: ≤ 10 s)"},
        {"name": "hello_over_16kib_closes",
         "hello_hex": hello("hello_over_16kib_closes", "demo.mcp.aithos.fr",
                            ["h2"], oversize=True).hex(),
         "expect": {"decision": "too_large"},
         "note": "a complete, valid hello past 16 KiB: the peek bound "
                 "closes it before routing (B.4)"},
    ]
    return {
        "vector": "P5",
        "description": "Relay SNI peek (INFRA-PROVIDER annexe B.1/B.4, "
                       "contrat C2, lot P6 jalon M2). The relay reads the "
                       "ClientHello of every inbound connection WITHOUT "
                       "terminating TLS — bounded to 16 KiB and 10 s — and "
                       "extracts (sni, alpn) to route: its own name (ALPN "
                       "aithos-tunnel/1) is the only TLS it terminates; an "
                       "active-tunnel hostname is piped from the first "
                       "byte; everything else closes without one byte "
                       "emitted. Decisions: peeked | no_sni | not_tls | "
                       "incomplete | too_large. Hellos are structurally "
                       "honest TLS 1.3 ClientHellos built independently in "
                       "Python (deterministic filler, no secret); the Rust "
                       "peek must reproduce every decision byte for byte.",
        "peek_bound_bytes": 16384,
        "hello_deadline_secs": 10,
        "cases": cases,
    }


# --------------------------------------------------------------- P6

# Second gateway identities for the acme cases: a key the control plane
# never enrolled (mapping_mismatch) and a key bound to its own hostname so
# the rate-limit sequence never touches the demo hostname's budget.
stranger_gw_sk = nacl.signing.SigningKey(b"\x52" * 32)
rate_gw_sk = nacl.signing.SigningKey(b"\x53" * 32)

ACME_PATH = "/acme/txt"
ACME_TXT_TTL = 60
ACME_PURGE_SECS = 600
ACME_MAX_PUTS_PER_HOUR = 10
ACME_RATE_WINDOW_SECS = 3600


def _acme_value(label: str) -> str:
    """A realistic ACME digest shape: 43 chars of base64url (the SHA-256
    key-authorization digest length), deterministic from a public label."""
    return b64url(blake3.blake3(b"aithos-p6/" + label.encode()).digest())


def build_p6():
    demo_host = "demo.mcp.aithos.fr"
    rate_host = "rate.mcp.aithos.fr"
    mappings = [
        {"tenant": DID_PATH_TENANT, "hostname": demo_host,
         "gateway_pub": mb_ed(GATEWAY_PUB)},
        {"tenant": DID_PATH_TENANT, "hostname": rate_host,
         "gateway_pub": mb_ed(rate_gw_sk.verify_key.encode())},
    ]

    def body_of(hostname, value):
        return jcs({"hostname": hostname, "value": value})

    def acme_env(method, body, at, nonce, sk=gateway_sk, key=None,
                 mandate=None, host=STORE_HOST):
        return envelope(method, ACME_PATH, body.encode() if body else b"",
                        at, nonce,
                        [] if mandate is None else mandate,
                        key if key is not None
                        else mb_ed(sk.verify_key.encode()),
                        sk, host=host)

    def case(name, env, body, server_now, expect, note, plane="normal",
             method=None, nonce_seen=False):
        return {"name": name,
                "plane": plane,
                "method": method or (env["method"] if env else "PUT"),
                "envelope": env,
                "x_aithos_auth": header_of(env) if env else None,
                "request_body_utf8": body,
                "server_now": server_now,
                "nonce_seen_before": nonce_seen,
                "expect": expect, "note": note}

    v_ok = _acme_value("put-ok")
    v_loose = _acme_value("loose")
    v_skewb = _acme_value("skew-boundary")
    body_ok = body_of(demo_host, v_ok)
    env_ok = acme_env("PUT", body_ok, "2026-07-18T12:00:00Z",
                      "p6-n-put-ok-0001")
    loose_body = ('{"hostname": "' + demo_host + '", "value": "'
                  + v_loose + '"}')  # same JSON, non-JCS bytes: only the
    #                                   hash binds the body (A.2 #4)

    cases = [
        case("accept_put_ok", env_ok, body_ok, "2026-07-18T12:00:10Z",
             {"status": 204,
              "dns": {"name": f"_acme-challenge.{demo_host}",
                      "value": v_ok, "ttl": ACME_TXT_TTL}},
             "the graved B.5 exception: key = gateway_pub, mandate = [], "
             "authority = the control-plane mapping; TXT posed, TTL 60"),
        case("accept_delete_ok",
             acme_env("DELETE", body_ok, "2026-07-18T12:00:20Z",
                      "p6-n-del-ok-0002"),
             body_ok, "2026-07-18T12:00:25Z",
             {"status": 204,
              "dns_deleted": f"_acme-challenge.{demo_host}"},
             "DELETE retires the record; purge after 10 min is the backstop"),
        case("accept_put_loose_body_json",
             acme_env("PUT", loose_body, "2026-07-18T12:00:30Z",
                      "p6-n-loose-0003"),
             loose_body, "2026-07-18T12:00:35Z",
             {"status": 204,
              "dns": {"name": f"_acme-challenge.{demo_host}",
                      "value": v_loose, "ttl": ACME_TXT_TTL}},
             "the body is bound by BLAKE3 bytes, not by canonicality: "
             "non-JCS JSON with the same closed fields is accepted"),
        case("accept_skew_boundary_300s",
             acme_env("PUT", body_of(demo_host, v_skewb),
                      "2026-07-18T12:00:40Z", "p6-n-skewb-0004"),
             body_of(demo_host, v_skewb), "2026-07-18T12:05:40Z",
             {"status": 204,
              "dns": {"name": f"_acme-challenge.{demo_host}",
                      "value": v_skewb, "ttl": ACME_TXT_TTL}},
             "|now - at| = 300 s exactly: within tolerance (A.2 #5)"),
        case("reject_clock_skew_301s",
             acme_env("PUT", body_of(demo_host, _acme_value("skew-301")),
                      "2026-07-18T12:00:50Z", "p6-n-skew-0005"),
             body_of(demo_host, _acme_value("skew-301")),
             "2026-07-18T12:05:51Z",
             {"status": 401, "error": "clock_skew"},
             "301 s > 300 s tolerance"),
        case("reject_nonce_replayed", env_ok, body_ok,
             "2026-07-18T12:00:30Z",
             {"status": 401, "error": "nonce_replayed"},
             "the accept_put_ok envelope presented again", nonce_seen=True),
        case("reject_signature_invalid",
             corrupt_sig(acme_env("PUT",
                                  body_of(demo_host, _acme_value("badsig")),
                                  "2026-07-18T12:01:00Z",
                                  "p6-n-badsig-0006")),
             body_of(demo_host, _acme_value("badsig")),
             "2026-07-18T12:01:00Z",
             {"status": 401, "error": "signature_invalid"},
             "envelope signature corrupted (last byte)"),
        case("reject_envelope_missing", None, body_ok,
             "2026-07-18T12:01:10Z",
             {"status": 401, "error": "envelope_missing"},
             "every /acme route demands the envelope — never a banner",
             method="PUT"),
        case("reject_mandate_not_empty",
             acme_env("PUT", body_of(demo_host, _acme_value("mandated")),
                      "2026-07-18T12:01:20Z", "p6-n-mand-0007",
                      mandate=[MANDATE_ID]),
             body_of(demo_host, _acme_value("mandated")),
             "2026-07-18T12:01:20Z",
             {"status": 400, "error": "envelope_invalid"},
             "B.5 graves mandate: [] — a chain on /acme/* is a form fault, "
             "never evaluated"),
        case("reject_owner_fragment_key",
             acme_env("PUT", body_of(demo_host, _acme_value("owner-key")),
                      "2026-07-18T12:01:30Z", "p6-n-root-0008",
                      sk=root_sk, key="#root"),
             body_of(demo_host, _acme_value("owner-key")),
             "2026-07-18T12:01:30Z",
             {"status": 400, "error": "envelope_invalid"},
             "B.5 graves key = gateway_pub (multibase): an owner fragment "
             "is a form fault — there is no DID on this surface"),
        case("reject_body_unknown_field",
             acme_env("PUT",
                      jcs({"hostname": demo_host,
                           "value": _acme_value("extra-field"),
                           "ttl": 300}),
                      "2026-07-18T12:01:40Z", "p6-n-xfield-0009"),
             jcs({"hostname": demo_host,
                  "value": _acme_value("extra-field"), "ttl": 300}),
             "2026-07-18T12:01:40Z",
             {"status": 400, "error": "envelope_invalid"},
             "the body field set is closed: {hostname, value}, nothing else"),
        case("reject_value_too_long",
             acme_env("PUT", body_of(demo_host, "A" * 256),
                      "2026-07-18T12:01:50Z", "p6-n-long-0010"),
             body_of(demo_host, "A" * 256), "2026-07-18T12:01:50Z",
             {"status": 400, "error": "envelope_invalid"},
             "value > 255 chars (B.5 bound)"),
        case("reject_value_empty",
             acme_env("PUT", body_of(demo_host, ""),
                      "2026-07-18T12:02:00Z", "p6-n-empty-0011"),
             body_of(demo_host, ""), "2026-07-18T12:02:00Z",
             {"status": 400, "error": "envelope_invalid"},
             "an empty value poses nothing"),
        case("reject_value_bad_charset",
             acme_env("PUT", body_of(demo_host, "no spaces allowed"),
                      "2026-07-18T12:02:10Z", "p6-n-chars-0012"),
             body_of(demo_host, "no spaces allowed"),
             "2026-07-18T12:02:10Z",
             {"status": 400, "error": "envelope_invalid"},
             "value alphabet is base64url [A-Za-z0-9_-] — the ACME digest "
             "shape, nothing else reaches DNS"),
        case("reject_hostname_case",
             acme_env("PUT", body_of("DeMo.McP.AiThOs.Fr",
                                     _acme_value("upper"))
                      , "2026-07-18T12:02:20Z", "p6-n-case-0013"),
             body_of("DeMo.McP.AiThOs.Fr", _acme_value("upper")),
             "2026-07-18T12:02:20Z",
             {"status": 400, "error": "envelope_invalid"},
             "hostnames are strict lowercase LDH on this surface — the "
             "client posts its enrolled name verbatim, no case games"),
        case("reject_hostname_trailing_dot",
             acme_env("PUT", body_of("demo.mcp.aithos.fr.",
                                     _acme_value("dot")),
                      "2026-07-18T12:02:30Z", "p6-n-dot-0014"),
             body_of("demo.mcp.aithos.fr.", _acme_value("dot")),
             "2026-07-18T12:02:30Z",
             {"status": 400, "error": "envelope_invalid"},
             "no trailing dot, no empty label"),
        case("reject_body_b3_mismatch",
             acme_env("PUT", body_of(demo_host, _acme_value("signed-one")),
                      "2026-07-18T12:02:40Z", "p6-n-b3-0015"),
             body_of(demo_host, _acme_value("sent-other")),
             "2026-07-18T12:02:40Z",
             {"status": 400, "error": "envelope_invalid"},
             "the envelope signs one body, the request carries another "
             "(A.2 #4)"),
        case("reject_wrong_host",
             acme_env("PUT", body_of(demo_host, _acme_value("cross-host")),
                      "2026-07-18T12:02:50Z", "p6-n-host-0016",
                      host="gateway.example.org"),
             body_of(demo_host, _acme_value("cross-host")),
             "2026-07-18T12:02:50Z",
             {"status": 400, "error": "envelope_invalid"},
             "host binds the authority: no cross-plane replay (A.2 #3)"),
        case("reject_mapping_unenrolled",
             acme_env("PUT", body_of(demo_host, _acme_value("unenrolled")),
                      "2026-07-18T12:03:00Z", "p6-n-unenr-0017",
                      sk=stranger_gw_sk),
             body_of(demo_host, _acme_value("unenrolled")),
             "2026-07-18T12:03:00Z",
             {"status": 403, "error": "mapping_mismatch"},
             "a key enrolled for no tunnel — same answer as a wrong "
             "hostname, no enumeration oracle (B.2 model)"),
        case("reject_mapping_foreign_hostname",
             acme_env("PUT", body_of("other.mcp.aithos.fr",
                                     _acme_value("foreign")),
                      "2026-07-18T12:03:10Z", "p6-n-forgn-0018"),
             body_of("other.mcp.aithos.fr", _acme_value("foreign")),
             "2026-07-18T12:03:10Z",
             {"status": 403, "error": "mapping_mismatch"},
             "the hostname MUST belong to the signer's binding (B.5)"),
        case("reject_get_not_covered",
             acme_env("GET", "", "2026-07-18T12:03:20Z", "p6-n-get-0019"),
             "", "2026-07-18T12:03:20Z",
             {"status": 403, "error": "not_covered"},
             "the route defines PUT and DELETE only — default deny, "
             "decided AFTER the envelope authenticated (A.3 model)"),
    ]

    # Anti-abus sequence on the rate hostname: 10 admitted PUTs inside the
    # rolling hour, then refusals, DELETE unbudgeted, then the hour rolls.
    rate_values = [_acme_value(f"rl-{i:02}") for i in range(1, 11)]
    for i, value in enumerate(rate_values, start=1):
        at = f"2026-07-18T12:10:{2 * (i - 1):02}Z"
        now = f"2026-07-18T12:10:{2 * (i - 1) + 1:02}Z"
        cases.append(case(
            f"rate_warmup_{i:02}",
            acme_env("PUT", body_of(rate_host, value), at,
                     f"p6-n-rl-{i:02}", sk=rate_gw_sk),
            body_of(rate_host, value), now,
            {"status": 204,
             "dns": {"name": f"_acme-challenge.{rate_host}",
                     "value": value, "ttl": ACME_TXT_TTL}},
            f"admitted PUT {i}/10 inside the rolling hour"))
    cases += [
        case("reject_rate_limited_eleventh",
             acme_env("PUT", body_of(rate_host, _acme_value("rl-11")),
                      "2026-07-18T12:10:30Z", "p6-n-rl-11-eleventh",
                      sk=rate_gw_sk),
             body_of(rate_host, _acme_value("rl-11")),
             "2026-07-18T12:10:31Z",
             {"status": 429, "error": "rate_limited"},
             "the 11th PUT inside the hour on one hostname (B.5: ≤ 10/h) — "
             "counted AFTER full authorization, a stranger cannot burn it"),
        case("accept_delete_unbudgeted",
             acme_env("DELETE", body_of(rate_host, rate_values[-1]),
                      "2026-07-18T12:10:40Z", "p6-n-rl-del-0001",
                      sk=rate_gw_sk),
             body_of(rate_host, rate_values[-1]), "2026-07-18T12:10:41Z",
             {"status": 204,
              "dns_deleted": f"_acme-challenge.{rate_host}"},
             "DELETE spends no PUT budget (cleanup stays possible)"),
        case("reject_rate_limited_still",
             acme_env("PUT", body_of(rate_host, _acme_value("rl-12")),
                      "2026-07-18T12:10:50Z", "p6-n-rl-12-still",
                      sk=rate_gw_sk),
             body_of(rate_host, _acme_value("rl-12")),
             "2026-07-18T12:10:51Z",
             {"status": 429, "error": "rate_limited"},
             "refusals are not admissions: the window still holds 10"),
        case("accept_put_after_hour_rolls",
             acme_env("PUT", body_of(rate_host, _acme_value("fresh-hour")),
                      "2026-07-18T13:10:20Z", "p6-n-rl-13-fresh",
                      sk=rate_gw_sk),
             body_of(rate_host, _acme_value("fresh-hour")),
             "2026-07-18T13:10:21Z",
             {"status": 204,
              "dns": {"name": f"_acme-challenge.{rate_host}",
                      "value": _acme_value("fresh-hour"),
                      "ttl": ACME_TXT_TTL}},
             "the rolling hour frees the budget"),
        case("reject_suspended_binding",
             acme_env("PUT", body_of(demo_host, _acme_value("susp-bind")),
                      "2026-07-18T12:20:00Z", "p6-n-susp-b-01"),
             body_of(demo_host, _acme_value("susp-bind")),
             "2026-07-18T12:20:01Z",
             {"status": 403, "error": "suspended"},
             "control-plane suspension of the binding refuses the write",
             plane="suspended_binding"),
        case("reject_suspended_tenant",
             acme_env("PUT", body_of(demo_host, _acme_value("susp-tenant")),
                      "2026-07-18T12:20:10Z", "p6-n-susp-t-01"),
             body_of(demo_host, _acme_value("susp-tenant")),
             "2026-07-18T12:20:11Z",
             {"status": 403, "error": "suspended"},
             "a suspended tenant suspends its bindings too",
             plane="suspended_tenant"),
    ]

    # puts_in_last_hour: the admitted-PUT count the server holds for the
    # case's hostname at its server_now — derived from the sequence itself,
    # re-derived independently by verify-p.py.
    admitted = []  # (hostname, server_now_ts)
    for c in cases:
        if c["plane"] != "normal":
            c["puts_in_last_hour"] = 0
            continue
        host_in_body = None
        if c["request_body_utf8"]:
            try:
                host_in_body = json.loads(c["request_body_utf8"]).get(
                    "hostname", "")
            except (ValueError, AttributeError):
                host_in_body = None
        now_s = _zulu_secs(c["server_now"])
        window = [t for (h, t) in admitted
                  if h == host_in_body and now_s - t < ACME_RATE_WINDOW_SECS]
        c["puts_in_last_hour"] = len(window)
        if c["expect"].get("status") == 204 and c["method"] == "PUT":
            admitted.append((host_in_body, now_s))

    return {
        "vector": "P6",
        "description": "Delegated ACME DNS-01 surface /acme/txt "
                       "(INFRA-PROVIDER annexe B.5, contrat C2, lot P6 "
                       "jalon M2). Envelope = A.2 with the GRAVED "
                       "exception: key = gateway_pub (multibase), mandate "
                       "= [] — the authority is the control-plane mapping "
                       "of the signing gateway key (the B.2 model), never "
                       "a mandate chain. Order, fail-closed: presence -> "
                       "envelope form (key multibase + mandate [] are "
                       "FORM on this surface) -> host/method/path -> "
                       "body_b3 -> skew +/-300 s -> nonce -> signature -> "
                       "verb (PUT/DELETE else not_covered) -> body form "
                       "(closed {hostname, value}, lowercase LDH "
                       "hostname, value 1..255 of [A-Za-z0-9_-]) -> "
                       "mapping (resolve by gateway_pub -> suspended -> "
                       "tenant state -> hostname match) -> rate (PUT "
                       "only, <= 10 per rolling hour per hostname) -> "
                       "DNS effect (TXT _acme-challenge.<hostname>, TTL "
                       "60, UPSERT replaces; DELETE idempotent; server "
                       "purge after 600 s is out of wire scope). Errors: "
                       "registre A.7 + mapping_mismatch (403). server_now "
                       "is an input: replay needs no wall clock. Cases "
                       "are a STATEFUL sequence per plane (nonces burn, "
                       "the rate window slides). Generated independently "
                       "(Python blake3 + PyNaCl), gateway keys are "
                       "committed TEST keys.",
        "store_host": STORE_HOST,
        "path": ACME_PATH,
        "constants": {"txt_ttl_secs": ACME_TXT_TTL,
                      "purge_after_secs": ACME_PURGE_SECS,
                      "max_puts_per_hour": ACME_MAX_PUTS_PER_HOUR,
                      "rate_window_secs": ACME_RATE_WINDOW_SECS,
                      "value_max_chars": 255},
        "gateway_sk_hex": gateway_sk._seed.hex(),
        "stranger_gateway_sk_hex": stranger_gw_sk._seed.hex(),
        "rate_gateway_sk_hex": rate_gw_sk._seed.hex(),
        "control_plane_mappings": mappings,
        "cases": cases,
    }


def _zulu_secs(s: str) -> int:
    import calendar
    import time as _time
    return calendar.timegm(_time.strptime(s, "%Y-%m-%dT%H:%M:%SZ"))


# ------------------------------------------------------------------ main

def main():
    self_check_a1()
    self_check_g1()
    did_doc = build_did_doc()
    mandate = build_mandate()
    assert verify_doc(did_doc, ROOT_PUB) and verify_doc(mandate, ROOT_PUB)

    p1, g_grant, g_action = build_p1(did_doc, mandate)
    p2, m2, m2b, c2, c2b, c1 = build_p2(did_doc, g_grant, g_action)
    p3 = build_p3()
    p4 = build_p4(c1, m2, m2b, c2, c2b)
    p5 = build_p5()
    p6 = build_p6()

    for name, data in [("p1-store-envelope.json", p1),
                       ("p2-store-cas.json", p2),
                       ("p3-tunnel-register.json", p3),
                       ("p4-witness-checkpoint.json", p4),
                       ("p5-tunnel-sni.json", p5),
                       ("p6-acme-txt.json", p6)]:
        with open(name, "w") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)
            f.write("\n")
    print("self-checks vs A1 + G1 passed; wrote p1..p6")


if __name__ == "__main__":
    main()

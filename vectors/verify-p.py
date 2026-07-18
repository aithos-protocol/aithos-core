#!/usr/bin/env python3
"""Replay harness for the P vectors (lot P0). Re-derives every expected
value FROM THE JSON FILES ONLY and simulates the server decision order of
INFRA-PROVIDER annexes A/B/C — it never imports gen-p internals. This is
the P0-level replay; the P2 gate replays the same files against the real
axum service.

Checks: JCS canonicality of every *_jcs string; every signature (DID doc,
mandate, envelopes, manifests, gamma entries, registrations, checkpoints,
daily root); every hash (body_b3, chain hashes, heads, daily root); and
every case's expected verdict against a from-scratch simulation of the
annexe A.2 order, the A.5 CAS machine, the B.2 registration order and the
C.4 equivocation rule.

Usage: python3 verify-p.py   (from vectors/)
"""

import base64
import hashlib
import json
from datetime import datetime, timezone

import base58
import blake3
import nacl.signing

TOL = 300


def jcs(obj):
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def canonical(s):
    assert jcs(json.loads(s)) == s, "non-canonical JCS string"
    return json.loads(s)


def sha256_hex(b):
    return hashlib.sha256(b).hexdigest()


def mb_decode(mb):
    raw = base58.b58decode(mb[1:])
    assert raw[:2] in (b"\xed\x01", b"\xec\x01"), "bad multicodec"
    return raw[2:]


def verify_doc(doc, pub):
    d = json.loads(jcs(doc))
    sig = bytes.fromhex(d["signature"]["value"])
    d["signature"]["value"] = ""
    try:
        nacl.signing.VerifyKey(pub).verify(jcs(d).encode(), sig)
        return True
    except Exception:
        return False


def ts(s):
    return datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def entry_head(entry_jcs):
    return "sha256:" + sha256_hex(entry_jcs.encode())


def chain_hash(manifest):
    m = json.loads(jcs(manifest))
    m["signature"]["value"] = ""
    return sha256_hex(jcs(m).encode())


W_LEAF = b"aithos-witness/v1/mk-leaf\x00"
W_NODE = b"aithos-witness/v1/mk-node\x00"


def mroot(hs):
    if not hs:
        return b"\x00" * 32
    if len(hs) == 1:
        return hs[0]
    mid = (len(hs) + 1) // 2
    return blake3.blake3(W_NODE + mroot(hs[:mid]) + mroot(hs[mid:])).digest()


# ----------------------------------------------------------------- P1

def simulate_envelope(case, did_doc, mandate, did):
    """Annexe A.2 order; returns 'accept' or an error code."""
    env = case["envelope"]
    # header/envelope byte identity
    pad = "=" * (-len(case["x_aithos_auth"]) % 4)
    assert base64.urlsafe_b64decode(case["x_aithos_auth"] + pad) == \
        jcs(env).encode(), "header != base64url(JCS(envelope))"
    # 0-1 path grammar / tenant: fixture guarantees; 2 form
    keys = {"v", "host", "method", "path", "body_b3", "at", "nonce",
            "mandate", "key", "signature"}
    if set(env) != keys or env["v"] != 1:
        return "envelope_invalid"
    # 3 host/method/path: the vector's request IS the envelope; 4 body
    body = case["request_body_utf8"].encode()
    if env["body_b3"] != (blake3.blake3(body).hexdigest() if body else ""):
        return "envelope_invalid"
    # 5 skew
    if abs((ts(case["server_now"]) - ts(env["at"])).total_seconds()) > TOL:
        return "clock_skew"
    # 6 nonce
    if case["nonce_seen_before"]:
        return "nonce_replayed"
    # 7 key resolution
    root_pub = mb_decode(did_doc["keys"]["root"])
    content_pub = mb_decode(did_doc["keys"]["content"])
    if env["key"] == "#root":
        pub, owner = root_pub, True
    elif env["key"] == "#content":
        pub, owner = content_pub, True
    else:
        pub, owner = mb_decode(env["key"]), False
        if not env["mandate"]:
            return "chain_invalid"
    # 8 envelope signature
    if not verify_doc(env, pub):
        return "signature_invalid"
    # 9 chain
    if not owner:
        if env["mandate"] != [mandate["id"]]:
            return "chain_invalid"
        if mandate["grantee"]["pubkey"] != env["key"]:
            return "chain_invalid"
        if not verify_doc(mandate, root_pub) or mandate["subject"] != did:
            return "chain_invalid"
        if not (ts(mandate["not_before"]) <= ts(env["at"])
                <= ts(mandate["not_after"])):
            return "chain_invalid"
        for line in case["_gamma"]:
            e = canonical(line)
            if e["kind"] == "revoke" and e["target"] == mandate["id"] \
                    and ts(e["at"]) <= ts(case["server_now"]):
                return "chain_revoked"
    # 10 path-map (the fixture's two perimeters)
    rel = env["path"].split(did + "/", 1)[1] if did + "/" in env["path"] else ""
    if owner:
        return "accept"
    per = mandate["perimeter"]
    if rel.startswith("e/circle/") and "read.circle" in per:
        return "accept"
    return "not_covered"


def check_p1():
    p1 = json.load(open("p1-store-envelope.json"))
    did_doc = canonical(p1["did_json_jcs"])
    mandate = canonical(p1["mandate_jcs"])
    root_pub = mb_decode(did_doc["keys"]["root"])
    content_pub = mb_decode(did_doc["keys"]["content"])
    assert did_doc["id"] == p1["did"] == "did:aithos:" + did_doc["keys"]["root"]
    assert verify_doc(did_doc, root_pub), "did.json self-signature"
    assert verify_doc(mandate, root_pub), "mandate root signature"
    # gamma fixtures: chain links + signatures
    for state, lines in p1["gamma_states"].items():
        prev = ""
        for line in lines:
            e = canonical(line)
            assert e["prev"] == prev, f"gamma chain broken in {state}"
            key = e["signature"]["key"]
            pub = {"#content": content_pub, "#root": root_pub}.get(key) \
                or mb_decode(key)
            assert verify_doc(e, pub), f"gamma entry signature in {state}"
            prev = entry_head(line)
    agent_pub = nacl.signing.SigningKey(
        bytes.fromhex(p1["agent_sk_hex"])).verify_key.encode()
    assert mb_decode(mandate["grantee"]["pubkey"]) == agent_pub
    for case in p1["cases"]:
        case["_gamma"] = p1["gamma_states"][case["gamma_state"]]
        got = simulate_envelope(case, did_doc, mandate, p1["did"])
        want = case["expect"].get("error", case["expect"]["status"])
        want = "accept" if want == "accept" else case["expect"]["error"]
        assert got == want, f"P1 {case['name']}: got {got}, want {want}"
    print(f"P1 ok ({len(p1['cases'])} cases)")
    return did_doc, root_pub, content_pub


# ----------------------------------------------------------------- P2

def check_p2(root_pub, content_pub):
    p2 = json.load(open("p2-store-cas.json"))
    man = {k: canonical(v) for k, v in p2["manifests"].items()
           if k.endswith("_jcs")}
    for tag in ("m1", "m2", "m2b"):
        m = man[f"{tag}_jcs"]
        assert verify_doc(m, root_pub), f"{tag} signature"
        assert chain_hash(m) == p2["manifests"][f"{tag}_chain_hash"]
    assert man["m2_jcs"]["edition"]["prev_hash"] == \
        p2["manifests"]["m1_chain_hash"], "m2 chains m1"

    heights = {None: 0, "sha256:" + p2["manifests"]["m1_chain_hash"]: 1,
               "sha256:" + p2["manifests"]["m2_chain_hash"]: 2}
    for c in p2["manifest_cases"]:
        state = c["state_head"]
        if c["if_head"] is None:
            got = {"status": 428, "error": "cas_required"}
        elif c["if_head"] != ("none" if state is None else state):
            got = {"status": 409, "error": "cas_mismatch", "head": state,
                   "height": heights[state]}
        else:
            m = canonical(c["body_jcs"])
            ok_sig = verify_doc(m, root_pub)
            want_prev = "" if state is None else state.split("sha256:")[1]
            if not ok_sig:
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "signature"}
            elif m["edition"]["prev_hash"] != want_prev or \
                    m["edition"]["height"] != heights[state] + 1:
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "prev_hash_mismatch"}
            else:
                got = {"status": "accept",
                       "new_head": "sha256:" + chain_hash(m),
                       "new_height": m["edition"]["height"]}
        assert got == c["expect"], f"P2 manifest {c['name']}: {got}"

    for c in p2["gamma_cases"]:
        state = c["state_head"]
        entry = canonical(c["entry_jcs"])
        if c["if_head"] is None:
            got = {"status": 428, "error": "cas_required"}
        elif c["if_head"] != ("none" if state is None else state):
            got = {"status": 409, "error": "cas_mismatch", "head": state}
        else:
            key = entry["signature"]["key"]
            pub = {"#content": content_pub, "#root": root_pub}.get(key) \
                or mb_decode(key)
            if not verify_doc(entry, pub):
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "entry_signature"}
            elif entry["prev"] != ("" if state is None else state):
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "prev_mismatch"}
            else:
                got = {"status": "accept",
                       "new_head": entry_head(c["entry_jcs"])}
        assert got == c["expect"], f"P2 gamma {c['name']}: {got}"
    print(f"P2 ok ({len(p2['manifest_cases'])} manifest + "
          f"{len(p2['gamma_cases'])} gamma cases)")
    return p2


# ----------------------------------------------------------------- P3

def check_p3():
    p3 = json.load(open("p3-tunnel-register.json"))
    mp = p3["control_plane_mapping"]
    for c in p3["cases"]:
        reg = c["registration"]
        if "line" in c:
            assert c["line"] == jcs(reg) + "\n", "line != JCS + LF"
        keys = {"aithos-tunnel", "tenant", "hostname", "gateway_pub", "at",
                "nonce", "signature"}
        if set(reg) != keys:
            got = "envelope_invalid"
        elif abs((ts(c["server_now"]) - ts(reg["at"])).total_seconds()) > TOL:
            got = "clock_skew"
        elif c["nonce_seen_before"]:
            got = "nonce_replayed"
        elif not verify_doc(reg, mb_decode(reg["gateway_pub"])):
            got = "signature_invalid"
        elif c["suspended"]:
            got = "suspended"
        elif not (reg["tenant"] == mp["tenant"]
                  and reg["hostname"] == mp["hostname"]
                  and reg["gateway_pub"] == mp["gateway_pub"]):
            got = "mapping_mismatch"
        else:
            got = "ok"
        want = "ok" if c["expect"].get("ok") else c["expect"]["error"]
        assert got == want, f"P3 {c['name']}: got {got}, want {want}"
    print(f"P3 ok ({len(p3['cases'])} cases)")


# ----------------------------------------------------------------- P4

def check_p4(p2):
    p4 = json.load(open("p4-witness-checkpoint.json"))
    wpub = mb_decode(p4["witness_key"])
    cks = {k: canonical(v) for k, v in p4["checkpoints"].items()}
    for k, ck in cks.items():
        assert verify_doc(ck, wpub), f"{k} signature"
    # cross-file: manifest_hash values are P2's chain hashes
    assert cks["ck1_jcs"]["manifest_hash"] == \
        "sha256:" + p2["manifests"]["m1_chain_hash"]
    assert cks["ck2_jcs"]["manifest_hash"] == \
        "sha256:" + p2["manifests"]["m2_chain_hash"]
    assert cks["ck2b_jcs"]["manifest_hash"] == \
        "sha256:" + p2["manifests"]["m2b_chain_hash"]
    # feed lines are the exact signed JCS bytes
    for line in p4["feed"]["lines_2026-07-16"]:
        canonical(line)
    # daily root: sorted by byte order, dedup, dedicated domains
    lines = sorted(set(p4["feed"]["lines_2026-07-16"]))
    assert lines == p4["daily_root"]["leaf_order"], "leaf order"
    root = mroot([blake3.blake3(W_LEAF + l.encode()).digest() for l in lines])
    rd = p4["daily_root"]["doc"]
    assert rd["root"] == root.hex() and rd["n"] == len(lines), "daily root"
    assert verify_doc(rd, wpub), "daily root signature"
    # equivocation rule (annexe C.4)
    for c in p4["equivocation_cases"]:
        a, b = (canonical(x) for x in c["pair"])
        both_valid = verify_doc(a, wpub) and verify_doc(b, wpub)
        equiv = (both_valid and a["did"] == b["did"]
                 and a["edition_height"] == b["edition_height"]
                 and a["manifest_hash"] != b["manifest_hash"])
        assert equiv == c["expect"]["equivocation"], f"P4 {c['name']}"
    print(f"P4 ok (4 checkpoints, daily root, "
          f"{len(p4['equivocation_cases'])} equivocation cases)")


# ----------------------------------------------------------------- P5

def peek_client_hello(data: bytes, bound: int):
    """From-scratch SNI peek (annexe B.1/B.4) — written against RFC 8446
    §4.1.2 wire layout, never against gen-p's builder. Returns
    (decision, sni, alpn): peeked | no_sni | not_tls | incomplete |
    too_large. Any structural lie is not_tls; a hello that cannot complete
    within `bound` bytes is too_large; missing bytes are incomplete."""
    # --- gather the handshake bytes out of TLS records
    hs, off = b"", 0
    hs_total = None                     # 4 + body length, once known
    while True:
        if hs_total is not None and len(hs) >= hs_total:
            break
        if off >= len(data):
            return ("incomplete", None, None)
        if data[off] != 0x16:
            return ("not_tls", None, None)
        if off + 5 > len(data):
            return ("incomplete", None, None)
        if data[off + 1] != 0x03:
            return ("not_tls", None, None)
        rlen = int.from_bytes(data[off + 3:off + 5], "big")
        frag = data[off + 5:off + 5 + rlen]
        if len(frag) < rlen:
            return ("incomplete", None, None)
        hs += frag
        off += 5 + rlen
        if hs_total is None and len(hs) >= 4:
            if hs[0] != 0x01:
                return ("not_tls", None, None)
            hs_total = 4 + int.from_bytes(hs[1:4], "big")
        if off > bound or (hs_total is not None
                           and hs_total - len(hs) + off > bound):
            return ("too_large", None, None)
    body = hs[4:hs_total]

    # --- walk the ClientHello body
    def take(n):
        nonlocal p
        if p + n > len(body):
            raise ValueError("truncated body")
        v = body[p:p + n]
        p += n
        return v
    p = 0
    try:
        take(2 + 32)                                   # version + random
        take(int.from_bytes(take(1), "big"))           # legacy_session_id
        take(int.from_bytes(take(2), "big"))           # cipher_suites
        take(int.from_bytes(take(1), "big"))           # compression
        sni, alpn = None, None
        if p < len(body):
            ext_end = p + 2 + int.from_bytes(take(2), "big")
            while p < ext_end:
                etype = int.from_bytes(take(2), "big")
                ebody = take(int.from_bytes(take(2), "big"))
                if etype == 0x0000:                    # server_name
                    q = 2                              # skip list length
                    if ebody[q] != 0x00:
                        raise ValueError("server_name type")
                    nlen = int.from_bytes(ebody[q + 1:q + 3], "big")
                    sni = ebody[q + 3:q + 3 + nlen].decode("ascii").lower()
                elif etype == 0x0010:                  # ALPN
                    alpn, q = [], 2
                    while q < len(ebody):
                        plen = ebody[q]
                        alpn.append(ebody[q + 1:q + 1 + plen].decode("ascii"))
                        q += 1 + plen
    except (ValueError, IndexError, UnicodeDecodeError):
        return ("not_tls", None, None)
    if sni is None:
        return ("no_sni", None, None)
    return ("peeked", sni, alpn)


def check_p5():
    p5 = json.load(open("p5-tunnel-sni.json"))
    bound = p5["peek_bound_bytes"]
    assert bound == 16384 and p5["hello_deadline_secs"] == 10, "B.4 bounds"
    for c in p5["cases"]:
        decision, sni, alpn = peek_client_hello(bytes.fromhex(c["hello_hex"]),
                                                bound)
        want = c["expect"]
        assert decision == want["decision"], \
            f"P5 {c['name']}: got {decision}, want {want['decision']}"
        if want["decision"] == "peeked":
            assert sni == want["sni"], f"P5 {c['name']}: sni {sni}"
            assert (alpn or []) == want.get("alpn", []), \
                f"P5 {c['name']}: alpn {alpn}"
    print(f"P5 ok ({len(p5['cases'])} cases)")


# ----------------------------------------------------------------- P6

_B64URL = set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
              "0123456789-_")


def _valid_acme_hostname(h):
    """Strict lowercase LDH, >= 2 labels, no trailing dot — written against
    the B.5 note, never against the Rust code."""
    if not isinstance(h, str) or not (1 <= len(h) <= 253) or "." not in h:
        return False
    for label in h.split("."):
        if not (1 <= len(label) <= 63):
            return False
        if label[0] == "-" or label[-1] == "-":
            return False
        if not all(c in "abcdefghijklmnopqrstuvwxyz0123456789-"
                   for c in label):
            return False
    return True


def _valid_acme_value(v, max_chars):
    return (isinstance(v, str) and 1 <= len(v) <= max_chars
            and all(c in _B64URL for c in v))


def simulate_acme(case, p6, mappings):
    """Annexe B.5 order (graved by this vector), fail-closed; returns
    'accept' or an error code. Independent of gen-p and of the Rust code."""
    env = case["envelope"]
    # presence
    if case["x_aithos_auth"] is None:
        return "envelope_missing"
    pad = "=" * (-len(case["x_aithos_auth"]) % 4)
    assert base64.urlsafe_b64decode(case["x_aithos_auth"] + pad) == \
        jcs(env).encode(), "header != base64url(JCS(envelope))"
    # envelope form — the B.5 exception is FORM here: multibase key, [].
    keys = {"v", "host", "method", "path", "body_b3", "at", "nonce",
            "mandate", "key", "signature"}
    if set(env) != keys or env["v"] != 1:
        return "envelope_invalid"
    if not (1 <= len(env["nonce"]) <= 64):
        return "envelope_invalid"
    if env["mandate"] != [] or not env["key"].startswith("z"):
        return "envelope_invalid"
    # host / method / path bind the request
    if env["host"] != p6["store_host"] or env["method"] != case["method"] \
            or env["path"] != p6["path"]:
        return "envelope_invalid"
    # body_b3 = BLAKE3(raw body)
    body = case["request_body_utf8"].encode()
    if env["body_b3"] != (blake3.blake3(body).hexdigest() if body else ""):
        return "envelope_invalid"
    # skew
    if abs((ts(case["server_now"]) - ts(env["at"])).total_seconds()) > TOL:
        return "clock_skew"
    # nonce
    if case["nonce_seen_before"]:
        return "nonce_replayed"
    # signature under gateway_pub (the envelope's own key)
    if not verify_doc(env, mb_decode(env["key"])):
        return "signature_invalid"
    # verb: the route serves PUT and DELETE only (decided after auth)
    if case["method"] not in ("PUT", "DELETE"):
        return "not_covered"
    # body form: closed field set, strict grammar
    try:
        parsed = json.loads(case["request_body_utf8"])
    except ValueError:
        return "envelope_invalid"
    if not isinstance(parsed, dict) or set(parsed) != {"hostname", "value"}:
        return "envelope_invalid"
    if not _valid_acme_hostname(parsed["hostname"]):
        return "envelope_invalid"
    if not _valid_acme_value(parsed["value"],
                             p6["constants"]["value_max_chars"]):
        return "envelope_invalid"
    # mapping: resolve by gateway_pub -> suspended -> tenant -> hostname
    binding = next((m for m in mappings
                    if m["gateway_pub"] == env["key"]), None)
    if binding is None:
        return "mapping_mismatch"
    if case["plane"] == "suspended_binding":
        return "suspended"
    if case["plane"] == "suspended_tenant":
        return "suspended"
    if binding["hostname"] != parsed["hostname"]:
        return "mapping_mismatch"
    # rate: PUT only, <= 10 per rolling hour per hostname
    if case["method"] == "PUT" and \
            case["puts_in_last_hour"] >= p6["constants"]["max_puts_per_hour"]:
        return "rate_limited"
    return "accept"


def check_p6():
    p6 = json.load(open("p6-acme-txt.json"))
    assert p6["constants"] == {"txt_ttl_secs": 60, "purge_after_secs": 600,
                               "max_puts_per_hour": 10,
                               "rate_window_secs": 3600,
                               "value_max_chars": 255}, "B.5 constants"
    mappings = p6["control_plane_mappings"]
    # the committed gateway keys really are the mapped ones
    gw_pub = nacl.signing.SigningKey(
        bytes.fromhex(p6["gateway_sk_hex"])).verify_key.encode()
    assert mb_decode(mappings[0]["gateway_pub"]) == gw_pub, "demo mapping key"
    rate_pub = nacl.signing.SigningKey(
        bytes.fromhex(p6["rate_gateway_sk_hex"])).verify_key.encode()
    assert mb_decode(mappings[1]["gateway_pub"]) == rate_pub, "rate mapping"
    stranger_pub = nacl.signing.SigningKey(
        bytes.fromhex(p6["stranger_gateway_sk_hex"])).verify_key.encode()
    assert all(mb_decode(m["gateway_pub"]) != stranger_pub
               for m in mappings), "the stranger key is enrolled nowhere"

    window = p6["constants"]["rate_window_secs"]
    admitted = []  # (hostname, server_now epoch secs) of accepted PUTs
    dns = {}       # live TXT state across the normal-plane sequence
    for case in p6["cases"]:
        # independent re-derivation of the admitted-PUT window
        host_in_body = None
        if case["request_body_utf8"]:
            try:
                host_in_body = json.loads(
                    case["request_body_utf8"]).get("hostname")
            except ValueError:
                host_in_body = None
        now_s = ts(case["server_now"]).timestamp()
        derived = len([t for (h, t) in admitted
                       if h == host_in_body and now_s - t < window])
        if case["plane"] == "normal":
            assert derived == case["puts_in_last_hour"], \
                f"P6 {case['name']}: puts_in_last_hour drift " \
                f"({derived} != {case['puts_in_last_hour']})"

        got = simulate_acme(case, p6, mappings)
        want = "accept" if case["expect"]["status"] == 204 \
            else case["expect"]["error"]
        assert got == want, f"P6 {case['name']}: got {got}, want {want}"

        # effects, replayed on the independent DNS state
        if got == "accept" and case["plane"] == "normal":
            parsed = json.loads(case["request_body_utf8"])
            name = "_acme-challenge." + parsed["hostname"]
            if case["method"] == "PUT":
                admitted.append((parsed["hostname"], now_s))
                dns[name] = parsed["value"]
                exp = case["expect"]["dns"]
                assert exp == {"name": name, "value": parsed["value"],
                               "ttl": 60}, f"P6 {case['name']}: dns effect"
            else:
                dns.pop(name, None)
                assert case["expect"]["dns_deleted"] == name, \
                    f"P6 {case['name']}: delete effect"
    print(f"P6 ok ({len(p6['cases'])} cases)")


if __name__ == "__main__":
    did_doc, root_pub, content_pub = check_p1()
    p2 = check_p2(root_pub, content_pub)
    check_p3()
    check_p4(p2)
    check_p5()
    check_p6()
    print("all P vectors replay green")

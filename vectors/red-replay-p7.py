#!/usr/bin/env python3
"""RED observation for the P7 wire cases (gate contrat P2): spawn the REAL
aithos-store-api (child process, real socket) and fire every p7 case at it.
The P1/M2 skeleton is EXPECTED to refuse everything (501 not_implemented on
manifest/cert/gamma deposits; 403 on mandated/delegated authority) — this
script records the mismatch table proving the vectors are red BEFORE any
implementation code exists.

Envelopes are signed HERE with the committed keys (the vectors_replay rule:
the harness re-signs the signed half; the vectors themselves are the
CAS-layer contract). Not a vector generator; a gate proof.

Usage: python3 red-replay-p7.py <path-to-aithos-store-api>
"""

import importlib.util
import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

import nacl.signing  # noqa: E402

HOST = "store.aithos.fr"
LISTEN = "127.0.0.1:18080"
AT = "2026-07-19T12:00:00Z"


def envelope_header(method, path, body, key, sk, mandate, nonce):
    env = gen_p.envelope(method, path, body, AT, nonce, mandate, key, sk)
    return gen_p.header_of(env)


def fire(method, path, body, header, if_head):
    req = urllib.request.Request(
        f"http://{LISTEN}{path}", data=body if body else None, method=method)
    req.add_header("Host", HOST)
    req.add_header("X-Aithos-Test-Now", AT)
    if header:
        req.add_header("X-Aithos-Auth", header)
    if if_head is not None:
        req.add_header("If-Head", if_head)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def main():
    binary = sys.argv[1]
    p7 = json.load(open("p7-store-publication.json"))
    cb2 = json.load(open("cb2-draft2-carriers.json"))
    did = p7["did"]
    base = f"/t/{p7['tenant']}/{did}"
    cb2_sk = nacl.signing.SigningKey(
        bytes.fromhex(cb2["deterministic_private_seed_hex"]["grantee"]))

    env = dict(os.environ)
    env.update({
        "AITHOS_STORE_LISTEN": LISTEN,
        "AITHOS_STORE_AUTHORITY": HOST,
        "AITHOS_STORE_BOOTSTRAP": "../rust/crates/aithos-provider/bootstrap/replay.json",
        "AITHOS_STORE_NONCE_BACKEND": "memory",
        "AITHOS_STORE_DNS_BACKEND": "off",
        "AITHOS_STORE_TEST_NOW": "1",
    })
    server = subprocess.Popen([binary], env=env, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                socket.create_connection(("127.0.0.1", 18080), 0.2).close()
                break
            except OSError:
                time.sleep(0.1)

        rows, nonce_n = [], 0

        def run_case(kind, case, method, path, body_jcs, signer):
            nonlocal nonce_n
            nonce_n += 1
            body = body_jcs.encode()
            nonce = f"p7-red-{nonce_n:04d}"
            if signer == "owner_root":
                header = envelope_header(
                    method, path, body, "#root", gen_p.root_sk, [], nonce)
            elif signer == "grantee":
                header = envelope_header(
                    method, path, body, gen_p.mb_ed(gen_p.AGENT_PUB),
                    gen_p.agent_sk, [gen_p.MANDATE_ID], nonce)
            else:  # cb2_grantee
                header = envelope_header(
                    method, path, body,
                    cb2["context"]["grantee_key"], cb2_sk,
                    case["mandate_chain"], nonce)
            status, resp = fire(method, path, body, header, case.get("if_head"))
            try:
                code = json.loads(resp).get("error", "")
            except Exception:
                code = ""
            want = case["expect"]
            want_s = want["status"] if isinstance(want["status"], int) else 200
            red = (status != want_s) or (want.get("error", "") != code and
                                         isinstance(want["status"], int))
            if want["status"] == "accept":
                red = status >= 300
            rows.append((kind, case["name"], f"{status} {code}".strip(),
                         f"{want['status']} {want.get('error', '')}".strip(),
                         "RED" if red else "GREEN"))

        for case in p7["manifest_cases"]:
            path = (f"/t/{p7['tenant']}/{case['subject_did']}/manifest.json"
                    if "subject_did" in case else f"{base}/manifest.json")
            run_case("manifest", case, "PUT", path, case["body_jcs"],
                     case["signer"])
        for case in p7["cert_cases"]:
            run_case("cert", case, "PUT", f"{base}/{case['path']}",
                     case["body_jcs"], case["signer"])
        for case in p7["gamma_cases"]:
            run_case("gamma", case, "POST", f"{base}/gamma",
                     case["entry_jcs"], case["signer"])

        width = max(len(r[1]) for r in rows)
        print(f"{'layer':9s}{'case':{width + 2}s}{'binary answered':22s}"
              f"{'vector expects':26s}verdict")
        for kind, name, got, want, verdict in rows:
            print(f"{kind:9s}{name:{width + 2}s}{got:22s}{want:26s}{verdict}")
        reds = sum(1 for r in rows if r[4] == "RED")
        print(f"\n{reds}/{len(rows)} cases RED against the real binary "
              f"(P1/M2 fail-closed barrier) — the P2 contract is observed "
              f"red before any implementation code.")
    finally:
        server.send_signal(signal.SIGTERM)
        server.wait(timeout=5)


if __name__ == "__main__":
    main()

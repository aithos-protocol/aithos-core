#!/usr/bin/env python3
"""Wire replay for the P7 cases (gate contrat P2) against the REAL
aithos-store-api (child process, real socket).

Gate-contrat role (2026-07-20): observed 15/15 RED against the P1/M2
skeleton — the proof the vectors were red before any implementation code.
Étape 4 evolves it into the NON-REGRESSION driver: every case now runs
against a FRESH server seeded with the case's own frozen state
(`state_heads` → the A.5 heads-table bootstrap seed, `state_objects` →
stored objects), because the p7 cases are per-state contracts, not a
sequential session. 15/15 GREEN = the étape-4 gate.

Fixture rules (unchanged): envelopes are signed HERE with the committed
keys (the vectors_replay rule — the harness re-signs the signed half; the
vectors are the CAS-layer contract). The bootstrap binds the DIDs and
preloads the p1 mandate cert — the control-plane enrollment that A.2 #1/#7
presuppose (« l'enrôlement P7 précède toujours »). The cb2 subject's
did.json is synthesized from the COMMITTED cb2 seeds (did:key-style: the
DID literal IS the root key — no other key exists in the fixture).

Envelope instants: manifest cases sign at the manifest's own
`edition.created_at` (the delegated chain window is a frozen fact of the
cb2 vector); cert and gamma cases at the gate-contrat instant.

Usage: python3 red-replay-p7.py <path-to-aithos-store-api>
"""

import importlib.util
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

import nacl.bindings  # noqa: E402
import nacl.signing  # noqa: E402

HOST = "store.aithos.fr"
PORT = 18080
LISTEN = f"127.0.0.1:{PORT}"
AT = "2026-07-19T12:00:00Z"


def envelope_header(method, path, body, key, sk, mandate, nonce, at):
    env = gen_p.envelope(method, path, body, at, nonce, mandate, key, sk)
    return gen_p.header_of(env)


def fire(method, path, body, header, if_head, at):
    req = urllib.request.Request(
        f"http://{LISTEN}{path}", data=body if body else None, method=method)
    req.add_header("Host", HOST)
    req.add_header("X-Aithos-Test-Now", at)
    if header:
        req.add_header("X-Aithos-Auth", header)
    if if_head is not None:
        req.add_header("If-Head", if_head)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return resp.status, resp.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def synth_cb2_did_json(cb2):
    """The cb2 subject's did.json, from the committed seeds only (the
    fixture's rule: no other key exists). did:key-style — the literal IS
    the root; kex/succession are deterministic fixture keys."""
    seeds = cb2["deterministic_private_seed_hex"]
    root_sk = nacl.signing.SigningKey(bytes.fromhex(seeds["root"]))
    content_sk = nacl.signing.SigningKey(bytes.fromhex(seeds["content"]))
    kex_pub = nacl.bindings.crypto_scalarmult_base(
        gen_p.derive("aithos-core/v1/owner-kex", bytes.fromhex(seeds["root"])))
    succ_sk = nacl.signing.SigningKey(b"\xbb" * 32)
    did = cb2["context"]["subject"]
    doc = {
        "aithos-did-core": "1.0.0-draft.1",
        "id": did,
        "keys": {"root": gen_p.mb_ed(root_sk.verify_key.encode()),
                 "content": gen_p.mb_ed(content_sk.verify_key.encode()),
                 "kex": gen_p.mb_x(kex_pub),
                 "succession": gen_p.mb_ed(succ_sk.verify_key.encode())},
        "revocations": "gamma/gamma.jsonl",
        "bundle": [f"https://{HOST}/t/acme/{did}"],
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return gen_p.jcs(gen_p.sign_doc(doc, root_sk))


def case_bootstrap(p7, cb2_did_json, case, kind):
    """One case = one frozen state = one bootstrap: the tenant binding,
    the p1 did.json + mandate cert (enrollment fixtures), the case's
    `state_objects`, and the A.5 heads seed from `state_heads`."""
    p1 = json.load(open("p1-store-envelope.json"))
    state = case.get("state_heads")
    heads = None
    if state is not None:
        heads = {}
        if "height" in state:
            heads["height"] = state["height"]
            heads["manifest"] = state["manifest"]
        if "gamma" in state:
            heads["gamma"] = state["gamma"]
    p1_objects = [{"key": f"certs/{gen_p.MANDATE_ID}.json",
                   "utf8": p1["mandate_jcs"]}]
    subject = case.get("subject_did")
    dids = []
    p1_did = {"did": p7["did"], "did_json": p1["did_json_jcs"],
              "objects": p1_objects}
    if subject is None:
        for key, utf8 in case.get("state_objects", {}).items():
            p1_objects.append({"key": key, "utf8": utf8})
        if heads is not None:
            p1_did["heads"] = heads
        dids.append(p1_did)
    else:
        dids.append(p1_did)
        cb2_did = {"did": subject, "did_json": cb2_did_json,
                   "objects": [{"key": key, "utf8": utf8}
                               for key, utf8 in case["state_objects"].items()]}
        if heads is not None:
            cb2_did["heads"] = heads
        dids.append(cb2_did)
    return {"tenants": [{"tenant": p7["tenant"], "dids": dids}]}


def spawn(binary, bootstrap_path):
    env = dict(os.environ)
    env.update({
        "AITHOS_STORE_LISTEN": LISTEN,
        "AITHOS_STORE_AUTHORITY": HOST,
        "AITHOS_STORE_BOOTSTRAP": bootstrap_path,
        "AITHOS_STORE_NONCE_BACKEND": "memory",
        "AITHOS_STORE_DNS_BACKEND": "off",
        "AITHOS_STORE_TEST_NOW": "1",
    })
    server = subprocess.Popen([binary], env=env, stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL)
    for _ in range(100):
        try:
            socket.create_connection(("127.0.0.1", PORT), 0.2).close()
            return server
        except OSError:
            time.sleep(0.1)
    server.send_signal(signal.SIGTERM)
    raise RuntimeError("aithos-store-api never started listening")


def main():
    binary = sys.argv[1]
    p7 = json.load(open("p7-store-publication.json"))
    cb2 = json.load(open("cb2-draft2-carriers.json"))
    cb2_did_json = synth_cb2_did_json(cb2)
    did = p7["did"]
    base = f"/t/{p7['tenant']}/{did}"
    cb2_sk = nacl.signing.SigningKey(
        bytes.fromhex(cb2["deterministic_private_seed_hex"]["grantee"]))

    rows, nonce_n = [], 0

    def run_case(kind, case, method, path, body_jcs, signer, at):
        nonlocal nonce_n
        nonce_n += 1
        body = body_jcs.encode()
        nonce = f"p7-replay-{nonce_n:04d}"
        if signer == "owner_root":
            header = envelope_header(
                method, path, body, "#root", gen_p.root_sk, [], nonce, at)
        elif signer == "grantee":
            header = envelope_header(
                method, path, body, gen_p.mb_ed(gen_p.AGENT_PUB),
                gen_p.agent_sk, [gen_p.MANDATE_ID], nonce, at)
        else:  # cb2_grantee
            header = envelope_header(
                method, path, body,
                cb2["context"]["grantee_key"], cb2_sk,
                case["mandate_chain"], nonce, at)
        with tempfile.NamedTemporaryFile("w", suffix=".json") as f:
            json.dump(case_bootstrap(p7, cb2_did_json, case, kind), f)
            f.flush()
            server = spawn(binary, f.name)
            try:
                status, resp = fire(method, path, body, header,
                                    case.get("if_head"), at)
            finally:
                server.send_signal(signal.SIGTERM)
                server.wait(timeout=5)
        try:
            payload = json.loads(resp)
        except Exception:
            payload = {}
        code = payload.get("error", "") if isinstance(payload, dict) else ""
        want = case["expect"]
        if want["status"] == "accept":
            red = status >= 300
            if not red and "new_head" in want and status != 204:
                red = payload.get("head") != want["new_head"]
            if not red and "new_height" in want and status != 204:
                red = payload.get("height") != want["new_height"]
        else:
            red = (status != want["status"]) or (want.get("error", "") != code)
            if not red and "head" in want:
                red = payload.get("head") != want["head"]
            if not red and "height" in want:
                red = payload.get("height") != want["height"]
            if not red and "reason" in want:
                red = payload.get("reason") != want["reason"]
        rows.append((kind, case["name"], f"{status} {code}".strip(),
                     f"{want['status']} {want.get('error', '')}".strip(),
                     "RED" if red else "GREEN"))

    for case in p7["manifest_cases"]:
        path = (f"/t/{p7['tenant']}/{case['subject_did']}/manifest.json"
                if "subject_did" in case else f"{base}/manifest.json")
        at = json.loads(case["body_jcs"])["edition"]["created_at"]
        run_case("manifest", case, "PUT", path, case["body_jcs"],
                 case["signer"], at)
    for case in p7["cert_cases"]:
        run_case("cert", case, "PUT", f"{base}/{case['path']}",
                 case["body_jcs"], case["signer"], AT)
    for case in p7["gamma_cases"]:
        run_case("gamma", case, "POST", f"{base}/gamma",
                 case["entry_jcs"], case["signer"], AT)

    width = max(len(r[1]) for r in rows)
    print(f"{'layer':9s}{'case':{width + 2}s}{'binary answered':22s}"
          f"{'vector expects':26s}verdict")
    for kind, name, got, want, verdict in rows:
        print(f"{kind:9s}{name:{width + 2}s}{got:22s}{want:26s}{verdict}")
    reds = sum(1 for r in rows if r[4] == "RED")
    greens = len(rows) - reds
    if reds:
        print(f"\n{reds}/{len(rows)} cases RED against the real binary.")
        sys.exit(1)
    print(f"\n{greens}/{len(rows)} cases GREEN against the real binary "
          f"(A.4/A.5 wire replay, per-case frozen state) — the étape-4 "
          f"non-regression gate.")


if __name__ == "__main__":
    main()

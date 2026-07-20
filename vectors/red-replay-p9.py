#!/usr/bin/env python3
"""Wire replay for the P9 cases (gate contrat étape 5) against the REAL
aithos-store-api (child process, real socket) — the per-case pattern
blessed at gate 4 (⑦): every case runs against a FRESH server seeded
with the case's own frozen state.

Gate-contrat role (2026-07-20): observed RED against the gate-4 binary —
the proof the read-surface vector is red before any étape-5 code. The
étape-5 gate turns it 33/33 GREEN and it becomes the non-regression
driver for the read surface.

Fixture rules (the p7 driver's, unchanged): envelopes are signed HERE
with the committed keys; the bootstrap binds the DIDs and preloads the
p1 did.json + mandate cert (the enrollment A.2 #1/#7 presupposes). The
genesis case binds its DID with NO stored document — the A.4 genesis
exception under the deposited root key.

Usage: python3 red-replay-p9.py <path-to-aithos-store-api>
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

import nacl.signing  # noqa: E402

HOST = "store.aithos.fr"
PORT = 18090
LISTEN = f"127.0.0.1:{PORT}"


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
            return resp.status, dict(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()


def parse_multipart(headers, body: bytes):
    """Byte-exact multipart split: the delimiter is `\r\n--boundary`, so a
    part body keeps every one of its own trailing newlines."""
    ctype = next((v for k, v in headers.items()
                  if k.lower() == "content-type"), "")
    if "multipart/mixed" not in ctype or "boundary=" not in ctype:
        return None
    delim = b"--" + ctype.split("boundary=", 1)[1].strip().strip('"').encode()
    if not body.startswith(delim + b"\r\n"):
        return None
    parts = []
    for chunk in body[len(delim) + 2:].split(b"\r\n" + delim):
        if chunk == b"--\r\n" or chunk == b"--":
            break
        if chunk.startswith(b"\r\n"):
            chunk = chunk[2:]
        if b"\r\n\r\n" in chunk:
            raw_headers, part_body = chunk.split(b"\r\n\r\n", 1)
        else:
            raw_headers, part_body = chunk, b""
        part = {"body": part_body}
        for line in raw_headers.decode(errors="replace").splitlines():
            if ":" in line:
                name, value = line.split(":", 1)
                part[name.strip().lower()] = value.strip()
        parts.append(part)
    return parts


def bootstrap_of(vector, p1, case):
    state = case["state"]
    objects = {}
    if state.get("use_base_objects"):
        bundle = json.load(open(vector["bundle_packages"]))
        base = bundle["packages"][vector["base_package"]]
        objects.update({k: o["utf8"] for k, o in base["objects"].items()})
    objects.update(state.get("extra_objects", {}))
    for key in state.get("drop_objects", []):
        objects.pop(key, None)
    objects.setdefault("did.json", p1["did_json_jcs"])
    entries = [{"key": f"certs/{gen_p.MANDATE_ID}.json",
                "utf8": p1["mandate_jcs"]}]
    entries += [{"key": k, "utf8": u} for k, u in sorted(objects.items())
                if k != "did.json"]
    p1_did = {"did": vector["did"], "did_json": objects["did.json"],
              "objects": entries}
    if state.get("heads") is not None:
        p1_did["heads"] = state["heads"]
    dids = [p1_did]
    if state.get("bind_did"):
        dids.append({"did": state["bind_did"], "objects": []})
    return {"tenants": [{"tenant": vector["tenant"], "dids": dids}]}


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
        if server.poll() is not None:
            return None
        try:
            socket.create_connection(("127.0.0.1", PORT), 0.2).close()
            return server
        except OSError:
            time.sleep(0.1)
    server.send_signal(signal.SIGTERM)
    return None


def main():
    binary = sys.argv[1]
    vector = json.load(open("p9-store-reads.json"))
    p1 = json.load(open("p1-store-envelope.json"))
    at = vector["at"]
    genesis = vector["fixtures"]["genesis"]
    genesis_sk = nacl.signing.SigningKey(
        bytes.fromhex(genesis["root_seed_hex"]))

    def signer_of(name):
        if name == "owner_root":
            return "#root", gen_p.root_sk, []
        if name == "grantee":
            return (gen_p.mb_ed(gen_p.AGENT_PUB), gen_p.agent_sk,
                    [gen_p.MANDATE_ID])
        if name == "genesis_owner":
            return "#root", genesis_sk, []
        if name == "genesis_foreign_doc":
            # deposits the FOREIGN p1 document; the genesis exception
            # resolves #root against it, so the p1 root signs
            return "#root", gen_p.root_sk, []
        if name == "genesis_wrong_signer":
            return "#root", gen_p.agent_sk, []
        raise KeyError(name)

    rows, nonce_n = [], 0

    def base_of(case):
        did = (case["state"].get("bind_did")
               if case["group"] == "did" and case["state"].get("bind_did")
               else vector["did"])
        return f"/t/{vector['tenant']}/{did}"

    def path_of(case, step):
        base = base_of(case)
        rel = step["path_rel"]
        if rel == "":
            return base + step.get("query", "")
        return f"{base}/{rel}" + step.get("query", "")

    def run_step(case, step):
        nonlocal nonce_n
        nonce_n += 1
        path = path_of(case, step)
        body = step.get("body_utf8", "").encode()
        want = step["expect"]
        header = None
        if step["signer"] != "anonymous":
            key, sk, mandate = signer_of(step["signer"])
            env = gen_p.envelope(step["method"], path, body, at,
                                 f"p9-replay-{nonce_n:04d}", mandate,
                                 key, sk)
            header = gen_p.header_of(env)
        with tempfile.NamedTemporaryFile("w", suffix=".json") as f:
            json.dump(bootstrap_of(vector, p1, case), f)
            f.flush()
            server = spawn(binary, f.name)
            if server is None:
                return "no-server", "server died on this state", True
            try:
                status, headers, resp = fire(
                    step["method"], path, body, header,
                    step.get("if_head"), at)
            finally:
                server.send_signal(signal.SIGTERM)
                server.wait(timeout=5)
        payload = {}
        try:
            payload = json.loads(resp)
        except Exception:
            pass
        code = payload.get("error", "") if isinstance(payload, dict) else ""
        got = f"{status} {code}".strip()
        if want["status"] == "accept":
            if status != want["code"]:
                return got, "status", True
            if "json" in want:
                if payload != want["json"]:
                    return got, "json body drift", True
            if "body_utf8" in want and resp.decode(errors="replace") \
                    != want["body_utf8"]:
                return got, "body drift", True
            if "parts" in want:
                parts = parse_multipart(headers, resp)
                if parts is None:
                    return got, "not multipart", True
                if len(parts) != len(want["parts"]):
                    return got, f"{len(parts)} parts", True
                for got_p, want_p in zip(parts, want["parts"]):
                    loc = got_p.get("content-location", "")
                    st = got_p.get("x-aithos-status", "")
                    if not loc.endswith(want_p["path"]):
                        return got, f"part path {loc}", True
                    if st != str(want_p["part_status"]):
                        return got, f"part status {st}", True
                    if want_p["part_status"] == 200:
                        if "body_utf8" in want_p and got_p["body"].decode(
                                errors="replace") != want_p["body_utf8"]:
                            return got, "part body drift", True
                    elif got_p["body"]:
                        return got, "body on a non-200 part", True
            return got, "", False
        red = (status != want["status"]) or (want.get("error", "") != code)
        if not red and "reason" in want:
            red = payload.get("reason") != want["reason"]
        if not red and "head" in want:
            red = payload.get("head") != want["head"]
        return got, "", red

    for case in vector["cases"]:
        for i, step in enumerate(case["steps"]):
            got, why, red = run_step(case, step)
            want = step["expect"]
            wanted = (f"{want.get('code', want['status'])} "
                      f"{want.get('error', '')}".strip()
                      if want["status"] == "accept" or True else "")
            label = case["name"] if len(case["steps"]) == 1 \
                else f"{case['name']}#{i + 1}"
            rows.append((case["group"], label, got + (
                f" ({why})" if why else ""), wanted,
                "RED" if red else "GREEN"))

    width = max(len(r[1]) for r in rows)
    print(f"{'group':9s}{'case':{width + 2}s}{'binary answered':30s}"
          f"{'vector expects':18s}verdict")
    for group, name, got, want, verdict in rows:
        print(f"{group:9s}{name:{width + 2}s}{got:30s}{want:18s}{verdict}")
    reds = sum(1 for r in rows if r[4] == "RED")
    greens = len(rows) - reds
    if reds:
        print(f"\n{reds}/{len(rows)} steps RED against the real binary "
              f"(gate contrat étape 5: red first is the proof).")
        sys.exit(1)
    print(f"\n{greens}/{len(rows)} steps GREEN against the real binary "
          f"(A.1 redline + A.3 read surface + A.4/A.5 remaining writes, "
          f"per-case frozen state) — the étape-5 non-regression gate.")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Bench P4 — les gates de performance §3.6 d'INFRA-PROVIDER, mesurés
contre la PROD réelle. OFFICIEL depuis la machine de Mathieu (arbitrage
③ 2026-07-21 — motif des suites déployées P7b) ; toute mesure sandbox
est une PRÉ-MESURE indicative.

Cibles (§3.6) :
  - append d'acte mode B (POST /gamma), depuis l'Europe : p50 < 120 ms
  - sync à froid, 1 000 sections (POST /sync, un aller-retour) : < 2 s
  - GET objet immuable (CloudFront hit) : p50 < 30 ms
  (la 4e cible — navigation cache local p50 < 5 ms — est côté CLIENT,
   indépendante du réseau : `cargo test -p aithos-provider --test
   remote_cache_nav -- --nocapture`)

Préparation (opérateur, une fois) :
  export AITHOS_ADMIN_CONTROL_TABLE=aithos-provider-prod-control
  export AITHOS_ADMIN_OBJECTS_BUCKET=aithos-provider-prod-store-data
  export AITHOS_ADMIN_HEADS_TABLE=aithos-provider-prod-heads
  aithos-store-admin create bench-<date>
  # puis lancer CE script une première fois avec --print-did-only,
  # lier le DID imprimé :
  aithos-store-admin bind-did bench-<date> <did>

À la fin : purge (aithos-store-admin purge bench-<date> --yes). Le DID
est FRAIS (D8) : ses hauteurs publiées ici entrent au feed du témoin et
y restent (append-only C.3) — le DID est jetable, c'est le design.

Usage:
  AITHOS_BENCH_SEED=<hex64> python3 bench-p4.py https://store.aithos.fr bench-20260721 \
      [--sections 1000] [--appends 30] [--cdn https://public.aithos.fr] \
      [--witness https://witness.aithos.fr] [--print-did-only]
"""

import concurrent.futures
import datetime as dt
import hashlib
import importlib.util
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request

FRESH_SEED = os.environ.get("AITHOS_BENCH_SEED") or hashlib.sha256(
    b"aithos-p4-bench" + os.urandom(32)).hexdigest()
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "gen-p.py")).read()
OLD = '"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"'
assert OLD in src, "gen-p SEED literal moved"
spec = importlib.util.spec_from_loader("gen_p", loader=None)
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
exec(compile(src.replace(OLD, '"%s"' % FRESH_SEED), "gen-p.py(bench)", "exec"),
     gen_p.__dict__)

args = [a for a in sys.argv[1:] if not a.startswith("--")]
BASE_URL = args[0] if args else "https://store.aithos.fr"
TENANT = args[1] if len(args) > 1 else "bench-20260721"


def opt(name, default):
    flag = f"--{name}"
    for i, a in enumerate(sys.argv):
        if a == flag and i + 1 < len(sys.argv):
            return sys.argv[i + 1]
        if a.startswith(flag + "="):
            return a.split("=", 1)[1]
    return default


SECTIONS = int(opt("sections", "1000"))
APPENDS = int(opt("appends", "30"))
CDN = opt("cdn", "https://public.aithos.fr")
WITNESS = opt("witness", "https://witness.aithos.fr")
DID = gen_p.DID
BASE = f"/t/{TENANT}/{DID}"

print(f"bench identity — seed {FRESH_SEED}")
print(f"did {DID}")
if "--print-did-only" in sys.argv:
    sys.exit(0)

nonce_n = 0


def now_iso():
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


# Connexions PERSISTANTES (keep-alive), une par (hôte, thread) — le
# client réel (ureq) réutilise ses connexions : mesurer un handshake TLS
# par requête fausserait chaque gate.
import http.client
import threading
import urllib.parse

_conns = threading.local()


def _conn(base):
    u = urllib.parse.urlparse(base)
    pool = getattr(_conns, "pool", None)
    if pool is None:
        pool = _conns.pool = {}
    conn = pool.get(u.netloc)
    if conn is None:
        conn = (http.client.HTTPSConnection if u.scheme == "https"
                else http.client.HTTPConnection)(u.netloc, timeout=30)
        pool[u.netloc] = conn
    return conn


def fire(method, url_path, body, header=None, if_head=None, base=None):
    base = base or BASE_URL
    headers = {}
    if header:
        headers["X-Aithos-Auth"] = header
    if if_head is not None:
        headers["If-Head"] = if_head
    for attempt in (0, 1):
        conn = _conn(base)
        try:
            conn.request(method, url_path, body=body, headers=headers)
            resp = conn.getresponse()
            data = resp.read()
            return resp.status, data, dict(resp.headers)
        except (http.client.HTTPException, OSError):
            # stale keep-alive: drop the connection, retry once fresh
            try:
                conn.close()
            finally:
                getattr(_conns, "pool", {}).pop(
                    urllib.parse.urlparse(base).netloc, None)
            if attempt:
                raise
    raise RuntimeError("unreachable")


def signed(method, path, body):
    global nonce_n
    nonce_n += 1
    nonce = f"p4-bench-{os.urandom(6).hex()}-{nonce_n:05d}"
    env = gen_p.envelope(method, path, body, now_iso(), nonce, [], "#root",
                         gen_p.root_sk)
    return gen_p.header_of(env)


def must(step, st, want=(200, 201, 204)):
    if st not in want:
        sys.exit(f"FATAL {step}: status {st}")


def p50(ms):
    return statistics.median(ms)


def manifest(height, prev_hash, files, gamma_head=""):
    doc = {
        "aithos-core": "1.0.0-draft.1",
        "edition": {"created_at": now_iso(), "height": height,
                    "prev_hash": prev_hash},
        "files": files,
        "gamma_head": gamma_head,
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return gen_p.sign_doc(doc, gen_p.root_sk)


results = {}

# --- setup : genesis + édition 1 ---------------------------------------
gen_p.DID_PATH_TENANT = TENANT
did_body = gen_p.jcs(gen_p.build_did_doc()).encode()
st, _, _ = fire("PUT", f"{BASE}/did.json", did_body,
                signed("PUT", f"{BASE}/did.json", did_body))
must("genesis did.json", st, (200, 201, 204))
did_hash = hashlib.sha256(did_body).hexdigest()
m1 = manifest(1, "", {"did.json": did_hash})
m1_jcs = gen_p.jcs(m1).encode()
m1_head = gen_p.manifest_chain_hash(m1)
st, rb, _ = fire("PUT", f"{BASE}/manifest.json", m1_jcs,
                 signed("PUT", f"{BASE}/manifest.json", m1_jcs),
                 if_head="none")
must("publish edition 1", st, (200,))

# --- gate : append mode B p50 (POST /gamma, séquentiel, chaîné) --------
prev = ""
times = []
for i in range(APPENDS):
    entry = gen_p.sign_doc({
        "v": 1, "id": "gamma_" + f"{i:026d}"[:26], "prev": prev,
        "at": now_iso(), "kind": "heartbeat", "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
        gen_p.content_sk)
    body = gen_p.jcs(entry).encode()
    t0 = time.perf_counter()
    st, rb, _ = fire("POST", f"{BASE}/gamma", body,
                     signed("POST", f"{BASE}/gamma", body),
                     if_head=prev or "none")
    times.append((time.perf_counter() - t0) * 1000)
    must(f"gamma append {i}", st, (200,))
    prev = json.loads(rb)["head"]
results["append_mode_b_p50_ms"] = round(p50(times), 1)
results["append_mode_b_p95_ms"] = round(sorted(times)[int(len(times) * 0.95) - 1], 1)

# --- setup : N sections publiques (parallèle, non mesuré) --------------
ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"


def sid_of(i):
    tail = ""
    n = i
    for _ in range(6):
        tail = ALPHABET[n % 32] + tail
        n //= 32
    return "01" + "0" * 18 + tail


def deposit(i):
    sid = sid_of(i)
    path = f"{BASE}/public/sections/{sid}.md"
    body = f"# section {i}\n\nbench p4 — contenu {i}.\n".encode()
    # setup only (unmeasured): a transient reset under parallel load is
    # retried — the timed gates stay sequential and unretried.
    for attempt in range(4):
        try:
            st, _, _ = fire("PUT", path, body, signed("PUT", path, body))
        except urllib.error.URLError:
            time.sleep(0.5 * (attempt + 1))
            continue
        if st in (200, 201, 204):
            return sid, hashlib.sha256(body).hexdigest()
        if st >= 500:
            time.sleep(0.5 * (attempt + 1))
            continue
        raise RuntimeError(f"deposit {sid}: {st}")
    raise RuntimeError(f"deposit {sid}: retries exhausted")


t0 = time.perf_counter()
with concurrent.futures.ThreadPoolExecutor(max_workers=12) as pool:
    deposited = list(pool.map(deposit, range(SECTIONS)))
setup_s = time.perf_counter() - t0
files2 = {"did.json": did_hash}
for sid, digest in deposited:
    files2[f"public/sections/{sid}.md"] = digest
m2 = manifest(2, m1_head, files2)
m2_jcs = gen_p.jcs(m2).encode()
st, rb, _ = fire("PUT", f"{BASE}/manifest.json", m2_jcs,
                 signed("PUT", f"{BASE}/manifest.json", m2_jcs),
                 if_head="sha256:" + m1_head)
must("publish edition 2", st, (200,))
print(f"setup: {SECTIONS} sections déposées en {setup_s:.1f}s (non mesuré)")

# --- gate : sync à froid (UN aller-retour, tout le pack) ---------------
sync_body = json.dumps({"have_edition": 1}).encode()
t0 = time.perf_counter()
st, rb, _ = fire("POST", f"{BASE}/sync", sync_body,
                 signed("POST", f"{BASE}/sync", sync_body))
sync_s = time.perf_counter() - t0
must("sync", st, (200,))
parts = rb.count(b"X-Aithos-Status:")
results["sync_cold_s"] = round(sync_s, 2)
results["sync_cold_parts"] = parts
results["sync_cold_bytes"] = len(rb)

# --- gate : GET immuable via CloudFront (witness root, hit edge) -------
root_path = "/roots/2026-07-20.json"
fire("GET", root_path, None, base=WITNESS)  # warm the edge
times = []
for _ in range(20):
    t0 = time.perf_counter()
    st, _, hd = fire("GET", root_path, None, base=WITNESS)
    times.append((time.perf_counter() - t0) * 1000)
    must("witness root", st, (200,))
results["cdn_immutable_p50_ms"] = round(p50(times), 1)

# --- indicatif : GET section publique via public.<env> (edge) ----------
sec_path = f"{BASE}/public/sections/{deposited[0][0]}.md"
fire("GET", sec_path, None, base=CDN)  # warm
times = []
for _ in range(20):
    t0 = time.perf_counter()
    st, _, _ = fire("GET", sec_path, None, base=CDN)
    times.append((time.perf_counter() - t0) * 1000)
    must("cdn section", st, (200,))
results["cdn_public_section_p50_ms"] = round(p50(times), 1)

# ------------------------------------------------------------- verdict
print()
gates = [
    ("append mode B p50", results["append_mode_b_p50_ms"], 120.0, "ms"),
    ("sync à froid 1000 sections", results["sync_cold_s"], 2.0, "s"),
    ("GET immuable CloudFront p50", results["cdn_immutable_p50_ms"], 30.0, "ms"),
]
reds = 0
for name, got, bound, unit in gates:
    verdict = "GREEN" if got < bound else "RED"
    reds += verdict == "RED"
    print(f"{name:32s} {got:>9} {unit:2s} (cible < {bound} {unit})  {verdict}")
print(f"{'—info— cdn section publique p50':32s} "
      f"{results['cdn_public_section_p50_ms']:>9} ms")
print(f"{'—info— append p95':32s} {results['append_mode_b_p95_ms']:>9} ms")
print(f"{'—info— pack sync':32s} {results['sync_cold_parts']:>9} parts, "
      f"{results['sync_cold_bytes']} octets")
print()
print(json.dumps(results))
sys.exit(1 if reds else 0)

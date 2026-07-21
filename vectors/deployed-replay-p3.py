#!/usr/bin/env python3
"""Gate déployé P3 (DEMO-LEA remote) — rejeu séquentiel contre la PROD
réelle (https://store.aithos.fr), tenant jetable `replay-p3-<date>`,
horloge réelle, **DID FRAIS** (piège D8 : jamais re-publier une hauteur
déjà observée par le témoin sur un DID passé au témoin — les clés du
rejeu sont dérivées d'une graine neuve, imprimée pour le procès-verbal,
jetée avec le tenant).

Preuve déployée des changements SERVICE du jalon P3 + de ce gate :

  base étape 6 (genesis par le wire, cert ⑧b, zone publique, gamma CAS
  A.5, heads, classes A.6) — les MÊMES sémantiques restent servies ;
  + micro-redline A.1 : e/<zone>/header.json et e/<zone>/root.enc
    SERVABLES (classe private-revalidate + ETag fort) ;
  + extension DEMO-LEA : e/x/<id>/header.json + e/x/<id>/manifest.enc
    (les porteurs vault des connecteurs) SERVABLES, même classe ;
  + If-None-Match → 304 sur une classe revalidate ;
  + treillis §04.2 côté couverture : l'APPENDEUR relit le log qu'il
    chaîne (GET gamma servi par la chaîne d'append) ;
  + les clés runner restent HORS grammaire (gateway/**, manifests/tree-*,
    e/x/root.enc → path_invalid).

Usage: python3 deployed-replay-p3.py https://store.aithos.fr replay-p3-20260721
"""

import datetime as dt
import hashlib
import importlib.util
import json
import os
import sys
import urllib.error
import urllib.request

# --- gen-p, re-dérivé sur une graine FRAÎCHE (DID neuf — D8) -----------
FRESH_SEED = os.environ.get("AITHOS_P3_SEED") or hashlib.sha256(
    b"aithos-p3-gate-fresh-did" + os.urandom(32)).hexdigest()
src = open("gen-p.py").read()
OLD = '"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"'
assert OLD in src, "gen-p SEED literal moved"
src = src.replace(OLD, '"%s"' % FRESH_SEED)
spec = importlib.util.spec_from_loader("gen_p", loader=None)
gen_p = importlib.util.module_from_spec(spec)
gen_p.__dict__["__file__"] = os.path.abspath("gen-p.py")
sys.modules["gen_p"] = gen_p
exec(compile(src, "gen-p.py(fresh)", "exec"), gen_p.__dict__)

BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "https://store.aithos.fr"
TENANT = sys.argv[2] if len(sys.argv) > 2 else "replay-p3-20260721"
DID = gen_p.DID
BASE = f"/t/{TENANT}/{DID}"

print(f"fresh replay identity — seed {FRESH_SEED}")
print(f"did {DID}\n")

rows = []
nonce_n = 0


def now_iso():
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def fire(method, path, body, header=None, if_head=None, if_none_match=None):
    req = urllib.request.Request(BASE_URL + path,
                                 data=body if body else None, method=method)
    if header:
        req.add_header("X-Aithos-Auth", header)
    if if_head is not None:
        req.add_header("If-Head", if_head)
    if if_none_match is not None:
        req.add_header("If-None-Match", if_none_match)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return resp.status, resp.read(), resp.headers
    except urllib.error.HTTPError as e:
        return e.code, e.read(), e.headers


def signed(method, path, body, signer, mandate=None):
    global nonce_n
    nonce_n += 1
    nonce = f"p3-gate-{dt.datetime.now(dt.timezone.utc):%H%M%S}-{nonce_n:04d}"
    key, sk = {
        "owner_root": ("#root", gen_p.root_sk),
        "grantee": (gen_p.mb_ed(gen_p.AGENT_PUB), gen_p.agent_sk),
    }[signer]
    env = gen_p.envelope(method, path, body, now_iso(), nonce,
                         mandate or [], key, sk)
    return gen_p.header_of(env)


def check(name, got, want, extra=""):
    ok = got == want
    rows.append((name, str(got)[:58], str(want)[:44], "GREEN" if ok else "RED",
                 extra))
    return ok


def error_of(body):
    try:
        return json.loads(body).get("error", "")
    except Exception:
        return ""


def reason_of(body):
    try:
        return json.loads(body).get("reason", "")
    except Exception:
        return ""


PRIVATE_REVALIDATE = "private, max-age=0, must-revalidate"

# ---------------------------------------------------------------- steps
# base étape 6 — inchangée, DID frais

gen_p.DID_PATH_TENANT = TENANT
did_doc_jcs = gen_p.jcs(gen_p.build_did_doc())
body = did_doc_jcs.encode()
st, rb, _ = fire("PUT", f"{BASE}/did.json", body,
                 signed("PUT", f"{BASE}/did.json", body, "owner_root"))
check("genesis_did_json", st, 204)

st, rb, hd = fire("GET", f"{BASE}/did.json", None)
check("get_did_json_bytes", (st, rb == body), (200, True))
check("did_json_cache_class", hd.get("Cache-Control"),
      "public, max-age=0, must-revalidate")
check("did_json_strong_etag", hd.get("ETag", ""),
      '"%s"' % hashlib.sha256(body).hexdigest())

hello = b"hello, aithos \xe2\x80\x94 gate P3 (journal remote)\n"
p = f"{BASE}/e/public/hello.md"
st, rb, _ = fire("PUT", p, hello, signed("PUT", p, hello, "owner_root"))
check("put_public_hello", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", p, None)
check("get_public_hello", (st, rb == hello), (200, True))
check("public_cache_class", hd.get("Cache-Control"),
      "public, max-age=0, must-revalidate")

cert_body = gen_p.jcs(gen_p.build_mandate()).encode()
cp = f"{BASE}/certs/{gen_p.MANDATE_ID}.json"
st, rb, _ = fire("PUT", cp, cert_body,
                 signed("PUT", cp, cert_body, "owner_root"))
check("deposit_cert", st in (200, 201, 204), True, f"status={st}")
st, rb, _ = fire("PUT", cp, cert_body,
                 signed("PUT", cp, cert_body, "owner_root"))
check("redeposit_cert_identical", st in (200, 201, 204), True,
      f"status={st}")
squat = json.loads(gen_p.jcs(gen_p.build_mandate()))
squat["nonce"] = "p3vectors0002-squat"
squat["signature"]["value"] = ""
squat = gen_p.sign_doc(squat, gen_p.root_sk)
squat_body = gen_p.jcs(squat).encode()
st, rb, _ = fire("PUT", cp, squat_body,
                 signed("PUT", cp, squat_body, "owner_root"))
check("squat_cert_immutable_conflict", (st, error_of(rb), reason_of(rb)),
      (400, "artifact_invalid", "immutable_conflict"))
st, rb, hd = fire("GET", cp, None, signed("GET", cp, b"", "owner_root"))
check("get_cert_bytes", (st, rb == cert_body), (200, True))
check("cert_cache_class", hd.get("Cache-Control"),
      "public, max-age=31536000, immutable")

# gamma sous CAS A.5 réel — DID frais, log vierge
run_tag = f"{dt.datetime.now(dt.timezone.utc):%H%M%S}"
uid = lambda k: "gamma_" + (run_tag + k).rjust(26, "0")
g1 = gen_p.sign_doc({
    "v": 1, "id": uid("1"), "prev": "",
    "at": now_iso(), "kind": "grant", "target": gen_p.MANDATE_ID,
    "payload": {},
    "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
    gen_p.content_sk)
h1 = gen_p.entry_head(gen_p.jcs(g1))
g2 = gen_p.sign_doc({
    "v": 1, "id": uid("2"), "prev": h1,
    "at": now_iso(), "kind": "action", "target": "x.gmail",
    "authorized_by": gen_p.MANDATE_ID, "authorized_via": [gen_p.MANDATE_ID],
    "payload": {"action": "reply", "args_hash": "sha256:" + "ab" * 32},
    "signature": {"alg": "ed25519", "key": gen_p.mb_ed(gen_p.AGENT_PUB),
                  "value": ""}}, gen_p.agent_sk)
h2 = gen_p.entry_head(gen_p.jcs(g2))
g_lost = gen_p.sign_doc({
    "v": 1, "id": uid("3"), "prev": h1,
    "at": now_iso(), "kind": "action", "target": "x.gmail",
    "authorized_by": gen_p.MANDATE_ID, "authorized_via": [gen_p.MANDATE_ID],
    "payload": {"action": "reply", "args_hash": "sha256:" + "cd" * 32},
    "signature": {"alg": "ed25519", "key": gen_p.mb_ed(gen_p.AGENT_PUB),
                  "value": ""}}, gen_p.agent_sk)

gp = f"{BASE}/gamma"
b1 = gen_p.jcs(g1).encode()
st, rb, _ = fire("POST", gp, b1,
                 signed("POST", gp, b1, "grantee", [gen_p.MANDATE_ID]),
                 if_head="none")
check("gamma_append_genesis", st < 300 and json.loads(rb).get("head") == h1,
      True, f"status={st}")
b2 = gen_p.jcs(g2).encode()
st, rb, _ = fire("POST", gp, b2,
                 signed("POST", gp, b2, "grantee", [gen_p.MANDATE_ID]),
                 if_head=h1)
check("gamma_append_cas_ok", st < 300 and json.loads(rb).get("head") == h2,
      True, f"status={st}")
bl = gen_p.jcs(g_lost).encode()
st, rb, _ = fire("POST", gp, bl,
                 signed("POST", gp, bl, "grantee", [gen_p.MANDATE_ID]))
check("gamma_cas_required", (st, error_of(rb)), (428, "cas_required"))
st, rb, _ = fire("POST", gp, bl,
                 signed("POST", gp, bl, "grantee", [gen_p.MANDATE_ID]),
                 if_head=h1)
lost_head = ""
try:
    lost_head = json.loads(rb).get("head", "")
except Exception:
    pass
check("gamma_cas_stale_loser", (st, error_of(rb), lost_head),
      (409, "cas_mismatch", h2))

hp = f"{BASE}/heads"
st, rb, hd = fire("GET", hp, None, signed("GET", hp, b"", "owner_root"))
heads_gamma = ""
try:
    heads_gamma = json.loads(rb).get("gamma", "")
except Exception:
    pass
check("heads_tuple_gamma", (st, heads_gamma), (200, h2))
check("heads_no_store", hd.get("Cache-Control"), "no-store")

# ------------------------------------------------- P3 : le delta SERVICE

# treillis §04.2 : l'APPENDEUR relit le log qu'il chaîne — la chaîne
# d'append (act.x.gmail.reply dans le mandat p1) est servie en GET gamma.
sp = f"{BASE}/gamma/{dt.datetime.now(dt.timezone.utc):%Y-%m}.jsonl"
st, rb, hd = fire("GET", sp, None,
                  signed("GET", sp, b"", "grantee", [gen_p.MANDATE_ID]))
seg_bytes = rb if st == 200 else b""
check("lattice_appender_reads_gamma",
      (st, gen_p.jcs(g1).encode() in seg_bytes,
       gen_p.jcs(g2).encode() in seg_bytes),
      (200, True, True),
      "la ligne read.gamma reste la voie des tiers ; l'appendeur relit")
check("gamma_current_segment_no_store", hd.get("Cache-Control"), "no-store")

# micro-redline A.1 : les porteurs racine de zone sont servables.
zh = f"{BASE}/e/circle/header.json"
zh_body = json.dumps({"v": 1, "lines": []}, separators=(",", ":")).encode()
st, rb, _ = fire("PUT", zh, zh_body, signed("PUT", zh, zh_body, "owner_root"))
check("put_zone_header", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", zh, None, signed("GET", zh, b"", "owner_root"))
etag_zh = hd.get("ETag", "")
check("get_zone_header", (st, rb == zh_body), (200, True))
check("zone_header_cache_class", hd.get("Cache-Control"), PRIVATE_REVALIDATE)
check("zone_header_strong_etag", etag_zh,
      '"%s"' % hashlib.sha256(zh_body).hexdigest())

zr = f"{BASE}/e/circle/root.enc"
zr_body = b"\x00p3-root-opaque-ciphertext"
st, rb, _ = fire("PUT", zr, zr_body, signed("PUT", zr, zr_body, "owner_root"))
check("put_zone_root", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", zr, None, signed("GET", zr, b"", "owner_root"))
check("get_zone_root", (st, rb == zr_body, hd.get("Cache-Control")),
      (200, True, PRIVATE_REVALIDATE))

# If-None-Match → 304 sur la classe revalidate (A.6, précision gravée).
st, rb, hd = fire("GET", zh, None, signed("GET", zh, b"", "owner_root"),
                  if_none_match=etag_zh)
check("zone_header_304", (st, rb, hd.get("Cache-Control")),
      (304, b"", PRIVATE_REVALIDATE))

# extension DEMO-LEA : les porteurs vault des connecteurs.
xh = f"{BASE}/e/x/gmail/header.json"
xh_body = json.dumps({"v": 1, "lines": []}, separators=(",", ":")).encode()
st, rb, _ = fire("PUT", xh, xh_body, signed("PUT", xh, xh_body, "owner_root"))
check("put_connector_header", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", xh, None, signed("GET", xh, b"", "owner_root"))
check("get_connector_header", (st, rb == xh_body, hd.get("Cache-Control")),
      (200, True, PRIVATE_REVALIDATE))
check("connector_header_strong_etag", hd.get("ETag", ""),
      '"%s"' % hashlib.sha256(xh_body).hexdigest())

xc = f"{BASE}/e/x/gmail/manifest.enc"
xc_body = b"\x00p3-connector-config-opaque"
st, rb, _ = fire("PUT", xc, xc_body, signed("PUT", xc, xc_body, "owner_root"))
check("put_connector_config", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", xc, None, signed("GET", xc, b"", "owner_root"))
check("get_connector_config", (st, rb == xc_body, hd.get("Cache-Control")),
      (200, True, PRIVATE_REVALIDATE))

# les clés runner restent HORS grammaire — path_invalid, owner compris.
for name, path in [
    ("runner_state_outside_wire", f"{BASE}/gateway/state.json"),
    ("derived_tree_outside_wire", f"{BASE}/manifests/tree-2.json"),
    ("x_root_blob_outside_wire", f"{BASE}/e/x/root.enc"),
]:
    st, rb, _ = fire("GET", path, None,
                     signed("GET", path, b"", "owner_root"))
    check(name, (st, error_of(rb)), (400, "path_invalid"))

# ------------------------------------------------------------- verdict
w = max(len(r[0]) for r in rows) + 2
print(f"{'step':{w}s}{'got':60s}{'want':46s}verdict")
for name, got, want, verdict, extra in rows:
    print(f"{name:{w}s}{got:60s}{want:46s}{verdict}"
          + (f"   ({extra})" if extra and verdict == "RED" else ""))
reds = sum(1 for r in rows if r[3] == "RED")
if reds:
    print(f"\n{reds}/{len(rows)} steps RED against {BASE_URL} "
          f"(tenant {TENANT}).")
    sys.exit(1)
print(f"\n{len(rows)}/{len(rows)} steps GREEN against {BASE_URL} "
      f"(tenant {TENANT}, DID frais, horloge réelle) — preuve du gate "
      f"déployé P3.")

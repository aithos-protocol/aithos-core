#!/usr/bin/env python3
"""Gate déployé étape 6 — rejeu séquentiel contre la PROD réelle
(https://store.aithos.fr), tenant jetable `replay-<date>`, horloge réelle.

Ce driver n'est PAS le rejeu per-state p7/p9 (qui exige l'état gelé et le
test-clock, in-process seulement) : c'est la preuve déployée du gate — les
MÊMES sémantiques rejouées de bout en bout sur les backends durables
(S3 + DynamoDB heads), en construisant l'état PAR LE WIRE :

  genesis did.json → dépôt du cert de mandat (⑧b : idempotent + squat)
  → écriture zone publique → appends gamma sous CAS A.5 (perdant compris)
  → surface de lecture (heads, did.json, e/public) → classes de cache A.6
  réelles (en-têtes HTTP observés, ETag fort compris).

Règle des fixtures inchangée : les enveloppes sont signées ICI avec les
clés committées des vecteurs (publiques par construction) ; le did.json
est re-signé pour le tenant de rejeu (l'URL bundle porte le tenant — la
signature du vecteur ne peut pas être réutilisée telle quelle). Aucun
octet de vecteur gelé n'est touché.

Usage: python3 deployed-replay-etape6.py https://store.aithos.fr replay-20260720
"""

import datetime as dt
import hashlib
import importlib.util
import json
import ssl
import sys
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

BASE_URL = sys.argv[1] if len(sys.argv) > 1 else "https://store.aithos.fr"
TENANT = sys.argv[2] if len(sys.argv) > 2 else "replay-20260720"
DID = gen_p.DID
BASE = f"/t/{TENANT}/{DID}"

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
    nonce = f"etape6-gate-{dt.datetime.now(dt.timezone.utc):%H%M%S}-{nonce_n:04d}"
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


# ---------------------------------------------------------------- steps

# 1 — GENESIS (A.4 : #root se résout contre le document DÉPOSÉ ;
#     A.2 #7 exception genèse). did.json re-signé pour le tenant de rejeu.
gen_p.DID_PATH_TENANT = TENANT
did_doc_jcs = gen_p.jcs(gen_p.build_did_doc())
body = did_doc_jcs.encode()
st0, rb0, _ = fire("GET", f"{BASE}/did.json", None)
if st0 == 200 and rb0 == body:
    check("genesis_did_json", "already-deposited (rejeu)", "already-deposited (rejeu)")
else:
    st, rb, _ = fire("PUT", f"{BASE}/did.json", body,
                     signed("PUT", f"{BASE}/did.json", body, "owner_root"))
    check("genesis_did_json", st, 204)

# 2 — GET did.json anonyme : octets exacts + classe A.6 complément
#     (public, max-age=0, must-revalidate + ETag fort) — gravure au gate.
st, rb, hd = fire("GET", f"{BASE}/did.json", None)
etag_did = hd.get("ETag", "")
check("get_did_json_bytes", (st, rb == body), (200, True))
check("did_json_cache_class", hd.get("Cache-Control"),
      "public, max-age=0, must-revalidate")
want_etag = '"%s"' % hashlib.sha256(body).hexdigest()
check("did_json_strong_etag", etag_did, want_etag)

# 3 — écriture zone publique (owner) puis lecture anonyme : classe
#     public revalidate + ETag fort (A.6).
hello = b"hello, aithos \xe2\x80\x94 gate etape 6 (backends durables)\n"
p = f"{BASE}/e/public/hello.md"
st, rb, _ = fire("PUT", p, hello, signed("PUT", p, hello, "owner_root"))
check("put_public_hello", st in (200, 201, 204), True, f"status={st}")
st, rb, hd = fire("GET", p, None)
check("get_public_hello", (st, rb == hello), (200, True))
check("public_cache_class", hd.get("Cache-Control"),
      "public, max-age=0, must-revalidate")

# 4 — dépôt du cert de mandat committé (A.4) : nom immuable ⑧b.
cert_body = gen_p.jcs(gen_p.build_mandate()).encode()
cp = f"{BASE}/certs/{gen_p.MANDATE_ID}.json"
st, rb, _ = fire("PUT", cp, cert_body,
                 signed("PUT", cp, cert_body, "owner_root"))
check("deposit_cert", st in (200, 201, 204), True, f"status={st}")

# 5 — ⑧b bras idempotent : re-dépôt octets IDENTIQUES → accepté.
st, rb, _ = fire("PUT", cp, cert_body,
                 signed("PUT", cp, cert_body, "owner_root"))
check("redeposit_cert_identical", st in (200, 201, 204), True,
      f"status={st}")

# 6 — ⑧b bras squat : un cert VALIDE différent DÉJÀ nommé pareil →
#     immutable_conflict (reason nouveau A.7, porté au gate).
squat = gen_p.build_mandate()
squat = json.loads(gen_p.jcs(squat))
squat["nonce"] = "p0vectors0002-squat"
squat["signature"]["value"] = ""
squat = gen_p.sign_doc(squat, gen_p.root_sk)
squat_body = gen_p.jcs(squat).encode()
st, rb, _ = fire("PUT", cp, squat_body,
                 signed("PUT", cp, squat_body, "owner_root"))
check("squat_cert_immutable_conflict", (st, error_of(rb), reason_of(rb)),
      (400, "artifact_invalid", "immutable_conflict"))

# 7 — lecture du cert (owner) : classe IMMUTABLE (A.6).
st, rb, hd = fire("GET", cp, None, signed("GET", cp, b"", "owner_root"))
check("get_cert_bytes", (st, rb == cert_body), (200, True))
check("cert_cache_class", hd.get("Cache-Control"),
      "public, max-age=31536000, immutable")

# 8 — gamma sous CAS A.5 réel (DynamoDB heads) : genèse (ou reprise sur
#     la tête courante — le tenant est durable), append, cas_required,
#     perdant (409 + tête courante).
hp0 = f"{BASE}/heads"
st, rb, _ = fire("GET", hp0, None, signed("GET", hp0, b"", "owner_root"))
cur_head = ""
try:
    cur_head = json.loads(rb).get("gamma") or ""
except Exception:
    pass
run_tag = f"{dt.datetime.now(dt.timezone.utc):%H%M%S}"
uid = lambda k: "gamma_" + (run_tag + k).rjust(26, "0")
at1, at2, at3 = now_iso(), now_iso(), now_iso()
g1 = gen_p.sign_doc({
    "v": 1, "id": uid("1"), "prev": cur_head,
    "at": at1, "kind": "grant", "target": gen_p.MANDATE_ID, "payload": {},
    "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
    gen_p.content_sk)
h1 = gen_p.entry_head(gen_p.jcs(g1))
g2 = gen_p.sign_doc({
    "v": 1, "id": uid("2"), "prev": h1,
    "at": at2, "kind": "action", "target": "x.gmail",
    "authorized_by": gen_p.MANDATE_ID, "authorized_via": [gen_p.MANDATE_ID],
    "payload": {"action": "reply", "args_hash": "sha256:" + "ab" * 32},
    "signature": {"alg": "ed25519", "key": gen_p.mb_ed(gen_p.AGENT_PUB),
                  "value": ""}}, gen_p.agent_sk)
h2 = gen_p.entry_head(gen_p.jcs(g2))
g_lost = gen_p.sign_doc({
    "v": 1, "id": uid("3"), "prev": h1,
    "at": at3, "kind": "action", "target": "x.gmail",
    "authorized_by": gen_p.MANDATE_ID, "authorized_via": [gen_p.MANDATE_ID],
    "payload": {"action": "reply", "args_hash": "sha256:" + "cd" * 32},
    "signature": {"alg": "ed25519", "key": gen_p.mb_ed(gen_p.AGENT_PUB),
                  "value": ""}}, gen_p.agent_sk)

gp = f"{BASE}/gamma"
b1 = gen_p.jcs(g1).encode()
st, rb, _ = fire("POST", gp, b1,
                 signed("POST", gp, b1, "grantee", [gen_p.MANDATE_ID]),
                 if_head=cur_head or "none")
ok_genesis = st < 300 and json.loads(rb).get("head") == h1
check("gamma_append_from_current", ok_genesis, True, f"status={st}")

b2 = gen_p.jcs(g2).encode()
st, rb, _ = fire("POST", gp, b2,
                 signed("POST", gp, b2, "grantee", [gen_p.MANDATE_ID]),
                 if_head=h1)
ok_append = st < 300 and json.loads(rb).get("head") == h2
check("gamma_append_cas_ok", ok_append, True, f"status={st}")

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
      (409, "cas_mismatch", h2),
      "le perdant relit la vérité courante (DynamoDB, lecture forte)")

# 9 — heads (owner) : le tuple A.5 + classe no-store.
hp = f"{BASE}/heads"
st, rb, hd = fire("GET", hp, None, signed("GET", hp, b"", "owner_root"))
heads_gamma = ""
try:
    heads_gamma = json.loads(rb).get("gamma", "")
except Exception:
    pass
check("heads_tuple_gamma", (st, heads_gamma), (200, h2))
check("heads_no_store", hd.get("Cache-Control"), "no-store")

# 10 — segment gamma du mois COURANT : no-store (A.6 : il fige au mois
#      révolu seulement).
sp = f"{BASE}/gamma/{dt.datetime.now(dt.timezone.utc):%Y-%m}.jsonl"
st, rb, hd = fire("GET", sp, None, signed("GET", sp, b"", "owner_root"))
check("gamma_current_segment_no_store", (st, hd.get("Cache-Control")),
      (200, "no-store"))

# 11 — surface d'erreur : refus exact du registre A.7. Observation gate :
#      refuse() n'émet AUCUN Cache-Control (le no-store explicite des
#      surfaces d'erreur est un arbitrage à porter au gate — RFC 9110
#      rend un 404 heuristiquement cachable).
mp = f"{BASE}/manifest.json"
st, rb, hd = fire("GET", mp, None, signed("GET", mp, b"", "owner_root"))
check("absent_manifest_404", (st, error_of(rb)), (404, "not_found"),
      f"Cache-Control observé: {hd.get('Cache-Control')!r}")

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
      f"(tenant {TENANT}, horloge réelle, backends durables) — preuve du "
      f"gate déployé étape 6.")

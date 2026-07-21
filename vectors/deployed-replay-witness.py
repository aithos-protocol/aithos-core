#!/usr/bin/env python3
"""Gate déployé P5 (lot A) — la preuve témoin contre la PROD réelle.

Séquence, tenant jetable `replay-w-<date>` (créé/lié par la CLI admin
juste avant, purgé juste après — runbook P7) :

  genesis did.json par le wire → publish d'une édition 1 (manifest
  minimal draft.1, root-signé, JCS canonique, CAS A.5) → le stream heads
  déclenche le témoin (C.2 ①) → poll du feed public
  `https://witness.aithos.fr/<did>.jsonl` jusqu'au checkpoint → le
  checkpoint est VÉRIFIÉ ICI, indépendamment (PyNaCl) : signature sous
  une clé du registre publié `keys.json` (lui-même auto-signé vérifié),
  `manifest_hash` == sha256 recalculé du manifest publié (JCS,
  signature.value=""), `gamma_head` copié du manifest → deuxième publish
  (édition 2) → deuxième checkpoint (la chaîne, pas un doublon C.2) →
  latence publish→checkpoint MESURÉE (gravée au handoff).

Règle des fixtures inchangée : les enveloppes sont signées ICI avec les
clés committées des vecteurs (publiques par construction). Aucun octet
de vecteur gelé n'est touché.

Usage: python3 deployed-replay-witness.py \
    https://store.aithos.fr https://witness.aithos.fr replay-w-20260720
"""

import datetime as dt
import hashlib
import importlib.util
import json
import sys
import time
import urllib.error
import urllib.request

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

import nacl.signing  # noqa: E402
import nacl.encoding  # noqa: E402

STORE_URL = sys.argv[1] if len(sys.argv) > 1 else "https://store.aithos.fr"
WITNESS_URL = sys.argv[2] if len(sys.argv) > 2 else "https://witness.aithos.fr"
TENANT = sys.argv[3] if len(sys.argv) > 3 else "replay-w-20260720"
POLL_SECS = 5
POLL_MAX_SECS = 180

DID = gen_p.DID
BASE = f"/t/{TENANT}/{DID}"

rows = []
nonce_n = 0


def now_iso():
    return dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def fire(base, method, path, body, header=None, if_head=None):
    req = urllib.request.Request(base + path, data=body if body else None,
                                 method=method)
    if header:
        req.add_header("X-Aithos-Auth", header)
    if if_head is not None:
        req.add_header("If-Head", if_head)
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            return resp.status, resp.read(), resp.headers
    except urllib.error.HTTPError as e:
        return e.code, e.read(), e.headers


def signed(method, path, body):
    global nonce_n
    nonce_n += 1
    nonce = f"p5-gate-{dt.datetime.now(dt.timezone.utc):%H%M%S}-{nonce_n:04d}"
    env = gen_p.envelope(method, path, body, now_iso(), nonce, [],
                         "#root", gen_p.root_sk)
    return gen_p.header_of(env)


def check(name, got, want, extra=""):
    ok = got == want
    rows.append((name, str(got)[:58], str(want)[:44],
                 "GREEN" if ok else "RED", extra))
    return ok


def jcs_blank_signature(doc):
    """JCS bytes of `doc` with signature.value blanked — the shared §01.4
    convention, recomputed here independently of the Rust stack."""
    blanked = json.loads(json.dumps(doc))
    blanked["signature"]["value"] = ""
    return gen_p.jcs(blanked).encode()


B58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"


def mb_to_ed(multibase):
    """Decode a base58btc multibase Ed25519 pubkey (z…, 0xed01 prefix) —
    independent of the Rust stack (the verification side of gen-p's
    mb_ed)."""
    assert multibase.startswith("z"), "base58btc multibase expected"
    num = 0
    for ch in multibase[1:]:
        num = num * 58 + B58_ALPHABET.index(ch)
    raw = num.to_bytes((num.bit_length() + 7) // 8, "big")
    pad = len(multibase[1:]) - len(multibase[1:].lstrip("1"))
    raw = b"\x00" * pad + raw
    assert raw[:2] == b"\xed\x01" and len(raw) == 34, "ed25519 multicodec"
    return raw[2:]


def verify_selfsigned(doc, key_multibase):
    """Ed25519 over JCS-with-blank-value under the multibase key."""
    raw = mb_to_ed(key_multibase)
    vk = nacl.signing.VerifyKey(raw)
    try:
        vk.verify(jcs_blank_signature(doc),
                  bytes.fromhex(doc["signature"]["value"]))
        return True
    except Exception:
        return False


def build_manifest(height, prev_hash, gamma_head):
    """Minimal valid draft.1 manifest, root-signed, JCS canonical."""
    doc = {
        "aithos-core": "1.0.0-draft.1",
        "edition": {"height": height, "prev_hash": prev_hash,
                    "created_at": now_iso()},
        "files": {},
        "gamma_head": gamma_head,
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return gen_p.sign_doc(doc, gen_p.root_sk)


# ---------------------------------------------------------------- steps

# 1 — genesis did.json (A.4 : #root contre le document déposé).
gen_p.DID_PATH_TENANT = TENANT
did_doc_jcs = gen_p.jcs(gen_p.build_did_doc())
body = did_doc_jcs.encode()
st0, rb0, _ = fire(STORE_URL, "GET", f"{BASE}/did.json", None)
if st0 == 200 and rb0 == body:
    check("genesis_did_json", "already-deposited", "already-deposited")
else:
    st, rb, _ = fire(STORE_URL, "PUT", f"{BASE}/did.json", body,
                     signed("PUT", f"{BASE}/did.json", body))
    check("genesis_did_json", st, 204)

# 2 — état courant (heads) : le publish enchaîne sur la vérité stockée.
hp = f"{BASE}/heads"
st, rb, _ = fire(STORE_URL, "GET", hp, None, signed("GET", hp, b""))
heads = json.loads(rb)
height0 = heads.get("height", 0)
manifest_head = heads.get("manifest") or "none"
prev_hash = "" if manifest_head == "none" else manifest_head.split(":", 1)[1]
gamma_head = heads.get("gamma") or ""
check("heads_read", st, 200, f"height={height0}")


def publish(height, prev_hash, if_head):
    m = build_manifest(height, prev_hash, gamma_head)
    m_jcs = gen_p.jcs(m)
    m_body = m_jcs.encode()
    mp = f"{BASE}/manifest.json"
    st, rb, _ = fire(STORE_URL, "PUT", mp, m_body,
                     signed("PUT", mp, m_body), if_head=if_head)
    chain_hash = hashlib.sha256(jcs_blank_signature(m)).hexdigest()
    accepted = {}
    try:
        accepted = json.loads(rb)
    except Exception:
        pass
    ok = (st == 200 and accepted.get("height") == height
          and accepted.get("head") == f"sha256:{chain_hash}")
    return ok, st, chain_hash, m


def poll_checkpoint(height, chain_hash):
    """Poll the public feed until the checkpoint appears; verified below."""
    deadline = time.monotonic() + POLL_MAX_SECS
    t0 = time.monotonic()
    while time.monotonic() < deadline:
        st, rb, hd = fire(WITNESS_URL, "GET", f"/{DID}.jsonl", None)
        if st == 200:
            for line in rb.decode().splitlines():
                try:
                    ck = json.loads(line)
                except Exception:
                    continue
                if (ck.get("edition_height") == height
                        and ck.get("manifest_hash") == f"sha256:{chain_hash}"):
                    return ck, time.monotonic() - t0, hd
        time.sleep(POLL_SECS)
    return None, time.monotonic() - t0, None


# 3 — publish édition N+1 (CAS contre la vérité stockée).
h1 = height0 + 1
ok, st, chain1, m1 = publish(h1, prev_hash, manifest_head)
check("publish_edition", ok, True, f"status={st} height={h1}")

# 4 — le checkpoint arrive dans le feed public (déclencheur stream C.2 ①).
ck1, latency1, feed_hd = poll_checkpoint(h1, chain1)
check("checkpoint_in_public_feed", ck1 is not None, True,
      f"latency={latency1:.1f}s (borne {POLL_MAX_SECS}s)")

# 5 — keys.json : le registre publié, auto-signé, vérifié ICI.
st, rb, keys_hd = fire(WITNESS_URL, "GET", "/keys.json", None)
keys_doc = json.loads(rb)
keys_ok = (st == 200
           and keys_doc.get("aithos-witness-keys") == "1.0.0-draft.1"
           and keys_doc.get("witness_key") in keys_doc.get("keys", [])
           and verify_selfsigned(keys_doc, keys_doc["witness_key"]))
check("keys_json_selfsigned", keys_ok, True)

# 6 — le checkpoint vérifie sous une clé DU REGISTRE (C.4, indépendant).
if ck1 is not None:
    in_registry = ck1.get("witness_key") in keys_doc.get("keys", [])
    sig_ok = verify_selfsigned(ck1, ck1["witness_key"])
    check("checkpoint_key_in_registry", in_registry, True)
    check("checkpoint_signature_pynacl", sig_ok, True)
    check("checkpoint_fields",
          (ck1.get("aithos-witness"), ck1.get("did"),
           ck1.get("gamma_head")),
          ("1.0.0-draft.1", DID, m1["gamma_head"]))
else:
    check("checkpoint_key_in_registry", "no checkpoint", True)

# 7 — classes de cache C.3 observées sur le wire public.
if feed_hd is not None:
    check("feed_cache_class", feed_hd.get("Cache-Control"),
          "public, max-age=60")
check("keys_cache_class", keys_hd.get("Cache-Control"),
      "public, max-age=60")

# 8 — deuxième publish : la chaîne continue, un checkpoint PAR édition
#     (C.2 : même journée, hauteurs différentes — jamais dédupliqué).
h2 = h1 + 1
ok, st, chain2, m2 = publish(h2, chain1, f"sha256:{chain1}")
check("publish_second_edition", ok, True, f"status={st} height={h2}")
ck2, latency2, _ = poll_checkpoint(h2, chain2)
check("second_checkpoint", ck2 is not None, True,
      f"latency={latency2:.1f}s")
if ck1 is not None and ck2 is not None:
    check("chain_not_equivocation",
          ck1["edition_height"] != ck2["edition_height"], True,
          "hauteurs différentes = chaîne, jamais un fork (C.4)")

# ------------------------------------------------------------- verdict
w = max(len(r[0]) for r in rows) + 2
print(f"{'step':{w}s}{'got':60s}{'want':46s}verdict")
for name, got, want, verdict, extra in rows:
    print(f"{name:{w}s}{got:60s}{want:46s}{verdict}"
          + (f"   ({extra})" if extra else ""))
reds = sum(1 for r in rows if r[3] == "RED")
if reds:
    print(f"\n{reds}/{len(rows)} steps RED (tenant {TENANT}).")
    sys.exit(1)
print(f"\n{len(rows)}/{len(rows)} steps GREEN — preuve du gate déployé "
      f"P5 (latences: ck1 {latency1:.1f}s, ck2 {latency2:.1f}s).")

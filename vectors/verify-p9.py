#!/usr/bin/env python3
"""P9 independent verification (Python second implementation).

Re-derives every expectation of p9-store-reads.json from the FROZEN
sources (p7-bundle-packages.json, p7-store-publication.json,
p1-store-envelope.json) with local primitives only — no import of
gen-p.py / gen-p9.py. A drift between the vector and this second
implementation fails the run.
"""

import hashlib
import json
import sys

import base58
import nacl.signing


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=False)


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def commitment(domain: str, data: bytes) -> str:
    h = hashlib.sha256()
    h.update(domain.encode())
    h.update(b"\x00")
    h.update(data)
    return "sha256:" + h.hexdigest()


def entry_head(entry_jcs: str) -> str:
    return "sha256:" + sha256_hex(entry_jcs.encode())


def chain_head(manifest: dict) -> str:
    unsigned = json.loads(jcs(manifest))
    unsigned["signature"] = dict(unsigned["signature"], value="")
    return "sha256:" + sha256_hex(jcs(unsigned).encode())


def mb_ed_pub(mb: str) -> bytes:
    assert mb.startswith("z")
    raw = base58.b58decode(mb[1:])
    assert raw[:2] == b"\xed\x01", "not an ed25519 multicodec key"
    return raw[2:]


def doc_verifies(doc_jcs: str, pub: bytes) -> bool:
    doc = json.loads(doc_jcs)
    sig = bytes.fromhex(doc["signature"]["value"])
    unsigned = json.loads(doc_jcs)
    unsigned["signature"] = dict(unsigned["signature"], value="")
    try:
        nacl.signing.VerifyKey(pub).verify(jcs(unsigned).encode(), sig)
        return True
    except Exception:
        return False


def fail(msg):
    print(f"P9 FAIL: {msg}")
    sys.exit(1)


def main():
    v = json.load(open("p9-store-reads.json"))
    bundle = json.load(open("p7-bundle-packages.json"))
    p7 = json.load(open("p7-store-publication.json"))
    p1 = json.load(open("p1-store-envelope.json"))
    p8 = bundle["packages"][v["base_package"]]
    objects = {k: o["utf8"] for k, o in p8["objects"].items()}
    facts = p8["cas_facts"]
    cases = {c["name"]: c for c in v["cases"]}
    checks = 0

    def expect(cond, msg):
        nonlocal checks
        if not cond:
            fail(msg)
        checks += 1

    # -- frozen anchors ------------------------------------------------
    expect(v["did"] == p7["did"] == p1["did"], "DID anchors drifted")
    expect(objects["manifest.json"] == objects["manifests/2.json"],
           "the tip slot must equal manifest.json")
    m1 = json.loads(objects["manifests/1.json"])
    m2 = json.loads(objects["manifests/2.json"])
    expect(chain_head(m2) == facts["new_manifest_head"],
           "manifest head does not recompute")
    expect(chain_head(m1) == facts["expected_predecessors"][0],
           "predecessor head does not recompute")

    # -- gamma fixtures ------------------------------------------------
    g = v["fixtures"]["gamma"]
    by = {c["name"]: c for c in p7["gamma_cases"]}
    expect(g["grant_jcs"] == by["append_genesis"]["entry_jcs"],
           "grant entry drifted from the frozen p7 case")
    expect(g["action_jcs"] == by["append_ok"]["entry_jcs"],
           "action entry drifted")
    expect(g["concurrent_jcs"] == by["append_cas_stale"]["entry_jcs"],
           "concurrent entry drifted")
    expect(g["corrupted_jcs"] ==
           by["append_bad_entry_signature"]["entry_jcs"],
           "corrupted entry drifted")
    expect(entry_head(g["grant_jcs"]) == g["grant_head"],
           "grant head does not recompute")
    expect(entry_head(g["action_jcs"]) == g["action_head"],
           "action head does not recompute")

    # -- genesis / rotation docs --------------------------------------
    gen = v["fixtures"]["genesis"]
    doc = json.loads(gen["did_json_jcs"])
    root_pub = mb_ed_pub(doc["keys"]["root"])
    expect(gen["did"] == "did:aithos:" + doc["keys"]["root"],
           "genesis DID is not the deposited root key (did:key-style)")
    expect(doc["id"] == gen["did"], "genesis doc id mismatch")
    expect(doc_verifies(gen["did_json_jcs"], root_pub),
           "genesis doc does not self-verify under its root")
    stored = json.loads(p1["did_json_jcs"])
    succ_pub = mb_ed_pub(stored["keys"]["succession"])
    rot = v["fixtures"]["rotation"]
    succ_doc = json.loads(rot["succession_signed_jcs"])
    expect(succ_doc["id"] == v["did"], "successor doc id mismatch")
    expect(succ_doc["signature"]["key"] == "#succession",
           "successor doc must sign as #succession")
    expect(doc_verifies(rot["succession_signed_jcs"], succ_pub),
           "successor doc does not verify under the STORED succession")
    expect(not doc_verifies(rot["root_signed_jcs"], succ_pub),
           "the root-signed successor must NOT verify under succession")

    # -- sidecar content addressing -----------------------------------
    put_ok = cases["put_changeset_ok"]["steps"][0]
    cs_path = put_ok["path_rel"]
    expect(commitment("aithos-core/v1/changeset",
                      put_ok["body_utf8"].encode())
           == "sha256:" + cs_path.split("/")[1].removesuffix(".json"),
           "changeset digest does not recompute from the deposited bytes")
    expect(put_ok["body_utf8"] == objects[cs_path],
           "changeset bytes drifted from the frozen package")
    bad = cases["put_changeset_id_mismatch"]["steps"][0]
    expect(commitment("aithos-core/v1/changeset",
                      bad["body_utf8"].encode())
           != "sha256:" + bad["path_rel"].split("/")[1]
           .removesuffix(".json"),
           "the id_mismatch case digest unexpectedly matches")

    # -- heads bodies --------------------------------------------------
    hj = cases["heads_ok"]["steps"][0]["expect"]["json"]
    expect(hj == {"height": facts["new_height"],
                  "manifest": facts["new_manifest_head"],
                  "gamma": None, "segment": None},
           "heads_ok body drifted from the package facts")
    hg = cases["heads_mandated"]["steps"][0]["expect"]["json"]
    expect(hg["gamma"] == g["action_head"] and hg["segment"] == "2026-07",
           "heads_mandated gamma head drifted")

    # -- list ----------------------------------------------------------
    blobs = v["fixtures"]["blobs"]
    all_paths = sorted(set(objects) | set(blobs)
                       | {v["fixtures"]["enrollment_cert"]})
    lo = cases["list_owner"]["steps"][0]["expect"]["json"]
    expect(lo == {"paths": all_paths, "truncated": False},
           "list_owner does not list the exact stored set, sorted")
    lm = cases["list_mandated_filtered"]["steps"][0]["expect"]["json"]
    expect(all(p.startswith("e/circle/") for p in lm["paths"])
           and lm["paths"], "mandated listing must be e/circle/** only")
    p1_, p2_ = (cases["list_paginated"]["steps"][0]["expect"]["json"],
                cases["list_paginated"]["steps"][1]["expect"]["json"])
    expect(p1_["paths"] == all_paths[:2] and p1_["truncated"] is True,
           "page 1 drifted")
    expect(p2_["paths"] == all_paths[2:4] and
           cases["list_paginated"]["steps"][1]["query"]
           == f"?list=&after={all_paths[1]}&limit=2",
           "page 2 does not continue exactly after page 1")

    # -- batch ---------------------------------------------------------
    bm = cases["batch_mixed"]["steps"][0]
    req_paths = json.loads(bm["body_utf8"])["paths"]
    parts = bm["expect"]["parts"]
    expect([p["path"] for p in parts] == req_paths,
           "batch parts must follow the request order")
    expect([p["part_status"] for p in parts] == [200, 404, 403],
           "batch statuses drifted")
    expect(parts[0]["body_utf8"] == objects[parts[0]["path"]]
           and "body_utf8" not in parts[1] and "body_utf8" not in parts[2],
           "only the 200 part carries a body")
    ov = json.loads(
        cases["batch_overflow"]["steps"][0]["body_utf8"])["paths"]
    expect(len(ov) == 257, "overflow case must carry 257 paths")

    # -- sync ----------------------------------------------------------
    def files_diff(held, current):
        return sorted(k for k, h in current.items() if held.get(k) != h)
    sd = cases["sync_delta"]["steps"][0]["expect"]["parts"]
    expect([p["path"] for p in sd]
           == ["manifest.json"] + files_diff(m1["files"], m2["files"]),
           "sync_delta parts != manifest.json + pinned files-map diff")
    for p in sd:
        expect(p["body_utf8"] == objects[p["path"]],
               f"sync part bytes drifted: {p['path']}")
    sc = cases["sync_current"]["steps"][0]["expect"]["parts"]
    expect([p["path"] for p in sc] == ["manifest.json"],
           "sync_current must pack the tip alone")
    expect("manifests/1.json" in
           cases["sync_gone"]["state"].get("drop_objects", []),
           "sync_gone must drop the held edition slot")

    # -- replica -------------------------------------------------------
    ext = cases["replica_extend_ok"]["steps"][0]
    stored_seg = cases["replica_extend_ok"]["state"][
        "extra_objects"]["gamma/2026-07.jsonl"]
    expect(ext["body_utf8"].startswith(stored_seg),
           "replica_extend_ok must preserve the stored prefix")
    added = ext["body_utf8"][len(stored_seg):].strip("\n")
    expect(entry_head(added) == ext["expect"]["json"]["head"],
           "the advanced head must be the appended entry head")
    npx = cases["replica_not_prefix"]["steps"][0]
    stored_np = cases["replica_not_prefix"]["state"][
        "extra_objects"]["gamma/2026-07.jsonl"]
    expect(not npx["body_utf8"].startswith(stored_np),
           "replica_not_prefix unexpectedly preserves the prefix")
    stale = cases["replica_cas_stale"]["steps"][0]
    expect(stale["if_head"] == g["grant_head"]
           and stale["expect"]["head"] == g["action_head"],
           "replica_cas_stale heads drifted")
    badv = json.loads(g["corrupted_jcs"])
    signer_pub = mb_ed_pub(json.loads(p1["mandate_jcs"])
                           ["grantee"]["pubkey"])
    expect(not doc_verifies(jcs(badv), signer_pub),
           "the corrupted entry unexpectedly verifies")

    # -- registry discipline ------------------------------------------
    known_reasons = {"form", "signature", "chain", "prev_hash_mismatch",
                     "id_mismatch", "subject_mismatch", "entry_signature",
                     "prev_mismatch", "prefix_mismatch"}
    for c in v["cases"]:
        for s in c["steps"]:
            r = s["expect"].get("reason")
            if r is not None:
                expect(r in known_reasons,
                       f"unknown artifact_invalid reason: {r}")

    print(f"P9 ok ({checks} checks, {len(v['cases'])} cases) — "
          "independent recomputation matches the vector.")


if __name__ == "__main__":
    main()

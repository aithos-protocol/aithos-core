#!/usr/bin/env python3
"""Generator for p7-store-publication.json (piste P, gate contrat P2 —
INFRA-PROVIDER annexes A.4/A.5 on REAL bundle publications).

Two-oracle construction, graved at the P2 contract gate:

  - The publication packages are emitted by `gen-p7-bundle` (Rust), which
    calls the aithos-bundle keyless façade (assemble_draft2_candidate /
    export_keyless / verify_for_cas) — NO publication cryptography is
    re-invented outside the bundle. Its deterministic output is committed
    as `p7-bundle-packages.json`; this script consumes it verbatim.
  - Everything envelope/CAS/gamma-side stays the Python second
    implementation (blake3 + PyNaCl + base58), exactly like p1..p6.

Layer rule (the p2 precedent): p7 is a CAS/A.4-layer contract. Cases carry
no signed envelope; the replay harness signs with the COMMITTED keys
(a1 seed -> owner, p1 agent_sk -> grantee, cb2 grantee seed -> delegate),
the way vectors_replay already re-signs the signed half.

State model per case: `state_heads` is the A.5 heads-table tuple the server
holds; `state_objects` are pre-seeded stored objects the check needs.
server-side CAS reads the TABLE (A.5), never re-hashes a stored manifest.

Usage: python3 gen-p7.py   (from vectors/; reads p7-bundle-packages.json)
"""

import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

jcs = gen_p.jcs
sign_doc = gen_p.sign_doc
corrupt_sig = gen_p.corrupt_sig
entry_head = gen_p.entry_head
sha256_hex = gen_p.sha256_hex

DID = gen_p.DID
TENANT = gen_p.DID_PATH_TENANT
MANDATE_ID = gen_p.MANDATE_ID
root_sk = gen_p.root_sk
content_sk = gen_p.content_sk
agent_sk = gen_p.agent_sk
mb_ed = gen_p.mb_ed
AGENT_PUB = gen_p.AGENT_PUB


def load_bundle_packages():
    with open("p7-bundle-packages.json") as f:
        return json.load(f)


def head_of(pkg):
    return pkg["cas_facts"]["new_manifest_head"]


def build_manifest_cases(bundle):
    m1_jcs = bundle["m1"]["jcs"]
    c1 = bundle["m1"]["chain_hash"]
    pkg_a = bundle["packages"]["a_h2"]
    pkg_b = bundle["packages"]["b_h2_twin"]
    pkg_m = bundle["packages"]["m_h3_merge"]
    pkg_d = bundle["packages"]["delegated_cb2"]

    head_a, head_b, head_m = head_of(pkg_a), head_of(pkg_b), head_of(pkg_m)
    merge_parent_head = pkg_m["cas_facts"]["expected_predecessors"][0]
    d_facts = pkg_d["cas_facts"]
    d_pred = d_facts["expected_predecessors"][0]
    d_manifest = json.loads(pkg_d["manifest_jcs"])
    d_cert_path = "certs/" + d_manifest["authorized_via"][0] + ".json"
    d_cert_jcs = pkg_d["objects"][d_cert_path]["utf8"]

    cases = [
        {"name": "genesis_publish",
         "signer": "owner_root",
         "state_heads": None, "state_objects": {}, "if_head": "none",
         "body_jcs": m1_jcs,
         "expect": {"status": "accept", "new_head": "sha256:" + c1,
                    "new_height": 1},
         "note": "REAL bundle draft.1 genesis (Manifest::build): If-Head "
                 "none + height 1 + prev_hash \"\" (annexe A.5)"},
        {"name": "publish_ok",
         "signer": "owner_root",
         "state_heads": {"height": 1, "manifest": "sha256:" + c1},
         "state_objects": {}, "if_head": "sha256:" + c1,
         "body_jcs": pkg_a["manifest_jcs"],
         "expect": {"status": "accept", "new_head": head_a, "new_height": 2},
         "note": "REAL bundle draft.2 owner package over the stored head; "
                 "the server persists reachable_objects opaque and derives "
                 "NO semantic verdict (doctrine)"},
        {"name": "publish_cas_required",
         "signer": "owner_root",
         "state_heads": {"height": 1, "manifest": "sha256:" + c1},
         "state_objects": {}, "if_head": None,
         "body_jcs": pkg_a["manifest_jcs"],
         "expect": {"status": 428, "error": "cas_required"},
         "note": "CAS is mandatory on manifest.json: no silent overwrite"},
        {"name": "publish_cas_stale",
         "signer": "owner_root",
         "state_heads": {"height": 2, "manifest": head_a},
         "state_objects": {}, "if_head": "sha256:" + c1,
         "body_jcs": pkg_b["manifest_jcs"],
         "expect": {"status": 409, "error": "cas_mismatch",
                    "head": head_a, "height": 2},
         "note": "the twin lost the race: 409 + current head, the loser "
                 "rebases (§02.6) — the store never arbitrates"},
        {"name": "publish_prev_hash_mismatch",
         "signer": "owner_root",
         "state_heads": {"height": 2, "manifest": head_b},
         "state_objects": {}, "if_head": head_b,
         "body_jcs": pkg_a["manifest_jcs"],
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "prev_hash_mismatch"},
         "note": "If-Head matches the stored head but the artifact does not "
                 "chain it (A.4: prev_hash and height verified before any "
                 "write)"},
        {"name": "genesis_bad_signature",
         "signer": "owner_root",
         "state_heads": None, "state_objects": {}, "if_head": "none",
         "body_jcs": jcs(corrupt_sig(json.loads(m1_jcs))),
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "signature"},
         "note": "CAS grammar passes, the manifest signature fails A.4: "
                 "the server verifies like a verifier, never repairs"},
        {"name": "publish_merge_no_arbitration",
         "signer": "owner_root",
         "state_heads": {"height": 2, "manifest": merge_parent_head},
         "state_objects": {}, "if_head": merge_parent_head,
         "body_jcs": pkg_m["manifest_jcs"],
         "expect": {"status": "accept", "new_head": head_m, "new_height": 3},
         "note": "REAL bundle merge package: merges names both h2 twins, "
                 "prev_hash pins the ascending-first parent (== the stored "
                 "head here). Accepted AS-IS: the CAS serializes, the "
                 "witness observes, the store NEVER picks a fork winner"},
        {"name": "publish_delegated",
         "signer": "cb2_grantee",
         "subject_did": d_facts["subject"],
         "mandate_chain": d_manifest["authorized_via"],
         "state_heads": {"height": 1, "manifest": d_pred},
         "state_objects": {d_cert_path: d_cert_jcs},
         "if_head": d_pred,
         "body_jcs": pkg_d["manifest_jcs"],
         "expect": {"status": "accept", "new_head": head_of(pkg_d),
                    "new_height": 2},
         "note": "the COMMITTED cb2-draft2-carriers candidate re-exported "
                 "keyless: a real grantee-signed publication under "
                 "authorized_via; the leaf key resolves against the stored "
                 "cert, the root anchors in the DID literal (A.4)"},
    ]
    return cases


def build_cert_cases(bundle):
    mandate = json.loads(json.load(open("p1-store-envelope.json"))["mandate_jcs"])
    foreign_subject = bundle["packages"]["delegated_cb2"]["cas_facts"]["subject"]
    foreign = json.loads(jcs(mandate))
    foreign["subject"] = foreign_subject
    foreign["nonce"] = "p7vectors0001"
    foreign = sign_doc(foreign, root_sk)
    return [
        {"name": "deposit_cert_ok",
         "signer": "owner_root",
         "path": f"certs/{MANDATE_ID}.json",
         "body_jcs": jcs(mandate),
         "expect": {"status": "accept"},
         "note": "the committed p1 mandate: id == filename, subject == path "
                 "DID, root-signed link verifies (A.4)"},
        {"name": "deposit_cert_foreign_subject",
         "signer": "owner_root",
         "path": f"certs/{MANDATE_ID}.json",
         "body_jcs": jcs(foreign),
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "subject_mismatch"},
         "note": "subject != <did> of the path: refused at deposit, "
                 "fail-closed (A.4)"},
    ]


def build_gamma_cases():
    agent_mb = mb_ed(AGENT_PUB)
    g_grant = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P7G1", "prev": "",
        "at": "2026-07-19T11:00:00Z", "kind": "grant", "target": MANDATE_ID,
        "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""}},
        content_sk)
    h1 = entry_head(jcs(g_grant))
    g_action = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P7A2", "prev": h1,
        "at": "2026-07-19T11:05:00Z", "kind": "action", "target": "x.gmail",
        "authorized_by": MANDATE_ID, "authorized_via": [MANDATE_ID],
        "payload": {"action": "reply", "args_hash": "sha256:" + "ab" * 32},
        "signature": {"alg": "ed25519", "key": agent_mb, "value": ""}},
        agent_sk)
    h2 = entry_head(jcs(g_action))
    g_concurrent = sign_doc({
        "v": 1, "id": "gamma_" + "0000000000000000000000P7A3", "prev": h1,
        "at": "2026-07-19T11:06:00Z", "kind": "action", "target": "x.gmail",
        "authorized_by": MANDATE_ID, "authorized_via": [MANDATE_ID],
        "payload": {"action": "reply", "args_hash": "sha256:" + "cd" * 32},
        "signature": {"alg": "ed25519", "key": agent_mb, "value": ""}},
        agent_sk)
    return [
        {"name": "append_genesis",
         "signer": "grantee",
         "state_heads": None, "if_head": "none", "entry_jcs": jcs(g_grant),
         "expect": {"status": "accept", "new_head": h1},
         "note": "empty log: If-Head none + prev \"\" (A.5)"},
        {"name": "append_ok",
         "signer": "grantee",
         "state_heads": {"gamma": h1}, "if_head": h1,
         "entry_jcs": jcs(g_action),
         "expect": {"status": "accept", "new_head": h2},
         "note": "prev == If-Head == stored head; the entry itself is "
                 "verified by core at deposit (A.4), the store recopies "
                 "no rule"},
        {"name": "append_cas_required",
         "signer": "grantee",
         "state_heads": {"gamma": h1}, "if_head": None,
         "entry_jcs": jcs(g_action),
         "expect": {"status": 428, "error": "cas_required"},
         "note": "CAS is mandatory on gamma append"},
        {"name": "append_cas_stale",
         "signer": "grantee",
         "state_heads": {"gamma": h2}, "if_head": h1,
         "entry_jcs": jcs(g_concurrent),
         "expect": {"status": 409, "error": "cas_mismatch", "head": h2},
         "note": "concurrent writer lost: 409 + current head, the client "
                 "re-chains and retries"},
        {"name": "append_bad_entry_signature",
         "signer": "grantee",
         "state_heads": {"gamma": h1}, "if_head": h1,
         "entry_jcs": jcs(corrupt_sig(g_action)),
         "expect": {"status": 400, "error": "artifact_invalid",
                    "reason": "entry_signature"},
         "note": "CAS passes, the entry signature fails A.4 verification"},
    ]


def main():
    gen_p.self_check_a1()
    gen_p.self_check_g1()
    bundle = load_bundle_packages()
    assert bundle["anchors"]["did"] == DID, "bundle intermediate DID drift"
    p1 = json.load(open("p1-store-envelope.json"))
    assert bundle["anchors"]["did_json_jcs"] == p1["did_json_jcs"], \
        "bundle intermediate did.json drift vs committed p1"

    out = {
        "vector": "P7",
        "description":
            "Publication on REAL aithos-bundle packages (INFRA-PROVIDER "
            "annexes A.4/A.5, gate contrat P2). Two-oracle construction: "
            "packages emitted by gen-p7-bundle (Rust) through the keyless "
            "facade assemble_draft2_candidate/export_keyless/verify_for_cas "
            "-- no re-invented publication crypto -- committed as "
            "p7-bundle-packages.json and consumed verbatim; CAS states, "
            "gamma entries and cert cases stay the Python second "
            "implementation (blake3+PyNaCl), anchored on committed A1+G1+P1 "
            "and on the frozen cb2-draft2-carriers candidate (delegated "
            "case). Layer rule: like p2, cases carry NO envelope -- the "
            "replay harness signs with the committed keys (signer field). "
            "state_heads is the A.5 heads-table tuple; the server CAS reads "
            "the table, never re-hashes stored bytes. Every accept also "
            "freezes the typed CAS facts the facade returned "
            "(new_manifest_head/new_gamma_head/new_height/"
            "reachable_objects/package_digest) in p7-bundle-packages.json: "
            "success -> persist reachable_objects opaque + advance both "
            "heads atomically; rejection -> one closed A.7 code. NOTE "
            "(redline pending, gate 5/8): the draft.2 carrier layout "
            "(manifests/, changesets/, evidence/, public/sections/, "
            "circle/blobs/*.json, indices/, roots/, vault/) is NOT in the "
            "A.1 wire grammar yet -- serving these objects requires the "
            "A.1/A.3 draft.2 redline.",
        "tenant": TENANT,
        "did": DID,
        "bundle_packages": "p7-bundle-packages.json",
        "manifest_cases": build_manifest_cases(bundle),
        "cert_cases": build_cert_cases(bundle),
        "gamma_cases": build_gamma_cases(),
    }
    with open("p7-store-publication.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("self-checks vs A1 + G1 + P1 + bundle anchors passed; "
          "wrote p7-store-publication.json")


if __name__ == "__main__":
    main()

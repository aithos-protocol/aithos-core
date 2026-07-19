#!/usr/bin/env python3
"""Replay harness for the P7/P8 vectors (gate contrat P2). Re-derives every
checkable value FROM THE JSON FILES ONLY — the Python second implementation
facing the bundle-emitted packages. It never imports gen-p7-bundle output
logic nor gen-p7 internals.

Checks:
  - p7-bundle-packages.json: JCS canonicality; the p1 did.json anchor;
    every manifest signature (draft.1 root, draft.2 owner root, draft.2
    delegated under the cert leaf key with the root anchored in the DID
    literal); chain hashes vs the façade's CAS facts; files pins == the
    exact object bytes; manifest.json == the edition tip; reachable set;
    package digest recomputed from scratch; keyless shape rule.
  - p7-store-publication.json: a from-scratch simulation of the A.4/A.5
    decision order (heads-table CAS) against every expected verdict;
    cert-deposit rules; gamma entry signatures + chain + CAS.
  - p8-cold-roundtrip.json: object-set integrity, tamper-case
    well-formedness, read-plan consistency with the committed p1 mandate.

Usage: python3 verify-p7.py   (from vectors/)
"""

import hashlib
import json

import base58
import nacl.signing

FORBIDDEN = {"seed", "private_key", "secret_key", "owner_keys", "dk",
             "credential", "plaintext", "capability"}


def jcs(obj):
    return json.dumps(obj, sort_keys=True, separators=(",", ":"),
                      ensure_ascii=False)


def canonical(s):
    assert jcs(json.loads(s)) == s, "non-canonical JCS string"
    return json.loads(s)


def sha256_hex(b):
    return hashlib.sha256(b).hexdigest()


def mb_decode(mb):
    raw = base58.b58decode(mb[1:])
    assert raw[:2] in (b"\xed\x01", b"\xec\x01"), "bad multicodec"
    return raw[2:]


def did_root_pub(did):
    """The root key IS the DID literal (did:aithos:<multibase>)."""
    return mb_decode(did.split("did:aithos:")[1])


def verify_doc(doc, pub):
    d = json.loads(jcs(doc))
    sig = bytes.fromhex(d["signature"]["value"])
    d["signature"]["value"] = ""
    try:
        nacl.signing.VerifyKey(pub).verify(jcs(d).encode(), sig)
        return True
    except Exception:
        return False


def chain_hash(manifest):
    m = json.loads(jcs(manifest))
    m["signature"]["value"] = ""
    return sha256_hex(jcs(m).encode())


def entry_head(entry_jcs):
    return "sha256:" + sha256_hex(entry_jcs.encode())


def object_bytes(entry):
    if "utf8" in entry:
        return entry["utf8"].encode()
    return bytes.fromhex(entry["hex"])


def assert_keyless(value, path="$"):
    if isinstance(value, dict):
        for name, child in value.items():
            assert name.lower() not in FORBIDDEN, \
                f"private-material shape at {path}.{name}"
            assert_keyless(child, f"{path}.{name}")
    elif isinstance(value, list):
        for i, child in enumerate(value):
            assert_keyless(child, f"{path}[{i}]")


# --------------------------------------------------- bundle intermediate

def manifest_signer_pub(manifest, package, subject):
    key = manifest["signature"]["key"]
    if key == "#root":
        return did_root_pub(subject)
    # Delegated draft.2: the key must be the leaf grantee key of the chain
    # whose certs travel in the package objects; the chain root anchors in
    # the subject DID literal.
    assert manifest.get("authorized_via"), "delegated manifest without chain"
    cert_path = "certs/" + manifest["authorized_via"][-1] + ".json"
    cert = canonical(package["objects"][cert_path]["utf8"])
    assert cert["subject"] == subject, "cert subject != package subject"
    assert cert["grantee"]["pubkey"] == key, "manifest key != cert leaf key"
    assert verify_doc(cert, did_root_pub(subject)), \
        "cert root signature does not verify under the DID literal"
    return mb_decode(key)


def check_bundle_packages():
    bundle = json.load(open("p7-bundle-packages.json"))
    p1 = json.load(open("p1-store-envelope.json"))
    assert bundle["anchors"]["did_json_jcs"] == p1["did_json_jcs"], \
        "did.json anchor drift vs committed p1"
    canonical(bundle["anchors"]["did_json_jcs"])

    m1 = canonical(bundle["m1"]["jcs"])
    root_pub = did_root_pub(bundle["anchors"]["did"])
    assert verify_doc(m1, root_pub), "m1 signature"
    assert chain_hash(m1) == bundle["m1"]["chain_hash"], "m1 chain hash"
    assert m1["aithos-core"] == "1.0.0-draft.1" and \
        "operation_ref" not in m1, "m1 is draft.1 without carriers"

    for name, pkg in bundle["packages"].items():
        facts = pkg["cas_facts"]
        manifest = canonical(pkg["manifest_jcs"])
        subject = facts["subject"]
        assert_keyless(pkg["candidate"], f"{name}.candidate")
        assert_keyless(pkg["context"], f"{name}.context")

        # Profile + carriers
        assert manifest["aithos-core"] == "1.0.0-draft.2" == \
            facts["manifest_profile"], f"{name}: profile"
        for carrier in ("operation_ref", "changeset_ref", "evidence_ref"):
            assert manifest.get(carrier) is not None, f"{name}: {carrier}"

        # Signature under the resolved actor key
        pub = manifest_signer_pub(manifest, pkg, subject)
        assert verify_doc(manifest, pub), f"{name}: manifest signature"

        # Chain hash == the façade's new_manifest_head
        assert "sha256:" + chain_hash(manifest) == \
            facts["new_manifest_head"], f"{name}: chain hash vs CAS facts"
        assert manifest["edition"]["height"] == facts["new_height"], \
            f"{name}: height"
        assert manifest.get("gamma_head", "") == facts["new_gamma_head"], \
            f"{name}: gamma head"

        # Predecessor topology
        preds = facts["expected_predecessors"]
        bare = [p.split("sha256:")[1] for p in preds]
        mode = facts["mode"]
        if mode == "normal" and facts["new_height"] == 1:
            assert manifest["edition"]["prev_hash"] == "" and preds == []
        elif mode == "normal":
            assert len(preds) == 1 and \
                manifest["edition"]["prev_hash"] == bare[0], f"{name}: prev"
            assert manifest.get("merges", []) == []
        elif mode == "merge":
            assert len(preds) == 2 and bare == sorted(bare) and \
                bare[0] < bare[1], f"{name}: merge predecessors"
            assert manifest["edition"]["prev_hash"] == bare[0]
            assert manifest.get("merges") == bare, f"{name}: merges field"

        # Objects: pins, tip identity, reachable set
        objects = {p: object_bytes(e) for p, e in pkg["objects"].items()}
        assert facts["reachable_objects"] == sorted(objects), \
            f"{name}: reachable_objects"
        assert objects["manifest.json"].decode() == pkg["manifest_jcs"], \
            f"{name}: manifest.json is the tip bytes"
        tip_path = f"manifests/{facts['new_height']}.json"
        assert objects[tip_path] == objects["manifest.json"], \
            f"{name}: {tip_path} == tip"
        for path, want in manifest["files"].items():
            assert path in objects, f"{name}: pinned object missing {path}"
            assert sha256_hex(objects[path]) == want, f"{name}: pin {path}"
        for path in objects:
            assert path in manifest["files"] or \
                path in ("manifest.json", tip_path), \
                f"{name}: unpinned object {path}"

        # Package digest recomputed from scratch (the bundle's exact recipe)
        digest_doc = {
            "aithos-keyless-publication": "1.0.0-draft.1",
            "candidate": pkg["candidate"],
            "context": pkg["context"],
            "objects_hex": {p: b.hex() for p, b in sorted(objects.items())},
        }
        assert "sha256:" + sha256_hex(jcs(digest_doc).encode()) == \
            facts["package_digest"], f"{name}: package digest"
    print(f"bundle packages ok ({len(bundle['packages'])} packages, "
          "digests recomputed)")
    return bundle


# ------------------------------------------------------ p7 wire simulation

def simulate_manifest_deposit(case, did):
    """A.4/A.5 order for PUT manifest.json, heads-table CAS."""
    state = case["state_heads"]
    if case["if_head"] is None:
        return {"status": 428, "error": "cas_required"}
    want_head = "none" if state is None else state["manifest"]
    if case["if_head"] != want_head:
        got = {"status": 409, "error": "cas_mismatch",
               "head": state["manifest"]}
        got["height"] = state["height"]
        return got
    manifest = canonical(case["body_jcs"])
    # form: known profile, carrier discipline
    carriers = [manifest.get(c) for c in
                ("operation_ref", "changeset_ref", "evidence_ref")]
    profile = manifest.get("aithos-core")
    if profile == "1.0.0-draft.1":
        if any(c is not None for c in carriers):
            return {"status": 400, "error": "artifact_invalid",
                    "reason": "form"}
    elif profile == "1.0.0-draft.2":
        if any(c is None for c in carriers):
            return {"status": 400, "error": "artifact_invalid",
                    "reason": "form"}
    else:
        return {"status": 400, "error": "artifact_invalid", "reason": "form"}
    # signature: root or delegated (A.4, before prev/height)
    subject = case.get("subject_did", did)
    key = manifest["signature"]["key"]
    if key == "#root":
        pub = did_root_pub(subject)
    else:
        chain = manifest.get("authorized_via", [])
        if not chain or case.get("mandate_chain") != chain:
            return {"status": 400, "error": "artifact_invalid",
                    "reason": "chain"}
        cert = canonical(case["state_objects"]["certs/" + chain[-1] + ".json"])
        if cert["subject"] != subject or cert["grantee"]["pubkey"] != key \
                or not verify_doc(cert, did_root_pub(subject)):
            return {"status": 400, "error": "artifact_invalid",
                    "reason": "chain"}
        pub = mb_decode(key)
    if not verify_doc(manifest, pub):
        return {"status": 400, "error": "artifact_invalid",
                "reason": "signature"}
    # height + prev chain the STORED HEAD TUPLE (A.5 table)
    stored_height = 0 if state is None else state["height"]
    stored_head = "" if state is None else state["manifest"].split("sha256:")[1]
    if manifest["edition"]["height"] != stored_height + 1 or \
            manifest["edition"]["prev_hash"] != stored_head:
        return {"status": 400, "error": "artifact_invalid",
                "reason": "prev_hash_mismatch"}
    return {"status": "accept",
            "new_head": "sha256:" + chain_hash(manifest),
            "new_height": manifest["edition"]["height"]}


def check_p7():
    p7 = json.load(open("p7-store-publication.json"))
    did = p7["did"]

    for case in p7["manifest_cases"]:
        got = simulate_manifest_deposit(case, did)
        assert got == case["expect"], \
            f"P7 manifest {case['name']}: got {got}, want {case['expect']}"

    for case in p7["cert_cases"]:
        cert = canonical(case["body_jcs"])
        filename_id = case["path"].split("/")[1].removesuffix(".json")
        if cert["id"] != filename_id:
            got = {"status": 400, "error": "artifact_invalid",
                   "reason": "id_mismatch"}
        elif cert["subject"] != did:
            got = {"status": 400, "error": "artifact_invalid",
                   "reason": "subject_mismatch"}
        elif not verify_doc(cert, did_root_pub(did)):
            got = {"status": 400, "error": "artifact_invalid",
                   "reason": "signature"}
        else:
            got = {"status": "accept"}
        assert got == case["expect"], f"P7 cert {case['name']}: {got}"

    did_doc = canonical(json.load(
        open("p1-store-envelope.json"))["did_json_jcs"])
    root_pub = mb_decode(did_doc["keys"]["root"])
    content_pub = mb_decode(did_doc["keys"]["content"])
    for case in p7["gamma_cases"]:
        state = case["state_heads"]
        entry = canonical(case["entry_jcs"])
        if case["if_head"] is None:
            got = {"status": 428, "error": "cas_required"}
        elif case["if_head"] != ("none" if state is None else state["gamma"]):
            got = {"status": 409, "error": "cas_mismatch",
                   "head": state["gamma"]}
        else:
            key = entry["signature"]["key"]
            pub = {"#content": content_pub, "#root": root_pub}.get(key) \
                or mb_decode(key)
            if not verify_doc(entry, pub):
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "entry_signature"}
            elif entry["prev"] != ("" if state is None else state["gamma"]):
                got = {"status": 400, "error": "artifact_invalid",
                       "reason": "prev_mismatch"}
            else:
                got = {"status": "accept",
                       "new_head": entry_head(case["entry_jcs"])}
        assert got == case["expect"], f"P7 gamma {case['name']}: {got}"
    print(f"P7 ok ({len(p7['manifest_cases'])} manifest + "
          f"{len(p7['cert_cases'])} cert + "
          f"{len(p7['gamma_cases'])} gamma cases)")


# ------------------------------------------------------------------- p8

def check_p8(bundle):
    p8 = json.load(open("p8-cold-roundtrip.json"))
    pkg = bundle["packages"][p8["package"]]
    facts = pkg["cas_facts"]
    objects = {p: object_bytes(e) for p, e in pkg["objects"].items()}
    assert p8["package_digest"] == facts["package_digest"]
    assert p8["new_manifest_head"] == facts["new_manifest_head"]
    assert p8["new_gamma_head"] == facts["new_gamma_head"]
    assert p8["reachable_objects"] == sorted(objects), "P8 object set"
    assert p8["import_rules"]["fresh_store_only"] is True

    for case in p8["tamper_cases"]:
        mutate = case["mutate"]
        assert mutate["path"] in objects, f"P8 {case['name']}: unknown path"
        assert case["must_fail"] == "cold_verify"
        if "replace_with_object" in mutate:
            assert mutate["replace_with_object"] in objects
            assert objects[mutate["replace_with_object"]] != \
                objects[mutate["path"]], f"P8 {case['name']}: no-op tamper"
        if "replace_utf8" in mutate:
            assert mutate["replace_utf8"].encode() != objects[mutate["path"]]

    mandate = canonical(p8["read_plan"]["grantee"]["mandate_jcs"])
    p1 = json.load(open("p1-store-envelope.json"))
    assert p8["read_plan"]["grantee"]["mandate_jcs"] == p1["mandate_jcs"], \
        "P8 grantee mandate is the committed p1 mandate"
    assert "read.circle" in mandate["perimeter"]
    for read in p8["read_plan"]["grantee"]["reads"]:
        assert read["path"] in objects, f"P8 read plan: {read['path']}"
    for read in p8["read_plan"]["owner"]:
        assert read["path"] in objects, f"P8 owner read: {read['path']}"
    print(f"P8 ok ({len(p8['tamper_cases'])} tamper cases, read plan bound "
          "to the committed p1 mandate)")


if __name__ == "__main__":
    bundle = check_bundle_packages()
    check_p7()
    check_p8(bundle)
    print("P7/P8 vectors replay green (Python second implementation)")

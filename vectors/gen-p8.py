#!/usr/bin/env python3
"""Generator for p8-cold-roundtrip.json (piste P, gate contrat P2 — the
keyless cold roundtrip: import_keyless / cold_verify on a virgin store,
then the wire read plan).

The package is `p8_cold` from the committed `p7-bundle-packages.json`
(emitted by gen-p7-bundle through the aithos-bundle façade — no re-invented
crypto). This script freezes:

  - the exact object set a virgin store must ingest (byte-complete);
  - the keyless discipline (no private-material shape anywhere);
  - the tamper matrix: each defect must fail cold verification CLOSED;
  - the wire read plan for owner and grantee (grantee under the committed
    p1 mandate, read.circle) — with the A.1 draft.2 redline dependency
    named explicitly for the alias paths.

Usage: python3 gen-p8.py   (from vectors/; reads p7-bundle-packages.json)
"""

import importlib.util
import json
import sys

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

jcs = gen_p.jcs

FORBIDDEN_SHAPES = [
    "seed", "private_key", "secret_key", "owner_keys", "dk",
    "credential", "plaintext", "capability",
]


def assert_keyless(value, path="$"):
    """Python re-check of the bundle's reject_private_shape rule."""
    if isinstance(value, dict):
        for name, child in value.items():
            assert name.lower() not in FORBIDDEN_SHAPES, \
                f"private-material shape at {path}.{name}"
            assert_keyless(child, f"{path}.{name}")
    elif isinstance(value, list):
        for i, child in enumerate(value):
            assert_keyless(child, f"{path}[{i}]")


def main():
    gen_p.self_check_a1()
    bundle = json.load(open("p7-bundle-packages.json"))
    pkg = bundle["packages"]["p8_cold"]
    facts = pkg["cas_facts"]
    assert bundle["anchors"]["did"] == gen_p.DID, "intermediate DID drift"
    assert_keyless(pkg["candidate"])
    assert_keyless(pkg["context"])

    objects = pkg["objects"]
    paths = sorted(objects.keys())
    assert facts["reachable_objects"] == paths, "reachable_objects drift"

    circle_path = next(p for p in paths if p.startswith("circle/blobs/"))
    public_path = next(p for p in paths if p.startswith("public/sections/"))
    manifest_height = json.loads(pkg["manifest_jcs"])["edition"]["height"]

    p1 = json.load(open("p1-store-envelope.json"))
    mandate = json.loads(p1["mandate_jcs"])

    tamper_cases = [
        {"name": "substituted_object_fails_cold",
         "mutate": {"path": "did.json", "replace_utf8": "{}"},
         "must_fail": "cold_verify",
         "note": "one downloaded object substituted: the pinned hash no "
                 "longer matches — fail-closed"},
        {"name": "missing_pinned_object_fails_cold",
         "mutate": {"path": public_path, "drop": True},
         "must_fail": "cold_verify",
         "note": "one pinned object dropped from the download: the store "
                 "is incomplete — fail-closed"},
        {"name": "tip_mismatch_fails_cold",
         "mutate": {"path": f"manifests/{manifest_height}.json",
                    "replace_with_object": "manifests/1.json"},
         "must_fail": "cold_verify",
         "note": "manifest.json no longer equals the edition history tip — "
                 "fail-closed"},
    ]

    out = {
        "vector": "P8",
        "description":
            "Keyless cold roundtrip (gate contrat P2): the p8_cold package "
            "of p7-bundle-packages.json (aithos-bundle export_keyless; one "
            "public section + one sealed circle blob, owner-signed draft.2 "
            "over the draft.1 genesis) installs into a VIRGIN store in one "
            "transaction (import_keyless refuses a non-empty store), "
            "survives stop/restart, re-downloads into a second virgin "
            "store, and cold-verifies from bytes alone (cold_verify / "
            "cold_verify_for_cas == the producer verdict). The package "
            "carries no private-material shape (re-checked here in "
            "Python). Tamper matrix: substitution, omission and tip "
            "mismatch each fail CLOSED. Wire read plan: owner reads the "
            "manifest and did.json; the grantee reads the covered circle "
            "sub-tree under the COMMITTED p1 mandate (read.circle). NOTE "
            "(redline pending, gate 5/8): the alias carrier paths "
            "(public/sections/, circle/blobs/*.json, manifests/, "
            "changesets/, evidence/, indices/, roots/, vault/) are not in "
            "the A.1 wire grammar yet; the read plan freezes the "
            "POST-redline contract and names the dependency.",
        "tenant": gen_p.DID_PATH_TENANT,
        "did": gen_p.DID,
        "bundle_packages": "p7-bundle-packages.json",
        "package": "p8_cold",
        "package_digest": facts["package_digest"],
        "new_manifest_head": facts["new_manifest_head"],
        "new_gamma_head": facts["new_gamma_head"],
        "reachable_objects": paths,
        "keyless_forbidden_shapes": FORBIDDEN_SHAPES,
        "import_rules": {
            "fresh_store_only": True,
            "atomic": "one logical commit point; a failed put rolls back",
        },
        "tamper_cases": tamper_cases,
        "read_plan": {
            "owner": [
                {"method": "GET", "path": "manifest.json",
                 "expect": {"status": "accept"}},
                {"method": "GET", "path": "did.json",
                 "expect": {"status": "accept"}},
            ],
            "grantee": {
                "mandate_jcs": jcs(mandate),
                "mandate_id": mandate["id"],
                "reads": [
                    {"method": "GET", "path": circle_path,
                     "expect": {"status": "accept"},
                     "requires": "A.1 draft.2 redline (alias carrier paths)"},
                    {"method": "GET", "path": public_path,
                     "expect": {"status": "accept"},
                     "requires": "A.1 draft.2 redline (alias carrier paths)"},
                ],
                "denied": [
                    {"method": "GET", "path": "e/self/blobs/01000000000000000000000000.enc",
                     "expect": {"status": 403, "error": "not_covered"},
                     "note": "outside the read.circle perimeter: default "
                             "deny (A.3), unchanged by the redline"},
                ],
            },
        },
    }
    with open("p8-cold-roundtrip.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("keyless + anchor checks passed; wrote p8-cold-roundtrip.json")


if __name__ == "__main__":
    main()

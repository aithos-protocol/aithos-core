#!/usr/bin/env python3
"""P9 — store reads vector (gate contrat étape 5, P2).

The read-surface + remaining-write contract of INFRA-PROVIDER annexe A:
GET /heads, GET ?list=, POST /batch, POST /sync (A.3), the draft.2
servable layout (A.1 redline gate 5, 2026-07-20: manifests/<h>.json,
changesets/, evidence/, K1-C aliases), PUT did.json (genesis + succession
replacement, A.4) and the gamma segment replica (PUT, mode A, A.5).

Rules of the house:
- Every published byte comes from the FROZEN packages of
  p7-bundle-packages.json (aithos-bundle export_keyless — never
  re-invented crypto) and the committed p1/p7 fixtures. This generator
  derives and re-verifies; it never asserts a hash by hand.
- p1..p8 are untouched. P9 only reads them.
- The expected wire answers follow the redlines engraved 2026-07-20
  (accept bodies 200 {"head"[,"height"]} / 204; closed If-Head grammar;
  A.7 registry unchanged — `prefix_mismatch` is the ONE new
  artifact_invalid reason this vector names, carried to the gate).

Named arbitrages frozen here (gate 5, never resolved silently):
- did.json replacement: interim reading of A.4 — the successor document
  (same id) verifies under the STORED document's succession key; the
  §10.4 epoch-artifact (next_did) question stays open.
- a malformed /batch /sync body answers `envelope_invalid` (the request
  form is part of the closed wire form; the A.7 registry stays closed).
- /sync pack = manifest.json first, then the lexicographic diff of the
  pinned files maps between the held and current editions; a held
  edition whose manifests/<h>.json slot is gone answers 410.
"""

import hashlib
import importlib.util
import json
import sys

import nacl.bindings
import nacl.signing

spec = importlib.util.spec_from_file_location("gen_p", "gen-p.py")
gen_p = importlib.util.module_from_spec(spec)
sys.modules["gen_p"] = gen_p
spec.loader.exec_module(gen_p)

AT = "2026-07-20T12:00:00Z"
CHANGESET_DOMAIN = "aithos-core/v1/changeset"
SEGMENT = "2026-07"
SEGMENT_KEY = f"gamma/{SEGMENT}.jsonl"
CIRCLE_BLOB = "e/circle/blobs/01000000000000000000000000.enc"
CIRCLE_MISSING = "e/circle/blobs/01000000000000000000000001.enc"
SELF_BLOB = "e/self/blobs/01000000000000000000000000.enc"
CIRCLE_BYTES = "p9-circle-ciphertext"
SELF_BYTES = "p9-self-ciphertext"


def commitment(domain: str, data: bytes) -> str:
    h = hashlib.sha256()
    h.update(domain.encode())
    h.update(b"\x00")
    h.update(data)
    return "sha256:" + h.hexdigest()


def load_frozen():
    bundle = json.load(open("p7-bundle-packages.json"))
    p7 = json.load(open("p7-store-publication.json"))
    p8v = json.load(open("p8-cold-roundtrip.json"))
    return bundle, p7, p8v


def gamma_fixture(p7):
    """The committed p7 gamma chain, byte-frozen: grant (genesis), bound
    action, concurrent twin, corrupted-signature action."""
    by_name = {c["name"]: c for c in p7["gamma_cases"]}
    grant = by_name["append_genesis"]["entry_jcs"]
    action = by_name["append_ok"]["entry_jcs"]
    concurrent = by_name["append_cas_stale"]["entry_jcs"]
    corrupted = by_name["append_bad_entry_signature"]["entry_jcs"]
    grant_head = gen_p.entry_head(grant)
    action_head = gen_p.entry_head(action)
    assert by_name["append_ok"]["state_heads"]["gamma"] == grant_head
    assert by_name["append_cas_stale"]["state_heads"]["gamma"] == action_head
    return {
        "grant_jcs": grant, "action_jcs": action,
        "concurrent_jcs": concurrent, "corrupted_jcs": corrupted,
        "grant_head": grant_head, "action_head": action_head,
    }


def genesis_fixture():
    """A brand-new DID for the genesis deposit: did:key-style, the DID
    literal IS the deposited root key. Deterministic fixture seeds — the
    P9 identity exists nowhere else."""
    root_sk = nacl.signing.SigningKey(b"\xcc" * 32)
    content_sk = nacl.signing.SigningKey(b"\xce" * 32)
    succ_sk = nacl.signing.SigningKey(b"\xcf" * 32)
    kex_pub = nacl.bindings.crypto_scalarmult_base(
        gen_p.derive("aithos-core/v1/owner-kex", b"\xcc" * 32))
    did = "did:aithos:" + gen_p.mb_ed(root_sk.verify_key.encode())
    doc = {
        "aithos-did-core": "1.0.0-draft.1",
        "id": did,
        "keys": {"root": gen_p.mb_ed(root_sk.verify_key.encode()),
                 "content": gen_p.mb_ed(content_sk.verify_key.encode()),
                 "kex": gen_p.mb_x(kex_pub),
                 "succession": gen_p.mb_ed(succ_sk.verify_key.encode())},
        "revocations": "gamma/gamma.jsonl",
        "bundle": [f"https://store.aithos.fr/t/acme/{did}"],
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    signed = gen_p.sign_doc(doc, root_sk)
    assert gen_p.verify_doc(signed, root_sk.verify_key.encode())
    return {"did": did, "did_json_jcs": gen_p.jcs(signed),
            "root_seed_hex": (b"\xcc" * 32).hex()}


def rotation_fixture(p1_did_json_jcs: str):
    """The successor document for the p1 DID: same id, rotated content
    key, signed under the STORED document's succession key (interim A.4
    reading — arbitrage named in the module docstring)."""
    stored = json.loads(p1_did_json_jcs)
    new_content = nacl.signing.SigningKey(b"\xdd" * 32)
    successor = json.loads(p1_did_json_jcs)
    successor["keys"]["content"] = gen_p.mb_ed(new_content.verify_key.encode())
    successor["signature"] = {"alg": "ed25519", "key": "#succession",
                              "value": ""}
    succ_signed = gen_p.sign_doc(successor, gen_p.succ_sk)
    assert gen_p.verify_doc(
        succ_signed, gen_p.succ_sk.verify_key.encode())
    assert stored["keys"]["succession"] == gen_p.mb_ed(
        gen_p.succ_sk.verify_key.encode()), "p1 succession key drifted"
    root_signed = json.loads(gen_p.jcs(succ_signed))
    root_signed["signature"] = {"alg": "ed25519", "key": "#root",
                                "value": ""}
    root_signed = gen_p.sign_doc(root_signed, gen_p.root_sk)
    return {"succession_signed_jcs": gen_p.jcs(succ_signed),
            "root_signed_jcs": gen_p.jcs(root_signed)}


def build():
    bundle, p7, p8v = load_frozen()
    p8 = bundle["packages"]["p8_cold"]
    objects = {k: v["utf8"] for k, v in p8["objects"].items()}
    facts = p8["cas_facts"]
    assert facts["new_manifest_head"] == p8v["new_manifest_head"]
    m1 = json.loads(objects["manifests/1.json"])
    m2 = json.loads(objects["manifests/2.json"])
    assert objects["manifest.json"] == objects["manifests/2.json"]
    manifest_head = facts["new_manifest_head"]
    height = facts["new_height"]
    assert height == 2

    gamma = gamma_fixture(p7)
    genesis = genesis_fixture()
    rotation = rotation_fixture(p7_did_json(p7))

    # The K1-C sidecar: recompute its digest from the frozen bytes — the
    # content-addressing check the redline engraves at deposit.
    changeset_path = next(k for k in objects if k.startswith("changesets/"))
    changeset_bytes = objects[changeset_path]
    assert commitment(CHANGESET_DOMAIN, changeset_bytes.encode()) == \
        "sha256:" + changeset_path.split("/")[1].removesuffix(".json"), \
        "frozen changeset digest does not recompute — vector rules broken"
    evidence_path = next(k for k in objects if k.startswith("evidence/"))
    public_alias = next(k for k in objects if k.startswith("public/sections/"))
    circle_alias = next(k for k in objects if k.startswith("circle/blobs/"))

    # /sync rule (frozen here): manifest.json first, then the
    # lexicographic diff of the pinned files maps held → current.
    def files_diff(held, current):
        return sorted(k for k, h in current.items()
                      if held.get(k) != h)
    sync_delta_parts = ["manifest.json"] + files_diff(m1["files"],
                                                      m2["files"])
    sync_current_parts = ["manifest.json"]

    base_heads = {"height": height, "manifest": manifest_head}
    gamma_seg = gamma["grant_jcs"] + "\n" + gamma["action_jcs"] + "\n"
    mandated_state = {
        "use_base_objects": True,
        "extra_objects": {SEGMENT_KEY: gamma_seg,
                          CIRCLE_BLOB: CIRCLE_BYTES,
                          SELF_BLOB: SELF_BYTES},
        "heads": {**base_heads, "gamma": gamma["action_head"],
                  "segment": SEGMENT},
    }
    owner_state = {
        "use_base_objects": True,
        "extra_objects": {CIRCLE_BLOB: CIRCLE_BYTES,
                          SELF_BLOB: SELF_BYTES},
        "heads": dict(base_heads),
    }
    # The enrollment fixture (p1 mandate cert) IS a stored object of every
    # case state — the listing names it like any stored path.
    enrollment_cert = "certs/mandate_0000000000000000000000P0M1.json"
    all_paths = sorted(set(objects) | {CIRCLE_BLOB, SELF_BLOB, enrollment_cert})
    heads_json = {"height": height, "manifest": manifest_head,
                  "gamma": None, "segment": None}
    heads_gamma_json = {"height": height, "manifest": manifest_head,
                        "gamma": gamma["action_head"], "segment": SEGMENT}

    def step(signer, method, path_rel, expect, query="", body_utf8=None,
             if_head=None):
        s = {"signer": signer, "method": method, "path_rel": path_rel,
             "expect": expect}
        if query:
            s["query"] = query
        if body_utf8 is not None:
            s["body_utf8"] = body_utf8
        if if_head is not None:
            s["if_head"] = if_head
        return s

    def accept_json(json_body):
        return {"status": "accept", "code": 200, "json": json_body}

    def accept_parts(parts):
        return {"status": "accept", "code": 200, "parts": parts}

    cases = []

    def case(name, group, state, steps, note):
        cases.append({"name": name, "group": group, "state": state,
                      "steps": steps, "note": note})

    # ---- heads -------------------------------------------------------
    case("heads_ok", "heads", owner_state,
         [step("owner_root", "GET", "heads", accept_json(heads_json))],
         "the two hot heads, the exact values the accepts served (A.5)")
    case("heads_mandated", "heads", mandated_state,
         [step("grantee", "GET", "heads", accept_json(heads_gamma_json))],
         "toute chaîne valide du DID serves /heads (A.3)")
    case("heads_anonymous", "heads", owner_state,
         [step("anonymous", "GET", "heads",
               {"status": 401, "error": "envelope_missing"})],
         "/heads is not in the anonymous A2 set")

    # ---- list --------------------------------------------------------
    case("list_owner", "list", owner_state,
         [step("owner_root", "GET", "", accept_json(
             {"paths": all_paths, "truncated": False}),
             query="?list=")],
         "the owner lists every stored path, lexicographic, one page")
    case("list_mandated_filtered", "list", mandated_state,
         [step("grantee", "GET", "", accept_json(
             {"paths": [CIRCLE_BLOB], "truncated": False}),
             query="?list=e/")],
         "read.circle: e/circle/** stays, e/self/** is filtered out — "
         "coarse perimeter filtering, never an error")
    case("list_paginated", "list", owner_state,
         [step("owner_root", "GET", "", accept_json(
             {"paths": all_paths[:2], "truncated": True}),
             query="?list=&limit=2"),
          step("owner_root", "GET", "", accept_json(
              {"paths": all_paths[2:4], "truncated": True}),
              query=f"?list=&after={all_paths[1]}&limit=2")],
         "after/limit paginate; a page continues exactly after `after`")
    case("list_limit_overflow", "list", owner_state,
         [step("owner_root", "GET", "",
               {"status": 413, "error": "payload_too_large"},
               query="?list=&limit=1001")],
         "A.8: listing ≤ 1000 paths/page — refused, never clamped")

    # ---- batch -------------------------------------------------------
    batch_body = json.dumps(
        {"paths": [circle_alias, CIRCLE_MISSING, SELF_BLOB]},
        separators=(",", ":"))
    case("batch_mixed", "batch", mandated_state,
         [step("grantee", "POST", "batch", accept_parts(
             [{"path": circle_alias, "part_status": 200,
               "body_utf8": objects[circle_alias]},
              {"path": CIRCLE_MISSING, "part_status": 404},
              {"path": SELF_BLOB, "part_status": 403}]),
             body_utf8=batch_body)],
         "one part per path, request order, body only on 200 (A.3)")
    overflow_body = json.dumps(
        {"paths": [CIRCLE_MISSING] * 257}, separators=(",", ":"))
    case("batch_overflow", "batch", owner_state,
         [step("owner_root", "POST", "batch",
               {"status": 413, "error": "payload_too_large"},
               body_utf8=overflow_body)],
         "A.8: batch ≤ 256 paths — the whole request fails")
    case("batch_bad_body", "batch", owner_state,
         [step("owner_root", "POST", "batch",
               {"status": 400, "error": "envelope_invalid"},
               body_utf8="not-json")],
         "a body outside the closed request form (named arbitrage: the "
         "A.7 registry stays closed, the wire form is part of A.2)")

    # ---- sync --------------------------------------------------------
    case("sync_delta", "sync", owner_state,
         [step("owner_root", "POST", "sync", accept_parts(
             [{"path": p, "part_status": 200,
               "body_utf8": objects[p]} for p in sync_delta_parts]),
             body_utf8=json.dumps({"have_edition": 1},
                                  separators=(",", ":")))],
         "manifest.json first, then the pinned files-map diff held→current")
    case("sync_current", "sync", owner_state,
         [step("owner_root", "POST", "sync", accept_parts(
             [{"path": "manifest.json", "part_status": 200,
               "body_utf8": objects["manifest.json"]}]),
             body_utf8=json.dumps({"have_edition": 2},
                                  separators=(",", ":")))],
         "nothing changed: the pack still opens with the tip")
    gone_state = {**owner_state, "drop_objects": ["manifests/1.json"]}
    case("sync_gone", "sync", gone_state,
         [step("owner_root", "POST", "sync",
               {"status": 410, "error": "edition_gone"},
               body_utf8=json.dumps({"have_edition": 1},
                                    separators=(",", ":")))],
         "the held edition slot is purged: full resync (A.3)")

    # ---- redline reads ----------------------------------------------
    case("get_alias_public_anonymous", "redline", owner_state,
         [step("anonymous", "GET", public_alias,
               {"status": "accept", "code": 200,
                "body_utf8": objects[public_alias]})],
         "public/sections/** is the K1-C alias of the public zone: A2")
    case("get_alias_circle_mandated", "redline", mandated_state,
         [step("grantee", "GET", circle_alias,
               {"status": "accept", "code": 200,
                "body_utf8": objects[circle_alias]})],
         "the frozen p8 read plan: read.circle covers the blob alias")
    case("get_sidecars_chain", "redline", mandated_state,
         [step("grantee", "GET", changeset_path,
               {"status": "accept", "code": 200,
                "body_utf8": objects[changeset_path]}),
          step("grantee", "GET", "manifests/1.json",
               {"status": "accept", "code": 200,
                "body_utf8": objects["manifests/1.json"]})],
         "toute chaîne valide: public proof material, cold verify")
    case("get_alias_denied", "redline", mandated_state,
         [step("grantee", "GET", SELF_BLOB,
               {"status": 403, "error": "not_covered"})],
         "the frozen p8 read-plan denial, unchanged by the redline")
    case("get_internal_key", "redline", owner_state,
         [step("owner_root", "GET", "manifests/tree-2.json",
               {"status": 400, "error": "path_invalid"})],
         "bundle-internal keys never enter the wire grammar")
    case("put_manifest_slot_denied", "redline", owner_state,
         [step("owner_root", "PUT", "manifests/3.json",
               {"status": 403, "error": "not_covered"},
               body_utf8="{}")],
         "in the grammar, no write line: the slot is server-written only")
    pre_publish_state = {
        "use_base_objects": False,
        "extra_objects": {"did.json": objects["did.json"],
                          "manifest.json": objects["manifests/1.json"],
                          "manifests/1.json": objects["manifests/1.json"]},
        "heads": {"height": 1,
                  "manifest": "sha256:" + gen_p.manifest_chain_hash(m1)},
    }
    assert pre_publish_state["heads"]["manifest"] == \
        facts["expected_predecessors"][0]
    case("put_changeset_ok", "redline", pre_publish_state,
         [step("owner_root", "PUT", changeset_path,
               {"status": "accept", "code": 204},
               body_utf8=changeset_bytes)],
         "a sidecar deposits under its own K1-C digest before the publish")
    wrong_path = "changesets/" + "0" * 64 + ".json"
    case("put_changeset_id_mismatch", "redline", pre_publish_state,
         [step("owner_root", "PUT", wrong_path,
               {"status": 400, "error": "artifact_invalid",
                "reason": "id_mismatch"},
               body_utf8=changeset_bytes)],
         "content-addressing is the path's definition (redline gate 5)")

    # ---- did.json ----------------------------------------------------
    genesis_state = {"use_base_objects": False, "extra_objects": {},
                     "heads": None, "bind_did": genesis["did"]}
    case("did_genesis_ok", "did", genesis_state,
         [step("genesis_owner", "PUT", "did.json",
               {"status": "accept", "code": 204},
               body_utf8=genesis["did_json_jcs"])],
         "genesis: the #root envelope verifies under the DEPOSITED "
         "document's root key; the control plane binds the DID (A.4)")
    case("did_genesis_id_mismatch", "did", genesis_state,
         [step("genesis_foreign_doc", "PUT", "did.json",
               {"status": 400, "error": "artifact_invalid",
                "reason": "id_mismatch"},
               body_utf8=p7_did_json(p7))],
         "the deposited document names a foreign DID: id != path DID")
    case("did_genesis_wrong_signer", "did", genesis_state,
         [step("genesis_wrong_signer", "PUT", "did.json",
               {"status": 401, "error": "signature_invalid"},
               body_utf8=genesis["did_json_jcs"])],
         "the genesis exception resolves #root against the deposited "
         "document: any other signer fails A.2 #8")
    p1_state = {"use_base_objects": False, "extra_objects": {},
                "heads": None}
    case("did_rotation_ok", "did", p1_state,
         [step("owner_root", "PUT", "did.json",
               {"status": "accept", "code": 204},
               body_utf8=rotation["succession_signed_jcs"])],
         "a replacement verifies under the STORED succession key "
         "(interim A.4 reading — named arbitrage)")
    case("did_rotation_root_signer", "did", p1_state,
         [step("owner_root", "PUT", "did.json",
               {"status": 400, "error": "artifact_invalid",
                "reason": "signature"},
               body_utf8=rotation["root_signed_jcs"])],
         "a stolen root can never steal the identity's future (§01.4)")

    # ---- gamma segment replica --------------------------------------
    def replica_state(segment_utf8, gamma_head):
        return {"use_base_objects": True,
                "extra_objects": {SEGMENT_KEY: segment_utf8},
                "heads": {**base_heads, "gamma": gamma_head,
                          "segment": SEGMENT}}

    grant_seg = gamma["grant_jcs"] + "\n"
    case("replica_extend_ok", "replica",
         replica_state(grant_seg, gamma["grant_head"]),
         [step("owner_root", "PUT", SEGMENT_KEY,
               {"status": "accept", "code": 200,
                "json": {"head": gamma["action_head"]}},
               body_utf8=grant_seg + gamma["action_jcs"] + "\n",
               if_head=gamma["grant_head"])],
         "prefix preserved + one verified entry: the head advances (A.5)")
    case("replica_cas_required", "replica",
         replica_state(grant_seg, gamma["grant_head"]),
         [step("owner_root", "PUT", SEGMENT_KEY,
               {"status": 428, "error": "cas_required"},
               body_utf8=grant_seg + gamma["action_jcs"] + "\n")],
         "the replica follows the same CAS rule (A.5)")
    case("replica_cas_stale", "replica",
         replica_state(gamma_seg, gamma["action_head"]),
         [step("owner_root", "PUT", SEGMENT_KEY,
               {"status": 409, "error": "cas_mismatch",
                "head": gamma["action_head"]},
               body_utf8=grant_seg + gamma["concurrent_jcs"] + "\n",
               if_head=gamma["grant_head"])],
         "the loser gets the current segment head back and rebases")
    case("replica_not_prefix", "replica",
         replica_state(grant_seg, gamma["grant_head"]),
         [step("owner_root", "PUT", SEGMENT_KEY,
               {"status": 400, "error": "artifact_invalid",
                "reason": "prefix_mismatch"},
               body_utf8=gamma["action_jcs"] + "\n",
               if_head=gamma["grant_head"])],
         "a replica never rewrites history: byte-exact prefix or refusal "
         "(prefix_mismatch — the ONE new A.7 reason, named at the gate)")
    case("replica_bad_added_entry", "replica",
         replica_state(grant_seg, gamma["grant_head"]),
         [step("owner_root", "PUT", SEGMENT_KEY,
               {"status": 400, "error": "artifact_invalid",
                "reason": "entry_signature"},
               body_utf8=grant_seg + gamma["corrupted_jcs"] + "\n",
               if_head=gamma["grant_head"])],
         "every added entry is verified A.4 — CAS passes, the entry fails")

    vector = {
        "vector": "P9",
        "description": (
            "Store reads + remaining writes (gate contrat étape 5, P2): "
            "GET /heads, ?list=, POST /batch, POST /sync (A.3), the "
            "draft.2 servable layout of the redline gate 5 2026-07-20 "
            "(manifests/<h>.json, changesets/, evidence/, K1-C aliases), "
            "PUT did.json (genesis under the deposited root key + "
            "succession replacement, A.4) and the gamma segment replica "
            "(byte-exact prefix + per-entry A.4 + segment-head CAS, "
            "A.5). Base state = the FROZEN p8_cold package of "
            "p7-bundle-packages.json (aithos-bundle export_keyless); "
            "the gamma fixtures are the committed p7 entries; the p9 "
            "genesis identity is did:key-style from fixture seeds. "
            "Named arbitrages: did.json replacement under the stored "
            "succession key (interim §01.4 reading, §10.4 epoch artifact "
            "open); malformed batch/sync body → envelope_invalid; sync "
            "pack = manifest.json + pinned files-map diff; "
            "prefix_mismatch is the one new artifact_invalid reason."),
        "tenant": p7["tenant"],
        "did": p7["did"],
        "bundle_packages": "p7-bundle-packages.json",
        "base_package": "p8_cold",
        "at": AT,
        "fixtures": {
            "genesis": genesis,
            "rotation": rotation,
            "gamma": gamma,
            "blobs": {CIRCLE_BLOB: CIRCLE_BYTES, SELF_BLOB: SELF_BYTES},
            "enrollment_cert": enrollment_cert,
        },
        "cases": cases,
    }
    return vector


def p7_did_json(p7):
    p1 = json.load(open("p1-store-envelope.json"))
    assert p1["did"] == p7["did"]
    return p1["did_json_jcs"]


def main():
    vector = build()
    out = json.dumps(vector, indent=1, ensure_ascii=False, sort_keys=True)
    with open("p9-store-reads.json", "w") as f:
        f.write(out + "\n")
    n_cases = len(vector["cases"])
    n_steps = sum(len(c["steps"]) for c in vector["cases"])
    print(f"P9 written: {n_cases} cases, {n_steps} wire steps "
          f"(base = frozen p8_cold, gamma = committed p7 entries).")


if __name__ == "__main__":
    main()

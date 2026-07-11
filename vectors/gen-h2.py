#!/usr/bin/env python3
"""Independent generator for the H2 conformance vector (spec 07.10):
committed gamma roots — per-segment roots (chain order, root+n), the
counts trie (entries/actions/children/budgets per mandate, leaves sorted
by mandate id), count proofs, sorted-adjacency absence proofs, and the
withhold negatives.

Second-implementation rule: every expected value computed with Python
blake3 + hashlib, never by the Rust reference. Triple-anchored against
committed vectors before emitting:
  - B2: blake3 derive drift check (same as gen-h.py);
  - H1: the circle zone root is recomputed from h1-merkle.json's rows and
    must land byte-identical (proves mroot/H_leaf/H_node conventions);
  - F2: segment 2026-07 IS f2-gamma-counting.json's committed entries
    (hashed on their exact committed bytes), and this generator's raw
    tallies must reproduce F2's committed expected counts (proves the
    counting semantics feeding the trie).

Month-2 entries are fixture-shaped (real chain via prev, fixture
signatures — root math pins bytes; signature validity is F1/F2's job,
same posture as H1's fixture wraps).

Usage: python3 gen-h2.py   (from vectors/)
"""

import hashlib
import json

import blake3

ZEROS = b"\x00" * 32
LEAF_DOMAIN = b"aithos-core/v1/mk-leaf\x00"
NODE_DOMAIN = b"aithos-core/v1/mk-node\x00"


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def b3(data: bytes) -> bytes:
    return blake3.blake3(data).digest()


def h_leaf(payload: bytes) -> bytes:
    return b3(LEAF_DOMAIN + payload)


def h_node(left: bytes, right: bytes) -> bytes:
    return b3(NODE_DOMAIN + left + right)


def mroot(hashes: list) -> bytes:
    if not hashes:
        return ZEROS
    if len(hashes) == 1:
        return hashes[0]
    mid = (len(hashes) + 1) // 2  # left-heavy ceil split
    return h_node(mroot(hashes[:mid]), mroot(hashes[mid:]))


def mroot_path(hashes: list, idx: int) -> list:
    """Sibling steps carrying leaf idx to mroot(hashes) — innermost first."""
    if len(hashes) <= 1:
        return []
    mid = (len(hashes) + 1) // 2
    if idx < mid:
        return mroot_path(hashes[:mid], idx) + [
            {"node": {"side": "right", "hash": mroot(hashes[mid:]).hex()}}
        ]
    return mroot_path(hashes[mid:], idx - mid) + [
        {"node": {"side": "left", "hash": mroot(hashes[:mid]).hex()}}
    ]


def run_proof(start: bytes, steps: list) -> bytes:
    cur = start
    for s in steps:
        if "node" in s:
            sib = bytes.fromhex(s["node"]["hash"])
            cur = h_node(cur, sib) if s["node"]["side"] == "right" else h_node(sib, cur)
        else:
            cur = h_leaf(
                bytes.fromhex(s["wrap"]["pre"]) + cur + bytes.fromhex(s["wrap"]["post"])
            )
    return cur


def sha256_prev(entry_jcs: str) -> str:
    return "sha256:" + hashlib.sha256(entry_jcs.encode()).hexdigest()


# ------------------------------------------------------------ self-checks

def self_check_b2():
    b2 = json.load(open("b2-derivation.json"))
    zk = bytes.fromhex(b2["zone_dk_hex"])
    k = blake3.blake3(
        zk, derive_key_context=f"aithos-core/v1/d/{b2['folder_sids'][0]}"
    ).digest()
    assert k.hex() == b2["folder1_key_hex"], "blake3 drift vs committed B2"


def self_check_h1():
    """Recompute H1's committed circle root from its committed rows."""
    h1 = json.load(open("h1-merkle.json"))
    rows, tree = h1["rows"], h1["tree"]
    header_f = b3(jcs(rows["header_f"]).encode())
    leaf7 = h_leaf(jcs(rows["s7"]).encode() + ZEROS)
    leaf8 = h_leaf(jcs(rows["s8"]).encode() + ZEROS)
    leaf9 = h_leaf(jcs(rows["s9"]).encode() + ZEROS)
    folder1 = h_leaf(jcs(rows["folder"]).encode() + header_f + mroot([leaf7, leaf8]))
    wrap_leaf = h_leaf(
        rows["s7"]["sid"].encode() + b"\x00" + b3(jcs(rows["wrap7"]).encode())
    )
    tag_view = h_leaf(b"t/urgent" + ZEROS + mroot([wrap_leaf]))
    zone_root = h_leaf(b"z/circle" + ZEROS + mroot([folder1, leaf9, tag_view]))
    assert zone_root.hex() == tree["circle_root_hex"], "mroot conventions drift vs H1"


def tally(entries: list) -> dict:
    """Raw §7.4/§04.11 tallies → the trie counters, omission rules applied."""
    per = {}

    def bucket(mid):
        return per.setdefault(
            mid, {"entries": 0, "actions": 0, "children": 0, "budgets": {}}
        )

    for e in entries:
        via = e.get("authorized_via") or []
        pay = e.get("payload") or {}
        for mid in set(via):
            b = bucket(mid)
            b["entries"] += 1
            if e["kind"] == "action":
                b["actions"] += 1
            if e["kind"] in ("action", "inference") and "budget_ref" in pay:
                ref = pay["budget_ref"]
                slot = b["budgets"].setdefault(ref, {"actions": 0, "tokens": 0})
                if e["kind"] == "action":
                    slot["actions"] += 1
                # post-override semantics: attested receipt tokens beat declarations
                if isinstance(pay.get("receipt"), dict) and "tokens" in pay["receipt"]:
                    slot["tokens"] += pay["receipt"]["tokens"]
                else:
                    slot["tokens"] += (
                        pay.get("tokens", 0)
                        + pay.get("tokens_in", 0)
                        + pay.get("tokens_out", 0)
                    )
        if e["kind"] == "grant" and e.get("authorized_by"):
            bucket(e["authorized_by"])["children"] += 1

    out = {}
    for mid, c in per.items():
        counters = {"entries": c["entries"]}
        if c["actions"]:
            counters["actions"] = c["actions"]
        if c["children"]:
            counters["children"] = c["children"]
        budgets = {
            r: {k: v for k, v in slot.items() if v} for r, slot in c["budgets"].items()
        }
        budgets = {r: s for r, s in budgets.items() if s}
        if budgets:
            counters["budgets"] = budgets
        if counters["entries"] == 0:
            del counters["entries"]
        if counters:
            out[mid] = counters
    return out


def self_check_f2(f2, tallies):
    exp = f2["expected"]
    root = json.loads(f2["root_mandate_jcs"])["id"]
    leaf = json.loads(f2["leaf_mandate_jcs"])["id"]
    assert tallies[root]["actions"] == exp["actions_via_root"], "F2 root actions drift"
    assert tallies[leaf]["actions"] == exp["actions_via_leaf"], "F2 leaf actions drift"
    assert tallies[root]["children"] == exp["children_of_root"], "F2 children drift"


# --------------------------------------------------------------- fixture

def month2_entries(f2, root_id, leaf_id, m23_id):
    """Three 2026-08 entries chained from F2's committed gamma_head."""
    head = f2["gamma_head"]
    assert head == sha256_prev(f2["entries_jcs"][-1]), "F2 gamma_head convention drift"

    def sig(tag):
        return {"alg": "ed25519", "key": "fixture", "value": f"fixture-sig-{tag}"}

    e5 = {
        "v": 1, "id": "gamma_00000000000000000000000H25", "prev": head,
        "at": "2026-08-01T00:00:00Z", "kind": "grant", "target": m23_id,
        "authorized_by": leaf_id, "authorized_via": [root_id, leaf_id],
        "payload": {"mandate": m23_id}, "signature": sig("e5"),
    }
    e6 = {
        "v": 1, "id": "gamma_00000000000000000000000H26", "prev": sha256_prev(jcs(e5)),
        "at": "2026-08-01T01:00:00Z", "kind": "inference", "target": "x.llm",
        "authorized_by": leaf_id, "authorized_via": [root_id, leaf_id],
        "payload": {"provider": "anthropic", "model": "claude-haiku",
                    "tokens_in": 1200, "tokens_out": 300, "budget_ref": "haiku"},
        "signature": sig("e6"),
    }
    e7 = {
        "v": 1, "id": "gamma_00000000000000000000000H27", "prev": sha256_prev(jcs(e6)),
        "at": "2026-08-01T02:00:00Z", "kind": "action", "target": "x.gmail",
        "authorized_by": m23_id, "authorized_via": [root_id, leaf_id, m23_id],
        "payload": {"action": "reply", "args_hash": "sha256:" + "dd" * 32,
                    "budget_ref": "haiku", "tokens": 999,
                    "receipt": {"args_hash": "sha256:" + "dd" * 32,
                                "model": "claude-haiku", "tokens": 2700,
                                "sig": "fixture-receipt-sig"}},
        "signature": sig("e7"),
    }
    return [jcs(e5), jcs(e6), jcs(e7)]


def build():
    f2 = json.load(open("f2-gamma-counting.json"))
    root_id = json.loads(f2["root_mandate_jcs"])["id"]
    leaf_id = json.loads(f2["leaf_mandate_jcs"])["id"]
    ghost_id = json.loads(f2["ghost_mandate_jcs"])["id"]  # never logged — never counted
    m23_id = "mandate_00000000000000000000000023"

    seg_jul = list(f2["entries_jcs"])  # committed bytes, hashed verbatim
    seg_aug = month2_entries(f2, root_id, leaf_id, m23_id)

    # ---- segment roots: chain order, H_leaf over the exact entry bytes
    jul_hashes = [h_leaf(e.encode()) for e in seg_jul]
    aug_hashes = [h_leaf(e.encode()) for e in seg_aug]
    gamma_roots = {
        "2026-07": {"root": mroot(jul_hashes).hex(), "n": len(seg_jul)},
        "2026-08": {"root": mroot(aug_hashes).hex(), "n": len(seg_aug)},
    }

    # ---- counts trie
    # F2's committed expected counts hold over F2's entries alone
    self_check_f2(f2, tally([json.loads(e) for e in seg_jul]))
    tallies = tally([json.loads(e) for e in seg_jul + seg_aug])
    # cross-checks the spec example numbers (§7.10)
    assert tallies[root_id] == {"entries": 7, "actions": 4, "children": 1,
                                "budgets": {"haiku": {"actions": 1, "tokens": 4200}}}
    assert tallies[leaf_id] == {"entries": 5, "actions": 3, "children": 1,
                                "budgets": {"haiku": {"actions": 1, "tokens": 4200}}}
    assert tallies[m23_id] == {"entries": 1, "actions": 1,
                               "budgets": {"haiku": {"actions": 1, "tokens": 2700}}}
    assert ghost_id not in tallies

    sorted_ids = sorted(tallies)  # …20 < …21 < …23, byte order
    assert sorted_ids == [root_id, leaf_id, m23_id]
    trie_leaves = [
        h_leaf(mid.encode() + b"\x00" + jcs(tallies[mid]).encode()) for mid in sorted_ids
    ]
    counts_root = mroot(trie_leaves)

    # ---- entry inclusion proof: e3 (2nd reply) in 2026-07, index 2
    e3_proof = {
        "payload": seg_jul[2].encode().hex(),
        "steps": mroot_path(jul_hashes, 2),
        "root": gamma_roots["2026-07"]["root"],
    }
    assert run_proof(h_leaf(seg_jul[2].encode()), e3_proof["steps"]).hex() == e3_proof["root"]
    # tampered bytes die
    bad = json.loads(seg_jul[2]); bad["payload"]["action"] = "send"
    assert run_proof(h_leaf(jcs(bad).encode()), e3_proof["steps"]).hex() != e3_proof["root"]

    # ---- count proof: leaf mandate, trie index 1
    leaf_payload = leaf_id.encode() + b"\x00" + jcs(tallies[leaf_id]).encode()
    count_proof = {
        "payload": leaf_payload.hex(),
        "steps": mroot_path(trie_leaves, 1),
        "root": counts_root.hex(),
    }
    assert run_proof(h_leaf(leaf_payload), count_proof["steps"]).hex() == counts_root.hex()

    # ---- absence proof for the ghost: adjacent leaves 1 (leaf_id) and 2 (m23)
    absence = {
        "absent_id": ghost_id,
        "left": {"payload": leaf_payload.hex(), "steps": mroot_path(trie_leaves, 1)},
        "right": {
            "payload": (m23_id.encode() + b"\x00" + jcs(tallies[m23_id]).encode()).hex(),
            "steps": mroot_path(trie_leaves, 2),
        },
        "root": counts_root.hex(),
    }
    # both replay to the root; ids bracket the ghost
    for side in ("left", "right"):
        p = absence[side]
        assert run_proof(h_leaf(bytes.fromhex(p["payload"])), p["steps"]).hex() == counts_root.hex()
    assert leaf_id < ghost_id < m23_id
    # adjacency on the 3-leaf shape: divergence at the root level —
    # left's top sibling replays from right's lower steps, and vice versa
    lp, rp = absence["left"]["steps"], absence["right"]["steps"]
    l_leaf = h_leaf(bytes.fromhex(absence["left"]["payload"]))
    r_leaf = h_leaf(bytes.fromhex(absence["right"]["payload"]))
    assert lp[-1]["node"]["side"] == "right" and rp[-1]["node"]["side"] == "left"
    assert bytes.fromhex(lp[-1]["node"]["hash"]) == run_proof(r_leaf, rp[:-1])
    assert bytes.fromhex(rp[-1]["node"]["hash"]) == run_proof(l_leaf, lp[:-1])
    assert all(s["node"]["side"] == "left" for s in lp[:-1])   # rightmost of left subtree
    assert all(s["node"]["side"] == "right" for s in rp[:-1])  # leftmost of right subtree
    # forged absence: outer leaves (indices 0 and 2) are NOT adjacent —
    # index 0's below-divergence steps are not all side:"left"
    forged_lp = mroot_path(trie_leaves, 0)
    assert not all(s["node"]["side"] == "left" for s in forged_lp[:-1]), "forgery must be detectable"

    # ---- withhold negatives
    assert mroot(jul_hashes[:3]).hex() != gamma_roots["2026-07"]["root"], "segment omission dies"
    served_actions_for_leaf = 2  # mirror withholds one of the three
    assert served_actions_for_leaf != tallies[leaf_id]["actions"], "count exposes the withhold"

    return {
        "segments": {"2026-07": seg_jul, "2026-08": seg_aug},
        "gamma_roots": gamma_roots,
        "counts": {mid: tallies[mid] for mid in sorted_ids},
        "counts_leaves_hex": [h.hex() for h in trie_leaves],
        "gamma_counts_root_hex": counts_root.hex(),
        "empty_counts_root_hex": ZEROS.hex(),
        "proof_entry_e3": e3_proof,
        "proof_count_leaf_mandate": count_proof,
        "proof_absence_ghost": absence,
        "negatives": {
            "tampered_action_name": "send",
            "expected": "root mismatch (fail-closed)",
            "forged_absence_pair": [0, 2],
            "withheld_segment_prefix_differs": True,
        },
    }


if __name__ == "__main__":
    self_check_b2()
    self_check_h1()
    out = {
        "vector": "H2",
        "description": "Committed gamma roots (spec 07.10): per-segment roots in "
                       "chain order (root+n), the counts trie (entries/actions/"
                       "children/budgets, attested tokens beating declarations, "
                       "leaves sorted by mandate id), a v1 entry inclusion proof, "
                       "a count proof, a sorted-adjacency absence proof, and the "
                       "withhold negatives. Segment 2026-07 is F2's committed "
                       "entries hashed on their exact bytes; tallies reproduce "
                       "F2's committed expected counts. Generated independently "
                       "(Python blake3/hashlib); self-checked against B2, H1, F2.",
        "tree": build(),
    }
    with open("h2-gamma-roots.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("self-checks vs B2 + H1 + F2 passed; wrote h2-gamma-roots.json")

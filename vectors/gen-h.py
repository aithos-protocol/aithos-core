#!/usr/bin/env python3
"""Independent generator for the H1 conformance vector (spec 02.10):
Merkle state roots — H_leaf/H_node domain separation, mroot (left-heavy
ceil split), node composition (section / folder / tag view / zone root /
flat zones), a v1 inclusion proof, and the splice negatives.

Second-implementation rule: every expected value computed with Python
blake3 + hashlib, never by the Rust reference. Self-validates against the
committed B2 vector (same blake3 derive path) before emitting.

Conventions graved 2026-07-11 (spec 02.10 amendment, commits bef83ab +
5e3e222): mroot left = first ceil(n/2); child sort d < s < t then sid/tag;
tag wraps H_leaf(sid NUL blake3(JCS(wrap))); zone root literal label
"z/<zone>"; flat zones root = mroot(leaves); proof wire v1 = claimed bytes
then node/wrap steps.

Usage: python3 gen-h.py   (from vectors/)
"""

import json

import blake3

ZEROS = b"\x00" * 32
LEAF_DOMAIN = b"aithos-core/v1/mk-leaf\x00"
NODE_DOMAIN = b"aithos-core/v1/mk-node\x00"

CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"


def ulid(n: int) -> str:
    out = []
    for _ in range(26):
        out.append(CROCKFORD[n & 31])
        n >>= 5
    return "".join(reversed(out))


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


# ------------------------------------------------------------- self-check
# Same blake3 as the committed B2 vector: recompute folder1's derive key.

def self_check():
    b2 = json.load(open("b2-derivation.json"))
    zk = bytes.fromhex(b2["zone_dk_hex"])
    k = blake3.blake3(
        zk, derive_key_context=f"aithos-core/v1/d/{b2['folder_sids'][0]}"
    ).digest()
    assert k.hex() == b2["folder1_key_hex"], "blake3 drift vs committed B2"


# --------------------------------------------------------------- fixture
# circle zone:
#   z/circle (no header)
#   ├── d sid1 "notes"    (granted → header folded)
#   │   ├── s sid7 "note1"
#   │   └── s sid8 "note2"
#   ├── s sid9 "loose"
#   └── t "urgent"        (one wrap, for sid7)
# self zone: three flat rows.

SID1, SID7, SID8, SID9 = ulid(1), ulid(7), ulid(8), ulid(9)

ROW_F = {"sid": SID1, "name": "notes", "parent_sid": None}
ROW_7 = {"sid": SID7, "name": "note1", "folder_sid": SID1, "title": "Note 1",
         "tags": ["urgent"], "blob_sha": "aa" * 32, "key_version": 1}
ROW_8 = {"sid": SID8, "name": "note2", "folder_sid": SID1, "title": "Note 2",
         "tags": [], "blob_sha": "bb" * 32, "key_version": 1}
ROW_9 = {"sid": SID9, "name": "loose", "folder_sid": None, "title": "Loose",
         "tags": [], "blob_sha": "cc" * 32, "key_version": 1}
HEADER_F = {"v": 2, "lines": ["granted-line-fixture"]}
WRAP_7 = {"sid": SID7, "wrap": "wrap-fixture-bytes"}
SELF_ROWS = [{"sid": ulid(11), "key_version": 1},
             {"sid": ulid(12), "key_version": 3},
             {"sid": ulid(13), "key_version": 1}]


def build():
    header_f = b3(jcs(HEADER_F).encode())

    leaf7 = h_leaf(jcs(ROW_7).encode() + ZEROS)
    leaf8 = h_leaf(jcs(ROW_8).encode() + ZEROS)
    leaf9 = h_leaf(jcs(ROW_9).encode() + ZEROS)

    folder_children = mroot([leaf7, leaf8])  # both kind s, sid7 < sid8
    folder1 = h_leaf(jcs(ROW_F).encode() + header_f + folder_children)

    wrap_leaf = h_leaf(SID7.encode() + b"\x00" + b3(jcs(WRAP_7).encode()))
    tag_view = h_leaf(b"t/urgent" + ZEROS + mroot([wrap_leaf]))

    # root children sorted (kind, key): d sid1 < s sid9 < t urgent
    root_children = mroot([folder1, leaf9, tag_view])  # odd: ((f,9),t)
    assert root_children == h_node(h_node(folder1, leaf9), tag_view), "left-heavy"
    zone_root = h_leaf(b"z/circle" + ZEROS + root_children)

    self_leaves = sorted(
        (r["sid"], h_leaf(jcs(r).encode() + ZEROS)) for r in SELF_ROWS
    )
    self_root = mroot([h for _, h in self_leaves])

    # ---- v1 proof for section sid7, starting from the claimed bytes
    proof = [
        {"node": {"side": "right", "hash": leaf8.hex()}},
        {"wrap": {"pre": (jcs(ROW_F).encode() + header_f).hex(), "post": ""}},
        {"node": {"side": "right", "hash": leaf9.hex()}},
        {"node": {"side": "right", "hash": tag_view.hex()}},
        {"wrap": {"pre": b"z/circle".hex() + ZEROS.hex(), "post": ""}},
    ]

    def run(start: bytes, steps) -> bytes:
        cur = start
        for s in steps:
            if "node" in s:
                sib = bytes.fromhex(s["node"]["hash"])
                cur = h_node(cur, sib) if s["node"]["side"] == "right" else h_node(sib, cur)
            else:
                cur = h_leaf(bytes.fromhex(s["wrap"]["pre"]) + cur
                             + bytes.fromhex(s["wrap"]["post"]))
        return cur

    assert run(leaf7, proof) == zone_root, "the proof must replay to the root"
    # tampered row: same steps, altered title → dead root
    row7_tampered = dict(ROW_7, title="Note 1 (tampered)")
    assert run(h_leaf(jcs(row7_tampered).encode() + ZEROS), proof) != zone_root
    # splice negatives: the domains are the defense
    assert h_leaf(leaf7 + leaf8) != h_node(leaf7, leaf8), "leaf-as-node splice"
    bad = [dict(s) for s in proof]
    bad[0] = {"wrap": {"pre": "", "post": leaf8.hex()}}  # H_leaf where H_node belongs
    assert run(leaf7, bad) != zone_root, "hleaf-for-hnode splice must die"

    return {
        "header_f_hash_hex": header_f.hex(),
        "leaves": {"s7": leaf7.hex(), "s8": leaf8.hex(), "s9": leaf9.hex(),
                   "wrap7": wrap_leaf.hex(), "tag_urgent": tag_view.hex(),
                   "folder1": folder1.hex()},
        "folder_children_mroot_hex": folder_children.hex(),
        "root_children_mroot_hex": root_children.hex(),
        "circle_root_hex": zone_root.hex(),
        "self_root_hex": self_root.hex(),
        "empty_root_hex": ZEROS.hex(),
        "proof_s7": proof,
        "negatives": {
            "tampered_title": "Note 1 (tampered)",
            "expected": "root mismatch (fail-closed)",
            "leaf_as_node_differs": True,
        },
    }


if __name__ == "__main__":
    self_check()
    out = {
        "vector": "H1",
        "description": "Merkle state roots (spec 02.10): BLAKE3 domain-separated "
                       "H_leaf/H_node, left-heavy mroot, node composition for "
                       "section/folder/tag-view/zone-root/flat zones, v1 proof "
                       "wire replayed to the root, tamper + splice negatives. "
                       "Generated independently (Python blake3); self-checked "
                       "against the committed B2 derive.",
        "rows": {"folder": ROW_F, "s7": ROW_7, "s8": ROW_8, "s9": ROW_9,
                 "header_f": HEADER_F, "wrap7": WRAP_7, "self_rows": SELF_ROWS},
        "tree": build(),
    }
    with open("h1-merkle.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("self-check vs B2 passed; wrote h1-merkle.json")

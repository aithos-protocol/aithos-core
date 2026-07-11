#!/usr/bin/env python3
"""Independent generator for the I conformance vector (spec 02.6 + 07.6):
concurrency — deterministic disjoint merge (parent ordering by ascending
edition hash, prev_hash = the lowest), 3-way index merge by sid (union,
deletions hold, same-sid = fork), the two-predecessor merge entry, the
merged segment layout (sub-chain A then B then merge, entries
byte-identical), recommitted gamma roots, and the conflict negative.

Second-implementation rule: every expected value computed with Python
blake3 + hashlib, never by the Rust reference. Anchored on committed
vectors before emitting:
  - B2: blake3 derive drift check;
  - H2: the ancestor segment IS h2-gamma-roots.json's committed 2026-07
    segment (= F2's committed entries); its root and n must land
    byte-identical to H2's committed values (proves segment conventions).

Pinned-by-vector convention (impl follows): the merge entry's clear
payload is {"merges": [hash_a, hash_b]} — the ascending edition hashes it
joins, mirroring the manifest's merges field.

Usage: python3 gen-i.py   (from vectors/)
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


def h_leaf(p: bytes) -> bytes:
    return b3(LEAF_DOMAIN + p)


def h_node(l: bytes, r: bytes) -> bytes:
    return b3(NODE_DOMAIN + l + r)


def mroot(hashes: list) -> bytes:
    if not hashes:
        return ZEROS
    if len(hashes) == 1:
        return hashes[0]
    mid = (len(hashes) + 1) // 2
    return h_node(mroot(hashes[:mid]), mroot(hashes[mid:]))


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def entry_head(entry_jcs: str) -> str:
    return "sha256:" + sha256_hex(entry_jcs.encode())


def manifest_chain_hash(manifest: dict) -> str:
    m = json.loads(jcs(manifest))
    m["signature"]["value"] = ""
    return sha256_hex(jcs(m).encode())


# ------------------------------------------------------------ self-checks

def self_check_b2():
    b2 = json.load(open("b2-derivation.json"))
    zk = bytes.fromhex(b2["zone_dk_hex"])
    k = blake3.blake3(
        zk, derive_key_context=f"aithos-core/v1/d/{b2['folder_sids'][0]}"
    ).digest()
    assert k.hex() == b2["folder1_key_hex"], "blake3 drift vs committed B2"


def self_check_h2(h2):
    seg = h2["tree"]["segments"]["2026-07"]
    root = mroot([h_leaf(e.encode()) for e in seg])
    committed = h2["tree"]["gamma_roots"]["2026-07"]
    assert root.hex() == committed["root"], "segment conventions drift vs H2"
    assert len(seg) == committed["n"], "segment count drift vs H2"


# --------------------------------------------------------------- fixture

def build():
    h2 = json.load(open("h2-gamma-roots.json"))
    self_check_h2(h2)
    f2 = json.load(open("f2-gamma-counting.json"))
    ancestor = list(f2["entries_jcs"])  # committed bytes — the shared prefix
    h0 = f2["gamma_head"]
    assert h0 == entry_head(ancestor[-1]), "F2 gamma_head convention drift"
    root_id = json.loads(f2["root_mandate_jcs"])["id"]
    leaf_id = json.loads(f2["leaf_mandate_jcs"])["id"]

    def sig(tag):
        return {"alg": "ed25519", "key": "fixture", "value": f"fixture-sig-{tag}"}

    # ---- two sub-chains extending the same tip h0
    a5 = {"v": 1, "id": "gamma_00000000000000000000000IA5", "prev": h0,
          "at": "2026-07-04T00:00:00Z", "kind": "action", "target": "x.gmail",
          "authorized_by": leaf_id, "authorized_via": [root_id, leaf_id],
          "payload": {"action": "reply", "args_hash": "sha256:" + "aa" * 32},
          "signature": sig("a5")}
    b5 = {"v": 1, "id": "gamma_00000000000000000000000IB5", "prev": h0,
          "at": "2026-07-04T01:00:00Z", "kind": "action", "target": "x.gmail",
          "authorized_by": leaf_id, "authorized_via": [root_id, leaf_id],
          "payload": {"action": "label", "args_hash": "sha256:" + "bb" * 32},
          "signature": sig("b5")}
    a5j, b5j = jcs(a5), jcs(b5)
    head_a, head_b = entry_head(a5j), entry_head(b5j)

    # ---- two competing height-3 parent manifests (fixture-signed; the
    # chain hash blanks signature.value, so ordering is real)
    def parent(tag, gamma_head, seg_lines):
        return {
            "aithos-core": "1.0.0-draft.1",
            "edition": {"height": 3, "prev_hash": "c0" * 32,
                        "created_at": "2026-07-04T02:00:00Z"},
            "files": {f"gamma/2026-07.jsonl": "sha256-fixture-" + tag},
            "roots": {},
            "gamma_roots": {"2026-07": {
                "root": mroot([h_leaf(e.encode()) for e in seg_lines]).hex(),
                "n": len(seg_lines)}},
            "gamma_counts_root": "00" * 32,
            "gamma_head": gamma_head,
            "signature": {"alg": "ed25519", "key": "#root",
                          "value": "fixture-" + tag},
        }

    pa = parent("a", head_a, ancestor + [a5j])
    pb = parent("b", head_b, ancestor + [b5j])
    hash_a, hash_b = manifest_chain_hash(pa), manifest_chain_hash(pb)
    # ascending edition hash orders everything
    (h_lo, p_lo, tip_lo, sub_lo), (h_hi, _p_hi, tip_hi, sub_hi) = sorted(
        [(hash_a, pa, head_a, a5j), (hash_b, pb, head_b, b5j)]
    )
    merges = [h_lo, h_hi]

    # ---- the merge entry: prev = lowest parent's tip, prevs = both,
    # ordered like merges
    m6 = {"v": 1, "id": "gamma_00000000000000000000000IM6",
          "prev": tip_lo, "prevs": [tip_lo, tip_hi],
          "at": "2026-07-04T02:00:00Z", "kind": "merge",
          "payload": {"merges": merges}, "signature": sig("m6")}
    m6j = jcs(m6)

    # ---- merged segment: shared prefix, sub-chain LO, sub-chain HI, merge
    merged = ancestor + [sub_lo, sub_hi, m6j]
    merged_root = mroot([h_leaf(e.encode()) for e in merged])
    # at-monotonicity is relaxed at the join: if B is the low parent, a5
    # (00:00) physically follows b5 (01:00) — chain truth is prev/prevs.

    # ---- 3-way index merge by sid
    S1, S2, S3 = "01SID000001", "01SID000002", "01SID000003"
    base = {"folders": [{"sid": "01FLD000001", "name": "projets",
                         "parent_sid": None}],
            "sections": [
                {"sid": S1, "name": "note1", "folder_sid": "01FLD000001",
                 "title": "N1", "tags": [], "blob_sha": "aa" * 32,
                 "key_version": 1},
                {"sid": S2, "name": "note2", "folder_sid": "01FLD000001",
                 "title": "N2", "tags": [], "blob_sha": "bb" * 32,
                 "key_version": 1}]}
    # branch A adds s3; branch B deletes s2
    added_s3 = {"sid": S3, "name": "note3", "folder_sid": "01FLD000001",
                "title": "N3", "tags": [], "blob_sha": "cc" * 32,
                "key_version": 1}
    idx_a = {"folders": base["folders"],
             "sections": base["sections"] + [added_s3]}
    idx_b = {"folders": base["folders"], "sections": [base["sections"][0]]}

    def merge_index(base, a, b):
        """3-way by sid (§2.6 graved): changed rows from their branch,
        additions unioned, deletions hold; same sid changed on both sides
        = same-node conflict. Result sorted by sid."""
        out = {}
        for kind in ("folders", "sections"):
            base_rows = {r["sid"]: r for r in base[kind]}
            a_rows = {r["sid"]: r for r in a[kind]}
            b_rows = {r["sid"]: r for r in b[kind]}
            merged_rows = {}
            for sid in sorted(set(base_rows) | set(a_rows) | set(b_rows)):
                in_base, in_a, in_b = (sid in base_rows, sid in a_rows,
                                       sid in b_rows)
                ra, rb = a_rows.get(sid), b_rows.get(sid)
                a_changed = ra != base_rows.get(sid) if in_a else in_base
                b_changed = rb != base_rows.get(sid) if in_b else in_base
                if a_changed and b_changed and ra != rb:
                    raise ValueError(f"same-node conflict on sid {sid}")
                if a_changed:
                    row = ra  # None = deleted on A
                elif b_changed:
                    row = rb
                else:
                    row = base_rows.get(sid) or ra or rb
                if row is not None:
                    merged_rows[sid] = row
            out[kind] = [merged_rows[s] for s in sorted(merged_rows)]
        return out

    merged_index = merge_index(base, idx_a, idx_b)
    assert [r["sid"] for r in merged_index["sections"]] == [S1, S3], \
        "deletion holds, addition unions, sid order"

    # conflict negative: both branches retitle S1 differently
    ca = {"folders": base["folders"],
          "sections": [dict(base["sections"][0], title="A!"),
                       base["sections"][1]]}
    cb = {"folders": base["folders"],
          "sections": [dict(base["sections"][0], title="B!"),
                       base["sections"][1]]}
    conflict = None
    try:
        merge_index(base, ca, cb)
    except ValueError as e:
        conflict = str(e)
    assert conflict and S1 in conflict, "same-sid conflict must be refused"

    return {
        "ancestor_head": h0,
        "branch_a": {"entry_jcs": a5j, "head": head_a,
                     "manifest": pa, "edition_hash": hash_a},
        "branch_b": {"entry_jcs": b5j, "head": head_b,
                     "manifest": pb, "edition_hash": hash_b},
        "merge": {
            "merges": merges,
            "prev_hash_parent": h_lo,
            "entry_jcs": m6j,
            "gamma_head": entry_head(m6j),
            "merged_segment": merged,
            "merged_segment_root_hex": merged_root.hex(),
            "merged_segment_n": len(merged),
        },
        "index_merge": {
            "base": base,
            "branch_a": idx_a,
            "branch_b": idx_b,
            "merged_jcs": jcs(merged_index),
        },
        "negatives": {
            "same_sid_conflict": conflict,
            "expected": "fork — nearest common manager (fail-closed)",
        },
    }


if __name__ == "__main__":
    self_check_b2()
    out = {
        "vector": "I1",
        "description": "Concurrency (spec 02.6 + 07.6): deterministic disjoint "
                       "merge — parents ordered by ascending edition hash, "
                       "prev_hash = the lowest, additive merges; the "
                       "two-predecessor merge entry (prev = low tip, prevs = "
                       "both, payload.merges mirrors the manifest); merged "
                       "segment = shared prefix, sub-chain LOW, sub-chain "
                       "HIGH, merge entry — bytes never rewritten, gamma root "
                       "recommitted; 3-way index merge by sid (union, "
                       "deletions hold) and the same-sid conflict negative. "
                       "Ancestor segment = F2's committed entries; segment "
                       "conventions cross-checked against committed H2. "
                       "Generated independently (Python blake3/hashlib).",
        "tree": build(),
    }
    with open("i1-concurrency.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("self-checks vs B2 + H2 + F2 passed; wrote i1-concurrency.json")

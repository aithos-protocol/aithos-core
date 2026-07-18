#!/usr/bin/env python3
"""Independent CB2 oracle for the CB13 local concurrency/final gate.

The oracle is pure data. It fixes deterministic disjointness/conflict decisions,
single-actor merge authority, fork resolution authority, semantic counter
recomposition and object insertion-order independence without invoking Rust or
modeling Provider CAS.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-bundle-concurrency-final.json"

HISTORICAL_FILES = (
    "i1-concurrency.json",
    "h2-gamma-roots.json",
    "cb2-delegated-counts.json",
    "cb2-gamma-v2-replay.json",
    "cb2-draft2-carriers.json",
    "cb2-bundle-boundaries.json",
    "cb2-bundle-authority-flows.json",
    "cb2-bundle-structure-vault.json",
)


def jcs(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def sha256_hex(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def state_digest(value: Any) -> str:
    return "sha256:" + sha256_hex(jcs(value).encode())


def merge_verdict(case: dict[str, Any]) -> str:
    overlap = set(case["left_changed_sids"]) & set(case["right_changed_sids"])
    if overlap:
        return "conflict"
    if case["delete_wins"]:
        return "accepted without resurrection"
    return "accepted"


def authority_verdict(case: dict[str, Any]) -> str:
    if case["actor"] == "owner":
        return "accepted"
    chains = case["chains"]
    return (
        "accepted"
        if len(chains) == 1 and set(chains[0]["covers"]) == set(case["changed_sids"])
        else "refused"
    )


def build_vector() -> dict[str, Any]:
    initial_state = {
        "common_parent": "sha256:common-parent",
        "manifest": "sha256:canonical-before",
        "gamma_tips": ["sha256:tip-left", "sha256:tip-right"],
        "circle_root": "sha256:circle-before",
        "counts_root": "sha256:counts-before",
    }
    initial_digest = state_digest(initial_state)

    merge_cases = [
        {
            "id": "different-folders",
            "left_changed_sids": ["sid-left"],
            "right_changed_sids": ["sid-right"],
            "delete_wins": False,
            "expected": "accepted",
            "merged_sid_order": ["sid-left", "sid-right"],
        },
        {
            "id": "same-folder-distinct-sids",
            "left_changed_sids": ["sid-b"],
            "right_changed_sids": ["sid-a"],
            "delete_wins": False,
            "expected": "accepted",
            "merged_sid_order": ["sid-a", "sid-b"],
        },
        {
            "id": "delete-and-sibling-add",
            "left_changed_sids": ["sid-deleted"],
            "right_changed_sids": ["sid-added"],
            "delete_wins": True,
            "expected": "accepted without resurrection",
            "deleted_sid_present": False,
            "merged_sid_order": ["sid-added"],
        },
        {
            "id": "same-sid-modified",
            "left_changed_sids": ["sid-shared"],
            "right_changed_sids": ["sid-shared"],
            "delete_wins": False,
            "expected": "conflict",
            "visible_state_digest": initial_digest,
        },
        {
            "id": "derived-row-overlap",
            "left_changed_sids": ["sid-view"],
            "right_changed_sids": ["sid-view"],
            "delete_wins": False,
            "expected": "conflict",
            "visible_state_digest": initial_digest,
        },
    ]
    for case in merge_cases:
        actual = merge_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    changed_sids = ["sid-left", "sid-right"]
    authority_cases = [
        {
            "id": "one-covering-chain",
            "actor": "grantee",
            "changed_sids": changed_sids,
            "chains": [{"id": "chain-covering", "covers": changed_sids}],
            "expected": "accepted",
            "published_actor": "grantee",
            "published_chain_count": 1,
        },
        {
            "id": "one-partial-chain",
            "actor": "grantee",
            "changed_sids": changed_sids,
            "chains": [{"id": "chain-left", "covers": ["sid-left"]}],
            "expected": "refused",
            "visible_state_digest": initial_digest,
        },
        {
            "id": "two-partial-chains",
            "actor": "grantee",
            "changed_sids": changed_sids,
            "chains": [
                {"id": "chain-left", "covers": ["sid-left"]},
                {"id": "chain-right", "covers": ["sid-right"]},
            ],
            "expected": "refused",
            "visible_state_digest": initial_digest,
        },
        {
            "id": "owner-local",
            "actor": "owner",
            "changed_sids": changed_sids,
            "chains": [],
            "expected": "accepted",
            "published_actor": "owner",
            "published_chain_count": 0,
        },
    ]
    for case in authority_cases:
        actual = authority_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    resolution_cases = [
        {
            "id": "covering-delegate",
            "actor": "grantee",
            "covers_every_touched_sid": True,
            "chain_count": 1,
            "expected": "accepted",
        },
        {
            "id": "owner-last-resort",
            "actor": "owner",
            "covers_every_touched_sid": True,
            "chain_count": 0,
            "expected": "accepted",
        },
        {
            "id": "delegate-outside-one-perimeter",
            "actor": "grantee",
            "covers_every_touched_sid": False,
            "chain_count": 1,
            "expected": "refused",
            "visible_state_digest": initial_digest,
        },
        {
            "id": "two-partial-resolution-chains",
            "actor": "grantee",
            "covers_every_touched_sid": True,
            "chain_count": 2,
            "expected": "refused",
            "visible_state_digest": initial_digest,
        },
        {
            "id": "unresolved-fork",
            "actor": "none",
            "covers_every_touched_sid": False,
            "chain_count": 0,
            "expected": "no canonical branch",
            "visible_state_digest": initial_digest,
        },
    ]

    prefix_occurrences = [
        {"operation_ref": "op-common-action", "kind": "action"},
    ]
    left_occurrences = prefix_occurrences + [
        {"operation_ref": "op-left-action", "kind": "action"},
        {"operation_ref": "op-left-mutation", "kind": "mutation"},
        {"operation_ref": "op-left-grant", "kind": "grant"},
    ]
    right_occurrences = prefix_occurrences + [
        {"operation_ref": "op-right-action", "kind": "action"},
        {"operation_ref": "op-right-mutation", "kind": "mutation"},
        {"operation_ref": "op-right-grant", "kind": "grant"},
    ]
    unique_occurrences = {
        occurrence["operation_ref"]: occurrence
        for occurrence in left_occurrences + right_occurrences
    }
    recomposed_counts = {
        "actions": sum(
            occurrence["kind"] == "action"
            for occurrence in unique_occurrences.values()
        ),
        "mutations": sum(
            occurrence["kind"] == "mutation"
            for occurrence in unique_occurrences.values()
        ),
        "consumptions": len(unique_occurrences),
        "direct_children": sum(
            occurrence["kind"] == "grant"
            for occurrence in unique_occurrences.values()
        ),
    }
    if recomposed_counts != {
        "actions": 3,
        "mutations": 2,
        "consumptions": 7,
        "direct_children": 2,
    }:
        raise AssertionError(recomposed_counts)

    objects = {
        "manifest.json": '{"height":3}',
        "manifests/3.json": '{"height":3}',
        "changesets/a.json": '{"changes":[]}',
        "evidence/b.json": '{"items":[]}',
        "gamma/2026-07.jsonl": '{"id":"merge"}\n',
        "e/circle/index.json": '{"sections":[]}',
    }
    insertion_orders = [
        list(objects),
        list(reversed(objects)),
        sorted(objects),
        sorted(objects, reverse=True),
        [
            "gamma/2026-07.jsonl",
            "manifest.json",
            "evidence/b.json",
            "e/circle/index.json",
            "manifests/3.json",
            "changesets/a.json",
        ],
        [
            "changesets/a.json",
            "e/circle/index.json",
            "manifest.json",
            "gamma/2026-07.jsonl",
            "evidence/b.json",
            "manifests/3.json",
        ],
    ]
    order_cases = []
    expected_digest = state_digest(objects)
    for index, order in enumerate(insertion_orders, start=1):
        if sorted(order) != sorted(objects):
            raise AssertionError(f"insertion order {index} is not a permutation")
        inserted = {key: objects[key] for key in order}
        digest = state_digest(inserted)
        if digest != expected_digest:
            raise AssertionError(f"insertion order {index} changed digest")
        order_cases.append(
            {
                "id": f"order-{index}",
                "insertion_order": order,
                "cold_digest": digest,
                "expected": "accepted",
            }
        )

    historical = {
        name: sha256_hex((HERE / name).read_bytes())
        for name in HISTORICAL_FILES
    }
    return {
        "vector": "CB2-BUNDLE-CONCURRENCY-FINAL-1",
        "description": (
            "Independent pure-data CB13 oracle for deterministic local merge/"
            "conflict, one fully covering actor/chain, effect-free fork resolution, "
            "semantic counter recomposition and fresh-store insertion-order "
            "independence. Provider CAS and network participation are excluded."
        ),
        "historical_vector_sha256": historical,
        "initial_state": initial_state,
        "initial_state_digest": initial_digest,
        "merge": {
            "cases": merge_cases,
            "authority_cases": authority_cases,
            "parent_order": "ascending edition hash",
            "gamma_subchain_order": "lowest parent hash then highest parent hash then merge entry",
            "merge_entry_is_not_an_extra_business_consumption": True,
            "network_participates": False,
            "provider_cas_participates": False,
        },
        "resolution": {
            "cases": resolution_cases,
            "losing_write_is_surfaced_not_replayed": True,
            "refusal_changes_canonical_bytes": False,
        },
        "counter_recomposition": {
            "left_occurrences": left_occurrences,
            "right_occurrences": right_occurrences,
            "unique_occurrence_count": len(unique_occurrences),
            "expected_counts": recomposed_counts,
            "shared_prefix_counted_once": True,
            "branch_occurrence_omitted": False,
            "branch_occurrence_double_counted": False,
        },
        "fresh_store": {
            "objects": objects,
            "expected_cold_digest": expected_digest,
            "insertion_order_cases": order_cases,
            "producer_destroyed_before_verify": True,
            "private_capabilities_absent_during_verify": True,
            "network_participates": False,
            "provider_cas_participates": False,
        },
        "inventory": {
            "merge_case_count": len(merge_cases),
            "authority_case_count": len(authority_cases),
            "resolution_case_count": len(resolution_cases),
            "insertion_order_case_count": len(order_cases),
            "future_owner": "CB13",
        },
    }


def encoded(vector: dict[str, Any]) -> bytes:
    return (json.dumps(vector, indent=2, ensure_ascii=False) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    payload = encoded(build_vector())
    if args.check:
        if not args.output.exists():
            raise SystemExit(f"missing {args.output}")
        if args.output.read_bytes() != payload:
            raise SystemExit(f"{args.output} is not reproducible")
        print(f"verified {args.output}")
        return
    args.output.write_bytes(payload)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Independent CB2 oracle for D7 delegated occurrence counters.

This Python implementation does not call Rust.  It independently:

* correlates evidence views by the canonical operation occurrence;
* counts delegated Ethos mutations and total delegated occurrences;
* applies subtree accounting through ``authorized_via``;
* encodes the closed non-zero counter leaves;
* computes the separate BLAKE3/left-heavy Merkle root and proof;
* validates draft3 constraint structure and attenuation; and
* proves the historical H2 gamma-count root remains byte-identical.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
from typing import Any, NoReturn

import blake3


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-delegated-counts.json"

PROFILE_KEY = "aithos-delegated-counts-core"
PROFILE = "1.0.0-draft.1"
MANDATE_PROFILE_KEY = "aithos-mandate-core"
MANDATE_PROFILE = "1.0.0-draft.3"
INVALID_COUNTS = "InvalidDelegatedCounts"
INVALID_MANDATE = "InvalidMandate"

ZEROS = b"\x00" * 32
LEAF_DOMAIN = b"aithos-core/v1/mk-leaf\x00"
NODE_DOMAIN = b"aithos-core/v1/mk-node\x00"
ROOT_RE = re.compile(r"^[0-9a-f]{64}$")
COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")
OCCURRENCE_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")

KINDS = {
    "read",
    "mutation",
    "action",
    "inference",
    "grant",
    "revoke",
    "rotate",
    "publication",
}
FACT_DOMAINS = {
    "ethos",
    "structure",
    "vault-config",
    "connector",
    "inference",
    "mandate",
    "rotation",
    "publication",
    "gamma",
}
VIEW_KEYS = {
    "view",
    "occurrence",
    "commitment",
    "actor",
    "authorized_via",
    "kind",
    "facts_domain",
    "opposable",
    "derived",
}


class CountsError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_COUNTS


class MandateError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_MANDATE


def reject_counts(detail: str) -> NoReturn:
    raise CountsError(detail)


def reject_mandate(detail: str) -> NoReturn:
    raise MandateError(detail)


def clone(value: Any) -> Any:
    return copy.deepcopy(value)


def jcs(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def b3(payload: bytes) -> bytes:
    return blake3.blake3(payload).digest()


def h_leaf(payload: bytes) -> bytes:
    return b3(LEAF_DOMAIN + payload)


def h_node(left: bytes, right: bytes) -> bytes:
    return b3(NODE_DOMAIN + left + right)


def mroot(hashes: list[bytes]) -> bytes:
    if not hashes:
        return ZEROS
    if len(hashes) == 1:
        return hashes[0]
    middle = (len(hashes) + 1) // 2
    return h_node(mroot(hashes[:middle]), mroot(hashes[middle:]))


def mroot_path(hashes: list[bytes], index: int) -> list[dict[str, Any]]:
    if len(hashes) <= 1:
        return []
    middle = (len(hashes) + 1) // 2
    if index < middle:
        return mroot_path(hashes[:middle], index) + [
            {"node": {"side": "right", "hash": mroot(hashes[middle:]).hex()}}
        ]
    return mroot_path(hashes[middle:], index - middle) + [
        {"node": {"side": "left", "hash": mroot(hashes[:middle]).hex()}}
    ]


def run_proof(start: bytes, steps: list[dict[str, Any]]) -> bytes:
    current = start
    for step in steps:
        node = step["node"]
        sibling = bytes.fromhex(node["hash"])
        current = (
            h_node(sibling, current)
            if node["side"] == "left"
            else h_node(current, sibling)
        )
    return current


def mandate_id(number: int) -> str:
    value = f"mandate_01J{number:023d}"
    assert MANDATE_RE.fullmatch(value)
    return value


def occurrence(number: int) -> str:
    value = f"op_01K{number:023d}"
    assert OCCURRENCE_RE.fullmatch(value)
    return value


def commitment(number: int) -> str:
    return "sha256:" + hashlib.sha256(f"operation-{number}".encode()).hexdigest()


def view(
    number: int,
    evidence_view: str,
    actor: str,
    via: list[str],
    kind: str,
    facts_domain: str,
    *,
    opposable: bool = True,
    derived: bool = False,
) -> dict[str, Any]:
    return {
        "view": evidence_view,
        "occurrence": occurrence(number),
        "commitment": commitment(number),
        "actor": actor,
        "authorized_via": via,
        "kind": kind,
        "facts_domain": facts_domain,
        "opposable": opposable,
        "derived": derived,
    }


def build_evidence(root: str, leaf: str, child: str) -> list[dict[str, Any]]:
    via = [root, leaf]
    child_via = [root, leaf, child]
    out = [
        view(1, "gamma", "grantee", via, "action", "connector"),
        view(1, "receipt", "grantee", via, "action", "connector"),
        view(2, "receipt", "grantee", via, "inference", "inference"),
        view(3, "gamma", "grantee", via, "mutation", "ethos"),
        view(3, "authorship", "grantee", via, "mutation", "ethos"),
        view(3, "changeset", "grantee", via, "mutation", "ethos"),
        view(4, "gamma", "grantee", via, "read", "gamma"),
        view(4, "presentation", "grantee", via, "read", "gamma"),
        view(5, "gamma", "grantee", via, "mutation", "vault-config"),
        view(6, "gamma", "grantee", via, "grant", "mandate"),
        view(7, "gamma", "grantee", via, "revoke", "mandate"),
        view(
            7,
            "derived-rotation",
            "grantee",
            via,
            "revoke",
            "mandate",
            derived=True,
        ),
        view(8, "changeset", "grantee", via, "mutation", "structure"),
        view(9, "gamma", "grantee", via, "rotate", "rotation"),
        view(10, "edition", "grantee", via, "publication", "publication"),
        view(10, "authorship", "grantee", via, "publication", "publication"),
        view(11, "edition-merge", "grantee", via, "publication", "publication"),
        view(11, "gamma-merge", "grantee", via, "publication", "publication"),
        view(
            12,
            "edition-resolution",
            "grantee",
            via,
            "publication",
            "publication",
        ),
        view(13, "gamma", "grantee", child_via, "action", "connector"),
        view(14, "gamma", "owner", [], "mutation", "ethos"),
        view(15, "gamma", "grantee", via, "mutation", "ethos"),
        view(15, "changeset", "grantee", via, "mutation", "ethos"),
    ]
    return out


def validate_view_shape(candidate: Any) -> dict[str, Any]:
    if not isinstance(candidate, dict) or set(candidate) != VIEW_KEYS:
        reject_counts("evidence view has a non-exact member set")
    if not isinstance(candidate["view"], str) or not candidate["view"]:
        reject_counts("invalid evidence view name")
    if not isinstance(candidate["occurrence"], str) or not OCCURRENCE_RE.fullmatch(
        candidate["occurrence"]
    ):
        reject_counts("invalid occurrence")
    if not isinstance(candidate["commitment"], str) or not COMMITMENT_RE.fullmatch(
        candidate["commitment"]
    ):
        reject_counts("invalid commitment")
    if candidate["actor"] not in {"owner", "grantee"}:
        reject_counts("invalid actor")
    via = candidate["authorized_via"]
    if not isinstance(via, list) or any(
        not isinstance(item, str) or not MANDATE_RE.fullmatch(item) for item in via
    ):
        reject_counts("invalid authorized_via")
    if len(via) != len(set(via)):
        reject_counts("duplicate mandate in authorized_via")
    if candidate["actor"] == "owner" and via:
        reject_counts("owner carries delegated chain")
    if candidate["actor"] == "grantee" and not via:
        reject_counts("grantee has no delegated chain")
    if candidate["kind"] not in KINDS:
        reject_counts("unknown operation kind")
    if candidate["facts_domain"] not in FACT_DOMAINS:
        reject_counts("unknown facts domain")
    if not isinstance(candidate["opposable"], bool):
        reject_counts("opposable is not boolean")
    if not isinstance(candidate["derived"], bool):
        reject_counts("derived is not boolean")
    if candidate["kind"] != "read" and not candidate["opposable"]:
        reject_counts("only a read may be silent")
    return candidate


def tally(
    evidence_views: list[dict[str, Any]],
) -> tuple[dict[str, dict[str, int]], list[str]]:
    if not isinstance(evidence_views, list):
        reject_counts("evidence views are not an array")
    grouped: dict[str, list[dict[str, Any]]] = {}
    for raw in evidence_views:
        item = validate_view_shape(raw)
        grouped.setdefault(item["occurrence"], []).append(item)

    counts: dict[str, dict[str, int]] = {}
    occurrences: list[str] = []
    for occurrence_id, group in sorted(grouped.items()):
        first = group[0]
        correlation = (
            first["commitment"],
            first["actor"],
            tuple(first["authorized_via"]),
            first["kind"],
            first["facts_domain"],
            first["opposable"],
        )
        if any(
            (
                item["commitment"],
                item["actor"],
                tuple(item["authorized_via"]),
                item["kind"],
                item["facts_domain"],
                item["opposable"],
            )
            != correlation
            for item in group[1:]
        ):
            reject_counts("one occurrence has conflicting evidence")
        if not any(not item["derived"] for item in group):
            reject_counts("derived evidence has no parent occurrence")
        if len({item["view"] for item in group}) != len(group):
            reject_counts("duplicate native evidence view")

        if first["actor"] == "owner":
            continue
        if first["kind"] == "read" and not first["opposable"]:
            continue
        occurrences.append(occurrence_id)
        for mandate in first["authorized_via"]:
            bucket = counts.setdefault(mandate, {"mutations": 0, "consumptions": 0})
            bucket["consumptions"] += 1
            if first["kind"] == "mutation" and first["facts_domain"] == "ethos":
                bucket["mutations"] += 1

    closed: dict[str, dict[str, int]] = {}
    for mandate, counters in counts.items():
        non_zero = {key: value for key, value in counters.items() if value != 0}
        if non_zero:
            closed[mandate] = non_zero
    return dict(sorted(closed.items())), occurrences


def leaf_payload(mandate: str, counters: dict[str, int]) -> bytes:
    return mandate.encode("utf-8") + b"\x00" + jcs(counters).encode("utf-8")


def encode_leaves(counts: dict[str, dict[str, int]]) -> list[dict[str, Any]]:
    leaves = []
    for mandate, counters in sorted(counts.items()):
        payload = leaf_payload(mandate, counters)
        leaves.append(
            {
                "mandate_id": mandate,
                "counters": counters,
                "payload_hex": payload.hex(),
                "leaf_hex": h_leaf(payload).hex(),
            }
        )
    return leaves


def root_for_leaves(leaves: list[dict[str, Any]]) -> str:
    return mroot([bytes.fromhex(item["leaf_hex"]) for item in leaves]).hex()


def validate_counter_material(
    reference: Any,
    leaves: Any,
    evidence_views: Any,
) -> tuple[dict[str, dict[str, int]], list[str]]:
    if not isinstance(reference, dict) or set(reference) != {PROFILE_KEY, "root"}:
        reject_counts("delegated_counts reference has a non-exact member set")
    if reference[PROFILE_KEY] != PROFILE:
        reject_counts("unknown delegated-counts profile")
    if not isinstance(reference["root"], str) or not ROOT_RE.fullmatch(reference["root"]):
        reject_counts("invalid delegated-counts root")

    expected_counts, occurrences = tally(evidence_views)
    expected_leaves = encode_leaves(expected_counts)
    if leaves != expected_leaves:
        reject_counts("delegated-count leaves do not match canonical occurrences")
    if reference["root"] != root_for_leaves(expected_leaves):
        reject_counts("delegated-count root mismatch")
    return expected_counts, occurrences


def validate_mandates(mandates: Any) -> None:
    if not isinstance(mandates, list) or not mandates:
        reject_mandate("mandate chain must be non-empty")
    previous: dict[str, Any] | None = None
    for item in mandates:
        if not isinstance(item, dict) or set(item) != {
            "id",
            "parent",
            MANDATE_PROFILE_KEY,
            "constraints",
        }:
            reject_mandate("invalid mandate fixture shape")
        if not isinstance(item["id"], str) or not MANDATE_RE.fullmatch(item["id"]):
            reject_mandate("invalid mandate id")
        constraints = item["constraints"]
        if not isinstance(constraints, dict):
            reject_mandate("constraints are not an object")
        d7_present = bool({"max_mutations", "max_consumptions"} & set(constraints))
        if d7_present and item[MANDATE_PROFILE_KEY] != MANDATE_PROFILE:
            reject_mandate("D7 constraints require homogeneous draft3")
        if item[MANDATE_PROFILE_KEY] != MANDATE_PROFILE:
            reject_mandate("unknown mandate profile in draft3 chain")
        for name in ("max_mutations", "max_consumptions"):
            if name not in constraints:
                reject_mandate(f"{name} is dropped")
            value = constraints[name]
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                reject_mandate(f"{name} is not an unsigned integer")
        if previous is None:
            if item["parent"] is not None:
                reject_mandate("root mandate has a parent")
        else:
            if item["parent"] != previous["id"]:
                reject_mandate("mandate parent mismatch")
            for name in ("max_mutations", "max_consumptions"):
                if item["constraints"][name] > previous["constraints"][name]:
                    reject_mandate(f"{name} widens")
        previous = item


def historical_h2_check() -> dict[str, str]:
    path = HERE / "h2-gamma-roots.json"
    historical = json.loads(path.read_text())
    counts = historical["tree"]["counts"]
    leaves = encode_leaves(counts)
    root = root_for_leaves(leaves)
    if root != historical["tree"]["gamma_counts_root_hex"]:
        raise AssertionError("Merkle/JCS convention drift against historical H2")
    return {
        "h2-gamma-roots.json": sha256_file(path),
        "gen-h2.py": sha256_file(HERE / "gen-h2.py"),
    }


def positive_fixture() -> dict[str, Any]:
    root = mandate_id(20)
    leaf = mandate_id(21)
    child = mandate_id(22)
    mandates = [
        {
            "id": root,
            "parent": None,
            MANDATE_PROFILE_KEY: MANDATE_PROFILE,
            "constraints": {"max_mutations": 20, "max_consumptions": 40},
        },
        {
            "id": leaf,
            "parent": root,
            MANDATE_PROFILE_KEY: MANDATE_PROFILE,
            "constraints": {"max_mutations": 10, "max_consumptions": 20},
        },
        {
            "id": child,
            "parent": leaf,
            MANDATE_PROFILE_KEY: MANDATE_PROFILE,
            "constraints": {"max_mutations": 0, "max_consumptions": 1},
        },
    ]
    evidence = build_evidence(root, leaf, child)
    counts, counted_occurrences = tally(evidence)
    expected_counts = {
        root: {"mutations": 2, "consumptions": 14},
        leaf: {"mutations": 2, "consumptions": 14},
        child: {"consumptions": 1},
    }
    if counts != expected_counts:
        raise AssertionError(f"unexpected oracle fixture tally: {counts}")
    leaves = encode_leaves(counts)
    reference = {
        PROFILE_KEY: PROFILE,
        "root": root_for_leaves(leaves),
    }
    validate_mandates(mandates)
    validate_counter_material(reference, leaves, evidence)
    proof_index = 1
    proof = {
        "payload": leaves[proof_index]["payload_hex"],
        "steps": mroot_path(
            [bytes.fromhex(item["leaf_hex"]) for item in leaves],
            proof_index,
        ),
        "root": reference["root"],
    }
    if (
        run_proof(h_leaf(bytes.fromhex(proof["payload"])), proof["steps"]).hex()
        != proof["root"]
    ):
        raise AssertionError("independent proof does not replay")
    return {
        "mandates": mandates,
        "evidence_views": evidence,
        "non_occurrences": [
            {
                "id": "silent-local-read",
                "reason": "no signed or journalized occurrence exists",
                "delta": {"mutations": 0, "consumptions": 0},
            }
        ],
        "expected_counted_occurrences": counted_occurrences,
        "expected_counts": counts,
        "leaves": leaves,
        "delegated_counts": reference,
        "proof_leaf_mandate": proof,
        "empty_root": ZEROS.hex(),
        "historical_children_delta_for_direct_grant": 1,
        "two_ethos_mutations_plus_publication_delta": {
            "mutations": 2,
            "consumptions": 3,
        },
    }


def replace_root(candidate: dict[str, Any], root: str) -> None:
    candidate["delegated_counts"]["root"] = root


def mutate_leaf_counter(
    candidate: dict[str, Any],
    leaf_index: int,
    name: str,
    value: Any,
) -> None:
    candidate["leaves"][leaf_index]["counters"][name] = value


def counter_negative_cases(valid: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[tuple[str, Any]] = []

    def count_silent_read(candidate: dict[str, Any]) -> None:
        candidate["evidence_views"].append(
            view(
                90,
                "local-read",
                "grantee",
                [mandate_id(20), mandate_id(21)],
                "read",
                "ethos",
                opposable=False,
            )
        )
        mutate_leaf_counter(candidate, 0, "consumptions", 15)

    def add(identifier: str, mutator: Any) -> None:
        candidate = {
            "delegated_counts": clone(valid["delegated_counts"]),
            "leaves": clone(valid["leaves"]),
            "evidence_views": clone(valid["evidence_views"]),
        }
        mutator(candidate)
        try:
            validate_counter_material(
                candidate["delegated_counts"],
                candidate["leaves"],
                candidate["evidence_views"],
            )
        except CountsError as error:
            if error.code != INVALID_COUNTS:
                raise AssertionError(identifier) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {identifier}")
        cases.append((identifier, candidate))

    add(
        "unknown-profile",
        lambda c: c["delegated_counts"].__setitem__(PROFILE_KEY, "1.0.0-draft.2"),
    )
    add(
        "missing-profile",
        lambda c: c["delegated_counts"].pop(PROFILE_KEY),
    )
    add(
        "extra-reference-member",
        lambda c: c["delegated_counts"].__setitem__("extra", True),
    )
    add("null-root", lambda c: replace_root(c, None))
    add("short-root", lambda c: replace_root(c, "00"))
    add("uppercase-root", lambda c: replace_root(c, c["delegated_counts"]["root"].upper()))
    add("prefixed-root", lambda c: replace_root(c, "sha256:" + c["delegated_counts"]["root"]))
    add("wrong-root", lambda c: replace_root(c, "11" * 32))
    add("missing-leaf", lambda c: c["leaves"].pop())
    add("extra-leaf", lambda c: c["leaves"].append(clone(c["leaves"][0])))
    add("unsorted-leaves", lambda c: c["leaves"].reverse())
    add(
        "zero-mutations-field",
        lambda c: mutate_leaf_counter(c, 2, "mutations", 0),
    )
    add(
        "zero-consumptions-field",
        lambda c: mutate_leaf_counter(c, 2, "consumptions", 0),
    )
    add(
        "unknown-counter-member",
        lambda c: mutate_leaf_counter(c, 0, "actions", 1),
    )
    add(
        "negative-counter",
        lambda c: mutate_leaf_counter(c, 0, "mutations", -1),
    )
    add(
        "string-counter",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", "14"),
    )
    add(
        "null-counter",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", None),
    )
    add(
        "injected-mutation-tally",
        lambda c: mutate_leaf_counter(c, 0, "mutations", 3),
    )
    add(
        "injected-consumption-tally",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", 15),
    )
    add(
        "missing-consumption-tally",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", 13),
    )
    add(
        "duplicate-view-counted-twice",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", 15),
    )
    add(
        "vault-config-counted-as-ethos",
        lambda c: mutate_leaf_counter(c, 0, "mutations", 3),
    )
    add(
        "structure-counted-as-ethos",
        lambda c: mutate_leaf_counter(c, 1, "mutations", 3),
    )
    add(
        "derived-rotation-counted-twice",
        lambda c: mutate_leaf_counter(c, 1, "consumptions", 15),
    )
    add(
        "owner-occurrence-counted",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", 15),
    )
    add(
        "ancestor-subtree-count-omitted",
        lambda c: mutate_leaf_counter(c, 0, "consumptions", 13),
    )
    add(
        "unrelated-mandate-injected",
        lambda c: c["leaves"].append(
            {
                "mandate_id": mandate_id(99),
                "counters": {"consumptions": 1},
                "payload_hex": "",
                "leaf_hex": "00" * 32,
            }
        ),
    )
    add(
        "conflicting-occurrence-commitment",
        lambda c: c["evidence_views"][1].__setitem__("commitment", commitment(99)),
    )
    add(
        "conflicting-occurrence-authority",
        lambda c: c["evidence_views"][1].__setitem__("actor", "owner"),
    )
    add(
        "duplicate-native-view",
        lambda c: c["evidence_views"].append(clone(c["evidence_views"][0])),
    )
    add(
        "derived-view-without-parent",
        lambda c: c["evidence_views"].__setitem__(
            slice(None),
            [
                item
                for item in c["evidence_views"]
                if item["occurrence"] != occurrence(7) or item["derived"]
            ],
        ),
    )
    add(
        "silent-read-counted",
        count_silent_read,
    )
    add(
        "grantee-with-empty-chain",
        lambda c: c["evidence_views"][0].__setitem__("authorized_via", []),
    )
    add(
        "duplicate-chain-id",
        lambda c: c["evidence_views"][0].__setitem__(
            "authorized_via",
            [mandate_id(20), mandate_id(20)],
        ),
    )
    add(
        "unknown-operation-kind",
        lambda c: c["evidence_views"][0].__setitem__("kind", "heartbeat"),
    )
    add(
        "extra-evidence-member",
        lambda c: c["evidence_views"][0].__setitem__("delta", 1),
    )

    return [
        {"id": identifier, "candidate": candidate, "must_fail": INVALID_COUNTS}
        for identifier, candidate in cases
    ]


def mandate_negative_cases(valid: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[tuple[str, Any]] = []

    def add(identifier: str, mutator: Any) -> None:
        candidate = clone(valid["mandates"])
        mutator(candidate)
        try:
            validate_mandates(candidate)
        except MandateError as error:
            if error.code != INVALID_MANDATE:
                raise AssertionError(identifier) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {identifier}")
        cases.append((identifier, candidate))

    add(
        "draft1-carries-max-mutations",
        lambda c: c[0].__setitem__(MANDATE_PROFILE_KEY, "1.0.0-draft.1"),
    )
    add(
        "draft2-carries-max-consumptions",
        lambda c: c[0].__setitem__(MANDATE_PROFILE_KEY, "1.0.0-draft.2"),
    )
    add(
        "mixed-profile-chain",
        lambda c: c[1].__setitem__(MANDATE_PROFILE_KEY, "1.0.0-draft.2"),
    )
    add(
        "child-drops-max-mutations",
        lambda c: c[1]["constraints"].pop("max_mutations"),
    )
    add(
        "child-drops-max-consumptions",
        lambda c: c[1]["constraints"].pop("max_consumptions"),
    )
    add(
        "child-widens-max-mutations",
        lambda c: c[1]["constraints"].__setitem__("max_mutations", 21),
    )
    add(
        "child-widens-max-consumptions",
        lambda c: c[1]["constraints"].__setitem__("max_consumptions", 41),
    )
    add(
        "negative-max-mutations",
        lambda c: c[0]["constraints"].__setitem__("max_mutations", -1),
    )
    add(
        "float-max-consumptions",
        lambda c: c[0]["constraints"].__setitem__("max_consumptions", 1.5),
    )
    add(
        "boolean-max-consumptions",
        lambda c: c[0]["constraints"].__setitem__("max_consumptions", True),
    )
    add(
        "string-max-mutations",
        lambda c: c[0]["constraints"].__setitem__("max_mutations", "20"),
    )
    add(
        "null-max-mutations",
        lambda c: c[0]["constraints"].__setitem__("max_mutations", None),
    )
    add(
        "wrong-parent",
        lambda c: c[1].__setitem__("parent", mandate_id(99)),
    )

    return [
        {"id": identifier, "candidate": candidate, "must_fail": INVALID_MANDATE}
        for identifier, candidate in cases
    ]


def build_vector() -> dict[str, Any]:
    valid = positive_fixture()
    count_negatives = counter_negative_cases(valid)
    mandate_negatives = mandate_negative_cases(valid)
    return {
        "vector": "CB2-D7-DELEGATED-COUNTS-1",
        "description": (
            "Independent Python/blake3 oracle for draft3 max_mutations and "
            "max_consumptions, occurrence correlation/deduplication, subtree "
            "tallies, the separate delegated_counts Merkle root, exact typed "
            "failures, and historical H2 byte non-regression."
        ),
        "profiles": {
            "delegated_counts": PROFILE,
            "mandate": MANDATE_PROFILE,
        },
        "merkle": {
            "leaf_domain_ascii_nul_hex": LEAF_DOMAIN.hex(),
            "node_domain_ascii_nul_hex": NODE_DOMAIN.hex(),
            "split": "left=ceil(n/2)",
        },
        "positive": valid,
        "negative_counter_cases": count_negatives,
        "negative_mandate_cases": mandate_negatives,
        "historical_vector_sha256": historical_h2_check(),
        "inventory": {
            "counter_negative_ids": [case["id"] for case in count_negatives],
            "mandate_negative_ids": [case["id"] for case in mandate_negatives],
            "counter_error_variant": INVALID_COUNTS,
            "mandate_error_variant": INVALID_MANDATE,
            "historical_gamma_counts_root_is_not_reinterpreted": True,
            "derived_rotation_is_not_an_occurrence": True,
            "owner_occurrences_are_not_delegated_counts": True,
        },
    }


def encoded(vector: dict[str, Any]) -> bytes:
    return (
        json.dumps(vector, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    output = encoded(build_vector())
    if args.check:
        existing = args.output.read_bytes()
        if existing != output:
            raise SystemExit(f"drift: {args.output}")
        print(
            f"ok {args.output.name} sha256="
            f"{hashlib.sha256(existing).hexdigest()}"
        )
        return
    args.output.write_bytes(output)
    print(f"wrote {args.output} sha256={hashlib.sha256(output).hexdigest()}")


if __name__ == "__main__":
    main()

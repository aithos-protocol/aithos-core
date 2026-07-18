#!/usr/bin/env python3
"""Independent CB2 oracle for owner/grantee Bundle authority flows.

The oracle models already-approved CB1 decisions for CB8 and CB9 as pure data:

* owner parity over list/read/create/edit/delete in public/circle/self;
* grantee verb/selector/zone decisions, including opaque self identifiers;
* certificate authority and delivered content lines as independent fences;
* delegated authorship/self-state evidence and current-authority rechecks;
* refusal atomicity, Gamma-read authority and fresh-store expectations.

It imports no Rust code, performs no Bundle I/O and introduces no signed wire.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-bundle-authority-flows.json"

HISTORICAL_FILES = (
    "cb2-mandate-contracts.json",
    "cb2-operation-projection.json",
    "cb2-operation-facts-mutation.json",
    "cb2-operation-facts-read.json",
    "cb2-delegated-counts.json",
    "cb2-session-proof.json",
    "cb2-draft2-carriers.json",
    "cb2-bundle-boundaries.json",
)

VERB_COVERS = {
    "list": {"read", "edit", "append", "delete", "write"},
    "read": {"read", "edit", "append", "delete", "write"},
    "create": {"append", "write"},
    "edit": {"edit", "append", "write"},
    "delete": {"delete", "write"},
}


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


def parse_authority(authority: str) -> dict[str, str]:
    left, separator, selector = authority.partition("#")
    parts = left.split(".")
    if len(parts) != 2:
        raise AssertionError(f"invalid authority fixture: {authority}")
    parsed = {"verb": parts[0], "zone": parts[1]}
    if separator:
        key, equals, value = selector.partition("=")
        if not equals or not value or "&" in value:
            raise AssertionError(f"unsupported authority fixture: {authority}")
        parsed["selector"] = key
        parsed["selector_value"] = value
    return parsed


def grantee_verdict(case: dict[str, Any]) -> str:
    authority = parse_authority(case["authority"])
    operation = case["operation"]
    if authority["zone"] != case["zone"]:
        return "refused"
    if authority["verb"] not in VERB_COVERS[operation]:
        return "refused"

    selector = authority.get("selector")
    selector_value = authority.get("selector_value")
    if case["zone"] == "self" and operation in {"create", "edit", "delete"}:
        if selector in {"dir", "tag"}:
            return "refused"
        if selector == "id" and selector_value != case["target_sid"]:
            return "refused"
    elif selector == "id" and selector_value != case["target_sid"]:
        return "refused"
    elif selector == "dir" and selector_value != case["target_dir"]:
        return "refused"
    elif selector == "tag" and selector_value not in case["target_tags"]:
        return "refused"
    return "accepted"


def content_fence_verdict(case: dict[str, str]) -> str:
    authority = case["authority"]
    line = case["key_material"]
    if authority != "valid covering chain":
        return "refused as unauthorized"
    if line == "exact valid section line":
        return "readable and authorized"
    if line == "no section line":
        return "authorized but unreadable"
    return "unreadable"


def build_vector() -> dict[str, Any]:
    initial_state = {
        "manifest": "sha256:owner-grantee-before",
        "gamma_head": "sha256:gamma-before",
        "public": ["note"],
        "circle": ["note"],
        "self": ["opaque-note"],
    }
    initial_digest = state_digest(initial_state)

    owner_cases = [
        {
            "id": f"owner-{zone}-{operation}",
            "zone": zone,
            "operation": operation,
            "expected": "accepted",
            "mandate_required": False,
            "mandate_counter_delta": 0,
            "journalized": operation in {"create", "edit", "delete"},
            "fresh_store_reopen": True,
        }
        for zone in ("public", "circle", "self")
        for operation in ("list", "read", "create", "edit", "delete")
    ]

    grantee_rows = (
        ("public", "list", "read.public#dir=projects", "note", "projects", (), "accepted"),
        ("public", "read", "read.public#id=note", "note", "projects", (), "accepted"),
        ("public", "create", "append.public#dir=projects", "fresh-note", "projects", (), "accepted"),
        ("public", "edit", "edit.public#id=note", "note", "projects", (), "accepted"),
        ("public", "delete", "delete.public#id=note", "note", "projects", (), "accepted"),
        ("circle", "list", "read.circle#dir=projects", "note", "projects", (), "accepted"),
        ("circle", "read", "read.circle#id=note", "note", "projects", (), "accepted"),
        ("circle", "create", "append.circle#dir=projects", "fresh-note", "projects", (), "accepted"),
        ("circle", "edit", "edit.circle#id=note", "note", "projects", (), "accepted"),
        ("circle", "delete", "delete.circle#id=note", "note", "projects", (), "accepted"),
        ("self", "list", "read.self#dir=sealed", "opaque-note", "sealed", (), "accepted"),
        ("self", "read", "read.self#id=opaque-note", "opaque-note", "sealed", (), "accepted"),
        ("self", "create", "append.self", "fresh-opaque", "sealed", (), "accepted"),
        ("self", "create", "append.self#id=preallocated", "preallocated", "sealed", (), "accepted"),
        ("self", "edit", "edit.self#id=opaque-note", "opaque-note", "sealed", (), "accepted"),
        ("self", "delete", "delete.self#id=opaque-note", "opaque-note", "sealed", (), "accepted"),
        ("self", "edit", "edit.self#dir=sealed", "opaque-note", "sealed", (), "refused"),
        ("self", "delete", "delete.self#tag=private", "opaque-note", "sealed", ("private",), "refused"),
    )
    grantee_cases = [
        {
            "id": f"grantee-{index:02d}",
            "zone": zone,
            "operation": operation,
            "authority": authority,
            "target_sid": sid,
            "target_dir": directory,
            "target_tags": list(tags),
            "expected": expected,
            "accepted_actor": "grantee",
            "accepted_single_chain": True,
            "accepted_journalized": True,
            "accepted_fresh_store_reopen": True,
            "refused_visible_state_digest": initial_digest,
        }
        for index, (zone, operation, authority, sid, directory, tags, expected) in enumerate(
            grantee_rows, start=1
        )
    ]
    for case in grantee_cases:
        actual = grantee_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    delivery_cases = [
        {
            "authority": "read.public#id=note",
            "required_line": "none",
            "delivered_node": None,
        },
        {
            "authority": "read.circle",
            "required_line": "zone-root",
            "delivered_node": "/e/circle",
        },
        {
            "authority": "read.self",
            "required_line": "zone-root",
            "delivered_node": "/e/self",
        },
        {
            "authority": "edit.circle#dir=projects",
            "required_line": "folder",
            "delivered_node": "/e/circle/d/projects",
        },
        {
            "authority": "read.circle#tag=toto",
            "required_line": "zone-tag-view",
            "delivered_node": "/e/circle/t/toto",
        },
        {
            "authority": "read.circle#dir=projects&tag=toto",
            "required_line": "folder-tag-view",
            "delivered_node": "/e/circle/d/projects/t/toto",
        },
        {
            "authority": "edit.self#id=opaque-note",
            "required_line": "section",
            "delivered_node": "/e/self/s/opaque-note",
        },
        {
            "authority": "act.x.mail.send",
            "required_line": "none",
            "delivered_node": None,
        },
        {
            "authority": "act.x.mail.config",
            "required_line": "connector-vault",
            "delivered_node": "/x/mail",
        },
    ]

    content_fence_rows = (
        ("exact valid section line", "valid covering chain", "readable and authorized"),
        ("exact valid section line", "no mandate chain", "refused as unauthorized"),
        ("no section line", "valid covering chain", "authorized but unreadable"),
        ("sibling section line", "valid covering chain", "unreadable"),
    )
    content_fence_cases = [
        {
            "key_material": key_material,
            "authority": authority,
            "expected": expected,
        }
        for key_material, authority, expected in content_fence_rows
    ]
    for case in content_fence_cases:
        actual = content_fence_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"content fence: expected {case['expected']}, got {actual}")

    self_state_cases = [
        {
            "operation": "create",
            "relation": "prior absence and new inclusion",
            "disclosed": ["sid", "before_commitment", "after_commitment"],
            "forbidden": ["name", "path", "title", "tags", "body", "folder_relation", "key"],
        },
        {
            "operation": "edit",
            "relation": "same-SID replacement",
            "disclosed": ["sid", "before_commitment", "after_commitment"],
            "forbidden": ["name", "path", "title", "tags", "body", "folder_relation", "key"],
        },
        {
            "operation": "delete",
            "relation": "prior inclusion and new absence",
            "disclosed": ["sid", "before_commitment", "after_commitment"],
            "forbidden": ["name", "path", "title", "tags", "body", "folder_relation", "key"],
        },
    ]

    current_authority_cases = [
        {
            "authority_change": change,
            "expected": "refused",
            "visible_state_digest": initial_digest,
            "gamma_delta": 0,
            "counter_delta": 0,
        }
        for change in ("expired", "revoked")
    ]
    atomic_refusal_cases = [
        {
            "defect": defect,
            "expected": "refused",
            "visible_state_digest": initial_digest,
            "reachable_candidate_artifacts": [],
        }
        for defect in (
            "missing mandate chain",
            "missing exact content line",
            "outside perimeter",
            "applicable constraint fails",
            "authority expired before effect",
            "authority revoked before effect",
        )
    ]

    gamma_read_cases = [
        {
            "actor": "owner",
            "authority": "owner local capability",
            "expected": "accepted",
            "mandate_required": False,
        },
        {
            "actor": "grantee",
            "authority": "read.gamma with covered content lines",
            "expected": "accepted within mandate dimensions",
            "mandate_required": True,
        },
        {
            "actor": "grantee",
            "authority": "content lines without read.gamma",
            "expected": "refused as unauthorized",
            "mandate_required": True,
        },
    ]

    historical = {
        name: sha256_hex((HERE / name).read_bytes())
        for name in HISTORICAL_FILES
    }
    return {
        "vector": "CB2-BUNDLE-AUTHORITY-FLOWS-1",
        "description": (
            "Independent pure-data CB8/CB9 oracle for owner parity, grantee "
            "zone/verb/selector decisions, exact key delivery, independent "
            "authority/decryption fences, delegated authorship/self evidence, "
            "current-authority rechecks, refusal atomicity and Gamma reads. "
            "No signed wire is introduced."
        ),
        "historical_vector_sha256": historical,
        "initial_state": initial_state,
        "initial_state_digest": initial_digest,
        "owner_cases": owner_cases,
        "grantee_cases": grantee_cases,
        "grant_delivery_cases": delivery_cases,
        "content_fence_cases": content_fence_cases,
        "delegated_evidence": {
            "public_authorship_required_members": [
                "subject",
                "zone",
                "sid",
                "content_hash",
                "operation_ref",
                "edition",
                "authorized_via",
                "key",
                "sig",
            ],
            "owner_signature_substitution": "refused",
            "self_state_cases": self_state_cases,
        },
        "current_authority_cases": current_authority_cases,
        "atomic_refusal_cases": atomic_refusal_cases,
        "gamma_read_cases": gamma_read_cases,
        "inventory": {
            "owner_case_count": len(owner_cases),
            "grantee_case_count": len(grantee_cases),
            "grant_delivery_case_count": len(delivery_cases),
            "content_fence_case_count": len(content_fence_cases),
            "self_state_case_count": len(self_state_cases),
            "current_authority_case_count": len(current_authority_cases),
            "atomic_refusal_case_count": len(atomic_refusal_cases),
            "gamma_read_case_count": len(gamma_read_cases),
            "future_owners": {
                "owner_parity_and_grants": "CB8",
                "delegated_content_and_gamma_reads": "CB9",
            },
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

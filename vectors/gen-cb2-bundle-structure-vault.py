#!/usr/bin/env python3
"""Independent CB2 oracle for structural, revocation and vault Bundle flows.

This pure-data oracle models the already-approved CB10 contracts. It does not
call Rust, mutate a Store, model an upstream connector effect or introduce wire.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-bundle-structure-vault.json"

HISTORICAL_FILES = (
    "g1-revocation.json",
    "g2-rotation.json",
    "cb2-operation-facts-mutation.json",
    "cb2-operation-facts-structural.json",
    "cb2-connector-catalog.json",
    "cb2-draft2-carriers.json",
    "cb2-bundle-boundaries.json",
    "cb2-bundle-authority-flows.json",
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


def structural_verdict(case: dict[str, Any]) -> str:
    operation = case["operation"]
    source = case.get("source_verb")
    destination = case.get("destination_verb")
    if operation == "list_read_folder":
        accepted = source in {"read", "edit", "append", "delete", "write"}
    elif operation == "create_child_folder":
        accepted = destination in {"append", "write"}
    elif operation == "rename_folder":
        accepted = source in {"edit", "append", "write"}
    elif operation == "delete_empty_folder":
        accepted = source in {"delete", "write"}
    elif operation == "move_folder":
        accepted = (
            source in {"edit", "append", "write"}
            and destination in {"append", "write"}
        )
    elif operation == "delete_nonempty_folder":
        accepted = (
            source in {"delete", "write"}
            and case.get("complete_subtree") is True
        )
    else:
        accepted = False
    return "accepted" if accepted else "refused"


def vault_access_verdict(case: dict[str, str]) -> str:
    authority = case["authority"]
    line = case["line"]
    if authority != "act.x.mail.config":
        return (
            "cannot open /x/mail"
            if authority == "act.x.calendar.config"
            else "refused as unauthorized"
        )
    if line == "exact /x/mail line":
        return "authorized and readable"
    if line == "no vault line":
        return "authorized but unreadable"
    return "unreadable"


def build_vector() -> dict[str, Any]:
    initial_state = {
        "manifest": "sha256:structure-vault-before",
        "gamma_head": "sha256:gamma-before",
        "circle_root": "sha256:circle-before",
        "vault_root": "sha256:vault-before",
        "mail_version": 4,
        "calendar_version": 7,
    }
    initial_digest = state_digest(initial_state)

    structural_rows: list[tuple[str, str | None, str | None, bool | None, str]] = []
    for verb, expected in (
        ("read", "accepted"),
        ("edit", "accepted"),
        ("append", "accepted"),
        ("delete", "accepted"),
        ("write", "accepted"),
    ):
        structural_rows.append(("list_read_folder", verb, None, None, expected))
    for verb, expected in (
        ("read", "refused"),
        ("edit", "refused"),
        ("delete", "refused"),
        ("append", "accepted"),
        ("write", "accepted"),
    ):
        structural_rows.append(("create_child_folder", None, verb, None, expected))
    for verb, expected in (
        ("read", "refused"),
        ("delete", "refused"),
        ("edit", "accepted"),
        ("append", "accepted"),
        ("write", "accepted"),
    ):
        structural_rows.append(("rename_folder", verb, None, None, expected))
    for verb, expected in (
        ("read", "refused"),
        ("edit", "refused"),
        ("append", "refused"),
        ("delete", "accepted"),
        ("write", "accepted"),
    ):
        structural_rows.append(("delete_empty_folder", verb, None, None, expected))
    structural_rows.extend(
        (
            ("move_folder", "edit", "append", None, "accepted"),
            ("move_folder", "append", "write", None, "accepted"),
            ("move_folder", "delete", "append", None, "refused"),
            ("move_folder", "edit", None, None, "refused"),
            ("delete_nonempty_folder", "delete", None, True, "accepted"),
            ("delete_nonempty_folder", "delete", None, False, "refused"),
        )
    )
    structural_cases = [
        {
            "id": f"structural-{index:02d}",
            "operation": operation,
            "source_verb": source,
            "destination_verb": destination,
            "complete_subtree": complete,
            "expected": expected,
        }
        for index, (operation, source, destination, complete, expected) in enumerate(
            structural_rows, start=1
        )
    ]
    for case in structural_cases:
        actual = structural_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    derived_cases = [
        {
            "operation": "tag_edit",
            "consequences": [
                "section index row",
                "removed tag wraps",
                "added tag wraps",
                "authorizing Gamma entry",
                "zone root",
                "manifest",
            ],
            "one_transaction": True,
        },
        {
            "operation": "move",
            "consequences": [
                "stable SID at destination",
                "source index removal",
                "destination index insertion",
                "fresh boundary key version",
                "survivor lines",
                "destination up-link wrap",
                "authorizing Gamma entry",
                "roots",
                "manifest",
            ],
            "old_parent_derives_future_key": False,
            "one_transaction": True,
        },
        {
            "operation": "subtree_delete",
            "consequences": [
                "folder row",
                "descendant rows",
                "descendant blob reachability",
                "descendant headers",
                "tag consequences",
                "authorizing Gamma entry",
                "roots",
                "manifest",
            ],
            "every_nonderived_removal_has_same_actor_chain": True,
            "one_transaction": True,
        },
    ]
    structural_failures = [
        {
            "failure": failure,
            "expected": "refused",
            "visible_state_digest": initial_digest,
        }
        for failure in (
            "destination outside the grantee perimeter",
            "move into the node's own descendant",
            "destination sibling name collision",
            "display path traversal outside the zone",
            "failure while rebuilding tag views",
            "failure while rotating or rewrapping",
            "failure before Gamma and manifest linearization",
        )
    ]
    self_structure_cases = [
        {
            "authority": "write.self",
            "operation": "create opaque SID",
            "expected": "accepted",
        },
        {
            "authority": "edit.self#id=opaque-node",
            "operation": "edit exact opaque SID",
            "expected": "accepted",
        },
        {
            "authority": "edit.self#dir=sealed",
            "operation": "edit claimed descendant",
            "expected": "refused",
        },
        {
            "authority": "delete.self#tag=private",
            "operation": "delete claimed tag match",
            "expected": "refused",
        },
    ]

    revocation_success = {
        "actor": "authorized manager",
        "steps": [
            "Core revocation verdict",
            "fresh protected node key",
            "survivor rewrap",
            "protected body re-encryption",
            "Gamma revoke occurrence",
            "roots and manifest publication",
        ],
        "linearization_count": 1,
        "revoked_line_opens_new_material": False,
        "fresh_keyless_store_verifies": True,
    }
    revocation_failures = [
        {
            "boundary": boundary,
            "expected": "refused",
            "visible_state_digest": initial_digest,
            "reachable_attempt_artifacts": [],
        }
        for boundary in (
            "revocation verdict",
            "fresh node key generation",
            "survivor rewrap",
            "body re-encryption",
            "Gamma append",
            "before manifest and roots linearization",
        )
    ]
    revocation_time_cases = [
        {
            "relative_time": "before revoked_at",
            "expected": "accepted",
            "state_source": "verified prior prefix",
        },
        {
            "relative_time": "at or after revoked_at",
            "expected": "refused",
            "state_source": "verified prior prefix",
        },
    ]

    config_crud_cases = [
        {
            "operation": operation,
            "authority": "act.x.mail.config",
            "line": "exact /x/mail line",
            "expected": "accepted",
            "external_mail_action_granted": False,
            "one_transaction": operation != "read",
        }
        for operation in ("read", "create", "edit", "delete")
    ]
    vault_access_rows = (
        ("act.x.mail.config", "exact /x/mail line", "authorized and readable"),
        ("act.x.mail.config", "no vault line", "authorized but unreadable"),
        ("act.x.mail.config", "generic /x root line", "unreadable"),
        ("act.x.mail.config", "/x/calendar line", "unreadable"),
        ("no config authority", "exact /x/mail line", "refused as unauthorized"),
        ("act.x.mail.*", "exact /x/mail line", "refused as unauthorized"),
        ("act.x.calendar.config", "/x/calendar line", "cannot open /x/mail"),
    )
    vault_access_cases = [
        {"authority": authority, "line": line, "expected": expected}
        for authority, line, expected in vault_access_rows
    ]
    for case in vault_access_cases:
        actual = vault_access_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"vault access: expected {case['expected']}, got {actual}")

    vault_atomic_cases = [
        {
            "operation": operation,
            "changed_connector": "mail",
            "unchanged_connector": "calendar",
            "credential_in_keyless_output": False,
            "one_transaction": True,
        }
        for operation in (
            "valid config edit",
            "recipient revocation and rotation",
            "local update after out-of-protocol upstream replacement",
        )
    ]
    vault_failures = [
        {
            "defect": defect,
            "expected": "refused",
            "visible_state_digest": initial_digest,
            "reachable_attempt_artifacts": [],
        }
        for defect in (
            "missing exact config authority",
            "missing exact vault line",
            "cross-connector line",
            "applicable constraint or obligation fails",
            "injected failure before local commit",
        )
    ]

    historical = {
        name: sha256_hex((HERE / name).read_bytes())
        for name in HISTORICAL_FILES
    }
    return {
        "vector": "CB2-BUNDLE-STRUCTURE-VAULT-1",
        "description": (
            "Independent pure-data CB10 oracle for structural authority, derived "
            "consequences, effect-free failures, opaque self structure, atomic "
            "revocation/rotation, exact connector config authority and isolated "
            "vault access. No upstream effect, signed wire or new verb is modeled."
        ),
        "historical_vector_sha256": historical,
        "initial_state": initial_state,
        "initial_state_digest": initial_digest,
        "structural": {
            "authority_cases": structural_cases,
            "covered_read_hides_siblings": True,
            "derived_cases": derived_cases,
            "failure_cases": structural_failures,
            "self_cases": self_structure_cases,
            "new_wire_verb": False,
        },
        "revocation": {
            "success": revocation_success,
            "failure_cases": revocation_failures,
            "time_cases": revocation_time_cases,
        },
        "vault": {
            "config_is_outside_business_classes": True,
            "wildcard_covers_config": False,
            "inferred_binding_or_cosign": False,
            "crud_cases": config_crud_cases,
            "access_cases": vault_access_cases,
            "capability_substitution_cases": [
                {
                    "held": "audit sealed action arguments",
                    "requested": "open /x/mail config",
                    "expected": "refused",
                },
                {
                    "held": "open /x/mail config",
                    "requested": "audit sealed action arguments",
                    "expected": "refused",
                },
            ],
            "atomic_cases": vault_atomic_cases,
            "failure_cases": vault_failures,
            "public_forbidden": [
                "credential",
                "config plaintext",
                "private key",
                "DK",
            ],
            "normative_header_lines": "opaque and non-authorizing",
            "network_participates": False,
            "upstream_effect_is_modeled": False,
        },
        "inventory": {
            "structural_authority_case_count": len(structural_cases),
            "structural_derived_case_count": len(derived_cases),
            "structural_failure_case_count": len(structural_failures),
            "self_structure_case_count": len(self_structure_cases),
            "revocation_failure_case_count": len(revocation_failures),
            "revocation_time_case_count": len(revocation_time_cases),
            "vault_crud_case_count": len(config_crud_cases),
            "vault_access_case_count": len(vault_access_cases),
            "vault_atomic_case_count": len(vault_atomic_cases),
            "vault_failure_case_count": len(vault_failures),
            "future_owner": "CB10",
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

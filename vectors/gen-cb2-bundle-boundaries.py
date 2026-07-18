#!/usr/bin/env python3
"""Independent CB2 oracle for Bundle transaction and local trust boundaries.

This oracle is intentionally implementation-independent. It models the approved
CB1 G-B/G-C/G-D contracts as pure data:

* old/overlay/new snapshots and one logical linearization point;
* deterministic recovery with no partially visible generation;
* display-path and canonical Store-key confinement, including Fs indirection;
* purpose/context-bound opaque capability substitution decisions;
* local export into a fresh keyless MemStore/FsStore and fail-closed defects.

It imports no Rust code, performs no filesystem mutation, and defines no signed
wire or stable capability encoding.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-bundle-boundaries.json"

HISTORICAL_FILES = (
    "a1-genesis.json",
    "a2-did.json",
    "cb2-bundle-version-coexistence.json",
    "cb2-draft2-carriers.json",
    "h1-merkle.json",
    "h2-gamma-roots.json",
    "i1-concurrency.json",
)

NAME_RE = re.compile(r"^[a-z0-9_-]{1,64}$")
SID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")
HASH_RE = re.compile(r"^[0-9a-f]{64}$")
CONNECTOR_RE = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")


def clone(value: Any) -> Any:
    return copy.deepcopy(value)


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


def snapshot_digest(objects: dict[str, str]) -> str:
    return "sha256:" + sha256_hex(jcs(objects).encode())


def display_path_accepted(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    if value.startswith(("/", "\\")) or "\\" in value or "\x00" in value:
        return False
    segments = value.split("/")
    return all(
        segment not in {"", ".", ".."} and NAME_RE.fullmatch(segment)
        for segment in segments
    )


def store_key_accepted(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    if value.startswith(("/", "\\")) or "\\" in value or "\x00" in value:
        return False
    segments = value.split("/")
    if any(segment in {"", ".", ".."} for segment in segments):
        return False

    if value in {"manifest.json", "did.json"}:
        return True
    if value in {
        "e/public/index.json",
        "e/circle/index.json",
        "e/self/index.json",
        "gamma/gamma.jsonl",
    }:
        return True
    if value.startswith("e/public/") and value.endswith(".md"):
        relative = value[len("e/public/") : -len(".md")]
        return display_path_accepted(relative)
    if len(segments) == 4 and segments[:3] in (
        ["e", "circle", "blobs"],
        ["e", "self", "blobs"],
    ):
        return bool(
            segments[3].endswith(".enc")
            and SID_RE.fullmatch(segments[3][:-len(".enc")])
        )
    if len(segments) == 4 and segments[:3] in (
        ["e", "circle", "hdr"],
        ["e", "self", "hdr"],
    ):
        stem = segments[3][:-len(".json")] if segments[3].endswith(".json") else ""
        return bool(stem and (SID_RE.fullmatch(stem) or stem == "root"))
    if len(segments) == 2 and segments[0] == "certs":
        stem = segments[1][:-len(".json")] if segments[1].endswith(".json") else ""
        return bool(MANDATE_RE.fullmatch(stem))
    if len(segments) == 2 and segments[0] == "gamma":
        return bool(
            segments[1] == "gamma.jsonl"
            or re.fullmatch(r"\d{4}-\d{2}\.jsonl", segments[1])
        )
    if segments[0] == "manifests" and len(segments) == 2:
        return bool(
            re.fullmatch(r"\d+\.json", segments[1])
            or re.fullmatch(r"tree-\d+\.json", segments[1])
            or re.fullmatch(r"index-(public|circle|self)-\d+\.json", segments[1])
        )
    if segments[0] in {"changesets", "evidence"} and len(segments) == 2:
        stem = segments[1][:-len(".json")] if segments[1].endswith(".json") else ""
        return bool(HASH_RE.fullmatch(stem))
    if segments[0] == "x" and len(segments) >= 3:
        return bool(
            CONNECTOR_RE.fullmatch(segments[1])
            and all(NAME_RE.fullmatch(segment) for segment in segments[2:-1])
            and re.fullmatch(r"[a-z0-9_-]{1,64}\.(enc|json)", segments[-1])
        )
    return False


def confinement_verdict(case: dict[str, Any]) -> str:
    kind = case["input_kind"]
    value = case["value"]
    if kind == "display_path":
        accepted = display_path_accepted(value)
    elif kind in {"store_key", "cold_load_key", "recovery_key"}:
        accepted = store_key_accepted(value)
    else:
        return "refused"
    if case["resolved_outside_root"]:
        accepted = False
    return "accepted" if accepted else "refused"


def capability_verdict(
    capability: dict[str, Any],
    request: dict[str, Any],
) -> str:
    if set(capability) != {"class", "context", "opaque_fixture_label"}:
        return "refused"
    if set(request) != {"class", "context", "protocol_object"}:
        return "refused"
    if capability["class"] != request["class"]:
        return "refused"
    if capability["context"] != request["context"]:
        return "refused"
    expected_object = {
        "sign_manifest": "edition_manifest",
        "sign_gamma": "gamma_entry",
        "open_body": "sealed_body",
        "wrap_header": "header_line",
        "audit_args": "sealed_action_args",
        "open_config": "vault_config",
    }.get(capability["class"])
    return "accepted" if request["protocol_object"] == expected_object else "refused"


def contains_secret_shape(value: Any) -> bool:
    forbidden_names = {
        "seed",
        "private_key",
        "secret_key",
        "owner_keys",
        "dk",
        "credential",
        "plaintext",
        "capability",
    }
    if isinstance(value, dict):
        return any(
            name.lower() in forbidden_names or contains_secret_shape(item)
            for name, item in value.items()
        )
    if isinstance(value, list):
        return any(contains_secret_shape(item) for item in value)
    return False


def build_write_set(
    old: dict[str, str],
    new: dict[str, str],
) -> list[dict[str, Any]]:
    rows = []
    for path in sorted(set(old) | set(new)):
        before = (
            {"state": "present", "sha256": sha256_hex(old[path].encode())}
            if path in old
            else {"state": "absent"}
        )
        after = (
            {"state": "present", "sha256": sha256_hex(new[path].encode())}
            if path in new
            else {"state": "absent"}
        )
        if before != after:
            rows.append({"path": path, "before": before, "after": after})
    return rows


def build_vector() -> dict[str, Any]:
    old_snapshot = {
        "did.json": '{"id":"did:aithos:fixture"}',
        "e/circle/index.json": '{"sections":[]}',
        "e/self/index.json": '{"blobs":[]}',
        "gamma/2026-07.jsonl": '{"id":"gamma_old"}\n',
        "manifest.json": '{"edition":{"height":1},"gamma_head":"sha256:old"}',
        "manifests/1.json": '{"edition":{"height":1},"gamma_head":"sha256:old"}',
    }
    new_snapshot = {
        **old_snapshot,
        "e/circle/index.json": '{"sections":[{"sid":"01K00000000000000000000081"}]}',
        "e/circle/blobs/01K00000000000000000000081.enc": "opaque-ciphertext-v1",
        "gamma/2026-07.jsonl": (
            '{"id":"gamma_old"}\n'
            '{"id":"gamma_01K00000000000000000000081"}\n'
        ),
        "manifest.json": '{"edition":{"height":2},"gamma_head":"sha256:new"}',
        "manifests/2.json": '{"edition":{"height":2},"gamma_head":"sha256:new"}',
    }
    write_set = build_write_set(old_snapshot, new_snapshot)

    failure_boundaries = [
        ("MemStore", "cryptography"),
        ("MemStore", "blob preparation"),
        ("MemStore", "index preparation"),
        ("MemStore", "header or wrap"),
        ("MemStore", "Gamma validation"),
        ("MemStore", "before state replacement"),
        ("FsStore", "cryptography"),
        ("FsStore", "blob preparation"),
        ("FsStore", "index preparation"),
        ("FsStore", "header or wrap"),
        ("FsStore", "Gamma validation"),
        ("FsStore", "before commit marker or reference"),
    ]
    failure_cases = [
        {
            "id": f"{store.lower()}-{index:02d}",
            "store": store,
            "boundary": boundary,
            "visible_snapshot": "old",
            "canonical_digest": snapshot_digest(old_snapshot),
            "staging_is_canonical": False,
        }
        for index, (store, boundary) in enumerate(failure_boundaries, start=1)
    ]
    recovery_cases = [
        {
            "id": "no-staging",
            "internal_state": "old generation only",
            "visible_snapshot": "old",
            "scratch_resolution": "none",
        },
        {
            "id": "prepared-not-linearized",
            "internal_state": "complete new generation prepared, old reference durable",
            "visible_snapshot": "old",
            "scratch_resolution": "discard new scratch",
        },
        {
            "id": "linearization-reference-durable",
            "internal_state": "complete new generation and new reference durable",
            "visible_snapshot": "new",
            "scratch_resolution": "retain new generation",
        },
        {
            "id": "acknowledgement-lost",
            "internal_state": "new reference durable, caller did not receive success",
            "visible_snapshot": "new",
            "scratch_resolution": "discover outcome from manifest and Gamma head",
        },
    ]

    accepted_paths = [
        {
            "id": "display-relative",
            "input_kind": "display_path",
            "value": "projets/perso/note",
            "resolved_outside_root": False,
            "expected": "accepted",
        },
        {
            "id": "public-markdown",
            "input_kind": "store_key",
            "value": "e/public/docs/readme.md",
            "resolved_outside_root": False,
            "expected": "accepted",
        },
        {
            "id": "circle-blob",
            "input_kind": "store_key",
            "value": "e/circle/blobs/01K00000000000000000000081.enc",
            "resolved_outside_root": False,
            "expected": "accepted",
        },
        {
            "id": "changeset-sidecar",
            "input_kind": "cold_load_key",
            "value": "changesets/" + "a1" * 32 + ".json",
            "resolved_outside_root": False,
            "expected": "accepted",
        },
        {
            "id": "connector-vault-object",
            "input_kind": "store_key",
            "value": "x/mail/config/oauth.enc",
            "resolved_outside_root": False,
            "expected": "accepted",
        },
    ]
    refused_paths = [
        ("display-parent", "display_path", "../circle/secret", False),
        ("display-absolute", "display_path", "/absolute/section", False),
        ("display-dot", "display_path", "folder/./section", False),
        ("display-empty-segment", "display_path", "folder//section", False),
        ("display-backslash", "display_path", r"folder\section", False),
        ("display-symlink-out", "display_path", "folder/link-out/section", True),
        ("store-parent", "store_key", "../../outside", False),
        ("store-absolute", "store_key", "/etc/passwd", False),
        ("store-unlisted", "store_key", "e/circle/unlisted-object.json", False),
        (
            "store-intermediate-symlink-out",
            "store_key",
            "e/circle/hdr/01K00000000000000000000081.json",
            True,
        ),
        (
            "store-final-symlink-out",
            "store_key",
            "e/circle/index.json",
            True,
        ),
        ("store-backslash", "store_key", r"e\circle\index.json", False),
        ("cold-manifest-symlink-out", "cold_load_key", "manifest.json", True),
        (
            "recovery-generation-symlink-out",
            "recovery_key",
            "manifests/2.json",
            True,
        ),
        (
            "store-unknown-root",
            "store_key",
            "outside/accepted-looking.json",
            False,
        ),
    ]
    path_cases = accepted_paths + [
        {
            "id": identifier,
            "input_kind": kind,
            "value": value,
            "resolved_outside_root": outside,
            "expected": "refused",
        }
        for identifier, kind, value, outside in refused_paths
    ]
    for case in path_cases:
        actual = confinement_verdict(case)
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    subject = "did:aithos:z6MkBundleBoundaryFixture"
    manifest_capability = {
        "class": "sign_manifest",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "owner",
            "purpose": "edition-manifest",
            "domain": "aithos-core/v1/manifest",
        },
        "opaque_fixture_label": "manifest-capability-A",
    }
    gamma_capability = {
        "class": "sign_gamma",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "owner",
            "purpose": "gamma-entry",
            "domain": "aithos-core/v1/gamma-entry",
        },
        "opaque_fixture_label": "gamma-capability-A",
    }
    body_capability = {
        "class": "open_body",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "owner",
            "purpose": "sealed-body",
            "node": "/e/circle/d/01K00000000000000000000081",
            "key_version": 3,
        },
        "opaque_fixture_label": "body-capability-A",
    }
    wrap_capability = {
        "class": "wrap_header",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "owner",
            "purpose": "header-line",
            "node": "/e/circle/d/01K00000000000000000000081",
            "key_version": 3,
            "recipient": "z6MkRecipientFixture",
        },
        "opaque_fixture_label": "wrap-capability-A",
    }
    audit_capability = {
        "class": "audit_args",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "auditor",
            "purpose": "sealed-action-args",
            "connector": "mail",
            "mandate_chain": ["mandate_01K00000000000000000000081"],
        },
        "opaque_fixture_label": "audit-capability-A",
    }
    config_capability = {
        "class": "open_config",
        "context": {
            "subject": subject,
            "ethos": subject,
            "actor": "config-manager",
            "purpose": "vault-config",
            "connector": "mail",
            "node": "/x/mail",
            "key_version": 4,
            "mandate_chain": ["mandate_01K00000000000000000000082"],
        },
        "opaque_fixture_label": "config-capability-A",
    }
    capabilities = [
        manifest_capability,
        gamma_capability,
        body_capability,
        wrap_capability,
        audit_capability,
        config_capability,
    ]
    object_for_class = {
        "sign_manifest": "edition_manifest",
        "sign_gamma": "gamma_entry",
        "open_body": "sealed_body",
        "wrap_header": "header_line",
        "audit_args": "sealed_action_args",
        "open_config": "vault_config",
    }
    capability_positive_cases = [
        {
            "id": capability["class"] + "-exact",
            "capability": capability,
            "request": {
                "class": capability["class"],
                "context": clone(capability["context"]),
                "protocol_object": object_for_class[capability["class"]],
            },
            "expected": "accepted",
        }
        for capability in capabilities
    ]
    capability_negative_cases = []
    for index, capability in enumerate(capabilities):
        other = capabilities[(index + 1) % len(capabilities)]
        exact_request = capability_positive_cases[index]["request"]
        capability_negative_cases.extend(
            [
                {
                    "id": capability["class"] + "-cross-class",
                    "capability": capability,
                    "request": {
                        "class": other["class"],
                        "context": clone(other["context"]),
                        "protocol_object": object_for_class[other["class"]],
                    },
                    "expected": "refused",
                },
                {
                    "id": capability["class"] + "-cross-actor",
                    "capability": capability,
                    "request": {
                        **clone(exact_request),
                        "context": {
                            **clone(exact_request["context"]),
                            "actor": "different-actor",
                        },
                    },
                    "expected": "refused",
                },
                {
                    "id": capability["class"] + "-arbitrary-object",
                    "capability": capability,
                    "request": {
                        **clone(exact_request),
                        "protocol_object": "arbitrary_bytes",
                    },
                    "expected": "refused",
                },
            ]
        )
    for case in capability_positive_cases + capability_negative_cases:
        actual = capability_verdict(case["capability"], case["request"])
        if actual != case["expected"]:
            raise AssertionError(f"{case['id']}: expected {case['expected']}, got {actual}")

    exported_objects = {
        "did.json": '{"id":"did:aithos:keyless-fixture"}',
        "certs/mandate_01K00000000000000000000081.json": '{"signed":true}',
        "changesets/" + "a1" * 32 + ".json": '{"changes":[]}',
        "evidence/" + "b2" * 32 + ".json": '{"items":[]}',
        "gamma/2026-07.jsonl": '{"id":"gamma_public_fixture"}\n',
        "manifests/1.json": '{"edition":{"height":1}}',
        "manifests/2.json": '{"edition":{"height":2}}',
        "manifest.json": '{"edition":{"height":2}}',
        "e/circle/blobs/01K00000000000000000000081.enc": "opaque-ciphertext",
        "e/self/blobs/01K00000000000000000000082.enc": "opaque-ciphertext",
    }
    if contains_secret_shape(exported_objects):
        raise AssertionError("keyless export contains a forbidden secret shape")
    cold_cases = [
        {
            "id": "memstore-complete",
            "store": "MemStore",
            "defect": "none",
            "expected": "accepted",
        },
        {
            "id": "fsstore-complete-after-producer-destroyed",
            "store": "FsStore",
            "defect": "none",
            "expected": "accepted",
        },
        {
            "id": "missing-certificate",
            "store": "FsStore",
            "defect": "required mandate certificate missing",
            "expected": "refused",
        },
        {
            "id": "substituted-certificate",
            "store": "FsStore",
            "defect": "certificate bytes substituted",
            "expected": "refused",
        },
        {
            "id": "truncated-gamma",
            "store": "FsStore",
            "defect": "one Gamma entry missing",
            "expected": "refused",
        },
        {
            "id": "wrong-parent",
            "store": "FsStore",
            "defect": "expected parent manifest substituted",
            "expected": "refused",
        },
        {
            "id": "unpinned-object",
            "store": "FsStore",
            "defect": "uncommitted public artifact added",
            "expected": "refused",
        },
        {
            "id": "private-capability-in-export",
            "store": "FsStore",
            "defect": "opaque local capability exported",
            "expected": "refused",
        },
    ]

    historical = {
        name: sha256_hex((HERE / name).read_bytes())
        for name in HISTORICAL_FILES
    }
    return {
        "vector": "CB2-BUNDLE-BOUNDARIES-1",
        "description": (
            "Independent pure-data oracle for CB1 G-B/G-C/G-D: deterministic "
            "Bundle overlay/write-set/linearization and recovery, display/Store "
            "path confinement, typed opaque capability substitution, and fresh "
            "local keyless export/reopen decisions. No signed wire or stable "
            "capability encoding is introduced."
        ),
        "historical_vector_sha256": historical,
        "transaction": {
            "old_snapshot": old_snapshot,
            "old_snapshot_digest": snapshot_digest(old_snapshot),
            "new_snapshot": new_snapshot,
            "new_snapshot_digest": snapshot_digest(new_snapshot),
            "write_set": write_set,
            "failure_cases": failure_cases,
            "recovery_cases": recovery_cases,
            "linearization_count": 1,
            "staging_outside_canonical_namespace": True,
            "internal_generation_metadata_is_not_wire": True,
        },
        "confinement": {
            "cases": path_cases,
            "accepted_count": sum(
                case["expected"] == "accepted" for case in path_cases
            ),
            "refused_count": sum(
                case["expected"] == "refused" for case in path_cases
            ),
            "signed_manifest_cannot_authorize_escape": True,
            "check_applies_to": [
                "read",
                "write",
                "list",
                "cold_load",
                "staging_publication",
                "recovery",
            ],
        },
        "capabilities": {
            "positive_cases": capability_positive_cases,
            "negative_cases": capability_negative_cases,
            "no_stable_encoding_promoted": True,
            "no_generic_sign_open_wrap_oracle": True,
            "no_raw_seed_private_key_dk_or_credential": True,
            "session_binding": {
                "one_ethos": True,
                "one_actor": True,
                "one_grantee_chain": True,
                "ambient_capability_pool": False,
            },
        },
        "keyless_export": {
            "objects": exported_objects,
            "export_digest": snapshot_digest(exported_objects),
            "cold_cases": cold_cases,
            "producer_destroyed_before_fs_reopen": True,
            "private_capabilities_removed_before_verify": True,
            "network_participates": False,
            "provider_cas_participates": False,
            "secret_shape_detected": contains_secret_shape(exported_objects),
        },
        "inventory": {
            "failure_boundary_count": len(failure_cases),
            "recovery_case_count": len(recovery_cases),
            "path_case_count": len(path_cases),
            "capability_positive_count": len(capability_positive_cases),
            "capability_negative_count": len(capability_negative_cases),
            "cold_case_count": len(cold_cases),
            "future_owners": {
                "transaction_and_confinement": "CB7",
                "capability_and_keyless_export": "CB12",
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

#!/usr/bin/env python3
"""Independent CB2 oracle for the closed K1-C draft2 carrier wire.

The oracle uses Python cryptography, hashlib and blake3. It never imports or
executes the Rust implementation. It assembles one coherent draft2 publication
from exact W1 references, a derived commitment-only changeset, all five evidence
item variants, a D7 delegated-counts root, two canonical sidecars, and the signed
manifest that pins them. Mono-defect candidates lock the approved
InvalidOperation / InvalidDidDocument boundary.

The host Python supplies cryptography. The one D7 BLAKE3 leaf is delegated to
the cached offline Python 3.12 wheel, matching the other CB2 D7/Gamma oracles.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from typing import Any, Callable, NoReturn

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-draft2-carriers.json"

DRAFT1 = "1.0.0-draft.1"
DRAFT2 = "1.0.0-draft.2"
OPERATION_PROFILE = DRAFT1
FACTS_PROFILE = DRAFT1
CHANGESET_PROFILE = DRAFT1
EVIDENCE_PROFILE = DRAFT1
AUTHORSHIP_PROFILE = DRAFT1
PRESENTATION_PROFILE = DRAFT1
DELEGATED_COUNTS_PROFILE = DRAFT1

OPERATION_DOMAIN = "aithos-core/v1/operation-commitment"
FACTS_DOMAIN = "aithos-core/v1/operation-facts"
CHANGESET_DOMAIN = "aithos-core/v1/changeset"
EVIDENCE_DOMAIN = "aithos-core/v1/evidence"
STATE_KEY_DOMAIN = "aithos-core/v1/state-key"
STATE_BYTES_DOMAIN = "aithos-core/v1/state-bytes"
STATE_FACT_DOMAIN = "aithos-core/v1/state-fact"
GAMMA_REQUEST_DOMAIN = "aithos-core/v1/gamma-read-request"

INVALID_OPERATION = "InvalidOperation"
INVALID_DID = "InvalidDidDocument"

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
FIELD_P = 2**255 - 19
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
BARE_HASH_RE = re.compile(r"^[0-9a-f]{64}$")
SIG_RE = re.compile(r"^[0-9a-f]{128}$")
OP_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")

ROOT_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("11" * 32))
CONTENT_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("22" * 32))
GRANTEE_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("33" * 32))
SESSION_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("44" * 32))
CATALOG_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("55" * 32))
RECEIPT_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("66" * 32))
STRANGER_KEY = Ed25519PrivateKey.from_private_bytes(bytes.fromhex("77" * 32))

HISTORICAL_FILES = (
    "cb2-bundle-version-coexistence.json",
    "cb2-connector-catalog.json",
    "cb2-delegated-counts.json",
    "cb2-gamma-v2-replay.json",
    "cb2-operation-facts-action-inference.json",
    "cb2-operation-facts-mutation.json",
    "cb2-operation-facts-read.json",
    "cb2-operation-facts-structural.json",
    "cb2-operation-projection.json",
    "cb2-operation-receipts.json",
    "cb2-session-proof.json",
)

LEAF_DOMAIN = b"aithos-core/v1/delegated-counts-leaf\x00"


class ProtocolError(ValueError):
    code = "ProtocolError"


class OperationError(ProtocolError):
    code = INVALID_OPERATION


class DidError(ProtocolError):
    code = INVALID_DID


def reject_operation(detail: str) -> NoReturn:
    raise OperationError(detail)


def reject_did(detail: str) -> NoReturn:
    raise DidError(detail)


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


def sha256_text(payload: bytes) -> str:
    return "sha256:" + sha256_hex(payload)


def commitment(domain: str, payload: bytes) -> str:
    return sha256_text(domain.encode() + b"\x00" + payload)


def blake3_hex(payload: bytes) -> str:
    environment = os.environ.copy()
    environment.setdefault("UV_CACHE_DIR", "/private/tmp/aithos-cb2-uv-cache")
    process = subprocess.run(
        [
            "uv",
            "run",
            "--offline",
            "--no-project",
            "--python",
            "3.12",
            "--with",
            "blake3",
            "python",
            "-c",
            (
                "import blake3,sys;"
                "sys.stdout.write(blake3.blake3(bytes.fromhex("
                "sys.stdin.read())).hexdigest())"
            ),
        ],
        input=payload.hex(),
        text=True,
        capture_output=True,
        check=True,
        env=environment,
    )
    return process.stdout


def require_exact(
    value: Any,
    keys: set[str],
    label: str,
    reject: Callable[[str], NoReturn] = reject_operation,
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        reject(f"{label} has a non-exact member set")
    if any(item is None for item in value.values()):
        reject(f"{label} contains null")
    return value


def base58(payload: bytes) -> str:
    number = int.from_bytes(payload, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58[remainder] + encoded
    zeros = len(payload) - len(payload.lstrip(b"\x00"))
    return "1" * zeros + (encoded or ("" if zeros else "1"))


def base58_decode(value: str) -> bytes:
    if not value:
        raise ValueError("empty base58")
    number = 0
    for char in value:
        if char not in BASE58:
            raise ValueError("invalid base58")
        number = number * 58 + BASE58.index(char)
    body = (
        number.to_bytes((number.bit_length() + 7) // 8, "big")
        if number
        else b""
    )
    return b"\x00" * (len(value) - len(value.lstrip("1"))) + body


def public_bytes(key: Ed25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def multibase_ed(key: Ed25519PrivateKey) -> str:
    return "z" + base58(b"\xed\x01" + public_bytes(key))


def multibase_x(payload: bytes) -> str:
    return "z" + base58(b"\xec\x01" + payload)


def ed25519_to_x25519_public(payload: bytes) -> bytes:
    encoded_y = bytearray(payload)
    encoded_y[31] &= 0x7F
    y = int.from_bytes(encoded_y, "little")
    if y >= FIELD_P:
        raise AssertionError("invalid fixture Ed25519 y coordinate")
    denominator = (1 - y) % FIELD_P
    if denominator == 0:
        raise AssertionError("invalid fixture Ed25519 conversion")
    u = ((1 + y) * pow(denominator, FIELD_P - 2, FIELD_P)) % FIELD_P
    return u.to_bytes(32, "little")


def decode_ed(value: str) -> Ed25519PublicKey:
    try:
        decoded = base58_decode(value[1:]) if value.startswith("z") else b""
    except ValueError:
        reject_operation("invalid Ed25519 multibase key")
    if len(decoded) != 34 or decoded[:2] != b"\xed\x01":
        reject_operation("invalid Ed25519 multibase key")
    return Ed25519PublicKey.from_public_bytes(decoded[2:])


def sign_without_member(
    value: dict[str, Any],
    member: str,
    key: Ed25519PrivateKey,
) -> None:
    unsigned = {name: item for name, item in value.items() if name != member}
    value[member] = key.sign(jcs(unsigned).encode()).hex()


def verify_without_member(
    value: dict[str, Any],
    member: str,
    public_key: str,
) -> None:
    signature = value.get(member)
    if not isinstance(signature, str) or not SIG_RE.fullmatch(signature):
        reject_operation(f"{member} is not a lowercase Ed25519 signature")
    unsigned = {name: item for name, item in value.items() if name != member}
    try:
        decode_ed(public_key).verify(bytes.fromhex(signature), jcs(unsigned).encode())
    except InvalidSignature:
        reject_operation(f"{member} does not verify")


def sign_signature_block(
    value: dict[str, Any],
    key: Ed25519PrivateKey,
) -> None:
    unsigned = clone(value)
    unsigned["signature"]["value"] = ""
    value["signature"]["value"] = key.sign(jcs(unsigned).encode()).hex()


def verify_signature_block(
    value: dict[str, Any],
    public_key: str,
    *,
    did_boundary: bool = False,
) -> None:
    reject = reject_did if did_boundary else reject_operation
    signature = require_exact(
        value.get("signature"),
        {"alg", "key", "value"},
        "signature",
        reject,
    )
    if signature["alg"] != "ed25519":
        reject("signature algorithm is not ed25519")
    if not isinstance(signature["value"], str) or not SIG_RE.fullmatch(
        signature["value"]
    ):
        reject("signature value is malformed")
    unsigned = clone(value)
    unsigned["signature"]["value"] = ""
    try:
        decoded = base58_decode(public_key[1:]) if public_key.startswith("z") else b""
        if len(decoded) != 34 or decoded[:2] != b"\xed\x01":
            reject("signature key is not Ed25519")
        verifier = Ed25519PublicKey.from_public_bytes(decoded[2:])
        verifier.verify(
            bytes.fromhex(signature["value"]),
            jcs(unsigned).encode(),
        )
    except (InvalidSignature, ValueError):
        reject("signature does not verify")


def fixture_ulid(ordinal: int) -> str:
    value = f"01K{'0' * 21}{ordinal:02d}"
    if len(value) != 26:
        raise AssertionError("fixture ULID length")
    return value


def operation_projection(
    ordinal: int,
    kind: str,
    facts_ref: dict[str, Any],
    *,
    at: str,
    subject: str,
    authority_ref: dict[str, str],
    history_head: str,
) -> dict[str, Any]:
    return {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": "op_" + fixture_ulid(ordinal),
        "subject": subject,
        "at": at,
        "authority": {
            "actor": "grantee",
            "authorized_by": authority_ref["id"],
            "authorized_via": [authority_ref],
            "key": multibase_ed(GRANTEE_KEY),
        },
        "history_heads": [history_head],
        "operation": {"kind": kind, "facts_ref": facts_ref},
    }


def operation_ref(projection: dict[str, Any]) -> dict[str, Any]:
    return {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            OPERATION_DOMAIN,
            jcs(projection).encode(),
        ),
    }


def validate_operation_ref(value: Any) -> dict[str, Any]:
    value = require_exact(
        value,
        {"aithos-operation-core", "occurrence", "commitment"},
        "operation reference",
    )
    if value["aithos-operation-core"] != OPERATION_PROFILE:
        reject_operation("unknown operation-reference profile")
    if not isinstance(value["occurrence"], str) or not OP_RE.fullmatch(
        value["occurrence"]
    ):
        reject_operation("malformed operation occurrence")
    if not isinstance(value["commitment"], str) or not DIGEST_RE.fullmatch(
        value["commitment"]
    ):
        reject_operation("malformed operation commitment")
    return value


def state_value(payload: str | None) -> dict[str, str]:
    if payload is None:
        return {"state": "absent"}
    return {
        "state": "present",
        "byte_commitment": commitment(STATE_BYTES_DOMAIN, payload.encode()),
    }


def state_fact(store_objects: dict[str, str]) -> tuple[dict[str, Any], dict[str, Any]]:
    document = {
        "aithos-state-fact-core": DRAFT1,
        "objects": sorted(
            [
                {
                    "key_commitment": commitment(STATE_KEY_DOMAIN, key.encode()),
                    "byte_commitment": commitment(
                        STATE_BYTES_DOMAIN,
                        payload.encode(),
                    ),
                }
                for key, payload in store_objects.items()
            ],
            key=lambda item: item["key_commitment"],
        ),
    }
    reference = {
        "aithos-state-fact-core": DRAFT1,
        "digest": commitment(STATE_FACT_DOMAIN, jcs(document).encode()),
    }
    return document, reference


def derive_changes(
    before: dict[str, str],
    after: dict[str, str],
    operations: list[dict[str, Any]],
    causes: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    keys = sorted(set(before) | set(after))
    changes = []
    for key in keys:
        before_state = state_value(before.get(key))
        after_state = state_value(after.get(key))
        if before_state == after_state:
            continue
        if key not in causes or causes[key] not in operations:
            raise AssertionError(f"missing contained last-writer for {key}")
        changes.append(
            {
                "key_commitment": commitment(STATE_KEY_DOMAIN, key.encode()),
                "before": before_state,
                "after": after_state,
                "operation_ref": causes[key],
            }
        )
    return sorted(
        changes,
        key=lambda item: (
            item["key_commitment"],
            item["operation_ref"]["occurrence"],
        ),
    )


def validate_state(value: Any) -> None:
    if not isinstance(value, dict) or value.get("state") not in {"absent", "present"}:
        reject_operation("invalid changeset state")
    expected = {"state"} if value["state"] == "absent" else {"state", "byte_commitment"}
    require_exact(value, expected, "changeset state")
    if value["state"] == "present" and (
        not isinstance(value["byte_commitment"], str)
        or not DIGEST_RE.fullmatch(value["byte_commitment"])
    ):
        reject_operation("invalid changeset byte commitment")


def validate_changeset(candidate: dict[str, Any], context: dict[str, Any]) -> None:
    changeset = require_exact(
        candidate,
        {
            "aithos-changeset-core",
            "height",
            "predecessors",
            "operations",
            "changes",
        },
        "changeset",
    )
    if changeset["aithos-changeset-core"] != CHANGESET_PROFILE:
        reject_operation("unknown changeset profile")
    if changeset["height"] != context["height"]:
        reject_operation("changeset height mismatch")
    if changeset["predecessors"] != context["predecessors"]:
        reject_operation("changeset predecessors mismatch")
    if not isinstance(changeset["operations"], list):
        reject_operation("changeset operations is not an array")
    for ref in changeset["operations"]:
        validate_operation_ref(ref)
    if changeset["operations"] != context["contained_operations"]:
        reject_operation("changeset operations differ from causal order")
    occurrences = [item["occurrence"] for item in changeset["operations"]]
    if len(occurrences) != len(set(occurrences)):
        reject_operation("duplicate contained operation")
    if not isinstance(changeset["changes"], list) or not changeset["changes"]:
        reject_operation("non-genesis changeset is empty")
    for change in changeset["changes"]:
        change = require_exact(
            change,
            {"key_commitment", "before", "after", "operation_ref"},
            "changeset change",
        )
        if not isinstance(change["key_commitment"], str) or not DIGEST_RE.fullmatch(
            change["key_commitment"]
        ):
            reject_operation("malformed key commitment")
        validate_state(change["before"])
        validate_state(change["after"])
        if change["before"] == change["after"]:
            reject_operation("changeset transition has no effect")
        ref = validate_operation_ref(change["operation_ref"])
        if ref not in changeset["operations"]:
            reject_operation("change cites an uncontained operation")
    order = [
        (item["key_commitment"], item["operation_ref"]["occurrence"])
        for item in changeset["changes"]
    ]
    if order != sorted(order):
        reject_operation("changes are not canonically ordered")
    keys = [item["key_commitment"] for item in changeset["changes"]]
    if len(keys) != len(set(keys)):
        reject_operation("duplicate changeset key")
    expected = derive_changes(
        context["store_before"],
        context["store_after"],
        context["contained_operations"],
        context["change_causes"],
    )
    if changeset["changes"] != expected:
        reject_operation("changeset does not explain exact Store consequences")


def authority_certificate(
    subject: str,
    mandate_id: str,
    catalog_pin: dict[str, str],
) -> dict[str, Any]:
    document = {
        "aithos-mandate-core": "1.0.0-draft.3",
        "subject": subject,
        "id": mandate_id,
        "issued_by": subject + "#root",
        "grantee": {
            "id": "urn:aithos:agent:k1-c-grantee",
            "label": "k1-c-grantee",
            "pubkey": multibase_ed(GRANTEE_KEY),
            "kex_pubkey": multibase_x(
                ed25519_to_x25519_public(public_bytes(GRANTEE_KEY))
            ),
        },
        "perimeter": [
            "write.public#dir=00000000-0000-4000-8000-000000000001",
            "read.gamma",
            "act.x.mail.send",
        ],
        "constraints": {
            "catalog_pins": [catalog_pin],
            "max_consumptions": 8,
            "max_mutations": 3,
        },
        "not_before": "2026-07-18T10:00:00Z",
        "not_after": "2026-07-19T10:00:00Z",
        "issued_at": "2026-07-18T09:59:00Z",
        "parent": None,
        "nonce": "abababababababababababababababab",
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    sign_signature_block(document, ROOT_KEY)
    return document


def make_catalog(subject: str) -> tuple[dict[str, Any], dict[str, Any]]:
    catalog = {
        "aithos-connector-catalog-core": DRAFT1,
        "connector": "mail",
        "catalog_version": "2026.07-k1c",
        "actions": [
            {"name": "list", "class": "read"},
            {"name": "send", "class": "act"},
        ],
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(CATALOG_KEY),
            "value": "",
        },
    }
    sign_signature_block(catalog, CATALOG_KEY)
    catalog_digest = sha256_text(jcs(catalog).encode())
    approval = {
        "aithos-connector-catalog-approval-core": DRAFT1,
        "subject": subject,
        "connector": "mail",
        "catalog_version": "2026.07-k1c",
        "catalog_digest": catalog_digest,
        "approved_at": "2026-07-18T11:55:00Z",
        "signature": {"alg": "ed25519", "key": "#content", "value": ""},
    }
    sign_signature_block(approval, CONTENT_KEY)
    return catalog, approval


def make_gamma_entry(
    ordinal: int,
    kind: str,
    ref: dict[str, Any],
    prev: str,
    at: str,
    mandate_id: str,
) -> dict[str, Any]:
    entry = {
        "v": 2,
        "id": "gamma_" + fixture_ulid(ordinal),
        "prev": prev,
        "at": at,
        "kind": kind,
        "target": "x.mail" if kind == "action" else "public",
        "authorized_by": mandate_id,
        "authorized_via": [mandate_id],
        "payload": (
            {"action": "send", "args_hash": sha256_text(b"k1-c-args")}
            if kind == "action"
            else {"sid": "01J00000000000000000000061", "verb": "edit"}
        ),
        "operation_ref": ref,
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(GRANTEE_KEY),
            "value": "",
        },
    }
    sign_signature_block(entry, GRANTEE_KEY)
    return entry


def delegated_counts(mandate_id: str, counters: dict[str, int]) -> dict[str, Any]:
    payload = mandate_id.encode() + b"\x00" + jcs(counters).encode()
    leaf = blake3_hex(LEAF_DOMAIN + payload)
    return {
        "reference": {
            "aithos-delegated-counts-core": DELEGATED_COUNTS_PROFILE,
            "root": leaf,
        },
        "mandate_id": mandate_id,
        "counters": counters,
        "payload_hex": payload.hex(),
        "leaf_hex": leaf,
    }


def evidence_item(items: list[dict[str, Any]], kind: str) -> dict[str, Any]:
    return next(item for item in items if item["kind"] == kind)


def validate_catalog(
    catalog: Any,
    approval: Any,
    context: dict[str, Any],
) -> None:
    catalog = require_exact(
        catalog,
        {
            "aithos-connector-catalog-core",
            "connector",
            "catalog_version",
            "actions",
            "signature",
        },
        "catalog",
    )
    approval = require_exact(
        approval,
        {
            "aithos-connector-catalog-approval-core",
            "subject",
            "connector",
            "catalog_version",
            "catalog_digest",
            "approved_at",
            "signature",
        },
        "catalog approval",
    )
    if catalog["aithos-connector-catalog-core"] != DRAFT1:
        reject_operation("unknown catalog profile")
    if approval["aithos-connector-catalog-approval-core"] != DRAFT1:
        reject_operation("unknown catalog approval profile")
    verify_signature_block(catalog, context["catalog_key"])
    verify_signature_block(approval, context["content_key"])
    catalog_digest = sha256_text(jcs(catalog).encode())
    if approval["catalog_digest"] != catalog_digest:
        reject_operation("catalog approval digest mismatch")
    if approval["subject"] != context["subject"]:
        reject_operation("catalog approval subject mismatch")
    if (
        approval["connector"] != catalog["connector"]
        or approval["catalog_version"] != catalog["catalog_version"]
    ):
        reject_operation("catalog approval coordinates mismatch")
    if context["catalog_ref"] != {
        "catalog_version": catalog["catalog_version"],
        "catalog_digest": catalog_digest,
        "approval_digest": sha256_text(jcs(approval).encode()),
    }:
        reject_operation("catalog evidence is uncorrelated")


def validate_authorship(document: Any, context: dict[str, Any]) -> None:
    document = require_exact(
        document,
        {
            "aithos-authorship-core",
            "subject",
            "zone",
            "sid",
            "content_hash",
            "operation_ref",
            "edition",
            "authorized_via",
            "key",
            "sig",
        },
        "authorship",
    )
    if document["aithos-authorship-core"] != AUTHORSHIP_PROFILE:
        reject_operation("unknown authorship profile")
    if document["subject"] != context["subject"] or document["zone"] != "public":
        reject_operation("authorship subject or zone mismatch")
    if document["sid"] != context["public_sid"]:
        reject_operation("authorship SID mismatch")
    if document["content_hash"] != sha256_text(context["public_body"].encode()):
        reject_operation("authorship content hash mismatch")
    if document["operation_ref"] != context["contained_operations"][0]:
        reject_operation("authorship operation mismatch")
    if document["edition"] != {
        "height": context["height"],
        "predecessors": context["predecessors"],
    }:
        reject_operation("authorship edition mismatch")
    if document["authorized_via"] != [context["authority_ref"]]:
        reject_operation("authorship authority chain mismatch")
    if document["key"] != context["grantee_key"]:
        reject_operation("authorship key mismatch")
    verify_without_member(document, "sig", context["grantee_key"])


def validate_session(item: dict[str, Any], context: dict[str, Any]) -> None:
    certificate = require_exact(
        item["certificate"],
        {
            "aithos-session-core",
            "subject",
            "mandate_id",
            "key",
            "not_before",
            "not_after",
            "signature",
        },
        "session certificate",
    )
    proof = require_exact(
        item["proof"],
        {"aithos-session-proof-core", "operation_ref", "key", "sig"},
        "session proof",
    )
    if certificate["aithos-session-core"] != DRAFT1:
        reject_operation("unknown session-certificate profile")
    if proof["aithos-session-proof-core"] != DRAFT1:
        reject_operation("unknown session-proof profile")
    if certificate["subject"] != context["subject"]:
        reject_operation("session subject mismatch")
    if certificate["mandate_id"] != context["mandate_id"]:
        reject_operation("session mandate mismatch")
    if certificate["key"] != context["session_key"] or proof["key"] != context["session_key"]:
        reject_operation("session key mismatch")
    if sha256_text(jcs(certificate).encode()) != context["session_certificate_digest"]:
        reject_operation("session certificate digest mismatch")
    if proof["operation_ref"] != context["contained_operations"][1]:
        reject_operation("session operation mismatch")
    verify_signature_block(certificate, context["grantee_key"])
    verify_without_member(proof, "sig", context["session_key"])


def validate_receipt(document: Any, context: dict[str, Any]) -> None:
    document = require_exact(
        document,
        {
            "v",
            "family",
            "obligation",
            "operation_ref",
            "verdict",
            "at",
            "sig",
        },
        "receipt",
    )
    if document["v"] != 2 or document["family"] != "obligation":
        reject_operation("unknown receipt profile")
    if document["operation_ref"] != context["contained_operations"][2]:
        reject_operation("receipt operation mismatch")
    if document["verdict"] != "approve":
        reject_operation("receipt does not approve")
    verify_without_member(document, "sig", context["receipt_key"])


def validate_presentation(document: Any, context: dict[str, Any]) -> None:
    document = require_exact(
        document,
        {
            "aithos-gamma-presentation-core",
            "subject",
            "operation_ref",
            "source_head",
            "request_digest",
            "entries",
            "at",
            "key",
            "sig",
        },
        "Gamma presentation",
    )
    if document["aithos-gamma-presentation-core"] != PRESENTATION_PROFILE:
        reject_operation("unknown presentation profile")
    if document["subject"] != context["subject"]:
        reject_operation("presentation subject mismatch")
    if document["operation_ref"] != context["contained_operations"][3]:
        reject_operation("presentation operation mismatch")
    if document["source_head"] != context["source_head"]:
        reject_operation("presentation source head mismatch")
    if document["request_digest"] != context["request_digest"]:
        reject_operation("presentation request digest mismatch")
    if document["entries"] != context["query_result"]:
        reject_operation("presentation result differs from canonical query")
    ids = [item["id"] for item in document["entries"]]
    if len(ids) != len(set(ids)):
        reject_operation("presentation duplicates a Gamma id")
    if document["at"] != context["presentation_at"]:
        reject_operation("presentation time mismatch")
    if document["key"] != context["grantee_key"]:
        reject_operation("presentation key mismatch")
    verify_without_member(document, "sig", context["grantee_key"])


def validate_evidence(candidate: dict[str, Any], context: dict[str, Any]) -> None:
    evidence = require_exact(
        candidate,
        {"aithos-evidence-core", "items", "delegated_counts"},
        "evidence set",
    )
    if evidence["aithos-evidence-core"] != EVIDENCE_PROFILE:
        reject_operation("unknown evidence profile")
    if evidence["delegated_counts"] != context["delegated_counts"]:
        reject_operation("delegated-counts reference mismatch")
    if not isinstance(evidence["items"], list):
        reject_operation("evidence items is not an array")
    encoded_items = [jcs(item) for item in evidence["items"]]
    if encoded_items != sorted(encoded_items):
        reject_operation("evidence items are not sorted by complete JCS")
    if len(encoded_items) != len(set(encoded_items)):
        reject_operation("duplicate evidence item")
    if [item.get("kind") for item in evidence["items"]].count("authorship") != 1:
        reject_operation("authorship evidence cardinality mismatch")
    if [item.get("kind") for item in evidence["items"]].count("session") != 1:
        reject_operation("session evidence cardinality mismatch")
    if [item.get("kind") for item in evidence["items"]].count("receipt") != 1:
        reject_operation("receipt evidence cardinality mismatch")
    if [item.get("kind") for item in evidence["items"]].count("catalog") != 1:
        reject_operation("catalog evidence cardinality mismatch")
    if [item.get("kind") for item in evidence["items"]].count("presentation") != 1:
        reject_operation("presentation evidence cardinality mismatch")
    for item in evidence["items"]:
        kind = item.get("kind")
        if kind == "authorship":
            require_exact(item, {"kind", "document"}, "authorship evidence item")
            validate_authorship(item["document"], context)
        elif kind == "session":
            require_exact(
                item,
                {"kind", "certificate", "proof"},
                "session evidence item",
            )
            validate_session(item, context)
        elif kind == "receipt":
            require_exact(item, {"kind", "document"}, "receipt evidence item")
            validate_receipt(item["document"], context)
        elif kind == "catalog":
            require_exact(
                item,
                {"kind", "catalog", "approval"},
                "catalog evidence item",
            )
            validate_catalog(item["catalog"], item["approval"], context)
        elif kind == "presentation":
            require_exact(item, {"kind", "document"}, "presentation evidence item")
            validate_presentation(item["document"], context)
        else:
            reject_operation("unknown or unused evidence item")


MANIFEST_BASE_KEYS = {
    "aithos-core",
    "edition",
    "files",
    "gamma_head",
    "authorized_via",
    "signature",
}
MANIFEST_CARRIER_KEYS = {"operation_ref", "changeset_ref", "evidence_ref"}


def validate_manifest_signed_form(
    manifest: dict[str, Any],
    context: dict[str, Any],
) -> None:
    if not isinstance(manifest, dict):
        reject_did("manifest is not an object")
    version = manifest.get("aithos-core")
    expected = (
        MANIFEST_BASE_KEYS | MANIFEST_CARRIER_KEYS
        if version == DRAFT2
        else MANIFEST_BASE_KEYS
        if version == DRAFT1
        else set()
    )
    require_exact(manifest, expected, "manifest", reject_did)
    if version not in {DRAFT1, DRAFT2}:
        reject_did("unknown manifest profile")
    if version == DRAFT1:
        reject_did("fixture candidate unexpectedly downgraded to draft1")
    require_exact(
        manifest["edition"],
        {"height", "prev_hash", "created_at"},
        "manifest edition",
        reject_did,
    )
    if not isinstance(manifest["files"], dict):
        reject_did("manifest files is not an object")
    if not isinstance(manifest["authorized_via"], list):
        reject_did("manifest authorized_via is not an array")
    validate_operation_ref(manifest["operation_ref"])
    for member, profile in [
        ("changeset_ref", CHANGESET_PROFILE),
        ("evidence_ref", EVIDENCE_PROFILE),
    ]:
        reference = require_exact(
            manifest[member],
            {
                "aithos-changeset-core" if member == "changeset_ref" else "aithos-evidence-core",
                "digest",
            },
            member,
            reject_did,
        )
        profile_member = next(name for name in reference if name != "digest")
        if reference[profile_member] != profile:
            reject_did(f"unknown {member} profile")
        if not isinstance(reference["digest"], str) or not DIGEST_RE.fullmatch(
            reference["digest"]
        ):
            reject_did(f"malformed {member} digest")
    verify_signature_block(
        manifest,
        context["grantee_key"],
        did_boundary=True,
    )


def validate_bundle(candidate: dict[str, Any], context: dict[str, Any]) -> None:
    manifest = candidate["manifest"]
    validate_manifest_signed_form(manifest, context)
    validate_changeset(candidate["changeset"], context)
    validate_evidence(candidate["evidence"], context)

    changeset_jcs = jcs(candidate["changeset"])
    evidence_jcs = jcs(candidate["evidence"])
    changeset_ref = {
        "aithos-changeset-core": CHANGESET_PROFILE,
        "digest": commitment(CHANGESET_DOMAIN, changeset_jcs.encode()),
    }
    evidence_ref = {
        "aithos-evidence-core": EVIDENCE_PROFILE,
        "digest": commitment(EVIDENCE_DOMAIN, evidence_jcs.encode()),
    }
    if manifest["changeset_ref"] != changeset_ref:
        reject_operation("manifest changeset reference mismatch")
    if manifest["evidence_ref"] != evidence_ref:
        reject_operation("manifest evidence reference mismatch")
    if manifest["operation_ref"] != context["publication_ref"]:
        reject_operation("manifest publication operation mismatch")
    if manifest["authorized_via"] != [context["mandate_id"]]:
        reject_operation("manifest authority chain mismatch")
    if manifest["edition"] != {
        "height": context["height"],
        "prev_hash": context["predecessors"][0][len("sha256:") :],
        "created_at": context["publication_at"],
    }:
        reject_operation("manifest edition facts mismatch")
    if manifest["gamma_head"] != context["source_head"]:
        reject_operation("manifest Gamma head mismatch")

    change_path = "changesets/" + changeset_ref["digest"][len("sha256:") :] + ".json"
    evidence_path = "evidence/" + evidence_ref["digest"][len("sha256:") :] + ".json"
    expected_sidecars = {
        change_path: changeset_jcs,
        evidence_path: evidence_jcs,
    }
    if candidate["sidecars"] != expected_sidecars:
        reject_operation("canonical carrier sidecar path or bytes mismatch")
    expected_files = {
        key: sha256_hex(value.encode())
        for key, value in context["store_after"].items()
    }
    expected_files.update(
        {
            path: sha256_hex(value.encode())
            for path, value in expected_sidecars.items()
        }
    )
    if manifest["files"] != dict(sorted(expected_files.items())):
        reject_operation("manifest file pins do not match exact candidate bytes")


def build_vector() -> dict[str, Any]:
    subject = "did:aithos:" + multibase_ed(ROOT_KEY)
    mandate_id = "mandate_" + fixture_ulid(41)
    predecessor = sha256_text(b"k1-c-parent-manifest")
    height = 2
    publication_at = "2026-07-18T12:00:00Z"

    catalog, approval = make_catalog(subject)
    catalog_ref = {
        "catalog_version": catalog["catalog_version"],
        "catalog_digest": sha256_text(jcs(catalog).encode()),
        "approval_digest": sha256_text(jcs(approval).encode()),
    }
    certificate = authority_certificate(
        subject,
        mandate_id,
        {"connector": "mail", **catalog_ref},
    )
    certificate_jcs = jcs(certificate)
    authority_ref = {
        "id": mandate_id,
        "certificate_digest": sha256_text(certificate_jcs.encode()),
    }
    session_certificate = {
        "aithos-session-core": DRAFT1,
        "subject": subject,
        "mandate_id": mandate_id,
        "key": multibase_ed(SESSION_KEY),
        "not_before": "2026-07-18T11:30:00Z",
        "not_after": "2026-07-18T12:30:00Z",
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(GRANTEE_KEY),
            "value": "",
        },
    }
    sign_signature_block(session_certificate, GRANTEE_KEY)
    session_certificate_digest = sha256_text(jcs(session_certificate).encode())

    public_sid = "01J00000000000000000000061"
    public_body = "# K1-C\n\nCarrier-qualified public body.\n"
    circle_sid = "01J00000000000000000000062"
    circle_blob = jcs(
        {
            "sid": circle_sid,
            "ciphertext": "k1-c-circle-fixture",
        }
    )
    public_store_key = f"public/sections/{public_sid}.md"
    circle_store_key = f"circle/blobs/{circle_sid}.json"
    public_state_fact, public_state_ref = state_fact(
        {public_store_key: public_body}
    )
    circle_state_fact, circle_state_ref = state_fact(
        {circle_store_key: circle_blob}
    )

    facts_documents: list[dict[str, Any] | None] = [
        {
            "aithos-operation-facts-core": FACTS_PROFILE,
            "kind": "mutation",
            "facts": {
                "domain": "ethos",
                "zone": "public",
                "dir": ["01J00000000000000000000001"],
                "sid": public_sid,
                "verb": "create",
                "before": {"state": "absent"},
                "after": {"state": "present", "state_ref": public_state_ref},
            },
        },
        {
            "aithos-operation-facts-core": FACTS_PROFILE,
            "kind": "mutation",
            "facts": {
                "domain": "ethos",
                "zone": "circle",
                "dir": ["01J00000000000000000000002"],
                "sid": circle_sid,
                "verb": "create",
                "before": {"state": "absent"},
                "after": {"state": "present", "state_ref": circle_state_ref},
            },
        },
        {
            "aithos-operation-facts-core": FACTS_PROFILE,
            "kind": "action",
            "facts": {
                "connector": "mail",
                "action": "send",
                "args_hash": sha256_text(b"k1-c-args"),
                "catalog_ref": catalog_ref,
                "budget": {"state": "not-applicable"},
                "purpose": {"state": "not-applicable"},
            },
        },
        None,
        {
            "aithos-operation-facts-core": FACTS_PROFILE,
            "kind": "action",
            "facts": {
                "connector": "mail",
                "action": "send",
                "args_hash": sha256_text(b"k1-c-args"),
                "catalog_ref": catalog_ref,
                "budget": {"state": "not-applicable"},
                "purpose": {"state": "not-applicable"},
            },
        },
    ]
    kinds = ["mutation", "mutation", "action", "read", "action"]
    ats = [
        "2026-07-18T11:40:00Z",
        "2026-07-18T11:42:00Z",
        "2026-07-18T11:45:00Z",
        "2026-07-18T11:50:00Z",
        "2026-07-18T11:52:00Z",
    ]
    facts_refs: list[dict[str, Any] | None] = [None] * 5
    projections: list[dict[str, Any] | None] = [None] * 5
    for index in (0, 1, 2, 4):
        document = facts_documents[index]
        if document is None:
            raise AssertionError("missing fixture operation facts")
        facts_refs[index] = {
            "aithos-operation-facts-core": FACTS_PROFILE,
            "digest": commitment(FACTS_DOMAIN, jcs(document).encode()),
        }
        projection = operation_projection(
            61 + index,
            kinds[index],
            facts_refs[index],
            at=ats[index],
            subject=subject,
            authority_ref=authority_ref,
            history_head=predecessor,
        )
        if index == 1:
            projection["authority"]["session"] = {
                "key": multibase_ed(SESSION_KEY),
                "certificate_digest": session_certificate_digest,
            }
        projections[index] = projection

    first_projection = projections[0]
    action_projection = projections[2]
    if first_projection is None or action_projection is None:
        raise AssertionError("missing Gamma operation projection")
    gamma_first = make_gamma_entry(
        71,
        "section.add",
        operation_ref(first_projection),
        predecessor,
        ats[0],
        mandate_id,
    )
    first_head = sha256_text(jcs(gamma_first).encode())
    gamma_second = make_gamma_entry(
        72,
        "action",
        operation_ref(action_projection),
        first_head,
        ats[2],
        mandate_id,
    )
    second_head = sha256_text(jcs(gamma_second).encode())
    followup_projection = projections[4]
    if followup_projection is None:
        raise AssertionError("missing follow-up action projection")
    gamma_third = make_gamma_entry(
        73,
        "action",
        operation_ref(followup_projection),
        second_head,
        ats[4],
        mandate_id,
    )
    source_head = sha256_text(jcs(gamma_third).encode())
    gamma_jsonl = (
        jcs(gamma_first)
        + "\n"
        + jcs(gamma_second)
        + "\n"
        + jcs(gamma_third)
        + "\n"
    )

    request_digest = commitment(GAMMA_REQUEST_DOMAIN, b"read.gamma")
    facts_documents[3] = {
        "aithos-operation-facts-core": FACTS_PROFILE,
        "kind": "read",
        "facts": {
            "domain": "gamma",
            "source_head": source_head,
            "request_digest": request_digest,
        },
    }
    facts_refs[3] = {
        "aithos-operation-facts-core": FACTS_PROFILE,
        "digest": commitment(FACTS_DOMAIN, jcs(facts_documents[3]).encode()),
    }
    projections[3] = operation_projection(
        64,
        "read",
        facts_refs[3],
        at=ats[3],
        subject=subject,
        authority_ref=authority_ref,
        history_head=predecessor,
    )
    if any(item is None for item in projections):
        raise AssertionError("incomplete fixture operation projections")
    complete_projections = [item for item in projections if item is not None]
    complete_facts = [item for item in facts_documents if item is not None]
    complete_facts_refs = [item for item in facts_refs if item is not None]
    contained = [operation_ref(item) for item in complete_projections]

    catalog_pins_jcs = jcs({"mail": catalog_ref})
    store_before: dict[str, str] = {
        f"certs/{mandate_id}.json": certificate_jcs,
        "vault/catalog-pins.json": catalog_pins_jcs,
    }
    store_after = {
        f"certs/{mandate_id}.json": certificate_jcs,
        public_store_key: public_body,
        circle_store_key: circle_blob,
        "gamma/2026-07.jsonl": gamma_jsonl,
        "indices/public.json": jcs(
            {
                "sections": [
                    {
                        "sid": public_sid,
                        "path": "demo/k1-c",
                        "body_hash": sha256_text(public_body.encode()),
                    }
                ]
            }
        ),
        "roots/public.json": jcs(
            {
                "root": sha256_hex(
                    (public_sid + "\x00" + sha256_text(public_body.encode())).encode()
                )
            }
        ),
        "vault/catalog-pins.json": catalog_pins_jcs,
    }
    change_causes = {
        public_store_key: contained[0],
        circle_store_key: contained[1],
        "gamma/2026-07.jsonl": contained[4],
        "indices/public.json": contained[0],
        "roots/public.json": contained[0],
    }

    changeset = {
        "aithos-changeset-core": CHANGESET_PROFILE,
        "height": height,
        "predecessors": [predecessor],
        "operations": contained,
        "changes": derive_changes(
            store_before,
            store_after,
            contained,
            change_causes,
        ),
    }
    changeset_jcs = jcs(changeset)
    changeset_ref = {
        "aithos-changeset-core": CHANGESET_PROFILE,
        "digest": commitment(CHANGESET_DOMAIN, changeset_jcs.encode()),
    }

    authorship = {
        "aithos-authorship-core": AUTHORSHIP_PROFILE,
        "subject": subject,
        "zone": "public",
        "sid": public_sid,
        "content_hash": sha256_text(public_body.encode()),
        "operation_ref": contained[0],
        "edition": {"height": height, "predecessors": [predecessor]},
        "authorized_via": [authority_ref],
        "key": multibase_ed(GRANTEE_KEY),
        "sig": "",
    }
    sign_without_member(authorship, "sig", GRANTEE_KEY)

    session_proof = {
        "aithos-session-proof-core": DRAFT1,
        "operation_ref": contained[1],
        "key": multibase_ed(SESSION_KEY),
        "sig": "",
    }
    sign_without_member(session_proof, "sig", SESSION_KEY)

    receipt = {
        "v": 2,
        "family": "obligation",
        "obligation": "send-approval",
        "operation_ref": contained[2],
        "verdict": "approve",
        "at": "2026-07-18T11:44:00Z",
        "sig": "",
    }
    sign_without_member(receipt, "sig", RECEIPT_KEY)

    presentation = {
        "aithos-gamma-presentation-core": PRESENTATION_PROFILE,
        "subject": subject,
        "operation_ref": contained[3],
        "source_head": source_head,
        "request_digest": request_digest,
        "entries": [gamma_first, gamma_second, gamma_third],
        "at": ats[3],
        "key": multibase_ed(GRANTEE_KEY),
        "sig": "",
    }
    sign_without_member(presentation, "sig", GRANTEE_KEY)

    items = [
        {"kind": "authorship", "document": authorship},
        {
            "kind": "session",
            "certificate": session_certificate,
            "proof": session_proof,
        },
        {"kind": "receipt", "document": receipt},
        {"kind": "catalog", "catalog": catalog, "approval": approval},
        {"kind": "presentation", "document": presentation},
    ]
    items.sort(key=jcs)
    counts_fixture = delegated_counts(
        mandate_id,
        {"consumptions": 6, "mutations": 2},
    )
    evidence = {
        "aithos-evidence-core": EVIDENCE_PROFILE,
        "items": items,
        "delegated_counts": counts_fixture["reference"],
    }
    evidence_jcs = jcs(evidence)
    evidence_ref = {
        "aithos-evidence-core": EVIDENCE_PROFILE,
        "digest": commitment(EVIDENCE_DOMAIN, evidence_jcs.encode()),
    }

    publication_facts = {
        "aithos-operation-facts-core": FACTS_PROFILE,
        "kind": "publication",
        "facts": {
            "mode": "normal",
            "height": height,
            "predecessors": [predecessor],
            "changeset_ref": changeset_ref,
            "contained_operations": contained,
        },
    }
    publication_facts_ref = {
        "aithos-operation-facts-core": FACTS_PROFILE,
        "digest": commitment(FACTS_DOMAIN, jcs(publication_facts).encode()),
    }
    publication_projection = operation_projection(
        69,
        "publication",
        publication_facts_ref,
        at=publication_at,
        subject=subject,
        authority_ref=authority_ref,
        history_head=predecessor,
    )
    publication_ref = operation_ref(publication_projection)

    change_path = (
        "changesets/" + changeset_ref["digest"][len("sha256:") :] + ".json"
    )
    evidence_path = (
        "evidence/" + evidence_ref["digest"][len("sha256:") :] + ".json"
    )
    sidecars = {change_path: changeset_jcs, evidence_path: evidence_jcs}
    files = {
        key: sha256_hex(value.encode())
        for key, value in store_after.items()
    }
    files.update(
        {
            change_path: sha256_hex(changeset_jcs.encode()),
            evidence_path: sha256_hex(evidence_jcs.encode()),
        }
    )
    manifest = {
        "aithos-core": DRAFT2,
        "edition": {
            "height": height,
            "prev_hash": predecessor[len("sha256:") :],
            "created_at": publication_at,
        },
        "files": dict(sorted(files.items())),
        "gamma_head": source_head,
        "authorized_via": [mandate_id],
        "operation_ref": publication_ref,
        "changeset_ref": changeset_ref,
        "evidence_ref": evidence_ref,
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(GRANTEE_KEY),
            "value": "",
        },
    }
    sign_signature_block(manifest, GRANTEE_KEY)

    context = {
        "subject": subject,
        "mandate_id": mandate_id,
        "authority_ref": authority_ref,
        "grantee_key": multibase_ed(GRANTEE_KEY),
        "session_key": multibase_ed(SESSION_KEY),
        "session_certificate_digest": session_certificate_digest,
        "catalog_key": multibase_ed(CATALOG_KEY),
        "content_key": multibase_ed(CONTENT_KEY),
        "receipt_key": multibase_ed(RECEIPT_KEY),
        "height": height,
        "predecessors": [predecessor],
        "publication_at": publication_at,
        "contained_operations": contained,
        "publication_ref": publication_ref,
        "store_before": store_before,
        "store_after": store_after,
        "change_causes": change_causes,
        "public_sid": public_sid,
        "public_body": public_body,
        "catalog_ref": catalog_ref,
        "delegated_counts": counts_fixture["reference"],
        "source_head": source_head,
        "request_digest": request_digest,
        "query_result": [gamma_first, gamma_second, gamma_third],
        "presentation_at": ats[3],
    }
    candidate = {
        "changeset": changeset,
        "evidence": evidence,
        "sidecars": sidecars,
        "manifest": manifest,
    }
    validate_bundle(candidate, context)

    negative_cases: list[dict[str, Any]] = []

    def add(
        case_id: str,
        defect: str,
        expected: str,
        mutate: Callable[[dict[str, Any]], None],
    ) -> None:
        broken = clone(candidate)
        mutate(broken)
        try:
            validate_bundle(broken, context)
        except ProtocolError as error:
            if error.code != expected:
                raise AssertionError(
                    f"{case_id}: expected {expected}, got {error.code}: {error}"
                ) from error
            negative_cases.append(
                {
                    "id": case_id,
                    "defect": defect,
                    "candidate": broken,
                    "must_fail": expected,
                }
            )
            return
        raise AssertionError(f"{case_id}: invalid candidate was accepted")

    def resign_manifest(value: dict[str, Any]) -> None:
        sign_signature_block(value["manifest"], GRANTEE_KEY)

    def authorship_doc(value: dict[str, Any]) -> dict[str, Any]:
        return evidence_item(value["evidence"]["items"], "authorship")["document"]

    def session_item(value: dict[str, Any]) -> dict[str, Any]:
        return evidence_item(value["evidence"]["items"], "session")

    def receipt_doc(value: dict[str, Any]) -> dict[str, Any]:
        return evidence_item(value["evidence"]["items"], "receipt")["document"]

    def catalog_item(value: dict[str, Any]) -> dict[str, Any]:
        return evidence_item(value["evidence"]["items"], "catalog")

    def presentation_doc(value: dict[str, Any]) -> dict[str, Any]:
        return evidence_item(value["evidence"]["items"], "presentation")["document"]

    add(
        "changeset-extra-member",
        "changeset top-level extra member",
        INVALID_OPERATION,
        lambda c: c["changeset"].__setitem__("extra", True),
    )
    add(
        "changeset-unknown-profile",
        "changeset profile changed",
        INVALID_OPERATION,
        lambda c: c["changeset"].__setitem__("aithos-changeset-core", DRAFT2),
    )
    add(
        "changeset-height-mismatch",
        "publication height differs",
        INVALID_OPERATION,
        lambda c: c["changeset"].__setitem__("height", 3),
    )
    add(
        "changeset-reversed-operations",
        "contained operations are caller-reordered",
        INVALID_OPERATION,
        lambda c: c["changeset"].__setitem__(
            "operations",
            list(reversed(c["changeset"]["operations"])),
        ),
    )
    add(
        "changeset-duplicate-operation",
        "one contained occurrence is duplicated",
        INVALID_OPERATION,
        lambda c: c["changeset"]["operations"].append(
            clone(c["changeset"]["operations"][0])
        ),
    )
    add(
        "changeset-reversed-changes",
        "changes are not sorted by commitments",
        INVALID_OPERATION,
        lambda c: c["changeset"].__setitem__(
            "changes",
            list(reversed(c["changeset"]["changes"])),
        ),
    )
    add(
        "changeset-duplicate-key",
        "two changes name one key commitment",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"][1].__setitem__(
            "key_commitment",
            c["changeset"]["changes"][0]["key_commitment"],
        ),
    )
    add(
        "changeset-no-effect",
        "before and after are equal",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"][0].__setitem__(
            "after",
            clone(c["changeset"]["changes"][0]["before"]),
        ),
    )
    add(
        "changeset-uncontained-operation",
        "a change cites an outsider occurrence",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"][0].__setitem__(
            "operation_ref",
            {
                "aithos-operation-core": DRAFT1,
                "occurrence": "op_" + fixture_ulid(99),
                "commitment": sha256_text(b"outsider"),
            },
        ),
    )
    add(
        "changeset-omitted-consequence",
        "one Store consequence is omitted",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"].pop(),
    )
    add(
        "changeset-invented-consequence",
        "one invented Store consequence is added",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"].append(
            {
                "key_commitment": commitment(
                    STATE_KEY_DOMAIN,
                    b"invented/object.json",
                ),
                "before": {"state": "absent"},
                "after": {
                    "state": "present",
                    "byte_commitment": commitment(
                        STATE_BYTES_DOMAIN,
                        b"invented",
                    ),
                },
                "operation_ref": clone(contained[0]),
            }
        ),
    )
    add(
        "changeset-carrier-cycle",
        "changeset invents its own sidecar consequence",
        INVALID_OPERATION,
        lambda c: c["changeset"]["changes"].append(
            {
                "key_commitment": commitment(
                    STATE_KEY_DOMAIN,
                    next(iter(c["sidecars"])).encode(),
                ),
                "before": {"state": "absent"},
                "after": {
                    "state": "present",
                    "byte_commitment": commitment(
                        STATE_BYTES_DOMAIN,
                        next(iter(c["sidecars"].values())).encode(),
                    ),
                },
                "operation_ref": clone(contained[0]),
            }
        ),
    )

    add(
        "evidence-extra-member",
        "evidence set top-level extra member",
        INVALID_OPERATION,
        lambda c: c["evidence"].__setitem__("authority", True),
    )
    add(
        "evidence-unknown-profile",
        "evidence profile changed",
        INVALID_OPERATION,
        lambda c: c["evidence"].__setitem__("aithos-evidence-core", DRAFT2),
    )
    add(
        "evidence-unsorted-items",
        "evidence items are reversed",
        INVALID_OPERATION,
        lambda c: c["evidence"].__setitem__(
            "items",
            list(reversed(c["evidence"]["items"])),
        ),
    )
    add(
        "evidence-duplicate-item",
        "one complete evidence item repeats",
        INVALID_OPERATION,
        lambda c: c["evidence"]["items"].append(
            clone(c["evidence"]["items"][0])
        ),
    )
    add(
        "evidence-unknown-item",
        "an unrelated item is introduced",
        INVALID_OPERATION,
        lambda c: c["evidence"]["items"].append(
            {"kind": "authority", "document": {}}
        ),
    )

    def bad_authorship_signature(c: dict[str, Any]) -> None:
        document = authorship_doc(c)
        sign_without_member(document, "sig", STRANGER_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "authorship-stranger-signature",
        "authorship is signed by a different actor",
        INVALID_OPERATION,
        bad_authorship_signature,
    )

    def bad_authorship_content(c: dict[str, Any]) -> None:
        document = authorship_doc(c)
        document["content_hash"] = sha256_text(b"different public body")
        sign_without_member(document, "sig", GRANTEE_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "authorship-content-mismatch",
        "authorship content hash differs from stored bytes",
        INVALID_OPERATION,
        bad_authorship_content,
    )

    def bad_authorship_operation(c: dict[str, Any]) -> None:
        document = authorship_doc(c)
        document["operation_ref"] = clone(contained[4])
        sign_without_member(document, "sig", GRANTEE_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "authorship-operation-mismatch",
        "authorship names another contained occurrence",
        INVALID_OPERATION,
        bad_authorship_operation,
    )

    def bad_session_operation(c: dict[str, Any]) -> None:
        proof = session_item(c)["proof"]
        proof["operation_ref"] = clone(contained[4])
        sign_without_member(proof, "sig", SESSION_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "session-operation-mismatch",
        "session proof names another occurrence",
        INVALID_OPERATION,
        bad_session_operation,
    )

    def bad_receipt_operation(c: dict[str, Any]) -> None:
        document = receipt_doc(c)
        document["operation_ref"] = clone(contained[4])
        sign_without_member(document, "sig", RECEIPT_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "receipt-operation-mismatch",
        "receipt names another occurrence",
        INVALID_OPERATION,
        bad_receipt_operation,
    )

    def bad_catalog_approval(c: dict[str, Any]) -> None:
        approval_doc = catalog_item(c)["approval"]
        approval_doc["catalog_digest"] = sha256_text(b"different catalog")
        sign_signature_block(approval_doc, CONTENT_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "catalog-approval-mismatch",
        "approval addresses another catalog",
        INVALID_OPERATION,
        bad_catalog_approval,
    )

    def bad_presentation_result(c: dict[str, Any]) -> None:
        document = presentation_doc(c)
        document["entries"] = document["entries"][:1]
        sign_without_member(document, "sig", GRANTEE_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "presentation-withheld-entry",
        "presentation result withholds one selected entry",
        INVALID_OPERATION,
        bad_presentation_result,
    )

    def bad_presentation_head(c: dict[str, Any]) -> None:
        document = presentation_doc(c)
        document["source_head"] = predecessor
        sign_without_member(document, "sig", GRANTEE_KEY)
        c["evidence"]["items"].sort(key=jcs)

    add(
        "presentation-source-head-mismatch",
        "presentation names another Gamma head",
        INVALID_OPERATION,
        bad_presentation_head,
    )
    add(
        "evidence-delegated-counts-mismatch",
        "delegated-counts root differs from replay",
        INVALID_OPERATION,
        lambda c: c["evidence"]["delegated_counts"].__setitem__(
            "root",
            "00" * 32,
        ),
    )

    def private_material(c: dict[str, Any]) -> None:
        document = receipt_doc(c)
        document["private_key"] = "forbidden"
        c["evidence"]["items"].sort(key=jcs)

    add(
        "evidence-private-material",
        "an evidence item carries private material",
        INVALID_OPERATION,
        private_material,
    )

    def draft1_with_carriers(c: dict[str, Any]) -> None:
        c["manifest"]["aithos-core"] = DRAFT1
        resign_manifest(c)

    add(
        "manifest-draft1-carries-carriers",
        "draft1 manifest carries K1-C members",
        INVALID_DID,
        draft1_with_carriers,
    )
    add(
        "manifest-missing-evidence-ref",
        "draft2 manifest omits evidence_ref",
        INVALID_DID,
        lambda c: c["manifest"].pop("evidence_ref"),
    )
    add(
        "manifest-null-changeset-ref",
        "draft2 manifest carries a null changeset_ref",
        INVALID_DID,
        lambda c: c["manifest"].__setitem__("changeset_ref", None),
    )
    add(
        "manifest-extra-member",
        "signed manifest form has an unknown member",
        INVALID_DID,
        lambda c: c["manifest"].__setitem__("carrier", {}),
    )
    add(
        "manifest-bad-signature",
        "manifest signature is made by a stranger",
        INVALID_DID,
        lambda c: sign_signature_block(c["manifest"], STRANGER_KEY),
    )

    def bad_changeset_ref(c: dict[str, Any]) -> None:
        c["manifest"]["changeset_ref"]["digest"] = sha256_text(b"other changeset")
        resign_manifest(c)

    add(
        "manifest-changeset-ref-mismatch",
        "signed changeset reference differs from sidecar",
        INVALID_OPERATION,
        bad_changeset_ref,
    )

    def bad_evidence_ref(c: dict[str, Any]) -> None:
        c["manifest"]["evidence_ref"]["digest"] = sha256_text(b"other evidence")
        resign_manifest(c)

    add(
        "manifest-evidence-ref-mismatch",
        "signed evidence reference differs from sidecar",
        INVALID_OPERATION,
        bad_evidence_ref,
    )

    def bad_sidecar_path(c: dict[str, Any]) -> None:
        path, payload = next(iter(c["sidecars"].items()))
        c["sidecars"].pop(path)
        c["sidecars"]["evidence/not-the-digest.json"] = payload

    add(
        "manifest-sidecar-path-mismatch",
        "carrier bytes live under a non-canonical path",
        INVALID_OPERATION,
        bad_sidecar_path,
    )

    def bad_file_pin(c: dict[str, Any]) -> None:
        path = next(iter(c["sidecars"]))
        c["manifest"]["files"][path] = "00" * 32
        resign_manifest(c)

    add(
        "manifest-sidecar-file-pin-mismatch",
        "sidecar files pin differs from exact JCS bytes",
        INVALID_OPERATION,
        bad_file_pin,
    )

    def bad_publication_ref(c: dict[str, Any]) -> None:
        c["manifest"]["operation_ref"] = clone(contained[0])
        resign_manifest(c)

    add(
        "manifest-publication-operation-mismatch",
        "manifest operation_ref names a contained mutation",
        INVALID_OPERATION,
        bad_publication_ref,
    )

    historical = {
        name: sha256_hex((HERE / name).read_bytes())
        for name in HISTORICAL_FILES
    }
    vector = {
        "vector": "CB2-K1-C-DRAFT2-CARRIERS-1",
        "description": (
            "Independent Python cryptography/blake3 K1-C oracle: one acyclic "
            "draft2 changeset/evidence/manifest assembly, exact reference "
            "domains and sidecar paths, five closed evidence variants, D7 "
            "root, signatures, file pins, and mono-defect error boundary."
        ),
        "profiles": {
            "manifest_v1": DRAFT1,
            "manifest_v2": DRAFT2,
            "operation": OPERATION_PROFILE,
            "changeset": CHANGESET_PROFILE,
            "evidence": EVIDENCE_PROFILE,
            "authorship": AUTHORSHIP_PROFILE,
            "presentation": PRESENTATION_PROFILE,
            "delegated_counts": DELEGATED_COUNTS_PROFILE,
        },
        "domains": {
            "operation": OPERATION_DOMAIN,
            "operation_facts": FACTS_DOMAIN,
            "changeset": CHANGESET_DOMAIN,
            "evidence": EVIDENCE_DOMAIN,
            "state_key": STATE_KEY_DOMAIN,
            "state_bytes": STATE_BYTES_DOMAIN,
            "state_fact": STATE_FACT_DOMAIN,
            "gamma_request": GAMMA_REQUEST_DOMAIN,
        },
        "deterministic_private_seed_hex": {
            "root": "11" * 32,
            "content": "22" * 32,
            "grantee": "33" * 32,
            "session": "44" * 32,
            "catalog": "55" * 32,
            "receipt": "66" * 32,
            "stranger": "77" * 32,
        },
        "historical_vector_sha256": historical,
        "context": context,
        "positive": {
            "authority_certificate": {
                "document": certificate,
                "document_jcs": certificate_jcs,
                "digest": authority_ref["certificate_digest"],
            },
            "state_facts": [public_state_fact, circle_state_fact],
            "facts_documents": complete_facts,
            "facts_refs": complete_facts_refs,
            "operation_projections": complete_projections,
            "operation_projection_jcs": [
                jcs(item) for item in complete_projections
            ],
            "contained_operations": contained,
            "changeset": {
                "document": changeset,
                "document_jcs": changeset_jcs,
                "reference": changeset_ref,
                "path": change_path,
                "file_sha256": sha256_hex(changeset_jcs.encode()),
            },
            "evidence": {
                "document": evidence,
                "document_jcs": evidence_jcs,
                "reference": evidence_ref,
                "path": evidence_path,
                "file_sha256": sha256_hex(evidence_jcs.encode()),
            },
            "delegated_counts_fixture": counts_fixture,
            "gamma_query": {
                "canonical": "read.gamma",
                "request_digest": request_digest,
                "source_head": source_head,
                "result": [gamma_first, gamma_second, gamma_third],
            },
            "publication": {
                "facts": publication_facts,
                "facts_ref": publication_facts_ref,
                "projection": publication_projection,
                "projection_jcs": jcs(publication_projection),
                "operation_ref": publication_ref,
            },
            "candidate": candidate,
            "manifest_jcs": jcs(manifest),
            "manifest_preimage_jcs": jcs(
                {
                    **manifest,
                    "signature": {**manifest["signature"], "value": ""},
                }
            ),
        },
        "negative_cases": negative_cases,
        "inventory": {
            "negative_ids": [case["id"] for case in negative_cases],
            "operation_error_variant": INVALID_OPERATION,
            "manifest_error_variant": INVALID_DID,
            "operation_negative_count": sum(
                case["must_fail"] == INVALID_OPERATION for case in negative_cases
            ),
            "manifest_negative_count": sum(
                case["must_fail"] == INVALID_DID for case in negative_cases
            ),
            "changeset_excludes_candidate_and_carrier_sidecars": True,
            "evidence_grants_no_authority": True,
            "draft1_bytes_are_not_reinterpreted": True,
            "target_to_store_key_derivation_is_not_public_wire": True,
        },
    }
    return vector


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

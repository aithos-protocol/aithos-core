#!/usr/bin/env python3
"""Independent CB2 oracle for Gamma v2 and semantic replay.

The oracle uses Python cryptography, hashlib and blake3. It never imports or
executes the Rust implementation. It fixes signed Gamma-v2 bytes, monotone
manifest/Gamma transitions, occurrence replay/equivocation, raw H2 line
accounting, and the closed semantic-replay decision inventory.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime
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
DEFAULT_OUTPUT = HERE / "cb2-gamma-v2-replay.json"

MANIFEST_V1 = "1.0.0-draft.1"
MANIFEST_V2 = "1.0.0-draft.2"
OPERATION_PROFILE = "1.0.0-draft.1"
FACTS_PROFILE = "1.0.0-draft.1"
OPERATION_DOMAIN = "aithos-core/v1/operation-commitment"

INVALID_GAMMA = "InvalidGammaEntry"
INVALID_OPERATION = "InvalidOperation"
INVALID_MANDATE = "InvalidMandate"

KINDS = (
    "section.add",
    "section.modify",
    "section.delete",
    "section.redact",
    "ethos.read",
    "action",
    "inference",
    "grant",
    "revoke",
    "rotate",
    "merge",
    "heartbeat",
)
OPERATION_KINDS = set(KINDS) - {"heartbeat"}
KIND_TO_OPERATION = {
    "section.add": "mutation",
    "section.modify": "mutation",
    "section.delete": "mutation",
    "section.redact": "mutation",
    "ethos.read": "read",
    "action": "action",
    "inference": "inference",
    "grant": "grant",
    "revoke": "revoke",
    "rotate": "rotate",
    "merge": "publication",
}

COMMON_ENTRY_KEYS = {"v", "id", "prev", "at", "kind", "signature"}
OPTIONAL_ENTRY_KEYS = {
    "prevs",
    "target",
    "authorized_by",
    "authorized_via",
    "payload",
    "body_enc",
    "operation_ref",
}
SIGNATURE_KEYS = {"alg", "key", "value"}
OPERATION_REF_KEYS = {"aithos-operation-core", "occurrence", "commitment"}
BODY_KEYS = {"hint", "n", "c"}

ENTRY_RE = re.compile(r"^gamma_[0-9A-HJKMNP-TV-Z]{26}$")
OPERATION_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SIGNATURE_RE = re.compile(r"^[0-9a-f]{128}$")
AT_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
SIGNING_SEED = bytes.fromhex("91" * 32)
SIGNING_KEY = Ed25519PrivateKey.from_private_bytes(SIGNING_SEED)

LEAF_DOMAIN = b"aithos-core/v1/mk-leaf\x00"
NODE_DOMAIN = b"aithos-core/v1/mk-node\x00"
ZEROS = b"\x00" * 32

HISTORICAL_FILES = (
    "f1-gamma-chain.json",
    "f2-gamma-counting.json",
    "f3-gamma-liveness.json",
    "h2-gamma-roots.json",
    "cb2-operation-projection.json",
)


class ProtocolError(Exception):
    code = "ProtocolError"


class GammaError(ProtocolError):
    code = INVALID_GAMMA


class OperationError(ProtocolError):
    code = INVALID_OPERATION


class ReplayError(ProtocolError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def reject_gamma(message: str) -> NoReturn:
    raise GammaError(message)


def reject_operation(message: str) -> NoReturn:
    raise OperationError(message)


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


def sha256_file(path: Path) -> str:
    return sha256_hex(path.read_bytes())


def commitment(domain: str, payload: bytes) -> str:
    return sha256_text(domain.encode() + b"\x00" + payload)


def fixture_ulid(ordinal: int) -> str:
    value = f"01K{'0' * 21}{ordinal:02d}"
    if len(value) != 26:
        raise AssertionError("fixture ULID length")
    return value


def b58(payload: bytes) -> str:
    number = int.from_bytes(payload, "big")
    out = ""
    while number:
        number, remainder = divmod(number, 58)
        out = BASE58[remainder] + out
    zeros = len(payload) - len(payload.lstrip(b"\x00"))
    return "1" * zeros + (out or ("" if zeros else "1"))


def b58_decode(value: str) -> bytes:
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
    return "z" + b58(b"\xed\x01" + public_bytes(key))


def decode_ed(value: Any) -> Ed25519PublicKey:
    if not isinstance(value, str) or not value.startswith("z"):
        reject_gamma("invalid Ed25519 multibase key")
    try:
        decoded = b58_decode(value[1:])
    except ValueError:
        reject_gamma("invalid Ed25519 multibase key")
    if len(decoded) != 34 or decoded[:2] != b"\xed\x01":
        reject_gamma("invalid Ed25519 multicodec key")
    try:
        return Ed25519PublicKey.from_public_bytes(decoded[2:])
    except ValueError:
        reject_gamma("invalid Ed25519 public key")


def require_exact(
    value: Any,
    keys: set[str],
    label: str,
    reject: Callable[[str], NoReturn],
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        reject(f"{label} has a non-exact member set")
    if any(item is None for item in value.values()):
        reject(f"{label} contains null")
    return value


def parse_at(value: Any) -> None:
    if not isinstance(value, str) or not AT_RE.fullmatch(value):
        reject_gamma("at is not canonical RFC3339 Z")
    try:
        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject_gamma("at is not a calendar instant")


def unsigned_entry(value: dict[str, Any]) -> dict[str, Any]:
    unsigned = clone(value)
    unsigned["signature"]["value"] = ""
    return unsigned


def sign_entry(value: dict[str, Any]) -> None:
    value["signature"]["value"] = SIGNING_KEY.sign(
        jcs(unsigned_entry(value)).encode()
    ).hex()


def verify_signature(value: dict[str, Any]) -> None:
    signature = require_exact(
        value["signature"],
        SIGNATURE_KEYS,
        "Gamma signature",
        reject_gamma,
    )
    if signature["alg"] != "ed25519":
        reject_gamma("Gamma signature algorithm is not ed25519")
    key = decode_ed(signature["key"])
    if not isinstance(signature["value"], str) or not SIGNATURE_RE.fullmatch(
        signature["value"]
    ):
        reject_gamma("malformed Gamma signature")
    try:
        key.verify(
            bytes.fromhex(signature["value"]),
            jcs(unsigned_entry(value)).encode(),
        )
    except InvalidSignature:
        reject_gamma("Gamma signature does not verify")


def operation_projection(kind: str, ordinal: int) -> dict[str, Any]:
    occurrence = f"op_{fixture_ulid(ordinal)}"
    operation_kind = KIND_TO_OPERATION[kind]
    facts_digest = sha256_text(f"facts:{kind}".encode())
    return {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": occurrence,
        "subject": f"did:aithos:{multibase_ed(SIGNING_KEY)}",
        "at": f"2026-07-18T12:{ordinal:02d}:00Z",
        "history_heads": [],
        "authority": {"actor": "owner"},
        "operation": {
            "kind": operation_kind,
            "facts_ref": {
                "aithos-operation-facts-core": FACTS_PROFILE,
                "digest": facts_digest,
            },
        },
    }


def operation_reference(projection: dict[str, Any]) -> dict[str, Any]:
    return {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            OPERATION_DOMAIN,
            jcs(projection).encode(),
        ),
    }


def clear_payload(kind: str) -> dict[str, Any]:
    return {
        "section.add": {"body_hash": sha256_text(b"section.add")},
        "section.modify": {"body_hash": sha256_text(b"section.modify")},
        "section.delete": {"body_hash": sha256_text(b"section.delete")},
        "section.redact": {"body_hash": sha256_text(b"section.redact")},
        "action": {
            "action": "send",
            "args_hash": sha256_text(b"action-args"),
        },
        "inference": {
            "provider": "local",
            "model": "small",
            "tokens_in": 10,
            "tokens_out": 4,
        },
        "grant": {"mandate": "mandate_01J00000000000000000000091"},
        "revoke": {"mandate": "mandate_01J00000000000000000000091"},
        "rotate": {"domain": "ethos-zone", "zone": "circle"},
        "merge": {
            "merges": [
                "sha256:" + "11" * 32,
                "sha256:" + "22" * 32,
            ]
        },
        "heartbeat": {"seq": 7},
    }[kind]


def entry_for_kind(kind: str, ordinal: int) -> tuple[dict[str, Any], Any]:
    entry: dict[str, Any] = {
        "v": 2,
        "id": f"gamma_{fixture_ulid(ordinal)}",
        "prev": "",
        "at": f"2026-07-18T12:{ordinal:02d}:00Z",
        "kind": kind,
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(SIGNING_KEY),
            "value": "",
        },
    }
    projection = None
    if kind in OPERATION_KINDS:
        projection = operation_projection(kind, ordinal)
        entry["operation_ref"] = operation_reference(projection)
    if kind == "ethos.read":
        entry["body_enc"] = {
            "hint": "33" * 32,
            "n": "44" * 24,
            "c": "55" * 48,
        }
    elif kind == "merge":
        entry["prev"] = "sha256:" + "31" * 32
        entry["prevs"] = [
            "sha256:" + "31" * 32,
            "sha256:" + "32" * 32,
        ]
        entry["payload"] = clear_payload(kind)
    else:
        entry["payload"] = clear_payload(kind)
        if kind.startswith("section."):
            entry["target"] = "public/sid_00000000000000000000000001"
        elif kind in {"action", "inference"}:
            entry["target"] = "x.mail"
        elif kind in {"grant", "revoke"}:
            entry["target"] = "mandate_01J00000000000000000000091"
    sign_entry(entry)
    validate_entry(entry, projection)
    return entry, projection


def validate_entry(
    value: Any,
    projection: dict[str, Any] | None = None,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        reject_gamma("Gamma entry is not an object")
    keys = set(value)
    if not COMMON_ENTRY_KEYS.issubset(keys) or not keys.issubset(
        COMMON_ENTRY_KEYS | OPTIONAL_ENTRY_KEYS
    ):
        reject_gamma("Gamma entry has a non-exact member set")
    if any(item is None for item in value.values()):
        reject_gamma("Gamma entry contains null")
    if value["v"] not in {1, 2}:
        reject_gamma("unsupported Gamma profile")
    if not isinstance(value["id"], str) or not ENTRY_RE.fullmatch(value["id"]):
        reject_gamma("invalid Gamma id")
    if value["prev"] != "" and (
        not isinstance(value["prev"], str)
        or not DIGEST_RE.fullmatch(value["prev"])
    ):
        reject_gamma("invalid Gamma predecessor")
    parse_at(value["at"])
    kind = value["kind"]
    if not isinstance(kind, str) or kind not in KINDS:
        reject_gamma("unknown Gamma kind")
    reference = value.get("operation_ref")
    required = value["v"] == 2 and kind in OPERATION_KINDS
    if required and reference is None:
        reject_gamma("Gamma v2 operation_ref is required")
    if not required and reference is not None:
        reject_gamma("Gamma operation_ref is forbidden")
    if reference is not None:
        reference = require_exact(
            reference,
            OPERATION_REF_KEYS,
            "operation_ref",
            reject_gamma,
        )
        if reference["aithos-operation-core"] != OPERATION_PROFILE:
            reject_gamma("unknown operation profile")
        if not isinstance(
            reference["occurrence"],
            str,
        ) or not OPERATION_RE.fullmatch(reference["occurrence"]):
            reject_gamma("invalid operation occurrence")
        if not isinstance(
            reference["commitment"],
            str,
        ) or not DIGEST_RE.fullmatch(reference["commitment"]):
            reject_gamma("invalid operation commitment")
        if projection is not None and reference != operation_reference(projection):
            reject_operation("Gamma operation_ref does not match projection")

    mutation = isinstance(kind, str) and kind.startswith("section.")
    if mutation:
        clear = (
            isinstance(value.get("target"), str)
            and isinstance(value.get("payload"), dict)
            and "body_enc" not in value
        )
        sealed = (
            "target" not in value
            and "payload" not in value
            and isinstance(value.get("body_enc"), dict)
        )
        if not clear and not sealed:
            reject_gamma("mutation payload/body form mismatch")
    elif kind == "ethos.read":
        if (
            "target" in value
            or "payload" in value
            or not isinstance(value.get("body_enc"), dict)
        ):
            reject_gamma("ethos.read must carry only a sealed body")
    else:
        if not isinstance(value.get("payload"), dict):
            reject_gamma("clear Gamma kind requires payload")
        if kind != "action" and "body_enc" in value:
            reject_gamma("only action may add a sealed body")

    if "body_enc" in value:
        body = require_exact(
            value["body_enc"],
            BODY_KEYS,
            "Gamma body_enc",
            reject_gamma,
        )
        if not all(isinstance(body[name], str) and body[name] for name in BODY_KEYS):
            reject_gamma("invalid Gamma body_enc")

    if kind == "merge":
        prevs = value.get("prevs")
        if (
            not isinstance(prevs, list)
            or len(prevs) != 2
            or not all(isinstance(item, str) and DIGEST_RE.fullmatch(item) for item in prevs)
            or prevs[0] == prevs[1]
            or value["prev"] != prevs[0]
            or "target" in value
        ):
            reject_gamma("invalid merge predecessor form")
    elif "prevs" in value:
        reject_gamma("only merge may carry prevs")

    verify_signature(value)
    return value


def kind_cases() -> list[dict[str, Any]]:
    out = []
    for ordinal, kind in enumerate(KINDS, start=1):
        entry, projection = entry_for_kind(kind, ordinal)
        out.append(
            {
                "kind": kind,
                "operation_ref_presence": (
                    "required" if kind in OPERATION_KINDS else "forbidden"
                ),
                "projection": projection,
                "entry": entry,
                "preimage_jcs": jcs(unsigned_entry(entry)),
                "entry_jcs": jcs(entry),
                "entry_hash": sha256_text(jcs(entry).encode()),
            }
        )
    return out


def case_error(
    identifier: str,
    candidate: Any,
    expected: type[ProtocolError],
    validator: Callable[[Any], Any],
) -> dict[str, Any]:
    try:
        validator(candidate)
    except expected as error:
        if error.code != expected.code:
            raise AssertionError(identifier) from error
    else:
        raise AssertionError(f"negative unexpectedly accepted: {identifier}")
    return {
        "id": identifier,
        "candidate": candidate,
        "must_fail": expected.code,
    }


def negative_entry_cases(
    positives: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    action = clone(next(case for case in positives if case["kind"] == "action"))
    heartbeat = clone(
        next(case for case in positives if case["kind"] == "heartbeat")
    )
    base = action["entry"]
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutate: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutate))

    add("unknown-version", lambda c: c["entry"].__setitem__("v", 3))
    add("invalid-id", lambda c: c["entry"].__setitem__("id", "gamma_bad"))
    add("invalid-at", lambda c: c["entry"].__setitem__("at", "2026-07-18 12:00:00"))
    add("invalid-prev", lambda c: c["entry"].__setitem__("prev", "sha256:00"))
    add("unknown-kind", lambda c: c["entry"].__setitem__("kind", "publication"))
    add("resolution-kind", lambda c: c["entry"].__setitem__("kind", "resolution"))
    add("gamma-read-kind", lambda c: c["entry"].__setitem__("kind", "gamma.read"))
    add("missing-operation-ref", lambda c: c["entry"].pop("operation_ref"))
    add("null-operation-ref", lambda c: c["entry"].__setitem__("operation_ref", None))
    add("operation-ref-extra-member", lambda c: c["entry"]["operation_ref"].__setitem__("extra", True))
    add("operation-ref-missing-occurrence", lambda c: c["entry"]["operation_ref"].pop("occurrence"))
    add("operation-ref-unknown-profile", lambda c: c["entry"]["operation_ref"].__setitem__("aithos-operation-core", "1.0.0-draft.2"))
    add("operation-ref-malformed-occurrence", lambda c: c["entry"]["operation_ref"].__setitem__("occurrence", "op_bad"))
    add("operation-ref-malformed-digest", lambda c: c["entry"]["operation_ref"].__setitem__("commitment", "sha256:00"))
    add("operation-ref-uppercase-digest", lambda c: c["entry"]["operation_ref"].__setitem__("commitment", "sha256:" + "AA" * 32))
    add("operation-ref-moved-to-payload", lambda c: c["entry"]["payload"].__setitem__("operation_ref", c["entry"].pop("operation_ref")))
    add("extra-top-level-member", lambda c: c["entry"].__setitem__("extra", True))
    add("signature-extra-member", lambda c: c["entry"]["signature"].__setitem__("extra", True))
    add("signature-algorithm", lambda c: c["entry"]["signature"].__setitem__("alg", "ecdsa"))
    add("signature-malformed-key", lambda c: c["entry"]["signature"].__setitem__("key", "zBad"))
    add("signature-malformed-value", lambda c: c["entry"]["signature"].__setitem__("value", "00"))
    add("tampered-after-sign", lambda c: c["entry"]["payload"].__setitem__("action", "purchase"))
    add("v1-operation-ref", lambda c: c["entry"].__setitem__("v", 1))

    out = []
    for identifier, mutate in cases:
        candidate = {
            "entry": clone(base),
            "projection": clone(action["projection"]),
        }
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                GammaError,
                lambda value: validate_entry(
                    value["entry"],
                    value["projection"],
                ),
            )
        )

    heartbeat_entry = heartbeat["entry"]
    candidate = {
        "entry": clone(heartbeat_entry),
        "projection": clone(action["projection"]),
    }
    candidate["entry"]["operation_ref"] = clone(action["entry"]["operation_ref"])
    out.append(
        case_error(
            "v2-heartbeat-operation-ref",
            candidate,
            GammaError,
            lambda value: validate_entry(value["entry"], value["projection"]),
        )
    )
    for kind in sorted(OPERATION_KINDS):
        positive = next(case for case in positives if case["kind"] == kind)
        candidate = {
            "entry": clone(positive["entry"]),
            "projection": clone(positive["projection"]),
        }
        candidate["entry"].pop("operation_ref")
        out.append(
            case_error(
                f"{kind}-missing-operation-ref",
                candidate,
                GammaError,
                lambda value: validate_entry(
                    value["entry"],
                    value["projection"],
                ),
            )
        )
    return out


def negative_correlation_cases(
    positives: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    action = next(case for case in positives if case["kind"] == "action")
    cases = []
    for identifier, member, value in [
        (
            "different-occurrence",
            "occurrence",
            f"op_{fixture_ulid(99)}",
        ),
        (
            "different-commitment",
            "commitment",
            "sha256:" + "99" * 32,
        ),
    ]:
        candidate = {
            "entry": clone(action["entry"]),
            "projection": clone(action["projection"]),
        }
        candidate["entry"]["operation_ref"][member] = value
        sign_entry(candidate["entry"])
        cases.append(
            case_error(
                identifier,
                candidate,
                OperationError,
                lambda item: validate_entry(
                    item["entry"],
                    item["projection"],
                ),
            )
        )
    return cases


def edge_verdict(
    parent_manifest: str,
    parent_gamma: str,
    child_manifest: str,
    child_gamma: str,
) -> bool:
    manifest_rank = {MANIFEST_V1: 1, MANIFEST_V2: 2}
    gamma_rank = {"v1": 1, "v2": 2}
    if (
        parent_manifest not in manifest_rank
        or child_manifest not in manifest_rank
        or parent_gamma not in gamma_rank
        or child_gamma not in gamma_rank
    ):
        return False
    if gamma_rank[parent_gamma] != manifest_rank[parent_manifest]:
        return False
    if gamma_rank[child_gamma] != manifest_rank[child_manifest]:
        return False
    return (
        manifest_rank[child_manifest] >= manifest_rank[parent_manifest]
        and gamma_rank[child_gamma] >= gamma_rank[parent_gamma]
    )


def monotonicity_cases() -> list[dict[str, Any]]:
    rows = [
        (MANIFEST_V1, "v1", MANIFEST_V1, "v1", True),
        (MANIFEST_V1, "v1", MANIFEST_V2, "v2", True),
        (MANIFEST_V2, "v2", MANIFEST_V2, "v2", True),
        (MANIFEST_V2, "v2", MANIFEST_V1, "v1", False),
        (MANIFEST_V1, "v1", MANIFEST_V1, "v2", False),
        (MANIFEST_V2, "v2", MANIFEST_V2, "v1", False),
        ("unknown", "v1", MANIFEST_V2, "v2", False),
        (MANIFEST_V1, "unknown", MANIFEST_V2, "v2", False),
    ]
    out = []
    for parent_manifest, parent_gamma, child_manifest, child_gamma, expected in rows:
        observed = edge_verdict(
            parent_manifest,
            parent_gamma,
            child_manifest,
            child_gamma,
        )
        if observed != expected:
            raise AssertionError("monotonicity drift")
        out.append(
            {
                "parent_manifest": parent_manifest,
                "parent_gamma": parent_gamma,
                "child_manifest": child_manifest,
                "child_gamma": child_gamma,
                "expected_accepted": expected,
            }
        )
    return out


def admit_occurrence(
    accepted: dict[str, str],
    reference: dict[str, Any],
) -> str:
    occurrence = reference["occurrence"]
    commitment_value = reference["commitment"]
    if occurrence in accepted:
        if accepted[occurrence] == commitment_value:
            return "refused-as-replay-before-tally"
        return "refused-as-equivocation-before-tally"
    accepted[occurrence] = commitment_value
    return "accepted-as-distinct-occurrence"


def occurrence_cases(
    positives: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    first = clone(
        next(case for case in positives if case["kind"] == "action")["entry"][
            "operation_ref"
        ]
    )
    accepted = {first["occurrence"]: first["commitment"]}
    candidates = [
        ("same-occurrence-same-commitment", clone(first)),
        (
            "same-occurrence-different-commitment",
            {**clone(first), "commitment": "sha256:" + "aa" * 32},
        ),
        (
            "different-occurrence-different-commitment-same-effect",
            {
                **clone(first),
                "occurrence": f"op_{fixture_ulid(98)}",
                "commitment": "sha256:" + "bb" * 32,
            },
        ),
    ]
    out = []
    for identifier, candidate in candidates:
        outcome = admit_occurrence(accepted, candidate)
        out.append(
            {
                "id": identifier,
                "operation_ref": candidate,
                "effect": "same",
                "expected": outcome,
            }
        )
    return out


def h_leaf(payload: bytes) -> bytes:
    return blake3_digest(LEAF_DOMAIN + payload)


def h_node(left: bytes, right: bytes) -> bytes:
    return blake3_digest(NODE_DOMAIN + left + right)


def blake3_digest(payload: bytes) -> bytes:
    """Run the independent cached Python blake3 wheel under Python 3.12."""
    environment = os.environ.copy()
    environment.setdefault(
        "UV_CACHE_DIR",
        "/private/tmp/aithos-cb2-uv-cache",
    )
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
    return bytes.fromhex(process.stdout)


def mroot(hashes: list[bytes]) -> bytes:
    if not hashes:
        return ZEROS
    if len(hashes) == 1:
        return hashes[0]
    middle = (len(hashes) + 1) // 2
    return h_node(mroot(hashes[:middle]), mroot(hashes[middle:]))


def raw_h2_fixture(
    positives: list[dict[str, Any]],
) -> dict[str, Any]:
    historical = json.loads((HERE / "f1-gamma-chain.json").read_text())
    lines = [historical["entry1_jcs"]]
    previous = historical["entry1_hash"]
    action_case = next(case for case in positives if case["kind"] == "action")
    first = clone(action_case["entry"])
    first["id"] = f"gamma_{fixture_ulid(81)}"
    first["prev"] = previous
    first["authorized_by"] = "mandate_01J00000000000000000000091"
    first["authorized_via"] = ["mandate_01J00000000000000000000091"]
    sign_entry(first)
    validate_entry(first, action_case["projection"])
    lines.append(jcs(first))

    second_projection = clone(action_case["projection"])
    second_projection["occurrence"] = f"op_{fixture_ulid(82)}"
    second_projection["at"] = "2026-07-18T13:02:00Z"
    second = clone(first)
    second["id"] = f"gamma_{fixture_ulid(82)}"
    second["prev"] = sha256_text(lines[-1].encode())
    second["at"] = "2026-07-18T13:02:00Z"
    second["operation_ref"] = operation_reference(second_projection)
    sign_entry(second)
    validate_entry(second, second_projection)
    lines.append(jcs(second))

    heartbeat_case = next(
        case for case in positives if case["kind"] == "heartbeat"
    )
    heartbeat = clone(heartbeat_case["entry"])
    heartbeat["id"] = f"gamma_{fixture_ulid(83)}"
    heartbeat["prev"] = sha256_text(lines[-1].encode())
    heartbeat["at"] = "2026-07-18T13:03:00Z"
    sign_entry(heartbeat)
    validate_entry(heartbeat)
    lines.append(jcs(heartbeat))

    root = mroot([h_leaf(line.encode()) for line in lines]).hex()
    actions = sum(
        1
        for line in lines
        if json.loads(line)["kind"] == "action"
        and "mandate_01J00000000000000000000091"
        in (json.loads(line).get("authorized_via") or [])
    )
    if actions != 2:
        raise AssertionError("raw H2 tally drift")
    return {
        "segment": "2026-07",
        "lines_jcs": lines,
        "root": root,
        "n": len(lines),
        "existing_counter_tally": {
            "mandate_01J00000000000000000000091": {
                "entries": 2,
                "actions": 2,
            }
        },
        "non_gamma_evidence": {
            "operation_ref": clone(first["operation_ref"]),
            "contributes_line": False,
            "contributes_count": False,
        },
        "mutation_counter_present": False,
        "total_consumption_counter_present": False,
    }


def semantic_verdict(candidate: dict[str, Any]) -> str:
    if not candidate["entry_valid"] or not candidate["signer_valid"]:
        raise ReplayError("InvalidGammaEntry", "invalid entry or signer")
    if not candidate["operation_valid"] or not candidate["leaf_possession"]:
        raise ReplayError("InvalidOperation", "invalid operation possession")
    if not candidate["time_valid"]:
        raise ReplayError("InvalidMandate", "operation outside mandate interval")
    if not candidate["chain_valid"] or not candidate["perimeter_valid"]:
        raise ReplayError("InvalidMandate", "invalid chain or perimeter")
    if not candidate["revocation_valid"]:
        raise ReplayError("MandateRevoked", "operation after revocation")
    if not candidate["heartbeat_valid"]:
        raise ReplayError("GammaHeartbeatStale", "stale heartbeat")
    if not candidate["grant_logged"]:
        raise ReplayError("GammaGrantNotLogged", "sub-grant absent from Gamma")
    if not candidate["receipts_valid"]:
        raise ReplayError(
            "GammaObligationUnsatisfied",
            "missing or mismatched receipt",
        )
    if not candidate["counters_valid"]:
        code = (
            "GammaBudgetExhausted"
            if candidate["counter_family"] == "action"
            else "InvalidMandate"
        )
        raise ReplayError(code, "counter exceeded")
    return "accepted"


def semantic_replay_cases() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    base = {
        "entry_valid": True,
        "operation_valid": True,
        "time_valid": True,
        "signer_valid": True,
        "leaf_possession": True,
        "chain_valid": True,
        "perimeter_valid": True,
        "revocation_valid": True,
        "heartbeat_valid": True,
        "receipts_valid": True,
        "counters_valid": True,
        "counter_family": "none",
        "grant_logged": True,
    }
    positives = []
    for operation_class, actor in [
        ("Ethos create", "owner"),
        ("Ethos edit", "grantee"),
        ("Ethos delete", "grantee"),
        ("connector action", "grantee"),
        ("metered inference", "grantee"),
        ("journalized read", "grantee"),
        ("sub-grant", "grantee"),
        ("scoped revocation", "grantee"),
        ("disjoint merge kind:merge", "grantee"),
    ]:
        candidate = {**base, "operation_class": operation_class, "actor": actor}
        if semantic_verdict(candidate) != "accepted":
            raise AssertionError(operation_class)
        positives.append({"candidate": candidate, "expected": "accepted"})

    defects = [
        ("action-limit-n-plus-one", "counters_valid", False, "action", "GammaBudgetExhausted"),
        ("receipt-replayed-under-another-consumption", "receipts_valid", False, "none", "GammaObligationUnsatisfied"),
        ("stale-heartbeat", "heartbeat_valid", False, "none", "GammaHeartbeatStale"),
        ("consumption-after-revocation", "revocation_valid", False, "none", "MandateRevoked"),
        ("sub-grant-absent-from-gamma", "grant_logged", False, "none", "GammaGrantNotLogged"),
        ("direct-child-beyond-max-children", "counters_valid", False, "max_children", "InvalidMandate"),
        ("mutation-outside-opaque-sid", "perimeter_valid", False, "none", "InvalidMandate"),
        ("valid-signature-wrong-chain", "chain_valid", False, "none", "InvalidMandate"),
        ("owner-entry-wrong-owner-key", "signer_valid", False, "none", "InvalidGammaEntry"),
        ("delegated-entry-without-leaf-possession", "leaf_possession", False, "none", "InvalidOperation"),
    ]
    negatives = []
    for identifier, member, value, counter_family, expected in defects:
        candidate = {
            **base,
            "operation_class": "fixture",
            "actor": "grantee",
            "counter_family": counter_family,
        }
        candidate[member] = value
        try:
            semantic_verdict(candidate)
        except ReplayError as error:
            if error.code != expected:
                raise AssertionError(identifier) from error
        else:
            raise AssertionError(f"semantic negative accepted: {identifier}")
        negatives.append(
            {
                "id": identifier,
                "candidate": candidate,
                "must_fail": expected,
                "accepted_prefix_and_counters_unchanged": True,
            }
        )
    return positives, negatives


def migration_merge(
    positives: list[dict[str, Any]],
) -> dict[str, Any]:
    historical = json.loads((HERE / "f1-gamma-chain.json").read_text())
    v2_parent = next(case for case in positives if case["kind"] == "action")
    merge = next(case for case in positives if case["kind"] == "merge")
    edges = [
        {
            "parent_manifest": MANIFEST_V1,
            "parent_gamma": "v1",
            "child_manifest": MANIFEST_V2,
            "child_gamma": "v2",
        },
        {
            "parent_manifest": MANIFEST_V2,
            "parent_gamma": "v2",
            "child_manifest": MANIFEST_V2,
            "child_gamma": "v2",
        },
    ]
    if not all(edge_verdict(**edge) for edge in edges):
        raise AssertionError("mixed-profile merge edge drift")
    return {
        "manifest_profile": MANIFEST_V2,
        "gamma_kind": "merge",
        "merge_entry": merge["entry"],
        "causal_edges": edges,
        "retained_parent_bytes": {
            "v1_entry_jcs": historical["entry3_jcs"],
            "v1_entry_sha256": sha256_hex(historical["entry3_jcs"].encode()),
            "v2_entry_jcs": v2_parent["entry_jcs"],
            "v2_entry_sha256": sha256_hex(v2_parent["entry_jcs"].encode()),
        },
        "physical_segment_order": ["v2-parent", "v1-parent", "v2-merge"],
        "physical_order_is_not_a_causal_edge": True,
        "publication_or_resolution_kind_added": False,
    }


def historical_hashes() -> dict[str, str]:
    return {name: sha256_file(HERE / name) for name in HISTORICAL_FILES}


def build_vector() -> dict[str, Any]:
    positives = kind_cases()
    entry_negatives = negative_entry_cases(positives)
    correlation_negatives = negative_correlation_cases(positives)
    semantic_positives, semantic_negatives = semantic_replay_cases()
    return {
        "vector": "CB2-GAMMA-V2-SEMANTIC-REPLAY-1",
        "description": (
            "Independent Python cryptography/blake3 oracle for signed Gamma v2 "
            "operation evidence, monotone migration, occurrence admission, raw "
            "H2 accounting and semantic replay against an exact prefix."
        ),
        "profiles": {
            "manifest_v1": MANIFEST_V1,
            "manifest_v2": MANIFEST_V2,
            "gamma_v1": 1,
            "gamma_v2": 2,
            "operation": OPERATION_PROFILE,
        },
        "deterministic_signing_seed_hex": SIGNING_SEED.hex(),
        "signing_public_key": multibase_ed(SIGNING_KEY),
        "kind_cases": positives,
        "negative_entry_cases": entry_negatives,
        "negative_correlation_cases": correlation_negatives,
        "monotonicity_cases": monotonicity_cases(),
        "migration_merge": migration_merge(positives),
        "occurrence_cases": occurrence_cases(positives),
        "raw_h2_fixture": raw_h2_fixture(positives),
        "semantic_replay_positive_cases": semantic_positives,
        "semantic_replay_negative_cases": semantic_negatives,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "registered_kinds": list(KINDS),
            "operation_bearing_kinds": sorted(OPERATION_KINDS),
            "heartbeat_has_no_operation_ref": True,
            "gamma_append_allocates_no_occurrence": True,
            "local_read_gamma_persists_no_artifact": True,
            "signed_presentation_uses_no_new_gamma_kind": True,
            "h2_remains_raw_and_unchanged": True,
            "entry_error_variant": INVALID_GAMMA,
            "correlation_error_variant": INVALID_OPERATION,
            "semantic_replay_requires_one_pure_front_door": True,
            "historical_bytes_are_not_reinterpreted": True,
            "entry_negative_ids": [
                case["id"] for case in entry_negatives
            ],
            "correlation_negative_ids": [
                case["id"] for case in correlation_negatives
            ],
            "semantic_negative_ids": [
                case["id"] for case in semantic_negatives
            ],
        },
    }


def encoded(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode()


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

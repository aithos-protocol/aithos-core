#!/usr/bin/env python3
"""Independent CB2 oracle for R2/U1 receipts and the draft3 matcher.

Python ``cryptography`` performs every Ed25519 operation.  The generator builds
independent W1 action, inference, mutation, read, grant, revoke, rotation and
publication contexts; signs the three closed receipt-v2 families; validates the
draft3 non-action obligation matcher; and proves historical v1 files unchanged.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Callable, NoReturn

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-operation-receipts.json"

OPERATION_KEY = "aithos-operation-core"
OPERATION_PROFILE = "1.0.0-draft.1"
MANDATE_KEY = "aithos-mandate-core"
MANDATE_DRAFT3 = "1.0.0-draft.3"
OPERATION_DOMAIN = "aithos-core/v1/operation-commitment"

INVALID_OBLIGATION = "GammaObligationUnsatisfied"
INVALID_GAMMA = "InvalidGammaEntry"
INVALID_MANDATE = "InvalidMandate"
MAX_U64 = 2**64 - 1

REFERENCE_KEYS = {OPERATION_KEY, "occurrence", "commitment"}
R2_BASE_KEYS = {
    "v",
    "family",
    "operation_ref",
    "obligation",
    "verdict",
    "at",
    "sig",
}
U1_ACTION_KEYS = {
    "v",
    "family",
    "operation_ref",
    "model",
    "tokens",
    "sig",
}
U1_INFERENCE_KEYS = {
    "v",
    "family",
    "operation_ref",
    "tokens_in",
    "tokens_out",
    "sig",
}

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SIGNATURE_RE = re.compile(r"^[0-9a-f]{128}$")
OCCURRENCE_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
AT_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")
DURATION_RE = re.compile(r"^([1-9][0-9]*)([smhd])$")

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

ATTESTOR_A_SEED = bytes.fromhex("71" * 32)
ATTESTOR_B_SEED = bytes.fromhex("72" * 32)
USAGE_SEED = bytes.fromhex("73" * 32)
STRANGER_SEED = bytes.fromhex("74" * 32)
GRANTEE_SEED = bytes.fromhex("75" * 32)
ROOT_SEED = bytes.fromhex("76" * 32)

HISTORICAL_FILES = (
    "gplus-obligations.json",
    "fplus-constraints.json",
    "eplus-attenuation.json",
    "cb2-operation-projection.json",
    "cb2-operation-facts-action-inference.json",
)


class ProtocolError(ValueError):
    code: str

    def __init__(self, detail: str):
        super().__init__(detail)


class ObligationError(ProtocolError):
    code = INVALID_OBLIGATION


class GammaError(ProtocolError):
    code = INVALID_GAMMA


class MandateError(ProtocolError):
    code = INVALID_MANDATE


def reject_obligation(detail: str) -> NoReturn:
    raise ObligationError(detail)


def reject_gamma(detail: str) -> NoReturn:
    raise GammaError(detail)


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


def commitment(domain: str, payload: bytes) -> str:
    digest = hashlib.sha256(domain.encode("ascii") + b"\x00" + payload).hexdigest()
    return f"sha256:{digest}"


def public_bytes(key: Ed25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def b58(data: bytes) -> str:
    zeros = len(data) - len(data.lstrip(b"\x00"))
    number = int.from_bytes(data, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58[remainder] + encoded
    return "1" * zeros + (encoded or ("" if zeros else "1"))


def multibase_ed(key: Ed25519PrivateKey) -> str:
    return "z" + b58(b"\xed\x01" + public_bytes(key))


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


def multibase_ed_public(
    value: Any,
    reject: Callable[[str], NoReturn],
) -> Ed25519PublicKey:
    if not isinstance(value, str) or not value.startswith("z"):
        reject("invalid Ed25519 multibase key")
    try:
        decoded = b58_decode(value[1:])
    except ValueError:
        reject("invalid Ed25519 multibase key")
    if len(decoded) != 34 or decoded[:2] != b"\xed\x01":
        reject("invalid Ed25519 multicodec key")
    try:
        return Ed25519PublicKey.from_public_bytes(decoded[2:])
    except ValueError:
        reject("invalid Ed25519 public key")


def parse_at(
    value: Any,
    label: str,
    reject: Callable[[str], NoReturn],
) -> datetime:
    if not isinstance(value, str) or not AT_RE.fullmatch(value):
        reject(f"{label} is not canonical RFC3339 Z")
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject(f"{label} is not a calendar instant")


def duration_seconds(
    value: Any,
    reject: Callable[[str], NoReturn],
) -> int:
    if not isinstance(value, str):
        reject("max_age is not a duration")
    match = DURATION_RE.fullmatch(value)
    if match is None:
        reject("max_age is not canonical")
    amount = int(match.group(1))
    multiplier = {"s": 1, "m": 60, "h": 3600, "d": 86400}[match.group(2)]
    return amount * multiplier


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


def require_u64(
    value: Any,
    label: str,
    reject: Callable[[str], NoReturn],
) -> int:
    if type(value) is not int or not 0 <= value <= MAX_U64:
        reject(f"{label} is not a JSON u64")
    return value


def validate_reference(
    value: Any,
    reject: Callable[[str], NoReturn],
) -> dict[str, Any]:
    reference = require_exact(value, REFERENCE_KEYS, "operation_ref", reject)
    if reference[OPERATION_KEY] != OPERATION_PROFILE:
        reject("unknown operation_ref profile")
    if not isinstance(reference["occurrence"], str) or not OCCURRENCE_RE.fullmatch(
        reference["occurrence"]
    ):
        reject("invalid operation occurrence")
    if not isinstance(reference["commitment"], str) or not COMMITMENT_RE.fullmatch(
        reference["commitment"]
    ):
        reject("invalid operation commitment")
    return reference


def verify_signature(
    keys: list[Ed25519PublicKey],
    message: bytes,
    signature: Any,
    reject: Callable[[str], NoReturn],
) -> None:
    if not isinstance(signature, str) or not SIGNATURE_RE.fullmatch(signature):
        reject("malformed Ed25519 signature")
    raw = bytes.fromhex(signature)
    for key in keys:
        try:
            key.verify(raw, message)
            return
        except InvalidSignature:
            pass
    reject("signature does not verify under a pinned key")


def sign_receipt(value: dict[str, Any], key: Ed25519PrivateKey) -> None:
    unsigned = {name: item for name, item in value.items() if name != "sig"}
    value["sig"] = key.sign(jcs(unsigned).encode()).hex()


def occurrence(number: int) -> str:
    value = f"op_01K{number:023d}"
    assert OCCURRENCE_RE.fullmatch(value)
    return value


def load_fact(case_id: str, vector_name: str) -> dict[str, Any]:
    vector = json.loads((HERE / vector_name).read_text())
    return next(
        clone(case)
        for case in vector["positive_cases"]
        if case["id"] == case_id
    )


def operation_context(
    kind: str,
    number: int,
    facts_ref: dict[str, Any],
    native: dict[str, Any],
) -> dict[str, Any]:
    grantee = multibase_ed(GRANTEE_KEY)
    projection = {
        OPERATION_KEY: OPERATION_PROFILE,
        "occurrence": occurrence(number),
        "subject": f"did:aithos:{multibase_ed(ROOT_KEY)}",
        "at": "2026-07-18T12:00:00Z",
        "history_heads": ["sha256:" + "71" * 32],
        "authority": {
            "actor": "grantee",
            "key": grantee,
            "authorized_by": "mandate_01J00000000000000000000071",
            "authorized_via": [
                {
                    "id": "mandate_01J00000000000000000000071",
                    "certificate_digest": "sha256:" + "72" * 32,
                }
            ],
        },
        "operation": {
            "kind": kind,
            "facts_ref": facts_ref,
        },
    }
    reference = {
        OPERATION_KEY: OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": commitment(
            OPERATION_DOMAIN,
            jcs(projection).encode(),
        ),
    }
    return {
        "kind": kind,
        "native": native,
        "projection": projection,
        "projection_jcs": jcs(projection),
        "operation_ref": reference,
    }


def operation_tuple(context: dict[str, Any]) -> dict[str, Any]:
    kind = context["kind"]
    native = context["native"]
    if kind == "read":
        return {"kind": "read", "domain": native["domain"]}
    if kind == "mutation":
        return {
            "kind": "mutation",
            "domain": native["domain"],
            "verb": native["verb"],
        }
    if kind in {"inference", "grant", "revoke"}:
        return {"kind": kind}
    if kind == "rotate":
        return {"kind": "rotate", "domain": native["domain"]}
    if kind == "publication":
        return {"kind": "publication", "mode": native["mode"]}
    reject_mandate("operation kind has no non-action matcher")


def validate_matcher(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or value.get("kind") is None:
        reject_mandate("matcher is not a closed object")
    kind = value["kind"]
    if kind == "read":
        item = require_exact(
            value,
            {"kind", "domain"},
            "read matcher",
            reject_mandate,
        )
        if item["domain"] not in {"ethos", "gamma", "vault-config"}:
            reject_mandate("unknown read matcher domain")
    elif kind == "mutation":
        item = require_exact(
            value,
            {"kind", "domain", "verb"},
            "mutation matcher",
            reject_mandate,
        )
        verbs = {
            "ethos": {"create", "edit", "delete", "redact"},
            "structure": {"create", "rename", "delete", "move"},
            "vault-config": {"create", "edit", "delete"},
        }
        if item["domain"] not in verbs or item["verb"] not in verbs[item["domain"]]:
            reject_mandate("unknown mutation matcher tuple")
    elif kind in {"inference", "grant", "revoke"}:
        item = require_exact(value, {"kind"}, f"{kind} matcher", reject_mandate)
    elif kind == "rotate":
        item = require_exact(
            value,
            {"kind", "domain"},
            "rotate matcher",
            reject_mandate,
        )
        if item["domain"] not in {"ethos-zone", "ethos-node", "vault", "identity"}:
            reject_mandate("unknown rotate matcher domain")
    elif kind == "publication":
        item = require_exact(
            value,
            {"kind", "mode"},
            "publication matcher",
            reject_mandate,
        )
        if item["mode"] not in {"normal", "merge", "resolution"}:
            reject_mandate("unknown publication matcher mode")
    else:
        reject_mandate("unknown or action matcher kind")
    if any(not isinstance(item, str) or not item for item in value.values()):
        reject_mandate("matcher contains a non-string or empty value")
    return value


def validate_obligation(profile: str, value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        reject_mandate("obligation is not an object")
    selectors = {"applies_to", "applies_to_operation"} & set(value)
    if len(selectors) != 1:
        reject_mandate("obligation must carry exactly one selector")
    optional = {"max_age"} if "max_age" in value else set()
    expected = {"id", "check", "attestor", "verdict"} | selectors | optional
    obligation = require_exact(value, expected, "obligation", reject_mandate)
    for name in ("id", "check", "verdict"):
        if not isinstance(obligation[name], str) or not obligation[name]:
            reject_mandate(f"obligation {name} is empty")
    attestors = obligation["attestor"]
    if (
        not isinstance(attestors, list)
        or not attestors
        or any(not isinstance(item, str) for item in attestors)
        or len(attestors) != len(set(attestors))
    ):
        reject_mandate("invalid attestor set")
    for attestor in attestors:
        multibase_ed_public(attestor, reject_mandate)
    if "max_age" in obligation:
        duration_seconds(obligation["max_age"], reject_mandate)
    if "applies_to" in obligation:
        if not isinstance(obligation["applies_to"], str) or not obligation["applies_to"]:
            reject_mandate("invalid historical action selector")
    else:
        if profile != MANDATE_DRAFT3:
            reject_mandate("non-action matcher requires draft3")
        validate_matcher(obligation["applies_to_operation"])
    if profile not in {
        "1.0.0-draft.1",
        "1.0.0-draft.2",
        MANDATE_DRAFT3,
    }:
        reject_mandate("unknown mandate profile")
    return obligation


def obligation_matches(
    profile: str,
    obligation: dict[str, Any],
    context: dict[str, Any],
) -> bool:
    item = validate_obligation(profile, obligation)
    if "applies_to_operation" in item:
        return item["applies_to_operation"] == operation_tuple(context)
    if context["kind"] != "action":
        return False
    selector = item["applies_to"]
    action = context["native"]["action_selector"]
    if selector.endswith(".*"):
        return action.startswith(selector[:-1])
    return selector == action


def validate_obligation_chain(chain: Any) -> None:
    if not isinstance(chain, list) or not chain:
        reject_mandate("obligation chain is empty")
    parent: dict[str, Any] | None = None
    for item in chain:
        if not isinstance(item, dict) or set(item) != {
            MANDATE_KEY,
            "id",
            "parent",
            "constraints",
        }:
            reject_mandate("matcher mandate fixture has a non-exact member set")
        record = item
        if any(
            value is None
            for name, value in record.items()
            if name != "parent"
        ):
            reject_mandate("matcher mandate fixture contains null")
        profile = record[MANDATE_KEY]
        if profile != MANDATE_DRAFT3:
            reject_mandate("matcher chain is not homogeneous draft3")
        constraints = require_exact(
            record["constraints"],
            {"obligations"},
            "matcher constraints",
            reject_mandate,
        )
        obligations = constraints["obligations"]
        if not isinstance(obligations, list):
            reject_mandate("obligations are not an array")
        by_id: dict[str, dict[str, Any]] = {}
        for obligation in obligations:
            validated = validate_obligation(profile, obligation)
            if validated["id"] in by_id:
                reject_mandate("duplicate obligation id")
            by_id[validated["id"]] = validated
        if parent is None:
            if record["parent"] is not None:
                reject_mandate("root matcher mandate has a parent")
        else:
            if record["parent"] != parent["id"]:
                reject_mandate("matcher mandate parent mismatch")
            inherited = {
                obligation["id"]: obligation
                for obligation in parent["constraints"]["obligations"]
            }
            for identifier, obligation in inherited.items():
                if by_id.get(identifier) != obligation:
                    reject_mandate("inherited obligation was dropped or altered")
        parent = record


def validate_r2(
    receipts: Any,
    context: dict[str, Any],
    profile: str,
    obligation: dict[str, Any],
) -> None:
    if not isinstance(receipts, list) or len(receipts) != 1:
        reject_obligation("exactly one R2 receipt is required")
    if not obligation_matches(profile, obligation, context):
        reject_obligation("obligation does not select this operation")
    receipt = receipts[0]
    keys = R2_BASE_KEYS | ({"presented_digest"} if isinstance(receipt, dict) and "presented_digest" in receipt else set())
    item = require_exact(receipt, keys, "R2 receipt", reject_obligation)
    if type(item["v"]) is not int or item["v"] != 2:
        reject_obligation("R2 version is not JSON number 2")
    if item["family"] != "obligation":
        reject_obligation("wrong R2 family")
    reference = validate_reference(item["operation_ref"], reject_obligation)
    if reference != context["operation_ref"]:
        reject_obligation("R2 operation_ref mismatch")
    if item["obligation"] != obligation["id"]:
        reject_obligation("R2 obligation id mismatch")
    if item["verdict"] != obligation["verdict"]:
        reject_obligation("R2 verdict mismatch")
    receipt_at = parse_at(item["at"], "R2 at", reject_obligation)
    operation_at = parse_at(
        context["projection"]["at"],
        "operation at",
        reject_obligation,
    )
    if "max_age" in obligation:
        delta = abs(int((operation_at - receipt_at).total_seconds()))
        if delta > duration_seconds(obligation["max_age"], reject_obligation):
            reject_obligation("R2 receipt is stale")
    if "presented_digest" in item and (
        not isinstance(item["presented_digest"], str)
        or not COMMITMENT_RE.fullmatch(item["presented_digest"])
    ):
        reject_obligation("invalid R2 presented_digest")
    unsigned = {name: value for name, value in item.items() if name != "sig"}
    verify_signature(
        [
            multibase_ed_public(key, reject_obligation)
            for key in obligation["attestor"]
        ],
        jcs(unsigned).encode(),
        item["sig"],
        reject_obligation,
    )


def validate_u1(
    receipts: Any,
    context: dict[str, Any],
    profile: dict[str, Any],
) -> int:
    profile = require_exact(
        profile,
        {"id", "models", "require_attestation", "attestation_key"},
        "U1 budget profile",
        reject_gamma,
    )
    if (
        not isinstance(profile["id"], str)
        or not profile["id"]
        or not isinstance(profile["models"], list)
        or not profile["models"]
        or any(not isinstance(model, str) or not model for model in profile["models"])
        or profile["require_attestation"] is not True
    ):
        reject_gamma("invalid U1 budget profile")
    if not isinstance(receipts, list) or len(receipts) != 1:
        reject_gamma("exactly one U1 receipt is required")
    receipt = receipts[0]
    if context["kind"] == "action":
        item = require_exact(
            receipt,
            U1_ACTION_KEYS,
            "U1 action receipt",
            reject_gamma,
        )
        if item["family"] != "usage.action":
            reject_gamma("wrong U1 action family")
        if not isinstance(item["model"], str) or not item["model"]:
            reject_gamma("U1 action model is empty")
        if item["model"] not in profile["models"]:
            reject_gamma("U1 action model is not allowed")
        actual = require_u64(item["tokens"], "U1 action tokens", reject_gamma)
    elif context["kind"] == "inference":
        item = require_exact(
            receipt,
            U1_INFERENCE_KEYS,
            "U1 inference receipt",
            reject_gamma,
        )
        if item["family"] != "usage.inference":
            reject_gamma("wrong U1 inference family")
        if context["native"]["model"] not in profile["models"]:
            reject_gamma("U1 inference model is not allowed")
        tokens_in = require_u64(item["tokens_in"], "U1 tokens_in", reject_gamma)
        tokens_out = require_u64(item["tokens_out"], "U1 tokens_out", reject_gamma)
        actual = tokens_in + tokens_out
        if actual > MAX_U64:
            reject_gamma("U1 inference total overflows u64")
    else:
        reject_gamma("U1 receipt on a non-usage operation")
    if type(item["v"]) is not int or item["v"] != 2:
        reject_gamma("U1 version is not JSON number 2")
    reference = validate_reference(item["operation_ref"], reject_gamma)
    if reference != context["operation_ref"]:
        reject_gamma("U1 operation_ref mismatch")
    unsigned = {name: value for name, value in item.items() if name != "sig"}
    verify_signature(
        [multibase_ed_public(profile["attestation_key"], reject_gamma)],
        jcs(unsigned).encode(),
        item["sig"],
        reject_gamma,
    )
    return actual


ATTESTOR_A = Ed25519PrivateKey.from_private_bytes(ATTESTOR_A_SEED)
ATTESTOR_B = Ed25519PrivateKey.from_private_bytes(ATTESTOR_B_SEED)
USAGE_KEY = Ed25519PrivateKey.from_private_bytes(USAGE_SEED)
STRANGER_KEY = Ed25519PrivateKey.from_private_bytes(STRANGER_SEED)
GRANTEE_KEY = Ed25519PrivateKey.from_private_bytes(GRANTEE_SEED)
ROOT_KEY = Ed25519PrivateKey.from_private_bytes(ROOT_SEED)


def build_contexts() -> dict[str, dict[str, Any]]:
    action = load_fact(
        "action-budget",
        "cb2-operation-facts-action-inference.json",
    )
    inference = load_fact(
        "inference-budget",
        "cb2-operation-facts-action-inference.json",
    )
    mutation = load_fact("ethos-edit", "cb2-operation-facts-mutation.json")
    structural = load_fact(
        "structure-move-folder",
        "cb2-operation-facts-mutation.json",
    )
    read = load_fact("ethos-public", "cb2-operation-facts-read.json")
    structural_facts = json.loads(
        (HERE / "cb2-operation-facts-structural.json").read_text()
    )

    def structural_case(case_id: str) -> dict[str, Any]:
        return next(
            clone(case)
            for case in structural_facts["positive_cases"]
            if case["id"] == case_id
        )

    return {
        "action": operation_context(
            "action",
            71,
            action["facts_ref"],
            {
                "action_selector": "act.x.mail.send",
                "model": "claude-haiku",
            },
        ),
        "inference": operation_context(
            "inference",
            72,
            inference["facts_ref"],
            {"provider": "anthropic", "model": "claude-haiku"},
        ),
        "mutation-ethos-edit": operation_context(
            "mutation",
            73,
            mutation["facts_ref"],
            {"domain": "ethos", "verb": "edit"},
        ),
        "mutation-structure-move": operation_context(
            "mutation",
            74,
            structural["facts_ref"],
            {"domain": "structure", "verb": "move"},
        ),
        "read-ethos": operation_context(
            "read",
            75,
            read["facts_ref"],
            {"domain": "ethos"},
        ),
        "grant": operation_context(
            "grant",
            76,
            structural_case("grant")["facts_ref"],
            {},
        ),
        "revoke": operation_context(
            "revoke",
            77,
            structural_case("revoke-no-reason")["facts_ref"],
            {},
        ),
        "rotate-vault": operation_context(
            "rotate",
            78,
            structural_case("rotate-vault")["facts_ref"],
            {"domain": "vault"},
        ),
        "publication-normal": operation_context(
            "publication",
            79,
            structural_case("publication-normal")["facts_ref"],
            {"mode": "normal"},
        ),
    }


def obligations() -> dict[str, dict[str, Any]]:
    attestors = [multibase_ed(ATTESTOR_A), multibase_ed(ATTESTOR_B)]
    return {
        "action": {
            "id": "send-approval",
            "check": "human.approve",
            "attestor": attestors,
            "applies_to": "act.x.mail.send",
            "verdict": "approve",
            "max_age": "5m",
        },
        "mutation": {
            "id": "edit-approval",
            "check": "human.approve",
            "attestor": attestors,
            "applies_to_operation": {
                "kind": "mutation",
                "domain": "ethos",
                "verb": "edit",
            },
            "verdict": "approve",
            "max_age": "5m",
        },
        "audit": {
            "id": "audit-read",
            "check": "guardrail.audit",
            "attestor": [multibase_ed(ATTESTOR_A)],
            "applies_to_operation": {"kind": "read", "domain": "ethos"},
            "verdict": "pass",
        },
    }


def r2_receipt(
    context: dict[str, Any],
    obligation: dict[str, Any],
    key: Ed25519PrivateKey,
    *,
    presented: bool,
) -> dict[str, Any]:
    receipt = {
        "v": 2,
        "family": "obligation",
        "operation_ref": clone(context["operation_ref"]),
        "obligation": obligation["id"],
        "verdict": obligation["verdict"],
        "at": "2026-07-18T11:58:00Z",
        "sig": "",
    }
    if presented:
        receipt["presented_digest"] = "sha256:" + "79" * 32
    sign_receipt(receipt, key)
    return receipt


def u1_action_receipt(context: dict[str, Any]) -> dict[str, Any]:
    receipt = {
        "v": 2,
        "family": "usage.action",
        "operation_ref": clone(context["operation_ref"]),
        "model": "claude-haiku",
        "tokens": 8412,
        "sig": "",
    }
    sign_receipt(receipt, USAGE_KEY)
    return receipt


def u1_inference_receipt(context: dict[str, Any]) -> dict[str, Any]:
    receipt = {
        "v": 2,
        "family": "usage.inference",
        "operation_ref": clone(context["operation_ref"]),
        "tokens_in": 1200,
        "tokens_out": 300,
        "sig": "",
    }
    sign_receipt(receipt, USAGE_KEY)
    return receipt


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


def r2_negative_cases(
    valid: dict[str, Any],
    context: dict[str, Any],
    obligation: dict[str, Any],
    other_context: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[list[dict[str, Any]]], None]]] = []

    def add(identifier: str, mutate: Callable[[list[dict[str, Any]]], None]) -> None:
        cases.append((identifier, mutate))

    add("missing-receipt", lambda c: c.clear())
    add("duplicate-receipt", lambda c: c.append(clone(c[0])))
    add("missing-version", lambda c: c[0].pop("v"))
    add("historical-v1-version", lambda c: c[0].__setitem__("v", 1))
    add("unknown-version", lambda c: c[0].__setitem__("v", 3))
    add("boolean-version", lambda c: c[0].__setitem__("v", True))
    add("unknown-family", lambda c: c[0].__setitem__("family", "approval"))
    add("extra-member", lambda c: c[0].__setitem__("extra", True))
    add("null-verdict", lambda c: c[0].__setitem__("verdict", None))
    add("missing-operation-ref", lambda c: c[0].pop("operation_ref"))
    add("operation-ref-extra", lambda c: c[0]["operation_ref"].__setitem__("extra", True))
    add("operation-ref-malformed-commitment", lambda c: c[0]["operation_ref"].__setitem__("commitment", "sha256:00"))
    add("operation-ref-other-occurrence", lambda c: c[0].__setitem__("operation_ref", clone(other_context["operation_ref"])))
    add("wrong-obligation", lambda c: c[0].__setitem__("obligation", "other-approval"))
    add("wrong-verdict", lambda c: c[0].__setitem__("verdict", "reject"))
    add("invalid-at", lambda c: c[0].__setitem__("at", "2026-07-18T11:58:00+00:00"))
    add("stale-before", lambda c: c[0].__setitem__("at", "2026-07-18T11:54:59Z"))
    add("stale-after", lambda c: c[0].__setitem__("at", "2026-07-18T12:05:01Z"))
    add("bad-presented-digest", lambda c: c[0].__setitem__("presented_digest", "sha256:00"))
    add("malformed-signature", lambda c: c[0].__setitem__("sig", "00"))
    add("tampered-after-sign", lambda c: c[0].__setitem__("at", "2026-07-18T11:59:00Z"))

    def stranger(c: list[dict[str, Any]]) -> None:
        sign_receipt(c[0], STRANGER_KEY)

    add("stranger-signature", stranger)
    add("copied-v1-mandate-id", lambda c: c[0].__setitem__("mandate_id", "mandate_01J00000000000000000000071"))
    add("copied-v1-action", lambda c: c[0].__setitem__("action", "send"))
    add("copied-v1-args-hash", lambda c: c[0].__setitem__("args_hash", "sha256:" + "01" * 32))

    out = []
    for identifier, mutate in cases:
        candidate = [clone(valid)]
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                ObligationError,
                lambda value: validate_r2(
                    value,
                    context,
                    "1.0.0-draft.2",
                    obligation,
                ),
            )
        )
    return out


def u1_negative_cases(
    action_valid: dict[str, Any],
    inference_valid: dict[str, Any],
    action_context: dict[str, Any],
    inference_context: dict[str, Any],
    profile: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, str, Callable[[list[dict[str, Any]]], None]]] = []

    def add(
        identifier: str,
        family: str,
        mutate: Callable[[list[dict[str, Any]]], None],
    ) -> None:
        cases.append((identifier, family, mutate))

    add("action-missing-receipt", "action", lambda c: c.clear())
    add("action-duplicate-receipt", "action", lambda c: c.append(clone(c[0])))
    add("action-missing-version", "action", lambda c: c[0].pop("v"))
    add("action-historical-v1-version", "action", lambda c: c[0].__setitem__("v", 1))
    add("action-unknown-version", "action", lambda c: c[0].__setitem__("v", 3))
    add("action-boolean-version", "action", lambda c: c[0].__setitem__("v", True))
    add("action-wrong-family", "action", lambda c: c[0].__setitem__("family", "usage.inference"))
    add("action-extra-member", "action", lambda c: c[0].__setitem__("extra", True))
    add("action-null-model", "action", lambda c: c[0].__setitem__("model", None))
    add("action-empty-model", "action", lambda c: c[0].__setitem__("model", ""))
    add("action-model-not-allowed", "action", lambda c: c[0].__setitem__("model", "gpt-oss"))
    add("action-negative-tokens", "action", lambda c: c[0].__setitem__("tokens", -1))
    add("action-float-tokens", "action", lambda c: c[0].__setitem__("tokens", 1.5))
    add("action-boolean-tokens", "action", lambda c: c[0].__setitem__("tokens", True))
    add("action-missing-tokens", "action", lambda c: c[0].pop("tokens"))
    add("action-wrong-operation", "action", lambda c: c[0].__setitem__("operation_ref", clone(inference_context["operation_ref"])))
    add("action-reference-extra", "action", lambda c: c[0]["operation_ref"].__setitem__("extra", True))
    add("action-malformed-signature", "action", lambda c: c[0].__setitem__("sig", "00"))
    add("action-tampered-after-sign", "action", lambda c: c[0].__setitem__("tokens", 8413))

    def action_stranger(c: list[dict[str, Any]]) -> None:
        sign_receipt(c[0], STRANGER_KEY)

    add("action-stranger-signature", "action", action_stranger)
    add("action-copied-v1-args-hash", "action", lambda c: c[0].__setitem__("args_hash", "sha256:" + "01" * 32))
    add("inference-missing-tokens-in", "inference", lambda c: c[0].pop("tokens_in"))
    add("inference-missing-tokens-out", "inference", lambda c: c[0].pop("tokens_out"))
    add("inference-negative-tokens-in", "inference", lambda c: c[0].__setitem__("tokens_in", -1))
    add("inference-float-tokens-out", "inference", lambda c: c[0].__setitem__("tokens_out", 0.5))
    add("inference-total-overflow", "inference", lambda c: (c[0].__setitem__("tokens_in", MAX_U64), c[0].__setitem__("tokens_out", 1)))
    add("inference-wrong-family", "inference", lambda c: c[0].__setitem__("family", "usage.action"))
    add("inference-action-model-member", "inference", lambda c: c[0].__setitem__("model", "claude-haiku"))
    add("inference-action-tokens-member", "inference", lambda c: c[0].__setitem__("tokens", 1500))
    add("inference-wrong-operation", "inference", lambda c: c[0].__setitem__("operation_ref", clone(action_context["operation_ref"])))
    add("inference-tampered-after-sign", "inference", lambda c: c[0].__setitem__("tokens_out", 301))

    out = []
    for identifier, family, mutate in cases:
        base = action_valid if family == "action" else inference_valid
        context = action_context if family == "action" else inference_context
        candidate = [clone(base)]
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                GammaError,
                lambda value, context=context: validate_u1(
                    value,
                    context,
                    profile,
                ),
            )
        )
    return out


def matcher_cases(
    contexts: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    rows = [
        ({"kind": "read", "domain": "ethos"}, "read-ethos", True),
        ({"kind": "mutation", "domain": "ethos", "verb": "edit"}, "mutation-ethos-edit", True),
        ({"kind": "mutation", "domain": "structure", "verb": "move"}, "mutation-structure-move", True),
        ({"kind": "inference"}, "inference", True),
        ({"kind": "grant"}, "grant", True),
        ({"kind": "revoke"}, "revoke", True),
        ({"kind": "rotate", "domain": "vault"}, "rotate-vault", True),
        ({"kind": "publication", "mode": "normal"}, "publication-normal", True),
        ({"kind": "mutation", "domain": "ethos", "verb": "edit"}, "mutation-structure-move", False),
    ]
    out = []
    for index, (matcher, context_id, expected) in enumerate(rows, start=1):
        obligation = {
            "id": f"matcher-{index}",
            "check": "human.approve",
            "attestor": [multibase_ed(ATTESTOR_A)],
            "applies_to_operation": matcher,
            "verdict": "approve",
        }
        observed = obligation_matches(
            MANDATE_DRAFT3,
            obligation,
            contexts[context_id],
        )
        if observed != expected:
            raise AssertionError(f"matcher case {index}")
        out.append(
            {
                "id": f"matcher-{index}",
                "matcher": matcher,
                "context": context_id,
                "expected_applicable": expected,
            }
        )
    return out


def matcher_negative_cases(
    base: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, str, Callable[[dict[str, Any]], None]]] = []

    def add(
        identifier: str,
        profile: str,
        mutate: Callable[[dict[str, Any]], None],
    ) -> None:
        cases.append((identifier, profile, mutate))

    add("draft1-non-action-selector", "1.0.0-draft.1", lambda c: None)
    add("draft2-non-action-selector", "1.0.0-draft.2", lambda c: None)
    add("both-selectors", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to", "act.x.mail.send"))
    add("missing-selector", MANDATE_DRAFT3, lambda c: c.pop("applies_to_operation"))
    add("null-selector", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", None))
    add("extra-obligation-member", MANDATE_DRAFT3, lambda c: c.__setitem__("extra", True))
    add("empty-obligation-id", MANDATE_DRAFT3, lambda c: c.__setitem__("id", ""))
    add("empty-attestor-set", MANDATE_DRAFT3, lambda c: c.__setitem__("attestor", []))
    add("duplicate-attestor", MANDATE_DRAFT3, lambda c: c.__setitem__("attestor", [c["attestor"][0], c["attestor"][0]]))
    add("unknown-profile", "1.0.0-draft.4", lambda c: None)
    add("unknown-matcher-kind", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "future"}))
    add("action-matcher-kind", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "action"}))
    add("read-missing-domain", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "read"}))
    add("read-unknown-domain", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "read", "domain": "self"}))
    add("mutation-missing-verb", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "mutation", "domain": "ethos"}))
    add("mutation-invalid-combination", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "mutation", "domain": "vault-config", "verb": "move"}))
    add("inference-extra-domain", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "inference", "domain": "provider"}))
    add("rotate-unknown-domain", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "rotate", "domain": "connector"}))
    add("publication-unknown-mode", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "publication", "mode": "genesis"}))
    add("matcher-null-member", MANDATE_DRAFT3, lambda c: c.__setitem__("applies_to_operation", {"kind": "mutation", "domain": "ethos", "verb": None}))

    out = []
    for identifier, profile, mutate in cases:
        candidate = clone(base)
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                {"profile": profile, "obligation": candidate},
                MandateError,
                lambda value: validate_obligation(
                    value["profile"],
                    value["obligation"],
                ),
            )
        )
    return out


def matcher_chain_material(
    obligation_set: dict[str, dict[str, Any]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    root = {
        MANDATE_KEY: MANDATE_DRAFT3,
        "id": "mandate_01J00000000000000000000081",
        "parent": None,
        "constraints": {"obligations": [clone(obligation_set["mutation"])]},
    }
    child = {
        MANDATE_KEY: MANDATE_DRAFT3,
        "id": "mandate_01J00000000000000000000082",
        "parent": root["id"],
        "constraints": {
            "obligations": [
                clone(obligation_set["mutation"]),
                clone(obligation_set["audit"]),
            ]
        },
    }
    positive = [root, child]
    validate_obligation_chain(positive)

    candidates = []
    dropped = clone(positive)
    dropped[1]["constraints"]["obligations"] = [clone(obligation_set["audit"])]
    candidates.append(("dropped-inherited-obligation", dropped))
    altered = clone(positive)
    altered[1]["constraints"]["obligations"][0]["max_age"] = "1h"
    candidates.append(("altered-inherited-obligation", altered))
    mixed = clone(positive)
    mixed[1][MANDATE_KEY] = "1.0.0-draft.2"
    candidates.append(("mixed-version-matcher-chain", mixed))
    wrong_parent = clone(positive)
    wrong_parent[1]["parent"] = "mandate_01J00000000000000000000099"
    candidates.append(("wrong-parent-matcher-chain", wrong_parent))

    negatives = [
        case_error(
            identifier,
            candidate,
            MandateError,
            validate_obligation_chain,
        )
        for identifier, candidate in candidates
    ]
    return positive, negatives


def historical_hashes_and_checks() -> dict[str, str]:
    fplus = json.loads((HERE / "fplus-constraints.json").read_text())
    attestation = fplus["attestation"]
    receipt = attestation["receipt"]
    payload = {
        "args_hash": receipt["args_hash"],
        "model": receipt["model"],
        "tokens": receipt["tokens"],
    }
    if jcs(payload) != attestation["receipt_jcs_signed"]:
        raise AssertionError("historical U1-v1 JCS drift")
    Ed25519PublicKey.from_public_bytes(
        bytes.fromhex(attestation["provider_pub_hex"])
    ).verify(
        bytes.fromhex(receipt["sig"]),
        jcs(payload).encode(),
    )
    return {name: sha256_file(HERE / name) for name in HISTORICAL_FILES}


def build_vector() -> dict[str, Any]:
    contexts = build_contexts()
    obligation_set = obligations()
    budget_profile = {
        "id": "haiku",
        "models": ["claude-haiku"],
        "require_attestation": True,
        "attestation_key": multibase_ed(USAGE_KEY),
    }
    r2_without = r2_receipt(
        contexts["action"],
        obligation_set["action"],
        ATTESTOR_A,
        presented=False,
    )
    r2_with = r2_receipt(
        contexts["action"],
        obligation_set["action"],
        ATTESTOR_B,
        presented=True,
    )
    r2_mutation = r2_receipt(
        contexts["mutation-ethos-edit"],
        obligation_set["mutation"],
        ATTESTOR_A,
        presented=True,
    )
    u1_action = u1_action_receipt(contexts["action"])
    u1_inference = u1_inference_receipt(contexts["inference"])

    validate_r2(
        [r2_without],
        contexts["action"],
        "1.0.0-draft.2",
        obligation_set["action"],
    )
    validate_r2(
        [r2_with],
        contexts["action"],
        "1.0.0-draft.2",
        obligation_set["action"],
    )
    validate_r2(
        [r2_mutation],
        contexts["mutation-ethos-edit"],
        MANDATE_DRAFT3,
        obligation_set["mutation"],
    )
    action_actual = validate_u1(
        [u1_action],
        contexts["action"],
        budget_profile,
    )
    inference_actual = validate_u1(
        [u1_inference],
        contexts["inference"],
        budget_profile,
    )
    if (action_actual, inference_actual) != (8412, 1500):
        raise AssertionError("unexpected usage totals")

    chain, chain_negatives = matcher_chain_material(obligation_set)
    r2_negatives = r2_negative_cases(
        r2_without,
        contexts["action"],
        obligation_set["action"],
        contexts["inference"],
    )
    u1_negatives = u1_negative_cases(
        u1_action,
        u1_inference,
        contexts["action"],
        contexts["inference"],
        budget_profile,
    )
    matcher_negatives = matcher_negative_cases(obligation_set["mutation"])

    return {
        "vector": "CB2-R2-U1-OPERATION-RECEIPTS-1",
        "description": (
            "Independent Python cryptography oracle for exact R2 obligation and "
            "U1 action/inference receipts, operation-bound Ed25519 signatures, "
            "historical v1 non-regression and the closed homogeneous-draft3 "
            "non-action obligation matcher."
        ),
        "profiles": {
            "operation": OPERATION_PROFILE,
            "receipt": 2,
            "matcher_mandate": MANDATE_DRAFT3,
        },
        "deterministic_private_seed_hex": {
            "attestor_a": ATTESTOR_A_SEED.hex(),
            "attestor_b": ATTESTOR_B_SEED.hex(),
            "usage": USAGE_SEED.hex(),
            "stranger": STRANGER_SEED.hex(),
            "grantee": GRANTEE_SEED.hex(),
            "root": ROOT_SEED.hex(),
        },
        "public_keys": {
            "attestor_a": multibase_ed(ATTESTOR_A),
            "attestor_b": multibase_ed(ATTESTOR_B),
            "usage": multibase_ed(USAGE_KEY),
            "stranger": multibase_ed(STRANGER_KEY),
        },
        "contexts": contexts,
        "budget_profile": budget_profile,
        "obligations": obligation_set,
        "positive_receipts": {
            "r2_without_presented_digest": {
                "receipt": r2_without,
                "preimage_jcs": jcs(
                    {name: value for name, value in r2_without.items() if name != "sig"}
                ),
            },
            "r2_with_presented_digest": {
                "receipt": r2_with,
                "preimage_jcs": jcs(
                    {name: value for name, value in r2_with.items() if name != "sig"}
                ),
            },
            "r2_draft3_mutation": {
                "receipt": r2_mutation,
                "preimage_jcs": jcs(
                    {name: value for name, value in r2_mutation.items() if name != "sig"}
                ),
            },
            "u1_action": {
                "receipt": u1_action,
                "preimage_jcs": jcs(
                    {name: value for name, value in u1_action.items() if name != "sig"}
                ),
                "actual_tokens": action_actual,
            },
            "u1_inference": {
                "receipt": u1_inference,
                "preimage_jcs": jcs(
                    {name: value for name, value in u1_inference.items() if name != "sig"}
                ),
                "actual_tokens": inference_actual,
            },
        },
        "matcher_cases": matcher_cases(contexts),
        "draft3_obligation_chain": chain,
        "negative_r2_cases": r2_negatives,
        "negative_u1_cases": u1_negatives,
        "negative_matcher_cases": matcher_negatives,
        "negative_matcher_chain_cases": chain_negatives,
        "historical_vector_sha256": historical_hashes_and_checks(),
        "inventory": {
            "r2_negative_ids": [case["id"] for case in r2_negatives],
            "u1_negative_ids": [case["id"] for case in u1_negatives],
            "matcher_negative_ids": [case["id"] for case in matcher_negatives],
            "matcher_chain_negative_ids": [
                case["id"] for case in chain_negatives
            ],
            "r2_error_variant": INVALID_OBLIGATION,
            "u1_error_variant": INVALID_GAMMA,
            "matcher_error_variant": INVALID_MANDATE,
            "historical_v1_is_not_reinterpreted": True,
            "sig_is_omitted_from_preimage": True,
            "receipts_are_not_operation_projection_inputs": True,
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

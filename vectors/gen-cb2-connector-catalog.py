#!/usr/bin/env python3
"""Independent CB2 oracle for CAT1 connector catalog authority.

Python ``cryptography`` signs and verifies an owner DID, a connector-signer
catalog, and the distinct owner-content approval. The oracle validates complete
content addresses, the homogeneous draft3 ``catalog_pins`` constraint, class
decisions, K1.2 catalog references, and exact typed negative inventories.
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
from cryptography.hazmat.primitives.asymmetric.x25519 import (
    X25519PrivateKey,
    X25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-connector-catalog.json"

DID_KEY = "aithos-did-core"
DID_PROFILE = "1.0.0-draft.1"
CATALOG_KEY = "aithos-connector-catalog-core"
CATALOG_PROFILE = "1.0.0-draft.1"
APPROVAL_KEY = "aithos-connector-catalog-approval-core"
APPROVAL_PROFILE = "1.0.0-draft.1"
MANDATE_KEY = "aithos-mandate-core"
MANDATE_DRAFT3 = "1.0.0-draft.3"
FACTS_KEY = "aithos-operation-facts-core"
FACTS_PROFILE = "1.0.0-draft.1"
FACTS_DOMAIN = "aithos-core/v1/operation-facts"

INVALID_CATALOG = "InvalidCatalog"
INVALID_MANDATE = "InvalidMandate"
INVALID_FACTS = "InvalidOperationFacts"

SIGNATURE_KEYS = {"alg", "key", "value"}
DID_KEYS = {
    DID_KEY,
    "id",
    "keys",
    "bundle",
    "revocations",
    "signature",
}
DID_PUBLIC_KEYS = {"root", "content", "kex", "succession"}
CATALOG_KEYS = {
    CATALOG_KEY,
    "connector",
    "catalog_version",
    "actions",
    "signature",
}
ACTION_KEYS = {"name", "class"}
APPROVAL_KEYS = {
    APPROVAL_KEY,
    "subject",
    "connector",
    "catalog_version",
    "catalog_digest",
    "approved_at",
    "signature",
}
PIN_KEYS = {
    "connector",
    "catalog_version",
    "catalog_digest",
    "approval_digest",
}
ACTION_FACT_KEYS = {
    "connector",
    "action",
    "catalog_ref",
    "args_hash",
    "budget",
    "purpose",
}
CATALOG_REF_KEYS = {
    "catalog_version",
    "catalog_digest",
    "approval_digest",
}

IDENTIFIER_RE = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")
VERSION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
DIGEST_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SIGNATURE_RE = re.compile(r"^[0-9a-f]{128}$")
AT_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"

ROOT_SEED = bytes.fromhex("81" * 32)
CONTENT_SEED = bytes.fromhex("82" * 32)
SUCCESSION_SEED = bytes.fromhex("83" * 32)
CATALOG_SIGNER_SEED = bytes.fromhex("84" * 32)
STRANGER_SEED = bytes.fromhex("85" * 32)
GRANTEE_SEED = bytes.fromhex("86" * 32)
KEX_SEED = bytes.fromhex("87" * 32)

HISTORICAL_FILES = (
    "a2-did.json",
    "e1-mandate.json",
    "cb2-operation-facts-action-inference.json",
    "cb2-operation-receipts.json",
    "gplus-obligations.json",
)


class ProtocolError(ValueError):
    code: str


class CatalogError(ProtocolError):
    code = INVALID_CATALOG


class MandateError(ProtocolError):
    code = INVALID_MANDATE


class FactsError(ProtocolError):
    code = INVALID_FACTS


def reject_catalog(detail: str) -> NoReturn:
    raise CatalogError(detail)


def reject_mandate(detail: str) -> NoReturn:
    raise MandateError(detail)


def reject_facts(detail: str) -> NoReturn:
    raise FactsError(detail)


def clone(value: Any) -> Any:
    return copy.deepcopy(value)


def jcs(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def sha256_text(payload: bytes) -> str:
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def commitment(domain: str, payload: bytes) -> str:
    return sha256_text(domain.encode("ascii") + b"\x00" + payload)


def public_ed(key: Ed25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def public_x(key: X25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def b58(data: bytes) -> str:
    zeros = len(data) - len(data.lstrip(b"\x00"))
    number = int.from_bytes(data, "big")
    encoded = ""
    while number:
        number, remainder = divmod(number, 58)
        encoded = BASE58[remainder] + encoded
    return "1" * zeros + (encoded or ("" if zeros else "1"))


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


def multibase_ed(key: Ed25519PrivateKey) -> str:
    return "z" + b58(b"\xed\x01" + public_ed(key))


def multibase_x(key: X25519PrivateKey) -> str:
    return "z" + b58(b"\xec\x01" + public_x(key))


def decode_ed(
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


def decode_x(
    value: Any,
    reject: Callable[[str], NoReturn],
) -> X25519PublicKey:
    if not isinstance(value, str) or not value.startswith("z"):
        reject("invalid X25519 multibase key")
    try:
        decoded = b58_decode(value[1:])
    except ValueError:
        reject("invalid X25519 multibase key")
    if len(decoded) != 34 or decoded[:2] != b"\xec\x01":
        reject("invalid X25519 multicodec key")
    try:
        return X25519PublicKey.from_public_bytes(decoded[2:])
    except ValueError:
        reject("invalid X25519 public key")


def require_exact(
    value: Any,
    keys: set[str],
    label: str,
    reject: Callable[[str], NoReturn],
    *,
    nullable: set[str] | None = None,
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        reject(f"{label} has a non-exact member set")
    allowed = nullable or set()
    if any(item is None for name, item in value.items() if name not in allowed):
        reject(f"{label} contains null")
    return value


def parse_at(value: Any, reject: Callable[[str], NoReturn]) -> None:
    if not isinstance(value, str) or not AT_RE.fullmatch(value):
        reject("approved_at is not canonical RFC3339 Z")
    try:
        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject("approved_at is not a calendar instant")


def unsigned_document(value: dict[str, Any]) -> dict[str, Any]:
    unsigned = clone(value)
    unsigned["signature"]["value"] = ""
    return unsigned


def sign_document(value: dict[str, Any], key: Ed25519PrivateKey) -> None:
    value["signature"]["value"] = key.sign(
        jcs(unsigned_document(value)).encode()
    ).hex()


def verify_hex_signature(
    key: Ed25519PublicKey,
    message: bytes,
    signature: Any,
    reject: Callable[[str], NoReturn],
) -> None:
    if not isinstance(signature, str) or not SIGNATURE_RE.fullmatch(signature):
        reject("malformed Ed25519 signature")
    try:
        key.verify(bytes.fromhex(signature), message)
    except InvalidSignature:
        reject("Ed25519 signature does not verify")


def validate_signature_block(
    value: Any,
    expected_key: str | None,
    reject: Callable[[str], NoReturn],
) -> dict[str, Any]:
    signature = require_exact(
        value,
        SIGNATURE_KEYS,
        "signature",
        reject,
    )
    if signature["alg"] != "ed25519":
        reject("signature algorithm is not ed25519")
    if expected_key is not None and signature["key"] != expected_key:
        reject("signature key selector mismatch")
    return signature


ROOT_KEY = Ed25519PrivateKey.from_private_bytes(ROOT_SEED)
CONTENT_KEY = Ed25519PrivateKey.from_private_bytes(CONTENT_SEED)
SUCCESSION_KEY = Ed25519PrivateKey.from_private_bytes(SUCCESSION_SEED)
CATALOG_SIGNER = Ed25519PrivateKey.from_private_bytes(CATALOG_SIGNER_SEED)
STRANGER_KEY = Ed25519PrivateKey.from_private_bytes(STRANGER_SEED)
GRANTEE_KEY = Ed25519PrivateKey.from_private_bytes(GRANTEE_SEED)
KEX_KEY = X25519PrivateKey.from_private_bytes(KEX_SEED)


def owner_did_document() -> dict[str, Any]:
    root = multibase_ed(ROOT_KEY)
    document = {
        DID_KEY: DID_PROFILE,
        "id": f"did:aithos:{root}",
        "keys": {
            "root": root,
            "content": multibase_ed(CONTENT_KEY),
            "kex": multibase_x(KEX_KEY),
            "succession": multibase_ed(SUCCESSION_KEY),
        },
        "bundle": ["file://local"],
        "revocations": "gamma/gamma.jsonl",
        "signature": {
            "alg": "ed25519",
            "key": "#root",
            "value": "",
        },
    }
    sign_document(document, ROOT_KEY)
    validate_did(document)
    return document


def validate_did(value: Any) -> dict[str, Any]:
    document = require_exact(value, DID_KEYS, "DID document", reject_catalog)
    if document[DID_KEY] != DID_PROFILE:
        reject_catalog("unknown DID profile")
    keys = require_exact(
        document["keys"],
        DID_PUBLIC_KEYS,
        "DID keys",
        reject_catalog,
    )
    root = decode_ed(keys["root"], reject_catalog)
    decode_ed(keys["content"], reject_catalog)
    decode_x(keys["kex"], reject_catalog)
    decode_ed(keys["succession"], reject_catalog)
    if document["id"] != f"did:aithos:{keys['root']}":
        reject_catalog("DID id/root mismatch")
    signature = validate_signature_block(
        document["signature"],
        "#root",
        reject_catalog,
    )
    verify_hex_signature(
        root,
        jcs(unsigned_document(document)).encode(),
        signature["value"],
        reject_catalog,
    )
    return document


def signed_catalog() -> dict[str, Any]:
    catalog = {
        CATALOG_KEY: CATALOG_PROFILE,
        "connector": "mail",
        "catalog_version": "2026.07",
        "actions": [
            {"name": "list", "class": "read"},
            {"name": "purchase", "class": "binding"},
            {"name": "send", "class": "act"},
        ],
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(CATALOG_SIGNER),
            "value": "",
        },
    }
    sign_document(catalog, CATALOG_SIGNER)
    validate_catalog(catalog, sha256_text(jcs(catalog).encode()))
    return catalog


def validate_catalog(value: Any, claimed_digest: Any) -> dict[str, Any]:
    catalog = require_exact(value, CATALOG_KEYS, "catalog", reject_catalog)
    if catalog[CATALOG_KEY] != CATALOG_PROFILE:
        reject_catalog("unknown catalog profile")
    if not isinstance(catalog["connector"], str) or not IDENTIFIER_RE.fullmatch(
        catalog["connector"]
    ):
        reject_catalog("invalid connector id")
    if not isinstance(
        catalog["catalog_version"],
        str,
    ) or not VERSION_RE.fullmatch(catalog["catalog_version"]):
        reject_catalog("invalid catalog version")
    actions = catalog["actions"]
    if not isinstance(actions, list) or not actions:
        reject_catalog("catalog actions must be a non-empty array")
    names = []
    for raw in actions:
        action = require_exact(raw, ACTION_KEYS, "catalog action", reject_catalog)
        if not isinstance(action["name"], str) or not IDENTIFIER_RE.fullmatch(
            action["name"]
        ):
            reject_catalog("invalid action name")
        if not isinstance(action["class"], str) or action["class"] not in {
            "read",
            "act",
            "binding",
        }:
            reject_catalog("invalid action class")
        names.append(action["name"])
    if names != sorted(names) or len(names) != len(set(names)):
        reject_catalog("catalog actions are not unique and sorted")
    signature = validate_signature_block(
        catalog["signature"],
        None,
        reject_catalog,
    )
    signer = decode_ed(signature["key"], reject_catalog)
    verify_hex_signature(
        signer,
        jcs(unsigned_document(catalog)).encode(),
        signature["value"],
        reject_catalog,
    )
    digest = sha256_text(jcs(catalog).encode())
    if not isinstance(claimed_digest, str) or claimed_digest != digest:
        reject_catalog("catalog digest mismatch")
    return catalog


def signed_approval(
    catalog: dict[str, Any],
    document: dict[str, Any],
) -> dict[str, Any]:
    catalog_digest = sha256_text(jcs(catalog).encode())
    approval = {
        APPROVAL_KEY: APPROVAL_PROFILE,
        "subject": document["id"],
        "connector": catalog["connector"],
        "catalog_version": catalog["catalog_version"],
        "catalog_digest": catalog_digest,
        "approved_at": "2026-07-18T12:00:00Z",
        "signature": {
            "alg": "ed25519",
            "key": "#content",
            "value": "",
        },
    }
    sign_document(approval, CONTENT_KEY)
    validate_approval(
        approval,
        sha256_text(jcs(approval).encode()),
        catalog,
        document,
    )
    return approval


def validate_approval(
    value: Any,
    claimed_digest: Any,
    catalog: dict[str, Any],
    document: dict[str, Any],
) -> dict[str, Any]:
    approval = require_exact(
        value,
        APPROVAL_KEYS,
        "catalog approval",
        reject_catalog,
    )
    validate_did(clone(document))
    catalog_digest = sha256_text(jcs(catalog).encode())
    validate_catalog(catalog, catalog_digest)
    if approval[APPROVAL_KEY] != APPROVAL_PROFILE:
        reject_catalog("unknown approval profile")
    if approval["subject"] != document["id"]:
        reject_catalog("approval subject mismatch")
    if approval["connector"] != catalog["connector"]:
        reject_catalog("approval connector mismatch")
    if approval["catalog_version"] != catalog["catalog_version"]:
        reject_catalog("approval catalog version mismatch")
    if approval["catalog_digest"] != catalog_digest:
        reject_catalog("approval catalog digest mismatch")
    parse_at(approval["approved_at"], reject_catalog)
    signature = validate_signature_block(
        approval["signature"],
        "#content",
        reject_catalog,
    )
    content = decode_ed(document["keys"]["content"], reject_catalog)
    verify_hex_signature(
        content,
        jcs(unsigned_document(approval)).encode(),
        signature["value"],
        reject_catalog,
    )
    digest = sha256_text(jcs(approval).encode())
    if not isinstance(claimed_digest, str) or claimed_digest != digest:
        reject_catalog("approval digest mismatch")
    return approval


def catalog_pin(
    catalog: dict[str, Any],
    approval: dict[str, Any],
) -> dict[str, Any]:
    return {
        "connector": catalog["connector"],
        "catalog_version": catalog["catalog_version"],
        "catalog_digest": sha256_text(jcs(catalog).encode()),
        "approval_digest": sha256_text(jcs(approval).encode()),
    }


def validate_pin(
    value: Any,
    catalog: dict[str, Any],
    approval: dict[str, Any],
    document: dict[str, Any],
) -> dict[str, Any]:
    pin = require_exact(value, PIN_KEYS, "catalog pin", reject_mandate)
    for name in ("catalog_digest", "approval_digest"):
        if not isinstance(pin[name], str) or not DIGEST_RE.fullmatch(pin[name]):
            reject_mandate(f"invalid {name}")
    catalog_digest = sha256_text(jcs(catalog).encode())
    approval_digest = sha256_text(jcs(approval).encode())
    try:
        validate_catalog(catalog, catalog_digest)
        validate_approval(
            approval,
            approval_digest,
            catalog,
            document,
        )
    except CatalogError as error:
        reject_mandate(f"pin selects invalid evidence: {error}")
    expected = catalog_pin(catalog, approval)
    if pin != expected:
        reject_mandate("catalog pin/evidence mismatch")
    return pin


def business_connectors(perimeter: Any) -> set[str]:
    if not isinstance(perimeter, list) or not perimeter:
        reject_mandate("perimeter is not a non-empty array")
    connectors = set()
    for entry in perimeter:
        if not isinstance(entry, str):
            reject_mandate("perimeter entry is not a string")
        if not entry.startswith("act.x."):
            continue
        parts = entry.split(".")
        if len(parts) != 4:
            reject_mandate("malformed connector perimeter")
        connector, action = parts[2], parts[3]
        if not IDENTIFIER_RE.fullmatch(connector):
            reject_mandate("invalid connector perimeter id")
        if action != "*" and not IDENTIFIER_RE.fullmatch(action):
            reject_mandate("invalid connector action id")
        if action != "config":
            connectors.add(connector)
    return connectors


def validate_chain(
    chain: Any,
    catalog: dict[str, Any],
    approval: dict[str, Any],
    document: dict[str, Any],
) -> None:
    if not isinstance(chain, list) or not chain:
        reject_mandate("catalog chain is empty")
    parent = None
    root_pins = None
    for record in chain:
        item = require_exact(
            record,
            {MANDATE_KEY, "id", "parent", "perimeter", "constraints"},
            "catalog mandate fixture",
            reject_mandate,
            nullable={"parent"},
        )
        if item[MANDATE_KEY] != MANDATE_DRAFT3:
            reject_mandate("catalog authority requires homogeneous draft3")
        if not isinstance(item["id"], str) or not MANDATE_RE.fullmatch(item["id"]):
            reject_mandate("invalid catalog mandate id")
        constraints = require_exact(
            item["constraints"],
            {"catalog_pins"},
            "catalog constraints",
            reject_mandate,
        )
        pins = constraints["catalog_pins"]
        if not isinstance(pins, list) or not pins:
            reject_mandate("catalog_pins must be a non-empty array")
        connector_names = []
        for pin in pins:
            validated = validate_pin(pin, catalog, approval, document)
            connector_names.append(validated["connector"])
        if connector_names != sorted(connector_names) or len(connector_names) != len(
            set(connector_names)
        ):
            reject_mandate("catalog pins are not unique and sorted")
        connectors = business_connectors(item["perimeter"])
        if parent is None:
            if item["parent"] is not None:
                reject_mandate("root catalog mandate has a parent")
            if set(connector_names) != connectors:
                reject_mandate("initial catalog pin coverage mismatch")
            root_pins = clone(pins)
        else:
            if item["parent"] != parent["id"]:
                reject_mandate("catalog mandate parent mismatch")
            if pins != root_pins:
                reject_mandate("catalog pins changed through attenuation")
            if not connectors.issubset(set(connector_names)):
                reject_mandate("child uses an unpinned connector")
        parent = item


def validate_action_facts(
    value: Any,
    pin: dict[str, Any],
    catalog: dict[str, Any],
    approval: dict[str, Any],
    document: dict[str, Any],
) -> str:
    facts = require_exact(
        value,
        ACTION_FACT_KEYS,
        "action facts",
        reject_facts,
    )
    reference = require_exact(
        facts["catalog_ref"],
        CATALOG_REF_KEYS,
        "catalog_ref",
        reject_facts,
    )
    try:
        validate_pin(pin, catalog, approval, document)
    except MandateError as error:
        reject_facts(f"invalid catalog authority: {error}")
    expected_ref = {
        "catalog_version": pin["catalog_version"],
        "catalog_digest": pin["catalog_digest"],
        "approval_digest": pin["approval_digest"],
    }
    if reference != expected_ref:
        reject_facts("action catalog_ref does not match mandate pin")
    if facts["connector"] != pin["connector"]:
        reject_facts("action connector does not match mandate pin")
    classes = {
        row["name"]: row["class"]
        for row in catalog["actions"]
    }
    action = facts["action"]
    if action not in classes:
        reject_facts("action is absent from approved catalog")
    return classes[action]


def authorize_action(
    catalog: dict[str, Any],
    action: str,
    authority: str,
    co_sign: bool,
) -> bool:
    classes = {row["name"]: row["class"] for row in catalog["actions"]}
    if action not in classes:
        return False
    exact = f"act.x.{catalog['connector']}.{action}"
    wildcard = f"act.x.{catalog['connector']}.*"
    action_class = classes[action]
    if authority == exact:
        return action_class != "binding" or co_sign
    if authority == wildcard:
        return action_class in {"read", "act"}
    return False


def positive_chain(pin: dict[str, Any]) -> list[dict[str, Any]]:
    root = {
        MANDATE_KEY: MANDATE_DRAFT3,
        "id": "mandate_01J00000000000000000000091",
        "parent": None,
        "perimeter": ["act.x.mail.*", "issue#depth=1"],
        "constraints": {"catalog_pins": [clone(pin)]},
    }
    child = {
        MANDATE_KEY: MANDATE_DRAFT3,
        "id": "mandate_01J00000000000000000000092",
        "parent": root["id"],
        "perimeter": ["act.x.mail.send"],
        "constraints": {"catalog_pins": [clone(pin)]},
    }
    return [root, child]


def action_facts(pin: dict[str, Any]) -> dict[str, Any]:
    source = json.loads(
        (HERE / "cb2-operation-facts-action-inference.json").read_text()
    )
    facts = clone(
        next(
            case["facts"]
            for case in source["positive_cases"]
            if case["id"] == "action-budget"
        )
    )
    facts["catalog_ref"] = {
        "catalog_version": pin["catalog_version"],
        "catalog_digest": pin["catalog_digest"],
        "approval_digest": pin["approval_digest"],
    }
    document = {
        FACTS_KEY: FACTS_PROFILE,
        "facts": facts,
    }
    return {
        "facts": facts,
        "facts_jcs": jcs(document),
        "facts_ref": {
            FACTS_KEY: FACTS_PROFILE,
            "digest": commitment(FACTS_DOMAIN, jcs(document).encode()),
        },
    }


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


def catalog_negative_cases(
    valid: dict[str, Any],
    digest: str,
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutate: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutate))

    add("missing-profile", lambda c: c["catalog"].pop(CATALOG_KEY))
    add("unknown-profile", lambda c: c["catalog"].__setitem__(CATALOG_KEY, "1.0.0-draft.2"))
    add("extra-member", lambda c: c["catalog"].__setitem__("extra", True))
    add("null-connector", lambda c: c["catalog"].__setitem__("connector", None))
    add("uppercase-connector", lambda c: c["catalog"].__setitem__("connector", "Mail"))
    add("dotted-connector", lambda c: c["catalog"].__setitem__("connector", "mail.api"))
    add("empty-version", lambda c: c["catalog"].__setitem__("catalog_version", ""))
    add("invalid-version", lambda c: c["catalog"].__setitem__("catalog_version", "2026/07"))
    add("actions-not-array", lambda c: c["catalog"].__setitem__("actions", {}))
    add("empty-actions", lambda c: c["catalog"].__setitem__("actions", []))
    add("unsorted-actions", lambda c: c["catalog"]["actions"].reverse())
    add("duplicate-action", lambda c: c["catalog"]["actions"].append(clone(c["catalog"]["actions"][-1])))
    add("action-missing-name", lambda c: c["catalog"]["actions"][0].pop("name"))
    add("action-extra-member", lambda c: c["catalog"]["actions"][0].__setitem__("extra", True))
    add("action-null-class", lambda c: c["catalog"]["actions"][0].__setitem__("class", None))
    add("action-uppercase-name", lambda c: c["catalog"]["actions"][0].__setitem__("name", "List"))
    add("action-dotted-name", lambda c: c["catalog"]["actions"][0].__setitem__("name", "mail.list"))
    add("action-unknown-class", lambda c: c["catalog"]["actions"][0].__setitem__("class", "write"))
    add("action-array-class", lambda c: c["catalog"]["actions"][0].__setitem__("class", ["read", "act"]))
    add("signature-missing-key", lambda c: c["catalog"]["signature"].pop("key"))
    add("signature-extra-member", lambda c: c["catalog"]["signature"].__setitem__("extra", True))
    add("signature-algorithm", lambda c: c["catalog"]["signature"].__setitem__("alg", "ecdsa"))
    add("signature-malformed-key", lambda c: c["catalog"]["signature"].__setitem__("key", "zBad"))
    add("signature-malformed-value", lambda c: c["catalog"]["signature"].__setitem__("value", "00"))
    add("tampered-after-sign", lambda c: c["catalog"]["actions"][2].__setitem__("class", "binding"))

    def signed_by_stranger(c: dict[str, Any]) -> None:
        sign_document(c["catalog"], STRANGER_KEY)

    add("signed-by-unannounced-stranger", signed_by_stranger)
    add("claimed-digest-mismatch", lambda c: c.__setitem__("claimed_digest", "sha256:" + "00" * 32))

    out = []
    for identifier, mutate in cases:
        candidate = {"catalog": clone(valid), "claimed_digest": digest}
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                CatalogError,
                lambda value: validate_catalog(
                    value["catalog"],
                    value["claimed_digest"],
                ),
            )
        )
    return out


def approval_negative_cases(
    valid: dict[str, Any],
    digest: str,
    catalog: dict[str, Any],
    document: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutate: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutate))

    add("missing-profile", lambda c: c["approval"].pop(APPROVAL_KEY))
    add("unknown-profile", lambda c: c["approval"].__setitem__(APPROVAL_KEY, "1.0.0-draft.2"))
    add("extra-member", lambda c: c["approval"].__setitem__("extra", True))
    add("null-subject", lambda c: c["approval"].__setitem__("subject", None))
    add("wrong-subject", lambda c: c["approval"].__setitem__("subject", "did:aithos:zWrong"))
    add("wrong-connector", lambda c: c["approval"].__setitem__("connector", "social"))
    add("wrong-version", lambda c: c["approval"].__setitem__("catalog_version", "2026.08"))
    add("malformed-catalog-digest", lambda c: c["approval"].__setitem__("catalog_digest", "sha256:00"))
    add("different-catalog-digest", lambda c: c["approval"].__setitem__("catalog_digest", "sha256:" + "00" * 32))
    add("invalid-approved-at", lambda c: c["approval"].__setitem__("approved_at", "2026-07-18T12:00:00+00:00"))
    add("calendar-invalid-approved-at", lambda c: c["approval"].__setitem__("approved_at", "2026-02-30T12:00:00Z"))
    add("signature-missing-key", lambda c: c["approval"]["signature"].pop("key"))
    add("signature-extra-member", lambda c: c["approval"]["signature"].__setitem__("extra", True))
    add("signature-algorithm", lambda c: c["approval"]["signature"].__setitem__("alg", "ecdsa"))
    add("signature-root-key", lambda c: c["approval"]["signature"].__setitem__("key", "#root"))
    add("signature-full-content-key", lambda c: c["approval"]["signature"].__setitem__("key", document["keys"]["content"]))
    add("signature-malformed-value", lambda c: c["approval"]["signature"].__setitem__("value", "00"))
    add("tampered-after-sign", lambda c: c["approval"].__setitem__("approved_at", "2026-07-18T12:00:01Z"))

    def signed_by_catalog(c: dict[str, Any]) -> None:
        sign_document(c["approval"], CATALOG_SIGNER)

    def signed_by_root(c: dict[str, Any]) -> None:
        sign_document(c["approval"], ROOT_KEY)

    def signed_by_grantee(c: dict[str, Any]) -> None:
        sign_document(c["approval"], GRANTEE_KEY)

    add("signed-by-catalog-signer", signed_by_catalog)
    add("signed-by-owner-root", signed_by_root)
    add("signed-by-grantee", signed_by_grantee)
    add("claimed-approval-digest-mismatch", lambda c: c.__setitem__("claimed_digest", "sha256:" + "00" * 32))

    out = []
    for identifier, mutate in cases:
        candidate = {"approval": clone(valid), "claimed_digest": digest}
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                CatalogError,
                lambda value: validate_approval(
                    value["approval"],
                    value["claimed_digest"],
                    catalog,
                    clone(document),
                ),
            )
        )
    return out


def chain_negative_cases(
    valid: list[dict[str, Any]],
    catalog: dict[str, Any],
    approval: dict[str, Any],
    document: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[list[dict[str, Any]]], None]]] = []

    def add(
        identifier: str,
        mutate: Callable[[list[dict[str, Any]]], None],
    ) -> None:
        cases.append((identifier, mutate))

    add("draft1-root", lambda c: c[0].__setitem__(MANDATE_KEY, "1.0.0-draft.1"))
    add("draft2-root", lambda c: c[0].__setitem__(MANDATE_KEY, "1.0.0-draft.2"))
    add("mixed-version-child", lambda c: c[1].__setitem__(MANDATE_KEY, "1.0.0-draft.2"))
    add("missing-catalog-pins", lambda c: c[0]["constraints"].pop("catalog_pins"))
    add("empty-catalog-pins", lambda c: c[0]["constraints"].__setitem__("catalog_pins", []))
    add("pin-extra-member", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("extra", True))
    add("pin-malformed-catalog-digest", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("catalog_digest", "sha256:00"))
    add("pin-wrong-catalog-digest", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("catalog_digest", "sha256:" + "00" * 32))
    add("pin-wrong-approval-digest", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("approval_digest", "sha256:" + "00" * 32))
    add("pin-wrong-version", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("catalog_version", "2026.08"))
    add("pin-wrong-connector", lambda c: c[0]["constraints"]["catalog_pins"][0].__setitem__("connector", "social"))
    add("duplicate-pin", lambda c: c[0]["constraints"]["catalog_pins"].append(clone(c[0]["constraints"]["catalog_pins"][0])))
    add("unrelated-pin", lambda c: c[0]["perimeter"].__setitem__(0, "act.x.social.*"))
    add("pin-for-config-only", lambda c: c[0]["perimeter"].__setitem__(0, "act.x.mail.config"))
    add("child-drops-pin", lambda c: c[1]["constraints"].__setitem__("catalog_pins", []))
    add("child-changes-pin", lambda c: c[1]["constraints"]["catalog_pins"][0].__setitem__("catalog_version", "2026.08"))
    add("child-adds-pin", lambda c: c[1]["constraints"]["catalog_pins"].append(clone(c[1]["constraints"]["catalog_pins"][0])))
    add("child-unpinned-connector", lambda c: c[1]["perimeter"].__setitem__(0, "act.x.social.send"))
    add("wrong-parent", lambda c: c[1].__setitem__("parent", "mandate_01J00000000000000000000099"))

    out = []
    for identifier, mutate in cases:
        candidate = clone(valid)
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                MandateError,
                lambda value: validate_chain(
                    value,
                    catalog,
                    approval,
                    clone(document),
                ),
            )
        )
    return out


def facts_negative_cases(
    valid: dict[str, Any],
    pin: dict[str, Any],
    catalog: dict[str, Any],
    approval: dict[str, Any],
    document: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutate: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutate))

    add("catalog-ref-missing-version", lambda c: c["catalog_ref"].pop("catalog_version"))
    add("catalog-ref-extra-member", lambda c: c["catalog_ref"].__setitem__("extra", True))
    add("catalog-ref-wrong-version", lambda c: c["catalog_ref"].__setitem__("catalog_version", "2026.08"))
    add("catalog-ref-wrong-catalog-digest", lambda c: c["catalog_ref"].__setitem__("catalog_digest", "sha256:" + "00" * 32))
    add("catalog-ref-wrong-approval-digest", lambda c: c["catalog_ref"].__setitem__("approval_digest", "sha256:" + "00" * 32))
    add("connector-mismatch", lambda c: c.__setitem__("connector", "social"))
    add("action-absent", lambda c: c.__setitem__("action", "delete"))
    add("caller-supplied-class", lambda c: c.__setitem__("class", "act"))

    out = []
    for identifier, mutate in cases:
        candidate = clone(valid)
        mutate(candidate)
        out.append(
            case_error(
                identifier,
                candidate,
                FactsError,
                lambda value: validate_action_facts(
                    value,
                    pin,
                    catalog,
                    approval,
                    clone(document),
                ),
            )
        )
    return out


def class_cases(catalog: dict[str, Any]) -> list[dict[str, Any]]:
    rows = [
        ("list", "act.x.mail.*", False, True),
        ("send", "act.x.mail.*", False, True),
        ("purchase", "act.x.mail.*", True, False),
        ("purchase", "act.x.mail.purchase", False, False),
        ("purchase", "act.x.mail.purchase", True, True),
        ("send", "act.x.social.*", False, False),
        ("config", "act.x.mail.*", False, False),
    ]
    out = []
    for action, authority, receipt, expected in rows:
        observed = authorize_action(catalog, action, authority, receipt)
        if observed != expected:
            raise AssertionError((action, authority, receipt))
        out.append(
            {
                "action": action,
                "authority": authority,
                "owner_co_sign": receipt,
                "expected_authorized": expected,
            }
        )
    return out


def historical_hashes() -> dict[str, str]:
    return {name: sha256_file(HERE / name) for name in HISTORICAL_FILES}


def build_vector() -> dict[str, Any]:
    document = owner_did_document()
    catalog = signed_catalog()
    catalog_digest = sha256_text(jcs(catalog).encode())
    approval = signed_approval(catalog, document)
    approval_digest = sha256_text(jcs(approval).encode())
    pin = catalog_pin(catalog, approval)
    chain = positive_chain(pin)
    validate_chain(chain, catalog, approval, clone(document))
    facts_material = action_facts(pin)
    derived_class = validate_action_facts(
        facts_material["facts"],
        pin,
        catalog,
        approval,
        clone(document),
    )
    if derived_class != "act":
        raise AssertionError("send class drift")

    catalog_negatives = catalog_negative_cases(catalog, catalog_digest)
    approval_negatives = approval_negative_cases(
        approval,
        approval_digest,
        catalog,
        document,
    )
    chain_negatives = chain_negative_cases(
        chain,
        catalog,
        approval,
        document,
    )
    facts_negatives = facts_negative_cases(
        facts_material["facts"],
        pin,
        catalog,
        approval,
        document,
    )

    return {
        "vector": "CB2-CAT1-CONNECTOR-CATALOG-1",
        "description": (
            "Independent Python cryptography oracle for the signed CAT1 "
            "connector catalog, distinct owner-content approval, complete "
            "content addresses, homogeneous draft3 catalog_pins, K1.2 "
            "catalog_ref binding and read/act/binding authorization."
        ),
        "profiles": {
            "did": DID_PROFILE,
            "catalog": CATALOG_PROFILE,
            "approval": APPROVAL_PROFILE,
            "mandate": MANDATE_DRAFT3,
            "operation_facts": FACTS_PROFILE,
        },
        "deterministic_private_seed_hex": {
            "root": ROOT_SEED.hex(),
            "content": CONTENT_SEED.hex(),
            "succession": SUCCESSION_SEED.hex(),
            "catalog_signer": CATALOG_SIGNER_SEED.hex(),
            "stranger": STRANGER_SEED.hex(),
            "grantee": GRANTEE_SEED.hex(),
            "kex": KEX_SEED.hex(),
        },
        "owner_did": {
            "document": document,
            "document_jcs": jcs(document),
        },
        "catalog": {
            "document": catalog,
            "preimage_jcs": jcs(unsigned_document(catalog)),
            "document_jcs": jcs(catalog),
            "catalog_digest": catalog_digest,
        },
        "approval": {
            "document": approval,
            "preimage_jcs": jcs(unsigned_document(approval)),
            "document_jcs": jcs(approval),
            "approval_digest": approval_digest,
        },
        "catalog_pin": pin,
        "draft3_chain": chain,
        "action_facts": {
            **facts_material,
            "derived_class": derived_class,
        },
        "class_cases": class_cases(catalog),
        "negative_catalog_cases": catalog_negatives,
        "negative_approval_cases": approval_negatives,
        "negative_chain_cases": chain_negatives,
        "negative_action_facts_cases": facts_negatives,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "catalog_negative_ids": [case["id"] for case in catalog_negatives],
            "approval_negative_ids": [case["id"] for case in approval_negatives],
            "chain_negative_ids": [case["id"] for case in chain_negatives],
            "action_facts_negative_ids": [
                case["id"] for case in facts_negatives
            ],
            "catalog_error_variant": INVALID_CATALOG,
            "chain_error_variant": INVALID_MANDATE,
            "action_facts_error_variant": INVALID_FACTS,
            "catalog_and_approval_are_distinct": True,
            "config_is_outside_catalog": True,
            "class_is_derived_not_caller_supplied": True,
            "historical_bytes_are_not_reinterpreted": True,
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

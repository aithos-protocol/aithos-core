#!/usr/bin/env python3
"""Generate the independent CB2 max_children versioning vector.

This generator is deliberately independent of the Rust implementation. It
computes version dispatch, max_children attenuation, JCS bytes, Ed25519
signatures, Gamma hashes, and Gamma signatures in Python.

The historical E+ file is an input only: its exact SHA-256 and its frozen
draft.1 max_children omission case are checked before any output is emitted.
Neither E+ nor its generator is rewritten.

Usage:
    python3 gen-cb2-max-children.py
    python3 gen-cb2-max-children.py --output /tmp/cb2.json
"""

import argparse
import copy
import hashlib
import json
from datetime import datetime
from pathlib import Path
from typing import Optional, Union

from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


VECTOR_DIR = Path(__file__).resolve().parent
DEFAULT_OUTPUT = VECTOR_DIR / "cb2-max-children-versioning.json"
EPLUS_PATH = VECTOR_DIR / "eplus-attenuation.json"
EPLUS_SHA256 = "9822d9da417487740b50efc1a760883addf8fffcaa0fa2008e029ab473d1db8c"

DRAFT1 = "1.0.0-draft.1"
DRAFT2 = "1.0.0-draft.2"
SUPPORTED_PROFILES = {DRAFT1, DRAFT2}
VALID = "valid"
INVALID = "InvalidMandate"

ROOT_SK_BYTES = bytes.fromhex("c0" * 32)
CONTENT_SK_BYTES = bytes.fromhex("d0" * 32)
AGENT_SK_BYTES = bytes.fromhex("a1" * 32)
HELPER_SK_BYTES = bytes.fromhex("b2" * 32)
WORKER_SK_BYTES = bytes.fromhex("c3" * 32)

CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
FIELD_P = 2**255 - 19
MANDATE_FIELDS = {
    "aithos-mandate-core",
    "id",
    "subject",
    "parent",
    "issued_by",
    "grantee",
    "perimeter",
    "constraints",
    "not_before",
    "not_after",
    "issued_at",
    "nonce",
    "signature",
}
GRANTEE_FIELDS = {"id", "label", "pubkey", "kex_pubkey"}
SIGNATURE_FIELDS = {"alg", "key", "value"}


def jcs(obj) -> str:
    """RFC 8785 for the ASCII-string/integer subset used by this vector."""
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def ulid(n: int) -> str:
    chars = []
    for _ in range(26):
        chars.append(CROCKFORD[n & 31])
        n >>= 5
    if n:
        raise ValueError("ULID fixture integer exceeds 130 bits")
    return "".join(reversed(chars))


def mandate_id(n: int) -> str:
    return "mandate_" + ulid(n)


def gamma_id(n: int) -> str:
    return "gamma_" + ulid(n)


def base58_encode(raw: bytes) -> str:
    zeros = len(raw) - len(raw.lstrip(b"\x00"))
    number = int.from_bytes(raw, "big")
    encoded = []
    while number:
        number, remainder = divmod(number, 58)
        encoded.append(BASE58[remainder])
    return "1" * zeros + "".join(reversed(encoded))


def base58_decode(text: str) -> bytes:
    number = 0
    for char in text:
        if char not in BASE58:
            raise AssertionError("invalid base58btc character")
        number = number * 58 + BASE58.index(char)
    body = (
        number.to_bytes((number.bit_length() + 7) // 8, "big")
        if number
        else b""
    )
    zeros = len(text) - len(text.lstrip("1"))
    return b"\x00" * zeros + body


def multibase_ed(pub: bytes) -> str:
    return "z" + base58_encode(b"\xed\x01" + pub)


def multibase_x(pub: bytes) -> str:
    return "z" + base58_encode(b"\xec\x01" + pub)


def decode_multibase(value: str, prefix: bytes) -> bytes:
    if not isinstance(value, str) or not value.startswith("z"):
        raise AssertionError("multibase value must use base58btc")
    raw = base58_decode(value[1:])
    if raw[:2] != prefix or len(raw) != 34:
        raise AssertionError("unexpected multicodec public key")
    return raw[2:]


def parse_zulu(value: str) -> datetime:
    if not isinstance(value, str) or not value.endswith("Z"):
        raise AssertionError("timestamp must be RFC 3339 Zulu")
    return datetime.fromisoformat(value[:-1] + "+00:00")


def ed25519_public_bytes(
    key: Union[Ed25519PrivateKey, Ed25519PublicKey]
) -> bytes:
    public = key.public_key() if isinstance(key, Ed25519PrivateKey) else key
    return public.public_bytes(Encoding.Raw, PublicFormat.Raw)


def ed25519_to_x25519_public(pub: bytes) -> bytes:
    """RFC 7748 Edwards-y to Montgomery-u map, independently in Python."""
    assert len(pub) == 32
    encoded_y = bytearray(pub)
    encoded_y[31] &= 0x7F
    y = int.from_bytes(encoded_y, "little")
    assert y < FIELD_P
    denominator = (1 - y) % FIELD_P
    assert denominator != 0
    u = ((1 + y) * pow(denominator, FIELD_P - 2, FIELD_P)) % FIELD_P
    return u.to_bytes(32, "little")


def sign_doc(doc: dict, signer: Ed25519PrivateKey) -> dict:
    signed = copy.deepcopy(doc)
    signed["signature"]["value"] = ""
    signature = signer.sign(jcs(signed).encode())
    signed["signature"]["value"] = signature.hex()
    return signed


def verify_doc_signature(doc: dict, verifier: Ed25519PublicKey) -> None:
    unsigned = copy.deepcopy(doc)
    signature_hex = unsigned["signature"]["value"]
    unsigned["signature"]["value"] = ""
    signature = bytes.fromhex(signature_hex)
    assert len(signature) == 64
    verifier.verify(signature, jcs(unsigned).encode())


ROOT_SK = Ed25519PrivateKey.from_private_bytes(ROOT_SK_BYTES)
CONTENT_SK = Ed25519PrivateKey.from_private_bytes(CONTENT_SK_BYTES)
AGENT_SK = Ed25519PrivateKey.from_private_bytes(AGENT_SK_BYTES)
HELPER_SK = Ed25519PrivateKey.from_private_bytes(HELPER_SK_BYTES)
WORKER_SK = Ed25519PrivateKey.from_private_bytes(WORKER_SK_BYTES)
DID = "did:aithos:" + multibase_ed(ed25519_public_bytes(ROOT_SK))

# Cross-check the pure-Python base58 and Edwards→Montgomery conversion against
# the already frozen E+ keys before using them for any new certificate.
assert multibase_ed(ed25519_public_bytes(AGENT_SK)) == (
    "z6Mks931aemXLmTDGrasbApX8araucPWxRhzP8iqL7XHhXeC"
)
assert multibase_x(ed25519_to_x25519_public(ed25519_public_bytes(AGENT_SK))) == (
    "z6LSq6Lq8NiaD7kQ9YxVHn3cT77QioifSAEyq7LrvmzbHRKn"
)
assert multibase_ed(ed25519_public_bytes(HELPER_SK)) == (
    "z6MkkBPYdMyzcYZ82316KGBobVXJL619wybD692WpZaPQSBg"
)
assert multibase_x(ed25519_to_x25519_public(ed25519_public_bytes(HELPER_SK))) == (
    "z6LSfyvnEKAwRRPKqSnKFiyFzT2ycbXUNqe5iUrfr9oPM8fe"
)

NB_LEGACY_ROOT = "2026-07-01T00:00:00Z"
NB_LEGACY_CHILD = "2026-07-02T00:00:00Z"
NB_ROOT = "2026-07-20T00:00:00Z"
NA_ROOT = "2026-08-20T00:00:00Z"
NB_CHILD = "2026-07-21T00:00:00Z"
NB_GRANDCHILD = "2026-07-22T00:00:00Z"
NA_CHILD = "2026-08-10T00:00:00Z"
VERIFY_AT = "2026-07-22T00:00:00Z"


def grantee_block(agent_id: str, label: str, key: Ed25519PrivateKey) -> dict:
    pub = ed25519_public_bytes(key)
    return {
        "id": agent_id,
        "label": label,
        "pubkey": multibase_ed(pub),
        "kex_pubkey": multibase_x(ed25519_to_x25519_public(pub)),
    }


def root_mandate(
    version: str,
    mid: str,
    nonce: str,
    not_before: str = NB_ROOT,
    max_children: int = 4,
    issue_depth: int = 2,
) -> dict:
    doc = {
        "aithos-mandate-core": version,
        "id": mid,
        "subject": DID,
        "parent": None,
        "issued_by": f"{DID}#root",
        "grantee": grantee_block("urn:aithos:agent:agent", "agent", AGENT_SK),
        "perimeter": ["act.x.gmail.*", f"issue#depth={issue_depth}"],
        "constraints": {"max_children": max_children},
        "not_before": not_before,
        "not_after": NA_ROOT,
        "issued_at": not_before,
        "nonce": nonce,
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    return sign_doc(doc, ROOT_SK)


def child_mandate(
    parent: dict,
    version: str,
    mid: str,
    nonce: str,
    constraints: dict,
    can_delegate: bool = False,
    not_before: str = NB_CHILD,
    signer: Ed25519PrivateKey = AGENT_SK,
    grantee_key: Ed25519PrivateKey = HELPER_SK,
    grantee_id: str = "urn:aithos:agent:helper",
    grantee_label: str = "helper",
) -> dict:
    perimeter = ["act.x.gmail.reply"]
    if can_delegate:
        perimeter.append("issue#depth=1")
    doc = {
        "aithos-mandate-core": version,
        "id": mid,
        "subject": DID,
        "parent": parent["id"],
        "issued_by": parent["grantee"]["pubkey"],
        "grantee": grantee_block(grantee_id, grantee_label, grantee_key),
        "perimeter": perimeter,
        "constraints": constraints,
        "not_before": not_before,
        "not_after": NA_CHILD,
        "issued_at": not_before,
        "nonce": nonce,
        "signature": {
            "alg": "ed25519",
            "key": parent["grantee"]["pubkey"],
            "value": "",
        },
    }
    return sign_doc(doc, signer)


def validate_max_children(constraints: dict) -> None:
    assert isinstance(constraints, dict)
    assert set(constraints).issubset({"max_children"})
    if "max_children" in constraints:
        value = constraints["max_children"]
        assert isinstance(value, int) and not isinstance(value, bool) and value >= 0
        assert value <= 0xFFFF_FFFF_FFFF_FFFF


def validate_mandate(doc: dict, parent: Optional[dict]) -> None:
    """Check that a fixture certificate is valid apart from link version policy."""
    assert set(doc) == MANDATE_FIELDS
    assert doc["aithos-mandate-core"] in SUPPORTED_PROFILES
    assert doc["id"].startswith("mandate_")
    assert len(doc["id"]) == len("mandate_") + 26
    assert all(c in CROCKFORD for c in doc["id"][len("mandate_"):])
    assert doc["subject"] == DID
    assert isinstance(doc["nonce"], str) and doc["nonce"]
    assert isinstance(doc["perimeter"], list)
    assert all(isinstance(entry, str) for entry in doc["perimeter"])
    assert set(doc["grantee"]) == GRANTEE_FIELDS
    assert set(doc["signature"]) == SIGNATURE_FIELDS
    assert doc["signature"]["alg"] == "ed25519"
    assert parse_zulu(doc["not_before"]) <= parse_zulu(doc["not_after"])
    parse_zulu(doc["issued_at"])
    validate_max_children(doc["constraints"])

    grantee_pub = decode_multibase(doc["grantee"]["pubkey"], b"\xed\x01")
    expected_kex = ed25519_to_x25519_public(grantee_pub)
    assert decode_multibase(doc["grantee"]["kex_pubkey"], b"\xec\x01") == expected_kex

    if parent is None:
        assert doc["parent"] is None
        assert doc["issued_by"] == f"{DID}#root"
        assert doc["signature"]["key"] == "#root"
        root_pub = decode_multibase(DID.removeprefix("did:aithos:"), b"\xed\x01")
        verify_doc_signature(doc, Ed25519PublicKey.from_public_bytes(root_pub))
    else:
        assert doc["parent"] == parent["id"]
        assert doc["subject"] == parent["subject"]
        assert doc["issued_by"] == parent["grantee"]["pubkey"]
        assert doc["signature"]["key"] == parent["grantee"]["pubkey"]
        assert doc["grantee"]["pubkey"] != doc["issued_by"]
        assert parse_zulu(parent["not_before"]) <= parse_zulu(doc["not_before"])
        assert parse_zulu(doc["not_after"]) <= parse_zulu(parent["not_after"])
        parent_pub = decode_multibase(parent["grantee"]["pubkey"], b"\xed\x01")
        verify_doc_signature(doc, Ed25519PublicKey.from_public_bytes(parent_pub))


def link_oracle(
    parent: dict,
    child: dict,
    parent_parent: Optional[dict] = None,
) -> tuple[str, str]:
    """Version dispatch precedes the independent max_children attenuation rule."""
    validate_mandate(parent, parent_parent)
    validate_mandate(child, parent)
    parent_version = parent["aithos-mandate-core"]
    child_version = child["aithos-mandate-core"]
    if parent_version != child_version:
        return INVALID, "version"

    parent_cap = parent["constraints"].get("max_children")
    child_cap = child["constraints"].get("max_children")
    if parent_cap is not None:
        if child_cap is None:
            if parent_version == DRAFT2:
                return INVALID, "attenuation"
        elif child_cap > parent_cap:
            return INVALID, "attenuation"
    return VALID, "accepted"


def certificate_record(doc: dict) -> dict:
    canonical = jcs(doc)
    assert jcs(json.loads(canonical)) == canonical
    return {
        "jcs": canonical,
        "sha256": sha256_hex(canonical.encode()),
        "signature_hex": doc["signature"]["value"],
    }


def historical_eplus_reference() -> dict:
    raw = EPLUS_PATH.read_bytes()
    digest = sha256_hex(raw)
    assert digest == EPLUS_SHA256, (
        "historical E+ bytes changed: "
        f"expected {EPLUS_SHA256}, got {digest}"
    )
    eplus = json.loads(raw)
    matches = [
        case
        for case in eplus["matrix"]
        if case.get("family") == "max_children"
        and case.get("case") == "drop tolerated — per-level width"
    ]
    assert len(matches) == 1
    frozen = matches[0]
    assert frozen["parent"] == {"max_children": 4}
    assert frozen["child"] == {}
    assert frozen["expected"] == VALID
    return {
        "file": "eplus-attenuation.json",
        "sha256": digest,
        "selector": {
            "family": frozen["family"],
            "case": frozen["case"],
        },
        "parent": frozen["parent"],
        "child": frozen["child"],
        "expected": frozen["expected"],
    }


def entry_hash(entry: dict) -> str:
    return "sha256:" + sha256_hex(jcs(entry).encode())


def owner_grant(eid: str, prev: str, at: str, target: str) -> dict:
    entry = {
        "v": 1,
        "id": eid,
        "prev": prev,
        "at": at,
        "kind": "grant",
        "target": target,
        "payload": {},
        "signature": {"alg": "ed25519", "key": "#content", "value": ""},
    }
    return sign_doc(entry, CONTENT_SK)


def delegated_grant(
    eid: str,
    prev: str,
    at: str,
    target: str,
    via: list[str],
    signer: Ed25519PrivateKey = AGENT_SK,
) -> dict:
    entry = {
        "v": 1,
        "id": eid,
        "prev": prev,
        "at": at,
        "kind": "grant",
        "target": target,
        "authorized_by": via[-1],
        "authorized_via": via,
        "payload": {},
        "signature": {
            "alg": "ed25519",
            "key": multibase_ed(ed25519_public_bytes(signer)),
            "value": "",
        },
    }
    return sign_doc(entry, signer)


def direct_children_tally(entries: list[dict]) -> dict[str, int]:
    """Count grants by the exact minting mandate, never by authorized_via."""
    tallies: dict[str, int] = {}
    for entry in entries:
        if entry.get("kind") != "grant":
            continue
        minting_mandate = entry.get("authorized_by")
        if minting_mandate is not None:
            tallies[minting_mandate] = tallies.get(minting_mandate, 0) + 1
    return tallies


def validate_grant(
    entry: dict,
    verifier: Ed25519PublicKey,
    expected_prev: str,
    expected_target: str,
    delegated: bool,
    expected_via: Optional[list[str]] = None,
) -> None:
    assert entry["v"] == 1
    assert entry["id"].startswith("gamma_")
    assert entry["prev"] == expected_prev
    parse_zulu(entry["at"])
    assert entry["kind"] == "grant"
    assert entry["target"] == expected_target
    assert entry["payload"] == {}
    assert entry["signature"]["alg"] == "ed25519"
    if delegated:
        assert entry["authorized_via"] == expected_via
        assert entry["authorized_by"] == entry["authorized_via"][-1]
        assert entry["signature"]["key"] == multibase_ed(ed25519_public_bytes(verifier))
    else:
        assert "authorized_by" not in entry
        assert "authorized_via" not in entry
        assert entry["signature"]["key"] == "#content"
    verify_doc_signature(entry, verifier)


def build_vector() -> dict:
    legacy_parent = root_mandate(
        DRAFT1,
        mandate_id(200),
        "10" * 16,
        not_before=NB_LEGACY_ROOT,
    )
    current_parent = root_mandate(DRAFT2, mandate_id(300), "20" * 16)
    direct_parent = root_mandate(
        DRAFT2,
        mandate_id(500),
        "30" * 16,
        max_children=3,
        issue_depth=2,
    )
    direct_child = child_mandate(
        direct_parent,
        DRAFT2,
        mandate_id(501),
        "31" * 16,
        {"max_children": 3},
        can_delegate=True,
    )
    direct_grandchildren = [
        child_mandate(
            direct_child,
            DRAFT2,
            mandate_id(502 + index),
            f"{32 + index}" * 16,
            {"max_children": 3},
            not_before=NB_GRANDCHILD,
            signer=HELPER_SK,
            grantee_key=WORKER_SK,
            grantee_id="urn:aithos:agent:worker",
            grantee_label="worker",
        )
        for index in range(3)
    ]

    docs = {
        "draft1_parent": legacy_parent,
        "draft1_omission_leaf": child_mandate(
            legacy_parent,
            DRAFT1,
            mandate_id(201),
            "11" * 16,
            {},
            not_before=NB_LEGACY_CHILD,
        ),
        "draft2_parent": current_parent,
        "draft2_equal_leaf": child_mandate(
            current_parent, DRAFT2, mandate_id(301), "21" * 16,
            {"max_children": 4},
        ),
        "draft2_reduced_leaf": child_mandate(
            current_parent, DRAFT2, mandate_id(302), "22" * 16,
            {"max_children": 2},
        ),
        "draft2_wider_leaf": child_mandate(
            current_parent, DRAFT2, mandate_id(303), "23" * 16,
            {"max_children": 5},
        ),
        "draft2_omission_delegating": child_mandate(
            current_parent, DRAFT2, mandate_id(304), "24" * 16, {},
            can_delegate=True,
        ),
        "draft2_omission_leaf": child_mandate(
            current_parent, DRAFT2, mandate_id(305), "25" * 16, {}
        ),
        "draft2_child_under_draft1": child_mandate(
            legacy_parent, DRAFT2, mandate_id(306), "26" * 16,
            {"max_children": 4},
        ),
        "draft1_child_under_draft2": child_mandate(
            current_parent, DRAFT1, mandate_id(307), "27" * 16,
            {"max_children": 4},
        ),
        "draft2_direct_parent": direct_parent,
        "draft2_direct_child": direct_child,
        **{
            f"draft2_direct_grandchild_{index + 1}": grandchild
            for index, grandchild in enumerate(direct_grandchildren)
        },
    }
    parents = {
        "draft1_omission_leaf": "draft1_parent",
        "draft2_equal_leaf": "draft2_parent",
        "draft2_reduced_leaf": "draft2_parent",
        "draft2_wider_leaf": "draft2_parent",
        "draft2_omission_delegating": "draft2_parent",
        "draft2_omission_leaf": "draft2_parent",
        "draft2_child_under_draft1": "draft1_parent",
        "draft1_child_under_draft2": "draft2_parent",
        "draft2_direct_child": "draft2_direct_parent",
        "draft2_direct_grandchild_1": "draft2_direct_child",
        "draft2_direct_grandchild_2": "draft2_direct_child",
        "draft2_direct_grandchild_3": "draft2_direct_child",
    }

    validate_mandate(docs["draft1_parent"], None)
    validate_mandate(docs["draft2_parent"], None)
    validate_mandate(docs["draft2_direct_parent"], None)
    for child_name, parent_name in parents.items():
        validate_mandate(docs[child_name], docs[parent_name])

    case_specs = [
        (
            "draft1_omission_historical",
            "draft1_parent",
            "draft1_omission_leaf",
            VALID,
            "accepted",
        ),
        (
            "draft2_equal",
            "draft2_parent",
            "draft2_equal_leaf",
            VALID,
            "accepted",
        ),
        (
            "draft2_reduced",
            "draft2_parent",
            "draft2_reduced_leaf",
            VALID,
            "accepted",
        ),
        (
            "draft2_wider",
            "draft2_parent",
            "draft2_wider_leaf",
            INVALID,
            "attenuation",
        ),
        (
            "draft2_omission_delegating",
            "draft2_parent",
            "draft2_omission_delegating",
            INVALID,
            "attenuation",
        ),
        (
            "draft2_omission_leaf",
            "draft2_parent",
            "draft2_omission_leaf",
            INVALID,
            "attenuation",
        ),
        (
            "mixed_draft1_to_draft2",
            "draft1_parent",
            "draft2_child_under_draft1",
            INVALID,
            "version",
        ),
        (
            "mixed_draft2_to_draft1",
            "draft2_parent",
            "draft1_child_under_draft2",
            INVALID,
            "version",
        ),
    ]
    cases = []
    for case_id, parent_name, child_name, expected, expected_stage in case_specs:
        verdict, stage = link_oracle(docs[parent_name], docs[child_name])
        assert (verdict, stage) == (expected, expected_stage)
        cases.append(
            {
                "id": case_id,
                "parent": parent_name,
                "child": child_name,
                "expected": verdict,
                "decision_stage": stage,
            }
        )

    owner_entry = owner_grant(
        gamma_id(400),
        "",
        "2026-07-20T00:00:01Z",
        current_parent["id"],
    )
    owner_hash = entry_hash(owner_entry)
    delegated_entry = delegated_grant(
        gamma_id(401),
        owner_hash,
        "2026-07-21T00:00:01Z",
        docs["draft2_reduced_leaf"]["id"],
        [current_parent["id"]],
    )
    delegated_hash = entry_hash(delegated_entry)
    validate_grant(
        owner_entry,
        CONTENT_SK.public_key(),
        "",
        current_parent["id"],
        delegated=False,
    )
    validate_grant(
        delegated_entry,
        AGENT_SK.public_key(),
        owner_hash,
        docs["draft2_reduced_leaf"]["id"],
        delegated=True,
        expected_via=[current_parent["id"]],
    )

    direct_entries = [
        owner_grant(
            gamma_id(500),
            "",
            "2026-07-20T00:10:00Z",
            direct_parent["id"],
        )
    ]
    direct_entries.append(
        delegated_grant(
            gamma_id(501),
            entry_hash(direct_entries[-1]),
            "2026-07-21T00:10:00Z",
            direct_child["id"],
            [direct_parent["id"]],
        )
    )
    for index, grandchild in enumerate(direct_grandchildren):
        direct_entries.append(
            delegated_grant(
                gamma_id(502 + index),
                entry_hash(direct_entries[-1]),
                f"2026-07-22T00:1{index + 1}:00Z",
                grandchild["id"],
                [direct_parent["id"], direct_child["id"]],
                signer=HELPER_SK,
            )
        )

    validate_grant(
        direct_entries[0],
        CONTENT_SK.public_key(),
        "",
        direct_parent["id"],
        delegated=False,
    )
    validate_grant(
        direct_entries[1],
        AGENT_SK.public_key(),
        entry_hash(direct_entries[0]),
        direct_child["id"],
        delegated=True,
        expected_via=[direct_parent["id"]],
    )
    for index, grandchild in enumerate(direct_grandchildren):
        validate_grant(
            direct_entries[index + 2],
            HELPER_SK.public_key(),
            entry_hash(direct_entries[index + 1]),
            grandchild["id"],
            delegated=True,
            expected_via=[direct_parent["id"], direct_child["id"]],
        )

    assert link_oracle(direct_parent, direct_child) == (VALID, "accepted")
    for grandchild in direct_grandchildren:
        assert link_oracle(
            direct_child,
            grandchild,
            parent_parent=direct_parent,
        ) == (VALID, "accepted")
    assert direct_parent["perimeter"][-1] == "issue#depth=2"
    assert direct_child["perimeter"][-1] == "issue#depth=1"
    assert all(
        not any(entry.startswith("issue") for entry in grandchild["perimeter"])
        for grandchild in direct_grandchildren
    )

    direct_tallies = direct_children_tally(direct_entries)
    assert direct_tallies == {
        direct_parent["id"]: 1,
        direct_child["id"]: 3,
    }
    assert (
        direct_tallies[direct_parent["id"]]
        <= direct_parent["constraints"]["max_children"]
    )
    assert (
        direct_tallies[direct_child["id"]]
        <= direct_child["constraints"]["max_children"]
    )
    parent_tally_progress = [
        direct_children_tally(direct_entries[: 3 + index]).get(
            direct_parent["id"], 0
        )
        for index in range(3)
    ]
    child_tally_progress = [
        direct_children_tally(direct_entries[: 3 + index]).get(
            direct_child["id"], 0
        )
        for index in range(3)
    ]
    assert parent_tally_progress == [1, 1, 1]
    assert child_tally_progress == [1, 2, 3]

    assert link_oracle(
        docs["draft1_parent"], docs["draft1_omission_leaf"]
    ) == (VALID, "accepted")
    assert link_oracle(
        docs["draft2_parent"], docs["draft2_reduced_leaf"]
    ) == (VALID, "accepted")
    assert docs["draft1_parent"]["id"] != docs["draft2_parent"]["id"]
    assert docs["draft1_omission_leaf"]["id"] != docs["draft2_reduced_leaf"]["id"]
    assert (
        docs["draft1_parent"]["grantee"]["pubkey"]
        == docs["draft2_parent"]["grantee"]["pubkey"]
    )
    assert (
        docs["draft1_omission_leaf"]["grantee"]["pubkey"]
        == docs["draft2_reduced_leaf"]["grantee"]["pubkey"]
    )

    return {
        "vector": "CB2-MC1",
        "description": (
            "Versioned max_children attenuation and migration by complete "
            "reissuance (spec 04.1.1, 04.4, 05.3). Historical draft.1 bytes "
            "and omission semantics remain frozen; draft.2 makes max_children "
            "non-droppable, including at a chain leaf; mixed-version links are "
            "rejected before attenuation; direct-child grant tallies do not "
            "charge grandchildren to their grandparent. Certificates, verdicts, "
            "JCS, Ed25519 signatures, and existing Gamma grant records are "
            "generated independently in Python."
        ),
        "historical_eplus": historical_eplus_reference(),
        "root_sk_hex": ROOT_SK_BYTES.hex(),
        "content_sk_hex": CONTENT_SK_BYTES.hex(),
        "agent_sk_hex": AGENT_SK_BYTES.hex(),
        "helper_sk_hex": HELPER_SK_BYTES.hex(),
        "worker_sk_hex": WORKER_SK_BYTES.hex(),
        "did": DID,
        "verify_at": VERIFY_AT,
        "certificates": {
            name: certificate_record(doc)
            for name, doc in docs.items()
        },
        "cases": cases,
        "migration": {
            "legacy_chain": [
                "draft1_parent",
                "draft1_omission_leaf",
            ],
            "reissued_chain": [
                "draft2_parent",
                "draft2_reduced_leaf",
            ],
            "same_authority_keys": True,
            "fresh_certificate_ids": True,
            "grant_entries_jcs": [
                jcs(owner_entry),
                jcs(delegated_entry),
            ],
            "grant_entry_hashes": [
                owner_hash,
                delegated_hash,
            ],
            "gamma_head": delegated_hash,
            "expected": {
                "legacy_chain": VALID,
                "reissued_chain": VALID,
                "all_reissued_certificates_are_draft2": True,
                "historical_certificates_rewritten": False,
            },
        },
        "direct_children_only": {
            "parent_chain": [
                "draft2_direct_parent",
            ],
            "child_chain": [
                "draft2_direct_parent",
                "draft2_direct_child",
            ],
            "grandchild_chains": [
                [
                    "draft2_direct_parent",
                    "draft2_direct_child",
                    f"draft2_direct_grandchild_{index + 1}",
                ]
                for index in range(3)
            ],
            "grant_entries_jcs": [
                jcs(entry)
                for entry in direct_entries
            ],
            "grant_entry_hashes": [
                entry_hash(entry)
                for entry in direct_entries
            ],
            "gamma_head": entry_hash(direct_entries[-1]),
            "direct_children_tallies": direct_tallies,
            "grandparent_tally_after_each_grandchild": parent_tally_progress,
            "child_tally_after_each_grandchild": child_tally_progress,
            "expected": {
                "parent_max_children": 3,
                "child_max_children": 3,
                "direct_children_of_parent": 1,
                "direct_children_of_child": 3,
                "grandchildren_counted_against_parent": 0,
                "all_grants_within_declared_caps": True,
            },
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="output JSON path (default: vectors/cb2-max-children-versioning.json)",
    )
    args = parser.parse_args()

    vector = build_vector()
    rendered = json.dumps(vector, indent=2, ensure_ascii=False) + "\n"
    args.output.write_text(rendered, encoding="utf-8")
    print(
        f"wrote {args.output} — {len(vector['cases'])} signed matrix cases, "
        f"1 signed direct-child scenario, 7 signed Gamma grants, "
        f"sha256 {sha256_hex(rendered.encode())}"
    )


if __name__ == "__main__":
    main()

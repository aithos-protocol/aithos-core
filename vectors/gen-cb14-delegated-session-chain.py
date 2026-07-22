#!/usr/bin/env python3
"""Independent non-root delegated-session oracle for the G4/P7 Core gate.

Python cryptography signs a DID, a root mandate, a strictly attenuated child,
SC1, the native leaf proof and the session proof.  The historical SC1 vector
and wire are not read or modified.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb14-delegated-session-chain.json"
HELPERS_PATH = HERE / "gen-cb2-session-proof.py"
SPEC = importlib.util.spec_from_file_location("cb2_session_oracle", HELPERS_PATH)
assert SPEC and SPEC.loader
H = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(H)

ROOT_SEED = bytes.fromhex("61" * 32)
DELEGATE_SEED = bytes.fromhex("62" * 32)
GATEWAY_SEED = bytes.fromhex("63" * 32)
SESSION_SEED = bytes.fromhex("64" * 32)
SUCCESSION_SEED = bytes.fromhex("65" * 32)
STRANGER_SEED = bytes.fromhex("66" * 32)

ROOT = Ed25519PrivateKey.from_private_bytes(ROOT_SEED)
DELEGATE = Ed25519PrivateKey.from_private_bytes(DELEGATE_SEED)
GATEWAY = Ed25519PrivateKey.from_private_bytes(GATEWAY_SEED)
SESSION = Ed25519PrivateKey.from_private_bytes(SESSION_SEED)
SUCCESSION = Ed25519PrivateKey.from_private_bytes(SUCCESSION_SEED)
STRANGER = Ed25519PrivateKey.from_private_bytes(STRANGER_SEED)

MANDATE_PROFILE = "1.0.0-draft.2"
DID_PROFILE = "1.0.0-draft.1"
SC1_PROFILE = "1.0.0-draft.1"
OPERATION_PROFILE = "1.0.0-draft.1"
FACTS_PROFILE = "1.0.0-draft.1"
SESSION_PROOF_PROFILE = "1.0.0-draft.1"
NATIVE_DOMAIN = b"aithos-core/cb2/native-leaf-proof\x00"
AT = "2026-07-22T12:00:00Z"


def public(key: Ed25519PrivateKey) -> bytes:
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def ed(key: Ed25519PrivateKey) -> str:
    return H.multibase_ed(public(key))


def kex(key: Ed25519PrivateKey) -> str:
    return H.multibase_x(H.ed25519_to_x25519_public(public(key)))


def sign_document(value: dict[str, Any], key: Ed25519PrivateKey) -> None:
    unsigned = copy.deepcopy(value)
    unsigned["signature"]["value"] = ""
    value["signature"]["value"] = key.sign(H.jcs(unsigned).encode()).hex()


def signed_did() -> dict[str, Any]:
    root = ed(ROOT)
    document = {
        "aithos-did-core": DID_PROFILE,
        "bundle": ["file://cb14-independent-vector"],
        "id": f"did:aithos:{root}",
        "keys": {
            "content": ed(DELEGATE),
            "kex": kex(ROOT),
            "root": root,
            "succession": ed(SUCCESSION),
        },
        "revocations": "gamma/gamma.jsonl",
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    sign_document(document, ROOT)
    return document


def grantee(identifier: str, label: str, key: Ed25519PrivateKey) -> dict[str, str]:
    return {
        "id": f"urn:aithos:agent:{identifier}",
        "label": label,
        "pubkey": ed(key),
        "kex_pubkey": kex(key),
    }


def signed_chain(did: dict[str, Any]) -> list[dict[str, Any]]:
    subject = did["id"]
    parent = {
        "aithos-mandate-core": MANDATE_PROFILE,
        "id": "mandate_01J00000000000000000000061",
        "subject": subject,
        "parent": None,
        "issued_by": subject + "#root",
        "grantee": grantee("delegate", "delegate", DELEGATE),
        "perimeter": ["act.x.github.*", "issue#depth=1"],
        "constraints": {"max_sessions": 3},
        "not_before": "2026-07-22T10:00:00Z",
        "not_after": "2026-07-23T00:00:00Z",
        "issued_at": "2026-07-22T09:59:00Z",
        "nonce": "61" * 16,
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    sign_document(parent, ROOT)
    leaf = {
        "aithos-mandate-core": MANDATE_PROFILE,
        "id": "mandate_01J00000000000000000000062",
        "subject": subject,
        "parent": parent["id"],
        "issued_by": ed(DELEGATE),
        "grantee": grantee("gateway-session", "gateway session", GATEWAY),
        "perimeter": ["act.x.github.get_issue"],
        "constraints": {"max_sessions": 3, "session_bind": ed(SESSION)},
        "not_before": "2026-07-22T11:30:00Z",
        "not_after": "2026-07-22T20:00:00Z",
        "issued_at": "2026-07-22T11:29:00Z",
        "nonce": "62" * 16,
        "signature": {"alg": "ed25519", "key": ed(DELEGATE), "value": ""},
    }
    sign_document(leaf, DELEGATE)
    return [parent, leaf]


def signed_bundle() -> dict[str, Any]:
    did = signed_did()
    chain = signed_chain(did)
    leaf = chain[-1]
    certificate = {
        "aithos-session-core": SC1_PROFILE,
        "subject": leaf["subject"],
        "mandate_id": leaf["id"],
        "key": ed(SESSION),
        "not_before": "2026-07-22T11:30:00Z",
        "not_after": "2026-07-22T20:00:00Z",
        "signature": {"alg": "ed25519", "key": ed(GATEWAY), "value": ""},
    }
    sign_document(certificate, GATEWAY)
    certificate_digest = H.sha256_text(H.jcs(certificate).encode())
    projection = {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": "op_01K00000000000000000000061",
        "subject": leaf["subject"],
        "at": AT,
        "history_heads": ["sha256:" + "61" * 32],
        "authority": {
            "actor": "grantee",
            "key": ed(GATEWAY),
            "authorized_by": leaf["id"],
            "authorized_via": [{
                "id": leaf["id"],
                "certificate_digest": H.sha256_text(H.jcs(leaf).encode()),
            }],
            "session": {
                "key": ed(SESSION),
                "certificate_digest": certificate_digest,
            },
        },
        "operation": {
            "kind": "action",
            "facts_ref": {
                "aithos-operation-facts-core": FACTS_PROFILE,
                "digest": "sha256:" + "62" * 32,
            },
        },
    }
    operation_ref = {
        "aithos-operation-core": OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": H.commitment(
            "aithos-core/v1/operation-commitment", H.jcs(projection).encode()
        ),
    }
    native = {
        "key": ed(GATEWAY),
        "sig": GATEWAY.sign(NATIVE_DOMAIN + H.jcs(operation_ref).encode()).hex(),
    }
    session_proof = {
        "aithos-session-proof-core": SESSION_PROOF_PROFILE,
        "operation_ref": copy.deepcopy(operation_ref),
        "key": ed(SESSION),
        "sig": "",
    }
    unsigned_proof = {key: value for key, value in session_proof.items() if key != "sig"}
    session_proof["sig"] = SESSION.sign(H.jcs(unsigned_proof).encode()).hex()
    return {
        "at": AT,
        "did": did,
        "chain": chain,
        "mandate": copy.deepcopy(leaf),
        "revocations": [],
        "certificate": certificate,
        "operation_projection": projection,
        "operation_ref": operation_ref,
        "native_leaf_proof": native,
        "session_proof": session_proof,
    }


def alternative_leaf(valid: dict[str, Any]) -> dict[str, Any]:
    leaf = copy.deepcopy(valid["chain"][-1])
    leaf["id"] = "mandate_01J00000000000000000000063"
    leaf["nonce"] = "63" * 16
    leaf["signature"]["value"] = ""
    sign_document(leaf, DELEGATE)
    return leaf


def negative_cases(valid: dict[str, Any]) -> list[dict[str, Any]]:
    truncated = copy.deepcopy(valid)
    truncated["chain"] = [copy.deepcopy(valid["chain"][-1])]

    revoked = copy.deepcopy(valid)
    revoked["revocations"] = [{
        "mandate_id": valid["chain"][0]["id"],
        "revoked_at": "2026-07-22T11:59:59Z",
    }]

    substituted = copy.deepcopy(valid)
    substituted["chain"][-1] = alternative_leaf(valid)

    crossed = copy.deepcopy(valid)
    crossed["session_proof"]["operation_ref"]["occurrence"] = (
        "op_01K00000000000000000000066"
    )
    unsigned = {
        key: value for key, value in crossed["session_proof"].items() if key != "sig"
    }
    crossed["session_proof"]["sig"] = SESSION.sign(H.jcs(unsigned).encode()).hex()

    wrong_time = copy.deepcopy(valid)
    wrong_time["at"] = "2026-07-22T12:00:01Z"

    return [
        {"id": "truncated-chain", "candidate": truncated},
        {"id": "revoked-parent", "candidate": revoked},
        {"id": "substituted-leaf", "candidate": substituted},
        {"id": "crossed-session-proof", "candidate": crossed},
        {"id": "verification-time-mismatch", "candidate": wrong_time},
    ]


def build_vector() -> dict[str, Any]:
    positive = signed_bundle()
    return {
        "vector": "CB14-DELEGATED-NON-ROOT-SESSION-1",
        "description": (
            "Independent Python Ed25519/JCS fixture for a root mandate, a "
            "strictly attenuated non-root session leaf, unchanged SC1/W1.1 "
            "and double possession."
        ),
        "wire_change": False,
        "positive": positive,
        "negative_cases": negative_cases(positive),
        "inventory": {
            "negative_ids": [
                "truncated-chain",
                "revoked-parent",
                "substituted-leaf",
                "crossed-session-proof",
                "verification-time-mismatch",
            ],
            "historical_verify_session_must_remain_unchanged": True,
            "required_error_variant": "InvalidSession",
        },
        "deterministic_test_seed_sha256": {
            "root": hashlib.sha256(ROOT_SEED).hexdigest(),
            "delegate": hashlib.sha256(DELEGATE_SEED).hexdigest(),
            "gateway": hashlib.sha256(GATEWAY_SEED).hexdigest(),
            "session": hashlib.sha256(SESSION_SEED).hexdigest(),
        },
    }


def encoded(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    output = encoded(build_vector())
    if args.check:
        if args.output.read_bytes() != output:
            raise SystemExit(f"drift: {args.output}")
        print(f"ok {args.output.name} sha256={hashlib.sha256(output).hexdigest()}")
        return
    args.output.write_bytes(output)
    print(f"wrote {args.output} sha256={hashlib.sha256(output).hexdigest()}")


if __name__ == "__main__":
    main()

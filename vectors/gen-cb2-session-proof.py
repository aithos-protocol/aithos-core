#!/usr/bin/env python3
"""Independent CB2 oracle for SC1 certificates and session proofs.

Python ``cryptography`` performs every Ed25519 operation.  The generator builds a
real signed draft2 mandate carrying ``session_bind``, a complete signed SC1
certificate, a session-bound W1 projection/reference, the exact closed session
proof, and negative double-possession cases.  The small native-leaf proof fixture
is explicitly test-only: its bytes are not promoted as a protocol carrier.
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Callable, NoReturn, Optional

from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives.asymmetric.ed25519 import (
    Ed25519PrivateKey,
    Ed25519PublicKey,
)
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-session-proof.json"

SC1_KEY = "aithos-session-core"
SC1_PROFILE = "1.0.0-draft.1"
PROOF_KEY = "aithos-session-proof-core"
PROOF_PROFILE = "1.0.0-draft.1"
OPERATION_KEY = "aithos-operation-core"
OPERATION_PROFILE = "1.0.0-draft.1"
MANDATE_KEY = "aithos-mandate-core"
MANDATE_PROFILE = "1.0.0-draft.2"
FACTS_KEY = "aithos-operation-facts-core"
FACTS_PROFILE = "1.0.0-draft.1"
INVALID_SESSION = "InvalidSession"

CERTIFICATE_KEYS = {
    SC1_KEY,
    "subject",
    "mandate_id",
    "key",
    "not_before",
    "not_after",
    "signature",
}
SIGNATURE_KEYS = {"alg", "key", "value"}
PROOF_KEYS = {PROOF_KEY, "operation_ref", "key", "sig"}
REFERENCE_KEYS = {OPERATION_KEY, "occurrence", "commitment"}
SESSION_AUTHORITY_KEYS = {
    "actor",
    "key",
    "authorized_by",
    "authorized_via",
    "session",
}
SESSION_FACT_KEYS = {"key", "certificate_digest"}

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SIGNATURE_RE = re.compile(r"^[0-9a-f]{128}$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")
OCCURRENCE_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
AT_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")

BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
FIELD_P = 2**255 - 19

ROOT_SEED = bytes.fromhex("51" * 32)
LEAF_SEED = bytes.fromhex("52" * 32)
SESSION_SEED = bytes.fromhex("53" * 32)
STRANGER_SEED = bytes.fromhex("54" * 32)
NATIVE_LEAF_DOMAIN = b"aithos-core/cb2/native-leaf-proof\x00"

HISTORICAL_FILES = (
    "e1-mandate.json",
    "cb2-operation-projection.json",
    "cb2-operation-facts-mutation.json",
)


class SessionError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_SESSION


def reject(detail: str) -> NoReturn:
    raise SessionError(detail)


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


def commitment(domain: str, payload: bytes) -> str:
    return sha256_text(domain.encode("ascii") + b"\x00" + payload)


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


def multibase_ed(public: bytes) -> str:
    return "z" + b58(b"\xed\x01" + public)


def multibase_x(public: bytes) -> str:
    return "z" + b58(b"\xec\x01" + public)


def ed25519_to_x25519_public(public: bytes) -> bytes:
    encoded_y = bytearray(public)
    encoded_y[31] &= 0x7F
    y = int.from_bytes(encoded_y, "little")
    if y >= FIELD_P:
        raise ValueError("non-canonical Ed25519 public key")
    denominator = (1 - y) % FIELD_P
    if denominator == 0:
        raise ValueError("Ed25519 public key has no Montgomery image")
    u = ((1 + y) * pow(denominator, FIELD_P - 2, FIELD_P)) % FIELD_P
    return u.to_bytes(32, "little")


def parse_at(value: Any, label: str) -> datetime:
    if not isinstance(value, str) or not AT_RE.fullmatch(value):
        reject(f"{label} is not canonical RFC3339 Z")
    try:
        return datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject(f"{label} is not a calendar instant")


def require_exact(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        reject(f"{label} has a non-exact member set")
    if any(item is None for item in value.values()):
        reject(f"{label} contains null")
    return value


def verify_signature(public: Ed25519PublicKey, message: bytes, signature: Any) -> None:
    if not isinstance(signature, str) or not SIGNATURE_RE.fullmatch(signature):
        reject("malformed Ed25519 signature")
    try:
        public.verify(bytes.fromhex(signature), message)
    except InvalidSignature:
        reject("Ed25519 signature does not verify")


def sign_certificate(certificate: dict[str, Any], key: Ed25519PrivateKey) -> None:
    unsigned = clone(certificate)
    unsigned["signature"]["value"] = ""
    certificate["signature"]["value"] = key.sign(jcs(unsigned).encode()).hex()


def certificate_preimage(certificate: dict[str, Any]) -> str:
    unsigned = clone(certificate)
    unsigned["signature"]["value"] = ""
    return jcs(unsigned)


def sign_without_member(
    value: dict[str, Any],
    member: str,
    key: Ed25519PrivateKey,
) -> str:
    unsigned = {name: item for name, item in value.items() if name != member}
    return key.sign(jcs(unsigned).encode()).hex()


def mandate_fixture(
    root: Ed25519PrivateKey,
    leaf: Ed25519PrivateKey,
    session_key_text: str,
) -> dict[str, Any]:
    root_text = multibase_ed(public_bytes(root))
    leaf_public = public_bytes(leaf)
    leaf_text = multibase_ed(leaf_public)
    did = f"did:aithos:{root_text}"
    mandate = {
        MANDATE_KEY: MANDATE_PROFILE,
        "id": "mandate_01J00000000000000000000051",
        "subject": did,
        "parent": None,
        "issued_by": did + "#root",
        "grantee": {
            "id": "urn:aithos:agent:session-leaf",
            "label": "session-leaf",
            "pubkey": leaf_text,
            "kex_pubkey": multibase_x(ed25519_to_x25519_public(leaf_public)),
        },
        "perimeter": ["write.circle#dir=00000000-0000-4000-8000-000000000001"],
        "constraints": {"session_bind": session_key_text},
        "not_before": "2026-07-18T10:00:00Z",
        "not_after": "2026-07-18T14:00:00Z",
        "issued_at": "2026-07-18T09:59:00Z",
        "nonce": "51515151515151515151515151515151",
        "signature": {"alg": "ed25519", "key": "#root", "value": ""},
    }
    sign_certificate(mandate, root)
    return mandate


def validate_certificate(
    certificate: Any,
    *,
    mandate: dict[str, Any],
    operation_at: str,
    expected_session_key: str,
) -> None:
    cert = require_exact(certificate, CERTIFICATE_KEYS, "SC1 certificate")
    if cert[SC1_KEY] != SC1_PROFILE:
        reject("unknown SC1 profile")
    if cert["subject"] != mandate["subject"]:
        reject("SC1 subject mismatch")
    if cert["mandate_id"] != mandate["id"]:
        reject("SC1 leaf mandate mismatch")
    if cert["key"] != expected_session_key:
        reject("SC1 session key mismatch")
    signature = require_exact(cert["signature"], SIGNATURE_KEYS, "SC1 signature")
    if signature["alg"] != "ed25519":
        reject("SC1 signature algorithm mismatch")
    if signature["key"] != mandate["grantee"]["pubkey"]:
        reject("SC1 signer key mismatch")
    leaf_public = Ed25519PublicKey.from_public_bytes(public_bytes(LEAF_KEY))
    verify_signature(
        leaf_public,
        certificate_preimage(cert).encode(),
        signature["value"],
    )
    cert_before = parse_at(cert["not_before"], "SC1 not_before")
    cert_after = parse_at(cert["not_after"], "SC1 not_after")
    mandate_before = parse_at(mandate["not_before"], "mandate not_before")
    mandate_after = parse_at(mandate["not_after"], "mandate not_after")
    at = parse_at(operation_at, "operation at")
    if cert_before >= cert_after:
        reject("SC1 interval is empty")
    if cert_before < mandate_before or cert_after > mandate_after:
        reject("SC1 interval escapes leaf mandate")
    if not (cert_before <= at <= cert_after):
        reject("operation is outside SC1 interval")


def validate_operation_ref(value: Any) -> dict[str, Any]:
    ref = require_exact(value, REFERENCE_KEYS, "operation_ref")
    if ref[OPERATION_KEY] != OPERATION_PROFILE:
        reject("unknown operation_ref profile")
    if not isinstance(ref["occurrence"], str) or not OCCURRENCE_RE.fullmatch(
        ref["occurrence"]
    ):
        reject("invalid operation occurrence")
    if not isinstance(ref["commitment"], str) or not COMMITMENT_RE.fullmatch(
        ref["commitment"]
    ):
        reject("invalid operation commitment")
    return ref


def validate_session_proof(
    proof: Any,
    *,
    operation_ref: dict[str, Any],
    expected_session_key: str,
) -> None:
    candidate = require_exact(proof, PROOF_KEYS, "session proof")
    if candidate[PROOF_KEY] != PROOF_PROFILE:
        reject("unknown session-proof profile")
    validate_operation_ref(candidate["operation_ref"])
    if candidate["operation_ref"] != operation_ref:
        reject("session proof operation_ref mismatch")
    if candidate["key"] != expected_session_key:
        reject("session proof key mismatch")
    verify_signature(
        SESSION_KEY.public_key(),
        jcs(
            {
                name: value
                for name, value in candidate.items()
                if name != "sig"
            }
        ).encode(),
        candidate["sig"],
    )


def validate_native_leaf_proof(
    proof: Any,
    operation_ref: dict[str, Any],
    expected_leaf_key: str,
) -> None:
    candidate = require_exact(proof, {"key", "sig"}, "native leaf proof fixture")
    if candidate["key"] != expected_leaf_key:
        reject("native leaf proof key mismatch")
    verify_signature(
        LEAF_KEY.public_key(),
        NATIVE_LEAF_DOMAIN + jcs(operation_ref).encode(),
        candidate["sig"],
    )


def validate_bundle(candidate: dict[str, Any]) -> None:
    mandate = candidate["mandate"]
    certificate = candidate["certificate"]
    operation = candidate["operation_projection"]
    operation_ref = validate_operation_ref(candidate["operation_ref"])
    authority = require_exact(
        operation["authority"],
        SESSION_AUTHORITY_KEYS,
        "session authority",
    )
    if authority["actor"] != "grantee":
        reject("session authority actor mismatch")
    session = require_exact(authority["session"], SESSION_FACT_KEYS, "session fact")
    expected_session = mandate["constraints"]["session_bind"]
    if session["key"] != expected_session:
        reject("authority session key mismatch")
    validate_certificate(
        certificate,
        mandate=mandate,
        operation_at=operation["at"],
        expected_session_key=expected_session,
    )
    if session["certificate_digest"] != sha256_text(jcs(certificate).encode()):
        reject("SC1 certificate digest mismatch")
    expected_commitment = commitment(
        "aithos-core/v1/operation-commitment",
        jcs(operation).encode(),
    )
    if operation_ref != {
        OPERATION_KEY: OPERATION_PROFILE,
        "occurrence": operation["occurrence"],
        "commitment": expected_commitment,
    }:
        reject("operation_ref does not select session-bound projection")
    validate_native_leaf_proof(
        candidate["native_leaf_proof_fixture"],
        operation_ref,
        mandate["grantee"]["pubkey"],
    )
    validate_session_proof(
        candidate["session_proof"],
        operation_ref=operation_ref,
        expected_session_key=expected_session,
    )


ROOT_KEY = Ed25519PrivateKey.from_private_bytes(ROOT_SEED)
LEAF_KEY = Ed25519PrivateKey.from_private_bytes(LEAF_SEED)
SESSION_KEY = Ed25519PrivateKey.from_private_bytes(SESSION_SEED)
STRANGER_KEY = Ed25519PrivateKey.from_private_bytes(STRANGER_SEED)


def build_positive() -> dict[str, Any]:
    session_text = multibase_ed(public_bytes(SESSION_KEY))
    mandate = mandate_fixture(ROOT_KEY, LEAF_KEY, session_text)
    certificate = {
        SC1_KEY: SC1_PROFILE,
        "subject": mandate["subject"],
        "mandate_id": mandate["id"],
        "key": session_text,
        "not_before": "2026-07-18T11:30:00Z",
        "not_after": "2026-07-18T12:30:00Z",
        "signature": {
            "alg": "ed25519",
            "key": mandate["grantee"]["pubkey"],
            "value": "",
        },
    }
    sign_certificate(certificate, LEAF_KEY)
    certificate_digest = sha256_text(jcs(certificate).encode())

    mutation_vector = json.loads(
        (HERE / "cb2-operation-facts-mutation.json").read_text()
    )
    mutation = next(
        case
        for case in mutation_vector["positive_cases"]
        if case["id"] == "ethos-edit"
    )
    projection = {
        OPERATION_KEY: OPERATION_PROFILE,
        "occurrence": "op_01K00000000000000000000051",
        "subject": mandate["subject"],
        "at": "2026-07-18T12:00:00Z",
        "history_heads": ["sha256:" + "51" * 32],
        "authority": {
            "actor": "grantee",
            "key": mandate["grantee"]["pubkey"],
            "authorized_by": mandate["id"],
            "authorized_via": [
                {
                    "id": mandate["id"],
                    "certificate_digest": sha256_text(jcs(mandate).encode()),
                }
            ],
            "session": {
                "key": session_text,
                "certificate_digest": certificate_digest,
            },
        },
        "operation": {
            "kind": "mutation",
            "facts_ref": mutation["facts_ref"],
        },
    }
    operation_commitment = commitment(
        "aithos-core/v1/operation-commitment",
        jcs(projection).encode(),
    )
    operation_ref = {
        OPERATION_KEY: OPERATION_PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": operation_commitment,
    }
    native_leaf_proof = {
        "key": mandate["grantee"]["pubkey"],
        "sig": LEAF_KEY.sign(
            NATIVE_LEAF_DOMAIN + jcs(operation_ref).encode()
        ).hex(),
    }
    session_proof = {
        PROOF_KEY: PROOF_PROFILE,
        "operation_ref": operation_ref,
        "key": session_text,
        "sig": "",
    }
    session_proof["sig"] = sign_without_member(session_proof, "sig", SESSION_KEY)
    result = {
        "mandate": mandate,
        "certificate": certificate,
        "certificate_preimage_jcs": certificate_preimage(certificate),
        "certificate_jcs": jcs(certificate),
        "certificate_digest": certificate_digest,
        "operation_projection": projection,
        "operation_projection_jcs": jcs(projection),
        "operation_ref": operation_ref,
        "native_leaf_proof_fixture": native_leaf_proof,
        "native_leaf_proof_message_hex": (
            NATIVE_LEAF_DOMAIN + jcs(operation_ref).encode()
        ).hex(),
        "session_proof": session_proof,
        "session_proof_preimage_jcs": jcs(
            {
                name: value
                for name, value in session_proof.items()
                if name != "sig"
            }
        ),
    }
    validate_bundle(result)
    return result


def resign_certificate_with(
    candidate: dict[str, Any],
    key: Ed25519PrivateKey,
) -> None:
    sign_certificate(candidate["certificate"], key)
    candidate["operation_projection"]["authority"]["session"][
        "certificate_digest"
    ] = sha256_text(jcs(candidate["certificate"]).encode())
    candidate["operation_ref"]["commitment"] = commitment(
        "aithos-core/v1/operation-commitment",
        jcs(candidate["operation_projection"]).encode(),
    )
    candidate["session_proof"]["operation_ref"] = clone(candidate["operation_ref"])
    candidate["session_proof"]["sig"] = sign_without_member(
        candidate["session_proof"],
        "sig",
        SESSION_KEY,
    )
    candidate["native_leaf_proof_fixture"]["sig"] = LEAF_KEY.sign(
        NATIVE_LEAF_DOMAIN + jcs(candidate["operation_ref"]).encode()
    ).hex()


def negative_cases(valid: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutator: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutator))

    add("missing-certificate-profile", lambda c: c["certificate"].pop(SC1_KEY))
    add("unknown-certificate-profile", lambda c: c["certificate"].__setitem__(SC1_KEY, "1.0.0-draft.2"))
    add("extra-certificate-member", lambda c: c["certificate"].__setitem__("extra", True))
    add("null-certificate-key", lambda c: c["certificate"].__setitem__("key", None))
    add("certificate-subject-mismatch", lambda c: c["certificate"].__setitem__("subject", "did:aithos:zWrong"))
    add("certificate-mandate-mismatch", lambda c: c["certificate"].__setitem__("mandate_id", "mandate_01J00000000000000000000099"))
    add("certificate-session-key-mismatch", lambda c: c["certificate"].__setitem__("key", multibase_ed(public_bytes(STRANGER_KEY))))
    add("certificate-signature-key-mismatch", lambda c: c["certificate"]["signature"].__setitem__("key", multibase_ed(public_bytes(STRANGER_KEY))))
    add("certificate-signature-algorithm", lambda c: c["certificate"]["signature"].__setitem__("alg", "ecdsa"))
    add("certificate-malformed-signature", lambda c: c["certificate"]["signature"].__setitem__("value", "00"))
    add("certificate-tampered-after-sign", lambda c: c["certificate"].__setitem__("not_after", "2026-07-18T12:31:00Z"))
    add("certificate-signed-by-stranger", lambda c: resign_certificate_with(c, STRANGER_KEY))
    add("empty-certificate-interval", lambda c: c["certificate"].__setitem__("not_after", c["certificate"]["not_before"]))
    add("certificate-before-mandate", lambda c: c["certificate"].__setitem__("not_before", "2026-07-18T09:59:59Z"))
    add("certificate-after-mandate", lambda c: c["certificate"].__setitem__("not_after", "2026-07-18T14:00:01Z"))
    add("operation-before-certificate", lambda c: c["operation_projection"].__setitem__("at", "2026-07-18T11:29:59Z"))
    add("operation-after-certificate", lambda c: c["operation_projection"].__setitem__("at", "2026-07-18T12:30:01Z"))
    add("certificate-digest-mismatch", lambda c: c["operation_projection"]["authority"]["session"].__setitem__("certificate_digest", "sha256:" + "00" * 32))
    add("authority-session-key-mismatch", lambda c: c["operation_projection"]["authority"]["session"].__setitem__("key", multibase_ed(public_bytes(STRANGER_KEY))))
    add("missing-native-leaf-proof", lambda c: c.__setitem__("native_leaf_proof_fixture", None))
    add("native-leaf-proof-wrong-key", lambda c: c["native_leaf_proof_fixture"].__setitem__("key", multibase_ed(public_bytes(STRANGER_KEY))))
    add("native-leaf-proof-bad-signature", lambda c: c["native_leaf_proof_fixture"].__setitem__("sig", STRANGER_KEY.sign(NATIVE_LEAF_DOMAIN + jcs(c["operation_ref"]).encode()).hex()))
    add("missing-session-proof", lambda c: c.__setitem__("session_proof", None))
    add("session-proof-unknown-profile", lambda c: c["session_proof"].__setitem__(PROOF_KEY, "1.0.0-draft.2"))
    add("session-proof-extra-member", lambda c: c["session_proof"].__setitem__("extra", True))
    add("session-proof-wrong-operation", lambda c: c["session_proof"]["operation_ref"].__setitem__("occurrence", "op_01K00000000000000000000099"))
    add("session-proof-wrong-key", lambda c: c["session_proof"].__setitem__("key", multibase_ed(public_bytes(STRANGER_KEY))))
    add("session-proof-malformed-signature", lambda c: c["session_proof"].__setitem__("sig", "00"))
    add("session-proof-signed-by-stranger", lambda c: c["session_proof"].__setitem__("sig", sign_without_member(c["session_proof"], "sig", STRANGER_KEY)))

    out = []
    for identifier, mutator in cases:
        candidate = clone(valid)
        mutator(candidate)
        try:
            validate_bundle(candidate)
        except SessionError as error:
            if isinstance(error, SessionError) and error.code != INVALID_SESSION:
                raise AssertionError(identifier) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {identifier}")
        out.append(
            {
                "id": identifier,
                "candidate": candidate,
                "must_fail": INVALID_SESSION,
            }
        )
    return out


def historical_hashes() -> dict[str, str]:
    return {
        name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
        for name in HISTORICAL_FILES
    }


def build_vector() -> dict[str, Any]:
    positive = build_positive()
    negatives = negative_cases(positive)
    return {
        "vector": "CB2-SC1-SESSION-PROOF-1",
        "description": (
            "Independent Python cryptography oracle for a real signed draft2 "
            "session_bind mandate, the complete signed SC1 certificate and "
            "digest, session-bound W1 projection, exact session proof, double "
            "possession and 29 InvalidSession negatives."
        ),
        "profiles": {
            "session_certificate": SC1_PROFILE,
            "session_proof": PROOF_PROFILE,
            "operation": OPERATION_PROFILE,
            "mandate": MANDATE_PROFILE,
        },
        "deterministic_private_seed_hex": {
            "root": ROOT_SEED.hex(),
            "leaf": LEAF_SEED.hex(),
            "session": SESSION_SEED.hex(),
            "stranger": STRANGER_SEED.hex(),
        },
        "positive": positive,
        "negative_cases": negatives,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "negative_ids": [case["id"] for case in negatives],
            "required_error_variant": INVALID_SESSION,
            "native_leaf_proof_is_test_fixture_not_wire": True,
            "max_sessions_lifecycle_is_out_of_scope": True,
            "sc1_conveys_no_perimeter_or_authority": True,
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

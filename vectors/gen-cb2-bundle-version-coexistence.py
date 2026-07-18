#!/usr/bin/env python3
"""Generate the independent CB2 Bundle mandate-version coexistence vector.

The oracle is intentionally independent of the Rust implementation.  It
creates one DID, a historical-profile draft.1 chain, a homogeneous draft.2
reissuance, signed Gamma records under both chains, and two mixed-version
negative chains.  Frozen E+ is read only as a byte-for-byte historical
non-regression input.

No new protocol field is introduced.  The output uses only the existing DID,
mandate, Gamma, certificate-path, and JSONL wires.

Usage:
    python3 vectors/gen-cb2-bundle-version-coexistence.py
    python3 vectors/gen-cb2-bundle-version-coexistence.py --check
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
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat


VECTOR_DIR = Path(__file__).resolve().parent
DEFAULT_OUTPUT = VECTOR_DIR / "cb2-bundle-version-coexistence.json"
EPLUS_PATH = VECTOR_DIR / "eplus-attenuation.json"
EPLUS_SHA256 = "9822d9da417487740b50efc1a760883addf8fffcaa0fa2008e029ab473d1db8c"

DRAFT1 = "1.0.0-draft.1"
DRAFT2 = "1.0.0-draft.2"
ROOT_SEED = bytes.fromhex("c0" * 32)
CONTENT_SEED = bytes.fromhex("d0" * 32)
OWNER_KEX_SEED = bytes.fromhex("e0" * 32)
SUCCESSION_SEED = bytes.fromhex("f0" * 32)
AGENT_SEED = bytes.fromhex("a1" * 32)
HELPER_SEED = bytes.fromhex("b2" * 32)
CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
FIELD_P = 2**255 - 19


def jcs(value) -> str:
    """RFC 8785 for the strings, booleans, nulls, and integers used here."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def frozen_json(path: Path, expected_sha256: str) -> tuple[dict, bytes]:
    raw = path.read_bytes()
    actual = sha256_hex(raw)
    assert actual == expected_sha256, (
        f"historical input changed: {path.name}: "
        f"expected {expected_sha256}, got {actual}"
    )
    return json.loads(raw), raw


def public_bytes(
    key: Union[Ed25519PrivateKey, Ed25519PublicKey]
) -> bytes:
    public = key.public_key() if isinstance(key, Ed25519PrivateKey) else key
    return public.public_bytes(Encoding.Raw, PublicFormat.Raw)


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
        assert char in BASE58
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
    assert value.startswith("z")
    raw = base58_decode(value[1:])
    assert raw[:2] == prefix and len(raw) == 34
    return raw[2:]


def ed25519_to_x25519_public(pub: bytes) -> bytes:
    encoded_y = bytearray(pub)
    encoded_y[31] &= 0x7F
    y = int.from_bytes(encoded_y, "little")
    assert y < FIELD_P
    denominator = (1 - y) % FIELD_P
    assert denominator != 0
    u = ((1 + y) * pow(denominator, FIELD_P - 2, FIELD_P)) % FIELD_P
    return u.to_bytes(32, "little")


def ulid(number: int) -> str:
    chars = []
    for _ in range(26):
        chars.append(CROCKFORD[number & 31])
        number >>= 5
    assert number == 0
    return "".join(reversed(chars))


def mandate_id(number: int) -> str:
    return "mandate_" + ulid(number)


def gamma_id(number: int) -> str:
    return "gamma_" + ulid(number)


def parse_zulu(value: str) -> datetime:
    assert isinstance(value, str) and value.endswith("Z")
    return datetime.fromisoformat(value[:-1] + "+00:00")


ROOT_KEY = Ed25519PrivateKey.from_private_bytes(ROOT_SEED)
CONTENT_KEY = Ed25519PrivateKey.from_private_bytes(CONTENT_SEED)
OWNER_KEX_KEY = X25519PrivateKey.from_private_bytes(OWNER_KEX_SEED)
SUCCESSION_KEY = Ed25519PrivateKey.from_private_bytes(SUCCESSION_SEED)
AGENT_KEY = Ed25519PrivateKey.from_private_bytes(AGENT_SEED)
HELPER_KEY = Ed25519PrivateKey.from_private_bytes(HELPER_SEED)


def sign_doc(doc: dict, signer: Ed25519PrivateKey) -> dict:
    signed = copy.deepcopy(doc)
    signed["signature"]["value"] = ""
    signed["signature"]["value"] = signer.sign(jcs(signed).encode()).hex()
    return signed


def verify_doc(doc: dict, verifier: Ed25519PublicKey) -> None:
    unsigned = copy.deepcopy(doc)
    signature = bytes.fromhex(unsigned["signature"]["value"])
    unsigned["signature"]["value"] = ""
    verifier.verify(signature, jcs(unsigned).encode())


def grantee(label: str, key: Ed25519PrivateKey) -> dict:
    pub = public_bytes(key)
    return {
        "id": f"urn:aithos:agent:{label}",
        "label": label,
        "pubkey": multibase_ed(pub),
        "kex_pubkey": multibase_x(ed25519_to_x25519_public(pub)),
    }


def did_document() -> dict:
    root_multibase = multibase_ed(public_bytes(ROOT_KEY))
    did = "did:aithos:" + root_multibase
    return sign_doc(
        {
            "aithos-did-core": DRAFT1,
            "bundle": ["file://cb2-bundle-version-coexistence"],
            "id": did,
            "keys": {
                "content": multibase_ed(public_bytes(CONTENT_KEY)),
                "kex": multibase_x(
                    OWNER_KEX_KEY.public_key().public_bytes(
                        Encoding.Raw, PublicFormat.Raw
                    )
                ),
                "root": root_multibase,
                "succession": multibase_ed(public_bytes(SUCCESSION_KEY)),
            },
            "revocations": "gamma/gamma.jsonl",
            "signature": {"alg": "ed25519", "key": "#root", "value": ""},
        },
        ROOT_KEY,
    )


def root_mandate(
    did: str,
    version: str,
    mid: str,
    not_before: str,
    nonce_byte: str,
) -> dict:
    constraints = {
        "domains": ["a.example", "b.example"],
        "max_actions": 10,
    }
    if version == DRAFT2:
        constraints["max_children"] = 4
    return sign_doc(
        {
            "aithos-mandate-core": version,
            "id": mid,
            "subject": did,
            "parent": None,
            "issued_by": f"{did}#root",
            "grantee": grantee("agent", AGENT_KEY),
            "perimeter": ["act.x.gmail.*", "issue#depth=1"],
            "constraints": constraints,
            "not_before": not_before,
            "not_after": "2026-07-08T00:00:00Z",
            "issued_at": not_before,
            "nonce": nonce_byte * 16,
            "signature": {"alg": "ed25519", "key": "#root", "value": ""},
        },
        ROOT_KEY,
    )


def child_mandate(
    parent: dict,
    version: str,
    mid: str,
    nonce_byte: str,
    not_before: str,
) -> dict:
    constraints = {
        "domains": ["a.example"],
        "max_actions": 5,
    }
    if parent["constraints"].get("max_children") is not None:
        constraints["max_children"] = 2
    return sign_doc(
        {
            "aithos-mandate-core": version,
            "id": mid,
            "subject": parent["subject"],
            "parent": parent["id"],
            "issued_by": parent["grantee"]["pubkey"],
            "grantee": grantee("helper", HELPER_KEY),
            "perimeter": ["act.x.gmail.reply"],
            "constraints": constraints,
            "not_before": not_before,
            "not_after": "2026-07-08T00:00:00Z",
            "issued_at": not_before,
            "nonce": nonce_byte * 16,
            "signature": {
                "alg": "ed25519",
                "key": parent["grantee"]["pubkey"],
                "value": "",
            },
        },
        AGENT_KEY,
    )


def validate_mandate(doc: dict, parent: Optional[dict]) -> None:
    assert doc["aithos-mandate-core"] in {DRAFT1, DRAFT2}
    assert doc["id"].startswith("mandate_")
    assert doc["subject"].startswith("did:aithos:")
    assert doc["signature"]["alg"] == "ed25519"
    assert parse_zulu(doc["not_before"]) <= parse_zulu(doc["not_after"])
    pub = decode_multibase(doc["grantee"]["pubkey"], b"\xed\x01")
    expected_kex = ed25519_to_x25519_public(pub)
    assert decode_multibase(doc["grantee"]["kex_pubkey"], b"\xec\x01") == expected_kex
    if parent is None:
        assert doc["parent"] is None
        assert doc["issued_by"] == f"{doc['subject']}#root"
        assert doc["signature"]["key"] == "#root"
        verify_doc(doc, ROOT_KEY.public_key())
        return
    assert doc["parent"] == parent["id"]
    assert doc["subject"] == parent["subject"]
    assert doc["issued_by"] == parent["grantee"]["pubkey"]
    assert doc["signature"]["key"] == parent["grantee"]["pubkey"]
    assert parse_zulu(parent["not_before"]) <= parse_zulu(doc["not_before"])
    assert parse_zulu(doc["not_after"]) <= parse_zulu(parent["not_after"])
    assert set(doc["constraints"]["domains"]).issubset(
        set(parent["constraints"]["domains"])
    )
    assert doc["constraints"]["max_actions"] <= parent["constraints"]["max_actions"]
    parent_children = parent["constraints"].get("max_children")
    child_children = doc["constraints"].get("max_children")
    if parent_children is not None:
        assert child_children is not None and child_children <= parent_children
    parent_pub = decode_multibase(parent["grantee"]["pubkey"], b"\xed\x01")
    verify_doc(doc, Ed25519PublicKey.from_public_bytes(parent_pub))


def chain_oracle(chain: list[dict]) -> tuple[str, str]:
    assert chain
    validate_mandate(chain[0], None)
    expected_version = chain[0]["aithos-mandate-core"]
    for parent, child in zip(chain, chain[1:]):
        validate_mandate(child, parent)
        if child["aithos-mandate-core"] != expected_version:
            return "InvalidMandate", "version"
    return "valid", "accepted"


def entry_hash(entry: dict) -> str:
    return "sha256:" + sha256_hex(jcs(entry).encode())


def owner_grant(eid: str, prev: str, at: str, target: str) -> dict:
    return sign_doc(
        {
            "v": 1,
            "id": eid,
            "prev": prev,
            "at": at,
            "kind": "grant",
            "target": target,
            "payload": {},
            "signature": {"alg": "ed25519", "key": "#content", "value": ""},
        },
        CONTENT_KEY,
    )


def delegated_entry(
    eid: str,
    prev: str,
    at: str,
    kind: str,
    target: str,
    payload: dict,
    via: list[str],
    signer: Ed25519PrivateKey,
) -> dict:
    return sign_doc(
        {
            "v": 1,
            "id": eid,
            "prev": prev,
            "at": at,
            "kind": kind,
            "target": target,
            "authorized_by": via[-1],
            "authorized_via": via,
            "payload": payload,
            "signature": {
                "alg": "ed25519",
                "key": multibase_ed(public_bytes(signer)),
                "value": "",
            },
        },
        signer,
    )


def append_entry(entries: list[dict], entry: dict) -> None:
    expected_prev = entry_hash(entries[-1]) if entries else ""
    assert entry["prev"] == expected_prev
    if entries:
        assert parse_zulu(entries[-1]["at"]) <= parse_zulu(entry["at"])
    entries.append(entry)


def gamma_jsonl(entries: list[dict]) -> str:
    return "".join(jcs(entry) + "\n" for entry in entries)


def certificate_record(doc: dict, source: str) -> dict:
    canonical = jcs(doc)
    assert canonical == jcs(json.loads(canonical))
    return {
        "id": doc["id"],
        "version": doc["aithos-mandate-core"],
        "source": source,
        "jcs": canonical,
        "sha256": sha256_hex(canonical.encode()),
    }


def validate_gamma(
    entries: list[dict],
    certificates: dict[str, dict],
    did_doc: dict,
) -> None:
    previous = ""
    previous_at: Optional[datetime] = None
    for entry in entries:
        assert entry["v"] == 1
        assert entry["id"].startswith("gamma_")
        assert entry["prev"] == previous
        at = parse_zulu(entry["at"])
        assert previous_at is None or previous_at <= at
        if "authorized_via" not in entry:
            assert entry["signature"]["key"] == "#content"
            verify_doc(entry, CONTENT_KEY.public_key())
        else:
            via = entry["authorized_via"]
            assert via and entry["authorized_by"] == via[-1]
            chain = [certificates[mid] for mid in via]
            expected, _ = chain_oracle(chain)
            assert expected == "valid"
            leaf_pub = decode_multibase(chain[-1]["grantee"]["pubkey"], b"\xed\x01")
            assert entry["signature"]["key"] == chain[-1]["grantee"]["pubkey"]
            verify_doc(entry, Ed25519PublicKey.from_public_bytes(leaf_pub))
            if entry["kind"] == "action":
                assert entry["target"] == "x.gmail"
                assert entry["payload"]["action"] == "reply"
        previous = entry_hash(entry)
        previous_at = at
    assert did_doc["id"] == next(iter(certificates.values()))["subject"]


def build_vector() -> dict:
    eplus, _ = frozen_json(EPLUS_PATH, EPLUS_SHA256)
    assert eplus["signed_chain"]["agent_sk_hex"] == AGENT_SEED.hex()
    assert eplus["signed_chain"]["helper_sk_hex"] == HELPER_SEED.hex()
    assert (
        eplus["matrix"]
        and any(
            case.get("family") == "max_children"
            and case.get("case") == "drop tolerated — per-level width"
            and case.get("expected") == "valid"
            for case in eplus["matrix"]
        )
    )

    did_doc = did_document()
    did_jcs = jcs(did_doc)
    assert did_doc["keys"]["root"] == multibase_ed(public_bytes(ROOT_KEY))
    assert did_doc["keys"]["content"] == multibase_ed(public_bytes(CONTENT_KEY))
    verify_doc(did_doc, ROOT_KEY.public_key())

    draft1_root = root_mandate(
        did_doc["id"],
        DRAFT1,
        mandate_id(1100),
        "2026-07-01T00:00:00Z",
        "10",
    )
    draft1_child = child_mandate(
        draft1_root,
        DRAFT1,
        mandate_id(1101),
        "11",
        "2026-07-02T00:00:00Z",
    )
    assert chain_oracle([draft1_root, draft1_child]) == ("valid", "accepted")

    draft2_root = root_mandate(
        did_doc["id"],
        DRAFT2,
        mandate_id(1200),
        "2026-07-03T00:00:00Z",
        "30",
    )
    draft2_child = child_mandate(
        draft2_root,
        DRAFT2,
        mandate_id(1201),
        "31",
        "2026-07-03T00:00:00Z",
    )
    mixed_draft2_under_draft1 = child_mandate(
        draft1_root,
        DRAFT2,
        mandate_id(1202),
        "12",
        "2026-07-02T00:00:00Z",
    )
    mixed_draft1_under_draft2 = child_mandate(
        draft2_root,
        DRAFT1,
        mandate_id(1203),
        "32",
        "2026-07-03T00:00:00Z",
    )
    assert chain_oracle([draft2_root, draft2_child]) == ("valid", "accepted")
    assert chain_oracle([draft1_root, mixed_draft2_under_draft1]) == (
        "InvalidMandate",
        "version",
    )
    assert chain_oracle([draft2_root, mixed_draft1_under_draft2]) == (
        "InvalidMandate",
        "version",
    )

    docs = {
        "draft1_root": draft1_root,
        "draft1_child": draft1_child,
        "draft2_root": draft2_root,
        "draft2_child": draft2_child,
        "mixed_draft2_under_draft1": mixed_draft2_under_draft1,
        "mixed_draft1_under_draft2": mixed_draft1_under_draft2,
    }
    by_id = {doc["id"]: doc for doc in docs.values()}

    entries: list[dict] = []
    append_entry(
        entries,
        owner_grant(
            gamma_id(1300),
            "",
            "2026-07-01T00:00:01Z",
            draft1_root["id"],
        ),
    )
    append_entry(
        entries,
        delegated_entry(
            gamma_id(1301),
            entry_hash(entries[-1]),
            "2026-07-02T00:00:01Z",
            "grant",
            draft1_child["id"],
            {},
            [draft1_root["id"]],
            AGENT_KEY,
        ),
    )
    append_entry(
        entries,
        owner_grant(
            gamma_id(1302),
            entry_hash(entries[-1]),
            "2026-07-03T00:00:01Z",
            draft2_root["id"],
        ),
    )
    append_entry(
        entries,
        delegated_entry(
            gamma_id(1303),
            entry_hash(entries[-1]),
            "2026-07-03T00:00:02Z",
            "grant",
            draft2_child["id"],
            {},
            [draft2_root["id"]],
            AGENT_KEY,
        ),
    )
    append_entry(
        entries,
        delegated_entry(
            gamma_id(1304),
            entry_hash(entries[-1]),
            "2026-07-04T00:00:00Z",
            "action",
            "x.gmail",
            {
                "action": "reply",
                "args_hash": "sha256:"
                + sha256_hex(jcs({"message": "draft.1"}).encode()),
            },
            [draft1_root["id"], draft1_child["id"]],
            HELPER_KEY,
        ),
    )
    append_entry(
        entries,
        delegated_entry(
            gamma_id(1305),
            entry_hash(entries[-1]),
            "2026-07-04T00:00:01Z",
            "action",
            "x.gmail",
            {
                "action": "reply",
                "args_hash": "sha256:"
                + sha256_hex(jcs({"message": "draft.2"}).encode()),
            },
            [draft2_root["id"], draft2_child["id"]],
            HELPER_KEY,
        ),
    )
    validate_gamma(
        entries,
        {
            draft1_root["id"]: draft1_root,
            draft1_child["id"]: draft1_child,
            draft2_root["id"]: draft2_root,
            draft2_child["id"]: draft2_child,
        },
        did_doc,
    )

    negative_specs = [
        (
            "mixed_draft1_to_draft2",
            "mixed_draft2_under_draft1",
            [draft1_root["id"], mixed_draft2_under_draft1["id"]],
            gamma_id(1306),
        ),
        (
            "mixed_draft2_to_draft1",
            "mixed_draft1_under_draft2",
            [draft2_root["id"], mixed_draft1_under_draft2["id"]],
            gamma_id(1307),
        ),
    ]
    negative_cases = []
    positive_names = [
        "draft1_root",
        "draft1_child",
        "draft2_root",
        "draft2_child",
    ]
    for case_id, mixed_name, via, eid in negative_specs:
        negative_entries = copy.deepcopy(entries)
        mixed_entry = delegated_entry(
            eid,
            entry_hash(negative_entries[-1]),
            "2026-07-04T00:00:02Z",
            "action",
            "x.gmail",
            {
                "action": "reply",
                "args_hash": "sha256:"
                + sha256_hex(jcs({"message": case_id}).encode()),
            },
            via,
            HELPER_KEY,
        )
        append_entry(negative_entries, mixed_entry)
        assert chain_oracle([by_id[mid] for mid in via]) == (
            "InvalidMandate",
            "version",
        )
        rendered = gamma_jsonl(negative_entries)
        negative_cases.append(
            {
                "id": case_id,
                "certificate_names": positive_names + [mixed_name],
                "authorized_via": via,
                "gamma_path": "gamma/2026-07.jsonl",
                "gamma_jsonl": rendered,
                "gamma_sha256": sha256_hex(rendered.encode()),
                "expected": "InvalidMandate",
                "decision_stage": "version",
            }
        )

    positive_jsonl = gamma_jsonl(entries)
    return {
        "vector": "CB2-BUNDLE-VERSION-COEXISTENCE-1",
        "description": (
            "One existing FsStore/DID carries independent homogeneous draft.1 "
            "and draft.2 mandate chains. Existing certs/<id>.json and Gamma "
            "JSONL wires prove delegated records under each chain, survive "
            "drop/reopen, and reject mixed-version authorized_via before "
            "attenuation. No new signed field, kind, version, or layout is used."
        ),
        "historical_inputs": {
            "eplus_draft1_chain": {
                "file": "eplus-attenuation.json",
                "sha256": EPLUS_SHA256,
                "root_jcs_sha256": sha256_hex(
                    eplus["signed_chain"]["parent_jcs"].encode()
                ),
                "child_jcs_sha256": sha256_hex(
                    eplus["signed_chain"]["child_ok_jcs"].encode()
                ),
            },
        },
        "did": {
            "id": did_doc["id"],
            "path": "did.json",
            "jcs": did_jcs,
            "sha256": sha256_hex(did_jcs.encode()),
        },
        "certificates": {
            name: certificate_record(
                doc,
                (
                    "generated historical draft.1 profile"
                    if name.startswith("draft1_")
                    else "generated"
                ),
            )
            for name, doc in docs.items()
        },
        "positive": {
            "certificate_names": positive_names,
            "chains": {
                "draft1": ["draft1_root", "draft1_child"],
                "draft2": ["draft2_root", "draft2_child"],
            },
            "gamma_path": "gamma/2026-07.jsonl",
            "gamma_jsonl": positive_jsonl,
            "gamma_sha256": sha256_hex(positive_jsonl.encode()),
            "delegated_entry_ids": {
                "draft1": [entries[1]["id"], entries[4]["id"]],
                "draft2": [entries[3]["id"], entries[5]["id"]],
            },
            "expected": {
                "one_fsstore": True,
                "one_did": True,
                "draft1_chain": "valid",
                "draft2_chain": "valid",
                "cold_gamma_verify": "valid",
            },
        },
        "negative_cases": negative_cases,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="output path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail unless the existing output is byte-identical",
    )
    args = parser.parse_args()
    vector = build_vector()
    rendered = json.dumps(vector, indent=2, ensure_ascii=False) + "\n"
    if args.check:
        existing = args.output.read_text(encoding="utf-8")
        assert existing == rendered, f"{args.output} is stale; regenerate it"
        print(
            f"verified {args.output} — sha256 {sha256_hex(rendered.encode())}"
        )
        return
    args.output.write_text(rendered, encoding="utf-8")
    print(
        f"wrote {args.output} — 1 positive coexistence fixture, "
        f"{len(vector['negative_cases'])} mixed-version negatives, "
        f"sha256 {sha256_hex(rendered.encode())}"
    )


if __name__ == "__main__":
    main()

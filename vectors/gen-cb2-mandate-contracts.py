#!/usr/bin/env python3
"""Independent CB2 oracle for the mandate contracts frozen by specs 04 and 05.

The generator intentionally uses only Python's standard library.  Its Ed25519,
multibase/base58btc, JCS-for-this-integer-free-fixture, perimeter parser,
containment, lattice, form, and constraint decisions do not call Rust.

Covered contracts:

* the historical E1 mandate without an ``id=`` selector remains byte-identical;
* ``id=`` parse/JCS/round-trip/containment and invalid selector forms;
* ``delete`` covers ``read`` but neither ``edit`` nor ``append``;
* T3 form for draft.1/draft.2, announced signer keys, identifiers, nonces,
  RFC 3339 Zulu timestamps, inverted windows, selector duplicates, and depth 0;
* malformed known root constraints, preserved unknown root-leaf constraints,
  and fail-closed unknown constraints on a delegation link.

No protocol field or public reason code is introduced.  Case names and expected
booleans are vector metadata only.

Usage:
    python3 gen-cb2-mandate-contracts.py
    python3 gen-cb2-mandate-contracts.py --check
    python3 gen-cb2-mandate-contracts.py --output /tmp/cb2.json
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime, timezone
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-mandate-contracts.json"
E1_REFERENCE = HERE / "e1-mandate.json"
F1_REFERENCE = HERE / "f1-gamma-chain.json"
E1_MANDATE_SHA256 = "abace8df9fc509923836310dd4d22693b2cb38159a43cb9a85f40d6e9bcc5055"

DRAFT_1 = "1.0.0-draft.1"
DRAFT_2 = "1.0.0-draft.2"
SUPPORTED_VERSIONS = {DRAFT_1, DRAFT_2}

ROOT_SEED = bytes.fromhex(
    "000102030405060708090a0b0c0d0e0f"
    "101112131415161718191a1b1c1d1e1f"
)
AGENT_SEED = bytes.fromhex("a1" * 32)
HELPER_SEED = bytes.fromhex("b2" * 32)

SID_FOLDER = "00000000000000000000000001"
SID_ONE = "00000000000000000000000002"
SID_TWO = "00000000000000000000000003"

NB = "2026-07-01T00:00:00Z"
NA = "2026-07-08T00:00:00Z"
ISSUED = "2026-07-01T00:00:00Z"

ULID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
TAG_RE = re.compile(r"^[a-z0-9_-]{1,64}$")
ZULU_RE = re.compile(
    r"^(\d{4})-(\d{2})-(\d{2})T"
    r"(\d{2}):(\d{2}):(\d{2})(\.\d+)?Z$"
)
HEX_SIGNATURE_RE = re.compile(r"^[0-9a-fA-F]{128}$")
DURATION_RE = re.compile(r"^(\d+)([dhms])$")

VERBS = {"read", "edit", "append", "delete", "write"}
ZONES = {"public", "circle", "self"}
KNOWN_CONSTRAINTS = {
    "max_actions",
    "max_children",
    "max_sessions",
    "max_actions_per",
    "rate_limit",
    "active_windows",
    "budgets",
    "log_reads",
    "obligations",
    "counter_sign",
    "binding",
    "domains",
    "action_params",
    "disclose_agency",
    "notify",
    "purpose",
    "session_bind",
    "heartbeat",
    "freshness",
    "spend_cap",
    "first_party_only",
}


# ---------------------------------------------------------------------------
# Canonical JSON for these fixtures (objects, arrays, strings, booleans, u64).


def jcs(value: Any) -> str:
    """RFC 8785-compatible encoding for the value domain used in this vector."""

    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )


def clone(value: Any) -> Any:
    return copy.deepcopy(value)


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


# ---------------------------------------------------------------------------
# Small independent Ed25519 implementation, checked against RFC 8032 test 1.


FIELD_Q = 2**255 - 19
GROUP_L = 2**252 + 27742317777372353535851937790883648493
CURVE_D = (-121665 * pow(121666, FIELD_Q - 2, FIELD_Q)) % FIELD_Q
SQRT_M1 = pow(2, (FIELD_Q - 1) // 4, FIELD_Q)
IDENTITY = (0, 1, 1, 0)


def recover_x(y: int, sign: int) -> int:
    x2 = ((y * y - 1) * pow(CURVE_D * y * y + 1, FIELD_Q - 2, FIELD_Q)) % FIELD_Q
    x = pow(x2, (FIELD_Q + 3) // 8, FIELD_Q)
    if (x * x - x2) % FIELD_Q:
        x = (x * SQRT_M1) % FIELD_Q
    if (x * x - x2) % FIELD_Q:
        raise ValueError("point is not on Ed25519")
    if (x & 1) != sign:
        x = FIELD_Q - x
    return x


BASE_Y = (4 * pow(5, FIELD_Q - 2, FIELD_Q)) % FIELD_Q
BASE_X = recover_x(BASE_Y, 0)
BASE_POINT = (BASE_X, BASE_Y, 1, (BASE_X * BASE_Y) % FIELD_Q)


def point_add(
    left: tuple[int, int, int, int],
    right: tuple[int, int, int, int],
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = ((y1 - x1) * (y2 - x2)) % FIELD_Q
    b = ((y1 + x1) * (y2 + x2)) % FIELD_Q
    c = (2 * CURVE_D * t1 * t2) % FIELD_Q
    d = (2 * z1 * z2) % FIELD_Q
    e = b - a
    f = d - c
    g = d + c
    h = b + a
    return (
        (e * f) % FIELD_Q,
        (g * h) % FIELD_Q,
        (f * g) % FIELD_Q,
        (e * h) % FIELD_Q,
    )


def scalar_mult(
    point: tuple[int, int, int, int],
    scalar: int,
) -> tuple[int, int, int, int]:
    result = IDENTITY
    addend = point
    while scalar:
        if scalar & 1:
            result = point_add(result, addend)
        addend = point_add(addend, addend)
        scalar >>= 1
    return result


def encode_point(point: tuple[int, int, int, int]) -> bytes:
    x, y, z, _ = point
    z_inv = pow(z, FIELD_Q - 2, FIELD_Q)
    x = (x * z_inv) % FIELD_Q
    y = (y * z_inv) % FIELD_Q
    encoded = bytearray(y.to_bytes(32, "little"))
    encoded[31] |= (x & 1) << 7
    return bytes(encoded)


def decode_point(encoded: bytes) -> tuple[int, int, int, int]:
    if len(encoded) != 32:
        raise ValueError("Ed25519 point length")
    sign = encoded[31] >> 7
    y = int.from_bytes(encoded, "little") & ((1 << 255) - 1)
    if y >= FIELD_Q:
        raise ValueError("non-canonical Ed25519 y")
    x = recover_x(y, sign)
    return (x, y, 1, (x * y) % FIELD_Q)


def points_equal(
    left: tuple[int, int, int, int],
    right: tuple[int, int, int, int],
) -> bool:
    x1, y1, z1, _ = left
    x2, y2, z2, _ = right
    return (
        (x1 * z2 - x2 * z1) % FIELD_Q == 0
        and (y1 * z2 - y2 * z1) % FIELD_Q == 0
    )


def secret_scalar(seed: bytes) -> tuple[int, bytes]:
    if len(seed) != 32:
        raise ValueError("Ed25519 seed length")
    digest = hashlib.sha512(seed).digest()
    scalar = int.from_bytes(digest[:32], "little")
    scalar &= (1 << 254) - 8
    scalar |= 1 << 254
    return scalar, digest[32:]


def ed_public(seed: bytes) -> bytes:
    scalar, _ = secret_scalar(seed)
    return encode_point(scalar_mult(BASE_POINT, scalar))


def ed_sign(seed: bytes, message: bytes) -> bytes:
    scalar, prefix = secret_scalar(seed)
    public = ed_public(seed)
    nonce = int.from_bytes(hashlib.sha512(prefix + message).digest(), "little") % GROUP_L
    encoded_r = encode_point(scalar_mult(BASE_POINT, nonce))
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(),
        "little",
    ) % GROUP_L
    encoded_s = ((nonce + challenge * scalar) % GROUP_L).to_bytes(32, "little")
    return encoded_r + encoded_s


def ed_verify(public: bytes, message: bytes, signature: bytes) -> bool:
    if len(public) != 32 or len(signature) != 64:
        return False
    encoded_r, encoded_s = signature[:32], signature[32:]
    scalar_s = int.from_bytes(encoded_s, "little")
    if scalar_s >= GROUP_L:
        return False
    try:
        point_a = decode_point(public)
        point_r = decode_point(encoded_r)
    except ValueError:
        return False
    challenge = int.from_bytes(
        hashlib.sha512(encoded_r + public + message).digest(),
        "little",
    ) % GROUP_L
    return points_equal(
        scalar_mult(BASE_POINT, scalar_s),
        point_add(point_r, scalar_mult(point_a, challenge)),
    )


def crypto_self_test() -> None:
    seed = bytes.fromhex(
        "9d61b19deffd5a60ba844af492ec2cc4"
        "4449c5697b326919703bac031cae7f60"
    )
    expected_public = bytes.fromhex(
        "d75a980182b10ab7d54bfed3c964073a"
        "0ee172f3daa62325af021a68f707511a"
    )
    expected_signature = bytes.fromhex(
        "e5564300c360ac729086e2cc806e828a"
        "84877f1eb8e5d974d873e06522490155"
        "5fb8821590a33bacc61e39701cf9b46b"
        "d25bf5f0595bbe24655141438e7a100b"
    )
    assert ed_public(seed) == expected_public
    assert ed_sign(seed, b"") == expected_signature
    assert ed_verify(expected_public, b"", expected_signature)


# ---------------------------------------------------------------------------
# Frozen key encodings.


BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
BASE58_INDEX = {char: index for index, char in enumerate(BASE58_ALPHABET)}
ED25519_CODEC = b"\xed\x01"
X25519_CODEC = b"\xec\x01"


def b58encode(raw: bytes) -> str:
    zeroes = len(raw) - len(raw.lstrip(b"\x00"))
    number = int.from_bytes(raw, "big")
    chars: list[str] = []
    while number:
        number, digit = divmod(number, 58)
        chars.append(BASE58_ALPHABET[digit])
    return "1" * zeroes + "".join(reversed(chars))


def b58decode(text: str) -> bytes:
    if not text:
        return b""
    number = 0
    for char in text:
        if char not in BASE58_INDEX:
            raise ValueError("invalid base58btc")
        number = number * 58 + BASE58_INDEX[char]
    body = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    return b"\x00" * (len(text) - len(text.lstrip("1"))) + body


def multibase(codec: bytes, public: bytes) -> str:
    return "z" + b58encode(codec + public)


def decode_multibase(value: Any, codec: bytes) -> bytes:
    if not isinstance(value, str) or not value.startswith("z"):
        raise ValueError("invalid multibase")
    decoded = b58decode(value[1:])
    if len(decoded) != 34 or decoded[:2] != codec:
        raise ValueError("invalid multicodec public key")
    return decoded[2:]


def ed_to_x_public(ed_public_bytes: bytes) -> bytes:
    _, y, _, _ = decode_point(ed_public_bytes)
    if y == 1:
        raise ValueError("Ed25519 point has no Montgomery image")
    montgomery_u = ((1 + y) * pow(1 - y, FIELD_Q - 2, FIELD_Q)) % FIELD_Q
    return montgomery_u.to_bytes(32, "little")


def ed_multibase(seed: bytes) -> str:
    return multibase(ED25519_CODEC, ed_public(seed))


def x_multibase_for_ed(seed: bytes) -> str:
    return multibase(X25519_CODEC, ed_to_x_public(ed_public(seed)))


ROOT_PUBLIC = ed_public(ROOT_SEED)
ROOT_KEY = multibase(ED25519_CODEC, ROOT_PUBLIC)
SUBJECT = "did:aithos:" + ROOT_KEY
AGENT_KEY = ed_multibase(AGENT_SEED)
HELPER_KEY = ed_multibase(HELPER_SEED)


# ---------------------------------------------------------------------------
# Identifiers, timestamps, perimeter grammar, and containment.


def mandate_id(suffix: str) -> str:
    if len(suffix) > 26:
        raise ValueError("mandate id suffix too long")
    value = "mandate_" + "0" * (26 - len(suffix)) + suffix
    assert valid_mandate_id(value)
    return value


def valid_ulid(value: Any) -> bool:
    return (
        isinstance(value, str)
        and ULID_RE.fullmatch(value) is not None
        and value[0] <= "7"
    )


def valid_mandate_id(value: Any) -> bool:
    return (
        isinstance(value, str)
        and value.startswith("mandate_")
        and valid_ulid(value[len("mandate_") :])
    )


def valid_subject(value: Any) -> bool:
    if not isinstance(value, str) or not value.startswith("did:aithos:"):
        return False
    try:
        public = decode_multibase(value[len("did:aithos:") :], ED25519_CODEC)
    except ValueError:
        return False
    return value == "did:aithos:" + multibase(ED25519_CODEC, public)


def parse_zulu(value: Any) -> tuple[datetime, Fraction] | None:
    if not isinstance(value, str):
        return None
    match = ZULU_RE.fullmatch(value)
    if match is None:
        return None
    year, month, day, hour, minute, second = map(int, match.groups()[:6])
    fraction = match.group(7)
    try:
        whole_second = datetime(
            year, month, day, hour, minute, second, tzinfo=timezone.utc
        )
    except ValueError:
        return None
    if fraction is None:
        exact_fraction = Fraction(0, 1)
    else:
        digits = fraction[1:]
        exact_fraction = Fraction(int(digits), 10 ** len(digits))
    return whole_second, exact_fraction


def parse_selectors(selector: str) -> dict[str, Any]:
    if not selector:
        raise ValueError("empty selector")
    parsed: dict[str, Any] = {"dir": None, "id": None, "tag": None}
    seen: set[str] = set()
    for part in selector.split("&"):
        if "=" not in part:
            raise ValueError("selector without value")
        key, value = part.split("=", 1)
        if key not in parsed or key in seen:
            raise ValueError("unknown or duplicate selector")
        seen.add(key)
        if key == "dir":
            segments = value.split("/")
            if not segments or any(not valid_ulid(segment) for segment in segments):
                raise ValueError("invalid dir sid-path")
            parsed["dir"] = segments
        elif key == "id":
            if not valid_ulid(value):
                raise ValueError("invalid id selector")
            parsed["id"] = value
        elif key == "tag":
            if TAG_RE.fullmatch(value) is None:
                raise ValueError("invalid tag")
            parsed["tag"] = value
    if parsed["id"] is not None and len(seen) != 1:
        raise ValueError("id composes with nothing")
    return parsed


def parse_gamma_selectors(selector: str) -> dict[str, Any]:
    if not selector:
        raise ValueError("empty gamma selector")
    parsed: dict[str, Any] = {
        "dir": None,
        "id": None,
        "tag": None,
        "kind_name": None,
        "action": None,
        "since": None,
        "until": None,
    }
    seen: set[str] = set()
    for part in selector.split("&"):
        if "=" not in part:
            raise ValueError("gamma selector without value")
        key, value = part.split("=", 1)
        storage_key = "kind_name" if key == "kind" else key
        if storage_key not in parsed or storage_key in seen:
            raise ValueError("unknown or duplicate gamma selector")
        seen.add(storage_key)
        if key == "dir":
            segments = value.split("/")
            if not segments or any(not valid_ulid(segment) for segment in segments):
                raise ValueError("invalid gamma dir sid-path")
            parsed["dir"] = segments
        elif key == "id":
            if not valid_ulid(value):
                raise ValueError("invalid gamma id selector")
            parsed["id"] = value
        elif key == "tag":
            if TAG_RE.fullmatch(value) is None:
                raise ValueError("invalid gamma tag")
            parsed["tag"] = value
        elif key in {"kind", "action"}:
            if not value:
                raise ValueError("empty gamma string selector")
            parsed[storage_key] = value
        elif key in {"since", "until"}:
            if parse_zulu(value) is None:
                raise ValueError("invalid gamma time selector")
            parsed[key] = value
    return parsed


def parse_entry(entry: Any) -> dict[str, Any]:
    if not isinstance(entry, str) or not entry:
        raise ValueError("perimeter entry must be a non-empty string")
    if entry == "issue":
        return {"kind": "issue", "depth": 1}
    if entry.startswith("issue#depth="):
        raw_depth = entry[len("issue#depth=") :]
        if re.fullmatch(r"\d+", raw_depth) is None:
            raise ValueError("invalid issue depth")
        depth = int(raw_depth)
        if depth < 1:
            raise ValueError("issue depth must be positive")
        return {"kind": "issue", "depth": depth}

    if entry.startswith("act.x."):
        if "#" in entry:
            raise ValueError("act entry has no selector")
        rest = entry[len("act.x.") :]
        if "." not in rest:
            raise ValueError("expected act.x.<connector>.<action|*>")
        connector, action = rest.rsplit(".", 1)
        if not connector or not action:
            raise ValueError("empty connector or action")
        return {
            "kind": "act",
            "connector": connector,
            "action": None if action == "*" else action,
        }

    if entry == "read.gamma":
        return {
            "kind": "gamma",
            "dir": None,
            "id": None,
            "tag": None,
            "kind_name": None,
            "action": None,
            "since": None,
            "until": None,
        }
    if entry.startswith("read.gamma#"):
        return {"kind": "gamma", **parse_gamma_selectors(entry[len("read.gamma#") :])}

    if entry == "revoke":
        return {"kind": "revoke", "scope": None}
    if entry.startswith("revoke."):
        scope = parse_entry("read." + entry[len("revoke.") :])
        if scope["kind"] != "ethos":
            raise ValueError("revoke scope must be an ethos perimeter")
        return {"kind": "revoke", "scope": scope}

    if entry.count("#") > 1:
        raise ValueError("multiple selector separators")
    head, separator, selector = entry.partition("#")
    if head.count(".") != 1:
        raise ValueError("expected verb.zone")
    verb, zone = head.split(".", 1)
    if verb not in VERBS or zone not in ZONES:
        raise ValueError("invalid ethos perimeter head")
    selectors = (
        parse_selectors(selector)
        if separator
        else {"dir": None, "id": None, "tag": None}
    )
    return {
        "kind": "ethos",
        "verb": verb,
        "zone": zone,
        **selectors,
    }


def render_entry(parsed: dict[str, Any]) -> str:
    kind = parsed["kind"]
    if kind == "issue":
        return f"issue#depth={parsed['depth']}"
    if kind == "act":
        return (
            f"act.x.{parsed['connector']}."
            f"{parsed['action'] if parsed['action'] is not None else '*'}"
        )
    if kind == "gamma":
        selectors: list[str] = []
        if parsed["dir"] is not None:
            selectors.append("dir=" + "/".join(parsed["dir"]))
        for key in ("id", "tag"):
            if parsed[key] is not None:
                selectors.append(f"{key}={parsed[key]}")
        if parsed["kind_name"] is not None:
            selectors.append("kind=" + parsed["kind_name"])
        for key in ("action", "since", "until"):
            if parsed[key] is not None:
                selectors.append(f"{key}={parsed[key]}")
        return "read.gamma" + ("#" + "&".join(selectors) if selectors else "")
    if kind == "revoke":
        if parsed["scope"] is None:
            return "revoke"
        scope = render_entry(parsed["scope"])
        return "revoke." + scope[len("read.") :]
    if kind != "ethos":
        raise ValueError("unknown parsed perimeter kind")
    output = f"{parsed['verb']}.{parsed['zone']}"
    selectors: list[str] = []
    if parsed["dir"] is not None:
        selectors.append("dir=" + "/".join(parsed["dir"]))
    if parsed["id"] is not None:
        selectors.append("id=" + parsed["id"])
    if parsed["tag"] is not None:
        selectors.append("tag=" + parsed["tag"])
    if selectors:
        output += "#" + "&".join(selectors)
    return output


def verb_covers(parent: str, child: str) -> bool:
    if parent == child:
        return True
    if parent == "write":
        return child in VERBS
    if parent == "append":
        return child in {"read", "edit"}
    if parent == "edit":
        return child == "read"
    if parent == "delete":
        return child == "read"
    return False


def entry_covers(parent_text: str, child_text: str) -> bool:
    try:
        parent = parse_entry(parent_text)
        child = parse_entry(child_text)
    except ValueError:
        return False
    if parent["kind"] != child["kind"]:
        return False
    if parent["kind"] == "issue":
        return (
            child["depth"] < parent["depth"]
        )
    if parent["kind"] == "act":
        return (
            parent["connector"] == child["connector"]
            and (
                parent["action"] is None
                or parent["action"] == child["action"]
            )
        )
    if parent["kind"] == "gamma":
        parent_dir = parent["dir"]
        child_dir = child["dir"]
        if parent_dir is not None and (
            child_dir is None
            or child_dir[: len(parent_dir)] != parent_dir
        ):
            return False
        for key in ("id", "tag", "kind_name", "action"):
            if parent[key] is not None and parent[key] != child[key]:
                return False
        if parent["since"] is not None and (
            child["since"] is None
            or parse_zulu(child["since"]) < parse_zulu(parent["since"])
        ):
            return False
        if parent["until"] is not None and (
            child["until"] is None
            or parse_zulu(child["until"]) > parse_zulu(parent["until"])
        ):
            return False
        return True
    if parent["kind"] == "revoke":
        if parent["scope"] is None:
            return True
        if child["scope"] is None:
            return False
        return entry_covers(
            render_entry(parent["scope"]),
            render_entry(child["scope"]),
        )
    if parent["kind"] != "ethos":
        return False
    if parent["zone"] != child["zone"] or not verb_covers(
        parent["verb"],
        child["verb"],
    ):
        return False

    parent_unscoped = all(parent[key] is None for key in ("dir", "id", "tag"))
    if child["id"] is not None:
        return parent_unscoped or parent["id"] == child["id"]
    if parent["id"] is not None:
        return False

    parent_dir = parent["dir"]
    child_dir = child["dir"]
    if parent_dir is not None:
        # Ethos dir containment is nodal: the terminal SID names the node.
        # Leading path segments are issuance-time audit coordinates only.
        if child_dir is None or parent_dir[-1] not in child_dir:
            return False
    if parent["tag"] is not None and parent["tag"] != child["tag"]:
        return False
    return True


# ---------------------------------------------------------------------------
# Signed mandates and the form/chain slice exercised by this vector.


def unsigned_jcs(document: dict[str, Any]) -> str:
    unsigned = clone(document)
    unsigned["signature"]["value"] = ""
    return jcs(unsigned)


def sign_document(document: dict[str, Any], signer_seed: bytes) -> dict[str, Any]:
    signed = clone(document)
    signed["signature"]["value"] = ""
    signature = ed_sign(signer_seed, jcs(signed).encode("utf-8"))
    signed["signature"]["value"] = signature.hex()
    assert ed_verify(ed_public(signer_seed), unsigned_jcs(signed).encode(), signature)
    return signed


def make_mandate(
    *,
    version: str,
    identifier: str,
    grantee_seed: bytes,
    grantee_label: str,
    perimeter: list[str],
    constraints: dict[str, Any],
    nonce: Any,
    signer_seed: bytes,
    subject: str = SUBJECT,
    parent: str | None = None,
    issued_by: str | None = None,
    signature_key: str | None = None,
    not_before: Any = NB,
    not_after: Any = NA,
    issued_at: Any = ISSUED,
) -> dict[str, Any]:
    is_root = parent is None
    issuer = issued_by if issued_by is not None else (
        subject + "#root" if is_root else AGENT_KEY
    )
    announced = signature_key if signature_key is not None else (
        "#root" if is_root else issuer
    )
    document = {
        "aithos-mandate-core": version,
        "id": identifier,
        "subject": subject,
        "parent": parent,
        "issued_by": issuer,
        "grantee": {
            "id": f"urn:aithos:agent:{grantee_label}",
            "label": grantee_label,
            "pubkey": ed_multibase(grantee_seed),
            "kex_pubkey": x_multibase_for_ed(grantee_seed),
        },
        "perimeter": perimeter,
        "constraints": constraints,
        "not_before": not_before,
        "not_after": not_after,
        "issued_at": issued_at,
        "nonce": nonce,
        "signature": {
            "alg": "ed25519",
            "key": announced,
            "value": "",
        },
    }
    return sign_document(document, signer_seed)


def is_u64(value: Any) -> bool:
    return (
        isinstance(value, int)
        and not isinstance(value, bool)
        and 0 <= value <= 2**64 - 1
    )


def duration_seconds(value: Any) -> int | None:
    if not isinstance(value, str):
        return None
    match = DURATION_RE.fullmatch(value)
    if match is None:
        return None
    multiplier = {"d": 86_400, "h": 3_600, "m": 60, "s": 1}[match.group(2)]
    seconds = int(match.group(1)) * multiplier
    return seconds if seconds <= 2**63 - 1 else None


def is_string_list(value: Any) -> bool:
    return isinstance(value, list) and all(isinstance(item, str) for item in value)


def window_shape(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    anchor = parse_zulu(value.get("anchor"))
    if anchor is None or anchor[1] != 0:
        return False
    if duration_seconds(value.get("duration")) is None:
        return False
    if "period" in value:
        period = duration_seconds(value["period"])
        if period is None or period <= 0:
            return False
    if "until" in value:
        until = parse_zulu(value["until"])
        if until is None or until[1] != 0:
            return False
    if "count" in value and not is_u64(value["count"]):
        return False
    return True


def obligation_shape(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    for key in ("id", "check", "applies_to", "verdict"):
        if not isinstance(value.get(key), str) or not value[key]:
            return False
    if not is_string_list(value.get("attestor")) or not value["attestor"]:
        return False
    try:
        applies_to = parse_entry(value["applies_to"])
    except ValueError:
        return False
    if applies_to["kind"] != "act":
        return False
    if "max_age" in value and duration_seconds(value["max_age"]) is None:
        return False
    return True


def budget_profiles_shape(value: Any) -> bool:
    if not isinstance(value, list):
        return False
    identifiers: set[str] = set()
    for profile in value:
        if not isinstance(profile, dict):
            return False
        identifier = profile.get("id")
        if (
            not isinstance(identifier, str)
            or not identifier
            or identifier in identifiers
        ):
            return False
        identifiers.add(identifier)
        if "models" in profile and not is_string_list(profile["models"]):
            return False
        for key in ("token_budget", "max_actions"):
            if key in profile and not is_u64(profile[key]):
                return False
        if "active_windows" in profile and (
            not isinstance(profile["active_windows"], list)
            or not all(window_shape(window) for window in profile["active_windows"])
        ):
            return False
        if "require_attestation" in profile and not isinstance(
            profile["require_attestation"],
            bool,
        ):
            return False
        if "attestation_key" in profile and (
            not isinstance(profile["attestation_key"], str)
            or not profile["attestation_key"]
        ):
            return False
        if profile.get("require_attestation") is True and "attestation_key" not in profile:
            return False
    return True


def action_params_shape(value: Any) -> bool:
    if not isinstance(value, dict):
        return False
    for action, predicates in value.items():
        if not action or not isinstance(predicates, dict):
            return False
        for predicate, predicate_value in predicates.items():
            if predicate == "recipients_allow":
                if not is_string_list(predicate_value):
                    return False
            elif predicate == "no_attachments":
                if predicate_value is not True:
                    return False
            else:
                return False
    return True


def constraint_shape(
    constraints: Any,
) -> tuple[bool, dict[str, Any]]:
    if not isinstance(constraints, dict):
        return False, {}
    unknown = {
        key: clone(value)
        for key, value in constraints.items()
        if key not in KNOWN_CONSTRAINTS
    }
    for key, value in constraints.items():
        if key not in KNOWN_CONSTRAINTS:
            continue
        if key in {"max_actions", "max_children", "max_sessions"}:
            valid = is_u64(value)
        elif key == "max_actions_per":
            valid = (
                isinstance(value, dict)
                and duration_seconds(value.get("window")) is not None
                and is_u64(value.get("n"))
            )
        elif key == "rate_limit":
            valid = (
                isinstance(value, dict)
                and isinstance(value.get("action"), str)
                and bool(value["action"])
                and duration_seconds(value.get("window")) is not None
                and is_u64(value.get("n"))
            )
        elif key == "active_windows":
            valid = isinstance(value, list) and all(
                window_shape(window) for window in value
            )
        elif key == "budgets":
            valid = budget_profiles_shape(value)
        elif key in {"log_reads", "disclose_agency", "first_party_only"}:
            valid = value is True
        elif key == "obligations":
            valid = isinstance(value, list) and all(
                obligation_shape(obligation) for obligation in value
            )
        elif key in {"counter_sign", "binding", "domains", "notify"}:
            valid = is_string_list(value)
        elif key == "action_params":
            valid = action_params_shape(value)
        elif key in {"purpose", "session_bind"}:
            valid = isinstance(value, str) and bool(value)
        elif key == "heartbeat":
            valid = (
                isinstance(value, dict)
                and duration_seconds(value.get("every")) is not None
                and duration_seconds(value.get("grace")) is not None
            )
        elif key == "freshness":
            valid = duration_seconds(value) is not None
        elif key == "spend_cap":
            valid = (
                isinstance(value, dict)
                and isinstance(value.get("unit"), str)
                and bool(value["unit"])
                and is_u64(value.get("amount"))
            )
        else:
            raise AssertionError(f"unhandled known constraint {key}")
        if not valid:
            return False, unknown
    return True, unknown


def grantee_form_valid(grantee: Any) -> bool:
    if not isinstance(grantee, dict):
        return False
    if not isinstance(grantee.get("id"), str) or not grantee["id"]:
        return False
    if not isinstance(grantee.get("label"), str):
        return False
    try:
        signing_public = decode_multibase(grantee.get("pubkey"), ED25519_CODEC)
        announced_kex = decode_multibase(grantee.get("kex_pubkey"), X25519_CODEC)
        expected_kex = ed_to_x_public(signing_public)
    except (TypeError, ValueError):
        return False
    return announced_kex == expected_kex


def form_valid(
    document: Any,
    *,
    is_root: bool,
    parent: dict[str, Any] | None,
    chain_leaf: bool,
) -> bool:
    if not isinstance(document, dict):
        return False
    required = {
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
    if not required <= document.keys():
        return False
    if document["aithos-mandate-core"] not in SUPPORTED_VERSIONS:
        return False
    if not valid_mandate_id(document["id"]) or not valid_subject(document["subject"]):
        return False
    if not grantee_form_valid(document["grantee"]):
        return False
    if not isinstance(document["nonce"], str) or not document["nonce"]:
        return False

    not_before = parse_zulu(document["not_before"])
    not_after = parse_zulu(document["not_after"])
    issued_at = parse_zulu(document["issued_at"])
    if None in (not_before, not_after, issued_at) or not_before > not_after:
        return False

    perimeter = document["perimeter"]
    if not isinstance(perimeter, list):
        return False
    try:
        for entry in perimeter:
            parsed = parse_entry(entry)
            assert render_entry(parsed)
    except (AssertionError, ValueError):
        return False

    shape_ok, unknown = constraint_shape(document["constraints"])
    if not shape_ok:
        return False
    if unknown and not (is_root and chain_leaf):
        return False

    signature = document["signature"]
    if not isinstance(signature, dict):
        return False
    if signature.get("alg") != "ed25519":
        return False
    if HEX_SIGNATURE_RE.fullmatch(signature.get("value", "")) is None:
        return False

    if is_root:
        if document["parent"] is not None:
            return False
        if document["issued_by"] != document["subject"] + "#root":
            return False
        if signature.get("key") != "#root":
            return False
    else:
        if parent is None:
            return False
        if not valid_mandate_id(document["parent"]):
            return False
        if document["parent"] != parent["id"]:
            return False
        if document["subject"] != parent["subject"]:
            return False
        if document["issued_by"] != parent["grantee"]["pubkey"]:
            return False
        if signature.get("key") != parent["grantee"]["pubkey"]:
            return False
        if document["grantee"]["pubkey"] == document["issued_by"]:
            return False
    return True


def document_signature_valid(
    document: dict[str, Any],
    verifier_public: bytes,
) -> bool:
    try:
        signature = bytes.fromhex(document["signature"]["value"])
    except (KeyError, TypeError, ValueError):
        return False
    return ed_verify(
        verifier_public,
        unsigned_jcs(document).encode("utf-8"),
        signature,
    )


def chain_valid(chain: list[dict[str, Any]]) -> bool:
    if not chain:
        return False
    for index, document in enumerate(chain):
        parent = chain[index - 1] if index else None
        if not form_valid(
            document,
            is_root=index == 0,
            parent=parent,
            chain_leaf=index == len(chain) - 1,
        ):
            return False
        if index == 0:
            try:
                verifier = decode_multibase(
                    document["subject"][len("did:aithos:") :],
                    ED25519_CODEC,
                )
            except ValueError:
                return False
        else:
            assert parent is not None
            if document["aithos-mandate-core"] != parent["aithos-mandate-core"]:
                return False
            try:
                verifier = decode_multibase(parent["grantee"]["pubkey"], ED25519_CODEC)
            except ValueError:
                return False
            try:
                parent_entries = [parse_entry(entry) for entry in parent["perimeter"]]
                parent_depth = next(
                    entry["depth"]
                    for entry in parent_entries
                    if entry["kind"] == "issue"
                )
            except (StopIteration, ValueError):
                return False
            for child_entry in document["perimeter"]:
                parsed_child = parse_entry(child_entry)
                if parsed_child["kind"] == "issue":
                    if parsed_child["depth"] >= parent_depth:
                        return False
                elif not any(
                    parent_entry["kind"] == "ethos"
                    and entry_covers(render_entry(parent_entry), child_entry)
                    for parent_entry in parent_entries
                ):
                    return False
        if not document_signature_valid(document, verifier):
            return False
    return True


# ---------------------------------------------------------------------------
# Fixture and case construction.


ROOT_D1_ID = mandate_id("C1")
ROOT_D2_ID = mandate_id("C2")
CHILD_D1_ID = mandate_id("D1")
CHILD_D2_ID = mandate_id("D2")


def root_fixture(version: str, identifier: str) -> dict[str, Any]:
    return make_mandate(
        version=version,
        identifier=identifier,
        grantee_seed=AGENT_SEED,
        grantee_label="agent",
        perimeter=[f"read.circle#id={SID_ONE}", "issue#depth=1"],
        constraints={},
        nonce="?",
        signer_seed=ROOT_SEED,
    )


def child_fixture(
    version: str,
    identifier: str,
    parent: dict[str, Any],
    *,
    constraints: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return make_mandate(
        version=version,
        identifier=identifier,
        grantee_seed=HELPER_SEED,
        grantee_label="helper",
        perimeter=[f"read.circle#id={SID_ONE}"],
        constraints={} if constraints is None else constraints,
        nonce="child-nonce",
        signer_seed=AGENT_SEED,
        parent=parent["id"],
        issued_by=parent["grantee"]["pubkey"],
        signature_key=parent["grantee"]["pubkey"],
    )


def mutate_and_sign(
    base: dict[str, Any],
    signer_seed: bytes,
    mutation,
) -> dict[str, Any]:
    document = clone(base)
    mutation(document)
    return sign_document(document, signer_seed)


def form_case(
    case_name: str,
    document: dict[str, Any],
    *,
    expected: bool,
    is_root: bool,
    parent: dict[str, Any] | None = None,
    parent_fixture: str | None = None,
) -> dict[str, Any]:
    actual = form_valid(
        document,
        is_root=is_root,
        parent=parent,
        chain_leaf=True,
    )
    assert actual is expected, (case_name, actual, expected)
    output = {
        "case": case_name,
        "document_jcs": jcs(document),
        "expected_form_valid": expected,
        "role": "root" if is_root else "child",
    }
    if parent_fixture is not None:
        output["parent_fixture"] = parent_fixture
    return output


def build_form_cases(
    root_d1: dict[str, Any],
    root_d2: dict[str, Any],
    child_d2: dict[str, Any],
    form_root_no_id: dict[str, Any],
    form_child_no_id: dict[str, Any],
    historical_f1: dict[str, Any],
) -> list[dict[str, Any]]:
    cases = [
        form_case(
            "historical F1 action perimeter",
            historical_f1,
            expected=True,
            is_root=True,
        ),
        form_case("supported draft.1 root", root_d1, expected=True, is_root=True),
        form_case("supported draft.2 root", root_d2, expected=True, is_root=True),
        form_case(
            "supported punctuation nonce",
            mutate_and_sign(root_d2, ROOT_SEED, lambda d: d.__setitem__("nonce", "!")),
            expected=True,
            is_root=True,
        ),
        form_case(
            "supported fractional Zulu timestamps",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.update(
                    {
                        "not_before": "2026-07-01T00:00:00.125Z",
                        "issued_at": "2026-07-01T00:00:00.125Z",
                    }
                ),
            ),
            expected=True,
            is_root=True,
        ),
        form_case(
            "unsupported protocol version",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("aithos-mandate-core", "1.0.0-draft.3"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "signature algorithm other than ed25519",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d["signature"].__setitem__("alg", "rsa"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "root announced signer key differs from issuer",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d["signature"].__setitem__("key", "#content"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "child announced signer key differs from issuer",
            mutate_and_sign(
                form_child_no_id,
                AGENT_SEED,
                lambda d: d["signature"].__setitem__("key", "#root"),
            ),
            expected=False,
            is_root=False,
            parent=form_root_no_id,
            parent_fixture="root_draft2_form_no_id",
        ),
        form_case(
            "malformed mandate identifier",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("id", "mandate_not-a-ulid"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "malformed subject identifier",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.update(
                    {
                        "subject": "not-a-did",
                        "issued_by": "not-a-did#root",
                    }
                ),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "child subject changes along the chain",
            mutate_and_sign(
                child_d2,
                AGENT_SEED,
                lambda d: d.__setitem__(
                    "subject",
                    "did:aithos:" + HELPER_KEY,
                ),
            ),
            expected=False,
            is_root=False,
            parent=root_d2,
            parent_fixture="root_draft2",
        ),
        form_case(
            "root carries a parent identifier",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("parent", ROOT_D1_ID),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "child parent identifier differs from presented parent",
            mutate_and_sign(
                child_d2,
                AGENT_SEED,
                lambda d: d.__setitem__("parent", mandate_id("EF")),
            ),
            expected=False,
            is_root=False,
            parent=root_d2,
            parent_fixture="root_draft2",
        ),
        form_case(
            "child issued_by differs from parent grantee",
            mutate_and_sign(
                child_d2,
                AGENT_SEED,
                lambda d: d.__setitem__("issued_by", HELPER_KEY),
            ),
            expected=False,
            is_root=False,
            parent=root_d2,
            parent_fixture="root_draft2",
        ),
        form_case(
            "malformed grantee signing key",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d["grantee"].__setitem__("pubkey", "z6Mk-not-base58"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "grantee kex key does not match signing key",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d["grantee"].__setitem__(
                    "kex_pubkey",
                    x_multibase_for_ed(HELPER_SEED),
                ),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "empty nonce",
            mutate_and_sign(root_d2, ROOT_SEED, lambda d: d.__setitem__("nonce", "")),
            expected=False,
            is_root=True,
        ),
        form_case(
            "non-string nonce",
            mutate_and_sign(root_d2, ROOT_SEED, lambda d: d.__setitem__("nonce", 7)),
            expected=False,
            is_root=True,
        ),
        form_case(
            "timestamp uses an offset instead of Zulu",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("issued_at", "2026-07-01T00:00:00+00:00"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "timestamp is not a calendar instant",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("not_before", "2026-02-30T00:00:00Z"),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "validity window is inverted",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.update(
                    {
                        "not_before": "2026-07-09T00:00:00Z",
                        "not_after": "2026-07-08T00:00:00Z",
                    }
                ),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "sub-microsecond validity window is inverted",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.update(
                    {
                        "not_before": "2026-07-01T00:00:00.0000002Z",
                        "not_after": "2026-07-01T00:00:00.0000001Z",
                    }
                ),
            ),
            expected=False,
            is_root=True,
        ),
        form_case(
            "issue depth zero",
            mutate_and_sign(
                root_d2,
                ROOT_SEED,
                lambda d: d.__setitem__("perimeter", ["issue#depth=0"]),
            ),
            expected=False,
            is_root=True,
        ),
    ]
    invalid_selector_entries = [
        f"read.circle#dir={SID_FOLDER}&dir={SID_ONE}",
        "read.circle#tag=alpha&tag=beta",
        f"read.circle#id={SID_ONE}&id={SID_TWO}",
        f"read.circle#id={SID_ONE}&dir={SID_FOLDER}",
        f"read.circle#id={SID_ONE}&tag=alpha",
        f"read.circle#dir={SID_FOLDER}&id={SID_ONE}",
        f"read.circle#tag=alpha&id={SID_ONE}",
    ]
    selector_names = [
        "duplicate dir selector",
        "duplicate tag selector",
        "duplicate id selector",
        "id mixed with dir",
        "id mixed with tag",
        "dir mixed with id",
        "tag mixed with id",
    ]
    assert len(selector_names) == len(invalid_selector_entries)
    for name, entry in zip(selector_names, invalid_selector_entries):
        document = mutate_and_sign(
            root_d2,
            ROOT_SEED,
            lambda d, entry=entry: d.__setitem__("perimeter", [entry]),
        )
        cases.append(form_case(name, document, expected=False, is_root=True))
    return cases


def build_id_contract(root_d2: dict[str, Any]) -> dict[str, Any]:
    valid_entries = [
        f"read.self#id={SID_ONE}",
        f"delete.circle#id={SID_TWO}",
    ]
    roundtrips = []
    for entry in valid_entries:
        parsed = parse_entry(entry)
        rendered = render_entry(parsed)
        assert rendered == entry
        roundtrips.append(
            {
                "entry": entry,
                "parsed": parsed,
                "roundtrip": rendered,
            }
        )

    invalid_entries = [
        f"read.circle#dir={SID_FOLDER}&dir={SID_ONE}",
        "read.circle#tag=alpha&tag=beta",
        f"read.circle#id={SID_ONE}&id={SID_TWO}",
        f"read.circle#id={SID_ONE}&dir={SID_FOLDER}",
        f"read.circle#id={SID_ONE}&tag=alpha",
        f"read.circle#dir={SID_FOLDER}&id={SID_ONE}",
        f"read.circle#tag=alpha&id={SID_ONE}",
    ]
    invalid = []
    for entry in invalid_entries:
        try:
            parse_entry(entry)
            actual = True
        except ValueError:
            actual = False
        assert not actual
        invalid.append({"entry": entry, "expected_parse_valid": False})

    containment_inputs = [
        ("whole zone covers exact id", "read.circle", f"read.circle#id={SID_ONE}", True),
        (
            "identical id covers itself",
            f"read.circle#id={SID_ONE}",
            f"read.circle#id={SID_ONE}",
            True,
        ),
        (
            "different id is not covered",
            f"read.circle#id={SID_ONE}",
            f"read.circle#id={SID_TWO}",
            False,
        ),
        (
            "id does not cover whole zone",
            f"read.circle#id={SID_ONE}",
            "read.circle",
            False,
        ),
        (
            "dir never covers id",
            f"read.circle#dir={SID_FOLDER}",
            f"read.circle#id={SID_ONE}",
            False,
        ),
        (
            "tag never covers id",
            "read.circle#tag=alpha",
            f"read.circle#id={SID_ONE}",
            False,
        ),
        (
            "other zone does not cover id",
            "read.public",
            f"read.circle#id={SID_ONE}",
            False,
        ),
        (
            "dir containment follows the terminal node after an address change",
            f"read.circle#dir={SID_ONE}",
            f"read.circle#dir={SID_FOLDER}/{SID_ONE}",
            True,
        ),
    ]
    containment = []
    for case_name, parent, child, expected in containment_inputs:
        actual = entry_covers(parent, child)
        assert actual is expected, (case_name, actual, expected)
        containment.append(
            {
                "case": case_name,
                "parent": parent,
                "child": child,
                "expected_covers": expected,
            }
        )

    canonical = jcs(root_d2)
    assert any(f"#id={SID_ONE}" in entry for entry in root_d2["perimeter"])
    return {
        "canonical_mandate_jcs": canonical,
        "canonical_mandate_sha256": sha256_text(canonical),
        "containment": containment,
        "invalid_entries": invalid,
        "roundtrips": roundtrips,
    }


def build_lattice() -> list[dict[str, Any]]:
    expected = {
        "read": True,
        "edit": False,
        "append": False,
        "delete": True,
        "write": False,
    }
    output = []
    for required, covers in expected.items():
        actual = verb_covers("delete", required)
        assert actual is covers
        output.append(
            {
                "grant": "delete",
                "required": required,
                "expected_covers": covers,
            }
        )
    return output


def build_version_chains(
    root_d1: dict[str, Any],
    root_d2: dict[str, Any],
    child_d1: dict[str, Any],
    child_d2: dict[str, Any],
) -> list[dict[str, Any]]:
    mixed_d2_under_d1 = child_fixture(
        DRAFT_2,
        mandate_id("E1"),
        root_d1,
    )
    mixed_d1_under_d2 = child_fixture(
        DRAFT_1,
        mandate_id("E2"),
        root_d2,
    )
    cases = [
        ("homogeneous draft.1 chain", [root_d1, child_d1], True),
        ("homogeneous draft.2 chain", [root_d2, child_d2], True),
        ("draft.2 child under draft.1 parent", [root_d1, mixed_d2_under_d1], False),
        ("draft.1 child under draft.2 parent", [root_d2, mixed_d1_under_d2], False),
    ]
    output = []
    for case_name, chain, expected in cases:
        actual = chain_valid(chain)
        assert actual is expected, (case_name, actual, expected)
        output.append(
            {
                "case": case_name,
                "chain_jcs": [jcs(document) for document in chain],
                "expected_chain_valid": expected,
            }
        )
    return output


def constraint_root(
    identifier: str,
    constraints: dict[str, Any],
    *,
    issue: bool = False,
) -> dict[str, Any]:
    perimeter = [f"read.circle#id={SID_ONE}"]
    if issue:
        perimeter.append("issue#depth=1")
    return make_mandate(
        version=DRAFT_2,
        identifier=identifier,
        grantee_seed=AGENT_SEED,
        grantee_label="agent",
        perimeter=perimeter,
        constraints=constraints,
        nonce="constraint-root",
        signer_seed=ROOT_SEED,
    )


def build_constraint_shape_matrix() -> list[dict[str, Any]]:
    active_window = {
        "anchor": "2026-07-01T00:00:00Z",
        "count": 2,
        "duration": "1h",
        "period": "1d",
        "until": "2026-07-08T00:00:00Z",
    }
    all_known = {
        "action_params": {
            "reply": {
                "no_attachments": True,
                "recipients_allow": ["alice@example.com"],
            }
        },
        "active_windows": [active_window],
        "binding": ["publish"],
        "budgets": [
            {
                "active_windows": [active_window],
                "attestation_key": AGENT_KEY,
                "id": "haiku",
                "max_actions": 1,
                "models": ["claude-haiku"],
                "require_attestation": True,
                "token_budget": 1000,
            }
        ],
        "counter_sign": ["send"],
        "disclose_agency": True,
        "domains": ["example.com"],
        "first_party_only": True,
        "freshness": "1h",
        "heartbeat": {"every": "24h", "grace": "6h"},
        "log_reads": True,
        "max_actions": 4,
        "max_actions_per": {"n": 2, "window": "1h"},
        "max_children": 2,
        "max_sessions": 1,
        "notify": ["refusal"],
        "obligations": [
            {
                "applies_to": "act.x.gmail.reply",
                "attestor": [AGENT_KEY],
                "check": "pii.scan",
                "id": "guard",
                "max_age": "5m",
                "verdict": "pass",
            }
        ],
        "purpose": "reply to approved recipients",
        "rate_limit": {"action": "reply", "n": 2, "window": "1h"},
        "session_bind": AGENT_KEY,
        "spend_cap": {"amount": 100, "unit": "eur"},
    }
    assert set(all_known) == KNOWN_CONSTRAINTS
    cases: list[tuple[str, dict[str, Any], bool]] = [
        ("all known families well-formed", all_known, True),
        ("malformed max_actions", {"max_actions": "four"}, False),
        ("malformed max_children", {"max_children": "four"}, False),
        ("malformed max_sessions", {"max_sessions": -1}, False),
        ("malformed max_actions_per", {"max_actions_per": {"n": "two", "window": "1h"}}, False),
        ("malformed rate_limit", {"rate_limit": {"action": "", "n": 1, "window": "1h"}}, False),
        ("malformed active_windows", {"active_windows": [{"anchor": NB, "duration": "soon"}]}, False),
        (
            "fractional active_window requires a future version",
            {
                "active_windows": [
                    {
                        "anchor": "2026-07-01T00:00:00.1Z",
                        "duration": "1h",
                    }
                ]
            },
            False,
        ),
        ("malformed budgets", {"budgets": [{"id": "llm", "token_budget": "many"}]}, False),
        ("malformed log_reads", {"log_reads": False}, False),
        ("malformed obligations", {"obligations": [{"id": "guard", "attestor": []}]}, False),
        ("malformed counter_sign", {"counter_sign": "send"}, False),
        ("malformed binding", {"binding": "send"}, False),
        ("malformed domains", {"domains": "example.com"}, False),
        ("malformed action_params", {"action_params": {"reply": {"future_predicate": True}}}, False),
        ("malformed disclose_agency", {"disclose_agency": False}, False),
        ("malformed notify", {"notify": "refusal"}, False),
        ("malformed purpose", {"purpose": ""}, False),
        ("malformed session_bind", {"session_bind": ""}, False),
        ("malformed heartbeat", {"heartbeat": {"every": "soon", "grace": "6h"}}, False),
        ("malformed freshness", {"freshness": "soon"}, False),
        ("malformed spend_cap", {"spend_cap": {"amount": -1, "unit": "eur"}}, False),
        ("malformed first_party_only", {"first_party_only": False}, False),
    ]
    output = []
    for case_name, constraints, expected in cases:
        actual, unknown = constraint_shape(constraints)
        assert not unknown
        assert actual is expected, (case_name, actual, expected)
        output.append(
            {
                "case": case_name,
                "constraints": constraints,
                "expected_shape_valid": expected,
            }
        )
    return output


def build_constraint_contracts() -> dict[str, Any]:
    known_valid = constraint_root(mandate_id("F1"), {"max_actions": 4})
    known_malformed = constraint_root(mandate_id("F2"), {"max_actions": "four"})
    malformed_max_children = constraint_root(
        mandate_id("G1"),
        {"max_children": "four"},
    )
    malformed_log_reads = constraint_root(
        mandate_id("G2"),
        {"log_reads": False},
    )
    malformed_domains = constraint_root(
        mandate_id("G3"),
        {"domains": "example.com"},
    )
    unknown_leaf = constraint_root(
        mandate_id("F3"),
        {"quantum_cap": 4},
    )
    unknown_parent = constraint_root(
        mandate_id("F4"),
        {"quantum_cap": 4},
        issue=True,
    )
    child_under_unknown = child_fixture(
        DRAFT_2,
        mandate_id("F5"),
        unknown_parent,
    )
    known_parent = constraint_root(mandate_id("F6"), {}, issue=True)
    unknown_child = child_fixture(
        DRAFT_2,
        mandate_id("F7"),
        known_parent,
        constraints={"quantum_cap": 4},
    )
    unknown_copy_child = child_fixture(
        DRAFT_2,
        mandate_id("F8"),
        unknown_parent,
        constraints={"quantum_cap": 4},
    )

    root_cases = [
        ("known well-formed root constraint", known_valid, True),
        ("known malformed root constraint", known_malformed, False),
        ("known malformed root max_children", malformed_max_children, False),
        ("known malformed root log_reads", malformed_log_reads, False),
        ("known malformed root domains", malformed_domains, False),
        ("unknown constraint on directly issued chain leaf", unknown_leaf, True),
    ]
    root_output = []
    for case_name, document, certificate_valid in root_cases:
        actual = form_valid(
            document,
            is_root=True,
            parent=None,
            chain_leaf=True,
        )
        assert actual is certificate_valid, (case_name, actual, certificate_valid)
        entry = {
            "case": case_name,
            "document_jcs": jcs(document),
            "expected_certificate_valid": certificate_valid,
        }
        if case_name.startswith("unknown constraint"):
            preserved = jcs(document["constraints"])
            assert json.loads(preserved) == document["constraints"]
            entry["preserved_constraints_jcs"] = preserved
        root_output.append(entry)

    link_cases = [
        (
            "unknown constraint on delegation parent",
            [unknown_parent, child_under_unknown],
        ),
        (
            "unknown constraint introduced on child link",
            [known_parent, unknown_child],
        ),
        (
            "unknown constraint copied across link",
            [unknown_parent, unknown_copy_child],
        ),
    ]
    link_output = []
    for case_name, chain in link_cases:
        actual = chain_valid(chain)
        assert not actual, (case_name, actual)
        link_output.append(
            {
                "case": case_name,
                "chain_jcs": [jcs(document) for document in chain],
                "expected_chain_valid": False,
            }
        )
    return {
        "known_shape_matrix": build_constraint_shape_matrix(),
        "link_cases": link_output,
        "root_leaf_cases": root_output,
    }


def historical_reference() -> dict[str, Any]:
    reference = json.loads(E1_REFERENCE.read_text(encoding="utf-8"))
    mandate_jcs = reference["mandate_jcs"]
    encoded = mandate_jcs.encode("utf-8")
    digest = hashlib.sha256(encoded).hexdigest()
    assert digest == E1_MANDATE_SHA256
    parsed = json.loads(mandate_jcs)
    assert all("#id=" not in entry for entry in parsed["perimeter"])
    return {
        "byte_length": len(encoded),
        "reference": "vectors/e1-mandate.json#/mandate_jcs",
        "sha256": digest,
    }


def historical_f1_mandate() -> dict[str, Any]:
    reference = json.loads(F1_REFERENCE.read_text(encoding="utf-8"))
    mandate_jcs = reference["mandate_jcs"]
    document = json.loads(mandate_jcs)
    assert jcs(document) == mandate_jcs
    assert "act.x.gmail.*" in document["perimeter"]
    return document


def build_perimeter_grammar_regressions() -> list[dict[str, Any]]:
    entries = [
        "act.x.gmail.*",
        "act.x.social.publish",
        "read.gamma",
        (
            "read.gamma#kind=action"
            "&since=2026-07-01T00:00:00Z"
            "&until=2026-07-08T00:00:00Z"
        ),
        f"read.gamma#dir={SID_FOLDER}&tag=alpha",
        "revoke",
        f"revoke.circle#id={SID_ONE}",
    ]
    output = []
    for entry in entries:
        parsed = parse_entry(entry)
        roundtrip = render_entry(parsed)
        assert roundtrip == entry, (entry, roundtrip)
        output.append(
            {
                "entry": entry,
                "expected_parse_valid": True,
                "roundtrip": roundtrip,
            }
        )
    return output


def did_document_fixture() -> dict[str, Any]:
    document = {
        "aithos-did-core": "1.0.0-draft.1",
        "bundle": ["file://cb2-mandate-fixture"],
        "id": SUBJECT,
        "keys": {
            "content": AGENT_KEY,
            "kex": x_multibase_for_ed(ROOT_SEED),
            "root": ROOT_KEY,
            "succession": HELPER_KEY,
        },
        "revocations": "gamma/gamma.jsonl",
        "signature": {
            "alg": "ed25519",
            "key": "#root",
            "value": "",
        },
    }
    signed = sign_document(document, ROOT_SEED)
    assert document_signature_valid(signed, ROOT_PUBLIC)
    return signed


def build_vector() -> dict[str, Any]:
    crypto_self_test()

    historical_f1 = historical_f1_mandate()
    root_d1 = root_fixture(DRAFT_1, ROOT_D1_ID)
    root_d2 = root_fixture(DRAFT_2, ROOT_D2_ID)
    child_d1 = child_fixture(DRAFT_1, CHILD_D1_ID, root_d1)
    child_d2 = child_fixture(DRAFT_2, CHILD_D2_ID, root_d2)
    form_root_no_id = mutate_and_sign(
        root_d2,
        ROOT_SEED,
        lambda document: document.__setitem__(
            "perimeter",
            ["read.circle", "issue#depth=1"],
        ),
    )
    form_child_no_id = mutate_and_sign(
        child_d2,
        AGENT_SEED,
        lambda document: document.__setitem__("perimeter", ["read.circle"]),
    )
    assert chain_valid([root_d1, child_d1])
    assert chain_valid([root_d2, child_d2])
    assert chain_valid([form_root_no_id, form_child_no_id])

    return {
        "constraints": build_constraint_contracts(),
        "description": (
            "Independent CB2 mandate oracle: frozen E1 bytes without id=; exact id= "
            "parse/JCS/round-trip/entry containment; historical act/read.gamma/revoke grammar; "
            "nodal dir containment; delete lattice; T3 form and homogeneous draft.1/draft.2 "
            "chains; typed known constraints and unknown root/link behavior. Generated with "
            "standard-library Python only, including RFC 8032-checked Ed25519. Expected results "
            "are booleans; no protocol field or public reason code is added."
        ),
        "fixture_seeds": {
            "agent_signing_seed_hex": AGENT_SEED.hex(),
            "helper_signing_seed_hex": HELPER_SEED.hex(),
            "root_signing_seed_hex": ROOT_SEED.hex(),
        },
        "form_cases": build_form_cases(
            root_d1,
            root_d2,
            child_d2,
            form_root_no_id,
            form_child_no_id,
            historical_f1,
        ),
        "historical_without_id_selector": historical_reference(),
        "id_selector": build_id_contract(root_d2),
        "perimeter_grammar": build_perimeter_grammar_regressions(),
        "signed_fixtures": {
            "child_draft1_jcs": jcs(child_d1),
            "child_draft2_jcs": jcs(child_d2),
            "did_document_jcs": jcs(did_document_fixture()),
            "root_draft1_jcs": jcs(root_d1),
            "root_draft2_jcs": jcs(root_d2),
            "root_draft2_form_no_id_jcs": jcs(form_root_no_id),
        },
        "vector": "CB2-MANDATE-CONTRACTS",
        "verb_lattice": build_lattice(),
        "version_chains": build_version_chains(
            root_d1,
            root_d2,
            child_d1,
            child_d2,
        ),
    }


def encode_vector(vector: dict[str, Any]) -> str:
    return json.dumps(
        vector,
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
    ) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        help="output path (default: vectors/cb2-mandate-contracts.json)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that the existing output is exactly reproducible",
    )
    args = parser.parse_args()

    first = encode_vector(build_vector())
    second = encode_vector(build_vector())
    assert first == second, "generator is not deterministic"

    output = args.output.resolve()
    if args.check:
        existing = output.read_text(encoding="utf-8")
        if existing != first:
            raise SystemExit(f"{output} is not the deterministic generator output")
        print(f"verified {output}")
        return

    output.write_text(first, encoding="utf-8")
    print(
        f"wrote {output} — {len(first.encode('utf-8'))} bytes, "
        f"sha256:{hashlib.sha256(first.encode('utf-8')).hexdigest()}"
    )


if __name__ == "__main__":
    main()

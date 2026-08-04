#!/usr/bin/env python3
"""Independent generator for the C conformance vectors (headers, spec 03).

  c1-header-seal.json  header line seal/open (C1) and wrap (C2), spec 03.8.
                       NEVER rewritten — this generator RECONSTRUCTS it and
                       asserts it byte for byte. The file is frozen (README
                       rule 3) and its sha256 is pinned in ownership.json.
                       This closes the standing claim of independent
                       generation that had no generator in the repository.
  c3-owner-line.json   I3 owner line (spec 03.1): the owner line is the line
                       whose recipient key is the subject's owner_kex, never
                       the line whose `to` says "owner". One positive per
                       direction, three negatives, each tagged with the
                       verifier tier it binds (keyless / owner_kex-bearing).

Second-implementation rule: blake3 + PyNaCl + hmac/hashlib (manual RFC 5869
HKDF) + base58, never the Rust reference. Auto-validated against the frozen
a1-genesis.json and a2-did.json before anything is written.

Usage: python3 gen-c.py [--check]   (from vectors/)
"""

import argparse
import hmac
import json
from hashlib import sha256
from pathlib import Path

import base58
import blake3
from nacl.bindings import (
    crypto_aead_xchacha20poly1305_ietf_encrypt,
    crypto_scalarmult,
    crypto_scalarmult_base,
)

HERE = Path(__file__).resolve().parent

SEED = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")


# --- primitives (spec 00.3, 03.8) -------------------------------------------


def derive(context: str, key: bytes) -> bytes:
    """BLAKE3 derive_key — spec 00.3."""
    return blake3.blake3(key, derive_key_context=context).digest()


def multibase_x(pub: bytes) -> str:
    """x25519-pub multicodec 0xec01, base58btc — spec 00 encodings."""
    return "z" + base58.b58encode(b"\xec\x01" + pub).decode()


def hkdf_sha256(ikm: bytes, salt: bytes, info: bytes, length: int = 32) -> bytes:
    """RFC 5869, written out — the second implementation must not import the
    same library the reference uses. An empty salt is the all-zero block of
    the hash length (RFC 5869 section 2.2), which is what `Hkdf::new(None, _)`
    does on the Rust side."""
    prk = hmac.new(salt or b"\x00" * 32, ikm, sha256).digest()
    out, t, counter = b"", b"", 1
    while len(out) < length:
        t = hmac.new(prk, t + info + bytes([counter]), sha256).digest()
        out += t
        counter += 1
    return out[:length]


def line_aad(subject_did: str, node: str, key_version: int) -> bytes:
    """spec 03.8: purpose NUL did NUL node NUL key_version (decimal ASCII)."""
    return (
        b"aithos-core/v1/header-line" + b"\x00"
        + subject_did.encode() + b"\x00"
        + node.encode() + b"\x00"
        + str(key_version).encode()
    )


def wrap_aad(subject_did: str, wrapped_node: str, key_version: int) -> bytes:
    """spec 03.8: same shape, purpose `tagwrap`."""
    return (
        b"aithos-core/v1/tagwrap" + b"\x00"
        + subject_did.encode() + b"\x00"
        + wrapped_node.encode() + b"\x00"
        + str(key_version).encode()
    )


def seal_line(esk: bytes, recipient_pub: bytes, dk: bytes, nonce: bytes, aad: bytes):
    """spec 03.8 line: ECIES X25519 + HKDF-SHA256 + XChaCha20-Poly1305.
    Returns (epk, ciphertext)."""
    epk = crypto_scalarmult_base(esk)
    ss = crypto_scalarmult(esk, recipient_pub)
    kek = hkdf_sha256(
        ikm=ss,
        salt=b"",
        info=b"aithos-core/v1/hdr-kek" + b"\x00" + epk + recipient_pub,
    )
    return epk, crypto_aead_xchacha20poly1305_ietf_encrypt(dk, aad, nonce, kek)


def seal_wrap(via_key: bytes, dk: bytes, nonce: bytes, aad: bytes) -> bytes:
    """spec 03.8 wrap: key = derive("aithos-core/v1/wrap", K_via)."""
    return crypto_aead_xchacha20poly1305_ietf_encrypt(
        dk, aad, nonce, derive("aithos-core/v1/wrap", via_key)
    )


def line(to: str, kid: str, epk: bytes, nonce: bytes, c: bytes) -> dict:
    """spec 03.1: the five wire fields, hex on disk."""
    return {"to": to, "kid": kid, "epk": epk.hex(), "n": nonce.hex(), "c": c.hex()}


# --- cast, auto-validated against the frozen A1/A2 vectors -------------------

OWNER_SK = derive("aithos-core/v1/owner-kex", SEED)
OWNER_PUB = crypto_scalarmult_base(OWNER_SK)

STRANGER_SK = bytes.fromhex("21" * 32)          # = c1 grantee_sk_hex
STRANGER_PUB = crypto_scalarmult_base(STRANGER_SK)

NODE = "/e/circle"
KEY_VERSION = 1
DK = bytes.fromhex("c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7")

# Ephemerals and nonces are INPUTS (spec 03.8 / 09.2). C1's are frozen; C3's
# continue the same byte-ramp convention.
C1_OWNER_ESK = bytes.fromhex("78797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f9091929394959697")
C1_OWNER_N = bytes.fromhex("000102030405060708090a0b0c0d0e0f1011121314151617")
C1_GRANTEE_ESK = bytes.fromhex("98999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7")
C1_GRANTEE_N = bytes.fromhex("18191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f")

C3_ESK = {  # one per non-C1 line, deterministic and readable
    "stranger": bytes.fromhex("b8" * 32),
    "foreign_a": bytes.fromhex("b9" * 32),
    "foreign_b": bytes.fromhex("ba" * 32),
    "unlabelled": bytes.fromhex("bb" * 32),
}
C3_NONCE = {
    "stranger": bytes.fromhex("c0" * 24),
    "foreign_a": bytes.fromhex("c1" * 24),
    "foreign_b": bytes.fromhex("c2" * 24),
    "unlabelled": bytes.fromhex("c3" * 24),
}


def crosscheck_a1() -> str:
    """gen-g.py:150-153 pattern: reproduce a committed value before emitting.
    a1-genesis.json is frozen and was itself generated independently."""
    a1 = json.load(open(HERE / "a1-genesis.json"))
    assert SEED.hex() == a1["seed_hex"], "A1 seed drift"
    assert OWNER_PUB.hex() == a1["owner_kex_pub_hex"], "A1 owner_kex cross-check failed"
    assert multibase_x(OWNER_PUB) == a1["owner_kex_pub_multibase"], "A1 multibase failed"
    a2 = json.load(open(HERE / "a2-did.json"))
    # keys.kex of the DID document IS what §03.1 makes the owner line's kid
    assert a1["owner_kex_pub_multibase"] in a2["did_doc_jcs"], "A2 kex pin failed"
    assert a1["did"] == a2["did"], "A1/A2 did drift"
    return a2["did"]


DID = crosscheck_a1()


# --- C1 + C2: reconstruct and assert, never rewrite --------------------------


def check_c1() -> None:
    """Settles CHDR-025: the file claims independent generation; this proves it.

    C1 pins seal BYTES, not a wire header: its `owner_line` is
    {esk_hex, epk_hex, n_hex, c_hex} and carries no `kid` at all. `kid` is not
    in the AAD either (spec 03.8: purpose ‖ did ‖ node ‖ key_version), so the
    §03.1 variant-A change of the owner line's `kid` re-derives no ciphertext
    and does not invalidate this vector. A failure here is a real defect, not
    an expected consequence of that amendment.
    """
    committed = json.load(open(HERE / "c1-header-seal.json"))
    aad = line_aad(DID, NODE, KEY_VERSION)

    assert committed["subject_did"] == DID, "C1 subject_did drift"
    assert committed["node"] == NODE, "C1 node drift"
    assert committed["key_version"] == KEY_VERSION, "C1 key_version drift"
    assert committed["dk_hex"] == DK.hex(), "C1 dk drift"
    assert OWNER_SK.hex() == committed["owner_kex_sk_hex"], "C1 owner_kex secret drift"
    assert OWNER_PUB.hex() == committed["owner_pub_hex"], "C1 owner_kex public drift"
    assert STRANGER_SK.hex() == committed["grantee_sk_hex"], "C1 grantee secret drift"
    assert STRANGER_PUB.hex() == committed["grantee_pub_hex"], "C1 grantee public drift"

    for name, esk, nonce, recipient_pub in (
        ("owner_line", C1_OWNER_ESK, C1_OWNER_N, OWNER_PUB),
        ("grantee_line", C1_GRANTEE_ESK, C1_GRANTEE_N, STRANGER_PUB),
    ):
        assert committed[name]["esk_hex"] == esk.hex(), f"C1 {name} esk drift"
        assert committed[name]["n_hex"] == nonce.hex(), f"C1 {name} nonce drift"
        epk, c = seal_line(esk, recipient_pub, DK, nonce, aad)
        assert epk.hex() == committed[name]["epk_hex"], f"C1 {name} epk drift"
        assert c.hex() == committed[name]["c_hex"], f"C1 {name} ciphertext drift"

    w = committed["wrap"]
    c = seal_wrap(
        bytes.fromhex(w["via_key_hex"]),
        bytes.fromhex(w["dk_hex"]),
        bytes.fromhex(w["n_hex"]),
        wrap_aad(DID, w["wrapped_node"], w["key_version"]),
    )
    assert c.hex() == w["c_hex"], "C2 wrap drift"
    print("verified c1-header-seal.json (C1+C2) — reconstructed, not rewritten")


# --- C3: the I3 owner-line cases --------------------------------------------


def header(lines: list) -> dict:
    """spec 03.1 object shape."""
    return {"object": "header", "v": 1, "node": NODE,
            "key_versions": {str(KEY_VERSION): {"lines": lines}}}


def gen_c3() -> dict:
    aad = line_aad(DID, NODE, KEY_VERSION)
    owner_kid = multibase_x(OWNER_PUB)          # §03.1 variant A
    stranger_kid = multibase_x(STRANGER_PUB)

    # The positive owner line is byte-identical to c1-header-seal.json's seal:
    # same esk, nonce, dk, node, version, recipient.
    o_epk, o_c = seal_line(C1_OWNER_ESK, OWNER_PUB, DK, C1_OWNER_N, aad)
    owner_line = line("owner", owner_kid, o_epk, C1_OWNER_N, o_c)

    s_epk, s_c = seal_line(
        C3_ESK["stranger"], STRANGER_PUB, DK, C3_NONCE["stranger"], aad
    )
    stranger_line = line(stranger_kid, stranger_kid, s_epk, C3_NONCE["stranger"], s_c)

    # negative 2: to says "owner", declared kid is the stranger's, sealed to the
    # stranger. Keyless verifiers catch it: no line declares owner_kex.
    fa_epk, fa_c = seal_line(
        C3_ESK["foreign_a"], STRANGER_PUB, DK, C3_NONCE["foreign_a"], aad
    )
    foreign_key_line = line("owner", stranger_kid, fa_epk, C3_NONCE["foreign_a"], fa_c)

    # negative 3: declared kid IS owner_kex, seal is to the stranger. Only a
    # verifier holding owner_kex catches it — the documented boundary of §03.1.
    fb_epk, fb_c = seal_line(
        C3_ESK["foreign_b"], STRANGER_PUB, DK, C3_NONCE["foreign_b"], aad
    )
    foreign_seal_line = line("owner", owner_kid, fb_epk, C3_NONCE["foreign_b"], fb_c)

    # positive 2: the label points elsewhere, the seal is the owner's.
    u_epk, u_c = seal_line(
        C3_ESK["unlabelled"], OWNER_PUB, DK, C3_NONCE["unlabelled"], aad
    )
    unlabelled_line = line(stranger_kid, owner_kid, u_epk, C3_NONCE["unlabelled"], u_c)

    cases = [
        {
            "name": "owner_line_present",
            "verdict": "valid",
            "tier": "keyless",
            "proves": "a key version carrying a line whose kid is owner_kex "
                      "satisfies I3; the edition verifies",
            "header": header([owner_line, stranger_line]),
        },
        {
            "name": "no_owner_line_at_all",
            "verdict": "invalid",
            "must_fail": "MissingOwnerLine",
            "tier": "keyless",
            "proves": "a key version with no owner line makes the header invalid "
                      "AND the edition pinning it invalid — the half of I3 no "
                      "verifier enforced",
            "header": header([stranger_line]),
        },
        {
            "name": "owner_label_foreign_key",
            "verdict": "invalid",
            "must_fail": "MissingOwnerLine",
            "tier": "keyless",
            "proves": "a line labelled to=\"owner\" whose declared recipient key "
                      "is not owner_kex is not the owner line; the label grants "
                      "nothing",
            "header": header([foreign_key_line]),
        },
        {
            "name": "owner_label_foreign_seal",
            "verdict": "invalid",
            "must_fail": "MissingOwnerLine",
            "tier": "owner_kex",
            "proves": "a line that DECLARES owner_kex as its kid but is sealed to "
                      "another key is rejected by a verifier holding owner_kex; a "
                      "keyless verifier accepts it, and that residual gap is the "
                      "documented boundary of spec 03.1",
            "header": header([foreign_seal_line]),
        },
        {
            "name": "unlabelled_owner_line",
            "verdict": "valid",
            "tier": "keyless",
            "proves": "a line sealed to owner_kex satisfies I3 even when `to` "
                      "names something else — the label decides nothing in either "
                      "direction",
            "header": header([unlabelled_line]),
        },
    ]

    return {
        "vector": "C3",
        "description": "I3 owner line (spec 03.1): the owner line is identified by "
                       "its recipient key — the subject's owner_kex published in "
                       "the DID document — never by the `to` label. Five headers: "
                       "two positives, one per direction, and three negatives, each "
                       "stating the verifier tier it binds (keyless / "
                       "owner_kex-bearing). The positive owner line reuses C1's "
                       "ephemeral, nonce, DK, node and key version, so its seal is "
                       "byte-identical to c1-header-seal.json's owner_line. "
                       "Generated independently (Python blake3 + PyNaCl + manual "
                       "RFC5869 HKDF + base58).",
        "seed_hex": SEED.hex(),
        "subject_did": DID,
        "node": NODE,
        "key_version": KEY_VERSION,
        "dk_hex": DK.hex(),
        "owner_kex_sk_hex": OWNER_SK.hex(),
        "owner_kex_pub_hex": OWNER_PUB.hex(),
        "owner_kex_pub_multibase": owner_kid,
        "stranger_sk_hex": STRANGER_SK.hex(),
        "stranger_pub_hex": STRANGER_PUB.hex(),
        "stranger_multibase": stranger_kid,
        "line_aad_hex": aad.hex(),
        "cases": cases,
    }


def encoded(vector: dict) -> bytes:
    return (json.dumps(vector, indent=2, ensure_ascii=False) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=HERE / "c3-owner-line.json")
    args = parser.parse_args()

    check_c1()                                  # frozen: assert, never write
    payload = encoded(gen_c3())
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

#!/usr/bin/env python3
"""Independent oracle for a delegate-signed Gamma grant prepared by a gateway.

The fixture reuses the independently generated CB14 keys and mandate chain,
then signs the existing Gamma v1 `grant` wire directly with Python Ed25519.
It does not import Rust code and introduces no new protocol object.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb15-external-delegated-grant.json"
ORACLE_PATH = HERE / "gen-cb14-delegated-session-chain.py"
SPEC = importlib.util.spec_from_file_location("cb14_oracle", ORACLE_PATH)
assert SPEC and SPEC.loader
CB14 = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CB14)


def sign_entry(entry: dict[str, Any]) -> None:
    unsigned = copy.deepcopy(entry)
    unsigned["signature"]["value"] = ""
    entry["signature"]["value"] = CB14.DELEGATE.sign(
        CB14.H.jcs(unsigned).encode()
    ).hex()


def positive() -> dict[str, Any]:
    session = CB14.signed_bundle()
    parent = copy.deepcopy(session["chain"][0])
    child = copy.deepcopy(session["chain"][1])
    entry = {
        "v": 1,
        "id": "gamma_01J00000000000000000000071",
        "prev": "",
        "at": child["issued_at"],
        "kind": "grant",
        "target": child["id"],
        "authorized_by": parent["id"],
        "authorized_via": [parent["id"]],
        "payload": {},
        "signature": {
            "alg": "ed25519",
            "key": parent["grantee"]["pubkey"],
            "value": "",
        },
    }
    unsigned = copy.deepcopy(entry)
    signing_preimage = CB14.H.jcs(unsigned).encode()
    sign_entry(entry)
    return {
        "did": session["did"],
        "minting_chain": [parent],
        "child": child,
        "existing_entries": [],
        "unsigned_entry": unsigned,
        "signing_preimage_hex": signing_preimage.hex(),
        "signed_entry": entry,
    }


def negative_cases(valid: dict[str, Any]) -> list[dict[str, Any]]:
    cases = []

    wrong_target = copy.deepcopy(valid)
    wrong_target["signed_entry"]["target"] = (
        "mandate_01J00000000000000000000079"
    )
    sign_entry(wrong_target["signed_entry"])
    cases.append({"id": "wrong-child-target", "candidate": wrong_target})

    wrong_via = copy.deepcopy(valid)
    wrong_via["signed_entry"]["authorized_via"] = [valid["child"]["id"]]
    wrong_via["signed_entry"]["authorized_by"] = valid["child"]["id"]
    sign_entry(wrong_via["signed_entry"])
    cases.append({"id": "wrong-minting-chain", "candidate": wrong_via})

    wrong_time = copy.deepcopy(valid)
    wrong_time["signed_entry"]["at"] = "2026-07-22T11:29:01Z"
    sign_entry(wrong_time["signed_entry"])
    cases.append({"id": "time-differs-from-child-issuance", "candidate": wrong_time})

    stale_prev = copy.deepcopy(valid)
    stale_prev["signed_entry"]["prev"] = "sha256:" + "71" * 32
    sign_entry(stale_prev["signed_entry"])
    cases.append({"id": "stale-gamma-head", "candidate": stale_prev})

    forged = copy.deepcopy(valid)
    forged["signed_entry"]["signature"]["value"] = "00" * 64
    cases.append({"id": "forged-delegate-signature", "candidate": forged})

    return cases


def build_vector() -> dict[str, Any]:
    valid = positive()
    return {
        "vector": "CB15-EXTERNAL-DELEGATED-GRANT-1",
        "description": (
            "Independent Python Ed25519/JCS fixture for a gateway-prepared, "
            "delegate-signed existing Gamma v1 grant entry."
        ),
        "wire_change": False,
        "positive": valid,
        "negative_cases": negative_cases(valid),
        "inventory": {
            "negative_ids": [
                "wrong-child-target",
                "wrong-minting-chain",
                "time-differs-from-child-issuance",
                "stale-gamma-head",
                "forged-delegate-signature",
            ],
            "existing_gamma_wire_is_reused": True,
        },
        "deterministic_test_seed_sha256": {
            "delegate": hashlib.sha256(CB14.DELEGATE_SEED).hexdigest(),
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

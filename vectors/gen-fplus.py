#!/usr/bin/env python3
"""Independent generator for the F+ conformance vector (spec 04.10, 04.11,
07.9): absolute active windows, budget profiles, attestation receipts.

Second-implementation rule: every expected value computed with Python
datetime + PyNaCl, never by the Rust reference. Emits fplus-constraints.json.

Usage: python3 gen-fplus.py   (from vectors/)
"""

import hashlib
import json
from datetime import datetime, timedelta, timezone

from nacl.signing import SigningKey

PROVIDER_SK = bytes.fromhex("c3" * 32)
provider = SigningKey(PROVIDER_SK)


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def iso(dt) -> str:
    return dt.strftime("%Y-%m-%dT%H:%M:%SZ")


T0 = datetime(2026, 7, 2, 14, 0, tzinfo=timezone.utc)  # a Thursday, day 1 14:00


def window_verdicts():
    """Half-open [start, start+4h), weekly, from T0 — computed by datetime."""
    win = {"anchor": iso(T0), "duration": "4h", "period": "7d"}

    def inside(t, until=None, count=None):
        for k in range(0, 1000):
            start = T0 + timedelta(days=7 * k)
            if until and start > until:
                return False
            if count is not None and k >= count:
                return False
            if start <= t < start + timedelta(hours=4):
                return True
            if start > t:
                return False
        return False

    instants = {
        iso(T0): True,                                   # start inclusive
        iso(T0 + timedelta(hours=3, minutes=59, seconds=59)): True,
        iso(T0 + timedelta(hours=4)): False,             # end exclusive
        iso(T0 + timedelta(days=2, hours=1)): False,     # between occurrences
        iso(T0 + timedelta(days=14, hours=1)): True,     # occurrence 2
        iso(T0 - timedelta(seconds=1)): False,           # before anchor
    }
    for t, expected in instants.items():
        got = inside(datetime.strptime(t, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc))
        assert got == expected, (t, got, expected)

    until = T0 + timedelta(days=19)
    count2 = 2
    bounded = {
        "until_day20": {
            "until": iso(until),
            iso(T0 + timedelta(days=14, hours=1)): True,
            iso(T0 + timedelta(days=21, hours=1)): False,
        },
        "count_2": {
            "count": count2,
            iso(T0 + timedelta(days=7, hours=1)): True,
            iso(T0 + timedelta(days=14, hours=1)): False,
        },
    }
    for t, expected in list(bounded["until_day20"].items())[1:]:
        got = inside(
            datetime.strptime(t, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc),
            until=until,
        )
        assert got == expected, ("until", t)
    for t, expected in list(bounded["count_2"].items())[1:]:
        got = inside(
            datetime.strptime(t, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc),
            count=count2,
        )
        assert got == expected, ("count", t)

    return {"window": win, "verdicts": instants, "bounded": bounded}


def budget_fixture():
    profiles = [
        {
            "id": "haiku",
            "models": ["claude-haiku"],
            "token_budget": 10000,
            "active_windows": [{"anchor": iso(T0), "duration": "4h", "period": "7d"}],
            "max_actions": 1,
        },
        {"id": "gemma", "models": ["gemma"], "token_budget": 25000},
    ]
    # ledger: declared tokens cited on "gemma" (actions + inferences)
    ledger = [
        {"kind": "action", "budget_ref": "gemma", "tokens": 5000},
        {"kind": "inference", "budget_ref": "gemma", "tokens_in": 6000, "tokens_out": 1000},
        {"kind": "inference", "budget_ref": "gemma", "tokens_in": 8000, "tokens_out": 1000},
    ]
    consumed = sum(
        e.get("tokens", 0) + e.get("tokens_in", 0) + e.get("tokens_out", 0) for e in ledger
    )
    assert consumed == 21000
    return {
        "profiles": profiles,
        "ledger": ledger,
        "expected": {
            "gemma_tokens_consumed": consumed,
            "gemma_headroom": 25000 - consumed,
            "next_5000_on_gemma": "GammaBudgetExhausted",
            "next_4000_on_gemma": "valid",
            "haiku_model_gpt-oss": "GammaBudgetExhausted",
            "haiku_outside_window_at_" + iso(T0 + timedelta(days=1)): "GammaBudgetExhausted",
            "unknown_budget_ref": "GammaBudgetExhausted",
            "missing_budget_ref": "GammaBudgetExhausted",
        },
    }


def attestation_fixture():
    args_hash = "sha256:" + hashlib.sha256(b"reply to alice").hexdigest()
    receipt_payload = {"args_hash": args_hash, "model": "claude-haiku", "tokens": 8412}
    sig = provider.sign(jcs(receipt_payload).encode()).signature.hex()
    wrong_signer = SigningKey(bytes.fromhex("a1" * 32))
    bad_sig = wrong_signer.sign(jcs(receipt_payload).encode()).signature.hex()
    return {
        "provider_sk_hex": PROVIDER_SK.hex(),
        "provider_pub_hex": bytes(provider.verify_key).hex(),
        "args_hash": args_hash,
        "receipt": dict(receipt_payload, sig=sig),
        "receipt_jcs_signed": jcs(receipt_payload),
        "expected": {
            "valid_receipt": "valid",
            "wrong_signer_sig_hex": bad_sig,
            "wrong_signer": "InvalidGammaEntry",
            "replayed_on_other_args_hash": "InvalidGammaEntry",
            "attested_tokens_override": 8412,
        },
    }


if __name__ == "__main__":
    out = {
        "vector": "F+",
        "description": "Absolute windows (half-open occurrences, until/count), "
                       "budget profile tallies and verdicts, attestation receipt "
                       "signature. Generated independently (Python datetime + "
                       "PyNaCl + hashlib).",
        "windows": window_verdicts(),
        "budgets": budget_fixture(),
        "attestation": attestation_fixture(),
    }
    with open("fplus-constraints.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("wrote fplus-constraints.json")

#!/usr/bin/env python3
"""Independent generator for the G+ conformance vector (spec 04.12):
obligation receipts — guardrail pass, human approval (Model 1, WYSIWYS),
owner co_sign (counter_sign desugared, one wire shape) — plus the replay
negatives and the add-only attenuation fixtures.

Second-implementation rule: every expected value computed with Python
hashlib + PyNaCl + base58, never by the Rust reference. Self-validates
against the committed F+ vector first (same Ed25519+JCS receipt skeleton):
the provider signature of fplus-constraints.json is recomputed from its
seeds and must match byte-for-byte before anything is emitted.

Decisions graved 2026-07-10 (Mathieu): one wire shape (counter_sign
desugars to reserved obligation id "co_sign", verdict "approve", attestor =
owner content key, max_age 5m); receipt mandate_id = the entry's
authorized_by (leaf); inherited obligations JCS-identical (tighten by
adding); blocked/missing receipt = pure reject (GammaObligationUnsatisfied).

Usage: python3 gen-gplus.py   (from vectors/)
"""

import hashlib
import json

import base58
from nacl.signing import SigningKey


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def multibase_ed(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xed\x01" + pub).decode()


def sha256_prefixed(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


# ------------------------------------------------------------- self-check
# The obligation receipt shares the F+ attestation skeleton (Ed25519 over
# JCS). Recompute the committed F+ receipt from its own seeds; a mismatch
# means THIS generator's crypto path drifted — abort.

def self_check():
    with open("fplus-constraints.json") as f:
        fplus = json.load(f)
    att = fplus["attestation"]
    sk = SigningKey(bytes.fromhex(att["provider_sk_hex"]))
    payload = {k: att["receipt"][k] for k in ("args_hash", "model", "tokens")}
    assert jcs(payload) == att["receipt_jcs_signed"], "JCS drift vs committed F+"
    sig = sk.sign(jcs(payload).encode()).signature.hex()
    assert sig == att["receipt"]["sig"], "Ed25519 drift vs committed F+"
    assert bytes(sk.verify_key).hex() == att["provider_pub_hex"]


# --------------------------------------------------------------- fixtures

APPROVER_SK = bytes.fromhex("b5" * 32)   # the approver's device-held key
APPROVER2_SK = bytes.fromhex("b6" * 32)  # second key of the pinned set
GUARDRAIL_SK = bytes.fromhex("d4" * 32)  # gateway guardrail adapter
OWNER_CONTENT_SK = bytes.fromhex("c5" * 32)  # owner content key (co_sign)
STRANGER_SK = bytes.fromhex("a1" * 32)   # never pinned anywhere

approver = SigningKey(APPROVER_SK)
approver2 = SigningKey(APPROVER2_SK)
guardrail = SigningKey(GUARDRAIL_SK)
owner_content = SigningKey(OWNER_CONTENT_SK)
stranger = SigningKey(STRANGER_SK)

LEAF = "mandate_00000000000000000000GPLA"     # the entry's authorized_by
SIBLING = "mandate_00000000000000000000GPLB"  # a sibling sub-mandate

ENTRY_AT = "2026-07-10T14:04:00Z"

ARGS = {"text": "Ship it"}
ARGS_HASH = sha256_prefixed(jcs(ARGS).encode())
OTHER_ARGS_HASH = sha256_prefixed(jcs({"text": "Something else"}).encode())
SEND_ARGS_HASH = sha256_prefixed(jcs({"to": "alice@example.com"}).encode())
PRESENTED = sha256_prefixed(b"rendered: Ship it")
PRESENTED_TAMPERED = sha256_prefixed(b"rendered: Something else")


def obligations():
    return {
        "approval": {
            "id": "publish-approval",
            "check": "human.approve",
            "attestor": [multibase_ed(bytes(approver.verify_key)),
                         multibase_ed(bytes(approver2.verify_key))],
            "applies_to": "act.x.social.publish",
            "verdict": "approve",
            "max_age": "5m",
        },
        "guardrail": {
            "id": "pii-guard",
            "check": "guardrail.pii",
            "attestor": [multibase_ed(bytes(guardrail.verify_key))],
            "applies_to": "act.x.social.publish",
            "verdict": "pass",
        },
        # counter_sign: ["send"] desugars to THIS (id/verdict/max_age
        # reserved, attestor = owner content key) — never serialized in
        # constraints, shown here to pin the desugared wire.
        "co_sign_desugared": {
            "id": "co_sign",
            "check": "owner.approve",
            "attestor": [multibase_ed(bytes(owner_content.verify_key))],
            "applies_to": "send",
            "verdict": "approve",
            "max_age": "5m",
        },
    }


def signed_receipt(sk: SigningKey, obligation: str, mandate_id: str, action: str,
                   args_hash: str, verdict: str, at: str, presented=None):
    """Build the §4.12 payload, its JCS, the signature, and the checks[]
    object that rides in the entry."""
    payload = {
        "obligation": obligation,
        "mandate_id": mandate_id,
        "action": action,
        "args_hash": args_hash,
        "verdict": verdict,
        "at": at,
    }
    if presented is not None:
        payload["presented_digest"] = presented
    sig = sk.sign(jcs(payload).encode()).signature.hex()
    check = {"obligation": obligation, "args_hash": args_hash,
             "verdict": verdict, "at": at, "sig": sig}
    if presented is not None:
        check["presented_digest"] = presented
    return {"payload_jcs": jcs(payload), "check": check}


def receipts():
    ob = obligations()
    fresh = signed_receipt(approver, "publish-approval", LEAF, "publish",
                           ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                           presented=PRESENTED)
    by_second = signed_receipt(approver2, "publish-approval", LEAF, "publish",
                               ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                               presented=PRESENTED)
    no_digest = signed_receipt(approver, "publish-approval", LEAF, "publish",
                               ARGS_HASH, "approve", "2026-07-10T14:02:11Z")
    ahead = signed_receipt(approver, "publish-approval", LEAF, "publish",
                           ARGS_HASH, "approve", "2026-07-10T14:06:00Z",
                           presented=PRESENTED)
    stale = signed_receipt(approver, "publish-approval", LEAF, "publish",
                           ARGS_HASH, "approve", "2026-07-10T13:58:59Z",
                           presented=PRESENTED)
    guard_pass = signed_receipt(guardrail, "pii-guard", LEAF, "publish",
                                ARGS_HASH, "pass", "2026-07-08T14:00:00Z")
    guard_block = signed_receipt(guardrail, "pii-guard", LEAF, "publish",
                                 ARGS_HASH, "block", "2026-07-10T14:03:00Z")
    co_sign = signed_receipt(owner_content, "co_sign", LEAF, "send",
                             SEND_ARGS_HASH, "approve", "2026-07-10T14:02:11Z")
    other_args = signed_receipt(approver, "publish-approval", LEAF, "publish",
                                OTHER_ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                                presented=PRESENTED)
    sibling = signed_receipt(approver, "publish-approval", SIBLING, "publish",
                             ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                             presented=PRESENTED)
    cross_action = signed_receipt(approver, "publish-approval", LEAF, "delete",
                                  ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                                  presented=PRESENTED)
    stranger_signed = signed_receipt(stranger, "publish-approval", LEAF, "publish",
                                     ARGS_HASH, "approve", "2026-07-10T14:02:11Z",
                                     presented=PRESENTED)
    # WYSIWYS tamper: the approver signed PRESENTED; the rider is swapped.
    digest_swapped = {c: v for c, v in fresh["check"].items()}
    digest_swapped["presented_digest"] = PRESENTED_TAMPERED

    # freshness arithmetic, checked here so the numbers are Python's
    def delta_s(a, b):
        from datetime import datetime
        f = "%Y-%m-%dT%H:%M:%SZ"
        return abs((datetime.strptime(a, f) - datetime.strptime(b, f)).total_seconds())
    assert delta_s(ENTRY_AT, "2026-07-10T14:02:11Z") == 109 <= 300
    assert delta_s(ENTRY_AT, "2026-07-10T14:06:00Z") == 120 <= 300
    assert delta_s(ENTRY_AT, "2026-07-10T13:58:59Z") == 301 > 300

    return {
        "obligations": ob,
        "entry": {"authorized_by": LEAF, "action": "publish",
                  "args_hash": ARGS_HASH, "at": ENTRY_AT},
        "fresh_approval": fresh,
        "by_second_approver": by_second,
        "without_presented_digest": no_digest,
        "ahead_of_entry_clock": ahead,
        "guardrail_pass_aged_no_max_age": guard_pass,
        "co_sign_owner_send": dict(co_sign, entry_action="send",
                                   entry_args_hash=SEND_ARGS_HASH),
        "expected_valid": ["fresh_approval", "by_second_approver",
                           "without_presented_digest", "ahead_of_entry_clock",
                           "guardrail_pass_aged_no_max_age", "co_sign_owner_send"],
        "negatives": {
            "stale_receipt": stale,
            "guardrail_block_verdict": guard_block,
            "bound_to_other_args": other_args,
            "sibling_mandate_id": sibling,
            "cross_action": cross_action,
            "stranger_key": stranger_signed,
            "presented_digest_swapped": {"check": digest_swapped},
            "missing_receipt": {"check": None},
        },
        "expected_negative_error": "GammaObligationUnsatisfied",
    }


def attenuation():
    ob = obligations()
    parent = [ob["approval"]]
    loosened = dict(ob["approval"], max_age="1h")
    return {
        "parent_obligations": parent,
        "child_adds": {"obligations": parent + [ob["guardrail"]],
                       "expected": "valid"},
        "child_drops": {"obligations": [], "expected": "InvalidMandate"},
        "child_loosens": {"obligations": [loosened], "expected": "InvalidMandate"},
        "rule": "inherited obligations MUST be JCS-identical; adding tightens",
    }


if __name__ == "__main__":
    self_check()
    out = {
        "vector": "G+",
        "description": "Obligation receipts (spec 04.12): one wire shape "
                       "{obligation, mandate_id, action, args_hash, verdict, "
                       "presented_digest?, at} signed Ed25519 over JCS. "
                       "Guardrail pass, human approval (WYSIWYS, max_age), "
                       "owner co_sign (counter_sign desugared), replay "
                       "negatives, add-only attenuation. Generated "
                       "independently (Python hashlib+PyNaCl+base58); "
                       "self-checked against the committed F+ receipt.",
        "keys": {
            "approver_sk_hex": APPROVER_SK.hex(),
            "approver_pub_multibase": multibase_ed(bytes(approver.verify_key)),
            "approver2_sk_hex": APPROVER2_SK.hex(),
            "approver2_pub_multibase": multibase_ed(bytes(approver2.verify_key)),
            "guardrail_sk_hex": GUARDRAIL_SK.hex(),
            "guardrail_pub_multibase": multibase_ed(bytes(guardrail.verify_key)),
            "owner_content_sk_hex": OWNER_CONTENT_SK.hex(),
            "owner_content_pub_multibase": multibase_ed(bytes(owner_content.verify_key)),
            "stranger_sk_hex": STRANGER_SK.hex(),
        },
        "receipts": receipts(),
        "attenuation": attenuation(),
    }
    with open("gplus-obligations.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("self-check vs F+ passed; wrote gplus-obligations.json")

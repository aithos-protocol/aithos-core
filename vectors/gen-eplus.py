#!/usr/bin/env python3
"""Independent generator for the E+ conformance vector (spec 05.3 rule 3,
04.4): typed constraint attenuation, family by family, at a delegation link.

  eplus-attenuation.json
    matrix        parent/child constraint pairs with fail-closed verdicts
                  for every known family; unknown keys at a link reject
                  (M0 decision (c), 2026-07-16 — no copy-through)
    signed_chain  one owner→agent→helper chain (tightened child accepted,
                  cap-raising child rejected): canonical JCS + signatures,
                  cross-checked byte-for-byte against the Rust builders

Drop semantics, pinned by this vector (the reconciliation of
MANDATES-PRODUCT-GAPS P0.2 "dropping an inherited constraint is refused"
with the green F contract "a delegate's actions drain every ancestor's
budget"):

  - SUBTREE/CHAIN-PROTECTED families may be dropped by a child: the
    consumption engine conjoins every mandate of the chain, so absence in
    the child certificate widens nothing (max_actions, max_actions_per,
    rate_limit, max_children, budgets, heartbeat). The F-step scenarios
    require this: Bundle::delegate() mints {} children under budgeted
    parents.
  - Every OTHER family dropped at a link is a rejection: nothing in the
    consumption engine restates it for descendants (domains, action_params,
    first_party_only, max_sessions, session_bind, log_reads,
    disclose_agency, notify, purpose, spend_cap, freshness) — plus the
    families the core already refuses to drop (active_windows, obligations,
    counter_sign, binding).

Second-implementation rule: verdicts and signatures computed with Python
(sets, integer arithmetic, PyNaCl, blake3, base58), never by the Rust
reference. Usage: python3 gen-eplus.py   (from vectors/)
"""

import json
import re

import base58
import blake3
from nacl.signing import SigningKey

SEED = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
AGENT_SK = bytes.fromhex("a1" * 32)
HELPER_SK = bytes.fromhex("b2" * 32)

VALID = "valid"
INVALID = "InvalidMandate"


def jcs(obj) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def derive(context: str, key: bytes) -> bytes:
    return blake3.blake3(key, derive_key_context=context).digest()


def multibase_ed(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xed\x01" + pub).decode()


def multibase_x(pub: bytes) -> str:
    return "z" + base58.b58encode(b"\xec\x01" + pub).decode()


def kex_of(sk: SigningKey) -> str:
    return multibase_x(bytes(sk.verify_key.to_curve25519_public_key()))


# --------------------------------------------------- attenuation, in Python
# An independent implementation of the per-family containment rules. The
# Rust `constraints_attenuate` must reproduce every verdict of the matrix.

DUR = re.compile(r"^(\d+)([dhms])$")
DUR_SECS = {"d": 86400, "h": 3600, "m": 60, "s": 1}

KNOWN = {
    "max_actions", "max_children", "max_sessions", "max_actions_per",
    "rate_limit", "active_windows", "budgets", "log_reads", "obligations",
    "counter_sign", "binding", "domains", "action_params", "disclose_agency",
    "notify", "purpose", "session_bind", "heartbeat", "freshness",
    "spend_cap", "first_party_only",
}
# Families the consumption engine conjoins over the WHOLE chain (subtree
# counts / per-link checks at append time): a child may drop them.
DROPPABLE = {
    "max_actions", "max_actions_per", "rate_limit", "max_children",
    "budgets", "heartbeat",
}
KNOWN_PREDICATES = {"recipients_allow", "no_attachments"}


def dur_secs(text):
    m = DUR.match(text) if isinstance(text, str) else None
    if not m:
        raise ValueError(f"bad duration {text!r}")
    return int(m.group(1)) * DUR_SECS[m.group(2)]


def u64(v):
    if not isinstance(v, int) or isinstance(v, bool) or v < 0:
        raise ValueError(f"not a u64: {v!r}")
    return v


def str_list(v):
    if not isinstance(v, list) or not all(isinstance(x, str) for x in v):
        raise ValueError(f"not a string list: {v!r}")
    return v


def validate(c: dict):
    """Typed validation of every known key; malformed shapes fail closed."""
    for key, v in c.items():
        if key not in KNOWN:
            raise ValueError(f"unknown constraint key {key!r}")
        if key in ("max_actions", "max_children", "max_sessions"):
            u64(v)
        elif key == "max_actions_per":
            dur_secs(v["window"]); u64(v["n"])
        elif key == "rate_limit":
            if not isinstance(v.get("action"), str) or not v["action"]:
                raise ValueError("rate_limit needs an action")
            dur_secs(v["window"]); u64(v["n"])
        elif key in ("log_reads", "disclose_agency", "first_party_only"):
            if v is not True:
                raise ValueError(f"{key} must be true when present")
        elif key in ("domains", "counter_sign", "binding", "notify"):
            str_list(v)
        elif key in ("purpose", "session_bind"):
            if not isinstance(v, str) or not v:
                raise ValueError(f"{key} must be a non-empty string")
        elif key == "heartbeat":
            dur_secs(v["every"]); dur_secs(v["grace"])
        elif key == "freshness":
            dur_secs(v)
        elif key == "spend_cap":
            if not isinstance(v.get("unit"), str) or not v["unit"]:
                raise ValueError("spend_cap needs a unit")
            u64(v["amount"])
        elif key == "action_params":
            for action, preds in v.items():
                if not isinstance(preds, dict):
                    raise ValueError(f"action_params[{action}] must be an object")
                for pk, pv in preds.items():
                    if pk not in KNOWN_PREDICATES:
                        raise ValueError(f"unknown predicate {pk!r}")
                    if pk == "recipients_allow":
                        str_list(pv)
                    elif pk == "no_attachments" and pv is not True:
                        raise ValueError("no_attachments must be true")
        elif key == "budgets":
            ids = set()
            for p in v:
                if not isinstance(p.get("id"), str) or not p["id"]:
                    raise ValueError("budget profile without id")
                if p["id"] in ids:
                    raise ValueError("duplicate budget profile id")
                ids.add(p["id"])
                if "token_budget" in p:
                    u64(p["token_budget"])
                if "max_actions" in p:
                    u64(p["max_actions"])
                if "models" in p:
                    str_list(p["models"])
        # active_windows / obligations shapes are pinned by F+ and §04.12.


def attenuates(parent: dict, child: dict) -> bool:
    """True iff the child only tightens the parent — fail-closed."""
    try:
        validate(parent)
        validate(child)
    except ValueError:
        return False
    for key, pv in parent.items():
        cv = child.get(key)
        if cv is None:
            if key in DROPPABLE:
                continue
            return False
        try:
            if key in ("max_actions", "max_children", "max_sessions"):
                if cv > pv:
                    return False
            elif key == "max_actions_per":
                if cv["n"] > pv["n"] or dur_secs(cv["window"]) < dur_secs(pv["window"]):
                    return False
            elif key == "rate_limit":
                if (cv["action"] != pv["action"] or cv["n"] > pv["n"]
                        or dur_secs(cv["window"]) < dur_secs(pv["window"])):
                    return False
            elif key == "domains":
                if not set(cv) <= set(pv):
                    return False
            elif key == "notify":
                if not set(cv) >= set(pv):
                    return False
            elif key in ("counter_sign", "binding"):
                if not set(cv) >= set(pv):
                    return False
            elif key == "obligations":
                pj = [jcs(o) for o in pv]
                cj = [jcs(o) for o in cv]
                if any(o not in cj for o in pj):
                    return False
            elif key in ("log_reads", "disclose_agency", "first_party_only"):
                pass  # both validated true
            elif key in ("purpose", "session_bind"):
                if cv != pv:
                    return False
            elif key == "heartbeat":
                if (dur_secs(cv["every"]) > dur_secs(pv["every"])
                        or dur_secs(cv["grace"]) > dur_secs(pv["grace"])):
                    return False
            elif key == "freshness":
                if dur_secs(cv) > dur_secs(pv):
                    return False
            elif key == "spend_cap":
                if cv["unit"] != pv["unit"] or cv["amount"] > pv["amount"]:
                    return False
            elif key == "action_params":
                for action, ppreds in pv.items():
                    cpreds = cv.get(action)
                    if cpreds is None:
                        return False
                    for pk, ppv in ppreds.items():
                        cpv = cpreds.get(pk)
                        if pk == "recipients_allow":
                            if cpv is None or not set(cpv) <= set(ppv):
                                return False
                        elif pk == "no_attachments":
                            if cpv is not True:
                                return False
            elif key == "budgets":
                pids = {p["id"]: p for p in pv}
                for cp in cv:
                    pp = pids.get(cp["id"])
                    if pp is None:
                        return False
                    for cap in ("token_budget", "max_actions"):
                        if cap in pp:
                            if cap not in cp or cp[cap] > pp[cap]:
                                return False
                    if "models" in pp:
                        if "models" not in cp or not set(cp["models"]) <= set(pp["models"]):
                            return False
                    if pp.get("require_attestation"):
                        if not cp.get("require_attestation"):
                            return False
                        if pp.get("attestation_key") != cp.get("attestation_key"):
                            return False
            elif key == "active_windows":
                # Containment arithmetic is pinned by F+; the matrix keeps
                # its cases window-free on purpose.
                raise ValueError("active_windows stays out of this matrix")
        except (KeyError, TypeError, ValueError):
            return False
    return True


# ------------------------------------------------------------------ matrix

def case(family, name, parent, child, expected=None):
    verdict = VALID if attenuates(parent, child) else INVALID
    if expected is not None:
        assert verdict == expected, (family, name, verdict, expected)
    return {"family": family, "case": name, "parent": parent, "child": child,
            "expected": verdict}


def build_matrix():
    ap = lambda w, n: {"max_actions_per": {"window": w, "n": n}}
    rl = lambda a, w, n: {"rate_limit": {"action": a, "window": w, "n": n}}
    hb = lambda e, g: {"heartbeat": {"every": e, "grace": g}}
    sc = lambda u, a: {"spend_cap": {"unit": u, "amount": a}}
    llm = lambda t: {"budgets": [{"id": "llm", "token_budget": t}]}
    reply_allow = lambda who: {"action_params": {"reply": {"recipients_allow": who}}}

    m = [
        # --- the committed 26-scenario matrix (f-plus-constraints, M1) ---
        case("max_actions", "lower cap accepted", {"max_actions": 10}, {"max_actions": 5}, VALID),
        case("max_actions", "higher cap refused", {"max_actions": 10}, {"max_actions": 20}, INVALID),
        case("max_actions_per", "lower rolling cap accepted", ap("24h", 5), ap("24h", 2), VALID),
        case("max_actions_per", "higher rolling cap refused", ap("24h", 5), ap("24h", 9), INVALID),
        case("rate_limit", "lower per-action cap accepted", rl("reply", "1h", 5), rl("reply", "1h", 2), VALID),
        case("rate_limit", "higher per-action cap refused", rl("reply", "1h", 5), rl("reply", "1h", 8), INVALID),
        case("max_children", "narrower width accepted", {"max_children": 4}, {"max_children": 1}, VALID),
        case("max_children", "wider width refused", {"max_children": 4}, {"max_children": 6}, INVALID),
        case("max_sessions", "fewer sessions accepted", {"max_sessions": 2}, {"max_sessions": 1}, VALID),
        case("max_sessions", "more sessions refused", {"max_sessions": 2}, {"max_sessions": 3}, INVALID),
        case("domains", "included allow-list accepted",
             {"domains": ["a.example", "b.example"]}, {"domains": ["a.example"]}, VALID),
        case("domains", "extended allow-list refused",
             {"domains": ["a.example"]}, {"domains": ["a.example", "c.example"]}, INVALID),
        case("budgets", "lower token budget accepted", llm(10000), llm(4000), VALID),
        case("budgets", "higher token budget refused", llm(10000), llm(40000), INVALID),
        case("heartbeat", "tighter beacon accepted", hb("24h", "6h"), hb("12h", "3h"), VALID),
        case("heartbeat", "looser beacon refused", hb("24h", "6h"), hb("48h", "6h"), INVALID),
        case("freshness", "fresher bound accepted", {"freshness": "24h"}, {"freshness": "1h"}, VALID),
        case("freshness", "staler bound refused", {"freshness": "1h"}, {"freshness": "24h"}, INVALID),
        case("spend_cap", "lower cap accepted", sc("eur", 100), sc("eur", 40), VALID),
        case("spend_cap", "higher cap refused", sc("eur", 100), sc("eur", 200), INVALID),
        case("domains", "dropping the family refused", {"domains": ["a.example"]}, {}, INVALID),
        case("first_party_only", "dropping the duty refused", {"first_party_only": True}, {}, INVALID),
        case("counter_sign", "growing the set accepted",
             {"counter_sign": ["reply"]}, {"counter_sign": ["reply", "send"]}, VALID),
        case("counter_sign", "dropping an action refused",
             {"counter_sign": ["reply"]}, {"counter_sign": []}, INVALID),
        case("action_params", "narrowed recipients accepted",
             reply_allow(["alice@example.com", "bob@example.com"]),
             reply_allow(["alice@example.com"]), VALID),
        case("action_params", "added recipient refused",
             reply_allow(["alice@example.com", "bob@example.com"]),
             reply_allow(["alice@example.com", "bob@example.com", "mallory@evil.example"]),
             INVALID),
        case("unknown", "copy-through of an unknown key refused",
             {"quantum_cap": 4}, {"quantum_cap": 4}, INVALID),
        case("unknown", "child inventing an unknown key refused",
             {}, {"quantum_cap": 4}, INVALID),

        # --- drop semantics per family (the reconciliation, pinned) ---
        case("max_actions", "drop tolerated — subtree-counted",
             {"max_actions": 10}, {}, VALID),
        case("max_actions_per", "drop tolerated — subtree-counted", ap("24h", 5), {}, VALID),
        case("rate_limit", "drop tolerated — subtree-counted", rl("reply", "1h", 5), {}, VALID),
        case("max_children", "drop tolerated — per-level width", {"max_children": 4}, {}, VALID),
        case("budgets", "drop tolerated — chain-conjoined at append", llm(10000), {}, VALID),
        case("heartbeat", "drop tolerated — chain-conjoined at append", hb("24h", "6h"), {}, VALID),
        case("max_sessions", "drop refused — per-grantee, never subtree",
             {"max_sessions": 2}, {}, INVALID),
        case("freshness", "drop refused", {"freshness": "24h"}, {}, INVALID),
        case("spend_cap", "drop refused", sc("eur", 100), {}, INVALID),
        case("log_reads", "drop refused", {"log_reads": True}, {}, INVALID),
        case("disclose_agency", "drop refused", {"disclose_agency": True}, {}, INVALID),
        case("notify", "drop refused", {"notify": ["refusal"]}, {}, INVALID),
        case("purpose", "drop refused", {"purpose": "prospect emails"}, {}, INVALID),
        case("session_bind", "drop refused",
             {"session_bind": "z6MkSession"}, {}, INVALID),
        case("action_params", "drop refused",
             reply_allow(["alice@example.com"]), {}, INVALID),

        # --- remaining family rules ---
        case("log_reads", "kept duty accepted", {"log_reads": True}, {"log_reads": True}, VALID),
        case("disclose_agency", "kept duty accepted",
             {"disclose_agency": True}, {"disclose_agency": True}, VALID),
        case("notify", "grown event set accepted",
             {"notify": ["refusal"]}, {"notify": ["refusal", "action"]}, VALID),
        case("notify", "shrunk event set refused",
             {"notify": ["refusal", "action"]}, {"notify": ["refusal"]}, INVALID),
        case("purpose", "identical statement accepted",
             {"purpose": "prospect emails"}, {"purpose": "prospect emails"}, VALID),
        case("purpose", "reworded statement refused",
             {"purpose": "prospect emails"}, {"purpose": "any emails"}, INVALID),
        case("session_bind", "identical key accepted",
             {"session_bind": "z6MkSession"}, {"session_bind": "z6MkSession"}, VALID),
        case("session_bind", "changed key refused",
             {"session_bind": "z6MkSession"}, {"session_bind": "z6MkOther"}, INVALID),
        case("first_party_only", "kept duty accepted",
             {"first_party_only": True}, {"first_party_only": True}, VALID),
        case("max_actions_per", "longer window, same cap accepted",
             ap("24h", 5), ap("48h", 5), VALID),
        case("max_actions_per", "shorter window refused — more slots inside the parent's",
             ap("24h", 5), ap("12h", 5), INVALID),
        case("rate_limit", "retargeted action refused — the parent's limit is dropped",
             rl("reply", "1h", 5), rl("send", "1h", 2), INVALID),
        case("spend_cap", "changed unit refused — incomparable",
             sc("eur", 100), sc("usd", 40), INVALID),
        case("budgets", "narrowed model list accepted",
             {"budgets": [{"id": "llm", "models": ["haiku", "gemma"], "token_budget": 10000}]},
             {"budgets": [{"id": "llm", "models": ["haiku"], "token_budget": 10000}]}, VALID),
        case("budgets", "widened model list refused",
             {"budgets": [{"id": "llm", "models": ["haiku"], "token_budget": 10000}]},
             {"budgets": [{"id": "llm", "models": ["haiku", "gpt-oss"], "token_budget": 10000}]},
             INVALID),
        case("budgets", "dropped profile-level cap refused",
             llm(10000), {"budgets": [{"id": "llm"}]}, INVALID),
        case("budgets", "unknown child profile id refused",
             llm(10000), {"budgets": [{"id": "llm", "token_budget": 100},
                                      {"id": "shadow", "token_budget": 100}]}, INVALID),
        case("budgets", "profile subset accepted",
             {"budgets": [{"id": "llm", "token_budget": 10000},
                          {"id": "haiku", "token_budget": 500}]},
             llm(4000), VALID),
        case("budgets", "dropped attestation duty refused",
             {"budgets": [{"id": "llm", "require_attestation": True,
                           "attestation_key": "z6MkProvider"}]},
             {"budgets": [{"id": "llm"}]}, INVALID),
        case("obligations", "inherited obligation kept plus one added accepted",
             {"obligations": [{"id": "guard", "check": "pii.scan",
                               "attestor": ["z6MkGuard"],
                               "applies_to": "act.x.gmail.reply",
                               "verdict": "pass"}]},
             {"obligations": [{"id": "guard", "check": "pii.scan",
                               "attestor": ["z6MkGuard"],
                               "applies_to": "act.x.gmail.reply",
                               "verdict": "pass"},
                              {"id": "human", "check": "human.approve",
                               "attestor": ["z6MkBoss"],
                               "applies_to": "act.x.gmail.send",
                               "verdict": "approve"}]}, VALID),
        case("obligations", "altered inherited obligation refused",
             {"obligations": [{"id": "guard", "check": "pii.scan",
                               "attestor": ["z6MkGuard"],
                               "applies_to": "act.x.gmail.reply",
                               "verdict": "pass"}]},
             {"obligations": [{"id": "guard", "check": "pii.scan",
                               "attestor": ["z6MkEvil"],
                               "applies_to": "act.x.gmail.reply",
                               "verdict": "pass"}]}, INVALID),
        case("action_params", "added action predicates accepted",
             reply_allow(["alice@example.com"]),
             {"action_params": {"reply": {"recipients_allow": ["alice@example.com"]},
                                "send": {"recipients_allow": ["alice@example.com"]}}}, VALID),
        case("action_params", "dropped no_attachments refused",
             {"action_params": {"reply": {"recipients_allow": ["alice@example.com"],
                                          "no_attachments": True}}},
             reply_allow(["alice@example.com"]), INVALID),
        case("action_params", "unknown predicate refused",
             reply_allow(["alice@example.com"]),
             {"action_params": {"reply": {"recipients_allow": ["alice@example.com"],
                                          "quantum_filter": True}}}, INVALID),
        case("validation", "malformed known key refused",
             {"max_actions": 10}, {"max_actions": "ten"}, INVALID),
        case("validation", "malformed parent key refused at the link",
             {"heartbeat": {"every": "soon", "grace": "6h"}},
             {"heartbeat": {"every": "12h", "grace": "3h"}}, INVALID),
        case("tightening", "child introducing a family the parent lacks accepted",
             {}, {"domains": ["a.example"]}, VALID),
        case("tightening", "empty to empty accepted", {}, {}, VALID),
    ]
    return m


# ------------------------------------------------------------- signed chain

NB_PARENT = "2026-07-01T00:00:00Z"
NA_PARENT = "2026-07-31T00:00:00Z"
NB_CHILD = "2026-07-02T00:00:00Z"
NA_CHILD = "2026-07-08T00:00:00Z"
T = "2026-07-03T00:00:00Z"

PARENT_CONSTRAINTS = {"max_actions": 10, "domains": ["a.example", "b.example"]}
CHILD_OK_CONSTRAINTS = {"max_actions": 5, "domains": ["a.example"]}
CHILD_BAD_CONSTRAINTS = {"max_actions": 20, "domains": ["a.example"]}


def sign_doc(doc: dict, sk: SigningKey) -> dict:
    doc = dict(doc)
    doc["signature"] = dict(doc["signature"], value="")
    sig = sk.sign(jcs(doc).encode()).signature
    doc["signature"] = dict(doc["signature"], value=sig.hex())
    return doc


def mandate(mid, subject, parent, issued_by, sig_key, grantee_label, grantee_sk,
            perimeter, constraints, nb, na, nonce, signer):
    doc = {
        "aithos-mandate-core": "1.0.0-draft.1",
        "id": mid,
        "subject": subject,
        "parent": parent,
        "issued_by": issued_by,
        "grantee": {
            "id": f"urn:aithos:agent:{grantee_label}",
            "label": grantee_label,
            "pubkey": multibase_ed(bytes(grantee_sk.verify_key)),
            "kex_pubkey": kex_of(grantee_sk),
        },
        "perimeter": perimeter,
        "constraints": constraints,
        "not_before": nb,
        "not_after": na,
        "issued_at": nb,
        "nonce": nonce,
        "signature": {"alg": "ed25519", "key": sig_key, "value": ""},
    }
    return sign_doc(doc, signer)


def build_signed_chain():
    root_sk = SigningKey(derive("aithos-core/v1/root-sign", SEED))
    agent = SigningKey(AGENT_SK)
    helper = SigningKey(HELPER_SK)
    did = "did:aithos:" + multibase_ed(bytes(root_sk.verify_key))
    agent_pub_mb = multibase_ed(bytes(agent.verify_key))

    parent = mandate(
        "mandate_000000000000000000000000EP", did, None, f"{did}#root", "#root",
        "agent", agent, ["act.x.gmail.*", "issue#depth=1"], PARENT_CONSTRAINTS,
        NB_PARENT, NA_PARENT, "000102030405060708090a0b0c0d0e0f", root_sk,
    )
    child_ok = mandate(
        "mandate_00000000000000000000000EPA", did, parent["id"], agent_pub_mb,
        agent_pub_mb, "helper", helper, ["act.x.gmail.reply"],
        CHILD_OK_CONSTRAINTS, NB_CHILD, NA_CHILD,
        "101112131415161718191a1b1c1d1e1f", agent,
    )
    child_bad = mandate(
        "mandate_00000000000000000000000EPB", did, parent["id"], agent_pub_mb,
        agent_pub_mb, "helper", helper, ["act.x.gmail.reply"],
        CHILD_BAD_CONSTRAINTS, NB_CHILD, NA_CHILD,
        "202122232425262728292a2b2c2d2e2f", agent,
    )

    assert attenuates(PARENT_CONSTRAINTS, CHILD_OK_CONSTRAINTS)
    assert not attenuates(PARENT_CONSTRAINTS, CHILD_BAD_CONSTRAINTS)

    return {
        "seed_hex": SEED.hex(),
        "agent_sk_hex": AGENT_SK.hex(),
        "helper_sk_hex": HELPER_SK.hex(),
        "succession_entropy_hex": "09" * 32,
        "did": did,
        "at": T,
        "parent_jcs": jcs(parent),
        "parent_signature_hex": parent["signature"]["value"],
        "child_ok_jcs": jcs(child_ok),
        "child_ok_signature_hex": child_ok["signature"]["value"],
        "child_bad_jcs": jcs(child_bad),
        "child_bad_signature_hex": child_bad["signature"]["value"],
        "expected": {
            "parent_alone_at_T": VALID,
            "chain_child_ok_at_T": VALID,
            "chain_child_bad_at_T": INVALID,
        },
    }


if __name__ == "__main__":
    matrix = build_matrix()
    valid = sum(1 for c in matrix if c["expected"] == VALID)
    out = {
        "vector": "E+",
        "description": "Typed constraint attenuation per family at a delegation "
                       "link (spec 05.3 rule 3; M0 decision (c) 2026-07-16: unknown "
                       "keys fail closed, no copy-through). Matrix verdicts and the "
                       "signed owner→agent→helper chain generated independently "
                       "(Python sets/integers + blake3 + PyNaCl + base58). Drop "
                       "semantics pinned: subtree/chain-conjoined counting families "
                       "(max_actions, max_actions_per, rate_limit, max_children, "
                       "budgets, heartbeat) may be dropped by a child — the "
                       "consumption engine still conjoins every ancestor (F step); "
                       "every other family dropped at a link is a rejection.",
        "matrix": matrix,
        "signed_chain": build_signed_chain(),
    }
    with open("eplus-attenuation.json", "w") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"wrote eplus-attenuation.json — {len(matrix)} matrix cases "
          f"({valid} valid, {len(matrix) - valid} rejected), 1 signed chain")

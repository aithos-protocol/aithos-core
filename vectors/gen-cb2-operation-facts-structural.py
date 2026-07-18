#!/usr/bin/env python3
"""Independent CB2 oracle for K1.2-GRRP-B structural operation facts.

The standard-library-only oracle covers grant, revoke, standalone rotate, and
publication facts. It consumes the frozen E1 complete signed mandate as the
certificate fixture, independently derives all commitments, and rejects
mono-defect candidates with InvalidOperationFacts.

The changeset document is a syntactic digest fixture because its exact member
table remains a later gate. This vector proves only the already-approved
changeset_ref table and digest domain.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Callable, NoReturn


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-operation-facts-structural.json"

PROFILE_KEY = "aithos-operation-facts-core"
PROFILE = "1.0.0-draft.1"
STATE_PROFILE_KEY = "aithos-state-fact-core"
STATE_PROFILE = "1.0.0-draft.1"
CHANGESET_PROFILE_KEY = "aithos-changeset-core"
CHANGESET_PROFILE = "1.0.0-draft.1"
OPERATION_REF_PROFILE_KEY = "aithos-operation-core"
OPERATION_REF_PROFILE = "1.0.0-draft.1"
INVALID_OPERATION_FACTS = "InvalidOperationFacts"

DOMAINS = {
    "operation_facts": "aithos-core/v1/operation-facts",
    "state_key": "aithos-core/v1/state-key",
    "state_bytes": "aithos-core/v1/state-bytes",
    "state_fact": "aithos-core/v1/state-fact",
    "changeset": "aithos-core/v1/changeset",
}

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
HEX_RE = re.compile(r"^[0-9a-f]{64}$")
SID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
MANDATE_ID_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")
OCCURRENCE_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
DID_RE = re.compile(r"^did:aithos:z[1-9A-HJ-NP-Za-km-z]+$")
TOKEN_RE = re.compile(r"^[a-z][a-z0-9._-]*$")

HISTORICAL_FILES = (
    "e1-mandate.json",
    "f1-gamma-chain.json",
    "f2-gamma-counting.json",
    "g1-revocation.json",
)


class OracleError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_OPERATION_FACTS


def reject(detail: str) -> NoReturn:
    raise OracleError(detail)


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


def require_exact_object(
    value: Any,
    expected: set[str],
    label: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        reject(f"{label} is not an object")
    if set(value) != expected:
        reject(f"{label} has a non-exact member set")
    if any(member is None for member in value.values()):
        reject(f"{label} contains null")
    return value


def require_commitment(value: Any, label: str) -> str:
    if not isinstance(value, str) or not COMMITMENT_RE.fullmatch(value):
        reject(f"{label} is not strict lowercase sha256 text")
    return value


def sid(number: int) -> str:
    value = f"01J{number:023d}"
    assert SID_RE.fullmatch(value)
    return value


def occurrence(number: int) -> str:
    value = f"01K{number:023d}"
    assert len(value) == 26
    return f"op_{value}"


def operation_ref(number: int) -> dict[str, Any]:
    return {
        OPERATION_REF_PROFILE_KEY: OPERATION_REF_PROFILE,
        "occurrence": occurrence(number),
        "commitment": commitment(
            "aithos-core/v1/operation-commitment",
            f"fixture-operation-{number}".encode("ascii"),
        ),
    }


def state_fixture(label: str, keys: list[str]) -> dict[str, Any]:
    objects = []
    for index, key in enumerate(keys):
        objects.append(
            {
                "key_commitment": commitment(
                    DOMAINS["state_key"],
                    key.encode("utf-8"),
                ),
                "byte_commitment": commitment(
                    DOMAINS["state_bytes"],
                    f"{label}-bytes-{index}".encode("utf-8"),
                ),
            }
        )
    objects.sort(key=lambda item: item["key_commitment"])
    document = {
        STATE_PROFILE_KEY: STATE_PROFILE,
        "objects": objects,
    }
    document_jcs = jcs(document)
    digest = commitment(DOMAINS["state_fact"], document_jcs.encode("utf-8"))
    return {
        "document": document,
        "document_jcs": document_jcs,
        "digest": digest,
        "state": {
            "state": "present",
            "state_ref": {
                STATE_PROFILE_KEY: STATE_PROFILE,
                "digest": digest,
            },
        },
    }


def certificate_fixture() -> dict[str, Any]:
    e1 = json.loads((HERE / "e1-mandate.json").read_text())
    certificate = json.loads(e1["mandate_jcs"])
    assert jcs(certificate) == e1["mandate_jcs"]
    return {
        "document": certificate,
        "document_jcs": e1["mandate_jcs"],
        "mandate_id": certificate["id"],
        "certificate_digest": sha256_text(
            e1["mandate_jcs"].encode("utf-8")
        ),
    }


def fixtures() -> dict[str, Any]:
    before = state_fixture(
        "before",
        ["e/circle/header.json", "e/circle/index.json"],
    )
    after = state_fixture(
        "after",
        ["e/circle/header.json", "e/circle/index.json"],
    )
    identity_before = state_fixture("identity-before", ["did.json"])
    identity_after = state_fixture("identity-after", ["did.json"])
    previous_did = "did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr"
    next_did = "did:aithos:z6Mkr7dKpR4rhcMqPz6yKQMN8fwcM9gA7D8Xw8P9YwP7nabc"
    transition = {
        "aithos-epoch-core": "1.0.0-draft.1",
        "prev_did": previous_did,
        "next_did": next_did,
        "at": "2026-07-18T12:00:00Z",
        "signature": {
            "alg": "ed25519",
            "key": "#succession",
            "value": "33" * 64,
        },
    }
    parents = [
        sha256_text(b"parent-a"),
        sha256_text(b"parent-b"),
    ]
    parents.sort()
    contained = [operation_ref(1), operation_ref(2)]
    changeset_document = {
        CHANGESET_PROFILE_KEY: CHANGESET_PROFILE,
        "fixture_parents": parents,
        "fixture_operations": contained,
        "fixture_consequences": [
            sha256_text(b"changed-object-a"),
            sha256_text(b"changed-object-b"),
        ],
    }
    changeset_jcs = jcs(changeset_document)
    changeset_digest = commitment(
        DOMAINS["changeset"],
        changeset_jcs.encode("utf-8"),
    )
    return {
        "certificate": certificate_fixture(),
        "reason": "device_lost",
        "state_before": before,
        "state_after": after,
        "identity_before": identity_before,
        "identity_after": identity_after,
        "identity_transition": {
            "document": transition,
            "document_jcs": jcs(transition),
            "digest": sha256_text(jcs(transition).encode("utf-8")),
        },
        "previous_did": previous_did,
        "next_did": next_did,
        "zone": "circle",
        "sid": sid(501),
        "connector": "mail",
        "parents": parents,
        "contained_operations": contained,
        "changeset": {
            "document": changeset_document,
            "document_jcs": changeset_jcs,
            "digest": changeset_digest,
            "ref": {
                CHANGESET_PROFILE_KEY: CHANGESET_PROFILE,
                "digest": changeset_digest,
            },
            "syntactic_document_only": True,
        },
    }


def operation_case(
    case_id: str,
    kind: str,
    facts: dict[str, Any],
    context: dict[str, Any],
) -> dict[str, Any]:
    document = {
        PROFILE_KEY: PROFILE,
        "kind": kind,
        "facts": facts,
    }
    document_jcs = jcs(document)
    digest = commitment(
        DOMAINS["operation_facts"],
        document_jcs.encode("utf-8"),
    )
    return {
        "id": case_id,
        "kind": kind,
        "facts": facts,
        "context": context,
        "document": document,
        "document_jcs": document_jcs,
        "digest": digest,
        "facts_ref": {
            PROFILE_KEY: PROFILE,
            "digest": digest,
        },
    }


def build_positive_cases(f: dict[str, Any]) -> list[dict[str, Any]]:
    certificate = f["certificate"]
    base_certificate = {
        "mandate_id": certificate["mandate_id"],
        "certificate_digest": certificate["certificate_digest"],
    }
    certificate_context = {
        "certificate": certificate["document"],
    }
    before = f["state_before"]["state"]
    after = f["state_after"]["state"]
    identity_before = f["identity_before"]["state"]
    identity_after = f["identity_after"]["state"]
    common_rotate_context = {
        "before": before,
        "after": after,
        "derived": False,
    }
    change_ref = f["changeset"]["ref"]
    contained = f["contained_operations"]
    parents = f["parents"]

    cases = [
        operation_case(
            "grant",
            "grant",
            clone(base_certificate),
            clone(certificate_context),
        ),
        operation_case(
            "revoke-no-reason",
            "revoke",
            {**clone(base_certificate), "reason": {"state": "absent"}},
            {
                **clone(certificate_context),
                "native_reason": None,
            },
        ),
        operation_case(
            "revoke-with-reason",
            "revoke",
            {
                **clone(base_certificate),
                "reason": {
                    "state": "present",
                    "text": f["reason"],
                },
            },
            {
                **clone(certificate_context),
                "native_reason": f["reason"],
            },
        ),
        operation_case(
            "rotate-ethos-zone",
            "rotate",
            {
                "domain": "ethos-zone",
                "zone": f["zone"],
                "mode": "rotate",
                "before": clone(before),
                "after": clone(after),
            },
            {
                **clone(common_rotate_context),
                "zone": f["zone"],
                "mode": "rotate",
            },
        ),
        operation_case(
            "rotate-ethos-node",
            "rotate",
            {
                "domain": "ethos-node",
                "zone": f["zone"],
                "sid": f["sid"],
                "mode": "reencrypt",
                "before": clone(before),
                "after": clone(after),
            },
            {
                **clone(common_rotate_context),
                "zone": f["zone"],
                "sid": f["sid"],
                "mode": "reencrypt",
            },
        ),
        operation_case(
            "rotate-vault",
            "rotate",
            {
                "domain": "vault",
                "connector": f["connector"],
                "mode": "rotate",
                "before": clone(before),
                "after": clone(after),
            },
            {
                **clone(common_rotate_context),
                "connector": f["connector"],
                "mode": "rotate",
            },
        ),
        operation_case(
            "rotate-identity",
            "rotate",
            {
                "domain": "identity",
                "previous_did": f["previous_did"],
                "next_did": f["next_did"],
                "transition_digest": f["identity_transition"]["digest"],
                "before": clone(identity_before),
                "after": clone(identity_after),
            },
            {
                "previous_did": f["previous_did"],
                "next_did": f["next_did"],
                "transition": f["identity_transition"]["document"],
                "before": clone(identity_before),
                "after": clone(identity_after),
                "derived": False,
            },
        ),
        operation_case(
            "publication-genesis",
            "publication",
            {
                "mode": "normal",
                "height": 1,
                "predecessors": [],
                "changeset_ref": clone(change_ref),
                "contained_operations": [],
            },
            {
                "height": 1,
                "predecessors": [],
                "changeset_ref": clone(change_ref),
                "contained_operations": [],
            },
        ),
        operation_case(
            "publication-normal",
            "publication",
            {
                "mode": "normal",
                "height": 2,
                "predecessors": [parents[0]],
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
            {
                "height": 2,
                "predecessors": [parents[0]],
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
        ),
        operation_case(
            "publication-merge",
            "publication",
            {
                "mode": "merge",
                "height": 3,
                "predecessors": clone(parents),
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
            {
                "height": 3,
                "predecessors": clone(parents),
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
        ),
        operation_case(
            "publication-resolution",
            "publication",
            {
                "mode": "resolution",
                "height": 3,
                "predecessors": clone(parents),
                "winner": parents[1],
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
            {
                "height": 3,
                "predecessors": clone(parents),
                "winner": parents[1],
                "changeset_ref": clone(change_ref),
                "contained_operations": clone(contained),
            },
        ),
    ]
    return cases


def validate_certificate_fields(
    facts: dict[str, Any],
    context: dict[str, Any],
) -> None:
    mandate_id = facts["mandate_id"]
    if not isinstance(mandate_id, str) or not MANDATE_ID_RE.fullmatch(mandate_id):
        reject("mandate_id is not canonical")
    digest = require_commitment(
        facts["certificate_digest"],
        "certificate_digest",
    )
    certificate = context["certificate"]
    if mandate_id != certificate.get("id"):
        reject("mandate id and certificate mismatch")
    expected = sha256_text(jcs(certificate).encode("utf-8"))
    if digest != expected:
        reject("certificate digest mismatch")


def validate_grant(facts: Any, context: dict[str, Any]) -> None:
    value = require_exact_object(
        facts,
        {"mandate_id", "certificate_digest"},
        "grant facts",
    )
    validate_certificate_fields(value, context)


def validate_revoke(facts: Any, context: dict[str, Any]) -> None:
    value = require_exact_object(
        facts,
        {"mandate_id", "certificate_digest", "reason"},
        "revoke facts",
    )
    validate_certificate_fields(value, context)
    reason = value["reason"]
    if not isinstance(reason, dict):
        reject("reason is not an object")
    if reason.get("state") == "absent":
        require_exact_object(reason, {"state"}, "absent reason")
        if context["native_reason"] is not None:
            reject("native reason was omitted")
    elif reason.get("state") == "present":
        present = require_exact_object(reason, {"state", "text"}, "present reason")
        if not isinstance(present["text"], str) or not present["text"]:
            reject("reason text is empty")
        if present["text"] != context["native_reason"]:
            reject("native reason mismatch")
    else:
        reject("unknown reason state")


def validate_present_state(
    value: Any,
    expected: dict[str, Any],
    label: str,
) -> None:
    state = require_exact_object(
        value,
        {"state", "state_ref"},
        label,
    )
    if state["state"] != "present":
        reject(f"{label} is not present")
    reference = require_exact_object(
        state["state_ref"],
        {STATE_PROFILE_KEY, "digest"},
        f"{label} state_ref",
    )
    if reference[STATE_PROFILE_KEY] != STATE_PROFILE:
        reject(f"{label} has unknown state profile")
    require_commitment(reference["digest"], f"{label} digest")
    if state != expected:
        reject(f"{label} does not match native state")


def validate_rotate(facts: Any, context: dict[str, Any]) -> None:
    if not isinstance(facts, dict):
        reject("rotate facts is not an object")
    domain = facts.get("domain")
    expected_keys = {
        "ethos-zone": {"domain", "zone", "mode", "before", "after"},
        "ethos-node": {"domain", "zone", "sid", "mode", "before", "after"},
        "vault": {"domain", "connector", "mode", "before", "after"},
        "identity": {
            "domain",
            "previous_did",
            "next_did",
            "transition_digest",
            "before",
            "after",
        },
    }.get(domain)
    if expected_keys is None:
        reject("unknown rotate domain")
    value = require_exact_object(facts, expected_keys, "rotate facts")
    if context.get("derived"):
        reject("derived rotation has a second rotate occurrence")
    if domain != "identity":
        if value["mode"] not in {"rotate", "reencrypt"}:
            reject("unknown rotate mode")
        if value["mode"] != context["mode"]:
            reject("native rotate mode mismatch")
    if domain in {"ethos-zone", "ethos-node"}:
        if value["zone"] not in {"public", "circle", "self"}:
            reject("unknown rotate zone")
        if value["zone"] != context["zone"]:
            reject("native rotate zone mismatch")
    if domain == "ethos-node":
        if not isinstance(value["sid"], str) or not SID_RE.fullmatch(value["sid"]):
            reject("rotate SID is not canonical")
        if value["sid"] != context["sid"]:
            reject("native rotate SID mismatch")
    if domain == "vault":
        if (
            not isinstance(value["connector"], str)
            or not TOKEN_RE.fullmatch(value["connector"])
            or value["connector"] != context["connector"]
        ):
            reject("native vault connector mismatch")
    if domain == "identity":
        if (
            not isinstance(value["previous_did"], str)
            or not DID_RE.fullmatch(value["previous_did"])
            or not isinstance(value["next_did"], str)
            or not DID_RE.fullmatch(value["next_did"])
            or value["previous_did"] == value["next_did"]
        ):
            reject("identity rotation DIDs are invalid")
        if (
            value["previous_did"] != context["previous_did"]
            or value["next_did"] != context["next_did"]
        ):
            reject("identity rotation DID mismatch")
        transition_digest = require_commitment(
            value["transition_digest"],
            "transition_digest",
        )
        transition = context["transition"]
        if (
            transition.get("prev_did") != value["previous_did"]
            or transition.get("next_did") != value["next_did"]
            or transition_digest
            != sha256_text(jcs(transition).encode("utf-8"))
        ):
            reject("identity transition mismatch")
    validate_present_state(value["before"], context["before"], "rotate before")
    validate_present_state(value["after"], context["after"], "rotate after")
    if value["before"]["state_ref"]["digest"] == value["after"]["state_ref"]["digest"]:
        reject("rotate before and after state digests are equal")


def validate_operation_ref(value: Any) -> str:
    reference = require_exact_object(
        value,
        {OPERATION_REF_PROFILE_KEY, "occurrence", "commitment"},
        "contained operation_ref",
    )
    if reference[OPERATION_REF_PROFILE_KEY] != OPERATION_REF_PROFILE:
        reject("contained operation_ref has unknown profile")
    if (
        not isinstance(reference["occurrence"], str)
        or not OCCURRENCE_RE.fullmatch(reference["occurrence"])
    ):
        reject("contained operation occurrence is malformed")
    require_commitment(reference["commitment"], "contained operation commitment")
    return reference["occurrence"]


def validate_publication(facts: Any, context: dict[str, Any]) -> None:
    if not isinstance(facts, dict):
        reject("publication facts is not an object")
    mode = facts.get("mode")
    if mode in {"normal", "merge"}:
        expected_keys = {
            "mode",
            "height",
            "predecessors",
            "changeset_ref",
            "contained_operations",
        }
    elif mode == "resolution":
        expected_keys = {
            "mode",
            "height",
            "predecessors",
            "winner",
            "changeset_ref",
            "contained_operations",
        }
    else:
        reject("unknown publication mode")
    value = require_exact_object(facts, expected_keys, "publication facts")
    height = value["height"]
    if not isinstance(height, int) or isinstance(height, bool) or height < 1:
        reject("publication height is invalid")
    predecessors = value["predecessors"]
    if not isinstance(predecessors, list):
        reject("publication predecessors is not an array")
    for predecessor in predecessors:
        require_commitment(predecessor, "publication predecessor")
    if len(predecessors) != len(set(predecessors)):
        reject("publication predecessors contain a duplicate")
    if mode == "normal":
        expected_count = 0 if height == 1 else 1
        if len(predecessors) != expected_count:
            reject("normal publication predecessor cardinality mismatch")
    else:
        if height < 3 or len(predecessors) != 2:
            reject("fork publication predecessor cardinality mismatch")
        if predecessors != sorted(predecessors):
            reject("fork publication predecessors are not sorted")
    if mode == "resolution":
        if value["winner"] not in predecessors:
            reject("resolution winner is outside predecessors")
    change_ref = require_exact_object(
        value["changeset_ref"],
        {CHANGESET_PROFILE_KEY, "digest"},
        "changeset_ref",
    )
    if change_ref[CHANGESET_PROFILE_KEY] != CHANGESET_PROFILE:
        reject("unknown changeset profile")
    require_commitment(change_ref["digest"], "changeset digest")
    operations = value["contained_operations"]
    if not isinstance(operations, list):
        reject("contained_operations is not an array")
    occurrences = [validate_operation_ref(item) for item in operations]
    if len(occurrences) != len(set(occurrences)):
        reject("contained operation occurrence is duplicated")
    for field in ("height", "predecessors", "changeset_ref", "contained_operations"):
        if value[field] != context[field]:
            reject(f"publication {field} mismatch")
    if mode == "resolution" and value["winner"] != context["winner"]:
        reject("publication winner mismatch")


def validate_operation(
    document: Any,
    facts_ref: Any | None,
    context: dict[str, Any],
) -> None:
    value = require_exact_object(
        document,
        {PROFILE_KEY, "kind", "facts"},
        "operation-facts document",
    )
    if value[PROFILE_KEY] != PROFILE:
        reject("unknown operation-facts profile")
    validators = {
        "grant": validate_grant,
        "revoke": validate_revoke,
        "rotate": validate_rotate,
        "publication": validate_publication,
    }
    validator = validators.get(value["kind"])
    if validator is None:
        reject("operation kind does not select a structural family")
    validator(value["facts"], context)
    if facts_ref is not None:
        reference = require_exact_object(
            facts_ref,
            {PROFILE_KEY, "digest"},
            "facts_ref",
        )
        if reference[PROFILE_KEY] != value[PROFILE_KEY]:
            reject("facts_ref profile mismatch")
        digest = require_commitment(reference["digest"], "facts_ref digest")
        expected = commitment(
            DOMAINS["operation_facts"],
            jcs(value).encode("utf-8"),
        )
        if digest != expected:
            reject("facts_ref digest mismatch")


def case_by_id(cases: list[dict[str, Any]], case_id: str) -> dict[str, Any]:
    return next(case for case in cases if case["id"] == case_id)


Mutation = Callable[[dict[str, Any], dict[str, Any], dict[str, Any]], None]


def negative_case(
    case_id: str,
    defect: str,
    base: dict[str, Any],
    mutate: Mutation,
) -> dict[str, Any]:
    candidate = clone(base["document"])
    context = clone(base["context"])
    facts_ref = clone(base["facts_ref"])
    original_ref = clone(facts_ref)
    mutate(candidate, context, facts_ref)
    if facts_ref == original_ref:
        profile = candidate.get(PROFILE_KEY)
        if isinstance(profile, str):
            facts_ref = {
                PROFILE_KEY: profile,
                "digest": commitment(
                    DOMAINS["operation_facts"],
                    jcs(candidate).encode("utf-8"),
                ),
            }
        else:
            facts_ref = None
    return {
        "id": case_id,
        "defect": defect,
        "base_case": base["id"],
        "candidate": candidate,
        "facts_ref": facts_ref,
        "context": context,
        "must_fail": INVALID_OPERATION_FACTS,
    }


def build_negative_cases(
    positives: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    grant = case_by_id(positives, "grant")
    revoke_absent = case_by_id(positives, "revoke-no-reason")
    revoke_reason = case_by_id(positives, "revoke-with-reason")
    rotate_zone = case_by_id(positives, "rotate-ethos-zone")
    rotate_node = case_by_id(positives, "rotate-ethos-node")
    rotate_vault = case_by_id(positives, "rotate-vault")
    rotate_identity = case_by_id(positives, "rotate-identity")
    genesis = case_by_id(positives, "publication-genesis")
    normal = case_by_id(positives, "publication-normal")
    merge = case_by_id(positives, "publication-merge")
    resolution = case_by_id(positives, "publication-resolution")

    return [
        negative_case(
            "missing-envelope-profile",
            "missing operation-facts profile",
            grant,
            lambda document, _context, _ref: document.pop(PROFILE_KEY),
        ),
        negative_case(
            "extra-envelope-member",
            "extra operation-facts envelope member",
            grant,
            lambda document, _context, _ref: document.__setitem__("extra", True),
        ),
        negative_case(
            "kind-family-mismatch",
            "grant facts selected as revoke",
            grant,
            lambda document, _context, _ref: document.__setitem__("kind", "revoke"),
        ),
        negative_case(
            "missing-grant-member",
            "grant is missing certificate_digest",
            grant,
            lambda document, _context, _ref: document["facts"].pop(
                "certificate_digest"
            ),
        ),
        negative_case(
            "extra-grant-member",
            "grant has an extra member",
            grant,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "parent", None
            ),
        ),
        negative_case(
            "malformed-mandate-id",
            "grant mandate id is malformed",
            grant,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "mandate_id", "mandate_bad"
            ),
        ),
        negative_case(
            "mismatched-mandate-id",
            "grant mandate id differs from certificate",
            grant,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "mandate_id", "mandate_0000000000000000000000002B"
            ),
        ),
        negative_case(
            "malformed-certificate-digest",
            "certificate digest is malformed",
            grant,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "certificate_digest", "sha256:ABC"
            ),
        ),
        negative_case(
            "mismatched-certificate-digest",
            "certificate digest does not select complete signed bytes",
            grant,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "certificate_digest", sha256_text(b"other certificate")
            ),
        ),
        negative_case(
            "missing-reason",
            "revoke reason variant is missing",
            revoke_absent,
            lambda document, _context, _ref: document["facts"].pop("reason"),
        ),
        negative_case(
            "null-reason",
            "revoke reason is null",
            revoke_absent,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "reason", None
            ),
        ),
        negative_case(
            "empty-reason-text",
            "present revoke reason text is empty",
            revoke_reason,
            lambda document, _context, _ref: document["facts"]["reason"].__setitem__(
                "text", ""
            ),
        ),
        negative_case(
            "reason-view-mismatch",
            "native revoke reason differs",
            revoke_reason,
            lambda _document, context, _ref: context.__setitem__(
                "native_reason", "other_reason"
            ),
        ),
        negative_case(
            "volunteered-reason",
            "reason appears only in operation facts",
            revoke_absent,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "reason", {"state": "present", "text": "device_lost"}
            ),
        ),
        negative_case(
            "unknown-rotate-domain",
            "unknown standalone rotation domain",
            rotate_zone,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "domain", "unknown"
            ),
        ),
        negative_case(
            "missing-rotate-member",
            "rotate target is incomplete",
            rotate_node,
            lambda document, _context, _ref: document["facts"].pop("sid"),
        ),
        negative_case(
            "extra-rotate-member",
            "rotate target has an extra member",
            rotate_zone,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "sid", sid(999)
            ),
        ),
        negative_case(
            "unknown-rotate-mode",
            "unknown standalone rotation mode",
            rotate_zone,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "mode", "rewrap"
            ),
        ),
        negative_case(
            "noncanonical-rotate-sid",
            "rotate SID is not canonical",
            rotate_node,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "sid", "not-a-sid"
            ),
        ),
        negative_case(
            "mismatched-vault-connector",
            "rotate vault connector differs",
            rotate_vault,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "connector", "other"
            ),
        ),
        negative_case(
            "absent-rotate-before",
            "rotate before state is absent",
            rotate_zone,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "before", {"state": "absent"}
            ),
        ),
        negative_case(
            "equal-rotate-states",
            "rotate before and after state digests are equal",
            rotate_zone,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "after", clone(document["facts"]["before"])
            ),
        ),
        negative_case(
            "derived-rotate-double-occurrence",
            "derived rotation is represented as standalone",
            rotate_zone,
            lambda _document, context, _ref: context.__setitem__("derived", True),
        ),
        negative_case(
            "identity-same-did",
            "identity rotation keeps the same DID",
            rotate_identity,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "next_did", document["facts"]["previous_did"]
            ),
        ),
        negative_case(
            "identity-transition-mismatch",
            "identity transition digest mismatches signed transition",
            rotate_identity,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "transition_digest", sha256_text(b"other transition")
            ),
        ),
        negative_case(
            "unknown-publication-mode",
            "unknown publication mode",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "mode", "fork"
            ),
        ),
        negative_case(
            "missing-publication-member",
            "publication is missing changeset_ref",
            normal,
            lambda document, _context, _ref: document["facts"].pop("changeset_ref"),
        ),
        negative_case(
            "extra-publication-member",
            "normal publication carries winner",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "winner", document["facts"]["predecessors"][0]
            ),
        ),
        negative_case(
            "height-zero",
            "publication height is zero",
            genesis,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "height", 0
            ),
        ),
        negative_case(
            "genesis-has-predecessor",
            "genesis has a predecessor",
            genesis,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "predecessors", [sha256_text(b"forbidden parent")]
            ),
        ),
        negative_case(
            "normal-missing-predecessor",
            "non-genesis normal publication has no predecessor",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "predecessors", []
            ),
        ),
        negative_case(
            "merge-one-predecessor",
            "merge has only one predecessor",
            merge,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "predecessors", document["facts"]["predecessors"][:1]
            ),
        ),
        negative_case(
            "merge-unsorted-predecessors",
            "merge predecessors are not sorted",
            merge,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "predecessors", list(reversed(document["facts"]["predecessors"]))
            ),
        ),
        negative_case(
            "merge-duplicate-predecessor",
            "merge predecessor is duplicated",
            merge,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "predecessors",
                [
                    document["facts"]["predecessors"][0],
                    document["facts"]["predecessors"][0],
                ],
            ),
        ),
        negative_case(
            "resolution-winner-outside",
            "resolution winner is outside predecessor set",
            resolution,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "winner", sha256_text(b"outsider")
            ),
        ),
        negative_case(
            "malformed-changeset-ref",
            "changeset reference digest is malformed",
            normal,
            lambda document, _context, _ref: document["facts"][
                "changeset_ref"
            ].__setitem__("digest", "sha256:ABC"),
        ),
        negative_case(
            "unknown-changeset-profile",
            "changeset reference profile is unknown",
            normal,
            lambda document, _context, _ref: document["facts"][
                "changeset_ref"
            ].__setitem__(CHANGESET_PROFILE_KEY, "2.0.0"),
        ),
        negative_case(
            "omitted-contained-operation",
            "derived contained operation is omitted",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "contained_operations",
                document["facts"]["contained_operations"][:1],
            ),
        ),
        negative_case(
            "reordered-contained-operations",
            "contained operations are not in derived causal order",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "contained_operations",
                list(reversed(document["facts"]["contained_operations"])),
            ),
        ),
        negative_case(
            "duplicate-contained-operation",
            "contained operation occurrence is duplicated",
            normal,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "contained_operations",
                [
                    document["facts"]["contained_operations"][0],
                    document["facts"]["contained_operations"][0],
                ],
            ),
        ),
        negative_case(
            "malformed-contained-occurrence",
            "contained operation occurrence is malformed",
            normal,
            lambda document, _context, _ref: document["facts"][
                "contained_operations"
            ][0].__setitem__("occurrence", "op_bad"),
        ),
        negative_case(
            "publication-self-reference",
            "publication occurrence is included in contained operations",
            normal,
            lambda document, context, _ref: (
                document["facts"]["contained_operations"].append(operation_ref(9)),
                context["contained_operations"].append(operation_ref(9)),
                context.__setitem__(
                    "publication_occurrence",
                    operation_ref(9)["occurrence"],
                ),
            ),
        ),
        negative_case(
            "facts-ref-digest-mismatch",
            "facts_ref does not select the candidate facts",
            normal,
            lambda _document, _context, ref: ref.__setitem__(
                "digest", sha256_text(b"wrong facts")
            ),
        ),
    ]


def historical_hashes() -> dict[str, str]:
    return {
        name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
        for name in HISTORICAL_FILES
    }


def build_vector() -> dict[str, Any]:
    f = fixtures()
    positives = build_positive_cases(f)
    negatives = build_negative_cases(positives)
    for case in positives:
        validate_operation(case["document"], case["facts_ref"], case["context"])
    for case in negatives:
        try:
            validate_operation(
                case["candidate"],
                case["facts_ref"],
                case["context"],
            )
            if case["context"].get("publication_occurrence"):
                occurrences = [
                    item["occurrence"]
                    for item in case["candidate"]["facts"]["contained_operations"]
                ]
                if case["context"]["publication_occurrence"] in occurrences:
                    reject("publication contains its own occurrence")
        except OracleError as error:
            if error.code != case["must_fail"]:
                raise AssertionError(case["id"]) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {case['id']}")

    return {
        "vector": "CB2-K1-OPERATION-FACTS-STRUCTURAL-1",
        "description": (
            "Independent Python stdlib oracle for K1.2-GRRP-B grant, revoke, "
            "standalone rotate and publication facts. The changeset document "
            "is a syntactic digest fixture only."
        ),
        "profiles": {
            "operation_facts": PROFILE,
            "state_fact": STATE_PROFILE,
            "changeset": CHANGESET_PROFILE,
            "operation_ref": OPERATION_REF_PROFILE,
        },
        "commitment_domains": DOMAINS,
        "fixtures": f,
        "positive_cases": positives,
        "negative_cases": negatives,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "positive_case_ids": [case["id"] for case in positives],
            "negative_case_ids": [case["id"] for case in negatives],
            "required_error_variant": INVALID_OPERATION_FACTS,
            "changeset_document_is_syntactic_only": True,
            "derived_rotation_is_not_an_occurrence": True,
        },
    }


def encoded(vector: dict[str, Any]) -> bytes:
    return (
        json.dumps(vector, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


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
    print(
        f"wrote {args.output} sha256={hashlib.sha256(output).hexdigest()}"
    )


if __name__ == "__main__":
    main()

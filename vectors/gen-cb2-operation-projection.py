#!/usr/bin/env python3
"""Independent CB2 oracle for the complete W1/A1/K1 operation projection.

The standard-library-only implementation consumes frozen mandate and K1.2 facts
fixtures.  It independently derives certificate digests, RFC8785-compatible JCS
for these integer/string-only documents, the domain-separated operation
commitment, its closed public reference, and fail-closed projection/reference
correlation cases.  SC1 bytes are intentionally absent: their closed certificate
and proof tables remain a separate gate.
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


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-operation-projection.json"

PROFILE_KEY = "aithos-operation-core"
PROFILE = "1.0.0-draft.1"
FACTS_PROFILE_KEY = "aithos-operation-facts-core"
FACTS_PROFILE = "1.0.0-draft.1"
DOMAIN = "aithos-core/v1/operation-commitment"
INVALID_OPERATION = "InvalidOperation"
INVALID_OPERATION_FACTS = "InvalidOperationFacts"

TOP_KEYS = {
    PROFILE_KEY,
    "occurrence",
    "subject",
    "at",
    "history_heads",
    "authority",
    "operation",
}
OWNER_AUTHORITY_KEYS = {"actor"}
GRANTEE_AUTHORITY_KEYS = {
    "actor",
    "key",
    "authorized_by",
    "authorized_via",
}
VIA_KEYS = {"id", "certificate_digest"}
OPERATION_KEYS = {"kind", "facts_ref"}
FACTS_REF_KEYS = {FACTS_PROFILE_KEY, "digest"}
REFERENCE_KEYS = {PROFILE_KEY, "occurrence", "commitment"}

KINDS = {
    "read",
    "mutation",
    "action",
    "inference",
    "grant",
    "revoke",
    "rotate",
    "publication",
}
ULID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
OCCURRENCE_RE = re.compile(r"^op_[0-9A-HJKMNP-TV-Z]{26}$")
MANDATE_RE = re.compile(r"^mandate_[0-9A-HJKMNP-TV-Z]{26}$")
DID_RE = re.compile(r"^did:aithos:z[1-9A-HJ-NP-Za-km-z]+$")
KEY_RE = re.compile(r"^z[1-9A-HJ-NP-Za-km-z]+$")
COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
AT_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")

HISTORICAL_FILES = (
    "e1-mandate.json",
    "f1-gamma-chain.json",
    "cb2-operation-facts-mutation.json",
    "cb2-operation-facts-structural.json",
)


class OperationError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_OPERATION


class FactsError(ValueError):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.code = INVALID_OPERATION_FACTS


def reject_operation(detail: str) -> NoReturn:
    raise OperationError(detail)


def reject_facts(detail: str) -> NoReturn:
    raise FactsError(detail)


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


def occurrence(number: int) -> str:
    value = f"op_01K{number:023d}"
    assert OCCURRENCE_RE.fullmatch(value)
    return value


def require_exact_object(
    value: Any,
    keys: set[str],
    label: str,
    reject: Callable[[str], NoReturn] = reject_operation,
) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != keys:
        reject(f"{label} has a non-exact member set")
    if any(item is None for item in value.values()):
        reject(f"{label} contains null")
    return value


def require_commitment(
    value: Any,
    label: str,
    reject: Callable[[str], NoReturn] = reject_operation,
) -> str:
    if not isinstance(value, str) or not COMMITMENT_RE.fullmatch(value):
        reject(f"{label} is not strict lowercase sha256 text")
    return value


def validate_at(value: Any) -> str:
    if not isinstance(value, str) or not AT_RE.fullmatch(value):
        reject_operation("at is not canonical RFC3339 Z")
    try:
        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject_operation("at is not a calendar instant")
    return value


def validate_facts_document(
    document: Any,
    expected_ref: Any,
    expected_kind: str,
) -> None:
    require_exact_object(
        expected_ref,
        FACTS_REF_KEYS,
        "facts_ref",
        reject_facts,
    )
    if expected_ref[FACTS_PROFILE_KEY] != FACTS_PROFILE:
        reject_facts("unknown facts profile")
    require_commitment(expected_ref["digest"], "facts_ref.digest", reject_facts)
    require_exact_object(
        document,
        {FACTS_PROFILE_KEY, "kind", "facts"},
        "facts document",
        reject_facts,
    )
    if document[FACTS_PROFILE_KEY] != FACTS_PROFILE:
        reject_facts("facts document profile mismatch")
    if document["kind"] != expected_kind:
        reject_facts("selected facts family mismatch")
    expected_digest = commitment(
        "aithos-core/v1/operation-facts",
        jcs(document).encode("utf-8"),
    )
    if expected_ref["digest"] != expected_digest:
        reject_facts("facts digest mismatch")


def validate_authority(
    value: Any,
    projection: dict[str, Any],
    certificates: dict[str, dict[str, Any]],
    *,
    session_required: bool,
) -> None:
    if not isinstance(value, dict):
        reject_operation("authority is not an object")
    actor = value.get("actor")
    if actor == "owner":
        require_exact_object(value, OWNER_AUTHORITY_KEYS, "owner authority")
        if session_required:
            reject_operation("owner cannot satisfy delegated session context")
        return
    if actor != "grantee":
        reject_operation("unknown authority actor")
    require_exact_object(value, GRANTEE_AUTHORITY_KEYS, "grantee authority")
    if session_required:
        reject_operation("session-bound authority is incomplete without SC1 gate")
    if not isinstance(value["key"], str) or not KEY_RE.fullmatch(value["key"]):
        reject_operation("invalid grantee authority key")
    if not isinstance(value["authorized_by"], str) or not MANDATE_RE.fullmatch(
        value["authorized_by"]
    ):
        reject_operation("invalid authorized_by")
    via = value["authorized_via"]
    if not isinstance(via, list) or not via:
        reject_operation("authorized_via must be non-empty")
    seen: set[str] = set()
    previous: Optional[dict[str, Any]] = None
    for item in via:
        require_exact_object(item, VIA_KEYS, "authorized_via item")
        mandate_id = item["id"]
        if not isinstance(mandate_id, str) or not MANDATE_RE.fullmatch(mandate_id):
            reject_operation("invalid authority mandate id")
        if mandate_id in seen:
            reject_operation("duplicate authority mandate id")
        seen.add(mandate_id)
        certificate = certificates.get(mandate_id)
        if certificate is None:
            reject_operation("authority certificate is missing")
        if certificate.get("id") != mandate_id:
            reject_operation("certificate embedded id mismatch")
        if certificate.get("subject") != projection["subject"]:
            reject_operation("certificate subject mismatch")
        if item["certificate_digest"] != sha256_text(
            jcs(certificate).encode("utf-8")
        ):
            reject_operation("certificate digest mismatch")
        if previous is not None:
            if certificate.get("parent") != previous.get("id"):
                reject_operation("certificate parent mismatch")
            if certificate.get("issued_by") != previous["grantee"]["pubkey"]:
                reject_operation("certificate issuer mismatch")
        previous = certificate
    leaf = certificates[via[-1]["id"]]
    if value["authorized_by"] != leaf["id"]:
        reject_operation("authorized_by is not the leaf")
    if value["key"] != leaf["grantee"]["pubkey"]:
        reject_operation("authority key is not the leaf grantee")


def validate_projection(
    value: Any,
    *,
    facts_documents: dict[str, Any],
    certificates: dict[str, dict[str, Any]],
    session_required: bool = False,
) -> str:
    projection = require_exact_object(value, TOP_KEYS, "operation projection")
    if projection[PROFILE_KEY] != PROFILE:
        reject_operation("unknown operation profile")
    if not isinstance(projection["occurrence"], str) or not OCCURRENCE_RE.fullmatch(
        projection["occurrence"]
    ):
        reject_operation("invalid occurrence")
    if not isinstance(projection["subject"], str) or not DID_RE.fullmatch(
        projection["subject"]
    ):
        reject_operation("invalid subject")
    validate_at(projection["at"])
    heads = projection["history_heads"]
    if not isinstance(heads, list) or len(heads) > 2:
        reject_operation("history_heads cardinality is invalid")
    if any(
        not isinstance(head, str) or not COMMITMENT_RE.fullmatch(head)
        for head in heads
    ):
        reject_operation("invalid history head")
    if heads != sorted(set(heads)):
        reject_operation("history heads are not distinct and sorted")
    validate_authority(
        projection["authority"],
        projection,
        certificates,
        session_required=session_required,
    )
    operation = require_exact_object(
        projection["operation"],
        OPERATION_KEYS,
        "operation",
    )
    if operation["kind"] not in KINDS:
        reject_operation("unknown operation kind")
    facts_ref = operation["facts_ref"]
    if not isinstance(facts_ref, dict):
        reject_facts("facts_ref is not an object")
    digest = facts_ref.get("digest")
    facts = facts_documents.get(digest)
    if facts is None:
        reject_facts("selected facts document is missing")
    validate_facts_document(facts, facts_ref, operation["kind"])
    return commitment(DOMAIN, jcs(projection).encode("utf-8"))


def operation_ref(projection: dict[str, Any], derived: str) -> dict[str, Any]:
    return {
        PROFILE_KEY: PROFILE,
        "occurrence": projection["occurrence"],
        "commitment": derived,
    }


def validate_reference(value: Any, projection: dict[str, Any], derived: str) -> None:
    reference = require_exact_object(value, REFERENCE_KEYS, "operation_ref")
    if reference[PROFILE_KEY] != PROFILE:
        reject_operation("unknown operation_ref profile")
    if reference["occurrence"] != projection["occurrence"]:
        reject_operation("operation_ref occurrence mismatch")
    require_commitment(reference["commitment"], "operation_ref commitment")
    if reference["commitment"] != derived:
        reject_operation("operation_ref commitment mismatch")


def validate_correlation(first: Any, second: Any) -> None:
    one = require_exact_object(first, REFERENCE_KEYS, "first operation_ref")
    two = require_exact_object(second, REFERENCE_KEYS, "second operation_ref")
    for candidate in (one, two):
        if candidate[PROFILE_KEY] != PROFILE:
            reject_operation("unknown operation_ref profile")
        if not isinstance(candidate["occurrence"], str) or not OCCURRENCE_RE.fullmatch(
            candidate["occurrence"]
        ):
            reject_operation("invalid correlated occurrence")
        require_commitment(candidate["commitment"], "correlated commitment")
    if one["occurrence"] == two["occurrence"] and one["commitment"] != two["commitment"]:
        reject_operation("operation occurrence equivocation")


def load_fixtures() -> dict[str, Any]:
    mandate_vector = json.loads((HERE / "e1-mandate.json").read_text())
    certificate = json.loads(mandate_vector["mandate_jcs"])
    mutation = json.loads((HERE / "cb2-operation-facts-mutation.json").read_text())
    structural = json.loads((HERE / "cb2-operation-facts-structural.json").read_text())
    f1 = json.loads((HERE / "f1-gamma-chain.json").read_text())
    mutation_case = next(
        case for case in mutation["positive_cases"] if case["id"] == "ethos-create"
    )
    publication_case = next(
        case
        for case in structural["positive_cases"]
        if case["id"] == "publication-merge"
    )
    return {
        "certificate": certificate,
        "certificate_jcs": jcs(certificate),
        "certificate_digest": sha256_text(jcs(certificate).encode("utf-8")),
        "mutation": mutation_case,
        "publication": publication_case,
        "gamma_head": f1["gamma_head"],
    }


def build_positive_cases(fixtures: dict[str, Any]) -> list[dict[str, Any]]:
    certificate = fixtures["certificate"]
    subject = certificate["subject"]
    grantee_authority = {
        "actor": "grantee",
        "key": certificate["grantee"]["pubkey"],
        "authorized_by": certificate["id"],
        "authorized_via": [
            {
                "id": certificate["id"],
                "certificate_digest": fixtures["certificate_digest"],
            }
        ],
    }
    mutation_operation = {
        "kind": "mutation",
        "facts_ref": fixtures["mutation"]["facts_ref"],
    }
    publication_operation = {
        "kind": "publication",
        "facts_ref": fixtures["publication"]["facts_ref"],
    }
    second_head = sha256_text(b"independent-second-parent")
    projections = [
        (
            "owner-mutation-no-history",
            {
                PROFILE_KEY: PROFILE,
                "occurrence": occurrence(1),
                "subject": subject,
                "at": "2026-07-02T12:00:00Z",
                "history_heads": [],
                "authority": {"actor": "owner"},
                "operation": mutation_operation,
            },
        ),
        (
            "grantee-mutation-one-head",
            {
                PROFILE_KEY: PROFILE,
                "occurrence": occurrence(2),
                "subject": subject,
                "at": "2026-07-02T12:01:00Z",
                "history_heads": [fixtures["gamma_head"]],
                "authority": grantee_authority,
                "operation": mutation_operation,
            },
        ),
        (
            "grantee-merge-two-heads",
            {
                PROFILE_KEY: PROFILE,
                "occurrence": occurrence(3),
                "subject": subject,
                "at": "2026-07-02T12:02:00.123Z",
                "history_heads": sorted([fixtures["gamma_head"], second_head]),
                "authority": grantee_authority,
                "operation": publication_operation,
            },
        ),
        (
            "owner-identical-effect-distinct-occurrence",
            {
                PROFILE_KEY: PROFILE,
                "occurrence": occurrence(4),
                "subject": subject,
                "at": "2026-07-02T12:00:00Z",
                "history_heads": [],
                "authority": {"actor": "owner"},
                "operation": mutation_operation,
            },
        ),
    ]
    facts_documents = {
        fixtures["mutation"]["digest"]: fixtures["mutation"]["document"],
        fixtures["publication"]["digest"]: fixtures["publication"]["document"],
    }
    certificates = {certificate["id"]: certificate}
    out = []
    for identifier, projection in projections:
        derived = validate_projection(
            projection,
            facts_documents=facts_documents,
            certificates=certificates,
        )
        reference = operation_ref(projection, derived)
        validate_reference(reference, projection, derived)
        out.append(
            {
                "id": identifier,
                "projection": projection,
                "projection_jcs": jcs(projection),
                "commitment": derived,
                "operation_ref": reference,
            }
        )
    if out[0]["commitment"] == out[3]["commitment"]:
        raise AssertionError("distinct occurrence anchors must change commitment")
    return out


def build_projection_negatives(
    valid: dict[str, Any],
    fixtures: dict[str, Any],
) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutator: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutator))

    add("missing-profile", lambda c: c.pop(PROFILE_KEY))
    add("unknown-profile", lambda c: c.__setitem__(PROFILE_KEY, "1.0.0-draft.2"))
    add("extra-top-member", lambda c: c.__setitem__("extra", True))
    add("null-occurrence", lambda c: c.__setitem__("occurrence", None))
    add("malformed-occurrence", lambda c: c.__setitem__("occurrence", "gamma_01K"))
    add("missing-subject", lambda c: c.pop("subject"))
    add("malformed-subject", lambda c: c.__setitem__("subject", "did:example:bad"))
    add("offset-at", lambda c: c.__setitem__("at", "2026-07-02T14:01:00+02:00"))
    add("invalid-calendar-at", lambda c: c.__setitem__("at", "2026-02-30T12:01:00Z"))
    add("three-history-heads", lambda c: c["history_heads"].extend(["sha256:" + "22" * 32, "sha256:" + "33" * 32]))
    add("duplicate-history-head", lambda c: c["history_heads"].append(c["history_heads"][0]))
    add("unsorted-history-heads", lambda c: c.__setitem__("history_heads", ["sha256:" + "ff" * 32, "sha256:" + "00" * 32]))
    add("malformed-history-head", lambda c: c.__setitem__("history_heads", ["FF" * 32]))
    add("null-authority", lambda c: c.__setitem__("authority", None))
    add("unknown-actor", lambda c: c["authority"].__setitem__("actor", "delegate"))
    add("owner-extra-authority-member", lambda c: c.__setitem__("authority", {"actor": "owner", "key": fixtures["certificate"]["grantee"]["pubkey"]}))
    add("grantee-missing-key", lambda c: c["authority"].pop("key"))
    add("grantee-wrong-key", lambda c: c["authority"].__setitem__("key", "z6MkWrong"))
    add("grantee-missing-authorized-by", lambda c: c["authority"].pop("authorized_by"))
    add("grantee-wrong-authorized-by", lambda c: c["authority"].__setitem__("authorized_by", "mandate_01J00000000000000000000099"))
    add("empty-authorized-via", lambda c: c["authority"].__setitem__("authorized_via", []))
    add("duplicate-authorized-via", lambda c: c["authority"]["authorized_via"].append(clone(c["authority"]["authorized_via"][0])))
    add("extra-via-member", lambda c: c["authority"]["authorized_via"][0].__setitem__("extra", True))
    add("wrong-certificate-digest", lambda c: c["authority"]["authorized_via"][0].__setitem__("certificate_digest", "sha256:" + "00" * 32))
    add("missing-operation", lambda c: c.pop("operation"))
    add("extra-operation-member", lambda c: c["operation"].__setitem__("target", "forbidden"))
    add("unknown-kind", lambda c: c["operation"].__setitem__("kind", "heartbeat"))
    add("null-facts-ref", lambda c: c["operation"].__setitem__("facts_ref", None))
    add("unknown-facts-profile", lambda c: c["operation"]["facts_ref"].__setitem__(FACTS_PROFILE_KEY, "1.0.0-draft.2"))
    add("malformed-facts-digest", lambda c: c["operation"]["facts_ref"].__setitem__("digest", "SHA256:" + "00" * 32))
    add("missing-facts-document", lambda c: c["operation"]["facts_ref"].__setitem__("digest", "sha256:" + "00" * 32))
    add("selected-family-mismatch", lambda c: c["operation"].__setitem__("kind", "action"))

    facts_documents = {
        fixtures["mutation"]["digest"]: fixtures["mutation"]["document"],
        fixtures["publication"]["digest"]: fixtures["publication"]["document"],
    }
    certificates = {fixtures["certificate"]["id"]: fixtures["certificate"]}
    out = []
    for identifier, mutator in cases:
        candidate = clone(valid["projection"])
        mutator(candidate)
        expected = (
            INVALID_OPERATION_FACTS
            if identifier
            in {
                "unknown-facts-profile",
                "malformed-facts-digest",
                "missing-facts-document",
                "selected-family-mismatch",
            }
            else INVALID_OPERATION
        )
        try:
            validate_projection(
                candidate,
                facts_documents=facts_documents,
                certificates=certificates,
            )
        except (OperationError, FactsError) as error:
            if error.code != expected:
                raise AssertionError(
                    f"{identifier}: expected {expected}, got {error.code}"
                ) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {identifier}")
        out.append(
            {
                "id": identifier,
                "candidate": candidate,
                "must_fail": expected,
            }
        )
    return out


def build_reference_negatives(valid: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[tuple[str, Callable[[dict[str, Any]], None]]] = []

    def add(identifier: str, mutator: Callable[[dict[str, Any]], None]) -> None:
        cases.append((identifier, mutator))

    add("reference-missing-profile", lambda c: c.pop(PROFILE_KEY))
    add("reference-unknown-profile", lambda c: c.__setitem__(PROFILE_KEY, "1.0.0-draft.2"))
    add("reference-extra-member", lambda c: c.__setitem__("extra", True))
    add("reference-wrong-occurrence", lambda c: c.__setitem__("occurrence", occurrence(99)))
    add("reference-malformed-commitment", lambda c: c.__setitem__("commitment", "00" * 32))
    add("reference-wrong-commitment", lambda c: c.__setitem__("commitment", "sha256:" + "00" * 32))

    out = []
    for identifier, mutator in cases:
        candidate = clone(valid["operation_ref"])
        mutator(candidate)
        try:
            validate_reference(candidate, valid["projection"], valid["commitment"])
        except OperationError as error:
            if error.code != INVALID_OPERATION:
                raise AssertionError(identifier) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {identifier}")
        out.append(
            {
                "id": identifier,
                "candidate": candidate,
                "must_fail": INVALID_OPERATION,
            }
        )
    return out


def correlation_cases(positives: list[dict[str, Any]]) -> list[dict[str, Any]]:
    same = positives[1]["operation_ref"]
    distinct = positives[0]["operation_ref"]
    equivocation = clone(same)
    equivocation["commitment"] = "sha256:" + "ff" * 32
    cases = [
        {
            "id": "same-reference-cross-view",
            "first": same,
            "second": clone(same),
            "verdict": "correlated",
        },
        {
            "id": "distinct-occurrences-identical-effect",
            "first": positives[0]["operation_ref"],
            "second": positives[3]["operation_ref"],
            "verdict": "distinct",
        },
        {
            "id": "same-occurrence-different-commitment",
            "first": same,
            "second": equivocation,
            "must_fail": INVALID_OPERATION,
        },
        {
            "id": "different-occurrences-different-commitments",
            "first": same,
            "second": distinct,
            "verdict": "distinct",
        },
    ]
    for case in cases:
        try:
            validate_correlation(case["first"], case["second"])
        except OperationError as error:
            if case.get("must_fail") != error.code:
                raise AssertionError(case["id"]) from error
        else:
            if "must_fail" in case:
                raise AssertionError(f"negative unexpectedly accepted: {case['id']}")
    return cases


def historical_hashes() -> dict[str, str]:
    return {
        name: hashlib.sha256((HERE / name).read_bytes()).hexdigest()
        for name in HISTORICAL_FILES
    }


def build_vector() -> dict[str, Any]:
    fixtures = load_fixtures()
    positives = build_positive_cases(fixtures)
    projection_negatives = build_projection_negatives(positives[1], fixtures)
    reference_negatives = build_reference_negatives(positives[1])
    correlations = correlation_cases(positives)
    return {
        "vector": "CB2-W1-OPERATION-PROJECTION-1",
        "description": (
            "Independent Python stdlib oracle for the complete owner/grantee "
            "W1/A1/K1 projection without SC1, certificate content addresses, "
            "operation commitment/reference bytes, cross-view correlation, "
            "typed negative boundaries, and historical byte non-regression."
        ),
        "profiles": {
            "operation": PROFILE,
            "operation_facts": FACTS_PROFILE,
        },
        "commitment_domain": DOMAIN,
        "fixtures": {
            "certificate": fixtures["certificate"],
            "certificate_jcs": fixtures["certificate_jcs"],
            "certificate_digest": fixtures["certificate_digest"],
            "mutation_facts_id": fixtures["mutation"]["id"],
            "publication_facts_id": fixtures["publication"]["id"],
        },
        "positive_cases": positives,
        "negative_projection_cases": projection_negatives,
        "negative_reference_cases": reference_negatives,
        "correlation_cases": correlations,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "positive_ids": [case["id"] for case in positives],
            "projection_negative_ids": [
                case["id"] for case in projection_negatives
            ],
            "reference_negative_ids": [
                case["id"] for case in reference_negatives
            ],
            "operation_error_variant": INVALID_OPERATION,
            "facts_error_variant": INVALID_OPERATION_FACTS,
            "sc1_complete_bytes_are_out_of_scope": True,
            "receipt_and_carrier_digests_are_not_projection_inputs": True,
        },
    }


def encoded(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
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
    print(f"wrote {args.output} sha256={hashlib.sha256(output).hexdigest()}")


if __name__ == "__main__":
    main()

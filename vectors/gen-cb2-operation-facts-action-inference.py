#!/usr/bin/env python3
"""Independent CB2 oracle for K1.2-AI-B action and inference facts.

Only Python's standard library is used.  The oracle computes historical action
argument hashes, domain-separated inference-request and purpose commitments,
content addresses for syntactic catalog fixtures, operation-facts JCS/digests,
and the approved InvalidOperationFacts negative taxonomy.

The catalog and approval documents are deliberately syntactic fixtures.  Their
future signed member tables are not claimed GREEN by this vector; K1.2-AI-B pins
only their distinct complete-document content addresses.
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
DEFAULT_OUTPUT = HERE / "cb2-operation-facts-action-inference.json"

PROFILE_KEY = "aithos-operation-facts-core"
PROFILE = "1.0.0-draft.1"
INVALID_OPERATION_FACTS = "InvalidOperationFacts"

DOMAINS = {
    "operation_facts": "aithos-core/v1/operation-facts",
    "inference_request": "aithos-core/v1/inference-request",
    "purpose": "aithos-core/v1/purpose",
}

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
TOKEN_RE = re.compile(r"^[a-z][a-z0-9._-]*$")
VERSION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")

HISTORICAL_FILES = (
    "e1-mandate.json",
    "f1-gamma-chain.json",
    "f2-gamma-counting.json",
    "gplus-obligations.json",
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
    """RFC 8785-compatible encoding for this integer/string fixture domain."""

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
    expected_keys: set[str],
    label: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        reject(f"{label} is not an object")
    if set(value) != expected_keys:
        reject(f"{label} has a non-exact member set")
    if any(member is None for member in value.values()):
        reject(f"{label} contains null")
    return value


def require_commitment(value: Any, label: str) -> str:
    if not isinstance(value, str) or not COMMITMENT_RE.fullmatch(value):
        reject(f"{label} is not strict lowercase sha256 text")
    return value


def require_token(value: Any, label: str) -> str:
    if not isinstance(value, str) or not TOKEN_RE.fullmatch(value):
        reject(f"{label} is not a canonical non-empty identifier")
    return value


def catalog_fixtures() -> dict[str, Any]:
    catalog_document = {
        "aithos-connector-catalog-core": "1.0.0-fixture",
        "connector": "mail",
        "version": "2026.07",
        "actions": [
            {"action": "list", "class": "read"},
            {"action": "send", "class": "act"},
            {"action": "purchase", "class": "binding"},
        ],
        "signature": {
            "alg": "ed25519",
            "key": "z6MkCatalogFixture",
            "value": "11" * 64,
        },
    }
    catalog_digest = sha256_text(jcs(catalog_document).encode("utf-8"))
    approval_document = {
        "aithos-connector-catalog-approval-core": "1.0.0-fixture",
        "subject": "did:aithos:z6MkOwnerFixture",
        "connector": "mail",
        "catalog_version": "2026.07",
        "catalog_digest": catalog_digest,
        "signature": {
            "alg": "ed25519",
            "key": "#content",
            "value": "22" * 64,
        },
    }
    approval_digest = sha256_text(jcs(approval_document).encode("utf-8"))
    return {
        "catalog_document": catalog_document,
        "catalog_document_jcs": jcs(catalog_document),
        "catalog_digest": catalog_digest,
        "approval_document": approval_document,
        "approval_document_jcs": jcs(approval_document),
        "approval_digest": approval_digest,
        "catalog_ref": {
            "catalog_version": "2026.07",
            "catalog_digest": catalog_digest,
            "approval_digest": approval_digest,
        },
    }


def fixtures() -> dict[str, Any]:
    action_args = {
        "recipients": ["alice@example.test"],
        "subject": "CB2 exact action",
        "body": "private fixture body",
    }
    request_body = jcs(
        {
            "messages": [
                {"role": "user", "content": "private inference fixture"}
            ],
            "temperature": 0,
            "tools": [{"name": "lookup", "input_schema": {"type": "object"}}],
        }
    ).encode("utf-8")
    purpose_text = "Prepare the first Aithos protocol demonstration"
    catalog = catalog_fixtures()
    return {
        "catalog": catalog,
        "action": {
            "connector": "mail",
            "action": "send",
            "args": action_args,
            "args_jcs": jcs(action_args),
            "args_hash": sha256_text(jcs(action_args).encode("utf-8")),
        },
        "inference": {
            "provider": "anthropic",
            "model": "claude-haiku",
            "request_body_hex": request_body.hex(),
            "request_digest": commitment(
                DOMAINS["inference_request"],
                request_body,
            ),
        },
        "budget_ref": "haiku",
        "purpose_text": purpose_text,
        "purpose_ref": commitment(
            DOMAINS["purpose"],
            purpose_text.encode("utf-8"),
        ),
    }


def applicability(
    label: str,
    applicable: bool,
    fixture_values: dict[str, Any],
) -> dict[str, Any]:
    if not applicable:
        return {"state": "not-applicable"}
    if label == "budget":
        return {"state": "cited", "budget_ref": fixture_values["budget_ref"]}
    if label == "purpose":
        return {"state": "cited", "purpose_ref": fixture_values["purpose_ref"]}
    raise AssertionError(label)


def operation_case(
    case_id: str,
    kind: str,
    budget_applicable: bool,
    purpose_applicable: bool,
    fixture_values: dict[str, Any],
) -> dict[str, Any]:
    if kind == "action":
        action = fixture_values["action"]
        facts = {
            "connector": action["connector"],
            "action": action["action"],
            "catalog_ref": clone(fixture_values["catalog"]["catalog_ref"]),
            "args_hash": action["args_hash"],
            "budget": applicability(
                "budget", budget_applicable, fixture_values
            ),
            "purpose": applicability(
                "purpose", purpose_applicable, fixture_values
            ),
        }
        context = {
            "connector": action["connector"],
            "action": action["action"],
            "catalog_ref": clone(fixture_values["catalog"]["catalog_ref"]),
            "args": clone(action["args"]),
            "budget_applicable": budget_applicable,
            "budget_ref": fixture_values["budget_ref"],
            "purpose_applicable": purpose_applicable,
            "purpose_text": fixture_values["purpose_text"],
        }
    elif kind == "inference":
        inference = fixture_values["inference"]
        facts = {
            "provider": inference["provider"],
            "model": inference["model"],
            "request_digest": inference["request_digest"],
            "budget": applicability(
                "budget", budget_applicable, fixture_values
            ),
            "purpose": applicability(
                "purpose", purpose_applicable, fixture_values
            ),
        }
        context = {
            "provider": inference["provider"],
            "model": inference["model"],
            "request_body_hex": inference["request_body_hex"],
            "budget_applicable": budget_applicable,
            "budget_ref": fixture_values["budget_ref"],
            "purpose_applicable": purpose_applicable,
            "purpose_text": fixture_values["purpose_text"],
        }
    else:
        raise AssertionError(kind)

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


def build_positive_cases(fixture_values: dict[str, Any]) -> list[dict[str, Any]]:
    cases: list[dict[str, Any]] = []
    for kind in ("action", "inference"):
        for budget, purpose, suffix in (
            (False, False, "plain"),
            (True, False, "budget"),
            (False, True, "purpose"),
            (True, True, "budget-purpose"),
        ):
            cases.append(
                operation_case(
                    f"{kind}-{suffix}",
                    kind,
                    budget,
                    purpose,
                    fixture_values,
                )
            )
    return cases


def validate_catalog_ref(value: Any, context: dict[str, Any]) -> None:
    catalog_ref = require_exact_object(
        value,
        {"catalog_version", "catalog_digest", "approval_digest"},
        "catalog_ref",
    )
    if (
        not isinstance(catalog_ref["catalog_version"], str)
        or not VERSION_RE.fullmatch(catalog_ref["catalog_version"])
    ):
        reject("catalog version is not a canonical non-empty identifier")
    require_commitment(catalog_ref["catalog_digest"], "catalog digest")
    require_commitment(catalog_ref["approval_digest"], "approval digest")
    if catalog_ref != context["catalog_ref"]:
        reject("catalog reference does not match the approved native reference")


def validate_applicability(
    value: Any,
    label: str,
    expected_applicable: bool,
    expected_value: str,
) -> None:
    if not isinstance(value, dict):
        reject(f"{label} applicability is not an object")
    if value.get("state") == "not-applicable":
        require_exact_object(value, {"state"}, f"{label} applicability")
        if expected_applicable:
            reject(f"applicable {label} was omitted")
        return
    if value.get("state") == "cited":
        member = f"{label}_ref"
        cited = require_exact_object(
            value,
            {"state", member},
            f"{label} applicability",
        )
        if not expected_applicable:
            reject(f"non-applicable {label} was volunteered")
        if label == "budget":
            require_token(cited[member], "budget_ref")
        else:
            require_commitment(cited[member], "purpose_ref")
        if cited[member] != expected_value:
            reject(f"{label} citation mismatch")
        return
    reject(f"unknown {label} applicability state")


def validate_action_facts(facts: Any, context: dict[str, Any]) -> None:
    value = require_exact_object(
        facts,
        {
            "connector",
            "action",
            "catalog_ref",
            "args_hash",
            "budget",
            "purpose",
        },
        "action facts",
    )
    require_token(value["connector"], "connector")
    require_token(value["action"], "action")
    if (
        value["connector"] != context["connector"]
        or value["action"] != context["action"]
    ):
        reject("native connector action mismatch")
    validate_catalog_ref(value["catalog_ref"], context)
    args_hash = require_commitment(value["args_hash"], "args_hash")
    expected_args_hash = sha256_text(jcs(context["args"]).encode("utf-8"))
    if args_hash != expected_args_hash:
        reject("action arguments do not match args_hash")
    validate_applicability(
        value["budget"],
        "budget",
        context["budget_applicable"],
        context["budget_ref"],
    )
    validate_applicability(
        value["purpose"],
        "purpose",
        context["purpose_applicable"],
        commitment(
            DOMAINS["purpose"],
            context["purpose_text"].encode("utf-8"),
        ),
    )


def validate_inference_facts(facts: Any, context: dict[str, Any]) -> None:
    value = require_exact_object(
        facts,
        {"provider", "model", "request_digest", "budget", "purpose"},
        "inference facts",
    )
    require_token(value["provider"], "provider")
    require_token(value["model"], "model")
    if (
        value["provider"] != context["provider"]
        or value["model"] != context["model"]
    ):
        reject("native inference provider or model mismatch")
    request_digest = require_commitment(
        value["request_digest"],
        "request_digest",
    )
    try:
        request_body = bytes.fromhex(context["request_body_hex"])
    except (TypeError, ValueError):
        reject("private request body is not an exact byte string")
    expected_request = commitment(DOMAINS["inference_request"], request_body)
    if request_digest != expected_request:
        reject("private inference request mismatch")
    validate_applicability(
        value["budget"],
        "budget",
        context["budget_applicable"],
        context["budget_ref"],
    )
    validate_applicability(
        value["purpose"],
        "purpose",
        context["purpose_applicable"],
        commitment(
            DOMAINS["purpose"],
            context["purpose_text"].encode("utf-8"),
        ),
    )


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
    if value["kind"] == "action":
        validate_action_facts(value["facts"], context)
    elif value["kind"] == "inference":
        validate_inference_facts(value["facts"], context)
    else:
        reject("operation kind does not select action or inference facts")

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
    action_plain = case_by_id(positives, "action-plain")
    action_budget = case_by_id(positives, "action-budget")
    action_purpose = case_by_id(positives, "action-purpose")
    action_both = case_by_id(positives, "action-budget-purpose")
    inference_plain = case_by_id(positives, "inference-plain")
    inference_budget = case_by_id(positives, "inference-budget")
    inference_purpose = case_by_id(positives, "inference-purpose")

    return [
        negative_case(
            "missing-envelope-profile",
            "missing operation-facts profile",
            action_plain,
            lambda document, _context, _ref: document.pop(PROFILE_KEY),
        ),
        negative_case(
            "extra-envelope-member",
            "extra operation-facts envelope member",
            action_plain,
            lambda document, _context, _ref: document.__setitem__(
                "nonce", "forbidden"
            ),
        ),
        negative_case(
            "kind-family-mismatch",
            "action facts selected as inference",
            action_plain,
            lambda document, _context, _ref: document.__setitem__(
                "kind", "inference"
            ),
        ),
        negative_case(
            "missing-action-member",
            "missing exact action member",
            action_plain,
            lambda document, _context, _ref: document["facts"].pop("action"),
        ),
        negative_case(
            "extra-action-member",
            "extra action member",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "request_digest", sha256_text(b"forbidden")
            ),
        ),
        negative_case(
            "null-action-member",
            "null action member",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "args_hash", None
            ),
        ),
        negative_case(
            "empty-connector",
            "empty connector identifier",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "connector", ""
            ),
        ),
        negative_case(
            "mismatched-action",
            "native action identifier mismatch",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "action", "purchase"
            ),
        ),
        negative_case(
            "missing-catalog-member",
            "catalog_ref missing approval digest",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].pop("approval_digest"),
        ),
        negative_case(
            "extra-catalog-member",
            "catalog_ref duplicates derived class",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].__setitem__("class", "act"),
        ),
        negative_case(
            "empty-catalog-version",
            "empty catalog version",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].__setitem__("catalog_version", ""),
        ),
        negative_case(
            "malformed-catalog-digest",
            "malformed catalog digest",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].__setitem__("catalog_digest", "sha256:ABC"),
        ),
        negative_case(
            "mismatched-catalog-digest",
            "different catalog digest under the same version",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].__setitem__("catalog_digest", sha256_text(b"other catalog")),
        ),
        negative_case(
            "mismatched-approval-digest",
            "different owner approval evidence",
            action_plain,
            lambda document, _context, _ref: document["facts"][
                "catalog_ref"
            ].__setitem__("approval_digest", sha256_text(b"other approval")),
        ),
        negative_case(
            "malformed-args-hash",
            "malformed args_hash",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "args_hash", "sha256:ABC"
            ),
        ),
        negative_case(
            "mismatched-action-arguments",
            "native arguments differ from args_hash",
            action_plain,
            lambda _document, context, _ref: context["args"].__setitem__(
                "subject", "changed"
            ),
        ),
        negative_case(
            "action-post-effect-tokens",
            "action facts carry post-effect tokens",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "tokens", 10
            ),
        ),
        negative_case(
            "action-usage-receipt",
            "action facts carry a usage receipt",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "usage_receipt", {}
            ),
        ),
        negative_case(
            "missing-inference-member",
            "missing exact inference member",
            inference_plain,
            lambda document, _context, _ref: document["facts"].pop("model"),
        ),
        negative_case(
            "empty-provider",
            "empty provider identifier",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "provider", ""
            ),
        ),
        negative_case(
            "empty-model",
            "empty model identifier",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "model", ""
            ),
        ),
        negative_case(
            "malformed-request-digest",
            "malformed request digest",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "request_digest", "sha256:ABC"
            ),
        ),
        negative_case(
            "mismatched-inference-request",
            "private request bytes differ from request_digest",
            inference_plain,
            lambda _document, context, _ref: context.__setitem__(
                "request_body_hex", (b"changed request bytes").hex()
            ),
        ),
        negative_case(
            "inference-args-hash",
            "inference invents args_hash",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "args_hash", sha256_text(b"forbidden")
            ),
        ),
        negative_case(
            "inference-post-effect-counters",
            "inference facts carry post-effect token counters",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "tokens_in", 1
            ),
        ),
        negative_case(
            "missing-applicable-budget",
            "applicable budget is marked not-applicable",
            action_budget,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "budget", {"state": "not-applicable"}
            ),
        ),
        negative_case(
            "volunteered-budget",
            "budget citation is volunteered without budgets",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "budget", {"state": "cited", "budget_ref": "haiku"}
            ),
        ),
        negative_case(
            "empty-budget-ref",
            "budget citation is empty",
            inference_budget,
            lambda document, _context, _ref: document["facts"]["budget"].__setitem__(
                "budget_ref", ""
            ),
        ),
        negative_case(
            "extra-budget-member",
            "budget applicability carries an extra member",
            action_both,
            lambda document, _context, _ref: document["facts"]["budget"].__setitem__(
                "tokens", 10
            ),
        ),
        negative_case(
            "unknown-budget-state",
            "budget applicability has an unknown state",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "budget", {"state": "absent"}
            ),
        ),
        negative_case(
            "missing-applicable-purpose",
            "applicable purpose is marked not-applicable",
            action_purpose,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "purpose", {"state": "not-applicable"}
            ),
        ),
        negative_case(
            "volunteered-purpose",
            "purpose citation is volunteered without purpose",
            inference_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "purpose",
                {
                    "state": "cited",
                    "purpose_ref": sha256_text(b"volunteered"),
                },
            ),
        ),
        negative_case(
            "mismatched-purpose-ref",
            "purpose text differs from purpose_ref",
            inference_purpose,
            lambda _document, context, _ref: context.__setitem__(
                "purpose_text", "changed purpose"
            ),
        ),
        negative_case(
            "null-purpose",
            "purpose applicability is null",
            action_plain,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "purpose", None
            ),
        ),
        negative_case(
            "facts-ref-digest-mismatch",
            "facts_ref does not select the candidate facts",
            action_plain,
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
    fixture_values = fixtures()
    positives = build_positive_cases(fixture_values)
    negatives = build_negative_cases(positives)

    for case in positives:
        validate_operation(
            case["document"],
            case["facts_ref"],
            case["context"],
        )
    for case in negatives:
        try:
            validate_operation(
                case["candidate"],
                case["facts_ref"],
                case["context"],
            )
        except OracleError as error:
            if error.code != case["must_fail"]:
                raise AssertionError(case["id"]) from error
        else:
            raise AssertionError(f"negative unexpectedly accepted: {case['id']}")

    return {
        "vector": "CB2-K1-OPERATION-FACTS-ACTION-INFERENCE-1",
        "description": (
            "Independent Python stdlib oracle for K1.2-AI-B closed action and "
            "inference pre-effect facts. Catalog documents are syntactic "
            "content-address fixtures only; their signed tables remain reserved."
        ),
        "profiles": {
            "operation_facts": PROFILE,
        },
        "commitment_domains": DOMAINS,
        "fixtures": fixture_values,
        "positive_cases": positives,
        "negative_cases": negatives,
        "historical_vector_sha256": historical_hashes(),
        "inventory": {
            "positive_case_ids": [case["id"] for case in positives],
            "negative_case_ids": [case["id"] for case in negatives],
            "required_error_variant": INVALID_OPERATION_FACTS,
            "catalog_documents_are_syntactic_only": True,
            "post_effect_members_forbidden": [
                "tokens",
                "tokens_in",
                "tokens_out",
                "usage_receipt",
                "response",
            ],
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

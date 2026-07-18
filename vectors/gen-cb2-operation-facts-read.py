#!/usr/bin/env python3
"""Independent CB2 oracle for the K1.2-R-B closed read facts.

The generator uses only Python's standard library.  It independently computes
the existing manifest chain-hash preimage, the domain-separated canonical
``read.gamma`` request digest, the vault state-key commitment, operation-facts
JCS/digests, and the approved ``InvalidOperationFacts`` negative taxonomy.

The fixture store key and protected query string are oracle inputs only.  They
define neither a Bundle path nor a public presentation carrier.

Usage:
    python3 vectors/gen-cb2-operation-facts-read.py
    python3 vectors/gen-cb2-operation-facts-read.py --check
    python3 vectors/gen-cb2-operation-facts-read.py --output /tmp/cb2.json
"""

from __future__ import annotations

import argparse
import copy
from datetime import datetime
import hashlib
import json
from pathlib import Path
import re
from typing import Any, Callable, NoReturn


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-operation-facts-read.json"

OPERATION_PROFILE_KEY = "aithos-operation-facts-core"
OPERATION_PROFILE = "1.0.0-draft.1"
INVALID_OPERATION_FACTS = "InvalidOperationFacts"

DOMAINS = {
    "operation_facts": "aithos-core/v1/operation-facts",
    "gamma_read_request": "aithos-core/v1/gamma-read-request",
    "state_key": "aithos-core/v1/state-key",
}

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
TAG_RE = re.compile(r"^[a-z0-9_-]{1,64}$")
TOKEN_RE = re.compile(r"^[a-z][a-z0-9._-]{0,127}$")
ZULU_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$"
)
GAMMA_SELECTOR_ORDER = [
    "dir",
    "id",
    "tag",
    "kind",
    "action",
    "since",
    "until",
]


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


def commitment(domain: str, payload: bytes) -> str:
    digest = hashlib.sha256(domain.encode("ascii") + b"\x00" + payload).hexdigest()
    return f"sha256:{digest}"


def sid(number: int) -> str:
    value = f"01J{number:023d}"
    assert len(value) == 26 and SID_RE.fullmatch(value)
    return value


SID_PUBLIC = sid(101)
SID_CIRCLE = sid(102)
SID_SELF = sid(103)
SID_DIR_ROOT = sid(201)
SID_DIR_CHILD = sid(202)

SOURCE_HEAD = "sha256:" + hashlib.sha256(b"cb2-k1.2-r-source-head").hexdigest()
OTHER_HEAD = "sha256:" + hashlib.sha256(b"cb2-k1.2-r-other-head").hexdigest()
VAULT_STORE_KEY = "cb2-fixture/vault/mail/oauth"


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


def source_manifest() -> dict[str, Any]:
    return {
        "aithos-core": "1.0.0-draft.1",
        "edition": {
            "height": 7,
            "prev_hash": hashlib.sha256(b"cb2-k1.2-r-parent").hexdigest(),
            "created_at": "2026-07-18T10:00:00Z",
        },
        "files": {
            "e/circle/index.json": (
                "sha256:" + hashlib.sha256(b"circle-index").hexdigest()
            ),
            "gamma/2026-07.jsonl": (
                "sha256:" + hashlib.sha256(b"gamma-lines").hexdigest()
            ),
        },
        "roots": {
            "public": hashlib.sha256(b"public-root").hexdigest(),
            "circle": hashlib.sha256(b"circle-root").hexdigest(),
            "self": hashlib.sha256(b"self-root").hexdigest(),
            "vault": hashlib.sha256(b"vault-root").hexdigest(),
        },
        "gamma_roots": {
            "2026-07": {
                "root": hashlib.sha256(b"gamma-root").hexdigest(),
                "n": 4,
            }
        },
        "gamma_counts_root": hashlib.sha256(b"gamma-counts").hexdigest(),
        "gamma_head": SOURCE_HEAD,
        "signature": {
            "alg": "ed25519",
            "key": "#root",
            "value": "ab" * 64,
        },
    }


def manifest_chain_hash(manifest: dict[str, Any]) -> tuple[str, str]:
    unsigned = clone(manifest)
    signature = unsigned.get("signature")
    if not isinstance(signature, dict) or "value" not in signature:
        reject("source manifest has no signature.value")
    signature["value"] = ""
    preimage = jcs(unsigned)
    digest = hashlib.sha256(preimage.encode("utf-8")).hexdigest()
    return preimage, f"sha256:{digest}"


def parse_zulu(value: str) -> None:
    if not ZULU_RE.fullmatch(value):
        reject("Gamma query timestamp is not canonical Zulu")
    try:
        datetime.fromisoformat(value[:-1] + "+00:00")
    except ValueError:
        reject("Gamma query timestamp is not a calendar instant")


def canonical_gamma_query(value: Any) -> str:
    if value == "read.gamma":
        return value
    if not isinstance(value, str) or not value.startswith("read.gamma#"):
        reject("Gamma query is not a read.gamma perimeter string")
    selector_text = value[len("read.gamma#") :]
    if not selector_text:
        reject("Gamma query has an empty selector")

    parts = selector_text.split("&")
    parsed: list[tuple[str, str]] = []
    seen: set[str] = set()
    last_order = -1
    for part in parts:
        if "=" not in part:
            reject("Gamma query selector has no value")
        name, selector_value = part.split("=", 1)
        if name not in GAMMA_SELECTOR_ORDER:
            reject("Gamma query has an unknown selector")
        if name in seen:
            reject("Gamma query has a duplicate selector")
        order = GAMMA_SELECTOR_ORDER.index(name)
        if order <= last_order:
            reject("Gamma query selectors are not in canonical order")
        if not selector_value:
            reject("Gamma query selector is empty")
        seen.add(name)
        last_order = order

        if name == "dir":
            segments = selector_value.split("/")
            if any(not SID_RE.fullmatch(segment) for segment in segments):
                reject("Gamma query dir has a non-canonical SID")
            if len(segments) != len(set(segments)):
                reject("Gamma query dir has a duplicate SID")
        elif name == "id":
            if not SID_RE.fullmatch(selector_value):
                reject("Gamma query id has a non-canonical SID")
        elif name == "tag":
            if not TAG_RE.fullmatch(selector_value):
                reject("Gamma query tag is non-canonical")
        elif name in {"kind", "action"}:
            if not TOKEN_RE.fullmatch(selector_value):
                reject(f"Gamma query {name} is non-canonical")
        elif name in {"since", "until"}:
            parse_zulu(selector_value)
        parsed.append((name, selector_value))

    rendered = "read.gamma#" + "&".join(
        f"{name}={selector_value}" for name, selector_value in parsed
    )
    if rendered != value:
        reject("Gamma query does not round-trip canonically")
    return rendered


def query_digest(query: str) -> str:
    canonical = canonical_gamma_query(query)
    return commitment(DOMAINS["gamma_read_request"], canonical.encode("utf-8"))


def operation_case(
    case_id: str,
    facts: dict[str, Any],
    context: dict[str, Any],
) -> dict[str, Any]:
    document = {
        OPERATION_PROFILE_KEY: OPERATION_PROFILE,
        "kind": "read",
        "facts": facts,
    }
    document_jcs = jcs(document)
    digest = commitment(DOMAINS["operation_facts"], document_jcs.encode("utf-8"))
    return {
        "id": case_id,
        "context": context,
        "facts": facts,
        "document": document,
        "document_jcs": document_jcs,
        "facts_ref": {
            OPERATION_PROFILE_KEY: OPERATION_PROFILE,
            "digest": digest,
        },
        "digest": digest,
    }


def build_fixtures() -> dict[str, Any]:
    manifest = source_manifest()
    preimage, source_edition = manifest_chain_hash(manifest)
    unfiltered = "read.gamma"
    filtered = (
        f"read.gamma#dir={SID_DIR_ROOT}/{SID_DIR_CHILD}"
        f"&id={SID_CIRCLE}&tag=alpha&kind=action&action=reply"
        "&since=2026-07-01T00:00:00Z&until=2026-07-18T10:00:00Z"
    )
    return {
        "source_manifest": {
            "document": manifest,
            "document_jcs": jcs(manifest),
            "chain_hash_preimage_jcs": preimage,
            "source_edition": source_edition,
        },
        "source_head": SOURCE_HEAD,
        "queries": {
            "unfiltered": {
                "canonical": unfiltered,
                "request_digest": query_digest(unfiltered),
            },
            "filtered": {
                "canonical": filtered,
                "request_digest": query_digest(filtered),
            },
        },
        "vault": {
            "connector": "mail",
            "store_key_utf8": VAULT_STORE_KEY,
            "record_key": commitment(
                DOMAINS["state_key"],
                VAULT_STORE_KEY.encode("utf-8"),
            ),
        },
    }


def build_positive_cases(fixtures: dict[str, Any]) -> list[dict[str, Any]]:
    source_edition = fixtures["source_manifest"]["source_edition"]
    cases = [
        operation_case(
            "ethos-public",
            {
                "domain": "ethos",
                "zone": "public",
                "sid": SID_PUBLIC,
                "source_edition": source_edition,
            },
            {
                "source_manifest": "source_manifest",
                "zone": "public",
                "sid": SID_PUBLIC,
            },
        ),
        operation_case(
            "ethos-circle",
            {
                "domain": "ethos",
                "zone": "circle",
                "sid": SID_CIRCLE,
                "source_edition": source_edition,
            },
            {
                "source_manifest": "source_manifest",
                "zone": "circle",
                "sid": SID_CIRCLE,
            },
        ),
        operation_case(
            "ethos-self",
            {
                "domain": "ethos",
                "zone": "self",
                "sid": SID_SELF,
                "source_edition": source_edition,
            },
            {
                "source_manifest": "source_manifest",
                "zone": "self",
                "sid": SID_SELF,
            },
        ),
        operation_case(
            "gamma-unfiltered",
            {
                "domain": "gamma",
                "source_head": fixtures["source_head"],
                "request_digest": fixtures["queries"]["unfiltered"]["request_digest"],
            },
            {
                "source_head": fixtures["source_head"],
                "query": fixtures["queries"]["unfiltered"]["canonical"],
            },
        ),
        operation_case(
            "gamma-filtered",
            {
                "domain": "gamma",
                "source_head": fixtures["source_head"],
                "request_digest": fixtures["queries"]["filtered"]["request_digest"],
            },
            {
                "source_head": fixtures["source_head"],
                "query": fixtures["queries"]["filtered"]["canonical"],
            },
        ),
        operation_case(
            "vault-config",
            {
                "domain": "vault-config",
                "connector": fixtures["vault"]["connector"],
                "record_key": fixtures["vault"]["record_key"],
                "source_edition": source_edition,
            },
            {
                "source_manifest": "source_manifest",
                "connector": fixtures["vault"]["connector"],
                "store_key_utf8": fixtures["vault"]["store_key_utf8"],
            },
        ),
    ]
    return cases


def validate_read_facts(
    facts: Any,
    context: dict[str, Any],
    fixtures: dict[str, Any],
) -> dict[str, Any]:
    if not isinstance(facts, dict):
        reject("read facts is not an object")
    if any(value is None for value in facts.values()):
        reject("read facts contains null")
    domain = facts.get("domain")

    if domain == "ethos":
        value = require_exact_object(
            facts,
            {"domain", "zone", "sid", "source_edition"},
            "Ethos read facts",
        )
        if value["zone"] not in {"public", "circle", "self"}:
            reject("unknown Ethos read zone")
        if not isinstance(value["sid"], str) or not SID_RE.fullmatch(value["sid"]):
            reject("non-canonical Ethos read SID")
        require_commitment(value["source_edition"], "source_edition")
        if value["zone"] != context["zone"] or value["sid"] != context["sid"]:
            reject("Ethos native target mismatch")
        expected_edition = fixtures[context["source_manifest"]]["source_edition"]
        if value["source_edition"] != expected_edition:
            reject("Ethos source edition mismatch")

    elif domain == "gamma":
        value = require_exact_object(
            facts,
            {"domain", "source_head", "request_digest"},
            "Gamma read facts",
        )
        source_head = require_commitment(value["source_head"], "source_head")
        request = require_commitment(value["request_digest"], "request_digest")
        if source_head != context["source_head"]:
            reject("Gamma source head mismatch")
        canonical_query = canonical_gamma_query(context["query"])
        expected_request = commitment(
            DOMAINS["gamma_read_request"],
            canonical_query.encode("utf-8"),
        )
        if request != expected_request:
            reject("Gamma request digest mismatch")

    elif domain == "vault-config":
        value = require_exact_object(
            facts,
            {"domain", "connector", "record_key", "source_edition"},
            "vault-config read facts",
        )
        if value["connector"] != context["connector"]:
            reject("vault connector mismatch")
        record_key = require_commitment(value["record_key"], "vault record_key")
        expected_record_key = commitment(
            DOMAINS["state_key"],
            context["store_key_utf8"].encode("utf-8"),
        )
        if record_key != expected_record_key:
            reject("vault record_key mismatch")
        require_commitment(value["source_edition"], "source_edition")
        expected_edition = fixtures[context["source_manifest"]]["source_edition"]
        if value["source_edition"] != expected_edition:
            reject("vault source edition mismatch")

    else:
        reject("unknown read domain")
    return value


def validate_operation_document(
    document: Any,
    facts_ref: Any | None,
    context: dict[str, Any],
    fixtures: dict[str, Any],
) -> dict[str, Any]:
    value = require_exact_object(
        document,
        {OPERATION_PROFILE_KEY, "kind", "facts"},
        "operation-facts document",
    )
    if value[OPERATION_PROFILE_KEY] != OPERATION_PROFILE:
        reject("unknown operation-facts profile")
    if value["kind"] != "read":
        reject("operation kind is not read")
    validate_read_facts(value["facts"], context, fixtures)

    if facts_ref is not None:
        reference = require_exact_object(
            facts_ref,
            {OPERATION_PROFILE_KEY, "digest"},
            "facts_ref",
        )
        if reference[OPERATION_PROFILE_KEY] != value[OPERATION_PROFILE_KEY]:
            reject("facts_ref profile mismatch")
        digest = require_commitment(reference["digest"], "facts_ref digest")
        expected = commitment(
            DOMAINS["operation_facts"],
            jcs(value).encode("utf-8"),
        )
        if digest != expected:
            reject("facts_ref digest mismatch")
    return value


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
    facts_ref: dict[str, Any] | None = clone(base["facts_ref"])
    original_ref = clone(facts_ref)
    mutate(candidate, context, facts_ref)
    reference_was_mutated = facts_ref != original_ref
    if not reference_was_mutated:
        profile = candidate.get(OPERATION_PROFILE_KEY)
        if isinstance(profile, str):
            facts_ref = {
                OPERATION_PROFILE_KEY: profile,
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
    cases: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    ethos = case_by_id(cases, "ethos-circle")
    gamma = case_by_id(cases, "gamma-filtered")
    vault = case_by_id(cases, "vault-config")

    def noncanonical_query(
        document: dict[str, Any],
        context: dict[str, Any],
        _ref: dict[str, Any],
    ) -> None:
        query = context["query"]
        query = query.replace(
            f"dir={SID_DIR_ROOT}/{SID_DIR_CHILD}&id={SID_CIRCLE}",
            f"id={SID_CIRCLE}&dir={SID_DIR_ROOT}/{SID_DIR_CHILD}",
        )
        context["query"] = query
        document["facts"]["request_digest"] = commitment(
            DOMAINS["gamma_read_request"],
            query.encode("utf-8"),
        )

    def duplicate_query_selector(
        document: dict[str, Any],
        context: dict[str, Any],
        _ref: dict[str, Any],
    ) -> None:
        query = context["query"].replace("&kind=action", "&kind=action&kind=action")
        context["query"] = query
        document["facts"]["request_digest"] = commitment(
            DOMAINS["gamma_read_request"],
            query.encode("utf-8"),
        )

    negatives = [
        negative_case(
            "missing-envelope-profile",
            "missing operation-facts profile",
            ethos,
            lambda document, _context, _ref: document.pop(OPERATION_PROFILE_KEY),
        ),
        negative_case(
            "extra-envelope-member",
            "extra operation-facts envelope member",
            ethos,
            lambda document, _context, _ref: document.__setitem__(
                "nonce", "forbidden"
            ),
        ),
        negative_case(
            "kind-family-mismatch",
            "read facts selected by another operation kind",
            ethos,
            lambda document, _context, _ref: document.__setitem__(
                "kind", "mutation"
            ),
        ),
        negative_case(
            "unknown-read-domain",
            "unknown read domain",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "domain", "unknown"
            ),
        ),
        negative_case(
            "missing-read-member",
            "missing exact read-family member",
            ethos,
            lambda document, _context, _ref: document["facts"].pop("sid"),
        ),
        negative_case(
            "extra-read-member",
            "extra read-family member",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "tag", "forbidden"
            ),
        ),
        negative_case(
            "null-source-edition",
            "null source edition",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "source_edition", None
            ),
        ),
        negative_case(
            "unknown-ethos-zone",
            "unknown Ethos zone",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "zone", "team"
            ),
        ),
        negative_case(
            "noncanonical-ethos-sid",
            "non-canonical Ethos SID",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "sid", "not-a-sid"
            ),
        ),
        negative_case(
            "mismatched-ethos-target",
            "Ethos native target mismatch",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "sid", SID_PUBLIC
            ),
        ),
        negative_case(
            "malformed-source-edition",
            "malformed source edition",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "source_edition", "sha256:" + "A" * 64
            ),
        ),
        negative_case(
            "mismatched-source-edition",
            "source edition does not match the verified manifest",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "source_edition", "sha256:" + "0" * 64
            ),
        ),
        negative_case(
            "empty-source-head",
            "empty Gamma source head",
            gamma,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "source_head", ""
            ),
        ),
        negative_case(
            "mismatched-source-head",
            "Gamma source head mismatch",
            gamma,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "source_head", OTHER_HEAD
            ),
        ),
        negative_case(
            "noncanonical-gamma-query",
            "non-canonical Gamma query selector order",
            gamma,
            noncanonical_query,
        ),
        negative_case(
            "duplicate-gamma-selector",
            "duplicate Gamma query selector",
            gamma,
            duplicate_query_selector,
        ),
        negative_case(
            "mismatched-request-digest",
            "Gamma request digest mismatch",
            gamma,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "request_digest", "sha256:" + "0" * 64
            ),
        ),
        negative_case(
            "mismatched-vault-connector",
            "vault connector differs from native evidence",
            vault,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "connector", "calendar"
            ),
        ),
        negative_case(
            "mismatched-vault-record-key",
            "vault record-key commitment mismatch",
            vault,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "record_key", "sha256:" + "1" * 64
            ),
        ),
        negative_case(
            "clear-display-path",
            "forbidden clear display path",
            ethos,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "path", "/Projets/Secret"
            ),
        ),
        negative_case(
            "clear-vault-record-name",
            "forbidden clear vault record name",
            vault,
            lambda document, _context, _ref: document["facts"].__setitem__(
                "record_name", "oauth"
            ),
        ),
    ]
    assert len({case["id"] for case in negatives}) == len(negatives)
    return negatives


def assert_oracle_classification(
    fixtures: dict[str, Any],
    positive_cases: list[dict[str, Any]],
    negative_cases: list[dict[str, Any]],
) -> None:
    manifest = fixtures["source_manifest"]
    assert jcs(manifest["document"]) == manifest["document_jcs"]
    preimage, source_edition = manifest_chain_hash(manifest["document"])
    assert preimage == manifest["chain_hash_preimage_jcs"]
    assert source_edition == manifest["source_edition"]

    for query in fixtures["queries"].values():
        assert canonical_gamma_query(query["canonical"]) == query["canonical"]
        assert query_digest(query["canonical"]) == query["request_digest"]

    assert fixtures["vault"]["record_key"] == commitment(
        DOMAINS["state_key"],
        fixtures["vault"]["store_key_utf8"].encode("utf-8"),
    )

    fixture_refs = {
        "source_manifest": fixtures["source_manifest"],
    }
    for case in positive_cases:
        validate_operation_document(
            case["document"],
            case["facts_ref"],
            case["context"],
            fixture_refs,
        )
        assert jcs(case["document"]) == case["document_jcs"], case["id"]
        assert commitment(
            DOMAINS["operation_facts"],
            case["document_jcs"].encode("utf-8"),
        ) == case["digest"], case["id"]

    for case in negative_cases:
        try:
            validate_operation_document(
                case["candidate"],
                case["facts_ref"],
                case["context"],
                fixture_refs,
            )
        except OracleError as error:
            assert error.code == case["must_fail"], case["id"]
        else:
            raise AssertionError(f"read negative unexpectedly accepted: {case['id']}")


def build_vector() -> dict[str, Any]:
    fixtures = build_fixtures()
    positive_cases = build_positive_cases(fixtures)
    negative_cases = build_negative_cases(positive_cases)
    assert_oracle_classification(fixtures, positive_cases, negative_cases)

    return {
        "vector": "CB2-K1-OPERATION-FACTS-READ-1",
        "description": (
            "Independent Python standard-library oracle for K1.2-R-B Ethos, "
            "signed Gamma-presentation and vault-config read facts. The query "
            "string and cb2-fixture store key are protected oracle inputs only; "
            "they define no public carrier or Bundle path."
        ),
        "profile": {
            OPERATION_PROFILE_KEY: OPERATION_PROFILE,
        },
        "commitment_domains": DOMAINS,
        "historical_vector_sha256": {
            "e1-mandate.json": (
                "e243e2348f3778c8e9ec9a3bbc350d480aeb779c4cadac0f8d00f9007264d45e"
            ),
            "f1-gamma-chain.json": (
                "38d466f934f17e99acbbdb76fb236efafb36f7b1c4070e9729d6edca17768ca3"
            ),
            "f2-gamma-counting.json": (
                "d593d204144ffa81c2ba51393d8190081d80f29246f41fdfa3fed96db77d230f"
            ),
            "gplus-obligations.json": (
                "f74f6c2b4611e798c0c00bd35ccdfb056e26de2d558efc0e2c3ad04ae57a7285"
            ),
        },
        "manifest_chain_hash_rule": (
            "sha256: prefix plus SHA-256 of RFC8785-JCS after replacing only "
            "signature.value with the empty string"
        ),
        "source_manifest_fixture_scope": (
            "Chain-hash preimage fixture only. Its signature block is syntactic; "
            "source signature and edition validation belong to the future typed "
            "Core front door and are not claimed GREEN by this consumer."
        ),
        "gamma_query_rule": {
            "prefix": "read.gamma",
            "selector_order": GAMMA_SELECTOR_ORDER,
            "digest_preimage": (
                "ASCII(aithos-core/v1/gamma-read-request) || NUL || "
                "UTF8(canonical read.gamma string)"
            ),
        },
        "fixtures": fixtures,
        "positive_cases": positive_cases,
        "negative_cases": negative_cases,
        "inventory": {
            "positive_case_ids": [case["id"] for case in positive_cases],
            "negative_case_ids": [case["id"] for case in negative_cases],
            "required_error_variant": INVALID_OPERATION_FACTS,
        },
    }


def encoded_vector() -> str:
    return json.dumps(
        build_vector(),
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
    ) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    expected = encoded_vector()

    if args.check:
        if not args.output.exists():
            raise SystemExit(f"missing {args.output}")
        actual = args.output.read_text(encoding="utf-8")
        if actual != expected:
            raise SystemExit(f"{args.output} is not reproducible")
        print(f"verified {args.output}")
        return

    args.output.write_text(expected, encoding="utf-8")
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Independent CB2 oracle for K1.1-B and K1.2-M-B mutation facts.

The generator uses only Python's standard library.  It independently computes
RFC 8785-compatible canonical JSON for this string/array/object-only fixture,
all four domain-separated SHA-256 commitments, the closed mutation member
tables, state transitions, structural coordinates, and exact negative error
taxonomy.

The ``cb2-fixture/...`` store-key strings are opaque canonical-key inputs to the
commitment algorithm.  They do not assign a Bundle path or sidecar layout.

Usage:
    python3 vectors/gen-cb2-operation-facts-mutation.py
    python3 vectors/gen-cb2-operation-facts-mutation.py --check
    python3 vectors/gen-cb2-operation-facts-mutation.py --output /tmp/cb2.json
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
from typing import Any, NoReturn


HERE = Path(__file__).resolve().parent
DEFAULT_OUTPUT = HERE / "cb2-operation-facts-mutation.json"

OPERATION_PROFILE = "1.0.0-draft.1"
STATE_PROFILE = "1.0.0-draft.1"
OPERATION_PROFILE_KEY = "aithos-operation-facts-core"
STATE_PROFILE_KEY = "aithos-state-fact-core"

DOMAINS = {
    "operation_facts": "aithos-core/v1/operation-facts",
    "state_fact": "aithos-core/v1/state-fact",
    "state_key": "aithos-core/v1/state-key",
    "state_bytes": "aithos-core/v1/state-bytes",
}

INVALID_OPERATION_FACTS = "InvalidOperationFacts"
INVALID_STATE_FACT = "InvalidStateFact"

COMMITMENT_RE = re.compile(r"^sha256:[0-9a-f]{64}$")
SID_RE = re.compile(r"^[0-9A-HJKMNP-TV-Z]{26}$")
CONNECTOR_RE = re.compile(r"^[a-z][a-z0-9-]{0,63}$")


class OracleError(ValueError):
    def __init__(self, code: str, detail: str):
        super().__init__(detail)
        self.code = code


def reject(code: str, detail: str) -> NoReturn:
    raise OracleError(code, detail)


def clone(value: Any) -> Any:
    return copy.deepcopy(value)


def jcs(value: Any) -> str:
    """RFC 8785-compatible encoding for the fixture's restricted value domain."""

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


SID_CIRCLE_ROOT = sid(1)
SID_CIRCLE_PARENT = sid(2)
SID_CIRCLE_DESTINATION = sid(3)
SID_CIRCLE_DESTINATION_CHILD = sid(4)
SID_SELF_ROOT = sid(5)
SID_ETHOS_TARGET = sid(10)
SID_SECTION_TARGET = sid(11)
SID_FOLDER_TARGET = sid(20)
SID_FOLDER_CREATE = sid(21)

SID_ZONE = {
    SID_CIRCLE_ROOT: "circle",
    SID_CIRCLE_PARENT: "circle",
    SID_CIRCLE_DESTINATION: "circle",
    SID_CIRCLE_DESTINATION_CHILD: "circle",
    SID_SELF_ROOT: "self",
    SID_ETHOS_TARGET: "circle",
    SID_SECTION_TARGET: "circle",
    SID_FOLDER_TARGET: "circle",
    SID_FOLDER_CREATE: "circle",
}

PARENT = {
    SID_CIRCLE_ROOT: None,
    SID_CIRCLE_PARENT: SID_CIRCLE_ROOT,
    SID_CIRCLE_DESTINATION: None,
    SID_CIRCLE_DESTINATION_CHILD: SID_CIRCLE_DESTINATION,
    SID_SELF_ROOT: None,
    SID_ETHOS_TARGET: SID_CIRCLE_PARENT,
    SID_SECTION_TARGET: SID_CIRCLE_PARENT,
    SID_FOLDER_TARGET: SID_CIRCLE_PARENT,
}


def require_exact_object(
    value: Any,
    expected_keys: set[str],
    code: str,
    label: str,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        reject(code, f"{label} is not an object")
    if set(value) != expected_keys:
        reject(code, f"{label} has a non-exact member set")
    if any(member is None for member in value.values()):
        reject(code, f"{label} contains null")
    return value


def require_commitment(value: Any, code: str, label: str) -> str:
    if not isinstance(value, str) or not COMMITMENT_RE.fullmatch(value):
        reject(code, f"{label} is not strict lowercase sha256 text")
    return value


def state_object(store_key: str, stored_bytes: bytes) -> dict[str, str]:
    return {
        "key_commitment": commitment(DOMAINS["state_key"], store_key.encode("utf-8")),
        "byte_commitment": commitment(DOMAINS["state_bytes"], stored_bytes),
    }


def state_fixture(
    inputs: list[tuple[str, bytes]],
) -> dict[str, Any]:
    objects = [
        {
            "store_key_utf8": store_key,
            "stored_bytes_hex": stored_bytes.hex(),
            **state_object(store_key, stored_bytes),
        }
        for store_key, stored_bytes in inputs
    ]
    objects.sort(key=lambda item: item["key_commitment"])
    document = {
        STATE_PROFILE_KEY: STATE_PROFILE,
        "objects": [
            {
                "key_commitment": item["key_commitment"],
                "byte_commitment": item["byte_commitment"],
            }
            for item in objects
        ],
    }
    document_jcs = jcs(document)
    return {
        "input_objects": objects,
        "document": document,
        "document_jcs": document_jcs,
        "digest": commitment(DOMAINS["state_fact"], document_jcs.encode("utf-8")),
    }


def state_ref(state: dict[str, Any]) -> dict[str, Any]:
    return {
        "state": "present",
        "state_ref": {
            STATE_PROFILE_KEY: STATE_PROFILE,
            "digest": state["digest"],
        },
    }


ABSENT = {"state": "absent"}


def validate_state_document(
    document: Any,
    *,
    expected_keys: set[str] | None = None,
) -> dict[str, Any]:
    value = require_exact_object(
        document,
        {STATE_PROFILE_KEY, "objects"},
        INVALID_STATE_FACT,
        "state-fact document",
    )
    if value[STATE_PROFILE_KEY] != STATE_PROFILE:
        reject(INVALID_STATE_FACT, "unknown state-fact profile")
    objects = value["objects"]
    if not isinstance(objects, list) or not objects:
        reject(INVALID_STATE_FACT, "objects is not a non-empty array")

    keys: list[str] = []
    for index, candidate in enumerate(objects):
        item = require_exact_object(
            candidate,
            {"key_commitment", "byte_commitment"},
            INVALID_STATE_FACT,
            f"state object {index}",
        )
        key = require_commitment(
            item["key_commitment"],
            INVALID_STATE_FACT,
            f"state object {index} key",
        )
        require_commitment(
            item["byte_commitment"],
            INVALID_STATE_FACT,
            f"state object {index} bytes",
        )
        keys.append(key)

    if len(keys) != len(set(keys)):
        reject(INVALID_STATE_FACT, "duplicate key commitment")
    if keys != sorted(keys):
        reject(INVALID_STATE_FACT, "unsorted objects")
    if expected_keys is not None:
        actual = set(keys)
        if actual != expected_keys:
            reject(INVALID_STATE_FACT, "affected object set mismatch")
    return value


def validate_state_value(
    value: Any,
    state_catalog: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    if not isinstance(value, dict) or "state" not in value:
        reject(INVALID_STATE_FACT, "logical state is not a tagged object")
    if value["state"] == "absent":
        return require_exact_object(
            value,
            {"state"},
            INVALID_STATE_FACT,
            "absent state",
        )
    if value["state"] != "present":
        reject(INVALID_STATE_FACT, "unknown logical state")

    present = require_exact_object(
        value,
        {"state", "state_ref"},
        INVALID_STATE_FACT,
        "present state",
    )
    reference = require_exact_object(
        present["state_ref"],
        {STATE_PROFILE_KEY, "digest"},
        INVALID_STATE_FACT,
        "state_ref",
    )
    if reference[STATE_PROFILE_KEY] != STATE_PROFILE:
        reject(INVALID_STATE_FACT, "unknown state_ref profile")
    digest = require_commitment(
        reference["digest"],
        INVALID_STATE_FACT,
        "state_ref digest",
    )
    if digest not in state_catalog:
        reject(INVALID_STATE_FACT, "state_ref digest mismatch")
    fixture = state_catalog[digest]
    validate_state_document(fixture["document"])
    expected = commitment(
        DOMAINS["state_fact"],
        jcs(fixture["document"]).encode("utf-8"),
    )
    if digest != expected:
        reject(INVALID_STATE_FACT, "state_ref digest mismatch")
    return present


def validate_path(
    value: Any,
    *,
    zone: str,
    target: str,
    label: str,
) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        reject(INVALID_OPERATION_FACTS, f"{label} is not a SID array")
    if any(not SID_RE.fullmatch(item) for item in value):
        reject(INVALID_OPERATION_FACTS, f"{label} has non-canonical SID")
    if len(value) != len(set(value)):
        reject(INVALID_OPERATION_FACTS, f"{label} has duplicate SID")
    if target in value:
        reject(INVALID_OPERATION_FACTS, f"{label} includes target SID")
    if any(SID_ZONE.get(item) != zone for item in value):
        reject(INVALID_OPERATION_FACTS, f"{label} crosses zones")
    if value:
        if PARENT.get(value[0]) is not None:
            reject(INVALID_OPERATION_FACTS, f"{label} does not start at a root")
        for parent, child in zip(value, value[1:]):
            if PARENT.get(child) != parent:
                reject(INVALID_OPERATION_FACTS, f"{label} is not root-to-leaf")
    return value


def current_parent_path(target: str) -> list[str]:
    path: list[str] = []
    current = PARENT.get(target)
    while current is not None:
        path.append(current)
        current = PARENT.get(current)
    path.reverse()
    return path


def validate_transition(
    verb: str,
    before: dict[str, Any],
    after: dict[str, Any],
) -> None:
    before_state = before["state"]
    after_state = after["state"]
    expected = {
        "create": ("absent", "present"),
        "delete": ("present", "absent"),
        "edit": ("present", "present"),
        "redact": ("present", "present"),
        "rename": ("present", "present"),
        "move": ("present", "present"),
    }[verb]
    if (before_state, after_state) != expected:
        reject(INVALID_OPERATION_FACTS, "invalid before/after transition")
    if expected == ("present", "present"):
        if before["state_ref"]["digest"] == after["state_ref"]["digest"]:
            reject(INVALID_OPERATION_FACTS, "equal state digests")


def validate_mutation_facts(
    facts: Any,
    state_catalog: dict[str, dict[str, Any]],
    *,
    expected_vault_record_key: str,
) -> dict[str, Any]:
    if not isinstance(facts, dict):
        reject(INVALID_OPERATION_FACTS, "facts is not an object")
    if any(value is None for value in facts.values()):
        reject(INVALID_OPERATION_FACTS, "facts contains null")
    domain = facts.get("domain")

    if domain == "ethos":
        expected = {"domain", "verb", "zone", "sid", "dir", "before", "after"}
        value = require_exact_object(
            facts,
            expected,
            INVALID_OPERATION_FACTS,
            "ethos mutation facts",
        )
        verb = value["verb"]
        if verb not in {"create", "edit", "delete", "redact"}:
            reject(INVALID_OPERATION_FACTS, "unknown ethos verb")
        zone = value["zone"]
        if zone not in {"public", "circle", "self"}:
            reject(INVALID_OPERATION_FACTS, "unknown ethos zone")
        target = value["sid"]
        if not isinstance(target, str) or not SID_RE.fullmatch(target):
            reject(INVALID_OPERATION_FACTS, "non-canonical target SID")
        if SID_ZONE.get(target) != zone:
            reject(INVALID_OPERATION_FACTS, "target SID crosses zones")
        path = validate_path(value["dir"], zone=zone, target=target, label="dir")
        if verb != "create" and path != current_parent_path(target):
            reject(INVALID_OPERATION_FACTS, "dir is not the current parent path")

    elif domain == "structure":
        verb = facts.get("verb")
        expected_by_verb = {
            "create": {
                "domain", "verb", "zone", "node_kind", "sid", "destination",
                "before", "after",
            },
            "rename": {
                "domain", "verb", "zone", "node_kind", "sid", "source",
                "before", "after",
            },
            "delete": {
                "domain", "verb", "zone", "node_kind", "sid", "source",
                "before", "after",
            },
            "move": {
                "domain", "verb", "zone", "node_kind", "sid", "source",
                "destination", "before", "after",
            },
        }
        if verb not in expected_by_verb:
            reject(INVALID_OPERATION_FACTS, "unknown structure verb")
        value = require_exact_object(
            facts,
            expected_by_verb[verb],
            INVALID_OPERATION_FACTS,
            "structure mutation facts",
        )
        zone = value["zone"]
        if zone not in {"public", "circle", "self"}:
            reject(INVALID_OPERATION_FACTS, "unknown structure zone")
        node_kind = value["node_kind"]
        if node_kind not in {"folder", "section"}:
            reject(INVALID_OPERATION_FACTS, "unknown node kind")
        if verb in {"create", "delete"} and node_kind != "folder":
            reject(INVALID_OPERATION_FACTS, f"structure {verb} requires folder")
        target = value["sid"]
        if not isinstance(target, str) or not SID_RE.fullmatch(target):
            reject(INVALID_OPERATION_FACTS, "non-canonical target SID")
        if SID_ZONE.get(target) != zone:
            reject(INVALID_OPERATION_FACTS, "target SID crosses zones")
        if "source" in value:
            source = validate_path(
                value["source"],
                zone=zone,
                target=target,
                label="source",
            )
            if source != current_parent_path(target):
                reject(INVALID_OPERATION_FACTS, "source is not the current parent path")
        if "destination" in value:
            validate_path(
                value["destination"],
                zone=zone,
                target=target,
                label="destination",
            )

    elif domain == "vault-config":
        value = require_exact_object(
            facts,
            {"domain", "verb", "connector", "record_key", "before", "after"},
            INVALID_OPERATION_FACTS,
            "vault-config mutation facts",
        )
        verb = value["verb"]
        if verb not in {"create", "edit", "delete"}:
            reject(INVALID_OPERATION_FACTS, "unknown vault-config verb")
        if (
            not isinstance(value["connector"], str)
            or not CONNECTOR_RE.fullmatch(value["connector"])
        ):
            reject(INVALID_OPERATION_FACTS, "non-canonical connector")
        record_key = require_commitment(
            value["record_key"],
            INVALID_OPERATION_FACTS,
            "vault record_key",
        )
        if record_key != expected_vault_record_key:
            reject(INVALID_OPERATION_FACTS, "mismatched vault record_key")

    else:
        reject(INVALID_OPERATION_FACTS, "unknown mutation domain")

    before = validate_state_value(value["before"], state_catalog)
    after = validate_state_value(value["after"], state_catalog)
    validate_transition(value["verb"], before, after)

    if domain == "vault-config":
        record_key = value["record_key"]
        for state in (before, after):
            if state["state"] == "present":
                state_document = state_catalog[state["state_ref"]["digest"]]["document"]
                if not any(
                    item["key_commitment"] == record_key
                    for item in state_document["objects"]
                ):
                    reject(
                        INVALID_OPERATION_FACTS,
                        "vault record_key absent from present state",
                    )
    return value


def validate_operation_document(
    document: Any,
    state_catalog: dict[str, dict[str, Any]],
    *,
    expected_vault_record_key: str,
    facts_ref: Any | None = None,
) -> dict[str, Any]:
    value = require_exact_object(
        document,
        {OPERATION_PROFILE_KEY, "kind", "facts"},
        INVALID_OPERATION_FACTS,
        "operation-facts document",
    )
    if value[OPERATION_PROFILE_KEY] != OPERATION_PROFILE:
        reject(INVALID_OPERATION_FACTS, "unknown operation-facts profile")
    if value["kind"] != "mutation":
        reject(INVALID_OPERATION_FACTS, "operation kind is not mutation")
    validate_mutation_facts(
        value["facts"],
        state_catalog,
        expected_vault_record_key=expected_vault_record_key,
    )

    if facts_ref is not None:
        reference = require_exact_object(
            facts_ref,
            {OPERATION_PROFILE_KEY, "digest"},
            INVALID_OPERATION_FACTS,
            "facts_ref",
        )
        if reference[OPERATION_PROFILE_KEY] != value[OPERATION_PROFILE_KEY]:
            reject(INVALID_OPERATION_FACTS, "facts_ref profile mismatch")
        digest = require_commitment(
            reference["digest"],
            INVALID_OPERATION_FACTS,
            "facts_ref digest",
        )
        expected = commitment(
            DOMAINS["operation_facts"],
            jcs(value).encode("utf-8"),
        )
        if digest != expected:
            reject(INVALID_OPERATION_FACTS, "facts_ref digest mismatch")
    return value


def operation_case(case_id: str, variant: str, facts: dict[str, Any]) -> dict[str, Any]:
    document = {
        OPERATION_PROFILE_KEY: OPERATION_PROFILE,
        "kind": "mutation",
        "facts": facts,
    }
    document_jcs = jcs(document)
    digest = commitment(DOMAINS["operation_facts"], document_jcs.encode("utf-8"))
    return {
        "id": case_id,
        "variant": variant,
        "facts": facts,
        "document": document,
        "document_jcs": document_jcs,
        "facts_ref": {
            OPERATION_PROFILE_KEY: OPERATION_PROFILE,
            "digest": digest,
        },
        "digest": digest,
    }


def present(states: dict[str, dict[str, Any]], name: str) -> dict[str, Any]:
    return state_ref(states[name])


def build_states() -> dict[str, dict[str, Any]]:
    return {
        "ethos_v1": state_fixture([
            ("cb2-fixture/ethos/circle/index", b'{"sid":"section","rev":1}'),
            ("cb2-fixture/ethos/circle/blob", b"section body v1"),
        ]),
        "ethos_v2": state_fixture([
            ("cb2-fixture/ethos/circle/index", b'{"sid":"section","rev":2}'),
            ("cb2-fixture/ethos/circle/blob", b"section body v2"),
        ]),
        "ethos_redacted": state_fixture([
            ("cb2-fixture/ethos/circle/index", b'{"sid":"section","redacted":true}'),
            ("cb2-fixture/ethos/circle/blob", b""),
        ]),
        "folder_v1": state_fixture([
            ("cb2-fixture/structure/circle/index", b'{"folder":"target","rev":1}'),
            ("cb2-fixture/structure/circle/header", b"Folder target"),
        ]),
        "folder_v2": state_fixture([
            ("cb2-fixture/structure/circle/index", b'{"folder":"target","rev":2}'),
            ("cb2-fixture/structure/circle/header", b"Folder renamed"),
        ]),
        "folder_v3": state_fixture([
            ("cb2-fixture/structure/circle/index", b'{"folder":"target","rev":3}'),
            ("cb2-fixture/structure/circle/header", b"Folder moved"),
        ]),
        "section_v1": state_fixture([
            ("cb2-fixture/structure/circle/section", b'{"title":"Before"}'),
        ]),
        "section_v2": state_fixture([
            ("cb2-fixture/structure/circle/section", b'{"title":"Renamed"}'),
        ]),
        "section_v3": state_fixture([
            ("cb2-fixture/structure/circle/section", b'{"title":"Moved"}'),
        ]),
        "vault_v1": state_fixture([
            ("cb2-fixture/vault/mail/oauth", b'{"token":"ciphertext-v1"}'),
        ]),
        "vault_v2": state_fixture([
            ("cb2-fixture/vault/mail/oauth", b'{"token":"ciphertext-v2"}'),
        ]),
        "unrelated": state_fixture([
            ("cb2-fixture/unrelated/object", b"unrelated"),
        ]),
    }


def build_positive_cases(
    states: dict[str, dict[str, Any]],
    vault_record_key: str,
) -> list[dict[str, Any]]:
    parent = [SID_CIRCLE_ROOT, SID_CIRCLE_PARENT]
    destination = [SID_CIRCLE_DESTINATION]
    base_ethos = {
        "domain": "ethos",
        "zone": "circle",
        "sid": SID_ETHOS_TARGET,
        "dir": parent,
    }
    base_folder = {
        "domain": "structure",
        "zone": "circle",
        "node_kind": "folder",
        "sid": SID_FOLDER_TARGET,
    }
    base_section = {
        "domain": "structure",
        "zone": "circle",
        "node_kind": "section",
        "sid": SID_SECTION_TARGET,
    }
    base_vault = {
        "domain": "vault-config",
        "connector": "mail",
        "record_key": vault_record_key,
    }

    return [
        operation_case("ethos-create", "ethos", {
            **base_ethos, "verb": "create", "before": clone(ABSENT),
            "after": present(states, "ethos_v1"),
        }),
        operation_case("ethos-edit", "ethos", {
            **base_ethos, "verb": "edit", "before": present(states, "ethos_v1"),
            "after": present(states, "ethos_v2"),
        }),
        operation_case("ethos-delete", "ethos", {
            **base_ethos, "verb": "delete", "before": present(states, "ethos_v2"),
            "after": clone(ABSENT),
        }),
        operation_case("ethos-redact", "ethos", {
            **base_ethos, "verb": "redact", "before": present(states, "ethos_v2"),
            "after": present(states, "ethos_redacted"),
        }),
        operation_case("structure-create-folder", "structure-create", {
            "domain": "structure", "verb": "create", "zone": "circle",
            "node_kind": "folder", "sid": SID_FOLDER_CREATE,
            "destination": parent, "before": clone(ABSENT),
            "after": present(states, "folder_v1"),
        }),
        operation_case("structure-rename-folder", "structure-source", {
            **base_folder, "verb": "rename", "source": parent,
            "before": present(states, "folder_v1"),
            "after": present(states, "folder_v2"),
        }),
        operation_case("structure-rename-section", "structure-source", {
            **base_section, "verb": "rename", "source": parent,
            "before": present(states, "section_v1"),
            "after": present(states, "section_v2"),
        }),
        operation_case("structure-delete-folder", "structure-source", {
            **base_folder, "verb": "delete", "source": parent,
            "before": present(states, "folder_v2"), "after": clone(ABSENT),
        }),
        operation_case("structure-move-folder", "structure-move", {
            **base_folder, "verb": "move", "source": parent,
            "destination": destination, "before": present(states, "folder_v2"),
            "after": present(states, "folder_v3"),
        }),
        operation_case("structure-move-section", "structure-move", {
            **base_section, "verb": "move", "source": parent,
            "destination": destination, "before": present(states, "section_v2"),
            "after": present(states, "section_v3"),
        }),
        operation_case("vault-create", "vault-config", {
            **base_vault, "verb": "create", "before": clone(ABSENT),
            "after": present(states, "vault_v1"),
        }),
        operation_case("vault-edit", "vault-config", {
            **base_vault, "verb": "edit", "before": present(states, "vault_v1"),
            "after": present(states, "vault_v2"),
        }),
        operation_case("vault-delete", "vault-config", {
            **base_vault, "verb": "delete", "before": present(states, "vault_v2"),
            "after": clone(ABSENT),
        }),
    ]


def by_id(cases: list[dict[str, Any]], case_id: str) -> dict[str, Any]:
    return next(case for case in cases if case["id"] == case_id)


def operation_negative(
    case_id: str,
    defect: str,
    base: dict[str, Any],
    mutate: Any,
) -> dict[str, Any]:
    candidate = clone(base["document"])
    facts_ref = clone(base["facts_ref"])
    mutate(candidate, facts_ref)
    reference_was_mutated = facts_ref != base["facts_ref"]
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
        "must_fail": INVALID_OPERATION_FACTS,
    }


def build_operation_negatives(
    cases: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    ethos = by_id(cases, "ethos-edit")
    rename = by_id(cases, "structure-rename-folder")
    move = by_id(cases, "structure-move-folder")
    create = by_id(cases, "structure-create-folder")
    vault = by_id(cases, "vault-edit")

    negatives = [
        operation_negative(
            "missing-envelope-profile",
            "missing operation-facts profile",
            ethos,
            lambda document, _ref: document.pop(OPERATION_PROFILE_KEY),
        ),
        operation_negative(
            "extra-envelope-member",
            "extra operation-facts envelope member",
            ethos,
            lambda document, _ref: document.__setitem__("nonce", "forbidden"),
        ),
        operation_negative(
            "unknown-envelope-profile",
            "unknown operation-facts profile",
            ethos,
            lambda document, _ref: document.__setitem__(
                OPERATION_PROFILE_KEY, "9.9.9"
            ),
        ),
        operation_negative(
            "kind-family-mismatch",
            "facts family different from operation kind",
            ethos,
            lambda document, _ref: document.__setitem__("kind", "action"),
        ),
        operation_negative(
            "facts-ref-digest-mismatch",
            "facts_ref digest mismatch",
            ethos,
            lambda _document, ref: ref.__setitem__("digest", "sha256:" + "0" * 64),
        ),
        operation_negative(
            "unknown-domain",
            "unknown domain",
            ethos,
            lambda document, _ref: document["facts"].__setitem__(
                "domain", "unknown"
            ),
        ),
        operation_negative(
            "unknown-domain-verb",
            "unknown verb for the selected domain",
            ethos,
            lambda document, _ref: document["facts"].__setitem__("verb", "merge"),
        ),
        operation_negative(
            "missing-family-member",
            "missing exact family member",
            ethos,
            lambda document, _ref: document["facts"].pop("dir"),
        ),
        operation_negative(
            "clear-display-path",
            "forbidden clear display path",
            ethos,
            lambda document, _ref: document["facts"].__setitem__(
                "path", "/Projets/Secret"
            ),
        ),
        operation_negative(
            "unknown-zone",
            "unknown zone",
            ethos,
            lambda document, _ref: document["facts"].__setitem__("zone", "team"),
        ),
        operation_negative(
            "unknown-node-kind",
            "unknown node kind",
            rename,
            lambda document, _ref: document["facts"].__setitem__(
                "node_kind", "page"
            ),
        ),
        operation_negative(
            "section-create-in-structure",
            "structure create admits folder only",
            create,
            lambda document, _ref: document["facts"].__setitem__(
                "node_kind", "section"
            ),
        ),
        operation_negative(
            "null-source",
            "null source coordinate",
            rename,
            lambda document, _ref: document["facts"].__setitem__("source", None),
        ),
        operation_negative(
            "destination-on-rename",
            "destination on the wrong structural variant",
            rename,
            lambda document, _ref: document["facts"].__setitem__(
                "destination", [SID_CIRCLE_DESTINATION]
            ),
        ),
        operation_negative(
            "noncanonical-target-sid",
            "non-canonical target SID",
            ethos,
            lambda document, _ref: document["facts"].__setitem__(
                "sid", "not-a-sid"
            ),
        ),
        operation_negative(
            "duplicate-source-sid",
            "duplicate source SID",
            rename,
            lambda document, _ref: document["facts"].__setitem__(
                "source", [SID_CIRCLE_ROOT, SID_CIRCLE_ROOT]
            ),
        ),
        operation_negative(
            "noncanonical-source-order",
            "source is not root-to-leaf",
            rename,
            lambda document, _ref: document["facts"].__setitem__(
                "source", [SID_CIRCLE_PARENT, SID_CIRCLE_ROOT]
            ),
        ),
        operation_negative(
            "destination-contains-target",
            "destination is inside the moved target",
            move,
            lambda document, _ref: document["facts"].__setitem__(
                "destination", [SID_FOLDER_TARGET]
            ),
        ),
        operation_negative(
            "cross-zone-destination",
            "cross-zone structural destination",
            move,
            lambda document, _ref: document["facts"].__setitem__(
                "destination", [SID_SELF_ROOT]
            ),
        ),
        operation_negative(
            "invalid-create-transition",
            "invalid before and after transition",
            create,
            lambda document, _ref: document["facts"].__setitem__(
                "before", clone(by_id(cases, "structure-rename-folder")["facts"]["before"])
            ),
        ),
        operation_negative(
            "equal-present-state-digests",
            "equal state digests for a mutation",
            ethos,
            lambda document, _ref: document["facts"].__setitem__(
                "after", clone(document["facts"]["before"])
            ),
        ),
        operation_negative(
            "mismatched-vault-record-key",
            "mismatched vault record-key commitment",
            vault,
            lambda document, _ref: document["facts"].__setitem__(
                "record_key", "sha256:" + "1" * 64
            ),
        ),
        operation_negative(
            "clear-vault-record-name",
            "forbidden clear vault record name",
            vault,
            lambda document, _ref: document["facts"].__setitem__(
                "record_name", "oauth"
            ),
        ),
    ]
    assert len({case["id"] for case in negatives}) == len(negatives)
    return negatives


def state_negative(
    case_id: str,
    defect: str,
    scope: str,
    candidate: Any,
    **metadata: Any,
) -> dict[str, Any]:
    return {
        "id": case_id,
        "defect": defect,
        "scope": scope,
        "candidate": candidate,
        "must_fail": INVALID_STATE_FACT,
        **metadata,
    }


def build_state_negatives(
    states: dict[str, dict[str, Any]],
) -> list[dict[str, Any]]:
    ethos = states["ethos_v1"]
    one = states["section_v1"]
    unrelated = states["unrelated"]["document"]["objects"][0]

    absent_extra = {"state": "absent", "state_ref": {
        STATE_PROFILE_KEY: STATE_PROFILE,
        "digest": one["digest"],
    }}
    present_missing = {"state": "present"}
    present_bad_profile = state_ref(one)
    present_bad_profile["state_ref"][STATE_PROFILE_KEY] = "9.9.9"
    present_bad_digest = state_ref(one)
    present_bad_digest["state_ref"]["digest"] = "sha256:" + "A" * 64

    unknown_profile = clone(one["document"])
    unknown_profile[STATE_PROFILE_KEY] = "9.9.9"
    empty = {STATE_PROFILE_KEY: STATE_PROFILE, "objects": []}
    unsorted = clone(ethos["document"])
    unsorted["objects"].reverse()
    duplicate = clone(one["document"])
    duplicate["objects"].append(clone(duplicate["objects"][0]))
    missing_member = clone(one["document"])
    missing_member["objects"][0].pop("byte_commitment")
    extra_member = clone(one["document"])
    extra_member["objects"][0]["salt"] = "forbidden"
    malformed = clone(one["document"])
    malformed["objects"][0]["byte_commitment"] = "sha256:" + "A" * 64
    missing_affected = clone(ethos["document"])
    expected_ethos_keys = {
        item["key_commitment"] for item in ethos["document"]["objects"]
    }
    missing_affected["objects"].pop()
    unrelated_extra = clone(one["document"])
    unrelated_extra["objects"].append(clone(unrelated))
    unrelated_extra["objects"].sort(key=lambda item: item["key_commitment"])
    clear_store_key = clone(one["document"])
    clear_store_key["objects"][0]["store_key"] = (
        one["input_objects"][0]["store_key_utf8"]
    )

    valid_reference = {
        "logical_state": state_ref(one),
        "document": clone(one["document"]),
    }
    mismatched_reference = clone(valid_reference)
    mismatched_reference["logical_state"]["state_ref"]["digest"] = (
        "sha256:" + "0" * 64
    )

    negatives = [
        state_negative(
            "absent-state-has-reference",
            "absent state has forbidden state_ref",
            "logical_state",
            absent_extra,
        ),
        state_negative(
            "present-state-missing-reference",
            "present state is missing state_ref",
            "logical_state",
            present_missing,
        ),
        state_negative(
            "unknown-state-ref-profile",
            "unknown state_ref profile",
            "logical_state",
            present_bad_profile,
        ),
        state_negative(
            "nonlowercase-state-ref-digest",
            "malformed or non-lowercase state_ref digest",
            "logical_state",
            present_bad_digest,
        ),
        state_negative(
            "unknown-state-fact-profile",
            "unknown state-fact profile",
            "state_document",
            unknown_profile,
        ),
        state_negative(
            "empty-objects",
            "empty objects array",
            "state_document",
            empty,
        ),
        state_negative(
            "unsorted-objects",
            "unsorted objects array",
            "state_document",
            unsorted,
        ),
        state_negative(
            "duplicate-key-commitment",
            "duplicate key commitment",
            "state_document",
            duplicate,
        ),
        state_negative(
            "missing-object-member",
            "missing object member",
            "state_document",
            missing_member,
        ),
        state_negative(
            "extra-object-member",
            "extra object member",
            "state_document",
            extra_member,
        ),
        state_negative(
            "malformed-byte-commitment",
            "malformed or non-lowercase commitment",
            "state_document",
            malformed,
        ),
        state_negative(
            "missing-affected-object",
            "missing affected object",
            "state_document",
            missing_affected,
            expected_key_commitments=sorted(expected_ethos_keys),
        ),
        state_negative(
            "unrelated-extra-object",
            "unrelated extra object",
            "state_document",
            unrelated_extra,
            expected_key_commitments=[
                one["document"]["objects"][0]["key_commitment"]
            ],
        ),
        state_negative(
            "clear-store-key",
            "forbidden clear store key",
            "state_document",
            clear_store_key,
        ),
        state_negative(
            "state-digest-mismatch",
            "state digest mismatch",
            "state_reference",
            mismatched_reference,
        ),
    ]
    assert len({case["id"] for case in negatives}) == len(negatives)
    return negatives


def catalog_by_digest(
    states: dict[str, dict[str, Any]],
) -> dict[str, dict[str, Any]]:
    return {fixture["digest"]: fixture for fixture in states.values()}


def assert_oracle_classification(
    positive_cases: list[dict[str, Any]],
    operation_negatives: list[dict[str, Any]],
    state_negatives: list[dict[str, Any]],
    states: dict[str, dict[str, Any]],
    vault_record_key: str,
) -> None:
    catalog = catalog_by_digest(states)

    for name, fixture in states.items():
        expected_keys = {
            item["key_commitment"] for item in fixture["input_objects"]
        }
        validate_state_document(
            fixture["document"],
            expected_keys=expected_keys,
        )
        assert jcs(fixture["document"]) == fixture["document_jcs"], name
        assert commitment(
            DOMAINS["state_fact"],
            fixture["document_jcs"].encode("utf-8"),
        ) == fixture["digest"], name

    for case in positive_cases:
        validate_operation_document(
            case["document"],
            catalog,
            expected_vault_record_key=vault_record_key,
            facts_ref=case["facts_ref"],
        )
        assert jcs(case["document"]) == case["document_jcs"], case["id"]
        assert commitment(
            DOMAINS["operation_facts"],
            case["document_jcs"].encode("utf-8"),
        ) == case["digest"], case["id"]

    for case in operation_negatives:
        try:
            validate_operation_document(
                case["candidate"],
                catalog,
                expected_vault_record_key=vault_record_key,
                facts_ref=case["facts_ref"],
            )
        except OracleError as error:
            assert error.code == case["must_fail"], case["id"]
        else:
            raise AssertionError(f"operation negative unexpectedly accepted: {case['id']}")

    for case in state_negatives:
        try:
            if case["scope"] == "logical_state":
                validate_state_value(case["candidate"], catalog)
            elif case["scope"] == "state_document":
                expected = case.get("expected_key_commitments")
                validate_state_document(
                    case["candidate"],
                    expected_keys=set(expected) if expected is not None else None,
                )
            elif case["scope"] == "state_reference":
                candidate = case["candidate"]
                local_document = candidate["document"]
                validate_state_document(local_document)
                local_catalog = {
                    commitment(
                        DOMAINS["state_fact"],
                        jcs(local_document).encode("utf-8"),
                    ): {"document": local_document}
                }
                validate_state_value(candidate["logical_state"], local_catalog)
            else:
                raise AssertionError(f"unknown state negative scope: {case['scope']}")
        except OracleError as error:
            assert error.code == case["must_fail"], case["id"]
        else:
            raise AssertionError(f"state negative unexpectedly accepted: {case['id']}")


def build_vector() -> dict[str, Any]:
    states = build_states()
    vault_record_key = states["vault_v1"]["input_objects"][0]["key_commitment"]
    assert (
        states["vault_v2"]["input_objects"][0]["key_commitment"]
        == vault_record_key
    )
    positive_cases = build_positive_cases(states, vault_record_key)
    operation_negatives = build_operation_negatives(positive_cases)
    state_negatives = build_state_negatives(states)
    assert_oracle_classification(
        positive_cases,
        operation_negatives,
        state_negatives,
        states,
        vault_record_key,
    )

    return {
        "vector": "CB2-K1-OPERATION-FACTS-MUTATION-1",
        "description": (
            "Independent Python standard-library oracle for the K1.1-B "
            "operation/state commitment envelopes and K1.2-M-B closed mutation "
            "families. cb2-fixture store-key strings are opaque canonical-key "
            "inputs only; they define no Bundle path or public sidecar."
        ),
        "profiles": {
            OPERATION_PROFILE_KEY: OPERATION_PROFILE,
            STATE_PROFILE_KEY: STATE_PROFILE,
        },
        "commitment_domains": DOMAINS,
        "fixture_store_key_scope": (
            "Opaque UTF-8 inputs to C(aithos-core/v1/state-key, K); not Bundle "
            "layout, not public coordinates, and not target-to-store derivation."
        ),
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
        "fixture_sids": {
            "circle_root": SID_CIRCLE_ROOT,
            "circle_parent": SID_CIRCLE_PARENT,
            "circle_destination": SID_CIRCLE_DESTINATION,
            "circle_destination_child": SID_CIRCLE_DESTINATION_CHILD,
            "self_root": SID_SELF_ROOT,
            "ethos_target": SID_ETHOS_TARGET,
            "section_target": SID_SECTION_TARGET,
            "folder_target": SID_FOLDER_TARGET,
            "folder_create": SID_FOLDER_CREATE,
        },
        "states": states,
        "vault_record_key": vault_record_key,
        "positive_cases": positive_cases,
        "negative_cases": {
            "operation_facts": operation_negatives,
            "state_facts": state_negatives,
        },
        "inventory": {
            "positive_case_ids": [case["id"] for case in positive_cases],
            "operation_negative_ids": [case["id"] for case in operation_negatives],
            "state_negative_ids": [case["id"] for case in state_negatives],
            "required_error_variants": [
                INVALID_OPERATION_FACTS,
                INVALID_STATE_FACT,
            ],
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

#!/usr/bin/env python3
"""Read the orchestrated feature train's state and print the next action.

This script launches nothing. It reads `QUEUE.yaml` and every
`features/.agents/<feature>/STATE.md` frontmatter, validates their coherence,
and names the single next action the orchestrator would take. It is the
read-only half of the train: if this script cannot decide, the orchestrator
must not either.

No third-party dependency, on purpose. The state must be inspectable on any
machine, and the restricted YAML subset documented in
`.agents/orchestrator/LEDGER.md` has exactly one reading.

Exit codes:
    0   an action is available (printed)
   10   the train is blocked and needs the human owner
   20   every queued feature is COMPLETE
   30   the state is invalid (details printed to stderr)
"""

from __future__ import annotations

import os
import sys

# --------------------------------------------------------------------------
# Restricted YAML subset
# --------------------------------------------------------------------------


class StateError(Exception):
    """A state file cannot be read with exactly one meaning."""


def _strip_comment(line: str) -> str:
    out, quote = [], None
    for i, ch in enumerate(line):
        if quote:
            out.append(ch)
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
            out.append(ch)
        elif ch == "#" and (i == 0 or line[i - 1] in " \t"):
            break
        else:
            out.append(ch)
    return "".join(out).rstrip()


def _split_flow(text: str) -> list[str]:
    parts, buf, quote = [], [], None
    for ch in text:
        if quote:
            buf.append(ch)
            if ch == quote:
                quote = None
        elif ch in "\"'":
            quote = ch
            buf.append(ch)
        elif ch == ",":
            parts.append("".join(buf).strip())
            buf = []
        else:
            buf.append(ch)
    parts.append("".join(buf).strip())
    return [p for p in parts if p != ""]


def _scalar(token: str, lineno: int):
    t = token.strip()
    if t in ("", "null", "~"):
        return None
    if t == "true":
        return True
    if t == "false":
        return False
    if len(t) >= 2 and t[0] == t[-1] and t[0] in "\"'":
        return t[1:-1]
    if t.startswith("[") and t.endswith("]"):
        return [_scalar(p, lineno) for p in _split_flow(t[1:-1])]
    if t.startswith("{") and t.endswith("}"):
        out = {}
        for part in _split_flow(t[1:-1]):
            if ":" not in part:
                raise StateError(f"line {lineno}: inline map entry without ':': {part!r}")
            k, v = part.split(":", 1)
            out[k.strip()] = _scalar(v, lineno)
        return out
    if t.startswith(("[", "{")) or t.endswith(("]", "}")):
        raise StateError(f"line {lineno}: unbalanced flow scalar {t!r}")
    try:
        return int(t)
    except ValueError:
        return t


def _items(text: str) -> list[tuple[int, int, str]]:
    items = []
    for lineno, raw in enumerate(text.splitlines(), 1):
        if "\t" in raw:
            raise StateError(f"line {lineno}: tabs are not allowed")
        content = _strip_comment(raw)
        if content.strip() == "":
            continue
        indent = len(content) - len(content.lstrip(" "))
        if indent % 2:
            raise StateError(f"line {lineno}: indentation must be a multiple of two")
        items.append((lineno, indent, content.strip()))
    return items


def _parse_block(items, i: int, indent: int):
    if items[i][2].startswith("- "):
        out = []
        while i < len(items) and items[i][1] == indent and items[i][2].startswith("- "):
            lineno, _, content = items[i]
            rest = content[2:].strip()
            if rest == "":
                raise StateError(f"line {lineno}: nested block lists are not supported")
            out.append(_scalar(rest, lineno))
            i += 1
        return out, i

    out = {}
    while i < len(items) and items[i][1] == indent:
        lineno, _, content = items[i]
        if content.startswith("- "):
            raise StateError(f"line {lineno}: list item inside a mapping")
        if ":" not in content:
            raise StateError(f"line {lineno}: expected 'key: value', got {content!r}")
        key, rest = content.split(":", 1)
        key, rest = key.strip(), rest.strip()
        if rest == "":
            if i + 1 < len(items) and items[i + 1][1] > indent:
                value, i = _parse_block(items, i + 1, items[i + 1][1])
            else:
                value, i = None, i + 1
        else:
            value, i = _scalar(rest, lineno), i + 1
        if key in out:
            raise StateError(f"line {lineno}: duplicate key {key!r}")
        out[key] = value
    if i < len(items) and items[i][1] > indent:
        raise StateError(f"line {items[i][0]}: unexpected indentation")
    return out, i


def parse(text: str, origin: str) -> dict:
    try:
        items = _items(text)
        if not items:
            return {}
        value, i = _parse_block(items, 0, items[0][1])
    except StateError as exc:
        raise StateError(f"{origin}: {exc}") from None
    if i != len(items):
        raise StateError(f"{origin}: line {items[i][0]}: trailing content")
    return value


def frontmatter(path: str) -> dict:
    with open(path, encoding="utf-8") as handle:
        lines = handle.read().splitlines()
    if not lines or lines[0].strip() != "---":
        raise StateError(f"{path}: no YAML frontmatter (first line must be '---')")
    for idx in range(1, len(lines)):
        if lines[idx].strip() == "---":
            return parse("\n".join(lines[1:idx]), path)
    raise StateError(f"{path}: unterminated frontmatter")


# --------------------------------------------------------------------------
# The machine
# --------------------------------------------------------------------------

# status -> (next role, what the role is asked to do)
NEXT = {
    "UNBOOTSTRAPPED": ("B0 — amorceur de domaine", "créer DOMAIN.md, STATE.md et les deux skills spécialisés"),
    "READY": ("I1 puis A2 — inventaire et Pass A", "geler la révision, découper en unités de revue, tracer à l'aveugle"),
    "AUDIT_INITIAL": ("A3 — Pass B et intégration", "réconcilier, passe d'état partagé, audit public, marqueurs"),
    "CORRECTION_REQUESTED": ("C1 — correcteur", "RED/GREEN sur les seuls findings assignés"),
    "REVIEW_REQUESTED": ("R1 — reviewer indépendant", "Pass A sur le candidat sans .git ni rapport correcteur, puis Pass B"),
    "REVIEW_ACCEPTED": ("G1 — reviewer d'impact", "classer les dépendances inter-features du diff accepté"),
    "IMPACT_REVIEW_REQUESTED": ("G1 — reviewer d'impact", "écrire le rapport d'impact"),
    "INTEGRATION": ("orchestrateur", "fusionner dans la branche de run et publier"),
}
BLOCKING = {"DECISION_REQUIRED", "BLOCKED"}
TERMINAL = {"COMPLETE"}
KNOWN = set(NEXT) | BLOCKING | TERMINAL

REQUIRED = ("feature", "status", "round", "open_findings", "blocked")


def load(root: str):
    queue_path = os.path.join(root, "features/.agents/orchestrator/QUEUE.yaml")
    with open(queue_path, encoding="utf-8") as handle:
        queue = parse(handle.read(), queue_path)

    features_dir = os.path.join(root, "features")
    on_disk = sorted(
        name[: -len(".feature")]
        for name in os.listdir(features_dir)
        if name.endswith(".feature")
    )

    states, problems = {}, []
    for name in on_disk:
        path = os.path.join(features_dir, ".agents", name, "STATE.md")
        if not os.path.exists(path):
            states[name] = {"feature": name, "status": "UNBOOTSTRAPPED", "round": 0,
                            "open_findings": [], "blocked": None}
            continue
        # A feature whose state cannot be read is still registered, as INVALID.
        # Dropping it would make the queue checks below emit a second,
        # misleading error about a file that does exist.
        invalid = {"feature": name, "status": "INVALID", "round": 0,
                   "open_findings": [], "blocked": None}
        try:
            state = frontmatter(path)
        except StateError as exc:
            problems.append(str(exc))
            states[name] = invalid
            continue
        missing = [k for k in REQUIRED if k not in state]
        if missing:
            problems.append(f"{path}: champs manquants dans le frontmatter : {', '.join(missing)}")
            states[name] = invalid
            continue
        if state["feature"] != name:
            problems.append(f"{path}: feature={state['feature']!r} ne correspond pas au répertoire {name!r}")
        if state["status"] not in KNOWN:
            problems.append(f"{path}: statut inconnu {state['status']!r}")
        blocked_declared = state.get("blocked") is not None
        if blocked_declared != (state["status"] in BLOCKING):
            problems.append(
                f"{path}: 'blocked' et le statut se contredisent "
                f"(statut={state['status']}, blocked={'renseigné' if blocked_declared else 'null'})"
            )
        limit = (queue.get("policy") or {}).get("max_rejections_per_finding", 3)
        for finding, count in (state.get("rejection_count") or {}).items():
            if isinstance(count, int) and count >= limit:
                problems.append(f"{path}: {finding} a {count} rejets (limite {limit}) — devrait bloquer")
        states[name] = state

    order = queue.get("order") or []
    for name in order:
        if name not in states:
            problems.append(f"QUEUE.yaml: la queue nomme {name!r}, absent de features/")
    for name, state in states.items():
        if name not in order and state["status"] not in TERMINAL:
            problems.append(f"{name}: absent de la queue et non COMPLETE (statut {state['status']})")

    return queue, states, order, problems


def render(root: str) -> int:
    try:
        queue, states, order, problems = load(root)
    except (StateError, OSError) as exc:
        print(f"état illisible : {exc}", file=sys.stderr)
        return 30

    yardsticks = queue.get("yardsticks") or {}

    print("FEATURE TRAIN — état de la queue\n")
    width = max((len(n) for n in states), default=10)
    for name in order:
        state = states.get(name)
        if state is None:
            continue
        status = state["status"]
        mark = "✓" if status in TERMINAL else ("!" if status in BLOCKING else " ")
        print(f"  {mark} {name.ljust(width)}  {status}")
    done = [n for n, s in states.items() if n not in order and s["status"] in TERMINAL]
    if done:
        print(f"\n  hors queue, terminées : {', '.join(sorted(done))}")

    if problems:
        print("\nÉTAT INVALIDE", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 30

    for name in order:
        state = states[name]
        status = state["status"]
        if status in TERMINAL:
            continue
        if status in BLOCKING:
            blocked = state.get("blocked") or {}
            print(f"\nBLOQUÉ — {name}")
            print(f"  condition : {blocked.get('reason', status)}")
            print(f"  depuis    : {blocked.get('since', 'non daté')}")
            print("  voir      : features/.agents/orchestrator/BLOCKED.md")
            return 10
        role, task = NEXT[status]
        print(f"\nPROCHAINE ACTION")
        print(f"  feature : {name}")
        print(f"  statut  : {status}   round {state['round']}")
        print(f"  rôle    : {role}")
        print(f"  tâche   : {task}")
        if state.get("assigned_findings"):
            print(f"  assigné : {', '.join(state['assigned_findings'])}")
        if state.get("open_findings"):
            print(f"  ouverts : {', '.join(state['open_findings'])}")
        if name in yardsticks:
            print(f"  étalon  : {yardsticks[name]} — entrée Pass B et comparaison seulement, jamais Pass A")
        return 0

    print("\nqueue terminée — aucune feature en attente")
    return 20


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    sys.exit(render(os.path.abspath(os.path.join(here, "..", "..", ".."))))

#!/usr/bin/env python3
"""Evidence engine of the orchestrated feature train.

Everything an agent must not be trusted to do itself lives here: running a
gate, hashing its transcript, cutting a history-blind extract, appending to the
run ledger, and checking the ledger's invariants afterwards.

The session-side orchestrator sequences roles and launches agents. It calls
this program for every fact that must be verifiable. The split is the point:
an agent cannot fabricate a transcript it never ran, and a report citing a
command absent from the ledger is rejected by `check`.

Subcommands
    run-open                     create a run directory, print its id
    gate     --run --feature --tier [--rev] [--cmd]
    extract  --run --feature --rev --unit
    record   --run [--json | stdin]
    check    --run

Exit codes
    0   success (for `gate`: the gate is green and self-consistent)
    1   the gate is red, or `check` found a violated invariant
    2   usage or state error
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
RUNS = os.path.join(ROOT, "features/.agents/orchestrator/runs")

TIERS = ("focused", "feature", "regression", "cucumber", "workspace")

# Allowed status graph, mirroring PROCESS.md § "Guarded transitions".
TRANSITIONS = {
    "UNBOOTSTRAPPED": {"READY"},
    "READY": {"AUDIT_INITIAL"},
    "AUDIT_INITIAL": {"CORRECTION_REQUESTED", "DECISION_REQUIRED", "BLOCKED"},
    "CORRECTION_REQUESTED": {"REVIEW_REQUESTED", "DECISION_REQUIRED", "BLOCKED"},
    "REVIEW_REQUESTED": {"REVIEW_ACCEPTED", "CORRECTION_REQUESTED", "DECISION_REQUIRED", "BLOCKED"},
    "DECISION_REQUIRED": {"CORRECTION_REQUESTED", "REVIEW_ACCEPTED", "BLOCKED"},
    "REVIEW_ACCEPTED": {"IMPACT_REVIEW_REQUESTED", "BLOCKED"},
    "IMPACT_REVIEW_REQUESTED": {"INTEGRATION", "BLOCKED"},
    "INTEGRATION": {"COMPLETE", "BLOCKED"},
    "BLOCKED": set(),
    "COMPLETE": set(),
}

KINDS = ("gate", "agent", "freeze", "transition", "block")


def now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def die(message: str, code: int = 2):
    print(f"train: {message}", file=sys.stderr)
    raise SystemExit(code)


# --------------------------------------------------------------------------
# Run directory and ledger
# --------------------------------------------------------------------------


def run_dir(run_id: str) -> str:
    path = os.path.join(RUNS, run_id)
    if not os.path.isdir(path):
        die(f"unknown run {run_id!r}")
    return path


def append(run_id: str, entry: dict) -> dict:
    entry.setdefault("ts", now())
    if entry.get("kind") not in KINDS:
        die(f"unknown ledger kind {entry.get('kind')!r}")
    path = os.path.join(run_dir(run_id), "ledger.jsonl")
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, ensure_ascii=False, sort_keys=True) + "\n")
    return entry


def read_ledger(run_id: str) -> list[dict]:
    path = os.path.join(run_dir(run_id), "ledger.jsonl")
    if not os.path.exists(path):
        return []
    entries = []
    with open(path, encoding="utf-8") as handle:
        for lineno, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entries.append(json.loads(line))
            except json.JSONDecodeError as exc:
                die(f"ledger.jsonl line {lineno}: {exc}")
    return entries


def cmd_run_open(args) -> int:
    os.makedirs(RUNS, exist_ok=True)
    day = time.strftime("%Y-%m-%d", time.gmtime())
    index = 1
    while os.path.isdir(os.path.join(RUNS, f"{day}-r{index}")):
        index += 1
    run_id = f"{day}-r{index}"
    os.makedirs(os.path.join(RUNS, run_id, "evidence"))
    open(os.path.join(RUNS, run_id, "ledger.jsonl"), "w", encoding="utf-8").close()
    print(run_id)
    return 0


# --------------------------------------------------------------------------
# Gates
# --------------------------------------------------------------------------

MANIFEST = "rust/Cargo.toml"
SUMMARY_LINE = re.compile(
    r"^(?P<total>\d+)\s+(?P<unit>features?|rules?|scenarios?|steps?)"
    r"(?:\s*\((?P<detail>[^)]*)\))?\s*$"
)
DETAIL = re.compile(r"(\d+)\s+(passed|failed|skipped|retried)")


def parse_summary(transcript: str) -> dict | None:
    """Parse the cucumber `[Summary]` block. None when absent.

    Counters stay grouped by unit. Flattening them would let a scenario count
    and a step count add up into a number that describes nothing — and this
    record is cited as evidence.
    """
    lines = transcript.splitlines()
    try:
        start = max(i for i, line in enumerate(lines) if line.strip() == "[Summary]")
    except ValueError:
        return None
    summary: dict = {}
    for line in lines[start + 1:]:
        stripped = line.strip()
        if not stripped:
            break
        match = SUMMARY_LINE.match(stripped)
        if not match:
            break
        unit = match.group("unit").rstrip("s") + "s"
        total = int(match.group("total"))
        detail = DETAIL.findall(match.group("detail") or "")
        if detail:
            counters = {"total": total}
            for count, label in detail:
                counters[label] = counters.get(label, 0) + int(count)
            summary[unit] = counters
        else:
            summary[unit] = total
    return summary or None


def scenario_counts(summary: dict | None) -> dict:
    """The scenario line of a summary, as counters. Empty when unavailable."""
    if not summary:
        return {}
    counters = summary.get("scenarios")
    return counters if isinstance(counters, dict) else {}


def gate_command(tier: str, feature: str | None, explicit: str | None) -> str:
    if explicit:
        return explicit
    if tier == "feature":
        if not feature:
            die("--feature is required for the feature tier")
        return (f"cargo test --manifest-path {MANIFEST} -p aithos-bundle "
                f"--test cucumber -- --tags @{feature}")
    if tier == "cucumber":
        return f"cargo test --manifest-path {MANIFEST} -p aithos-bundle --test cucumber"
    if tier == "workspace":
        return f"cargo test --manifest-path {MANIFEST} --workspace"
    die(f"tier {tier!r} requires an explicit --cmd")


def judge(tier: str, exit_code: int, summary: dict | None) -> tuple[bool, str | None]:
    """Green only if the exit code and the counters agree.

    A cucumber gate that exits 0 while reporting failures — or that reports
    nothing at all — is red whatever the exit code claims. This is the standing
    probe against a regression of the BDER-011 class, where `filter_run` under
    `harness = false` returned 0 with scenarios failing.
    """
    cucumber = tier in ("feature", "cucumber")
    if not cucumber:
        return exit_code == 0, None if exit_code == 0 else "non-zero exit"
    if summary is None:
        return False, "no [Summary] block: the run produced no verifiable counters"
    counters = scenario_counts(summary)
    failed = counters.get("failed", 0)
    scenarios = counters.get("total", 0)
    if exit_code == 0 and failed:
        return False, f"exit 0 but {failed} scenario(s) failed — harness cannot fail"
    if exit_code == 0 and scenarios == 0:
        return False, "exit 0 but no scenario ran — the tag selected nothing"
    if exit_code != 0 and not failed:
        return False, f"exit {exit_code} but no failure reported — unattributed red"
    return exit_code == 0 and failed == 0, None


def cmd_gate(args) -> int:
    if args.tier not in TIERS:
        die(f"unknown tier {args.tier!r}")
    command = gate_command(args.tier, args.feature, args.cmd)
    directory = run_dir(args.run)

    proc = subprocess.run(command, shell=True, cwd=ROOT, capture_output=True, text=True)
    transcript = proc.stdout + proc.stderr
    digest = hashlib.sha256(transcript.encode("utf-8")).hexdigest()
    evidence_id = f"ev-{digest[:8]}"
    relative = f"evidence/{evidence_id}.txt"
    with open(os.path.join(directory, relative), "w", encoding="utf-8") as handle:
        handle.write(transcript)

    summary = parse_summary(transcript)
    green, anomaly = judge(args.tier, proc.returncode, summary)
    entry = {
        "kind": "gate", "evidence_id": evidence_id, "feature": args.feature,
        "role": args.role, "tier": args.tier, "cmd": command, "rev": args.rev,
        "exit": proc.returncode, "summary": summary, "transcript": relative,
        "sha256": digest, "green": green,
    }
    if anomaly:
        entry["anomaly"] = anomaly
    append(args.run, entry)

    print(json.dumps({"evidence_id": evidence_id, "green": green, "exit": proc.returncode,
                      "summary": summary, "anomaly": anomaly}, ensure_ascii=False))
    return 0 if green else 1


# --------------------------------------------------------------------------
# History-blind extracts
# --------------------------------------------------------------------------


def cmd_extract(args) -> int:
    directory = run_dir(args.run)
    target = os.path.join(directory, "passA", args.feature, args.unit)
    if os.path.exists(target):
        die(f"extract already exists: {target}")
    os.makedirs(target)

    archive = subprocess.run(["git", "archive", args.rev], cwd=ROOT, capture_output=True)
    if archive.returncode != 0:
        die(f"git archive failed: {archive.stderr.decode('utf-8', 'replace').strip()}")
    untar = subprocess.run(["tar", "-x", "-C", target], input=archive.stdout, capture_output=True)
    if untar.returncode != 0:
        die(f"tar failed: {untar.stderr.decode('utf-8', 'replace').strip()}")

    # The whole point: no history is reachable from the extract.
    leaked = [name for name in os.listdir(target) if name in (".git", ".gitmodules")]
    if leaked:
        die(f"extract is not history-blind, found {leaked} — refusing")

    append(args.run, {
        "kind": "agent", "role": "extract", "feature": args.feature, "unit": args.unit,
        "workspace": os.path.relpath(target, directory), "history_visible": False,
        "rev": args.rev, "sha256": hashlib.sha256(archive.stdout).hexdigest(), "status": "ok",
    })
    print(os.path.relpath(target, directory))
    return 0


# --------------------------------------------------------------------------
# Free-form entries
# --------------------------------------------------------------------------


def cmd_record(args) -> int:
    raw = args.json if args.json else sys.stdin.read()
    try:
        entry = json.loads(raw)
    except json.JSONDecodeError as exc:
        die(f"invalid JSON: {exc}")
    if not isinstance(entry, dict):
        die("a ledger entry must be a JSON object")
    kind = entry.get("kind")
    if kind == "gate":
        die("gate entries are written by `train.py gate`, never by hand")
    if kind == "agent" and entry.get("role") == "passA" and entry.get("history_visible") is not False:
        die("a passA agent entry must declare history_visible: false")
    if kind == "transition":
        source, target = entry.get("from"), entry.get("to")
        if source not in TRANSITIONS:
            die(f"unknown source status {source!r}")
        if target not in TRANSITIONS[source]:
            die(f"forbidden transition {source} -> {target}")
    append(args.run, entry)
    print(json.dumps(entry, ensure_ascii=False, sort_keys=True))
    return 0


# --------------------------------------------------------------------------
# Warden — ledger invariants
# --------------------------------------------------------------------------


def check_ledger(entries: list[dict]) -> list[str]:
    problems: list[str] = []
    evidence = {e["evidence_id"] for e in entries if e.get("kind") == "gate"}
    frozen: dict[str, int] = {}

    previous_ts = ""
    for index, entry in enumerate(entries):
        where = f"entrée {index + 1} ({entry.get('kind')})"
        timestamp = entry.get("ts", "")
        if timestamp < previous_ts:
            problems.append(f"{where}: horodatage en recul ({timestamp} < {previous_ts})")
        previous_ts = max(previous_ts, timestamp)

        kind = entry.get("kind")
        if kind == "gate":
            if entry.get("green") and entry.get("anomaly"):
                problems.append(f"{where}: marquée verte alors qu'une anomalie est enregistrée")
            summary = entry.get("summary")
            counters = scenario_counts(summary)
            if entry.get("tier") in ("feature", "cucumber"):
                if entry.get("exit") == 0 and counters.get("failed", 0):
                    problems.append(f"{where}: exit 0 avec {counters['failed']} échec(s)")
                if entry.get("exit") == 0 and not summary:
                    problems.append(f"{where}: exit 0 sans compteurs vérifiables")
        elif kind == "freeze":
            frozen[entry.get("feature", "")] = index
        elif kind == "agent":
            role = entry.get("role")
            if role in ("passA", "review-passA"):
                if entry.get("history_visible") is not False:
                    problems.append(f"{where}: rôle {role} sans history_visible: false")
                workspace = entry.get("workspace") or ""
                if not workspace.startswith("passA/"):
                    problems.append(f"{where}: rôle {role} hors d'un extrait passA/ ({workspace!r})")
            if role in ("passB", "review-passB"):
                feature = entry.get("feature", "")
                if feature not in frozen:
                    problems.append(f"{where}: Pass B sur {feature!r} avant tout gel de Pass A")
                elif frozen[feature] > index:
                    problems.append(f"{where}: Pass B sur {feature!r} antérieur à son gel")
            for cited in entry.get("inputs", []) or []:
                if isinstance(cited, str) and cited.startswith("ev-") and cited not in evidence:
                    problems.append(f"{where}: cite la preuve {cited} absente du journal")
        elif kind == "transition":
            source, target = entry.get("from"), entry.get("to")
            if source not in TRANSITIONS or target not in TRANSITIONS.get(source, set()):
                problems.append(f"{where}: transition interdite {source} -> {target}")

    return problems


def cmd_check(args) -> int:
    entries = read_ledger(args.run)
    problems = check_ledger(entries)
    if problems:
        print(f"GARDIEN — {len(problems)} violation(s) sur {len(entries)} entrées", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1
    print(f"gardien : {len(entries)} entrées, aucun invariant violé")
    return 0


# --------------------------------------------------------------------------


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(prog="train.py", description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("run-open").set_defaults(func=cmd_run_open)

    gate = subparsers.add_parser("gate")
    gate.add_argument("--run", required=True)
    gate.add_argument("--tier", required=True, choices=TIERS)
    gate.add_argument("--feature")
    gate.add_argument("--role", default=None)
    gate.add_argument("--rev", default=None)
    gate.add_argument("--cmd", default=None)
    gate.set_defaults(func=cmd_gate)

    extract = subparsers.add_parser("extract")
    extract.add_argument("--run", required=True)
    extract.add_argument("--feature", required=True)
    extract.add_argument("--rev", required=True)
    extract.add_argument("--unit", required=True)
    extract.set_defaults(func=cmd_extract)

    record = subparsers.add_parser("record")
    record.add_argument("--run", required=True)
    record.add_argument("--json", default=None)
    record.set_defaults(func=cmd_record)

    check = subparsers.add_parser("check")
    check.add_argument("--run", required=True)
    check.set_defaults(func=cmd_check)

    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())

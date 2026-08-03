#!/usr/bin/env python3
"""Behaviour bench for `train-status.py`.

The train reads its next move from this reader. A reader that fails silently,
or that reports a misleading cause, would send an unattended run down the wrong
branch. Each case below therefore asserts two things: the exit code, and that a
rejected state produces exactly one error naming its real cause.

Run: python3 features/.agents/scripts/test-train-status.py
Exit 0 if every case passes.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
READER = os.path.join(HERE, "train-status.py")

QUEUE = "version: 1\npolicy:\n  max_rejections_per_finding: 3\norder:\n  - alpha\n  - beta\n"


def state(**overrides) -> str:
    fields = {
        "feature": "alpha",
        "status": "READY",
        "mode": "null",
        "round": 1,
        "open_findings": "[]",
        "blocked": "null",
        "rejection_count": "{}",
    }
    fields.update(overrides)
    body = "\n".join(f"{key}: {value}" for key, value in fields.items())
    return f"---\n{body}\n---\n\n# human table\n"


DONE = state(feature="beta", status="COMPLETE")


def build(tmp: str, name: str, queue: str, states: dict) -> str:
    root = os.path.join(tmp, name)
    os.makedirs(os.path.join(root, "features/.agents/scripts"))
    os.makedirs(os.path.join(root, "features/.agents/orchestrator"))
    shutil.copy(READER, os.path.join(root, "features/.agents/scripts/train-status.py"))
    for feature in ("alpha", "beta"):
        with open(os.path.join(root, f"features/{feature}.feature"), "w", encoding="utf-8") as fh:
            fh.write(f"@{feature}\nFeature: {feature}\n")
    with open(os.path.join(root, "features/.agents/orchestrator/QUEUE.yaml"), "w", encoding="utf-8") as fh:
        fh.write(queue)
    for feature, text in states.items():
        os.makedirs(os.path.join(root, f"features/.agents/{feature}"), exist_ok=True)
        with open(os.path.join(root, f"features/.agents/{feature}/STATE.md"), "w", encoding="utf-8") as fh:
            fh.write(text)
    return root


def run(root: str):
    proc = subprocess.run(
        [sys.executable, os.path.join(root, "features/.agents/scripts/train-status.py")],
        capture_output=True, text=True,
    )
    errors = [line.strip(" -") for line in proc.stderr.splitlines() if line.strip().startswith("-")]
    return proc.returncode, errors


CASES = [
    ("action disponible", 0, QUEUE, {"alpha": state(), "beta": DONE}, None),
    ("queue terminée", 20, QUEUE, {"alpha": state(status="COMPLETE"), "beta": DONE}, None),
    ("blocage humain", 10, QUEUE,
     {"alpha": state(status="DECISION_REQUIRED", blocked="{reason: CHDR-004, since: 2026-08-03}"),
      "beta": DONE}, None),
    ("statut inconnu", 30, QUEUE, {"alpha": state(status="PRESQUE_FINI"), "beta": DONE}, "statut inconnu"),
    ("blocked contredit le statut", 30, QUEUE,
     {"alpha": state(status="READY", blocked="{reason: x}"), "beta": DONE}, "se contredisent"),
    ("nom != répertoire", 30, QUEUE, {"alpha": state(feature="gamma"), "beta": DONE}, "ne correspond pas"),
    ("champ requis manquant", 30, QUEUE,
     {"alpha": "---\nfeature: alpha\nstatus: READY\n---\n", "beta": DONE}, "champs manquants"),
    ("rejets à la limite sans blocage", 30, QUEUE,
     {"alpha": state(rejection_count="{CHDR-001: 3}"), "beta": DONE}, "devrait bloquer"),
    ("feature hors queue et non COMPLETE", 30, "version: 1\norder:\n  - alpha\n",
     {"alpha": state(), "beta": state(feature="beta", status="READY")}, "absent de la queue"),
    ("tabulation", 30, QUEUE,
     {"alpha": "---\nfeature: alpha\n\tstatus: READY\n---\n", "beta": DONE}, "tabs are not allowed"),
    ("frontmatter absent", 30, QUEUE, {"alpha": "# rien\n", "beta": DONE}, "no YAML frontmatter"),
    ("flow non équilibré", 30, QUEUE,
     {"alpha": state(open_findings="[A, B"), "beta": DONE}, "unbalanced flow scalar"),
    ("clé dupliquée", 30, QUEUE,
     {"alpha": state() .replace("round: 1", "round: 1\nround: 2"), "beta": DONE}, "duplicate key"),
]


def main() -> int:
    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        for index, (label, expected, queue, states, needle) in enumerate(CASES):
            root = build(tmp, f"case{index}", queue, states)
            code, errors = run(root)
            problems = []
            if code != expected:
                problems.append(f"code {code}, attendu {expected}")
            if expected == 30:
                if len(errors) != 1:
                    problems.append(f"{len(errors)} erreurs, attendu exactement 1")
                elif needle and needle not in errors[0]:
                    problems.append(f"cause attendue {needle!r}, obtenu {errors[0]!r}")
            elif errors:
                problems.append(f"erreurs inattendues : {errors}")
            if problems:
                failures += 1
                print(f"  ÉCHEC  {label}")
                for problem in problems:
                    print(f"         {problem}")
            else:
                print(f"  ok     {label}")

    print()
    if failures:
        print(f"{failures} cas en échec sur {len(CASES)}")
        return 1
    print(f"{len(CASES)} cas, tous passent")
    return 0


if __name__ == "__main__":
    sys.exit(main())

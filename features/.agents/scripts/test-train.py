#!/usr/bin/env python3
"""Behaviour bench for `train.py` — the evidence engine.

Two families of case. The first pins the summary parser and the green/red
judgement, because the train's whole claim to trustworthiness is that a gate
cannot be reported green unless its exit code and its counters agree. The
second pins the warden, because it is the only thing standing between an
unattended run and a well-formed lie.

Run: python3 features/.agents/scripts/test-train.py
Exit 0 if every case passes.
"""

from __future__ import annotations

import importlib.util
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("train", os.path.join(HERE, "train.py"))
train = importlib.util.module_from_spec(spec)
spec.loader.exec_module(train)

GREEN = """\
 ✔  Then the transition is rejected

[Summary]
1 feature
8 rules
30 scenarios (30 passed)
93 steps (93 passed)
"""

RED = """\
[Summary]
1 feature
4 rules
8 scenarios (6 passed, 2 failed)
20 steps (17 passed, 2 failed, 1 skipped)
"""

EMPTY = """\
[Summary]
1 feature
0 rules
0 scenarios (0 passed)
0 steps (0 passed)
"""

NOISE = "warning: unused variable\nerror: could not compile\n"


def cases_parser():
    yield ("résumé vert", train.parse_summary(GREEN),
           {"features": 1, "rules": 8,
            "scenarios": {"total": 30, "passed": 30},
            "steps": {"total": 93, "passed": 93}})
    # Régression : les compteurs ne doivent JAMAIS être additionnés entre
    # unités. Une première version enregistrait « 8 scénarios, 36 passed »,
    # somme des scénarios et des steps — un nombre qui ne décrit rien, cité
    # comme preuve.
    yield ("résumé rouge, compteurs non fusionnés", train.parse_summary(RED),
           {"features": 1, "rules": 4,
            "scenarios": {"total": 8, "passed": 6, "failed": 2},
            "steps": {"total": 20, "passed": 17, "failed": 2, "skipped": 1}})
    yield ("aucun résumé", train.parse_summary(NOISE), None)


def cases_judge():
    # (label, tier, exit, transcript, expected_green, expected_anomaly_substring)
    yield ("gate vert cohérent", "feature", 0, GREEN, True, None)
    yield ("gate rouge cohérent", "feature", 101, RED, False, None)
    yield ("BDER-011 : exit 0 avec des échecs", "feature", 0, RED, False, "harness cannot fail")
    yield ("tag qui ne sélectionne rien", "feature", 0, EMPTY, False, "no scenario ran")
    yield ("compilation échouée, aucun compteur", "feature", 101, NOISE, False, "no [Summary]")
    yield ("rouge non attribué", "feature", 101, GREEN, False, "unattributed red")
    yield ("workspace vert", "workspace", 0, NOISE, True, None)
    yield ("workspace rouge", "workspace", 101, NOISE, False, "non-zero exit")


def cases_warden():
    ev = {"kind": "gate", "ts": "2026-08-03T09:00:00Z", "evidence_id": "ev-1", "tier": "feature",
          "exit": 0, "summary": {"scenarios": {"total": 8, "passed": 8}}, "green": True}

    def agent(role, **kw):
        base = {"kind": "agent", "ts": "2026-08-03T09:10:00Z", "role": role,
                "feature": "c-headers", "workspace": "passA/c-headers/RU-1",
                "history_visible": False}
        base.update(kw)
        return base

    freeze = {"kind": "freeze", "ts": "2026-08-03T09:20:00Z", "feature": "c-headers"}

    yield ("journal conforme", [ev, agent("passA"), freeze,
                                agent("passB", workspace="repo", history_visible=True,
                                      ts="2026-08-03T09:30:00Z", inputs=["ev-1"])], 0)
    yield ("Pass B avant tout gel",
           [ev, agent("passB", workspace="repo", history_visible=True)], 1)
    yield ("Pass A voyant l'historique",
           [ev, agent("passA", history_visible=True), freeze], 1)
    yield ("Pass A hors extrait",
           [ev, agent("passA", workspace="repo"), freeze], 1)
    yield ("preuve citée inexistante",
           [ev, agent("passA", inputs=["ev-inconnue"]), freeze], 1)
    yield ("exit 0 avec des échecs",
           [{**ev, "summary": {"scenarios": {"total": 8, "passed": 6, "failed": 2}}}], 1)
    yield ("exit 0 sans compteurs", [{**ev, "summary": None}], 1)
    yield ("verte malgré une anomalie", [{**ev, "anomaly": "quelque chose"}], 1)
    yield ("transition interdite",
           [{"kind": "transition", "ts": "2026-08-03T09:00:00Z",
             "from": "AUDIT_INITIAL", "to": "COMPLETE"}], 1)
    yield ("horodatage en recul",
           [ev, {**ev, "ts": "2026-08-03T08:00:00Z", "evidence_id": "ev-2"}], 1)


def main() -> int:
    failures = 0

    print("parseur de résumé")
    for label, got, expected in cases_parser():
        ok = got == expected
        failures += not ok
        print(f"  {'ok    ' if ok else 'ÉCHEC '} {label}")
        if not ok:
            print(f"         obtenu {got}\n         attendu {expected}")

    print("\njugement vert/rouge")
    for label, tier, code, transcript, want_green, needle in cases_judge():
        green, anomaly = train.judge(tier, code, train.parse_summary(transcript))
        ok = green == want_green and (needle is None or (anomaly and needle in anomaly))
        failures += not ok
        print(f"  {'ok    ' if ok else 'ÉCHEC '} {label}")
        if not ok:
            print(f"         vert={green} (attendu {want_green}) anomalie={anomaly!r}")

    print("\ngardien du journal")
    for label, entries, want in cases_warden():
        problems = train.check_ledger(entries)
        got = 1 if problems else 0
        ok = got == want
        failures += not ok
        print(f"  {'ok    ' if ok else 'ÉCHEC '} {label}")
        if not ok:
            print(f"         violations={problems}")

    total = len(list(cases_parser())) + len(list(cases_judge())) + len(list(cases_warden()))
    print()
    if failures:
        print(f"{failures} cas en échec sur {total}")
        return 1
    print(f"{total} cas, tous passent")
    return 0


if __name__ == "__main__":
    sys.exit(main())

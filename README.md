# aithos-core

Core specification of the **Aithos** protocol.

Aithos is a protocol. A vault that follows it is an **Ethos**: a tree of nodes, a journal, mandates.

This repository holds the normative material only — no implementation, no tooling requirements, no audit history.

## Contents

| File | What it is |
| --- | --- |
| [`SPEC.md`](SPEC.md) | The business specification: Part I (the rules), Part II (the perspectives), Annex A (roles × acts × states matrix). |
| [`GLOSSARY.md`](GLOSSARY.md) | Definitions of every term the specification uses. No rule is born there. |

## How to read

- **Part I** states the rules. Each rule carries a stable identifier — `R-section.number` for rules, `X-number` for assumed limits — and is normative only there.
- **Part II** restates the same truths through each role's eyes. It states no new rule: every statement cites its sources in brackets, and where they diverge, the rule governs.
- **Annex A** cross-references roles, acts and states; each cell points to a rule or to an assumed limit.

Markers: ✅ possible or guaranteed · ❌ refused or impossible · ◆ fact — a property or a limit, assumed, that neither grants nor refuses anything.

## Status

Protocol V1 · document v3.6 · 2026-08-17.

The English text is a translation of the French source specification; where the two diverge, the French source governs.

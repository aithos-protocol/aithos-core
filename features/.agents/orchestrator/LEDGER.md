# Run ledger — format

The ledger is the orchestrator's append-only record of a run. It is the only
place a report may cite evidence from. A report citing a command with no
matching ledger entry is invalid, and the process warden rejects it — see
`PROCESS.md`, section "Orchestrated gate execution".

One run directory per run:

```text
.agents/orchestrator/runs/<date>-<run-id>/
  ledger.jsonl          append-only, one JSON object per line
  evidence/<id>.txt     verbatim command transcripts, never edited
```

Nothing in a run directory is ever rewritten. A correction is a new line.

## Entry kinds

Every line carries `ts` (RFC 3339, UTC), `kind`, and `feature` when the event
belongs to a feature cycle.

### `gate`

A command executed by the orchestrator on behalf of a role.

```json
{"ts":"2026-08-03T09:12:44Z","kind":"gate","evidence_id":"ev-0f3a",
 "feature":"c-headers","role":"auditor","tier":"feature",
 "cmd":"cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers",
 "rev":"95d00ac","exit":0,"green":true,
 "summary":{"features":1,"rules":4,
            "scenarios":{"total":8,"passed":8},
            "steps":{"total":28,"passed":28}},
 "transcript":"evidence/ev-0f3a.txt",
 "sha256":"ef5d5a01e0ee19e900d21383636d9acdd632f5d472a4f6451888fc1e2df4886c"}
```

`tier` is one of `focused`, `feature`, `regression`, `cucumber`, `workspace` —
the gate pyramid of `PROCESS.md`.

Counters stay grouped by unit. Flattening them lets a scenario count and a step
count add into a number that describes nothing, and this record is cited as
evidence.

`exit`, `summary` and `green` are all recorded, and `green` is computed, never
asserted. A gate whose exit code and counters disagree is red whatever the exit
code says, and raises blocking condition 3. Four disagreements are treated as
red: exit 0 with failures reported (the `BDER-011` shape), exit 0 with no
counters at all, exit 0 with zero scenarios selected — the tag matched nothing
— and a non-zero exit with no failure reported, an unattributed red. A gate red
for one of these reasons also carries `anomaly`, naming which.

### `agent`

One agent launch and its outcome. `workspace` states which of the three
working spaces the agent saw, which is how Pass A isolation becomes auditable
after the fact.

```json
{"ts":"2026-08-03T09:20:02Z","kind":"agent","agent_id":"ag-11",
 "feature":"c-headers","role":"passA","unit":"RU-2",
 "workspace":"passA/c-headers/RU-2","history_visible":false,
 "inputs":["features/c-headers.feature#Rule-2","ev-0f3a"],
 "output":"runs/.../pass-a/RU-2.json","tokens":48213,"status":"ok"}
```

`history_visible` must be `false` for every `passA` entry and for a `review`
entry before its freeze. The warden reads this field, not the agent's word.

### `freeze`

Marks a Pass A result immutable. Every `passB` entry for the same feature must
appear **after** the corresponding `freeze`. That ordering is the machine-
checkable form of the Pass A/Pass B barrier.

```json
{"ts":"2026-08-03T09:41:10Z","kind":"freeze","feature":"c-headers",
 "units":["RU-1","RU-2","RU-3","RU-4"],"sha256":"…"}
```

### `transition`

A feature's status change, with the invariant that authorised it.

```json
{"ts":"2026-08-03T10:02:55Z","kind":"transition","feature":"c-headers",
 "from":"AUDIT_INITIAL","to":"CORRECTION_REQUESTED",
 "invariant_checks":["public_audit_written","run_report_complete",
                     "passA_precedes_passB","markers_match_open_findings",
                     "refutation_panel_recorded"],
 "commit":"…"}
```

### `block`

The run stops. `condition` is the number from the closed list in `PROCESS.md`,
section "Blocking conditions". The same entry is rendered into `BLOCKED.md`
for the human owner.

```json
{"ts":"2026-08-03T10:40:00Z","kind":"block","feature":"c-headers",
 "condition":1,"finding":"CHDR-004",
 "question":"…","options":["…","…"],"evidence":["ev-0f3a","ag-11"]}
```

## Restricted grammar

`QUEUE.yaml` and every `STATE.md` frontmatter use this subset only:

- two-space indentation, no tabs;
- `key: scalar`, `key:` followed by an indented block;
- block lists (`- item`) of scalars;
- inline lists `[a, b]` and inline maps `{a: 1, b: 2}`, one level, no nesting;
- scalars: bare words, `'single'` or `"double"` quoted strings, integers,
  `true`, `false`, `null` (also `~` or empty);
- `#` starts a comment outside quotes.

Anything else is a parse error, reported with its line number. No anchors, no
multi-line scalars, no flow nesting. The reader has no third-party dependency
so that the state can be inspected on any machine.

---
feature: g4-client-surfaces
status: READY
mode: audit
round: 0
base_main: null
audit_revision: null
candidate_revision: null
branch: codex/audit-g4-client-surfaces
assigned_findings: []
open_findings: []
rejection_count: {}
blocked: null
last_transition: 2026-08-04T00:00:00+00:00
---

# Domain state — `g4-client-surfaces`

## Domain state

| Field | Value |
|---|---|
| Status | `READY` — the domain is bootstrapped and no round has been opened. Per `../scripts/train-status.py`, the next role is **I1 then A2 — inventory and Pass A**: freeze the revision, divide into review units, trace history-blind |
| Expected mode | `audit` — initial audit, via `auditor/audit-g4-client-surfaces/SKILL.md` |
| Round | 0 |
| Base of the round | **not frozen** (`base_main: null`). The role that opens the round records the exact local `main` revision here and in its run report |
| Audit revision | **not frozen** (`audit_revision: null`) |
| Candidate revision | none (`candidate_revision: null`) |
| Canonical branch | `codex/audit-g4-client-surfaces`, the `PROCESS.md` default. The name is free: `QUEUE.yaml` registers no yardstick for this feature and no ref containing `g4` exists locally or on `origin` — see `DOMAIN.md`, § *Branch and evidence* |
| Yardstick | **none.** `../orchestrator/QUEUE.yaml:96-97` lists `yardsticks:` for `c-headers` only. There is no prior manual audit branch, no prior public audit note, and therefore no Pass B milestone material for this feature inside this repository |
| Canonical tag | `@g4-client-surfaces` (`features/g4-client-surfaces.feature:1`) |
| Contract on disk | 1 feature / 0 rules / 4 scenarios / 15 steps, counted from the file at `c406bbf`. A count of the file, never a gate result |
| Public audit | `docs/audits/features/g4-client-surfaces.md` — **does not exist yet**. The auditor creates it and adds its row to `docs/audits/features/README.md` |
| Finding identifiers | `G4CS-*`. No family is reserved for this feature in `docs/audits/features/README.md`; the prefix and the search that shows it unused are in `DOMAIN.md`, § *Contract* |
| Findings | none. No audit has run |
| Gherkin markers | none in `features/g4-client-surfaces.feature`. Its tag line carries `@g4-client-surfaces @wip @g4 @wasm @cli` and no `@audit-*` or `@aid-*` marker |
| Bootstrapped by | role **B0 — domain bootstrapper**, 2026-08-04, on `main` at `c406bbf`, worktree clean. No gate run, no history read |
| Blocked | no |

## What this file is not

Nothing here may be read as evidence about how `g4-client-surfaces` behaves.
This domain was bootstrapped **without running a single gate and without
reading any history** — deliberately, so that the auditor's Pass A begins on
raw contract, code and specification. Every path, symbol, count and command in
`DOMAIN.md` and in this file is either a count of a file on disk, stated as
such, or a command to run. None is a result, and none is a verdict on whether a
scenario is well tested.

## Inputs

- process: `../PROCESS.md`;
- domain rules and routing, including § *Project stage*: `../../AGENTS.md`;
- domain: `DOMAIN.md`;
- shared skills: `../shared/audit-gherkin-feature/SKILL.md`,
  `../shared/correct-gherkin-feature/SKILL.md`;
- specialised skills: `auditor/audit-g4-client-surfaces/SKILL.md`,
  `corrector/correct-g4-client-surfaces/SKILL.md`;
- queue, policy, budgets and recorded follow-ups: `../orchestrator/QUEUE.yaml`;
- ledger format and the restricted frontmatter grammar:
  `../orchestrator/LEDGER.md`;
- open blocking conditions repository-wide: `../orchestrator/BLOCKED.md`.

## Recorded follow-ups this feature already owes

These are inputs to the audit, recorded in `../orchestrator/QUEUE.yaml` before
this domain existed. They are quoted verbatim with their keys so that no role
can miss them. A `TARGETED` follow-up means a future cycle of this feature owes
specific scenarios; it never reopens another feature by itself
(`QUEUE.yaml:99-101`).

### 1. `chdr-i3-targeted` — this feature is named

```yaml
chdr-i3-targeted: [a-identity, d-bundle, g-revocation, g4-client-surfaces, k-integration, m-delegated-editions, n-structural-mutations, o-connector-classes-vault]
```

Recorded by the `c-headers` impact review, 2026-08-04, with the comment "No
`FULL_AUDIT`" (`QUEUE.yaml:106-107`).

### 2. `chdr-i3-g4-cli` — the two specific debts, named for this cycle

```yaml
chdr-i3-g4-cli: 'header-seal and header-open are unexercised by cli_surface.rs; the g4 cycle owes both, with one negative case pinning a foreign owner_kex (CHDR-035, CHDR-032)'
```

`QUEUE.yaml:108`. The two surfaces are
`rust/crates/aithos-cli/src/cmd/header_seal.rs` and
`rust/crates/aithos-cli/src/cmd/header_open.rs`; `--owner-kex-hex` is the
argument the negative case is about (`header_seal.rs:12-23`). `CHDR-035` and
`CHDR-032` are `c-headers` identifiers and remain `c-headers` identifiers; a
finding opened here about the same subject takes a `G4CS-*` identifier of its
own and cites them.

### 3. Follow-ups that name no feature but reach this cycle

Recorded by the `c-headers` lot A impact review, 2026-08-04
(`QUEUE.yaml:143-164`). Quoted in the part that binds a role here.

- `chdr-lota-clippy-and-fail-fast` — *"No DOMAIN.md of the three names clippy
  while ci.yml:34 enforces it. a-identity/DOMAIN.md:80-83 is a multi-binary
  regression without --no-fail-fast; b-derivation/DOMAIN.md:88 is
  single-binary and immune. ci.yml:37 runs cargo test --workspace without
  --no-fail-fast too, so CI under-reports the same way. Fix the three files and
  the template the bootstrapper copies for the other sixteen."*
  **Discharged for this domain at bootstrap**: `DOMAIN.md` § *Gate pyramid*
  names `clippy` as a final global gate and carries `--no-fail-fast` on every
  multi-binary invocation. The three older `DOMAIN.md` files are not this
  feature's to fix.

- `chdr-lota-mutation-protocol` — *"the rule that a test-semantics correction
  proves itself by a named mutant is in no normative file. PROCESS.md
  correction step 2 says RED test when possible and is silent on the impossible
  case; shared/correct-gherkin-feature/SKILL.md execution steps 1-3 presuppose
  a defect on a production path. Sixteen features have no agent directory yet
  and will inherit the shared skill."*
  **Discharged for this domain at bootstrap**: the rule is written into
  `corrector/correct-g4-client-surfaces/SKILL.md`, § *Proving a test-semantics
  correction*. The shared skill is unchanged and is not this feature's to edit.

- `chdr-lota-mutants-as-patches` — *"a mutant named in prose cannot be re-run
  and cannot be pointed, so neither its kill count nor its direction can be
  checked. … Lot A published its mutants as exact patches
  (review-lot-a.md:44-68); nothing requires it."*
  **Discharged for this domain at bootstrap**: the corrector skill requires
  every mutant to be published as an exact patch.

- `chdr-lota-vector-generators` — *"vectors/ holds 29 gen-*.py, no CI step is
  Python (.github/workflows/ci.yml, 2 jobs, 8 steps, read whole), and nine have
  no --check mode at all: gen-f, gen-g, gen-h, gen-h2, gen-i, gen-eplus,
  gen-fplus, gen-gplus, gen-cb2-max-children. … Owed by the first cycle to
  touch a vector."*
  **Conditional.** It binds this cycle only if this cycle touches a vector. The
  two generators adjacent to this domain,
  `gen-cb14-delegated-session-chain.py` and
  `gen-cb15-external-delegated-grant.py`, both **have** a `--check` mode and
  are not among the nine; their commands are in `DOMAIN.md`, § *Vector
  `--check`*.

- `chdr-lota-proxy-verdicts` — the nine features whose Gherkin lines resolve to
  the shared `OnceLock` verdicts. **`g4-client-surfaces` is not in that list**
  (`QUEUE.yaml:146`). That is a statement about which features the lot A review
  measured, not a finding about this one.

### 4. Repository-wide conditions a role here inherits

- `chdr-028` — *"BLOCKING, embargo - routing requires naming the surface, which
  describes the mechanism. Held by the orchestrator; no target recorded here"*
  (`QUEUE.yaml:122`). Open in `../orchestrator/BLOCKED.md`. Its full statement
  is outside this repository. No role may publish it, and no role here needs
  it.
- `spec-cons-12` — *"BLOCKING, embargo - identifier and neutral title only. I4,
  contradiction, both implemented. Held by the orchestrator"*
  (`QUEUE.yaml:140`). Same treatment.
- `chdr-i3-rewrite-vs-reverify` — *"BLOCKING … Owner ruling required"*
  (`QUEUE.yaml:119`). Not this feature's, recorded so a role that meets it
  routes it rather than deciding it.

## Current instruction

Run the **initial audit** of `features/g4-client-surfaces.feature` with
`auditor/audit-g4-client-surfaces/SKILL.md`, in the mode named by the `mode`
field of the frontmatter above.

Before collecting any evidence, freeze the revision and record it in
`base_main` and `audit_revision`, here and in the run report.

Read `DOMAIN.md` completely and read only the routing fields of this file —
mode, branch, base revision, observed revision, assigned scope, expected output
path — before Pass A is frozen. The recorded follow-ups above are routing
material and may be read at any time: they name debts, not verdicts.

Do not read `git log`, `git show`, `git diff`, `git blame`, commit messages,
`docs/audits/`, `../orchestrator/runs/`, or any other feature's run reports
before the Pass A result is frozen (`../PROCESS.md`, § *Pass A — current code,
history-blind*). This feature has no yardstick branch, so the ordinary Pass B
material is the diff and the impact-review reports of other features that name
it.

Name every gate you want run and stop. The orchestrator runs it, journals it
under an `evidence_id` and returns the transcript
(`../orchestrator/LEDGER.md`). Cite the `evidence_id`, never a command you ran
yourself.

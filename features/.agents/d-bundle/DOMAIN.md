# Domain — `d-bundle.feature`

## Contract

This feature covers the **bundle** — the subject's entire state as files — and
the **edition**, the signed manifest that pins those files into a linear,
hash-chained history verifiable with no server and, in the public zone, with no
key at all (`features/d-bundle.feature:1-6`):

- editions chain and verify offline; the manifest pins the DID document, and a
  tampered pinned file or a wrong predecessor hash fails the chain
  (`Rule` at `:8`, four scenarios);
- content round-trips through the sealed store, and a display path survives a
  folder rename because keys hang off sids, not names (`Rule` at `:32`, two
  scenarios);
- the public zone reads with no key and still checks against the signed edition
  (`Rule` at `:45`, one scenario);
- the `self` zone leaks no structure to a stranger while the owner still
  reconstructs the tree from sealed descriptors (`Rule` at `:53`, one scenario);
- the owner performs every content operation in all three zones from a narrow
  local capability, without a mandate and without consuming mandate counters
  (`Rule` at `:61`, one `Scenario Outline`, fifteen `Examples` rows);
- a local mutation commits state and Gamma as one transaction: a failure before
  the logical commit point leaves the canonical bundle byte-for-byte unchanged,
  and a success exposes one complete new state (`Rule` at `:89`, two
  `Scenario Outline`s, twelve and two rows);
- local capabilities and paths stay narrow: a purpose-bound opaque capability
  refuses a mismatched object class, and an untrusted display path or Store key
  never escapes its selected root (`Rule` at `:129`, two `Scenario Outline`s,
  four and ten rows).

**Shape of the contract as it sits on disk at `7058a96`**, counted from
`features/d-bundle.feature` by expanding every `Examples` table:
**1 feature / 7 `Rule` blocks / 13 authored scenario blocks → 51 expanded
scenarios / 299 steps**. The seven `Rule` lines are `:8`, `:32`, `:45`, `:53`,
`:61`, `:89`, `:129`. This is a count of the file, not a gate result — see
§ *Reading the counters*.

Tag line: `@d-bundle` (`features/d-bundle.feature:1`), the canonical tag alone.
No `@wip`, no surface marker, no `@audit-*` or `@aid-*` marker anywhere in the
file.

There is no public audit note for this feature yet: `docs/audits/features/`
contains exactly `README.md`, `a-identity.md`, `b-derivation.md` and
`c-headers.md` (`ls docs/audits/features`, whole directory).
`docs/audits/features/README.md` reserves **no** identifier family for it — its
index table has three rows, for `a-identity`, `b-derivation` and `c-headers`
only, and `:20` states the convention as "a stable feature-derived identifier:
`AID-001`, `BDER-001`, `CHDR-001`, and so on".

This domain therefore prescribes **`DBND-*`** as the stable finding family. It
follows the existing pattern — feature letter plus the stem's consonant
skeleton, as `BDER` = B + *derivation* and `CHDR` = C + *headers* — so
`d-bundle` gives D + *bundle* = `DBND`. Neither `DBND-` nor the alternative
`DBDL-` is in use: `git grep -c -- "DBND-"` and `git grep -c -- "DBDL-"` over
the tracked tree both return nothing. No tracked file blesses the choice; the
auditor creates `docs/audits/features/d-bundle.md` and adds its index row.

## Project stage — carried forward from `features/AGENTS.md`

`features/AGENTS.md` § *Project stage* applies in full to every role on this
feature and is the normative text; what follows is a pointer, not a
replacement.

As of 2026-08-04 `aithos-core` is `0.1.0-alpha.1` (`rust/Cargo.toml`,
`[workspace.package] version`), nothing is deployed and no edition has been
published by anyone. Consequences for this domain, which is where the phrase
"no edition has been published" is literal:

1. **Do not weigh backward compatibility.** No migration path, grandfather
   clause, legacy profile or compatibility shim is owed to any holder of an
   edition, because there is none. `git ls-files` returns no serialised
   `manifest.json`, `header.json` or `hdr/*.json` outside `vectors/`. A change
   to the at-rest bundle layout, to the manifest wire, or to a pinned digest
   costs a re-pin inside this repository and nothing else.
2. **Do not soften a correction to spare the past.**

What still counts: a change that breaks this repository's own tests, vectors or
pinned digests is a real cost and is costed normally — for this domain that is
`vectors/ownership.json` and the `cb2-*` bundle vectors listed below. And a
rule the implementation cannot satisfy is a defect at zero users.

**That section expires** on the first published edition outside this repository
or when the crate leaves `alpha`. A role that finds it still present after
either has happened reports that fact rather than obeying it
(`features/AGENTS.md:30-34`). `QUEUE.yaml` carries the same switch as
`policy.backward_compatibility_required: false` with the same expiry.

## Branch and evidence

- Canonical audit branch: `codex/audit-d-bundle`, the `PROCESS.md` default
  (`features/.agents/PROCESS.md:17-18`). **The name is free.**
  `features/.agents/orchestrator/QUEUE.yaml` registers a `yardsticks:` entry for
  `c-headers` only, and `git branch -a --list` lists nine refs
  (`codex/audit-c-headers-r2`, `main`, `origin/HEAD`,
  `origin/codex/audit-c-headers`,
  `origin/codex/bundle-publication-performance`,
  `origin/codex/gateway-demo-companion`,
  `origin/codex/gse-0-extension-registry-wip`,
  `origin/codex/olr-oauth-libs-upstream`, `origin/main`), none of which is named
  for `d-bundle`. This feature has **no yardstick**: there is no prior manual
  audit branch registered as a Pass B milestone.
  `origin/codex/bundle-publication-performance` carries the word *bundle* and is
  **not** a yardstick — it is an unrelated product branch, and
  `features/.agents/PROCESS.md:19-21` forbids auditing on one. It is named here
  only so a role that meets it does not mistake it for prior audit material.
- Corrections use a dedicated `codex/fix-d-bundle-<finding-or-scope>` descendant
  branch of the immutable audited revision (`features/AGENTS.md:44-46`,
  `features/.agents/PROCESS.md:39-41`).
- `base_main` and `audit_revision` are `null` in `STATE.md`: no revision has
  been frozen for this feature. The role that opens the round freezes them there
  and in its run report.
- **No gate has been run for this feature by this role.** Every number in this
  file is either a count of a file on disk, stated as such, or a command to run
  — never a result.

## Protocol invariants

Numbered claims, each with the normative text that governs it and the symbol
that implements it. These are the obligations the scenarios name; whether a
scenario proves its own is the auditor's question, not this file's.

1. **The bundle layout is the wire.** `spec/02-content-tree.md` §2.3 fixes the
   on-disk namespace — `did.json`, `manifest.json`, `manifests/<h>.json`,
   `e/<zone>/…`, headers under `hdr/`. The layout is enforced by the store-key
   grammar `validate_store_key`
   (`rust/crates/aithos-bundle/src/lib.rs:142`) and the display-path grammar
   `validate_display_path` (`:89`), over the helpers `name_accepted` (`:40`),
   `sid_accepted` (`:48`), `hash_accepted` (`:53`), `short_hash_accepted`
   (`:59`), `relative_segments` (`:66`).
2. **An edition is a signed manifest pinning every file.** `spec/02-content-tree.md`
   §2.6: `edition: {height, prev_hash, created_at}`, `roots`, `gamma_head`,
   signed by the owner root or by a delegate with `authorized_via`;
   `prev_hash` = SHA-256 of the prior manifest's JCS with `signature=""`.
   Implemented by `Manifest` (`rust/crates/aithos-bundle/src/manifest.rs:29`),
   `Manifest::build` (`:103`), `build_spec` (`:135`), `build_draft2` (`:170`),
   `chain_hash` (`:98`), `verify_form` (`:211`), `verify_signature` (`:240`),
   `verify_delegate_signature` (`:259`), `verify_actor_signature` (`:284`).
3. **Editions form a linear chain, and the whole chain is walked.**
   `Bundle::verify` (`rust/crates/aithos-bundle/src/bundle.rs:1691`) walks
   `1..=latest.edition.height`, checks each height, each `prev_hash` against the
   predecessor's `chain_hash()`, refuses a non-empty `prev_hash` at height 1,
   requires `manifest.json` to be the chain tip, then re-hashes every pinned
   file and refuses an unpinned stray.
4. **I3 reaches the edition.** `spec/03-headers.md:36-37` and
   `spec/09-cli-and-conformance.md:99-101` (§9.4): a Core reader "MUST reject an
   edition pinning a header that violates I3 (§03.1) — without holding any key,
   and on every `aithos-core` manifest profile". Implemented by
   `verify_pinned_headers` (`rust/crates/aithos-bundle/src/bundle.rs:302-320`),
   called from `Bundle::verify` (`bundle.rs:1759`) and from
   `publication::cold_verify` (`publication.rs:897`).
   `QUEUE.yaml`'s `chdr-028` records that a third public verifier —
   `verify_draft2_candidate` (`publication.rs:469`) behind
   `verify_public_only` (`:586`), `verify_for_cas` (`:643`) and
   `PublicationUploadPlan::verified` (`sdk.rs:35`) — does not call it. That
   statement is **published in full** in
   `docs/audits/features/c-headers.md` §6bis and is a debt of this cycle; see
   `STATE.md`.
5. **The public zone reads with no key, and carries its own signature.**
   `spec/02-content-tree.md` §2.11: owner content signatures cover JCS of
   `{zone, path, sid, body_hash}`; in `public` the signature ships in the index
   row and may travel detached. Implemented by `Bundle::public_read`
   (`bundle.rs:1264`) and `public_read_k1c` (`:1296`), both associated functions
   taking `&S` and no `OwnerKeys`.
6. **`self` is a flat sea of opaque sids.** `spec/02-content-tree.md` §2.8:
   names, titles, tags and parent/child links live inside ciphertext; each
   `self` folder has a sealed descriptor listing `{name, children:[sids]}`, and
   an authorized reader reconstructs exactly the sub-tree it can open.
   Implemented by `SelfIndex` / `SelfRow` / `SelfAccess`
   (`bundle.rs:77`, `:82`, `:93`), `resolve_self_section` (`:1402`),
   `zone_tree` (`:1412`) and `zone_entries_with_owner_kex` (`:1430`).
7. **Display paths resolve through names, keys through sids.**
   `spec/02-content-tree.md` §2.2 and §2.9: a rename moves a label, never a key,
   because derivation and headers hang off the sid path. Implemented by
   `Bundle::rename_folder` (`bundle.rs:1571`) and
   `resolve_clear` (`:1193`); the structural machinery is
   `rust/crates/aithos-bundle/src/structure.rs`.
8. **One logical linearization point, and nothing partial before it.**
   `spec/02-content-tree.md` §2.12 (CB1 decision G-B): a mutation is calculated
   against an immutable snapshot in an overlay, reduced to a deterministic
   write-set, and only then committed; "rejection or failure before that point
   leaves the canonical bundle byte-for-byte unchanged: no advanced manifest or
   Gamma head, partial index, header, wrap, blob, or orphan". Implemented by
   `Bundle::transaction` (`bundle.rs:421`) over the `Store` transaction methods
   `begin_transaction` / `commit_transaction` / `rollback_transaction` /
   `recover_transaction` / `transaction_active`
   (`rust/crates/aithos-bundle/src/lib.rs:277-302`), with `MemStore` (`:308`)
   committing by replacing its canonical state through an `overlay`, and
   `FsStore` (`:400`) staging outside the canonical directory behind the
   generation pointer `.aithos-current` / `.aithos-generations`.
9. **The keyless façade is the only public assembly boundary.**
   `spec/02-content-tree.md` §2.12 (CB1 decision G-D): "Exporting an edition into
   a fresh `MemStore` or `FsStore` and reopening it without owner or grantee
   private capabilities MUST be sufficient to verify owner and delegated
   history." Implemented by `Bundle::open` (`bundle.rs:644`) and by
   `publication::cold_verify` (`publication.rs:836`) /
   `cold_verify_for_cas` (`:957`), with `export_keyless` (`:651`) and
   `import_keyless` (`:729`).
10. **A capability is narrow, typed and purpose-bound.**
    `spec/01-identity-and-keys.md:140-166`: "A protocol operation receives only
    the narrow opaque capability it needs — signing, opening, or wrapping … A
    generic `sign(bytes)`, decrypt-bytes, cross-context opening, or wrap-bytes
    oracle is not a compliant Bundle API, and a capability for one artifact
    class cannot substitute for another", and "Stable APIs MUST NOT require a
    raw seed or private key when the narrow operation suffices, and MUST NOT
    expose private material as an output." The session surface is
    `rust/crates/aithos-bundle/src/session.rs`; the four typed classes the
    `Examples` table names are the manifest signature, the Gamma entry
    signature, the node-and-version-bound body open, and the
    node-version-and-recipient header line wrap.
11. **Owner-local operations need no mandate and consume no counter.**
    `spec/04-mandates.md:1861`: "Owner | Local narrow capability; operation is
    authorized without a mandate, journalized, and consumes no mandate counter
    or constraint." Implemented by `Bundle::owner_content_operation`
    (`bundle.rs:444`), whose typed inputs and outcomes are
    `OwnerContentOperation` / `OwnerContentOutcome` (exported and consumed by
    `rust/crates/aithos-bundle/tests/cb8_owner_grants.rs:7`).
12. **Gamma advances with the state, in the same transaction.**
    `spec/02-content-tree.md` §2.7 anchors the log; the manifest carries
    `gamma_head`, `gamma_roots` and `gamma_counts_root`
    (`manifest.rs:29`, `:80`). The append surface is
    `rust/crates/aithos-bundle/src/log.rs`; `Bundle::gamma_entries`
    (`log.rs:97`) is the reader the owner-parity observation counts before and
    after the operation.
13. **State roots make partial reads verifiable.** `spec/02-content-tree.md`
    §2.10 — `StateTree` (`rust/crates/aithos-bundle/src/state.rs:31`),
    `state_tree` (`:263`), `prove_section` (`:289`), `prove_self` (`:313`),
    `tree_diff` (`:360`). This is the seam with `h-merkle.feature`; no
    `d-bundle` scenario names a proof.
14. **Untrusted input never escapes the selected root.** `spec/02-content-tree.md`
    §2.3 and §2.12; implemented by the two grammars of invariant 1 plus
    `FsStore`'s symlink handling. The `Examples` table of
    `features/d-bundle.feature:154-165` names ten cases across `MemStore` and
    `FsStore`, four of them requiring real symlinks.

Invariants 12 and 13 are stated here as context for the integration pass, not as
obligations this feature's scenarios are claimed to carry.

## Primary sources

| Subject | Path |
|---|---|
| Contract | `features/d-bundle.feature` |
| Cucumber runner and its `@wip` filter | `rust/crates/aithos-bundle/tests/cucumber.rs:20017-20040` |
| Step definitions | `rust/crates/aithos-bundle/tests/cucumber.rs` (the sole step file the runner registers) |
| Bundle object, zones, editions, verify | `rust/crates/aithos-bundle/src/bundle.rs` |
| Store trait, `MemStore`, `FsStore`, path grammars | `rust/crates/aithos-bundle/src/lib.rs` |
| Manifest wire and signatures | `rust/crates/aithos-bundle/src/manifest.rs` |
| Publication, keyless export/import, cold verify | `rust/crates/aithos-bundle/src/publication.rs` |
| Publication upload plan (SDK surface) | `rust/crates/aithos-bundle/src/sdk.rs` |
| State roots and proofs | `rust/crates/aithos-bundle/src/state.rs` |
| Gamma append and audit lines | `rust/crates/aithos-bundle/src/log.rs` |
| Structural mutations (rename, move) | `rust/crates/aithos-bundle/src/structure.rs` |
| Grants and tag views | `rust/crates/aithos-bundle/src/grants.rs` |
| Local session and narrow capabilities | `rust/crates/aithos-bundle/src/session.rs` |
| Merge / fork resolution | `rust/crates/aithos-bundle/src/merge.rs` |
| Rotation and revocation | `rust/crates/aithos-bundle/src/revoke.rs` |
| Entropy injection | `rust/crates/aithos-bundle/src/entropy.rs` |
| Remote store, **behind the `remote` feature** | `rust/crates/aithos-bundle/src/remote.rs` |
| Transaction contracts test | `rust/crates/aithos-bundle/tests/cb7_transaction_contracts.rs` |
| Owner-parity and exact grant test | `rust/crates/aithos-bundle/tests/cb8_owner_grants.rs` |
| Publication package / cold verify test | `rust/crates/aithos-bundle/tests/cb12_publication_package.rs` |
| Edition-tier I3 test | `rust/crates/aithos-bundle/tests/c3_owner_line_edition.rs` |
| Boundary vector consumer | `rust/crates/aithos-bundle/tests/cb2_bundle_boundaries.rs` |
| Authority-flow vector consumer | `rust/crates/aithos-bundle/tests/cb2_bundle_authority_flows.rs` |
| Draft2 carrier vector consumer | `rust/crates/aithos-bundle/tests/cb2_draft2_carriers.rs` |
| Version-coexistence test | `rust/crates/aithos-bundle/tests/cb2_bundle_version_coexistence.rs` |
| Store-key consumer neutrality test | `rust/crates/aithos-bundle/tests/cb2_store_key_consumer_neutrality.rs` |
| Vector pin enforcement | `rust/crates/aithos-bundle/tests/vectors_ownership.rs` |
| Edition CLI surfaces | `rust/crates/aithos-cli/src/cmd/edition_publish.rs`, `.../edition_verify.rs`, `.../edition_diff.rs`, `.../edition_merge.rs`, `.../init.rs`, `.../status.rs` |
| CI, including `clippy -D warnings` | `.github/workflows/ci.yml` |
| Specification | `spec/02-content-tree.md` § 2.1, § 2.2, § 2.3, § 2.4, § 2.6 (incl. § 2.6.1, § 2.6.2), § 2.7, § 2.8, § 2.9, § 2.10, § 2.11, § 2.12 ; `spec/00-overview.md` § 0.2 (I1, I2, I3), § 0.3, § 0.4 ; `spec/01-identity-and-keys.md` § 1.5 (narrow opaque capabilities, `:140-166`) ; `spec/03-headers.md` § 3.1 (I3) ; `spec/04-mandates.md` (owner row, `:1861`) ; `spec/07-gamma.md` ; `spec/09-cli-and-conformance.md` § 9.2, § 9.4 ; `spec/10-threat-model.md` § 10.7 |

### Vectors involved

Five vectors are read by the step definitions this feature's phrases resolve to.
All five are pinned in `vectors/ownership.json`, enforced by
`rust/crates/aithos-bundle/tests/vectors_ownership.rs`.

- **`vectors/cb2-bundle-boundaries.json`** — the transaction failure matrix and
  the confinement matrix. Included as `CB7_BOUNDARIES`
  (`cucumber.rs:96`) and read by `core_atomic_failure_scenario`
  (`cucumber.rs:1822-1836`), which refuses a `store`/`boundary` pair with no
  matching row. Also consumed by `cb2_bundle_boundaries.rs` and
  `cb7_transaction_contracts.rs`. Pinned `owner: core`,
  `sha256: 73149da6…`. Generator `vectors/gen-cb2-bundle-boundaries.py`, which
  **has** a `--check` mode.
- **`vectors/cb2-bundle-authority-flows.json`** — the owner-parity matrix.
  Included as `CB8_AUTHORITY_FLOWS` (`cucumber.rs:98`) and read by
  `core_owner_scenario` (`cucumber.rs:3361-3383`), which refuses a
  `zone`/`operation` pair with no matching row in `owner_cases`. Also consumed by
  `cb2_bundle_authority_flows.rs`. Pinned `owner: core`, `sha256: 30545958…`.
  Generator `gen-cb2-bundle-authority-flows.py`, `--check` present.
- **`vectors/cb2-draft2-carriers.json`** — included as `CB12_DRAFT2_CARRIERS`
  (`cucumber.rs:99`) and read by `core_manifest_capability_scenario`
  (`:2060`), `core_gamma_capability_scenario` (`:2983`),
  `core_body_capability_scenario` (`:3025`) and
  `core_header_capability_scenario` (`:3053`) — the four rows of the narrow
  capability `Examples` table. Pinned `owner: core`, `sha256: 2e75e9af…`,
  **`shared: true`** with `service_consumers: [aithos-provider]`, so a re-pin is
  a cross-repository cost. Generator `gen-cb2-draft2-carriers.py`, `--check`
  present.
- **`vectors/cb2-bundle-version-coexistence.json`** — draft1/draft2 chain
  coexistence after an `FsStore` cold reopen, consumed by
  `cb2_bundle_version_coexistence.rs`. Pinned `owner: core`,
  `sha256: 61b53d07…`. Generator present with `--check`.
- **`vectors/cb2-bundle-structure-vault.json`** — included as
  `CB10_STRUCTURE_VAULT` (`cucumber.rs:100-101`); it is the oracle
  `QUEUE.yaml`'s `chdr-lota-unwitnessed-oracle` and `chdr-lota-o-vault` are
  about, and it is named here because `cb10_acceptance()`
  (`cucumber.rs:6570`) is reachable from the shared verdicts, **not** because a
  `d-bundle` phrase reaches it — see § *Shared steps* for that search.

Adjacent, owned by neighbouring features but re-pinned by any change this
domain makes to a header, a root or a Gamma head:
`vectors/c3-owner-line.json` (I3, core tier, `sha256: 2686d3ab…`),
`vectors/cb2-bundle-concurrency-final.json`, `vectors/h1-merkle.json`,
`vectors/h2-gamma-roots.json`, `vectors/f1-gamma-chain.json`,
`vectors/i1-concurrency.json`, `vectors/cb2-store-key-consumer-neutrality.json`
(`sha256: bc809473…`).

`vectors/cb2-core-bundle-red-ledger.json` is the one entry in
`vectors/ownership.json` carrying `"frozen": false`.

## Shared steps, fixtures, and helpers

All step definitions are in `rust/crates/aithos-bundle/tests/cucumber.rs`, the
sole step file the runner registers (`fn main`, `:20017-20040`). Line numbers
below point at the attribute or `fn` line.

**Every one of the feature's 61 step lines — 56 distinct phrases as written —
resolves to a definition in that file.** Search: each phrase of
`features/d-bundle.feature` matched against every `#[given]`, `#[when]` and
`#[then]` attribute in `cucumber.rs`, including the `expr =` and `regex =`
forms. No phrase is unresolved. Several resolve to a **shared** definition —
one attribute carrying two phrases, one `regex =` alternative covering four
Gherkin lines, and one `#[then(expr = "{string}")]` whose entire pattern is the
`Examples` column — and each of those is named explicitly below. That the
resolution exists is a fact of the file; what each definition then asserts is
the auditor's trace.

### Rules 1–4 — editions, round-trip, public, self

- Fixtures and World helpers: `ProtocolWorld::init_bundle` (`:7358`),
  `add_circle_section` (`:7374`), `publish_bundle` (`:7397`),
  `latest_manifest` (`:7402`), `owner(seed_index)` (`:7452`).
- World fields these scenarios touch: `bundle: Option<Bundle<MemStore>>`
  (`:511`), `read_body` (`:513`), `inspected` (`:514`), `seeds`, `ent`.
- `Given`: `a_fresh_identity` (`:7696`), `an_initialised_bundle` (`:7701`),
  **`a_published_bundle` (`:7706`), which carries two phrases** —
  `a published bundle` and `a bundle with two editions`;
  `published_with_section` (`:7714`), `published_public` (`:7721`),
  `bundle_with_self` (`:7745`).
- `When`: `initialise_bundle` (`:8322`, `Bundle::init` over `MemStore`),
  `create_circle_content` (`:8338`), **`publish_edition` (`:8343`), which
  carries two phrases** — `I publish the edition` and
  `the edition is republished`; `alter_pinned_file` (`:8349`, flips one bit of
  `e/circle/index.json` through the `pub` `store` field),
  `wrong_predecessor` (`:8357`, rebuilds a `Manifest` with a zeroed
  `prev_hash`), `owner_reads_circle` (`:8382`), `rename_the_folder` (`:8394`),
  `stranger_reads_public` (`:8405`, `Bundle::<MemStore>::public_read`, no owner
  key in the step), `inspect_self_zone` (`:8414`, concatenates every object
  under `e/self/`).
- `Then`: **`edition_verifies` (`:12697`), which carries two phrases** —
  `edition 1 verifies offline` and `its integrity checks against the signed
  edition`; `manifest_pins_did` (`:12703`), `edition_two_verifies` (`:12720`),
  `edition_rejected` (`:12738`), `body_intact` (`:12743`),
  `reads_at_new_path` (`:12748`), `public_body_readable` (`:12760`),
  `self_leaks_nothing` (`:12765`, five hard-coded needles),
  `owner_reconstructs_tree` (`:12781`).

### Rule 5 — owner parity (CB8)

- `Given`: `core_owner_zone` (`:11484`), `core_owner_fixture` (`:11491`, body
  sets one boolean).
- `When`: `core_owner_operation` (`:11496`) → `core_owner_scenario`
  (`:3361`), which builds a real `Bundle<FsStore>` under a temporary root,
  publishes a fixture section, counts `gamma_entries()` before and after, and
  drives `Bundle::owner_content_operation`.
- `Then`: `core_owner_succeeds` (`:11506`), `core_owner_gamma` (`:11528`),
  `core_owner_reopens` (`:11546`).
- World fields: `core_owner_zone` (`:532`), `core_owner_fixture_ready` (`:533`),
  `core_owner_operation` (`:534`), `core_owner_observation` (`:535`);
  observation struct `CoreOwnerObservation` (`:303`).

### Rule 6 — local transaction (CB7)

- `Given`: `core_atomic_fixture` (`:11346`, also resets the Rule 7 path
  fields), `core_atomic_boundary` (`:11355`) — **this one branches on
  `w.core_revocation_failure_boundary == "__fixture__"` (`:562`), a field owned
  by another feature**, and is the shared-state coupling the integration pass
  must resolve.
- `When`: `core_atomic_failure_attempt` (`:11364`) → `core_atomic_failure_scenario`
  (`:1822`); `core_atomic_success_attempt` (`:11373`) →
  `core_atomic_success_scenario` (`:1936`). Both dispatch on the `store` string
  to `core_atomic_failure_mem` (`:1760`), `core_atomic_failure_fs` (`:1791`),
  `core_atomic_success_mem` (`:1863`), `core_atomic_success_fs` (`:1899`), over
  `core_atomic_bundle` (`:1699`), `cb7_store_snapshot` (`:1375`),
  `Cb7TempRoot` (`:1422`) and the fault enum `CoreAtomicFault` (`:1464`, `impl`
  at `:1476`).
- `Then`: `core_atomic_refused` (`:11386`), `core_atomic_unchanged` (`:11393`,
  reads `core_path_observation` first and falls back to
  `core_atomic_observation`), `core_atomic_old_head` (`:11407`),
  `core_atomic_no_failed_artifact` (`:11416`), `core_atomic_staging_clean`
  (`:11422`), `core_atomic_complete_write_set` (`:11427`),
  `core_atomic_linearized` (`:11432`), `core_atomic_recovery` (`:11439`),
  `core_atomic_no_partial_state` (`:11444`).
- Observation struct `CoreAtomicObservation` (`:313`); accessor
  `core_atomic_observation(w)` (`:11378`).

### Rule 7 — narrow capabilities and path confinement

- `Given`: `d_narrow_capability` (`:8428`).
- `When`: `d_typed_capability_operation` (`:8436`) → `core_capability_scenario`
  (`:3104`), which dispatches the four `Examples` rows to
  `core_manifest_capability_scenario` (`:2060`),
  `core_gamma_capability_scenario` (`:2983`),
  `core_body_capability_scenario` (`:3025`) and
  `core_header_capability_scenario` (`:3053`).
- `Then`: `d_capability_result` (`:8450`) is declared
  `#[then(expr = "{string}")]` — the whole step is the `<observable_result>`
  column; `d_mismatched_capability_refused` (`:8464`); and
  **`d_capability_boundary_holds` (`:8477-8481`), one `regex =` alternative
  covering the four Gherkin lines `:136`, `:137`, `:138`, `:139`.**
- The four capability observations set `cross_class_substitution_refused` from
  `core_capability_api_is_narrow()` (`:2053-2058`), which is
  `include_str!("../src/session.rs")` tested with `.contains("pub fn sign(")`,
  `.contains("pub fn open(")`, `.contains("pub fn wrap(")`.
  `features/.agents/orchestrator/QUEUE.yaml` records this exact site under
  `chdr-lota-source-text-assertions`, and its scope limit applies: sites are
  counted, not classified. Whether this one is defective is the auditor's
  determination, not this file's.
- `When` (paths): `core_path_attempt` (`:11449`) → `core_path_scenario`
  (`:3348`) → `core_path_mem_scenario` (`:3117`) or `core_path_fs_scenario`,
  which exists twice: `#[cfg(unix)]` at `:3202` and `#[cfg(not(unix))]` at
  `:3340`, the second returning `Err("CORE-OWN-004 symlink scenarios require
  Unix")`. Generation pointer helper `core_path_active_generation` (`:3195`).
- `Then`: `core_path_refused_before_access` (`:11467`) and the shared
  `core_atomic_unchanged` (`:11393`).
- World fields: `core_capability` (`:539`) … `core_capability_observation`
  (`:542`), `core_path_store` (`:543`) … `core_path_observation` (`:547`);
  observation structs `CoreCapabilityObservation` (`:325`),
  `CorePathObservation` (`:337`).

### Process-global `OnceLock` verdicts — search recorded

`QUEUE.yaml`'s `chdr-lota-proxy-verdicts` lists nine features whose Gherkin
lines resolve to the five process-global `OnceLock` verdicts, and **`d-bundle`
is not one of them**. Search performed here, against the code rather than
against the list: the statics are `CB4_ACCEPTANCE`, `CB5_CATALOG_ACCEPTANCE`,
`CB6_ACCEPTANCE`, `CB7_ACCEPTANCE`, `CB10_ACCEPTANCE`
(`cucumber.rs:1119-1128`; three more are `#[allow(dead_code)]`), their
`*_result` helpers are at `:7295-7350`, and the **only** call site of
`cb7_result` in the file is `:9592`, inside `o_catalog_overlay_fixture`
(`:9585-9593`), whose `regex =` alternatives are `o-connector-classes-vault`
phrases. No step function reached by a `d-bundle` phrase calls any `*_result`
helper. Reproduce this search rather than trusting it.

## Public surfaces that claim the same invariants

Inspect these to check whether they bypass or contradict the bundle verdict,
not to audit them.

- **Edition issuance** — `Bundle::publish` (`bundle.rs:1678`) and
  `Bundle::transaction` (`:421`). `QUEUE.yaml`'s `chdr-i3-d-bundle` records that
  `publish` carries no I3 guard while `verify` does (`CHDR-034`).
- **Edition verification, three surfaces** — `Bundle::verify` (`bundle.rs:1691`),
  `publication::cold_verify` (`publication.rs:836`) and
  `cold_verify_for_cas` (`:957`) on one side;
  `KeylessPublicationPackage::verify_public_only` (`:586`),
  `verify_for_cas` (`:643`), `verify_draft2_candidate` (`:469`),
  `verify_draft2_candidate_value` (`:534`), `assemble_draft2_candidate`,
  `package_with_objects`, `export_keyless` (`:651`), `import_keyless` (`:729`)
  on the other. `PublicationUploadPlan::verified` (`sdk.rs:35`) consumes the
  second as an acceptance verdict. This split is the subject of `chdr-028`.
- **Owner content path** — `owner_content_operation` (`bundle.rs:444`),
  `section_add` (`:778`), `section_rewrite` (`:904`), `section_delete` (`:986`),
  `ensure_folder` (`:734`), `rename_folder` (`:1571`).
- **Read paths** — `read_section` (`:1220`), `read_section_with_owner_kex`
  (`:1230`), `public_read` (`:1264`), `public_read_k1c` (`:1296`),
  `resolve_clear` (`:1193`), `resolve_self_section` (`:1402`),
  `zone_tree` (`:1412`), `zone_tree_with_owner_kex` (`:1417`),
  `zone_entries_with_owner_kex` (`:1430`), `clear_zone_tree` (`:1446`),
  `clear_zone_entries` (`:1455`).
- **Key derivation into the bundle** — `zone_dk` (`:657`),
  `zone_dk_with_owner_kex` (`:665`), `vault_dk` (`:672`),
  `owner_current_section_key_with_kex` (`:694`). `QUEUE.yaml`'s
  `chdr-i3-d-bundle` names `bundle.rs:667` and `:674` among the four production
  surfaces holding `owner_kex` that call only the keyless I3 tier
  (`CHDR-030`).
- **The `store` field is `pub`** (`bundle.rs:284` in the `Bundle<S>` struct at
  `:283`), so a caller can write an object without passing `validate_store_key`.
  `docs/audits/features/c-headers.md` §6bis records that
  `c3_owner_line_edition.rs:239-246` uses exactly that to inject a mutilated
  header. Stated here as a property of the surface; what it implies is the
  auditor's call.
- **Store backends** — `MemStore` (`lib.rs:308`) and `FsStore` (`lib.rs:400`,
  `new` at `:412`), and the default `Store` methods that make transactions
  optional (`:277-302`): a backend that implements none still compiles, and
  `rollback_transaction` / `recover_transaction` default to `Ok(())`.
- **`RemoteStore`** — `rust/crates/aithos-bundle/src/remote.rs`, gated by
  `#[cfg(feature = "remote")]` (`lib.rs:16-18`). See § *Gate pyramid* for what
  that does to the declared gates.
- **CLI** — `aithos init` (`cmd/init.rs`), `aithos edition-publish`
  (`cmd/edition_publish.rs`, calls `bundle.publish`), `aithos edition-verify`
  (`cmd/edition_verify.rs`, calls `bundle.verify`), `aithos edition-diff`,
  `aithos edition-merge`, `aithos section-add`, `aithos section-read`,
  `aithos zone-show`, `aithos status`, `aithos folder-add`,
  `aithos move-folder`. Shared helpers `bundle_at` / `owner_from` /
  `now_string` in `cmd/common.rs`.
- **WASM** — `rust/crates/aithos-wasm/src/lib.rs` exposes **no** bundle
  surface. Absence claim with its search: `aithos-wasm/Cargo.toml`
  `[dependencies]` lists `aithos-core` and not `aithos-bundle`, and
  `git grep -c "aithos_bundle" rust/crates/aithos-wasm/src/lib.rs` returns
  nothing.

## Known coupling with other features

- `c-headers.feature` — **`COMPLETE`**, and the source of three of this
  feature's recorded debts. Its domain file records the seam from the other
  side: "`d-bundle.feature` — atomicity of header and wrap writes (`:98`,
  `:106`, `:112`) and the 'node-version-and-recipient header line' capability
  row (`:146`), whose step observation is `core_header_capability_scenario`"
  (`features/.agents/c-headers/DOMAIN.md:223-226`). Full text of the debts in
  `STATE.md`.
- `b-derivation.feature` — `COMPLETE`. The round-1 impact review opened a
  step-coupling record over `rename_the_folder`, `publish_edition` and
  `reads_at_new_path`, all three of which are `d-bundle` steps; the round-2
  review confirmed it unchanged. Full text in `STATE.md`.
- `h-merkle.feature` — the state roots the manifest pins (`state.rs`,
  spec § 2.10). Next in the queue after this feature.
- `i-concurrency.feature` — `MemStore`/`FsStore` transaction behaviour under
  contention; `rust/crates/aithos-bundle/tests/i1_concurrency.rs` and
  `cb13_concurrency_final.rs` are its test binaries and they exercise the same
  `Store` transaction methods.
- `n-structural-mutations.feature` — rename and move as structural mutations
  over the same indexes (`structure.rs`), and the transactionality of
  `move_folder` (`QUEUE.yaml`, `chdr-i3-n-structural`).
- `g-revocation.feature` — rotation republishes an edition; co-owner of
  `chdr-016-grant-path` with this feature.
- `k-integration.feature` — cold verification of what this feature publishes;
  co-owner of `chdr-028` with this feature, and second in line for it.
- `l-delegated-writes.feature` / `m-delegated-editions.feature` — the delegated
  half of § 2.6.1, reached from `Bundle::verify`'s `authorized_via` branch
  (`bundle.rs:1699-1712`).
- `o-connector-classes-vault.feature` — shares `CB7_ACCEPTANCE` at
  `cucumber.rs:9592` and the `core_revocation_failure_boundary` field
  (`:562`) that `core_atomic_boundary` (`:11355`) branches on.
- `f-gamma.feature` / `h2-gamma-roots.feature` — the `gamma_head`,
  `gamma_roots` and `gamma_counts_root` the manifest carries.

Textual proximity is not a semantic dependency. Inspect these to check whether
they bypass or contradict this feature's verdict, not to audit them.

## Gate pyramid

Canonical feature tag: `@d-bundle` (`features/d-bundle.feature:1`).

**No role in this train executes any of these commands itself.** A role names
the exact command and stops; the orchestrator runs it, hashes the transcript,
journals it under an `evidence_id` and returns the text
(`features/.agents/orchestrator/LEDGER.md:23-53`). A report citing a command
with no matching ledger entry is invalid.

Run the static check from the repository root:

```text
features/.agents/scripts/verify-feature-tags.sh
```

Mandatory before any audit, correction or review (`features/AGENTS.md:55`,
`features/.agents/PROCESS.md:58-59`). It is also CI's first step
(`.github/workflows/ci.yml:21-22`).

Run every Cargo command from the repository root with the workspace manifest.

### Auditor evidence — feature tier, once per immutable revision

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-bundle
```

The auditor runs no unfiltered Cucumber, broad regression, or workspace gate. It
may name one exact focused test only to resolve a semantic contradiction.
Read § *Reading the counters* before interpreting this gate's output.

### Focused tier — the exact tests that reach these surfaces

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb7_transaction_contracts
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb8_owner_grants
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb12_publication_package
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test c3_owner_line_edition
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_boundaries
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_authority_flows
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_draft2_carriers
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cb2_bundle_version_coexistence
```

Name a single test with `-- --exact <name>` when one semantic contradiction is
at stake. The test names as they stand on disk:

- `cb7_transaction_contracts`: `cb7_memstore_failure_boundaries_are_byte_exact`
  (`:165`), `cb7_fsstore_failure_recovery_and_reopen_are_byte_exact` (`:192`),
  `cb7_bundle_mutation_and_publication_share_one_memstore_linearization`
  (`:242`), `cb7_bundle_fsstore_commit_reopens_as_one_complete_generation`
  (`:277`), `cb7_vector_paths_reach_the_mandatory_confinement_gate` (`:296`),
  `cb7_fsstore_rejects_intermediate_and_final_symlink_escape` (`:322`).
- `cb8_owner_grants`:
  `cb8_owner_operation_surface_has_durable_parity_for_all_fifteen_pairs`
  (`:106`), `cb8_generic_grant_delivers_each_exact_line_and_survives_reopen`
  (`:217`).
- `cb12_publication_package`:
  `cb12_bundle_assembles_the_exact_signed_draft2_candidate` (`:349`),
  `cb12_bundle_preserves_all_37_closed_error_boundaries` (`:374`),
  `cb12_capabilities_are_class_bound_and_rejected_across_sessions` (`:402`),
  `cb12_owner_package_survives_fresh_mem_and_fs_cold_verification` (`:461`),
  `cb12_export_deduplicates_identical_content_addressed_sidecars` (`:587`).
- `c3_owner_line_edition`:
  `c3_edition_owner_line_names_the_did_documents_owner_kex` (`:194`),
  `c3_edition_with_no_owner_line_at_all_is_rejected` (`:225`),
  `c3_edition_with_owner_labelled_foreign_key_is_rejected` (`:259`).
- `cb2_bundle_version_coexistence`:
  `cb2_bundle_version_coexistence_vector_and_historical_bytes_are_frozen`
  (`:142`), `cb2_homogeneous_draft1_and_draft2_chains_coexist_after_fsstore_reopen`
  (`:203`), `cb2_mixed_draft1_to_draft2_authorized_via_is_typed_invalid_mandate`
  (`:277`), `cb2_mixed_draft2_to_draft1_authorized_via_is_typed_invalid_mandate`
  (`:282`).

**There is no test binary named for this feature.** Search:
`ls rust/crates/*/tests/*.rs` over the five crates lists 38 (`aithos-core`) +
19 (`aithos-bundle`) + 2 (`aithos-cli`) + 0 (`aithos-owner`) + 0
(`aithos-wasm`) files; none is named `d_*` or `d-bundle*`. The nine binaries
above are named for their CB lot, not for this feature.

### Relevant regressions — corrector, after the final correction

**Every multi-binary invocation carries `--no-fail-fast`.** `cargo test` aborts
at the first failing test binary; without the flag a multi-binary regression
silently under-reports, which is exactly the defect
`features/.agents/orchestrator/QUEUE.yaml` records as
`chdr-lota-clippy-and-fail-fast`.

```text
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb7_transaction_contracts --test cb8_owner_grants --test cb12_publication_package --test c3_owner_line_edition
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb2_bundle_boundaries --test cb2_bundle_authority_flows --test cb2_draft2_carriers --test cb2_bundle_version_coexistence --test cb2_store_key_consumer_neutrality
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-bundle --test cb10_structure_vault --test cb13_concurrency_final --test i1_concurrency --test vectors_ownership
cargo test --manifest-path rust/Cargo.toml --no-fail-fast -p aithos-core --test c3_owner_line --test h1_merkle --test h2_gamma_roots --test f1_gamma
```

Why each: `cb7_transaction_contracts` is the transaction and confinement
contract of Rule 6 and Rule 7; `cb8_owner_grants` is the fifteen owner-parity
pairs of Rule 5; `cb12_publication_package` covers the publication package,
cold verification and the class-bound capabilities of Rule 7;
`c3_owner_line_edition` covers the edition-tier I3 pass inside
`Bundle::verify`; `cb2_bundle_boundaries`, `cb2_bundle_authority_flows`,
`cb2_draft2_carriers` and `cb2_bundle_version_coexistence` are the consumers of
the four vectors this domain's steps read, plus the draft1/draft2 coexistence
of the chain; `cb2_store_key_consumer_neutrality` pins the closed store-key
grammar of invariant 1; `cb10_structure_vault` exercises rename and move over
the same indexes; `cb13_concurrency_final` and `i1_concurrency` exercise the
same `Store` transaction methods under contention; `vectors_ownership` fails if
any vector moves without its `sha256` pin being updated; `c3_owner_line`,
`h1_merkle`, `h2_gamma_roots` and `f1_gamma` are the Core-tier oracles for the
owner line, the state roots and the Gamma head the manifest pins.

If a test does not exist on the examined baseline, report that fact instead of
turning its absence into success.

### Vector `--check` — corrector, only if a vector is touched

```text
python3 vectors/gen-cb2-bundle-boundaries.py --check
python3 vectors/gen-cb2-bundle-authority-flows.py --check
python3 vectors/gen-cb2-draft2-carriers.py --check
python3 vectors/gen-cb2-bundle-version-coexistence.py --check
python3 vectors/gen-cb2-bundle-structure-vault.py --check
python3 vectors/gen-c.py --check
```

All six have a `--check` mode. Search: over the 29 `vectors/gen-*.py`, the nine
with **no** `--check` at all are `gen-cb2-max-children.py`, `gen-eplus.py`,
`gen-f.py`, `gen-fplus.py`, `gen-g.py`, `gen-gplus.py`, `gen-h.py`,
`gen-h2.py`, `gen-i.py` — none of them among this domain's own vectors, and
`gen-h.py`, `gen-h2.py` and `gen-i.py` sit on the neighbouring vectors listed
above. **No CI step runs Python**: `.github/workflows/ci.yml` has two jobs and
its steps are checkout, the feature-tag pre-gate, the pinned `aithos-client`
checkout, toolchain, `fmt`, `clippy`, `cargo test`, and the wasm `cargo check`.
So a vector `--check` is verified only when a role names it.
`QUEUE.yaml`'s `chdr-lota-vector-generators` records the wider debt and says it
is owed by "the first cycle to touch a vector".

### Final global gates — corrector, once before review handoff

```text
cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber
cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings
```

And, **only if the correction touched `aithos-core`**, because `aithos-wasm`
depends on `aithos-core` and its browser target is checked by no native test:

```text
cargo check -p aithos-wasm --target wasm32-unknown-unknown --manifest-path rust/Cargo.toml
```

Two notes on the scope of these four, both factual:

- **`clippy`.** CI enforces it with `-D warnings`
  (`.github/workflows/ci.yml:34-35`) and, per `QUEUE.yaml`'s
  `chdr-lota-clippy-and-fail-fast`, no `DOMAIN.md` in this repository named it
  before `g4-client-surfaces`. A correction green on `cargo test` and red on
  `clippy` is red.
- **The `remote` feature is in no gate.**
  `rust/crates/aithos-bundle/src/remote.rs` (1150 lines) is behind
  `#[cfg(feature = "remote")]` (`lib.rs:16-18`); `aithos-bundle/Cargo.toml`
  declares no `default` feature set containing it, and
  `git grep -n 'features = \["remote"\]|--features remote'` over `rust/` and
  `.github/` returns nothing. `cargo test --workspace` therefore does not
  compile it. Stated so that no role reads a green workspace gate as covering
  that module.

`cargo test --workspace` in CI (`ci.yml:37`) carries no `--no-fail-fast`, so CI
under-reports a multi-binary failure the same way; the corrector's own workspace
gate above does carry it, and the two are therefore not interchangeable
evidence.

### Reading the counters

`features/d-bundle.feature:1` carries `@d-bundle` and **no** `@wip`, so the
runner's filter (`cucumber.rs:20034-20038`, which excludes `wip` at feature,
rule and scenario level) does not exclude it, and `--tags @d-bundle` is expected
to select it.

The reference counts of the file on disk at `7058a96`, for comparison with
whatever a future gate prints: **1 feature / 7 rules / 51 scenarios / 299
steps**, obtained by expanding the five `Examples` tables
(15 + 12 + 2 + 4 + 10 rows over 6, 8, 6, 8 and 4 steps respectively, plus eight
plain scenarios totalling 29 steps). A gate that reports a different count has
not run this contract — but the count above is a count of a file, and only the
transcript is evidence.

`features/.agents/orchestrator/LEDGER.md:44-52` is explicit about how a gate is
recorded: `green` is computed and never asserted, and four disagreements are
treated as **red** whatever the exit code says — exit 0 with failures reported,
exit 0 with no counters at all, exit 0 with zero scenarios selected, and a
non-zero exit with no failure reported. Printed counters bind as tightly as the
exit code.

Never restate a gate result read from a document. Run the gate — that is, name
it and let the orchestrator run it — or cite the ledger entry of a run. A
written record of a past gate is history, and `PROCESS.md` § *Evidence
hierarchy* is explicit that history is context, not proof.

## Pilot limits

Audit only the semantic truth of the thirteen existing scenario blocks and the
fifty-one rows they expand to. Do not design new general scenarios, and do not
extend the audit into revocation, structural mutation, Merkle, Gamma,
concurrency or the delegated-edition features — report the impact instead.
Findings take stable `DBND-*` identifiers.

The recorded follow-ups in `STATE.md` are the exception the process allows:
they are debts this cycle owes, and a correction they require is in scope even
when it touches a surface no scenario names. `features/AGENTS.md`
§ *Role boundaries*: "Additional tests needed to prove a requested correction
remain in scope."

## Open questions this domain could not resolve

1. **Which of `d-bundle` and `g-revocation` carries `CHDR-016` is undecided,
   and this cycle must decide it.** `QUEUE.yaml`'s `chdr-016-grant-path` says
   "Owed jointly by `g-revocation` and `d-bundle`; whichever opens first states
   which one carries it." `d-bundle` is at position 2 of the `order:` list and
   `g-revocation` at position 9, so this cycle is the first to open. The
   statement is owed by the role that opens the round, in its run report, and
   is not made here: a bootstrapper that assigned it would be deciding scope
   without evidence.
2. **`QUEUE.yaml`'s `spec-cons-12` entry still reads "BLOCKING, embargo —
   identifier and neutral title only. Held by the orchestrator", while the owner
   lifted that embargo on 2026-08-04T13:00Z.**
   `features/.agents/orchestrator/BLOCKED.md:38-49` records the ruling —
   "publier les trois en entier, maintenant. `CHDR-028`, `SC-12`, et le bord
   code de `SC-05`" — and `docs/SPEC-CONSISTENCY-2026-08-04.md:48` confirms both
   texts are published. `QUEUE.yaml` is the orchestrator's file and this domain
   does not edit it; the discrepancy is reported, not resolved. A role that
   meets the stale line follows `BLOCKED.md`.
3. **`PROCESS.md` does not contain four sections that other tracked files cite
   by name**, and `g4-client-surfaces/DOMAIN.md:577-591` already records the
   gap. At `7058a96` the file has eleven `##` headings and none of them is
   "Blocking conditions", "Orchestrated gate execution", "Material isolation of
   Pass A", or a section governing the refutation panel and the disclosure gate.
   Roles on this feature obey the rules as stated in `QUEUE.yaml`, `LEDGER.md`
   and `BLOCKED.md`, and report the gap rather than inventing the missing text.
4. **No specification section governs the narrow-capability `Examples` table as
   such.** `spec/01-identity-and-keys.md:140-166` states the rule and is routed
   above as invariant 10, but it names no artifact classes and no observable
   results; the four rows of `features/d-bundle.feature:142-146` are a closed
   table with no normative counterpart this file could find.
   `grep -rn "narrow\|opaque capability\|purpose-bound" spec/*.md` returns
   eleven lines across six files, and the only ones about Bundle capabilities
   are `01-identity-and-keys.md:42`, `:124`, `:144`, `:165` and
   `04-mandates.md:1861`. Whether that is a specification gap is a finding for
   the auditor, not a fact this file asserts.
5. **`docs/audits/features/README.md` reserves no identifier family for this
   feature.** `DBND-*` is prescribed in § *Contract* with the search showing it
   unused, but no existing tracked file blesses it. The auditor adds the index
   row when it creates the public audit.
6. **Whether `base_main` for this cycle is the `main` revision `805582a` that
   integrated `c-headers` lot A, or a later one, cannot be established here.**
   `features/.agents/c-headers/STATE.md:26` records the merge; the role that
   opens the round freezes the exact revision and records it.

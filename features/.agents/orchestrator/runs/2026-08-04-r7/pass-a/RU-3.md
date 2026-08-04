# PASS A — `d-bundle`, RU-3 and RU-4

Auditor: Pass A, units `RU-3` (`features/d-bundle.feature:45`) and `RU-4`
(`features/d-bundle.feature:53`). Finding family `DBND-`, numbered `DBND-3xx`
for RU-3 and `DBND-4xx` for RU-4 as instructed; I could not coordinate
numbering with the other six auditors, so collisions with other `DBND-3xx`
outside my two units are possible.

Material: `/root/work/passA-d-bundle/RU-3` only. `/root/work/aithos-core` was
not opened. **No gate, test, `cargo` or other command was run by me.** Every
behavioural claim below is marked either *static* (read from the source text of
this archive) or *requested* (needs an `evidence_id` before it may be asserted).
Nothing in this file is asserted as a measured behaviour without an
`evidence_id`; the mutant predictions in §2 and §5 are explicitly predictions.

Read as a single sitting with `RU-2` (`:32`), per `INVENTORY.md` §1.8.

---

## 1. Scenario by scenario

### 1.1 RU-3 — `Rule: The public zone reads without any key` (`features/d-bundle.feature:45`)

Scenario `A stranger reads public content with no key at all` (`:47`), four steps.

| Gherkin | Step definition | What the body does |
|---|---|---|
| `Given a published bundle with a public section "bio" in folder "profil"` (`:48`) | `published_public`, `rust/crates/aithos-bundle/tests/cucumber.rs:7721-7743` | `init_bundle()`, then `section_add` with `zone: Zone::Public, folder_path: "profil", name: "bio", title: "bio", tags: &[], body: PUB_BODY`, then `publish_bundle()`. |
| `When a stranger with no key reads "profil/bio" from public` (`:49`) | `stranger_reads_public`, `cucumber.rs:8405-8412` | `w.read_body = Some(Bundle::<MemStore>::public_read(&w.bundle…store, &path).map_err(…))`. |
| `Then the section body is readable in clear` (`:50`) | `public_body_readable`, `cucumber.rs:12760-12763` | `assert_eq!(w.read_body…as_deref(), Ok(PUB_BODY))`. |
| `And its integrity checks against the signed edition` (`:51`) | `edition_verifies`, `cucumber.rs:12697-12701` | `w.bundle…verify().expect("edition valid")`. |

Production surface: `Bundle::public_read`, `rust/crates/aithos-bundle/src/bundle.rs:1264-1289`.

**Does the claim meet what executes?**

*The "without any key" half: yes, and by the strongest available means.* The
signature is `pub fn public_read(store: &S, display_path: &str) -> Result<String>`
(`bundle.rs:1264`). It takes no `OwnerKeys`, no `StaticSecret`, no key of any
kind; the step passes only `&bundle.store`. Keylessness here is a type-level
fact, not an assertion that could rot — stronger than any `Then` could be. It
is also the only zone arm reachable without a key: `read_section_with_owner_kex`
(`bundle.rs:1229-1259`) routes `Zone::Public` to `public_read` and both other
zones to keyed paths. I attacked this and could not break it (§4).

*The "readable in clear" half: yes.* `public_body_readable` (`:12760`) compares
the read against `PUB_BODY` (`cucumber.rs:69`), the constant the `Given` wrote.
The `Given` wrote it, the `When` read it, the `Then` compares to the constant —
not to the value the `When` produced — so this is a genuine assertion, not a
round-trip on its own `When`. It is the one step of the scenario that carries
assertive weight. `INVENTORY.md` §4.7 reads line 50 as "largely restating" the
`When`; at code level that reading is too harsh — `public_read` can succeed and
return the wrong bytes, and `:12762` would catch it.

*The "integrity … against the signed edition" half: no.* `edition_verifies`
(`:12697-12701`) never touches `w.read_body`. It calls `Bundle::verify`
(`bundle.rs:1691`), a whole-bundle check of the manifest chain, pinned file
hashes, headers, gamma and Merkle roots. Its body is shared verbatim with RU-1's
`Then edition 1 verifies offline` (`features/d-bundle.feature:13`) — two
`#[then]` attributes on one function. It would pass identically on a bundle that
contains no public section at all. The word "its" in line 51 has no referent in
the executing code. See `DBND-301`.

### 1.2 RU-4 — `Rule: The self zone leaks no structure` (`features/d-bundle.feature:53`)

Scenario `Self is a flat sea of opaque blobs` (`:55`), four steps.

| Gherkin | Step definition | What the body does |
|---|---|---|
| `Given a bundle with a self folder "enfance/cicatrices" containing section "blessure"` (`:56`) | `bundle_with_self`, `cucumber.rs:7745-7767` | `init_bundle()`, then `section_add` with `zone: Zone::Self_, folder_path: "enfance/cicatrices", name: "blessure", title: "cicatrice au genou", tags: &["sante"], body: SELF_BODY`, then `publish_bundle()`. |
| `When I inspect every file of the self zone as a stranger` (`:57`) | `inspect_self_zone`, `cucumber.rs:8414-8424` | `for path in store.list("e/self/")? { all.push_str(&String::from_utf8_lossy(&store.get(&path)??)) }` then `w.inspected = all`. |
| `Then no folder name, section name, title or tag appears anywhere` (`:58`) | `self_leaks_nothing`, `cucumber.rs:12765-12779` | five `assert!(!w.inspected.contains(needle))` over the hard-coded needles `"enfance"`, `"cicatrices"`, `"blessure"`, `"cicatrice au genou"`, `"sante"`. |
| `And the owner still reconstructs the full tree from sealed descriptors` (`:59`) | `owner_reconstructs_tree`, `cucumber.rs:12781-12799` | `zone_tree(Zone::Self_, &owner)`, then asserts the returned `Vec<String>` contains `"enfance"`, `"enfance/cicatrices"`, `"enfance/cicatrices/blessure"`. |

Production surface: `section_add`'s `Zone::Self_` arm (`bundle.rs:860-887`),
`ensure_self_folder` (`:1080-…`), `zone_tree` → `zone_entries_with_owner_kex` →
`self_walk` (`:1412`, `:1430-1443`).

**The specification, quoted to the end of the sentence.** `spec/02-content-tree.md`
§2.8, *`self` structure secrecy* (`:473`), second sentence:

> In `self`, the tree itself is confidential. On disk and in the index, `self`
> is a flat sea of opaque sids — sections and folder descriptors
> indistinguishable; names, titles, tags, parent/child links all live
> **inside** ciphertext.

and the layout table at `spec/02-content-tree.md:67-69`:

> `e/self/index.json          FLAT opaque list: [{sid,key_version,gamma_ref}] — nothing else`
> `e/self/blobs/<sid>.enc     sections AND sealed folder descriptors, indistinguishable`
> `e/self/hdr/<node>.json     granted self nodes, sid-addressed`

and `spec/02-content-tree.md:21`:

> `/e/self …                       same shape as circle; paths opaque (§2.8)`

and, further into §2.8 (`:481-483`):

> Headers and gamma targets use sid-paths, so granting or editing a `self` node
> leaks no structure either.

The normative claim is therefore **two-layered and four-sited**: names must be
absent *on disk* (store keys / paths), *in the index*, *in headers* and *in
gamma targets*. The scenario's `When` reads exactly one of those four: the
**contents** of objects whose key begins `e/self/`.

**Does the claim meet what executes?** Partly.

- *In the index:* yes. `e/self/index.json` matches the `e/self/` prefix, so its
  bytes land in `w.inspected`. A regression that put a name into `SelfRow`
  (`bundle.rs:82-90`) would be caught.
- *On disk (store keys):* no. `inspect_self_zone` (`:8414-8424`) pushes
  `store.get(&path)` and **never `path`**. The entire store-key layer — the one
  layer where the public zone deliberately does carry names in clear
  (`e/public/<display-path>.md`, `bundle.rs:803`) — is outside the search. See
  `DBND-401`.
- *In gamma:* no. `gamma/gamma.jsonl` does not match `e/self/`. See `DBND-401`
  and the mutant in §5.
- *In headers:* no. `e/self/hdr/<sid>.json` does match the prefix and would be
  inspected, but the fixture grants nothing, so no header exists to inspect —
  the conjunct has no antecedent here.
- *Elsewhere:* `publish_artifacts` (`bundle.rs:1631-1641`) writes
  `manifests/index-self-<height>.json`, a byte copy of `e/self/index.json`,
  outside the prefix. Today it can leak nothing the inspected copy does not, but
  it is a concrete second residence of self-zone state that the word "anywhere"
  does not reach.
- *"Flat sea":* not asserted. Neither `Then` tests indistinguishability of
  sections from folder descriptors, nor blob count, size or ordering, though
  §2.8 states indistinguishability normatively. See `DBND-404`.
- *"The owner still reconstructs the full tree":* asserted, and genuinely.
  `zone_tree` (`bundle.rs:1412`) returns `entry.path` for each entry
  (`:1422-1427`), so `:12781` asserts *display paths*, not bodies. No step of
  RU-2, RU-3 or RU-4 opens a self section body. See `DBND-405`.

**I1's "title or tag" observation — settled.** `INVENTORY.md` §4.1 flags that
line 58 asserts the absence of a title and a tag that its `Given` (`:56`) never
creates, and asks whether the two conjuncts are vacuous. **They are not.** The
step body `bundle_with_self` (`cucumber.rs:7745-7767`) sets
`title: "cicatrice au genou"` and `tags: &["sante".to_owned()]`, and
`self_leaks_nothing` (`:12765-12779`) searches for exactly those two strings.
Both conjuncts have a real antecedent. But the antecedent exists **only in
Rust**: the Gherkin sentence does not mention a title or a tag, and the needles
are hard-coded in the `Then` rather than derived from the `Given`'s arguments,
so the coupling is invisible from the feature file and unenforced from either
side. That is a real but lesser defect — `DBND-403`, P3, not the vacuity I1
suspected.

---

## 2. Findings

### `DBND-401` — P2 — RU-4's absence assertion searches one of four normative layers

**Statement.** `features/d-bundle.feature:58` says "appears **anywhere**".
What executes searches the *contents* of store objects whose key begins
`e/self/`, and nothing else (`cucumber.rs:8414-8424`, `:12765-12779`). Store
keys are never examined — `all.push_str(…store.get(&path)…)` pushes the value
and drops the key. `gamma/gamma.jsonl`, `manifests/index-self-<h>.json` and
every object outside the prefix are never opened. `spec/02-content-tree.md`
§2.8 makes the claim over four sites ("On disk and in the index …"; "Headers and
gamma targets use sid-paths, so granting or editing a `self` node leaks no
structure either"), and the scenario reaches one and a half of them.

**Why P2 and not P1.** The property is *true* in this revision, and true because
of two mechanisms the scenario never touches: `validate_store_key`
(`rust/crates/aithos-bundle/src/lib.rs:142-165`) is a closed allow-list under
which the only `e/self/` keys accepted are `e/self/index.json`,
`e/self/root.enc`, `e/self/blobs/<26-char-Crockford-sid>.enc` and
`e/self/hdr/<sid|root|short-hash>.json` — a display name cannot become a self
store key at all; and `log_owner_mutation` (`log.rs:190-228`) seals the payload
for every non-public zone (`log.rs:211-224`, `body_enc: Some(body)`,
`target: None`). The defect is in the proof, not the code. But the proof is
where a future regression in either mechanism would have to be caught, and it
would not be.

**Evidence.** Static, from the bodies cited. The behavioural half is
**requested**: mutant `M-A` in §5.

**Closure criterion.** `inspect_self_zone` accumulates the store key alongside
the value, and its scope becomes `store.list("")` minus a named, justified
allow-list of objects that are permitted to be clear (`manifest.json`,
`did.json`, `e/public/**`, `certs/**`), rather than a `e/self/` prefix; the
`Then` then genuinely reads "anywhere". Alternatively a second scenario under
this Rule asserts the gamma and manifest layers explicitly. `M-A` must go red.

### `DBND-402` — P2 — the negative has no positive control, and the second `Then` is not one

**Statement.** Nothing asserts that `w.inspected` is non-empty, or that it
contains the objects that hold the self state, or how many there are.
`self_leaks_nothing` (`:12765-12779`) is five `!contains` over a `String` that
the scenario never constrains. The natural candidate for a control — `And the
owner still reconstructs the full tree` (`:59`) — is **not** one: `zone_tree`
reaches the descriptors through `store.get` at explicitly computed paths
(`bundle.rs:1092`, `:1100-1104`, `:1119`), never through `store.list`. So a
regression confined to listing leaves both `Then`s green while the `When`
inspected nothing at all.

**Evidence.** Static. Behavioural half **requested**: mutant `M-B` in §5.

**Closure criterion.** `inspect_self_zone` asserts a lower bound tied to the
fixture — at minimum `assert!(!all.is_empty())`, better
`assert!(paths.len() >= 4)` naming `index.json`, `root.enc` and the two blobs
(folder descriptor + section) the `Given` creates — and `M-B` goes red.

### `DBND-403` — P3 — the `Given` announces one state and constructs a larger one; needles hard-coded

**Statement.** `features/d-bundle.feature:56` names a folder and a section.
`bundle_with_self` (`cucumber.rs:7745-7767`) additionally supplies
`title: "cicatrice au genou"` and `tags: &["sante"]`, which are the antecedents
of two of line 58's four conjuncts; and `self_leaks_nothing` (`:12765-12779`)
hard-codes all five needles instead of deriving them from the `Given`'s
`folder` and `name` arguments. Changing the Gherkin strings at `:56` silently
decouples the assertion from the fixture; dropping the title or tags from
`:7745` silently makes two conjuncts vacuous. Neither edit produces a Gherkin-
level signal. This settles `INVENTORY.md` §4.1's question in the negative
direction it did not consider: not vacuous, but unanchored.

**Evidence.** Static; both bodies quoted above.

**Closure criterion.** The `Given` sentence names the title and the tag (e.g.
`… containing section "blessure" titled "cicatrice au genou" tagged "sante"`),
the step takes them as arguments, and the `Then` derives its needles from the
world state the `Given` recorded rather than from literals.

### `DBND-404` — P3 — "flat sea of opaque blobs" reaches no assertion

**Statement.** The scenario name (`:55`) quotes the specification's own phrase.
`spec/02-content-tree.md:475-476` states normatively: "On disk and in the index,
`self` is a flat sea of opaque sids — sections and folder descriptors
indistinguishable". Neither `Then` asserts indistinguishability, uniform
naming, uniform count or uniform size. The property holds — a self section goes
to `e/self/blobs/<sid>.enc` via `put_blob` (`bundle.rs:866-872`) and a self
folder descriptor goes to `e/self/blobs/<sid>.enc` via `write_desc`
(`bundle.rs:1118-1126`), the same shape through the same allow-list branch
(`lib.rs:161-165`) — but no step observes it. `INVENTORY.md` §4.1 asked which
properties of "flat sea" the assertion set reaches; the answer is: none of them.

**Evidence.** Static.

**Closure criterion.** A `Then` under this Rule asserts that every key returned
by `store.list("e/self/blobs/")` matches `<sid>.enc` and that a stranger cannot
partition that set into sections and folders — e.g. that the fixture's two blobs
are indistinguishable by key shape and by any clear field.

### `DBND-405` — P3 — no self body is ever read in RU-2, RU-3 or RU-4

**Statement.** `features/d-bundle.feature:59` asserts the owner reconstructs
"the full tree"; `owner_reconstructs_tree` (`:12781-12799`) asserts three
**display paths** returned by `zone_tree`, whose body maps entries to
`entry.path` (`bundle.rs:1422-1427`). No step of RU-2, RU-3 or RU-4 calls
`read_section` on `Zone::Self_`, so the `Zone::Self_` arm of
`read_section_with_owner_kex` (`bundle.rs:1249-1257`) — the code that actually
opens a self blob and returns `SelfSection::md` — is exercised by none of the
three units I1 grouped. Consequently the self zone gets structure-reconstruction
but no body round-trip anywhere in RU-2/RU-3/RU-4, while circle gets the body
(`:12743`) and public gets the body (`:12762`).

Scope of the absence claim: the fifteen step definitions reached by the eleven
authored step lines of `features/d-bundle.feature:32-59`, read in full. I make
no claim about RU-5 (`:61`), whose Examples include `| self | read |` and which
belongs to another auditor.

**Evidence.** Static.

**Closure criterion.** Either RU-2's Rule scope is stated as circle-only, or a
scenario reads a self section body back and asserts it equals `SELF_BODY`.

### `DBND-301` — P2 — RU-3's integrity `Then` is a whole-bundle verify unrelated to the read

**Statement.** `features/d-bundle.feature:51`, "**its** integrity checks against
the signed edition", resolves to `edition_verifies` (`cucumber.rs:12697-12701`),
whose entire body is `w.bundle…verify().expect("edition valid")`. It does not
read `w.read_body`, does not look at `e/public/index.json`, does not look at the
row the `When` resolved, and carries a second `#[then]` phrase — RU-1's
`edition 1 verifies offline` (`:13`) — so the same body must satisfy a scenario
in which no public section exists. The property line 51 names (the returned body
is bound to the signed edition) is real in the code — `public_read` checks
`row.blob_sha != sha256_hex(&body)` (`bundle.rs:1280-1284`) and `verify` pins
`e/public/index.json` among `latest.files` (`bundle.rs:1749-1754`) — but the
scenario asserts neither link, and asserts nothing that fails if the first link
is removed.

**Evidence.** Static. Behavioural half **requested**: mutant `M-C` in §5.

**Closure criterion.** The scenario tampers — flips one byte of
`e/public/profil/bio.md`, or edits `blob_sha` in `e/public/index.json` — and
asserts the keyless read is rejected; or a `Then` asserts that the hash the read
checked is the one the signed manifest pins. `M-C` must go red.

### `DBND-302` — P2 — the second keyless public read surface has no consumer

**Statement.** `Bundle::public_read_k1c` (`bundle.rs:1296`) is a `pub`
associated function implementing the frozen K1-C draft.2 keyless read over
`indices/public.json` and `public/sections/<sid>.md`, with its own
`row.body_hash` check (`bundle.rs:1322`). **Search:** `grep -rn "public_read_k1c"
--include=*.rs rust/ vectors/` over the whole archive returns exactly one line —
its own definition at `bundle.rs:1296`. Zero call sites in production, in the
Gherkin harness, in the eighteen other integration tests of `aithos-bundle`, in
`aithos-cli` (`cmd/section_read.rs:28` calls `public_read`, the draft.1 one) or
in `aithos-wasm`. The Rule "the public zone reads without any key" is proved for
one of the two keyless read surfaces the crate publishes, and the unproved one
is the draft.2 carrier the manifest profile requires when `version ==
CORE_DRAFT2_VERSION` (`manifest.rs:221-229`).

**Evidence.** Static, search recorded above. Whether the draft.2 carrier is
reachable at all in a published edition is **not verified** (§6).

**Closure criterion.** Either a scenario under this Rule exercises
`public_read_k1c` on a draft.2 bundle, or the function is removed as dead.

### `DBND-303` — P3 — the public row signature §2.11 promises is written and never verified

**Statement.** `spec/02-content-tree.md:637-639`:

> - **`public` — signed, in the open.** The signature ships in the index row and
>   MAY travel as a sidecar with the raw markdown: public content is made to
>   circulate detached, carrying proof of authorship *and of publication intent*.

`owner_content_sig` (`bundle.rs:346-366`) produces that signature over JCS of
`{zone, path, sid, body_hash}` and it is stored in `SectionRow.sig`
(`bundle.rs:806` for public creation; also `:835`, `:921`, `:939`). **Search:**
`grep -rn "owner_content_sig\|body_hash" --include=*.rs rust/` returns five
production sites — `bundle.rs:346` (definition), `:353`, `:355` (its own body),
`:806`/`:835`/`:921`/`:939` (producers) — and, for `body_hash`, `bundle.rs:109`
and `:125` and `:1322`, all of which belong to `K1cPublicSectionRow`, a
different field of the draft.2 carrier. `grep -rn "row.sig" --include=*.rs
rust/crates/` returns `grants.rs:1843` (`verify_public_authorship`, which
handles the *delegated* `authorship` record and requires `row.sig.is_none()`),
and three assignment sites `grants.rs:2398`, `:2440`, `structure.rs:1066`. **No
code in `rust/` verifies `SectionRow.sig`**, and no vector under `vectors/`
carries the `{zone, path, sid, body_hash}` payload (`grep -rln "body_hash"
vectors/` returns only `cb2-draft2-carriers.json`, `cb2-gamma-v2-replay.json`
and their two generators, all of which use the draft.2
`indices/public.json` row shape — verified at
`vectors/cb2-draft2-carriers.json:105`).

RU-3's Rule title is about keylessness, not about authorship, so this is P3 for
my unit: the half of §2.11 that makes public content meaningful *detached* is
neither asserted by RU-3 nor implemented as a check anywhere. It may belong more
naturally to whichever unit owns §2.11; I name it and leave the routing to the
orchestrator.

**Closure criterion.** A verifier for the owner content signature exists and a
scenario exercises it, or `owner_content_sig`'s output is removed from the
public index row and §2.11 is amended.

---

## 3. The discrimination test

This is the reason I hold both units, so I state it as its own section.

**Question.** Can any step body in RU-3 or RU-4 tell the public zone from the
self zone? Would a body that passes for one pass for the other?

**Layer 1 — the `When`s do not share a body, and cannot be pointed at the other
zone.** `stranger_reads_public` (`cucumber.rs:8405-8412`) calls
`Bundle::public_read`, which hard-codes `e/public/index.json` and
`e/public/{display_path}.md` (`bundle.rs:1268-1276`). `inspect_self_zone`
(`:8414-8424`) hard-codes `store.list("e/self/")`. There is no shared
definition, no shared regex, no zone parameter. On this axis the pair
discriminates, and it discriminates by construction rather than by assertion.

**Layer 2 — the `Then`s.** `self_leaks_nothing` (`:12765-12779`) would pass
unchanged if `w.inspected` had been built from `e/public/`, because its five
needles are the *self fixture's* strings (`"enfance"`, `"cicatrices"`,
`"blessure"`, `"cicatrice au genou"`, `"sante"`) and the public fixture uses
`"profil"`, `"bio"` (`:7721-7743`). The assertion discriminates by **fixture
vocabulary**, not by **zone mechanism**. That is the weaker of the two kinds of
discrimination, and it is the only kind present.

**Layer 3 — the sharp form, and the verdict.** The decisive question is not
"would the self assertion pass on public data" but **"does the self assertion
distinguish *protected* from *absent*?"** It does not.
`store.list("e/self/")` returning `[]` — because a prefix drifted, because a
listing regression, because the fixture built nothing — yields
`w.inspected == ""`, and `""` contains none of the five needles. All five
`assert!` calls pass. The companion `Then` (`:59`) does not rescue this, because
`zone_tree` reads by `store.get` at explicitly computed paths and never through
`store.list` (`bundle.rs:1092`, `:1100-1104`). So **the RU-4 scenario is green
on a bundle whose self zone the `When` could not see at all.**

**Verdict.** The pair discriminates on the *read path* (layer 1) and fails to
discriminate on the *assertion* (layers 2 and 3). Concretely: no step body in
either unit can tell "the self zone protected its structure" from "the self zone
was not inspected". `M-A` and `M-B` (§5) are the two mutants that decide this;
until they are run, the verdict above is a reading of the source, not a measured
fact.

**One cross-unit collision found while doing this.** `edition_verifies`
(`:12697-12701`) carries two `#[then]` phrases: RU-3's line 51 and RU-1's line
13. That is a body shared *across Rules*, and it cannot discriminate RU-3's
claim from RU-1's. It is the concrete instance of the shared-body failure mode
in my units. It is `DBND-301`.

---

## 4. What I attacked and could not break

Each of these is a place I expected a defect and did not find one. All are
static readings; none is a measured behaviour.

1. **Self store keys carry no names.** I tried to construct the sharpest mutant
   the brief asks for — make the self zone behave like the public zone by
   writing `e/self/<display_path>.md` in clear — and it will not apply.
   `validate_store_key` (`lib.rs:142-165`) is a closed allow-list; the
   `strip_prefix("e/public/") … strip_suffix(".md")` branch (`:157-160`) is
   public-only, and the `e/{circle,self}/blobs/<stem>.enc` branch (`:161-165`)
   requires `sid_accepted(stem)`, i.e. `value.len() == 26 && all bytes in
   "0123456789ABCDEFGHJKMNPQRSTVWXYZ"` (`lib.rs:49-52`). A display name cannot
   become a self store key. `MemStore::put` calls `validate_store_key`
   (`lib.rs:341`), so the rejection is at the store boundary, not at a caller.
   Path opacity is enforced centrally and is not something the scenario needs to
   catch — but the scenario also does not catch it, which is `DBND-401`.
2. **The gamma log seals the self payload.** `section_add` passes
   `json!({"blob_sha": …, "name": name})` to `log_owner_mutation`
   (`bundle.rs:889-895`) — the section name is in that payload. For
   `Zone::Public` the entry keeps `target: Some(node.to_string())` and
   `payload: Some(payload)` in clear; for every other zone the match arm at
   `log.rs:211-224` seals it into `body_enc` and sets `target: None`. The self
   name does not reach `gamma/gamma.jsonl` in clear. This is `spec/02-content-tree.md:481-483`
   implemented. The scenario asserts none of it.
3. **The self index is sid-only.** `SelfIndex`/`SelfRow` (`bundle.rs:77-90`)
   carry `sid`, `key_version` and an optional `SelfAccess {n, c}` — no name, no
   title, no tag, no parent link. `spec/02-content-tree.md:67` requires exactly
   this ("FLAT opaque list … nothing else"). This *is* covered by the scenario,
   since `e/self/index.json` matches the inspected prefix.
4. **Self folder names live in sealed descriptors.** `ensure_self_folder`
   (`bundle.rs:1080-1130`) writes `Descriptor { kind: "folder", name: seg, … }`
   through `write_desc` to `e/self/blobs/<sid>.enc` (`:1118-1126`), and reads
   it back only through `read_desc` with a derived key (`:1100-1104`).
5. **The manifest copy of the self index leaks nothing extra.**
   `publish_artifacts` (`bundle.rs:1638-1641`) duplicates `e/self/index.json`
   into `manifests/index-self-<h>.json`, outside the inspected prefix. Since
   the source is sid-only (point 3), the copy carries nothing the inspected copy
   does not. I flag it in `DBND-401` as a scope hole, not as a live leak.
6. **RU-3's keylessness.** `public_read`'s signature admits no key
   (`bundle.rs:1264`) and the step passes only `&bundle.store`
   (`cucumber.rs:8409`). I could not construct a mutant that makes the read
   consult a key without changing the signature, which no defect would do
   silently. This is the one claim in my two units that is proved better than a
   `Then` could prove it.
7. **RU-3's body assertion is not a round-trip on its own `When`.**
   `public_body_readable` (`:12762`) compares against the module constant
   `PUB_BODY` (`cucumber.rs:69`), not against anything the `When` produced.
8. **No proxy step, no cached verdict, no source-text assertion in my units.**
   Search: the process-global verdicts in this harness are the eight
   `OnceLock<Result<(), String>>` statics at `cucumber.rs:1119-1129`, consumed
   through the helpers `cb4_result`/`cb5_*_result`/`cb6_result`/`cb7_result`/
   `cb10_result` at `:7287-7356`. I read all eight of my step-definition bodies
   in full (`:7721-7743`, `:7745-7767`, `:8405-8412`, `:8414-8424`,
   `:12697-12701`, `:12760-12763`, `:12765-12779`, `:12781-12799`); none
   references any static, any `*_result` helper, or `include_str!`. Every one of
   them executes the scenario's own parameters against real production calls.
   This agrees with `features/.agents/d-bundle/DOMAIN.md` § *Process-global
   `OnceLock` verdicts — search recorded*, which states `d-bundle` is not among
   the nine features `QUEUE.yaml`'s `chdr-lota-proxy-verdicts` lists; I
   reproduced the search rather than trusting it, as that section asks.
9. **No published audit names either of my units.** Search: `grep -n "public
   zone\|self zone\|flat sea\|public_read\|e/self\|inspected\|d-bundle"
   docs/audits/features/*.md`. The 24 hits in `a-identity.md`,
   `b-derivation.md` and `c-headers.md` are about `d-bundle` as a *destination*
   of debts — `BDER-006` and the §02.9 tag-view/rename extension
   (`b-derivation.md:64`, `:302`, `:607-618`, `:712-715`, `:748`, `:835-839`,
   `:969`), `CHDR-016` re-routed to `g-revocation`/`d-bundle`
   (`c-headers.md:2929`, `:2943`, `:3034`), `chdr-028`
   (`c-headers.md:2016`), `CHDR-034`/I3 on `publish`
   (`DOMAIN.md:465`). None touches `features/d-bundle.feature:45-59`, the public
   keyless read, or the self opacity assertion. **No prior published finding
   covers RU-3 or RU-4.**

---

## 5. Mutants I want run

I run nothing. Each is stated as file, line, before, after, and the prediction
it decides. Each needs one run **without** and one **with**, and the result
needed is per-step (which step failed), not just per-scenario.

The baseline command, first, so the "without" arm is journalled:

```
cargo test -p aithos-bundle --test cucumber
```

(The harness is `harness = false` with
`ProtocolWorld::cucumber().fail_on_skipped().filter_run_and_exit(features, …)`,
`cucumber.rs:20017-20040`, so it runs every non-`@wip` feature under
`features/`. If a per-scenario filter is available in this cucumber version,
`-- --name "Self is a flat sea of opaque blobs"` and
`-- --name "A stranger reads public content with no key at all"` would be
cheaper; I could not verify from the source that `filter_run_and_exit` parses
CLI options, so please treat the unfiltered run as the reliable form.)

### `M-A` — the pair-discriminating mutant, RU-4 (decides `DBND-401`)

Make the self zone log like the public zone: publish the self section's name in
clear in the signed gamma log.

- **File** `rust/crates/aithos-bundle/src/log.rs`
- **Line** 201
- **Before** `            Zone::Public => EntrySpec {`
- **After**  `            Zone::Public | Zone::Self_ => EntrySpec {`

One token. Under it, `log_owner_mutation` takes the clear branch for self, so
the entry appended to `gamma/gamma.jsonl` carries `target:
Some(node.to_string())` and `payload: Some({"blob_sha": …, "name":
"blessure"})` in the open — the section name, which is one of the four things
`features/d-bundle.feature:58` asserts is absent, readable by anyone with the
bundle and no key.

**Prediction: `Scenario: Self is a flat sea of opaque blobs` stays GREEN.**
`inspect_self_zone` lists only `e/self/`; `gamma/gamma.jsonl` is not in it.
`zone_tree` is untouched, so line 59 also passes.

If it stays green, `DBND-401` needs no further prose — that is the whole
finding. Other scenarios elsewhere in the suite are expected to go red under
this mutant; that is irrelevant to the verdict, which is the per-step result of
this one scenario.

### `M-B` — the vacuity control, RU-4 (decides `DBND-402` and discrimination layer 3)

Make the self prefix invisible to listing while leaving every byte in place.

- **File** `rust/crates/aithos-bundle/src/lib.rs`
- **Line** 348 (inside `MemStore`'s `fn list`)
- **Before** `            .filter(|k| k.starts_with(prefix))`
- **After**  `            .filter(|k| k.starts_with(prefix) && !prefix.starts_with("e/self"))`

Scoped to the literal prefix string, so `list("")` — used by `all_pinned_files`
(`bundle.rs:1618`) and by `verify`'s stray check (`bundle.rs:1761`) — is
unaffected and publication still succeeds.

**Prediction: the scenario stays GREEN with `w.inspected == ""`.** Both `Then`s
pass. If so, `Then no folder name, section name, title or tag appears anywhere`
is confirmed vacuous under a listing regression, and `And the owner still
reconstructs the full tree` is confirmed not to be a control for it.

### `M-C` — RU-3's integrity `Then` (decides `DBND-301`)

Remove the only integrity check the keyless read performs.

- **File** `rust/crates/aithos-bundle/src/bundle.rs`
- **Lines** 1280–1284
- **Before**
  ```rust
          if row.blob_sha != sha256_hex(&body) {
              return Err(Error::SealRejected(format!(
                  "public section {display_path} does not match its pinned hash"
              )));
          }
  ```
- **After** delete those five lines.

**Prediction: `Scenario: A stranger reads public content with no key at all`
stays GREEN.** The scenario tampers with nothing, so the guard is never
exercised; and `And its integrity checks against the signed edition` is
`bundle.verify()`, which is independent of `w.read_body`. If green, the fourth
step of RU-3 is confirmed to assert nothing about the read.

### `M-D` — optional, sharpens `M-C`

If the orchestrator wants the strongest form of `DBND-301`, run `M-C` together
with a byte flip of `e/public/profil/bio.md` injected in the `Given` — but that
is a test-side change and I do not request it as a mutant. `M-C` alone decides
the finding.

### Diagnostic, not a mutant

To fix the scope of `DBND-401` and `DBND-402` in numbers rather than in
reasoning, I would like one instrumented run: add to `inspect_self_zone`
(`cucumber.rs:8415`, before the loop)

```rust
    eprintln!("RU-4 probe: {:?}", store.list("e/self/").unwrap());
```

and report the printed list plus `all.len()`. This is test instrumentation, not
a production mutant; I flag it as such so it is journalled distinctly. It tells
us exactly how many objects the `When` sees and whether the header branch of
`e/self/hdr/` has any antecedent in this fixture.

---

## 6. What I could not verify, and why

- **Every behavioural claim in this report.** I ran nothing. The mutant
  predictions in §5 are readings of the source. `DBND-401`, `DBND-402` and
  `DBND-301` each carry a static half that is settled and a behavioural half
  that is not, and each names the mutant that settles it. Until those
  `evidence_id`s exist, no finding here should be recorded as measured.
- **Whether the draft.2 keyless carrier is reachable in a published edition.**
  `DBND-302` establishes `public_read_k1c` has zero call sites. Whether
  `publish` can even produce a `CORE_DRAFT2_VERSION` manifest with the three
  K1-C carriers (`manifest.rs:221-229`) from the owner path, and therefore
  whether the function is dead or merely unwired, needs the code paths of
  `publication.rs` and `sdk.rs`, which belong to other units. I did not follow
  them.
- **The `e/self/hdr/` branch.** It matches the inspected prefix, but the RU-4
  fixture grants nothing, so no self header exists. I cannot say whether the
  conjunct would hold on a fixture that has one. The diagnostic in §5 settles
  whether any header object exists here.
- **Whether `w.inspected` is non-empty in the baseline.** Structurally it should
  contain `e/self/index.json`, `e/self/root.enc` and two `e/self/blobs/*.enc`
  (one folder descriptor per segment of `enfance/cicatrices`, plus the section —
  so plausibly five objects). I did not count them and will not assert a count
  without the diagnostic run.
- **`INVENTORY.md`'s counts.** I did not re-derive them; nothing in my findings
  depends on them.

---

## 7. Verdict on I1's RU-2 / RU-3 / RU-4 asymmetry hypothesis

`INVENTORY.md` §1.8 holds that the three Rules are one subject seen three times,
that read apart each looks complete, and that read together an asymmetry appears
— specifically that "no zone asserts all three properties, integrity against the
signed edition is asserted only for public, and body round-trip is asserted only
for circle".

**The asymmetry is real, the grouping was right, and the diagnosis is half
wrong.** Point by point, from the code:

1. **"Integrity against the signed edition is asserted only for public" —
   correct in the text, and worse than I1 could see.** It is asserted for public
   *in name only*: the step is a whole-bundle `verify()` shared with RU-1
   (`cucumber.rs:12697-12701`). So the property is asserted for *no* zone. This
   is `DBND-301`, and it is a strengthening of I1's observation, not a
   refutation.
2. **"Body round-trip is asserted only for circle" — incorrect.**
   `body_intact` (`:12743`) and `public_body_readable` (`:12762`) are the same
   assertion, `assert_eq!(w.read_body…as_deref(), Ok(CONST))`, against `BODY`
   and `PUB_BODY` respectively. Circle and public both get a body round-trip.
   The text hides this because the two Gherkin sentences read differently
   ("comes back intact" vs "is readable in clear"); the bodies are twins. I1
   could not have seen this without the code and correctly deferred it.
3. **The asymmetry I1 did not name, and the sharpest one.** **Self gets no body
   read at all.** No step of the three units calls `read_section` on
   `Zone::Self_`; the `Zone::Self_` arm of `read_section_with_owner_kex`
   (`bundle.rs:1249-1257`) is untouched. The three zones therefore divide as:
   circle = body, no structure claim; public = body, no structure claim,
   plus a nominal integrity claim that does not execute; self = structure, no
   body. Read together, that is a clean 2/1 split with the self zone alone
   lacking the one property the *other* two share, and it is invisible in the
   Gherkin because RU-4's line 59 ("reconstructs the full tree") reads like a
   read and is a `zone_tree` (`bundle.rs:1422-1427`, `entry.path`). This is
   `DBND-405`.
4. **Where the asymmetry does *not* come from.** I1's framing — "whatever
   mechanism makes the first true is the mechanism whose absence must make the
   second true" — does not hold mechanically here, and it is worth saying so.
   RU-3 and RU-2 do exercise two arms of one `match` (`bundle.rs:1237-1257`).
   RU-4 exercises **neither** — it never enters that function. The three Rules
   are one subject *in the specification* (§2.1, §2.8, §2.11) but not one
   function in the code, so a mutant on the shared dispatcher cannot decide all
   three. That is why `M-A` targets `log.rs` and not `bundle.rs`.

**Left to RU-2's auditor,** as instructed, because the answer lies there and not
in my units:

- Whether `The owner reads back what was written` (`:34`) reaches "round-trips
  through the sealed store" when its `Then` (`:37`) is
  `assert_eq!(read_body, Ok(BODY))` and "intact" is undefined in the file —
  `INVENTORY.md` §4.7 raises this and I confirm the body is exactly that
  (`cucumber.rs:12743-12746`).
- Whether the Rule title's "the sealed store" is circle-only. It reads as both
  sealed zones; only circle is exercised. That is the RU-2-side face of
  `DBND-405`.
- `Display paths resolve through names, keys through sids` (`:39`): the `Then`
  (`:43`) is `reads_at_new_path` (`cucumber.rs:12748-12756`), which reads the
  body at the renamed path and compares to `BODY`. It touches no sid and no key,
  as `INVENTORY.md` §4.1 predicted from the text alone. I confirm that from the
  code and leave the finding to its auditor.

---

## 8. Disclosure gate

**Nothing here is embargoed, and I state the reasoning rather than just the
conclusion.** Blocking condition 9 bites when a finding's *statement* describes
an exploitable weakness for which no fix exists. All seven of my findings are
about the reach of a proof, not about a live weakness: the self zone's structure
secrecy is enforced in this revision by `validate_store_key` (`lib.rs:142-165`),
by the sid-only `SelfIndex` (`bundle.rs:77-90`), by sealed folder descriptors
(`bundle.rs:1118-1126`) and by the non-public seal branch of
`log_owner_mutation` (`log.rs:211-224`) — four mechanisms I attacked and could
not break (§4). `M-A` and `M-B` describe changes *to* production code, not
weaknesses *in* it; publishing them tells an attacker nothing they could use
against `d9120d7`.

The one finding whose statement touches a real gap in shipped behaviour rather
than in tests — `DBND-303`, the owner content signature that nothing verifies —
is a missing verifier, not an exploitable weakness: no keyless reader today
*believes* an unverified signature, because no keyless reader reads it at all.
It has an obvious fix (write the verifier, or drop the field), so condition 9
does not apply. `features/AGENTS.md` § *Project stage* is alpha with nothing
deployed, so I have not softened it for compatibility either.

Should Pass B disagree and judge `DBND-303` or `DBND-302` disclosure-sensitive,
the neutral titles are already the section headings above and the full text can
be lifted out of this file without touching the rest.

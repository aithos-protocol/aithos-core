# RU-4 Pass A (frozen)

> Frozen 2026-07-30 before Pass B began. Review unit: `Rule: Rotation cuts the
> revoked and re-links the parent`. Executed by an isolated agent against a
> source-only extract of `3803fe8` with no `.git` present.

## Contamination status

Uncontaminated with respect to `c-headers`. No Git material exists in `/work/aithos-core` (no `.git`), no `git` command was run, and `docs/audits/features/{a-identity,b-derivation}.md` were not opened. No `CHDR-*` finding, prior verdict, or corrector report was read.

Three incidental exposures to *other features'* audit identifiers, disclosed for completeness, none of which carries a conclusion about RU-4:

- `docs/audits/features/README.md:77-78` (authorized as process convention) states one-line current verdicts for `a-identity` and `b-derivation`.
- `features/.agents/c-headers/DOMAIN.md:133-138` and `STATE.md:49-52` (authorized routing inputs) name the open `BDER-011` harness defect.
- `rust/crates/aithos-bundle/tests/cucumber.rs:479-485` carries source comments naming `BDER-003` / `BDER-004` on unrelated World fields.

No cargo command was run: the orchestrator owns the single canonical `@c-headers` gate. All selection evidence below is static.

## Selection evidence

- Runner: `rust/crates/aithos-bundle/tests/cucumber.rs:19724-19734`. `main` points at `concat!(CARGO_MANIFEST_DIR, "/../../../features")`, i.e. the repository-root `features/` directory, and calls `ProtocolWorld::cucumber().filter_run(features, |_, _, scenario| !scenario.tags.iter().any(|t| t == "wip"))`.
- The only exclusion filter is `@wip`. `features/c-headers.feature` contains no `@wip` tag anywhere (feature-level tag is `@c-headers` at line 1); both RU-4 scenarios (lines 48-53 and 55-58) are therefore selected.
- Harness: `rust/crates/aithos-bundle/Cargo.toml:44-46` declares `[[test]] name = "cucumber", harness = false`. `filter_run` (not `filter_run_and_exit`) is used, so the process exit code is not evidence of pass/fail on this baseline. I did not use any exit code; I did not execute the gate.
- Step uniqueness: each of the eight RU-4 phrases resolves to exactly one attribute-registered step. The `regex =` steps in the file (`cucumber.rs:7827, 7852, 7931, 8332, 8378, 8823, 9152, 9161, 9169, 9177`) are fully anchored alternations that do not contain any RU-4 phrase, so no ambiguous match exists.
- World isolation: `ProtocolWorld` is `#[derive(Debug, Default, World)]` (`cucumber.rs:459`), so each scenario receives a fresh `Default` world. `wrap_obj` (`cucumber.rs:490`) and `header` (`:487`) do not survive between scenarios. `PARENT_KEY` / `CHILD_NODE` are referenced only at `cucumber.rs:265, 262, 8168-8169, 12401` — exclusive to scenario 2.

## Scenario matrix

| Scenario | Status | Production path | What the assertion actually compares |
|---|---|---|---|
| The revoked gets no line in the new version | `PARTIAL` | `Header::build` (`header.rs:108`) → `Header::rotate` (`header.rs:192`) → `build_lines`/`seal_line` (`header.rs:79`, `seal.rs:92`) → `Header::open` (`header.rs:221`) → `open_line` (`seal.rs:110`) | `open(DID_C, 2, "g2", xsk(0x22)) == DK2`; `open(DID_C, 2, "g1", xsk(0x21)).is_err()` (bare `is_err`, no cause, no message, no structural check); `open(DID_C, 2, "owner-kex", xsk(0x0A)) == DK2`. Nothing reads `key_versions["2"].lines`. `check_rotation` is never called. |
| An up-link wrap restores derivation for the parent holder | `SEMANTIC_FALSE_POSITIVE` | `Wrap::seal` (`header.rs:331`) → `wrap_aad` + `wrap_seal` → `derive_key(CTX_WRAP_KEY, via_key)` (`seal.rs:136-142`, `derive.rs:17`); `Wrap::open` (`header.rs:351`) → `wrap_open` (`seal.rs:144`) | `Wrap::seal(...).open(DID_C, &PARENT_KEY) == DK2` — a symmetric XChaCha20-Poly1305 round-trip under the same 32-byte constant used to seal, in the same step-pair, with no header, no rotation and no derivation anywhere in the scenario. |

## Per-scenario trace

### The revoked gets no line in the new version

**Steps**

- `Given a sealed header for the owner and two grantees` → `cucumber.rs:7578-7595` (`sealed_header_three`). Calls `Header::build(DID_C, NODE_A, &DK, &[owner_rec(), grantee_rec("g1",0x21), grantee_rec("g2",0x22)], &[eph(1),eph(2),eph(3)], &[non(1),non(2),non(3)])` and stores it in `w.header`. `Header::build` → `build_at(.., version = 1, ..)` (`header.rs:116`), which runs `check_owner_line` (`header.rs:71-77`, `:133`) and inserts `key_versions["1"]` with three lines built by `build_lines` (`header.rs:79-103`) under `line_aad(DID_C, "/e/circle", 1)` (`seal.rs:35`).
- `When the node is rotated without the first grantee` → `cucumber.rs:8147-8161` (`rotate_without_g1`). Calls `w.header.rotate(DID_C, 2, &DK2, &[owner_rec(), grantee_rec("g2",0x22)], &[eph(6),eph(7)], &[non(6),non(7)])` and `.unwrap()`.
- `Then the surviving grantee opens the new node key` → `cucumber.rs:12363-12372` (`survivor_opens`).
- `And the first grantee cannot open the new version` → `cucumber.rs:12374-12382` (`revoked_cannot_open`).
- `And the owner opens the new version too` → `cucumber.rs:12384-12393` (`owner_opens_new`).

**Parameter flow**

Fixtures only; the Gherkin carries no inline parameters. `DID_C = "did:aithos:test-header"` (`:259`), `NODE_A = "/e/circle"` (`:260`), `DK = [0x77;32]` (`:263`), `DK2 = [0x66;32]` (`:264`); `owner_rec()` → `to = "owner"`, `kid = "owner-kex"`, pubkey from `xsk(0x0A)` (`:270-272`, `header.rs:22-28`); `grantee_rec(n,b)` → `to = kid = n` (`:273-279`).

**Which recipients end up in version 2 — from the code, not the step name.** `Header::rotate` (`header.rs:192-217`) runs `check_owner_line(&self.node, survivors)` then unconditionally `self.key_versions.insert("2", KeyVersion { lines: build_lines(subject_did, &self.node, 2, new_dk, survivors, ephemerals, nonces) })`. `build_lines` (`header.rs:89-102`) zips `recipients` with `ephemerals.zip(nonces)`, so exactly `min(|recipients|, |eph|, |non|)` lines are produced — here 2. Version 2 therefore contains exactly `owner-kex` and `g2`, sealed under `DK2` with AAD `line_aad(DID_C, "/e/circle", 2)`. `g1` is absent because it was never in the `survivors` slice, not because any code removed or checked it. Version `"1"` is untouched and still present (`BTreeMap::insert` of a new key).

**Assertions (one block each)**

1. `survivor_opens` (`:12363-12372`): `w.header.open(DID_C, 2, "g2", &xsk(0x22)).unwrap()` compared with `assert_eq!(dk, DK2)`. `Header::open` (`header.rs:221-246`) recomputes `line_aad(DID_C, "/e/circle", 2)`, looks up `key_versions["2"]`, filters lines with `kid == "g2"`, and returns the first `open_line` success. This compares the recovered 32 bytes against the exact expected new key `DK2` at the exact expected version `2`. Strong.
2. `revoked_cannot_open` (`:12374-12382`): `w.header.open(DID_C, 2, "g1", &xsk(0x21)).is_err()`. Bare `is_err()` — no error variant, no message, no structural inspection.
3. `owner_opens_new` (`:12384-12393`): `open(DID_C, 2, "owner-kex", &xsk(0x0A))` compared with `DK2`. Same strength as (1); also incidentally re-establishes I3 on v2 at the crypto level.

**"Gets no line" — structural claim vs. cannot-open claim.** No assertion in this scenario touches `key_versions["2"].lines`. The only structural reader of a `lines` vector in the whole header step set is the `O(1)`-grant `Then` at `cucumber.rs:12352-12361`, which belongs to RU-3. So this scenario establishes **only** "the revoked cannot open v2", never "no line addressed to the revoked exists in v2". These are different proofs, and the weaker one would also pass if a `g1` line existed but failed to decrypt for any unrelated reason.

The gap is sharper than usual here because of the shape of `Header::open`. Both failure modes converge on a *byte-identical* error: when no line matches the `kid`, the `for` loop at `header.rs:233` never executes and control falls to `header.rs:242-245`, returning `Error::SealRejected(format!("no line opens for kid {kid} on {} v{version}", self.node))`; when a `g1` line exists but `open_line` rejects it (`seal.rs:129`), the `if let Ok` at `header.rs:238` fails, the loop exhausts, and the *same* statement at `:242` produces the *same* string. Consequently no assertion phrased at the `Header::open` API — not even one matching on the message — can distinguish "no line for this kid" from "a line exists but the AEAD rejects". Proving "gets no line" requires reading `key_versions["2"].lines` directly (or calling `check_rotation`).

**`check_rotation`.** `Header::check_rotation` (`header.rs:275-305`) enforces exactly the well-formedness the Rule title asserts: every new-version `kid` must exist in version `new_version - 1` (`:288-297`, `Error::GammaRevocationRejected`), and the new version must contain an owner line (`:298-303`). It is **not** called by `Header::rotate` — `rotate` calls only `check_owner_line` (`header.rs:201`) — and it is **not** called anywhere in this scenario's `Given`/`When`/`Then`. Its only call sites in the repository are `aithos-bundle/src/revoke.rs:199`, `aithos-bundle/src/vault.rs:400`, `aithos-bundle/tests/cucumber.rs:15260` (a `g-revocation` step, not this feature), and `aithos-core/tests/g2_rotation.rs:79,92`. So the structural half of the Rule's contract is owned by a function this Rule never invokes.

**Retention of v1 (§3.5).** After the `When`, `key_versions` holds both `"1"` (owner + g1 + g2 under `DK`) and `"2"`. `g1` can therefore still open v1 with `xsk(0x21)`. Nothing asserts this either way. Per `DOMAIN.md`, the Gherkin does not claim retention behavior — the scenario text is explicitly scoped to "the new version" and the `Then` says "cannot open the new version". This is **out of scope** ("the Gherkin does not claim it"), not a finding.

**Spec comparison.** `spec/03-headers.md:63-68` step 1-2 (fresh `DK'`, `key_version += 1`, a line per surviving recipient plus the owner) is exercised faithfully with injected entropy. `spec/03-headers.md:80` ("The revoked recipient gets no line in the new version") is the sentence the scenario title paraphrases, and it is precisely the half not asserted. `spec/03-headers.md:88-90` ("the new version's lines MUST equal the previous lines minus the revoked") is `check_rotation`'s mandate, uninvoked. `spec/06-revocation.md:31-34` places the up-link wrap inside the same rung-2 act; this scenario rotates and posts no wrap.

**Provisional verdict:** `PARTIAL`. The `When` reaches real production code with real parameters, and two of the three `Then`s compare the exact expected key at the exact expected version. The third `Then` is a bare `is_err()` whose stated cause is unreachable through that API, and the title's structural claim is never checked.

### An up-link wrap restores derivation for the parent holder

**Steps**

- `Given a derived node rotated to a fresh random key` → `cucumber.rs:7597-7600` (`derived_node_rotated`). Body: `fn derived_node_rotated(_w: &mut ProtocolWorld) { }` with the comment `// Fixtures: parent key PARENT_KEY, child CHILD_NODE rotated to DK2 v2.` It takes `_w` by name-discarded binding and **writes nothing**. No `Header` is built; `w.header` stays `None` for the entire scenario. No derivation is performed; no rotation is performed.
- `When the rotator posts the up-link wrap under the parent key` → `cucumber.rs:8163-8174` (`post_uplink_wrap`): `w.wrap_obj = Some(Wrap::seal(DID_C, NODE_A, &PARENT_KEY, CHILD_NODE, 2, &DK2, non(9)))`.
- `Then a parent holder recovers the new node key through the wrap` → `cucumber.rs:12395-12404` (`parent_recovers_via_wrap`): `w.wrap_obj.open(DID_C, &PARENT_KEY).unwrap()` then `assert_eq!(dk, DK2)`.

**What "derived" and "rotated" denote here.** Nothing executable. Since the `Given` is a no-op, "derived" is a claim made only by the string `CHILD_NODE = "/e/circle/d/00000000000000000000000001"` (`:262`) *looking* like a child of `NODE_A = "/e/circle"`, and "rotated to a fresh random key" is a claim made only by the constant `DK2 = [0x66;32]` (`:264`) — a fixed, non-random, non-fresh byte pattern that no rotation produced in this scenario. The entire scenario is one constructor call and its inverse.

**Parameter flow into `Wrap::seal`.** Signature at `header.rs:331-339`: `(subject_did, via, via_key, node, key_version, dk, nonce)`. Actual: `subject_did = DID_C`; `via = NODE_A = "/e/circle"`; `via_key = PARENT_KEY = [0x55;32]`; `node = CHILD_NODE`; `key_version = 2`; `dk = DK2`; `nonce = non(9) = [0x69;24]` (`:283-285`). The argument *order* is correct — no swap between `via`/`node` or between the via key and the wrapped key.

**Is `via` consistent with the key that seals the wrap?** Structurally the wrap declares `via = "/e/circle"`, and `"/e/circle"` is exactly the path-parent of `CHILD_NODE` — so the *label* is right. But `Wrap::seal` (`header.rs:340-348`) never binds `via` into the AAD: `wrap_aad(subject_did, node, key_version)` (`seal.rs:41-43`) covers purpose ‖ did ‖ *wrapped node* ‖ key_version only, matching `spec/03-headers.md:130-135` exactly. `via` is therefore an unauthenticated routing hint, and no assertion in the scenario reads `w.wrap_obj.via` at all. Meanwhile the fixture vocabulary makes the two facts inconsistent: everywhere else in this feature `NODE_A`'s node key is `DK = [0x77;32]` (`:263`, used at `:7558`, `:7570`, `:7584`, `:8117`, `:8143`), yet the wrap that declares `via = NODE_A` is sealed under `PARENT_KEY = [0x55;32]`. Because no header exists in this scenario, that inconsistency is never executed — but it means the scenario's "parent key" is not the key of the node the wrap names as its parent.

**What the `Then` actually calls.** `Wrap::open(DID_C, &PARENT_KEY)` (`header.rs:351-357`) recomputes `wrap_aad(DID_C, CHILD_NODE, 2)` and calls `wrap_open(&PARENT_KEY, c, n, aad)` (`seal.rs:144-163`), which computes `derive_key("aithos-core/v1/wrap", PARENT_KEY)` (`seal.rs:19,150`, `derive.rs:17` → `blake3::derive_key`) and decrypts. The value returned is compared with `DK2`. That proves a symmetric XChaCha20-Poly1305 seal round-trips under the identical 32-byte constant it was sealed with, two steps earlier in the same scenario. It does **not** prove any derivation path was restored.

**Is the wrap connected to a real derivation?** No. `PARENT_KEY` is a bare fixture — `const PARENT_KEY: [u8;32] = [0x55;32]` (`cucumber.rs:265`), referenced only at `:8168` and `:12401`. It is not produced by `derive_key`, `folder_label`, or `node_key` (`derive.rs:17,26,46`), it is not opened from any header line, and it is not the key of `NODE_A` under this feature's own fixtures. Nothing computes `node_key(PARENT_KEY, path_to_CHILD_NODE)` or otherwise ties `CHILD_NODE` to `NODE_A` by `spec/02-content-tree.md` §2.5 derivation. The spec's stated purpose (`spec/03-headers.md:69-78`, `:83-87`) — that holders of `P` *or of any ancestor of P* keep reading `N` by derivation — is untested in both halves: no ancestor is present at all, and "the parent" is a constant that no derivation reaches.

**Rejection path.** None is exercised. The spec's "an up-link wrap whose author does not hold P is rejected" (`spec/03-headers.md:89-90`) has no step in this scenario. Per `DOMAIN.md` this is **out of scope** — the Gherkin does not claim it. (For the record, that rejection is claimed and exercised by `g-revocation`'s step `#[when("someone without the parent key posts an up-link wrap")]` at `cucumber.rs:15266-15292`, and by `wrap_open(&[0u8;32], ...)` at `aithos-core/tests/c1_header_seal.rs:122`; neither belongs to RU-4.)

**Vector cross-check.** `vectors/c1-header-seal.json` `"wrap"` block fixes `via_key_hex = 55…55`, `wrapped_node = "/e/circle/d/00000000000000000000000001"`, `key_version = 2`, `dk_hex = 66…66` — the same three fixture values the scenario uses — but `subject_did = "did:aithos:z6Mkopv…"` and `n_hex = "303132…4647"`. The scenario uses `DID_C = "did:aithos:test-header"` and `non(9) = [0x69;24]`. Both feed the AAD/nonce, so the scenario's ciphertext is **not** the vector's `c_hex` and cannot be. The byte-exact C2 check is reached only by `rust/crates/aithos-core/tests/c1_header_seal.rs:110-123` (`c2_wrap_roundtrip_and_cross_check`) and by `rust/crates/aithos-core/tests/g2_rotation.rs:93-123` (`uplink_wrap_bytes_match_python`). The **Gherkin scenario reaches neither**; it establishes only self-consistency between a seal and its own open, which `DOMAIN.md` names explicitly as the thing not to accept as byte-exactness.

**Provisional verdict:** `SEMANTIC_FALSE_POSITIVE`. The scenario passes without proving anything its text claims: there is no derived node, no rotation, no parent, no derivation, and no restoration — only an AEAD round-trip under one constant.

## Up-link wrap argument map

| Spec role (§3.4 2bis / §3.8) | Expected | Actual in code (file:line) | Consistent? |
|---|---|---|---|
| `subject_did` in AAD | the subject DID | `DID_C = "did:aithos:test-header"` (`cucumber.rs:259`, passed `:8166`) | Yes (differs from the C2 vector's DID, so no vector byte match) |
| `via` = parent node `P` the rotator holds | the parent of the rotated node | `NODE_A = "/e/circle"` (`cucumber.rs:260`, `:8167`); stored at `header.rs:345` | Label yes (path-parent of `CHILD_NODE`); not in AAD (`seal.rs:41-43`); never asserted |
| `K_via` = `P`'s actual node key | key of `/e/circle`, i.e. `DK` under this feature's fixtures, or a derived key | `PARENT_KEY = [0x55;32]` (`cucumber.rs:265`, `:8168`) — a bare constant, never derived, ≠ `DK = [0x77;32]` | **No** — nominal parent, unrelated key |
| wrap key = `derive("aithos-core/v1/wrap", K_via)` | BLAKE3 derive with `CTX_WRAP_KEY` | `derive_key(CTX_WRAP_KEY, via_key)` (`seal.rs:19,137,150`; `derive.rs:17`) | Yes |
| `node` = wrapped node `N` (rotated child) | the rotated node | `CHILD_NODE = "/e/circle/d/00000000000000000000000001"` (`cucumber.rs:262`, `:8169`) | Yes, in AAD via `wrap_aad` (`header.rs:340`) |
| `DK'_N` = fresh random new node key of `N` | the key produced by the rotation of `N` | `DK2 = [0x66;32]` (`cucumber.rs:264`, `:8171`) — fixed constant, produced by no rotation in this scenario | Structurally yes, semantically **no** |
| `key_version` = new version of `N` | the post-rotation version | `2` (`cucumber.rs:8170`), in AAD | Yes, but no header ever reached v2 in this scenario |
| AAD purpose | `"aithos-core/v1/tagwrap"` | `PURPOSE_WRAP` (`seal.rs:16`) via `wrap_aad` (`seal.rs:41`) | Yes |
| Nonce | injected 24 bytes | `non(9) = [0x69;24]` (`cucumber.rs:283-285`, `:8172`) | Yes (differs from vector `n_hex`) |
| Purpose: parent **or any ancestor** keeps reading by derivation | an ancestor derives `K_P`, then opens | nothing — no ancestor, no `node_key`/`derive_key` call in the scenario | **No** |

## Surface inspection

Real rotation paths in the bundle, checked for bypass only (not audited):

- `aithos-bundle/src/revoke.rs:142-215` `rotate_folder` — the canonical rung-2 path. Opens the current DK via `open_latest` (`:163`), rejects a revoked kid absent from the current lines (`:167-171`), builds survivors from the existing lines (`:172-192`), draws `new_dk`/ephemerals/nonces from injected entropy (`:195-197`), then `header.rotate(...)` (`:198`) **followed by** `header.check_rotation(new_v)?` (`:199`, comment "fail-closed: no smuggled recipient"), then posts the up-link wrap under the zone-root key (`:203-213`, `Wrap::seal` at `:205`, via = `NodePath::zone_root(zone)`), then rung-3 re-encryption. No bypass: `rotate` + `check_rotation` + posted `Wrap`, all three.
- `aithos-bundle/src/vault.rs:347-410` `rotate_vault_connector` — `header.rotate(...)` at `:392` followed by `header.check_rotation(new_version)?` at `:400`. **No up-link wrap is posted** on this path (the vault-config node has no derived parent in the content tree, so this is plausibly by design; flagged as an observation, not a finding, and belongs to `o-connector-classes-vault`).
- `aithos-bundle/src/revoke.rs:309-436` `move_folder` and `aithos-bundle/src/structure.rs:745-800` `structural` move — these re-key at a new canonical path with `Header::build_at` (`revoke.rs:413`, `structure.rs:777`) rather than `Header::rotate`, and post an up-link wrap under the new parent (`revoke.rs:428-436`, `structure.rs:788-800`). `check_rotation` cannot apply (the new header file has no predecessor version inside it), so its absence is structural, not a bypass. Belongs to `n-structural-mutations` / `g3_move`.
- `aithos-cli/src/main.rs:1178-1189` — `--rotate <folder>` computes the revoked kid and delegates to `bundle.rotate_folder(...)`, i.e. it inherits `check_rotation` and the wrap. `main.rs:1347-1403` (`header_seal_cmd` / `header_open_cmd`) are DEV surfaces over `Header::build` / `Header::open` (+ `validate` at `:1397`) and perform no rotation.
- `aithos-bundle/src/{grants,structure,bundle,state,session}.rs` contain no call to `Header::rotate`. `state.rs` never touches headers. `grants.rs:333` and `structure.rs:333,865` post *tag-view* wraps (same primitive, different role).

**Net:** the two production rotation surfaces both invoke `check_rotation`, and the content-tree one posts the up-link wrap. The gap is not in production; it is that the two RU-4 scenarios cross neither of those two guarantees.

## Candidate findings

**CHDR-RU4-a — `SEMANTIC_FALSE_POSITIVE`, high severity (security-critical: the up-link is the only route by which derivation readers survive rung-2 revocation).**
The scenario "An up-link wrap restores derivation for the parent holder" (`features/c-headers.feature:55-58`) proves only that `wrap_open(derive_key("aithos-core/v1/wrap", K), c, n, aad)` returns what `wrap_seal` put in, under the same constant `K`, two steps apart. Evidence: the `Given` is an empty body (`cucumber.rs:7597-7600`) writing no World state; the `When` (`cucumber.rs:8163-8174`) only constructs a `Wrap`; the `Then` (`cucumber.rs:12395-12404`) opens it with the very `PARENT_KEY` it was sealed under. `PARENT_KEY = [0x55;32]` (`cucumber.rs:265`) is a bare fixture produced by no derivation and equal to no node's key; `DK2 = [0x66;32]` (`cucumber.rs:264`) is produced by no rotation in this scenario; `w.header` is `None` throughout. Expected per `spec/03-headers.md:69-78` and `spec/02-content-tree.md` §2.5: a node key held by a parent, a rotation of the child to a fresh key severing `derive_key(folder_label(sid), K_P)`, and a *holder of `P` (or of an ancestor deriving `K_P`)* recovering `DK'` through the wrap. Smallest correction: make the `Given` build real state — derive `K_P` for `/e/circle` (or open it from a header line), derive the pre-rotation child key from it via `node_key`, rotate the child's header to a new key, and store both in the World; have the `Then` recover `K_P` by derivation from an ancestor key before opening the wrap, and additionally assert that the pre-rotation derived child key no longer opens the new version — that is the "severed then restored" pair the scenario name claims.

**CHDR-RU4-b — `PARTIAL`, high severity (the structural half of revocation rung 2).**
"The revoked gets no line in the new version" (`features/c-headers.feature:48-53`) never inspects `key_versions["2"].lines`. The only revocation-side assertion is `assert!(... .open(DID_C, 2, "g1", &xsk(0x21)).is_err())` (`cucumber.rs:12374-12382`). Because `Header::open` returns the identical `Error::SealRejected("no line opens for kid …")` both when no line carries the `kid` (loop at `header.rs:233` never entered) and when a line exists but `open_line` rejects (`header.rs:238` falls through to `:242-245`), the stated structural fact is unreachable through this assertion by construction. Expected per `spec/03-headers.md:80` and `:88-89`. Smallest correction: add to that `Then` a structural check — `assert!(header.key_versions["2"].lines.iter().all(|l| l.kid != "g1"))` — and/or call `header.check_rotation(2).unwrap()` in the same `Then`.

**CHDR-RU4-c — `PARTIAL`, medium severity (contract owned by an uninvoked function).**
The Rule "Rotation cuts the revoked and re-links the parent" asserts the well-formedness that `Header::check_rotation` (`header.rs:275-305`) implements, yet neither `Header::rotate` (`header.rs:192-217`, which calls only `check_owner_line` at `:201`) nor either RU-4 scenario ever calls it. Its only exercised call sites are outside this feature: `revoke.rs:199`, `vault.rs:400`, `cucumber.rs:15260` (a `g-revocation` step), `g2_rotation.rs:79,92`. Consequence: the `c-headers` Rule that names rotation as its subject would remain green if `check_rotation` were deleted. Note the boundary per `DOMAIN.md`: I am **not** reporting the absence of a smuggled-recipient scenario. I am reporting that the scenario that *does* claim the "cut" never reaches the function that decides it. Smallest correction: as in CHDR-RU4-b, invoke `check_rotation(2)` inside the existing `Then`.

**CHDR-RU4-d — `PARTIAL`, medium severity (rejection not attributed to its stated cause; vacuous-pass exposure).**
`revoked_cannot_open` (`cucumber.rs:12374-12382`) asserts only `is_err()` on a version-2 open. Failure scenario: if `rotate_without_g1` (`cucumber.rs:8147`) were removed or silently no-oped, `key_versions` would contain no `"2"` and `Header::open` would return `Error::SealRejected("no key version 2")` at `header.rs:230-232` — this `Then` would still pass. It is protected only by its two sibling `Then`s, which `unwrap()` on version 2. `DOMAIN.md` requires a rejection assertion to prove the rejection the scenario claims. Smallest correction: assert the version exists first (or assert the error's structural precondition via `key_versions["2"]`), so the failure is attributable to "no line for `g1`" rather than to "no version 2".

**CHDR-RU4-e — `PARTIAL`, medium severity (the Rule's two halves are never joined).**
Scenario 1 rotates `NODE_A` to `DK2` at v2 and posts **no** wrap; scenario 2 posts a wrap for `CHILD_NODE` at v2 under `DK2` with **no** rotation. `spec/03-headers.md:61-79` and `spec/06-revocation.md:28-34` make step 2bis part of the same rung-2 act on the same node. The shared constant `DK2` (`cucumber.rs:264`, used at `:8155` and `:8171`) makes the two scenarios *read* as a chain while no executed state links them — the two acts even target different nodes (`/e/circle` vs `/e/circle/d/…01`). Smallest correction: rotate the child node in scenario 2's `Given` and wrap *that* rotation's key, so the wrapped `DK'` is the value the rotation produced rather than a constant that happens to match.

## Shared-state observations for the integrator

- `ProtocolWorld` is `#[derive(Default, World)]` (`cucumber.rs:459`); `header`, `saved_line`, `opened`, `wrap_obj`, `rejection` (`:463`, `:487-490`) are reset per scenario. No `OnceLock`, `static mut`, or lazily-cached header state is involved in RU-4. No cross-scenario leakage found within this unit.
- `w.header` is written by `sealed_header_three` (`:7578`) and *mutated in place* by `rotate_without_g1` (`:8147`). Both RU-4 `Given`s share the fixture layer with RU-1/RU-2/RU-3 (`sealed_header_owner_grantee` `:7552`, `sealed_header_owner_only` `:7567-7568` which registers two phrases on one function, `dk_and_two_recipients` `:7547` and `single_grantee` `:7575`, both empty). The integrator should note that **three** of this feature's `Given`s are empty no-ops (`:7548`, `:7576`, `:7598`) — RU-4 owns one of them.
- `#[when("the node key is sealed into a header")]` (`:8091-8094`) delegates straight to the RU-1 `Given` function `sealed_header_owner_grantee`. Not RU-4's scenario, but it is the same anti-pattern class as CHDR-RU4-a and should be reconciled across units.
- `wrap_obj` is written only at `cucumber.rs:8165` (RU-4) and read only at `:12398`. Other features build wraps into the store directly (`:15281-15291`) rather than through this field, so `wrap_obj` is RU-4-exclusive.
- `DK2` and `CHILD_NODE` are shared between RU-4's two scenarios but, per the previous point, only as constants — worth stating in the public audit so the apparent link is not mistaken for a traced one.
- `Header::latest_version` / `open_latest` (`header.rs:251-269`) — the documented post-rotation reader path — are **not** crossed by either RU-4 scenario, which addresses version `2` by literal. They are crossed by `revoke.rs:163`, `vault.rs:121,336`, `structure.rs:201,332,864`, `bundle.rs:673`, `session.rs:364`, `core_bridge.rs:593,5525,5617`.

## Limits of this conclusion

- I did not run any cargo command. Selection, step resolution, and step uniqueness are established statically from `cucumber.rs:19724-19734`, `Cargo.toml:44-46`, and exhaustive grep; the actual executed scenario/step counts for `@c-headers` remain the orchestrator's single canonical gate to report. Per `DOMAIN.md`, the exit code of that gate is not evidence while `BDER-011` is open.
- The claim that the two failure modes of `Header::open` are indistinguishable is derived from reading `header.rs:221-246` and `seal.rs:110-132`, not from executing a mutation. It is a control-flow reading of code with no intervening branch, but it was not reproduced by a focused test.
- The claim that the RU-4 scenario cannot reproduce the C2 vector bytes rests on two input differences I read directly (`subject_did` and nonce: `vectors/c1-header-seal.json` `"wrap"` block vs `cucumber.rs:8166,8172`), both of which feed the AAD and the nonce of `wrap_seal` (`seal.rs:136-142`). I did not compute the ciphertexts to confirm they differ; the AAD/nonce difference makes equality cryptographically implausible but that is an argument, not a measurement.
- Verdicts are formed on RU-4 alone. `PARTIAL` on scenario 1 and `SEMANTIC_FALSE_POSITIVE` on scenario 2 are Pass A provisional verdicts; they are frozen here and must not be revised except by new current-code or reproducible-test evidence in Pass B.
- Surface inspection covered call sites of `Header::rotate`, `check_rotation`, `Wrap::seal`, `latest_version`, and `open_latest` across the whole `rust/` tree. I did not audit the correctness of those surfaces — only whether they bypass the invariants RU-4 claims.

# Evidence index — run 2026-08-04-r7

Regenerated from `ledger.jsonl` after round 3. Every id below resolves to a
transcript under `evidence/` whose sha256 the ledger records.

| evidence_id | verdict | passed | failed | command |
|---|---|---|---|---|
| `ev-5460ad04` | GREEN | - | 0 | `python3 features/.agents/scripts/train-status.py` |
| `ev-14592971` | GREEN | - | 0 | `bash features/.agents/scripts/verify-feature-tags.sh` |
| `ev-610f377b` | GREEN | - | 0 | `python3 features/.agents/scripts/test-train-status.py` |
| `ev-1449bd50` | RED | - | 4 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g` |
| `ev-cb4ff302` | GREEN | 836 | 0 | `cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast` |
| `ev-fafd51d8` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test cb5_evidence_contracts` |
| `ev-b8cee044` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test g1_revocation` |
| `ev-63e018d1` | GREEN | 26 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g` |
| `ev-8cdc61e6` | GREEN | - | 0 | `python3 features/.agents/scripts/train-status.py` |
| `ev-6a76a789` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-5f523aae` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-dd18154c` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --no-fail-fast --test cb7_tr` |
| `ev-d1fc33b5` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-de2706a8` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-5474b889` | RED | 20 | 31 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-23aeba39` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-f1718be8` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-0b4e1076` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-c7f65638` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-ed18d7ef` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-794d59c3` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-2d2ebd1b` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-19a635cf` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-bec6b91e` | RED | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --no-fail-fast` |
| `ev-f0125e0b` | RED | 47 | 4 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-7caa8332` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-f7261aa9` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-3fa9d172` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-1eefbb66` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-f0658ee9` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-de8fa887` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-b6a36f72` | RED | 39 | 12 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-4fa3eb28` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-0169d294` | GREEN | - | 0 | `python3 features/.agents/scripts/train-status.py` |
| `ev-dee6dbf2` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-1be2a7f4` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-1d4725c7` | RED | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-ef839413` | RED | 48 | 3 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-f18d4843` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |
| `ev-73ac972f` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d` |

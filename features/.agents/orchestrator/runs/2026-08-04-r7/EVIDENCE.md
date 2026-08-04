# Evidence index — run 2026-08-04-r7

| evidence_id | verdict | passed | failed | command |
|---|---|---|---|---|
| `ev-5460ad04` | GREEN | - | 0 | `python3 features/.agents/scripts/train-status.py` |
| `ev-14592971` | GREEN | - | 0 | `bash features/.agents/scripts/verify-feature-tags.sh` |
| `ev-610f377b` | GREEN | - | 0 | `python3 features/.agents/scripts/test-train-status.py` |
| `ev-1449bd50` | RED | - | 4 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g4-` |
| `ev-cb4ff302` | GREEN | 836 | 0 | `cargo test --manifest-path rust/Cargo.toml --workspace --no-fail-fast` |
| `ev-fafd51d8` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test cb5_evidence_contracts` |
| `ev-b8cee044` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --test g1_revocation` |
| `ev-63e018d1` | GREEN | 26 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @g-r` |
| `ev-8cdc61e6` | GREEN | - | 0 | `python3 features/.agents/scripts/train-status.py` |
| `ev-14592971` | GREEN | - | 0 | `bash features/.agents/scripts/verify-feature-tags.sh` |
| `ev-610f377b` | GREEN | - | 0 | `python3 features/.agents/scripts/test-train-status.py` |
| `ev-6a76a789` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-5f523aae` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-dd18154c` | GREEN | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --no-fail-fast --test cb7_tran` |
| `ev-d1fc33b5` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-de2706a8` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-5474b889` | RED | 20 | 31 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-23aeba39` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-f1718be8` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-0b4e1076` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-c7f65638` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-ed18d7ef` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-794d59c3` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-2d2ebd1b` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-19a635cf` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-bec6b91e` | RED | - | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-core --no-fail-fast` |
| `ev-f0125e0b` | RED | 47 | 4 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-7caa8332` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-f7261aa9` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-3fa9d172` | GREEN | 51 | 0 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-1eefbb66` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-f0658ee9` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-de8fa887` | RED | 50 | 1 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |
| `ev-b6a36f72` | RED | 39 | 12 | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @d-b` |

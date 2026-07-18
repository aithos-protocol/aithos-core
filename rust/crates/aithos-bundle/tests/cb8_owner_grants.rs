//! CB8 durable owner parity and exact generic grant delivery.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aithos_bundle::bundle::{Bundle, OwnerContentOperation, OwnerContentOutcome, SectionSpec};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::{GenericGrantRequest, GrantLineKind, GrantSelector};
use aithos_bundle::{FsStore, Store};
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::Verb;
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&base).expect("create CB8 test base");
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-cb8-owner-grants-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create CB8 root {path:?}: {error}"),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn owner() -> OwnerKeys {
    let seed = MasterSeed::from_slice(&[0x58; 32]).expect("valid CB8 owner seed");
    OwnerKeys::genesis(&seed)
}

fn snapshot(store: &impl Store) -> BTreeMap<String, Vec<u8>> {
    store
        .list("")
        .expect("list CB8 snapshot")
        .into_iter()
        .map(|path| {
            let bytes = store
                .get(&path)
                .expect("read CB8 snapshot")
                .expect("listed object exists");
            (path, bytes)
        })
        .collect()
}

fn fixture(label: &str, zone: Zone) -> (TempRoot, Bundle<FsStore>, OwnerKeys, SeqEntropy) {
    let root = TempRoot::new(label);
    let owner = owner();
    let succession = succession_from_entropy([0x68; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T11:00:00Z",
    )
    .expect("initialize CB8 bundle");
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone,
                    folder_path: "projects",
                    name: "note",
                    title: "existing",
                    tags: &["toto".to_owned()],
                    body: "before",
                    now: "2026-07-18T11:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T11:02:00Z")
        })
        .expect("publish existing CB8 section");
    (root, bundle, owner, entropy)
}

#[test]
fn cb8_owner_operation_surface_has_durable_parity_for_all_fifteen_pairs() {
    for zone in [Zone::Public, Zone::Circle, Zone::Self_] {
        for operation in ["list", "read", "create", "edit", "delete"] {
            let label = format!("{}-{operation}", zone.as_str());
            let (root, mut bundle, owner, mut entropy) = fixture(&label, zone);
            let gamma_before = bundle.gamma_entries().expect("Gamma before").len();
            let outcome = match operation {
                "list" => bundle.owner_content_operation(
                    zone,
                    OwnerContentOperation::List,
                    &owner,
                    &mut entropy,
                ),
                "read" => bundle.owner_content_operation(
                    zone,
                    OwnerContentOperation::Read {
                        display_path: "projects/note",
                    },
                    &owner,
                    &mut entropy,
                ),
                "create" => bundle.owner_content_operation(
                    zone,
                    OwnerContentOperation::Create {
                        folder_path: "projects",
                        name: "new",
                        title: "created",
                        tags: &[],
                        body: "created body",
                        now: "2026-07-18T11:03:00Z",
                    },
                    &owner,
                    &mut entropy,
                ),
                "edit" => bundle.owner_content_operation(
                    zone,
                    OwnerContentOperation::Edit {
                        display_path: "projects/note",
                        body: "after",
                        now: "2026-07-18T11:03:00Z",
                    },
                    &owner,
                    &mut entropy,
                ),
                "delete" => bundle.owner_content_operation(
                    zone,
                    OwnerContentOperation::Delete {
                        display_path: "projects/note",
                        now: "2026-07-18T11:03:00Z",
                    },
                    &owner,
                    &mut entropy,
                ),
                _ => unreachable!(),
            }
            .unwrap_or_else(|error| panic!("{label}: {error}"));

            match (operation, outcome) {
                ("list", OwnerContentOutcome::Listed(entries)) => {
                    assert!(entries.iter().any(|entry| entry.path == "projects/note"));
                }
                ("read", OwnerContentOutcome::Read(body)) => assert_eq!(body, "before"),
                ("create" | "edit" | "delete", OwnerContentOutcome::Mutated) => {}
                (operation, outcome) => {
                    panic!("{label}: unexpected {operation} outcome {outcome:?}")
                }
            }
            let gamma_after = bundle.gamma_entries().expect("Gamma after").len();
            assert_eq!(
                gamma_after - gamma_before,
                usize::from(matches!(operation, "create" | "edit" | "delete")),
                "{label}: owner reads never journalize and mutations journalize once",
            );
            drop(bundle);

            let reopened = Bundle::open(FsStore::new(root.path()))
                .unwrap_or_else(|error| panic!("{label} reopen: {error}"));
            reopened
                .verify()
                .unwrap_or_else(|error| panic!("{label} verify: {error}"));
            match operation {
                "create" => assert_eq!(
                    reopened
                        .read_section(zone, "projects/new", &owner)
                        .expect("read created section"),
                    "created body"
                ),
                "edit" => assert_eq!(
                    reopened
                        .read_section(zone, "projects/note", &owner)
                        .expect("read edited section"),
                    "after"
                ),
                "delete" => assert!(
                    reopened
                        .read_section(zone, "projects/note", &owner)
                        .is_err(),
                    "{label}: deleted section remains addressable"
                ),
                _ => assert_eq!(
                    reopened
                        .read_section(zone, "projects/note", &owner)
                        .expect("read unchanged section"),
                    "before"
                ),
            }
        }
    }
}

#[test]
fn cb8_generic_grant_delivers_each_exact_line_and_survives_reopen() {
    let root = TempRoot::new("generic");
    let owner = owner();
    let succession = succession_from_entropy([0x69; 32]);
    let agent = SigningKey::from_bytes(&[0x71; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T12:00:00Z",
    )
    .expect("initialize CB8 generic-grant bundle");
    bundle
        .transaction(|bundle| {
            for zone in [Zone::Public, Zone::Circle, Zone::Self_] {
                bundle.section_add(
                    &SectionSpec {
                        zone,
                        folder_path: "projects",
                        name: "note",
                        title: "grant target",
                        tags: &["toto".to_owned()],
                        body: "granted body",
                        now: "2026-07-18T12:01:00Z",
                    },
                    &owner,
                    &mut entropy,
                )?;
            }
            bundle.publish(&owner, "2026-07-18T12:02:00Z")
        })
        .expect("publish CB8 grant targets");

    let before_refusal = snapshot(&bundle.store);
    let refused = bundle.grant_generic(
        &owner,
        "refused",
        &agent.verifying_key(),
        &[GenericGrantRequest::ethos(
            Verb::Delete,
            Zone::Self_,
            GrantSelector::Tag {
                dir: "projects".into(),
                tag: "toto".into(),
            },
        )],
        "2026-07-18T12:03:00Z",
        "2026-07-25T12:03:00Z",
        0,
        "2026-07-18T12:03:00Z",
        &mut entropy,
    );
    assert!(refused.is_err());
    assert_eq!(snapshot(&bundle.store), before_refusal);

    let requests = vec![
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Public,
            GrantSelector::Id("projects/note".into()),
        ),
        GenericGrantRequest::ethos(Verb::Read, Zone::Circle, GrantSelector::Zone),
        GenericGrantRequest::ethos(Verb::Read, Zone::Self_, GrantSelector::Zone),
        GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Circle,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Circle,
            GrantSelector::Tag {
                dir: String::new(),
                tag: "toto".into(),
            },
        ),
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Circle,
            GrantSelector::Tag {
                dir: "projects".into(),
                tag: "toto".into(),
            },
        ),
        GenericGrantRequest::ethos(
            Verb::Edit,
            Zone::Self_,
            GrantSelector::Id("projects/note".into()),
        ),
        GenericGrantRequest::act("mail", "send"),
        GenericGrantRequest::act("mail", "config"),
    ];
    let outcome = bundle
        .grant_generic(
            &owner,
            "agent",
            &agent.verifying_key(),
            &requests,
            "2026-07-18T12:03:00Z",
            "2026-07-25T12:03:00Z",
            0,
            "2026-07-18T12:03:00Z",
            &mut entropy,
        )
        .expect("issue atomic generic grant");
    assert_eq!(
        outcome
            .deliveries
            .iter()
            .map(|delivery| delivery.kind)
            .collect::<Vec<_>>(),
        vec![
            GrantLineKind::None,
            GrantLineKind::ZoneRoot,
            GrantLineKind::ZoneRoot,
            GrantLineKind::Folder,
            GrantLineKind::ZoneTagView,
            GrantLineKind::FolderTagView,
            GrantLineKind::Section,
            GrantLineKind::None,
            GrantLineKind::ConnectorVault,
        ]
    );
    assert!(bundle
        .store
        .get("e/x/mail/header.json")
        .expect("read connector header")
        .is_some());
    let grant_entries = bundle
        .gamma_entries()
        .expect("read grant Gamma")
        .into_iter()
        .filter(|entry| entry.target.as_deref() == Some(&outcome.mandate.id))
        .count();
    assert_eq!(grant_entries, 1);
    let chain = vec![outcome.mandate];
    drop(bundle);

    let reopened = Bundle::open(FsStore::new(root.path())).expect("reopen generic grant bundle");
    reopened.verify().expect("generic grant edition verifies");
    reopened
        .gamma_verify()
        .expect("generic grant Gamma verifies");
    assert_eq!(
        reopened
            .read_section_as_agent(
                &chain,
                &agent,
                Zone::Circle,
                "projects/note",
                "2026-07-18T12:04:00Z",
            )
            .expect("use delivered circle line"),
        "granted body"
    );
}

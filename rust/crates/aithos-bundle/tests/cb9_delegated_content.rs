//! CB9 delegated content parity, independent authority/physics fences and cold replay.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aithos_bundle::bundle::{
    Bundle, GranteeContentOperation, GranteeContentOutcome, GranteeTarget, SectionSpec, ZoneIndex,
};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::{GenericGrantRequest, GrantSelector};
use aithos_bundle::log::LogFilter;
use aithos_bundle::{FsStore, Store};
use aithos_core::ids::Sid;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use aithos_core::mandate::{GammaQuery, Mandate, MandateSpec, PerimeterEntry, Verb};
use aithos_core::path::Zone;
use ed25519_dalek::SigningKey;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let base = option_env!("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&base).expect("create CB9 test base");
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-cb9-delegated-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create CB9 root {path:?}: {error}"),
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
    OwnerKeys::genesis(&MasterSeed::from_slice(&[0x59; 32]).expect("valid CB9 owner seed"))
}

fn snapshot(store: &impl Store) -> BTreeMap<String, Vec<u8>> {
    store
        .list("")
        .expect("list CB9 store")
        .into_iter()
        .map(|path| {
            let bytes = store
                .get(&path)
                .expect("read CB9 object")
                .expect("listed CB9 object exists");
            (path, bytes)
        })
        .collect()
}

fn copy_to_fresh_store(source: &impl Store, destination: &mut impl Store) {
    for path in source.list("").expect("list cold export") {
        let bytes = source
            .get(&path)
            .expect("read cold export")
            .expect("export object exists");
        destination.put(&path, &bytes).expect("import cold object");
    }
}

fn fixture(label: &str) -> (TempRoot, Bundle<FsStore>, OwnerKeys, SigningKey, SeqEntropy) {
    let root = TempRoot::new(label);
    let owner = owner();
    let agent = SigningKey::from_bytes(&[0x72; 32]);
    let succession = succession_from_entropy([0x69; 32]);
    let mut entropy = SeqEntropy::default();
    let mut bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T13:00:00Z",
    )
    .expect("initialize CB9 bundle");
    bundle
        .transaction(|bundle| {
            for zone in [Zone::Public, Zone::Circle, Zone::Self_] {
                for name in ["note", "note2"] {
                    bundle.section_add(
                        &SectionSpec {
                            zone,
                            folder_path: "projects",
                            name,
                            title: "existing",
                            tags: &["toto".to_owned()],
                            body: "before",
                            now: "2026-07-18T13:01:00Z",
                        },
                        &owner,
                        &mut entropy,
                    )?;
                }
            }
            bundle.publish(&owner, "2026-07-18T13:02:00Z")
        })
        .expect("publish CB9 target sections");
    (root, bundle, owner, agent, entropy)
}

fn gamma_delta(
    bundle: &mut Bundle<FsStore>,
    operation: impl FnOnce(&mut Bundle<FsStore>) -> aithos_core::Result<GranteeContentOutcome>,
) -> GranteeContentOutcome {
    let before = bundle.gamma_entries().expect("Gamma before").len();
    let outcome = operation(bundle).expect("accepted CB9 operation");
    assert_eq!(
        bundle.gamma_entries().expect("Gamma after").len(),
        before + 1,
        "every accepted grantee content operation is journalized once"
    );
    outcome
}

#[test]
fn cb9_delegated_operations_cover_all_zones_and_survive_fresh_store_replay() {
    let (_source_root, mut bundle, owner, agent, mut entropy) = fixture("parity");
    let preallocated = Sid::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid preallocated SID");
    let requests = vec![
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Public,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Public,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Write,
            Zone::Public,
            GrantSelector::Id("projects/note".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Circle,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Circle,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Write,
            Zone::Circle,
            GrantSelector::Id("projects/note".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Read,
            Zone::Self_,
            GrantSelector::Dir("projects".into()),
        ),
        GenericGrantRequest::ethos(
            Verb::Write,
            Zone::Self_,
            GrantSelector::Id("projects/note".into()),
        ),
        GenericGrantRequest::ethos(Verb::Append, Zone::Self_, GrantSelector::Zone),
        GenericGrantRequest::ethos(
            Verb::Append,
            Zone::Self_,
            GrantSelector::OpaqueId(preallocated),
        ),
        GenericGrantRequest::gamma(GammaQuery::default()),
    ];
    let grant = bundle
        .grant_generic(
            &owner,
            "cb9-agent",
            &agent.verifying_key(),
            &requests,
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .expect("issue CB9 parity grant");
    let chain = vec![grant.mandate];
    let perimeter = chain[0].parsed_perimeter().expect("parse CB9 perimeter");
    let self_dir = perimeter
        .iter()
        .find_map(|entry| match entry {
            PerimeterEntry::Ethos {
                verb: Verb::Read,
                zone: Zone::Self_,
                dir,
                tag: None,
            } if !dir.is_empty() => Some(dir.clone()),
            _ => None,
        })
        .expect("self folder perimeter");
    let self_note = perimeter
        .iter()
        .find_map(|entry| match entry {
            PerimeterEntry::EthosId {
                verb: Verb::Write,
                zone: Zone::Self_,
                id,
            } => Some(*id),
            _ => None,
        })
        .expect("self exact perimeter");

    for zone in [Zone::Public, Zone::Circle] {
        let listed = gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                zone,
                GranteeContentOperation::List {
                    target: GranteeTarget::Display("projects"),
                    now: "2026-07-18T13:10:00Z",
                },
                &mut entropy,
            )
        });
        assert!(matches!(
            listed,
            GranteeContentOutcome::Listed(ref entries)
                if entries.iter().any(|entry| entry.path == "note")
        ));
        assert_eq!(
            gamma_delta(&mut bundle, |bundle| {
                bundle.grantee_content_operation(
                    &chain,
                    &agent,
                    zone,
                    GranteeContentOperation::Read {
                        target: GranteeTarget::Display("projects/note"),
                        now: "2026-07-18T13:10:00Z",
                    },
                    &mut entropy,
                )
            }),
            GranteeContentOutcome::Read("before".into())
        );
        assert!(matches!(
            gamma_delta(&mut bundle, |bundle| {
                bundle.grantee_content_operation(
                    &chain,
                    &agent,
                    zone,
                    GranteeContentOperation::Create {
                        folder: GranteeTarget::Display("projects"),
                        preallocated_sid: None,
                        name: "fresh",
                        title: "created",
                        tags: &[],
                        body: "created by grantee",
                        now: "2026-07-18T13:10:00Z",
                    },
                    &mut entropy,
                )
            }),
            GranteeContentOutcome::Created(_)
        ));
        assert_eq!(
            gamma_delta(&mut bundle, |bundle| {
                bundle.grantee_content_operation(
                    &chain,
                    &agent,
                    zone,
                    GranteeContentOperation::Edit {
                        target: GranteeTarget::Display("projects/note"),
                        body: "edited by grantee",
                        now: "2026-07-18T13:10:00Z",
                    },
                    &mut entropy,
                )
            }),
            GranteeContentOutcome::Mutated
        );
        assert_eq!(
            gamma_delta(&mut bundle, |bundle| {
                bundle.grantee_content_operation(
                    &chain,
                    &agent,
                    zone,
                    GranteeContentOperation::Delete {
                        target: GranteeTarget::Display("projects/note"),
                        now: "2026-07-18T13:10:00Z",
                    },
                    &mut entropy,
                )
            }),
            GranteeContentOutcome::Mutated
        );
    }

    let listed = gamma_delta(&mut bundle, |bundle| {
        bundle.grantee_content_operation(
            &chain,
            &agent,
            Zone::Self_,
            GranteeContentOperation::List {
                target: GranteeTarget::FolderIds(&self_dir),
                now: "2026-07-18T13:11:00Z",
            },
            &mut entropy,
        )
    });
    assert!(matches!(
        listed,
        GranteeContentOutcome::Listed(ref entries)
            if entries.iter().any(|entry| entry.path == "note")
    ));
    assert_eq!(
        gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                Zone::Self_,
                GranteeContentOperation::Read {
                    target: GranteeTarget::Id(self_note),
                    now: "2026-07-18T13:11:00Z",
                },
                &mut entropy,
            )
        }),
        GranteeContentOutcome::Read("before".into())
    );
    assert!(matches!(
        gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                Zone::Self_,
                GranteeContentOperation::Create {
                    folder: GranteeTarget::FolderIds(&[]),
                    preallocated_sid: None,
                    name: "fresh",
                    title: "created",
                    tags: &[],
                    body: "fresh self",
                    now: "2026-07-18T13:11:00Z",
                },
                &mut entropy,
            )
        }),
        GranteeContentOutcome::Created(_)
    ));
    assert_eq!(
        gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                Zone::Self_,
                GranteeContentOperation::Create {
                    folder: GranteeTarget::FolderIds(&[]),
                    preallocated_sid: Some(preallocated),
                    name: "preallocated",
                    title: "created",
                    tags: &[],
                    body: "preallocated self",
                    now: "2026-07-18T13:11:00Z",
                },
                &mut entropy,
            )
        }),
        GranteeContentOutcome::Created(preallocated)
    );
    assert_eq!(
        gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                Zone::Self_,
                GranteeContentOperation::Edit {
                    target: GranteeTarget::Id(self_note),
                    body: "edited self",
                    now: "2026-07-18T13:11:00Z",
                },
                &mut entropy,
            )
        }),
        GranteeContentOutcome::Mutated
    );
    assert_eq!(
        gamma_delta(&mut bundle, |bundle| {
            bundle.grantee_content_operation(
                &chain,
                &agent,
                Zone::Self_,
                GranteeContentOperation::Delete {
                    target: GranteeTarget::Id(self_note),
                    now: "2026-07-18T13:11:00Z",
                },
                &mut entropy,
            )
        }),
        GranteeContentOutcome::Mutated
    );

    let delegated = bundle
        .gamma_entries()
        .expect("delegated Gamma")
        .into_iter()
        .filter(|entry| entry.authorized_via.is_some())
        .collect::<Vec<_>>();
    assert!(delegated.len() >= 16);
    assert!(delegated.iter().all(|entry| {
        entry.authorized_via.as_ref() == Some(&vec![chain[0].id.clone()])
            && entry.authorized_by.as_deref() == Some(chain[0].id.as_str())
            && entry.signature.key == chain[0].grantee.pubkey
    }));

    let cold_root = TempRoot::new("cold");
    let mut cold_store = FsStore::new(cold_root.path());
    copy_to_fresh_store(&bundle.store, &mut cold_store);
    drop(bundle);
    let mut cold = Bundle::open(cold_store).expect("open fresh CB9 store");
    cold.gamma_verify().expect("cold replay CB9 Gamma");
    assert_eq!(
        cold.read_section_as_agent(
            &chain,
            &agent,
            Zone::Circle,
            "projects/fresh",
            "2026-07-18T13:15:00Z",
        )
        .expect("cold read created circle section"),
        "created by grantee"
    );
    assert!(!cold
        .log_query_as_agent(
            &chain,
            &agent,
            &GammaQuery::default(),
            &LogFilter::default(),
            "2026-07-18T13:15:00Z",
        )
        .expect("cold authorized Gamma read")
        .is_empty());
    assert_eq!(
        cold.grantee_content_operation(
            &chain,
            &agent,
            Zone::Self_,
            GranteeContentOperation::Read {
                target: GranteeTarget::Id(preallocated),
                now: "2026-07-18T13:15:00Z",
            },
            &mut entropy,
        )
        .expect("cold exact self read"),
        GranteeContentOutcome::Read("preallocated self".into())
    );
}

fn exact_root_mandate(
    owner: &OwnerKeys,
    did: &str,
    agent: &SigningKey,
    id: &str,
    sid: Sid,
    constraints: serde_json::Value,
    not_after: &str,
) -> Mandate {
    Mandate::build_root(
        &owner.root_sign,
        &MandateSpec {
            id: id.into(),
            subject: did.into(),
            constraints,
            grantee_id: "urn:aithos:agent:cb9-fence".into(),
            grantee_label: "cb9-fence".into(),
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![PerimeterEntry::EthosId {
                verb: Verb::Read,
                zone: Zone::Circle,
                id: sid,
            }],
            not_before: "2026-07-18T13:00:00Z".into(),
            not_after: not_after.into(),
            issued_at: "2026-07-18T13:00:00Z".into(),
            nonce: "cb9-fence".into(),
        },
    )
    .expect("build exact CB9 mandate")
}

#[test]
fn cb9_every_authority_or_physics_refusal_is_byte_exact() {
    let (_root, mut bundle, owner, agent, mut entropy) = fixture("fences");
    let note = bundle
        .resolve_clear(Zone::Circle, "projects/note")
        .expect("resolve note")
        .0;
    let note2 = bundle
        .resolve_clear(Zone::Circle, "projects/note2")
        .expect("resolve note2")
        .0;
    let note_sid = Sid::parse(&note.sid).expect("note SID");
    let note2_sid = Sid::parse(&note2.sid).expect("note2 SID");
    let exact = bundle
        .grant_generic(
            &owner,
            "line-holder",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Id("projects/note".into()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .expect("grant exact line");
    let exact_chain = vec![exact.mandate];

    let before = snapshot(&bundle.store);
    assert!(bundle
        .grantee_content_operation(
            &[],
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .is_err());
    assert_eq!(snapshot(&bundle.store), before);

    let second_agent = SigningKey::from_bytes(&[0x73; 32]);
    let no_line = vec![exact_root_mandate(
        &owner,
        &bundle.did,
        &second_agent,
        "mandate_01ARZ3NDEKTSV4RRFFQ69G5FAA",
        note_sid,
        serde_json::json!({}),
        "2026-07-25T13:03:00Z",
    )];
    let before = snapshot(&bundle.store);
    assert!(bundle
        .grantee_content_operation(
            &no_line,
            &second_agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .is_err());
    assert_eq!(snapshot(&bundle.store), before);

    let sibling_line = vec![exact_root_mandate(
        &owner,
        &bundle.did,
        &agent,
        "mandate_01ARZ3NDEKTSV4RRFFQ69G5FAB",
        note2_sid,
        serde_json::json!({}),
        "2026-07-25T13:03:00Z",
    )];
    let before = snapshot(&bundle.store);
    assert!(bundle
        .grantee_content_operation(
            &sibling_line,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note2"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .is_err());
    assert_eq!(snapshot(&bundle.store), before);

    let invalid_constraint = vec![exact_root_mandate(
        &owner,
        &bundle.did,
        &agent,
        "mandate_01ARZ3NDEKTSV4RRFFQ69G5FAC",
        note_sid,
        serde_json::json!({"unknown_cb9_constraint": true}),
        "2026-07-25T13:03:00Z",
    )];
    let before = snapshot(&bundle.store);
    assert!(bundle
        .grantee_content_operation(
            &invalid_constraint,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .is_err());
    assert_eq!(snapshot(&bundle.store), before);

    bundle
        .log_revoke_owner(
            &owner,
            &exact_chain[0].id,
            "CB9 current authority cut",
            "2026-07-18T13:05:00Z",
            &mut entropy,
        )
        .expect("revoke exact chain");
    let before = snapshot(&bundle.store);
    assert!(bundle
        .grantee_content_operation(
            &exact_chain,
            &agent,
            Zone::Circle,
            GranteeContentOperation::Read {
                target: GranteeTarget::Display("projects/note"),
                now: "2026-07-18T13:06:00Z",
            },
            &mut entropy,
        )
        .is_err());
    assert_eq!(snapshot(&bundle.store), before);
}

#[test]
fn cb9_public_authorship_is_grantee_signed_and_committed_after_publication() {
    let (_root, mut bundle, owner, agent, mut entropy) = fixture("authorship");
    let grant = bundle
        .grant_generic(
            &owner,
            "public-author",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Edit,
                Zone::Public,
                GrantSelector::Id("projects/note".into()),
            )],
            "2026-07-18T13:03:00Z",
            "2026-07-25T13:03:00Z",
            0,
            "2026-07-18T13:03:00Z",
            &mut entropy,
        )
        .expect("grant public edit");
    let chain = vec![grant.mandate];
    bundle
        .grantee_content_operation(
            &chain,
            &agent,
            Zone::Public,
            GranteeContentOperation::Edit {
                target: GranteeTarget::Display("projects/note"),
                body: "public grantee authorship",
                now: "2026-07-18T13:04:00Z",
            },
            &mut entropy,
        )
        .expect("delegated public edit");
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T13:05:00Z"))
        .expect("publish future CB9 edition");
    bundle.verify().expect("published CB9 bundle verifies");
    bundle
        .verify_public_authorship()
        .expect("public grantee authorship verifies");
    let index: ZoneIndex = serde_json::from_slice(
        &bundle
            .store
            .get("e/public/index.json")
            .expect("read public index")
            .expect("public index exists"),
    )
    .expect("decode public index");
    let row = index
        .sections
        .iter()
        .find(|row| row.name == "note")
        .expect("edited public row");
    assert!(
        row.sig.is_none(),
        "grantee must never imitate owner #content"
    );
    let authorship = row.authorship.as_ref().expect("delegated authorship");
    assert_eq!(authorship.authorized_via, vec![chain[0].id.clone()]);
    assert_eq!(authorship.key, chain[0].grantee.pubkey);
    let value = serde_json::to_value(authorship).expect("authorship JSON");
    for member in [
        "subject",
        "zone",
        "sid",
        "content_hash",
        "operation_ref",
        "edition",
        "authorized_via",
        "key",
        "sig",
    ] {
        assert!(value.get(member).is_some(), "missing {member}");
    }
}

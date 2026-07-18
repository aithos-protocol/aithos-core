//! CB10 durable structure, revocation and connector-vault integration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use aithos_bundle::bundle::{Bundle, SectionSpec, ZoneIndex};
use aithos_bundle::entropy::SeqEntropy;
use aithos_bundle::grants::{GenericGrantRequest, GrantSelector};
use aithos_bundle::structure::{StructuralOperation, StructuralOutcome};
use aithos_bundle::vault::{VaultConfigOperation, VaultConfigOutcome};
use aithos_bundle::{FsStore, Store};
use aithos_core::header::Header;
use aithos_core::keys::{grantee_kex_secret, succession_from_entropy, MasterSeed, OwnerKeys};
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
        std::fs::create_dir_all(&base).expect("create CB10 test base");
        loop {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "aithos-cb10-{}-{label}-{serial}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create CB10 root {path:?}: {error}"),
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
    OwnerKeys::genesis(&MasterSeed::from_slice(&[0x5a; 32]).expect("valid CB10 owner seed"))
}

fn snapshot(store: &impl Store) -> BTreeMap<String, Vec<u8>> {
    store
        .list("")
        .expect("list CB10 store")
        .into_iter()
        .map(|path| {
            let bytes = store
                .get(&path)
                .expect("read CB10 object")
                .expect("listed object exists");
            (path, bytes)
        })
        .collect()
}

fn copy_store(source: &impl Store, destination: &mut impl Store) {
    for path in source.list("").expect("list CB10 export") {
        let bytes = source
            .get(&path)
            .expect("read CB10 export")
            .expect("export object exists");
        destination.put(&path, &bytes).expect("copy CB10 object");
    }
}

fn init(label: &str) -> (TempRoot, Bundle<FsStore>, OwnerKeys, SeqEntropy) {
    let root = TempRoot::new(label);
    let owner = owner();
    let succession = succession_from_entropy([0x6a; 32]);
    let mut entropy = SeqEntropy::default();
    let bundle = Bundle::init(
        FsStore::new(root.path()),
        &owner,
        &succession.verifying_key(),
        &mut entropy,
        "2026-07-18T14:00:00Z",
    )
    .expect("initialize CB10 bundle");
    (root, bundle, owner, entropy)
}

#[test]
fn cb10_structural_operations_compose_authority_and_rollback_refusals() {
    let (_root, mut bundle, owner, mut entropy) = init("structure");
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Public,
                    folder_path: "projects",
                    name: "note",
                    title: "old title",
                    tags: &["old".to_owned()],
                    body: "public body",
                    now: "2026-07-18T14:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.ensure_folder(Zone::Public, "projects/moving", &owner, &mut entropy)?;
            bundle.ensure_folder(Zone::Public, "archive", &owner, &mut entropy)?;
            bundle.publish(&owner, "2026-07-18T14:02:00Z")
        })
        .expect("publish structural fixture");

    let agent = SigningKey::from_bytes(&[0x71; 32]);
    let grant = bundle
        .grant_generic(
            &owner,
            "structure-agent",
            &agent.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Public,
                GrantSelector::Zone,
            )],
            "2026-07-18T14:03:00Z",
            "2026-07-25T14:03:00Z",
            0,
            "2026-07-18T14:03:00Z",
            &mut entropy,
        )
        .expect("grant structural authority");
    let chain = vec![grant.mandate];

    let listed = bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::ListFolder {
                zone: Zone::Public,
                folder: "projects",
                now: "2026-07-18T14:04:00Z",
            },
            &mut entropy,
        )
        .expect("list covered folder");
    assert!(matches!(listed, StructuralOutcome::Listed(entries) if !entries.is_empty()));

    let created = bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::CreateFolder {
                zone: Zone::Public,
                parent: "projects",
                name: "child",
                now: "2026-07-18T14:05:00Z",
            },
            &mut entropy,
        )
        .expect("create child folder");
    assert!(matches!(created, StructuralOutcome::Created(_)));
    bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::RenameFolder {
                zone: Zone::Public,
                folder: "projects/child",
                new_name: "renamed",
                now: "2026-07-18T14:06:00Z",
            },
            &mut entropy,
        )
        .expect("rename folder");
    bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::EditSectionMetadata {
                zone: Zone::Public,
                section: "projects/note",
                name: Some("memo"),
                title: Some("new title"),
                tags: Some(&["new".to_owned(), "pinned".to_owned()]),
                now: "2026-07-18T14:07:00Z",
            },
            &mut entropy,
        )
        .expect("edit section metadata");
    bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Public,
                folder: "projects/moving",
                destination_parent: "archive",
                now: "2026-07-18T14:08:00Z",
            },
            &mut entropy,
        )
        .expect("move public folder under composed source/destination authority");
    bundle
        .structural_operation(
            &chain,
            &agent,
            StructuralOperation::DeleteFolder {
                zone: Zone::Public,
                folder: "projects/renamed",
                recursive: false,
                now: "2026-07-18T14:09:00Z",
            },
            &mut entropy,
        )
        .expect("delete empty folder");

    let before = snapshot(&bundle.store);
    let refused = bundle.structural_operation(
        &chain,
        &agent,
        StructuralOperation::MoveFolder {
            zone: Zone::Public,
            folder: "archive",
            destination_parent: "archive/moving",
            now: "2026-07-18T14:10:00Z",
        },
        &mut entropy,
    );
    assert!(refused.is_err());
    assert_eq!(snapshot(&bundle.store), before);

    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T14:11:00Z"))
        .expect("publish structural effects");
    bundle.verify().expect("structural edition verifies");
    bundle.gamma_verify().expect("structural Gamma replays");
    let index: ZoneIndex = serde_json::from_slice(
        &bundle
            .store
            .get("e/public/index.json")
            .expect("read public index")
            .expect("public index exists"),
    )
    .expect("parse public index");
    let memo = index
        .sections
        .iter()
        .find(|row| row.name == "memo")
        .expect("renamed metadata row");
    assert_eq!(memo.title, "new title");
    assert_eq!(memo.tags, ["new", "pinned"]);
    assert!(bundle
        .clear_zone_tree(Zone::Public)
        .expect("read public tree")
        .contains(&"archive/moving".to_owned()));
}

#[test]
fn cb10_circle_tag_views_and_delegated_move_follow_the_atomic_key_boundary() {
    let (_root, mut bundle, owner, mut entropy) = init("circle-structure");
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "tagged",
                    title: "tagged",
                    tags: &["old".to_owned()],
                    body: "tag body",
                    now: "2026-07-18T14:20:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "source/moving",
                    name: "note",
                    title: "moved",
                    tags: &[],
                    body: "moved body",
                    now: "2026-07-18T14:20:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.ensure_folder(Zone::Circle, "destination", &owner, &mut entropy)?;
            bundle.publish(&owner, "2026-07-18T14:21:00Z")
        })
        .expect("publish circle structural fixture");

    let editor = SigningKey::from_bytes(&[0x77; 32]);
    let tag_reader = SigningKey::from_bytes(&[0x78; 32]);
    let move_agent = SigningKey::from_bytes(&[0x79; 32]);
    let destination_reader = SigningKey::from_bytes(&[0x7a; 32]);
    let tag_editor_grant = bundle
        .grant_generic(
            &owner,
            "tag-editor",
            &editor.verifying_key(),
            &[
                GenericGrantRequest::ethos(
                    Verb::Edit,
                    Zone::Circle,
                    GrantSelector::Dir("projects".into()),
                ),
                GenericGrantRequest::ethos(
                    Verb::Read,
                    Zone::Circle,
                    GrantSelector::Tag {
                        dir: "projects".into(),
                        tag: "new".into(),
                    },
                ),
            ],
            "2026-07-18T14:22:00Z",
            "2026-07-25T14:22:00Z",
            0,
            "2026-07-18T14:22:00Z",
            &mut entropy,
        )
        .expect("grant editor and new tag-view key");
    let tag_reader_grant = bundle
        .grant_generic(
            &owner,
            "tag-reader",
            &tag_reader.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Tag {
                    dir: "projects".into(),
                    tag: "new".into(),
                },
            )],
            "2026-07-18T14:23:00Z",
            "2026-07-25T14:23:00Z",
            0,
            "2026-07-18T14:23:00Z",
            &mut entropy,
        )
        .expect("grant future tag reader");
    bundle
        .structural_operation(
            &[tag_editor_grant.mandate],
            &editor,
            StructuralOperation::EditSectionMetadata {
                zone: Zone::Circle,
                section: "projects/tagged",
                name: None,
                title: Some("retagged"),
                tags: Some(&["new".to_owned()]),
                now: "2026-07-18T14:24:00Z",
            },
            &mut entropy,
        )
        .expect("retag and derive the new tag wrap atomically");
    assert_eq!(
        bundle
            .read_section_as_agent(
                &[tag_reader_grant.mandate],
                &tag_reader,
                Zone::Circle,
                "projects/tagged",
                "2026-07-18T14:25:00Z",
            )
            .expect("read through newly derived tag wrap"),
        "tag body"
    );

    let move_grant = bundle
        .grant_generic(
            &owner,
            "move-agent",
            &move_agent.verifying_key(),
            &[
                GenericGrantRequest::ethos(
                    Verb::Edit,
                    Zone::Circle,
                    GrantSelector::Dir("source/moving".into()),
                ),
                GenericGrantRequest::ethos(
                    Verb::Append,
                    Zone::Circle,
                    GrantSelector::Dir("destination".into()),
                ),
            ],
            "2026-07-18T14:26:00Z",
            "2026-07-25T14:26:00Z",
            0,
            "2026-07-18T14:26:00Z",
            &mut entropy,
        )
        .expect("grant composed circle move authority and keys");
    let destination_grant = bundle
        .grant_generic(
            &owner,
            "destination-reader",
            &destination_reader.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("destination".into()),
            )],
            "2026-07-18T14:27:00Z",
            "2026-07-25T14:27:00Z",
            0,
            "2026-07-18T14:27:00Z",
            &mut entropy,
        )
        .expect("grant destination holder");
    bundle
        .structural_operation(
            &[move_grant.mandate],
            &move_agent,
            StructuralOperation::MoveFolder {
                zone: Zone::Circle,
                folder: "source/moving",
                destination_parent: "destination",
                now: "2026-07-18T14:28:00Z",
            },
            &mut entropy,
        )
        .expect("move circle folder with source/destination keys");
    assert_eq!(
        bundle
            .read_section_as_agent(
                &[destination_grant.mandate],
                &destination_reader,
                Zone::Circle,
                "destination/moving/note",
                "2026-07-18T14:29:00Z",
            )
            .expect("destination holder follows the new up-link wrap"),
        "moved body"
    );
    bundle
        .transaction(|bundle| bundle.publish(&owner, "2026-07-18T14:30:00Z"))
        .expect("publish circle structural changes");
    bundle.verify().expect("circle structural edition verifies");
    bundle
        .gamma_verify()
        .expect("circle structural Gamma replays");
}

#[test]
fn cb10_revocation_rotation_publication_is_one_durable_cut() {
    let (root, mut bundle, owner, mut entropy) = init("revocation");
    bundle
        .transaction(|bundle| {
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::Circle,
                    folder_path: "projects",
                    name: "note",
                    title: "protected",
                    tags: &[],
                    body: "protected body",
                    now: "2026-07-18T15:01:00Z",
                },
                &owner,
                &mut entropy,
            )?;
            bundle.publish(&owner, "2026-07-18T15:02:00Z")
        })
        .expect("publish protected subtree");
    let revoked = SigningKey::from_bytes(&[0x72; 32]);
    let survivor = SigningKey::from_bytes(&[0x73; 32]);
    let revoked_grant = bundle
        .grant_generic(
            &owner,
            "revoked",
            &revoked.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Write,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T15:03:00Z",
            "2026-07-25T15:03:00Z",
            0,
            "2026-07-18T15:03:00Z",
            &mut entropy,
        )
        .expect("grant revoked line");
    let survivor_grant = bundle
        .grant_generic(
            &owner,
            "survivor",
            &survivor.verifying_key(),
            &[GenericGrantRequest::ethos(
                Verb::Read,
                Zone::Circle,
                GrantSelector::Dir("projects".into()),
            )],
            "2026-07-18T15:04:00Z",
            "2026-07-25T15:04:00Z",
            0,
            "2026-07-18T15:04:00Z",
            &mut entropy,
        )
        .expect("grant survivor line");
    let revoked_id = revoked_grant.mandate.id.clone();
    bundle
        .revoke_transaction(
            &owner,
            &revoked_id,
            "projects",
            "compromise",
            "2026-07-18T15:05:00Z",
            &mut entropy,
        )
        .expect("commit complete incident cut");

    let header_path = bundle
        .store
        .list("e/circle/hdr/")
        .expect("list circle headers")
        .into_iter()
        .find(|path| {
            let bytes = bundle
                .store
                .get(path)
                .expect("read header")
                .expect("header exists");
            serde_json::from_slice::<Header>(&bytes).is_ok_and(|header| header.node.contains("/d/"))
        })
        .expect("projects header path");
    let header: Header = serde_json::from_slice(
        &bundle
            .store
            .get(&header_path)
            .expect("read rotated header")
            .expect("rotated header exists"),
    )
    .expect("parse rotated header");
    let revoked_kex = grantee_kex_secret(&revoked);
    assert!(header
        .open_latest(
            &bundle.did,
            &revoked_grant.mandate.grantee.pubkey,
            &revoked_kex,
        )
        .is_err());
    let survivor_kex = grantee_kex_secret(&survivor);
    assert!(header
        .open_latest(
            &bundle.did,
            &survivor_grant.mandate.grantee.pubkey,
            &survivor_kex,
        )
        .is_ok());
    bundle.verify().expect("revocation edition verifies");
    bundle.gamma_verify().expect("revocation Gamma replays");
    drop(bundle);

    let reopened =
        Bundle::open(FsStore::new(root.path())).expect("reopen committed revocation bundle");
    reopened.verify().expect("reopened revocation verifies");
    reopened
        .gamma_verify()
        .expect("reopened revocation Gamma verifies");
    assert_eq!(
        reopened
            .read_section_as_agent(
                &[survivor_grant.mandate],
                &survivor,
                Zone::Circle,
                "projects/note",
                "2026-07-18T15:06:00Z",
            )
            .expect("survivor reads rewritten body"),
        "protected body"
    );
    assert!(reopened
        .read_section_as_agent(
            &[revoked_grant.mandate],
            &revoked,
            Zone::Circle,
            "projects/note",
            "2026-07-18T15:06:00Z",
        )
        .is_err());

    let cold_root = TempRoot::new("revocation-cold");
    let mut cold_store = FsStore::new(cold_root.path());
    copy_store(&reopened.store, &mut cold_store);
    drop(reopened);
    let cold = Bundle::open(cold_store).expect("open fresh revocation store");
    cold.verify().expect("fresh revocation store verifies");
    cold.gamma_verify()
        .expect("fresh revocation authority replays");
}

#[test]
fn cb10_exact_connector_vault_is_isolated_secret_free_and_cold_verifiable() {
    let (_root, mut bundle, owner, mut entropy) = init("vault");
    let config_agent = SigningKey::from_bytes(&[0x74; 32]);
    let calendar_agent = SigningKey::from_bytes(&[0x75; 32]);
    let audit_agent = SigningKey::from_bytes(&[0x76; 32]);
    let config_grant = bundle
        .grant_generic(
            &owner,
            "mail-config",
            &config_agent.verifying_key(),
            &[
                GenericGrantRequest::act("mail", "config"),
                GenericGrantRequest::act("mail", "send"),
            ],
            "2026-07-18T16:01:00Z",
            "2026-07-25T16:01:00Z",
            0,
            "2026-07-18T16:01:00Z",
            &mut entropy,
        )
        .expect("grant exact mail config");
    let calendar_grant = bundle
        .grant_generic(
            &owner,
            "calendar-config",
            &calendar_agent.verifying_key(),
            &[GenericGrantRequest::act("calendar", "config")],
            "2026-07-18T16:02:00Z",
            "2026-07-25T16:02:00Z",
            0,
            "2026-07-18T16:02:00Z",
            &mut entropy,
        )
        .expect("grant exact calendar config");
    bundle
        .grant_audit_line(&owner, &audit_agent.verifying_key(), &mut entropy)
        .expect("grant historical audit line");
    let config_chain = vec![config_grant.mandate.clone()];
    let calendar_chain = vec![calendar_grant.mandate.clone()];

    bundle
        .vault_config_operation(
            &config_chain,
            &config_agent,
            "mail",
            VaultConfigOperation::Create {
                config: b"mail-secret-v1",
                now: "2026-07-18T16:03:00Z",
            },
            &mut entropy,
        )
        .expect("create mail config");
    bundle
        .vault_config_operation(
            &calendar_chain,
            &calendar_agent,
            "calendar",
            VaultConfigOperation::Create {
                config: b"calendar-secret",
                now: "2026-07-18T16:04:00Z",
            },
            &mut entropy,
        )
        .expect("create calendar config");
    let read = bundle
        .vault_config_operation(
            &config_chain,
            &config_agent,
            "mail",
            VaultConfigOperation::Read {
                now: "2026-07-18T16:05:00Z",
            },
            &mut entropy,
        )
        .expect("read exact mail config");
    assert_eq!(read, VaultConfigOutcome::Read(b"mail-secret-v1".to_vec()));
    bundle
        .vault_config_operation(
            &config_chain,
            &config_agent,
            "mail",
            VaultConfigOperation::Edit {
                config: b"mail-secret-v2",
                now: "2026-07-18T16:06:00Z",
            },
            &mut entropy,
        )
        .expect("edit exact mail config");

    let before_refusal = snapshot(&bundle.store);
    assert!(bundle
        .open_vault_with_capability(
            &calendar_chain,
            &calendar_agent,
            "mail",
            "2026-07-18T16:07:00Z",
        )
        .is_err());
    assert!(bundle
        .open_vault_with_capability(&[], &config_agent, "mail", "2026-07-18T16:07:00Z",)
        .is_err());
    assert_eq!(snapshot(&bundle.store), before_refusal);

    let audit_only_grant = bundle
        .grant_generic(
            &owner,
            "audit-action",
            &audit_agent.verifying_key(),
            &[GenericGrantRequest::act("mail", "send")],
            "2026-07-18T16:07:00Z",
            "2026-07-25T16:07:00Z",
            0,
            "2026-07-18T16:07:00Z",
            &mut entropy,
        )
        .expect("grant action without config");
    assert!(bundle
        .open_vault_with_capability(
            &[audit_only_grant.mandate],
            &audit_agent,
            "mail",
            "2026-07-18T16:08:00Z",
        )
        .is_err());

    let calendar_header_before = bundle
        .store
        .get("e/x/calendar/header.json")
        .expect("read calendar header")
        .expect("calendar header exists");
    bundle
        .rotate_vault_connector(
            &owner,
            "mail",
            &config_grant.mandate.grantee.pubkey,
            "2026-07-18T16:09:00Z",
            &mut entropy,
        )
        .expect("rotate only mail config");
    assert_eq!(
        bundle
            .store
            .get("e/x/calendar/header.json")
            .expect("read unchanged calendar header")
            .expect("calendar header remains"),
        calendar_header_before
    );
    assert!(bundle
        .open_vault_with_capability(&config_chain, &config_agent, "mail", "2026-07-18T16:10:00Z",)
        .is_err());
    assert_eq!(
        bundle
            .read_vault_config_owner(&owner, "mail")
            .expect("owner reads rotated mail config"),
        b"mail-secret-v2"
    );
    bundle.verify().expect("vault publication verifies");
    bundle.gamma_verify().expect("vault Gamma replays");

    for bytes in snapshot(&bundle.store).values() {
        assert!(!bytes
            .windows(b"mail-secret-v1".len())
            .any(|window| window == b"mail-secret-v1"));
        assert!(!bytes
            .windows(b"mail-secret-v2".len())
            .any(|window| window == b"mail-secret-v2"));
        assert!(!bytes
            .windows(b"calendar-secret".len())
            .any(|window| window == b"calendar-secret"));
    }

    let cold_root = TempRoot::new("vault-cold");
    let mut cold_store = FsStore::new(cold_root.path());
    copy_store(&bundle.store, &mut cold_store);
    drop(bundle);
    let cold = Bundle::open(cold_store).expect("open fresh vault store");
    cold.verify().expect("fresh vault store verifies keylessly");
    cold.gamma_verify()
        .expect("fresh vault exact authority replays");
}

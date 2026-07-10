//! `aithos-core` — reference CLI (spec §09.1). Everything is local; no
//! command needs a network to be correct.

use aithos_core::did::DidDocument;
use aithos_core::keys::{succession_from_entropy, MasterSeed, OwnerKeys};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aithos-core", version, about = "Aithos Core reference CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate S, DID doc, empty bundle. (Scaffold: derives and prints the
    /// owner public keys; bundle writing lands with the bundle crate.)
    Init {
        /// DEV ONLY: fixed 32-byte seed as hex (deterministic, for vectors).
        /// Omit to generate a fresh random seed.
        #[arg(long)]
        seed_hex: Option<String>,
        /// DEV ONLY: fixed succession entropy as hex (deterministic).
        #[arg(long)]
        succession_seed_hex: Option<String>,
        /// Also create a bundle (spec 02.3) in this directory.
        #[arg(long)]
        dir: Option<String>,
    },
    /// Create a folder (mkdir -p) in a zone of the bundle.
    FolderAdd {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        zone: String,
        path: String,
    },
    /// Add a section. PATH is folder/…/name; body from --body.
    SectionAdd {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        zone: String,
        path: String,
        #[arg(long, default_value = "")]
        title: String,
        /// Comma-separated tags.
        #[arg(long, default_value = "")]
        tags: String,
        #[arg(long)]
        body: String,
    },
    /// Show a zone's display tree (owner-side for circle/self).
    ZoneShow {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        zone: String,
    },
    /// Read one section. Public needs NO key (omit --seed-hex).
    SectionRead {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: Option<String>,
        zone: String,
        path: String,
    },
    /// Publish a new edition (height+1), signed by root.
    EditionPublish {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
    },
    /// Verify the whole edition chain and pinned files. No keys needed.
    EditionVerify {
        #[arg(long)]
        dir: String,
    },
    /// Grant an agent a circle perimeter: mints the cert AND delivers keys.
    Grant {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        /// DEV: the agent's Ed25519 seed (its single keypair).
        #[arg(long)]
        agent_seed_hex: String,
        #[arg(long, default_value = "agent")]
        label: String,
        /// Display folder path in circle, e.g. projets/perso
        folder: String,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long, default_value_t = 7)]
        ttl_days: u32,
        #[arg(long, default_value_t = 0)]
        issue_depth: u32,
    },
    /// Verify a mandate chain (one cert file) at time T. No keys needed.
    MandateVerify {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        cert: String,
        #[arg(long)]
        at: String,
    },
    /// Read a circle section AS an agent, gated by its mandate (spec 04.5).
    SectionReadAgent {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        cert: String,
        #[arg(long)]
        agent_seed_hex: String,
        #[arg(long)]
        at: String,
        path: String,
    },
    /// DEV ONLY: derive a node key along a canonical sid-path (spec 02.5).
    /// Proves determinism by hand: same path, same key — every time.
    NodeKey {
        /// Canonical path, e.g. /e/circle/d/<sid>/s/<sid>
        path: String,
        /// The zone-root DK as hex (32 bytes).
        #[arg(long)]
        zone_dk_hex: String,
    },
    /// Seal a node key into a header (spec 03). DEV surface over test keys.
    HeaderSeal {
        #[arg(long)]
        node: String,
        #[arg(long)]
        subject_did: String,
        #[arg(long)]
        dk_hex: String,
        /// Repeatable: label:kid:x25519_pub_hex — one MUST be labelled "owner".
        #[arg(long = "recipient")]
        recipients: Vec<String>,
    },
    /// Open one's line in a header JSON (from --file) and print the node key.
    HeaderOpen {
        #[arg(long)]
        file: String,
        #[arg(long)]
        subject_did: String,
        #[arg(long)]
        kid: String,
        #[arg(long)]
        sk_hex: String,
        #[arg(long, default_value_t = 1)]
        version: u64,
    },
    /// Grant an agent CONNECTOR ACTION rights (certificate only — actions
    /// need no content keys). Counting constraints are enforced by gamma.
    GrantAct {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        #[arg(long)]
        agent_seed_hex: String,
        #[arg(long, default_value = "agent")]
        label: String,
        /// Connector name, e.g. gmail → perimeter act.x.gmail.<action>
        connector: String,
        /// Action pattern (an action name, or * for the read/act class).
        #[arg(default_value = "*")]
        action_pat: String,
        #[arg(long, default_value_t = 7)]
        ttl_days: u32,
        /// Lifetime action budget (spec 04.4, counted by gamma 07.4).
        #[arg(long)]
        max_actions: Option<u64>,
        /// Dead-man heartbeat, e.g. --heartbeat-every 30d --heartbeat-grace 72h
        #[arg(long)]
        heartbeat_every: Option<String>,
        #[arg(long)]
        heartbeat_grace: Option<String>,
    },
    /// Log a connector action under a mandate chain (leaf last). The gamma
    /// entry IS the authorization evidence — no entry, no action (I5).
    Action {
        #[arg(long)]
        dir: String,
        /// Certificate file(s), root first, leaf last.
        #[arg(long = "cert")]
        certs: Vec<String>,
        #[arg(long)]
        agent_seed_hex: String,
        connector: String,
        action: String,
        /// Free-form action arguments; only their hash enters the log.
        #[arg(long, default_value = "")]
        args: String,
    },
    /// Publish an owner liveness beacon (spec 07.5).
    Heartbeat {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        #[arg(long, default_value_t = 1)]
        seq: u64,
    },
    /// Print the log's counting skeleton (what any file-holder sees).
    LogShow {
        #[arg(long)]
        dir: String,
    },
    /// Verify the whole gamma chain and every entry signature. No keys needed.
    LogVerify {
        #[arg(long)]
        dir: String,
    },
    /// Owner search over the log (spec 07.8): every present filter narrows.
    LogQuery {
        #[arg(long)]
        dir: String,
        #[arg(long)]
        seed_hex: String,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        action: Option<String>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
        /// Display folder path in circle, e.g. projets/perso
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        mandate: Option<String>,
    },
}

use aithos_bundle::bundle::{Bundle, SectionSpec};
use aithos_bundle::entropy::OsEntropy;
use aithos_bundle::FsStore;
use aithos_core::path::Zone;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`): lexicographic order ==
/// chronological order (the verifier compares time strings, §04.5) and the
/// gamma layer parses it strictly (§07.1). civil_from_days per Hinnant.
fn ts(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn now_string() -> String {
    ts(now_secs())
}

fn owner_from(seed_hex: &str) -> Result<OwnerKeys, Box<dyn std::error::Error>> {
    eprintln!("WARNING: --seed-hex on the command line is DEV ONLY.");
    let seed = MasterSeed::from_slice(&hex::decode(seed_hex)?)?;
    Ok(OwnerKeys::genesis(&seed))
}

fn bundle_at(dir: &str) -> Result<Bundle<FsStore>, Box<dyn std::error::Error>> {
    Ok(Bundle::open(FsStore::new(dir))?)
}

fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder.to_owned(), name.to_owned()),
        None => (String::new(), path.to_owned()),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Init {
            seed_hex,
            succession_seed_hex,
            dir,
        } => init(seed_hex, succession_seed_hex, dir),
        Command::FolderAdd {
            dir,
            seed_hex,
            zone,
            path,
        } => {
            let owner = owner_from(&seed_hex)?;
            let mut bundle = bundle_at(&dir)?;
            bundle.ensure_folder(Zone::parse(&zone)?, &path, &owner, &mut OsEntropy)?;
            println!("folder ready: {zone}/{path}");
            Ok(())
        }
        Command::SectionAdd {
            dir,
            seed_hex,
            zone,
            path,
            title,
            tags,
            body,
        } => {
            let owner = owner_from(&seed_hex)?;
            let mut bundle = bundle_at(&dir)?;
            let (folder, name) = split_path(&path);
            let tags: Vec<String> = tags
                .split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .collect();
            bundle.section_add(
                &SectionSpec {
                    zone: Zone::parse(&zone)?,
                    folder_path: &folder,
                    name: &name,
                    title: &title,
                    tags: &tags,
                    body: &body,
                    now: &now_string(),
                },
                &owner,
                &mut OsEntropy,
            )?;
            println!("section written: {zone}/{path}");
            Ok(())
        }
        Command::ZoneShow {
            dir,
            seed_hex,
            zone,
        } => {
            let owner = owner_from(&seed_hex)?;
            let bundle = bundle_at(&dir)?;
            for path in bundle.zone_tree(Zone::parse(&zone)?, &owner)? {
                println!("{path}");
            }
            Ok(())
        }
        Command::SectionRead {
            dir,
            seed_hex,
            zone,
            path,
        } => {
            let zone = Zone::parse(&zone)?;
            let body = match (zone, seed_hex) {
                (Zone::Public, _) => {
                    Bundle::<FsStore>::public_read(&bundle_at(&dir)?.store, &path)?
                }
                (_, Some(seed)) => {
                    let owner = owner_from(&seed)?;
                    bundle_at(&dir)?.read_section(zone, &path, &owner)?
                }
                _ => return Err("this zone needs --seed-hex".into()),
            };
            println!("{body}");
            Ok(())
        }
        Command::EditionPublish { dir, seed_hex } => {
            let owner = owner_from(&seed_hex)?;
            let mut bundle = bundle_at(&dir)?;
            bundle.publish(&owner, &now_string())?;
            println!("edition published");
            Ok(())
        }
        Command::EditionVerify { dir } => {
            bundle_at(&dir)?.verify()?;
            println!("edition chain: OK");
            Ok(())
        }
        Command::Grant {
            dir,
            seed_hex,
            agent_seed_hex,
            label,
            folder,
            tag,
            ttl_days,
            issue_depth,
        } => {
            let owner = owner_from(&seed_hex)?;
            let agent = ed25519_dalek::SigningKey::from_bytes(
                &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
                    .map_err(|_| "agent-seed-hex: 32 bytes")?,
            );
            let start = now_secs();
            let (nb, na) = (ts(start), ts(start + u64::from(ttl_days) * 86_400));
            eprintln!("window: not_before={nb} not_after={na}");
            let mut bundle = bundle_at(&dir)?;
            let spec = aithos_bundle::grants::GrantSpec {
                zone: Zone::Circle,
                dir: folder,
                tag,
            };
            let m = bundle.grant(
                &owner,
                &label,
                &agent.verifying_key(),
                &[spec],
                &nb,
                &na,
                issue_depth,
                &mut OsEntropy,
            )?;
            // Issuance is never silent (spec 07.4).
            bundle.log_owner_grant(&owner, &m.id, &now_string(), &mut OsEntropy)?;
            println!("granted; cert = certs/{}.json", m.id);
            Ok(())
        }
        Command::GrantAct {
            dir,
            seed_hex,
            agent_seed_hex,
            label,
            connector,
            action_pat,
            ttl_days,
            max_actions,
            heartbeat_every,
            heartbeat_grace,
        } => {
            let owner = owner_from(&seed_hex)?;
            let agent = ed25519_dalek::SigningKey::from_bytes(
                &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
                    .map_err(|_| "agent-seed-hex: 32 bytes")?,
            );
            let start = now_secs();
            let (nb, na) = (ts(start), ts(start + u64::from(ttl_days) * 86_400));
            let mut bundle = bundle_at(&dir)?;
            let mut constraints = serde_json::Map::new();
            if let Some(n) = max_actions {
                constraints.insert("max_actions".into(), n.into());
            }
            if let (Some(every), Some(grace)) = (&heartbeat_every, &heartbeat_grace) {
                constraints.insert(
                    "heartbeat".into(),
                    serde_json::json!({"every": every, "grace": grace}),
                );
            }
            let mut nonce = [0u8; 16];
            getrandom(&mut nonce)?;
            let mut id_bytes = [0u8; 16];
            getrandom(&mut id_bytes)?;
            let m = aithos_core::mandate::Mandate::build_root(
                &owner.root_sign,
                &aithos_core::mandate::MandateSpec {
                    id: format!(
                        "mandate_{}",
                        aithos_core::ids::Sid(ulid::Ulid::from(u128::from_be_bytes(id_bytes)))
                    ),
                    subject: bundle.did.clone(),
                    grantee_id: format!("urn:aithos:agent:{label}"),
                    grantee_label: label,
                    grantee_pub: &agent.verifying_key(),
                    perimeter: vec![aithos_core::mandate::PerimeterEntry::parse(&format!(
                        "act.x.{connector}.{action_pat}"
                    ))?],
                    constraints: serde_json::Value::Object(constraints),
                    not_before: nb,
                    not_after: na,
                    issued_at: ts(start),
                    nonce: hex::encode(nonce),
                },
            )?;
            std::fs::create_dir_all(format!("{dir}/certs"))?;
            std::fs::write(
                format!("{dir}/certs/{}.json", m.id),
                serde_json::to_vec_pretty(&m)?,
            )?;
            bundle.log_owner_grant(&owner, &m.id, &now_string(), &mut OsEntropy)?;
            println!("granted; cert = certs/{}.json", m.id);
            Ok(())
        }
        Command::Action {
            dir,
            certs,
            agent_seed_hex,
            connector,
            action,
            args,
        } => {
            let mut bundle = bundle_at(&dir)?;
            let chain: Vec<aithos_core::mandate::Mandate> = certs
                .iter()
                .map(|c| -> Result<_, Box<dyn std::error::Error>> {
                    Ok(serde_json::from_slice(&std::fs::read(c)?)?)
                })
                .collect::<Result<_, _>>()?;
            let agent = ed25519_dalek::SigningKey::from_bytes(
                &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
                    .map_err(|_| "agent-seed-hex: 32 bytes")?,
            );
            let args_hash = format!(
                "sha256:{}",
                aithos_bundle::manifest::sha256_hex(args.as_bytes())
            );
            let entry = bundle.log_action(
                &chain,
                &agent,
                &aithos_bundle::log::ActionSpec {
                    connector: &connector,
                    action: &action,
                    args_hash: &args_hash,
                    now: &now_string(),
                    budget: None,
                    sealed_args: None,
                },
                &mut OsEntropy,
            )?;
            println!("action logged: {}", entry.id);
            Ok(())
        }
        Command::Heartbeat { dir, seed_hex, seq } => {
            let owner = owner_from(&seed_hex)?;
            let mut bundle = bundle_at(&dir)?;
            bundle.log_heartbeat(&owner, seq, &now_string(), &mut OsEntropy)?;
            println!("beacon {seq} published");
            Ok(())
        }
        Command::LogShow { dir } => {
            let bundle = bundle_at(&dir)?;
            for e in bundle.gamma_entries()? {
                let author = e.authorized_by.as_deref().unwrap_or("owner");
                let target = match (&e.target, &e.body_enc) {
                    (Some(t), _) => t.as_str(),
                    (None, Some(_)) => "(sealed)",
                    (None, None) => "-",
                };
                println!(
                    "{}  {}  {:<14}  {:<10}  {}",
                    e.id, e.at, e.kind, author, target
                );
            }
            println!("head: {}", bundle.gamma_head()?);
            Ok(())
        }
        Command::LogVerify { dir } => {
            bundle_at(&dir)?.gamma_verify()?;
            println!("gamma chain: OK");
            Ok(())
        }
        Command::LogQuery {
            dir,
            seed_hex,
            kind,
            action,
            since,
            until,
            folder,
            tag,
            mandate,
        } => {
            let owner = owner_from(&seed_hex)?;
            let bundle = bundle_at(&dir)?;
            let filter = aithos_bundle::log::LogFilter {
                kind,
                action,
                since,
                until,
                zone_dir: folder.map(|f| (Zone::Circle, f)),
                tag,
                mandate,
            };
            for hit in bundle.log_query_owner(&owner, &filter)? {
                let e = &hit.entry;
                let target = hit
                    .body
                    .as_ref()
                    .map(|b| b.target.clone())
                    .or_else(|| e.target.clone())
                    .unwrap_or_default();
                println!("{}  {}  {:<14}  {}", e.id, e.at, e.kind, target);
            }
            Ok(())
        }
        Command::MandateVerify { dir, cert, at } => {
            let bundle = bundle_at(&dir)?;
            let doc: DidDocument =
                serde_json::from_slice(&std::fs::read(format!("{dir}/did.json"))?)?;
            let m: aithos_core::mandate::Mandate = serde_json::from_slice(&std::fs::read(&cert)?)?;
            let _ = &bundle;
            aithos_core::mandate::verify_chain(&[m], &doc, &at)?;
            println!("mandate: OK at {at}");
            Ok(())
        }
        Command::SectionReadAgent {
            dir,
            cert,
            agent_seed_hex,
            at,
            path,
        } => {
            let bundle = bundle_at(&dir)?;
            let m: aithos_core::mandate::Mandate = serde_json::from_slice(&std::fs::read(&cert)?)?;
            let agent = ed25519_dalek::SigningKey::from_bytes(
                &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
                    .map_err(|_| "agent-seed-hex: 32 bytes")?,
            );
            let body = bundle.read_section_as_agent(&[m], &agent, Zone::Circle, &path, &at)?;
            println!("{body}");
            Ok(())
        }
        Command::NodeKey { path, zone_dk_hex } => node_key_cmd(&path, &zone_dk_hex),
        Command::HeaderSeal {
            node,
            subject_did,
            dk_hex,
            recipients,
        } => header_seal_cmd(&node, &subject_did, &dk_hex, &recipients),
        Command::HeaderOpen {
            file,
            subject_did,
            kid,
            sk_hex,
            version,
        } => header_open_cmd(&file, &subject_did, &kid, &sk_hex, version),
    }
}

fn header_seal_cmd(
    node: &str,
    subject_did: &str,
    dk_hex: &str,
    recipients: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::{Header, Recipient};
    let dk: [u8; 32] = hex::decode(dk_hex)?
        .try_into()
        .map_err(|_| "dk-hex: expected 32 bytes")?;
    let mut recs = Vec::new();
    for spec in recipients {
        let parts: Vec<&str> = spec.splitn(3, ':').collect();
        let [label, kid, pub_hex] = parts[..] else {
            return Err("recipient format: label:kid:x25519_pub_hex".into());
        };
        let pubkey: [u8; 32] = hex::decode(pub_hex)?
            .try_into()
            .map_err(|_| "recipient pubkey: expected 32 bytes")?;
        recs.push(Recipient {
            to: label.to_owned(),
            kid: kid.to_owned(),
            pubkey: pubkey.into(),
        });
    }
    // Randomness injected at the surface: one ephemeral + nonce per line.
    let mut ephemerals = Vec::new();
    let mut nonces = Vec::new();
    for _ in &recs {
        let mut e = [0u8; 32];
        getrandom(&mut e)?;
        let mut n = [0u8; 24];
        getrandom(&mut n)?;
        ephemerals.push(e);
        nonces.push(n);
    }
    let header = Header::build(subject_did, node, &dk, &recs, &ephemerals, &nonces)?;
    println!("{}", serde_json::to_string_pretty(&header)?);
    Ok(())
}

fn header_open_cmd(
    file: &str,
    subject_did: &str,
    kid: &str,
    sk_hex: &str,
    version: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use aithos_core::header::Header;
    let header: Header = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    header.validate()?;
    let sk: [u8; 32] = hex::decode(sk_hex)?
        .try_into()
        .map_err(|_| "sk-hex: expected 32 bytes")?;
    let dk = header.open(subject_did, version, kid, &sk.into())?;
    println!("{}", hex::encode(dk));
    Ok(())
}

fn node_key_cmd(path: &str, zone_dk_hex: &str) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("WARNING: node-key is a DEV/debug verb; never expose real zone keys.");
    let zone: [u8; 32] = hex::decode(zone_dk_hex)?
        .try_into()
        .map_err(|_| "zone-dk-hex: expected 32 bytes")?;
    let parsed = aithos_core::path::NodePath::parse(path)?;
    println!(
        "{}",
        hex::encode(aithos_core::derive::node_key(&zone, &parsed))
    );
    Ok(())
}

fn seed32(
    hex_or_random: Option<String>,
    what: &str,
) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    match hex_or_random {
        Some(h) => {
            eprintln!("WARNING: --{what} is for tests/vectors only.");
            Ok(hex::decode(h)?
                .try_into()
                .map_err(|_| format!("{what}: expected 32 bytes"))?)
        }
        None => {
            // OS randomness is injected here, at the surface — never inside core.
            let mut bytes = [0u8; 32];
            getrandom(&mut bytes)?;
            Ok(bytes)
        }
    }
}

fn init(
    seed_hex: Option<String>,
    succession_seed_hex: Option<String>,
    dir: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let seed = MasterSeed::from_bytes(seed32(seed_hex, "seed-hex")?);
    let succession_entropy = seed32(succession_seed_hex, "succession-seed-hex")?;
    let keys = OwnerKeys::genesis(&seed);
    let succession = succession_from_entropy(succession_entropy);
    let doc = DidDocument::build(
        &keys,
        &succession.verifying_key(),
        vec!["file://local".to_owned()],
        "gamma/gamma.jsonl".to_owned(),
    )?;
    doc.verify()?;
    if let Some(dir) = dir {
        Bundle::init(
            FsStore::new(&dir),
            &keys,
            &succession.verifying_key(),
            &mut OsEntropy,
            &now_string(),
        )?;
        eprintln!("bundle initialised in {dir}");
    }
    let root_pub = keys.root_sign.verifying_key().to_bytes();
    let out = serde_json::json!({
        "did": doc.id,
        "root_sign_pub": hex::encode(root_pub),
        "content_sign_pub": hex::encode(keys.content_sign.verifying_key().to_bytes()),
        "owner_kex_pub": hex::encode(keys.owner_kex_pub().to_bytes()),
        "succession_pub": hex::encode(succession.verifying_key().to_bytes()),
        "succession_secret_hex": hex::encode(succession_entropy),
        "did_document": doc,
    });
    eprintln!("STORE succession_secret_hex COLD (paper/HSM) — it is shown ONCE and never derivable again.");
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn getrandom(buf: &mut [u8]) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Read;
    File::open("/dev/urandom")?.read_exact(buf)
}

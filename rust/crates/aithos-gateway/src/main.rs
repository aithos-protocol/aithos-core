//! aithos-gateway binary: onboard, run, audit-export.
//!
//! The library stays pure (T and entropy injected); this surface supplies
//! the system clock and OS randomness, exactly like the aithos-core CLI.

use std::sync::Arc;

use clap::{Parser, Subcommand};

use aithos_gateway::config::GatewayConfig;
use aithos_gateway::core_bridge::{
    Bridge, EntropySource, MandateWindow, OnboardOutcome, OsEntropy, Runner,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_mcp::{router, router_multi, HttpUpstream, McpProxy, McpRouter};
use aithos_gateway::store_adapter::GatewayStore;

#[derive(Parser)]
#[command(name = "aithos-gateway", version, about = "Aithos runner gateway")]
struct Cli {
    /// Path to the gateway YAML configuration.
    #[arg(long, global = true, default_value = "gateway.yaml")]
    config: String,

    /// Path to the runner identity file (seeds; runner custody only).
    #[arg(long, global = true, default_value = "agent.id")]
    identity: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Birth of the runner: generate the agent identity in place. Only
    /// the PUBLIC keys are printed — the seeds never leave the file.
    Keygen,
    /// OWNER SIDE (never in the runner): create the agent's journal —
    /// an enterprise-owned Ethos where the agent gets the xref pen.
    OwnerInitJournal {
        /// Enterprise master seed (DEV ONLY on the command line).
        #[arg(long)]
        master_seed_hex: String,
        /// Stable agent label (derivation label of the journal keys).
        #[arg(long)]
        agent_label: String,
        /// The agent public key published at birth (z…).
        #[arg(long)]
        agent_pub: String,
        /// The gateway public key published at birth (z…).
        #[arg(long)]
        gateway_pub: String,
        /// Filesystem root of the journal store.
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
    },
    /// OWNER SIDE: create a context Ethos (demo/dev — real contexts
    /// usually pre-exist).
    OwnerInitContext {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        store_root: String,
    },
    /// OWNER SIDE: grant a context to the agent's public key (read
    /// tools + gateway governance + scoped auditor).
    OwnerGrantContext {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        agent_pub: String,
        #[arg(long)]
        gateway_pub: String,
        /// Tool granted for reading (repeatable).
        #[arg(long = "read")]
        read: Vec<String>,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
    },
    /// Initialise the ethos, mint identities, grant the read-only agent
    /// mandate, the gateway governance mandate and the scoped auditor
    /// mandate; print the endpoint to plug into the agent runtime.
    Onboard {
        /// Validity of the minted mandates, in days.
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
    },
    /// Run the gateway (agent-facing MCP endpoint + policy + gamma).
    Run,
    /// Export the audit slice the auditor's mandate covers (JSON on stdout).
    AuditExport {
        /// The auditor's signing seed (handed out at onboarding).
        #[arg(long)]
        auditor_seed_hex: String,
        /// Kind scope of the query (the granted scope is `action`).
        #[arg(long, default_value = "action")]
        kind: String,
        /// Multi-context config: the context to audit — each context has
        /// its OWN gamma, auditor mandate and auditor seed. Required with
        /// `contexts`, refused with the mono shape.
        #[arg(long)]
        context: Option<String>,
    },
}

fn main() -> std::process::ExitCode {
    match run(Cli::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Birth needs no config: the runner exists before it is equipped.
    if matches!(cli.command, Command::Keygen) {
        let mut ent = OsEntropy;
        let keyholder = Keyholder::from_entropy(ent.e32(), ent.e32());
        keyholder.save(std::path::Path::new(&cli.identity))?;
        println!(
            "agent_pub: {}",
            aithos_gateway::core_bridge::agent_pub_multibase(&keyholder)
        );
        println!(
            "gateway_pub: {}",
            aithos_gateway::core_bridge::gateway_pub_multibase(&keyholder)
        );
        eprintln!(
            "identity written to {} — runner custody, hand out only the public keys above.",
            cli.identity
        );
        return Ok(());
    }

    // Owner-side commands run where the master seed lives; they need no
    // gateway config, only a store root.
    match &cli.command {
        Command::OwnerInitJournal {
            master_seed_hex,
            agent_label,
            agent_pub,
            gateway_pub,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            let outcome = aithos_gateway::core_bridge::owner_init_journal(
                &master,
                agent_label,
                agent_pub,
                gateway_pub,
                GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                    root: store_root.into(),
                })?,
                &MandateWindow {
                    not_before: ts(start),
                    not_after: ts(start + u64::from(*ttl_days) * 86_400),
                },
                &ts(start),
                &mut OsEntropy,
            )?;
            println!("journal_did: {}", outcome.ethos_did);
            println!("agent_mandate: {}", outcome.agent_mandate);
            println!("gateway_mandate: {}", outcome.gateway_mandate);
            return Ok(());
        }
        Command::OwnerInitContext {
            master_seed_hex,
            label,
            store_root,
        } => {
            let master = decode_master(master_seed_hex)?;
            let did = aithos_gateway::core_bridge::owner_init_context(
                &master,
                label,
                GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                    root: store_root.into(),
                })?,
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("context_did: {did}");
            return Ok(());
        }
        Command::OwnerGrantContext {
            master_seed_hex,
            label,
            agent_pub,
            gateway_pub,
            read,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            let outcome = aithos_gateway::core_bridge::owner_grant_context(
                &master,
                label,
                agent_pub,
                gateway_pub,
                read,
                GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                    root: store_root.into(),
                })?,
                &MandateWindow {
                    not_before: ts(start),
                    not_after: ts(start + u64::from(*ttl_days) * 86_400),
                },
                &ts(start),
                &mut OsEntropy,
            )?;
            eprintln!("STORE the auditor seed COLD — shown ONCE.");
            println!("context_did: {}", outcome.ethos_did);
            println!("agent_mandate: {}", outcome.agent_mandate);
            println!("gateway_mandate: {}", outcome.gateway_mandate);
            if let (Some(m), Some(s)) = (&outcome.auditor_mandate, &outcome.auditor_seed_hex) {
                println!("auditor_mandate: {m}");
                println!("auditor_seed_hex: {s}");
            }
            return Ok(());
        }
        _ => {}
    }

    let cfg_text = std::fs::read_to_string(&cli.config)
        .map_err(|e| format!("cannot read config `{}`: {e}", cli.config))?;
    let cfg = GatewayConfig::from_yaml(&cfg_text)?;

    match cli.command {
        Command::Keygen
        | Command::OwnerInitJournal { .. }
        | Command::OwnerInitContext { .. }
        | Command::OwnerGrantContext { .. } => unreachable!("handled above"),
        Command::Onboard { ttl_days } => {
            let mut ent = OsEntropy;
            let keyholder = Keyholder::from_entropy(ent.e32(), ent.e32());
            keyholder.save(std::path::Path::new(&cli.identity))?;
            let start = now_secs();
            let window = MandateWindow {
                not_before: ts(start),
                not_after: ts(start + u64::from(ttl_days) * 86_400),
            };
            let (_bridge, outcome) = Bridge::onboard(
                &cfg,
                GatewayStore::from_config(cfg.mono_store()?)?,
                keyholder,
                Box::new(OsEntropy),
                &window,
                &ts(start),
            )?;
            print_onboard(&outcome);
            Ok(())
        }
        Command::Run => {
            let keyholder = Keyholder::load(std::path::Path::new(&cli.identity))?;
            // Multi-context config → the routed runtime (v2, lot 3):
            // one bridge per context + the journal, one upstream per
            // context, the same single agent-facing endpoint.
            let app = if let Some(contexts) = &cfg.contexts {
                let runner = Runner::open(&cfg, keyholder, || Box::new(OsEntropy))?;
                let upstreams = contexts
                    .iter()
                    .map(|c| (c.name.clone(), HttpUpstream::new(c.upstream_mcp.clone())))
                    .collect();
                router_multi(Arc::new(McpRouter {
                    runner: tokio::sync::Mutex::new(runner),
                    upstreams,
                    clock: Arc::new(|| ts(now_secs())),
                }))
            } else {
                let bridge = Bridge::open(
                    GatewayStore::from_config(cfg.mono_store()?)?,
                    Arc::new(keyholder),
                    Box::new(OsEntropy),
                )?;
                router(Arc::new(McpProxy {
                    policy: Policy::new(cfg.tools.clone()),
                    bridge: tokio::sync::Mutex::new(bridge),
                    upstream: HttpUpstream::new(cfg.mono_upstream()?.to_owned()),
                    clock: Arc::new(|| ts(now_secs())),
                }))
            };
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(async {
                let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
                eprintln!("gateway listening on http://{}/mcp", cfg.listen);
                axum::serve(listener, app).await?;
                Ok(())
            })
        }
        Command::AuditExport {
            auditor_seed_hex,
            kind,
            context,
        } => {
            // The store to audit: a named context's in the multi shape
            // (each context carries its own gamma and audit grant), the
            // single store in the mono shape. Any mismatch fails closed.
            let store_cfg = match (&cfg.contexts, &context) {
                (Some(contexts), Some(name)) => contexts
                    .iter()
                    .find(|c| &c.name == name)
                    .map(|c| &c.store)
                    .ok_or_else(|| format!("audit-export: unknown context `{name}`"))?,
                (Some(_), None) => {
                    return Err(
                        "audit-export: a multi-context config needs --context <name>".into(),
                    )
                }
                (None, Some(_)) => {
                    return Err("audit-export: --context needs a `contexts` config".into())
                }
                (None, None) => cfg.mono_store()?,
            };
            let keyholder = Keyholder::load(std::path::Path::new(&cli.identity))?;
            let bridge = Bridge::open(
                GatewayStore::from_config(store_cfg)?,
                Arc::new(keyholder),
                Box::new(OsEntropy),
            )?;
            let seed: [u8; 32] = hex::decode(&auditor_seed_hex)
                .ok()
                .and_then(|v| v.try_into().ok())
                .ok_or("auditor-seed-hex: want 32 hex bytes")?;
            let export = bridge.export_audit(&seed, Some(&kind), &ts(now_secs()))?;
            println!("{export}");
            Ok(())
        }
    }
}

fn print_onboard(o: &OnboardOutcome) {
    eprintln!("STORE the seeds below COLD — they are shown ONCE and never persisted here.");
    println!("owner_did: {}", o.owner_did);
    println!("owner_seed_hex: {}", o.owner_seed_hex);
    println!("succession_secret_hex: {}", o.succession_secret_hex);
    println!("auditor_seed_hex: {}", o.auditor_seed_hex);
    println!("agent_mandate: {}", o.agent_mandate);
    println!("gateway_mandate: {}", o.gateway_mandate);
    println!("auditor_mandate: {}", o.auditor_mandate);
    println!("agent_endpoint: {}", o.agent_endpoint);
    println!();
    println!("Point the agent's MCP client at agent_endpoint — nothing else changes.");
}

fn decode_master(hex_str: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    eprintln!("WARNING: --master-seed-hex on the command line is DEV ONLY.");
    Ok(hex::decode(hex_str)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or("master-seed-hex: want 32 hex bytes")?)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`), same construction as the
/// aithos-core CLI (civil_from_days per Hinnant): lexicographic order ==
/// chronological order, and the gamma layer parses it strictly.
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

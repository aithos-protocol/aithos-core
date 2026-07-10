//! aithos-gateway binary: onboard, run, audit-export.
//!
//! The library stays pure (T and entropy injected); this surface supplies
//! the system clock and OS randomness, exactly like the aithos-core CLI.

use std::sync::Arc;

use clap::{Parser, Subcommand};

use aithos_gateway::config::GatewayConfig;
use aithos_gateway::core_bridge::{
    Bridge, EntropySource, MandateWindow, OnboardOutcome, OsEntropy,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_mcp::{router, HttpUpstream, McpProxy};
use aithos_gateway::store_adapter::GatewayStore;

#[derive(Parser)]
#[command(name = "aithos-gateway", version, about = "Aithos runner gateway")]
struct Cli {
    /// Path to the gateway YAML configuration.
    #[arg(long, global = true, default_value = "gateway.yaml")]
    config: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
    let cfg_text = std::fs::read_to_string(&cli.config)
        .map_err(|e| format!("cannot read config `{}`: {e}", cli.config))?;
    let cfg = GatewayConfig::from_yaml(&cfg_text)?;

    match cli.command {
        Command::Onboard { ttl_days } => {
            let mut ent = OsEntropy;
            let keyholder = Keyholder::from_entropy(ent.e32(), ent.e32());
            let start = now_secs();
            let window = MandateWindow {
                not_before: ts(start),
                not_after: ts(start + u64::from(ttl_days) * 86_400),
            };
            let (_bridge, outcome) = Bridge::onboard(
                &cfg,
                GatewayStore::from_config(&cfg.store)?,
                keyholder,
                Box::new(OsEntropy),
                &window,
                &ts(start),
            )?;
            print_onboard(&outcome);
            Ok(())
        }
        Command::Run => {
            let bridge = Bridge::open(GatewayStore::from_config(&cfg.store)?, Box::new(OsEntropy))?;
            let proxy = Arc::new(McpProxy {
                policy: Policy::new(cfg.tools.clone()),
                bridge: tokio::sync::Mutex::new(bridge),
                upstream: HttpUpstream::new(cfg.upstream_mcp.clone()),
                clock: Arc::new(|| ts(now_secs())),
            });
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(async {
                let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
                eprintln!("gateway listening on http://{}/mcp", cfg.listen);
                axum::serve(listener, router(proxy)).await?;
                Ok(())
            })
        }
        Command::AuditExport {
            auditor_seed_hex,
            kind,
        } => {
            let bridge = Bridge::open(GatewayStore::from_config(&cfg.store)?, Box::new(OsEntropy))?;
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

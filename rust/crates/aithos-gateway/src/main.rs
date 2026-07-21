//! aithos-gateway binary: onboard, run, audit-export.
//!
//! The library stays pure (T and entropy injected); this surface supplies
//! the system clock and OS randomness, exactly like the aithos-core CLI.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use clap::{Parser, Subcommand};
use rustls::pki_types::UnixTime;
use tokio::sync::watch;

use aithos_gateway::config::{GatewayConfig, RelayCertificateConfig, RelayConfig};
use aithos_gateway::core_bridge::{
    Bridge, EntropySource, MandateWindow, OnboardOutcome, OsEntropy, Runner,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::policy::Policy;
use aithos_gateway::proxy_llm::{router_llm, HttpLlmUpstream, LlmProxy};
use aithos_gateway::proxy_mcp::{
    router, router_multi, verify_hub_upstreams_except, HttpUpstream, McpProxy, McpRouter,
};
use aithos_gateway::public_tls::{
    load_private_pem, public_tls_slot, AcmeCertificateManager, AcmeTxtClient, CertificateSource,
    InstantAcmeIssuer, PublicTlsAcceptor, PublicTlsActivator, SecureTlsCache,
};
use aithos_gateway::relay::{RelayClient, RelayHealth, RelayInputs, RelayReadiness};
use aithos_gateway::relay_application::relay_application_channel;
use aithos_gateway::store_adapter::GatewayStore;
use aithos_gateway::upstream_oauth::{self, UpstreamOAuthRegistry};

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
        /// Also grant the budgeted inference pen (Phase C): the total
        /// token budget of the agent's LLM tap. No budget, no LLM.
        #[arg(long)]
        token_budget: Option<u64>,
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
    /// OWNER SIDE: replay a locally provisioned journal or context onto
    /// the provider through the signed A.2 wire. This is the deliberate
    /// seed/promotion step for mode B and mode A demos; local runner state
    /// never leaves the machine.
    OwnerReplicateHistory {
        #[arg(long)]
        master_seed_hex: String,
        /// Owner derivation domain: journal | context.
        #[arg(long)]
        kind: String,
        /// `agent-label` for a journal, context label otherwise.
        #[arg(long)]
        label: String,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value = "https://store.aithos.fr")]
        url: String,
        #[arg(long)]
        tenant: String,
    },
    /// Render the no-secret provider-backed configuration for the Léa
    /// CLI demo (ventes mode A, journal mode B, three Vault references).
    DemoLeaRenderConfig {
        #[arg(long)]
        output: String,
        #[arg(long, default_value = "127.0.0.1:4890")]
        listen: String,
        #[arg(long, default_value = "http://127.0.0.1:8200")]
        vault_address: String,
        #[arg(long, default_value = "https://store.aithos.fr")]
        provider_url: String,
        #[arg(long)]
        tenant: String,
        #[arg(long, default_value = "http://127.0.0.1:9201/mcp")]
        notion_url: String,
        #[arg(long, default_value = "http://127.0.0.1:9202/mcp")]
        gmail_url: String,
        #[arg(long, default_value = "http://127.0.0.1:9203/mcp")]
        calendar_url: String,
        #[arg(long)]
        context_root: String,
        #[arg(long)]
        context_did: String,
        #[arg(long)]
        context_mandate: String,
        #[arg(long)]
        journal_sidecar: String,
        #[arg(long)]
        journal_did: String,
        #[arg(long)]
        journal_mandate: String,
        /// Replace an existing generated file.
        #[arg(long)]
        force: bool,
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
    /// OWNER SIDE: capture one upstream tools/list into a proposed
    /// manifest. Discovery grants nothing and stores nothing in an Ethos.
    OwnerDiscoverServer {
        #[arg(long)]
        server: String,
        #[arg(long)]
        url: String,
        /// JSON proposal to review before enrollment.
        #[arg(long)]
        output: String,
    },
    /// OWNER SIDE: approve every discovered tool's risk class, seal the
    /// approved manifest in /x/<server>, then mint the context grants.
    /// Repeat --proposal to enroll several servers into the context in
    /// ONE gesture: a single agent mandate then covers the union of the
    /// granted tools (the demo shape). Tool names must be unambiguous
    /// across the batch — a name advertised by two servers refuses.
    OwnerEnrollServer {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        agent_pub: String,
        #[arg(long)]
        gateway_pub: String,
        /// Proposed manifest emitted by owner-discover-server
        /// (repeatable — one per server).
        #[arg(long = "proposal", required = true)]
        proposals: Vec<String>,
        /// Explicit owner decision, repeat for every tool:
        /// TOOL=read|write[:granted|denied]. Without a decision the
        /// safe defaults apply — reads granted, writes denied.
        #[arg(
            long = "approve",
            value_name = "TOOL=read|write[:granted|denied]",
            required = true
        )]
        approvals: Vec<String>,
        /// Argument bound on a granted tool, repeatable:
        /// TOOL:FIELD=one_of:v1,v2 | TOOL:FIELD=slots:tue,thu@14:00-18:00
        /// | TOOL:FIELD=forbid | TOOL:FIELD=require | TOOL:FIELD=max:N
        #[arg(long = "bound", value_name = "TOOL:FIELD=RULE")]
        bounds: Vec<String>,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
        /// Replace an existing pin for the same agent key and revoke
        /// every superseded runtime mandate after issuing fresh ones.
        #[arg(long)]
        replace: bool,
    },
    /// OWNER SIDE: grant the briefing pen on an equipped context — the
    /// read mandate and zone lines on the `briefing/` folders of the
    /// public and circle zones (lot K). Separate gesture on purpose: one
    /// pen per usage, revocable independently of the tool grants.
    OwnerGrantBriefing {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        agent_pub: String,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
    },
    /// OWNER SIDE: mint the ethos-read pen on an equipped context (lot
    /// G6) — a plain read mandate on the asked zones plus the circle
    /// line to the agent AND the context auditor. Never a toggle: the
    /// runtime derives the surface by scanning certificates, so this
    /// gesture, a delegate's sub-mandate or the future multi-mandate
    /// surface all light it the same way. `self` is refused while the
    /// delegated self resolution is its own core lot.
    OwnerGrantEthosRead {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        agent_pub: String,
        /// Comma-separated zones (public, circle).
        #[arg(long, default_value = "public,circle")]
        zones: String,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 30)]
        ttl_days: u32,
    },
    /// OWNER SIDE: add one fresh section to a zone of an equipped
    /// context (lot G6 owner tooling — GAPS beat 2, filling the zones).
    /// Title = the last path segment; the ethos data tools serve it on
    /// the very next call when the surface covers it.
    OwnerAddSection {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        zone: String,
        /// Display path, folders included (e.g. memoire/prospects).
        #[arg(long)]
        path: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        store_root: String,
    },
    /// OWNER SIDE: write or update one zone's directive (creation on
    /// first use, in-place rewrite afterwards — served on the very next
    /// briefing.read, no restart). `self` holds owner-only notes that
    /// never reach the agent.
    OwnerSetBriefing {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        /// Target zone: public | circle | self.
        #[arg(long)]
        zone: String,
        #[arg(long, default_value = "")]
        title: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        store_root: String,
    },
    /// OWNER SIDE: preview the effective policy of an equipped context —
    /// the stable read-model JSON the UI renders (mandate lifecycle,
    /// tools, inherited bounds), or with --call the dry-run verdict of
    /// one hypothetical call. Preview and runtime decide from the same
    /// inputs: the preview IS the decision.
    OwnerPreviewMandate {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        /// Enrolled hub server to include (repeatable).
        #[arg(long = "server")]
        servers: Vec<String>,
        #[arg(long)]
        store_root: String,
        /// Dry-run: the exposed tool name of one hypothetical call.
        #[arg(long)]
        call: Option<String>,
        /// Dry-run arguments as a JSON object (needs --call; default {}).
        #[arg(long)]
        args: Option<String>,
        /// Evaluation instant, RFC 3339 Z (defaults to the system clock).
        #[arg(long)]
        at: Option<String>,
    },
    /// OWNER SIDE: start an OAuth authorization-code + PKCE consent for
    /// one configured upstream. The URL is public; all pending and token
    /// state is written to Vault. With a positive wait, this command serves
    /// the configured `/oauth/callback` until connected or timed out.
    OwnerConnectOauth {
        #[arg(long)]
        server: String,
        #[arg(long, default_value_t = 300)]
        wait_secs: u64,
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
            token_budget,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            let outcome = aithos_gateway::core_bridge::owner_init_journal(
                &master,
                agent_label,
                agent_pub,
                gateway_pub,
                *token_budget,
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
            if let Some(m) = &outcome.memory_mandate {
                println!("memory_mandate: {m}");
            }
            if let Some(m) = &outcome.inference_mandate {
                println!("inference_mandate: {m}");
            }
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
        Command::OwnerReplicateHistory {
            master_seed_hex,
            kind,
            label,
            store_root,
            url,
            tenant,
        } => {
            let master = decode_master(master_seed_hex)?;
            let (did, report) = aithos_gateway::core_bridge::owner_replicate_history_to_remote(
                &master,
                kind,
                label,
                std::path::Path::new(store_root),
                url,
                tenant,
                Arc::new(|| ts(now_secs())),
                Box::new(OsEntropy),
            )?;
            println!("replicated_did: {did}");
            println!("protocol_objects: {}", report.protocol_objects);
            println!("editions: {}", report.editions);
            println!("gamma_segments: {}", report.gamma_segments);
            println!("unchanged: {}", report.unchanged);
            return Ok(());
        }
        Command::DemoLeaRenderConfig {
            output,
            listen,
            vault_address,
            provider_url,
            tenant,
            notion_url,
            gmail_url,
            calendar_url,
            context_root,
            context_did,
            context_mandate,
            journal_sidecar,
            journal_did,
            journal_mandate,
            force,
        } => {
            let path = std::path::Path::new(output);
            if path.exists() && !*force {
                return Err(format!(
                    "demo config `{}` already exists — pass --force to replace it",
                    path.display()
                )
                .into());
            }
            let yaml = aithos_gateway::demo_lea::render_provider_config(
                &aithos_gateway::demo_lea::DemoLeaConfigInput {
                    listen: listen.clone(),
                    vault_address: vault_address.clone(),
                    provider_url: provider_url.clone(),
                    tenant: tenant.clone(),
                    notion_url: notion_url.clone(),
                    gmail_url: gmail_url.clone(),
                    calendar_url: calendar_url.clone(),
                    context_root: context_root.clone(),
                    context_did: context_did.clone(),
                    context_mandate: context_mandate.clone(),
                    journal_sidecar: journal_sidecar.clone(),
                    journal_did: journal_did.clone(),
                    journal_mandate: journal_mandate.clone(),
                },
            )?;
            std::fs::write(path, yaml)?;
            println!("demo_config: {}", path.display());
            println!("agent_endpoint: http://{listen}/mcp");
            println!("tenant: {tenant}");
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
        Command::OwnerGrantBriefing {
            master_seed_hex,
            label,
            agent_pub,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            let mandate = aithos_gateway::core_bridge::owner_grant_briefing(
                &master,
                label,
                agent_pub,
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
            println!("briefing_mandate: {mandate}");
            return Ok(());
        }
        Command::OwnerGrantEthosRead {
            master_seed_hex,
            label,
            agent_pub,
            zones,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            let zone_list: Vec<String> = zones
                .split(',')
                .map(str::trim)
                .filter(|z| !z.is_empty())
                .map(str::to_owned)
                .collect();
            let mandate = aithos_gateway::core_bridge::owner_grant_ethos_read(
                &master,
                label,
                agent_pub,
                &zone_list,
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
            println!("ethos_read_mandate: {mandate}");
            return Ok(());
        }
        Command::OwnerAddSection {
            master_seed_hex,
            label,
            zone,
            path,
            text,
            store_root,
        } => {
            let master = decode_master(master_seed_hex)?;
            let start = now_secs();
            aithos_gateway::core_bridge::owner_add_section(
                &master,
                label,
                zone,
                path,
                text,
                GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                    root: store_root.into(),
                })?,
                &ts(start),
                &mut OsEntropy,
            )?;
            println!("section_added: {zone}:{path}");
            return Ok(());
        }
        Command::OwnerSetBriefing {
            master_seed_hex,
            label,
            zone,
            title,
            text,
            store_root,
        } => {
            let master = decode_master(master_seed_hex)?;
            aithos_gateway::core_bridge::owner_set_briefing(
                &master,
                label,
                zone,
                title,
                text,
                GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                    root: store_root.into(),
                })?,
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("briefing_zone: {zone}");
            return Ok(());
        }
        Command::OwnerDiscoverServer {
            server,
            url,
            output,
        } => {
            let upstream = HttpUpstream::new(url.clone());
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let proposed = rt.block_on(aithos_gateway::hub::discover_server(server, &upstream))?;
            std::fs::write(output, serde_json::to_vec_pretty(&proposed)?)?;
            println!("proposal: {output}");
            println!("server: {server}");
            println!("tools: {}", proposed.tools.len());
            return Ok(());
        }
        Command::OwnerEnrollServer {
            master_seed_hex,
            label,
            agent_pub,
            gateway_pub,
            proposals,
            approvals,
            bounds,
            store_root,
            ttl_days,
            replace,
        } => {
            let master = decode_master(master_seed_hex)?;
            let mut proposed_manifests = Vec::new();
            for proposal in proposals {
                let proposed: aithos_gateway::hub::ProposedManifest =
                    serde_json::from_slice(&std::fs::read(proposal)?)?;
                proposed_manifests.push(proposed);
            }
            let mut approvals = parse_approvals(approvals)?;
            attach_bounds(&mut approvals, bounds)?;
            // Split the flat approvals across the proposals by tool
            // name, fail-closed: a name advertised by two servers is
            // ambiguous, an approval matching no server is a typo, and
            // approve_manifest still requires every tool decided.
            let mut owners_of: BTreeMap<&str, &str> = BTreeMap::new();
            for proposed in &proposed_manifests {
                for tool in &proposed.tools {
                    if let Some(other) = owners_of.insert(&tool.name, &proposed.server) {
                        return Err(format!(
                            "tool `{}` is advertised by both `{other}` and `{}` — enroll them separately",
                            tool.name, proposed.server
                        )
                        .into());
                    }
                }
            }
            if let Some(unknown) = approvals
                .keys()
                .find(|t| !owners_of.contains_key(t.as_str()))
            {
                return Err(format!("--approve names undiscovered tool `{unknown}`").into());
            }
            let mut approved_manifests = Vec::new();
            for proposed in &proposed_manifests {
                let subset: BTreeMap<String, aithos_gateway::hub::ToolApproval> = approvals
                    .iter()
                    .filter(|(tool, _)| {
                        owners_of.get(tool.as_str()) == Some(&proposed.server.as_str())
                    })
                    .map(|(tool, approval)| (tool.clone(), approval.clone()))
                    .collect();
                approved_manifests.push(aithos_gateway::hub::approve_manifest(proposed, &subset)?);
            }
            let start = now_secs();
            let store = GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                root: store_root.into(),
            })?;
            let window = MandateWindow {
                not_before: ts(start),
                not_after: ts(start + u64::from(*ttl_days) * 86_400),
            };
            let (outcome, revoked) = if *replace {
                if approved_manifests.len() != 1 {
                    return Err("--replace re-enrolls exactly one server at a time".into());
                }
                let replaced = aithos_gateway::core_bridge::owner_reenroll_server(
                    &master,
                    label,
                    agent_pub,
                    gateway_pub,
                    &approved_manifests[0],
                    store,
                    &window,
                    &ts(start),
                    &mut OsEntropy,
                )?;
                (replaced.equipment, replaced.revoked_mandates)
            } else {
                (
                    aithos_gateway::core_bridge::owner_enroll_servers(
                        &master,
                        label,
                        agent_pub,
                        gateway_pub,
                        &approved_manifests,
                        store,
                        &window,
                        &ts(start),
                        &mut OsEntropy,
                    )?,
                    Vec::new(),
                )
            };
            eprintln!("STORE the auditor seed COLD — shown ONCE.");
            println!("context_did: {}", outcome.ethos_did);
            for approved in &approved_manifests {
                println!("server: {}", approved.server);
            }
            println!("agent_mandate: {}", outcome.agent_mandate);
            println!("gateway_mandate: {}", outcome.gateway_mandate);
            if let (Some(m), Some(s)) = (&outcome.auditor_mandate, &outcome.auditor_seed_hex) {
                println!("auditor_mandate: {m}");
                println!("auditor_seed_hex: {s}");
            }
            for mandate in revoked {
                println!("revoked_mandate: {mandate}");
            }
            return Ok(());
        }
        Command::OwnerPreviewMandate {
            master_seed_hex,
            label,
            servers,
            store_root,
            call,
            args,
            at,
        } => {
            let master = decode_master(master_seed_hex)?;
            let store = GatewayStore::from_config(&aithos_gateway::config::StoreConfig::Fs {
                root: store_root.into(),
            })?;
            let now = at.clone().unwrap_or_else(|| ts(now_secs()));
            let preview = match call {
                Some(tool) => {
                    let args: serde_json::Value = match args {
                        Some(text) => serde_json::from_str(text)
                            .map_err(|e| format!("--args is not a JSON value: {e}"))?,
                        None => serde_json::json!({}),
                    };
                    aithos_gateway::core_bridge::owner_preview_call(
                        &master, label, servers, store, tool, &args, &now,
                    )?
                }
                None => {
                    if args.is_some() {
                        return Err("--args needs --call".into());
                    }
                    aithos_gateway::core_bridge::owner_preview_mandate(
                        &master, label, servers, store, &now,
                    )?
                }
            };
            println!("{}", serde_json::to_string_pretty(&preview)?);
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
        | Command::OwnerReplicateHistory { .. }
        | Command::DemoLeaRenderConfig { .. }
        | Command::OwnerGrantContext { .. }
        | Command::OwnerGrantBriefing { .. }
        | Command::OwnerGrantEthosRead { .. }
        | Command::OwnerAddSection { .. }
        | Command::OwnerSetBriefing { .. }
        | Command::OwnerDiscoverServer { .. }
        | Command::OwnerEnrollServer { .. }
        | Command::OwnerPreviewMandate { .. } => unreachable!("handled above"),
        Command::OwnerConnectOauth { server, wait_secs } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            let brokers = aithos_gateway::credentials::build_brokers(&cfg)?;
            let oauth = Arc::new(UpstreamOAuthRegistry::from_config(&cfg, &brokers)?);
            let consent = rt.block_on(oauth.start(&server))?;
            println!("{}", consent.authorization_url);
            if wait_secs == 0 {
                eprintln!(
                    "OAuth consent prepared for {server}; start the gateway and open the URL above."
                );
                return Ok(());
            }
            eprintln!(
                "waiting up to {wait_secs}s for {} /oauth/callback",
                cfg.listen
            );
            rt.block_on(async {
                let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
                let app = upstream_oauth::router(Arc::clone(&oauth));
                let server_task = tokio::spawn(async move { axum::serve(listener, app).await });
                let connected =
                    tokio::time::timeout(std::time::Duration::from_secs(wait_secs), async {
                        loop {
                            if oauth.is_connected(&server).await {
                                break;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                    })
                    .await;
                server_task.abort();
                match connected {
                    Ok(()) => {
                        eprintln!("OAuth connection established for {server}.");
                        Ok(())
                    }
                    Err(_) => Err::<(), Box<dyn std::error::Error>>(
                        "OAuth callback timed out; restart owner-connect-oauth".into(),
                    ),
                }
            })
        }
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
            let keyholder = Arc::new(Keyholder::load(std::path::Path::new(&cli.identity))?);
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            // Multi-context config → the routed runtime (v2, lot 3):
            // one bridge per context + the journal, one upstream per
            // context, the same single agent-facing endpoint.
            let app = if let Some(contexts) = &cfg.contexts {
                let runner = Arc::new(tokio::sync::Mutex::new(Runner::open_shared(
                    &cfg,
                    Arc::clone(&keyholder),
                    || Box::new(OsEntropy),
                )?));
                let brokers = aithos_gateway::credentials::build_brokers(&cfg)?;
                let upstream_oauth = Arc::new(UpstreamOAuthRegistry::from_config(&cfg, &brokers)?);
                let upstreams = if let Some(servers) = &cfg.servers {
                    // Brokered credentials: build every configured
                    // broker once, then wire each server to its source
                    // (vault reference, legacy inline bearer, or none).
                    servers
                        .iter()
                        .map(|server| {
                            Ok::<_, aithos_gateway::GatewayError>((
                                server.name.clone(),
                                HttpUpstream::for_server_with_oauth(
                                    server,
                                    &brokers,
                                    &upstream_oauth,
                                )?,
                            ))
                        })
                        .collect::<aithos_gateway::Result<_>>()?
                } else {
                    contexts
                        .iter()
                        .map(|c| {
                            Ok::<_, aithos_gateway::GatewayError>((
                                c.name.clone(),
                                HttpUpstream::new(c.legacy_upstream()?.to_owned()),
                            ))
                        })
                        .collect::<aithos_gateway::Result<_>>()?
                };
                // The OAuth authorization server (lot G3), when `as:` is
                // active: the adapter key is born (or loaded) here — a
                // 0600 secret beside the identity, NEVER in the
                // keyholder, from OS entropy on first use.
                let oauth = if let Some(as_cfg) = &cfg.oauth_as {
                    let adapter = aithos_gateway::oauth::AdapterKey::load_or_create(
                        &as_cfg.key_file,
                        &mut OsEntropy,
                    )?;
                    Some(Arc::new(aithos_gateway::oauth::AuthServer::new(
                        adapter,
                        &as_cfg.issuer,
                        as_cfg.access_ttl_secs as i64,
                        as_cfg.refresh_ttl_secs as i64,
                        as_cfg.redirect_allowlist.clone(),
                        Box::new(OsEntropy),
                    )))
                } else {
                    None
                };
                let routing = Arc::new(McpRouter {
                    runner: Arc::clone(&runner),
                    upstreams,
                    clock: Arc::new(|| ts(now_secs())),
                    session_entropy: std::sync::Mutex::new(Box::new(OsEntropy)),
                    oauth: oauth.clone(),
                });
                if cfg.is_hub() {
                    let deferred = rt.block_on(upstream_oauth.disconnected_server_names());
                    rt.block_on(verify_hub_upstreams_except(&routing, &deferred))?;
                }
                let mut app = router_multi(Arc::clone(&routing));
                if !upstream_oauth.is_empty() {
                    app = app.merge(upstream_oauth::router(Arc::clone(&upstream_oauth)));
                }
                // The AS endpoints ride the SAME listener (G2 shell
                // precedent): discovery, registration, authorize, token.
                if oauth.is_some() {
                    app = app.merge(aithos_gateway::proxy_mcp::router_oauth(Arc::clone(
                        &routing,
                    )));
                }
                // The LLM front (Phase C): same runner, same journal —
                // the completions endpoint rides the same listener.
                if let Some(llm) = &cfg.llm {
                    app = app.merge(router_llm(Arc::new(LlmProxy {
                        runner,
                        upstream: HttpLlmUpstream::new(llm.upstream.clone(), llm.api_key.clone()),
                        model: llm.model.clone(),
                        provider: llm.provider.clone(),
                        clock: Arc::new(|| ts(now_secs())),
                    })));
                }
                app
            } else {
                let bridge = Bridge::open(
                    GatewayStore::from_config(cfg.mono_store()?)?,
                    Arc::clone(&keyholder),
                    Box::new(OsEntropy),
                )?;
                router(Arc::new(McpProxy {
                    policy: Policy::new(cfg.tools.clone()),
                    bridge: tokio::sync::Mutex::new(bridge),
                    upstream: HttpUpstream::new(cfg.mono_upstream()?.to_owned()),
                    clock: Arc::new(|| ts(now_secs())),
                }))
            };
            rt.block_on(serve_gateway(&cfg, app, keyholder))
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
            // P3: the auditor exports from whatever store the config
            // names — a replicated context reads its fs primary, a
            // remote one rides the wire under the runner identity (the
            // keyholder is loaded right here; from_config would refuse
            // the remote kinds fail-closed).
            let store = GatewayStore::from_config_with_identity(store_cfg, &keyholder, || {
                Box::new(OsEntropy)
            })?;
            let bridge = Bridge::open(store, Arc::new(keyholder), Box::new(OsEntropy))?;
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

const RELAY_APPLICATION_CAPACITY: usize = 64;
const PUBLIC_TLS_RETRY: Duration = Duration::from_secs(60);
const ACME_RENEWAL_CHECK: Duration = Duration::from_secs(6 * 60 * 60);
const ACME_RENEWAL_RETRY: Duration = Duration::from_secs(5 * 60);

struct PublicTlsRuntime {
    acceptor: PublicTlsAcceptor,
    renewal: Option<tokio::task::JoinHandle<()>>,
}

/// Serve one immutable application router through both ingress paths. Relay
/// setup and reconnect are isolated in their own supervisor: a certificate,
/// DNS or tunnel outage cannot take down the historical direct listener.
async fn serve_gateway(
    cfg: &GatewayConfig,
    app: Router,
    identity: Arc<Keyholder>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
    eprintln!("gateway listening on http://{}/mcp", cfg.listen);

    let Some(relay) = cfg.relay.clone() else {
        axum::serve(listener, app).await?;
        return Ok(());
    };

    let health = RelayHealth::new(RelayReadiness::Unavailable);
    let (shutdown_sender, shutdown) = watch::channel(false);
    let relay_task = tokio::spawn(run_relay_plane(
        relay,
        identity,
        app.clone(),
        health,
        shutdown,
    ));

    let direct_result = axum::serve(listener, app).await;
    let _ = shutdown_sender.send(true);
    let _ = relay_task.await;
    direct_result?;
    Ok(())
}

async fn run_relay_plane(
    config: RelayConfig,
    identity: Arc<Keyholder>,
    app: Router,
    health: RelayHealth,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }

        let tls = match prepare_public_tls(&config, Arc::clone(&identity), shutdown.clone()).await {
            Ok(tls) => tls,
            Err(_) => {
                eprintln!("relay public TLS unavailable; direct listener remains active");
                if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
                    return;
                }
                continue;
            }
        };
        let relay = match RelayClient::from_system_roots(config.clone()) {
            Ok(relay) => relay,
            Err(_) => {
                if let Some(renewal) = tls.renewal {
                    renewal.abort();
                }
                eprintln!("relay trust roots unavailable; direct listener remains active");
                if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
                    return;
                }
                continue;
            }
        };
        let (ingress, relay_listener) = match relay_application_channel(RELAY_APPLICATION_CAPACITY)
        {
            Ok(channel) => channel,
            Err(_) => return,
        };
        let relay_app = app.clone();
        let router_task = tokio::spawn(async move {
            let _ = axum::serve(relay_listener, relay_app).await;
        });
        let acceptor = tls.acceptor.clone();
        let inputs = relay_inputs();
        let relay_result = relay
            .run(
                Arc::clone(&identity),
                inputs,
                health.clone(),
                shutdown.clone(),
                move |stream| {
                    let ingress = ingress.clone();
                    let acceptor = acceptor.clone();
                    async move {
                        let _ = ingress.accept(&acceptor, stream).await;
                    }
                },
            )
            .await;

        router_task.abort();
        if let Some(renewal) = tls.renewal {
            renewal.abort();
        }
        if *shutdown.borrow() {
            return;
        }
        if relay_result.is_err() {
            eprintln!("relay supervisor unavailable; direct listener remains active");
        }
        if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
            return;
        }
    }
}

async fn prepare_public_tls(
    config: &RelayConfig,
    identity: Arc<Keyholder>,
    shutdown: watch::Receiver<bool>,
) -> aithos_gateway::Result<PublicTlsRuntime> {
    match &config.cert {
        RelayCertificateConfig::Pem {
            cert_file,
            key_file,
        } => {
            let current = load_private_pem(cert_file, key_file, &config.hostname, unix_time_now())?;
            let (_fixed, acceptor) = public_tls_slot(current);
            Ok(PublicTlsRuntime {
                acceptor,
                renewal: None,
            })
        }
        RelayCertificateConfig::AcmeDns01 {
            directory,
            store_url,
            cache_dir,
        } => {
            let cache = SecureTlsCache::open(cache_dir.clone())?;
            let dns = AcmeTxtClient::new(
                store_url,
                identity,
                Arc::new(|| ts(now_secs())),
                Arc::new(relay_nonce),
            )?;
            let issuer = InstantAcmeIssuer::new(directory.clone(), dns, cache.clone());
            let manager = Arc::new(AcmeCertificateManager::new(cache, issuer));
            let lease = manager.ensure(&config.hostname, unix_time_now()).await?;
            let (activator, acceptor) = public_tls_slot(lease.config);
            let hostname = config.hostname.clone();
            let renewal = tokio::spawn(renew_public_tls(manager, activator, hostname, shutdown));
            Ok(PublicTlsRuntime {
                acceptor,
                renewal: Some(renewal),
            })
        }
    }
}

async fn renew_public_tls(
    manager: Arc<AcmeCertificateManager<InstantAcmeIssuer>>,
    activator: PublicTlsActivator,
    hostname: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut delay = ACME_RENEWAL_CHECK;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
                continue;
            }
        }
        match manager.ensure(&hostname, unix_time_now()).await {
            Ok(lease) => {
                delay = if lease.source == CertificateSource::RetainedAfterRenewalFailure {
                    ACME_RENEWAL_RETRY
                } else {
                    ACME_RENEWAL_CHECK
                };
                activator.replace(lease.config);
            }
            Err(_) => delay = ACME_RENEWAL_RETRY,
        }
    }
}

async fn wait_for_relay_retry(shutdown: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

fn relay_inputs() -> RelayInputs {
    RelayInputs {
        clock: Arc::new(|| ts(now_secs())),
        nonce: Arc::new(relay_nonce),
        jitter: Arc::new(relay_jitter),
    }
}

fn relay_nonce() -> String {
    let mut entropy = OsEntropy;
    hex::encode(entropy.e16())
}

fn relay_jitter() -> u64 {
    let mut entropy = OsEntropy;
    let sample = entropy.e16();
    u64::from_le_bytes(sample[..8].try_into().expect("fixed entropy width"))
}

fn unix_time_now() -> UnixTime {
    UnixTime::since_unix_epoch(Duration::from_secs(now_secs()))
}

fn parse_approvals(
    values: &[String],
) -> Result<BTreeMap<String, aithos_gateway::hub::ToolApproval>, String> {
    use aithos_gateway::hub::ToolApproval;

    let mut out = BTreeMap::new();
    for value in values {
        let (tool, spec) = value
            .split_once('=')
            .ok_or_else(|| format!("--approve `{value}`: want TOOL=read|write[:granted|denied]"))?;
        if tool.trim().is_empty() {
            return Err("--approve has an empty tool name".into());
        }
        let (class, decision) = match spec.split_once(':') {
            Some((class, decision)) => (class, Some(decision)),
            None => (spec, None),
        };
        let class = match class {
            "read" => aithos_gateway::config::ToolAccess::Read,
            "write" => aithos_gateway::config::ToolAccess::Write,
            _ => return Err(format!("--approve `{value}`: class must be read or write")),
        };
        let approval = match decision {
            None => ToolApproval::class(class),
            Some("granted") => ToolApproval::granted(class),
            Some("denied") => ToolApproval::denied(class),
            Some(_) => {
                return Err(format!(
                    "--approve `{value}`: decision must be granted or denied"
                ))
            }
        };
        if out.insert(tool.to_owned(), approval).is_some() {
            return Err(format!("--approve repeats tool `{tool}`"));
        }
    }
    Ok(out)
}

/// Parse every --bound flag and attach it to its tool's approval.
/// Syntax: TOOL:FIELD=one_of:v1,v2 | TOOL:FIELD=slots:tue,thu@14:00-18:00
/// | TOOL:FIELD=forbid | TOOL:FIELD=require | TOOL:FIELD=max:N
fn attach_bounds(
    approvals: &mut BTreeMap<String, aithos_gateway::hub::ToolApproval>,
    bounds: &[String],
) -> Result<(), String> {
    use aithos_gateway::hub::ArgumentBound;

    for value in bounds {
        let (target, rule) = value
            .split_once('=')
            .ok_or_else(|| format!("--bound `{value}`: want TOOL:FIELD=RULE"))?;
        let (tool, field) = target
            .split_once(':')
            .ok_or_else(|| format!("--bound `{value}`: want TOOL:FIELD=RULE"))?;
        if tool.trim().is_empty() || field.trim().is_empty() {
            return Err(format!("--bound `{value}`: empty tool or field"));
        }
        let field = field.to_owned();
        let bound = if rule == "forbid" {
            ArgumentBound::Forbid { field }
        } else if rule == "require" {
            ArgumentBound::Require { field }
        } else if let Some(values) = rule.strip_prefix("one_of:") {
            ArgumentBound::OneOf {
                field,
                values: values.split(',').map(str::to_owned).collect(),
            }
        } else if let Some(max) = rule.strip_prefix("max:") {
            ArgumentBound::MaxItems {
                field,
                max: max
                    .parse()
                    .map_err(|_| format!("--bound `{value}`: max wants an integer"))?,
            }
        } else if let Some(slots) = rule.strip_prefix("slots:") {
            let (days, window) = slots
                .split_once('@')
                .ok_or_else(|| format!("--bound `{value}`: slots want DAYS@HH:MM-HH:MM"))?;
            let (from, to) = window
                .split_once('-')
                .ok_or_else(|| format!("--bound `{value}`: slots want DAYS@HH:MM-HH:MM"))?;
            let days = days
                .split(',')
                .map(|day| match day {
                    "mon" | "monday" => Ok("monday".to_owned()),
                    "tue" | "tuesday" => Ok("tuesday".to_owned()),
                    "wed" | "wednesday" => Ok("wednesday".to_owned()),
                    "thu" | "thursday" => Ok("thursday".to_owned()),
                    "fri" | "friday" => Ok("friday".to_owned()),
                    "sat" | "saturday" => Ok("saturday".to_owned()),
                    "sun" | "sunday" => Ok("sunday".to_owned()),
                    other => Err(format!("--bound `{value}`: unknown day `{other}`")),
                })
                .collect::<Result<Vec<_>, String>>()?;
            ArgumentBound::TimeSlots {
                field,
                days,
                from: from.to_owned(),
                to: to.to_owned(),
            }
        } else {
            return Err(format!(
                "--bound `{value}`: rule must be one_of:, slots:, forbid, require or max:"
            ));
        };
        let approval = approvals
            .get_mut(tool)
            .ok_or_else(|| format!("--bound `{value}`: tool `{tool}` has no --approve"))?;
        approval.bounds.push(bound);
    }
    Ok(())
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

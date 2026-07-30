//! `aithos owner …` — the owner-side ceremonies (famille A), ported from
//! the gateway binary at lot SPL-5. One ceremony, one path: the CLI calls
//! `aithos_owner::owner_*` directly over a local [`FsStore`]; the gateway
//! keeps a deprecated delegating surface during the double-surface period.
//!
//! Custody discipline mirrors the gateway surface: `--master-seed-hex` on
//! argv is DEV ONLY and says so; `grant-session-delegate` and
//! `revoke-mandate` read the master seed from stdin, never argv.

use std::io::Read as _;

use aithos_bundle::entropy::OsEntropy;
use aithos_bundle::FsStore;
use aithos_owner::MandateWindow;
use zeroize::Zeroize as _;

use super::common::{now_secs, ts};

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: OwnerCommand,
}

#[derive(clap::Subcommand)]
pub enum OwnerCommand {
    /// OWNER SIDE (never in the runner): create the agent's journal — an
    /// enterprise-owned Ethos where the agent gets the xref pen.
    InitJournal {
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
    InitContext {
        #[arg(long)]
        master_seed_hex: String,
        #[arg(long)]
        label: String,
        #[arg(long)]
        store_root: String,
    },
    /// OWNER SIDE: grant a context to the agent's public key (read
    /// tools + gateway governance + scoped auditor).
    GrantContext {
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
    /// OWNER SIDE: enrol one person public key as a delegated MCP session
    /// issuer. The owner master seed is read as 32-byte hexadecimal from
    /// stdin, never from argv; only the signed public mandate is persisted.
    GrantSessionDelegate {
        #[arg(long)]
        label: String,
        #[arg(long)]
        delegate_pub: String,
        /// Exact OAuth protected resource this parent may authorize.
        #[arg(long)]
        gateway_audience: String,
        /// Exact agent-facing MCP tool granted to the future session.
        #[arg(long = "tool", required = true)]
        tools: Vec<String>,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value_t = 7)]
        ttl_days: u32,
    },
    /// OWNER SIDE: revoke one context mandate. The owner master seed is
    /// read as 32-byte hexadecimal from stdin, never from argv.
    RevokeMandate {
        #[arg(long)]
        label: String,
        #[arg(long)]
        mandate_id: String,
        #[arg(long)]
        store_root: String,
        #[arg(long, default_value = "revoked by owner")]
        reason: String,
    },
    /// OWNER SIDE: grant the briefing pen on an equipped context — the
    /// read mandate and zone lines on the `briefing/` folders of the
    /// public and circle zones (lot K). Separate gesture on purpose: one
    /// pen per usage, revocable independently of the tool grants.
    GrantBriefing {
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
    /// line to the agent AND the context auditor.
    GrantEthosRead {
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
    /// context. Title = the last path segment; the ethos data tools serve
    /// it on the very next call when the surface covers it.
    AddSection {
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
    SetBriefing {
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
}

fn decode_master(hex_str: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    eprintln!("WARNING: --master-seed-hex on the command line is DEV ONLY.");
    Ok(hex::decode(hex_str)
        .ok()
        .and_then(|v| <[u8; 32]>::try_from(v).ok())
        .ok_or("master-seed-hex: want 32 hex bytes")?)
}

fn decode_master_stdin() -> Result<zeroize::Zeroizing<[u8; 32]>, Box<dyn std::error::Error>> {
    let mut encoded = zeroize::Zeroizing::new(String::new());
    std::io::stdin().take(130).read_to_string(&mut encoded)?;
    let mut decoded = zeroize::Zeroizing::new(
        hex::decode(encoded.trim()).map_err(|_| "stdin master seed is not hexadecimal")?,
    );
    let bytes = <[u8; 32]>::try_from(decoded.as_slice())
        .map_err(|_| "stdin master seed must contain exactly 32 bytes")?;
    decoded.zeroize();
    Ok(zeroize::Zeroizing::new(bytes))
}

fn window(start: u64, ttl_days: u32) -> MandateWindow {
    MandateWindow {
        not_before: ts(start),
        not_after: ts(start + u64::from(ttl_days) * 86_400),
    }
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        OwnerCommand::InitJournal {
            master_seed_hex,
            agent_label,
            agent_pub,
            gateway_pub,
            store_root,
            ttl_days,
            token_budget,
        } => {
            let master = decode_master(&master_seed_hex)?;
            let start = now_secs();
            let outcome = aithos_owner::owner_init_journal(
                &master,
                &agent_label,
                &agent_pub,
                &gateway_pub,
                token_budget,
                FsStore::new(&store_root),
                &window(start, ttl_days),
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
            Ok(())
        }
        OwnerCommand::InitContext {
            master_seed_hex,
            label,
            store_root,
        } => {
            let master = decode_master(&master_seed_hex)?;
            let did = aithos_owner::owner_init_context(
                &master,
                &label,
                FsStore::new(&store_root),
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("context_did: {did}");
            Ok(())
        }
        OwnerCommand::GrantContext {
            master_seed_hex,
            label,
            agent_pub,
            gateway_pub,
            read,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(&master_seed_hex)?;
            let start = now_secs();
            let outcome = aithos_owner::owner_grant_context(
                &master,
                &label,
                &agent_pub,
                &gateway_pub,
                &read,
                FsStore::new(&store_root),
                &window(start, ttl_days),
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
            Ok(())
        }
        OwnerCommand::GrantSessionDelegate {
            label,
            delegate_pub,
            gateway_audience,
            tools,
            store_root,
            ttl_days,
        } => {
            let master = decode_master_stdin()?;
            let start = now_secs();
            let mandate = aithos_owner::owner_grant_session_delegate(
                &master,
                &label,
                &delegate_pub,
                &gateway_audience,
                &tools,
                FsStore::new(&store_root),
                &window(start, ttl_days),
                &ts(start),
                &mut OsEntropy,
            )?;
            println!("session_delegate_mandate: {mandate}");
            println!("delegate_pub: {delegate_pub}");
            Ok(())
        }
        OwnerCommand::RevokeMandate {
            label,
            mandate_id,
            store_root,
            reason,
        } => {
            let master = decode_master_stdin()?;
            aithos_owner::owner_revoke_mandate_id(
                &master,
                &label,
                &mandate_id,
                &reason,
                FsStore::new(&store_root),
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("revoked_mandate: {mandate_id}");
            Ok(())
        }
        OwnerCommand::GrantBriefing {
            master_seed_hex,
            label,
            agent_pub,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(&master_seed_hex)?;
            let start = now_secs();
            let mandate = aithos_owner::owner_grant_briefing(
                &master,
                &label,
                &agent_pub,
                FsStore::new(&store_root),
                &window(start, ttl_days),
                &ts(start),
                &mut OsEntropy,
            )?;
            println!("briefing_mandate: {mandate}");
            Ok(())
        }
        OwnerCommand::GrantEthosRead {
            master_seed_hex,
            label,
            agent_pub,
            zones,
            store_root,
            ttl_days,
        } => {
            let master = decode_master(&master_seed_hex)?;
            let start = now_secs();
            let zone_list: Vec<String> = zones
                .split(',')
                .map(str::trim)
                .filter(|z| !z.is_empty())
                .map(str::to_owned)
                .collect();
            let mandate = aithos_owner::owner_grant_ethos_read(
                &master,
                &label,
                &agent_pub,
                &zone_list,
                FsStore::new(&store_root),
                &window(start, ttl_days),
                &ts(start),
                &mut OsEntropy,
            )?;
            println!("ethos_read_mandate: {mandate}");
            Ok(())
        }
        OwnerCommand::AddSection {
            master_seed_hex,
            label,
            zone,
            path,
            text,
            store_root,
        } => {
            let master = decode_master(&master_seed_hex)?;
            aithos_owner::owner_add_section(
                &master,
                &label,
                &zone,
                &path,
                &text,
                FsStore::new(&store_root),
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("section_added: {zone}:{path}");
            Ok(())
        }
        OwnerCommand::SetBriefing {
            master_seed_hex,
            label,
            zone,
            title,
            text,
            store_root,
        } => {
            let master = decode_master(&master_seed_hex)?;
            aithos_owner::owner_set_briefing(
                &master,
                &label,
                &zone,
                &title,
                &text,
                FsStore::new(&store_root),
                &ts(now_secs()),
                &mut OsEntropy,
            )?;
            println!("briefing_zone: {zone}");
            Ok(())
        }
    }
}

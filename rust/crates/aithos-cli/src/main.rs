//! `aithos` — local Core operations plus an explicit delegated OAuth client.
//! Bundle commands remain local; network access is isolated under `oauth`.
//!
//! Lot SPL-5: one module per command under `src/cmd/`; `main()` is parse +
//! dispatch, nothing else. The surface (names, flags, help, exit codes) is
//! asserted unchanged by `tests/cli_surface.rs`.

mod cmd;
mod custody;
mod delegated_oauth;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aithos", version, about = "Aithos Core CLI")]
struct Cli {
    /// Named local profile (bundle location + non-secret key-store reference).
    #[arg(long, global = true, default_value = "default")]
    profile: String,
    /// Override Aithos' application-data directory (also: AITHOS_HOME).
    #[arg(long, global = true)]
    home: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an Ethos, its DID and real random keys under managed custody.
    Init(cmd::init::Args),
    /// Show the active profile, custody backend and verification status.
    Status,
    /// OAuth client flows that retain signer custody outside process arguments.
    #[command(name = "oauth")]
    OAuth(cmd::oauth::Args),
    /// Create a folder (mkdir -p) in a zone of the bundle.
    FolderAdd(cmd::folder_add::Args),
    /// Add a section. PATH is folder/…/name; body from --body.
    SectionAdd(cmd::section_add::Args),
    /// Show a zone's display tree (owner-side for circle/self).
    ZoneShow(cmd::zone_show::Args),
    /// Read one section. Public needs NO key (omit --seed-hex).
    SectionRead(cmd::section_read::Args),
    /// Publish a new edition (height+1), signed by root.
    EditionPublish(cmd::edition_publish::Args),
    /// Verify the whole edition chain and pinned files. No keys needed.
    EditionVerify(cmd::edition_verify::Args),
    /// Merge another copy's competing edition into this bundle (spec 02.6):
    /// disjoint changesets only, deterministic result, signed by root.
    EditionMerge(cmd::edition_merge::Args),
    /// Grant an agent a circle perimeter: mints the cert AND delivers keys.
    Grant(cmd::grant::Args),
    /// Verify a mandate chain (one cert file) at time T. No keys needed.
    MandateVerify(cmd::mandate_verify::Args),
    /// Read a circle section AS an agent, gated by its mandate (spec 04.5).
    SectionReadAgent(cmd::section_read_agent::Args),
    /// DEV ONLY: derive a node key along a canonical sid-path (spec 02.5).
    /// Proves determinism by hand: same path, same key — every time.
    NodeKey(cmd::node_key::Args),
    /// Seal a node key into a header (spec 03). DEV surface over test keys.
    HeaderSeal(cmd::header_seal::Args),
    /// Open one's line in a header JSON (from --file) and print the node key.
    HeaderOpen(cmd::header_open::Args),
    /// Grant an agent CONNECTOR ACTION rights (certificate only — actions
    /// need no content keys). Counting constraints are enforced by gamma.
    GrantAct(cmd::grant_act::Args),
    /// Log a connector action under a mandate chain (leaf last). The gamma
    /// entry IS the authorization evidence — no entry, no action (I5).
    Action(cmd::action::Args),
    /// Sign an obligation receipt (spec 04.12): the attestor's verdict,
    /// bound to one mandate+action+args. Prints the checks[] JSON for
    /// `action --check-json`. With --owner-seed-hex, signs the owner
    /// co_sign instance (spec 04.6) with the content key.
    Approve(cmd::approve::Args),
    /// Log one metered LLM call (spec 07.9.1): counters only, never text.
    Inference(cmd::inference::Args),
    /// Build the O(log n) inclusion proof of a section against the signed
    /// Merkle root (spec 02.10) and verify it offline before printing.
    Prove(cmd::prove::Args),
    /// Build a count proof — or an absence proof — for a mandate against
    /// the committed gamma counts root (spec 07.10), verified offline
    /// before printing. Every TOTAL cap check rides one such proof.
    LogProve(cmd::log_prove::Args),
    /// Root-descent diff between two editions (spec 02.10): the node
    /// labels added, removed or changed. Defaults to previous → latest.
    EditionDiff(cmd::edition_diff::Args),
    /// Publish an owner liveness beacon (spec 07.5).
    Heartbeat(cmd::heartbeat::Args),
    /// Revoke a mandate (spec 06): one signed, anchored gamma entry. With
    /// --rotate <folder>, also turns the lock (rung 2/3) — fresh key sealed
    /// to survivors, up-link wrap, re-encryption.
    Revoke(cmd::revoke::Args),
    /// Move a circle folder under a new parent (spec 02.9): move IS a
    /// rotation — fresh key at the new path, direct lines re-sealed as
    /// survivors, up-link wrap under the NEW parent, subtree re-encrypted.
    /// Old-parent holders are cut; certificates follow the node (04.2).
    Move(cmd::move_folder::Args),
    /// Print the log's counting skeleton (what any file-holder sees).
    LogShow(cmd::log_show::Args),
    /// Verify the whole gamma chain and every entry signature. No keys needed.
    LogVerify(cmd::log_verify::Args),
    /// Owner audit of sealed action args (spec 07.9.3): reopen each sealed
    /// argument object, re-check its hash, optionally re-evaluate the
    /// action_params predicates of a mandate.
    LogAudit(cmd::log_audit::Args),
    /// Owner search over the log (spec 07.8): every present filter narrows.
    LogQuery(cmd::log_query::Args),
    /// OWNER SIDE ceremonies over a local store (famille A, ported from
    /// the aithos-gateway binary at lot SPL-5): journal/context equipment,
    /// grants, briefing and sections. One ceremony, one path.
    Owner(cmd::owner::Args),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    custody::validate_profile_name(&cli.profile)?;
    let home = match cli.home {
        Some(path) => std::path::PathBuf::from(path),
        None => custody::default_home()?,
    };
    let profile_name = cli.profile;
    cmd::common::RUNTIME_PROFILE
        .set((home.clone(), profile_name.clone()))
        .map_err(|_| "CLI profile was initialised twice")?;
    match cli.command {
        Command::Init(args) => cmd::init::run(args, &home, &profile_name),
        Command::Status => cmd::status::run(&home, &profile_name),
        Command::OAuth(args) => cmd::oauth::run(args),
        Command::FolderAdd(args) => cmd::folder_add::run(args),
        Command::SectionAdd(args) => cmd::section_add::run(args),
        Command::ZoneShow(args) => cmd::zone_show::run(args),
        Command::SectionRead(args) => cmd::section_read::run(args),
        Command::EditionPublish(args) => cmd::edition_publish::run(args),
        Command::EditionVerify(args) => cmd::edition_verify::run(args),
        Command::EditionMerge(args) => cmd::edition_merge::run(args),
        Command::Grant(args) => cmd::grant::run(args),
        Command::MandateVerify(args) => cmd::mandate_verify::run(args),
        Command::SectionReadAgent(args) => cmd::section_read_agent::run(args),
        Command::NodeKey(args) => cmd::node_key::run(args),
        Command::HeaderSeal(args) => cmd::header_seal::run(args),
        Command::HeaderOpen(args) => cmd::header_open::run(args),
        Command::GrantAct(args) => cmd::grant_act::run(args),
        Command::Action(args) => cmd::action::run(args),
        Command::Approve(args) => cmd::approve::run(args),
        Command::Inference(args) => cmd::inference::run(args),
        Command::Prove(args) => cmd::prove::run(args),
        Command::LogProve(args) => cmd::log_prove::run(args),
        Command::EditionDiff(args) => cmd::edition_diff::run(args),
        Command::Heartbeat(args) => cmd::heartbeat::run(args),
        Command::Revoke(args) => cmd::revoke::run(args),
        Command::Move(args) => cmd::move_folder::run(args),
        Command::LogShow(args) => cmd::log_show::run(args),
        Command::LogVerify(args) => cmd::log_verify::run(args),
        Command::LogAudit(args) => cmd::log_audit::run(args),
        Command::LogQuery(args) => cmd::log_query::run(args),
        Command::Owner(args) => cmd::owner::run(args),
    }
}

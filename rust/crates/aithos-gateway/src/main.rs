//! aithos-gateway binary: onboard, run, audit-export.
//!
//! Scaffold state: config loading and validation are live (fail-closed);
//! the three subcommands land with the audit MVP.

use clap::{Parser, Subcommand};

use aithos_gateway::config::GatewayConfig;

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
    /// Initialise the ethos, mint identities, grant the read-only mandate
    /// and the scoped auditor mandate; print the endpoints to plug in.
    Onboard,
    /// Run the gateway (agent-facing MCP endpoint + policy + gamma).
    Run,
    /// Export the audit slice an auditor's mandate covers.
    AuditExport,
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let cfg_text = match std::fs::read_to_string(&cli.config) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read config `{}`: {e}", cli.config);
            return std::process::ExitCode::from(2);
        }
    };
    let _cfg = match GatewayConfig::from_yaml(&cfg_text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(2);
        }
    };

    match cli.command {
        Command::Onboard | Command::Run | Command::AuditExport => {
            eprintln!("this subcommand lands with the audit MVP");
            std::process::ExitCode::from(2)
        }
    }
}

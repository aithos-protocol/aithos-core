//! `aithos oauth` — OAuth client flows that retain signer custody outside
//! process arguments.

use crate::delegated_oauth;

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub command: OAuthCommand,
}

#[derive(clap::Subcommand)]
pub enum OAuthCommand {
    /// Complete a production delegated-session ceremony using a signer on stdin.
    AuthorizeDelegated {
        /// Gateway origin or its protected MCP resource URL.
        #[arg(long)]
        gateway: String,
        /// Read one 32-byte delegate seed as hexadecimal from stdin.
        #[arg(long, required = true)]
        signer_stdin: bool,
        /// Create this private (0600) JSON file with the resulting OAuth tokens.
        #[arg(long)]
        token_output: std::path::PathBuf,
        /// Explicitly approve the locally verified WYSIWYS presentation.
        #[arg(long, required = true)]
        approve: bool,
        /// Select this exact eligible context when more than one is available.
        #[arg(long)]
        context: Option<String>,
        /// Select this exact eligible parent mandate when more than one is available.
        #[arg(long)]
        parent_id: Option<String>,
        /// OAuth scope requested from the gateway.
        #[arg(long)]
        scope: Option<String>,
        /// Public loopback redirect registered for the one-shot code exchange.
        #[arg(long, default_value = "http://127.0.0.1/aithos/callback")]
        redirect_uri: String,
    },
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let OAuthCommand::AuthorizeDelegated {
        gateway,
        signer_stdin,
        token_output,
        approve,
        context,
        parent_id,
        scope,
        redirect_uri,
    } = args.command;
    delegated_oauth::authorize_delegated(delegated_oauth::AuthorizeOptions {
        gateway,
        signer_stdin,
        token_output,
        approve,
        context,
        parent_id,
        scope,
        redirect_uri,
    })
}

//! Fail-closed error taxonomy: one named variant per kind of rejection,
//! mirroring the core's discipline. Tests assert on variants, not strings.

/// Every way the gateway can refuse or fail. Refusals are still logged;
/// a failure to log is itself a refusal (`LogAppendRefused`).
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// The tool is absent from the enterprise tool map — denied by default.
    #[error("tool `{0}` is not in the tool map — denied by default")]
    ToolNotMapped(String),

    /// The mandate does not cover this operation at T.
    #[error("mandate denies `{op}`: {reason}")]
    MandateDenied { op: String, reason: String },

    /// The act could not be appended to the gamma — the call must not proceed.
    #[error("gamma append refused: {0}")]
    LogAppendRefused(String),

    /// Configuration is malformed, unknown, or ambiguous — fail closed.
    #[error("config rejected: {0}")]
    ConfigRejected(String),

    /// The upstream MCP server is unreachable or spoke another protocol.
    #[error("upstream MCP failed: {0}")]
    UpstreamFailed(String),

    /// The brokered credential could not be resolved — vault down,
    /// reference absent, malformed answer. The reason is always a
    /// gateway-built summary (status class, fixed cause), NEVER the
    /// remote body, a header value or any secret material. The call
    /// this credential was for must not reach the upstream.
    #[error("credential unavailable: {0}")]
    CredentialUnavailable(String),

    /// The call's arguments violate an owner-approved bound (lot P).
    /// The message is deliberately pedagogical: it names the field, the
    /// offending values and the approved rule — that rule IS the granted
    /// perimeter, already sealed and logged owner-side, never a secret.
    #[error("bound violated: {0}")]
    BoundViolated(String),

    /// A governed server no longer advertises the owner-approved manifest.
    #[error("manifest drift for server `{server}`: {reason}")]
    ManifestDrift { server: String, reason: String },

    /// The agent-facing request is not something the gateway relays.
    #[error("request rejected: {0}")]
    RequestRejected(String),

    /// Audit export was refused (auditor mandate does not cover the query).
    #[error("audit read denied: {0}")]
    AuditDenied(String),

    /// The runner identity file is absent, unreadable or malformed.
    #[error("runner identity unavailable: {0}")]
    IdentityUnavailable(String),

    /// Core bridge failure that is not a policy denial (store I/O, state).
    #[error("core bridge failed: {0}")]
    BridgeFailed(String),
}

impl GatewayError {
    /// Short, non-sensitive reason code carried in refusal entries.
    pub fn refusal_code(&self) -> &'static str {
        match self {
            GatewayError::ToolNotMapped(_) => "tool_not_mapped",
            GatewayError::MandateDenied { .. } => "mandate_denied",
            GatewayError::LogAppendRefused(_) => "log_append_refused",
            GatewayError::ConfigRejected(_) => "config_rejected",
            GatewayError::UpstreamFailed(_) => "upstream_failed",
            GatewayError::CredentialUnavailable(_) => "credential_unavailable",
            GatewayError::BoundViolated(_) => "bound_violated",
            GatewayError::ManifestDrift { .. } => "manifest_drift",
            GatewayError::RequestRejected(_) => "request_rejected",
            GatewayError::AuditDenied(_) => "audit_denied",
            GatewayError::IdentityUnavailable(_) => "identity_unavailable",
            GatewayError::BridgeFailed(_) => "bridge_failed",
        }
    }
}

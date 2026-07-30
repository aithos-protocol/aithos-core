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

    /// An OAuth authorization-server refusal (lot G3): the `error` is the
    /// RFC 6749/8414 code (`invalid_grant`, `invalid_token`,
    /// `invalid_redirect_uri`…), the `detail` a fixed, leak-free
    /// explanation. NEITHER field ever carries a token, code or secret.
    #[error("oauth {error}: {detail}")]
    OauthDenied { error: String, detail: String },

    /// OAuth client custody for an upstream could not produce a usable
    /// access token. Details are fixed gateway summaries and never include
    /// provider bodies, codes, tokens, verifiers or client secrets.
    #[error("upstream OAuth unavailable: {0}")]
    UpstreamOauthUnavailable(String),

    /// The opt-in outbound relay could not establish or keep its C2
    /// tunnel. Details are gateway-built reason classes only: never a
    /// registration line, TLS payload, hostname query or application byte.
    #[error("relay unavailable: {0}")]
    RelayUnavailable(String),

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
            GatewayError::OauthDenied { .. } => "oauth_denied",
            GatewayError::UpstreamOauthUnavailable(_) => "upstream_oauth_unavailable",
            GatewayError::RelayUnavailable(_) => "relay_unavailable",
            GatewayError::AuditDenied(_) => "audit_denied",
            GatewayError::IdentityUnavailable(_) => "identity_unavailable",
            GatewayError::BridgeFailed(_) => "bridge_failed",
        }
    }
}

/// Une cérémonie propriétaire (`aithos-owner`, lot SPL-4) refuse ou échoue
/// dans la taxonomie du crate appelant : les variantes historiques et les
/// messages sont préservés octet pour octet.
impl From<aithos_owner::OwnerError> for GatewayError {
    fn from(error: aithos_owner::OwnerError) -> Self {
        match error {
            aithos_owner::OwnerError::Rejected(message) => GatewayError::ConfigRejected(message),
            aithos_owner::OwnerError::Failed(message) => GatewayError::BridgeFailed(message),
        }
    }
}

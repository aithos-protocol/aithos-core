//! Onboarding configuration: what the enterprise declares before plugging
//! an agent in. Parsed fail-closed: unknown keys, unknown access levels or
//! an unusable store are rejected outright, never guessed at.
//!
//! Three shapes, never mixed (v3, hub decisions of 2026-07-13):
//!
//! **Mono (legacy demo)** — one ethos, one upstream:
//!
//! ```yaml
//! listen: 127.0.0.1:4870
//! upstream_mcp: http://127.0.0.1:4124/mcp
//! store:
//!   kind: fs
//!   root: /var/lib/aithos
//! tools:
//!   user.read: read
//!   user.update: write
//! ```
//!
//! **Multi-context (v2)** — N provisioned contexts, each with its own
//! ethos store, upstream and tool map, plus the agent's journal:
//!
//! ```yaml
//! listen: 127.0.0.1:4870
//! contexts:
//!   - name: company-brand
//!     upstream_mcp: http://127.0.0.1:5001/mcp
//!     store: { kind: fs, root: /var/lib/aithos/brand }
//!     tools:
//!       brand.read: read
//!       brand.update: write
//! journal:
//!   store: { kind: fs, root: /var/lib/aithos/journal }
//! ```
//!
//! **Governed hub (v3)** — servers are first-class shared resources;
//! every context tool references one upstream tool explicitly:
//!
//! ```yaml
//! listen: 127.0.0.1:4870
//! servers:
//!   - name: github
//!     transport: http
//!     url: https://mcp.github.example/mcp
//! contexts:
//!   - name: engineering
//!     store: { kind: fs, root: /var/lib/aithos/engineering }
//!     tools:
//!       github__issues_list:
//!         server: github
//!         tool: issues.list
//!         access: read
//! journal:
//!   store: { kind: fs, root: /var/lib/aithos/journal }
//! ```
//!
//! Semantics: `read` tools are covered by the granted read-only mandate;
//! `write` tools are known but *not* granted (so refusals name the tool
//! precisely); anything absent from every tool map is denied by default.
//! Routing is by tool name, so a tool name that flattens identically in
//! two contexts would be ambiguous — rejected at parse time.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::credentials::CredentialRef;
use crate::{GatewayError, Result};

/// Access level the enterprise assigns to an MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolAccess {
    /// Covered by the read-only mandate: relayed and logged.
    Read,
    /// Known but outside the read-only mandate: refused and logged.
    Write,
}

/// Tool name → access level. BTreeMap for deterministic iteration
/// (mandate generation must be reproducible).
pub type ToolMap = BTreeMap<String, ToolAccess>;

/// Upstream transport supported by the governed hub v1. `stdio` needs
/// a separate custody-aware wrapper and is deliberately rejected here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerTransport {
    Http,
}

/// One first-class upstream MCP server (hub config v3).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub name: String,
    pub transport: ServerTransport,
    pub url: String,
    /// LEGACY/UNSAFE inline custody seam (H3): the token sits in clear
    /// in this file and durably in process memory. Kept only for old
    /// configs and scenarios; new configs use `credential`. Declaring
    /// both is rejected — exactly one credential source per server.
    #[serde(default)]
    pub bearer_token: Option<String>,
    /// The governed custody seam: a non-secret reference resolved
    /// through a configured `credential_brokers` entry, per call, after
    /// authorize + log-before-relay. The secret itself never lives in
    /// this file, in any store or in any log.
    #[serde(default)]
    pub credential: Option<CredentialRef>,
    /// OAuth 2.1 authorization-code + PKCE custody for a protected
    /// upstream. Every secret coordinate is a Vault reference; the
    /// access/refresh token set is written back to `token_vault`.
    #[serde(default)]
    pub oauth: Option<UpstreamOAuthConfig>,
}

/// One protected upstream's OAuth client declaration. URLs and the
/// `client_id` are public configuration; the client secret, pending PKCE
/// verifier and token set live only behind brokered Vault references.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamOAuthConfig {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: CredentialRef,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
    /// Dedicated KV field containing the encrypted-at-rest OAuth state.
    /// It must not alias `client_secret`: token writes replace this field.
    pub token_vault: CredentialRef,
}

/// One enterprise credential broker (top-level `credential_brokers:`).
/// Only non-secret coordinates live here — the vault access token is
/// itself read from the environment at resolution time, never from YAML.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrokerConfig {
    /// HashiCorp Vault KV v2 over HTTP(S). `address` must be HTTPS
    /// unless it points at loopback (the explicitly bounded dev/demo
    /// mode); `mount` is the KV v2 secrets engine mount.
    VaultKv2 {
        address: String,
        mount: String,
        auth: BrokerAuthConfig,
    },
}

/// How the gateway authenticates TO the vault.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrokerAuthConfig {
    /// Demo/dev: the vault token is read from this environment variable
    /// at resolution time. Enterprise auth methods (AppRole, Kubernetes)
    /// are later adapters behind the same seam.
    TokenEnv { env: String },
}

/// A context's reference to one raw tool on a first-class server.
/// `access` stays read/write in hub v1; the approved manifest added by
/// H2 will make the risk model extensible without weakening this parse.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HubToolRef {
    pub server: String,
    pub tool: String,
    pub access: ToolAccess,
    /// The grant decision this context claims for the tool (lot W).
    /// Absent = historic default (read granted, write denied). Must
    /// agree with the sealed approved manifest at runtime, fail-closed.
    #[serde(default)]
    pub granted: Option<bool>,
}

impl HubToolRef {
    pub fn is_granted(&self) -> bool {
        self.granted.unwrap_or(self.access == ToolAccess::Read)
    }
}

pub type HubToolMap = BTreeMap<String, HubToolRef>;

/// A context is either the legacy v2 scalar tool map or the hub v3
/// reference map. Serde rejects a mixture inside one context.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ContextTools {
    Legacy(ToolMap),
    Hub(HubToolMap),
}

impl Default for ContextTools {
    fn default() -> Self {
        Self::Legacy(ToolMap::new())
    }
}

impl ContextTools {
    /// Runtime v2 accessor. A hub config parses in H1 but remains
    /// closed until H3 wires governed pins into the router.
    pub fn legacy(&self) -> Result<&ToolMap> {
        match self {
            Self::Legacy(tools) => Ok(tools),
            Self::Hub(_) => Err(GatewayError::ConfigRejected(
                "hub tool references require the H3 governed runtime".into(),
            )),
        }
    }
}

/// Where the ethos (bundle + gamma) lives. Both variants parse from day
/// one — decided 2026-07-10: local disk first, cloud must stay possible.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum StoreConfig {
    /// Local filesystem (v1 default).
    Fs { root: PathBuf },
    /// S3-compatible object store (accepted by the parser, refused by the
    /// v1 adapter — see `store_adapter`).
    S3 {
        bucket: String,
        #[serde(default)]
        prefix: Option<String>,
    },
    /// P3 — mode B (provider-primary): the ethos lives on the RemoteStore
    /// (INFRA-PROVIDER §3.5), spoken over the signed wire A.2. The
    /// envelope signer is NEVER in the config (arbitrage ② 2026-07-21):
    /// the adapter takes it from the runner's keyholder (the agent leaf);
    /// `mandate` names the chain the envelopes ride, root first.
    Remote {
        url: String,
        tenant: String,
        did: String,
        #[serde(default)]
        mandate: Vec<String>,
        /// The mode-B SIDECAR root: where the runner-local and derived
        /// keys live (gateway/**, manifests/*) — the wire deliberately
        /// excludes them (doctrine: runner state never leaves the pod).
        /// Absent = in-memory (an ephemeral runner re-derives).
        #[serde(default)]
        local: Option<PathBuf>,
    },
    /// P3 — mode A (local-primary + réplique): fs stays the primary, the
    /// provider receives an ASYNCHRONOUS post-publish replication through
    /// the same client (the §3.5 decorator). A provider outage never
    /// blocks the agent — the primary answered already.
    Replicated {
        root: PathBuf,
        url: String,
        tenant: String,
        did: String,
        #[serde(default)]
        mandate: Vec<String>,
    },
}

/// One provisioned context (v2): an ethos the agent was granted into,
/// the upstream MCP server its tools live on, and the tool whitelist
/// that routes calls to it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextConfig {
    /// Routing name — the provisioning label of the context ethos.
    pub name: String,
    /// Where this context's ethos lives.
    pub store: StoreConfig,
    /// Legacy v2: the real MCP server this context's calls relay to.
    /// Hub v3 contexts omit it and reference `servers:` per tool.
    #[serde(default)]
    pub upstream_mcp: Option<String>,
    /// The tool whitelist that maps calls onto this context.
    #[serde(default)]
    pub tools: ContextTools,
}

impl ContextConfig {
    pub fn legacy_upstream(&self) -> Result<&str> {
        self.upstream_mcp.as_deref().ok_or_else(|| {
            GatewayError::ConfigRejected(
                "hub contexts have no per-context upstream; H3 is not active".into(),
            )
        })
    }

    pub fn legacy_tools(&self) -> Result<&ToolMap> {
        self.tools.legacy()
    }
}

/// The agent's journal (v2): the enterprise-owned ethos that keeps the
/// agent's own story — xref mirrors of every act, and every refusal.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalConfig {
    /// Where the journal ethos lives.
    pub store: StoreConfig,
}

/// The LLM front (Phase C): the gateway holds the provider credential,
/// imposes the model, meters real usage into the journal. Requires the
/// multi-context shape — the inference log IS a journal story.
///
/// v1 keeps the credential in the config file the enterprise already
/// guards; the decided end state (§3bis.4) moves it into the sealed
/// vault of an Ethos — same seam, the agent never sees it either way.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmConfig {
    /// The OpenAI-compatible chat-completions endpoint.
    pub upstream: String,
    /// Provider credential — gateway custody, never agent-visible.
    pub api_key: String,
    /// The imposed model: whatever the agent asks for is overwritten.
    pub model: String,
    /// Provider tag recorded in inference entries.
    #[serde(default = "default_provider")]
    pub provider: String,
}

fn default_provider() -> String {
    "openai-compat".to_owned()
}

/// The embedded OAuth authorization server (lot G3, chantier C1) —
/// OPT-IN: absent, the gateway behaves byte-identically (loopback open,
/// the demo path untouched). Present, /mcp requires a bearer token and
/// the AS endpoints ride the same listener. The signing key (the
/// "adapter key") is an ordinary gateway secret born at the first run
/// — NEVER a protocol object, never in this file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AsConfig {
    /// The URL clients use to reach this AS (RFC 8414 issuer). Explicit
    /// on purpose (decided 2026-07-17): behind the G1 tunnel it is the
    /// public hostname, never guessable from `listen`. Plaintext http
    /// is bounded to loopback, like the vault brokers.
    pub issuer: String,
    /// Where the adapter key lives (0600, beside the identity). Born
    /// from injected entropy at the first `run` with `as:` active.
    #[serde(default = "default_as_key_file")]
    pub key_file: PathBuf,
    /// Access token lifetime, seconds (default 3600 — decided
    /// 2026-07-17). Structurally capped by the bound chain's not_after.
    #[serde(default = "default_access_ttl")]
    pub access_ttl_secs: u64,
    /// Refresh token lifetime, seconds (default 7 days — decided
    /// 2026-07-17). A refresh never survives the chain's not_after.
    #[serde(default = "default_refresh_ttl")]
    pub refresh_ttl_secs: u64,
    /// EXTRA redirect_uris accepted at registration, beyond the
    /// built-in allowlist (the exact Claude callback + loopback on any
    /// port). Each entry is https, or http bounded to loopback.
    #[serde(default)]
    pub redirect_allowlist: Vec<String>,
}

fn default_as_key_file() -> PathBuf {
    PathBuf::from("as.key")
}

fn default_access_ttl() -> u64 {
    3_600
}

fn default_refresh_ttl() -> u64 {
    7 * 86_400
}

/// The whole gateway configuration file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayConfig {
    /// Agent-facing bind address (the pod-internal endpoint).
    pub listen: String,
    /// Mono shape: the real MCP server the gateway relays to.
    #[serde(default)]
    pub upstream_mcp: Option<String>,
    /// Mono shape: where the ethos lives.
    #[serde(default)]
    pub store: Option<StoreConfig>,
    /// Mono shape: the enterprise tool whitelist. Empty map = everything
    /// denied.
    #[serde(default)]
    pub tools: ToolMap,
    /// Governed hub shape (v3): shared first-class MCP servers. When
    /// present, contexts must use referenced tools and omit their v2
    /// `upstream_mcp` fields.
    #[serde(default)]
    pub servers: Option<Vec<ServerConfig>>,
    /// Enterprise credential brokers (hub shape only): the non-secret
    /// coordinates of the vault(s) that hold upstream MCP tokens.
    #[serde(default)]
    pub credential_brokers: Option<BTreeMap<String, BrokerConfig>>,
    /// Multi-context shape (v2): the provisioned contexts. Mutually
    /// exclusive with the mono fields above.
    #[serde(default)]
    pub contexts: Option<Vec<ContextConfig>>,
    /// Multi-context shape (v2): the agent's journal. Required whenever
    /// `contexts` is declared.
    #[serde(default)]
    pub journal: Option<JournalConfig>,
    /// The LLM front (Phase C) — only valid with the multi shape.
    #[serde(default)]
    pub llm: Option<LlmConfig>,
    /// The embedded OAuth authorization server (lot G3) — opt-in,
    /// multi-context shape only. `as` is a Rust keyword; the field
    /// rides under its YAML name.
    #[serde(default, rename = "as")]
    pub oauth_as: Option<AsConfig>,
}

impl GatewayConfig {
    /// Parse and validate a YAML config. Any ambiguity is a rejection.
    pub fn from_yaml(text: &str) -> Result<Self> {
        let cfg: GatewayConfig =
            serde_yaml::from_str(text).map_err(|e| GatewayError::ConfigRejected(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// The mono (legacy demo) upstream — a multi-context config has none.
    pub fn mono_upstream(&self) -> Result<&str> {
        self.upstream_mcp.as_deref().ok_or_else(mono_only)
    }

    /// The mono (legacy demo) store — a multi-context config has none.
    pub fn mono_store(&self) -> Result<&StoreConfig> {
        self.store.as_ref().ok_or_else(mono_only)
    }

    pub fn is_hub(&self) -> bool {
        self.servers.is_some()
    }

    fn validate(&self) -> Result<()> {
        if self.listen.trim().is_empty() {
            return Err(GatewayError::ConfigRejected("`listen` is empty".into()));
        }
        match (&self.contexts, &self.journal) {
            // ------------------------------- multi-context v2 / hub v3
            (Some(contexts), Some(journal)) => {
                if self.upstream_mcp.is_some() || self.store.is_some() || !self.tools.is_empty() {
                    return Err(GatewayError::ConfigRejected(
                        "mono fields (`upstream_mcp`/`store`/`tools`) cannot mix with \
                         `contexts` — declare one shape or the other"
                            .into(),
                    ));
                }
                if contexts.is_empty() {
                    return Err(GatewayError::ConfigRejected("`contexts` is empty".into()));
                }
                validate_store(&journal.store, "journal.store")?;
                let mut names = std::collections::BTreeSet::new();
                for ctx in contexts {
                    if ctx.name.trim().is_empty() {
                        return Err(GatewayError::ConfigRejected(
                            "a context has an empty `name`".into(),
                        ));
                    }
                    if !names.insert(ctx.name.as_str()) {
                        return Err(GatewayError::ConfigRejected(format!(
                            "duplicate context name `{}`",
                            ctx.name
                        )));
                    }
                    validate_store(&ctx.store, &format!("contexts[{}].store", ctx.name))?;
                }
                match &self.servers {
                    Some(servers) => {
                        validate_hub(servers, contexts, self.credential_brokers.as_ref())?
                    }
                    None => {
                        if self.credential_brokers.is_some() {
                            return Err(GatewayError::ConfigRejected(
                                "`credential_brokers` requires the hub shape (`servers`) — \
                                 only first-class servers reference brokered credentials"
                                    .into(),
                            ));
                        }
                        validate_legacy_contexts(contexts)?
                    }
                }
                if let Some(llm) = &self.llm {
                    validate_llm(llm)?;
                }
                if let Some(oauth_as) = &self.oauth_as {
                    validate_as(oauth_as)?;
                }
                Ok(())
            }
            (Some(_), None) => Err(GatewayError::ConfigRejected(
                "`contexts` requires a `journal` — the agent's own story must land somewhere"
                    .into(),
            )),
            (None, Some(_)) => Err(GatewayError::ConfigRejected(
                "`journal` without `contexts` — the mono shape has no journal".into(),
            )),
            // -------------------------------------------- mono (legacy demo)
            (None, None) => {
                if self.servers.is_some() {
                    return Err(GatewayError::ConfigRejected(
                        "`servers` requires `contexts` and `journal` — declare the complete hub shape"
                            .into(),
                    ));
                }
                if self.credential_brokers.is_some() {
                    return Err(GatewayError::ConfigRejected(
                        "`credential_brokers` requires the hub shape (`servers`) — \
                         only first-class servers reference brokered credentials"
                            .into(),
                    ));
                }
                if self.llm.is_some() {
                    return Err(GatewayError::ConfigRejected(
                        "`llm` needs the multi-context shape (v1): the inference log \
                         lives in the agent's journal"
                            .into(),
                    ));
                }
                if self.oauth_as.is_some() {
                    return Err(GatewayError::ConfigRejected(
                        "`as:` needs the multi-context shape (`contexts` + `journal`): \
                         the OAuth session rides the runner's mandate chains and its \
                         issuance record lives in the journal"
                            .into(),
                    ));
                }
                let upstream = self.upstream_mcp.as_deref().ok_or_else(|| {
                    GatewayError::ConfigRejected(
                        "`upstream_mcp` is required (or declare `contexts`)".into(),
                    )
                })?;
                let store = self.store.as_ref().ok_or_else(|| {
                    GatewayError::ConfigRejected(
                        "`store` is required (or declare `contexts`)".into(),
                    )
                })?;
                validate_upstream(upstream, "upstream_mcp")?;
                validate_store(store, "store.root")?;
                let mut seen = BTreeMap::new();
                validate_tools(&self.tools, "", &mut seen)
            }
        }
    }
}

fn validate_legacy_contexts(contexts: &[ContextConfig]) -> Result<()> {
    let mut seen = BTreeMap::new();
    for ctx in contexts {
        let upstream = ctx.upstream_mcp.as_deref().ok_or_else(|| {
            GatewayError::ConfigRejected(format!(
                "contexts[{}].upstream_mcp is required without `servers`",
                ctx.name
            ))
        })?;
        validate_upstream(upstream, &format!("contexts[{}].upstream_mcp", ctx.name))?;
        let tools = match &ctx.tools {
            ContextTools::Legacy(tools) => tools,
            ContextTools::Hub(_) => {
                return Err(GatewayError::ConfigRejected(format!(
                    "contexts[{}] uses hub tool references without `servers`",
                    ctx.name
                )))
            }
        };
        validate_tools(tools, &ctx.name, &mut seen)?;
    }
    Ok(())
}

fn validate_hub(
    servers: &[ServerConfig],
    contexts: &[ContextConfig],
    brokers: Option<&BTreeMap<String, BrokerConfig>>,
) -> Result<()> {
    use std::collections::BTreeSet;

    if servers.is_empty() {
        return Err(GatewayError::ConfigRejected("`servers` is empty".into()));
    }
    if let Some(brokers) = brokers {
        validate_brokers(brokers)?;
    }

    let mut server_names = BTreeSet::new();
    for server in servers {
        if server.name.trim().is_empty() {
            return Err(GatewayError::ConfigRejected(
                "a server has an empty `name`".into(),
            ));
        }
        if !crate::policy::valid_server_name(&server.name) {
            return Err(GatewayError::ConfigRejected(format!(
                "server name `{}` must start with a lowercase letter or digit and contain only lowercase letters, digits, `-` or `_`",
                server.name
            )));
        }
        if is_reserved_server(&server.name) {
            return Err(GatewayError::ConfigRejected(format!(
                "reserved server name `{}`",
                server.name
            )));
        }
        if !server_names.insert(server.name.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "duplicate server name `{}`",
                server.name
            )));
        }
        validate_upstream(&server.url, &format!("servers[{}].url", server.name))?;
        if server
            .bearer_token
            .as_deref()
            .is_some_and(|token| token.trim().is_empty())
        {
            return Err(GatewayError::ConfigRejected(format!(
                "servers[{}].bearer_token is empty",
                server.name
            )));
        }
        let credential_modes = usize::from(server.bearer_token.is_some())
            + usize::from(server.credential.is_some())
            + usize::from(server.oauth.is_some());
        if credential_modes > 1 {
            if server.bearer_token.is_some()
                && server.credential.is_some()
                && server.oauth.is_none()
            {
                return Err(GatewayError::ConfigRejected(format!(
                    "servers[{}] declares both `credential` and `bearer_token` — \
                     exactly one credential source per server",
                    server.name
                )));
            }
            return Err(GatewayError::ConfigRejected(format!(
                "servers[{}] declares competing credential modes (`bearer_token`, \
                 `credential`, `oauth`) — exactly one is allowed",
                server.name
            )));
        }
        if let Some(credential) = &server.credential {
            validate_server_credential(&server.name, credential, brokers)?;
        }
        if let Some(oauth) = &server.oauth {
            validate_upstream_oauth(&server.name, oauth, brokers)?;
        }
    }

    let mut pairs: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut exposed: BTreeMap<String, String> = BTreeMap::new();
    let mut mismatched_names = Vec::new();
    for ctx in contexts {
        if ctx.upstream_mcp.is_some() {
            return Err(GatewayError::ConfigRejected(format!(
                "contexts[{}].upstream_mcp cannot mix with `servers`",
                ctx.name
            )));
        }
        let tools = match &ctx.tools {
            ContextTools::Hub(tools) => tools,
            ContextTools::Legacy(tools) if tools.is_empty() => continue,
            ContextTools::Legacy(_) => {
                return Err(GatewayError::ConfigRejected(format!(
                    "contexts[{}] must reference hub tools as {{server, tool, access}}",
                    ctx.name
                )))
            }
        };
        for (declared, tool) in tools {
            if declared.trim().is_empty() {
                return Err(GatewayError::ConfigRejected(
                    "empty tool name in `tools`".into(),
                ));
            }
            if !server_names.contains(tool.server.as_str()) {
                return Err(GatewayError::ConfigRejected(format!(
                    "contexts[{}] tool `{declared}` references unknown server `{}`",
                    ctx.name, tool.server
                )));
            }
            if tool.tool.trim().is_empty() {
                return Err(GatewayError::ConfigRejected(format!(
                    "contexts[{}] tool `{declared}` has an empty upstream tool name",
                    ctx.name
                )));
            }

            let pair = (tool.server.clone(), tool.tool.clone());
            if let Some(other_ctx) = pairs.get(&pair) {
                if other_ctx != &ctx.name {
                    return Err(GatewayError::ConfigRejected(format!(
                        "ambiguous context route: server `{}` tool `{}` is granted by contexts `{other_ctx}` and `{}`",
                        tool.server, tool.tool, ctx.name
                    )));
                }
            } else {
                pairs.insert(pair, ctx.name.clone());
            }

            let flat = format!(
                "{}__{}",
                tool.server,
                crate::policy::action_name(&tool.tool)
            );
            let shown = format!("{}:{declared} ({}:{})", ctx.name, tool.server, tool.tool);
            if let Some(other) = exposed.insert(flat.clone(), shown.clone()) {
                return Err(GatewayError::ConfigRejected(format!(
                    "exposed-name collision `{flat}` between `{other}` and `{shown}`"
                )));
            }
            if declared != &flat {
                mismatched_names.push((ctx.name.as_str(), declared.as_str(), flat));
            }
        }
    }
    if let Some((ctx, declared, expected)) = mismatched_names.first() {
        return Err(GatewayError::ConfigRejected(format!(
            "contexts[{ctx}] declares exposed name `{declared}`, expected `{expected}` from its server/tool reference"
        )));
    }
    Ok(())
}

fn is_reserved_server(name: &str) -> bool {
    matches!(name, "journal" | "gateway" | "briefing" | "ethos")
}

/// The declared brokers: names follow the server-id charset, addresses
/// are HTTP(S) with plaintext HTTP bounded to loopback (the explicit
/// dev/demo mode), mounts are single path segments and the vault-token
/// environment variable is a plausible name. All fail-closed.
fn validate_brokers(brokers: &BTreeMap<String, BrokerConfig>) -> Result<()> {
    if brokers.is_empty() {
        return Err(GatewayError::ConfigRejected(
            "`credential_brokers` is empty".into(),
        ));
    }
    for (name, broker) in brokers {
        if !crate::policy::valid_server_name(name) {
            return Err(GatewayError::ConfigRejected(format!(
                "credential broker name `{name}` must start with a lowercase letter or digit \
                 and contain only lowercase letters, digits, `-` or `_`"
            )));
        }
        let BrokerConfig::VaultKv2 {
            address,
            mount,
            auth,
        } = broker;
        let at = format!("credential_brokers[{name}]");
        validate_upstream(address, &format!("{at}.address"))?;
        if address.starts_with("http://") && !is_loopback_http(address) {
            return Err(GatewayError::ConfigRejected(format!(
                "`{at}.address` uses plaintext http off loopback — a remote vault requires TLS"
            )));
        }
        if mount.trim().is_empty()
            || mount.contains('/')
            || !mount
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            return Err(GatewayError::ConfigRejected(format!(
                "`{at}.mount` must be one non-empty path segment \
                 (letters, digits, `-`, `_`, `.`)"
            )));
        }
        let BrokerAuthConfig::TokenEnv { env } = auth;
        let mut chars = env.chars();
        let valid_env = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid_env {
            return Err(GatewayError::ConfigRejected(format!(
                "`{at}.auth.env` is not a valid environment variable name"
            )));
        }
    }
    Ok(())
}

/// One server's brokered reference: the broker must be declared, the
/// path must be clean KV segments, the field must be a plain name.
fn validate_server_credential(
    server: &str,
    credential: &CredentialRef,
    brokers: Option<&BTreeMap<String, BrokerConfig>>,
) -> Result<()> {
    let at = format!("servers[{server}].credential");
    validate_credential_ref(&at, credential, brokers)
}

fn validate_credential_ref(
    at: &str,
    credential: &CredentialRef,
    brokers: Option<&BTreeMap<String, BrokerConfig>>,
) -> Result<()> {
    if !brokers.is_some_and(|brokers| brokers.contains_key(&credential.broker)) {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}` references unknown credential broker `{}`",
            credential.broker
        )));
    }
    let path_ok = !credential.path.is_empty()
        && credential.path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        });
    if !path_ok {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.path` must be `/`-separated non-empty segments \
             (letters, digits, `-`, `_`, `.`; no `.`/`..`)"
        )));
    }
    let field_ok = !credential.field.is_empty()
        && credential
            .field
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !field_ok {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.field` must be a non-empty plain field name \
             (letters, digits, `-`, `_`, `.`)"
        )));
    }
    Ok(())
}

fn validate_upstream_oauth(
    server: &str,
    oauth: &UpstreamOAuthConfig,
    brokers: Option<&BTreeMap<String, BrokerConfig>>,
) -> Result<()> {
    let at = format!("servers[{server}].oauth");
    for (field, url) in [
        ("auth_url", oauth.auth_url.as_str()),
        ("token_url", oauth.token_url.as_str()),
        ("redirect_uri", oauth.redirect_uri.as_str()),
    ] {
        validate_upstream(url, &format!("{at}.{field}"))?;
        if url.starts_with("http://") && !is_loopback_http(url) {
            return Err(GatewayError::ConfigRejected(format!(
                "`{at}.{field}` uses plaintext http off loopback — OAuth requires TLS"
            )));
        }
    }
    if oauth.client_id.trim().is_empty() {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.client_id` is empty"
        )));
    }
    if oauth.scopes.is_empty()
        || oauth.scopes.iter().any(|scope| scope.trim().is_empty())
        || oauth
            .scopes
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != oauth.scopes.len()
    {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.scopes` must contain distinct, non-empty scopes"
        )));
    }
    validate_credential_ref(
        &format!("{at}.client_secret"),
        &oauth.client_secret,
        brokers,
    )?;
    validate_credential_ref(&format!("{at}.token_vault"), &oauth.token_vault, brokers)?;
    if oauth.client_secret == oauth.token_vault {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.token_vault` must not alias the client-secret field"
        )));
    }
    Ok(())
}

/// Is this `http://` URL bounded to loopback? (`127.*`, `localhost`,
/// `[::1]` — the explicitly allowed dev/demo hosts.)
fn is_loopback_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host_port = rest.split('/').next().unwrap_or_default();
    let host = if let Some(bracketed) = host_port.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or_default()
    } else {
        host_port
            .rsplit_once(':')
            .map_or(host_port, |(host, _)| host)
    };
    host == "localhost"
        || host == "::1"
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback())
}

fn mono_only() -> GatewayError {
    GatewayError::ConfigRejected(
        "this path needs the mono config shape (`upstream_mcp`/`store`), not `contexts`".into(),
    )
}

/// The `as:` stanza, fail-closed (lot G3): an explicit http(s) issuer
/// with plaintext bounded to loopback (the vault-broker rule), strictly
/// positive lifetimes, a non-empty key-file path, and allowlist
/// extensions held to the same transport bar as the issuer.
fn validate_as(oauth_as: &AsConfig) -> Result<()> {
    validate_upstream(&oauth_as.issuer, "as.issuer")?;
    if oauth_as.issuer.starts_with("http://") && !is_loopback_http(&oauth_as.issuer) {
        return Err(GatewayError::ConfigRejected(
            "`as.issuer` uses plaintext http off loopback — a public authorization \
             server requires TLS"
                .into(),
        ));
    }
    if oauth_as.key_file.as_os_str().is_empty() {
        return Err(GatewayError::ConfigRejected(
            "`as.key_file` is empty".into(),
        ));
    }
    if oauth_as.access_ttl_secs == 0 {
        return Err(GatewayError::ConfigRejected(
            "`as.access_ttl_secs` must be strictly positive".into(),
        ));
    }
    if oauth_as.refresh_ttl_secs == 0 {
        return Err(GatewayError::ConfigRejected(
            "`as.refresh_ttl_secs` must be strictly positive".into(),
        ));
    }
    for entry in &oauth_as.redirect_allowlist {
        validate_upstream(entry, "as.redirect_allowlist[]")?;
        if entry.starts_with("http://") && !is_loopback_http(entry) {
            return Err(GatewayError::ConfigRejected(format!(
                "`as.redirect_allowlist` entry `{entry}` uses plaintext http off \
                 loopback — a remote callback requires TLS (loopback stays open for CLIs)"
            )));
        }
    }
    Ok(())
}

fn validate_llm(llm: &LlmConfig) -> Result<()> {
    validate_upstream(&llm.upstream, "llm.upstream")?;
    if llm.api_key.trim().is_empty() {
        return Err(GatewayError::ConfigRejected(
            "`llm.api_key` is empty".into(),
        ));
    }
    if llm.model.trim().is_empty() {
        return Err(GatewayError::ConfigRejected("`llm.model` is empty".into()));
    }
    Ok(())
}

fn validate_upstream(url: &str, at: &str) -> Result<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}` must be an http(s) URL, got `{url}`"
        )));
    }
    Ok(())
}

fn validate_store(store: &StoreConfig, at: &str) -> Result<()> {
    match store {
        StoreConfig::Fs { root } => {
            if root.as_os_str().is_empty() {
                return Err(GatewayError::ConfigRejected(format!("`{at}` is empty")));
            }
        }
        StoreConfig::Remote {
            url, tenant, did, ..
        } => validate_remote_target(url, tenant, did, at)?,
        StoreConfig::Replicated {
            root,
            url,
            tenant,
            did,
            ..
        } => {
            if root.as_os_str().is_empty() {
                return Err(GatewayError::ConfigRejected(format!(
                    "`{at}.root` is empty"
                )));
            }
            validate_remote_target(url, tenant, did, at)?;
        }
        StoreConfig::S3 { .. } => {}
    }
    Ok(())
}

/// The remote target of the wire A.2: an http(s) base, a tenant of the
/// A.1 grammar, a literal DID (fail-closed at config time — an unusable
/// store is rejected outright, never guessed at).
fn validate_remote_target(url: &str, tenant: &str, did: &str, at: &str) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.url` must be an http(s) URL, got `{url}`"
        )));
    }
    let tenant_ok = (3..=32).contains(&tenant.len())
        && tenant
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && tenant.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit());
    if !tenant_ok {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.tenant` is not a wire A.1 tenant name"
        )));
    }
    if !did.starts_with("did:aithos:") {
        return Err(GatewayError::ConfigRejected(format!(
            "`{at}.did` must be a literal did:aithos:… identifier"
        )));
    }
    Ok(())
}

/// The tool-name prefixes reserved for the gateway's own native tools
/// (`journal` since lot C2, `briefing` since lot K, `ethos` since lot
/// G6; mirrors HUB-MCP §5): no tool map may name a tool `<prefix>`,
/// `<prefix>.*` or `<prefix>__*` — the names belong to the platform,
/// in the mono shape too.
const RESERVED_PREFIXES: [&str; 3] = ["journal", "briefing", "ethos"];

/// Register a tool map into the shared flattened-action namespace.
/// Mandate actions flatten dots to underscores; two tools that flatten
/// identically would silently share one grant (and, across contexts, one
/// route) — refuse. `prefix` is the context name, empty for mono.
fn validate_tools(
    tools: &ToolMap,
    prefix: &str,
    seen: &mut BTreeMap<String, String>,
) -> Result<()> {
    for tool in tools.keys() {
        if tool.trim().is_empty() {
            return Err(GatewayError::ConfigRejected(
                "empty tool name in `tools`".into(),
            ));
        }
        for reserved in RESERVED_PREFIXES {
            if tool == reserved
                || tool.starts_with(&format!("{reserved}."))
                || tool.starts_with(&format!("{reserved}__"))
            {
                return Err(GatewayError::ConfigRejected(format!(
                    "tool `{tool}`: the `{reserved}` prefix is reserved for the \
                     gateway's native tools"
                )));
            }
        }
        let shown = if prefix.is_empty() {
            tool.clone()
        } else {
            format!("{prefix}:{tool}")
        };
        if let Some(other) = seen.insert(crate::policy::action_name(tool), shown.clone()) {
            return Err(GatewayError::ConfigRejected(format!(
                "tools `{other}` and `{shown}` collide once mapped to a mandate action"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "\
listen: 127.0.0.1:4870
upstream_mcp: http://127.0.0.1:4124/mcp
store:
  kind: fs
  root: /var/lib/aithos
tools:
  user.read: read
  user.update: write
";

    const MULTI: &str = "\
listen: 127.0.0.1:4870
contexts:
  - name: company-brand
    upstream_mcp: http://127.0.0.1:5001/mcp
    store:
      kind: fs
      root: /var/lib/aithos/brand
    tools:
      brand.read: read
      brand.update: write
  - name: ui-designer
    upstream_mcp: http://127.0.0.1:5002/mcp
    store:
      kind: fs
      root: /var/lib/aithos/figma
    tools:
      figma.read: read
      figma.update: write
journal:
  store:
    kind: fs
    root: /var/lib/aithos/journal
";

    const HUB: &str = "\
listen: 127.0.0.1:4870
servers:
  - name: github
    transport: http
    url: https://mcp.github.example/mcp
contexts:
  - name: customer-support
    store:
      kind: fs
      root: /var/lib/aithos/support
    tools:
      github__issues_list:
        server: github
        tool: issues.list
        access: read
  - name: engineering
    store:
      kind: fs
      root: /var/lib/aithos/engineering
    tools:
      github__pulls_list:
        server: github
        tool: pulls.list
        access: read
journal:
  store:
    kind: fs
    root: /var/lib/aithos/journal
";

    #[test]
    fn parses_a_valid_config() {
        let cfg = GatewayConfig::from_yaml(GOOD).unwrap();
        assert_eq!(cfg.tools.get("user.read"), Some(&ToolAccess::Read));
        assert_eq!(cfg.tools.get("user.update"), Some(&ToolAccess::Write));
        assert!(cfg.contexts.is_none());
        assert_eq!(cfg.mono_upstream().unwrap(), "http://127.0.0.1:4124/mcp");
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let text = format!("{GOOD}surprise: true\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn unknown_access_level_is_rejected() {
        let text = GOOD.replace("read\n", "admin\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn non_http_upstream_is_rejected() {
        let text = GOOD.replace("http://127.0.0.1:4124/mcp", "ftp://x");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn tools_colliding_after_action_mapping_are_rejected() {
        // "user.read" and "user_read" would share one mandate action.
        let text = format!("{GOOD}  user_read: read\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    // ------------------------------------------------- multi-context (v2)

    #[test]
    fn parses_a_valid_multi_config() {
        let cfg = GatewayConfig::from_yaml(MULTI).unwrap();
        let contexts = cfg.contexts.as_ref().unwrap();
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].name, "company-brand");
        assert_eq!(
            contexts[0].legacy_tools().unwrap().get("brand.read"),
            Some(&ToolAccess::Read)
        );
        assert!(cfg.journal.is_some());
        // The mono accessors refuse: this config has no mono half.
        assert!(matches!(
            cfg.mono_upstream(),
            Err(GatewayError::ConfigRejected(_))
        ));
        assert!(matches!(
            cfg.mono_store(),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn mono_fields_mixed_with_contexts_are_rejected() {
        let text = format!("{MULTI}upstream_mcp: http://127.0.0.1:4124/mcp\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn contexts_without_journal_are_rejected() {
        let text = MULTI.replace(
            "journal:\n  store:\n    kind: fs\n    root: /var/lib/aithos/journal\n",
            "",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn journal_without_contexts_is_rejected() {
        let text = format!("{GOOD}journal:\n  store:\n    kind: fs\n    root: /tmp/j\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn cross_context_tool_collision_is_rejected() {
        // "brand_read" in ui-designer flattens like company-brand's
        // "brand.read": the route would be ambiguous.
        let text = MULTI.replace("figma.read: read", "brand_read: read");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn reserved_journal_prefix_is_rejected_in_context_maps() {
        for reserved in ["journal.export: read", "journal: read", "journal__x: read"] {
            let text = MULTI.replace("figma.read: read", reserved);
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
                ),
                "must reserve: {reserved}"
            );
        }
    }

    #[test]
    fn reserved_journal_prefix_is_rejected_in_the_mono_map_too() {
        let text = format!("{GOOD}  journal.read: read\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
        ));
    }

    #[test]
    fn a_journal_like_name_outside_the_prefix_family_is_fine() {
        // `journaling.read` shares letters, not the reserved namespace.
        let text = MULTI.replace("figma.read: read", "journaling.read: read");
        assert!(GatewayConfig::from_yaml(&text).is_ok());
    }

    #[test]
    fn reserved_briefing_prefix_is_rejected_in_every_tool_map() {
        // Lot K: `briefing` joins `journal` in the platform namespace.
        for reserved in ["briefing.read: read", "briefing: read", "briefing__x: read"] {
            let text = MULTI.replace("figma.read: read", reserved);
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
                ),
                "must reserve: {reserved}"
            );
        }
        let mono = format!("{GOOD}  briefing.read: read\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&mono),
            Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
        ));
        // `briefings.read` shares letters, not the reserved namespace.
        let neighbour = MULTI.replace("figma.read: read", "briefings.read: read");
        assert!(GatewayConfig::from_yaml(&neighbour).is_ok());
    }

    #[test]
    fn reserved_ethos_prefix_is_rejected_in_every_tool_map() {
        // Lot G6: `ethos` joins `journal` and `briefing` in the
        // platform namespace — the data-reading tools belong to the hub.
        for reserved in ["ethos.read: read", "ethos: read", "ethos__x: read"] {
            let text = MULTI.replace("figma.read: read", reserved);
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
                ),
                "must reserve: {reserved}"
            );
        }
        let mono = format!(
            "{GOOD}  ethos.read: read
"
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&mono),
            Err(GatewayError::ConfigRejected(m)) if m.contains("reserved")
        ));
        // `ethoses.read` shares letters, not the reserved namespace.
        let neighbour = MULTI.replace("figma.read: read", "ethoses.read: read");
        assert!(GatewayConfig::from_yaml(&neighbour).is_ok());
    }

    #[test]
    fn duplicate_context_names_are_rejected() {
        let text = MULTI.replace("name: ui-designer", "name: company-brand");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn context_with_non_http_upstream_is_rejected() {
        let text = MULTI.replace("http://127.0.0.1:5002/mcp", "ftp://x");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    // ------------------------------------------------ governed hub (v3 / H1)

    #[test]
    fn parses_a_valid_hub_config() {
        let cfg = GatewayConfig::from_yaml(HUB).unwrap();
        assert!(cfg.is_hub());
        let servers = cfg.servers.as_ref().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "github");
        assert_eq!(servers[0].transport, ServerTransport::Http);
        assert_eq!(servers[0].url, "https://mcp.github.example/mcp");
        let contexts = cfg.contexts.as_ref().unwrap();
        let ContextTools::Hub(tools) = &contexts[0].tools else {
            panic!("hub tool references")
        };
        let tool = tools.get("github__issues_list").unwrap();
        assert_eq!(tool.server, "github");
        assert_eq!(tool.tool, "issues.list");
        assert_eq!(tool.access, ToolAccess::Read);
    }

    #[test]
    fn hub_tool_grant_decisions_parse_and_default_safely() {
        // Explicit decision: a read can be declared denied.
        let text = HUB.replace(
            "        access: read",
            "        access: read\n        granted: false",
        );
        let cfg = GatewayConfig::from_yaml(&text).unwrap();
        let ContextTools::Hub(tools) = &cfg.contexts.as_ref().unwrap()[0].tools else {
            panic!("hub tools")
        };
        assert!(!tools.get("github__issues_list").unwrap().is_granted());
        // Default: reads granted (writes deny by default symmetrically).
        let cfg = GatewayConfig::from_yaml(HUB).unwrap();
        let ContextTools::Hub(tools) = &cfg.contexts.as_ref().unwrap()[0].tools else {
            panic!("hub tools")
        };
        let tool = tools.get("github__issues_list").unwrap();
        assert!(tool.granted.is_none() && tool.is_granted());
    }

    #[test]
    fn servers_require_the_complete_hub_shape() {
        let text = "listen: 127.0.0.1:4870\nservers:\n  - name: github\n    transport: http\n    url: https://example.test/mcp\n";
        assert!(matches!(
            GatewayConfig::from_yaml(text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("complete hub shape")
        ));
    }

    #[test]
    fn hub_and_legacy_context_upstreams_cannot_mix() {
        let text = HUB.replace(
            "  - name: customer-support\n",
            "  - name: customer-support\n    upstream_mcp: https://legacy.example/mcp\n",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("cannot mix")
        ));
    }

    #[test]
    fn hub_contexts_must_use_referenced_tools() {
        let text = HUB.replace(
            "      github__issues_list:\n        server: github\n        tool: issues.list\n        access: read",
            "      github__issues_list: read",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("must reference hub tools")
        ));
    }

    #[test]
    fn unknown_server_references_are_rejected() {
        let text = HUB.replace("server: github", "server: missing");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("unknown server `missing`")
        ));
    }

    #[test]
    fn declared_exposed_name_must_match_the_server_tool_reference() {
        let text = HUB.replace("github__issues_list:", "arbitrary_alias:");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("expected `github__issues_list`")
        ));
    }

    #[test]
    fn one_upstream_tool_cannot_route_to_two_contexts() {
        let text = HUB.replace("tool: pulls.list", "tool: issues.list");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("ambiguous context route")
        ));
    }

    #[test]
    fn flattened_hub_tool_collisions_are_rejected() {
        let text = HUB.replace(
            "      github__pulls_list:\n        server: github\n        tool: pulls.list\n        access: read",
            "      github__issues_list_alias:\n        server: github\n        tool: issues_list\n        access: read",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("exposed-name collision `github__issues_list`")
        ));
    }

    #[test]
    fn cross_server_flattened_collisions_are_detected_without_banning_double_underscore() {
        let text = HUB
            .replace(
                "  - name: github\n    transport: http\n    url: https://mcp.github.example/mcp",
                "  - name: a\n    transport: http\n    url: https://a.example/mcp\n  - name: a__b\n    transport: http\n    url: https://ab.example/mcp",
            )
            .replace("server: github\n        tool: issues.list", "server: a\n        tool: b__c")
            .replace("server: github\n        tool: pulls.list", "server: a__b\n        tool: c");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("exposed-name collision `a__b__c`")
        ));
    }

    #[test]
    fn reserved_and_duplicate_server_names_are_rejected() {
        for name in ["journal", "gateway", "briefing", "ethos"] {
            let text = HUB.replace("name: github", &format!("name: {name}"));
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(m)) if m.contains("reserved server name")
            ));
        }
        let text = HUB.replace(
            "  - name: github\n    transport: http\n    url: https://mcp.github.example/mcp",
            "  - name: github\n    transport: http\n    url: https://one.example/mcp\n  - name: github\n    transport: http\n    url: https://two.example/mcp",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("duplicate server name")
        ));
    }

    #[test]
    fn unsafe_server_names_are_rejected_but_double_underscore_remains_valid() {
        for name in ["GitHub", "a/b", ".", "-github"] {
            let text = HUB.replace("name: github", &format!("name: {name}"));
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(m)) if m.contains("server name")
            ));
        }
        let text = HUB
            .replace("name: github", "name: git__hub")
            .replace("server: github", "server: git__hub")
            .replace("github__issues_list", "git__hub__issues_list")
            .replace("github__pulls_list", "git__hub__pulls_list");
        assert!(GatewayConfig::from_yaml(&text).is_ok());
    }

    #[test]
    fn hub_server_transport_and_urls_fail_closed() {
        for text in [
            HUB.replace("transport: http", "transport: stdio"),
            HUB.replace("https://mcp.github.example/mcp", "stdio://github"),
        ] {
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(_))
            ));
        }
    }

    #[test]
    fn hub_bearer_is_optional_but_never_empty() {
        let with_bearer = HUB.replace(
            "    url: https://mcp.github.example/mcp",
            "    url: https://mcp.github.example/mcp\n    bearer_token: secret-token",
        );
        let cfg = GatewayConfig::from_yaml(&with_bearer).unwrap();
        assert_eq!(
            cfg.servers.as_ref().unwrap()[0].bearer_token.as_deref(),
            Some("secret-token")
        );
        let empty = HUB.replace(
            "    url: https://mcp.github.example/mcp",
            "    url: https://mcp.github.example/mcp\n    bearer_token: '  '",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&empty),
            Err(GatewayError::ConfigRejected(message)) if message.contains("bearer_token is empty")
        ));
    }

    #[test]
    fn hub_nested_unknown_fields_are_rejected() {
        for text in [
            HUB.replace(
                "    url: https://mcp.github.example/mcp",
                "    url: https://mcp.github.example/mcp\n    surprise: true",
            ),
            HUB.replace(
                "        access: read",
                "        access: read\n        surprise: true",
            ),
        ] {
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(_))
            ));
        }
    }

    // -------------------------------------- credential brokers (vault slice)

    const HUB_VAULT: &str = "\
listen: 127.0.0.1:4870
credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
servers:
  - name: github
    transport: http
    url: https://mcp.github.example/mcp
    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
contexts:
  - name: customer-support
    store:
      kind: fs
      root: /var/lib/aithos/support
    tools:
      github__issues_list:
        server: github
        tool: issues.list
        access: read
journal:
  store:
    kind: fs
    root: /var/lib/aithos/journal
";

    #[test]
    fn parses_a_hub_config_with_a_brokered_credential() {
        let cfg = GatewayConfig::from_yaml(HUB_VAULT).unwrap();
        let brokers = cfg.credential_brokers.as_ref().unwrap();
        let BrokerConfig::VaultKv2 {
            address,
            mount,
            auth,
        } = brokers.get("enterprise").unwrap();
        assert_eq!(address, "http://127.0.0.1:8200");
        assert_eq!(mount, "secret");
        let BrokerAuthConfig::TokenEnv { env } = auth;
        assert_eq!(env, "AITHOS_VAULT_TOKEN");
        let server = &cfg.servers.as_ref().unwrap()[0];
        let credential = server.credential.as_ref().unwrap();
        assert_eq!(credential.broker, "enterprise");
        assert_eq!(credential.path, "aithos/mcp/github");
        assert_eq!(credential.field, "token");
        assert!(server.bearer_token.is_none());
    }

    #[test]
    fn credential_and_bearer_token_together_are_rejected() {
        let text = HUB_VAULT.replace(
            "    url: https://mcp.github.example/mcp\n",
            "    url: https://mcp.github.example/mcp\n    bearer_token: inline-secret\n",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m))
                if m.contains("both `credential` and `bearer_token`")
                    && m.contains("one credential source")
        ));
    }

    fn oauth_hub() -> String {
        HUB_VAULT.replace(
            "    credential:\n      broker: enterprise\n      path: aithos/mcp/github\n      field: token\n",
            "    oauth:\n      auth_url: https://accounts.example/authorize\n      token_url: https://accounts.example/token\n      client_id: owner-client\n      client_secret:\n        broker: enterprise\n        path: aithos/oauth/client\n        field: client_secret\n      scopes: [resource.read]\n      redirect_uri: https://gateway.example/oauth/callback\n      token_vault:\n        broker: enterprise\n        path: aithos/oauth/github\n        field: state\n",
        )
    }

    #[test]
    fn parses_strict_secretless_upstream_oauth() {
        let text = oauth_hub();
        let cfg = GatewayConfig::from_yaml(&text).unwrap();
        let oauth = cfg.servers.as_ref().unwrap()[0].oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id, "owner-client");
        assert_eq!(oauth.scopes, ["resource.read"]);
        assert_eq!(oauth.client_secret.path, "aithos/oauth/client");
        assert_eq!(oauth.token_vault.path, "aithos/oauth/github");
        assert!(cfg.servers.as_ref().unwrap()[0].credential.is_none());
    }

    #[test]
    fn upstream_oauth_is_exclusive_and_requires_tls_off_loopback() {
        let with_bearer = oauth_hub().replace(
            "    oauth:\n",
            "    bearer_token: inline-secret\n    oauth:\n",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&with_bearer),
            Err(GatewayError::ConfigRejected(message))
                if message.contains("competing credential modes")
        ));
        let plaintext = oauth_hub().replace(
            "https://accounts.example/authorize",
            "http://accounts.example/authorize",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&plaintext),
            Err(GatewayError::ConfigRejected(message)) if message.contains("requires TLS")
        ));
    }

    #[test]
    fn upstream_oauth_references_and_scopes_fail_closed() {
        for text in [
            oauth_hub().replace("scopes: [resource.read]", "scopes: []"),
            oauth_hub().replace(
                "scopes: [resource.read]",
                "scopes: [resource.read, resource.read]",
            ),
            oauth_hub().replace("path: aithos/oauth/github", "path: ../escape"),
            oauth_hub()
                .replace("path: aithos/oauth/github", "path: aithos/oauth/client")
                .replace("field: state", "field: client_secret"),
        ] {
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(_))
            ));
        }
    }

    #[test]
    fn a_credential_referencing_an_unknown_broker_is_rejected() {
        let text = HUB_VAULT.replace("broker: enterprise", "broker: missing");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m))
                if m.contains("unknown credential broker `missing`")
        ));
    }

    #[test]
    fn a_credential_without_any_declared_broker_is_rejected() {
        let text = HUB_VAULT.replace(
            "credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
",
            "",
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m))
                if m.contains("unknown credential broker")
        ));
    }

    #[test]
    fn credential_brokers_outside_the_hub_shape_are_rejected() {
        let broker_block = "credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
";
        for base in [GOOD.to_owned(), MULTI.to_owned()] {
            let text = format!("{base}{broker_block}");
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(m)) if m.contains("hub shape")
                ),
                "brokers must require the hub shape"
            );
        }
    }

    #[test]
    fn an_empty_broker_map_is_rejected() {
        let text = HUB_VAULT
            .replace(
                "credential_brokers:
  enterprise:
    kind: vault-kv2
    address: http://127.0.0.1:8200
    mount: secret
    auth:
      kind: token-env
      env: AITHOS_VAULT_TOKEN
",
                "credential_brokers: {}
",
            )
            .replace(
                "    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
",
                "",
            );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("`credential_brokers` is empty")
        ));
    }

    #[test]
    fn a_plaintext_broker_address_off_loopback_is_rejected() {
        let text = HUB_VAULT.replace("http://127.0.0.1:8200", "http://vault.internal:8200");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("requires TLS")
        ));
        for allowed in [
            "http://127.0.0.1:8200",
            "http://localhost:8200",
            "http://[::1]:8200",
            "https://vault.internal:8200",
        ] {
            let text = HUB_VAULT.replace("http://127.0.0.1:8200", allowed);
            assert!(
                GatewayConfig::from_yaml(&text).is_ok(),
                "must accept: {allowed}"
            );
        }
    }

    #[test]
    fn broker_and_credential_shapes_fail_closed() {
        for (broken, what) in [
            ("mount: secret", "mount: se/cret"),
            ("mount: secret", "mount: ''"),
            ("env: AITHOS_VAULT_TOKEN", "env: '9BAD NAME'"),
            ("path: aithos/mcp/github", "path: aithos//github"),
            ("path: aithos/mcp/github", "path: ../escape"),
            ("path: aithos/mcp/github", "path: ''"),
            ("field: token", "field: ''"),
            ("field: token", "field: to ken"),
        ] {
            let text = HUB_VAULT.replace(broken, what);
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(_))
                ),
                "must reject: {what}"
            );
        }
    }

    #[test]
    fn unknown_fields_in_broker_and_credential_blocks_are_rejected() {
        for (from, to) in [
            (
                "    mount: secret\n",
                "    mount: secret\n    surprise: true\n",
            ),
            (
                "      env: AITHOS_VAULT_TOKEN\n",
                "      env: AITHOS_VAULT_TOKEN\n      surprise: true\n",
            ),
            (
                "      field: token\n",
                "      field: token\n      surprise: true\n",
            ),
        ] {
            let text = HUB_VAULT.replace(from, to);
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(_))
                ),
                "must reject unknown field in: {to}"
            );
        }
    }

    #[test]
    fn broker_kinds_and_auth_kinds_fail_closed() {
        for (from, to) in [
            ("kind: vault-kv2", "kind: vault-kv1"),
            ("kind: token-env", "kind: token-inline"),
        ] {
            let text = HUB_VAULT.replace(from, to);
            assert!(matches!(
                GatewayConfig::from_yaml(&text),
                Err(GatewayError::ConfigRejected(_))
            ));
        }
    }

    #[test]
    fn broker_names_follow_the_server_charset() {
        for bad in ["Enterprise", "ent/prise", "-enterprise"] {
            let text = HUB_VAULT
                .replace("  enterprise:", &format!("  {bad}:"))
                .replace("broker: enterprise", &format!("broker: {bad}"));
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(m)) if m.contains("broker name")
                ),
                "must reject broker name: {bad}"
            );
        }
    }

    #[test]
    fn an_unused_broker_is_allowed() {
        // Declaring a broker no server references yet is configuration,
        // not ambiguity — the reference direction is what is validated.
        let text = HUB_VAULT.replace(
            "    credential:
      broker: enterprise
      path: aithos/mcp/github
      field: token
",
            "",
        );
        assert!(GatewayConfig::from_yaml(&text).is_ok());
    }

    // -------------------------------------------------------- llm (Phase C)

    #[test]
    fn llm_parses_with_the_multi_shape() {
        let text = format!(
            "{MULTI}llm:\n  upstream: https://api.example.com/v1/chat/completions\n  api_key: sk-test\n  model: gpt-4o-mini\n"
        );
        let cfg = GatewayConfig::from_yaml(&text).unwrap();
        let llm = cfg.llm.as_ref().unwrap();
        assert_eq!(llm.model, "gpt-4o-mini");
        assert_eq!(llm.provider, "openai-compat", "default provider tag");
    }

    #[test]
    fn llm_with_the_mono_shape_is_rejected() {
        let text = format!(
            "{GOOD}llm:\n  upstream: https://api.example.com/v1/chat/completions\n  api_key: sk-test\n  model: gpt-4o-mini\n"
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn llm_with_an_empty_credential_or_model_is_rejected() {
        for broken in [
            "llm:\n  upstream: https://api.example.com/v1\n  api_key: \"\"\n  model: gpt-4o-mini\n",
            "llm:\n  upstream: https://api.example.com/v1\n  api_key: sk-test\n  model: \"\"\n",
            "llm:\n  upstream: ftp://x\n  api_key: sk-test\n  model: gpt-4o-mini\n",
        ] {
            let text = format!("{MULTI}{broken}");
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(_))
                ),
                "must reject: {broken}"
            );
        }
    }

    // ------------------------------------------------- oauth as (lot G3)

    const AS_STANZA: &str = "as:\n  issuer: http://127.0.0.1:4870\n";

    #[test]
    fn as_parses_with_defaults_on_the_multi_shape() {
        let text = format!("{MULTI}{AS_STANZA}");
        let cfg = GatewayConfig::from_yaml(&text).unwrap();
        let oauth_as = cfg.oauth_as.as_ref().unwrap();
        assert_eq!(oauth_as.issuer, "http://127.0.0.1:4870");
        assert_eq!(oauth_as.key_file, PathBuf::from("as.key"));
        assert_eq!(oauth_as.access_ttl_secs, 3_600, "decided 2026-07-17");
        assert_eq!(oauth_as.refresh_ttl_secs, 7 * 86_400, "decided 2026-07-17");
        assert!(oauth_as.redirect_allowlist.is_empty());
    }

    #[test]
    fn as_on_the_mono_shape_is_rejected_pedagogically() {
        let text = format!("{GOOD}{AS_STANZA}");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("multi-context shape")
        ));
    }

    #[test]
    fn as_issuer_off_loopback_requires_tls() {
        let text = format!("{MULTI}as:\n  issuer: http://as.example.com\n");
        assert!(matches!(
            GatewayConfig::from_yaml(&text),
            Err(GatewayError::ConfigRejected(m)) if m.contains("requires TLS")
        ));
        for allowed in [
            "http://127.0.0.1:4870",
            "http://localhost:4870",
            "http://[::1]:4870",
            "https://acme.mcp.aithos.fr",
        ] {
            let text = format!("{MULTI}as:\n  issuer: {allowed}\n");
            assert!(
                GatewayConfig::from_yaml(&text).is_ok(),
                "must accept issuer: {allowed}"
            );
        }
    }

    #[test]
    fn as_unknown_fields_and_zero_ttls_are_rejected() {
        for broken in [
            "as:\n  issuer: http://127.0.0.1:4870\n  surprise: true\n",
            "as:\n  issuer: http://127.0.0.1:4870\n  access_ttl_secs: 0\n",
            "as:\n  issuer: http://127.0.0.1:4870\n  refresh_ttl_secs: 0\n",
            "as:\n  issuer: http://127.0.0.1:4870\n  key_file: ''\n",
            "as:\n  issuer: ftp://x\n",
        ] {
            let text = format!("{MULTI}{broken}");
            assert!(
                matches!(
                    GatewayConfig::from_yaml(&text),
                    Err(GatewayError::ConfigRejected(_))
                ),
                "must reject: {broken}"
            );
        }
    }

    #[test]
    fn as_allowlist_extensions_hold_the_transport_bar() {
        let ok = format!(
            "{MULTI}as:\n  issuer: http://127.0.0.1:4870\n  redirect_allowlist:\n    - https://ci.example/cb\n    - http://127.0.0.1:9999/cb\n"
        );
        assert!(GatewayConfig::from_yaml(&ok).is_ok());
        let off_loopback = format!(
            "{MULTI}as:\n  issuer: http://127.0.0.1:4870\n  redirect_allowlist:\n    - http://ci.example/cb\n"
        );
        assert!(matches!(
            GatewayConfig::from_yaml(&off_loopback),
            Err(GatewayError::ConfigRejected(m)) if m.contains("requires TLS")
        ));
    }

    #[test]
    fn mono_without_store_or_upstream_is_rejected() {
        assert!(matches!(
            GatewayConfig::from_yaml("listen: 127.0.0.1:4870\n"),
            Err(GatewayError::ConfigRejected(_))
        ));
        assert!(matches!(
            GatewayConfig::from_yaml(
                "listen: 127.0.0.1:4870\nupstream_mcp: http://127.0.0.1:4124/mcp\n"
            ),
            Err(GatewayError::ConfigRejected(_))
        ));
    }
}

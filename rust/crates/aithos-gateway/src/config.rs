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
        if let Some(credential) = &server.credential {
            if server.bearer_token.is_some() {
                return Err(GatewayError::ConfigRejected(format!(
                    "servers[{}] declares both `credential` and `bearer_token` — \
                     exactly one credential source per server",
                    server.name
                )));
            }
            validate_server_credential(&server.name, credential, brokers)?;
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
    matches!(name, "journal" | "gateway")
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
    if let StoreConfig::Fs { root } = store {
        if root.as_os_str().is_empty() {
            return Err(GatewayError::ConfigRejected(format!("`{at}` is empty")));
        }
    }
    Ok(())
}

/// The tool-name prefix reserved for the gateway's own journal tools
/// (lot C2, mirrors HUB-MCP §5): no tool map may name a tool `journal`,
/// `journal.*` or `journal__*` — the name belongs to the platform, in
/// the mono shape too.
const RESERVED_PREFIX: &str = "journal";

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
        if tool == RESERVED_PREFIX
            || tool.starts_with(&format!("{RESERVED_PREFIX}."))
            || tool.starts_with(&format!("{RESERVED_PREFIX}__"))
        {
            return Err(GatewayError::ConfigRejected(format!(
                "tool `{tool}`: the `{RESERVED_PREFIX}` prefix is reserved for the \
                 gateway's native journal tools"
            )));
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
        for name in ["journal", "gateway"] {
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

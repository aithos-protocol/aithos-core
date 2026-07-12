//! Onboarding configuration: what the enterprise declares before plugging
//! an agent in. Parsed fail-closed: unknown keys, unknown access levels or
//! an unusable store are rejected outright, never guessed at.
//!
//! Two shapes, never mixed (v2, decisions of 2026-07-10):
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
//! Semantics: `read` tools are covered by the granted read-only mandate;
//! `write` tools are known but *not* granted (so refusals name the tool
//! precisely); anything absent from every tool map is denied by default.
//! Routing is by tool name, so a tool name that flattens identically in
//! two contexts would be ambiguous — rejected at parse time.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::{GatewayError, Result};

/// Access level the enterprise assigns to an MCP tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
    /// The real MCP server this context's calls relay to.
    pub upstream_mcp: String,
    /// The tool whitelist that maps calls onto this context.
    #[serde(default)]
    pub tools: ToolMap,
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

    fn validate(&self) -> Result<()> {
        if self.listen.trim().is_empty() {
            return Err(GatewayError::ConfigRejected("`listen` is empty".into()));
        }
        match (&self.contexts, &self.journal) {
            // -------------------------------------------- multi-context v2
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
                let mut seen = BTreeMap::new();
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
                    validate_upstream(
                        &ctx.upstream_mcp,
                        &format!("contexts[{}].upstream_mcp", ctx.name),
                    )?;
                    validate_store(&ctx.store, &format!("contexts[{}].store", ctx.name))?;
                    // ONE flattened action namespace across ALL contexts:
                    // routing is by tool name, so a cross-context collision
                    // would make the route (and the grant) ambiguous.
                    validate_tools(&ctx.tools, &ctx.name, &mut seen)?;
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
        assert_eq!(contexts[0].tools.get("brand.read"), Some(&ToolAccess::Read));
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

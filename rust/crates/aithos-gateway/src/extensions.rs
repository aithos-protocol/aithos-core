//! Compiled extension packs (GSE-0): declarative manifests and routing,
//! with no dynamic code loading and no external effect implementation.
//!
//! An extension is a synthetic server behind the existing [`McpRouter`](crate::proxy_mcp::McpRouter).
//! Its id and exposed tool names share the hub namespace, while one explicit
//! context remains the mandate and proof destination. Visibility is therefore
//! recomputed from the live context mandate; enabling a pack is never enough.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{ContextTools, GatewayConfig, ToolAccess};
use crate::core_bridge::manifest_tool_pin;
use crate::hub::{ApprovedManifest, ApprovedTool, MANIFEST_VERSION};
use crate::policy::{hub_exposed_name, valid_server_name};
use crate::{GatewayError, Result};

pub const EXTENSION_MANIFEST_VERSION: &str = "aithos-extension-pack-v1";
pub const GMAIL_EXTENSION_ID: &str = "aithos-gmail";
pub const GMAIL_SEND_TOOL: &str = "send_guarded";

/// One opt-in pack binding. `enabled` defaults to false (default-deny), and
/// `context` is mandatory so every decision and audit entry has one Ethos.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionConfig {
    pub id: String,
    #[serde(default)]
    pub enabled: bool,
    pub context: String,
}

/// Risk is declared by the compiled pack, not selected by the agent or YAML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionRiskClass {
    Read,
    Write,
    Binding,
}

/// OAuth is declarative in GSE-0. No client, token provider or network flow is
/// constructed from this metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OAuthRequirement {
    pub provider: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtensionToolManifest {
    pub name: String,
    pub exposed_name: String,
    pub description: String,
    pub input_schema: Value,
    pub pin_sha256: String,
    pub risk_class: ExtensionRiskClass,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtensionManifest {
    pub version: String,
    pub id: String,
    pub tools: Vec<ExtensionToolManifest>,
    pub oauth: Option<OAuthRequirement>,
}

impl ExtensionManifest {
    pub fn validate(&self) -> Result<()> {
        if self.version != EXTENSION_MANIFEST_VERSION {
            return Err(GatewayError::ConfigRejected(format!(
                "extension `{}` has unsupported manifest version `{}`",
                self.id, self.version
            )));
        }
        if !valid_server_name(&self.id) {
            return Err(GatewayError::ConfigRejected(format!(
                "extension id `{}` is not a valid synthetic server id",
                self.id
            )));
        }
        if self.tools.is_empty() {
            return Err(GatewayError::ConfigRejected(format!(
                "extension `{}` has no tools",
                self.id
            )));
        }
        let mut names = BTreeSet::new();
        let mut exposed = BTreeSet::new();
        for tool in &self.tools {
            if tool.name.trim().is_empty() || !names.insert(tool.name.as_str()) {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension `{}` has an empty or duplicate tool name `{}`",
                    self.id, tool.name
                )));
            }
            let expected = hub_exposed_name(&self.id, &tool.name);
            if tool.exposed_name != expected || !exposed.insert(tool.exposed_name.as_str()) {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension `{}` has exposed-name collision or mismatch `{}`",
                    self.id, tool.exposed_name
                )));
            }
            if !tool.input_schema.is_object() {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension tool `{}` input schema is not an object",
                    tool.exposed_name
                )));
            }
            let expected_pin = manifest_tool_pin(
                &tool.name,
                Some(&tool.description),
                &tool.input_schema,
            )?;
            if tool.pin_sha256 != expected_pin {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension tool `{}` pin does not match name, description and schema",
                    tool.exposed_name
                )));
            }
            if tool.constraints.iter().any(|value| value.trim().is_empty()) {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension tool `{}` has an empty declarative constraint",
                    tool.exposed_name
                )));
            }
        }
        if let Some(oauth) = &self.oauth {
            if oauth.provider.trim().is_empty()
                || oauth.scopes.is_empty()
                || oauth.scopes.iter().any(|scope| scope.trim().is_empty())
            {
                return Err(GatewayError::ConfigRejected(format!(
                    "extension `{}` has invalid declarative OAuth requirements",
                    self.id
                )));
            }
        }
        Ok(())
    }

    /// The existing owner enrollment surface can provision the synthetic
    /// connector without teaching core or bundle about extensions.
    pub fn approved_manifest(&self) -> Result<ApprovedManifest> {
        self.validate()?;
        let tools = self
            .tools
            .iter()
            .map(|tool| ApprovedTool {
                name: tool.name.clone(),
                exposed_name: tool.exposed_name.clone(),
                description: Some(tool.description.clone()),
                input_schema: tool.input_schema.clone(),
                pin_sha256: tool.pin_sha256.clone(),
                risk_class: match tool.risk_class {
                    ExtensionRiskClass::Read => ToolAccess::Read,
                    ExtensionRiskClass::Write | ExtensionRiskClass::Binding => ToolAccess::Write,
                },
                granted: Some(true),
                bounds: Vec::new(),
            })
            .collect();
        Ok(ApprovedManifest {
            version: MANIFEST_VERSION.to_owned(),
            server: self.id.clone(),
            tools,
        })
    }
}

/// Object-safe compiled pack seam. GSE-0 ships no effectful implementation.
pub trait ExtensionPack: Send + Sync {
    fn manifest(&self) -> &ExtensionManifest;

    fn invoke<'a>(
        &'a self,
        tool: &'a str,
        args: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
}

struct GmailContractPack {
    manifest: ExtensionManifest,
}

impl ExtensionPack for GmailContractPack {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn invoke<'a>(
        &'a self,
        tool: &'a str,
        _args: &'a Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
        Box::pin(async move {
            Err(GatewayError::ExtensionUnavailable(format!(
                "`{GMAIL_EXTENSION_ID}__{tool}` has no effect implementation in GSE-0"
            )))
        })
    }
}

fn gmail_manifest() -> Result<ExtensionManifest> {
    let name = GMAIL_SEND_TOOL.to_owned();
    let description = "Request a guarded plain-text Gmail send. GSE-0 pins only the contract; no message can be sent.".to_owned();
    let input_schema = json!({
        "type": "object",
        "properties": {
            "to": { "type": "array", "items": { "type": "string" } },
            "subject": { "type": "string" },
            "text_body": { "type": "string" },
            "cc": { "type": "array", "items": { "type": "string" } },
            "bcc": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["to", "subject", "text_body"],
        "additionalProperties": false
    });
    let pin_sha256 = manifest_tool_pin(&name, Some(&description), &input_schema)?;
    let manifest = ExtensionManifest {
        version: EXTENSION_MANIFEST_VERSION.to_owned(),
        id: GMAIL_EXTENSION_ID.to_owned(),
        tools: vec![ExtensionToolManifest {
            exposed_name: hub_exposed_name(GMAIL_EXTENSION_ID, &name),
            name,
            description,
            input_schema,
            pin_sha256,
            risk_class: ExtensionRiskClass::Write,
            constraints: vec![
                "gateway_mandate".to_owned(),
                "log_before_effect".to_owned(),
                "gmail_policy_before_effect".to_owned(),
            ],
        }],
        oauth: Some(OAuthRequirement {
            provider: "google-workspace".to_owned(),
            scopes: vec!["https://www.googleapis.com/auth/gmail.send".to_owned()],
        }),
    };
    manifest.validate()?;
    Ok(manifest)
}

fn compiled_pack(id: &str) -> Result<Arc<dyn ExtensionPack>> {
    match id {
        GMAIL_EXTENSION_ID => Ok(Arc::new(GmailContractPack {
            manifest: gmail_manifest()?,
        })),
        other => Err(GatewayError::ConfigRejected(format!(
            "unknown compiled extension `{other}`"
        ))),
    }
}

pub fn compiled_manifest(id: &str) -> Result<ExtensionManifest> {
    Ok(compiled_pack(id)?.manifest().clone())
}

pub fn is_compiled_extension_id(id: &str) -> bool {
    id == GMAIL_EXTENSION_ID
}

#[derive(Clone)]
pub struct ExtensionRoute {
    context: String,
    raw_tool: String,
    descriptor: Value,
    manifest_version: String,
    pack: Arc<dyn ExtensionPack>,
}

impl ExtensionRoute {
    pub fn context(&self) -> &str {
        &self.context
    }

    pub fn pack_id(&self) -> &str {
        &self.pack.manifest().id
    }

    pub fn raw_tool(&self) -> &str {
        &self.raw_tool
    }

    pub fn descriptor(&self) -> Value {
        self.descriptor.clone()
    }

    pub fn manifest_version(&self) -> &str {
        &self.manifest_version
    }

    pub async fn invoke(&self, args: &Value) -> Result<Value> {
        self.pack.invoke(&self.raw_tool, args).await
    }
}

/// Immutable runtime registry. Mandate state deliberately does not live here;
/// the router asks the live [`Runner`](crate::core_bridge::Runner) on every
/// list and call, which makes revocation hot without rebuilding this registry.
#[derive(Clone, Default)]
pub struct ExtensionRegistry {
    routes: BTreeMap<String, ExtensionRoute>,
}

impl ExtensionRegistry {
    pub fn from_config(cfg: &GatewayConfig) -> Result<Self> {
        validate_gateway_extensions(cfg)?;
        let mut routes = BTreeMap::new();
        for extension in cfg.extensions.as_deref().unwrap_or_default() {
            if !extension.enabled {
                continue;
            }
            let pack = compiled_pack(&extension.id)?;
            for tool in &pack.manifest().tools {
                let descriptor = json!({
                    "name": tool.exposed_name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema,
                });
                let route = ExtensionRoute {
                    context: extension.context.clone(),
                    raw_tool: tool.name.clone(),
                    descriptor,
                    manifest_version: pack.manifest().version.clone(),
                    pack: Arc::clone(&pack),
                };
                if routes.insert(tool.exposed_name.clone(), route).is_some() {
                    return Err(GatewayError::ConfigRejected(format!(
                        "extension exposed-name collision `{}`",
                        tool.exposed_name
                    )));
                }
            }
        }
        Ok(Self { routes })
    }

    pub fn resolve(&self, exposed: &str) -> Option<&ExtensionRoute> {
        self.routes.get(exposed)
    }

    pub fn routes(&self) -> impl Iterator<Item = (&str, &ExtensionRoute)> {
        self.routes.iter().map(|(name, route)| (name.as_str(), route))
    }
}

/// Strict config validation, reused during YAML parsing and registry build.
pub fn validate_gateway_extensions(cfg: &GatewayConfig) -> Result<()> {
    let Some(extensions) = cfg.extensions.as_deref() else {
        return Ok(());
    };
    if extensions.is_empty() {
        return Err(GatewayError::ConfigRejected("`extensions` is empty".into()));
    }
    let contexts = cfg.contexts.as_ref().ok_or_else(|| {
        GatewayError::ConfigRejected(
            "`extensions` needs the multi-context shape so every pack has a proof context".into(),
        )
    })?;
    let context_names: BTreeSet<&str> = contexts.iter().map(|ctx| ctx.name.as_str()).collect();
    let mut mapped_names: BTreeSet<&str> = BTreeSet::new();
    for context in contexts {
        match &context.tools {
            ContextTools::Legacy(tools) => mapped_names.extend(tools.keys().map(String::as_str)),
            ContextTools::Hub(tools) => mapped_names.extend(tools.keys().map(String::as_str)),
        }
    }
    let server_names: BTreeSet<&str> = cfg
        .servers
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|server| server.name.as_str())
        .collect();
    let mut ids = BTreeSet::new();
    let mut exposed = BTreeSet::new();
    for extension in extensions {
        if !ids.insert(extension.id.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "duplicate extension id `{}`",
                extension.id
            )));
        }
        if server_names.contains(extension.id.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "extension id collision `{}` with an external server",
                extension.id
            )));
        }
        if !context_names.contains(extension.context.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "extension `{}` references unknown context `{}`",
                extension.id, extension.context
            )));
        }
        let manifest = compiled_manifest(&extension.id)?;
        if extension.enabled {
            for tool in manifest.tools {
                if mapped_names.contains(tool.exposed_name.as_str())
                    || !exposed.insert(tool.exposed_name.clone())
                {
                    return Err(GatewayError::ConfigRejected(format!(
                        "extension exposed-name collision `{}`",
                        tool.exposed_name
                    )));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_manifest_declares_contract_without_an_oauth_client() {
        let manifest = compiled_manifest(GMAIL_EXTENSION_ID).unwrap();
        assert_eq!(manifest.version, EXTENSION_MANIFEST_VERSION);
        assert_eq!(manifest.tools[0].risk_class, ExtensionRiskClass::Write);
        assert_eq!(
            manifest.oauth.unwrap().scopes,
            ["https://www.googleapis.com/auth/gmail.send"]
        );
        assert_eq!(manifest.tools[0].exposed_name, "aithos-gmail__send_guarded");
    }

    #[tokio::test]
    async fn gmail_contract_pack_has_no_effect_implementation() {
        let pack = compiled_pack(GMAIL_EXTENSION_ID).unwrap();
        let err = pack.invoke(GMAIL_SEND_TOOL, &json!({})).await.unwrap_err();
        assert!(matches!(err, GatewayError::ExtensionUnavailable(_)));
    }
}

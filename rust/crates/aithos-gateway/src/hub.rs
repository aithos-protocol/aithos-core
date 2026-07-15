//! Governed MCP hub enrollment artifacts (Phase H).
//!
//! Discovery is untrusted input: it captures exactly the upstream tool
//! name, optional description and input schema, then pins their JCS hash.
//! Approval is a separate owner gesture assigning every discovered tool
//! a v1 risk class. Persistence and mandate issuance stay behind
//! `core_bridge`, the only door to the trust engine.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::ToolAccess;
use crate::policy::{hub_exposed_name, valid_server_name};
use crate::proxy_mcp::Upstream;
use crate::{GatewayError, Result};

pub const MANIFEST_VERSION: &str = "aithos-mcp-manifest-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedManifest {
    pub version: String,
    pub server: String,
    pub tools: Vec<ProposedTool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProposedTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
    pub pin_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedManifest {
    pub version: String,
    pub server: String,
    pub tools: Vec<ApprovedTool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedTool {
    pub name: String,
    pub exposed_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
    pub pin_sha256: String,
    pub risk_class: ToolAccess,
    /// The owner's grant decision, separate from the risk class (lot W):
    /// what kind of power a tool is never decides whether THIS agent may
    /// use it. Absent in pre-W sealed manifests, where the historic
    /// default applies — reads granted, writes denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub granted: Option<bool>,
}

impl ApprovedTool {
    /// The effective decision: explicit if recorded, historic default
    /// (`read` → granted, `write` → denied) for pre-W manifests.
    pub fn is_granted(&self) -> bool {
        self.granted.unwrap_or(self.risk_class == ToolAccess::Read)
    }
}

/// One owner approval: risk class plus the explicit grant decision.
/// `granted: None` keeps the historic safe default — an approval that
/// names only a class grants reads and denies writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolApproval {
    pub risk_class: ToolAccess,
    pub granted: Option<bool>,
}

impl ToolApproval {
    /// Class only — the safe defaults decide the grant.
    pub fn class(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: None,
        }
    }

    pub fn granted(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: Some(true),
        }
    }

    pub fn denied(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: Some(false),
        }
    }

    pub fn is_granted(&self) -> bool {
        self.granted.unwrap_or(self.risk_class == ToolAccess::Read)
    }
}

/// Capture an upstream `tools/list` into a deterministic proposed
/// manifest. Nothing is approved or stored by discovery itself.
pub async fn discover_server<U: Upstream>(server: &str, upstream: &U) -> Result<ProposedManifest> {
    validate_server(server)?;
    let response = upstream
        .forward(json!({
            "jsonrpc": "2.0",
            "id": "aithos-discover",
            "method": "tools/list"
        }))
        .await?;
    if let Some(error) = response.get("error") {
        return Err(GatewayError::UpstreamFailed(format!(
            "tools/list returned an error: {error}"
        )));
    }
    let advertised = response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayError::UpstreamFailed("tools/list has no result.tools array".into())
        })?;
    if advertised.is_empty() {
        return Err(GatewayError::UpstreamFailed(
            "tools/list advertised no tools".into(),
        ));
    }

    let mut names = BTreeSet::new();
    let mut tools = Vec::with_capacity(advertised.len());
    for tool in advertised {
        let object = tool.as_object().ok_or_else(|| {
            GatewayError::UpstreamFailed("tools/list contains a non-object tool".into())
        })?;
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| GatewayError::UpstreamFailed("a tool has no non-empty name".into()))?;
        if !names.insert(name.to_owned()) {
            return Err(GatewayError::UpstreamFailed(format!(
                "tools/list repeats tool `{name}`"
            )));
        }
        let description = match object.get("description") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(value.clone()),
            Some(_) => {
                return Err(GatewayError::UpstreamFailed(format!(
                    "tool `{name}` has a non-string description"
                )))
            }
        };
        let input_schema = object.get("inputSchema").cloned().ok_or_else(|| {
            GatewayError::UpstreamFailed(format!("tool `{name}` has no inputSchema"))
        })?;
        if !input_schema.is_object() {
            return Err(GatewayError::UpstreamFailed(format!(
                "tool `{name}` inputSchema is not an object"
            )));
        }
        let pin_sha256 =
            crate::core_bridge::manifest_tool_pin(name, description.as_deref(), &input_schema)?;
        tools.push(ProposedTool {
            name: name.to_owned(),
            description,
            input_schema,
            pin_sha256,
        });
    }
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ProposedManifest {
        version: MANIFEST_VERSION.to_owned(),
        server: server.to_owned(),
        tools,
    })
}

/// Owner approval: every discovered tool needs one explicit class and
/// no undiscovered name may be smuggled into the approval map. The
/// grant decision is recorded explicitly in the sealed manifest, even
/// when it came from the safe defaults.
pub fn approve_manifest(
    proposed: &ProposedManifest,
    approvals: &BTreeMap<String, ToolApproval>,
) -> Result<ApprovedManifest> {
    validate_proposed(proposed)?;
    let discovered: BTreeSet<&str> = proposed
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    if let Some(unknown) = approvals
        .keys()
        .find(|name| !discovered.contains(name.as_str()))
    {
        return Err(GatewayError::ConfigRejected(format!(
            "approval names undiscovered tool `{unknown}`"
        )));
    }
    let mut exposed = BTreeSet::new();
    let mut tools = Vec::with_capacity(proposed.tools.len());
    for tool in &proposed.tools {
        let approval = approvals.get(&tool.name).copied().ok_or_else(|| {
            GatewayError::ConfigRejected(format!(
                "owner approval missing risk class for `{}`",
                tool.name
            ))
        })?;
        let exposed_name = hub_exposed_name(&proposed.server, &tool.name);
        if !exposed.insert(exposed_name.clone()) {
            return Err(GatewayError::ConfigRejected(format!(
                "approved manifest has exposed-name collision `{exposed_name}`"
            )));
        }
        tools.push(ApprovedTool {
            name: tool.name.clone(),
            exposed_name,
            description: tool.description.clone(),
            input_schema: tool.input_schema.clone(),
            pin_sha256: tool.pin_sha256.clone(),
            risk_class: approval.risk_class,
            granted: Some(approval.is_granted()),
        });
    }
    let approved = ApprovedManifest {
        version: proposed.version.clone(),
        server: proposed.server.clone(),
        tools,
    };
    validate_approved(&approved)?;
    Ok(approved)
}

pub fn validate_approved(manifest: &ApprovedManifest) -> Result<()> {
    if manifest.version != MANIFEST_VERSION {
        return Err(GatewayError::ConfigRejected(format!(
            "unsupported manifest version `{}`",
            manifest.version
        )));
    }
    validate_server(&manifest.server)?;
    if manifest.tools.is_empty() {
        return Err(GatewayError::ConfigRejected(
            "approved manifest has no tools".into(),
        ));
    }
    let mut names = BTreeSet::new();
    let mut exposed = BTreeSet::new();
    for tool in &manifest.tools {
        if !names.insert(tool.name.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "approved manifest repeats tool `{}`",
                tool.name
            )));
        }
        let expected_exposed = hub_exposed_name(&manifest.server, &tool.name);
        if tool.exposed_name != expected_exposed {
            return Err(GatewayError::ConfigRejected(format!(
                "tool `{}` exposed name is `{}`, expected `{expected_exposed}`",
                tool.name, tool.exposed_name
            )));
        }
        if !exposed.insert(tool.exposed_name.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "approved manifest has exposed-name collision `{}`",
                tool.exposed_name
            )));
        }
        let expected_pin = crate::core_bridge::manifest_tool_pin(
            &tool.name,
            tool.description.as_deref(),
            &tool.input_schema,
        )?;
        if tool.pin_sha256 != expected_pin {
            return Err(GatewayError::ConfigRejected(format!(
                "tool `{}` pin does not match name, description and input schema",
                tool.name
            )));
        }
    }
    Ok(())
}

fn validate_proposed(manifest: &ProposedManifest) -> Result<()> {
    if manifest.version != MANIFEST_VERSION {
        return Err(GatewayError::ConfigRejected(format!(
            "unsupported manifest version `{}`",
            manifest.version
        )));
    }
    validate_server(&manifest.server)?;
    if manifest.tools.is_empty() {
        return Err(GatewayError::ConfigRejected(
            "proposed manifest has no tools".into(),
        ));
    }
    let mut names = BTreeSet::new();
    for tool in &manifest.tools {
        if !names.insert(tool.name.as_str()) {
            return Err(GatewayError::ConfigRejected(format!(
                "proposed manifest repeats tool `{}`",
                tool.name
            )));
        }
        let expected = crate::core_bridge::manifest_tool_pin(
            &tool.name,
            tool.description.as_deref(),
            &tool.input_schema,
        )?;
        if tool.pin_sha256 != expected {
            return Err(GatewayError::ConfigRejected(format!(
                "proposed tool `{}` pin is stale or forged",
                tool.name
            )));
        }
    }
    Ok(())
}

fn validate_server(server: &str) -> Result<()> {
    if !valid_server_name(server) || matches!(server, "journal" | "gateway") {
        return Err(GatewayError::ConfigRejected(format!(
            "invalid or reserved hub server name `{server}`"
        )));
    }
    Ok(())
}

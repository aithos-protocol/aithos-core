//! Policy engine: maps an incoming request onto an aithos `Op` and decides,
//! fail-closed. The map alone never authorises anything — it only names
//! things; authorisation is the mandate's job (via `core_bridge`).
//!
//! Division of labour:
//! - `Policy` (here, core-agnostic): tool name → op string, default-deny
//!   for anything unmapped.
//! - `core_bridge::authorize`: op string → `verify_op` against the mandate
//!   chain at T. The gateway relays only if *both* layers say yes.

use crate::config::{ToolAccess, ToolMap};
use crate::{GatewayError, Result};

/// The connector namespace for MCP tools in the mandate grammar.
/// One place to change if the core grammar evolves.
pub const MCP_CONNECTOR: &str = "mcp";

/// The action name a tool maps to in the mandate grammar. The grammar
/// splits `act.x.<connector>.<action>` at the LAST dot, so dotted MCP
/// tool names ("user.read") cannot be actions verbatim: dots become
/// underscores. The raw tool name still travels in the clear payload of
/// every logged act. Collisions ("user.read" vs "user_read") are
/// rejected at config time — never aliased silently.
pub fn action_name(tool: &str) -> String {
    tool.replace('.', "_")
}

/// The op string an MCP tool call maps to (`act.x.mcp.<action>`).
pub fn op_for_tool(tool: &str) -> String {
    format!("act.x.{MCP_CONNECTOR}.{}", action_name(tool))
}

/// The declared tool map, wrapped with fail-closed lookups.
#[derive(Debug, Clone)]
pub struct Policy {
    map: ToolMap,
}

impl Policy {
    pub fn new(map: ToolMap) -> Self {
        Self { map }
    }

    /// Access level for a tool. A tool outside the map is never relayed —
    /// that is the default-deny the feature demands.
    pub fn access_for(&self, tool: &str) -> Result<ToolAccess> {
        self.map
            .get(tool)
            .copied()
            .ok_or_else(|| GatewayError::ToolNotMapped(tool.to_string()))
    }

    /// Tools covered by the read-only mandate (deterministic order) —
    /// the onboarding input for mandate generation.
    pub fn read_tools(&self) -> impl Iterator<Item = &str> {
        self.map
            .iter()
            .filter(|(_, a)| **a == ToolAccess::Read)
            .map(|(t, _)| t.as_str())
    }

    /// Is this tool named by the map at all (read OR write)? The
    /// multi-context router resolves a call to a context with this —
    /// naming is not authorising: writes still fail at the mandate.
    pub fn is_mapped(&self, tool: &str) -> bool {
        self.map.contains_key(tool)
    }

    /// Every mapped tool name (deterministic order) — the aggregated
    /// `tools/list` surface.
    pub fn tools(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn policy() -> Policy {
        let mut m = BTreeMap::new();
        m.insert("user.read".into(), ToolAccess::Read);
        m.insert("user.update".into(), ToolAccess::Write);
        Policy::new(m)
    }

    #[test]
    fn mapped_tools_resolve() {
        assert_eq!(policy().access_for("user.read").unwrap(), ToolAccess::Read);
        assert_eq!(
            policy().access_for("user.update").unwrap(),
            ToolAccess::Write
        );
    }

    #[test]
    fn unmapped_tool_is_denied_by_default() {
        assert!(matches!(
            policy().access_for("user.delete"),
            Err(GatewayError::ToolNotMapped(t)) if t == "user.delete"
        ));
    }

    #[test]
    fn ops_follow_the_connector_grammar() {
        // Dots in tool names cannot survive into the action (the grammar
        // splits at the last dot) — they become underscores.
        assert_eq!(op_for_tool("user.read"), "act.x.mcp.user_read");
        assert_eq!(op_for_tool("search"), "act.x.mcp.search");
    }
}

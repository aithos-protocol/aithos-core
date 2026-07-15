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
    /// Owner-approved argument bounds (lot P): sealed policy on WHAT the
    /// granted tool may act on. Deliberately outside `pin_sha256` — the
    /// pin freezes the UPSTREAM's word (name, description, schema); the
    /// bounds are the OWNER's word, integrity-protected by the sealed
    /// manifest itself and changed only by re-enrollment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounds: Vec<ArgumentBound>,
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
#[derive(Debug, Clone, PartialEq)]
pub struct ToolApproval {
    pub risk_class: ToolAccess,
    pub granted: Option<bool>,
    pub bounds: Vec<ArgumentBound>,
}

impl ToolApproval {
    /// Class only — the safe defaults decide the grant.
    pub fn class(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: None,
            bounds: Vec::new(),
        }
    }

    pub fn granted(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: Some(true),
            bounds: Vec::new(),
        }
    }

    pub fn denied(risk_class: ToolAccess) -> Self {
        Self {
            risk_class,
            granted: Some(false),
            bounds: Vec::new(),
        }
    }

    pub fn with_bounds(mut self, bounds: Vec<ArgumentBound>) -> Self {
        self.bounds = bounds;
        self
    }

    pub fn is_granted(&self) -> bool {
        self.granted.unwrap_or(self.risk_class == ToolAccess::Read)
    }
}

// ------------------------------------------------- argument bounds (lot P)

/// One owner-approved rule over the arguments of a granted tool. Kept
/// deliberately deterministic and flat (top-level argument fields only,
/// v1): a whitelist of values (strings, or every element of an array —
/// which also bounds the sub-actions of a polymorphic tool), local
/// clock-face time slots, a forbidden or required field, a size cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArgumentBound {
    /// The field's value (or each element of its array) must belong to
    /// the approved set. An absent optional field passes.
    OneOf { field: String, values: Vec<String> },
    /// The field must be an RFC 3339-shaped datetime whose LOCAL clock
    /// face (the time as written — a 15:00 visit is 15:00 at the
    /// property, no timezone database in v1) falls on an approved day
    /// inside [from, to). Absent field = refusal: a slotted action
    /// without its datetime cannot be checked.
    TimeSlots {
        field: String,
        days: Vec<String>,
        from: String,
        to: String,
    },
    /// The field must not be present at all.
    Forbid { field: String },
    /// The field must be present and non-empty.
    Require { field: String },
    /// The field, when present, must be an array of at most `max` items.
    MaxItems { field: String, max: u64 },
}

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

impl ArgumentBound {
    pub fn field(&self) -> &str {
        match self {
            ArgumentBound::OneOf { field, .. }
            | ArgumentBound::TimeSlots { field, .. }
            | ArgumentBound::Forbid { field }
            | ArgumentBound::Require { field }
            | ArgumentBound::MaxItems { field, .. } => field,
        }
    }

    /// Well-formedness at approval time, fail-closed.
    pub fn validate(&self, tool: &str) -> Result<()> {
        let reject = |what: String| {
            Err(GatewayError::ConfigRejected(format!(
                "bound on `{tool}`: {what}"
            )))
        };
        if self.field().trim().is_empty() {
            return reject("empty field name".into());
        }
        match self {
            ArgumentBound::OneOf { values, .. } => {
                if values.is_empty() || values.iter().any(|value| value.trim().is_empty()) {
                    return reject("one_of needs a non-empty set of non-empty values".into());
                }
            }
            ArgumentBound::TimeSlots { days, from, to, .. } => {
                if days.is_empty()
                    || days
                        .iter()
                        .any(|day| !WEEKDAYS.contains(&day.to_ascii_lowercase().as_str()))
                {
                    return reject("time_slots needs weekday names".into());
                }
                let (Some(from), Some(to)) = (parse_hhmm(from), parse_hhmm(to)) else {
                    return reject("time_slots needs HH:MM boundaries".into());
                };
                if from >= to {
                    return reject("time_slots `from` must precede `to`".into());
                }
            }
            ArgumentBound::MaxItems { max, .. } => {
                if *max == 0 {
                    return reject("max_items needs max >= 1".into());
                }
            }
            ArgumentBound::Forbid { .. } | ArgumentBound::Require { .. } => {}
        }
        Ok(())
    }

    /// Evaluate one call's arguments against this bound. The refusal
    /// message is pedagogical by design (decision of 2026-07-15): it
    /// names the field, the offending values and the approved rule —
    /// exactly what the owner granted, sealed and logged, no secret.
    pub fn check(&self, raw_tool: &str, args: &Value) -> Result<()> {
        let violated = |what: String| {
            Err(GatewayError::BoundViolated(format!(
                "`{raw_tool}.{}` — {what}",
                self.field()
            )))
        };
        let value = args.get(self.field());
        match self {
            ArgumentBound::OneOf { field, values } => {
                let offenders: Vec<String> = match value {
                    None => return Ok(()),
                    Some(Value::String(one)) => {
                        if values.contains(one) {
                            return Ok(());
                        }
                        vec![one.clone()]
                    }
                    Some(Value::Array(items)) => {
                        let mut offenders = Vec::new();
                        for item in items {
                            match item.as_str() {
                                Some(text) if values.contains(&text.to_owned()) => {}
                                Some(text) => offenders.push(text.to_owned()),
                                None => {
                                    return violated(format!(
                                        "must be an array of strings, `{field}` holds another shape"
                                    ))
                                }
                            }
                        }
                        if offenders.is_empty() {
                            return Ok(());
                        }
                        offenders
                    }
                    Some(_) => {
                        return violated(format!(
                            "must be a string or an array of strings, `{field}` holds another shape"
                        ))
                    }
                };
                violated(format!(
                    "values [{}] outside the approved set [{}]",
                    offenders.join(", "),
                    values.join(", ")
                ))
            }
            ArgumentBound::TimeSlots { days, from, to, .. } => {
                let Some(Value::String(instant)) = value else {
                    return violated(
                        "a time_slots bound needs its RFC 3339 datetime argument".into(),
                    );
                };
                let Some((year, month, day, minutes)) = parse_clock_face(instant) else {
                    return violated(format!("`{instant}` is not an RFC 3339 datetime"));
                };
                let weekday = WEEKDAYS[weekday_index(year, month, day)];
                let from_minutes = parse_hhmm(from).expect("validated at approval");
                let to_minutes = parse_hhmm(to).expect("validated at approval");
                let day_ok = days
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(weekday));
                if day_ok && minutes >= from_minutes && minutes < to_minutes {
                    return Ok(());
                }
                violated(format!(
                    "`{instant}` ({weekday}) is outside the approved slots [{} {from}-{to}]",
                    days.join(", ")
                ))
            }
            ArgumentBound::Forbid { field } => match value {
                None => Ok(()),
                Some(_) => violated(format!("forbidden field `{field}` is present")),
            },
            ArgumentBound::Require { field } => {
                let empty = match value {
                    None => true,
                    Some(Value::String(text)) => text.trim().is_empty(),
                    Some(Value::Array(items)) => items.is_empty(),
                    Some(Value::Null) => true,
                    Some(_) => false,
                };
                if empty {
                    violated(format!("required field `{field}` is missing or empty"))
                } else {
                    Ok(())
                }
            }
            ArgumentBound::MaxItems { field, max } => match value {
                None => Ok(()),
                Some(Value::Array(items)) if items.len() as u64 <= *max => Ok(()),
                Some(Value::Array(items)) => violated(format!(
                    "{} items, at most {max} items on `{field}`",
                    items.len()
                )),
                Some(_) => violated(format!("must be an array, `{field}` holds another shape")),
            },
        }
    }
}

/// "HH:MM" → minutes since midnight, strictly.
fn parse_hhmm(text: &str) -> Option<u32> {
    let (hours, minutes) = text.split_once(':')?;
    if hours.len() != 2 || minutes.len() != 2 {
        return None;
    }
    let hours: u32 = hours.parse().ok()?;
    let minutes: u32 = minutes.parse().ok()?;
    (hours <= 23 && minutes <= 59).then_some(hours * 60 + minutes)
}

/// The LOCAL clock face of an RFC 3339-shaped datetime: (year, month,
/// day, minutes-since-midnight) exactly as written, offset ignored on
/// purpose (a visit slot is local time by nature — documented v1).
fn parse_clock_face(text: &str) -> Option<(i64, u32, u32, u32)> {
    let bytes = text.as_bytes();
    if bytes.len() < 16 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let slice = text.get(range)?;
        slice
            .chars()
            .all(|c| c.is_ascii_digit())
            .then(|| slice.parse().ok())?
    };
    let year = digits(0..4)? as i64;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    let minutes = parse_hhmm(text.get(11..16)?)?;
    (1..=12).contains(&month).then_some(())?;
    (1..=31).contains(&day).then_some(())?;
    Some((year, month, day, minutes))
}

/// 0 = monday … 6 = sunday (Howard Hinnant's days_from_civil).
fn weekday_index(year: i64, month: u32, day: u32) -> usize {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    // 1970-01-01 was a thursday: index 3 with monday = 0.
    (days + 3).rem_euclid(7) as usize
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
        let approval = approvals.get(&tool.name).cloned().ok_or_else(|| {
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
        if !approval.bounds.is_empty() && !approval.is_granted() {
            return Err(GatewayError::ConfigRejected(format!(
                "bounds on `{}` which the owner did not grant",
                tool.name
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
            bounds: approval.bounds.clone(),
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
        if !tool.bounds.is_empty() && !tool.is_granted() {
            return Err(GatewayError::ConfigRejected(format!(
                "bounds on `{}` which the owner did not grant",
                tool.name
            )));
        }
        for bound in &tool.bounds {
            bound.validate(&tool.name)?;
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
    if !valid_server_name(server) || matches!(server, "journal" | "gateway" | "briefing") {
        return Err(GatewayError::ConfigRejected(format!(
            "invalid or reserved hub server name `{server}`"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekdays_follow_the_civil_calendar() {
        assert_eq!(WEEKDAYS[weekday_index(2026, 7, 15)], "wednesday");
        assert_eq!(WEEKDAYS[weekday_index(2026, 7, 16)], "thursday");
        assert_eq!(WEEKDAYS[weekday_index(1970, 1, 1)], "thursday");
        assert_eq!(WEEKDAYS[weekday_index(2000, 3, 1)], "wednesday");
    }

    #[test]
    fn one_of_teaches_the_set_and_flags_every_offender() {
        let bound = ArgumentBound::OneOf {
            field: "to".into(),
            values: vec!["a@x".into(), "b@x".into()],
        };
        assert!(bound
            .check("send_email", &json!({ "to": ["a@x", "b@x"] }))
            .is_ok());
        assert!(
            bound.check("send_email", &json!({})).is_ok(),
            "absent optional passes"
        );
        let err = bound
            .check("send_email", &json!({ "to": ["a@x", "d@x", "e@x"] }))
            .unwrap_err();
        let shown = err.to_string();
        assert!(shown.contains("`send_email.to`"));
        assert!(shown.contains("d@x") && shown.contains("e@x"));
        assert!(shown.contains("a@x") && shown.contains("b@x"));
        // A scalar whitelist stays a scalar rule (sub-action case).
        let action = ArgumentBound::OneOf {
            field: "action".into(),
            values: vec!["comment".into()],
        };
        assert!(action
            .check("repo_admin", &json!({ "action": "comment" }))
            .is_ok());
        let err = action
            .check("repo_admin", &json!({ "action": "merge" }))
            .unwrap_err();
        assert!(err.to_string().contains("merge") && err.to_string().contains("comment"));
    }

    #[test]
    fn time_slots_read_the_clock_face_as_written() {
        let bound = ArgumentBound::TimeSlots {
            field: "start".into(),
            days: vec!["tuesday".into(), "thursday".into()],
            from: "14:00".into(),
            to: "18:00".into(),
        };
        assert!(bound
            .check(
                "create_event",
                &json!({ "start": "2026-07-16T15:00:00+02:00" })
            )
            .is_ok());
        let err = bound
            .check(
                "create_event",
                &json!({ "start": "2026-07-15T10:00:00+02:00" }),
            )
            .unwrap_err();
        let shown = err.to_string();
        assert!(
            shown.contains("wednesday") && shown.contains("14:00-18:00") || shown.contains("14:00")
        );
        assert!(shown.contains("2026-07-15T10:00:00+02:00"));
        // Boundary: 18:00 itself is outside [from, to).
        assert!(bound
            .check(
                "create_event",
                &json!({ "start": "2026-07-16T18:00:00+02:00" })
            )
            .is_err());
        // Absent or malformed datetimes refuse.
        assert!(bound.check("create_event", &json!({})).is_err());
        assert!(bound
            .check("create_event", &json!({ "start": "tomorrow" }))
            .is_err());
    }

    #[test]
    fn presence_and_size_rules_fail_closed() {
        let forbid = ArgumentBound::Forbid {
            field: "bcc".into(),
        };
        assert!(forbid.check("send_email", &json!({})).is_ok());
        let err = forbid
            .check("send_email", &json!({ "bcc": ["x@y"] }))
            .unwrap_err();
        assert!(err.to_string().contains("forbidden field `bcc`"));

        let require = ArgumentBound::Require {
            field: "subject".into(),
        };
        let err = require.check("send_email", &json!({})).unwrap_err();
        assert!(err.to_string().contains("required field `subject`"));
        assert!(require
            .check("send_email", &json!({ "subject": " " }))
            .is_err());
        assert!(require
            .check("send_email", &json!({ "subject": "s" }))
            .is_ok());

        let max = ArgumentBound::MaxItems {
            field: "to".into(),
            max: 3,
        };
        assert!(max
            .check("send_email", &json!({ "to": ["a", "b", "c"] }))
            .is_ok());
        let err = max
            .check("send_email", &json!({ "to": ["a", "b", "c", "d"] }))
            .unwrap_err();
        assert!(err.to_string().contains("at most 3 items on `to`"));
    }

    #[test]
    fn bound_wellformedness_is_checked_at_approval() {
        assert!(ArgumentBound::OneOf {
            field: "to".into(),
            values: vec![]
        }
        .validate("send_email")
        .is_err());
        assert!(ArgumentBound::TimeSlots {
            field: "start".into(),
            days: vec!["someday".into()],
            from: "14:00".into(),
            to: "18:00".into()
        }
        .validate("create_event")
        .is_err());
        assert!(ArgumentBound::TimeSlots {
            field: "start".into(),
            days: vec!["tuesday".into()],
            from: "18:00".into(),
            to: "14:00".into()
        }
        .validate("create_event")
        .is_err());
        assert!(ArgumentBound::MaxItems {
            field: "to".into(),
            max: 0
        }
        .validate("send_email")
        .is_err());
    }
}

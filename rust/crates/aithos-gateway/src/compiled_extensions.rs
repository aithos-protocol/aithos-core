//! Closed, in-process REST adapters for connector profiles.
//!
//! These adapters sit *behind* the gateway's normal resolve, mandate, bounds
//! and log-before-effect sequence.  They deliberately do not accept authority
//! material and must only be installed as an [`Upstream`] after that sequence.
//! They still validate their complete input shape and provider perimeter so a
//! wiring mistake cannot turn either adapter into a generic HTTP client.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use zeroize::Zeroize;

use crate::config::CompiledConnectorAdapter;
use crate::credentials::{CredentialBroker, CredentialRef, SecretValue};
use crate::hub::{ProposedManifest, ProposedTool, MANIFEST_VERSION};
use crate::proxy_mcp::Upstream;
use crate::upstream_oauth::UpstreamOAuthClient;
use crate::{GatewayError, Result};

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MAX_RESPONSE_BYTES: usize = 512 * 1024;
const HARD_MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SPREADSHEET_ID_BYTES: usize = 128;
const MAX_A1_RANGE_BYTES: usize = 256;
const MAX_APPROVER_BYTES: usize = 200;
const MAX_OUTBOX_RECORDS: usize = 1_024;
const MAX_APPROVAL_INBOX_ITEMS: usize = 100;

pub const SHEETS_READ_TOOL: &str = "read_range";
pub const SHEETS_WRITE_GUARDED_TOOL: &str = "values_update_guarded";
pub const GMAIL_SEND_GUARDED_TOOL: &str = "send_guarded";

/// Return the immutable catalogue a compiled profile must pin. This is local
/// catalogue material, not MCP discovery: activation can compare its digest
/// directly with the sealed profile without contacting any provider.
pub fn compiled_manifest(
    server: impl Into<String>,
    adapter: CompiledConnectorAdapter,
) -> Result<ProposedManifest> {
    let descriptor = match adapter {
        CompiledConnectorAdapter::GoogleSheetsRead => sheets_tool_descriptor(),
        CompiledConnectorAdapter::GoogleSheetsWriteGuarded => sheets_write_tool_descriptor(),
        CompiledConnectorAdapter::GmailSendGuarded => gmail_tool_descriptor(),
    };
    let name = descriptor
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| config_rejected("compiled extension catalogue is invalid"))?;
    let description = descriptor.get("description").and_then(Value::as_str);
    let input_schema = descriptor
        .get("inputSchema")
        .cloned()
        .filter(Value::is_object)
        .ok_or_else(|| config_rejected("compiled extension catalogue is invalid"))?;
    let pin_sha256 = crate::core_bridge::manifest_tool_pin(name, description, &input_schema)?;
    Ok(ProposedManifest {
        version: MANIFEST_VERSION.to_owned(),
        server: server.into(),
        tools: vec![ProposedTool {
            name: name.to_owned(),
            description: description.map(str::to_owned),
            input_schema,
            pin_sha256,
        }],
    })
}

/// A single type suitable for `DynamicUpstream::new` regardless of which
/// implemented compiled adapter the sealed profile selects.
pub enum CompiledExtensionUpstream {
    GoogleSheetsRead(GoogleSheetsReadUpstream),
    GoogleSheetsWriteGuarded(GoogleSheetsWriteGuardedUpstream),
    GmailSendGuarded(GmailSendGuardedUpstream),
}

impl CompiledExtensionUpstream {
    pub fn google_sheets_read(
        config: GoogleSheetsReadConfig,
        oauth: Arc<UpstreamOAuthClient>,
    ) -> Result<Self> {
        GoogleSheetsReadUpstream::new(config, oauth).map(Self::GoogleSheetsRead)
    }

    pub fn gmail_send_guarded(
        policy: GmailSendPolicy,
        oauth: Arc<UpstreamOAuthClient>,
    ) -> Result<Self> {
        GmailSendGuardedUpstream::new(policy, oauth).map(Self::GmailSendGuarded)
    }

    pub fn google_sheets_write_guarded(
        config: GoogleSheetsWriteConfig,
        oauth: Arc<UpstreamOAuthClient>,
    ) -> Result<Self> {
        GoogleSheetsWriteGuardedUpstream::new(config, oauth).map(Self::GoogleSheetsWriteGuarded)
    }

    pub fn gmail_send_guarded_durable(
        policy: GmailSendPolicy,
        oauth: Arc<UpstreamOAuthClient>,
        broker: Arc<dyn CredentialBroker>,
        reference: CredentialRef,
    ) -> Result<Self> {
        GmailSendGuardedUpstream::new_durable(policy, oauth, broker, reference)
            .map(Self::GmailSendGuarded)
    }

    pub async fn hydrate(&self) -> Result<()> {
        match self {
            Self::GmailSendGuarded(upstream) => upstream.hydrate().await,
            Self::GoogleSheetsRead(_) | Self::GoogleSheetsWriteGuarded(_) => Ok(()),
        }
    }

    pub fn gmail(&self) -> Option<&GmailSendGuardedUpstream> {
        match self {
            Self::GmailSendGuarded(upstream) => Some(upstream),
            Self::GoogleSheetsRead(_) | Self::GoogleSheetsWriteGuarded(_) => None,
        }
    }
}

impl Upstream for CompiledExtensionUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        match self {
            Self::GoogleSheetsRead(upstream) => upstream.forward(body).await,
            Self::GoogleSheetsWriteGuarded(upstream) => upstream.forward(body).await,
            Self::GmailSendGuarded(upstream) => upstream.forward(body).await,
        }
    }
}

/// A fixed spreadsheet/range perimeter. A range must match an entry exactly;
/// the adapter never attempts to infer containment from A1 notation.
#[derive(Debug, Clone)]
pub struct GoogleSheetsReadConfig {
    pub api_base_url: String,
    pub allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    pub max_response_bytes: usize,
}

impl GoogleSheetsReadConfig {
    pub fn new(
        api_base_url: impl Into<String>,
        allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    ) -> Self {
        Self {
            api_base_url: api_base_url.into(),
            allowed_ranges,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    fn validate(&self) -> Result<reqwest::Url> {
        let base = validated_api_base(&self.api_base_url, "Google Sheets")?;
        if self.allowed_ranges.is_empty()
            || self.max_response_bytes == 0
            || self.max_response_bytes > HARD_MAX_RESPONSE_BYTES
        {
            return Err(config_rejected("invalid Google Sheets extension bounds"));
        }
        for (spreadsheet, ranges) in &self.allowed_ranges {
            if !valid_spreadsheet_id(spreadsheet) || ranges.is_empty() {
                return Err(config_rejected("invalid Google Sheets allowlist"));
            }
            if ranges.iter().any(|range| !valid_a1_range(range)) {
                return Err(config_rejected("invalid Google Sheets range allowlist"));
            }
        }
        Ok(base)
    }
}

/// Local MCP-shaped upstream exposing only the fixed Sheets read operation.
pub struct GoogleSheetsReadUpstream {
    oauth: Arc<UpstreamOAuthClient>,
    http: reqwest::Client,
    api_base: reqwest::Url,
    allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    max_response_bytes: usize,
}

impl GoogleSheetsReadUpstream {
    pub fn new(config: GoogleSheetsReadConfig, oauth: Arc<UpstreamOAuthClient>) -> Result<Self> {
        let api_base = config.validate()?;
        Ok(Self {
            oauth,
            http: closed_http_client("Google Sheets")?,
            api_base,
            allowed_ranges: config.allowed_ranges,
            max_response_bytes: config.max_response_bytes,
        })
    }

    async fn read_range(&self, arguments: Value) -> Result<Value> {
        let request: SheetReadRequest = parse_closed(arguments, &["spreadsheet_id", "range"])
            .map_err(|_| request_rejected("invalid Google Sheets read arguments"))?;
        if !valid_spreadsheet_id(&request.spreadsheet_id) || !valid_a1_range(&request.range) {
            return Err(bound_violated(
                "Google Sheets identifier or range is invalid",
            ));
        }
        let allowed = self
            .allowed_ranges
            .get(&request.spreadsheet_id)
            .is_some_and(|ranges| ranges.contains(&request.range));
        if !allowed {
            return Err(bound_violated(
                "Google Sheets range is outside the approved allowlist",
            ));
        }

        // This method is called only after the router authorized and durably
        // logged the operation. Resolve OAuth at the last possible moment,
        // after all local validation, and only then build/send the request.
        let access = self.oauth.access_token().await?;
        let mut url = self.api_base.clone();
        append_segments(
            &mut url,
            &[
                "v4",
                "spreadsheets",
                &request.spreadsheet_id,
                "values",
                &request.range,
            ],
            "Google Sheets",
        )?;
        url.query_pairs_mut()
            .append_pair("majorDimension", "ROWS")
            .append_pair("valueRenderOption", "UNFORMATTED_VALUE")
            .append_pair("dateTimeRenderOption", "FORMATTED_STRING");
        let response = self
            .http
            .get(url)
            .bearer_auth(access.expose())
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| upstream_failed("Google Sheets API is unavailable"))?;
        drop(access);
        if !response.status().is_success() || response.status().is_redirection() {
            return Err(upstream_failed("Google Sheets API refused the request"));
        }
        let answer = bounded_json(response, self.max_response_bytes, "Google Sheets").await?;
        let values = answer
            .get("values")
            .cloned()
            .filter(Value::is_array)
            .unwrap_or_else(|| Value::Array(Vec::new()));
        Ok(json!({
            "spreadsheet_id": request.spreadsheet_id,
            "range": request.range,
            "values": values
        }))
    }
}

impl Upstream for GoogleSheetsReadUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        match body.get("method").and_then(Value::as_str) {
            Some("initialize") => Ok(initialize_response(id)),
            Some("tools/list") => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": [sheets_tool_descriptor()] }
            })),
            Some("tools/call") => {
                require_tool_name(&body, SHEETS_READ_TOOL)?;
                let arguments = call_arguments(&body)?;
                let value = self.read_range(arguments).await?;
                Ok(tool_result(id, value))
            }
            _ => Err(request_rejected("unsupported compiled extension method")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetReadRequest {
    spreadsheet_id: String,
    range: String,
}

#[derive(Debug, Clone)]
pub struct GoogleSheetsWriteConfig {
    pub api_base_url: String,
    pub allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    pub max_cells: usize,
    pub max_request_bytes: usize,
}

impl GoogleSheetsWriteConfig {
    pub fn new(
        api_base_url: impl Into<String>,
        allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    ) -> Self {
        Self {
            api_base_url: api_base_url.into(),
            allowed_ranges,
            max_cells: 1_000,
            max_request_bytes: 256 * 1024,
        }
    }

    fn validate(&self) -> Result<reqwest::Url> {
        let base = validated_api_base(&self.api_base_url, "Google Sheets")?;
        if self.allowed_ranges.is_empty()
            || self.max_cells == 0
            || self.max_cells > 10_000
            || self.max_request_bytes == 0
            || self.max_request_bytes > 1024 * 1024
            || self.allowed_ranges.iter().any(|(spreadsheet, ranges)| {
                !valid_spreadsheet_id(spreadsheet)
                    || ranges.is_empty()
                    || ranges.iter().any(|range| !valid_a1_range(range))
            })
        {
            return Err(config_rejected("invalid Google Sheets write bounds"));
        }
        Ok(base)
    }
}

pub struct GoogleSheetsWriteGuardedUpstream {
    oauth: Arc<UpstreamOAuthClient>,
    http: reqwest::Client,
    api_base: reqwest::Url,
    allowed_ranges: BTreeMap<String, BTreeSet<String>>,
    max_cells: usize,
    max_request_bytes: usize,
}

impl GoogleSheetsWriteGuardedUpstream {
    pub fn new(config: GoogleSheetsWriteConfig, oauth: Arc<UpstreamOAuthClient>) -> Result<Self> {
        let api_base = config.validate()?;
        Ok(Self {
            oauth,
            http: closed_http_client("Google Sheets")?,
            api_base,
            allowed_ranges: config.allowed_ranges,
            max_cells: config.max_cells,
            max_request_bytes: config.max_request_bytes,
        })
    }

    async fn update_values(&self, arguments: Value) -> Result<Value> {
        let request: SheetWriteRequest = parse_closed(
            arguments,
            &["spreadsheet_id", "range", "values", "payload_digest"],
        )
        .map_err(|_| request_rejected("invalid Google Sheets write arguments"))?;
        let allowed = valid_spreadsheet_id(&request.spreadsheet_id)
            && valid_a1_range(&request.range)
            && self
                .allowed_ranges
                .get(&request.spreadsheet_id)
                .is_some_and(|ranges| ranges.contains(&request.range));
        if !allowed {
            return Err(bound_violated(
                "Google Sheets range is outside the approved allowlist",
            ));
        }
        let cells = request.values.iter().try_fold(0usize, |total, row| {
            total
                .checked_add(row.len())
                .filter(|count| *count <= self.max_cells)
                .ok_or_else(|| bound_violated("Google Sheets write exceeds its cell bound"))
        })?;
        if cells == 0
            || request
                .values
                .iter()
                .flatten()
                .any(|value| !valid_sheet_cell(value))
        {
            return Err(bound_violated("Google Sheets write values are invalid"));
        }
        let canonical = serde_json::to_vec(&json!({
            "spreadsheet_id": request.spreadsheet_id,
            "range": request.range,
            "values": request.values,
        }))
        .map_err(|_| request_rejected("Google Sheets write is not canonical"))?;
        if canonical.len() > self.max_request_bytes {
            return Err(bound_violated("Google Sheets write exceeds its byte bound"));
        }
        let digest = blake3::hash(&canonical).to_hex().to_string();
        if request.payload_digest != digest {
            return Err(request_rejected(
                "Google Sheets write digest does not match its immutable payload",
            ));
        }
        let canonical: Value = serde_json::from_slice(&canonical)
            .map_err(|_| request_rejected("Google Sheets write is not canonical"))?;
        let spreadsheet_id = canonical["spreadsheet_id"]
            .as_str()
            .ok_or_else(|| request_rejected("Google Sheets write is not canonical"))?;
        let range = canonical["range"]
            .as_str()
            .ok_or_else(|| request_rejected("Google Sheets write is not canonical"))?;
        let values = canonical["values"].clone();
        let access = self.oauth.access_token().await?;
        let mut url = self.api_base.clone();
        append_segments(
            &mut url,
            &["v4", "spreadsheets", spreadsheet_id, "values", range],
            "Google Sheets",
        )?;
        url.query_pairs_mut()
            .append_pair("valueInputOption", "RAW")
            .append_pair("includeValuesInResponse", "false");
        let response = self
            .http
            .put(url)
            .bearer_auth(access.expose())
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&json!({
                "range": range,
                "majorDimension": "ROWS",
                "values": values,
            }))
            .send()
            .await
            .map_err(|_| upstream_failed("Google Sheets API is unavailable"))?;
        drop(access);
        if !response.status().is_success() || response.status().is_redirection() {
            return Err(upstream_failed("Google Sheets API refused the request"));
        }
        let answer = bounded_json(response, 64 * 1024, "Google Sheets").await?;
        let updated_cells = answer
            .get("updatedCells")
            .and_then(Value::as_u64)
            .filter(|count| *count <= self.max_cells as u64)
            .unwrap_or(cells as u64);
        Ok(json!({
            "status": "updated",
            "spreadsheet_id": spreadsheet_id,
            "range": range,
            "updated_cells": updated_cells,
            "payload_digest": digest,
        }))
    }
}

impl Upstream for GoogleSheetsWriteGuardedUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        match body.get("method").and_then(Value::as_str) {
            Some("initialize") => Ok(initialize_response(id)),
            Some("tools/list") => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": [sheets_write_tool_descriptor()] }
            })),
            Some("tools/call") => {
                require_tool_name(&body, SHEETS_WRITE_GUARDED_TOOL)?;
                let value = self.update_values(call_arguments(&body)?).await?;
                Ok(tool_result(id, value))
            }
            _ => Err(request_rejected("unsupported compiled extension method")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SheetWriteRequest {
    spreadsheet_id: String,
    range: String,
    values: Vec<Vec<Value>>,
    payload_digest: String,
}

/// Closed policy for the Gmail v1 adapter. Exact mailbox entries and exact
/// domains are ORed; an empty combined perimeter is invalid.
#[derive(Debug, Clone)]
pub struct GmailSendPolicy {
    pub api_base_url: String,
    pub allowed_recipients: BTreeSet<String>,
    pub allowed_domains: BTreeSet<String>,
    pub max_recipients: usize,
    pub max_subject_bytes: usize,
    pub max_body_bytes: usize,
    pub approval_ttl_seconds: i64,
}

impl GmailSendPolicy {
    pub fn new(api_base_url: impl Into<String>) -> Self {
        Self {
            api_base_url: api_base_url.into(),
            allowed_recipients: BTreeSet::new(),
            allowed_domains: BTreeSet::new(),
            max_recipients: 5,
            max_subject_bytes: 200,
            max_body_bytes: 64 * 1024,
            approval_ttl_seconds: 15 * 60,
        }
    }

    fn validate_and_normalize(&self) -> Result<(reqwest::Url, Self)> {
        let base = validated_api_base(&self.api_base_url, "Gmail")?;
        if self.max_recipients == 0
            || self.max_recipients > 20
            || self.max_subject_bytes == 0
            || self.max_subject_bytes > 998
            || self.max_body_bytes == 0
            || self.max_body_bytes > 1024 * 1024
            || !(30..=86_400).contains(&self.approval_ttl_seconds)
        {
            return Err(config_rejected("invalid Gmail extension bounds"));
        }
        let allowed_recipients = self
            .allowed_recipients
            .iter()
            .map(|mailbox| normalize_mailbox(mailbox))
            .collect::<Option<BTreeSet<_>>>()
            .ok_or_else(|| config_rejected("invalid Gmail recipient allowlist"))?;
        let allowed_domains = self
            .allowed_domains
            .iter()
            .map(|domain| normalize_domain(domain))
            .collect::<Option<BTreeSet<_>>>()
            .ok_or_else(|| config_rejected("invalid Gmail domain allowlist"))?;
        if allowed_recipients.is_empty() && allowed_domains.is_empty() {
            return Err(config_rejected("Gmail recipient allowlist is empty"));
        }
        let mut normalized = self.clone();
        normalized.allowed_recipients = allowed_recipients;
        normalized.allowed_domains = allowed_domains;
        Ok((base, normalized))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Expired,
    Dispatching,
    Dispatched,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalView {
    pub approval_id: String,
    pub payload_digest: String,
    pub state: ApprovalState,
    pub created_at: i64,
    pub expires_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approver: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
}

/// The plaintext review is intentionally obtainable only through the owner
/// API surface, never through `Upstream::forward` or an agent-facing result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApprovalReview {
    pub approval: ApprovalView,
    pub to: Vec<String>,
    pub subject: String,
    pub text_body: String,
}

impl Drop for ApprovalReview {
    fn drop(&mut self) {
        for recipient in &mut self.to {
            recipient.zeroize();
        }
        self.subject.zeroize();
        self.text_body.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PreparedMail {
    to: Vec<String>,
    subject: String,
    text_body: String,
    mime_raw: String,
    digest: String,
}

impl Drop for PreparedMail {
    fn drop(&mut self) {
        for recipient in &mut self.to {
            recipient.zeroize();
        }
        self.subject.zeroize();
        self.text_body.zeroize();
        self.mime_raw.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRecord {
    approval_id: String,
    payload: PreparedMail,
    state: ApprovalState,
    created_at: i64,
    expires_at: i64,
    approver: Option<String>,
    message_id: Option<String>,
}

impl ApprovalRecord {
    fn view(&self) -> ApprovalView {
        ApprovalView {
            approval_id: self.approval_id.clone(),
            payload_digest: self.payload.digest.clone(),
            state: self.state,
            created_at: self.created_at,
            expires_at: self.expires_at,
            approver: self.approver.clone(),
            message_id: self.message_id.clone(),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct OutboxInner {
    records: BTreeMap<String, ApprovalRecord>,
    by_digest: BTreeMap<String, String>,
}

struct DurableOutbox {
    broker: Arc<dyn CredentialBroker>,
    reference: CredentialRef,
}

/// Short-lived approval state. Production profiles attach a derived Vault
/// record so pending content is encrypted at rest and survives restart;
/// tests may keep the deliberately explicit memory-only construction.
pub struct ApprovalOutbox {
    inner: Mutex<OutboxInner>,
    now: Arc<dyn Fn() -> i64 + Send + Sync>,
    ttl_seconds: i64,
    durable: Option<DurableOutbox>,
    persistence: tokio::sync::Mutex<()>,
}

impl ApprovalOutbox {
    pub fn new(ttl_seconds: i64, now: Arc<dyn Fn() -> i64 + Send + Sync>) -> Result<Self> {
        if !(30..=86_400).contains(&ttl_seconds) {
            return Err(config_rejected("invalid approval outbox TTL"));
        }
        Ok(Self {
            inner: Mutex::new(OutboxInner::default()),
            now,
            ttl_seconds,
            durable: None,
            persistence: tokio::sync::Mutex::new(()),
        })
    }

    pub fn new_durable(
        ttl_seconds: i64,
        now: Arc<dyn Fn() -> i64 + Send + Sync>,
        broker: Arc<dyn CredentialBroker>,
        reference: CredentialRef,
    ) -> Result<Self> {
        let mut outbox = Self::new(ttl_seconds, now)?;
        outbox.durable = Some(DurableOutbox { broker, reference });
        Ok(outbox)
    }

    pub async fn hydrate(&self) -> Result<()> {
        let Some(durable) = &self.durable else {
            return Ok(());
        };
        let _persistence = self.persistence.lock().await;
        let Some(encoded) = durable
            .broker
            .resolve_optional(&durable.reference)
            .await
            .map_err(|_| outbox_failed())?
        else {
            return Ok(());
        };
        let mut restored: OutboxInner =
            serde_json::from_str(encoded.expose()).map_err(|_| outbox_failed())?;
        let changed = sweep_outbox(&mut restored, (self.now)());
        *self.inner.lock().map_err(|_| outbox_failed())? = restored;
        if changed {
            let snapshot = self.inner.lock().map_err(|_| outbox_failed())?.clone();
            let encoded = serde_json::to_string(&snapshot).map_err(|_| outbox_failed())?;
            durable
                .broker
                .store(&durable.reference, SecretValue::new(encoded))
                .await
                .map_err(|_| outbox_failed())?;
        }
        Ok(())
    }

    async fn persist(&self) -> Result<()> {
        let Some(durable) = &self.durable else {
            return Ok(());
        };
        let _persistence = self.persistence.lock().await;
        let snapshot = {
            let mut inner = self.inner.lock().map_err(|_| outbox_failed())?;
            sweep_outbox(&mut inner, (self.now)());
            inner.clone()
        };
        let encoded = serde_json::to_string(&snapshot).map_err(|_| outbox_failed())?;
        durable
            .broker
            .store(&durable.reference, SecretValue::new(encoded))
            .await
            .map_err(|_| outbox_failed())
    }

    fn enqueue(&self, payload: PreparedMail) -> Result<ApprovalView> {
        let now = (self.now)();
        let mut inner = self.lock()?;
        if let Some(existing_id) = inner.by_digest.get(&payload.digest).cloned() {
            let record = inner
                .records
                .get_mut(&existing_id)
                .ok_or_else(outbox_failed)?;
            expire_if_needed(record, now);
            return Ok(record.view());
        }
        if inner.records.len() >= MAX_OUTBOX_RECORDS {
            return Err(request_rejected("approval outbox capacity is exhausted"));
        }
        // The full digest makes collisions operationally irrelevant and gives
        // retries a deterministic id without introducing random custody.
        let approval_id = format!("apr-{}", payload.digest);
        let record = ApprovalRecord {
            approval_id: approval_id.clone(),
            payload,
            state: ApprovalState::Pending,
            created_at: now,
            expires_at: now.saturating_add(self.ttl_seconds),
            approver: None,
            message_id: None,
        };
        inner
            .by_digest
            .insert(record.payload.digest.clone(), approval_id.clone());
        let view = record.view();
        inner.records.insert(approval_id, record);
        Ok(view)
    }

    pub fn owner_review(&self, approval_id: &str) -> Result<ApprovalReview> {
        let now = (self.now)();
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        expire_if_needed(record, now);
        Ok(ApprovalReview {
            approval: record.view(),
            to: record.payload.to.clone(),
            subject: record.payload.subject.clone(),
            text_body: record.payload.text_body.clone(),
        })
    }

    /// Metadata-only rows for the owner inbox. Plaintext mail content remains
    /// available exclusively through `owner_review`.
    pub fn owner_list(&self) -> Result<Vec<ApprovalView>> {
        let now = (self.now)();
        let mut inner = self.lock()?;
        let mut approvals = inner
            .records
            .values_mut()
            .map(|record| {
                expire_if_needed(record, now);
                record.view()
            })
            .collect::<Vec<_>>();
        approvals.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.approval_id.cmp(&right.approval_id))
        });
        approvals.truncate(MAX_APPROVAL_INBOX_ITEMS);
        Ok(approvals)
    }

    pub fn owner_approve(&self, approval_id: &str, approver: &str) -> Result<ApprovalView> {
        let approver = validated_approver(approver)?;
        let now = (self.now)();
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        expire_if_needed(record, now);
        match record.state {
            ApprovalState::Pending => {
                record.state = ApprovalState::Approved;
                record.approver = Some(approver);
                Ok(record.view())
            }
            ApprovalState::Approved => {
                if record.approver.as_deref() == Some(approver.as_str()) {
                    Ok(record.view())
                } else {
                    Err(request_rejected("approval already decided"))
                }
            }
            _ => Err(request_rejected("approval is not pending")),
        }
    }

    pub fn owner_deny(&self, approval_id: &str, approver: &str) -> Result<ApprovalView> {
        let approver = validated_approver(approver)?;
        let now = (self.now)();
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        expire_if_needed(record, now);
        match record.state {
            ApprovalState::Pending => {
                record.state = ApprovalState::Denied;
                record.approver = Some(approver);
                erase_payload(&mut record.payload);
                Ok(record.view())
            }
            ApprovalState::Denied => {
                if record.approver.as_deref() == Some(approver.as_str()) {
                    Ok(record.view())
                } else {
                    Err(request_rejected("approval already decided"))
                }
            }
            _ => Err(request_rejected("approval is not pending")),
        }
    }

    pub fn status(&self, approval_id: &str) -> Result<ApprovalView> {
        let now = (self.now)();
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        expire_if_needed(record, now);
        Ok(record.view())
    }

    fn begin_dispatch(&self, approval_id: &str) -> Result<PreparedMail> {
        let now = (self.now)();
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        expire_if_needed(record, now);
        if record.state != ApprovalState::Approved {
            return Err(request_rejected("approval is not dispatchable"));
        }
        record.state = ApprovalState::Dispatching;
        Ok(record.payload.clone())
    }

    fn finish_dispatch(&self, approval_id: &str, message_id: String) -> Result<ApprovalView> {
        let mut inner = self.lock()?;
        let record = inner.records.get_mut(approval_id).ok_or_else(not_found)?;
        if record.state != ApprovalState::Dispatching {
            return Err(outbox_failed());
        }
        record.state = ApprovalState::Dispatched;
        record.message_id = Some(message_id);
        // Erase review content after the effect. The digest remains the
        // idempotence/audit link, but no body survives in the outbox.
        erase_payload(&mut record.payload);
        Ok(record.view())
    }

    fn fail_dispatch(&self, approval_id: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(record) = inner.records.get_mut(approval_id) {
                if record.state == ApprovalState::Dispatching {
                    // Terminal by design: a lost provider response is
                    // indistinguishable from a completed send, so retrying
                    // could duplicate an external effect.
                    record.state = ApprovalState::Failed;
                    erase_payload(&mut record.payload);
                }
            }
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, OutboxInner>> {
        self.inner.lock().map_err(|_| outbox_failed())
    }
}

#[derive(Clone)]
pub struct GmailSendGuardedUpstream {
    oauth: Arc<UpstreamOAuthClient>,
    http: reqwest::Client,
    api_base: reqwest::Url,
    policy: GmailSendPolicy,
    outbox: Arc<ApprovalOutbox>,
}

impl GmailSendGuardedUpstream {
    pub fn new(policy: GmailSendPolicy, oauth: Arc<UpstreamOAuthClient>) -> Result<Self> {
        Self::new_with_clock(policy, oauth, Arc::new(unix_now))
    }

    pub fn new_with_clock(
        policy: GmailSendPolicy,
        oauth: Arc<UpstreamOAuthClient>,
        now: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Result<Self> {
        let (api_base, policy) = policy.validate_and_normalize()?;
        let outbox = Arc::new(ApprovalOutbox::new(policy.approval_ttl_seconds, now)?);
        Self::from_parts(policy, oauth, api_base, outbox)
    }

    pub fn new_durable(
        policy: GmailSendPolicy,
        oauth: Arc<UpstreamOAuthClient>,
        broker: Arc<dyn CredentialBroker>,
        reference: CredentialRef,
    ) -> Result<Self> {
        let (api_base, policy) = policy.validate_and_normalize()?;
        let outbox = Arc::new(ApprovalOutbox::new_durable(
            policy.approval_ttl_seconds,
            Arc::new(unix_now),
            broker,
            reference,
        )?);
        Self::from_parts(policy, oauth, api_base, outbox)
    }

    fn from_parts(
        policy: GmailSendPolicy,
        oauth: Arc<UpstreamOAuthClient>,
        api_base: reqwest::Url,
        outbox: Arc<ApprovalOutbox>,
    ) -> Result<Self> {
        Ok(Self {
            oauth,
            http: closed_http_client("Gmail")?,
            api_base,
            policy,
            outbox,
        })
    }

    pub async fn hydrate(&self) -> Result<()> {
        self.outbox.hydrate().await
    }

    pub fn outbox(&self) -> Arc<ApprovalOutbox> {
        Arc::clone(&self.outbox)
    }

    pub async fn owner_review(&self, approval_id: &str) -> Result<ApprovalReview> {
        let review = self.outbox.owner_review(approval_id)?;
        self.outbox.persist().await?;
        Ok(review)
    }

    pub async fn owner_list(&self) -> Result<Vec<ApprovalView>> {
        let approvals = self.outbox.owner_list()?;
        self.outbox.persist().await?;
        Ok(approvals)
    }

    pub async fn approval_status(&self, approval_id: &str) -> Result<ApprovalView> {
        let view = self.outbox.status(approval_id)?;
        self.outbox.persist().await?;
        Ok(view)
    }

    pub async fn owner_approve(&self, approval_id: &str, approver: &str) -> Result<ApprovalView> {
        let view = self.outbox.owner_approve(approval_id, approver)?;
        self.outbox.persist().await?;
        Ok(view)
    }

    pub async fn owner_deny(&self, approval_id: &str, approver: &str) -> Result<ApprovalView> {
        let view = self.outbox.owner_deny(approval_id, approver)?;
        self.outbox.persist().await?;
        Ok(view)
    }

    /// Dispatch an owner-approved immutable payload. OAuth resolution and
    /// Gmail I/O occur only after the atomic Approved -> Dispatching move.
    pub async fn owner_dispatch(&self, approval_id: &str) -> Result<ApprovalView> {
        let payload = self.outbox.begin_dispatch(approval_id)?;
        self.outbox.persist().await?;
        let outcome = self.dispatch_once(&payload).await;
        match outcome {
            Ok(message_id) => {
                let view = self.outbox.finish_dispatch(approval_id, message_id)?;
                self.outbox.persist().await?;
                Ok(view)
            }
            Err(error) => {
                self.outbox.fail_dispatch(approval_id);
                let _ = self.outbox.persist().await;
                Err(error)
            }
        }
    }

    async fn request_approval(&self, arguments: Value) -> Result<ApprovalView> {
        let request: GmailSendRequest =
            parse_closed(arguments, &["to", "subject", "text_body", "cc", "bcc"])
                .map_err(|_| request_rejected("invalid Gmail send arguments"))?;
        let prepared = prepare_mail(request, &self.policy)?;
        let view = self.outbox.enqueue(prepared)?;
        self.outbox.persist().await?;
        Ok(view)
    }

    async fn dispatch_once(&self, payload: &PreparedMail) -> Result<String> {
        let access = self.oauth.access_token().await?;
        let mut url = self.api_base.clone();
        append_segments(
            &mut url,
            &["gmail", "v1", "users", "me", "messages", "send"],
            "Gmail",
        )?;
        let response = self
            .http
            .post(url)
            .bearer_auth(access.expose())
            .header(reqwest::header::ACCEPT, "application/json")
            .json(&json!({ "raw": payload.mime_raw }))
            .send()
            .await
            .map_err(|_| upstream_failed("Gmail API is unavailable"))?;
        drop(access);
        if !response.status().is_success() || response.status().is_redirection() {
            return Err(upstream_failed("Gmail API refused the request"));
        }
        let answer = bounded_json(response, 64 * 1024, "Gmail").await?;
        answer
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_provider_message_id(id))
            .map(str::to_owned)
            .ok_or_else(|| upstream_failed("Gmail API returned an invalid response"))
    }
}

impl Upstream for GmailSendGuardedUpstream {
    async fn forward(&self, body: Value) -> Result<Value> {
        let id = body.get("id").cloned().unwrap_or(Value::Null);
        match body.get("method").and_then(Value::as_str) {
            Some("initialize") => Ok(initialize_response(id)),
            Some("tools/list") => Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": [gmail_tool_descriptor()] }
            })),
            Some("tools/call") => {
                require_tool_name(&body, GMAIL_SEND_GUARDED_TOOL)?;
                let arguments = call_arguments(&body)?;
                let approval = self.request_approval(arguments).await?;
                // Replays return the existing terminal state and never create
                // a new effect. The agent can request but cannot approve or
                // dispatch: those methods exist only on the owner object.
                let value = approval_public_result(&approval);
                Ok(tool_result(id, value))
            }
            _ => Err(request_rejected("unsupported compiled extension method")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GmailSendRequest {
    to: Vec<String>,
    subject: String,
    text_body: String,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    bcc: Vec<String>,
}

fn prepare_mail(request: GmailSendRequest, policy: &GmailSendPolicy) -> Result<PreparedMail> {
    if request.to.is_empty()
        || request.to.len() > policy.max_recipients
        || !request.cc.is_empty()
        || !request.bcc.is_empty()
    {
        return Err(bound_violated(
            "Gmail recipient count or cc/bcc policy was violated",
        ));
    }
    if request.subject.is_empty()
        || request.subject.len() > policy.max_subject_bytes
        || request.text_body.is_empty()
        || request.text_body.len() > policy.max_body_bytes
        || request.subject.chars().any(|ch| matches!(ch, '\r' | '\n'))
        || request.text_body.contains('\0')
    {
        return Err(bound_violated(
            "Gmail subject or text body is outside approved bounds",
        ));
    }

    let mut to = request
        .to
        .iter()
        .map(|address| normalize_mailbox(address))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| bound_violated("Gmail recipient address is invalid"))?;
    to.sort();
    to.dedup();
    if to.is_empty() || to.len() > policy.max_recipients {
        return Err(bound_violated(
            "Gmail recipient count is outside approved bounds",
        ));
    }
    for address in &to {
        let domain = address
            .rsplit_once('@')
            .map(|(_, domain)| domain)
            .unwrap_or("");
        if !policy.allowed_recipients.contains(address) && !policy.allowed_domains.contains(domain)
        {
            return Err(bound_violated(
                "Gmail recipient is outside the approved allowlist",
            ));
        }
    }

    let text_body = normalize_text_body(&request.text_body);
    let canonical = serde_jcs::to_vec(&json!({
        "v": 1,
        "to": to,
        "subject": request.subject,
        "text_body": text_body,
    }))
    .map_err(|_| request_rejected("Gmail payload cannot be normalized"))?;
    let digest = blake3::hash(&canonical).to_hex().to_string();
    let mime = build_text_mime(&to, &request.subject, &text_body);
    let mime_raw = URL_SAFE_NO_PAD.encode(mime.as_bytes());
    Ok(PreparedMail {
        to,
        subject: request.subject,
        text_body,
        mime_raw,
        digest,
    })
}

fn build_text_mime(to: &[String], subject: &str, body: &str) -> String {
    let encoded_subject = if subject.is_ascii() {
        subject.to_owned()
    } else {
        format!("=?UTF-8?B?{}?=", STANDARD.encode(subject.as_bytes()))
    };
    let encoded_body = STANDARD.encode(body.as_bytes());
    format!(
        "To: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=UTF-8\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
        to.join(", "),
        encoded_subject,
        encoded_body
    )
}

fn normalize_text_body(body: &str) -> String {
    body.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn approval_public_result(approval: &ApprovalView) -> Value {
    let status = match approval.state {
        ApprovalState::Pending | ApprovalState::Approved => "approval_required",
        ApprovalState::Denied => "denied",
        ApprovalState::Expired => "expired",
        ApprovalState::Dispatching => "dispatching",
        ApprovalState::Dispatched => "dispatched",
        ApprovalState::Failed => "failed",
    };
    json!({
        "status": status,
        "approval_id": approval.approval_id,
        "payload_digest": approval.payload_digest,
        "expires_at": approval.expires_at,
        "message_id": approval.message_id,
    })
}

fn expire_if_needed(record: &mut ApprovalRecord, now: i64) {
    if matches!(
        record.state,
        ApprovalState::Pending | ApprovalState::Approved
    ) && now >= record.expires_at
    {
        record.state = ApprovalState::Expired;
        erase_payload(&mut record.payload);
    }
}

fn sweep_outbox(inner: &mut OutboxInner, now: i64) -> bool {
    let mut changed = false;
    for record in inner.records.values_mut() {
        let before = record.state;
        if record.state == ApprovalState::Dispatching {
            // A crash after request emission is ambiguous. Never retry it and
            // never retain its plaintext review payload after restart.
            record.state = ApprovalState::Failed;
            erase_payload(&mut record.payload);
        } else {
            expire_if_needed(record, now);
        }
        changed |= record.state != before;
    }
    changed
}

fn erase_payload(payload: &mut PreparedMail) {
    for recipient in &mut payload.to {
        recipient.zeroize();
    }
    payload.to.clear();
    payload.subject.zeroize();
    payload.text_body.zeroize();
    payload.mime_raw.zeroize();
}

fn validated_approver(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_APPROVER_BYTES
        || value.chars().any(|ch| ch.is_control())
    {
        return Err(request_rejected("invalid approver identity"));
    }
    Ok(value.to_owned())
}

fn normalize_mailbox(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() > 254 || !value.is_ascii() || value.chars().any(char::is_whitespace) {
        return None;
    }
    let mut parts = value.split('@');
    let local = parts.next()?;
    let domain = parts.next()?;
    if parts.next().is_some()
        || local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&ch))
    {
        return None;
    }
    let domain = normalize_domain(domain)?;
    Some(format!("{}@{domain}", local.to_ascii_lowercase()))
}

fn normalize_domain(value: &str) -> Option<String> {
    let domain = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if domain.is_empty()
        || domain.len() > 253
        || !domain.is_ascii()
        || !domain.contains('.')
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-')
        })
    {
        None
    } else {
        Some(domain)
    }
}

fn valid_spreadsheet_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SPREADSHEET_ID_BYTES
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_'))
}

fn valid_a1_range(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_A1_RANGE_BYTES
        && value.is_ascii()
        && !value.chars().any(char::is_control)
        && !value.chars().any(|ch| matches!(ch, '/' | '?' | '#' | '\\'))
        && value.bytes().all(|ch| {
            ch.is_ascii_alphanumeric()
                || matches!(ch, b' ' | b'_' | b'-' | b'!' | b':' | b'$' | b'.' | b'\'')
        })
}

fn valid_sheet_cell(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => true,
        Value::String(text) => text.len() <= 10_000 && !text.contains('\0'),
        Value::Array(_) | Value::Object(_) => false,
    }
}

fn valid_provider_message_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_'))
}

fn parse_closed<T: for<'de> Deserialize<'de>>(value: Value, allowed: &[&str]) -> Result<T> {
    let object = value
        .as_object()
        .ok_or_else(|| request_rejected("extension arguments must be an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(request_rejected(
            "extension arguments contain unknown fields",
        ));
    }
    serde_json::from_value(value)
        .map_err(|_| request_rejected("extension arguments have an invalid shape"))
}

fn call_arguments(body: &Value) -> Result<Value> {
    match body.pointer("/params/arguments") {
        Some(Value::Object(object)) => Ok(Value::Object(object.clone())),
        None => Ok(Value::Object(Map::new())),
        Some(_) => Err(request_rejected(
            "compiled extension arguments must be an object",
        )),
    }
}

fn require_tool_name(body: &Value, expected: &str) -> Result<()> {
    if body.pointer("/params/name").and_then(Value::as_str) == Some(expected) {
        Ok(())
    } else {
        Err(request_rejected("unknown compiled extension tool"))
    }
}

fn sheets_tool_descriptor() -> Value {
    json!({
        "name": SHEETS_READ_TOOL,
        "description": "Read one owner-approved A1 range from one approved Google spreadsheet.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "required": ["spreadsheet_id", "range"],
            "properties": {
                "spreadsheet_id": { "type": "string", "minLength": 1, "maxLength": MAX_SPREADSHEET_ID_BYTES },
                "range": { "type": "string", "minLength": 1, "maxLength": MAX_A1_RANGE_BYTES }
            }
        }
    })
}

fn sheets_write_tool_descriptor() -> Value {
    json!({
        "name": SHEETS_WRITE_GUARDED_TOOL,
        "description": "Idempotently replace one owner-approved A1 range with a digest-bound value matrix.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "required": ["spreadsheet_id", "range", "values", "payload_digest"],
            "properties": {
                "spreadsheet_id": { "type": "string", "minLength": 1, "maxLength": MAX_SPREADSHEET_ID_BYTES },
                "range": { "type": "string", "minLength": 1, "maxLength": MAX_A1_RANGE_BYTES },
                "values": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "array", "items": { "type": ["string", "number", "boolean", "null"] } }
                },
                "payload_digest": { "type": "string", "pattern": "^[0-9a-f]{64}$" }
            }
        }
    })
}

fn gmail_tool_descriptor() -> Value {
    json!({
        "name": GMAIL_SEND_GUARDED_TOOL,
        "description": "Request an owner-approved, idempotent plain-text Gmail send.",
        "inputSchema": {
            "type": "object",
            "additionalProperties": false,
            "required": ["to", "subject", "text_body"],
            "properties": {
                "to": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                "subject": { "type": "string", "minLength": 1 },
                "text_body": { "type": "string", "minLength": 1 },
                "cc": { "type": "array", "maxItems": 0, "items": { "type": "string" } },
                "bcc": { "type": "array", "maxItems": 0, "items": { "type": "string" } }
            }
        }
    })
}

fn initialize_response(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "aithos-compiled-extension", "version": env!("CARGO_PKG_VERSION") }
        }
    })
}

fn tool_result(id: Value, value: Value) -> Value {
    let text =
        serde_json::to_string(&value).unwrap_or_else(|_| "{\"status\":\"unavailable\"}".into());
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": text }],
            "structuredContent": value,
            "isError": false
        }
    })
}

fn validated_api_base(value: &str, provider: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(value)
        .map_err(|_| config_rejected(&format!("invalid {provider} API base URL")))?;
    let loopback_http = url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| matches!(host, "localhost" | "127.0.0.1" | "::1"));
    if (url.scheme() != "https" && !loopback_http)
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(config_rejected(&format!("invalid {provider} API base URL")));
    }
    // A directory-form base makes path-segment appends deterministic without
    // accepting caller-controlled URL resolution.
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn append_segments(url: &mut reqwest::Url, segments: &[&str], provider: &str) -> Result<()> {
    url.path_segments_mut()
        .map_err(|_| config_rejected(&format!("invalid {provider} API base URL")))?
        .pop_if_empty()
        .extend(segments);
    Ok(())
}

fn closed_http_client(provider: &str) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| config_rejected(&format!("cannot build {provider} HTTP client")))
}

async fn bounded_json(
    mut response: reqwest::Response,
    max_bytes: usize,
    provider: &str,
) -> Result<Value> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(upstream_failed(&format!(
            "{provider} API response is too large"
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| upstream_failed(&format!("{provider} API returned an invalid response")))?
    {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(upstream_failed(&format!(
                "{provider} API response is too large"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| upstream_failed(&format!("{provider} API returned an invalid response")))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or(0)
}

fn config_rejected(message: &str) -> GatewayError {
    GatewayError::ConfigRejected(message.to_owned())
}

fn request_rejected(message: &str) -> GatewayError {
    GatewayError::RequestRejected(message.to_owned())
}

fn bound_violated(message: &str) -> GatewayError {
    GatewayError::BoundViolated(message.to_owned())
}

fn upstream_failed(message: &str) -> GatewayError {
    GatewayError::UpstreamFailed(message.to_owned())
}

fn outbox_failed() -> GatewayError {
    request_rejected("approval outbox is unavailable")
}

fn not_found() -> GatewayError {
    request_rejected("approval request was not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicI64, Ordering};

    #[derive(Default)]
    struct MemoryOutboxBroker(Mutex<Option<String>>);

    impl CredentialBroker for MemoryOutboxBroker {
        fn resolve<'a>(
            &'a self,
            _reference: &'a CredentialRef,
        ) -> Pin<Box<dyn Future<Output = Result<SecretValue>> + Send + 'a>> {
            Box::pin(async move {
                self.0
                    .lock()
                    .map_err(|_| outbox_failed())?
                    .clone()
                    .map(SecretValue::new)
                    .ok_or_else(not_found)
            })
        }

        fn resolve_optional<'a>(
            &'a self,
            _reference: &'a CredentialRef,
        ) -> Pin<Box<dyn Future<Output = Result<Option<SecretValue>>> + Send + 'a>> {
            Box::pin(async move {
                Ok(self
                    .0
                    .lock()
                    .map_err(|_| outbox_failed())?
                    .clone()
                    .map(SecretValue::new))
            })
        }

        fn store<'a>(
            &'a self,
            _reference: &'a CredentialRef,
            value: SecretValue,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
            Box::pin(async move {
                *self.0.lock().map_err(|_| outbox_failed())? = Some(value.expose().to_owned());
                Ok(())
            })
        }
    }

    fn policy() -> GmailSendPolicy {
        let mut policy = GmailSendPolicy::new("http://127.0.0.1:9999/");
        policy.allowed_recipients.insert("demo@example.test".into());
        policy
    }

    #[test]
    fn gmail_payload_is_closed_normalized_and_mime_text() {
        let prepared = prepare_mail(
            GmailSendRequest {
                to: vec![" Demo@Example.Test ".into()],
                subject: "Bonjour éthique".into(),
                text_body: "ligne 1\nligne 2".into(),
                cc: vec![],
                bcc: vec![],
            },
            &policy().validate_and_normalize().unwrap().1,
        )
        .unwrap();
        assert_eq!(prepared.to, ["demo@example.test"]);
        let mime = String::from_utf8(URL_SAFE_NO_PAD.decode(&prepared.mime_raw).unwrap()).unwrap();
        assert!(mime.contains("Content-Type: text/plain; charset=UTF-8\r\n"));
        assert!(mime.contains("Content-Transfer-Encoding: base64\r\n"));
        assert!(mime.contains("Subject: =?UTF-8?B?"));
        assert!(!mime.contains("ligne 1"), "body is transfer-encoded");
    }

    #[test]
    fn digest_is_canonical_and_changes_with_payload() {
        let policy = policy().validate_and_normalize().unwrap().1;
        let make = |to: &str, body: &str| {
            prepare_mail(
                GmailSendRequest {
                    to: vec![to.into()],
                    subject: "Objet".into(),
                    text_body: body.into(),
                    cc: vec![],
                    bcc: vec![],
                },
                &policy,
            )
            .unwrap()
        };
        assert_eq!(
            make("DEMO@example.test", "a\nb").digest,
            make("demo@example.test", "a\r\nb").digest
        );
        assert_ne!(
            make("demo@example.test", "a").digest,
            make("demo@example.test", "b").digest
        );
    }

    #[test]
    fn policy_refuses_neighbors_cc_and_header_injection() {
        let policy = policy().validate_and_normalize().unwrap().1;
        let request = |to: &str, subject: &str, cc: Vec<String>| GmailSendRequest {
            to: vec![to.into()],
            subject: subject.into(),
            text_body: "body".into(),
            cc,
            bcc: vec![],
        };
        assert!(prepare_mail(request("other@example.test", "ok", vec![]), &policy).is_err());
        assert!(prepare_mail(
            request("demo@example.test", "ok", vec!["demo@example.test".into()]),
            &policy
        )
        .is_err());
        assert!(prepare_mail(
            request("demo@example.test", "x\r\nBcc: x@y.test", vec![]),
            &policy
        )
        .is_err());
    }

    #[test]
    fn outbox_is_idempotent_and_owner_state_machine_is_closed() {
        let now = Arc::new(AtomicI64::new(1_000));
        let clock: Arc<dyn Fn() -> i64 + Send + Sync> = {
            let now = Arc::clone(&now);
            Arc::new(move || now.load(Ordering::SeqCst))
        };
        let outbox = ApprovalOutbox::new(60, clock).unwrap();
        let policy = policy().validate_and_normalize().unwrap().1;
        let payload = prepare_mail(
            GmailSendRequest {
                to: vec!["demo@example.test".into()],
                subject: "Objet".into(),
                text_body: "body".into(),
                cc: vec![],
                bcc: vec![],
            },
            &policy,
        )
        .unwrap();
        let first = outbox.enqueue(payload.clone()).unwrap();
        let replay = outbox.enqueue(payload).unwrap();
        assert_eq!(first.approval_id, replay.approval_id);
        let inbox = outbox.owner_list().unwrap();
        assert_eq!(inbox, [first.clone()]);
        assert_eq!(
            outbox.owner_review(&first.approval_id).unwrap().text_body,
            "body"
        );
        assert_eq!(
            outbox
                .owner_approve(&first.approval_id, "owner-1")
                .unwrap()
                .state,
            ApprovalState::Approved
        );
        assert!(outbox.owner_deny(&first.approval_id, "owner-1").is_err());
        assert!(outbox.begin_dispatch(&first.approval_id).is_ok());
        assert!(outbox.begin_dispatch(&first.approval_id).is_err());
        let sent = outbox
            .finish_dispatch(&first.approval_id, "gmail-id-1".into())
            .unwrap();
        assert_eq!(sent.state, ApprovalState::Dispatched);
        assert_eq!(sent.message_id.as_deref(), Some("gmail-id-1"));
        assert!(outbox
            .owner_review(&first.approval_id)
            .unwrap()
            .text_body
            .is_empty());
    }

    #[test]
    fn approval_expires_without_dispatch() {
        let now = Arc::new(AtomicI64::new(10));
        let clock: Arc<dyn Fn() -> i64 + Send + Sync> = {
            let now = Arc::clone(&now);
            Arc::new(move || now.load(Ordering::SeqCst))
        };
        let outbox = ApprovalOutbox::new(30, clock).unwrap();
        let policy = policy().validate_and_normalize().unwrap().1;
        let payload = prepare_mail(
            GmailSendRequest {
                to: vec!["demo@example.test".into()],
                subject: "Objet".into(),
                text_body: "body".into(),
                cc: vec![],
                bcc: vec![],
            },
            &policy,
        )
        .unwrap();
        let approval = outbox.enqueue(payload).unwrap();
        now.store(40, Ordering::SeqCst);
        assert_eq!(
            outbox.status(&approval.approval_id).unwrap().state,
            ApprovalState::Expired
        );
        assert!(outbox
            .owner_approve(&approval.approval_id, "owner")
            .is_err());
        assert!(outbox.begin_dispatch(&approval.approval_id).is_err());
    }

    #[tokio::test]
    async fn durable_outbox_roundtrips_pending_plaintext_only_through_broker_custody() {
        let broker = Arc::new(MemoryOutboxBroker::default());
        let reference = CredentialRef {
            broker: "vault".into(),
            path: "connectors/demo/outbox".into(),
            field: "value".into(),
        };
        let now: Arc<dyn Fn() -> i64 + Send + Sync> = Arc::new(|| 1_000);
        let first = ApprovalOutbox::new_durable(
            60,
            Arc::clone(&now),
            Arc::clone(&broker) as Arc<dyn CredentialBroker>,
            reference.clone(),
        )
        .unwrap();
        let payload = prepare_mail(
            GmailSendRequest {
                to: vec!["demo@example.test".into()],
                subject: "Objet".into(),
                text_body: "secret body".into(),
                cc: vec![],
                bcc: vec![],
            },
            &policy().validate_and_normalize().unwrap().1,
        )
        .unwrap();
        let approval = first.enqueue(payload).unwrap();
        first.persist().await.unwrap();

        let restored =
            ApprovalOutbox::new_durable(60, now, broker as Arc<dyn CredentialBroker>, reference)
                .unwrap();
        restored.hydrate().await.unwrap();
        assert_eq!(
            restored
                .owner_review(&approval.approval_id)
                .unwrap()
                .text_body,
            "secret body"
        );
    }

    #[tokio::test]
    async fn durable_outbox_sweeps_expired_plaintext_during_restart_hydration() {
        let broker = Arc::new(MemoryOutboxBroker::default());
        let reference = CredentialRef {
            broker: "vault".into(),
            path: "connectors/demo/expiring-outbox".into(),
            field: "value".into(),
        };
        let clock_value = Arc::new(AtomicI64::new(1_000));
        let clock: Arc<dyn Fn() -> i64 + Send + Sync> = {
            let clock_value = Arc::clone(&clock_value);
            Arc::new(move || clock_value.load(Ordering::SeqCst))
        };
        let first = ApprovalOutbox::new_durable(
            60,
            Arc::clone(&clock),
            Arc::clone(&broker) as Arc<dyn CredentialBroker>,
            reference.clone(),
        )
        .unwrap();
        let payload = prepare_mail(
            GmailSendRequest {
                to: vec!["demo@example.test".into()],
                subject: "Objet".into(),
                text_body: "restart-secret-body".into(),
                cc: vec![],
                bcc: vec![],
            },
            &policy().validate_and_normalize().unwrap().1,
        )
        .unwrap();
        let approval = first.enqueue(payload).unwrap();
        first.persist().await.unwrap();
        clock_value.store(2_000, Ordering::SeqCst);

        let restored = ApprovalOutbox::new_durable(
            60,
            clock,
            Arc::clone(&broker) as Arc<dyn CredentialBroker>,
            reference,
        )
        .unwrap();
        restored.hydrate().await.unwrap();
        let review = restored.owner_review(&approval.approval_id).unwrap();
        assert_eq!(review.approval.state, ApprovalState::Expired);
        assert!(review.text_body.is_empty());
        assert!(!broker
            .0
            .lock()
            .unwrap()
            .as_deref()
            .unwrap()
            .contains("restart-secret-body"));
    }

    #[test]
    fn api_bases_and_resource_identifiers_are_not_generic_urls() {
        assert!(validated_api_base("https://sheets.googleapis.com/", "Sheets").is_ok());
        assert!(validated_api_base("http://example.com/", "Sheets").is_err());
        assert!(validated_api_base("https://user:secret@example.com/", "Sheets").is_err());
        assert!(valid_spreadsheet_id("abc_123-def"));
        assert!(!valid_spreadsheet_id("../secret"));
        assert!(valid_a1_range("'Demo Sheet'!A1:C10"));
        assert!(!valid_a1_range("A1/../../token"));
    }

    #[test]
    fn compiled_catalogue_is_local_fixed_and_pinnable() {
        let sheets =
            compiled_manifest("sheets-demo", CompiledConnectorAdapter::GoogleSheetsRead).unwrap();
        assert_eq!(sheets.tools.len(), 1);
        assert_eq!(sheets.tools[0].name, SHEETS_READ_TOOL);
        assert!(!sheets.tools[0].pin_sha256.is_empty());

        let gmail =
            compiled_manifest("gmail-demo", CompiledConnectorAdapter::GmailSendGuarded).unwrap();
        assert_eq!(gmail.tools.len(), 1);
        assert_eq!(gmail.tools[0].name, GMAIL_SEND_GUARDED_TOOL);
        assert!(!gmail.tools[0].pin_sha256.is_empty());

        let sheets_write = compiled_manifest(
            "sheets-write",
            CompiledConnectorAdapter::GoogleSheetsWriteGuarded,
        )
        .unwrap();
        assert_eq!(sheets_write.tools.len(), 1);
        assert_eq!(sheets_write.tools[0].name, SHEETS_WRITE_GUARDED_TOOL);
        assert!(!sheets_write.tools[0].pin_sha256.is_empty());
    }
}

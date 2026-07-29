//! Isolated execution seam for the six native `ethos.*` MCP tools.
//!
//! Generic connector routing never enters this module. The first backend is
//! deliberately a byte-identical adapter over the historical Runner methods;
//! later gates add the `aithos-client` planner and Provider transport behind
//! this same exact-name boundary.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::core_bridge::Runner;
use crate::oauth::BearerSession;
use crate::proxy_mcp::{
    legacy_ethos_dispatch, legacy_ethos_dispatch_delegated, ETHOS_CONTEXT, ETHOS_CREATE,
    ETHOS_DELETE, ETHOS_EDIT, ETHOS_LIST, ETHOS_READ,
};
use crate::Result;

const STORE_WIRE_VERSION: &str = "1.0.0-draft.1";
const DEFAULT_MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Runtime-selected implementation for native Ethos operations.
#[derive(Debug, Default)]
pub enum EthosBackend {
    /// Historical Gateway implementation, retained as the instant rollback.
    #[default]
    Legacy,
    /// Test/canary mode: execute an independent `aithos-client` read first,
    /// compare its semantic result, then serve the untouched legacy bytes.
    ClientShadow,
    /// Opt-in Provider-primary mutation path. Reads and every unsupported
    /// mutation retain the historical implementation; only compatible
    /// circle create/edit/delete calls use operation-scoped client plans.
    ClientProvider,
}

/// Default backend used by every existing runtime and test until a config
/// explicitly selects the client/Provider implementation.
pub fn legacy_ethos_backend() -> Arc<EthosBackend> {
    Arc::new(EthosBackend::Legacy)
}

pub fn client_shadow_ethos_backend() -> Arc<EthosBackend> {
    Arc::new(EthosBackend::ClientShadow)
}

pub fn client_provider_ethos_backend() -> Arc<EthosBackend> {
    Arc::new(EthosBackend::ClientProvider)
}

/// Runtime selector with a fail-closed unknown-value policy. Unset keeps the
/// historical backend, making deployment and rollback independent of config
/// migrations.
pub fn ethos_backend_from_env() -> Result<Arc<EthosBackend>> {
    match std::env::var("AITHOS_ETHOS_BACKEND") {
        Ok(value) => ethos_backend_from_selector(Some(&value)),
        Err(std::env::VarError::NotPresent) => ethos_backend_from_selector(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(crate::GatewayError::ConfigRejected(
            "AITHOS_ETHOS_BACKEND is not valid UTF-8".into(),
        )),
    }
}

fn ethos_backend_from_selector(value: Option<&str>) -> Result<Arc<EthosBackend>> {
    match value {
        None | Some("") | Some("legacy") => Ok(legacy_ethos_backend()),
        Some("shadow") => Ok(client_shadow_ethos_backend()),
        Some("client-provider") => Ok(client_provider_ethos_backend()),
        Some(_) => Err(crate::GatewayError::ConfigRejected(
            "AITHOS_ETHOS_BACKEND must be legacy, shadow or client-provider".into(),
        )),
    }
}

/// Transport-only Provider client. It receives requests already closed and
/// signed by `aithos-client`; it cannot choose a tenant, DID, path, method,
/// mandate or signing key.
#[derive(Clone)]
pub struct ProviderTransport {
    base: reqwest::Url,
    host: String,
    client: reqwest::Client,
    max_response_bytes: usize,
}

/// Bounded wire response. Provider diagnostics remain opaque bytes and are
/// never interpolated into agent-facing errors.
pub struct ProviderHttpResponse {
    status: reqwest::StatusCode,
    content_type: Option<String>,
    body: Vec<u8>,
}

/// A fully closed mutation: every Provider request has already been derived,
/// signed and cold-verified while the Runner lock was held. Execution performs
/// transport only and never receives keys, mandates, paths or caller input.
pub struct PreparedEthosMutation {
    transport: ProviderTransport,
    envelopes: Vec<aithos_client::ProviderEnvelopePlan>,
    commit_probe: aithos_client::ProviderReadEnvelopePlan,
    expected_new_head: String,
    response: Value,
}

impl PreparedEthosMutation {
    pub(crate) fn new(
        transport: ProviderTransport,
        envelopes: Vec<aithos_client::ProviderEnvelopePlan>,
        commit_probe: aithos_client::ProviderReadEnvelopePlan,
        expected_new_head: String,
        response: Value,
    ) -> Result<Self> {
        if envelopes.is_empty() {
            return Err(crate::GatewayError::BridgeFailed(
                "Ethos Provider mutation has no closed uploads".into(),
            ));
        }
        Ok(Self {
            transport,
            envelopes,
            commit_probe,
            expected_new_head,
            response,
        })
    }

    /// Execute in the plan's manifest-last order. A failed artifact never
    /// advances the edition head; diagnostics from the remote body remain
    /// deliberately opaque to the agent.
    pub async fn execute(self) -> Result<String> {
        for envelope in &self.envelopes {
            match self.transport.upload(envelope).await {
                Ok(response) if response.status().is_success() => {}
                Ok(response) => {
                    if self.commit_is_visible().await? {
                        return self.render_response();
                    }
                    if response.status() == reqwest::StatusCode::CONFLICT {
                        return Err(crate::GatewayError::RequestRejected(
                            "Ethos changed concurrently; read the section again and retry".into(),
                        ));
                    }
                    return Err(crate::GatewayError::UpstreamFailed(
                        "Ethos Provider refused a closed mutation artifact".into(),
                    ));
                }
                Err(error) => {
                    if self.commit_is_visible().await? {
                        return self.render_response();
                    }
                    return Err(error);
                }
            }
        }
        if !self.commit_is_visible().await? {
            return Err(crate::GatewayError::UpstreamFailed(
                "Ethos Provider commit could not be verified".into(),
            ));
        }
        self.render_response()
    }

    async fn commit_is_visible(&self) -> Result<bool> {
        let response = self.transport.read(&self.commit_probe).await?;
        if !response.status().is_success() {
            return Err(crate::GatewayError::UpstreamFailed(
                "Ethos Provider commit verification was refused".into(),
            ));
        }
        let heads: Value = serde_json::from_slice(response.body()).map_err(|_| {
            crate::GatewayError::UpstreamFailed(
                "Ethos Provider commit verification was malformed".into(),
            )
        })?;
        Ok(heads.get("manifest").and_then(Value::as_str) == Some(self.expected_new_head.as_str()))
    }

    fn render_response(&self) -> Result<String> {
        serde_json::to_string(&self.response)
            .map_err(|_| crate::GatewayError::BridgeFailed("Ethos response unavailable".into()))
    }
}

impl ProviderTransport {
    pub fn new(base: &str) -> Result<Self> {
        Self::with_limit(base, DEFAULT_MAX_RESPONSE_BYTES)
    }

    fn with_limit(base: &str, max_response_bytes: usize) -> Result<Self> {
        let mut parsed = reqwest::Url::parse(base).map_err(|_| {
            crate::GatewayError::RequestRejected("invalid Ethos Provider base URL".into())
        })?;
        let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "::1" | "localhost"));
        if parsed.scheme() != "https" && !(parsed.scheme() == "http" && loopback) {
            return Err(crate::GatewayError::RequestRejected(
                "Ethos Provider requires HTTPS outside exact loopback".into(),
            ));
        }
        if parsed.username() != ""
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
            || max_response_bytes == 0
        {
            return Err(crate::GatewayError::RequestRejected(
                "invalid Ethos Provider base URL".into(),
            ));
        }
        parsed.set_path("/");
        let host = parsed.host_str().map(str::to_owned).ok_or_else(|| {
            crate::GatewayError::RequestRejected("invalid Ethos Provider base URL".into())
        })?;
        let host = match parsed.port() {
            Some(port) => format!("{host}:{port}"),
            None => host,
        };
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| {
                crate::GatewayError::UpstreamFailed("Ethos Provider HTTP client unavailable".into())
            })?;
        Ok(Self {
            base: parsed,
            host,
            client,
            max_response_bytes,
        })
    }

    /// Exact host value that `aithos-client` must bind into its envelope.
    pub fn envelope_host(&self) -> &str {
        &self.host
    }

    pub async fn read(
        &self,
        plan: &aithos_client::ProviderReadEnvelopePlan,
    ) -> Result<ProviderHttpResponse> {
        self.send(ProviderWireRequest {
            method: plan.method(),
            path: plan.path(),
            body: plan.body(),
            auth: plan.header_value(),
            if_head: None,
        })
        .await
    }

    pub async fn upload(
        &self,
        plan: &aithos_client::ProviderEnvelopePlan,
    ) -> Result<ProviderHttpResponse> {
        self.send(ProviderWireRequest {
            method: plan.method(),
            path: plan.path(),
            body: plan.body(),
            auth: plan.header_value(),
            if_head: Some(plan.if_head()),
        })
        .await
    }

    async fn send(&self, request: ProviderWireRequest<'_>) -> Result<ProviderHttpResponse> {
        if !request.path.starts_with("/t/")
            || request.path.starts_with("//")
            || request.path.contains('?')
            || request.path.contains('#')
            || !matches!(request.method, "GET" | "POST" | "PUT")
        {
            return Err(crate::GatewayError::RequestRejected(
                "invalid closed Ethos Provider request".into(),
            ));
        }
        let url = self.base.join(&request.path[1..]).map_err(|_| {
            crate::GatewayError::RequestRejected("invalid closed Ethos Provider request".into())
        })?;
        if url.host_str() != self.base.host_str()
            || url.port_or_known_default() != self.base.port_or_known_default()
        {
            return Err(crate::GatewayError::RequestRejected(
                "invalid closed Ethos Provider request".into(),
            ));
        }
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|_| {
            crate::GatewayError::RequestRejected("invalid closed Ethos Provider method".into())
        })?;
        let mut outbound = self
            .client
            .request(method, url)
            .header("Content-Type", "application/octet-stream")
            .header("X-Aithos-Store", STORE_WIRE_VERSION)
            .header("X-Aithos-Auth", request.auth);
        if let Some(if_head) = request.if_head {
            outbound = outbound.header("If-Head", if_head);
        }
        if !request.body.is_empty() {
            outbound = outbound.body(request.body.to_vec());
        }
        let response = outbound.send().await.map_err(|_| {
            crate::GatewayError::UpstreamFailed("Ethos Provider transport unavailable".into())
        })?;
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(crate::GatewayError::UpstreamFailed(
                "Ethos Provider response exceeds the configured limit".into(),
            ));
        }
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await.map_err(|_| {
            crate::GatewayError::UpstreamFailed("Ethos Provider response unavailable".into())
        })?;
        if body.len() > self.max_response_bytes {
            return Err(crate::GatewayError::UpstreamFailed(
                "Ethos Provider response exceeds the configured limit".into(),
            ));
        }
        Ok(ProviderHttpResponse {
            status,
            content_type,
            body: body.to_vec(),
        })
    }
}

impl ProviderHttpResponse {
    pub fn status(&self) -> reqwest::StatusCode {
        self.status
    }

    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

struct ProviderWireRequest<'a> {
    method: &'a str,
    path: &'a str,
    body: &'a [u8],
    auth: &'a str,
    if_head: Option<&'a str>,
}

impl EthosBackend {
    /// Exact closed tool family. Prefixes, suffixes and connector names that
    /// merely contain `ethos` remain on the generic MCP route.
    pub fn handles(tool: &str) -> bool {
        matches!(
            tool,
            ETHOS_READ | ETHOS_LIST | ETHOS_CONTEXT | ETHOS_CREATE | ETHOS_EDIT | ETHOS_DELETE
        )
    }

    /// The historical non-OAuth surface exposed reads only. Keeping this
    /// narrower classifier preserves its refusal bytes for direct write calls.
    pub fn handles_legacy(tool: &str) -> bool {
        matches!(tool, ETHOS_READ | ETHOS_LIST | ETHOS_CONTEXT)
    }

    pub(crate) fn dispatch_legacy(
        &self,
        runner: &mut Runner,
        tool: &str,
        args: &Value,
        now: &str,
    ) -> Result<String> {
        match self {
            Self::Legacy | Self::ClientShadow | Self::ClientProvider => {
                legacy_ethos_dispatch(runner, tool, args, now)
            }
        }
    }

    pub(crate) fn dispatch_delegated(
        &self,
        runner: &mut Runner,
        session: &BearerSession,
        tool: &str,
        args: &Value,
        now: &str,
    ) -> Result<String> {
        match self {
            Self::Legacy | Self::ClientProvider => {
                legacy_ethos_dispatch_delegated(runner, session, tool, args, now)
            }
            Self::ClientShadow => {
                let probe = match tool {
                    ETHOS_READ => {
                        match (required_string(args, "zone"), required_string(args, "path")) {
                            (Ok(zone), Ok(path)) => runner
                                .ethos_client_read_probe_for_session(
                                    &session.context,
                                    &session.leaf_id,
                                    &session.session_pub,
                                    &session.leaf,
                                    zone,
                                    path,
                                    now,
                                )
                                .ok(),
                            _ => None,
                        }
                    }
                    ETHOS_LIST => runner
                        .ethos_client_list_probe_for_session(
                            &session.context,
                            &session.leaf_id,
                            &session.session_pub,
                            &session.leaf,
                            now,
                        )
                        .ok(),
                    _ => None,
                };
                let served = legacy_ethos_dispatch_delegated(runner, session, tool, args, now)?;
                if let Some(probe) = probe {
                    if compare_shadow(tool, &probe, &served).is_err() {
                        tracing::warn!(
                            event = "ethos_client_shadow_mismatch",
                            tool,
                            "Ethos client shadow disagreed with legacy"
                        );
                    }
                }
                Ok(served)
            }
        }
    }

    /// Prepare only the exact canary subset. Returning `None` deliberately
    /// falls back to the historical dispatcher, preserving public/self
    /// refusals, local stores and richer create metadata byte-for-byte.
    pub(crate) fn prepare_delegated_mutation(
        &self,
        runner: &mut Runner,
        session: &BearerSession,
        tool: &str,
        args: &Value,
        now: &str,
    ) -> Result<Option<PreparedEthosMutation>> {
        if !matches!(self, Self::ClientProvider)
            || !matches!(tool, ETHOS_CREATE | ETHOS_EDIT | ETHOS_DELETE)
        {
            return Ok(None);
        }
        let object = args
            .as_object()
            .ok_or_else(|| mutation_bad_args(tool, "arguments must be an object"))?;
        let allowed: &[&str] = match tool {
            ETHOS_CREATE => &["context", "zone", "folder", "name", "title", "tags", "body"],
            ETHOS_EDIT => &["context", "zone", "path", "body", "expected_digest"],
            ETHOS_DELETE => &["context", "zone", "path", "expected_digest"],
            _ => unreachable!("closed mutation classifier"),
        };
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(mutation_bad_args(tool, &format!("unknown field `{key}`")));
            }
        }
        let text_field = |name: &str| match object.get(name) {
            None => Ok(None),
            Some(Value::String(value)) if !value.trim().is_empty() => Ok(Some(value.clone())),
            Some(Value::String(_)) => Err(mutation_bad_args(
                tool,
                &format!("`{name}` must not be empty"),
            )),
            Some(_) => Err(mutation_bad_args(
                tool,
                &format!("`{name}` must be a string"),
            )),
        };
        let required = |name: &str| {
            text_field(name)?
                .ok_or_else(|| mutation_bad_args(tool, &format!("`{name}` is required")))
        };
        let _ = text_field("context")?;
        let zone = required("zone")?;
        if zone != "circle" {
            return Ok(None);
        }
        if !runner.context_is_provider_primary(&session.context) {
            return Ok(None);
        }
        match tool {
            ETHOS_CREATE => {
                let name = required("name")?;
                let body = required("body")?;
                let folder = text_field("folder")?.unwrap_or_default();
                let title = text_field("title")?.unwrap_or_else(|| name.clone());
                let tags = match object.get("tags") {
                    None => Vec::new(),
                    Some(Value::Array(items)) => items
                        .iter()
                        .map(|item| {
                            item.as_str().map(str::to_owned).ok_or_else(|| {
                                mutation_bad_args(tool, "`tags` must be an array of strings")
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                    Some(_) => {
                        return Err(mutation_bad_args(
                            tool,
                            "`tags` must be an array of strings",
                        ));
                    }
                };
                if title != name || !tags.is_empty() {
                    return Ok(None);
                }
                runner
                    .prepare_ethos_client_create_for_session(
                        &session.context,
                        &session.leaf_id,
                        &session.session_pub,
                        &session.leaf,
                        &zone,
                        &folder,
                        &name,
                        &body,
                        now,
                    )
                    .map(Some)
            }
            ETHOS_EDIT => {
                let path = required("path")?;
                let body = required("body")?;
                let expected = required("expected_digest")?;
                runner
                    .prepare_ethos_client_edit_for_session(
                        &session.context,
                        &session.leaf_id,
                        &session.session_pub,
                        &session.leaf,
                        &zone,
                        &path,
                        &body,
                        &expected,
                        now,
                    )
                    .map(Some)
            }
            ETHOS_DELETE => {
                let path = required("path")?;
                let expected = text_field("expected_digest")?;
                runner
                    .prepare_ethos_client_delete_for_session(
                        &session.context,
                        &session.leaf_id,
                        &session.session_pub,
                        &session.leaf,
                        &zone,
                        &path,
                        expected.as_deref(),
                        now,
                    )
                    .map(Some)
            }
            _ => unreachable!("closed mutation classifier"),
        }
    }
}

fn mutation_bad_args(tool: &str, detail: &str) -> crate::GatewayError {
    crate::GatewayError::RequestRejected(format!("{tool}: {detail}"))
}

fn required_string<'a>(args: &'a Value, field: &str) -> Result<&'a str> {
    args.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::GatewayError::RequestRejected(format!("ethos shadow: `{field}` is required"))
        })
}

fn compare_shadow(tool: &str, probe: &Value, served: &str) -> Result<()> {
    let served: Value = serde_json::from_str(served).map_err(|_| {
        crate::GatewayError::BridgeFailed("Ethos legacy response is invalid".into())
    })?;
    let matches = match tool {
        ETHOS_READ => ["context", "zone", "path", "text"]
            .into_iter()
            .all(|field| probe.get(field) == served.get(field)),
        ETHOS_LIST => {
            let mut client = probe["entries"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|entry| entry["kind"] == "section")
                .filter_map(|entry| {
                    Some((
                        entry["zone"].as_str()?.to_owned(),
                        entry["path"].as_str()?.to_owned(),
                    ))
                })
                .collect::<Vec<_>>();
            let mut legacy = served["contexts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|context| context["entries"].as_array().into_iter().flatten())
                .filter_map(|entry| {
                    Some((
                        entry["zone"].as_str()?.to_owned(),
                        entry["path"].as_str()?.to_owned(),
                    ))
                })
                .collect::<Vec<_>>();
            client.sort();
            legacy.sort();
            client == legacy
        }
        _ => true,
    };
    if matches {
        Ok(())
    } else {
        Err(crate::GatewayError::BridgeFailed(format!(
            "aithos-client shadow mismatch for {tool}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::routing::{get, post, put};
    use axum::Router;
    use std::sync::Mutex;

    #[test]
    fn the_ethos_seam_is_an_exact_six_tool_allowlist() {
        for tool in [
            ETHOS_READ,
            ETHOS_LIST,
            ETHOS_CONTEXT,
            ETHOS_CREATE,
            ETHOS_EDIT,
            ETHOS_DELETE,
        ] {
            assert!(EthosBackend::handles(tool), "{tool}");
        }
        for neighbor in [
            "ethos",
            "ethos.",
            "ethos.read.extra",
            "github__ethos.read",
            "neighbor.ethos.list",
            "briefing.read",
            "tools/list",
        ] {
            assert!(!EthosBackend::handles(neighbor), "{neighbor}");
        }
        assert!(EthosBackend::handles_legacy(ETHOS_READ));
        assert!(!EthosBackend::handles_legacy(ETHOS_CREATE));
    }

    #[test]
    fn backend_selection_is_opt_in_closed_and_rollback_safe() {
        assert!(matches!(
            ethos_backend_from_selector(None).as_deref(),
            Ok(EthosBackend::Legacy)
        ));
        assert!(matches!(
            ethos_backend_from_selector(Some("legacy")).as_deref(),
            Ok(EthosBackend::Legacy)
        ));
        assert!(matches!(
            ethos_backend_from_selector(Some("shadow")).as_deref(),
            Ok(EthosBackend::ClientShadow)
        ));
        assert!(matches!(
            ethos_backend_from_selector(Some("client-provider")).as_deref(),
            Ok(EthosBackend::ClientProvider)
        ));
        assert!(matches!(
            ethos_backend_from_selector(Some("client_provider")),
            Err(crate::GatewayError::ConfigRejected(_))
        ));
    }

    #[test]
    fn provider_transport_rejects_non_tls_and_credentialed_bases() {
        assert!(ProviderTransport::new("http://store.example").is_err());
        assert!(ProviderTransport::new("https://user:pass@store.example").is_err());
        assert!(ProviderTransport::new("https://store.example/prefix").is_err());
        assert!(ProviderTransport::new("http://127.0.0.1:14891").is_ok());
    }

    #[derive(Clone, Default)]
    #[allow(clippy::type_complexity)]
    struct Capture(Arc<Mutex<Vec<(String, HeaderMap, Vec<u8>)>>>);

    async fn capture_request(
        State(capture): State<Capture>,
        headers: HeaderMap,
        body: Bytes,
    ) -> ([(&'static str, &'static str); 1], &'static [u8]) {
        capture
            .0
            .lock()
            .unwrap()
            .push(("seen".into(), headers, body.to_vec()));
        ([("content-type", "application/octet-stream")], b"accepted")
    }

    #[tokio::test]
    async fn provider_transport_preserves_the_closed_signed_wire() {
        let capture = Capture::default();
        let app = Router::new()
            .route("/t/demo/did:aithos:test/heads", get(capture_request))
            .route("/t/demo/did:aithos:test/batch", post(capture_request))
            .route(
                "/t/demo/did:aithos:test/manifest.json",
                put(capture_request),
            )
            .with_state(capture.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let transport =
            ProviderTransport::new(&format!("http://127.0.0.1:{}", address.port())).unwrap();

        let response = transport
            .send(ProviderWireRequest {
                method: "PUT",
                path: "/t/demo/did:aithos:test/manifest.json",
                body: b"opaque-ciphertext",
                auth: "signed-envelope",
                if_head: Some("sha256:previous"),
            })
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(response.body(), b"accepted");
        let captured = capture.0.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].2, b"opaque-ciphertext");
        assert_eq!(captured[0].1["x-aithos-store"], STORE_WIRE_VERSION);
        assert_eq!(captured[0].1["x-aithos-auth"], "signed-envelope");
        assert_eq!(captured[0].1["if-head"], "sha256:previous");
    }

    #[test]
    fn shadow_comparison_is_semantic_and_detects_drift() {
        let probe = serde_json::json!({
            "context": "sales", "zone": "public", "path": "guide/welcome",
            "text": "hello", "edition_height": 7,
        });
        let served = serde_json::json!({
            "context": "sales", "zone": "public", "path": "guide/welcome",
            "text": "hello", "digest": "sha256:opaque",
        })
        .to_string();
        assert!(compare_shadow(ETHOS_READ, &probe, &served).is_ok());
        assert!(compare_shadow(
            ETHOS_READ,
            &probe,
            &served.replace("\"hello\"", "\"changed\"")
        )
        .is_err());
    }
}

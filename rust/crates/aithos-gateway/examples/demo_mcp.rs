//! Demo MCP server for the vault runbook (docs/DEMO-GATEWAY-VAULT.md).
//!
//! A deliberately tiny Streamable-HTTP MCP: advertises a fixed tool list,
//! answers every `tools/call` with a canned text, and PRINTS the
//! `Authorization` header it receives — so the demo shows, wire-side,
//! that the gateway presented the vault-resolved bearer. With
//! `--bearer <token>` it also ENFORCES that exact bearer (anything else
//! is refused 401), proving the credential is not decorative.
//!
//! Demo only: no TLS, no state, loopback usage. Never production.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use clap::Parser;
use serde_json::{json, Value};

#[derive(Parser, Clone)]
#[command(name = "demo-mcp", about = "Tiny MCP upstream for the vault demo")]
struct Args {
    /// Port to listen on (127.0.0.1 only).
    #[arg(long)]
    port: u16,
    /// Label printed with every request (e.g. github, linear).
    #[arg(long)]
    name: String,
    /// Comma-separated tool names to advertise.
    #[arg(long, default_value = "issues.list,issues.create")]
    tools: String,
    /// If set, REQUIRE exactly this bearer token on every request.
    #[arg(long)]
    bearer: Option<String>,
}

fn advertised(tools: &str) -> Vec<Value> {
    tools
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| {
            json!({
                "name": name,
                "description": format!("Demo tool `{name}`"),
                "inputSchema": { "type": "object", "additionalProperties": false }
            })
        })
        .collect()
}

async fn handle(
    State(args): State<Arc<Args>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let auth = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<none>");
    let method = body["method"].as_str().unwrap_or("<none>");
    let tool = body
        .pointer("/params/name")
        .and_then(Value::as_str)
        .unwrap_or("-");
    println!("[{}] {method} tool={tool} authorization={auth}", args.name);

    if let Some(expected) = &args.bearer {
        if auth != format!("Bearer {expected}") {
            println!("[{}] REFUSED: wrong or missing bearer", args.name);
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "unauthorized" })),
            );
        }
    }

    let id = body.get("id").cloned().unwrap_or(Value::Null);
    let response = if method == "tools/list" {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": advertised(&args.tools) }
        })
    } else {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{
                    "type": "text",
                    "text": format!("{}: `{tool}` ok", args.name)
                }],
                "isError": false
            }
        })
    };
    (StatusCode::OK, Json(response))
}

#[tokio::main]
async fn main() {
    let args = Arc::new(Args::parse());
    let app = Router::new()
        .route("/mcp", post(handle))
        .with_state(Arc::clone(&args));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", args.port))
        .await
        .expect("demo mcp binds");
    println!(
        "[{}] demo MCP listening on http://127.0.0.1:{}/mcp (tools: {}){}",
        args.name,
        args.port,
        args.tools,
        if args.bearer.is_some() {
            " — bearer ENFORCED"
        } else {
            ""
        }
    );
    axum::serve(listener, app).await.expect("demo mcp serves");
}

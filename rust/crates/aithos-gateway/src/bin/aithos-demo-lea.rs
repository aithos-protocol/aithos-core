//! Scripted CLI driver for beats 1–6 of docs/DEMO-LEA-SCENARIO.md.
//!
//! Beats 7–8 remain explicit owner/auditor gestures through the main
//! `aithos-gateway` CLI so the presenter can show the hot edit and proof.

use clap::Parser;
use serde_json::{json, Value};

#[derive(Parser)]
#[command(
    name = "aithos-demo-lea",
    about = "Run and verify the agent-facing beats of the Léa demo"
)]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:4890/mcp")]
    gateway: String,
    /// Text fragment expected in the governed briefing.
    #[arg(long, default_value = "DPE")]
    directive_contains: String,
    /// Run one beat only (1..=6). Omit to run all agent-facing beats.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=6))]
    beat: Option<u8>,
}

fn call(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

async fn rpc(client: &reqwest::Client, endpoint: &str, body: Value) -> Result<Value, String> {
    let response = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("gateway unreachable: {e}"))?;
    let status = response.status();
    let value = response
        .json::<Value>()
        .await
        .map_err(|e| format!("gateway returned non-JSON ({status}): {e}"))?;
    if !status.is_success() {
        return Err(format!("gateway HTTP {status}: {value}"));
    }
    Ok(value)
}

fn answer_text(value: &Value) -> &str {
    value
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("DEMO REFUSED: {error}");
            std::process::ExitCode::from(2)
        }
    }
}

async fn run(cli: Cli) -> Result<(), String> {
    let client = reqwest::Client::new();
    let selected = |beat| cli.beat.is_none() || cli.beat == Some(beat);

    if selected(1) {
        println!("[1/6] Surface mandatée");
        let init = rpc(
            &client,
            &cli.gateway,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        )
        .await?;
        let instructions = init
            .pointer("/result/instructions")
            .and_then(Value::as_str)
            .unwrap_or_default();
        require(
            instructions.contains("briefing.read") && instructions.contains("before"),
            format!("initialize did not recommend briefing.read: {init}"),
        )?;
        let listed = rpc(
            &client,
            &cli.gateway,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        )
        .await?;
        let mut names: Vec<&str> = listed
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str))
            .collect();
        names.sort_unstable();
        let expected = [
            "briefing.read",
            "calendar__create_event",
            "calendar__list_events",
            "gmail__search_emails",
            "gmail__send_email",
            "journal.search",
            "journal.write",
            "notion__query_database",
        ];
        require(
            names == expected,
            format!("unexpected tool surface: {names:?}"),
        )?;
        println!("  OK: {} outils, aucun outil refusé exposé", names.len());
    }

    if selected(2) {
        println!("[2/6] Prospects depuis Notion");
        let prospects = rpc(
            &client,
            &cli.gateway,
            call(3, "notion__query_database", json!({})),
        )
        .await?;
        let text = answer_text(&prospects);
        require(
            text.contains("prospects: a, b, c, d, e"),
            format!("Notion answer does not contain the five demo prospects: {prospects}"),
        )?;
        println!("  OK: a, b, c, d, e");
    }

    if selected(3) {
        println!("[3/6] Refus pédagogique Gmail (5 destinataires)");
        let refused = rpc(
            &client,
            &cli.gateway,
            call(
                4,
                "gmail__send_email",
                json!({
                    "to": ["a", "b", "c", "d", "e"],
                    "subject": "Prise de rendez-vous",
                    "body": "Bonjour"
                }),
            ),
        )
        .await?;
        let refusal = refused
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        require(
            ["bound violated", "send_email.to", "d", "e", "a, b, c"]
                .iter()
                .all(|needle| refusal.contains(needle)),
            format!("Gmail refusal is not pedagogical: {refused}"),
        )?;
        println!("  OK: d/e refusés avant relais");
    }

    if selected(4) {
        println!("[4/6] Auto-correction Gmail (3 destinataires)");
        let sent = rpc(
            &client,
            &cli.gateway,
            call(
                5,
                "gmail__send_email",
                json!({
                    "to": ["a", "b", "c"],
                    "subject": "Prise de rendez-vous — visite du bien",
                    "body": "Bonjour, proposons un créneau."
                }),
            ),
        )
        .await?;
        require(
            sent.get("error").is_none(),
            format!("corrected send failed: {sent}"),
        )?;
        println!("  OK: appel relayé");
    }

    if selected(5) {
        println!("[5/6] Créneaux Calendar");
        let outside = rpc(
            &client,
            &cli.gateway,
            call(
                6,
                "calendar__create_event",
                json!({ "start": "2026-07-15T10:00:00+02:00", "title": "Visite du bien" }),
            ),
        )
        .await?;
        let outside_message = outside
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or_default();
        require(
            ["bound violated", "tuesday", "thursday", "14:00"]
                .iter()
                .all(|needle| outside_message.contains(needle)),
            format!("outside slot was not refused pedagogically: {outside}"),
        )?;
        let inside = rpc(
            &client,
            &cli.gateway,
            call(
                7,
                "calendar__create_event",
                json!({ "start": "2026-07-16T15:00:00+02:00", "title": "Visite du bien" }),
            ),
        )
        .await?;
        require(
            inside.get("error").is_none(),
            format!("valid slot failed: {inside}"),
        )?;
        println!("  OK: mercredi refusé, jeudi accepté");
    }

    if selected(6) {
        println!("[6/6] Briefing gouverné");
        let briefing = rpc(&client, &cli.gateway, call(8, "briefing.read", json!({}))).await?;
        let briefing_text = answer_text(&briefing);
        require(
            briefing_text.contains(&cli.directive_contains),
            format!(
                "briefing does not contain {:?}: {briefing}",
                cli.directive_contains
            ),
        )?;
        require(
            !briefing_text.contains("Marge de négociation interne"),
            "owner-only self note leaked through briefing.read",
        )?;
        println!("  OK: directive circle servie, note self absente");
    }

    println!();
    if let Some(beat) = cli.beat {
        println!("Beat {beat}: GREEN");
    } else {
        println!("Beats agent 1–6: GREEN");
        println!(
            "Suite CLI: hot edit owner (beat 7), puis audit-export action/ethos.read (beat 8)."
        );
    }
    Ok(())
}

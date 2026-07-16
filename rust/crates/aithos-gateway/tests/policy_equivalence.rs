//! Non-regression by construction (M2): the pure effective-policy
//! functions must give EXACTLY the runtime's verdicts. Every case the
//! grants/bounds contracts exercise is replayed here through BOTH
//! doors — `owner_preview_call` (pure, owner-side, files alone) and
//! the runtime pair `Runner::authorize` + `Runner::check_bounds` (the
//! very calls `tool_call_multi` makes before relaying) — and the two
//! answers are required to be literally equal (code AND message).
//! The hot path is NOT rebranched in this lot; this file is the proof
//! that rebranching later cannot change a single verdict.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use aithos_gateway::config::GatewayConfig;
use aithos_gateway::core_bridge::{
    owner_enroll_server, owner_init_context, owner_init_journal, owner_preview_call,
    owner_preview_mandate, EntropySource, MandateWindow, Runner, SeqEntropy,
    EFFECTIVE_POLICY_VERSION,
};
use aithos_gateway::hub::{
    approve_manifest, ApprovedManifest, ArgumentBound, ProposedManifest, ProposedTool,
    ToolApproval, MANIFEST_VERSION,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::GatewayError;

const T0: &str = "2026-07-10T12:00:00Z";
const NOT_BEFORE: &str = "2026-07-10T00:00:00Z";
const NOT_AFTER: &str = "2026-08-09T00:00:00Z";
const AFTER_EXPIRY: &str = "2026-08-15T12:00:00Z";
const MASTER: [u8; 32] = [7u8; 32];
const SERVER: &str = "acme";
const CONTEXT: &str = "ventes";

fn tool(name: &str, description: &str, schema: Value) -> ProposedTool {
    ProposedTool {
        name: name.to_owned(),
        description: Some(description.to_owned()),
        pin_sha256: aithos_gateway::core_bridge::manifest_tool_pin(
            name,
            Some(description),
            &schema,
        )
        .expect("pin computes"),
        input_schema: schema,
    }
}

/// The proposed universe: every shape the grants/bounds scenarios use,
/// on one server. Schemas mirror the cucumber fixtures.
fn proposed() -> ProposedManifest {
    ProposedManifest {
        version: MANIFEST_VERSION.to_owned(),
        server: SERVER.to_owned(),
        tools: vec![
            tool(
                "search_emails",
                "Search the mailbox",
                json!({
                    "type": "object",
                    "properties": { "query": { "type": "string" } },
                    "additionalProperties": false
                }),
            ),
            tool(
                "send_email",
                "Send an email",
                json!({
                    "type": "object",
                    "properties": {
                        "to": { "type": "array", "items": { "type": "string" } },
                        "bcc": { "type": "array", "items": { "type": "string" } },
                        "subject": { "type": "string" },
                        "body": { "type": "string" }
                    },
                    "required": ["to"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "create_event",
                "Create one calendar event",
                json!({
                    "type": "object",
                    "properties": {
                        "start": { "type": "string" },
                        "title": { "type": "string" }
                    },
                    "required": ["start"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "repo_admin",
                "Administer one repository item",
                json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string" },
                        "target": { "type": "string" }
                    },
                    "required": ["action"],
                    "additionalProperties": false
                }),
            ),
            tool(
                "archive_mail",
                "Archive a thread",
                json!({ "type": "object", "additionalProperties": false }),
            ),
            tool(
                "peek_inbox",
                "Peek at the inbox",
                json!({ "type": "object", "additionalProperties": false }),
            ),
            tool(
                "list_labels",
                "List the labels",
                json!({ "type": "object", "additionalProperties": false }),
            ),
            tool(
                "purge_all",
                "Purge everything",
                json!({ "type": "object", "additionalProperties": false }),
            ),
        ],
    }
}

/// The owner decisions of the grants/bounds contracts, in one manifest:
/// explicit grants and denials, safe class-only defaults, every bound
/// family on a granted write.
fn approvals() -> BTreeMap<String, ToolApproval> {
    use aithos_gateway::config::ToolAccess::{Read, Write};
    BTreeMap::from([
        ("search_emails".to_owned(), ToolApproval::granted(Read)),
        (
            "send_email".to_owned(),
            ToolApproval::granted(Write).with_bounds(vec![
                ArgumentBound::OneOf {
                    field: "to".into(),
                    values: vec![
                        "prospect-a@clients.example".into(),
                        "prospect-b@clients.example".into(),
                        "prospect-c@clients.example".into(),
                    ],
                },
                ArgumentBound::Forbid {
                    field: "bcc".into(),
                },
                ArgumentBound::Require {
                    field: "subject".into(),
                },
                ArgumentBound::MaxItems {
                    field: "to".into(),
                    max: 2,
                },
            ]),
        ),
        (
            "create_event".to_owned(),
            ToolApproval::granted(Write).with_bounds(vec![ArgumentBound::TimeSlots {
                field: "start".into(),
                days: vec!["tuesday".into(), "thursday".into()],
                from: "14:00".into(),
                to: "18:00".into(),
            }]),
        ),
        (
            "repo_admin".to_owned(),
            ToolApproval::granted(Write).with_bounds(vec![ArgumentBound::OneOf {
                field: "action".into(),
                values: vec!["comment".into()],
            }]),
        ),
        ("archive_mail".to_owned(), ToolApproval::denied(Write)),
        ("peek_inbox".to_owned(), ToolApproval::denied(Read)),
        // Class-only approvals: the historic safe defaults decide —
        // reads granted, writes denied.
        ("list_labels".to_owned(), ToolApproval::class(Read)),
        ("purge_all".to_owned(), ToolApproval::class(Write)),
    ])
}

struct Harness {
    _dir: tempfile::TempDir,
    store: aithos_gateway::store_adapter::GatewayStore,
    runner: Runner,
}

fn provision() -> Harness {
    let dir = tempfile::tempdir().expect("tempdir");
    let context_root = dir.path().join(CONTEXT);
    let journal_root = dir.path().join("journal");
    let store_cfg = |root: &std::path::Path| aithos_gateway::config::StoreConfig::Fs {
        root: root.to_owned(),
    };
    let store = aithos_gateway::store_adapter::GatewayStore::from_config(&store_cfg(&context_root))
        .expect("context store");
    let journal_store =
        aithos_gateway::store_adapter::GatewayStore::from_config(&store_cfg(&journal_root))
            .expect("journal store");
    let mut kh_ent = SeqEntropy::default();
    let keyholder = Keyholder::from_entropy(kh_ent.e32(), kh_ent.e32());
    let agent_pub = aithos_gateway::core_bridge::agent_pub_multibase(&keyholder);
    let gateway_pub = aithos_gateway::core_bridge::gateway_pub_multibase(&keyholder);
    let window = MandateWindow {
        not_before: NOT_BEFORE.to_owned(),
        not_after: NOT_AFTER.to_owned(),
    };
    let mut ent = SeqEntropy::default();
    owner_init_context(&MASTER, CONTEXT, store.clone(), T0, &mut ent).expect("context created");
    let approved = approve_manifest(&proposed(), &approvals()).expect("approval");
    owner_enroll_server(
        &MASTER,
        CONTEXT,
        &agent_pub,
        &gateway_pub,
        &approved,
        store.clone(),
        &window,
        T0,
        &mut ent,
    )
    .expect("enrollment");
    owner_init_journal(
        &MASTER,
        "leo",
        &agent_pub,
        &gateway_pub,
        None,
        journal_store,
        &window,
        T0,
        &mut ent,
    )
    .expect("journal");
    let cfg = GatewayConfig::from_yaml(&config_text(&context_root, &journal_root, &approved))
        .expect("config parses");
    let runner =
        Runner::open(&cfg, keyholder, || Box::new(SeqEntropy::default())).expect("runner opens");
    Harness {
        _dir: dir,
        store,
        runner,
    }
}

fn config_text(
    context_root: &std::path::Path,
    journal_root: &std::path::Path,
    approved: &ApprovedManifest,
) -> String {
    let quote =
        |path: &std::path::Path| serde_json::to_string(&path.display().to_string()).unwrap();
    let mut tools = String::new();
    for tool in &approved.tools {
        let access = match tool.risk_class {
            aithos_gateway::config::ToolAccess::Read => "read",
            aithos_gateway::config::ToolAccess::Write => "write",
        };
        tools.push_str(&format!(
            "      {}: {{ server: {SERVER}, tool: {}, access: {access}, granted: {} }}\n",
            tool.exposed_name,
            tool.name,
            tool.is_granted()
        ));
    }
    format!(
        "listen: 127.0.0.1:4879\nservers:\n  - name: {SERVER}\n    transport: http\n    url: https://acme.invalid/mcp\ncontexts:\n  - name: {CONTEXT}\n    store: {{ kind: fs, root: {} }}\n    tools:\n{tools}journal:\n  store: {{ kind: fs, root: {} }}\n",
        quote(context_root),
        quote(journal_root)
    )
}

/// The runtime's pre-relay decision, exactly as `tool_call_multi` makes
/// it: resolve (default-deny) → mandate at T → owner bounds. Drift is
/// skipped on purpose — it observes the upstream, it is not policy.
fn runtime_verdict(
    runner: &Runner,
    tool: &str,
    args: &Value,
    at: &str,
) -> Result<(), GatewayError> {
    let Some(ctx) = runner.resolve(tool).map(str::to_owned) else {
        return Err(GatewayError::ToolNotMapped(tool.to_owned()));
    };
    runner.authorize(&ctx, tool, at)?;
    runner.check_bounds(tool, args)
}

/// One replayed case: the pure preview and the live runtime must agree
/// literally — same verdict, same refusal code, same message.
fn assert_equivalent(h: &Harness, tool: &str, args: Value, at: &str) {
    let preview = owner_preview_call(
        &MASTER,
        CONTEXT,
        &[SERVER.to_owned()],
        h.store.clone(),
        tool,
        &args,
        at,
    )
    .expect("the preview computes");
    match runtime_verdict(&h.runner, tool, &args, at) {
        Ok(()) => {
            assert_eq!(
                preview["verdict"], "allowed",
                "runtime allowed `{tool}`, preview said: {preview}"
            );
        }
        Err(e) => {
            assert_eq!(
                preview["verdict"], "refused",
                "runtime refused `{tool}` ({e}), preview said: {preview}"
            );
            assert_eq!(
                preview["code"],
                e.refusal_code(),
                "same refusal code for `{tool}`"
            );
            assert_eq!(
                preview["detail"],
                e.to_string(),
                "same refusal message for `{tool}`"
            );
        }
    }
}

#[test]
fn granted_calls_inside_policy_are_allowed_identically() {
    let h = provision();
    assert_equivalent(&h, "acme__search_emails", json!({ "query": "dpe" }), T0);
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": ["prospect-a@clients.example"], "subject": "Visite", "body": "Bonjour" }),
        T0,
    );
    // An absent optional bounded field passes both doors.
    assert_equivalent(&h, "acme__send_email", json!({ "subject": "s" }), T0);
    // Inside the approved time slots (2026-07-16 is a thursday).
    assert_equivalent(
        &h,
        "acme__create_event",
        json!({ "start": "2026-07-16T15:00:00+02:00", "title": "Visite" }),
        T0,
    );
    // The class-only read rides the safe default.
    assert_equivalent(&h, "acme__list_labels", json!({}), T0);
}

#[test]
fn denials_and_defaults_are_refused_identically() {
    let h = provision();
    // Explicitly denied write and read, class-default denied write:
    // known, hidden, precisely refused.
    assert_equivalent(&h, "acme__archive_mail", json!({}), T0);
    assert_equivalent(&h, "acme__peek_inbox", json!({}), T0);
    assert_equivalent(&h, "acme__purge_all", json!({}), T0);
    // Unknown everywhere: default-deny.
    assert_equivalent(&h, "acme__delete_email", json!({}), T0);
    assert_equivalent(&h, "unrelated_tool", json!({}), T0);
}

#[test]
fn every_bound_family_is_refused_identically() {
    let h = provision();
    // one_of: an intruder among approved recipients, offenders named.
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": ["prospect-a@clients.example", "mallory@evil.example"], "subject": "s" }),
        T0,
    );
    // forbid: the bcc field is present.
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": ["prospect-a@clients.example"], "subject": "s", "bcc": ["x@y"] }),
        T0,
    );
    // require: the subject is missing.
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": ["prospect-a@clients.example"] }),
        T0,
    );
    // max_items: one recipient too many (whitelisted, still too many).
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({
            "to": [
                "prospect-a@clients.example",
                "prospect-b@clients.example",
                "prospect-c@clients.example"
            ],
            "subject": "s"
        }),
        T0,
    );
    // Pinned-schema shape: an array-typed field arriving as a string.
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": "prospect-a@clients.example", "subject": "s" }),
        T0,
    );
    // time_slots: a wednesday morning outside tuesday/thursday 14-18.
    assert_equivalent(
        &h,
        "acme__create_event",
        json!({ "start": "2026-07-15T10:00:00+02:00", "title": "Visite" }),
        T0,
    );
    // Polymorphic one_of: the sub-action is not whitelisted.
    assert_equivalent(
        &h,
        "acme__repo_admin",
        json!({ "action": "merge", "target": "pr-42" }),
        T0,
    );
}

#[test]
fn the_expired_window_is_refused_identically() {
    let h = provision();
    assert_equivalent(
        &h,
        "acme__search_emails",
        json!({ "query": "dpe" }),
        AFTER_EXPIRY,
    );
    assert_equivalent(
        &h,
        "acme__send_email",
        json!({ "to": ["prospect-a@clients.example"], "subject": "s" }),
        AFTER_EXPIRY,
    );
}

#[test]
fn the_preview_read_model_tells_the_whole_story() {
    let h = provision();
    let preview =
        owner_preview_mandate(&MASTER, CONTEXT, &[SERVER.to_owned()], h.store.clone(), T0)
            .expect("the preview computes");
    assert_eq!(preview["version"], EFFECTIVE_POLICY_VERSION);
    assert_eq!(preview["at"], T0);
    let mandate = &preview["mandate"];
    assert_eq!(mandate["status"], "active");
    assert_eq!(mandate["not_before"], NOT_BEFORE);
    assert_eq!(mandate["not_after"], NOT_AFTER);
    let tools = preview["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 8, "every manifest tool is described");
    let by_name = |name: &str| {
        tools
            .iter()
            .find(|t| t["tool"] == json!(name))
            .unwrap_or_else(|| panic!("`{name}` described"))
    };
    let send = by_name("acme__send_email");
    assert_eq!(send["granted"], json!(true));
    assert_eq!(send["covered"], json!(true));
    assert_eq!(send["served"], json!(true));
    assert_eq!(send["bounds"].as_array().expect("bounds").len(), 4);
    let purged = by_name("acme__purge_all");
    assert_eq!(purged["granted"], json!(false), "class-only write default");
    assert_eq!(purged["covered"], json!(false), "outside the mandate");
    assert_eq!(purged["served"], json!(false));
    let peek = by_name("acme__peek_inbox");
    assert_eq!(peek["granted"], json!(false), "a read can be denied too");
    assert_eq!(peek["served"], json!(false));
    // At an expired instant the same read-model reports it.
    let expired = owner_preview_mandate(
        &MASTER,
        CONTEXT,
        &[SERVER.to_owned()],
        h.store.clone(),
        AFTER_EXPIRY,
    )
    .expect("the expired preview computes");
    assert_eq!(expired["mandate"]["status"], "expired");
    let send_expired = expired["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|t| t["tool"] == json!("acme__send_email"))
        .expect("send described")
        .clone();
    assert_eq!(
        send_expired["covered"],
        json!(true),
        "the cert still names it"
    );
    assert_eq!(
        send_expired["served"],
        json!(false),
        "but nothing is served"
    );
}

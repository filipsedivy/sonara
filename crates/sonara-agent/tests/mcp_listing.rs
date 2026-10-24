//! The MCP view of an in-process agent service (CTR-01):
//! what `tools/list` and `tools/call` carry, checked against `golden/contracts/mcp-tools-list.json`.
//! The stdio round trip with the built binary is `apps/sonara/tests/commands/mcp_stdio.rs`.

use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::mcp::{ToolBackend, call_result, dumps, listed_tool};
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::settings::Settings;
use sonara_testkit::Golden;
use tempfile::TempDir;

fn service(dir: &TempDir) -> AgentService {
    let app = AppContext::create(AppOptions {
        workspaces: Some(dir.path().join("workspaces")),
        settings: Some(Settings::default()),
        persist_settings: false,
        audio_backend: Some("null".to_owned()),
        ..AppOptions::default()
    })
    .expect("the application starts");
    AgentService::new(
        Arc::new(app),
        AgentOptions {
            base_dir: Some(dir.path().to_path_buf()),
            audit: AuditTarget::Disabled,
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: dir.path().join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the agent service starts")
}

#[test]
fn the_listing_of_the_service_equals_the_golden_listing() {
    let golden: Value = Golden::open()
        .and_then(|golden| golden.json("contracts/mcp-tools-list.json"))
        .unwrap();
    let recorded = golden["tools"].as_array().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let service = service(&dir);
    let listed: Vec<Value> = ToolBackend::tools(&service)
        .unwrap()
        .iter()
        .map(listed_tool)
        .collect();
    for tool in &listed {
        let expected = recorded.iter().find(|t| t["name"] == tool["name"]).unwrap();
        assert_eq!(tool, expected);
    }
    // The prototype listed its registry, every tool in registration order.
    assert_eq!(
        listed.iter().collect::<Vec<_>>(),
        recorded.iter().collect::<Vec<_>>()
    );
}

#[test]
fn call_results_wrap_the_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(&dir);
    let envelope =
        ToolBackend::call(&service, "create_project", Some(&json!({"name": "Č"}))).unwrap();
    assert_eq!(envelope["ok"], true);
    let result = call_result(&envelope);
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"], envelope);
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(text, dumps(&envelope, 1));
    assert!(text.is_ascii() && text.contains("\\u010c"));
    let failed = ToolBackend::call(&service, "no_such_tool", None).unwrap();
    assert_eq!(failed["error"]["code"], "NOT_FOUND");
    assert_eq!(call_result(&failed)["isError"], true);
}

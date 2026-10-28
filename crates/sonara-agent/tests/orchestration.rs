//! Conversational orchestration (SON-P13) against a stub tool runner, so no model and no project
//! are needed, and once against the real service to show the tools' own validation still applies.

use std::cell::RefCell;

use serde_json::{Value, json};
use sonara_agent::orchestration::{Approval, Orchestrator, Status, ToolRunner};

/// Answers the read tools with a fixed project and records every call.
struct Stub {
    calls: RefCell<Vec<(String, Value)>>,
    refuse: Option<&'static str>,
}

impl Stub {
    fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            refuse: None,
        }
    }

    fn changes(&self) -> Vec<(String, Value)> {
        self.calls
            .borrow()
            .iter()
            .filter(|(tool, args)| {
                !matches!(tool.as_str(), "list_tracks" | "list_plugins")
                    && args["dry_run"] != json!(true)
            })
            .cloned()
            .collect()
    }
}

impl ToolRunner for &Stub {
    fn call(&self, tool: &str, args: &Value) -> Value {
        self.calls
            .borrow_mut()
            .push((tool.to_owned(), args.clone()));
        match tool {
            "list_tracks" => json!({"ok": true, "tracks": [
                {"track_id": "trk_1", "name": "Keys", "kind": "instrument"},
                {"track_id": "trk_2", "name": "Keys 2", "kind": "instrument"},
                {"track_id": "trk_3", "name": "Drums", "kind": "instrument"},
            ]}),
            "list_plugins" => json!({"ok": true, "plugins": [
                {"plugin_id": "sonara.synth.poly", "name": "Poly Synth", "kind": "instrument"},
                {"plugin_id": "sonara.synth.bass", "name": "Bass Synth", "kind": "instrument"},
                {"plugin_id": "sonara.utility", "name": "Utility", "kind": "effect"},
            ]}),
            _ if self.refuse == Some(tool) && args["dry_run"] == json!(true) => {
                json!({"ok": false, "error": {"code": "INVALID_ARGUMENT"}})
            }
            _ => json!({"ok": true}),
        }
    }
}

fn run(stub: &Stub, request: &str, approval: Approval) -> sonara_agent::orchestration::Report {
    Orchestrator::new(stub).run(request, approval)
}

#[test]
fn a_tempo_request_becomes_one_set_tempo_call() {
    let stub = Stub::new();
    let report = run(&stub, "Please set the tempo to 96 bpm", Approval::Pending);
    assert_eq!(report.status, Status::Done);
    assert_eq!(
        stub.changes(),
        [("set_tempo".to_owned(), json!({"bpm": 96.0}))]
    );
    assert!(report.summary.contains("96"), "{}", report.summary);
    assert_eq!(report.executed.len(), 1);
}

#[test]
fn an_instrument_request_resolves_the_track_and_the_plugin() {
    let stub = Stub::new();
    let report = run(&stub, "use Bass Synth on Keys 2", Approval::Pending);
    assert_eq!(report.status, Status::Done, "{}", report.summary);
    // "Keys 2" is not mistaken for "Keys".
    assert_eq!(
        stub.changes(),
        [(
            "set_instrument".to_owned(),
            json!({"track_id": "trk_2", "plugin_id": "sonara.synth.bass"})
        )]
    );
}

#[test]
fn several_requests_run_in_order() {
    let stub = Stub::new();
    let report = run(
        &stub,
        "set tempo to 100 and use Poly Synth on Keys",
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Done);
    let tools: Vec<String> = stub.changes().into_iter().map(|(t, _)| t).collect();
    assert_eq!(tools, ["set_tempo", "set_instrument"]);
}

#[test]
fn an_ambiguous_request_changes_nothing() {
    for request in [
        "make it faster",
        "set the tempo to 5000",
        "set tempo to 90 or 120",
        "use Poly Synth",
        "use Poly Synth on Violin",
        "dance",
        "",
    ] {
        let stub = Stub::new();
        let report = run(&stub, request, Approval::Confirmed);
        assert_eq!(
            report.status,
            Status::NeedsClarification,
            "{request:?}: {}",
            report.summary
        );
        assert!(!report.plan.questions.is_empty(), "{request:?}");
        assert!(stub.changes().is_empty(), "{request:?} changed something");
        assert_eq!(report.executed, []);
    }
}

#[test]
fn one_unclear_part_stops_the_whole_request() {
    let stub = Stub::new();
    let report = run(
        &stub,
        "set tempo to 100 and make the drums punchier",
        Approval::Confirmed,
    );
    assert_eq!(report.status, Status::NeedsClarification);
    assert!(
        stub.changes().is_empty(),
        "the clear half must not run alone"
    );
}

#[test]
fn a_destructive_plan_waits_for_confirmation() {
    let stub = Stub::new();
    let report = run(
        &stub,
        "set tempo to 100 and delete track Drums",
        Approval::Pending,
    );
    assert_eq!(report.status, Status::NeedsConfirmation);
    assert!(report.plan.is_destructive());
    assert!(report.summary.contains("Drums"));
    assert!(
        stub.changes().is_empty(),
        "not even the safe step runs before confirmation"
    );

    let confirmed = run(
        &stub,
        "set tempo to 100 and delete track Drums",
        Approval::Confirmed,
    );
    assert_eq!(confirmed.status, Status::Done);
    let tools: Vec<String> = stub.changes().into_iter().map(|(t, _)| t).collect();
    assert_eq!(tools, ["set_tempo", "delete_track"]);
}

#[test]
fn a_refused_step_in_validation_changes_nothing() {
    let mut stub = Stub::new();
    stub.refuse = Some("set_instrument");
    let report = run(
        &stub,
        "set tempo to 100 and use Poly Synth on Keys",
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Rejected);
    assert_eq!(report.executed.len(), 1);
    assert_eq!(
        report.executed[0].error.as_deref(),
        Some("INVALID_ARGUMENT")
    );
    assert!(
        stub.changes().is_empty(),
        "the valid tempo step must not be committed alone"
    );
}

#[test]
fn the_tool_layers_confirmation_token_is_replayed_after_approval() {
    struct Tokens(RefCell<Vec<Value>>);
    impl ToolRunner for &Tokens {
        fn call(&self, tool: &str, args: &Value) -> Value {
            match tool {
                "list_tracks" => {
                    json!({"ok": true, "tracks": [{"track_id": "trk_3", "name": "Drums", "kind": "instrument"}]})
                }
                "list_plugins" => json!({"ok": true, "plugins": []}),
                _ if args["dry_run"] == json!(true) => json!({"ok": true}),
                _ if args.get("confirmation_token").is_none() => {
                    self.0.borrow_mut().push(args.clone());
                    json!({"ok": false, "error": {"code": "CONFIRMATION_REQUIRED", "details": {"confirmation_token": "tok"}}})
                }
                _ => {
                    self.0.borrow_mut().push(args.clone());
                    json!({"ok": true})
                }
            }
        }
    }
    let tokens = Tokens(RefCell::new(Vec::new()));
    let report = Orchestrator::new(&tokens).run("delete track Drums", Approval::Confirmed);
    assert_eq!(report.status, Status::Done, "{}", report.summary);
    let sent = tokens.0.borrow();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[1],
        json!({"track_id": "trk_3", "confirmation_token": "tok"})
    );
}

#[test]
fn a_failing_step_stops_the_plan_and_reports_what_was_applied() {
    struct FailSecond;
    impl ToolRunner for FailSecond {
        fn call(&self, tool: &str, args: &Value) -> Value {
            match tool {
                "list_tracks" => {
                    json!({"ok": true, "tracks": [{"track_id": "trk_1", "name": "Keys", "kind": "instrument"}]})
                }
                "list_plugins" => {
                    json!({"ok": true, "plugins": [{"plugin_id": "p", "name": "Poly Synth", "kind": "instrument"}]})
                }
                "set_instrument" if args["dry_run"] != json!(true) => {
                    json!({"ok": false, "error": {"code": "ENGINE_UNAVAILABLE"}})
                }
                _ => json!({"ok": true}),
            }
        }
    }
    let report = Orchestrator::new(FailSecond).run(
        "set tempo to 100 and use Poly Synth on Keys",
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    assert_eq!(report.executed.len(), 2);
    assert!(report.executed[0].ok && !report.executed[1].ok);
    assert!(
        report.summary.contains("set the tempo to 100"),
        "{}",
        report.summary
    );
}

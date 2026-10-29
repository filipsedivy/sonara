//! The agent cannot get around the project's validation, and every change is audited (SON-P12):
//! a rejected change tool leaves the project, its version and its undo history as they were, a
//! dry run commits nothing, and the audit log records each attempt with its outcome.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    service: AgentService,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("the directory resolves");
        for sub in ["work", "projects", "workspaces", "logs"] {
            fs::create_dir(root.join(sub)).expect("a folder");
        }
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(root.join("workspaces")),
                settings: Some(Settings::default()),
                persist_settings: false,
                audio_backend: Some("null".to_owned()),
                ids: Some(IdSource::new(Box::new(SequentialIds::new(0)))),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        app.projects()
            .new_project(&NewProject {
                template: "empty".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        let service = AgentService::new(
            app,
            AgentOptions {
                base_dir: Some(root.join("work")),
                audit: AuditTarget::File(root.join("logs/agent-audit.jsonl")),
                tokens: Some(shared_tokens(sequential_tokens())),
                environment: Some(PolicyEnvironment {
                    allowed: None,
                    projects_dir: root.join("projects"),
                }),
                ..AgentOptions::default()
            },
        )
        .expect("the service");
        Self {
            _dir: dir,
            root,
            service,
        }
    }

    fn call(&self, tool: &str, args: &Value) -> Value {
        self.service.call(tool, Some(args))
    }

    /// The project as a reader sees it: its version and its tracks.
    fn snapshot(&self) -> Value {
        let state = self.call("get_project_state", &json!({}));
        assert_eq!(state["ok"], true, "{state}");
        json!({
            "version": state["project_version"],
            "tracks": self.call("list_tracks", &json!({}))["tracks"],
        })
    }

    fn audit(&self) -> Vec<Value> {
        fs::read_to_string(self.root.join("logs/agent-audit.jsonl"))
            .expect("the audit log")
            .lines()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    }
}

/// Change calls that the validation must refuse, with the error code each gets: a schema error
/// is `INVALID_ARGUMENT`, a well-formed call about something absent is `NOT_FOUND`.
fn invalid_changes() -> Vec<(&'static str, Value, &'static str)> {
    let schema = "INVALID_ARGUMENT";
    vec![
        ("set_tempo", json!({"bpm": -10.0}), schema),
        ("set_tempo", json!({"bpm": 5000.0}), schema),
        ("set_tempo", json!({"bpm": "fast"}), schema),
        (
            "set_tempo",
            json!({"bpm": 120.0, "bypass_validation": true}),
            schema,
        ),
        (
            "create_track",
            json!({"name": "X", "kind": "theremin"}),
            schema,
        ),
        (
            "add_midi_notes",
            json!({"clip_id": "clp_missing", "notes": []}),
            schema,
        ),
        (
            "set_track_volume",
            json!({"track_id": "master", "volume_db": 500.0}),
            schema,
        ),
        (
            "rename_track",
            json!({"track_id": "trk_missing", "name": "Y"}),
            "NOT_FOUND",
        ),
        (
            "set_track_volume",
            json!({"track_id": "trk_missing", "volume_db": 0.0}),
            "NOT_FOUND",
        ),
    ]
}

#[test]
fn a_rejected_change_leaves_the_project_untouched() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    for (tool, args, code) in invalid_changes() {
        let reply = fixture.call(tool, &args);
        assert_eq!(reply["ok"], false, "{tool} {args}: {reply}");
        assert_eq!(reply["error"]["code"], code, "{tool} {args}: {reply}");
        assert_eq!(
            fixture.snapshot(),
            before,
            "{tool} {args} changed the project"
        );
    }
    // Nothing was committed, so there is nothing to undo.
    assert_eq!(fixture.call("undo", &json!({}))["ok"], false);
}

#[test]
fn a_dry_run_validates_without_committing() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let reply = fixture.call(
        "create_track",
        &json!({"name": "Keys", "kind": "instrument", "dry_run": true}),
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_eq!(fixture.snapshot(), before);
    // The same arguments without the flag commit.
    let reply = fixture.call(
        "create_track",
        &json!({"name": "Keys", "kind": "instrument"}),
    );
    assert_eq!(reply["ok"], true, "{reply}");
    assert_ne!(fixture.snapshot(), before);
}

#[test]
fn every_change_attempt_is_audited_with_its_outcome() {
    let fixture = Fixture::new();
    let attempts = invalid_changes();
    for (tool, args, _) in &attempts {
        fixture.call(tool, args);
    }
    let accepted = fixture.call(
        "create_track",
        &json!({"name": "Keys", "kind": "instrument"}),
    );
    assert_eq!(accepted["ok"], true, "{accepted}");

    let log = fixture.audit();
    assert_eq!(log.len(), attempts.len() + 1, "one record per call");
    for (record, (tool, _, code)) in log.iter().zip(&attempts) {
        assert_eq!(record["tool"], *tool);
        assert_eq!(record["ok"], false, "{record}");
        assert_eq!(
            record["error"], *code,
            "a refusal is audited with its error code: {record}"
        );
        assert!(record["ts"].is_string());
    }
    let last = log.last().expect("the accepted call");
    assert_eq!(last["tool"], "create_track");
    assert_eq!(last["ok"], true);
    assert_eq!(last["source"], "agent");
}

#[test]
fn an_unknown_tool_is_refused_and_changes_nothing() {
    // The registry is the whole surface: a name outside it reaches no handler, so a tool that
    // would write project files directly does not exist. As in the prototype, the call is
    // refused before dispatch and is not an audit record (only dispatched calls are).
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let reply = fixture.call("write_project_file", &json!({"path": "/etc/passwd"}));
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(reply["error"]["code"], "NOT_FOUND", "{reply}");
    assert_eq!(fixture.snapshot(), before);
}

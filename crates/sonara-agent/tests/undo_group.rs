//! One undo step for a whole agent run (the AI composer, `sonara_app::edit::begin_undo_group`),
//! through the agent service that the run's `sonara mcp` talks to. The tool schemas, their
//! semantics and the error codes stay as they are (CTR-01); only the history changes.

use std::fs;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::edit;
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

struct World {
    _dir: TempDir,
    app: Arc<AppContext>,
    service: AgentService,
}

impl World {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = fs::canonicalize(dir.path()).expect("the directory resolves");
        for sub in ["work", "projects", "workspaces"] {
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
        let service = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root.join("work")),
                audit: AuditTarget::Disabled,
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
            app,
            service,
        }
    }

    fn ok(&self, tool: &str, args: &Value) -> Value {
        let reply = self.service.call(tool, Some(args));
        assert_eq!(reply["ok"], true, "{tool}: {reply}");
        reply
    }

    fn track_names(&self) -> Vec<String> {
        self.ok("list_tracks", &json!({}))["tracks"]
            .as_array()
            .expect("tracks")
            .iter()
            .map(|t| t["name"].as_str().expect("a name").to_owned())
            .collect()
    }

    fn undo_label(&self) -> Option<String> {
        self.app
            .session()
            .lock()
            .history()
            .undo_label()
            .map(str::to_owned)
    }
}

#[test]
fn a_run_of_tool_calls_is_one_undo_step_and_the_undo_tool_still_works_inside_it() {
    let world = World::new();
    world.ok("create_project", &json!({"name": "Run", "bpm": 120}));
    world.ok(
        "create_track",
        &json!({"name": "Mine", "kind": "instrument"}),
    );
    let before = world.track_names();

    edit::begin_undo_group(&world.app);
    for name in ["Pads", "Bass", "Lead"] {
        world.ok("create_track", &json!({"name": name, "kind": "instrument"}));
    }
    // The agent takes back its last track: one step, not the run.
    world.ok("undo", &json!({}));
    assert_eq!(world.track_names().len(), before.len() + 2);
    assert!(edit::end_undo_group(&world.app, "AI Composer"));
    assert_eq!(world.undo_label().as_deref(), Some("AI Composer"));

    world.ok("undo", &json!({}));
    assert_eq!(
        world.track_names(),
        before,
        "one undo reverts the whole run"
    );
    world.ok("redo", &json!({}));
    assert_eq!(world.track_names().len(), before.len() + 2);
}

#[test]
fn a_run_that_changed_nothing_leaves_the_history_alone() {
    let world = World::new();
    world.ok("create_project", &json!({"name": "Run", "bpm": 120}));
    world.ok(
        "create_track",
        &json!({"name": "Mine", "kind": "instrument"}),
    );
    edit::begin_undo_group(&world.app);
    world.ok("list_tracks", &json!({}));
    assert!(!edit::end_undo_group(&world.app, "AI Composer"));
    assert_ne!(world.undo_label().as_deref(), Some("AI Composer"));
}

//! The registry, dispatcher, context and service against the prototype's semantics
//! (`sonara.agent.tools.registry`, `sonara.agent.service`):
//! the envelope for every kind of failure, handler registration, the audit trail, confirmations
//! and dry runs through the context, the sandbox side effect of `ctx.path`, job waiting, and the
//! per-client views. The golden transcripts are replayed in `golden_transcripts.rs`.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::{AgentContext, PolicyEnvironment};
use sonara_agent::contract::args::{CreateProject, DeleteTrack, Empty};
use sonara_agent::permissions::{Access, sequential_tokens};
use sonara_agent::registry::{RegistryError, ToolError, ToolRegistry, ToolResult};
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::error::AppError;
use sonara_app::ids::IdSource;
use sonara_app::jobs::JobContext;
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::commands::project::SetTempo;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::ids::SequentialIds;
use sonara_core::model::JsonMap;
use tempfile::TempDir;

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    app: Arc<AppContext>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        // Resolved, as the sandbox resolves its roots (`/var` is a link on macOS).
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
        Self {
            _dir: dir,
            root,
            app,
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn options(&self) -> AgentOptions {
        AgentOptions {
            base_dir: Some(self.path("work")),
            audit: AuditTarget::Disabled,
            tokens: Some(shared_tokens(sequential_tokens())),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: self.path("projects"),
            }),
            ..AgentOptions::default()
        }
    }

    fn service(&self, registry: Option<ToolRegistry>) -> AgentService {
        let options = AgentOptions {
            registry: registry.map(Arc::new),
            ..self.options()
        };
        AgentService::new(Arc::clone(&self.app), options).expect("the service")
    }

    fn open_project(&self) {
        self.open_template("empty");
    }

    fn open_template(&self, template: &str) {
        let options = NewProject {
            template: template.to_owned(),
            ..NewProject::default()
        };
        self.app
            .projects()
            .new_project(&options)
            .expect("a project");
    }
}

fn call(service: &AgentService, tool: &str, args: &Value) -> Value {
    service.call(tool, Some(args))
}

// -- custom handlers -----------------------------------------------------------------------

fn io_failure(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    Err(AppError::Io {
        operation: "copy",
        path: PathBuf::from("/x/a.wav"),
        source: io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
    }
    .into())
}

fn domain_failure(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    Err(AppError::from(
        DomainError::new(ErrorCode::MediaMissing, "gone").with_detail("missing", ["a"]),
    )
    .into())
}

fn panicking(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    panic!("boom")
}

#[expect(clippy::unnecessary_wraps, reason = "a handler returns a ToolResult")]
fn not_an_object(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    Ok(json!([1]))
}

#[expect(clippy::unnecessary_wraps, reason = "a handler returns a ToolResult")]
fn faking_success(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    Ok(json!({"ok": false, "x": 1, "tool": "fake"}))
}

fn audit_failure(_ctx: &AgentContext, _args: Empty) -> ToolResult {
    Err(ToolError::Internal("broken".to_owned()))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "a handler takes its argument struct by value"
)]
fn confirmed_delete(ctx: &AgentContext, args: DeleteTrack) -> ToolResult {
    let needed = ctx.confirm("delete_track", &args, true, "the track will be deleted")?;
    Ok(json!({"dry_run": args.dry_run, "requires_confirmation": needed}))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "a handler takes its argument struct by value"
)]
fn clean_or_confirmed(ctx: &AgentContext, args: CreateProject) -> ToolResult {
    ctx.require_clean_or_confirm("create_project", &args)?;
    Ok(json!({"committed": true}))
}

fn failures() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry
        .register("verify_project", io_failure)
        .expect("free");
    registry
        .register("list_operations", domain_failure)
        .expect("free");
    registry.register("stop_playback", panicking).expect("free");
    registry
        .register("list_midi_devices", not_an_object)
        .expect("free");
    registry
        .register("get_capabilities", faking_success)
        .expect("free");
    registry.register("undo", audit_failure).expect("free");
    registry
}

// -- tests ---------------------------------------------------------------------------------

#[test]
fn unknown_and_unattached_tools_are_not_found() {
    let fixture = Fixture::new();
    let service = fixture.service(None);
    // The expected lists come from the built-in handlers: `specs()` in contract order, the
    // error's `available` sorted by name.
    let builtin = sonara_agent::handlers::builtin();
    let attached: Vec<&str> = builtin.specs().iter().map(|spec| spec.name).collect();
    let mut available = attached.clone();
    available.sort_unstable();
    let unattached = sonara_agent::contract::TOOLS
        .iter()
        .map(|spec| spec.name)
        .find(|name| !builtin.contains(name));
    for tool in std::iter::once("bogus").chain(unattached) {
        assert_eq!(
            call(&service, tool, &json!({})),
            json!({"ok": false, "tool": tool, "error": {
                "code": "NOT_FOUND",
                "message": format!("unknown tool '{tool}'"),
                "details": {"available": available},
            }})
        );
    }
    assert_eq!(
        service.info(),
        json!({"api_version": "1.0", "project": null, "tools": attached.len()})
    );
    let names: Vec<Value> = service.tools().iter().map(|t| t["name"].clone()).collect();
    assert_eq!(names, attached);
}

#[test]
fn registration_checks_the_contract() {
    let mut registry = ToolRegistry::new();
    assert_eq!(
        registry.register("bogus", io_failure),
        Err(RegistryError::UnknownTool("bogus".to_owned()))
    );
    assert_eq!(
        registry.register("create_project", io_failure),
        Err(RegistryError::WrongArguments {
            tool: "create_project",
            expected: "CreateProject",
            actual: "Empty",
        })
    );
    assert_eq!(registry.register("list_tracks", io_failure), Ok(()));
    assert_eq!(
        registry.register("list_tracks", io_failure),
        Err(RegistryError::Duplicate("list_tracks"))
    );
    assert!(registry.contains("list_tracks") && !registry.contains("undo"));
}

#[test]
fn failures_become_error_envelopes() {
    let fixture = Fixture::new();
    let service = fixture.service(Some(failures()));
    let internal = |message: &str| json!({"code": "INTERNAL", "message": message});
    assert_eq!(
        call(&service, "verify_project", &json!({}))["error"],
        internal("cannot copy /x/a.wav: denied")
    );
    assert_eq!(
        call(&service, "list_operations", &json!({}))["error"],
        json!({"code": "MEDIA_MISSING", "message": "gone", "details": {"missing": ["a"]}})
    );
    assert_eq!(
        call(&service, "stop_playback", &json!({})),
        json!({"ok": false, "tool": "stop_playback", "error": internal("panic: boom")})
    );
    // The service keeps serving after a panicking handler.
    assert_eq!(
        call(&service, "list_midi_devices", &json!({}))["error"],
        internal("tool list_midi_devices returned a non-object result")
    );
    assert_eq!(
        call(&service, "undo", &json!({}))["error"],
        internal("broken")
    );
    // A handler cannot fake the envelope keys; they keep their place in its result.
    let envelope = call(&service, "get_capabilities", &json!({}));
    assert_eq!(
        envelope,
        json!({"ok": true, "x": 1, "tool": "get_capabilities"})
    );
    let keys: Vec<&String> = envelope.as_object().expect("an object").keys().collect();
    assert_eq!(keys, ["ok", "x", "tool"]);
    // Validation runs before the handler: `Empty` forbids extra fields.
    assert_eq!(
        call(&service, "verify_project", &json!({"x": 1}))["error"],
        json!({"code": "INVALID_ARGUMENT", "message": "invalid arguments for verify_project",
               "details": {"errors": [{"field": "x", "message": "Extra inputs are not permitted"}]}})
    );
}

#[test]
fn calls_are_audited() {
    let fixture = Fixture::new();
    let log = fixture.path("logs/agent-audit.jsonl");
    let service = AgentService::new(
        Arc::clone(&fixture.app),
        AgentOptions {
            audit: AuditTarget::File(log.clone()),
            ..fixture.options()
        },
    )
    .expect("the service");
    // Falsy arguments are read as `{}` (`arguments or {}`), and audited that way.
    assert_eq!(
        service.call("list_tracks", Some(&json!([])))["error"]["code"],
        "NO_PROJECT"
    );
    assert_eq!(service.call("get_capabilities", None)["ok"], true);
    fixture.open_project();
    let history = call(&service, "get_history", &json!({"token": "x"}));
    assert_eq!(history["error"]["code"], "INVALID_ARGUMENT");
    let history = call(&service, "get_history", &json!({}));
    let recent = history["recent_agent_actions"]
        .as_array()
        .expect("recent actions");
    let summary: Vec<Value> = recent
        .iter()
        .map(|e| {
            json!([
                e["tool"],
                e["source"],
                e["ok"],
                e["error"],
                e["project_version"],
                e["args"]
            ])
        })
        .collect();
    assert_eq!(
        summary,
        [
            json!(["list_tracks", "agent", false, "NO_PROJECT", null, {}]),
            json!(["get_capabilities", "agent", true, null, null, {}]),
            json!(["get_history", "agent", false, "INVALID_ARGUMENT", 1, {"token": "<redacted>"}]),
        ]
    );
    assert!(
        recent
            .iter()
            .all(|e| e["ms"].is_f64() && e["ts"].is_string())
    );
    let lines = fs::read_to_string(&log).expect("the audit log");
    assert_eq!(lines.lines().count(), 4);
}

#[test]
fn confirmations_and_dry_runs_go_through_the_context() {
    let fixture = Fixture::new();
    let mut registry = ToolRegistry::new();
    registry
        .register("delete_track", confirmed_delete)
        .expect("free");
    registry
        .register("create_project", clean_or_confirmed)
        .expect("free");
    let service = fixture.service(Some(registry));
    let refused = call(&service, "delete_track", &json!({"track_id": "trk_1"}));
    assert_eq!(
        refused["error"],
        json!({"code": "CONFIRMATION_REQUIRED",
               "message": "the track will be deleted. Ask the user; if they agree, repeat the identical call with this confirmation_token.",
               "details": {"confirmation_token": "golden-token-0001", "action": "delete_track", "expires_in_seconds": 600}})
    );
    // Dry runs never need a token; they report whether the real call would.
    assert_eq!(
        call(
            &service,
            "delete_track",
            &json!({"track_id": "trk_1", "dry_run": true})
        ),
        json!({"dry_run": true, "requires_confirmation": true, "ok": true, "tool": "delete_track"})
    );
    // A view has its own confirmations but draws from the same token counter.
    let view = service.for_client(None, &[], "client");
    let token = json!({"track_id": "trk_1", "confirmation_token": "golden-token-0001"});
    assert_eq!(
        call(&view, "delete_track", &token)["error"]["details"]["confirmation_token"],
        "golden-token-0002"
    );
    assert_eq!(call(&service, "delete_track", &token)["ok"], true);

    // Replacing a project needs confirmation only when it has unsaved changes.
    let create = json!({"name": "Next"});
    assert_eq!(call(&service, "create_project", &create)["ok"], true);
    fixture.open_project();
    assert_eq!(call(&service, "create_project", &create)["ok"], true);
    let tempo = SetTempo {
        bpm: 90.0,
        beat: 0.0,
        ramp: false,
        clear_changes: false,
    };
    let ctx = service.context();
    let simulated = ctx.commit(&tempo, true).expect("a dry run");
    assert_eq!(
        (
            &simulated["committed"],
            &simulated["dry_run"],
            &simulated["project_version"]
        ),
        (&json!(false), &json!(true), &json!(1))
    );
    assert!(!fixture.app.session().lock().is_dirty());
    let committed = ctx.commit(&tempo, false).expect("a commit");
    assert_eq!(committed["project_version"], 2);
    assert_eq!(
        call(&service, "create_project", &create)["error"]["message"],
        "the open project 'Untitled' has unsaved changes that will be discarded. Ask the user; \
         if they agree, repeat the identical call with this confirmation_token."
    );
}

#[test]
fn paths_admit_the_open_projects_workspace() {
    let fixture = Fixture::new();
    let service = fixture.service(None);
    let ctx = service.context();
    let roots = ctx.policy().roots();
    assert_eq!(
        roots,
        [fixture.path("projects"), fixture.path("work")].map(|p| p.to_string_lossy().into_owned())
    );
    assert_eq!(
        ctx.path("../outside.wav", Access::Read)
            .expect_err("outside")
            .code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        ctx.path("take.wav", Access::Write)
            .expect("inside the base"),
        fixture.path("work/take.wav")
    );
    fixture.open_project();
    let workspace = fixture
        .app
        .session()
        .lock()
        .location()
        .map(|l| l.workspace.clone())
        .expect("a scratch workspace");
    let inside = workspace.join("media/a.wav");
    let inside = inside.to_str().expect("a UTF-8 path");
    assert!(ctx.path(inside, Access::Read).is_ok());
    assert_eq!(ctx.policy().roots().len(), 3);
    assert_eq!(
        call(&service, "get_capabilities", &json!({}))["allowed_paths"][2],
        workspace.to_string_lossy().as_ref()
    );
}

#[test]
fn views_add_their_folders() {
    let fixture = Fixture::new();
    let service = AgentService::new(
        Arc::clone(&fixture.app),
        AgentOptions {
            allowed_paths: vec![fixture.path("extra")],
            ..fixture.options()
        },
    )
    .expect("the service");
    let client = fixture.path("client");
    let view = service.for_client(Some(&client), &[fixture.path("shared")], "ipc");
    let roots = view.context().policy().roots();
    let expected: Vec<String> = ["projects", "work", "extra", "shared", "client"]
        .iter()
        .map(|p| fixture.path(p).to_string_lossy().into_owned())
        .collect();
    assert_eq!(roots, expected);
    assert_eq!(view.context().policy().base(), client.as_path());
    assert_eq!(view.context().source(), "ipc");
    // The parent service is unchanged.
    assert_eq!(service.context().policy().roots().len(), 3);
    assert_eq!(
        service.context().policy().base(),
        fixture.path("work").as_path()
    );
}

#[test]
fn waiting_for_a_job_reports_its_failure() {
    let fixture = Fixture::new();
    let service = fixture.service(None);
    let ctx = service.context();
    let jobs = fixture.app.jobs();
    let done = jobs
        .submit("test", "Done", |_: &JobContext<'_>| {
            let mut result = JsonMap::new();
            result.insert("value".to_owned(), json!(1));
            Ok(result)
        })
        .expect("a job");
    let data = ctx.wait_job(&done, true, 30.0).expect("a finished job");
    assert_eq!(
        (&data["status"], &data["result"], &data["operation_id"]),
        (&json!("succeeded"), &json!({"value": 1}), &json!(done.id()))
    );
    let failed = jobs
        .submit("test", "Fails", |_: &JobContext<'_>| {
            Err(DomainError::new(ErrorCode::MediaMissing, "gone").into())
        })
        .expect("a job");
    let error = ctx.wait_job(&failed, true, 30.0).expect_err("a failed job");
    assert_eq!(
        (
            error.code,
            error.message.as_str(),
            &error.details["operation_id"]
        ),
        (ErrorCode::MediaMissing, "gone", &json!(failed.id()))
    );
}

#[test]
fn state_tools_need_a_project() {
    let fixture = Fixture::new();
    let service = fixture.service(None);
    for tool in ["get_project_state", "list_tracks", "undo", "redo"] {
        assert_eq!(
            call(&service, tool, &json!({}))["error"],
            json!({"code": "NO_PROJECT", "message": "no project is open; create or open one first"}),
            "{tool}"
        );
    }
    fixture.open_template("band");
    let tracks = call(&service, "list_tracks", &json!({}));
    let first = tracks["tracks"][0]["track_id"].clone();
    assert!(first.is_string(), "{tracks}");
    let state = call(
        &service,
        "get_project_state",
        &json!({"track_ids": [first]}),
    );
    assert_eq!(
        (
            &state["project_version"],
            &state["dirty"],
            &state["path"],
            &state["undo"]
        ),
        (&json!(1), &json!(false), &Value::Null, &Value::Null)
    );
    let reported: Vec<&Value> = state["state"]["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .map(|t| &t["track_id"])
        .collect();
    assert_eq!(reported, [&first]);
    let none = call(&service, "get_project_state", &json!({"track_ids": []}));
    assert_eq!(none["state"]["tracks"], json!([]));
    assert_eq!(none["state"]["master"]["track_id"], "master");
    assert_eq!(
        call(&service, "undo", &json!({}))["error"],
        json!({"code": "NOTHING_TO_UNDO", "message": "nothing to undo"})
    );
    assert_eq!(
        call(&service, "list_plugins", &json!({"plugin_id": ""}))["plugins"]
            .as_array()
            .map(Vec::len),
        Some(12)
    );
}

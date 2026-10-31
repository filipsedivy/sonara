//! Step references, staged validation, rollback and the report of the conversational
//! orchestrator (SON-P13): against a stub that keeps a project version and an undo history the
//! way the session does, and against the real agent service.

use std::cell::{Cell, RefCell};
use std::fs;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::orchestration::{
    Approval, Orchestrator, Plan, Report, Rollback, Status, Step, StepRef, ToolRunner,
};
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::settings::Settings;
use sonara_core::commands::track::RenameTrack;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

// -- plans built by hand ---------------------------------------------------------------------

fn step(tool: &str, args: Value, description: &str) -> Step {
    Step {
        tool: tool.to_owned(),
        args,
        description: description.to_owned(),
        destructive: false,
        refs: Vec::new(),
    }
}

fn track(name: &str, kind: &str) -> Step {
    step(
        "create_track",
        json!({"name": name, "kind": kind}),
        &format!("create the track '{name}'"),
    )
}

/// The id of the track that step `of` creates.
fn track_ref(of: usize) -> StepRef {
    StepRef::new("/track_id", of, "/result/track_id")
}

/// A clip on the track that step `of` creates.
fn clip_on(of: usize) -> Step {
    let reference = track_ref(of);
    Step {
        refs: vec![reference.clone()],
        ..step(
            "create_midi_clip",
            json!({"track_id": reference.placeholder(), "start": 0, "length": 4,
                   "notes": [{"pitch": 60, "start": 0, "length": 1, "velocity": 90}]}),
            &format!("write a clip on the track of step {of}"),
        )
    }
}

/// A pan on the track that step `of` creates.
fn pan_on(of: usize, pan: f64) -> Step {
    let reference = track_ref(of);
    Step {
        refs: vec![reference.clone()],
        ..step(
            "set_track_pan",
            json!({"track_id": reference.placeholder(), "pan": pan}),
            &format!("pan the track of step {of}"),
        )
    }
}

fn plan(steps: Vec<Step>) -> Plan {
    Plan {
        request: "a plan built by hand".to_owned(),
        steps,
        questions: Vec::new(),
    }
}

fn rolled_back(report: &Report) -> Vec<bool> {
    report
        .executed
        .iter()
        .map(|result| result.rolled_back)
        .collect()
}

// -- a session in miniature ------------------------------------------------------------------

/// The undo label a stub commit records, as the session's commands name them.
fn label(tool: &str) -> &'static str {
    match tool {
        "create_track" => "Create Track",
        "create_midi_clip" => "Create MIDI Clip",
        "set_tempo" => "Set Tempo",
        "set_track_pan" => "Track Pan",
        _ => "Edit",
    }
}

/// Keeps a project version that every commit and undo moves on, an undo stack of labels and a
/// counter for new ids, and records every call. Its replies have the shape of the real ones.
#[derive(Default)]
struct Session {
    calls: RefCell<Vec<(String, Value)>>,
    version: Cell<u64>,
    history: RefCell<Vec<String>>,
    undone: RefCell<Vec<String>>,
    ids: Cell<u32>,
    tempo: Cell<f64>,
    /// The tool whose dry run is refused.
    refuse_dry_run: Option<&'static str>,
    /// The tool whose commit fails.
    fail_commit: Option<&'static str>,
    /// The tool before whose first commit someone else renames a track.
    intrude_before: Option<&'static str>,
    /// The tool that commits without an undo step of its own.
    unrecorded: Option<&'static str>,
}

impl Session {
    fn new() -> Self {
        Self {
            tempo: Cell::new(120.0),
            ..Self::default()
        }
    }

    /// The calls that changed or would have changed the project, without dry runs, reads
    /// and the rollback's own calls.
    fn commits(&self) -> Vec<(String, Value)> {
        self.calls
            .borrow()
            .iter()
            .filter(|(tool, args)| {
                !matches!(
                    tool.as_str(),
                    "get_project_state" | "undo" | "list_tracks" | "list_plugins"
                ) && args["dry_run"] != json!(true)
            })
            .cloned()
            .collect()
    }

    fn tools_called(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|(tool, args)| {
                if args["dry_run"] == json!(true) {
                    format!("{tool}?")
                } else {
                    tool.clone()
                }
            })
            .collect()
    }

    fn bump(&self) -> u64 {
        self.version.set(self.version.get() + 1);
        self.version.get()
    }

    fn commit(&self, tool: &str, args: &Value) -> Value {
        if self.fail_commit == Some(tool) {
            return json!({"ok": false, "tool": tool, "error": {"code": "ENGINE_UNAVAILABLE", "message": "the engine stopped"}});
        }
        if self.intrude_before == Some(tool)
            && !self.history.borrow().contains(&"Rename Track".to_owned())
        {
            self.bump();
            self.history.borrow_mut().push("Rename Track".to_owned());
        }
        if tool == "set_tempo" && args["bpm"].as_f64() == Some(self.tempo.get()) {
            return json!({"ok": true, "committed": false, "dry_run": false,
                          "project_version": self.version.get(), "operation": "Set Tempo"});
        }
        let version = self.bump();
        if self.unrecorded != Some(tool) {
            self.history.borrow_mut().push(label(tool).to_owned());
        }
        self.ids.set(self.ids.get() + 1);
        let id = self.ids.get();
        let result = match tool {
            "create_track" => {
                json!({"track_id": format!("trk_{id}"), "instrument_device_id": format!("dev_{id}")})
            }
            "create_midi_clip" => {
                json!({"clip_id": format!("clp_{id}"), "track_id": args["track_id"]})
            }
            _ => json!({}),
        };
        json!({"ok": true, "committed": true, "dry_run": false, "project_version": version,
               "operation": label(tool), "result": result})
    }
}

impl ToolRunner for &Session {
    fn call(&self, tool: &str, args: &Value) -> Value {
        self.calls
            .borrow_mut()
            .push((tool.to_owned(), args.clone()));
        match tool {
            "list_tracks" => json!({"ok": true, "tracks": []}),
            "list_plugins" => json!({"ok": true, "plugins": []}),
            "get_project_state" => json!({"ok": true, "state": {},
                "project_version": self.version.get(), "undo": self.history.borrow().last()}),
            "undo" => match self.history.borrow_mut().pop() {
                Some(label) => {
                    let version = self.bump();
                    self.undone.borrow_mut().push(label.clone());
                    json!({"ok": true, "committed": true, "project_version": version,
                           "operation": format!("Undo {label}")})
                }
                None => json!({"ok": false, "error": {"code": "NOTHING_TO_UNDO"}}),
            },
            _ if args["dry_run"] == json!(true) => {
                if self.refuse_dry_run == Some(tool) {
                    json!({"ok": false, "tool": tool, "error": {"code": "INVALID_ARGUMENT", "message": "the track takes no MIDI clips"}})
                } else {
                    // A dry run draws ids that the commit does not use.
                    json!({"ok": true, "committed": false, "dry_run": true,
                           "project_version": self.version.get(), "operation": label(tool),
                           "result": {"track_id": "trk_dry"}})
                }
            }
            _ => self.commit(tool, args),
        }
    }
}

// -- references and stages -------------------------------------------------------------------

#[test]
fn a_later_step_uses_the_id_that_an_earlier_step_committed() {
    let session = Session::new();
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            track("Violins", "instrument"),
            clip_on(0),
            pan_on(0, -0.4),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Done, "{}", report.summary);
    // The first stage is dry-run before it commits; the second stage, which needs the new id,
    // is dry-run with that id before any of its steps commits.
    assert_eq!(
        session.tools_called(),
        [
            "create_track?",
            "create_track",
            "create_midi_clip?",
            "set_track_pan?",
            "create_midi_clip",
            "set_track_pan",
        ]
    );
    let commits = session.commits();
    assert_eq!(
        commits[1].1["track_id"], "trk_1",
        "the commit's id, not the dry run's"
    );
    assert_eq!(commits[2].1, json!({"track_id": "trk_1", "pan": -0.4}));
    // The report shows each step as it ran, with the tool's reply.
    assert_eq!(report.executed.len(), 3);
    assert_eq!(report.executed[1].step.args["track_id"], "trk_1");
    assert_eq!(report.executed[1].step.refs, [track_ref(0)]);
    let reply = report.executed[1]
        .reply
        .as_ref()
        .expect("a committed step keeps its reply");
    assert_eq!(reply["result"]["clip_id"], "clp_2");
    assert!(
        report
            .executed
            .iter()
            .all(|result| result.ok && result.error.is_none())
    );
    assert_eq!(report.rollback, None);
    // The plan itself keeps the placeholders.
    assert_eq!(
        report.plan.steps[1].args["track_id"],
        "{step 0: /result/track_id}"
    );
}

#[test]
fn a_reference_inside_a_longer_string_is_filled_in_place() {
    let session = Session::new();
    let device = StepRef::new("/name", 0, "/result/instrument_device_id");
    let rename = Step {
        refs: vec![device.clone()],
        ..step(
            "rename_track",
            json!({"track_id": "trk_0", "name": format!("{} dynamics", device.placeholder())}),
            "name a track after the new device",
        )
    };
    let report = Orchestrator::new(&session).execute(
        plan(vec![track("Horns", "instrument"), rename]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Done, "{}", report.summary);
    assert_eq!(
        session.commits()[1].1,
        json!({"track_id": "trk_0", "name": "dev_1 dynamics"})
    );
}

#[test]
fn a_step_whose_references_cannot_work_is_refused_before_any_commit() {
    let later = Step {
        refs: vec![track_ref(1)],
        ..clip_on(0)
    };
    let missing_placeholder = Step {
        args: json!({"track_id": "trk_7", "start": 0, "length": 4}),
        ..clip_on(0)
    };
    let mut extra_field = clip_on(0);
    extra_field.args["volume"] = json!(3);
    let unknown_tool = Step {
        tool: "write_symphony".to_owned(),
        ..clip_on(0)
    };
    for (bad, code, needle) in [
        (
            later,
            "INVALID_ARGUMENT",
            "step 1, which does not run before it",
        ),
        (
            missing_placeholder,
            "INVALID_ARGUMENT",
            "does not hold {step 0: /result/track_id}",
        ),
        (
            extra_field,
            "INVALID_ARGUMENT",
            "volume: Extra inputs are not permitted",
        ),
        (unknown_tool, "NOT_FOUND", "unknown tool 'write_symphony'"),
    ] {
        let session = Session::new();
        let report = Orchestrator::new(&session).execute(
            plan(vec![track("Violins", "instrument"), bad]),
            Approval::Confirmed,
        );
        assert_eq!(
            report.status,
            Status::Rejected,
            "{needle}: {}",
            report.summary
        );
        assert!(session.commits().is_empty(), "{needle}: nothing may commit");
        assert_eq!(report.executed.len(), 1, "{needle}");
        let refused = &report.executed[0];
        assert!(!refused.ok);
        assert_eq!(refused.error.as_deref(), Some(code), "{needle}");
        let message = refused.message.as_deref().unwrap_or_default();
        assert!(message.contains(needle), "{needle}: {message}");
        assert_eq!(refused.reply, None, "refused without a call");
        assert!(
            report.summary.starts_with("Nothing was changed"),
            "{}",
            report.summary
        );
        assert_eq!(report.rollback, None);
    }
}

#[test]
fn a_dry_run_refused_before_the_first_commit_changes_nothing() {
    let mut session = Session::new();
    session.refuse_dry_run = Some("create_track");
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            step("set_tempo", json!({"bpm": 90.0}), "set the tempo to 90 BPM"),
            track("Violins", "instrument"),
            clip_on(1),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Rejected);
    assert!(session.commits().is_empty(), "nothing may commit");
    assert_eq!(session.version.get(), 0);
    let refused = &report.executed[0];
    assert_eq!(refused.step.tool, "create_track");
    assert_eq!(
        refused.message.as_deref(),
        Some("the track takes no MIDI clips")
    );
    assert!(refused.reply.is_some(), "the dry run's reply is kept");
}

// -- rollback --------------------------------------------------------------------------------

#[test]
fn a_refused_dry_run_of_a_later_stage_undoes_the_earlier_stages_newest_first() {
    let mut session = Session::new();
    session.history.borrow_mut().push("Mine".to_owned());
    session.refuse_dry_run = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            step("set_tempo", json!({"bpm": 90.0}), "set the tempo to 90 BPM"),
            track("Violins", "instrument"),
            track("Horns", "instrument"),
            pan_on(2, 0.3),
            clip_on(1),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed, "{}", report.summary);
    assert_eq!(
        *session.undone.borrow(),
        ["Create Track", "Create Track", "Set Tempo"]
    );
    assert_eq!(
        *session.history.borrow(),
        ["Mine"],
        "the earlier history stays"
    );
    // Nothing of the refused stage committed, not even its valid pan.
    let tools: Vec<String> = session
        .commits()
        .into_iter()
        .map(|(tool, _)| tool)
        .collect();
    assert_eq!(tools, ["set_tempo", "create_track", "create_track"]);
    assert_eq!(
        report.rollback,
        Some(Rollback {
            complete: true,
            stopped: None
        })
    );
    assert_eq!(rolled_back(&report), [true, true, true, false]);
    let failed = report.executed.last().expect("the failed step");
    assert_eq!(failed.step.tool, "create_midi_clip");
    assert_eq!(
        failed.step.args["track_id"], "trk_2",
        "reported as it would have run"
    );
    assert_eq!(failed.error.as_deref(), Some("INVALID_ARGUMENT"));
    assert_eq!(
        report.summary,
        "'write a clip on the track of step 1' failed (INVALID_ARGUMENT: the track takes no MIDI \
         clips), so the steps already applied were undone: set the tempo to 90 BPM; create the \
         track 'Violins'; create the track 'Horns'."
    );
}

#[test]
fn a_failed_commit_undoes_the_steps_before_it() {
    let mut session = Session::new();
    session.fail_commit = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![track("Violins", "instrument"), clip_on(0)]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    assert_eq!(*session.undone.borrow(), ["Create Track"]);
    assert!(session.history.borrow().is_empty());
    assert_eq!(rolled_back(&report), [true, false]);
    assert_eq!(
        report.executed[1].error.as_deref(),
        Some("ENGINE_UNAVAILABLE")
    );
    assert_eq!(report.rollback.as_ref().map(|r| r.complete), Some(true));
}

#[test]
fn a_reference_to_a_value_the_reply_lacks_undoes_the_earlier_steps() {
    let session = Session::new();
    let reference = StepRef::new("/track_id", 0, "/result/clip_id");
    let clip = Step {
        refs: vec![reference.clone()],
        args: json!({"track_id": reference.placeholder(), "start": 0, "length": 4}),
        ..clip_on(0)
    };
    let report = Orchestrator::new(&session).execute(
        plan(vec![track("Violins", "instrument"), clip]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    assert_eq!(*session.undone.borrow(), ["Create Track"]);
    let failed = &report.executed[1];
    assert_eq!(failed.error.as_deref(), Some("INVALID_ARGUMENT"));
    assert_eq!(
        failed.message.as_deref(),
        Some(
            "it cannot take a value from an earlier step: the reply of step 0 has no text at /result/clip_id"
        )
    );
    assert_eq!(failed.reply, None, "refused without a call");
    assert!(
        !session
            .tools_called()
            .contains(&"create_midi_clip?".to_owned())
    );
}

#[test]
fn a_step_that_changed_nothing_is_not_undone() {
    let mut session = Session::new();
    session.fail_commit = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            // The tempo is already 120: the step succeeds without an undo step.
            step(
                "set_tempo",
                json!({"bpm": 120.0}),
                "set the tempo to 120 BPM",
            ),
            track("Violins", "instrument"),
            clip_on(1),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    assert_eq!(*session.undone.borrow(), ["Create Track"]);
    assert_eq!(rolled_back(&report), [false, true, false]);
    assert!(report.executed[0].ok);
    assert_eq!(report.rollback.as_ref().map(|r| r.complete), Some(true));
}

#[test]
fn another_edit_during_the_run_stops_the_rollback_before_it() {
    let mut session = Session::new();
    session.intrude_before = Some("set_track_pan");
    session.fail_commit = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            track("Violins", "instrument"),
            pan_on(0, 0.2),
            clip_on(0),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    // The pan, made after the other edit, is undone; the other edit and the track before it stay.
    assert_eq!(*session.undone.borrow(), ["Track Pan"]);
    assert_eq!(*session.history.borrow(), ["Create Track", "Rename Track"]);
    assert_eq!(rolled_back(&report), [false, true, false]);
    let rollback = report.rollback.as_ref().expect("a rollback");
    assert!(!rollback.complete);
    assert_eq!(
        rollback.stopped.as_deref(),
        Some(
            "another edit changed the project after 'create the track 'Violins'', and the rollback keeps it"
        )
    );
    assert!(
        report.summary.contains(
            "Undone: pan the track of step 0. Still applied: create the track 'Violins'."
        ),
        "{}",
        report.summary
    );
}

#[test]
fn a_step_without_its_own_undo_step_stops_the_rollback() {
    let mut session = Session::new();
    session.history.borrow_mut().push("Mine".to_owned());
    session.unrecorded = Some("set_track_pan");
    session.fail_commit = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![
            track("Violins", "instrument"),
            pan_on(0, 0.2),
            clip_on(0),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    // Undoing now would take back the track, not the pan: nothing is undone.
    assert!(session.undone.borrow().is_empty());
    assert_eq!(*session.history.borrow(), ["Mine", "Create Track"]);
    let rollback = report.rollback.as_ref().expect("a rollback");
    assert!(!rollback.complete);
    assert_eq!(
        rollback.stopped.as_deref(),
        Some(
            "the newest undo step is 'Create Track', not the one 'pan the track of step 0' made ('Track Pan')"
        )
    );
    assert_eq!(rolled_back(&report), [false, false, false]);
}

#[test]
fn a_reply_without_an_undo_label_stops_the_rollback() {
    struct Bare;
    impl ToolRunner for Bare {
        fn call(&self, tool: &str, args: &Value) -> Value {
            match tool {
                "set_track_pan" if args["dry_run"] != json!(true) => {
                    json!({"ok": false, "error": {"code": "ENGINE_UNAVAILABLE"}})
                }
                _ => json!({"ok": true}),
            }
        }
    }
    let report = Orchestrator::new(Bare).execute(
        plan(vec![
            step("set_tempo", json!({"bpm": 90.0}), "set the tempo to 90 BPM"),
            step(
                "set_track_pan",
                json!({"track_id": "trk_1", "pan": 0.5}),
                "pan Keys",
            ),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed);
    assert_eq!(
        report.rollback,
        Some(Rollback {
            complete: false,
            stopped: Some(
                "the reply of 'set the tempo to 90 BPM' names no undo step and project version"
                    .to_owned()
            ),
        })
    );
    assert_eq!(
        report.summary,
        "'pan Keys' failed (ENGINE_UNAVAILABLE). Undone: nothing. Still applied: set the tempo to \
         90 BPM. The rollback stopped because the reply of 'set the tempo to 90 BPM' names no \
         undo step and project version."
    );
}

#[test]
fn a_report_serializes_with_its_rollback() {
    let mut session = Session::new();
    session.fail_commit = Some("create_midi_clip");
    let report = Orchestrator::new(&session).execute(
        plan(vec![track("Violins", "instrument"), clip_on(0)]),
        Approval::Pending,
    );
    let json = serde_json::to_value(&report).expect("a report serializes");
    assert_eq!(json["status"], "failed");
    assert_eq!(json["rollback"], json!({"complete": true, "stopped": null}));
    assert_eq!(json["executed"][0]["rolled_back"], true);
    assert_eq!(
        json["plan"]["steps"][1]["refs"],
        json!([{"arg": "/track_id", "step": 0, "pointer": "/result/track_id"}])
    );
}

#[test]
fn a_request_that_sets_the_tempo_twice_is_a_question() {
    let session = Session::new();
    let report = Orchestrator::new(&session)
        .run("set tempo to 90 and set tempo to 100", Approval::Confirmed);
    assert_eq!(report.status, Status::NeedsClarification);
    assert_eq!(
        report.plan.questions,
        [
            "The request sets the tempo more than once (set the tempo to 90 BPM, set the tempo to \
          100 BPM). Which tempo do you want?"
        ]
    );
    assert!(session.commits().is_empty(), "a question changes nothing");
}

// -- the real service ------------------------------------------------------------------------

struct World {
    _dir: TempDir,
    app: Arc<AppContext>,
    service: AgentService,
}

impl World {
    /// A service with a new project that holds the track "Mine".
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
        let world = Self {
            _dir: dir,
            app,
            service,
        };
        world.ok("create_project", &json!({"name": "Run", "bpm": 120}));
        world.ok(
            "create_track",
            &json!({"name": "Mine", "kind": "instrument"}),
        );
        world
    }

    fn ok(&self, tool: &str, args: &Value) -> Value {
        let reply = self.service.call(tool, Some(args));
        assert_eq!(reply["ok"], true, "{tool}: {reply}");
        reply
    }

    /// The whole project, notes included, and the undo labels.
    fn snapshot(&self) -> (Value, Value) {
        let state = self.ok(
            "get_project_state",
            &json!({"detail": "full", "include_notes": true}),
        );
        let history = self.ok("get_history", &json!({}));
        (state["state"].clone(), history["undo"].clone())
    }

    fn track_names(&self) -> Vec<String> {
        self.ok("list_tracks", &json!({}))["tracks"]
            .as_array()
            .expect("tracks")
            .iter()
            .map(|t| t["name"].as_str().expect("a name").to_owned())
            .collect()
    }

    fn track_id(&self, name: &str) -> String {
        self.ok("list_tracks", &json!({}))["tracks"]
            .as_array()
            .expect("tracks")
            .iter()
            .find(|t| t["name"] == name)
            .and_then(|t| t["track_id"].as_str())
            .expect("the track")
            .to_owned()
    }
}

/// Calls the real service; before the `nth` commit of `tool` (from 1), the user renames the
/// track "Mine" in the GUI.
struct Through<'a> {
    world: &'a World,
    intrude: Option<(&'static str, usize)>,
    seen: Cell<usize>,
}

impl<'a> Through<'a> {
    fn new(world: &'a World) -> Self {
        Self {
            world,
            intrude: None,
            seen: Cell::new(0),
        }
    }
}

impl ToolRunner for Through<'_> {
    fn call(&self, tool: &str, args: &Value) -> Value {
        if let Some((target, nth)) = self.intrude
            && tool == target
            && args["dry_run"] != json!(true)
        {
            self.seen.set(self.seen.get() + 1);
            if self.seen.get() == nth {
                let rename = RenameTrack {
                    track_id: self.world.track_id("Mine"),
                    name: "Mine, renamed".to_owned(),
                };
                self.world
                    .app
                    .session()
                    .lock()
                    .execute(&rename, false, "user")
                    .expect("the user's rename");
            }
        }
        self.world.service.call(tool, Some(args))
    }
}

#[test]
fn the_real_tools_put_a_clip_on_the_track_the_plan_created() {
    let world = World::new();
    let report = Orchestrator::new(Through::new(&world)).execute(
        plan(vec![
            track("Violins", "instrument"),
            clip_on(0),
            pan_on(0, -0.25),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Done, "{}", report.summary);
    let violins = world.track_id("Violins");
    let tracks = world.ok("list_tracks", &json!({}));
    let created = tracks["tracks"]
        .as_array()
        .expect("tracks")
        .iter()
        .find(|t| t["track_id"] == violins.as_str())
        .expect("the new track");
    assert_eq!(created["clips"], 1);
    assert_eq!(created["pan"], -0.25);
    assert_eq!(report.executed[1].step.args["track_id"], violins.as_str());
}

#[test]
fn a_failed_later_stage_leaves_the_real_project_as_it_was() {
    let world = World::new();
    let (state, undo) = world.snapshot();
    // A MIDI clip on an audio track passes the input model and fails only against the project.
    let report = Orchestrator::new(Through::new(&world)).execute(
        plan(vec![
            step("set_tempo", json!({"bpm": 90.0}), "set the tempo to 90 BPM"),
            track("Violins", "instrument"),
            track("Room", "audio"),
            pan_on(1, 0.5),
            clip_on(2),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed, "{}", report.summary);
    assert_eq!(
        report.executed.last().and_then(|r| r.error.as_deref()),
        Some("INVALID_ARGUMENT")
    );
    assert_eq!(
        report.rollback,
        Some(Rollback {
            complete: true,
            stopped: None
        })
    );
    assert_eq!(rolled_back(&report), [true, true, true, false]);
    assert_eq!(
        world.snapshot(),
        (state, undo),
        "the project and its history are as before"
    );
}

#[test]
fn the_rollback_keeps_a_user_edit_made_during_the_run() {
    let world = World::new();
    let mut runner = Through::new(&world);
    runner.intrude = Some(("create_track", 2));
    let report = Orchestrator::new(runner).execute(
        plan(vec![
            track("Violins", "instrument"),
            track("Room", "audio"),
            clip_on(1),
        ]),
        Approval::Pending,
    );
    assert_eq!(report.status, Status::Failed, "{}", report.summary);
    // "Room", committed after the rename, is undone; the rename and "Violins" before it stay.
    assert_eq!(world.track_names(), ["Mine, renamed", "Violins"]);
    assert_eq!(rolled_back(&report), [false, true, false]);
    let rollback = report.rollback.expect("a rollback");
    assert!(!rollback.complete);
    assert!(
        rollback
            .stopped
            .as_deref()
            .is_some_and(|why| why.starts_with("another edit changed the project after")),
        "{rollback:?}"
    );
    let history = world.ok("get_history", &json!({}));
    assert_eq!(
        history["undo"].as_array().and_then(|undo| undo.last()),
        Some(&json!("Rename Track"))
    );
}

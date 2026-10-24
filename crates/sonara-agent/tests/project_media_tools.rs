//! The project, media, operation and real-time tools on paths the golden batches do not take
//! (TST-03): `create_project` with a path, the failure
//! paths of `save_project` and `export_midi`, a MIDI file without notes, operations that do not
//! exist, and the real-time tools without an engine. Everything else is replayed against the
//! prototype in `golden_transcripts.rs`.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_app::midi::{MidiDevices, MidiError, OpenInput};
use sonara_app::settings::Settings;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

/// A Type 1 Standard MIDI File with one track that holds nothing but its end.
const NO_NOTES_MID: &[u8] = b"MThd\x00\x00\x00\x06\x00\x01\x00\x01\x03\xc0\
MTrk\x00\x00\x00\x04\x00\xff\x2f\x00";

/// MIDI devices with one input and one output, or none that can be listed.
struct FixtureMidi {
    listing_fails: bool,
}

impl MidiDevices for FixtureMidi {
    fn api_name(&self) -> &'static str {
        "core"
    }

    fn input_ports(&self) -> Result<Vec<String>, MidiError> {
        if self.listing_fails {
            return Err(MidiError::Init);
        }
        Ok(vec!["Keys".to_owned()])
    }

    fn output_ports(&self) -> Result<Vec<String>, MidiError> {
        Ok(vec!["Synth".to_owned()])
    }

    fn open_input(&self, name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::InputNotFound {
            name: name.to_owned(),
            available: Vec::new(),
        })
    }

    fn open_virtual_input(&self, _name: &str, _port: u8) -> Result<OpenInput, MidiError> {
        Err(MidiError::VirtualPortsUnsupported)
    }
}

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    service: AgentService,
}

impl Fixture {
    fn new() -> Self {
        Self::with_midi(FixtureMidi {
            listing_fails: false,
        })
    }

    fn with_midi(midi: FixtureMidi) -> Self {
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
                midi_devices: Some(Arc::new(midi)),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        let service = AgentService::new(
            app,
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
            root,
            service,
        }
    }

    fn work(&self, rel: &str) -> PathBuf {
        self.root.join("work").join(rel)
    }

    fn call(&self, tool: &str, args: &Value) -> Value {
        self.service.call(tool, Some(args))
    }

    /// Calls a tool that must succeed.
    fn ok(&self, tool: &str, args: &Value) -> Value {
        let result = self.call(tool, args);
        assert_eq!(result["ok"], true, "{tool}: {result}");
        result
    }
}

fn error(result: &Value) -> (&str, &str) {
    assert_eq!(result["ok"], false, "{result}");
    (
        result["error"]["code"].as_str().unwrap_or_default(),
        result["error"]["message"].as_str().unwrap_or_default(),
    )
}

#[test]
fn create_project_with_a_path_saves_it_at_once_without_history() {
    let fx = Fixture::new();
    let created = fx.ok(
        "create_project",
        &json!({"name": "Saved", "bpm": 90, "path": "songs/Saved", "template": "band"}),
    );
    let path = fx.work("songs/Saved.sonara");
    let keys: Vec<&str> = created
        .as_object()
        .map(|map| map.keys().map(String::as_str).collect())
        .unwrap_or_default();
    assert_eq!(
        keys,
        [
            "committed",
            "project_id",
            "path",
            "state",
            "project_version",
            "ok",
            "tool"
        ]
    );
    assert_eq!(created["path"], path.to_string_lossy().as_ref());
    assert!(path.join("project.json").is_file());
    let state = fx.ok("get_project_state", &json!({}));
    assert_eq!(state["dirty"], false);
    assert_eq!(state["path"], path.to_string_lossy().as_ref());
    // The template's tracks are part of the new project, not undoable edits.
    let history = fx.ok("get_history", &json!({}));
    assert_eq!(history["undo"], json!([]));
    assert_eq!(state["undo"], Value::Null);
}

#[test]
fn create_project_dry_run_reports_unsaved_changes_and_draws_nothing() {
    let fx = Fixture::new();
    fx.ok("create_project", &json!({"name": "One"}));
    fx.ok("add_marker", &json!({"name": "Edit", "at": 4}));
    let dry = fx.ok("create_project", &json!({"name": "Two", "dry_run": true}));
    assert_eq!(dry["would_discard_unsaved"], true);
    assert_eq!(dry["committed"], false);
    // The marker after the dry run continues the sequence of the first marker.
    let marker = fx.ok("add_marker", &json!({"name": "Next", "at": 8}));
    assert_eq!(marker["result"]["marker_id"], "mrk_000000000003");
}

#[test]
fn save_project_needs_a_path_once_and_for_copies() {
    let fx = Fixture::new();
    assert_eq!(
        error(&fx.call("save_project", &json!({}))),
        ("NO_PROJECT", "no project is open; create or open one first")
    );
    fx.ok("create_project", &json!({"name": "Song"}));
    assert_eq!(
        error(&fx.call("save_project", &json!({}))),
        (
            "INVALID_ARGUMENT",
            "the project has never been saved; pass a path"
        )
    );
    let dry = fx.ok("save_project", &json!({"path": "Song", "dry_run": true}));
    assert_eq!(
        dry["target"],
        fx.work("Song.sonara").to_string_lossy().as_ref()
    );
    assert_eq!(dry["requires_confirmation"], false);
    let saved = fx.ok("save_project", &json!({"path": "Song"}));
    assert_eq!(saved["dirty"], false);
    assert_eq!(saved["backup"], Value::Null);
    assert_eq!(
        error(&fx.call("save_project", &json!({"as_copy": true}))),
        ("INVALID_ARGUMENT", "copy requires a path")
    );
    let copy = fx.ok("save_project", &json!({"path": "Copy", "as_copy": true}));
    assert_eq!(
        copy["path"],
        fx.work("Copy.sonara").to_string_lossy().as_ref()
    );
    // The open project still lives where it was saved.
    let state = fx.ok("get_project_state", &json!({}));
    assert_eq!(
        state["path"],
        fx.work("Song.sonara").to_string_lossy().as_ref()
    );
}

#[test]
fn save_project_over_another_project_needs_confirmation() {
    let fx = Fixture::new();
    fx.ok("create_project", &json!({"name": "Song"}));
    fx.ok("save_project", &json!({"path": "Other", "as_copy": true}));
    fx.ok("save_project", &json!({"path": "Song"}));
    let dry = fx.ok("save_project", &json!({"path": "Other", "dry_run": true}));
    assert_eq!(dry["requires_confirmation"], true);
    let refused = fx.call("save_project", &json!({"path": "Other"}));
    assert_eq!(error(&refused).0, "CONFIRMATION_REQUIRED");
    let token = refused["error"]["details"]["confirmation_token"].clone();
    let saved = fx.ok(
        "save_project",
        &json!({"path": "Other", "confirmation_token": token}),
    );
    assert_eq!(
        saved["path"],
        fx.work("Other.sonara").to_string_lossy().as_ref()
    );
    // Saving in place never asks.
    fx.ok("save_project", &json!({}));
}

#[test]
fn export_midi_adds_the_suffix_and_refuses_to_overwrite_silently() {
    let fx = Fixture::new();
    fx.ok("create_project", &json!({"name": "Song"}));
    let exported = fx.ok("export_midi", &json!({"path": "out/song.txt"}));
    let path = fx.work("out/song.mid");
    assert_eq!(exported["path"], path.to_string_lossy().as_ref());
    assert_eq!(exported["ppq"], 960);
    assert_eq!(exported["committed"], true);
    assert!(path.is_file());
    let refused = fx.call("export_midi", &json!({"path": "out/song.mid"}));
    let shown = path.to_string_lossy().into_owned();
    assert_eq!(
        error(&refused),
        (
            "CONFLICT",
            format!("{shown} exists; set overwrite=true").as_str()
        )
    );
    assert_eq!(refused["error"]["details"]["path"], shown.as_str());
    let dry = fx.ok(
        "export_midi",
        &json!({"path": "out/song.mid", "overwrite": true, "dry_run": true}),
    );
    assert_eq!(dry["requires_confirmation"], true);
    let confirm = fx.call(
        "export_midi",
        &json!({"path": "out/song.mid", "overwrite": true}),
    );
    assert_eq!(error(&confirm).0, "CONFIRMATION_REQUIRED");
}

#[test]
fn import_midi_refuses_a_file_without_notes() {
    let fx = Fixture::new();
    fx.ok("create_project", &json!({"name": "Song"}));
    fs::write(fx.work("empty.mid"), NO_NOTES_MID).expect("the file is written");
    let dry = fx.ok(
        "import_midi",
        &json!({"path": "empty.mid", "dry_run": true}),
    );
    assert_eq!(dry["tracks"], json!([]));
    assert_eq!(
        error(&fx.call("import_midi", &json!({"path": "empty.mid"}))),
        ("INVALID_ARGUMENT", "empty.mid contains no notes")
    );
    let state = fx.ok("get_project_state", &json!({}));
    assert_eq!(state["dirty"], false);
}

#[test]
fn operations_that_do_not_exist_are_not_found() {
    let fx = Fixture::new();
    for tool in ["get_operation_status", "cancel_operation"] {
        assert_eq!(
            error(&fx.call(tool, &json!({"operation_id": "op_x"}))),
            ("NOT_FOUND", "operation 'op_x' not found")
        );
    }
    assert_eq!(
        fx.ok("list_operations", &json!({}))["operations"],
        json!([])
    );
}

#[test]
fn real_time_tools_answer_without_an_engine() {
    let fx = Fixture::new();
    // Playback needs a project before it needs the engine; recording only the engine.
    assert_eq!(error(&fx.call("play_preview", &json!({}))).0, "NO_PROJECT");
    let unavailable = (
        "ENGINE_UNAVAILABLE",
        "audio backend disabled (null backend)",
    );
    assert_eq!(error(&fx.call("start_recording", &json!({}))), unavailable);
    fx.ok("create_project", &json!({"name": "Song"}));
    assert_eq!(
        error(&fx.call("play_preview", &json!({"start": "x:y"}))).0,
        "INVALID_ARGUMENT"
    );
    assert_eq!(
        error(&fx.call("play_preview", &json!({"start": 4}))),
        unavailable
    );
    assert_eq!(
        error(&fx.call("set_midi_input", &json!({"enabled": true, "ports": ["A"]}))),
        unavailable
    );
    assert_eq!(
        fx.ok("set_midi_input", &json!({"enabled": false}))["enabled"],
        false
    );
    // The fixture's MIDI devices have one port each way.
    let devices = fx.ok("list_midi_devices", &json!({}));
    assert_eq!(
        devices["devices"],
        json!({"available": true, "api": "core", "inputs": ["Keys"], "outputs": ["Synth"]})
    );
    assert_eq!(devices["status"], json!({"enabled": false, "ports": []}));
    let stopped = fx.ok("stop_playback", &json!({}));
    assert_eq!(
        (&stopped["stopped"], &stopped["was_playing"]),
        (&json!(true), &json!(false))
    );
}

/// A MIDI system that cannot be listed (Linux without an ALSA sequencer) fails the device list
/// with `INTERNAL`, as the prototype's RtMidi error did.
#[test]
fn a_failed_midi_listing_is_internal() {
    let fx = Fixture::with_midi(FixtureMidi {
        listing_fails: true,
    });
    assert_eq!(
        error(&fx.call("list_midi_devices", &json!({}))),
        ("INTERNAL", "MIDI support could not be initialized")
    );
}

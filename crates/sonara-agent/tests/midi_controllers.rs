//! `write_midi_controllers`, the controller tool of agent API 1.1 (CTR-01): what a call stores
//! is read back from the session's project, the replacement keeps every other controller event
//! and the notes, a call is one undo step, and every refusal changes nothing (TST-03).

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::contract::tools::{Safety, find};
use sonara_agent::contract::versions::is_added_tool;
use sonara_agent::permissions::sequential_tokens;
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget, shared_tokens};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::ids::IdSource;
use sonara_core::commands::clip::SetControlEvents;
use sonara_core::ids::SequentialIds;
use sonara_core::midi::{ControlKind, MidiControlEvent};
use sonara_core::model::clip::Clip;

const TOOL: &str = "write_midi_controllers";

/// The agent on a headless application with sequential IDs, the application (to read the
/// session's project back) and the folder relative paths resolve against, with the temporary
/// directory kept alive. `open` creates a project first.
fn world(open: bool) -> (tempfile::TempDir, Arc<AppContext>, AgentService) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().to_path_buf();
    for sub in ["work", "projects", "workspaces"] {
        std::fs::create_dir(root.join(sub)).expect("a folder");
    }
    let app = Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(root.join("workspaces")),
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
            audit: AuditTarget::File(root.join("audit.jsonl")),
            tokens: Some(shared_tokens(sequential_tokens())),
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: root.join("projects"),
            }),
            ..AgentOptions::default()
        },
    )
    .expect("the service");
    if open {
        call(&service, "create_project", &json!({"name": "Controllers"}));
    }
    (dir, app, service)
}

fn call(service: &AgentService, tool: &str, args: &Value) -> Value {
    let reply = service.call(tool, Some(args));
    assert_eq!(reply["ok"], true, "{tool}: {reply}");
    reply
}

fn version(app: &AppContext) -> u64 {
    app.session().lock().version()
}

/// The error of a call that must fail; the project must be unchanged by it.
fn refused(app: &AppContext, service: &AgentService, args: &Value) -> Value {
    let before = version(app);
    let reply = service.call(TOOL, Some(args));
    assert_eq!(reply["ok"], false, "{args}: {reply}");
    assert_eq!(
        version(app),
        before,
        "{args}: a refused call changes nothing"
    );
    reply["error"].clone()
}

/// The `details.errors` of an argument-validation refusal.
fn validation_errors(app: &AppContext, service: &AgentService, args: &Value) -> Value {
    let error = refused(app, service, args);
    assert_eq!(error["code"], "INVALID_ARGUMENT", "{args}: {error}");
    assert_eq!(error["message"], format!("invalid arguments for {TOOL}"));
    error["details"]["errors"].clone()
}

/// A MIDI clip of four beats with two notes on a new instrument track.
fn midi_clip(service: &AgentService) -> String {
    let track = call(service, "create_track", &json!({"name": "Strings"}))["result"]["track_id"]
        .as_str()
        .expect("a track ID")
        .to_owned();
    let reply = call(
        service,
        "create_midi_clip",
        &json!({"track_id": track, "start": 0, "length": 4,
                "notes": [{"pitch": 60, "start": 0, "length": 2},
                          {"pitch": 64, "start": 2, "length": 2}]}),
    );
    reply["result"]["clip_id"]
        .as_str()
        .expect("a clip ID")
        .to_owned()
}

/// A stored controller event as `(beat, kind, number, value, channel)`.
type Stored = (f64, ControlKind, i64, i64, i64);

/// The controller events of the MIDI clip `clip_id`, in stored order.
fn controls(app: &AppContext, clip_id: &str) -> Vec<Stored> {
    let session = app.session().lock();
    let project = session.require_project().expect("an open project");
    let (_, clip) = project.clip(clip_id).expect("the clip");
    let Clip::Midi(clip) = clip else {
        panic!("{clip_id} is a MIDI clip");
    };
    clip.controls
        .iter()
        .map(|e| (e.beat, e.kind, e.number, e.value, e.channel))
        .collect()
}

/// The notes of the MIDI clip `clip_id` as `(pitch, start, length)`.
fn notes(app: &AppContext, clip_id: &str) -> Vec<(u8, f64, f64)> {
    let session = app.session().lock();
    let project = session.require_project().expect("an open project");
    let (_, clip) = project.clip(clip_id).expect("the clip");
    let Clip::Midi(clip) = clip else {
        panic!("{clip_id} is a MIDI clip");
    };
    clip.notes
        .iter()
        .map(|n| (n.pitch, n.start, n.length))
        .collect()
}

/// Stores `events` in the clip as the user's own data (a recorded take or an imported file),
/// through the core command, outside the tool.
fn seed(app: &AppContext, clip_id: &str, events: &[Stored]) {
    let events = events
        .iter()
        .map(|&(beat, kind, number, value, channel)| {
            MidiControlEvent::new(beat, kind, value, number, channel).expect("a valid event")
        })
        .collect();
    let command = SetControlEvents {
        clip_id: clip_id.to_owned(),
        events,
        start: 0.0,
        end: f64::INFINITY,
    };
    app.session()
        .lock()
        .execute(&command, false, "user")
        .expect("the seeded events");
}

/// A silent mono WAV file of `frames` frames at 48 kHz.
fn write_silence(path: &Path, frames: u32) {
    let len = frames * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&48_000_u32.to_le_bytes());
    bytes.extend_from_slice(&96_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.resize(bytes.len() + len as usize, 0);
    std::fs::write(path, bytes).expect("the WAV file");
}

#[test]
fn the_tool_is_an_undoable_edit_added_in_1_1() {
    let spec = find(TOOL).expect("the tool");
    assert_eq!(spec.safety, Safety::Write);
    assert!(!spec.idempotent);
    assert!(is_added_tool(TOOL));
    let (_dir, _app, service) = world(false);
    assert!(service.tools().iter().any(|tool| tool["name"] == TOOL));
}

#[test]
fn the_events_are_stored_in_beat_order_on_the_controller() {
    let (_dir, app, service) = world(true);
    let clip = midi_clip(&service);
    let reply = call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 11,
                "events": [{"beat": 2, "value": 90}, {"beat": 0, "value": 40},
                           {"beat": 1.5, "value": 64}]}),
    );
    assert_eq!(reply["committed"], true);
    assert_eq!(reply["dry_run"], false);
    assert_eq!(
        reply["changes"],
        json!({"created": [], "updated": [{"kind": "clip", "id": clip}], "deleted": [],
               "flags": ["notes"]})
    );
    assert_eq!(
        controls(&app, &clip),
        [
            (0.0, ControlKind::Cc, 11, 40, 0),
            (1.5, ControlKind::Cc, 11, 64, 0),
            (2.0, ControlKind::Cc, 11, 90, 0),
        ]
    );
    // The notes are untouched.
    assert_eq!(notes(&app, &clip), [(60, 0.0, 2.0), (64, 2.0, 2.0)]);
    // The new events carry the channel asked for.
    call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 64, "channel": 9,
                "events": [{"beat": 0, "value": 127}, {"beat": 3.75, "value": 0}]}),
    );
    let pedal: Vec<Stored> = controls(&app, &clip)
        .into_iter()
        .filter(|event| event.2 == 64)
        .collect();
    assert_eq!(
        pedal,
        [
            (0.0, ControlKind::Cc, 64, 127, 9),
            (3.75, ControlKind::Cc, 64, 0, 9)
        ]
    );
}

#[test]
fn a_write_replaces_its_controller_on_every_channel_and_keeps_the_rest() {
    let (_dir, app, service) = world(true);
    let clip = midi_clip(&service);
    seed(
        &app,
        &clip,
        &[
            (0.0, ControlKind::Cc, 11, 30, 0),
            (0.5, ControlKind::PitchBend, 0, -2048, 0),
            (1.0, ControlKind::Cc, 1, 70, 0),
            (2.0, ControlKind::ChannelPressure, 0, 50, 0),
            (3.0, ControlKind::Cc, 11, 80, 3),
            (3.5, ControlKind::Cc, 12, 5, 0),
        ],
    );
    call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 11, "events": [{"beat": 1, "value": 100}]}),
    );
    assert_eq!(
        controls(&app, &clip),
        [
            (0.5, ControlKind::PitchBend, 0, -2048, 0),
            (1.0, ControlKind::Cc, 1, 70, 0),
            (1.0, ControlKind::Cc, 11, 100, 0),
            (2.0, ControlKind::ChannelPressure, 0, 50, 0),
            (3.5, ControlKind::Cc, 12, 5, 0),
        ]
    );
}

#[test]
fn a_dry_run_reports_the_change_and_stores_nothing() {
    let (_dir, app, service) = world(true);
    let clip = midi_clip(&service);
    let before = version(&app);
    let reply = call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 1, "dry_run": true,
                "events": [{"beat": 0, "value": 10}]}),
    );
    assert_eq!(reply["committed"], false);
    assert_eq!(reply["dry_run"], true);
    assert_eq!(
        reply["changes"]["updated"],
        json!([{"kind": "clip", "id": clip}])
    );
    assert_eq!(version(&app), before);
    assert_eq!(controls(&app, &clip), []);
}

#[test]
fn one_undo_takes_a_whole_write_back() {
    let (_dir, app, service) = world(true);
    let clip = midi_clip(&service);
    call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 11,
                "events": [{"beat": 0, "value": 20}, {"beat": 2, "value": 40}]}),
    );
    let first = controls(&app, &clip);
    call(
        &service,
        TOOL,
        &json!({"clip_id": clip, "controller": 11,
                "events": [{"beat": 1, "value": 60}, {"beat": 3, "value": 80},
                           {"beat": 3.5, "value": 90}]}),
    );
    // Each call is its own undo step.
    let history = call(&service, "get_history", &json!({}));
    assert_eq!(
        history["undo"],
        json!([
            "Create Track",
            "Create MIDI Clip",
            "Edit Controllers",
            "Edit Controllers"
        ])
    );
    call(&service, "undo", &json!({}));
    assert_eq!(controls(&app, &clip), first);
    call(&service, "undo", &json!({}));
    assert_eq!(controls(&app, &clip), []);
    call(&service, "redo", &json!({}));
    assert_eq!(controls(&app, &clip), first);
}

#[test]
fn an_unknown_clip_is_not_found() {
    let (_dir, app, service) = world(true);
    midi_clip(&service);
    let error = refused(
        &app,
        &service,
        &json!({"clip_id": "clp_missing", "controller": 11, "events": [{"beat": 0, "value": 1}]}),
    );
    assert_eq!(error["code"], "NOT_FOUND", "{error}");
    assert_eq!(error["message"], "clip 'clp_missing' not found", "{error}");
}

#[test]
fn an_audio_clip_has_no_controllers() {
    let (dir, app, service) = world(true);
    write_silence(&dir.path().join("work/silence.wav"), 24_000);
    let imported = call(&service, "import_audio", &json!({"path": "silence.wav"}));
    let clip = imported["clip_id"]
        .as_str()
        .unwrap_or_else(|| panic!("a clip ID in {imported}"))
        .to_owned();
    let error = refused(
        &app,
        &service,
        &json!({"clip_id": clip, "controller": 11, "events": [{"beat": 0, "value": 1}]}),
    );
    assert_eq!(error["code"], "INVALID_ARGUMENT", "{error}");
    assert_eq!(
        error["message"],
        format!("clip '{clip}' is not a MIDI clip")
    );
}

#[test]
fn without_a_project_the_tool_refuses() {
    let (_dir, app, service) = world(false);
    let error = refused(
        &app,
        &service,
        &json!({"clip_id": "clp_1", "controller": 11, "events": [{"beat": 0, "value": 1}]}),
    );
    assert_eq!(error["code"], "NO_PROJECT", "{error}");
}

#[test]
fn arguments_out_of_their_bounds_fail_validation_with_field_paths() {
    let (_dir, app, service) = world(true);
    let clip = midi_clip(&service);
    let event = json!({"beat": 0, "value": 1});
    let cases = [
        (
            json!({"clip_id": clip, "controller": 128, "events": [event]}),
            "controller",
            "Input should be less than or equal to 127",
        ),
        (
            json!({"clip_id": clip, "controller": -1, "events": [event]}),
            "controller",
            "Input should be greater than or equal to 0",
        ),
        (
            json!({"clip_id": clip, "controller": 11, "events": [{"beat": 0, "value": 128}]}),
            "events.0.value",
            "Input should be less than or equal to 127",
        ),
        (
            json!({"clip_id": clip, "controller": 11,
                   "events": [event, {"beat": -0.5, "value": 1}]}),
            "events.1.beat",
            "Input should be greater than or equal to 0",
        ),
        (
            json!({"clip_id": clip, "controller": 11, "events": [{"beat": "inf", "value": 1}]}),
            "events.0.beat",
            "Input should be a finite number",
        ),
        (
            json!({"clip_id": clip, "controller": 11, "events": []}),
            "events",
            "List should have at least 1 item after validation, not 0",
        ),
        (
            json!({"clip_id": clip, "controller": 11, "events": [event], "channel": 16}),
            "channel",
            "Input should be less than or equal to 15",
        ),
        (
            json!({"controller": 11, "events": [event]}),
            "clip_id",
            "Field required",
        ),
        (
            json!({"clip_id": clip, "controller": 11, "events": [event], "start": 0}),
            "start",
            "Extra inputs are not permitted",
        ),
        (
            json!({"clip_id": clip, "controller": 11,
                   "events": [{"beat": 0, "value": 1, "channel": 2}]}),
            "events.0.channel",
            "Extra inputs are not permitted",
        ),
    ];
    for (args, field, message) in cases {
        let errors = validation_errors(&app, &service, &args);
        assert_eq!(
            errors,
            json!([{"field": field, "message": message}]),
            "{args}"
        );
    }
    let too_many: Vec<Value> = (0..20_001)
        .map(|i| json!({"beat": f64::from(i) / 1000.0, "value": 1}))
        .collect();
    let errors = validation_errors(
        &app,
        &service,
        &json!({"clip_id": clip, "controller": 11, "events": too_many}),
    );
    assert_eq!(errors[0]["field"], "events", "{errors}");
    assert_eq!(controls(&app, &clip), []);
}

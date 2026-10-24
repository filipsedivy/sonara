//! The piano roll keyboard's click: `AppContext::audition_note` plays or releases one preview
//! note of a track's instrument. Its failure paths (no project, a non-instrument track, a pitch
//! or velocity out of range) and, with a real player, that a preview note actually renders a
//! signal on a stopped transport.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use sonara_app::context::{AppContext, AppOptions};
use sonara_app::error::AppError;
use sonara_app::projects::NewProject;
use sonara_core::error::ErrorCode;
use sonara_core::ids::SequentialIds;
use tempfile::TempDir;

/// An application with a new project on `backend`, its own temporary workspace.
struct Live {
    app: Arc<AppContext>,
    _dir: TempDir,
}

impl Live {
    fn new(backend: &str) -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let app = AppContext::create(AppOptions {
            workspaces: Some(dir.path().join("ws")),
            persist_settings: false,
            audio_backend: Some(backend.to_owned()),
            ids: Some(sonara_app::ids::IdSource::new(Box::new(
                SequentialIds::new(0),
            ))),
            ..AppOptions::default()
        })
        .expect("the application");
        app.projects()
            .new_project(&NewProject {
                name: "Audition".to_owned(),
                ..NewProject::default()
            })
            .expect("a project");
        Self {
            app: Arc::new(app),
            _dir: dir,
        }
    }

    fn create(&self, name: &str, kind: &str, instrument: Option<&str>) -> String {
        use sonara_app::edit::{self, Edit};
        let outcome = edit::apply(
            &self.app,
            Edit::from_json(serde_json::json!({"op": "create_track", "name": name,
                "kind": kind, "instrument": instrument}))
            .expect("an edit"),
        )
        .expect("a track");
        outcome.result["track_id"]
            .as_str()
            .expect("a track ID")
            .to_owned()
    }
}

fn domain_code(error: &AppError) -> ErrorCode {
    match error {
        AppError::Domain(domain) => domain.code,
        other => panic!("a domain error: {other}"),
    }
}

#[test]
fn audition_note_needs_an_open_project() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let app = AppContext::create(AppOptions {
        workspaces: Some(dir.path().join("ws")),
        persist_settings: false,
        audio_backend: Some("native-null".to_owned()),
        ..AppOptions::default()
    })
    .expect("the application");
    let error = app
        .audition_note("trk_000000000001", 60, 100, true)
        .unwrap_err();
    assert_eq!(domain_code(&error), ErrorCode::NoProject);
}

#[test]
fn audition_note_needs_a_known_instrument_track() {
    let live = Live::new("native-null");
    let error = live
        .app
        .audition_note("trk_nope", 60, 100, true)
        .unwrap_err();
    assert_eq!(domain_code(&error), ErrorCode::NotFound);

    let audio = live.create("Vox", "audio", None);
    let error = live.app.audition_note(&audio, 60, 100, true).unwrap_err();
    assert_eq!(domain_code(&error), ErrorCode::InvalidArgument);
}

#[test]
fn audition_note_validates_pitch_and_velocity() {
    let live = Live::new("native-null");
    let synth = live.create("Synth", "instrument", Some("sonara.synth.poly"));
    let error = live.app.audition_note(&synth, 128, 100, true).unwrap_err();
    assert_eq!(domain_code(&error), ErrorCode::InvalidArgument);
    let error = live.app.audition_note(&synth, 60, 0, true).unwrap_err();
    assert_eq!(domain_code(&error), ErrorCode::InvalidArgument);
    // A velocity of 0 is fine for a NoteOff: nothing but the pitch matters there.
    live.app
        .audition_note(&synth, 60, 0, false)
        .expect("a NoteOff needs no velocity");
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

/// A GUI preview note reaches the graph and renders a signal, on a transport the audition
/// itself creates and builds a graph for, though nothing plays: the real oracle (the device's
/// measured output peak), not just "no panic" (TST-10).
#[test]
fn audition_note_sounds_on_a_stopped_transport() {
    let live = Live::new("native-null");
    let synth = live.create("Synth", "instrument", Some("sonara.synth.poly"));
    assert!(!live.app.has_transport(), "no transport before a click");

    live.app
        .audition_note(&synth, 60, 110, true)
        .expect("the preview note plays");
    assert!(live.app.has_transport(), "the click creates the transport");

    let transport = live.app.transport();
    assert!(
        wait_until(Duration::from_secs(5), || transport
            .output_peaks()
            .iter()
            .any(|&peak| peak > 0.0)),
        "the preview note shows in the device's output peaks"
    );

    live.app
        .audition_note(&synth, 60, 0, false)
        .expect("the preview note releases");
    drop(transport);
    live.app.close();
}

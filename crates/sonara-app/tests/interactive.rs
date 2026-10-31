//! The pieces of an interactive session that the desktop application composes: the recovery offer of scratch sessions, the autosave timer and its stop,
//! the autosave a discarded project drops, the delivery of session events from other
//! threads (the R part of `test_agent_edits_over_ipc_appear_in_gui_and_share_undo`; the IPC
//! side is in `sonara-agent`'s `gui_attach` test), and the saved window layout.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use sonara_app::autosave::{Autosaver, autosave_interval};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::events::{ChangeNotice, EventPump, SessionNotice, Subscription};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_app::window_layout::{
    Pane, PaneLayout, ScreenArea, WindowLayout, load_window_layout, save_window_layout,
};
use sonara_core::commands::track::CreateTrack;
use sonara_core::commands::{ChangeSet, Flag};
use sonara_core::ids::{self, IdGenerator as _};
use sonara_core::model::track::TrackKind;
use sonara_core::session::EventKind;
use tempfile::TempDir;

/// How long a test waits for a background thread before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

fn application(dir: &Path) -> Arc<AppContext> {
    Arc::new(
        AppContext::create(AppOptions {
            workspaces: Some(dir.join("workspaces")),
            settings: Some(Settings::default()),
            persist_settings: false,
            audio_backend: Some("null".to_owned()),
            ..AppOptions::default()
        })
        .expect("the application"),
    )
}

/// A new "Untitled" project from the "empty" template, as the desktop starts with.
fn untitled(app: &AppContext) -> PathBuf {
    app.projects()
        .new_project(&NewProject::default())
        .expect("a new project");
    app.session()
        .lock()
        .location()
        .expect("a location")
        .workspace
        .clone()
}

/// Creates an audio track with `source` as the edit's source; returns the track's ID.
fn create_track(app: &AppContext, name: &str, source: &str) -> String {
    let mut ids = app.ids();
    let command = CreateTrack {
        name: name.into(),
        kind: TrackKind::Audio,
        index: None,
        color: None,
        output_id: ids::MASTER_ID.into(),
        instrument_plugin: None,
        instrument_preset: None,
        parent_id: None,
        track_id: ids.new_id(ids::TRACK),
        instrument_id: ids.new_id(ids::DEVICE),
        part: None,
    };
    app.session()
        .lock()
        .execute(&command, false, source)
        .expect("the track is created");
    command.track_id
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < PATIENCE, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(10));
    }
}

// -- recovery and autosave ---------------------------------------------------------------------

#[test]
fn only_scratch_workspaces_with_an_autosave_are_offered() {
    let dir = TempDir::new().expect("a temporary folder");
    let app = application(dir.path());
    let workspace = untitled(&app);
    assert_eq!(
        app.projects()
            .recoverable_scratch_sessions()
            .expect("listed"),
        [],
        "a clean new project has no autosave"
    );
    create_track(&app, "Vocals", "gui");
    let written = app.projects().autosave().expect("autosaved");
    assert!(written.is_some());
    // An extracted archive's workspace with an autosave is not a scratch session: opening the
    // archive offers it instead.
    let extracted = dir.path().join("workspaces/arc-0123456789ab");
    fs::create_dir_all(extracted.join(".autosave")).expect("a folder");
    for file in ["info.json", "project.json"] {
        fs::copy(
            workspace.join(".autosave").join(file),
            extracted.join(".autosave").join(file),
        )
        .expect("copied");
    }
    let sessions = app
        .projects()
        .recoverable_scratch_sessions()
        .expect("listed");
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    assert_eq!(sessions[0].workspace, workspace);
    assert_eq!(sessions[0].name.as_deref(), Some("Untitled"));
    assert!(
        sessions[0]
            .saved_at
            .as_deref()
            .is_some_and(|stamp| stamp.ends_with('Z')),
        "{sessions:?}"
    );
    // Recovering it restores the edit, dirty.
    let other = application(dir.path());
    untitled(&other);
    let project = other
        .projects()
        .recover_session(&sessions[0].workspace)
        .expect("recovered");
    assert_eq!(project.tracks.len(), 1);
    assert!(other.session().lock().is_dirty());
}

#[test]
fn a_discarded_dirty_project_drops_its_autosave() {
    let dir = TempDir::new().expect("a temporary folder");
    let app = application(dir.path());
    let workspace = untitled(&app);
    assert!(!app.projects().discard_autosave(), "clean: nothing to drop");
    create_track(&app, "Vocals", "gui");
    app.projects().autosave().expect("autosaved");
    assert!(workspace.join(".autosave/info.json").exists());
    assert!(app.projects().discard_autosave());
    assert!(!workspace.join(".autosave").exists());
    assert_eq!(
        app.projects()
            .recoverable_scratch_sessions()
            .expect("listed"),
        []
    );
}

#[test]
fn the_autosave_interval_follows_the_settings() {
    assert_eq!(autosave_interval(60), Some(Duration::from_secs(60)));
    assert_eq!(autosave_interval(1), Some(Duration::from_secs(1)));
    assert_eq!(autosave_interval(0), None, "0 turns autosaving off");
    assert_eq!(autosave_interval(-5), None);
}

#[test]
fn the_autosaver_writes_dirty_projects_until_it_stops() {
    let dir = TempDir::new().expect("a temporary folder");
    let app = application(dir.path());
    let workspace = untitled(&app);
    let autosaver =
        Autosaver::start(Arc::clone(&app), Duration::from_millis(20)).expect("the timer runs");
    // A clean project is not autosaved.
    thread::sleep(Duration::from_millis(100));
    assert!(!workspace.join(".autosave").exists());
    create_track(&app, "Vocals", "agent");
    let info = workspace.join(".autosave/info.json");
    wait_until("the autosave", || info.exists());
    autosaver.stop();
    fs::remove_dir_all(workspace.join(".autosave")).expect("removed");
    create_track(&app, "Drums", "agent");
    thread::sleep(Duration::from_millis(200));
    assert!(!info.exists(), "a stopped timer writes nothing");
}

// -- events ------------------------------------------------------------------------------------

#[test]
fn the_pump_delivers_edits_from_every_thread_in_order() {
    let dir = TempDir::new().expect("a temporary folder");
    let app = application(dir.path());
    untitled(&app);
    let (sender, received) = mpsc::channel::<SessionNotice>();
    // An agent edit from a foreign thread after subscribing, before the pump runs: it waits in
    // the subscription.
    let subscription = Subscription::new(app.session());
    let agent = Arc::clone(&app);
    thread::spawn(move || create_track(&agent, "Strings", "agent"))
        .join()
        .expect("the agent thread");
    let pump = EventPump::start(subscription, move |event| {
        // The receiver outlives the pump in this test.
        sender.send(SessionNotice::from(event)).expect("delivered");
    })
    .expect("the pump runs");
    // Then the GUI's own undo and a redo.
    app.session().lock().undo("gui").expect("undone");
    app.session().lock().redo("gui").expect("redone");
    let notices: Vec<SessionNotice> = (0..3)
        .map(|_| received.recv_timeout(PATIENCE).expect("an event"))
        .collect();
    let kinds: Vec<(&str, &str, &str)> = notices
        .iter()
        .map(|n| (n.kind, n.source.as_str(), n.label.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("command", "agent", "Create Track"),
            ("undo", "gui", "Create Track"),
            ("redo", "gui", "Create Track"),
        ]
    );
    assert!(
        notices
            .windows(2)
            .all(|pair| pair[0].version < pair[1].version)
    );
    assert_eq!(
        notices[0].foreign_edit().as_deref(),
        Some("agent: Create Track")
    );
    assert_eq!(notices[1].foreign_edit(), None, "the GUI's own undo");
    pump.stop();
    create_track(&app, "Bass", "agent");
    assert!(
        received.recv_timeout(Duration::from_millis(200)).is_err(),
        "a stopped pump delivers nothing"
    );
}

#[test]
fn foreign_edits_name_their_source() {
    let notice = |source: &str, label: &str| SessionNotice {
        version: 3,
        kind: "command",
        label: label.to_owned(),
        source: source.to_owned(),
        changes: ChangeNotice::of(EventKind::Command, &ChangeSet::flags([Flag::Tempo])),
    };
    assert_eq!(
        notice("sonara-mcp", "Set Tempo").foreign_edit().as_deref(),
        Some("sonara-mcp: Set Tempo")
    );
    assert_eq!(notice("user", "Set Tempo").foreign_edit(), None);
    assert_eq!(notice("gui", "Set Tempo").foreign_edit(), None);
    assert_eq!(
        notice("agent", "").foreign_edit(),
        None,
        "loads have no label"
    );
    let json = serde_json::to_value(notice("agent", "Set Tempo")).expect("serializable");
    assert_eq!(
        json,
        serde_json::json!({"version": 3, "kind": "command", "label": "Set Tempo", "source": "agent",
            "changes": {"created": [], "updated": [], "deleted": [], "flags": ["tempo"],
                        "structure_changed": false}})
    );
}

// -- window layout -----------------------------------------------------------------------------

#[test]
fn the_window_layout_round_trips() {
    let dir = TempDir::new().expect("a temporary folder");
    let path = dir.path().join("config/window.json");
    assert_eq!(load_window_layout(&path), None, "nothing saved yet");
    let layout = WindowLayout {
        x: 120.0,
        y: 64.5,
        width: 1440.0,
        height: 900.0,
        maximized: false,
        panes: PaneLayout {
            browser: Pane {
                size: 300.0,
                open: false,
            },
            ..PaneLayout::default()
        },
    };
    save_window_layout(&layout, &path).expect("saved");
    assert_eq!(load_window_layout(&path), Some(layout));
    // An older file without `maximized` and the panes reads as not maximized, with the
    // prototype's dock sizes (240, 280 and 300 pixels, all showing).
    fs::write(&path, r#"{"x": 1, "y": 2, "width": 800, "height": 600}"#).expect("written");
    let old = load_window_layout(&path).expect("a layout");
    assert!(!old.maximized);
    assert_eq!(old.panes, PaneLayout::default());
    assert_eq!(
        (
            old.panes.browser.size,
            old.panes.inspector.size,
            old.panes.editors.size
        ),
        (240.0, 280.0, 300.0)
    );
    // A pane without a usable size takes its default; the others keep theirs.
    fs::write(
        &path,
        r#"{"x": 1, "y": 2, "width": 800, "height": 600, "panes": {
            "browser": {"size": -5, "open": false}, "editors": {"size": 420, "open": true}}}"#,
    )
    .expect("written");
    let panes = load_window_layout(&path).expect("a layout").panes;
    assert_eq!(
        panes.browser,
        Pane {
            size: 240.0,
            open: false
        }
    );
    assert_eq!(panes.editors.size, 420.0);
    assert_eq!(panes.inspector, PaneLayout::default().inspector);
    for bad in [
        "not json",
        r#"{"x": 0, "y": 0, "width": -1, "height": 600}"#,
        r#"{"x": 0, "y": 0, "width": 0, "height": 600}"#,
        r#"{"x": 0}"#,
    ] {
        fs::write(&path, bad).expect("written");
        assert_eq!(load_window_layout(&path), None, "{bad}");
    }
}

#[test]
fn a_layout_off_every_display_is_not_visible() {
    let main = ScreenArea {
        x: 0.0,
        y: 0.0,
        width: 1728.0,
        height: 1117.0,
    };
    let side = ScreenArea {
        x: 1728.0,
        y: -200.0,
        width: 2560.0,
        height: 1440.0,
    };
    let at = |x: f64, y: f64| WindowLayout {
        x,
        y,
        width: 1440.0,
        height: 900.0,
        maximized: false,
        panes: PaneLayout::default(),
    };
    assert!(at(100.0, 50.0).is_visible_on(&[main]));
    assert!(at(2000.0, -150.0).is_visible_on(&[main, side]));
    assert!(
        !at(2000.0, -150.0).is_visible_on(&[main]),
        "the side display is gone"
    );
    assert!(
        !at(-1430.0, 50.0).is_visible_on(&[main]),
        "only 10 px on the display"
    );
    assert!(
        !at(100.0, 1100.0).is_visible_on(&[main]),
        "the title bar is below"
    );
    assert!(!at(100.0, 50.0).is_visible_on(&[]));
}

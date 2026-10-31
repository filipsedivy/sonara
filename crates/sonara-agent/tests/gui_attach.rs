//! An agent attached to the desktop's session (the R part of the prototype's
//! `test_agent_edits_over_ipc_appear_in_gui_and_share_undo` and
//! `test_gui_edit_is_visible_to_agent`;).
//!
//! The desktop runs the IPC server with app kind `gui` on its application and forwards the
//! session's events to its window through an [`EventPump`]. Here the pump's sink stands in for
//! the window: an agent's edits, made over IPC on connection threads, reach it in order; the
//! GUI's undo and the agent's redo walk one shared history; and the GUI's own edit is visible
//! to the agent. The end-to-end run with the real window and `sonara mcp` is the ignored test
//! in `apps/sonara-desktop/src-tauri/tests/attach.rs`.
//!
//! Unix only, like the IPC tests: the server listens on a Unix domain socket there.
#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use sonara_agent::ipc::{ClientOptions, IpcClient, IpcServer, ServerOptions, find_sessions};
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::events::{EventPump, SessionNotice, Subscription};
use sonara_app::projects::NewProject;
use sonara_app::settings::Settings;
use sonara_core::commands::track::CreateTrack;
use sonara_core::ids::{self, IdGenerator as _};
use sonara_core::model::track::TrackKind;
use tempfile::TempDir;

/// How long the window may take to see an event.
const PATIENCE: Duration = Duration::from_secs(10);

/// The desktop's session: the application with a new "Untitled" project, the IPC server with
/// app kind `gui`, and the event pump whose sink is the "window".
struct Desktop {
    _dir: TempDir,
    runtime: PathBuf,
    app: Arc<AppContext>,
    server: Option<IpcServer>,
    pump: Option<EventPump>,
    window: Receiver<SessionNotice>,
}

impl Desktop {
    fn start() -> Self {
        let dir = tempfile::tempdir().expect("a temporary folder");
        let root = fs::canonicalize(dir.path()).expect("the folder resolves");
        let app = Arc::new(
            AppContext::create(AppOptions {
                workspaces: Some(root.join("workspaces")),
                settings: Some(Settings::default()),
                persist_settings: false,
                audio_backend: Some("null".to_owned()),
                ..AppOptions::default()
            })
            .expect("the application"),
        );
        app.projects()
            .new_project(&NewProject::default())
            .expect("Untitled");
        let (sender, window) = mpsc::channel();
        let pump = EventPump::start(Subscription::new(app.session()), move |event| {
            // The window may have closed already; the pump keeps going regardless.
            let _ = sender.send(SessionNotice::from(event));
        })
        .expect("the pump runs");
        let agent = AgentService::new(
            Arc::clone(&app),
            AgentOptions {
                base_dir: Some(root.clone()),
                audit: AuditTarget::File(root.join("audit.jsonl")),
                allowed_paths: vec![root.clone()],
                ..AgentOptions::default()
            },
        )
        .expect("the agent service");
        // Short, so that the socket path stays within the 100-byte limit.
        let runtime = root.join("run");
        let server = IpcServer::start(
            Arc::new(agent),
            ServerOptions {
                app_kind: "gui".to_owned(),
                directory: Some(runtime.clone()),
            },
        )
        .expect("the server starts");
        Self {
            _dir: dir,
            runtime,
            app,
            server: Some(server),
            pump: Some(pump),
            window,
        }
    }

    /// The next event the window receives.
    fn next(&self) -> SessionNotice {
        self.window.recv_timeout(PATIENCE).expect("an event")
    }

    fn track_ids(&self) -> Vec<String> {
        self.app
            .session()
            .project()
            .map(|project| project.tracks.iter().map(|t| t.id.clone()).collect())
            .unwrap_or_default()
    }

    /// The desktop's shutdown order: the pump, the IPC server, the application.
    fn close(mut self) {
        if let Some(pump) = self.pump.take() {
            pump.stop();
        }
        if let Some(server) = self.server.take() {
            server.close();
        }
        self.app.close();
    }
}

/// The envelope of a successful call.
fn ok(envelope: &Value) -> &Value {
    assert_eq!(envelope["ok"], true, "{envelope}");
    envelope
}

#[test]
fn agent_edits_over_ipc_reach_the_window_and_share_undo() {
    let desktop = Desktop::start();
    let sessions = find_sessions(&desktop.runtime);
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].app, "gui");
    let client = Arc::new(
        IpcClient::connect_discovered(
            &desktop.runtime,
            &ClientOptions {
                client: "sonara-mcp".to_owned(),
                ..ClientOptions::default()
            },
        )
        .expect("attached"),
    );
    assert_eq!(client.discovery().app, "gui");
    // The agent works on a foreign thread, as the MCP server's connection does.
    let agent = Arc::clone(&client);
    let (track, generated) = thread::spawn(move || {
        let track = agent
            .call(
                "create_track",
                Some(&json!({"name": "Agent Strings", "preset": "Strings"})),
            )
            .expect("an envelope");
        let track_id = track["result"]["track_id"]
            .as_str()
            .expect("an ID")
            .to_owned();
        let generated = agent
            .call(
                "generate_midi",
                Some(&json!({
                    "track_id": track_id,
                    "kind": "chords",
                    "key": "D minor",
                    "progression": ["i", "VI", "III", "VII"],
                })),
            )
            .expect("an envelope");
        (track, generated)
    })
    .join()
    .expect("the agent's thread");
    let track_id = ok(&track)["result"]["track_id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    ok(&generated);
    // The window saw both edits, in order.
    let created = desktop.next();
    let chords = desktop.next();
    // Agent edits keep the service's source; the client's name goes to the audit log.
    assert_eq!(
        (
            created.kind,
            created.source.as_str(),
            created.label.as_str()
        ),
        ("command", "agent", "Create Track")
    );
    assert_eq!(
        created.foreign_edit().as_deref(),
        Some("agent: Create Track")
    );
    assert_eq!(chords.source, "agent");
    assert!(created.version < chords.version);
    assert!(desktop.track_ids().contains(&track_id));
    // The GUI's undo removes the agent's clip and then the track: one shared history.
    for _ in 0..2 {
        desktop.app.session().lock().undo("gui").expect("undone");
    }
    let undone: Vec<(&str, String)> = (0..2)
        .map(|_| desktop.next())
        .map(|n| (n.kind, n.source))
        .collect();
    assert_eq!(
        undone,
        [("undo", "gui".to_owned()), ("undo", "gui".to_owned())]
    );
    assert!(!desktop.track_ids().contains(&track_id));
    // ...and the agent's redo brings the track back.
    let redo = client.call("redo", Some(&json!({}))).expect("an envelope");
    ok(&redo);
    let redone = desktop.next();
    assert_eq!((redone.kind, redone.source.as_str()), ("redo", "agent"));
    assert!(desktop.track_ids().contains(&track_id));
    drop(client);
    desktop.close();
}

#[test]
fn the_guis_edit_is_visible_to_the_agent() {
    let desktop = Desktop::start();
    let client = IpcClient::connect_discovered(&desktop.runtime, &ClientOptions::default())
        .expect("attached");
    let before = client
        .call("list_tracks", Some(&json!({})))
        .expect("an envelope");
    let count = ok(&before)["tracks"].as_array().expect("tracks").len();
    // The GUI's own edit (source "gui"), seen by the agent through the same session.
    gui_adds_an_audio_track(&desktop.app);
    let after = client
        .call("list_tracks", Some(&json!({})))
        .expect("an envelope");
    let tracks = ok(&after)["tracks"].as_array().expect("tracks");
    assert_eq!(tracks.len(), count + 1);
    assert_eq!(tracks[count]["kind"], "audio");
    let notice = desktop.next();
    assert_eq!(
        (notice.source.as_str(), notice.foreign_edit()),
        ("gui", None)
    );
    drop(client);
    desktop.close();
}

/// `window.add_track(TrackKind.AUDIO)`: the GUI creates an audio track with source `gui`.
fn gui_adds_an_audio_track(app: &AppContext) {
    let mut ids = app.ids();
    let command = CreateTrack {
        name: "Audio 1".into(),
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
        .execute(&command, false, "gui")
        .expect("the GUI's edit");
}

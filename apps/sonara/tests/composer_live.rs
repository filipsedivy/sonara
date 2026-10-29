//! The AI composer against a real Claude Code (local only, like the hardware tests of HW-01).
//!
//! It needs a signed-in `claude` and spends a little of the user's quota, so it is ignored by
//! default: `cargo test -p sonara --test composer_live -- --ignored --nocapture`.
//!
//! A headless `sonara serve` stands in for the GUI window: the run is pinned to its session
//! exactly as the desktop pins it to its own, through a copy of its discovery file.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::json;
use sonara_agent::ipc::{ClientOptions, IpcClient};
use sonara_app::composer::{self, Event, Run, Workspace};

const PROMPT: &str = "Set the tempo to 97 BPM and create one instrument track named Probe. \
Then try to call save_project, and tell me in one sentence whether that tool exists for you. \
Do nothing else.";

/// A headless session that is stopped when it drops.
struct Session {
    child: Child,
    home: tempfile::TempDir,
}

impl Session {
    fn start() -> Self {
        let home = tempfile::tempdir().expect("a home");
        let child = Command::new(env!("CARGO_BIN_EXE_sonara"))
            .args(["serve", "--audio", "null"])
            .env("SONARA_HOME", home.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sonara serve starts");
        let mut session = Self { child, home };
        // The status line names the discovery file once the server listens.
        let stderr = session.child.stderr.take().expect("stderr");
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let line = rx
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("sonara serve reports that it listens");
            if line.contains("listening on") {
                break;
            }
        }
        session
    }

    fn discovery(&self) -> PathBuf {
        let run = self.home.path().join("run");
        fs::read_dir(&run)
            .expect("the run directory")
            .map(|entry| entry.expect("an entry").path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("session-"))
            })
            .expect("a discovery file")
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.child.kill() {
            eprintln!("cannot stop sonara serve: {error}");
        }
        if let Err(error) = self.child.wait() {
            eprintln!("cannot reap sonara serve: {error}");
        }
    }
}

#[test]
#[ignore = "needs a signed-in Claude Code and spends quota; run with --ignored"]
fn a_real_run_stops_on_request_and_reports_it() {
    let claude = composer::locate().expect("Claude Code is installed");
    let session = Session::start();
    let client =
        IpcClient::connect_discovered(&session.home.path().join("run"), &ClientOptions::default())
            .expect("the session accepts a client");
    client
        .call("create_project", Some(&json!({"name": "Stop test"})))
        .expect("a project is created");
    let workspace = Workspace::create(&session.discovery()).expect("a workspace");
    let invocation = composer::build(
        &claude.path,
        Path::new(env!("CARGO_BIN_EXE_sonara")),
        &workspace,
        "Create twenty instrument tracks named Track 1 to Track 20, one call at a time, \
         and say a sentence after each.",
    );
    let (tx, rx) = mpsc::channel();
    let run = Run::start(&invocation, workspace, move |event| {
        tx.send(event).ok();
    })
    .expect("the run starts");
    // Stop at the first answer of a tool of Sonara's.
    let mut events = Vec::new();
    let mut stopped_at = None;
    loop {
        let event = rx
            .recv_timeout(Duration::from_secs(120))
            .expect("the run ends in time");
        if stopped_at.is_none() && matches!(event, Event::ToolResult { .. }) {
            run.stop();
            stopped_at = Some(Instant::now());
        }
        let exited = matches!(event, Event::Exited { .. });
        events.push(event);
        if exited {
            break;
        }
    }
    let took = stopped_at
        .expect("a tool answered before the end")
        .elapsed();
    eprintln!("stopped in {took:?}");
    assert!(
        took < Duration::from_secs(10),
        "it stopped promptly, without escalation"
    );
    assert!(
        matches!(events.last(), Some(Event::Exited { stopped: true, .. })),
        "{:?}",
        events.last()
    );
}

#[test]
#[ignore = "needs a signed-in Claude Code and spends quota; run with --ignored"]
fn a_real_run_composes_into_the_pinned_session_and_cannot_save() {
    let claude = composer::locate().expect("Claude Code is installed");
    let session = Session::start();
    // The window always has a project open; a headless session starts without one.
    let client =
        IpcClient::connect_discovered(&session.home.path().join("run"), &ClientOptions::default())
            .expect("the session accepts a client");
    let created = client
        .call(
            "create_project",
            Some(&json!({"name": "Live test", "bpm": 120})),
        )
        .expect("a project is created");
    assert_eq!(created["ok"], json!(true), "{created}");
    let workspace = Workspace::create(&session.discovery()).expect("a workspace");
    let invocation = composer::build(
        &claude.path,
        Path::new(env!("CARGO_BIN_EXE_sonara")),
        &workspace,
        PROMPT,
    );

    let (tx, rx) = mpsc::channel();
    let _run = Run::start(&invocation, workspace, move |event| {
        tx.send(event).ok();
    })
    .expect("the run starts");
    let mut events = Vec::new();
    loop {
        let event = rx
            .recv_timeout(Duration::from_secs(240))
            .expect("the run ends in time");
        eprintln!("{event:?}");
        let exited = matches!(event, Event::Exited { .. });
        events.push(event);
        if exited {
            break;
        }
    }

    let connected = events.iter().any(|event| {
        matches!(event, Event::Init { mcp_servers, .. }
            if mcp_servers.iter().any(|s| s.name == "sonara" && s.status == "connected"))
    });
    assert!(connected, "the Sonara tools connected");
    let used = |tool: &str| {
        events
            .iter()
            .any(|event| matches!(event, Event::ToolUse { tool: t, .. } if t == tool))
    };
    assert!(used("create_track"), "the agent created a track");
    assert!(
        !events.iter().any(|event| matches!(
            event,
            Event::ToolResult { ok: true, id, .. } if events.iter().any(|e| matches!(
                e, Event::ToolUse { id: use_id, tool, .. } if use_id == id && tool == "save_project"))
        )),
        "save_project never succeeded"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Result { success: true, .. })),
        "the run succeeded"
    );

    // The edits are in the session the run was pinned to.
    let state = client
        .call("get_project_state", Some(&json!({})))
        .expect("the project state")
        .to_string();
    assert!(
        state.contains("Probe"),
        "the track is in the project: {state}"
    );
    assert!(state.contains("97"), "the tempo is in the project: {state}");
}

#[test]
#[ignore = "needs a signed-in Claude Code and spends quota; run with --ignored"]
fn a_real_run_can_search_and_fetch_from_the_web() {
    let claude = composer::locate().expect("Claude Code is installed");
    let session = Session::start();
    let workspace = Workspace::create(&session.discovery()).expect("a workspace");
    let invocation = composer::build(
        &claude.path,
        Path::new(env!("CARGO_BIN_EXE_sonara")),
        &workspace,
        "Use WebSearch to find the typical tempo range of lo-fi hip hop, then use WebFetch on \
         https://en.wikipedia.org/wiki/Lo-fi_music, and answer in one sentence. Do not touch the \
         project.",
    );
    let (tx, rx) = mpsc::channel();
    let _run = Run::start(&invocation, workspace, move |event| {
        tx.send(event).ok();
    })
    .expect("the run starts");
    let mut events = Vec::new();
    loop {
        let event = rx
            .recv_timeout(Duration::from_secs(240))
            .expect("the run ends in time");
        let exited = matches!(event, Event::Exited { .. });
        events.push(event);
        if exited {
            break;
        }
    }
    for tool in ["WebSearch", "WebFetch"] {
        let id = events
            .iter()
            .find_map(|event| match event {
                Event::ToolUse { id, tool: t, .. } if t == tool => Some(id.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the agent used {tool}"));
        assert!(
            events.iter().any(|event| matches!(
                event,
                Event::ToolResult { id: result_id, ok: true, .. } if *result_id == id
            )),
            "{tool} answered: {events:#?}"
        );
    }
}

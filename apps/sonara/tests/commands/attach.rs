//! Attaching to a running session (the port of the prototype's `tests/agent/test_mcp_attach.py`):
//! `sonara serve` runs a headless session with the IPC server in one
//! folder, and the MCP server, started from another folder, attaches to it, as Claude Code does
//! by default. Relative paths in tool calls resolve against the MCP client's folder, which does
//! not open up the rest of the file system, and the edits land in the running session.
//!
//! The prototype exported audio to check the paths ([`mcp_attach_export`]); the paths of
//! `export_midi` are checked the same way.
//!
//! Unix only: the session is stopped with SIGTERM.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{Value, json};

use crate::mcp_stdio::Server;
use crate::support::Sandbox;

/// A running `sonara serve`.
struct Serve {
    child: Child,
}

impl Serve {
    /// Starts `sonara serve` in `cwd` and waits until it listens.
    fn start(sandbox: &Sandbox, cwd: &Path) -> Self {
        let mut child = sandbox
            .command()
            .current_dir(cwd)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sonara serve starts");
        let mut stderr = BufReader::new(child.stderr.take().expect("stderr is piped"));
        loop {
            let mut line = String::new();
            let read = stderr
                .read_line(&mut line)
                .expect("the session's stderr is readable");
            assert!(read > 0, "serve ended early");
            if line.starts_with("sonara serve: listening on ") {
                assert!(line.trim_end().ends_with("; Ctrl+C to stop"), "{line}");
                break;
            }
        }
        // Keep draining stderr so that the session never blocks on it.
        std::thread::spawn(move || {
            let mut sink = String::new();
            while stderr.read_line(&mut sink).is_ok_and(|n| n > 0) {
                sink.clear();
            }
        });
        Self { child }
    }

    /// Stops the session with SIGTERM and returns its exit code.
    fn stop(mut self) -> i32 {
        let pid = self.child.id().to_string();
        let killed = Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .expect("kill runs");
        assert!(killed.success());
        let status = self.child.wait().expect("the session can be waited for");
        status.code().expect("the session exits normally")
    }
}

impl Drop for Serve {
    /// A failed test must not leave the session running.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The discovery files of running sessions in the sandbox's runtime folder.
fn discovery_files(sandbox: &Sandbox) -> Vec<PathBuf> {
    let run = sandbox.root().join("home/run");
    let Ok(entries) = std::fs::read_dir(&run) else {
        return Vec::new();
    };
    entries
        .map(|entry| entry.expect("a directory entry").path())
        .filter(|path| {
            let named = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.starts_with("session-"));
            named && path.extension().is_some_and(|ext| ext == "json")
        })
        .collect()
}

fn folders(sandbox: &Sandbox) -> (PathBuf, PathBuf) {
    let gui = sandbox.root().join("gui-cwd");
    let client = sandbox.root().join("claude-project");
    std::fs::create_dir(&gui).expect("the session's folder");
    std::fs::create_dir(&client).expect("the client's folder");
    (gui, client)
}

/// `create_project`, a drum track with one generated bar, as the prototype's scenario.
fn build_song(mcp: &mut Server) -> Value {
    let create = mcp.call(2, "create_project", json!({"name": "Attached", "bpm": 120}));
    let track = mcp.call(
        3,
        "create_track",
        json!({"name": "Drums", "instrument": "sonara.drums"}),
    );
    let track_id = track["result"]["track_id"]
        .as_str()
        .expect("create_track returns the track ID")
        .to_owned();
    let generated = mcp.call(
        4,
        "generate_midi",
        json!({"track_id": track_id, "kind": "drums", "bars": 1}),
    );
    assert_eq!(generated["ok"], true, "{generated}");
    create
}

#[test]
fn mcp_attaches_to_running_session_and_uses_client_paths() {
    let sandbox = Sandbox::new();
    let (gui, client) = folders(&sandbox);
    let session = Serve::start(&sandbox, &gui);
    assert_eq!(discovery_files(&sandbox).len(), 1);

    let client_arg = client.to_str().unwrap();
    let mut mcp = Server::spawn(
        &sandbox,
        &client,
        &["mcp", "--connect", "always", "--base-dir", client_arg],
    );
    mcp.initialize("2025-06-18");
    let create = build_song(&mut mcp);
    assert_eq!(create["ok"], true, "{create}");
    let export = mcp.call(5, "export_midi", json!({"path": "out/attached.mid"}));
    assert_eq!(export["ok"], true, "{export}");
    let outside = sandbox.root().join("elsewhere.mid");
    let denied = mcp.call(6, "export_midi", json!({"path": outside.to_str().unwrap()}));
    let (code, stderr) = mcp.finish();
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("sonara-mcp ready: connected to running Sonara (headless) at "),
        "{stderr}"
    );

    // Relative paths resolve against the MCP client's folder, not the session's.
    assert!(client.join("out/attached.mid").exists());
    assert!(!gui.join("out").exists());
    // The client's base folder does not open up the rest of the file system.
    assert_eq!(denied["ok"], false);
    assert_eq!(denied["error"]["code"], "PERMISSION_DENIED");
    assert!(!outside.exists());

    // The project the MCP client built lives in the running session: other clients see it,
    // with `--connect always` and with the default `auto`.
    let state = sandbox.run(&["tool", "get_project_state", "--connect", "always"]);
    assert_eq!(state.code, 0, "{}", state.stderr);
    assert_eq!(state.json["state"]["name"], "Attached");
    let tracks = sandbox.run(&["tool", "list_tracks"]);
    assert_eq!(
        tracks.json["tracks"][0]["name"], "Drums",
        "{}",
        tracks.stdout
    );

    assert_eq!(session.stop(), 0);
    assert_eq!(
        discovery_files(&sandbox),
        Vec::<PathBuf>::new(),
        "the session removes its discovery file"
    );
}

#[test]
fn batch_runs_in_the_running_session() {
    let sandbox = Sandbox::new();
    let (gui, _) = folders(&sandbox);
    let session = Serve::start(&sandbox, &gui);
    let file = sandbox.root().join("steps.json");
    std::fs::write(
        &file,
        serde_json::to_vec(&json!([
            {"id": "p", "tool": "create_project", "args": {"name": "Shared"}},
            {"tool": "create_track", "args": {"name": "Bus", "kind": "bus"}},
        ]))
        .unwrap(),
    )
    .unwrap();
    let run = sandbox.run(&["batch", file.to_str().unwrap(), "--connect", "always"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    // A second process sees the batch's project: the calls ran in the session.
    let tracks = sandbox.run(&["tool", "list_tracks", "--connect", "always"]);
    assert_eq!(tracks.json["tracks"][0]["name"], "Bus");
    // `--connect never` runs in-process, in a fresh session.
    let fresh = sandbox.run(&["tool", "list_tracks", "--connect", "never"]);
    assert_eq!(fresh.json["error"]["code"], "NO_PROJECT");
    assert_eq!(session.stop(), 0);
}

#[test]
fn mcp_attach_export() {
    let sandbox = Sandbox::new();
    let (gui, client) = folders(&sandbox);
    let session = Serve::start(&sandbox, &gui);
    let client_arg = client.to_str().unwrap();
    let mut mcp = Server::spawn(
        &sandbox,
        &client,
        &["mcp", "--connect", "always", "--base-dir", client_arg],
    );
    mcp.initialize("2025-06-18");
    build_song(&mut mcp);
    let export = mcp.call(5, "export_audio", json!({"path": "out/attached.wav"}));
    let outside = sandbox.root().join("elsewhere.wav");
    let denied = mcp.call(
        6,
        "export_audio",
        json!({"path": outside.to_str().unwrap()}),
    );
    mcp.finish();
    assert_eq!(export["ok"], true, "{export}");
    assert!(client.join("out/attached.wav").exists());
    assert!(!gui.join("out").exists());
    assert_eq!(denied["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(session.stop(), 0);
}

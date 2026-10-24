//! Claude Code's view of the desktop session, end to end (the E part of the
//! prototype's `test_agent_edits_over_ipc_appear_in_gui_and_share_undo`).
//!
//! `sonara gui` starts the `sonara-desktop` next to it; `sonara mcp --connect always` attaches
//! to the window's session over IPC, as Claude Code does; its edit reaches the window (the
//! status bar's "agent: Create Track", which the desktop logs), and a second client's undo and
//! the MCP server's redo walk one shared history. The event delivery itself and the GUI's own
//! undo are tested in the container (`sonara-agent`'s `gui_attach`, `sonara-app`'s
//! `interactive`).
//!
//! It opens a window, so it needs a macOS display: `cargo test -p sonara-desktop --test attach
//! -- --ignored`. Both executables come from the same build (`cargo build --workspace`). In a
//! debug build the page loads from Vite's development server (`devUrl`); without one the window
//! shows an empty page, which this test does not look at.

#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

/// How long the window may take to start, and a line to appear.
const PATIENCE: Duration = Duration::from_secs(60);

/// A sandbox: `<root>/home` is `SONARA_HOME`, `<root>/projects` is `SONARA_PROJECTS_DIR`.
struct Sandbox {
    _dir: TempDir,
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        // Short, so that the IPC socket path stays within its 100-byte limit.
        let dir = tempfile::Builder::new()
            .prefix("sd-")
            .tempdir_in("/tmp")
            .expect("a temporary folder");
        let root = fs::canonicalize(dir.path()).expect("the folder resolves");
        fs::create_dir_all(root.join("home/config")).expect("a config folder");
        fs::create_dir(root.join("projects")).expect("a projects folder");
        // No audio device and no autosave timer: the test edits only.
        fs::write(
            root.join("home/config/settings.json"),
            r#"{"audio": {"backend": "null"}, "autosave_seconds": 0}"#,
        )
        .expect("the settings");
        Self { _dir: dir, root }
    }

    /// The built `sonara` next to the built `sonara-desktop`.
    fn sonara() -> PathBuf {
        let sonara = Path::new(env!("CARGO_BIN_EXE_sonara-desktop")).with_file_name("sonara");
        assert!(
            sonara.is_file(),
            "{} is missing: build the workspace first (cargo build --workspace)",
            sonara.display()
        );
        sonara
    }

    fn command(&self) -> Command {
        let mut command = Command::new(Self::sonara());
        command
            .current_dir(&self.root)
            .env("SONARA_HOME", self.root.join("home"))
            .env("SONARA_PROJECTS_DIR", self.root.join("projects"))
            .env_remove("SONARA_AGENT_ALLOWED_PATHS");
        command
    }

    /// The discovery file of a running `gui` session, once it is written.
    fn wait_for_gui_session(&self) -> Value {
        let run = self.root.join("home/run");
        let start = Instant::now();
        loop {
            let found = fs::read_dir(&run)
                .into_iter()
                .flatten()
                .flatten()
                .find_map(|entry| {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let named = name.starts_with("session-")
                        && Path::new(&name)
                            .extension()
                            .is_some_and(|ext| ext == "json");
                    let text = fs::read_to_string(entry.path()).ok().filter(|_| named)?;
                    serde_json::from_str::<Value>(&text).ok()
                });
            if let Some(discovery) = found {
                return discovery;
            }
            assert!(
                start.elapsed() < PATIENCE,
                "the desktop wrote no discovery file"
            );
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// The running desktop, started through `sonara gui`; its stderr lines arrive on `log`.
struct Desktop {
    child: Child,
    log: Receiver<String>,
}

impl Desktop {
    fn start(sandbox: &Sandbox) -> Self {
        let mut child = sandbox
            .command()
            .arg("gui")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sonara gui starts");
        let stderr = BufReader::new(child.stderr.take().expect("stderr is piped"));
        let (lines, log) = mpsc::channel();
        thread::spawn(move || {
            for line in stderr.lines().map_while(Result::ok) {
                if lines.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, log }
    }

    /// Waits for a log line containing `text`.
    fn expect_log(&self, text: &str) -> String {
        let start = Instant::now();
        let mut seen = Vec::new();
        while let Some(left) = PATIENCE.checked_sub(start.elapsed()) {
            match self.log.recv_timeout(left) {
                Ok(line) if line.contains(text) => return line,
                Ok(line) => seen.push(line),
                Err(_) => break,
            }
        }
        panic!(
            "the desktop never logged {text:?}; it logged:\n{}",
            seen.join("\n")
        );
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        // The window has no scripted quit yet; the sandbox goes with the test.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `sonara mcp --connect always`, driven with JSON-RPC lines as Claude Code drives it.
struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr: PathBuf,
    next_id: u64,
}

impl Mcp {
    fn attach(sandbox: &Sandbox) -> Self {
        let stderr = sandbox.root.join("mcp.log");
        let mut child = sandbox
            .command()
            .args(["mcp", "--connect", "always"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(&stderr).expect("the MCP log"))
            .spawn()
            .expect("sonara mcp starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        let mut mcp = Self {
            child,
            stdin,
            stdout,
            stderr,
            next_id: 1,
        };
        let init = mcp.request(
            "initialize",
            &json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "claude-code", "version": "1"},
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "sonara", "{init}");
        mcp.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        mcp
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        writeln!(stdin, "{message}").expect("the server reads stdin");
        stdin.flush().expect("the server reads stdin");
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .expect("stdout is readable");
            assert!(read > 0, "the MCP server ended before answering {method}");
            let reply: Value = serde_json::from_str(&line).expect("one JSON message per line");
            if reply["id"] == json!(id) {
                return reply;
            }
        }
    }

    /// A tool call's envelope.
    fn call(&mut self, name: &str, arguments: &Value) -> Value {
        let reply = self.request("tools/call", &json!({"name": name, "arguments": arguments}));
        reply["result"]["structuredContent"].clone()
    }

    fn track_names(&mut self) -> Vec<String> {
        let tracks = self.call("list_tracks", &json!({}));
        tracks["tracks"]
            .as_array()
            .expect("a track list")
            .iter()
            .map(|track| track["name"].as_str().expect("a name").to_owned())
            .collect()
    }

    /// Closes stdin and returns the server's stderr.
    fn finish(mut self) -> String {
        drop(self.stdin.take());
        let _ = self.child.wait();
        fs::read_to_string(&self.stderr).expect("the MCP log")
    }
}

#[test]
#[ignore = "opens a window: needs a macOS display (cargo test -p sonara-desktop --test attach -- --ignored)"]
fn sonara_mcp_attaches_to_the_desktop_and_shares_its_history() {
    let sandbox = Sandbox::new();
    let desktop = Desktop::start(&sandbox);
    let discovery = sandbox.wait_for_gui_session();
    assert_eq!(discovery["app"], "gui", "{discovery}");
    desktop.expect_log("agents attach at");

    let mut mcp = Mcp::attach(&sandbox);
    let created = mcp.call(
        "create_track",
        &json!({"name": "Agent Strings", "kind": "audio"}),
    );
    assert_eq!(created["ok"], true, "{created}");
    assert!(mcp.track_names().contains(&"Agent Strings".to_owned()));
    // The window's status bar shows the agent's edit.
    desktop.expect_log("status: sonara-mcp: Create Track");

    // Another client (the CLI) undoes the MCP server's edit: one history for every client.
    let undo = sandbox
        .command()
        .args(["tool", "undo", "--connect", "always"])
        .output()
        .expect("sonara tool runs");
    let undone: Value = serde_json::from_slice(&undo.stdout).expect("a JSON envelope");
    assert_eq!(undone["ok"], true, "{undone}");
    desktop.expect_log("status: sonara-cli: Create Track");
    assert!(!mcp.track_names().contains(&"Agent Strings".to_owned()));
    let redo = mcp.call("redo", &json!({}));
    assert_eq!(redo["ok"], true, "{redo}");
    assert!(mcp.track_names().contains(&"Agent Strings".to_owned()));

    let log = mcp.finish();
    assert!(
        log.contains("connected to running Sonara (gui)"),
        "the MCP server did not attach to the window: {log}"
    );
}

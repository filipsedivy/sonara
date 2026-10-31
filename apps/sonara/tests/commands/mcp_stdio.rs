//! The real MCP server over stdio (the port of the prototype's `tests/agent/test_mcp_stdio.py`):
//! `sonara mcp` driven with JSON-RPC lines, as the golden recorder drove the
//! prototype's `sonara-mcp`.
//!
//! The prototype's scenario ends with an export ([`mcp_stdio_export`]). Attaching to a running
//! session is `attach`.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};

use serde_json::{Value, json};
use sonara_agent::contract::versions::{missing_additions, recorded_tools};
use sonara_agent::handlers::builtin;
use sonara_agent::mcp::{INSTRUCTIONS, dumps};
use sonara_testkit::Golden;

use crate::support::Sandbox;

/// A running `sonara mcp` and its pipes.
pub(crate) struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr_path: PathBuf,
}

impl Server {
    /// `sonara mcp --connect never --audio null --base-dir ROOT --allow ROOT` in `sandbox`.
    fn start(sandbox: &Sandbox) -> Self {
        let root = sandbox.root().to_str().expect("a UTF-8 sandbox path");
        Self::spawn(
            sandbox,
            sandbox.root(),
            &[
                "mcp",
                "--connect",
                "never",
                "--audio",
                "null",
                "--base-dir",
                root,
                "--allow",
                root,
            ],
        )
    }

    /// `sonara ARGS` in `cwd`, with `sandbox`'s home; stderr goes to `<root>/server.log`.
    pub(crate) fn spawn(sandbox: &Sandbox, cwd: &Path, args: &[&str]) -> Self {
        let stderr_path = sandbox.root().join("server.log");
        let mut child = sandbox
            .command()
            .current_dir(cwd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(&stderr_path).expect("the server log"))
            .spawn()
            .expect("sonara mcp starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Self {
            child,
            stdin,
            stdout,
            stderr_path,
        }
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin is open until finish");
        let line = serde_json::to_string(message).expect("a JSON message");
        writeln!(stdin, "{line}").expect("the server reads stdin");
        stdin.flush().expect("the server reads stdin");
    }

    /// Sends a request and returns the reply with its ID (one JSON object per line).
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        message["params"] = params;
        self.send(&message);
        loop {
            let mut line = String::new();
            let read = self
                .stdout
                .read_line(&mut line)
                .expect("the server's stdout is readable");
            assert!(
                read > 0,
                "the server closed stdout before answering {method}"
            );
            let reply: Value = serde_json::from_str(&line).expect("one JSON message per line");
            if reply.get("id") == Some(&json!(id)) {
                return reply;
            }
        }
    }

    /// `initialize` with `version`, then `notifications/initialized`; the `initialize` result.
    pub(crate) fn initialize(&mut self, version: &str) -> Value {
        let reply = self.request(
            1,
            "initialize",
            json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "sonara-test", "version": "1"},
            }),
        );
        self.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        reply["result"].clone()
    }

    /// `tools/call`: checks the result's shape and returns the envelope.
    pub(crate) fn call(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        let mut params = json!({"name": name});
        params["arguments"] = arguments;
        let reply = self.request(id, "tools/call", params);
        let result = &reply["result"];
        let envelope = result["structuredContent"].clone();
        assert!(envelope.is_object(), "{reply}");
        // The text mirrors the structured result for clients without structured output: the
        // envelope as `json.dumps(envelope, indent=1)`.
        let content = result["content"].as_array().expect("a content list");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], dumps(&envelope, 1));
        assert_eq!(result["isError"], json!(envelope["ok"] != true));
        // A 2025-06-18 peer gets the result without the newer result fields.
        assert!(result.get("resultType").is_none(), "{result}");
        envelope
    }

    /// Closes stdin and returns the exit code and the server's stderr.
    pub(crate) fn finish(mut self) -> (i32, String) {
        drop(self.stdin.take());
        let status = self.child.wait().expect("the server can be waited for");
        let stderr = std::fs::read_to_string(&self.stderr_path).expect("the server log");
        (status.code().expect("the server exits normally"), stderr)
    }
}

#[test]
fn mcp_stdio_end_to_end() {
    let mut golden: Value = Golden::open()
        .and_then(|golden| golden.json("contracts/mcp-tools-list.json"))
        .unwrap();
    // GLD-01 deviation: the recorder pinned the prototype's own version (0.1.0).
    // `serverInfo.version` reports the current application version (VER-01), so it moves with
    // every release; pin it to the build's version instead of the recording's.
    golden["server_info"]["version"] = json!(env!("CARGO_PKG_VERSION"));
    let sandbox = Sandbox::new();
    let mut server = Server::start(&sandbox);

    let init = server.initialize("2025-06-18");
    assert_eq!(init["protocolVersion"], golden["protocol_version"]);
    assert_eq!(init["serverInfo"], golden["server_info"]);
    assert_eq!(init["capabilities"], golden["capabilities"]);
    assert_eq!(init["instructions"], INSTRUCTIONS);
    assert!(INSTRUCTIONS.contains("render_preview"));

    // The listing: every registered tool. Without the recorded additions of API 1.1
    // (`contract::versions`, CTR-01), each is exactly as the prototype listed it, in the same
    // order; the recording is API 1.0 (GLD-01).
    let listing = server.request(2, "tools/list", json!({}));
    let published = listing["result"]["tools"].as_array().unwrap();
    assert!(listing["result"].get("ttlMs").is_none());
    let registered: Vec<&str> = builtin().specs().iter().map(|spec| spec.name).collect();
    let names: Vec<&str> = published
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, registered);
    assert_eq!(missing_additions(published), []);
    let listed = recorded_tools(published);
    let recorded = golden["tools"].as_array().unwrap();
    for tool in &listed {
        let expected = recorded.iter().find(|t| t["name"] == tool["name"]).unwrap();
        assert_eq!(tool, expected, "{}", tool["name"]);
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let listed_names: Vec<&str> = listed.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let all: Vec<&str> = recorded
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(listed_names, all, "every recorded tool is listed");

    let create = server.call(3, "create_project", json!({"name": "MCP Song", "bpm": 128}));
    assert_eq!(create["ok"], true);
    assert_eq!(create["state"]["tempo"][0]["bpm"], json!(128.0));
    let track = server.call(
        4,
        "create_track",
        json!({"name": "Drums", "kind": "instrument", "instrument": "sonara.drums",
               "preset": "Punchy 909"}),
    );
    let track_id = track["result"]["track_id"].as_str().unwrap().to_owned();
    let generated = server.call(
        5,
        "generate_midi",
        json!({"track_id": track_id, "kind": "drums", "style": "house", "bars": 2}),
    );
    assert_eq!(generated["ok"], true);
    assert!(generated["note_count"].as_u64().unwrap() > 10);
    let bad = server.call(6, "create_track", json!({"name": "x", "kind": "trumpet"}));
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["error"]["code"], "INVALID_ARGUMENT");
    // An unknown tool is a failed envelope, not a protocol error.
    let unknown = server.call(7, "no_such_tool", json!({}));
    assert_eq!(unknown["error"]["code"], "NOT_FOUND");
    // Calls without arguments get `{}`.
    let reply = server.request(8, "tools/call", json!({"name": "list_tracks"}));
    assert_eq!(reply["result"]["structuredContent"]["ok"], true);
    assert_eq!(server.request(9, "ping", json!({}))["result"], json!({}));

    let (code, stderr) = server.finish();
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stderr.contains("WARNING sonara.mcp: sonara-mcp ready: headless in-process session"),
        "{stderr}"
    );
}

#[test]
fn mcp_falls_back_to_the_newest_handshake_version() {
    // An unknown protocol version gets the SDK's newest version with an `initialize` handshake,
    // as the prototype's SDK answers (`LATEST_HANDSHAKE_VERSION`).
    let sandbox = Sandbox::new();
    let mut server = Server::start(&sandbox);
    let init = server.initialize("1999-01-01");
    assert_eq!(init["protocolVersion"], "2025-11-25");
    let (code, _) = server.finish();
    assert_eq!(code, 0);
}

#[test]
fn mcp_ends_quietly_when_the_client_never_initializes() {
    let sandbox = Sandbox::new();
    let server = Server::start(&sandbox);
    let (code, stderr) = server.finish();
    assert_eq!(code, 0, "{stderr}");
}

#[test]
fn mcp_stdio_export() {
    let sandbox = Sandbox::new();
    let mut server = Server::start(&sandbox);
    server.initialize("2025-06-18");
    server.call(2, "create_project", json!({"name": "MCP Song", "bpm": 128}));
    let track = server.call(
        3,
        "create_track",
        json!({"name": "Drums", "kind": "instrument", "instrument": "sonara.drums",
               "preset": "Punchy 909"}),
    );
    let track_id = track["result"]["track_id"].as_str().unwrap().to_owned();
    server.call(
        4,
        "generate_midi",
        json!({"track_id": track_id, "kind": "drums", "style": "house", "bars": 2}),
    );
    let export = server.call(5, "export_audio", json!({"path": "mcp-out/drums.wav"}));
    assert_eq!(export["ok"], true, "{export}");
    let wav = export["result"]["outputs"][0]["path"].as_str().unwrap();
    assert_eq!(
        std::path::Path::new(wav),
        sandbox.root().join("mcp-out/drums.wav")
    );
    let (code, _) = server.finish();
    assert_eq!(code, 0);
    // The prototype also measured the file: integrated loudness above -40 LUFS and more than
    // 3.5 s (two bars at 128 BPM plus the tail).
    let run = sandbox.run(&["analyze", wav]);
    assert!(run.json["analysis"]["integrated_lufs"].as_f64().unwrap() > -40.0);
    assert!(run.json["analysis"]["duration_seconds"].as_f64().unwrap() > 3.5);
}

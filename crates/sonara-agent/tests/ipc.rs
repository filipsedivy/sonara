//! The IPC server and client (`sonara.agent.ipc`): the
//! prototype's `tests/agent/test_ipc.py` (discovery, authentication, protocol checks and shared
//! session state), plus the client context, the audit source, closing and the socket path. The
//! protocol's every message and error is replayed against the prototype's recording in
//! `ipc_protocol.rs`.
//!
//! Like the prototype's suite, the tests that depend on Unix domain sockets or process ids run on
//! Unix only; `round_trip_over_the_native_transport` runs everywhere (TCP on Windows).

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};
use sonara_agent::context::PolicyEnvironment;
use sonara_agent::contract::API_VERSION;
use sonara_agent::ipc::{
    ClientContext, ClientError, ClientOptions, IpcClient, IpcServer, ServerOptions, Transport,
    find_sessions,
};
use sonara_agent::service::{AgentOptions, AgentService, AuditTarget};
use sonara_app::context::{AppContext, AppOptions};
use sonara_app::settings::Settings;
use sonara_core::error::ErrorCode;
use tempfile::TempDir;

/// An application with the null audio backend in a temporary folder (the prototype's `app`
/// fixture), and the runtime directory of the servers.
struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    app: Arc<AppContext>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temporary directory");
        // Resolved, as the sandbox resolves its roots (`/var` is a link on macOS).
        let root = fs::canonicalize(dir.path()).expect("the directory resolves");
        for sub in ["workspaces", "projects"] {
            fs::create_dir(root.join(sub)).expect("a folder");
        }
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
        Self {
            _dir: dir,
            root,
            app,
        }
    }

    /// Short, so that the socket path stays within the 100-byte limit.
    fn runtime(&self) -> PathBuf {
        self.root.join("run")
    }

    fn options(&self) -> AgentOptions {
        AgentOptions {
            base_dir: Some(self.root.clone()),
            source: "ipc".to_owned(),
            audit: AuditTarget::File(self.root.join("audit.jsonl")),
            allowed_paths: vec![self.root.clone()],
            environment: Some(PolicyEnvironment {
                allowed: None,
                projects_dir: self.root.join("projects"),
            }),
            ..AgentOptions::default()
        }
    }

    fn agent(&self, options: AgentOptions) -> Arc<AgentService> {
        Arc::new(AgentService::new(Arc::clone(&self.app), options).expect("the agent service"))
    }

    /// The prototype's `server` fixture.
    fn server(&self) -> IpcServer {
        self.server_for(self.agent(self.options()))
    }

    fn server_for(&self, agent: Arc<AgentService>) -> IpcServer {
        IpcServer::start(
            agent,
            ServerOptions {
                app_kind: "test".to_owned(),
                directory: Some(self.runtime()),
            },
        )
        .expect("the server starts")
    }

    fn connect(&self, options: &ClientOptions) -> IpcClient {
        IpcClient::connect_discovered(&self.runtime(), options).expect("a running session")
    }

    fn track_names(&self) -> Vec<String> {
        self.app
            .session()
            .project()
            .map(|project| {
                project
                    .tracks
                    .iter()
                    .map(|track| track.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// A connection that writes raw lines (`_raw` of the prototype's tests).
struct Raw {
    reader: BufReader<Box<dyn Read>>,
    writer: Box<dyn Write>,
}

impl Raw {
    fn connect(server: &IpcServer) -> Self {
        let (reader, writer): (Box<dyn Read>, Box<dyn Write>) = match server.transport() {
            #[cfg(unix)]
            Transport::Unix => {
                let stream =
                    std::os::unix::net::UnixStream::connect(server.address()).expect("connects");
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .expect("a timeout");
                (
                    Box::new(stream.try_clone().expect("cloned")),
                    Box::new(stream),
                )
            }
            #[cfg(not(unix))]
            Transport::Unix => unreachable!("the server listens on TCP here"),
            Transport::Tcp => {
                let stream = std::net::TcpStream::connect(server.address()).expect("connects");
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .expect("a timeout");
                (
                    Box::new(stream.try_clone().expect("cloned")),
                    Box::new(stream),
                )
            }
        };
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }

    fn send_line(&mut self, line: &str) {
        assert!(self.try_send(line), "sent");
    }

    /// Whether the line could be sent: a closed connection refuses it.
    fn try_send(&mut self, line: &str) -> bool {
        match self.writer.write_all(format!("{line}\n").as_bytes()) {
            Ok(()) => true,
            Err(error) if is_closed(&error) => false,
            Err(error) => panic!("cannot send: {error}"),
        }
    }

    fn send(&mut self, message: &Value) {
        self.send_line(&message.to_string());
    }

    /// The next reply, or `None` once the server closed the connection.
    fn reply(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(serde_json::from_str(&line).expect("a JSON reply")),
            // Linux resets a Unix domain socket closed with unread data.
            Err(error) if is_closed(&error) => None,
            Err(error) => panic!("cannot read: {error}"),
        }
    }

    /// Sends the messages one by one and collects the replies until the connection closes.
    fn exchange(server: &IpcServer, messages: &[Value]) -> Vec<Value> {
        let mut raw = Self::connect(server);
        let mut replies = Vec::new();
        for message in messages {
            if !raw.try_send(&message.to_string()) {
                break;
            }
            match raw.reply() {
                Some(reply) => replies.push(reply),
                None => break,
            }
        }
        replies
    }
}

/// The errors of a connection the other side closed.
fn is_closed(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
    )
}

fn hello(token: &str, protocol: i64) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "hello", "params": {"token": token, "protocol": protocol}})
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    fs::metadata(path).expect("exists").permissions().mode() & 0o777
}

#[cfg(unix)]
#[test]
fn discovery_file_is_private_and_found() {
    let fixture = Fixture::new();
    let server = fixture.server();
    assert_eq!(mode(server.discovery_path()), 0o600);
    assert_eq!(mode(&fixture.runtime()), 0o700);
    assert_eq!(mode(Path::new(server.address())), 0o600, "the socket");
    let sessions = find_sessions(&fixture.runtime());
    let addresses: Vec<&str> = sessions.iter().map(|d| d.address.as_str()).collect();
    assert_eq!(addresses, [server.address()]);
    assert_eq!(sessions[0].app, "test");
    assert_eq!(sessions[0].protocol, sonara_agent::ipc::PROTOCOL_VERSION);
    assert_eq!(sessions[0].transport, Transport::Unix);
    assert_eq!(sessions[0].token, server.token());
    assert_eq!(sessions[0].pid, i64::from(std::process::id()));
    assert_eq!(sessions[0].path, server.discovery_path());
    assert_eq!(
        server.discovery_path(),
        fixture
            .runtime()
            .join(format!("session-{}.json", std::process::id()))
    );
    // `json.dump`'s layout and key order.
    let text = fs::read_to_string(server.discovery_path()).expect("readable");
    let prefix = format!(
        r#"{{"pid": {}, "transport": "unix", "address": "{}", "token": "{}", "protocol": 1, "app": "test", "started": "#,
        std::process::id(),
        server.address(),
        server.token()
    );
    assert!(text.starts_with(&prefix), "{text}");
    assert!(server.token().len() == 64 && server.token().bytes().all(|b| b.is_ascii_hexdigit()));
}

#[test]
fn round_trip_mutates_server_session() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let client = fixture.connect(&ClientOptions::default());
    assert_eq!(client.server_info()["server"], "sonara");
    assert_eq!(client.discovery().address, server.address());
    assert!(client.ping().expect("pong"));
    let names: BTreeSet<String> = client
        .tools()
        .expect("tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect();
    let listed: BTreeSet<String> = fixture
        .agent(fixture.options())
        .tools()
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name").to_owned())
        .collect();
    assert_eq!(names, listed);
    assert!(names.contains("create_project") && names.contains("undo"));
    let res = client
        .call(
            "create_project",
            Some(&json!({"name": "Over IPC", "bpm": 101})),
        )
        .expect("an envelope");
    assert_eq!(res["ok"], true, "{res}");
    let res = client
        .call(
            "create_track",
            Some(&json!({"name": "Remote", "kind": "bus"})),
        )
        .expect("an envelope");
    assert_eq!(res["ok"], true, "{res}");
    // The server-side session (which the GUI would observe) holds the edit.
    let project = fixture.app.session().project().expect("a project");
    assert_eq!(project.name, "Over IPC");
    assert_eq!(fixture.track_names(), ["Remote"]);
    // Errors travel inside the envelope, not as transport failures.
    let bad = client
        .call(
            "rename_track",
            Some(&json!({"track_id": "trk_nope", "name": "x"})),
        )
        .expect("an envelope");
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["error"]["code"], "NOT_FOUND");
    // Undo through IPC is the session's undo.
    assert_eq!(client.call("undo", None).expect("an envelope")["ok"], true);
    assert_eq!(fixture.track_names(), Vec::<String>::new());
}

#[test]
fn wrong_token_rejected() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let replies = Raw::exchange(
        &server,
        &[
            hello("bad", 1),
            json!({"jsonrpc": "2.0", "id": 2, "method": "info"}),
        ],
    );
    assert_eq!(
        replies,
        [
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32001, "message": "authentication failed"}})
        ],
        "the connection closes"
    );
}

#[test]
fn must_authenticate_first() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let replies = Raw::exchange(
        &server,
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}})],
    );
    assert_eq!(replies[0]["error"]["code"], -32001);
}

#[test]
fn protocol_mismatch_rejected() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let replies = Raw::exchange(
        &server,
        &[hello(server.token(), 999), hello(server.token(), 1)],
    );
    assert_eq!(
        replies,
        [
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": -32002, "message": "protocol mismatch: server speaks 1"}})
        ],
        "the connection closes"
    );
}

#[test]
fn parse_errors_and_unknown_methods() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let mut raw = Raw::connect(&server);
    raw.send_line("this is not json");
    assert_eq!(
        raw.reply().expect("a reply"),
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "parse error"}})
    );
    raw.send(&hello(server.token(), 1));
    let welcome = raw.reply().expect("a reply");
    assert_eq!(
        welcome["result"],
        json!({"server": "sonara", "protocol": 1, "api_version": API_VERSION, "project": null,
               "tools": fixture.agent(fixture.options()).tools().len()})
    );
    raw.send(&json!({"jsonrpc": "2.0", "id": 2, "method": "nope", "params": {}}));
    let reply = raw.reply().expect("a reply");
    assert_eq!(
        reply,
        json!({"jsonrpc": "2.0", "id": 2, "error": {"code": -32000, "message": "unknown method 'nope'",
               "data": {"code": "NOT_FOUND", "message": "unknown method 'nope'"}}})
    );
}

#[test]
fn client_with_wrong_token_raises() {
    let fixture = Fixture::new();
    let _server = fixture.server();
    let mut forged = find_sessions(&fixture.runtime()).remove(0);
    forged.token = "0".repeat(64);
    let error = IpcClient::connect(forged, &ClientOptions::default()).expect_err("refused");
    assert_eq!(error.code(), ErrorCode::PermissionDenied);
    assert!(error.to_string().contains("authentication"));
    let domain = error.to_domain();
    assert_eq!(domain.message, "authentication failed");
    assert_eq!(domain.details["rpc_code"], -32001);
}

#[cfg(unix)]
#[test]
fn stale_discovery_files_are_removed() {
    let fixture = Fixture::new();
    let runtime = fixture.runtime();
    fs::create_dir_all(&runtime).expect("created");
    let discovery = |pid: u32, address: &Path, started: f64| {
        json!({"pid": pid, "transport": "unix", "address": address, "token": "x", "protocol": 1,
               "started": started})
        .to_string()
    };
    let dead = runtime.join("session-999999.json");
    fs::write(
        &dead,
        discovery(999_999, &fixture.root.join("gone.sock"), 1.0),
    )
    .expect("written");
    let missing_socket = runtime.join(format!("session-{}-x.json", std::process::id()));
    fs::write(
        &missing_socket,
        discovery(std::process::id(), &fixture.root.join("no.sock"), 2.0),
    )
    .expect("written");
    let garbage = runtime.join("session-garbage.json");
    fs::write(&garbage, "{not json").expect("written");
    assert_eq!(find_sessions(&runtime), []);
    assert!(!dead.exists() && !missing_socket.exists());
    assert!(
        garbage.exists(),
        "unreadable files are skipped, not removed"
    );
    assert!(IpcClient::connect_discovered(&runtime, &ClientOptions::default()).is_none());
}

#[cfg(unix)]
#[test]
fn sessions_are_listed_newest_first() {
    let fixture = Fixture::new();
    let runtime = fixture.runtime();
    fs::create_dir_all(&runtime).expect("created");
    // A live process with existing "sockets": this process and plain files.
    for (name, started) in [("a", 5.0), ("b", 9.0), ("c", 1.0)] {
        let address = fixture.root.join(format!("{name}.sock"));
        fs::write(&address, "").expect("written");
        let body = json!({"pid": std::process::id(), "transport": "unix", "address": address, "token": name,
                          "protocol": 1, "app": name, "started": started});
        fs::write(
            runtime.join(format!("session-{name}.json")),
            body.to_string(),
        )
        .expect("written");
    }
    // Not a discovery file name.
    fs::write(runtime.join("other.json"), "{}").expect("written");
    let apps: Vec<String> = find_sessions(&runtime).into_iter().map(|d| d.app).collect();
    assert_eq!(apps, ["b", "a", "c"]);
}

#[test]
fn concurrent_clients_share_one_session() {
    let fixture = Fixture::new();
    let _server = fixture.server();
    let first = fixture.connect(&ClientOptions::default());
    assert_eq!(
        first
            .call("create_project", Some(&json!({"name": "Shared"})))
            .expect("an envelope")["ok"],
        true
    );
    let runtime = fixture.runtime();
    let workers: Vec<_> = (0..4)
        .map(|i| {
            let runtime = runtime.clone();
            thread::spawn(move || {
                let client = IpcClient::connect_discovered(&runtime, &ClientOptions::default())
                    .ok_or_else(|| "no session".to_owned())?;
                for j in 0..5 {
                    let res = client
                        .call(
                            "create_track",
                            Some(&json!({"name": format!("T{i}-{j}"), "kind": "bus"})),
                        )
                        .map_err(|error| error.to_string())?;
                    if res["ok"] != true {
                        return Err(res.to_string());
                    }
                }
                Ok::<(), String>(())
            })
        })
        .collect();
    for worker in workers {
        worker
            .join()
            .expect("the worker finishes")
            .expect("every call succeeds");
    }
    drop(first);
    let mut names = fixture.track_names();
    names.sort();
    let mut expected: Vec<String> = (0..4)
        .flat_map(|i| (0..5).map(move |j| format!("T{i}-{j}")))
        .collect();
    expected.sort();
    assert_eq!(names, expected);
    // 20 independent commits, each its own undo step (after the project's creation).
    let session = fixture.app.session().lock();
    assert_eq!(session.history().labels().0.len(), 20);
}

#[cfg(unix)]
#[test]
fn server_close_removes_discovery() {
    let fixture = Fixture::new();
    let options = AgentOptions {
        audit: AuditTarget::Disabled,
        ..fixture.options()
    };
    let server = fixture.server_for(fixture.agent(options));
    let path = server.discovery_path().to_path_buf();
    let address = PathBuf::from(server.address());
    assert!(path.exists() && address.exists());
    server.close();
    assert!(!path.exists() && !address.exists());
    assert_eq!(find_sessions(&fixture.runtime()), []);
}

#[test]
fn dropping_the_server_closes_it() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let path = server.discovery_path().to_path_buf();
    drop(server);
    assert!(!path.exists());
    assert!(IpcClient::connect_discovered(&fixture.runtime(), &ClientOptions::default()).is_none());
}

/// As in the prototype (daemon connection threads), closing stops new connections but keeps
/// serving the open ones.
#[test]
fn close_keeps_open_connections() {
    let fixture = Fixture::new();
    let server = fixture.server();
    let client = fixture.connect(&ClientOptions::default());
    let discovery = client.discovery().clone();
    server.close();
    let res = client.call("list_tracks", None).expect("still served");
    assert_eq!(res["error"]["code"], "NO_PROJECT");
    let error =
        IpcClient::connect(discovery, &ClientOptions::default()).expect_err("not listening");
    assert_eq!(error.code(), ErrorCode::EngineUnavailable);
    assert!(
        error
            .to_string()
            .starts_with("cannot connect to Sonara at "),
        "{error}"
    );
}

/// The prototype's MCP attach test, over IPC: relative paths resolve against the client's
/// directory, which is allowed; the rest of the file system stays closed.
#[test]
fn client_context_sets_the_working_directory() {
    let fixture = Fixture::new();
    let gui = fixture.root.join("gui-cwd");
    let client_dir = fixture.root.join("claude-project");
    fs::create_dir(&gui).expect("created");
    fs::create_dir(&client_dir).expect("created");
    let options = AgentOptions {
        base_dir: Some(gui.clone()),
        allowed_paths: Vec::new(),
        source: "agent".to_owned(),
        audit: AuditTarget::Disabled,
        ..fixture.options()
    };
    let _server = fixture.server_for(fixture.agent(options));
    let client = fixture.connect(&ClientOptions {
        client: "sonara-mcp".to_owned(),
        context: Some(ClientContext {
            base_dir: Some(client_dir.clone()),
            allowed_paths: Vec::new(),
        }),
        ..ClientOptions::default()
    });
    let created = client
        .call(
            "create_project",
            Some(&json!({"name": "Attached", "bpm": 120})),
        )
        .expect("an envelope");
    assert_eq!(created["ok"], true, "{created}");
    let saved = client
        .call(
            "save_project",
            Some(&json!({"path": "songs/attached.sonara"})),
        )
        .expect("an envelope");
    assert_eq!(saved["ok"], true, "{saved}");
    assert!(client_dir.join("songs").join("attached.sonara").exists());
    assert!(!gui.join("songs").exists());
    let outside = client
        .call(
            "save_project",
            Some(&json!({"path": fixture.root.join("elsewhere.sonara")})),
        )
        .expect("an envelope");
    assert_eq!(outside["ok"], false);
    assert_eq!(outside["error"]["code"], "PERMISSION_DENIED");
    assert_eq!(
        fixture.app.session().project().expect("a project").name,
        "Attached"
    );
}

/// Without a context the server's own agent serves the client (its source); with one, a view
/// named after the client (at most 64 characters).
#[test]
fn the_client_name_is_the_audit_source_of_its_view() {
    let fixture = Fixture::new();
    let _server = fixture.server();
    let plain = fixture.connect(&ClientOptions {
        client: "plain".to_owned(),
        ..ClientOptions::default()
    });
    let long_name = "n".repeat(100);
    let viewed = fixture.connect(&ClientOptions {
        client: long_name.clone(),
        context: Some(ClientContext {
            base_dir: Some(fixture.root.clone()),
            allowed_paths: vec![fixture.root.join("projects")],
        }),
        ..ClientOptions::default()
    });
    plain.call("list_tracks", None).expect("an envelope");
    viewed.call("list_tracks", None).expect("an envelope");
    let audit = fs::read_to_string(fixture.root.join("audit.jsonl")).expect("the audit log");
    let sources: Vec<String> = audit
        .lines()
        .map(|line| {
            let entry: Value = serde_json::from_str(line).expect("JSON");
            entry["source"].as_str().expect("a source").to_owned()
        })
        .collect();
    assert_eq!(sources, ["ipc".to_owned(), long_name[..64].to_owned()]);
}

#[cfg(unix)]
#[test]
fn long_runtime_directories_use_a_short_socket_path() {
    let fixture = Fixture::new();
    let deep = fixture.root.join("d".repeat(60)).join("e".repeat(40));
    let options = AgentOptions {
        audit: AuditTarget::Disabled,
        ..fixture.options()
    };
    let server = IpcServer::start(
        fixture.agent(options),
        ServerOptions {
            app_kind: "test".to_owned(),
            directory: Some(deep.clone()),
        },
    )
    .expect("the server starts");
    let address = PathBuf::from(server.address());
    assert_eq!(
        address.file_name().and_then(|name| name.to_str()),
        Some(format!("s-{}.sock", std::process::id()).as_str())
    );
    let folder = address.parent().expect("a folder");
    assert!(folder.starts_with(std::env::temp_dir()));
    assert!(
        folder
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("sonara-"))
    );
    assert_eq!(mode(folder), 0o700);
    assert_eq!(server.discovery_path().parent(), Some(deep.as_path()));
    let client =
        IpcClient::connect_discovered(&deep, &ClientOptions::default()).expect("a session");
    assert!(client.ping().expect("pong"));
    drop(client);
    server.close();
    assert!(!address.exists());
}

#[test]
fn round_trip_over_the_native_transport() {
    let fixture = Fixture::new();
    let server = fixture.server();
    assert_eq!(server.transport(), Transport::native());
    let client = fixture.connect(&ClientOptions::default());
    assert!(client.ping().expect("pong"));
    let res = client
        .call("list_tracks", Some(&json!({})))
        .expect("an envelope");
    assert_eq!(res["error"]["code"], "NO_PROJECT");
    let mut raw = Raw::connect(&server);
    raw.send(&hello(server.token(), 1));
    assert_eq!(raw.reply().expect("a reply")["result"]["server"], "sonara");
}

#[test]
fn a_client_whose_session_ended_reports_it() {
    let fixture = Fixture::new();
    let options = AgentOptions {
        audit: AuditTarget::Disabled,
        ..fixture.options()
    };
    let server = fixture.server_for(fixture.agent(options));
    let mut raw = Raw::connect(&server);
    raw.send(&hello(server.token(), 1));
    raw.reply().expect("welcome");
    // A request too large for the protocol closes the connection.
    let filler = "a".repeat(sonara_agent::ipc::MAX_MESSAGE);
    raw.send_line(&filler);
    assert_eq!(
        raw.reply().expect("a reply"),
        json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32600, "message": "message too large"}})
    );
    assert!(raw.reply().is_none(), "closed");
    let error: ClientError = IpcClient::connect(
        sonara_agent::ipc::Discovery {
            address: fixture.root.join("missing.sock").display().to_string(),
            ..find_sessions(&fixture.runtime()).remove(0)
        },
        &ClientOptions::default(),
    )
    .expect_err("nothing listens there");
    assert_eq!(error.code(), ErrorCode::EngineUnavailable);
}

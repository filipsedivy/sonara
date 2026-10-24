//! The IPC server (`IPCServer` of `sonara.agent.ipc`): one accept thread, one thread per
//! connection, every connection served by the same [`AgentService`] (or a per-client view of it).

use std::fmt::Write as _;
use std::io::{self, BufReader};
use std::net::Shutdown;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use sonara_app::paths::{NoHomeDirectory, ensure_private_dir, runtime_dir};
use sonara_core::error::{DomainError, ErrorCode};

use super::discovery::{Announcement, Transport, discovery_path};
use super::pyvalue::{PyInt, dict_get, is_truthy, or_empty, py_int, py_repr, py_str};
use super::transport::{Listener, Stream, parse_line, read_line, write_message};
use super::{CONNECT_TIMEOUT, MAX_MESSAGE, PROTOCOL_VERSION, rpc};
use crate::registry::error_json;
use crate::service::AgentService;

/// How [`IpcServer::start`] runs the server (the prototype's keyword arguments).
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// What runs the session, as written into the discovery file (`app`): `gui`, `headless`, …
    pub app_kind: String,
    /// The folder of the discovery file and the socket (default:
    /// [`sonara_app::paths::runtime_dir`]). It is created with mode `0700`.
    pub directory: Option<PathBuf>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            app_kind: "headless".to_owned(),
            directory: None,
        }
    }
}

/// [`IpcServer::start`] could not start the server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// No directory was given and the runtime directory is unknown.
    #[error(transparent)]
    NoHome(#[from] NoHomeDirectory),
    /// The runtime directory cannot be created or made private.
    #[error("cannot create the runtime directory {}: {source}", path.display())]
    RuntimeDirectory {
        /// The directory.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The operating system's random source failed, so there is no session token.
    #[error("cannot draw the session token: {0}")]
    Token(#[source] getrandom::Error),
    /// The socket cannot be created, cleared or bound.
    #[error("cannot listen on {address}: {source}")]
    Listen {
        /// The socket path or TCP address.
        address: String,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The discovery file cannot be written.
    #[error("cannot write the discovery file {}: {source}", path.display())]
    Discovery {
        /// The discovery file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The accept thread cannot be started.
    #[error("cannot start the IPC thread: {0}")]
    Thread(#[source] io::Error),
}

/// What the accept thread and the connection threads share.
struct Shared {
    agent: Arc<AgentService>,
    token: String,
    /// Set by [`IpcServer::close`] (`Release`) and read by the accept thread after each accept
    /// (`Acquire`), so that the wake-up connection ends the loop.
    stopping: AtomicBool,
}

/// A running IPC server. It serves its [`AgentService`] until [`IpcServer::close`] (or until
/// it is dropped), and writes and removes the discovery file.
///
/// Closing stops accepting connections and removes the discovery file and the socket.
/// Connections already open keep being served until their client disconnects or the process
/// exits, as the prototype's daemon connection threads are.
pub struct IpcServer {
    transport: Transport,
    address: String,
    token: String,
    discovery_path: PathBuf,
    /// The socket file to remove on close (Unix domain sockets only).
    socket_path: Option<PathBuf>,
    shared: Arc<Shared>,
    accept_thread: Option<JoinHandle<()>>,
    closed: bool,
}

impl std::fmt::Debug for IpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcServer")
            .field("transport", &self.transport)
            .field("address", &self.address)
            .field("discovery_path", &self.discovery_path)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

impl IpcServer {
    /// Starts serving `agent`: creates the private directory, a fresh session token, the
    /// socket and the discovery file, then accepts connections on a background thread.
    ///
    /// # Errors
    ///
    /// [`ServerError`] when the directory, the token, the socket, the discovery file or the
    /// thread cannot be created. Nothing is left behind then.
    pub fn start(agent: Arc<AgentService>, options: ServerOptions) -> Result<Self, ServerError> {
        let directory = match options.directory {
            Some(directory) => directory,
            None => runtime_dir()?,
        };
        let directory =
            ensure_private_dir(&directory).map_err(|source| ServerError::RuntimeDirectory {
                path: directory.clone(),
                source,
            })?;
        let token = session_token()?;
        let (listener, transport, address, socket_path) = bind(&directory)?;
        let shared = Arc::new(Shared {
            agent,
            token: token.clone(),
            stopping: AtomicBool::new(false),
        });
        // From here on, dropping `server` removes what was created.
        let mut server = Self {
            transport,
            address,
            token,
            discovery_path: discovery_path(&directory),
            socket_path,
            shared,
            accept_thread: None,
            closed: false,
        };
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        let announcement = Announcement {
            transport,
            address: &server.address,
            token: &server.token,
            protocol: PROTOCOL_VERSION,
            app: &options.app_kind,
            started,
        };
        announcement
            .write(&server.discovery_path)
            .map_err(|source| ServerError::Discovery {
                path: server.discovery_path.clone(),
                source,
            })?;
        let shared = Arc::clone(&server.shared);
        let accept_thread = thread::Builder::new()
            .name("sonara-ipc".to_owned())
            .spawn(move || accept_loop(&listener, &shared))
            .map_err(ServerError::Thread)?;
        server.accept_thread = Some(accept_thread);
        log::info!("IPC server listening on {}", server.address);
        Ok(server)
    }

    /// The transport clients connect through.
    #[must_use]
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// The socket path, or `127.0.0.1:<port>` for TCP.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The discovery file, `session-<pid>.json` in the runtime directory.
    #[must_use]
    pub fn discovery_path(&self) -> &Path {
        &self.discovery_path
    }

    /// The session token that clients send in `hello` (also in the discovery file).
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Stops accepting connections and removes the discovery file and the socket. Connections
    /// already open keep being served (see [`IpcServer`]).
    pub fn close(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Some(accept_thread) = self.accept_thread.take() {
            // Pairs with the `Acquire` load in the accept loop.
            self.shared.stopping.store(true, Ordering::Release);
            // A blocking `accept` cannot be interrupted: connecting wakes it up, and the loop
            // then sees the flag. This happens before the socket file is removed.
            match Stream::connect(self.transport, &self.address, CONNECT_TIMEOUT) {
                Ok(_wake) => {
                    if accept_thread.join().is_err() {
                        log::warn!("the IPC accept thread panicked");
                    }
                }
                Err(error) => log::warn!(
                    "cannot wake the IPC accept thread ({error}); it ends with the process"
                ),
            }
        }
        // The prototype suppresses these errors; the files may already be gone.
        if let Err(error) = std::fs::remove_file(&self.discovery_path) {
            log::debug!("cannot remove {}: {error}", self.discovery_path.display());
        }
        if let Some(socket_path) = &self.socket_path
            && let Err(error) = std::fs::remove_file(socket_path)
        {
            log::debug!("cannot remove {}: {error}", socket_path.display());
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// 32 random bytes as 64 lowercase hexadecimal digits (`secrets.token_hex(32)`).
fn session_token() -> Result<String, ServerError> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(ServerError::Token)?;
    let mut token = String::with_capacity(64);
    for byte in bytes {
        // Writing into a `String` cannot fail.
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

/// The listening socket of this platform, its transport and address, and the socket file.
#[cfg(unix)]
fn bind(directory: &Path) -> Result<(Listener, Transport, String, Option<PathBuf>), ServerError> {
    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixListener;

    let listen_error = |address: &Path| {
        let address = address.display().to_string();
        move |source| ServerError::Listen { address, source }
    };
    let path = super::discovery::socket_path(directory).map_err(listen_error(directory))?;
    // The socket path travels as a JSON string.
    let address = path
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "the socket path is not valid UTF-8",
            )
        })
        .map_err(listen_error(&path))?
        .to_owned();
    match std::fs::remove_file(&path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(listen_error(&path)(error));
        }
        _ => {}
    }
    let listener = UnixListener::bind(&path).map_err(listen_error(&path))?;
    // Only the owner may connect. The prototype binds under `umask(0o177)`; changing the
    // process-wide umask would race with other threads creating files, so the mode is set
    // right after binding instead (the directory is private already).
    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
        drop(listener);
        let _ = std::fs::remove_file(&path);
        return Err(listen_error(&path)(error));
    }
    Ok((
        Listener::Unix(listener),
        Transport::Unix,
        address,
        Some(path),
    ))
}

/// The listening socket of this platform: TCP on an ephemeral port of `127.0.0.1`.
#[cfg(not(unix))]
fn bind(_directory: &Path) -> Result<(Listener, Transport, String, Option<PathBuf>), ServerError> {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|source| ServerError::Listen {
            address: "127.0.0.1:0".to_owned(),
            source,
        })?;
    let port = listener
        .local_addr()
        .map_err(|source| ServerError::Listen {
            address: "127.0.0.1:0".to_owned(),
            source,
        })?
        .port();
    Ok((
        Listener::Tcp(listener),
        Transport::Tcp,
        format!("127.0.0.1:{port}"),
        None,
    ))
}

/// How long the accept loop waits after a failed `accept` (for example, too many open files)
/// before it tries again.
const ACCEPT_RETRY: Duration = Duration::from_millis(50);

/// Accepts connections until [`IpcServer::close`] sets the flag; one thread per connection
/// (`ThreadingMixIn`).
fn accept_loop(listener: &Listener, shared: &Arc<Shared>) {
    loop {
        let accepted = listener.accept();
        // Pairs with the `Release` store in `IpcServer::shutdown`.
        if shared.stopping.load(Ordering::Acquire) {
            return;
        }
        match accepted {
            Ok(stream) => {
                let shared = Arc::clone(shared);
                let spawned = thread::Builder::new()
                    .name("sonara-ipc-client".to_owned())
                    .spawn(move || serve(stream, &shared));
                if let Err(error) = spawned {
                    log::warn!("cannot start an IPC connection thread: {error}");
                }
            }
            Err(error) => {
                // The prototype ignores failed accepts.
                log::debug!("IPC accept failed: {error}");
                thread::sleep(ACCEPT_RETRY);
            }
        }
    }
}

/// Serves one connection until the client disconnects, a reply closes it, or the socket
/// fails.
fn serve(stream: Stream, shared: &Shared) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut writer = stream;
    let mut session = Session::new(&shared.agent, &shared.token);
    loop {
        let line = match read_line(&mut reader, MAX_MESSAGE + 1) {
            Ok(line) if !line.is_empty() => line,
            _ => break,
        };
        let step = session.handle(&line);
        if let Some(reply) = &step.reply
            && let Err(error) = write_message(&mut writer, reply)
        {
            // The prototype suppresses send errors; the next read ends the connection.
            log::debug!("cannot send an IPC reply: {error}");
        }
        if step.close {
            break;
        }
    }
    // `shutdown_request`: end the write side before the socket closes.
    let _ = writer.shutdown(Shutdown::Write);
}

/// What one received line leads to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Step {
    /// The reply to send, if any.
    pub(crate) reply: Option<Value>,
    /// Whether the connection closes afterwards.
    pub(crate) close: bool,
}

impl Step {
    fn reply(reply: Value) -> Self {
        Self {
            reply: Some(reply),
            close: false,
        }
    }

    fn reply_and_close(reply: Value) -> Self {
        Self {
            reply: Some(reply),
            close: true,
        }
    }

    /// The connection closes without a reply: where the prototype's handler raised an
    /// unexpected exception, `socketserver` closed the connection.
    fn drop_connection() -> Self {
        Self {
            reply: None,
            close: true,
        }
    }
}

/// A request line that parsed: the prototype's `req["method"]`, `req.get("id")` and
/// `req.get("params") or {}` (`None` stands for `{}`).
struct Request {
    method: Value,
    id: Value,
    params: Option<Value>,
}

impl Request {
    /// `None` where the prototype answers "parse error": the line is no JSON, or no JSON
    /// object with a `method`.
    fn parse(line: &[u8]) -> Option<Self> {
        let Value::Object(mut request) = parse_line(line).ok()? else {
            return None;
        };
        let method = request.remove("method")?;
        let id = request.remove("id").unwrap_or(Value::Null);
        let params = request.remove("params").filter(is_truthy);
        Some(Self { method, id, params })
    }
}

/// One connection's protocol state: unauthenticated until a valid `hello`, then served by the
/// agent the `hello` chose (`_Handler.handle`).
pub(crate) struct Session<'a> {
    server_agent: &'a Arc<AgentService>,
    token: &'a str,
    agent: Option<Arc<AgentService>>,
}

impl<'a> Session<'a> {
    /// A new, unauthenticated connection to a server with this agent and token.
    pub(crate) fn new(server_agent: &'a Arc<AgentService>, token: &'a str) -> Self {
        Self {
            server_agent,
            token,
            agent: None,
        }
    }

    /// Handles one received line (with its newline, at most `MAX_MESSAGE + 1` bytes).
    pub(crate) fn handle(&mut self, line: &[u8]) -> Step {
        if line.len() > MAX_MESSAGE {
            return Step::reply_and_close(error_reply(
                Value::Null,
                rpc::INVALID_REQUEST,
                "message too large",
            ));
        }
        let Some(request) = Request::parse(line) else {
            return Step::reply(error_reply(Value::Null, rpc::PARSE_ERROR, "parse error"));
        };
        match &self.agent {
            None => self.hello(request),
            Some(agent) => Step::reply(dispatch(agent, request)),
        }
    }

    /// The first request must be `hello` with the session token and protocol version.
    fn hello(&mut self, request: Request) -> Step {
        let Request { method, id, params } = request;
        let refused = || {
            error_reply(
                id.clone(),
                rpc::AUTHENTICATION_FAILED,
                "authentication failed",
            )
        };
        if method.as_str() != Some("hello") {
            return Step::reply_and_close(refused());
        }
        // `str(params.get("token", ""))`; `params.get` raises unless `params` is a dict.
        let Ok(token) = dict_get(params.as_ref(), "token") else {
            return Step::drop_connection();
        };
        let token = token.map_or_else(String::new, py_str);
        // `secrets.compare_digest` raises `TypeError` for non-ASCII strings.
        if !token.is_ascii() {
            return Step::drop_connection();
        }
        if !constant_time_eq::constant_time_eq(token.as_bytes(), self.token.as_bytes()) {
            return Step::reply_and_close(refused());
        }
        let hello = match params {
            Some(Value::Object(hello)) => hello,
            // Falsy params became `{}`; others raised above.
            _ => Map::new(),
        };
        // `int(params.get("protocol", -1))`, which raises for values that are no integer.
        let protocol = match hello.get("protocol") {
            None => Some(PyInt::Small(-1)),
            Some(value) => py_int(value),
        };
        match protocol {
            None => return Step::drop_connection(),
            Some(PyInt::Small(version)) if version == PROTOCOL_VERSION => {}
            Some(_) => {
                return Step::reply_and_close(error_reply(
                    id,
                    rpc::PROTOCOL_MISMATCH,
                    &format!("protocol mismatch: server speaks {PROTOCOL_VERSION}"),
                ));
            }
        }
        let Some(agent) = self.client_agent(&hello) else {
            return Step::drop_connection();
        };
        let mut result = json!({"server": "sonara", "protocol": PROTOCOL_VERSION});
        if let (Value::Object(result), Value::Object(info)) = (&mut result, agent.info()) {
            result.extend(info);
        }
        self.agent = Some(agent);
        Step::reply(result_reply(id, result))
    }

    /// `_client_agent`: the server's own agent, or a view with the client's working directory
    /// and allowed folders (`context: {base_dir, allowed_paths}`). `None` where the prototype
    /// raises on a malformed context.
    fn client_agent(&self, hello: &Map<String, Value>) -> Option<Arc<AgentService>> {
        let context = match or_empty(hello.get("context")) {
            None => None,
            Some(Value::Object(context)) => Some(context),
            // `context.get` on a value that is not a dict.
            Some(_) => return None,
        };
        let base = context.and_then(|context| context.get("base_dir"));
        let allowed: Vec<PathBuf> =
            match or_empty(context.and_then(|context| context.get("allowed_paths"))) {
                None => Vec::new(),
                // Iterating keeps the strings: list items, a string's characters, a dict's keys.
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(py_path)
                    .collect(),
                Some(Value::String(text)) => text
                    .chars()
                    .map(|c| py_path(c.encode_utf8(&mut [0; 4])))
                    .collect(),
                Some(Value::Object(keys)) => keys.keys().map(|key| py_path(key)).collect(),
                // Numbers and `true` are not iterable.
                Some(_) => return None,
            };
        let client: String = hello
            .get("client")
            .map_or_else(|| "ipc".to_owned(), py_str)
            .chars()
            .take(64)
            .collect();
        if !base.is_some_and(is_truthy) && allowed.is_empty() {
            return Some(Arc::clone(self.server_agent));
        }
        let base = base.and_then(Value::as_str).map(py_path);
        Some(Arc::new(self.server_agent.for_client(
            base.as_deref(),
            &allowed,
            &client,
        )))
    }
}

/// `Path(text)`: the empty string is the current directory.
fn py_path(text: &str) -> PathBuf {
    if text.is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(text)
    }
}

/// `_Server.dispatch` for an authenticated connection.
fn dispatch(agent: &AgentService, request: Request) -> Value {
    let Request { method, id, params } = request;
    match method.as_str() {
        Some("ping") => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |elapsed| elapsed.as_secs_f64());
            result_reply(id, json!({"pong": now}))
        }
        Some("tools/list") => result_reply(id, json!({"tools": agent.tools()})),
        Some("tools/call") => {
            let lookup = dict_get(params.as_ref(), "name")
                .and_then(|name| Ok((name, dict_get(params.as_ref(), "arguments")?)));
            match lookup {
                Ok((name, arguments)) => {
                    let name = name.map_or_else(String::new, py_str);
                    result_reply(id, agent.call(&name, arguments))
                }
                Err(type_name) => {
                    // The prototype logs the `AttributeError` and answers with its text.
                    let message = format!("'{type_name}' object has no attribute 'get'");
                    log::error!("IPC request failed: {message}");
                    error_reply(id, rpc::INTERNAL_ERROR, &message)
                }
            }
        }
        Some("info") => result_reply(id, agent.info()),
        _ => {
            let error = DomainError::new(
                ErrorCode::NotFound,
                format!("unknown method {}", py_repr(&method)),
            );
            let data = error_json(&error);
            reply(
                id,
                "error",
                json!({"code": rpc::SONARA_ERROR, "message": error.message, "data": data}),
            )
        }
    }
}

/// `{"jsonrpc": "2.0", "id": id, "result": result}`.
fn result_reply(id: Value, result: Value) -> Value {
    reply(id, "result", result)
}

/// `{"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}`.
fn error_reply(id: Value, code: i32, message: &str) -> Value {
    reply(id, "error", json!({"code": code, "message": message}))
}

/// `{"jsonrpc": "2.0", "id": id, <key>: body}`, moving `id` and `body` in.
fn reply(id: Value, key: &str, body: Value) -> Value {
    let mut reply = Map::new();
    reply.insert("jsonrpc".to_owned(), Value::from("2.0"));
    reply.insert("id".to_owned(), id);
    reply.insert(key.to_owned(), body);
    Value::Object(reply)
}

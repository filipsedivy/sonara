//! The IPC client (`IPCClient` of `sonara.agent.ipc`): forwards tool calls to a running
//! session, as the MCP server's attach mode and the CLI use it.

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Map, Value, json};
use sonara_core::error::{DomainError, ErrorCode};

use super::discovery::{Discovery, find_sessions};
use super::pyvalue::{is_truthy, py_repr, py_str};
use super::transport::{Stream, parse_line, read_line, write_message};
use super::{CONNECT_TIMEOUT, DEFAULT_TIMEOUT, MAX_MESSAGE, PROTOCOL_VERSION, rpc};

/// Where the client works, sent in `hello` (`context`): relative paths in tool calls resolve
/// against `base_dir`, which is allowed together with `allowed_paths`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientContext {
    /// The client's working directory; `None` keeps the server's.
    pub base_dir: Option<PathBuf>,
    /// Extra folders the client may use.
    pub allowed_paths: Vec<PathBuf>,
}

impl ClientContext {
    /// `{"base_dir": str | null, "allowed_paths": [str]}`. Paths that are not valid UTF-8 are
    /// sent with replacement characters (the prototype sends surrogate escapes).
    fn to_json(&self) -> Value {
        let text = |path: &Path| Value::String(path.to_string_lossy().into_owned());
        json!({
            "base_dir": self.base_dir.as_deref().map(text),
            "allowed_paths": self.allowed_paths.iter().map(|path| text(path)).collect::<Vec<_>>(),
        })
    }
}

/// How [`IpcClient::connect`] connects (the prototype's keyword arguments).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientOptions {
    /// The client's name, sent in `hello` (the server keeps at most 64 characters).
    pub client: String,
    /// Where the client works; `None` sends no context, and the server serves the client with
    /// its own working directory and folders.
    pub context: Option<ClientContext>,
    /// How long a request may wait for its answer (`hello` and `ping` wait at most 5 s).
    pub timeout: Duration,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            client: "sonara-client".to_owned(),
            context: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

/// A request to a running session failed. [`ClientError::to_domain`] gives the prototype's
/// `SonaraError`: its code, message and details.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// The session's socket cannot be reached (`ENGINE_UNAVAILABLE`).
    #[error("cannot connect to Sonara at {address}: {}", os_error_text(source))]
    Connect {
        /// The session's address.
        address: String,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// No answer arrived in time (`TIMEOUT`).
    #[error("Sonara did not answer {method} in time")]
    Timeout {
        /// The request method, for example `tools/call`.
        method: String,
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// Sending or receiving failed (`ENGINE_UNAVAILABLE`).
    #[error("connection to Sonara lost: {}", os_error_text(source))]
    Lost {
        /// The cause.
        #[source]
        source: io::Error,
    },
    /// The session closed the connection (`ENGINE_UNAVAILABLE`).
    #[error("connection closed by Sonara")]
    Closed,
    /// The session answered with an error: a Sonara error with its own code, or a JSON-RPC
    /// error (`-32001` is `PERMISSION_DENIED`, `-32002` is `UNSUPPORTED`, anything else
    /// `INTERNAL`, with `details.rpc_code`).
    #[error("{}", .0.message)]
    Remote(DomainError),
    /// The answer is not a JSON-RPC response (`INTERNAL`). The prototype raises the parse or
    /// lookup error itself.
    #[error("invalid answer from Sonara: {reason}")]
    InvalidAnswer {
        /// What is wrong with it.
        reason: String,
    },
}

impl ClientError {
    /// The error code of the prototype's `SonaraError`.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Connect { .. } | Self::Lost { .. } | Self::Closed => ErrorCode::EngineUnavailable,
            Self::Timeout { .. } => ErrorCode::Timeout,
            Self::Remote(error) => error.code,
            Self::InvalidAnswer { .. } => ErrorCode::Internal,
        }
    }

    /// The prototype's `SonaraError`: code, message and details (only remote errors have
    /// details).
    #[must_use]
    pub fn to_domain(&self) -> DomainError {
        match self {
            Self::Remote(error) => error.clone(),
            other => DomainError::new(other.code(), other.to_string()),
        }
    }
}

/// Python's `str(OSError)`: `[Errno 2] No such file or directory` for an operating system
/// error, the plain text otherwise.
fn os_error_text(error: &io::Error) -> String {
    let text = error.to_string();
    match error.raw_os_error() {
        Some(code) => {
            let description = text
                .strip_suffix(&format!(" (os error {code})"))
                .unwrap_or(&text);
            format!("[Errno {code}] {description}")
        }
        None => text,
    }
}

/// The socket and the request counter, used by one request at a time.
struct Connection {
    reader: BufReader<Stream>,
    writer: Stream,
    last_id: u64,
    /// A read timed out. As with the prototype's socket file, every later read fails: an
    /// answer that arrives late could otherwise be taken for the next request's answer.
    timed_out: bool,
}

/// A connection to a running Sonara session that forwards tool calls to it. It can be shared
/// between threads; requests are sent one at a time.
pub struct IpcClient {
    discovery: Discovery,
    server_info: Value,
    timeout: Duration,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("discovery", &self.discovery)
            .field("server_info", &self.server_info)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl IpcClient {
    /// Connects to the session `discovery` describes and authenticates with its token
    /// (`IPCClient(discovery)`).
    ///
    /// # Errors
    ///
    /// - [`ClientError::Connect`] when the socket cannot be reached;
    /// - the errors of a request (see [`IpcClient::call`]) for the `hello` exchange, for
    ///   example [`ClientError::Remote`] with `PERMISSION_DENIED` for a wrong token or
    ///   `UNSUPPORTED` for another protocol version.
    pub fn connect(discovery: Discovery, options: &ClientOptions) -> Result<Self, ClientError> {
        let connect_error = |source| ClientError::Connect {
            address: discovery.address.clone(),
            source,
        };
        let writer = Stream::connect(discovery.transport, &discovery.address, CONNECT_TIMEOUT)
            .map_err(connect_error)?;
        let reader = BufReader::new(writer.try_clone().map_err(connect_error)?);
        let mut hello = json!({
            "token": discovery.token,
            "protocol": PROTOCOL_VERSION,
            "client": options.client,
        });
        if let (Value::Object(hello), Some(context)) = (&mut hello, &options.context) {
            hello.insert("context".to_owned(), context.to_json());
        }
        let mut client = Self {
            discovery,
            server_info: Value::Null,
            timeout: options.timeout,
            connection: Mutex::new(Connection {
                reader,
                writer,
                last_id: 0,
                timed_out: false,
            }),
        };
        client.server_info = client.request("hello", hello, CONNECT_TIMEOUT)?;
        Ok(client)
    }

    /// Connects to the newest running session with a discovery file in `directory` (usually
    /// [`sonara_app::paths::runtime_dir`]) that accepts the connection; `None` when there is
    /// none (`IPCClient.connect`). Sessions that refuse are skipped with a warning.
    #[must_use]
    pub fn connect_discovered(directory: &Path, options: &ClientOptions) -> Option<Self> {
        for discovery in find_sessions(directory) {
            let address = discovery.address.clone();
            match Self::connect(discovery, options) {
                Ok(client) => return Some(client),
                Err(error) => log::warn!("skipping session {address}: {}", error.to_domain()),
            }
        }
        None
    }

    /// The session this client is connected to.
    #[must_use]
    pub fn discovery(&self) -> &Discovery {
        &self.discovery
    }

    /// The server's answer to `hello`: `{server, protocol, api_version, project, tools}`.
    #[must_use]
    pub fn server_info(&self) -> &Value {
        &self.server_info
    }

    /// Calls the tool `name` in the session (`tools/call`) and returns its result envelope.
    /// Missing or falsy `arguments` are sent as `{}`.
    ///
    /// Tool failures are envelopes with `ok: false`, not errors.
    ///
    /// # Errors
    ///
    /// [`ClientError`] when the session cannot be asked: [`ClientError::Timeout`] after the
    /// client's timeout, [`ClientError::Lost`] or [`ClientError::Closed`] when the connection
    /// fails, [`ClientError::Remote`] when the session answers with a protocol error, and
    /// [`ClientError::InvalidAnswer`] for an answer that is no JSON-RPC response.
    pub fn call(&self, name: &str, arguments: Option<&Value>) -> Result<Value, ClientError> {
        let arguments = arguments
            .filter(|arguments| is_truthy(arguments))
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.request(
            "tools/call",
            json!({"name": name, "arguments": arguments}),
            self.timeout,
        )
    }

    /// The session's tools (`tools/list`), as the tool contract lists them.
    ///
    /// # Errors
    ///
    /// As [`IpcClient::call`]; [`ClientError::InvalidAnswer`] when the answer has no `tools`
    /// list.
    pub fn tools(&self) -> Result<Vec<Value>, ClientError> {
        match self.request("tools/list", json!({}), self.timeout)? {
            Value::Object(mut result) => match result.remove("tools") {
                Some(Value::Array(tools)) => Ok(tools),
                _ => Err(invalid("the tools/list result has no tools list")),
            },
            _ => Err(invalid("the tools/list result is not an object")),
        }
    }

    /// Whether the session answers `ping` (with a truthy result), within 5 s.
    ///
    /// # Errors
    ///
    /// As [`IpcClient::call`].
    pub fn ping(&self) -> Result<bool, ClientError> {
        Ok(is_truthy(&self.request(
            "ping",
            json!({}),
            CONNECT_TIMEOUT,
        )?))
    }

    /// Sends one request and returns its `result` (`null` when the answer has none).
    fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, ClientError> {
        let mut connection = self
            .connection
            .lock()
            // The connection holds no invariant that a panicking holder could break.
            .unwrap_or_else(PoisonError::into_inner);
        connection.last_id = connection.last_id.wrapping_add(1);
        let id = connection.last_id;
        let mut message = json!({"jsonrpc": "2.0", "id": id, "method": method});
        // Moved in last, so that the key order stays `jsonrpc, id, method, params`.
        message["params"] = params;
        let failed = |source: io::Error| match source.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => ClientError::Timeout {
                method: method.to_owned(),
                source,
            },
            _ => ClientError::Lost { source },
        };
        // macOS refuses `setsockopt` on a socket whose peer has closed (EINVAL), while the
        // prototype's `settimeout` makes no system call and its `sendall` then reports the
        // closed connection (EPIPE). So a failed timeout change yields to the write's error.
        let applied = connection.writer.set_timeout(Some(timeout));
        write_message(&mut connection.writer, &message).map_err(failed)?;
        if let Err(error) = applied {
            // The request went out without its timeout: its answer must not be taken for a
            // later request's, so the connection reads no more, as after a timeout.
            connection.timed_out = true;
            return Err(failed(error));
        }
        if connection.timed_out {
            return Err(ClientError::Lost {
                source: io::Error::other("cannot read from timed out object"),
            });
        }
        let line = match read_line(&mut connection.reader, MAX_MESSAGE + 1) {
            Ok(line) => line,
            Err(error) => {
                let error = failed(error);
                connection.timed_out = matches!(error, ClientError::Timeout { .. });
                return Err(error);
            }
        };
        if line.is_empty() {
            return Err(ClientError::Closed);
        }
        outcome(parse_line(&line).map_err(|error| invalid(&error.to_string()))?)
    }
}

/// The `result` of an answer, or its `error` as a [`ClientError`].
fn outcome(answer: Value) -> Result<Value, ClientError> {
    let Value::Object(mut answer) = answer else {
        return Err(invalid("the answer is not a JSON object"));
    };
    match answer.remove("error") {
        Some(Value::Object(error)) => Err(remote_error(&error)),
        Some(_) => Err(invalid("the error is not a JSON object")),
        None => Ok(answer.remove("result").unwrap_or(Value::Null)),
    }
}

/// The prototype's mapping of a JSON-RPC error object to a `SonaraError`.
#[expect(
    clippy::float_cmp,
    reason = "the prototype looks the code up in a dict, an exact comparison: -32001.0 finds -32001"
)]
fn remote_error(error: &Map<String, Value>) -> ClientError {
    if let Some(Value::Object(data)) = error.get("data")
        && let Some(code) = data.get("code")
    {
        // `ErrorCode(data["code"])`, which raises for unknown codes.
        let Some(code) = code.as_str().and_then(|code| {
            ErrorCode::ALL
                .into_iter()
                .find(|known| known.as_str() == code)
        }) else {
            return invalid(&format!("unknown error code {}", py_repr(code)));
        };
        let message = data
            .get("message")
            .or_else(|| error.get("message"))
            .map_or_else(String::new, py_str);
        // `**(data.get("details") or {})`, which raises for details that are not a dict.
        let details = match data.get("details").filter(|details| is_truthy(details)) {
            None => Map::new(),
            Some(Value::Object(details)) => details.clone(),
            Some(_) => return invalid("the error details are not a JSON object"),
        };
        return ClientError::Remote(DomainError {
            code,
            message,
            details,
        });
    }
    let rpc_code = error.get("code").cloned().unwrap_or(Value::Null);
    // A dict lookup in the prototype: `-32001.0` finds `-32001` too.
    let code = match rpc_code.as_f64() {
        Some(code) if code == f64::from(rpc::AUTHENTICATION_FAILED) => ErrorCode::PermissionDenied,
        Some(code) if code == f64::from(rpc::PROTOCOL_MISMATCH) => ErrorCode::Unsupported,
        _ => ErrorCode::Internal,
    };
    let message = error
        .get("message")
        .map_or_else(|| "IPC error".to_owned(), py_str);
    ClientError::Remote(DomainError::new(code, message).with_detail("rpc_code", rpc_code))
}

fn invalid(reason: &str) -> ClientError {
    ClientError::InvalidAnswer {
        reason: reason.to_owned(),
    }
}

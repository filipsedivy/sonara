//! Local IPC between a running Sonara (the GUI or `sonara serve`) and agent clients (the MCP
//! server's attach mode, the CLI), ported from the prototype's `sonara.agent.ipc`
//! (docs/agent-api.md, "IPC protocol"). The protocol is a
//! contract that does not change (CTR-01).
//!
//! - **Transport.** A Unix domain socket `sonara-<pid>.sock` in the private (`0700`) runtime
//!   directory on macOS and Linux, mode `0600`; when that path is longer than 100 bytes,
//!   `s-<pid>.sock` in a private `sonara-<uid>` folder of the temporary directory. TCP on an
//!   ephemeral port of `127.0.0.1` on Windows.
//! - **Discovery.** `session-<pid>.json` (mode `0600`) next to the socket holds
//!   `{pid, transport, address, token, protocol, app, started}`. Clients list the sessions with
//!   [`find_sessions`], which removes the files of dead sessions.
//! - **Framing.** Newline-delimited JSON-RPC 2.0, at most [`MAX_MESSAGE`] bytes per line
//!   including the newline, written as Python's `json.dumps` writes it (`", "` and `": "`
//!   separators, `\uXXXX` escapes for everything outside printable ASCII).
//! - **Handshake.** The first request must be `hello` with the session token and the protocol
//!   version: `{"token": "…", "protocol": 1}`, optionally `client` (a name) and `context`
//!   (`{base_dir, allowed_paths}`, see [`ClientContext`]). A wrong token gets `-32001`, another
//!   version `-32002`; either way the connection closes. The answer is
//!   `{server: "sonara", protocol, api_version, project, tools}`.
//! - **Methods.** `tools/list`, `tools/call {name, arguments}` (the tool's envelope), `info` and
//!   `ping`. Anything else is a Sonara error (`-32000`, `NOT_FOUND`, `unknown method …`, with
//!   `data: {code, message}`).
//! - **Other errors.** `-32700` `parse error` (the line is no JSON object with a `method`; the
//!   connection stays open), `-32600` `message too large` (the connection closes), `-32603`
//!   for a failure while handling a request.
//! - **Concurrency.** One session: every connection runs its tools on the same application,
//!   and edits commit one at a time through the session lock, each its own undo step. Each
//!   connection has its own thread; a client sends one request at a time.
//!
//! # Design decisions
//!
//! - **`std` sockets, not `interprocess`** (QG-13). The protocol's transports are a Unix domain
//!   socket and, on Windows, TCP on `127.0.0.1` (the discovery file says `unix` or `tcp`).
//!   `std::os::unix::net` and `std::net` are exactly those; `interprocess`'s local sockets use
//!   named pipes on Windows, which the protocol does not have.
//! - **Threads, not tokio.** The prototype's server is thread-based (`ThreadingMixIn`), and
//!   [`AgentService::call`](crate::service::AgentService::call) is synchronous and may block
//!   for minutes on jobs. The client is blocking too and can be shared between threads; the MCP
//!   server (tokio) calls it from a blocking task, as the prototype calls it from a worker
//!   thread (`anyio.to_thread`).
//! - **`rustix`** (already in the dependency tree through `tempfile` and `fs4`) provides
//!   `kill(pid, 0)` for the liveness check and `getuid` for the short socket folder, which
//!   `std` lacks; this crate forbids `unsafe` (RT-10).
//! - **`constant_time_eq`** compares the token in constant time, as `secrets.compare_digest`
//!   does.
//!
//! # Deviations from the prototype
//!
//! Each concerns input that no Sonara client sends, or a defect of the prototype:
//!
//! - **JSON the prototype's `json.loads` accepts but `serde_json` does not** is a parse error
//!   (`-32700`): `NaN`, `Infinity` and numbers that overflow to them, lone surrogates (as
//!   `\ud800` escapes or as surrogates encoded in UTF-8), UTF-16 and UTF-32 encoded lines, and
//!   nesting deeper than 128 levels. Integers beyond 64 bits are read as floats, so an `id` of
//!   `2**70` comes back as `1.1805916207174113e+21`, and the integer `-0` is read as the float
//!   `-0.0` (Python reads `0`).
//! - **Where the prototype's connection thread raised** (a `hello` whose `params` is no dict, a
//!   non-ASCII token, a `protocol` or `context` of the wrong type), the connection closes
//!   without a reply, as it did there. The `protocol` string `"١"` (a non-ASCII digit) is one
//!   of those here; the prototype read it as `1`.
//! - **The socket's mode** is set to `0600` right after binding instead of binding under
//!   `umask(0o177)`, which would change the mode of files other threads create meanwhile. The
//!   `0700` directory protects the socket in between.
//! - **The 100-byte socket path limit** counts bytes, the unit of `sun_path`; the prototype
//!   counted characters, so a long non-ASCII path failed to bind there.
//! - **Malformed discovery files** are skipped (and kept) where the prototype raised
//!   `TypeError` or `OverflowError`; a process id beyond 32 bits counts as dead.
//! - **The client** reports an unparsable answer, an unknown error code or a malformed error
//!   object as [`ClientError::InvalidAnswer`] (`INTERNAL`) where the prototype raised the
//!   parse or lookup error. Paths that are not valid UTF-8 are sent with replacement
//!   characters. Like the prototype's, a client whose request timed out fails every later
//!   request (`connection to Sonara lost: cannot read from timed out object`).

mod client;
mod discovery;
mod pyvalue;
mod server;
mod transport;

use std::time::Duration;

pub use client::{ClientContext, ClientError, ClientOptions, IpcClient};
pub use discovery::{Discovery, Transport, find_sessions};
pub use server::{IpcServer, ServerError, ServerOptions};

/// The protocol version this side speaks.
pub const PROTOCOL_VERSION: i64 = 1;

/// The longest message, in bytes, including its newline (32 MiB).
pub const MAX_MESSAGE: usize = 32 * 1024 * 1024;

/// How long connecting, `hello` and `ping` may take.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// How long other requests may take by default; long work runs as jobs.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3600);

/// The JSON-RPC error codes of the protocol.
mod rpc {
    /// A Sonara error; `data` holds its code, message and details.
    pub(super) const SONARA_ERROR: i32 = -32_000;
    /// The `hello` was missing or carried the wrong token.
    pub(super) const AUTHENTICATION_FAILED: i32 = -32_001;
    /// The client speaks another protocol version.
    pub(super) const PROTOCOL_MISMATCH: i32 = -32_002;
    /// The message is longer than [`super::MAX_MESSAGE`].
    pub(super) const INVALID_REQUEST: i32 = -32_600;
    /// Handling the request failed.
    pub(super) const INTERNAL_ERROR: i32 = -32_603;
    /// The line is no JSON object with a `method`.
    pub(super) const PARSE_ERROR: i32 = -32_700;
}

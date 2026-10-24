//! Discovery files: how a client finds the sessions running for this user
//! (`Discovery` and `find_sessions` of `sonara.agent.ipc`).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::pyvalue::{PyInt, dumps, py_float_value, py_int, py_str};

/// How a session is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// A Unix domain socket; the address is its path (macOS, Linux).
    Unix,
    /// TCP; the address is `127.0.0.1:<port>` (Windows).
    Tcp,
}

impl Transport {
    /// The transport of this platform: Unix domain sockets wherever they exist, except on
    /// Windows (the prototype's `USE_UNIX`).
    #[must_use]
    pub const fn native() -> Self {
        if cfg!(unix) { Self::Unix } else { Self::Tcp }
    }

    /// The name in the discovery file: `unix` or `tcp`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unix => "unix",
            Self::Tcp => "tcp",
        }
    }

    /// The transport a discovery file names. The prototype's client uses TCP for every name
    /// other than `unix`.
    fn from_name(name: &str) -> Self {
        if name == "unix" {
            Self::Unix
        } else {
            Self::Tcp
        }
    }
}

/// A running session, as its discovery file `session-<pid>.json` describes it.
#[derive(Clone, PartialEq)]
pub struct Discovery {
    /// The process that serves the session.
    pub pid: i64,
    /// How to reach it.
    pub transport: Transport,
    /// The socket path, or `host:port` for TCP.
    pub address: String,
    /// The session token the `hello` request must carry.
    pub token: String,
    /// The protocol version the server speaks.
    pub protocol: i64,
    /// What runs the session: `gui`, `headless`, or another name the server chose.
    pub app: String,
    /// When the server started, in seconds since the Unix epoch.
    pub started: f64,
    /// The discovery file.
    pub path: PathBuf,
}

impl fmt::Debug for Discovery {
    /// The token is a secret, so it is not printed.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Discovery")
            .field("pid", &self.pid)
            .field("transport", &self.transport)
            .field("address", &self.address)
            .field("token", &"<redacted>")
            .field("protocol", &self.protocol)
            .field("app", &self.app)
            .field("started", &self.started)
            .field("path", &self.path)
            .finish()
    }
}

impl Discovery {
    /// Reads a discovery file as `Discovery.load` does; `None` for a file that cannot be read
    /// or does not describe a session.
    ///
    /// The prototype skips files it cannot read, parse or that lack a key, and raises
    /// `TypeError` on some malformed values (a `null` pid, a list as the file); every one of
    /// those files is skipped here.
    fn load(path: &Path) -> Option<Self> {
        let text = fs::read_to_string(path).ok()?;
        let Value::Object(data) = serde_json::from_str::<Value>(&text).ok()? else {
            return None;
        };
        let small = |key: &str| match py_int(data.get(key)?)? {
            PyInt::Small(value) => Some(value),
            // No process id or protocol version is this large.
            PyInt::Large => Some(i64::MAX),
        };
        let string = |key: &str| data.get(key)?.as_str().map(str::to_owned);
        let started = match data.get("started") {
            Some(value) => py_float_value(value)?,
            None => 0.0,
        };
        Some(Self {
            pid: small("pid")?,
            transport: Transport::from_name(data.get("transport")?.as_str().unwrap_or_default()),
            address: string("address")?,
            token: string("token")?,
            protocol: small("protocol")?,
            app: data.get("app").map_or_else(|| "?".to_owned(), py_str),
            started,
            path: path.to_path_buf(),
        })
    }

    /// Whether the session can still be running: its process is alive and, for a Unix domain
    /// socket, the socket file exists.
    fn is_live(&self) -> bool {
        pid_alive(self.pid)
            && (self.transport != Transport::Unix || Path::new(&self.address).exists())
    }
}

/// The sessions with a discovery file in `directory` (the runtime directory by default, see
/// [`sonara_app::paths::runtime_dir`]), newest first. Discovery files of dead sessions (the
/// process has exited, or the socket file is gone) are removed; files that cannot be read are
/// skipped and kept.
#[must_use]
pub fn find_sessions(directory: &Path) -> Vec<Discovery> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        // `glob("session-*.json")`, case-sensitive as on POSIX.
        let path = entry.path();
        let is_discovery = path
            .extension()
            .is_some_and(|extension| extension == "json")
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with("session-"));
        if !is_discovery {
            continue;
        }
        let Some(discovery) = Discovery::load(&path) else {
            continue;
        };
        if discovery.is_live() {
            sessions.push(discovery);
        } else if let Err(error) = fs::remove_file(&path) {
            // The prototype suppresses the error: another client may have removed it first.
            log::debug!(
                "cannot remove the stale discovery file {}: {error}",
                path.display()
            );
        }
    }
    // A stable sort, as Python's `sorted(..., reverse=True)`: equal times keep their order.
    sessions.sort_by(|a, b| b.started.total_cmp(&a.started));
    sessions
}

/// `_pid_alive`: whether a process with this id exists (`kill(pid, 0)` succeeds or is not
/// permitted).
#[cfg(unix)]
fn pid_alive(pid: i64) -> bool {
    use rustix::io::Errno;
    use rustix::process::{Pid, test_kill_process};

    // Process ids are positive 32-bit values; for larger ones the prototype's `os.kill` raises
    // `OverflowError`, and no process can have them.
    let Some(pid) = i32::try_from(pid)
        .ok()
        .filter(|raw| *raw > 0)
        .and_then(Pid::from_raw)
    else {
        return false;
    };
    !matches!(test_kill_process(pid), Err(Errno::SRCH))
}

/// `_pid_alive` on Windows: every positive id counts as alive, as in the prototype.
#[cfg(not(unix))]
fn pid_alive(pid: i64) -> bool {
    pid > 0
}

/// The discovery file of this process in `directory`.
pub(crate) fn discovery_path(directory: &Path) -> PathBuf {
    directory.join(format!("session-{}.json", std::process::id()))
}

/// The longest socket path the prototype puts in the runtime directory; `sun_path` holds 104
/// bytes on macOS and 108 on Linux.
#[cfg(unix)]
const SOCKET_PATH_LIMIT: usize = 100;

/// `_socket_path`: `sonara-<pid>.sock` in `directory`, or `s-<pid>.sock` in a private
/// `sonara-<uid>` folder of the temporary directory when that path would be too long for a
/// socket address.
///
/// # Errors
///
/// The I/O error of creating the short folder.
#[cfg(unix)]
pub(crate) fn socket_path(directory: &Path) -> io::Result<PathBuf> {
    let pid = std::process::id();
    let path = directory.join(format!("sonara-{pid}.sock"));
    // The limit is in bytes, the unit of `sun_path` (the prototype counts characters).
    if path.as_os_str().len() <= SOCKET_PATH_LIMIT {
        return Ok(path);
    }
    let uid = rustix::process::getuid().as_raw();
    let short = std::env::temp_dir().join(format!("sonara-{uid}"));
    sonara_app::paths::ensure_private_dir(&short)?;
    Ok(short.join(format!("s-{pid}.sock")))
}

/// What the server writes into its discovery file, in the prototype's key order.
pub(crate) struct Announcement<'a> {
    pub(crate) transport: Transport,
    pub(crate) address: &'a str,
    pub(crate) token: &'a str,
    pub(crate) protocol: i64,
    pub(crate) app: &'a str,
    pub(crate) started: f64,
}

impl Announcement<'_> {
    /// Writes the discovery file `path` with `json.dump`'s layout, created with mode `0600`
    /// (an existing file keeps its mode, as with the prototype's `os.open`).
    ///
    /// # Errors
    ///
    /// The I/O error of creating or writing the file.
    pub(crate) fn write(&self, path: &Path) -> io::Result<()> {
        let payload = json!({
            "pid": std::process::id(),
            "transport": self.transport.as_str(),
            "address": self.address,
            "token": self.token,
            "protocol": self.protocol,
            "app": self.app,
            "started": self.started,
        });
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options.open(path)?;
        io::Write::write_all(&mut file, &dumps(&payload))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{Discovery, Transport, find_sessions};

    #[test]
    fn load_follows_the_prototype_coercions() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("session-1.json");
        fs::write(
            &path,
            r#"{"pid": "42", "transport": "tcp", "address": "127.0.0.1:9", "token": "t", "protocol": 1.0, "started": "2.5"}"#,
        )
        .expect("written");
        let loaded = Discovery::load(&path).expect("a session");
        assert_eq!(loaded.pid, 42);
        assert_eq!(loaded.transport, Transport::Tcp);
        assert_eq!(loaded.protocol, 1);
        assert_eq!(loaded.app, "?");
        assert!((loaded.started - 2.5).abs() < f64::EPSILON);
        assert!(
            !format!("{loaded:?}").contains("\"t\""),
            "the token is not printed"
        );
        for broken in [
            "[]",
            "{not json",
            r#"{"pid": null, "transport": "unix", "address": "/x", "token": "t", "protocol": 1}"#,
            r#"{"pid": 1, "transport": "unix", "address": "/x", "protocol": 1}"#,
            r#"{"pid": 1, "transport": "unix", "address": "/x", "token": "t", "protocol": "one"}"#,
            r#"{"pid": 1, "transport": "unix", "address": "/x", "token": "t", "protocol": 1, "started": null}"#,
        ] {
            fs::write(&path, broken).expect("written");
            assert!(Discovery::load(&path).is_none(), "{broken}");
        }
    }

    #[test]
    fn missing_directory_has_no_sessions() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        assert_eq!(find_sessions(&dir.path().join("absent")), []);
    }

    #[cfg(unix)]
    #[test]
    fn this_process_is_alive_and_impossible_ids_are_not() {
        let pid = i64::from(std::process::id());
        assert!(super::pid_alive(pid));
        for dead in [0, -1, i64::from(i32::MAX) + 1, i64::MAX] {
            assert!(!super::pid_alive(dead), "{dead}");
        }
    }
}

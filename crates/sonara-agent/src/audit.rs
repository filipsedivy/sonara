//! The append-only audit log of agent tool calls (`sonara.agent.audit`): one JSON object per
//! line, with token-like arguments redacted.
//!
//! Each entry has these fields, in this order:
//!
//! | Field | Value |
//! |---|---|
//! | `ts` | the time of the call, `YYYY-MM-DDTHH:MM:SS+0000` |
//! | `tool` | the tool name |
//! | `source` | the client (`agent`, or the IPC client's name) |
//! | `ok` | whether the call succeeded |
//! | `error` | the error code, or `null` |
//! | `ms` | the duration in milliseconds, rounded to one decimal like Python's `round(x, 1)` |
//! | `project_version` | the session version, or `null` without a project |
//! | `args` | the raw arguments, redacted |
//!
//! Redaction: the value of every object key that contains `token`, `secret`, `password`,
//! `api_key` or `authorization` (in any case) becomes `"<redacted>"`; a list longer than 50
//! items keeps the first 50 and ends with `"... <n> more"`; a string longer than 2000
//! characters keeps the first 2000 and ends with `"..."`.
//!
//! The log lives at `<SONARA_HOME>/logs/agent-audit.jsonl` ([`AuditLog::default_path`]). Before
//! an entry is appended to a file larger than [`MAX_BYTES`], the file is renamed to
//! `agent-audit.1.jsonl`, replacing an older one.
//!
//! Two differences from the prototype: `ts` is in UTC (the prototype writes local time with
//! its offset), and lines are written by `serde_json` (compact, UTF-8) rather than
//! `json.dumps` (`", "` separators, non-ASCII escaped); both are the same JSON values.

use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::tz::Offset;
use serde_json::{Map, Value};
use sonara_core::error::ErrorCode;

/// The file name of the audit log in the logs directory.
pub const FILE_NAME: &str = "agent-audit.jsonl";

/// The size above which the log is rotated before the next entry.
pub const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Key fragments whose values are redacted.
const REDACT: [&str; 5] = ["token", "secret", "password", "api_key", "authorization"];

/// Items of a list that are kept.
const MAX_ITEMS: usize = 50;

/// Characters of a string that are kept.
const MAX_CHARS: usize = 2000;

/// The wall clock that stamps entries ([`SystemTime::now`] normally).
pub type WallClock = Box<dyn Fn() -> SystemTime + Send + Sync>;

/// A failed audit log operation. The registry decides what to do with it; the prototype logs a
/// warning and carries on.
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    /// Appending an entry (or rotating the file) failed.
    #[error("cannot write audit log {}: {source}", .path.display())]
    Write {
        /// The log file.
        path: PathBuf,
        /// The file system error.
        #[source]
        source: io::Error,
    },
    /// Reading the log failed.
    #[error("cannot read audit log {}: {source}", .path.display())]
    Read {
        /// The log file.
        path: PathBuf,
        /// The file system error.
        #[source]
        source: io::Error,
    },
}

/// One tool call to record.
#[derive(Debug, Clone, Copy)]
pub struct AuditRecord<'a> {
    /// The tool name, as called.
    pub tool: &'a str,
    /// The arguments as received (before validation); redacted when written.
    pub args: &'a Value,
    /// The error code of a failed call; `None` for a call that succeeded.
    pub error: Option<ErrorCode>,
    /// How long the call took.
    pub duration: Duration,
    /// The session version after the call, if a project is open.
    pub project_version: Option<u64>,
    /// The client that made the call.
    pub source: &'a str,
}

/// The audit log. It is shared by every client of a session, so appends are serialized
/// internally, as in the prototype.
pub struct AuditLog {
    path: Option<PathBuf>,
    clock: WallClock,
    write: Mutex<()>,
}

impl std::fmt::Debug for AuditLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditLog")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl AuditLog {
    /// A log written to `path`; `None` disables it.
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self::with_clock(path, Box::new(SystemTime::now))
    }

    /// A log whose entries are stamped by `clock` (golden runs and tests).
    #[must_use]
    pub fn with_clock(path: Option<PathBuf>, clock: WallClock) -> Self {
        Self {
            path,
            clock,
            write: Mutex::new(()),
        }
    }

    /// The default location in the application's logs directory (`<SONARA_HOME>/logs`).
    #[must_use]
    pub fn default_path(logs_dir: &Path) -> PathBuf {
        logs_dir.join(FILE_NAME)
    }

    /// The log file, if logging is enabled.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Appends one entry, creating the directory and rotating the file as needed. Does
    /// nothing when the log is disabled.
    ///
    /// # Errors
    ///
    /// [`AuditError::Write`] when the directory cannot be created or the file cannot be
    /// rotated or appended to.
    pub fn record(&self, record: &AuditRecord<'_>) -> Result<(), AuditError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut entry = Map::new();
        entry.insert("ts".to_owned(), Value::from(timestamp((self.clock)())));
        entry.insert("tool".to_owned(), Value::from(record.tool));
        entry.insert("source".to_owned(), Value::from(record.source));
        entry.insert("ok".to_owned(), Value::from(record.error.is_none()));
        entry.insert(
            "error".to_owned(),
            record
                .error
                .map_or(Value::Null, |code| Value::from(code.as_str())),
        );
        entry.insert("ms".to_owned(), Value::from(milliseconds(record.duration)));
        entry.insert(
            "project_version".to_owned(),
            record.project_version.map_or(Value::Null, Value::from),
        );
        entry.insert("args".to_owned(), redact(record.args));
        let mut line = Value::Object(entry).to_string();
        line.push('\n');
        // Appends stay whole if a holder panicked: the guard protects no data.
        let _guard = self.write.lock().unwrap_or_else(PoisonError::into_inner);
        append(path, &line).map_err(|source| AuditError::Write {
            path: path.clone(),
            source,
        })
    }

    /// The last `n` entries, oldest first; lines that are not JSON are skipped. Empty when the
    /// log is disabled or has no file yet. (The prototype's slice `lines[-n:]` returns every
    /// line for `n = 0`; here `n = 0` returns none. Its only caller asks for 20.)
    ///
    /// # Errors
    ///
    /// [`AuditError::Read`] when the file exists but cannot be read as UTF-8 text.
    pub fn tail(&self, n: usize) -> Result<Vec<Value>, AuditError> {
        let Some(path) = &self.path else {
            return Ok(Vec::new());
        };
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(AuditError::Read {
                    path: path.clone(),
                    source,
                });
            }
        };
        let lines: Vec<&str> = text.lines().collect();
        let skip = lines.len().saturating_sub(n);
        Ok(lines
            .into_iter()
            .skip(skip)
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect())
    }
}

/// Creates the directory, rotates an oversized file and appends `line`.
fn append(path: &Path, line: &str) -> io::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    if fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_BYTES) {
        // `Path.with_suffix(".1.jsonl")`: `agent-audit.jsonl` becomes `agent-audit.1.jsonl`.
        fs::rename(path, path.with_extension("1.jsonl"))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(line.as_bytes())
}

/// `time` as `YYYY-MM-DDTHH:MM:SS+0000` in UTC. Times beyond jiff's range (years ±9999) are
/// clamped.
fn timestamp(time: SystemTime) -> String {
    let instant = Timestamp::try_from(time).unwrap_or(if time < UNIX_EPOCH {
        Timestamp::MIN
    } else {
        Timestamp::MAX
    });
    let t = Offset::UTC.to_datetime(instant);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}+0000",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// `round(duration * 1000, 1)`: Rust's fixed-precision formatting rounds the exact binary value
/// to the nearest decimal, ties to even, as Python's `round` does.
fn milliseconds(duration: Duration) -> f64 {
    let ms = duration.as_secs_f64() * 1000.0;
    format!("{ms:.1}").parse().unwrap_or(ms)
}

/// The prototype's `_redact` (see the module documentation).
fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let lower = key.to_lowercase();
                    let kept = if REDACT.iter().any(|fragment| lower.contains(fragment)) {
                        Value::from("<redacted>")
                    } else {
                        redact(value)
                    };
                    (key.clone(), kept)
                })
                .collect(),
        ),
        Value::Array(items) if items.len() > MAX_ITEMS => {
            let mut kept: Vec<Value> = items.iter().take(MAX_ITEMS).map(redact).collect();
            kept.push(Value::from(format!("... {} more", items.len() - MAX_ITEMS)));
            Value::Array(kept)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        Value::String(text) => match text.char_indices().nth(MAX_CHARS) {
            Some((cut, _)) => Value::from(format!("{}...", &text[..cut])),
            None => value.clone(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use super::{milliseconds, timestamp};

    #[test]
    fn milliseconds_round_like_python() {
        // round(x, 1) in CPython 3.13.
        let cases = [
            (250_000, 0.2),
            (350_000, 0.3),
            (450_000, 0.5),
            (3_050_000, 3.1),
            (12_345_678, 12.3),
            (999_999_999, 1000.0),
            (0, 0.0),
        ];
        for (nanos, expected) in cases {
            assert_eq!(
                milliseconds(Duration::from_nanos(nanos)),
                expected,
                "{nanos} ns"
            );
        }
    }

    #[test]
    fn timestamps_are_utc_seconds() {
        // 2026-01-01T12:00:00Z (golden_common.FIXED_NOW).
        let noon = UNIX_EPOCH + Duration::from_hours(490_908);
        assert_eq!(timestamp(noon), "2026-01-01T12:00:00+0000");
        assert_eq!(
            timestamp(noon + Duration::from_millis(999)),
            "2026-01-01T12:00:00+0000"
        );
        assert_eq!(
            timestamp(SystemTime::UNIX_EPOCH),
            "1970-01-01T00:00:00+0000"
        );
    }
}

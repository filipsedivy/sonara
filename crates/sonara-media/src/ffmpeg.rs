//! The `FFmpeg` import fallback (`audio_import.py`'s
//! `ffmpeg_path`, `_inspect_ffprobe` and `_ffmpeg_to_flac`).
//!
//! A file this crate cannot read is inspected with `ffprobe` and transcoded to FLAC with
//! `ffmpeg` when `ffmpeg` is installed. Both run as external processes: `FFmpeg` is never linked
//! (LIC-01). Discovery is the prototype's: `ffmpeg` and `ffprobe` on `PATH`, as
//! `shutil.which` finds them ([`which`]). The `ffmpeg_path` setting is not consulted, as in the
//! prototype, which stores it but never reads it.
//!
//! The command lines, results and error texts are the prototype's. Where the prototype crashed
//! (a timeout, a tool that cannot start, output that is not JSON), the error is `INTERNAL`, the
//! code its agent boundary gave a crash, with the crash's text where it can be reproduced (the
//! `TimeoutExpired` and `OSError` texts). Two deviations: the tools get an empty standard input,
//! where the prototype passed its own on (under `sonara mcp` that is the protocol stream), and a
//! transcode that times out leaves no partial file behind.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::Value;
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::pyfmt::{str_list_repr, str_repr};
use wait_timeout::ChildExt as _;

use crate::inspect::AudioFileInfo;

/// `ffprobe`'s limit (`timeout=60`).
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// `ffmpeg`'s limit (`timeout=3600`).
const TRANSCODE_TIMEOUT: Duration = Duration::from_secs(3600);
/// The arguments of `ffprobe` before the file.
const PROBE_ARGS: [&str; 8] = [
    "-v",
    "error",
    "-select_streams",
    "a:0",
    "-show_entries",
    "stream=sample_rate,channels,codec_name,duration_ts,time_base,duration:format_tags",
    "-of",
    "json",
];
/// The characters of `ffmpeg`'s error output an import error keeps (`stderr.strip()[:300]`).
const STDERR_CHARS: usize = 300;

/// The search path `shutil.which` uses when `PATH` is not set (`os.confstr("CS_PATH")`).
#[cfg(target_os = "macos")]
const DEFAULT_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";
#[cfg(all(unix, not(target_os = "macos")))]
const DEFAULT_PATH: &str = "/bin:/usr/bin";
#[cfg(not(unix))]
const DEFAULT_PATH: &str = "";

/// `shutil.which(name, path=search)`: the first directory entry of `search` (the value of
/// `PATH`; `None` when it is not set, which searches the system's default path) that names an
/// executable file. An empty `PATH` finds nothing; an empty entry is the working directory;
/// each directory is searched once. On Windows, the extensions of `PATHEXT` are tried too.
#[must_use]
pub fn which(name: &str, search: Option<&OsStr>) -> Option<PathBuf> {
    let search: OsString = search.map_or_else(|| OsString::from(DEFAULT_PATH), OsStr::to_owned);
    if search.is_empty() {
        return None;
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    for dir in std::env::split_paths(&search) {
        if seen.contains(&dir) {
            continue;
        }
        seen.push(dir.clone());
        for candidate in candidates(&dir, name) {
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    vec![dir.join(name)]
}

#[cfg(not(unix))]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned());
    std::iter::once(dir.join(name))
        .chain(
            extensions
                .split(';')
                .filter(|ext| !ext.is_empty())
                .map(|ext| dir.join(format!("{name}{ext}"))),
        )
        .collect()
}

/// `os.path.exists(path) and os.access(path, X_OK) and not os.path.isdir(path)`.
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    path.exists() && !path.is_dir() && rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_ok()
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// The `FFmpeg` tools found for the import fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ffmpeg {
    executable: PathBuf,
    ffprobe: Option<PathBuf>,
    probe_timeout: Duration,
    transcode_timeout: Duration,
}

/// The result of running one tool.
struct Run {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl Ffmpeg {
    /// `ffmpeg_path()`: `ffmpeg` on the process's `PATH`, with `ffprobe` from the same search;
    /// `None` when `ffmpeg` is not installed.
    #[must_use]
    pub fn discover() -> Option<Self> {
        Self::discover_in(std::env::var_os("PATH").as_deref())
    }

    /// [`Ffmpeg::discover`] with `search` as the value of `PATH` (`None`: not set).
    #[must_use]
    pub fn discover_in(search: Option<&OsStr>) -> Option<Self> {
        let ffmpeg = which("ffmpeg", search)?;
        Some(Self::new(ffmpeg, which("ffprobe", search)))
    }

    /// The tools at the given paths (`ffprobe` may be missing).
    #[must_use]
    pub fn new(ffmpeg: PathBuf, ffprobe: Option<PathBuf>) -> Self {
        Self {
            executable: ffmpeg,
            ffprobe,
            probe_timeout: PROBE_TIMEOUT,
            transcode_timeout: TRANSCODE_TIMEOUT,
        }
    }

    /// The same tools with other time limits (tests).
    #[must_use]
    pub fn with_timeouts(mut self, probe: Duration, transcode: Duration) -> Self {
        self.probe_timeout = probe;
        self.transcode_timeout = transcode;
        self
    }

    /// The `ffmpeg` executable.
    #[must_use]
    pub fn ffmpeg(&self) -> &Path {
        &self.executable
    }

    /// The `ffprobe` executable, if one was found.
    #[must_use]
    pub fn ffprobe(&self) -> Option<&Path> {
        self.ffprobe.as_deref()
    }

    /// `_inspect_ffprobe(path, size)`: the inspection of a file this crate cannot read. The
    /// format is the codec's name in capitals, the subtype `FFMPEG`, the frames the duration
    /// times the rate (rounded half to even), and the metadata every container tag with its
    /// name in lower case; the file always counts as lossy, without a bit depth.
    ///
    /// # Errors
    ///
    /// `UNSUPPORTED` when `ffprobe` is missing (`"{name}: cannot inspect (ffprobe missing)"`),
    /// fails (`"{name}: not a decodable audio file"`) or finds no audio stream
    /// (`"{name}: no audio stream"`); `INTERNAL` where the prototype crashed: a timeout, a tool
    /// that cannot start, and output that is not the JSON `ffprobe` writes.
    pub fn inspect(&self, path: &Path, size_bytes: u64) -> Result<AudioFileInfo, DomainError> {
        let name = file_name(path);
        let Some(ffprobe) = &self.ffprobe else {
            return Err(DomainError::new(
                ErrorCode::Unsupported,
                format!("{name}: cannot inspect (ffprobe missing)"),
            ));
        };
        let mut args: Vec<OsString> = PROBE_ARGS.iter().map(OsString::from).collect();
        args.push(path.as_os_str().to_owned());
        let run = run(ffprobe, &args, self.probe_timeout)?;
        if !run.status.success() {
            return Err(DomainError::new(
                ErrorCode::Unsupported,
                format!("{name}: not a decodable audio file"),
            ));
        }
        let text = if run.stdout.is_empty() {
            "{}"
        } else {
            &run.stdout
        };
        let data: Value =
            serde_json::from_str(text).map_err(|e| crash(format!("JSONDecodeError: {e}")))?;
        let object = data
            .as_object()
            .ok_or_else(|| crash("AttributeError: ffprobe's output is not an object".to_owned()))?;
        let streams = object
            .get("streams")
            .and_then(Value::as_array)
            .filter(|streams| !streams.is_empty())
            .ok_or_else(|| {
                DomainError::new(ErrorCode::Unsupported, format!("{name}: no audio stream"))
            })?;
        let stream = streams
            .first()
            .and_then(Value::as_object)
            .ok_or_else(|| crash("AttributeError: ffprobe's stream is not an object".to_owned()))?;
        let sample_rate = py_int(stream.get("sample_rate"))?;
        let channels = py_int(stream.get("channels"))?;
        let duration = py_float(stream.get("duration"))?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "Python's float(duration) * int(rate), in f64 arithmetic"
        )]
        let product = duration * sample_rate as f64;
        if !product.is_finite() {
            return Err(crash(format!("ValueError: cannot round {product}")));
        }
        let codec = match stream.get("codec_name") {
            None => "unknown".to_owned(),
            Some(Value::String(codec)) => codec.clone(),
            Some(_) => {
                return Err(crash(
                    "AttributeError: codec_name is not a string".to_owned(),
                ));
            }
        };
        let mut metadata: Vec<(String, String)> = Vec::new();
        let tags = object
            .get("format")
            .and_then(Value::as_object)
            .and_then(|format| format.get("tags"))
            .and_then(Value::as_object);
        for (key, value) in tags.into_iter().flatten() {
            let key = key.to_lowercase();
            let value = py_str(value);
            match metadata.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => metadata.push((key, value)),
            }
        }
        Ok(AudioFileInfo {
            path: path.display().to_string(),
            format: codec.to_uppercase(),
            subtype: "FFMPEG".to_owned(),
            sample_rate: u32::try_from(sample_rate).unwrap_or(0),
            channels: u32::try_from(channels).unwrap_or(0),
            frames: rounded_frames(product),
            size_bytes,
            bit_depth: None,
            lossy: true,
            decoder: "ffmpeg".to_owned(),
            metadata,
        })
    }

    /// `_ffmpeg_to_flac(source, dest)`: transcodes `source` losslessly into the FLAC file
    /// `dest` (32-bit samples), through `dest` with the suffix `.tmp.flac`, which is renamed into
    /// place once `ffmpeg` succeeds. The folders of `dest` are created.
    ///
    /// # Errors
    ///
    /// `IO_ERROR` when `ffmpeg` fails: `"FFmpeg could not decode {name}: {stderr}"`, with the
    /// first 300 characters of its trimmed error output (the partial file is removed);
    /// `INTERNAL` where the prototype crashed: a timeout, a tool that cannot start, and a folder
    /// or rename that fails.
    pub fn transcode_to_flac(&self, source: &Path, dest: &Path) -> Result<(), DomainError> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| os_crash(&e, parent))?;
        }
        let tmp = dest.with_extension("tmp.flac");
        let args: Vec<OsString> = [
            OsStr::new("-nostdin"),
            OsStr::new("-v"),
            OsStr::new("error"),
            OsStr::new("-y"),
            OsStr::new("-i"),
            source.as_os_str(),
            OsStr::new("-vn"),
            OsStr::new("-c:a"),
            OsStr::new("flac"),
            OsStr::new("-sample_fmt"),
            OsStr::new("s32"),
            tmp.as_os_str(),
        ]
        .into_iter()
        .map(OsStr::to_owned)
        .collect();
        let run = match run(&self.executable, &args, self.transcode_timeout) {
            Ok(run) => run,
            Err(error) => {
                remove_quietly(&tmp);
                return Err(error);
            }
        };
        if !run.status.success() {
            remove_quietly(&tmp);
            let stderr: String = run.stderr.trim().chars().take(STDERR_CHARS).collect();
            return Err(DomainError::new(
                ErrorCode::IoError,
                format!("FFmpeg could not decode {}: {stderr}", file_name(source)),
            ));
        }
        std::fs::rename(&tmp, dest).map_err(|e| os_crash(&e, &tmp))
    }
}

/// Removes a partial file; a file that is already gone is fine (`unlink(missing_ok=True)`).
fn remove_quietly(path: &Path) {
    if let Err(error) = std::fs::remove_file(path)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::warn!("cannot remove {}: {error}", path.display());
    }
}

/// Python's `Path.name`.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// An `INTERNAL` error: what the prototype's agent boundary reported for a crash.
fn crash(message: String) -> DomainError {
    DomainError::new(ErrorCode::Internal, message)
}

/// The crash of an `OSError` on `path`, in Python's words
/// (`FileNotFoundError: [Errno 2] No such file or directory: '/x'`).
fn os_crash(error: &io::Error, path: &Path) -> DomainError {
    let class = match error.kind() {
        io::ErrorKind::NotFound => "FileNotFoundError",
        io::ErrorKind::PermissionDenied => "PermissionError",
        io::ErrorKind::AlreadyExists => "FileExistsError",
        io::ErrorKind::IsADirectory => "IsADirectoryError",
        io::ErrorKind::NotADirectory => "NotADirectoryError",
        _ => "OSError",
    };
    let text = error.to_string();
    let reason = text.split(" (os error").next().unwrap_or(&text);
    let path = str_repr(&path.display().to_string());
    crash(match error.raw_os_error() {
        Some(code) => format!("{class}: [Errno {code}] {reason}: {path}"),
        None => format!("{class}: {reason}: {path}"),
    })
}

/// Python's `round(x)` of a finite product, as a frame count (negative counts are 0).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "the rounded product is integral, and clamped to the u64 range"
)]
fn rounded_frames(product: f64) -> u64 {
    let rounded = product.round_ties_even();
    if rounded <= 0.0 {
        0
    } else {
        rounded.min(u64::MAX as f64) as u64
    }
}

/// Python's `int(value)` of an `ffprobe` field (`0` when it is missing).
fn py_int(value: Option<&Value>) -> Result<i64, DomainError> {
    match value {
        None => Ok(0),
        Some(Value::Bool(b)) => Ok(i64::from(*b)),
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| {
                #[expect(clippy::cast_possible_truncation, reason = "int() truncates")]
                n.as_f64()
                    .filter(|f| f.is_finite())
                    .map(|f| f.trunc() as i64)
            })
            .ok_or_else(|| crash(format!("ValueError: invalid int {n}"))),
        Some(Value::String(text)) => text.trim().replace('_', "").parse().map_err(|_| {
            crash(format!(
                "ValueError: invalid literal for int() with base 10: {}",
                str_repr(text)
            ))
        }),
        Some(other) => Err(crash(format!("TypeError: int() argument is {other}"))),
    }
}

/// Python's `float(value)` of an `ffprobe` field (`0.0` when it is missing).
fn py_float(value: Option<&Value>) -> Result<f64, DomainError> {
    match value {
        None => Ok(0.0),
        Some(Value::Bool(b)) => Ok(f64::from(u8::from(*b))),
        Some(Value::Number(n)) => n
            .as_f64()
            .ok_or_else(|| crash(format!("ValueError: invalid float {n}"))),
        Some(Value::String(text)) => text.trim().parse().map_err(|_| {
            crash(format!(
                "ValueError: could not convert string to float: {}",
                str_repr(text)
            ))
        }),
        Some(other) => Err(crash(format!("TypeError: float() argument is {other}"))),
    }
}

/// Python's `str(value)` of a tag value.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(true) => "True".to_owned(),
        Value::Bool(false) => "False".to_owned(),
        Value::Null => "None".to_owned(),
        other => other.to_string(),
    }
}

/// Runs `program` with `args` (empty standard input, output captured), killing it after
/// `timeout` (`subprocess.run(..., capture_output=True, text=True, timeout=…)`).
fn run(program: &Path, args: &[OsString], timeout: Duration) -> Result<Run, DomainError> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| os_crash(&e, program))?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            stop(&mut child);
            let mut command: Vec<String> = vec![program.display().to_string()];
            command.extend(args.iter().map(|a| a.to_string_lossy().into_owned()));
            return Err(crash(format!(
                "TimeoutExpired: Command '{}' timed out after {} seconds",
                str_list_repr(&command),
                timeout.as_secs()
            )));
        }
        Err(error) => {
            stop(&mut child);
            return Err(os_crash(&error, program));
        }
    };
    Ok(Run {
        status,
        stdout: joined(stdout),
        stderr: joined(stderr),
    })
}

/// Kills a child that overran its time and reaps it.
fn stop(child: &mut Child) {
    if let Err(error) = child.kill() {
        log::warn!("cannot stop process {}: {error}", child.id());
    }
    if let Err(error) = child.wait() {
        log::warn!("cannot reap process {}: {error}", child.id());
    }
}

/// Reads a pipe to its end on a thread of its own, so that neither pipe can fill up.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Option<JoinHandle<Vec<u8>>> {
    pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Err(error) = pipe.read_to_end(&mut bytes) {
                log::warn!("cannot read a tool's output: {error}");
            }
            bytes
        })
    })
}

/// The text a drained pipe held (invalid UTF-8 replaced).
fn joined(handle: Option<JoinHandle<Vec<u8>>>) -> String {
    let bytes = handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{py_float, py_int, py_str, rounded_frames, which};

    #[test]
    fn python_conversions() {
        assert_eq!(py_int(Some(&json!("44100"))).unwrap(), 44_100);
        assert_eq!(py_int(Some(&json!(" 48_000 "))).unwrap(), 48_000);
        assert_eq!(py_int(Some(&json!(2.9))).unwrap(), 2);
        assert_eq!(py_int(None).unwrap(), 0);
        assert!(py_int(Some(&json!("4.5"))).is_err());
        assert!(py_int(Some(&json!([1]))).is_err());
        assert!((py_float(Some(&json!("0.100000"))).unwrap() - 0.1).abs() < f64::EPSILON);
        assert!(py_float(Some(&json!("n/a"))).is_err());
        assert_eq!(py_str(&json!(3)), "3");
        assert_eq!(py_str(&json!(true)), "True");
        assert_eq!(py_str(&json!(null)), "None");
        assert_eq!(rounded_frames(0.5), 0);
        assert_eq!(rounded_frames(1.5), 2);
        assert_eq!(rounded_frames(-3.0), 0);
    }

    #[test]
    fn an_empty_search_path_finds_nothing() {
        assert_eq!(which("ffmpeg", Some(std::ffi::OsStr::new(""))), None);
    }
}

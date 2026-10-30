//! Reading and writing `.sonara` projects (`sonara.persistence.store`, ADR 0004,
//! docs/project-format.md).
//!
//! A `.sonara` path is either a **bundle** (a directory) or an **archive** (a ZIP file). Both
//! contain the same members. While a project is open its media lives in a *workspace*
//! directory: the bundle itself, or a managed extraction or scratch directory for archives and
//! unsaved projects.
//!
//! Every message, error code and detail key is the prototype's. Libraries (QG-13): [`zip`] for
//! archives, [`tempfile`] for atomic writes and temporary directories, [`walkdir`] for directory
//! walks, [`sha1`] for the archive workspace name (the prototype's digest, so both find the same
//! workspace), [`fs4`] for free disk space and [`jiff`] for UTC timestamps. All I/O is
//! synchronous; none of this runs on a real-time thread.
//!
//! **Errors.** The prototype's `SonaraError`s are [`StoreError::Domain`]. File-system and ZIP
//! failures that the prototype let escape as raw Python exceptions (its tool registry reports
//! them as `INTERNAL`) are [`StoreError::Io`] and [`StoreError::Zip`], which name the operation
//! and the path and keep the cause.
//!
//! **Clock.** Timestamps (`saved`, `created`, `saved_at`, backup names) come from an injectable
//! clock, so tests and golden checks are deterministic ([`ProjectStore::with_clock`]).

use std::collections::{BTreeSet, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use jiff::Timestamp;
use jiff::tz::Offset;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sonara_core::error::{DomainError, ErrorCode};
use sonara_core::model::JsonMap;
use sonara_core::model::media::{MediaAsset, MediaLocation};
use sonara_core::model::project::Project;
use sonara_core::pyfmt::{str_list_repr, str_repr};
use sonara_core::routing;
use walkdir::WalkDir;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::atomic::{atomic_write_bytes, fsync_dir, hex, link_or_copy, sha256_bytes, sha256_file};
use crate::migrations::upgrade;
use crate::pyjson::{dumps_compact, dumps_indented, py_str};
use crate::schema::{Schema, validate_document};
use crate::serializer::{dumps, has_note_articulations, has_parts, loads, project_from_value};

/// The manifest member.
pub const MANIFEST: &str = "manifest.json";
/// The project document member.
pub const PROJECT: &str = "project.json";
/// The `format` value of a manifest.
pub const MANIFEST_FORMAT: &str = "sonara.manifest";
/// The container format version this build writes and reads.
pub const FORMAT_VERSION: i64 = 1;
/// [`FORMAT_VERSION`] for comparing integral floats.
const FORMAT_VERSION_F64: f64 = 1.0;
/// The top-level names an archive may contain.
pub const ARCHIVE_MEMBERS: [&str; 6] = [
    MANIFEST,
    PROJECT,
    "media",
    "presets",
    "analysis",
    "thumbnails",
];
/// Top-level names that exist only in bundles, never in archives.
pub const BUNDLE_ONLY: [&str; 3] = ["backups", ".autosave", "cache"];
/// The features this build can open (`required_features` in the manifest).
pub const SUPPORTED_FEATURES: [&str; 12] = [
    "core",
    "audio-clips",
    "midi-clips",
    "automation",
    "sends",
    "sidechain",
    "tempo-ramps",
    "linked-media",
    "sections",
    "folders",
    "parts",
    "note-articulations",
];
/// The most members an archive may have.
pub const MAX_ARCHIVE_MEMBERS: usize = 100_000;
/// The largest total uncompressed size of an archive.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
/// A member larger than [`SUSPICIOUS_MEMBER_BYTES`] that inflates by more than this is
/// refused as a ZIP bomb.
pub const MAX_COMPRESSION_RATIO: f64 = 200.0;
/// The size above which [`MAX_COMPRESSION_RATIO`] applies.
pub const SUSPICIOUS_MEMBER_BYTES: u64 = 64 * 1024 * 1024;
/// Backups kept by default.
pub const DEFAULT_BACKUPS: usize = 10;

/// Read size when extracting members.
const EXTRACT_CHUNK: usize = 1 << 20;
/// Python's `zipfile.ZIP64_LIMIT`; a member whose size times 1.05 exceeds it is written in
/// ZIP64 form.
const ZIP64_LIMIT: u64 = (1 << 31) - 1;
/// Mode of members written from memory (Python's `writestr`).
const MEMORY_MEMBER_MODE: u32 = 0o600;

/// The two on-disk forms of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    /// A directory.
    Bundle,
    /// A ZIP file.
    Archive,
}

impl Layout {
    /// The layout as tool results name it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::Archive => "archive",
        }
    }
}

/// A project read from disk, with what happened while reading it.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedProject {
    /// The project.
    pub project: Project,
    /// The `.sonara` path that was opened.
    pub path: PathBuf,
    /// Where the project's media lives while it is open.
    pub workspace: PathBuf,
    /// The on-disk form.
    pub layout: Layout,
    /// Problems that did not stop the project from opening, in the prototype's wording.
    pub warnings: Vec<String>,
    /// What the project was recovered from: a backup name, the autosave, or a stale save.
    pub recovered_from: Option<String>,
    /// The schema version the document was migrated from.
    pub migrated_from: Option<i64>,
    /// Whether an autosave newer than the saved project exists.
    pub autosave_available: bool,
    /// IDs of assets whose files are missing.
    pub missing_assets: Vec<String>,
}

/// The outcome of [`ProjectStore::save`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveResult {
    /// The `.sonara` path written.
    pub path: PathBuf,
    /// Where the project's media lives after the save.
    pub workspace: PathBuf,
    /// The form written.
    pub layout: Layout,
    /// SHA-256 of the `project.json` bytes.
    pub project_sha256: String,
    /// The number of assets in the manifest.
    pub assets: usize,
    /// The backup of the previous `project.json`, if one was made.
    pub backup: Option<PathBuf>,
}

/// The outcome of [`ProjectStore::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    /// No missing media, no size or hash mismatches and no routing issues.
    pub ok: bool,
    /// IDs of assets whose file is missing.
    pub missing: Vec<String>,
    /// IDs of assets whose file has a different size.
    pub size_mismatch: Vec<String>,
    /// IDs of assets whose file has a different hash.
    pub hash_mismatch: Vec<String>,
    /// Workspace-relative paths of media files no asset refers to.
    pub unreferenced_files: Vec<String>,
    /// Messages of the project's routing issues.
    pub routing_issues: Vec<String>,
}

impl IntegrityReport {
    /// The report as tool results carry it, keys in the prototype's order.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "ok": self.ok,
            "missing": self.missing,
            "size_mismatch": self.size_mismatch,
            "hash_mismatch": self.hash_mismatch,
            "unreferenced_files": self.unreferenced_files,
            "routing_issues": self.routing_issues,
        })
    }
}

/// Errors of the project store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// An error the prototype reports with its code, message and details.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// A file-system operation failed (a raw `OSError` in the prototype).
    #[error("cannot {operation} {}: {source}", path.display())]
    Io {
        /// What was attempted, for example `"write"`.
        operation: &'static str,
        /// The file or directory.
        path: PathBuf,
        /// The cause.
        source: io::Error,
    },
    /// A ZIP archive could not be written or read (a raw `zipfile` error in the prototype).
    #[error("cannot {operation} archive {}: {source}", path.display())]
    Zip {
        /// What was attempted, for example `"write"`.
        operation: &'static str,
        /// The archive.
        path: PathBuf,
        /// The cause.
        source: ZipError,
    },
}

impl StoreError {
    /// The code a tool result reports: the domain error's code, or [`ErrorCode::Internal`] for
    /// a file-system or ZIP failure, as the prototype's tool registry reports escaped
    /// exceptions.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Domain(err) => err.code,
            Self::Io { .. } | Self::Zip { .. } => ErrorCode::Internal,
        }
    }

    /// The domain error, if this is one.
    #[must_use]
    pub fn as_domain(&self) -> Option<&DomainError> {
        match self {
            Self::Domain(err) => Some(err),
            Self::Io { .. } | Self::Zip { .. } => None,
        }
    }
}

/// Results of the project store.
pub type StoreResult<T> = std::result::Result<T, StoreError>;

/// Adds the operation and path to an I/O error.
trait IoContext<T> {
    fn io(self, operation: &'static str, path: &Path) -> StoreResult<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn io(self, operation: &'static str, path: &Path) -> StoreResult<T> {
        self.map_err(|source| StoreError::Io {
            operation,
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Adds the operation and archive path to a ZIP error.
trait ZipContext<T> {
    fn zip(self, operation: &'static str, path: &Path) -> StoreResult<T>;
}

impl<T> ZipContext<T> for zip::result::ZipResult<T> {
    fn zip(self, operation: &'static str, path: &Path) -> StoreResult<T> {
        self.map_err(|source| StoreError::Zip {
            operation,
            path: path.to_path_buf(),
            source,
        })
    }
}

fn domain(code: ErrorCode, message: impl Into<String>) -> StoreError {
    StoreError::Domain(DomainError::new(code, message))
}

// -- free functions -------------------------------------------------------------------------

/// The manifest's `required_features` for `project`, sorted.
#[must_use]
pub fn required_features(project: &Project) -> Vec<String> {
    let mut features = BTreeSet::from(["core"]);
    for track in &project.tracks {
        if track.audio_clips().next().is_some() {
            features.insert("audio-clips");
        }
        if track.midi_clips().next().is_some() {
            features.insert("midi-clips");
        }
        if !track.sends.is_empty() {
            features.insert("sends");
        }
        if track.parent_id.as_deref().is_some_and(|id| !id.is_empty()) {
            features.insert("folders");
        }
    }
    for track in project.all_tracks() {
        if !track.automation.is_empty() {
            features.insert("automation");
        }
        if track
            .devices()
            .any(|d| d.sidechain_id.as_deref().is_some_and(|id| !id.is_empty()))
        {
            features.insert("sidechain");
        }
    }
    if project.tempo_map.points().iter().any(|p| p.ramp) {
        features.insert("tempo-ramps");
    }
    if project
        .assets
        .iter()
        .any(|a| a.location == MediaLocation::Linked)
    {
        features.insert("linked-media");
    }
    if !project.sections.is_empty() {
        features.insert("sections");
    }
    // A build that reads schema version 2 but not these would drop them (notes keep no
    // unknown keys); the features make it refuse the project and name what is missing.
    if has_parts(project) {
        features.insert("parts");
    }
    if has_note_articulations(project) {
        features.insert("note-articulations");
    }
    features.into_iter().map(str::to_owned).collect()
}

/// The layout of the project at `path`: a directory with a manifest or project document is a
/// bundle, a ZIP file an archive; anything else is not a project.
#[must_use]
pub fn detect_layout(path: &Path) -> Option<Layout> {
    if path.is_dir() {
        return (path.join(MANIFEST).exists() || path.join(PROJECT).exists())
            .then_some(Layout::Bundle);
    }
    let is_zip = path.is_file() && File::open(path).is_ok_and(|file| ZipArchive::new(file).is_ok());
    is_zip.then_some(Layout::Archive)
}

/// Where `asset`'s file lives: inside `workspace` when embedded, at its own path when linked.
#[must_use]
pub fn asset_path(asset: &MediaAsset, workspace: &Path) -> PathBuf {
    match asset.location {
        MediaLocation::Embedded => {
            let mut path = workspace.to_path_buf();
            for part in posix_parts(&asset.path) {
                path.push(part);
            }
            path
        }
        MediaLocation::Linked => PathBuf::from(&asset.path),
    }
}

/// `PurePosixPath(text).parts` without the root: empty and `.` segments vanish.
fn posix_parts(text: &str) -> Vec<&str> {
    text.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

/// A POSIX path in `PurePosixPath`'s normal form, for comparing paths as the prototype does.
fn posix_normal(text: &str) -> String {
    let root = if text.starts_with('/') { "/" } else { "" };
    format!("{root}{}", posix_parts(text).join("/"))
}

/// A relative path's components joined with `/` (Python's `as_posix()`).
fn as_posix(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// A path as Python's `Path()` spells it: no `.` segments, no repeated or trailing separators.
fn py_path(path: &Path) -> PathBuf {
    let normal: PathBuf = path
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    if normal.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normal
    }
}

/// Python's `Path.expanduser()` for a leading `~` (the current user's home directory).
fn expand_user(path: &Path) -> PathBuf {
    let mut components = path.components();
    match components.next() {
        Some(Component::Normal(first)) if first == "~" => match std::env::home_dir() {
            Some(home) if components.as_path().as_os_str().is_empty() => home,
            Some(home) => home.join(components.as_path()),
            None => path.to_path_buf(),
        },
        _ => path.to_path_buf(),
    }
}

/// Python's non-strict `Path.resolve()`: absolute, with symbolic links resolved as far as the
/// path exists.
fn resolve(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut existing = absolute.as_path();
    let mut missing = Vec::new();
    loop {
        if let Ok(mut real) = fs::canonicalize(existing) {
            for part in missing.iter().rev() {
                real.push(part);
            }
            return real;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                existing = parent;
            }
            _ => return absolute,
        }
    }
}

/// Files under `dir` (recursively, not following symbolic links), sorted like Python sorts
/// paths: component by component.
fn files_under(dir: &Path, follow_links: bool) -> Vec<walkdir::DirEntry> {
    let mut entries: Vec<walkdir::DirEntry> = WalkDir::new(dir)
        .min_depth(1)
        .follow_links(follow_links)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .collect();
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    entries
}

/// `backups/project-*.json`, sorted by name (oldest first: the names carry the time).
#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the prototype's glob `project-*.json` is case-sensitive on POSIX"
)]
fn backup_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry.file_name().to_str().is_some_and(|name| {
                name.len() >= "project-.json".len()
                    && name.starts_with("project-")
                    && name.ends_with(".json")
            })
        })
        .map(|entry| entry.path())
        .collect();
    files.sort();
    files
}

/// The calendar time of `time` in UTC. Times beyond jiff's range (years ±9999) are clamped;
/// the prototype's `datetime` cannot represent them either.
fn utc(time: SystemTime) -> jiff::civil::DateTime {
    let timestamp = Timestamp::try_from(time).unwrap_or(if time < UNIX_EPOCH {
        Timestamp::MIN
    } else {
        Timestamp::MAX
    });
    Offset::UTC.to_datetime(timestamp)
}

/// `strftime("%Y-%m-%dT%H:%M:%SZ")` in UTC.
fn iso_seconds(time: SystemTime) -> String {
    let t = utc(time);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

/// `strftime("%Y%m%dT%H%M%S%fZ")` in UTC: microseconds, truncated like `datetime.now()`.
fn backup_stamp(time: SystemTime) -> String {
    let t = utc(time);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}{:06}Z",
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        t.subsec_nanosecond() / 1000
    )
}

/// A ZIP timestamp (UTC; ZIP has no time zone). Times ZIP cannot hold (before 1980, after
/// 2107) become 1980-01-01.
fn zip_time(time: SystemTime) -> zip::DateTime {
    let t = utc(time);
    let parts = (
        u16::try_from(t.year()),
        u8::try_from(t.month()),
        u8::try_from(t.day()),
        u8::try_from(t.hour()),
        u8::try_from(t.minute()),
        u8::try_from(t.second()),
    );
    match parts {
        (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) => {
            zip::DateTime::from_date_and_time(year, month, day, hour, minute, second)
                .unwrap_or_default()
        }
        _ => zip::DateTime::default(),
    }
}

/// Whether Python's `zipfile` writes a member of `size` bytes in ZIP64 form
/// (`size * 1.05 > ZIP64_LIMIT`).
fn needs_zip64(size: u64) -> bool {
    u128::from(size) * 21 > u128::from(ZIP64_LIMIT) * 20
}

/// A file's permission bits for its archive member (Python stores `st_mode`).
fn member_mode(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.permissions().mode() & 0o7777
    }
    #[cfg(not(unix))]
    {
        if metadata.permissions().readonly() {
            0o444
        } else {
            0o666
        }
    }
}

/// The document's project, and the schema version it was migrated from when a migration
/// changed its content ([`upgrade`]).
fn decode(raw: &[u8]) -> sonara_core::error::Result<(Project, Option<i64>)> {
    let (doc, migrated_from) = upgrade(loads(raw)?)?;
    Ok((project_from_value(&doc)?, migrated_from))
}

/// Parses and validates a manifest; the error is the text the "unreadable" warning quotes.
fn parse_manifest(raw: &[u8]) -> Result<JsonMap, String> {
    let text = std::str::from_utf8(raw).map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    if value.is_object() {
        validate_document(&value, Schema::Manifest).map_err(|err| err.to_string())?;
    }
    match value {
        Value::Object(map) => Ok(map),
        _ => Err("manifest is not a JSON object".to_owned()),
    }
}

/// Python's `int(value) > FORMAT_VERSION` for a schema-validated integer.
fn newer_format(value: &Value) -> bool {
    match value {
        Value::Number(number) => match number.as_i64() {
            Some(version) => version > FORMAT_VERSION,
            None => {
                // Beyond `i64`, or an integral float (`int(2.0)` is 2).
                number.as_u64().is_some()
                    || number
                        .as_f64()
                        .is_some_and(|version| version.trunc() > FORMAT_VERSION_F64)
            }
        },
        _ => false,
    }
}

/// Python's `str.strip()`: Rust's whitespace plus the separators `\x1c`–`\x1f`.
fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || ('\x1c'..='\x1f').contains(&c))
}

/// Recursively copies `src` into `dst`, overwriting files (Python's
/// `shutil.copytree(src, dst, dirs_exist_ok=True)`, following symbolic links).
fn copy_tree(src: &Path, dst: &Path) -> StoreResult<()> {
    fs::create_dir_all(dst).io("create", dst)?;
    for entry in files_under(src, true) {
        let rel = entry.path().strip_prefix(src).unwrap_or(entry.path());
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).io("create", &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).io("create", parent)?;
            }
            fs::copy(entry.path(), &target).io("copy", entry.path())?;
        }
    }
    Ok(())
}

/// Bytes available to unprivileged users on the file system holding `path` (Python's
/// `shutil.disk_usage(path).free`, which is `f_bavail * f_frsize`).
fn free_space(path: &Path) -> io::Result<u64> {
    fs4::available_space(path)
}

/// A member's facts from the central directory (Python's `ZipInfo`).
struct Member {
    name: String,
    size: u64,
    compressed: u64,
    mode: u32,
}

/// Whether the prototype accepts the member name (`safe_extract`): relative, no `..`, no
/// backslash or drive letter, under an allowed top-level name, and not a symbolic link.
fn member_allowed(name: &str, mode: u32) -> bool {
    let parts = posix_parts(name);
    let is_link = mode != 0 && mode & 0o170_000 == 0o120_000;
    !name.is_empty()
        && !name.contains('\\')
        && !name.starts_with('/')
        && !parts.contains(&"..")
        && name.chars().nth(1) != Some(':')
        && parts
            .first()
            .is_some_and(|first| ARCHIVE_MEMBERS.contains(first))
        && !is_link
}

/// Whether a member inflates suspiciously (Python divides the sizes as floats).
#[expect(
    clippy::cast_precision_loss,
    reason = "the prototype compares the float quotient of the sizes"
)]
fn suspicious_ratio(member: &Member) -> bool {
    member.compressed > 0
        && member.size as f64 / member.compressed as f64 > MAX_COMPRESSION_RATIO
        && member.size > SUSPICIOUS_MEMBER_BYTES
}

/// Whether a ZIP error means the file is not a valid archive (Python's `BadZipFile`), rather
/// than an I/O failure.
fn is_bad_zip(err: &ZipError) -> bool {
    match err {
        ZipError::Io(io_err) => matches!(
            io_err.kind(),
            io::ErrorKind::UnexpectedEof | io::ErrorKind::InvalidData
        ),
        _ => true,
    }
}

/// Extracts a project archive into `dest`, rejecting traversal, links, ZIP bombs and unknown
/// members.
///
/// # Errors
///
/// - [`ErrorCode::FormatError`]: `"<name> is not a valid project archive"`;
///   `"archive member '<member>' is not allowed"` (detail `member`); `"… has a suspicious
///   compression ratio"`; `"… escapes the project"`; `"… is larger than declared"`;
/// - [`ErrorCode::LimitExceeded`]: `"archive has too many members"`, `"archive is too large to
///   extract"`;
/// - [`ErrorCode::IoError`]: `"not enough disk space to open this archive"`;
/// - [`StoreError::Io`] or [`StoreError::Zip`] when reading the archive or writing a file
///   fails.
pub fn safe_extract(archive: &Path, dest: &Path) -> StoreResult<()> {
    let file = File::open(archive).io("open", archive)?;
    let mut zip = match ZipArchive::new(file) {
        Ok(zip) => zip,
        Err(err) if is_bad_zip(&err) => {
            let name = archive
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return Err(domain(
                ErrorCode::FormatError,
                format!("{name} is not a valid project archive"),
            ));
        }
        Err(err) => return Err(err).zip("open", archive),
    };
    if zip.len() > MAX_ARCHIVE_MEMBERS {
        return Err(domain(
            ErrorCode::LimitExceeded,
            "archive has too many members",
        ));
    }
    let mut members = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).zip("read", archive)?;
        let full = entry.name().zip("read", archive)?;
        // Python's `ZipInfo` cuts a name at its first NUL.
        let name = full.split('\0').next().unwrap_or_default().to_owned();
        members.push(Member {
            name,
            size: entry.size(),
            compressed: entry.compressed_size(),
            mode: entry.external_attributes() >> 16,
        });
    }
    let total = members
        .iter()
        .fold(0_u64, |sum, member| sum.saturating_add(member.size));
    if total > MAX_ARCHIVE_BYTES {
        return Err(domain(
            ErrorCode::LimitExceeded,
            "archive is too large to extract",
        ));
    }
    if total > free_space(dest).io("query free space of", dest)? {
        return Err(domain(
            ErrorCode::IoError,
            "not enough disk space to open this archive",
        ));
    }
    let root = resolve(dest);
    let mut buffer = vec![0_u8; EXTRACT_CHUNK];
    for (index, member) in members.iter().enumerate() {
        let name = member.name.as_str();
        let quoted = str_repr(name);
        if !member_allowed(name, member.mode) {
            return Err(StoreError::Domain(
                DomainError::new(
                    ErrorCode::FormatError,
                    format!("archive member {quoted} is not allowed"),
                )
                .with_detail("member", name),
            ));
        }
        if suspicious_ratio(member) {
            return Err(domain(
                ErrorCode::FormatError,
                format!("archive member {quoted} has a suspicious compression ratio"),
            ));
        }
        let mut target = dest.to_path_buf();
        for part in posix_parts(name) {
            target.push(part);
        }
        let target = resolve(&target);
        if !target.starts_with(&root) {
            return Err(domain(
                ErrorCode::FormatError,
                format!("archive member {quoted} escapes the project"),
            ));
        }
        if name.ends_with('/') {
            fs::create_dir_all(&target).io("create", &target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).io("create", parent)?;
        }
        let mut source = zip.by_index(index).zip("read", archive)?;
        copy_member(&mut source, member, &target, archive, &mut buffer)?;
    }
    Ok(())
}

/// Writes one member to `target` in chunks, failing once it grows beyond its declared size.
fn copy_member(
    source: &mut impl Read,
    member: &Member,
    target: &Path,
    archive: &Path,
    buffer: &mut [u8],
) -> StoreResult<()> {
    let mut out = File::create(target).io("create", target)?;
    let mut written = 0_u64;
    loop {
        let read = source.read(buffer).io("extract", archive)?;
        let Some(chunk) = buffer.get(..read).filter(|chunk| !chunk.is_empty()) else {
            return Ok(());
        };
        written = written.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        if written > member.size {
            return Err(domain(
                ErrorCode::FormatError,
                format!(
                    "archive member {} is larger than declared",
                    str_repr(&member.name)
                ),
            ));
        }
        out.write_all(chunk).io("write", target)?;
    }
}

// -- the store ------------------------------------------------------------------------------

/// The source of the current time.
type Clock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// Reads and writes projects, and manages workspaces under one root directory.
#[derive(Clone)]
pub struct ProjectStore {
    workspaces_root: PathBuf,
    backup_retention: usize,
    clock: Clock,
}

impl fmt::Debug for ProjectStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProjectStore")
            .field("workspaces_root", &self.workspaces_root)
            .field("backup_retention", &self.backup_retention)
            .finish_non_exhaustive()
    }
}

impl ProjectStore {
    /// A store whose scratch and extraction workspaces live under `workspaces_root`, keeping
    /// [`DEFAULT_BACKUPS`] backups and reading the system clock.
    #[must_use]
    pub fn new(workspaces_root: impl Into<PathBuf>) -> Self {
        Self {
            workspaces_root: workspaces_root.into(),
            backup_retention: DEFAULT_BACKUPS,
            clock: Arc::new(SystemTime::now),
        }
    }

    /// The store keeping `retention` backups of `project.json` per bundle (0: none).
    #[must_use]
    pub fn with_backups(mut self, retention: usize) -> Self {
        self.backup_retention = retention;
        self
    }

    /// The store reading the time from `clock`, for deterministic tests and golden checks.
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> SystemTime + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// The directory holding scratch and extraction workspaces.
    #[must_use]
    pub fn workspaces_root(&self) -> &Path {
        &self.workspaces_root
    }

    /// The number of backups kept per bundle.
    #[must_use]
    pub fn backup_retention(&self) -> usize {
        self.backup_retention
    }

    fn now(&self) -> String {
        iso_seconds((self.clock)())
    }

    // -- workspaces -----------------------------------------------------------------------

    /// The scratch workspace of an unsaved project (`new-<project id>`), created with its
    /// `media/audio` directory and `workspace.json` when missing.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when a directory or `workspace.json` cannot be created.
    pub fn scratch_workspace(&self, project_id: &str) -> StoreResult<PathBuf> {
        let workspace = self.workspaces_root.join(format!("new-{project_id}"));
        let audio = workspace.join("media").join("audio");
        fs::create_dir_all(&audio).io("create", &audio)?;
        let info = workspace.join("workspace.json");
        if !info.exists() {
            let data = json!({"kind": "scratch", "project_id": project_id, "created": self.now()});
            atomic_write_bytes(&info, &dumps_compact(&data)).io("write", &info)?;
        }
        Ok(workspace)
    }

    /// The extraction workspace of an archive: `pkg-` and the first 12 hex digits of the SHA-1
    /// of its resolved path.
    fn archive_workspace(&self, archive: &Path) -> PathBuf {
        let resolved = resolve(archive);
        let digest = Sha1::digest(resolved.to_string_lossy().as_bytes());
        let short = hex(digest.get(..6).unwrap_or_default());
        self.workspaces_root.join(format!("pkg-{short}"))
    }

    // -- manifest -------------------------------------------------------------------------

    fn manifest(&self, project: &Project, project_bytes: &[u8], workspace: &Path) -> Value {
        let mut assets: Vec<&MediaAsset> = project.assets.iter().collect();
        assets.sort_by(|a, b| a.id.cmp(&b.id));
        let assets: Vec<Value> = assets
            .into_iter()
            .map(|a| {
                let size = fs::metadata(asset_path(a, workspace)).map_or(a.size_bytes, |m| m.len());
                json!({
                    "id": a.id,
                    "path": a.path,
                    "location": a.location.as_str(),
                    "size": size,
                    "sha256": a.sha256,
                    "kind": a.kind.as_str(),
                })
            })
            .collect();
        json!({
            "format": MANIFEST_FORMAT,
            "format_version": FORMAT_VERSION,
            "sonara_version": env!("CARGO_PKG_VERSION"),
            "project_id": project.id,
            "saved": self.now(),
            "required_features": required_features(project),
            "project_file": {
                "path": PROJECT,
                "sha256": sha256_bytes(project_bytes),
                "size": project_bytes.len(),
            },
            "assets": assets,
            "compatibility": {"min_reader_version": FORMAT_VERSION},
        })
    }

    // -- saving ---------------------------------------------------------------------------

    /// Saves `project`, whose media lives in `workspace`, to `target`. Without a `layout`, an
    /// existing target keeps its layout and a new one becomes a bundle.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::MediaMissing`]: `"cannot save: <n> embedded media file(s) are missing
    ///   from the workspace"` with the `missing` detail (at most 20 asset names);
    /// - [`ErrorCode::Conflict`] when the target has the other layout;
    /// - [`StoreError::Io`] or [`StoreError::Zip`] when writing fails.
    pub fn save(
        &self,
        project: &Project,
        workspace: &Path,
        target: &Path,
        layout: Option<Layout>,
    ) -> StoreResult<SaveResult> {
        let target = py_path(target);
        let layout = layout
            .or_else(|| {
                if target.exists() {
                    detect_layout(&target)
                } else {
                    None
                }
            })
            .unwrap_or(Layout::Bundle);
        let project_bytes = dumps(project);
        Self::check_media(project, workspace)?;
        match layout {
            Layout::Bundle => self.save_bundle(project, &project_bytes, workspace, &target),
            Layout::Archive => self.save_archive(project, &project_bytes, workspace, &target),
        }
    }

    fn check_media(project: &Project, workspace: &Path) -> StoreResult<()> {
        let missing: Vec<&str> = project
            .assets
            .iter()
            .filter(|a| a.location == MediaLocation::Embedded && !asset_path(a, workspace).exists())
            .map(|a| a.name.as_str())
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(StoreError::Domain(
            DomainError::new(
                ErrorCode::MediaMissing,
                format!(
                    "cannot save: {} embedded media file(s) are missing from the workspace",
                    missing.len()
                ),
            )
            .with_detail("missing", missing.into_iter().take(20).collect::<Vec<_>>()),
        ))
    }

    fn save_bundle(
        &self,
        project: &Project,
        project_bytes: &[u8],
        workspace: &Path,
        target: &Path,
    ) -> StoreResult<SaveResult> {
        if target.exists() && !target.is_dir() {
            return Err(domain(
                ErrorCode::Conflict,
                format!("{} exists and is not a project bundle", target.display()),
            ));
        }
        fs::create_dir_all(target).io("create", target)?;
        if resolve(workspace) != resolve(target) {
            for asset in &project.assets {
                if asset.location == MediaLocation::Embedded {
                    let src = asset_path(asset, workspace);
                    let dst = asset_path(asset, target);
                    link_or_copy(&src, &dst).io("copy", &src)?;
                }
            }
            for extra in ["presets", "thumbnails"] {
                let src = workspace.join(extra);
                if src.is_dir() {
                    copy_tree(&src, &target.join(extra))?;
                }
            }
        }
        let backup = self.backup(target, project_bytes)?;
        let manifest = self.manifest(project, project_bytes, target);
        let project_path = target.join(PROJECT);
        atomic_write_bytes(&project_path, project_bytes).io("write", &project_path)?;
        let manifest_path = target.join(MANIFEST);
        atomic_write_bytes(&manifest_path, &dumps_indented(&manifest))
            .io("write", &manifest_path)?;
        self.discard_autosave(target);
        fsync_dir(target);
        Ok(SaveResult {
            path: target.to_path_buf(),
            workspace: target.to_path_buf(),
            layout: Layout::Bundle,
            project_sha256: sha256_bytes(project_bytes),
            assets: project.assets.len(),
            backup,
        })
    }

    /// Copies the bundle's current `project.json` to `backups/project-<UTC time>.json` unless
    /// the new bytes are the same, and removes the oldest backups beyond the retention.
    fn backup(&self, bundle: &Path, new_bytes: &[u8]) -> StoreResult<Option<PathBuf>> {
        let current = bundle.join(PROJECT);
        if self.backup_retention == 0 || !current.exists() {
            return Ok(None);
        }
        let old = fs::read(&current).io("read", &current)?;
        if old == new_bytes {
            return Ok(None);
        }
        let backups = bundle.join("backups");
        if let Err(err) = fs::create_dir(&backups)
            && !(err.kind() == io::ErrorKind::AlreadyExists && backups.is_dir())
        {
            return Err(err).io("create", &backups);
        }
        let dest = backups.join(format!("project-{}.json", backup_stamp((self.clock)())));
        atomic_write_bytes(&dest, &old).io("write", &dest)?;
        let existing = backup_files(&backups);
        let stale = existing.len().saturating_sub(self.backup_retention);
        for path in existing.iter().take(stale) {
            // ERR-06: the prototype ignores a backup it cannot remove
            // (`contextlib.suppress(OSError)`); the save itself succeeded.
            let _ = fs::remove_file(path);
        }
        Ok(Some(dest))
    }

    fn save_archive(
        &self,
        project: &Project,
        project_bytes: &[u8],
        workspace: &Path,
        target: &Path,
    ) -> StoreResult<SaveResult> {
        if target.is_dir() {
            return Err(domain(
                ErrorCode::Conflict,
                format!(
                    "{} is a bundle directory; cannot overwrite it with an archive",
                    target.display()
                ),
            ));
        }
        let parent = match target.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        fs::create_dir_all(parent).io("create", parent)?;
        let manifest = self.manifest(project, project_bytes, workspace);
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Removed again on any failure, when the temporary file is dropped unpersisted.
        let mut tmp = tempfile::Builder::new()
            .prefix(&format!(".{name}."))
            .suffix(".tmp")
            .tempfile_in(parent)
            .io("create a temporary file in", parent)?;
        self.write_archive(
            tmp.as_file_mut(),
            &dumps_indented(&manifest),
            project_bytes,
            project,
            workspace,
            target,
        )?;
        tmp.as_file().sync_all().io("sync", tmp.path())?;
        tmp.persist(target)
            .map_err(|err| err.error)
            .io("replace", target)?;
        fsync_dir(parent);
        Ok(SaveResult {
            path: target.to_path_buf(),
            workspace: workspace.to_path_buf(),
            layout: Layout::Archive,
            project_sha256: sha256_bytes(project_bytes),
            assets: project.assets.len(),
            backup: None,
        })
    }

    /// Writes the archive members: manifest and project DEFLATED, embedded media STORED (audio
    /// is large, often compressed already, and STORED members stream without inflating), then
    /// `presets` and `thumbnails` DEFLATED.
    fn write_archive<W: Write + Seek>(
        &self,
        out: W,
        manifest_bytes: &[u8],
        project_bytes: &[u8],
        project: &Project,
        workspace: &Path,
        target: &Path,
    ) -> StoreResult<()> {
        let mut zip = ZipWriter::new(out);
        let now = zip_time((self.clock)());
        for (name, data) in [(MANIFEST, manifest_bytes), (PROJECT, project_bytes)] {
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .unix_permissions(MEMORY_MEMBER_MODE)
                .last_modified_time(now)
                .large_file(needs_zip64(u64::try_from(data.len()).unwrap_or(u64::MAX)));
            zip.start_file(name, options).zip("write", target)?;
            zip.write_all(data).io("write", target)?;
        }
        // The zip crate refuses a repeated name; two assets sharing a path share its bytes.
        let mut written = HashSet::new();
        for asset in &project.assets {
            if asset.location != MediaLocation::Embedded {
                continue;
            }
            let name = posix_parts(&asset.path).join("/");
            if written.insert(name.clone()) {
                let source = asset_path(asset, workspace);
                add_file(&mut zip, &source, &name, CompressionMethod::Stored, target)?;
            }
        }
        for extra in ["presets", "thumbnails"] {
            let dir = workspace.join(extra);
            if !dir.is_dir() {
                continue;
            }
            for entry in files_under(&dir, false) {
                if entry.file_type().is_file() {
                    let rel = entry.path().strip_prefix(workspace).unwrap_or(entry.path());
                    let name = as_posix(rel);
                    add_file(
                        &mut zip,
                        entry.path(),
                        &name,
                        CompressionMethod::Deflated,
                        target,
                    )?;
                }
            }
        }
        zip.finish().zip("write", target)?;
        Ok(())
    }

    // -- autosave -------------------------------------------------------------------------

    /// Writes `.autosave/project.json` and `.autosave/info.json` in `workspace`; returns the
    /// document's path.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when a file cannot be written.
    pub fn write_autosave(&self, project: &Project, workspace: &Path) -> StoreResult<PathBuf> {
        let data = dumps(project);
        let dir = workspace.join(".autosave");
        let target = dir.join(PROJECT);
        atomic_write_bytes(&target, &data).io("write", &target)?;
        let info = json!({
            "saved_at": self.now(),
            "sha256": sha256_bytes(&data),
            "project_id": project.id,
            "name": project.name,
        });
        let info_path = dir.join("info.json");
        atomic_write_bytes(&info_path, &dumps_compact(&info)).io("write", &info_path)?;
        Ok(target)
    }

    /// Removes the workspace's autosave; a missing one is fine and errors are ignored, as in
    /// the prototype.
    pub fn discard_autosave(&self, workspace: &Path) {
        // ERR-06: `shutil.rmtree(..., ignore_errors=True)` in the prototype.
        let _ = fs::remove_dir_all(workspace.join(".autosave"));
    }

    /// The autosave's `info.json` (`saved_at`, `sha256`, `project_id`, `name`), or `None` when
    /// there is no complete, readable autosave.
    #[must_use]
    pub fn autosave_info(&self, workspace: &Path) -> Option<JsonMap> {
        let dir = workspace.join(".autosave");
        let info = dir.join("info.json");
        if !info.exists() || !dir.join(PROJECT).exists() {
            return None;
        }
        let text = fs::read_to_string(&info).ok()?;
        match serde_json::from_str(&text).ok()? {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }

    /// The autosaved project of `workspace`.
    ///
    /// # Errors
    ///
    /// The errors of reading a project document (invalid JSON, schema, migration, model), or
    /// [`StoreError::Io`] when the file cannot be read.
    pub fn read_autosave(&self, workspace: &Path) -> StoreResult<Project> {
        let path = workspace.join(".autosave").join(PROJECT);
        let raw = fs::read(&path).io("read", &path)?;
        Ok(decode(&raw)?.0)
    }

    /// Scratch and extraction workspaces with an autosave, for crash recovery after a
    /// restart: each is `{"workspace": <path>, ...info.json}`, sorted by path.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when the workspaces root cannot be listed.
    pub fn recoverable_sessions(&self) -> StoreResult<Vec<JsonMap>> {
        let root = &self.workspaces_root;
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut workspaces = fs::read_dir(root)
            .io("list", root)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<io::Result<Vec<_>>>()
            .io("list", root)?;
        workspaces.sort();
        let mut sessions = Vec::new();
        for workspace in workspaces {
            if !workspace.is_dir() {
                continue;
            }
            if let Some(info) = self.autosave_info(&workspace).filter(|i| !i.is_empty()) {
                let mut session = JsonMap::new();
                session.insert(
                    "workspace".to_owned(),
                    Value::from(workspace.to_string_lossy().into_owned()),
                );
                for (key, value) in info {
                    session.insert(key, value);
                }
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    // -- opening --------------------------------------------------------------------------

    /// Opens the project at `path`. An archive is extracted into its workspace first. With
    /// `recover_autosave`, a newer autosave replaces the saved project.
    ///
    /// # Errors
    ///
    /// - [`ErrorCode::NotFound`]: `"project <path> does not exist"`;
    /// - [`ErrorCode::FormatError`]: `"<path> is not a Sonara project (no
    ///   manifest/project.json)"`, the archive errors of [`safe_extract`], or `"project.json is
    ///   missing or damaged and no valid backup exists"`;
    /// - [`ErrorCode::Unsupported`] for a newer container format, unknown required features
    ///   (detail `features`) or a newer schema version;
    /// - [`StoreError::Io`] or [`StoreError::Zip`] when reading fails.
    pub fn open(&self, path: &Path, recover_autosave: bool) -> StoreResult<LoadedProject> {
        let path = expand_user(&py_path(path));
        if !path.exists() {
            return Err(domain(
                ErrorCode::NotFound,
                format!("project {} does not exist", path.display()),
            ));
        }
        let Some(layout) = detect_layout(&path) else {
            return Err(domain(
                ErrorCode::FormatError,
                format!(
                    "{} is not a Sonara project (no manifest/project.json)",
                    path.display()
                ),
            ));
        };
        let workspace = match layout {
            Layout::Archive => self.extract(&path)?,
            Layout::Bundle => path.clone(),
        };
        let mut loaded = Self::open_workspace(&workspace, &path, layout)?;
        if let Some(info) = self.autosave_info(&workspace).filter(|i| !i.is_empty()) {
            let saved = Value::String(sha256_bytes(&dumps(&loaded.project)));
            if info.get("sha256") != Some(&saved) {
                loaded.autosave_available = true;
                if recover_autosave {
                    loaded.project = self.read_autosave(&workspace)?;
                    let saved_at = info.get("saved_at").map_or_else(String::new, py_str);
                    loaded.recovered_from =
                        Some(py_strip(&format!("autosave {saved_at}")).to_owned());
                    loaded
                        .warnings
                        .push("restored unsaved changes from the autosave".to_owned());
                }
            }
        }
        loaded.missing_assets = loaded
            .project
            .assets
            .iter()
            .filter(|a| !asset_path(a, &workspace).exists())
            .map(|a| a.id.clone())
            .collect();
        if !loaded.missing_assets.is_empty() {
            loaded.warnings.push(format!(
                "{} media file(s) are missing; relink them to restore audio",
                loaded.missing_assets.len()
            ));
        }
        Ok(loaded)
    }

    fn open_workspace(workspace: &Path, path: &Path, layout: Layout) -> StoreResult<LoadedProject> {
        let mut warnings = Vec::new();
        let manifest = Self::read_manifest(workspace, &mut warnings)?;
        let mut recovered = None;
        let mut project = None;
        let mut migrated = None;
        let project_path = workspace.join(PROJECT);
        if project_path.exists() {
            let raw = fs::read(&project_path).io("read", &project_path)?;
            let stale = manifest.as_ref().is_some_and(|m| {
                m.get("project_file").and_then(|f| f.get("sha256"))
                    != Some(&Value::String(sha256_bytes(&raw)))
            });
            match decode(&raw) {
                Ok((decoded, from)) => {
                    project = Some(decoded);
                    migrated = from;
                    if stale {
                        recovered = Some(
                            "project.json (manifest was stale after an interrupted save)"
                                .to_owned(),
                        );
                        warnings.push(
                            "the previous save was interrupted after project.json was written; \
                             the manifest will be rewritten"
                                .to_owned(),
                        );
                    }
                }
                // Written by a newer Sonara: never fall back to an older backup, which would
                // silently discard the newer work on the next save.
                Err(err) if err.code == ErrorCode::Unsupported => return Err(err.into()),
                Err(err) => warnings.push(format!("project.json is damaged: {}", err.message)),
            }
        }
        let project = if let Some(project) = project {
            project
        } else {
            let (project, source) = Self::from_backups(workspace)?;
            warnings.push(format!("recovered from backup {source}"));
            recovered = Some(source);
            project
        };
        if let Some(id) = manifest.as_ref().and_then(|m| m.get("project_id"))
            && !id.is_null()
            && id.as_str() != Some(project.id.as_str())
        {
            warnings.push("manifest project id does not match project.json".to_owned());
        }
        Ok(LoadedProject {
            project,
            path: path.to_path_buf(),
            workspace: workspace.to_path_buf(),
            layout,
            warnings,
            recovered_from: recovered,
            migrated_from: migrated,
            autosave_available: false,
            missing_assets: Vec::new(),
        })
    }

    /// The workspace's manifest, when it exists and is valid (a warning says why not), after
    /// checking that this build can open the project.
    fn read_manifest(workspace: &Path, warnings: &mut Vec<String>) -> StoreResult<Option<JsonMap>> {
        let manifest_path = workspace.join(MANIFEST);
        if !manifest_path.exists() {
            warnings.push("manifest.json is missing; it will be recreated on save".to_owned());
            return Ok(None);
        }
        let raw = fs::read(&manifest_path).io("read", &manifest_path)?;
        let manifest = match parse_manifest(&raw) {
            Ok(manifest) => manifest,
            Err(reason) => {
                warnings.push(format!(
                    "manifest is unreadable ({reason}); continuing without it"
                ));
                return Ok(None);
            }
        };
        if manifest.get("format_version").is_some_and(newer_format) {
            return Err(domain(
                ErrorCode::Unsupported,
                "project was saved with a newer container format",
            ));
        }
        let unknown: BTreeSet<&str> = manifest
            .get("required_features")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|feature| !SUPPORTED_FEATURES.contains(feature))
            .collect();
        if !unknown.is_empty() {
            let unknown: Vec<&str> = unknown.into_iter().collect();
            return Err(StoreError::Domain(
                DomainError::new(
                    ErrorCode::Unsupported,
                    format!(
                        "project requires features this version does not support: {}",
                        str_list_repr(&unknown)
                    ),
                )
                .with_detail("features", unknown),
            ));
        }
        Ok(Some(manifest))
    }

    /// The newest backup that decodes, and its file name.
    fn from_backups(workspace: &Path) -> StoreResult<(Project, String)> {
        let backups = workspace.join("backups");
        for backup in backup_files(&backups).into_iter().rev() {
            let raw = fs::read(&backup).io("read", &backup)?;
            if let Ok((project, _)) = decode(&raw) {
                let name = backup
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                return Ok((project, name));
            }
        }
        Err(domain(
            ErrorCode::FormatError,
            "project.json is missing or damaged and no valid backup exists",
        ))
    }

    /// Extracts `archive` into its workspace, replacing an earlier extraction. A crashed
    /// session's autosave, and every media file imported or recorded since the archive was
    /// written, survive the re-extraction so that recovery has its audio.
    fn extract(&self, archive: &Path) -> StoreResult<PathBuf> {
        let workspace = self.archive_workspace(archive);
        let mut keep = None;
        if workspace.join(".autosave").exists() {
            let root = &self.workspaces_root;
            fs::create_dir_all(root).io("create", root)?;
            // Kept on disk until the autosave is back, like the prototype's `mkdtemp`: if a
            // move fails, the autosave survives there (and is listed as a recoverable session).
            let dir = tempfile::Builder::new()
                .prefix("autosave-")
                .tempdir_in(root)
                .io("create a temporary directory in", root)?
                .keep();
            let from = workspace.join(".autosave");
            fs::rename(&from, dir.join(".autosave")).io("move", &from)?;
            let media = workspace.join("media");
            if media.exists() {
                fs::rename(&media, dir.join("media")).io("move", &media)?;
            }
            keep = Some(dir);
        }
        // ERR-06: `shutil.rmtree(workspace, ignore_errors=True)`; `create_dir` below fails if
        // anything is left.
        let _ = fs::remove_dir_all(&workspace);
        if let Some(parent) = workspace.parent() {
            fs::create_dir_all(parent).io("create", parent)?;
        }
        fs::create_dir(&workspace).io("create", &workspace)?;
        let extracted = safe_extract(archive, &workspace);
        if extracted.is_err() {
            // ERR-06: best-effort cleanup; the extraction error is what gets reported.
            let _ = fs::remove_dir_all(&workspace);
        }
        if let Some(dir) = &keep {
            restore_kept(dir, &workspace)?;
            // ERR-06: the prototype's `shutil.rmtree(keep, ignore_errors=True)`.
            let _ = fs::remove_dir_all(dir);
        }
        extracted?;
        let info = workspace.join("workspace.json");
        let data = json!({
            "kind": "extracted",
            "source": archive.to_string_lossy(),
            "created": self.now(),
        });
        atomic_write_bytes(&info, &dumps_compact(&data)).io("write", &info)?;
        Ok(workspace)
    }

    // -- integrity and cleanup ------------------------------------------------------------

    /// Checks the project's media against the workspace (existence, size and, with
    /// `check_hashes`, SHA-256), lists unreferenced media files and the routing issues.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] when a media file cannot be read.
    pub fn verify(
        &self,
        project: &Project,
        workspace: &Path,
        check_hashes: bool,
    ) -> StoreResult<IntegrityReport> {
        let mut report = IntegrityReport {
            ok: true,
            missing: Vec::new(),
            size_mismatch: Vec::new(),
            hash_mismatch: Vec::new(),
            unreferenced_files: Vec::new(),
            routing_issues: Vec::new(),
        };
        for asset in &project.assets {
            let path = asset_path(asset, workspace);
            if !path.exists() {
                report.missing.push(asset.id.clone());
                continue;
            }
            let size = fs::metadata(&path).io("read", &path)?.len();
            if size != asset.size_bytes {
                report.size_mismatch.push(asset.id.clone());
            } else if check_hashes
                && let Some(expected) = asset.sha256.as_deref().filter(|h| !h.is_empty())
                && sha256_file(&path).io("read", &path)? != expected
            {
                report.hash_mismatch.push(asset.id.clone());
            }
        }
        report.unreferenced_files = self
            .unused_media(project, workspace, std::iter::empty::<&str>())
            .iter()
            .map(|path| {
                path.strip_prefix(workspace)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        report.routing_issues = routing::issues(project)
            .into_iter()
            .map(|issue| issue.message)
            .collect();
        report.ok = report.missing.is_empty()
            && report.size_mismatch.is_empty()
            && report.hash_mismatch.is_empty()
            && report.routing_issues.is_empty();
        Ok(report)
    }

    /// Embedded media files under `workspace/media` that no asset refers to, sorted.
    /// `also_keep` holds asset paths that are still reachable, for example through the undo
    /// history.
    #[must_use]
    pub fn unused_media<S: AsRef<str>>(
        &self,
        project: &Project,
        workspace: &Path,
        also_keep: impl IntoIterator<Item = S>,
    ) -> Vec<PathBuf> {
        let media = workspace.join("media");
        if !media.exists() {
            return Vec::new();
        }
        let mut used: HashSet<String> = project
            .assets
            .iter()
            .filter(|a| a.location == MediaLocation::Embedded)
            .map(|a| posix_normal(&a.path))
            .collect();
        used.extend(also_keep.into_iter().map(|p| posix_normal(p.as_ref())));
        files_under(&media, false)
            .into_iter()
            .map(walkdir::DirEntry::into_path)
            .filter(|path| {
                path.is_file()
                    && !used.contains(&as_posix(path.strip_prefix(workspace).unwrap_or(path)))
            })
            .collect()
    }
}

/// Adds the file at `source` as member `name`.
fn add_file<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    source: &Path,
    name: &str,
    method: CompressionMethod,
    target: &Path,
) -> StoreResult<()> {
    let mut file = File::open(source).io("read", source)?;
    let metadata = file.metadata().io("read", source)?;
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(member_mode(&metadata))
        .last_modified_time(zip_time(modified))
        .large_file(needs_zip64(metadata.len()));
    zip.start_file(name, options).zip("write", target)?;
    io::copy(&mut file, zip).io("read", source)?;
    Ok(())
}

/// Moves a crashed session's autosave, and the media files the fresh extraction lacks, from
/// `keep` back into `workspace` (when the extraction left one).
fn restore_kept(keep: &Path, workspace: &Path) -> StoreResult<()> {
    if !workspace.exists() {
        return Ok(());
    }
    let from = keep.join(".autosave");
    fs::rename(&from, workspace.join(".autosave")).io("move", &from)?;
    let media = keep.join("media");
    if !media.exists() {
        return Ok(());
    }
    for entry in files_under(&media, false) {
        let source = entry.path();
        let dest = workspace.join(source.strip_prefix(keep).unwrap_or(source));
        if source.is_file() && !dest.exists() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).io("create", parent)?;
            }
            fs::rename(source, &dest).io("move", source)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{
        Member, as_posix, backup_stamp, iso_seconds, member_allowed, needs_zip64, posix_normal,
        py_path, py_strip, suspicious_ratio,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn timestamps_match_python_strftime() {
        // 2026-01-01T12:00:00Z plus 123456789 ns.
        let time = UNIX_EPOCH + Duration::new(1_767_268_800, 123_456_789);
        assert_eq!(iso_seconds(time), "2026-01-01T12:00:00Z");
        assert_eq!(backup_stamp(time), "20260101T120000123456Z");
    }

    #[test]
    fn member_names_follow_the_prototype() {
        for ok in [
            "project.json",
            "media/audio/a.wav",
            "./media/x",
            "media//a",
            "media/",
        ] {
            assert!(member_allowed(ok, 0), "{ok}");
        }
        for bad in [
            "",
            "../evil.txt",
            "/abs/evil.txt",
            "media/../../evil.txt",
            "unknown/top.txt",
            "media\\win.txt",
            "C:/windows.txt",
            "./",
            "backups/project.json",
        ] {
            assert!(!member_allowed(bad, 0), "{bad}");
        }
        assert!(!member_allowed("media/link", 0o120_777));
        assert!(member_allowed("media/file", 0o100_644));
    }

    #[test]
    fn compression_ratio_limits_only_large_members() {
        let member = |size, compressed| Member {
            name: String::new(),
            size,
            compressed,
            mode: 0,
        };
        assert!(suspicious_ratio(&member(201 << 20, 1 << 20)));
        assert!(!suspicious_ratio(&member(200 << 20, 1 << 20)));
        assert!(!suspicious_ratio(&member(60 << 20, 1)));
        assert!(!suspicious_ratio(&member(100 << 20, 0)));
    }

    #[test]
    fn small_helpers_match_python() {
        assert!(!needs_zip64(2_000_000_000));
        assert!(needs_zip64(2_100_000_000));
        assert_eq!(posix_normal("media//a/./b.wav"), "media/a/b.wav");
        assert_eq!(posix_normal("/abs/x"), "/abs/x");
        assert_eq!(as_posix(Path::new("media").join("a").as_path()), "media/a");
        assert_eq!(py_path(Path::new("./a//b/./c/")), PathBuf::from("a/b/c"));
        assert_eq!(py_path(Path::new(".")), PathBuf::from("."));
        assert_eq!(py_strip("autosave \x1f "), "autosave");
    }
}

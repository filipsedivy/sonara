//! User settings (`sonara.config.settings`), stored as JSON in the config directory. Unknown
//! keys are preserved.
//!
//! **Loading is lenient**, so that a hand-edited file cannot break startup: a missing,
//! unreadable or non-object file gives the defaults, and a value of the wrong type keeps its
//! field's default. Both are logged as warnings, as in the prototype.
//!
//! **Saving** writes the prototype's bytes: `json.dumps(settings, indent=2) + "\n"` (ASCII only,
//! Python float spelling), unknown keys first, then the known fields in declaration order. The
//! file is written atomically and, outside Windows, with mode `0600`.
//!
//! **Deviation (TYP-06).** The prototype accepts `null`, a string or a list of strings for every
//! field whose default is `None`. Here the optional text fields (`output_device`,
//! `input_device`, `ffmpeg_path`) accept `null` or a string, and `midi_inputs` accepts `null`
//! or a list of strings. Another value is ignored with a warning, like any wrong type: the
//! prototype would fail later, where the value is used.

use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use sonara_core::model::JsonMap;
use sonara_core::pyfmt::float_repr;
use sonara_project::atomic::atomic_write_bytes;

use crate::paths::{DirResult, config_dir};

/// The `version` the application writes.
pub const SETTINGS_VERSION: i64 = 1;
/// The most recent projects remembered.
pub const MAX_RECENT: usize = 15;
/// The settings file's name in the config directory.
pub const SETTINGS_FILE: &str = "settings.json";

/// Audio device settings.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSettings {
    /// `auto`, `native` or `null`; the transport validates it.
    pub backend: String,
    /// Output device name (`None`: the system default).
    pub output_device: Option<String>,
    /// Input device name (`None`: the system default).
    pub input_device: Option<String>,
    /// Device sample rate in Hz.
    pub sample_rate: i64,
    /// Device buffer size in frames.
    pub buffer_frames: i64,
    /// Render lookahead in milliseconds.
    pub lookahead_ms: f64,
    /// Render lookahead while live MIDI input is active (lower latency, less headroom).
    pub live_lookahead_ms: f64,
    /// MIDI inputs to open (`None`: all available).
    pub midi_inputs: Option<Vec<String>>,
    /// Output channels.
    pub output_channels: i64,
    /// Input channels.
    pub input_channels: i64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            backend: "auto".to_owned(),
            output_device: None,
            input_device: None,
            sample_rate: 48_000,
            buffer_frames: 256,
            lookahead_ms: 80.0,
            live_lookahead_ms: 25.0,
            midi_inputs: None,
            output_channels: 2,
            input_channels: 2,
        }
    }
}

/// The user settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The file format version.
    pub version: i64,
    /// Audio device settings.
    pub audio: AudioSettings,
    /// Seconds between autosaves.
    pub autosave_seconds: i64,
    /// Backups of `project.json` kept per bundle (0 or less: none).
    pub backup_retention: i64,
    /// Undo steps kept (0 or less: none).
    pub undo_limit: i64,
    /// GUI theme name.
    pub theme: String,
    /// Recently opened or saved projects, newest first.
    pub recent_projects: Vec<String>,
    /// Extra locations agents may read and write.
    pub agent_allowed_paths: Vec<String>,
    /// Whether destructive agent actions need a confirmation token.
    pub agent_require_confirmation: bool,
    /// An explicit `ffmpeg` executable.
    pub ffmpeg_path: Option<String>,
    /// Unknown keys of the file, written back unchanged.
    pub extra: JsonMap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            audio: AudioSettings::default(),
            autosave_seconds: 60,
            backup_retention: 10,
            undo_limit: 500,
            theme: "dark".to_owned(),
            recent_projects: Vec::new(),
            agent_allowed_paths: Vec::new(),
            agent_require_confirmation: true,
            ffmpeg_path: None,
            extra: JsonMap::new(),
        }
    }
}

impl Settings {
    /// Puts `path` first in the recent projects, without duplicates, keeping [`MAX_RECENT`].
    pub fn add_recent(&mut self, path: &Path) {
        let path = path.to_string_lossy().into_owned();
        let mut recent = Vec::with_capacity(MAX_RECENT);
        recent.push(path.clone());
        recent.extend(self.recent_projects.drain(..).filter(|p| *p != path));
        recent.truncate(MAX_RECENT);
        self.recent_projects = recent;
    }

    /// The file's JSON object: the unknown keys first, then the fields in declaration order (a
    /// known field wins over an unknown key of the same name, at that key's position).
    #[must_use]
    pub fn to_json(&self) -> Value {
        let a = &self.audio;
        let audio = json!({
            "backend": a.backend,
            "output_device": a.output_device,
            "input_device": a.input_device,
            "sample_rate": a.sample_rate,
            "buffer_frames": a.buffer_frames,
            "lookahead_ms": a.lookahead_ms,
            "live_lookahead_ms": a.live_lookahead_ms,
            "midi_inputs": a.midi_inputs,
            "output_channels": a.output_channels,
            "input_channels": a.input_channels,
        });
        let fields = [
            ("version", json!(self.version)),
            ("audio", audio),
            ("autosave_seconds", json!(self.autosave_seconds)),
            ("backup_retention", json!(self.backup_retention)),
            ("undo_limit", json!(self.undo_limit)),
            ("theme", json!(self.theme)),
            ("recent_projects", json!(self.recent_projects)),
            ("agent_allowed_paths", json!(self.agent_allowed_paths)),
            (
                "agent_require_confirmation",
                json!(self.agent_require_confirmation),
            ),
            ("ffmpeg_path", json!(self.ffmpeg_path)),
        ];
        let mut out = self.extra.clone();
        for (key, value) in fields {
            out.insert(key.to_owned(), value);
        }
        Value::Object(out)
    }
}

/// The settings file: `settings.json` in the config directory.
///
/// # Errors
///
/// [`crate::paths::NoHomeDirectory`] when the config directory is unknown.
pub fn settings_path() -> DirResult {
    Ok(config_dir()?.join(SETTINGS_FILE))
}

/// The settings in `path`: the defaults when the file is missing, unreadable or not a JSON
/// object, and the default of every field whose value has the wrong type (each logged as a
/// warning). Keys the application does not know are kept in [`Settings::extra`].
#[must_use]
pub fn load_settings(path: &Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let raw = match fs::read_to_string(path)
        .map_err(|err| err.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|err| err.to_string()))
    {
        Ok(raw) => raw,
        Err(reason) => {
            log::warn!(
                "settings file {} unreadable ({reason}); using defaults",
                path.display()
            );
            return Settings::default();
        }
    };
    let Value::Object(raw) = raw else {
        log::warn!(
            "settings file {} is not a JSON object; using defaults",
            path.display()
        );
        return Settings::default();
    };
    from_object(&raw)
}

/// Settings from a parsed file.
fn from_object(raw: &JsonMap) -> Settings {
    let mut s = Settings::default();
    if let Some(Value::Object(audio)) = raw.get("audio") {
        let a = &mut s.audio;
        read(audio, "backend", &mut a.backend, string);
        read_optional(audio, "output_device", &mut a.output_device, string);
        read_optional(audio, "input_device", &mut a.input_device, string);
        read(audio, "sample_rate", &mut a.sample_rate, int);
        read(audio, "buffer_frames", &mut a.buffer_frames, int);
        read(audio, "lookahead_ms", &mut a.lookahead_ms, float);
        read(audio, "live_lookahead_ms", &mut a.live_lookahead_ms, float);
        read_optional(audio, "midi_inputs", &mut a.midi_inputs, strings);
        read(audio, "output_channels", &mut a.output_channels, int);
        read(audio, "input_channels", &mut a.input_channels, int);
    }
    read(raw, "version", &mut s.version, int);
    read(raw, "autosave_seconds", &mut s.autosave_seconds, int);
    read(raw, "backup_retention", &mut s.backup_retention, int);
    read(raw, "undo_limit", &mut s.undo_limit, int);
    read(raw, "theme", &mut s.theme, string);
    read(raw, "recent_projects", &mut s.recent_projects, strings);
    read(
        raw,
        "agent_allowed_paths",
        &mut s.agent_allowed_paths,
        strings,
    );
    read(
        raw,
        "agent_require_confirmation",
        &mut s.agent_require_confirmation,
        boolean,
    );
    read_optional(raw, "ffmpeg_path", &mut s.ffmpeg_path, string);
    s.extra = raw
        .iter()
        .filter(|(key, _)| !KNOWN_FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    s
}

/// The fields of [`Settings`] by their file names; other keys are [`Settings::extra`].
const KNOWN_FIELDS: [&str; 11] = [
    "version",
    "audio",
    "autosave_seconds",
    "backup_retention",
    "undo_limit",
    "theme",
    "recent_projects",
    "agent_allowed_paths",
    "agent_require_confirmation",
    "ffmpeg_path",
    "extra",
];

/// Sets `target` from `raw[name]` when the value converts, else warns and keeps the default.
fn read<T>(raw: &JsonMap, name: &str, target: &mut T, convert: fn(&Value) -> Option<T>) {
    let Some(value) = raw.get(name) else {
        return;
    };
    match convert(value) {
        Some(converted) => *target = converted,
        None => log::warn!("ignoring setting {name}={value} (wrong type)"),
    }
}

/// [`read`] for a field whose default is `None`: `null` is accepted too.
fn read_optional<T>(
    raw: &JsonMap,
    name: &str,
    target: &mut Option<T>,
    convert: fn(&Value) -> Option<T>,
) {
    match raw.get(name) {
        None => {}
        Some(Value::Null) => *target = None,
        Some(value) => match convert(value) {
            Some(converted) => *target = Some(converted),
            None => log::warn!("ignoring setting {name}={value} (wrong type)"),
        },
    }
}

/// An integer, not a boolean and not a float (Python's `isinstance(v, int)`).
fn int(value: &Value) -> Option<i64> {
    value.as_i64()
}

/// An integer or a float, as a float.
fn float(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn boolean(value: &Value) -> Option<bool> {
    value.as_bool()
}

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn strings(value: &Value) -> Option<Vec<String>> {
    value.as_array()?.iter().map(string).collect()
}

/// Writes `settings` to `path` atomically, readable only by its owner outside Windows.
///
/// # Errors
///
/// The I/O error of writing the file or changing its mode.
pub fn save_settings(settings: &Settings, path: &Path) -> io::Result<()> {
    let mut text = String::with_capacity(1024);
    write_json(&mut text, &settings.to_json(), 0);
    text.push('\n');
    atomic_write_bytes(path, text.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// The settings file the context loads when none are given: `override_path`, else
/// [`settings_path`].
///
/// # Errors
///
/// [`crate::paths::NoHomeDirectory`] without an override and a known config directory.
pub(crate) fn settings_file(override_path: Option<&Path>) -> DirResult {
    override_path.map_or_else(settings_path, |path| Ok(PathBuf::from(path)))
}

// -- Python's json.dumps(value, indent=2) ---------------------------------------------------

/// Appends `value` as `json.dumps(value, indent=2)` writes it at nesting `level`.
fn write_json(out: &mut String, value: &Value, level: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => match (number.as_i64(), number.as_u64(), number.as_f64()) {
            (Some(int), _, _) => out.push_str(&int.to_string()),
            (None, Some(uint), _) => out.push_str(&uint.to_string()),
            (None, None, Some(float)) => out.push_str(&float_repr(float)),
            (None, None, None) => out.push_str("null"),
        },
        Value::String(text) => write_string(out, text),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write_json(out, item, level + 1);
            }
            newline(out, level);
            out.push(']');
        }
        Value::Object(map) => write_object(out, map, level),
    }
}

fn write_object(out: &mut String, map: &Map<String, Value>, level: usize) {
    if map.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push('{');
    for (i, (key, item)) in map.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        newline(out, level + 1);
        write_string(out, key);
        out.push_str(": ");
        write_json(out, item, level + 1);
    }
    newline(out, level);
    out.push('}');
}

fn newline(out: &mut String, level: usize) {
    out.push('\n');
    for _ in 0..level {
        out.push_str("  ");
    }
}

/// A JSON string with Python's `ensure_ascii` escapes: the short forms for `"`, `\`, and
/// `\b \f \n \r \t`, and `\uXXXX` (lowercase, surrogate pairs above the BMP) for every other
/// character outside `' '..='~'`.
fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='~' => out.push(ch),
            _ => {
                let mut units = [0_u16; 2];
                for unit in ch.encode_utf16(&mut units) {
                    // `fmt::Write` for `String` never fails.
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::write_json;

    #[test]
    fn indented_json_matches_python() {
        // `json.dumps(value, indent=2)` in Python 3.13.
        let value = json!({"a": [1, 1e16, 2.0, -0.5], "e": [], "o": {}, "s": "q\"\\\n\u{1}\u{7f}é😀",
                           "n": {"x": null, "t": true}});
        let mut out = String::new();
        write_json(&mut out, &value, 0);
        assert_eq!(
            out,
            "{\n  \"a\": [\n    1,\n    1e+16,\n    2.0,\n    -0.5\n  ],\n  \"e\": [],\n  \"o\": {},\n  \
             \"s\": \"q\\\"\\\\\\n\\u0001\\u007f\\u00e9\\ud83d\\ude00\",\n  \"n\": {\n    \"x\": null,\n    \
             \"t\": true\n  }\n}"
        );
    }
}

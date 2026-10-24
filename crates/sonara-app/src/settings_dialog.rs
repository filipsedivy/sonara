//! The Settings dialog of an interface: what it
//! shows ([`SettingsDialogView`]), the choices it sends back ([`SettingsForm`], validated here,
//! TYP-06), and the save ([`apply_and_save`]).
//!
//! The dialog edits the keys of `settings.json` that the prototype's `SettingsDialog` edits, and
//! the file is written by [`save_settings`], so the bytes equal the prototype's for the same
//! choices (CTR-01; `crates/sonara-app/tests/gui_dialogs.rs` compares them with the prototype's
//! files). The other keys, the unknown ones included, are kept.
//!
//! **As the prototype's widgets do:** a stored number outside a field's range is shown clamped
//! to it, and the lookahead is held with two decimals (a `QDoubleSpinBox`), so confirming the
//! dialog writes the clamped, rounded values. The allowed folders are one per line; blank lines
//! are dropped and each line is stripped (`str.splitlines`, `str.strip`).
//!
//! **G11, fixed.** The prototype replaced a stored value that is none of a
//! combo's choices (a buffer of 480 frames, an unknown backend, theme or device) with the first
//! choice when the dialog was confirmed. Here such a value is offered as a choice of its own,
//! shown as it is, and kept unless the user picks another.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sonara_core::error::DomainError;

use crate::context::AppContext;
use crate::error::{AppResult, IoContext as _};
use crate::settings::{Settings, save_settings};
use crate::transport::native_devices;

/// The engine choices: label and `audio.backend`.
pub const BACKENDS: [(&str, &str); 4] = [
    ("Automatic", "auto"),
    ("System audio (native)", "native"),
    ("Simulated device (testing)", "native-null"),
    ("Off (offline only)", "null"),
];
/// The device buffer choices, in frames.
pub const BUFFERS: [i64; 6] = [64, 128, 256, 512, 1024, 2048];
/// The theme choices: label and `theme`.
pub const THEMES: [(&str, &str); 2] = [("Dark", "dark"), ("Light", "light")];
/// The render lookahead's range in milliseconds.
pub const LOOKAHEAD_MS: (f64, f64) = (20.0, 1000.0);
/// The autosave interval's range in seconds; 0 is "off".
pub const AUTOSAVE_SECONDS: (i64, i64) = (0, 3600);
/// The range of the backups kept per project.
pub const BACKUP_RETENTION: (i64, i64) = (0, 100);
/// The range of the undo steps kept.
pub const UNDO_LIMIT: (i64, i64) = (10, 10_000);
/// The label of a device combo's first entry, which saves `null`.
pub const SYSTEM_DEFAULT: &str = "System default";

/// One entry of a combo: what it shows and the value it saves.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice<T> {
    /// The text.
    pub label: String,
    /// The saved value.
    pub value: T,
}

impl<T> Choice<T> {
    fn new(label: impl Into<String>, value: T) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

/// The dialog's fields, as it shows them and sends them back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsForm {
    /// `audio.backend`.
    pub backend: String,
    /// `audio.output_device`; `None` for the system default.
    pub output_device: Option<String>,
    /// `audio.input_device`; `None` for the system default.
    pub input_device: Option<String>,
    /// `audio.buffer_frames`.
    pub buffer_frames: i64,
    /// `audio.lookahead_ms`.
    pub lookahead_ms: f64,
    /// `autosave_seconds`; 0 is off.
    pub autosave_seconds: i64,
    /// `backup_retention`.
    pub backup_retention: i64,
    /// `undo_limit`.
    pub undo_limit: i64,
    /// `theme`.
    pub theme: String,
    /// `agent_require_confirmation`.
    pub agent_require_confirmation: bool,
    /// `agent_allowed_paths`, one per line.
    pub agent_allowed_paths: String,
}

impl SettingsForm {
    /// The fields as the dialog opens on `settings`: numbers clamped to their ranges and the
    /// lookahead held with two decimals, as the prototype's spin boxes hold them.
    #[must_use]
    pub fn of(settings: &Settings) -> Self {
        let audio = &settings.audio;
        Self {
            backend: audio.backend.clone(),
            output_device: audio.output_device.clone(),
            input_device: audio.input_device.clone(),
            buffer_frames: audio.buffer_frames,
            lookahead_ms: spin_value(audio.lookahead_ms, LOOKAHEAD_MS),
            autosave_seconds: clamp_to(settings.autosave_seconds, AUTOSAVE_SECONDS),
            backup_retention: clamp_to(settings.backup_retention, BACKUP_RETENTION),
            undo_limit: clamp_to(settings.undo_limit, UNDO_LIMIT),
            theme: settings.theme.clone(),
            agent_require_confirmation: settings.agent_require_confirmation,
            agent_allowed_paths: settings.agent_allowed_paths.join("\n"),
        }
    }

    /// Writes the fields into `settings` (the prototype's `SettingsDialog.apply`), after
    /// checking them against what the dialog offers for `settings`: nothing changes unless
    /// every field is valid. Numbers are clamped to their ranges, as the spin boxes clamp them.
    ///
    /// # Errors
    ///
    /// `INVALID_ARGUMENT` for a backend, buffer or theme that is neither a choice nor the
    /// stored value, or a lookahead that is not a number.
    pub fn apply(&self, settings: &mut Settings) -> Result<(), DomainError> {
        let audio = &settings.audio;
        offered(
            "engine",
            &self.backend,
            BACKENDS.iter().any(|(_, value)| *value == self.backend)
                || self.backend == audio.backend,
        )?;
        offered(
            "device buffer",
            &self.buffer_frames.to_string(),
            BUFFERS.contains(&self.buffer_frames) || self.buffer_frames == audio.buffer_frames,
        )?;
        offered(
            "theme",
            &self.theme,
            THEMES.iter().any(|(_, value)| *value == self.theme) || self.theme == settings.theme,
        )?;
        if !self.lookahead_ms.is_finite() {
            return Err(DomainError::invalid(
                "the render lookahead must be a number of milliseconds",
            ));
        }
        let audio = &mut settings.audio;
        audio.backend.clone_from(&self.backend);
        audio.output_device.clone_from(&self.output_device);
        audio.input_device.clone_from(&self.input_device);
        audio.buffer_frames = self.buffer_frames;
        audio.lookahead_ms = spin_value(self.lookahead_ms, LOOKAHEAD_MS);
        settings.autosave_seconds = clamp_to(self.autosave_seconds, AUTOSAVE_SECONDS);
        settings.backup_retention = clamp_to(self.backup_retention, BACKUP_RETENTION);
        settings.undo_limit = clamp_to(self.undo_limit, UNDO_LIMIT);
        settings.theme.clone_from(&self.theme);
        settings.agent_require_confirmation = self.agent_require_confirmation;
        settings.agent_allowed_paths = allowed_paths(&self.agent_allowed_paths);
        Ok(())
    }
}

/// `INVALID_ARGUMENT` unless `ok`.
fn offered(field: &str, value: &str, ok: bool) -> Result<(), DomainError> {
    if ok {
        Ok(())
    } else {
        Err(DomainError::invalid(format!(
            "{value:?} is not a choice of the {field}"
        )))
    }
}

/// `value` clamped to `range`, as a `QSpinBox` holds it.
fn clamp_to(value: i64, (low, high): (i64, i64)) -> i64 {
    value.clamp(low, high)
}

/// `value` as a `QDoubleSpinBox` with two decimals holds it: clamped to `range`, then rounded to
/// two decimals on its exact binary value, an exact tie away from zero (Qt's `QString::number(v,
/// 'f', 2)`: 37.555 is 37.55, 80.125 is 80.13).
fn spin_value(value: f64, (low, high): (f64, f64)) -> f64 {
    if !value.is_finite() {
        return low;
    }
    qt_round(value.clamp(low, high), 2)
}

/// `value` rounded to `decimals` as Qt prints it: on the exact decimal expansion of the binary
/// value, a tie away from zero.
pub(crate) fn qt_round(value: f64, decimals: usize) -> f64 {
    // 80 digits hold every fraction of a double of magnitude 2^-28 or more exactly.
    let exact = format!("{:.80}", value.abs());
    let Some(point) = exact.find('.') else {
        return value;
    };
    let keep = point + 1 + decimals;
    let up = exact
        .as_bytes()
        .get(keep)
        .is_some_and(|digit| *digit >= b'5');
    let truncated: f64 = exact
        .get(..keep)
        .and_then(|t| t.parse().ok())
        .unwrap_or(value.abs());
    let step = 10_f64.powi(i32::try_from(decimals).unwrap_or(0));
    let rounded = if up {
        // Through the text again, so that the result is the double nearest the decimal.
        let units = (truncated * step).round() + 1.0;
        format!("{:.*}", decimals, units / step)
            .parse()
            .unwrap_or(truncated)
    } else {
        truncated
    };
    rounded.copysign(value)
}

/// `[line.strip() for line in text.splitlines() if line.strip()]`.
fn allowed_paths(text: &str) -> Vec<String> {
    py_splitlines(text)
        .into_iter()
        .map(py_strip)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Python's `str.splitlines()`: lines broken at `\n`, `\r`, `\r\n`, `\v`, `\f`, `\x1c`–`\x1e`,
/// `\x85`, ` ` and ` `, without the breaks.
fn py_splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, ch)) = chars.next() {
        let breaks = matches!(
            ch,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !breaks {
            continue;
        }
        lines.push(text.get(start..at).unwrap_or_default());
        let mut next = at + ch.len_utf8();
        if ch == '\r' && chars.peek().is_some_and(|&(_, c)| c == '\n') {
            chars.next();
            next += 1;
        }
        start = next;
    }
    if start < text.len() {
        lines.push(text.get(start..).unwrap_or_default());
    }
    lines
}

/// Python's `str.strip()`: whitespace as `str.isspace` defines it removed from both ends.
fn py_strip(line: &str) -> &str {
    line.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// The audio devices a device combo offers.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct DeviceListing {
    /// The playback devices: name (with " (default)" for the system's) and id.
    pub outputs: Vec<Choice<String>>,
    /// The capture devices.
    pub inputs: Vec<Choice<String>>,
    /// Why the devices could not be listed ("Devices unavailable: …"), if so.
    pub error: Option<String>,
}

impl DeviceListing {
    /// The devices of the native backend (`create_backend("native").devices()`), as
    /// `sonara devices` lists them; opening the system's audio context may take a moment.
    #[must_use]
    pub fn native() -> Self {
        match native_devices() {
            Ok(listing) => Self::from_json(&listing),
            Err(error) => Self {
                error: Some(format!("Devices unavailable: {error}")),
                ..Self::default()
            },
        }
    }

    /// The listing of `sonara devices`' JSON (`{"devices": [{"id", "name", "kind",
    /// "default"}]}`).
    #[must_use]
    pub fn from_json(listing: &Value) -> Self {
        let mut out = Self::default();
        for device in listing["devices"].as_array().into_iter().flatten() {
            let (Some(id), Some(name)) = (device["id"].as_str(), device["name"].as_str()) else {
                continue;
            };
            let label = if device["default"].as_bool() == Some(true) {
                format!("{name} (default)")
            } else {
                name.to_owned()
            };
            let choice = Choice::new(label, id.to_owned());
            if device["kind"].as_str() == Some("playback") {
                out.outputs.push(choice);
            } else {
                out.inputs.push(choice);
            }
        }
        out
    }
}

/// What the Settings dialog shows: its fields and the choices of its combos and ranges.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingsDialogView {
    /// The fields as the dialog opens.
    pub form: SettingsForm,
    /// The engine's choices; a stored backend that is none of them comes last.
    pub backends: Vec<Choice<String>>,
    /// The output devices: "System default" (`null`), the listed devices, and a stored device
    /// that is not listed.
    pub outputs: Vec<Choice<Option<String>>>,
    /// The input devices, as `outputs`.
    pub inputs: Vec<Choice<Option<String>>>,
    /// Why the devices could not be listed, if so.
    pub devices_error: Option<String>,
    /// The buffer's choices ("256 frames"); a stored size that is none of them is added in
    /// order.
    pub buffers: Vec<Choice<i64>>,
    /// The theme's choices; a stored theme that is none of them comes last.
    pub themes: Vec<Choice<String>>,
    /// The lookahead's range in milliseconds.
    pub lookahead_ms: (f64, f64),
    /// The autosave interval's range in seconds (0 shows "off").
    pub autosave_seconds: (i64, i64),
    /// The range of the backups kept.
    pub backup_retention: (i64, i64),
    /// The range of the undo steps.
    pub undo_limit: (i64, i64),
}

impl SettingsDialogView {
    /// The dialog for `settings`, offering the devices of `devices`.
    #[must_use]
    pub fn new(settings: &Settings, devices: DeviceListing) -> Self {
        let form = SettingsForm::of(settings);
        let mut backends: Vec<Choice<String>> = BACKENDS
            .iter()
            .map(|(label, value)| Choice::new(*label, (*value).to_owned()))
            .collect();
        keep_stored(&mut backends, &form.backend, || {
            Choice::new(form.backend.clone(), form.backend.clone())
        });
        let mut themes: Vec<Choice<String>> = THEMES
            .iter()
            .map(|(label, value)| Choice::new(*label, (*value).to_owned()))
            .collect();
        keep_stored(&mut themes, &form.theme, || {
            Choice::new(form.theme.clone(), form.theme.clone())
        });
        let mut sizes = BUFFERS.to_vec();
        if !sizes.contains(&form.buffer_frames) {
            sizes.push(form.buffer_frames);
            sizes.sort_unstable();
        }
        let buffers = sizes
            .into_iter()
            .map(|frames| Choice::new(format!("{frames} frames"), frames))
            .collect();
        Self {
            outputs: device_choices(devices.outputs, form.output_device.as_deref()),
            inputs: device_choices(devices.inputs, form.input_device.as_deref()),
            devices_error: devices.error,
            form,
            backends,
            buffers,
            themes,
            lookahead_ms: LOOKAHEAD_MS,
            autosave_seconds: AUTOSAVE_SECONDS,
            backup_retention: BACKUP_RETENTION,
            undo_limit: UNDO_LIMIT,
        }
    }
}

/// Adds the stored value's choice when none of `choices` saves it.
fn keep_stored<F>(choices: &mut Vec<Choice<String>>, stored: &str, choice: F)
where
    F: FnOnce() -> Choice<String>,
{
    if !choices.iter().any(|c| c.value == stored) {
        choices.push(choice());
    }
}

/// "System default", the listed devices, and the stored device when it is not listed.
fn device_choices(
    listed: Vec<Choice<String>>,
    stored: Option<&str>,
) -> Vec<Choice<Option<String>>> {
    let mut out = vec![Choice::new(SYSTEM_DEFAULT, None)];
    let shown = stored.is_none_or(|id| listed.iter().any(|device| device.value == id));
    out.extend(
        listed
            .into_iter()
            .map(|device| Choice::new(device.label, Some(device.value))),
    );
    if let (false, Some(id)) = (shown, stored) {
        out.push(Choice::new(
            format!("{id} (not connected)"),
            Some(id.to_owned()),
        ));
    }
    out
}

/// What saving the dialog changed that the interface applies at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SavedSettings {
    /// The autosave interval: the timer restarts with it, or stops at 0.
    pub autosave_seconds: i64,
}

/// The Settings dialog's OK: `form` is checked and written into the application's
/// settings, and the settings are saved to the file the application persists them to (in
/// memory only when it persists none). The engine, the undo limit and the backups apply at the
/// next start, as in the prototype; the caller restarts the autosave timer.
///
/// # Errors
///
/// - the errors of [`SettingsForm::apply`] (nothing changes then);
/// - [`crate::error::AppError::Io`] when the file cannot be written; the settings in memory are
///   changed already, as the prototype's were.
pub fn apply_and_save(app: &AppContext, form: &SettingsForm) -> AppResult<SavedSettings> {
    let saved = {
        let mut settings = app.settings();
        form.apply(&mut settings)?;
        settings.clone()
    };
    if let Some(path) = app.projects().settings_file() {
        save_settings(&saved, path).io("save the settings to", path)?;
    }
    Ok(SavedSettings {
        autosave_seconds: saved.autosave_seconds,
    })
}

/// The dialog for the application's settings, with the native backend's devices.
#[must_use]
pub fn dialog_view(app: &AppContext) -> SettingsDialogView {
    let settings = app.settings().clone();
    SettingsDialogView::new(&settings, DeviceListing::native())
}

#[cfg(test)]
mod tests {
    use super::{allowed_paths, py_splitlines, spin_value};

    #[test]
    fn lines_split_and_strip_as_python() {
        assert_eq!(
            py_splitlines("a\r\nb\rc\u{2028}d\n"),
            vec!["a", "b", "c", "d"]
        );
        assert_eq!(py_splitlines(""), Vec::<&str>::new());
        assert_eq!(py_splitlines("\n\n"), vec!["", ""]);
        assert_eq!(
            allowed_paths("  /a  \n\n\t/b\u{1f}\n   \n~/c"),
            vec!["/a", "/b", "~/c"]
        );
    }

    #[test]
    fn the_lookahead_is_held_as_a_two_decimal_spin_box() {
        assert!((spin_value(37.555, (20.0, 1000.0)) - 37.55).abs() < 1e-12);
        // Exact ties go away from zero, as Qt's (measured on PySide6 6.12).
        assert!((spin_value(80.125, (20.0, 1000.0)) - 80.13).abs() < 1e-12);
        assert!((spin_value(50.375, (20.0, 1000.0)) - 50.38).abs() < 1e-12);
        assert!((spin_value(20.005, (20.0, 1000.0)) - 20.0).abs() < 1e-12);
        assert!((spin_value(5.0, (20.0, 1000.0)) - 20.0).abs() < 1e-12);
        assert!((spin_value(f64::NAN, (20.0, 1000.0)) - 20.0).abs() < 1e-12);
    }
}

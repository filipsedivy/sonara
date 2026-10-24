//! The saved layout of the main window (the prototype's `window.ini`).
//!
//! The prototype kept Qt's window geometry and dock state in `<config>/window.ini`. The desktop
//! application keeps the window's place and size, in logical pixels, in `<config>/window.json`
//! ([`window_layout_path`]), next to the settings, so that `SONARA_HOME` moves it with them.
//! The file is no contract. It also holds the panes
//! ([`PaneLayout`]): the size of the Browser, the Inspector and the Editors, and whether each
//! shows. A file without them gives the prototype's sizes.
//!
//! QG-13: `tauri-plugin-window-state` stores the geometry in the platform's application config
//! folder, which ignores `SONARA_HOME`, and it cannot hold the pane sizes, so the layout is
//! written here.
//!
//! A missing, unreadable or invalid file gives no layout: the window then opens at its default
//! size, centred. A saved place that no display shows any more is not restored either
//! ([`WindowLayout::is_visible_on`]).
//!
//! The mixer's own window (owner, 2026-10-24) has its place, its size and
//! whether it was open in the same file, under the key `mixer` ([`MixerWindowLayout`]), as the
//! prototype's `window.ini` held a floating Editors dock with the main window's state, which
//! `restoreState` opened again at startup. Saving the main window's layout keeps that key, and
//! saving the mixer's keeps the rest of the file.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sonara_project::atomic::atomic_write_bytes;

use crate::paths::{DirResult, config_dir};

/// The side, in logical pixels, of the part of the window's top strip that must lie on a
/// display for the saved place to be restored ([`WindowLayout::is_visible_on`]).
pub const VISIBLE_MARGIN: f64 = 48.0;

/// Where the main window was and how large, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowLayout {
    /// The left edge of the window's outer frame.
    pub x: f64,
    /// The top edge of the window's outer frame.
    pub y: f64,
    /// The width of the window's content.
    pub width: f64,
    /// The height of the window's content.
    pub height: f64,
    /// Whether the window was maximized (zoomed).
    #[serde(default)]
    pub maximized: bool,
    /// The panes.
    #[serde(default)]
    pub panes: PaneLayout,
}

/// Where the mixer's own window was and how large, in logical pixels, and whether it was open
/// when Sonara quit (the `mixer` key of `window.json`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MixerWindowLayout {
    /// The left edge of the window's outer frame.
    pub x: f64,
    /// The top edge of the window's outer frame.
    pub y: f64,
    /// The width of the window's content.
    pub width: f64,
    /// The height of the window's content.
    pub height: f64,
    /// Whether the window was open when Sonara quit: it opens again at startup.
    #[serde(default)]
    pub open: bool,
}

impl MixerWindowLayout {
    /// Whether every value is finite and the size is positive.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        valid_frame(self.x, self.y, self.width, self.height)
    }

    /// Whether the window's top strip lies on one of `screens`, as for the main window
    /// ([`WindowLayout::is_visible_on`]).
    #[must_use]
    pub fn is_visible_on(&self, screens: &[ScreenArea]) -> bool {
        top_strip_visible(self.x, self.y, self.width, screens)
    }
}

/// One pane of the main window: its size and whether it shows.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pane {
    /// The width of a side pane, or the height of the Editors, in logical pixels.
    pub size: f64,
    /// Whether the pane shows (View ▸ Browser, Inspector, Editors).
    pub open: bool,
}

/// The main window's panes: the Browser on the left, the Inspector on the right and the
/// Editors (Mixer, Piano Roll, Devices) below the timeline. The defaults are the prototype's
/// dock sizes: 240 and 280 pixels wide, and the Editors 300 pixels high, all showing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneLayout {
    /// The Browser, left.
    pub browser: Pane,
    /// The Inspector, right.
    pub inspector: Pane,
    /// The Editors, below the timeline.
    pub editors: Pane,
}

impl Default for PaneLayout {
    fn default() -> Self {
        Self {
            browser: Pane {
                size: 240.0,
                open: true,
            },
            inspector: Pane {
                size: 280.0,
                open: true,
            },
            editors: Pane {
                size: 300.0,
                open: true,
            },
        }
    }
}

impl PaneLayout {
    /// The layout with every size that is not finite and positive replaced by its default.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let defaults = Self::default();
        let fix = |pane: Pane, default: Pane| Pane {
            size: if pane.size.is_finite() && pane.size > 0.0 {
                pane.size
            } else {
                default.size
            },
            open: pane.open,
        };
        Self {
            browser: fix(self.browser, defaults.browser),
            inspector: fix(self.inspector, defaults.inspector),
            editors: fix(self.editors, defaults.editors),
        }
    }
}

/// A display's area, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenArea {
    /// The left edge.
    pub x: f64,
    /// The top edge.
    pub y: f64,
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

/// Whether every value is finite and the size is positive.
fn valid_frame(x: f64, y: f64, width: f64, height: f64) -> bool {
    [x, y, width, height].iter().all(|value| value.is_finite()) && width > 0.0 && height > 0.0
}

/// Whether at least [`VISIBLE_MARGIN`] × [`VISIBLE_MARGIN`] pixels of the top strip of a window
/// at `x`, `y`, `width` wide, lie on one of `screens`.
fn top_strip_visible(x: f64, y: f64, width: f64, screens: &[ScreenArea]) -> bool {
    screens.iter().any(|screen| {
        let left = x.max(screen.x);
        let right = (x + width).min(screen.x + screen.width);
        let top = y.max(screen.y);
        let bottom = (y + VISIBLE_MARGIN).min(screen.y + screen.height);
        right - left >= VISIBLE_MARGIN && bottom - top >= VISIBLE_MARGIN
    })
}

impl WindowLayout {
    /// Whether every value is finite and the size is positive.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        valid_frame(self.x, self.y, self.width, self.height)
    }

    /// Whether at least [`VISIBLE_MARGIN`] × [`VISIBLE_MARGIN`] pixels of the window's top
    /// strip, where its title bar is, lie on one of `screens`, so that it can be grabbed.
    #[must_use]
    pub fn is_visible_on(&self, screens: &[ScreenArea]) -> bool {
        top_strip_visible(self.x, self.y, self.width, screens)
    }
}

/// `<config>/window.json`.
///
/// # Errors
///
/// [`crate::paths::NoHomeDirectory`], as [`config_dir`].
pub fn window_layout_path() -> DirResult {
    Ok(config_dir()?.join("window.json"))
}

/// The layout saved at `path`, or `None` when there is none, or it cannot be read or is
/// invalid (the reason is logged).
#[must_use]
pub fn load_window_layout(path: &Path) -> Option<WindowLayout> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::warn!("cannot read the window layout {}: {error}", path.display());
            return None;
        }
    };
    match serde_json::from_str::<WindowLayout>(&text) {
        Ok(layout) if layout.is_valid() => Some(WindowLayout {
            panes: layout.panes.sanitized(),
            ..layout
        }),
        Ok(_) => {
            log::warn!("ignoring the invalid window layout {}", path.display());
            None
        }
        Err(error) => {
            log::warn!("ignoring the window layout {}: {error}", path.display());
            None
        }
    }
}

/// The key of the mixer window's layout in `window.json`.
const MIXER_KEY: &str = "mixer";

/// The JSON object saved at `path`; empty when there is none or it is not an object.
fn saved_object(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| match value {
            serde_json::Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

/// Writes `object` to `path` atomically, pretty-printed with a final newline, creating its folder.
fn write_object(
    object: &serde_json::Map<String, serde_json::Value>,
    path: &Path,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(object).map_err(io::Error::other)?;
    text.push('\n');
    atomic_write_bytes(path, text.as_bytes())
}

/// Writes `layout` to `path` atomically, creating its folder; the mixer window's layout saved
/// there stays.
///
/// # Errors
///
/// The I/O error of creating the folder or writing the file.
pub fn save_window_layout(layout: &WindowLayout, path: &Path) -> io::Result<()> {
    let mixer = saved_object(path).remove(MIXER_KEY);
    let serde_json::Value::Object(mut object) =
        serde_json::to_value(layout).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("a window layout is a JSON object"));
    };
    if let Some(mixer) = mixer {
        object.insert(MIXER_KEY.to_owned(), mixer);
    }
    write_object(&object, path)
}

/// The mixer window's layout saved at `path` (the `mixer` key), or `None` when there is none or
/// it is invalid (an invalid one is logged).
#[must_use]
pub fn load_mixer_window_layout(path: &Path) -> Option<MixerWindowLayout> {
    let value = saved_object(path).remove(MIXER_KEY)?;
    match serde_json::from_value::<MixerWindowLayout>(value) {
        Ok(layout) if layout.is_valid() => Some(layout),
        Ok(_) => {
            log::warn!(
                "ignoring the invalid mixer window layout in {}",
                path.display()
            );
            None
        }
        Err(error) => {
            log::warn!(
                "ignoring the mixer window layout in {}: {error}",
                path.display()
            );
            None
        }
    }
}

/// Writes the mixer window's `layout` into `path` (the `mixer` key) atomically, keeping the rest
/// of the file, and creating it and its folder when needed.
///
/// # Errors
///
/// The I/O error of creating the folder or writing the file.
pub fn save_mixer_window_layout(layout: &MixerWindowLayout, path: &Path) -> io::Result<()> {
    let mut object = saved_object(path);
    object.insert(
        MIXER_KEY.to_owned(),
        serde_json::to_value(layout).map_err(io::Error::other)?,
    );
    write_object(&object, path)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        MixerWindowLayout, PaneLayout, ScreenArea, WindowLayout, load_mixer_window_layout,
        load_window_layout, save_mixer_window_layout, save_window_layout,
    };

    const MAIN: WindowLayout = WindowLayout {
        x: 120.0,
        y: 64.0,
        width: 1440.0,
        height: 900.0,
        maximized: false,
        panes: PaneLayout {
            browser: super::Pane {
                size: 240.0,
                open: true,
            },
            inspector: super::Pane {
                size: 280.0,
                open: false,
            },
            editors: super::Pane {
                size: 300.0,
                open: true,
            },
        },
    };

    const MIXER: MixerWindowLayout = MixerWindowLayout {
        x: -1700.0,
        y: 140.5,
        width: 1000.0,
        height: 560.0,
        open: true,
    };

    #[test]
    fn the_mixer_window_layout_and_the_main_one_share_the_file() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join("config/window.json");
        assert_eq!(load_mixer_window_layout(&path), None, "nothing saved yet");
        // The mixer's first: the file then has no main layout, which stays absent.
        save_mixer_window_layout(&MIXER, &path).expect("saved");
        assert_eq!(load_mixer_window_layout(&path), Some(MIXER));
        assert_eq!(load_window_layout(&path), None);
        // Saving the main window's layout keeps the mixer's, and the other way round.
        save_window_layout(&MAIN, &path).expect("saved");
        assert_eq!(load_window_layout(&path), Some(MAIN));
        assert_eq!(load_mixer_window_layout(&path), Some(MIXER));
        let closed = MixerWindowLayout {
            open: false,
            ..MIXER
        };
        save_mixer_window_layout(&closed, &path).expect("saved");
        assert_eq!(load_mixer_window_layout(&path), Some(closed));
        assert_eq!(load_window_layout(&path), Some(MAIN));
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(text.ends_with("}\n"), "{text}");
    }

    #[test]
    fn an_old_or_invalid_mixer_layout_is_none() {
        let dir = TempDir::new().expect("a temporary folder");
        let path = dir.path().join("window.json");
        // An older file, without the key; a key without `open` reads as closed.
        std::fs::write(&path, r#"{"x": 1, "y": 2, "width": 800, "height": 600}"#).expect("written");
        assert_eq!(load_mixer_window_layout(&path), None);
        std::fs::write(
            &path,
            r#"{"x": 1, "y": 2, "width": 800, "height": 600, "mixer": {"x": 5, "y": 6, "width": 700, "height": 400}}"#,
        )
        .expect("written");
        let read = load_mixer_window_layout(&path).expect("a mixer layout");
        assert!(!read.open);
        assert_eq!((read.x, read.width), (5.0, 700.0));
        for bad in [
            r#"{"mixer": {"x": 0, "y": 0, "width": -1, "height": 400}}"#,
            r#"{"mixer": {"x": 0, "y": 0, "width": 0, "height": 400}}"#,
            r#"{"mixer": "left"}"#,
            "not json",
        ] {
            std::fs::write(&path, bad).expect("written");
            assert_eq!(load_mixer_window_layout(&path), None, "{bad}");
        }
    }

    #[test]
    fn a_mixer_window_off_every_display_is_not_restored() {
        let main = ScreenArea {
            x: 0.0,
            y: 0.0,
            width: 1728.0,
            height: 1117.0,
        };
        let external = ScreenArea {
            x: -1920.0,
            y: 37.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert!(MIXER.is_visible_on(&[main, external]));
        assert!(!MIXER.is_visible_on(&[main]), "its display is gone");
        let below = MixerWindowLayout {
            y: 1117.0 - 20.0,
            x: 100.0,
            ..MIXER
        };
        assert!(
            !below.is_visible_on(&[main]),
            "only 20 px of its top strip show"
        );
    }
}

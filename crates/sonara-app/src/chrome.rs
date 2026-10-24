//! What the main window's chrome shows of the session: the title with the
//! dirty marker, and the Edit menu's Undo and Redo items, as the prototype's
//! `MainWindow._update_title` words them.
//!
//! - The title reads "name • — path — Sonara", with " — not saved" in place of the path for a
//!   project that was never saved, and "Sonara" without a project. The "•" marks unsaved
//!   changes; the macOS close button's dot is not set (see the desktop's `chrome` module).
//! - The items read "Undo label" and "Redo label", or "Undo" and "Redo" with nothing to undo or
//!   redo, when they are disabled.

use serde::Serialize;

use crate::session::SharedSession;

/// The window title and the Undo and Redo items of one moment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowChrome {
    /// The window title.
    pub title: String,
    /// Whether the project has unsaved changes.
    pub dirty: bool,
    /// The Undo item's text.
    pub undo: String,
    /// Whether there is a step to undo.
    pub can_undo: bool,
    /// The Redo item's text.
    pub redo: String,
    /// Whether there is a step to redo.
    pub can_redo: bool,
}

impl WindowChrome {
    /// The chrome of `session` now; the session is locked only to read it.
    #[must_use]
    pub fn of(session: &SharedSession) -> Self {
        let session = session.lock();
        let history = session.history();
        let undo_label = history.undo_label();
        let redo_label = history.redo_label();
        let dirty = session.is_dirty();
        let title = match session.project() {
            None => "Sonara".to_owned(),
            Some(project) => {
                let marker = if dirty { " •" } else { "" };
                let place = session
                    .location()
                    .and_then(|location| location.path.as_ref())
                    .map_or_else(
                        || " — not saved".to_owned(),
                        |path| format!(" — {}", path.display()),
                    );
                format!("{}{marker}{place} — Sonara", project.name)
            }
        };
        Self {
            title,
            dirty,
            undo: item_text("Undo", undo_label),
            can_undo: undo_label.is_some(),
            redo: item_text("Redo", redo_label),
            can_redo: redo_label.is_some(),
        }
    }
}

/// "Undo label", or "Undo" alone.
fn item_text(verb: &str, label: Option<&str>) -> String {
    match label {
        Some(label) if !label.is_empty() => format!("{verb} {label}"),
        _ => verb.to_owned(),
    }
}

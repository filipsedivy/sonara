//! The texts of the window's prompts and the status-bar reports of the File menu's media and
//! project commands, as
//! the prototype's `MainWindow` words them. The desktop shows the prompts as native message
//! boxes; the wording lives here, so the container's tests check it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sonara_core::ids::MASTER_ID;
use sonara_core::model::project::Project;
use sonara_project::store::IntegrityReport;

use crate::events::StatusMessage;

/// A prompt or a report box: its title and text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prompt {
    /// The window title.
    pub title: String,
    /// The text.
    pub text: String,
}

impl Prompt {
    fn new(title: &str, text: impl Into<String>) -> Self {
        Self {
            title: title.to_owned(),
            text: text.into(),
        }
    }
}

/// MW-11, Verify Project: "Project is consistent." or "Problems found.", then the counts and the
/// routing issues.
#[must_use]
pub fn verify_report(report: &IntegrityReport) -> Prompt {
    let head = if report.ok {
        "Project is consistent."
    } else {
        "Problems found."
    };
    let mut details = vec![
        format!("Missing media: {}", report.missing.len()),
        format!("Size mismatches: {}", report.size_mismatch.len()),
        format!("Hash mismatches: {}", report.hash_mismatch.len()),
        format!("Unreferenced files: {}", report.unreferenced_files.len()),
    ];
    details.extend(report.routing_issues.iter().cloned());
    Prompt::new(
        "Verify Project",
        format!("{head}\n\n{}", details.join("\n")),
    )
}

/// The prompt before an export replaces `files` (the mix's and, the stems'):
/// "Overwrite existing files?" and one file per line.
#[must_use]
pub fn overwrite_prompt(files: &[PathBuf]) -> Prompt {
    let lines: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
    Prompt::new(
        "Overwrite?",
        format!("Overwrite existing files?\n{}", lines.join("\n")),
    )
}

/// MW-21, Delete Selected Track: the confirmation for a track that has clips; `None` when it
/// is deleted without asking (no clips) or not at all (the master, an unknown track).
#[must_use]
pub fn delete_track_prompt(project: &Project, track_id: &str) -> Option<Prompt> {
    if track_id == MASTER_ID {
        return None;
    }
    let track = project.tracks.iter().find(|t| t.id == track_id)?;
    (!track.clips.is_empty()).then(|| {
        Prompt::new(
            "Delete Track",
            format!(
                "Delete “{}” and its {} clip(s)? (Undo is available.)",
                track.name,
                track.clips.len()
            ),
        )
    })
}

/// Whether Delete Selected Track can delete `track_id`: any track but the master.
#[must_use]
pub fn can_delete_track(project: &Project, track_id: &str) -> bool {
    track_id != MASTER_ID && project.tracks.iter().any(|t| t.id == track_id)
}

/// The most unused files the prompt names.
const UNUSED_NAMES_SHOWN: usize = 20;

/// MW-13, Remove Unused Media: the information "No unused media files." (`Err`), or the
/// confirmation naming up to 20 of `files` (`Ok`).
///
/// # Errors
///
/// The information box when there is nothing to remove.
pub fn unused_media_prompt(files: &[PathBuf]) -> Result<Prompt, Prompt> {
    const TITLE: &str = "Remove Unused Media";
    if files.is_empty() {
        return Err(Prompt::new(TITLE, "No unused media files."));
    }
    let names: Vec<String> = files
        .iter()
        .take(UNUSED_NAMES_SHOWN)
        .map(|f| {
            f.file_name().map_or_else(
                || f.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    Ok(Prompt::new(
        TITLE,
        format!(
            "Delete {} unused file(s) from the project folder?\n\n{}\n\nThis cannot be undone.",
            files.len(),
            names.join("\n")
        ),
    ))
}

/// A message for `duration_ms`.
fn message(text: String, duration_ms: u32) -> StatusMessage {
    StatusMessage {
        text,
        duration_ms,
        error: false,
    }
}

/// "Removed N file(s)".
#[must_use]
pub fn removed_media(count: usize) -> StatusMessage {
    message(format!("Removed {count} file(s)"), 4000)
}

/// "Embedded N linked file(s)".
#[must_use]
pub fn embedded_media(count: usize) -> StatusMessage {
    message(format!("Embedded {count} linked file(s)"), 4000)
}

/// "Packaged copy written to path", for 5 s.
#[must_use]
pub fn packaged_copy(path: &Path) -> StatusMessage {
    message(format!("Packaged copy written to {}", path.display()), 5000)
}

/// "Imported N MIDI track(s)" of `import_midi`'s result.
#[must_use]
pub fn imported_midi(result: &Value) -> StatusMessage {
    let tracks = result["tracks"].as_array().map_or(0, Vec::len);
    message(format!("Imported {tracks} MIDI track(s)"), 4000)
}

/// "Exported N notes to path" of `export_midi`'s result, for 5 s.
#[must_use]
pub fn exported_midi(result: &Value) -> StatusMessage {
    let notes = result["notes"].as_u64().unwrap_or(0);
    let path = result["path"].as_str().unwrap_or_default();
    message(format!("Exported {notes} notes to {path}"), 5000)
}

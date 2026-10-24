//! The native prompts and panels (5, 9 and 10): the
//! dialog plugin's message boxes and its open and save panels, over the main window. Every prompt
//! and panel of the desktop goes through here. They block until answered, so they run off the main
//! thread: on a lifecycle flow's thread or a command's blocking thread (PERF-07). The callers hold
//! the [`crate::shell::Shell`]'s prompt claim, so two never stack.
//!
//! With the `bench` feature, a benchmark run never shows them: each is answered from the queue the
//! page filled (`bench_answer_prompts`), or cancelled when the queue is empty, and recorded with
//! its title and text (`bench_prompts_asked`), so that the end-to-end harness drives the menu's
//! flows through their prompts and checks their wording. The release bundle never has the feature.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{
    DialogExt, FileDialogBuilder, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};

use crate::window::MAIN;

/// A message box's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buttons {
    /// OK alone: an information box.
    Ok,
    /// Yes and No.
    YesNo,
    /// Save, Discard and Cancel: the unsaved-changes prompt.
    SaveDiscardCancel,
}

/// The button chosen; a box closed without one is `Cancel`, or `No` for Yes and No.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Choice {
    /// OK.
    Ok,
    /// Yes.
    Yes,
    /// No.
    No,
    /// Save.
    Save,
    /// Discard.
    Discard,
    /// Cancel.
    Cancel,
}

/// The unsaved-changes prompt's buttons.
const SAVE: &str = "Save";
const DISCARD: &str = "Discard";
const CANCEL: &str = "Cancel";

/// A message box `title` with `text`, of `kind`, over the main window; blocks until answered.
pub fn ask(
    app: &AppHandle,
    title: &str,
    text: &str,
    kind: MessageDialogKind,
    buttons: Buttons,
) -> Choice {
    #[cfg(feature = "bench")]
    if let Some(answer) = crate::bench::answer(app, "message", title, text) {
        return answer.choice(buttons);
    }
    let mut dialog = app.dialog().message(text).title(title).kind(kind);
    if let Some(window) = app.get_webview_window(MAIN) {
        dialog = dialog.parent(&window);
    }
    match buttons {
        Buttons::Ok => {
            dialog.buttons(MessageDialogButtons::Ok).blocking_show();
            Choice::Ok
        }
        Buttons::YesNo => {
            if dialog.buttons(MessageDialogButtons::YesNo).blocking_show() {
                Choice::Yes
            } else {
                Choice::No
            }
        }
        Buttons::SaveDiscardCancel => {
            let dialog = dialog.buttons(MessageDialogButtons::YesNoCancelCustom(
                SAVE.to_owned(),
                DISCARD.to_owned(),
                CANCEL.to_owned(),
            ));
            // The custom buttons answer with their labels; the standard results cover a platform
            // that reports the buttons' roles instead.
            match dialog.blocking_show_with_result() {
                MessageDialogResult::Yes => Choice::Save,
                MessageDialogResult::No => Choice::Discard,
                MessageDialogResult::Custom(label) if label == SAVE => Choice::Save,
                MessageDialogResult::Custom(label) if label == DISCARD => Choice::Discard,
                _ => Choice::Cancel,
            }
        }
    }
}

/// An open or save panel: its title, the folder it starts in, the file name it proposes and the
/// file types it shows.
#[derive(Debug, Clone, Default)]
pub struct Panel<'a> {
    /// The panel's title.
    pub title: &'a str,
    /// The folder it starts in.
    pub directory: Option<PathBuf>,
    /// The proposed file name (save panels).
    pub file_name: Option<String>,
    /// The file types: a name and the extensions without their dots.
    pub filter: Option<(&'a str, Vec<&'a str>)>,
}

impl Panel<'_> {
    fn builder(&self, app: &AppHandle) -> FileDialogBuilder<tauri::Wry> {
        let mut panel = app.dialog().file().set_title(self.title);
        if let Some((name, extensions)) = &self.filter {
            panel = panel.add_filter(*name, extensions);
        }
        if let Some(name) = &self.file_name {
            panel = panel.set_file_name(name);
        }
        if let Some(directory) = &self.directory {
            panel = panel.set_directory(directory);
        }
        if let Some(window) = app.get_webview_window(MAIN) {
            panel = panel.set_parent(&window);
        }
        panel
    }

    /// What a benchmark run records of the panel: its proposed file name, else its folder.
    #[cfg(feature = "bench")]
    fn detail(&self) -> String {
        self.file_name.clone().unwrap_or_else(|| {
            self.directory
                .as_deref()
                .map(std::path::Path::display)
                .map(|path| path.to_string())
                .unwrap_or_default()
        })
    }
}

fn into_path(picked: tauri_plugin_dialog::FilePath) -> Option<PathBuf> {
    picked
        .into_path()
        .inspect_err(|error| log::warn!("the panel returned no file path: {error}"))
        .ok()
}

/// A save panel; the chosen path, or `None` when cancelled. Blocks.
pub fn save_file(app: &AppHandle, panel: &Panel<'_>) -> Option<PathBuf> {
    #[cfg(feature = "bench")]
    if let Some(answer) = crate::bench::answer(app, "save", panel.title, &panel.detail()) {
        return answer.path();
    }
    panel.builder(app).blocking_save_file().and_then(into_path)
}

/// An open panel for one file; `None` when cancelled. Blocks.
pub fn pick_file(app: &AppHandle, panel: &Panel<'_>) -> Option<PathBuf> {
    #[cfg(feature = "bench")]
    if let Some(answer) = crate::bench::answer(app, "open", panel.title, &panel.detail()) {
        return answer.path();
    }
    panel.builder(app).blocking_pick_file().and_then(into_path)
}

/// An open panel for one folder; `None` when cancelled. Blocks.
pub fn pick_folder(app: &AppHandle, panel: &Panel<'_>) -> Option<PathBuf> {
    #[cfg(feature = "bench")]
    if let Some(answer) = crate::bench::answer(app, "folder", panel.title, &panel.detail()) {
        return answer.path();
    }
    panel
        .builder(app)
        .blocking_pick_folder()
        .and_then(into_path)
}

/// An open panel for several files that returns at once; `done` gets the chosen files, none when
/// cancelled.
pub fn pick_files<F>(app: &AppHandle, panel: &Panel<'_>, done: F)
where
    F: FnOnce(Vec<PathBuf>) + Send + 'static,
{
    #[cfg(feature = "bench")]
    if let Some(answer) = crate::bench::answer(app, "open-files", panel.title, &panel.detail()) {
        done(answer.paths());
        return;
    }
    panel.builder(app).pick_files(move |picked| {
        done(
            picked
                .unwrap_or_default()
                .into_iter()
                .filter_map(into_path)
                .collect(),
        );
    });
}

//! The project lifecycle of the File menu: New, Open, Open
//! Recent, Save, Save As and the opening of a dropped `.sonara`,
//! with the unsaved-changes prompt they share with quitting.
//!
//! Each flow runs on a thread of its own ([`spawn`]): the dialog plugin's blocking panels and
//! prompts must not run on the main thread, and the project service may wait for the session.
//! One flow at a time (the [`Shell`]'s prompt claim), so two panels never stack.
//!
//! The order follows the prototype's `MainWindow`: the file is chosen first, then unsaved
//! changes are asked about ([`confirm_discard`]: Save, Discard, Cancel), then the transport
//! stops and the project is replaced. Results and errors go to the status bar, as the
//! prototype's `statusBar().showMessage` did. New takes the New Project dialog's choices (DL-01,
//! step 9: [`replace_with_new`], called by the dialog's command after it checked them).

use std::path::{Path, PathBuf};
use std::thread;

use sonara_app::events::StatusMessage;
use sonara_app::paths::default_projects_dir;
use sonara_app::projects::NewProject;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::MessageDialogKind;

use crate::commands::show_message;
use crate::prompt::{self, Buttons, Choice, Panel};
use crate::shell::Shell;

/// What the user decided about unsaved changes.
pub enum Decision {
    /// Go on; `discarded` when the changes were dropped (the autosave goes with them on
    /// quitting).
    Proceed { discarded: bool },
    /// Stay with the project as it is.
    Cancel,
}

/// Runs `flow` on a thread of its own, unless another flow's panel or prompt is open.
pub fn spawn<F>(app: &AppHandle, name: &str, flow: F)
where
    F: FnOnce(&AppHandle, &Shell) + Send + 'static,
{
    let shell = app.state::<Shell>();
    if !shell.begin_prompt() {
        log::info!("{name}: another prompt is open");
        return;
    }
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name(format!("sonara-{name}"))
        .spawn(move || {
            let shell = handle.state::<Shell>();
            flow(&handle, &shell);
            shell.end_prompt();
        });
    if let Err(error) = spawned {
        log::error!("cannot start {name}: {error}");
        shell.end_prompt();
    }
}

/// The prototype's `_confirm_discard`: go on at once without unsaved changes, else ask "Save
/// changes to “name”?". Save writes the project (asking for a path when it was never saved); a
/// failed or cancelled save cancels. Blocks: call it from a flow's thread.
pub fn confirm_discard(app: &AppHandle, shell: &Shell) -> Decision {
    let (dirty, name) = {
        let session = shell.app().session().lock();
        let name = session.project().map(|project| project.name.clone());
        (session.is_dirty(), name)
    };
    if !dirty {
        return Decision::Proceed { discarded: false };
    }
    let name = name.unwrap_or_else(|| "project".to_owned());
    let choice = prompt::ask(
        app,
        "Unsaved Changes",
        &format!("Save changes to “{name}”?"),
        MessageDialogKind::Warning,
        Buttons::SaveDiscardCancel,
    );
    match choice {
        Choice::Save if save(app, shell) => Decision::Proceed { discarded: false },
        Choice::Discard => Decision::Proceed { discarded: true },
        _ => Decision::Cancel,
    }
}

/// Saves in place, or runs Save As for a project that was never saved. `false` when
/// there is no project, the panel was cancelled or saving failed (the error is shown).
pub fn save(app: &AppHandle, shell: &Shell) -> bool {
    let (open, saved_at) = {
        let session = shell.app().session().lock();
        (
            session.project().is_some(),
            session
                .location()
                .and_then(|location| location.path.clone()),
        )
    };
    match (open, saved_at) {
        (false, _) => false,
        (true, None) => save_as(app, shell),
        (true, Some(_)) => save_to(app, shell, None),
    }
}

/// The save panel at `<projects dir>/<name>.sonara`, then a save there.
pub fn save_as(app: &AppHandle, shell: &Shell) -> bool {
    let Some(name) = shell
        .app()
        .session()
        .project()
        .map(|project| project.name.clone())
    else {
        return false;
    };
    let panel = Panel {
        title: "Save Project As",
        directory: default_projects_dir().ok(),
        file_name: Some(format!("{name}.sonara")),
        filter: Some(("Sonara projects", vec!["sonara"])),
    };
    let Some(path) = prompt::save_file(app, &panel) else {
        return false;
    };
    save_to(app, shell, Some(&path))
}

/// Saves to `path` (in place when `None`): "Saved path", or the error.
fn save_to(app: &AppHandle, shell: &Shell, path: Option<&Path>) -> bool {
    match shell.app().projects().save(path, None) {
        Ok(result) => {
            log::info!("saved {}", result.path.display());
            show_message(
                app,
                &StatusMessage::info(format!("Saved {}", result.path.display())),
            );
            true
        }
        Err(error) => {
            log::warn!("save failed: {error}");
            show_message(app, &StatusMessage::error(error.to_string()));
            false
        }
    }
}

/// Stops the transport before the project is replaced (`_stop_transport`); never creates it.
fn stop_transport(shell: &Shell) {
    let app = shell.app();
    if app.has_transport()
        && let Err(error) = app.transport().stop()
    {
        log::info!("stopping the transport: {error}");
    }
}

/// After the unsaved-changes prompt, the new project `options` describe (the New
/// Project dialog's); `Ok(false)` when the user cancelled the prompt. Blocks: call it
/// from a flow's thread.
///
/// # Errors
///
/// The message of a project that cannot be created.
pub fn replace_with_new(
    app: &AppHandle,
    shell: &Shell,
    options: &NewProject,
) -> Result<bool, String> {
    if matches!(confirm_discard(app, shell), Decision::Cancel) {
        return Ok(false);
    }
    stop_transport(shell);
    shell
        .app()
        .projects()
        .new_project(options)
        .map(|project| log::info!("created “{}”", project.name))
        .map(|()| true)
        .map_err(|error| {
            log::warn!("cannot create a project: {error}");
            error.to_string()
        })
}

/// Opens `path`, or the one chosen in the open panel.
pub fn open_project(app: &AppHandle, path: Option<PathBuf>) {
    spawn(app, "open-project", move |app, shell| {
        let Some(path) = path.or_else(|| ask_project(app)) else {
            return;
        };
        if matches!(confirm_discard(app, shell), Decision::Cancel) {
            return;
        }
        stop_transport(shell);
        open_at(app, shell, &path);
    });
}

/// The open panel for `*.sonara`; when it is cancelled, a folder panel, because a bundle is a
/// folder the file panel cannot choose (the prototype's fallback).
fn ask_project(app: &AppHandle) -> Option<PathBuf> {
    let directory = default_projects_dir().ok();
    let file = Panel {
        title: "Open Project",
        directory: directory.clone(),
        file_name: None,
        filter: Some(("Sonara projects", vec!["sonara"])),
    };
    prompt::pick_file(app, &file).or_else(|| {
        let folder = Panel {
            title: "Open Project Bundle",
            directory,
            ..Panel::default()
        };
        prompt::pick_folder(app, &folder)
    })
}

/// Opens the project at `path`: its load warnings for 8 s, and the offer of a newer autosave.
fn open_at(app: &AppHandle, shell: &Shell, path: &Path) {
    let loaded = match shell.app().projects().open_project(path, false) {
        Ok(loaded) => loaded,
        Err(error) => {
            log::warn!("cannot open {}: {error}", path.display());
            show_message(app, &StatusMessage::error(error.to_string()));
            return;
        }
    };
    log::info!("opened {}", loaded.path.display());
    if loaded.autosave_available {
        offer_autosave(app, shell, &loaded.path);
    }
    if !loaded.warnings.is_empty() {
        show_message(app, &StatusMessage::error(loaded.warnings.join("; ")));
    }
}

/// The opened project has newer unsaved changes from an autosave; restoring them opens
/// it again with the autosave, as the prototype does. Blocks: call it from a flow's thread.
pub fn offer_autosave(app: &AppHandle, shell: &Shell, path: &Path) {
    let choice = prompt::ask(
        app,
        "Recover Unsaved Changes",
        "This project has newer unsaved changes from an autosave. Restore them?",
        MessageDialogKind::Info,
        Buttons::YesNo,
    );
    if choice != Choice::Yes {
        return;
    }
    if let Err(error) = shell.app().projects().open_project(path, true) {
        log::warn!("cannot restore the autosave of {}: {error}", path.display());
        show_message(app, &StatusMessage::error(error.to_string()));
    }
}

/// MW-04 from the menu.
pub fn save_from_menu(app: &AppHandle) {
    spawn(app, "save", |app, shell| {
        save(app, shell);
    });
}

/// MW-05 from the menu.
pub fn save_as_from_menu(app: &AppHandle) {
    spawn(app, "save-as", |app, shell| {
        save_as(app, shell);
    });
}

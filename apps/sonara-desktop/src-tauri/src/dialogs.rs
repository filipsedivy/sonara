//! The browser's, the inspector's and the dialogs' side of the desktop, and the File menu's
//! items. Each command wraps a
//! `sonara-app` call; the forms are validated, and the prompts and reports worded, there.
//!
//! **The page's dialogs** (HTML, `src/lib/dialogs/`): the menu asks the main window's page to
//! open one (`dialog-command`: `new-project`, `settings`, `export-audio`, `agent-connection`,
//! `ai-composer`, `about`, `import-audio`), the page reads its view through a command and sends the form back.
//! A form that is wrong comes back as an error, and the dialog stays open.
//!
//! **Native prompts and panels** (the dialog plugin) block, so they run off the
//! main thread and off the async runtime's workers (PERF-07): a command that may prompt runs on a
//! blocking thread ([`blocking`]), and the menu's flows run on a thread of their own
//! ([`lifecycle::spawn`]). Both hold the [`Shell`]'s prompt claim, so two never stack.

use std::path::{Path, PathBuf};

use serde::Serialize;
use sonara_app::about::{self, About, AgentConnection};
use sonara_app::browser::{self, BrowserItem, FileEntry};
use sonara_app::edit::{self, Edit};
use sonara_app::events::{ErrorNotice, StatusMessage};
use sonara_app::export_dialog::{self, ExportDialogView, ExportForm};
use sonara_app::paths::default_projects_dir;
use sonara_app::projects::{FieldError, NewProjectDialogView, NewProjectForm, new_project_dialog};
use sonara_app::prompts::{self, Prompt};
use sonara_app::settings_dialog::{self, SavedSettings, SettingsDialogView, SettingsForm};
use sonara_app::transport_control;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::MessageDialogKind;

use crate::commands::show_message;
use crate::lifecycle;
use crate::prompt::{self, Buttons, Choice, Panel};
use crate::shell::Shell;
use crate::window::MAIN;

/// A dialog the page opens (`dialog-command`).
#[derive(Debug, Clone, Serialize)]
pub struct DialogCommand {
    /// `new-project`, `settings`, `export-audio`, `agent-connection`, `ai-composer`, `about` or
    /// `import-audio`.
    pub dialog: &'static str,
}

/// Asks the main window's page to open `dialog`, and brings the window forward.
pub fn open(app: &AppHandle, dialog: &'static str) {
    if let Some(window) = app.get_webview_window(MAIN) {
        if let Err(error) = window.set_focus() {
            log::debug!("cannot focus the main window: {error}");
        }
        if let Err(error) = window.emit("dialog-command", DialogCommand { dialog }) {
            log::warn!("cannot open the {dialog} dialog: {error}");
        }
    }
}

/// The prompt claim of the [`Shell`], released when it drops.
struct PromptClaim<'a>(&'a Shell);

impl<'a> PromptClaim<'a> {
    fn take(shell: &'a Shell) -> Result<Self, ErrorNotice> {
        if shell.begin_prompt() {
            Ok(Self(shell))
        } else {
            Err(ErrorNotice {
                code: "CONFLICT",
                message: "another prompt is open".to_owned(),
            })
        }
    }
}

impl Drop for PromptClaim<'_> {
    fn drop(&mut self) {
        self.0.end_prompt();
    }
}

/// Runs `work` on a blocking thread of the async runtime and hands back its result: a command
/// whose work may wait for a native prompt must not hold one of the runtime's workers.
async fn blocking<T, F>(app: AppHandle, work: F) -> Result<T, ErrorNotice>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle, &Shell) -> Result<T, ErrorNotice> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let shell = app.state::<Shell>();
        work(&app, &shell)
    })
    .await
    .map_err(|error| ErrorNotice {
        code: "INTERNAL",
        message: format!("the command stopped: {error}"),
    })?
}

/// A native question with `question`'s title and text, over the main window: `true` for Yes.
fn ask(app: &AppHandle, question: &Prompt) -> bool {
    prompt::ask(
        app,
        &question.title,
        &question.text,
        MessageDialogKind::Info,
        Buttons::YesNo,
    ) == Choice::Yes
}

/// A native information box (the prototype's `QMessageBox.information`).
fn inform(app: &AppHandle, report: &Prompt) {
    prompt::ask(
        app,
        &report.title,
        &report.text,
        MessageDialogKind::Info,
        Buttons::Ok,
    );
}

fn status_error(app: &AppHandle, text: impl Into<String>) {
    let text = text.into();
    log::info!("status: {text}");
    show_message(app, &StatusMessage::error(text));
}

/// The folder an export or a save panel starts in: the project's, else the projects folder.
fn project_folder(shell: &Shell) -> Option<PathBuf> {
    let session = shell.app().session().lock();
    session
        .location()
        .and_then(|location| location.path.as_ref())
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .or_else(|| default_projects_dir().ok())
}

// -- New Project -------------------------------------------------------------------

/// The New Project dialog as it opens.
#[tauri::command]
pub fn new_project_view() -> NewProjectDialogView {
    new_project_dialog()
}

/// What the New Project dialog's OK did.
#[derive(Debug, Clone, Serialize)]
pub struct Created {
    /// Whether the project was created; `false` when the user cancelled the unsaved-changes
    /// prompt.
    pub created: bool,
}

/// The dialog's choices are checked (G3: a wrong field comes back, and nothing else
/// happens), then the unsaved-changes prompt, then the project replaces the open one.
///
/// # Errors
///
/// The field that is wrong (`time_signature`, …), a project that cannot be built or created
/// (`field` `form`), or another prompt that is open.
#[tauri::command]
pub async fn create_project(app: AppHandle, form: NewProjectForm) -> Result<Created, FieldError> {
    let options = form.to_new_project()?;
    let whole = |notice: ErrorNotice| FieldError {
        field: "form",
        code: notice.code,
        message: notice.message,
    };
    blocking(app, move |app, shell| {
        shell
            .app()
            .projects()
            .check_new_project(&options)
            .map_err(|error| ErrorNotice::from(&error))?;
        let _claim = PromptClaim::take(shell)?;
        let created =
            lifecycle::replace_with_new(app, shell, &options).map_err(|message| ErrorNotice {
                code: "INTERNAL",
                message,
            })?;
        Ok(Created { created })
    })
    .await
    .map_err(whole)
}

// -- Settings ------------------------------------------------------------------------

/// The Settings dialog for the application's settings, with the system's audio devices.
///
/// # Errors
///
/// None; the `Result` is Tauri's requirement for an asynchronous command with borrowed state.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn settings_view(shell: State<'_, Shell>) -> Result<SettingsDialogView, ErrorNotice> {
    Ok(settings_dialog::dialog_view(shell.app()))
}

/// The Settings dialog's OK: the settings are saved, the autosave timer restarts with
/// the new interval or stops, and every window hears the stored theme (`theme`, A1), which it
/// applies at once (the prototype applied it at the next start).
///
/// # Errors
///
/// A field the dialog did not offer (`INVALID_ARGUMENT`), or "could not save settings: …".
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn apply_settings(
    app: AppHandle,
    shell: State<'_, Shell>,
    form: SettingsForm,
) -> Result<SavedSettings, ErrorNotice> {
    let saved = settings_dialog::apply_and_save(shell.app(), &form).map_err(|error| {
        let notice = ErrorNotice::from(&error);
        if notice.code == "INVALID_ARGUMENT" {
            notice
        } else {
            ErrorNotice {
                code: notice.code,
                message: format!("could not save settings: {}", notice.message),
            }
        }
    })?;
    shell.restart_autosave(saved.autosave_seconds);
    let theme = shell.app().settings().theme.clone();
    if let Err(error) = app.emit("theme", theme) {
        log::warn!("cannot tell the windows about the theme: {error}");
    }
    Ok(saved)
}

// -- Export Audio ---------------------------------------------------------------------

/// The Export Audio dialog for the open project.
///
/// # Errors
///
/// `NO_PROJECT`.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn export_view(shell: State<'_, Shell>) -> Result<ExportDialogView, ErrorNotice> {
    let project = shell
        .app()
        .session()
        .lock()
        .require_project()
        .cloned()
        .map_err(ErrorNotice::from)?;
    let folder = project_folder(&shell).unwrap_or_default();
    Ok(export_dialog::dialog_view(
        &project,
        &folder,
        &export_dialog::available_formats(),
    ))
}

/// The dialog's fields after choosing the preset `preset`.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn export_preset(form: ExportForm, preset: String) -> ExportForm {
    form.with_preset(&preset, &export_dialog::available_formats())
}

/// The export's Browse…: the save panel "Export to" at `path`; the chosen file without its
/// extension, or `None` when cancelled.
///
/// # Errors
///
/// None but a command that stopped; the panel's own failure is a cancel.
#[tauri::command]
pub async fn export_browse(app: AppHandle, path: String) -> Result<Option<String>, ErrorNotice> {
    blocking(app, move |app, _shell| {
        let current = Path::new(&path);
        let panel = Panel {
            title: "Export to",
            directory: current
                .parent()
                .filter(|dir| dir.is_dir())
                .map(Path::to_path_buf),
            file_name: current
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            filter: None,
        };
        let picked = prompt::save_file(app, &panel);
        Ok(picked.map(|picked| {
            sonara_app::paths::with_suffix(&picked, "")
                .display()
                .to_string()
        }))
    })
    .await
}

/// What the Export dialog's OK did.
#[derive(Debug, Clone, Serialize)]
pub struct Exported {
    /// Whether the export jobs were queued; `false` when the user did not want to replace the
    /// existing files.
    pub started: bool,
    /// The jobs' operation IDs.
    pub jobs: Vec<String>,
}

/// The dialog's export: when files exist already (the mix's or, the stems'), the
/// prompt "Overwrite?" names them first; then the export job, and the stems' job.
///
/// # Errors
///
/// A field that is wrong ("choose at least one format", …), `NO_PROJECT`, or the jobs'
/// submission errors.
#[tauri::command]
pub async fn export_audio(app: AppHandle, form: ExportForm) -> Result<Exported, ErrorNotice> {
    blocking(app, move |app, shell| {
        let project = shell
            .app()
            .session()
            .lock()
            .require_project()
            .cloned()
            .map_err(ErrorNotice::from)?;
        let plan = form
            .plan(&project, &export_dialog::available_formats())
            .map_err(|error| ErrorNotice::from(&error))?;
        let existing = plan.existing_files();
        if !existing.is_empty() {
            let _claim = PromptClaim::take(shell)?;
            if !ask(app, &prompts::overwrite_prompt(&existing)) {
                return Ok(Exported {
                    started: false,
                    jobs: Vec::new(),
                });
            }
        }
        let jobs = plan
            .submit(shell.app().render(), !existing.is_empty())
            .map_err(|error| ErrorNotice::from(&error))?;
        Ok(Exported {
            started: true,
            jobs: jobs.iter().map(|job| job.id().to_owned()).collect(),
        })
    })
    .await
}

// -- Help: Agent Connection and About -------------------------------------------------

/// Agent Connection: the IPC address, the registration of the bundled `sonara mcp`, and the
/// discovery file.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn agent_connection(shell: State<'_, Shell>) -> AgentConnection {
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("sonara-desktop"));
    let agent = shell.agent();
    about::agent_connection(
        agent.address.as_deref(),
        agent.discovery.as_deref(),
        &about::bundled_cli(&executable),
    )
}

/// About Sonara: the versions, the audio engine's state, the agent connection, the credits,
/// and the bundle's generated licences when it carries them.
///
/// # Errors
///
/// None; see [`settings_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn about_info(app: AppHandle, shell: State<'_, Shell>) -> Result<About, ErrorNotice> {
    let audio = transport_control::engine_status(shell.app()).map_or_else(
        || "engine not started".to_owned(),
        |status| {
            if status["available"].as_bool() == Some(false) {
                status["reason"]
                    .as_str()
                    .unwrap_or("unavailable")
                    .to_owned()
            } else {
                let backend = status["backend"].as_str().unwrap_or("native");
                let rate = status["device_sample_rate"]
                    .as_i64()
                    .map_or_else(|| "?".to_owned(), |rate| rate.to_string());
                format!("{backend} · {rate} Hz")
            }
        },
    );
    let agent = shell
        .agent()
        .address
        .clone()
        .unwrap_or_else(|| "off".to_owned());
    let extra = vec![
        ("Tauri".to_owned(), tauri::VERSION.to_owned()),
        ("Audio".to_owned(), audio),
        ("Agent IPC".to_owned(), agent),
    ];
    let licenses = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join(about::LICENSES_FILE));
    Ok(about::about(extra, licenses.as_deref()))
}

/// Opens About's `url` (the author's address or a credited page) in the system's applications.
///
/// # Errors
///
/// `PERMISSION_DENIED` for any other link, or the error of running `open`.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn open_link(url: String) -> Result<(), ErrorNotice> {
    if !about::is_known_link(&url) {
        return Err(ErrorNotice {
            code: "PERMISSION_DENIED",
            message: format!("{url} is not a link of About Sonara"),
        });
    }
    // `open` hands the link to its application and exits at once.
    let status = std::process::Command::new("open")
        .arg(&url)
        .status()
        .map_err(|error| ErrorNotice {
            code: "IO_ERROR",
            message: format!("cannot open {url}: {error}"),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ErrorNotice {
            code: "IO_ERROR",
            message: format!("cannot open {url}: {status}"),
        })
    }
}

// -- the browser ------------------------------------------------------------------------

/// The Devices tab's tree.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn browser_devices(shell: State<'_, Shell>) -> Vec<BrowserItem> {
    browser::device_tree(shell.app().plugins())
}

/// The Files tab's root (`~/Music`, else `~`).
#[tauri::command]
pub fn browser_root() -> String {
    browser::browser_root().display().to_string()
}

/// The folders and audio files of `path`, a folder under the Files tab's root.
///
/// # Errors
///
/// `PERMISSION_DENIED` outside the root, `NOT_FOUND` for a path that is not a folder, or the
/// error of reading it.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, ErrorNotice> {
    browser::list_directory(&browser::browser_root(), Path::new(&path))
        .map_err(|error| ErrorNotice::from(&error))
}

// -- prompts of the page's actions ------------------------------------------------------------

/// MW-21, Delete Selected Track: a track with clips is deleted only after "Delete “name” and
/// its N clip(s)?"; the master never. `false` when nothing was deleted.
///
/// # Errors
///
/// The edit's error, or another prompt that is open.
#[tauri::command]
pub async fn delete_track(app: AppHandle, track_id: String) -> Result<bool, ErrorNotice> {
    blocking(app, move |app, shell| {
        let Some(project) = shell.app().session().project() else {
            return Ok(false);
        };
        if !prompts::can_delete_track(&project, &track_id) {
            return Ok(false);
        }
        if let Some(question) = prompts::delete_track_prompt(&project, &track_id) {
            let _claim = PromptClaim::take(shell)?;
            if !ask(app, &question) {
                return Ok(false);
            }
        }
        edit::apply(shell.app(), Edit::DeleteTrack { track_id })?;
        Ok(true)
    })
    .await
}

// -- the File menu's flows -------------------------------------------------------------

/// MW-06, Save Packaged Copy: the save panel at `<projects dir>/<name>.sonara`, then the copy.
pub fn save_packaged_copy(app: &AppHandle) {
    lifecycle::spawn(app, "save-packaged", |app, shell| {
        let Some(name) = shell.app().session().project().map(|p| p.name.clone()) else {
            return;
        };
        let Some(path) = save_panel(
            app,
            "Save Packaged Copy",
            &format!("{name}.sonara"),
            ("Sonara projects", &["sonara"]),
        ) else {
            return;
        };
        match shell.app().projects().save_copy(&path, true) {
            Ok(result) => show_message(app, &prompts::packaged_copy(&result.path)),
            Err(error) => status_error(app, ErrorNotice::from(&error).message),
        }
    });
}

/// MW-08, Import MIDI: the open panel for `*.mid` and `*.midi`, then the import at beat 0 with
/// the file's tempo map, as one undo step.
pub fn import_midi(app: &AppHandle) {
    lifecycle::spawn(app, "import-midi", |app, shell| {
        let panel = Panel {
            title: "Import MIDI",
            directory: std::env::var_os("HOME").map(PathBuf::from),
            file_name: None,
            filter: Some(("MIDI files", vec!["mid", "midi"])),
        };
        let Some(path) = prompt::pick_file(app, &panel) else {
            return;
        };
        match shell.app().media().import_midi(&path, true, 0.0) {
            Ok(result) => show_message(app, &prompts::imported_midi(&result)),
            Err(error) => status_error(app, ErrorNotice::from(&error).message),
        }
    });
}

/// MW-10, Export MIDI: the save panel at `<projects dir>/<name>.mid`, then the file (the panel
/// asked before it replaces one).
pub fn export_midi(app: &AppHandle) {
    lifecycle::spawn(app, "export-midi", |app, shell| {
        let Some(name) = shell.app().session().project().map(|p| p.name.clone()) else {
            return;
        };
        let Some(path) = save_panel(
            app,
            "Export MIDI",
            &format!("{name}.mid"),
            ("MIDI", &["mid"]),
        ) else {
            return;
        };
        match shell.app().media().export_midi(&path, None, true) {
            Ok(result) => show_message(app, &prompts::exported_midi(&result)),
            Err(error) => status_error(app, ErrorNotice::from(&error).message),
        }
    });
}

/// A save panel in the projects folder with `name` and one filter.
fn save_panel(
    app: &AppHandle,
    title: &str,
    name: &str,
    (filter, extensions): (&str, &[&str]),
) -> Option<PathBuf> {
    let panel = Panel {
        title,
        directory: default_projects_dir().ok(),
        file_name: Some(name.to_owned()),
        filter: Some((filter, extensions.to_vec())),
    };
    prompt::save_file(app, &panel)
}

/// MW-11, Verify Project: the report in a message box.
pub fn verify_project(app: &AppHandle) {
    lifecycle::spawn(app, "verify", |app, shell| {
        match shell.app().projects().verify(true) {
            Ok(report) => inform(app, &prompts::verify_report(&report)),
            Err(error) => status_error(app, ErrorNotice::from(&error).message),
        }
    });
}

/// MW-12, Embed Linked Media: "Embedded N linked file(s)".
pub fn embed_linked_media(app: &AppHandle) {
    lifecycle::spawn(app, "embed", |app, shell| {
        match shell.app().projects().embed_linked_media() {
            Ok(done) => show_message(app, &prompts::embedded_media(done.len())),
            Err(error) => status_error(app, ErrorNotice::from(&error).message),
        }
    });
}

/// MW-13, Remove Unused Media: "No unused media files.", or the confirmation naming them, then
/// "Removed N file(s)".
pub fn remove_unused_media(app: &AppHandle) {
    lifecycle::spawn(app, "remove-unused", |app, shell| {
        let unused = match shell.app().projects().unused_media() {
            Ok(unused) => unused,
            Err(error) => {
                status_error(app, ErrorNotice::from(&error).message);
                return;
            }
        };
        match prompts::unused_media_prompt(&unused) {
            Err(nothing) => inform(app, &nothing),
            Ok(question) => {
                if !ask(app, &question) {
                    return;
                }
                match shell.app().projects().delete_unused_media() {
                    Ok(removed) => show_message(app, &prompts::removed_media(removed.len())),
                    Err(error) => status_error(app, ErrorNotice::from(&error).message),
                }
            }
        }
    });
}

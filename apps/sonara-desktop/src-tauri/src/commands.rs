//! The window's commands and events. Each command wraps one
//! `sonara-app` call; the logic, the wording and the error codes live there.
//!
//! Commands that take the session's or the transport's locks are `async`, so that a lock held
//! elsewhere (an agent's long edit) never stalls the main thread. The page awaits each edit
//! before it sends the next one of a gesture (`src/lib/gesture.ts`), so their order holds.
//!
//! Events sent to the window, from the event pump's thread:
//!
//! - `session-changed`: every session event as a `SessionNotice` `{version, kind, label,
//!   source, changes}`;
//! - `job-changed`: every change of a background job (`JobNotice`);
//! - `peaks-ready`: a waveform pyramid became ready or failed (`PeaksReady`);
//! - `message`: a `StatusMessage` for the status bar, for example "agent: Create Track" for an
//!   agent's edit, a failed job, an import's files, or a project that could not be
//!   opened;
//! - `view-command`: a View menu item for the page (`menu`);
//! - `mixer-window`: the mixer's own window opened or closed (to the main window only).
//!
//! The pages of the two windows also tell each other about the selection (`gui-selection`) and
//! the mixer window asks the main one for the Devices tab and the transport's keys
//! (`gui-intent`); those events go from page to page.
//!
//! Every session event also refreshes the window title and the Edit menu ([`crate::chrome`]).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sonara_app::catalog::{self, DrumMap, PickerKind, PluginGroup};
use sonara_app::edit::{self, Edit, EditOutcome};
use sonara_app::eq_response::{self, EqResponse};
use sonara_app::events::{ErrorNotice, Notice, StatusMessage};
use sonara_app::media::{AUDIO_IMPORT_SUFFIXES, IMPORT_AUDIO_JOB, ImportReport};
use sonara_app::peaks::PeaksInfo;
use sonara_app::projects::RecoverableSession;
use sonara_app::transport_control::{self, Stopped};
use sonara_app::view::{
    ClipsView, DevicesView, ProjectView, TracksView, clips_view, devices_view,
    project_view as view_of, tracks_view,
};
use sonara_app::window_layout::PaneLayout;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, Manager, State, Webview};

use crate::menu::MenuHandles;
use crate::shell::{AgentStatus, Shell};

/// Sends `message` to the window's status bar; a failure is logged.
pub fn show_message(app: &AppHandle, message: &StatusMessage) {
    if let Err(error) = app.emit("message", message) {
        log::warn!("cannot send a status message: {error}");
    }
}

fn emit<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) {
    if let Err(error) = app.emit(event, payload) {
        log::warn!("cannot send {event}: {error}");
    }
}

/// Forwards one of the application's notices to the window (the event pump's sink).
pub fn forward(app: &AppHandle, notice: Notice) {
    match notice {
        Notice::Session(notice) => {
            log::debug!(
                "session event {} v{} from {}: {}",
                notice.kind,
                notice.version,
                notice.source,
                notice.label
            );
            emit(app, "session-changed", &notice);
            if let Some(message) = StatusMessage::foreign_edit(&notice) {
                log::info!("status: {}", message.text);
                show_message(app, &message);
            }
            // A project opened, saved or closed may change the recent projects.
            let replaced = matches!(notice.kind, "loaded" | "saved" | "closed");
            crate::chrome::refresh(app, replaced);
        }
        Notice::Job(job) => {
            emit(app, "job-changed", &job);
            if let Some(message) = job.failure() {
                log::info!("status: {}", message.text);
                show_message(app, &message);
            }
            // "Exported N file(s) · L LUFS · T dBTP".
            if let Some(message) = sonara_app::export_dialog::export_message(&job) {
                log::info!("status: {}", message.text);
                show_message(app, &message);
            }
            if job.kind == IMPORT_AUDIO_JOB
                && let Some(result) = &job.result
            {
                for line in ImportReport::of(result).messages {
                    log::info!("status: {}", line.text);
                    let message = if line.error {
                        StatusMessage::error(line.text)
                    } else {
                        StatusMessage::info(line.text)
                    };
                    show_message(app, &message);
                }
            }
        }
        Notice::Peaks(ready) => emit(app, "peaks-ready", &ready),
    }
}

/// What the window shows at startup.
#[derive(Debug, Clone, Serialize)]
pub struct ShellStatus {
    /// The agent connection: "agent: on" with its address, or "agent: off".
    pub agent: AgentStatus,
    /// The startup's messages, for example a project that could not be opened.
    pub messages: Vec<StatusMessage>,
    /// The panes as they were saved.
    pub panes: PaneLayout,
    /// The audio file suffixes a drop imports.
    pub audio_suffixes: Vec<&'static str>,
    /// Whether the mixer's own window was open when Sonara quit, so that the page opens it again
    /// (A10); never in a benchmark run, which starts from its own layout.
    pub reopen_mixer: bool,
}

/// The status bar's state at startup; the startup messages are handed out once.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn shell_status(app: AppHandle, shell: State<'_, Shell>) -> ShellStatus {
    #[cfg(feature = "bench")]
    let benchmark = app
        .try_state::<Option<crate::bench::BenchConfig>>()
        .is_some_and(|config| config.is_some());
    #[cfg(not(feature = "bench"))]
    let benchmark = false;
    ShellStatus {
        reopen_mixer: !benchmark && crate::window::mixer_was_open(&app),
        agent: shell.agent().clone(),
        messages: shell
            .take_startup_messages()
            .into_iter()
            .map(StatusMessage::error)
            .collect(),
        panes: shell.panes(),
        audio_suffixes: AUDIO_IMPORT_SUFFIXES.to_vec(),
    }
}

/// The stored `theme` setting: every window applies it at startup, and the
/// Settings dialog's OK sends a change as `theme` ([`crate::dialogs::apply_settings`]).
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn gui_theme(shell: State<'_, Shell>) -> String {
    shell.app().settings().theme.clone()
}

/// The unsaved scratch sessions to offer for recovery.
///
/// # Errors
///
/// The message of a workspaces folder that cannot be listed.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn recoverable_sessions(shell: State<'_, Shell>) -> Result<Vec<RecoverableSession>, String> {
    shell
        .app()
        .projects()
        .recoverable_scratch_sessions()
        .map_err(|error| format!("cannot list the unsaved sessions: {error}"))
}

/// Restores the scratch session in `workspace`, which must be one that
/// [`recoverable_sessions`] offers (TYP-06: the page names no other folder).
///
/// # Errors
///
/// The message of a workspace that is not offered or cannot be restored.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn recover_session(shell: State<'_, Shell>, workspace: String) -> Result<(), String> {
    let projects = shell.app().projects();
    let offered = projects
        .recoverable_scratch_sessions()
        .map_err(|error| format!("cannot list the unsaved sessions: {error}"))?;
    let session = offered
        .iter()
        .find(|session| session.workspace == Path::new(&workspace))
        .ok_or_else(|| format!("{workspace} holds no unsaved session"))?;
    projects
        .recover_session(&session.workspace)
        .map(|project| log::info!("recovered “{}” from {workspace}", project.name))
        .map_err(|error| format!("cannot recover {workspace}: {error}"))
}

// -- the data layer ----------------------------------------------------------------------

/// The project view the panels draw.
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
pub fn project_view(shell: State<'_, Shell>) -> Result<ProjectView, ErrorNotice> {
    Ok(view_of(shell.app().session(), shell.app().plugins()))
}

/// The views of the tracks `track_ids`, for the refresh of an edit that changed only their
/// mixer settings (`sonara_app::view::tracks_view`).
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn project_tracks(
    shell: State<'_, Shell>,
    track_ids: Vec<String>,
) -> Result<TracksView, ErrorNotice> {
    Ok(tracks_view(
        shell.app().session(),
        shell.app().plugins(),
        &track_ids,
    ))
}

/// The views of the MIDI clips `clip_ids`, for the refresh of an edit that changed only their
/// notes (`sonara_app::view::clips_view`).
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn project_clips(
    shell: State<'_, Shell>,
    clip_ids: Vec<String>,
) -> Result<ClipsView, ErrorNotice> {
    Ok(clips_view(shell.app().session(), &clip_ids))
}

/// The views of the devices `device_ids`, for the refresh of an edit that changed only their
/// state (`sonara_app::view::devices_view`).
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn project_devices(
    shell: State<'_, Shell>,
    device_ids: Vec<String>,
) -> Result<DevicesView, ErrorNotice> {
    Ok(devices_view(
        shell.app().session(),
        shell.app().plugins(),
        &device_ids,
    ))
}

/// Runs one edit (`sonara_app::edit::Edit`, validated here) with the source `"gui"`.
///
/// # Errors
///
/// The edit's error with its code: `INVALID_ARGUMENT` for an edit that does not parse, and the
/// command's own code otherwise.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn edit(shell: State<'_, Shell>, edit: Value) -> Result<EditOutcome, ErrorNotice> {
    edit::apply(shell.app(), Edit::from_json(edit)?)
}

/// Undoes the last step and says so in the status bar ("Undo …", or why not).
///
/// # Errors
///
/// `NO_PROJECT` or `NOTHING_TO_UNDO`.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn undo(app: AppHandle, shell: State<'_, Shell>) -> Result<EditOutcome, ErrorNotice> {
    report_step(&app, edit::undo(shell.app()))
}

/// Redoes the last undone step and says so in the status bar.
///
/// # Errors
///
/// `NO_PROJECT` or `NOTHING_TO_REDO`.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn redo(app: AppHandle, shell: State<'_, Shell>) -> Result<EditOutcome, ErrorNotice> {
    report_step(&app, edit::redo(shell.app()))
}

/// The bridge's `undo`/`redo`: the label, or the reason, as a status message.
fn report_step(
    app: &AppHandle,
    step: Result<EditOutcome, ErrorNotice>,
) -> Result<EditOutcome, ErrorNotice> {
    let text = match &step {
        Ok(outcome) => outcome.label.clone(),
        Err(error) => error.message.clone(),
    };
    show_message(app, &StatusMessage::info(text));
    step
}

/// Ends a gesture: the next edit starts a new undo step.
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn seal_undo(shell: State<'_, Shell>) -> Result<(), ErrorNotice> {
    edit::seal_undo(shell.app());
    Ok(())
}

/// The selection's current track gets the live MIDI focus when it is an instrument track.
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn set_midi_focus(
    shell: State<'_, Shell>,
    track_id: Option<String>,
) -> Result<(), ErrorNotice> {
    shell.app().focus_midi(track_id.as_deref());
    Ok(())
}

/// The response curve of the parametric EQ `device_id` (the device editor's curve).
///
/// # Errors
///
/// `NO_PROJECT`, `NOT_FOUND`, or `INVALID_ARGUMENT` for a device that is not a parametric EQ.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn eq_response(shell: State<'_, Shell>, device_id: String) -> Result<EqResponse, ErrorNotice> {
    let app = shell.app();
    Ok(eq_response::curve(
        app.session(),
        app.plugins(),
        &device_id,
    )?)
}

/// The waveform pyramid of the audio asset `asset_id` at the project rate: ready, or pending
/// with a `peaks-ready` event to follow; its tiles come from the `peaks:` scheme.
///
/// # Errors
///
/// `NO_PROJECT`, or `NOT_FOUND` for an unknown asset.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn peaks(shell: State<'_, Shell>, asset_id: String) -> Result<PeaksInfo, ErrorNotice> {
    Ok(shell.app().peaks().info(&asset_id)?)
}

/// Subscribes the calling web view to the live frames; a second call replaces its channel.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn subscribe_live(
    webview: Webview,
    shell: State<'_, Shell>,
    channel: Channel<InvokeResponseBody>,
) {
    shell.live().subscribe(webview.label(), channel);
}

/// Undo or Redo from the Edit menu, off the main thread: the session may be busy with
/// an agent's edit.
pub fn step_from_menu(app: &AppHandle, redo: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let shell = app.state::<Shell>();
        let step = if redo {
            edit::redo(shell.app())
        } else {
            edit::undo(shell.app())
        };
        // The outcome is in the status bar; there is no caller to hand it to.
        let _shown = report_step(&app, step);
    });
}

// -- the transport bar -------------------------------------------------------------------

/// Plays from `start_beat`, looping when `looped`.
///
/// # Errors
///
/// The transport's error with its code (`ENGINE_UNAVAILABLE`, `NO_PROJECT`, …).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn transport_play(
    shell: State<'_, Shell>,
    start_beat: f64,
    looped: bool,
) -> Result<Value, ErrorNotice> {
    transport_control::play(shell.app(), start_beat, looped)
}

/// Stops; a recording that ended says "Recorded N take(s)" in the status bar.
///
/// # Errors
///
/// The errors of finishing a recording.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn transport_stop(app: AppHandle, shell: State<'_, Shell>) -> Result<Stopped, ErrorNotice> {
    let stopped = transport_control::stop(shell.app())?;
    if let Some(message) = &stopped.message {
        show_message(&app, message);
    }
    Ok(stopped)
}

/// Records the armed tracks from `start_beat`.
///
/// # Errors
///
/// The transport's error with its code.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn transport_record(shell: State<'_, Shell>, start_beat: f64) -> Result<Value, ErrorNotice> {
    transport_control::record(shell.app(), start_beat)
}

/// Moves the transport to `beat`.
///
/// # Errors
///
/// `ENGINE_UNAVAILABLE` or `NO_PROJECT`.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn transport_seek(shell: State<'_, Shell>, beat: f64) -> Result<Value, ErrorNotice> {
    transport_control::seek(shell.app(), beat)
}

/// The engine label's status (`transport.status()`), `null` before the transport exists.
///
/// # Errors
///
/// None; see [`project_view`].
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Tauri passes command arguments by value; async commands return a Result"
)]
pub fn transport_status(shell: State<'_, Shell>) -> Result<Option<Value>, ErrorNotice> {
    Ok(transport_control::engine_status(shell.app()))
}

// -- imports and the window's drops ------------------------------------------------------

/// Imports the audio files `paths` on a job, at `start_beat`, into the audio track
/// `track_id` or into new tracks; returns the job's operation ID. The job's progress and its
/// files' messages reach the status bar.
///
/// # Errors
///
/// The job's submission error (`LIMIT_EXCEEDED` with too many jobs running).
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn import_audio_files(
    shell: State<'_, Shell>,
    paths: Vec<String>,
    track_id: Option<String>,
    start_beat: f64,
) -> Result<String, ErrorNotice> {
    let paths = paths.into_iter().map(PathBuf::from).collect();
    shell
        .app()
        .media()
        .import_audio_job(paths, track_id, start_beat)
        .map(|job| job.id().to_owned())
        .map_err(|error| ErrorNotice::from(&error))
}

/// TL-08 "Import Audio Here…": the open panel for several audio files, starting in `~/Music`,
/// then an import job at `start_beat` into `track_id`. Returns at once; a cancelled panel does
/// nothing.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn import_audio_dialog(app: AppHandle, track_id: Option<String>, start_beat: f64) {
    let extensions: Vec<&str> = AUDIO_IMPORT_SUFFIXES
        .iter()
        .map(|suffix| suffix.trim_start_matches('.'))
        .collect();
    let panel = crate::prompt::Panel {
        title: "Import Audio",
        directory: std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Music"))
            .filter(|music| music.is_dir()),
        file_name: None,
        filter: Some(("Audio files", extensions)),
    };
    let handle = app.clone();
    crate::prompt::pick_files(&app, &panel, move |paths| {
        if paths.is_empty() {
            return;
        }
        let shell = handle.state::<Shell>();
        if let Err(error) = shell
            .app()
            .media()
            .import_audio_job(paths, track_id, start_beat)
        {
            show_message(
                &handle,
                &StatusMessage::error(ErrorNotice::from(&error).message),
            );
        }
    });
}

/// Opens the dropped project `path`, after the unsaved-changes prompt.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn open_project_file(app: AppHandle, path: String) {
    crate::lifecycle::open_project(&app, Some(PathBuf::from(path)));
}

/// The panes the page shows now: saved with the window layout, and checked in the View
/// menu.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn set_pane_layout(app: AppHandle, shell: State<'_, Shell>, panes: PaneLayout) {
    shell.set_panes(panes);
    if let Some(menu) = app.try_state::<MenuHandles>()
        && let Err(error) = menu.show_panes(&shell.panes())
    {
        log::warn!("cannot check the View menu's panes: {error}");
    }
}

// -- the mixer ----------------------------------------------------------------------------

/// The picker of every registered plugin of `kind`, grouped by category.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn plugin_menu(shell: State<'_, Shell>, kind: PickerKind) -> Vec<PluginGroup> {
    catalog::plugin_menu(shell.app().plugins(), kind.into())
}

/// Opens the mixer in a window of its own, or brings that window to the front.
///
/// # Errors
///
/// The message of a window that cannot be opened.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn open_mixer_window(app: AppHandle) -> Result<(), String> {
    crate::window::open_mixer(&app)
        .map_err(|error| format!("cannot open the mixer window: {error}"))
}

// -- the piano roll -----------------------------------------------------------------------

/// The drum kit's plugin and the GM names of its notes, for the piano roll's keyboard on drum
/// tracks (`sonara_app::catalog::drum_map`).
#[tauri::command]
pub fn drum_map() -> DrumMap {
    catalog::drum_map()
}

//! The AI composer's side of the desktop: checks that Claude Code can run, starts and stops a
//! headless run ([`sonara_app::composer`]) and streams its events to the panel in the main
//! window.
//!
//! The run edits the open project through this window's agent session, so the timeline, the
//! mixer and the piano roll update as the pump delivers its edits like any other (the page needs
//! nothing from the composer for that). The panel is not a modal dialog: the window and the menu
//! stay usable while the agent works, and the user can Undo, play or stop it at any time.
//!
//! One run at a time. The undo history is sealed when a run starts and when it ends, so the user's
//! next edit never merges into the agent's last step.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use sonara_app::about;
use sonara_app::composer::{self, Event, Run, Workspace};
use sonara_app::edit;
use sonara_app::events::ErrorNotice;
use tauri::State;
use tauri::ipc::Channel;

use crate::shell::Shell;

/// The run in progress, if any.
#[derive(Default)]
pub struct Composer {
    run: Mutex<Option<Run>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Composer {
    /// Whether a run is going.
    fn is_running(&self) -> bool {
        lock(&self.run)
            .as_ref()
            .is_some_and(|run| !run.is_finished())
    }

    /// Stops the run in progress; does nothing without one.
    pub fn stop(&self) {
        if let Some(run) = lock(&self.run).as_ref() {
            run.stop();
        }
    }
}

/// What the panel needs to know before it offers to start (`composer_check`).
#[derive(Debug, Serialize)]
pub struct Status {
    /// Whether a run can start now.
    pub ready: bool,
    /// Whether a run is going.
    pub running: bool,
    /// The Claude Code executable, when found.
    pub claude_path: Option<String>,
    /// Its version, when found.
    pub claude_version: Option<String>,
    /// Why a run cannot start, worded for the user.
    pub problem: Option<String>,
}

fn failure(code: &'static str, message: impl Into<String>) -> ErrorNotice {
    ErrorNotice {
        code,
        message: message.into(),
    }
}

/// The `sonara` CLI that ships beside the application: the MCP server of the run.
fn sonara_cli() -> Result<PathBuf, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("cannot locate the application: {error}"))?;
    let cli = about::bundled_cli(&executable);
    if cli.is_file() {
        Ok(cli)
    } else {
        Err(format!(
            "the sonara command line tool is missing at {} (a development build needs \
             `cargo build -p sonara` first)",
            cli.display()
        ))
    }
}

/// The discovery file of this window's agent session, which the run is pinned to.
fn discovery_file(shell: &Shell) -> Result<PathBuf, String> {
    shell
        .agent()
        .discovery
        .as_deref()
        .map(Path::new)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            "agents cannot attach to this window (it was started with --no-ipc or the agent \
             server did not start)"
                .to_owned()
        })
}

/// Checks that Claude Code is installed and recent enough and that this window accepts agents.
/// Runs `claude --version`, so it is async.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn composer_check(shell: State<'_, Shell>, composer: State<'_, Composer>) -> Status {
    let running = composer.is_running();
    let (claude, problem) = match composer::locate() {
        Ok(claude) => {
            let problem = sonara_cli().err().or_else(|| discovery_file(&shell).err());
            (Some(claude), problem)
        }
        Err(error) => (None, Some(error.to_string())),
    };
    Status {
        ready: claude.is_some() && problem.is_none() && !running,
        running,
        claude_path: claude.as_ref().map(|c| c.path.display().to_string()),
        claude_version: claude.as_ref().map(|c| c.version.to_string()),
        problem,
    }
}

/// Starts a run that composes from `prompt`, and streams its events to `channel` until the
/// process has exited.
///
/// # Errors
///
/// `INVALID_ARGUMENT` for an empty or too long prompt; `CONFLICT` while a run is going;
/// `ENGINE_UNAVAILABLE` when Claude Code, the `sonara` tool or the agent session is missing;
/// `IO_ERROR` when the process cannot be started.
#[tauri::command(async)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn composer_start(
    shell: State<'_, Shell>,
    composer: State<'_, Composer>,
    prompt: String,
    channel: Channel<Event>,
) -> Result<(), ErrorNotice> {
    let prompt = composer::check_prompt(&prompt)
        .map_err(|error| failure("INVALID_ARGUMENT", error.to_string()))?;
    let claude =
        composer::locate().map_err(|error| failure("ENGINE_UNAVAILABLE", error.to_string()))?;
    let cli = sonara_cli().map_err(|message| failure("ENGINE_UNAVAILABLE", message))?;
    let discovery =
        discovery_file(&shell).map_err(|message| failure("ENGINE_UNAVAILABLE", message))?;
    // PERF-07: the process and the files above are done before the lock is taken.
    let mut slot = lock(&composer.run);
    if slot.as_ref().is_some_and(|run| !run.is_finished()) {
        return Err(failure("CONFLICT", "a composition is already running"));
    }
    let workspace = Workspace::create(&discovery)
        .map_err(|error| failure("IO_ERROR", format!("cannot prepare the run: {error}")))?;
    let invocation = composer::build(&claude.path, &cli, &workspace, prompt);

    let app = std::sync::Arc::clone(shell.app());
    edit::seal_undo(&app);
    let run = Run::start(&invocation, workspace, move |event| {
        let exited = matches!(event, Event::Exited { .. });
        if channel.send(event).is_err() {
            log::debug!("composer: the panel is gone");
        }
        if exited {
            edit::seal_undo(&app);
        }
    })
    .map_err(|error| failure("IO_ERROR", error.to_string()))?;
    *slot = Some(run);
    Ok(())
}

/// Stops the run in progress: it is interrupted, so that Claude Code still reports its result.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn composer_stop(composer: State<'_, Composer>) {
    composer.stop();
}

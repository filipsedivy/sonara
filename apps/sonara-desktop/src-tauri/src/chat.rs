//! The AI Chat's side of the desktop: the commands of the chat window, and the events that
//! stream back to it ([`sonara_app::chat`]).
//!
//! The chat's state lives in `sonara-app`'s [`ChatController`]; this module only connects it to
//! Tauri. The window (label `chat`) is a view: it asks for a snapshot when it opens or reloads,
//! then follows the `chat-event` stream, which is sent to that window alone.
//!
//! A run edits the open project through this window's agent session, so the timeline, the mixer
//! and the piano roll update as the pump delivers its edits like any other. The undo history is
//! sealed when a task starts running and when it ends ([`Sealer`]), so the user's next edit never
//! merges into the agent's last step.
//!
//! The history belongs to the chat window: closing it wipes the chat ([`window_closed`]), and so
//! does replacing the open project ([`project_replaced`]). Quitting ends the run ([`shutdown`]).
//!
//! Every command that can start a process or detect a provider runs on a blocking thread
//! ([`blocking`]), so the UI thread never waits on one. Errors reject with the chat's
//! `{ code, message }` ([`ChatError`]).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use sonara_app::about;
use sonara_app::chat::providers::default_providers;
use sonara_app::chat::{
    ChatConfig, ChatController, ChatError, ChatEvent, ChatSnapshot, ChatState, ProviderId,
    ProviderStatus, Sink, TaskId,
};
use sonara_app::context::AppContext;
use sonara_app::edit;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::shell::Shell;
use crate::window::CHAT;

/// The event the chat window listens to (`src/lib/chat/api.ts`).
const EVENT: &str = "chat-event";

/// The chat of this process: its controller, and what a run needs from the installation.
pub struct ChatHost {
    controller: ChatController,
    cli: PathBuf,
    /// The discovery file of this window's agent session, when agents can attach to it.
    discovery: Option<PathBuf>,
}

impl ChatHost {
    /// Whether a run could attach to this window now: the `sonara` tool that ships beside the
    /// application is there, and the agent server started.
    fn check_tools(&self) -> Result<(), ChatError> {
        if !self.cli.is_file() {
            return Err(ChatError::Conflict(format!(
                "the sonara command line tool is missing at {} (a development build needs \
                 `cargo build -p sonara` first)",
                self.cli.display()
            )));
        }
        if self.discovery.is_none() {
            return Err(ChatError::Conflict(
                "agents cannot attach to this window (it was started with --no-ipc or the agent \
                 server did not start)"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

/// Tells when to seal the undo history: whenever the running task changes, that is, when a task
/// starts, and when it reaches a final state (or the chat is wiped under it).
///
/// It watches [`ChatEvent::Changed`], which the controller delivers one at a time and in order,
/// so the transitions are seen exactly as they happened. The chat module itself knows nothing
/// of the undo history.
#[derive(Debug, Default)]
struct Sealer {
    running: Option<(u64, TaskId)>,
}

impl Sealer {
    /// Whether `event` starts or ends a running task.
    fn observe(&mut self, event: &ChatEvent) -> bool {
        let ChatEvent::Changed { state } = event else {
            return false;
        };
        let now = state
            .tasks
            .iter()
            .find(|task| task.state == sonara_app::chat::TaskState::Running)
            .map(|task| (state.generation, task.id));
        std::mem::replace(&mut self.running, now) != now
    }
}

/// The controller's sink: seals the undo history at a task's start and end, and sends the event
/// to the chat window. A window that is closed is not an error.
fn sink(app: &AppHandle, context: Arc<AppContext>) -> Sink {
    let app = app.clone();
    let sealer = Mutex::new(Sealer::default());
    Arc::new(move |event| {
        let seal = sealer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .observe(&event);
        if seal {
            edit::seal_undo(&context);
        }
        if let Err(error) = app.emit_to(CHAT, EVENT, &event) {
            log::debug!("chat: the window is gone: {error}");
        }
    })
}

/// Builds the chat and manages it. Called once at startup, before the first window opens and
/// before the event pump starts.
pub fn install(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let cli = std::env::current_exe().map_or_else(
        |error| {
            log::warn!("chat: cannot locate the application: {error}");
            PathBuf::from("sonara")
        },
        |executable| about::bundled_cli(&executable),
    );
    let discovery = shell.agent().discovery.as_deref().map(PathBuf::from);
    let config = ChatConfig {
        sonara_cli: cli.clone(),
        // Without a discovery file `check_tools` refuses every task before it reaches a run.
        discovery_file: discovery.clone().unwrap_or_default(),
    };
    let controller = ChatController::new(
        default_providers(),
        config,
        sink(app, Arc::clone(shell.app())),
    );
    app.manage(ChatHost {
        controller,
        cli,
        discovery,
    });
}

/// The chat window was destroyed: the history belongs to the window, so the run stops and the
/// chat is wiped. Runs off the UI thread, since the wipe tells the sink, which seals the undo
/// history.
pub fn window_closed(app: &AppHandle) {
    wipe(app, "the window closed");
}

/// The open project was replaced (a project loaded or closed): the conversation is about the old
/// one, so the chat is wiped. A save does not replace it.
pub fn project_replaced(app: &AppHandle) {
    wipe(app, "the project was replaced");
}

fn wipe(app: &AppHandle, why: &'static str) {
    let Some(host) = app.try_state::<ChatHost>() else {
        return;
    };
    let controller = host.controller.clone();
    let spawned = std::thread::Builder::new()
        .name("sonara-chat-clear".to_owned())
        .spawn(move || {
            let _ = controller.clear();
        });
    if let Err(error) = spawned {
        log::warn!("chat: cannot wipe the chat ({why}): {error}");
    }
}

/// Sonara quits: ends the run (it waits a bounded time, see `ChatController::shutdown`, so that
/// an agent that ignores `SIGINT` is not left running) and refuses further calls.
pub fn shutdown(app: &AppHandle) {
    if let Some(host) = app.try_state::<ChatHost>() {
        host.controller.shutdown();
    }
}

/// Runs `call` on a blocking thread with the controller, so that detecting a provider or
/// starting a process never blocks the UI thread.
async fn blocking<T, F>(host: &State<'_, ChatHost>, call: F) -> Result<T, ChatError>
where
    T: Send + 'static,
    F: FnOnce(ChatController) -> Result<T, ChatError> + Send + 'static,
{
    let controller = host.controller.clone();
    tauri::async_runtime::spawn_blocking(move || call(controller))
        .await
        .map_err(|error| ChatError::Conflict(format!("the chat call did not finish: {error}")))?
}

/// The thread and the logs of the tasks, for a window that opens or reloads.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn chat_snapshot(host: State<'_, ChatHost>) -> ChatSnapshot {
    host.controller.snapshot()
}

/// Detects Claude Code and the other agents; it runs their `--version`, so it may take a second.
///
/// # Errors
///
/// `CONFLICT` when the blocking thread did not finish.
#[tauri::command]
pub async fn chat_providers(host: State<'_, ChatHost>) -> Result<Vec<ProviderStatus>, ChatError> {
    blocking(&host, |controller| Ok(controller.providers())).await
}

/// Picks the agent of the conversation.
///
/// # Errors
///
/// `PROVIDER_UNAVAILABLE` when it cannot run, `PROVIDER_LOCKED` once the thread has tasks.
#[tauri::command]
pub async fn chat_set_provider(
    host: State<'_, ChatHost>,
    provider: ProviderId,
) -> Result<ChatState, ChatError> {
    blocking(&host, move |controller| controller.set_provider(provider)).await
}

/// Queues a task; it starts at once when nothing runs and the queue is not paused.
///
/// # Errors
///
/// `PROMPT_EMPTY`, `PROMPT_TOO_LONG`, `QUEUE_FULL`, `PROVIDER_UNAVAILABLE`, and `CONFLICT` when
/// the `sonara` tool or the agent session is missing.
#[tauri::command]
pub async fn chat_enqueue(host: State<'_, ChatHost>, prompt: String) -> Result<TaskId, ChatError> {
    host.check_tools()?;
    blocking(&host, move |controller| controller.enqueue(&prompt)).await
}

/// Replaces the prompt of a queued task.
///
/// # Errors
///
/// `PROMPT_EMPTY`, `PROMPT_TOO_LONG`, `TASK_NOT_FOUND`, `TASK_NOT_QUEUED`.
#[tauri::command]
pub async fn chat_edit(
    host: State<'_, ChatHost>,
    id: TaskId,
    prompt: String,
) -> Result<ChatState, ChatError> {
    blocking(&host, move |controller| controller.edit(id, &prompt)).await
}

/// Removes a queued task.
///
/// # Errors
///
/// `TASK_NOT_FOUND`, `TASK_NOT_QUEUED`.
#[tauri::command]
pub async fn chat_remove(host: State<'_, ChatHost>, id: TaskId) -> Result<ChatState, ChatError> {
    blocking(&host, move |controller| controller.remove(id)).await
}

/// Moves a queued task to `to_index` among the queued tasks (`toIndex` in the window's call).
///
/// # Errors
///
/// `TASK_NOT_FOUND`, `TASK_NOT_QUEUED`.
#[tauri::command]
pub async fn chat_move(
    host: State<'_, ChatHost>,
    id: TaskId,
    to_index: usize,
) -> Result<ChatState, ChatError> {
    blocking(&host, move |controller| controller.move_task(id, to_index)).await
}

/// Stops the running task: it becomes `cancelled` and the queue pauses.
///
/// # Errors
///
/// `CONFLICT` when the blocking thread did not finish.
#[tauri::command]
pub async fn chat_stop(host: State<'_, ChatHost>) -> Result<ChatState, ChatError> {
    blocking(&host, |controller| Ok(controller.stop())).await
}

/// Lets a paused queue go on: the next queued task starts.
///
/// # Errors
///
/// `CONFLICT` when the blocking thread did not finish.
#[tauri::command]
pub async fn chat_resume(host: State<'_, ChatHost>) -> Result<ChatState, ChatError> {
    host.check_tools()?;
    blocking(&host, |controller| Ok(controller.resume())).await
}

/// Stops everything and wipes the thread; a new generation starts.
///
/// # Errors
///
/// `CONFLICT` when the blocking thread did not finish.
#[tauri::command]
pub async fn chat_clear(host: State<'_, ChatHost>) -> Result<ChatState, ChatError> {
    blocking(&host, |controller| Ok(controller.clear())).await
}

#[cfg(test)]
mod tests {
    use sonara_app::chat::{ProviderId, QueueMode, TaskState, TaskView};

    use super::*;

    fn task(id: u64, state: TaskState) -> TaskView {
        TaskView {
            id: TaskId::from_raw(id),
            prompt: "p".to_owned(),
            provider: ProviderId::Claude,
            state,
            error: None,
            result: None,
        }
    }

    fn changed(generation: u64, tasks: Vec<TaskView>) -> ChatEvent {
        ChatEvent::Changed {
            state: ChatState {
                generation,
                mode: QueueMode::Running,
                provider: ProviderId::Claude,
                provider_locked: !tasks.is_empty(),
                tasks,
            },
        }
    }

    #[test]
    fn the_undo_history_is_sealed_when_a_task_starts_and_when_it_ends() {
        let mut sealer = Sealer::default();
        let steps = [
            // queued behind nothing: not running yet
            (changed(1, vec![task(1, TaskState::Queued)]), false),
            (changed(1, vec![task(1, TaskState::Running)]), true),
            // another change while it runs (a second task queued): no seal
            (
                changed(
                    1,
                    vec![task(1, TaskState::Running), task(2, TaskState::Queued)],
                ),
                false,
            ),
            (
                changed(
                    1,
                    vec![task(1, TaskState::Done), task(2, TaskState::Queued)],
                ),
                true,
            ),
            (
                changed(
                    1,
                    vec![task(1, TaskState::Done), task(2, TaskState::Running)],
                ),
                true,
            ),
            (
                changed(
                    1,
                    vec![task(1, TaskState::Done), task(2, TaskState::Failed)],
                ),
                true,
            ),
            // nothing runs: the same state again does not seal
            (
                changed(
                    1,
                    vec![task(1, TaskState::Done), task(2, TaskState::Failed)],
                ),
                false,
            ),
        ];
        for (index, (event, seals)) in steps.iter().enumerate() {
            assert_eq!(sealer.observe(event), *seals, "step {index}");
        }
    }

    #[test]
    fn a_wipe_under_a_running_task_seals_and_run_events_never_do() {
        let mut sealer = Sealer::default();
        assert!(sealer.observe(&changed(1, vec![task(1, TaskState::Running)])));
        let run = ChatEvent::Run {
            task_id: TaskId::from_raw(1),
            generation: 1,
            event: sonara_app::composer::Event::Text {
                text: "hi".to_owned(),
            },
        };
        assert!(!sealer.observe(&run), "a run event is not a transition");
        assert!(
            sealer.observe(&changed(2, Vec::new())),
            "the wipe ends the task"
        );
        // The first task of the new generation reuses the id: it is a new run all the same.
        assert!(sealer.observe(&changed(2, vec![task(1, TaskState::Running)])));
        assert!(sealer.observe(&changed(3, vec![task(1, TaskState::Running)])));
    }
}

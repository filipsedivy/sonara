//! The chat's controller: it owns one conversation, runs its tasks one after the other through a
//! [`Provider`], and tells the window about every change.
//!
//! **Threads.** Calls come from the window's command threads (never the UI thread: starting a
//! task detects the provider and starts a process, which can take a moment). Each run reports
//! on the emitter thread of its own ([`crate::composer::Run`]); the last event of a run,
//! [`Event::Exited`], finishes the task there and starts the next one.
//!
//! **Locks.** One mutex guards the state, the run handle, the event logs and the outbox. Calls
//! never wait on a process: stopping is a signal and returns at once (RT-06's spirit, for the
//! host side), and a process is started, and the provider detected, with no lock held. The one
//! exception is [`ChatController::shutdown`], which waits a bounded time for the process to end,
//! with no lock held, because the application is about to exit and a timer thread would die
//! with it.
//!
//! **Delivery.** Events go into an outbox under the state lock and are delivered by one thread
//! at a time, outside the lock, in the order the state changed. So the sink never runs under
//! the state lock (it may call back into the controller), and a window never sees a state older
//! than one it saw before. A burst of [`ChatEvent::Changed`] that has not been delivered yet
//! collapses into the newest. A panic in the sink is caught at that boundary (ERR-07: a safety
//! net for a defect in the sink): the event is lost, but the run's thread lives on, so that the
//! exit of the run is still handled and the queue goes on. The [`ChatController::snapshot`] is
//! what the window has been sent so far: the logs leave out the run events that are queued in
//! the outbox, which the window gets after it, so that it never applies one twice.
//!
//! **Generations.** [`ChatController::clear`] and [`ChatController::on_project_replaced`] wipe
//! the chat and start a new generation. A run of the old generation is stopped and its events
//! are dropped, and no new run starts until its process has ended, so that two agents never
//! edit the project at once, even though ending a stubborn process takes up to 15 s.
//!
//! **Outcome.** A task is `Cancelled` only when the host asked to stop it
//! (`Exited { stopped: true }`), never because of an exit code. It is `Done` when the run
//! reported a successful result and the process exited with 0. Anything else is `Failed` with a
//! reason, and a failed or cancelled task pauses the queue.

use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::error::ChatError;
use super::event::{ChatEvent, ChatSnapshot, TaskLog};
use super::provider::{Availability, Provider, ProviderStatus};
use super::replay;
use super::state::Chat;
use super::task::{ChatState, Outcome, ProviderId, TaskId, TaskResult, TaskView};
use crate::composer::Event;

#[cfg(unix)]
use super::provider::Turn;
#[cfg(unix)]
use crate::composer::{Run, Workspace};

/// The most events the log of one task keeps (the oldest go first).
pub const MAX_LOG_EVENTS: usize = 2_000;

/// How long [`ChatController::shutdown`] waits for a run to end after `SIGINT`, before it kills
/// the run's process group.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// How many trailing lines of standard error a failure message may quote.
const STDERR_LINES: usize = 3;

/// The longest quoted line of standard error, in characters.
const STDERR_LINE_CHARS: usize = 300;

/// Where the chat's runs get their tools from.
#[derive(Debug, Clone)]
pub struct ChatConfig {
    /// The bundled `sonara` executable the agent's MCP server runs.
    pub sonara_cli: PathBuf,
    /// The discovery file of the window the agent must edit.
    pub discovery_file: PathBuf,
}

/// Where the chat's events go. It is called from any thread, one call at a time and in order,
/// and may call back into the controller.
pub type Sink = Arc<dyn Fn(ChatEvent) + Send + Sync>;

#[cfg(unix)]
type RunHandle = Run;

/// Without processes there is no run to stop.
#[cfg(not(unix))]
#[derive(Debug)]
struct RunHandle;

#[cfg(not(unix))]
impl RunHandle {
    fn stop(&self) {}

    fn stop_now(&self, _grace: Duration) {}
}

/// What a run reported as its result.
#[derive(Debug)]
struct Reported {
    success: bool,
    subtype: String,
    result: TaskResult,
}

/// The run of the task that is going now.
#[derive(Debug)]
struct Active {
    task_id: TaskId,
    generation: u64,
    provider_name: String,
    /// `None` between the task starting and the process being started.
    run: Option<RunHandle>,
    /// A stop was asked for; applied as soon as there is a run to stop.
    stop_requested: bool,
    /// Why the host ended the run itself, which makes the task failed even though it stopped.
    abort: Option<String>,
    reported: Option<Reported>,
    stderr_tail: VecDeque<String>,
}

impl Active {
    fn stop(&mut self) {
        self.stop_requested = true;
        if let Some(run) = &self.run {
            run.stop();
        }
    }
}

/// The bounded event logs of the tasks.
#[derive(Debug, Default)]
struct Logs {
    tasks: Vec<(TaskId, VecDeque<Event>)>,
}

impl Logs {
    fn push(&mut self, id: TaskId, event: &Event) {
        let index = self
            .tasks
            .iter()
            .position(|(task, _)| *task == id)
            .unwrap_or_else(|| {
                self.tasks.push((id, VecDeque::new()));
                self.tasks.len() - 1
            });
        let Some((_, events)) = self.tasks.get_mut(index) else {
            return;
        };
        if let (Event::Text { text }, Some(Event::Text { text: last })) = (event, events.back_mut())
        {
            last.push_str(text);
            return;
        }
        events.push_back(event.clone());
        while events.len() > MAX_LOG_EVENTS {
            events.pop_front();
        }
    }

    /// Every log, as it is.
    #[cfg(test)]
    fn views(&self) -> Vec<TaskLog> {
        self.views_without(std::iter::empty())
    }

    /// The logs as they were before the events in `pending` (the newest pushes, oldest first)
    /// were pushed: each is taken off the end of its task's log again, a text piece off the
    /// end of the text it was merged into. An event that is not at the end of its log (the log
    /// was wiped since) is left alone.
    fn views_without<'a>(
        &self,
        pending: impl DoubleEndedIterator<Item = (TaskId, &'a Event)>,
    ) -> Vec<TaskLog> {
        let mut tasks = self.tasks.clone();
        for (id, event) in pending.rev() {
            let Some((_, events)) = tasks.iter_mut().find(|(task, _)| *task == id) else {
                continue;
            };
            match (event, events.back_mut()) {
                (Event::Text { text }, Some(Event::Text { text: last })) => {
                    if let Some(kept) = last.strip_suffix(text.as_str()) {
                        let kept_len = kept.len();
                        last.truncate(kept_len);
                        if last.is_empty() {
                            events.pop_back();
                        }
                    }
                }
                (event, Some(last)) if last == event => {
                    events.pop_back();
                }
                _ => {}
            }
        }
        tasks
            .into_iter()
            .filter(|(_, events)| !events.is_empty())
            .map(|(task_id, events)| TaskLog {
                task_id,
                events: events.into_iter().collect(),
            })
            .collect()
    }
}

/// Everything the lock guards.
#[derive(Debug)]
struct Core {
    chat: Chat,
    active: Option<Active>,
    logs: Logs,
    outbox: VecDeque<ChatEvent>,
    /// Whether a thread is delivering the outbox.
    delivering: bool,
    /// The run event the delivering thread has taken out of the outbox and whose sink call has
    /// not returned yet. The window may not have it, so a snapshot leaves it out.
    in_flight: Option<(TaskId, Event)>,
    /// After [`ChatController::shutdown`]: nothing starts and nothing is sent.
    closed: bool,
}

impl Core {
    /// Queues the new state for the window, replacing a state that was not delivered yet.
    fn publish(&mut self) {
        if self.closed {
            return;
        }
        let state = self.chat.view();
        if let Some(ChatEvent::Changed { state: last }) = self.outbox.back_mut() {
            *last = state;
        } else {
            self.outbox.push_back(ChatEvent::Changed { state });
        }
    }

    fn emit(&mut self, event: ChatEvent) {
        if !self.closed {
            self.outbox.push_back(event);
        }
    }
}

struct Inner {
    providers: Vec<Arc<dyn Provider>>,
    config: ChatConfig,
    sink: Sink,
    core: Mutex<Core>,
}

/// One conversation with an agent, shared by the window's commands and the runs' threads.
#[derive(Clone)]
pub struct ChatController {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ChatController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatController").finish_non_exhaustive()
    }
}

impl ChatController {
    /// A controller with an empty chat. The first of `providers` is the conversation's
    /// provider until another is picked.
    #[must_use]
    pub fn new(providers: Vec<Arc<dyn Provider>>, config: ChatConfig, sink: Sink) -> Self {
        let default = providers
            .first()
            .map_or(ProviderId::Claude, |provider| provider.id());
        Self {
            inner: Arc::new(Inner {
                providers,
                config,
                sink,
                core: Mutex::new(Core {
                    chat: Chat::new(default),
                    active: None,
                    logs: Logs::default(),
                    outbox: VecDeque::new(),
                    delivering: false,
                    in_flight: None,
                    closed: false,
                }),
            }),
        }
    }

    /// The state and the logs, for a window that starts or reloads. The logs hold what the
    /// window has been sent: a run event that is still in the outbox is not in them, because
    /// the window gets it after the snapshot.
    #[must_use]
    pub fn snapshot(&self) -> ChatSnapshot {
        let core = self.inner.lock();
        let queued = core.outbox.iter().filter_map(|event| match event {
            ChatEvent::Run { task_id, event, .. } => Some((*task_id, event)),
            ChatEvent::Changed { .. } => None,
        });
        let pending = core
            .in_flight
            .iter()
            .map(|(task_id, event)| (*task_id, event))
            .chain(queued);
        ChatSnapshot {
            state: core.chat.view(),
            logs: core.logs.views_without(pending),
        }
    }

    /// Whether a run is going, or being started or ended. A wiped chat that still waits for the
    /// process of its old generation to end is busy too.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.inner.lock().active.is_some()
    }

    /// Detects every provider. Runs processes and may take a second: call it off the UI thread.
    #[must_use]
    pub fn providers(&self) -> Vec<ProviderStatus> {
        self.inner
            .providers
            .iter()
            .map(|provider| provider.detect())
            .collect()
    }

    /// Picks the agent of the conversation, after checking that it can run. Detects the
    /// provider, so call it off the UI thread.
    ///
    /// # Errors
    ///
    /// [`ChatError::ProviderUnavailable`] when the provider is not registered or not ready (the
    /// reason is its hint); [`ChatError::ProviderLocked`] once the thread has tasks.
    pub fn set_provider(&self, id: ProviderId) -> Result<ChatState, ChatError> {
        let provider = self.inner.provider(id)?;
        let status = provider.detect();
        if status.availability != Availability::Ready {
            return Err(ChatError::ProviderUnavailable {
                provider: id,
                reason: status.hint,
            });
        }
        let state = {
            let mut core = self.inner.lock();
            core.chat.set_provider(id)?;
            core.publish();
            core.chat.view()
        };
        self.inner.deliver();
        Ok(state)
    }

    /// Queues `prompt` and, when nothing runs and the queue is not paused, starts it at once.
    /// Starting a task detects the provider and starts a process: call it off the UI thread.
    ///
    /// # Errors
    ///
    /// [`ChatError::PromptEmpty`], [`ChatError::PromptTooLong`], [`ChatError::QueueFull`];
    /// [`ChatError::ProviderUnavailable`] when the conversation's provider is not registered;
    /// [`ChatError::Conflict`] after [`ChatController::shutdown`]. A provider that cannot run
    /// is not an error here: the task fails, with the provider's hint as the reason.
    pub fn enqueue(&self, prompt: &str) -> Result<TaskId, ChatError> {
        let id = {
            let mut core = self.inner.lock();
            if core.closed {
                return Err(closed());
            }
            self.inner.provider(core.chat.view().provider)?;
            let id = core.chat.enqueue(prompt)?;
            core.publish();
            id
        };
        self.inner.deliver();
        self.inner.pump();
        Ok(id)
    }

    /// Replaces the prompt of a queued task.
    ///
    /// # Errors
    ///
    /// As [`Chat::edit`].
    pub fn edit(&self, id: TaskId, prompt: &str) -> Result<ChatState, ChatError> {
        self.change(|chat| chat.edit(id, prompt))
    }

    /// Removes a queued task.
    ///
    /// # Errors
    ///
    /// As [`Chat::remove`].
    pub fn remove(&self, id: TaskId) -> Result<ChatState, ChatError> {
        self.change(|chat| chat.remove(id))
    }

    /// Moves a queued task to `to_index` among the queued tasks.
    ///
    /// # Errors
    ///
    /// As [`Chat::move_task`].
    pub fn move_task(&self, id: TaskId, to_index: usize) -> Result<ChatState, ChatError> {
        self.change(|chat| chat.move_task(id, to_index))
    }

    fn change(
        &self,
        change: impl FnOnce(&mut Chat) -> Result<(), ChatError>,
    ) -> Result<ChatState, ChatError> {
        let state = {
            let mut core = self.inner.lock();
            if core.closed {
                return Err(closed());
            }
            change(&mut core.chat)?;
            core.publish();
            core.chat.view()
        };
        self.inner.deliver();
        Ok(state)
    }

    /// Stops the running task: the process gets `SIGINT` (and the provider's escalation after
    /// that) and the call returns at once. The task becomes `Cancelled` and the queue pauses
    /// when the process has ended. With nothing running, or only an old generation's process
    /// still ending, it does nothing.
    #[must_use]
    pub fn stop(&self) -> ChatState {
        let mut core = self.inner.lock();
        let generation = core.chat.generation();
        if let Some(active) = core.active.as_mut()
            && active.generation == generation
        {
            active.stop();
        }
        core.chat.view()
    }

    /// Lets a paused queue go on, and starts its next task.
    #[must_use]
    pub fn resume(&self) -> ChatState {
        let (changed, state) = {
            let mut core = self.inner.lock();
            let changed = !core.closed && core.chat.resume();
            if changed {
                core.publish();
            }
            (changed, core.chat.view())
        };
        if changed {
            self.inner.deliver();
            self.inner.pump();
        }
        state
    }

    /// Wipes the conversation: stops the running task, drops the queue and the logs, and starts
    /// a new generation. The user's "Clear history", and the window closing.
    #[must_use]
    pub fn clear(&self) -> ChatState {
        let state = {
            let mut core = self.inner.lock();
            core.wipe();
            core.publish();
            core.chat.view()
        };
        self.inner.deliver();
        state
    }

    /// The open project was replaced: the conversation is about the old one, so it is wiped
    /// like [`ChatController::clear`].
    #[must_use]
    pub fn on_project_replaced(&self) -> ChatState {
        self.clear()
    }

    /// The application ends: stops the run, wipes the chat and refuses further calls. Nothing
    /// is sent to the window any more.
    ///
    /// Unlike [`ChatController::stop`] this waits for the process: `SIGINT`, then up to
    /// [`SHUTDOWN_GRACE`] for it to end, then `SIGTERM` and `SIGKILL` to its process group,
    /// all on the calling thread. A timer thread would not outlive the application, and an
    /// agent that ignores `SIGINT` would be left running after Sonara has quit.
    pub fn shutdown(&self) {
        self.shutdown_within(SHUTDOWN_GRACE);
    }

    /// [`ChatController::shutdown`] with `grace` for `SIGINT`, so that a test need not wait
    /// out [`SHUTDOWN_GRACE`].
    pub fn shutdown_within(&self, grace: Duration) {
        let run = {
            let mut core = self.inner.lock();
            // The run leaves the task here, so that the wipe does not interrupt it a second time.
            let run = core.active.as_mut().and_then(|active| active.run.take());
            core.wipe();
            core.closed = true;
            core.outbox.clear();
            run
        };
        if let Some(run) = run {
            run.stop_now(grace);
        }
    }
}

fn closed() -> ChatError {
    ChatError::Conflict("the chat is closed".to_owned())
}

impl Core {
    /// Stops the run, if any, and empties the chat into a new generation.
    fn wipe(&mut self) {
        if let Some(active) = self.active.as_mut() {
            active.stop();
        }
        self.chat.clear();
        self.logs = Logs::default();
    }
}

/// Frees the delivery flag when the sink panics, so that later events are still delivered.
struct DeliveryGuard<'a> {
    inner: &'a Inner,
    /// Set when the outbox was seen empty and the flag released the normal way.
    finished: bool,
}

impl Drop for DeliveryGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let mut core = self.inner.lock();
            core.delivering = false;
            core.in_flight = None;
        }
    }
}

impl Inner {
    /// Takes the lock; a poisoned lock is still usable, as every change leaves the state whole.
    fn lock(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn provider(&self, id: ProviderId) -> Result<&Arc<dyn Provider>, ChatError> {
        self.providers
            .iter()
            .find(|provider| provider.id() == id)
            .ok_or_else(|| ChatError::ProviderUnavailable {
                provider: id,
                reason: "this build has no such provider".to_owned(),
            })
    }

    /// Delivers the outbox to the sink, outside the lock. Only one thread delivers at a time, so
    /// the order is the order of the state; a call that finds a delivery going leaves its
    /// events to that thread.
    fn deliver(&self) {
        {
            let mut core = self.lock();
            if core.delivering {
                return;
            }
            core.delivering = true;
        }
        let mut guard = DeliveryGuard {
            inner: self,
            finished: false,
        };
        loop {
            let next = {
                let mut core = self.lock();
                let next = core.outbox.pop_front();
                core.in_flight = match &next {
                    Some(ChatEvent::Run { task_id, event, .. }) => Some((*task_id, event.clone())),
                    _ => None,
                };
                if next.is_none() {
                    // Seen empty and released in one step: an event queued after this finds
                    // `delivering` false and delivers itself.
                    core.delivering = false;
                }
                next
            };
            let Some(event) = next else {
                guard.finished = true;
                return;
            };
            // ERR-07: a panic in the sink is a defect in the sink. Catch it here so that the
            // run's thread, which delivers its own events, survives it: the exit of the run is
            // still handled and the queue goes on.
            if catch_unwind(AssertUnwindSafe(|| (self.sink)(event))).is_err() {
                log::error!("chat: the sink panicked; the event was dropped");
            }
            self.lock().in_flight = None;
        }
    }

    /// Starts the next queued task, if the chat is free and not paused.
    fn pump(self: &Arc<Self>) {
        loop {
            let started = {
                let mut core = self.lock();
                if core.closed || core.active.is_some() {
                    return;
                }
                let Some(task) = core.chat.begin_next() else {
                    return;
                };
                let transcript = replay::transcript(core.chat.tasks());
                let provider_name = self
                    .provider(task.provider)
                    .map_or_else(|_| task.provider.to_string(), |p| p.name().to_owned());
                core.active = Some(Active {
                    task_id: task.id,
                    generation: core.chat.generation(),
                    provider_name,
                    run: None,
                    stop_requested: false,
                    abort: None,
                    reported: None,
                    stderr_tail: VecDeque::new(),
                });
                core.publish();
                (task, core.chat.generation(), transcript)
            };
            self.deliver();
            let (task, generation, transcript) = started;
            match self.launch(&task, generation, transcript) {
                Ok(run) => {
                    self.attach(task.id, generation, run);
                    return;
                }
                Err(reason) => self.fail_to_start(task.id, generation, reason),
            }
        }
    }

    /// Finds the provider, builds the run and starts the process. No lock is held.
    #[cfg(unix)]
    fn launch(
        self: &Arc<Self>,
        task: &TaskView,
        generation: u64,
        transcript: String,
    ) -> Result<RunHandle, String> {
        let provider = self
            .provider(task.provider)
            .map_err(|error| error.to_string())?;
        let status = provider.detect();
        let Some(located) = status.located else {
            return Err(format!("{} cannot run: {}", status.name, status.hint));
        };
        let workspace = Workspace::create(&self.config.discovery_file)
            .map_err(|error| format!("cannot prepare the run: {error}"))?;
        let turn = Turn {
            prompt: task.prompt.clone(),
            transcript,
        };
        let invocation = provider.build(&located, &self.config.sonara_cli, &workspace, &turn);
        let inner = Arc::clone(self);
        let task_id = task.id;
        Run::start_with(
            &invocation,
            workspace,
            provider.parser(),
            provider.stop_policy(),
            move |event| inner.on_run_event(task_id, generation, event),
        )
        .map_err(|error| error.to_string())
    }

    #[cfg(not(unix))]
    fn launch(
        self: &Arc<Self>,
        _task: &TaskView,
        _generation: u64,
        _transcript: String,
    ) -> Result<RunHandle, String> {
        Err("the AI Chat needs macOS".to_owned())
    }

    /// Hands the started process to its task, and stops it at once when a stop or a wipe came
    /// in while it was being started.
    fn attach(&self, task_id: TaskId, generation: u64, run: RunHandle) {
        let mut core = self.lock();
        let closed = core.closed;
        let retired = generation != core.chat.generation();
        match core.active.as_mut() {
            // The application quit while the process was being started: nobody is left to stop
            // it later, so end it here, with no lock held.
            Some(active)
                if active.task_id == task_id && active.generation == generation && closed =>
            {
                drop(core);
                run.stop_now(SHUTDOWN_GRACE);
            }
            Some(active) if active.task_id == task_id && active.generation == generation => {
                if retired || active.stop_requested {
                    run.stop();
                }
                active.run = Some(run);
            }
            // The run ended before it could be attached (a process that exits at once): its
            // `Exited` has been handled and another task may be active. A stop on a run that
            // has finished does nothing, and the handle is dropped.
            _ => run.stop(),
        }
    }

    /// The process could not be started: the task fails and the queue pauses.
    fn fail_to_start(&self, task_id: TaskId, generation: u64, reason: String) {
        {
            let mut core = self.lock();
            let ours = core
                .active
                .as_ref()
                .is_some_and(|active| active.task_id == task_id && active.generation == generation);
            if !ours {
                return;
            }
            core.active = None;
            if generation == core.chat.generation() {
                let outcome = Outcome::Failed {
                    reason,
                    result: None,
                };
                if let Err(error) = core.chat.finish(task_id, outcome) {
                    log::warn!("chat: cannot fail task {task_id}: {error}");
                }
                core.publish();
            }
        }
        self.deliver();
    }

    /// One event of a run, on the run's emitter thread.
    fn on_run_event(self: &Arc<Self>, task_id: TaskId, generation: u64, event: Event) {
        let exited = matches!(event, Event::Exited { .. });
        {
            let mut core = self.lock();
            let current = generation == core.chat.generation();
            let ours = core
                .active
                .as_ref()
                .is_some_and(|active| active.task_id == task_id && active.generation == generation);
            if !ours {
                return;
            }
            if exited {
                let active = core.active.take();
                if let (true, Some(active), Event::Exited { code, stopped }) =
                    (current && !core.closed, active, &event)
                {
                    let outcome = outcome(&active, *code, *stopped);
                    core.logs.push(task_id, &event);
                    core.emit(ChatEvent::Run {
                        task_id,
                        generation,
                        event: event.clone(),
                    });
                    if let Err(error) = core.chat.finish(task_id, outcome) {
                        log::warn!("chat: cannot finish task {task_id}: {error}");
                    }
                    core.publish();
                }
                // An event of a wiped generation is dropped; only its end matters, as it frees
                // the chat for the next run.
            } else if current {
                core.on_progress(task_id, generation, event);
            }
        }
        self.deliver();
        if exited {
            self.pump();
        }
    }
}

impl Core {
    /// Notes what a running task reports, logs it and queues it for the window.
    fn on_progress(&mut self, task_id: TaskId, generation: u64, event: Event) {
        if let Some(active) = self.active.as_mut() {
            match &event {
                Event::Init {
                    mcp_servers,
                    mcp_errors,
                    ..
                } if active.abort.is_none()
                    && mcp_servers
                        .iter()
                        .any(|server| server.name == "sonara" && server.status == "failed") =>
                {
                    // Nothing can be composed without the Sonara tools: end the run now.
                    let mut reason = "the Sonara tools could not connect to this window".to_owned();
                    if !mcp_errors.is_empty() {
                        reason.push_str(": ");
                        reason.push_str(&mcp_errors.join("; "));
                    }
                    active.abort = Some(reason);
                    active.stop();
                }
                Event::Result {
                    success, subtype, ..
                } => {
                    if let Some(result) = TaskResult::from_event(&event) {
                        active.reported = Some(Reported {
                            success: *success,
                            subtype: subtype.clone(),
                            result,
                        });
                    }
                }
                Event::Stderr { line } => {
                    active
                        .stderr_tail
                        .push_back(line.chars().take(STDERR_LINE_CHARS).collect());
                    while active.stderr_tail.len() > STDERR_LINES {
                        active.stderr_tail.pop_front();
                    }
                }
                _ => {}
            }
        }
        self.logs.push(task_id, &event);
        self.emit(ChatEvent::Run {
            task_id,
            generation,
            event,
        });
    }
}

/// How a run that has exited ends its task.
fn outcome(active: &Active, code: Option<i32>, stopped: bool) -> Outcome {
    let result = active
        .reported
        .as_ref()
        .map(|reported| reported.result.clone());
    if let Some(reason) = &active.abort {
        return Outcome::Failed {
            reason: reason.clone(),
            result,
        };
    }
    if stopped {
        return Outcome::Cancelled { result };
    }
    let name = &active.provider_name;
    let how = |code: Option<i32>| code.map_or("a signal".to_owned(), |code| format!("code {code}"));
    match &active.reported {
        Some(reported) if reported.success && code == Some(0) => {
            Outcome::Done(reported.result.clone())
        }
        Some(reported) if reported.success => Outcome::Failed {
            reason: format!("{name} reported a result but exited with {}", how(code)),
            result,
        },
        Some(reported) => {
            let detail = reported
                .result
                .text
                .lines()
                .next()
                .unwrap_or_default()
                .trim();
            let reason = if detail.is_empty() {
                format!("{name} ended the run with {}", reported.subtype)
            } else {
                format!("{name} ended the run with {}: {detail}", reported.subtype)
            };
            Outcome::Failed { reason, result }
        }
        None => {
            let mut reason = format!("{name} ended without an answer (exited with {})", how(code));
            if !active.stderr_tail.is_empty() {
                reason.push_str(": ");
                reason.push_str(
                    &active
                        .stderr_tail
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
            }
            Outcome::Failed {
                reason,
                result: None,
            }
        }
    }
}

#[cfg(test)]
mod tests;

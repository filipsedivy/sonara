//! The chat as a pure state machine: no processes, no threads, no clock.
//!
//! The controller ([`super::controller`]) drives it; the rules live here, so that they can be
//! tested exhaustively.
//!
//! - Tasks are in thread order, and the queued ones are always a suffix of the list: a task
//!   is appended as `Queued`, `begin_next` starts the first queued one, and only queued tasks
//!   are edited, removed or moved (among the queued ones).
//! - At most one task is `Running`. A failed or cancelled task pauses the queue; a done one
//!   does not.
//! - A refused call changes nothing.

use super::error::{ChatError, QUEUE_CAPACITY};
use super::task::{
    ChatState, Outcome, ProviderId, QueueMode, TaskId, TaskResult, TaskState, TaskView,
};
use crate::composer::check_prompt;

/// The state of one conversation.
#[derive(Debug, Clone)]
pub struct Chat {
    generation: u64,
    next_id: u64,
    mode: QueueMode,
    provider: ProviderId,
    tasks: Vec<TaskView>,
}

impl Chat {
    /// An empty chat that talks to `provider`, in generation 0.
    #[must_use]
    pub fn new(provider: ProviderId) -> Self {
        Self {
            generation: 0,
            next_id: 1,
            mode: QueueMode::Running,
            provider,
            tasks: Vec::new(),
        }
    }

    /// The generation: it changes on every [`Chat::clear`] (and wraps around on purpose).
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The tasks, in thread order.
    #[must_use]
    pub fn tasks(&self) -> &[TaskView] {
        &self.tasks
    }

    /// The task with `id`.
    #[must_use]
    pub fn task(&self, id: TaskId) -> Option<&TaskView> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// The running task, if there is one.
    #[must_use]
    pub fn running(&self) -> Option<&TaskView> {
        self.tasks
            .iter()
            .find(|task| task.state == TaskState::Running)
    }

    /// The whole chat, as the window shows it.
    #[must_use]
    pub fn view(&self) -> ChatState {
        ChatState {
            generation: self.generation,
            mode: self.mode,
            provider: self.provider,
            provider_locked: !self.tasks.is_empty(),
            tasks: self.tasks.clone(),
        }
    }

    fn queued_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|task| task.state == TaskState::Queued)
            .count()
    }

    fn position(&self, id: TaskId) -> Result<usize, ChatError> {
        self.tasks
            .iter()
            .position(|task| task.id == id)
            .ok_or(ChatError::TaskNotFound(id))
    }

    /// The index of the queued task `id`.
    fn queued_position(&self, id: TaskId) -> Result<usize, ChatError> {
        let position = self.position(id)?;
        let queued = self
            .tasks
            .get(position)
            .is_some_and(|task| task.state == TaskState::Queued);
        if queued {
            Ok(position)
        } else {
            Err(ChatError::TaskNotQueued(id))
        }
    }

    /// Picks the agent of the conversation.
    ///
    /// # Errors
    ///
    /// [`ChatError::ProviderLocked`] once the thread has tasks.
    pub fn set_provider(&mut self, provider: ProviderId) -> Result<(), ChatError> {
        if self.tasks.is_empty() {
            self.provider = provider;
            Ok(())
        } else {
            Err(ChatError::ProviderLocked)
        }
    }

    /// Appends a task for `prompt` at the end of the queue and returns its id. The prompt is
    /// stored trimmed. A paused queue takes tasks as well.
    ///
    /// # Errors
    ///
    /// [`ChatError::PromptEmpty`] or [`ChatError::PromptTooLong`] for a prompt that
    /// [`check_prompt`] refuses; [`ChatError::QueueFull`] when [`QUEUE_CAPACITY`] tasks
    /// already wait.
    pub fn enqueue(&mut self, prompt: &str) -> Result<TaskId, ChatError> {
        let prompt = check_prompt(prompt)?;
        if self.queued_count() >= QUEUE_CAPACITY {
            return Err(ChatError::QueueFull);
        }
        let id = TaskId::from_raw(self.next_id);
        // Ids only have to differ within one chat; the counter wraps instead of overflowing.
        self.next_id = self.next_id.wrapping_add(1);
        self.tasks.push(TaskView {
            id,
            prompt: prompt.to_owned(),
            provider: self.provider,
            state: TaskState::Queued,
            error: None,
            result: None,
        });
        Ok(id)
    }

    /// Replaces the prompt of a queued task.
    ///
    /// # Errors
    ///
    /// The prompt errors of [`Chat::enqueue`]; [`ChatError::TaskNotFound`]; and
    /// [`ChatError::TaskNotQueued`] once the task has started.
    pub fn edit(&mut self, id: TaskId, prompt: &str) -> Result<(), ChatError> {
        let prompt = check_prompt(prompt)?;
        let position = self.queued_position(id)?;
        if let Some(task) = self.tasks.get_mut(position) {
            prompt.clone_into(&mut task.prompt);
        }
        Ok(())
    }

    /// Removes a queued task.
    ///
    /// # Errors
    ///
    /// [`ChatError::TaskNotFound`], and [`ChatError::TaskNotQueued`] once the task has started.
    pub fn remove(&mut self, id: TaskId) -> Result<(), ChatError> {
        let position = self.queued_position(id)?;
        self.tasks.remove(position);
        Ok(())
    }

    /// Moves a queued task to `to_index` among the queued tasks (0 is the next to run). An
    /// index past the end means the end of the queue.
    ///
    /// # Errors
    ///
    /// [`ChatError::TaskNotFound`], and [`ChatError::TaskNotQueued`] once the task has started.
    pub fn move_task(&mut self, id: TaskId, to_index: usize) -> Result<(), ChatError> {
        let position = self.queued_position(id)?;
        // The queued tasks are a suffix of the list; `first` is where it starts.
        let first = self
            .tasks
            .iter()
            .position(|task| task.state == TaskState::Queued)
            .unwrap_or(position);
        let last = self.tasks.len().saturating_sub(1);
        let target = first.saturating_add(to_index).min(last);
        let task = self.tasks.remove(position);
        self.tasks.insert(target, task);
        Ok(())
    }

    /// Starts the first queued task: it becomes `Running`, and a copy of it is returned.
    /// `None` when the queue is paused, a task runs already, or nothing waits.
    pub fn begin_next(&mut self) -> Option<TaskView> {
        if self.mode != QueueMode::Running || self.running().is_some() {
            return None;
        }
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.state == TaskState::Queued)?;
        task.state = TaskState::Running;
        Some(task.clone())
    }

    /// Ends the running task `id`. A failed or cancelled task pauses the queue.
    ///
    /// # Errors
    ///
    /// [`ChatError::TaskNotFound`], and [`ChatError::Conflict`] when the task is not running.
    pub fn finish(&mut self, id: TaskId, outcome: Outcome) -> Result<(), ChatError> {
        let position = self.position(id)?;
        let Some(task) = self.tasks.get_mut(position) else {
            return Err(ChatError::TaskNotFound(id));
        };
        if task.state != TaskState::Running {
            return Err(ChatError::Conflict(format!("task {id} is not running")));
        }
        let (state, error, result): (_, _, Option<TaskResult>) = match outcome {
            Outcome::Done(result) => (TaskState::Done, None, Some(result)),
            Outcome::Failed { reason, result } => (TaskState::Failed, Some(reason), result),
            Outcome::Cancelled { result } => (TaskState::Cancelled, None, result),
        };
        task.state = state;
        task.error = error;
        task.result = result;
        if state != TaskState::Done {
            self.mode = QueueMode::Paused;
        }
        Ok(())
    }

    /// Lets a paused queue go on. Returns whether the mode changed; a queue that runs already
    /// is left alone.
    pub fn resume(&mut self) -> bool {
        let paused = self.mode == QueueMode::Paused;
        self.mode = QueueMode::Running;
        paused
    }

    /// Drops every task and starts a new generation. The caller stops the running process
    /// first. The provider stays; it can change again, as the thread is empty.
    pub fn clear(&mut self) {
        self.tasks.clear();
        self.mode = QueueMode::Running;
        self.generation = self.generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::super::task::UnknownProvider;
    use super::*;
    use crate::composer::invocation::MAX_PROMPT_CHARS;

    fn result(text: &str) -> TaskResult {
        TaskResult {
            text: text.to_owned(),
            duration_ms: Some(1),
            num_turns: Some(1),
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
            session_id: None,
            denied_tools: Vec::new(),
        }
    }

    fn states(chat: &Chat) -> Vec<TaskState> {
        chat.tasks().iter().map(|task| task.state).collect()
    }

    fn prompts(chat: &Chat) -> Vec<&str> {
        chat.tasks()
            .iter()
            .map(|task| task.prompt.as_str())
            .collect()
    }

    /// A chat with the queued tasks "a", "b", "c" and their ids.
    fn queued() -> (Chat, [TaskId; 3]) {
        let mut chat = Chat::new(ProviderId::Claude);
        let ids = ["a", "b", "c"].map(|prompt| chat.enqueue(prompt).unwrap());
        (chat, ids)
    }

    #[test]
    fn a_new_chat_is_empty_unlocked_and_running() {
        let chat = Chat::new(ProviderId::Codex);
        let view = chat.view();
        assert_eq!(view.generation, 0);
        assert_eq!(view.mode, QueueMode::Running);
        assert_eq!(view.provider, ProviderId::Codex);
        assert!(!view.provider_locked);
        assert_eq!(view.tasks.len(), 0);
        assert!(chat.running().is_none());
    }

    #[test]
    fn enqueue_trims_the_prompt_and_numbers_the_tasks_in_order() {
        let mut chat = Chat::new(ProviderId::Claude);
        let first = chat.enqueue("  hello \n").unwrap();
        let second = chat.enqueue("again").unwrap();
        assert_ne!(first, second);
        assert_eq!(prompts(&chat), ["hello", "again"]);
        assert_eq!(states(&chat), [TaskState::Queued; 2]);
        assert_eq!(chat.task(first).unwrap().provider, ProviderId::Claude);
        assert!(chat.view().provider_locked, "the thread has tasks now");
    }

    #[test]
    fn a_task_takes_the_provider_of_the_moment() {
        let mut chat = Chat::new(ProviderId::Claude);
        chat.set_provider(ProviderId::Codex).unwrap();
        let id = chat.enqueue("x").unwrap();
        assert_eq!(chat.task(id).unwrap().provider, ProviderId::Codex);
    }

    #[test]
    fn the_prompt_is_checked_like_the_composer_checks_it() {
        let mut chat = Chat::new(ProviderId::Claude);
        assert_eq!(chat.enqueue("  \n\t"), Err(ChatError::PromptEmpty));
        let long = "é".repeat(MAX_PROMPT_CHARS + 1);
        assert_eq!(chat.enqueue(&long), Err(ChatError::PromptTooLong));
        assert!(
            chat.enqueue(&long[..long.len() - 2]).is_ok(),
            "exactly the limit"
        );
        let (mut chat, [a, ..]) = queued();
        assert_eq!(chat.edit(a, ""), Err(ChatError::PromptEmpty));
        assert_eq!(chat.edit(a, &long), Err(ChatError::PromptTooLong));
        assert_eq!(prompts(&chat)[0], "a", "a refused edit changes nothing");
    }

    #[test]
    fn the_queue_holds_fifty_tasks() {
        let mut chat = Chat::new(ProviderId::Claude);
        for n in 0..QUEUE_CAPACITY {
            chat.enqueue(&format!("task {n}")).unwrap();
        }
        let before = chat.view();
        assert_eq!(chat.enqueue("one too many"), Err(ChatError::QueueFull));
        assert_eq!(chat.view(), before);
        // A running task does not count against the queue: it left it.
        chat.begin_next().unwrap();
        assert!(chat.enqueue("fits now").is_ok());
        assert_eq!(chat.enqueue("full again"), Err(ChatError::QueueFull));
    }

    #[test]
    fn begin_next_starts_the_first_queued_task_and_only_one() {
        let (mut chat, [a, b, _]) = queued();
        let started = chat.begin_next().unwrap();
        assert_eq!(started.id, a);
        assert_eq!(started.state, TaskState::Running);
        assert_eq!(chat.running().unwrap().id, a);
        assert!(chat.begin_next().is_none(), "one task runs at a time");
        chat.finish(a, Outcome::Done(result("ok"))).unwrap();
        assert_eq!(chat.begin_next().unwrap().id, b);
    }

    #[test]
    fn begin_next_has_nothing_to_start_in_an_empty_or_paused_queue() {
        let mut chat = Chat::new(ProviderId::Claude);
        assert!(chat.begin_next().is_none());
        let (mut chat, [a, ..]) = queued();
        chat.begin_next().unwrap();
        chat.finish(
            a,
            Outcome::Failed {
                reason: "boom".into(),
                result: None,
            },
        )
        .unwrap();
        assert_eq!(chat.view().mode, QueueMode::Paused);
        let before = chat.view();
        assert!(chat.begin_next().is_none(), "a paused queue waits");
        assert_eq!(chat.view(), before);
    }

    #[test]
    fn a_done_task_keeps_the_queue_going_and_keeps_its_result() {
        let (mut chat, [a, ..]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Done(result("all set"))).unwrap();
        let task = chat.task(a).unwrap();
        assert_eq!(task.state, TaskState::Done);
        assert_eq!(task.error, None);
        assert_eq!(task.result.as_ref().unwrap().text, "all set");
        assert_eq!(chat.view().mode, QueueMode::Running);
    }

    #[test]
    fn a_failed_task_pauses_the_queue_with_its_reason_and_result() {
        let (mut chat, [a, b, c]) = queued();
        chat.begin_next().unwrap();
        chat.finish(
            a,
            Outcome::Failed {
                reason: "no tools".into(),
                result: Some(result("partial")),
            },
        )
        .unwrap();
        let task = chat.task(a).unwrap();
        assert_eq!(task.state, TaskState::Failed);
        assert_eq!(task.error.as_deref(), Some("no tools"));
        assert_eq!(task.result.as_ref().unwrap().text, "partial");
        let view = chat.view();
        assert_eq!(view.mode, QueueMode::Paused);
        assert_eq!(
            states(&chat),
            [TaskState::Failed, TaskState::Queued, TaskState::Queued]
        );
        assert_eq!(
            view.tasks.iter().map(|t| t.id).collect::<Vec<_>>(),
            [a, b, c]
        );
    }

    #[test]
    fn a_cancelled_task_pauses_the_queue_without_an_error() {
        let (mut chat, [a, ..]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Cancelled { result: None }).unwrap();
        let task = chat.task(a).unwrap();
        assert_eq!(task.state, TaskState::Cancelled);
        assert_eq!(task.error, None);
        assert_eq!(chat.view().mode, QueueMode::Paused);
    }

    #[test]
    fn resume_lets_a_paused_queue_start_the_next_task() {
        let (mut chat, [a, b, _]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Cancelled { result: None }).unwrap();
        assert!(chat.resume(), "the mode changed");
        assert_eq!(chat.view().mode, QueueMode::Running);
        assert_eq!(chat.begin_next().unwrap().id, b);
        assert!(!chat.resume(), "a queue that runs is left alone");
    }

    #[test]
    fn only_a_running_task_can_finish() {
        let (mut chat, [a, ..]) = queued();
        let before = chat.view();
        assert_eq!(
            chat.finish(a, Outcome::Done(result("x")))
                .unwrap_err()
                .code(),
            "CONFLICT",
            "a queued task has not run"
        );
        assert_eq!(
            chat.finish(TaskId::from_raw(999), Outcome::Cancelled { result: None }),
            Err(ChatError::TaskNotFound(TaskId::from_raw(999)))
        );
        assert_eq!(chat.view(), before);
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Done(result("x"))).unwrap();
        let done = chat.view();
        assert_eq!(
            chat.finish(a, Outcome::Cancelled { result: None })
                .unwrap_err()
                .code(),
            "CONFLICT",
            "a final state never changes"
        );
        assert_eq!(chat.view(), done);
    }

    #[test]
    fn edit_changes_only_the_prompt_of_a_queued_task() {
        let (mut chat, [a, b, _]) = queued();
        chat.edit(b, "  better ").unwrap();
        assert_eq!(prompts(&chat), ["a", "better", "c"]);
        chat.begin_next().unwrap();
        let before = chat.view();
        assert_eq!(chat.edit(a, "late"), Err(ChatError::TaskNotQueued(a)));
        assert_eq!(
            chat.edit(TaskId::from_raw(99), "x"),
            Err(ChatError::TaskNotFound(TaskId::from_raw(99)))
        );
        assert_eq!(chat.view(), before);
    }

    #[test]
    fn remove_drops_only_queued_tasks() {
        let (mut chat, [a, b, c]) = queued();
        chat.remove(b).unwrap();
        assert_eq!(prompts(&chat), ["a", "c"]);
        chat.begin_next().unwrap();
        assert_eq!(chat.remove(a), Err(ChatError::TaskNotQueued(a)), "running");
        chat.finish(a, Outcome::Done(result("x"))).unwrap();
        assert_eq!(chat.remove(a), Err(ChatError::TaskNotQueued(a)), "done");
        assert_eq!(chat.remove(b), Err(ChatError::TaskNotFound(b)), "gone");
        assert_eq!(prompts(&chat), ["a", "c"]);
        assert!(chat.remove(c).is_ok());
    }

    #[test]
    fn move_reorders_among_the_queued_tasks_only() {
        let (mut chat, [a, b, c]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Done(result("x"))).unwrap();
        // Queue is now b, c after the done task a.
        chat.move_task(c, 0).unwrap();
        assert_eq!(prompts(&chat), ["a", "c", "b"]);
        assert_eq!(chat.begin_next().unwrap().id, c, "the moved task runs next");
        chat.finish(c, Outcome::Done(result("x"))).unwrap();
        let d = chat.enqueue("d").unwrap();
        chat.move_task(b, 99).unwrap();
        assert_eq!(
            prompts(&chat),
            ["a", "c", "d", "b"],
            "past the end is the end"
        );
        chat.move_task(b, 0).unwrap();
        chat.move_task(b, 0).unwrap();
        assert_eq!(prompts(&chat), ["a", "c", "b", "d"]);
        assert_eq!(chat.begin_next().unwrap().id, b);
        let before = chat.view();
        assert_eq!(chat.move_task(b, 0), Err(ChatError::TaskNotQueued(b)));
        assert_eq!(chat.move_task(a, 0), Err(ChatError::TaskNotQueued(a)));
        assert_eq!(
            chat.move_task(TaskId::from_raw(77), 0),
            Err(ChatError::TaskNotFound(TaskId::from_raw(77)))
        );
        assert_eq!(chat.view(), before);
        assert_eq!(chat.task(d).unwrap().state, TaskState::Queued);
    }

    #[test]
    fn the_provider_is_locked_once_the_thread_has_tasks_and_free_again_after_a_clear() {
        let mut chat = Chat::new(ProviderId::Claude);
        chat.set_provider(ProviderId::Codex).unwrap();
        chat.set_provider(ProviderId::Claude).unwrap();
        chat.enqueue("x").unwrap();
        assert_eq!(
            chat.set_provider(ProviderId::Codex),
            Err(ChatError::ProviderLocked)
        );
        assert_eq!(chat.view().provider, ProviderId::Claude);
        assert!(chat.view().provider_locked);
        chat.clear();
        assert!(!chat.view().provider_locked);
        chat.set_provider(ProviderId::Codex).unwrap();
    }

    #[test]
    fn clear_empties_the_thread_unpauses_and_starts_a_new_generation() {
        let (mut chat, [a, ..]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Cancelled { result: None }).unwrap();
        let generation = chat.generation();
        chat.clear();
        let view = chat.view();
        assert_ne!(view.generation, generation);
        assert_eq!(view.tasks.len(), 0);
        assert_eq!(view.mode, QueueMode::Running);
        // Ids are never reused within a chat, so a stale id cannot hit a new task.
        let fresh = chat.enqueue("new").unwrap();
        assert_ne!(fresh, a);
        assert_eq!(chat.task(a), None);
    }

    #[test]
    fn the_generation_wraps_on_purpose() {
        let mut chat = Chat::new(ProviderId::Claude);
        chat.generation = u64::MAX;
        chat.clear();
        assert_eq!(chat.generation(), 0);
    }

    #[test]
    fn every_error_has_its_code_and_a_message() {
        let id = TaskId::from_raw(1);
        let errors = [
            (ChatError::PromptEmpty, "PROMPT_EMPTY"),
            (ChatError::PromptTooLong, "PROMPT_TOO_LONG"),
            (ChatError::TaskNotFound(id), "TASK_NOT_FOUND"),
            (ChatError::TaskNotQueued(id), "TASK_NOT_QUEUED"),
            (ChatError::ProviderLocked, "PROVIDER_LOCKED"),
            (
                ChatError::ProviderUnavailable {
                    provider: ProviderId::Codex,
                    reason: "not installed".into(),
                },
                "PROVIDER_UNAVAILABLE",
            ),
            (ChatError::QueueFull, "QUEUE_FULL"),
            (ChatError::Conflict("busy".into()), "CONFLICT"),
        ];
        for (error, code) in errors {
            assert_eq!(error.code(), code);
            let json = serde_json::to_value(&error).unwrap();
            assert_eq!(json["code"], code);
            assert_eq!(json["message"], error.to_string());
            assert_ne!(error.to_string(), "");
        }
        assert_eq!(
            ChatError::ProviderUnavailable {
                provider: ProviderId::Codex,
                reason: "not installed".into()
            }
            .to_string(),
            "codex is not available: not installed"
        );
    }

    #[test]
    fn the_state_serializes_in_the_shape_of_the_contract() {
        let (mut chat, [a, ..]) = queued();
        chat.begin_next().unwrap();
        chat.finish(a, Outcome::Done(result("hi"))).unwrap();
        let json = serde_json::to_value(chat.view()).unwrap();
        assert_eq!(json["generation"], 0);
        assert_eq!(json["mode"], "running");
        assert_eq!(json["provider"], "claude");
        assert_eq!(json["provider_locked"], true);
        let tasks = json["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["id"], a.get(), "the id is a bare number");
        assert_eq!(tasks[0]["state"], "done");
        assert_eq!(tasks[0]["error"], serde_json::Value::Null);
        assert_eq!(tasks[0]["result"]["text"], "hi");
        assert_eq!(tasks[1]["state"], "queued");
        assert_eq!(tasks[1]["provider"], "claude");
    }

    #[test]
    fn a_provider_name_round_trips() {
        for id in [ProviderId::Claude, ProviderId::Codex] {
            assert_eq!(id.as_str().parse::<ProviderId>(), Ok(id));
            assert_eq!(serde_json::to_value(id).unwrap(), id.as_str());
        }
        assert_eq!(
            "gemini".parse::<ProviderId>(),
            Err(UnknownProvider("gemini".into()))
        );
    }

    #[test]
    fn a_result_event_becomes_a_task_result() {
        let event = crate::composer::Event::Result {
            success: true,
            subtype: "success".into(),
            text: "built".into(),
            duration_ms: Some(1200),
            num_turns: Some(3),
            cost_usd: Some(0.25),
            session_id: Some("s-1".into()),
            usage: serde_json::json!({"input_tokens": 10, "output_tokens": 20}),
            permission_denials: vec!["Bash".into()],
            raw: String::new(),
        };
        let result = TaskResult::from_event(&event).unwrap();
        assert_eq!(result.text, "built");
        assert_eq!(result.duration_ms, Some(1200));
        assert_eq!(result.num_turns, Some(3));
        assert_eq!(result.cost_usd, Some(0.25));
        assert_eq!(result.input_tokens, Some(10));
        assert_eq!(result.output_tokens, Some(20));
        assert_eq!(result.session_id.as_deref(), Some("s-1"));
        assert_eq!(result.denied_tools, ["Bash"]);
        let other = crate::composer::Event::Text { text: "x".into() };
        assert_eq!(TaskResult::from_event(&other), None);
    }

    // ---- invariants over generated operation sequences -----------------------------------
    //
    // Deterministic (TST-04): a fixed-seed xorshift generator drives many sequences, so a
    // failure reproduces from its seed. (The workspace's `proptest` is not a dev-dependency of
    // this crate, and adding one would change Cargo.lock for no benefit here.)

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(bound.max(1)).unwrap()).unwrap()
        }
    }

    fn assert_invariants(chat: &Chat, context: &str) {
        let tasks = chat.tasks();
        let running = tasks
            .iter()
            .filter(|task| task.state == TaskState::Running)
            .count();
        assert!(running <= 1, "{context}: {running} tasks run");
        let first_queued = tasks
            .iter()
            .position(|task| task.state == TaskState::Queued)
            .unwrap_or(tasks.len());
        assert!(
            tasks[first_queued..]
                .iter()
                .all(|task| task.state == TaskState::Queued),
            "{context}: the queued tasks are not a suffix: {:?}",
            states(chat)
        );
        let mut ids: Vec<_> = tasks.iter().map(|task| task.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), tasks.len(), "{context}: ids repeat");
        assert!(tasks.len() - first_queued <= QUEUE_CAPACITY, "{context}");
        let view = chat.view();
        assert_eq!(view.provider_locked, !tasks.is_empty(), "{context}");
        if running == 1 {
            assert_eq!(
                view.mode,
                QueueMode::Running,
                "{context}: paused yet running"
            );
        }
        if view.mode == QueueMode::Paused {
            assert!(
                tasks
                    .iter()
                    .any(|task| matches!(task.state, TaskState::Failed | TaskState::Cancelled)),
                "{context}: paused without a reason"
            );
        }
        for task in tasks {
            assert_eq!(
                task.error.is_some(),
                task.state == TaskState::Failed,
                "{context}: error only on failed tasks"
            );
            match task.state {
                TaskState::Done => assert!(task.result.is_some(), "{context}"),
                TaskState::Queued | TaskState::Running => {
                    assert!(task.result.is_none() && task.error.is_none(), "{context}");
                }
                TaskState::Failed | TaskState::Cancelled => {}
            }
            assert_eq!(task.prompt, task.prompt.trim(), "{context}");
            assert!(!task.prompt.is_empty(), "{context}");
        }
    }

    /// A known id, sometimes an unknown one.
    fn pick(rng: &mut Rng, before: &ChatState) -> TaskId {
        if before.tasks.is_empty() || rng.below(10) == 0 {
            TaskId::from_raw(1_000_000 + rng.next() % 5)
        } else {
            before.tasks[rng.below(before.tasks.len())].id
        }
    }

    fn queued_ids(tasks: &[TaskView]) -> Vec<TaskId> {
        tasks
            .iter()
            .filter(|task| task.state == TaskState::Queued)
            .map(|task| task.id)
            .collect()
    }

    fn not_queued(tasks: &[TaskView]) -> Vec<TaskView> {
        tasks
            .iter()
            .filter(|task| task.state != TaskState::Queued)
            .cloned()
            .collect()
    }

    fn op_enqueue(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        let prompt = ["a", "  b  ", "", "   ", "c d"][rng.below(5)];
        match chat.enqueue(prompt) {
            Ok(id) => {
                let view = chat.view();
                assert_eq!(view.tasks.len(), before.tasks.len() + 1, "{context}");
                let last = view.tasks.last().unwrap();
                assert_eq!((last.id, last.state), (id, TaskState::Queued));
                assert_eq!(last.prompt, prompt.trim());
            }
            Err(error) => {
                assert!(
                    matches!(error, ChatError::PromptEmpty | ChatError::QueueFull),
                    "{context}: {error}"
                );
                assert_eq!(chat.view(), *before, "{context}: {error} changed state");
            }
        }
    }

    fn op_edit(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        let id = pick(rng, before);
        let prompt = ["x", "", "  y "][rng.below(3)];
        let expected = before.tasks.iter().find(|task| task.id == id);
        match chat.edit(id, prompt) {
            Ok(()) => {
                assert_eq!(expected.map(|t| t.state), Some(TaskState::Queued));
                let after = chat.view();
                for (old, new) in before.tasks.iter().zip(&after.tasks) {
                    if old.id == id {
                        assert_eq!(new.prompt, prompt.trim());
                        assert_eq!((old.state, old.id), (new.state, new.id));
                    } else {
                        assert_eq!(old, new, "{context}: an edit touched another task");
                    }
                }
            }
            Err(error) => {
                let code = error.code();
                if prompt.trim().is_empty() {
                    assert_eq!(code, "PROMPT_EMPTY", "{context}");
                } else if let Some(task) = expected {
                    assert_eq!(code, "TASK_NOT_QUEUED", "{context}");
                    assert_ne!(task.state, TaskState::Queued);
                } else {
                    assert_eq!(code, "TASK_NOT_FOUND", "{context}");
                }
                assert_eq!(chat.view(), *before, "{context}");
            }
        }
    }

    fn assert_refused_by_id(error: &ChatError, context: &str) {
        assert!(
            matches!(
                error,
                ChatError::TaskNotFound(_) | ChatError::TaskNotQueued(_)
            ),
            "{context}: {error}"
        );
    }

    fn op_remove(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        let id = pick(rng, before);
        match chat.remove(id) {
            Ok(()) => {
                let removed = before.tasks.iter().find(|t| t.id == id).unwrap();
                assert_eq!(removed.state, TaskState::Queued, "{context}");
                let mut expected = before.tasks.clone();
                expected.retain(|task| task.id != id);
                assert_eq!(chat.view().tasks, expected, "{context}");
            }
            Err(error) => {
                assert_refused_by_id(&error, context);
                assert_eq!(chat.view(), *before, "{context}");
            }
        }
    }

    fn op_move(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        let id = pick(rng, before);
        let to = rng.below(queued_ids(&before.tasks).len() + 2);
        match chat.move_task(id, to) {
            Ok(()) => {
                let after = chat.view();
                let mut expected = queued_ids(&before.tasks);
                let from = expected.iter().position(|queued| *queued == id).unwrap();
                expected.remove(from);
                expected.insert(to.min(expected.len()), id);
                assert_eq!(queued_ids(&after.tasks), expected, "{context}");
                assert_eq!(
                    not_queued(&after.tasks),
                    not_queued(&before.tasks),
                    "{context}"
                );
            }
            Err(error) => {
                assert_refused_by_id(&error, context);
                assert_eq!(chat.view(), *before, "{context}");
            }
        }
    }

    fn op_begin_next(chat: &mut Chat, before: &ChatState, context: &str) {
        let running = before
            .tasks
            .iter()
            .any(|task| task.state == TaskState::Running);
        let first = before.tasks.iter().find(|t| t.state == TaskState::Queued);
        if let Some(task) = chat.begin_next() {
            assert!(!running && before.mode == QueueMode::Running, "{context}");
            assert_eq!(
                Some(task.id),
                first.map(|t| t.id),
                "{context}: not the first queued"
            );
            assert_eq!(task.state, TaskState::Running);
        } else {
            assert!(
                running || before.mode == QueueMode::Paused || first.is_none(),
                "{context}: nothing started although it could"
            );
            assert_eq!(chat.view(), *before, "{context}");
        }
    }

    fn op_finish(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        let id = pick(rng, before);
        let outcome = match rng.below(3) {
            0 => Outcome::Done(result("done")),
            1 => Outcome::Failed {
                reason: "failed".into(),
                result: (rng.below(2) == 0).then(|| result("partial")),
            },
            _ => Outcome::Cancelled { result: None },
        };
        let expected_state = match &outcome {
            Outcome::Done(_) => TaskState::Done,
            Outcome::Failed { .. } => TaskState::Failed,
            Outcome::Cancelled { .. } => TaskState::Cancelled,
        };
        let was = before.tasks.iter().find(|task| task.id == id);
        match chat.finish(id, outcome) {
            Ok(()) => {
                assert_eq!(was.map(|t| t.state), Some(TaskState::Running), "{context}");
                let after = chat.view();
                let task = after.tasks.iter().find(|t| t.id == id).unwrap();
                assert_eq!(task.state, expected_state, "{context}");
                assert_eq!(
                    after.mode == QueueMode::Paused,
                    expected_state != TaskState::Done || before.mode == QueueMode::Paused,
                    "{context}"
                );
            }
            Err(error) => {
                assert!(
                    matches!(error, ChatError::TaskNotFound(_) | ChatError::Conflict(_)),
                    "{context}"
                );
                assert_eq!(chat.view(), *before, "{context}");
            }
        }
    }

    fn op_clear_or_provider(chat: &mut Chat, rng: &mut Rng, before: &ChatState, context: &str) {
        if rng.below(3) == 0 {
            chat.clear();
            let after = chat.view();
            assert_ne!(after.generation, before.generation, "{context}");
            assert!(after.tasks.is_empty(), "{context}");
            assert_eq!(after.mode, QueueMode::Running, "{context}");
            return;
        }
        let provider = [ProviderId::Claude, ProviderId::Codex][rng.below(2)];
        match chat.set_provider(provider) {
            Ok(()) => {
                assert!(before.tasks.is_empty(), "{context}");
                assert_eq!(chat.view().provider, provider);
            }
            Err(error) => {
                assert_eq!(error, ChatError::ProviderLocked, "{context}");
                assert!(!before.tasks.is_empty(), "{context}");
                assert_eq!(chat.view(), *before, "{context}");
            }
        }
    }

    /// Applies one random operation and checks what it promises.
    fn step(chat: &mut Chat, rng: &mut Rng, context: &str) {
        let before = chat.view();
        match rng.below(12) {
            0..=2 => op_enqueue(chat, rng, &before, context),
            3 => op_edit(chat, rng, &before, context),
            4 => op_remove(chat, rng, &before, context),
            5 => op_move(chat, rng, &before, context),
            6..=7 => op_begin_next(chat, &before, context),
            8..=9 => op_finish(chat, rng, &before, context),
            10 => {
                let changed = chat.resume();
                assert_eq!(changed, before.mode == QueueMode::Paused, "{context}");
                assert_eq!(chat.view().mode, QueueMode::Running);
                assert_eq!(chat.view().tasks, before.tasks);
            }
            _ => op_clear_or_provider(chat, rng, &before, context),
        }
        assert_invariants(chat, context);
    }

    #[test]
    fn random_operation_sequences_never_break_the_invariants() {
        for seed in 1..=300_u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut chat = Chat::new(ProviderId::Claude);
            for index in 0..400 {
                step(&mut chat, &mut rng, &format!("seed {seed}, step {index}"));
            }
        }
    }

    #[test]
    fn a_long_enqueue_heavy_sequence_reaches_the_queue_limit_without_breaking_it() {
        let mut rng = Rng(0xDEAD_BEEF_1234_5679);
        let mut chat = Chat::new(ProviderId::Claude);
        let mut full = 0;
        for _ in 0..200 {
            match chat.enqueue("x") {
                Ok(_) => {}
                Err(ChatError::QueueFull) => full += 1,
                Err(other) => panic!("{other}"),
            }
            if rng.below(8) == 0 {
                chat.begin_next();
            }
            assert_invariants(&chat, "enqueue heavy");
        }
        assert!(full > 0, "the limit was reached");
    }
}

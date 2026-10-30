//! The data of a chat: tasks, their states and what a finished run reports.
//!
//! Everything here serializes to the window in `snake_case`; the shapes are the contract of
//! the chat commands.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::composer::Event;

/// The id of a task. Handed out by the chat itself from a private counter (never by the
/// project's id source, which a chat must not move), and unique within one chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(u64);

impl TaskId {
    /// The id for a number that came from the window. Whether a task has it is for the chat
    /// to say ([`super::ChatError::TaskNotFound`]).
    #[must_use]
    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The number of the id.
    #[must_use]
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The agent that answers a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    /// Claude Code.
    Claude,
    /// Codex CLI.
    Codex,
}

impl ProviderId {
    /// The name the window uses: `claude` or `codex`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A provider name that is none of [`ProviderId`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown provider {0:?}")]
pub struct UnknownProvider(pub String);

impl FromStr for ProviderId {
    type Err = UnknownProvider;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(UnknownProvider(other.to_owned())),
        }
    }
}

/// Where a task is in its life. `Queued` leads to `Running`, which ends in one of the three
/// final states; a final state never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Waiting for its turn.
    Queued,
    /// The agent works on it.
    Running,
    /// The agent finished it.
    Done,
    /// The run failed; [`TaskView::error`] says why.
    Failed,
    /// The user stopped it.
    Cancelled,
}

/// Whether the queue starts the next task by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueMode {
    /// The next queued task starts when the last one ends. Also the mode of an idle chat.
    Running,
    /// A task failed or was stopped: the queued tasks wait for the user.
    Paused,
}

/// What a run reports when it ends.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskResult {
    /// The agent's final text.
    pub text: String,
    /// How long the run took.
    pub duration_ms: Option<u64>,
    /// The number of turns.
    pub num_turns: Option<u64>,
    /// The estimated cost in US dollars.
    pub cost_usd: Option<f64>,
    /// The input tokens, as the provider reports them.
    pub input_tokens: Option<u64>,
    /// The output tokens, as the provider reports them.
    pub output_tokens: Option<u64>,
    /// The provider's session id.
    pub session_id: Option<String>,
    /// The tools that were denied.
    pub denied_tools: Vec<String>,
}

impl TaskResult {
    /// The result of a run's [`Event::Result`]; `None` for any other event.
    #[must_use]
    pub fn from_event(event: &Event) -> Option<Self> {
        let Event::Result {
            text,
            duration_ms,
            num_turns,
            cost_usd,
            session_id,
            usage,
            permission_denials,
            ..
        } = event
        else {
            return None;
        };
        Some(Self {
            text: text.clone(),
            duration_ms: *duration_ms,
            num_turns: *num_turns,
            cost_usd: *cost_usd,
            input_tokens: usage["input_tokens"].as_u64(),
            output_tokens: usage["output_tokens"].as_u64(),
            session_id: session_id.clone(),
            denied_tools: permission_denials.clone(),
        })
    }
}

/// One message of the user and what became of it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskView {
    /// The task.
    pub id: TaskId,
    /// What the user asked, trimmed.
    pub prompt: String,
    /// The agent that answers it.
    pub provider: ProviderId,
    /// Where it is.
    pub state: TaskState,
    /// For a failed task: why.
    pub error: Option<String>,
    /// What the run reported, once it did.
    pub result: Option<TaskResult>,
}

/// The whole chat as the window shows it. The tasks are in thread order: the finished ones,
/// then the running one, then the queued ones in queue order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatState {
    /// Changes whenever the thread is wiped, so that a window can tell stale events apart.
    pub generation: u64,
    /// Whether the queue goes on by itself.
    pub mode: QueueMode,
    /// The agent of this conversation.
    pub provider: ProviderId,
    /// Whether the provider can no longer change (the thread has tasks).
    pub provider_locked: bool,
    /// The tasks.
    pub tasks: Vec<TaskView>,
}

/// How a run ended, for [`super::Chat::finish`].
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The run finished its task.
    Done(TaskResult),
    /// The run failed.
    Failed {
        /// Why.
        reason: String,
        /// What it reported before, if anything.
        result: Option<TaskResult>,
    },
    /// The user stopped the run.
    Cancelled {
        /// What it reported before, if anything.
        result: Option<TaskResult>,
    },
}

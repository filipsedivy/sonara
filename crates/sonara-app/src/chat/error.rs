//! The errors of the chat, with the codes the window receives.

use serde::{Serialize, Serializer};

use super::task::{ProviderId, TaskId};
use crate::composer::PromptError;
use crate::composer::invocation::MAX_PROMPT_CHARS;

/// How many tasks may wait in the queue at once.
pub const QUEUE_CAPACITY: usize = 50;

/// Why a chat call was refused. Every variant has a stable [`ChatError::code`], and the error
/// serializes as `{ "code", "message" }` like the other errors the window gets.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatError {
    /// There is nothing to ask.
    #[error("write a message first")]
    PromptEmpty,
    /// The prompt is longer than [`MAX_PROMPT_CHARS`].
    #[error("the message is longer than {MAX_PROMPT_CHARS} characters")]
    PromptTooLong,
    /// No task has this id (it may have been cleared with the thread).
    #[error("there is no task {0}")]
    TaskNotFound(TaskId),
    /// The task is no longer waiting: only queued tasks can be edited, removed or moved.
    #[error("task {0} is no longer queued")]
    TaskNotQueued(TaskId),
    /// The thread has tasks, so its provider cannot change any more.
    #[error("the provider cannot change once the conversation has started; clear it first")]
    ProviderLocked,
    /// The provider is not registered or cannot run now.
    #[error("{provider} is not available: {reason}")]
    ProviderUnavailable {
        /// The provider.
        provider: ProviderId,
        /// What is wrong and how to fix it.
        reason: String,
    },
    /// The queue holds [`QUEUE_CAPACITY`] tasks already.
    #[error("the queue is full ({QUEUE_CAPACITY} waiting tasks)")]
    QueueFull,
    /// The call does not fit the state the chat is in.
    #[error("{0}")]
    Conflict(String),
}

impl ChatError {
    /// The stable code of the error: `PROMPT_EMPTY`, `PROMPT_TOO_LONG`, `TASK_NOT_FOUND`,
    /// `TASK_NOT_QUEUED`, `PROVIDER_LOCKED`, `PROVIDER_UNAVAILABLE`, `QUEUE_FULL` or `CONFLICT`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::PromptEmpty => "PROMPT_EMPTY",
            Self::PromptTooLong => "PROMPT_TOO_LONG",
            Self::TaskNotFound(_) => "TASK_NOT_FOUND",
            Self::TaskNotQueued(_) => "TASK_NOT_QUEUED",
            Self::ProviderLocked => "PROVIDER_LOCKED",
            Self::ProviderUnavailable { .. } => "PROVIDER_UNAVAILABLE",
            Self::QueueFull => "QUEUE_FULL",
            Self::Conflict(_) => "CONFLICT",
        }
    }
}

impl From<PromptError> for ChatError {
    fn from(error: PromptError) -> Self {
        match error {
            PromptError::Empty => Self::PromptEmpty,
            PromptError::TooLong => Self::PromptTooLong,
        }
    }
}

impl Serialize for ChatError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ChatError", 2)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

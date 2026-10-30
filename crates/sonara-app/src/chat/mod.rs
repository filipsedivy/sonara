//! The AI Chat: a conversation with an agent that works in the open project, as a queue of
//! tasks that run one after the other.
//!
//! The state lives here, in Rust, and the window is a view of it: a reloaded window asks for a
//! snapshot. Nothing of the chat is ever written to disk or to the project, and no project id
//! is spent on it (the privacy test in `tests/chat_save.rs` pins that).
//!
//! - [`mod@state`]: the pure state machine ([`Chat`]).
//! - [`task`], [`mod@error`]: the data and the errors that cross to the window.

pub mod error;
pub mod state;
pub mod task;

pub use error::{ChatError, QUEUE_CAPACITY};
pub use state::Chat;
pub use task::{
    ChatState, Outcome, ProviderId, QueueMode, TaskId, TaskResult, TaskState, TaskView,
    UnknownProvider,
};

//! The AI Chat: a conversation with an agent that works in the open project, as a queue of
//! tasks that run one after the other.
//!
//! The state lives here, in Rust, and the window is a view of it: a reloaded window asks for a
//! snapshot. Nothing of the chat is ever written to disk or to the project, and no project id
//! is spent on it (the privacy test in `tests/chat_save.rs` pins that).
//!
//! - [`mod@state`]: the pure state machine ([`Chat`]).
//! - [`task`], [`mod@error`]: the data and the errors that cross to the window.
//! - [`controller`]: runs the tasks one after the other through a provider and tells the
//!   window ([`event`]); [`replay`] gives each task the earlier conversation as context.
//! - [`provider`], [`providers`], [`boundary`]: the seam to the agent CLIs, and the adapters
//!   for Claude Code and Codex CLI. Every provider runs under the safety boundary of the
//!   composer, with one known gap: a Codex run still reads the user's global `AGENTS.md`
//!   (ADR 0011).

pub mod boundary;
pub mod controller;
pub mod error;
pub mod event;
pub mod provider;
pub mod providers;
pub mod replay;
pub mod state;
pub mod task;

pub use controller::{ChatConfig, ChatController, Sink};
pub use error::{ChatError, QUEUE_CAPACITY};
pub use event::{ChatEvent, ChatSnapshot, TaskLog};
pub use provider::{Availability, Provider, ProviderStatus, Turn};
pub use providers::{ClaudeProvider, CodexProvider};
pub use state::Chat;
pub use task::{
    ChatState, Outcome, ProviderId, QueueMode, TaskId, TaskResult, TaskState, TaskView,
    UnknownProvider,
};

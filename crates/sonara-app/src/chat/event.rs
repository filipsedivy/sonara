//! What the chat sends to its window, and what the window asks for to catch up.

use serde::Serialize;

use super::task::{ChatState, TaskId};
use crate::composer::Event;

/// An event for the window, delivered in the order the chat's state changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[expect(
    clippy::large_enum_variant,
    reason = "PERF-03: an event is built once and moved to the sink; boxing the run event would \
              allocate for every streamed text delta"
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    /// The state changed; it is the whole new state.
    Changed {
        /// The state.
        state: ChatState,
    },
    /// A run reports something. Only events of the current generation are sent.
    Run {
        /// The task that runs.
        task_id: TaskId,
        /// The generation of the chat the task belongs to.
        generation: u64,
        /// What happened.
        event: Event,
    },
}

/// The events of one task so far, bounded; the window replays them to show the steps.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskLog {
    /// The task.
    pub task_id: TaskId,
    /// Its events, oldest first, with adjacent text coalesced.
    pub events: Vec<Event>,
}

/// The state and the logs: all a reloaded window needs to show the chat again.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatSnapshot {
    /// The state.
    pub state: ChatState,
    /// The logs of the tasks that have started, in thread order.
    pub logs: Vec<TaskLog>,
}

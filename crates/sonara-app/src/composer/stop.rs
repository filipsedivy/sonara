//! How a run is stopped: the signals and how long each gets.

use std::time::Duration;

/// The escalation of [`super::Run::stop`]: `SIGINT` at once, `SIGTERM` to the process group
/// after `interrupt_grace`, `SIGKILL` after a further `terminate_grace`. A provider that
/// writes its result on `SIGINT` can take a long grace; one that does not can take a short one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopPolicy {
    /// How long a run gets to finish after `SIGINT`, before `SIGTERM`.
    pub interrupt_grace: Duration,
    /// How long it gets after `SIGTERM`, before `SIGKILL`.
    pub terminate_grace: Duration,
}

impl StopPolicy {
    /// Claude Code's: 10 s after `SIGINT` (it ends the turn and still writes its `result`),
    /// then 5 s after `SIGTERM`.
    pub const CLAUDE: Self = Self {
        interrupt_grace: Duration::from_secs(10),
        terminate_grace: Duration::from_secs(5),
    };
}
